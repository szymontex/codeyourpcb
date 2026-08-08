import { describe, it, expect } from 'vitest';
import {
  collectImportedFiles,
  importedPaths,
  normalisePath,
  resolveAgainst,
} from '../imports';

/**
 * The browser's half of following an `import`.
 *
 * The engine resolves imports and cannot read a file - a browser tab has no
 * disk - so the host fetches and hands it a map of path to text. Getting the
 * keys wrong is silent: the engine asks for `lib/x.cypcb`, the host stored
 * `./lib/x.cypcb`, and the design comes back missing every module with a
 * message about a file nobody can find.
 */

describe('the paths a design imports', () => {
  it('finds both forms', () => {
    const source = [
      'version 1',
      'import "lib/blocks.cypcb"',
      'import Divider, LedDriver from "lib/more.cypcb"',
      'board b { size 10mm x 10mm layers 2 }',
    ].join('\n');

    expect(importedPaths(source)).toEqual(['lib/blocks.cypcb', 'lib/more.cypcb']);
  });

  it('names each file once, however many times it is imported', () => {
    const source = 'import A from "lib/x.cypcb"\nimport B from "lib/x.cypcb"\n';
    expect(importedPaths(source)).toEqual(['lib/x.cypcb']);
  });

  it('does not fetch what a comment mentions', () => {
    const source = [
      '// import "lib/old.cypcb" - this was replaced',
      '/* import "lib/older.cypcb" */',
      'import "lib/current.cypcb"',
    ].join('\n');

    expect(importedPaths(source)).toEqual(['lib/current.cypcb']);
  });

  it('finds nothing in a design that imports nothing', () => {
    expect(importedPaths('board b { layers 2 }')).toEqual([]);
  });
});

describe('the key a file is stored under', () => {
  it('flattens the way the engine flattens', () => {
    // `normalise` in crates/cypcb-parser/src/imports.rs. Disagree with it and
    // the host supplies a file the engine never asks for.
    expect(normalisePath('./lib/x.cypcb')).toBe('lib/x.cypcb');
    expect(normalisePath('lib/../lib/x.cypcb')).toBe('lib/x.cypcb');
    expect(normalisePath('lib//x.cypcb')).toBe('lib/x.cypcb');
    expect(normalisePath('../shared/x.cypcb')).toBe('../shared/x.cypcb');
  });

  it('resolves a path against the file that wrote it', () => {
    expect(resolveAgainst('', 'lib/blocks.cypcb')).toBe('lib/blocks.cypcb');
    expect(resolveAgainst('lib/dot.cypcb', 'shared/tiny.cypcb')).toBe('lib/shared/tiny.cypcb');
    expect(resolveAgainst('lib/nested/dot.cypcb', '../tiny.cypcb')).toBe('lib/tiny.cypcb');
  });
});

describe('collecting what a design imports', () => {
  function readerFor(files: Record<string, string>) {
    const asked: string[] = [];
    const read = async (path: string) => {
      asked.push(path);
      return files[path] ?? null;
    };
    return { read, asked };
  }

  it('follows imports of imports, relative to the file that wrote them', async () => {
    const { read, asked } = readerFor({
      'lib/dot.cypcb': 'import "shared/tiny.cypcb"\nmodule Dot { pin P }',
      'lib/shared/tiny.cypcb': 'footprint TINY { courtyard 1mm x 1mm }',
    });

    const files = await collectImportedFiles('import Dot from "lib/dot.cypcb"', read);

    expect(Object.keys(files).sort()).toEqual(['lib/dot.cypcb', 'lib/shared/tiny.cypcb']);
    expect(asked).toEqual(['lib/dot.cypcb', 'lib/shared/tiny.cypcb']);
  });

  it('asks for each file once, whatever the shape of the graph', async () => {
    // Two libraries importing the same third. Fetching it twice is one wasted
    // request in a browser and a hang in a cycle.
    const { read, asked } = readerFor({
      'a.cypcb': 'import "c.cypcb"',
      'b.cypcb': 'import "c.cypcb"',
      'c.cypcb': 'import "a.cypcb"',
    });

    await collectImportedFiles('import "a.cypcb"\nimport "b.cypcb"', read);

    expect(asked).toEqual(['a.cypcb', 'b.cypcb', 'c.cypcb']);
  });

  it('leaves out what it cannot read rather than inventing it', async () => {
    const { read } = readerFor({ 'lib/here.cypcb': 'module Here { pin P }' });

    const files = await collectImportedFiles(
      'import "lib/here.cypcb"\nimport "lib/gone.cypcb"',
      read,
    );

    expect(Object.keys(files)).toEqual(['lib/here.cypcb']);
  });
});
