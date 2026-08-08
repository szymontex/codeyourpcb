import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

/**
 * A net that states a current gets its width from the engine, not from here.
 *
 * The IPC-2221 formula was written twice: `cypcb-calc` in Rust, which the
 * router, `cypcb check` and the language server all go through, and
 * `ipc2221MinWidthNm` in `viewer/src/wasm.ts`, which the browser used while a
 * user routed by hand. Measured against each other before the second was
 * deleted, they agreed to within a nanometre:
 *
 *   current    viewer      cypcb-calc
 *   100mA      12542       12541
 *   250mA      44385       44385
 *   500mA      115465      115465
 *   1A         300376      300376
 *   2A         781411      781410
 *   5A         2765426     2765426
 *
 * That is what a duplicate looks like right up until somebody edits one side.
 * The language server had already drifted on this exact rule - it used 1.37
 * mils for an ounce of copper against 1.378 - and quoted widths 0.58% off what
 * the router would draw.
 */

const SRC = join(__dirname, '..');

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

describe('one trace-width formula', () => {
  it('no viewer source computes IPC-2221 itself', () => {
    // The constants, not the function name: a copy pasted under another name
    // is the thing this is here to catch. 0.048 is k for an external layer,
    // 0.725 the area exponent, 1.378 the thickness of an ounce of copper.
    const constants = ['0.048', '0.725', '1.378'];
    const offenders: string[] = [];

    for (const file of sourceFiles(SRC)) {
      const text = readFileSync(file, 'utf8');
      // Comments may name the constants - this test does itself. Only code
      // counts.
      const code = text
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .split('\n')
        .filter(line => !line.trim().startsWith('//'))
        .join('\n');
      const found = constants.filter(constant => code.includes(constant));
      if (found.length >= 2) {
        offenders.push(`${file.slice(SRC.length + 1)}: ${found.join(', ')}`);
      }
    }

    expect(offenders, 'the IPC-2221 formula belongs to cypcb-calc alone').toEqual([]);
  });

  it('the interaction layer asks the engine for the width', () => {
    const source = readFileSync(join(SRC, 'interaction.ts'), 'utf8');
    expect(
      source.includes('min_trace_width_for_current_ma'),
      'the routing width has to come from the engine',
    ).toBe(true);
    expect(
      source.includes('ipc2221MinWidthNm'),
      'the deleted helper must not come back',
    ).toBe(false);
  });

  it('the engine interface and both implementations carry the call', () => {
    const wasm = readFileSync(join(SRC, 'wasm.ts'), 'utf8');
    const occurrences = wasm.split('min_trace_width_for_current_ma').length - 1;
    // The raw wasm interface, the PcbEngine interface, the adapter (its
    // declaration and the call it forwards) and the fallback engine.
    expect(
      occurrences,
      'interface, adapter and fallback all have to declare it',
    ).toBeGreaterThanOrEqual(5);
  });
});
