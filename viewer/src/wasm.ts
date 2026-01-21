/**
 * WASM module loading utilities
 * Will be updated when the WASM module is built
 */

import type { BoardSnapshot } from './types';

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
}

/**
 * Load the WASM module and return the PCB engine instance
 *
 * @returns The PCB engine if loaded successfully, null if not ready
 */
export async function loadWasm(): Promise<PcbEngine | null> {
  // Placeholder - actual loading will be implemented after wasm-pack build
  // The module will be available at ./pkg/cypcb_render.js once built
  console.log('WASM loading not yet implemented');
  return null;
}
