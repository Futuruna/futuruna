#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[mint] $*"
    "$@"
}

RELEASE_RUNA="./target/release/runa"

run_step cargo test --quiet
run_step cargo build --release
run_step ./scripts/first-run-canary.sh
run_step ./scripts/rust-interop-canary.sh
run_step ./scripts/from-rust-downstream-canary.sh
run_step ./scripts/from-rust-differential.sh
run_step "$RELEASE_RUNA" test
run_step "$RELEASE_RUNA" test --run
run_step "$RELEASE_RUNA" expect tests/expect
run_step "$RELEASE_RUNA" test --check-codegen
run_step "$RELEASE_RUNA" test --roundtrip tests
run_step "$RELEASE_RUNA" run tests/codegen_integration_regression_test.runa
run_step ./scripts/storage-canary.sh
run_step ./scripts/wasm-canary.sh
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-02.runa
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-03.runa
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-04.runa
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-05.runa
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-06.runa
run_step "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-07.runa

echo
echo "[mint] Futuruna is mint."
