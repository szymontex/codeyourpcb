import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * What the dev server says about itself has to be what it does.
 *
 * Its own doc comment said "no authentication and no origin check" for as long
 * as the origin check existed above it, and a run of this project's own
 * heartbeat read that sentence, believed it, and wrote a next action to add a
 * guard that was already there. A stale comment costs a reader a fire.
 *
 * The first version of this file searched for phrases and passed against all
 * three mutations written for it: the phrase it hunted was split across two
 * comment lines, `realpathSync` appeared in an import whether or not anything
 * called it, and "the block above the function" was found by taking the last
 * `/**` in a slice, which is a different block. What is checked now is the
 * shape of the file rather than the presence of words in it.
 */

const SERVER = join(__dirname, '..', '..', 'server.ts');

/** The file with comment furniture removed, so a claim split over two lines reads as one. */
function prose(text: string): string {
  return text
    .split('\n')
    .map((line) => line.replace(/^\s*\*\s?/, '').trim())
    .join(' ')
    .replace(/\s+/g, ' ');
}

/** The doc block immediately above `name`, with nothing but blank lines between. */
function docBlockAbove(text: string, name: string): string {
  const at = text.indexOf(name);
  expect(at, `${name} is in the file`).toBeGreaterThan(-1);
  const before = text.slice(0, at).trimEnd();
  expect(before.endsWith('*/'), `${name} has a doc block immediately above it`).toBe(true);
  const opens = before.lastIndexOf('/**');
  return before.slice(opens);
}

describe('the dev server describes the guards it has', () => {
  const source = readFileSync(SERVER, 'utf-8');

  it('does not claim to have no origin check while it has one', () => {
    expect(source, 'the guard itself').toContain('function allowedOrigin(');
    // Read as one line, so a claim broken over two comment lines still reads
    // as the claim it is - which is how the first version of this test missed
    // its own mutation.
    const flat = prose(source);
    const claims = flat.split('used to open with was')[0];
    expect(
      claims.includes('no authentication and no origin check'),
      'the file claims to have no origin check and defines one',
    ).toBe(false);
  });

  it('says it resolves links while it resolves them', () => {
    // The call, not the import: an import stays behind when the code that used
    // it goes, and the first version of this test was satisfied by the import.
    const resolvesLinks = /realpathSync\(/.test(source.replace(/^import .*$/gm, ''));
    expect(resolvesLinks, 'the guard resolves links').toBe(true);
    expect(prose(source), 'and the prose beside it says so').toContain('symlink');
  });

  it('names the other guard where a reader meets this one', () => {
    // A reader who finds one guard stops looking, so the block above the path
    // guard has to name the origin guard. Taken as the block immediately above
    // the function rather than as the last comment in a slice: something was
    // once inserted between the two, and the comment stayed where it was.
    const block = docBlockAbove(source, 'function insideWatchDir(');
    expect(prose(block), 'the block above the path guard').toContain('allowedOrigin');
    expect(prose(block), 'and it is the block that describes it').toContain(
      'The file a client asked for',
    );
  });
});
