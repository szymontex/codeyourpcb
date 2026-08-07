import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * The first thing a new user does is follow the README, and it sent them to a
 * port nothing listens on.
 *
 * `vite.config.ts` has served 4321 for as long as the config has existed.
 * `README.md`, `CONTRIBUTING.md` and `viewer/start.sh` all said 5173 - and
 * `start.sh` printed it three lines above Vite's own banner announcing 4321,
 * so the program contradicted itself on screen. A note in
 * `.gsd/milestones/M001/slices/S07/tasks/T03-SUMMARY.md` had recorded the
 * discrepancy and nothing user-facing was corrected.
 *
 * This pins every claim to the config rather than to each other, so the next
 * person to change the port finds out here instead of from a blank browser
 * tab.
 */

const here = dirname(fileURLToPath(import.meta.url));
const viewer = join(here, '..', '..');
const repo = join(viewer, '..');

function read(path: string): string {
  return readFileSync(path, 'utf8');
}

/** The port Vite is configured to serve on. */
function configuredPort(): number {
  const config = read(join(viewer, 'vite.config.ts'));
  const match = config.match(/port:\s*(\d+)/);
  expect(match, 'vite.config.ts has to state a port').not.toBeNull();
  return Number(match![1]);
}

/** Every `localhost:NNNN` a file names, deduplicated. */
function portsNamedIn(path: string): number[] {
  const found = read(path).matchAll(/localhost:(\d+)/g);
  return [...new Set([...found].map((m) => Number(m[1])))];
}

describe('the front door names the port the dev server actually serves', () => {
  const port = configuredPort();

  // The WebSocket server has its own port and legitimately appears beside the
  // dev server's; anything else naming localhost has to be the dev server.
  const websocketPort = 4322;

  it.each([
    ['README.md', join(repo, 'README.md')],
    ['CONTRIBUTING.md', join(repo, 'CONTRIBUTING.md')],
    ['viewer/start.sh', join(viewer, 'start.sh')],
  ])('%s sends the reader to the right place', (_name, path) => {
    const named = portsNamedIn(path).filter((p) => p !== websocketPort);
    expect(named).not.toHaveLength(0);
    for (const p of named) {
      expect(p).toBe(port);
    }
  });

  it('the desktop shell loads the same server', () => {
    // Tauri's devUrl is the one place that was already right, and a port
    // change that missed it would leave the desktop app pointing at nothing.
    const tauri = read(join(repo, 'src-tauri', 'tauri.conf.json'));
    expect(tauri).toContain(`http://localhost:${port}`);
  });
});
