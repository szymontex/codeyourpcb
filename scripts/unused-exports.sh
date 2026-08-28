#!/usr/bin/env bash
#
# Exports in the viewer that nothing outside their own file names.
#
# `walkaround.ts` was 680 lines nothing imported, and it survived a year of
# green runs because nothing counted. Whole modules are guarded now by
# `viewer/src/__tests__/no-module-is-written-and-never-imported.test.ts`; this
# is the finer question - a module that is imported for one thing can still
# export six others nobody wants.
#
# It prints rather than deletes, and it is a diagnostic rather than a gate: an
# export can be unused today and part of a public shape tomorrow, and deciding
# which is which is a person's job. What the script removes is the excuse that
# nobody knows how many there are.
#
# Usage: ./scripts/unused-exports.sh [--values-only]
set -euo pipefail

cd "$(dirname "$0")/.."

python3 - "$@" <<'PY'
import os, re, sys

values_only = "--values-only" in sys.argv

root = "viewer/src"
files = []
for base, _, names in os.walk(root):
    for name in names:
        if name.endswith(".ts") and not name.endswith(".d.ts"):
            files.append(os.path.join(base, name))

texts = {path: open(path).read() for path in files}

kinds = r"function|const|class|enum" if values_only else r"function|const|class|interface|type|enum"
exported = re.compile(rf"^export\s+(?:async\s+)?(?:{kinds})\s+([A-Za-z_][A-Za-z0-9_]*)", re.M)

found = []
for path, text in texts.items():
    for match in exported.finditer(text):
        name = match.group(1)
        word = re.compile(r"\b" + re.escape(name) + r"\b")
        if not any(word.search(other) for p, other in texts.items() if p != path):
            found.append((path, name))

for path, name in sorted(found):
    print(f"{path}: {name}")
print(f"total {len(found)}")
PY
