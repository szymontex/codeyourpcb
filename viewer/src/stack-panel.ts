/**
 * The stack manager: the board's own build, as a table.
 *
 * A design states its stack in the language - `copper 1oz`, `prepreg 0.1mm dk
 * 4.5`, `finish "ENIG"`, `drill Top to Inner1` - and until this panel existed
 * reading it back meant reading the source. A stack is the one part of a
 * design that is a table rather than a list of statements: eleven entries with
 * four columns each, where what a person wants is to see the sandwich.
 *
 * The two functions that decide what is shown are pure and take the snapshot's
 * own `StackupInfo`. The DOM is built from what they return, so what the panel
 * says can be tested without a browser - which is the lesson this project
 * learned from an error list whose three lines lived inside a closure nothing
 * could reach.
 */

import type { StackupInfo } from './types';

/** One row of the stack table. */
export interface StackRow {
  /** What the design calls this layer, or what it is when it named nothing. */
  label: string;
  /** The language's word for the material: `copper`, `prepreg`, `core`, ... */
  kind: string;
  /** Its thickness, ready to print, or an empty string when it stated none. */
  thickness: string;
  /** Material, colour and dielectric numbers, in the order a fab reads them. */
  detail: string;
  /**
   * How much of the board's thickness this layer is, from 0 to 1.
   *
   * The bar beside the row. `0` when either the layer or the board did not
   * state a thickness - a bar drawn from a guess is a drawing that lies.
   */
  share: number;
}

/** Millimetres, trimmed: `0.035mm`, `1.095mm`, not `0.035000mm`. */
function mm(nm: number): string {
  return `${Number((nm / 1_000_000).toFixed(4))}mm`;
}

/**
 * What each entry of the stack shows.
 *
 * A dielectric slot pressed from several sheets says so: a fabricator hits a
 * target thickness with the prepreg they stock, and a panel that showed only
 * the first sheet would show a thinner board than the one being built.
 */
export function stackRows(stack: StackupInfo): StackRow[] {
  const total = stack.total_thickness_nm ?? 0;
  return stack.layers.map((layer) => {
    const slot = layer.slot_thickness_nm ?? layer.thickness_nm;
    const detail: string[] = [];
    if (layer.material) detail.push(layer.material);
    if (layer.color) detail.push(layer.color);
    if (layer.dk_x1000 != null) detail.push(`dk ${layer.dk_x1000 / 1000}`);
    if (layer.df_x1000000 != null) detail.push(`df ${layer.df_x1000000 / 1_000_000}`);
    // Where the layer stops, when it does not run the whole panel. A
    // rigid-flex build is several stacks on one board, and a table that showed
    // only the layers would tell a reader it is one - which is what this panel
    // did for the week between the language gaining `covers` and this line.
    if (layer.coverage_region) {
      detail.push(`${layer.coverage_covers ? 'covers' : 'outside'} ${layer.coverage_region}`);
    }
    if (layer.sheets_nm.length > 1) {
      detail.push(`${layer.sheets_nm.length} sheets: ${layer.sheets_nm.map(mm).join(' + ')}`);
    }
    return {
      label: layer.name || layer.kind,
      kind: layer.kind,
      thickness: slot != null ? mm(slot) : '',
      detail: detail.join(' · '),
      share: total > 0 && slot != null ? slot / total : 0,
    };
  });
}

/**
 * What the fabricator is asked for beyond the layers themselves.
 *
 * Each line is a statement the design made. A design that stated none gets an
 * empty list rather than a row of "no": silence is the rest, the way it is in
 * the language.
 */
export function stackSummary(stack: StackupInfo): string[] {
  const lines: string[] = [];
  if (stack.total_thickness_nm != null) {
    lines.push(`${mm(stack.total_thickness_nm)} thick`);
  }
  if (stack.finish) lines.push(`finish ${stack.finish}`);
  if (stack.impedance_controlled) lines.push('impedance controlled');
  if (stack.edges_plated) lines.push('edges plated');
  if (stack.castellated_pads) lines.push('castellated pads');
  if (stack.edge_connector) lines.push(`${stack.edge_connector} edge connector`);
  for (const [start, end] of stack.drill_pairs) {
    lines.push(`drills ${start} to ${end}`);
  }
  return lines;
}

/** One build of the board: the whole panel, or one area of it. */
export interface StackBuild {
  /** What this build is called: the board's own name, or an area's. */
  name: string;
  /** The layers pressed here, in stack order. */
  rows: StackRow[];
  /** How thick the board is here, ready to print, or an empty string. */
  thickness: string;
}

/**
 * The builds this board has: one, or one per area a layer stops at.
 *
 * A rigid-flex board is several stacks on one panel, and a table of layers
 * says it is one. The areas come from the engine rather than from a filter
 * written here: the same question the fabricator's document asks, asked once.
 *
 * A board whose layers stop nowhere gets a single build, which is the table
 * this panel always showed.
 */
export function stackBuilds(stack: StackupInfo): StackBuild[] {
  const rows = stackRows(stack);
  const whole: StackBuild = {
    name: 'whole board',
    rows,
    thickness: stack.total_thickness_nm != null ? mm(stack.total_thickness_nm) : '',
  };
  const areas = stack.areas ?? [];
  if (areas.length === 0) return [whole];

  return [
    whole,
    ...areas.map((area) => ({
      name: area.name,
      rows: area.layers.map((index) => rows[index]).filter((row) => row != null),
      thickness: area.thickness_nm != null ? mm(area.thickness_nm) : '',
    })),
  ];
}

/** The colour a layer's bar is drawn in, by what it is made of. */
const SWATCH: Record<string, string> = {
  copper: '#c07a3e',
  prepreg: '#7d8b5a',
  core: '#6b7a4a',
  mask: '#2f6d3f',
  silk: '#d8d4cc',
  paste: '#9aa0a6',
  coverlay: '#a8862f',
  stiffener: '#5a5f66',
};

/**
 * Draw the stack into a host element.
 *
 * `undefined` means the design states no stack, which most do - and that is a
 * sentence rather than an empty table, because a board with no stack is not a
 * board with no layers.
 */
export function renderStack(host: HTMLElement, stack?: StackupInfo): void {
  host.textContent = '';

  if (!stack || stack.layers.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'sp-empty';
    empty.textContent =
      'This design states no stackup. Add one in a board block to say how it is built.';
    host.appendChild(empty);
    return;
  }

  const builds = stackBuilds(stack);
  for (const build of builds) {
    // The heading is drawn only when there is more than one build: a rigid
    // board has one, and a table with "whole board" written over it says
    // nothing a reader did not already know.
    if (builds.length > 1) {
      const heading = document.createElement('div');
      heading.className = 'sp-build';
      heading.textContent = build.thickness
        ? `${build.name} - ${build.thickness}`
        : build.name;
      host.appendChild(heading);
    }
    host.appendChild(buildTable(build));
  }

  const summary = stackSummary(stack);
  if (summary.length > 0) {
    const list = document.createElement('ul');
    list.className = 'sp-summary';
    for (const item of summary) {
      const entry = document.createElement('li');
      entry.textContent = item;
      list.appendChild(entry);
    }
    host.appendChild(list);
  }
}

/** One build as a table of rows. */
function buildTable(build: StackBuild): HTMLElement {
  const table = document.createElement('div');
  table.className = 'sp-table';
  for (const row of build.rows) {
    const line = document.createElement('div');
    line.className = 'sp-row';

    const bar = document.createElement('span');
    bar.className = 'sp-bar';
    bar.style.background = SWATCH[row.kind] ?? '#8a8a8a';
    // A minimum of one pixel so a 17-micron foil is still visible beside a
    // millimetre of core; the number beside it is the honest one.
    bar.style.height = `${Math.max(1, Math.round(row.share * 160))}px`;
    line.appendChild(bar);

    const body = document.createElement('div');
    body.className = 'sp-body';

    const name = document.createElement('div');
    name.className = 'sp-name';
    name.textContent = row.label;
    body.appendChild(name);

    if (row.detail) {
      const detail = document.createElement('div');
      detail.className = 'sp-detail';
      detail.textContent = row.detail;
      body.appendChild(detail);
    }

    const thickness = document.createElement('div');
    thickness.className = 'sp-thickness';
    thickness.textContent = row.thickness;

    line.appendChild(body);
    line.appendChild(thickness);
    table.appendChild(line);
  }
  return table;
}
