# S07: E2E Test Suite & Quality Gates — Research

**Date:** 2026-03-13

## Summary

S07 covers four distinct work areas: (1) E2E browser tests covering every user action, (2) linter compliance (clippy, ESLint, rustfmt), (3) input sanitization and error message quality, and (4) web reliability (WASM load failure recovery, WebSocket reconnection, malformed `.cypcb` handling). There is currently **zero frontend test infrastructure** — no test runner, no E2E framework, no ESLint config. The Rust side has 864 tests but 3 are failing (2 export/filesystem, 1 world/sync), 81 clippy warnings in cypcb-parser alone (122 total across workspace), and 680 rustfmt diffs. This is primarily an infrastructure-creation and cleanup slice, not a feature slice.

The biggest risk is scope — "every user action" across a complex EDA app is a massive test surface. The approach should be: Playwright for E2E (browser tests with screenshots and click simulation), Vitest for unit tests of pure TS functions (viewport math, hit-testing, undo stack), ESLint flat config for TypeScript linting, then systematic clippy/rustfmt cleanup. Web reliability testing (WASM failure, WS reconnect, malformed files) fits naturally into the E2E suite as specific scenarios.

## Recommendation

**Playwright** for E2E tests (browser-based, screenshot capture, click simulation). It's the industry standard, has first-class Vite support, and the roadmap explicitly calls for "screenshots and click simulation." **Vitest** for unit tests of pure TypeScript modules (viewport, hit-test, undo stack, url-state). ESLint v9 flat config with `@typescript-eslint` for TS linting.

Split into tasks roughly as:
1. **T01: Linter compliance** — `cargo fmt`, fix all clippy warnings, set up ESLint, fix TS lint issues. This is mechanical but large (680 fmt diffs, 122 clippy warnings). Doing this first avoids merge conflicts with later work.
2. **T02: Test infrastructure + unit tests** — Install Playwright + Vitest, configure both, write Vitest unit tests for pure modules (viewport, hit-test, undo, url-state, file-picker utilities).
3. **T03: E2E test suite** — Playwright tests covering all user actions: load app, edit code, render board, toggle layers, 3D view, routing, undo/redo, keyboard shortcuts, theme toggle, file open/share, error panel, DRC violations. With screenshot capture on failure.
4. **T04: Input sanitization, error messages, web reliability** — Fix innerHTML XSS in error panel, test malformed `.cypcb` handling, WASM load failure recovery, WS disconnect/reconnect, edge cases.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| E2E browser testing | Playwright (`@playwright/test`) | Screenshots, click simulation, cross-browser, Vite integration. Explicitly matches roadmap requirement. |
| TS unit testing | Vitest | Shares Vite config, fast, native ESM/TS support, no extra transform config needed. |
| TS linting | ESLint v9 + `@typescript-eslint` | Industry standard. No config exists currently — needs flat config setup. |
| Code duplication detection | `cargo clippy` + `jscpd` (or manual review) | Roadmap mentions "zero code duplication above threshold." `jscpd` can scan both Rust and TS. |
| Rust autofix | `cargo clippy --fix` + `cargo fmt` | Can auto-fix most of the 680 fmt diffs and many clippy suggestions mechanically. |

## Existing Code and Patterns

- `viewer/src/main.ts` (1591 lines) — Main entry point, all UI event listeners, keyboard shortcuts, WASM loading, WebSocket connection. **This is the primary E2E test target.** All user actions are wired here.
- `viewer/src/interaction.ts` (471 lines) — Mouse interaction handlers (zoom, pan, select, resize drag, routing). Pure state machine pattern — good candidate for Vitest unit tests.
- `viewer/src/viewport.ts` (118 lines) — Pure math functions (`worldToScreen`, `screenToWorld`, `zoomAtPoint`, `pan`, `fitBoard`). **Ideal Vitest target** — no DOM deps, pure functions.
- `viewer/src/undo.ts` (308 lines) — `UndoStack` class with `BoardCommand` interface. Pure logic, no DOM deps. **Ideal Vitest target.**
- `viewer/src/hit-test.ts` (98 lines) — `hitTestTrace()` function with geometric calculations. **Pure function, easy to unit test.**
- `viewer/src/url-state.ts` (39 lines) — URL encode/decode for view state. **Trivially testable.**
- `viewer/src/wasm.ts` (955 lines) — WASM loading, mock engine fallback, `WasmPcbEngineAdapter`. Contains `parseSource()` JS parser. Mock engine pattern enables testing without WASM.
- `viewer/src/renderer.ts` (848 lines) — Canvas 2D renderer. Side-effect heavy, tests via E2E screenshots.
- `viewer/src/renderer3d.ts` (901 lines) — Three.js 3D renderer. WebGL-dependent, test via E2E toggle + screenshot.
- `viewer/src/routing.ts` (433 lines) — Manual routing state machine. Mix of pure logic and engine calls.
- `viewer/test-wasm-integration.mjs` — Existing Node.js integration test for WASM engine. Can serve as pattern for WASM-level tests.
- `examples/*.cypcb` — 13 example files including `invalid.cypcb` and `unknown_keyword.cypcb`. **Use as test fixtures.**
- `crates/cypcb-export/src/job.rs` — 2 failing tests (`test_export_duration_tracked`, `test_export_result_has_files`) due to temp dir filesystem issues. Need fixing.
- `crates/cypcb-world/src/sync.rs` — 1 failing test (`test_sync_named_pin`). Pre-existing, noted in S05 summary.

## Constraints

- **No X server in CI** — Playwright must run headless. 3D (WebGL) tests may need `--ignore-gpu-blocklist` Chromium flag or skip visual assertions for WebGL content.
- **WASM build required** — E2E tests need `viewer/pkg/` populated. Build step: `./viewer/build-wasm.sh` before tests.
- **Dev server on port 4321** — Vite dev server configured at `http://localhost:4321`. Playwright `webServer` config should launch this.
- **Monaco Editor** — Heavy dependency loaded in web worker. May cause timing issues in E2E tests; need explicit waits for editor readiness.
- **Three.js lazy-loaded** — 3D renderer only loads on first toggle click. Tests must click 3D button and wait for dynamic import.
- **No ESLint config exists** — Must create from scratch. ESLint v9 flat config format since v10 is now the default.
- **clippy errors block `-- -D warnings`** — Must fix all 122 warnings before clippy quality gate can be enforced.
- **680 rustfmt diffs** — `cargo fmt` will reformat aggressively. Should be done in a single dedicated commit.
- **innerHTML XSS** — `errorList.innerHTML` in main.ts inserts DRC violation text (user-controllable via `.cypcb` content) without HTML escaping. Must fix.
- **`cargo clippy --fix`** — Can auto-fix many issues but requires `--allow-dirty` in a non-clean workspace. Apply selectively per crate.
- **Desktop crates excluded** — `cypcb-cli` and `cypcb-desktop` (Tauri) can't compile in this environment (missing system deps: pkg-config, gio-2.0). Exclude from quality gates.
- **tsconfig strict mode already enabled** — `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch` all true. `tsc --noEmit` passes clean currently.

## Common Pitfalls

- **Playwright + WASM timing** — WASM loading is async and shows "Loading WASM..." status. Tests must wait for "Ready (WASM)" or "Ready (Mock)" before interacting. Use `page.waitForSelector('#status-text:has-text("Ready")')`.
- **Canvas-based interactions** — PCB viewer uses `<canvas>` elements, not DOM elements. Click simulation needs coordinate-based actions (`page.click('#pcb-canvas', { position: { x, y } })`), not selector-based clicks. Screenshot comparison is the primary assertion method for visual state.
- **Monaco editor setup time** — Monaco worker initialization takes 1-3s. E2E tests that type in the editor need to wait for editor readiness before sending keystrokes.
- **WebGL context limits** — Running many 3D tests sequentially may exhaust WebGL contexts. Consider `--workers=1` for 3D test suite or explicit context disposal between tests.
- **rustfmt + clippy ordering** — Run `cargo fmt` first, then `cargo clippy --fix`, then manual fixes. Reverse order wastes effort since fmt changes lines.
- **clippy auto-fix scope** — `cargo clippy --fix` can fix ~50% of warnings (unused imports, Option::map patterns, if-let). The rest (ptr_arg, too-many-args) need manual intervention.
- **Mock engine fallback** — When WASM fails to load, the viewer falls back to a mock engine. E2E tests should verify BOTH paths: real WASM and graceful fallback.

## Open Risks

- **Scope creep on "every user action"** — The app has 25+ distinct interaction points (buttons, keyboard shortcuts, mouse gestures, drag operations). Full coverage may take 3-4 tasks instead of the planned scope. Need to prioritize: core flows first, edge cases second.
- **WebGL in headless Chromium** — Three.js rendering in headless mode may differ from headed. Screenshot assertions for 3D content may need generous tolerances or be assertion-only (element exists, no crash) without pixel comparison.
- **Existing test failures** — 3 Rust tests already fail. Fixing them is part of "quality gates" but root-causing the export tests (filesystem) and world test (sync) may reveal deeper issues.
- **clippy ptr_arg in parser** — The `&mut Vec<ParseError>` pattern is used throughout the parser converter methods (14 instances). Changing to `&mut [ParseError]` is incorrect since these methods `push()` to the Vec. Correct fix is `#[allow(clippy::ptr_arg)]` or refactoring to return errors instead of push. Either way, this needs careful handling.
- **Code duplication metric** — Roadmap says "zero code duplication above threshold" but doesn't define the threshold. Need to decide: what tool, what threshold, what to measure (Rust only? TS too?).

## User Actions Inventory (E2E Test Coverage Target)

### Core Flows
1. App loads → WASM initializes → "Ready" status shown
2. Editor toggle (Ctrl+E or button) → editor panel shows/hides
3. Type `.cypcb` code in editor → board renders in viewer
4. Open file (button) → file picker → load → render
5. Save file (Ctrl+S, web) → File System Access API
6. Share design (Ctrl+Shift+S) → URL with encoded state

### Board Interaction
7. Pan (middle-click drag / Ctrl+left-click drag)
8. Zoom (scroll wheel, centered on cursor)
9. Fit to board (F key or button)
10. Select component (click on canvas)
11. Rotate component (R key)
12. Delete trace (Delete/Backspace)
13. Resize board (drag handles)

### Routing
14. Start route (click pad) → routing mode
15. Route with waypoints (click) → angle-snapped segments
16. Complete route (click target pad) → trace added
17. Cancel route (Escape)
18. Autoroute (button) → autorouter runs → traces appear

### Layers & Views
19. Toggle Top/Bottom layer visibility (checkboxes)
20. Toggle ratsnest (checkbox)
21. Toggle grid snap (checkbox)
22. Toggle 3D view (button or '3' key)
23. 3D orbit/zoom/pan (in 3D mode)

### Undo/Redo
24. Undo (Ctrl+Z) → last action reverted
25. Redo (Ctrl+Shift+Z / Ctrl+Y) → action re-applied

### Error Handling
26. DRC violations → error badge → error panel → click to locate
27. Parse errors in editor → Monaco diagnostics inline
28. Invalid .cypcb file → graceful error display
29. WASM load failure → mock engine fallback → "Ready (Mock)"

### Theme
30. Toggle theme (Ctrl+Shift+T or button) → light↔dark
31. Theme persists across reload (localStorage)

### Web Reliability
32. WebSocket disconnect → auto-reconnect after 2s
33. Malformed .cypcb → parser error, no crash
34. URL state roundtrip → encode → decode → same view

## Requirements Mapping

This slice primarily supports:
- **EDIT-07** (undo/redo) — verify via E2E tests
- **UI-01/02/03/04** (dark/light mode, OS preference, manual toggle) — verify via E2E tests
- **WEB-04** (Chrome/Firefox/Safari/Edge) — Playwright cross-browser capability
- **WEB-05/06** (File System Access API) — verify via E2E tests
- **WEB-07/08** (share via URL, load from URL) — verify via E2E tests
- **EDIT-01/02/03** (syntax highlighting, auto-completion, error highlighting) — verify editor integration via E2E

Quality gates (not requirements but M002 success criteria):
- All clippy warnings fixed
- ESLint configured and passing
- rustfmt applied
- E2E test suite with screenshots

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Playwright E2E | `bobmatnyc/claude-mpm-skills@playwright-e2e-testing` (1.2K installs) | available |
| Playwright E2E | `alinaqi/claude-bootstrap@playwright-testing` (336 installs) | available |
| Vitest | `pproenca/dot-skills@vitest` (331 installs) | available |
| Vitest | `bobmatnyc/claude-mpm-skills@vitest` (272 installs) | available |
| ESLint | `knoopx/pi@eslint` (26 installs) | available — low installs |
| TypeScript strict | `eins78/skills@typescript-strict-patterns` (19 installs) | available — low installs |

The Playwright and Vitest skills from `bobmatnyc` have the highest install counts and are directly relevant. The ESLint and TypeScript skills have low install counts — likely not worth installing.

## Sources

- Playwright configuration for Vite projects (source: Context7 `/microsoft/playwright` docs)
- Vitest integration with existing vite.config.ts (source: Context7 `/vitest-dev/vitest` docs)
- Existing viewer codebase: `viewer/src/main.ts`, `viewer/src/interaction.ts`, `viewer/src/wasm.ts`
- Existing Rust test results: `cargo test --workspace` output (864 tests, 3 failures)
- Clippy analysis: `cargo clippy --workspace` output (122 warnings across 7 crates)
- rustfmt analysis: `cargo fmt --check` output (680 diffs across workspace)
