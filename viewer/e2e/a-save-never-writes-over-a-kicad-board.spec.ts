import { test, expect, type Page } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * A save never writes over a KiCad board.
 *
 * Ctrl+S spliced the copper, as trace blocks of this language, onto the end
 * of the KiCad text and wrote that over the `.kicad_pcb` it came from. The
 * reader stops at the board's closing bracket, so the file still opened -
 * without the copper, and without a word. The Open button put the same
 * spliced text on the recent list, and the board reopened from there without
 * it too. Writing the board back as KiCad would lose what the importer does
 * not carry, so a KiCad board is saved as the design `from-kicad` writes,
 * beside it, and the board stays as it was.
 */

const NAME = 'led_blink.kicad_pcb';
const KICAD = fs.readFileSync(path.resolve(__dirname, '../../tests/fixtures/benchmark/led_blink.kicad_pcb'), 'utf-8');

type Written = { to?: string; picker?: string; text?: string };

async function fakeFileSystem(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as never as {
      __writes: Written[];
      __nextOpen: { name: string; text: string };
      showOpenFilePicker: unknown;
      showSaveFilePicker: unknown;
    };
    w.__writes = [];
    const handle = (name: string, text: string) => ({
      kind: 'file',
      name,
      getFile: async () => new File([text], name),
      createWritable: async () => ({
        write: async (data: string) => {
          w.__writes.push({ to: name, text: String(data) });
        },
        close: async () => {},
      }),
    });
    w.showOpenFilePicker = async () => [handle(w.__nextOpen.name, w.__nextOpen.text)];
    w.showSaveFilePicker = async (options: { suggestedName?: string }) => {
      w.__writes.push({ picker: options?.suggestedName });
      return handle(options?.suggestedName ?? 'new', '');
    };
  });
}

async function openKicadBoard(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  await page.evaluate((text) => {
    (window as never as { __nextOpen: unknown }).__nextOpen = { name: 'led_blink.kicad_pcb', text };
  }, KICAD);
  await page.evaluate(() => (document.querySelector('#open-btn') as HTMLElement).click());
  await page.locator('#pm-open-btn').click();
  await expect(page.locator('#status-text')).toContainText(`Loaded ${NAME}`, { timeout: 10_000 });
}

/** Draw a trace on the net the board's own trace is on, beside it. */
async function drawCopper(page: Page): Promise<void> {
  const added = await page.evaluate(() => {
    const engine = (window as any).__pcbEngine;
    const [trace] = engine.get_snapshot().traces;
    const s = trace.segments[0];
    const x = Number(s.start_x);
    const y = Number(s.start_y) + 1_000_000;
    return engine.add_trace(trace.net_name, 'Top', 250_000, [x, y, x + 2_000_000, y]);
  });
  expect(added, 'the engine refused the trace').not.toBe(4294967295);
}

/** The board as the pads, nets and copper it holds - never the ids. */
function fingerprint(page: Page) {
  return page.evaluate(() => {
    const s = (window as any).__pcbEngine.get_snapshot();
    const n = (v: unknown) => Math.round(Number(v));
    return {
      pads: s.components.flatMap((c: any) => c.pads.map((p: any) => `${c.refdes}.${p.number}@${n(p.x_nm)},${n(p.y_nm)}`)).sort(),
      nets: s.nets.map((net: any) => `${net.name}:${net.connections.map((c: unknown) => JSON.stringify(c)).sort()}`).sort(),
      copper: s.traces.flatMap((t: any) => t.segments.map((g: any) =>
        `${t.net_name}|${t.layer}|${n(t.width)}|${n(g.start_x)},${n(g.start_y)}-${n(g.end_x)},${n(g.end_y)}`)).sort(),
      vias: (s.vias ?? []).map((v: any) => `${v.net_name}@${n(v.x)},${n(v.y)}`).sort(),
    };
  });
}

async function save(page: Page): Promise<Written[]> {
  await page.evaluate(() => {
    (window as never as { __writes: Written[] }).__writes = [];
  });
  await page.locator('#pcb-canvas').focus().catch(() => {});
  await page.keyboard.press('Control+s');
  await expect.poll(() => page.evaluate(() => (window as never as { __writes: Written[] }).__writes.length)).toBeGreaterThan(0);
  return page.evaluate(() => (window as never as { __writes: Written[] }).__writes);
}

function recent(page: Page): Promise<{ name: string; source: string }[]> {
  return page.evaluate(() => JSON.parse(localStorage.getItem('cypcb-settings') ?? '{}').recentFiles ?? []);
}

test.describe('a save never writes over a KiCad board', () => {
  test.beforeEach(async ({ page }) => {
    await fakeFileSystem(page);
    await openKicadBoard(page);
    await drawCopper(page);
  });

  test('Ctrl+S writes a .cypcb beside the board and nothing to the board', async ({ page }) => {
    const writes = await save(page);
    expect(writes.map(({ to, picker }) => ({ to, picker }))).toEqual([
      { picker: 'led_blink.cypcb', to: undefined },
      { picker: undefined, to: 'led_blink.cypcb' },
    ]);
    await expect(page.locator('#status-text')).toContainText(`${NAME} untouched`);
  });

  test('the saved design reads back with the same pads, nets and copper', async ({ page }) => {
    const before = await fingerprint(page);
    expect(before.copper.length, 'the control: the board has its own trace and the drawn one').toBeGreaterThanOrEqual(2);
    const writes = await save(page);
    const design = writes.find((w) => w.to === 'led_blink.cypcb')!.text!;
    expect(design.trimStart().startsWith('(kicad_pcb'), 'KiCad text went into the .cypcb').toBe(false);

    const errors = await page.evaluate((text) => (window as any).__pcbEngine.load_source(text), design);
    expect(errors, 'the saved design does not parse').toBe('');
    expect(await fingerprint(page)).toEqual(before);
  });

  test('after the save the design is the .cypcb, and Ctrl+S writes to it', async ({ page }) => {
    await save(page);
    expect(await page.evaluate(() => (window as any).__loadedKind())).toBe('cypcb');
    const again = await save(page);
    expect(again.map(({ to, picker }) => ({ to, picker }))).toEqual([{ to: 'led_blink.cypcb', picker: undefined }]);
  });

  test('copper drawn on a KiCad board comes back from the recent list', async ({ page }) => {
    const before = await fingerprint(page);
    await page.evaluate(() => (document.querySelector('#open-btn') as HTMLElement).click());

    const list = await recent(page);
    // The board's own entry is the board as it was: nothing spliced on.
    expect(list.find((e) => e.name === NAME)?.source).toBe(KICAD);

    await page.locator('.pm-recent-item', { has: page.locator('img[alt="led_blink.cypcb"]') }).click();
    await expect(page.locator('#status-text')).toContainText('led_blink.cypcb');
    expect(await fingerprint(page)).toEqual(before);
  });
});

test('a KiCad board with its copper unchanged adds nothing to the recent list', async ({ page }) => {
  // The control for the test above: the list is not given a design for every
  // KiCad board opened, only for one whose copper changed here.
  await fakeFileSystem(page);
  await openKicadBoard(page);
  await page.evaluate(() => (document.querySelector('#open-btn') as HTMLElement).click());
  expect((await recent(page)).map((e) => e.name)).toEqual([NAME]);
});
