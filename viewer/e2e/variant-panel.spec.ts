import { test, expect, type Page } from '@playwright/test';

/** Board with 2 components and a net — enough for routing to generate variants */
const ROUTABLE_BOARD = `version 1
board test {
  size 50mm x 50mm
  layers 2
}
component R1 resistor "0402" {
  value "1k"
  at 10mm, 10mm
}
component R2 resistor "0402" {
  value "1k"
  at 30mm, 30mm
}
net VCC {
  R1.1
  R2.1
}`;

/** The `window.__variantPanel` surface `src/variant-panel.ts` publishes. */
interface VariantDebug {
  visible: boolean;
  variantCount: number;
  activeIndex: number;
  hoveredIndex: number;
  variants: Array<{ name: string; composite: number }>;
}

function variantDebug(page: Page): Promise<VariantDebug> {
  return page.evaluate(() => (window as never as { __variantPanel: VariantDebug }).__variantPanel);
}

/**
 * Route the board and wait until the panel holds a set of variants.
 *
 * Every test here used to click Route, sleep two seconds and then hedge -
 * `if (debug.visible)`, or `test.skip()` when it was not - so a routing run
 * that produced nothing left the test green. Two seconds is also a guess:
 * routing this board takes what it takes. Waiting for the condition is both
 * honest and faster, and it fails with a timeout naming what never happened.
 */
async function routeAndWaitForVariants(page: Page, minimum = 2): Promise<VariantDebug> {
  await page.locator('#route-btn').click();
  await page.waitForFunction(
    n => ((window as never as { __variantPanel?: VariantDebug }).__variantPanel?.variantCount ?? 0) >= n,
    minimum,
    { timeout: 30_000 },
  );
  return variantDebug(page);
}

// Skipped, and the reason has moved on twice.
//
// The Route split-button is `display:none` in index.html pending D5, which is
// why this was skipped first. Running these assertions with the wrapper
// unhidden then showed that the panel had no code path that could show it:
// measured 2026-08-08, routing this board gave `[Routing] Routed 1 segments in
// 0s` and left `__variantPanel` at `{visible: false, variantCount: 0}`.
//
// **The panel has since been deleted** - `a9e8c7a`, "delete the variant panel,
// which nothing could reach". `showVariants`, `initVariantPanel`,
// `hideVariants` and `isVariantPanelVisible` are in no file under `src/`. The
// engine keeps `auto_route_variants()` and `src/wasm.ts` still declares it;
// nothing calls either. This file asserts against a screen no build produces,
// and is kept only because the requirement behind it is not withdrawn.
//
// So unhiding the button is necessary and not sufficient. What is written
// below is what the panel has to do the day somebody wires it up.
test.describe.skip('Variant Panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Load a board and dismiss project manager
    await page.evaluate((src) => (window as never as { __loadBoard: (s: string) => void }).__loadBoard(src), ROUTABLE_BOARD);
  });

  test('variant panel is initially hidden', async ({ page }) => {
    const panel = page.locator('#variant-panel');
    await expect(panel).toHaveClass(/hidden/);

    // Debug surface exists and shows hidden state
    const debug = await variantDebug(page);
    expect(debug).toBeDefined();
    expect(debug.visible).toBe(false);
    expect(debug.variantCount).toBe(0);
  });

  test('route button generates variants and shows panel', async ({ page }) => {
    const debug = await routeAndWaitForVariants(page);

    expect(debug.visible).toBe(true);
    expect(debug.variantCount).toBeGreaterThanOrEqual(2);
    expect(debug.variants).toHaveLength(debug.variantCount);

    // Each variant has a name and composite score
    for (const v of debug.variants) {
      expect(typeof v.name).toBe('string');
      expect(v.name.length).toBeGreaterThan(0);
      expect(typeof v.composite).toBe('number');
    }

    // Panel DOM should be visible, with a row per variant
    await expect(page.locator('#variant-panel')).not.toHaveClass(/hidden/);
    const rows = page.locator('.variant-row');
    await expect(rows).toHaveCount(debug.variantCount);

    // First row is active - the ranking puts its pick first
    expect(debug.activeIndex).toBe(0);
    await expect(rows.first()).toHaveClass(/active/);
  });

  test('hover on non-active variant triggers preview state', async ({ page }) => {
    await routeAndWaitForVariants(page);

    // Hover on the second variant row (non-active)
    const rows = page.locator('.variant-row');
    await rows.nth(1).hover();

    expect((await variantDebug(page)).hoveredIndex).toBe(1);

    // Move mouse away — hover clears
    await page.locator('#variant-panel-header').hover();
    await page.waitForFunction(
      () => (window as never as { __variantPanel: VariantDebug }).__variantPanel.hoveredIndex === -1,
      undefined,
      { timeout: 5_000 },
    );
  });

  test('clicking a variant makes it active', async ({ page }) => {
    await routeAndWaitForVariants(page);

    // Click the second variant
    const rows = page.locator('.variant-row');
    await rows.nth(1).click();

    expect((await variantDebug(page)).activeIndex).toBe(1);

    // Second row should have 'active' class
    await expect(rows.nth(1)).toHaveClass(/active/);
    // First row should not
    await expect(rows.first()).not.toHaveClass(/active/);
  });

  test('variant panel clears on new Route click', async ({ page }) => {
    const first = await routeAndWaitForVariants(page);
    expect(first.variantCount).toBeGreaterThanOrEqual(2);

    // A second route empties the panel before it fills it again. The old test
    // sighed at this - "Either visible with fresh results or hidden if routing
    // failed" - and asserted nothing. Both halves are checked here: the clear
    // is synchronous with the click, the refill is what routing produces.
    await page.locator('#route-btn').click();
    await page.waitForFunction(
      () => (window as never as { __variantPanel: VariantDebug }).__variantPanel.variantCount === 0,
      undefined,
      { timeout: 5_000 },
    );

    await page.waitForFunction(
      () => (window as never as { __variantPanel: VariantDebug }).__variantPanel.variantCount >= 2,
      undefined,
      { timeout: 30_000 },
    );
    const second = await variantDebug(page);
    expect(second.visible).toBe(true);
    expect(second.activeIndex).toBe(0);
  });

  test('tuning slider re-route clears variant panel', async ({ page }) => {
    await routeAndWaitForVariants(page);

    // Change tuning slider to trigger re-route. The opener is `#route-menu-btn`
    // - this clicked `#tuning-toggle`, which does not exist in index.html and
    // never has, so the test could only ever have failed on a missing locator.
    await page.locator('#route-menu-btn').click();
    await page.evaluate(() => {
      const slider = document.getElementById('tune-via-cost') as HTMLInputElement;
      slider.value = '3.0';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // The panel belongs to the results it was built from, so a re-route has to
    // empty it rather than leave stale rows on screen.
    await page.waitForFunction(
      () => (window as never as { __variantPanel: VariantDebug }).__variantPanel.variantCount === 0,
      undefined,
      { timeout: 10_000 },
    );
    expect((await variantDebug(page)).visible).toBe(false);
  });

  test('debug surface reflects variant count and active index', async ({ page }) => {
    // Before routing
    const before = await variantDebug(page);
    expect(before.visible).toBe(false);
    expect(before.variantCount).toBe(0);
    expect(before.activeIndex).toBe(0);

    const after = await routeAndWaitForVariants(page);
    expect(after.variantCount).toBeGreaterThanOrEqual(2);
    expect(after.activeIndex).toBe(0);
    expect(after.variants).toHaveLength(after.variantCount);
  });
});
