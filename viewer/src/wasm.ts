/**
 * WASM module loading utilities
 *
 * This module provides the interface for loading the PcbEngine from WASM.
 * If the WASM module is not available, it falls back to a mock implementation
 * that provides the same interface for development and testing.
 *
 * Architecture note: The WASM build doesn't include tree-sitter (too complex for WASM),
 * so parsing is done in JavaScript. The WASM engine provides:
 * - load_snapshot(): Load a pre-parsed BoardSnapshot
 * - get_snapshot(): Get the current board state
 * - query_point(): Query components at a point
 *
 * This module provides an adapter (WasmPcbEngineAdapter) that adds load_source()
 * by parsing in JavaScript and calling load_snapshot() on the WASM engine.
 */

import type { BoardSnapshot, ComponentInfo, PadInfo, NetInfo, PinRef, BoardInfo, TraceInfo, TraceSegmentInfo, ViaInfo, ViolationInfo, SilkShape } from './types';
import { pointToSegmentDistance } from './geometry';

/**
 * Recursively convert all BigInt values in an object to plain Numbers.
 * WASM (Rust i64) returns BigInt which can't be mixed with Number in JS arithmetic.
 */
function deepBigIntToNumber(obj: any): any {
  try {
    return JSON.parse(JSON.stringify(obj, (_key, value) =>
      typeof value === 'bigint' ? Number(value) : value
    ));
  } catch {
    return obj;
  }
}

/** Sanitize a WASM-returned snapshot, converting all BigInt values to Number. */
/**
 * JS-side silk clearance check.
 *
 * Silk shapes live only in JS (not in WASM ECS), so this check runs client-side.
 * For each silk segment/circle/arc on a component, checks if it overlaps any
 * copper pad of a DIFFERENT component on the same side. Reports violations
 * when the silk-to-pad distance is less than min_silk_clearance.
 */
export function checkSilkClearance(snapshot: BoardSnapshot, minClearanceNm: number): ViolationInfo[] {
  const violations: ViolationInfo[] = [];
  if (!snapshot.components || snapshot.components.length === 0) return violations;

  // Helper: ensure Number (WASM may return BigInt for nm values)
  const N = (v: any): number => typeof v === 'bigint' ? Number(v) : (v as number);

  // Build pad world positions for all components
  const allPads: { comp: ComponentInfo; pad: PadInfo; wx: number; wy: number; side: 'top' | 'bottom' }[] = [];
  for (const comp of snapshot.components) {
    const rad = (N(comp.rotation_mdeg) / 1000) * (Math.PI / 180);
    const cos = Math.cos(rad);
    const sin = Math.sin(rad);
    for (const pad of comp.pads) {
      const px = N(pad.x_nm), py = N(pad.y_nm);
      const rx = px * cos - py * sin;
      const ry = px * sin + py * cos;
      const side = (N(pad.layer_mask) & 1) ? 'top' as const : 'bottom' as const;
      allPads.push({ comp, pad, wx: N(comp.x_nm) + rx, wy: N(comp.y_nm) + ry, side });
    }
  }

  for (const comp of snapshot.components) {
    if (!comp.silk || comp.silk.length === 0) continue;

    const rad = (N(comp.rotation_mdeg) / 1000) * (Math.PI / 180);
    const cos = Math.cos(rad);
    const sin = Math.sin(rad);
    const compX = N(comp.x_nm), compY = N(comp.y_nm);

    // Which face this component's legend prints on.
    //
    // A shape may state a side - the EasyEDA parser sets one - and the engine
    // does not: a footprint's artwork lives in footprint coordinates and the
    // part decides where it goes. Reading only the shape meant every
    // engine-supplied legend compared `undefined` against a pad's side, matched
    // nothing, and skipped the check without saying so. The part's own pads are
    // the fallback, because a legend prints on the face its copper is on.
    const componentSide = comp.pads.some((pad) => N(pad.layer_mask) & 1)
      ? ('top' as const)
      : ('bottom' as const);

    for (const shape of comp.silk) {
      const silkSide = shape.layer ?? componentSide;

      // Transform silk shape to world coordinates
      if (shape.type === 'segment') {
        const x1 = N(shape.x1), y1 = N(shape.y1), x2 = N(shape.x2), y2 = N(shape.y2);
        const sx1 = compX + x1 * cos - y1 * sin;
        const sy1 = compY + x1 * sin + y1 * cos;
        const sx2 = compX + x2 * cos - y2 * sin;
        const sy2 = compY + x2 * sin + y2 * cos;
        const halfSilk = (N(shape.width) || 150_000) / 2;

        // Check against pads of OTHER components on the same side
        for (const p of allPads) {
          if (p.comp.refdes === comp.refdes) continue; // Skip own pads
          if (p.side !== silkSide) continue; // Different side

          const padRadius = Math.max(N(p.pad.width_nm), N(p.pad.height_nm)) / 2;
          const exclusion = padRadius + halfSilk + minClearanceNm;

          if (segmentNearPoint(sx1, sy1, sx2, sy2, p.wx, p.wy, exclusion)) {
            violations.push({
              kind: 'silk-clearance',
              x_nm: p.wx,
              y_nm: p.wy,
              message: `${comp.refdes} silk ↔ ${p.comp.refdes}.${p.pad.number}: Silk-to-pad clearance violation`,
            });
            break; // One violation per silk shape is enough
          }
        }
      } else if (shape.type === 'circle') {
        const scx = N(shape.cx), scy = N(shape.cy);
        const cx = compX + scx * cos - scy * sin;
        const cy = compY + scx * sin + scy * cos;
        const halfSilk = (N(shape.width) || 150_000) / 2;
        const outerRadius = N(shape.radius) + halfSilk;

        for (const p of allPads) {
          if (p.comp.refdes === comp.refdes) continue;
          if (p.side !== silkSide) continue;

          const padRadius = Math.max(N(p.pad.width_nm), N(p.pad.height_nm)) / 2;
          const dist = Math.hypot(cx - p.wx, cy - p.wy);

          if (dist < outerRadius + padRadius + minClearanceNm) {
            violations.push({
              kind: 'silk-clearance',
              x_nm: p.wx,
              y_nm: p.wy,
              message: `${comp.refdes} silk ↔ ${p.comp.refdes}.${p.pad.number}: Silk-to-pad clearance violation`,
            });
            break;
          }
        }
      }
      // Arc shapes: skip for now (uncommon to overlap pads)
    }
  }

  return violations;
}

/** Point-to-segment distance check (used by silk clearance). */
function segmentNearPoint(
  sx: number, sy: number, ex: number, ey: number,
  px: number, py: number, radius: number,
): boolean {
  const dx = ex - sx;
  const dy = ey - sy;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1) return Math.hypot(px - sx, py - sy) <= radius;
  let t = ((px - sx) * dx + (py - sy) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  const nearX = sx + t * dx;
  const nearY = sy + t * dy;
  return Math.hypot(px - nearX, py - nearY) <= radius;
}

function sanitizeSnapshot(snap: any): BoardSnapshot {
  return deepBigIntToNumber(snap) as BoardSnapshot;
}

// ---------------------------------------------------------------------------
// Dynamic footprint registry — populated from EasyEDA API at insert time
// ---------------------------------------------------------------------------

/**
 * Registry of footprint pad definitions fetched from EasyEDA.
 * Key: footprint/package name as it appears in .cypcb source (e.g. "LQFP-48", "QFN-24").
 * Value: PadInfo[] with positions relative to component origin.
 *
 * When a user inserts a JLCPCB component, the insert flow pre-fetches the
 * footprint from EasyEDA and registers it here before the editor re-parses.
 * The parser's getFootprintPads() checks this registry first.
 */
const dynamicFootprintRegistry = new Map<string, { pads: PadInfo[]; silk: SilkShape[] }>();

/**
 * Register a dynamic footprint for use by the parser.
 */
export function registerDynamicFootprint(packageName: string, pads: PadInfo[], silk: SilkShape[] = []): void {
  dynamicFootprintRegistry.set(packageName, { pads, silk });

  // And tell the engine, which is the half that was missing: the map above
  // feeds the JavaScript parser, so the pads were drawn while the board model
  // held an unknown footprint - nothing to measure clearance against and
  // nothing to export. A fetch can land before the engine exists, so
  // `replayRegisteredFootprints` hands over whatever arrived early.
  if (engineInstance) {
    const error = engineInstance.register_footprint(packageName, pads, silk);
    if (error) {
      console.warn(`[Footprint] The engine refused ${packageName}: ${error}`);
    }
  }
}

/**
 * Hand the engine every footprint that was registered before it existed.
 *
 * Called once the engine is created. Without it a part fetched during startup
 * is known to the drawing code and unknown to the model for the rest of the
 * session.
 */
function replayRegisteredFootprints(engine: PcbEngine): void {
  for (const [name, { pads, silk }] of dynamicFootprintRegistry) {
    const error = engine.register_footprint(name, pads, silk);
    if (error) {
      console.warn(`[Footprint] The engine refused ${name}: ${error}`);
    }
  }
}

/**
 * Check if a dynamic footprint is registered for the given package name.
 */
export function hasDynamicFootprint(packageName: string): boolean {
  return dynamicFootprintRegistry.has(packageName);
}

/**
 * Get all registered dynamic footprint names (for debugging).
 */
export function getRegisteredFootprints(): string[] {
  return Array.from(dynamicFootprintRegistry.keys());
}

/**
 * Registry mapping package names to 3D model UUIDs.
 * Populated during footprint fetch. Used by the parser to set model_3d on components.
 */
const model3dRegistry = new Map<string, string>();

/**
 * Register a 3D model UUID for a package name.
 */
export function register3DModel(packageName: string, uuid: string): void {
  model3dRegistry.set(packageName, uuid);
  console.log(`[3D] Registered model for ${packageName}: ${uuid}`);
}

/**
 * Get a registered 3D model UUID for a package name.
 */
export function get3DModelUuid(packageName: string): string | null {
  return model3dRegistry.get(packageName) ?? null;
}

/**
 * Interface for the PCB rendering engine exposed from Rust/WASM
 */
export interface PcbEngine {
  /**
   * Teach the engine a footprint that did not come from the source file.
   *
   * A part fetched from a supplier arrives as pads and silk artwork with no
   * `footprint` block behind it. Without this the engine never hears about it:
   * the JavaScript side draws the pads and the board model has an unknown
   * footprint, so DRC measures nothing there and an export leaves it out.
   *
   * Returns an empty string on success, an error message otherwise.
   */
  register_footprint(name: string, pads: PadInfo[], silk: SilkShape[]): string;
  /** Load and parse a .cypcb source file, returns error message if failed */
  load_source(source: string): string;
  /** Load routing results from .ses file content */
  load_routes(sesContent: string): void;
  /** Get the current board state as a snapshot */
  get_snapshot(): BoardSnapshot;
  /** Query what's at a specific point (in nanometers), returns list of entity descriptions */
  query_point(x_nm: number, y_nm: number): string[];

  // -- Trace mutation API (T03) --

  /**
   * Add a trace to the board.
   * @param net_name Net this trace belongs to
   * @param layer "Top" | "Bottom" | "Inner0" etc.
   * @param width_nm Trace width in nanometers
   * @param segments Flat coordinate array [x1,y1,x2,y2, x3,y3,x4,y4, ...] (4 values per segment, in nm)
   * @returns Entity index (u32), or 0xFFFFFFFF on error
   */
  add_trace(net_name: string, layer: string, width_nm: number, segments: number[]): number;

  /**
   * Remove a trace by entity index.
   * @returns true if found and removed
   */
  remove_trace(trace_id: number): boolean;

  /**
   * Find a trace entity at a given point with tolerance.
   * @param x_nm X coordinate in nanometers
   * @param y_nm Y coordinate in nanometers
   * @param tolerance_nm Search radius in nanometers
   * @returns Entity index, or 0xFFFFFFFF if none found
   */
  get_trace_at_point(x_nm: number, y_nm: number, tolerance_nm: number): number;

  /**
   * Run DRC and return the number of violations.
   */
  run_drc_incremental(): number;

  /**
   * Get the number of trace entities.
   */
  trace_count(): number;

  /**
   * Export all traces and vias as DSL trace blocks.
   * Returns empty string if no traces exist.
   */
  export_traces_as_dsl(): string;

  /** Get minimum copper clearance in nanometers from active design rules. */
  get_min_clearance_nm(): number;

  /**
   * Rotate a component by delta millidegrees.
   * @param refdes Reference designator (e.g. "R1")
   * @param delta_mdeg Rotation delta in millidegrees (90° = 90000)
   * @returns true if component found and rotated, false otherwise
   */
  rotate_component(refdes: string, delta_mdeg: number): boolean;

  /**
   * Set the board outline size.
   * @param width_nm Board width in nanometers
   * @param height_nm Board height in nanometers
   * @returns true if board exists and was resized, false otherwise
   */
  set_board_size(width_nm: number, height_nm: number): boolean;

  /**
   * Run the built-in A* autorouter.
   * Returns JSON: {"ok":true,"routed":N,"unrouted":N} or {"ok":false,"error":"..."}
   */
  auto_route(): string;

  /**
   * Run the autorouter with custom tuning parameters.
   * @param params JSON string: {"via_cost":N,"layer_preference":N,"roundness":N,"density":N}
   * Returns JSON: {"ok":true,"routed":N,"unrouted":N} or {"ok":false,"error":"..."}
   */
  auto_route_with_params(params: string): string;

  /**
   * Generate multiple routing variants with different strategies/configs.
   * Returns JSON array of VariantResult objects, sorted by composite score (best first).
   * On error, returns {"ok":false,"error":"..."}.
   */
  auto_route_variants(): string;

  /** Run routing with debug output — returns JSON with pipeline stages */
  auto_route_debug(params: string): string;

  /** Free the engine (for WASM memory management) */
  free?(): void;
}

/**
 * Raw WASM PcbEngine interface (what Rust actually exports)
 */
interface WasmPcbEngine {
  load_snapshot(snapshot: BoardSnapshot): string;
  get_snapshot(): BoardSnapshot;
  query_point(x_nm: bigint, y_nm: bigint): string[];
  add_trace_json(net_name: string, layer_str: string, width_nm: bigint, segments_json: string): number;
  remove_trace(trace_id: number): boolean;
  get_trace_at_point(x_nm: bigint, y_nm: bigint, tolerance_nm: bigint): number;
  run_drc_incremental(): number;
  trace_count(): number;
  export_traces_as_dsl(): string;
  get_min_clearance_nm(): number;
  get_violations_json(): string;
  rotate_component(refdes: string, delta_mdeg: number): boolean;
  set_board_size(width_nm: bigint, height_nm: bigint): boolean;
  auto_route(): string;
  auto_route_with_params(params_json: string): string;
  auto_route_variants(): string;
  auto_route_debug(params_json: string): string;
  register_footprint(name: string, pads_json: string, silk_json: string): string;
  free(): void;
}

let wasmModule: any = null;
let engineInstance: PcbEngine | null = null;

// ============================================================================
// Shared parsing utilities (used by both Mock and WASM adapter)
// ============================================================================

/**
 * Parse a unit value to nanometers.
 */
function parseUnit(value: number, unit: string): number {
  switch (unit) {
    case 'mm':
      return Math.round(value * 1_000_000);
    case 'mil':
      return Math.round(value * 25_400);
    case 'inch':
      return Math.round(value * 25_400_000);
    default:
      return Math.round(value * 1_000_000);
  }
}

/**
 * Get standard pad definitions for common footprints.
 * First checks the dynamic footprint registry (populated from EasyEDA API),
 * then falls back to hardcoded templates for common packages.
 */
function getFootprintPads(footprint: string): PadInfo[] {
  // Check dynamic registry first (populated by JLCPCB component inserts)
  const dynamic = dynamicFootprintRegistry.get(footprint);
  if (dynamic) return dynamic.pads;

  // Hardcoded templates for common packages (matches Rust footprint library)
  // All coordinates are in nanometers, relative to component origin.
  // layer_mask: 1 = TopCopper (SMD), 3 = TopCopper|BottomCopper (THT)
  const padTemplates: Record<string, PadInfo[]> = {
    // 0402 (1005 metric): pad_span=1.0mm, pad=0.6×0.5mm
    '0402': [
      { number: '1', x_nm: -500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm:  500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // 0603 (1608 metric): pad_span=1.6mm, pad=0.9×0.95mm
    '0603': [
      { number: '1', x_nm: -800_000, y_nm: 0, width_nm: 900_000, height_nm: 950_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm:  800_000, y_nm: 0, width_nm: 900_000, height_nm: 950_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // 0805 (2012 metric): pad_span=1.9mm, pad=1.0×1.45mm
    '0805': [
      { number: '1', x_nm: -950_000, y_nm: 0, width_nm: 1_000_000, height_nm: 1_450_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm:  950_000, y_nm: 0, width_nm: 1_000_000, height_nm: 1_450_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // 1206 (3216 metric): pad_span=3.4mm, pad=1.15×1.8mm
    '1206': [
      { number: '1', x_nm: -1_700_000, y_nm: 0, width_nm: 1_150_000, height_nm: 1_800_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm:  1_700_000, y_nm: 0, width_nm: 1_150_000, height_nm: 1_800_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // PIN-HDR-1x2: 100mil (2.54mm) pitch, drill=1.0mm, pad=1.7mm
    // Pin 1 square (rect), Pin 2 round (circle)
    'PIN-HDR-1x2': [
      { number: '1', x_nm: -1_270_000, y_nm: 0, width_nm: 1_700_000, height_nm: 1_700_000, shape: 'rect',   layer_mask: 3, drill_nm: 1_000_000 },
      { number: '2', x_nm:  1_270_000, y_nm: 0, width_nm: 1_700_000, height_nm: 1_700_000, shape: 'circle', layer_mask: 3, drill_nm: 1_000_000 },
    ],
    // SOT-23: 3-pin transistor/regulator, pad=0.6×1.0mm
    'SOT-23': [
      { number: '1', x_nm: -950_000, y_nm: -1_000_000, width_nm: 600_000, height_nm: 1_000_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm:  950_000, y_nm: -1_000_000, width_nm: 600_000, height_nm: 1_000_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '3', x_nm:        0, y_nm:  1_000_000, width_nm: 600_000, height_nm: 1_000_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // SOIC-8: row_span=5.4mm (half=2.7mm), pitch=1.27mm, pad=1.5×0.6mm
    // Pins 1-4 left side (bottom→top), pins 5-8 right side (bottom→top)
    'SOIC-8': [
      { number: '1', x_nm: -2_700_000, y_nm: -1_905_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm: -2_700_000, y_nm:   -635_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '3', x_nm: -2_700_000, y_nm:    635_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '4', x_nm: -2_700_000, y_nm:  1_905_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '5', x_nm:  2_700_000, y_nm: -1_905_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '6', x_nm:  2_700_000, y_nm:   -635_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '7', x_nm:  2_700_000, y_nm:    635_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '8', x_nm:  2_700_000, y_nm:  1_905_000, width_nm: 1_500_000, height_nm: 600_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // DIP-8: 300mil (7.62mm) row spacing, 100mil (2.54mm) pitch, drill=0.8mm, pad=1.6mm
    // Pins 1-4 left side (top→bottom): y = +150, +50, -50, -150 mil
    // Pins 5-8 right side (bottom→top): y = -150, -50, +50, +150 mil
    // layer_mask: 3 = both top and bottom (through-hole)
    'DIP-8': [
      { number: '1', x_nm: -3_810_000, y_nm:  3_810_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '2', x_nm: -3_810_000, y_nm:  1_270_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '3', x_nm: -3_810_000, y_nm: -1_270_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '4', x_nm: -3_810_000, y_nm: -3_810_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '5', x_nm:  3_810_000, y_nm: -3_810_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '6', x_nm:  3_810_000, y_nm: -1_270_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '7', x_nm:  3_810_000, y_nm:  1_270_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
      { number: '8', x_nm:  3_810_000, y_nm:  3_810_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'oblong', layer_mask: 3, drill_nm: 800_000 },
    ],
  };

  return padTemplates[footprint] || [];
}

/** Get silkscreen shapes for a footprint from the dynamic registry. */
function getFootprintSilk(footprint: string): SilkShape[] {
  return dynamicFootprintRegistry.get(footprint)?.silk ?? [];
}

/**
 * Parse .cypcb source code into a BoardSnapshot.
 * This is the JavaScript parser used when tree-sitter is not available (WASM mode).
 */
/**
 * Normalize logical pin names to physical pad numbers.
 * Mirrors the Rust normalize_pin_name() in sync.rs.
 */
function normalizePinName(name: string): string {
  switch (name.toLowerCase()) {
    case 'a': case 'anode': return '1';
    case 'k': case 'ka': case 'cathode': return '2';
    case '+': case 'pos': case 'positive': case 'p': return '1';
    case '-': case 'neg': case 'negative': case 'n': return '2';
    case 'b': case 'base': return '1';
    case 'c': case 'collector': return '2';
    case 'e': case 'emitter': return '3';
    default: return name;
  }
}

/**
 * IPC-2221 minimum trace width calculation for external copper layers.
 * Formula: W = (I / (k * dT^b))^(1/c) where k=0.048, b=0.44, c=0.725
 * Returns width in nanometers.
 *
 * @param current_ma Current in milliamps
 * @param tempRise Temperature rise in °C (default 10°C)
 * @param copperOz Copper weight in oz/ft² (default 1oz)
 */
/**
 * Minimum trace width in nanometers for a current, per IPC-2221.
 *
 * External layer, 10C rise, 1oz copper. The engine computes the same thing in
 * `PcbEngine::min_trace_width_for_current_ma`, via `cypcb-calc`, which is the
 * one implementation in the Rust workspace. This copy exists only until the
 * viewer stops owning the board model; until then it is the one JavaScript
 * copy, not the third.
 *
 * Exported because `interaction.ts` picks a routing width from the same rule
 * and used to inline the arithmetic twice.
 */
export function ipc2221MinWidthNm(current_ma: number, tempRise = 10, copperOz = 1): number {
  const currentA = current_ma / 1000;
  if (currentA <= 0) return 0;
  // Cross-sectional area in mils^2 (IPC-2221 external layer)
  const k = 0.048;
  const b = 0.44;
  const c = 0.725;
  const areaMils2 = Math.pow(currentA / (k * Math.pow(tempRise, b)), 1 / c);
  // 1oz copper is 1.378 mils thick. The language server had 1.37 here, which
  // quoted the user a width 0.58% off what the router would draw.
  const thicknessMils = copperOz * 1.378;
  const widthMils = areaMils2 / thicknessMils;
  return Math.round(widthMils * 25_400);
}

/**
 * Check traces against net current constraints using IPC-2221.
 * Returns violations for traces that are too narrow for the specified current.
 */
function checkTraceCurrentViolations(snapshot: BoardSnapshot): ViolationInfo[] {
  const violations: ViolationInfo[] = [];
  if (!snapshot.nets || !snapshot.traces) return violations;

  // Build net name → current constraint map
  const netCurrentMa = new Map<string, number>();
  const netWidthConstraint = new Map<string, number>();
  for (const net of snapshot.nets) {
    if (net.current_ma) netCurrentMa.set(net.name, net.current_ma);
    if (net.width_nm) netWidthConstraint.set(net.name, net.width_nm);
  }

  for (const trace of snapshot.traces) {
    const currentMa = netCurrentMa.get(trace.net_name);
    if (!currentMa) continue;

    const minWidthNm = ipc2221MinWidthNm(currentMa);
    const traceWidthNm = Math.round(trace.width);

    if (traceWidthNm < minWidthNm) {
      // Find trace midpoint for violation marker
      const midSeg = trace.segments[Math.floor(trace.segments.length / 2)];
      const mx = (midSeg.start_x + midSeg.end_x) / 2;
      const my = (midSeg.start_y + midSeg.end_y) / 2;

      const traceWidthMm = (traceWidthNm / 1e6).toFixed(2);
      const minWidthMm = (minWidthNm / 1e6).toFixed(2);
      const currentStr = currentMa >= 1000 ? `${(currentMa / 1000).toFixed(1)}A` : `${currentMa}mA`;

      violations.push({
        kind: 'trace-width-current',
        x_nm: Math.round(mx),
        y_nm: Math.round(my),
        message: `Trace ${trace.net_name}: width ${traceWidthMm}mm too thin for ${currentStr} — IPC-2221 recommends ≥${minWidthMm}mm`,
      });
    }
  }

  return violations;
}

function parseSource(source: string): { snapshot: BoardSnapshot; errors: string[] } {
  const errors: string[] = [];
  const lines = source.split('\n');

  let board: BoardInfo | null = null;
  let currentBoard: BoardInfo | null = null;
  const components: ComponentInfo[] = [];
  const nets: Map<string, NetInfo> = new Map();
  const traces: TraceInfo[] = [];
  const vias: ViaInfo[] = [];
  let currentComponent: Partial<ComponentInfo> | null = null;
  let currentNet: { name: string; pins: string[]; constraints: { width_nm?: number; clearance_nm?: number; current_ma?: number } } | null = null;
  let currentTrace: { netName: string; layer: string; width: number; locked: boolean; segments: TraceSegmentInfo[] } | null = null;
  let currentFootprint: { name: string; pads: PadInfo[] } | null = null;
  const customFootprints = new Map<string, PadInfo[]>();
  let braceDepth = 0;
  let inBoard = false;
  let inComponent = false;
  let inNet = false;
  let inTrace = false;
  let inFootprint = false;
  let inZone = false; // skip zone/keepout blocks gracefully
  let traceIdCounter = 200_000;

  for (let lineNum = 0; lineNum < lines.length; lineNum++) {
    const line = lines[lineNum].trim();
    if (!line || line.startsWith('//')) continue;

    // Count braces
    const openBraces = (line.match(/{/g) || []).length;
    const closeBraces = (line.match(/}/g) || []).length;

    // Parse version (ignore)
    if (line.startsWith('version ')) {
      continue;
    }

    // Parse board definition
    const boardMatch = line.match(/^board\s+(\w+)\s*{?$/);
    if (boardMatch) {
      currentBoard = {
        name: boardMatch[1],
        width_nm: 0,
        height_nm: 0,
        layer_count: 2,
      };
      inBoard = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse component definition
    const compMatch = line.match(/^component\s+(\w+)\s+(\w+)\s+"([^"]+)"\s*{?$/);
    if (compMatch) {
      const pads = getFootprintPads(compMatch[3]);

      // Compute body dimensions from pad bounding box
      let bodyWidthNm = 0;
      let bodyHeightNm = 0;
      if (pads.length > 0) {
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (const pad of pads) {
          const hw = pad.width_nm / 2;
          const hh = pad.height_nm / 2;
          minX = Math.min(minX, pad.x_nm - hw);
          minY = Math.min(minY, pad.y_nm - hh);
          maxX = Math.max(maxX, pad.x_nm + hw);
          maxY = Math.max(maxY, pad.y_nm + hh);
        }
        bodyWidthNm = maxX - minX;
        bodyHeightNm = maxY - minY;
      }

      currentComponent = {
        refdes: compMatch[1],
        value: '',
        x_nm: 0,
        y_nm: 0,
        rotation_mdeg: 0,
        footprint: compMatch[3],
        pads,
        body_width_nm: bodyWidthNm,
        body_height_nm: bodyHeightNm,
        model_3d: get3DModelUuid(compMatch[3]),
        silk: getFootprintSilk(compMatch[3]),
      };
      inComponent = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse net definition (with optional constraints in square brackets)
    const netMatch = line.match(/^net\s+(\w+)\s*(\[.*?\])?\s*{?$/);
    if (netMatch) {
      const netConstraints: { width_nm?: number; clearance_nm?: number; current_ma?: number } = {};
      if (netMatch[2]) {
        const block = netMatch[2];
        const wm = block.match(/width\s+(\d+(?:\.\d+)?)(mm|mil|nm)/);
        if (wm) netConstraints.width_nm = parseUnit(parseFloat(wm[1]), wm[2]);
        const cm = block.match(/clearance\s+(\d+(?:\.\d+)?)(mm|mil|nm)/);
        if (cm) netConstraints.clearance_nm = parseUnit(parseFloat(cm[1]), cm[2]);
        const curMa = block.match(/current\s+(\d+(?:\.\d+)?)(mA|A)/);
        if (curMa) {
          netConstraints.current_ma = curMa[2] === 'A'
            ? parseFloat(curMa[1]) * 1000
            : parseFloat(curMa[1]);
        }
      }
      currentNet = { name: netMatch[1], pins: [], constraints: netConstraints };
      inNet = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse trace definition: trace NET_NAME { ... }
    const traceMatch = line.match(/^trace\s+(\w+)\s*{?$/);
    if (traceMatch) {
      currentTrace = { netName: traceMatch[1], layer: 'Top', width: 200_000, locked: false, segments: [] };
      inTrace = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse footprint definition: footprint NAME { ... }
    const fpMatch = line.match(/^footprint\s+(\w+)\s*{?$/);
    if (fpMatch) {
      currentFootprint = { name: fpMatch[1], pads: [] };
      inFootprint = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse zone/keepout (skip gracefully — renderer doesn't support yet)
    const zoneMatch = line.match(/^(?:zone|keepout)\s+(\w+)\s*{?$/);
    if (zoneMatch) {
      inZone = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse board properties
    if (inBoard && currentBoard) {
      const sizeMatch = line.match(/^size\s+(\d+(?:\.\d+)?)(mm|mil|inch)\s+x\s+(\d+(?:\.\d+)?)(mm|mil|inch)$/);
      if (sizeMatch) {
        currentBoard.width_nm = parseUnit(parseFloat(sizeMatch[1]), sizeMatch[2]);
        currentBoard.height_nm = parseUnit(parseFloat(sizeMatch[3]), sizeMatch[4]);
      }
      const layersMatch = line.match(/^layers\s+(\d+)$/);
      if (layersMatch) {
        currentBoard.layer_count = parseInt(layersMatch[1], 10);
      }
    }

    // Parse component properties
    if (inComponent && currentComponent) {
      const valueMatch = line.match(/^value\s+"([^"]*)"$/);
      if (valueMatch) {
        currentComponent.value = valueMatch[1];
      }
      const atMatch = line.match(/^at\s+(-?\d+(?:\.\d+)?)(mm|mil|inch),\s*(-?\d+(?:\.\d+)?)(mm|mil|inch)(?:\s+rotate\s+(\d+(?:\.\d+)?))?$/);
      if (atMatch) {
        currentComponent.x_nm = parseUnit(parseFloat(atMatch[1]), atMatch[2]);
        currentComponent.y_nm = parseUnit(parseFloat(atMatch[3]), atMatch[4]);
        if (atMatch[5]) {
          currentComponent.rotation_mdeg = Math.round(parseFloat(atMatch[5]) * 1000);
        }
      }
      // Parse standalone rotate property
      const rotateMatch = line.match(/^rotate\s+(\d+(?:\.\d+)?)(?:\s*(?:deg|degrees))?$/);
      if (rotateMatch) {
        currentComponent.rotation_mdeg = Math.round(parseFloat(rotateMatch[1]) * 1000);
      }
      // Parse lcsc attribute
      const lcscMatch = line.match(/^lcsc\s+"([^"]*)"$/);
      if (lcscMatch) {
        // Store LCSC ID in metadata (could be used for BOM export)
        // For now, just acknowledge it so it doesn't get flagged as unknown
      }
    }

    // Parse net pins (strip inline comments)
    if (inNet && currentNet) {
      const pinMatch = line.match(/^(\w+)\.(\w+)/);
      if (pinMatch) {
        currentNet.pins.push(`${pinMatch[1]}.${pinMatch[2]}`);
      }
    }

    // Parse footprint properties (pad definitions)
    if (inFootprint && currentFootprint) {
      // pad 1 rect at -2.54mm, 0mm size 1.5mm x 2mm [drill 1mm]
      const padMatch = line.match(/^pad\s+(\w+)\s+(rect|circle|roundrect|oblong)\s+at\s+(-?\d+(?:\.\d+)?)(mm|mil|nm),\s*(-?\d+(?:\.\d+)?)(mm|mil|nm)\s+size\s+(-?\d+(?:\.\d+)?)(mm|mil|nm)\s+x\s+(-?\d+(?:\.\d+)?)(mm|mil|nm)(?:\s+drill\s+(-?\d+(?:\.\d+)?)(mm|mil|nm))?$/);
      if (padMatch) {
        const drillNm = padMatch[11] ? parseUnit(parseFloat(padMatch[11]), padMatch[12]) : null;
        currentFootprint.pads.push({
          number: padMatch[1],
          shape: padMatch[2],
          x_nm: parseUnit(parseFloat(padMatch[3]), padMatch[4]),
          y_nm: parseUnit(parseFloat(padMatch[5]), padMatch[6]),
          width_nm: parseUnit(parseFloat(padMatch[7]), padMatch[8]),
          height_nm: parseUnit(parseFloat(padMatch[9]), padMatch[10]),
          layer_mask: drillNm ? 3 : 1, // THT pads on both layers, SMD on top only
          drill_nm: drillNm,
        });
      }
    }

    // Parse trace properties
    if (inTrace && currentTrace) {
      const layerMatch = line.match(/^layer\s+(Top|Bottom|Inner\d+)$/);
      if (layerMatch) {
        currentTrace.layer = layerMatch[1];
      }
      const widthMatch = line.match(/^width\s+(-?\d+(?:\.\d+)?)(mm|mil|nm)$/);
      if (widthMatch) {
        currentTrace.width = parseUnit(parseFloat(widthMatch[1]), widthMatch[2]);
      }
      if (line === 'locked') {
        currentTrace.locked = true;
      }
      // Parse path: path X1mm,Y1mm -> X2mm,Y2mm -> ...
      const pathMatch = line.match(/^path\s+(.+)$/);
      if (pathMatch) {
        const pointStrs = pathMatch[1].split('->').map(s => s.trim());
        const points: { x: number; y: number }[] = [];
        for (const ps of pointStrs) {
          const coordMatch = ps.match(/^(-?\d+(?:\.\d+)?)(mm|mil|nm),\s*(-?\d+(?:\.\d+)?)(mm|mil|nm)$/);
          if (coordMatch) {
            points.push({
              x: parseUnit(parseFloat(coordMatch[1]), coordMatch[2]),
              y: parseUnit(parseFloat(coordMatch[3]), coordMatch[4]),
            });
          }
        }
        // Convert consecutive points to segments
        for (let i = 0; i < points.length - 1; i++) {
          currentTrace.segments.push({
            start_x: points[i].x,
            start_y: points[i].y,
            end_x: points[i + 1].x,
            end_y: points[i + 1].y,
          });
        }
      }
      // Parse via: via X,Y drill D
      const viaMatch = line.match(/^via\s+(-?\d+(?:\.\d+)?)(mm|mil|nm),\s*(-?\d+(?:\.\d+)?)(mm|mil|nm)(?:\s+drill\s+(-?\d+(?:\.\d+)?)(mm|mil|nm))?$/);
      if (viaMatch) {
        vias.push({
          id: traceIdCounter++,
          x: parseUnit(parseFloat(viaMatch[1]), viaMatch[2]),
          y: parseUnit(parseFloat(viaMatch[3]), viaMatch[4]),
          drill: viaMatch[5] ? parseUnit(parseFloat(viaMatch[5]), viaMatch[6]) : 300_000,
          outer_diameter: viaMatch[5] ? parseUnit(parseFloat(viaMatch[5]), viaMatch[6]) * 2 : 600_000,
          net_name: currentTrace.netName,
        });
      }
    }

    // Handle closing braces
    if (closeBraces > 0) {
      braceDepth -= closeBraces;

      if (braceDepth <= 0) {
        if (inBoard && currentBoard) {
          board = currentBoard;
          currentBoard = null;
          inBoard = false;
        }
        if (inComponent && currentComponent) {
          components.push(currentComponent as ComponentInfo);
          currentComponent = null;
          inComponent = false;
        }
        if (inNet && currentNet) {
          const connections: PinRef[] = currentNet.pins.map(pin => {
            const [component, pinNum] = pin.split('.');
            return { component, pin: normalizePinName(pinNum) };
          });
          nets.set(currentNet.name, {
            name: currentNet.name,
            id: nets.size,
            connections,
            ...currentNet.constraints,
          });
          currentNet = null;
          inNet = false;
        }
        if (inTrace && currentTrace) {
          if (currentTrace.segments.length > 0) {
            traces.push({
              id: traceIdCounter++,
              segments: currentTrace.segments,
              width: currentTrace.width,
              layer: currentTrace.layer,
              net_name: currentTrace.netName,
              locked: currentTrace.locked,
            });
          }
          currentTrace = null;
          inTrace = false;
        }
        if (inFootprint && currentFootprint) {
          if (currentFootprint.pads.length > 0) {
            customFootprints.set(currentFootprint.name, currentFootprint.pads);
          }
          currentFootprint = null;
          inFootprint = false;
        }
        if (inZone) {
          inZone = false;
        }
        braceDepth = 0;
      }
    }

    braceDepth += openBraces;
  }

  // Post-process: fix up components using custom footprints
  for (const comp of components) {
    if (comp.pads.length === 0 && customFootprints.has(comp.footprint)) {
      comp.pads = customFootprints.get(comp.footprint)!;
      // Recompute body dimensions from pads
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const pad of comp.pads) {
        const hw = pad.width_nm / 2, hh = pad.height_nm / 2;
        minX = Math.min(minX, pad.x_nm - hw);
        minY = Math.min(minY, pad.y_nm - hh);
        maxX = Math.max(maxX, pad.x_nm + hw);
        maxY = Math.max(maxY, pad.y_nm + hh);
      }
      comp.body_width_nm = maxX - minX;
      comp.body_height_nm = maxY - minY;
    }
  }

  return {
    snapshot: { board, components, nets: Array.from(nets.values()), violations: [], traces, vias, ratsnest: [] },
    errors,
  };
}

/**
 * Parse FreeRouting .ses (session) file to extract routing results.
 * Returns traces and vias that can be added to a BoardSnapshot.
 */
export function parseSesFile(sesContent: string): { traces: BoardSnapshot['traces']; vias: BoardSnapshot['vias'] } {
  const traces: BoardSnapshot['traces'] = [];
  const vias: BoardSnapshot['vias'] = [];

  // Default resolution: mil 10 = 1/10 mil = 2540 nm
  let resolution = 2540; // nm per unit

  // Parse resolution from routes section
  const resMatch = sesContent.match(/\(routes[\s\S]*?\(resolution\s+(\w+)\s+(\d+)\)/);
  if (resMatch) {
    const unit = resMatch[1];
    const divisor = parseInt(resMatch[2], 10);
    if (unit === 'mil') {
      resolution = Math.round(25400 / divisor); // 1 mil = 25400 nm
    } else if (unit === 'mm') {
      resolution = Math.round(1_000_000 / divisor);
    }
  }
  console.log('[SES Parser] Resolution:', resolution, 'nm per unit');

  // Find network_out content
  const networkOutStart = sesContent.indexOf('(network_out');
  if (networkOutStart === -1) {
    console.log('[SES Parser] No network_out section found');
    return { traces, vias };
  }
  const networkSection = sesContent.slice(networkOutStart);

  // Find net blocks by counting parentheses (more reliable than regex)
  let pos = 0;
  while (true) {
    const netStart = networkSection.indexOf('(net ', pos);
    if (netStart === -1) break;

    // Find net name
    const nameMatch = networkSection.slice(netStart).match(/\(net\s+(\w+)/);
    if (!nameMatch) break;
    const netName = nameMatch[1];

    // Find where this net ends by counting parentheses
    let depth = 0;
    let netEnd = netStart;
    for (let i = netStart; i < networkSection.length; i++) {
      if (networkSection[i] === '(') depth++;
      if (networkSection[i] === ')') depth--;
      if (depth === 0) {
        netEnd = i + 1;
        break;
      }
    }

    const netContent = networkSection.slice(netStart, netEnd);
    pos = netEnd;

    // Find all wire paths in this net
    const wirePathRegex = /\(path\s+(\S+)\s+(\d+)\s+([\d\s\-]+)\)/g;
    let pathMatch;

    while ((pathMatch = wirePathRegex.exec(netContent)) !== null) {
      const layerStr = pathMatch[1];
      const width = parseInt(pathMatch[2], 10) * resolution;
      const coordsStr = pathMatch[3].trim();
      const coords = coordsStr.split(/\s+/).map(s => parseInt(s, 10));

      // Convert layer name (F.Cu -> Top, B.Cu -> Bottom)
      const layer = layerStr === 'B.Cu' ? 'Bottom' : 'Top';

      // Create segments from coordinate pairs
      const segments: { start_x: number; start_y: number; end_x: number; end_y: number }[] = [];
      for (let i = 0; i < coords.length - 2; i += 2) {
        segments.push({
          start_x: coords[i] * resolution,
          start_y: coords[i + 1] * resolution,
          end_x: coords[i + 2] * resolution,
          end_y: coords[i + 3] * resolution,
        });
      }

      if (segments.length > 0) {
        traces.push({
          id: traces.length,
          segments,
          width,
          layer,
          net_name: netName,
          locked: false,
        });
      }
    }

    // Find vias in this net
    const viaRegex = /\(via\s+\w+\s+(\d+)\s+(\d+)\)/g;
    let viaMatch;
    while ((viaMatch = viaRegex.exec(netContent)) !== null) {
      vias.push({
        id: vias.length,
        x: parseInt(viaMatch[1], 10) * resolution,
        y: parseInt(viaMatch[2], 10) * resolution,
        drill: 300_000,
        outer_diameter: 600_000,
        net_name: netName,
      });
    }
  }

  console.log('[SES Parser] Parsed', traces.length, 'traces,', vias.length, 'vias');
  return { traces, vias };
}

// ============================================================================
// Shared route-loading helpers
// ============================================================================

/**
 * Regenerate star-topology ratsnest for unrouted nets.
 * Called after load_source and after applying routes.
 */
function regenerateRatsnest(snapshot: BoardSnapshot): void {
  snapshot.ratsnest = [];
  for (const net of snapshot.nets) {
    if (net.connections.length < 2) continue;

    const hasTraces = snapshot.traces.some(t => t.net_name === net.name);
    if (hasTraces) continue;

    const positions: { x: number; y: number }[] = [];
    for (const conn of net.connections) {
      const comp = snapshot.components.find(c => c.refdes === conn.component);
      if (comp) {
        const rad = ((comp.rotation_mdeg || 0) / 1000) * (Math.PI / 180);
        const cos = Math.cos(rad);
        const sin = Math.sin(rad);
        const pad = comp.pads.find(p => p.number === conn.pin);
        const px = pad?.x_nm ?? 0;
        const py = pad?.y_nm ?? 0;
        positions.push({
          x: comp.x_nm + (px * cos - py * sin),
          y: comp.y_nm + (px * sin + py * cos),
        });
      }
    }

    if (positions.length >= 2) {
      for (let i = 1; i < positions.length; i++) {
        snapshot.ratsnest.push({
          start_x: positions[0].x,
          start_y: positions[0].y,
          end_x: positions[i].x,
          end_y: positions[i].y,
          net_name: net.name,
        });
      }
    }
  }
}

/**
 * Apply parsed routes to a snapshot: replace traces/vias and regenerate ratsnest.
 */
function applyRoutesToSnapshot(
  snapshot: BoardSnapshot,
  sesContent: string,
): void {
  const { traces, vias } = parseSesFile(sesContent);
  snapshot.traces = traces;
  snapshot.vias = vias;
  regenerateRatsnest(snapshot);
}

// ============================================================================
// Geometry utilities (used by MockPcbEngine hit-testing and DRC)
// ============================================================================

/**
 * Approximate minimum distance between two line segments.
 * Tests endpoints of each segment against the other segment.
 */
function segmentToSegmentDistance(
  s1: TraceSegmentInfo,
  s2: TraceSegmentInfo,
): number {
  // Sample several points: endpoints + midpoints, take minimum
  const d1 = pointToSegmentDistance(s1.start_x, s1.start_y, s2.start_x, s2.start_y, s2.end_x, s2.end_y);
  const d2 = pointToSegmentDistance(s1.end_x, s1.end_y, s2.start_x, s2.start_y, s2.end_x, s2.end_y);
  const d3 = pointToSegmentDistance(s2.start_x, s2.start_y, s1.start_x, s1.start_y, s1.end_x, s1.end_y);
  const d4 = pointToSegmentDistance(s2.end_x, s2.end_y, s1.start_x, s1.start_y, s1.end_x, s1.end_y);
  return Math.min(d1, d2, d3, d4);
}

// ============================================================================
// WASM Engine Adapter
// ============================================================================

/**
 * Adapter that wraps the raw WASM PcbEngine and provides the load_source() method.
 *
 * The WASM engine doesn't include tree-sitter, so parsing is done in JavaScript.
 * This adapter parses the source, then calls load_snapshot() on the WASM engine.
 * Query operations use the WASM engine's spatial index for efficiency.
 */
/**
 * Build TraceSegmentInfo[] from a flat coordinate array and normalize layer name.
 * Shared between WasmPcbEngineAdapter and MockPcbEngine to eliminate duplication.
 */
function buildTraceSegments(
  segments: number[],
  layer: string,
): { traceSegments: TraceSegmentInfo[]; normalizedLayer: string } {
  const traceSegments: TraceSegmentInfo[] = [];
  for (let i = 0; i < segments.length; i += 4) {
    traceSegments.push({
      start_x: segments[i], start_y: segments[i + 1],
      end_x: segments[i + 2], end_y: segments[i + 3],
    });
  }
  const normalizedLayer = layer === 'TopCopper' ? 'Top' : layer === 'BottomCopper' ? 'Bottom' : layer;
  return { traceSegments, normalizedLayer };
}

class WasmPcbEngineAdapter implements PcbEngine {
  private wasmEngine: WasmPcbEngine;
  private cachedSnapshot: BoardSnapshot | null = null;
  /** Preserved net constraint data — survives cache invalidation */
  private cachedNetConstraints: Map<string, { width_nm?: number; clearance_nm?: number; current_ma?: number }> = new Map();
  /** Auto-increment entity ID for JS-fallback trace/via mutations */
  private nextEntityId = 100_000;

  constructor(wasmEngine: WasmPcbEngine) {
    this.wasmEngine = wasmEngine;
  }

  register_footprint(name: string, pads: PadInfo[], silk: SilkShape[]): string {
    return this.wasmEngine.register_footprint(name, JSON.stringify(pads), JSON.stringify(silk));
  }

  load_source(source: string): string {
    // Parse in JavaScript
    const { snapshot, errors } = parseSource(source);

    // Generate ratsnest for unrouted nets
    regenerateRatsnest(snapshot);

    // Cache snapshot + preserve net constraints separately (survives cache invalidation)
    this.cachedSnapshot = snapshot;
    this.cachedNetConstraints.clear();
    for (const net of snapshot.nets) {
      if (net.width_nm || net.clearance_nm || net.current_ma) {
        this.cachedNetConstraints.set(net.name, {
          width_nm: net.width_nm,
          clearance_nm: net.clearance_nm,
          current_ma: net.current_ma,
        });
      }
    }

    // Store snapshot and load into WASM engine for queries
    const wasmError = this.wasmEngine.load_snapshot(snapshot);
    if (wasmError) {
      errors.push(wasmError);
    }

    // Replay parsed traces into WASM engine so export_traces_as_dsl() can find them
    // Update snapshot trace IDs to match WASM-assigned IDs
    if (snapshot.traces.length > 0 && typeof this.wasmEngine.add_trace_json === 'function') {
      for (const trace of snapshot.traces) {
        const flatSegs: number[] = [];
        for (const seg of trace.segments) {
          flatSegs.push(
            Math.round(seg.start_x), Math.round(seg.start_y),
            Math.round(seg.end_x), Math.round(seg.end_y),
          );
        }
        const wasmId = this.wasmEngine.add_trace_json(
          trace.net_name,
          trace.layer,
          BigInt(Math.round(trace.width)),
          JSON.stringify(flatSegs),
        );
        // Update the trace ID in snapshot to match WASM entity ID
        if (wasmId !== 0xFFFFFFFF) {
          trace.id = wasmId;
        }
      }
      console.log(`[TracePersist] Replayed ${snapshot.traces.length} traces into WASM engine`);
    }

    return errors.join('\n');
  }

  load_routes(sesContent: string): void {
    if (!this.cachedSnapshot) return;
    applyRoutesToSnapshot(this.cachedSnapshot, sesContent);
  }

  get_snapshot(): BoardSnapshot {
    // Return cached snapshot with traces/ratsnest that we added in JS
    // The WASM engine's get_snapshot() would have empty traces since
    // we only populated components/board, not Trace entities
    if (this.cachedSnapshot) {
      // Get DRC violations from WASM (computed in Rust)
      const wasmSnapshot = sanitizeSnapshot(this.wasmEngine.get_snapshot());
      const wasmViolations = wasmSnapshot.violations || [];
      // Add JS-side violations (silk data + trace current check only exist in JS)
      const silkViolations = checkSilkClearance(this.cachedSnapshot, Number(this.get_min_clearance_nm()));
      const currentViolations = checkTraceCurrentViolations(this.cachedSnapshot);
      return {
        ...this.cachedSnapshot,
        violations: [...wasmViolations, ...silkViolations, ...currentViolations],
      };
    }
    // Cache was invalidated — get fresh snapshot from WASM and restore net constraints
    const wasmSnap = sanitizeSnapshot(this.wasmEngine.get_snapshot());
    if (this.cachedNetConstraints.size > 0 && wasmSnap.nets) {
      for (const net of wasmSnap.nets) {
        const c = this.cachedNetConstraints.get(net.name);
        if (c) {
          net.width_nm = c.width_nm;
          net.clearance_nm = c.clearance_nm;
          net.current_ma = c.current_ma;
        }
      }
    }
    // Also run JS-side DRC (current violations)
    const silkV = checkSilkClearance(wasmSnap, Number(this.get_min_clearance_nm()));
    const currentV = checkTraceCurrentViolations(wasmSnap);
    if (silkV.length > 0 || currentV.length > 0) {
      wasmSnap.violations = [...(wasmSnap.violations || []), ...silkV, ...currentV];
    }
    return wasmSnap;
  }

  query_point(x_nm: number, y_nm: number): string[] {
    // Use WASM spatial index for efficient queries
    // The WASM engine rebuilds the spatial index in populate_from_snapshot()
    return this.wasmEngine.query_point(BigInt(x_nm), BigInt(y_nm));
  }

  add_trace(net_name: string, layer: string, width_nm: number, segments: number[]): number {
    // Try WASM method first; fall back to JS-side snapshot mutation
    // when the WASM module doesn't expose add_trace_json
    if (typeof this.wasmEngine.add_trace_json === 'function') {
      const id = this.wasmEngine.add_trace_json(net_name, layer, BigInt(width_nm), JSON.stringify(segments));
      this.cachedSnapshot = null;
      return id;
    }

    // JS fallback: mutate cached snapshot directly (same logic as MockPcbEngine)
    if (!this.cachedSnapshot) return 0xFFFFFFFF;
    if (segments.length < 4 || segments.length % 4 !== 0) return 0xFFFFFFFF;

    const id = this.nextEntityId++;
    const { traceSegments, normalizedLayer } = buildTraceSegments(segments, layer);
    this.cachedSnapshot.traces.push({
      id, segments: traceSegments, width: width_nm,
      layer: normalizedLayer, net_name, locked: false,
    });
    return id;
  }

  remove_trace(trace_id: number): boolean {
    let removed = false;

    // Try WASM method
    if (typeof this.wasmEngine.remove_trace === 'function') {
      removed = this.wasmEngine.remove_trace(trace_id);
      if (removed) {
        this.cachedSnapshot = null;
      }
    }

    // Also try JS-side snapshot (trace may have a JS-assigned ID not in WASM)
    if (!removed && this.cachedSnapshot) {
      const idx = this.cachedSnapshot.traces.findIndex(t => t.id === trace_id);
      if (idx !== -1) {
        this.cachedSnapshot.traces.splice(idx, 1);
        removed = true;
      }
    }

    return removed;
  }

  get_trace_at_point(x_nm: number, y_nm: number, tolerance_nm: number): number {
    return this.wasmEngine.get_trace_at_point(BigInt(x_nm), BigInt(y_nm), BigInt(tolerance_nm));
  }

  run_drc_incremental(): number {
    if (typeof this.wasmEngine.run_drc_incremental === 'function') {
      return this.wasmEngine.run_drc_incremental();
    }
    return 0; // No-op when WASM doesn't support DRC
  }

  trace_count(): number {
    if (typeof this.wasmEngine.trace_count === 'function') {
      return this.wasmEngine.trace_count();
    }
    return this.cachedSnapshot?.traces?.length ?? 0;
  }

  export_traces_as_dsl(): string {
    if (typeof this.wasmEngine.export_traces_as_dsl === 'function') {
      const result = this.wasmEngine.export_traces_as_dsl();
      console.log(`[WASM] export_traces_as_dsl: ${result.length} chars`);
      return result;
    }
    console.warn('[WASM] export_traces_as_dsl NOT available in WASM module — using fallback');
    return '';
  }

  get_min_clearance_nm(): number {
    if (typeof this.wasmEngine.get_min_clearance_nm === 'function') {
      return this.wasmEngine.get_min_clearance_nm();
    }
    return 150_000; // Default 0.15mm fallback
  }

  rotate_component(refdes: string, delta_mdeg: number): boolean {
    const result = this.wasmEngine.rotate_component(refdes, delta_mdeg);
    if (result) {
      this.cachedSnapshot = null;
    }
    return result;
  }

  set_board_size(width_nm: number, height_nm: number): boolean {
    const result = this.wasmEngine.set_board_size(BigInt(width_nm), BigInt(height_nm));
    if (result) {
      this.cachedSnapshot = null;
    }
    return result;
  }

  auto_route(): string {
    const result = this.wasmEngine.auto_route();
    this.cachedSnapshot = null; // Invalidate cache — routes changed
    return result;
  }

  auto_route_with_params(params: string): string {
    const result = this.wasmEngine.auto_route_with_params(params);
    this.cachedSnapshot = null; // Invalidate cache — routes changed
    return result;
  }

  auto_route_variants(): string {
    const result = this.wasmEngine.auto_route_variants();
    this.cachedSnapshot = null; // Invalidate cache — routes changed
    return result;
  }

  auto_route_debug(params: string): string {
    return this.wasmEngine.auto_route_debug(params);
  }

  free(): void {
    this.wasmEngine.free();
  }
}

// ============================================================================
// Mock Engine (fallback when WASM is unavailable)
// ============================================================================

/**
 * Mock PCB engine for development/testing without WASM.
 * Uses the same JavaScript parser as the WASM adapter.
 */
class MockPcbEngine implements PcbEngine {
  private snapshot: BoardSnapshot = { board: null, components: [], nets: [], violations: [], traces: [], vias: [], ratsnest: [] };
  /** Footprints handed to the engine that did not come from source. */
  private registered = new Map<string, { pads: PadInfo[]; silk: SilkShape[] }>();
  /** Next mock entity ID counter */
  private nextEntityId = 1000;

  register_footprint(name: string, pads: PadInfo[], silk: SilkShape[]): string {
    this.registered.set(name, { pads, silk });
    return '';
  }

  load_source(source: string): string {
    const { snapshot, errors } = parseSource(source);
    regenerateRatsnest(snapshot);
    this.snapshot = snapshot;
    return errors.join('\n');
  }

  load_routes(sesContent: string): void {
    applyRoutesToSnapshot(this.snapshot, sesContent);
  }

  get_snapshot(): BoardSnapshot {
    return this.snapshot;
  }

  query_point(x_nm: number, y_nm: number): string[] {
    const result: string[] = [];

    for (const comp of this.snapshot.components) {
      // Check if point is within component bounds (simplified)
      const compWidth = 2_000_000; // 2mm default
      const compHeight = 1_000_000; // 1mm default

      if (x_nm >= comp.x_nm - compWidth / 2 &&
          x_nm <= comp.x_nm + compWidth / 2 &&
          y_nm >= comp.y_nm - compHeight / 2 &&
          y_nm <= comp.y_nm + compHeight / 2) {
        result.push(comp.refdes);
      }
    }

    return result;
  }

  // -- Trace mutation API (MockPcbEngine) --

  add_trace(net_name: string, layer: string, width_nm: number, segments: number[]): number {
    // Validate inputs
    if (segments.length < 4 || segments.length % 4 !== 0) {
      return 0xFFFFFFFF;
    }
    const validLayers = ['Top', 'TopCopper', 'Bottom', 'BottomCopper'];
    const isInner = /^Inner\d+$/.test(layer);
    if (!validLayers.includes(layer) && !isInner) {
      return 0xFFFFFFFF;
    }

    const id = this.nextEntityId++;
    const { traceSegments, normalizedLayer } = buildTraceSegments(segments, layer);

    this.snapshot.traces.push({
      id,
      segments: traceSegments,
      width: width_nm,
      layer: normalizedLayer,
      net_name,
      locked: false,
    });

    console.log(`[MockEngine] add_trace: net=${net_name} layer=${normalizedLayer} id=${id} segs=${traceSegments.length}`);
    return id;
  }

  remove_trace(trace_id: number): boolean {
    const idx = this.snapshot.traces.findIndex(t => t.id === trace_id);
    if (idx === -1) return false;
    this.snapshot.traces.splice(idx, 1);
    console.log(`[MockEngine] remove_trace: id=${trace_id}`);
    return true;
  }

  get_trace_at_point(x_nm: number, y_nm: number, tolerance_nm: number): number {
    let bestId = 0xFFFFFFFF;
    let bestDist = Infinity;

    for (const trace of this.snapshot.traces) {
      const halfWidth = trace.width / 2;
      for (const seg of trace.segments) {
        const dist = pointToSegmentDistance(
          x_nm, y_nm,
          seg.start_x, seg.start_y,
          seg.end_x, seg.end_y,
        );
        if (dist <= halfWidth + tolerance_nm && dist < bestDist) {
          bestDist = dist;
          bestId = trace.id;
        }
      }
    }

    return bestId;
  }

  run_drc_incremental(): number {
    // Mock: simple clearance check between traces
    // Real DRC is in Rust — this just provides a realistic interface
    const violations: ViolationInfo[] = [];
    const MIN_CLEARANCE = 150_000; // 0.15mm in nm

    for (let i = 0; i < this.snapshot.traces.length; i++) {
      for (let j = i + 1; j < this.snapshot.traces.length; j++) {
        const t1 = this.snapshot.traces[i];
        const t2 = this.snapshot.traces[j];
        if (t1.net_name === t2.net_name) continue; // Same net is fine
        if (t1.layer !== t2.layer) continue; // Different layers don't interact

        for (const s1 of t1.segments) {
          for (const s2 of t2.segments) {
            const dist = segmentToSegmentDistance(s1, s2);
            const required = t1.width / 2 + t2.width / 2 + MIN_CLEARANCE;
            if (dist < required) {
              const mx = (s1.start_x + s1.end_x + s2.start_x + s2.end_x) / 4;
              const my = (s1.start_y + s1.end_y + s2.start_y + s2.end_y) / 4;
              violations.push({
                kind: 'clearance',
                x_nm: mx,
                y_nm: my,
                message: `Clearance violation between ${t1.net_name} and ${t2.net_name}: ${(dist / 1_000_000).toFixed(2)}mm < ${(required / 1_000_000).toFixed(2)}mm required`,
              });
            }
          }
        }
      }
    }

    // Add IPC-2221 current violations
    const currentViolations = checkTraceCurrentViolations(this.snapshot);
    violations.push(...currentViolations);

    this.snapshot.violations = violations;
    console.log(`[MockEngine] run_drc_incremental: ${violations.length} violations`);
    return violations.length;
  }

  trace_count(): number {
    return this.snapshot.traces.length;
  }

  export_traces_as_dsl(): string {
    return ''; // Mock engine: no export
  }

  get_min_clearance_nm(): number {
    return 150_000; // Mock: 0.15mm default
  }

  rotate_component(refdes: string, delta_mdeg: number): boolean {
    const comp = this.snapshot.components.find(c => c.refdes === refdes);
    if (!comp) {
      console.warn(`[MockEngine] rotate_component: ${refdes} not found`);
      return false;
    }
    // Normalize to [0, 360000)
    comp.rotation_mdeg = ((comp.rotation_mdeg + delta_mdeg) % 360_000 + 360_000) % 360_000;
    console.log(`[MockEngine] rotate_component: ${refdes} → ${comp.rotation_mdeg / 1000}°`);
    return true;
  }

  set_board_size(width_nm: number, height_nm: number): boolean {
    if (!this.snapshot.board) {
      console.warn('[MockEngine] set_board_size: no board');
      return false;
    }
    this.snapshot.board.width_nm = width_nm;
    this.snapshot.board.height_nm = height_nm;
    console.log(`[MockEngine] set_board_size: ${width_nm / 1e6}mm x ${height_nm / 1e6}mm`);
    return true;
  }

  auto_route(): string {
    console.warn('[MockEngine] auto_route not available in mock mode');
    return '{"ok":false,"error":"Autorouter not available in mock mode"}';
  }

  auto_route_with_params(_params: string): string {
    console.warn('[MockEngine] auto_route_with_params not available in mock mode');
    return '{"ok":false,"error":"Autorouter not available in mock mode"}';
  }

  auto_route_variants(): string {
    console.warn('[MockEngine] auto_route_variants not available in mock mode');
    return '{"ok":false,"error":"Variant generation not available in mock mode"}';
  }

  auto_route_debug(_params: string): string {
    return '{"ok":false,"error":"Debug routing not available in mock mode"}';
  }
}

// ============================================================================
// Module loading
// ============================================================================

/**
 * Load the WASM module and return the PCB engine instance.
 * Falls back to mock implementation if WASM is not available.
 *
 * @returns The PCB engine instance
 */
export async function loadWasm(): Promise<PcbEngine> {
  if (engineInstance) {
    return engineInstance;
  }

  // Try to load the real WASM module first
  try {
    // Direct import - Vite will handle bundling with vite-plugin-wasm
    const wasm = await import('../pkg/cypcb_render.js');
    await wasm.default();
    wasmModule = wasm;

    // Wrap the WASM engine with our adapter that provides load_source()
    const rawEngine = new wasm.PcbEngine() as unknown as WasmPcbEngine;
    engineInstance = new WasmPcbEngineAdapter(rawEngine);
    replayRegisteredFootprints(engineInstance);
    console.log('WASM module loaded successfully');
    return engineInstance;
  } catch (e) {
    console.log('WASM not available, using mock:', e);
  }

  // Fallback to MockPcbEngine when:
  // - Development without WASM build
  // - Environments where WASM fails to load
  // - Testing without the Rust backend
  console.log('Using MockPcbEngine (WASM fallback)');
  engineInstance = new MockPcbEngine();
  replayRegisteredFootprints(engineInstance);
  return engineInstance;
}

/**
 * Get the current engine instance (if loaded)
 */
export function getEngine(): PcbEngine | null {
  return engineInstance;
}

/**
 * Helper to load source and get snapshot in one call
 */
export function loadAndSnapshot(source: string): { snapshot: BoardSnapshot; errors: string } | null {
  if (!engineInstance) return null;

  const errors = engineInstance.load_source(source);
  const snapshot = engineInstance.get_snapshot();

  return { snapshot, errors };
}

/**
 * Check if the engine is using the real WASM implementation
 */
export function isWasmLoaded(): boolean {
  return wasmModule !== null;
}
