import { test, expect } from '@playwright/test';

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
