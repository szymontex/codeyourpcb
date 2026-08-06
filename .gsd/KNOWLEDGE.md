# Knowledge Base — CodeYourPCB

Append-only register of project-specific rules, patterns, and lessons learned.

---

### K001: Toolbar button styling consistency
**Context:** Route button was originally a filled green button (`background: var(--success)`) standing out from the rest of the icon-only toolbar.
**Rule:** All toolbar buttons must use `.tb-btn` base style (transparent bg, no border). No filled/colored buttons in resting state. Color only for active states (e.g. `.routing` uses `--warning`).
**Applies to:** `viewer/index.html` toolbar CSS, any new toolbar button.

### K002: LCSC footprint fetch is async — thumbnails must handle timing
**Context:** Components with `lcsc "CXXXXXX"` attributes have NO pads in the initial snapshot. Pads only appear after async EasyEDA API fetch completes (`autoFetchLcscFootprints()`). If a thumbnail is generated before fetch, it shows an empty board.
**Rule:** Always regenerate thumbnails after LCSC fetch completes. Use `reloadAfterLcscFetch()` helper in the fetch `.then()` callback. Additionally, `onRefreshThumbnail` regenerates from current snapshot when PM opens — catches any timing edge cases.
**Applies to:** `viewer/src/main.ts` (all `autoFetchLcscFootprints` call sites), `viewer/src/project-manager.ts`.

### K003: `const`/`let` temporal dead zone — `typeof` doesn't help
**Context:** `typeof interactionState !== 'undefined'` does NOT protect against TDZ for `const`/`let` declarations. It still throws `ReferenceError`.
**Rule:** Use a separate `let interactionReady = false` flag set after initialization. Never rely on `typeof` for TDZ checking of `const`/`let` variables.
**Applies to:** `viewer/src/main.ts` `resize()` function.

### K004: Vite cache can serve stale transforms after edits
**Context:** Vite's `node_modules/.vite` cache sometimes holds stale transformed files, causing compile errors for code that no longer exists in source.
**Rule:** When encountering phantom compile errors ("await can only be used in async function" on a line with no await), delete `node_modules/.vite` and restart Vite.
**Applies to:** Any Vite dev server debugging.

### K005: WebSocket dev server vs standalone Vite
**Context:** `npx vite` runs frontend only (port 5173). `npx tsx server.ts` runs WS server (4322) + file watcher + Vite (port 4321). Features like workspace file listing require the WS server.
**Rule:** Features must work in both modes. Use graceful degradation — if WS is not connected, hide WS-dependent sections rather than showing errors. Project manager hides "Workspace" section when no WS, shows "Your projects" (localStorage-based) which works everywhere.
**Applies to:** `viewer/src/main.ts` WS callbacks, `viewer/src/project-manager.ts`.

### K006: Offscreen thumbnail render size matters
**Context:** Original 200×150 thumbnails had components too small to see (sub-pixel on small displays). Board takes most of the thumbnail area, leaving pads at 2-3 pixels.
**Rule:** Thumbnail render size is 400×300px. CSS displays at full card width with `aspect-ratio: 4/3`. This gives enough resolution for component pads to be clearly visible.
**Applies to:** `viewer/src/project-manager.ts` `generateThumbnail()`.

### K007: Port conflicts when running dev server
**Context:** `server.ts` spawns Vite as child process. If a previous Vite or server is still running, it fails with "Port XXXX already in use".
**Rule:** Kill existing processes on ports 4321, 4322, 5173 before starting dev server. Check with `netstat -tlnp | grep -E "4321|4322|5173"`.
**Applies to:** Development workflow.

### K008: WS connection timing vs showProjectManager
**Context:** `showProjectManager()` is called during `init()` before `connectWebSocket()` assigns `wsConnection`. Calling `wsConnection?.send()` in PM callbacks at init time sends nothing (wsConnection is null).
**Rule:** WS-dependent features (like file listing) should be triggered from `onConnect` callback (fires when WS actually connects), not from `showProjectManager()`. The `onConnect` callback auto-sends `list-files` request.
**Applies to:** `viewer/src/main.ts` WS + project manager integration.

### K009: Split-button pattern for Route
**Context:** Route has a split-button — main button triggers routing, caret opens dropdown with options (auto-route toggle, tuning sliders).
**Rule:** Keep the split-button group (`.tb-route-group`). Main button = action, caret = options. The caret dropdown `#route-menu-dropdown` contains auto-route checkbox + 4 tuning sliders (via cost, layer preference, roundness, density). Sliders debounce at 300ms.
**Applies to:** `viewer/index.html`, `viewer/src/main.ts`.

### K010: DRC per-pad entity architecture (KiCad pattern)
**Context:** DRC clearance check compares copper features pair-by-pair. Components are single ECS entities with `NetConnections` (multiple nets). If only component-level AABB is in spatial index, same-net exemption fails because component has multiple nets and clearance check can't determine which pad is involved.
**Rule:** Each pad of a placed component must have its own ECS entity with `PadInstance` marker + `NetId` + `Position`. The spatial index must have per-pad AABB entries (not per-component courtyard). Clearance check finds `NetId` on the pad entity directly → same-net exemption works correctly: trace on VCC near VCC pad = skip, trace on VCC near GND pad = violation. This mirrors KiCad's `BOARD_CONNECTED_ITEM::GetNet()` architecture.
**Applies to:** `crates/cypcb-render/src/lib.rs` (`populate_from_snapshot`, `rebuild_spatial_index_full`), `crates/cypcb-drc/src/rules/clearance.rs`.
**Status 2026-08-06:** half built, and the half that is missing is named here rather than implied. `ClearanceRule` resolves a component to its pad copper in its narrow phase (`component_pads` in `clearance.rs`), so a trace between two pads no longer reads as a short and each pad's own layers are respected. What does not exist is the per-pad entity: the spatial index still holds one courtyard box per component, and the same-net exemption is still per component, so a trace on GND is exempt from every pad of a part that has one GND pin - including its VCC pad. Matching `PadDef::number` against `PinConnection::pin` is what closes it.

### K011: Pad AABB must use rotated bounds, not max(half_w, half_h)
**Violated today in one place:** `RoutingGrid::populate_pads` (`crates/cypcb-autoroute/src/grid.rs`) still marks every pad as a circle of `max(w, h) / 2`. The checker uses rotated bounds, the router does not, and the two therefore disagree about where copper is.
**Context:** Rectangular pads (e.g. SOIC-8: 1.5mm × 0.6mm) have different dimensions per axis. Using `max(half_w, half_h)` as square AABB makes the pad look 1.5mm × 1.5mm, causing false clearance violations between adjacent pads at 1.27mm pitch (actual edge-to-edge 0.67mm, AABB edge-to-edge -0.23mm → overlap).
**Rule:** Compute tight AABB for rotated rectangle: `half_x = |cos|*hw + |sin|*hh`, `half_y = |sin|*hw + |cos|*hh`. This gives the minimum axis-aligned bounding box that fully contains the rotated pad.
**Applies to:** `crates/cypcb-render/src/lib.rs` `rebuild_spatial_index_full()` pad AABB computation.

### K012: Trace entities must have NetId as separate ECS component
**Context:** `Trace` struct has a `net_id` field, but clearance check queries `ecs.query::<(Entity, &NetId)>()` looking for `NetId` as a separate ECS component. If trace is spawned with only `spawn_entity(trace)`, `NetId` component is missing → trace invisible to same-net exemption → false DRC violations.
**Rule:** Always spawn traces and vias as `spawn_entity((trace, net_id))` — tuple bundle with both the data component and `NetId` as separate ECS component. Applies to `add_trace()`, `parse_route_segment()`, `parse_route_via()`.
**Applies to:** `crates/cypcb-render/src/lib.rs` all trace/via spawn sites.

### K013: DRC triggers — complete list of mutation points
**Context:** DRC must run after every mutation that changes copper geometry or board outline. Missing a trigger = stale violations.
**Rule:** DRC (`run_drc_internal()`) must be called after: `load_snapshot`, `add_trace`, `remove_trace`, `autoroute*`, `sync_from_source`, `rotate_component`, `set_board_size`. JS-side: all `BoardCommand.execute()` and `.undo()` methods call `engine.run_drc_incremental()`. When adding new mutation APIs, always add DRC trigger.
**Applies to:** `crates/cypcb-render/src/lib.rs`, `viewer/src/undo.ts`.

### K014: Silk clearance check runs in JS, not WASM
**Context:** Silk shapes (`SilkShape[]`) live only in JS snapshot — they come from EasyEDA footprint data parsed client-side. Rust ECS has no silk geometry.
**Rule:** Silk-to-pad clearance is checked in `checkSilkClearance()` in `viewer/src/wasm.ts`. Results are merged with WASM violations in `get_snapshot()`. When silk geometry is eventually modeled in Rust, this should move to the Rust DRC engine.
**Applies to:** `viewer/src/wasm.ts` `WasmPcbEngineAdapter.get_snapshot()`.

### K015: Routing clearance must come from DesignRules, not hardcoded
**Context:** Routing obstacle detection had `150_000` (0.15mm) hardcoded. If user changes DRC preset (e.g. JLCPCB 4-layer 0.1mm), routing would use wrong clearance.
**Rule:** Routing reads clearance from `state.routing.clearanceNm` which is set from `engine.get_min_clearance_nm()` at route start. Never hardcode clearance values in routing code.
**Applies to:** `viewer/src/routing.ts` `updatePreview()`, `viewer/src/interaction.ts` route start.

### K016: DRC violation messages enriched with entity names post-check
**Context:** Raw DRC violations only have entity IDs. Users need to see component names (R1, U1), net names (VCC, GND), and entity types (trace, via, pad).
**Rule:** `enrich_violation_messages()` in `run_drc()` post-processes all violations — builds RefDes, NetId→name, PadInstance→parent, Trace/Via→net lookups and prepends entity labels to messages. Format: `trace 'VCC' ↔ pad on R1: Clearance violation...`. JS `formatViolationDetail()` parses the enriched message to show human-readable panel entries.
**Applies to:** `crates/cypcb-drc/src/lib.rs` `enrich_violation_messages()`, `viewer/src/main.ts` `formatViolationDetail()`.
