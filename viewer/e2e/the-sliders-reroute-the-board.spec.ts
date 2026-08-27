import { test, expect } from '@playwright/test';

/**
 * A tuning slider re-routes the board, which it stopped doing when the freeze
 * made it unbearable.
 *
 * The handler used to end with `console.log('[Tuning] Params updated - click
 * Route to apply')` and a comment saying an automatic re-route freezes the
 * browser. It did, while the engine ran on this thread: a drag emits a value
 * every few pixels and each one blocked everything for as long as a route
 * takes. With the routing worker in place the slider can mean what it says,
 * and this is the test that says it does.
 */

interface TuningDebug {
  readonly active: boolean;
  readonly lastResult: string | null;
}

test.describe('The tuning sliders', () => {
  test('a slider change re-routes the board with no click anywhere', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    await page.locator('[data-template="blink"]').click();
    await page.waitForFunction(
      () => {
        const snapshot = (window as never as {
          __pcbEngine?: { get_snapshot(): { components?: unknown[] } };
        }).__pcbEngine?.get_snapshot();
        return (snapshot?.components?.length ?? 0) > 0;
      },
      undefined,
      { timeout: 15_000 },
    );

    const surface = await page.evaluate(
      () => typeof (window as never as { __tuningWorker?: TuningDebug }).__tuningWorker,
    );
    test.skip(surface !== 'object', 'this build has no tuning debug surface');

    const before = await page.evaluate(() => {
      const snapshot = (window as never as {
        __pcbEngine: { get_snapshot(): { traces?: unknown[] } };
      }).__pcbEngine.get_snapshot();
      return snapshot.traces?.length ?? 0;
    });

    // The whole toolbar anchor is display:none until D5 unhides the autorouter,
    // so the slider is moved the way a hand moves it: set the value, tell the
    // page it changed.
    await page.evaluate(() => {
      const slider = document.querySelector('#tune-density') as HTMLInputElement;
      slider.value = String(Number(slider.value) + 0.5);
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    });

    await page.waitForFunction(
      () => (window as never as { __tuningWorker: TuningDebug }).__tuningWorker.lastResult !== null,
      undefined,
      { timeout: 120_000 },
    );

    const result = await page.evaluate(
      () => (window as never as { __tuningWorker: TuningDebug }).__tuningWorker.lastResult,
    );
    expect(result).toContain('"ok":true');
    await expect(page.locator('#status-text')).toContainText('Tuned', { timeout: 10_000 });

    const after = await page.evaluate(() => {
      const snapshot = (window as never as {
        __pcbEngine: { get_snapshot(): { traces?: unknown[] } };
      }).__pcbEngine.get_snapshot();
      return snapshot.traces?.length ?? 0;
    });
    expect(after).toBeGreaterThan(before);
  });
});
