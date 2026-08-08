import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

/**
 * A skipped suite nobody wrote down is a coverage claim nobody can check.
 *
 * Three requirements in `.gsd/REQUIREMENTS.md` carried `Status: validated`
 * with a validation line counting E2E tests - "7 E2E tests verify panel
 * lifecycle, hover preview, click selection" - and those suites are
 * `test.describe.skip`, because the Route split-button they drive is
 * `display:none`. The document said the feature was proven by tests that do
 * not run.
 *
 * The requirements file now lists every skipped suite and why. This keeps the
 * two in step: skip a suite without writing it down, or write down one that
 * runs again, and this fails.
 */

const ROOT = join(__dirname, '..', '..', '..');
const E2E = join(ROOT, 'viewer', 'e2e');

/**
 * Suites where the whole file is switched off.
 *
 * `describe.skip` is a suite nobody runs. `test.skip(condition, reason)` inside
 * a test is a runtime guard - `one-checker.spec.ts` uses it to step aside when
 * a build exposes no engine handle - and counting those as disabled coverage
 * would make this test demand documentation for tests that do run.
 */
function skippedSuites(): string[] {
  return readdirSync(E2E)
    .filter(name => name.endsWith('.spec.ts'))
    .filter(name => /(?:test\.)?describe\.skip\s*\(/.test(readFileSync(join(E2E, name), 'utf8')))
    .sort();
}

describe('the requirements name every skipped suite', () => {
  const requirements = readFileSync(join(ROOT, '.gsd', 'REQUIREMENTS.md'), 'utf8');
  /**
   * The registry is the table, not the document.
   *
   * Searching the whole file passes as long as a suite is mentioned anywhere -
   * and each of these is also named in the requirement it used to validate, so
   * deleting its row from the table changed nothing. Measured: with the
   * tuning-panel row removed the first version of this test still passed.
   */
  const gaps = requirements.slice(requirements.indexOf('## Coverage gaps'));

  it('every suite that is skipped is listed in the coverage gaps', () => {
    const listed = skippedSuites().filter(name => !gaps.includes(name));
    expect(
      listed,
      'these suites do not run and REQUIREMENTS.md does not say so',
    ).toEqual([]);
  });

  it('every suite listed as skipped really is skipped', () => {
    const claimed = [...gaps.matchAll(/viewer\/e2e\/([\w-]+\.spec\.ts)/g)].map(m => m[1]);
    expect(claimed.length, 'the coverage gaps table lost its rows').toBeGreaterThan(0);

    const running = skippedSuites();
    const stale = [...new Set(claimed)].filter(name => !running.includes(name));
    expect(
      stale,
      'REQUIREMENTS.md calls these skipped and they run - take them off the list',
    ).toEqual([]);
  });

  it('no requirement claims validation from a suite that is skipped', () => {
    // The specific defect this file exists for. A requirement may cite E2E
    // tests only if the suites behind it run.
    const blocks = requirements.split('\n### ').slice(1);
    const lying = blocks
      .filter(block => /- Status: validated/.test(block))
      .filter(block => skippedSuites().some(suite => block.includes(suite)))
      .map(block => block.split('\n')[0]);
    expect(lying, 'validated by a test that does not run').toEqual([]);
  });
});
