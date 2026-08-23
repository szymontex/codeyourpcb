import { defineConfig, devices } from '@playwright/test';

/**
 * The port the e2e run serves the viewer on.
 *
 * Not 4321. That is Astro's default, and this container runs other Astro
 * projects: a gate run failed with `http://localhost:4321 is already used`
 * because a different repository's dev server held it, and the alternative
 * that config offered - `reuseExistingServer` - is worse than failing, since
 * it would have run the viewer's own e2e suite against somebody else's app.
 *
 * `--strictPort` so a busy port fails here rather than silently moving the
 * server somewhere the tests are not looking.
 */
const PORT = Number(process.env.CYPCB_E2E_PORT ?? 4327);

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false, // serial — WASM + canvas state is shared
  retries: 0,
  reporter: 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
    headless: true,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `npm run dev -- --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
