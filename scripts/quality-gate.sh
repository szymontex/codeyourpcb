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
echo "[1/9] cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo-fmt"
else
  fail "cargo-fmt"
fi
echo ""

# Stage 2: Clippy (strict, whole workspace)
#
# `cypcb-desktop` used to be excluded here and in stage 3 because it needs
# system GTK and WebKit. Nothing installed those, so the crate went unbuilt for
# long enough to rot: nine compile errors from the Tauri v1 to v2 move, plus an
# icon the macro refused, all found the first time anybody ran it. The
# dependencies are in `scripts/setup-dev.sh` now, so the exclusion has nothing
# left to protect and a crate nobody compiles is a crate nobody maintains.
echo "[2/9] cargo clippy"
# The second reader is behind a feature, so the plain run does not lint it
# either - the same gap the test stage below had.
if cargo clippy --workspace --all-targets -- -D warnings 2>&1 \
  && cargo clippy -p cypcb-parser --features tree-sitter-parser --all-targets -- -D warnings 2>&1; then
  pass "cargo-clippy"
else
  fail "cargo-clippy"
fi
echo ""

# Stage 3: Rust tests
# The generated Tree-sitter parser, asked the same question as viewer/pkg and
# for the same reason: `crates/cypcb-parser/grammar/src/parser.c` is committed
# and `build.rs` compiles whatever is there - it does not regenerate, it panics
# if the file is missing. A `grammar.js` committed without the parser it
# generates means the language parses by the old grammar while its source says
# otherwise, and nothing in this gate would notice.
#
# Commit order again rather than bytes: `tree-sitter-cli` is a caret range in
# grammar/package.json and its lockfile is not in git, so two machines can
# generate different C from one grammar.
GRAMMAR_COMMIT=$(git log -1 --format=%H -- crates/cypcb-parser/grammar/grammar.js)
PARSER_COMMIT=$(git log -1 --format=%H -- crates/cypcb-parser/grammar/src)
if [ -n "$GRAMMAR_COMMIT" ] && [ -n "$PARSER_COMMIT" ] \
  && ! git merge-base --is-ancestor "$GRAMMAR_COMMIT" "$PARSER_COMMIT"; then
  echo ""
  echo "  the committed parser predates the grammar it comes from:"
  echo "    grammar.js last changed by $(git log -1 --format='%h %s' "$GRAMMAR_COMMIT")"
  echo "    grammar/src last changed by $(git log -1 --format='%h %s' "$PARSER_COMMIT")"
  echo "  regenerate with (cd crates/cypcb-parser/grammar && npx tree-sitter generate) and commit grammar/src."
  fail "stale tree-sitter parser"
fi
# And whether the committed parser is the one this grammar makes. Commit order
# catches a forgotten regeneration and says nothing about a `parser.c` edited by
# hand or generated from a grammar that was amended afterwards. This comparison
# can be made now that `tree-sitter-cli` is pinned to an exact version in
# grammar/package.json with `grammar/package-lock.json` in git: the generator is
# the same one everywhere, so a difference is the repository's and not the
# machine's. The local binary is used rather than `npx`, which would happily
# fetch a different one.
TREE_SITTER_BIN=crates/cypcb-parser/grammar/node_modules/.bin/tree-sitter
if [ -x "$TREE_SITTER_BIN" ]; then
  if ! (cd crates/cypcb-parser/grammar && ./node_modules/.bin/tree-sitter generate) >/dev/null 2>&1; then
    fail "tree-sitter generate"
  fi
  REGENERATED=$(git diff --name-only HEAD -- crates/cypcb-parser/grammar/src)
  if [ -n "$REGENERATED" ]; then
    echo ""
    echo "  the committed parser is not the one this grammar generates:"
    echo "$REGENERATED" | sed 's/^/    /'
    echo "  commit crates/cypcb-parser/grammar/src."
    fail "parser does not match its grammar"
  fi
else
  echo "  (parser not regenerated: no CLI at $TREE_SITTER_BIN - run npm ci in crates/cypcb-parser/grammar)"
fi
UNTRACKED_PARSER=$(git ls-files --others --exclude-standard crates/cypcb-parser/grammar/src)
if [ -n "$UNTRACKED_PARSER" ]; then
  echo ""
  echo "  the generator writes files under grammar/src that git does not track:"
  echo "$UNTRACKED_PARSER" | sed 's/^/    /'
  echo "  commit them, or stop writing them."
  fail "untracked tree-sitter output"
fi

echo "[3/9] cargo test"
# The Rust reader is what `parse` is now. The tests that check it against the
# tree-sitter parser need that parser as well, which the plain run does not
# build - named explicitly, because a test nobody runs is not a test.
#
# The whole crate under the feature, not named targets. This used to name
# `--test differential` and `--test error_parity`, which left the crate's own
# `--lib` target uncompiled: 98 unit tests over the tree-sitter reader had not
# been built since `pad <name>` shipped and turned `pad.number` into a
# `String`, and nothing said so. Naming targets one at a time is how a target
# goes missing, so the browser build below is run the same way - it named one
# test out of twelve targets.
#
# Every other non-default feature in the workspace was checked when this line
# was written and all of them compile: `cypcb-drc/parallel`,
# `cypcb-library/jlcpcb`, and `cypcb-platform`'s `desktop`, `web` and
# `native-dialogs`. They are not run here because nothing in them is a second
# implementation of something the default build already has.
if cargo test --workspace 2>&1 \
  && cargo test -p cypcb-parser --features tree-sitter-parser 2>&1 \
  && cargo test -p cypcb-render --no-default-features --features wasm 2>&1; then
  pass "cargo-test"
else
  fail "cargo-test"
fi
echo ""

# Stage 4: ESLint
# Nothing here type-checked the viewer until 2026-08-27. `npm run build` is
# `build:wasm && tsc && vite build`, and the gate ran neither: Vite strips types
# rather than checking them and Playwright starts its server the same way, so a
# viewer that cannot compile passed every stage. Measured before the stage was
# written - `npx tsc --noEmit` in `viewer` -> no output, exit 0, over 136
# project files - so this starts green rather than starting with a hundred
# errors nobody will fix.
#
# tsconfig.json includes `src`, `e2e` and the root `*.ts`, which is what makes
# this worth a stage: the specs and the dev server are code too.
echo "[4/9] tsc --noEmit"
TSC_LOG=$(mktemp)
if (cd viewer && npx tsc --noEmit 2>&1 | tee "$TSC_LOG"); then
  pass "tsc"
else
  echo ""
  echo "  first errors:"
  grep -E "error TS" "$TSC_LOG" | head -10 || true
  rm -f "$TSC_LOG"
  fail "tsc"
fi
rm -f "$TSC_LOG"
echo ""

echo "[5/9] eslint"
if (cd viewer && npx eslint src/ e2e/ *.ts) 2>&1; then
  pass "eslint"
else
  fail "eslint"
fi
echo ""

# Stage 5: Vitest
echo "[6/9] vitest"
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
# suite silently attaches to whatever is already listening on the e2e port - a
# dev server someone left running from another checkout, or anything else at
# all - and reports the result as if it had tested this tree. Proven by
# pointing a bare `python3 -m http.server` at that port: the default run
# happily executed the whole suite against it, while CI=1 stops with "already
# used".
#
# That port is no longer 4321. It was, and 4321 is Astro's default, so a gate
# run failed here because another repository's dev server in this container
# held it. `CYPCB_E2E_PORT` overrides, and the default is 4327.
echo "[7/9] playwright (rebuilding viewer/pkg first)"
# Before rebuilding, ask whether the *committed* bundle is the one this source
# builds. The rebuild below makes the suite honest about the working tree and
# says nothing about what a clean clone carries, and on 2026-08-27 those were
# different: `viewer/pkg/cypcb_render_bg.wasm` was last committed by 19b88db
# and `d27ad56` added two lines to Cargo.lock afterwards, so every clone of
# this branch served an engine built against an older dependency set - 5,241
# bytes of difference nobody had looked at.
#
# The question is asked of the history rather than of the bytes on purpose.
# `rust-toolchain.toml` pins the channel and not a version, and binaryen is not
# pinned at all, so two machines can build the same source into different
# bytes; a comparison would then fail for the wrong reason. What is not
# toolchain-dependent is the order of commits: the newest commit that changed a
# build input must not be newer than the commit that last changed viewer/pkg.
INPUTS_COMMIT=$(git log -1 --format=%H -- 'crates/*/src' Cargo.lock viewer/build-wasm.sh)
PKG_COMMIT=$(git log -1 --format=%H -- viewer/pkg)
if [ -n "$INPUTS_COMMIT" ] && [ -n "$PKG_COMMIT" ] \
  && ! git merge-base --is-ancestor "$INPUTS_COMMIT" "$PKG_COMMIT"; then
  echo ""
  echo "  the committed viewer/pkg predates a build input:"
  echo "    inputs last changed by $(git log -1 --format='%h %s' "$INPUTS_COMMIT")"
  echo "    viewer/pkg last changed by $(git log -1 --format='%h %s' "$PKG_COMMIT")"
  echo "  rebuild with ./viewer/build-wasm.sh and commit viewer/pkg."
  fail "stale viewer/pkg"
fi
if ./viewer/build-wasm.sh >/dev/null 2>&1; then
  :
else
  fail "build-wasm"
fi
# The bindings beside the module, compared byte for byte against what the
# rebuild just wrote. The history check above cannot see this one: it asks when
# `viewer/pkg` last changed, and a commit that refreshes the module alone -
# `52ec725` did exactly that, `git add -f` on the wasm and nothing else - makes
# the whole directory look current while `cypcb_render.js` stays a generation
# behind. The glue is not decoration: it names every method the Rust side
# exports, `auto_route_with_params` among 29 of them, and calls into the module
# by symbol. Bindings from one API against a module built from another fail in
# the browser and nowhere else.
#
# These files can be compared where the `.wasm` cannot: wasm-bindgen writes
# them and its version is pinned in `Cargo.lock`, while the module's bytes come
# out of whichever rustc the channel resolves to and whichever binaryen is
# installed.
GENERATED_BINDINGS=$(git ls-files viewer/pkg | grep -v '\.wasm$' || true)
if [ -n "$GENERATED_BINDINGS" ]; then
  # shellcheck disable=SC2086
  DRIFTED=$(git diff --name-only HEAD -- $GENERATED_BINDINGS)
  if [ -n "$DRIFTED" ]; then
    echo ""
    echo "  the committed bindings are not the ones this source generates:"
    echo "$DRIFTED" | sed 's/^/    /'
    echo "  commit viewer/pkg after ./viewer/build-wasm.sh."
    fail "stale viewer/pkg bindings"
  fi
fi
# The pair checked against each other rather than against the toolchain. The
# module's bytes cannot be compared - `rust-toolchain.toml` pins `stable` and
# binaryen comes from the operating system - but what the bindings ask of the
# module can be: every `wasm.<symbol>` the glue calls has to be a symbol the
# module exports. That holds whoever compiled it, and it is the failure a
# mismatched pair actually produces - a call into a name that is not there.
if command -v wasm-dis >/dev/null 2>&1; then
  MODULE_EXPORTS=$(wasm-dis viewer/pkg/cypcb_render_bg.wasm 2>/dev/null \
    | grep -oE '\(export "[^"]+"' | sed 's/(export "//; s/"$//' | sort -u)
  GLUE_CALLS=$(grep -oE 'wasm\.[a-zA-Z0-9_]+' viewer/pkg/cypcb_render.js | sed 's/^wasm\.//' | sort -u)
  ABSENT=$(comm -23 <(echo "$GLUE_CALLS") <(echo "$MODULE_EXPORTS"))
  if [ -n "$ABSENT" ]; then
    echo ""
    echo "  the bindings call symbols the module does not export:"
    echo "$ABSENT" | sed 's/^/    /'
    echo "  rebuild with ./viewer/build-wasm.sh and commit viewer/pkg."
    fail "bindings and module disagree"
  fi
  UNCALLED=$(comm -13 <(echo "$GLUE_CALLS") <(echo "$MODULE_EXPORTS"))
  if [ -n "$UNCALLED" ]; then
    echo "  (the module exports $(echo "$UNCALLED" | wc -l) symbol(s) the bindings never call)"
  fi
else
  echo "  (bindings not checked against the module: no wasm-dis - install binaryen)"
fi

# And whether the rebuild wrote anything nobody tracks. `viewer/.gitignore`
# carried `pkg/` from the wasm-pack era while six files inside it were tracked,
# so a new artifact appearing there was invisible in `git status` and shipped to
# nobody. The rule is gone; this is what replaces it - a generated file that is
# not in git is either an artifact this repository forgot to ship or one it
# should not be writing.
UNTRACKED_PKG=$(git ls-files --others --exclude-standard viewer/pkg)
if [ -n "$UNTRACKED_PKG" ]; then
  echo ""
  echo "  the build wrote files under viewer/pkg that git does not track:"
  echo "$UNTRACKED_PKG" | sed 's/^/    /'
  echo "  commit them, or stop writing them."
  fail "untracked viewer/pkg output"
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
echo "[8/9] autorouter benchmark"
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
echo "[9/9] jscpd"
if (cd viewer && npx jscpd --exitCode 1) 2>&1; then
  pass "jscpd"
else
  fail "jscpd"
fi
echo ""

echo "=== All stages passed ==="
