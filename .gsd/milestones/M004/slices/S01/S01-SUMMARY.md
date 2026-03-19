---
id: S01
parent: M004
milestone: M004
provides:
  - "cypcb-kicad::pcb_parser module — parse_kicad_pcb() and parse_kicad_pcb_str() parsing .kicad_pcb files into BoardWorld + FootprintLibrary + RoutingResult + metadata"
  - "KicadPcbError structured error type (5 variants) with field-level context for diagnosable failures"
  - "KicadBenchmark and BenchmarkComplexity types with BENCHMARKS const array and get_benchmarks() accessor"
  - "3 benchmark .kicad_pcb fixtures (led_blink/stm32_breakout/multi_ic) covering simple/medium/complex tiers"
  - "CLI parse-kicad subcommand emitting KicadPcbMetadata as JSON"
  - "Ratsnest compatibility proof — parsed BoardWorld feeds extract_ratsnest() on all 3 benchmarks"
requires:
  - slice: none
    provides: first slice in M004, no dependencies
affects:
  - S03
  - S07
key_files:
  - crates/cypcb-kicad/src/pcb_parser.rs
  - crates/cypcb-kicad/tests/pcb_parser_tests.rs
  - crates/cypcb-kicad/tests/benchmark_parse.rs
  - crates/cypcb-kicad/tests/ratsnest_compat.rs
  - crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb
  - tests/fixtures/benchmark/led_blink.kicad_pcb
  - tests/fixtures/benchmark/stm32_breakout.kicad_pcb
  - tests/fixtures/benchmark/multi_ic.kicad_pcb
  - tests/fixtures/benchmark/README.md
  - crates/cypcb-cli/src/commands/parse_kicad.rs
key_decisions:
  - "Used symbolic_expressions crate directly instead of kicad_parse_gen — kicad_parse_gen only handles KiCad 5 module keyword, not KiCad 7/8 footprint keyword"
  - "Synthetic benchmark fixtures instead of real projects — search didn't yield downloadable KiCad 8 files with permissive licenses; synthetic files are license-clean and precisely controlled"
  - "KicadBenchmark uses &'static str fields for BENCHMARKS const array compatibility"
  - "Board outline as bounding box — non-rectangular outlines approximated as axis-aligned rectangles (polygon support deferred)"
  - "Footprints registered by full library link name (e.g. Resistor_SMD:R_0402) to avoid naming collisions"
patterns_established:
  - "S-expression tree walking: list_name() → match name → extract children — reusable for any KiCad S-expr parsing"
  - "parse_layer_names() handles *.Cu wildcard expansion for through-hole pad layers"
  - "KiCad 5/6 fp_text + KiCad 8 property keyword both handled for backward compat"
  - "workspace_root() helper in integration tests resolves CARGO_MANIFEST_DIR up 2 levels for workspace-level fixtures"
  - "CLI subcommand pattern: Args struct with run() method, miette error wrapping, JSON output"
observability_surfaces:
  - "KicadPcbMetadata struct returned from every parse (version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count)"
  - "CLI parse-kicad emits structured JSON metadata to stdout"
  - "KicadPcbError variants carry structured context (field name, version, description)"
  - "BENCHMARKS constant and get_benchmarks() provide programmatic fixture access"
drill_down_paths:
  - .gsd/milestones/M004/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M004/slices/S01/tasks/T03-SUMMARY.md
duration: 60min
verification_result: passed
completed_at: 2026-03-14
---

# S01: KiCad PCB Parser & Benchmark Fixtures

**Custom KiCad 5/6/7/8 .kicad_pcb parser with 3 benchmark fixtures and CLI integration — parsed BoardWorld feeds autorouter's extract_ratsnest() on all fixtures, proving S03 compatibility.**

## What Happened

Built a complete `.kicad_pcb` parser in three tasks:

**T01 (25min):** Created `pcb_parser.rs` (~600 lines) using the `symbolic_expressions` crate directly, since `kicad_parse_gen` only handles KiCad 5's `module` keyword. The parser walks S-expression trees extracting board outline (Edge.Cuts bounding box), footprints with pads (shape, size, drill, layers, net), nets (interned via `NetRegistry`, net 0 skipped), and existing traces/vias as `RoutingResult` for reference scoring. Handles both `footprint` (KiCad 7/8) and `module` (KiCad 5/6) keywords, plus `property` (KiCad 8) and `fp_text` (KiCad 5/6) for reference designator/value extraction. Created a synthetic minimal test fixture (2 components, 3 nets, 1 segment, 1 via) and 10 integration tests covering all contract points plus error paths.

**T02 (20min):** Created 3 benchmark `.kicad_pcb` fixtures at `tests/fixtures/benchmark/`: led_blink (7 components, 7 nets, 2-layer), stm32_breakout (29 components, 40 nets, 2-layer), multi_ic (52 components, 94 nets, 4-layer). Added `BENCHMARKS` const array and `get_benchmarks()` to the parser module, plus 5 integration tests validating all fixtures parse with correct metadata. Fixtures are synthetic (license-clean, precisely controlled) since directly downloadable real KiCad 8 projects weren't found.

**T03 (15min):** Wired the parser into the CLI as `parse-kicad` subcommand emitting `KicadPcbMetadata` as JSON. Added `Serialize` derive to metadata struct. Created `ratsnest_compat.rs` proving all 3 benchmark fixtures feed `extract_ratsnest()` successfully — the ratsnest is non-empty with multi-pad nets, proving the parser output is consumable by S03's routing engine.

## Verification

All slice-level verification checks pass:

- ✅ `cargo test -p cypcb-kicad` — 39 tests pass (22 unit + 10 parser integration + 5 benchmark + 2 ratsnest)
- ✅ `cargo test -p cypcb-kicad --test benchmark_parse` — 5/5 pass, all 3 fixtures parse with correct counts
- ✅ `cargo test -p cypcb-kicad --test ratsnest_compat` — 2/2 pass, ratsnest non-empty on all benchmarks
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — exits 0, JSON: 7 components, 7 nets, 2 layers
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/stm32_breakout.kicad_pcb` — exits 0, JSON: 29 components, 40 nets, 2 layers
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/multi_ic.kicad_pcb` — exits 0, JSON: 52 components, 94 nets, 4 layers
- ✅ Failure paths: empty input → `SexprParseError`, unsupported version → `UnsupportedVersion`

## Requirements Advanced

- R101 (KiCad .kicad_pcb Board Parser) — fully implemented: parser extracts board outline, footprints, pads, nets, traces, vias from KiCad 5-8 format files
- R102 (Benchmark Suite from Real KiCad Projects) — partially advanced: 3 fixtures exist and parse correctly, but they are synthetic rather than downloaded from real projects. Benchmark metadata and programmatic access infrastructure is complete.

## Requirements Validated

- R101 — validated by 39 passing tests: synthetic minimal fixture (contract), 3 benchmark fixtures (integration), ratsnest compatibility (S03 readiness), CLI JSON output (operational), error paths (robustness)

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- R102 notes updated: "fixtures are synthetic (license-clean)" — original description said "downloaded" but task plan allowed synthetic fallback. Fixtures still serve their purpose as autorouter benchmark inputs.

## Deviations

- Synthetic benchmark fixtures instead of real open-source projects — search didn't yield directly downloadable KiCad 8 .kicad_pcb files with permissive licenses. Task plan explicitly allowed this fallback.
- `KicadBenchmark` changed from `String` to `&'static str` fields — required for `const` array, minor API surface change.
- 4 extra tests beyond plan specification (error paths, module keyword compat, layer count) — improved coverage without scope creep.

## Known Limitations

- Board outline uses axis-aligned bounding box — non-rectangular outlines (arcs, rounded corners) are approximated as rectangles. Sufficient for benchmark routing but not full KiCad fidelity.
- Synthetic fixtures don't exercise all real-world edge cases (gr_arc, custom pad shapes, zone fills, keepout areas, net classes). Real KiCad files may need parser fixes.
- Pad positions stored as footprint-local coordinates — absolute positions require combining footprint position + rotation + pad offset at query time.
- Y-axis passed through without inversion (KiCad Y-down matches existing footprint importer convention).

## Follow-ups

- When real open-source KiCad 7/8 projects are found, add them as additional benchmark fixtures and fix any parser edge cases they expose.
- Board outline polygon support if complex board shapes needed in later slices.

## Files Created/Modified

- `crates/cypcb-kicad/src/pcb_parser.rs` — NEW: complete .kicad_pcb parser module (~600 lines)
- `crates/cypcb-kicad/src/lib.rs` — added `pub mod pcb_parser` and re-exports
- `crates/cypcb-kicad/Cargo.toml` — added `symbolic_expressions`, `cypcb-router`, `serde`, `cypcb-autoroute` (dev) dependencies
- `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — synthetic KiCad 8 test fixture
- `crates/cypcb-kicad/tests/pcb_parser_tests.rs` — 10 integration tests
- `crates/cypcb-kicad/tests/benchmark_parse.rs` — 5 benchmark integration tests
- `crates/cypcb-kicad/tests/ratsnest_compat.rs` — 2 ratsnest compatibility tests
- `tests/fixtures/benchmark/led_blink.kicad_pcb` — simple benchmark (7 components, 7 nets, 2-layer)
- `tests/fixtures/benchmark/stm32_breakout.kicad_pcb` — medium benchmark (29 components, 40 nets, 2-layer)
- `tests/fixtures/benchmark/multi_ic.kicad_pcb` — complex benchmark (52 components, 94 nets, 4-layer)
- `tests/fixtures/benchmark/README.md` — fixture documentation
- `crates/cypcb-cli/Cargo.toml` — added `cypcb-kicad` dependency
- `crates/cypcb-cli/src/commands/parse_kicad.rs` — NEW: parse-kicad CLI command
- `crates/cypcb-cli/src/commands/mod.rs` — registered parse_kicad module
- `crates/cypcb-cli/src/main.rs` — added ParseKicad subcommand

## Forward Intelligence

### What the next slice should know
- `parse_kicad_pcb(path)` returns `KicadPcbParseResult { world, library, reference_routes, metadata }`. The `reference_routes` field is `Option<RoutingResult>` — present when the `.kicad_pcb` file contains `segment`/`via` elements.
- `get_benchmarks()` returns `Vec<(KicadBenchmark, PathBuf)>` with absolute paths to all 3 fixtures. Use this for automated benchmark iteration.
- The `BoardWorld` from parsing has components placed and nets interned but no routing — call `extract_ratsnest()` to get the net connectivity for routing.
- S02 (scoring) can call `parse_kicad_pcb()` → get `reference_routes` → score them as a baseline. The reference routes contain trace segments and vias from the original fixture.

### What's fragile
- Synthetic fixtures cover standard KiCad 8 format but not edge cases (gr_arc outlines, custom pad shapes, polygon pours, keepout areas) — if real KiCad files are added later, the parser may need fixes.
- Board outline is a bounding box only — any downstream slice assuming polygon outline will break.

### Authoritative diagnostics
- `cargo run -p cypcb-cli -- parse-kicad <file>` — prints metadata JSON, fastest way to verify any .kicad_pcb file parses correctly
- `cargo test -p cypcb-kicad --test ratsnest_compat -- --nocapture` — shows net extraction counts per benchmark, proves S03 compatibility

### What assumptions changed
- Original plan assumed real KiCad projects downloadable from GitHub — synthetic fixtures used instead. Functionally equivalent for benchmark validation but lack real-world edge case coverage.
