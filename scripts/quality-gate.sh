#!/usr/bin/env bash
set -euo pipefail

# Quality Gate — runs all lint, test, and E2E checks in sequence.
# Exits non-zero on first failure. Designed for CI and local pre-merge verification.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

pass() { echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; exit 1; }

echo "=== Quality Gate ==="
echo ""

# Stage 1: Rust formatting
echo "[1/8] cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo-fmt"
else
  fail "cargo-fmt"
fi
echo ""

# Stage 2: Clippy (strict, excluding desktop crates - they need system GTK/webkit)
echo "[2/8] cargo clippy"
if cargo clippy --workspace --exclude cypcb-desktop --all-targets -- -D warnings 2>&1; then
  pass "cargo-clippy"
else
  fail "cargo-clippy"
fi
echo ""

# Stage 3: Rust tests
echo "[3/8] cargo test"
# The Rust reader is what `parse` is now. The two tests that check it against
# the tree-sitter parser need that parser as well, which the plain run does not
# build - named explicitly, because a test nobody runs is not a test.
if cargo test --workspace --exclude cypcb-desktop 2>&1 \
  && cargo test -p cypcb-parser --features tree-sitter-parser --test differential 2>&1 \
  && cargo test -p cypcb-parser --features tree-sitter-parser --test error_parity 2>&1 \
  && cargo test -p cypcb-render --no-default-features --features wasm \
       --test the_browser_build_reads_the_language 2>&1; then
  pass "cargo-test"
else
  fail "cargo-test"
fi
echo ""

# Stage 4: ESLint
echo "[4/8] eslint"
if (cd viewer && npx eslint src/ e2e/ *.ts) 2>&1; then
  pass "eslint"
else
  fail "eslint"
fi
echo ""

# Stage 5: Vitest
echo "[5/8] vitest"
VITEST_LOG=$(mktemp)
if (cd viewer && npx vitest run 2>&1 | tee "$VITEST_LOG"); then
  pass "vitest"
else
  # Which test. Two stages have now failed inside a full gate run and passed
  # on their own, and neither left a name behind: a hundred lines of progress
  # scroll past and the stage ends in a bare "FAILED". A flake nobody can name
  # is a flake nobody can fix.
  echo ""
  echo "  failing tests:"
  grep -E "FAIL |^\s+×" "$VITEST_LOG" | head -20 || true
  fail "vitest"
fi
rm -f "$VITEST_LOG"
echo ""

# Stage 6: Playwright E2E
#
# The wasm bundle is rebuilt first, on purpose. `viewer/pkg` is a committed
# artifact and nothing else in this gate regenerates it, so the browser suite
# ran against whatever engine was compiled the last time somebody remembered -
# on 2026-08-08 that was three fires of Rust changes out of date, and an E2E
# test written to prove a silkscreen rule reached the browser passed against
# the old rule and failed against the new one. A gate that tests a stale
# artifact is a gate that lies.
#
# CI=1 turns off `reuseExistingServer` in playwright.config.ts. Without it the
# suite silently attaches to whatever is already listening on 4321 - a dev
# server someone left running from another checkout, or anything else at all -
# and reports the result as if it had tested this tree. Proven by pointing a
# bare `python3 -m http.server` at that port: the default run happily executed
# the whole suite against it, while CI=1 stops with "already used".
echo "[6/8] playwright (rebuilding viewer/pkg first)"
if ./viewer/build-wasm.sh >/dev/null 2>&1; then
  :
else
  fail "build-wasm"
fi
PLAYWRIGHT_LOG=$(mktemp)
if (cd viewer && CI=1 npx playwright test 2>&1 | tee "$PLAYWRIGHT_LOG"); then
  pass "playwright"
else
  # Which spec. The stage used to end in a bare "playwright FAILED" while the
  # names scrolled past in a hundred lines of progress output, and a flake seen
  # twice still had no name to chase.
  echo ""
  echo "  failing specs:"
  grep -E "^\s+[0-9]+\) |✘" "$PLAYWRIGHT_LOG" | head -20 || true
  fail "playwright"
fi
rm -f "$PLAYWRIGHT_LOG"
echo ""

# Stage 7: Autorouter benchmark — regression gate + performance benchmark
echo "[7/8] autorouter benchmark"
if cargo test --release -p cypcb-autoroute -- benchmark_regression 2>&1; then
  pass "benchmark-regression"
else
  fail "benchmark-regression"
fi
# DRC ratchets across every fixture. led_blink alone reported 3 violations
# while stm32_breakout sat at 312 and multi_ic at 383 - the gate could not see
# the router's real output.
if cargo test --release -p cypcb-autoroute -- benchmark_all_fixtures_drc --ignored 2>&1; then
  pass "benchmark-all-fixtures-drc"
else
  fail "benchmark-all-fixtures-drc"
fi
# What the router lays has to arrive in the fabrication files. The fixtures are
# already routed in this stage, so the check costs one export each.
if cargo test --release -p cypcb-autoroute -- what_the_router_lays --ignored 2>&1; then
  pass "routed-copper-reaches-the-files"
else
  fail "routed-copper-reaches-the-files"
fi

# Everything measured about the router assumes re-running gives the same
# answer: fourteen dropped instruments, two sweeps and five ratchets are all
# differences between single runs. Rust randomises HashMap iteration order per
# process, so one map walked to order work would make every one of those
# numbers a coin toss - and nothing else here would notice.
if cargo test --release -p cypcb-autoroute -- the_same_board_routed_twice --ignored 2>&1; then
  pass "router-is-repeatable"
else
  fail "router-is-repeatable"
fi

if cargo test --release -p cypcb-autoroute -- benchmark_500 --ignored 2>&1; then
  pass "benchmark-500"
else
  fail "benchmark-500"
fi
echo ""

# Stage 8: Code duplication check
echo "[8/8] jscpd"
if (cd viewer && npx jscpd --exitCode 1) 2>&1; then
  pass "jscpd"
else
  fail "jscpd"
fi
echo ""

echo "=== All stages passed ==="
