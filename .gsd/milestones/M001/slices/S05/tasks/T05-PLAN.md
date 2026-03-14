# T05: LSP Server Setup

**Slice:** S05 — **Milestone:** M001

## Description

Create the cypcb-lsp crate with basic LSP functionality: hover, diagnostics from DRC, and document synchronization.

Purpose: IDE integration provides autocomplete, hover info, and real-time error feedback. This plan implements the core LSP infrastructure and two key features (hover, diagnostics). DEV-02 requirement (first half).
Output: Working LSP server with hover and DRC diagnostics

## Must-Haves

- [ ] "LSP server starts and responds to initialize request"
- [ ] "Hover over component shows footprint and value"
- [ ] "Hover over net shows connected pins"
- [ ] "DRC errors appear as diagnostics (squiggles)"

## Files

- `crates/cypcb-lsp/Cargo.toml`
- `crates/cypcb-lsp/src/lib.rs`
- `crates/cypcb-lsp/src/main.rs`
- `crates/cypcb-lsp/src/backend.rs`
- `crates/cypcb-lsp/src/document.rs`
- `crates/cypcb-lsp/src/hover.rs`
- `crates/cypcb-lsp/src/diagnostics.rs`
- `Cargo.toml`
