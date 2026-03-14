# S02: Rendering

**Goal:** Create the cypcb-render WASM crate that bridges Rust board data to JavaScript.
**Demo:** Create the cypcb-render WASM crate that bridges Rust board data to JavaScript.

## Must-Haves


## Tasks

- [x] **T01: WASM Crate Setup**
  - Create the cypcb-render WASM crate that bridges Rust board data to JavaScript.

Purpose: Enable JavaScript to load .cypcb source, parse it, and receive structured board data for rendering. This is the foundation for the web viewer.

Output: Compilable WASM crate with PcbEngine and BoardSnapshot types.
- [x] **T02: Frontend Scaffolding**
  - Create minimal frontend scaffolding for the PCB viewer web application.

Purpose: Establish the TypeScript/Vite project structure that will load the WASM module and render to canvas. This runs in parallel with WASM crate setup.

Output: Working dev server with HTML canvas and TypeScript infrastructure ready for WASM integration.
- [x] **T03: WASM Build & Integration**
  - Build WASM module and integrate with TypeScript frontend.

Purpose: Connect the Rust PcbEngine to the JavaScript viewer. After this plan, the viewer can load .cypcb source and receive structured board data.

Output: Working WASM integration where TypeScript can call PcbEngine.load_source() and get_snapshot().
- [x] **T04: Canvas Renderer**
  - Implement Canvas 2D rendering with viewport transformation and layer colors.

Purpose: Visualize board data on screen. This is the core rendering engine that transforms nanometer coordinates to screen pixels and draws components.

Output: Working canvas renderer that displays board outline, components, and pads with zoom/pan navigation.
- [x] **T05: Interaction Controls**
  - Integrate rendering with interaction handling for a complete minimal viewer.

Purpose: Wire together WASM, rendering, and user interaction so the user can view and navigate their board design. This completes the core verification UI.

Output: Working PCB viewer with zoom/pan navigation, layer toggles, and component selection.
- [x] **T06: Hot Reload** `est:5m12s`
  - Implement hot reload for instant feedback when .cypcb files change.

Purpose: Enable the core development workflow - edit code, see changes immediately. This is critical for verifying the concept works.

Output: File watcher that triggers browser re-render on save, preserving viewport and selection.
- [x] **T07: Visual Verification** `est:3m`
  - Visual verification that the minimal PCB viewer works correctly.

Purpose: Human verification that the concept is proven - can see board, navigate, and iterate with hot reload.

Output: Confirmed working Phase 2 deliverable ready for next phase.
- [x] **T08: Fix WASM Build**
  - Fix WASM build by resolving getrandom WASM compatibility issues.

Purpose: The cypcb-render crate fails to compile for wasm32-unknown-unknown because bevy_ecs -> bevy_utils -> ahash depends on getrandom, which requires explicit WASM configuration. Once fixed, the real Rust-based PcbEngine can replace the JavaScript MockPcbEngine.

Output: Working wasm-pack build that produces viewer/pkg/ artifacts.
- [x] **T09: Enable Real WASM Integration**
  - Enable real WASM integration by uncommenting the WASM import code in wasm.ts.

Purpose: With the WASM build now working (from 02-08), the viewer can use the real Rust-based PcbEngine instead of the JavaScript MockPcbEngine. This completes the WASM integration gap.

Output: viewer/src/wasm.ts uses real WASM module with MockPcbEngine as fallback only when WASM is unavailable.

## Files Likely Touched

- `crates/cypcb-render/Cargo.toml`
- `crates/cypcb-render/src/lib.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `Cargo.toml`
- `viewer/index.html`
- `viewer/package.json`
- `viewer/tsconfig.json`
- `viewer/src/main.ts`
- `viewer/src/wasm.ts`
- `viewer/vite.config.ts`
- `viewer/src/wasm.ts`
- `viewer/package.json`
- `viewer/build-wasm.sh`
- `viewer/src/viewport.ts`
- `viewer/src/renderer.ts`
- `viewer/src/layers.ts`
- `viewer/src/main.ts`
- `viewer/src/interaction.ts`
- `crates/cypcb-watcher/Cargo.toml`
- `crates/cypcb-watcher/src/lib.rs`
- `viewer/server.ts`
- `viewer/src/main.ts`
- `viewer/package.json`
- `Cargo.toml`
- `Cargo.toml`
- `crates/cypcb-render/Cargo.toml`
- `.cargo/config.toml`
- `viewer/build-wasm.sh`
- `viewer/src/wasm.ts`
