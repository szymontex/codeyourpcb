#!/usr/bin/env bash
# Is the committed `viewer/pkg` the module this source builds?
#
# `viewer/pkg` is a committed artifact: a clean clone of this branch serves
# whatever was compiled the last time somebody rebuilt it. If a source the
# module is built from moved after that commit, every clone runs an engine that
# is not this branch's - on 2026-08-27 that was a module built against an older
# dependency set, 5,241 bytes of difference nobody had looked at.
#
# The question is asked of the history rather than of the bytes on purpose.
# `rust-toolchain.toml` pins the channel and not a version, and binaryen is not
# pinned at all, so two machines build this module into different bytes and a
# byte comparison would fail for the wrong reason.
#
# What counts as a source is the part that was wrong. It used to be all of
# `crates/*/src` and all of `Cargo.lock`, and both are wider than the module:
# `viewer/pkg` is `cargo build -p cypcb-render --no-default-features --features
# wasm`, whose path dependencies are 10 of this workspace's 17 crates. The
# nightly gate went red on 2026-08-30 because `81c6b71` changed
# `cypcb-library` and `cypcb-cli` - neither reachable from the module - and
# added two lines to `Cargo.lock`, both of them inside those same two packages'
# entries. The gate asked for a megabyte of rebuilt wasm to answer a change
# that cannot reach it, which is how a check earns the reflex to ignore it.
#
# So each half is asked precisely:
#   - the crates are the module's own dependency closure, from `cargo tree`
#   - `Cargo.lock` counts when an entry for a package in that closure changed,
#     which is what a version bump the module actually links looks like, and
#     not when an entry for a crate the module never sees did
#
# History alone cannot answer it, which is the other half of what went wrong.
# `315b227` rewrote a doc comment in `crates/cypcb-parser/src/ast.rs` - an
# example inside `///` lines, read by a test and by nothing that compiles into
# the module. The input's commit moved, so the history said stale; the rebuild
# produced the committed module byte for byte, so it was not. No commit could
# have made that check green: rebuilding changed no file, so there was nothing
# to commit, and the next run asked the same question again.
#
# So the history is the first half and the rebuild is the second, and both have
# to say stale before this does. **Run this after `viewer/build-wasm.sh`**: the
# second half is "did rebuilding this source change the committed module", read
# from `git status` on `viewer/pkg`. That comparison is against this machine's
# own rebuild rather than against another machine's bytes, which is the
# comparison `rust-toolchain.toml` and an unpinned binaryen make meaningless.
#
# The question is asked of the committed state on purpose, so the commit that
# moves an input would be graded by the *next* run: `315b227` shipped a module
# the gate then called stale, and four runs said green before the nightly said
# otherwise. Asking the working tree instead answers the wrong question - a
# fire is mid-edit whenever it runs the gate, and a red for every uncommitted
# change to a crate the module links would only mean running the gate twice for
# every such change. So the working tree gets a **notice** rather than a
# verdict: nothing committed has moved, but rebuilding what is in the tree
# changed the module, so `viewer/pkg` belongs in the same commit as the edit.
# That is the sentence that was missing when `315b227` was committed.
#
# Prints nothing and exits 0 when the committed module is current; says which
# input moved and exits 1 when it is not.
#
# `--print-inputs` prints the paths the first half asks about, one per line,
# and answers nothing else. `--lock-packages OLD NEW` prints the closure
# packages whose `Cargo.lock` entries differ between two lock files.
# `--verdict MOVED REBUILT` prints `stale`, `notice` or `current` for the two
# answers given to it. All three exist so the parts can be tested without a
# repository whose history is a fixture.
set -uo pipefail

cd "$(dirname "$0")/.."

# The crates the module is built from.
#
# If cargo cannot answer, or the workspace stops keeping a package's source at
# `crates/<package>/src`, the set widens back to every crate and the whole lock
# file rather than narrowing to a wrong answer. This check is allowed to ask
# for a rebuild nobody needed; it is not allowed to miss one somebody did.
closure_names() {
  cargo tree -q -p cypcb-render --no-default-features --features wasm \
    --target wasm32-unknown-unknown -e normal,build --prefix none 2>/dev/null \
    | awk '{ print $1 }' | sort -u
}

NAMES=$(closure_names)
WORKSPACE_NAMES=$(printf '%s\n' "$NAMES" | grep '^cypcb-' || true)

WIDE=0
SOURCES=()
if [ -z "$WORKSPACE_NAMES" ]; then
  WIDE=1
else
  while read -r name; do
    [ -z "$name" ] && continue
    if [ -d "crates/$name/src" ]; then
      SOURCES+=("crates/$name/src")
    else
      WIDE=1
    fi
  done <<< "$WORKSPACE_NAMES"
fi

if [ "$WIDE" -eq 1 ]; then
  SOURCES=()
  for dir in crates/*/src; do
    SOURCES+=("$dir")
  done
fi
SOURCES+=("viewer/build-wasm.sh")

# The lock file comparison, as its own step so it can be run against two files
# that are not this repository's history.
lock_packages_changed() {
  local old_lock="$1" new_lock="$2"
  # The names go on the command line rather than down a pipe: the program
  # itself arrives on this python's stdin, so a pipe into it is read as more
  # program and the names never arrive. The first version of this function did
  # exactly that and reported no change to any package at all.
  # shellcheck disable=SC2086
  python3 - "$old_lock" "$new_lock" $NAMES <<'PY'
import re
import sys

def entries(path):
    """Every `[[package]]` block in a Cargo.lock, keyed by package name.

    A name can appear twice - two versions of the same crate in one tree - so
    the blocks are collected into a list and compared as a set. What is inside
    a block is the version, the source, the checksum and the names it depends
    on, which is everything about that package the lock file decides.
    """
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    found = {}
    for block in text.split("[[package]]"):
        name = re.search(r'^name = "(.+)"$', block, re.M)
        if name:
            found.setdefault(name.group(1), []).append(block.strip())
    return found

old, new = entries(sys.argv[1]), entries(sys.argv[2])
for name in sorted(set(sys.argv[3:])):
    if sorted(old.get(name, [])) != sorted(new.get(name, [])):
        print(name)
PY
}

# What the two halves add up to.
#
#   an input moved and the rebuild changes the module  -> stale
#   an input moved and the rebuild changes nothing     -> current
#   nothing committed moved and the rebuild changes it -> notice
#   neither                                            -> current
verdict() {
  local moved="$1" rebuilt="$2"
  if [ -n "$moved" ]; then
    if [ -n "$rebuilt" ]; then
      echo stale
    else
      echo current
    fi
  elif [ -n "$rebuilt" ]; then
    echo notice
  else
    echo current
  fi
}

case "${1:-}" in
  --print-inputs)
    printf '%s\n' "${SOURCES[@]}"
    exit 0
    ;;
  --lock-packages)
    lock_packages_changed "${2:?old lock file}" "${3:?new lock file}"
    exit 0
    ;;
  --verdict)
    verdict "${2-}" "${3-}"
    exit 0
    ;;
  "") ;;
  *)
    echo "usage: $(basename "$0") [--print-inputs | --lock-packages OLD NEW | --verdict MOVED REBUILT]" >&2
    exit 2
    ;;
esac

PKG_COMMIT=$(git log -1 --format=%H -- viewer/pkg)
if [ -z "$PKG_COMMIT" ]; then
  # Nothing committed under viewer/pkg: there is no artifact to be stale.
  exit 0
fi

MOVED=$(git diff --name-only "$PKG_COMMIT" HEAD -- "${SOURCES[@]}")

LOCK_REASON=""
if ! git diff --quiet "$PKG_COMMIT" HEAD -- Cargo.lock; then
  if [ "$WIDE" -eq 1 ]; then
    LOCK_REASON="Cargo.lock"
  else
    OLD_LOCK=$(mktemp)
    trap 'rm -f "$OLD_LOCK"' EXIT
    if git show "$PKG_COMMIT:Cargo.lock" > "$OLD_LOCK" 2>/dev/null; then
      CHANGED=$(lock_packages_changed "$OLD_LOCK" Cargo.lock | tr '\n' ' ')
      if [ -n "${CHANGED// /}" ]; then
        LOCK_REASON="Cargo.lock (${CHANGED% })"
      fi
    else
      # No lock file at that commit to compare against: say so by widening.
      LOCK_REASON="Cargo.lock"
    fi
  fi
fi

# Whether rebuilding this source changes what is committed. Cheap enough to
# ask before the answer is needed, and needed by two of the three verdicts.
REBUILT=$(git status --porcelain -- viewer/pkg)

case "$(verdict "$MOVED$LOCK_REASON" "$REBUILT")" in
  current)
    exit 0
    ;;
  notice)
    echo "  note: nothing committed has moved, and rebuilding this source changes"
    echo "        what is committed:"
    echo "$REBUILT" | sed 's/^/    /'
    echo "        commit viewer/pkg with the change, or the next run reds on it."
    exit 0
    ;;
esac

echo "  the committed viewer/pkg predates a build input:"
echo "    viewer/pkg last committed by $(git log -1 --format='%h %s' "$PKG_COMMIT")"
if [ -n "$MOVED" ]; then
  echo "$MOVED" | head -10 | sed 's/^/    changed since: /'
  TOTAL=$(echo "$MOVED" | wc -l)
  if [ "$TOTAL" -gt 10 ]; then
    echo "    ... and $((TOTAL - 10)) more"
  fi
fi
if [ -n "$LOCK_REASON" ]; then
  echo "    changed since: $LOCK_REASON"
fi
echo "  and rebuilding this source changes what is committed:"
echo "$REBUILT" | sed 's/^/    /'
echo "  commit viewer/pkg."
exit 1
