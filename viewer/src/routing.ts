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
  /** Whether grid snap is active */
  gridSnapEnabled: boolean;
  /** Grid spacing in nm (default 1_270_000 = 1.27mm = 50mil) */
  gridSpacing: number;
  /** Whether 45°/90° angle snap is active (toggle with A key) */
  angleSnapEnabled: boolean;
  /** Whether magnetic snap to destination pads is active */
  magneticSnapEnabled: boolean;
  /** Magnetic snap radius in nm (default 1mm) */
  magneticSnapRadius: number;
  /** Pad currently snapped to via magnetic snap (null if none in range) */
  snappedToPad: PadHit | null;
  /** All pads on the same net as startPad, pre-computed at route start */
  targetPads: PadHit[];
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
    gridSnapEnabled: false,
    gridSpacing: 1_270_000, // 1.27mm = 50mil default
    angleSnapEnabled: false,
    magneticSnapEnabled: true,
    magneticSnapRadius: 1_000_000, // 1mm
    snappedToPad: null,
    targetPads: [],
  };
}

// ---------------------------------------------------------------------------
// Pad hit-testing
// ---------------------------------------------------------------------------

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
// Grid snapping
// ---------------------------------------------------------------------------

/**
 * Snap a point to the nearest grid intersection.
 * Grid origin is (0, 0). Spacing is in nm.
 */
export function snapToGrid(
  point: { x: number; y: number },
  spacing: number,
): { x: number; y: number } {
  return {
    x: Math.round(point.x / spacing) * spacing,
    y: Math.round(point.y / spacing) * spacing,
  };
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
// Angle snap toggle
// ---------------------------------------------------------------------------

/**
 * Toggle 45°/90° angle constraint on/off.
 */
export function toggleAngleSnap(state: RoutingState): RoutingState {
  const next = !state.angleSnapEnabled;
  console.log(`[Route] Angle snap: ${next ? 'ON' : 'OFF'}`);
  return { ...state, angleSnapEnabled: next };
}

// ---------------------------------------------------------------------------
// Target pad computation & magnetic snap
// ---------------------------------------------------------------------------

/**
 * Compute pad center in world coordinates, accounting for component rotation.
 * (public version for use by computeTargetPads)
 */
export function padWorldPosition(comp: ComponentInfo, pad: PadInfo): [number, number] {
  const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const rx = pad.x_nm * cos - pad.y_nm * sin;
  const ry = pad.x_nm * sin + pad.y_nm * cos;
  return [comp.x_nm + rx, comp.y_nm + ry];
}

/**
 * Pre-compute all pads on a given net, excluding the start pad.
 * Called once when routing starts — avoids per-frame scanning.
 */
export function computeTargetPads(
  snapshot: BoardSnapshot | null,
  netName: string,
  excludeComp: string,
  excludePad: string,
): PadHit[] {
  if (!snapshot || !netName) return [];

  // Build set of pin refs on the net for fast lookup
  const pinRefs = new Set<string>();
  for (const net of snapshot.nets) {
    if (net.name === netName) {
      for (const conn of net.connections) {
        pinRefs.add(`${conn.component}.${conn.pin}`);
      }
      break;
    }
  }

  const result: PadHit[] = [];
  for (const comp of snapshot.components) {
    for (const pad of comp.pads) {
      const pinRef = `${comp.refdes}.${pad.number}`;
      if (!pinRefs.has(pinRef)) continue;
      // Exclude the start pad itself
      if (comp.refdes === excludeComp && pad.number === excludePad) continue;

      const [wx, wy] = padWorldPosition(comp, pad);
      result.push({
        component: comp,
        pad,
        worldX: wx,
        worldY: wy,
        netName,
      });
    }
  }
  return result;
}

/**
 * Find the nearest target pad within the magnetic snap radius.
 * Uses dual threshold: world radius OR screen-pixel radius (15px / scale),
 * whichever is larger. This ensures pads are easy to snap to at any zoom.
 */
export function findNearestTargetPad(
  worldX: number,
  worldY: number,
  state: RoutingState,
  viewportScale: number,
): PadHit | null {
  if (!state.magneticSnapEnabled || state.targetPads.length === 0) return null;

  // Dual threshold: world radius OR 15px converted to world coords
  const screenRadiusWorld = 15 / viewportScale;
  const effectiveRadius = Math.max(state.magneticSnapRadius, screenRadiusWorld);

  let bestDist = Infinity;
  let bestPad: PadHit | null = null;

  for (const tp of state.targetPads) {
    const dx = worldX - tp.worldX;
    const dy = worldY - tp.worldY;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist <= effectiveRadius && dist < bestDist) {
      bestDist = dist;
      bestPad = tp;
    }
  }

  return bestPad;
}

// ---------------------------------------------------------------------------
// State machine transitions
// ---------------------------------------------------------------------------

/**
 * Start a route from a pad click.
 * Takes snapshot to pre-compute targetPads for the net.
 * Returns updated state (caller should set highlightedNet from state.netName).
 */
export function startRoute(
  state: RoutingState,
  padHit: PadHit,
  snapshot?: BoardSnapshot | null,
): RoutingState {
  // Detect layer from pad's layer mask
  const layer = (padHit.pad.layer_mask & 0x02) ? 'Bottom' : 'Top';

  // Pre-compute target pads for magnetic snap
  const targets = snapshot
    ? computeTargetPads(snapshot, padHit.netName, padHit.component.refdes, padHit.pad.number)
    : [];

  console.log(`[Route] idle → routing: pad ${padHit.component.refdes}.${padHit.pad.number} net=${padHit.netName} layer=${layer} targets=${targets.length}`);

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
    snappedToPad: null,
    targetPads: targets,
  };
}

/**
 * Update the preview segment during mouse move (while routing).
 *
 * Pipeline: grid snap → magnetic snap → angle snap (if enabled & not snapped).
 * Magnetic snap wins: when cursor is near a target pad, endpoint locks to pad center.
 * Angle snap is optional (toggle A) and only applies when no magnetic snap is active.
 *
 * @param viewportScale  Current viewport scale (px per nm) — needed for screen-px snap threshold
 */
export function updatePreview(
  state: RoutingState,
  cursorWorld: { x: number; y: number },
  viewportScale?: number,
): RoutingState {
  if (state.mode !== 'routing') return state;

  // Apply grid snap first if enabled
  const gridAdjusted = state.gridSnapEnabled
    ? snapToGrid(cursorWorld, state.gridSpacing)
    : cursorWorld;

  // Magnetic snap: check if cursor is near a target pad
  const scale = viewportScale ?? 1;
  const magneticHit = findNearestTargetPad(gridAdjusted.x, gridAdjusted.y, state, scale);

  let endPoint: { x: number; y: number };
  let angleDeg: number;

  if (magneticHit) {
    // Magnetic snap wins — lock to pad center
    endPoint = { x: magneticHit.worldX, y: magneticHit.worldY };
    // Compute angle for display only
    const dx = endPoint.x - state.anchorPoint.x;
    const dy = endPoint.y - state.anchorPoint.y;
    angleDeg = Math.round(((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360);
  } else if (state.angleSnapEnabled) {
    // Angle snap (only when enabled and not magnetically snapped)
    const snapped = computeSnappedPoint(state.anchorPoint, gridAdjusted);
    endPoint = { x: snapped.x, y: snapped.y };
    angleDeg = snapped.angleDeg;
  } else {
    // Free movement (no angle snap, no magnetic snap)
    endPoint = gridAdjusted;
    const dx = endPoint.x - state.anchorPoint.x;
    const dy = endPoint.y - state.anchorPoint.y;
    angleDeg = Math.round(((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360);
  }

  return {
    ...state,
    previewSegment: {
      start_x: state.anchorPoint.x,
      start_y: state.anchorPoint.y,
      end_x: endPoint.x,
      end_y: endPoint.y,
    },
    snapAngle: angleDeg,
    snappedToPad: magneticHit,
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

  // Note: caller is responsible for resetting state to idle (clearing snappedToPad, targetPads)
  return {
    segments: allSegments,
    netName: state.netName,
    layer: state.currentLayer,
    width: state.traceWidth,
  };
}

/**
 * Reset routing state to idle, preserving user preferences (grid snap, angle snap, magnetic snap).
 */
export function resetToIdle(state: RoutingState): RoutingState {
  return {
    ...createRoutingState(),
    gridSnapEnabled: state.gridSnapEnabled,
    gridSpacing: state.gridSpacing,
    angleSnapEnabled: state.angleSnapEnabled,
    magneticSnapEnabled: state.magneticSnapEnabled,
    magneticSnapRadius: state.magneticSnapRadius,
  };
}

/**
 * Cancel the in-progress route.
 */
export function cancelRoute(state: RoutingState): RoutingState {
  if (state.mode !== 'routing') return state;
  console.log('[Route] routing → idle: cancelled');
  return resetToIdle(state);
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
