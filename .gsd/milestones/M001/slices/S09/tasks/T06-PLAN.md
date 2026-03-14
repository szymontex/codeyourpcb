# T06: Metadata & Version Tracking

**Slice:** S09 — **Milestone:** M001

## Description

Implement metadata viewing, footprint preview extraction, version tracking, and 3D model association.

Purpose: Covers LIB-03 (3D STEP models), LIB-07 (version tracking), LIB-08 (footprint preview), LIB-09 (metadata viewing).
Output: metadata.rs and preview.rs modules with version tracking, preview extraction, and model association.

## Must-Haves

- [ ] "User can view component metadata including datasheet URL, specs, manufacturer"
- [ ] "User can preview footprint geometry before adding to board"
- [ ] "User can associate 3D STEP model path with a component"
- [ ] "User can track library versions with import timestamps"

## Files

- `crates/cypcb-library/src/metadata.rs`
- `crates/cypcb-library/src/preview.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/manager.rs`
