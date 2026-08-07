import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * Two things the desktop build scripts got wrong, and neither could be caught
 * by compiling anything.
 *
 * **They ran Tauri from the wrong directory.** The CLI looks for
 * `tauri.conf.json` in the current folder or below it, and `src-tauri` is a
 * sibling of `viewer`, not a child - so `npm run build:desktop` died with
 * "Couldn't recognize the current folder as a Tauri project" on any machine,
 * with or without the system libraries. The desktop build had never worked
 * from these scripts.
 *
 * **They printed success either way.** `build-linux.sh` ran the build, ignored
 * its exit code, printed "Build complete!" and exited 0, then listed the
 * installers as "(not created)". Anything chaining on it saw a pass.
 *
 * Neither is checkable by the compiler and the crate they build cannot even be
 * compiled here, so this reads the scripts as text.
 */

const here = dirname(fileURLToPath(import.meta.url));
const viewer = join(here, '..', '..');
const repo = join(viewer, '..');

function read(path: string): string {
  return readFileSync(path, 'utf8');
}

describe('the desktop scripts do what they print', () => {
  const pkg = JSON.parse(read(join(viewer, 'package.json'))) as {
    scripts: Record<string, string>;
  };

  it('the Tauri project is where the CLI has to be run from', () => {
    // The premise the scripts depend on: this is a sibling of viewer, so a
    // Tauri command started inside viewer cannot find it.
    expect(existsSync(join(repo, 'src-tauri', 'tauri.conf.json'))).toBe(true);
    expect(existsSync(join(viewer, 'src-tauri'))).toBe(false);
  });

  it.each(['dev:desktop', 'build:desktop'])(
    '`%s` leaves viewer/ before running tauri',
    (name) => {
      const script = pkg.scripts[name];
      expect(script, `${name} has to exist`).toBeTruthy();
      expect(script).toContain('tauri');
      expect(
        script.includes('cd ..'),
        `${name} runs \`${script}\` from viewer/, where tauri cannot find src-tauri`
      ).toBe(true);
    }
  );

  it.each([
    ['build-linux.sh', 'Build FAILED'],
    ['build-macos.sh', 'Build FAILED'],
    ['build-windows.bat', 'Build FAILED'],
  ])('%s reports a failed build as failed', (name, marker) => {
    const script = read(join(repo, name));
    expect(script).toContain(marker);
    // And says so before claiming otherwise.
    expect(script.indexOf(marker)).toBeLessThan(script.indexOf('Build complete!'));
  });

  it('the shell build scripts stop rather than continue after a failure', () => {
    for (const name of ['build-linux.sh', 'build-macos.sh']) {
      const script = read(join(repo, name));
      expect(script, `${name} has to check the build's exit status`).toMatch(
        /if !\s*npm run build:desktop/
      );
      expect(script).toContain('exit 1');
    }
  });
});
