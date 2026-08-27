#!/usr/bin/env bash
#
# The quality gate, run on a schedule rather than when somebody remembers.
#
# D10, answered 2026-08-27: no GitHub Actions, the suite runs on this server.
# That leaves two things this script has to get right, because a scheduled run
# shares a machine with the work.
#
#   1. It must not disturb a checkout somebody is using. The gate rebuilds
#      viewer/pkg and Playwright starts a server, so a run against a tree with
#      uncommitted work in it would both lie about that work and trample it.
#      A dirty tree is a skip, not a failure - the fire that is mid-edit is not
#      a regression.
#   2. It must not run twice at once. Two gates on one machine fight over the
#      Playwright port and the cargo target directory.
#
# Everything it learns goes in one line at the top of the log, so a reader who
# opens the newest file sees the verdict before the scrollback.
#
# Usage: scripts/scheduled-gate.sh [log-directory]
#   default log directory: /config/gate-runs

set -uo pipefail

REPO=$(cd "$(dirname "$0")/.." && pwd)
LOG_DIR=${1:-/config/gate-runs}
STAMP=$(date +%Y-%m-%dT%H-%M-%S)
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/$STAMP.log"

say() {
    echo "$1" | tee -a "$LOG"
}

cd "$REPO" || exit 2

say "scheduled gate, $STAMP, $REPO"
say "branch: $(git rev-parse --abbrev-ref HEAD), commit: $(git rev-parse --short HEAD)"

# A lock rather than a process name. The first version of this asked `pgrep -f
# scripts/quality-gate.sh`, which matched the shell that had the string in its
# own command line - the guard fired on itself and every run reported "already
# running". A lock file is the thing that is actually being contended.
exec 9>"$LOG_DIR/.lock"
if ! flock -n 9; then
    say "VERDICT: skipped - a gate is already running"
    exit 0
fi

# viewer/pkg is a committed artifact the gate itself rebuilds, so a difference
# there is the gate's own doing and not somebody's work in progress.
DIRTY=$(git status --porcelain | grep -v "^.. viewer/pkg/" || true)
if [ -n "$DIRTY" ]; then
    say "VERDICT: skipped - the working tree is busy"
    echo "$DIRTY" | sed 's/^/  /' | tee -a "$LOG"
    exit 0
fi

say "running ./scripts/quality-gate.sh"
START=$(date +%s)
./scripts/quality-gate.sh >>"$LOG" 2>&1
CODE=$?
ELAPSED=$(( $(date +%s) - START ))

if [ "$CODE" -eq 0 ]; then
    say "VERDICT: green, all stages passed, ${ELAPSED}s"
else
    STAGE=$(grep -E "^\[[0-9]+/[0-9]+\]" "$LOG" | tail -1)
    say "VERDICT: red, exit $CODE after ${ELAPSED}s, last stage: ${STAGE:-unknown}"
fi

# The newest run, findable without knowing today's date.
ln -sf "$LOG" "$LOG_DIR/latest.log"

# Keep a month of runs; the logs carry every stage's output and grow.
ls -1t "$LOG_DIR"/*.log 2>/dev/null | tail -n +31 | while read -r old; do
    [ "$old" = "$LOG_DIR/latest.log" ] || rm -f "$old"
done

exit "$CODE"
