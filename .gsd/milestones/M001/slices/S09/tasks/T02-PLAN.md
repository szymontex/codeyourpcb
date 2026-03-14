# T02: KiCad S-Expression Parser

**Slice:** S09 — **Milestone:** M001

## Description

Implement KiCad .kicad_mod S-expression parser and .pretty folder importer using lexpr.

Purpose: Enables importing the most popular open-source PCB footprint format. Covers LIB-04 (KiCad import) and LIB-11 (auto-organize dropped folders).
Output: KiCadSource struct implementing LibrarySource trait, parsing .kicad_mod files with lexpr.

## Must-Haves

- [ ] "KiCad .kicad_mod files parse into Component structs with correct metadata"
- [ ] "KiCad .pretty folders import as libraries with all footprints indexed"
- [ ] "System auto-organizes dropped folders with kicad:: namespace prefix"

## Files

- `crates/cypcb-library/src/sources/mod.rs`
- `crates/cypcb-library/src/sources/kicad.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/Cargo.toml`
