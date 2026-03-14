# T03: KiCad Footprint Import

**Slice:** S05 — **Milestone:** M001

## Description

Create the cypcb-kicad crate to import KiCad .kicad_mod footprint files using the kicad_parse_gen library.

Purpose: Enable users to use KiCad's extensive footprint libraries directly in their designs. FTP-05 requirement. This allows full KiCad compatibility - if it exports to Gerber from KiCad, it should work here.
Output: Working KiCad footprint import with library scanning

## Must-Haves

- [ ] "KiCad .kicad_mod files can be parsed and converted to internal Footprint"
- [ ] "All pad shapes (rect, circle, roundrect, oval) are supported"
- [ ] "Through-hole pads with drill are handled correctly"
- [ ] "Library directories (.pretty folders) can be scanned"

## Files

- `crates/cypcb-kicad/Cargo.toml`
- `crates/cypcb-kicad/src/lib.rs`
- `crates/cypcb-kicad/src/footprint.rs`
- `crates/cypcb-kicad/src/library.rs`
- `Cargo.toml`
