#!/usr/bin/env bash
# Who tests this name?
#
# This project keeps two thirds of its tests beside the code they cover: 127 of
# the 204 files under crates/*/src carry a `#[cfg(test)]` module. A census that
# searches crates/*/tests alone therefore reports "nothing covers this" for a
# name with four assertions under it, and this repository has answered that way
# four times - three in checks and once in its own backlog, where it nearly
# bought a session of writing tests that already existed.
#
# Usage:
#   scripts/who-tests-this.sh <name>
#   scripts/who-tests-this.sh --selftest
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Files under crates/*/src that both name the symbol and carry a test module.
beside_the_code() {
    local name="$1"
    # `|| true` on both halves: grep answers 1 when it matches nothing, and
    # under `set -o pipefail` that ends the run - so a name covered on one side
    # and absent on the other printed nothing at all, which is the answer this
    # script exists to give.
    { grep -rl --include=*.rs -- "$name" "$REPO_ROOT"/crates/*/src 2>/dev/null || true; } \
        | { xargs -r grep -l '#\[cfg(test)\]' || true; } \
        | sed "s|^$REPO_ROOT/||" \
        | sort
}

# Integration test files that name the symbol.
in_the_tests_directory() {
    local name="$1"
    { grep -rl --include=*.rs -- "$name" "$REPO_ROOT"/crates/*/tests 2>/dev/null || true; } \
        | sed "s|^$REPO_ROOT/||" \
        | sort
}

report() {
    local name="$1"
    local beside integration
    beside="$(beside_the_code "$name")"
    integration="$(in_the_tests_directory "$name")"
    local n_beside n_integration
    n_beside="$(printf '%s' "$beside" | grep -c . || true)"
    n_integration="$(printf '%s' "$integration" | grep -c . || true)"
    printf 'name=%s beside-the-code=%s in-tests-dir=%s\n' \
        "$name" "$n_beside" "$n_integration"
    # Written as `if`, not `&&`: under `set -e` a false test at the end of a
    # function ends the script, and this one printed nothing at all for a name
    # with no integration test - which is the very case it exists to report.
    if [ -n "$beside" ]; then
        printf '  beside: %s\n' $beside
    fi
    if [ -n "$integration" ]; then
        printf '  tests:  %s\n' $integration
    fi
    [ "$((n_beside + n_integration))" -gt 0 ]
}

selftest() {
    local failed=0

    # A name tested only beside its code: the narrow search answers nothing.
    #
    # Captured into a variable rather than piped into grep. `report` exits 1
    # when a name is covered nowhere, and under `set -o pipefail` that status
    # survives the pipe and makes the `if` false whatever grep found - so this
    # assertion passed while reading a scan that had been narrowed to the wrong
    # directory, which is the one failure it exists to catch.
    local answer
    answer="$(report width_in_glyphs || true)"
    if printf '%s' "$answer" | grep -q 'beside-the-code=0'; then
        echo "FAIL: width_in_glyphs is tested in its own file and this says otherwise"
        failed=1
    fi
    if [ -n "$(in_the_tests_directory width_in_glyphs)" ]; then
        echo "FAIL: width_in_glyphs is not in crates/*/tests, so it is the wrong example"
        failed=1
    fi

    # A name that is nowhere: the answer has to be nothing on both halves.
    if report no_such_symbol_anywhere_here >/dev/null 2>&1; then
        echo "FAIL: an invented name was reported as covered"
        failed=1
    fi

    # A name tested from crates/*/tests, so the other half is not dead either.
    if [ -z "$(in_the_tests_directory pad_to_grid_node)" ]; then
        echo "FAIL: pad_to_grid_node is named by an integration test and this missed it"
        failed=1
    fi

    if [ "$failed" -eq 0 ]; then
        echo "who-tests-this: 4 assertions, all passing"
    fi
    return "$failed"
}

case "${1:-}" in
    --selftest) selftest ;;
    "") echo "usage: scripts/who-tests-this.sh <name> | --selftest" >&2; exit 2 ;;
    *) report "$1" ;;
esac
