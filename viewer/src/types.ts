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
  /** Component body width in nanometers (from footprint bounds). */
  body_width_nm: number;
  /** Component body height in nanometers (from footprint bounds). */
  body_height_nm: number;
  /** Optional path/key to a GLB 3D model file (null until populated). */
  model_3d: string | null;
  /** Silkscreen shapes (outlines, markers, text) relative to component origin */
  silk: SilkShape[];
}

/**
 * A silkscreen drawing primitive, relative to component origin.
 * Coordinates in nanometers.
 */
/**
 * `layer` is optional because nothing requires it: the engine does not send
 * one - the board model keeps a footprint's artwork in footprint coordinates
 * and the part's own side decides where it prints - and the renderer draws
 * every shape in the silkscreen colour without asking. Declaring it required
 * described neither producer.
 */
export type SilkShape =
  | { type: 'segment'; x1: number; y1: number; x2: number; y2: number; width: number; layer?: 'top' | 'bottom' }
  | { type: 'circle'; cx: number; cy: number; radius: number; width: number; layer?: 'top' | 'bottom' }
  | { type: 'arc'; cx: number; cy: number; radius: number; startAngle: number; endAngle: number; width: number; layer?: 'top' | 'bottom' };

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
  /** Trace width constraint in nm (from `[width 0.3mm]`). */
  width_nm?: number;
  /** Clearance constraint in nm (from `[clearance 0.2mm]`). */
  clearance_nm?: number;
  /** Current constraint in milliamps (from `[current 2A]`). */
  current_ma?: number;
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
  /** Entity index for selection/hit-testing */
  id: number;
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
  /** Entity index for selection/hit-testing */
  id: number;
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
