/**
 * KiCad-style obstacle avoidance for interactive routing.
 *
 * When the preview trace crosses a pad of another net, reroutes
 * the ENTIRE path from anchor to cursor to go around the obstacle.
 *
 * Approach: for each colliding pad, generate candidate routes that
 * pass along two edges of the pad's exclusion zone, pick the shortest
 * valid candidate. Uses buildInitialTrace for 45° constrained sub-paths.
 */

import { type Vec2, Dir45, buildInitialTrace } from './direction45';
import type { BoardSnapshot } from './types';
import { padWorldPosition } from './routing';

interface PadRect {
  x: number;
  y: number;
  left: number;
  right: number;
  top: number;    // min Y
  bottom: number; // max Y
  netName: string;
}

/** Force all values to plain Number (WASM BigInt guard). */
const N = Number;

/**
 * Build pad exclusion rectangles for all pads NOT on the routing net.
 */
export function buildPadRects(
  snapshot: BoardSnapshot,
  routingNet: string,
  clearance: number,
  traceWidth: number,
  padNetMap: Map<string, string>,
): PadRect[] {
  const margin = N(clearance) + N(traceWidth) / 2;
  const rects: PadRect[] = [];

  for (const comp of snapshot.components) {
    for (const pad of comp.pads) {
      const key = `${comp.refdes}.${pad.number}`;
      const net = padNetMap.get(key) ?? '';
      if (net === routingNet || !net) continue;

      const [px, py] = padWorldPosition(comp, pad);
      const hw = N(pad.width_nm) / 2 + margin;
      const hh = N(pad.height_nm) / 2 + margin;

      rects.push({
        x: N(px), y: N(py),
        left: N(px) - hw, right: N(px) + hw,
        top: N(py) - hh, bottom: N(py) + hh,
        netName: net,
      });
    }
  }
  return rects;
}

/**
 * Test if a line segment crosses a rectangle (Cohen-Sutherland).
 */
function segCrossesRect(ax: number, ay: number, bx: number, by: number, r: PadRect): boolean {
  function code(x: number, y: number): number {
    let c = 0;
    if (x < r.left) c |= 1;
    else if (x > r.right) c |= 2;
    if (y < r.top) c |= 4;
    else if (y > r.bottom) c |= 8;
    return c;
  }
  let c1 = code(ax, ay), c2 = code(bx, by);
  let x0 = ax, y0 = ay, x1 = bx, y1 = by;
  for (let i = 0; i < 20; i++) {
    if ((c1 | c2) === 0) return true;
    if ((c1 & c2) !== 0) return false;
    const co = c1 || c2;
    let x = 0, y = 0;
    if (co & 8) { x = x0 + (x1 - x0) * (r.bottom - y0) / (y1 - y0); y = r.bottom; }
    else if (co & 4) { x = x0 + (x1 - x0) * (r.top - y0) / (y1 - y0); y = r.top; }
    else if (co & 2) { y = y0 + (y1 - y0) * (r.right - x0) / (x1 - x0); x = r.right; }
    else if (co & 1) { y = y0 + (y1 - y0) * (r.left - x0) / (x1 - x0); x = r.left; }
    if (co === c1) { x0 = x; y0 = y; c1 = code(x0, y0); }
    else { x1 = x; y1 = y; c2 = code(x1, y1); }
  }
  return true;
}

/** Check if ANY segment of a path crosses a rect. */
function pathCrossesRect(path: Vec2[], r: PadRect): boolean {
  for (let i = 0; i < path.length - 1; i++) {
    if (segCrossesRect(path[i].x, path[i].y, path[i + 1].x, path[i + 1].y, r)) return true;
  }
  return false;
}

/** Check if path crosses ANY of the given rects. */
function pathCrossesAnyRect(path: Vec2[], rects: PadRect[]): boolean {
  for (const r of rects) {
    if (pathCrossesRect(path, r)) return true;
  }
  return false;
}

/** Path length. */
function pathLen(pts: Vec2[]): number {
  let len = 0;
  for (let i = 1; i < pts.length; i++) {
    len += Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
  }
  return len;
}

/**
 * Build a candidate route from start to end that goes via a waypoint.
 * Uses BuildInitialTrace for 45° constrained segments.
 * Returns path points or null if any sub-path is degenerate.
 */
function routeViaWaypoint(start: Vec2, waypoint: Vec2, end: Vec2): Vec2[] {
  const seg1 = buildInitialTrace(start, waypoint, Dir45.UNDEFINED);
  const seg2 = buildInitialTrace(waypoint, end, Dir45.UNDEFINED);

  // Merge: seg1 + seg2 (skip duplicate waypoint)
  const result = [...seg1];
  for (let i = 1; i < seg2.length; i++) {
    result.push(seg2[i]);
  }
  return result;
}

/**
 * For a path that collides with a pad rect, try routing around it
 * via each of the 4 corners of the exclusion zone.
 *
 * KiCad routes the detour from the LAST point before the collision
 * to the FIRST point after it. We simplify: route from path start
 * to path end via a corner waypoint.
 */
function dodgeAroundRect(
  start: Vec2,
  end: Vec2,
  rect: PadRect,
  allRects: PadRect[],
): Vec2[] | null {
  // 4 corner waypoints just outside the exclusion zone
  const margin = 100_000; // extra 0.1mm
  const corners: Vec2[] = [
    { x: rect.left - margin,  y: rect.top - margin },     // top-left
    { x: rect.right + margin, y: rect.top - margin },     // top-right
    { x: rect.right + margin, y: rect.bottom + margin },  // bottom-right
    { x: rect.left - margin,  y: rect.bottom + margin },  // bottom-left
  ];

  let bestPath: Vec2[] | null = null;
  let bestLen = Infinity;

  for (const corner of corners) {
    const candidate = routeViaWaypoint(start, corner, end);
    if (candidate.length < 3) continue;

    // Check this candidate doesn't cross the SAME rect
    if (pathCrossesRect(candidate, rect)) continue;

    const len = pathLen(candidate);
    if (len < bestLen) {
      bestLen = len;
      bestPath = candidate;
    }
  }

  // If single-corner dodge fails, try 2-corner routes (go around 2 edges)
  if (!bestPath) {
    for (let i = 0; i < corners.length; i++) {
      const j = (i + 1) % corners.length;
      const c1 = corners[i];
      const c2 = corners[j];

      const seg1 = buildInitialTrace(start, c1, Dir45.UNDEFINED);
      const seg2: Vec2[] = [c1, c2]; // straight edge along exclusion zone
      const seg3 = buildInitialTrace(c2, end, Dir45.UNDEFINED);

      const candidate = [...seg1];
      for (const p of seg2.slice(1)) candidate.push(p);
      for (const p of seg3.slice(1)) candidate.push(p);

      if (pathCrossesRect(candidate, rect)) continue;

      const len = pathLen(candidate);
      if (len < bestLen) {
        bestLen = len;
        bestPath = candidate;
      }
    }
  }

  return bestPath;
}

/**
 * Main dodge function: given a path, reroute around all colliding pad rects.
 * Iterates until no collisions remain (max 8 passes).
 */
export function dodgeObstacles(
  originalPath: Vec2[],
  snapshot: BoardSnapshot,
  routingNet: string,
  clearance: number,
  traceWidth: number,
  padNetMap: Map<string, string>,
): Vec2[] {
  const rects = buildPadRects(snapshot, routingNet, clearance, traceWidth, padNetMap);
  if (rects.length === 0 || originalPath.length < 2) return originalPath;

  let path = originalPath;
  const start = path[0];
  const end = path[path.length - 1];

  for (let pass = 0; pass < 8; pass++) {
    // Find first colliding rect
    let hitRect: PadRect | null = null;
    for (const r of rects) {
      if (pathCrossesRect(path, r)) {
        hitRect = r;
        break;
      }
    }
    if (!hitRect) break; // No more collisions

    // Try to dodge around this rect
    const dodged = dodgeAroundRect(start, end, hitRect, rects);
    if (!dodged) break; // Can't dodge — give up

    path = dodged;

    // Safety: don't let path explode
    if (path.length > 40) break;
  }

  return path;
}
