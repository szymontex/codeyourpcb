/**
 * TypeScript interfaces matching Rust BoardSnapshot
 * These types will be used when receiving data from the WASM module
 */

export interface BoardSnapshot {
  board: BoardInfo | null;
  components: ComponentInfo[];
  nets: NetInfo[];
}

export interface BoardInfo {
  name: string;
  width_nm: number;
  height_nm: number;
  layer_count: number;
}

export interface ComponentInfo {
  refdes: string;
  value: string;
  x_nm: number;
  y_nm: number;
  rotation_mdeg: number;
  footprint: string;
  pads: PadInfo[];
}

export interface PadInfo {
  number: string;
  x_nm: number;
  y_nm: number;
  width_nm: number;
  height_nm: number;
  shape: string;
  layer_mask: number;
  drill_nm: number | null;
}

export interface NetInfo {
  name: string;
  id: number;
  connections: PinRef[];
}

export interface PinRef {
  component: string;
  pin: string;
}
