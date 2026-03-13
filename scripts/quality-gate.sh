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
echo "[1/6] cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo-fmt"
else
  fail "cargo-fmt"
fi
echo ""

# Stage 2: Clippy (strict, excluding desktop crates)
echo "[2/6] cargo clippy"
if cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings 2>&1; then
  pass "cargo-clippy"
else
  fail "cargo-clippy"
fi
echo ""

# Stage 3: Rust tests
echo "[3/6] cargo test"
if cargo test --workspace --exclude cypcb-cli --exclude cypcb-desktop 2>&1; then
  pass "cargo-test"
else
  fail "cargo-test"
fi
echo ""

# Stage 4: ESLint
echo "[4/6] eslint"
if (cd viewer && npx eslint src/) 2>&1; then
  pass "eslint"
else
  fail "eslint"
fi
echo ""

# Stage 5: Vitest
echo "[5/6] vitest"
if (cd viewer && npx vitest run) 2>&1; then
  pass "vitest"
else
  fail "vitest"
fi
echo ""

# Stage 6: Playwright E2E
echo "[6/6] playwright"
if (cd viewer && npx playwright test) 2>&1; then
  pass "playwright"
else
  fail "playwright"
fi
echo ""

echo "=== All stages passed ==="
