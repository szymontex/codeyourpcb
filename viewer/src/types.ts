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
  /** Copper pours, as the copper they become (absent from older snapshots) */
  pours?: PourInfo[];
  /** Zones as the design states them, sent to the engine so it can fill them */
  zones?: ZoneInfo[];
}

/**
 * A zone as written: an outline, a layer and possibly a net.
 *
 * The host parses these because it holds the source text. What they become -
 * copper cut around every pad and trace on the layer - is computed in the
 * engine and comes back as `pours`, so the screen and the Gerber cannot
 * disagree.
 */
export interface ZoneInfo {
  /** Name the design gave it, empty when it gave none */
  name: string;
  /** "pour" for copper, "keepout" for an area nothing may enter */
  kind: string;
  /** Layers it covers, as a layer mask: bit 0 top, bit 1 bottom */
  layer_mask: number;
  /** Net name it pours to, empty when it names none */
  net: string;
  /** Its outline: [min x, min y, max x, max y] in nm */
  bounds: [number, number, number, number];
}

/**
 * A copper pour, as the copper it actually becomes.
 *
 * The engine sends the rectangles a fabricator receives, not the zone the
 * designer drew: a plane is its outline minus every piece of foreign copper
 * and the clearance around it. Drawing the outline would hide exactly the
 * mistakes a pour causes - a plane swallowing a pad, an island cut off from
 * the net it is meant to be.
 */
export interface PourInfo {
  /** Net name this pour belongs to, empty when it names none */
  net: string;
  /** Copper layers it covers, as a layer mask */
  layer_mask: number;
  /** The filled copper: [min x, min y, max x, max y] in nm */
  rects: [number, number, number, number][];
}

/**
 * A DRC violation for display in the viewer
 */
export interface ViolationInfo {
  /** Violation type: clearance, drill-size, unconnected-pin, etc. */
  kind: string;
  /**
   * 1-based line of the definition this is about, when the model knows it.
   *
   * A violation is discovered in board coordinates, not source ones, so this
   * comes from the offending entity's own span. Absent for a violation the
   * model cannot trace back to a definition.
   */
  line?: number;
  /** 1-based column, alongside `line`. */
  column?: number;
  /** X location in nanometers */
  x_nm: number;
  /** Y location in nanometers */
  y_nm: number;
  /** Human-readable message */
  message: string;
  /**
   * The copper this is about, where it is an area rather than a point:
   * [min x, min y, max x, max y] in nm.
   *
   * A clearance fault happens at a place. An orphaned pour island is a sheet,
   * and zooming to its centre shows copper that looks like every other part of
   * the plane.
   */
  area?: [number, number, number, number];
}

export interface BoardInfo {
  name: string;
  width_nm: number;
  height_nm: number;
  layer_count: number;
  /**
   * The board's real edge, when the design states one.
   *
   * `[x, y]` pairs in nanometres, closing back on the first. Absent means the
   * board is the rectangle `width_nm` by `height_nm` describes - which is what
   * this screen drew for every board, whatever shape it was, until the outline
   * reached the snapshot.
   */
  outline?: Array<[number, number]>;
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
  /**
   * Layer the via starts on, named the way the DSL names it.
   *
   * Absent on snapshots made before the span was carried, and a via that says
   * nothing goes through - which is what every via was until then.
   */
  start_layer?: string;
  /** Layer the via ends on. */
  end_layer?: string;
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
