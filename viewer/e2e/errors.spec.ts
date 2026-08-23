import { test, expect } from '@playwright/test';

/** Minimal board source to dismiss project manager overlay */
const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;

test.describe('Error Display', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Dismiss project manager overlay so editor/canvas area is accessible
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
  });

  test('malformed code in editor triggers error handling', async ({ page }) => {
    // Open editor
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Type malformed .cypcb code into Monaco
    await page.click('.monaco-editor .view-lines');
    await page.keyboard.type('version 1\n\nboard bad {\n  size  // missing value\n}', { delay: 10 });

    // Wait for debounce (300ms) + processing
    await page.waitForTimeout(800);

    // The engine should have processed this — check via console or status
    // Parse errors are logged to console, and snapshot updates
    const consoleMessages: string[] = [];
    page.on('console', (msg) => consoleMessages.push(msg.text()));

    // The board should have been processed by the engine
    const hasSnapshot = await page.evaluate(() => {
      return typeof (window as any).__renderState !== 'undefined';
    });
    expect(hasSnapshot).toBe(true);
  });

  test('DRC violations show error badge', async ({ page }) => {
    // Open editor and input DRC-triggering code
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Type code with components too close (triggers clearance violation)
    await page.click('.monaco-editor .view-lines');
    const drcCode = [
      'version 1',
      '',
      'board drc_test {',
      '    size 30mm x 30mm',
      '    layers 2',
      '}',
      '',
      'component R1 resistor "0402" {',
      '    value "10k"',
      '    at 10mm, 15mm',
      '}',
      '',
      'component R2 resistor "0402" {',
      '    value "10k"',
      '    at 10.5mm, 15mm',
      '}',
    ].join('\n');

    await page.keyboard.type(drcCode, { delay: 5 });

    // Wait for debounce + DRC processing
    await page.waitForTimeout(1000);

    // Two 0402 parts half a millimetre apart is a clearance violation on every
    // preset this tool ships, so the badge has to appear. The previous version
    // asked `if (badgeVisible)` and, when it was not, checked that the status
    // bar did not say "WASM Error" - which passes on a checker that reports
    // nothing at all, the one failure this test is named after.
    await expect(
      page.locator('#error-badge'),
      'two parts 0.5mm apart produced no violation badge',
    ).toBeVisible({ timeout: 5_000 });

    const count = await page.locator('#error-count').textContent();
    expect(Number(count)).toBeGreaterThan(0);

    // Click badge to open error panel
    await page.click('#error-badge');
    await expect(page.locator('#error-panel')).toBeVisible();

    // The panel says what is wrong, not just that something is.
    await expect(page.locator('#error-panel')).toContainText(/clearance/i);
  });

  // The panel's three lines that call the grouper were the one part of that
  // feature nothing exercised: `groupByContact` has unit tests, the editor
  // markers have a pipeline test, and `populateErrorList` is a closure inside
  // `main.ts` that touches the DOM on import, so only a browser reaches it.
  // The fixture above cannot see grouping either way - two parts 0.5mm apart
  // are one contact reported once - so this one is built to produce two rows
  // about a single pair.
  test('the panel lists one item per contact, not one per row', async ({ page }) => {
    // R3 sits on a trace that steps around it, so two of the trace's three
    // segments touch the same component. Measured with the release binary:
    // `clearance: 2`, and `(2 clearance rows describe 1 contacts)`.
    const GROUPED_BOARD = [
      'version 1',
      '',
      'board grouping {',
      '    size 30mm x 30mm',
      '    layers 2',
      '}',
      '',
      'component R1 resistor "0402" {',
      '    value "10k"',
      '    at 5mm, 15mm',
      '}',
      '',
      'component R2 resistor "0402" {',
      '    value "10k"',
      '    at 25mm, 15mm',
      '}',
      '',
      'component R3 resistor "0402" {',
      '    value "10k"',
      '    at 15mm, 15mm',
      '}',
      '',
      'net SIG {',
      '    R1.1',
      '    R2.1',
      '}',
      '',
      'trace SIG {',
      '    from R1.1',
      '    via 14mm, 15mm',
      '    via 16mm, 15mm',
      '    to R2.1',
      '    layer Top',
      '    width 0.2mm',
      '}',
    ].join('\n');

    await page.evaluate((src) => (window as any).__loadBoard(src), GROUPED_BOARD);
    await expect(
      page.locator('#error-badge'),
      'a trace crossing a component produced no violation badge',
    ).toBeVisible({ timeout: 10_000 });

    await page.click('#error-badge');
    await expect(page.locator('#error-panel')).toBeVisible();

    // One item, though the rule reported two rows.
    await expect(
      page.locator('#error-list .error-item').filter({ hasText: 'Copper clearance' }),
    ).toHaveCount(1);

    // And the panel says so, rather than quietly dropping a row.
    await expect(page.locator('#error-panel')).toContainText(
      '2 clearance rows describe 1 contacts',
    );
    await expect(page.locator('#error-panel')).toContainText(
      'and 1 more place where the same two touch',
    );
  });

  test('error panel close button works', async ({ page }) => {
    // Verify error panel is hidden by default
    await expect(page.locator('#error-panel')).toBeHidden();

    // Error panel close button should exist
    await expect(page.locator('#error-panel-close')).toBeAttached();
  });

  test('app handles invalid input without crashing', async ({ page }) => {
    // Wait for WASM init before checking state
    await page.waitForFunction(
      () => document.getElementById('status-text')?.textContent?.includes('Ready'),
      { timeout: 10000 }
    );
    const result = await page.evaluate(() => {
      try {
        const status = document.getElementById('status-text')?.textContent;
        return { ok: true, status };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    });
    expect(result.ok).toBe(true);
    expect(result.status).toContain('Ready');
  });

  test('WASM engine is loaded and functional', async ({ page }) => {
    // Wait for WASM init to complete — status transitions to 'Ready' once loaded
    await page.waitForFunction(
      () => document.getElementById('status-text')?.textContent?.includes('Ready'),
      { timeout: 10000 }
    );
    const statusText = await page.textContent('#status-text');
    expect(statusText).toContain('Ready');
    // Verify engine object exists (proves WASM bridge loaded, not just mock)
    const hasEngine = await page.evaluate(() => typeof (window as any).__pcbEngine !== 'undefined' || typeof (window as any).__undoStack !== 'undefined');
    expect(hasEngine).toBe(true);
  });
});
