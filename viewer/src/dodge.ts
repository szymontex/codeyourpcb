/**
 * Simple obstacle dodge — deterministic pad avoidance.
 *
 * Instead of complex graph-based walkaround, this adds waypoints
 * around each obstacle pad that the path crosses. For each collision:
 * - Compute pad bounding box + clearance
 * - Try 4 dodge routes (above, below, left, right of pad)
 * - Pick the shortest valid one
 *
 * This runs per-frame during routing preview.
 */

import type { Vec2 } from './direction45';
import type { BoardSnapshot } from './types';
import { padWorldPosition } from './routing';

interface PadObstacle {
  x: number;
  y: number;
  halfW: number; // half-width including clearance
  halfH: number; // half-height including clearance
  netName: string;
}

/**
 * Build a list of pad obstacles for other nets.
 */
export function buildPadObstacles(
  snapshot: BoardSnapshot,
  routingNet: string,
  clearance: number,
  traceWidth: number,
  padNetMap: Map<string, string>,
): PadObstacle[] {
  const margin = clearance + traceWidth / 2;
  const obstacles: PadObstacle[] = [];

  for (const comp of snapshot.components) {
    for (const pad of comp.pads) {
      const key = `${comp.refdes}.${pad.number}`;
      const net = padNetMap.get(key) ?? '';
      if (net === routingNet || !net) continue;

      const [px, py] = padWorldPosition(comp, pad);
      obstacles.push({
        x: Number(px),
        y: Number(py),
        halfW: Number(pad.width_nm) / 2 + margin,
        halfH: Number(pad.height_nm) / 2 + margin,
        netName: net,
      });
    }
  }
  return obstacles;
}

/**
 * Check if a line segment intersects a rectangle.
 */
function segHitsRect(
  ax: number, ay: number, bx: number, by: number,
  obs: PadObstacle,
): boolean {
  const left = obs.x - obs.halfW;
  const right = obs.x + obs.halfW;
  const top = obs.y - obs.halfH;
  const bottom = obs.y + obs.halfH;

  // Cohen-Sutherland outcode
  function outcode(x: number, y: number): number {
    let code = 0;
    if (x < left) code |= 1;
    else if (x > right) code |= 2;
    if (y < top) code |= 4;
    else if (y > bottom) code |= 8;
    return code;
  }

  let c1 = outcode(ax, ay);
  let c2 = outcode(bx, by);

  let x0 = ax, y0 = ay, x1 = bx, y1 = by;

  for (let i = 0; i < 20; i++) {
    if ((c1 | c2) === 0) return true;   // both inside
    if ((c1 & c2) !== 0) return false;  // both outside same side

    const cOut = c1 !== 0 ? c1 : c2;
    let x = 0, y = 0;
    if (cOut & 8) { x = x0 + (x1 - x0) * (bottom - y0) / (y1 - y0); y = bottom; }
    else if (cOut & 4) { x = x0 + (x1 - x0) * (top - y0) / (y1 - y0); y = top; }
    else if (cOut & 2) { y = y0 + (y1 - y0) * (right - x0) / (x1 - x0); x = right; }
    else if (cOut & 1) { y = y0 + (y1 - y0) * (left - x0) / (x1 - x0); x = left; }

    if (cOut === c1) { x0 = x; y0 = y; c1 = outcode(x0, y0); }
    else { x1 = x; y1 = y; c2 = outcode(x1, y1); }
  }
  return true;
}

/**
 * For a path that hits an obstacle, compute a detour around it.
 * Returns a new path that goes around the pad via one of 4 corners.
 */
function dodgeSingleObstacle(path: Vec2[], obs: PadObstacle): Vec2[] {
  // Find first segment that hits the obstacle
  let hitIdx = -1;
  for (let i = 0; i < path.length - 1; i++) {
    if (segHitsRect(path[i].x, path[i].y, path[i + 1].x, path[i + 1].y, obs)) {
      hitIdx = i;
      break;
    }
  }
  if (hitIdx < 0) return path;

  const a = path[hitIdx];
  const b = path[hitIdx + 1];

  // 4 candidate dodge points (corners of the obstacle rect + small margin)
  const m = 100_000; // extra 0.1mm margin
  const corners: Vec2[] = [
    { x: obs.x - obs.halfW - m, y: obs.y - obs.halfH - m }, // top-left
    { x: obs.x + obs.halfW + m, y: obs.y - obs.halfH - m }, // top-right
    { x: obs.x + obs.halfW + m, y: obs.y + obs.halfH + m }, // bottom-right
    { x: obs.x - obs.halfW - m, y: obs.y + obs.halfH + m }, // bottom-left
  ];

  // Try each corner as dodge point, build 2-segment detour (a→corner→b)
  let bestPath: Vec2[] | null = null;
  let bestLen = Infinity;

  for (const corner of corners) {
    // Check the detour segments don't hit the same obstacle
    const seg1Hits = segHitsRect(a.x, a.y, corner.x, corner.y, obs);
    const seg2Hits = segHitsRect(corner.x, corner.y, b.x, b.y, obs);

    if (!seg1Hits && !seg2Hits) {
      const len = Math.hypot(corner.x - a.x, corner.y - a.y) +
                  Math.hypot(b.x - corner.x, b.y - corner.y);
      if (len < bestLen) {
        bestLen = len;
        // Build new path: [...before hit, a, corner, b, ...after hit]
        bestPath = [
          ...path.slice(0, hitIdx + 1),
          { x: corner.x, y: corner.y },
          ...path.slice(hitIdx + 1),
        ];
      }
    }
  }

  return bestPath ?? path;
}

/**
 * Dodge all obstacle pads in the path.
 * Iteratively resolves collisions (max 10 passes).
 */
export function dodgeObstacles(
  originalPath: Vec2[],
  snapshot: BoardSnapshot,
  routingNet: string,
  clearance: number,
  traceWidth: number,
  padNetMap: Map<string, string>,
): Vec2[] {
  const obstacles = buildPadObstacles(snapshot, routingNet, clearance, traceWidth, padNetMap);
  if (obstacles.length === 0) return originalPath;

  let path = originalPath;

  for (let pass = 0; pass < 10; pass++) {
    let anyHit = false;
    for (const obs of obstacles) {
      // Check if any segment hits this obstacle
      let hits = false;
      for (let i = 0; i < path.length - 1; i++) {
        if (segHitsRect(path[i].x, path[i].y, path[i + 1].x, path[i + 1].y, obs)) {
          hits = true;
          break;
        }
      }
      if (hits) {
        path = dodgeSingleObstacle(path, obs);
        anyHit = true;
      }
    }
    if (!anyHit) break;

    // Safety: don't let path explode
    if (path.length > 50) break;
  }

  return path;
}
