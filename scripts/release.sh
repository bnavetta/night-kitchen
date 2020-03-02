#!/bin/bash

set -eou pipefail
cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
    echo "Working directory is not clean!" > &2
    exit 1
fi

if [ $# -ne 2 ]; then
    echo "Usage: $0 <version>" > &2
    exit 1
fi

version="$1"

echo "Releasing v$version"

sed -i "s/^pkgver=.*/pkgver='$version'/" packaging/arch/PKGBUILD
cargo bump "$version"

git add .
git commit -m "Release v$version"
git tag "v$version"
git push --tags
# This will kick off the GitHub workflow to create a release
