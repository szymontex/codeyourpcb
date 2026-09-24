#!/usr/bin/env bash
set -euo pipefail

# Quality Gate — runs all lint, test, and E2E checks in sequence.
# Exits non-zero on first failure. Designed for CI and local pre-merge verification.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# One gate at a time per build directory.
#
# Two gates in one checkout fight over `target/` and over `viewer/pkg`, which
# stage 7 rebuilds. On 2026-09-12 two of them ran at once and the fix applied
# was worse than the contention: `pkill -f quality-gate.sh` matched both, and
# because that pattern kills the shell and not the `cargo test` child it had
# already spawned, one log went quiet immediately while the other kept growing
# for seven minutes from an orphan. Neither log carried an `EXIT=` line,
# because the line is written by the shell after this script returns.
#
# This project had already learned the lesson once, in the scheduled runner:
# its first guard was a `pgrep` on this script's name, replaced because a name
# matches anything whose command line contains it. The answer there was a lock
# on the thing being contended, and it is the answer here.
#
# The key is the build directory rather than the checkout, so the scheduled
# runner - which builds in its own worktree with its own `CARGO_TARGET_DIR` -
# does not serialise against a gate somebody runs by hand in the main tree.
#
# Waiting rather than skipping: a gate that skipped would hand its caller a
# silence indistinguishable from a pass. If the wait runs out, this fails and
# says why, because a gate that ran alongside another proves nothing about
# either tree.
GATE_BUILD_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
mkdir -p "$GATE_BUILD_DIR"

# Who is holding it, and is anybody still watching them.
#
# `exec 9>` opens the descriptor without close-on-exec, so every child inherits
# it, and a `flock` hangs on the open file description rather than on the
# process that took it. Kill the shell and the description lives on in
# `cargo test`: the lock stays held by a process nobody is waiting for, and the
# next gate waits the full thirty minutes below for it. Reproduced in this
# container - parent killed, child alive with `ppid 1`, lock still held,
# released only when the child died.
#
# So the message names the holder instead of naming the file. There is no
# `lsof` and no `fuser` here; walking `/proc/[0-9]*/fd` as this user finds it,
# and **`ppid 1` is what tells an orphan from a running gate** - a gate that is
# genuinely working has its own shell above it. Waiting for a real gate is
# correct and waiting for an orphan is half an hour thrown away, and until
# 2026-09-14 the two looked identical from here.
GATE_LOCK="$GATE_BUILD_DIR/.gate.lock"

# One scan, read before blocking: after the lock is taken the holder is gone.
# `comm` rather than the full command line, so the answer is one token with no
# spaces and the line below stays parsable without quoting.
gate_lock_holder() {
  local target fd pid ppid
  target=$(readlink -f "$GATE_LOCK")
  for fd in /proc/[0-9]*/fd/*; do
    pid=${fd#/proc/}
    pid=${pid%%/*}
    [ "$pid" = "$$" ] && continue
    [ "$(readlink "$fd" 2>/dev/null)" = "$target" ] || continue
    ppid=$(awk '/^PPid:/{print $2}' "/proc/$pid/status" 2>/dev/null)
    [ "$ppid" = "$$" ] && continue
    printf '%s %s %s' "$pid" "${ppid:-0}" "$(cat "/proc/$pid/comm" 2>/dev/null || echo unknown)"
    return 0
  done
  printf 'none 0 none'
}

exec 9>"$GATE_LOCK"
GATE_LOCK_START=$(date +%s)
GATE_LOCK_HOLDER="none 0 none"
if ! flock -n 9; then
  GATE_LOCK_HOLDER=$(gate_lock_holder)
  read -r _hpid _hppid _hcmd <<<"$GATE_LOCK_HOLDER"
  echo "=== Quality Gate ==="
  echo "  waiting: another gate holds $GATE_LOCK"
  if [ "$_hppid" = "1" ]; then
    echo "    pid $_hpid, ppid 1 - ORPHAN, nothing is waiting for it: $_hcmd"
  else
    echo "    pid $_hpid, ppid $_hppid: $_hcmd"
  fi
  echo "  started waiting at $(date +%H:%M:%S); giving up after 30 minutes"
  if ! flock -w 1800 9; then
    echo "  ✗ another gate still holds the lock after 30 minutes - not running"
    echo "    Nothing was checked. Find the other run before reading this as a result."
    exit 1
  fi
  echo "  lock acquired at $(date +%H:%M:%S)"
  echo ""
fi

# One line on every run, including the runs that waited for nothing.
#
# A line printed only when waiting makes its absence unreadable: "nobody
# waited" and "nobody measures this" look the same, and that is what the first
# twenty-four logs in this project's run directory look like. `waited=0` is the
# whole point of it. There is no `holder_orphan` field because `holder_ppid=1`
# already says that, and one fact in two places is one fact that goes wrong in
# one of them. The time is UTC and ISO 8601, so sorting these lines as text
# sorts them by when they happened.
read -r GATE_LOCK_PID GATE_LOCK_PPID GATE_LOCK_CMD <<<"$GATE_LOCK_HOLDER"
printf 'GATE-LOCK waited=%s holder_pid=%s holder_ppid=%s holder_cmd=%s acquired=%s\n' \
  "$(( $(date +%s) - GATE_LOCK_START ))" \
  "$GATE_LOCK_PID" "$GATE_LOCK_PPID" "$GATE_LOCK_CMD" \
  "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# And who else is writing into the build directory, which the lock above
# cannot answer: it serialises the runs that take it, and a command somebody
# types by hand takes nothing. A red that came from a shared target directory
# reads exactly like a red that came from the tree, and on 2026-09-14 one of
# them cost a morning to tell apart. Diagnostic only - it never fails a run.
printf 'GATE-BUILDS %s dir=%s\n' \
  "$("$REPO_ROOT/scripts/who-is-building.sh" "$GATE_BUILD_DIR")" "$GATE_BUILD_DIR"

# Test threads follow the host's load at the start of the run, capped at five.
#
# `CARGO_BUILD_JOBS` limits compilation and nothing else: libtest starts one
# thread per core for every test binary, so the test stage used every core the
# host has. The owner's call on 2026-09-24 was four or five cores for running
# tests, fewer when the host is already busy. The count is read once, here,
# from the one-minute load, and the line below carries the load it came from,
# so a slow run can be read against how busy the host was when it started.
# Five is a ceiling whatever the host reports; two is a floor, so a busy host
# still gets a run that finishes.
GATE_LOAD1=$(cut -d' ' -f1 /proc/loadavg)
GATE_CORES=$(nproc)
GATE_TEST_THREADS=$(awk -v cores="$GATE_CORES" -v load="$GATE_LOAD1" \
  'BEGIN { t = int(cores - load); if (t > 5) t = 5; if (t < 2) t = 2; print t }')
export RUST_TEST_THREADS=$GATE_TEST_THREADS
printf 'GATE-TEST-THREADS threads=%s load1=%s cores=%s\n' \
  "$GATE_TEST_THREADS" "$GATE_LOAD1" "$GATE_CORES"

# How many stages this file declares, how many announced themselves, and how
# many checks reported a pass. The closing line used to be an unconditional
# echo: a stage deleted, commented out or short-circuited left every other
# stage green and the same sentence at the bottom, and nothing in the run said
# how many had been run. A run that skipped a stage is not a pass.
STAGES_DECLARED=18
STAGES_RUN=0
CHECKS_PASSED=0

# The label a reader sees is now counted rather than typed, which is the same
# argument the comment below makes about numbers living in two places.
#
# Each stage closes with how long it took. The log used to carry one number
# for the whole run, so a question about which stage the time goes to was
# answered by reading `finished in` lines and guessing at what lay between
# them. A failed stage closes too: the time to the first red is a number
# somebody asks about.
stage() {
  stage_done
  STAGES_RUN=$((STAGES_RUN + 1))
  STAGE_NAME=$1
  STAGE_START=$(date +%s)
  echo "[$STAGES_RUN/$STAGES_DECLARED] $1"
}

stage_done() {
  [ -n "${STAGE_NAME:-}" ] || return 0
  printf 'STAGE-TIME %s/%s %ss %s\n' \
    "$STAGES_RUN" "$STAGES_DECLARED" "$(( $(date +%s) - STAGE_START ))" "$STAGE_NAME"
  STAGE_NAME=
}

pass() { CHECKS_PASSED=$((CHECKS_PASSED + 1)); echo "  ✓ $1"; }
fail() { echo "  ✗ $1"; stage_done; exit 1; }

echo "=== Quality Gate ==="
echo ""

# The stage headings below carry no number. They used to, and the numbers had
# drifted: two comments both said "Stage 10", nothing said 9 or 14, and the
# `[n/N]` labels a reader actually sees were counting something else. One fact
# in two places is one fact that goes wrong in one of them, and the label is
# the place a reader looks.
# Rust formatting
stage "cargo fmt --check"
if cargo fmt --check 2>&1; then
  pass "cargo-fmt"
else
  fail "cargo-fmt"
fi
echo ""

# Clippy (strict, whole workspace)
#
# `cypcb-desktop` used to be excluded here and in stage 3 because it needs
# system GTK and WebKit. Nothing installed those, so the crate went unbuilt for
# long enough to rot: nine compile errors from the Tauri v1 to v2 move, plus an
# icon the macro refused, all found the first time anybody ran it. The
# dependencies are in `scripts/setup-dev.sh` now, so the exclusion has nothing
# left to protect and a crate nobody compiles is a crate nobody maintains.
stage "cargo clippy"
# The second reader is behind a feature, so the plain run does not lint it
# either - the same gap the test stage below had.
if cargo clippy --workspace --all-targets -- -D warnings 2>&1 \
  && cargo clippy -p cypcb-parser --features tree-sitter-parser --all-targets -- -D warnings 2>&1; then
  pass "cargo-clippy"
else
  fail "cargo-clippy"
fi
echo ""

stage "cargo check --all-features"
# Every feature this workspace declares, compiled. Eleven features exist and
# **five are switched on by nothing** - not a stage, not a `default`, not a
# dependent crate: `cypcb-drc/parallel`, `cypcb-library/jlcpcb` and the three
# on `cypcb-platform`. `jlcpcb` is not dead wood - it gates five places that
# really call `reqwest` - so this is production code that nothing compiled
# until 2026-09-14. `parallel` is the other kind: it buys `rayon` and
# `grep -rn 'feature = "parallel"' crates/cypcb-drc/src` answers nothing.
#
# `--workspace --all-features` cannot be run whole: `cypcb-platform` carries a
# deliberate `compile_error!` refusing `desktop` and `web` at once, so that
# crate is excluded here and checked once per exclusive feature instead. Warm,
# the three commands cost about fourteen seconds together.
if cargo check --workspace --all-features --exclude cypcb-platform 2>&1 \
  && cargo check -p cypcb-platform --features desktop 2>&1 \
  && cargo check -p cypcb-platform --features web 2>&1; then
  pass "all-features-compile"
else
  fail "all-features-compile"
fi
echo ""

# Rust tests
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
  # Against the tree rather than against HEAD. The first version of this asked
  # `git diff HEAD -- grammar/src`, which is a different question: it fails for
  # any *uncommitted* grammar change, including the one being tested, so a
  # commit that touches the language could never pass its own gate. What is
  # actually being asked is whether the parser on disk is the one this grammar
  # produces, so the answer is a before-and-after of the regeneration itself.
  BEFORE=$(find crates/cypcb-parser/grammar/src -type f -exec md5sum {} + | sort)
  if ! (cd crates/cypcb-parser/grammar && ./node_modules/.bin/tree-sitter generate) >/dev/null 2>&1; then
    fail "tree-sitter generate"
  fi
  AFTER=$(find crates/cypcb-parser/grammar/src -type f -exec md5sum {} + | sort)
  if [ "$BEFORE" != "$AFTER" ]; then
    echo ""
    echo "  the parser in the tree is not the one this grammar generates;"
    echo "  regenerating changed:"
    diff <(echo "$BEFORE") <(echo "$AFTER") | grep '^[<>]' | sed 's/^/    /' | head -10
    echo "  the files are correct now - commit crates/cypcb-parser/grammar/src."
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

stage "cargo test"
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
#
# Where `cargo nextest` is installed it runs these, on the same thread count
# as everything else. `cargo test` runs one test binary after another, and the
# five slowest binaries alone were about fifty-four of this stage's hundred
# seconds of execution, so extra threads inside one binary bought nothing: the
# stage took 117 s on every core and 118-120 s on five. Nextest takes tests
# from every binary into one pool of that size. Measured on 2026-09-24, same
# tree, five threads, warm: 119-122 s with `cargo test`, 78-79 s with nextest,
# the same 2860 tests passing and 81 skipped. Nextest does not run doctests,
# so `cargo test --doc` runs them after it, and the line below says which
# runner a log came from.
if cargo nextest --version >/dev/null 2>&1; then
  GATE_TEST_RUNNER=nextest
else
  GATE_TEST_RUNNER=cargo-test
fi
echo "GATE-TEST-RUNNER $GATE_TEST_RUNNER threads=$GATE_TEST_THREADS"
run_tests() {
  if [ "$GATE_TEST_RUNNER" = nextest ]; then
    cargo nextest run --test-threads "$GATE_TEST_THREADS" "$@" 2>&1 \
      && cargo test --doc "$@" 2>&1
  else
    cargo test "$@" 2>&1
  fi
}
if run_tests --workspace \
  && run_tests -p cypcb-parser --features tree-sitter-parser \
  && run_tests -p cypcb-render --no-default-features --features wasm; then
  pass "cargo-test"
else
  fail "cargo-test"
fi
echo ""

# ESLint
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
stage "tsc --noEmit"
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

stage "eslint"
if (cd viewer && npx eslint src/ e2e/ *.ts) 2>&1; then
  pass "eslint"
else
  fail "eslint"
fi
echo ""

# Vitest
stage "vitest"
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

# Playwright E2E
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
#
# The page's WebSocket has a port of its own, and it was the one thing the
# e2e port did not isolate: the client dialled 4322 whatever served it, so a
# `npm start` running from another checkout answered every spec and pushed its
# board into the page. `CYPCB_E2E_WS_PORT` overrides, the default is 4328, and
# nothing listens there; `the-page-dials-its-own-websocket.spec.ts` checks it.
stage "playwright (rebuilding viewer/pkg first)"
# The module is rebuilt, and then asked whether the committed one is the same.
# The rebuild makes the browser suite honest about the working tree; the
# question afterwards is about what a clean clone carries, and on 2026-08-27
# those were different - a module built against an older dependency set, 5,241
# bytes nobody had looked at.
#
# Both halves of that question live in the script: which sources can reach the
# module, and whether rebuilding them changes what is committed. Asked here
# until 2026-08-31, it was asked of the file history alone and of every crate
# in the workspace, so it sent the nightly gate red over two crates the module
# never links and over a doc comment that changes no byte of it.
if ./viewer/build-wasm.sh >/dev/null 2>&1; then
  :
else
  fail "build-wasm"
fi
if ! ./scripts/wasm-pkg-stale.sh; then
  echo ""
  fail "stale viewer/pkg"
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

# Autorouter benchmark — regression gate + performance benchmark
stage "autorouter benchmark"
# In the test profile, not `--release`. The routing crates are optimized
# there too (see `[profile.test.package.*]` in Cargo.toml), and a release
# build of the same crates cost 2m 41s after every router change in the
# nightly run of 2026-09-23 for code no other stage uses. The six benchmark
# boards route to the same hash of every segment and via in both profiles.
if cargo test -p cypcb-autoroute -- benchmark_regression 2>&1; then
  pass "benchmark-regression"
else
  fail "benchmark-regression"
fi
# Every stage below names ignored cases with a filter, and a filter that
# matches nothing is a stage that passes: `cargo test` exits 0 having run no
# test at all. That is not hypothetical - a stage added on 2026-09-13 passed
# two names in one invocation, which libtest reads as one filter, and it ran
# zero tests across four binaries and reported a pass. So the count is read
# back: at least one case has to say it passed, and no binary may say FAILED.
ignored_cases() {
  local out
  out=$(cargo test -p cypcb-autoroute "$@" --ignored 2>&1)
  echo "$out"
  echo "$out" | grep -q "test result: FAILED" && return 1
  echo "$out" | grep -qE "test result: ok\. [1-9][0-9]* passed"
}

# DRC ratchets across every fixture. led_blink alone reported 3 violations
# while stm32_breakout sat at 312 and multi_ic at 383 - the gate could not see
# the router's real output.
if ignored_cases -- benchmark_all_fixtures_drc; then
  pass "benchmark-all-fixtures-drc"
else
  fail "benchmark-all-fixtures-drc"
fi
# What the router lays has to arrive in the fabrication files. The fixtures are
# already routed in this stage, so the check costs one export each.
if ignored_cases -- what_the_router_lays; then
  pass "routed-copper-reaches-the-files"
else
  fail "routed-copper-reaches-the-files"
fi

# Everything measured about the router assumes re-running gives the same
# answer: fourteen dropped instruments, two sweeps and five ratchets are all
# differences between single runs. Rust randomises HashMap iteration order per
# process, so one map walked to order work would make every one of those
# numbers a coin toss - and nothing else here would notice.
if ignored_cases -- the_same_board_routed_twice; then
  pass "router-is-repeatable"
else
  fail "router-is-repeatable"
fi

if ignored_cases -- benchmark_500; then
  pass "benchmark-500"
else
  fail "benchmark-500"
fi

# The two ratchets under R-08's anatomy, which ran in no stage until
# 2026-09-13. `cargo test --workspace` skips an ignored test and this file
# named four of them by hand; these two were not among the four, so
# `netted_total == 775`, `netted_on_grid == 3`, `netted_turned == 0`,
# `entries_narrow == entries_total` and `sharp_on_grid == 0` were assertions
# nothing executed. Their own comment calls them "a ratchet, like
# ENTRY_CENSUS" - and ENTRY_CENSUS runs, because `benchmark_all_fixtures_drc`
# is named above. **A ratchet nobody runs is a comment**, and the two canon
# sections these hold carried the oldest `Verified:` dates in the file for
# exactly that reason: nothing could move them, so nobody had cause to read
# them. Both together cost under four seconds in release.
if ignored_cases --test sharp_entry_anatomy --; then
  pass "the-anatomy-ratchets"
else
  fail "the-anatomy-ratchets"
fi

# The two ignored cases that assert something and were reached by no filter.
# The census behind this: 50 tests in this repository carry `#[ignore]` and
# **40 of them contain no assertion at all** - they print a run rather than
# judge it, and adding one to a gate buys only that it still compiles. Six more
# are named above. Of the four that assert and were not run, these two cost
# 2.07s and 4.94s measured on 2026-09-13; the other two cost 98s and 100s
# between them for three assertions, which is a question about the price of a
# stage rather than about technique and is left for the owner.
#
# A third joined them on 2026-09-14, found by re-reading the canon rather than
# by the census: `how_much_does_the_answer_depend_on_the_direction` carries two
# assertions, costs 0.68s, and had run in no stage since it was written on
# 2026-09-11. The canon's list of what nothing measures still named its subject
# as unmeasured - a test nothing runs and a document that does not know it
# exists are the same omission seen from two sides.
#
# A fourth joined on 2026-09-24: the smoother held to never cutting a net with
# `stop_at_own_copper` on, the setting under which it cut `mains-sequencer`.
# `benchmark_all_fixtures_drc` holds the same line with the setting off on the
# boards it routes anyway; this one routes the six fixtures and that board
# again, at 3.53s measured the day it was added.
#
# Each is run on its own, because libtest takes one filter and passing two
# names in one invocation matches nothing.
if ignored_cases --test routed_copper_reaches_the_files \
      -- --exact which_layers_the_router_joins_with_a_via \
  && ignored_cases --test benchmark_validation -- --exact benchmark_full_matrix \
  && ignored_cases --test benchmark_validation \
      -- --exact smoothing_never_adds_a_net_piece_on_its_own_copper \
  && ignored_cases --test the_same_pair_routed_from_either_end \
      -- --exact how_much_does_the_answer_depend_on_the_direction; then
  pass "the-cheap-ignored-assertions"
else
  fail "the-cheap-ignored-assertions"
fi
echo ""

# Code duplication check
stage "jscpd"
if (cd viewer && npx jscpd --exitCode 1) 2>&1; then
  pass "jscpd"
else
  fail "jscpd"
fi
echo ""

# engine methods the browser never reaches
#
# `scripts/unused-engine-api.sh` was written as a report and nothing ran it,
# so the thing it was written to find - a variant search whose panel had been
# deleted - was found again by hand five weeks later. It keeps its list and
# gains a number: the count of unreached methods has to be the one the script
# records, so neither a new dead wrapper nor a deletion can pass unremarked.
stage "engine API reach"
if ./scripts/unused-engine-api.sh 2>&1; then
  pass "unused-engine-api"
else
  fail "unused-engine-api"
fi
echo ""

# the desktop application starts and draws something
#
# `scripts/desktop-smoke.sh` was written on 2026-08-12 and nothing ran it, so
# the only crate that reaches a person's machine as an application was the one
# nothing here started. It builds what it photographs first: the smoke test
# refuses a bundle older than `viewer/src`, and the tree it was wired into had
# a `viewer/dist` a week behind. The bundle dials its WebSocket on the smoke's
# own port, not on a dev server's, and the script builds the binary itself so
# that the window shows this bundle and not the page `devUrl` points at. The
# desktop app dials no socket unless built with CYPCB_DESKTOP_DEV_SOCKET=1, so
# the smoke's bundle is built with it: that dial is how it proves its origin.
stage "desktop smoke"
if (cd viewer && CYPCB_DESKTOP_DEV_SOCKET=1 CYPCB_WS_PORT="${CYPCB_SMOKE_WS_PORT:-4329}" npm run build) >/dev/null 2>&1 \
    && ./scripts/desktop-smoke.sh; then
  pass "desktop-smoke"
else
  fail "desktop-smoke"
fi
echo ""

# figures a comment states and nothing reads back
#
# `scripts/claims-in-comments.sh` was written on 2026-08-29 after a comment
# about the dev server's origin check was believed for as long as the check
# existed. It counts the figures in comments that no test names - 35 of 190 -
# and nothing ran it either, so the census was a number nobody had looked at
# since the day it was taken. The list stays a person's call; the count is
# held here, the way the engine API's is.
stage "claims in comments"
if ./scripts/claims-in-comments.sh; then
  pass "claims-in-comments"
else
  fail "claims-in-comments"
fi
echo ""

# exported values in the viewer that nothing else names
#
# `walkaround.ts` was 680 lines nothing imported and it survived a year of
# green runs. Whole modules are guarded by a vitest case; this is the finer
# half - a module imported for one thing can still export others nobody wants.
# The types it lists stay a diagnostic, because an exported interface beside
# its function is ordinary style. The values are the gate, and the viewer is
# already at zero.
stage "unused exports"
if ./scripts/unused-exports.sh --values-only; then
  pass "unused-exports"
else
  fail "unused-exports"
fi
echo ""

# the scheduled runner's own decisions
#
# `scripts/scheduled-gate.sh` pushes to a shared branch at 04:30 with nobody
# watching, and it was proved by one hand run on one day - the day every
# branch happened to line up, so four of its five publish outcomes had never
# happened at all. This stage makes them happen, against throwaway
# repositories in a temporary directory.
stage "scheduled-gate selftest"
if ./scripts/scheduled-gate-selftest.sh 2>&1; then
  pass "scheduled-gate-selftest"
else
  fail "scheduled-gate-selftest"
fi
# The neighbour scan printed at the top of every run. A diagnostic that has
# stopped working reports the same zero as a quiet machine, which is the one
# failure a reader of `GATE-BUILDS sharing=0` cannot see.
if ./scripts/who-is-building.sh --selftest 2>&1; then
  pass "who-is-building"
else
  fail "who-is-building"
fi
# The other diagnostic that answers zero when it has stopped working. This one
# answers the question a census asks - who tests this name - and the narrow
# version of that question has now been asked wrongly four times here.
if ./scripts/who-tests-this.sh --selftest 2>&1; then
  pass "who-tests-this"
else
  fail "who-tests-this"
fi
echo ""

# the repository does not name the machine it is built on
#
# This repository is public and the machine is not. On 2026-09-11 a grep found
# a host name, a container home, a log directory and a checkout path across
# thirty-two tracked files written over four months - none of it about circuit
# boards, all of it pasted out of a terminal by somebody who could see it and
# forgot that a reader cannot. A patch would have let the next paste back in.
stage "no private paths"
if ./scripts/no-private-paths.sh; then
  pass "no-private-paths"
else
  fail "no-private-paths"
fi
echo ""

# no number in a file this project writes comes from nowhere
#
# Three literals in the KiCad writer's zone node turned out to be somebody's
# safety margin rather than the fab table - a pour standing twice as far off
# every pad as the house asks, a fill floor wider than any fab preset's trace
# width, a relief at nearly double what the Gerbers carried for the same board.
# Each was found by hand, months apart, and the third only because the first
# two made somebody look.
stage "no invented numbers"
if ./scripts/no-invented-numbers.sh; then
  pass "no-invented-numbers"
else
  fail "no-invented-numbers"
fi
echo ""

# one door out of nanometres
#
# A Gerber coordinate is `X1000000Y500000` and an Excellon one is `X0150`: the
# decimal point is implied by a format the header declares, so a dimension in
# those files carries no dot and stage 16 cannot see it. This check asks about
# the act instead - turning a length into text happens in `coords.rs` and
# nowhere else - which is the only shape that can fail there.
stage "one door out of nanometres"
if ./scripts/one-door-coordinates.sh; then
  pass "one-door-coordinates"
else
  fail "one-door-coordinates"
fi
echo ""

# The same scan again, because the first one answers about an instant and a run
# takes half an hour. A neighbour that starts after the top of the run is
# invisible to the line printed there: on 2026-09-16 a run whose first sample
# read `sharing=0` died in a doctest on *two different versions of crate
# `cypcb_drc`*, which is what a target directory looks like after two builds
# have written into it. Diagnostic only, like the first - it never fails a run,
# and its value is that a red carries evidence about the window rather than
# about one moment of it.
stage_done

printf 'GATE-BUILDS-AT-END %s dir=%s\n' \
  "$("$REPO_ROOT/scripts/who-is-building.sh" "$GATE_BUILD_DIR")" "$GATE_BUILD_DIR"

if [ "$STAGES_RUN" -ne "$STAGES_DECLARED" ]; then
  echo "=== $STAGES_RUN of $STAGES_DECLARED stages ran ==="
  echo "  A stage that never announced itself was skipped, and every other"
  echo "  stage passing says nothing about the one that did not run. Move"
  echo "  STAGES_DECLARED in the commit that moves the stages."
  exit 1
fi

echo "=== All stages passed: $STAGES_RUN of $STAGES_DECLARED stages, $CHECKS_PASSED checks ==="
