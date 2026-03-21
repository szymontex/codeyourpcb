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
      const b = pts[i + 1];
      const c = pts[i + 2];

      // Use existing first segment direction as posture hint
      const existingDir = dirFromSeg(a, b);

      // Can we replace A→B→C with a direct A→C?
      // Try both postures, pick one that matches existing direction
      let bestDirect: Vec2[] | null = null;
      for (let j = 0; j < 2; j++) {
        const d = buildInitialTrace(a, c, Dir45.UNDEFINED, j === 1);
        if (d.length > 3) continue; // too many segments
        // Prefer posture that matches existing first segment direction
        if (d.length >= 2) {
          const newDir = dirFromSeg(d[0], d[1]);
          if (newDir === existingDir || bestDirect === null) {
            bestDirect = d;
            if (newDir === existingDir) break; // perfect match
          }
        }
      }

      if (bestDirect && bestDirect.length <= 3) {
        const oldLen = segLen(a, b) + segLen(b, c);
        const newLen = pathLen(bestDirect);

        if (newLen <= oldLen * 1.01) {
          // Build candidate with this replacement
          const candidate = [...pts.slice(0, i), ...bestDirect, ...pts.slice(i + 3)];
          if (hasCollision(candidate)) continue;
          pts = candidate;
          improved = true;
          break;
        }
      }

      // Try 3→2 reduction: A→B→C→D
      if (i + 3 < pts.length) {
        const dd = pts[i + 3];
        let bestDirect2: Vec2[] | null = null;
        for (let j = 0; j < 2; j++) {
          const d2 = buildInitialTrace(a, dd, Dir45.UNDEFINED, j === 1);
          if (d2.length > 3) continue;
          if (d2.length >= 2) {
            const newDir = dirFromSeg(d2[0], d2[1]);
            if (newDir === existingDir || bestDirect2 === null) {
              bestDirect2 = d2;
              if (newDir === existingDir) break;
            }
          }
        }

        if (bestDirect2 && bestDirect2.length <= 3) {
          const oldLen = segLen(a, b) + segLen(b, c) + segLen(c, dd);
          const newLen = pathLen(bestDirect2);

          if (newLen <= oldLen * 1.01) {
            const candidate = [...pts.slice(0, i), ...bestDirect2, ...pts.slice(i + 4)];
            if (hasCollision(candidate)) continue;
            pts = candidate;
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
