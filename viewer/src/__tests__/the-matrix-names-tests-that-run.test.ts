import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * The editor half of the feature matrix, tied to the suites that prove it.
 *
 * `docs/competition-feature-matrix.md` compares this project with nine other
 * tools, and the run that corrected its language rows could not measure the
 * rows about the viewer: a command line cannot see a ratsnest, a layer
 * checkbox or a 3D scene. Playwright can, and each of those rows already has
 * a suite driving it.
 *
 * So the claim and its evidence are held together here. A row may not read as
 * missing while a suite exercises it, and the suite named beside it has to be
 * one that runs - `describe.skip` is how three other suites in this directory
 * stopped running while the document that cited them said nothing.
 */

const ROOT = join(__dirname, '..', '..', '..');
const E2E = join(ROOT, 'viewer', 'e2e');

/** A row of the matrix, and the test that says it is true. */
interface Row {
  /** The feature column, exactly as the table writes it. */
  feature: string;
  /** The suite in `viewer/e2e` that drives it. */
  spec: string;
  /** Enough of a test title to fail when that test is renamed away. */
  title: string;
}

const ROWS: Row[] = [
  {
    feature: 'Interactive trace routing',
    spec: 'routing-ux.spec.ts',
    title: 'complete route pad-to-pad',
  },
  {
    feature: 'Ratsnest display',
    spec: 'board-interaction.spec.ts',
    title: 'ratsnest checkbox toggles',
  },
  {
    feature: 'Layer visibility toggle',
    spec: 'board-interaction.spec.ts',
    title: 'top layer checkbox toggles off and on',
  },
  {
    feature: 'DRC markers on canvas',
    spec: 'errors.spec.ts',
    title: 'DRC violations show error badge',
  },
  {
    feature: '3D board visualization',
    spec: 'three-d-view.spec.ts',
    title: '3D button activates Three.js renderer',
  },
  {
    feature: 'Layer stack-up view',
    spec: 'stack-panel.spec.ts',
    title: 'it shows a row per stackup entry',
  },
];

/** The CodeYourPCB cell of the row with this feature name. */
function ourCell(matrix: string, feature: string): string {
  const cells = matrix
    .split('\n')
    .filter(line => line.startsWith('|'))
    .map(line => line.split('|'))
    .filter(fields => fields.length > 3 && fields[1].trim() === feature);
  expect(cells.length, `the matrix has no single row named ${feature}`).toBe(1);
  return cells[0][2].trim();
}

describe('the matrix names tests that run', () => {
  const matrix = readFileSync(
    join(ROOT, 'docs', 'competition-feature-matrix.md'),
    'utf8',
  );

  it.each(ROWS)('$feature is claimed and $spec proves it', row => {
    const claim = ourCell(matrix, row.feature);
    expect(
      claim.startsWith('❌'),
      `${row.feature} reads ${claim} while ${row.spec} drives it`,
    ).toBe(false);

    const source = readFileSync(join(E2E, row.spec), 'utf8');
    expect(
      source.includes(row.title),
      `${row.spec} has no test called ${row.title}, so the row above rests on nothing`,
    ).toBe(true);
    expect(
      /(?:test\.)?describe\.skip\s*\(/.test(source),
      `${row.spec} does not run, so ${row.feature} is a claim nothing checks`,
    ).toBe(false);
  });
});
