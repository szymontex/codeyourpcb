# S03: Routing UX Upgrade — Research

**Researched:** 2026-03-13
**Domain:** Interactive PCB routing UX — Canvas 2D, state machine, hit-testing
**Confidence:** HIGH

<research_summary>
## Summary

Explored the full routing pipeline: `routing.ts` (state machine, 433 LOC), `interaction.ts` (mouse handlers, 471 LOC), `renderer.ts` (drawRoutingPreview, 959 LOC), `main.ts` (wiring, 1634 LOC), plus supporting modules (`hit-test.ts`, `render-config.ts`, `types.ts`, `layers.ts`, `wasm.ts`).

The routing state machine is well-structured (idle → routing → idle) with pad hit-testing, grid snap, angle snap, DRC preview, and trace commit via undo stack. But the UX is missing four critical pieces that the slice must deliver: (1) net-aware target pad highlighting during routing, (2) ratsnest-as-guide showing nearest unconnected pad, (3) magnetic snap to destination pads, and (4) angle constraint as toggleable option instead of always-on. All the building blocks exist — `buildPadNetMap()`, `RatsnestInfo` in snapshot, pad world-position math, the highlighting system from S01 — they just need to be wired into the routing flow.

**Primary recommendation:** Extend the existing routing state machine with net-context fields, add keyboard handler for angle toggle, wire ratsnest filtering into the routing preview draw pass, and add magnetic snap as a distance threshold in `updatePreview()`. No new libraries needed. No architecture changes. Pure feature additions on solid foundations.
</research_summary>

<codebase_analysis>
## Codebase Analysis

### What Exists (and works)

| Module | LOC | Key Functions | Status |
|--------|-----|---------------|--------|
| `routing.ts` | 433 | `startRoute`, `updatePreview`, `addWaypoint`, `completeRoute`, `cancelRoute`, `flipLayer`, `hitTestPad`, `computeSnappedPoint`, `snapToGrid`, `createDrcPreviewChecker` | Solid state machine. Grid+angle snap work. DRC preview works. |
| `interaction.ts` | 471 | `setupInteraction` — mouse handlers for pan, zoom, click→route, resize drag | Click-to-start and click-to-complete routing wired. Hover drives preview. |
| `renderer.ts` | 959 | `render()`, `drawRoutingPreview()`, `drawPad()`, `drawTrace()`, `drawRatsnest()` | Preview draws committed+dashed segments. Pad net highlighting from S01 works via `highlightedNet` in RenderState. |
| `render-config.ts` | 128 | `RenderConfig`, `LodTier`, `buildPadNetMap()`, `getLodTier()` | Pad-to-net map used for highlight. S03 consumes this. |
| `main.ts` | 1634 | Wiring: render loop builds `RenderState`, interaction callbacks route to undo stack | `highlightedNet` set on trace click, cleared on deselect. Not set during routing. |
| `types.ts` | 120 | `BoardSnapshot`, `RatsnestInfo`, `NetInfo`, `PinRef` | `RatsnestInfo` has `start_x/y`, `end_x/y`, `net_name`. Ready to use. |
| `hit-test.ts` | 67 | `hitTestTrace()` | Trace hit-testing for selection. |
| `wasm.ts` | ~900 | `PcbEngine` interface, mock engine | `add_trace`, `remove_trace`, `get_snapshot`, `run_drc_incremental` all work. |

### What's Missing (slice scope)

1. **Net-aware target pad highlighting during routing** — When routing starts on a pad, all other pads on the same net should glow as valid destinations. Currently `highlightedNet` is only set when clicking a trace (in `onTraceSelect`), never during routing.

2. **Ratsnest as routing guide** — `RatsnestInfo` lines exist in `BoardSnapshot.ratsnest` and `drawRatsnest()` renders them. But during routing, the user needs to see which ratsnest line leads to the nearest unconnected pad on the active net. Currently ratsnest draws all lines uniformly with no filtering by active net.

3. **Magnetic snap to destination pad** — When the cursor approaches a target pad (same net), it should snap to that pad's center. Currently `updatePreview()` only does grid snap → angle snap. No pad proximity check.

4. **Angle constraint toggle** — `computeSnappedPoint()` is always called in `updatePreview()`. No way to disable it. The slice must make angle snap toggleable (keyboard shortcut, default off per roadmap — "optional toggle, not forced").

5. **Keyboard handlers** — Zero `keydown` listeners in the entire viewer. Need Escape (cancel route), F (flip layer — function exists but not wired), angle toggle key, and potentially snap toggle.

6. **Routing E2E tests** — Zero tests for routing. Need: start route on pad, complete route on target pad, verify trace added, verify net highlighting during route, verify cancel.

7. **Routing diagnostic surface** — `__routingState` debug surface exists (mode, anchorPoint, snapAngle, netName, layer, segments, violations) but no E2E tests consume it.

### Integration Points (from S01)

- `buildPadNetMap()` → ready, builds `Map<"refdes.pin", netName>` from `NetInfo.connections`
- `RenderState.highlightedNet` → exists, wired through to `drawPad()` and `drawTrace()` glow/dim
- `pullSnapshot()` → centralized snapshot refresh, rebuilds padNetMap
- `window.__renderDiag.highlightedNet` → exposed for E2E verification
- `window.__routingState` → exposed for E2E verification of routing mode, net, etc.
- `window.__loadBoard(source)` → E2E board loading helper

### Types Available

```typescript
// Already in types.ts — ratsnest is the key type for routing guide
interface RatsnestInfo {
  start_x: number;  // unrouted connection start (nm)
  start_y: number;
  end_x: number;    // unrouted connection end (nm)
  end_y: number;
  net_name: string;  // which net this unrouted line belongs to
}

// Already in routing.ts — PadHit carries everything needed
interface PadHit {
  component: ComponentInfo;
  pad: PadInfo;
  worldX: number;  // pad center world coords (nm)
  worldY: number;
  netName: string;
}
```

### State Machine Current Flow

```
IDLE
  └─ click pad with netName → startRoute() → ROUTING
      ├─ mousemove → updatePreview() (grid snap → angle snap → dashed preview)
      ├─ click empty → addWaypoint() (commit segment, new anchor)
      ├─ click pad → completeRoute() → add trace → IDLE
      └─ (no Escape handler — missing!)
```

### State Machine Target Flow (after S03)

```
IDLE
  └─ click pad with netName → startRoute() → ROUTING
      ├─ SET highlightedNet = startPad.netName (target pads glow)
      ├─ FILTER ratsnest to show only active net lines (routing guide)
      ├─ mousemove → updatePreview():
      │   ├─ grid snap (if enabled)
      │   ├─ magnetic snap (if near target pad, snap to center)
      │   ├─ angle snap (if enabled — toggleable, default OFF)
      │   └─ dashed preview + snap indicator
      ├─ click empty → addWaypoint()
      ├─ click target pad → completeRoute() → add trace → CLEAR highlight → IDLE
      ├─ Escape → cancelRoute() → CLEAR highlight → IDLE
      ├─ F → flipLayer()
      └─ A → toggle angle constraint
```
</codebase_analysis>

<implementation_approach>
## Implementation Approach

### Task Breakdown (5 logical units)

**T01: Routing state machine upgrade + keyboard handlers**
- Add `angleSnapEnabled` field to `RoutingState` (default: `false`)
- Add `magneticSnapEnabled` field (default: `true`)
- Add `magneticSnapRadius` field (default: `1_000_000` nm = 1mm)
- Modify `updatePreview()`: check target pad proximity before angle snap
- Make `computeSnappedPoint()` conditional on `angleSnapEnabled`
- Add `toggleAngleSnap(state)` function
- Add keyboard listener in `main.ts` or `interaction.ts`:
  - `Escape` → cancel route
  - `F` → flip layer (already have `flipLayer()`)
  - `A` → toggle angle snap
- Unit tests for new state transitions

**T02: Net-aware highlighting during routing**
- When `startRoute()` fires, set `highlightedNet` on `RenderState` to the active net
- When route completes or cancels, clear `highlightedNet`
- This automatically activates existing pad glow/dim from S01's `drawPad()`
- Existing trace glow/dim from `drawTrace()` also lights up
- Wire in `interaction.ts` callbacks → `main.ts` render state update
- Ratsnest filtering: during routing, emphasize ratsnest lines matching active net (brighter/thicker), dim others

**T03: Magnetic snap to destination pads**
- In `updatePreview()`, after grid snap but before angle snap:
  - Scan all pads in snapshot for same-net pads (use padNetMap)
  - If cursor is within `magneticSnapRadius` of a target pad center, snap to it
  - Set a flag in routing state (`snappedToPad: PadHit | null`) for visual feedback
- Renderer draws magnetic snap indicator (crosshair / ring on target pad)
- Needs efficient pad lookup — current `hitTestPad()` scans all pads but only returns closest. Can reuse with net filter.

**T04: Routing E2E tests**
- New `viewer/e2e/routing-ux.spec.ts`
- Load routing-test.cypcb (3 nets, 3 components — simple enough for deterministic tests)
- Test: start route on pad → verify `__routingState.mode === 'routing'`
- Test: complete route pad-to-pad → verify trace added via `__pcbEngine.trace_count()`
- Test: cancel route with Escape → verify mode back to idle
- Test: net highlighting during routing → verify `__renderDiag.highlightedNet` set
- Test: angle toggle → verify `__routingState.angleSnapEnabled` toggles
- All tests use diagnostic surfaces, no pixel comparison

**T05: Routing unit tests**
- Test `updatePreview` with magnetic snap
- Test `updatePreview` with angle snap off (free-form)
- Test `toggleAngleSnap` state transition
- Test `completeRoute` with magnetic-snapped point

### Key Design Decisions to Make

1. **Magnetic snap vs angle snap priority** — When cursor is near a target pad AND angle snap is on, which wins? Recommendation: magnetic snap wins. If you're close to the target, you want to hit it. This matches KiCad behavior.

2. **Angle snap default** — Roadmap says "optional toggle (not forced)". Current code forces it always on. Change default to `false`. User toggles with `A` key.

3. **Ratsnest guide rendering** — Two options: (a) filter ratsnest to only show active net lines, (b) show all but emphasize active net. Recommendation: (b) — keep spatial context, just brighten active net lines and dim the rest.

4. **Magnetic snap radius** — 1mm world-space is generous. KiCad uses a grid-dependent snap radius. Start with fixed 1mm, S04 can make it configurable.

5. **Keyboard scope** — Add listeners on `document` or `canvas`? Canvas-scoped is cleaner (doesn't interfere with editor). But canvas doesn't receive keyboard events without focus. Recommendation: `document`-level with routing mode guard — only intercept keys when `routingState.mode === 'routing'`, plus `R` to start route mode from idle.

### Files to Create/Modify

| File | Action | What Changes |
|------|--------|-------------|
| `viewer/src/routing.ts` | MODIFY | Add `angleSnapEnabled`, `magneticSnapEnabled`, `magneticSnapRadius`, `snappedToPad` to RoutingState. New `toggleAngleSnap()`, `findNearestTargetPad()`. Update `updatePreview()` with magnetic snap + conditional angle snap. |
| `viewer/src/interaction.ts` | MODIFY | Add keyboard listener for Escape/F/A during routing. |
| `viewer/src/renderer.ts` | MODIFY | Update `drawRoutingPreview()` with magnetic snap indicator. Update ratsnest drawing to emphasize active net during routing. |
| `viewer/src/main.ts` | MODIFY | Set `highlightedNet` when routing starts/ends. Wire keyboard events. Expose new routing debug fields. |
| `viewer/src/__tests__/routing.test.ts` | NEW | Unit tests for routing state machine. |
| `viewer/e2e/routing-ux.spec.ts` | NEW | E2E tests for routing flow. |
</implementation_approach>

<risks_and_constraints>
## Risks & Constraints

### Risk 1: Pad Hit-Testing Performance During Routing
**What:** `updatePreview()` runs on every mousemove (throttled by rAF). If we add target pad scanning for magnetic snap, iterating all pads on every move could be expensive for boards with 500+ components.
**Severity:** Medium
**Mitigation:** Pre-compute target pad positions once when routing starts (filter pads by net, compute world positions, store as array). Scan only target pads during move — for a typical net that's 2-10 pads, not 500+.

### Risk 2: Keyboard Event Conflicts with Monaco Editor
**What:** Document-level keyboard listeners for routing (Escape, F, A) could interfere with the Monaco editor when it's focused.
**Severity:** Medium
**Mitigation:** Guard keyboard handlers: only intercept when `routingState.mode === 'routing'` OR when canvas has focus. Editor panel has its own keyboard handling through Monaco. Check `document.activeElement` before intercepting.

### Risk 3: E2E Canvas Click Targeting in Headless
**What:** Routing E2E tests need to click on specific pads, which are small Canvas shapes. In headless Chromium, coordinate mapping can be unreliable.
**Severity:** Medium
**Mitigation:** Use `__loadBoard()` to load routing-test.cypcb with known component positions. Calculate pad screen coordinates via `page.evaluate()` using viewport's `worldToScreen()`. Fit board first for deterministic viewport. Use generous hit tolerance (already 0.5mm in code). Verify via diagnostic surface, not pixel comparison.

### Risk 4: Ratsnest Guide Accuracy
**What:** `RatsnestInfo` in the snapshot may not update after each routing action (e.g., after completing one trace of a multi-pad net). If ratsnest is stale, the guide leads to already-connected pads.
**Severity:** Low
**Mitigation:** `pullSnapshot()` refreshes the entire snapshot including ratsnest after each route completion. During a multi-segment route (waypoints), ratsnest shows the state at route-start, which is correct — the route isn't committed yet.

### Risk 5: Angle Snap Default Change
**What:** Changing angle snap default from `true` to `false` changes existing behavior. Users who relied on forced angle snap will see different routing behavior.
**Severity:** Low (beta product, no users relying on this yet)
**Mitigation:** Clear keyboard shortcut hint in status bar during routing: "A: angle snap · F: flip layer · Esc: cancel"
</risks_and_constraints>

<requirements_analysis>
## Requirements Analysis

### Requirements This Slice Directly Delivers

From the milestone definition:
> "User can route a trace pad-to-pad with: target pad highlighting, magnetic snap to destination, ratsnest as guide, and angle constraint as optional toggle (not forced)"

This maps to:
1. **Target pad highlighting** — set `highlightedNet` during routing, S01 glow/dim system does the rest
2. **Magnetic snap to destination** — new proximity check in `updatePreview()`, snap to pad center
3. **Ratsnest as guide** — emphasize active net ratsnest lines during routing
4. **Angle constraint as optional toggle** — add `angleSnapEnabled` field + `A` keyboard toggle, default `false`

### Requirements Indirectly Advanced

- **EDIT-05 (code folding)** — N/A
- **UI-09 (canvas renderer theme syncs)** — ratsnest and routing preview respect theme colors (already do)

### Success Criteria (from M003 Roadmap)

> "Routing flow verified by E2E test: route from pad A to pad B with net highlight and magnetic snap"

E2E test plan directly targets this: load routing-test.cypcb, click pad, verify highlighting, route to target, verify trace added.

### Boundary Outputs (for S07)

- Routing interaction with net highlighting, magnetic snap, angle toggle
- Testable routing state machine (start → guide → snap → place)
- `__routingState` debug surface extended with `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad`
- E2E tests in `routing-ux.spec.ts` for S07 to extend
</requirements_analysis>

<dont_hand_roll>
## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Spatial indexing for pad proximity | R-tree or quadtree for pad lookup | Simple filtered array scan | With ≤10 target pads per net, linear scan is O(10) per frame — no spatial index needed. Over-engineering for PCB pad counts. |
| Complex routing algorithm | Maze router / push-and-shove | Existing click-to-place state machine | S03 is UX, not routing algorithm. Autorouter exists in `cypcb-autoroute` for algorithmic routing. Manual routing = human decides path. |
| Custom keyboard shortcut framework | keybind manager, shortcut registry | Direct `addEventListener('keydown')` with guards | Only 3-4 keys to handle. Framework overhead not justified. |
| Ratsnest computation | Client-side ratsnest calculation | Engine's `get_snapshot().ratsnest` | Ratsnest is computed in Rust engine and included in BoardSnapshot. Don't recompute client-side. |
</dont_hand_roll>

<common_pitfalls>
## Common Pitfalls

### Pitfall 1: Stale Pad-Net Map During Routing
**What goes wrong:** `buildPadNetMap()` uses snapshot at route-start. If another process modifies the board mid-route, the map is stale.
**Why it happens:** `padNetMap` is rebuilt in `pullSnapshot()` which only runs on route completion, not during.
**How to avoid:** For S03, this is acceptable — manual routing doesn't concurrently modify nets. Just document it.
**Warning signs:** Target pad highlighting shows wrong pads.

### Pitfall 2: World-to-Screen Coordinate Drift
**What goes wrong:** Magnetic snap radius calculated in world coords doesn't match expected screen behavior at different zoom levels.
**Why it happens:** A 1mm world radius is tiny at zoom-out (1px) and huge at zoom-in (200px).
**How to avoid:** Use dual threshold: world radius (1mm) OR screen-pixel radius (min 15px), whichever is larger. This gives consistent UX at any zoom.
**Warning signs:** Snap feels too aggressive at close zoom, too weak at far zoom.

### Pitfall 3: Angle Snap + Magnetic Snap Conflict
**What goes wrong:** When both are enabled and cursor is near a target pad, angle snap pulls the endpoint away from the pad center.
**Why it happens:** Angle snap forces 45°/90° angles regardless of pad position.
**How to avoid:** Magnetic snap takes absolute priority when target pad is within radius. Skip angle snap for that segment.
**Warning signs:** Route "almost" reaches the target pad but misses by a few pixels.

### Pitfall 4: E2E Pad Click Precision
**What goes wrong:** Headless test clicks at computed pad coordinates but misses the pad.
**Why it happens:** Viewport state after `fitBoard()` may differ slightly between runs, or the board canvas bounding rect includes toolbar offsets.
**How to avoid:** Use `page.evaluate()` to compute exact screen coordinates from viewport state. Add generous PAD_HIT_TOLERANCE. Verify via `__routingState.mode` not pixel color.
**Warning signs:** E2E test flakes — sometimes enters routing, sometimes doesn't.
</common_pitfalls>

<skill_suggestions>
## Skill Suggestions

### Relevant Skills Found

1. **`currents-dev/playwright-best-practices-skill@playwright-best-practices`** — 9.2K installs
   - Potentially useful for E2E test patterns, but our existing test patterns (diagnostic surfaces, `__loadBoard`) are already well-established and specific to Canvas testing.
   - Install: `npx skills add currents-dev/playwright-best-practices-skill@playwright-best-practices`

2. **`tscircuit/skill@tscircuit`** — 157 installs
   - EDA/PCB circuit skill. Unlikely to be directly useful since CodeYourPCB has its own architecture.

3. **`l3wi/claude-eda@eda-pcb`** — 56 installs
   - Generic EDA/PCB skill. Same — our codebase is specific enough that generic EDA guidance adds little.

### Recommendation

None of these skills are worth installing for S03. The routing UX work is tightly coupled to our existing codebase patterns. The Playwright skill has high install count but our Canvas-specific E2E patterns (diagnostic surfaces, evaluate-based coordinate math) are more specialized than what a generic Playwright skill would provide.
</skill_suggestions>

<open_questions>
## Open Questions

### 1. Snap feedback visual design
**What we know:** KiCad shows a circle/diamond indicator when snapped to a pad. We need something similar in our Canvas renderer.
**What's unclear:** Exact visual style — circle, crosshair, diamond? Color matching the net color?
**Recommendation:** Use a pulsing circle at the target pad center in the net color, similar to existing anchor point dot (white circle with net-colored stroke) but slightly larger. Implement and iterate.

### 2. Ratsnest line to nearest unconnected pad
**What we know:** `RatsnestInfo` has `start_x/y`, `end_x/y`, `net_name`. During routing, we want to draw the ratsnest line from the current cursor/anchor to the nearest unconnected target pad.
**What's unclear:** Does the engine's ratsnest already compute this from the current routing anchor, or does it compute from the original pad positions? Most likely the latter — ratsnest is computed once at snapshot time, not live during routing.
**Recommendation:** During routing, draw a lightweight "guide line" from the current anchor to the nearest unconnected pad on the active net (computed client-side from pad positions), in addition to the standard ratsnest. This gives the user directional guidance without requiring engine-side ratsnest recalculation.

### 3. Multi-pad net routing sequence
**What we know:** A net like VCC might connect 5 pads. The user routes one connection at a time.
**What's unclear:** After completing one route in a net, should the system automatically highlight the next unconnected pad as a target?
**Recommendation:** For S03, keep it simple: after completing a route, clear highlighting and return to idle. The user clicks the next pad to start a new route. The ratsnest lines already show remaining unconnected pins. Auto-continue can come in S07 polish.
</open_questions>

<sources>
## Sources

### Primary (HIGH confidence)
- Direct codebase exploration of `routing.ts`, `interaction.ts`, `renderer.ts`, `render-config.ts`, `main.ts`, `types.ts`, `hit-test.ts`, `wasm.ts`, `layers.ts`
- S01-SUMMARY.md — pad highlighting, padNetMap, RenderConfig, diagnostic surfaces
- S02-SUMMARY.md — 3D debug surface pattern, geometry count tracking
- DECISIONS.md — existing routing decisions (grid before angle snap, onTraceAdd callback, net highlight glow parameters)
- Existing E2E tests in `viewer/e2e/` — established patterns for Canvas testing via diagnostic surfaces

### Secondary (MEDIUM confidence)
- M003-ROADMAP.md boundary map — S01→S03 contract (pad highlighting, RenderConfig), S03→S07 contract (testable routing state machine)
- M003-CONTEXT.md — routing UX pain points, KiCad reference behavior
</sources>

<metadata>
## Metadata

**Research scope:**
- Core: routing.ts state machine, interaction.ts mouse handlers, renderer.ts preview drawing
- Supporting: render-config.ts (pad-net map, LOD), types.ts (RatsnestInfo, BoardSnapshot), wasm.ts (engine API)
- Patterns: existing diagnostic surface pattern, E2E board loading, pad hit-testing
- Risks: headless E2E precision, keyboard conflicts, snap radius scaling

**Confidence breakdown:**
- Codebase analysis: HIGH — read every relevant file completely
- Implementation approach: HIGH — building on proven patterns (S01 highlighting, existing routing state machine)
- Risks: HIGH — identified from concrete code paths, not speculation
- E2E strategy: HIGH — proven patterns from renderer-quality.spec.ts and three-d-view.spec.ts

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (30 days — all findings are codebase-specific, no external dependency drift)
</metadata>

---

*Slice: S03 — Routing UX Upgrade*
*Parent: M003 — From Prototype to Tool*
*Research completed: 2026-03-13*
*Ready for planning: yes*
