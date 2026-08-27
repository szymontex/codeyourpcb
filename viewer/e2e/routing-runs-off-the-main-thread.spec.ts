import { test, expect } from '@playwright/test';

/**
 * Routing runs on a worker, and the page keeps answering while it does.
 *
 * The owner's report was one sentence - the browser freezes totally - and it
 * was accurate: `auto_route_with_params` ran on the main thread, so nothing
 * painted, nothing scrolled and the cancel button could not be clicked until
 * the run ended. R201 to R203 are that report as requirements.
 *
 * The proof that matters is not that a spinner appears. It is that this test
 * can ask the page a question mid-run and get an answer: a blocked main thread
 * cannot execute `page.evaluate`, so a reply while `__routingWorker.active` is
 * true is the thing the report said was impossible.
 */

interface RoutingDebug {
  readonly active: boolean;
  readonly lastResult: string | null;
}

test.describe('Routing off the main thread', () => {
  test('the page answers while the worker routes', async ({ page }) => {
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

    const debugSurface = await page.evaluate(
      () =>
        typeof (window as never as { __routingWorker?: RoutingDebug }).__routingWorker +
        '/' +
        typeof (window as never as { __triggerRouting?: () => void }).__triggerRouting,
    );
    test.skip(debugSurface !== 'object/function', 'this build has no routing debug surface');

    // The Route button is inside a `display:none` anchor until D5 unhides the
    // autorouter, so the run is started the way the button starts it.
    await page.evaluate(() => {
      (window as never as { __triggerRouting: () => void }).__triggerRouting();
    });

    // Both facts read in one evaluation, because reading them one after the
    // other lets a fast run finish in between and turn a pass into a flake.
    const midRun = await page.waitForFunction(
      () => {
        const worker = (window as never as { __routingWorker: RoutingDebug }).__routingWorker;
        const overlay = document.querySelector('#routing-status');
        const visible = overlay instanceof HTMLElement && overlay.offsetParent !== null;
        return worker.active ? { active: true, visible, answeredAt: performance.now() } : null;
      },
      undefined,
      { timeout: 10_000 },
    );

    // `waitForFunction` resolves only on a truthy value, so the wait itself is
    // the assertion that a mid-run answer arrived at all; what is read here is
    // what the page said while it was routing.
    const seen = (await midRun.jsonValue()) as { active: boolean; visible: boolean; answeredAt: number };
    expect(seen.active).toBe(true);
    expect(seen.visible).toBe(true);
    expect(seen.answeredAt).toBeGreaterThan(0);

    // The run finishes and says what it did.
    await page.waitForFunction(
      () => {
        const worker = (window as never as { __routingWorker: RoutingDebug }).__routingWorker;
        return !worker.active && worker.lastResult !== null;
      },
      undefined,
      { timeout: 120_000 },
    );

    const result = await page.evaluate(
      () => (window as never as { __routingWorker: RoutingDebug }).__routingWorker.lastResult,
    );
    expect(result).toContain('"ok":true');
    await expect(page.locator('#status-text')).toContainText('Routed', { timeout: 10_000 });
  });
});
