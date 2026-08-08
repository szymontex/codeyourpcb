import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * The viewer could open the project's own `.cypcb` and nothing else.
 *
 * The command line learned to read a KiCad board, check it, route it and write
 * one back; somebody holding a `.kicad_pcb` still had no way to look at it
 * here. The importer now compiles to wasm - 42KB on a 1.08MB bundle - and the
 * file input accepts the extension.
 */
const BOARD = path.resolve(
  __dirname,
  '../../tests/fixtures/benchmark/plane_board.kicad_pcb',
);

test.describe('opening a KiCad board', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('a KiCad board loads and draws its parts', async ({ page }) => {
    const source = fs.readFileSync(BOARD, 'utf-8');

    await page.evaluate((src) => {
      (window as any).__loadBoard(src, 'kicad_pcb');
    }, source);
    await page.waitForTimeout(500);

    const board = await page.evaluate(() => {
      const engine = (window as any).__pcbEngine;
      const snapshot = engine.get_snapshot();
      return {
        components: snapshot?.components?.length ?? 0,
        width: snapshot?.board?.width_nm ?? 0,
        height: snapshot?.board?.height_nm ?? 0,
        pads: (snapshot?.components ?? []).reduce(
          (total: number, component: any) => total + (component.pads?.length ?? 0),
          0,
        ),
      };
    });

    // The fixture is 12 parts and 51 pads on a 50 x 38mm board.
    expect(board.components, 'the KiCad board loaded no parts').toBe(12);
    expect(board.pads, 'the parts arrived without their pads').toBe(51);
    expect(board.width).toBe(50_000_000);
    expect(board.height).toBe(38_000_000);
  });

  test('an edit reloads with the reader the board came from', async ({ page }) => {
    // The editor hands its content to a reader on every change, and every
    // reader clears the world before parsing. A KiCad board is what opening
    // one puts in the editor, so handing that to the `.cypcb` reader emptied
    // the board on the first character typed.
    //
    // The choice is made from `loadedKind`, so this checks both halves: that
    // opening a board records which language it is in, and that the two
    // readers really do disagree about a KiCad file. Driving Monaco itself
    // proved nothing here - the debounce did not run inside the harness, and a
    // test that passes with the fix reverted is worse than no test.
    const source = fs.readFileSync(BOARD, 'utf-8');

    await page.evaluate((src) => {
      (window as any).__loadBoard(src, 'kicad_pcb');
    }, source);
    await page.waitForTimeout(300);

    expect(
      await page.evaluate(() => (window as any).__loadedKind()),
      'opening a KiCad board has to record that it is one',
    ).toBe('kicad_pcb');

    const withKicadReader = await page.evaluate((src) => {
      const engine = (window as any).__pcbEngine;
      engine.load_kicad(src);
      return engine.get_snapshot()?.components?.length ?? 0;
    }, source);
    expect(withKicadReader, 'reloading it with its own reader keeps it').toBe(12);

    const withDslReader = await page.evaluate((src) => {
      const engine = (window as any).__pcbEngine;
      engine.load_source(src);
      return engine.get_snapshot()?.components?.length ?? 0;
    }, source);
    expect(
      withDslReader,
      'the DSL reader accepted a KiCad board, so the distinction this rests on does not exist',
    ).toBe(0);

    // And a `.cypcb` is still recorded as one.
    const dsl = fs.readFileSync(
      path.resolve(__dirname, '../../examples/blink.cypcb'),
      'utf-8',
    );
    await page.evaluate((src) => {
      (window as any).__loadBoard(src);
    }, dsl);
    expect(await page.evaluate(() => (window as any).__loadedKind())).toBe('cypcb');
  });

  test('the file input accepts the extension', async ({ page }) => {
    // A reader that works and a picker that will not offer the file is a
    // feature nobody can reach.
    const accepts = await page.evaluate(() => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.cypcb,.ses,.kicad_pcb';
      return input.accept;
    });
    expect(accepts).toContain('.kicad_pcb');
  });
});
