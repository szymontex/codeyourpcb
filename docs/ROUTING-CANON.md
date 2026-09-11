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

*Applies when:* a net declares a `current`. Silent on every net that does not, which today is every net on every fixture.

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

*Applies when:* a net declares a working voltage. **Nothing can declare one**, so this rule is silent on every board - see "Blocked on the model".

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

*Applies when:* always. Two trace segments meet at a junction; no declaration needed.

No two runs of copper on one net and one layer meet at an internal angle below
90 degrees. The angle is the one between the two arms of the junction, so a
straight run is 180 degrees and a 45 degree turn leaves 135. Where copper
meets a land the wedge is measured the other way round - between the edge of
the trace and the edge of the land, where square-on is 90 - and that case is
R-08. The same wedge therefore carries two different numbers in the two
rules, and both are right.

**Two cases sit between this rule and R-08 and neither owns them.** A trace
that crosses its own land and carries on has no junction here and no entry
there - R-08 defines an entry as a segment *ending* inside the land's copper
- yet its edges cut the land's edge twice and each crossing is the same
wedge. Unowned, and named here so it is not mistaken for covered. And a
junction whose vertex lies inside a land's copper is reported by this rule
today although the copper there is merged and has no wedge: `AcuteAngleRule`
queries `(Entity, &Trace, Option<&Curve>)` and has no pad geometry at all, so
that is a known false positive rather than a finding.

Etchant sits in the wedge of an acute corner longer than it sits on open
copper, and undercuts the trace from inside the corner. The accepted fix is two
45 degree corners, or a radius, in place of one acute corner.

Condition: the count of junctions with an internal angle below 90 degrees is 0.

Source: nextpcb and pcbsync acid trap articles, read 2026-09-11. The threshold
is an angle, not a dimension; no source gives a length.

In this repo: **enforced.** `AcuteAngleRule` is in the registry
(`crates/cypcb-drc/src/lib.rs:196`) and reports `ViolationKind::AcidTrap`;
every wedge count in this document came out of running it. What has no
reader is the constant: `min_acid_trap`
(`crates/cypcb-rules/src/constraints.rs:167`) is named nowhere outside its
own crate, so the rule enforces a threshold of its own rather than the fab
table's. This paragraph read "not enforced" until 2026-09-11, which made
the one rule this project has been measuring with deny its own existence.

The cut that removes such a junction, and the floor below which cutting is
cosmetic, are R-10.

### R-04 Stub length `[O]`

*Applies when:* a net declares a signal speed **and** the copper has a connectivity graph. Neither exists, so this rule is silent on every board.

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

*Applies when:* the board has a pour on a copper layer adjacent to the trace. A board with no pour has no reference copper and this rule says nothing. R-13 adds the impedance gate; this rule states none, and the two are kept apart on purpose.

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

In this repo: nothing measures either condition. See R-13 for why no
source states a permitted fraction, what bridges a crossing that cannot be
avoided, and what a two-layer board changes.

### R-06 Violations are reported per rule, not as one total `[O]`

*Applies when:* always, because it is about the shape of the output rather than the board.

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

*Applies when:* the board has holes - a via, a through-hole pad, a mounting hole.

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

*Applies when:* a trace ends on a pad. Every routed board.

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

*Applies when:* a pad sits inside a pour on its own net. A board with no pour says nothing here.

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
no relief on a net declaring more than 3 A. See R-15 for what the relief
costs in service, what the via array under a bottom-terminated part actually
requires, and the one condition that spans two pads rather than one.

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

*Applies when:* R-03 reported an acute junction. This rule is what to do about one, not how to find it.

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

*Applies when:* the board is graded. The full four-tier form needs a declared acceptance class; for a board graded by a house table see part 4 below.

**A board that declares no class.** Found by walking this canon against a real
fixture: a board graded by a house table declares no acceptance class, and tiers
1, 2 and 4 work without one while tier 3 has no ladder to climb. The rule is
therefore: **tier 3 reads the house table's own figures when no class is
declared**, and the report says which of the two it used. It does not report
"not checked" - a board graded against a fabricator's numbers is graded, and the
tier that prices how far under those numbers a feature sits works the same way
either side. Only the three `IpcClass` presets
(`crates/cypcb-rules/src/presets/mod.rs:58`, `:60`, `:62`) declare a class; every
house preset does not.

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

*The field R-11 would need does not exist:* `DrcViolation` has no severity, so
the tiers below have nowhere to live in a row of output. See R-18.

### R-12 Rip-up and reroute `[O]`

*Applies when:* never, to a board. This is a rule about the router's loop, and it is here rather than in the routing document for now - see R-16's entry conditions, which it does not meet.

*And it should not become a check.* Asked which of the checkable rules to write
first, the answer for this one was to write none of it: its three conditions -
a constant net order, the shape of the cost against the paper's equation, the
subset re-routed rather than everything - are three separate measurements on
code, two of them already taken, and not one check on a board. Implementing it
would add a registry entry that can never fire on any board and say nothing the
three measurements do not already say.

A negotiated-congestion router tears at the granularity its data structure can
restore, and every departure from the published algorithm is named as a
departure and measured.

**What PathFinder specifies.** Source: L. McMurchie and C. Ebeling, *PathFinder:
A Negotiation-Based Performance-Driven Router for FPGAs*, ACM/SIGDA FPGA 1995,
read in full 2026-09-11.

*Granularity.* "Only one net is ripped up at a time, but every net is ripped up
and rerouted on every iteration, even if the net does not pass through a
congested area." The reason is negotiation rather than economy: "In this way
nets passing through uncongested areas can be diverted to make room for other
nets currently in congested regions." The paper also fixes the order: "Nets are
ripped up and rerouted in the same order every [iteration]."

*The cost.* `c_n = (b_n + h_n) * p_n` (equation 1): `b_n` the base cost of node
`n`, set in the paper to its intrinsic delay `d_n`; `h_n` "related to the
history of congestion on `n` during previous iterations"; `p_n` "related to the
number of other signals presently using `n`". Update rule, in the paper's
words: "Each iteration that node C is shared, `h_n` is increased slightly", and
"The effect of `h_n` is to permanently increase the cost of using congested
nodes so that routes through other nodes are attempted."

*The loop, as written.* Rip up routing tree `RT_i` [3]; `RT_i <- s_i` [4]; loop
until all sinks are found [5]; "Initialize priority queue PQ to `RT_i` at cost
0" [6]; on finding a sink, backtrace and add every node of the path to `RT_i`
[13]-[16].

*Multi-terminal nets.* "this updated `RT_i` is the source for the search for the
next sink (step 6). In this way, all locations on routes to previously-found
sinks are used as potential sources for routes to subsequent sinks. This is
similar to Prim's algorithm for determining a minimum spanning tree over an
undirected graph. This algorithm for constructing the routing tree is identical
to an algorithm suggested by [Takahashi80]."

*The order sinks are visited is not a requirement.* In the base algorithm the
next sink is whichever the wave reaches first - "A breadth-first search for the
closest sink `t_ij` is performed" - so the order is emergent, not chosen. The
timing variant does choose it: sinks are routed in decreasing slack-ratio order
and the queue is seeded at `A_ij * d_j`, which the paper introduces to hold the
critical path rather than to make routing succeed. A project ordering its pads
by a greedy nearest-neighbour spanning tree on Manhattan distance is therefore
choosing a performance heuristic, not violating the algorithm - but it is also
precomputing an order that the published form derives from the search itself.

*Timing variant, for completeness.* `C_n = A_ij * d_n + (1 - A_ij) * c_n`
(equation 2), slack ratio `A_ij = D_ij / D_max`. Theorem 1: if `h_n <= d_n` for
all nodes, no routed path exceeds `D_max`.

**What VPR does differently.** Source: Verilog-to-Routing documentation,
command-line options page, read 2026-09-11 - the page carries no version
string, which is recorded here because it limits what can be claimed from it.
VPR exposes `--min_incremental_reroute_fanout`, default 16: "Incrementally
re-route nets with fanout above the specified threshold. This attempts to reuse
the legal (i.e. non-congested) parts of the routing tree for high fanout nets,
with the aim of reducing router execution time." Partial tearing therefore
exists in practice and its unit is a pruned branch of the routing tree.

**The claim this project makes about VPR is not supported by what was read.**
`crates/cypcb-autoroute/src/pathfinder_v2.rs` describes re-routing only the nets
that pass through an overused cell as the VPR optimisation. The primary paper
says the opposite for PathFinder, and the VTR documentation read here does not
state the narrower rule either; ripping up only the illegal routes is published,
but for a different router - a just-in-time FPGA routing paper describes
ripping up only illegal routes and then adjusting costs across the resource
graph (read 2026-09-11). Condition: either that comment gains a citation naming
the router it came from, or it drops the words "the VPR optimisation" and
stands as this project's own departure with its own measurement, tagged `[D]`.

**Decomposition: the multi-sink wave is the standard and pad-to-pad is the
deviation.** PathFinder seeds the frontier with the whole partial tree at cost 0
(step [6]); this project seeds it with one pad and searches to another pad.
Ending a connection on the net's own copper fixes the far end of that search and
leaves the near end where it was, which is half of the published form. Checkable
condition for full adoption: the first expansion frontier of connection `k`
contains every cell the net already owns, not one cell. This project's own
measurement says why the half-step is not enough - with the end test removed and
only the start-and-end swap left, `led_blink` goes from zero shorts to one, so
the asymmetry the swap introduces is a defect of having a start pad to choose at
all, which the seeded frontier does not have.

**Three departures this project has, stated as conditions.**

1. *Net order: met.* `order_nets`
   (`crates/cypcb-autoroute/src/orchestrator.rs:192-216`) sorts net indices with
   a stable `sort_by` on two keys - power nets last, then Manhattan span
   ascending - and `pathfinder_loop` receives that `Vec<usize>` once and reuses
   the same slice every iteration. No map iteration takes part. Condition: the
   order a run starts with is the order every iteration uses.
2. *Every net every iteration: not met, and this is the departure.*
   `nets_needing_reroute` (`crates/cypcb-autoroute/src/pathfinder_v2.rs:1399`,
   called at `:596`) keeps only the nets touching an overused cell, so a subset
   is re-routed each iteration where the paper re-routes all of them and gives
   the reason - a net in clear space can be diverted to make room for one that
   is stuck. Condition: the departure is measured against the paper's form on
   the benchmark set, or it is named in the code as this project's own choice
   rather than as somebody else's optimisation.
3. *Cost shape: multiplicative inside, additive outside.* `congestion_cost`
   (`crates/cypcb-autoroute/src/congestion.rs:214-228`) returns
   `(1.0 + history) * (1.0 + overuse) - 1.0 + ring_penalty * ring`, which is the
   shape of equation (1) with the base normalised to 1: history multiplies
   present overuse. But the value enters the total additively - the successor
   cost in `find_path_congestion_augmented` is base plus congestion plus
   crowding plus pad crossing plus stacking - so the node's own base cost is
   never scaled by present congestion, which equation (1) does scale. Condition:
   a sweep that compares the additive form against `(b + h) * p` on the
   benchmark set, or the difference stands recorded here and unmeasured.

**The dependency question, and why it does not appear in the literature.** In
PathFinder it cannot arise: step [3] erases the entire routing tree of the net
before its sinks are re-routed, so no connection outlives the one it grew from.
In VPR's incremental reroute it is answered structurally rather than by a rule -
a net's routing is a tree rooted at the source, so pruning an illegal branch
leaves every surviving node with its path to the root. Nothing read here states
a rule for the case where connection `k` ends on connection `j`'s copper,
because in both published designs that relation is the tree edge itself.
Searches on 2026-09-11 that returned no such rule: rip-up of a connection
another connection terminates on; partial rip-up semantics for a net routed as
independent two-pin connections.

**In this repo:** tearing is net-wide -
`crates/cypcb-autoroute/src/pathfinder_v2.rs:610-623` clears the net's cells,
rings and holes, drops `routed_paths` for that net and rebuilds its spanning
tree - and that is the only tearing with defined semantics here, because
`routed_paths` is `HashMap<u32, Vec<Vec<GridNode>>>`
(`crates/cypcb-autoroute/src/pathfinder_v2.rs:288`): a flat list of paths with
no parent relation to prune. The prerequisite for partial tearing is not a
dependency field but the rooted tree the published routers keep.

### R-13 Return path, the threshold that does not exist `[P]`

*Applies when:* as R-05. A pour on the adjacent layer, and a net that declares a controlled impedance.

R-05 states the rule and its two conditions. This section answers three
questions it leaves open: what fraction of a trace may run without reference
copper, what to do where a gap has to be crossed, and how much of either this
project could measure.

**There is no published fraction, and the reason is that the published
condition is binary.** Searching on 2026-09-11 for a percentage of trace length
permitted without reference copper beneath it - three query families, on plane
coverage thresholds, on percent-of-length rules and on high-speed design-rule
lists - returned no threshold of that shape. What the sources state instead is
continuity over the whole length and an absolute prohibition on crossing a gap:
route a high-speed signal adjacent to a solid reference, never across a split,
because the return current has to detour around the gap and the detour is the
loop that radiates (high-speed design guides from several vendors, read
2026-09-11). R-05's two conditions are therefore counts and not ratios, and
that is not an omission in them. The uncovered share belongs in the report as a
diagnostic - it says how badly a board fails, which a count does not - but it
carries no threshold, because no source read here supplies one.

**Crossing a split, and what to do instead `[P]`.** A signal that must cross a
gap in its reference is bridged by a stitching capacitor at the crossing, so
the return has a path across the gap rather than around it. Published numbers:
values of 10 nF to 100 nF, 0.1 uF being the value used in the measured study
below; placement within 0.1 in (2.54 mm) of the trace; spacing between bridges
no more than a twentieth of a wavelength at the highest frequency of concern
(high-speed routing guides, read 2026-09-11). The measured case: an article on
stitching capacitors across an imperfect reference, dated 2020-02-04 and read
2026-09-11, reports crosstalk alleviation of as much as 10 dB, far-end
crosstalk noise falling from 135 mVpp with no capacitor to 72 mVpp with two,
and - the number that decides part selection - an ESL of 0.5 nH intensifying
far-end crosstalk by 1.1 dB near 500 MHz.

Condition: for each crossing, a component bridging the two pour regions with a
pad on each, within 2.54 mm of the crossing point. A crossing without one is
the fault; a crossing with one is a reported bridge.

**Loop area has no published bound `[R]`.** What is published is a
proportionality: the field radiated by a differential-mode current loop rises
with the loop area and with the square of the frequency, so a larger loop
radiates more at the same current (Henry W. Ott, *Electromagnetic Compatibility
Engineering*, as reproduced in a vendor EMC tutorial, read 2026-09-11 - the
equation's constant was not accessible in what was read, so no figure is
carried here). A proportionality ranks two layouts; it does not pass or fail
one. Loop area therefore enters this canon as a comparative number and never as
a threshold, and any claim about an emission level in volts per metre needs a
field solver and is out of scope for this project.

**What is measurable from what this project already holds.**

1. *Coverage: measurable.* Pours are zones of kind `CopperPour` carrying a net
   and a layer mask, the reference layer for a trace is the one `ImpedanceRule`
   already derives from the stackup through `CopperEnvironment`
   (`crates/cypcb-drc/src/rules/impedance.rs:30`), and `query_region_on_layers`
   already answers what copper lies in a region - it is what `compute_crossings`
   uses (`crates/cypcb-autoroute/src/scoring.rs:423`). Measure the share of each
   segment's footprint that projects onto reference copper on the adjacent
   copper layer.
2. *Split crossings: measurable.* Intersect the segment footprint with the
   boundary of the reference copper and count the crossings. One modelling limit
   belongs here rather than in a surprise later: a zone in this model is a
   rectangle (`bounds: Rect`, `crates/cypcb-world/src/components/zone.rs:64`),
   so a gap here is the absence of pour or a cut made by other copper, not an
   arbitrary slot drawn inside a polygon.
3. *Loop area: an estimate, and mostly not a routing variable at all.* Above the
   crossover the return runs in a band directly beneath the trace, so where the
   reference is continuous the loop area is set by the dielectric separation in
   the stackup and not by where the router put the copper: area is about length
   times layer separation whatever the route. It becomes a routing variable
   exactly where coverage fails, and there the estimate is the uncovered length
   times the separation plus the detour the return takes around the gap. Report
   it for uncovered spans only, labelled an estimate. This is why R-05 gives the
   router two numbers to chase and not three.
4. *Anything that needs a field solver is not a rule this project can carry.*
   That is a finding rather than a gap: coverage and crossings are geometry and
   this project holds the geometry; emission levels are physics this project
   does not model.

**The gate this rule needs is already in the model.** R-05 is stated against
nets that declare controlled impedance, and `impedance_ohms_x100` in the
per-net constraints is exactly that declaration -
`crates/cypcb-drc/src/rules/impedance.rs:92` and `:216` read it. Unlike R-04,
which waits on a signal speed the model does not hold, R-13 needs no new field
and does not belong under "Blocked on the model". For every other net the two
conditions are a report and not a fault.

**On a two-layer board the strict form does not apply, and that is every
fixture in this project's benchmark set.** Three consequences, stated so that
nobody reads a four-layer rule onto a two-layer board:

- The reference is not a plane but a pour, and the pour is cut by the traces
  routed on that same layer. Coverage has to be measured against the filled
  geometry - the pieces and spokes `fill_zone` returns
  (`crates/cypcb-world/src/copper.rs:47`) - and never against the zone's
  declared rectangle, which covers copper that is not there.
- Every trace on the opposite layer is itself a gap in the reference of the
  trace above it. A two-layer board with any routing on the reference layer
  therefore has crossings by construction, and a rule that fails the board for
  having them fails every board this project ships.
- In its strict form, zero crossings and zero uncovered length, R-13 is a
  four-layer rule.

**In this repo:** nothing measures either condition - the same finding R-05
records. The three pieces needed - pour geometry, stackup-derived reference
layer, spatial index - are all present and none is called for this purpose.

### R-14 Via stitching `[P]`

*Applies when:* the board has two pours of one net on different layers, or a via that changes which pour is a signal's reference. Where the design declares no frequency the rule screens at a stated stand-in of 1 GHz and says so in every row - see part 5.

Two pours of one net on different layers are tied together by a field of vias,
and a signal via that changes reference has a return via beside it. R-05 states
why the return path matters and R-13 states how coverage and splits are
measured; the conditions here are distances.

**1. Spacing of a stitching field - the sources do not agree, and both forms
are carried.** The published form is a fraction of a wavelength at the highest
frequency of concern, with the wavelength taken inside the board,
`lambda = c / (f * sqrt(effective dielectric constant))`, and the guidance to
assume 1 GHz where the design does not say (via-stitching guides, read
2026-09-11).

- `lambda / 20` is the figure most often given, on the argument that a gap in
  the ground network is then electrically small at the frequency of concern.
- `lambda / 10` is given for high-frequency work as the practical compromise,
  explicitly at the cost of some EMI performance.

Worked from the sources' own formula at an effective dielectric constant of 4:
at 1 GHz `lambda` is 150 mm, so `lambda / 20` is 7.5 mm and `lambda / 10` is
15 mm; at 10 GHz the same arithmetic gives 0.75 mm and 1.5 mm, and the sources
name those two figures themselves.

Condition: a pitch is checked against the frequency the design declares, not
against a house default. Where the design declares no frequency, 1 GHz is the
stated stand-in and the check is a report rather than a failure.

**2. A return via beside a signal via - the published distances differ by an
order of magnitude, so all three are given.** When a signal changes layer its
return has to change reference with it, and the published fix is a via on the
reference net placed next to the signal via (high-speed routing guides, read
2026-09-11).

- Within 50 mil (1.27 mm) of the signal via for designs up to 5 GHz.
- Within 20 mil (0.51 mm) above 10 GHz, and the same figure for critical
  signals generally.
- A device vendor's interface design guide asks for ground stitching vias
  placed symmetrically within 200 mil (5.08 mm) centre to centre of the signal
  transition vias.

The three do not reconcile and this canon does not reconcile them: the ratio
between the loosest and the tightest is ten to one. Condition: the distance
from each signal via to the nearest reference via spanning the same layer pair
is measured and reported per via; a threshold is applied only where the design
declares a frequency, and then it is the figure for that frequency with its
source named.

**3. What this project holds, and the finding that changes the question.** Both
conditions are geometry this project already carries, and the first is further
along than expected.

*What exists.* A pour declares its stitch pitch in the language - `stitch
<pitch>` inside a zone (`crates/cypcb-parser/src/parser.rs:1416`,
`crates/cypcb-parser/src/ast.rs:1136`) - and it is carried on the zone's entity
as `StitchPitch` (`crates/cypcb-world/src/components/zone.rs:87`). A generator
places the field: `stitching_vias` (`crates/cypcb-world/src/stitch.rs:56`)
walks the pour's rectangle on a grid at `StitchSpec::pitch`, starting half a
pitch inside the edge so the field is symmetric, and keeps a point only where
the pour is present on both sides and nothing foreign is in the way on either
outer layer. The via itself is fixed at a 0.3 mm hole in a 0.6 mm pad with
0.3 mm clearance (`StitchSpec::at`, `crates/cypcb-world/src/stitch.rs:32-42`).
Generated vias are marked `Stitched` so the writer does not emit them back as
hand-placed copper (`crates/cypcb-world/src/dsl.rs:266`).

*The finding.* Because the generator drops every grid point that is blocked,
**the declared pitch is not the spacing the board gets.** Where routing is
dense the field thins out, and the number that matters - the largest gap the
return current has to cross - is a property of the placed field and not of the
declaration. Checking the declaration against `lambda / 20` therefore checks
something that is not on the board. Condition: measure the maximum
nearest-neighbour distance over a pour's placed stitching vias and compare
that. Nothing computes it today. See "Declared is not measured".

*The second condition needs nothing new either.* A via carries its net, its
position and the layers it spans - `Via { position, drill, outer_diameter,
start_layer, end_layer, net_id, locked }`
(`crates/cypcb-world/src/components/trace.rs:727-742`) - so the distance from a
signal via to the nearest reference-net via spanning the same layer pair is a
query over data already in the world. Nothing computes it today, and no rule in
`crates/cypcb-drc` asks for it: `grep -rln stitch crates/cypcb-drc/src/`
returns nothing at all.

**4. Unlike R-13, this is not a four-layer rule.** R-13's strict form fails on
two layers because the reference is a pour cut by the traces on its own layer.
R-14 is the opposite case, and this project's own code says why: a plane on a
two-layer board is two planes, one per side, and what ties them into one is a
field of vias through the pour (`crates/cypcb-world/src/stitch.rs:1-12`). The
field condition applies more strongly rather than less, because the two halves
of the reference are only as connected as the vias make them; and the
return-via condition applies unchanged, because a signal going top to bottom
changes which pour is its reference. What two layers change is which number is
trustworthy: on a pour perforated by its own routing the declared pitch says
least and the measured maximum gap says most.

**The stand-in, so this rule can fail a board.** Both thresholds need a
frequency and nothing in this model declares one, which left R-14 as a
measurement with no line to cross. Its own sources supply the stand-in: where a
design does not state a frequency, screen at **1 GHz**. At that frequency, with
an effective dielectric constant of 4, the wavelength inside the board is
`lambda = c / (f * sqrt(eps))` = 150 mm, so the two published fractions give
**7.5 mm at lambda/20 and 15 mm at lambda/10**, and the return-via figure that
applies below 5 GHz is **1.27 mm**.

Conditions under the stand-in: the largest nearest-neighbour gap in a pour's
**placed** stitching field is at most 7.5 mm - the placed field, per "Declared
is not measured", never the declared pitch - and the distance from a signal via
to the nearest reference via spanning the same layer pair is at most 1.27 mm.

Every report from this rule states the frequency it assumed. A board that
declares one is measured against that; a board that declares nothing is measured
against 1 GHz and told so in the row, because a threshold nobody chose is a
number the reader has to be able to argue with. **The stand-in is a screen, not
an acceptance criterion.**

At 1 GHz a 7.5 mm gap passes almost any real pour, so this rule will catch
nothing until a board declares a frequency or until a stitching field is thinned
by routing - which is the second case, and the reason the gap is measured on the
placed field rather than read off the declaration. A first run reporting zero is
the rule working, not the rule missing.

**In this repo:** the declaration and the generator exist; no check exists.

### R-15 One component, one connection style `[P]`

*Applies when:* a two-terminal component has both pads inside pours. A board with no pour says nothing here.

R-09 states thermal relief as geometry - spoke width floor, spoke count, the
per-package pairs, the 3 A threshold and the bottom-terminated exception - and
none of that is repeated here. R-15 is the manufacturing consequence: what the
relief costs in service, what the exception actually requires, and the failure
that neither number predicts.

**1. What the relief costs, worked out rather than quoted.** R-09 carries the
1 to 3 milliohms a four-spoke relief adds. At 5 A that is 5 to 15 mV dropped
and 25 to 75 mW dissipated in the joint's own copper - which is what makes
R-09's 3 A threshold a rule rather than a preference, and it is the only
arithmetic needed to see why.

**2. What the bottom-terminated exception requires `[P]`.** R-09 says a QFN,
DFN or DPAK takes a solid connection and a via array under IPC-7093. The
array's own published numbers, read 2026-09-11: holes of 0.25 to 0.33 mm,
because larger ones wick solder away during reflow; spacing 1.0 to 1.2 mm;
typically 9 to 25 vias depending on pad size; voiding held below the IPC-7093
limit of 50 percent. Without those, "a via array" is not a condition anything
can check.

**3. Mixing the two styles on one component is what actually breaks joints
`[P]`.** A pad tied straight into a pour has more thermal mass than a relieved
one beside it, so during reflow the solder on the plane side melts later; the
free end lifts and the part rotates off its pad. That is tombstoning, and the
published guidance is symmetry rather than any particular pattern: apply the
same connection style to both pads of a chip component, and treat it as
mandatory for 0603 and smaller, where the part is light enough for the torque
to win (thermal-relief and tombstoning guides, read 2026-09-11).

Condition, and it is the one this canon adds that no other rule states: **for a
two-terminal component whose pads both sit in pours, both pads have the same
connection style.** One relieved and one solid is a defect even where each pad
on its own satisfies R-09.

This is also the only rule in this canon that looks at a **component** rather
than at a feature of copper. Every other condition here - clearance, angle,
ring, coverage, spacing - is a property of one feature or of a pair of
features. Symmetry is a property of a pair of pads that belong to one part, so
a registry built to walk copper cannot express it without walking components
too.

**4. What this project holds - and three of these are a fire, not a gap.**

- *Spoke width and gap against the house table:* geometry from the filled pour,
  numbers from the design rules. `PourOptions` carries `thermal_gap`
  (`crates/cypcb-core/src/pour.rs:246`) and `spoke_width` (`:248`). Checkable
  today.
- *Surviving spoke count:* the arm mapping R-09 specifies, on the same filled
  pour. `thermal_spokes` (`crates/cypcb-core/src/pour.rs:272`) cuts a fixed
  cross of four whatever any table says, and `thermal_relief_spokes` still has
  zero readers outside its own crate - `grep -rln thermal_relief_spokes
  --include=*.rs crates/ | grep -v cypcb-rules | wc -l` returns `0`. So the
  count that matters is the one that survives clipping, not the one declared.
  See "Declared is not measured". Checkable today.
- *Symmetry across a component's pads:* the pads, their nets and their
  component are all in the world already - the same data
  `crates/cypcb-drc/src/rules/unrouted_pin.rs` walks. Checkable today.
- *The 3 A rule:* a net's current is declarable and already read by
  `crates/cypcb-drc/src/rules/trace_current.rs`, so the **violation** is
  detectable today. What is missing is the cure: nothing can ask for a solid
  connection, so a board can be told it is wrong and given no way to be right.
  That gap is R-09's.
- *The bottom-terminated case:* nothing in the model marks a thermal pad as the
  thermal pad of a QFN or DPAK, so this half is not checkable and should not be
  faked with a size heuristic.

**In this repo:** three conditions are enforceable against data already
present, with no new field and no new declaration - which makes R-15 the first
rule in this canon whose gap is that nobody wrote the check, rather than that
the model cannot answer.

### R-16 What a rule must carry to be enforceable here `[O]`

*Applies when:* never, to a board. This is the canon reading itself.

Nineteen rules, three states - and the four that read the canon rather than a
board (R-12, R-16, R-18) or arrived after this census was written (R-17, R-19)
are placed at the end of it. This section is the canon reading itself: which
rules the board is held to, which wait on somebody writing a check, and which
wait on the data model - and then it counts the missing **fields** rather than
the blocked rules, because a field that unblocks two rules is worth more than
either of them.

**Bucket 1 - enforced today. Four.** R-19 is the fourth and the loudest; it was written last because a rule that fires on every board leaves no gap to notice. The registry has 38 entries
(`crates/cypcb-drc/src/lib.rs`); three of them serve this canon.

| rule | what enforces it |
|---|---|
| R-01 width against current | `TraceCurrentRule` (`lib.rs:143`), silent on a net that declares no `current` |
| R-03 acute angles | `AcuteAngleRule` (`lib.rs:196`), reporting `ViolationKind::AcidTrap` |
| R-07 annular ring and hole spacing | six rules - `AnnularRingRule`, `HoleToHoleRule`, `ViaDiameterRule`, `ViaDrillRule`, `PadLandRule`, `DrillAspectRatioRule` |
| R-19 the flat clearance minimum | `ClearanceRule`, first in the registry, firing more than the rest together |

**Bucket 2 - checkable today, nobody wrote the check. Eleven.** Checkable is
not the same as testable: see "A rule with no subject cannot be tested" for
which of these have anything to fire on, measured on the fixtures rather than
argued. R-08 is the one to write first, and it is the only one of the eleven
whose subject exists on all six boards. Every quantity
these need is in the world already.

| rule | why it is checkable, in one clause |
|---|---|
| R-05 return path | pours, stackup-derived reference layer and the spatial index are present, gated on the net's declared impedance |
| R-06 reporting per kind | `DrcViolation` carries its kind and its measured distance; only the aggregation is missing |
| R-08 trace entry into a land | pad geometry, rotation and teardrop ratios are in the model; the entry angle is arithmetic on them |
| R-09 thermal relief geometry | the filled pour gives the spokes, the design rules give the numbers - but the 3 A case has no cure |
| R-10 mitring | the junction is already reported by R-03 and the cut is geometry on two arms |
| R-11 acceptance classes | `ViolationKind` gives the kinds and `clearance_contacts` the count tier 3 needs; the class gate exists only when the board picks `IpcClass1`, `IpcClass2` or `IpcClass3` (`crates/cypcb-rules/src/presets/mod.rs:58`, `:60`, `:62`) and is absent for a house preset |
| R-12 rip-up and reroute | its three conditions are properties of code, two of them already measured |
| R-13 return path, measured | same data as R-05; coverage and crossings are geometry, loop area an estimate by design |
| R-14 via stitching | vias carry net, position and layer span, pours carry their pitch; the measurement is available, the thresholds are not |
| R-15 thermal relief in manufacturing | spoke width, surviving spoke count and pad symmetry all read from data already present |
| R-17 the search grid | pad positions and a fab table are on every board; the check is a loop over pads with the router's own snap |

**Not about a board at all. Three.** R-12 is about the router's loop, R-16 is
this section, and R-18 is about a row of output. Each says so in its own
*applies when* line, and R-12 fails entry condition 1 below - which is the
argument for moving it out of this document.

**Bucket 3 - the model cannot answer. Two.**

| rule | what is missing |
|---|---|
| R-02 spacing against voltage | no working voltage on a net; the IPC table is written and has no caller |
| R-04 stub length | two things at once - no copper connectivity graph, and no declared signal speed |

**The fields, counted rather than the rules.** Six, and the first two tie on
count, so the ranking turns on what each one costs to obtain.

1. **A copper connectivity graph.** It gives R-04 half of its blocker and R-05
   and R-13 their per-net restriction. It also fixes two defects in the router's
   own output that have nothing to do with this canon: the flat
   `HashMap<u32, Vec<Vec<GridNode>>>` has no parent relation, which is why
   tearing can only be net-wide (R-12), and why
   `crates/cypcb-autoroute/tests/abandoned_connections.rs` has to switch on a
   log subscriber to name the connections that were dropped. *Derived from data
   already present - segments, vias, pads, nets - and the only field on this
   list nobody has to declare.* That is the ranking argument, not the count.
2. **Signal speed per net, as a bit rate or a rise time.** It gives R-04 its
   divisor and R-14 both of its thresholds, and it sharpens R-13, which leans on
   declared impedance as a stand-in for "this net cares about its reference".
   Two rules and three thresholds - a tie with the graph on hard counting, and
   R-04 needs both, so neither releases it alone. *A declaration the designer
   makes, and a net without one stays unchecked forever; R-01 is the precedent,
   enforced and silent on every net that declares no current.*
3. **Working voltage per net.** One rule, and the cheapest of the six: the table
   R-02 needs is already implemented and unused, so this field turns dead code
   into an enforced rule with no new arithmetic. *A declaration.*
4. **A way to ask for a solid connection.** No new check, two cures: neither
   R-09 nor R-15 can be complied with on a high-current pad today, because the
   violation is detectable and the fix is inexpressible. *A declaration, per pad
   or per net.*
5. **A bottom-terminated marker on a footprint.** Half of one rule - R-15's
   IPC-7093 clause. The honest alternative is a size heuristic, which this canon
   declines. *A marker on a part.*
6. **An acceptance class on the board.** R-11 grades against a class; a board
   picking an IPC preset already declares one, a house preset does not. *A
   declaration, half of which exists.*

**The entry condition a future rule has to meet.** Drawn from what separated
bucket 2 from bucket 3 in practice, not from principle.

1. **Every quantity it compares is in the world or derivable without a solver.**
   Coverage and crossings pass; an emission level in volts per metre does not,
   which is why R-13 carries a proportionality and no threshold.
2. **The threshold has a dated source and an applicability gate the model can
   evaluate from a declaration.** R-01 passes because `current` is declared and
   the rule is silent without it. R-14's thresholds fail because nothing
   declares a frequency, even though the geometry is fully available.
3. **A compliant board can express the cure.** The clause nobody would write
   from principle, and the one that bites: the 3 A rule is checkable and
   incurable, so enforcing it would produce a report no designer can act on.
   R-18 states the same condition for a row of output.
4. **The quantity checked is the measured one, not the declared one.** See
   "Declared is not measured" - four rules in a row had to be rewritten around
   it, and a fifth will unless it is an entry condition.

### R-17 The grid the router actually searches `[O]`

*Applies when:* always, before routing. It needs pad positions and a fab table, both of which exist on every board.

Every other rule here is about copper. This one is about the tool, and it
belongs in the canon because entry condition 1 puts it there: the grid is
derived from the fab table, which is as much a property of the board as the
clearance it comes from. A board whose pads collide on the grid is a board
fault; the grid is only how it is detected.

**1. What the resolution is a function of.** One cell is one legal track
position. `resolve_grid_resolution` (`crates/cypcb-autoroute/src/lib.rs:372`)
takes the fab table for net 0 and returns `min_trace_width + min_clearance`,
floored at 10 um. The comment records the measurement that settled it: a
half-clearance grid let two nets sit in adjacent cells whose copper overlapped -
238 DRC violations in 127.8 s against 124 in 9.7 s at track pitch, same board,
both fully routed.

`resolve_adaptive_grid_resolution` (`:398`) then applies three things in order.
An explicit `grid_resolution_nm` **returns immediately**, before anything else
in the function - it is an instruction and not a hint, and the comment says a
caller asking for 0.254 mm on a 100 mm board used to get 0.508 mm in silence.
Otherwise a board wider or taller than 80 mm is coarsened by 2, and above 200 mm
by 3. Finally `params.density`, clamped to 0.5 to 2.0, divides the result, so
density above 1 gives a finer grid. The 10 um floor applies throughout.

So the resolution is a function of the fab table, the board's larger dimension
and one tuning parameter - and of nothing about the parts on the board. That
last clause is the rule.

**2. What the grid costs in accuracy, and it is worse than half a cell.** A pad
centre is snapped by integer division, not by rounding: `nm_to_grid_x`
(`crates/cypcb-autoroute/src/grid.rs:470`) computes `(nm - origin) / resolution`,
which truncates toward zero. The node therefore sits at or below the pad centre
on each axis, and the error approaches a whole cell per axis rather than half of
one. Worst case radially is `resolution * sqrt(2)`.

| table | pitch = width + clearance | worst-case snap |
|---|---|---|
| JLCPCB standard, 2 layer (0.127 + 0.127) | 0.254 mm | 0.359 mm |
| JLCPCB standard, 4 layer (0.100 + 0.100) | 0.200 mm | 0.283 mm |
| JLCPCB advanced, 4 layer (0.090 + 0.090) | 0.180 mm | 0.255 mm |
| any of the above on a board over 80 mm | doubled | doubled |

The comparison is the point: on the two-layer table the worst-case snap of
0.359 mm is 72 percent of the 0.5 mm pad pitch of the LQFP-64 that `qfp_fanout`
is built around. Rounding rather than truncating would halve every number in
that column. Stated as a fact about the grid, not as a proposal.

**3. Nothing published fixes a router grid against the minimum feature size.**
Searched 2026-09-11. The published grid guidance is about the design grid a
designer places parts on, not the search grid a router walks. One line, and no
further looking - that answer has now been the right one three times.

**4. The rule this suggests, and this project can evaluate it today.**

> A board is not routable on the grid it was given when two pads of different
> nets snap to the same grid node, or when the worst-case snap exceeds half the
> smallest pad pitch on the board.

The first clause is the hard failure: two pads sharing one node cannot be told
apart by the search, so one of them is reachable only through the other. The
second is the warning: above half the pitch the snapped position can land closer
to the neighbouring pad than to its own.

Evaluable with no new field - pad positions come from the footprint library
placed by component position and rotation, the resolution from the two functions
above, and `nm_to_grid_x` is the same snap the router uses. The check is a loop
over pads, and it belongs **before** routing rather than after: a board that
fails it produces violations whose cause is the grid, and every number measured
on such a board describes the grid rather than the router.

### R-18 What a violation report owes its reader `[P]`

*Applies when:* never, to a board. This is about a row of output.

A report says what was measured, where, against what, by which rule, and what to
change - and it names what it did not check. R-06 says the counts must be per
kind and R-16 says a rule must have an expressible cure; R-18 is what a single
row has to carry for either of those to be readable.

**1. No standard specifies the contents of a violation report; the tools
converge.** Searching on 2026-09-11 for an IPC clause on report content returned
nothing - acceptance standards grade features, they do not specify the file a
checker writes. What the most documented tool publishes: a violation type, a
severity of error, warning, exclusion or ignore, a description, the positions of
the items involved, and item ids; severity configurable per rule type,
violations excludable individually, and the same set emitted as JSON from the
command line (KiCad documentation and issue tracker, read 2026-09-11). One thing
is a standing request there rather than a feature, and it is a gap this canon
names elsewhere: a report should state which checks were **not** performed
because their severity was set to ignore, since an ignored rule produces no rows
and therefore looks exactly like a rule that passed.

**2. What this project's violation carries today.** `DrcViolation`
(`crates/cypcb-drc/src/violation.rs:14-45`): `kind` at `:16`, `location` at
`:18`, `entity` at `:20`, `other_entity` at `:22`, `source_span` at `:24`,
`message` at `:26`, `actual` at `:34`, `required` at `:36`, `area` at `:44`.

- *Location:* carried, and better than the tools in one respect - `source_span`
  points at the line of the design file that caused it, which a coordinate
  cannot.
- *Items involved:* carried as `entity` and `other_entity`, stable within a run
  and meaningless across runs.
- *Measured and required values:* carried, optionally. See part 4.
- *Rule identity:* **partial.** `kind` is a category; the rule's own name exists
  only as `DrcRule::name()` and is never stored on the violation it produced, so
  a report cannot say which rule fired when two rules share a kind.
- *Severity:* **absent.** There is no field. Every violation leaves the checker
  equal, and the four tiers R-11 describes have nowhere to live in the row. This
  is the same absence R-11 names from the ranking side, and it is cheaper to fix
  than it looks, because R-11 already defines the tiers - what is missing is one
  mapping from kind to tier, beside `kind`.
- *Exclusion state:* absent, which follows from severity being absent.

**3. A report may not name a fault the model cannot cure.** R-16's third entry
condition, stated for output rather than for rules. Condition: **a kind is
registered as a defect only when some change expressible in the design clears
it; a kind with no such change is emitted as an advisory and labelled so in the
row.** The live instance is relief on a pad above 3 A - detectable today, with
no way to ask for a solid connection, so shipping it as a defect produces a row
a designer can read and cannot answer.

**4. Two violations measure a distance and throw it away.** Counted on
2026-09-11 across every constructor in `crates/cypcb-drc/src/violation.rs`:
seventeen kinds whose fault is a distance record it, fourteen whose fault is not
a distance correctly record nothing, and **two take the measurement as a
parameter and then set the field to `None`**:

- `hole_to_hole` (`:617-627`) - `actual: Nm` at `:620` and `required: Nm` at
  `:621`, both discarded at `:626-627`.
- `solder_mask_bridge` (`:683-693`) - the same, at `:686-687` and `:692-693`.

Both print the numbers into `message`, which is why no compiler warning ever
flagged the unused parameters, and why the fault is invisible to a reader of the
source but not to a reader of the output.

The consequence is measurable rather than theoretical. `cypcb check` ranks
violations worst-first by `shortfall(violation).unwrap_or(-1.0)`
(`crates/cypcb-cli/src/commands/check.rs:228-244`), and its own comment says
rules that measure no distance keep their order at the end, because a number
invented for them would sort them among the ones that have one. So these two
rows sort to the end beside the unrouted pins - **not because they measure
nothing, but because they measured and did not record it.** A hole 0.05 mm from
another where 0.15 mm was required is a two-thirds miss, and it sorts below a
trace that missed by five percent.

Condition, narrower than "every violation carries a number", because most of the
fourteen are right to carry none:

> Every violation whose kind measures a distance carries the distance it
> measured and the distance it required. A kind that measures a distance and
> reports `None` is a defect in the rule, not a property of the board.

**One kind escapes that condition and is worth naming rather than forcing.**
`DiffPairSkew` covers two different faults: a measured skew, which passes
`Some` (`crates/cypcb-drc/src/rules/diff_pair.rs:106`), and a pair naming a net
that is not on the board, which passes `None` (`:79`) and is right to. So "does
this kind measure a distance" is a property of the **call site** there, not of
the kind, and any table built on kinds needs this one exception written into it.

**5. Three numbers exist and no field can hold them.** `impedance` (`:1145`)
measures ohms, `acid_trap` (`:921`) measures degrees, and `neck_down` (`:1164`)
carries a comment saying the `actual`/`required` pair cannot say which of two
dimensions it means. In all three the number exists and lives only inside the
`message` string, where no ranking will ever see it. `acid_trap` is the only
rule this project wrote itself in the last week and it already falls into this
category, which is the argument for deciding rather than leaving it.

**In this repo, three conditions are unmet and countable:** severity has no
field, rule identity is not carried on the row, and two distance-measuring
constructors report no distance. The first two are each a one-field change; the
third is two lines.

### R-19 The flat clearance minimum `[P]`

*Applies when:* always. Two pieces of copper on one layer belonging to two
different nets. No declaration needed, which is why this is the rule that fires
most.

Copper of two different nets on one layer keeps at least the minimum the
fabricator publishes. The minimum is a property of the **net pair**, not of the
board, and it is the floor R-02 scales above rather than a rule beside it.

**1. What varies the minimum, in what the houses publish.** Layer count, carried
here by having a separate preset per layer count. Copper weight, which at least
one house publishes as a track-and-space table per 1, 2 and 3 oz and which this
project does **not** model - one preset carries one figure whatever the copper.
And the board edge, which is published separately everywhere and modelled
separately here as `min_edge_clearance`.

**2. Every fab preset states where its figure came from, and the three IPC ones
state that they came from nowhere.** That distinction is the point of the row,
so it is written out rather than tabulated: `crates/cypcb-rules/src/presets/`
carries a sourcing comment above each clearance figure - what the house
publishes, and for two of them what the number used to be and why it was
changed. The three `IpcClass` presets carry the opposite note, and it is
accurate: their ladder is *"this project's, not a table anybody can open"*.
IPC-2221's spacing table is voltage-based, which is R-02, and IPC-6012's classes
are acceptance criteria, which is R-11. **Neither publishes a flat spacing
ladder by class**, so the 0.2 / 0.15 / 0.1 mm figures are a house-style default
and the file says so where a reader will meet them.

**3. The figure is per pair, not per board.** `clearance_between`
(`crates/cypcb-rules/src/presets/mod.rs:419-427`) takes the stricter of the two
nets' constraints, so a net class can raise the floor for every pair that
touches it. Condition: **the required distance for a pair is the larger of the
two nets' minima, and the board-level figure is only the default neither net
overrode.**

**4. Relation to R-02, stated so neither rule can be read alone.** The flat
minimum applies to every pair of different nets, declared or not. R-02 applies
only where a net declares a working voltage, and it can only raise the figure.
Where both apply the required distance is the larger of the two. Where R-02 is
silent - which today is every board, because nothing declares a voltage - the
required distance is this rule's figure alone. R-19 is the floor; R-02 is the
part of the floor that moves with volts.

**5. What the rule skips before it measures anything.** Opened rather than
grepped: `crates/cypcb-drc/src/rules/clearance.rs`. Four skips, in order - the
same entity against itself, because a trace has several boxes in the spatial
index (`:164-167`); pairs whose layer masks do not overlap (`:169-172`); pairs
already checked, by canonical ordering (`:174-178`); and pairs on the same net
(`:180-222`). The same-net test has three branches and they are the rule rather
than an aside: two sides carrying a `NetId` compare directly; a trace against a
component passes when the trace's net appears in that component's pin
connections; two components pass when they share a net.

One refusal is deliberate and documented at `:197-201`: where pad geometry
exists the exemption is decided **per pad** further down, not for the whole
component, because a part with one GND pin is not a GND part and exempting the
component would hide a trace crossing its VCC pad.

**6. This rule cannot see copper drawn over copper, and that is why the fault
took three sessions to find.** Two runs of one net lying on each other are
same-net by construction and leave at `:222` before any distance is measured.
That is correct for this rule - same-net copper touching is a connection, not a
fault - but it means the rule that fires most in this project was structurally
blind to the defect that produced **142 of 195** acute reports on the benchmark
set. The rule that surfaced it was R-03, which counts junctions and does not ask
whose net they belong to.

Condition, and it generalises beyond this one case: **a fault between two pieces
of copper on one net is outside R-19's scope by definition, and any rule that
needs to see one has to measure geometry rather than clearance.**

**In this repo:** enforced. `ClearanceRule` is the first entry in the registry
(`crates/cypcb-drc/src/lib.rs:129`). Measured on one board rather than claimed
for all six: on `shift_driver` with `stop_at_own_copper` on, **27 of 32 rows**
are this rule.

## A rule with no subject cannot be tested

Measured on the six benchmark fixtures on 2026-09-11, before any of the rules
below were ranked: **780 pads, one pour, zero `.kicad_pro` files.** Pads per
board are 14, 278, 51, 140, 156 and 141; `plane_board` carries the only zone;
and net constraints reach the model only through a project file, so **no net on
any of these boards declares an impedance, a current or a voltage.**

That measurement sorts the rules this canon says are checkable today into three
states, and the third one is a trap:

1. **A subject on every board.** R-08 is the only one: every routed connection
   ends on two pads, and there are 780 of them.
2. **A subject on one board.** R-09, whose subject is the pads sitting inside
   `plane_board`'s single pour.
3. **No subject at all.** R-05, R-13 and R-14 are gated on declarations no
   fixture makes, or need a second pour that no fixture has. R-12 is not about
   a board.

**The trap is that a rule with no subject has no positive control.** A correct
implementation and one that returns an empty list are indistinguishable, so the
first green run means nothing - the same disease as a gate whose trigger cannot
fire, which is why this project's own checks now say "not applicable" rather
than "clean". For R-05, R-13 and R-14 the remedy is not code but a fixture: a
four-layer board with a pour, and for R-14 one with two pours of one net.
Writing them first would mean writing blind and verifying nothing.

**So every rule this project implements publishes its denominator** - the number
of things it examined beside the number it reported. Without it, "no findings"
cannot be told from "found nothing to look at", and on `qfp_fanout` those two
answers differ by several hundred.

## Declared is not measured

Three rules in a row had to be rewritten around the same mistake, which makes
it a design rule for this canon rather than three coincidences.

- **R-13, loop area.** Under continuous reference the area is length times the
  dielectric separation in the stackup, whatever path the router takes. The
  number is real and the router cannot move it, so checking it checks the
  stackup and calls it routing.
- **R-14, stitch pitch.** The pitch is declared on the pour and the generator
  drops every grid point that is blocked, so a dense board gets a thinner field
  than it asked for. The number that matters is the largest gap in the placed
  field, not the pitch in the source.
- **R-15, spoke count.** `thermal_spokes` cuts a fixed cross of four and the
  filler clips whatever the pour cannot carry, so the count a joint actually
  has is the count that survived, not the count in any table.

- **R-17, the grid position of a pad.** `nm_to_grid_x` snaps by integer
  division, so the node the search uses sits at or below the pad centre on
  each axis. The position the design declares is not the position the router
  works from, and the error always points the same way.

In each case the declared or theoretical quantity is available, cheap and
wrong, and the measured one takes work. A rule that takes the cheap number is
not a weaker rule - it is a rule about something else. R-16 carries this as the
fourth entry condition a new rule has to meet.

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
   the nearest continuous return copper. R-13 bounds what this is worth:
   under continuous reference the area is set by the stackup and not by the
   route, so it is a number for uncovered spans only.
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

Two rules cannot be enforced without a change to the data model. The rest are
waiting on code; R-16 sorts every rule and counts the missing fields rather
than the blocked rules.

**R-02, working voltage.** Nets have no voltage field: "voltage" does not
appear in `crates/cypcb-parser/src/ast.rs` or
`crates/cypcb-world/src/components/electrical.rs`. The table is written and
there is nothing to ask it with. `ClearanceRule` holds a flat `min_clearance`
instead.

**R-04, stub length.** Two gaps. First, copper has no connectivity graph. What
exists is geometry: `Trace { segments, width, layer, net_id, locked, source }`
and
`Via { position, drill, outer_diameter, start_layer, end_layer, net_id, locked }`
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

# R-12: net order - two keys, stable sort, no map iteration
sed -n '192,216p' crates/cypcb-autoroute/src/orchestrator.rs

# R-12: the subset that is re-routed each iteration
grep -n "nets_needing_reroute" crates/cypcb-autoroute/src/pathfinder_v2.rs

# R-12: the congestion term, and where it enters the total
sed -n '214,228p' crates/cypcb-autoroute/src/congestion.rs

# R-12: what a net's routing is stored as
grep -n "pub routed_paths" crates/cypcb-autoroute/src/pathfinder_v2.rs

# R-13: the gate the rule keys on, and that the rule already reads it
grep -n "impedance_ohms_x100" crates/cypcb-drc/src/rules/impedance.rs

# R-13: the reference layer comes from the stackup, not from a guess
grep -n "CopperEnvironment" crates/cypcb-drc/src/rules/impedance.rs

# R-13: a zone is a rectangle, so a gap is absent pour and not a slot
sed -n '60,66p' crates/cypcb-world/src/components/zone.rs

# R-13: coverage has to be measured against the filled geometry
grep -n "pub fn fill_zone" crates/cypcb-world/src/copper.rs

# R-13: the spatial query coverage would reuse, and its one caller today
grep -n "query_region_on_layers" crates/cypcb-autoroute/src/scoring.rs

# R-16: the registry's size, against the three rules bucket 1 names
grep -c "Box::new(rules::" crates/cypcb-drc/src/lib.rs

# R-16: the acceptance classes that gate R-11, and the house presets that do not
sed -n '56,63p' crates/cypcb-rules/src/presets/mod.rs

# R-17: an explicit resolution returns before anything else touches it
sed -n '398,412p' crates/cypcb-autoroute/src/lib.rs

# R-17: the snap truncates, which is why the error is a cell and not half of one
sed -n '468,478p' crates/cypcb-autoroute/src/grid.rs

# R-18: the two constructors that take a measurement and discard it
sed -n '617,628p;683,694p' crates/cypcb-drc/src/violation.rs

# R-18: what that costs - both rows sort to the end of the report
sed -n '228,244p' crates/cypcb-cli/src/commands/check.rs

# R-18: the kind that measures a distance at one call site and not the other
grep -n "diff_pair_skew" crates/cypcb-drc/src/rules/diff_pair.rs

# R-14: the pitch is declared on the pour and carried on its entity
grep -n "zone_stitch" crates/cypcb-parser/src/parser.rs
sed -n '85,88p' crates/cypcb-world/src/components/zone.rs

# R-14: the generator drops every blocked point, which is the finding
sed -n '44,60p' crates/cypcb-world/src/stitch.rs

# R-14: the via a stitching field is made of
sed -n '32,42p' crates/cypcb-world/src/stitch.rs

# R-14: nothing in the checker asks about any of it
grep -rln stitch crates/cypcb-drc/src/ | wc -l          # expect 0

# R-15: the two relief numbers the export path carries
sed -n '244,249p' crates/cypcb-core/src/pour.rs

# R-15: the spoke count is a fixed cross, and the constant has no reader
grep -n "fn thermal_spokes" crates/cypcb-core/src/pour.rs
grep -rln thermal_relief_spokes --include=*.rs crates/ \
  | grep -v cypcb-rules | wc -l                         # expect 0

# R-15: the data a symmetry check would walk already has a rule walking it
ls crates/cypcb-drc/src/rules/unrouted_pin.rs crates/cypcb-drc/src/rules/trace_current.rs

# R-19: the four skips, and the per-pad refusal with its documented reason
sed -n '162,180p;195,202p' crates/cypcb-drc/src/rules/clearance.rs

# R-19: the required distance is the stricter of the two nets, not the board's
sed -n '419,427p' crates/cypcb-rules/src/presets/mod.rs

# R-19: every fab preset sources its figure; the IPC ones say they cannot
grep -n "min_clearance" crates/cypcb-rules/src/presets/*.rs

# R-19: the rule that fires most is the first one the registry runs
sed -n '129p' crates/cypcb-drc/src/lib.rs

# A rule with no subject: what the fixtures actually carry
for f in tests/fixtures/benchmark/*.kicad_pcb; do
  printf '%-30s pads=%-4s zones=%s\n' "$(basename "$f")" \
    "$(grep -c '(pad ' "$f")" "$(grep -c '^  (zone' "$f")"
done

# and why no net on them declares anything: constraints arrive by project file
ls tests/fixtures/benchmark/*.kicad_pro 2>/dev/null | wc -l   # expect 0
```

Last verified: 2026-09-11, including R-10 through R-19 and the two sections on what a measurement is worth. Web sources were
read on 2026-09-11, and every repository claim in those four rules was read
against the working tree on the same day by opening the file rather than
grepping for the name: `DEFAULT_TOLERANCE`, `is_90_bend`, `compute_composite`
and the variant sort for R-10 and R-11; `nets_needing_reroute`, the tear block,
`congestion_cost` and the type of `routed_paths` for R-12; and
`impedance_ohms_x100`, `CopperEnvironment`, the zone's `bounds`, `fill_zone`
and `query_region_on_layers` for R-13. For R-14 and R-15: `StitchPitch`,
`StitchSpec::at`, `stitching_vias` and its doc comment, the `Via` struct's
seven fields, `zone_stitch`, `thermal_gap`, `spoke_width` and
`thermal_spokes`. For R-16 through R-18: the registry's 38 entries, the three
`IpcClass` variants, the early return for an explicit resolution, the integer
division in `nm_to_grid_x`, both constructors that discard their measurement,
the ranking comment in `check.rs`, and both `diff_pair_skew` call sites.
Negative claims were run rather than assumed: no file under `crates/cypcb-drc`
mentions stitching, `thermal_relief_spokes` has no reader outside its own
crate, and no search on 2026-09-11 found a published permitted fraction for
R-13, a router-grid rule for R-17, or a standard specifying report contents
for R-18.
