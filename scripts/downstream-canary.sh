#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

run_step() {
    echo
    echo "[downstream] $*"
    "$@"
}

RELEASE_RUNA="${RUNA_BIN:-./target/release/runa}"
TARGET_DIR="tests/downstream"
ENTRYPOINTS=(
    "tests/downstream/import_library_consumer_test.runa"
    "tests/downstream/import_stateful_consumer_test.runa"
    "tests/downstream/import_effect_consumer_test.runa"
)

run_step cargo build --release

run_step "$RELEASE_RUNA" fmt --check "$TARGET_DIR"
run_step "$RELEASE_RUNA" lint-library tests
run_step "$RELEASE_RUNA" lint-library --imports "$TARGET_DIR"

for entry in "${ENTRYPOINTS[@]}"; do
    run_step "$RELEASE_RUNA" check "$entry"
done

run_step "$RELEASE_RUNA" test --run "$TARGET_DIR"
run_step "$RELEASE_RUNA" test --check-codegen "$TARGET_DIR"
run_step "$RELEASE_RUNA" test --roundtrip "$TARGET_DIR"

echo
echo "[downstream] Authored downstream consumer fixtures passed for: ${ENTRYPOINTS[*]}"
