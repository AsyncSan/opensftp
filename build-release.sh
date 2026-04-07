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
        cp target/release/eurosftp "dist/opensftp-${VERSION}-linux-${ARCH}"
        chmod +x "dist/opensftp-${VERSION}-linux-${ARCH}"
        echo "Built: dist/opensftp-${VERSION}-linux-${ARCH}"
        ;;
    darwin)
        echo "--- Building macOS aarch64 (Apple Silicon) ---"
        rustup target add aarch64-apple-darwin 2>/dev/null || true
        cargo build --release --target aarch64-apple-darwin
        cp target/aarch64-apple-darwin/release/eurosftp "dist/opensftp-${VERSION}-macos-aarch64"
        chmod +x "dist/opensftp-${VERSION}-macos-aarch64"

        echo "--- Building macOS x86_64 (Intel) ---"
        rustup target add x86_64-apple-darwin 2>/dev/null || true
        cargo build --release --target x86_64-apple-darwin
        cp target/x86_64-apple-darwin/release/eurosftp "dist/opensftp-${VERSION}-macos-x86_64"
        chmod +x "dist/opensftp-${VERSION}-macos-x86_64"

        echo "--- Creating Universal Binary ---"
        lipo -create \
            "dist/opensftp-${VERSION}-macos-aarch64" \
            "dist/opensftp-${VERSION}-macos-x86_64" \
            -output "dist/opensftp-${VERSION}-macos-universal"
        chmod +x "dist/opensftp-${VERSION}-macos-universal"
        echo "Built: dist/opensftp-${VERSION}-macos-universal"
        ;;
    *)
        echo "--- Building Windows x86_64 ---"
        cargo build --release
        cp target/release/eurosftp.exe "dist/opensftp-${VERSION}-windows-x86_64.exe"
        echo "Built: dist/opensftp-${VERSION}-windows-x86_64.exe"
        ;;
esac

echo ""
echo "=== Artifacts ==="
ls -lh dist/opensftp-${VERSION}-* 2>/dev/null || echo "No artifacts found"
echo ""
echo "=== SHA256 ==="
if command -v sha256sum &>/dev/null; then
    sha256sum dist/opensftp-${VERSION}-*
elif command -v shasum &>/dev/null; then
    shasum -a 256 dist/opensftp-${VERSION}-*
fi
