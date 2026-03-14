---
estimated_steps: 5
estimated_files: 7
---

# T02: Curate benchmark fixtures and add KicadBenchmark metadata

**Slice:** S01 — KiCad PCB Parser & Benchmark Fixtures
**Milestone:** M004

## Description

Find and download 3 real open-source KiCad 7/8 PCB projects of varying complexity (simple, medium, complex), store them as benchmark fixtures, and write integration tests that parse each file and verify correctness. This directly delivers R102 — the benchmark suite that S03/S07 will use for autorouter validation.

## Steps

1. **Find 3 open-source KiCad 7+ projects** on GitHub with permissive licenses:
   - Simple: LED blink or similar (<10 components, <10 nets, 2-layer). Search for small KiCad 8 projects.
   - Medium: STM32/ESP32 breakout or Arduino shield (20-50 components, 20-80 nets, 2-layer).
   - Complex: Multi-IC board (50+ components, 80+ nets, 2+ layers).
   - Prefer projects with `.kicad_pcb` using the `footprint` keyword (KiCad 7/8).
   - If suitable real files aren't found for all tiers, create a realistic synthetic medium-complexity fixture by hand.

2. **Store fixture files** in `tests/fixtures/benchmark/`:
   - `led_blink.kicad_pcb` (simple)
   - `stm32_breakout.kicad_pcb` (medium) — or equivalent name matching the actual project
   - `multi_ic.kicad_pcb` (complex) — or equivalent name
   - Create `tests/fixtures/benchmark/README.md` documenting: source URL, license, original project name, complexity tier, expected component/net counts.

3. **Add `BENCHMARKS` constant** to `crates/cypcb-kicad/src/pcb_parser.rs`:
   - Array of `KicadBenchmark` descriptors (name, relative path from workspace root, complexity, expected counts, description).
   - Helper function `get_benchmarks() -> Vec<KicadBenchmark>` that resolves paths relative to the workspace root.

4. **Write integration test** `crates/cypcb-kicad/tests/benchmark_parse.rs`:
   - For each benchmark fixture:
     - Parse with `parse_kicad_pcb()`
     - Assert `metadata.component_count` is within expected range (±20% tolerance for medium/complex)
     - Assert `metadata.net_count` is within expected range
     - Assert board size is non-zero
     - Assert `library` has entries (footprints registered)
     - Assert `world.net_count() > 0`
   - Test names: `test_parse_led_blink`, `test_parse_stm32_breakout`, `test_parse_multi_ic`
   - One parametric test iterating all benchmarks: `test_all_benchmarks_parse`

5. **Fix any parser issues** discovered when parsing real files — real KiCad files will likely exercise edge cases the synthetic fixture didn't cover (unknown layer names, missing fields, nested footprint attributes, `fp_text` with `effects` blocks, etc.). Update parser with graceful handling and update tests.

## Observability Impact

- **Benchmark metadata as inspection surface:** `KicadBenchmark` descriptors expose expected component/net counts and complexity tier per fixture, making correctness assertions automatic. `get_benchmarks()` returns all descriptors for programmatic iteration.
- **Integration test coverage:** `benchmark_parse.rs` tests assert counts within ±20% tolerance. Test failures produce concrete diffs (expected vs actual counts) — no guessing needed.
- **README.md as documentation surface:** Each fixture is documented with source URL, license, and expected counts — a future agent can inspect this file to understand what benchmarks exist and where they came from.
- **Parser edge-case hardening:** Real-world files exercise edge cases (unknown layers, missing fields, `fp_text` variants). Parser fixes are observable through the existing `KicadPcbError` structured error variants.

## Must-Haves

- [ ] 3 `.kicad_pcb` files stored in `tests/fixtures/benchmark/` — one per complexity tier
- [ ] All 3 files parse without errors via `parse_kicad_pcb()`
- [ ] `KicadBenchmark` descriptors match actual file contents (counts within tolerance)
- [ ] README.md documents source, license, and complexity for each fixture
- [ ] Integration tests assert component count, net count, board size, library registration

## Verification

- `cargo test -p cypcb-kicad --test benchmark_parse` — all tests pass
- `ls tests/fixtures/benchmark/*.kicad_pcb | wc -l` returns 3
- `cat tests/fixtures/benchmark/README.md` shows source URLs and licenses

## Inputs

- `crates/cypcb-kicad/src/pcb_parser.rs` — parser from T01 (must already handle `footprint` keyword)
- `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — synthetic fixture from T01 (pattern reference)
- S01-RESEARCH.md — benchmark board selection criteria, `KicadBenchmark` type definition

## Expected Output

- `tests/fixtures/benchmark/led_blink.kicad_pcb` — simple benchmark file
- `tests/fixtures/benchmark/stm32_breakout.kicad_pcb` — medium benchmark file (name may vary)
- `tests/fixtures/benchmark/multi_ic.kicad_pcb` — complex benchmark file (name may vary)
- `tests/fixtures/benchmark/README.md` — fixture documentation
- `crates/cypcb-kicad/src/pcb_parser.rs` — updated with `BENCHMARKS` / `get_benchmarks()`
- `crates/cypcb-kicad/tests/benchmark_parse.rs` — integration tests for all 3 files
- `crates/cypcb-kicad/src/pcb_parser.rs` — parser fixes for real-world edge cases
