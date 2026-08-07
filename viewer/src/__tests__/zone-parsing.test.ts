import { describe, it, expect } from 'vitest';
import { parseSource } from '../wasm';

/**
 * A zone reaches the engine only if the host's parser carries it. The viewer
 * skipped `zone` and `keepout` blocks outright, so a board with a ground plane
 * arrived at the engine as a board without one, and the screen showed a bare
 * rectangle where the fabricator gets copper cut around every pad.
 *
 * What a zone becomes is not computed here on purpose: that geometry lives in
 * Rust and is what the Gerber carries. This parses the outline and hands it
 * over.
 */
describe('zones in the source reach the snapshot', () => {
  const source = `
board demo {
    size 30mm x 20mm
    layers 2
}

zone gnd_pour {
    bounds 2mm, 2mm to 28mm, 18mm
    layer top
    net GND
}

keepout mounting {
    bounds 5mm, 5mm to 8mm, 8mm
    layer all
}
`;

  it('carries a copper pour with its outline, layer and net', () => {
    const { snapshot } = parseSource(source);
    const zones = snapshot.zones ?? [];
    const pour = zones.find((z) => z.name === 'gnd_pour');

    expect(pour).toBeDefined();
    expect(pour!.kind).toBe('pour');
    expect(pour!.net).toBe('GND');
    expect(pour!.layer_mask).toBe(0b01);
    expect(pour!.bounds).toEqual([2_000_000, 2_000_000, 28_000_000, 18_000_000]);
  });

  it('tells a keepout from a pour, and gives it every layer', () => {
    const { snapshot } = parseSource(source);
    const keepout = (snapshot.zones ?? []).find((z) => z.name === 'mounting');

    expect(keepout).toBeDefined();
    expect(keepout!.kind).toBe('keepout');
    expect(keepout!.layer_mask).toBe(0b11);
    expect(keepout!.net).toBe('');
  });

  it('drops a zone that states no outline rather than sending a zero-size one', () => {
    const { snapshot } = parseSource(`
board demo {
    size 30mm x 20mm
    layers 2
}

zone nothing {
    layer top
}
`);
    expect((snapshot.zones ?? []).length).toBe(0);
  });
});
