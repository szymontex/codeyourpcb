---
id: T03
parent: S01
milestone: M004
provides:
  - "`parse-kicad` CLI subcommand that parses .kicad_pcb files and emits KicadPcbMetadata as JSON"
  - "Ratsnest compatibility proof: parsed BoardWorld + FootprintLibrary feeds extract_ratsnest() successfully"
key_files:
  - crates/cypcb-cli/src/commands/parse_kicad.rs
  - crates/cypcb-kicad/tests/ratsnest_compat.rs
key_decisions:
  - "Added serde as explicit dependency to cypcb-kicad crate to derive Serialize on KicadPcbMetadata"
patterns_established:
  - "CLI subcommand pattern: Args struct with run() method, miette error wrapping, JSON output via serde_json::to_string_pretty"
observability_surfaces:
  - "`cypcb parse-kicad <file>` emits structured JSON metadata to stdout (version, counts, board size, layers)"
  - "ratsnest_compat integration test prints net extraction counts to stderr for diagnostic visibility"
  - "CLI parse failures surface as miette-formatted diagnostics with file path context on stderr"
duration: 15min
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T03: CLI parse-kicad command and ratsnest compatibility proof

**Wired KiCad parser into CLI as `parse-kicad` subcommand and proved parsed BoardWorld feeds autorouter's extract_ratsnest() on all 3 benchmark fixtures.**

## What Happened

1. Added `cypcb-kicad` dependency to `cypcb-cli/Cargo.toml` and `serde` to `cypcb-kicad/Cargo.toml`.
2. Derived `serde::Serialize` on `KicadPcbMetadata` for JSON output.
3. Created `parse_kicad.rs` CLI command — accepts a `.kicad_pcb` file path, calls `parse_kicad_pcb()`, prints metadata as pretty JSON.
4. Registered `ParseKicad` variant in the CLI commands enum and match arm.
5. Added `cypcb-autoroute` as dev-dependency to `cypcb-kicad` and created `ratsnest_compat.rs` with two tests: one proving led_blink produces non-empty ratsnest with ≥2-pad nets, another proving all 3 benchmarks extract ratsnest without panic.

## Verification

**All slice-level verification checks pass (this is the final task of S01):**

- ✅ `cargo test -p cypcb-kicad` — 22 unit tests pass
- ✅ `cargo test -p cypcb-kicad --test benchmark_parse` — 5 integration tests pass (all 3 fixtures, counts match)
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb` — exits 0, valid JSON: 7 components, 7 nets, 2 layers, 40×30mm
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/stm32_breakout.kicad_pcb` — exits 0, valid JSON: 29 components, 40 nets
- ✅ `cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/multi_ic.kicad_pcb` — exits 0, valid JSON: 52 components, 94 nets, 4 layers
- ✅ `cargo test -p cypcb-kicad --test ratsnest_compat` — 2 tests pass, ratsnest non-empty, multi-pad nets present
- ✅ `cargo build -p cypcb-cli` — compiles without errors
- ✅ Failure-path checks: unit tests assert `SexprParseError` for empty input and `UnsupportedVersion` for version 1

## Diagnostics

- Run `cypcb parse-kicad <file>` on any `.kicad_pcb` to get structured JSON metadata.
- `cargo test -p cypcb-kicad --test ratsnest_compat -- --nocapture` shows net extraction counts per benchmark.
- Parse errors include miette-formatted context with file path and structured `KicadPcbError` variants.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-cli/Cargo.toml` — added `cypcb-kicad` dependency
- `crates/cypcb-cli/src/commands/parse_kicad.rs` — new CLI command for parsing .kicad_pcb files
- `crates/cypcb-cli/src/commands/mod.rs` — registered parse_kicad module and export
- `crates/cypcb-cli/src/main.rs` — added ParseKicad subcommand variant and match arm
- `crates/cypcb-kicad/Cargo.toml` — added serde dependency and cypcb-autoroute dev-dependency
- `crates/cypcb-kicad/src/pcb_parser.rs` — added `Serialize` derive to `KicadPcbMetadata`
- `crates/cypcb-kicad/tests/ratsnest_compat.rs` — new integration test proving autorouter compatibility
- `.gsd/milestones/M004/slices/S01/tasks/T03-PLAN.md` — added Observability Impact section
