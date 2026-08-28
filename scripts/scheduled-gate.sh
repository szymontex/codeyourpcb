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

# The nine stages, named rather than hard-coded, so the decision around them
# can be tested without paying for them. `scripts/scheduled-gate-selftest.sh`
# runs this script against throwaway repositories with `GATE_COMMAND=true` and
# `GATE_COMMAND=false`; nothing else sets it.
GATE_COMMAND=${GATE_COMMAND:-./scripts/quality-gate.sh}
STAMP=$(date +%Y-%m-%dT%H-%M-%S)
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/$STAMP.log"
BODY=$(mktemp)

say() {
    echo "$1" | tee -a "$BODY"
}

# The verdict goes first in the finished file. The first version of this said
# so in a comment and then appended it, which put it under six thousand lines
# of stage output - so the reader it was written for would have had to know to
# look at the end. The body is collected in a temporary file and the log is
# assembled once, verdict at the top.
finish() {
    { echo "$1"; cat "$BODY"; } > "$LOG"
    rm -f "$BODY"
    ln -sf "$LOG" "$LOG_DIR/latest.log"
    ls -1t "$LOG_DIR"/*.log 2>/dev/null | tail -n +31 | while read -r old; do
        [ "$old" = "$LOG_DIR/latest.log" ] || rm -f "$old"
    done
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
    finish "VERDICT: skipped - a gate is already running"
    echo "VERDICT: skipped - a gate is already running"
    exit 0
fi

# viewer/pkg is a committed artifact the gate itself rebuilds, so a difference
# there is the gate's own doing and not somebody's work in progress.
#
# A busy tree waits rather than giving up at once. On 2026-08-28 the 04:30 run
# skipped because a fire was mid-edit, and the consequence was that the gate
# did not run at all that night - the skip is quiet, the next run is a day
# away, and nobody reads a log that says "skipped" until something else goes
# wrong. Three checks twenty minutes apart cost an hour of waiting and catch
# every fire shorter than that; a tree still busy after them really is busy.
#
# `GATE_RETRY_ATTEMPTS` and `GATE_RETRY_SECONDS` exist so this is testable
# without waiting an hour to see it work.
ATTEMPTS=${GATE_RETRY_ATTEMPTS:-3}
INTERVAL=${GATE_RETRY_SECONDS:-1200}
WAITED=0
DIRTY=$(git status --porcelain | grep -v "^.. viewer/pkg/" || true)
while [ -n "$DIRTY" ] && [ "$ATTEMPTS" -gt 1 ]; do
    say "the working tree is busy, waiting ${INTERVAL}s ($((ATTEMPTS - 1)) more check(s))"
    sleep "$INTERVAL"
    WAITED=$(( WAITED + INTERVAL ))
    ATTEMPTS=$(( ATTEMPTS - 1 ))
    DIRTY=$(git status --porcelain | grep -v "^.. viewer/pkg/" || true)
done
if [ -n "$DIRTY" ]; then
    echo "$DIRTY" | sed 's/^/  /' >> "$BODY"
    VERDICT="VERDICT: skipped - the working tree is busy after waiting ${WAITED}s"
    finish "$VERDICT"
    echo "$VERDICT"
    echo "$DIRTY" | sed 's/^/  /'
    exit 0
fi
if [ "$WAITED" -gt 0 ]; then
    say "the tree went quiet after ${WAITED}s"
fi

say "running $GATE_COMMAND"
START=$(date +%s)
$GATE_COMMAND >>"$BODY" 2>&1
CODE=$?
ELAPSED=$(( $(date +%s) - START ))

if [ "$CODE" -eq 0 ]; then
    VERDICT="VERDICT: green, all stages passed, ${ELAPSED}s"

    # A green run moves `main` up to the commit it just proved.
    #
    # D12, answered 2026-08-28: "wdrazaj na main wszystko jak leci... grunt
    # zeby dzialalo i zeby nic nie stracic co wartosciowe". `main` had sat
    # sixteen days and 375 commits behind the work because publishing was a
    # thing somebody had to remember; this is the thing that remembers.
    #
    # Only ever a fast-forward, and only from the branch this checkout is on:
    # `--ff-only` semantics come from the ancestry test, so a `main` that has
    # moved on its own is left alone with a line saying why. Nothing here
    # forces, rebases or deletes - the rule the same answer set: lose nothing.
    if [ "${GATE_PUBLISH:-1}" = "1" ]; then
        BRANCH=$(git rev-parse --abbrev-ref HEAD)
        # `origin/main` is a cached answer, and every test below reads it.
        # A fetch that fails leaves the cache in place, so the run would
        # decide against whatever this checkout last saw - which is how a
        # green gate publishes nothing and says main already carries the
        # commit. If the refresh fails, the log says so before the decision.
        if ! git fetch -q origin 2>>"$BODY"; then
            say "the fetch from origin failed: the decision below reads a cached origin/main"
        fi
        if [ "$BRANCH" = "HEAD" ]; then
            say "not publishing: this checkout is on a detached HEAD"
        elif ! git rev-parse --verify -q origin/main >/dev/null; then
            say "not publishing: origin has no main"
        elif git merge-base --is-ancestor HEAD origin/main; then
            say "main already carries this commit"
        elif ! git merge-base --is-ancestor origin/main HEAD; then
            say "not publishing: main has commits this branch does not - fast-forward would lose them"
        else
            AHEAD=$(git rev-list --count origin/main..HEAD)
            if git push -q origin "HEAD:refs/heads/main" 2>>"$BODY"; then
                say "published: main fast-forwarded by $AHEAD commit(s) to $(git rev-parse --short HEAD)"
                VERDICT="$VERDICT, main fast-forwarded by $AHEAD"
            else
                say "publish failed: the push to main was refused, see above"
                VERDICT="$VERDICT, publish to main failed"
            fi
        fi
    fi
else
    STAGE=$(grep -E "^\[[0-9]+/[0-9]+\]" "$BODY" | tail -1)
    VERDICT="VERDICT: red, exit $CODE after ${ELAPSED}s, last stage: ${STAGE:-unknown}"
fi

finish "$VERDICT"
echo "$VERDICT"

exit "$CODE"
