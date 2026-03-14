# S07: Polish, Bugs & Verification — Research

**Date:** 2026-03-13

## Summary

S07 is the final slice of M003 — a cleanup pass that fixes quality gate failures, resolves follow-up items from S01-S06, adds version branding, and verifies the full milestone Definition of Done. The codebase is in good shape: 93/93 E2E tests pass, 127/127 unit tests pass, all 6 prior slices verified. Two quality gate stages currently fail (ESLint unused import, jscpd duplicate code), and there are a handful of polish items surfaced by previous slices.

The work is well-scoped and low-risk. No new features — just fixing what's broken, eliminating code smells, verifying bug fixes, and ensuring the 8-stage quality gate passes cleanly. The heaviest lift is the jscpd duplicate refactoring in `wasm.ts` (trace-building logic shared between `WasmPcbEngineAdapter` and `MockPcbEngine`). Everything else is surgical.

The milestone DOD has 10 items. Most are already satisfied by S01-S06. S07's job is to verify them end-to-end and close the remaining gaps: quality gate green, version naming, and explicit verification of M002 bug fixes.

## Recommendation

Execute as a single slice with 2-3 focused tasks:

1. **T01: Quality gate fixes** — ESLint unused import, jscpd duplicate refactoring in wasm.ts, verify all 8 stages pass
2. **T02: Polish & version naming** — beta label in UI/package versions, S06 error state improvement, errors.spec.ts flake fix
3. **T03: Milestone DOD verification** — run full quality gate, cross-check each DOD item, add any missing E2E assertions for M002 bug fixes (theme single-click in prefs modal)

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Trace segment building from flat array | Extract shared helper from existing duplicate code in wasm.ts | Both `WasmPcbEngineAdapter` and `MockPcbEngine` build `TraceSegmentInfo[]` identically — extract once, call twice |
| Theme single-click verification | Existing theme.spec.ts pattern (click → assert data-theme change) | Already proven pattern, just needs extension to prefs modal button |
| Quality gate execution | `scripts/quality-gate.sh` | Already runs all 8 stages in sequence with fail-fast |

## Existing Code and Patterns

- `viewer/src/wasm.ts` — Lines 630-638 (`WasmPcbEngineAdapter.add_trace` JS fallback) and lines 762-773 (`MockPcbEngine.add_trace`) contain identical trace-segment-building + layer-normalization logic. Extract to shared helper function.
- `viewer/src/main.ts:26` — Imports `showSearchPanel` from jlcpcb-panel but never uses it. Remove from import.
- `viewer/src/jlcpcb.ts` — `searchComponents()` catches all errors and returns `[]`. S06 follow-up suggests throwing on HTTP errors for distinct UI error state. The panel in `jlcpcb-panel.ts` already has an error CSS class path but it's unreachable.
- `viewer/e2e/errors.spec.ts:102` — S01 flagged a pre-existing flake. Test "app handles invalid input without crashing" is actually straightforward — likely stable now with PM dismissal in beforeEach.
- `viewer/e2e/theme.spec.ts` — Tests theme toggle via toolbar button but not via Preferences modal button (`#prefs-theme-btn`). Gap for M002 bug fix verification.
- `viewer/.jscpd.json` — Threshold is 0 (zero tolerance). The 1 clone found (11 lines, 128 tokens) in wasm.ts must be eliminated.
- `scripts/quality-gate.sh` — 8 stages. Stages 1-3 (Rust) pass. Stage 4 (ESLint) fails on unused import. Stages 5-6 (vitest/playwright) pass. Stage 7 (autorouter benchmark) passes. Stage 8 (jscpd) fails on wasm.ts duplicate.

## Constraints

- **Quality gate must pass all 8 stages** — this is a hard DOD requirement. Currently 2 stages fail.
- **No Rust changes needed** — all issues are in the viewer TypeScript code. Rust stages (fmt, clippy, test) already green.
- **Existing E2E tests must not regress** — 93/93 currently pass. Any refactoring must keep them green.
- **PM overlay blocks canvas** — all E2E tests interacting with canvas/editor must dismiss PM via `__loadBoard()` in beforeEach (pattern established in S05).
- **Version strings in 3 places** — `viewer/package.json`, `Cargo.toml` (workspace), `src-tauri/tauri.conf.json`. All currently `0.1.0`.
- **jscpd threshold is 0** — zero tolerance for duplicates >10 lines in `viewer/src/`. Must eliminate the clone, not raise the threshold.

## Common Pitfalls

- **Refactoring wasm.ts shared code breaks trace mutations** — The duplicate code is in a critical path (add_trace for both WASM adapter and Mock engine). Vitest routing tests + Playwright routing-ux tests will catch regressions, but test both engines after extraction.
- **Version bump format matters for Tauri updater** — Tauri expects semver. `0.1.0-beta` is valid semver but verify Tauri's updater endpoint format isn't broken by pre-release suffix.
- **ESLint fix is trivial but easy to miss on re-check** — Simply remove `showSearchPanel` from the import destructuring. Don't accidentally remove `hideSearchPanel` or `toggleSearchPanel` which are used.
- **JLCPCB error state improvement scope creep** — Making `searchComponents` throw on HTTP errors requires changes to both `jlcpcb.ts` and `jlcpcb-panel.ts`. Keep it minimal — distinguish error from empty, don't build elaborate retry UX.

## Open Risks

- **errors.spec.ts flake** — S01 flagged this but didn't specify the exact failure mode. The test itself looks stable with the current PM dismissal pattern. May have been fixed as a side effect of S05's `__loadBoard` beforeEach pattern. Run it 5x to confirm stability before marking fixed.
- **Tauri version string compatibility** — Adding `-beta` suffix to tauri.conf.json version may affect the updater endpoint URL pattern. Low risk since desktop builds are excluded from CI, but worth noting.
- **jscpd after refactoring may find new clones** — Extracting the shared helper could reveal other borderline duplicates that were previously below the 10-line threshold. Run jscpd after refactoring to confirm.

## Current State Snapshot

| Metric | Value |
|--------|-------|
| E2E tests | 93/93 pass (14 spec files) |
| Unit tests | 127/127 pass (11 test files) |
| Quality gate | Fails at stage 4 (ESLint) and stage 8 (jscpd) |
| ESLint errors | 1 (unused import `showSearchPanel` in main.ts) |
| TypeScript errors | 1 (same unused import, TS6133) |
| jscpd clones | 1 (11 lines in wasm.ts, 0.18% duplication) |
| Rust fmt/clippy/test | All green |
| Version strings | `0.1.0` everywhere (no beta label yet) |
| M002 bugs status | All 3 fixed (theme single-click, grid toggle, fit icon) but prefs-theme button lacks E2E test |

## Milestone DOD Checklist vs Current State

| DOD Item | Status | S07 Action |
|----------|--------|------------|
| 2D board view passes visual comparison | ✅ S01 delivered + 8 E2E tests | Verify via quality gate |
| 3D view renders traces, components, vias | ✅ S02 delivered + 6 E2E tests | Verify via quality gate |
| Routing flow verified by E2E test | ✅ S03 delivered + 6 E2E tests | Verify via quality gate |
| Preferences panel + unit change persist | ✅ S04 delivered + 15 E2E tests | Verify via quality gate |
| Project manager lists recent files + templates | ✅ S05 delivered + 14 E2E tests | Verify via quality gate |
| JLCPCB search returns results | ✅ S06 delivered + 6 E2E tests | Verify via quality gate |
| Toolbar clean, View menu has layer/grid/ratsnest | ✅ S04 delivered | Verify via quality gate |
| All M002 UI bugs verified fixed | ⚠️ Fixed but prefs-theme needs E2E | Add prefs-theme single-click test |
| E2E test suite extended | ✅ 93 tests across 14 files | Quality gate verification |
| 8-stage quality gate passes | ❌ 2 stages fail | Fix ESLint + jscpd |

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Playwright | currents-dev/playwright-best-practices-skill@playwright-best-practices | available (9.3K installs) — not needed, existing patterns are solid |
| Vite | antfu/skills@vite | available (9.3K installs) — not needed, no Vite config changes |

## Sources

- Quality gate failures diagnosed via `bash scripts/quality-gate.sh` (local run)
- jscpd clone location identified via `cd viewer && npx jscpd --exitCode 1` (wasm.ts:630-638 ↔ wasm.ts:762-773)
- S01-S06 follow-up items collected from slice summaries in `.gsd/milestones/M003/slices/*/S*-SUMMARY.md`
- Milestone DOD from `.gsd/milestones/M003/M003-ROADMAP.md`
