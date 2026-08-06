import { describe, it, expect } from 'vitest';
import { loadWasm, registerDynamicFootprint, hasDynamicFootprint } from '../wasm';
import type { PadInfo, SilkShape } from '../types';

/**
 * A footprint fetched from a supplier has no `footprint` block behind it. It
 * has to reach the engine, or the pads are drawn while the board model holds
 * an unknown footprint: nothing to measure clearance against and nothing to
 * export. The registry that feeds the JavaScript parser was the only half that
 * existed, and `tsc` stayed clean because nothing called the missing method.
 */
describe('dynamic footprint registration', () => {
  const pads: PadInfo[] = [
    { number: '1', shape: 'rect', x_nm: -500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000 },
    { number: '2', shape: 'rect', x_nm: 500_000, y_nm: 0, width_nm: 600_000, height_nm: 500_000 },
  ] as unknown as PadInfo[];
  const silk: SilkShape[] = [] as unknown as SilkShape[];

  it('reaches the engine, including footprints registered before it existed', async () => {
    // Registered first, with no engine to hand it to.
    registerDynamicFootprint('TEST_EARLY', pads, silk);
    expect(hasDynamicFootprint('TEST_EARLY')).toBe(true);

    const engine = await loadWasm();
    const seen = (engine as unknown as { registered?: Map<string, unknown> }).registered;
    expect(seen, 'the engine records what it was taught').toBeDefined();
    expect(seen?.has('TEST_EARLY'), 'a footprint fetched before load is replayed').toBe(true);

    // And one registered afterwards goes straight through.
    registerDynamicFootprint('TEST_LATE', pads, silk);
    expect(seen?.has('TEST_LATE'), 'a later fetch reaches the engine too').toBe(true);
  });
});
