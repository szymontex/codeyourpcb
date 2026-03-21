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
import { type Vec2, Dir45, dirFromSeg, buildInitialTrace } from './direction45';
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

    // Check if colinear with previous segment
    if (simplified.length >= 2) {
      const pp = simplified[simplified.length - 2];
      const dirPrev = dirFromSeg(pp, prev);
      const dirCurr = dirFromSeg(prev, curr);

      if (dirPrev !== Dir45.UNDEFINED && dirPrev === dirCurr) {
        // Colinear — extend previous segment instead of adding vertex
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
  let pts = traceVertices({ segments, id: 0, width: 0, layer: '', net_name: '', locked: false } as any);
  if (pts.length <= 3) return simplifyTrace(segments);

  // First simplify
  pts = ptsFromSegs(simplifyTrace(segments));
  if (pts.length <= 3) return ptsToSegs(pts);

  /** Check if a candidate point array creates collisions */
  function hasCollision(candidatePts: Vec2[]): boolean {
    if (!snapshot || !netName) return false;
    const obstacles = checkRouteObstacles(candidatePts, snapshot, netName, clearance ?? 150_000, traceWidth ?? 250_000, padNetMap);
    return obstacles.length > 0;
  }

  let improved = true;
  let iterations = 0;

  while (improved && iterations < 10) {
    improved = false;
    iterations++;

    // Try to merge consecutive segment pairs
    for (let i = 0; i < pts.length - 2; i++) {
      const a = pts[i];
      const c = pts[i + 2];

      // Can we replace A→B→C with a direct A→C via BuildInitialTrace?
      const direct = buildInitialTrace(a, c, Dir45.UNDEFINED);

      if (direct.length <= 2) {
        // Direct connection possible (straight line or single bend)
        // Check it's actually shorter
        const oldLen = segLen(a, pts[i + 1]) + segLen(pts[i + 1], c);
        const newLen = pathLen(direct);

        if (newLen <= oldLen * 1.01) { // allow 1% tolerance
          // Check collision before accepting
          const candidate = [...pts.slice(0, i + 1), ...pts.slice(i + 2)];
          if (hasCollision(candidate)) continue;
          // Replace: remove middle point
          pts.splice(i + 1, 1);
          improved = true;
          break; // restart scan
        }
      }

      // Try 3→2 reduction: A→B→C→D → try BuildInitialTrace(A, D)
      if (i + 3 < pts.length) {
        const d = pts[i + 3];
        const direct2 = buildInitialTrace(a, d, Dir45.UNDEFINED);

        if (direct2.length <= 3) { // at most one bend
          const oldLen = segLen(a, pts[i+1]) + segLen(pts[i+1], pts[i+2]) + segLen(pts[i+2], d);
          const newLen = pathLen(direct2);

          if (newLen <= oldLen * 1.01) {
            // Check collision before accepting
            const candidate = [...pts.slice(0, i + 1), ...direct2.slice(1, -1), ...pts.slice(i + 3)];
            if (hasCollision(candidate)) continue;
            // Replace A, B, C, D with direct path
            pts.splice(i + 1, 2, ...direct2.slice(1, -1));
            improved = true;
            break;
          }
        }
      }
    }
  }

  // Final simplify pass
  return simplifyTrace(ptsToSegs(pts));
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
