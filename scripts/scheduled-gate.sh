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
# The log directory is an argument and not a constant. It used to be a constant,
# and the constant was the one on the machine this runs on, which put a private
# path in a public repository - see scripts/no-private-paths.sh. The fallback
# below is the portable place a tool keeps state on Linux, so a stranger's clone
# writes somewhere sane; the scheduled entry that runs this passes its own
# directory, because that is where the reader of the verdict is looking.
#
# Usage: scripts/scheduled-gate.sh [log-directory]
#   default: $XDG_STATE_HOME/cypcb-gate, or $HOME/.local/state/cypcb-gate

set -uo pipefail

REPO=$(cd "$(dirname "$0")/.." && pwd)
LOG_DIR=${1:-${XDG_STATE_HOME:-$HOME/.local/state}/cypcb-gate}

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
    # Thirty of this runner's own logs. Two things this used to get wrong:
    # the glob took `manual-*` as well, so a burst of runs started by hand
    # would age out the scheduled history it is meant to keep; and only
    # `latest.log` was spared, while `manual-latest.log` - a symlink the same
    # glob matches - would have been removed the moment its own timestamp aged
    # far enough. A symlink removed here breaks a reader that follows it, and
    # this directory had 29 files when that was found, two short of the first
    # deletion.
    ls -1t "$LOG_DIR"/*.log 2>/dev/null | while read -r old; do
        [ -L "$old" ] && continue
        case ${old##*/} in manual-*) continue ;; esac
        echo "$old"
    done | tail -n +31 | while read -r old; do
        rm -f "$old"
    done
}

# A run log records one run. A file with no verdict is a record that ends in the
# middle of itself, and a file with two is two runs in one place - neither is a
# record anybody can read a month later.
#
# `manual-*` is excluded because a run somebody starts by hand writes no verdict
# line; symlinks point at files already counted; `$2` is the run being written
# right now, which has no verdict yet and must not be judged by the rule it is
# about to satisfy.
every_run_log_carries_one_verdict() {
    local dir=$1 current=${2:-} bad=0 seen=0 f n
    for f in "$dir"/*.log; do
        [ -e "$f" ] || continue
        [ -L "$f" ] && continue
        case ${f##*/} in manual-*) continue ;; esac
        [ "$f" = "$current" ] && continue
        seen=$((seen + 1))
        n=$(grep -c '^VERDICT:' "$f" 2>/dev/null || true)
        [ "$n" -eq 1 ] && continue
        bad=$((bad + 1))
        if [ "$n" -eq 0 ]; then
            echo "  x ${f##*/}: no verdict line - this runner writes the verdict"
            echo "      first and assembles the file once, so a file without one"
            echo "      was written over by something else"
        else
            echo "  x ${f##*/}: $n verdict lines - more than one run in one file"
        fi
    done
    # Printed on every run, including the runs with nothing to report: a line
    # that appears only on damage makes its absence unreadable.
    echo "LOG-INTEGRITY checked=$seen without_one_verdict=$bad"
    [ "$bad" -eq 0 ]
}

cd "$REPO" || exit 2

say "scheduled gate, $STAMP, $REPO"
BRANCH=$(git rev-parse --abbrev-ref HEAD)
COMMIT_AT_START=$(git rev-parse --short HEAD)
say "branch: $BRANCH, commit at start: $COMMIT_AT_START"

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

# Read after the lock, not before: another scheduled run in flight has a file
# with no verdict in it yet, which is a false red by construction. After
# `flock -n 9` this run is the only scheduled writer, and a run started by hand
# cannot raise it either because the rule skips `manual-*`.
#
# It does not stop the run. The damage has already happened and stopping does
# not undo it; stopping would trade today's good record for no record at all,
# which makes the check worse than the thing it guards; and a gate that is red
# for history nobody can repair is a gate somebody switches off. The count
# rides on the verdict instead, where a reader is already looking.
LOG_INTEGRITY=$(every_run_log_carries_one_verdict "$LOG_DIR" "$LOG" || true)
say "$LOG_INTEGRITY"
LOG_INTEGRITY_BAD=$(printf '%s' "$LOG_INTEGRITY" | sed -n 's/.*without_one_verdict=\([0-9]*\).*/\1/p')

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
# A tree still busy after the waiting is measured anyway, from the commit it
# has rather than from the files somebody is editing.
#
# The waiting was written for a fire that finishes; three of the four runs
# after it skipped, because a fire is mid-edit most of the night and the gate
# it was protecting simply never ran. A skip is quiet, and a quiet gate is the
# shape of failure this whole runner exists to prevent - so the tree stops
# being the thing measured. `git worktree` gives the committed tip its own
# directory: nothing there is half-edited, nothing this writes can trample the
# work in progress, and the commit it grades is the commit the publish step
# would push.
#
# What it borrows rather than builds: the viewer's `node_modules`, which is
# hundreds of megabytes and identical, and a cargo target directory of its own
# so a fire compiling at the same time neither waits for this nor is waited
# for. `GATE_WORKTREE=0` turns it off and the old skip comes back.
WORKTREE=""
if [ -n "$DIRTY" ] && [ "${GATE_WORKTREE:-1}" = "1" ]; then
    WORKTREE="$LOG_DIR/tip"
    rm -rf "$WORKTREE"
    if git worktree add --detach -f "$WORKTREE" HEAD >>"$BODY" 2>&1; then
        say "the tree is busy, so this run measures the committed tip in $WORKTREE"
        if [ -d "$REPO/viewer/node_modules" ] && [ ! -e "$WORKTREE/viewer/node_modules" ]; then
            ln -s "$REPO/viewer/node_modules" "$WORKTREE/viewer/node_modules"
        fi
        export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$LOG_DIR/target}"
        # And under the name the tree itself uses, because a stage that looks
        # for `target/release/cypcb` is asking about this checkout rather than
        # about cargo's environment.
        mkdir -p "$CARGO_TARGET_DIR"
        [ -e "$WORKTREE/target" ] || ln -s "$CARGO_TARGET_DIR" "$WORKTREE/target"
        cd "$WORKTREE" || exit 2
        DIRTY=""
    else
        say "the tip could not be checked out, so this run has nothing clean to measure"
        rm -rf "$WORKTREE"
        WORKTREE=""
    fi
fi

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

# The commit in the header was read before the waiting, and the waiting is
# exactly when a fire commits. On 2026-09-14 the header said one commit while
# the stages ran against the next one: the tree went quiet because somebody had
# committed, and the verdict was filed against the commit before theirs. A
# reader chasing that red opens a tree the red did not come from, which is
# worse than a log with no commit in it at all.
#
# So the commit is read again here, from the directory the stages are about to
# run in - the worktree when there is one, because that is the tree they see.
COMMIT=$(git rev-parse --short HEAD)
if [ "$COMMIT" != "$COMMIT_AT_START" ]; then
    say "the commit changed while this run waited: $COMMIT_AT_START -> $COMMIT"
fi
say "measuring commit: $COMMIT"

say "running $GATE_COMMAND"
START=$(date +%s)
$GATE_COMMAND >>"$BODY" 2>&1
CODE=$?
ELAPSED=$(( $(date +%s) - START ))

# The worktree is a scratch copy, and leaving it behind would have the next
# run remove a directory somebody might be reading. It goes as soon as the
# stages are done, before anything is published or said.
if [ -n "$WORKTREE" ]; then
    cd "$REPO" || exit 2
    git worktree remove --force "$WORKTREE" >>"$BODY" 2>&1 || rm -rf "$WORKTREE"
fi

if [ "$CODE" -eq 0 ]; then
    # The gate states how many stages it ran in its closing line, and this is
    # the one line a reader sees first: `all stages passed` reads the same
    # whether eighteen stages ran or one, which is what the count is for. A
    # gate that stated none is reported as having stated none rather than as
    # having passed everything.
    STAGES=$(grep -o 'All stages passed: .* stages, [0-9]* checks' "$BODY" | tail -1)
    if [ -n "$STAGES" ]; then
        VERDICT="VERDICT: green, ${STAGES#All stages passed: }, ${ELAPSED}s, commit $COMMIT"
    else
        VERDICT="VERDICT: green, the gate states no stage count, ${ELAPSED}s, commit $COMMIT"
    fi
    [ "${LOG_INTEGRITY_BAD:-0}" -gt 0 ] && VERDICT="$VERDICT, ${LOG_INTEGRITY_BAD} damaged log(s) in this directory"
    if [ -n "$WORKTREE" ]; then
        VERDICT="$VERDICT, measured from the committed tip"
    fi

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
    VERDICT="VERDICT: red, exit $CODE after ${ELAPSED}s, last stage: ${STAGE:-unknown}, commit $COMMIT"
    [ "${LOG_INTEGRITY_BAD:-0}" -gt 0 ] && VERDICT="$VERDICT, ${LOG_INTEGRITY_BAD} damaged log(s) in this directory"
fi

finish "$VERDICT"
echo "$VERDICT"

exit "$CODE"
