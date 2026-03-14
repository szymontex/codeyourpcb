# T02: IPC-2221 Trace Width Calculator

**Slice:** S05 — **Milestone:** M001

## Description

Create the cypcb-calc crate implementing IPC-2221 trace width calculation from current requirements.

Purpose: Users need trace width suggestions based on current carrying capacity. This calculator is used by both the LSP (for hover hints) and the router (for automatic width selection). INT-02 requirement.
Output: Working IPC-2221 calculator with proper limits and warnings

## Must-Haves

- [ ] "IPC-2221 trace width calculated from current, temperature rise, and copper weight"
- [ ] "Different constants for internal vs external layers"
- [ ] "Calculator warns about limits (>35A, >100C rise, etc.)"

## Files

- `crates/cypcb-calc/Cargo.toml`
- `crates/cypcb-calc/src/lib.rs`
- `crates/cypcb-calc/src/trace_width.rs`
- `Cargo.toml`
