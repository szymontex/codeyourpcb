/**
 * A click picks what a person can see, and prefers what they are working on.
 *
 * Reported by the owner: "moge zmieniac sciezki ktore sa na top jak mam tylko
 * bottom". `hit-test.ts` did not contain the word "layer" - it took the
 * nearest trace by distance alone, so a hidden layer answered a click as
 * readily as the one in front of you.
 */

import { describe, it, expect } from 'vitest';
import { hitTestTrace } from '../hit-test';
import { createLayerVisibility, toggleLayerVisible, type LayerVisibility } from '../layers';
import type { BoardSnapshot } from '../types';

const MM = 1_000_000;

const viewport = { x: 0, y: 0, scale: 0.01, width: 800, height: 600 } as never;

/** Two traces lying on top of each other, one per side of the board. */
function crossing(): BoardSnapshot {
  const segments = [{ start_x: 0, start_y: 10 * MM, end_x: 40 * MM, end_y: 10 * MM }];
  return {
    board: { width_nm: 40 * MM, height_nm: 20 * MM, layer_count: 2 },
    components: [],
    traces: [
      { id: 1, net_name: 'ONTOP', layer: 'Top', width: MM, segments },
      { id: 2, net_name: 'ONBOTTOM', layer: 'Bottom', width: MM, segments },
    ],
    vias: [],
    ratsnest: [],
  } as unknown as BoardSnapshot;
}

/** Screen coordinates of the point both traces run through. */
function onTheTraces(): [number, number] {
  return [0 * 0.01, 10 * MM * 0.01];
}

function view(active: string): LayerVisibility {
  return { ...createLayerVisibility(), activeLayer: active };
}

describe('a click knows which layers are on', () => {
  it('does not pick a trace on a hidden layer', () => {
    const onlyBottom = toggleLayerVisible(view('Bottom'), 'Top');
    const [x, y] = onTheTraces();
    const hit = hitTestTrace(crossing(), viewport, x, y, 5, onlyBottom);

    expect(hit).not.toBeNull();
    expect(hit!.trace.layer).toBe('Bottom');
  });

  it('picks nothing at all when every layer there is hidden', () => {
    let hidden = toggleLayerVisible(view('Top'), 'Top');
    hidden = toggleLayerVisible(hidden, 'Bottom');
    const [x, y] = onTheTraces();

    expect(hitTestTrace(crossing(), viewport, x, y, 5, hidden)).toBeNull();
  });

  /**
   * Where two layers cross, the distances are equal to within a rounding, so
   * distance alone answered with whichever trace the snapshot listed first.
   * The layer being worked on is the one a person means.
   */
  it('prefers the layer being drawn on where two cross', () => {
    const [x, y] = onTheTraces();

    const fromBottom = hitTestTrace(crossing(), viewport, x, y, 5, view('Bottom'));
    expect(fromBottom!.trace.layer).toBe('Bottom');

    const fromTop = hitTestTrace(crossing(), viewport, x, y, 5, view('Top'));
    expect(fromTop!.trace.layer).toBe('Top');
  });

  /** A caller with no view to offer gets what it always got. */
  it('picks by distance alone when told nothing about layers', () => {
    const [x, y] = onTheTraces();
    expect(hitTestTrace(crossing(), viewport, x, y, 5)).not.toBeNull();
  });
});
