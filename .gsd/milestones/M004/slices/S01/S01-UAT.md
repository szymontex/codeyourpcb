# S01: KiCad PCB Parser & Benchmark Fixtures — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice produces a parser library and CLI tool — all verification is via unit/integration tests, CLI output inspection, and test assertions. No browser, WASM, or live runtime required.

## Preconditions

- Rust toolchain installed (`cargo` available)
- Working directory is workspace root (`/workspace/codeyourpcb`)
- All crate dependencies resolved (`cargo fetch` or first build succeeds)
- Benchmark fixtures exist at `tests/fixtures/benchmark/*.kicad_pcb` (3 files)
- Synthetic test fixture exists at `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb`

## Smoke Test

Run `cargo test -p cypcb-kicad` — all 39 tests should pass (22 unit + 17 integration). If this fails, stop and investigate before proceeding.

## Test Cases

### 1. Core parser — minimal fixture

1. Run `cargo test -p cypcb-kicad --test pcb_parser_tests`
2. **Expected:** 10 tests pass:
   - `test_parse_minimal_fixture` — parses fixture, asserts version=20240108, 2 components, 3 nets, board size 40×30mm
   - `test_component_positions` — R1 at (10,15), LED1 at (25,15)
   - `test_pad_net_assignments` — pads assigned to correct nets (VCC, GND, NET1)
   - `test_reference_routes_extracted` — 1 trace segment + 1 via extracted
   - `test_footprint_library_registered` — both footprints in library by full name
   - `test_net_zero_skipped` — net 0 ("") not interned
   - `test_empty_input_returns_sexpr_error` — `parse_kicad_pcb_str("")` returns `SexprParseError`
   - `test_unsupported_version_returns_error` — version 1 returns `UnsupportedVersion`
   - `test_module_keyword_backward_compat` — KiCad 5 `module` keyword parsed same as `footprint`
   - `test_layer_count_extraction` — 2-layer count detected from layer definitions

### 2. Benchmark fixtures — all three parse

1. Run `cargo test -p cypcb-kicad --test benchmark_parse`
2. **Expected:** 5 tests pass:
   - `test_parse_led_blink` — 7 components, 7 nets, 2 layers, board size ≈40×30mm
   - `test_parse_stm32_breakout` — 29 components, 40 nets, 2 layers, board size ≈75×65mm
   - `test_parse_multi_ic` — 52 components, 94 nets, 4 layers, board size ≈100×80mm
   - `test_all_benchmarks_parse` — iterates `get_benchmarks()`, all parse with counts in expected range
   - `test_benchmarks_constant_matches_files` — `BENCHMARKS` constant entries match actual fixture file count

### 3. CLI parse-kicad — LED blink fixture

1. Run `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb`
2. **Expected:** Exit code 0, stdout contains valid JSON:
   ```json
   {
     "version": 20240108,
     "component_count": 7,
     "net_count": 7,
     "trace_segment_count": 3,
     "via_count": 2,
     "board_size_mm": [40.0, 30.0],
     "layer_count": 2
   }
   ```

### 4. CLI parse-kicad — STM32 breakout fixture

1. Run `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/stm32_breakout.kicad_pcb`
2. **Expected:** Exit code 0, JSON output shows `component_count: 29`, `net_count: 40`, `layer_count: 2`

### 5. CLI parse-kicad — multi-IC fixture

1. Run `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/multi_ic.kicad_pcb`
2. **Expected:** Exit code 0, JSON output shows `component_count: 52`, `net_count: 94`, `layer_count: 4`

### 6. Ratsnest compatibility — autorouter readiness

1. Run `cargo test -p cypcb-kicad --test ratsnest_compat`
2. **Expected:** 2 tests pass:
   - `ratsnest_from_led_blink_is_nonempty` — ratsnest has entries, at least some nets connect ≥2 pads
   - `ratsnest_from_all_benchmarks_succeeds` — all 3 benchmarks produce non-empty ratsnest without panic

### 7. Full crate test suite

1. Run `cargo test -p cypcb-kicad`
2. **Expected:** 39 tests pass (22 unit + 10 parser integration + 5 benchmark + 2 ratsnest), 0 failures

## Edge Cases

### Empty input error handling

1. The unit test `test_empty_input_returns_error` in `pcb_parser.rs` calls `parse_kicad_pcb_str("")`
2. **Expected:** Returns `KicadPcbError::SexprParseError`, not a panic

### Unsupported KiCad version

1. The unit test `test_unsupported_version_returns_error` calls `parse_kicad_pcb_str("(kicad_pcb (version 1))")`
2. **Expected:** Returns `KicadPcbError::UnsupportedVersion`, not silently continuing with bad data

### KiCad 5 module keyword backward compatibility

1. The integration test `test_module_keyword_backward_compat` uses `module` instead of `footprint`
2. **Expected:** Parser handles both keywords identically, producing valid BoardWorld

### Net 0 (empty net) skipped

1. The integration test `test_net_zero_skipped` verifies net 0 with name "" is not interned
2. **Expected:** Only nets with non-empty names appear in the parsed BoardWorld

### CLI with non-existent file

1. Run `cargo run -p cypcb-cli -- parse-kicad nonexistent.kicad_pcb`
2. **Expected:** Non-zero exit code, error message on stderr mentioning file not found

## Failure Signals

- Any `cargo test -p cypcb-kicad` failure — indicates parser regression or fixture corruption
- CLI `parse-kicad` exits non-zero on valid fixture — indicates CLI wiring or parser error
- JSON output has 0 components or 0 nets on benchmark fixture — parser is not extracting data
- `ratsnest_compat` tests panic or produce empty ratsnest — BoardWorld structure incompatible with autorouter, blocking S03
- Clippy warnings on `cypcb-kicad` — code quality regression

## Requirements Proved By This UAT

- R101 (KiCad .kicad_pcb Board Parser) — tests 1-7 prove parser extracts outline, footprints, pads, nets, traces, vias from KiCad 5-8 format
- R102 (Benchmark Suite) — tests 2-5 prove 3 fixtures exist, parse correctly, and have programmatic metadata. Edge case: fixtures are synthetic not downloaded, but serve same validation purpose.

## Not Proven By This UAT

- R102 completeness: fixtures are synthetic, not from real open-source KiCad projects. Edge cases from real-world KiCad files (gr_arc outlines, custom pad shapes, zone fills) are not tested.
- Reference routing scoring (R103/S02) — reference routes are extracted but not scored
- Autorouter consuming parsed BoardWorld for actual routing (S03) — only ratsnest extraction is proven, not full routing pipeline
- WASM integration — parser is Rust-only, no browser/WASM testing

## Notes for Tester

- The 3 benchmark fixtures are synthetic KiCad 8 files, not actual KiCad project outputs. They use valid KiCad S-expression format but don't exercise every possible KiCad feature (arcs, zones, keepouts, etc.).
- Board outline is always a bounding-box rectangle, even if the fixture used only gr_line elements. This is by design — polygon outlines deferred.
- The `parse-kicad` CLI command writes JSON to stdout and errors to stderr. Pipe stdout to `jq` for inspection: `cargo run -p cypcb-cli -- parse-kicad <file> | jq .`
- To run tests with output visible: `cargo test -p cypcb-kicad --test ratsnest_compat -- --nocapture`
