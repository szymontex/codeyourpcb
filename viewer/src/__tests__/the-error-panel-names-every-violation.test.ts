/**
 * The error panel has a name for every violation the checker can report.
 *
 * `npx vitest run src/__tests__/the-error-panel-names-every-violation.test.ts`
 *
 * The panel keeps a table of slug -> icon and label, and anything missing from
 * it falls back to the slug. That fallback is silent, so the table drifted:
 * the checker grew `trace-current`, `unrouted-pin`, `pour-island`,
 * `assertion` and `diff-pair-skew`, and the panel showed those five as a grey
 * triangle next to raw text like "diff-pair-skew" while every older kind read
 * as a sentence.
 *
 * The truth is the `Display` implementation of `ViolationKind` in
 * `crates/cypcb-drc/src/violation.rs` - the exact strings the engine puts in
 * the snapshot. This reads them out of the Rust source, so adding a rule
 * without naming it fails here rather than in front of a person.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { VIOLATION_KIND_META, describeViolationKind } from '../violation-kinds';

/** Every slug `ViolationKind`'s `Display` can write. */
function kindsTheEngineEmits(): string[] {
  const source = readFileSync(
    resolve(__dirname, '../../../crates/cypcb-drc/src/violation.rs'),
    'utf-8',
  );

  const kinds = [...source.matchAll(/ViolationKind::\w+ => write!\(f, "([a-z-]+)"\)/g)].map(
    (match) => match[1],
  );

  return [...new Set(kinds)];
}

describe('the error panel', () => {
  it('reads the kinds out of the checker', () => {
    // Guard the guard: a rename in the Rust source that this regex stops
    // matching would make every assertion below pass against an empty list.
    const kinds = kindsTheEngineEmits();
    expect(kinds.length).toBeGreaterThanOrEqual(18);
    expect(kinds).toContain('clearance');
  });

  it('has a name and an icon for every one of them', () => {
    const unnamed = kindsTheEngineEmits().filter((kind) => !(kind in VIOLATION_KIND_META));

    expect(
      unnamed,
      `these arrive in the panel as a raw slug: ${unnamed.join(', ')}`,
    ).toEqual([]);
  });

  it('names nothing the checker cannot report', () => {
    const emitted = new Set(kindsTheEngineEmits());
    const ghosts = Object.keys(VIOLATION_KIND_META).filter((kind) => !emitted.has(kind));

    expect(ghosts, `the panel names kinds that no rule reports: ${ghosts.join(', ')}`).toEqual([]);
  });

  it('gives a label that reads as words, not as a slug', () => {
    // A label equal to its own slug is the fallback wearing a table entry's
    // clothes, and it is what the five missing kinds looked like.
    const slugLabels = Object.entries(VIOLATION_KIND_META)
      .filter(([kind, meta]) => meta.label === kind)
      .map(([kind]) => kind);

    expect(slugLabels).toEqual([]);
  });

  it('still says something about a kind it has never heard of', () => {
    const unknown = describeViolationKind('rule-added-tomorrow');

    expect(unknown.label).toBe('rule-added-tomorrow');
    expect(unknown.icon).toBeTruthy();
  });
});
