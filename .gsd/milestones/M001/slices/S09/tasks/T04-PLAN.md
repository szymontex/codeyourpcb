# T04: Custom & JLCPCB Sources

**Slice:** S09 — **Milestone:** M001

## Description

Implement custom library source for user-created libraries and JLCPCB API client for parts catalog search.

Purpose: Covers LIB-02 (organize by manufacturer/function), LIB-05 (JLCPCB import), LIB-06 (custom libraries), LIB-10 (search paths config).
Output: CustomSource and JLCPCBSource implementing LibrarySource trait.

## Must-Haves

- [ ] "User can create custom component libraries with custom:: namespace"
- [ ] "User can organize components by manufacturer or function categories"
- [ ] "JLCPCB source is optional and requires user-provided API key"

## Files

- `crates/cypcb-library/src/sources/custom.rs`
- `crates/cypcb-library/src/sources/jlcpcb.rs`
- `crates/cypcb-library/src/sources/mod.rs`
- `crates/cypcb-library/Cargo.toml`
