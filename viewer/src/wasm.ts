/**
 * WASM module loading utilities
 *
 * This module provides the interface for loading the PcbEngine from WASM.
 * If the WASM module is not available, it falls back to a mock implementation
 * that provides the same interface for development and testing.
 *
 * Architecture note: The WASM build doesn't include tree-sitter (too complex for WASM),
 * so parsing is done in JavaScript. The WASM engine provides:
 * - load_source(): Read `.cypcb` in the engine and build the board from it
 * - load_snapshot(): Load a board somebody else parsed
 * - get_snapshot(): Get the current board state
 * - query_point(): Query components at a point
 *
 * This module provides an adapter (WasmPcbEngineAdapter) that hands source to
 * the engine, which carries the Rust reader. It used to parse `.cypcb` here in
 * TypeScript - a second reader of the same language, deleted on 2026-08-07
 * once the engine could do it. See `docs/one-parser.md`.
 */

import type { BoardSnapshot, PadInfo, TraceSegmentInfo, ViolationInfo, SilkShape } from './types';
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
  /**
   * The last load's parse and sync messages, each with the line it is about.
   *
   * `load_source` returns them as one blob of text, and the editor used to
   * recover a line by scanning that text for the word "line" followed by a
   * number - which no message writes, so every squiggle landed on line 1.
   */
  get_diagnostics_json(): string;
  /** Load and parse a .cypcb source file, returns error message if failed */
  load_source(source: string): string;

  /**
   * Read a `.kicad_pcb` and build the board from it.
   *
   * The command line learned to read, check, route and write KiCad boards
   * before the viewer could open one at all.
   */
  load_kicad(source: string): string;
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
  /** Read `.cypcb` and build the board from it. Exported since the engine
   *  carries the Rust reader; before that the host parsed and sent a snapshot. */
  load_source(source: string): string;
  /** Read a `.kicad_pcb` and build the board from it. */
  load_kicad(source: string): string;
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
 * Parse .cypcb source code into a BoardSnapshot.
 * This is the JavaScript parser used when tree-sitter is not available (WASM mode).
 */

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
 * This adapter hands the source to the engine and reads the board back.
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

export class WasmPcbEngineAdapter implements PcbEngine {
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

  get_diagnostics_json(): string {
    return (this.wasmEngine as unknown as { get_diagnostics_json(): string }).get_diagnostics_json();
  }

  load_source(source: string): string {
    // The engine parses. Until 2026-08-07 this line read `parseSource(source)`
    // - a second reader of the same language, written in TypeScript, which did
    // not instantiate modules or follow imports, so a design using either drew
    // differently on screen than it exported. The engine's reader is checked
    // against the tree-sitter one board by board in
    // `crates/cypcb-parser/tests/differential.rs`.
    const errors = this.wasmEngine.load_source(source);

    // The model now lives in Rust: components, nets, traces, vias, zones, the
    // copper the pours became, the ratsnest and the DRC violations all come
    // back from it. There is nothing left to replay into the engine, which is
    // what the trace loop here used to do.
    const snapshot = sanitizeSnapshot(this.wasmEngine.get_snapshot());
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

    return errors;
  }

  load_kicad(source: string): string {
    // Same shape as `load_source`: the engine reads, and everything the viewer
    // draws comes back in the snapshot.
    const errors = this.wasmEngine.load_kicad(source);
    const snapshot = sanitizeSnapshot(this.wasmEngine.get_snapshot());
    this.cachedSnapshot = snapshot;
    this.cachedNetConstraints.clear();
    return errors;
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
      // The engine is the checker. `silk-clearance` and `trace-current` used
      // to be re-implemented here and appended to what Rust found, so a board
      // that tripped either was told about it twice under two different names
      // - `trace-current` from the rule and `trace-width-current` from this
      // module - and the two copies had drifted: the Rust silk rule learned
      // about printed designators and about clipping the legend off copper,
      // and the TypeScript one knew about neither.
      const wasmSnapshot = sanitizeSnapshot(this.wasmEngine.get_snapshot());
      return {
        ...this.cachedSnapshot,
        violations: wasmSnapshot.violations || [],
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

  get_diagnostics_json(): string {
    // The fallback engine does not read the language, so it has nothing to
    // say about a line of it.
    return '[]';
  }

  load_source(_source: string): string {
    // There is one reader of this language now, and it lives in the engine.
    // This fallback used to carry a second one in TypeScript, which drew
    // module and import boards differently from what the CLI exported - the
    // whole reason for `docs/one-parser.md`. A clear refusal beats a board
    // that looks right and is not.
    return 'This build has no engine: the WASM module failed to load, and .cypcb is read by the engine.';
  }

  load_kicad(_source: string): string {
    // Same refusal, same reason: the KiCad reader is in the engine too.
    return 'This build has no engine: the WASM module failed to load, and .kicad_pcb is read by the engine.';
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

    // The fallback engine checks clearance and nothing else. It exists for a
    // browser where the WASM module failed to load, and a second copy of a
    // rule is what this change removed - a fallback that answers differently
    // from the engine is worse than one that answers less.
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
