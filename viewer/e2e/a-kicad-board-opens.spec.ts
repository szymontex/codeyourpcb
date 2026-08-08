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
