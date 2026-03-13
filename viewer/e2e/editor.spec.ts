import { test, expect } from '@playwright/test';

/** Minimal board source to dismiss project manager overlay */
const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;

test.describe('Editor Toggle & Code Input', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Dismiss project manager overlay so editor/canvas area is accessible
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
  });

  test('editor toggle button opens editor panel', async ({ page }) => {
    const editorContainer = page.locator('#editor-container');
    // Editor starts hidden
    await expect(editorContainer).toBeHidden();

    // Click the editor toggle
    await page.click('#editor-toggle');
    // Editor panel should become visible
    await expect(editorContainer).toBeVisible({ timeout: 5_000 });
  });

  test('Ctrl+E toggles editor panel', async ({ page }) => {
    const editorContainer = page.locator('#editor-container');
    await expect(editorContainer).toBeHidden();

    // Open via keyboard shortcut
    await page.keyboard.press('Control+e');
    await expect(editorContainer).toBeVisible({ timeout: 5_000 });

    // Close via keyboard shortcut — press again
    await page.keyboard.press('Control+e');
    await expect(editorContainer).toBeHidden();
  });

  test('Monaco editor loads and accepts input', async ({ page }) => {
    // Open editor
    await page.click('#editor-toggle');
    // Wait for Monaco to mount — it creates .monaco-editor
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Focus Monaco and type .cypcb code
    await page.click('.monaco-editor .view-lines');
    await page.keyboard.type('version 2\n\nboard test_board {\n  size 50mm x 30mm\n}', { delay: 10 });

    // Verify content was entered via Monaco's model
    const content = await page.evaluate(() => {
      const editors = (window as any).monaco?.editor?.getEditors?.();
      if (editors && editors.length > 0) {
        return editors[0].getValue();
      }
      // Fallback — check DOM text
      return document.querySelector('.monaco-editor .view-lines')?.textContent || '';
    });
    expect(content).toContain('version');
    expect(content).toContain('board');
  });

  test('editor re-toggle hides panel', async ({ page }) => {
    const editorContainer = page.locator('#editor-container');

    // Open
    await page.click('#editor-toggle');
    await expect(editorContainer).toBeVisible({ timeout: 5_000 });

    // Close
    await page.click('#editor-toggle');
    await expect(editorContainer).toBeHidden();
  });
});
