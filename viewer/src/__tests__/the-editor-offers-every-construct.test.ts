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

/**
 * Constructs the editor deliberately does not offer, and why.
 *
 * A completion is an invitation. `interface` parses and builds nothing - the
 * tracker has said so since the roadmap work - so offering it would invite a
 * person to write a block that does nothing on a board.
 */
const NOT_OFFERED: Record<string, string> = {
  interface: 'parses and builds nothing yet, so offering it would invite dead text',
};

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
    const missing = constructs.filter(
      (word) => !(word in NOT_OFFERED) && !source.includes(`label: '${word}', insert:`),
    );
    expect(missing, 'constructs the language has and the editor never suggests').toEqual([]);
  });

  it('says why anything is left out', () => {
    // A construct on that list has to be in the grammar, or the reason is
    // about something that no longer exists.
    for (const [word, why] of Object.entries(NOT_OFFERED)) {
      expect(constructs, `${word} is not a construct any more`).toContain(word);
      expect(why.length).toBeGreaterThan(20);
    }
  });
});
