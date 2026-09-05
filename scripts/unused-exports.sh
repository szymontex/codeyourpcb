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
# The two halves of what it prints are not the same question, and the first
# hand run said so: **33 exports nothing else names, and 0 of them are
# values**. An exported `interface` beside the function that returns it is
# ordinary style, and a gate on that number would fire on every new type. An
# exported function, const, class or enum that no other file names is dead
# code, and `walkaround.ts` is what that costs.
#
# So the type half stays a diagnostic - it prints, a person decides - and the
# value half is a gate: `--values-only` holds the count to `BASELINE_VALUES`,
# which the gate runs. Nothing was deleted to reach zero; zero is where the
# viewer already was, and this is what keeps it there.
#
# Usage: ./scripts/unused-exports.sh [--values-only]
set -euo pipefail

# Exported values in `viewer/src` that no other file names, counted on
# 2026-09-06. Only a person moves this, in the commit that moves the code.
BASELINE_VALUES=0

cd "$(dirname "$0")/.."

BASELINE_VALUES="$BASELINE_VALUES" python3 - "$@" <<'PY'
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

if values_only:
    baseline = int(os.environ["BASELINE_VALUES"])
    if len(found) != baseline:
        direction = "more" if len(found) > baseline else "fewer"
        print(
            f"scripts/unused-exports.sh: {len(found)} exported values nothing "
            f"else names, {direction} than the {baseline} this file records. "
            f"Call it, delete it, or move BASELINE_VALUES in the same commit."
        )
        raise SystemExit(1)
PY
