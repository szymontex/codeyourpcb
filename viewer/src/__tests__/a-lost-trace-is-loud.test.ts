import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  censusOfTraces,
  totalTraces,
  whatTheRoundTripLost,
  reportLostTraces,
} from '../trace-census';

/**
 * The instrument before the hunt.
 *
 * The owner reported traces disappearing while wiring by hand - draw one, draw
 * the next, the first one goes. Seen twice, cause not established. The tracker
 * records the shape of it: every interactive trace is written out as DSL and
 * parsed back, so a trace lives only as long as the writer can spell it, and
 * nothing counted.
 *
 * These are the counter's own tests. Breaking the round trip on purpose comes
 * after, and needs this to be trustworthy first.
 */

afterEach(() => {
  vi.restoreAllMocks();
});

describe('counting the traces a piece of DSL declares', () => {
  it('counts one block per trace, per net', () => {
    const dsl = `
trace GND {
  layer top
  width 0.25mm
  path 1mm,1mm to 5mm,1mm
}
trace VCC {
  layer top
  path 1mm,3mm to 5mm,3mm
}
trace GND {
  layer bottom
  path 1mm,5mm to 5mm,5mm
}`;
    const census = censusOfTraces(dsl);
    expect(census.get('GND')).toBe(2);
    expect(census.get('VCC')).toBe(1);
    expect(totalTraces(census)).toBe(3);
  });

  it('reads a net name that needs quoting', () => {
    // The case a split on whitespace loses. A design is free to name a net
    // `VBUS+` or `D-`, and a counter that cannot see those would report copper
    // missing that never went anywhere.
    const dsl = 'trace "VBUS+" {\n  path 0mm,0mm to 1mm,0mm\n}\ntrace "D-" {\n}';
    const census = censusOfTraces(dsl);
    expect(census.get('VBUS+')).toBe(1);
    expect(census.get('D-')).toBe(1);
    expect(totalTraces(census)).toBe(2);
  });

  it('does not count the word inside another identifier', () => {
    // `trace_width` and a net called `trace` are both things a real file has.
    const dsl = 'netclass Power [trace_width 0.5mm] { VCC }\n// a trace to nowhere\n';
    expect(totalTraces(censusOfTraces(dsl))).toBe(0);
    // The comment is the case that caught it: `// a trace to nowhere` counted
    // a net called `to` before the scanner required the block's own bracket.
    expect(censusOfTraces(dsl).has('to')).toBe(false);
  });

  it('counts nothing in an empty or absent section', () => {
    expect(totalTraces(censusOfTraces(''))).toBe(0);
    expect(totalTraces(censusOfTraces('board b { size 10mm x 10mm }'))).toBe(0);
  });
});

describe('what a round trip dropped', () => {
  const twoNets = 'trace GND {\n}\ntrace VCC {\n}\ntrace GND {\n}';

  it('says nothing when everything survives', () => {
    expect(whatTheRoundTripLost(censusOfTraces(twoNets), censusOfTraces(twoNets))).toEqual([]);
  });

  it('names the net and both counts when a trace goes missing', () => {
    const after = 'trace GND {\n}\ntrace VCC {\n}';
    const lost = whatTheRoundTripLost(censusOfTraces(twoNets), censusOfTraces(after));
    expect(lost).toEqual(['GND: 2 traces in, 1 out']);
  });

  it('reports a net that vanished entirely', () => {
    const after = 'trace GND {\n}\ntrace GND {\n}';
    expect(whatTheRoundTripLost(censusOfTraces(twoNets), censusOfTraces(after))).toEqual([
      'VCC: 1 trace in, 0 out',
    ]);
  });

  it('stays quiet when a trace appears, because that is a different fault', () => {
    const after = `${twoNets}\ntrace SIG {\n}`;
    expect(whatTheRoundTripLost(censusOfTraces(twoNets), censusOfTraces(after))).toEqual([]);
  });
});

describe('saying it out loud', () => {
  it('writes to console.error, not console.log', () => {
    // The whole reason this defect has survived two sightings is that the
    // round trip is silent. A lost trace is an error, and it goes where an
    // error goes.
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const lost = reportLostTraces('editor sync', 'trace GND {\n}\ntrace VCC {\n}', 'trace GND {\n}');

    expect(lost).toEqual(['VCC: 1 trace in, 0 out']);
    expect(spy).toHaveBeenCalledTimes(1);
    const said = String(spy.mock.calls[0][0]);
    expect(said).toContain('editor sync');
    expect(said).toContain('2 trace blocks in');
    expect(said).toContain('1 out');
    expect(said).toContain('VCC');
  });

  it('says nothing at all when nothing was lost', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(reportLostTraces('editor sync', 'trace GND {\n}', 'trace GND {\n}')).toEqual([]);
    expect(spy).not.toHaveBeenCalled();
  });
});
