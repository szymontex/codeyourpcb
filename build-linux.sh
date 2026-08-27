#!/bin/bash
echo "============================================"
echo "Building CodeYourPCB Desktop Installer"
echo "============================================"
echo ""
echo "This will create production installers for Linux."
echo "Output will be in: target/release/bundle/"
echo ""
echo "NOTE: This may take 10-20 minutes on first build."
echo ""

# linuxdeploy, which Tauri downloads to build the AppImage, is itself an
# AppImage - and an AppImage mounts itself with FUSE. A container usually has
# no `/dev/fuse`, so it fails with `dlopen(): error loading libfuse.so.2` and
# Tauri reports only `failed to run linuxdeploy`, which names the symptom and
# not the cause. Measured in this project's own container on 2026-08-12: the
# same binary answers `--version` normally once this is set, because the
# variable makes an AppImage unpack itself to a temporary directory instead of
# mounting.
#
# Set only when the device node is missing, so a machine that has FUSE keeps
# the faster path.
if [ ! -e /dev/fuse ]; then
    echo "[INFO] /dev/fuse is missing, so AppImages cannot mount themselves."
    echo "       Setting APPIMAGE_EXTRACT_AND_RUN=1 for the bundler."
    export APPIMAGE_EXTRACT_AND_RUN=1
fi

# Which bundles to build. `bundle.targets` in tauri.conf.json is "all", and on
# Linux that includes the AppImage, whose bundler fails inside a vendored script
# this project does not own - `linuxdeploy-plugin-gtk.sh` lines 300-310, the
# cause read line by line and recorded in docs/TRACKER.md. Tauri reports the
# whole build as failed when one bundler does, so `.deb` and `.rpm` were being
# built and then buried under "Build FAILED".
#
# A third-party bug is not a reason to ship nothing. This builds the two
# formats that do build, and attempts the AppImage only when asked - so the day
# the upstream script is fixed, `CYPCB_APPIMAGE=1 ./build-linux.sh` says so.
BUNDLES="deb,rpm"
if [ "${CYPCB_APPIMAGE:-0}" = "1" ]; then
    BUNDLES="deb,rpm,appimage"
    echo "[INFO] CYPCB_APPIMAGE=1: the AppImage is attempted as well."
    echo ""
fi

cd viewer
if ! npm run build:desktop -- --bundles "$BUNDLES"; then
    echo ""
    echo "============================================"
    echo "Build FAILED"
    echo "============================================"
    echo ""
    if [ "${CYPCB_APPIMAGE:-0}" = "1" ]; then
        echo "The AppImage was requested, and its bundler fails inside"
        echo "linuxdeploy-plugin-gtk.sh, a vendored script this project does not"
        echo "own - the cause is recorded in docs/TRACKER.md. Tauri reports the"
        echo "whole build as failed, so anything it did produce first is listed"
        echo "here rather than left unmentioned:"
        ls -lh ../target/release/bundle/deb/*.deb \
               ../target/release/bundle/rpm/*.rpm 2>/dev/null \
          || echo "  (nothing)"
        echo ""
        echo "Run without CYPCB_APPIMAGE=1 for the two formats that do build."
    else
        echo "Nothing was installed. On a fresh Linux box the usual cause is the"
        echo "system libraries Tauri links against - run ./setup-linux.sh first."
    fi
    exit 1
fi

echo ""
echo "============================================"
echo "Build complete!"
echo "============================================"
echo ""
# `src-tauri` is a workspace member, so cargo puts its output in the workspace
# root's target directory, not in one of its own. `cargo metadata` says
# target_directory is the repo root's `target`, and `src-tauri/target` does not
# exist and never did. This script announced that path and then looked in it,
# so a build that produced two installers reported "(not created)" for both,
# directly under the words "Build complete!".
BUNDLE=../target/release/bundle

echo "Installers created:"
echo ""
MISSING=0
echo "Debian package (.deb):"
ls -lh "$BUNDLE"/deb/*.deb 2>/dev/null || { echo "  (not created)"; MISSING=1; }
echo ""
echo "RPM package (.rpm):"
ls -lh "$BUNDLE"/rpm/*.rpm 2>/dev/null || { echo "  (not created)"; MISSING=1; }
echo ""
if [ "${CYPCB_APPIMAGE:-0}" = "1" ]; then
    echo "AppImage (portable):"
    ls -lh "$BUNDLE"/appimage/*.AppImage 2>/dev/null || { echo "  (not created)"; MISSING=1; }
else
    echo "AppImage (portable): not requested - set CYPCB_APPIMAGE=1 to attempt it."
fi
echo ""

# A build that exits 0 and leaves nothing behind is not a build that worked,
# and saying so under a "Build complete!" banner is how nobody notices.
if [ "$MISSING" -ne 0 ]; then
    echo "Tauri exited successfully and at least one installer is missing from"
    echo "$BUNDLE - check the bundler's own output above."
    exit 1
fi
