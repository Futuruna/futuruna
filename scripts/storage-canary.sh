#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

run_step() {
    echo
    echo "[storage-canary] $*"
    "$@"
}

capture_in_workdir() {
    local work_dir="$1"
    shift
    echo
    echo "[storage-canary] (cd $work_dir && $*)"
    local output
    output="$(cd "$work_dir" && "$@" 2>&1)"
    printf '%s\n' "$output"
}

assert_contains() {
    local haystack="$1"
    local needle="$2"
    if [[ "$haystack" != *"$needle"* ]]; then
        echo "[storage-canary] expected output to contain: $needle" >&2
        exit 1
    fi
}

assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    if [[ "$haystack" == *"$needle"* ]]; then
        echo "[storage-canary] expected output not to contain: $needle" >&2
        exit 1
    fi
}

RELEASE_RUNA="${RUNA_BIN:-./target/release/runa}"
if [[ "$RELEASE_RUNA" = /* ]]; then
    RUNA_BIN_ABS="$RELEASE_RUNA"
else
    RUNA_BIN_ABS="$ROOT_DIR/$RELEASE_RUNA"
fi
TARGET_DIR="tests/canary/storage"
COMMIT_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_tx_commit_savepoint_test.runa"
ROLLBACK_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_tx_rollback_fail_test.runa"
ROLLBACK_CHECK_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_tx_rollback_check_test.runa"
MIGRATE_CHAIN_SEED_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_chain_seed_v1.runa"
MIGRATE_CHAIN_TEST_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_chain_v3_test.runa"
MIGRATE_AUTO_SEED_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_auto_seed_v1.runa"
MIGRATE_AUTO_TEST_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_auto_v2_test.runa"
MIGRATE_UNSAFE_SEED_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_unsafe_seed_v1.runa"
MIGRATE_UNSAFE_TEST_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_unsafe_missing_test.runa"
MIGRATE_TYPE_SEED_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_type_seed_v1.runa"
MIGRATE_TYPE_TEST_FIXTURE="$ROOT_DIR/$TARGET_DIR/persist_migration_type_narrow_missing_test.runa"
WORK_DIR="${FUTURUNA_STORAGE_CANARY_WORKDIR:-$(mktemp -d "${TMPDIR:-/tmp}/futuruna-storage-canary.XXXXXX")}"

if [[ -z "${FUTURUNA_STORAGE_CANARY_WORKDIR:-}" ]]; then
    trap 'rm -rf "$WORK_DIR"' EXIT
else
    mkdir -p "$WORK_DIR"
fi

if [[ ! -x "$RELEASE_RUNA" ]] || find src Cargo.toml Cargo.lock -newer "$RELEASE_RUNA" -print -quit | grep -q .; then
    run_step cargo build --release
fi

run_step "$RELEASE_RUNA" fmt --check "$TARGET_DIR"

commit_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$COMMIT_FIXTURE")"
printf '%s\n' "$commit_output"
assert_contains "$commit_output" "committed=inner,outer"
assert_contains "$commit_output" "qtys=[10, 20]"

echo
echo "[storage-canary] expecting rollback fixture to fail"
set +e
rollback_output="$(cd "$WORK_DIR" && "$RUNA_BIN_ABS" run "$ROLLBACK_FIXTURE" 2>&1)"
rollback_status=$?
set -e
printf '%s\n' "$rollback_output"
if [[ "$rollback_status" -eq 0 ]]; then
    echo "[storage-canary] rollback fixture unexpectedly passed" >&2
    exit 1
fi
assert_contains "$rollback_output" "? rollback_guard FAILED"

check_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$ROLLBACK_CHECK_FIXTURE")"
printf '%s\n' "$check_output"
assert_contains "$check_output" "after_rollback=baseline"
assert_contains "$check_output" "after_rollback_qtys=[1]"
assert_not_contains "$check_output" "rolledback"

migrate_seed_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_CHAIN_SEED_FIXTURE")"
printf '%s\n' "$migrate_seed_output"
assert_contains "$migrate_seed_output" "migration_seed_v1=ok"

migrate_chain_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_CHAIN_TEST_FIXTURE")"
printf '%s\n' "$migrate_chain_output"
assert_contains "$migrate_chain_output" "migration_lanes=[cold, cold]"
assert_contains "$migrate_chain_output" "migration_qtys=[10, 10]"

migrate_auto_seed_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_AUTO_SEED_FIXTURE")"
printf '%s\n' "$migrate_auto_seed_output"
assert_contains "$migrate_auto_seed_output" "migration_auto_seed_v1=ok"

migrate_auto_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_AUTO_TEST_FIXTURE")"
printf '%s\n' "$migrate_auto_output"
assert_contains "$migrate_auto_output" "migration_auto_qtys=[0, 0]"

migrate_type_seed_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_TYPE_SEED_FIXTURE")"
printf '%s\n' "$migrate_type_seed_output"
assert_contains "$migrate_type_seed_output" "migration_type_seed_v1=ok"

echo
echo "[storage-canary] expecting type-change migration fixture to fail"
set +e
migrate_type_output="$(cd "$WORK_DIR" && "$RUNA_BIN_ABS" run "$MIGRATE_TYPE_TEST_FIXTURE" 2>&1)"
migrate_type_status=$?
set -e
printf '%s\n' "$migrate_type_output"
if [[ "$migrate_type_status" -eq 0 ]]; then
    echo "[storage-canary] type-change migration fixture unexpectedly passed" >&2
    exit 1
fi
assert_contains "$migrate_type_output" "stored SQL types differ"

migrate_unsafe_seed_output="$(capture_in_workdir "$WORK_DIR" "$RUNA_BIN_ABS" run "$MIGRATE_UNSAFE_SEED_FIXTURE")"
printf '%s\n' "$migrate_unsafe_seed_output"
assert_contains "$migrate_unsafe_seed_output" "migration_unsafe_seed_v1=ok"

echo
echo "[storage-canary] expecting unsafe migration fixture to fail"
set +e
migrate_unsafe_output="$(cd "$WORK_DIR" && "$RUNA_BIN_ABS" run "$MIGRATE_UNSAFE_TEST_FIXTURE" 2>&1)"
migrate_unsafe_status=$?
set -e
printf '%s\n' "$migrate_unsafe_output"
if [[ "$migrate_unsafe_status" -eq 0 ]]; then
    echo "[storage-canary] unsafe migration fixture unexpectedly passed" >&2
    exit 1
fi
assert_contains "$migrate_unsafe_output" "requires unsafe"

echo
echo "[storage-canary] Persisted transaction runtime canaries passed offline in $WORK_DIR"
