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
 * is right to.
 */
const KEEPS_THE_NAMES_NUMBER: Record<string, string> = {
  'layers.ts:583':
    'sorts by depth, where the name\'s own number is already the order',
};

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
    let sites = 0;

    for (const path of sourceFiles(SRC)) {
      const lines = readFileSync(path, 'utf8').split('\n');
      lines.forEach((line, index) => {
        if (!line.includes('Inner(\\d+)')) return;
        sites += 1;
        const where = `${path.split('/').pop()}:${index + 1}`;
        const near = lines.slice(index, index + 6).join(' ');
        const subtracts = near.includes('- 1') || near.includes('-1');
        if (!subtracts && !(where in KEEPS_THE_NAMES_NUMBER)) {
          offenders.push(`${where}: ${line.trim()}`);
        }
      });
    }

    console.log(
      `sites matching an inner layer name: ${sites} (floor ${MATCH_SITES_FLOOR}); ` +
        `reading the number without subtracting one: ${offenders.length}`,
    );

    expect(sites).toBeGreaterThanOrEqual(MATCH_SITES_FLOOR);
    expect(offenders).toEqual([]);
  });
});
