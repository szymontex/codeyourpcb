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

/**
 * The port the e2e page's WebSocket dials, chosen so that nothing listens.
 *
 * The client dialled 4322 whatever served it, so a run with `npm start` up
 * elsewhere in the container connected every spec to that server, which
 * pushed its own board into the page and hid the project manager. The specs
 * failed on a tree with nothing wrong in it. `vite.config.ts` hands
 * `CYPCB_WS_PORT` to the client, and it is the variable `server.ts` listens on.
 */
const WS_PORT = Number(process.env.CYPCB_E2E_WS_PORT ?? 4328);
// Set here so the specs can check the page dialled this port and no other.
process.env.CYPCB_E2E_WS_PORT = String(WS_PORT);

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false, // serial — WASM + canvas state is shared

  // Two browsers at a time, and the line has to be here or there are six.
  //
  // `fullyParallel: false` serialises the tests inside one file and nothing
  // else: files still run across workers, and with `workers` unset Playwright
  // takes half the cores. On a twelve-core host that is six headless browsers,
  // each with its own renderer and GPU process, on a machine that is also
  // compiling Rust and running somebody else's containers. The config said
  // serial and behaved six ways.
  //
  // Two rather than one: the stage is a hundred and twenty-six specs and one
  // worker makes the gate a great deal longer for a saving nobody asked for.
  // Two keeps the peak at two browsers, which is what this host can spare.
  //
  // Three since 2026-09-24, by the owner's call: the test stage no longer
  // takes every core (the gate caps it at five threads from the host's load),
  // which leaves room for a third browser, and this stage was the longest one
  // left in the gate.
  workers: 3,
  // One retry, and it is a report rather than a cover.
  //
  // Stage 7 of the quality gate failed on three consecutive runs of one tree -
  // four specs, then one, then two, a different subset each time - while every
  // failing spec passed when run alone. Nothing leaves the machine here; every
  // external call is mocked with `page.route`. What these specs assume is a
  // machine that is not otherwise busy, and this one is shared.
  //
  // With a retry, a spec that passes on the second attempt is reported as
  // flaky and the run still ends green, so the gate says "everything passed,
  // and these needed a second go" instead of answering differently every time
  // it is asked. A stage that answers differently run to run is a stage people
  // stop believing - and so is one that hides the second attempt, which is why
  // the flaky list is printed rather than swallowed.
  retries: 1,
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
    env: { CYPCB_WS_PORT: String(WS_PORT) },
    port: PORT,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
