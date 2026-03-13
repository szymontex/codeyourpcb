import { test, expect } from '@playwright/test';

test.describe('App Load & WASM Initialization', () => {
  test('page loads with correct title', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle('CodeYourPCB Viewer');
  });

  test('WASM initializes and reaches Ready state', async ({ page }) => {
    await page.goto('/');
    const status = page.locator('#status-text');
    // WASM loads async — wait for Ready text
    await expect(status).toContainText('Ready', { timeout: 15_000 });
  });

  test('status bar is visible', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await expect(page.locator('#status')).toBeVisible();
  });

  test('PCB canvas element exists and is visible', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    const canvas = page.locator('#pcb-canvas');
    await expect(canvas).toBeVisible();
    // Canvas should have non-zero dimensions
    const box = await canvas.boundingBox();
    expect(box).toBeTruthy();
    expect(box!.width).toBeGreaterThan(100);
    expect(box!.height).toBeGreaterThan(100);
  });

  test('toolbar elements are present', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await expect(page.locator('#editor-toggle')).toBeVisible();
    await expect(page.locator('#fit-btn')).toBeVisible();
    await expect(page.locator('#view-menu-btn')).toBeVisible();
    await expect(page.locator('#view-3d-btn')).toBeVisible();
    await expect(page.locator('#theme-toggle')).toBeVisible();
    await expect(page.locator('#prefs-btn')).toBeVisible();
    await expect(page.locator('#open-btn')).toBeVisible();
    // Layer checkboxes are inside View dropdown, not directly in toolbar
    await expect(page.locator('#view-menu-dropdown')).toHaveClass(/hidden/);
  });

  test('baseline screenshot of initial state', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await page.screenshot({ path: 'test-results/baseline-initial-state.png', fullPage: true });
  });
});
