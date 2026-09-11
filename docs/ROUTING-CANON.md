# Routing canon: what a good route is, in numbers

A router can only be judged against rules that carry a number or a condition a
program can test. This file is that list, and next to it the honest account of
which rules this project already enforces, which ones nothing enforces, and
which ones cannot be enforced at all until the data model carries a field it
does not carry today.

It is not a style guide and it is not a tutorial. Every rule here is either a
threshold or a predicate over a finished route.

## How sources are tagged

Every number carries a tag, because a number quoted from a blog looks exactly
like a number read out of the standard, and the difference only surfaces when a
fabricator disputes it.

- `[R]` A reproduction of a standard's table or formula in somebody else's
  text. Not the text of the standard.
- `[P]` Board house or vendor material.
- `[O]` The original article by the author of the rule.
- `[D]` Practice reported in discussion, with no number attached.

Dates are the day the source was read, not the day it was published.

## The rules

### R-01 Trace width against current and temperature rise `[R]`

A trace must have enough cross section for the current its net carries at the
temperature rise the design accepts.

    I = k * dT^0.44 * A^0.725          A in square mils, dT in degrees C
    k = 0.048 external, 0.024 internal
    A = (I / (k * dT^0.44))^(1/0.725)
    1 oz copper = 1.378 mil

Internal layers get half the constant because they dissipate into laminate
rather than into air. Condition: for every trace on a net that declares a
current, `width * thickness >= A_required` on each layer the net occupies.

Source: reproduction of the IPC-2221 formula, schemalyzer.com trace width
guide, read 2026-09-11. The same page puts the simplified form within 10% for
0.5 A to 10 A.

In this repo: enforced. `TraceCurrentRule` (`crates/cypcb-drc/src/rules/trace_current.rs`),
registered in `run_drc`, computed through `cypcb-calc`. Silent unless the net
declares `current`.

### R-02 Conductor spacing against working voltage `[R]`

Spacing between conductors of different nets must grow with the peak voltage
between them.

IPC-2221B table 6-1, millimetres, by peak voltage:

| V peak | B1 internal | B2 external, uncoated | B4 external, polymer coated |
|---|---|---|---|
| 15 | 0.05 | 0.1 | 0.05 |
| 30 | 0.05 | 0.1 | 0.05 |
| 50 | 0.1 | 0.6 | 0.13 |
| 100 | 0.1 | 0.6 | 0.13 |
| 150 | 0.2 | 0.6 | 0.4 |
| 170 | 0.2 | 1.25 | 0.4 |
| 250 | 0.2 | 1.25 | 0.4 |
| 300 | 0.2 | 1.25 | 0.4 |
| 500 | 0.25 | 2.5 | 0.8 |

Above 500 V the table becomes per-volt: B2 is `2.5 + 0.005 * (V - 500)` mm and
B1 is `0.25 + 0.0025 * (V - 500)` mm.

Source: reproduction of table 6-1, smpspowersupply.com IPC-2221 clearance page,
read 2026-09-11; the per-volt forms also appear in the protoexpress IPC-2221
article, read 2026-09-11. Columns B3 (above 3050 m) and A5 to A7 were not in
the reproduction and are a gap in this canon.

In this repo: the table exists and nothing calls it. See "Blocked on the model".

### R-03 Acute angles in copper `[P]`

No corner of a trace, and no junction of a trace with a land, forms an internal
angle below 90 degrees.

Etchant sits in the wedge of an acute corner longer than it sits on open
copper, and undercuts the trace from inside the corner. The accepted fix is two
45 degree corners, or a radius, in place of one acute corner.

Condition: the count of junctions with an internal angle below 90 degrees is 0.

Source: nextpcb and pcbsync acid trap articles, read 2026-09-11. The threshold
is an angle, not a dimension; no source gives a length.

In this repo: not enforced. `min_acid_trap` exists in
`crates/cypcb-rules/src/constraints.rs:167` and no code reads it.

The cut that removes such a junction, and the floor below which cutting is
cosmetic, are R-10.

### R-04 Stub length `[O]`

An unterminated branch resonates at a quarter wavelength and notches the
signal's own band out of it.

    length [in] < 0.3 / bit rate [Gbps]
    length [cm] < 0.75 / bit rate [Gbps]

At 1 Gbps in FR4 that is 0.3 in. Quarter-wave resonance in FR4 runs about
1.5 GHz per inch of stub, or 3.8 GHz per cm. A residual stub of 5 to 10 mil
pushes resonance near 150 GHz, which is out of the way of digital signalling.

Condition: on a net that declares a bit rate, the longest path from a branch
point to an end that is neither a pad nor a via is shorter than `0.3 / BR`
inches.

Source: EDN, "How long a stub is too long?: Rule of Thumb #18", read
2026-09-11.

In this repo: not enforceable yet, the model has no connectivity graph and no
declared signal speed. `max_stub_length`
(`crates/cypcb-rules/src/constraints.rs:91`) has no reader either. See "Blocked
on the model".

### R-05 Return path under a signal trace `[O]`

Return current takes the path of least impedance: least resistance at low
frequency, least inductance above the crossover, which is a band directly under
the trace.

The crossover, where plane resistance and inductive reactance are equal, is
given as roughly 5 kHz; above it most of the return flows in a band a few trace
heights wide beneath the signal trace.

Two separate conditions, both measurable on a finished route:

1. Coverage: the share of trace length with continuous reference copper on the
   adjacent copper layer beneath it is 100%.
2. Splits: the number of intersections between the trace's footprint and the
   edge of the reference pour is 0.

Source: learnemc.com, "Tracing Electric Current Paths", read 2026-09-11.

Gap: no public source found with a closed form for return current density
against lateral distance, so this canon states no number for the width of the
band. `sigcon.com/Pubs/news/3_7.htm` was checked on 2026-09-11 and does not
carry one.

In this repo: nothing measures either condition.

### R-06 Violations are reported per rule, not as one total `[O]`

A ranking that adds every rule together at the same price cannot tell a trace
that will overheat from a trace 10 um under the fab's minimum.

Condition: the score carries a count per violation kind, not a single
`drc_violations` total.

Source: this project's own measurement - see `shorts` and `clearance_contacts`
in `crates/cypcb-autoroute/src/scoring.rs`, which are exactly this split done
twice by hand. `crates/cypcb-autoroute/tests/a_crossing_is_charged_twice.rs`
runs the case that makes the total misleading: one contact, two terms, 1500
points.

In this repo: the data is there and the aggregation is not. `DrcViolation`
carries `kind: ViolationKind` with 35 variants
(`crates/cypcb-drc/src/violation.rs:51-120`) plus `actual` and `required` as
numbers rather than prose.

This rule gives the vector; R-11 says how to read it. Split apart they invite
the defect they were written against - a count per kind that is then added up
again is the single total under another name.

### R-07 Annular ring and hole-to-hole spacing `[R]`

A land must exceed its hole by two annular rings plus the fabrication
allowance, and laminate must remain between holes.

    land = hole + 2 * min_annular_ring + fabrication_allowance

IPC-6012E minimum annular ring: 0.001969 in (0.050 mm) external, 0.000975 in
(0.0248 mm) internal, measured from the drilled diameter. Class 2 permits up to
90 degrees of breakout with a teardrop at the junction. Fabrication allowance:
0.0045 in at tangency for a single lamination, 0.006 in for class 3; multiple
laminations take 0.006 in on the first cycle and 0.007 in after.

Hole to hole, edge to edge: not below 6 mil (0.15 mm), preferably 8 mil.
IPC-2221 section 9.2.4, section 9.2.7 in revision B.

Conditions: `outer_diameter >= drill + 2 * min_annular_ring` for every via, and
edge-to-edge distance between any two holes at least 0.15 mm.

Sources: summitinterconnect land size article; allpcb via-to-via spacing guide;
Altium "Vias 101". All read 2026-09-11.

In this repo: enforced, and the only rule of this group that is. `AnnularRingRule`,
`PadLandRule`, `HoleToHoleRule`, `ViaDiameterRule`, `ViaDrillRule` and
`DrillAspectRatioRule` all sit in the `run_drc` registry, and the router reads
`min_via_annular_ring` in `pathfinder_v2.rs`.

### R-08 Trace entry into a land `[P]`

A trace enters a land square on or at 45 degrees; an acute angle between trace
and land edge is not allowed.

Two failure modes, not one. The wedge traps etchant the way any acute corner
does, and a drill that wanders breaks the trace off the land. A teardrop
removes the internal angle and adds copper at the transition.

Conditions: the count of trace-to-land junctions with an internal angle below
90 degrees is 0; a teardrop is present wherever trace width is below land
diameter.

Sources: Altium DFM guidance on trace routing and solder joints; kingsunpcb
trace angle guide; nwengineeringllc on teardrops under class 3. All read
2026-09-11.

In this repo: no rule checks either condition, but half the model is already
there - `teardrops` is a DSL property with length and width ratios
(`crates/cypcb-parser/src/parser.rs:331-345`), reachable as `world.teardrops()`
(`crates/cypcb-world/src/dsl.rs:922`), and honoured by the Gerber writer and
the KiCad export.

### R-09 Thermal relief at a pad in a pour `[P]`

A pad tied into a pour needs spokes, or the pour sinks the soldering heat and
the joint comes out cold.

Spoke width at least 0.2 mm (8 mil); four spokes is both the norm and the
practical maximum. Typical pairs of spoke width and gap: 0.20 to 0.30 mm with
0.25 to 0.40 mm for general SMD, 0.30 to 0.50 mm with 0.40 to 0.60 mm for
through hole, 0.10 to 0.20 mm with 0.20 to 0.30 mm for 0402 and 0603. Below 3 A
continuous a relief is acceptable and costs roughly 1 to 3 milliohms; above 3 A
the connection should be solid. Bottom terminated parts - QFN, DFN, DPAK - take
a solid connection and a via array under IPC-7093.

Conditions: spoke count between 2 and 4, each spoke at least 0.2 mm wide, and
no relief on a net declaring more than 3 A.

Source: JLCPCB thermal relief design article, read 2026-09-11. Vendor material
citing IPC-2221B and IPC-7093; not the text of either standard.

In this repo: partly enforced. On the export path the geometry takes its
relief numbers from the house preset - `pour_thermal_gap` and `pour_spoke_width`
on `ExportPreset`, handed to the filler by `pour_options`
(`crates/cypcb-export/src/job.rs:115-119`). Two things are still outside that
wiring. The spoke count is not a parameter at all: `thermal_spokes()`
(`crates/cypcb-core/src/pour.rs:272`) cuts a fixed cross of four whatever the
house publishes, and `thermal_relief_spokes` has no reader. And the pour the
viewer draws still fills from `PourOptions::default()`
(`crates/cypcb-render/src/lib.rs:1959`), so what a designer sees on screen is
not what the preset orders.

What the export preset orders is held to the house's own design rules by
`crates/cypcb-cli/tests/the_kicad_board_carries_the_rules_it_was_checked_with.rs`,
which asserts `pour_thermal_gap` against `thermal_relief_gap` and
`pour_spoke_width` against `thermal_relief_spoke_width` for the same house.

This went unseen for a long time because the two shipped export presets both
publish 0.254 mm for gap and for spoke width, which is exactly what
`PourOptions::default()` uses (`crates/cypcb-core/src/pour.rs:242-261`). The
drawn copper agreed with the published table by coincidence, not by wiring, and
a house publishing anything else would have been silently ignored.

### R-10 Mitring an acute junction `[P]`

An interior angle below 90 degrees is cut away, not left, and the cut is
asymmetric.

**The geometry of the cut.** Two arms leave one point 45 degrees apart. Trim
`a` from one arm and `a * sqrt(2)` from the other; the chord between the two
new points then runs on a multiple of 45 degrees and the two joints it creates
are 90 and 135 degrees. A symmetric cut - equal trim on both arms - puts the
chord at 112.5 degrees, which is not a multiple of 45 and which this project's
own `is_valid_angle` (`crates/cypcb-autoroute/src/smoother.rs:22`) rejects.

**The floor.** `a >= 1.5 * w`, where `w` is trace width. Two bands of width `w`
whose centre lines meet at 45 degrees have already merged into one piece of
copper within `w / (2 * sin 22.5 degrees) = 1.307 * w` of the apex, so a cut
closer than that lands inside solid copper and moves the wedge rather than
removing it. 1.5 is 1.307 rounded to a number that can be written without a
square root beside it.

**When not to cut at all.** An interior angle below 45 degrees is copper
doubling back on itself. That is a path defect, not a corner defect, and the
connection is rerouted rather than mitred; cutting it hides the detour and
keeps it.

**What this project measured.** 53 wedges across the six benchmark boards,
every one of them at exactly 45 degrees. Of 58 wedges scanned for room, 58
clear the `1.5 * w` floor; the median shortest arm is `4.00 * w`, and the
per-board medians are 2.00, 5.66, 11.31, 2.00, 2.00 and 2.83 times the trace
width. This is this project's own measurement, the same status as the split
behind `shorts` and `clearance_contacts` in R-06, and it is a snapshot of the
router's defaults rather than a constant - the command that reproduces it is in
the verification block, and `stop_at_own_copper` moves it.

**In this repo:** the junctions are counted and none are cut. `acute-angle`
(`crates/cypcb-drc/src/rules/acute_angle.rs`) reports them; no pass in
`crates/cypcb-autoroute` rewrites them. `chamfer_corners`
(`crates/cypcb-autoroute/src/smoother.rs:342`) acts only on a 90 degree bend
and cannot reach this case: it classifies both segments and proceeds only when
one is horizontal and the other vertical - `is_90_bend = (dir_a == Horizontal
&& dir_b == Vertical) || (dir_a == Vertical && dir_b == Horizontal)`, and `if
!is_90_bend` pushes the segment through untouched
(`crates/cypcb-autoroute/src/smoother.rs:378-388`).

#### What the sources bound, and what they do not

**1. No published source found that bounds the angle at a junction of two
traces.** What is published is the 90 degree threshold for a corner of one
trace and for a trace meeting a land, stated repeatedly as a fabrication rule:
avoid angles below 90 degrees where a trace changes direction or meets a pad,
use a 45 degree chamfer or a curve instead (fabricator DFM guides, read
2026-09-11). Two things are absent from everything read. First, no clause
number: searching for IPC-2221 or IPC-2222 text on acute conductors returns
vendor articles that assert the rule and no standard text that states it, so
the 90 degree figure is `[P]` and not `[R]`. Second, nothing bounds the angle
where two separately routed runs of one net meet, which is exactly the geometry
this router produces. The searches that returned nothing, on 2026-09-11: IPC
clause text for acute conductors; DRC rules for a trace-to-trace junction
angle; fabricator rule lists for a minimum angle between traces. The same
sources are also explicit that this class of rule sits in DFM rather than in
DRC - a board can pass DRC and fail DFM on an acid trap - which is why no tool
reports it.

**2. No published mitre dimension covers this geometry.** The compensation
literature solves a 90 degree bend in a single trace, and generic fabrication
guidance gives the shape of the fix without a dimension - replace one 90 degree
corner with two 45 degree corners, or round it (fabricator DFM guides, read
2026-09-11). The only number available for a 45 degree junction between two
arms is this project's own `1.307 * w` merge distance.

An arc is the other accepted fix and removes the internal angle entirely. This
project flattens an arc to chords at a default tolerance of 10 microns
(`DEFAULT_TOLERANCE`, `crates/cypcb-world/src/arc.rs:62`), and the step that
follows from a tolerance is documented with it: `step = 2 * acos(1 - tolerance
/ radius)` (`crates/cypcb-world/src/arc.rs:20-23`). The interior angle between
two consecutive chords is `180 degrees - step`, so it stays at or above 90
degrees exactly when `radius >= tolerance * (2 + sqrt 2)`, which is
`3.414 * tolerance`. At the default tolerance that is a radius of 0.0341 mm -
below it the flattening itself draws the wedge this rule forbids.

**3. The 45 degree taboo is a manufacturing rule, not a signal-integrity one,
and the numbers are not close.** The etching mechanism is the documented
reason: etchant sits in an acute wedge longer than on open copper and undercuts
the trace (fabricator articles on acid traps, read 2026-09-11). The
signal-integrity reason does not survive measurement: for an 8 mil wide 50 ohm
microstrip in FR-4, a right-angle bend adds about 0.012 pF of excess
capacitance and about 1 ps of delay, and at a 100 ps rise time the reflection
off that discontinuity is 0.003 of the incoming step - right-angle bends are
fine to 2 Gbps and corners only begin to matter for 10 Gbps serial links
(Howard Johnson, *Who's Afraid of the Big Bad Bend?*, sigcon.com, read
2026-09-11). R-10 therefore belongs with the manufacturing rules and not in a
signal-integrity section.

A widely cited article states that etching is now done with alkaline rather
than acid, so acid traps are no longer a problem (Altium on routing-angle
myths, read 2026-09-11) `[P]`. On this project's boards the reason to cut a 45
degree junction is therefore not that etchant still pools in it - it is that
such a junction is a symptom of the search doubling back.

**4. What documented tools do at a corner.** KiCad 8.0, read 2026-09-11: the
router offers sharp and rounded corner modes, switched with Ctrl+/; Shove and
Walk Around modes always emit horizontal, vertical and 45 degree segments, and
free angles are available only in Highlight Collisions mode. FreeRouting, its
routing-options page, read 2026-09-11: a "45 Grad" setting restricts
interactive angles to multiples of 45 degrees and a "none" setting removes the
restriction; a pull-tight region from 0, which switches the algorithm off, to
999, which leaves it unrestricted; and an optional postroute pass that reduces
via count and cumulative trace length. Both constrain the direction a segment
may run. Neither documents a bound on the angle between two segments, which is
the same finding as part 1 seen from the tool side.

### R-11 Acceptance classes `[R]`

A violation is weighed against the acceptance class the board declares, and the
score publishes a tuple rather than one price per violation.

**What the classes are.** IPC-6012 states performance requirements for rigid
boards in three classes, with IPC-A-600 as the visual acceptance companion that
says what each condition looks like. The worked example where the classes
visibly differ is the annular ring: Class 2 permits breakout of up to 90
degrees of the land's circumference on internal layers, Class 3 permits none,
and the Class 3 minima are 0.050 mm on external layers measured from the inner
diameter of the finished plated hole, and 0.0248 mm on internal layers measured
from the drill diameter. Sources: vendor reproductions of the standard - a
fabricator's land-size article and a class 2 against class 3 comparison, both
read 2026-09-11. The clause text itself was not accessible, so every figure
here is a reproduction rather than the standard's own words. The second example
usually quoted, conductor width and spacing per class, is not carried here:
searching on 2026-09-11 returned vendor pages asserting that Class 3 requires
larger widths and spacing and none that gives the figure.

**Which class each rule in this canon belongs to.** One of eleven is graded by
class at all:

| rule | where it lives |
|---|---|
| R-01 width against current | design standard (IPC-2221), class-independent |
| R-02 spacing against voltage | design standard (IPC-2221 table 6-1), class-independent |
| R-03 acute angles | DFM guidance only, no acceptance standard |
| R-04 stub length | a design author's rule of thumb, no standard |
| R-05 return path | EMC practice, no standard |
| R-06 reporting per kind | this project's own rule |
| R-07 annular ring and hole spacing | **graded by class** - Class 2 permits breakout, Class 3 does not |
| R-08 trace entry into a land | DFM guidance; teardrops are discussed against Class 3 and are not themselves an acceptance criterion |
| R-09 thermal relief | design guidance (IPC-2221, IPC-7093 for bottom-terminated parts), not class-graded |
| R-10 mitring | DFM guidance only, like R-03 |
| R-11 this rule | this project's own reading of the standards above |

Five of the eleven exist only in DFM guidance or practice. That is not a defect
in the canon - it is the reason this section exists, because a board can pass
every acceptance criterion and still be refused at DFM review, and the two
facts have to be reported separately rather than added together.

**Nothing published ranks defect kinds against each other.** IPC-A-600 grades
each feature on a three-step ladder - acceptable, process indicator, defect -
where a process indicator does not affect form, fit or function and is not
grounds for rejection, and a defect is nonconforming and requires disposition.
The ladder is per feature and per class: the same condition can be acceptable
in Class 1, a process indicator in Class 2 and a defect in Class 3 (vendor
explainers of IPC-A-600 and IPC-A-610, read 2026-09-11) `[R]`. What no source
read here provides is a rate of exchange between kinds - nothing says what a
spacing under minimum is worth against an angle a fabricator dislikes, because
acceptance is decided feature by feature and not by a total. It follows that
the weighted composite in `crates/cypcb-autoroute/src/scoring.rs` is this
project's own invention and has to be defended as a decision rather than cited.

**The weighting this project adopts `[D]`: an order, not a sum.** Four tiers,
compared one after another, never added:

1. **A connection not made.** No class permits an open circuit, and no quantity
   of anything else offsets one.
2. **Copper touching copper**, measured at 0.00 mm. A board with a short does
   not work; a board with a gap under minimum is a yield risk a fabricator may
   still build.
3. **Class-graded features under minimum** - annular ring, spacing, hole
   spacing. These are what the acceptance ladder is for: report them per kind,
   against the class the board declares, and let the class decide whether each
   is a defect or a process indicator. Counted as **contacts, not rows**: one
   contact along a parallel run produces a dozen clearance rows, so a tier
   ranked on rows outweighs itself by accident. `clearance_contacts`
   (`crates/cypcb-drc/src/violation.rs:156`) already computes the contact
   count and the composite does not read it.
4. **Findings with no acceptance standard behind them** - acute angles, trace
   entry, mitring. Real, worth fixing, and never allowed to outweigh tier 3.

Condition: of two routed boards, the one with fewer tier-1 findings ranks
better whatever the other tiers say; ties fall to tier 2, then tier 3, then
tier 4. Precedent in this repository for the form, not for the tiers:
`generate_variants` already ranks complete boards first, then by shorts, then
by composite (`crates/cypcb-autoroute/src/variant.rs:496-510`).

**In this repo:** the score prices every violation at 1000 regardless of kind
(`compute_composite`, `crates/cypcb-autoroute/src/scoring.rs:583`), so tiers 3
and 4 are indistinguishable inside it, and tier 1 is absent from the score
altogether. The board that makes this concrete is `shift_driver` under
`stop_at_own_copper`: its clearance reports go 7 to 27 while its acute-angle
count falls 12 to 5. Under this rule that is a tier-3 regression of 20 bought
with a tier-4 improvement of 7, which is a bad trade stated in one line; under
a flat price per violation the same board reads as 19 to 32 and says nothing
about which kind moved.

## What this project already measures

### Board score

`crates/cypcb-autoroute/src/scoring.rs` returns `RoutingScore` with nine
fields. Six of them enter the composite.

| field | unit | computed in | in composite |
|---|---|---|---|
| `total_length` | nm | `TraceData::total_length`, `scoring.rs:239` | yes, divided by board diagonal |
| `via_count` | count | `scoring.rs:148` | yes, weight 1 |
| `drc_violations` | violation rows | `scoring.rs:155` | yes, x1000 |
| `clearance_contacts` | feature pairs | `scoring.rs:161` | no |
| `shorts` | violations measured at 0.00 mm | `scoring.rs:160` | no |
| `smoothness` | 0.0 to 1.0 | `compute_smoothness`, `scoring.rs:281` | yes, `(1-s) * 100` |
| `crossings` | segment intersections | `compute_crossings`, `scoring.rs:387` | yes, x500 |
| `layer_balance` | 0.0 to 1.0 | `compute_layer_balance`, `scoring.rs:494` | yes, `(1-b) * 50` |
| `composite` | dimensionless, lower is better | `compute_composite`, `scoring.rs:565` | - |

The bend penalty is the distance from the nearest multiple of 45 degrees over
22.5 degrees (`angle_penalty`, `scoring.rs:253`). Length is normalised by the
board diagonal, which falls back to 100 mm when no board is set
(`board_diagonal_nm`, `scoring.rs:542`).

Three things about this score are already measured and should not be
re-discovered:

- A crossing is charged twice. Two traces of different nets meeting at a point
  give `crossings` 1 and `shorts` 1, so the composite pays 500 and 1000 for one
  place - `crates/cypcb-autoroute/tests/a_crossing_is_charged_twice.rs`.
- `layer_balance` divides by the board's copper layers, not by the layers the
  route happened to use, so a single-layer route on a two-layer board scores 0 -
  `crates/cypcb-autoroute/tests/layer_balance_means_what_it_says.rs`.
- `smoothness` looks for corners between trace entities, because `apply_routes`
  emits one entity per segment - `crates/cypcb-autoroute/tests/smoothness_measures_the_corners.rs`.

### Rule registry

`run_drc` (`crates/cypcb-drc/src/lib.rs:128-200`) runs more than thirty rules.
The ones that back a canon rule are `TraceCurrentRule` (R-01), `AnnularRingRule`,
`PadLandRule`, `HoleToHoleRule`, `ViaDiameterRule`, `ViaDrillRule`,
`DrillAspectRatioRule` (R-07). `ImpedanceRule`, `DiffPairSkewRule` and
`BendRadiusRule` measure related properties and report "not checked" rather
than passing silently when the design does not describe the case.

### Ranking and gate

`generate_variants` sorts complete boards first, then by `shorts`, then by
`composite` (`crates/cypcb-autoroute/src/variant.rs:496-510`). The CI gate in
`crates/cypcb-autoroute/tests/benchmark_validation.rs` asserts 0 unrouted
connections, at least 70.0 mm of copper, composite at most 2100.0, at most 2
`drc_violations`, and smoothness at least 0.95.

## What nothing measures

### Rules that DRC checks and the ranking cannot see

Every violation weighs 1000 in the composite regardless of kind, so a trace
that will cook ranks level with a trace slightly under the fab's minimum. The
fix is mechanical: `DrcViolation` already carries its kind, so a count per kind
is built in the same place `shorts` is built today (`scoring.rs:160`).

### Properties nothing in the workspace computes

Checked by grep over `crates/*/src` on 2026-09-11: no hits for "return path",
"return current", "loop area", "split plane", "antipad", "crosstalk" or
"parallel run".

1. Return path coverage - for each segment, ask the spatial index whether
   continuous reference copper lies under its footprint on the adjacent layer,
   and report the share of length that has none.
2. Plane split crossings - intersect the segment footprint with the edges of
   the reference pour and count the crossings.
3. Loop area - once coverage exists, take the area between the trace axis and
   the nearest continuous return copper.
4. Acute angles - `shared_corner` (`scoring.rs:362`) already yields the angle
   between the two arms of a junction; count the junctions below 90 degrees.
5. Stub length - see R-04; needs the connectivity graph described below.
6. Parallel run length - for segment pairs on one layer, on different nets,
   whose directions differ by less than 10 degrees, sum the projected length
   within a corridor of N times the clearance.
7. Vias per net - `via_count` is a board total; group vias by `net_id` and
   publish the maximum and the distribution.
8. Direction symmetry - route the same pad pair A to B and B to A and compare
   cost, copper length, via count, and the intersection over union of the two
   cell sets.

### Constants a fab preset promises and nothing checks

Five fields in `crates/cypcb-rules/src/constraints.rs` have no reader anywhere
outside their own crate: `min_acid_trap`, `max_stub_length`,
`thermal_relief_spokes`, `max_vias_per_high_speed_net`, `diff_pair_gap`.
Control for the method: `min_hole_to_hole` and
`min_annular_ring` do show readers under the same grep, so the silence is real
and not an artefact.

`crates/cypcb-rules/src/clearance_table.rs` implements the R-02 table as
`voltage_clearance(voltage_v, coating)`. Outside its own file the module is
named twice in the whole repository: once in a doc line and once as `pub mod`.

The pattern is one pattern, not five accidents: a fab preset states a number,
nothing enforces it, and `cypcb check` reports a clean board that the fab in
question will not build.

## Blocked on the model

Two of the nine rules cannot be enforced without a change to the data model.
The other seven are waiting on code.

**R-02, working voltage.** Nets have no voltage field: "voltage" does not
appear in `crates/cypcb-parser/src/ast.rs` or
`crates/cypcb-world/src/components/electrical.rs`. The table is written and
there is nothing to ask it with. `ClearanceRule` holds a flat `min_clearance`
instead.

**R-04, stub length.** Two gaps. First, copper has no connectivity graph. What
exists is geometry: `Trace { segments, width, layer, net_id }` and
`Via { position, drill, outer_diameter, start_layer, end_layer, net_id }`
(`crates/cypcb-world/src/components/trace.rs:727-742`). The ratsnest in the
renderer is a star over pins, not over copper, and `UnroutedPinRule` is a
geometric touch test with no graph behind it. A stub needs connected components
per net: nodes at segment ends, pads and vias; an edge per segment; a via
joining nodes across `start_layer` to `end_layer`; pads joined by the same
touch test the rule already uses. A leaf is then a degree-1 node that is
neither pad nor via, and a stub is the path from a leaf to the nearest node of
degree 3 or more. Second, no net declares a signal speed: no hits for
`bit_rate`, `bitrate`, `data_rate` or `rise_time` anywhere in `crates/`, so the
divisor in `0.3 / BR` has no source. Until both exist, the useful thing to
publish is the longest branch per net as a bare number, with no threshold.

The same connectivity graph would also give R-05 its per-net restriction and
the per-net via count in one pass.

## Practice without a number

Reported practice, carried here because it names a failure this canon otherwise
has no rule for. Neither statement comes with a measurement, and neither should
be turned into a threshold without one.

**P-01 One bonding point per cable screen `[D]`.** In a console with an
integral patchbay, the screens of all audio cables are bonded only at the
patchbay, where the jack grounds are bussed together and that buss alone
returns to the common 0 V node. Condition, qualitative: one screen bonding
point per path, not two. Source: a private archive of DIY discussions, read
2026-09-11.

**P-02 Pin 1 to chassis, not to signal ground `[D]`.** Connecting connector pin
1 to audio ground rather than to the chassis is described as a period mistake,
found when tracing older equipment. Condition, qualitative: pin 1 lands on the
chassis. Source: a private archive of DIY discussions, read 2026-09-11.

## Verification

```bash
# Which metrics the score carries, and where each is computed
grep -n "pub [a-z_]*:" crates/cypcb-autoroute/src/scoring.rs

# Which rules the registry runs
sed -n '128,200p' crates/cypcb-drc/src/lib.rs

# Constants no code reads: expect matches only under crates/cypcb-rules
for f in min_acid_trap max_stub_length thermal_relief_spokes \
         max_vias_per_high_speed_net diff_pair_gap; do
  printf '%s: ' "$f"
  grep -rln "$f" --include=*.rs crates/ | grep -v '^crates/cypcb-rules/' | wc -l
done

# Control for the grep above: these two do have readers
grep -rln "min_hole_to_hole\|min_annular_ring" --include=*.rs crates/ \
  | grep -v '^crates/cypcb-rules/'

# The voltage table and its callers
grep -rn "voltage_clearance\|clearance_table" --include=*.rs crates/

# Two sources of truth for thermal relief
grep -n "thermal_gap\|spoke_width" crates/cypcb-core/src/pour.rs
grep -rn "thermal_relief" crates/cypcb-rules/src/presets/

# The gate these numbers are held to
cargo test -p cypcb-autoroute --test benchmark_validation

# R-10: the wedge count and the room each one has (R-10 is a snapshot of the
# router's defaults, not a constant)
cargo test --release -p cypcb-autoroute \
  --test can_a_wedge_be_cut_where_it_stands -- --nocapture

# R-10: the arc tolerance the mitring alternative is bounded by
sed -n '58,63p' crates/cypcb-world/src/arc.rs

# R-10: chamfer_corners refuses anything that is not a 90 degree bend
sed -n '376,390p' crates/cypcb-autoroute/src/smoother.rs

# R-11: the composite prices violation rows at 1000 and never reads the
# contact count
sed -n '565,590p' crates/cypcb-autoroute/src/scoring.rs

# R-11: the tiered ordering that already exists for variants
sed -n '496,512p' crates/cypcb-autoroute/src/variant.rs
```

Last verified: 2026-09-11, including R-10 and R-11. Web sources were read on
2026-09-11; every repository claim in those two rules was read against the
working tree on the same day, and the four file references they carry -
`DEFAULT_TOLERANCE`, `is_90_bend`, `compute_composite` and the variant sort -
were each opened rather than grepped for by name.
