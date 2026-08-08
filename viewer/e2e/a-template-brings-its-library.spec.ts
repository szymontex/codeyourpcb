import { test, expect } from '@playwright/test';

/**
 * A design split across files, opened the way a user opens one.
 *
 * The engine resolves `import "lib/blocks.cypcb"` and cannot read a file - a
 * browser tab has no disk - so the host fetches beside the design and hands
 * the text over. Until that was wired, picking a template that imports gave a
 * board with none of its parts on it and `unknown module` in the console.
 *
 * This drives the project manager rather than `__loadBoard`, because the
 * fetching is chosen by how the design arrived: a template is served over
 * HTTP, a file from the picker has no directory to fetch from.
 */

test.describe('A template that imports its blocks', () => {
  test('the imported modules arrive on the board', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    await page.locator('[data-template="sensor-front-end"]').click();

    // The first draw shows the design's own definitions, which is nothing but
    // a board; the parts arrive when the library does. Wait for the board the
    // template describes: two dividers of two parts each, plus the indicator.
    await page.waitForFunction(
      () => {
        const snapshot = (window as never as {
          __pcbEngine?: { get_snapshot(): { components?: unknown[] } };
        }).__pcbEngine?.get_snapshot();
        return (snapshot?.components?.length ?? 0) >= 6;
      },
      undefined,
      { timeout: 15_000 },
    );

    const refdes: string[] = await page.evaluate(() => {
      const snapshot = (window as never as {
        __pcbEngine: { get_snapshot(): { components: Array<{ refdes: string }> } };
      }).__pcbEngine.get_snapshot();
      return snapshot.components.map(c => c.refdes);
    });

    // Each instance brings its own copy of the imported module's parts, under
    // the instance name. Nothing in the design names a resistor.
    expect(refdes).toContain('DIV_A_RTOP');
    expect(refdes).toContain('DIV_B_RTOP');
    expect(refdes).toContain('STATUS_D1');

    // And the import is not reported as a problem, because it was followed.
    const errors: string[] = await page.evaluate(() => {
      const engine = (window as never as {
        __pcbEngine: { get_diagnostics_json?(): string };
      }).__pcbEngine;
      return engine.get_diagnostics_json ? JSON.parse(engine.get_diagnostics_json()) : [];
    });
    const aboutImports = errors.filter(d =>
      JSON.stringify(d).includes('import') || JSON.stringify(d).includes('unknown module'),
    );
    expect(aboutImports, 'the library was fetched, so nothing is missing').toEqual([]);
  });
});
