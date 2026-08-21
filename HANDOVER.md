# HANDOVER - 2026-08-21 - CodeYourPCB, layers and the interactive editor

## Goal

Turn the viewer into something a professional can lay out a multi-layer board
in. The owner's verdict that started this run: the editor was written as if a
board were one plane.

## Where the work lives

- **Container is the source of truth.** `ssh flightcore "docker exec -u abc code-server bash -lc 'export PATH=/config/.cargo/bin:$PATH; cd /workspace/codeyourpcb && <cmd>'"`
- Local `~/szymontex/codeyourpcb` is stale (branch `main`) - it is only used as
  a scratch path for editing `docs/TRACKER.md`.
- Branch `fix/cli-check-drc`, tracking `origin/fix/cli-check-drc`,
  **58 commits ahead of main, all pushed**, tree clean at `f0a4edc`.
- `docs/TRACKER.md` in the container is the real record. Read it first.

## Editing rule that must not be broken

Fetch a file **from the container**, edit locally, `scp` back. Never overwrite
the container with a local copy without checking `md5sum` on both sides. Two
regressions this run came from editing by string offsets - prefer line numbers
for anything spanning more than one function.

## State now

### Done, pushed, gate green

The layer system, built from nothing this run:

- `50fe549` copper on another layer is not an obstacle - `checkRouteObstacles`
  compared nets and never asked about layers, in both its loops
- `c61f7e6` a click picks what you can see and prefers the layer you are on -
  `hit-test.ts` did not contain the word "layer"
- `7520c8f` + `f0a4edc` a real multi-selection, `Ctrl+A`, delete-many, proven
  end to end in a browser on a fixture board
- `1c3f7b2` saved views on `Ctrl+Tab`; `04608de` per-layer weight;
  `899f6f1` colour editing incl. inner layers; `8321786` stack draw order and
  a grey ghost mode; `9748360` silkscreen/mask/drill/edge rows;
  `4080272` the duplicated toolbar picker removed;
  `7134755` the panel rebuilt against the design canon (0 elements under 15px)

Language, same run: `df20311` `pad "A1"`, `68a08c5` `net "VBUS+"` - without
these no USB-C, BGA or edge-connector board could be written down at all.

Fab tables: `0b3a5d0`, `eb1b646`, `157495a` - every preset now says whether its
numbers came off a published page, this tool's arithmetic, or a standard it
cannot cite.

### Open, reported by the owner, in their order

1. **Net labels stack on a multi-segment trace.** `GND GND` and `GNDGND`
   printed over each other - one label per segment with no spacing check.
2. **The stack cannot be defined the way a professional tool defines it.**
   `layers N` plus a `stackup` block with a type and a thickness per layer.
   No name, material, copper weight or impedance. The owner asked for parity
   with Altium/KiCad board definition; that was **not** promised, and the gap
   above is what is actually known to be missing.

### Also open, older

- The interactive router proposes a path it then refuses rather than finding
  one it can make (owner-reported, undiagnosed).
- `cypcb export --preset` names a file convention, not a rules table - two
  identically spelled flags on one binary.
- Allocator: 41% of routing instructions in malloc/free. A rewrite of
  `route_with_blockers`, a project rather than a fire.

## Running services from this session

A Vite dev server is up in the container on **5199**, tunnelled to the laptop:

    ssh -f -N -L 5199:172.16.7.2:5199 flightcore
    firefox http://localhost:5199/

If the tunnel is dead, re-run that line. If the server is dead:
`docker exec -u abc code-server bash -lc 'cd /workspace/codeyourpcb/viewer && npx vite --host 0.0.0.0 --port 5199'`

## Active rules

- Commit every material step, conventional commits, English, no AI
  attribution, hyphens not em dashes. Move DONE/NEXT-ACTION in
  `docs/TRACKER.md` in the **same** commit. Message via file: `scp msg.txt`,
  `git commit -F`.
- **Push after a green gate** - the owner gave standing consent 2026-08-20.
- `./scripts/quality-gate.sh` before any commit touching build, viewer or
  scripts.
- **Every new test gets a mutation.** A test a mutation does not kill is empty.
  This caught three vacuous tests this run.
- Verify before saying done. The command whose output proves the claim goes in
  the commit message. Screenshots and DOM audits count; reasoning does not.
- Report in Polish: result, next step, blocker.

## Next step

Net labels. `renderer.ts` draws one per segment with no spacing check, so a
trace that bends prints its name twice within a few pixels. Read the label
drawing path first, measure the collision on a fixture, then place labels along
the whole trace rather than per segment.

## Open questions for the owner

- How far towards Altium/KiCad board definition is actually wanted? The gap is
  large and no promise was made about parity.
- The variant panel is still unwired and unwireable while the Route UI is
  hidden behind D5. Wire it, delete it, or leave it parked?
