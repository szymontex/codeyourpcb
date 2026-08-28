#!/usr/bin/env bash
#
# Engine methods the browser never reaches.
#
# `cypcb-render` exposes the board engine to the viewer through
# `#[wasm_bindgen]`, and `viewer/src/wasm.ts` wraps every one of those methods
# whether or not anything calls it. A wrapper is not a caller: the variant
# search has had a bridge, a type and a mock stub since the panel that drove it
# was deleted, and nothing in the application has reached it since.
#
# This prints, as `unused-exports.sh` does, and for the same reason: a method
# nobody calls today may be the API a panel uses tomorrow, and that is a
# person's call. What it removes is the guess.
set -euo pipefail

cd "$(dirname "$0")/.."

python3 - <<'PY'
import re, os, subprocess

engine = open("crates/cypcb-render/src/lib.rs").read()
methods = sorted(set(re.findall(r"^\s*pub fn ([a-z_0-9]+)\s*\(", engine, re.M)))

viewer = {}
for base, _, names in os.walk("viewer/src"):
    for name in names:
        if name.endswith(".ts"):
            path = os.path.join(base, name)
            viewer[path] = open(path).read()

bridge = "viewer/src/wasm.ts"
unreached = []
for method in methods:
    call = re.compile(r"\." + re.escape(method) + r"\s*\(")
    # The bridge's own passthrough is not a caller: `wasm.ts` forwards a call
    # it was given. Anything else calling the method - including a facade in
    # `wasm.ts` that wraps several engine calls into one - is.
    passthrough = re.compile(r"this\.wasmEngine\." + re.escape(method) + r"\s*\(")
    reached = False
    for path, text in viewer.items():
        hits = list(call.finditer(text))
        if not hits:
            continue
        if path != bridge:
            reached = True
            break
        for hit in hits:
            line_start = text.rfind("\n", 0, hit.start()) + 1
            line_end = text.find("\n", hit.end())
            line = text[line_start:line_end if line_end != -1 else len(text)]
            if not passthrough.search(line):
                reached = True
                break
        if reached:
            break
    if reached:
        continue
    wrapped = bool(re.search(r"\b" + re.escape(method) + r"\s*\(", viewer.get(bridge, "")))
    unreached.append((method, wrapped))

for method, wrapped in unreached:
    where = "wrapped in wasm.ts, called by nothing" if wrapped else "not wrapped at all"
    print(f"{method}: {where}")
print(f"total {len(unreached)} of {len(methods)} engine methods")
PY
