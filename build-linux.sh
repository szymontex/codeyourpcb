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

cd viewer
if ! npm run build:desktop; then
    echo ""
    echo "============================================"
    echo "Build FAILED"
    echo "============================================"
    echo ""
    echo "Nothing was installed. On a fresh Linux box the usual cause is the"
    echo "system libraries Tauri links against - run ./setup-linux.sh first."
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
echo "AppImage (portable):"
ls -lh "$BUNDLE"/appimage/*.AppImage 2>/dev/null || { echo "  (not created)"; MISSING=1; }
echo ""
echo "Debian package (.deb):"
ls -lh "$BUNDLE"/deb/*.deb 2>/dev/null || { echo "  (not created)"; MISSING=1; }
echo ""

# A build that exits 0 and leaves nothing behind is not a build that worked,
# and saying so under a "Build complete!" banner is how nobody notices.
if [ "$MISSING" -ne 0 ]; then
    echo "Tauri exited successfully and at least one installer is missing from"
    echo "$BUNDLE - check the bundler's own output above."
    exit 1
fi
