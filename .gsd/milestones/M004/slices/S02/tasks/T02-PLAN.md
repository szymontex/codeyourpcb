---
estimated_steps: 4
estimated_files: 5
---

# T02: CLI score command and integration tests with baseline scores

**Slice:** S02 — Routing Quality Score System
**Milestone:** M004

## Description

Wire `score_board()` into the CLI and write integration tests that route real .cypcb boards, score them, and assert baseline metric values. This proves the scoring contract works end-to-end and establishes the baseline scores that S07's benchmark suite will compare against.

## Steps

1. **Create `score.rs` CLI command** — Follow `CheckCommand` pattern. `ScoreCommand` takes a .cypcb file path. Implementation: read file, parse with `CypcbParser`, build `BoardWorld` via `sync_ast_to_world`, build rules (JLCPCB 2-layer default), route with `route_board()`, apply routes, rebuild spatial index with traces, call `score_board()`, print `RoutingScore` as JSON via serde_json (add serde_json to cypcb-cli if not present) or manual format. Include `--weights` flag (optional, future-proofing — default weights for now).

2. **Register command** — Add `mod score;` and `pub use score::ScoreCommand;` in `commands/mod.rs`. Add `Score(commands::ScoreCommand)` variant to `Commands` enum in `main.rs`. Add match arm `Commands::Score(cmd) => cmd.run()`.

3. **Write integration tests** — New file `crates/cypcb-autoroute/tests/scoring_integration.rs`. Follow existing integration.rs pattern (`parse_board()`, `test_rules()` helpers). Tests:
   - `score_routed_blink`: Route blink.cypcb → apply routes → rebuild spatial index → score. Assert: total_length > Nm(0), via_count is u32, drc_violations == 0, smoothness in [0.0, 1.0], crossings == 0 (well-routed board shouldn't cross), layer_balance in [0.0, 1.0], composite > 0.0.
   - `score_routed_routing_test`: Same flow for routing-test.cypcb (3 components, 3 nets). Simpler board → verify scoring works on minimal input.
   - `score_empty_board_is_valid`: Score an unrouted board — verify no panic, metrics reflect empty state (total_length=0, via_count=0).
   - `score_json_serialization`: Route a board, score it, serialize to JSON string, verify it parses back and contains all 7 field names.

4. **Verify CLI build and full test suite** — Run `cargo build -p cypcb-cli` (may need pkg-config — if unavailable, just verify `cargo check -p cypcb-cli`). Run `cargo test -p cypcb-autoroute` for all tests including both unit and integration.

## Must-Haves

- [ ] CLI `score` command compiles and is registered in command dispatch
- [ ] ScoreCommand reads .cypcb file, routes, scores, outputs JSON
- [ ] Integration test routes blink.cypcb and validates all 7 metric ranges
- [ ] Integration test routes routing-test.cypcb successfully
- [ ] Scoring an empty/unrouted board does not panic
- [ ] RoutingScore JSON contains all 7 metric field names

## Verification

- `cargo test -p cypcb-autoroute --test scoring_integration` — all integration tests pass
- `cargo test -p cypcb-autoroute` — full test suite passes (unit + integration)
- `cargo check -p cypcb-cli` — CLI compiles with score command

## Inputs

- `crates/cypcb-autoroute/src/scoring.rs` — RoutingScore, score_board() from T01
- `crates/cypcb-autoroute/tests/integration.rs` — pattern for parse_board(), test_rules() helpers
- `crates/cypcb-cli/src/commands/check.rs` — pattern for simple CLI command
- `crates/cypcb-cli/src/commands/mod.rs` — command registration pattern
- `crates/cypcb-cli/src/main.rs` — subcommand dispatch pattern
- `examples/blink.cypcb` — primary test fixture (8 components, 7 nets)
- `examples/routing-test.cypcb` — secondary test fixture (3 components, 3 nets)

## Expected Output

- `crates/cypcb-cli/src/commands/score.rs` — new ScoreCommand implementation
- `crates/cypcb-cli/src/commands/mod.rs` — updated with score module registration
- `crates/cypcb-cli/src/main.rs` — updated with Score variant
- `crates/cypcb-autoroute/tests/scoring_integration.rs` — new integration test file with 4 tests validating scoring on real boards

## Observability Impact

- **CLI JSON output**: `cypcb score <file>` prints `RoutingScore` as pretty-printed JSON to stdout, making all 7 metrics inspectable by agents and CI scripts via `jq` or JSON parsing.
- **Structured error reporting**: File-not-found and parse errors are surfaced via `miette::Report` with file path context — agents see which file failed and why.
- **Integration test diagnostics**: Each scoring integration test prints metric values via `eprintln!` for CI log inspection when `--nocapture` is used.
- **Failure visibility**: Empty/unrouted board scoring returns well-defined defaults (length=0, vias=0, smoothness=1.0, balance=1.0, composite=0.0) — no panics, visible in test output.
