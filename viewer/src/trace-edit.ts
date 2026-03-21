/**
 * Trace segment/corner editing — KiCad-style drag operations.
 *
 * dragSegment: shifts a segment parallel to itself, adjacent segments
 * stretch/shrink to meet. Simple perpendicular offset + line intersection.
 *
 * dragCorner: moves a vertex, rebuilds adjacent segments via BuildInitialTrace.
 */

import type { BoardSnapshot, TraceInfo, TraceSegmentInfo } from './types';
import { pointToSegmentDistance } from './geometry';
import {
  type Vec2, Dir45,
  dirFromSeg, isDiagonal, buildInitialTrace,
  angleBetween, AngleType,
} from './direction45';

// ---------------------------------------------------------------------------
// Hit-test
// ---------------------------------------------------------------------------

export interface TraceSegmentHit {
  traceId: number;
  trace: TraceInfo;
  segmentIndex: number;
  nearCorner: boolean;
  cornerIndex?: number;
}

export function hitTestTraceSegment(
  snapshot: BoardSnapshot | null,
  worldX: number, worldY: number,
  toleranceNm: number,
): TraceSegmentHit | null {
  if (!snapshot?.traces || snapshot.traces.length === 0) return null;

  let bestDist = Infinity;
  let bestHit: TraceSegmentHit | null = null;

  for (const trace of snapshot.traces) {
    const hw = Number(trace.width) / 2;
    const hitR = toleranceNm + hw;
    const cornerR = Math.max(Number(trace.width), toleranceNm);
    const verts = traceVertices(trace);

    // Corners first (priority)
    for (let vi = 0; vi < verts.length; vi++) {
      const d = Math.hypot(worldX - verts[vi].x, worldY - verts[vi].y);
      if (d <= cornerR && d < bestDist) {
        bestDist = d;
        bestHit = { traceId: trace.id, trace, segmentIndex: Math.min(vi, trace.segments.length - 1), nearCorner: true, cornerIndex: vi };
      }
    }

    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const d = pointToSegmentDistance(worldX, worldY, Number(seg.start_x), Number(seg.start_y), Number(seg.end_x), Number(seg.end_y));
      if (d <= hitR && d < bestDist) {
        const sd = Math.hypot(worldX - Number(seg.start_x), worldY - Number(seg.start_y));
        const ed = Math.hypot(worldX - Number(seg.end_x), worldY - Number(seg.end_y));
        if (Math.min(sd, ed) <= cornerR) continue;
        bestDist = d;
        bestHit = { traceId: trace.id, trace, segmentIndex: i, nearCorner: false };
      }
    }
  }
  return bestHit;
}

export function traceVertices(trace: TraceInfo): Vec2[] {
  if (trace.segments.length === 0) return [];
  const pts: Vec2[] = [{ x: Number(trace.segments[0].start_x), y: Number(trace.segments[0].start_y) }];
  for (const seg of trace.segments) pts.push({ x: Number(seg.end_x), y: Number(seg.end_y) });
  return pts;
}

function verticesToSegments(pts: Vec2[]): TraceSegmentInfo[] {
  const segs: TraceSegmentInfo[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1];
    if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
    segs.push({ start_x: Math.round(a.x), start_y: Math.round(a.y), end_x: Math.round(b.x), end_y: Math.round(b.y) });
  }
  return segs;
}

// ---------------------------------------------------------------------------
// Line-line intersection (infinite lines)
// ---------------------------------------------------------------------------

/** Intersect line through p1 in direction d1 with line through p2 in direction d2. */
function lineIsect(p1: Vec2, d1: Vec2, p2: Vec2, d2: Vec2): Vec2 | null {
  const cross = d1.x * d2.y - d1.y * d2.x;
  if (Math.abs(cross) < 0.001) return null;
  const t = ((p2.x - p1.x) * d2.y - (p2.y - p1.y) * d2.x) / cross;
  return { x: p1.x + t * d1.x, y: p1.y + t * d1.y };
}

// ---------------------------------------------------------------------------
// dragSegment — simple perpendicular shift
// ---------------------------------------------------------------------------

/**
 * Drag a trace segment — KiCad dragSegment45.
 *
 * KiCad behavior: draws a line through `newPos` in the direction of the
 * dragged segment. Intersects this line with "guide lines" extending from
 * the fixed endpoints of adjacent segments. The intersection points become
 * the new junction points, effectively sliding the break-points along
 * the adjacent segments.
 *
 * For a trace like: A──────B╲C
 * Dragging the diagonal B→C means:
 * - guideA is a line from A in direction of A→B (horizontal)
 * - guideB is a line from C in direction of C→(next point) or drag_dir perpendicular
 * - s_current is a line through mouse position in direction of B→C (the diagonal)
 * - ip1 = s_current ∩ guideA → new B position (slides along horizontal)
 * - ip2 = s_current ∩ guideB → new C position
 * Result: A → ip1 → ip2 → D (endpoints A and D stay fixed)
 */
export function dragSegment(
  segments: TraceSegmentInfo[],
  segIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (segments.length === 0 || segIndex < 0 || segIndex >= segments.length) return null;

  const pts = traceVertices({ segments } as TraceInfo);
  if (pts.length < 2) return null;

  const idx = segIndex;
  const dragA = pts[idx];
  const dragB = pts[idx + 1];

  const ddx = dragB.x - dragA.x;
  const ddy = dragB.y - dragA.y;
  if (Math.abs(ddx) < 1 && Math.abs(ddy) < 1) return null;

  // drag_dir vector (direction of the dragged segment)
  const dragDirV: Vec2 = { x: ddx, y: ddy };

  // s_current: line through newPos in drag direction
  // guideA: line from the FIXED point before the dragged segment, in the direction of the previous segment
  // guideB: line from the FIXED point after the dragged segment, in the direction of the next segment

  let fixedA: Vec2; // anchor point for guide A (won't move)
  let guideDirA: Vec2; // direction of guide A

  if (idx > 0) {
    // There's a prev segment: guide from prevStart in direction of prev segment
    fixedA = pts[idx - 1];
    guideDirA = { x: dragA.x - fixedA.x, y: dragA.y - fixedA.y };
  } else {
    // First segment: pad is locked. Guide perpendicular to drag.
    fixedA = { ...dragA };
    guideDirA = { x: -ddy, y: ddx }; // perpendicular
  }

  let fixedB: Vec2;
  let guideDirB: Vec2;

  if (idx + 2 < pts.length) {
    // There's a next segment: guide from nextEnd in direction of next segment
    fixedB = pts[idx + 2];
    guideDirB = { x: dragB.x - fixedB.x, y: dragB.y - fixedB.y };
  } else {
    // Last segment: pad is locked. Guide perpendicular to drag.
    fixedB = { ...dragB };
    guideDirB = { x: -ddy, y: ddx }; // perpendicular
  }

  // Intersect s_current (line through newPos in dragDir) with guides
  const ip1 = lineIsect(newPos, dragDirV, fixedA, guideDirA);
  const ip2 = lineIsect(newPos, dragDirV, fixedB, guideDirB);

  if (!ip1 || !ip2) return null;

  // Build result: replace dragged segment section with fixedA → ip1 → ip2 → fixedB
  const before = pts.slice(0, Math.max(0, idx - 1));
  const after = pts.slice(Math.min(pts.length, idx + 3));

  // Include fixedA only if it's not already in 'before'
  const newPts: Vec2[] = [];
  if (idx > 1) {
    newPts.push(...before);
  }
  newPts.push(fixedA, ip1, ip2, fixedB);
  if (after.length > 0) {
    newPts.push(...after);
  }

  // Handle edge cases for first/last segments
  if (idx === 0) {
    // First segment dragged: result is dragA(locked) → ip2 → fixedB → ...rest
    // ip1 is on the perpendicular from dragA, don't need it
    const r = [pts[0], ip2, ...pts.slice(idx + 2)];
    return verticesToSegments(r);
  }

  if (idx === segments.length - 1) {
    // Last segment: result is ...rest → fixedA → ip1 → dragB(locked)
    const r = [...pts.slice(0, idx), ip1, pts[pts.length - 1]];
    return verticesToSegments(r);
  }

  // Middle segment: fixedA → ip1 → ip2 → fixedB
  const r = [...pts.slice(0, idx - 1), fixedA, ip1, ip2, fixedB, ...pts.slice(idx + 3)];
  return verticesToSegments(r);
}

// ---------------------------------------------------------------------------
// dragCorner
// ---------------------------------------------------------------------------

function dragCornerInternal(pts: Vec2[], target: Vec2): TraceSegmentInfo[] | null {
  if (pts.length === 0) return null;
  if (pts.length === 1) return verticesToSegments(buildInitialTrace(pts[0], target, Dir45.UNDEFINED));
  if (pts.length === 2) {
    const dir = dirFromSeg(pts[1], pts[0]);
    return verticesToSegments(buildInitialTrace(pts[0], target, Dir45.UNDEFINED, isDiagonal(dir)));
  }

  // Walk backward, try to splice a BuildInitialTrace from each vertex
  for (let i = pts.length - 2; i >= 0; i--) {
    const segDir = dirFromSeg(pts[i], pts[i + 1]);
    const startPt = pts[i];

    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;
      const firstDir = dirFromSeg(path[0], path[1]);
      if (firstDir === segDir) {
        return [...verticesToSegments(pts.slice(0, i + 1)), ...verticesToSegments(path)];
      }
    }

    const prevDir = i > 0 ? dirFromSeg(pts[i - 1], pts[i]) : Dir45.UNDEFINED;
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;
      const firstDir = dirFromSeg(path[0], path[1]);
      if (prevDir !== Dir45.UNDEFINED) {
        const ang = angleBetween(firstDir, prevDir);
        if (ang === AngleType.ANG_OBTUSE || ang === AngleType.ANG_STRAIGHT) {
          return [...verticesToSegments(pts.slice(0, i + 1)), ...verticesToSegments(path)];
        }
      }
    }
  }

  const lastDir = dirFromSeg(pts[pts.length - 2], pts[pts.length - 1]);
  return verticesToSegments(buildInitialTrace(pts[0], target, Dir45.UNDEFINED, isDiagonal(lastDir)));
}

export function dragCorner(
  segments: TraceSegmentInfo[],
  cornerIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (segments.length === 0) return null;
  const pts = traceVertices({ segments } as TraceInfo);
  if (cornerIndex < 0 || cornerIndex >= pts.length) return null;

  if (cornerIndex === 0) return dragCornerInternal(pts, newPos);

  if (cornerIndex === pts.length - 1) {
    const rev = [...pts].reverse();
    const result = dragCornerInternal(rev, newPos);
    if (!result) return null;
    return result.map(s => ({ start_x: s.end_x, start_y: s.end_y, end_x: s.start_x, end_y: s.start_y })).reverse();
  }

  // Middle: split and reroute both halves
  const leftResult = dragCornerInternal(pts.slice(0, cornerIndex + 1), newPos);
  const rightRev = [...pts.slice(cornerIndex)].reverse();
  const rightResult = dragCornerInternal(rightRev, newPos);

  if (!leftResult || !rightResult) {
    const newPts = [...pts];
    newPts[cornerIndex] = { x: Math.round(newPos.x), y: Math.round(newPos.y) };
    return verticesToSegments(newPts);
  }

  return [...leftResult, ...rightResult.map(s => ({ start_x: s.end_x, start_y: s.end_y, end_x: s.start_x, end_y: s.start_y })).reverse()];
}

// ---------------------------------------------------------------------------
// Rectangle selection
// ---------------------------------------------------------------------------

export function tracesInRect(snapshot: BoardSnapshot | null, x1: number, y1: number, x2: number, y2: number): number[] {
  if (!snapshot?.traces) return [];
  const minX = Math.min(x1, x2), maxX = Math.max(x1, x2), minY = Math.min(y1, y2), maxY = Math.max(y1, y2);
  return snapshot.traces.filter(t => t.segments.length > 0 && t.segments.every(s => {
    const sx = Number(s.start_x), sy = Number(s.start_y), ex = Number(s.end_x), ey = Number(s.end_y);
    return sx >= minX && sx <= maxX && sy >= minY && sy <= maxY && ex >= minX && ex <= maxX && ey >= minY && ey <= maxY;
  })).map(t => t.id);
}

export function componentsInRect(snapshot: BoardSnapshot | null, x1: number, y1: number, x2: number, y2: number): string[] {
  if (!snapshot?.components) return [];
  const minX = Math.min(x1, x2), maxX = Math.max(x1, x2), minY = Math.min(y1, y2), maxY = Math.max(y1, y2);
  return snapshot.components.filter(c => {
    const cx = Number(c.x_nm), cy = Number(c.y_nm);
    return cx >= minX && cx <= maxX && cy >= minY && cy <= maxY;
  }).map(c => c.refdes);
}
