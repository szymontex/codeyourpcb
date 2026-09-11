#!/usr/bin/env bash
#
# The repository is public. Our working machinery is not: the machine this is
# built on, the container it is built in, the directory the checkout happens to
# sit in, the directory the nightly log happens to land in. None of that is
# about circuit boards, and all of it was in this repository until 2026-09-11,
# when one grep found a host name and twenty-two container paths in files
# written over four months.
#
# A patch would have let it come back on the next fire that pasted a path out
# of a terminal. This is the gate instead.
#
# Two halves, because the two things being hidden have different shapes.
#
#   1. Absolute paths under the roots a personal machine keeps its work in.
#      That is a shape, so it is checked mechanically and needs no secret.
#   2. Names - a host, a container, a session. Those cannot be listed in a
#      public file without publishing exactly what they are, so they are read
#      from a deny-list OUTSIDE the repository. When there is no such file, as
#      on a stranger's clone, this half is skipped and says so out loud rather
#      than passing quietly.
#
# Usage: scripts/no-private-paths.sh
#   CYPCB_PRIVATE_NAMES  path to the deny-list, one name per line, # comments
#                        default: $HOME/.config/cypcb/private-names.txt

set -uo pipefail

cd "$(cd "$(dirname "$0")/.." && pwd)" || exit 1

DENYLIST=${CYPCB_PRIVATE_NAMES:-$HOME/.config/cypcb/private-names.txt}

# The roots a personal machine keeps work under. A path under one of these is
# either somebody's home, a container's home, or a mount point, and none of
# them is portable enough to belong in a public repository even when it is not
# secret.
SL=/
ROOTS='home|config|workspace|root|mnt'

# Written as a shape rather than a literal so a URL cannot trip it: the path
# must not be preceded by anything a path or a domain is made of. That is what
# separates a private path from the same letters at the end of a URL, which is
# a real case in this repository - one of the vendor schema links has a root
# name in its path.
#
# Nothing below spells a private path out. This file is scanned like every
# other, so its own probes are assembled from pieces at run time; a literal
# here would make the check fail on itself, which is how it failed the first
# time it was run against a tree that tracked it.
PATTERN="(^|[^A-Za-z0-9_./-])/($ROOTS)/[A-Za-z0-9._-]"

# Deliberate placeholders. `/home/user` is the shape an example in API
# documentation has to have to be an example at all; the angle-bracket forms are
# what this project replaced its own paths with, and a check that rejected them
# would reject the fix.
ALLOWED="${SL}ho""me${SL}user|${SL}ho""me${SL}<|${SL}con""fig${SL}<|<checkout>|<log-dir>|<home>"

# `.gsd` is the planning archive of a previous build process. It is scanned like
# everything else - a record of what was done is still a public file.

# Tracked files AND new ones that are not ignored. Tracked alone is the obvious
# choice and it is wrong: this very script passed its first gate run because it
# was still untracked, so the check could not see the file the check lives in.
# A leak is worth catching before the commit that lands it, not after.
files() {
    git ls-files -z --cached --others --exclude-standard
}

scan() {
    grep -InE "$PATTERN" 2>/dev/null | grep -vE "$ALLOWED"
}

# The control, first, because a check that cannot fail proves nothing about a
# tree it passes. Three lines: one that must be caught, one that must not be
# caught because it is a URL, and one that must not be caught because it is the
# documented placeholder.
PROBE_BAD="cd ${SL}work""space${SL}somewhere"
PROBE_URL="https:${SL}${SL}schema.tauri.app${SL}con""fig${SL}2"
PROBE_OK="path=\"${SL}ho""me${SL}user${SL}my-parts.db\""

control_caught=$(printf 'a: %s\n' "$PROBE_BAD" | scan | wc -l)
control_url=$(printf 'b: %s\n' "$PROBE_URL" | scan | wc -l)
control_allowed=$(printf 'c: %s\n' "$PROBE_OK" | scan | wc -l)

if [ "$control_caught" -ne 1 ] || [ "$control_url" -ne 0 ] || [ "$control_allowed" -ne 0 ]; then
    echo "no-private-paths: the check itself is broken and this run proves nothing"
    echo "  a private path was caught   : $control_caught (want 1)"
    echo "  a URL was caught            : $control_url (want 0)"
    echo "  a placeholder was caught    : $control_allowed (want 0)"
    exit 1
fi

status=0

FOUND=$(files | xargs -0 grep -InE "$PATTERN" 2>/dev/null | grep -vE "$ALLOWED")
if [ -n "$FOUND" ]; then
    echo "no-private-paths: a file names a private path"
    echo "$FOUND" | head -40
    echo ""
    echo "  Replace it with what the sentence is actually about. This project"
    echo "  uses <checkout>, <log-dir> and <home>."
    status=1
else
    echo "no-private-paths: no file names a private path"
fi

if [ -f "$DENYLIST" ]; then
    NAMES=$(grep -vE '^\s*(#|$)' "$DENYLIST")
    if [ -z "$NAMES" ]; then
        echo "no-private-paths: the deny-list is empty, so no name was checked"
    else
        HITS=$(files | xargs -0 grep -Ilf <(echo "$NAMES") 2>/dev/null)
        # The control for this half too: a name that is on the list has to be
        # found when it is present, or an empty result means nothing.
        probe=$(printf '%s\n' "$NAMES" | head -1)
        if ! printf 'x %s y\n' "$probe" | grep -qF "$probe"; then
            echo "no-private-paths: the name check is broken and proves nothing"
            status=1
        elif [ -n "$HITS" ]; then
            # The names themselves are not printed: this output is read in a
            # log that is itself shared.
            echo "no-private-paths: a file names something on the private list"
            echo "$HITS" | head -20
            status=1
        else
            echo "no-private-paths: no file names anything on the private list"
        fi
    fi
else
    echo "no-private-paths: no deny-list at the configured path, so names were"
    echo "  not checked - only path shapes were. Set CYPCB_PRIVATE_NAMES to a"
    echo "  file outside this repository to check names too."
fi

exit $status
