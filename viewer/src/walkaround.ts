/**
 * Walkaround algorithm for interactive routing.
 *
 * When the preview trace collides with obstacle pads/traces of other nets,
 * this module computes an alternative path that goes AROUND the obstacles
 * instead of through them — matching KiCad's walkaround routing behavior.
 *
 * Algorithm overview (simplified from KiCad's graph-based approach):
 * 1. Build convex hull polygons around each obstacle (pad/via/trace) with
 *    clearance + half trace width expansion
 * 2. Walk along the original path; when a segment enters a hull, follow
 *    the hull edge (CW or CCW) until the path exits
 * 3. Try both CW and CCW walkaround, return the shorter valid path
 *
 * Reference: KiCad pcbnew/router/pns_walkaround.cpp, pns_line.cpp, pns_utils.cpp
 */

import type { Vec2 } from './direction45';
import type { BoardSnapshot } from './types';
import { padWorldPosition } from './routing';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ObstacleHull {
  center: Vec2;
  /** Convex hull vertices in CCW order (closed: last connects to first) */
  polygon: Vec2[];
  netName: string;
  type: 'pad' | 'trace' | 'via';
}

interface Intersection {
  /** Parameter along the segment [0,1] */
  t: number;
  /** Intersection point */
  point: Vec2;
  /** Index of hull edge that was intersected */
  hullEdgeIdx: number;
}

// ---------------------------------------------------------------------------
// 1. Hull generation
// ---------------------------------------------------------------------------

const SQRT1_2 = Math.SQRT1_2; // ≈ 0.7071

/**
 * Build an octagonal hull around a rectangular pad — KiCad's OctagonalHull.
 * For circular pads the chamfer is set to produce an 8-sided polygon that
 * approximates the circle.
 *
 * @param padX      Pad center X (world nm)
 * @param padY      Pad center Y (world nm)
 * @param padW      Pad width (nm)
 * @param padH      Pad height (nm)
 * @param padShape  'rect' | 'circle' | 'oval' etc.
 * @param clearance DRC clearance (nm)
 * @param traceWidth Width of the trace being routed (nm)
 * @returns CCW-ordered convex hull polygon
 */
export function computeObstacleHull(
  padX: number,
  padY: number,
  padW: number,
  padH: number,
  padShape: string,
  clearance: number,
  traceWidth: number,
): Vec2[] {
  // Ensure all values are plain numbers (WASM may pass BigInt)
  padX = Number(padX);
  padY = Number(padY);
  padW = Number(padW);
  padH = Number(padH);
  clearance = Number(clearance);
  traceWidth = Number(traceWidth);
  const cl = clearance + Math.ceil(traceWidth / 2);

  // For circles/ovals: use chamfer to approximate the round shape
  // KiCad: chamfer = 2*(1 - 1/sqrt(2)) * (radius + clearance)
  let chamfer: number;

  if (padShape === 'circle') {
    const r = padW / 2;
    chamfer = Math.round(2 * (1 - SQRT1_2) * (r + cl));
  } else if (padShape === 'oval') {
    const r = Math.min(padW, padH) / 2;
    chamfer = Math.round(2 * (1 - SQRT1_2) * (r + cl));
  } else {
    // Rectangular: use a small chamfer for 45° corners
    chamfer = Math.round(cl * 0.4142); // (sqrt(2)-1) * cl ≈ 0.4142*cl
  }

  // p0 = top-left corner of the bounding box (before clearance)
  const p0x = padX - padW / 2;
  const p0y = padY - padH / 2;
  const sx = padW; // size x
  const sy = padH; // size y

  // Build octagonal hull — KiCad's OctagonalHull() vertex order
  // Starting from left edge, going CCW (matching our convention)
  // KiCad builds CW; we reverse for our CCW convention.
  const pts: Vec2[] = [];

  // KiCad order (CW in screen coords = CCW in math coords since Y is flipped):
  // left-top chamfer start, top-left chamfer end, top-right, right-top,
  // right-bottom, bottom-right, bottom-left, left-bottom
  pts.push({ x: p0x - cl,              y: p0y - cl + chamfer });

  if (chamfer > 0)
    pts.push({ x: p0x - cl + chamfer,  y: p0y - cl });

  pts.push({ x: p0x + sx + cl - chamfer, y: p0y - cl });

  if (chamfer > 0)
    pts.push({ x: p0x + sx + cl,       y: p0y - cl + chamfer });

  pts.push({ x: p0x + sx + cl,         y: p0y + sy + cl - chamfer });

  if (chamfer > 0)
    pts.push({ x: p0x + sx + cl - chamfer, y: p0y + sy + cl });

  pts.push({ x: p0x - cl + chamfer,    y: p0y + sy + cl });

  if (chamfer > 0)
    pts.push({ x: p0x - cl,            y: p0y + sy + cl - chamfer });

  // KiCad's vertex order is CW in screen space (Y-down).
  // Our convention is CCW in math space. In screen coords (Y-down), CW *is* the
  // "positive" winding. We keep the array as-is — the walkaround logic handles
  // both CW/CCW traversal by reversing when needed.
  return pts;
}

// ---------------------------------------------------------------------------
// 2. Segment-hull intersection
// ---------------------------------------------------------------------------

/**
 * Find all intersection points between a line segment and a convex hull polygon.
 * Returns intersections sorted by parameter t along the segment.
 */
export function segmentHullIntersections(
  segStart: Vec2,
  segEnd: Vec2,
  hull: Vec2[],
): Intersection[] {
  const results: Intersection[] = [];
  const n = hull.length;
  const dx = segEnd.x - segStart.x;
  const dy = segEnd.y - segStart.y;

  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const hx0 = hull[i].x;
    const hy0 = hull[i].y;
    const hx1 = hull[j].x;
    const hy1 = hull[j].y;

    const hdx = hx1 - hx0;
    const hdy = hy1 - hy0;

    // Solve: segStart + t*(segEnd-segStart) = hull[i] + u*(hull[j]-hull[i])
    const denom = dx * hdy - dy * hdx;
    if (Math.abs(denom) < 0.5) continue; // parallel

    const t = ((hx0 - segStart.x) * hdy - (hy0 - segStart.y) * hdx) / denom;
    const u = ((hx0 - segStart.x) * dy - (hy0 - segStart.y) * dx) / denom;

    // Include points at the boundaries (with small epsilon for numerical stability)
    const EPS = 1e-9;
    if (t >= -EPS && t <= 1 + EPS && u >= -EPS && u <= 1 + EPS) {
      results.push({
        t: Math.max(0, Math.min(1, t)),
        point: {
          x: segStart.x + t * dx,
          y: segStart.y + t * dy,
        },
        hullEdgeIdx: i,
      });
    }
  }

  // Sort by parameter t
  results.sort((a, b) => a.t - b.t);

  // Deduplicate very close intersections (same hull vertex hit twice)
  const deduped: Intersection[] = [];
  for (const r of results) {
    if (deduped.length === 0 || Math.abs(r.t - deduped[deduped.length - 1].t) > 1e-6) {
      deduped.push(r);
    }
  }

  return deduped;
}

// ---------------------------------------------------------------------------
// Hull point containment
// ---------------------------------------------------------------------------

/**
 * Test if a point is strictly inside a convex polygon.
 * Uses cross-product winding. Assumes polygon is in consistent winding order.
 */
function pointInHull(p: Vec2, hull: Vec2[]): boolean {
  const n = hull.length;
  if (n < 3) return false;

  let positive = 0;
  let negative = 0;

  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const cross =
      (hull[j].x - hull[i].x) * (p.y - hull[i].y) -
      (hull[j].y - hull[i].y) * (p.x - hull[i].x);

    if (cross > 0) positive++;
    else if (cross < 0) negative++;

    if (positive > 0 && negative > 0) return false;
  }

  return true;
}

// ---------------------------------------------------------------------------
// 3. Walkaround core — hull-edge following
// ---------------------------------------------------------------------------

/**
 * Follow hull edges from entry point to exit point.
 *
 * @param hull      Hull polygon vertices
 * @param entryIdx  Hull edge index where the path enters
 * @param entryPt   Entry intersection point
 * @param exitIdx   Hull edge index where the path exits
 * @param exitPt    Exit intersection point
 * @param cw        true = follow CW, false = follow CCW
 * @returns Array of points along the hull from entry to exit
 */
function followHull(
  hull: Vec2[],
  entryIdx: number,
  entryPt: Vec2,
  exitIdx: number,
  exitPt: Vec2,
  cw: boolean,
): Vec2[] {
  const n = hull.length;
  const result: Vec2[] = [{ x: entryPt.x, y: entryPt.y }];

  // Determine which vertex to start walking from
  let current: number;
  if (cw) {
    // CW: walk forward through hull indices (next vertex after entry edge)
    current = (entryIdx + 1) % n;
  } else {
    // CCW: walk backward (entry vertex itself)
    current = entryIdx;
  }

  // Walk hull vertices until we reach the exit edge
  const maxSteps = n + 2; // prevent infinite loops
  for (let step = 0; step < maxSteps; step++) {
    // Check if we've reached the exit edge
    if (cw) {
      // CW: we're at the exit when current vertex is the end of the exit edge
      // (or we've passed it)
      const prevIdx = (current + n - 1) % n;
      if (prevIdx === exitIdx) {
        result.push({ x: exitPt.x, y: exitPt.y });
        break;
      }
      if (current === (exitIdx + 1) % n) {
        result.push({ x: exitPt.x, y: exitPt.y });
        break;
      }
    } else {
      // CCW: we've reached exit when we arrive at the exit edge start
      if (current === exitIdx) {
        result.push({ x: exitPt.x, y: exitPt.y });
        break;
      }
      const nextIdx = (current + 1) % n;
      if (nextIdx === exitIdx || current === (exitIdx + n - 1) % n) {
        // Append current hull vertex then exit point
        result.push({ x: hull[current].x, y: hull[current].y });
        result.push({ x: exitPt.x, y: exitPt.y });
        break;
      }
    }

    // Append current hull vertex
    result.push({ x: hull[current].x, y: hull[current].y });

    // Advance
    if (cw) {
      current = (current + 1) % n;
    } else {
      current = (current + n - 1) % n;
    }
  }

  return result;
}

/**
 * Compute path length (sum of segment lengths).
 */
function pathLength(pts: Vec2[]): number {
  let len = 0;
  for (let i = 1; i < pts.length; i++) {
    const dx = pts[i].x - pts[i - 1].x;
    const dy = pts[i].y - pts[i - 1].y;
    len += Math.sqrt(dx * dx + dy * dy);
  }
  return len;
}

/**
 * Remove redundant colinear points from a path (simplification).
 */
function simplifyPath(pts: Vec2[]): Vec2[] {
  if (pts.length <= 2) return pts;

  const result: Vec2[] = [pts[0]];

  for (let i = 1; i < pts.length - 1; i++) {
    const prev = result[result.length - 1];
    const curr = pts[i];
    const next = pts[i + 1];

    // Check if curr is colinear with prev→next
    const cross =
      (curr.x - prev.x) * (next.y - prev.y) -
      (curr.y - prev.y) * (next.x - prev.x);

    // Also skip zero-length segments
    const dx = curr.x - prev.x;
    const dy = curr.y - prev.y;
    if (dx * dx + dy * dy < 1) continue;

    if (Math.abs(cross) > 100) {
      // Not colinear — keep the point
      result.push(curr);
    }
  }

  result.push(pts[pts.length - 1]);
  return result;
}

/**
 * Walk around a single obstacle hull for one segment of the path.
 *
 * Given a path that intersects a hull, computes a detour around it.
 * Returns null if walkaround is not possible (e.g. start inside hull).
 */
function walkaroundSingleHull(
  pathPts: Vec2[],
  hull: Vec2[],
  cw: boolean,
): Vec2[] | null {
  const result: Vec2[] = [];
  let insideHull = false;
  let entryPt: Intersection | null = null;

  for (let i = 0; i < pathPts.length - 1; i++) {
    const a = pathPts[i];
    const b = pathPts[i + 1];

    const ixns = segmentHullIntersections(a, b, hull);

    if (ixns.length === 0) {
      if (!insideHull) {
        // Segment is entirely outside — keep it
        if (result.length === 0 || dist2(result[result.length - 1], a) > 1) {
          result.push({ x: a.x, y: a.y });
        }
      }
      // If inside hull, we're still following the hull — skip this segment
      continue;
    }

    if (!insideHull) {
      // Path enters the hull at the first intersection
      const entry = ixns[0];

      // Add path up to entry point
      if (result.length === 0 || dist2(result[result.length - 1], a) > 1) {
        result.push({ x: a.x, y: a.y });
      }
      if (entry.t > 1e-6) {
        result.push({ x: entry.point.x, y: entry.point.y });
      }

      if (ixns.length >= 2) {
        // Segment enters and exits the hull in the same segment
        const exit = ixns[ixns.length - 1];
        const hullPath = followHull(hull, entry.hullEdgeIdx, entry.point,
                                     exit.hullEdgeIdx, exit.point, cw);
        for (const hp of hullPath) {
          result.push({ x: hp.x, y: hp.y });
        }
        // Continue with the rest of the segment after exit
        // insideHull stays false
      } else {
        // Only entry, no exit on this segment — we're entering the hull
        insideHull = true;
        entryPt = entry;
      }
    } else {
      // We're inside the hull, looking for the exit
      if (ixns.length > 0) {
        const exit = ixns[ixns.length - 1];

        // Follow hull from entry to exit
        if (entryPt) {
          const hullPath = followHull(hull, entryPt.hullEdgeIdx, entryPt.point,
                                       exit.hullEdgeIdx, exit.point, cw);
          for (const hp of hullPath) {
            result.push({ x: hp.x, y: hp.y });
          }
        }

        insideHull = false;
        entryPt = null;
      }
      // If no intersections while inside, continue looking
    }
  }

  // Add the final point if we're outside
  if (!insideHull) {
    const last = pathPts[pathPts.length - 1];
    if (result.length === 0 || dist2(result[result.length - 1], last) > 1) {
      result.push({ x: last.x, y: last.y });
    }
  } else {
    // Path ends inside the hull — walkaround failed
    return null;
  }

  return result.length >= 2 ? simplifyPath(result) : null;
}

/** Squared distance between two points. */
function dist2(a: Vec2, b: Vec2): number {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return dx * dx + dy * dy;
}

// ---------------------------------------------------------------------------
// 4. Full walkaround — iterate over all obstacles
// ---------------------------------------------------------------------------

/**
 * Build obstacle hulls from the board snapshot for all pads/vias NOT on
 * the current net.
 */
export function buildObstacleHulls(
  snapshot: BoardSnapshot | null,
  netName: string,
  clearance: number,
  traceWidth: number,
  padNetMap?: Map<string, string>,
): ObstacleHull[] {
  if (!snapshot) return [];

  const hulls: ObstacleHull[] = [];

  for (const comp of snapshot.components) {
    for (const pad of comp.pads) {
      const padKey = `${comp.refdes}.${pad.number}`;
      const padNet = padNetMap?.get(padKey) ?? '';

      // Skip pads on the same net or unconnected
      if (padNet === netName || !padNet) continue;

      const [px, py] = padWorldPosition(comp, pad);

      // Cast to Number — WASM may return BigInt for i64 fields
      let effW = Number(pad.width_nm);
      let effH = Number(pad.height_nm);
      if (Math.abs(Number(comp.rotation_mdeg) % 180000) > 100) {
        // Non-axis-aligned rotation: use bounding circle
        const diag = Math.sqrt(effW * effW + effH * effH);
        effW = diag;
        effH = diag;
      }

      const polygon = computeObstacleHull(px, py, effW, effH, pad.shape, clearance, traceWidth);

      hulls.push({
        center: { x: px, y: py },
        polygon,
        netName: padNet,
        type: 'pad',
      });
    }
  }

  // Build hulls for existing traces of other nets
  if (snapshot.traces) {
    for (const trace of snapshot.traces) {
      if (trace.net_name === netName || !trace.net_name) continue;

      for (const seg of trace.segments) {
        // Build a rectangular hull around each trace segment
        const cx = (seg.start_x + seg.end_x) / 2;
        const cy = (seg.start_y + seg.end_y) / 2;
        const dx = seg.end_x - seg.start_x;
        const dy = seg.end_y - seg.start_y;
        const len = Math.sqrt(dx * dx + dy * dy);
        if (len < 1) continue;

        // Build an axis-aligned bounding box hull for the trace segment
        // (simplification — a proper implementation would rotate)
        const hw = trace.width / 2;
        const minX = Math.min(seg.start_x, seg.end_x) - hw;
        const maxX = Math.max(seg.start_x, seg.end_x) + hw;
        const minY = Math.min(seg.start_y, seg.end_y) - hw;
        const maxY = Math.max(seg.start_y, seg.end_y) + hw;

        const bw = maxX - minX;
        const bh = maxY - minY;

        const polygon = computeObstacleHull(
          cx, cy, bw, bh, 'rect', clearance, traceWidth,
        );

        hulls.push({
          center: { x: cx, y: cy },
          polygon,
          netName: trace.net_name,
          type: 'trace',
        });
      }
    }
  }

  // Build hulls for vias of other nets
  if (snapshot.vias) {
    for (const via of snapshot.vias) {
      if (via.net_name === netName || !via.net_name) continue;

      const polygon = computeObstacleHull(
        via.x, via.y,
        via.outer_diameter, via.outer_diameter,
        'circle', clearance, traceWidth,
      );

      hulls.push({
        center: { x: via.x, y: via.y },
        polygon,
        netName: via.net_name,
        type: 'via',
      });
    }
  }

  return hulls;
}

/**
 * Check if a path has any collision with a set of obstacle hulls.
 */
function pathCollidesWithHull(path: Vec2[], hull: Vec2[]): boolean {
  for (let i = 0; i < path.length - 1; i++) {
    if (segmentHullIntersections(path[i], path[i + 1], hull).length > 0) {
      return true;
    }
  }
  // Also check if any path point is inside the hull
  for (const p of path) {
    if (pointInHull(p, hull)) return true;
  }
  return false;
}

/**
 * Main walkaround algorithm.
 *
 * Takes the original path and a set of obstacle hulls, returns a new path
 * that avoids all obstacles by following hull edges.
 *
 * Strategy (matching KiCad's WP_SHORTEST policy):
 * - For each obstacle that intersects the path, try both CW and CCW walkaround
 * - Pick the shorter result that doesn't create new collisions
 * - Iterate up to `maxIterations` times (obstacles may chain)
 *
 * @param originalPath  The initial trace path from buildInitialTrace
 * @param obstacles     Pre-computed obstacle hulls
 * @returns The walked-around path, or null if walkaround is stuck
 */
export function walkaroundPath(
  originalPath: Vec2[],
  obstacles: ObstacleHull[],
): Vec2[] | null {
  if (originalPath.length < 2 || obstacles.length === 0) return originalPath;

  let currentPath = originalPath;
  const maxIterations = obstacles.length * 2 + 4; // enough passes to clear all obstacles
  const maxLengthFactor = 10; // don't accept paths > 10× original length
  const originalLength = pathLength(originalPath);

  for (let iter = 0; iter < maxIterations; iter++) {
    // Find first obstacle that collides with current path
    let collidingHull: Vec2[] | null = null;

    for (const obs of obstacles) {
      if (pathCollidesWithHull(currentPath, obs.polygon)) {
        collidingHull = obs.polygon;
        break;
      }
    }

    if (!collidingHull) {
      // No more collisions — we're done
      return currentPath;
    }

    // Try both CW and CCW walkaround
    const pathCW = walkaroundSingleHull(currentPath, collidingHull, true);
    const pathCCW = walkaroundSingleHull(currentPath, collidingHull, false);

    // Pick the better result
    let best: Vec2[] | null = null;

    if (pathCW && pathCCW) {
      const lenCW = pathLength(pathCW);
      const lenCCW = pathLength(pathCCW);

      // Check if either path still collides with the same hull
      const cwStillCollides = pathCollidesWithHull(pathCW, collidingHull);
      const ccwStillCollides = pathCollidesWithHull(pathCCW, collidingHull);

      if (!cwStillCollides && !ccwStillCollides) {
        best = lenCW <= lenCCW ? pathCW : pathCCW;
      } else if (!cwStillCollides) {
        best = pathCW;
      } else if (!ccwStillCollides) {
        best = pathCCW;
      } else {
        // Both still collide — pick shorter and hope next iteration fixes it
        best = lenCW <= lenCCW ? pathCW : pathCCW;
      }
    } else if (pathCW) {
      best = pathCW;
    } else if (pathCCW) {
      best = pathCCW;
    }

    if (!best) {
      // Walkaround failed — stuck
      return null;
    }

    // Length explosion guard
    if (pathLength(best) > originalLength * maxLengthFactor) {
      return null;
    }

    currentPath = best;
  }

  // Iteration limit reached — check if we're collision-free
  for (const obs of obstacles) {
    if (pathCollidesWithHull(currentPath, obs.polygon)) {
      return null;
    }
  }

  return currentPath;
}
