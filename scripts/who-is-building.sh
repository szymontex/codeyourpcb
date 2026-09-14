#!/usr/bin/env bash
#
# Who else is writing into this build directory.
#
# On 2026-09-14 a scheduled run went red on a doc-test with "two different
# versions of crate cypcb_drc are being used" - a message about the build graph
# and not about the code. The tree it graded was clean and the same commands
# were green three times afterwards, so the failure was in how that run's
# target directory came to be. What was missing to say more is the one thing no
# log recorded: whether another cargo was writing into the same directory at
# the time. The gate's own `flock` serialises the runs that take it, and a
# command somebody types by hand takes nothing.
#
# So the gate says who else is in there before it starts, and says it on every
# run. A line printed only on trouble makes its absence unreadable: "nobody was
# building" and "nobody looked" come out the same.
#
# `scanned` is the denominator. A count with no denominator under it reads best
# exactly when the measurement has disappeared - a scan that can see nothing at
# all also reports zero neighbours.
#
# This is a diagnostic and never a gate. It exits 0 whatever it finds, because
# a neighbouring build is a thing a reader needs to know about a run rather
# than a reason to refuse the run.
#
# Usage: scripts/who-is-building.sh <directory>
#        scripts/who-is-building.sh --selftest

set -uo pipefail

# Every process above this one, so the gate that called it and the shell above
# that are not reported as neighbours: the gate holds its own lock file open,
# and that file lives in the directory being asked about.
ancestors() {
    local pid=$$ seen=""
    while [ -n "$pid" ] && [ "$pid" != "0" ] && [ "$pid" != "1" ]; do
        seen="$seen $pid"
        pid=$(awk '/^PPid:/{print $2}' "/proc/$pid/status" 2>/dev/null)
    done
    printf '%s' "$seen"
}

who_is_building() {
    local dir=$1
    local resolved mine scanned=0 sharing=0 who="" pid fd target

    resolved=$(readlink -f "$dir" 2>/dev/null) || resolved=$dir
    [ -n "$resolved" ] || resolved=$dir
    mine=" $(ancestors) "

    for proc in /proc/[0-9]*; do
        pid=${proc#/proc/}
        scanned=$(( scanned + 1 ))
        case $mine in *" $pid "*) continue ;; esac
        for fd in "$proc"/fd/*; do
            target=$(readlink "$fd" 2>/dev/null) || continue
            case $target in
                "$resolved"/*)
                    sharing=$(( sharing + 1 ))
                    who="$who${who:+,}$pid:$(tr -d ' ' < "/proc/$pid/comm" 2>/dev/null || echo unknown)"
                    break
                    ;;
            esac
        done
    done

    printf 'sharing=%s scanned=%s who=%s\n' "$sharing" "$scanned" "${who:-none}"
}

# Both arms, because a check whose failing arm has never been run is a check
# nobody has seen work. The positive arm holds a real descriptor open inside a
# real directory; the negative arm asks about a directory nothing has touched.
selftest() {
    local tmp failures=0 out
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' RETURN

    mkdir -p "$tmp/busy" "$tmp/quiet"

    out=$(who_is_building "$tmp/quiet")
    case $out in
        "sharing=0 "*" who=none") echo "  ok   a directory nobody is building in reports none" ;;
        *) echo "  FAIL a directory nobody is building in reports none"; echo "       $out"; failures=1 ;;
    esac
    case $out in
        *scanned=0*) echo "  FAIL the scan looked at something"; echo "       $out"; failures=1 ;;
        *) echo "  ok   the scan says how many processes it looked at" ;;
    esac

    # A process of our own, holding a descriptor open under the directory. It
    # is not a child of this shell's ancestry chain, so the exclusion above
    # must not hide it - which is the half of this that could silently pass.
    ( exec 8>"$tmp/busy/artifact"; sleep 30 ) &
    local holder=$!
    local waited=0
    while [ ! -e "$tmp/busy/artifact" ] && [ "$waited" -lt 50 ]; do
        sleep 0.1
        waited=$(( waited + 1 ))
    done

    out=$(who_is_building "$tmp/busy")
    kill "$holder" 2>/dev/null
    wait "$holder" 2>/dev/null

    case $out in
        "sharing=0 "*) echo "  FAIL a directory somebody is building in names them"; echo "       $out"; failures=1 ;;
        *" who=none") echo "  FAIL a directory somebody is building in names them"; echo "       $out"; failures=1 ;;
        *) echo "  ok   a directory somebody is building in names them" ;;
    esac

    return "$failures"
}

case ${1:-} in
    --selftest)
        echo "=== who is building ==="
        if selftest; then
            echo "=== the scan sees a neighbour and sees an empty directory ==="
            exit 0
        fi
        echo "=== the scan does not do what it says ==="
        exit 1
        ;;
    "")
        echo "usage: $0 <directory> | $0 --selftest" >&2
        exit 2
        ;;
    *)
        who_is_building "$1"
        exit 0
        ;;
esac
