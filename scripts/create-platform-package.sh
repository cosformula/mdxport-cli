#!/bin/bash
# Usage: ./scripts/create-platform-package.sh <platform> <binary-path>
# Example: ./scripts/create-platform-package.sh darwin-arm64 target/aarch64-apple-darwin/release/mdxport

set -euo pipefail

PLATFORM="$1"
BINARY="$2"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
PKG_NAME="@mdxport/${PLATFORM}"
OUT_DIR="npm/platforms/${PLATFORM}"

case "$PLATFORM" in
  darwin-arm64) OS=darwin; CPU=arm64 ;;
  darwin-x64)   OS=darwin; CPU=x64 ;;
  linux-x64)    OS=linux;  CPU=x64 ;;
  linux-arm64)  OS=linux;  CPU=arm64 ;;
  win32-x64)    OS=win32;  CPU=x64 ;;
  *) echo "Unknown platform: $PLATFORM"; exit 1 ;;
esac

mkdir -p "$OUT_DIR"

cat > "${OUT_DIR}/package.json" << EOF
{
  "name": "${PKG_NAME}",
  "version": "${VERSION}",
  "description": "mdxport binary for ${PLATFORM}",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/cosformula/mdxport"
  },
  "os": ["${OS}"],
  "cpu": ["${CPU}"],
  "files": [
    "mdxport*"
  ]
}
EOF

cp "$BINARY" "$OUT_DIR/"
chmod +x "$OUT_DIR/$(basename "$BINARY")"
echo "Created ${PKG_NAME}@${VERSION} → ${OUT_DIR}"
