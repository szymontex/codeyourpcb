# The in-house router: a plan, written before any code

D1 is answered. FreeRouting is out, the router this project ships is its own,
and the owner expects roughly a month before it is beyond criticism. The
approach was left to be proposed, and this is the proposal.

This document exists because a month of work does not start from a heartbeat
fire. It says what is wrong in numbers that already exist, what to build, and
what would have to happen on the six benchmark boards for each step to count.
Read `docs/routing.md` first - it is the measured record this plan is derived
from, and every number below is quoted from it rather than estimated here.

Nothing in this plan is a schedule. The one duration in it is the owner's own
figure, and it is theirs.

---

## 1. What is wrong today, in numbers

### The router does not finish

`where_the_band_comes_from` prints the overused-cell count per iteration:

```
stm32_breakout 0.24: 12 iterations, converged false, [695, 514, 340, 361, 377, 336, 297, 284, 289, 319, 304]
stm32_breakout 0.26:  7 iterations, converged false, [673, 495, 320, 348, 346, 365]
```

Negotiated congestion is supposed to end with no cell shared. On both dense
boards it ends on the stagnation break with **250 to 380 cells still shared**.
Every violation that follows is downstream of a loop that stopped without
solving the problem it was posed.

The trajectory also says where the leverage is. Half the violations of a
finished board are gone by iteration 4 - 286 to 148 on stm32_breakout - and the
tail is flat (125, 145, 140, 125, 120, 121). **Nothing that only adjusts the
late schedule can close this.**

### What the boards actually score

The honest column is `introduced` - what the routed board reports that the
imported fixture did not - because `before` and `after` move whenever the
checker changes. Introduced does not, and has not.

| board | introduced | shorts | own noise band |
|---|---|---|---|
| `led_blink` | 2 | 0 | 0 / 0 |
| `stm32_breakout` | 221 | 136 | 59 / 61 |
| `multi_ic` | 247 | 166 | 65 / 56 |
| `shift_driver` | 72 | 33 | 17 / 8 |
| `qfp_fanout` | 336 | 179 | 57 / 44 |
| `plane_board` | 28 | 13 | 0 / 0 |

The band is the second number that matters. Prices a hundredth apart ask the
router for the same trade and get boards up to 65 violations apart. **A tuning
value picked inside that band is noise with a decimal point.** The router is
deterministic - `is_the_router_repeatable` proves it to the nanometre - so the
band is not randomness. It is negotiated congestion amplifying a hundredth of a
price into a different rip-up order.

`plane_board` is the control: it routes identically at every price in the
range, to the violation, because a ground plane leaves far fewer nets competing
for the same cells. Along with `led_blink` it is one of two boards where **any**
movement is signal rather than noise.

### Where the violations are

`grid_vs_checker` cross-tabs every introduced clearance violation against what
the grid thought was in that cell:

| board | total | on a cell marked as a pad | `part <-> trace` on a pad cell |
|---|---|---|---|
| `stm32_breakout` | 206 | 151 (73%) | **109 (53%)** |
| `multi_ic` | 215 | 175 (81%) | **112 (52%)** |

The grid knew. A pad zone switches every obstacle off within its radius so a
route can reach the pin it is heading for, and inside that radius a *foreign*
part's pad is switched off with the rest. Half the defect count on both dense
boards comes through that one hole.

Narrowing the zone to the connection's own two pads was measured and made both
boards worse with nothing abandoned - so the router is not wandering through
pads it has no business near. It is inside its own pad's disc, colliding with a
neighbouring part's pad the disc happens to cover. **The radius is the suspect,
not the scope.**

### Seventeen instruments, and the one shape they share

`docs/routing.md` lists seventeen instruments built, measured and reverted.
Seven of them were vetoes; all seven lost. The file already states the rule it
learned: *if the instrument you are about to write returns a bool, write it as
an f64 instead and measure the price.*

There is a second regularity underneath it, and it is the one this plan is
built on. **Every failed instrument was trying to make a grid of whole cells
express a distance that is not a whole number of cells.** The arithmetic is
written out in the tracker for the via case: the keepout is

```
drill/2 + annular + clearance + width/2
= 0.15 + 0.127 + 0.127 + 0.0635
= 0.4675mm
```

which at 0.254mm per cell rounds up to a 2-cell disc - **0.508mm, against
0.277mm of actual copper.** The grid over-blocks by 83% and cannot do
otherwise. Hard-owning the via ring cost stm32_breakout 124 to 154 introduced
and multi_ic 42 to 55, and the tracker's own conclusion was that at this
resolution the grid *cannot express via clearance*.

The finer grid was then tried directly: 0.127mm instead of the track pitch took
stm32_breakout from 15.88 to 39.46 violations per 100mm of copper and doubled
the time. Halving the cell does not fix a quantisation problem, it halves it
and doubles the work.

### The score is not the objective

`compute_composite` adds six terms. Three were read for the first time on
2026-08-08 and two were not measuring what they are named after:

- **smoothness** reads 1.0 on every board ever routed, because this router lays
  45-degree turns and a 45-degree turn costs nothing.
- **crossings** is charged twice: two traces crossing at one point pay 500 as a
  crossing and 1000 for the same contact called a short.

Neither is a defect on its own terms. Both mean the six terms are not six
independent things, and a weight tuned on one moves another.

Below that sits a larger gap. **A\* minimises length, via cost and congestion
history. The board is graded on clearance violations and shorts.** Nothing
connects the two quantities. The search has never once been told what it is
being marked on.

### Three more, recorded so the plan does not rediscover them

- **The shipped default is fourth of eight on every board**, and the winner is
  different each time - `High-Density`, `Tight Pads`, `Pad Aware`. The eleven
  variants are eleven guesses at one question, and nothing has ever searched
  the space between them.
- **The repair pass wins by emptying the board.** On multi_ic it accepted an
  attempt carrying 108 routes against 945: `measure` applies the previous
  result before the next attempt, so the ratsnest is nearly empty and 40
  violations on an eighth of a board beats 256 on a whole one.
- **Vias stack at different depths.** 4, 7 and 15 hole-to-hole on the three
  dense boards, and the drill files show different layer spans, so they are not
  duplicates to filter. Two holes at one point at different depths is a routing
  decision, which makes it a cost-model question.

---

## 2. What to build

One sentence: **replace cell occupancy with a clearance field, and give the
search a term that predicts the violation the checker will report.**

### 2.1 A clearance field instead of an occupancy grid

Today `RoutingGrid` answers "is this cell free" with a bool, after bloating
every obstacle by `min_clearance + min_trace_width / 2` and rounding **up** to
whole cells. That rounding is the quantisation every failed instrument ran
into.

The replacement is one number per cell per layer: **the distance in nanometres
from the cell centre to the nearest foreign copper.** A distance transform over
the obstacle set computes it in one pass proportional to the cell count, and
the cell count is small - stm32_breakout's grid is 296 x 256 x 2, about 151
thousand cells, against roughly 900 A\* searches per iteration over the same
grid.

What that buys, item by item against the failure list:

- A via keepout of 0.4675mm is *0.4675mm*, not two cells. The 83% over-block
  disappears without changing the cell size.
- "Within clearance" stops being a cell count and becomes a comparison of two
  nanometre quantities - the same comparison `cypcb-drc` makes.
- The pad-zone radius, which the cross-tab names as the suspect behind half of
  all introduced faults, becomes a distance rather than a disc of cells.
- Every instrument in the dropped table that failed on quantisation can be
  re-asked. **This does not mean re-running them.** It means the ones whose
  stated mechanism was arithmetic - the via ring, the pad margin, the
  clearance block - have a reason to be re-measured, and the ones that failed
  for other reasons (ratsnest seeding, crowded-nets-first, best-of-N orderings)
  do not.

Cost to measure before believing any of it: the field is rebuilt whenever
copper changes, which is once per net per iteration in the worst case rather
than once per iteration. If that is too often, the fallback is an incremental
update over the cells a single route touches. **This is the plan's largest
performance risk and it gets measured first, not last** - see step 1 below.

The project already knows where its instructions go. Callgrind on
`shift_driver`: 1,178,718,274 data references, 8,395,613 D1 misses at a 0.7%
miss rate, 127,924 last-level misses at 0.0%. **The router is
instruction-bound, not memory-bound.** A field of `i32` nanometre distances is
four bytes a cell where the occupancy byte is one; on a working set this small
that is a bandwidth question the machine has already answered, and the real
question is how many instructions the transform adds. Measured, not assumed.

### 2.2 A cost term that predicts the violation

With a field in place, the node price gains one term, and it is a barrier
rather than a veto - the rule this vector has already paid for seven times:

```
clearance_penalty(node) =
    0                                    when field(node) >= required(pair)
    k * (required - field)^2 / required^2 when it is below
```

Continuous, zero in the legal region, rising steeply into the illegal one, and
unbounded at contact - so a short is the most expensive thing the search can
buy, which is what the project's own ranking already says it should be.

`required(pair)` is the same number `cypcb-drc` uses. That is the point: the
search would be minimising a smooth relaxation of the metric it is graded on
instead of a proxy nobody has connected to it.

`k` is a knob and will therefore be swept, per board, against each board's own
band. **The plan explicitly does not predict which value wins.** If `k` cannot
move any board outside its band, the term has failed and the honest outcome is
a row in the dropped table.

#### The question this section did not answer, and does now

The field knows distances and not nets. A barrier over it charges a route for
approaching **its own** pad, which is the one approach every route has to
make - so as written above the term would price connecting a net at all. That
gap held step 4 back for several fires and is the reason step 3 was deferred
behind it.

The search already carries the missing half. `pad_zones_per_net[net_idx]` is a
list of `PadZone { cx, cy, radius }` - the discs belonging to the net being
routed - built once per net and passed into the search as `&[PadZone]`, which
already consults them. So:

- **Outside the routing net's own pad zones**, the barrier applies. That is
  where foreign copper is, and where a violation the checker reports comes
  from.
- **Inside them**, the barrier is suppressed. A route reaching its own pin is
  not a fault and must not be priced as one.

No new structure, no per-net field, nothing rebuilt: one lookup the search
makes anyway.

#### And the half that creates, which is why it is written down here

Suppressing inside the zone re-creates the defect the cross-tab named. More
than half of every introduced violation is `part <-> trace` **on a cell the
grid marked as a pad** - 109 of stm32_breakout's 206, 112 of multi_ic's 215 -
and the mechanism is that a net's own pad zone covers a *neighbouring part's*
pad, which the zone switches off along with everything else. A barrier that
goes quiet inside the zone goes quiet there too.

The two structures answer different halves and the fix is to use each for what
it knows:

- the **field** gives a distance and no net, and is right outside the zone;
- `pad_owner` gives a net and no distance, is already per-cell, and is right
  inside it.

So inside a pad zone the barrier is not switched off but narrowed: it applies
to cells whose `pad_owner` is a **different** net, and to nothing else. That
keeps a route free to reach its own pin, keeps it paying for a stranger's pad
it is sitting on, and needs neither a new index nor a second field.

Stated so it can fail: if that narrowing does not move the cross-tab's
`part <-> trace on a pad cell` figure, the diagnosis behind this whole plan is
wrong about where the violations come from, and step 4 dies with it.

### 2.3 The loop that terminates

Only after the two above are measured. It is third for a reason: a convergence
fix on a router with the wrong cost model converges to the wrong board faster.

Two facts bound it. The runs differ from iteration 1, so nothing that adjusts
only the late schedule can close the band. And the stagnation test is reading a
signal that has stopped meaning anything - `does_overuse_track_the_violations`
found the iteration with the fewest violations (120) was the one the loop saw
with its **worst** overuse reading (460).

So the loop should keep the best board it has seen by the objective it is
graded on, not the last one, and not the one with the fewest overused cells -
which was measured and lost (stm32_breakout 133 to 152). With a cost term that
tracks DRC, "best seen" becomes a quantity worth keeping for the first time.

### 2.4 What is deliberately not in this plan

- **A new search algorithm.** Negotiated congestion is not the thing that has
  been measured as failing. Its termination and its prices are.
- **A finer grid.** Measured, doubled the time, nearly tripled the violation
  density.
- **Retuning the composite.** Two of its six terms are known not to measure
  what they are named after and two are known to double-charge. Tuning weights
  on top of that is fitting to an instrument with a documented fault.
- **Rewriting the variants.** They become a search over the knob space once
  there is a cost model worth searching, and that is after, not during.

---

## 3. How progress is measured

The same six boards, every time, and the same three commands. Anything not
measured this way did not happen.

```
cargo test --release -p cypcb-autoroute --test benchmark_validation -- --ignored --nocapture
cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored --nocapture
cargo test --release -p cypcb-autoroute --test grid_vs_checker -- --ignored --nocapture
```

**The acceptance bar, written down before any number is seen:**

1. A change counts when it moves at least one board **outside** that board's
   own measured band in the right direction.
2. A change is refused when it moves any board outside its band in the wrong
   direction, whatever the totals say. Totals across boards are not a
   criterion - the weighted-heuristic sweep looked like a 96-violation win on
   the total and was noise on five of six boards plus one real regression.
3. `led_blink` and `plane_board` have a band of zero. **Any** movement on
   either is signal, in both directions, and gets read before the totals.
4. Shorts outrank violations, because that is what the shipped ranking already
   does and because a short is the one fault a user sees without reading
   anything.
5. Every step re-runs the determinism check. If the router stops being
   repeatable, every number in `docs/routing.md` becomes unreadable and the
   step is reverted regardless of what it bought.

Each step below states its own falsifier. A step whose falsifier fires gets a
row in the dropped-instruments table with its numbers - that table is the most
valuable artefact this vector has produced, and it stays that way by being
allowed to grow.

### The order, and what each step has to show

**Step 1 - the field, with nothing reading it yet.**
Build the distance transform, populate it, and leave the router on the
occupancy grid. Assert the field agrees with the grid wherever the grid can
answer: a cell the grid calls blocked has a field below the bloat radius, and
one it calls free does not. Then measure the cost.
*Shows:* every board's routing output is **byte-identical** to today, and the
added time per board.
*Falsifier:* any board's violation count moves at all. Nothing reads the field
yet, so a movement means the transform perturbed something it must not touch.

**Step 2 - the via keepout reads the field. RUN, AND THE FALSIFIER FIRED.**
The narrowest possible first consumer, and the one whose arithmetic is already
written down. Nothing else changed.
*Showed:* not "no movement" but movement the wrong way. Measured in nanometres
instead of whole cells, the disc drops from 13 cells to 9 and **three boards
get worse outside their own bands while none gets better outside its own** -
`led_blink` 2/0 to 3/1 and `plane_board` 28/13 to 38/19, both on a band of
zero, and `qfp_fanout` shorts 149 to 199 against a band of 44. Compensating the
price for the smaller disc (0.25 x 13/9 = 0.36) makes it worse again. The full
table and the mechanism are in `docs/routing.md` under "The over-block is
load-bearing".
*What it means for this plan:* section 2.1's premise - that the grid's
rounding is a cost and only a cost - is refuted for this instrument. The
over-block was also a **margin**, worth 0.254mm where the fab asks 0.127mm,
and the search had been relying on it without anybody writing that down.
*What survives:* the field measures what it says it measures, and step 1's
numbers are unaffected. What does not survive is the idea that a consumer can
simply take the rounding away and keep everything else. **Any reader of the
field has to supply its own margin explicitly.** That is an argument for going
to step 4 next rather than for retrying step 2: a barrier term is non-zero
before contact, so it carries a margin by construction rather than by
accident.

**Step 3 - the pad zone reads the field.**
The cross-tab says this is where 52-53% of introduced faults are. The radius
becomes a distance and the foreign-pad case stops being switched off with the
rest.
*Shows:* the cross-tab re-run. The `part <-> trace on a pad cell` figure is the
number this step exists to move: 109 of 206 and 112 of 215 today.
*Falsifier:* the cross-tab does not move, or the boards get worse. Both were
the outcome the last three attempts at this had, and this attempt differs from
them only in having a real distance to work with.

**Step 4 - the clearance term in the cost. Unblocked as of 2026-08-13.**
Section 2.2, swept for `k`, per board, against the bands.
*Shows:* which boards move outside their band, and whether any `k` moves more
than one.
*Falsifier:* no `k` moves any board outside its band. Dropped table, and the
cost model is then a smaller idea than this plan claims.

**Step 5 - keep the best board, not the last.**
Only if 2 through 4 have produced something. Requires the objective from step 4
to be worth maximising.
*Shows:* per-iteration objective against the final board, on the two dense
fixtures.
*Falsifier:* the best-seen board is the last board on most runs, in which case
the loop is already doing this and the fix is a no-op.

**Step 6 - re-ask the dropped instruments whose mechanism was arithmetic.**
The via ring, the pad margin, the clearance block. Named individually, measured
individually, and the table updated either way.

**Step 7 - the variants become a search.**
`docs/routing.md` already has the next action written: measure how much better
the best point in a small grid is than the best shipped variant, per board,
before building anything that searches.

---

## 4. Where the current architecture fits

Most of it. This is a change to what the search sees and what it is charged,
not a rewrite.

**Unchanged:**

- `pathfinder_v2.rs` - the negotiated-congestion loop, its rip-up set and its
  iteration structure.
- `congestion.rs` - history cost is orthogonal to clearance and stays as it is.
- `astar_grid.rs` - the search itself, its frontier and its scratch space.
- `variant.rs` - generate-and-rank, and the lexicographic ranking, which
  `ranking_rule_sweep` measured against six alternatives and none beat it.
- `scoring.rs` - the composite keeps its documented faults until something
  measures them; step 4's term is a search cost, not a score term.
- The whole benchmark harness, the ratchets, the bands, and the determinism
  check.

**Changed:**

- `grid.rs` gains the field and the transform. Occupancy stays during steps 1
  to 3 so every step has something to be identical to.
- `cost.rs` gains the clearance term at step 4. It is already the right shape
  for it: `RoutingCost` precomputes its tables at construction and answers by
  lookup, and the field is one more table.
- `pathfinder_v2.rs`'s termination at step 5.

**Untouched but adjacent, and named so nobody is surprised:**

- `repair.rs` has its own recorded defect - it wins by emptying the board - and
  its own next action: restart each attempt from the board the first pass
  started from. That is independent of this plan and can be taken by any fire.
- `CELL_VIA` has been defined in `grid.rs` since it was written and never set.
  If step 2 lands, it either gets set or gets deleted; carrying a constant
  nothing writes is how the last four via attempts each started by
  rediscovering that the grid cannot be told about depth.

---

## 5. What this plan is betting

One claim, stated so it can be wrong: **the seventeen dropped instruments did
not fail because their ideas were bad. They failed because a grid of whole
cells cannot hold the quantity they were all reaching for.**

If that is right, step 2 moves boards outside their bands and steps 3 and 4
compound it. If it is wrong, step 2 shows nothing and this document has cost a
day rather than a month - which is the reason step 2 is the second step and not
the fifth.

**Step 2 ran, and the claim above is wrong as stated.** Three boards moved
outside their bands the wrong way and none moved outside its band the right
way; the numbers are in step 2 above and in `docs/routing.md`. What the
measurement found is that the grid's rounding was not only a cost. It was also
an unwritten margin - 0.254mm of it, where the fab asks 0.127mm - and the
search had come to depend on it.

The claim is therefore narrowed rather than abandoned, and the narrowed version
is the one the rest of this plan should be read against: **a grid of whole
cells cannot hold that quantity, and a consumer that takes the rounding away
must put a margin back deliberately.** Steps 3 and 4 are still worth running
under it - step 4 especially, since a barrier that is non-zero before contact
carries its margin by construction. What is no longer available is the
comfortable version, where each rounding removed is a violation saved.

The alternative reading is also on the table and should be said out loud: that
negotiated congestion on a fine-pitch two-layer escape is simply the wrong
algorithm, and `qfp_fanout`'s 57-violation band is the algorithm telling us so.
Nothing in this plan refutes that. What steps 1 to 4 do is make the question
answerable, because a router whose prices are real distances is one whose
remaining failures can be attributed.

---

## Verification

The numbers in this document are quoted from `docs/routing.md` and
`docs/TRACKER.md` rather than measured here. To re-derive them:

```bash
# The six boards, their routed values, and the ratchets they are checked against
cargo test --release -p cypcb-autoroute --test benchmark_validation -- --ignored --nocapture

# Each board's noise band, which is what every acceptance decision above is read against
cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored --nocapture

# The cross-tab behind "half the introduced faults are on a pad cell"
cargo test --release -p cypcb-autoroute --test grid_vs_checker -- --ignored --nocapture

# The determinism this whole record depends on
cargo test --release -p cypcb-autoroute --test benchmark_validation is_the_router_repeatable -- --ignored
```

Last verified: 2026-08-12.
