#!/usr/bin/env bash
#
# One door out of nanometres, for the two files a fabricator reads literally.
#
# A Gerber coordinate is `X1000000Y500000` and an Excellon one is `X0150`: the
# decimal point is implied by a format the header declares, so a dimension in
# those files carries no dot. That is why the other numbers gate cannot help
# here - its trigger is a decimal, and a decimal never appears. A check that
# cannot fail is not evidence, and this is the shape that can fail.
#
# The rule is about the ACT, not the number: turning a length into text happens
# in `crates/cypcb-export/src/coords.rs` and nowhere else. Four doors live
# there - `nm_to_gerber`, `nm_to_decimal`, `gerber_format_string` and the
# `CoordinateFormat` that holds how many decimal places there are - and today
# there is not one second door. So this goes in as a ratchet on a state that is
# already right, which is the only time a ratchet is cheap.
#
# What it must NOT fire on matters more than what it catches. A gate that
# catches the bad and fires on the good is switched off, and then it catches
# nothing. Two kinds of legitimate code look superficially like the fault:
#
#   - `.0` handed to a helper. `nm_to_decimal(hit.position.x.0, format)` is
#     exactly right, and a rule that cannot tell it from `.0` handed to a
#     format string lights up on every existing line.
#   - conversion to floating point for geometry. Eight places compute an arc,
#     a rotation or a mask expansion in `f64` and come back to integers without
#     touching a string. So the trigger is a conversion inside a format
#     argument, never a conversion.
#
# Usage: scripts/one-door-coordinates.sh
#
# NOT WIRED INTO THE GATE YET. It passes on the tree today and its own controls
# hold, but the browser stage has been red on a machine at load average 35 with
# somebody else's work on it, so the full gate has not been able to answer.
# Wiring it is a one-line change to scripts/quality-gate.sh once it can.

set -uo pipefail

cd "$(cd "$(dirname "$0")/.." && pwd)" || exit 1

DIRS=(crates/cypcb-export/src/gerber crates/cypcb-export/src/excellon)

# Everything before the first `mod tests` in a file. A test may say anything.
production() {
    awk '/^#\[cfg\(test\)\]/ { exit } { print NR ": " $0 }' "$1"
}

scan() {
    production "$1" | awk -v file="$1" '
        {
            n = $0
            sub(/^[0-9]+: /, "", n)
            lineno = $0
            sub(/:.*/, "", lineno)
            hit = ""

            # A length turned into millimetres before anything formats it.
            if (n ~ /to_mm\(/) { hit = "to_mm outside coords.rs" }

            # The same, done by hand.
            else if (n ~ /\/ ?1_000_000/ || n ~ /1e6/) { hit = "a division by a million" }

            # Formatting a number with a chosen precision, which is what the
            # helper is for.
            else if (n ~ /\{:\./) { hit = "a precision format" }

            else if (n ~ /\.0\.to_string\(\)/) { hit = "a length turned into text" }

            # Raw nanometres handed to a string. The helper being named on the
            # same call is what tells this from the legitimate case, and that
            # distinction is the whole rule.
            else if (n ~ /format!\(|write!\(|writeln!\(|push_str\(/) {
                if (n ~ /\.0/ && n !~ /nm_to_decimal\(/ && n !~ /nm_to_gerber\(/) {
                    hit = "raw nanometres handed to a string"
                }
            }

            if (hit != "") { printf "%s:%s: %s\n", file, lineno, hit }
        }
    '
}

scan_all() {
    for dir in "${DIRS[@]}"; do
        [ -d "$dir" ] || continue
        for file in "$dir"/*.rs; do
            [ -f "$file" ] || continue
            scan "$file"
        done
    done
}

# The controls, first. Two that must be caught and two that must pass, and the
# passing pair is the more important: it is the reason this rule is written the
# way it is rather than the obvious way.
probe=$(mktemp -d)
mkdir -p "$probe/gerber" "$probe/excellon"
cat > "$probe/gerber/probe.rs" <<'PROBE'
fn a(seg: &Seg, out: &mut String) {
    out.push_str(&format!("X{}Y{}D02*\n", seg.start.x.0, seg.start.y.0));
}
fn b(result: &mut String, dcode: u32, diameter: Nm) {
    result.push_str(&format!("%ADD{}C,{:.6}*%\n", dcode, diameter.0 as f64 / 1_000_000.0));
}
fn c(hit: &Hit, format: CoordinateFormat) -> String {
    let x = nm_to_decimal(hit.position.x.0, format);
    x
}
#[cfg(test)]
mod tests {
    #[test]
    fn d() {
        assert_eq!(config.line_width.to_mm(), 0.15);
    }
}
PROBE
caught=$(DIRS=("$probe/gerber") ; for f in "$probe"/gerber/*.rs; do scan "$f"; done | wc -l)
lines=$(for f in "$probe"/gerber/*.rs; do scan "$f"; done | sed 's/.*probe.rs:\([0-9]*\).*/\1/' | sort -n | tr '\n' ' ')
rm -rf "$probe"
if [ "$caught" -ne 2 ]; then
    echo "one-door-coordinates: the check itself is broken and this run proves nothing"
    echo "  the probe plants two faults and two legitimate lines"
    echo "  it should catch exactly 2, it caught $caught on lines: $lines"
    exit 1
fi

found=$(scan_all)
if [ -n "$found" ]; then
    echo "one-door-coordinates: a length becomes text outside coords.rs"
    echo "$found" | sed 's/^/  /'
    echo ""
    echo "  Every coordinate and every dimension in these two formats goes"
    echo "  through nm_to_gerber or nm_to_decimal, which know how many decimal"
    echo "  places the header declared. A number formatted here instead is"
    echo "  wrong by that factor and looks perfectly well-formed."
    exit 1
fi

echo "one-door-coordinates: every length in these writers goes through coords.rs"
echo "  Checked: ${DIRS[*]}"
exit 0
