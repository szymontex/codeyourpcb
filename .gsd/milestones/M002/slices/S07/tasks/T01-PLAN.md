---
estimated_steps: 5
estimated_files: 8
---

# T01: Rust lint cleanup — cargo fmt + clippy fix

**Slice:** S07 — E2E Test Suite & Quality Gates
**Milestone:** M002

## Description

680 rustfmt diffs and 122 clippy warnings across the workspace block the quality gate. This task applies `cargo fmt`, then systematically fixes all clippy warnings, then fixes the 3 pre-existing test failures. Desktop crates (cypcb-cli, cypcb-desktop) are excluded — they can't compile in this environment due to missing system deps.

The ordering matters: fmt first (bulk reformatting in one commit), clippy auto-fix second, manual clippy fixes third, test fixes last. This avoids wasted effort from reformatting code you just manually edited.

## Steps

1. Run `cargo fmt` across the entire workspace. Verify with `cargo fmt --check`. Commit: `style: cargo fmt across workspace`.
2. Run `cargo clippy --fix --workspace --exclude cypcb-cli --exclude cypcb-desktop --allow-dirty --allow-staged` to auto-fix mechanical warnings (unused imports, Option::map patterns, redundant closures, etc.).
3. Run `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` to identify remaining warnings. For `ptr_arg` on parser `&mut Vec<ParseError>` methods (14 instances), add `#[allow(clippy::ptr_arg)]` with a comment explaining the Vec is pushed to. For `too_many_arguments`, restructure into config structs where natural, `#[allow]` where restructuring would be churn.
4. Fix the 3 pre-existing test failures: (a) `test_export_duration_tracked` and `test_export_result_has_files` in cypcb-export — likely temp dir/filesystem issues, (b) `test_sync_named_pin` in cypcb-world — investigate root cause and fix.
5. Final verification: `cargo fmt --check` (zero diffs), `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` (zero warnings), `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop` (all pass).

## Must-Haves

- [ ] `cargo fmt --check` returns zero diffs across workspace
- [ ] `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` returns zero warnings
- [ ] All Rust tests pass (`cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop`)
- [ ] `#[allow(clippy::...)]` annotations include justification comments

## Verification

- `cargo fmt --check` — exit 0
- `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` — exit 0
- `cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop` — all pass, zero failures

## Inputs

- Research identified 122 clippy warnings (81 in cypcb-parser), 680 rustfmt diffs, 3 failing tests
- `ptr_arg` pattern in parser: 14 instances of `&mut Vec<ParseError>` in converter methods — these push to the Vec, so `&mut [ParseError]` would be wrong
- Desktop crates excluded: cypcb-cli and cypcb-desktop require pkg-config/gio-2.0 unavailable in this env

## Expected Output

- All `crates/*/src/*.rs` files reformatted and clippy-clean
- 3 previously-failing tests now pass
- Workspace compiles and tests cleanly with strict clippy
