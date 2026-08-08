import { describe, it, expect } from 'vitest';
import { readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/**
 * A setup script that misses one tool is a machine that fails on stage 6.
 *
 * The browser's system libraries were installed into a container by hand, so
 * the next recreate took the gate's playwright stage with it. `wasm-opt`
 * became required by the wasm build on the day the build stopped going through
 * wasm-pack. Neither was written anywhere a person could run, and both were
 * found the same way: something broke and somebody remembered.
 *
 * `scripts/setup-dev.sh` is that place now, and this test is what stops it
 * drifting from the scripts it serves: every tool the gate or the wasm build
 * checks for with `command -v` has to be named in the setup script.
 */

const ROOT = join(__dirname, '..', '..', '..');

function read(relative: string): string {
  return readFileSync(join(ROOT, relative), 'utf8');
}

/** Every tool a script refuses to run without. */
function requiredTools(script: string): string[] {
  const tools = new Set<string>();
  for (const match of script.matchAll(/command -v (\w[\w-]*)/g)) {
    tools.add(match[1]);
  }
  return [...tools];
}

describe('the setup script installs what the build needs', () => {
  const setup = read('scripts/setup-dev.sh');

  it('names every tool the wasm build refuses to run without', () => {
    const needed = requiredTools(read('viewer/build-wasm.sh'));
    expect(needed.length).toBeGreaterThan(1);

    // Whole words: `setup.includes('wasm-opt')` is also true of a script that
    // only ever says `wasm-opti`, and a guard that cannot tell those apart is
    // a guard that passes on a typo.
    const missing = needed.filter(tool => !new RegExp(`\\b${tool}\\b`).test(setup));
    expect(missing, 'the wasm build checks for these and setup-dev.sh never mentions them').toEqual(
      [],
    );
  });

  it('covers the gate stages that need something installed', () => {
    const gate = read('scripts/quality-gate.sh');
    // The gate runs these; each needs something the setup script provides.
    expect(gate).toContain('playwright');
    expect(setup).toContain('playwright install-deps');
    expect(setup).toContain('playwright install chromium');

    expect(gate).toContain('vitest');
    expect(setup).toContain('npm install');
  });

  it('pins wasm-bindgen to the version the workspace resolves', () => {
    // A CLI newer or older than the crate writes bindings the module rejects
    // when the page loads it, and the failure looks like a broken app rather
    // than a broken toolchain.
    expect(setup).toContain('Cargo.lock');
    expect(setup).toContain('cargo install wasm-bindgen-cli');
    expect(setup).toMatch(/--version "\$PINNED"/);
  });

  it('is executable, because a setup script nobody can run is a document', () => {
    const mode = statSync(join(ROOT, 'scripts/setup-dev.sh')).mode;
    expect(mode & 0o111, 'chmod +x scripts/setup-dev.sh').toBeGreaterThan(0);
  });
});
