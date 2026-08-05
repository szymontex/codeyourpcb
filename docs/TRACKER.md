# CodeYourPCB tracker - the control center

Last updated: 2026-08-05. Update after every material step: add to DONE, pull the next item into NEXT-ACTION, in the same commit as the change.

Read this file first. It is the source of truth for what is in flight and what comes next.

## Where the project stands

- Version 0.2.0-beta, ~44k lines (Rust workspace + TypeScript viewer). Last release commit `746bd63` (2026-04-15).
- GSD milestones M001-M005 are marked complete in `.gsd/STATE.md`. That status was written in March and predates the 2026-07-10 audit, which ran the code and found crash-tier defects. **Where the two disagree, the audit wins** - it executed commands, the milestone summaries did not.
- Development happens in the `code-server` container on the build host (12 cores, 10GB cap). Full clean release build: 34s. Full debug build: 20s.

## Phase map

- [x] P0 - Establish verified ground truth (2026-07-10 audit)
- [ ] P1 - Stop the bleeding: crashes, lying commands, broken fresh clone  <- current
- [ ] P2 - Green quality gate: clippy, fmt, all tests including WASM E2E
- [ ] P3 - Structural: one parser, one width formula, no orphan crates
- [ ] P4 - Roadmap features (copper pour, DSL v2 semantics, KiCad interop)
- [ ] P5 - Optimization passes (measured, per GP-002)

## Vectors (parallel branches - keep ALL moving)

### V1 - CLI and core correctness
- DONE: `cypcb check` runs real DRC (`crates/cypcb-cli/src/commands/check.rs`). Root cause of the disabled path was a corrupted 23GB `target/debug`, not the "cargo workspace dependency resolution issues" the TODO claimed. Adds `--preset` (8 fab presets) and `--no-drc`. Exit code 1 on violations, so it is CI-usable. Proof: `cypcb check examples/drc-test.cypcb` -> 9 violations, rc=1; `cargo test -p cypcb-cli` -> 7 + 11 passed.
- DONE: export no longer dies with `FootprintNotFound` on inline footprint definitions. `sync_ast_to_world` now takes the library by `&mut` and registers design footprints into it instead of into a clone it drops; `register_design`/`clear_design` keep re-syncs honest (deleted footprints disappear, shadowed built-ins come back). Also removes one full library clone per sync - per keystroke in the LSP, per hot reload in the viewer. Proof: `cypcb export examples/custom-footprint.cypcb` -> 13 files rc=0, PTH drill holds the three declared holes; `cargo test -p cypcb-world -p cypcb-cli` -> 140 + 73 + 7 + 12 passed.
- NEXT-ACTION: none - at a good stopping point. Next candidates: the four DRC checkers that are empty stubs (`trace_width.rs:53`, `solder_mask_bridge.rs:27`, `silk_clearance.rs:26`, `hole_to_hole.rs:37`) count toward "12 checkers" while doing nothing.

### V2 - Autorouter and routing quality
- DONE: removed the rip-up retry loop in `astar_improved.rs` that could never run twice (every path returned or broke on the first pass - the author had already worked this out in a comment and silenced the lint instead of deleting the loop). Took the dead `max_iterations` parameter out of two functions with it. Behavior unchanged, `clippy::never_loop` unblocked.
- DONE: all 3 failing integration tests fixed by one defect. PathFinder's rip-up called `mark_route(cell, u32::MAX)` before `clear_route(net_id)` - that marks the cell occupied and erases its owner in the same step, so the clear found nothing and every ripped-up route left a permanent wall. `RoutingGrid::clear_cells` clears the recorded cells directly, which is also `cells.len()` work instead of a full `width * height * layers` scan per rip-up. blink.cypcb: `Partial { unrouted_count: 5 }`, 23 segments, 21.0mm, score 5036.0, 123.07s -> `Complete`, 45 segments, 6 vias, 182.5mm, score 212.5, 84.40s. Integration suite 432.26s -> 86.75s, 5 passed 0 failed.
- **This defect was fixed once already.** M005-SUMMARY describes it exactly, quotes the same post-fix numbers (45 segments, 6 vias, 182.5mm) and claims a proof test `test_blink_led_zero_unrouted`. Neither the fix nor that test exists in the published history - `git log --all -S` points at `db11eb0 chore: clean repo for public release`. Treat every M005 "validated" claim as unverified until re-run.
- DONE: the CI regression gate no longer rewards giving up. It asserted `route_count > 0` plus quality thresholds, and every quality metric improves when the router abandons connections - so it passed on 1 unrouted / 7 routes and failed as soon as the router completed the board. Now asserts 0 unrouted and >= 20 routes first, with quality thresholds recalibrated against a complete solution. Verified in both directions: passes on HEAD, fails on `ded9461` with "1 unrouted connections, threshold 0".
- NEXT-ACTION: R107 "zero DRC violations from the autorouter". Measured on led_blink.kicad_pcb after the grid fix: **13 violations** (was 5 while a connection was abandoned). Classify them first - PROJECT.md claims the old 5 were grid-boundary artifacts rather than real crossings. Use `cypcb check` on the routed output to get kinds and coordinates.
- QUEUED: A* is disabled in the UI by the author's own note ("poor results, needs fundamental rewrite"). The integration suite still spends 86s in one test - profile before optimizing further.

### V3 - Build, CI and quality gates
- DONE: gate stages 1 and 2 are green for the first time. `cargo fmt --check` was failing on 30 files (rustfmt version drift, not authored noise) - reformatted the workspace. `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` was failing on 11 deny-level errors across 5 crates, including a tautological assertion (`comp.value.is_some() || true`) that made a parser test unable to fail. Proof: both commands exit 0; touched crates re-tested, 11 result lines all ok.
- DONE: `scripts/quality-gate.sh` no longer excludes `cypcb-cli` from the clippy and test stages. That exclusion is why `cypcb check` could ship as a no-op for months. Removing it surfaced one lint (`&PathBuf` parameter that wants `&Path`); with that fixed the gate covers the CLI. Proof: `cargo clippy --workspace --exclude cypcb-desktop -- -D warnings` -> Finished; `cargo test -p cypcb-cli` -> 7 + 12 passed.
- NEXT-ACTION: run `scripts/quality-gate.sh` end to end and record which of the 8 stages actually pass. Stages 4-6 and 8 (eslint, vitest, playwright, jscpd) have never been run in this environment - `viewer/node_modules` exists, so they are runnable.
- QUEUED: fresh clone does not build - `crates/cypcb-parser/grammar/src/parser.c` is gitignored (`.gitignore:19`) and `build.rs` panics without it. WASM-dependent Playwright tests only ever skip (`isWasmAvailable()` guard); `M005-VALIDATION.md` verdict is `needs-attention` for exactly this - CI needs a `wasm-pack build` step before Playwright. `cypcb check` + `cypcb export` in GitHub Actions (README lists it as Planned).

### V4 - Architecture and deduplication
- DONE: nothing this cycle.
- NEXT-ACTION: two independent `.cypcb` parsers - Rust tree-sitter (CLI, LSP) and a regex parser in `viewer/src/wasm.ts` (web). They will diverge; arguably already have. Decide the collapse strategy (WASM-export the Rust parser to the viewer) and execute.
- QUEUED: IPC-2221 trace width formula duplicated in 4 places with a divergent constant. Orphan crates with zero consumers: `cypcb-calc`, `cypcb-kicad`, `cypcb-watcher`, most of `cypcb-platform`.

### V5 - Features from the roadmap
- DONE: nothing this cycle.
- NEXT-ACTION: DSL v2 constructs parse but do nothing - no module instantiation, no import resolution, no constraint evaluation. Pick module instantiation first; it is what makes the DSL worth using over a schematic editor.
- QUEUED: copper pour / ground planes (needs a `Zone` type in the ECS model - none exists), KiCad `.kicad_pcb` export (import exists), parts engine, schematic generation, differential pairs, polygon board outline editing.

### V6 - Documentation truth
- DONE: nothing this cycle.
- NEXT-ACTION: README lists 9 features as done that are partial, broken or orphaned, and 8 as planned that have zero code. Rewrite the feature table against verified behavior, with a command next to each claim.
- QUEUED: `.gsd/REQUIREMENTS.md` never received the M005 status writeback - `STATE.md` counts "23 active, 0 validated" while REQUIREMENTS lists 14 as validated. Empty DRC checkers (`trace_width.rs:53`, `solder_mask_bridge.rs:27`, `silk_clearance.rs:26`, `hole_to_hole.rs:37`) count toward "12 checkers" while doing nothing.

### V7 - Performance (GP-002 discipline: measure, then optimize, publish before/after)
- DONE: nothing this cycle.
- NEXT-ACTION: establish baselines before touching anything - WASM binary size breakdown (`twiggy top`), autorouter benchmark composite on all 3 fixtures, render frame time on the largest example, allocation counts in the DRC hot loop.
- QUEUED: after baselines, attack in order of measured cost. Candidates: spatial index queries in DRC clearance checking, PathFinder congestion map churn, per-frame allocation in the 2D renderer, WASM size (670KB).

## Owner-decision queue (only the owner closes these)

| # | Decision | Options | Blocks |
|---|---|---|---|
| D1 | Autorouter direction | Rewrite in-house / bet on FreeRouting (needs Java 21 + external jar) / keep both | V2 beyond the clippy fix |
| D2 | Tauri desktop | Keep and fix the build (needs GTK/webkit deps documented per distro) / freeze / drop | V3 scope, README truth |
| D3 | Orphan crates | Delete `cypcb-calc`/`cypcb-watcher`/most of `cypcb-platform` / keep as planned surface | V4 cleanup |
| D4 | Default fab preset | `jlcpcb` (current) / make it mandatory in the DSL board block | V1 polish |

Route development AROUND these. Never wait on them.

## Background jobs

Check live, do not trust any snapshot in this file:

```
docker exec -u abc code-server bash -lc 'export PATH=/config/.cargo/bin:$PATH; cd /workspace/codeyourpcb && cargo test --workspace --exclude cypcb-desktop -j 12 2>&1 | tail -5'
```

## Cadence

- Never idle. Finish an item, take the next unblocked NEXT-ACTION.
- Commit and push after every material step; move DONE/NEXT-ACTION in the same commit.
- Verify before marking done: a command whose output proves the claim goes in the commit message or this file.

## Verification

Re-check the central claims of this file:

```
cd /workspace/codeyourpcb
cargo test --workspace --exclude cypcb-desktop -j 12      # test status
cargo clippy --workspace --exclude cypcb-desktop -j 12    # gate status
./target/release/cypcb check examples/drc-test.cypcb      # V1 DONE claim, expect 9 violations rc=1
git log --oneline -10                                     # what actually landed
```

Last verified: 2026-08-05.
