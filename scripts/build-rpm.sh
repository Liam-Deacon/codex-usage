#!/bin/bash
set -e

ARCH="$1"
TARGET="$2"
RUST_ARCH="$3"
VERSION="$4"

if [ -z "$ARCH" ] || [ -z "$TARGET" ] || [ -z "$RUST_ARCH" ] || [ -z "$VERSION" ]; then
    echo "Usage: $0 <arch> <target> <rust_arch> <version>"
    exit 1
fi

RPM_VERSION="${VERSION#v}"

mkdir -p release
mkdir -p rpmbuild/BUILD
mkdir -p rpmbuild/RPMS
mkdir -p rpmbuild/SOURCES
mkdir -p rpmbuild/SPECS
mkdir -p rpmbuild/SRPMS

mkdir -p rpmbuild/BUILD/codex-usage
cp "target/$TARGET/release/codex-usage" rpmbuild/BUILD/codex-usage/

cat > rpmbuild/SPECS/codex-usage.spec << 'SPECFILE'
Name: codex-usage
Version: __VERSION__
Release: 1
Summary: CLI tool to track OpenAI Codex usage with multi-account support
License: MIT
URL: https://github.com/Liam-Deacon/codex-usage
BuildArch: __ARCH__

%description
CLI tool to track OpenAI Codex usage with multi-account support

%install
mkdir -p %{buildroot}/usr/bin
cp %{_builddir}/codex-usage/codex-usage %{buildroot}/usr/bin/

%files
/usr/bin/codex-usage
SPECFILE

sed -i "s/__VERSION__/${RPM_VERSION}/g" rpmbuild/SPECS/codex-usage.spec
sed -i "s/__ARCH__/${RUST_ARCH}/g" rpmbuild/SPECS/codex-usage.spec

rpmbuild --target "$RUST_ARCH-redhat-linux" -bb rpmbuild/SPECS/codex-usage.spec
cp "rpmbuild/RPMS/$RUST_ARCH"/*.rpm release/
rm -rf rpmbuild
