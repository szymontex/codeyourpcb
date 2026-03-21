/**
 * Trace segment/corner editing — KiCad-style drag operations.
 * Faithful port of KiCad's LINE::dragSegment45 and dragCornerInternal.
 */

import type { BoardSnapshot, TraceInfo, TraceSegmentInfo } from './types';
import { pointToSegmentDistance } from './geometry';
import {
  type Vec2, Dir45,
  dirFromSeg, isDiagonal, buildInitialTrace,
  leftDir, rightDir, angleBetween, oppositeDir,
  AngleType,
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
  worldX: number,
  worldY: number,
  toleranceNm: number,
): TraceSegmentHit | null {
  if (!snapshot?.traces || snapshot.traces.length === 0) return null;

  let bestDist = Infinity;
  let bestHit: TraceSegmentHit | null = null;

  for (const trace of snapshot.traces) {
    const hitRadius = toleranceNm + Number(trace.width) / 2;
    const cornerRadius = Math.max(Number(trace.width), toleranceNm);

    // Corners first (priority)
    const verts = traceVertices(trace);
    for (let vi = 0; vi < verts.length; vi++) {
      const dist = Math.hypot(worldX - verts[vi].x, worldY - verts[vi].y);
      if (dist <= cornerRadius && dist < bestDist) {
        bestDist = dist;
        bestHit = {
          traceId: trace.id, trace,
          segmentIndex: Math.min(vi, trace.segments.length - 1),
          nearCorner: true, cornerIndex: vi,
        };
      }
    }

    // Segment bodies
    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const dist = pointToSegmentDistance(
        worldX, worldY,
        Number(seg.start_x), Number(seg.start_y),
        Number(seg.end_x), Number(seg.end_y),
      );
      if (dist <= hitRadius && dist < bestDist) {
        const startDist = Math.hypot(worldX - Number(seg.start_x), worldY - Number(seg.start_y));
        const endDist = Math.hypot(worldX - Number(seg.end_x), worldY - Number(seg.end_y));
        if (Math.min(startDist, endDist) <= cornerRadius) continue;
        bestDist = dist;
        bestHit = {
          traceId: trace.id, trace,
          segmentIndex: i, nearCorner: false,
        };
      }
    }
  }

  return bestHit;
}

export function traceVertices(trace: TraceInfo): Vec2[] {
  if (trace.segments.length === 0) return [];
  const pts: Vec2[] = [{ x: Number(trace.segments[0].start_x), y: Number(trace.segments[0].start_y) }];
  for (const seg of trace.segments) {
    pts.push({ x: Number(seg.end_x), y: Number(seg.end_y) });
  }
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
// Direction vector
// ---------------------------------------------------------------------------

function dirVec(dir: Dir45): Vec2 {
  const vecs: Vec2[] = [
    {x:0,y:1},{x:1,y:1},{x:1,y:0},{x:1,y:-1},
    {x:0,y:-1},{x:-1,y:-1},{x:-1,y:0},{x:-1,y:1},
  ];
  if (dir === Dir45.UNDEFINED || dir < 0 || dir > 7) return {x:0,y:0};
  return vecs[dir];
}

/** Intersect two infinite lines: p1+t*d1 = p2+u*d2. Returns point or null. */
function lineIsect(p1: Vec2, d1: Vec2, p2: Vec2, d2: Vec2): Vec2 | null {
  const cross = d1.x * d2.y - d1.y * d2.x;
  if (Math.abs(cross) < 0.001) return null;
  const t = ((p2.x - p1.x) * d2.y - (p2.y - p1.y) * d2.x) / cross;
  return { x: p1.x + t * d1.x, y: p1.y + t * d1.y };
}

// ---------------------------------------------------------------------------
// dragSegment — KiCad LINE::dragSegment45 faithful port
// ---------------------------------------------------------------------------

/**
 * Drag a trace segment parallel to itself.
 *
 * KiCad algorithm:
 * 1. Identify prev segment, dragged segment, next segment
 * 2. Insert zero-length guard segments at boundaries if needed
 * 3. Build guide lines from prev.A and next.B in their directions
 * 4. Intersect a line through newPos (in drag direction) with the guides
 * 5. Try all 4 guide combinations, pick shortest valid result
 */
export function dragSegment(
  segments: TraceSegmentInfo[],
  segIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (segments.length === 0 || segIndex < 0 || segIndex >= segments.length) return null;

  let pts = traceVertices({ segments } as TraceInfo);
  if (pts.length < 2) return null;

  let idx = segIndex;

  // Guard: ensure prev and next exist (KiCad inserts zero-length segments)
  if (idx === 0) {
    pts = [{ ...pts[0] }, ...pts];
    idx++;
  }
  if (idx >= pts.length - 2) {
    pts = [...pts, { ...pts[pts.length - 1] }];
  }

  // pts[idx-1] → pts[idx] is s_prev
  // pts[idx] → pts[idx+1] is dragged
  // pts[idx+1] → pts[idx+2] is s_next

  const dragDir = dirFromSeg(pts[idx], pts[idx + 1]);
  if (dragDir === Dir45.UNDEFINED) return null;
  const dragV = dirVec(dragDir);

  const prevA = pts[idx - 1];
  const nextB = pts[idx + 2];

  let dirPrev = dirFromSeg(prevA, pts[idx]);
  let dirNext = dirFromSeg(pts[idx + 1], nextB);

  // KiCad: if prev/next direction matches drag direction, force perpendicular
  if (dirPrev === dragDir) { dirPrev = leftDir(dragDir); }
  else if (dirPrev === Dir45.UNDEFINED) { dirPrev = leftDir(dragDir); }

  if (dirNext === dragDir) { dirNext = rightDir(dragDir); }
  else if (dirNext === Dir45.UNDEFINED) { dirNext = rightDir(dragDir); }

  // KiCad: if angle is obtuse/half-full, use both left/right as candidates
  const prevAngle = angleBetween(dirPrev, dragDir);
  const nextAngle = angleBetween(dirNext, dragDir);

  const guidesA: Dir45[] = (prevAngle === AngleType.ANG_OBTUSE || prevAngle === AngleType.ANG_HALF_FULL)
    ? [leftDir(dragDir), rightDir(dragDir)]
    : [dirPrev];

  const guidesB: Dir45[] = (nextAngle === AngleType.ANG_OBTUSE || nextAngle === AngleType.ANG_HALF_FULL)
    ? [leftDir(dragDir), rightDir(dragDir)]
    : [dirNext];

  // Line through newPos in drag direction
  const sCurrentP = newPos;
  const sCurrentD = dragV;

  let bestLen = Infinity;
  let bestResult: Vec2[] | null = null;

  for (const gA of guidesA) {
    for (const gB of guidesB) {
      const ip1 = lineIsect(sCurrentP, sCurrentD, prevA, dirVec(gA));
      const ip2 = lineIsect(sCurrentP, sCurrentD, nextB, dirVec(gB));
      if (!ip1 || !ip2) continue;

      // Build candidate paths and pick shortest
      const candidates: Vec2[][] = [];

      // Full 3-segment: prevA → ip1 → ip2 → nextB
      candidates.push([prevA, ip1, ip2, nextB]);

      // Try s1 intersects with s_next line
      const ipSn = lineIsect(prevA, dirVec(gA), nextB, dirVec(gB));
      if (ipSn) {
        candidates.push([prevA, ipSn, nextB]);
      }

      for (const cand of candidates) {
        // Validate: all segments must be H/V/45°
        let valid = true;
        let len = 0;
        for (let k = 0; k < cand.length - 1; k++) {
          const d = dirFromSeg(cand[k], cand[k + 1]);
          const segLen = Math.hypot(cand[k+1].x - cand[k].x, cand[k+1].y - cand[k].y);
          if (segLen > 100 && d === Dir45.UNDEFINED) { valid = false; break; }
          len += segLen;
        }
        if (!valid) continue;

        // Must actually pass through (or near) newPos
        // Check distance from newPos to nearest segment of candidate
        let minDist = Infinity;
        for (let k = 0; k < cand.length - 1; k++) {
          const d = pointToSegmentDistance(newPos.x, newPos.y, cand[k].x, cand[k].y, cand[k+1].x, cand[k+1].y);
          minDist = Math.min(minDist, d);
        }
        // Allow some tolerance (half trace width worth)
        if (minDist > 500_000) continue; // way too far

        if (len < bestLen) {
          bestLen = len;
          bestResult = cand;
        }
      }
    }
  }

  if (!bestResult) {
    // Fallback: perpendicular offset
    const perpV = { x: -dragV.y, y: dragV.x };
    const offset = (newPos.x - pts[idx].x) * perpV.x + (newPos.y - pts[idx].y) * perpV.y;
    const result = [...pts];
    result[idx] = { x: pts[idx].x + offset * perpV.x, y: pts[idx].y + offset * perpV.y };
    result[idx + 1] = { x: pts[idx+1].x + offset * perpV.x, y: pts[idx+1].y + offset * perpV.y };
    return verticesToSegments(result);
  }

  // Replace prev+dragged+next with the new path
  const result = [...pts.slice(0, idx - 1), ...bestResult, ...pts.slice(idx + 3)];
  return verticesToSegments(result);
}

// ---------------------------------------------------------------------------
// dragCorner — KiCad dragCornerInternal faithful port
// ---------------------------------------------------------------------------

/**
 * KiCad's dragCornerInternal: given a path from origin to some endpoint,
 * reroute the END to a new target position while keeping the beginning intact.
 * Walks backward through segments to find the best splice point.
 */
function dragCornerInternal(pts: Vec2[], target: Vec2): TraceSegmentInfo[] | null {
  if (pts.length === 0) return null;
  if (pts.length === 1) return verticesToSegments(buildInitialTrace(pts[0], target, Dir45.UNDEFINED));
  if (pts.length === 2) {
    const dir = dirFromSeg(pts[1], pts[0]);
    return verticesToSegments(buildInitialTrace(pts[0], target, Dir45.UNDEFINED, isDiagonal(dir)));
  }

  // Walk backward from end, try to splice a new BuildInitialTrace from each vertex
  for (let i = pts.length - 2; i >= 0; i--) {
    const segDir = dirFromSeg(pts[i], pts[i + 1]);
    const startPt = pts[i];

    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;

      const firstDir = dirFromSeg(path[0], path[1]);

      // Pick posture that continues existing direction
      if (firstDir === segDir) {
        const kept = verticesToSegments(pts.slice(0, i + 1));
        const added = verticesToSegments(path);
        return [...kept, ...added];
      }
    }

    // Try any obtuse continuation
    const prevDir = i > 0 ? dirFromSeg(pts[i - 1], pts[i]) : Dir45.UNDEFINED;
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(startPt, target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;
      const firstDir = dirFromSeg(path[0], path[1]);
      if (prevDir !== Dir45.UNDEFINED) {
        const angle = angleBetween(firstDir, prevDir);
        if (angle === AngleType.ANG_OBTUSE || angle === AngleType.ANG_STRAIGHT) {
          const kept = verticesToSegments(pts.slice(0, i + 1));
          const added = verticesToSegments(path);
          return [...kept, ...added];
        }
      }
    }
  }

  // Fallback
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

  if (cornerIndex === 0) {
    return dragCornerInternal(pts, newPos);
  }

  if (cornerIndex === pts.length - 1) {
    const rev = [...pts].reverse();
    const result = dragCornerInternal(rev, newPos);
    if (!result) return null;
    return result.map(s => ({
      start_x: s.end_x, start_y: s.end_y,
      end_x: s.start_x, end_y: s.start_y,
    })).reverse();
  }

  // Middle: split, reroute both halves
  const leftPts = pts.slice(0, cornerIndex + 1);
  const rightPts = pts.slice(cornerIndex);

  const leftResult = dragCornerInternal(leftPts, newPos);
  const rightRev = [...rightPts].reverse();
  const rightResult = dragCornerInternal(rightRev, newPos);

  if (!leftResult || !rightResult) {
    // Fallback: just move the vertex
    const newPts = [...pts];
    newPts[cornerIndex] = { x: Math.round(newPos.x), y: Math.round(newPos.y) };
    return verticesToSegments(newPts);
  }

  return [...leftResult, ...rightResult.map(s => ({
    start_x: s.end_x, start_y: s.end_y,
    end_x: s.start_x, end_y: s.start_y,
  })).reverse()];
}

// ---------------------------------------------------------------------------
// Rectangle selection
// ---------------------------------------------------------------------------

export function tracesInRect(
  snapshot: BoardSnapshot | null,
  x1: number, y1: number, x2: number, y2: number,
): number[] {
  if (!snapshot?.traces) return [];
  const minX = Math.min(x1, x2), maxX = Math.max(x1, x2);
  const minY = Math.min(y1, y2), maxY = Math.max(y1, y2);
  const result: number[] = [];
  for (const trace of snapshot.traces) {
    let allInside = true;
    for (const seg of trace.segments) {
      const sx = Number(seg.start_x), sy = Number(seg.start_y);
      const ex = Number(seg.end_x), ey = Number(seg.end_y);
      if (sx < minX || sx > maxX || sy < minY || sy > maxY || ex < minX || ex > maxX || ey < minY || ey > maxY) {
        allInside = false; break;
      }
    }
    if (allInside && trace.segments.length > 0) result.push(trace.id);
  }
  return result;
}

export function componentsInRect(
  snapshot: BoardSnapshot | null,
  x1: number, y1: number, x2: number, y2: number,
): string[] {
  if (!snapshot?.components) return [];
  const minX = Math.min(x1, x2), maxX = Math.max(x1, x2);
  const minY = Math.min(y1, y2), maxY = Math.max(y1, y2);
  const result: string[] = [];
  for (const comp of snapshot.components) {
    const cx = Number(comp.x_nm), cy = Number(comp.y_nm);
    if (cx >= minX && cx <= maxX && cy >= minY && cy <= maxY) result.push(comp.refdes);
  }
  return result;
}
