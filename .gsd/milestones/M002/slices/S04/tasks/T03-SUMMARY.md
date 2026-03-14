---
id: T03
parent: S04
milestone: M002
provides:
  - Rust ComponentInfo extended with body_width_nm, body_height_nm, model_3d fields
  - build_snapshot() populates body dimensions from Footprint.bounds with pad bbox fallback
  - Component body boxes rendered in 3D with SMD/THT height differentiation and IC/passive coloring
  - Refdes sprite labels on component bodies
  - Keyboard shortcut '3' toggles 3D view
  - FPS counter in __renderer3d debug surface
key_files:
  - crates/cypcb-render/src/snapshot.rs
  - crates/cypcb-render/src/lib.rs
  - viewer/src/types.ts
  - viewer/src/renderer3d.ts
  - viewer/src/main.ts
key_decisions:
  - Component height heuristic: SMD=1.2mm, THT=5mm, detected by presence of drill pads
  - Material coloring by refdes prefix: U/IC → dark gray metallic, R/C/L/others → tan passive
  - Refdes labels as THREE.Sprite with CanvasTexture for camera-facing readability
  - FPS computed per-second in animate loop, logged every 5 seconds
patterns_established:
  - Sprite label pattern: canvas → CanvasTexture → SpriteMaterial → Sprite for text labels in 3D
  - Pad bbox fallback: when footprint bounds are zero, compute from pad extents
observability_surfaces:
  - "window.__renderer3d.fps — current FPS (updated per second)"
  - "window.__renderer3d.meshCount — includes component body meshes + sprites"
  - "console [3D] Built N component bodies (M SMD, K THT) — component geometry stats"
  - "console [3D] FPS: X — logged every 5 seconds when 3D active"
  - "console [3D] Warning: component X has no body dimensions, using pad bbox fallback"
duration: 25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Component bodies, footprint bounds extension, and integration polish

**Added 3D component body rendering with body dimensions from Rust footprint bounds, refdes sprite labels, `3` keyboard shortcut, and FPS debug surface.**

## What Happened

Extended the Rust `ComponentInfo` struct with `body_width_nm`, `body_height_nm`, and `model_3d` fields. The `build_snapshot()` method now populates body dimensions from `Footprint.bounds`, with a fallback to computing bounding box from pads when bounds are zero. `model_3d` is always `None` for now (future GLB support).

Updated the TypeScript `ComponentInfo` interface to match. Added `buildComponents()` to `Renderer3D` which creates `BoxGeometry` bodies at correct positions/rotations, with height differentiation (1.2mm SMD vs 5mm THT) and material coloring (dark gray for ICs, tan for passives). Refdes labels rendered as `Sprite` objects with `CanvasTexture` for camera-facing text.

Added `3` key shortcut (skipped when editor/input focused), FPS tracking in the animate loop (logged every 5s), and `fps` field on `__renderer3d` debug surface. Sprite textures are properly disposed in clearBoardGroup and full dispose lifecycle.

## Verification

- `cargo test -p cypcb-render --all-features` — 32 tests pass (including new `test_component_body_dimensions_from_footprint`)
- `cargo check -p cypcb-render --all-features` — compiles cleanly (pre-existing parser clippy issues in dependency, not in render crate)
- `cd viewer && npx tsc --noEmit` — TypeScript compiles with zero errors
- `cd viewer && npx vite build` — production build succeeds; renderer3d chunk at 30.89 kB gzip 8.47 kB
- Browser visual verification not possible in headless CI environment

### Slice-level verification status

- ✅ `cargo check -p cypcb-render --all-features` — Rust compiles cleanly
- ✅ `cd viewer && npx tsc --noEmit` — TypeScript compiles with no errors
- ✅ `cd viewer && npx vite build` — production build succeeds (Three.js tree-shakes, chunk created)
- ⬜ Manual browser verification — blocked by headless environment (no X server)
- ✅ `viewer/src/renderer3d.ts` exports `Renderer3D` class with `init()`, `updateBoard()`, `dispose()` methods

## Diagnostics

- `window.__renderer3d` — `{ isActive, meshCount, drawCalls, fps }` when 3D active
- Console `[3D] Built N component bodies (M SMD, K THT)` — component rendering stats
- Console `[3D] FPS: X` — logged every 5 seconds
- Console `[3D] Warning: component X has no body dimensions, using pad bbox fallback` — missing bounds

## Deviations

None.

## Known Issues

- `cargo clippy --workspace --all-features` fails due to pre-existing clippy errors in `cypcb-parser` crate (51 issues: unused assignments, manual_range_contains, ptr_arg). Not introduced by this task.
- Browser visual verification skipped — headless environment lacks X server for headed Chromium.

## Files Created/Modified

- `crates/cypcb-render/src/snapshot.rs` — Added `body_width_nm`, `body_height_nm`, `model_3d` fields to `ComponentInfo`; updated test
- `crates/cypcb-render/src/lib.rs` — `build_snapshot()` populates body dims from footprint bounds with pad bbox fallback; new test `test_component_body_dimensions_from_footprint`; updated test fixtures
- `viewer/src/types.ts` — Added matching TS fields to `ComponentInfo` interface
- `viewer/src/renderer3d.ts` — Added `buildComponents()`, `createRefdesLabel()`, FPS tracking, sprite disposal, `fps` in debug surface
- `viewer/src/main.ts` — Added `3` key shortcut for 3D toggle
