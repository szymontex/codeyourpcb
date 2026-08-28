import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * No module in the viewer is written and never imported.
 *
 * A worker counts as imported: `main.ts` spawns one with
 * `new Worker(new URL('./routing-worker.ts', import.meta.url))`, which is a
 * reference by path rather than by `import`, and the scan below reads both.
 * Only `main.ts` itself is reached from outside the source tree.
 *
 * `walkaround.ts` was 680 lines of hull-based obstacle avoidance that nothing
 * imported, no test exercised and `docs/trace-routing.md` had already marked
 * "legacy, not currently used - dodge.ts replaced it". It was found by
 * counting, not by reading, and the same count is worth keeping: dead
 * TypeScript type-checks, lints and ships in every bundle exactly like the
 * live kind.
 *
 * Two modules are unimported on purpose and are listed here rather than
 * skipped, so the list has to be edited when the reason changes.
 */
const SRC = join(__dirname, '..');



function modules(): string[] {
  return readdirSync(SRC)
    .filter((name) => name.endsWith('.ts') && !name.endsWith('.d.ts'))
    .map((name) => name.replace(/\.ts$/, ''));
}

/** Every `.ts` file under the viewer's source, including tests and editor. */
function sources(): string[] {
  const found: string[] = [];
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith('.ts')) found.push(path);
    }
  };
  walk(SRC);
  return found;
}

describe('every module the viewer carries', () => {
  it('is imported by something, or is a named entry point', () => {
    const files = sources().map((path) => ({ path, text: readFileSync(path, 'utf8') }));

    const unimported = modules().filter((name) => {
      const own = join(SRC, `${name}.ts`);
      // An import by path, or a worker spawned by URL - both are references.
      const pattern = new RegExp(`['"\`][./]*${name}(\\.ts)?['"\`]`);
      return !files.some((file) => file.path !== own && pattern.test(file.text));
    });

    // Empty, and it has no exception list on purpose: a list of exceptions
    // with no reason beside each one becomes a place to put things, which is
    // how 680 unimported lines survived a year of green runs. `main.ts` is
    // reached from `index.html` and read by name in another test, and both of
    // those count as references here.
    expect(unimported.sort()).toEqual([]);
  });
});
