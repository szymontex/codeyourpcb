/**
 * Trace segment/corner editing — KiCad-style drag operations.
 *
 * Port of KiCad's LINE::dragSegment45 and LINE::dragCorner45 logic.
 * Allows dragging individual trace segments (parallel move) or
 * corners (vertex move) while maintaining 45° routing constraints.
 *
 * Reference: kicad/pcbnew/router/pns_line.cpp
 */

import type { BoardSnapshot, TraceInfo, TraceSegmentInfo } from './types';
import { pointToSegmentDistance } from './geometry';
import {
  type Vec2, Dir45,
  dirFromSeg, isDiagonal, buildInitialTrace,
  leftDir, rightDir, angleBetween,
  AngleType,
} from './direction45';

// ---------------------------------------------------------------------------
// Hit-test result for segment-level selection
// ---------------------------------------------------------------------------

export interface TraceSegmentHit {
  /** Trace entity ID */
  traceId: number;
  /** The full trace info */
  trace: TraceInfo;
  /** Index of the segment that was hit */
  segmentIndex: number;
  /** Whether the click is near a corner (vertex) rather than the segment body */
  nearCorner: boolean;
  /** If nearCorner is true, which corner (vertex) index (0 = first point of trace) */
  cornerIndex?: number;
}

// ---------------------------------------------------------------------------
// Segment-level hit testing
// ---------------------------------------------------------------------------

/**
 * Hit-test for a specific trace segment or corner.
 *
 * Returns which segment index within which trace was clicked, and whether
 * the click is near a corner (vertex) or the segment body.
 *
 * "Near corner" is defined as within `cornerRadius` of a segment endpoint.
 * The corner radius is max(half the trace width, toleranceNm).
 */
export function hitTestTraceSegment(
  snapshot: BoardSnapshot | null,
  worldX: number,
  worldY: number,
  toleranceNm: number,
): TraceSegmentHit | null {
  if (!snapshot?.traces || snapshot.traces.length === 0) return null;

  let bestDist = Infinity;
  let bestHit: TraceSegmentHit | null = null;

  for (const trace of snapshot.traces) {
    const hitRadius = toleranceNm + trace.width / 2;

    // First: check corners (vertices). Corners get priority over segments.
    const vertices = traceVertices(trace);
    const cornerRadius = Math.max(trace.width, toleranceNm);

    for (let vi = 0; vi < vertices.length; vi++) {
      const v = vertices[vi];
      const dx = worldX - v.x;
      const dy = worldY - v.y;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist <= cornerRadius && dist < bestDist) {
        bestDist = dist;
        // Find which segment this corner belongs to (for segmentIndex)
        const segIdx = Math.min(vi, trace.segments.length - 1);
        bestHit = {
          traceId: trace.id,
          trace,
          segmentIndex: segIdx,
          nearCorner: true,
          cornerIndex: vi,
        };
      }
    }

    // Then: check segment bodies
    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const dist = pointToSegmentDistance(
        worldX, worldY,
        seg.start_x, seg.start_y,
        seg.end_x, seg.end_y,
      );
      if (dist <= hitRadius && dist < bestDist) {
        // Check if this is actually near a corner
        const startDist = Math.hypot(worldX - seg.start_x, worldY - seg.start_y);
        const endDist = Math.hypot(worldX - seg.end_x, worldY - seg.end_y);
        const minEndpointDist = Math.min(startDist, endDist);

        if (minEndpointDist <= cornerRadius) {
          // Near a corner — handled above with higher precision
          continue;
        }

        bestDist = dist;
        bestHit = {
          traceId: trace.id,
          trace,
          segmentIndex: i,
          nearCorner: false,
        };
      }
    }
  }

  return bestHit;
}

/**
 * Extract ordered vertices from a trace's segments.
 * Consecutive segments share endpoints, so we deduplicate.
 */
export function traceVertices(trace: TraceInfo): Vec2[] {
  if (trace.segments.length === 0) return [];

  const pts: Vec2[] = [{ x: trace.segments[0].start_x, y: trace.segments[0].start_y }];
  for (const seg of trace.segments) {
    pts.push({ x: seg.end_x, y: seg.end_y });
  }
  return pts;
}

/**
 * Rebuild segments from an ordered list of vertices.
 */
function verticesToSegments(pts: Vec2[]): TraceSegmentInfo[] {
  const segs: TraceSegmentInfo[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i];
    const b = pts[i + 1];
    // Skip zero-length segments
    if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
    segs.push({
      start_x: Math.round(a.x),
      start_y: Math.round(a.y),
      end_x: Math.round(b.x),
      end_y: Math.round(b.y),
    });
  }
  return segs;
}

// ---------------------------------------------------------------------------
// Direction helpers (for segment drag)
// ---------------------------------------------------------------------------

function dirToVector(dir: Dir45): Vec2 {
  // Map direction enum to unit vector (in 45° increments).
  // N=0, NE=1, E=2, SE=3, S=4, SW=5, W=6, NW=7
  // Note: in our coordinate system Y increases upward (world coords).
  const vectors: Vec2[] = [
    { x: 0, y: 1 },   // N
    { x: 1, y: 1 },   // NE
    { x: 1, y: 0 },   // E
    { x: 1, y: -1 },  // SE
    { x: 0, y: -1 },  // S
    { x: -1, y: -1 }, // SW
    { x: -1, y: 0 },  // W
    { x: -1, y: 1 },  // NW
  ];
  if (dir === Dir45.UNDEFINED || dir < 0 || dir > 7) return { x: 0, y: 0 };
  return vectors[dir];
}

/**
 * Intersect two infinite lines, each defined by a point and direction vector.
 * Returns the intersection point, or null if lines are parallel.
 */
function lineIntersect(
  p1: Vec2, d1: Vec2,
  p2: Vec2, d2: Vec2,
): Vec2 | null {
  const cross = d1.x * d2.y - d1.y * d2.x;
  if (Math.abs(cross) < 1e-9) return null; // Parallel

  const dx = p2.x - p1.x;
  const dy = p2.y - p1.y;
  const t = (dx * d2.y - dy * d2.x) / cross;

  return {
    x: p1.x + t * d1.x,
    y: p1.y + t * d1.y,
  };
}


/**
 * Drag a trace segment parallel to its original direction.
 * Adjacent segments adjust to maintain 45° constraint.
 *
 * Port of KiCad's LINE::dragSegment45.
 *
 * @param segments  Current trace segments
 * @param segIndex  Index of the segment being dragged
 * @param newPos    New world position the segment should pass through
 * @returns New segments array, or null if drag is invalid
 */
export function dragSegment(
  segments: TraceSegmentInfo[],
  segIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (segments.length === 0 || segIndex < 0 || segIndex >= segments.length) return null;

  // Convert segments to vertices
  let pts = traceVertices({ segments } as TraceInfo);
  if (pts.length < 2) return null;

  // The dragged segment goes from pts[segIndex] to pts[segIndex+1]
  let idx = segIndex;

  // Ensure we have prev and next segments to work with.
  // If at the start, insert a duplicate point to create a zero-length prev segment.
  if (idx === 0) {
    pts = [{ ...pts[0] }, ...pts];
    idx++;
  }

  // If at the end, insert a duplicate point for zero-length next segment.
  if (idx === pts.length - 2) {
    pts = [...pts, { ...pts[pts.length - 1] }];
  }

  // Now we have: pts[idx-1] → pts[idx] → pts[idx+1] → pts[idx+2]
  //              s_prev         dragged           s_next
  const dragA = pts[idx];
  const dragB = pts[idx + 1];
  const dragDir = dirFromSeg(dragA, dragB);
  const dragVec = dirToVector(dragDir);

  if (dragDir === Dir45.UNDEFINED) return null;

  const prevA = pts[idx - 1];
  const nextB = pts[idx + 2];

  let dirPrev = dirFromSeg(prevA, dragA);
  let dirNext = dirFromSeg(dragB, nextB);

  // If prev direction equals drag direction, insert a new point and use perpendicular
  if (dirPrev === dragDir || dirPrev === Dir45.UNDEFINED) {
    dirPrev = leftDir(dragDir);
  }
  if (dirNext === dragDir || dirNext === Dir45.UNDEFINED) {
    dirNext = rightDir(dragDir);
  }


  // The dragged segment moves parallel: project the target point onto a line
  // parallel to the drag direction passing through newPos.
  // Then find intersections with the guide lines from prev.A and next.B.

  // Guide lines:
  // guideA: from prevA in direction of dirPrev (keeping prevA fixed)
  // guideB: from nextB in direction of dirNext (keeping nextB fixed)
  // current: line through newPos in direction of dragDir

  // Try both orientations for prev and next guides
  const guideDirsA = [dirPrev, isDiagonal(dirPrev) ? rightDir(dirPrev) : leftDir(dirPrev)];
  const guideDirsB = [dirNext, isDiagonal(dirNext) ? leftDir(dirNext) : rightDir(dirNext)];

  let bestLen = Infinity;
  let bestPts: Vec2[] | null = null;

  for (const gdA of guideDirsA) {
    for (const gdB of guideDirsB) {
      const gvA = dirToVector(gdA);
      const gvB = dirToVector(gdB);

      // Intersect current (line through newPos along dragDir) with guideA (from prevA along gdA)
      const ip1 = lineIntersect(newPos, dragVec, prevA, gvA);
      // Intersect current with guideB (from nextB along gdB)
      const ip2 = lineIntersect(newPos, dragVec, nextB, gvB);

      if (!ip1 || !ip2) continue;

      // Build candidate: prevA → ip1 → ip2 → nextB
      const candidate = [prevA, ip1, ip2, nextB];

      // Validate: all consecutive segments must be H/V/45°
      let valid = true;
      let totalLen = 0;
      for (let k = 0; k < candidate.length - 1; k++) {
        const d = dirFromSeg(candidate[k], candidate[k + 1]);
        if (d === Dir45.UNDEFINED) {
          // Check if it's a zero-length segment (ok)
          const dx = Math.abs(candidate[k].x - candidate[k + 1].x);
          const dy = Math.abs(candidate[k].y - candidate[k + 1].y);
          if (dx > 100 || dy > 100) { // tolerance for rounding
            valid = false;
            break;
          }
        }
        totalLen += Math.hypot(
          candidate[k + 1].x - candidate[k].x,
          candidate[k + 1].y - candidate[k].y,
        );
      }

      if (valid && totalLen < bestLen) {
        bestLen = totalLen;
        bestPts = candidate;
      }
    }
  }

  if (!bestPts) {
    // Fallback: simpler approach — just move the two vertices of the dragged segment
    // perpendicular to the drag direction by the offset amount
    const perpVec = { x: -dragVec.y, y: dragVec.x };
    // Project offset
    const offset = (newPos.x - dragA.x) * perpVec.x + (newPos.y - dragA.y) * perpVec.y;
    const newA = { x: dragA.x + offset * perpVec.x, y: dragA.y + offset * perpVec.y };
    const newB = { x: dragB.x + offset * perpVec.x, y: dragB.y + offset * perpVec.y };

    const result = [...pts];
    result[idx] = newA;
    result[idx + 1] = newB;
    return verticesToSegments(result);
  }

  // Replace the three segments (prev, dragged, next) with the new path
  const result = [
    ...pts.slice(0, idx - 1),
    ...bestPts,
    ...pts.slice(idx + 3),
  ];

  return verticesToSegments(result);
}

// ---------------------------------------------------------------------------
// dragCorner — KiCad's LINE::dragCorner45 port
// ---------------------------------------------------------------------------

/**
 * Drag a trace corner (vertex) to a new position.
 * Adjacent segments re-route with 45° constraint using BuildInitialTrace.
 *
 * Port of KiCad's LINE::dragCorner45.
 *
 * @param segments     Current trace segments
 * @param cornerIndex  Index of the vertex being dragged (0 = first point)
 * @param newPos       New world position for the vertex
 * @returns New segments array, or null if drag is invalid
 */
export function dragCorner(
  segments: TraceSegmentInfo[],
  cornerIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (segments.length === 0) return null;

  const pts = traceVertices({ segments } as TraceInfo);
  if (cornerIndex < 0 || cornerIndex >= pts.length) return null;


  if (cornerIndex === 0) {
    // Dragging the first point: re-route from newPos to the rest of the trace
    const pathForward = dragCornerInternal(pts, newPos);
    return pathForward;
  }

  if (cornerIndex === pts.length - 1) {
    // Dragging the last point: re-route from trace start to newPos
    // Reverse the points, do dragCornerInternal, reverse result
    const reversed = [...pts].reverse();
    const pathReversed = dragCornerInternal(reversed, newPos);
    if (!pathReversed) return null;
    // Reverse the resulting segments
    const reversedSegs = pathReversed.map(s => ({
      start_x: s.end_x,
      start_y: s.end_y,
      end_x: s.start_x,
      end_y: s.start_y,
    })).reverse();
    return reversedSegs;
  }

  // Middle vertex: split into two halves, reroute each half toward the new position
  const leftPts = pts.slice(0, cornerIndex + 1);
  const rightPts = pts.slice(cornerIndex);

  // Left half: re-route from start to newPos
  const leftResult = dragCornerInternal(leftPts, newPos);

  // Right half: reverse, re-route from end to newPos, reverse back
  const rightReversed = [...rightPts].reverse();
  const rightResult = dragCornerInternal(rightReversed, newPos);

  if (!leftResult || !rightResult) {
    // Fallback: just move the vertex
    const newPts = [...pts];
    newPts[cornerIndex] = { x: Math.round(newPos.x), y: Math.round(newPos.y) };
    return verticesToSegments(newPts);
  }

  // Reverse the right result back
  const rightSegs = rightResult.map(s => ({
    start_x: s.end_x,
    start_y: s.end_y,
    end_x: s.start_x,
    end_y: s.start_y,
  })).reverse();

  // Combine: left segments + right segments
  return [...leftResult, ...rightSegs];
}

/**
 * Internal helper for dragCorner45: re-route from the start of a path to a new endpoint.
 * Uses BuildInitialTrace to maintain 45° constraints.
 *
 * Port of KiCad's dragCornerInternal.
 */
function dragCornerInternal(
  pts: Vec2[],
  target: Vec2,
): TraceSegmentInfo[] | null {
  if (pts.length === 0) return null;

  if (pts.length === 1) {
    // Single point: build initial trace from that point to target
    const path = buildInitialTrace(pts[0], target, Dir45.UNDEFINED);
    return verticesToSegments(path);
  }

  if (pts.length === 2) {
    // Single segment: use its direction to pick posture
    const dir = dirFromSeg(pts[1], pts[0]); // reversed direction from KiCad
    const startDiag = isDiagonal(dir);
    const path = buildInitialTrace(pts[0], target, Dir45.UNDEFINED, startDiag);
    return verticesToSegments(path);
  }

  // Multi-segment: try to find a good splice point.
  // Walk backward from the end, trying to reroute from each vertex to the target.
  const numSegs = pts.length - 1;

  for (let i = numSegs - 1; i >= 0; i--) {
    const segDir = dirFromSeg(pts[i], pts[i + 1]);
    const startPt = pts[i];

    // Try both postures (straight-first and diagonal-first)
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;

      const firstDir = dirFromSeg(path[0], path[1]);

      // Prefer the posture that continues the existing direction
      if (firstDir === segDir) {
        // Splice: keep pts[0..i], append the new path
        const keptPts = pts.slice(0, i + 1);
        const keptSegs = verticesToSegments(keptPts);
        const newSegs = verticesToSegments(path);
        return [...keptSegs, ...newSegs];
      }
    }

    // If neither posture matched the direction, try with any valid 45° path
    const prevDir = i > 0 ? dirFromSeg(pts[i - 1], pts[i]) : Dir45.UNDEFINED;
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;

      const firstDir = dirFromSeg(path[0], path[1]);
      if (prevDir !== Dir45.UNDEFINED) {
        const angle = angleBetween(firstDir, prevDir);
        if (angle === AngleType.ANG_OBTUSE || angle === AngleType.ANG_STRAIGHT) {
          const keptPts = pts.slice(0, i + 1);
          const keptSegs = verticesToSegments(keptPts);
          const newSegs = verticesToSegments(path);
          return [...keptSegs, ...newSegs];
        }
      }
    }
  }

  // Fallback: route from the first point to the target
  const lastSeg = pts.length >= 2 ? dirFromSeg(pts[pts.length - 2], pts[pts.length - 1]) : Dir45.UNDEFINED;
  const startDiag = isDiagonal(lastSeg);
  const path = buildInitialTrace(pts[0], target, Dir45.UNDEFINED, startDiag);
  return verticesToSegments(path);
}

// ---------------------------------------------------------------------------
// Rectangle selection helpers
// ---------------------------------------------------------------------------

/**
 * Find all trace IDs whose segments are fully contained within a rectangle.
 */
export function tracesInRect(
  snapshot: BoardSnapshot | null,
  x1: number, y1: number,
  x2: number, y2: number,
): number[] {
  if (!snapshot?.traces) return [];

  const minX = Math.min(x1, x2);
  const maxX = Math.max(x1, x2);
  const minY = Math.min(y1, y2);
  const maxY = Math.max(y1, y2);

  const result: number[] = [];

  for (const trace of snapshot.traces) {
    let allInside = true;
    for (const seg of trace.segments) {
      if (
        seg.start_x < minX || seg.start_x > maxX ||
        seg.start_y < minY || seg.start_y > maxY ||
        seg.end_x < minX || seg.end_x > maxX ||
        seg.end_y < minY || seg.end_y > maxY
      ) {
        allInside = false;
        break;
      }
    }
    if (allInside && trace.segments.length > 0) {
      result.push(trace.id);
    }
  }

  return result;
}

/**
 * Find all component refdes whose center is within a rectangle.
 */
export function componentsInRect(
  snapshot: BoardSnapshot | null,
  x1: number, y1: number,
  x2: number, y2: number,
): string[] {
  if (!snapshot?.components) return [];

  const minX = Math.min(x1, x2);
  const maxX = Math.max(x1, x2);
  const minY = Math.min(y1, y2);
  const maxY = Math.max(y1, y2);

  const result: string[] = [];

  for (const comp of snapshot.components) {
    if (
      comp.x_nm >= minX && comp.x_nm <= maxX &&
      comp.y_nm >= minY && comp.y_nm <= maxY
    ) {
      result.push(comp.refdes);
    }
  }

  return result;
}
