#!/bin/bash
set -e

# Build the WASM module the viewer loads, into viewer/pkg for Vite to find.
#
# Three steps, each doing one thing: cargo compiles for wasm32, wasm-bindgen
# writes the JavaScript bindings, wasm-opt shrinks the module.
#
# This was `wasm-pack build --release` until 2026-08-08. The release profile
# now optimizes for speed, because the command-line router is what a release
# build is for, and the browser's size build lives in `wasm-release`. wasm-pack
# 0.14 cannot be told about a custom profile: `--profile wasm-release` makes it
# ignore `[package.metadata.wasm-pack.profile.*]` entirely and run its own
# plain `wasm-opt -O`, which refuses this module -
#
#   [wasm-validator error in function 0] Bulk memory operation
#   Fatal: error validating input
#
# - because the module uses bulk memory and non-trapping float conversions.
# Setting `wasm-opt = false` does not help either; the metadata is not read at
# all for a custom profile. Doing the three steps here removes the guessing.

cd "$(dirname "$0")/.."

echo "Building WASM module..."

if ! command -v wasm-bindgen &> /dev/null; then
    echo "wasm-bindgen not found. Install the CLI at the version the workspace"
    echo "pins: cargo install wasm-bindgen-cli --version \$(grep -A1 '^name = \"wasm-bindgen\"' Cargo.lock | grep version | cut -d'\"' -f2)"
    exit 1
fi

# wasm-opt is not optional. It used to be - the script warned and carried on,
# which shipped an unoptimized module the moment binaryen was missing from the
# machine, silently and a third larger.
if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt not found, and this build needs it."
    echo "  Debian/Ubuntu: apt-get install binaryen"
    echo "  macOS:         brew install binaryen"
    echo "  Or:            cargo install wasm-opt"
    exit 1
fi

if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "Adding wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Work around TLS allocation issue on some Linux systems
export GLIBC_TUNABLES=glibc.rtld.optional_static_tls=2048

# The directory this is built in used to end up inside the module. rustc
# records source paths - panic messages name the file they came from - so the
# same commit built in `/workspace/codeyourpcb` and in the scheduled gate's
# worktree produced two different files, and the gate reads that difference as
# a stale artifact. Measured 2026-08-31: without this line the two builds'
# `cypcb_render_bg.wasm` differ; with it both are
# `b0e94102cef39dec22557fc78f717e3d`.
#
# It is the checkout that is remapped and not the home directory, so this says
# nothing about two machines: the registry and the toolchain still live at
# their own paths. What it buys is that one machine's answer to "does
# rebuilding this source change the committed module" no longer depends on
# which directory the source is sitting in.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$(pwd)=/cypcb"

# The `wasm` feature carries the Rust reader, so this module parses .cypcb
# itself and PcbEngine::load_source is exported to JS.
#
# The note that used to sit here said the tree-sitter parser "does build for
# wasm32" and quoted sizes for it. That was false: cc fell back to the host
# compiler and the wasm linker skipped the wrong-architecture objects in
# silence, which cypcb-parser/build.rs now refuses to let happen.
#
# What the reader costs, measured on this script's output:
#
#   parser        raw          gzipped
#   none          751,995      290,004
#   Rust reader   1,044,164    411,147
#
# The 292KB is repaid by deleting parseSource from viewer/src/wasm.ts, the
# second reader that does not instantiate modules or follow imports.
#
# `wasm-release` is the release profile with `opt-level = "z"`. Measured on
# this module: 1,134,175 bytes at "z" against 1,460,669 at 3.
echo "  cargo build --profile wasm-release --target wasm32-unknown-unknown"
cargo build \
  --profile wasm-release \
  --target wasm32-unknown-unknown \
  -p cypcb-render \
  --no-default-features \
  --features wasm

echo "  wasm-bindgen"
wasm-bindgen \
  target/wasm32-unknown-unknown/wasm-release/cypcb_render.wasm \
  --target web \
  --out-dir viewer/pkg \
  --out-name cypcb_render

# Every feature rustc's wasm32 target emits for this module. Without the full
# list wasm-opt refuses it - `all used features should be allowed` - and the
# list is spelled out rather than passed as `--all-features` so that enabling a
# feature browsers may not have is a decision somebody makes on purpose.
echo "  wasm-opt -O4 --converge"
wasm-opt -O4 --converge \
  --enable-bulk-memory \
  --enable-nontrapping-float-to-int \
  --enable-sign-ext \
  --enable-mutable-globals \
  --enable-reference-types \
  --enable-multivalue \
  viewer/pkg/cypcb_render_bg.wasm \
  -o viewer/pkg/cypcb_render_bg.wasm

echo ""
echo "WASM build complete!"
echo "Output: viewer/pkg/"
ls -la viewer/pkg/
