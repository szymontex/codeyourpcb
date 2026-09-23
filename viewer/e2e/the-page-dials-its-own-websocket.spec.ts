import { test, expect } from '@playwright/test';

/**
 * The page dials the WebSocket port its run was given, never the default.
 *
 * `getWsUrl` in `src/main.ts` had 4322 written in, so an e2e page served on its
 * own port still connected to whatever `server.ts` held 4322 - a developer's,
 * or one kept up for a demo. That server pushed its board into the page and
 * the suite failed on a tree with nothing wrong in it.
 *
 * No server is started for this: the page is expected to dial a port nothing
 * listens on. A refused connection never reaches Playwright's `websocket`
 * event, so the constructor itself is watched.
 */
test('the page dials the e2e WebSocket port and not the dev default', async ({ page }) => {
  const wsPort = process.env.CYPCB_E2E_WS_PORT;
  expect(wsPort, 'playwright.config.ts sets CYPCB_E2E_WS_PORT').toBeTruthy();

  await page.addInitScript(() => {
    const dialled: string[] = [];
    (window as unknown as { __dialled: string[] }).__dialled = dialled;
    const Native = window.WebSocket;
    window.WebSocket = class extends Native {
      constructor(url: string | URL, protocols?: string | string[]) {
        dialled.push(String(url));
        super(url, protocols);
      }
    };
  });

  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

  const dialled = () =>
    page.evaluate(() => (window as unknown as { __dialled: string[] }).__dialled);

  await expect
    .poll(async () => (await dialled()).some((url) => new URL(url).port === wsPort), {
      message: `the page never dialled ${wsPort}`,
    })
    .toBe(true);
  const toDefault = (await dialled()).filter((url) => new URL(url).port === '4322');
  expect(toDefault, 'the page dialled the dev server default').toEqual([]);
});
