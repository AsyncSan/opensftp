#!/usr/bin/env bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

echo "=== Open SFTP Release Build v${VERSION} ==="
echo "Platform: ${OS} ${ARCH}"
echo ""

mkdir -p dist

case "$OS" in
    linux)
        echo "--- Building Linux ${ARCH} (native) ---"
        cargo build --release

        # Create a tar.gz with the binary (executable, named simply)
        STAGING="dist/staging-linux"
        rm -rf "$STAGING"
        mkdir -p "$STAGING"
        cp target/release/opensftp "$STAGING/opensftp"
        chmod +x "$STAGING/opensftp"

        TARBALL="dist/OpenSFTP-${VERSION}-linux-${ARCH}.tar.gz"
        tar -czf "$TARBALL" -C "$STAGING" opensftp
        rm -rf "$STAGING"
        echo "Built: $TARBALL"
        echo "  Extract and run: tar xzf $(basename $TARBALL) && ./opensftp"
        ;;

    darwin)
        echo "--- Building macOS aarch64 (Apple Silicon) ---"
        rustup target add aarch64-apple-darwin 2>/dev/null || true
        cargo build --release --target aarch64-apple-darwin

        echo "--- Building macOS x86_64 (Intel) ---"
        rustup target add x86_64-apple-darwin 2>/dev/null || true
        cargo build --release --target x86_64-apple-darwin

        echo "--- Creating Universal Binary ---"
        lipo -create \
            target/aarch64-apple-darwin/release/opensftp \
            target/x86_64-apple-darwin/release/opensftp \
            -output dist/opensftp-universal
        chmod +x dist/opensftp-universal

        echo "--- Creating macOS .app bundle ---"
        APP="dist/Open SFTP.app"
        rm -rf "$APP"
        mkdir -p "$APP/Contents/MacOS"
        mkdir -p "$APP/Contents/Resources"

        cp dist/opensftp-universal "$APP/Contents/MacOS/opensftp"
        chmod +x "$APP/Contents/MacOS/opensftp"

        # Copy icon if available
        if [ -f assets/icon.png ]; then
            cp assets/icon.png "$APP/Contents/Resources/AppIcon.png"
        fi

        # Info.plist
        cat > "$APP/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>opensftp</string>
    <key>CFBundleIdentifier</key>
    <string>de.renemurrell.opensftp</string>
    <key>CFBundleName</key>
    <string>Open SFTP</string>
    <key>CFBundleDisplayName</key>
    <string>Open SFTP</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
</dict>
</plist>
PLIST

        # Create .zip of the .app bundle (preserves executable permissions)
        ZIP="dist/OpenSFTP-${VERSION}-macOS.zip"
        (cd dist && zip -r -y "$(basename "$ZIP")" "Open SFTP.app")
        echo "Built: $ZIP"
        echo "  Unzip and drag to Applications, or double-click to run."

        # Also create standalone binaries for direct download
        cp target/aarch64-apple-darwin/release/opensftp "dist/opensftp-${VERSION}-macos-aarch64"
        cp target/x86_64-apple-darwin/release/opensftp "dist/opensftp-${VERSION}-macos-x86_64"
        cp dist/opensftp-universal "dist/opensftp-${VERSION}-macos-universal"
        chmod +x dist/opensftp-${VERSION}-macos-*

        # Cleanup temp
        rm -f dist/opensftp-universal
        ;;

    *)
        echo "--- Building Windows x86_64 ---"
        cargo build --release
        cp target/release/opensftp.exe "dist/OpenSFTP-${VERSION}-Windows.exe"
        echo "Built: dist/OpenSFTP-${VERSION}-Windows.exe"
        echo "  Double-click to run."
        ;;
esac

echo ""
echo "=== Artifacts ==="
ls -lh dist/OpenSFTP-${VERSION}-* dist/opensftp-${VERSION}-* 2>/dev/null || echo "No artifacts found"
echo ""
echo "=== SHA256 ==="
if command -v sha256sum &>/dev/null; then
    sha256sum dist/OpenSFTP-${VERSION}-* dist/opensftp-${VERSION}-* 2>/dev/null
elif command -v shasum &>/dev/null; then
    shasum -a 256 dist/OpenSFTP-${VERSION}-* dist/opensftp-${VERSION}-* 2>/dev/null
fi
