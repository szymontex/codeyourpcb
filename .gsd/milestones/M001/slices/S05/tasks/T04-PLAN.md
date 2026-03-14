# T04: FreeRouting DSN Export

**Slice:** S05 — **Milestone:** M001

## Description

Create the cypcb-router crate and implement Specctra DSN export for FreeRouting autorouter integration.

Purpose: FreeRouting requires DSN format input. This plan creates the export path; Plan 05-06 handles the import and CLI integration. INT-01 requirement (first half).
Output: Working DSN export that FreeRouting can read

## Must-Haves

- [ ] "Board model exports to Specctra DSN format"
- [ ] "Components, pads, nets all represented in DSN"
- [ ] "Net constraints (width, clearance) included in DSN rules"
- [ ] "DSN file readable by FreeRouting"

## Files

- `crates/cypcb-router/Cargo.toml`
- `crates/cypcb-router/src/lib.rs`
- `crates/cypcb-router/src/dsn.rs`
- `crates/cypcb-router/src/types.rs`
- `Cargo.toml`
