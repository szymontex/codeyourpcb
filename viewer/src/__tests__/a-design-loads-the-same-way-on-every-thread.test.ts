import { describe, it, expect } from 'vitest';
import { loadDesignInto, type Design, type DesignLoader } from '../design-load';
import type { PadInfo } from '../types';

/**
 * The page and the routing worker load a design through one function.
 *
 * The worker used to build its engine from the design text alone and read the
 * engine's answer as JSON, which it is not, so every load error was taken for
 * success. What this holds: the footprints go in before the load, the imports
 * go with it, the reader matches the kind, and every message comes back.
 */

const PAD = {
  number: '1', shape: 'rect', x_nm: 0, y_nm: 0, width_nm: 800_000, height_nm: 800_000,
  layer_mask: 1, drill_nm: null,
} as PadInfo;

function design(overrides: Partial<Design> = {}): Design {
  return {
    kind: 'cypcb',
    source: 'version 1\n',
    imports: { 'lib/blocks.cypcb': 'module blocks {}\n' },
    footprints: [{ name: 'FETCHED', pads: [PAD], silk: [] }],
    ...overrides,
  };
}

/** An engine that writes down what it was asked, and answers as told. */
function recorder(answers: { register?: string; load?: string } = {}): DesignLoader & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    register_footprint(name) {
      calls.push(`register ${name}`);
      return answers.register ?? '';
    },
    load_source_with_imports(_source, files) {
      calls.push(`load cypcb with ${Object.keys(files).join(',')}`);
      return answers.load ?? '';
    },
    load_kicad() {
      calls.push('load kicad');
      return answers.load ?? '';
    },
  };
}

describe('loadDesignInto', () => {
  it('teaches the engine the fetched footprints before it loads', () => {
    const engine = recorder();
    expect(loadDesignInto(engine, design())).toBe('');
    expect(engine.calls).toEqual(['register FETCHED', 'load cypcb with lib/blocks.cypcb']);
  });

  it('reads a KiCad board with the KiCad reader', () => {
    const engine = recorder();
    loadDesignInto(engine, design({ kind: 'kicad_pcb' }));
    expect(engine.calls).toEqual(['register FETCHED', 'load kicad']);
  });

  it('returns the plain text the engine answers a load with', () => {
    const engine = recorder({ load: "unknown footprint: 'NEVER_FETCHED'" });
    expect(loadDesignInto(engine, design())).toBe("unknown footprint: 'NEVER_FETCHED'");
  });

  it('returns a footprint the engine refused, named', () => {
    const engine = recorder({ register: 'Failed to deserialize pads' });
    expect(loadDesignInto(engine, design())).toBe('footprint FETCHED: Failed to deserialize pads');
  });
});
