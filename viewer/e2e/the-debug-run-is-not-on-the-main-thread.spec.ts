import { test, expect } from '@playwright/test';

/**
 * The debug run happens on a worker too.
 *
 * `auto_route_debug` routes the board and keeps every pass, so it is the
 * heaviest call the engine has and it was the last one still made from the
 * main thread: the panel that exists to explain a slow route froze the page
 * for longer than the route did.
 *
 * The proof is the same one the routing test uses - a question answered while
 * the run is in flight, which a blocked main thread cannot do.
 */

interface WorkerDebug {
  readonly active: boolean;
  readonly lastResult: string | null;
}

test.describe('The debug routing run', () => {
  test('the page answers while the debug worker works', async ({ page }) => {
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
      () =>
        typeof (window as never as { __debugWorker?: WorkerDebug }).__debugWorker +
        '/' +
        typeof (window as never as { __triggerDebugRouting?: () => void }).__triggerDebugRouting,
    );
    test.skip(surface !== 'object/function', 'this build has no debug worker surface');

    await page.evaluate(() => {
      (window as never as { __triggerDebugRouting: () => void }).__triggerDebugRouting();
    });

    const midRun = await page.waitForFunction(
      () => {
        const worker = (window as never as { __debugWorker: WorkerDebug }).__debugWorker;
        return worker.active ? { answeredAt: performance.now() } : null;
      },
      undefined,
      { timeout: 10_000 },
    );
    const seen = (await midRun.jsonValue()) as { answeredAt: number };
    expect(seen.answeredAt).toBeGreaterThan(0);

    await page.waitForFunction(
      () => {
        const worker = (window as never as { __debugWorker: WorkerDebug }).__debugWorker;
        return !worker.active && worker.lastResult !== null;
      },
      undefined,
      { timeout: 120_000 },
    );

    // The report reached the page, and the panel that reads it is there.
    const stages = await page.evaluate(
      () => (window as never as { __routeDebug?: { stages?: unknown[] } }).__routeDebug?.stages?.length ?? 0,
    );
    expect(stages).toBeGreaterThan(0);
    await expect(page.locator('#status-text')).toContainText('Debug:', { timeout: 10_000 });
  });
});
