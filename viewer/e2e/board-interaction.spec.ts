import { test, expect } from '@playwright/test';

test.describe('Board Interaction — Layer Visibility', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('top layer checkbox toggles off and on', async ({ page }) => {
    const topCb = page.locator('#layer-top');
    // Starts checked
    await expect(topCb).toBeChecked();

    // Uncheck
    await topCb.uncheck();
    await expect(topCb).not.toBeChecked();

    // Re-check
    await topCb.check();
    await expect(topCb).toBeChecked();
  });

  test('bottom layer checkbox toggles off and on', async ({ page }) => {
    const bottomCb = page.locator('#layer-bottom');
    await expect(bottomCb).toBeChecked();

    await bottomCb.uncheck();
    await expect(bottomCb).not.toBeChecked();

    await bottomCb.check();
    await expect(bottomCb).toBeChecked();
  });

  test('layer state persists across toggles', async ({ page }) => {
    const topCb = page.locator('#layer-top');
    const bottomCb = page.locator('#layer-bottom');

    // Uncheck both
    await topCb.uncheck();
    await bottomCb.uncheck();

    await expect(topCb).not.toBeChecked();
    await expect(bottomCb).not.toBeChecked();

    // Check only top
    await topCb.check();
    await expect(topCb).toBeChecked();
    await expect(bottomCb).not.toBeChecked();
  });

  test('fit-to-board via F key does not cause errors', async ({ page }) => {
    // Focus the canvas area first (click on it)
    await page.click('#pcb-canvas');
    // Press F to fit board
    await page.keyboard.press('f');

    // No error should appear — status should still show Ready
    const status = page.locator('#status-text');
    const text = await status.textContent();
    expect(text).not.toContain('Error');
  });

  test('ratsnest checkbox toggles', async ({ page }) => {
    const ratsnestCb = page.locator('#layer-ratsnest');
    await expect(ratsnestCb).toBeChecked();

    await ratsnestCb.uncheck();
    await expect(ratsnestCb).not.toBeChecked();

    await ratsnestCb.check();
    await expect(ratsnestCb).toBeChecked();
  });
});
