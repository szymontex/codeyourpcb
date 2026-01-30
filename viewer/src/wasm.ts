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

import type { BoardSnapshot, ComponentInfo, PadInfo, NetInfo, PinRef, BoardInfo } from './types';

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
  const padTemplates: Record<string, PadInfo[]> = {
    '0402': [
      { number: '1', x_nm: -500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm: 500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    '0603': [
      { number: '1', x_nm: -800_000, y_nm: 0, width_nm: 900_000, height_nm: 800_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm: 800_000, y_nm: 0, width_nm: 900_000, height_nm: 800_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    '0805': [
      { number: '1', x_nm: -950_000, y_nm: 0, width_nm: 1_100_000, height_nm: 1_200_000, shape: 'rect', layer_mask: 1, drill_nm: null },
      { number: '2', x_nm: 950_000, y_nm: 0, width_nm: 1_100_000, height_nm: 1_200_000, shape: 'rect', layer_mask: 1, drill_nm: null },
    ],
    // DIP-8: 8-pin through-hole, 100mil pitch, 300mil row spacing
    // Pins 1-4 on left side, 5-8 on right side (standard DIP numbering)
    // layer_mask: 3 = both top (1) and bottom (2) = through-hole
    'DIP-8': [
      { number: '1', x_nm: -3_810_000, y_nm:  1_905_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '2', x_nm: -3_810_000, y_nm:   635_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '3', x_nm: -3_810_000, y_nm:  -635_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '4', x_nm: -3_810_000, y_nm: -1_905_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '5', x_nm:  3_810_000, y_nm: -1_905_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '6', x_nm:  3_810_000, y_nm:  -635_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '7', x_nm:  3_810_000, y_nm:   635_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
      { number: '8', x_nm:  3_810_000, y_nm:  1_905_000, width_nm: 1_600_000, height_nm: 1_600_000, shape: 'circle', layer_mask: 3, drill_nm: 800_000 },
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
    const rawEngine = new wasm.PcbEngine() as WasmPcbEngine;
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
