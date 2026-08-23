import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { BLOCK_PROPERTIES } from '../editor/lsp-bridge';

/**
 * The editor offers every word a stack can be written with.
 *
 * Seven pieces of stackup vocabulary landed in the language between
 * 2026-08-22 and 2026-08-23. Measured before this test existed: the editor
 * offered **none** of them. Worse, inside `stackup { }` the block detector
 * answered `board`, so somebody writing a stack was offered `size`, `layers`
 * and `stackup` - three words none of which belong there.
 *
 * The list of words comes out of the generated grammar, because a list in this
 * file is a second place to forget one.
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
  'grammar.json'
);

interface Rule {
  type: string;
  value?: string;
  name?: string;
  members?: Rule[];
  content?: Rule;
}

/** The string literals a rule can begin with. */
function leadingWords(rule: Rule | undefined, rules: Record<string, Rule>, out: Set<string>): void {
  if (!rule) return;
  switch (rule.type) {
    case 'STRING':
      if (rule.value) out.add(rule.value);
      break;
    case 'SYMBOL':
      leadingWords(rules[rule.name!], rules, out);
      break;
    case 'SEQ':
      leadingWords(rule.members?.[0], rules, out);
      break;
    case 'CHOICE':
      for (const member of rule.members ?? []) leadingWords(member, rules, out);
      break;
    case 'FIELD':
      leadingWords(rule.content, rules, out);
      break;
    default:
      leadingWords(rule.content, rules, out);
  }
}

/** Every word a line inside `stackup { }` may start with. */
function stackupWords(): string[] {
  const grammar = JSON.parse(readFileSync(GRAMMAR, 'utf8')) as { rules: Record<string, Rule> };
  const out = new Set<string>();
  for (const rule of [
    'stackup_layer',
    'stackup_sheet',
    'stackup_finish',
    'stackup_edges',
    'stackup_pads',
    'stackup_connector',
    'stackup_impedance',
    'stackup_drill',
  ]) {
    leadingWords(grammar.rules[rule], grammar.rules, out);
  }
  return [...out].sort();
}

describe('the editor offers every stackup word', () => {
  it('reads more than a handful of words out of the grammar', () => {
    // The guard on the guard: a selector that matched nothing would make every
    // assertion below pass while proving nothing.
    const words = stackupWords();
    expect(words.length).toBeGreaterThanOrEqual(14);
  });

  it('offers each of them inside a stackup block', () => {
    const offered = new Set(BLOCK_PROPERTIES.stackup.map((entry) => entry.label));
    const missing = stackupWords().filter((word) => !offered.has(word));
    expect(missing, `the editor offers no completion for these: ${missing}`).toEqual([]);
  });

  it('offers nothing there that the grammar does not have', () => {
    // The other direction: a completion for a word the language refuses is
    // worse than none, because the editor taught it.
    const words = new Set(stackupWords());
    const invented = BLOCK_PROPERTIES.stackup
      .map((entry) => entry.label)
      .filter((label) => !words.has(label));
    expect(invented, `the editor invents these: ${invented}`).toEqual([]);
  });

  it('gives every one of them a snippet that starts with the word itself', () => {
    for (const entry of BLOCK_PROPERTIES.stackup) {
      expect(entry.snippet.startsWith(entry.label), `${entry.label}: ${entry.snippet}`).toBe(true);
      expect(entry.detail.length, `${entry.label} has no detail`).toBeGreaterThan(0);
    }
  });
});
