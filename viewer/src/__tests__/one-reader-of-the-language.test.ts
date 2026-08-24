import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

/**
 * One reader of the language, and nothing in the browser is the second.
 *
 * `viewer/src/wasm.ts` used to hold `parseSource` - 439 lines that read
 * `.cypcb` a second time, in TypeScript, so a board could be drawn without a
 * parser in the WASM build. `docs/one-parser.md` carries what that cost: on
 * `v2-imports.cypcb` the browser showed an empty board where the command line
 * saw six parts on seven nets, and on `v2-modules.cypcb` it showed two parts
 * under their in-module names against the command line's seven instantiated.
 *
 * The Rust reader replaced it on 2026-08-07. The width formula has had a guard
 * since the day its second copy was deleted; this one never did, so the claim
 * that there is one parser rested on nobody happening to write another.
 */

const SRC = join(__dirname, '..');
const REPO = join(SRC, '..', '..');

/** Every `.ts` file the viewer ships, tests excluded. */
function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === '__tests__' || entry.name === 'node_modules') continue;
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...sourceFiles(path));
    else if (entry.name.endsWith('.ts')) out.push(path);
  }
  return out;
}

/** The file with its comments removed: only what runs counts. */
function code(text: string): string {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');
}

describe('one reader of the language', () => {
  it('no viewer source parses .cypcb itself', () => {
    // The names the deleted reader used. A comment may say them - this file
    // does, and two in `wasm.ts` record where the call used to be.
    const readers = ['parseSource', 'parseBoard(', 'parseComponent(', 'parseFootprint('];
    const offenders: string[] = [];

    for (const file of sourceFiles(SRC)) {
      const found = readers.filter((name) => code(readFileSync(file, 'utf8')).includes(name));
      if (found.length > 0) {
        offenders.push(`${file.slice(SRC.length + 1)}: ${found.join(', ')}`);
      }
    }

    expect(offenders, 'reading the language belongs to cypcb-parser alone').toEqual([]);
  });

  it('the engine is what turns source into a board', () => {
    const source = readFileSync(join(SRC, 'wasm.ts'), 'utf8');
    expect(
      code(source).includes('load_source_with_imports'),
      'the browser hands source to the engine and takes a snapshot back',
    ).toBe(true);
  });

  it('the write-up says the reader is gone rather than that it is there', () => {
    // `docs/one-parser.md` was written while both readers existed and argued
    // for replacing one. Left in the present tense it tells a reader today
    // that the browser parses the DSL, which stopped being true on 2026-08-07.
    const doc = readFileSync(join(REPO, 'docs', 'one-parser.md'), 'utf8');
    expect(doc).toContain('Deleted on 2026-08-07');
    expect(doc).not.toContain('reads the DSL a second time in TypeScript');
  });
});
