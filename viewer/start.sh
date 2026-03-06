#!/bin/bash
# CodeYourPCB unified development start script
# Builds WASM if needed, then starts hot reload server with Vite

set -e

cd "$(dirname "$0")"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  CodeYourPCB Development Environment                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Check if WASM is built
if [ ! -f "pkg/cypcb_render_bg.wasm" ]; then
    echo "◆ WASM module not found, building..."
    ./build-wasm.sh
    echo ""
else
    echo "✓ WASM module found (pkg/cypcb_render_bg.wasm)"
fi

# Check if node_modules exists
if [ ! -d "node_modules" ]; then
    echo "◆ Installing npm dependencies..."
    npm install
    echo ""
else
    echo "✓ Node modules installed"
fi

echo ""
echo "◆ Starting development server..."
echo "  - Vite dev server (frontend)"
echo "  - WebSocket server (hot reload)"
echo "  - Watching ../examples/*.cypcb"
echo ""
echo "Open http://localhost:5173 in your browser"
echo ""

npx tsx server.ts "$@"
