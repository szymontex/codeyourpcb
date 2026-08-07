# Routing: what the autorouter does, and what has been measured on it

This is the record of a vector that has run about twenty experiments on one
algorithm. Two of them are in the shipped defaults. Nine are not, and each is
here with the numbers that killed it, so nobody spends a week re-discovering
them.

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

Two numbers per board, and they are not interchangeable:

- **Violations**: everything the DRC reports that the fixture did not already
  have. Introduced, not total - the benchmark fixtures are KiCad boards with
  faults of their own.
- **Shorts**: the violations measured at exactly 0.00mm. Copper touching
  copper. A board with a short cannot work; a board with a 0.05mm gap where
  0.13mm was required is a yield risk a fab may still build. The scorer counts
  both, `cypcb check` prints the split, and variant ranking puts shorts ahead
  of the composite.

Where the numbers come from:

| board | what it is | violations / shorts today |
|---|---|---|
| `led_blink.kicad_pcb` | small, 18 segments in the default routing | 2 / 0 |
| `stm32_breakout.kicad_pcb` | dense, 296x256 cells at 0.254mm | 133 / 58 |
| `multi_ic.kicad_pcb` | large, 197x158 cells at 0.508mm | 140 / 37 |

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

The value was swept, not chosen (`via_price_sweep::what_a_via_should_pay_for_crowding`):

| price | led_blink | stm32_breakout | multi_ic | total |
|---|---|---|---|---|
| 0.00 | 3 / 0 | 168 / 82 | 61 / 29 | 232 / 111 |
| **0.25** | 2 / 0 | 121 / 58 | 73 / 37 | **196 / 95** |
| 0.50 | 2 / 0 | 159 / 91 | 87 / 54 | 248 / 145 |
| 1.00 | 2 / 0 | 138 / 76 | 109 / 59 | 249 / 135 |
| 2.00 | 2 / 0 | 147 / 84 | 164 / 105 | 313 / 189 |

The response is not monotone and the boards do not agree: multi_ic is happiest
at zero, stm32_breakout has a floor at 0.25, and led_blink needs any price
above zero to stop putting a via 0.05mm from a foreign trace.

## The noise band, and why the ratchets carry it

Prices a hundredth apart ask the router for the same trade. What they differ by
is what negotiated congestion does on its own
(`via_price_sweep::how_much_of_the_price_is_noise`):

| price | stm32_breakout | multi_ic |
|---|---|---|
| 0.22 | 145 / 74 | 92 / 54 |
| 0.24 | 126 / 63 | 77 / 42 |
| 0.25 | 121 / 58 | 73 / 37 |
| 0.26 | 149 / 81 | 61 / 28 |
| 0.28 | 138 / 66 | 75 / 44 |
| **spread** | **28 violations** | **31 violations** |

That is the same size as the differences a sweep is choosing between. **A
tuning value picked inside this band is noise with a decimal point.**

`DRC_RATCHETS` in `benchmark_validation.rs` therefore holds each dense
fixture's measured value plus its measured spread. This is not slack: a
threshold set to one run fails on any unrelated change that perturbs rip-up
ordering, and a gate that cries wolf gets ignored. led_blink has no band - it
returned 2/0 at every price above zero.

## What the trajectory looks like

`where_the_band_comes_from` prints the overused count per iteration:

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

The pattern across all of them: **pricing copper that exists pays, blocking or
pricing space somebody might want does not.** An empty congestion map is not
blindness - it lets the first net take the cheap line and charges the rest for
what it actually took.

Two settings that help one board and hurt another are kept as variants rather
than defaults, which is what `--variants` is for: `pad_zone_blocks_foreign_copper`
(Guarded Pads) and `via_ring_penalty` (Priced Via Rings). `PathFinder Bare
Centre Line` is the router without the copper reservation, kept as a control.

## Verification

Every number above comes from one of these. All are `--ignored` diagnostics
except the gate.

```sh
# The gate: routes all three fixtures, holds both columns
cargo test --release -p cypcb-autoroute --test benchmark_validation -- --ignored

# The via price, and how much of it is noise
cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored --nocapture

# Trajectory per iteration, and whether overuse tracks violations
cargo test --release -p cypcb-autoroute --test where_the_band_comes_from -- --ignored --nocapture

# What a finer grid costs
cargo test --release -p cypcb-autoroute --test resolution_sweep -- --ignored --nocapture

# Which variant each board picks
cargo test --release -p cypcb-autoroute --test variant_picks_per_board -- --ignored --nocapture

# Every violation of every routed fixture, with coordinates
cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture
```

Last verified: 2026-08-07.
