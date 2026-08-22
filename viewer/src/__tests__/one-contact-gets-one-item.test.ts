import { describe, it, expect, vi } from 'vitest';
import { groupByContact, pairOf, gapOf, morePlacesNote } from '../violation-grouping';
import { updateDiagnostics } from '../editor/lsp-bridge';
import type { ViolationInfo } from '../types';

/**
 * The clearance rule reports per pair of segments, and a trace is a chain of
 * them: two features that touch along a run report once for each segment that
 * takes part. The shipped boards give 759 rows for 484 contacts, and one pair
 * of features accounts for 24 of those rows.
 *
 * The count is the rule's own and none of this moves it. What these hold is
 * the reading: one item per contact in the panel, one marker per contact in
 * the editor, and the worst of the group is the one kept.
 */

function clearance(pair: string, actualMm: number, line = 3): ViolationInfo {
  return {
    kind: 'clearance',
    message: `${pair}: Clearance violation: ${actualMm}mm actual, 0.15mm required`,
    x_nm: 1_000_000,
    y_nm: 2_000_000,
    line,
  };
}

describe('the two features a message is about', () => {
  it('is everything before the first colon', () => {
    expect(pairOf("U1 <-> trace 'GND': Clearance violation: 0.05mm actual")).toBe(
      "U1 <-> trace 'GND'"
    );
  });

  it('is the whole message when there is no colon', () => {
    expect(pairOf('drill too small')).toBe('drill too small');
  });

  it('is the same string for every segment of the same run', () => {
    const a = pairOf("R1.2 <-> trace 'VCC': Clearance violation: 0.02mm actual, 0.15mm required");
    const b = pairOf("R1.2 <-> trace 'VCC': Clearance violation: 0.09mm actual, 0.15mm required");
    expect(a).toBe(b);
  });
});

describe('the gap a message says it measured', () => {
  it('is read out of the message', () => {
    expect(gapOf('X <-> Y: Clearance violation: 0.07mm actual, 0.15mm required')).toBeCloseTo(0.07);
  });

  it('sorts last when the message does not carry one', () => {
    expect(gapOf('X <-> Y: too close')).toBe(Number.POSITIVE_INFINITY);
  });
});

describe('one contact gets one item', () => {
  it('keeps one row of a run and counts the rest', () => {
    const grouped = groupByContact([
      clearance('A <-> B', 0.09),
      clearance('A <-> B', 0.04),
      clearance('A <-> B', 0.11),
    ]);
    expect(grouped).toHaveLength(1);
    expect(grouped[0].others).toBe(2);
  });

  it('keeps the worst of the group, not the first', () => {
    const grouped = groupByContact([
      clearance('A <-> B', 0.09),
      clearance('A <-> B', 0.04),
      clearance('A <-> B', 0.11),
    ]);
    expect(grouped[0].violation.message).toContain('0.04mm actual');
  });

  it('keeps a row without a measured gap only when it is alone', () => {
    const noNumber: ViolationInfo = {
      kind: 'clearance',
      message: 'A <-> B: too close',
      x_nm: 0,
      y_nm: 0,
    };
    const grouped = groupByContact([noNumber, clearance('A <-> B', 0.08)]);
    expect(grouped[0].violation.message).toContain('0.08mm actual');
  });

  it('leaves two different contacts as two items', () => {
    const grouped = groupByContact([clearance('A <-> B', 0.04), clearance('C <-> D', 0.04)]);
    expect(grouped).toHaveLength(2);
    expect(grouped.every((g) => g.others === 0)).toBe(true);
  });

  it('does not group the kinds that report per feature', () => {
    const pin = (refdes: string): ViolationInfo => ({
      kind: 'unconnected-pin',
      message: `${refdes}: pin is not connected`,
      x_nm: 0,
      y_nm: 0,
    });
    const grouped = groupByContact([pin('R1.1'), pin('R1.1')]);
    expect(grouped).toHaveLength(2);
  });

  it('places a contact where its worst row arrived, not its first', () => {
    const grouped = groupByContact([
      clearance('A <-> B', 0.09),
      clearance('C <-> D', 0.01),
      clearance('A <-> B', 0.02),
    ]);
    expect(grouped.map((g) => pairOf(g.violation.message))).toEqual(['C <-> D', 'A <-> B']);
  });

  it('says how many more places the same two touch', () => {
    expect(morePlacesNote(1)).toContain('1 more place ');
    expect(morePlacesNote(23)).toContain('23 more places ');
  });
});

describe('the editor gets one marker per contact', () => {
  /** Monaco, reduced to what `updateDiagnostics` actually calls. */
  function fakeMonaco() {
    return {
      MarkerSeverity: { Error: 8, Warning: 4 },
      editor: { setModelMarkers: vi.fn() },
    } as any;
  }

  function fakeEditor() {
    return {
      getModel: () => ({
        getLineCount: () => 40,
        getLineMaxColumn: () => 60,
      }),
    } as any;
  }

  it('collapses a run into one marker carrying the note', () => {
    const monaco = fakeMonaco();
    updateDiagnostics(monaco, fakeEditor(), [], [
      clearance('A <-> B', 0.09),
      clearance('A <-> B', 0.04),
      clearance('A <-> B', 0.11),
    ]);
    const markers = monaco.editor.setModelMarkers.mock.calls[0][2];
    expect(markers).toHaveLength(1);
    expect(markers[0].message).toContain('0.04mm actual');
    expect(markers[0].message).toContain('2 more places');
  });

  it('leaves a lone violation without a note', () => {
    const monaco = fakeMonaco();
    updateDiagnostics(monaco, fakeEditor(), [], [clearance('A <-> B', 0.04)]);
    const markers = monaco.editor.setModelMarkers.mock.calls[0][2];
    expect(markers).toHaveLength(1);
    expect(markers[0].message).not.toContain('more place');
  });

  it('still marks the parse diagnostics it is given', () => {
    const monaco = fakeMonaco();
    updateDiagnostics(
      monaco,
      fakeEditor(),
      [{ message: 'unknown keyword', line: 7, column: 2, end_line: 7, end_column: 9 }],
      [clearance('A <-> B', 0.04)]
    );
    const markers = monaco.editor.setModelMarkers.mock.calls[0][2];
    expect(markers).toHaveLength(2);
    expect(markers[0].startLineNumber).toBe(7);
  });
});
