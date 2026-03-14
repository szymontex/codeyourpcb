# T02: IC Footprints

**Slice:** S03 — **Milestone:** M001

## Description

Add QFP/SOIC/SOT footprint families to the footprint library.

Purpose: Fulfill FTP-03 requirement for IC package support. These are essential for real-world PCB designs with microcontrollers and other ICs.

Output: Gull-wing footprint generators for SOIC, SOT, and QFP packages registered in FootprintLibrary.

## Must-Haves

- [ ] "SOIC-8 and SOIC-14 footprints available in library"
- [ ] "SOT-23 and SOT-23-5 footprints available in library"
- [ ] "TQFP-32 footprint available in library"
- [ ] "Footprints have correct pin counts and IPC-7351B dimensions"

## Files

- `crates/cypcb-world/src/footprint/gullwing.rs`
- `crates/cypcb-world/src/footprint/mod.rs`
- `crates/cypcb-world/src/footprint/library.rs`
