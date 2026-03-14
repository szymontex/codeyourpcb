# T01: WASM Crate Setup

**Slice:** S02 — **Milestone:** M001

## Description

Create the cypcb-render WASM crate that bridges Rust board data to JavaScript.

Purpose: Enable JavaScript to load .cypcb source, parse it, and receive structured board data for rendering. This is the foundation for the web viewer.

Output: Compilable WASM crate with PcbEngine and BoardSnapshot types.

## Must-Haves

- [ ] "WASM module compiles with wasm-pack"
- [ ] "BoardSnapshot can be serialized to JS"
- [ ] "PcbEngine can parse source and return snapshot"

## Files

- `crates/cypcb-render/Cargo.toml`
- `crates/cypcb-render/src/lib.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `Cargo.toml`
