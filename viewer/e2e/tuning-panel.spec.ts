import { test, expect } from '@playwright/test';

/** Minimal board source with components and nets so routing has something to work on */
const MINIMAL_BOARD = `version 1
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
test.describe.skip('Tuning Panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Load a board and dismiss project manager
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
  });

  test('tuning toggle button exists', async ({ page }) => {
    const toggle = page.locator('#tuning-toggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('title', 'Tuning parameters');
  });

  test('clicking toggle shows and hides tuning panel', async ({ page }) => {
    const toggle = page.locator('#tuning-toggle');
    const panel = page.locator('#tuning-panel');

    // Initially hidden
    await expect(panel).toHaveClass(/hidden/);

    // Click toggle → panel visible
    await toggle.click();
    await expect(panel).not.toHaveClass(/hidden/);

    // Click toggle again → panel hidden
    await toggle.click();
    await expect(panel).toHaveClass(/hidden/);
  });

  test('panel has 4 sliders with correct default values', async ({ page }) => {
    // Open panel
    await page.click('#tuning-toggle');
    await expect(page.locator('#tuning-panel')).not.toHaveClass(/hidden/);

    // Via Cost slider
    const viaCost = page.locator('#tune-via-cost');
    await expect(viaCost).toBeVisible();
    await expect(viaCost).toHaveValue('1');

    // Layer Preference slider
    const layerPref = page.locator('#tune-layer-pref');
    await expect(layerPref).toBeVisible();
    await expect(layerPref).toHaveValue('0');

    // Roundness slider
    const roundness = page.locator('#tune-roundness');
    await expect(roundness).toBeVisible();
    await expect(roundness).toHaveValue('0.5');

    // Density slider
    const density = page.locator('#tune-density');
    await expect(density).toBeVisible();
    await expect(density).toHaveValue('1');
  });

  test('changing slider updates value display and settings', async ({ page }) => {
    // Open panel
    await page.click('#tuning-toggle');

    // Change Via Cost slider via JavaScript (range inputs need programmatic value set + event dispatch)
    await page.evaluate(() => {
      const slider = document.getElementById('tune-via-cost') as HTMLInputElement;
      slider.value = '5.0';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // Check value display updated
    await expect(page.locator('#tune-via-cost-val')).toHaveText('5.0');

    // Check settings updated
    const settingsParams = await page.evaluate(
      () => (window as any).__settings?.autorouteParams
    );
    expect(settingsParams).toBeDefined();
    expect(settingsParams.viaCost).toBe(5);
  });

  test('debug surface reflects panel state', async ({ page }) => {
    // Initially hidden with default params
    const initial = await page.evaluate(() => (window as any).__tuningPanel);
    expect(initial.visible).toBe(false);
    expect(initial.params.viaCost).toBe(1.0);
    expect(initial.params.roundness).toBe(0.5);

    // Open panel
    await page.click('#tuning-toggle');
    const opened = await page.evaluate(() => (window as any).__tuningPanel);
    expect(opened.visible).toBe(true);

    // Change roundness
    await page.evaluate(() => {
      const slider = document.getElementById('tune-roundness') as HTMLInputElement;
      slider.value = '0.8';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const updated = await page.evaluate(() => (window as any).__tuningPanel);
    expect(updated.params.roundness).toBeCloseTo(0.8, 1);
  });

  test('slider values persist across reload via settings', async ({ page }) => {
    // Open panel and change values
    await page.click('#tuning-toggle');

    await page.evaluate(() => {
      const slider = document.getElementById('tune-density') as HTMLInputElement;
      slider.value = '1.5';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // Verify setting was stored
    const stored = await page.evaluate(() => {
      const raw = localStorage.getItem('cypcb-settings');
      return raw ? JSON.parse(raw) : null;
    });
    expect(stored?.autorouteParams?.density).toBe(1.5);

    // Reload page
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Load board again so __tuningPanel gets initialized
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);

    // Verify slider value was restored from settings
    const restoredVal = await page.evaluate(() => {
      const slider = document.getElementById('tune-density') as HTMLInputElement;
      return parseFloat(slider.value);
    });
    expect(restoredVal).toBe(1.5);

    // Verify debug surface reflects restored value
    const restored = await page.evaluate(() => (window as any).__tuningPanel);
    expect(restored.params.density).toBe(1.5);
  });

  test('slider change triggers debounced re-route (mock returns error)', async ({ page }) => {
    // Open panel
    await page.click('#tuning-toggle');

    // Set up console log listener for routing attempt
    const consoleLogs: string[] = [];
    page.on('console', (msg) => {
      if (msg.text().includes('[Tuning]')) {
        consoleLogs.push(msg.text());
      }
    });

    // Change via cost to trigger re-route
    await page.evaluate(() => {
      const slider = document.getElementById('tune-via-cost') as HTMLInputElement;
      slider.value = '3.0';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    // Wait for debounce (300ms) + processing time
    await page.waitForTimeout(500);

    // In mock mode, auto_route_with_params returns an error — verify the attempt was made
    const hasRouteLog = consoleLogs.some(log => log.includes('Re-routing with params'));
    expect(hasRouteLog).toBe(true);
  });
});
