import { test, expect } from '@playwright/test';

test.describe('Undo/Redo Keyboard Shortcuts', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('undo/redo buttons exist and start disabled', async ({ page }) => {
    const undoBtn = page.locator('#undo-btn');
    const redoBtn = page.locator('#redo-btn');

    await expect(undoBtn).toBeVisible();
    await expect(redoBtn).toBeVisible();
    await expect(undoBtn).toBeDisabled();
    await expect(redoBtn).toBeDisabled();
  });

  test('Ctrl+Z with empty stack does not crash', async ({ page }) => {
    // Focus canvas
    await page.click('#pcb-canvas');
    // Ctrl+Z on empty stack — should not throw
    await page.keyboard.press('Control+z');

    // App still functional
    const status = await page.locator('#status-text').textContent();
    expect(status).not.toContain('Error');
  });

  test('Ctrl+Shift+Z with empty stack does not crash', async ({ page }) => {
    await page.click('#pcb-canvas');
    await page.keyboard.press('Control+Shift+z');

    const status = await page.locator('#status-text').textContent();
    expect(status).not.toContain('Error');
  });

  test('undo stack debug surface is accessible', async ({ page }) => {
    // Verify the debug surface is installed
    const hasSurface = await page.evaluate(() => {
      return typeof (window as any).__undoStack !== 'undefined';
    });
    expect(hasSurface).toBe(true);

    // Check initial state
    const state = await page.evaluate(() => {
      const us = (window as any).__undoStack;
      return {
        depth: us?.depth,
        position: us?.position,
        canUndo: us?.canUndo,
        canRedo: us?.canRedo,
      };
    });
    expect(state.depth).toBe(0);
    expect(state.canUndo).toBe(false);
    expect(state.canRedo).toBe(false);
  });
});
