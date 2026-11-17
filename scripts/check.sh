#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo fmt --all -- --check
cargo check --all-targets --locked --offline
cargo test --locked --offline
cargo clippy --all-targets --all-features --locked --offline -- -D warnings
desktop-file-validate packaging/linux/desktop/metronomo.desktop
python3 -m unittest discover -s scripts/release/tests -v
