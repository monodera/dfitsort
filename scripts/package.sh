#!/usr/bin/env bash
# Builds dist/dfitsort-VERSION-TARGET.tar.gz: the binary, dfits/fitsort symlinks and docs.
# usage: scripts/package.sh TARGET   (BUILD overrides the build command, e.g. BUILD="cargo zigbuild")
set -euo pipefail
target=${1:?usage: $0 TARGET_TRIPLE}
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
${BUILD:-cargo build} --release --locked -p dfitsort --target "$target"
version=$(cargo pkgid -p dfitsort | sed 's/.*[#@]//')
name="dfitsort-$version-$target"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$name"
cp "target/$target/release/dfitsort" "$stage/$name/"
ln -s dfitsort "$stage/$name/dfits"
ln -s dfitsort "$stage/$name/fitsort"
cp README.md LICENSE-MIT LICENSE-APACHE NOTICE "$stage/$name/"
mkdir -p dist
tar -C "$stage" -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"
