#!/usr/bin/env bash
#
# Which figures in comments anything reads back.
#
# This project keeps meeting the same defect from two sides. A comment said the
# dev server had no origin check for as long as the check existed, and a
# heartbeat believed it. A tracker line said the rule registry held 24 rules
# when it held 31. Both were sentences nobody ran.
#
# A comment that states a **figure** is a claim: `0.127mm`, `41%`, `24 rules`.
# Some of those numbers are also asserted somewhere - a test names the same
# figure - and some are only prose. This counts both, per file, so the ones
# that are only prose can be looked at rather than guessed at.
#
# What it cannot do is decide whether a figure is right. It says which claims
# have a second reader and which have none, which is the question that keeps
# turning out to matter.
#
# Usage: scripts/claims-in-comments.sh [--unread]
#   --unread  list the claims no test names, rather than counting them

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

# Figures stated in a comment that no test names, counted on 2026-09-05. The
# gate runs this and holds the count to the number, in both directions: a new
# unread figure fails until somebody asserts it somewhere or raises this on
# purpose, and asserting one fails until the number comes down with it. The
# script was written on 2026-08-29 and nothing ran it, which is how a census
# becomes a number nobody has looked at since the day it was measured.
BASELINE_UNREAD=35

MODE=${1:-count}

# Every figure that appears in a comment, in the source directories a reader
# would trust: a millimetre, a micrometre, a percentage, an ounce, or a count
# of something named.
comment_figures() {
    grep -rhnE '^\s*(//|///|\*|#)' \
        --include=*.rs --include=*.ts \
        crates/*/src viewer/src viewer/server.ts 2>/dev/null \
    | grep -oE '[0-9]+(\.[0-9]+)?(mm|um|oz|%)' \
    | sort -u
}

# The figures a test names, whatever it does with them.
#
# Including the tests that live inside `src`. Rust puts unit tests in a
# `#[cfg(test)]` module in the file they test, and a census that reads only
# `crates/*/tests` calls every figure they assert unread - this project has
# already been wrong that way once, in a self-audit that claimed three helpers
# were untested because the grep never looked inside `src`.
tested_figures() {
    {
        grep -rhoE '[0-9]+(\.[0-9]+)?(mm|um|oz|%)' \
            --include=*.rs --include=*.ts \
            crates/*/tests viewer/src/__tests__ viewer/e2e 2>/dev/null
        # Everything from the first `#[cfg(test)]` in a source file onwards.
        for file in $(grep -rl '#\[cfg(test)\]' --include=*.rs crates/*/src 2>/dev/null); do
            awk '/#\[cfg\(test\)\]/ {inside = 1} inside' "$file"
        done | grep -oE '[0-9]+(\.[0-9]+)?(mm|um|oz|%)'
    } | sort -u
}

FIGURES=$(comment_figures)
TESTED=$(tested_figures)
UNREAD=$(comm -23 <(echo "$FIGURES") <(echo "$TESTED"))

# A figure on its own cannot be looked at. The first comment that states it
# can, so `--unread` names the file and the line as well: a census whose output
# is a column of numbers is a census nobody acts on.
if [ "$MODE" = "--unread" ]; then
    for figure in $UNREAD; do
        printf '%-12s %s\n' "$figure" \
            "$(grep -rnE "^\s*(//|///|\*|#).*${figure//./\\.}" \
                --include=*.rs --include=*.ts \
                crates/*/src viewer/src viewer/server.ts 2>/dev/null \
                | head -1 | cut -c1-140)"
    done
    exit 0
fi

total=$(echo "$FIGURES" | grep -c . || true)
unread=$(echo "$UNREAD" | grep -c . || true)
echo "figures stated in comments: $total"
echo "of those, named by no test:  $unread"
echo "read back by a test:         $((total - unread))"

if [ "$unread" -ne "$BASELINE_UNREAD" ]; then
    direction=$([ "$unread" -gt "$BASELINE_UNREAD" ] && echo more || echo fewer)
    echo ""
    echo "scripts/claims-in-comments.sh: $unread unread figures, $direction than"
    echo "the $BASELINE_UNREAD this file records. Run it with --unread to see"
    echo "which, then assert the figure or move BASELINE_UNREAD in the same commit."
    exit 1
fi
