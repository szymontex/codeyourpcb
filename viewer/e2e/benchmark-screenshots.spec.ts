import { test, expect } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Read a benchmark KiCad fixture file and return its content as a string.
 */
function readFixture(filename: string): string {
  const fixturePath = path.resolve(__dirname, '../../tests/fixtures/benchmark/', filename);
  return fs.readFileSync(fixturePath, 'utf-8');
}

/** Screenshot output directory */
const SCREENSHOT_DIR = 'test-results/benchmark';

/** Benchmark fixtures with their timeout configurations */
const FIXTURES = [
  { name: 'led_blink.kicad_pcb', slow: false },
  { name: 'stm32_breakout.kicad_pcb', slow: true },
  { name: 'multi_ic.kicad_pcb', slow: true },
] as const;

test.describe('Benchmark Screenshots', () => {
  for (const fixture of FIXTURES) {
    const baseName = fixture.name.replace('.kicad_pcb', '');

    test(`capture routed board: ${baseName}`, async ({ page }) => {
      // Extended timeout for complex boards
      if (fixture.slow) {
        test.slow();
        test.setTimeout(60_000);
      }

      // Collect page errors for assertion
      const pageErrors: Error[] = [];
      page.on('pageerror', (err) => pageErrors.push(err));

      // Navigate and wait for WASM Ready
      await page.goto('/');
      await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

      // Load the benchmark fixture
      const source = readFixture(fixture.name);
      await page.evaluate((src) => (window as any).__loadBoard(src), source);

      // Wait for board to render
      await page.waitForTimeout(1000);

      // Trigger routing
      const routeBtn = page.locator('#route-btn');
      await routeBtn.click();

      // Wait for routing to complete — watch for status change or timeout
      // In mock/WASM mode routing may finish quickly or take a while
      const timeout = fixture.slow ? 45_000 : 15_000;
      await page.waitForFunction(
        () => {
          const status = document.getElementById('status-text');
          if (!status) return false;
          const text = status.textContent || '';
          // Routing done when status shows result, variants, error, or returns to Ready
          return (
            text.includes('routed') ||
            text.includes('variant') ||
            text.includes('Variant') ||
            text.includes('Ready') ||
            text.includes('Error') ||
            text.includes('error')
          );
        },
        {},
        { timeout },
      );

      // Small settle time for canvas rendering
      await page.waitForTimeout(500);

      // Capture full-page screenshot
      await page.screenshot({
        path: path.join(SCREENSHOT_DIR, `${baseName}.png`),
        fullPage: true,
      });

      // Capture canvas-only screenshot
      const canvas = page.locator('#pcb-canvas');
      if (await canvas.isVisible()) {
        await canvas.screenshot({
          path: path.join(SCREENSHOT_DIR, `${baseName}-canvas.png`),
        });
      }

      // Assert no page errors occurred (screenshots are artifacts for human review)
      expect(pageErrors).toHaveLength(0);
    });
  }
});
