/**
 * Counting the traces that go through the editor, so a lost one is loud.
 *
 * Reported by the owner while wiring a board by hand: a trace that was already
 * drawn disappears when the next one is drawn. Seen twice, cause not
 * established, and what makes it hard is that nothing counts.
 *
 * Every interactive trace makes the same journey. `engine.add_trace` puts it in
 * the world, `syncTracesToEditor` replaces the whole managed section of the
 * editor with `engine.export_traces_as_dsl()`, and the editor's own debounce
 * then fires `loadDesign(content)` - which clears the world and parses the text
 * again. So a trace exists in copper only for as long as the DSL writer can
 * spell it, and anything the writer cannot express is gone on the next
 * keystroke with nothing said. `dsl.rs` documents one such gap already: a via's
 * outer diameter is rebuilt from a default rather than read.
 *
 * This is the instrument the tracker asked for before the hunt: make the round
 * trip countable, then break it on purpose.
 */

/** How many trace blocks a piece of DSL declares, per net. */
export type TraceCensus = Map<string, number>;

/**
 * Count `trace` blocks by net name.
 *
 * Deliberately a scanner over the text rather than a call into the parser: the
 * question is whether what the writer produced survives, and asking the parser
 * to establish what the writer produced compares the pair with itself. A net
 * name may be quoted - `trace "VBUS+" {` - and that is the case a naive split
 * on whitespace loses, which is why the quoted form is tested.
 */
export function censusOfTraces(dsl: string): TraceCensus {
  const census: TraceCensus = new Map();
  if (!dsl) return census;

  // Line comments first, because the word appears in prose as often as in
  // syntax: `// a trace to nowhere` counted a net called `to` in the first
  // version of this, which is a counter that invents copper.
  const code = dsl.replace(/\/\/[^\n]*/g, '');

  // `trace` at the start of a statement, then a net name (quoted or bare),
  // then the block or its attribute list. Requiring the bracket is what
  // separates a declaration from the same word in running text.
  const pattern = /(^|[\s{};])trace\s+(?:"([^"]*)"|([A-Za-z_][\w.$+-]*))\s*[[{]/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(code)) !== null) {
    const net = match[2] !== undefined ? match[2] : match[3];
    if (net === undefined) continue;
    census.set(net, (census.get(net) ?? 0) + 1);
  }
  return census;
}

/** Every trace block in the text, across all nets. */
export function totalTraces(census: TraceCensus): number {
  let total = 0;
  for (const count of census.values()) total += count;
  return total;
}

/**
 * What a round trip dropped, net by net.
 *
 * Returns one line per net whose count fell, and nothing when the two agree or
 * when the count rose - a trace that appears is a different fault and this is
 * not the instrument for it.
 */
export function whatTheRoundTripLost(before: TraceCensus, after: TraceCensus): string[] {
  const lost: string[] = [];
  for (const [net, had] of before) {
    const kept = after.get(net) ?? 0;
    if (kept < had) {
      lost.push(`${net}: ${had} trace${had === 1 ? '' : 's'} in, ${kept} out`);
    }
  }
  lost.sort();
  return lost;
}

/**
 * Compare two stages of the journey and say so out loud if copper went missing.
 *
 * Returns the lines it reported, so a caller can put them somewhere a user
 * looks. A silent `console.log` is what this defect has been hiding behind.
 */
export function reportLostTraces(stage: string, beforeDsl: string, afterDsl: string): string[] {
  const before = censusOfTraces(beforeDsl);
  const after = censusOfTraces(afterDsl);
  const lost = whatTheRoundTripLost(before, after);
  if (lost.length > 0) {
    console.error(
      `[trace-census] ${stage} lost copper: ${totalTraces(before)} trace blocks in, ` +
        `${totalTraces(after)} out\n  ${lost.join('\n  ')}`,
    );
  }
  return lost;
}
