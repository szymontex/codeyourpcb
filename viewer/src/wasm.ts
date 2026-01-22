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

  constructor(wasmEngine: WasmPcbEngine) {
    this.wasmEngine = wasmEngine;
  }

  load_source(source: string): string {
    // Parse in JavaScript
    const { snapshot, errors } = parseSource(source);

    // Store snapshot and load into WASM engine for queries
    const wasmError = this.wasmEngine.load_snapshot(snapshot);
    if (wasmError) {
      errors.push(wasmError);
    }

    return errors.join('\n');
  }

  get_snapshot(): BoardSnapshot {
    // Get snapshot from WASM engine - includes DRC violations computed in Rust
    // The WASM engine rebuilds spatial index and runs DRC in load_snapshot()
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
    const wasmPath = '../pkg/cypcb_render.js';
    const wasm = await import(/* @vite-ignore */ wasmPath);
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
