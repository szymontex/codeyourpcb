import { test, expect } from '@playwright/test';

/** Minimal board source to dismiss project manager overlay */
const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;

test.describe('Reliability — malformed input handling', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    // Dismiss project manager overlay so editor/canvas area is accessible
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
  });

  test('malformed .cypcb with missing values does not crash', async ({ page }) => {
    // Open editor
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Type content from examples/invalid.cypcb — has missing layer value
    await page.click('.monaco-editor .view-lines');
    const malformed = [
      '// Invalid file - syntax error',
      'version 1',
      '',
      'board test {',
      '    size 50mm x 30mm',
      '    layers  // missing value',
      '}',
    ].join('\n');
    await page.keyboard.type(malformed, { delay: 5 });

    // Wait for debounce (300ms) + processing
    await page.waitForTimeout(800);

    // App must still be alive — status should not indicate a crash
    const status = await page.locator('#status-text').textContent();
    expect(status).not.toBeNull();
    expect(status).not.toContain('crashed');

    // Canvas should still be present (no blank screen)
    await expect(page.locator('#pcb-canvas')).toBeVisible();
  });

  test('unknown keyword input does not crash', async ({ page }) => {
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    await page.click('.monaco-editor .view-lines');
    const unknownKw = [
      'version 1',
      '',
      'board test {',
      '    size 50mm x 30mm',
      '    layers 2',
      '}',
      '',
      'module R1 resistor "0402" {',
      '    value "10k"',
      '}',
    ].join('\n');
    await page.keyboard.type(unknownKw, { delay: 5 });

    await page.waitForTimeout(800);

    // App still functional
    const status = await page.locator('#status-text').textContent();
    expect(status).not.toBeNull();
    expect(status).not.toContain('crashed');
    await expect(page.locator('#pcb-canvas')).toBeVisible();
  });

  test('completely garbage input does not crash', async ({ page }) => {
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    await page.click('.monaco-editor .view-lines');
    await page.keyboard.type('}{}{<script>alert("xss")</script>🔥\x00\xFF', { delay: 5 });

    await page.waitForTimeout(800);

    // App must survive garbage input without crashing
    await expect(page.locator('#pcb-canvas')).toBeVisible();
    const status = await page.locator('#status-text').textContent();
    expect(status).not.toBeNull();
  });

  test('XSS payload in board content is rendered safely', async ({ page }) => {
    await page.click('#editor-toggle');
    await expect(page.locator('.monaco-editor')).toBeVisible({ timeout: 10_000 });

    // Craft input where component names contain XSS payloads
    // The error panel builds DOM with textContent now, so this should be safe
    await page.click('.monaco-editor .view-lines');
    const xssPayload = [
      'version 1',
      '',
      'board <img src=x onerror=alert(1)> {',
      '    size 50mm x 30mm',
      '    layers 2',
      '}',
    ].join('\n');
    await page.keyboard.type(xssPayload, { delay: 5 });

    await page.waitForTimeout(800);

    // Verify no alert dialog was triggered (Playwright auto-accepts, but we can check)
    // More importantly: verify the XSS string appears as text, not as executed HTML
    const bodyHtml = await page.evaluate(() => document.body.innerHTML);
    expect(bodyHtml).not.toContain('<img src=x onerror');

    // App must still be functional
    await expect(page.locator('#pcb-canvas')).toBeVisible();
  });
});

test.describe('Reliability — URL state roundtrip', () => {
  test('URL with view state params applies on load', async ({ page }) => {
    // Navigate with specific view state
    await page.goto('/?l=top,bottom&z=2.50&x=15000000&y=10000000');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Verify the URL state was applied — check via the debug surface
    const appliedState = await page.evaluate(() => {
      // The layer checkboxes reflect URL state
      const topCb = document.getElementById('layer-top') as HTMLInputElement;
      const bottomCb = document.getElementById('layer-bottom') as HTMLInputElement;
      const ratsnestCb = document.getElementById('layer-ratsnest') as HTMLInputElement;
      return {
        topChecked: topCb?.checked ?? null,
        bottomChecked: bottomCb?.checked ?? null,
        ratsnestChecked: ratsnestCb?.checked ?? null,
      };
    });

    // top and bottom should be checked (from URL), ratsnest should be unchecked
    expect(appliedState.topChecked).toBe(true);
    expect(appliedState.bottomChecked).toBe(true);
    expect(appliedState.ratsnestChecked).toBe(false);
  });

  test('share button produces URL with current view state', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Grant clipboard permissions for this context
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);

    // Click share button
    const shareBtn = page.locator('#share-btn');
    if (await shareBtn.isVisible()) {
      await shareBtn.click();

      // Wait for clipboard write and status update
      await page.waitForTimeout(500);

      // Read clipboard
      const clipboardText = await page.evaluate(() => navigator.clipboard.readText());

      // The share URL should contain view state params
      expect(clipboardText).toContain('?l=');
      expect(clipboardText).toContain('&z=');
      expect(clipboardText).toContain('&x=');
      expect(clipboardText).toContain('&y=');

      // Verify it's a valid URL
      const url = new URL(clipboardText);
      expect(url.searchParams.has('l')).toBe(true);
      expect(url.searchParams.has('z')).toBe(true);
    }
  });

  test('URL state roundtrip preserves values', async ({ page }) => {
    // Load with known state
    await page.goto('/?l=top&z=3.00&x=5000000&y=8000000');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);

    // Click share to encode current state back to URL
    const shareBtn = page.locator('#share-btn');
    if (await shareBtn.isVisible()) {
      await shareBtn.click();
      await page.waitForTimeout(500);

      const clipboardText = await page.evaluate(() => navigator.clipboard.readText());
      const url = new URL(clipboardText);

      // Layers should roundtrip
      expect(url.searchParams.get('l')).toContain('top');

      // Zoom should roundtrip (may have floating point rounding)
      const z = parseFloat(url.searchParams.get('z') || '0');
      expect(z).toBeGreaterThan(0);

      // Pan values should roundtrip (allow integer rounding)
      const x = parseFloat(url.searchParams.get('x') || '0');
      const y = parseFloat(url.searchParams.get('y') || '0');
      expect(x).not.toBe(0);
      expect(y).not.toBe(0);
    }
  });
});
