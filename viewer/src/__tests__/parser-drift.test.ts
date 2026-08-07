/**
 * Two parsers read this language, and this measures where they disagree.
 *
 * The browser build compiles `cypcb-parser` with `default-features = false`,
 * which leaves it with no parser at all, so `viewer/src/wasm.ts` reads the DSL
 * a second time in TypeScript - a hand-written line reader against a
 * tree-sitter grammar. What the screen draws therefore comes from a different
 * implementation than what `cypcb export` writes into the Gerbers, and nothing
 * has ever compared the two.
 *
 * This walks every board in `examples/` through both and diffs the model: the
 * parts and where they sit, the nets and which pins are on them, the traces
 * and the zones. It is a measurement first - the assertions below pin only
 * what the two already agree on, and every difference is named in the tracker
 * rather than hidden behind a tolerance.
 */

import { describe, it, expect } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { parseSource } from '../wasm';

const REPO_ROOT = resolve(__dirname, '../../..');
const EXAMPLES = join(REPO_ROOT, 'examples');

/** Boards that exist to demonstrate a parse error. */
const MEANT_TO_FAIL = ['invalid.cypcb', 'unknown_keyword.cypcb'];

function cypcbBinary(): string {
  // Newest wins. Both profiles are usually present and the stale one prints an
  // older model, which would read as parser drift that is really a build age.
  const candidates = ['release', 'debug']
    .map((profile) => join(REPO_ROOT, 'target', profile, 'cypcb'))
    .filter((path) => existsSync(path))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  if (candidates.length === 0) {
    throw new Error(
      'no cypcb binary in target/release or target/debug - run `cargo build -p cypcb-cli` first',
    );
  }
  return candidates[0];
}

function exampleFiles(): string[] {
  return readdirSync(EXAMPLES)
    .filter((name) => name.endsWith('.cypcb'))
    .filter((name) => !MEANT_TO_FAIL.includes(name))
    .sort();
}

/** What both sides are reduced to before comparing. */
interface Model {
  components: Array<{
    refdes: string;
    footprint: string;
    x_nm: number;
    y_nm: number;
    rotation_deg: number;
  }>;
  /** `NET/refdes.pin`, sorted - the connection list both sides can produce. */
  pins: string[];
  nets: string[];
  traces: number;
  vias: number;
  zones: Array<{ kind: string; net: string; bounds: [number, number, number, number] }>;
}

function rustModel(file: string): Model {
  const raw = execFileSync(cypcbBinary(), ['parse', join(EXAMPLES, file)], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  const model = JSON.parse(raw);
  if (!Array.isArray(model.components)) {
    throw new Error(
      `${cypcbBinary()} printed no board model for ${file} - it predates \`parse -o json\`, rebuild it`,
    );
  }

  const pins: string[] = [];
  for (const component of model.components) {
    for (const pin of component.pins) {
      pins.push(`${pin.net}/${component.refdes}.${pin.pin}`);
    }
  }

  return {
    components: model.components.map((c: any) => ({
      refdes: c.refdes,
      footprint: c.footprint,
      x_nm: c.x_nm,
      y_nm: c.y_nm,
      rotation_deg: c.rotation_deg,
    })),
    pins: pins.sort(),
    nets: model.nets.map((n: any) => n.name).sort(),
    traces: model.traces.length,
    vias: model.vias.length,
    zones: model.zones
      .map((z: any) => ({
        kind: z.kind === 'CopperPour' ? 'pour' : 'keepout',
        net: z.net ?? '',
        bounds: [z.min_x_nm, z.min_y_nm, z.max_x_nm, z.max_y_nm] as [number, number, number, number],
      }))
      .sort((a: any, b: any) => a.bounds[0] - b.bounds[0]),
  };
}

function viewerModel(file: string): Model {
  const source = readFileSync(join(EXAMPLES, file), 'utf8');
  const { snapshot } = parseSource(source);

  const pins: string[] = [];
  for (const net of snapshot.nets) {
    for (const connection of net.connections) {
      pins.push(`${net.name}/${connection.component}.${connection.pin}`);
    }
  }

  return {
    components: snapshot.components
      .map((c) => ({
        refdes: c.refdes,
        footprint: c.footprint,
        x_nm: c.x_nm,
        y_nm: c.y_nm,
        rotation_deg: c.rotation_mdeg / 1000,
      }))
      .sort((a, b) => a.refdes.localeCompare(b.refdes)),
    pins: pins.sort(),
    nets: snapshot.nets.map((n) => n.name).sort(),
    traces: snapshot.traces.length,
    vias: snapshot.vias.length,
    zones: (snapshot.zones ?? [])
      .map((z) => ({ kind: z.kind, net: z.net, bounds: z.bounds }))
      .sort((a, b) => a.bounds[0] - b.bounds[0]),
  };
}

/** Every example, parsed both ways once, so each test reads the same numbers. */
const measured = exampleFiles().map((file) => ({
  file,
  rust: rustModel(file),
  viewer: viewerModel(file),
}));

/**
 * Where the two parsers disagree today, measured 2026-08-07.
 *
 * This list is the point of the file. It is not a tolerance: every entry is a
 * board that draws differently in the browser than it exports from the CLI,
 * and the test fails both when a new disagreement appears and when one of
 * these is fixed without being struck off.
 *
 * - `v2-imports.cypcb`: the viewer follows no `import`, so it shows an empty
 *   board where the CLI sees six parts on seven nets.
 * - `v2-modules.cypcb`: the viewer does not instantiate modules. It draws the
 *   two parts written inside a module body as though they were top level,
 *   under their local names, and finds one net; the CLI instantiates and gets
 *   seven parts with prefixed names on five nets.
 * - `v2-interfaces.cypcb`: the mirror image - the viewer draws two parts and
 *   two nets from a module body that is never instantiated, where the CLI
 *   correctly draws nothing.
 * - the four trace files: a `trace NET { from A.1 to B.1 }` with no explicit
 *   geometry is copper in the CLI's model and in the Gerber, and invisible in
 *   the browser.
 */
const KNOWN_DRIFT: Record<string, string[]> = {
  'v2-imports.cypcb': ['components', 'nets', 'pins'],
  'v2-modules.cypcb': ['components', 'nets', 'pins'],
  'v2-interfaces.cypcb': ['components', 'nets', 'pins'],
  'four-layer.cypcb': ['traces'],
  'pour-island.cypcb': ['traces'],
  'syntax.cypcb': ['traces'],
  'uat-routing-locked.cypcb': ['traces'],
};

function driftFor(rust: Model, viewer: Model): string[] {
  const kinds: string[] = [];
  if (JSON.stringify(rust.components) !== JSON.stringify(viewer.components)) kinds.push('components');
  if (JSON.stringify(rust.nets) !== JSON.stringify(viewer.nets)) kinds.push('nets');
  if (JSON.stringify(rust.pins) !== JSON.stringify(viewer.pins)) kinds.push('pins');
  if (rust.traces !== viewer.traces) kinds.push('traces');
  if (rust.vias !== viewer.vias) kinds.push('vias');
  if (JSON.stringify(rust.zones) !== JSON.stringify(viewer.zones)) kinds.push('zones');
  return kinds;
}

describe('the two parsers on the same file', () => {
  it('disagree on exactly the boards this file names, and no others', () => {
    const measuredDrift: Record<string, string[]> = {};
    for (const { file, rust, viewer } of measured) {
      const kinds = driftFor(rust, viewer);
      if (kinds.length > 0) measuredDrift[file] = kinds;
    }
    expect(measuredDrift).toEqual(KNOWN_DRIFT);
  });

  it('agree on every board that uses neither modules nor a pin-to-pin trace', () => {
    // The plain subset - a board, parts, nets and explicit geometry - is where
    // the two implementations have always matched, and it is most of what
    // anybody writes. Named separately so a regression there reads as one.
    const plain = measured.filter(({ file }) => !(file in KNOWN_DRIFT));
    expect(plain.length).toBeGreaterThan(8);
    for (const { file, rust, viewer } of plain) {
      expect(driftFor(rust, viewer), `${file} drifted`).toEqual([]);
    }
  });

  it('read the same zones on every board that draws one', () => {
    // Zones are the newest thing both sides read, and the one place a fix
    // landed in both at once. Pinned on its own so it stays that way.
    for (const { file, rust, viewer } of measured) {
      expect(viewer.zones, `${file} zones`).toEqual(rust.zones);
    }
  });
});
