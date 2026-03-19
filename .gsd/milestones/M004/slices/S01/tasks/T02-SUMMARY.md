---
id: T02
parent: S01
milestone: M004
provides:
  - 3 benchmark .kicad_pcb fixtures (simple/medium/complex) in tests/fixtures/benchmark/
  - BENCHMARKS constant and get_benchmarks() in pcb_parser.rs for programmatic fixture access
  - Integration tests in benchmark_parse.rs validating all 3 fixtures parse with correct metadata
  - README.md documenting each fixture's source, license, complexity, and expected counts
key_files:
  - tests/fixtures/benchmark/led_blink.kicad_pcb
  - tests/fixtures/benchmark/stm32_breakout.kicad_pcb
  - tests/fixtures/benchmark/multi_ic.kicad_pcb
  - tests/fixtures/benchmark/README.md
  - crates/cypcb-kicad/src/pcb_parser.rs
  - crates/cypcb-kicad/tests/benchmark_parse.rs
key_decisions:
  - Created synthetic realistic fixtures instead of downloading real projects — search did not yield directly downloadable KiCad 8 .kicad_pcb files with permissive licenses; synthetic files are license-clean and purpose-built to cover the target complexity tiers
  - Changed KicadBenchmark struct fields from String to &'static str to enable BENCHMARKS as a const array (static lifetime requirement for compile-time constants)
patterns_established:
  - workspace_root() helper in integration tests resolves CARGO_MANIFEST_DIR up 2 levels to find workspace-level tests/fixtures/ directory
  - assert_within_tolerance() helper with configurable ±% tolerance for medium/complex benchmark counts (exact match for simple)
observability_surfaces:
  - KicadBenchmark descriptors in BENCHMARKS constant — programmatic access to expected counts per fixture
  - get_benchmarks() returns (KicadBenchmark, PathBuf) pairs for test iteration
  - Integration test failures produce concrete diffs showing expected range vs actual count
duration: 20min
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Curate benchmark fixtures and add KicadBenchmark metadata

**Created 3 synthetic KiCad 8 benchmark PCB fixtures (simple/medium/complex) with BENCHMARKS constant and integration tests — all parse successfully with correct metadata.**

## What Happened

1. Searched for real open-source KiCad 7/8 projects on GitHub but could not obtain directly downloadable files. Per task plan fallback, created realistic synthetic fixtures covering all 3 complexity tiers.

2. Created `tests/fixtures/benchmark/` with 3 `.kicad_pcb` files:
   - **led_blink.kicad_pcb** (Simple): 7 components, 7 nets, 2-layer, 40×30mm — LED circuit with connector, switch, resistors, capacitors, LED.
   - **stm32_breakout.kicad_pcb** (Medium): 29 components, 40 nets, 2-layer, 75×65mm — STM32F103C8T6 with USB-C, voltage regulator, crystal, SWD header, GPIO headers, LEDs, I2C pull-ups, reset circuit.
   - **multi_ic.kicad_pcb** (Complex): 52 components, 94 nets, 4-layer, 100×80mm — STM32F407 LQFP-100 + LAN8720A Ethernet PHY QFN-24 + W25Q32 SPI Flash + CAN transceiver + 2 voltage regulators + Ethernet magnetics + USB ESD + RJ45.

3. Added `BENCHMARKS` const array and `get_benchmarks()` to `pcb_parser.rs`. Changed `KicadBenchmark` fields from `String` to `&'static str` for const compatibility.

4. Wrote `benchmark_parse.rs` integration test with 5 tests: 3 individual fixture tests, 1 parametric test over all benchmarks, 1 constant-vs-files consistency test. Tolerance: exact for simple, ±20% for medium/complex.

5. No parser fixes needed — all 3 synthetic fixtures parsed on first try with the existing T01 parser. No edge cases discovered (expected since the fixtures are synthetic).

## Verification

- `cargo test -p cypcb-kicad --test benchmark_parse` — **5/5 tests pass**: test_parse_led_blink, test_parse_stm32_breakout, test_parse_multi_ic, test_all_benchmarks_parse, test_benchmarks_constant_matches_files
- `cargo test -p cypcb-kicad` — **37 total tests pass** (22 unit + 5 benchmark + 10 pcb_parser integration)
- `cargo clippy -p cypcb-kicad -- -D warnings` — clean
- `ls tests/fixtures/benchmark/*.kicad_pcb | wc -l` — returns 3
- `cat tests/fixtures/benchmark/README.md` — documents source, license, complexity for each fixture

### Slice-level verification (partial — T02 is second task):
- ✅ `cargo test -p cypcb-kicad` — all parser unit tests pass
- ✅ `cargo test -p cypcb-kicad --test benchmark_parse` — integration tests parse all 3 benchmark files, counts match
- ⏳ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — not yet (T03)
- ⏳ `cargo test -p cypcb-kicad --test ratsnest_compat` — not yet (T03)
- ✅ Failure-path check: empty input → SexprParseError, unsupported version → UnsupportedVersion (T01 tests still pass)

## Diagnostics

- Parse any benchmark file with `parse_kicad_pcb(path)` and inspect `result.metadata` for component/net/trace/via counts.
- `BENCHMARKS` constant and `get_benchmarks()` provide programmatic access to all fixture descriptors.
- Integration test failures show concrete "expected X ±Y%, got Z" messages.

## Deviations

- Used synthetic fixtures instead of real open-source projects — search couldn't locate directly downloadable KiCad 8 .kicad_pcb files. The task plan explicitly allows this as a fallback.
- Changed `KicadBenchmark` fields from `String` to `&'static str` — required for `const` array usage. This is a minor API change that doesn't affect consumers (both deref to `str`).
- No parser fixes needed (Step 5) — synthetic files don't exercise the same edge cases real files would. Real-world edge case handling may still be needed when actual KiCad project files are tested.

## Known Issues

- Synthetic fixtures don't exercise all real-world KiCad edge cases (e.g., gr_arc board outlines, custom pad shapes, zone fills, keepout areas, net classes with grouping). When real KiCad files are eventually tested, parser fixes may be needed.
- The multi_ic.kicad_pcb fixture was generated with a Python script for efficiency — if regeneration is needed, the script is at `/tmp/gen_multi_ic.py` (not committed).

## Files Created/Modified

- `tests/fixtures/benchmark/led_blink.kicad_pcb` — NEW: simple benchmark (7 components, 7 nets, 2-layer)
- `tests/fixtures/benchmark/stm32_breakout.kicad_pcb` — NEW: medium benchmark (29 components, 40 nets, 2-layer)
- `tests/fixtures/benchmark/multi_ic.kicad_pcb` — NEW: complex benchmark (52 components, 94 nets, 4-layer)
- `tests/fixtures/benchmark/README.md` — NEW: fixture documentation
- `crates/cypcb-kicad/src/pcb_parser.rs` — added BENCHMARKS const, get_benchmarks(), changed KicadBenchmark to &'static str fields
- `crates/cypcb-kicad/tests/benchmark_parse.rs` — NEW: 5 integration tests for benchmark fixtures
- `.gsd/milestones/M004/slices/S01/tasks/T02-PLAN.md` — added Observability Impact section
