import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { autoWidthNotice } from '../interaction';

/**
 * A width chosen for you is a width you get told about.
 *
 * A net that states a current and no width has its trace width picked by
 * IPC-2221 while the person is drawing. That was a `console.log` and nothing
 * else, so a 40A net silently became a 48mm trace - from a formula whose data
 * stops around 35A, which the engine has always known and no caller in the
 * browser could ask.
 *
 * `cypcb check` and the hover card both say it now. This is the third surface.
 */

const SRC = join(__dirname, '..');

describe('the sentence', () => {
  it('states the width, because nobody asked for it', () => {
    const said = autoWidthNotice('VCC', 1000, 300_376, '');
    expect(said).toContain('VCC');
    expect(said).toContain('1.0A');
    expect(said).toContain('0.300mm');
    expect(said).toContain('IPC-2221');
  });

  it('reads milliamps as milliamps', () => {
    expect(autoWidthNotice('SDA', 100, 12_542, '')).toContain('100mA');
  });

  it('appends what the standard says about its own answer', () => {
    const said = autoWidthNotice(
      'VBUS',
      40_000,
      48_690_000,
      'Current >35A: IPC-2221 accuracy degrades, consider experimental validation',
    );
    expect(said).toContain('48.690mm');
    expect(said).toContain('accuracy degrades');
  });

  it('says nothing extra when there is nothing to say', () => {
    // The half that keeps the other from being noise: an ordinary net trips
    // none of the calculator's ranges and the line stays one clause.
    expect(autoWidthNotice('VCC', 1000, 300_376, '')).not.toContain(' - ');
  });
});

describe('both places that choose a width', () => {
  // Two call sites pick an auto-width: starting a route from a pad, and
  // continuing one from an existing trace. A note that reaches only one of
  // them is a note that depends on how the user started.
  const source = readFileSync(join(SRC, 'interaction.ts'), 'utf8');

  it('route through the same sentence', () => {
    const calls = source.split('state.onStatus?.(autoWidthNotice(').length - 1;
    expect(calls).toBe(2);
  });

  it('ask the engine for the notes rather than restating the ranges', () => {
    // "35A" written here would be the fifth copy of this arithmetic in the
    // project, and the previous four had already drifted.
    expect(source).toContain('trace_width_notes_for_current_ma');
    expect(source).not.toContain('35A');
  });
});
