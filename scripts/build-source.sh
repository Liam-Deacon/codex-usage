#!/bin/bash
set -e

VERSION="$1"

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

TAG="${VERSION#v}"

mkdir -p release

git archive --prefix="codex-usage-$TAG/" -o "release/codex-usage-$TAG.tar.gz" HEAD
