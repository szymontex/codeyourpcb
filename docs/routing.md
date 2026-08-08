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

Introduced is not after minus before: routing removes faults too. Every
unrouted pin the fixture starts with is a violation that a successful route
retires.

Where the numbers come from (`drc_report`, re-run 2026-08-08 on all five
fixtures):

| board | what it is | before | after (the ratchet) | introduced | shorts |
|---|---|---|---|---|---|
| `led_blink.kicad_pcb` | small, 21 routes | 12 | **2** | 2 | 0 |
| `stm32_breakout.kicad_pcb` | dense, 296x256 cells at 0.254mm, 908 routes | 144 | **239** | 221 | 136 |
| `multi_ic.kicad_pcb` | large, 197x158 cells at 0.508mm, 1003 routes | 270 | **317** | 247 | 166 |
| `shift_driver.kicad_pcb` | DIP and 0805, 2 layers, 700 routes | 159 | **81** | 72 | 33 |
| `qfp_fanout.kicad_pcb` | LQFP-64 at 0.5mm on 2 layers, 1177 routes | 140 | **343** | 331 | 183 |

**`multi_ic` moved without the router changing.** The row above read 289 before
and 336 after until this run; it is 270 and 317 now, with **introduced
unchanged at 247**. The board is the same board and the routing is the same
routing - what dropped is nineteen violations the fixture already had, and they
went when the exporter started clipping the legend off solderable copper and
the footprint courtyards started enclosing their own land pattern. `before` and
`after` move whenever the *checker* changes; `introduced` is the column a
routing experiment should be read on, which is why this file says so above.

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

First, what it is not. **The router is deterministic**: routing each of the five
fixtures three times with the same config gives identical violations, shorts,
segments, vias and total copper down to the nanometre
(`is_the_router_repeatable`). So every number in this file is reproducible and
every comparison in it is between two real settings rather than two dice rolls.
The gate runs that check, because Rust randomises `HashMap` iteration order per
process and a single map walked to order work would turn all of it into noise
without anything else noticing.

What the band *is*, then, is the router's sensitivity to a price it ought to be
insensitive to. Prices a hundredth apart ask for the same trade and get
different boards
(`via_price_sweep::how_much_of_the_price_is_noise`):

| price | stm32_breakout | multi_ic |
|---|---|---|
| 0.22 | 221 / 127 | 231 / 143 |
| 0.24 | 207 / 115 | 261 / 163 |
| 0.25 | 221 / 136 | 247 / 166 |
| 0.26 | 245 / 147 | 258 / 166 |
| 0.28 | 238 / 119 | 249 / 162 |
| **spread** | **38 violations, 32 shorts** | **30 violations, 23 shorts** |

That is the same size as the differences a sweep is choosing between. **A
tuning value picked inside this band is noise with a decimal point** - not
because the router wobbles, but because negotiated congestion amplifies a
hundredth of a price into a different rip-up order and a different board.

The two boards added on 2026-08-08 were measured the same way:

| board | band across 0.22..0.28 | width |
|---|---|---|
| `shift_driver` | 62 to 74 | 12 violations |
| `qfp_fanout` | 298 to 400 | **102 violations** |

`qfp_fanout`'s band is a third of the value it guards. Negotiated congestion is
least stable on a fine-pitch escape with only two layers, which is worth knowing
before any number measured on that board is read as a result: **a 50-violation
difference there is inside the noise.** Its ratchet catches a collapse, not a
regression, and tightening it would need a repeatable router rather than a
tighter threshold.

`DRC_RATCHETS` in `benchmark_validation.rs` therefore holds each dense
fixture's measured value plus its measured spread. This is not slack: a
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

Two measurements taken on 2026-08-08, neither of them a knob.

**The default is fourth of eight on every board.** `variant_picks_per_board`,
ranked as the router ranks them - complete first, then fewest shorts, then
composite:

| board | winner | the default's place | winner vs default |
|---|---|---|---|
| `led_blink` | `High-Density` | 4th | 1 / 0 against 2 / 0 |
| `stm32_breakout` | `Tight Pads` | 4th | 216 / 86 against 239 / 136 |
| `multi_ic` | `Pad Aware` | 4th | 248 / 106 against 317 / 166 |

Three different winners, and the shipped default is mid-table on all three. It
is not a bad setting so much as nobody's best. `cypcb route --in-house` routes
best-of-eight and hands over the winner, so the command line is unaffected;
what runs on the default is `--fast`, the viewer's single-shot path, and every
ratchet in CI.

**More than half of every introduced violation is a trace on a part's pad, in a
cell the grid had marked as a pad.** `grid_vs_checker`, PathFinder, introduced
clearance violations cross-tabbed by what the grid thought was in the cell:

| board | total | on a pad cell | `part <-> trace` on a pad cell |
|---|---|---|---|
| `stm32_breakout` | 206 | 151 (73%) | **109 (53%)** |
| `multi_ic` | 215 | 175 (81%) | **112 (52%)** |

The grid knew. The cell was marked `pad` and the search routed through it
anyway, because a pad zone switches every obstacle off within its radius so
that a route can reach the pad it is heading for - and inside that radius a
*foreign* part's pad is switched off with the rest. That is the mechanism
behind half the defect count on both dense boards, and it is the same one the
margin sweep above moves: at two cells instead of three, stm32_breakout loses
50 shorts.

### The fourth board, and what it found on its first run

`shift_driver.kicad_pcb` was added on 2026-08-08 to be a board no setting was
fitted on: three 74HC595 in a chain driving 24 LEDs, 55 parts, 156 pads, 2
layers, 68x48mm. It is generated by
`tests/fixtures/benchmark/make_shift_driver.py`, which declares the circuit as
parts and nets and knows nothing about the router. Through-hole DIP beside 0805
chips is a mix the other three do not have - they are SMD-dominant. It routes
complete: 700 routes, 159 violations before, **81 after, 72 introduced**, and
nothing but the board outline reports before routing.

It disagreed with the variant ranking immediately:

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

| rule | led_blink | stm32_breakout | multi_ic | shift_driver | shorts | dominated |
|---|---|---|---|---|---|---|
| **lexicographic (shipped)** | 1/0 | 216/86 | 248/106 | 109/28 | **220** | 0 |
| `+ 0 x shorts` | 1/0 | 216/86 | 248/106 | 75/35 | 227 | 0 |
| `+ 1 x shorts` | 1/0 | 216/86 | 248/106 | 75/35 | 227 | 0 |
| `+ 2 x shorts` | 1/0 | 216/86 | 248/106 | 82/31 | 223 | 0 |
| `+ 5 x shorts` | 1/0 | 216/86 | 248/106 | 82/31 | 223 | 0 |
| `+ 10 x shorts` | 1/0 | 216/86 | 248/106 | 109/28 | 220 | 0 |
| `+ 20 x shorts` | 1/0 | 216/86 | 248/106 | 109/28 | 220 | 0 |

**No rule picks a dominated board, and only one of the four fixtures separates
them at all.** Three boards pick the same variant under every rule; the whole
question rests on `shift_driver`, which is one board's opinion.

The shipped rule wins the criterion it was judged against - no dominated picks,
fewest shorts at 220 - so it stays. Recorded here rather than acted on, because
the alternative is a documented rule overturned on a single board, and because
the number that should decide it is not in this table: how much a fab's yield
actually suffers from a 0.05mm gap against a short. Nobody here has that
number.

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

**Two boards later, the rule that fit would have produced is wrong on both.**
`shift_driver` and `qfp_fanout` were added and swept at every margin:

| margin | led_blink | stm32_breakout | multi_ic | shift_driver | qfp_fanout |
|---|---|---|---|---|---|
| 0 cells | 1 / 1 | 250 / 91, 1 unrouted | 406 / 193 | 98 / 31 | 367 / **163** |
| 1 cell | 2 / 1 | 233 / 82 | 305 / 113 | 92 / 38 | 369 / 169 |
| 2 cells | 2 / 1 | **216 / 86** | **290 / 131** | 87 / 38 | 493 / 250 |
| **3 cells (default)** | **2 / 0** | 239 / 136 | 336 / 166 | **81 / 33** | **343** / 183 |
| 5 cells | 4 / 1 | 273 / 164 | 353 / 189 | 418 / 254 | 418 / 254 |

`shift_driver` sits at 0.0039 routes per cell - between `led_blink`'s 0.0011 and
`stm32_breakout`'s 0.0120, exactly where the threshold would have gone - and it
wants **3**, the sparse board's answer. `qfp_fanout`, the densest fine-pitch
board of the five, also wants 3. Only the two boards the rule was fitted on want
2.

So the density story was a coincidence of three samples, and the constant the
project ships is the best answer on three boards out of five. The refusal to fit
it is the finding: **eight instruments in the dropped table below lost after
being built; this one lost before, for the price of two fixtures.**

What the fifth board did not settle is the ranking - see above. Every rule picks
the same variant on `qfp_fanout`, so four of the five boards agree under all
seven rules and the question still rests on `shift_driver` alone.

## Instruments that were measured and dropped

Each of these was built, measured on all three fixtures, and reverted. The
numbers are introduced violations unless stated.

| instrument | result |
|---|---|
| Hard-block cells within clearance during expansion (tried twice) | 3 -> 4 violations, 22 -> 20 routes; second attempt 4 and 19 |
| Mark the via ring as owned copper | stm32_breakout 124 -> 154, multi_ic 42 -> 55 |
| One extra cell of obstacle bloat | stm32_breakout 259 -> 285, multi_ic 143 -> 157 |
| Refuse a via whose keepout holds foreign copper | led_blink 3 -> 2, but stm32_breakout 180 -> 215 and multi_ic 128 -> 245, shorts 29 -> 111 |
| Finer grid, 0.127mm instead of the track pitch | stm32_breakout 15.88 -> 39.46 violations per 100mm of copper, time doubled |
| Best of N net orderings | identical results at 1, 3 and 5 attempts - every rotation of `order_nets` is worse, so the best-of always returns the unrotated one |
| Return the iteration with the fewest overused cells | stm32_breakout 133 -> 152, multi_ic 140 -> 174 |
| Seed the congestion map from ratsnest density | stm32_breakout 121 -> 142, multi_ic 73 -> 112 at the lightest weight; multi_ic segments 706 -> 1188 |
| Route the crowded nets first instead of the short ones | stm32_breakout 259 -> 302, multi_ic 143 -> 150; faster, which is not the objective |
| Refuse a foreign net's pad inside the routing net's own pad zone | stm32_breakout 239 -> 250 after, and **six connections abandoned**; multi_ic 336 -> 451 |
| Weight the pad price by depth into the pad's disc, full on its copper and tapering across the clearance | multi_ic 267 -> 413 after at price 20 and 106 -> 242 shorts; stm32_breakout better at 5 and 50, worse at 20; no price where both improve |
| Open a net's pad zone for the connection's own two pads instead of every pad the net has | stm32_breakout 239 -> 299 after and 136 -> 175 shorts, multi_ic 336 -> 398 and 166 -> 219, nothing left unrouted on either |
| Charge a dearer layer change on a pad, to close the one short the narrower opening adds | the short survives every price to 1000 on led_blink, and stm32_breakout goes 216 -> 263 after at 150 |
| Make the pad zone layer-aware, so a surface-mount pad stops opening the layer it has no copper on | stm32_breakout 239 -> 290 after, multi_ic 317 -> 382, qfp_fanout 343 -> 437; led_blink and shift_driver unchanged |
| Let the via keepout price count foreign **pads**, not only foreign routed copper | at the shipped price of 0.25: stm32_breakout 239 -> 259, multi_ic 336 -> 392 with 166 -> 216 shorts. At a price of its own it works and still loses: see below |

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
swept. `pad_price_sweep::what_a_foreign_pad_should_cost`, after / shorts:

| price | led_blink | stm32_breakout | multi_ic |
|---|---|---|---|
| 0 (default) | 2 / 0 | 239 / 136 | 336 / 166 |
| 5 | 1 / 1 | 313 / 175 | 257 / 102 |
| **20 (Pad Aware)** | 1 / 1 | 280 / 141 | **267 / 106** |
| 50 | 1 / 1 | 318 / 178 | 466 / 249 |
| 100 | 1 / 1 | 332 / 166 | 573 / 299 |

Three readings, and only one of them is a result:

1. **multi_ic wants the price** - 336 to 257-267 after, 166 to about 105
   shorts, far outside any noise band measured on this board.
2. **stm32_breakout does not.** Every price makes it worse; it picks `Low-Via`
   in best-of-seven and never sees this one.
3. **led_blink trades the wrong way.** Two near misses become one short at
   every price above zero, which is backwards under this project's own
   ranking, and is why it keeps `High-Density`.

The 5 that reads best on multi_ic is not a better value:
`how_much_of_the_pad_price_is_noise` routes it at 4, 5, 6 and 7 and gets 288,
257, 278, 276 after - **31 violations of spread, 21 of shorts**, which swallows
the 10 that separates 5 from 20. The variant stays at 20.

### The pad opening, swept (`pad_zone_margin_cells`)

A pad zone switches off every obstacle within its radius so a route can reach
the pad it is heading for. The radius was the pad's own copper plus a flat
three cells, under a comment reading "generous but safe" - 0.762mm on the
0.254mm grid, wider than the gap between the two pads of an 0402.
`pad_zone_margin_sweep`, after / shorts:

| margin | led_blink | stm32_breakout | multi_ic |
|---|---|---|---|
| 0 cells | 1 / 1 | 250 / 91, **1 unrouted** | 406 / 193 |
| 1 cell | 2 / 1 | 233 / 82 | 305 / 113 |
| **2 cells** | 2 / 1 | **216 / 86** | **290 / 131** |
| 3 cells (default) | 2 / 0 | 239 / 136 | 336 / 166 |
| 5 cells | 4 / 1 | 273 / 164 | 353 / 189 |

Two cells is better than the shipped three on both dense boards and on both
columns - stm32_breakout by 23 violations and **50 shorts**, multi_ic by 46 and
35, all outside the noise bands measured above. led_blink goes the other way,
trading two near misses for one short, which under this project's ranking is
the wrong direction. So it ships as the `PathFinder Tight Pads` variant rather
than as the default, and **stm32_breakout picks it in best-of-eight** at
216 / 86 where its previous pick, `Low-Via`, gave 216 / 114.

Zero is not the floor: with no margin at all stm32_breakout leaves a connection
unrouted and takes seven times as long, because a route cannot always reach a
pad through the clearance the grid bloated around it.

### A pad under a via, priced separately (`via_foreign_pad_penalty`, default 0)

The blind spot named below is real and closing it does exactly what the theory
says. At the narrower opening, `led_blink`'s via-on-a-pad short disappears the
moment a pad in the keepout costs anything at all - after / shorts:

| price | led_blink | stm32_breakout | multi_ic |
|---|---|---|---|
| 0 (default) | 2 / 1 | 216 / 86 | 290 / 131 |
| 0.02 | **1 / 0** | 272 / 147 | 277 / 121 |
| 0.05 | **1 / 0** | 239 / 124 | 330 / 168 |
| 0.10 | **1 / 0** | 242 / 108 | 339 / 180 |

And at the shipped opening it moves stm32_breakout's shorts 136 -> 122 at 0.10
while multi_ic goes 166 -> 206.

It still ships at zero, because no price and opening together beat what each
board already picks: stm32_breakout has 216 / 86 from `Tight Pads` at zero,
multi_ic has 267 / 106 from `Pad Aware`, and led_blink has 1 / 0 from
`High-Density`. The mechanism is confirmed and the knob is there; what is
missing is a board that wants it, and none of these three does.

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
(Pad Aware, which multi_ic picks in best-of-eight at 267 after / 106 shorts
against the default's 336 / 166) and `pad_zone_margin_cells` (Tight Pads, which
stm32_breakout picks at 216 / 86). `PathFinder Bare Centre Line` is the router
without the copper reservation, kept as a control.

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

# Every violation of every routed fixture, with coordinates
cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture
```

Last verified: 2026-08-07.
