#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
source /etc/os-release
if [[ "$ID" != ubuntu || "$VERSION_ID" != 24.04 || "$(dpkg --print-architecture)" != amd64 ]]; then
    echo 'Build the release package in Ubuntu 24.04 amd64; see docs/release.md.' >&2
    exit 1
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target/ubuntu-24.04}"
mkdir -p dist/build target
build_area="$(mktemp -d "$PWD/target/deb-build.XXXXXX")"
trap 'rm -rf "$build_area"' EXIT
python3 scripts/release/stage_package_source.py "$build_area/source"
(
    cd "$build_area/source"
    dpkg-buildpackage --build=binary --no-sign
)
for artifact in "$build_area"/metronomo_0.1.0-1_amd64.{deb,buildinfo,changes}; do
    mv "$artifact" dist/build/
done
python3 scripts/release/prepare-release.py --package dist/build/metronomo_0.1.0-1_amd64.deb
