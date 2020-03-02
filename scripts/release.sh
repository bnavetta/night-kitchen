#!/bin/bash

set -eou pipefail
cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
    echo "Working directory is not clean!"
    exit 1
fi

version=$(cargo read-manifest | jq -r '.version')

echo "Releasing v$version"

sed -i "s/^pkgver=.*$/pkgver='$version'/" packaging/arch/PKGBUILD

git add .
git commit -m "Release v$version"
git tag "v$version"
git push --tags
# This will kick off the GitHub workflow to create a release
