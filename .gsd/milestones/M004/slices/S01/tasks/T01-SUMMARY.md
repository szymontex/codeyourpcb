---
id: T01
parent: S01
milestone: M004
provides:
  - parse_kicad_pcb() and parse_kicad_pcb_str() functions for .kicad_pcb files
  - KicadPcbParseResult with BoardWorld, FootprintLibrary, optional RoutingResult, metadata
  - KicadPcbError structured error type with 5 variants
  - KicadBenchmark and BenchmarkComplexity types for T02
  - parse_layer_name() utility for KiCad string layer names
  - Synthetic test fixture (minimal.kicad_pcb) with 2 components, 3 nets, 1 segment, 1 via
key_files:
  - crates/cypcb-kicad/src/pcb_parser.rs
  - crates/cypcb-kicad/tests/pcb_parser_tests.rs
  - crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb
key_decisions:
  - Used symbolic_expressions crate directly (not kicad_parse_gen) for .kicad_pcb parsing since kicad_parse_gen only handles KiCad 5 module keyword, not KiCad 7/8 footprint keyword
  - Footprints registered in library by full library link name (e.g., "Resistor_SMD:R_0402") to avoid collisions between identically-named footprints from different libraries
  - KiCad net numbers mapped to internal NetId via HashMap during parse; net 0 ("") skipped
  - Board outline extracted as bounding box from Edge.Cuts gr_line/gr_rect/gr_poly elements
patterns_established:
  - S-expression tree walking pattern: list_name() → match name → extract children. Reusable for any future KiCad S-expr parsing.
  - parse_layer_names() handles *.Cu wildcard expansion for through-hole pad layers
  - KiCad property keyword (KiCad 8) and fp_text keyword (KiCad 5/6) both handled for reference/value extraction
observability_surfaces:
  - KicadPcbMetadata struct returned from every parse (version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count)
  - KicadPcbError variants carry structured context (field name, version number, description) for diagnosable failures
  - Unit tests assert specific error variants for empty input (SexprParseError) and unsupported version (UnsupportedVersion)
duration: 25min
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Build core .kicad_pcb S-expression parser

**Created a custom KiCad 5/6/7/8 .kicad_pcb parser using symbolic_expressions crate that extracts board outline, footprints with pads, nets, traces, and vias into BoardWorld + FootprintLibrary + RoutingResult.**

## What Happened

1. Added `symbolic_expressions = "0.4"` and `cypcb-router` dependencies to `cypcb-kicad/Cargo.toml`.
2. Created `pcb_parser.rs` (~600 lines) with the full parser:
   - `KicadPcbError` enum (5 variants: IoError, SexprParseError, MissingField, UnsupportedVersion, InvalidData)
   - `KicadPcbMetadata`, `KicadPcbParseResult`, `KicadBenchmark`, `BenchmarkComplexity` structs
   - `parse_kicad_pcb()` (file reader) and `parse_kicad_pcb_str()` (core parser)
   - Handles both `footprint` (KiCad 7/8) and `module` (KiCad 5/6) keywords
   - Handles both `property` (KiCad 8) and `fp_text` (KiCad 5/6) for refdes/value
   - `parse_layer_name()` maps KiCad layer strings to internal Layer enum
   - `parse_layer_names()` handles `*.Cu`, `*.Mask`, `*.Paste` wildcards
3. Updated `lib.rs` with `pub mod pcb_parser` and re-exports.
4. Created synthetic test fixture `minimal.kicad_pcb` (KiCad 8 format) with 2 footprints (SMD resistor + THT LED), 3 nets, board outline, 1 segment, 1 via.
5. Wrote 10 integration tests + 3 inline unit tests covering all must-haves.

## Verification

- `cargo test -p cypcb-kicad --test pcb_parser_tests` — **10/10 tests pass**: parse_minimal_fixture, component_positions, pad_net_assignments, reference_routes_extracted, footprint_library_registered, net_zero_skipped, empty_input_error, unsupported_version_error, module_keyword_compat, layer_count_extraction
- `cargo test -p cypcb-kicad` — **32 total tests pass** (22 unit + 10 integration), all existing footprint tests unaffected
- `cargo check -p cypcb-kicad` — clean, no errors
- `cargo clippy -p cypcb-kicad -- -D warnings` — clean

### Slice-level verification (partial — T01 is first task):
- ✅ `cargo test -p cypcb-kicad` — all parser unit tests pass
- ⏳ `cargo test -p cypcb-kicad --test benchmark_parse` — not yet (T02 will create benchmark fixtures)
- ⏳ `cargo run -p cypcb-cli -- parse-kicad ...` — not yet (T03 will create CLI command)
- ⏳ `cargo test -p cypcb-kicad --test ratsnest_compat` — not yet (T03 will create compatibility test)
- ✅ Failure-path check: empty input → SexprParseError, unsupported version → UnsupportedVersion

## Diagnostics

- Call `parse_kicad_pcb_str()` on any KiCad PCB content and inspect `result.metadata` for counts and version.
- Error types carry structured context: `MissingField { field, context }`, `UnsupportedVersion { version }`, `InvalidData(description)`.
- `parse_layer_name()` is public and can be tested independently for layer mapping verification.

## Deviations

- Added `KicadPcbParseResult` without `Debug` derive (BoardWorld doesn't implement Debug). Tests use pattern matching instead of `unwrap_err()`.
- Added 4 extra tests beyond the 6 specified in the plan (error paths, module keyword compat, layer count extraction) to improve coverage.
- Handles `property` keyword (KiCad 8 style) in addition to `fp_text` (KiCad 5/6 style) — not explicitly in plan but necessary for real KiCad 8 files.

## Known Issues

- Board outline extraction uses bounding box approximation — non-rectangular board outlines (arcs, rounded corners) will be approximated as axis-aligned rectangles.
- Y-axis convention: KiCad uses top-left origin with Y-down. Currently coordinates are passed through without inversion. This matches what the existing footprint importer does.
- Pad positions stored as footprint-local coordinates in FootprintLibrary; absolute pad positions require combining footprint position + rotation + pad offset at query time.

## Files Created/Modified

- `crates/cypcb-kicad/src/pcb_parser.rs` — NEW: complete .kicad_pcb parser module (~600 lines)
- `crates/cypcb-kicad/src/lib.rs` — added `pub mod pcb_parser` and re-exports
- `crates/cypcb-kicad/Cargo.toml` — added `symbolic_expressions` and `cypcb-router` dependencies
- `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — NEW: synthetic KiCad 8 test fixture
- `crates/cypcb-kicad/tests/pcb_parser_tests.rs` — NEW: 10 integration tests
- `.gsd/milestones/M004/slices/S01/S01-PLAN.md` — added Observability/Diagnostics section, failure-path verification
- `.gsd/milestones/M004/slices/S01/tasks/T01-PLAN.md` — added Observability Impact section
