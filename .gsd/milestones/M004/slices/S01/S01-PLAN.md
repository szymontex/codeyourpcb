# S01: KiCad PCB Parser & Benchmark Fixtures

**Goal:** Parse real KiCad 7/8 `.kicad_pcb` files into `BoardWorld` with correct footprints, pads, nets, board outline, and reference routing — and provide 3 benchmark fixtures with metadata for the autorouter validation pipeline.

**Demo:** `cargo test -p cypcb-kicad pcb_parser` passes, parsing 3 real KiCad projects. `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` prints metadata (component count, net count, board size). Parsed `BoardWorld` feeds `extract_ratsnest()` without errors.

## Must-Haves

- Custom S-expression parser handles KiCad 7/8 `footprint` keyword (not just KiCad 5 `module`)
- Board outline extracted from `gr_line`/`gr_rect` on Edge.Cuts layer → `BoardSize`
- All footprints extracted with position, rotation, reference designator, value
- All pads extracted with shape, size, drill, layers, net assignment
- Nets interned into `NetRegistry` via `BoardWorld::intern_net()`; net 0 ("") skipped
- Footprint pad geometry registered in `FootprintLibrary` (autorouter needs this)
- Existing traces (`segment`) and vias (`via`) extracted as `RoutingResult` for reference scoring
- Dimensions convert mm → nm via `Nm::from_mm()`
- 3 benchmark `.kicad_pcb` files stored in `tests/fixtures/benchmark/` with `KicadBenchmark` metadata
- CLI `parse-kicad` subcommand prints parse metadata as JSON
- Parsed `BoardWorld` produces valid ratsnest via `extract_ratsnest()` (proves S03 compatibility)

## Proof Level

- This slice proves: contract (parser contract for S03/S07 consumers)
- Real runtime required: no (unit + integration tests only, no browser/WASM)
- Human/UAT required: no

## Integration Closure

- Upstream surfaces consumed: `symbolic_expressions::parser::parse_str()` for S-expr tokenization, `cypcb-world` BoardWorld/FootprintLibrary API, `cypcb-router::types::RoutingResult` for reference routes
- New wiring introduced in this slice: `cypcb-kicad::pcb_parser` module, `parse-kicad` CLI subcommand, `cypcb-cli` → `cypcb-kicad` dependency
- What remains before the milestone is truly usable end-to-end: S02 (scoring), S03 (PathFinder routing engine), S04-S07

## Tasks

- [x] **T01: Build core .kicad_pcb S-expression parser** `est:3h`
  - Why: The parser is the primary deliverable of this slice. KiCad 7/8 uses `footprint` keyword which `kicad_parse_gen` doesn't handle — need custom parser using `symbolic_expressions` crate directly.
  - Files: `crates/cypcb-kicad/src/pcb_parser.rs`, `crates/cypcb-kicad/src/lib.rs`, `crates/cypcb-kicad/Cargo.toml`, `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb`, `crates/cypcb-kicad/tests/pcb_parser_tests.rs`
  - Do: Add `symbolic_expressions` dependency. Create `pcb_parser.rs` with `parse_kicad_pcb()` that tokenizes via `symbolic_expressions::parser::parse_str()`, walks S-expr tree extracting: version, nets, board outline (gr_line/gr_rect on Edge.Cuts), footprints (with pads, position, rotation, refdes, value, net connections), traces (segment), vias. Returns `KicadPcbParseResult { world, library, reference_routes, metadata }`. Create a hand-written minimal `.kicad_pcb` test fixture with KiCad 8 format (2 footprints, 3 nets, 1 segment, 1 via, board outline). Unit tests verify component count, net count, pad positions, net assignments, board size, and reference route extraction.
  - Verify: `cargo test -p cypcb-kicad pcb_parser` passes with all assertions
  - Done when: `parse_kicad_pcb()` correctly parses a synthetic KiCad 8 fixture into `BoardWorld` with verified component/net/pad/trace/via data

- [x] **T02: Curate benchmark fixtures and add KicadBenchmark metadata** `est:2h`
  - Why: R102 requires 3 real KiCad benchmark boards of varying complexity. These fixtures validate the parser against real-world files and provide ground truth for S03/S07 autorouter benchmarking.
  - Files: `tests/fixtures/benchmark/led_blink.kicad_pcb`, `tests/fixtures/benchmark/stm32_breakout.kicad_pcb`, `tests/fixtures/benchmark/multi_ic.kicad_pcb`, `tests/fixtures/benchmark/README.md`, `crates/cypcb-kicad/src/pcb_parser.rs` (add `KicadBenchmark`), `crates/cypcb-kicad/tests/benchmark_parse.rs`
  - Do: Find and download 3 open-source KiCad 7/8 projects from GitHub (simple: LED blink <10 components; medium: STM32 breakout 20-50 components; complex: multi-IC 50+ components). Store `.kicad_pcb` files in `tests/fixtures/benchmark/`. Add `KicadBenchmark` and `BenchmarkComplexity` types to `pcb_parser.rs`. Add `BENCHMARKS` constant array listing all fixtures with expected counts. Write integration test `benchmark_parse.rs` that parses each file, asserts component/net counts within expected range, verifies board outline is non-zero, verifies nets are interned. Add README.md documenting each fixture (source, license, complexity).
  - Verify: `cargo test -p cypcb-kicad --test benchmark_parse` passes — all 3 files parse, counts match expected
  - Done when: 3 real `.kicad_pcb` files parse successfully with correct metadata, benchmark descriptors exist

- [x] **T03: CLI parse-kicad command and ratsnest compatibility proof** `est:1h`
  - Why: Closes the slice by proving the parser output is consumable by downstream slices. CLI command makes the parser accessible for manual inspection. Ratsnest compatibility proves S03 can consume this data.
  - Files: `crates/cypcb-cli/Cargo.toml`, `crates/cypcb-cli/src/commands/parse_kicad.rs`, `crates/cypcb-cli/src/commands/mod.rs`, `crates/cypcb-cli/src/main.rs`, `crates/cypcb-kicad/tests/ratsnest_compat.rs`
  - Do: Add `cypcb-kicad` dependency to CLI crate. Create `parse_kicad.rs` command that calls `parse_kicad_pcb()`, prints metadata as JSON (version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count). Register in mod.rs and main.rs as `parse-kicad` subcommand. Write `ratsnest_compat.rs` integration test: parse the simplest benchmark fixture, call `extract_ratsnest(&mut world, &library)`, assert ratsnest is non-empty with correct net count.
  - Verify: `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` prints valid JSON. `cargo test -p cypcb-kicad --test ratsnest_compat` passes.
  - Done when: CLI command works on all 3 benchmark files, parsed BoardWorld produces valid ratsnest for autorouter consumption

## Observability / Diagnostics

- **Parse errors surface structured context:** `KicadPcbError` variants include field name, expected format, and KiCad version so failures are diagnosable without re-running under a debugger.
- **Metadata as inspection surface:** `KicadPcbMetadata` (version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count) is returned from every parse and printed by the CLI — serves as the primary runtime signal for correctness.
- **CLI JSON output:** `parse-kicad` subcommand emits metadata as structured JSON to stdout, enabling scripted verification and CI assertions.
- **Test-visible failure state:** Unit tests assert specific error variants for malformed/unsupported input. Benchmark integration tests assert counts within expected ranges and will fail with concrete diffs if the parser regresses.
- **Redaction:** No secrets or user data in `.kicad_pcb` files. No redaction constraints.

## Verification

- `cargo test -p cypcb-kicad` — all parser unit tests pass (synthetic fixture: counts, positions, nets)
- `cargo test -p cypcb-kicad --test benchmark_parse` — integration tests parse all 3 real benchmark files, assert component/net counts match expected
- `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — exits 0, prints valid JSON with correct metadata
- `cargo test -p cypcb-kicad --test ratsnest_compat` — parsed BoardWorld + FootprintLibrary feeds `extract_ratsnest()` producing non-empty ratsnest
- **Failure-path check:** `parse_kicad_pcb_str("")` returns `KicadPcbError::SexprParseError`; `parse_kicad_pcb_str("(kicad_pcb (version 1))")` returns `KicadPcbError::UnsupportedVersion` — unit tests assert both.

## Files Likely Touched

- `crates/cypcb-kicad/src/pcb_parser.rs` — NEW: core .kicad_pcb parser
- `crates/cypcb-kicad/src/lib.rs` — add `pub mod pcb_parser` + re-exports
- `crates/cypcb-kicad/Cargo.toml` — add `symbolic_expressions` + `cypcb-router` deps
- `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — synthetic test fixture
- `crates/cypcb-kicad/tests/pcb_parser_tests.rs` — unit tests
- `crates/cypcb-kicad/tests/benchmark_parse.rs` — benchmark integration tests
- `crates/cypcb-kicad/tests/ratsnest_compat.rs` — ratsnest compatibility test
- `tests/fixtures/benchmark/*.kicad_pcb` — 3 benchmark fixture files
- `tests/fixtures/benchmark/README.md` — fixture documentation
- `crates/cypcb-cli/Cargo.toml` — add `cypcb-kicad` dep
- `crates/cypcb-cli/src/commands/parse_kicad.rs` — NEW: parse-kicad command
- `crates/cypcb-cli/src/commands/mod.rs` — register parse_kicad
- `crates/cypcb-cli/src/main.rs` — add ParseKicad subcommand
