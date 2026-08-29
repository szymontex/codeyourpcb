#!/usr/bin/env bash
#
# What the scheduled gate decides, decided against real repositories.
#
# `scripts/scheduled-gate.sh` is the only thing in this project that writes to
# a shared branch without a person watching, and until this file existed it was
# proved the way the tracker warns against: run once, by hand, on one day, on
# the one repository where every branch happened to line up. Its publish step
# has five outcomes and the interesting four never happened during that run.
#
# So the outcomes are made to happen here. Each case builds a throwaway origin
# and a clone in a temporary directory, puts the real script in it, and reads
# the log the script writes. Nothing here touches this repository, the network,
# or the gate itself: `GATE_COMMAND` stands in for the nine stages, because
# what is under test is the decision around them rather than the stages.
#
# Usage: scripts/scheduled-gate-selftest.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GATE="$SCRIPT_DIR/scheduled-gate.sh"
FAILURES=0

export GIT_AUTHOR_NAME="gate selftest"
export GIT_AUTHOR_EMAIL="selftest@example.invalid"
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME"
export GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

ok()   { echo "  ok   $1"; }
bad()  { echo "  FAIL $1"; echo "       $2"; FAILURES=$(( FAILURES + 1 )); }

# One clone of one origin, with the script inside it, ready to run.
#
# The clone is on a branch with one commit `main` does not have, which is the
# state every real run is in.
new_case() {
    local name=$1
    CASE="$TMP/$name"
    mkdir -p "$CASE"
    git init -q --bare "$CASE/origin.git"
    git clone -q "$CASE/origin.git" "$CASE/work" 2>/dev/null
    cd "$CASE/work" || exit 2
    git symbolic-ref HEAD refs/heads/main
    mkdir -p scripts
    cp "$GATE" scripts/scheduled-gate.sh
    echo "a board" > board.txt
    # A stage that fails if it can see somebody's work in progress. Committed,
    # so the worktree has it too: this is what proves the stages ran where the
    # half-edited file is not, rather than in the tree beside it.
    printf '#!/bin/sh\ngrep -q "half an edit" board.txt && exit 1\nexit 0\n' > gate-probe.sh
    chmod +x gate-probe.sh
    git add -A
    git commit -qm "the first commit"
    git push -q origin main
    # A bare repository's HEAD is `master` until something says otherwise, and
    # a clone of it lands on a branch that does not exist - which is how the
    # first version of case 3 pushed nothing and reported it as a pass.
    git -C "$CASE/origin.git" symbolic-ref HEAD refs/heads/main
    git checkout -qb work-branch
    echo "a change" >> board.txt
    git commit -qam "the commit the gate proves"
}

# Run the real script in the case's clone. Everything expensive is replaced:
# the nine stages by `GATE_COMMAND`, the hour of waiting by one attempt.
run_gate() {
    local gate_command=${1:-true}
    ( cd "$CASE/work" && \
      GATE_COMMAND="$gate_command" \
      GATE_RETRY_ATTEMPTS=1 \
      GATE_RETRY_SECONDS=0 \
      ./scripts/scheduled-gate.sh "$CASE/logs" >/dev/null 2>&1 )
    LOG="$CASE/logs/latest.log"
}

says() {
    local what=$1 why=$2
    if grep -qF "$what" "$LOG" 2>/dev/null; then
        ok "$why"
    else
        bad "$why" "the log does not say '$what': $(head -1 "$LOG" 2>/dev/null)"
    fi
}

says_not() {
    local what=$1 why=$2
    if grep -qF "$what" "$LOG" 2>/dev/null; then
        bad "$why" "the log says '$what' and should not"
    else
        ok "$why"
    fi
}

echo "=== what the scheduled gate decides ==="

# 1. The ordinary night: green, and the branch is one commit ahead of main.
new_case green-publishes
run_gate true
says "VERDICT: green" "a green run says so"
says "published: main fast-forwarded by 1 commit(s)" "a green run publishes the commit it proved"
PUBLISHED=$(git -C "$CASE/origin.git" rev-parse main)
TIP=$(git -C "$CASE/work" rev-parse HEAD)
if [ "$PUBLISHED" = "$TIP" ]; then
    ok "origin's main is the commit the gate proved"
else
    bad "origin's main is the commit the gate proved" "main=$PUBLISHED tip=$TIP"
fi

# 2. The second night with no new work: nothing to publish, and it says so
#    rather than pushing again.
run_gate true
says "main already carries this commit" "a second run publishes nothing"

# 3. Somebody else moved main. The fast-forward would lose their commit, so it
#    does not happen - and this is the case that proves the fetch: this clone's
#    `origin/main` is stale until the script refreshes it, and without the
#    refresh the script reads its own old ref, calls the push a fast-forward,
#    and finds out from the remote instead.
new_case main-moved-elsewhere
git clone -q "$CASE/origin.git" "$CASE/other" 2>/dev/null
( cd "$CASE/other" && echo "somebody else" >> board.txt && \
  git commit -qam "a commit only main has" && git push -q origin main )
run_gate true
says "not publishing: main has commits this branch does not" "a main that moved on its own is left alone"
says_not "published:" "nothing is pushed over somebody else's commit"
KEPT=$(git -C "$CASE/origin.git" log --oneline -1 --format=%s main)
if [ "$KEPT" = "a commit only main has" ]; then
    ok "the other commit is still the tip of main"
else
    bad "the other commit is still the tip of main" "main's tip is '$KEPT'"
fi

# 4. A red gate publishes nothing, whatever the ancestry says.
new_case red-publishes-nothing
run_gate false
says "VERDICT: red" "a failing stage makes the run red"
says_not "published:" "a red run publishes nothing"
BEHIND=$(git -C "$CASE/origin.git" log --oneline -1 --format=%s main)
if [ "$BEHIND" = "the first commit" ]; then
    ok "main did not move on a red run"
else
    bad "main did not move on a red run" "main's tip is '$BEHIND'"
fi

# 5. A tree somebody is working in is measured anyway, from the commit it has.
#
#    The first version of this waited an hour and then skipped. Three of the
#    four nights after it skipped: a fire is mid-edit most of the night, so the
#    gate the waiting was protecting never ran at all. The tip gets its own
#    directory now, and the half-edited file is not in it.
new_case busy-tree-measures-the-tip
echo "half an edit" >> "$CASE/work/board.txt"
# The stage itself refuses to pass if it can see the edit, so a green verdict
# is proof of which directory it ran in rather than a claim about it.
run_gate ./gate-probe.sh
says "VERDICT: green" "a busy tree no longer stops the gate, and the stages ran where the edit is not"
says "measures the committed tip" "and the run says what it measured instead"
says "published: main fast-forwarded by 1 commit(s)" "a green run on the tip publishes it"
if [ -n "$(cd "$CASE/work" && git status --porcelain)" ]; then
    ok "the work in progress is still there afterwards"
else
    bad "the work in progress is still there afterwards" "the tree came back clean"
fi
if [ -d "$CASE/logs/tip" ]; then
    bad "the scratch worktree is cleaned up" "$CASE/logs/tip is still there"
else
    ok "the scratch worktree is cleaned up"
fi

# 5b. And the switch that brings the old behaviour back, for a machine where a
#     second checkout is not wanted.
new_case busy-tree-can-still-skip
echo "half an edit" >> "$CASE/work/board.txt"
( cd "$CASE/work" && GATE_WORKTREE=0 GATE_COMMAND=true GATE_RETRY_ATTEMPTS=1 \
  ./scripts/scheduled-gate.sh "$CASE/logs" >/dev/null 2>&1 )
LOG="$CASE/logs/latest.log"
says "VERDICT: skipped" "GATE_WORKTREE=0 skips a busy tree"
says_not "published:" "and a skipped run publishes nothing"

# 6. The switch exists for a reason: a run that is asked not to publish does
#    not, and still says the gate was green.
new_case publish-switch-off
( cd "$CASE/work" && GATE_PUBLISH=0 GATE_COMMAND=true GATE_RETRY_ATTEMPTS=1 \
  ./scripts/scheduled-gate.sh "$CASE/logs" >/dev/null 2>&1 )
LOG="$CASE/logs/latest.log"
says "VERDICT: green" "GATE_PUBLISH=0 still runs the gate"
says_not "published:" "GATE_PUBLISH=0 publishes nothing"

# 7. A refresh that fails is said out loud. `origin/main` is a cached answer
#    and every test above reads it; a fetch that cannot reach the remote
#    leaves the cache in place, so the run decides against whatever this
#    checkout last saw. Silence there is how a green gate publishes nothing
#    and calls it "main already carries this commit".
new_case unreachable-origin
git -C "$CASE/work" remote set-url origin "$CASE/there-is-no-repository-here.git"
run_gate true
says "the fetch from origin failed" "a refresh that fails is said out loud"

echo ""
if [ "$FAILURES" -eq 0 ]; then
    echo "=== the scheduled gate decides what it says it decides ==="
    exit 0
fi
echo "=== $FAILURES case(s) failed ==="
exit 1
