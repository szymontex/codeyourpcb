import { test, expect } from '@playwright/test';

/** Minimal board source for test setup */
const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;

test.describe('Project Manager', () => {
  test.beforeEach(async ({ page }) => {
    // Clear localStorage to ensure clean state
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    // Wait for WASM init
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('project manager is visible on fresh load', async ({ page }) => {
    const pm = page.locator('#project-manager');
    await expect(pm).toBeVisible();
  });

  test('toolbar is still visible while project manager is shown', async ({ page }) => {
    // PM should be visible
    await expect(page.locator('#project-manager')).toBeVisible();

    // Toolbar buttons should all be accessible — PM sits below toolbar (top: 41px)
    await expect(page.locator('#editor-toggle')).toBeVisible();
    await expect(page.locator('#fit-btn')).toBeVisible();
    await expect(page.locator('#view-menu-btn')).toBeVisible();
    await expect(page.locator('#open-btn')).toBeVisible();
    await expect(page.locator('#prefs-btn')).toBeVisible();
    await expect(page.locator('#theme-toggle')).toBeVisible();
  });

  test('shows template cards', async ({ page }) => {
    const pm = page.locator('#project-manager');
    await expect(pm).toBeVisible();

    // All 3 templates + blank card
    const cards = pm.locator('.pm-template-card');
    await expect(cards).toHaveCount(4);

    // Verify template names
    await expect(pm.locator('[data-template="blink"] h3')).toHaveText('Blink LED');
    await expect(pm.locator('[data-template="power-indicator"] h3')).toHaveText('Power Indicator');
    await expect(pm.locator('[data-template="simple-psu"] h3')).toHaveText('Simple PSU');
    await expect(pm.locator('[data-template-blank] h3')).toHaveText('Blank Board');
  });

  test('shows empty recent files message', async ({ page }) => {
    const emptyMsg = page.locator('.pm-recent-empty');
    await expect(emptyMsg).toHaveText('No recent files');
  });

  test('debug surface exposes state', async ({ page }) => {
    const debug = await page.evaluate(() => (window as any).__projectManager);
    expect(debug.visible).toBe(true);
    expect(debug.templateCount).toBe(3);
    expect(debug.recentFiles).toHaveLength(0);
  });

  test('clicking a template loads the board and hides project manager', async ({ page }) => {
    const pm = page.locator('#project-manager');
    await expect(pm).toBeVisible();

    // Click the "Blink LED" template
    await page.locator('[data-template="blink"]').click();

    // Project manager should be hidden
    await expect(pm).toBeHidden();

    // Status should show template loaded
    await expect(page.locator('#status-text')).toContainText('Blink LED');

    // Debug surface should reflect hidden state
    const debug = await page.evaluate(() => (window as any).__projectManager);
    expect(debug.visible).toBe(false);
  });

  test('clicking blank board loads empty scaffold and hides PM', async ({ page }) => {
    const pm = page.locator('#project-manager');
    await expect(pm).toBeVisible();

    await page.locator('[data-template-blank]').click();

    await expect(pm).toBeHidden();

    // Board snapshot should exist with default dimensions (50×50mm = 50_000_000 nm)
    const board = await page.evaluate(() => {
      const snap = (window as any).__pcbEngine?.get_snapshot?.();
      return snap?.board ? { w: snap.board.width_nm, h: snap.board.height_nm } : null;
    });
    expect(board).toBeTruthy();
    expect(board!.w).toBe(50_000_000);
    expect(board!.h).toBe(50_000_000);
  });

  test('recent files updated after loading a template', async ({ page }) => {
    // Load a template
    await page.locator('[data-template="blink"]').click();
    await expect(page.locator('#project-manager')).toBeHidden();

    // Check recent files in settings
    const recentFiles = await page.evaluate(() => {
      const raw = localStorage.getItem('cypcb-settings');
      if (!raw) return [];
      return JSON.parse(raw).recentFiles || [];
    });

    expect(recentFiles).toHaveLength(1);
    expect(recentFiles[0].name).toBe('Blink LED.cypcb');
    expect(recentFiles[0].timestamp).toBeGreaterThan(0);
  });

  test('recent files capped at 10', async ({ page }) => {
    // Seed 12 entries in localStorage
    await page.evaluate(() => {
      const entries = Array.from({ length: 12 }, (_, i) => ({
        name: `file-${i}.cypcb`,
        timestamp: Date.now() - i * 1000,
        thumbnail: null,
      }));
      const settings = JSON.parse(localStorage.getItem('cypcb-settings') || '{}');
      settings.recentFiles = entries;
      localStorage.setItem('cypcb-settings', JSON.stringify(settings));
    });

    // Load a template to trigger addRecentFile
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await page.locator('[data-template="blink"]').click();

    // Wait for template to load and PM to hide
    await expect(page.locator('#project-manager')).toBeHidden();

    const recentFiles = await page.evaluate(() => {
      const raw = localStorage.getItem('cypcb-settings');
      if (!raw) return [];
      return JSON.parse(raw).recentFiles || [];
    });

    // Should be capped at 10
    expect(recentFiles.length).toBeLessThanOrEqual(10);
    // Most recent should be first
    expect(recentFiles[0].name).toBe('Blink LED.cypcb');
  });

  test('open file button exists and is clickable', async ({ page }) => {
    const openBtn = page.locator('#pm-open-btn');
    await expect(openBtn).toBeVisible();
    await expect(openBtn).toHaveText('📁 Open File');
  });

  test('__loadBoard() hides project manager', async ({ page }) => {
    // PM should be visible initially
    await expect(page.locator('#project-manager')).toBeVisible();

    // Load a board via the E2E helper
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);

    // PM should now be hidden
    await expect(page.locator('#project-manager')).toBeHidden();
    const debug = await page.evaluate(() => (window as any).__projectManager);
    expect(debug.visible).toBe(false);
  });

  test('page reload shows PM with recent file listed', async ({ page }) => {
    // Load a template to create a recent file entry
    await page.locator('[data-template="power-indicator"]').click();
    await expect(page.locator('#project-manager')).toBeHidden();

    // Verify recent file was saved
    const before = await page.evaluate(() => {
      const raw = localStorage.getItem('cypcb-settings');
      return raw ? JSON.parse(raw).recentFiles?.length || 0 : 0;
    });
    expect(before).toBe(1);

    // Reload — PM should show again with the recent file
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await expect(page.locator('#project-manager')).toBeVisible();

    // Recent files list should show the previously loaded file
    const recentItem = page.locator('.pm-recent-item');
    await expect(recentItem).toHaveCount(1);
    await expect(page.locator('.pm-recent-name')).toHaveText('Power Indicator.cypcb');
  });

  test('showProjectManager() re-shows project manager after dismiss', async ({ page }) => {
    // First dismiss PM by loading a template
    await page.locator('[data-template="blink"]').click();
    await expect(page.locator('#project-manager')).toBeHidden();

    // Re-show PM via the debug surface (equivalent to desktop:new-file or Ctrl+N)
    await page.evaluate(() => {
      (window as any).__projectManager.show();
    });

    // PM should be visible again
    await expect(page.locator('#project-manager')).toBeVisible();
    const debug = await page.evaluate(() => (window as any).__projectManager);
    expect(debug.visible).toBe(true);
  });
});

test.describe('Editor → Board Reflow', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('editing board size in editor updates board dimensions after debounce', async ({ page }) => {
    // Load a board with known size (50mm × 50mm)
    const source50 = `version 1\n\nboard reflow_test {\n    size 50mm x 50mm\n    layers 2\n}`;
    await page.evaluate((src) => (window as any).__loadBoard(src), source50);

    // Verify initial board dimensions: 50mm = 50_000_000 nm
    const initial = await page.evaluate(() => {
      const snap = (window as any).__pcbEngine?.get_snapshot?.();
      return snap?.board ? { w: snap.board.width_nm, h: snap.board.height_nm } : null;
    });
    expect(initial).toBeTruthy();
    expect(initial!.w).toBe(50_000_000);
    expect(initial!.h).toBe(50_000_000);

    // Open editor panel
    await page.click('#editor-toggle');
    await expect(page.locator('#editor-container')).toBeVisible({ timeout: 5_000 });
    // Wait for Monaco to initialize
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Set editor content to a board with 80mm × 60mm via the __editor debug surface
    const source80 = `version 1\n\nboard reflow_test {\n    size 80mm x 60mm\n    layers 2\n}`;
    await page.evaluate((src) => {
      const editor = (window as any).__editor;
      if (editor) {
        editor.setValue(src);
      }
    }, source80);

    // Wait for debounce (300ms in setupEditorSync) + processing time
    await page.waitForTimeout(600);

    // Assert board dimensions changed to 80mm × 60mm
    const updated = await page.evaluate(() => {
      const snap = (window as any).__pcbEngine?.get_snapshot?.();
      return snap?.board ? { w: snap.board.width_nm, h: snap.board.height_nm } : null;
    });
    expect(updated).toBeTruthy();
    expect(updated!.w).toBe(80_000_000);
    expect(updated!.h).toBe(60_000_000);
  });
});
