#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_step() {
    echo
    echo "[canary] $*"
    "$@"
}

RELEASE_RUNA="${RUNA_BIN:-./target/release/runa}"
CANARY_DIR="${FUTURUNA_CANARY_DIR:-tests/canary}"

if [[ ! -x "$RELEASE_RUNA" ]]; then
    run_step cargo build --release
fi

run_step "$RELEASE_RUNA" fmt --check "$CANARY_DIR"
run_step "$RELEASE_RUNA" test --run "$CANARY_DIR"
run_step "$RELEASE_RUNA" test --check-codegen "$CANARY_DIR"
run_step "$RELEASE_RUNA" test --roundtrip "$CANARY_DIR"

echo
echo "[canary] Authored Futuruna canaries passed."
