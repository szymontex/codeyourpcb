# T08: Fix WASM Build

**Slice:** S02 — **Milestone:** M001

## Description

Fix WASM build by resolving getrandom WASM compatibility issues.

Purpose: The cypcb-render crate fails to compile for wasm32-unknown-unknown because bevy_ecs -> bevy_utils -> ahash depends on getrandom, which requires explicit WASM configuration. Once fixed, the real Rust-based PcbEngine can replace the JavaScript MockPcbEngine.

Output: Working wasm-pack build that produces viewer/pkg/ artifacts.

## Must-Haves

- [ ] "WASM module compiles with wasm-pack"
- [ ] "viewer/pkg/ directory contains WASM artifacts"

## Files

- `Cargo.toml`
- `crates/cypcb-render/Cargo.toml`
- `.cargo/config.toml`
- `viewer/build-wasm.sh`
