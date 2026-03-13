/**
 * Shared geometry utilities.
 * Extracted to avoid duplication across hit-test.ts, wasm.ts, and other modules.
 */

/**
 * Compute the minimum distance from a point to a line segment.
 * All coordinates must be in the same unit (e.g. nanometers).
 */
export function pointToSegmentDistance(
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
