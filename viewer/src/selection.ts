/**
 * What is selected, and what may be.
 *
 * The editor held a single `selectedTraceId`. Dragging a rectangle over a
 * board found every trace inside it, kept the first, and printed "Selected 12
 * trace(s)" - so the status line said one thing and the editor held another,
 * and `Delete` could only ever remove one trace. There was no select-all at
 * all.
 */

import type { BoardSnapshot } from './types';
import { isLayerVisible, type LayerVisibility } from './layers';

/**
 * Every trace a person could act on right now.
 *
 * Layers decide it: selecting what you cannot see is how a trace on the far
 * side of a board gets deleted by somebody who thought they were clearing the
 * front. The same rule the hit test follows.
 */
export function selectableTraceIds(
  snapshot: BoardSnapshot | null,
  layers?: LayerVisibility,
): number[] {
  if (!snapshot?.traces) return [];
  return snapshot.traces
    .filter((trace) => !layers || isLayerVisible(trace.layer, layers))
    .map((trace) => trace.id);
}

/**
 * The selection after a click, given whether the person is adding to it.
 *
 * Plain click replaces, ctrl or shift adds and removes - the two conventions
 * every editor shares, so nobody has to be told.
 */
export function selectionAfterClick(
  current: ReadonlySet<number>,
  id: number | null,
  additive: boolean,
): Set<number> {
  if (id === null) return additive ? new Set(current) : new Set();
  if (!additive) return new Set([id]);

  const next = new Set(current);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  return next;
}

/** The selection after a rectangle, which adds rather than replaces on ctrl. */
export function selectionAfterRect(
  current: ReadonlySet<number>,
  ids: readonly number[],
  additive: boolean,
): Set<number> {
  const next = additive ? new Set(current) : new Set<number>();
  for (const id of ids) next.add(id);
  return next;
}
