/**
 * TypeScript interfaces matching Rust BoardSnapshot
 * These types will be used when receiving data from the WASM module
 */

export interface BoardSnapshot {
  board: BoardInfo | null;
  components: ComponentInfo[];
  nets: NetInfo[];
  violations: ViolationInfo[];
  traces: TraceInfo[];
  vias: ViaInfo[];
  ratsnest: RatsnestInfo[];
}

/**
 * A DRC violation for display in the viewer
 */
export interface ViolationInfo {
  /** Violation type: clearance, drill-size, unconnected-pin, etc. */
  kind: string;
  /** X location in nanometers */
  x_nm: number;
  /** Y location in nanometers */
  y_nm: number;
  /** Human-readable message */
  message: string;
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

/**
 * A single segment of a trace (line from start to end)
 */
export interface TraceSegmentInfo {
  start_x: number;
  start_y: number;
  end_x: number;
  end_y: number;
}

/**
 * Trace information for rendering
 */
export interface TraceInfo {
  segments: TraceSegmentInfo[];
  width: number;
  layer: string;
  net_name: string;
  locked: boolean;
}

/**
 * Via information for rendering
 */
export interface ViaInfo {
  x: number;
  y: number;
  drill: number;
  outer_diameter: number;
  net_name: string;
}

/**
 * Ratsnest line for unrouted connections
 */
export interface RatsnestInfo {
  start_x: number;
  start_y: number;
  end_x: number;
  end_y: number;
  net_name: string;
}
