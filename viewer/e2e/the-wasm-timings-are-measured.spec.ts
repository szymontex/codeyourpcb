import { test, expect, type Page } from '@playwright/test';
import { execFileSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

/**
 * The browser half of the timings and memory in `docs/api/lsp-server.md`.
 *
 * The native half is `the_editor_timings_are_measured` in cypcb-render. This
 * runs the same boards through the WASM engine the page loads, a fresh
 * `PcbEngine` from `pkg/`, and prints each row in the form the page gives it:
 *
 *   cd viewer && npx playwright test e2e/the-wasm-timings-are-measured.spec.ts --workers=1
 *
 * The times and the megabytes are a measurement with a date on the page, so
 * nothing here holds them. What is held is what does not move with the
 * machine: every board in the table loads in the browser with the parts the
 * table states, and the page builds the editor without being asked.
 */

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const RUNS = 20;
const PAGES = 5;

function performanceSection(): string {
  const page = fs.readFileSync(path.join(ROOT, 'docs/api/lsp-server.md'), 'utf-8');
  const start = page.indexOf('## Performance Characteristics\n');
  expect(start, 'the page has a performance section').toBeGreaterThan(-1);
  const end = page.indexOf('\n## ', start + 3);
  return page.slice(start, end === -1 ? undefined : end);
}

interface Row {
  board: string;
  parts: number;
}

function boardRows(): Row[] {
  return performanceSection()
    .split('\n')
    .filter((line) => line.startsWith('| `'))
    .map((line) => {
      const cells = line.split('|').map((cell) => cell.trim());
      return { board: cells[1].replace(/`/g, ''), parts: Number(cells[3]) };
    });
}

/** What `examples/lib` holds, keyed as an example imports it. */
function libraryJson(): string {
  const listed = execFileSync('git', ['ls-files', 'examples/lib'], { cwd: ROOT, encoding: 'utf-8' });
  const files: Record<string, string> = {};
  for (const file of listed.split('\n').filter((name) => path.dirname(name) === 'examples/lib')) {
    files[`lib/${path.basename(file)}`] = fs.readFileSync(path.join(ROOT, file), 'utf-8');
  }
  return JSON.stringify(files);
}

async function ready(page: Page): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
}

const cell = ([median, max]: number[]) => `${median.toFixed(2)} / ${max.toFixed(2)}`;
const mb = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);

function spread(values: number[]): number[] {
  const sorted = [...values].sort((a, b) => a - b);
  return [sorted[Math.floor(sorted.length / 2)], sorted[sorted.length - 1]];
}

test.describe('the WASM timings on the page are measured', () => {
  test.setTimeout(180_000);

  test('every board in the table loads in the browser with the parts it states', async ({ page }) => {
    const rows = boardRows();
    expect(rows.length, 'the table is being read').toBeGreaterThanOrEqual(9);
    const library = libraryJson();
    await ready(page);

    const loaded: string[] = [];
    for (const row of rows) {
      const source = fs.readFileSync(path.join(ROOT, row.board), 'utf-8');
      const kicad = row.board.endsWith('.kicad_pcb');
      const measured = await page.evaluate(
        async ({ source, kicad, library, runs }) => {
          const url = '/pkg/cypcb_render.js';
          const wasm = await import(/* @vite-ignore */ url);
          await wasm.default();
          const load = (engine: any) =>
            kicad ? engine.load_kicad(source) : engine.load_source_with_imports(source, library);
          const time = (work: () => void) => {
            work();
            const taken: number[] = [];
            for (let i = 0; i < runs; i++) {
              const started = performance.now();
              work();
              taken.push(performance.now() - started);
            }
            taken.sort((a, b) => a - b);
            return [taken[Math.floor(runs / 2)], taken[runs - 1]];
          };
          const engine = new wasm.PcbEngine();
          const said: string = load(engine);
          const parts: number = engine.get_snapshot()?.components?.length ?? 0;
          const drc = time(() => engine.run_drc_incremental());
          const fresh = new wasm.PcbEngine();
          const whole = time(() => load(fresh));
          engine.free();
          fresh.free();
          return { said, parts, drc, whole };
        },
        { source, kicad, library, runs: RUNS },
      );
      expect(measured.said, `${row.board} loads in the browser`).toBe('');
      loaded.push(`${row.board}: ${measured.parts} parts`);
      console.log(`| \`${row.board}\` | ${measured.parts} | ${cell(measured.drc)} | ${cell(measured.whole)} |`);
    }
    expect(loaded, 'the browser loads the parts the table states').toEqual(
      rows.map((row) => `${row.board}: ${row.parts} parts`),
    );
  });

  test('the editor loads in the background without being opened, and the memory is printed', async ({ page, context }) => {
    const board = 'tests/fixtures/benchmark/multi_ic.kicad_pcb';
    const source = fs.readFileSync(path.join(ROOT, board), 'utf-8');
    const samples: Record<string, number[]> = { ready: [], board: [], editor: [], wasmReady: [], wasmBoard: [] };

    // Monaco is held at the network until the board is measured, so the heap
    // before it and after it are two numbers rather than a race with the
    // page's idle preload. Routing turns the HTTP cache off, so every page
    // asks for it again.
    let release: () => void = () => {};
    await page.route(/\/monaco-editor\.js/, async (route) => {
      await new Promise<void>((resolve) => {
        release = resolve;
      });
      await route.continue();
    });

    for (let run = 0; run < PAGES; run++) {
      await ready(page);
      const cdp = await context.newCDPSession(page);
      const heap = async () => {
        await cdp.send('HeapProfiler.collectGarbage');
        return (await cdp.send('Runtime.getHeapUsage')).usedSize;
      };
      const wasmBytes = () =>
        page.evaluate(async () => {
          const url = '/pkg/cypcb_render.js';
          const wasm = await import(/* @vite-ignore */ url);
          return (await wasm.default()).memory.buffer.byteLength as number;
        });

      samples.ready.push(await heap());
      samples.wasmReady.push(await wasmBytes());
      await page.evaluate((text) => (window as any).__loadBoard(text, 'kicad_pcb'), source);
      samples.board.push(await heap());
      samples.wasmBoard.push(await wasmBytes());

      await expect(page.locator('.monaco-editor'), 'Monaco is held').toHaveCount(0);
      release();
      // Nothing opens the editor here: the page builds it on its own once
      // the browser is idle, so the panel opens at once when it is asked for.
      await expect(page.locator('.monaco-editor').first()).toBeAttached({ timeout: 15_000 });
      samples.editor.push(await heap());
      await cdp.detach();
    }

    const delta = (to: number[], from: number[]) => to.map((value, i) => value - from[i]);
    const row = (what: string, values: number[]) => {
      const [median, max] = spread(values);
      console.log(`| ${what} | ${mb(median)} / ${mb(max)} |`);
    };
    row('JS heap, page ready', samples.ready);
    row(`JS heap, \`${board}\` loaded`, samples.board);
    row('JS heap added by the editor', delta(samples.editor, samples.board));
    row('WASM memory, page ready', samples.wasmReady);
    row(`WASM memory, \`${board}\` loaded`, samples.wasmBoard);
  });
});
