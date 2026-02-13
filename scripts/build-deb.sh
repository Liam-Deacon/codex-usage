#!/bin/bash
set -e

ARCH="$1"
TARGET="$2"
VERSION="$3"

if [ -z "$ARCH" ] || [ -z "$TARGET" ] || [ -z "$VERSION" ]; then
    echo "Usage: $0 <arch> <target> <version>"
    exit 1
fi

PKG_VERSION="${VERSION#v}-1"

mkdir -p release
mkdir -p pkg/usr/bin
cp "target/$TARGET/release/codex-usage" pkg/usr/bin/

mkdir -p pkg/DEBIAN
printf 'Package: codex-usage\nVersion: %s\nSection: utils\nPriority: optional\nArchitecture: %s\nDepends: libc6 (>= 2.17)\nMaintainer: Liam Deacon <liam@deacon.dev>\nDescription: CLI tool to track OpenAI Codex usage with multi-account support\n' "$PKG_VERSION" "$ARCH" > pkg/DEBIAN/control

dpkg-deb --build pkg "release/codex-usage_${PKG_VERSION}_${ARCH}.deb"
rm -rf pkg
