/**
 * Trace simplification / optimization.
 *
 * Reduces vertex count while preserving trace shape and 45° routing.
 * Equivalent to KiCad's OPTIMIZER with MERGE_SEGMENTS + MERGE_COLINEAR.
 *
 * Modes:
 * - simplify(): merge colinear and redundant vertices
 * - optimize(): try to reduce segment count by replacing 3-segment
 *   sections with shorter 2-segment alternatives
 */

import type { TraceSegmentInfo, BoardSnapshot } from './types';
import { type Vec2, Dir45, dirFromSeg, buildInitialTrace, angleBetween, AngleType } from './direction45';
import { traceVertices } from './trace-edit';
import { checkRouteObstacles } from './routing';

/**
 * Simplify a trace by removing redundant vertices.
 *
 * 1. Remove zero-length segments
 * 2. Merge colinear consecutive segments (same direction)
 * 3. Remove vertices that don't change direction
 *
 * Preserves first and last point (pad connections).
 */
export function simplifyTrace(segments: TraceSegmentInfo[]): TraceSegmentInfo[] {
  if (segments.length <= 1) return segments;

  const pts = traceVertices({ segments, id: 0, width: 0, layer: '', net_name: '', locked: false } as any);
  if (pts.length < 2) return segments;

  // Pass 1: remove zero-length segments and merge colinear
  const simplified: Vec2[] = [pts[0]];

  for (let i = 1; i < pts.length; i++) {
    const prev = simplified[simplified.length - 1];
    const curr = pts[i];

    // Skip zero-length
    if (Math.abs(curr.x - prev.x) < 100 && Math.abs(curr.y - prev.y) < 100) continue;

    // Check if colinear with previous segment (exact same direction only)
    if (simplified.length >= 2) {
      const pp = simplified[simplified.length - 2];
      const dirPrev = dirFromSeg(pp, prev);
      const dirCurr = dirFromSeg(prev, curr);

      if (dirPrev !== Dir45.UNDEFINED && dirPrev === dirCurr) {
        // Exact colinear — extend previous segment
        simplified[simplified.length - 1] = curr;
        continue;
      }
    }

    simplified.push(curr);
  }

  return ptsToSegs(simplified);
}

/**
 * Optimize a trace by trying to reduce segment count.
 *
 * For each consecutive triplet of segments (4 points), tries to replace
 * the middle section with a shorter BuildInitialTrace path. If the result
 * has fewer segments and same start/end, use it.
 *
 * Preserves first and last point (pad connections).
 * Maintains 45° constraint via BuildInitialTrace.
 */
export function optimizeTrace(
  segments: TraceSegmentInfo[],
  snapshot?: BoardSnapshot | null,
  netName?: string,
  clearance?: number,
  traceWidth?: number,
  padNetMap?: Map<string, string>,
): TraceSegmentInfo[] {
  // First: merge exact colinear segments
  let segs = simplifyTrace(segments);
  let pts = ptsFromSegs(segs);
  if (pts.length <= 3) return segs;

  const cl = clearance ?? 150_000;
  const tw = traceWidth ?? 250_000;

  // Try removing each internal vertex one at a time.
  // If the straight line between its neighbors doesn't collide, remove it.
  let changed = true;
  let passes = 0;

  while (changed && passes < 10) {
    changed = false;
    passes++;

    for (let i = 1; i < pts.length - 1; i++) {
      const before = pts[i - 1];
      const after = pts[i + 1];

      // Check: does direct before→after maintain valid 45° direction?
      const dir = dirFromSeg(before, after);
      if (dir === Dir45.UNDEFINED) continue;

      // Check: angles at junctions must not be acute (< 90°)
      // Professional PCB routing rule: always H/V → 45° → H/V
      // Never two diagonals in a row. Only remove vertex if:
      // 1. The resulting segment direction is the same as both neighbors (colinear)
      // 2. OR the resulting connection doesn't create diagonal→diagonal

      // Check: would this create two diagonals in a row?
      const newIsDiag = (dir === Dir45.NE || dir === Dir45.SE || dir === Dir45.SW || dir === Dir45.NW);

      if (i >= 2) {
        const prevDir = dirFromSeg(pts[i - 2], before);
        const prevIsDiag = (prevDir === Dir45.NE || prevDir === Dir45.SE || prevDir === Dir45.SW || prevDir === Dir45.NW);
        // Block: diagonal followed by diagonal
        if (prevIsDiag && newIsDiag) continue;
        // Block: any angle that isn't obtuse or straight
        if (prevDir !== Dir45.UNDEFINED) {
          const ang = angleBetween(prevDir, dir);
          if (ang !== AngleType.ANG_OBTUSE && ang !== AngleType.ANG_STRAIGHT) continue;
        }
      }

      if (i + 2 < pts.length) {
        const nextDir = dirFromSeg(after, pts[i + 2]);
        const nextIsDiag = (nextDir === Dir45.NE || nextDir === Dir45.SE || nextDir === Dir45.SW || nextDir === Dir45.NW);
        // Block: diagonal followed by diagonal
        if (newIsDiag && nextIsDiag) continue;
        if (nextDir !== Dir45.UNDEFINED) {
          const ang = angleBetween(dir, nextDir);
          if (ang !== AngleType.ANG_OBTUSE && ang !== AngleType.ANG_STRAIGHT) continue;
        }
      }

      // Check collision
      if (snapshot && netName) {
        const testPath = [before, after];
        const obstacles = checkRouteObstacles(testPath, snapshot, netName, cl, tw, padNetMap);
        if (obstacles.length > 0) continue;
      }

      // Safe to remove — direct connection is valid and collision-free
      pts.splice(i, 1);
      changed = true;
      break; // restart scan
    }
  }

  return ptsToSegs(pts);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function ptsToSegs(pts: Vec2[]): TraceSegmentInfo[] {
  const segs: TraceSegmentInfo[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1];
    if (Math.abs(a.x - b.x) < 100 && Math.abs(a.y - b.y) < 100) continue;
    segs.push({
      start_x: Math.round(a.x), start_y: Math.round(a.y),
      end_x: Math.round(b.x), end_y: Math.round(b.y),
    });
  }
  return segs;
}

function ptsFromSegs(segs: TraceSegmentInfo[]): Vec2[] {
  if (!segs.length) return [];
  const pts: Vec2[] = [{ x: segs[0].start_x, y: segs[0].start_y }];
  for (const s of segs) pts.push({ x: s.end_x, y: s.end_y });
  return pts;
}

function segLen(a: Vec2, b: Vec2): number {
  return Math.hypot(b.x - a.x, b.y - a.y);
}

function pathLen(pts: Vec2[]): number {
  let len = 0;
  for (let i = 1; i < pts.length; i++) len += segLen(pts[i-1], pts[i]);
  return len;
}
