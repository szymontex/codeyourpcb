import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * CLAUDE.md calls itself the canonical shortcut registry and asks that every
 * new shortcut be added to it and to the help modal. Ctrl+L reached neither:
 * it simplifies the selected trace, it has worked for as long as the handler
 * has existed, and no user could discover it.
 *
 * This reads the keys the code actually handles and requires the registry to
 * know each one. A shortcut the registry has never heard of is a feature only
 * its author can use.
 */
describe('the shortcut registry knows what the code handles', () => {
  const root = resolve(__dirname, '../../..');
  const registry = readFileSync(resolve(root, 'CLAUDE.md'), 'utf8');
  const sources = ['viewer/src/main.ts', 'viewer/src/interaction.ts'].map((p) =>
    readFileSync(resolve(root, p), 'utf8'),
  );

  /** Every literal key a handler compares against. */
  const handled = new Set<string>();
  for (const source of sources) {
    for (const match of source.matchAll(/e\.key === '([^']+)'/g)) {
      handled.add(match[1]);
    }
  }

  it('finds the handlers at all', () => {
    expect(handled.size, 'no key comparisons found - the scan is broken').toBeGreaterThan(10);
  });

  it('has an entry for every key', () => {
    const missing: string[] = [];
    for (const key of handled) {
      // The registry writes letters as their uppercase shortcut - `Ctrl+E`,
      // `R`, `Shift+R` - so a case-insensitive search for the letter inside a
      // shortcut column is what "documented" means here.
      const needle = key.length === 1 ? key.toUpperCase() : key;
      const documented =
        registry.includes(`\`${needle}\``) ||
        registry.includes(`+${needle}\``) ||
        registry.includes(`${needle}\` /`) ||
        registry.includes(`/ \`${needle}\``);
      if (!documented) missing.push(key);
    }

    expect(missing, `keys the code handles and CLAUDE.md does not list: ${missing.join(', ')}`).toEqual([]);
  });
});
