#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=scripts/step-runner.sh
source "$ROOT_DIR/scripts/step-runner.sh"

# With no arguments every step runs in order and the first failure stops the
# gate. CI runs the groups in parallel jobs with --keep-going; see
# docs/mint-gate.md and `./scripts/mint.sh --help`.
steps_init "[mint]" "rust checks interpreter codegen roundtrip" "$@"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

RELEASE_RUNA="./target/release/runa"

prerequisite cargo build --release
# Never inherit a stale corpus/model binary in the canonical gate. Large
# calculation CLI fixtures use the optimized compiler just built above;
# Rust unit tests and other integration helpers retain their normal profile.
export FUTURUNA_MODEL_TEST_RUNA="$ROOT_DIR/target/release/runa"
step rust cargo test --quiet
step checks ./scripts/first-run-canary.sh
step checks ./scripts/rust-interop-canary.sh
step checks ./scripts/from-rust-downstream-canary.sh
step checks ./scripts/from-rust-differential.sh
step checks ./scripts/compiler-cross-product-canary.sh
step checks "$RELEASE_RUNA" test
step interpreter "$RELEASE_RUNA" test --run
step checks "$RELEASE_RUNA" expect tests/expect
step checks "$RELEASE_RUNA" expect tests/issue-repros
step checks ./scripts/docs-oracle.sh
step codegen "$RELEASE_RUNA" test --check-codegen
step roundtrip "$RELEASE_RUNA" test --roundtrip tests
step checks "$RELEASE_RUNA" run tests/codegen_integration_regression_test.runa
step checks ./scripts/wasm-canary.sh
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-02.runa
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-03.runa
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-04.runa
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-05.runa
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-06.runa
step checks "$RELEASE_RUNA" check examples/danish-constitution-legacy/kapitel-07.runa

steps_finish
echo
if steps_running_all; then
    echo "[mint] Futuruna is mint."
else
    echo "[mint] Selected groups passed: ${STEPS_SELECTED[*]}"
fi
