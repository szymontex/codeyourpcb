#!/bin/bash
set -e

echo "============================================"
echo "CodeYourPCB Desktop - macOS Setup"
echo "============================================"
echo ""

# Check Node.js
echo "[1/5] Checking Node.js..."
if ! command -v node &> /dev/null; then
    echo "[ERROR] Node.js not found!"
    echo ""
    echo "Install options:"
    echo "1. Homebrew: brew install node"
    echo "2. Download from: https://nodejs.org/"
    echo ""
    exit 1
fi
echo "[OK] Node.js found: $(node --version)"

# Check Rust
echo ""
echo "[2/5] Checking Rust..."
if ! command -v cargo &> /dev/null; then
    echo "[WARN] Rust not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    if ! command -v cargo &> /dev/null; then
        echo "[ERROR] Rust installation failed!"
        exit 1
    fi
fi
echo "[OK] Rust found: $(cargo --version)"

# Install npm dependencies
echo ""
echo "[3/5] Installing frontend dependencies..."
cd viewer
npm install
cd ..
echo "[OK] Frontend dependencies installed"

# Verify Tauri CLI
echo ""
echo "[4/5] Verifying Tauri CLI..."
cd viewer
if ! npm run tauri -- --version &> /dev/null; then
    echo "[WARN] Tauri CLI not working, reinstalling..."
    npm install --save-dev @tauri-apps/cli@2
fi
cd ..
echo "[OK] Tauri CLI ready"

# Build verification
echo ""
echo "[5/5] Verifying project structure..."
if [ ! -f "src-tauri/Cargo.toml" ]; then
    echo "[ERROR] Tauri project not found at src-tauri/"
    exit 1
fi
echo "[OK] Tauri project structure verified"

echo ""
echo "============================================"
echo "Setup complete!"
echo "============================================"
echo ""
echo "To run in development mode:"
echo "  ./dev-macos.sh"
echo ""
echo "To build installer:"
echo "  ./build-macos.sh"
echo ""
