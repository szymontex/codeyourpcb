/**
 * Trace segment/corner editing — faithful port of KiCad's pns_line.cpp
 */

import type { BoardSnapshot, TraceInfo, TraceSegmentInfo } from './types';
import { pointToSegmentDistance } from './geometry';
import {
  type Vec2, Dir45,
  dirFromSeg, isDiagonal, buildInitialTrace,
  leftDir, rightDir, angleBetween, AngleType,
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
  snapshot: BoardSnapshot | null, worldX: number, worldY: number, toleranceNm: number,
): TraceSegmentHit | null {
  if (!snapshot?.traces) return null;
  let bestDist = Infinity;
  let bestHit: TraceSegmentHit | null = null;
  for (const trace of snapshot.traces) {
    const hw = Number(trace.width) / 2;
    const verts = traceVertices(trace);
    const cornerR = Math.max(Number(trace.width), toleranceNm);
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
      if (d <= toleranceNm + hw && d < bestDist) {
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
  if (!trace.segments.length) return [];
  const pts: Vec2[] = [{ x: Number(trace.segments[0].start_x), y: Number(trace.segments[0].start_y) }];
  for (const s of trace.segments) pts.push({ x: Number(s.end_x), y: Number(s.end_y) });
  return pts;
}

function v2s(pts: Vec2[]): TraceSegmentInfo[] {
  const r: TraceSegmentInfo[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1];
    if (Math.abs(a.x - b.x) < 1 && Math.abs(a.y - b.y) < 1) continue;
    r.push({ start_x: Math.round(a.x), start_y: Math.round(a.y), end_x: Math.round(b.x), end_y: Math.round(b.y) });
  }
  return r;
}

// ---------------------------------------------------------------------------
// Direction vector (45° unit vectors)
// ---------------------------------------------------------------------------
const DIR_VECS: Vec2[] = [
  {x:0,y:1},{x:1,y:1},{x:1,y:0},{x:1,y:-1},
  {x:0,y:-1},{x:-1,y:-1},{x:-1,y:0},{x:-1,y:1},
];
function dv(d: Dir45): Vec2 { return d >= 0 && d < 8 ? DIR_VECS[d] : {x:0,y:0}; }

/** Infinite line intersection: (p1 + t*d1) ∩ (p2 + u*d2) */
function lli(p1: Vec2, d1: Vec2, p2: Vec2, d2: Vec2): Vec2 | null {
  const cross = d1.x * d2.y - d1.y * d2.x;
  if (Math.abs(cross) < 0.001) return null;
  const t = ((p2.x - p1.x) * d2.y - (p2.y - p1.y) * d2.x) / cross;
  return { x: p1.x + t * d1.x, y: p1.y + t * d1.y };
}

/** Finite segment intersection: A→B ∩ C→D */
function ssi(a: Vec2, b: Vec2, c: Vec2, d: Vec2): Vec2 | null {
  const d1x = b.x-a.x, d1y = b.y-a.y, d2x = d.x-c.x, d2y = d.y-c.y;
  const cross = d1x*d2y - d1y*d2x;
  if (Math.abs(cross) < 0.001) return null;
  const t = ((c.x-a.x)*d2y - (c.y-a.y)*d2x) / cross;
  const u = ((c.x-a.x)*d1y - (c.y-a.y)*d1x) / cross;
  if (t < -0.001 || t > 1.001 || u < -0.001 || u > 1.001) return null;
  return { x: a.x+t*d1x, y: a.y+t*d1y };
}

// ---------------------------------------------------------------------------
// dragSegment45 — 1:1 port of KiCad LINE::dragSegment45
// ---------------------------------------------------------------------------

export function dragSegment(
  segments: TraceSegmentInfo[],
  segIndex: number,
  newPos: Vec2,
): TraceSegmentInfo[] | null {
  if (!segments.length || segIndex < 0 || segIndex >= segments.length) return null;

  // Build mutable point array
  let pts = traceVertices({ segments } as TraceInfo);
  if (pts.length < 2) return null;
  const origIndex = segIndex;
  let index = segIndex;

  // KiCad: ensure prev/next exist by inserting zero-length guard segments
  if (index === 0) {
    pts = [{ ...pts[0] }, ...pts]; // insert duplicate at start
    index++;
  }
  if (index === pts.length - 2) { // last segment
    pts = [...pts, { ...pts[pts.length - 1] }]; // insert duplicate at end
  }

  // Now: pts[index]→pts[index+1] = dragged segment
  //      pts[index-1]→pts[index] = s_prev
  //      pts[index+1]→pts[index+2] = s_next
  const dragDir = dirFromSeg(pts[index], pts[index + 1]);
  if (dragDir === Dir45.UNDEFINED) return null;

  let dirPrev = dirFromSeg(pts[index - 1], pts[index]);
  let dirNext = dirFromSeg(pts[index + 1], pts[index + 2]);

  // KiCad: if prev/next colinear with drag, insert another zero-length and force perpendicular
  if (dirPrev === dragDir) {
    pts.splice(index, 0, { ...pts[index] });
    index++;
    dirPrev = leftDir(dragDir);
  } else if (dirPrev === Dir45.UNDEFINED) {
    dirPrev = leftDir(dragDir);
  }
  if (dirNext === dragDir) {
    pts.splice(index + 2, 0, { ...pts[index + 1] });
    dirNext = rightDir(dragDir);
  } else if (dirNext === Dir45.UNDEFINED) {
    dirNext = rightDir(dragDir);
  }

  // Re-read after mutations
  const prevA = pts[index - 1];          // fixed start of prev segment
  const dragA = pts[index];              // start of dragged
  const dragB = pts[index + 1];          // end of dragged
  const nextB = pts[index + 2];          // fixed end of next segment

  // Build guide lines (KiCad logic exactly)
  type Guide = { origin: Vec2; dir: Vec2 };
  const guidesA: Guide[] = [];
  const guidesB: Guide[] = [];

  if (origIndex === 0) {
    guidesA.push({ origin: dragA, dir: dv(rightDir(dragDir)) });
    guidesA.push({ origin: dragA, dir: dv(leftDir(dragDir)) });
  } else {
    const ang = angleBetween(dirPrev, dragDir);
    if (ang === AngleType.ANG_OBTUSE || ang === AngleType.ANG_HALF_FULL) {
      guidesA.push({ origin: prevA, dir: dv(leftDir(dragDir)) });
      guidesA.push({ origin: prevA, dir: dv(rightDir(dragDir)) });
    } else {
      guidesA.push({ origin: dragA, dir: dv(dirPrev) });
      guidesA.push({ origin: dragA, dir: dv(dirPrev) }); // same for both
    }
  }

  if (origIndex === segments.length - 1) {
    guidesB.push({ origin: dragB, dir: dv(rightDir(dragDir)) });
    guidesB.push({ origin: dragB, dir: dv(leftDir(dragDir)) });
  } else {
    const ang = angleBetween(dirNext, dragDir);
    if (ang === AngleType.ANG_OBTUSE || ang === AngleType.ANG_HALF_FULL) {
      guidesB.push({ origin: nextB, dir: dv(leftDir(dragDir)) });
      guidesB.push({ origin: nextB, dir: dv(rightDir(dragDir)) });
    } else {
      guidesB.push({ origin: dragB, dir: dv(dirNext) });
      guidesB.push({ origin: dragB, dir: dv(dirNext) });
    }
  }

  // s_current: line through target in drag direction
  const dragDV = dv(dragDir);
  let bestLen = Infinity;
  let best: Vec2[] | null = null;

  for (let i = 0; i < 2; i++) {
    for (let j = 0; j < 2; j++) {
      const ip1 = lli(newPos, dragDV, guidesA[i].origin, guidesA[i].dir);
      const ip2 = lli(newPos, dragDV, guidesB[j].origin, guidesB[j].dir);
      if (!ip1 || !ip2) continue;

      // KiCad: s1=prevA→ip1, s2=ip1→ip2, s3=ip2→nextB
      // Try segment intersections to reduce to 3-point path
      let np: Vec2[];

      const si1 = ssi(prevA, ip1, dragB, nextB); // s1 ∩ s_next
      const si2 = ssi(ip2, nextB, prevA, dragA); // s3 ∩ s_prev
      const si3 = ssi(prevA, ip1, ip2, nextB);   // s1 ∩ s3

      if (si1) {
        np = [prevA, si1, nextB];
      } else if (si2) {
        np = [prevA, si2, nextB];
      } else if (si3) {
        np = [prevA, si3, nextB];
      } else {
        np = [prevA, ip1, ip2, nextB];
      }

      let len = 0;
      for (let k = 0; k < np.length - 1; k++) len += Math.hypot(np[k+1].x-np[k].x, np[k+1].y-np[k].y);
      if (len < bestLen) { bestLen = len; best = np; }
    }
  }

  if (!best) return null;

  // Replace: KiCad does m_line.Replace(aIndex, aIndex+1, best) then Simplify
  // aIndex and aIndex+1 are the two points of the original dragged segment
  // In our pts array (after insertions), the original segment is at [index, index+1]
  // But we need to map back to the ORIGINAL point array to replace correctly

  // Simpler: rebuild entire path using original points + best result
  const origPts = traceVertices({ segments } as TraceInfo);

  if (origIndex === 0) {
    // Replace first 2 points
    const r = [...best, ...origPts.slice(2)];
    return v2s(r);
  } else if (origIndex === segments.length - 1) {
    // Replace last 2 points
    const r = [...origPts.slice(0, origPts.length - 2), ...best];
    return v2s(r);
  } else {
    // Replace points at origIndex and origIndex+1
    const r = [...origPts.slice(0, origIndex), ...best, ...origPts.slice(origIndex + 2)];
    return v2s(r);
  }
}

// ---------------------------------------------------------------------------
// dragCorner
// ---------------------------------------------------------------------------

function dragCornerInternal(pts: Vec2[], target: Vec2): TraceSegmentInfo[] | null {
  if (!pts.length) return null;
  if (pts.length === 1) return v2s(buildInitialTrace(pts[0], target, Dir45.UNDEFINED));
  if (pts.length === 2) {
    const d = dirFromSeg(pts[1], pts[0]);
    return v2s(buildInitialTrace(pts[0], target, Dir45.UNDEFINED, isDiagonal(d)));
  }
  for (let i = pts.length - 2; i >= 0; i--) {
    const segDir = dirFromSeg(pts[i], pts[i + 1]);
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(pts[i], target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;
      if (dirFromSeg(path[0], path[1]) === segDir) {
        return [...v2s(pts.slice(0, i + 1)), ...v2s(path)];
      }
    }
    const pd = i > 0 ? dirFromSeg(pts[i-1], pts[i]) : Dir45.UNDEFINED;
    for (let j = 0; j < 2; j++) {
      const path = buildInitialTrace(pts[i], target, Dir45.UNDEFINED, j === 1);
      if (path.length < 2) continue;
      const fd = dirFromSeg(path[0], path[1]);
      if (pd !== Dir45.UNDEFINED) {
        const a = angleBetween(fd, pd);
        if (a === AngleType.ANG_OBTUSE || a === AngleType.ANG_STRAIGHT) {
          return [...v2s(pts.slice(0, i + 1)), ...v2s(path)];
        }
      }
    }
  }
  return v2s(buildInitialTrace(pts[0], target, Dir45.UNDEFINED, isDiagonal(dirFromSeg(pts[pts.length-2], pts[pts.length-1]))));
}

export function dragCorner(segments: TraceSegmentInfo[], cornerIndex: number, newPos: Vec2): TraceSegmentInfo[] | null {
  if (!segments.length) return null;
  const pts = traceVertices({ segments } as TraceInfo);
  if (cornerIndex < 0 || cornerIndex >= pts.length) return null;
  if (cornerIndex === 0) return dragCornerInternal(pts, newPos);
  if (cornerIndex === pts.length - 1) {
    const r = dragCornerInternal([...pts].reverse(), newPos);
    return r ? r.map(s => ({ start_x: s.end_x, start_y: s.end_y, end_x: s.start_x, end_y: s.start_y })).reverse() : null;
  }
  const lr = dragCornerInternal(pts.slice(0, cornerIndex + 1), newPos);
  const rr = dragCornerInternal([...pts.slice(cornerIndex)].reverse(), newPos);
  if (!lr || !rr) { const p = [...pts]; p[cornerIndex] = {x:Math.round(newPos.x),y:Math.round(newPos.y)}; return v2s(p); }
  return [...lr, ...rr.map(s => ({start_x:s.end_x,start_y:s.end_y,end_x:s.start_x,end_y:s.start_y})).reverse()];
}

// ---------------------------------------------------------------------------
// Rectangle selection
// ---------------------------------------------------------------------------

export function tracesInRect(snap: BoardSnapshot|null, x1: number, y1: number, x2: number, y2: number): number[] {
  if (!snap?.traces) return [];
  const [mnX,mxX,mnY,mxY] = [Math.min(x1,x2),Math.max(x1,x2),Math.min(y1,y2),Math.max(y1,y2)];
  return snap.traces.filter(t => t.segments.length > 0 && t.segments.every(s => {
    const [sx,sy,ex,ey] = [Number(s.start_x),Number(s.start_y),Number(s.end_x),Number(s.end_y)];
    return sx>=mnX&&sx<=mxX&&sy>=mnY&&sy<=mxY&&ex>=mnX&&ex<=mxX&&ey>=mnY&&ey<=mxY;
  })).map(t => t.id);
}

export function componentsInRect(snap: BoardSnapshot|null, x1: number, y1: number, x2: number, y2: number): string[] {
  if (!snap?.components) return [];
  const [mnX,mxX,mnY,mxY] = [Math.min(x1,x2),Math.max(x1,x2),Math.min(y1,y2),Math.max(y1,y2)];
  return snap.components.filter(c => { const [cx,cy]=[Number(c.x_nm),Number(c.y_nm)]; return cx>=mnX&&cx<=mxX&&cy>=mnY&&cy<=mxY; }).map(c => c.refdes);
}
