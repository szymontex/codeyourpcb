import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { BLOCK_PROPERTIES } from '../editor/lsp-bridge';

/**
 * The editor offers every word a zone block can be written with.
 *
 * `stitch`, `radius` and `hatch` each landed in the language and then waited
 * for somebody to remember the completion list: `stitch` waited months, and
 * `hatch` was written, filled, checked and drawn in the browser before the
 * editor heard about it. The stackup block has had a census like this since
 * its own seven words went missing; the zone block never did.
 *
 * The list comes out of the generated grammar, because a list in this file is
 * a second place to forget one.
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

/** Every word a line inside a zone, keepout, flex or region block may start with. */
function zoneWords(): string[] {
  const grammar = JSON.parse(readFileSync(GRAMMAR, 'utf8')) as { rules: Record<string, Rule> };
  const out = new Set<string>();
  leadingWords(grammar.rules.zone_property, grammar.rules, out);
  return [...out].sort();
}

describe('the editor offers every zone word', () => {
  it('reads the words out of the grammar rather than out of this file', () => {
    // The guard on the guard: a selector that matched nothing would make the
    // assertions below pass while proving nothing.
    expect(zoneWords().length).toBeGreaterThanOrEqual(5);
  });

  it('offers each of them inside a zone block', () => {
    const offered = new Set(BLOCK_PROPERTIES.zone.map((entry) => entry.label));
    const missing = zoneWords().filter((word) => !offered.has(word));
    expect(missing, `the editor offers no completion for these: ${missing}`).toEqual([]);
  });

  it('offers nothing there the language refuses', () => {
    const words = new Set(zoneWords());
    const invented = BLOCK_PROPERTIES.zone
      .map((entry) => entry.label)
      .filter((label) => !words.has(label));
    expect(invented, `the editor invents these: ${invented}`).toEqual([]);
  });

  it('gives every one of them a snippet that starts with the word itself', () => {
    for (const entry of BLOCK_PROPERTIES.zone) {
      expect(entry.snippet.startsWith(entry.label), `${entry.label}: ${entry.snippet}`).toBe(true);
      expect(entry.detail.length, `${entry.label} has no detail`).toBeGreaterThan(0);
    }
  });
});
