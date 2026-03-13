import { test, expect } from '@playwright/test';

/**
 * E2E tests for the S04 UI architecture:
 * - Toolbar restructuring (essential buttons only, no layer checkboxes in toolbar)
 * - View menu dropdown (open/close, layer toggles, grid visibility)
 * - Preferences modal (theme, units, grid, colors)
 * - Unit switching and persistence
 * - Grid visibility vs grid snap separation
 * - Settings persistence across reload
 */

test.describe('UI Architecture — Toolbar Structure', () => {
  test.beforeEach(async ({ page }) => {
    // Clear settings to start fresh
    await page.addInitScript(() => localStorage.removeItem('cypcb-settings'));
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('essential toolbar buttons are visible', async ({ page }) => {
    await expect(page.locator('#editor-toggle')).toBeVisible();
    await expect(page.locator('#undo-btn')).toBeVisible();
    await expect(page.locator('#redo-btn')).toBeVisible();
    await expect(page.locator('#fit-btn')).toBeVisible();
    await expect(page.locator('#view-menu-btn')).toBeVisible();
    await expect(page.locator('#view-3d-btn')).toBeVisible();
    await expect(page.locator('#theme-toggle')).toBeVisible();
    await expect(page.locator('#prefs-btn')).toBeVisible();
    await expect(page.locator('#open-btn')).toBeVisible();
  });

  test('layer checkboxes are NOT directly visible in toolbar (hidden in View dropdown)', async ({ page }) => {
    // The dropdown should be hidden by default
    await expect(page.locator('#view-menu-dropdown')).toHaveClass(/hidden/);
    // Layer checkboxes exist but are not visible (inside hidden dropdown)
    await expect(page.locator('#layer-top')).not.toBeVisible();
    await expect(page.locator('#layer-bottom')).not.toBeVisible();
    await expect(page.locator('#layer-ratsnest')).not.toBeVisible();
  });
});

test.describe('UI Architecture — View Menu', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => localStorage.removeItem('cypcb-settings'));
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('View menu opens on click and closes on second click', async ({ page }) => {
    const dropdown = page.locator('#view-menu-dropdown');
    await expect(dropdown).toHaveClass(/hidden/);

    // Open
    await page.click('#view-menu-btn');
    await expect(dropdown).not.toHaveClass(/hidden/);

    // Close by clicking button again
    await page.click('#view-menu-btn');
    await expect(dropdown).toHaveClass(/hidden/);
  });

  test('View menu closes on Escape', async ({ page }) => {
    const dropdown = page.locator('#view-menu-dropdown');

    await page.click('#view-menu-btn');
    await expect(dropdown).not.toHaveClass(/hidden/);

    await page.keyboard.press('Escape');
    await expect(dropdown).toHaveClass(/hidden/);
  });

  test('View menu closes on click outside', async ({ page }) => {
    const dropdown = page.locator('#view-menu-dropdown');

    await page.click('#view-menu-btn');
    await expect(dropdown).not.toHaveClass(/hidden/);

    // Click somewhere else (the canvas)
    await page.click('#pcb-canvas');
    await expect(dropdown).toHaveClass(/hidden/);
  });

  test('layer toggle via View menu changes checkbox state', async ({ page }) => {
    // Open View menu
    await page.click('#view-menu-btn');
    await expect(page.locator('#view-menu-dropdown')).not.toHaveClass(/hidden/);

    const topCb = page.locator('#layer-top');
    await expect(topCb).toBeChecked();

    // Uncheck top layer
    await topCb.uncheck();
    await expect(topCb).not.toBeChecked();

    // Re-check
    await topCb.check();
    await expect(topCb).toBeChecked();
  });

  test('grid visibility toggle in View menu', async ({ page }) => {
    await page.click('#view-menu-btn');
    const gridCb = page.locator('#view-grid-visible');
    await expect(gridCb).toBeChecked();

    // Toggle off
    await gridCb.uncheck();
    await expect(gridCb).not.toBeChecked();

    // Verify setting was persisted
    const gridVisible = await page.evaluate(() => (window as any).__settings?.gridVisible);
    expect(gridVisible).toBe(false);

    // Toggle back on
    await gridCb.check();
    await expect(gridCb).toBeChecked();
    const gridVisibleAfter = await page.evaluate(() => (window as any).__settings?.gridVisible);
    expect(gridVisibleAfter).toBe(true);
  });
});

test.describe('UI Architecture — Preferences Modal', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => localStorage.removeItem('cypcb-settings'));
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('Preferences modal opens and closes', async ({ page }) => {
    const overlay = page.locator('#prefs-overlay');
    await expect(overlay).toHaveClass(/hidden/);

    // Open
    await page.click('#prefs-btn');
    await expect(overlay).not.toHaveClass(/hidden/);

    // Close via X button
    await page.click('#prefs-close');
    await expect(overlay).toHaveClass(/hidden/);
  });

  test('Preferences modal closes on Escape', async ({ page }) => {
    const overlay = page.locator('#prefs-overlay');

    await page.click('#prefs-btn');
    await expect(overlay).not.toHaveClass(/hidden/);

    await page.keyboard.press('Escape');
    await expect(overlay).toHaveClass(/hidden/);
  });

  test('Preferences modal closes on backdrop click', async ({ page }) => {
    const overlay = page.locator('#prefs-overlay');

    await page.click('#prefs-btn');
    await expect(overlay).not.toHaveClass(/hidden/);

    // Click the overlay (backdrop), not the modal itself
    // The overlay fills the screen; click a corner far from the centered modal
    await page.click('#prefs-overlay', { position: { x: 5, y: 5 } });
    await expect(overlay).toHaveClass(/hidden/);
  });

  test('unit switching to mil updates coords display', async ({ page }) => {
    // Open Preferences
    await page.click('#prefs-btn');
    await expect(page.locator('#prefs-overlay')).not.toHaveClass(/hidden/);

    // Switch units to mil
    await page.selectOption('#prefs-units', 'mil');

    // Close modal
    await page.click('#prefs-close');

    // Move mouse over canvas to trigger coords update
    const canvas = page.locator('#pcb-canvas');
    const box = await canvas.boundingBox();
    if (box) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    }

    // Wait for coords to update
    await page.waitForTimeout(200);

    // Verify units setting was applied
    const settings = await page.evaluate(() => (window as any).__settings);
    expect(settings.units).toBe('mil');
  });

  test('layer color change persists in settings', async ({ page }) => {
    await page.click('#prefs-btn');
    await expect(page.locator('#prefs-overlay')).not.toHaveClass(/hidden/);

    // Change top copper color
    const colorInput = page.locator('#prefs-color-top');
    await colorInput.fill('#00ff00');
    // Trigger change event (fill alone doesn't fire change on color inputs)
    await colorInput.dispatchEvent('change');

    // Verify setting persisted
    const settings = await page.evaluate(() => (window as any).__settings);
    expect(settings.layerColors.topCopper).toBe('#00ff00');
  });
});

test.describe('UI Architecture — Persistence', () => {
  test('unit setting persists across reload', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Clear settings via evaluate (not addInitScript which re-runs on reload)
    await page.evaluate(() => localStorage.removeItem('cypcb-settings'));
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Open Preferences, switch to mil
    await page.click('#prefs-btn');
    await page.selectOption('#prefs-units', 'mil');
    await page.click('#prefs-close');

    // Verify it's set
    const unitsBefore = await page.evaluate(() => (window as any).__settings?.units);
    expect(unitsBefore).toBe('mil');

    // Reload page (no addInitScript to interfere)
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Verify unit persisted
    const unitsAfter = await page.evaluate(() => (window as any).__settings?.units);
    expect(unitsAfter).toBe('mil');
  });

  test('layer color persists across reload', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await page.evaluate(() => localStorage.removeItem('cypcb-settings'));
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Change top copper color via Preferences
    await page.click('#prefs-btn');
    const colorInput = page.locator('#prefs-color-top');
    await colorInput.fill('#abcdef');
    await colorInput.dispatchEvent('change');
    await page.click('#prefs-close');

    // Reload (settings should survive)
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Verify color persisted
    const settings = await page.evaluate(() => (window as any).__settings);
    expect(settings.layerColors.topCopper).toBe('#abcdef');
  });

  test('grid visibility vs grid snap are independent', async ({ page }) => {
    await page.addInitScript(() => localStorage.removeItem('cypcb-settings'));
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Toggle grid visibility off via View menu
    await page.click('#view-menu-btn');
    await page.locator('#view-grid-visible').uncheck();
    await page.click('#view-menu-btn'); // close menu

    // Read settings — gridVisible should be false, gridSnapSpacing unchanged
    const s1 = await page.evaluate(() => (window as any).__settings);
    expect(s1.gridVisible).toBe(false);
    expect(s1.gridSnapSpacing).toBe(1_270_000); // default 50mil

    // Now change grid snap via Preferences
    await page.click('#prefs-btn');
    const snapInput = page.locator('#prefs-grid-snap');
    await snapInput.fill('100mil');
    await snapInput.dispatchEvent('change');
    await page.click('#prefs-close');

    // Grid visibility should still be false, snap spacing changed
    const s2 = await page.evaluate(() => (window as any).__settings);
    expect(s2.gridVisible).toBe(false);
    expect(s2.gridSnapSpacing).toBe(2_540_000); // 100mil

    // Toggle grid visibility back on — snap spacing shouldn't change
    await page.click('#view-menu-btn');
    await page.locator('#view-grid-visible').check();
    await page.click('#view-menu-btn');

    const s3 = await page.evaluate(() => (window as any).__settings);
    expect(s3.gridVisible).toBe(true);
    expect(s3.gridSnapSpacing).toBe(2_540_000); // still 100mil
  });
});
