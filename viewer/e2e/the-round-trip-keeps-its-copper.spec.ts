import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Both halves of the round trip, counted, on files that actually carry copper.
 *
 * The owner has twice seen a trace vanish while wiring by hand. The census
 * added last commit reports a fall in the trace count per net, and it now runs
 * on both halves: `syncEditorTraces` compares the engine against the editor,
 * and `loadDesign` compares the text against the world it became.
 *
 * An instrument that cries wolf is worse than none, so this is the check that
 * it does not. Every bundled example that declares a trace is loaded, and any
 * `[trace-census]` line on the console fails the test with what it said.
 *
 * It is also the shape the hunt itself will take: when the disappearing trace
 * is reproduced, it will be a line in this output rather than a silence.
 */

const EXAMPLES = path.resolve(__dirname, '../../examples');

/** Every bundled example that declares at least one trace block. */
function examplesWithCopper(): string[] {
  return fs
    .readdirSync(EXAMPLES)
    .filter((name) => name.endsWith('.cypcb'))
    .filter((name) => /^\s*trace\s/m.test(fs.readFileSync(path.join(EXAMPLES, name), 'utf-8')))
    .sort();
}

test.describe('the trace census does not cry wolf', () => {
  test('every example that carries copper loads without losing any', async ({ page }) => {
    const names = examplesWithCopper();
    // If this ever reaches zero the test below passes by measuring nothing,
    // which is the failure mode two tests in this project have already had.
    expect(names.length).toBeGreaterThan(2);

    const complaints: string[] = [];
    page.on('console', (message) => {
      if (message.type() === 'error' && message.text().includes('[trace-census]')) {
        complaints.push(message.text());
      }
    });

    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    for (const name of names) {
      const source = fs.readFileSync(path.join(EXAMPLES, name), 'utf-8');
      await page.evaluate((src) => {
        (window as any).__loadBoard(src, 'cypcb');
      }, source);
      await page.waitForTimeout(120);
    }

    expect(
      complaints,
      `the census reported copper lost on a file nothing is wrong with:\n${complaints.join('\n')}`,
    ).toEqual([]);
  });

  test('the census is wired into the parse back at all', async ({ page }) => {
    // The control. A test that only ever asserts silence passes just as well
    // when the instrument is disconnected, so this feeds the loader a design
    // whose trace names a net the board never declares - copper the parser has
    // nowhere to put - and requires the census to say so.
    const complaints: string[] = [];
    page.on('console', (message) => {
      if (message.type() === 'error' && message.text().includes('[trace-census]')) {
        complaints.push(message.text());
      }
    });

    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    const orphan = [
      'version 1',
      'board b {',
      '  size 20mm x 20mm',
      '  layers 2',
      '}',
      'trace GHOST_NET {',
      '  layer top',
      '  width 0.25mm',
      '  path 2mm,2mm to 10mm,2mm',
      '}',
    ].join('\n');

    await page.evaluate((src) => {
      (window as any).__loadBoard(src, 'cypcb');
    }, orphan);
    await page.waitForTimeout(200);

    expect(complaints.join('\n')).toContain('GHOST_NET');
  });
});
