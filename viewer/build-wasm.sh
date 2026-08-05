#!/bin/bash
set -e

# Build WASM module with wasm-pack
# Output to viewer/pkg for Vite to find

cd "$(dirname "$0")/.."

echo "Building WASM module..."

# Check if wasm-pack is available
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Install with: cargo install wasm-pack"
    exit 1
fi

# Check if wasm32 target is installed
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "Adding wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Work around TLS allocation issue on some Linux systems
export GLIBC_TUNABLES=glibc.rtld.optional_static_tls=2048

# Built without the tree-sitter parser: the viewer parses .cypcb in JavaScript
# and hands the engine a snapshot.
#
# The parser does build for wasm32 - the "not WASM compatible" note in
# cypcb-parser/Cargo.toml is wrong - and switching to --features native exports
# PcbEngine::load_source to JS for 804,520 bytes against 702,357 here, so about
# 100KB buys deleting the duplicate JS parser in viewer/src/wasm.ts. Do both in
# one step or the module carries the parser twice.
wasm-pack build crates/cypcb-render \
  --target web \
  --release \
  --out-dir ../../viewer/pkg \
  --out-name cypcb_render \
  --no-default-features \
  --features wasm

# Post-build optimization with wasm-opt (if available)
# Note: wasm-pack already runs wasm-opt, but we run it again with aggressive settings
if command -v wasm-opt &> /dev/null; then
  echo ""
  echo "Running wasm-opt for additional size optimization..."
  wasm-opt -O4 --converge \
    --enable-bulk-memory \
    --enable-nontrapping-float-to-int \
    viewer/pkg/cypcb_render_bg.wasm \
    -o viewer/pkg/cypcb_render_bg.wasm
  echo "Optimized WASM size:"
  ls -lh viewer/pkg/cypcb_render_bg.wasm
else
  echo ""
  echo "wasm-opt not found, skipping additional optimization."
  echo "Install binaryen for smaller builds: cargo install wasm-opt"
fi

echo ""
echo "WASM build complete!"
echo "Output: viewer/pkg/"
ls -la viewer/pkg/
