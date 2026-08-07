import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * Two faults a setup script can carry that no compiler will ever see.
 *
 * **A developer's own path.** `setup-linux.sh` step 6 read
 * `cd /workspace/codeyourpcb` - the directory this project happens to be
 * checked out in on one build host. On anyone else's machine that line ends
 * the script, and it is the last step before the summary that tells the reader
 * what to run next.
 *
 * **A branch `set -e` makes unreachable.** The same step ran `cargo check`,
 * then `if [ $? -eq 0 ]`. With `set -e` a failing check exits the shell before
 * the test runs, so the "[WARN] ... may need additional dependencies" advice
 * could never print - on exactly the machine that needed it. Demonstrated:
 * `set -e; false; if [ $? -eq 0 ]; then ...; fi; echo AFTER` prints nothing
 * and exits 1.
 *
 * Neither is visible to `cargo` or `tsc`, and the crate these scripts build
 * cannot be compiled in CI, so this reads them as text.
 */

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..', '..');

const SCRIPTS = [
  'setup-linux.sh',
  'setup-macos.sh',
  'setup-windows.bat',
  'dev-linux.sh',
  'dev-macos.sh',
  'dev-windows.bat',
  'build-linux.sh',
  'build-macos.sh',
  'build-windows.bat',
];

function read(name: string): string {
  return readFileSync(join(repo, name), 'utf8');
}

/**
 * The same file with its comments dropped.
 *
 * The first version of this test read the whole file and failed on the comment
 * explaining the very bug it checks for - a test that cannot tell code from
 * prose about code.
 */
function code(name: string): string {
  return read(name)
    .split('\n')
    .filter((line) => !/^\s*(#|::|rem\b)/i.test(line))
    .join('\n');
}

describe('the setup scripts run on a stranger\'s machine', () => {
  it.each(SCRIPTS)('%s carries no absolute path from a developer box', (name) => {
    const script = code(name);
    // Anything rooted outside the checkout: a home directory, a build host's
    // workspace, a Windows drive letter. `$HOME/.cargo/env` is fine - it is
    // resolved on the reader's machine, not on ours.
    const suspects = [
      ...script.matchAll(/(?:^|\s)(\/(?:home|Users|workspace|root)\/\S+)/g),
      ...script.matchAll(/(?:^|\s)([A-Z]:\\\S+)/g),
    ].map((m) => m[1]);

    expect(suspects, `${name} names paths that only exist somewhere else`).toEqual([]);
  });

  it.each(SCRIPTS.filter((name) => name.endsWith('.sh')))(
    '%s does not test $? in a script that exits on failure',
    (name) => {
      const script = code(name);
      if (!/^set -e/m.test(script)) {
        return;
      }
      expect(
        /\$\?\s*-eq/.test(script),
        `${name} runs under \`set -e\`, so \`if [ $? -eq 0 ]\` is a branch that ` +
          'can never take its else arm - put the command in the condition'
      ).toBe(false);
    }
  );

  it('setup-linux.sh moves to its own directory before using relative paths', () => {
    const script = read('setup-linux.sh');
    expect(script).toMatch(/cd "\$\(dirname "\$0"\)"/);
    // And does it before the first relative `cd`.
    expect(script.indexOf('cd "$(dirname "$0")"')).toBeLessThan(script.indexOf('cd viewer'));
  });
});
