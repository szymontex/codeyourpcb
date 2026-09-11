#!/usr/bin/env bash
#
# A number in a file this project writes comes from the board, not from here.
#
# Three literals in the KiCad writer's zone node turned out to be somebody's
# safety margin rather than the fab table: a pour standing twice as far off
# every pad as the house asks, a fill floor wider than any fab preset's trace
# width, and a thermal relief at nearly double what the Gerbers carried for the
# same board. Each was found by hand, months apart, and the third only because
# the first two made somebody look.
#
# The rule this checks: a decimal number inside a string this file writes out
# has to be traceable to the model, or carry a reason beside it.
#
# SCOPE IS THE WHOLE CHECK. It looks only at the format string handed to
# `write!` or `writeln!`. Scoped to the file instead, it would report the
# arithmetic the writers cannot work without - `1_000_000.0` converting
# nanometres, `1000.0` converting degrees - and, worse, the comments explaining
# the very literals that were removed, one of which quotes "0.5mm against
# tables holding 0.2 to 0.3". A gate that reports the documentation of its own
# fix is a gate somebody switches off.
#
# Why a decimal number and not any number: every millimetre in these writers
# goes through a conversion that divides by a million and always leaves a dot.
# An integer in emitted text is a layer number, a format version or part of a
# token name, and none of those is a dimension.
#
# The allowlist is a rule rather than a list of strings. A flagged line passes
# when the comment block directly above its `write!` carries the marker below
# with a reason - anywhere in that block, so a reason can take more than one
# line. That keeps the permission beside the literal, so it dies when the
# literal does, instead of drifting in a file nobody opens.
#
# Usage: scripts/no-invented-numbers.sh [file ...]
#   with no arguments, the writers listed below

set -uo pipefail

cd "$(cd "$(dirname "$0")/.." && pwd)" || exit 1

MARKER='// literal:'

FILES=("$@")
if [ ${#FILES[@]} -eq 0 ]; then
    FILES=(
        crates/cypcb-kicad/src/board_writer.rs
    )
fi

scan() {
    awk -v marker="$MARKER" '
        {
            line = $0
        }
        # A contiguous comment block above a call is where a reason lives. The
        # flag survives across the block so a reason can run to several lines,
        # and any line that is neither comment nor blank clears it - which is
        # what keeps one reason from covering the next literal down the file.
        /^[[:space:]]*\/\// {
            if (line ~ marker) { pending = 1 }
            next_is_comment = 1
        }
        !/^[[:space:]]*\/\// && !/^[[:space:]]*$/ && !/write!\(|writeln!\(/ {
            if (!inside) { pending = 0 }
        }
        # Opening a write!/writeln! - the reason, if any, is the block above.
        /write!\(|writeln!\(/ {
            inside = 1
            reason = pending
            pending = 0
        }
        inside && line ~ /"/ {
            # Only the parts inside double quotes.
            text = line
            while (match(text, /"[^"]*"/)) {
                chunk = substr(text, RSTART, RLENGTH)
                if (chunk ~ /[0-9]+\.[0-9]+/ && !reason) {
                    printf "%s:%d: %s\n", FILENAME, NR, chunk
                }
                text = substr(text, RSTART + RLENGTH)
            }
        }
        # A line ending the call closes the span.
        inside && /\);[[:space:]]*$/ { inside = 0 }
    ' "$1"
}

# The control, first, because a check that cannot fail proves nothing about the
# files it passes. Two lines: one that must be caught, and one with the marker
# above it that must not be.
probe=$(mktemp)
cat > "$probe" <<'PROBE'
fn a() {
    let _ = writeln!(out, "    (min_thickness 0.25)");
}
fn b() {
    // literal: the format's own version, not a property of the board
    let _ = writeln!(out, "    (version 20221018) (x 0.25)");
}
fn c() {
    let scale = 1_000_000.0;
}
PROBE
caught=$(scan "$probe" | wc -l)
rm -f "$probe"
if [ "$caught" -ne 1 ]; then
    echo "no-invented-numbers: the check itself is broken and this run proves nothing"
    echo "  the probe should catch exactly one literal, it caught $caught"
    exit 1
fi

status=0
for file in "${FILES[@]}"; do
    if [ ! -f "$file" ]; then
        echo "no-invented-numbers: $file is not there"
        status=1
        continue
    fi
    found=$(scan "$file")
    if [ -n "$found" ]; then
        echo "no-invented-numbers: a number in emitted text comes from nowhere"
        echo "$found" | sed 's/^/  /'
        echo ""
        echo "  Either take it from the design rules, or put a line above the"
        echo "  write! saying why it is not a property of the board:"
        echo "      $MARKER <reason>"
        status=1
    fi
done

if [ $status -eq 0 ]; then
    echo "no-invented-numbers: every number in emitted text is the board's or has a reason"
fi
exit $status
