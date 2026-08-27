import { test, expect } from '@playwright/test';

/**
 * Cancel stops the run rather than hiding it.
 *
 * While routing was synchronous the cancel button could not do anything: the
 * engine had the thread, the click could not be delivered until it gave the
 * thread back, and the handler's own comment said cancel was "only meaningful
 * for async routing". It flipped the overlay off and the run finished anyway.
 * R203 is the escape hatch a person needs on a board that takes minutes.
 *
 * Terminating the worker is the whole mechanism - wasm has no cooperative
 * preemption - so what this checks is that nothing arrives afterwards: no
 * copper, no result, no late overwrite of the board the user is looking at.
 */

interface RoutingDebug {
  readonly active: boolean;
  readonly lastResult: string | null;
}

test.describe('Cancelling a routing run', () => {
  test('nothing arrives after the cancel', async ({ page }) => {
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
      () => typeof (window as never as { __routingWorker?: RoutingDebug }).__routingWorker,
    );
    test.skip(surface !== 'object', 'this build has no routing debug surface');

    const before = await page.evaluate(() => {
      const snapshot = (window as never as {
        __pcbEngine: { get_snapshot(): { traces?: unknown[] } };
      }).__pcbEngine.get_snapshot();
      return snapshot.traces?.length ?? 0;
    });

    // Start and cancel in one turn of the event loop, so there is no window in
    // which a fast board could finish and turn this into a race. The button is
    // inside the anchor D5 hides, so it is clicked rather than pressed.
    await page.evaluate(() => {
      (window as never as { __triggerRouting: () => void }).__triggerRouting();
      (document.querySelector('#cancel-route-btn') as HTMLButtonElement).click();
    });

    await expect(page.locator('#status-text')).toContainText('cancelled', { timeout: 5_000 });

    // Long enough for the abandoned run to have finished had it been left
    // alive: the same board routes in under two seconds in this suite.
    await page.waitForTimeout(5_000);

    const after = await page.evaluate(() => {
      const worker = (window as never as { __routingWorker: RoutingDebug }).__routingWorker;
      const snapshot = (window as never as {
        __pcbEngine: { get_snapshot(): { traces?: unknown[] } };
      }).__pcbEngine.get_snapshot();
      return { active: worker.active, lastResult: worker.lastResult, traces: snapshot.traces?.length ?? 0 };
    });

    expect(after.active).toBe(false);
    expect(after.lastResult).toBeNull();
    expect(after.traces).toBe(before);
  });
});
