import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { designKindOf } from '../design-load';

/**
 * A design is read by the reader its name asks for, and a new one starts clean.
 *
 * The desktop app's open and new file run only inside Tauri, so the browser
 * tests in `a-new-design-starts-clean.spec.ts` cannot reach them. Its open
 * handed every file to the `.cypcb` reader, a KiCad board included, and
 * neither handler forgot the design before it: the kind, the file handle, the
 * selection and the undo stack all carried over.
 */

const KICAD = '(kicad_pcb (version 20221018) (generator pcbnew))';
const DSL = 'version 1\n\nboard b {\n    size 10mm x 10mm\n}\n';

describe('designKindOf', () => {
  it('reads a KiCad board by its extension, on either platform, in any case', () => {
    expect(designKindOf('/boards/amp.kicad_pcb', DSL)).toBe('kicad_pcb');
    expect(designKindOf('C:\\boards\\Amp.KICAD_PCB', DSL)).toBe('kicad_pcb');
  });

  it('reads a .cypcb as the design language', () => {
    expect(designKindOf('/boards/amp.cypcb', DSL)).toBe('cypcb');
    expect(designKindOf('design.cypcb', '')).toBe('cypcb');
  });

  it('reads a KiCad board by its first token when the name does not say', () => {
    // A recent card keeps whatever name the board was stored under.
    expect(designKindOf('Blink LED.cypcb', `\n  ${KICAD}`)).toBe('kicad_pcb');
  });
});

/** The body of the listener the desktop app registers for `event`. */
function desktopHandler(main: string, event: string): string {
  const start = main.indexOf(`window.addEventListener('${event}'`);
  expect(start, `no listener for ${event}`).toBeGreaterThan(-1);
  const next = main.indexOf('window.addEventListener(', start + 1);
  return main.slice(start, next === -1 ? undefined : next);
}

describe('the desktop app starts each design clean', () => {
  const main = readFileSync(new URL('../main.ts', import.meta.url), 'utf-8');

  it('writes the kind in one place, with the text', () => {
    // Every loader used to write it itself, and the ones that did not left a
    // KiCad board's reader under the next design.
    expect(main.match(/\bloadedKind = /g)).toHaveLength(1);
  });

  for (const event of ['desktop:open-file', 'desktop:new-file']) {
    it(`${event} forgets the last design before it loads the next`, () => {
      const body = desktopHandler(main, event);
      const forgets = body.indexOf('beginNewDesign();');
      expect(forgets, `${event} keeps the last design's handle, selection and undo stack`).toBeGreaterThan(-1);
      expect(forgets).toBeLessThan(body.indexOf('loadDesign('));
    });
  }

  it('desktop:open-file reads the file with the reader its name asks for', () => {
    expect(desktopHandler(main, 'desktop:open-file')).toContain('loadDesign(content, designKindOf(path, content))');
  });
});
