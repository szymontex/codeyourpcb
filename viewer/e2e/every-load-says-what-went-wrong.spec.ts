import { test, expect, type Page, type Route } from '@playwright/test';

/**
 * Every load of a design says what went wrong, where a person can read it.
 *
 * `loadDesign` returns the engine's messages, and six places dropped them: the
 * reload after a supplier fetch, a new blank board, the test hook, the copper
 * a route or a tuning run brought back, and a new file in the desktop app. The
 * status line then said "Routed 4 segments" over a board that had not loaded.
 * Parse errors also reach the editor through `get_diagnostics_json`, with their
 * line; a footprint the engine refused has no line, and reaches nobody unless
 * the status line carries it.
 *
 * The desktop app's new file is not here: its handler is registered only
 * inside Tauri, and an empty design has nothing to fail but that same refusal.
 */

/** Pads with no position: the engine refuses them, the way it refuses any it cannot read. */
const UNREADABLE_PADS = [{ number: '1' }];

const TWO_RESISTORS = `version 1

board late {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    at 8mm, 10mm
}

component R2 resistor "0805" {
    at 22mm, 10mm
}

net SIG {
    R1.1
    R2.1
}
`;

/** A part the supplier fetch fills in, named by its package. */
function boardWithSupplierPart(pkg: string, lcsc: string): string {
  return `version 1

board fetched {
    size 30mm x 20mm
    layers 2
}

component U1 ic "${pkg}" {
    lcsc "${lcsc}"
    at 15mm, 10mm
}
`;
}

/** Two pads in EasyEDA's own field order, as `jlcpcb-search.spec.ts` serves them. */
const SUPPLIER_ANSWER = {
  result: {
    packageDetail: {
      dataStr: {
        head: { x: 4000, y: 3000 },
        shape: ['PAD~RECT~3960~3000~40~50~1~~1~0~', 'PAD~RECT~4040~3000~40~50~1~~2~0~'],
      },
    },
  },
};

async function ready(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
}

async function status(page: Page): Promise<string> {
  return (await page.locator('#status-text').textContent()) ?? '';
}

/** Hand the engine a footprint it will refuse, the way the supplier fetch hands one over. */
async function refuseFootprint(page: Page, name: string): Promise<void> {
  await page.evaluate(
    async ({ name, pads }) => {
      const wasm = await import('/src/wasm.ts' as string);
      wasm.registerDynamicFootprint(name, pads, []);
    },
    { name, pads: UNREADABLE_PADS },
  );
}

/**
 * Refuse a footprint the moment the next worker request has gone out.
 *
 * The worker then loads a design that is fine, and only the page's own load
 * of the copper it sends back meets the refusal - which is the load this is
 * about. Nothing outside can make the merge itself produce text that does not
 * read back, so this is how that load is made to say something.
 */
async function refuseAfterNextWorkerRequest(page: Page, name: string): Promise<void> {
  await page.evaluate(
    async ({ name, pads }) => {
      const wasm = await import('/src/wasm.ts' as string);
      const post = Worker.prototype.postMessage;
      Worker.prototype.postMessage = function (this: Worker, ...args: Parameters<Worker['postMessage']>) {
        post.apply(this, args);
        Worker.prototype.postMessage = post;
        wasm.registerDynamicFootprint(name, pads, []);
      } as Worker['postMessage'];
    },
    { name, pads: UNREADABLE_PADS },
  );
}

async function editorSays(page: Page, source: string): Promise<void> {
  await page.waitForFunction(() => Boolean((window as never as { __editor?: unknown }).__editor), undefined, {
    timeout: 10_000,
  });
  await page.evaluate((src) => (window as never as { __editor: { setValue(s: string): void } }).__editor.setValue(src), source);
}

/** The messages the editor underlines, as the engine gave them. */
async function editorMarkers(page: Page): Promise<string[]> {
  return page.evaluate(async () => {
    const panel = await import('/src/editor/editor-panel.ts' as string);
    const monaco = panel.getMonacoModule();
    return monaco ? monaco.editor.getModelMarkers({ owner: 'cypcb' }).map((m: { message: string }) => m.message) : [];
  });
}

test.describe('Every load says what went wrong', () => {
  // A route runs for seconds, and the default is thirty for the whole test.
  test.beforeEach(() => test.setTimeout(180_000));

  test('the test hook hands back what the engine said', async ({ page }) => {
    await ready(page);
    const said = await page.evaluate(
      (src) => (window as never as { __loadBoard(s: string): string }).__loadBoard(src),
      TWO_RESISTORS.replace('"0805" {\n    at 22mm', '"NO_SUCH_PACKAGE" {\n    at 22mm'),
    );
    expect(said).toContain("unknown footprint: 'NO_SUCH_PACKAGE'");
  });

  test('a footprint refused on the reload after a supplier fetch is on the status line', async ({ page }) => {
    await ready(page);
    await refuseFootprint(page, 'REFUSED_FETCHED');
    // Already registered, so the fetch has nothing to ask the supplier for and
    // goes straight to the reload.
    await editorSays(page, boardWithSupplierPart('REFUSED_FETCHED', 'C999001'));
    await expect(page.locator('#status-text')).toContainText('REFUSED_FETCHED', { timeout: 10_000 });
  });

  test('the reload after a supplier fetch takes back the error the missing footprint raised', async ({ page }) => {
    let answer: () => void = () => {};
    const answered = new Promise<void>((resolve) => {
      answer = resolve;
    });
    await page.route('**/easyeda-api/**', async (route: Route) => {
      await answered;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        headers: { 'Access-Control-Allow-Origin': '*' },
        body: JSON.stringify(SUPPLIER_ANSWER),
      });
    });
    await ready(page);
    await editorSays(page, boardWithSupplierPart('FETCHED_LATER', 'C999002'));

    // The control: before the supplier answers, the part is unknown and the
    // editor says so. Without it, an empty list below proves nothing.
    await expect
      .poll(() => editorMarkers(page), { timeout: 10_000 })
      .toContain("unknown footprint: 'FETCHED_LATER'");

    answer();
    await expect
      .poll(async () => (await page.evaluate(() => (window as never as {
        __pcbEngine: { get_snapshot(): { components: { refdes: string; pads: unknown[] }[] } };
      }).__pcbEngine.get_snapshot().components.find((c) => c.refdes === 'U1')?.pads.length ?? 0)), { timeout: 10_000 })
      .toBe(2);
    // The checker has things to say about the part now that it has pads; what
    // must be gone is the claim that it has none.
    await expect
      .poll(async () => (await editorMarkers(page)).filter((m) => m.includes('FETCHED_LATER')), { timeout: 10_000 })
      .toEqual([]);
  });

  test('a new blank board clears the last board\'s errors from the editor', async ({ page }) => {
    // Parse errors already reach the editor here, through the diagnostics the
    // engine keeps: this holds that, rather than saying it.
    await ready(page);
    await editorSays(page, boardWithSupplierPart('NOT_A_PACKAGE', 'C999003'));
    await expect.poll(() => editorMarkers(page), { timeout: 10_000 }).toContain("unknown footprint: 'NOT_A_PACKAGE'");

    await page.locator('[data-template-blank]').click();
    await expect.poll(() => editorMarkers(page), { timeout: 10_000 }).toEqual([]);
  });

  test('a new blank board says so when a fetched footprint was refused', async ({ page }) => {
    await ready(page);
    await refuseFootprint(page, 'REFUSED_BLANK');
    await page.locator('[data-template-blank]').click();
    await expect(page.locator('#status-text')).toContainText('REFUSED_BLANK', { timeout: 10_000 });
  });

  test('routed copper that does not load cleanly says so', async ({ page }) => {
    await ready(page);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), TWO_RESISTORS);
    await refuseAfterNextWorkerRequest(page, 'REFUSED_ROUTED');
    await page.evaluate(() => (window as never as { __triggerRouting: () => void }).__triggerRouting());
    await page.waitForFunction(
      () => /Routed|Routing failed/.test(document.querySelector('#status-text')?.textContent ?? ''),
      undefined,
      { timeout: 120_000 },
    );
    const said = await status(page);
    expect(said, 'the worker routed the board').toContain('Routed');
    expect(said).toContain('REFUSED_ROUTED');
  });

  test('tuned copper that does not load cleanly says so', async ({ page }) => {
    await ready(page);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), TWO_RESISTORS);
    await refuseAfterNextWorkerRequest(page, 'REFUSED_TUNED');
    await page.evaluate(() => {
      const slider = document.querySelector<HTMLInputElement>('#tune-via-cost')!;
      slider.value = String(Number(slider.value) + Number(slider.step || 1));
      slider.dispatchEvent(new Event('input'));
    });
    await page.waitForFunction(
      () => /Tuned|Tuning failed/.test(document.querySelector('#status-text')?.textContent ?? ''),
      undefined,
      { timeout: 120_000 },
    );
    const said = await status(page);
    expect(said, 'the worker tuned the board').toContain('Tuned');
    expect(said).toContain('REFUSED_TUNED');
  });
});
