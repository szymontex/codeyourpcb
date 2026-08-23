import { test, expect } from '@playwright/test';

/**
 * The stack manager, in a real browser.
 *
 * The two functions that decide what the panel says are pure and held by
 * `src/__tests__/the-stack-manager-shows-the-build.test.ts`. That suite runs
 * in node - `vitest.config.ts` says so and nothing here pulls jsdom - so the
 * element tree is held here instead, which is a better place to hold a panel:
 * this one opens it the way a person does.
 */

const BOARD = [
  'version 1',
  '',
  'board stacked {',
  '    size 30mm x 20mm',
  '    layers 2',
  '    stackup {',
  '        finish "ENIG"',
  '        edges plated',
  '        copper 1oz',
  '        core 1.5mm material "FR4" dk 4.5',
  '        copper 1oz',
  '    }',
  '}',
].join('\n');

test.describe('Stack manager', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('#pcb-canvas');
    // The project manager is open on a fresh load and covers the right-hand
    // side of the canvas, which is where this panel lives. A person dismisses
    // it before doing anything else; so does this.
    await page.evaluate(() => (window as any).__projectManager?.hide());
    await expect(page.locator('#project-manager')).toBeHidden();
  });

  test('the panel is closed until somebody opens it', async ({ page }) => {
    await expect(page.locator('#stack-panel')).toBeHidden();
  });

  test('it shows a row per stackup entry and the fabrication order beside it', async ({
    page,
  }) => {
    // The panel is opened first and the board loaded into it, rather than the
    // other way round with a wait in between. `pullSnapshot` redraws an open
    // panel, so every assertion below is then covered by Playwright's own
    // retrying rather than by a fixed sleep - which is what made this spec
    // fail once under a full parallel run and pass on its own.
    await page.click('#stack-toggle');
    await expect(page.locator('#stack-panel')).toBeVisible();

    // Through the load hook rather than the editor: Monaco closes a brace as
    // you type one, so a block typed line by line arrives with more braces
    // than it left with. What this test is about is the panel, not the editor.
    await page.evaluate((source) => (window as any).__loadBoard(source), BOARD);

    // Three entries: copper, core, copper.
    await expect(page.locator('#stack-panel .sp-row')).toHaveCount(3);
    // The copper is stated in ounces and shown as the thickness it is.
    await expect(page.locator('#stack-panel')).toContainText('0.035mm');
    // The dielectric's own numbers, in the units a fabricator reads.
    await expect(page.locator('#stack-panel')).toContainText('FR4');
    await expect(page.locator('#stack-panel')).toContainText('dk 4.5');
    // And what the fabricator is asked for beyond the layers.
    await expect(page.locator('#stack-panel .sp-summary')).toContainText('finish ENIG');
    await expect(page.locator('#stack-panel .sp-summary')).toContainText('edges plated');
  });

  test('a design with no stackup is told so in a sentence', async ({ page }) => {
    // Most designs state none, and that is not a board with no layers.
    await page.click('#stack-toggle');
    await expect(page.locator('#stack-panel')).toContainText('states no stackup');
    await expect(page.locator('#stack-panel .sp-row')).toHaveCount(0);
  });

  test('the close button closes it', async ({ page }) => {
    await page.click('#stack-toggle');
    await expect(page.locator('#stack-panel')).toBeVisible();
    await page.click('#stack-close');
    await expect(page.locator('#stack-panel')).toBeHidden();
  });
});
