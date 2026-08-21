/**
 * A selection that holds more than one thing.
 *
 * Reported by the owner: "nie moge zrobic ctrl a i usunac". The editor held a
 * single `selectedTraceId`; dragging a rectangle found every trace inside it,
 * kept the first, and printed "Selected 12 trace(s)", so the status line and
 * the editor disagreed and `Delete` could only ever remove one.
 */

import { describe, it, expect } from 'vitest';
import { selectableTraceIds, selectionAfterClick, selectionAfterRect } from '../selection';
import { createLayerVisibility, toggleLayerVisible } from '../layers';
import type { BoardSnapshot } from '../types';

function board(): BoardSnapshot {
  const seg = [{ start_x: 0, start_y: 0, end_x: 1, end_y: 0 }];
  return {
    board: { width_nm: 40, height_nm: 20, layer_count: 4 },
    components: [],
    traces: [
      { id: 1, net_name: 'A', layer: 'Top', width: 1, segments: seg },
      { id: 2, net_name: 'B', layer: 'Bottom', width: 1, segments: seg },
      { id: 3, net_name: 'C', layer: 'Inner1', width: 1, segments: seg },
    ],
    vias: [],
    ratsnest: [],
  } as unknown as BoardSnapshot;
}

describe('what can be selected', () => {
  it('offers every trace when every layer is on', () => {
    expect(selectableTraceIds(board(), createLayerVisibility())).toEqual([1, 2, 3]);
  });

  /**
   * Selecting what you cannot see is how a trace on the far side of a board
   * gets deleted by somebody who thought they were clearing the front.
   */
  it('leaves out traces on hidden layers', () => {
    const noBottom = toggleLayerVisible(createLayerVisibility(), 'Bottom');
    expect(selectableTraceIds(board(), noBottom)).toEqual([1, 3]);
  });

  it('offers everything when told nothing about layers', () => {
    expect(selectableTraceIds(board())).toEqual([1, 2, 3]);
  });

  it('has nothing to offer on an empty board', () => {
    expect(selectableTraceIds(null)).toEqual([]);
  });
});

describe('how a selection changes', () => {
  it('replaces on a plain click', () => {
    expect(selectionAfterClick(new Set([1, 2]), 3, false)).toEqual(new Set([3]));
  });

  it('adds and removes on a modified click', () => {
    expect(selectionAfterClick(new Set([1]), 2, true)).toEqual(new Set([1, 2]));
    expect(selectionAfterClick(new Set([1, 2]), 2, true)).toEqual(new Set([1]));
  });

  /** Clicking empty space clears, unless you were adding to a selection. */
  it('clears on a click into nothing, and keeps it when adding', () => {
    expect(selectionAfterClick(new Set([1, 2]), null, false)).toEqual(new Set());
    expect(selectionAfterClick(new Set([1, 2]), null, true)).toEqual(new Set([1, 2]));
  });

  it('takes everything a rectangle found', () => {
    expect(selectionAfterRect(new Set([9]), [1, 2, 3], false)).toEqual(new Set([1, 2, 3]));
    expect(selectionAfterRect(new Set([9]), [1, 2], true)).toEqual(new Set([9, 1, 2]));
  });
});
