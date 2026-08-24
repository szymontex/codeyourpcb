import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * The editor offers every construct the language has, not most of them.
 *
 * `the-editor-knows-every-keyword` holds the **highlighter** against the
 * grammar, and its own header records what that found: `outline`, `netclass`
 * and `diffpair` - three whole top-level constructs - written in plain text
 * beside coloured ones. The completion list is a second list with the same
 * failure mode and nothing held it: measured on 2026-08-24 it offered ten of
 * the seventeen words a file can start a definition with. `flex` was among the
 * seven missing, on the day the checker, both renderers and the hover card all
 * knew it.
 *
 * `interface` was the seventeenth, and it was left out on purpose for a reason
 * that had stopped being true - see the case below.
 *
 * The list of constructs comes out of the generated grammar, because a second
 * list in this file is a second place to forget.
 */

const GRAMMAR = join(
  __dirname,
  '..',
  '..',
  '..',
  'crates',
  'cypcb-parser',
  'grammar',
  'src',
  'grammar.json',
);
const BRIDGE = join(__dirname, '..', 'editor', 'lsp-bridge.ts');

interface Rule {
  type: string;
  value?: string;
  name?: string;
  members?: Rule[];
  content?: Rule;
}

/** The words a rule can begin with. */
function leadingWords(
  rule: Rule | undefined,
  rules: Record<string, Rule>,
  out: Set<string>,
  seen = new Set<string>(),
): void {
  if (!rule) return;
  if (rule.type === 'STRING' && rule.value) {
    out.add(rule.value);
    return;
  }
  if (rule.type === 'SYMBOL' && rule.name) {
    if (seen.has(rule.name)) return;
    seen.add(rule.name);
    leadingWords(rules[rule.name], rules, out, seen);
    return;
  }
  if (rule.type === 'SEQ' && rule.members) {
    for (const member of rule.members) {
      leadingWords(member, rules, out, seen);
      if (member.type === 'STRING' || member.type === 'SYMBOL' || member.type === 'PATTERN') break;
    }
    return;
  }
  if (rule.type === 'CHOICE' && rule.members) {
    for (const member of rule.members) leadingWords(member, rules, out, seen);
    return;
  }
  leadingWords(rule.content, rules, out, seen);
}

describe('the completion list', () => {
  const grammar = JSON.parse(readFileSync(GRAMMAR, 'utf8')) as { rules: Record<string, Rule> };
  const source = readFileSync(BRIDGE, 'utf8');

  const words = new Set<string>();
  leadingWords(grammar.rules['source_file'], grammar.rules, words);
  const constructs = [...words].filter((word) => /^[a-z]+$/.test(word)).sort();

  it('reads the constructs out of the grammar', () => {
    // Guard the guard: a reader that finds nothing would pass every case below.
    expect(constructs.length).toBeGreaterThanOrEqual(15);
    expect(constructs).toContain('board');
    expect(constructs).toContain('flex');
  });

  it('offers every construct a file can start with', () => {
    // Every one of them, with no exceptions. The first version of this test
    // carried a list of words left out on purpose and `interface` was on it,
    // on the grounds that it "parses and builds nothing" - which the tracker
    // had said since the roadmap work and which is false: `cypcb check` on a
    // module that implements an interface without exposing all of its pins
    // says `module 'Sensor' implements 'I2C' without pin SCL` and the example
    // documents that failure in its own header. A reason nobody re-measured is
    // how a construct stays hidden after the objection to it stops being true.
    const missing = constructs.filter(
      (word) => !source.includes(`label: '${word}', insert:`),
    );
    expect(missing, 'constructs the language has and the editor never suggests').toEqual([]);
  });
});
