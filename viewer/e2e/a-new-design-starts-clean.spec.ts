import { test, expect, type Page, type Route } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * A design put on screen starts with nothing of the one before it.
 *
 * The page keeps a handful of fields about the design it shows, and each path
 * that shows another one - the file picker, a dropped file, a template, a
 * recent card, New blank - reset some of them and not others. What survived
 * did harm: the reader, so a new board after a KiCad board went to the KiCad
 * reader on its first edit and emptied; the file handle, so Ctrl+S wrote a new
 * board over the file opened before it; a routing run, whose copper was merged
 * into the design that replaced the one it routed; the selection, whose trace
 * ids the next design reuses; and a library or a footprint that arrived after
 * its design had gone, which put that design back.
 *
 * The desktop app's open and new file cannot run here; the unit test
 * `a-design-is-read-by-the-reader-its-name-asks-for` holds those.
 */

const KICAD = fs.readFileSync(path.resolve(__dirname, '../../tests/fixtures/benchmark/plane_board.kicad_pcb'), 'utf-8');

function board(name: string, x2: number): string {
  return `version 1

board ${name} {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    at 8mm, 10mm
}

component R2 resistor "0805" {
    at ${x2}mm, 10mm
}

net SIG {
    R1.1
    R2.1
}
`;
}

function withTrace(name: string, x2: number): string {
  return `${board(name, x2)}
trace SIG {
    from R1.1
    to R2.1
    layer Top
    width 0.3mm
}
`;
}

type Written = { to?: string; picker?: string };

/** A save that asked where to write, and wrote there: nothing went to the file opened before. */
const savedAsNew = (name: string): Written[] => [{ picker: name }, { to: name }];

/**
 * A file system the page can open from and save to, and that writes down
 * where every save went. `__nextOpen` is the file the next Open hands over.
 */
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
        write: async () => {
          w.__writes.push({ to: name });
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

async function ready(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
}

async function showProjectManager(page: Page): Promise<void> {
  await page.evaluate(() => (document.querySelector('#open-btn') as HTMLElement).click());
}

async function openThroughPicker(page: Page, name: string, text: string): Promise<void> {
  await page.evaluate(({ name, text }) => {
    (window as never as { __nextOpen: unknown }).__nextOpen = { name, text };
  }, { name, text });
  await showProjectManager(page);
  await page.locator('#pm-open-btn').click();
  await expect(page.locator('#status-text')).toContainText(`Loaded ${name}`, { timeout: 10_000 });
}

async function dropFile(page: Page, name: string, text: string): Promise<void> {
  await page.evaluate(({ name, text }) => {
    const files = new DataTransfer();
    files.items.add(new File([text], name));
    document.getElementById('canvas-container')!.dispatchEvent(
      new DragEvent('drop', { dataTransfer: files, bubbles: true, cancelable: true }),
    );
  }, { name, text });
  await expect(page.locator('#status-text')).toContainText(`Loaded ${name}`, { timeout: 10_000 });
}

async function newBlank(page: Page): Promise<void> {
  await showProjectManager(page);
  await page.locator('[data-template-blank]').click();
}

const loadedKind = (page: Page) => page.evaluate(() => (window as never as { __loadedKind(): string }).__loadedKind());

/** Press Ctrl+S and say where the save went. */
async function save(page: Page): Promise<Written[]> {
  await page.evaluate(() => {
    (window as never as { __writes: Written[] }).__writes = [];
  });
  await page.locator('#pcb-canvas').focus().catch(() => {});
  await page.keyboard.press('Control+s');
  await expect.poll(() => page.evaluate(() => (window as never as { __writes: Written[] }).__writes.length)).toBeGreaterThan(0);
  return page.evaluate(() => (window as never as { __writes: Written[] }).__writes);
}

type Seen = { board: string | null; components: string[]; traces: number[] };
function seen(page: Page): Promise<Seen> {
  return page.evaluate(() => {
    const s = (window as never as {
      __pcbEngine: { get_snapshot(): { board?: { name?: string }; components: { refdes: string }[]; traces: { id: number }[] } };
    }).__pcbEngine.get_snapshot();
    return { board: s.board?.name ?? null, components: s.components.map((c) => c.refdes), traces: s.traces.map((t) => t.id) };
  });
}

async function editorText(page: Page): Promise<string> {
  await page.waitForFunction(() => Boolean((window as never as { __editor?: unknown }).__editor), undefined, { timeout: 10_000 });
  return page.evaluate(() => (window as never as { __editor: { getValue(): string } }).__editor.getValue());
}

/** Hold every request to `pattern` until the returned function is called. */
async function holdRequests(page: Page, pattern: string, answer: (route: Route) => Promise<void>) {
  let release: () => void = () => {};
  const released = new Promise<void>((resolve) => {
    release = resolve;
  });
  let asked = 0;
  await page.route(pattern, async (route) => {
    asked += 1;
    await released;
    await answer(route);
  });
  return { release, asked: () => asked };
}

test.describe('A new design starts clean', () => {
  test.beforeEach(async ({ page }) => {
    await fakeFileSystem(page);
    await ready(page);
    // Every path below starts from the case that found this: a KiCad board,
    // opened from a file, so the page holds both a reader and a handle.
    await openThroughPicker(page, 'plane.kicad_pcb', KICAD);
    expect(await loadedKind(page), 'the KiCad board is read as one').toBe('kicad_pcb');
  });

  test('New blank after a KiCad board is a .cypcb board, edits and all, and saves as a new file', async ({ page }) => {
    await newBlank(page);
    expect(await loadedKind(page)).toBe('cypcb');

    // What a person saw: the first edit emptied the board.
    const text = `${await editorText(page)}\ncomponent R1 resistor "0805" {\n    at 5mm, 5mm\n}\n`;
    await page.evaluate((src) => (window as never as { __editor: { setValue(s: string): void } }).__editor.setValue(src), text);
    await expect.poll(async () => (await seen(page)).components, { timeout: 10_000 }).toEqual(['R1']);

    expect(await save(page)).toEqual(savedAsNew('design.cypcb'));
  });

  test('a template after a KiCad board is a .cypcb board and saves as a new file', async ({ page }) => {
    await showProjectManager(page);
    await page.locator('[data-template="blink"]').click();
    await expect(page.locator('#status-text')).toContainText('Loaded template', { timeout: 10_000 });
    expect(await loadedKind(page)).toBe('cypcb');
    expect(await save(page)).toEqual(savedAsNew('Blink LED.cypcb'));
  });

  test('a recent .cypcb after a KiCad board is a .cypcb board and saves as a new file', async ({ page }) => {
    // A recent card to click: a template goes on the list when it opens.
    await showProjectManager(page);
    await page.locator('[data-template="blink"]').click();
    await expect(page.locator('#status-text')).toContainText('Loaded template', { timeout: 10_000 });
    await openThroughPicker(page, 'plane.kicad_pcb', KICAD);
    expect(await loadedKind(page)).toBe('kicad_pcb');

    await showProjectManager(page);
    await page.locator('.pm-recent-item', { hasText: 'Blink LED' }).first().click();
    await expect(page.locator('#status-text')).toContainText('Loaded: Blink LED.cypcb');
    expect(await loadedKind(page)).toBe('cypcb');
    expect(await save(page)).toEqual(savedAsNew('Blink LED.cypcb'));
  });

  test('a dropped .cypcb after a KiCad board is a .cypcb board and saves as a new file', async ({ page }) => {
    await dropFile(page, 'dropped.cypcb', board('dropped', 22));
    expect(await loadedKind(page)).toBe('cypcb');
    expect(await save(page)).toEqual(savedAsNew('dropped.cypcb'));
  });

  test('a file opened after a KiCad board saves to itself, and is given no other design\'s library', async ({ page }) => {
    // The library a template imports, fetched and held by the page.
    const template = await (await page.request.get('/templates/sensor-front-end.cypcb')).text();
    await showProjectManager(page);
    await page.locator('[data-template="sensor-front-end"]').click();
    // The library's parts are the ones named after its blocks.
    const fromLibrary = async () => (await seen(page)).components.filter((refdes) => refdes.includes('DIV'));
    await expect.poll(fromLibrary, { timeout: 15_000 }).not.toEqual([]);

    // The same text from a file: a file the picker opened has no directory
    // to read `lib/blocks.cypcb` from, and the engine says so.
    await openThroughPicker(page, 'mine.cypcb', template);
    expect(await loadedKind(page)).toBe('cypcb');
    expect(await fromLibrary(), 'the file drew the template\'s library').toEqual([]);
    expect(await save(page)).toEqual([{ to: 'mine.cypcb' }]);
  });
});

test.describe('Nothing of the last design reaches the next one', () => {
  test.beforeEach(async ({ page }) => {
    await fakeFileSystem(page);
    await ready(page);
  });

  test('a trace selected on the last design is not deleted from the next', async ({ page }) => {
    await dropFile(page, 'first.cypcb', withTrace('first', 22));
    const first = await seen(page);
    expect(first.traces).toHaveLength(1);

    await selectTheTrace(page, first.traces[0]);

    // The next design numbers its trace the same.
    await dropFile(page, 'second.cypcb', withTrace('second', 20));
    expect((await seen(page)).traces, 'the control: the id is reused').toEqual(first.traces);

    await page.keyboard.press('Delete');
    await page.waitForTimeout(300);
    expect((await seen(page)).traces, 'Delete removed a trace nobody picked').toEqual(first.traces);
  });

  test('undo does not reach into the next design', async ({ page }) => {
    await dropFile(page, 'first.cypcb', withTrace('first', 22));
    const first = await seen(page);
    await selectTheTrace(page, first.traces[0]);
    await page.keyboard.press('Delete');
    await expect.poll(async () => (await seen(page)).traces, { timeout: 5_000 }).toEqual([]);

    await dropFile(page, 'second.cypcb', withTrace('second', 20));
    const second = await seen(page);
    await page.keyboard.press('Control+z');
    await page.waitForTimeout(300);
    expect((await seen(page)).traces, 'undo put the last design\'s trace on this one').toEqual(second.traces);
  });
});

/** Click the middle of the only trace, the way a person selects it. */
async function selectTheTrace(page: Page, id: number): Promise<void> {
  const at = await page.evaluate(() => {
    const s = (window as never as { __pcbEngine: { get_snapshot(): { traces: { segments: { start_x: number; start_y: number; end_x: number; end_y: number }[] }[] } } }).__pcbEngine.get_snapshot();
    const seg = s.traces[0].segments[0];
    const x = (seg.start_x + seg.end_x) / 2;
    const y = (seg.start_y + seg.end_y) / 2;
    const vp = (window as never as { __viewport: { centerX: number; centerY: number; scale: number; width: number; height: number } }).__viewport;
    const rect = document.getElementById('pcb-canvas')!.getBoundingClientRect();
    return { x: rect.left + (x - vp.centerX) * vp.scale + vp.width / 2, y: rect.top + vp.height / 2 - (y - vp.centerY) * vp.scale };
  });
  await page.mouse.click(at.x, at.y);
  await expect
    .poll(() => page.evaluate(() => (window as never as { __renderState: { selectedTraceId: number | null } }).__renderState.selectedTraceId))
    .toBe(id);
}

test.describe('Nothing of the last design reaches the next one, in flight', () => {
  test.beforeEach(async ({ page }) => {
    await fakeFileSystem(page);
    await ready(page);
  });

  test('a routing run does not land on the design that replaced the one it routed', async ({ page }) => {
    test.setTimeout(120_000);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), board('routed', 22));
    // Started and replaced in one go: a two-part board routes in well under
    // the time a test takes to click twice.
    const running = await page.evaluate(() => {
      (window as never as { __triggerRouting(): void }).__triggerRouting();
      const active = (window as never as { __routingWorker: { active: boolean } }).__routingWorker.active;
      (document.querySelector('#open-btn') as HTMLElement).click();
      (document.querySelector('[data-template-blank]') as HTMLElement).click();
      return active;
    });
    expect(running, 'the control: the run is in flight when the design changes').toBe(true);

    await page.waitForFunction(() => !(window as never as { __routingWorker: { active: boolean } }).__routingWorker.active);
    await page.waitForTimeout(3_000);
    expect(await seen(page)).toEqual({ board: 'untitled', components: [], traces: [] });
    expect(await editorText(page)).not.toContain('trace SIG');
  });

  test('a tuning run does not land on the design that replaced the one it tuned', async ({ page }) => {
    test.setTimeout(120_000);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), board('tuned', 22));
    await page.evaluate(() => {
      const slider = document.querySelector<HTMLInputElement>('#tune-via-cost')!;
      slider.value = String(Number(slider.value) + Number(slider.step || 1));
      slider.dispatchEvent(new Event('input'));
    });
    // The run starts after a pause, so the design is replaced from inside the
    // wait, on the first frame it is in flight. That it was in flight is the
    // control. The next design has the same net, so copper tuned for the last
    // one has somewhere to land.
    await page.waitForFunction((text) => {
      if (!(window as never as { __tuningWorker: { active: boolean } }).__tuningWorker.active) return false;
      const files = new DataTransfer();
      files.items.add(new File([text], 'next.cypcb'));
      document.getElementById('canvas-container')!.dispatchEvent(
        new DragEvent('drop', { dataTransfer: files, bubbles: true, cancelable: true }),
      );
      return true;
    }, board('next', 20));
    await expect(page.locator('#status-text')).toContainText('Loaded next.cypcb');

    await page.waitForTimeout(5_000);
    expect(await seen(page)).toEqual({ board: 'next', components: ['R1', 'R2'], traces: [] });
    await expect(page.locator('#status-text')).not.toContainText('Tuned');
  });

  test('a library that arrives after its design has gone does not bring it back', async ({ page }) => {
    const held = await holdRequests(page, '**/templates/lib/blocks.cypcb', (route) => route.continue());
    await showProjectManager(page);
    await page.locator('[data-template="sensor-front-end"]').click();
    await expect.poll(held.asked, { timeout: 10_000 }).toBeGreaterThan(0);

    await newBlank(page);
    held.release();
    await page.waitForTimeout(2_000);
    expect(await seen(page)).toEqual({ board: 'untitled', components: [], traces: [] });
  });

  test('a footprint that arrives after its design has gone does not bring it back', async ({ page }) => {
    const held = await holdRequests(page, '**/easyeda-api/**', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        headers: { 'Access-Control-Allow-Origin': '*' },
        body: JSON.stringify({
          result: { packageDetail: { dataStr: { head: { x: 4000, y: 3000 }, shape: ['PAD~RECT~3960~3000~40~50~1~~1~0~', 'PAD~RECT~4040~3000~40~50~1~~2~0~'] } } },
        }),
      }),
    );
    await editorText(page);
    await page.evaluate((src) => (window as never as { __editor: { setValue(s: string): void } }).__editor.setValue(src), `version 1

board fetched {
    size 30mm x 20mm
    layers 2
}

component U1 ic "ARRIVES_LATE" {
    lcsc "C999061"
    at 15mm, 10mm
}
`);
    await expect.poll(held.asked, { timeout: 10_000 }).toBeGreaterThan(0);

    await newBlank(page);
    held.release();
    await page.waitForTimeout(2_000);
    expect(await seen(page)).toEqual({ board: 'untitled', components: [], traces: [] });
  });
});
