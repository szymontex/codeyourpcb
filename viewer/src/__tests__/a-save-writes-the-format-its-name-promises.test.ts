import { afterEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { designNameFor, formatOfText, refuseForeignFormat, saveFile } from '../file-access';

/**
 * A save writes the format its file name promises, and nothing else.
 *
 * Ctrl+S on a KiCad board spliced the copper, as trace blocks of this
 * language, onto the end of the KiCad text and wrote that over the
 * `.kicad_pcb`. The reader stops at the board's closing bracket, so the file
 * still opened - without the copper, and without a word. Every write now asks
 * `refuseForeignFormat` first; the browser paths are in
 * `a-save-never-writes-over-a-kicad-board.spec.ts`, the desktop's here.
 */

const KICAD = '(kicad_pcb (version 20221018) (generator "pcbnew")\n  (gr_text "a ) in a string" (at 0 0))\n)\n';
const DSL = 'version 1\n\nboard b {\n    size 10mm x 10mm\n}\n';
const SPLICED = `${KICAD}\n// --- Routed traces (auto-generated) ---\ntrace "" {\n    layer Top\n}\n`;

type Written = { to: string; text: string };

function handle(name: string, writes: Written[]) {
  return {
    kind: 'file',
    name,
    createWritable: async () => ({
      write: async (text: string) => {
        writes.push({ to: name, text });
      },
      close: async () => {},
    }),
  } as unknown as FileSystemFileHandle;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('formatOfText', () => {
  it('reads a KiCad board as one expression, brackets in strings included', () => {
    expect(formatOfText(KICAD)).toBe('kicad_pcb');
  });

  it('reads anything after the board as another format spliced on', () => {
    expect(formatOfText(SPLICED)).toBe('mixed');
  });

  it('reads a board that never closes as damaged, not as KiCad', () => {
    expect(formatOfText('(kicad_pcb (version 1)')).toBe('mixed');
  });

  it('reads a design as a design', () => {
    expect(formatOfText(DSL)).toBe('cypcb');
    expect(formatOfText('')).toBe('cypcb');
  });
});

describe('refuseForeignFormat', () => {
  it('lets each format into the file named for it', () => {
    expect(() => refuseForeignFormat('amp.kicad_pcb', KICAD)).not.toThrow();
    expect(() => refuseForeignFormat('amp.cypcb', DSL)).not.toThrow();
  });

  it('refuses copper spliced onto a KiCad board', () => {
    expect(() => refuseForeignFormat('amp.kicad_pcb', SPLICED)).toThrow(/Not saved: amp.kicad_pcb/);
  });

  it('refuses a design in a KiCad file and a KiCad board in a design, in any case', () => {
    expect(() => refuseForeignFormat('Amp.KICAD_PCB', DSL)).toThrow(/a .cypcb design/);
    expect(() => refuseForeignFormat('amp.cypcb', KICAD)).toThrow(/a KiCad board/);
  });

  it('names a KiCad board\'s design after it', () => {
    expect(designNameFor('amp.KiCad_Pcb')).toBe('amp.cypcb');
    expect(designNameFor('amp.cypcb')).toBe('amp.cypcb');
  });
});

describe('saveFile writes nothing the name does not promise', () => {
  it('to the file a board was opened from', async () => {
    const writes: Written[] = [];
    await expect(saveFile(SPLICED, handle('amp.kicad_pcb', writes), 'amp.kicad_pcb')).rejects.toThrow(/Not saved/);
    // A refusal is not a failed write to fall through to save-as on.
    expect(writes).toEqual([]);
  });

  it('to a file the save picker hands back', async () => {
    const writes: Written[] = [];
    vi.stubGlobal('window', {
      showOpenFilePicker: async () => [],
      showSaveFilePicker: async () => handle('amp.kicad_pcb', writes),
    });
    await expect(saveFile(DSL, null, 'amp.cypcb')).rejects.toThrow(/Not saved/);
    expect(writes).toEqual([]);
  });

  it('to a download', async () => {
    const clicks: string[] = [];
    vi.stubGlobal('window', {});
    vi.stubGlobal('document', {
      createElement: () => ({ click: () => clicks.push('download'), remove: () => {} }),
    });
    await expect(saveFile(DSL, null, 'amp.kicad_pcb')).rejects.toThrow(/Not saved/);
    expect(clicks).toEqual([]);
  });

  it('and still writes what it does promise', async () => {
    const writes: Written[] = [];
    await saveFile(DSL, handle('amp.cypcb', writes), 'amp.cypcb');
    expect(writes).toEqual([{ to: 'amp.cypcb', text: DSL }]);
  });
});

describe('the desktop app asks the same guard', () => {
  // Its save and save-as run only inside Tauri; what they call is read here.
  const desktop = readFileSync(new URL('../desktop.ts', import.meta.url), 'utf-8');

  function body(name: string): string {
    const start = desktop.indexOf(`async function ${name}(`);
    expect(start, `no ${name}`).toBeGreaterThan(-1);
    return desktop.slice(start, desktop.indexOf('\n}\n', start));
  }

  it('saves a KiCad board through save-as, never over it', () => {
    expect(body('handleSaveFile')).toContain("formatOfName(currentFilePath) === 'kicad_pcb'");
  });

  for (const [fn, command] of [['handleSaveFile', "'save_file'"], ['handleSaveFileAs', "'save_file_as'"]]) {
    it(`${fn} asks before ${command} writes`, () => {
      const text = body(fn);
      const asks = text.indexOf('refuseForeignFormat(');
      expect(asks, `${fn} writes without asking`).toBeGreaterThan(-1);
      expect(asks).toBeLessThan(text.indexOf(`invoke(${command}`));
    });
  }

  it('hands a KiCad board over as the design from-kicad writes, not with copper spliced on', () => {
    const main = readFileSync(new URL('../main.ts', import.meta.url), 'utf-8');
    const start = main.indexOf("window.addEventListener('desktop:content-request'");
    expect(start).toBeGreaterThan(-1);
    const handler = main.slice(start, main.indexOf('window.addEventListener(', start + 1));
    const kicad = handler.indexOf("if (loadedKind === 'kicad_pcb')");
    expect(kicad, 'a KiCad board goes out with trace blocks on its text').toBeGreaterThan(-1);
    expect(handler.indexOf('engine.design_as_dsl()', kicad)).toBeGreaterThan(kicad);
    expect(kicad).toBeLessThan(handler.indexOf('mergeTracesIntoDsl('));
  });

  it('names what the design leaves out before the desktop writes it', () => {
    // The desktop reports no status after its own save, so the list goes up
    // while the content is handed over. The browser test sees only the web
    // path.
    const main = readFileSync(new URL('../main.ts', import.meta.url), 'utf-8');
    const start = main.indexOf("window.addEventListener('desktop:content-request'");
    const handler = main.slice(start, main.indexOf('window.addEventListener(', start + 1));
    const kicad = handler.indexOf("if (loadedKind === 'kicad_pcb')");
    const note = handler.indexOf('notWrittenNote()', kicad);
    expect(note, 'a KiCad board is saved on the desktop without saying what it lost').toBeGreaterThan(kicad);
    expect(handler.indexOf('statusText.textContent', note)).toBeGreaterThan(note);
    expect(note).toBeLessThan(handler.indexOf("'desktop:content-response'", kicad));
  });
});
