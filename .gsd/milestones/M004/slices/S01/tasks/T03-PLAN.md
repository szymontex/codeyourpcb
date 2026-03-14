---
estimated_steps: 5
estimated_files: 5
---

# T03: CLI parse-kicad command and ratsnest compatibility proof

**Slice:** S01 — KiCad PCB Parser & Benchmark Fixtures
**Milestone:** M004

## Description

Wire the parser into the CLI as `parse-kicad` subcommand and prove the parser output is compatible with the autorouter's `extract_ratsnest()`. This closes the slice's boundary contract — S03 consumes `BoardWorld` + `FootprintLibrary` from this parser, so proving ratsnest extraction works is the critical integration proof.

## Steps

1. **Add `cypcb-kicad` dependency** to `crates/cypcb-cli/Cargo.toml`.

2. **Create `crates/cypcb-cli/src/commands/parse_kicad.rs`**:
   - `ParseKicadCommand` struct with `file: PathBuf` argument
   - `run()` method that calls `parse_kicad_pcb(&self.file)`, prints `KicadPcbMetadata` as JSON via `serde_json::to_string_pretty()`
   - On error, print the error with miette formatting
   - Add `Serialize` derive to `KicadPcbMetadata` in pcb_parser.rs if not already present

3. **Register the command** in `crates/cypcb-cli/src/commands/mod.rs` and `crates/cypcb-cli/src/main.rs`:
   - Add `mod parse_kicad; pub use parse_kicad::ParseKicadCommand;`
   - Add `ParseKicad(ParseKicadCommand)` variant to the `Commands` enum
   - Wire the match arm to call `cmd.run()`

4. **Write ratsnest compatibility test** `crates/cypcb-kicad/tests/ratsnest_compat.rs`:
   - Parse the simplest benchmark fixture (led_blink)
   - Call `cypcb_autoroute::orchestrator::extract_ratsnest(&mut result.world, &result.library)`
   - Assert the returned `Vec<NetRoute>` is non-empty
   - Assert the net count in the ratsnest matches the expected net count from the benchmark
   - This proves the parsed BoardWorld has correct component entities, footprint references, pad geometry in the library, and net connections — everything the autorouter needs
   - Add `cypcb-autoroute` as dev-dependency in `cypcb-kicad/Cargo.toml`

5. **Verify end-to-end**:
   - `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — prints JSON
   - Run on all 3 benchmark files — all succeed
   - `cargo test -p cypcb-kicad --test ratsnest_compat` — passes

## Must-Haves

- [ ] `parse-kicad` CLI subcommand accepts a `.kicad_pcb` file path and prints JSON metadata
- [ ] CLI exits 0 on all 3 benchmark files
- [ ] Parsed BoardWorld from benchmark fixture produces non-empty ratsnest via `extract_ratsnest()`
- [ ] `KicadPcbMetadata` is `Serialize`-able for JSON output

## Verification

- `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — exits 0, stdout is valid JSON with correct counts
- `cargo test -p cypcb-kicad --test ratsnest_compat` — ratsnest is non-empty, net count matches
- `cargo build -p cypcb-cli` — compiles without errors

## Inputs

- `crates/cypcb-kicad/src/pcb_parser.rs` — parser from T01 with `KicadPcbMetadata` type
- `tests/fixtures/benchmark/*.kicad_pcb` — benchmark fixtures from T02
- `crates/cypcb-autoroute/src/orchestrator.rs` — `extract_ratsnest()` function signature
- `crates/cypcb-cli/src/main.rs` — existing CLI structure (clap subcommands)

## Expected Output

- `crates/cypcb-cli/Cargo.toml` — updated with `cypcb-kicad` dependency
- `crates/cypcb-cli/src/commands/parse_kicad.rs` — new CLI command
- `crates/cypcb-cli/src/commands/mod.rs` — updated with parse_kicad module
- `crates/cypcb-cli/src/main.rs` — updated with ParseKicad subcommand
- `crates/cypcb-kicad/tests/ratsnest_compat.rs` — integration test proving autorouter compatibility

## Observability Impact

- **New CLI JSON output surface:** `cypcb parse-kicad <file>` emits `KicadPcbMetadata` as structured JSON to stdout. Future agents can pipe this through `jq` to inspect version, component_count, net_count, trace_segment_count, via_count, board_size_mm, and layer_count for any `.kicad_pcb` file.
- **Ratsnest extraction as integration smoke test:** `cargo test -p cypcb-kicad --test ratsnest_compat` verifies the parser→autorouter boundary. Failure here means the BoardWorld entities, footprint references, or net connections are malformed — the test output includes concrete counts for diagnosis.
- **CLI error reporting:** Parse failures surface as miette-formatted diagnostics on stderr with the file path context. Non-zero exit code signals failure to scripted callers.
- **Failure state visibility:** A parser regression that breaks ratsnest extraction will fail `ratsnest_compat` with assertion messages showing expected vs. actual net/pad counts. CLI failures on benchmark files will show the structured `KicadPcbError` variant.
