/**
 * What the error panel calls each kind of DRC violation.
 *
 * The engine sends a slug - `diff-pair-skew`, `pour-island` - and the panel
 * shows a person an icon and a name. A kind that is missing here falls back to
 * the slug itself, which is how the panel came to show rows reading
 * "trace-current" next to rows reading "Trace too narrow".
 *
 * The list this has to match is the `Display` implementation of
 * `ViolationKind` in `crates/cypcb-drc/src/violation.rs`, and
 * `the-error-panel-names-every-violation.test.ts` reads that file and fails
 * when the two drift apart.
 */
export interface ViolationKindMeta {
  icon: string;
  label: string;
}

export const VIOLATION_KIND_META: Record<string, ViolationKindMeta> = {
  'clearance':           { icon: '⚡', label: 'Copper clearance' },
  'edge-clearance':      { icon: '📐', label: 'Edge clearance' },
  'trace-width':         { icon: '📏', label: 'Trace too narrow' },
  'trace-current':       { icon: '🔥', label: 'Trace too narrow for its current' },
  'impedance':           { icon: '〰️', label: 'Impedance off target' },
  'neck-down':           { icon: '🧵', label: 'Neck does not describe a neck' },
  'drill-size':          { icon: '🔩', label: 'Drill too small' },
  'via-drill':           { icon: '🔩', label: 'Via drill too small' },
  'via-diameter':        { icon: '⭕', label: 'Via too small' },
  'annular-ring':        { icon: '🔘', label: 'Annular ring' },
  'hole-to-hole':        { icon: '🕳️', label: 'Holes too close' },
  'unconnected-pin':     { icon: '🔌', label: 'Unconnected pin' },
  'unrouted-pin':        { icon: '🪢', label: 'Pin has a net but no copper' },
  'keepout-violation':   { icon: '🚫', label: 'Keepout zone' },
  'courtyard-clearance': { icon: '📦', label: 'Components overlap' },
  'solder-mask-bridge':  { icon: '🩹', label: 'Solder mask bridge' },
  'silk-clearance':      { icon: '🏷️', label: 'Silk over copper' },
  'diff-pair-skew':      { icon: '🚻', label: 'Differential pair skew' },
  'pour-island':         { icon: '🏝️', label: 'Orphaned copper island' },
  'assertion':           { icon: '📋', label: 'Design assertion failed' },
  'stackup':             { icon: '🥞', label: 'Stackup contradicts the board' },
  'paste-clearance':     { icon: '🩻', label: 'Paste stencil web too thin' },
  'hole-to-edge':        { icon: '🪚', label: 'Hole too close to the board edge' },
  'drill-aspect-ratio':  { icon: '🕳️', label: 'Hole too deep to plate' },
  'slot-clearance':      { icon: '🪚', label: 'Copper too close to a milled slot' },
  'pad-land':            { icon: '⭕', label: 'Land around a hole too small to image' },
  'via-span':            { icon: '🕳️', label: 'Via span this build does not drill' },
  'flex-hole':           { icon: '📐', label: 'Hole where the board bends' },
  'empty-area':          { icon: '▭', label: 'Declared area with no area' },
  'area-off-board':      { icon: '🚧', label: 'Declared area off the board' },
  'area-overlap':        { icon: '🔀', label: 'Two stacks over one strip of board' },
  'bend-radius':         { icon: '🌀', label: 'Fold tighter than the ribbon takes' },
};

/** The icon and name for a kind, falling back to the slug the engine sent. */
export function describeViolationKind(kind: string): ViolationKindMeta {
  return VIOLATION_KIND_META[kind] ?? { icon: '⚠️', label: kind };
}
