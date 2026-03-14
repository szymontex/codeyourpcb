---
estimated_steps: 5
estimated_files: 5
---

# T03: Component bodies, footprint bounds extension, and integration polish

**Slice:** S04 — 3D Board Viewer
**Milestone:** M002

## Description

Adds visible component bodies to the 3D scene — without these, the board looks like bare copper. Extends the Rust `ComponentInfo` snapshot with `body_width_nm` and `body_height_nm` from the footprint `bounds` Rect, and adds an optional `model_3d` field for future GLB model paths. Updates the TS type to match. Renders fallback component bodies as colored boxes with refdes labels. Adds keyboard shortcut, geometry stats to debug surface, and verifies the dispose lifecycle is clean.

## Steps

1. **Extend Rust `ComponentInfo`** — In `crates/cypcb-render/src/snapshot.rs`, add three fields to `ComponentInfo`:
   - `body_width_nm: i64` — component body width from `Footprint.bounds`
   - `body_height_nm: i64` — component body height from `Footprint.bounds`
   - `model_3d: Option<String>` — optional path/key to a GLB model file (populated later, always `None` for now)
   
   In `crates/cypcb-render/src/lib.rs` `build_snapshot()`, populate `body_width_nm` and `body_height_nm` from `fp.bounds.width().0` and `fp.bounds.height().0` when the footprint is found, else default to pad bounding box or 0. Set `model_3d: None`. Update the test `test_board_snapshot_serializes` to include the new fields.

2. **Update TypeScript types** — In `viewer/src/types.ts`, add to `ComponentInfo`:
   - `body_width_nm: number`
   - `body_height_nm: number`
   - `model_3d: string | null`

3. **Build component body geometry** — In `renderer3d.ts`, add `buildComponents(components: ComponentInfo[])`:
   - For each component with `body_width_nm > 0 && body_height_nm > 0`: create a `BoxGeometry` with dimensions `(body_width_nm/1e6, body_height_nm/1e6, componentHeight)` where componentHeight is ~1.2mm for SMD parts (footprints with no through-hole pads) and ~5mm for THT parts (footprints with drill pads).
   - Position at `(x_nm/1e6, y_nm/1e6, boardThickness + componentHeight/2)` for top-side components.
   - Apply rotation around Z-axis matching `rotation_mdeg`.
   - Color: dark gray `#404040` with slight metallic material for IC packages, tan `#c2a366` for passives (detect by refdes prefix: R/C/L = passive, U/IC = IC).
   - For components with `body_width_nm === 0`, fall back to computing a bounding box from pads.
   - Add refdes label as a `Sprite` with `SpriteMaterial` using a dynamically created canvas texture. Only add labels when camera is close enough (or always for now, optimize later).

4. **Add keyboard shortcut and final wiring** — In `main.ts`:
   - Add `3` key shortcut to toggle 3D view (when not in editor text input focus).
   - Ensure 3D button shows pressed/active state when 3D is active (CSS class).
   - Wire layer changes to also call `renderer3d.updateLayerVisibility()` if 3D is active.
   - Add `fps` counter to `__renderer3d` debug surface (compute from frame delta).

5. **Verify full integration** — Run `cargo clippy --workspace --all-features -- -D warnings`, `cd viewer && npx tsc --noEmit`, `cd viewer && npx vite build`. In browser: load .cypcb, toggle 3D, confirm component boxes visible at correct positions with refdes labels, orbit around, toggle layers, switch theme, toggle back to 2D. Check `window.__renderer3d` has `{ isActive: true, meshCount: N, fps: ~60 }`.

## Must-Haves

- [ ] Rust `ComponentInfo` extended with `body_width_nm`, `body_height_nm`, `model_3d` fields
- [ ] Rust `build_snapshot()` populates body dimensions from `Footprint.bounds`
- [ ] TS `ComponentInfo` type updated with matching fields
- [ ] Component bodies rendered as colored boxes in 3D at correct positions/rotations
- [ ] Refdes labels visible as sprite text on component bodies
- [ ] Keyboard shortcut (`3` key) toggles 3D view
- [ ] `cargo clippy` and `tsc --noEmit` pass cleanly
- [ ] `vite build` succeeds (production build)
- [ ] `window.__renderer3d` debug surface reports `meshCount` and `fps`

## Verification

- `cargo clippy --workspace --all-features -- -D warnings` — Rust compiles cleanly with new fields
- `cd viewer && npx tsc --noEmit` — TypeScript compiles with updated types
- `cd viewer && npx vite build` — production build succeeds
- Browser: component boxes visible at correct positions, matching 2D component locations
- Browser: refdes labels readable on/near component bodies
- Browser: pressing `3` toggles between 2D and 3D
- Browser: `window.__renderer3d.meshCount > 0` when 3D active, `window.__renderer3d.isActive === false` after toggling back

## Observability Impact

- Signals added/changed: `[3D] Built N component bodies (M SMD, K THT)` log, `[3D] FPS: X` logged every 5 seconds when active
- How a future agent inspects this: `window.__renderer3d` now includes `fps` field; `meshCount` includes component body meshes
- Failure state exposed: Components with missing bounds logged as `[3D] Warning: component X has no body dimensions, using pad bbox fallback`

## Inputs

- `crates/cypcb-render/src/snapshot.rs` — existing `ComponentInfo` struct
- `crates/cypcb-render/src/lib.rs` — existing `build_snapshot()` method, footprint library access
- `viewer/src/renderer3d.ts` — T01+T02's Renderer3D with scene, copper geometry
- `viewer/src/types.ts` — existing TS types
- `viewer/src/main.ts` — existing toggle wiring from T01

## Expected Output

- `crates/cypcb-render/src/snapshot.rs` — modified, ComponentInfo has body dimension fields + model_3d
- `crates/cypcb-render/src/lib.rs` — modified, build_snapshot populates body dims from footprint bounds
- `viewer/src/types.ts` — modified, ComponentInfo TS type matches Rust
- `viewer/src/renderer3d.ts` — modified, component body + label rendering added
- `viewer/src/main.ts` — modified, keyboard shortcut + debug surface polish
