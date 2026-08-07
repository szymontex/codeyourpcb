#!/bin/bash
echo "============================================"
echo "Building CodeYourPCB Desktop Installer"
echo "============================================"
echo ""
echo "This will create production installers for Linux."
echo "Output will be in: src-tauri/target/release/bundle/"
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
echo "Installers created:"
echo ""
echo "AppImage (portable):"
ls -lh ../src-tauri/target/release/bundle/appimage/*.AppImage 2>/dev/null || echo "  (not created)"
echo ""
echo "Debian package (.deb):"
ls -lh ../src-tauri/target/release/bundle/deb/*.deb 2>/dev/null || echo "  (not created)"
echo ""
