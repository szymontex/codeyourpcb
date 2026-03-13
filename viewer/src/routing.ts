/**
 * Manual routing state machine and utilities.
 *
 * States: idle → routing (on pad click) → idle (on pad click / Escape)
 *
 * While routing, the user sees a preview trace following the cursor
 * with 45°/90° snapping and live DRC violation overlay.
 */

import type { ComponentInfo, PadInfo, TraceSegmentInfo, ViolationInfo, BoardSnapshot } from './types';
import type { PcbEngine } from './wasm';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type RoutingMode = 'idle' | 'routing';

export interface RoutingState {
  mode: RoutingMode;
  /** Pad that started the route (null when idle) */
  startPad: PadHit | null;
  /** Current copper layer name ('Top' | 'Bottom') */
  currentLayer: string;
  /** World-coordinate anchor for the current segment (nm) */
  anchorPoint: { x: number; y: number };
  /** Completed segments so far (anchor→anchor for multi-click routes) */
  committedSegments: TraceSegmentInfo[];
  /** Preview segment from last anchor to snapped cursor (shown as dashed) */
  previewSegment: TraceSegmentInfo | null;
  /** Snap angle in degrees (0, 45, 90, …) applied to the preview */
  snapAngle: number;
  /** Net name being routed (from start pad) */
  netName: string;
  /** DRC violations detected during preview (live feedback) */
  drcViolations: ViolationInfo[];
  /** Trace width in nm (default 250_000 = 0.25mm) */
  traceWidth: number;
}

export interface PadHit {
  component: ComponentInfo;
  pad: PadInfo;
  /** Pad center in world coordinates (nm) */
  worldX: number;
  worldY: number;
  /** Net this pad belongs to (empty string if unknown) */
  netName: string;
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

export function createRoutingState(): RoutingState {
  return {
    mode: 'idle',
    startPad: null,
    currentLayer: 'Top',
    anchorPoint: { x: 0, y: 0 },
    committedSegments: [],
    previewSegment: null,
    snapAngle: 0,
    netName: '',
    drcViolations: [],
    traceWidth: 250_000, // 0.25mm default
  };
}

// ---------------------------------------------------------------------------
// Pad hit-testing
// ---------------------------------------------------------------------------

/**
 * Compute pad center in world coordinates, accounting for component rotation.
 */
function padWorldPosition(comp: ComponentInfo, pad: PadInfo): [number, number] {
  const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const rx = pad.x_nm * cos - pad.y_nm * sin;
  const ry = pad.x_nm * sin + pad.y_nm * cos;
  return [comp.x_nm + rx, comp.y_nm + ry];
}

/**
 * Find the pad closest to a world-coordinate point, within tolerance.
 *
 * @param snapshot   Current board snapshot
 * @param worldX     Click position in nm
 * @param worldY     Click position in nm
 * @param toleranceNm  Hit radius in nm
 * @returns The closest pad or null
 */
export function hitTestPad(
  snapshot: BoardSnapshot | null,
  worldX: number,
  worldY: number,
  toleranceNm: number,
): PadHit | null {
  if (!snapshot) return null;

  let bestDist = Infinity;
  let bestHit: PadHit | null = null;

  for (const comp of snapshot.components) {
    for (const pad of comp.pads) {
      const [px, py] = padWorldPosition(comp, pad);
      const dx = worldX - px;
      const dy = worldY - py;
      const dist = Math.sqrt(dx * dx + dy * dy);

      // Tolerance is the explicit radius PLUS half the pad diagonal
      const padRadius = Math.sqrt(pad.width_nm * pad.width_nm + pad.height_nm * pad.height_nm) / 2;
      if (dist <= padRadius + toleranceNm && dist < bestDist) {
        bestDist = dist;

        // Find which net this pad belongs to
        const pinRef = `${comp.refdes}.${pad.number}`;
        let netName = '';
        for (const net of snapshot.nets) {
          if (net.connections.some(c => `${c.component}.${c.pin}` === pinRef)) {
            netName = net.name;
            break;
          }
        }

        bestHit = { component: comp, pad, worldX: px, worldY: py, netName };
      }
    }
  }

  return bestHit;
}

// ---------------------------------------------------------------------------
// Angle snapping
// ---------------------------------------------------------------------------

/** Allowed snap angles in degrees */
const SNAP_ANGLES_DEG = [0, 45, 90, 135, 180, 225, 270, 315];

/**
 * Snap a cursor point to the nearest 45°/90° angle from an anchor.
 * Returns the snapped world-coordinate endpoint and the angle used.
 */
export function computeSnappedPoint(
  anchor: { x: number; y: number },
  cursor: { x: number; y: number },
): { x: number; y: number; angleDeg: number } {
  const dx = cursor.x - anchor.x;
  const dy = cursor.y - anchor.y;
  const distance = Math.sqrt(dx * dx + dy * dy);

  if (distance < 1) {
    return { x: anchor.x, y: anchor.y, angleDeg: 0 };
  }

  // Angle from anchor to cursor (degrees, 0 = right, counter-clockwise)
  const rawAngleDeg = (Math.atan2(dy, dx) * 180) / Math.PI;
  // Normalize to 0-360
  const normalAngle = ((rawAngleDeg % 360) + 360) % 360;

  // Find closest snap angle
  let bestAngle = 0;
  let bestDiff = 360;
  for (const snap of SNAP_ANGLES_DEG) {
    let diff = Math.abs(normalAngle - snap);
    if (diff > 180) diff = 360 - diff;
    if (diff < bestDiff) {
      bestDiff = diff;
      bestAngle = snap;
    }
  }

  const radians = (bestAngle * Math.PI) / 180;
  return {
    x: anchor.x + distance * Math.cos(radians),
    y: anchor.y + distance * Math.sin(radians),
    angleDeg: bestAngle,
  };
}

// ---------------------------------------------------------------------------
// State machine transitions
// ---------------------------------------------------------------------------

/**
 * Start a route from a pad click.
 */
export function startRoute(
  state: RoutingState,
  padHit: PadHit,
): RoutingState {
  // Detect layer from pad's layer mask
  const layer = (padHit.pad.layer_mask & 0x02) ? 'Bottom' : 'Top';

  console.log(`[Route] idle → routing: pad ${padHit.component.refdes}.${padHit.pad.number} net=${padHit.netName} layer=${layer}`);

  return {
    ...state,
    mode: 'routing',
    startPad: padHit,
    currentLayer: layer,
    anchorPoint: { x: padHit.worldX, y: padHit.worldY },
    committedSegments: [],
    previewSegment: null,
    snapAngle: 0,
    netName: padHit.netName,
    drcViolations: [],
  };
}

/**
 * Update the preview segment during mouse move (while routing).
 */
export function updatePreview(
  state: RoutingState,
  cursorWorld: { x: number; y: number },
): RoutingState {
  if (state.mode !== 'routing') return state;

  const snapped = computeSnappedPoint(state.anchorPoint, cursorWorld);

  return {
    ...state,
    previewSegment: {
      start_x: state.anchorPoint.x,
      start_y: state.anchorPoint.y,
      end_x: snapped.x,
      end_y: snapped.y,
    },
    snapAngle: snapped.angleDeg,
  };
}

/**
 * Add a waypoint (click in empty space while routing).
 * Commits the current preview segment and starts a new one from the endpoint.
 */
export function addWaypoint(state: RoutingState): RoutingState {
  if (state.mode !== 'routing' || !state.previewSegment) return state;

  const seg = state.previewSegment;
  console.log(`[Route] waypoint at (${(seg.end_x / 1e6).toFixed(2)}, ${(seg.end_y / 1e6).toFixed(2)})mm`);

  return {
    ...state,
    committedSegments: [...state.committedSegments, seg],
    anchorPoint: { x: seg.end_x, y: seg.end_y },
    previewSegment: null,
  };
}

/**
 * Complete the route by clicking on a target pad.
 * Returns the final list of segments to add as a trace.
 */
export function completeRoute(
  state: RoutingState,
  targetPad: PadHit,
): { segments: TraceSegmentInfo[]; netName: string; layer: string; width: number } | null {
  if (state.mode !== 'routing') return null;

  // Build final segment from last anchor to target pad center
  const finalSeg: TraceSegmentInfo = {
    start_x: state.anchorPoint.x,
    start_y: state.anchorPoint.y,
    end_x: targetPad.worldX,
    end_y: targetPad.worldY,
  };

  const allSegments = [...state.committedSegments, finalSeg];

  console.log(`[Route] routing → idle: completed ${allSegments.length} segments to ${targetPad.component.refdes}.${targetPad.pad.number}`);

  return {
    segments: allSegments,
    netName: state.netName,
    layer: state.currentLayer,
    width: state.traceWidth,
  };
}

/**
 * Cancel the in-progress route.
 */
export function cancelRoute(state: RoutingState): RoutingState {
  if (state.mode !== 'routing') return state;
  console.log('[Route] routing → idle: cancelled');
  return createRoutingState();
}

/**
 * Flip the active copper layer during routing (Top ↔ Bottom).
 * Called when the user presses 'F' while routing.
 */
export function flipLayer(state: RoutingState): RoutingState {
  if (state.mode !== 'routing') return state;
  const newLayer = state.currentLayer === 'Top' ? 'Bottom' : 'Top';
  console.log(`[Route] layer flip: ${state.currentLayer} → ${newLayer}`);
  return { ...state, currentLayer: newLayer };
}

/**
 * Set DRC violations on the routing state (called after debounced DRC check).
 */
export function setDrcViolations(state: RoutingState, violations: ViolationInfo[]): RoutingState {
  return { ...state, drcViolations: violations };
}

// ---------------------------------------------------------------------------
// DRC preview (debounced)
// ---------------------------------------------------------------------------

/**
 * Build the flat segment array for the preview trace (committed + preview).
 * Format: [x1,y1,x2,y2, ...] matching PcbEngine.add_trace() signature.
 */
export function previewSegmentsFlat(state: RoutingState): number[] {
  const segs = [...state.committedSegments];
  if (state.previewSegment) segs.push(state.previewSegment);
  const flat: number[] = [];
  for (const s of segs) {
    flat.push(
      Math.round(s.start_x), Math.round(s.start_y),
      Math.round(s.end_x), Math.round(s.end_y),
    );
  }
  return flat;
}

/**
 * Create a debounced DRC checker that runs at most once per `intervalMs`.
 * Adds a temporary preview trace, runs DRC, removes the trace, and calls
 * the callback with the violations detected.
 */
export function createDrcPreviewChecker(
  engine: PcbEngine,
  intervalMs: number = 100,
): {
  check: (state: RoutingState) => void;
  cancel: () => void;
  onViolations: (cb: (violations: ViolationInfo[]) => void) => void;
} {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let callback: ((violations: ViolationInfo[]) => void) | null = null;

  function check(state: RoutingState): void {
    if (timer !== null) return; // Already scheduled

    timer = setTimeout(() => {
      timer = null;
      if (state.mode !== 'routing') return;

      const flat = previewSegmentsFlat(state);
      if (flat.length < 4) return;

      // Add temporary trace
      const tempId = engine.add_trace(
        state.netName,
        state.currentLayer,
        state.traceWidth,
        flat,
      );

      if (tempId === 0xFFFFFFFF) return;

      // Run DRC
      const violationCount = engine.run_drc_incremental();

      // Get violations from snapshot
      const snap = engine.get_snapshot();
      const violations = snap.violations || [];

      // Remove temporary trace
      engine.remove_trace(tempId);

      console.log(`[Route] DRC preview: ${violationCount} violations`);

      if (callback) callback(violations);
    }, intervalMs);
  }

  function cancel(): void {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function onViolations(cb: (violations: ViolationInfo[]) => void): void {
    callback = cb;
  }

  return { check, cancel, onViolations };
}
