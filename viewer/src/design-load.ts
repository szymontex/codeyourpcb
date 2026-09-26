/**
 * Everything an engine needs to hold the design the page shows, and the one
 * way it is put into an engine.
 *
 * The page has one engine and the routing worker builds another, and a design
 * is more than its text: the files it imports, and the footprints the host
 * fetched from a supplier, which no file describes. The worker used to build
 * its engine from the text alone, so a part drawn with either was missing
 * from the board it routed. Both threads now load through `loadDesignInto`,
 * so there is one order of steps and no second copy of it to drift.
 */

import type { PadInfo, SilkShape } from './types';

/** A footprint the host fetched, as it was handed to the engine. */
export interface FetchedFootprint {
  name: string;
  pads: PadInfo[];
  silk: SilkShape[];
}

export type DesignKind = 'cypcb' | 'kicad_pcb';

export interface Design {
  kind: DesignKind;
  source: string;
  /** The text of every file the design imports, by the path it names. */
  imports: Record<string, string>;
  footprints: FetchedFootprint[];
}

/**
 * Which reader a design needs: by its file name, or by its first token when
 * the name does not say - a recent file keeps its name but a KiCad board's
 * text opens with `(kicad_pcb` whatever it is called.
 */
export function designKindOf(name: string, source: string): DesignKind {
  return name.toLowerCase().endsWith('.kicad_pcb') || source.trimStart().startsWith('(kicad_pcb')
    ? 'kicad_pcb'
    : 'cypcb';
}

/** The part of an engine that loading a design uses. */
export interface DesignLoader {
  register_footprint(name: string, pads: PadInfo[], silk: SilkShape[]): string;
  load_source_with_imports(source: string, files: Record<string, string>): string;
  load_kicad(source: string): string;
}

/**
 * Load a design into an engine.
 *
 * Footprints first, because a load resolves the design's parts against the
 * library it has at that moment. Returns every message the engine gave, one
 * per line, and an empty string when there were none - the engine answers a
 * load with plain text, not JSON.
 */
export function loadDesignInto(engine: DesignLoader, design: Design): string {
  const errors: string[] = [];
  for (const footprint of design.footprints) {
    const refused = engine.register_footprint(footprint.name, footprint.pads, footprint.silk);
    if (refused) {
      errors.push(`footprint ${footprint.name}: ${refused}`);
    }
  }
  const loaded = design.kind === 'kicad_pcb'
    ? engine.load_kicad(design.source)
    : engine.load_source_with_imports(design.source, design.imports);
  if (loaded) {
    errors.push(loaded);
  }
  return errors.join('\n');
}

/** Is this a design as `Design` describes it? For the worker's own reading. */
export function isDesign(value: unknown): value is Design {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const design = value as { kind?: unknown; source?: unknown; imports?: unknown; footprints?: unknown };
  return (
    (design.kind === 'cypcb' || design.kind === 'kicad_pcb') &&
    typeof design.source === 'string' &&
    typeof design.imports === 'object' &&
    design.imports !== null &&
    Object.values(design.imports).every((text) => typeof text === 'string') &&
    Array.isArray(design.footprints) &&
    design.footprints.every(
      (footprint: { name?: unknown; pads?: unknown; silk?: unknown }) =>
        typeof footprint === 'object' &&
        footprint !== null &&
        typeof footprint.name === 'string' &&
        Array.isArray(footprint.pads) &&
        Array.isArray(footprint.silk),
    )
  );
}
