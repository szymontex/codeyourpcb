/**
 * Hit-testing for traces — converts a screen click to the nearest trace.
 * Uses perpendicular distance from point to each segment, accounting for trace copper width.
 */

import type { BoardSnapshot, TraceInfo } from './types';
import type { Viewport } from './viewport';
import { screenToWorld } from './viewport';
import { pointToSegmentDistance } from './geometry';
import { isLayerVisible, type LayerVisibility } from './layers';

export interface HitTestResult {
  trace: TraceInfo;
  segmentIndex: number;
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
  layers?: LayerVisibility,
): HitTestResult | null {
  if (!snapshot?.traces || snapshot.traces.length === 0) return null;

  const [worldX, worldY] = screenToWorld(viewport, screenX, screenY);
  // Convert pixel tolerance to world units (nanometers)
  const toleranceNm = tolerancePx / viewport.scale;

  let bestDist = Infinity;
  let bestTrace: TraceInfo | null = null;
  let bestSegIdx = 0;
  let bestOnActive = false;

  for (const trace of snapshot.traces) {
    // You cannot pick what you cannot see. This module did not contain the
    // word "layer", so a trace on a hidden layer answered a click as readily
    // as one in front of you - which is how a top trace gets edited while
    // only the bottom is being shown.
    if (layers && !isLayerVisible(trace.layer, layers)) continue;

    // Where two layers cross, the one being worked on wins. Distance alone
    // decides it otherwise, and at a crossing the distances are equal to
    // within a rounding - so the answer was whichever trace the snapshot
    // happened to list first.
    const onActive = layers ? trace.layer === layers.activeLayer : false;

    // Total tolerance = pixel tolerance in nm + half the trace copper width
    const hitRadius = toleranceNm + trace.width / 2;

    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const dist = pointToSegmentDistance(
        worldX, worldY,
        seg.start_x, seg.start_y,
        seg.end_x, seg.end_y,
      );
      if (dist > hitRadius) continue;
      const beatsOnLayer = onActive && !bestOnActive;
      const beatsOnDistance = onActive === bestOnActive && dist < bestDist;
      if (bestTrace === null || beatsOnLayer || beatsOnDistance) {
        bestDist = dist;
        bestTrace = trace;
        bestSegIdx = i;
        bestOnActive = onActive;
      }
    }
  }

  if (bestTrace) {
    return { trace: bestTrace, segmentIndex: bestSegIdx };
  }
  return null;
}
