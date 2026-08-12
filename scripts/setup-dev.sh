#!/bin/bash
set -e

# Everything `./scripts/quality-gate.sh` and `viewer/build-wasm.sh` need on a
# machine that has never run them.
#
# This exists because the answer used to be "somebody ran a command once".
# Chromium's system libraries were installed into a container's filesystem by
# hand, so the next `docker recreate` took stage 6 of the gate with it;
# `wasm-opt` was optional until the wasm build started requiring it, and
# `wasm-bindgen` has to match the version the workspace pins or the module it
# writes will not load. None of that was written down anywhere a person could
# run.
#
#   ./scripts/setup-dev.sh
#
# Safe to run again: every step checks before it installs.

cd "$(dirname "$0")/.."

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo &> /dev/null; then
        SUDO="sudo"
    else
        echo "[WARN] Not root and no sudo: the apt steps will be skipped."
        echo "       Run this as root, or install binaryen yourself."
    fi
fi

echo "============================================"
echo "CodeYourPCB - development environment"
echo "============================================"

# ---------------------------------------------------------------- rust ------
echo ""
echo "[1/6] Rust toolchain"
if ! command -v cargo &> /dev/null; then
    echo "[ERROR] cargo not found. Install Rust: https://rustup.rs"
    exit 1
fi
echo "[OK] $(cargo --version)"

if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "  adding wasm32-unknown-unknown"
    rustup target add wasm32-unknown-unknown
fi
echo "[OK] wasm32-unknown-unknown"

# ------------------------------------------------------------ wasm-bindgen --
# The CLI writes the JavaScript that loads the module, and a version that does
# not match the `wasm-bindgen` crate in Cargo.lock produces bindings the module
# rejects at load time.
echo ""
echo "[2/6] wasm-bindgen CLI"
PINNED=$(grep -A 1 '^name = "wasm-bindgen"$' Cargo.lock | grep '^version' | head -1 | cut -d'"' -f2)
if [ -z "$PINNED" ]; then
    echo "[ERROR] Could not read the wasm-bindgen version from Cargo.lock"
    exit 1
fi
HAVE=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [ "$HAVE" != "$PINNED" ]; then
    echo "  installing wasm-bindgen-cli $PINNED (have: ${HAVE:-none})"
    cargo install wasm-bindgen-cli --version "$PINNED" --locked
fi
echo "[OK] wasm-bindgen $(wasm-bindgen --version | awk '{print $2}') matches Cargo.lock"

# ---------------------------------------------------------------- binaryen --
# `viewer/build-wasm.sh` optimizes the module itself and refuses to run without
# this. It used to warn and carry on, which shipped a module a third larger.
echo ""
echo "[3/6] binaryen (wasm-opt)"
if ! command -v wasm-opt &> /dev/null; then
    if [ -n "$SUDO" ] || [ "$(id -u)" -eq 0 ]; then
        $SUDO apt-get update -qq
        $SUDO apt-get install -y binaryen
    else
        echo "[ERROR] wasm-opt missing and cannot install it. apt-get install binaryen"
        exit 1
    fi
fi
echo "[OK] $(wasm-opt --version)"

# ------------------------------------------------------------------ node ----
echo ""
echo "[4/6] Viewer dependencies"
if ! command -v node &> /dev/null; then
    echo "[ERROR] node not found. The viewer, its tests and the gate all need it."
    exit 1
fi
(cd viewer && npm install --no-audit --no-fund)
echo "[OK] node $(node --version), viewer dependencies installed"

# ------------------------------------------------------------- tauri deps --
# `cypcb-desktop` links GTK and WebKit through pkg-config. Without these the
# crate does not compile at all - the first error is "The pkg-config command
# could not be found", before anything GTK is even looked for - and the gate
# used to route around that by excluding the crate from clippy and from the
# test run. It stopped excluding it on 2026-08-12, so these are now the gate's
# dependencies rather than the desktop build's alone.
#
# The list is what was installed to make the crate compile on Ubuntu 24.04. It
# is proven sufficient; whether every entry is necessary is not measured, and
# `setup-linux.sh` carries the same set minus libxdo-dev and libssl-dev.
echo ""
echo "[5/6] Desktop (Tauri) system libraries"
if pkg-config --exists gtk+-3.0 webkit2gtk-4.1 2>/dev/null; then
    echo "[OK] GTK 3 and WebKit2GTK 4.1 are present"
elif [ -n "$SUDO" ] || [ "$(id -u)" -eq 0 ]; then
    $SUDO apt-get update -qq
    $SUDO apt-get install -y \
        pkg-config \
        libwebkit2gtk-4.1-dev \
        libgtk-3-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libxdo-dev \
        libssl-dev
    echo "[OK] Tauri system libraries installed"
else
    echo "[ERROR] cypcb-desktop needs GTK and WebKit and this cannot install them."
    echo "        apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \\"
    echo "          libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev"
    exit 1
fi

# ------------------------------------------------------------ playwright ----
# Stage 6 of the gate. `install-deps` is the apt half - chromium needs
# libnspr4 and friends - and `install chromium` is the browser itself.
echo ""
echo "[6/6] Playwright chromium"
if [ -n "$SUDO" ] || [ "$(id -u)" -eq 0 ]; then
    (cd viewer && $SUDO npx --yes playwright install-deps chromium)
else
    echo "[WARN] Skipping playwright system libraries: needs root."
fi
(cd viewer && npx --yes playwright install chromium)
echo "[OK] chromium installed"

# --------------------------------------------------------------- ownership --
# Running this as root - which the apt steps above want - leaves npm's cache
# owned by root, and the next `npm install` as a normal user stops with "Your
# cache folder contains root-owned files". Measured on 2026-08-08 in this
# project's container: `npm install -D eslint-plugin-playwright` as the
# development user failed exactly that way after an earlier root run.
if [ "$(id -u)" -eq 0 ]; then
    OWNER=$(stat -c '%u:%g' .)
    NPM_CACHE=$(npm config get cache 2>/dev/null)
    if [ -n "$NPM_CACHE" ] && [ -d "$NPM_CACHE" ]; then
        chown -R "$OWNER" "$NPM_CACHE"
    fi
    [ -d viewer/node_modules ] && chown -R "$OWNER" viewer/node_modules
    echo "[OK] npm cache and node_modules belong to $OWNER, not root"
fi

echo ""
echo "============================================"
echo "Done. Now:"
echo "  ./scripts/quality-gate.sh     all eight stages"
echo "  ./viewer/build-wasm.sh        rebuild the browser module"
echo "============================================"
