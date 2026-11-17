#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
mode="${1:-renderer}"
export METRONOMO_VALIDATION_DIR="${METRONOMO_VALIDATION_DIR:-$PWD/dist/evidence}"
case "$mode" in
    renderer)
        cargo test --release --locked --offline renderer_timing_matrix_validation -- --ignored --nocapture
        cargo test --release --locked --offline renderer_thirty_minute_validation -- --ignored --nocapture
        ;;
    device)
        echo 'Playing quiet clicks on the real default output for ten minutes.'
        cargo test --release --locked --offline default_output_ten_minute_validation -- --ignored --nocapture
        ;;
    *) echo "Usage: $0 [renderer|device]" >&2; exit 2 ;;
esac
