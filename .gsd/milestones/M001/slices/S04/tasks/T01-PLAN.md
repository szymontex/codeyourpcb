# T01: Export Crate Setup

**Slice:** S04 — **Milestone:** M001

## Description

Create the cypcb-export crate foundation with coordinate conversion and aperture management.

Purpose: Export functionality requires precise coordinate conversion from internal nanometers to Gerber decimal format, plus aperture definitions for all pad shapes. This foundation is used by all Gerber exporters.

Output: New cypcb-export crate with coordinate conversion and aperture utilities.

## Must-Haves

- [ ] "Export crate compiles successfully"
- [ ] "Coordinates convert from nanometers to Gerber decimal format"
- [ ] "Apertures generate valid D-code definitions"

## Files

- `crates/cypcb-export/Cargo.toml`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-export/src/coords.rs`
- `crates/cypcb-export/src/apertures.rs`
- `Cargo.toml`
