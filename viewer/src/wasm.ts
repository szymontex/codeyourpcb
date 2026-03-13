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

import type { BoardSnapshot, ComponentInfo, PadInfo, NetInfo, PinRef, BoardInfo, TraceSegmentInfo, ViolationInfo } from './types';

/**
 * Interface for the PCB rendering engine exposed from Rust/WASM
 */
export interface PcbEngine {
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
  get_violations_json(): string;
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
 */
function getFootprintPads(footprint: string): PadInfo[] {
  // Pad templates must match the Rust footprint library (cypcb-world/src/footprint/).
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

  return padTemplates[footprint] || padTemplates['0402'];
}

/**
 * Parse .cypcb source code into a BoardSnapshot.
 * This is the JavaScript parser used when tree-sitter is not available (WASM mode).
 */
function parseSource(source: string): { snapshot: BoardSnapshot; errors: string[] } {
  const errors: string[] = [];
  const lines = source.split('\n');

  let board: BoardInfo | null = null;
  let currentBoard: BoardInfo | null = null;
  const components: ComponentInfo[] = [];
  const nets: Map<string, NetInfo> = new Map();
  let currentComponent: Partial<ComponentInfo> | null = null;
  let currentNet: { name: string; pins: string[] } | null = null;
  let braceDepth = 0;
  let inBoard = false;
  let inComponent = false;
  let inNet = false;

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
      currentComponent = {
        refdes: compMatch[1],
        value: '',
        x_nm: 0,
        y_nm: 0,
        rotation_mdeg: 0,
        footprint: compMatch[3],
        pads: getFootprintPads(compMatch[3]),
      };
      inComponent = true;
      braceDepth += openBraces;
      continue;
    }

    // Parse net definition
    const netMatch = line.match(/^net\s+(\w+)\s*{?$/);
    if (netMatch) {
      currentNet = { name: netMatch[1], pins: [] };
      inNet = true;
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
      const atMatch = line.match(/^at\s+(\d+(?:\.\d+)?)(mm|mil|inch),\s*(\d+(?:\.\d+)?)(mm|mil|inch)(?:\s+rotate\s+(\d+(?:\.\d+)?))?$/);
      if (atMatch) {
        currentComponent.x_nm = parseUnit(parseFloat(atMatch[1]), atMatch[2]);
        currentComponent.y_nm = parseUnit(parseFloat(atMatch[3]), atMatch[4]);
        if (atMatch[5]) {
          currentComponent.rotation_mdeg = Math.round(parseFloat(atMatch[5]) * 1000);
        }
      }
    }

    // Parse net pins
    if (inNet && currentNet) {
      const pinMatch = line.match(/^(\w+)\.(\w+)$/);
      if (pinMatch) {
        currentNet.pins.push(`${pinMatch[1]}.${pinMatch[2]}`);
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
            return { component, pin: pinNum };
          });
          nets.set(currentNet.name, {
            name: currentNet.name,
            id: nets.size,
            connections,
          });
          currentNet = null;
          inNet = false;
        }
        braceDepth = 0;
      }
    }

    braceDepth += openBraces;
  }

  return {
    snapshot: { board, components, nets: Array.from(nets.values()), violations: [], traces: [], vias: [], ratsnest: [] },
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
// Geometry utilities (used by MockPcbEngine hit-testing and DRC)
// ============================================================================

/**
 * Shortest distance from point (px,py) to line segment (ax,ay)-(bx,by).
 */
function pointToSegmentDistance(
  px: number, py: number,
  ax: number, ay: number,
  bx: number, by: number,
): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) {
    const ex = px - ax;
    const ey = py - ay;
    return Math.sqrt(ex * ex + ey * ey);
  }
  let t = ((px - ax) * dx + (py - ay) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  const cx = ax + t * dx;
  const cy = ay + t * dy;
  const ex = px - cx;
  const ey = py - cy;
  return Math.sqrt(ex * ex + ey * ey);
}

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
class WasmPcbEngineAdapter implements PcbEngine {
  private wasmEngine: WasmPcbEngine;
  private cachedSnapshot: BoardSnapshot | null = null;

  constructor(wasmEngine: WasmPcbEngine) {
    this.wasmEngine = wasmEngine;
  }

  load_source(source: string): string {
    // Parse in JavaScript
    const { snapshot, errors } = parseSource(source);

    // Cache snapshot (traces/vias/ratsnest will be populated by load_routes)
    this.cachedSnapshot = snapshot;

    // Store snapshot and load into WASM engine for queries
    const wasmError = this.wasmEngine.load_snapshot(snapshot);
    if (wasmError) {
      errors.push(wasmError);
    }

    return errors.join('\n');
  }

  load_routes(sesContent: string): void {
    if (!this.cachedSnapshot) return;

    // Parse .ses file and extract routes
    const { traces, vias } = parseSesFile(sesContent);

    // Replace traces and vias in cached snapshot
    this.cachedSnapshot.traces = traces;
    this.cachedSnapshot.vias = vias;

    // Build set of routed connections (net + pin)
    const routedPins = new Set<string>();
    for (const trace of traces) {
      if (trace.net_name) {
        // For each net, we consider all pins in that net as "connected" if there are traces
        const net = this.cachedSnapshot.nets.find(n => n.name === trace.net_name);
        if (net) {
          for (const conn of net.connections) {
            routedPins.add(`${conn.component}.${conn.pin}`);
          }
        }
      }
    }

    // Regenerate ratsnest only for unrouted connections
    this.cachedSnapshot.ratsnest = [];
    for (const net of this.cachedSnapshot.nets) {
      if (net.connections.length < 2) continue;

      // Check if this net has any traces
      const hasTraces = traces.some(t => t.net_name === net.name);
      if (hasTraces) continue; // Skip routed nets

      // Get pin positions for unrouted net
      const positions: { x: number; y: number }[] = [];
      for (const conn of net.connections) {
        const comp = this.cachedSnapshot.components.find(c => c.refdes === conn.component);
        if (comp) {
          const pad = comp.pads.find(p => p.number === conn.pin);
          positions.push({
            x: comp.x_nm + (pad?.x_nm ?? 0),
            y: comp.y_nm + (pad?.y_nm ?? 0),
          });
        }
      }

      // Create star-topology ratsnest
      if (positions.length >= 2) {
        for (let i = 1; i < positions.length; i++) {
          this.cachedSnapshot.ratsnest.push({
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

  get_snapshot(): BoardSnapshot {
    // Return cached snapshot with traces/ratsnest that we added in JS
    // The WASM engine's get_snapshot() would have empty traces since
    // we only populated components/board, not Trace entities
    if (this.cachedSnapshot) {
      // Get DRC violations from WASM (computed in Rust)
      const wasmSnapshot = this.wasmEngine.get_snapshot();
      return {
        ...this.cachedSnapshot,
        violations: wasmSnapshot.violations || [],
      };
    }
    return this.wasmEngine.get_snapshot();
  }

  query_point(x_nm: number, y_nm: number): string[] {
    // Use WASM spatial index for efficient queries
    // The WASM engine rebuilds the spatial index in populate_from_snapshot()
    return this.wasmEngine.query_point(BigInt(x_nm), BigInt(y_nm));
  }

  add_trace(net_name: string, layer: string, width_nm: number, segments: number[]): number {
    const id = this.wasmEngine.add_trace_json(net_name, layer, BigInt(width_nm), JSON.stringify(segments));
    // Invalidate cached snapshot so next get_snapshot() picks up the new trace
    this.cachedSnapshot = null;
    return id;
  }

  remove_trace(trace_id: number): boolean {
    const removed = this.wasmEngine.remove_trace(trace_id);
    if (removed) {
      this.cachedSnapshot = null;
    }
    return removed;
  }

  get_trace_at_point(x_nm: number, y_nm: number, tolerance_nm: number): number {
    return this.wasmEngine.get_trace_at_point(BigInt(x_nm), BigInt(y_nm), BigInt(tolerance_nm));
  }

  run_drc_incremental(): number {
    return this.wasmEngine.run_drc_incremental();
  }

  trace_count(): number {
    return this.wasmEngine.trace_count();
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
  /** Next mock entity ID counter */
  private nextEntityId = 1000;

  load_source(source: string): string {
    const { snapshot, errors } = parseSource(source);
    this.snapshot = snapshot;
    return errors.join('\n');
  }

  load_routes(sesContent: string): void {
    // Parse .ses file and extract routes
    const { traces, vias } = parseSesFile(sesContent);

    // Replace traces and vias
    this.snapshot.traces = traces;
    this.snapshot.vias = vias;

    // Regenerate ratsnest only for unrouted nets
    this.snapshot.ratsnest = [];
    for (const net of this.snapshot.nets) {
      if (net.connections.length < 2) continue;

      // Skip nets that have traces
      const hasTraces = traces.some(t => t.net_name === net.name);
      if (hasTraces) continue;

      // Get pin positions for unrouted net
      const positions: { x: number; y: number }[] = [];
      for (const conn of net.connections) {
        const comp = this.snapshot.components.find(c => c.refdes === conn.component);
        if (comp) {
          const pad = comp.pads.find(p => p.number === conn.pin);
          positions.push({
            x: comp.x_nm + (pad?.x_nm ?? 0),
            y: comp.y_nm + (pad?.y_nm ?? 0),
          });
        }
      }

      // Create star-topology ratsnest
      if (positions.length >= 2) {
        for (let i = 1; i < positions.length; i++) {
          this.snapshot.ratsnest.push({
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
    const traceSegments: TraceSegmentInfo[] = [];
    for (let i = 0; i < segments.length; i += 4) {
      traceSegments.push({
        start_x: segments[i],
        start_y: segments[i + 1],
        end_x: segments[i + 2],
        end_y: segments[i + 3],
      });
    }

    // Normalize layer name
    const normalizedLayer = layer === 'TopCopper' ? 'Top' : layer === 'BottomCopper' ? 'Bottom' : layer;

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

    this.snapshot.violations = violations;
    console.log(`[MockEngine] run_drc_incremental: ${violations.length} violations`);
    return violations.length;
  }

  trace_count(): number {
    return this.snapshot.traces.length;
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
