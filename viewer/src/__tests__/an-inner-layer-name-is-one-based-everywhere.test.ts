import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

import { layerMaskBit, innerLayerIndex } from '../layers';

/**
 * An inner layer is `Inner(0)` in Rust and `Inner1` in every name a person
 * reads, and the boundary between the two counts is where an off-by-one
 * lives.
 *
 * `Layer::to_copper_mask` gives the first inner layer bit 2 and
 * `Layer`'s own `Display` writes it as `Inner1`, so every reader here has to
 * subtract the one back out before it indexes anything. Four places in this
 * directory match the name with a regular expression; the DRC's prose says
 * what happens when copies of a layer numbering drift - it gets fixed in one
 * and left standing in the other.
 */

const SRC = join(__dirname, '..');

/** Sites matching an inner layer name, today 5. */
const MATCH_SITES_FLOOR = 4;

/**
 * A site that reads the captured number without subtracting one, and why it
 * is right to - keyed by file and the function the site is in.
 *
 * It was keyed by line number, and an edit fifteen lines above the site moved
 * it from 583 to 598 and failed the gate on a tree with nothing wrong in it.
 * A function's name moves only when somebody renames the function, and an
 * entry that no longer names a site is itself a failure, so a rename cannot
 * leave an exception pointing at nothing.
 */
const KEEPS_THE_NAMES_NUMBER: Record<string, string> = {
  'layers.ts#layerDepth':
    'sorts by depth, where the name\'s own number is already the order',
};

/** The function a line sits in: the nearest `function name(` above it. */
function enclosingFunction(lines: readonly string[], index: number): string {
  for (let at = index; at >= 0; at--) {
    const found = /\bfunction\s+(\w+)\s*\(/.exec(lines[at]);
    if (found) return found[1];
  }
  return '(top level)';
}

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name === '__tests__') continue;
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...sourceFiles(path));
    else if (entry.name.endsWith('.ts')) out.push(path);
  }
  return out;
}

describe('an inner layer name is one-based everywhere', () => {
  it('gives Inner1 the bit Rust gives Inner(0)', () => {
    expect(layerMaskBit('Top')).toBe(1);
    expect(layerMaskBit('Bottom')).toBe(2);
    expect(layerMaskBit('Inner1')).toBe(1 << 2);
    expect(layerMaskBit('Inner2')).toBe(1 << 3);
  });

  it('indexes from zero what the name counts from one', () => {
    expect(innerLayerIndex('Inner1')).toBe(0);
    expect(innerLayerIndex('Inner2')).toBe(1);
    // A name that counts from zero is not a name this side writes.
    expect(innerLayerIndex('Inner0')).toBeNull();
    expect(innerLayerIndex('Top')).toBeNull();
  });

  it('subtracts the one back out at every site that matches the name', () => {
    const offenders: string[] = [];
    const keysSeen = new Set<string>();
    let sites = 0;

    for (const path of sourceFiles(SRC)) {
      const lines = readFileSync(path, 'utf8').split('\n');
      lines.forEach((line, index) => {
        if (!line.includes('Inner(\\d+)')) return;
        sites += 1;
        const file = path.split('/').pop();
        const key = `${file}#${enclosingFunction(lines, index)}`;
        keysSeen.add(key);
        const near = lines.slice(index, index + 6).join(' ');
        const subtracts = near.includes('- 1') || near.includes('-1');
        if (!subtracts && !(key in KEEPS_THE_NAMES_NUMBER)) {
          offenders.push(`${file}:${index + 1} (${key}): ${line.trim()}`);
        }
      });
    }

    console.log(
      `sites matching an inner layer name: ${sites} (floor ${MATCH_SITES_FLOOR}); ` +
        `reading the number without subtracting one: ${offenders.length}`,
    );

    expect(sites).toBeGreaterThanOrEqual(MATCH_SITES_FLOOR);
    expect(offenders).toEqual([]);
    expect(Object.keys(KEEPS_THE_NAMES_NUMBER).filter((key) => !keysSeen.has(key))).toEqual([]);
  });

  it('names the function a site is in, not the line it is on', () => {
    const lines = ['function outer(a) {', '  const f = (x) => x;', '  return f(a);', '}'];
    expect(enclosingFunction(lines, 2)).toBe('outer');
    expect(enclosingFunction(['const x = 1;'], 0)).toBe('(top level)');
  });
});
