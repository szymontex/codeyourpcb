/**
 * One row per contact, not one row per pair of segments.
 *
 * The clearance rule reports every pair of features that come too close, and a
 * trace is a chain of segments: two features that touch along a run report
 * once for each segment that takes part. On `qfp_fanout` the shipped boards
 * give 759 rows for 484 contacts, and one pair of features accounts for 24 of
 * them. The count is the rule's own and stays as it is - what changes here is
 * the reading, where twenty-four items saying the same thing push everything
 * else off the panel.
 *
 * `cypcb check` and the language server already group this way; this is the
 * same rule for the viewer's error panel and its editor markers.
 */

/** Everything the grouping needs to know about a violation. */
export interface GroupableViolation {
  kind: string;
  message: string;
}

/** A violation and how many more rows the same two features produced. */
export interface GroupedViolation<T> {
  violation: T;
  others: number;
}

/**
 * The two features a message is about.
 *
 * `U1 <-> trace 'GND': Clearance violation: ...` - everything before the first
 * colon names the pair, and it is the same string however many segments of the
 * same two features report it.
 */
export function pairOf(message: string): string {
  const colon = message.indexOf(':');
  return colon === -1 ? message : message.slice(0, colon).trim();
}

/**
 * The gap a clearance message says it measured, in mm.
 *
 * A message that does not carry one sorts last, so a row with a number always
 * beats a row without.
 */
export function gapOf(message: string): number {
  const measured = message.match(/([\d.]+)mm actual/);
  return measured ? parseFloat(measured[1]) : Number.POSITIVE_INFINITY;
}

/**
 * One entry per contact, each where its worst row arrived.
 *
 * A contact takes the place of the row that is kept, which is the one with the
 * smallest gap - not the place the pair was first seen. That is what
 * `cypcb check` prints, and the two listings say the same thing in the same
 * order.
 *
 * Only `clearance` is grouped. The other kinds report per feature, and two of
 * their messages being equal is two faults rather than one seen twice.
 */
export function groupByContact<T extends GroupableViolation>(
  violations: T[]
): GroupedViolation<T>[] {
  const bestOfPair = new Map<string, number>();
  const extras = new Map<string, number>();

  violations.forEach((violation, index) => {
    if (violation.kind !== 'clearance') return;
    const pair = pairOf(violation.message);
    extras.set(pair, (extras.get(pair) ?? 0) + 1);
    const best = bestOfPair.get(pair);
    if (best === undefined) {
      bestOfPair.set(pair, index);
    } else if (gapOf(violation.message) < gapOf(violations[best].message)) {
      bestOfPair.set(pair, index);
    }
  });

  const kept: GroupedViolation<T>[] = [];
  violations.forEach((violation, index) => {
    if (violation.kind !== 'clearance') {
      kept.push({ violation, others: 0 });
      return;
    }
    const pair = pairOf(violation.message);
    if (bestOfPair.get(pair) !== index) return;
    kept.push({ violation, others: (extras.get(pair) ?? 1) - 1 });
  });
  return kept;
}

/** What the panel and the editor marker say about the rows not shown. */
export function morePlacesNote(others: number): string {
  const places = others === 1 ? 'place' : 'places';
  return `and ${others} more ${places} where the same two touch; this is the worst`;
}
