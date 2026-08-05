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
- DONE: `MinTraceWidthRule` does something. It was registered in the engine and returned an empty vector behind a note about trace entities arriving "in Phase 5" - they arrived long ago. Only safe to enable after the rule-table merge: the router draws at 0.127mm and the DRC preset used to demand 0.15mm, so an active width check would have flagged every autorouted trace. Measured after: led_blink unchanged at 3 violations, all clearance. The sandbox test that ended in `let _ = tw;` now asserts 1.
- NEXT-ACTION: three stubs left, in order of value. `solder_mask_bridge.rs:27` needs per-pad mask expansion modelled; `silk_clearance.rs:26` needs silkscreen geometry, which the renderer already computes; `hole_to_hole.rs:37` only checks via-to-via and skips through-hole pad drills, which is the cheapest of the three.

### V2 - Autorouter and routing quality
- DONE: removed the rip-up retry loop in `astar_improved.rs` that could never run twice (every path returned or broke on the first pass - the author had already worked this out in a comment and silenced the lint instead of deleting the loop). Took the dead `max_iterations` parameter out of two functions with it. Behavior unchanged, `clippy::never_loop` unblocked.
- DONE: all 3 failing integration tests fixed by one defect. PathFinder's rip-up called `mark_route(cell, u32::MAX)` before `clear_route(net_id)` - that marks the cell occupied and erases its owner in the same step, so the clear found nothing and every ripped-up route left a permanent wall. `RoutingGrid::clear_cells` clears the recorded cells directly, which is also `cells.len()` work instead of a full `width * height * layers` scan per rip-up. blink.cypcb: `Partial { unrouted_count: 5 }`, 23 segments, 21.0mm, score 5036.0, 123.07s -> `Complete`, 45 segments, 6 vias, 182.5mm, score 212.5, 84.40s. Integration suite 432.26s -> 86.75s, 5 passed 0 failed.
- **This defect was fixed once already.** M005-SUMMARY describes it exactly, quotes the same post-fix numbers (45 segments, 6 vias, 182.5mm) and claims a proof test `test_blink_led_zero_unrouted`. Neither the fix nor that test exists in the published history - `git log --all -S` points at `db11eb0 chore: clean repo for public release`. Treat every M005 "validated" claim as unverified until re-run.
- DONE: the CI regression gate no longer rewards giving up. It asserted `route_count > 0` plus quality thresholds, and every quality metric improves when the router abandons connections - so it passed on 1 unrouted / 7 routes and failed as soon as the router completed the board. Now asserts 0 unrouted and >= 20 routes first, with quality thresholds recalibrated against a complete solution. Verified in both directions: passes on HEAD, fails on `ded9461` with "1 unrouted connections, threshold 0".
- DONE: R107 from 13 violations to 3. `tests/drc_report.rs` (new, `--ignored`) printed every violation with kind and coordinates instead of one number, and all 13 were `clearance` - mostly a trace against the pad it connects to, at 0.00mm. Cause: `apply_routes` spawned bare `Trace`/`Via` entities, while DRC's same-net exemption queries for a `NetId` *component*. The renderer already spawned `(trace, net_id)`; KNOWLEDGE.md K012 states the rule. Same routing before and after (0 unrouted, 22 routes, 79.61mm): 13 violations / composite 13542.6 -> 3 violations / composite 3542.6. Ratchets tightened to the measured values.
- DONE: classified the remaining 3. They are **real inter-net violations**, not grid artifacts - `R1 ↔ trace 'GND'` at 0.07mm (needs 0.15), `trace 'SW_OUT' ↔ trace 'GND'` at 0.00mm and `trace 'SW_OUT' ↔ via 'GND'` at 0.00mm. `.gsd/PROJECT.md` claimed the opposite and has been corrected.
- **Failed experiment, do not repeat as-is:** blocking any cell within `clearance_cells` of foreign copper during path expansion. The grid bloats pads, zones and locked traces by the clearance radius at build time, but routes marked while routing occupy their exact cells only, so two nets in neighbouring cells overlap physically (resolution is `min_clearance / 2` = 0.075mm, trace width 0.127mm). Adding the check to `pathfinder.rs` alone changed nothing - PathFinder has its own expansion loop in `find_path_congestion_augmented`. Adding it there too did fire (22 -> 20 routes, 1 -> 2 vias) but made things **worse**: 3 -> 4 violations, composite 3542.6 -> 4043.6. Reverted.
- NEXT-ACTION: the three violations have different shapes, so attack them separately. The 0.07mm pad case looks like `populate_pads` bloating by `clearance_cells` without accounting for the routed trace's own half-width. Confirm by computing the expected gap for the JLCPCB numbers before changing code.
- QUEUED: the trace-to-trace and trace-to-via overlaps at 0.00mm. Check whether they come from diagonal moves cutting the corner between two marked cells, and whether `optimize_vias(..., &[], ...)` - which is handed an empty slice of foreign segments - can move a via onto another net.
- QUEUED: A* is disabled in the UI by the author's own note ("poor results, needs fundamental rewrite"). The integration suite still spends 86s in one test - profile before optimizing further.

### V3 - Build, CI and quality gates
- DONE: gate stages 1 and 2 are green for the first time. `cargo fmt --check` was failing on 30 files (rustfmt version drift, not authored noise) - reformatted the workspace. `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` was failing on 11 deny-level errors across 5 crates, including a tautological assertion (`comp.value.is_some() || true`) that made a parser test unable to fail. Proof: both commands exit 0; touched crates re-tested, 11 result lines all ok.
- DONE: `scripts/quality-gate.sh` no longer excludes `cypcb-cli` from the clippy and test stages. That exclusion is why `cypcb check` could ship as a no-op for months. Removing it surfaced one lint (`&PathBuf` parameter that wants `&Path`); with that fixed the gate covers the CLI. Proof: `cargo clippy --workspace --exclude cypcb-desktop -- -D warnings` -> Finished; `cargo test -p cypcb-cli` -> 7 + 12 passed.
- DONE: ran the JavaScript-side stages for the first time and cleared two. Stage 4 (eslint) had 31 errors - 26 unused symbols, mostly dead functions and the imports feeding them, now deleted; 5 never-reassigned bindings. Stage 8 (jscpd) had one clone: `fetch3DModel` repeated the body of `fetch3DModelByUuid`. Both green, `tsc --noEmit` clean too.
- DONE: stage 5 (vitest) green, 149 of 149. Two stale-value bugs, not test bugs. Layer colours lived in three tables and the settings defaults - the ones feeding the Preferences colour pickers - carried an older palette, so the dialog showed five colours the board never draws; `render-config.ts` and `settings.ts` now both read `LAYER_COLORS` and the test asserts against it. The routing tests expected angle snap off while the code deliberately follows KiCad and starts it on.
- DONE: stage 6 starts. Vite was dying on `viewer/svg-pcb` and `viewer/circuitron` - reference checkouts of other projects, committed as submodule pointers with no `.gitmodules` entry, so a fresh clone gets empty directories and `git submodule update` fails. Untracked and ignored, Vite's scan pinned to this app. Then chromium needed `libnspr4.so`; installed with `npx playwright install-deps chromium` inside the container (container filesystem, not the image - a recreate needs it again). Also untracked 42MB of `viewer/test-results`.
- DONE: triaged the Playwright failures instead of fixing them one by one. The largest cause is not a bug: **the entire Route UI is hidden in `index.html`** behind `<span class="toolbar-anchor" style="display:none">` with the comment "Autorouter disabled - needs fundamental rewrite". A DOM probe against the running app confirmed `.tb-route-group` measures 0x0, which is why every `#route-btn` click timed out, and `#tuning-toggle` does not exist at all. `tuning-panel` and `variant-panel` now skip with that reason written above the describe. Also confirmed `#theme-toggle` is `display: none` by design, so the `ui-architecture` expectation of it is stale.
- **Gate status, measured 2026-08-05:** 1 fmt PASS, 2 clippy PASS, 3 cargo test PASS, 4 eslint PASS, 5 vitest PASS, 6 playwright **66 passed / 31 failed / 14 skipped** (was 68/43/0, and 0 attempted before the web server was fixed), 7 autorouter benchmark PASS, 8 jscpd PASS.
- DONE: `project-manager.spec.ts` from 8 failures to 2, plus `ui-architecture` fixed. A DOM probe against the running app showed the specs, not the app, were behind: 5 template cards ship (4 templates plus blank) where the tests expected 4 and `templateCount` 3; the recent-files section hides itself when empty instead of rendering a "No recent files" message; the import button reads "Import File" since "Open" was reserved for the workspace list; display names strip the `.cypcb` extension. Every one of those is in CLAUDE.md - the tests predate it.
- DONE: the editor cluster was one app bug wearing two masks. `ensureEditorReady()` guarded on a boolean set only after its await, so the toolbar click and the idle preload both ran `initEditor()` on the same container - two Monaco editors, which is exactly the `.monaco-editor` strict-mode violation, plus a race over the panel's visibility. On top of that the preload undid an "auto-show" by testing for a `hidden` class the container never carries, so the editor opened by itself a few seconds after every load. Memoized the init promise, dropped the bogus undo. `editor.spec.ts` fully green.
- DONE: `Ctrl+Shift+T` never toggled the theme. Rewriting two specs off the hidden `#theme-toggle` onto the documented shortcut exposed it: the handler matched `e.key === 'T'` only, and the browser reports `t` or `T` depending on layout and input path. Every other letter shortcut in `main.ts` already accepted both. The old specs could not have caught it - one asserted only that `data-theme` was still light or dark, which holds when nothing happens.
- DONE: the JLCPCB search cluster. The app scored the whole query as one string, so "0805 10k" - a package and a value, never in one field - matched nothing; terms are scored separately now, and attributes are read from the older `extra` shape the scorer had been ignoring. The specs were measuring through a mock that had drifted from the API: no CORS header on cross-origin fulfillments (browser dropped every response), one fixture answered for all 34 category endpoints (panel hit its 20-result limit), LCSC codes pinned to positions, and a debounce assertion of "exactly 1 request" from when a search hit one endpoint.
- DONE: the last two unblocked failures were both real defects. With ten or more recent files the project manager's template cards sat at `top: -205` with `scrollTop: 0` - `align-items: center` on a scrolling container puts the top of taller-than-viewport content outside the scrollable area, so the templates were unreachable by a user, not just by a test; `margin: auto` on the child fixes it. And the E2E mocks only intercepted `easyeda.com`, while the dev build talks to the Vite proxy paths, so those requests were leaving the machine for real on every run.
- **Gate status, measured 2026-08-05:** stage 6 playwright **94 passed / 3 failed / 14 skipped** (0 attempted this morning, then 68/43, 66/31, 73/24, 86/11, 90/7, 92/5). Every remaining failure is blocked on D5.
- NEXT-ACTION: nothing unblocked in this vector. The three `benchmark-screenshots` need the Route UI back (D5). Next fires should move to V1's empty DRC checkers or V7's baselines, both untouched.
- QUEUED: put the browser system deps in the image or a CI setup step. Audit the specs for assertions that cannot fail.
- QUEUED: audit the specs for assertions that cannot fail, like the theme one. That pattern hid a dead shortcut for months.
- QUEUED: three `benchmark-screenshots` failures that route boards through the hidden Route UI (blocked on D5). `populateRecentFiles` renders every stored entry while `addRecentFile` caps at 10, so a hand-seeded list shows more than the UI ever writes.
- QUEUED: put the browser system deps somewhere durable (container image or CI setup step) so stage 6 does not break on the next container recreate.
- QUEUED: fresh clone does not build - `crates/cypcb-parser/grammar/src/parser.c` is gitignored (`.gitignore:19`) and `build.rs` panics without it. WASM-dependent Playwright tests only ever skip (`isWasmAvailable()` guard); `M005-VALIDATION.md` verdict is `needs-attention` for exactly this - CI needs a `wasm-pack build` step before Playwright. `cypcb check` + `cypcb export` in GitHub Actions (README lists it as Planned).

### V4 - Architecture and deduplication
- DONE: manufacturer rules had two copies that disagreed - the router routed JLCPCB to 0.127mm clearance while the checker demanded 0.15mm, so a correctly routed board failed its own DRC. `cypcb-drc` already depended on `cypcb-rules` without using it; the seven fab presets are now one line each on top of `DesignRules::from_constraints`, and `drc_presets_do_not_diverge_from_routing_constraints` walks every pair so they cannot drift again. Proof: `cargo test -p cypcb-drc` -> 113 + 31 + 23 passed, consumers green, gate clean.
- DONE: settled the question the second parser rests on. `cypcb-parser`'s manifest says tree-sitter is "not WASM compatible"; it is - `cargo build -p cypcb-parser --target wasm32-unknown-unknown` succeeds, and so does the whole render crate with `--features native`. `PcbEngine::load_source` now lives in the wasm-bindgen impl, so the browser can reach it. Measured cost, wasm-pack release: 702,357 bytes with the parser absent against 804,520 with it exported, about 100KB.
- NEXT-ACTION: retire the JS parser in one step - flip `build-wasm.sh` to `--features native`, point `WasmPcbEngineAdapter.load_source` at the engine, delete the regex parser (roughly 600 lines around `viewer/src/wasm.ts:565-700`) and run vitest plus the WASM Playwright specs. Flipping the build alone would ship both parsers, so it has to be one change.
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
| D5 | Route UI hidden in the toolbar | Unhide it / keep it hidden until the router is rewritten | 13 E2E tests, and whether users can autoroute at all. It was hidden because results were poor. That premise has moved: PathFinder now completes led_blink with 0 unrouted, 45 segments and 3 DRC violations, against 5 unrouted and a board it gave up on. Worth a look before the next release. |

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
