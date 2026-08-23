# Routing: what the autorouter does, and what has been measured on it

This is the record of a vector that has run about twenty experiments on one
algorithm. Two of them are in the shipped defaults, three are variants a board
can pick, and ten are here with the numbers that killed them, so nobody spends
a week re-discovering them.

Read this before changing anything in `crates/cypcb-autoroute`.

## The algorithm

`PathFinder` (`pathfinder_v2.rs`) is negotiated congestion, the VPR algorithm:

1. Build a grid over the board. A cell is one track pitch - minimum trace width
   plus minimum clearance - unless the board is large enough that the adaptive
   rule doubles it.
2. Route every net with A*, cheapest path first, against an empty congestion
   map.
3. Count cells that more than one net occupies. Charge history cost on them.
4. Re-route only the nets that pass through an overused cell, at the new
   prices. Repeat.
5. Stop when no cell is shared, when the overused set has not shrunk for
   `stagnation_limit` iterations, or at the iteration cap.

Then `repair_routes` gets a pass, `postprocess` turns grid paths into segments
and vias, and `smoother` straightens what it can.

The pieces worth knowing when reading the code:

- `RoutingGrid` (`grid.rs`) holds occupancy per cell per layer, plus which net
  owns it. Pads, zones and locked traces are bloated by `min_clearance +
  min_trace_width / 2` and rounded up to whole cells.
- `CongestionMap` (`congestion.rs`) holds history cost and occupancy. Its
  `congestion_cost` is what makes a crowded cell expensive.
- `AutorouteConfig` (`lib.rs`) is every knob. The two that pay are described
  below; the rest are variants or diagnostics.
- `generate_variants` (`variant.rs`) routes the board several ways and keeps
  the best. Ranking is complete-first, then fewest shorts, then composite -
  never the composite alone, because a variant that abandons connections earns
  no violations for copper it never laid.

## What is measured

Three numbers per board, and mixing them up has cost this project a day
already. Two of them are called "violations" by something that prints them:

- **After**: every violation the DRC reports on the routed board. This is what
  `RoutingScore::drc_violations` holds - `violation_count()`, no subtraction -
  and therefore what the `DRC_RATCHETS` in `benchmark_validation.rs` compare
  against.
- **Introduced**: what the routed board reports that the fixture did not
  already report, matched violation by violation on kind, coordinate and
  message. This is what the sweeps below print, and what a routing experiment
  should be read on: the fixtures are KiCad boards with faults of their own,
  and an unrouted fixture reports every pin as unrouted.
- **Shorts**: the violations measured at exactly 0.00mm. Copper touching
  copper. A board with a short cannot work; a board with a 0.05mm gap where
  0.13mm was required is a yield risk a fab may still build. The scorer counts
  both, `cypcb check` prints the split, and variant ranking puts shorts ahead
  of the composite.

- **Contacts**: how many pairs of features the clearance violations describe.
  The clearance rule reports per pair of *segments*, and a trace is a chain of
  them: two features that touch along a run report once for each segment that
  takes part, so one contact can be two dozen rows. On a routed `multi_ic` the
  scorer reads **454 violations and 86 contacts**.

  Every count in this document is the row count, and that is deliberate:
  decided 2026-08-23, the rule keeps counting segment pairs because a violation
  is a *place* - two segments of one trace touching a pad at two points are two
  places an etch can fail - and because these numbers are regression ratchets
  before they are a report, where the finer count is the sensitive one.
  `RoutingScore::clearance_contacts` is published beside it, and `cypcb check`
  prints both when they differ, so a reader who wants "how many places on this
  board are in fault" has that number without any table here moving.

Introduced is not after minus before: routing removes faults too. Every
unrouted pin the fixture starts with is a violation that a successful route
retires.

Where the numbers come from (`drc_report`, re-run 2026-08-21 on all six
fixtures). Each board is graded against the fab table its own layer count asks
for, which the harness prints per row, so `multi_ic` is read against
`jlcpcb_standard_4layer` and the rest against `jlcpcb_standard_2layer`:

| board | what it is | table | before | after (the ratchet) | introduced | shorts |
|---|---|---|---|---|---|---|
| `led_blink.kicad_pcb` | small, 21 routes | 2layer | 12 | **2** | 2 | 0 |
| `stm32_breakout.kicad_pcb` | dense, 899 routes | 2layer | 156 | **199** | 175 | 95 |
| `multi_ic.kicad_pcb` | large, four copper layers, 970 routes | 4layer | 263 | **381** | 336 | 169 |
| `shift_driver.kicad_pcb` | DIP and 0805, 2 layers, 671 routes | 2layer | 156 | **65** | 65 | 34 |
| `plane_board.kicad_pcb` | poured GND plane, 181 routes | 2layer | 46 | **28** | 28 | 13 |
| `qfp_fanout.kicad_pcb` | LQFP-64 at 0.5mm on 2 layers, 1478 routes | 2layer | 140 | **318** | 318 | 149 |

**`multi_ic` moved because its yardstick did, and that is separated from
everything else that moved rather than asserted.** The harness graded all six
boards on the two-layer table until 2026-08-21; `multi_ic` has four copper
layers, so `cypcb check` reads it against the four-layer row and the harness did
not. Running the file both ways on the same commit isolates it: on the two-layer
table `multi_ic` gives 945 routes, 263 before, 316 after, 262 introduced, 7
hole-to-hole; on its own table, 970 / 263 / 381 / 336 / 8. **Every other board
is identical between the two runs.** The four-layer row is tighter on trace and
space and larger on the ring, so the router is solving a different problem and
the checker is marking a stricter one. A rule getting stricter is not a router
regression.

**The other four rows had already drifted, and this run does not say why.** The
table above was measured on 2026-08-08 and none of the rows except `led_blink`
reproduced today even on the old two-layer table - `stm32_breakout` read
144 / 239 / 221 and measures 156 / 199 / 175, `shift_driver` read 159 / 81 / 72
and measures 156 / 65 / 65, `qfp_fanout` read 140 / 336 / 336 and measures
140 / 318 / 318. Two weeks of checker and router changes sit between the two
dates and this file does not attribute the difference to any of them. `before`
and `after` move whenever the *checker* changes; `introduced` is the column a
routing experiment should be read on, which is why this file says so above.

`plane_board` was missing from this table entirely: it was added as a fixture
on 2026-08-08 and the table was written the same day.

## The two settings that pay

Both work the same way: **they charge for copper that exists**. Every
instrument that charged for copper somebody might want has lost.

### `reserve_trace_footprint` (default on)

A minimum-width trace is 0.127mm on a 0.254mm cell, so the cell beside the one
the search walked reads as free while the copper in it is touching. The router
marks the cells around each path node as well, and a via marks the cells its
ring covers - drill plus annular, not the keepout.

A reservation that cannot be relaxed is a veto, and vetoes lose here (see
below). This one yields: cells holding only reserved copper are marked
`CELL_HALO`, and a net that finds no path at all gets a second attempt that may
cross them. It never crosses another net's centre line, which is a short.

Measured when it landed: multi_ic 143 introduced violations to 64, copper
unchanged; stm32_breakout reached `Complete` where the strict version left 3
connections unrouted.

Re-measured 2026-08-21 with `drc_report`, whose second row is the *off* case
now - it had been set to `true`, which is the default, so the file ran the same
config twice under two labels and printed two identical halves. On today's
router the setting wins introduced violations on **all six** boards:

| board | on (shipped) | off |
|---|---|---|
| `led_blink` | 2 / 0 shorts | 4 / 2 |
| `stm32_breakout` | 175 / 95 | 265 / 181 |
| `multi_ic` | 336 / 169 | 401 / 272 |
| `shift_driver` | 65 / 34 | 95 / 27 |
| `plane_board` | 28 / 13 | 45 / 17 |
| `qfp_fanout` | 318 / 149 | 523 / 339 |

`shift_driver` is the one board that trades: 30 fewer introduced violations for
7 more shorts. Every other board is better on both columns, and the two that
this file used to say it cost - the small board's two violations - are on the
other side now.

### `via_foreign_copper_penalty` (default 0.25)

A via pays for each foreign cell inside the keepout its ring needs. A price,
not a veto - refusing such a via was measured and reverted.

The value was swept, not chosen
(`via_price_sweep::what_a_via_should_pay_for_crowding`, introduced / shorts,
re-measured 2026-08-07 after the router stopped deleting vias):

| price | led_blink | stm32_breakout | multi_ic | total |
|---|---|---|---|---|
| 0.00 | 4 / 1 | 265 / 129 | 272 / 173 | 541 / 303 |
| **0.25** | 2 / 0 | 221 / 136 | 247 / 166 | **470 / 302** |
| 0.50 | 2 / 0 | 234 / 145 | 250 / 165 | 486 / 310 |
| 1.00 | 2 / 0 | 205 / 125 | 237 / 150 | 444 / 275 |
| 2.00 | 2 / 0 | 195 / 119 | 254 / 179 | 451 / 298 |

The response is still not monotone, and one thing about it changed when the
vias stopped being deleted: **the boards now agree that a via should pay
something.** In the pre-fix sweep multi_ic was happiest at zero and the price
was a compromise; today zero is the worst column on all three fixtures, which
is the expected shape - a price on a via only bites once the via survives to
the output.

What the table does not license is a re-tune. 1.00 reads 16 violations better
than 0.25 on stm32_breakout and 10 on multi_ic, and both of those are inside
the noise band measured below (38 and 30). The shipped value stays at 0.25
until something moves a board further than the router moves it on its own.

## The noise band, and why the ratchets carry it

First, what it is not. **The router is deterministic**: routing each of the six
fixtures three times with the same config gives identical violations, shorts,
segments, vias and total copper down to the nanometre
(`is_the_router_repeatable`, re-checked 2026-08-21 with each board on the fab
table its own layer count asks for). So every number in this file is
reproducible and every comparison in it is between two real settings rather
than two dice rolls.

**One consequence of that table is worth stating here, because it explains
`multi_ic` rows throughout this file.** The grid cell is derived from the rule
set by the adaptive rule, and the four-layer row is tighter on trace and space,
so the cell is finer: `multi_ic` searches at **0.400mm** on its own table and at
**0.508mm** on the two-layer one. Every harness in this crate used the two-layer
table until 2026-08-21, so `multi_ic` rows measured before that date are a
different search and not merely a differently-marked result. Measured by running
`resolution_sweep` both ways on one commit.
The gate runs that check, because Rust randomises `HashMap` iteration order per
process and a single map walked to order work would turn all of it into noise
without anything else noticing.

What the band *is*, then, is the router's sensitivity to a price it ought to be
insensitive to. Prices a hundredth apart ask for the same trade and get
different boards
(`via_price_sweep::how_much_of_the_price_is_noise`):

Re-measured 2026-08-08 across all five sweepable fixtures at once, on boards
that are all fabricable for the first time - no copper outside an outline, no
two parts in the same place, no copper the file invents. Introduced violations
and shorts:

| price | stm32_breakout | multi_ic | shift_driver | qfp_fanout | plane_board |
|---|---|---|---|---|---|
| 0.22 | 220 / 149 | 200 / 133 | 65 / 34 | 331 / 175 | 28 / 13 |
| 0.24 | 231 / 154 | 265 / 189 | 66 / 35 | 366 / 191 | 28 / 13 |
| 0.25 | 172 / 93 | 255 / 187 | 65 / 34 | 309 / 147 | 28 / 13 |
| 0.26 | 188 / 112 | 236 / 171 | 67 / 34 | 312 / 166 | 28 / 13 |
| 0.28 | 182 / 104 | 245 / 176 | 50 / 27 | 311 / 167 | 28 / 13 |
| **spread** | **59 / 61** | **65 / 56** | **17 / 8** | **57 / 44** | **0 / 0** |

`plane_board` is the exception that sharpens the rule: it routes identically at
every price in the range, to the violation. A board with a ground plane has far
fewer nets competing for the same cells, so the rip-up order it produces does
not change when the price does. Every board that wobbles is a board where the
negotiation has room to go differently.

**The bands moved when the fixtures were repaired, and three of them widened.**
`stm32_breakout` went from 38 to 59 and `qfp_fanout` from 40 to 57. Any ratchet
set from an older band is now set inside its board's noise, which is a test
that fails for reasons unrelated to the change being made - `benchmark_validation`
carries the current numbers and the table it derives them from.

That is the same size as the differences a sweep is choosing between. **A
tuning value picked inside this band is noise with a decimal point** - not
because the router wobbles, but because negotiated congestion amplifies a
hundredth of a price into a different rip-up order and a different board.

The two boards added on 2026-08-08 were measured the same way:

| board | band across 0.22..0.28 | width |
|---|---|---|
| `shift_driver` | 62 to 74 | 12 violations |
| `qfp_fanout` | 296 to 336 | **40 violations** |

`qfp_fanout`'s band is the widest of the five in absolute terms, though not the
third of its value the broken version of the fixture showed - its headers used
to run past the board outline, and the numbers above are the re-measurement
after that was fixed. Negotiated congestion is least stable on a fine-pitch
escape with only two layers, which is worth knowing before any number measured
on that board is read as a result: **a 50-violation difference there is inside
the noise.** Its ratchet catches a collapse, not a
regression, and tightening it would need a repeatable router rather than a
tighter threshold.

`DRC_RATCHETS` in `benchmark_validation.rs` therefore holds each dense
fixture's measured value plus its measured spread. **`multi_ic`'s entry was
re-taken on 2026-08-21 by the same method** - the harness had graded every
board on a fixed two-layer table, and that board has four copper layers, so it
was marked against the wrong row and searched on a 0.508mm grid where the
shipped tool searches 0.400mm. Its routed value goes 316 / 200 to 381 / 175 and
its ratchet 356 / 243 to 415 / 224, loosening 59 on violations and tightening 19
on shorts. The other five are unchanged to the digit across that conversion,
which is the check that it reached nothing it should not. This is not slack: a
threshold set to one run fails on any unrelated change that perturbs rip-up
ordering, and a gate that cries wolf gets ignored. led_blink has no band - it
returned 2/0 at every price above zero.

## What the trajectory looks like

`where_the_band_comes_from` prints the overused count per iteration. The counts
below were measured on 2026-08-06, before the router stopped deleting vias;
what is claimed from them is the shape, and the shape is what bounds the work:

```
stm32_breakout 0.24: 12 iterations, converged false, [695, 514, 340, 361, 377, 336, 297, 284, 289, 319, 304]
stm32_breakout 0.26:  7 iterations, converged false, [673, 495, 320, 348, 346, 365]
```

Three facts follow, and they bound what is worth trying:

1. **The router never converges on either dense board.** It stops on the
   stagnation break with 250-380 cells still shared.
2. **The runs differ from iteration 1.** A price change alters the first
   routing, so everything after is a different trajectory. Nothing that only
   adjusts the late schedule can close the band.
3. **Overuse oscillates**, and in the tail it is inverted against board
   quality. `does_overuse_track_the_violations` routes stm32_breakout once per
   iteration cap: the board with the fewest violations of any iteration, 120,
   is the one the loop saw with its worst reading of 460 overused cells. The
   loop's stagnation test is reading a signal that has stopped meaning anything
   by the time it fires - and it costs about one violation, because the tail is
   flat (125, 145, 140, 125, 120, 121). Measured, and not worth a fix that
   doubles routing time.

Half the violations of a finished board are gone by iteration 4 (286 to 148 on
stm32_breakout). The leverage is in the head, not the tail.

## Where the violations actually are, and where the default sits

Two measurements, re-taken 2026-08-21 on six fixtures and thirteen variants,
each board ranked and graded on the fab table its own layer count asks for.
Neither is a knob.

**The default is nobody's best on five boards of six, and first on the sixth.**
`variant_picks_per_board`, ranked as the router ranks them - complete first,
then fewest shorts, then composite:

| board | winner | the default's place | winner vs default |
|---|---|---|---|
| `led_blink` | `High-Density` | 2nd | 1 / 0 against 2 / 0 |
| `stm32_breakout` | `Eager` | 2nd | 179 / 75 against 199 / 99 |
| `multi_ic` | `Eager Pads` | 4th | 371 / 128 against 381 / 175 |
| `shift_driver` | `Eager Light` | 7th | 62 / 20 against 65 / 34 |
| `plane_board` | `Eager Pads Priced Ring` | 5th | 10 / 4 against 28 / 13 |
| `qfp_fanout` | `Default` | **1st** | - |

Five different winners across six boards. This file used to say the default was
"fourth of eight on every board", measured on three fixtures; on six it ranges
from first to seventh, and the sentence it supported - that the default is not a
bad setting so much as nobody's best - now has one board that disagrees with it.
`cypcb route --in-house` routes best-of-N and hands over the winner, so the
command line is unaffected; what runs on the default is `--fast`, the viewer's
single-shot path, and every ratchet in CI.

**The majority of every introduced clearance violation sits in a cell the grid
had marked as a pad.** `grid_vs_checker`, PathFinder, introduced clearance
violations cross-tabbed by what the grid thought was in the cell:

| board | total | on a pad cell | `part <-> trace` on a pad cell |
|---|---|---|---|
| `led_blink` | 2 | 2 (100%) | 2 (100%) |
| `stm32_breakout` | 171 | **122 (71%)** | 85 (50%) |
| `multi_ic` | 203 | **131 (65%)** | 93 (46%) |
| `shift_driver` | 65 | **39 (60%)** | 19 (29%) |
| `plane_board` | 27 | **19 (70%)** | 15 (56%) |
| `qfp_fanout` | 291 | **161 (55%)** | 106 (36%) |

This file used to claim the stronger form - that more than half of every
introduced violation is specifically a `part <-> trace` fault on a pad cell,
measured at 53% and 52% on two boards. On six it is 50%, 46%, 29%, 56% and 36%,
so **that form is retracted**: what holds on every board is the pad cell, not
the part-to-trace shape within it.

The grid knew. The cell was marked `pad` and the search routed through it
anyway, because a pad zone switches every obstacle off within its radius so
that a route can reach the pad it is heading for - and inside that radius a
*foreign* part's pad is switched off with the rest. That mechanism accounts for
between 55% and 71% of the introduced clearance faults on every dense board
here.

### The fourth board, and what it found on its first run

`shift_driver.kicad_pcb` was added on 2026-08-08 to be a board no setting was
fitted on: three 74HC595 in a chain driving 24 LEDs, 55 parts, 156 pads, 2
layers, 68x48mm. It is generated by
`tests/fixtures/benchmark/make_shift_driver.py`, which declares the circuit as
parts and nets and knows nothing about the router. Through-hole DIP beside 0805
chips is a mix the other three do not have - they are SMD-dominant. It routes
complete: measured 2026-08-21 by `drc_report`, **671 routes, 156 violations
before, 65 after, 65 introduced**, and nothing but the board outline reports
before routing. When the fixture landed on 2026-08-08 the same line read 700
routes and 81 after; the board has not changed and the router has.

It disagreed with the variant ranking immediately, and the table below is that
disagreement **as it read on 2026-08-08**. It is kept because it is what the
section is about; the numbers no longer reproduce. Today `shift_driver` picks
`Eager Light` at 62 / 20 under the shipped rule, and the weighted rules pick a
variant reading 55 / 21 - so the front is still wide and still on this board:

| rank | variant | violations | shorts |
|---|---|---|---|
| **1** | `Bare Centre Line` | **109** | **28** |
| 2 | `Low-Via` | 82 | 31 |
| 3 | `Default` | 81 | 33 |
| 4 | `Pad Aware` | **75** | 35 |
| 8 | `High-Density` | 130 | 59 |

The ranking is lexicographic - complete, then fewest shorts, then composite -
so it hands over `Bare Centre Line` at 109 violations because it carries 28
shorts against `Pad Aware`'s 35. On the three older fixtures every rule picks
the same variant and this never showed.

**It was written up here as a defect, and the sweep below says it is not one.**
`Pad Aware` at 75/35 does not beat `Bare Centre Line` at 109/28: it is better on
one axis and worse on the other. Both sit on the Pareto front, and what the
fourth board actually revealed is that the front is **wide** - 34 violations
across for 7 shorts - where on the older three it is a point. Which end of it to
hand over is a judgement, not a bug.

### Which end of the front to hand over

`ranking_rule_sweep` scores seven rules against one routing pass per board -
the shipped lexicographic order, and `violations + W x shorts` for W in
0, 1, 2, 5, 10, 20. The criterion was written into the test before the numbers
were seen: a rule is out if it ever picks a **dominated** board, meaning another
complete variant has fewer violations *and* no more shorts; among the rest,
prefer the fewest shorts, since that is what this project ranks first.

Re-measured 2026-08-21 on all six fixtures, each ranked under the fab table its
own layer count asks for - which matters here more than anywhere else in this
file, because `multi_ic` searches a **0.400mm** grid on its own table against
0.508mm on the two-layer one, so ranking it under the wrong table ranks a
different set of routings:

| rule | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout | shorts | dominated |
|---|---|---|---|---|---|---|---|---|
| **lexicographic (shipped)** | 1/0 | 179/75 | 371/128 | 62/20 | 10/4 | 318/149 | **376** | 0 |
| `+ 0 x shorts` | 1/0 | 179/75 | 366/174 | 55/21 | 10/4 | 288/155 | 429 | 0 |
| `+ 1 x shorts` | 1/0 | 179/75 | 371/128 | 55/21 | 10/4 | 288/155 | 383 | 0 |
| `+ 2 x shorts` | 1/0 | 179/75 | 371/128 | 55/21 | 10/4 | 291/153 | 381 | 0 |
| `+ 5 x shorts` | 1/0 | 179/75 | 371/128 | 55/21 | 10/4 | 291/153 | 381 | 0 |
| `+ 10 x shorts` | 1/0 | 179/75 | 371/128 | 62/20 | 10/4 | 318/149 | 376 | 0 |
| `+ 20 x shorts` | 1/0 | 179/75 | 371/128 | 62/20 | 10/4 | 318/149 | 376 | 0 |

**No rule picks a dominated board, and three of the six fixtures separate them
rather than one.** `shift_driver`, `qfp_fanout` and - at `W = 0` only -
`multi_ic`. This file used to say the whole question rested on one board's
opinion; it rests on three, and the shipped rule still wins the criterion it
was judged against, with the fewest shorts at 376.

**Every difference between the rules is inside the boards' own noise bands,
though, and that is the reading this table did not have before.** `shift_driver`
separates by 7 violations and 1 short against a band of 17 / 8. `qfp_fanout`
separates by 30 and 6 against 60 / 46. `multi_ic` at `W = 0` separates by 5 and
46 against 34 / 49. Not one of those clears its own band. So the shipped rule is
kept because it wins the stated criterion and because nothing measured here can
tell the seven apart - not because the benchmark set prefers it.

The number that should decide it is still not in this table: how much a fab's
yield actually suffers from a 0.05mm gap against a short. Nobody here has that
number, and until somebody does, a rule change would be a preference rather than
a finding.

`variant_picks_per_board` no longer asserts that the winner also wins on
composite - that was asserting the opposite of the documented rule, and it is
what turned red here. It asserts what the feature promises: nothing the ranking
picks may be beaten on shorts *and* violations at once by a variant that routed
everything.

### What that does not license

The margin wants to be 3 on `led_blink` and 2 on the two dense boards, and the
boards separate cleanly by routes per grid cell - 0.0011 against 0.0120 and
0.0322, an order of magnitude. A threshold anywhere between would reproduce
every board's own best.

It was not implemented, because a two-regime rule fitted on three boards and
tested on the same three boards is not a measurement, it is a restatement.

**Two boards later, the rule that fit would have produced is wrong on both** -
and the table that showed it was measured on two fixtures that were broken at
the time. Re-measured 2026-08-08 on all six, after `multi_ic` stopped reading
two malformed coordinates as zero and `qfp_fanout` stopped collapsing two of
its four headers onto one geometry, and with `plane_board` added:

| margin | led_blink | stm32_breakout | multi_ic | shift_driver | qfp_fanout | plane_board |
|---|---|---|---|---|---|---|
| 0 cells | 1 / 1 | 250 / 91, 1 unrouted | 311 / 168 | 98 / 31 | 386 / 206 | **33** / 14 |
| 1 cell | 2 / 1 | 233 / 82 | 261 / 119 | 92 / 38 | 341 / 172 | 39 / **12** |
| 2 cells | 2 / 1 | **216** / 86 | **229** / **109** | 87 / 38 | 326 / 186 | 42 / 24 |
| **3 cells (default)** | **2** / **0** | 239 / 136 | 318 / 177 | **81** / **33** | **309** / **147** | 40 / 18 |

**Four of the six boards moved after this table was taken, all of them placement repairs rather than router changes.** `stm32_breakout` routes at 180 / 93 now against the 239 / 136 below, after two headers whose last four pins ran past its top edge were moved onto the board; `shift_driver` at 65 / 34 against 81 / 33, after three capacitors came in off the edge. And  `multi_ic` had twelve real courtyard overlaps repaired - it routes at 291 / 187 on 945 routes now, against the 318 / 177 above - and `plane_board` had one, and routes at 28 / 13 on 181 routes against its 40 / 18. Both are placement repairs rather than router changes: the boards are buildable now and they were not. The shape of the margin comparison is unaffected, since every column moved for the same reason and none of them by a margin change, but re-run the sweep before quoting a cell from it.
| 5 cells | 4 / 1 | 273 / 164 | 342 / 201 | 98 / 60 | 341 / 206 | 40 / 22 |

Two rows of the previous table were wrong and are corrected here: `multi_ic`
and `qfp_fanout` were measured on their broken selves, and `shift_driver` at
margin 5 carried `qfp_fanout`'s numbers copied into its column.

**Read against each board's own noise band, almost none of this is signal.**
The bands are the ones measured by `via_price_sweep::how_much_of_the_price_is_noise`:

| board | best margin | default (3) | difference | band | verdict |
|---|---|---|---|---|---|
| stm32_breakout | 216 at 2 | 239 | 23 | 30 | inside the band |
| multi_ic | 229 at 2 | 318 | **89** | 63 | **outside - wants 2** |
| shift_driver | 81 at 3 | 81 | 0 | 12 | the default is best |
| qfp_fanout | 309 at 3 | 309 | 0 | 40 | the default is best |
| plane_board | 33 at 0 | 40 | 7 | 9 | inside the band |

So the sentence this section used to carry - "only the two boards the rule was
fitted on want 2" - does not survive its own fixtures being repaired. On
today's numbers **one board separates the margins by more than its own noise,
and that board wants 2**; every other board is indifferent or prefers the
default. The `qfp_fanout` case in particular collapsed: it read 493 against 343
when its headers were stacked, and reads 326 against 309 now, which is well
inside its band of 40.

**Shorts tell a different story from violation counts, and it is worth
separating.** At the default the two dense boards short far more than at margin
2 - `stm32_breakout` 136 against 86, `multi_ic` 177 against 109 - while
`qfp_fanout` goes the other way, 147 against 186. A short is a fault a
fabricator will build; a clearance violation at 0.05mm is a fault a fabricator
will usually absorb. Anyone changing this constant should decide which of the
two they are minimising before reading the table, not after.

The constant is unchanged in this commit. Changing it moves five ratchets and
invalidates five measured bands, so it is its own piece of work with its own
before-and-after, not a line edited while writing a table.

### Margin 2 was then adopted, measured on all six, and reverted

The table above says margin 2 costs nothing outside any board's band and gains
`multi_ic` 89 violations. That was read on violation counts alone. The constant
was set to 2, every fixture re-routed, and every band re-measured at the new
value:

| board | violations 3 -> 2 | shorts 3 -> 2 | band 3 -> 2 |
|---|---|---|---|
| led_blink | 2 -> 2 | **0 -> 1** | not measured |
| stm32_breakout | 239 -> 216 | 136 -> 86 | 30 -> **77** |
| multi_ic | 318 -> **229** | 177 -> **109** | 63 -> 17 |
| shift_driver | 81 -> 87 | 33 -> 38 | 12 -> 11 |
| qfp_fanout | 309 -> 326 | 147 -> **186** | 40 -> 27 |
| plane_board | 40 -> 42 | 18 -> 24 | 9 -> 6 |

Totalled, margin 2 wins: 87 fewer violations and 67 fewer shorts across the
set. It was still reverted, for three reasons that only appear once the boards
are read one at a time.

**The gain sits entirely on the two boards the value would have been fitted
on.** `stm32_breakout` and `multi_ic` are the two that wanted 2 in the original
three-board sweep. Every fixture added since - `shift_driver`, `qfp_fanout`,
`plane_board` - is worse on shorts at 2, and so is `led_blink`. Four boards
against two, and the two are the ones that chose the value.

**`led_blink` gains its first short**, and it is not a near miss:

```
D1 <-> via 'GND': Clearance violation: 0.00mm actual, 0.13mm required
```

A via touching a foreign pad on the simplest board this project has. By this
document's own ranking - a short is a fault a fabricator builds, a 0.05mm
clearance miss is one they usually absorb - that outweighs violation counts
elsewhere.

**`stm32_breakout` becomes far less predictable.** Its spread across via prices
0.22..0.28 widens from 30 violations to 77. A board whose result moves by 77
with a setting nobody intends to change is a board whose ratchet measures the
weather.

So the answer is the same as the first time and the reason is sharper: this is
a value that reads well on the boards it was fitted to and badly on every board
added afterwards. It is the second entry in the dropped table below to be
dropped twice.


What the fifth board did not settle is the ranking - see above. Every rule picks
the same variant on `qfp_fanout`, so four of the five boards agree under all
seven rules and the question still rests on `shift_driver` alone.

## What the composite charges, and what it charges twice

`compute_composite` adds six terms with the default weights all at 1.0:

| term | charge |
|---|---|
| length | `total_length / board_diagonal` |
| vias | `via_count` |
| DRC | `drc_violations * 1000` |
| smoothness | `(1 - smoothness) * 100` |
| crossings | `crossings * 500` |
| balance | `(1 - layer_balance) * 50` |

Three of those were read for the first time on 2026-08-08, and two were not
measuring what their names say:

- **smoothness** scored 1.0 on every board ever routed without examining a
  corner - it looked for bends inside a trace entity and `apply_routes` gives
  every entity one segment. It measures real corners now and still reads 1.0,
  because this router lays 45-degree turns and a 45-degree turn costs nothing.
- **layer_balance** counted only the layers that carried copper, so a route
  using one layer of two scored a perfect 1.0 while a route using both scored
  0.200 on `led_blink`. It measures against the layers the board has now.
- **crossings** was correct, and it is charged twice. Two traces crossing at
  one point measure `crossings` 1, `drc_violations` 1, `shorts` 1: the
  composite pays 500 for the crossing and 1000 for the same contact called a
  short. The same two traces on opposite layers give zero for both.

The double charge is not a defect - both numbers are right about what they
measure - but the terms are not independent, and a weight tuned on one moves
the other. Nothing has been retuned on this: the finding is recorded so the
next person to touch `ScoreWeights` starts from it rather than from the
assumption that six terms mean six independent things.

## The stacked vias, and why the obvious fix is not one

Routing leaves holes on top of each other. Measured after routing, with
`drc_report`:

| fixture | hole-to-hole |
|---|---|
| stm32_breakout | 4 |
| multi_ic | 7 |
| qfp_fanout | 15 |

Most read as a net against itself:

```
hole-to-hole (12.446mm, 29.718mm): via 'VCC_3V3' <-> via 'VCC_3V3': 0.00mm actual, 0.50mm required
hole-to-hole (56.134mm, 41.910mm): via 'USB_DP'  <-> via 'OSC_IN':  0.00mm actual, 0.50mm required
```

which invites an obvious fix: a net's paths are each converted to vias
separately, so two paths that change layer at the same point would each ask for
one. Dropping the exact repeats per net is four lines.

**It is not the cause.** The drill files say so: at (56.134, 41.910) one via is
written into the `Top-In1` pass and the other into the through pass, so the two
have *different layer spans* and are not repeats of anything. Two holes at one
point with different depths is a routing decision, not a bookkeeping slip.

Measured anyway, because four lines is cheap: deduplicating by position and
span changed `qfp_fanout` from 309 violations to 291 and left **every
hole-to-hole count identical** - 4, 7 and 15 before and after. So it removes
something, and that something is not what it was written for. Reverted: a
change whose stated reason does not hold is a change nobody can maintain, and
the 18 violations it happened to remove are not worth carrying an explanation
that is false.

What a real fix has to do is stop the search from putting a via where the board
already has one, at any depth - which is a cost-model question, not a filter on
the output.

### The knob for that already exists, and it is the wrong lever

`CongestionMap` tracks which cells each via's ring covers and charges
`ring_penalty` per ring. The default is **0.0**: the search knows exactly where
every hole is and pays nothing to put another on top of it. That looks like the
whole answer.

Swept on all six fixtures, violations and hole-to-hole counts measured together
in one run (`what_a_via_ring_should_cost`, `--ignored`), re-measured 2026-08-21
with each board on the fab table its own layer count asks for:

| ring price | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout |
|---|---|---|---|---|---|---|
| 0 (default) | 2 / 0 | 199 / 4 | 381 / 8 | 65 / 0 | **28 / 0** | **318 / 27** |
| 1 | 2 / 0 | 210 / 4 | 390 / 12 | 64 / 0 | **28 / 0** | 385 / 17 |
| 3 | 2 / 0 | 264 / 4 | 393 / **6** | 66 / 0 | 34 / 0 | 374 / 21 |
| 8 | 2 / 0 | **171 / 1** | 396 / 11 | 66 / 0 | 34 / 0 | 384 / 20 |

Violations first, then hole-to-hole. Both columns now have a band:
`cypcb_autoroute::noise_band` gives 0, 59, 34, 17, 0, 60 for the violations and
`stacked_hole_band` gives 0, 3, 5, 0, 0, 24 for the holes.

**No price moves any board outside a band on either count, so the instrument is
still dropped and the reasons are now cleaner than they were.**

- `stm32_breakout` at 8 reads best on both columns - 171 against 199, one
  stacked hole against four - and neither move clears its band: 28 against 59,
  and 3 against 3, which is at it rather than past it.
- `multi_ic` is worse on violations at every price, by 9, 12 and 15 against a
  band of 34. Its stacking goes 12, 6, 11 against 8, all inside a band of 5.
  This file used to record it as *worse at stacking at every price tried*; one
  price improves it, and none of the four movements is distinguishable from
  noise.
- **`qfp_fanout` does not halve its stacked holes.** That reading is retracted:
  27 to 17 is 10, and this board's stacking band is **24** - the widest
  measured anywhere - so the whole movement is inside it. The 67 violations it
  pays at price 1 are real, being outside its band of 60. It is a cost with no
  measured benefit, not a trade.
- `plane_board` has a band of zero on both columns and prefers 0 or 1, which tie
  exactly at 28 / 0; 3 and 8 cost it 6 violations, and on that board 6 is
  signal.
- `shift_driver` and `led_blink` do not move at all outside noise.

The reason is in what the penalty charges for. A ring price makes the search
avoid *crossing* copper around an existing via. It says nothing about *placing*
a via on a cell that already has one, which is the fault being chased - and on
a crowded board, pushing routes away from rings creates the congestion that
makes another layer change attractive somewhere else. Dropped as the
nineteenth instrument.

### And the grid cannot be told, which is why `CELL_VIA` was never set

The obvious next step is to have the grid remember where the holes are, so a
cost term can ask. `CELL_VIA` has been defined in `grid.rs` since it was
written and is set nowhere, which looks like an oversight.

It is not. `is_free` reads

```rust
self.layers[layer][idx] == CELL_FREE
```

an equality against zero, so **any** bit set in a cell's byte is a hard veto.
Marking a via cell for information blocks it. Measured: recording each via on
the layers it spans, with nothing reading the flag, moved every board -
`qfp_fanout` 309 violations to 383 and `plane_board` 28 to 29, both past their
ratchets, while `multi_ic` went 291 to 222 and `shift_driver` 65 to 49. A
change that only writes a note is not supposed to have a before and after at
all.

### The fourth attempt: a hole count in the congestion map

Built where the third attempt said it had to go. `CongestionMap` gained a
`holes` array beside `ring` - how many vias pass through each cell, per layer -
marked when a net's path is committed, unmarked when it is ripped up, and read
by the search as `via_stack_penalty` per hole at the moment it decides to
change layer. Nowhere else: that is the only place a via is *placed*.

At 0.0 it reproduced the router exactly, every fixture byte for byte. Then:

| stack price | stm32_breakout | multi_ic | qfp_fanout |
|---|---|---|---|
| 0 | 180 / 4 | 291 / 7 | 309 / 27 |
| 5 | 180 / 4 | 291 / 7 | 309 / 27 |
| 20 | 180 / 4 | 291 / 7 | 309 / 27 |
| 100 | 180 / 4 | 291 / 7 | 309 / 27 |

Identical at every price, down to the route and via counts. A term that costs
a hundred and changes nothing is a term that is never charged.

**Two candidates for why, one of them now eliminated.** The first was config
plumbing: `variant.rs` builds its `AutorouteConfig` by copying seven fields and
ending `..AutorouteConfig::default()`, so anything else a caller sets is
replaced. It is not the cause. `via_price_sweep` sets
`via_foreign_copper_penalty` - which is not among those seven - calls
`route_board` directly, and gets a spread of 33 to 42 introduced violations, so
`route_board` reads fields outside that list. The sweep above used the same
route. (The seven-field copy is also not a defect in itself: what it drops
falls back to the shipped defaults, which are the measured values.)

The second candidate was that a net's rings are marked only once all of that
net's paths are committed, so the holes marked beside them would be invisible
to the net placing them. **Measured, and it is not the cause either.**
Instrumenting the via-transition branch to count how often it prices a layer
change at a cell already covered by a via ring:

| fixture | layer changes priced | on existing via copper | vias placed |
|---|---|---|---|
| multi_ic | 2,836,140 | 46,181 (1.63%) | 119 |
| qfp_fanout | 1,783,840 | 49,376 (2.77%) | 186 |

Tens of thousands of times per board, the search evaluates putting a via where
one already is, **with that information in front of it**. The congestion map
holds it at the right moment; the marking is not too late.

### The fifth attempt: the bug found, the price measured, and it still does not pay

The fourth attempt's wiring bug was one line. `congestion_map.set_stack_penalty(..)`
was never added beside `set_ring_penalty`, so the map's penalty stayed 0.0 and
`stacking_cost` returned zero however high the knob was turned. That is why a
price of 100 produced byte-identical boards.

Rebuilt with the check first, which is the whole lesson of the fourth attempt:
routing `multi_ic` left **364 holes recorded for 119 vias** when this was
written - about three layers each - so the map is populated before any price is
swept against it. The via count has moved since; the check is that the array is
non-empty, and it is.

Re-measured 2026-08-21, after the harness started grading each board on its own
fab table. `multi_ic` is read against `jlcpcb_standard_4layer` here; the rows
below are all measured that way, so they compare with each other and not with
the numbers this table carried before.

| stack price | stm32_breakout | multi_ic | qfp_fanout |
|---|---|---|---|
| 0 (shipped) | 199 / 4 | 381 / 8 | 318 / 27 |
| 5 | 230 / **0** | 394 / **3** | 385 / 31 |
| 20 | 208 / 2 | 394 / **3** | 389 / 31 |
| 100 | 223 / 1 | 394 / **3** | 391 / 30 |

Violations first, then stacked holes. Both columns now have a band:
`cypcb_autoroute::noise_band` for the violations and
`cypcb_autoroute::stacked_hole_band` for the holes, both measured as the spread
across five via prices a hair apart. **The answer is one per board, not one.**

| board | violation band | stacking band | violations at price 5 | stacking at price 5 | verdict |
|---|---|---|---|---|---|
| `stm32_breakout` | 59 | 3 | +31, noise | 4 to **0**, outside | it pays |
| `multi_ic` | 34 | 5 | +13, noise | 8 to 3, **at the band** | undecided |
| `qfp_fanout` | 60 | 24 | +67, **outside** | 27 to 31, noise | it costs |

`stm32_breakout` clears every stacked hole it has, and the 31 violations it
gives back are well inside its own noise. `qfp_fanout` gets 67 violations worse,
outside its band, and its stacking barely moves against a band of 24 - the
widest here, and the reason nothing about that board's stacking can be concluded
cheaply. `multi_ic` moves 5, which is exactly its band, so it is not evidence in
either direction.

**One board of three is not a default.** That is the shape this vector keeps
arriving at - the same reason `Eager Pads` and `Eager Light` are variants rather
than settings. The knob stays at 0.0 and stays measurable; what it is missing to
become a variant is a board-picks mechanism for `AutorouteConfig` fields that
the variant list does not already carry.

**Kept at 0.0 rather than reverted**, unlike the fourth attempt. The difference
is that this one demonstrably works: every nonzero price changes the boards, so
a future sweep is one command rather than a week of archaeology, and the
shipped default reproduces the router exactly. That is the same basis
`via_ring_penalty` is kept on.

**So the fourth attempt failed on its own wiring, not on the idea.** A price at
this decision point can work, because the data it needs is demonstrably there.
The fifth attempt should confirm the hole array is populated before trusting a
sweep of it - the cheapest check is to assert a nonzero count after routing one
fixture, which the fourth attempt never did.

This also explains why pricing the ring did nothing for stacking while
demonstrably changing the boards: it charges every one of those 46,181
evaluations for *crossing* copper, which moves routes around, and charges
nothing extra for the one thing being chased.

Reverted. A knob that provably does nothing is worse than no knob: it reads
like a lever and moves nothing, and the next person spends their fire finding
that out again.

So the memory a via-price term needs cannot live in the occupancy byte. It
belongs beside `CongestionMap`'s ring array, which already carries per-cell
counts the search reads as cost rather than as permission. Anyone starting
that work should start there, and should know that every veto this router has
been given has lost.

Note on numbers: this file used to record that `qfp_fanout` read 27 stacked
holes here and 15 in `drc_report`, and blamed two rule sets that do not share a
hole-to-hole minimum. **That reason was wrong and the gap is gone.** Both
harnesses ran their DRC through `DesignRules::jlcpcb_2layer`, which is one
constructor, so they could not have differed for the stated reason; and both
report 27 today. The 15 was a stale figure, not a second measurement. The advice
under it still holds for a different reason: rows measured on different fab
tables or on different dates do not compare, so compare within one table and
check the date it carries.

### The over-block is load-bearing, which was not what anybody expected

`docs/router-plan.md` is built on a claim: that the seventeen instruments in
the table below failed because a grid of whole cells cannot hold the quantity
they were reaching for. The via keepout is the case whose arithmetic was
already written down - `0.15 + 0.127 + 0.127 + 0.0635 = 0.4675mm`, which is
1.84 cells at 0.254mm and becomes 2 after `ceil`, a disc of 0.508mm around a
ring that is 0.277mm across. So the plan made it step 2, the narrowest possible
first consumer, and said that if no board moved outside its own band there, the
premise was weaker than the plan claimed.

The premise did worse than that. Measured in nanometres instead of cells, the
disc drops from 13 cells to 9 and **three boards get worse outside their own
bands while none gets better outside its own**:

| board | shipped | exact disc | band | reading |
|---|---|---|---|---|
| `led_blink` | 2 / 0 | **3 / 1** | 0 / 0 | regression, and a short on the simplest board |
| `stm32_breakout` | 187 / 99 | 213 / 113 | 59 / 61 | inside its band |
| `multi_ic` | 304 / 200 | 251 / 149 | 65 / 56 | inside its band |
| `shift_driver` | 65 / 34 | 67 / 39 | 17 / 8 | inside its band |
| `qfp_fanout` | 318 / 149 | 343 / **199** | 57 / 44 | shorts +50 against a band of 44 |
| `plane_board` | 28 / 13 | **38 / 19** | 0 / 0 | regression on the board that never moves |

`plane_board` is the one that settles it. It routes identically at every via
price in the sweep range - that is why its band is zero - and it moved by ten
violations and six shorts here. This is not the negotiation going differently;
it is the board getting worse.

**The obvious rescue was tried and lost.** The shipped price of 0.25 was tuned
against a 13-cell disc, so a 9-cell disc collects 1.44 times less crowding for
the same geometry, and 0.25 x 13/9 = 0.36 is the price that charges the same
total. At 0.36: `plane_board` 45 / 26, worse than both; `qfp_fanout` 393 / 226,
worse than both; `led_blink` still 3 / 1. Compensating the price does not
recover the boards, so what the extra ring of cells was doing is not
arithmetic that a coefficient can replace.

What it was doing is the interesting part, and it is a hypothesis rather than a
result: the over-block is a **margin**, and the search has been relying on it.
A via priced only where its copper actually reaches will sit one cell closer to
foreign copper, and one cell at this resolution is 0.254mm - more than the
0.127mm the fab requires. The quantisation was buying a safety margin nobody
wrote down, and taking it away leaves the price to do a job it was never
measured doing.

That does not refute the field, which measures what it says it measures. It
refutes the assumption that the grid's roundings are only ever costs. Anything
built on the field from here has to supply its own margin explicitly rather
than inherit one by accident - and the next instrument to try is a barrier term
that is non-zero *before* contact, which is what `docs/router-plan.md` step 4
already describes.

## Instruments that were measured and dropped

Each of these was built, measured and reverted. The fixture set grew from three
boards to six while this table did, so a row names the boards it was measured
on rather than claiming a set. The numbers are introduced violations unless
stated.

| instrument | result |
|---|---|
| Hard-block cells within clearance during expansion (tried twice) | 3 -> 4 violations, 22 -> 20 routes; second attempt 4 and 19 |
| Mark the via ring as owned copper | stm32_breakout 124 -> 154, multi_ic 42 -> 55 |
| One extra cell of obstacle bloat | stm32_breakout 259 -> 285, multi_ic 143 -> 157 |
| Refuse a via whose keepout holds foreign copper | led_blink 3 -> 2, but stm32_breakout 180 -> 215 and multi_ic 128 -> 245, shorts 29 -> 111 |
| Pad opening at 2 cells instead of 3 (tried twice) | Totals better - 87 fewer violations, 67 fewer shorts - but the gain is entirely on the two boards the value was fitted on; led_blink gains a 0.00mm via-to-pad short and stm32_breakout's band widens 30 -> 77 |
| Finer grid, 0.127mm instead of the track pitch | stm32_breakout 15.88 -> 39.46 violations per 100mm of copper, time doubled |
| Best of N net orderings | identical results at 1, 3 and 5 attempts - every rotation of `order_nets` is worse, so the best-of always returns the unrotated one |
| Return the iteration with the fewest overused cells | stm32_breakout 133 -> 152, multi_ic 140 -> 174 |
| Seed the congestion map from ratsnest density | stm32_breakout 121 -> 142, multi_ic 73 -> 112 at the lightest weight; multi_ic segments 706 -> 1188 |
| Route the crowded nets first instead of the short ones | stm32_breakout 259 -> 302, multi_ic 143 -> 150; faster, which is not the objective |
| Refuse a foreign net's pad inside the routing net's own pad zone | stm32_breakout 239 -> 250 after, and **six connections abandoned**; multi_ic 336 -> 451 |
| Weight the pad price by depth into the pad's disc, full on its copper and tapering across the clearance | multi_ic 267 -> 413 after at price 20 and 106 -> 242 shorts; stm32_breakout better at 5 and 50, worse at 20; no price where both improve |
| Open a net's pad zone for the connection's own two pads instead of every pad the net has | stm32_breakout 239 -> 299 after and 136 -> 175 shorts, multi_ic 336 -> 398 and 166 -> 219, nothing left unrouted on either |
| Charge a dearer layer change on a pad, to close the one short the narrower opening adds | the short survives every price to 1000 on led_blink, and stm32_breakout goes 216 -> 263 after at 150 |
| Taper the foreign-pad price by distance to the target, so a route pays full price only far from the pin it is reaching | `Pad Aware` on stm32_breakout 280 -> 332 after, qfp_fanout 558 -> 568 with shorts 330 -> 353; multi_ic 248 -> 244, inside its band; led_blink and shift_driver unchanged |
| Make the pad zone layer-aware, so a surface-mount pad stops opening the layer it has no copper on | stm32_breakout 239 -> 290 after, multi_ic 317 -> 382, qfp_fanout 343 -> 437; led_blink and shift_driver unchanged |
| Let the via keepout price count foreign **pads**, not only foreign routed copper | at the shipped price of 0.25: stm32_breakout 239 -> 259, multi_ic 336 -> 392 with 166 -> 216 shorts. At a price of its own it works and still loses: see below |
| Measure the via keepout in nanometres instead of whole cells, removing the 83% over-block | three boards worse outside their own bands and none better outside its own: `led_blink` 2/0 -> 3/1 and `plane_board` 28/13 -> 38/19, both on a band of zero, and `qfp_fanout` shorts 149 -> 199 against a band of 44. Compensating the price for the smaller disc makes it worse again: see below |

The pattern across all of them: **pricing copper that exists pays, blocking or
pricing space somebody might want does not.** An empty congestion map is not
blindness - it lets the first net take the cheap line and charges the rest for
what it actually took.

The last row is worth reading twice, because it refutes the obvious reading of
the cross-tab. Part-to-trace **on a cell the grid marked as a pad** is the
largest group of introduced faults on both dense boards - 109 of
stm32_breakout's 206 and 112 of multi_ic's 215 - and the obvious cause is that
a net's pad zone opens every pad the net has rather than the two the connection
runs between. Narrowing it to those two makes both boards **worse**, with
nothing abandoned. So the router is not walking through pads it has no business
near: it is inside its own pad's disc, colliding with a **neighbouring part's**
pad that the disc happens to cover. The zone's radius is the suspect, not its
scope.

The row before it is the seventh veto tried in this vector and the seventh to lose,
which is now a strong enough prior to state as a rule: **if the instrument you
are about to write returns a bool, write it as an f64 instead and measure the
price.** The same geometry, priced, is the `PathFinder Pad Aware` variant below.

### When the pad zone is open, which was the last untested dimension

**Which** pads a zone opens and **how wide** it opens them are both measured and
in the table above. **When** was not: the zone is open for the whole search, so
a route can cut through a stranger's pads on the far side of the board as
easily as it can reach its own pin.

Priced rather than forbidden, since eight vetoes have lost and a price
sometimes wins: `foreign_pad_penalty` scaled by how far the node still is from
its target, nothing at the pad and full price twenty cells away. It only moves
`Pad Aware`, because the shipped price is 0.

| board | `Pad Aware` today | with the taper |
|---|---|---|
| `led_blink` | 1 / 1 | 1 / 1 |
| `stm32_breakout` | **280 / 141** | **332 / 153** |
| `multi_ic` | 248 / 106 | **244 / 104** |
| `shift_driver` | 75 / 35 | 75 / 35 |
| `qfp_fanout` | 558 / 330 | 568 / **353** |

One board better by 4 violations, which is inside its measured band of 30. One
worse by 52, which is outside its band of 38. Reverted.

Reading it with the fifteen before it: pricing *where* copper may go has now
lost as a veto, as a flat price on a board that does not want it, as a taper by
depth into a pad, and as a taper by distance to the goal. The lever that keeps
working is the one that changes what the search *sees* - the trace footprint
reservation and the via ring - not what it is charged for crossing.

### The layer the pad zone forgot, and why remembering it lost

`PadZone` is a disc in x and y with no layer: `in_pad_zone(x, y, zones)`. A
pad zone switches obstacles off so a route can reach the pad it is heading
for, and it did that on **every** layer - so a surface-mount pad on the top
made foreign copper invisible on the bottom too, where the pad it exists for
has no copper at all. That reads like a plain modelling error, and it is the
kind of thing this file exists to stop being fixed on faith.

Giving `PadZone` the pad's own `layer_mask` and checking it costs:

| board | after, before the change | after, with it |
|---|---|---|
| `led_blink` | 2 | 2 |
| `stm32_breakout` | **239** | **290** |
| `multi_ic` | **317** | **382** |
| `shift_driver` | 81 | 81 |
| `qfp_fanout` | **343** | **437** |

Three boards worse by 51, 65 and 94 - all far outside their measured bands -
and two unchanged. The route counts moved too (stm32_breakout 908 -> 1031,
multi_ic 1003 -> 964), so the search really did take different paths.

The mechanism is the one this table has recorded eight times before, wearing a
new hat: **a restriction loses.** The blind opening was letting a route escape a
crowded pad area by dropping to the other layer; closing that escape is
correct about the copper and worse about the board. Reverted.

### The pad price, swept (`foreign_pad_penalty`)

A net's pad zone opens every cell near any of its own pins so a route can reach
them, and the pin next door comes free with it: 109 of stm32_breakout's 118
part-to-trace faults are routes taking that opening. The price shipped at 20 on
one measured point, which is the mistake the via price made before it was
swept. `pad_price_sweep::what_a_foreign_pad_should_cost`, after / shorts,
re-measured 2026-08-21 on all six fixtures with each board graded on the fab
table its own layer count asks for:

| price | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout |
|---|---|---|---|---|---|---|
| 0 (default) | 2 / 0 | 199 / 99 | 381 / 175 | 65 / 34 | 28 / 13 | 318 / 149 |
| 5 | 1 / 1 | 211 / 111 | 366 / 174 | 59 / 36 | 22 / 14 | 451 / 269 |
| **20 (Pad Aware)** | 1 / 1 | 262 / 152 | **348 / 129** | 59 / 36 | 22 / 14 | 442 / 276 |
| 50 | 1 / 1 | 281 / 157 | 458 / 185 | 59 / 36 | 25 / 16 | 528 / 303 |
| 100 | 1 / 1 | 330 / 174 | 563 / 224 | 59 / 36 | 27 / 17 | 525 / 287 |

Bands from `cypcb_autoroute::noise_band`: `led_blink` 0 / 0, `stm32_breakout`
59 / 61, `multi_ic` 34 / 49, `shift_driver` 17 / 8, `plane_board` 0 / 0,
`qfp_fanout` 60 / 46.

**No board wants this price by a margin outside its own band, and that reverses
what this section used to say.** The old table read `multi_ic` from 336 down to
257 and called it "far outside any noise band measured on this board". Today the
best row is 20, at 348 against 381 - a move of **33 against a band of 34**, and
46 shorts against a band of 49. Both are inside. The reading that made this
section is gone, and it did not survive the two things that changed under it:
that board is graded on its own four-layer table now, and its noise band was
re-measured from 65 / 56 to 34 / 49.

The board-by-board answers, each against its own band:

1. **`multi_ic` is undecided, not a win.** 33 and 46 are inside 34 and 49.
   `how_much_of_the_pad_price_is_noise` says it more directly: routing the same
   board at 4, 5, 6 and 7 gives 323, 366, 360 and 367 after - **44 violations
   of spread, 47 of shorts** on a knob moving by one unit, which is wider than
   anything the price buys.
2. **`stm32_breakout` does not want it.** 199 to 262 at 20 is 63, outside its
   band of 59, and 50 and 100 are worse still. Only price 5 is inside.
3. **`qfp_fanout` does not want it, by the largest margin here** - 318 to 451 at
   price 5, and no row below 442.
4. **`led_blink` and `plane_board` trade the wrong way.** Both have a band of
   zero on both columns, so every movement on them is signal: `led_blink` turns
   two near misses into one short, and `plane_board` gives up six violations to
   gain one short. Under this project's own ranking - complete first, then
   fewest shorts - that is backwards, and it is why `led_blink` keeps
   `High-Density`.
5. **`shift_driver` is noise.** Six fewer violations against a band of 17, two
   more shorts against a band of 8.

**The variant stays at 20 and this file no longer claims a measurement behind
it.** `Pad Aware` is a point in the variant list, and the list is ranked per
board by `variant_picks_per_board`; what is gone is the separate claim that 20
is where the price *should* sit. Nothing here says it should sit anywhere.

### The pad opening, swept (`pad_zone_margin_cells`)

A pad zone switches off every obstacle within its radius so a route can reach
the pad it is heading for. The radius was the pad's own copper plus a flat
three cells, under a comment reading "generous but safe" - 0.762mm on the
0.254mm grid, wider than the gap between the two pads of an 0402.
`pad_zone_margin_sweep`, after / shorts, re-measured 2026-08-21 on all six
fixtures with each board graded on the fab table its own layer count asks for.
The previous version of this table was three boards measured before either
fixture was repaired, and it said in its own text that a six-board table
elsewhere in this file disagreed with it. This run settles that: the six-board
one is current and the three-board one is gone.

| margin | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout |
|---|---|---|---|---|---|---|
| 0 cells | 1 / 1 | 158 / 63, **1 unrouted** | 447 / 174 | 79 / 31 | 30 / 20 | 384 / 205 |
| 1 cell | 2 / 1 | 217 / 95 | 405 / 138 | 73 / 36 | 39 / 16 | 348 / 172 |
| 2 cells (`Tight Pads`) | 2 / 1 | 281 / 148 | 397 / 189 | 71 / 34 | 36 / 19 | 324 / 185 |
| **3 cells (default)** | **2 / 0** | **199 / 99** | **381 / 175** | **65 / 34** | **28 / 13** | **318 / 149** |
| 5 cells | 4 / 1 | 297 / 204 | 388 / 155 | 81 / 54 | 44 / 25 | 350 / 210 |

**The shipped three cells is now the best row on every board, which reverses
what this section used to conclude.** The old text said two cells beat three on
both dense boards and both columns, "all outside the noise bands measured
above". Today two cells is worse than three on all six: `stm32_breakout` by 82
violations, which is outside its band of 59; `led_blink` and `plane_board` have
a band of zero on both columns, so their losses are signal too; the rest are
inside their bands and so are not evidence either way. Nothing prefers two.

`PathFinder Tight Pads` is still a point in the variant list, and the list is
ranked per board. What is gone is this section's separate claim that two cells
is the better opening - and with it the claim that `stm32_breakout` picks
`Tight Pads`. Measured 2026-08-21, no board picks it: `led_blink` takes
`High-Density` at 1 / 0, `stm32_breakout` `Eager` at 179 / 75, `multi_ic`
`Eager Pads` at 371 / 128, `shift_driver` `Eager Light` at 62 / 20,
`plane_board` `Eager Pads Priced Ring` at 10 / 4, `qfp_fanout` `Default` at
318 / 149.

Zero is not the floor: with no margin at all stm32_breakout leaves a connection
unrouted, because a route cannot always reach a pad through the clearance the
grid bloated around it. That part held - it is the one row of the old table
that reproduced.

### A pad under a via, priced separately (`via_foreign_pad_penalty`, default 0)

The blind spot named below is real and closing it does exactly what the theory
says. At the narrower opening, `led_blink`'s via-on-a-pad short disappears the
moment a pad in the keepout costs anything at all. Re-measured 2026-08-21 on
all six fixtures at both openings, after / shorts.

At the shipped opening of three cells:

| price | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout |
|---|---|---|---|---|---|---|
| 0 (default) | 2 / 0 | 199 / 99 | 381 / 175 | 65 / 34 | 28 / 13 | 318 / 149 |
| **0.02** | 2 / 0 | 179 / 89 | 400 / 150 | **51 / 23** | **25 / 11** | 293 / 147 |
| 0.05 | 2 / 0 | 196 / 107 | 396 / 185 | 51 / 23 | 25 / 11 | 335 / 168 |
| 0.10 | 2 / 0 | 217 / 126 | 386 / 179 | 52 / 24 | 25 / 11 | 334 / 169 |

At two cells:

| price | led_blink | stm32_breakout | multi_ic | shift_driver | plane_board | qfp_fanout |
|---|---|---|---|---|---|---|
| 0 | 2 / 1 | 281 / 148 | 397 / 189 | 71 / 34 | 36 / 19 | 324 / 185 |
| 0.02 | **1 / 0** | 182 / 81 | 377 / 153 | 54 / 29 | 38 / 17 | 280 / 152 |
| 0.05 | **1 / 0** | 199 / 97 | 378 / 190 | 47 / 22 | 36 / 16 | 313 / 177 |
| 0.10 | **1 / 0** | 210 / 120 | 373 / 151 | 46 / 22 | 38 / 17 | 277 / 139 |

**At the shipped opening a price of 0.02 is a real improvement on two boards
and a loss on none.** `plane_board` has a band of zero on both columns and goes
28 / 13 to 25 / 11, so both are signal. `shift_driver` drops 11 shorts against a
band of 8, which is outside it. Every other board's movement is inside its own
band in one direction or the other, and no board gets worse outside its band.

That is a stronger reading than this section carried before, and it was still
not a reason to ship anything. **The variant was built and ranked on 2026-08-21,
and it does not earn its place.**

A fourteenth point, `PathFinder Priced Pad Landing` - the default router with
this price at 0.02 and nothing else changed - was added to
`default_variant_configs` and ranked against the other thirteen on all six
boards. One board picks it: `qfp_fanout`, at **293 / 147 against `Default`'s
318 / 149**. That is 25 violations and **2 shorts**, against a band of 60 / 46.
Inside it, on both columns. Under this file's own rule - a move inside the band
is the negotiation going differently, not a better setting - that is not an
adoption.

The two boards where the price genuinely helped against the default config
already have better variants than it produces. `plane_board` goes 28 / 13 to
25 / 11 with the price, and picks `Eager Pads Priced Ring` at **10 / 4**.
`shift_driver` goes 65 / 34 to 51 / 23, and picks `Eager Light` at 62 / 20 -
fewer shorts, which is what the ranking sorts on before anything else.

So the variant was reverted rather than kept, and the price stays at zero. The
mechanism is still real and still uncosted; what this run establishes is that
pricing it does not beat what any board already has. Anyone returning to it
should start from a price other than 0.02, because 0.02 has now been ranked.

The one fault that keeps two cells out of the defaults has a name:
`D1 <-> via 'GND': 0.00mm` - a via whose ring lands on a part's pad. Two prices
were measured against it and both lost, and the second one found something
worth writing down: **`foreign_cells_in_via_keepout` counts `net_at` only**,
which is routed copper. A pad's net lives in `pad_net`, because a rip-up clears
`net_map` and a pad is not ripped up. So a via pays for landing its ring on
another net's trace and **nothing for landing it on another net's pad**. That
blind spot is real; counting those cells at the shipped price of 0.25 is not
the fix, because the disc around a via covers many pad cells on a dense board -
stm32_breakout 239 -> 259 and multi_ic 336 -> 392 with 50 more shorts.

Settings that help one board and hurt another are kept as variants rather than
defaults, which is what `--variants` is for: `pad_zone_blocks_foreign_copper`
(Guarded Pads), `via_ring_penalty` (Priced Via Rings) and `foreign_pad_penalty`
(Pad Aware) and `pad_zone_margin_cells` (Tight Pads). Neither is picked by any
board as of 2026-08-21 - `variant_picks_per_board` gives `High-Density`,
`Eager`, `Eager Pads`, `Eager Light`, `Eager Pads Priced Ring` and `Default` -
which is what a list of points looks like when the space has been searched
since they were added, not a reason to delete them. `PathFinder Bare Centre Line` is the router
without the copper reservation, kept as a control.

### The weighted heuristic, swept (`heuristic_weight`, default 1.0)

A* explores widely because its estimate of the remaining distance never
overestimates: that is what makes the path it returns the cheapest one.
Multiplying the estimate makes the search believe the goal is further than it
is, so it follows the most promising direction harder and settles for a path
that may cost up to that factor more. It is the standard way to trade
optimality for speed, and `AutorouteConfig::heuristic_weight` has been there
the whole time at 1.0, unmeasured.

All six fixtures at 1.0, 1.1, 1.25 and 1.5. The router is deterministic at a
fixed weight - the sweep run twice gives identical counts on every board, so
what follows is signal or it is nothing.

| board | 1.0 | 1.1 | 1.25 | 1.5 | band |
|---|---|---|---|---|---|
| led_blink | 2 / 0 | 4 / 2 | 3 / 1 | 3 / 1 | 0 / 0 |
| stm32_breakout | 184 / 97 | 218 / 129 | 167 / 74 | 192 / 111 | 59 / 61 |
| multi_ic | 297 / 193 | 252 / 158 | 233 / 121 | 308 / 195 | 65 / 56 |
| shift_driver | 65 / 34 | 66 / 21 | 69 / 29 | 99 / 25 | 17 / 8 |
| plane_board | 28 / 13 | 30 / 15 | 28 / 11 | 29 / 13 | 0 / 0 |
| qfp_fanout | 309 / 147 | 328 / 168 | 289 / 151 | 360 / 187 | 57 / 44 |

Violations / shorts, and each board's own noise band from
`via_price_sweep::how_much_of_the_price_is_noise` - the same bands the
ratchets carry.

**Read against those bands, 1.25 changes almost nothing.** Its totals look
like a win - 885 violations down to 789 and 504 shorts down to 428 - but board
by board every one of those movements is inside the board's own band except
three:

- multi_ic's shorts, 193 -> 121 against a band of 56: a real improvement.
- plane_board's shorts, 13 -> 11 against a band of zero: a real improvement.
- led_blink, 2 -> 3 violations and **0 -> 1 short**, against a band of zero on
  both: a real regression, on the simplest board in the set.

**What is not noise is the time.** Summed over the six boards, 2.44s at 1.0
against 1.68s at 1.25 - 31% faster - and 0.99s at 1.5. That is the trade the
instrument was built to make, and it makes it.

**The default stays 1.0.** A short is the one fault a user notices without
reading anything, and buying 31% off a run that already finishes in under a
second per board by introducing one on the simplest fixture is the wrong way
round. 1.1 is worse than both on almost every board and 1.5 is worse than 1.25
everywhere but the clock.

What this does *not* say is that the weight is worthless. Two of the three
signals were improvements, both on the boards with the most copper, and the
one regression is on the board with the least. A weight that varies with the
board - or one swept per variant, since each of the eleven is a different cost
model - is a different experiment from this one, and this is the measurement
it would start from.

## Verification

Every number above comes from one of these. All are `--ignored` diagnostics
except the gate.

```sh
# The gate: routes all three fixtures, holds both columns. Prints "after".
cargo test --release -p cypcb-autoroute --test benchmark_validation -- --ignored

# Before, after and introduced per fixture, with the kinds behind each
cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture

# The via price, and how much of it is noise
cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored --nocapture

# Trajectory per iteration, and whether overuse tracks violations
cargo test --release -p cypcb-autoroute --test where_the_band_comes_from -- --ignored --nocapture

# What a finer grid costs
cargo test --release -p cypcb-autoroute --test resolution_sweep -- --ignored --nocapture

# What a foreign pad should cost, and how much of that price is noise
cargo test --release -p cypcb-autoroute --test pad_price_sweep -- --ignored --nocapture

# How wide the opening around a pad should be
cargo test --release -p cypcb-autoroute --test pad_zone_margin_sweep -- --ignored --nocapture

# Which variant each board picks
cargo test --release -p cypcb-autoroute --test variant_picks_per_board -- --ignored --nocapture

# What a weighted heuristic buys, and what it costs
cargo test --release -p cypcb-autoroute --test heuristic_weight_sweep -- --ignored --nocapture

# Every violation of every routed fixture, with coordinates
cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture
```

Last verified: 2026-08-21, for the two tables `drc_report` and
`what_a_stacked_hole_should_cost` feed. The rest of this file carries its own
dates; where a section does not, it has not been re-measured since 2026-08-09.
