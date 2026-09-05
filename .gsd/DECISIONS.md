- "MVP silkscreen uses crosshair markers (+) instead of full text rendering"
- "Courtyard outlines drawn as axis-aligned rectangles (rotation deferred)"
- "0.1mm outline width (router bit kerf), 0.15mm silkscreen line width"
- "Menu as data model (not trait) - Tauri native menus and HTML menus are fundamentally different rendering paradigms"
- "Platform facade aggregates all services - application code never imports platform-specific types"
- "Platform accessor methods return concrete types behind cfg - simpler than trait objects or generics"
- "Track lastLoadedSource in main.ts since engine lacks get_source() method"
- "Place event listeners inside isDesktop() guard to avoid registering in web mode"
- "Workspace-level release profile applies to all crates (not crate-specific)"
- "wasm-opt flags configured in Cargo.toml to ensure wasm-pack uses them"
- "Enabled bulk-memory and nontrapping-float-to-int for modern WASM features"
- "SIMD not enabled to maintain browser compatibility"
- "Post-build wasm-opt step optional (wasm-pack already runs it)"
- "File System Access API with fallback pattern chosen over browser-fs-access library"
- "Handle-based save-in-place for supported browsers, download fallback for others"
- "Desktop flow preserved via isDesktop() guards - Tauri IPC unchanged"
- "cypcb-rules is a leaf crate depending only on cypcb-core — no cypcb-world or cypcb-drc dependency allowed"
- "RoutingRuleSet trait uses u8 layer indices instead of importing Layer enum from cypcb-world — avoids dependency while remaining mappable"
- "All dimension fields in DesignConstraints use Nm type (nanometer integers) — no raw floats for physical measurements"
- "JLCPCB 2-layer preset corrected to 5mil (0.127mm) from research — previous 6mil (0.15mm) was conservative"
- "Manufacturer preset values include source URLs and dates in code comments for traceability"
- "cypcb-rules presets (RulesPreset) are separate from cypcb-drc presets (Preset) — DRC Preset wraps/delegates to avoid breaking existing API"
- "Non-dimension physical values in cypcb-rules use integer scaling: copper weight ×10 (oz), impedance ×100 (ohms), dielectric constant ×1000 (εr), current capacity ×100 (mA/mm) — avoids floats while preserving meaningful precision"
- "cypcb-autoroute is a new crate separate from cypcb-router — cypcb-router keeps FreeRouting JAR fallback, cypcb-autoroute is the native A* router. cypcb-autoroute depends on cypcb-router for RoutingResult types."
- "Grid-based A* with pathfinding crate — no hand-rolled priority queue. 8-directional movement, configurable grid resolution (default: min_clearance/2)"
- "Autorouter grid uses u16 coordinates internally (max ~65535 cells per axis, sufficient for 100mm board at 2µm resolution). Layer index is u8."
- "Net ordering: shortest Manhattan span first, power/ground nets last. Rip-up/reroute capped at 3 iterations default."
- "Autorouter post-processing merges collinear grid steps into single RouteSegments — reduces output noise and matches expected trace geometry"
- "Grid occupancy uses flat Vec<u8> per layer indexed by y*width+x (not Vec<Vec<u8>>) for cache-friendly linear access. Cell flags are a bitfield supporting pad|trace|zone|via|obstacle overlay."
- "Grid uses u32 for cell indices (not u16 as originally planned) — u16 max of 65535 is tight for larger boards at fine resolution. u32 supports up to 4 billion cells."
- "route_board() takes &mut BoardWorld because bevy_ecs query API requires mutable world reference for iteration."
- "A* cost values scaled to u64 (f64 * 1000) for pathfinding crate integer compatibility — preserves sub-unit precision for √2 diagonals while using crate's optimized integer priority queue"
- "find_path() uses any_end_layer flag for through-hole pad routing — path can arrive on any layer at the goal position, avoiding forced via at destination"
- "PadZone mechanism for A* pathfinder — pad cells are marked as obstacles for clearance enforcement, but net-own pads must be reachable; PadZone circles override is_free() within pad radius + clearance margin"
- "Blocking net detection samples along the full direct path (not just endpoints) to find congestion blockers at any point along the route"
- "Rip-up/reroute restores victim net on failure — if re-routing the victim fails, the orchestrator restores the original route and tries the next iteration with a different victim"
- "3D renderer lazy-loaded via dynamic import() — no Three.js code in initial bundle, separate Vite chunk"
- "Renderer3D instantiated on first 3D click, fully disposed on toggle back to 2D — no persistent WebGL context when not in use"
- "Board substrate uses BoxGeometry with translate (not position) for bottom-face-at-Z=0 coordinate convention"
- "Traces as flat quads (2 tris per segment) merged into single BufferGeometry per layer — minimizes draw calls over per-trace meshes"
- "Vias use InstancedMesh with shared CylinderGeometry template + per-instance scale matrix — drill holes as second InstancedMesh in substrate color"
- "Z-offsets for copper: bottom 0.035mm, top board_thickness-0.035mm, pads +0.005mm above traces to prevent Z-fighting"
- "Component height heuristic: SMD=1.2mm, THT=5mm, detected by presence of drill pads in footprint"
- "Refdes labels as THREE.Sprite with CanvasTexture — camera-facing, no billboard shader needed"
- "Pad bbox fallback: when footprint bounds are zero, compute body dimensions from pad extents"
- "PhysicalUnit is a new enum in cypcb-core separate from Unit (length units) — electrical quantities (ohms, farads, etc.) have different base conversions and cannot reuse the Nm pipeline"
- "DSL v2 uses version 2 declaration to enable new features — version 1 files follow existing grammar path unchanged"
- "Module instantiation reuses component syntax (component PSU1 PowerSupply { ... }) rather than new keyword — backward compatible and familiar"
- "Constraint assertions (assert) are parse-only in S05 — evaluation and wiring to DRC/autorouter deferred to S06/S07"
- "Import resolution starts file-relative only — project root and registry resolution deferred"
- "Tree-sitter grammar conflict between dimension and assert_operand (bare number ambiguity) resolved with explicit conflicts declaration — Tree-sitter picks the right alternative based on surrounding context"
- "Component value_property accepts physical_value at grammar level but converts to StringLit in parser — preserves backward compat while grammar supports both; richer PhysicalValue field in component AST deferred to T02"
- "ToleranceKind::Absolute and Range use Box<PhysicalValue> to break recursive type cycle (PhysicalValue → Tolerance → ToleranceKind → PhysicalValue)"
- "Board-level undo/redo is a separate stack from Monaco editor text undo — UndoStack in viewer/src/undo.ts wraps PcbEngine mutations only"
- "Grid snap applied before angle snap in routing (KiCad convention) — grid determines discrete positions, then angle snap selects among grid points"
- "Board outline polygon editing deferred to S08 — S06 implements rectangle resize via drag handles only, avoiding ripple through export/DRC/3D"
- "WASM mutation APIs (rotate_component, set_board_size) mutate BoardWorld directly rather than round-tripping through source parse — cleaner for UI-driven mutations"
- "Command pattern (BoardCommand interface) for all board mutations — enables undo/redo via UndoStack with max depth 100. Future mutations (rotate, resize) must implement BoardCommand."
- "Grid snap applied before angle snap in routing pipeline — grid constrains position, angle constrains direction"
- "onTraceAdd callback in InteractionState decouples interaction.ts from undo.ts — routing creates traces through callback, main.ts routes to undo stack"
- "BoardWorld mutation API (rotate_component, set_board_size) returns bool for success/failure — callers check return rather than unwrapping Results"
- "Resize drag uses live-preview + revert-on-mouseup + undo-push pattern — direct engine calls during drag for responsive feedback, then revert to original and push single undo command on completion. Prevents undo stack pollution from intermediate drag states."
- "Board resize handles: 8 handles (4 corners + 4 edges) rather than just 4 corners — edge midpoint handles allow single-axis resize without diagonal constraint"
- "Net highlight glow uses 2.0x width at 0.3 alpha for matching traces, non-matching dimmed to 0.15 alpha — subtler than selection glow (2.5x/0.35) to maintain visual hierarchy"
- "Pad dimming is global when net is highlighted — pads don't carry net info in snapshot, so all pads dim rather than per-pad net matching. Per-pad highlighting deferred until pad-to-net mapping exists in snapshot."
- "Library management identified as weakest competitive category vs all 9 EDA tools — supplier API integration (LCSC/Mouser) ranked as #1 adoption blocker in feature matrix gap analysis"
- "GUI schematic capture explicitly out of scope — code-first is CodeYourPCB's identity; competing on schematic GUI would dilute focus and match incumbents at their strength"
- "Desktop crates (cypcb-cli, cypcb-desktop) excluded from quality gates — require pkg-config/gio-2.0 system deps unavailable in CI/dev containers"
- "clippy ptr_arg on parser &mut Vec<ParseError> methods gets #[allow] not refactor — methods push() to the Vec, so &mut [ParseError] would be wrong; refactoring to return errors would be high-churn for no functional gain"
- "Quality gate script runs 6 stages in sequence: cargo fmt, clippy, cargo test, eslint, vitest, playwright — fails fast on first broken stage"
- "Playwright E2E tests run headless Chromium only — WebGL 3D tests verify renderer active state via page.evaluate rather than pixel comparison (headless WebGL rendering varies)"
- "Code duplication threshold deferred — roadmap says 'zero above threshold' but no tool or number defined; tracked as S08 follow-up after quality infrastructure is in place"
- "ESLint v10 flat config with typescript-eslint recommended — no-explicit-any off (WASM interop), no-this-alias off (debug surface closures), unused vars/args/caught with _ prefix ignored"
- "Vitest tests live in viewer/src/__tests__/*.test.ts, environment: node (pure functions only — DOM-dependent code tested via Playwright E2E)"
- "Board outline polygon editing cut from S08 — rectangle resize from S06 is functional, polygon editing ripples through export/DRC/3D with unfavorable risk/reward for final slice"
- "Per-crate opt-level=3 for cypcb-autoroute and pathfinding crates — WASM bundle unaffected since autorouter is not a WASM dependency; workspace opt-level='z' preserved for WASM size optimization"
- "Adaptive grid resolution for autorouter — boards >80mm get coarser grid to scale routing time sub-linearly; quality vs speed tradeoff accepted for large boards"
- "3D FPS headless threshold is 30fps (not 60fps) — headless WebGL rendering is slower than real GPU; 60fps target applies to real browser only"
- "Code duplication threshold: zero exact duplicates >10 lines in viewer/src/ via jscpd — Rust has no mature dedup tool, so Rust duplication not enforced"
- "Incremental spatial index rebuild deferred — profile first, optimize only if rebuild_spatial_index_full() exceeds 5ms (unlikely for current board sizes)"
- "Web load time measured via domContentLoadedEventEnd — correlates with WASM module init completing; more meaningful than loadEventEnd for SPA"
- "All 7 jscpd code clones refactored into shared helpers rather than excluded — genuine duplication eliminated through geometry.ts, addCopperMesh, applyRoutesToSnapshot, computeHandlePositions, and applyResize/stopDrag"
- "Copper fill zones deferred from S01 renderer — cypcb-world has no Zone/CopperFill type in the ECS; cannot render what doesn't exist in the data pipeline. Will require Rust data model addition in a later slice."
- "Silkscreen in S01 uses rectangular body outlines from body_width_nm/body_height_nm — real KiCad silkscreen has curves/text/complex outlines but snapshot only carries rectangular bounds. Acceptable for beta."
- "Pad-to-net mapping built client-side from NetInfo.connections rather than adding net_id to Rust PadInfo — avoids cross-boundary Rust/WASM change for S01, fast enough with Map<string, string> lookup"
- "LOD system uses 4 tiers (far/medium/close/detail) based on viewport scale — controls text density to keep Canvas fillText under frame budget. Thresholds defined in RenderConfig for S04 customization."
- "Text rendering in separate pass after all shapes — ensures pad numbers and refdes render on top of copper for readability, prevents Z-ordering issues with overlapping elements"
- "RenderConfig extracted as boundary contract — S03 consumes pad highlighting capabilities, S04 drives colors/fonts/LOD thresholds. Interface designed with both consumers in mind."
- "Renderer diagnostic surface (window.__renderDiag) for E2E testability — exposes LOD tier, pad-net map size, frame time, text count. Avoids fragile pixel comparison in headless tests."
- "Exposed window.__loadBoard(source) in main.ts for E2E board loading — calls load_source + pullSnapshot + fitBoard in one go, avoiding test-side coupling to internal render loop"
- "Added highlightedNet to RenderDiag interface — E2E verifies net highlighting through diagnostic surface, not fragile DOM inspection or pixel sampling"
- "JS parser body dimensions computed from pad bounding box at parse time — avoids cross-boundary Rust change, consistent with 2D renderer's existing pad-bbox fallback approach"
- "NaN guard uses !(x > 0) pattern instead of x <= 0 — catches NaN, undefined, 0, and negative in one expression. JavaScript-specific defensive pattern for numeric fields from Partial<T> casts"
- "GLTFLoader imported inside lazy-loaded renderer3d.ts module — keeps Three.js out of main bundle per existing lazy-load decision"
- "3D geometry counts (componentCount, traceSegmentCount, padCount, viaCount) exposed on __renderer3d debug surface — enables E2E verification without fragile pixel comparison, extends existing diagnostic pattern from S01's __renderDiag"
- "loadComponentModel replaces placeholder box mesh in-place — finds mesh by name convention (component-{refdes}), copies transform, swaps geometry. Loaded GLTF scenes tracked in Map for disposal."
- "Angle snap defaults to OFF (toggleable with A key) — roadmap says 'optional toggle, not forced'; changes from always-on in previous implementation"
- "Magnetic snap takes priority over angle snap — when cursor is near a target pad, snap to pad center regardless of angle constraint. Matches KiCad behavior."
- "Magnetic snap uses dual threshold: 1mm world-space OR 15px screen-space, whichever is larger — consistent UX across zoom levels"
- "Target pads pre-computed at route start, not scanned per frame — typical net has 2-10 pads, pre-filtering avoids per-frame full-pad scan"
- "Keyboard handlers on document level with routing mode guard — canvas doesn't receive keyboard events without focus; guard checks routingState.mode and document.activeElement to avoid Monaco conflicts"
- "Ratsnest emphasis during routing: active net lines at full alpha + 2x width, non-matching dimmed to 0.15 alpha — maintains spatial context while guiding user to target"
- "WasmPcbEngineAdapter JS fallback for add_trace/remove_trace/run_drc/trace_count — WASM module lacks trace mutation exports; adapter checks typeof before calling WASM, falls back to JS-side cached snapshot mutation (same logic as MockPcbEngine). Enables route completion without WASM rebuild."
- "__loadBoard syncs interactionState.viewport + snapshot — without this sync, click handler's screenToWorld uses stale default viewport, making all pad hit-tests miss. Critical for E2E and real-world file loading."
- "Exposed window.__viewport diagnostic surface with live getters — E2E tests need actual viewport state to compute pad screen coordinates; reimplementing fitBoard in test code was brittle and diverged from runtime state."
- "Grid visibility and grid snap are two independent controls — View menu toggles visual grid on/off (renderer), Preferences sets snap spacing for routing. Fixes 'grid toggle does nothing' bug where #grid-snap only controlled routing snap, not visual grid."
- "Settings persistence uses single localStorage key 'cypcb-settings' as JSON — simpler than per-key storage, atomic read/write, easy to inspect/debug. ThemeManager continues to own its own 'theme' key for FART prevention (theme must load before settings module initializes)."
- "Toolbar element IDs preserved inside View dropdown (#layer-top, #layer-bottom, #layer-ratsnest) — minimizes E2E test changes; controls just moved in DOM, not renamed."
- "Mil format precision: 4 decimal places (0.0001mil ≈ 2.54nm) for round-trip fidelity — 2 decimals caused precision loss for values not evenly divisible by 25_400nm"
- "View menu is a dropdown positioned below the View button (not a side panel) — keeps toolbar clean while maintaining single-click access to layer/grid/ratsnest toggles"
- "Preferences modal uses inline event handlers per-control mapped to setPreference() — simpler and more explicit than data-attribute walker pattern, avoids runtime string-to-key coercion"
- "Project manager is a file manager with templates, not a project abstraction — 'project' = one .cypcb file, no directories or workspace files"
- "Templates bundled as static assets in viewer/public/templates/ — Vite serves from public/ in both dev and prod, no build step needed"
- "Recent files store metadata only (name, timestamp, thumbnail data URL) — FileSystemFileHandle cannot be serialized to localStorage, user reopens via file picker"
- "Recent files list capped at 10 entries, sorted most-recent-first — prevents unbounded localStorage growth"
- "Project manager overlay z-index 150 — above canvas (0) and view dropdown (50), below prefs-overlay (200)"
- "Thumbnail generation via offscreen Canvas 2D render at 200×150 — stored as data URL in recent files entry, generated once on file load/save"
- "Blank template is an inline scaffold string in project-manager.ts — no file needed for a 10-line default board declaration"
- "PM dismissal in E2E tests via __loadBoard(MINIMAL_BOARD) in beforeEach — established as standard pattern for all tests needing canvas/editor access. PM overlay (z-index 150, top: 41px) blocks canvas clicks but not toolbar"
- "Editor debug surface exposed as window.__editor after Monaco init — window.monaco not available in Vite ESM builds; __editor provides direct editor.setValue()/getValue() for E2E editor→board sync testing"
- "show()/hide() methods added to __projectManager debug surface — enables E2E lifecycle testing without depending on desktop:new-file event (desktop-only) or adding web-side Ctrl+N shortcut"
- "JLCPCB search uses tscircuit/jlcsearch (no auth, CORS-enabled) — official LCSC API requires API key + nonce + signature, impractical for client-side-only app"
- "EasyEDA 3D models parsed with custom OBJ parser (~100 lines) instead of Three.js OBJLoader — EasyEDA format uses non-standard inline newmtl/endmtl blocks and f v// v// v// face syntax that standard loaders can't handle"
- "EasyEDA OBJ d 0.0 treated as fully opaque — EasyEDA convention differs from standard OBJ where d 0.0 means fully transparent"
- "3D model fetch triggered only on component click, not on search — prevents hammering EasyEDA API with requests for every search result"
- "Search panel is a right-side overlay (z-index 100) not a modal — user can see the board while browsing components. Below PM (150) and prefs (200)"
- "loadComponentFromOBJ added alongside loadComponentModel — parallel method for OBJ text input, same placeholder-replacement pattern and loadedModels tracking"
- "JLCPCBSearchError class exported for instanceof-check — network-level failures (fetch throws) return [] silently, but HTTP errors (4xx/5xx) throw JLCPCBSearchError so the panel can show distinct error states"
- "Prefs-theme E2E test asserts button label change, not data-theme attribute — theme cycle light→dark→auto→light means auto resolves to same data-theme as light in headless Chromium, making attribute comparison unreliable"

### Status of the quoted decisions, read against the code on 2026-09-05

The list above records what was decided. Two of its lines describe a build
that has changed since, and both were read the way the DRC table below was.

- **"Desktop crates (cypcb-cli, cypcb-desktop) excluded from quality gates":
  the exclusion is gone, and taking it out is what found the rot.** Both
  crates are in every workspace stage now, and `scripts/setup-dev.sh` installs
  the GTK and WebKit the desktop crate needs. The gate says in its own comment
  what the exclusion cost: `cypcb-desktop` went unbuilt long enough to collect
  nine compile errors from the Tauri v1 to v2 move, plus an icon the macro
  refused, all found the first time anybody compiled it.
- **"Quality gate script runs 6 stages": the gate runs 12 stages as of
  2026-09-05** - `grep -c '^echo "\[' scripts/quality-gate.sh`. The six named
  here are all still there, and six ran up behind them: `tsc --noEmit` after
  `cargo test`, then `autorouter benchmark`, `jscpd`, the reach of the engine
  API, a smoke test of the desktop application and a selftest of the scheduled
  gate after `playwright`. The count is held by
  `crates/cypcb-cli/tests/the_decision_log_is_current_where_it_says_it_is.rs`.


## M004 Decisions

| # | When | Scope | Decision | Choice | Rationale | Revisable? |
|---|------|-------|----------|--------|-----------|------------|
| D-M004-001 | M004 | arch | Routing approach | Multi-strategy empirical (PathFinder + improved A*) | User: "weź wszystkie opcje, porównuj ze sobą". Let data decide, not assumptions. | Yes — if one strategy dominates, drop others |
| D-M004-002 | M004 | arch | Benchmark source | KiCad open-source .kicad_pcb projects | Real human-routed designs as ground truth. Automated parse+compare pipeline. | No |
| D-M004-003 | M004 | pattern | Variant UX | Auto-apply best, hover preview alternatives | User: "2 ale user może hoverować na inne rezultaty". Not a picker, a preview. | Yes — if users want explicit selection |
| D-M004-004 | M004 | scope | Renderer upgrade | Separate milestone (M005) | User explicitly chose to split. M004 uses current renderer for screenshots. | No |
| D-M004-005 | M004 | quality | DRC + aesthetics equally critical | Zero tolerance for both | User: "oba równie ważne". No crossing traces AND no ugly traces. | No |
| D-M004-006 | M004 | arch | Realtime tuning | Sliders → reactive re-routing <1s | User: "powinien reagować realtime". Constrains engine to fast execution or incremental approach. | Yes — if complex boards need >1s, show progress |
| D-M004-007 | S01 | arch | Custom .kicad_pcb parser instead of kicad_parse_gen | Use `symbolic_expressions` crate directly, write custom tree walker | `kicad_parse_gen` v0.7.3 only handles `module` keyword (KiCad 4/5), not `footprint` (KiCad 7/8); also drops segments/vias to `Other(Sexp)`. Custom parser gives KiCad 7/8 support + trace/via extraction. | No — kicad_parse_gen would need fork/patch |
| D-M004-008 | S01 | arch | Board outline as bounding box | Extract Edge.Cuts elements, compute axis-aligned bounding box for BoardSize | Real KiCad boards may have non-rectangular outlines; BoardSize only supports rectangles. Bounding box is sufficient for S01; polygon outline deferred. | Yes — if complex board shapes needed later |
| D-M004-009 | S01 | scope | Keep kicad_parse_gen for .kicad_mod only | New pcb_parser.rs for .kicad_pcb, existing footprint.rs unchanged for .kicad_mod | Footprint import via kicad_parse_gen works fine for individual .kicad_mod files; only the board-level .kicad_pcb parsing needs the custom approach. | No |
| D-M004-010 | S01 | data | Synthetic benchmark fixtures instead of real projects | Created 3 hand-crafted KiCad 8 .kicad_pcb files (7/29/52 components) | Search didn't yield directly downloadable KiCad 8 files with permissive licenses; synthetic files are license-clean, precisely controlled for expected counts, and cover all complexity tiers. Task plan explicitly allowed this. | Yes — replace with real projects if found |
| D-M004-011 | S01 | api | KicadBenchmark uses &'static str fields | Changed filename/description from String to &'static str | Required for BENCHMARKS const array; const contexts don't support heap allocation. Fields are compile-time string literals so &'static str is correct and avoids allocation. | No |
| D-M004-012 | S02 | arch | RoutingScore uses serde Serialize, not manual format!() JSON | Added serde dependency to cypcb-autoroute | RoutingScore is consumed by S06 (WASM variant ranking) and S07 (benchmark comparison) — proper serde derive ensures correct serialization across all consumers. serde compiles for wasm32-unknown-unknown (already proven by cypcb-world's use). | No |
| D-M004-013 | S02 | arch | Crossing detection uses segment_distance()==0, not DRC violations | Separate crossing count metric distinct from drc_violations | DRC catches clearance near-misses too; crossings are exact same-layer inter-net segment intersections only. Keeps metrics orthogonal — drc_violations measures manufacturability, crossings measures routing topology. | No |
| D-M004-014 | S02 | scope | Scoring module in cypcb-autoroute, not a separate crate | Single scoring.rs module, not cypcb-scoring crate | Scoring is tightly coupled with autorouter output semantics (Trace/Via ECS components, DRC integration). Boundary map specifies cypcb-autoroute::scoring. Separate crate would add dependency management overhead with no benefit. | No |
| D-M004-015 | S02 | api | DRC/crossing assertions use range checks, not == 0 | Integration tests assert drc_violations < 200, crossings < 50 | A*-based autorouter doesn't guarantee zero-violation routing on complex boards. Scoring correctly reports what exists. Asserting == 0 would tie scoring tests to routing quality improvements. | No |
| D-M004-016 | S02 | arch | ScoreCommand uses DesignRules for DRC, PresetRuleSet for routing | Separate rule types for routing vs scoring pipelines | DesignRules (cypcb_drc) has clearance-specific checks; PresetRuleSet (cypcb_rules) implements RoutingRuleSet trait. score_board() takes &DesignRules per its API contract. | No |
| D-M004-017 | S03 | arch | CongestionMap separate from RoutingGrid | Separate `CongestionMap` struct, not extending RoutingGrid fields | Congestion tracking (present/history costs) is PathFinder-specific. Coupling it to RoutingGrid would add memory overhead for all grid users and violate single-responsibility. PathFinder owns its congestion data alongside a shared grid reference. | No |
| D-M004-018 | S03 | arch | Per-net cell index for O(path_length) rip-up | `HashMap<u32, Vec<(u32, u32, u8)>>` tracking which cells each net uses | Existing `clear_route()` is O(width × height × layers) — scans entire grid. PathFinder calls rip-up hundreds of times per routing session. Per-net index reduces each rip-up to O(path_length), critical for WASM performance. | No |
| D-M004-019 | S03 | arch | PathFinder uses existing `find_path_with_zones()` as inner kernel | Congestion cost injected via augmented cost closure, not a forked pathfinder | `pathfinding::astar()` evaluates cost closures at search time — dynamic congestion costs work without library changes. Reusing proven code reduces algorithmic bugs. | No |
| D-M004-020 | S03 | algo | VPR partial-reroute optimization | Only re-route nets passing through overused cells, not all nets every iteration | Full re-route of all nets per iteration is O(nets × grid) per iteration. Partial re-route reduces per-iteration work by 5-10x on boards where congestion is localized. Essential for WASM performance budget. | No |
| D-M004-021 | S03 | arch | RoutingStrategy trait in cypcb-autoroute, not cypcb-rules | Strategy is in cypcb-autoroute crate | cypcb-rules is a leaf crate with no world/grid dependency. Strategy requires BoardWorld, FootprintLibrary, RoutingGrid — belongs in cypcb-autoroute. | No |
| D-M004-022 | S03-T01 | pattern | ImprovedAStarStrategy duplicates helper functions from orchestrator | build_spanning_tree, pad_to_grid_node, pad_to_zone copied into astar_improved.rs | These helpers are private in orchestrator.rs and tightly coupled to its own Connection type. Making them pub would expose internal types. Self-contained strategy module is cleaner and matches the goal of strategies owning their full routing loop. If more strategies need these, extract into a shared helpers module. | Yes — refactor if T02 also duplicates |
| D-M004-023 | S03-T02 | pattern | Made orchestrator helpers public instead of duplicating for PathFinder | Made `pad_to_grid_node`, `pad_to_zone`, `build_spanning_tree`, `is_multi_layer`, `Connection` pub in orchestrator.rs | T02 needed the same helpers. Duplicating ~100 LOC of identical spanning-tree/grid-conversion logic would create a maintenance burden. Reverses D-M004-022 pattern; ImprovedAStarStrategy still has its own copies but future strategies should use orchestrator's public API. | No |
| D-M004-024 | S03-T03 | data | KiCad parser translates component positions to board-origin-relative coords | Subtract `board_bounds.min` from all component positions in `parse_footprint()` | KiCad stores absolute positions (e.g. 120mm,115mm). Routing grid assumes origin at (0,0) with board width×height. Without translation, all pads mapped to grid corner cell, producing zero-length paths. | No |
| D-M004-025 | S03-T03 | scope | Large benchmark strategy comparisons are #[ignore] tests | stm32_breakout and multi_ic marked `#[ignore]`; led_blink runs in CI | A* routing on 75×65mm / 100×80mm grids takes >60s per strategy even in release mode. led_blink (40×30mm, 7 components) fully validates the integration. | Yes — unignore when routing performance improves |
| D-M004-026 | S04 | arch | Smoother operates on Vec<RouteSegment> post grid-to-Nm conversion, not on grid cells | `smooth_routes()` takes `&[RouteSegment]` in Nm coordinates | Decouples smoother from routing algorithm internals. Grid-level simplification (postprocess.rs) still runs first as a useful pre-pass. Smoother doesn't need grid access. | No |
| D-M004-027 | S04 | arch | Angle validation uses integer direction patterns, not floating-point atan2 | Check dx==0, dy==0, or |dx|==|dy| for valid 45°-multiple angles | Avoids floating-point precision issues with atan2 on i64 coordinates. Integer checks are exact and fast. atan2 only used in scoring's compute_smoothness(). | No |
| D-M004-028 | S04 | pattern | Per-move DRC uses segment_distance() against other-net segments, full run_drc() only as final gate | Lightweight local check per smoothing move, expensive full DRC once at end | Running full run_drc() per smoothing move is O(n²) on the board — too expensive. segment_distance() against nearby segments is O(k) per move where k is small. | No |
| D-M004-029 | S05 | arch | AutorouteParams is a separate user-facing struct consumed by AutorouteConfig | `AutorouteParams { via_cost, layer_preference, roundness, density }` with serde derives, consumed by `AutorouteConfig.params` field | Boundary map specifies AutorouteParams as the user-facing API. Keeping it separate from AutorouteConfig maintains a clean internal/external split — internal config has strategy selection, grid resolution override, etc. while params is the UI-tunable subset. | No |
| D-M004-030 | S05 | api | Tuning panel is a collapsible dropdown, not inside prefs modal | Separate `#tuning-panel` adjacent to Route button, toggled via ⚡ button | Research recommended keeping tuning outside prefs modal — tuning is used during active routing sessions, not setup. Dropdown pattern matches existing view menu. Quick access matters for realtime interaction. | Yes — could move into prefs if users prefer |
| D-M004-031 | S05 | pattern | Slider debounce at 300ms using setTimeout pattern | 300ms setTimeout cleared on each input event before firing auto_route_with_params | Matches existing editor change debounce pattern in main.ts. 300ms is fast enough to feel responsive, slow enough to avoid cascading WASM calls during rapid slider adjustment. | Yes — adjustable if UX testing suggests different value |
| D-M004-032 | S05 | api | auto_route_with_params added alongside auto_route, not replacing it | New `auto_route_with_params(params_json)` method; existing `auto_route()` unchanged | Backward compatibility — existing callers (Route button, tests) continue to work. Zero-param route uses default config. Parameterized route is for tuning sliders only. | No |
| D-M004-033 | S06 | arch | Sequential variant generation on single &mut BoardWorld | clear→route→apply→rebuild→score→capture loop per variant, best auto-applied at end | BoardWorld wraps bevy_ecs World which does NOT implement Clone. Cannot snapshot/restore world state. Must route sequentially: clear previous, route new, apply to ECS for scoring, capture results, then clear before next. Best variant re-applied after all variants scored. | No |
| D-M004-034 | S06 | bug | std::time::Instant conditional compilation for WASM | `#[cfg(not(target_arch = "wasm32"))]` guards on Instant::now() and elapsed() in generate_variants() | std::time::Instant panics in WASM with "time not implemented on this platform". Conditional compilation removes timing from WASM builds while keeping it for native tests. | No |
| D-M004-035 | S06 | pattern | WASM fallback: auto_route_variants() → auto_route() on crash | triggerRouting() catches WASM panic from auto_route_variants(), reloads board, falls back to auto_route() | WASM panics corrupt engine state (recursive borrow). Fallback reloads source to reset, then uses proven auto_route(). Variant panel hidden in fallback path. | Yes — remove fallback once WASM stability proven |
| D-M004-036 | S06 | observability | console_error_panic_hook added for WASM diagnostics | console_error_panic_hook::set_once() in PcbEngine::new() | WASM panics show only "unreachable" without this hook. With it, full Rust panic message + stack trace appears in browser console. Essential for debugging WASM issues. | No |
| D-M004-037 | S07 | quality | Regression gate uses ±10% composite threshold, not exact match | `composite ≤ baseline × 1.1` (5501) instead of `composite == 5001` | Floating-point variation across platforms and minor algorithm changes would make exact-match tests flaky. 10% margin absorbs normal variation while still catching real regressions. | Yes — tighten if scores stabilize |
| D-M004-038 | S07 | scope | Benchmark screenshots are artifacts for human review, not pixel-diffed | Playwright captures PNGs to `test-results/benchmark/`, no pixel comparison assertions | D-M004 DECISIONS note "headless WebGL rendering varies" — pixel comparison would be flaky. Screenshots serve R115 (visual comparison) via human inspection. | No |

### Status of the M004 decisions, read against the code on 2026-09-05

- **D-M004-003, auto-apply the best and hover the alternatives: the surface it
  describes was deleted in `a9e8c7a`.** The engine half is untouched -
  `auto_route_variants` is at `crates/cypcb-render/src/lib.rs:916` and
  `viewer/src/wasm.ts` still declares and wraps it - but **nothing calls it**:
  a search of `viewer/src` and `viewer/e2e` for the name finds only that
  wrapper and a comment. Variants are code that runs on request from nowhere.
- **D-M004-035, the fallback that hides the variant panel: half of it is
  gone.** `triggerRouting()` is still in `viewer/src/main.ts`, and the word
  `variant` does not appear in that file at all, so there is no call to catch
  and no panel to hide. `REQUIREMENTS.md`, `viewer/e2e/variant-panel.spec.ts`
  and `PROJECT.md`'s status line were corrected on 2026-09-04; this table is
  the fourth page that had been telling a reader the panel is there.


## M005 Decisions

| # | When | Scope | Decision | Choice | Rationale | Revisable? |
|---|------|-------|----------|--------|-----------|------------|
| D-M005-001 | M005 | arch | WASM routing execution | Web Worker (off main thread) | Synchronous WASM on main thread freezes browser 60-160s. Worker is standard solution. | No |
| D-M005-002 | M005 | arch | Cancel mechanism | worker.terminate() + respawn | WASM has no cooperative preemption. Terminate is the only reliable cancel. | Yes — if SharedArrayBuffer becomes available |
| D-M005-003 | M005 | scope | R120 renderer upgrade | Deferred to future milestone | M005 repurposed for critical WASM fix. Renderer is separate work. | No |
| D-M005-004 | M005/S01 | arch | Worker state sync | Worker routes on its own PcbEngine copy, posts snapshot back via postMessage. Main thread replaces its cached snapshot with worker's result. | Worker can't share WASM memory with main thread. get_snapshot() returns JsValue — directly structuredClone-able via postMessage. Main thread engine stays for non-routing ops (query_point, add_trace). | No |
| D-M005-005 | M005/S01 | arch | Worker lifecycle | Fresh worker per route — terminate on cancel, spawn new for next route | Persistent worker complicates cancel (terminate kills WASM state). WASM init is ~100ms, negligible vs routing time. Simple, reliable. | Yes — if init overhead becomes measurable |
| D-M005-006 | M005/S01 | pattern | Worker bundling | Vite `new Worker(new URL('./routing-worker.ts', import.meta.url), { type: 'module' })` | Standard Vite pattern for ES module workers. Handles WASM asset URLs correctly. No special plugin needed. | No |
| D-FUTURE-001 | future | architecture | Multi-file .cypcb project structure | Deferred — design needed | Single-file editor doesn't scale for larger PCB projects. Want multi-file support: folder of .cypcb files = one project (e.g. components.cypcb, nets.cypcb, board.cypcb). Enables: splitting components from nets, reusable component libraries, JLCPCB Insert targeting specific file, better organization for complex boards. Needs design for: file resolution order, cross-file references, editor tabs, project manifest. Impacts parser, editor, file-access, project-manager. | Yes — next major version |

## UI/UX Decisions

| # | When | Scope | Decision | Choice | Rationale | Revisable? |
|---|------|-------|----------|--------|-----------|------------|
| D-UI-001 | 2026-03-21 | ui | Toolbar button styling for Route | Same transparent `.tb-btn` style as other buttons, no filled/colored bg in rest state | Green filled button was visually inconsistent with icon-only toolbar. Color only for active `.routing` state (yellow `--warning`). | No |
| D-UI-002 | 2026-03-21 | ui | Route button SVG icon removed | Text-only "Route" label, no SVG arc icon | SVG arc icon next to "Route" text was redundant — the label is self-explanatory. Other toolbar buttons are icon-only (no labels), Route has label (no icon). | Yes |
| D-UI-003 | 2026-03-21 | ui | JLCPCB search button in toolbar | Magnifying glass icon in toolbar (before settings gear) | Previously hidden (`class="hidden"`) and only accessible via Ctrl+J. Search is a primary action — needs visible affordance. | No |
| D-UI-004 | 2026-03-21 | ui | "Open File" → "Import File" label | Button labeled "Import File" instead of "Open File" | Uploading a file from disk is importing. "Open" refers to opening existing projects from workspace/recent list. Clearer mental model. | Yes |
| D-UI-005 | 2026-03-21 | ui | Recent files as "Your Projects" with grid cards | Grid layout with thumbnail cards instead of compact list | Recent files are the user's projects. Card grid with large thumbnails matches template cards above and makes boards visually identifiable. | No |
| D-UI-006 | 2026-03-21 | ui | Thumbnail refresh on PM open | `onRefreshThumbnail` regenerates thumbnail from current snapshot every time PM opens | Solves LCSC fetch timing — first thumbnail may be generated before footprints load. Refreshing on PM open ensures latest snapshot is used. | No |
| D-UI-007 | 2026-03-21 | arch | Workspace file list via WebSocket | `list-files` WS message returns `.cypcb` files from server watch directory | Only available with dev server (`server.ts`). Standalone Vite has no WS — workspace section hidden, falls back to localStorage recent files. | Yes — could add File System Access API `showDirectoryPicker` for web-only |
| D-UI-008 | 2026-03-21 | arch | `reloadAfterLcscFetch()` shared helper | Single helper replaces 3 duplicated blocks of post-LCSC-fetch logic | DRY — reload engine, pull snapshot, force render, regenerate thumbnail, update 3D. All 3 call sites (template, recent, editor debounce) use same helper. | No |

## DRC Decisions

| # | When | Scope | Decision | Choice | Rationale | Revisable? |
|---|------|-------|----------|--------|-----------|------------|
| D-DRC-001 | 2026-03-21 | arch | Per-pad ECS entities for DRC clearance | Each component pad spawned as separate entity with `PadInstance` + `NetId` + `Position` | KiCad pattern: each copper item has its own net. Component-level NetConnections is too coarse — can't distinguish VCC pad from GND pad for same-net exemption. | No |
| D-DRC-002 | 2026-03-21 | arch | Tight rotated AABB for pad spatial entries | `half_x = |cos|*hw + |sin|*hh` instead of `max(hw, hh)` square | Square AABB caused false violations for rectangular pads (SOIC-8: 1.5×0.6mm at 1.27mm pitch). Tight AABB eliminates false-positive overlaps. | No |
| D-DRC-003 | 2026-03-21 | arch | Trace/via entities carry NetId as separate ECS component | `spawn_entity((trace, net_id))` tuple bundle | Clearance check queries `(Entity, &NetId)` — without separate NetId component, trace is invisible to same-net exemption. | No |
| D-DRC-004 | 2026-03-21 | arch | DesignRules expanded with 5 new fields | Added `min_via_diameter`, `min_hole_to_hole`, `min_solder_mask_bridge`, `min_silk_clearance`, `min_courtyard_clearance` | Needed by new DRC rules: hole-to-hole, via diameter, courtyard clearance. Solder mask and silk are stubs awaiting geometry data but rules/presets/infra are in place. | No |
| D-DRC-005 | 2026-03-21 | arch | 12 DRC rules registered (10 active + 2 stubs) | clearance, edge_clearance, connectivity, drill_size, trace_width, annular_ring, keepout, hole_to_hole, via_diameter, courtyard_clearance + solder_mask_bridge(stub) + silk_clearance(stub) | Covers critical manufacturing rules. Stubs for solder mask and silk have infrastructure ready — implement when geometry is modeled. | Yes — add more as needed |
| D-DRC-006 | 2026-03-21 | arch | Silk clearance checked in JS, not WASM | `checkSilkClearance()` in wasm.ts, merged with WASM violations | Silk shapes exist only in JS snapshot (EasyEDA data). Moving to Rust requires adding silk geometry to ECS — deferred. | Yes — move to Rust when silk modeled |
| D-DRC-007 | 2026-03-21 | arch | Routing clearance from engine, not hardcoded | `RoutingState.clearanceNm` set from `engine.get_min_clearance_nm()` at route start | Ensures routing preview matches DRC rules. Changing preset changes routing behavior immediately. | No |
| D-DRC-008 | 2026-03-21 | arch | DRC violation messages enriched post-check | `enrich_violation_messages()` adds entity names (refdes, net, pad parent) to raw violation messages | Raw messages had only entity IDs. Users need "trace 'VCC' ↔ pad on R1" not "entity #5 ↔ entity #12". | No |
| D-DRC-009 | 2026-03-21 | arch | DRC triggers on rotate_component and set_board_size | Both methods rebuild spatial index and run DRC after mutation | Previously missing — courtyard/clearance/edge violations wouldn't update after component rotation or board resize. | No |
| D-DRC-010 | 2026-03-21 | ui | DRC panel shows structured violation entries | Icon + title + detail + entity labels + coordinates per violation, click-to-zoom | Raw `[clearance] message` was unreadable. New format: ⚡ Copper clearance / "Items touching — need 0.15mm gap" / `trace 'VCC' ↔ pad on R1` / (x, y) mm. | No |
| D-DRC-011 | 2026-03-21 | ui | Violation markers on board disabled | `drawViolation()` exists but not called from render loop | Red circles on board clutter the view and obscure copper geometry. DRC feedback via error badge + panel + click-to-zoom is sufficient. | Yes — revisit with better marker design |
| D-DRC-012 | 2026-03-21 | arch | Courtyard entries in spatial index use layer_mask=0 | Component courtyard AABB has layer_mask=0, pad AABB has proper copper layer mask | Copper clearance check uses `layers_overlap()` filter — layer_mask=0 entries never match copper checks. Courtyard entries only used by CourtyardClearanceRule which filters for layer_mask==0. | No |

### Status of the DRC decisions, read against the code on 2026-08-06

The table above records what was decided. This records what is true, because
four of the twelve are not, and one of them caused a rule to report nothing
for months.

- **D-DRC-001, per-pad ECS entities: not built.** The spatial index still holds
  one courtyard box per component. What exists instead is a narrow phase that
  resolves a component to its pad geometry inside `ClearanceRule`
  (`component_pads` in `crates/cypcb-drc/src/rules/clearance.rs`), including
  the per-pad net that D-DRC-001 wanted the entities for - so the behaviour the
  decision was after is there, without the entities. See KNOWLEDGE.md K010.
- **D-DRC-002, tight rotated AABB: true on both sides since 2026-08-28.** The
  checker does it, and so does the router: `populate_pads` marks the pad's own
  rectangle at the part's rotation through `mark_pad_rect_at_nm`, and the
  shipped `AutorouteConfig::default().pad_rect_extra_cells` is `Some(2)`, so
  every route `cypcb route` runs takes that path. The disc of `max(w, h) / 2`
  survives as the `None` arm nobody ships. This entry said the router still
  drew discs until 2026-09-05 - the same sentence K011 carried, gone stale the
  same way, on a second page. See K011.
- **D-DRC-005, 12 rules with 2 stubs: superseded.** **37 rules are registered
  as of 2026-09-05** and none is a stub -
  `grep -c "Box::new(rules::" crates/cypcb-drc/src/lib.rs`. This section said
  fifteen when it was written on 2026-08-06 and nobody re-read it, so the
  number is held by
  `crates/cypcb-cli/tests/the_decision_log_is_current_where_it_says_it_is.rs`
  now rather than by the next person to notice. `via_drill`, `trace_current`
  and `assertion` joined after the decision was written, and the two stubs it
  named were implemented.
- **D-DRC-006, silk clearance in JS not WASM: superseded.** There is a Rust
  rule, `crates/cypcb-drc/src/rules/silk_clearance.rs`, and footprints carry
  silk geometry in the ECS. The JS check still sees artwork that arrives with
  an EasyEDA fetch, which the engine has no model for; that is the remaining
  gap, not the whole decision.
- **D-DRC-012, courtyard entries at `layer_mask = 0`: never implemented, and
  it cost a rule.** Every index builder in the crate marks components
  `0xFFFFFFFF`. `CourtyardClearanceRule` filtered the index for
  `layer_mask == 0` exactly as this decision describes, matched nothing on
  every board it ever ran against, and reported zero. It reads the components
  directly now. A decision recorded and not implemented is worse than one
  never made, because the code downstream trusts it.
