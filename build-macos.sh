#!/bin/bash
echo "============================================"
echo "Building CodeYourPCB Desktop Installer"
echo "============================================"
echo ""
echo "This will create a production installer for macOS."
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
    echo "Nothing was installed. Check the error above."
    exit 1
fi

echo ""
echo "============================================"
echo "Build complete!"
echo "============================================"
echo ""
echo "Installer location:"
ls -lh ../target/release/bundle/dmg/*.dmg
echo ""
