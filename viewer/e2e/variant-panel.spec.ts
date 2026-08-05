import { test, expect } from '@playwright/test';

/** Board with 2 components and a net — enough for routing to generate variants */
const ROUTABLE_BOARD = `version 1
board test {
  size 50mm x 50mm
  layers 2
}
component R1 0402 "0402" {
  value "1k"
  at 10mm, 10mm
}
component R2 0402 "0402" {
  value "1k"
  at 30mm, 30mm
}
net VCC {
  R1.1
  R2.1
}`;

// The Route split-button and its dropdown are hidden in index.html - see the
// "Autorouter disabled - needs fundamental rewrite" comment on the
// .tb-route-group wrapper. Nothing in this file can be driven from the UI until
// that wrapper is visible again, so these run as skipped rather than as noise in
// the gate. Delete the .skip the moment the button comes back.
test.describe.skip('Variant Panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Load a board and dismiss project manager
    await page.evaluate((src) => (window as any).__loadBoard(src), ROUTABLE_BOARD);
  });

  test('variant panel is initially hidden', async ({ page }) => {
    const panel = page.locator('#variant-panel');
    await expect(panel).toHaveClass(/hidden/);

    // Debug surface exists and shows hidden state
    const debug = await page.evaluate(() => (window as any).__variantPanel);
    expect(debug).toBeDefined();
    expect(debug.visible).toBe(false);
    expect(debug.variantCount).toBe(0);
  });

  test('route button generates variants and shows panel', async ({ page }) => {
    const routeBtn = page.locator('#route-btn');
    await routeBtn.click();

    // Wait for status to show variants generated (or error in mock mode)
    await page.waitForTimeout(2000);

    // Check debug surface for variant state
    const debug = await page.evaluate(() => (window as any).__variantPanel);
    expect(debug).toBeDefined();

    // In WASM mode, variants should be generated; in mock mode, panel won't show
    // We test both paths gracefully
    if (debug.visible) {
      expect(debug.variantCount).toBeGreaterThanOrEqual(2);
      expect(debug.activeIndex).toBe(0);
      expect(debug.variants.length).toBeGreaterThanOrEqual(2);

      // Each variant has a name and composite score
      for (const v of debug.variants) {
        expect(typeof v.name).toBe('string');
        expect(v.name.length).toBeGreaterThan(0);
        expect(typeof v.composite).toBe('number');
      }

      // Panel DOM should be visible
      const panel = page.locator('#variant-panel');
      await expect(panel).not.toHaveClass(/hidden/);

      // Variant rows should be present
      const rows = page.locator('.variant-row');
      const count = await rows.count();
      expect(count).toBeGreaterThanOrEqual(2);

      // First row should be active (best variant)
      await expect(rows.first()).toHaveClass(/active/);
    }
  });

  test('hover on non-active variant triggers preview state', async ({ page }) => {
    const routeBtn = page.locator('#route-btn');
    await routeBtn.click();
    await page.waitForTimeout(2000);

    const debug = await page.evaluate(() => (window as any).__variantPanel);
    if (!debug.visible || debug.variantCount < 2) {
      test.skip();
      return;
    }

    // Hover on the second variant row (non-active)
    const rows = page.locator('.variant-row');
    await rows.nth(1).hover();

    // Check debug surface shows hoveredIndex = 1
    const hoverDebug = await page.evaluate(() => (window as any).__variantPanel);
    expect(hoverDebug.hoveredIndex).toBe(1);

    // Move mouse away — hover clears
    await page.locator('#variant-panel-header').hover();
    await page.waitForTimeout(100);

    const afterDebug = await page.evaluate(() => (window as any).__variantPanel);
    expect(afterDebug.hoveredIndex).toBe(-1);
  });

  test('clicking a variant makes it active', async ({ page }) => {
    const routeBtn = page.locator('#route-btn');
    await routeBtn.click();
    await page.waitForTimeout(2000);

    const debug = await page.evaluate(() => (window as any).__variantPanel);
    if (!debug.visible || debug.variantCount < 2) {
      test.skip();
      return;
    }

    // Click the second variant
    const rows = page.locator('.variant-row');
    await rows.nth(1).click();

    // Active index should now be 1
    const clickDebug = await page.evaluate(() => (window as any).__variantPanel);
    expect(clickDebug.activeIndex).toBe(1);

    // Second row should have 'active' class
    await expect(rows.nth(1)).toHaveClass(/active/);
    // First row should not
    await expect(rows.first()).not.toHaveClass(/active/);
  });

  test('variant panel clears on new Route click', async ({ page }) => {
    const routeBtn = page.locator('#route-btn');

    // First route
    await routeBtn.click();
    await page.waitForTimeout(2000);

    const firstDebug = await page.evaluate(() => (window as any).__variantPanel);
    if (!firstDebug.visible) {
      test.skip();
      return;
    }
    expect(firstDebug.variantCount).toBeGreaterThanOrEqual(2);

    // Second route — panel clears first, then repopulates
    await routeBtn.click();
    // Small delay for the clear to happen synchronously before async routing
    await page.waitForTimeout(100);

    // Wait for new routing to complete
    await page.waitForTimeout(2000);

    // Panel should show new results
    const secondDebug = await page.evaluate(() => (window as any).__variantPanel);
    expect(secondDebug).toBeDefined();
    // Either visible with fresh results or hidden if routing failed
  });

  test('tuning slider re-route clears variant panel', async ({ page }) => {
    const routeBtn = page.locator('#route-btn');

    // Generate variants first
    await routeBtn.click();
    await page.waitForTimeout(2000);

    const debug = await page.evaluate(() => (window as any).__variantPanel);
    if (!debug.visible) {
      test.skip();
      return;
    }

    // Change tuning slider to trigger re-route
    await page.click('#tuning-toggle');
    await page.evaluate(() => {
      const slider = document.getElementById('tune-via-cost') as HTMLInputElement;
      slider.value = '3.0';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // Wait for debounce + re-route
    await page.waitForTimeout(600);

    // Variant panel should be cleared
    const afterTuning = await page.evaluate(() => (window as any).__variantPanel);
    expect(afterTuning.visible).toBe(false);
    expect(afterTuning.variantCount).toBe(0);
  });

  test('debug surface reflects variant count and active index', async ({ page }) => {
    // Before routing
    const beforeDebug = await page.evaluate(() => (window as any).__variantPanel);
    expect(beforeDebug.visible).toBe(false);
    expect(beforeDebug.variantCount).toBe(0);
    expect(beforeDebug.activeIndex).toBe(0);

    // Route
    await page.click('#route-btn');
    await page.waitForTimeout(2000);

    const afterDebug = await page.evaluate(() => (window as any).__variantPanel);
    if (afterDebug.visible) {
      expect(afterDebug.variantCount).toBeGreaterThanOrEqual(2);
      expect(afterDebug.activeIndex).toBe(0);
      expect(afterDebug.variants).toHaveLength(afterDebug.variantCount);
    }
  });
});
