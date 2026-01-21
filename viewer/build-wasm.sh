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

# Build the WASM module with the wasm feature (excludes tree-sitter)
wasm-pack build crates/cypcb-render \
  --target web \
  --out-dir ../../viewer/pkg \
  --out-name cypcb_render \
  --no-default-features \
  --features wasm

echo ""
echo "WASM build complete!"
echo "Output: viewer/pkg/"
ls -la viewer/pkg/
