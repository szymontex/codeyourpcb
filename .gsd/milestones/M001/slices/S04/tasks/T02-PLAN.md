# T02: Gerber Copper/Mask/Paste Export

**Slice:** S04 — **Milestone:** M001

## Description

Implement Gerber X2 export for copper layers (top, bottom, inner).

Purpose: Copper layers are the primary manufacturing files containing all conductive features - pads, traces, and copper zones. This is the core export functionality.

Output: Functions to export copper layer Gerber files with X2 attributes, pads, and traces.

## Must-Haves

- [ ] "Copper layer Gerber files contain valid header with X2 attributes"
- [ ] "Pads render as flash commands at correct positions"
- [ ] "Traces render as draw commands with correct width"

## Files

- `crates/cypcb-export/src/gerber/mod.rs`
- `crates/cypcb-export/src/gerber/header.rs`
- `crates/cypcb-export/src/gerber/copper.rs`
- `crates/cypcb-export/src/gerber/mask.rs`
- `crates/cypcb-export/src/lib.rs`
