import { describe, it, expect } from 'vitest';
import { updateDiagnostics, type SourceDiagnostic } from '../editor/lsp-bridge';

/**
 * The editor used to underline line 1 whatever line the fault was on.
 *
 * `load_source` returns its messages as one blob of text, and this function
 * recovered a line from them with `/[Ll]ine\s+(\d+)/`, falling back to 1. No
 * parse or sync message writes the word "line" - the location lives in a span
 * the engine dropped at the boundary - so the fallback ran every time. The
 * engine carries the line across now.
 */

/** The smallest stand-in for monaco this function needs. */
function fakeMonaco() {
  const set: { markers: { message: string; startLineNumber: number; startColumn: number }[] } = {
    markers: [],
  };
  return {
    monaco: {
      MarkerSeverity: { Error: 8, Warning: 4 },
      editor: {
        setModelMarkers: (_model: unknown, _owner: string, markers: typeof set.markers) => {
          set.markers = markers;
        },
      },
    } as never,
    editor: {
      getModel: () => ({
        getLineCount: () => 40,
        getLineMaxColumn: () => 80,
      }),
    },
    set,
  };
}

const ON_LINE_12: SourceDiagnostic = {
  message: "component 'R1' has no pin '3'. It has: 1, 2",
  line: 12,
  column: 5,
  end_line: 12,
  end_column: 9,
};

describe('the squiggle is on the line the engine named', () => {
  it('puts the marker where the diagnostic says, not on line 1', () => {
    const { monaco, editor, set } = fakeMonaco();
    updateDiagnostics(monaco, editor, [ON_LINE_12], []);

    const marker = set.markers.find((m) => m.message.includes('has no pin'));
    expect(marker, `markers: ${JSON.stringify(set.markers)}`).toBeTruthy();
    expect(marker!.startLineNumber).toBe(12);
    expect(marker!.startColumn).toBe(5);
  });

  it('does not run off the end of a shorter document', () => {
    // A stale diagnostic against a document the user has since cut down would
    // otherwise ask monaco for a line that no longer exists.
    const { monaco, editor, set } = fakeMonaco();
    updateDiagnostics(monaco, editor, [{ ...ON_LINE_12, line: 400, end_line: 400 }], []);

    expect(set.markers[0].startLineNumber).toBeLessThanOrEqual(40);
  });

  it('says nothing when the engine found nothing', () => {
    const { monaco, editor, set } = fakeMonaco();
    updateDiagnostics(monaco, editor, [], []);
    expect(set.markers).toHaveLength(0);
  });
});
