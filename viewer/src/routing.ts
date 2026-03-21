/**
 * Manual routing state machine and utilities.
 *
 * KiCad-compatible interactive routing:
 * - Traces constrained to H/V/45° segments
 * - 2-segment paths: one H/V + one 45° diagonal (or vice versa)
 * - Automatic posture detection from mouse movement trail
 * - '/' key to flip posture (straight-first ↔ diagonal-first)
 * - Magnetic snap to target pads
 * - Grid snap
 */

import type { ComponentInfo, PadInfo, TraceSegmentInfo, ViolationInfo, BoardSnapshot } from './types';
import type { PcbEngine } from './wasm';
import {
  type Vec2, Dir45, CornerMode,
  dirFromSeg, buildInitialTrace,
  MouseTrailTracer,
} from './direction45';
import { dodgeObstacles } from './dodge';

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
  /** KiCad-style multi-segment preview (replaces previewSegment for rendering) */
  previewPath: Vec2[];
  /** Current posture direction (KiCad DIRECTION_45) */
  currentDirection: Dir45;
  /** Corner mode: 45° mitered (default) or 90° */
  cornerMode: CornerMode;
  /** Obstacles detected on the preview path (pads/traces of other nets) */
  obstacles: ObstacleInfo[];
  /** Whether the current preview path has collisions */
  hasCollision: boolean;
  /** Minimum copper clearance in nm (from design rules, default 150_000 = 0.15mm) */
  clearanceNm: number;
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

// Module-level mouse trail tracer (KiCad-style posture detection)
const _mouseTrail = new MouseTrailTracer();
let _lastWalkaroundLog = 0;

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
    angleSnapEnabled: true, // KiCad: 45° snap is ON by default
    magneticSnapEnabled: true,
    magneticSnapRadius: 1_000_000, // 1mm
    snappedToPad: null,
    targetPads: [],
    previewPath: [],
    currentDirection: Dir45.UNDEFINED,
    cornerMode: CornerMode.MITERED_45,
    obstacles: [],
    hasCollision: false,
    clearanceNm: 150_000, // default; overridden from engine at route start
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
      const padRadius = Math.sqrt(Number(pad.width_nm) * Number(pad.width_nm) + Number(pad.height_nm) * Number(pad.height_nm)) / 2;
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
  console.log(`[Route] Angle snap: ${next ? 'ON (45°)' : 'OFF (free)'}`);
  return { ...state, angleSnapEnabled: next };
}

/**
 * Flip posture: straight-first ↔ diagonal-first.
 * KiCad: '/' key.
 */
export function flipPosture(state: RoutingState): RoutingState {
  if (state.mode !== 'routing') return state;
  _mouseTrail.flipPosture();
  console.log('[Route] Posture flipped (/)');
  return { ...state };
}

/**
 * Toggle corner mode: 45° mitered ↔ 90° only.
 */
export function toggleCornerMode(state: RoutingState): RoutingState {
  const next = state.cornerMode === CornerMode.MITERED_45
    ? CornerMode.MITERED_90
    : CornerMode.MITERED_45;
  console.log(`[Route] Corner mode: ${next === CornerMode.MITERED_45 ? '45° mitered' : '90° only'}`);
  return { ...state, cornerMode: next };
}

// ---------------------------------------------------------------------------
// Target pad computation & magnetic snap
// ---------------------------------------------------------------------------

/**
 * Compute pad center in world coordinates, accounting for component rotation.
 * (public version for use by computeTargetPads)
 */
export function padWorldPosition(comp: ComponentInfo, pad: PadInfo): [number, number] {
  const radians = (Number(comp.rotation_mdeg) / 1000) * (Math.PI / 180);
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const px = Number(pad.x_nm), py = Number(pad.y_nm);
  const rx = px * cos - py * sin;
  const ry = px * sin + py * cos;
  return [Number(comp.x_nm) + rx, Number(comp.y_nm) + ry];
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

  // Initialize mouse trail tracer (KiCad-style posture detection)
  _mouseTrail.clear();
  _mouseTrail.setDefaultDirections(Dir45.N, Dir45.UNDEFINED);
  _mouseTrail.addTrailPoint({ x: padHit.worldX, y: padHit.worldY });

  console.log(`[Route] idle → routing: pad ${padHit.component.refdes}.${padHit.pad.number} net=${padHit.netName} layer=${layer} targets=${targets.length}`);

  return {
    ...state,
    mode: 'routing',
    startPad: padHit,
    currentLayer: layer,
    anchorPoint: { x: padHit.worldX, y: padHit.worldY },
    committedSegments: [],
    previewSegment: null,
    previewPath: [],
    snapAngle: 0,
    netName: padHit.netName,
    drcViolations: [],
    snappedToPad: null,
    targetPads: targets,
    currentDirection: Dir45.UNDEFINED,
    cornerMode: state.cornerMode,
  };
}

/**
 * Update the preview trace during mouse move (while routing).
 *
 * KiCad-style behavior:
 * 1. Grid snap (if enabled)
 * 2. Magnetic snap to target pads (if cursor near one)
 * 3. Add point to mouse trail tracer
 * 4. Get posture from trail tracer (or manual override)
 * 5. Build 2-segment trace via BuildInitialTrace (H/V + 45° diagonal)
 *
 * The preview is a multi-point path, not a single segment.
 */
export function updatePreview(
  state: RoutingState,
  cursorWorld: { x: number; y: number },
  viewportScale?: number,
  snapshot?: BoardSnapshot | null,
  padNetMap?: Map<string, string>,
): RoutingState {
  if (state.mode !== 'routing') return state;

  // Apply grid snap first if enabled
  const gridAdjusted = state.gridSnapEnabled
    ? snapToGrid(cursorWorld, state.gridSpacing)
    : cursorWorld;

  // Magnetic snap: check if cursor is near a target pad
  const scale = viewportScale ?? 1;
  const magneticHit = findNearestTargetPad(gridAdjusted.x, gridAdjusted.y, state, scale);

  let endPoint: Vec2;

  if (magneticHit) {
    endPoint = { x: magneticHit.worldX, y: magneticHit.worldY };
  } else {
    endPoint = { x: gridAdjusted.x, y: gridAdjusted.y };
  }

  // Feed mouse trail tracer (KiCad posture detection)
  _mouseTrail.addTrailPoint(endPoint);

  // Get posture direction from mouse trail
  const anchor: Vec2 = state.anchorPoint;
  let direction: Dir45;

  if (state.angleSnapEnabled) {
    // KiCad-style: determine posture automatically from mouse trail
    direction = _mouseTrail.getPosture(endPoint);
  } else {
    // Free angle mode — no 45° constraint
    direction = Dir45.UNDEFINED;
  }

  // Build the 2-segment trace path using KiCad's BuildInitialTrace
  let previewPath: Vec2[];

  if (state.angleSnapEnabled) {
    previewPath = buildInitialTrace(anchor, endPoint, direction, undefined, state.cornerMode);
  } else {
    // Free angle: just a straight line
    previewPath = [{ ...anchor }, { ...endPoint }];
  }

  // Compute angle for display
  const dx = endPoint.x - anchor.x;
  const dy = endPoint.y - anchor.y;
  const angleDeg = Math.round(((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 360);

  // Check for collisions with pads/traces of other nets
  const fullPath = [
    ...state.committedSegments.flatMap(s => [
      { x: s.start_x, y: s.start_y },
    ]),
    ...previewPath,
  ];
  const obstacles = checkRouteObstacles(
    fullPath.length >= 2 ? fullPath : previewPath,
    snapshot ?? null,
    state.netName,
    state.clearanceNm,
    state.traceWidth,
    padNetMap,
  );

  // Dodge obstacles: reroute around pads of other nets
  let finalPath = previewPath;
  let hasCollision = obstacles.length > 0;

  if (hasCollision && snapshot) {
    // Build padNetMap on-the-fly if not provided (fallback)
    let netMap = padNetMap;
    if (!netMap || netMap.size === 0) {
      netMap = new Map<string, string>();
      for (const net of (snapshot.nets ?? [])) {
        for (const conn of net.connections) {
          netMap.set(`${conn.component}.${conn.pin}`, net.name);
        }
      }
    }
    if (netMap.size > 0) {
      finalPath = dodgeObstacles(previewPath, snapshot, state.netName, state.clearanceNm, state.traceWidth, netMap);
      const remaining = checkRouteObstacles(finalPath, snapshot, state.netName, state.clearanceNm, state.traceWidth, netMap);
      hasCollision = remaining.length > 0;
    }
  }

  // Recompute legacy previewSegment from final path
  const finalLastIdx = finalPath.length - 1;
  const finalPreviewSegment: TraceSegmentInfo | null = finalPath.length >= 2
    ? {
        start_x: finalPath[finalLastIdx - 1].x,
        start_y: finalPath[finalLastIdx - 1].y,
        end_x: finalPath[finalLastIdx].x,
        end_y: finalPath[finalLastIdx].y,
      }
    : null;

  return {
    ...state,
    previewSegment: finalPreviewSegment,
    previewPath: finalPath,
    snapAngle: angleDeg,
    snappedToPad: magneticHit,
    currentDirection: direction,
    obstacles,
    hasCollision,
  };
}

/**
 * Add a waypoint (click in empty space while routing).
 * Commits the preview path and starts a new segment from the endpoint.
 * Sets direction from the last committed segment (KiCad behavior).
 */
export function addWaypoint(state: RoutingState): RoutingState {
  if (state.mode !== 'routing' || state.previewPath.length < 2) return state;

  const path = state.previewPath;
  const newSegments: TraceSegmentInfo[] = [];

  // Convert path points to segments
  for (let i = 0; i < path.length - 1; i++) {
    const a = path[i];
    const b = path[i + 1];
    // Skip zero-length segments
    if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
    newSegments.push({
      start_x: a.x, start_y: a.y,
      end_x: b.x, end_y: b.y,
    });
  }

  if (newSegments.length === 0) return state;

  const lastSeg = newSegments[newSegments.length - 1];
  const endPt = { x: lastSeg.end_x, y: lastSeg.end_y };
  const newDir = dirFromSeg(
    { x: lastSeg.start_x, y: lastSeg.start_y },
    endPt,
  );

  console.log(`[Route] waypoint at (${(endPt.x / 1e6).toFixed(2)}, ${(endPt.y / 1e6).toFixed(2)})mm dir=${Dir45[newDir]}`);

  // Reset mouse trail for new segment
  _mouseTrail.clear();
  _mouseTrail.setDefaultDirections(newDir, Dir45.UNDEFINED);
  _mouseTrail.addTrailPoint(endPt);

  return {
    ...state,
    committedSegments: [...state.committedSegments, ...newSegments],
    anchorPoint: endPt,
    previewSegment: null,
    previewPath: [],
    currentDirection: newDir,
  };
}

/**
 * Complete the route by clicking on a target pad.
 * Builds final segments from anchor to target using BuildInitialTrace.
 */
export function completeRoute(
  state: RoutingState,
  targetPad: PadHit,
): { segments: TraceSegmentInfo[]; netName: string; layer: string; width: number } | null {
  if (state.mode !== 'routing') return null;

  const target: Vec2 = { x: targetPad.worldX, y: targetPad.worldY };
  const anchor: Vec2 = state.anchorPoint;

  // Build final path from anchor to target pad
  const finalPath = state.angleSnapEnabled
    ? buildInitialTrace(anchor, target, state.currentDirection, undefined, state.cornerMode)
    : [{ ...anchor }, { ...target }];

  // Convert path to segments
  const finalSegments: TraceSegmentInfo[] = [];
  for (let i = 0; i < finalPath.length - 1; i++) {
    const a = finalPath[i];
    const b = finalPath[i + 1];
    if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
    finalSegments.push({
      start_x: a.x, start_y: a.y,
      end_x: b.x, end_y: b.y,
    });
  }

  const allSegments = [...state.committedSegments, ...finalSegments];

  console.log(`[Route] routing → idle: completed ${allSegments.length} segments to ${targetPad.component.refdes}.${targetPad.pad.number}`);

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
    cornerMode: state.cornerMode,
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
// Obstacle detection — KiCad MarkObstacles mode (simplified)
// ---------------------------------------------------------------------------

export interface ObstacleInfo {
  /** Type of obstacle */
  type: 'pad' | 'trace' | 'via';
  /** World position of collision */
  x: number;
  y: number;
  /** Net name of the obstacle (empty if none) */
  netName: string;
  /** Component refdes (for pad obstacles) */
  refdes?: string;
  /** Pad number (for pad obstacles) */
  padNumber?: string;
}

/**
 * Check if a line segment intersects a rectangle (pad bounding box).
 * Uses Liang-Barsky algorithm for segment-rect intersection.
 */
function segmentIntersectsRect(
  sx: number, sy: number, ex: number, ey: number,
  rx: number, ry: number, rw: number, rh: number,
): boolean {
  const dx = ex - sx;
  const dy = ey - sy;

  const p = [-dx, dx, -dy, dy];
  const q = [sx - rx, rx + rw - sx, sy - ry, ry + rh - sy];

  let tMin = 0;
  let tMax = 1;

  for (let i = 0; i < 4; i++) {
    if (Math.abs(p[i]) < 1e-10) {
      if (q[i] < 0) return false;
    } else {
      const t = q[i] / p[i];
      if (p[i] < 0) {
        tMin = Math.max(tMin, t);
      } else {
        tMax = Math.min(tMax, t);
      }
      if (tMin > tMax) return false;
    }
  }

  return true;
}

/**
 * Check if a line segment comes within clearance of a circular obstacle.
 * Used for round pads and vias.
 */
function segmentNearCircle(
  sx: number, sy: number, ex: number, ey: number,
  cx: number, cy: number, radius: number,
): boolean {
  // Point-to-segment distance
  const dx = ex - sx;
  const dy = ey - sy;
  const lenSq = dx * dx + dy * dy;

  if (lenSq < 1) {
    // Zero-length segment — point-to-point distance
    const d = Math.hypot(cx - sx, cy - sy);
    return d <= radius;
  }

  let t = ((cx - sx) * dx + (cy - sy) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  const nearX = sx + t * dx;
  const nearY = sy + t * dy;
  const dist = Math.hypot(cx - nearX, cy - nearY);
  return dist <= radius;
}

/**
 * Check the preview path for collisions with pads of OTHER nets.
 * Returns list of obstacles found.
 *
 * KiCad equivalent: NODE::CheckColliding + NearestObstacle
 *
 * @param path     Preview path points (from BuildInitialTrace)
 * @param snapshot Board snapshot with all components
 * @param netName  Net being routed (pads on this net are exempt)
 * @param clearanceNm  Minimum clearance to maintain (default 150μm = 6mil)
 * @param traceWidth  Width of the trace being routed (nm)
 */
export function checkRouteObstacles(
  path: Vec2[],
  snapshot: BoardSnapshot | null,
  netName: string,
  clearanceNm: number = 150_000,
  traceWidth: number = 250_000,
  padNetMap?: Map<string, string>,
): ObstacleInfo[] {
  if (!snapshot || path.length < 2) return [];

  const obstacles: ObstacleInfo[] = [];
  const halfTrace = traceWidth / 2;
  const exclusionRadius = clearanceNm + halfTrace;

  for (const comp of snapshot.components) {
    const radians = (Number(comp.rotation_mdeg) / 1000) * (Math.PI / 180);
    const cos = Math.cos(radians);
    const sin = Math.sin(radians);

    for (const pad of comp.pads) {
      // Check which net this pad is on
      const padKey = `${comp.refdes}.${pad.number}`;
      const padNet = padNetMap?.get(padKey) ?? '';

      // Skip pads on the same net — they're our targets, not obstacles
      if (padNet === netName) continue;
      // Skip pads with no net — unconnected pads
      if (!padNet) continue;

      // Compute pad world position — Number() guards BigInt from WASM
      const pxn = Number(pad.x_nm), pyn = Number(pad.y_nm);
      const rx = pxn * cos - pyn * sin;
      const ry = pxn * sin + pyn * cos;
      const padX = Number(comp.x_nm) + rx;
      const padY = Number(comp.y_nm) + ry;

      // Pad bounding box with clearance
      const padW = Number(pad.width_nm) + exclusionRadius * 2;
      const padH = Number(pad.height_nm) + exclusionRadius * 2;

      // Check each segment of the path
      for (let i = 0; i < path.length - 1; i++) {
        const a = path[i];
        const b = path[i + 1];

        let hit = false;

        if (pad.shape === 'circle') {
          const padRadius = Number(pad.width_nm) / 2 + exclusionRadius;
          hit = segmentNearCircle(a.x, a.y, b.x, b.y, padX, padY, padRadius);
        } else {
          // Rectangle-based check
          hit = segmentIntersectsRect(
            a.x, a.y, b.x, b.y,
            padX - padW / 2, padY - padH / 2, padW, padH,
          );
        }

        if (hit) {
          obstacles.push({
            type: 'pad',
            x: padX,
            y: padY,
            netName: padNet,
            refdes: comp.refdes,
            padNumber: pad.number,
          });
          break; // One obstacle per pad is enough
        }
      }
    }
  }

  // Check existing traces of other nets
  if (snapshot.traces) {
    for (const trace of snapshot.traces) {
      if (trace.net_name === netName) continue;
      if (!trace.net_name) continue;

      for (const seg of trace.segments) {
        for (let i = 0; i < path.length - 1; i++) {
          const a = path[i];
          const b = path[i + 1];

          // Check if trace segments are within clearance
          // Simplified: check if any endpoint of our path segment is near the trace segment
          const traceClearance = clearanceNm + halfTrace + trace.width / 2;
          if (
            segmentNearCircle(seg.start_x, seg.start_y, seg.end_x, seg.end_y,
              (a.x + b.x) / 2, (a.y + b.y) / 2, traceClearance)
          ) {
            obstacles.push({
              type: 'trace',
              x: (seg.start_x + seg.end_x) / 2,
              y: (seg.start_y + seg.end_y) / 2,
              netName: trace.net_name,
            });
            break;
          }
        }
        if (obstacles.some(o => o.type === 'trace' && o.netName === trace.net_name)) break;
      }
    }
  }

  return obstacles;
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

  // Add preview path segments (KiCad-style multi-segment)
  if (state.previewPath && state.previewPath.length >= 2) {
    for (let i = 0; i < state.previewPath.length - 1; i++) {
      const a = state.previewPath[i];
      const b = state.previewPath[i + 1];
      if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
      segs.push({
        start_x: a.x, start_y: a.y,
        end_x: b.x, end_y: b.y,
      });
    }
  } else if (state.previewSegment) {
    segs.push(state.previewSegment);
  }

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
