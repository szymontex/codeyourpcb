import { test, expect, type Page } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * The routing worker routes the design the page shows, not a part of it.
 *
 * The worker builds its own engine, and it used to build it from the design
 * text alone: no footprint the host had fetched from a supplier, no file the
 * design imports. A part drawn with either was not on the board it routed,
 * and the error the engine gave for it - a plain string - was read as JSON,
 * failed to parse and was taken for success. So the route, the tuning run
 * and the debug run each answered about a smaller board and said nothing.
 *
 * Each check below reads what the user sees afterwards: the checker in the
 * page, which knows the whole design, finding pins no copper reaches.
 */

const HOST_PART = 'HOST_FETCHED_2PIN';

/** Two top-side pads 2mm apart, in the shape `easyeda-footprint-parser.ts` hands over. */
const HOST_PADS = [
  { number: '1', shape: 'rect', x_nm: -1_000_000, y_nm: 0, width_nm: 800_000, height_nm: 800_000, layer_mask: 1, drill_nm: null },
  { number: '2', shape: 'rect', x_nm: 1_000_000, y_nm: 0, width_nm: 800_000, height_nm: 800_000, layer_mask: 1, drill_nm: null },
];

const HOST_BOARD = `version 1

board host_part {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 8mm, 10mm
}

component U1 ic "${HOST_PART}" {
    value "fetched"
    at 22mm, 10mm
}

net SIG_A {
    R1.1
    U1.1
}

net SIG_B {
    R1.2
    U1.2
}
`;

interface Violation {
  message?: string;
}

async function ready(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
}

/** Register a footprint the way the supplier fetch does. */
async function registerFromHost(page: Page): Promise<void> {
  await page.evaluate(
    async ({ name, pads }) => {
      const wasm = await import('/src/wasm.ts' as string);
      wasm.registerDynamicFootprint(name, pads, []);
    },
    { name: HOST_PART, pads: HOST_PADS },
  );
}

/** The pins the page's own checker finds no copper on. */
async function barePins(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const snapshot = (window as never as {
      __pcbEngine: { get_snapshot(): { violations?: Violation[] } };
    }).__pcbEngine.get_snapshot();
    return (snapshot.violations ?? [])
      .map((v) => v.message ?? '')
      .filter((m) => m.includes('no copper reaches'));
  });
}

async function route(page: Page): Promise<string> {
  await page.evaluate(() => {
    (window as never as { __triggerRouting: () => void }).__triggerRouting();
  });
  await page.waitForFunction(
    () => {
      const worker = (window as never as {
        __routingWorker: { active: boolean };
      }).__routingWorker;
      const status = document.querySelector('#status-text')?.textContent ?? '';
      return !worker.active && /Routed|Routing failed/.test(status);
    },
    undefined,
    { timeout: 120_000 },
  );
  return (await page.locator('#status-text').textContent()) ?? '';
}

test.describe('The routing worker', () => {
  // A route runs for seconds, and the default is thirty for the whole test.
  test.beforeEach(() => test.setTimeout(180_000));

  test('routes a part whose footprint the host fetched', async ({ page }) => {
    await ready(page);
    await registerFromHost(page);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), HOST_BOARD);

    // The control: the page knows the fetched part, so its bare pins are
    // visible to the checker. Without it an empty list below proves nothing.
    const before = await barePins(page);
    expect(before.some((m) => m.includes('U1.1')), before.join('\n')).toBe(true);

    const status = await route(page);
    expect(status, 'the route ran').toContain('Routed');
    const after = await barePins(page);
    expect(after, `the worker left the fetched part bare; status: ${status}`).toEqual([]);
  });

  test('the debug run counts the fetched part', async ({ page }) => {
    await ready(page);
    await registerFromHost(page);
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), HOST_BOARD);

    await page.evaluate(() => {
      (window as never as { __triggerDebugRouting: () => void }).__triggerDebugRouting();
    });
    await page.waitForFunction(
      () => {
        const worker = (window as never as {
          __debugWorker: { active: boolean; lastResult: string | null };
        }).__debugWorker;
        const status = document.querySelector('#status-text')?.textContent ?? '';
        return !worker.active && (worker.lastResult !== null || status.includes('failed'));
      },
      undefined,
      { timeout: 120_000 },
    );
    const result = await page.evaluate(
      () => (window as never as { __debugWorker: { lastResult: string | null } }).__debugWorker.lastResult,
    );
    const report = JSON.parse(result ?? '{}') as { net_count?: number; unrouted_count?: number };
    // It saw the part: without it neither net has two pins to join, and a run
    // over no nets would pass the line below just the same.
    expect(report.net_count, result ?? '').toBe(2);
    expect(report.unrouted_count, 'the fetched part routes like any other').toBe(0);
  });

  test('routes the parts a template imports', async ({ page }) => {
    await ready(page);
    await page.locator('[data-template="sensor-front-end"]').click();
    await page.waitForFunction(
      () => {
        const snapshot = (window as never as {
          __pcbEngine?: { get_snapshot(): { components?: unknown[] } };
        }).__pcbEngine?.get_snapshot();
        return (snapshot?.components?.length ?? 0) >= 6;
      },
      undefined,
      { timeout: 15_000 },
    );

    const before = await barePins(page);
    expect(before.some((m) => m.includes('DIV_A_')), before.join('\n')).toBe(true);

    const status = await route(page);
    expect(status, 'the route ran').toContain('Routed');
    const after = await barePins(page);
    expect(
      after.filter((m) => m.includes('DIV_')),
      `the worker left the imported parts bare; status: ${status}`,
    ).toEqual([]);
  });

  test('a design the engine cannot load fails the route out loud', async ({ page }) => {
    await ready(page);
    // Nothing fetched this one: the page reports it, and the worker has to
    // say the same rather than route the board without it.
    const board = HOST_BOARD.replace(HOST_PART, 'NEVER_FETCHED_PART');
    await page.evaluate((src) => (window as never as { __loadBoard(s: string): void }).__loadBoard(src), board);

    const status = await route(page);
    expect(status).toContain('Routing failed');
    expect(status).toContain('NEVER_FETCHED_PART');
  });

  test('a KiCad board is not sent to the worker', async ({ page }) => {
    await ready(page);
    // The routed copper comes back as DSL and is merged into the text; merged
    // into a KiCad board's text it would be neither format.
    const board = fs.readFileSync(
      path.resolve(__dirname, '../../tests/fixtures/benchmark/plane_board.kicad_pcb'),
      'utf-8',
    );
    await page.evaluate(
      (src) => (window as never as { __loadBoard(s: string, kind: string): void }).__loadBoard(src, 'kicad_pcb'),
      board,
    );
    await page.evaluate(() => {
      (window as never as { __triggerRouting: () => void }).__triggerRouting();
    });
    await expect(page.locator('#status-text')).toContainText('Routing needs the design source');
    const active = await page.evaluate(
      () => (window as never as { __routingWorker: { active: boolean } }).__routingWorker.active,
    );
    expect(active).toBe(false);
  });
});
