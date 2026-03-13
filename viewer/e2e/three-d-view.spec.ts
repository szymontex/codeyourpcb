import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BLINK_PATH = path.resolve(__dirname, '../../examples/blink.cypcb');

/** Load blink.cypcb via the app's __loadBoard debug surface. */
async function loadBlink(page: import('@playwright/test').Page): Promise<void> {
  const content = fs.readFileSync(BLINK_PATH, 'utf-8');
  await page.evaluate((src) => {
    const loader = (window as any).__loadBoard;
    if (loader) loader(src);
  }, content);
  await page.waitForTimeout(500);
}

/** Activate 3D view and wait for renderer to be ready. */
async function activate3D(page: import('@playwright/test').Page): Promise<void> {
  await page.click('#view-3d-btn');
  await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });
  // Wait for renderer to finish building geometry
  await page.waitForFunction(
    () => (window as any).__renderer3d?.isActive === true,
    { timeout: 5_000 },
  );
  // Allow build methods to complete
  await page.waitForTimeout(300);
}

/** Read all geometry counters from the debug surface. */
async function getGeometryCounts(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const r = (window as any).__renderer3d;
    return {
      componentCount: r?.componentCount,
      traceSegmentCount: r?.traceSegmentCount,
      padCount: r?.padCount,
      viaCount: r?.viaCount,
      meshCount: r?.meshCount,
    };
  });
}

test.describe('3D View Toggle', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('3D button activates Three.js renderer', async ({ page }) => {
    // Click 3D button
    await page.click('#view-3d-btn');

    // Wait for Three.js canvas to appear inside canvas-container
    // Three.js creates a <canvas> element inside the container
    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    // Verify renderer3d debug surface reports active
    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive === true;
    });
    expect(isActive).toBe(true);
  });

  test('pressing 3 key toggles 3D view', async ({ page }) => {
    // Dismiss project manager overlay so canvas is clickable
    const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
    // Focus body to ensure key works
    await page.click('#pcb-canvas');
    await page.keyboard.press('3');

    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive === true;
    });
    expect(isActive).toBe(true);
  });

  test('toggling back to 2D removes 3D and restores canvas', async ({ page }) => {
    // Activate 3D
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    // 2D canvas should be hidden
    await expect(page.locator('#pcb-canvas')).toBeHidden();

    // Toggle back to 2D
    await page.click('#view-3d-btn');

    // Button should lose active class
    await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/);

    // 2D canvas should be visible again
    await expect(page.locator('#pcb-canvas')).toBeVisible();

    // renderer3d should be disposed
    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive ?? false;
    });
    expect(isActive).toBe(false);
  });
});

test.describe('3D Geometry Verification', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('3D view renders component geometry after loading board', async ({ page }) => {
    // Load a real board with components
    await loadBlink(page);

    // Toggle to 3D
    await activate3D(page);

    const counts = await getGeometryCounts(page);

    // blink.cypcb has multiple components — must see them in 3D
    expect(counts.componentCount).toBeGreaterThan(0);
    // Total mesh count includes board, components, pads, traces, vias
    expect(counts.meshCount).toBeGreaterThan(1);
  });

  test('debug surface reports valid geometry counts', async ({ page }) => {
    // Load board and toggle 3D
    await loadBlink(page);
    await activate3D(page);

    const counts = await getGeometryCounts(page);

    // All four counters must be numbers ≥ 0
    expect(typeof counts.componentCount).toBe('number');
    expect(typeof counts.traceSegmentCount).toBe('number');
    expect(typeof counts.padCount).toBe('number');
    expect(typeof counts.viaCount).toBe('number');

    expect(counts.componentCount).toBeGreaterThanOrEqual(0);
    expect(counts.traceSegmentCount).toBeGreaterThanOrEqual(0);
    expect(counts.padCount).toBeGreaterThanOrEqual(0);
    expect(counts.viaCount).toBeGreaterThanOrEqual(0);
  });

  test('3D toggle preserves geometry on re-toggle', async ({ page }) => {
    await loadBlink(page);

    // First toggle — capture counts
    await activate3D(page);
    const first = await getGeometryCounts(page);

    // Sanity: must have geometry to compare
    expect(first.componentCount).toBeGreaterThan(0);

    // Toggle back to 2D
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/);

    // Toggle to 3D again
    await activate3D(page);
    const second = await getGeometryCounts(page);

    // Re-init must reconstruct the same geometry
    expect(second.componentCount).toBe(first.componentCount);
    expect(second.meshCount).toBe(first.meshCount);
    expect(second.padCount).toBe(first.padCount);
    expect(second.viaCount).toBe(first.viaCount);
    expect(second.traceSegmentCount).toBe(first.traceSegmentCount);
  });
});
