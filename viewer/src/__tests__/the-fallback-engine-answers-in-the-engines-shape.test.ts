import { describe, it, expect, beforeAll, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { initSync, PcbEngine as RawEngine } from '../../pkg/cypcb_render.js';
import { MockPcbEngine, WasmPcbEngineAdapter, type PcbEngine, type WasmPcbEngine } from '../wasm';
import type { PadInfo } from '../types';

/**
 * The fallback engine answers every question in the shape the real one does.
 *
 * Every other test in this folder that needs an engine is handed the mock, so
 * a mock that answers differently from the engine hides whatever the adapter
 * gets wrong. It did: the adapter sent fetched pads to the engine as JSON
 * text, the engine refused every one of them, and the mock said yes. This
 * asks both the same question through `PcbEngine` - the real WASM behind
 * `WasmPcbEngineAdapter`, loaded the way the browser loads it - and compares
 * what kind of answer comes back: its type, empty or not, JSON or plain text,
 * and the keys of an object. The values may differ; the mock does not route.
 */

const WASM = new URL('../../pkg/cypcb_render_bg.wasm', import.meta.url);

const BOARD = `version 1

board b {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    at 5mm, 10mm
}

component R2 resistor "0402" {
    at 15mm, 10mm
}

net SIG {
    R1.1
    R2.1
}
`;

const PAD: PadInfo = {
  number: '1', shape: 'rect', x_nm: 0, y_nm: 0, width_nm: 800_000, height_nm: 800_000,
  layer_mask: 1, drill_nm: null,
};

/** What kind of answer this is: never the value itself. */
function kindOf(answer: unknown): string {
  if (typeof answer === 'string') {
    if (answer === '') return 'empty text';
    try {
      return `JSON ${kindOf(JSON.parse(answer))}`;
    } catch {
      return 'plain text';
    }
  }
  if (Array.isArray(answer)) {
    const kinds = [...new Set(answer.map((item) => typeof item))].sort();
    return `array of ${kinds.join('|') || 'nothing'}`;
  }
  if (typeof answer === 'object' && answer !== null) {
    return `object {${Object.keys(answer).sort().join(',')}}`;
  }
  return typeof answer;
}

type Question = (engine: PcbEngine) => unknown;

/**
 * One question per method the mock implements, asked of a board both engines
 * were given. `except` says why the two answer in different kinds on
 * purpose; the test then still checks what both must share.
 */
const QUESTIONS: Record<Exclude<keyof PcbEngine, 'free'>, { ask: Question; except?: string; share?: (kind: string) => boolean }> = {
  register_footprint: { ask: (e) => e.register_footprint('CONTRACT_PART', [PAD], []) },
  register_3d_model: { ask: (e) => e.register_3d_model('CONTRACT_PART', '00000000-0000-0000-0000-000000000000') },
  get_diagnostics_json: { ask: (e) => e.get_diagnostics_json() },
  load_source: {
    ask: (e) => e.load_source(BOARD),
    except: 'the mock has no reader of the language and refuses every load (docs/one-parser.md)',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  load_source_with_imports: {
    ask: (e) => e.load_source_with_imports(BOARD, { 'lib/unused.cypcb': 'version 1\n' }),
    except: 'the mock refuses every load, as above',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  load_kicad: {
    ask: (e) => e.load_kicad('(kicad_pcb (version 20221018) (generator contract))'),
    except: 'the mock has no KiCad reader and refuses every load',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  load_routes: { ask: (e) => e.load_routes('(session contract (routes (network_out)))') },
  get_snapshot: { ask: (e) => e.get_snapshot() },
  query_point: { ask: (e) => e.query_point(5_000_000, 10_000_000) },
  add_trace: { ask: (e) => e.add_trace('SIG', 'Top', 250_000, [5_000_000, 10_000_000, 15_000_000, 10_000_000]) },
  remove_trace: { ask: (e) => e.remove_trace(0xFFFFFFF0) },
  get_trace_at_point: { ask: (e) => e.get_trace_at_point(0, 0, 1_000) },
  run_drc_incremental: { ask: (e) => e.run_drc_incremental() },
  trace_count: { ask: (e) => e.trace_count() },
  export_traces_as_dsl: {
    ask: (e) => e.export_traces_as_dsl(),
    except: 'the mock exports nothing; the engine writes DSL, which is plain text',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  design_as_dsl: {
    ask: (e) => e.design_as_dsl(),
    except: 'the mock holds no design it could write; the engine writes the board as DSL, which is plain text',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  design_not_written: {
    ask: (e) => e.design_not_written(),
    except: 'the mock writes nothing and so leaves nothing out; the engine names what it left out as plain text',
    share: (kind) => kind === 'empty text' || kind === 'plain text',
  },
  get_min_clearance_nm: { ask: (e) => e.get_min_clearance_nm() },
  min_trace_width_for_current_ma: { ask: (e) => e.min_trace_width_for_current_ma(1_000) },
  trace_width_notes_for_current_ma: { ask: (e) => e.trace_width_notes_for_current_ma(1_000) },
  rotate_component: { ask: (e) => e.rotate_component('NOBODY', 90_000) },
  set_board_size: {
    ask: (e) => e.set_board_size(30_000_000, 30_000_000),
    except: 'the mock holds no board, so it has none to resize',
  },
  auto_route: { ask: (e) => e.auto_route(), except: 'the mock does not route', share: (kind) => kind.startsWith('JSON object') },
  auto_route_with_params: {
    ask: (e) => e.auto_route_with_params('{"via_cost":10}'),
    except: 'the mock does not route',
    share: (kind) => kind.startsWith('JSON object'),
  },
  auto_route_variants: {
    ask: (e) => e.auto_route_variants(),
    except: 'the mock does not route; the engine answers with an array of variants',
    share: (kind) => kind.startsWith('JSON '),
  },
  auto_route_debug: {
    ask: (e) => e.auto_route_debug('{}'),
    except: 'the mock does not route',
    share: (kind) => kind.startsWith('JSON object'),
  },
};

/**
 * Every method the WASM build exports, and which `PcbEngine` method reaches
 * it. A method nothing reaches says why, so a new export is a decision here
 * and not something the adapter silently never calls.
 */
const REACHED_BY: Record<keyof WasmPcbEngine, keyof PcbEngine | { unreached: string }> = {
  load_source: 'load_source',
  load_source_with_imports: 'load_source_with_imports',
  load_kicad: 'load_kicad',
  load_snapshot: { unreached: 'the host sent a parsed snapshot before the engine had a reader; nothing sends one now' },
  get_snapshot: 'get_snapshot',
  get_diagnostics_json: 'get_diagnostics_json',
  query_point: 'query_point',
  add_trace: { unreached: 'the adapter adds traces through add_trace_json' },
  add_trace_json: 'add_trace',
  remove_trace: 'remove_trace',
  get_trace_at_point: 'get_trace_at_point',
  run_drc_incremental: 'run_drc_incremental',
  trace_count: 'trace_count',
  export_traces_as_dsl: 'export_traces_as_dsl',
  design_as_dsl: 'design_as_dsl',
  design_not_written: 'design_not_written',
  get_min_clearance_nm: 'get_min_clearance_nm',
  min_trace_width_for_current_ma: 'min_trace_width_for_current_ma',
  trace_width_notes_for_current_ma: 'trace_width_notes_for_current_ma',
  get_violations_json: { unreached: 'violations reach the viewer inside get_snapshot' },
  rotate_component: 'rotate_component',
  set_board_size: 'set_board_size',
  auto_route: 'auto_route',
  auto_route_with_params: 'auto_route_with_params',
  auto_route_variants: 'auto_route_variants',
  auto_route_debug: 'auto_route_debug',
  register_footprint: 'register_footprint',
  register_3d_model: 'register_3d_model',
  free: 'free',
};

let real: PcbEngine;
let mock: PcbEngine;

beforeAll(() => {
  initSync({ module: readFileSync(WASM) });
});

beforeEach(() => {
  // Fresh engines for each question: one question's answer must not come
  // from what the one before it did to the board.
  // No cast: the build's own declarations are checked against the interface
  // the adapter is written to, so a type that drifts fails `tsc`.
  real = new WasmPcbEngineAdapter(new RawEngine());
  mock = new MockPcbEngine();
  for (const engine of [real, mock]) {
    engine.load_source(BOARD);
  }
});

describe('the mock answers in the kind the engine answers', () => {
  it('holds a board in the real engine, so the answers below are about one', () => {
    // The control: without it every question below is asked of an empty
    // engine, and an empty engine and the mock agree on a lot.
    expect(real.load_source(BOARD)).toBe('');
    expect(real.get_snapshot().components.map((c) => c.refdes).sort()).toEqual(['R1', 'R2']);
  });

  for (const [method, { ask, except, share }] of Object.entries(QUESTIONS)) {
    it(method, () => {
      const realAnswer = ask(real);
      const mockAnswer = ask(mock);
      if (!except) {
        // An empty list says nothing about what its items would be: the mock
        // holds no parts, so a point query finds none there.
        const empty = [realAnswer, mockAnswer].some((answer) => Array.isArray(answer) && answer.length === 0);
        if (empty) {
          expect(Array.isArray(mockAnswer) && Array.isArray(realAnswer)).toBe(true);
          return;
        }
        expect(kindOf(mockAnswer), `real: ${kindOf(realAnswer)}`).toBe(kindOf(realAnswer));
        return;
      }
      expect(typeof mockAnswer).toBe(typeof realAnswer);
      if (share) {
        expect(share(kindOf(realAnswer)), `real answered ${kindOf(realAnswer)}`).toBe(true);
        expect(share(kindOf(mockAnswer)), `mock answered ${kindOf(mockAnswer)}`).toBe(true);
      }
    });
  }
});

describe('every method the engine exports is accounted for', () => {
  it('names each export of the WASM build', () => {
    const exported = Object.getOwnPropertyNames(RawEngine.prototype)
      .filter((name) => name !== 'constructor' && !name.startsWith('__'))
      .sort();
    expect(Object.keys(REACHED_BY).sort()).toEqual(exported);
  });

  it('reaches each through a method the adapter has', () => {
    for (const [raw, by] of Object.entries(REACHED_BY)) {
      if (typeof by === 'string') {
        expect(typeof (WasmPcbEngineAdapter.prototype as unknown as Record<string, unknown>)[by], raw).toBe('function');
      }
    }
  });
});
