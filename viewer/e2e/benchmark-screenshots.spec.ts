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

// These drive routing through #route-btn, which index.html hides inside a
// .tb-route-group wrapper marked display:none while the router's quality is
// worked on (decision D5 in docs/TRACKER.md). Nothing here can be reached from
// the UI until that wrapper is visible again.
//
// They run as skipped rather than as three permanent failures, because a gate
// that is always red is a gate nobody reads - and this one exits at the
// Playwright stage, so the autorouter benchmark and the duplication check
// never ran at all while these failed. Delete the .skip the moment the button
// comes back.
test.describe.skip('Benchmark Screenshots', () => {
  for (const fixture of FIXTURES) {
    const baseName = fixture.name.replace('.kicad_pcb', '');
    // Per-fixture budgets, decided out here rather than inside the test: a
    // branch in a test body means the run took one of two paths and the report
    // does not say which.
    const testTimeout = fixture.slow ? 60_000 : 30_000;
    const routeTimeout = fixture.slow ? 45_000 : 15_000;

    test(`capture routed board: ${baseName}`, async ({ page }) => {
      test.setTimeout(testTimeout);

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
      await page.waitForFunction(
        () => {
          const text = document.getElementById('status-text')?.textContent ?? '';
          // Routing done when status shows result, variants, error, or returns to Ready
          return ['routed', 'variant', 'Variant', 'Ready', 'Error', 'error'].some(word =>
            text.includes(word),
          );
        },
        {},
        { timeout: routeTimeout },
      );

      // Small settle time for canvas rendering
      await page.waitForTimeout(500);

      // Capture full-page screenshot
      await page.screenshot({
        path: path.join(SCREENSHOT_DIR, `${baseName}.png`),
        fullPage: true,
      });

      // Capture canvas-only screenshot. The canvas is the application; if it
      // is not on screen the screenshot below is not the thing this test is
      // for, so that is a failure rather than a branch.
      const canvas = page.locator('#pcb-canvas');
      await expect(canvas).toBeVisible();
      await canvas.screenshot({
        path: path.join(SCREENSHOT_DIR, `${baseName}-canvas.png`),
      });

      // Assert no page errors occurred (screenshots are artifacts for human review)
      expect(pageErrors).toHaveLength(0);
    });
  }
});
