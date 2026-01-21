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

# Build the WASM module
# Note: This may fail due to bevy_ecs/getrandom compatibility issues
# When it does, the viewer will use the mock implementation
wasm-pack build crates/cypcb-render \
  --target web \
  --out-dir ../../viewer/pkg \
  --out-name cypcb_render || {
    echo ""
    echo "WASM build failed (likely due to bevy_ecs/getrandom WASM compatibility)"
    echo "The viewer will use the mock implementation until this is resolved."
    echo ""
    echo "To use the full WASM version, the getrandom dependency needs WASM support."
    echo "See: https://github.com/rust-random/getrandom/issues/295"
    exit 0
}

echo "WASM build complete: viewer/pkg/"
ls -la viewer/pkg/
