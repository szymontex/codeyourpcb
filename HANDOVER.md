# HANDOVER - 2026-08-21 - CodeYourPCB, layers and the interactive editor

## Goal

Turn the viewer into something a professional can lay out a multi-layer board
in. The owner's verdict that started this run: the editor was written as if a
board were one plane.

## Where the work lives

- **Container is the source of truth.** `ssh flightcore "docker exec -u abc code-server bash -lc 'export PATH=/config/.cargo/bin:$PATH; cd /workspace/codeyourpcb && <cmd>'"`
- Local `~/szymontex/codeyourpcb` is stale (branch `main`). It is only a
  scratch path for editing `docs/TRACKER.md`.
- Branch `fix/cli-check-drc`, tracking origin, **61 commits ahead of main, all
  pushed**, tree clean at `dc37137`.
- `docs/TRACKER.md` in the container is the real record. Read it first.

## Editing rules that must not be broken

- Fetch from the container, edit locally, `scp` back, check `md5sum` on both
  sides.
- **Cuts spanning more than one function go by line number, not string
  offset.** Two regressions this run came from `s.index()` matching a call
  inside a different function than the one being aimed at.
- Every new test gets a mutation. A test a mutation does not kill is empty -
  this caught four vacuous tests this run.
- Verify before saying done. The command whose output proves the claim goes in
  the commit message. Screenshots and DOM audits count; reasoning does not.

## The pattern worth knowing before touching anything

**Three files needed the same fix this run**, and a fourth will:
`checkRouteObstacles` in `routing.ts`, `hit-test.ts`, and `dodge.ts`. Each
asked which **net** a piece of copper was on and never which **layer**. The
Rust core has carried copper masks and a grid per layer since it was written;
the TypeScript editor was not written with layers in mind at all. When
something in the editor behaves as if the board were one plane, this is why.

Rule that came out of it: a through-hole pad is on every layer and must never
be skipped by a layer filter - it is a drilled hole, and copper cannot pass
through it on any layer.

## State now

### The owner's five reported defects - all closed

- `50fe549` copper on another layer is not an obstacle
- `c61f7e6` a click picks what you can see, prefers the layer you are on
- `7520c8f` + `f0a4edc` real multi-selection, `Ctrl+A`, delete-many, proven in
  a browser on a fixture board
- `c1010c6` net names printed once - an off-by-one drew every label twice, and
  nothing compared placements across segments
- `dc37137` the dodge avoids only copper it could hit, and reports what is
  still in the way rather than what was in the way before it ran

### The layer system, built this run

`1c3f7b2` saved views on `Ctrl+Tab` · `04608de` per-layer weight ·
`899f6f1` colour editing including the inner layers · `8321786` stack draw
order and a grey ghost mode · `9748360` silkscreen/mask/drill/edge rows ·
`4080272` the duplicated toolbar picker removed · `7134755` the panel rebuilt
against the design canon (0 elements under 15px, 0 overflowing)

### Language and fab tables, same run

`df20311` `pad "A1"` · `68a08c5` `net "VBUS+"` - without these no USB-C, BGA or
edge-connector board could be written down. `0b3a5d0`, `eb1b646`, `157495a` -
every preset now says whether its numbers came off a published page, this
tool's own arithmetic, or a standard it cannot cite.

## Blocked on the owner

**The board stack.** The language has `layers N` and a `stackup` block with a
type and a thickness per layer. It has no name, material, copper weight or
impedance. The owner asked for parity with Altium and KiCad board definition
and **no promise was made**. The question put to him, still unanswered: which
of those fields does his own work use? The field list decides whether this
becomes a tool or a table nobody fills in. Do not guess it.

## Licence to rewrite - stated by the owner 2026-08-21

"jak trzeba zrobic rewrite to sie zrobi, bo ten projekt i tak nie ma ani jednej
wersji stable a nawet alpha, tymbardziej beta."

There is no released version, so **backward compatibility is not a
constraint**. A file format change, a grammar change, a rewritten module or a
rewritten crate are all on the table when the current shape is the thing in the
way. This removes the usual reason to bolt a fix onto a structure that cannot
hold it - and three files needing the same layer fix this run is exactly that
kind of signal.

What does not change: the work is still measured, tested and mutated before it
is called done. A rewrite is licence to change the shape, not licence to skip
the proof.

## Unblocked work, largest first

1. **The allocator.** 41% of routing instructions are in malloc/free, measured
   with callgrind - the largest single number this project has measured about
   itself. The fix is named: rewrite `route_with_blockers` so the search owns
   its scratch space instead of the `pathfinding` crate. A project rather than
   a single pass, and now explicitly sanctioned by the licence above.
2. **Genuine KiCad fixtures as a re-baseline.** Every ratchet, every noise band
   and every table in `docs/routing.md` is measured against the current
   fixtures; converting them means re-measuring all of it in one commit.
3. **`cypcb export --preset`** names a file convention while the same flag on
   every other command names a design-rule table.
4. **The variant panel** is unwired and unwireable while the Route UI is hidden
   behind D5. Wire, delete, or leave parked - the owner's call.

## Running services from this session

Vite dev server in the container on **5199**, tunnelled to the laptop:

    ssh -f -N -L 5199:172.16.7.2:5199 flightcore
    firefox http://localhost:5199/

If the tunnel died, re-run that line. If the server died:
`docker exec -u abc code-server bash -lc 'cd /workspace/codeyourpcb/viewer && npx vite --host 0.0.0.0 --port 5199'`

No example this project ships carries routed copper - `grep -c '^trace ' examples/*.cypcb`
returns 0 for all of them. To see copper in the app, load a fixture through
`window.__loadBoard(src)`, the way `e2e/a-selection-can-be-deleted.spec.ts`
does.

## Active rules

- Commit every material step: conventional commits, English, no AI
  attribution, hyphens not em dashes. Move DONE/NEXT-ACTION in
  `docs/TRACKER.md` in the **same** commit. Message via file: `scp msg.txt`,
  `git commit -F`.
- **Push after a green gate** - standing consent given 2026-08-20.
- `./scripts/quality-gate.sh` before any commit touching build, viewer or
  scripts. It has caught a startup crash, a clippy doc lint and an undocumented
  shortcut this run - it is not a formality.
- New keyboard shortcuts must be listed in `CLAUDE.md` or the shortcut registry
  test fails.
- Report in Polish: result, next step, blocker.

## Next step

The owner's defect list is empty and the stack is blocked on his field list, so
take **the allocator**: measure first with callgrind on `shift_driver` to
confirm the 41% still stands, then rewrite `route_with_blockers` to reuse its
frontier and visited set across nets. The acceptance bar is the one this vector
already uses - a change counts only when it moves a board outside that board's
own noise band, and the six ratchets in `benchmark_validation` must not move
for a change that is meant to be pure speed.

Ask the owner about the stack fields when he next appears.
