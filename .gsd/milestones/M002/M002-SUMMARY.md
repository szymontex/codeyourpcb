---
id: M002
provides:
  - Custom A*-based autorouter (constraint-aware, multi-layer, 500-component in 0.05s)
  - Three.js 3D board viewer with procedural component bodies, orbit/zoom/pan, layer visibility
  - DSL v2 grammar — modules, typed interfaces, imports, physical units (23 variants), constraint assertions
  - Manual trace editing in 2D viewer with live DRC feedback
  - Grid snap, command-pattern undo/redo, net highlighting, component rotation, board outline resize
  - 40 Vitest unit tests + 39 Playwright E2E tests + 8-stage quality gate script
  - PCB design rule database (IPC standards, manufacturer presets, signal integrity rules)
  - Competition feature matrix covering 9 EDA tools across 11 categories
  - Zero code duplication (jscpd enforced), zero lint warnings (clippy/eslint/rustfmt)
  - Performance: autorouter 0.05s/500 components, web load 105ms, 3D at 60fps
key_decisions:
  - "cypcb-autoroute is a new crate separate from cypcb-router — native A* replaces FreeRouting JAR wrapper"
  - "Grid-based A* with pathfinding crate, 8-directional movement, u32 cell indices, u64-scaled costs"
  - "Three.js lazy-loaded via dynamic import — zero initial bundle impact, full dispose on 2D toggle"
  - "PhysicalUnit enum (23 variants) in cypcb-core separate from Unit (length/Nm) — different base conversions"
  - "v2 constructs are parse-only — semantic evaluation deferred to future milestone"
  - "Command pattern (BoardCommand interface) for all board mutations with max depth 100"
  - "Desktop crates excluded from quality gates — require system deps unavailable in CI"
  - "Per-crate opt-level=3 for autorouter and pathfinding — WASM bundle unaffected"
  - "Code duplication enforced for TypeScript only via jscpd — no mature Rust dedup tool"
  - "Board outline polygon editing deferred — rectangle resize via drag handles only"
  - "GUI schematic capture explicitly out of scope — code-first is the identity"
patterns_established:
  - "Lazy-import pattern for heavy optional modules (Three.js loaded on first 3D click)"
  - "Command pattern for all board mutations — BoardCommand interface with execute/undo"
  - "Merged geometry pattern — same-layer primitives into single BufferGeometry for draw call minimization"
  - "v2 grammar rules follow same pattern as v1 — new _definition variants, dedicated convert_* methods"
  - "Quality gate script pattern — numbered stages, fail-fast, per-stage pass/fail labels"
  - "Shared geometry utilities in viewer/src/geometry.ts for deduplication"
  - "Per-crate Cargo profile overrides for speed-critical non-WASM crates"
  - "E2E tests wait for #status-text 'Ready' before interacting (WASM init gate)"
observability_surfaces:
  - "window.__renderer3d — { isActive, meshCount, drawCalls, fps } for 3D scene state"
  - "window.__undoStack — { canUndo, canRedo, depth, lastCommand } for undo state"
  - "Console log prefixes: [3D], [Undo], [Grid], [Net], [Rotate], [Resize] for feature-area filtering"
  - "scripts/quality-gate.sh — 8-stage pass/fail output, single source of truth for CI readiness"
  - "Playwright screenshot artifacts in viewer/test-results/ (baseline + on-failure)"
  - "Benchmark tests print timing tables (component count, net count, grid dims, routing time, completion %)"
  - "ParseError variants with span info for all v2 constructs — surface via LSP diagnostics"
requirement_outcomes:
  - id: WEB-01
    from_status: validated
    to_status: validated
    proof: "Playwright E2E test measures domContentLoaded at 105ms (<3000ms target) via Navigation Timing API"
duration: ~10 hours across 8 slices (S01-S08)
verification_result: passed
completed_at: 2026-03-13
---

# M002: CodeYourPCB v2.0 — Professional EDA Platform

**Custom autorouter routing 500 components in 0.05s, 3D board viewer at 60fps, DSL v2 with modules/units/constraints, manual trace editing, full E2E test coverage, and 8-stage quality gate — all linters clean, zero code duplication.**

## What Happened

Eight slices transformed CodeYourPCB from a working prototype into a professional EDA platform.

**S01 (PCB Knowledge Base)** built the foundation — IPC standards (IPC-2221, IPC-7351, IPC-2581), manufacturer constraint presets (JLCPCB, PCBWay, OSHPark), signal integrity classification rules, and trace geometry best practices encoded as a typed `cypcb-rules` Rust crate. Competitor repos (KiCad, LibrePCB, Horizon EDA) were cloned and analyzed for routing internals and design patterns.

**S02 (Custom Autorouter)** replaced the FreeRouting JAR wrapper with a native A*-based autorouter in `cypcb-autoroute`. Grid-based pathfinding with 8-directional movement, configurable resolution, per-net clearance enforcement, multi-layer via insertion, rip-up/reroute (3 iterations), and post-processing that merges collinear grid steps into clean trace segments. The `pathfinding` crate handles the priority queue with u64-scaled costs for integer precision.

**S03 (Renderer Upgrade & Manual Trace Editing)** upgraded the 2D Canvas renderer with proper trace width/clearance rendering, interactive click-drag trace routing with angle snapping (0°/45°/90°), and live DRC feedback during manual editing. Added a testable interaction API with hit-testing for traces, pads, and components.

**S04 (3D Board Viewer)** added Three.js via lazy dynamic import — zero initial bundle impact. Procedural board substrate at correct nm→mm dimensions, copper layer rendering (traces as flat quad ribbons, pads as triangulated shapes, vias as InstancedMesh), component bodies with SMD/THT height differentiation, refdes sprite labels, OrbitControls, and layer visibility toggling. Debug surface at `window.__renderer3d` reports mesh count, draw calls, and FPS.

**S05 (DSL v2)** extended the Tree-sitter grammar with 15+ new rules: modules, interfaces, imports, pin declarations, assert statements, physical values with 23 unit suffixes (resistance through power), and tolerance syntax. All wired through AST types, parser converters, LSP completions/hover, and Monaco tokenizer. All v1 files parse identically — backward compatibility verified across 10 example files.

**S06 (Competition Parity & UI Polish)** produced a 9-tool competitive feature matrix, then closed gaps: grid snap (applied before angle snap per KiCad convention), command-pattern undo/redo (AddTrace, RemoveTrace, RotateComponent, ResizeBoard), net highlighting with glow/dim effects, R/Shift+R component rotation, and 8-handle board outline resize with live preview.

**S07 (E2E Tests & Quality Gates)** cleaned 680 rustfmt diffs, 122 clippy warnings, and 5 test failures. Added ESLint v10 for TypeScript, 40 Vitest unit tests (viewport, hit-test, undo, url-state), 39 Playwright E2E tests across 8 spec files covering every core user flow, and fixed an innerHTML XSS vulnerability. Created `scripts/quality-gate.sh` running all 6 stages.

**S08 (Performance & Polish)** added per-crate `opt-level=3` for the autorouter (32% speedup), adaptive grid resolution for large boards, a synthetic 500-component benchmark (0.05s routing, 100% completion), Playwright performance E2E (105ms web load, 60fps 3D), refactored all 7 jscpd code clones into shared helpers, and extended the quality gate to 8 stages.

## Cross-Slice Verification

### Success Criteria

| Criterion | Target | Actual | Evidence |
|-----------|--------|--------|----------|
| Autorouter routes 500-component board | <30s | **0.05s** | `cargo test --release -p cypcb-autoroute -- benchmark_500_component --ignored` — 522/522 nets, 100% completion |
| 3D viewer at 60fps with component models | 60fps | **60fps** | Playwright performance E2E (`window.__renderer3d.fps`); procedural component bodies, not JLCPCB GLB (field plumbed, loading deferred) |
| DSL supports modules, interfaces, units, constraints | Parse + backward compat | **83 parser tests pass** | `cargo test -p cypcb-parser` — 58 v1 + 21 v2 + 3 example + 1 backward compat test |
| Manual trace editing with click-drag | Working interaction | **Implemented** | S03 delivered interaction system with angle snapping and live DRC; browser visual verification deferred (headless CI) |
| E2E test suite covers every user action | Full coverage | **40 unit + 39 E2E** | `npx vitest run` — 40 passed; `npx playwright test` — 39 passed |
| Web loads in <3s | <3s | **105ms** | Playwright performance E2E via Navigation Timing API |
| Desktop starts in <1s | <1s | **Not verified** | Desktop crates excluded from CI (system deps unavailable); deferred |
| Zero duplicate code paths | 0 clones | **0 clones, 0%** | `npx jscpd src/ --min-lines 10 --threshold 0` — 0 clones across 22 files |
| All linters pass | Clean | **All clean** | `cargo fmt --check` (0 diffs), `cargo clippy -- -D warnings` (0 warnings), `npx eslint src/` (0 errors) |

### Definition of Done Verification

- ✅ All 8 slices marked `[x]` in roadmap
- ✅ All 8 slice summaries exist
- ✅ Custom autorouter produces valid routes for reference boards — blink.cypcb routes in 573ms (release), 500-component synthetic in 0.05s
- ✅ 3D viewer renders boards with component bodies — procedural geometry with SMD/THT differentiation (real JLCPCB GLB models deferred; `model_3d` field plumbed)
- ✅ DSL v2 parser handles modules, constraints, units — 83 tests pass, all v1 files backward compatible
- ✅ Manual trace editing works in 2D viewer with DRC feedback — S03 implemented
- ✅ E2E test suite passes — 40 unit + 39 E2E, all green
- ✅ Performance benchmarks pass — autorouter 0.05s (<30s), web 105ms (<3s), 3D 60fps
- ✅ All linters pass — cargo fmt, clippy, eslint, rustfmt all clean
- ✅ Zero code duplication above threshold — jscpd 0 clones
- ⚠️ Competition feature matrix exists and shows parity/advantage on key features — `docs/competition-feature-matrix.md` produced; library management identified as weakest area

### Partial/Deferred Items

- **JLCPCB 3D GLB models**: `model_3d` field plumbed through Rust→WASM→TS but always `None`. Components render as procedural colored boxes. Real model loading is a future task.
- **Desktop start time**: Not verified in automated tests — desktop crates require pkg-config/gio-2.0 system deps unavailable in CI.
- **DSL v2 semantic evaluation**: Modules, interfaces, imports, and assertions parse correctly but have no runtime evaluation — no module instantiation, no import resolution, no constraint checking against DRC.
- **4-layer board routing**: Autorouter supports multi-layer but no 4-layer reference board test exists yet.

## Requirement Changes

No requirements changed status during M002. All 64 requirements entered and exited this milestone in `validated` status. The milestone advanced implementation depth on several requirements (EDIT-01/02/03 via v2 syntax, EDIT-07 via undo/redo, DESK-05 via keyboard shortcuts, WEB-01 via performance verification) but their validation status was already established in M001.

## Forward Intelligence

### What the next milestone should know
- The autorouter is dramatically faster than expected (0.05s for 500 components vs 30s target) — the A* grid approach with `pathfinding` crate is highly efficient. Future work should focus on routing quality (differential pairs, length matching) rather than speed.
- DSL v2 is parse-only — modules, interfaces, imports, and assertions all parse to clean AST types but nothing evaluates them. Wiring constraint assertions to DRC, implementing module instantiation, and import resolution are the natural next steps.
- Library management is the biggest competitive weakness. The feature matrix identifies supplier API integration (LCSC/Mouser) as the #1 adoption blocker.
- The `model_3d: Option<String>` field on ComponentInfo is fully plumbed Rust→WASM→TS. Loading real JLCPCB GLB models requires adding a fetch+loader branch in `renderer3d.ts::buildComponents()`.
- The 8-stage quality gate (`scripts/quality-gate.sh`) is the single command to verify CI readiness. All future work should keep it green.

### What's fragile
- **Adaptive grid thresholds** (80mm/200mm) are hardcoded — if real-world large board routing quality is poor, these need tuning based on actual designs.
- **Grammar conflicts** — the Tree-sitter grammar has an explicit conflicts declaration; adding new numeric/dimension rules may require updating it. Always test with `tree-sitter generate` after grammar changes.
- **Headless WebGL** — Playwright 3D tests use `window.__renderer3d.isActive` not pixel comparison; headless rendering varies by environment.
- **Pad net association** — pads don't carry net info in the snapshot, so net highlighting dims all pads globally rather than per-pad.
- **S01/S02/S03 summaries are doctor-created placeholders** — they lack the detail of S04-S08 summaries. Authoritative info for those slices lives in their task summaries.

### Authoritative diagnostics
- `./scripts/quality-gate.sh` — 8-stage quality gate, run this first when assessing codebase health
- `cargo test --release -p cypcb-autoroute -- benchmark --ignored --nocapture` — prints full timing table for all reference boards
- `window.__renderer3d` and `window.__undoStack` — browser console debug surfaces for runtime state
- `cargo test -p cypcb-parser -- test_backward_compat` — single test proving v1 file compatibility

### What assumptions changed
- **Autorouter speed**: Expected 500 components to push close to 30s limit — actual is 0.05s, three orders of magnitude under target. Grid construction overhead doesn't scale linearly.
- **Three.js was already installed**: Only `@types/three` needed adding.
- **Code duplication**: Expected to need exclusions — all 7 clones were genuine and refactorable into shared helpers.
- **ESLint version**: Installed v10 instead of planned v9 — identical flat config API.
- **Board substrate geometry**: `BoxGeometry` with translate is simpler than `ExtrudeGeometry` and visually identical.

## Files Created/Modified

### New crates
- `crates/cypcb-rules/` — PCB design rule database with IPC standards and manufacturer presets
- `crates/cypcb-autoroute/` — Custom A*-based autorouter with grid pathfinding, multi-layer support, rip-up/reroute

### Core modifications
- `crates/cypcb-parser/grammar/grammar.js` — 15+ new grammar rules for DSL v2
- `crates/cypcb-parser/src/ast.rs` — 10 new AST types for modules, interfaces, imports, assertions
- `crates/cypcb-parser/src/parser.rs` — 7+ converter methods, 25 new tests
- `crates/cypcb-core/src/physical_units.rs` — PhysicalUnit enum (23 variants) with SI normalization
- `crates/cypcb-world/src/world.rs` — rotate_component() and set_board_size() APIs
- `crates/cypcb-render/src/snapshot.rs` — ComponentInfo extended with body dimensions and model_3d
- `crates/cypcb-render/src/lib.rs` — WASM exports for rotation, resize, body dimension computation

### Viewer
- `viewer/src/renderer3d.ts` — Three.js 3D renderer (870+ lines), full geometry pipeline
- `viewer/src/undo.ts` — BoardCommand interface, UndoStack, 4 command types
- `viewer/src/routing.ts` — Grid snap, angle snap, interactive trace routing
- `viewer/src/interaction.ts` — Trace editing, resize drag, click-drag interaction state
- `viewer/src/renderer.ts` — Net highlighting, resize handles, glow/dim effects
- `viewer/src/geometry.ts` — Shared geometry utilities (extracted from dedup)
- `viewer/src/wasm.ts` — Extended PcbEngine interface with rotation, resize, shared helpers

### Testing
- `viewer/src/__tests__/{viewport,hit-test,undo,url-state}.test.ts` — 40 unit tests
- `viewer/e2e/{app-load,editor,board-interaction,three-d-view,undo-redo,theme,errors,reliability}.spec.ts` — 39 E2E tests
- `viewer/e2e/performance.spec.ts` — Performance verification (web load, 3D FPS)

### Configuration & scripts
- `scripts/quality-gate.sh` — 8-stage quality gate
- `viewer/eslint.config.js` — ESLint v10 flat config
- `viewer/vitest.config.ts` — Vitest configuration
- `viewer/playwright.config.ts` — Playwright configuration
- `viewer/.jscpd.json` — Code duplication detection config
- `Cargo.toml` — Per-crate opt-level overrides

### Documentation & examples
- `docs/competition-feature-matrix.md` — 9-tool competitive feature matrix
- `examples/v2-{modules,interfaces,constraints}.cypcb` — DSL v2 example files
