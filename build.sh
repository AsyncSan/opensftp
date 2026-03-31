#!/usr/bin/env bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
NAME=$(grep '^name' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64)  ARCH="x86_64" ;;
    aarch64) ARCH="aarch64" ;;
    armv7l)  ARCH="armv7" ;;
esac

case "$OS" in
    linux)  EXT="" ;;
    darwin) EXT="" ;;
    *)      EXT=".exe" ;;
esac

ARTIFACT="${NAME}-${VERSION}-${OS}-${ARCH}${EXT}"
OUT_DIR="dist"

echo "Building ${NAME} v${VERSION} for ${OS}-${ARCH}..."
echo ""

cargo build --release

mkdir -p "${OUT_DIR}"
cp "target/release/${NAME}${EXT}" "${OUT_DIR}/${ARTIFACT}"

SIZE=$(du -h "${OUT_DIR}/${ARTIFACT}" | cut -f1)

echo ""
echo "Done: ${OUT_DIR}/${ARTIFACT} (${SIZE})"
