/**
 * Hit-testing for traces — converts a screen click to the nearest trace.
 * Uses perpendicular distance from point to each segment, accounting for trace copper width.
 */

import type { BoardSnapshot, TraceInfo } from './types';
import type { Viewport } from './viewport';
import { screenToWorld } from './viewport';

export interface HitTestResult {
  trace: TraceInfo;
  segmentIndex: number;
}

/**
 * Compute the minimum distance from a point to a line segment.
 * All coordinates in the same unit (nanometers).
 */
function pointToSegmentDistance(
  px: number, py: number,
  ax: number, ay: number,
  bx: number, by: number,
): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lenSq = dx * dx + dy * dy;

  if (lenSq === 0) {
    // Degenerate segment (zero length)
    const ex = px - ax;
    const ey = py - ay;
    return Math.sqrt(ex * ex + ey * ey);
  }

  // Parameter t for projection of P onto line AB, clamped to [0,1]
  let t = ((px - ax) * dx + (py - ay) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  // Closest point on segment
  const cx = ax + t * dx;
  const cy = ay + t * dy;

  const ex = px - cx;
  const ey = py - cy;
  return Math.sqrt(ex * ex + ey * ey);
}

/**
 * Find the trace closest to the given screen coordinates.
 *
 * @param snapshot   Current board snapshot
 * @param viewport   Current viewport state
 * @param screenX    Click X in screen pixels
 * @param screenY    Click Y in screen pixels
 * @param tolerancePx  Extra tolerance in screen pixels (beyond trace copper width)
 * @returns The closest trace and segment index, or null if nothing is within tolerance.
 */
export function hitTestTrace(
  snapshot: BoardSnapshot | null,
  viewport: Viewport,
  screenX: number,
  screenY: number,
  tolerancePx: number = 5,
): HitTestResult | null {
  if (!snapshot?.traces || snapshot.traces.length === 0) return null;

  const [worldX, worldY] = screenToWorld(viewport, screenX, screenY);
  // Convert pixel tolerance to world units (nanometers)
  const toleranceNm = tolerancePx / viewport.scale;

  let bestDist = Infinity;
  let bestTrace: TraceInfo | null = null;
  let bestSegIdx = 0;

  for (const trace of snapshot.traces) {
    // Total tolerance = pixel tolerance in nm + half the trace copper width
    const hitRadius = toleranceNm + trace.width / 2;

    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const dist = pointToSegmentDistance(
        worldX, worldY,
        seg.start_x, seg.start_y,
        seg.end_x, seg.end_y,
      );
      if (dist <= hitRadius && dist < bestDist) {
        bestDist = dist;
        bestTrace = trace;
        bestSegIdx = i;
      }
    }
  }

  if (bestTrace) {
    return { trace: bestTrace, segmentIndex: bestSegIdx };
  }
  return null;
}
