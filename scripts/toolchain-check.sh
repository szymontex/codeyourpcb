#!/usr/bin/env bash
# Are the tools that build the browser module the versions it is pinned to?
#
# `viewer/pkg` is committed, and the gate rebuilds it and compares bytes. That
# comparison is only about the source when every tool in the build is the same
# on every machine: rustc, which compiles it; wasm-bindgen, which writes its
# bindings; and wasm-opt, which rewrites the module last. Until 2026-09-26 the
# toolchain file said `stable` and binaryen came from whatever the operating
# system shipped, so nothing said a module built on one machine would be the
# same file on the next, and the gate would read a difference there as a stale
# module rather than as a different compiler.
#
# Each version has one home:
#   rustc         `channel` in rust-toolchain.toml
#   wasm-bindgen  the `wasm-bindgen` crate in Cargo.lock - the CLI writes the
#                 bindings the crate's side expects, and the two must agree
#   wasm-opt      WASM_OPT_VERSION below, because nothing else here names it
#
# Prints each tool against its pin and exits 0 when all three agree; names
# what to install and exits 1 when one does not. Run by the gate before it
# builds anything and by `viewer/build-wasm.sh` before it compiles.
set -uo pipefail

cd "$(dirname "$0")/.."

WASM_OPT_VERSION=108

RUST_VERSION=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
BINDGEN_VERSION=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)

if [ -z "$RUST_VERSION" ] || [ -z "$BINDGEN_VERSION" ]; then
  echo "toolchain-check: could not read a pin, so nothing below is a verdict"
  echo "  rust-toolchain.toml channel : ${RUST_VERSION:-none}"
  echo "  Cargo.lock wasm-bindgen     : ${BINDGEN_VERSION:-none}"
  exit 1
fi

# rustup installs a toolchain the file names and the machine lacks the moment
# anything asks for rustc, so asking would answer by downloading one. Asked
# with that turned off, a missing toolchain is an answer.
HAVE_RUST=$(RUSTUP_AUTO_INSTALL=0 rustc --version 2>/dev/null | awk '{ print $2 }')
HAVE_BINDGEN=""
if command -v wasm-bindgen &> /dev/null; then
  HAVE_BINDGEN=$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }')
fi
HAVE_WASM_OPT=""
if command -v wasm-opt &> /dev/null; then
  HAVE_WASM_OPT=$(wasm-opt --version 2>/dev/null | awk '{ print $3 }')
fi

WRONG=""
if [ "$HAVE_RUST" != "$RUST_VERSION" ]; then
  WRONG="${WRONG}
  rustc ${HAVE_RUST:-none}, pinned ${RUST_VERSION} by rust-toolchain.toml:
      rustup toolchain install        # run in this checkout; reads the file"
fi
if [ "$HAVE_BINDGEN" != "$BINDGEN_VERSION" ]; then
  WRONG="${WRONG}
  wasm-bindgen ${HAVE_BINDGEN:-none}, pinned ${BINDGEN_VERSION} by Cargo.lock:
      cargo install wasm-bindgen-cli --version ${BINDGEN_VERSION} --locked
      cargo binstall wasm-bindgen-cli --version ${BINDGEN_VERSION}   # prebuilt, seconds"
fi
if [ "$HAVE_WASM_OPT" != "$WASM_OPT_VERSION" ]; then
  WRONG="${WRONG}
  wasm-opt ${HAVE_WASM_OPT:-none}, pinned ${WASM_OPT_VERSION} by scripts/toolchain-check.sh:
      binaryen release version_${WASM_OPT_VERSION}:
      https://github.com/WebAssembly/binaryen/releases/tag/version_${WASM_OPT_VERSION}"
fi

if [ -n "$WRONG" ]; then
  echo "toolchain-check: the browser module is built with other tools than it is pinned to:"
  echo "$WRONG"
  echo ""
  echo "  Nothing promises other versions the same module, and the gate would"
  echo "  read a difference as a stale viewer/pkg."
  exit 1
fi

echo "toolchain-check: rustc $HAVE_RUST, wasm-bindgen $HAVE_BINDGEN, wasm-opt $HAVE_WASM_OPT - as pinned"
