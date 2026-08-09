import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { cypcbLanguage } from '../editor/cypcb-language';

/**
 * The editor colours the language, so it has to know all of it.
 *
 * A keyword the highlighter does not know is written in plain text beside ones
 * that are coloured, which tells a reader it is not part of the language.
 * Measured before this test existed: `outline`, `netclass` and `diffpair` -
 * three whole top-level constructs - plus `use`, and the properties `lcsc`,
 * `path`, `silk`, `point` and `implements`.
 *
 * The list of constructs comes out of the generated grammar, because a second
 * list in this file is a second place to forget.
 */

const GRAMMAR = join(__dirname, '..', '..', '..', 'crates', 'cypcb-parser', 'grammar', 'src', 'grammar.json');

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
    case 'SEQ':
      leadingWords(rule.members?.[0], rules, out);
      break;
    case 'CHOICE':
      for (const member of rule.members ?? []) leadingWords(member, rules, out);
      break;
    default:
      leadingWords(rule.content, rules, out);
  }
}

function topLevelKeywords(): string[] {
  const grammar = JSON.parse(readFileSync(GRAMMAR, 'utf8')) as { rules: Record<string, Rule> };
  const out = new Set<string>();
  for (const member of grammar.rules['_definition'].members ?? []) {
    leadingWords(grammar.rules[member.name!], grammar.rules, out);
  }
  return [...out].sort();
}

describe('the editor knows every keyword', () => {
  it('colours every construct the grammar has', () => {
    const known = new Set([
      ...(cypcbLanguage.keywords as string[]),
      ...((cypcbLanguage as unknown as { properties: string[] }).properties),
    ]);

    const grammar = topLevelKeywords();
    expect(grammar.length).toBeGreaterThan(12);

    const missing = grammar.filter(word => !known.has(word));
    expect(missing, 'the editor writes these in plain text').toEqual([]);
  });

  it('knows the properties a block takes', () => {
    // Not from the grammar: these are the words inside a block, and pulling
    // them out of the generated JSON drags in units and pad shapes too. This
    // list is the reader's own, from `unknown_property` in reader.rs.
    const properties = new Set((cypcbLanguage as unknown as { properties: string[] }).properties);
    for (const word of ['value', 'at', 'rotate', 'lcsc', 'width', 'clearance', 'current',
      'from', 'to', 'path', 'layer', 'via', 'locked', 'bounds', 'stackup',
      'description', 'pad', 'courtyard', 'silk', 'point', 'implements']) {
      expect(properties.has(word), `${word} is a property the editor does not colour`).toBe(true);
    }
  });
});
