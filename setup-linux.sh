#!/bin/bash
set -e

echo "============================================"
echo "CodeYourPCB Desktop - Linux (Ubuntu) Setup"
echo "============================================"
echo ""

# Check Node.js
echo "[1/6] Checking Node.js..."
if ! command -v node &> /dev/null; then
    echo "[ERROR] Node.js not found!"
    echo ""
    echo "Install with:"
    echo "  curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -"
    echo "  sudo apt-get install -y nodejs"
    echo ""
    exit 1
fi
echo "[OK] Node.js found: $(node --version)"

# Check Rust
echo ""
echo "[2/6] Checking Rust..."
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

# Check/Install GTK dependencies (CRITICAL for Tauri on Linux)
echo ""
echo "[3/6] Checking GTK dependencies..."
MISSING_DEPS=0

check_pkg() {
    if ! dpkg -l | grep -q "^ii  $1 "; then
        echo "[MISSING] $1"
        MISSING_DEPS=1
    fi
}

check_pkg "libwebkit2gtk-4.1-dev"
check_pkg "libgtk-3-dev"
check_pkg "libayatana-appindicator3-dev"
check_pkg "librsvg2-dev"
check_pkg "pkg-config"

if [ $MISSING_DEPS -eq 1 ]; then
    echo ""
    echo "[INFO] Installing missing GTK dependencies..."
    echo "This requires sudo password."
    echo ""
    sudo apt-get update
    sudo apt-get install -y \
        libwebkit2gtk-4.1-dev \
        build-essential \
        curl \
        wget \
        file \
        libgtk-3-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        pkg-config
fi
echo "[OK] GTK dependencies installed"

# Install npm dependencies
echo ""
echo "[4/6] Installing frontend dependencies..."
cd viewer
npm install
cd ..
echo "[OK] Frontend dependencies installed"

# Verify Tauri CLI
echo ""
echo "[5/6] Verifying Tauri CLI..."
cd viewer
if ! npm run tauri -- --version &> /dev/null; then
    echo "[WARN] Tauri CLI not working, reinstalling..."
    npm install --save-dev @tauri-apps/cli@2
fi
cd ..
echo "[OK] Tauri CLI ready"

# Build verification
echo ""
echo "[6/6] Verifying Tauri compilation..."
if [ ! -f "src-tauri/Cargo.toml" ]; then
    echo "[ERROR] Tauri project not found at src-tauri/"
    exit 1
fi

echo "[INFO] Testing compilation (this may take a few minutes)..."
cd /workspace/codeyourpcb
cargo check -p cypcb-desktop
if [ $? -eq 0 ]; then
    echo "[OK] Tauri project compiles successfully!"
else
    echo "[WARN] Compilation check failed - may need additional dependencies"
fi

echo ""
echo "============================================"
echo "Setup complete!"
echo "============================================"
echo ""
echo "To run in development mode:"
echo "  ./dev-linux.sh"
echo ""
echo "To build installer:"
echo "  ./build-linux.sh"
echo ""
