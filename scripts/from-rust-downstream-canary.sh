#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[from-rust-downstream] $*"
    "$@"
}

fail() {
    echo "[from-rust-downstream] error: $*" >&2
    exit 1
}

resolve_runa_bin() {
    local runa_bin="${RUNA_BIN:-./target/release/runa}"
    case "$runa_bin" in
        /*)
            printf '%s\n' "$runa_bin"
            ;;
        ./*)
            printf '%s/%s\n' "$ROOT_DIR" "${runa_bin#./}"
            ;;
        *)
            printf '%s\n' "$runa_bin"
            ;;
    esac
}

copy_fixture_dir() {
    local src_dir="$1"
    local dest_dir="$2"
    mkdir -p "$dest_dir"

    shopt -s nullglob
    local fixtures=("$src_dir"/*.rs)
    shopt -u nullglob

    if [[ "${#fixtures[@]}" -eq 0 ]]; then
        fail "no Rust fixtures found in $src_dir"
    fi

    local fixture
    for fixture in "${fixtures[@]}"; do
        cp -f "$fixture" "$dest_dir/$(basename "$fixture")"
    done
}

run_from_rust_lane() {
    local label="$1"
    local target_dir="$2"
    local expected_summary="$3"
    shift 3
    local output_file="$TMP_DIR/$label.out"

    echo
    echo "[from-rust-downstream] $RELEASE_RUNA from-rust --test $target_dir"
    if ! "$RELEASE_RUNA" from-rust --test "$target_dir" >"$output_file" 2>&1; then
        cat "$output_file" >&2
        fail "$label lane failed"
    fi

    cat "$output_file" >&2
    grep -Fq "$expected_summary" "$output_file" \
        || fail "$label lane did not report expected summary: $expected_summary"

    local required
    for required in "$@"; do
        grep -Fq "$required" "$output_file" \
            || fail "$label lane did not report expected marker: $required"
    done
}

RELEASE_RUNA="$(resolve_runa_bin)"
if ! command -v "$RELEASE_RUNA" >/dev/null 2>&1; then
    run_step cargo build --release
    RELEASE_RUNA="$ROOT_DIR/target/release/runa"
fi

if [[ "$RELEASE_RUNA" == "$ROOT_DIR/target/release/runa" ]] \
    && find src Cargo.toml Cargo.lock -newer "$RELEASE_RUNA" -print -quit | grep -q .; then
    run_step cargo build --release
fi

SOURCE_DIR="${FROM_RUST_DOWNSTREAM_DIR:-tests/from-rust/downstream}"
SUPPORTED_SOURCE_DIR="$SOURCE_DIR/supported"
UNSUPPORTED_SOURCE_DIR="$SOURCE_DIR/unsupported"
[[ -d "$SUPPORTED_SOURCE_DIR" ]] || fail "supported fixture dir not found: $SUPPORTED_SOURCE_DIR"
[[ -d "$UNSUPPORTED_SOURCE_DIR" ]] || fail "unsupported fixture dir not found: $UNSUPPORTED_SOURCE_DIR"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-from-rust-downstream.XXXXXX")"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

SUPPORTED_WORK_DIR="$TMP_DIR/supported"
UNSUPPORTED_WORK_DIR="$TMP_DIR/unsupported"
copy_fixture_dir "$SUPPORTED_SOURCE_DIR" "$SUPPORTED_WORK_DIR"
copy_fixture_dir "$UNSUPPORTED_SOURCE_DIR" "$UNSUPPORTED_WORK_DIR"

run_from_rust_lane supported "$SUPPORTED_WORK_DIR" "From-rust: 5 matched"
run_from_rust_lane \
    unsupported \
    "$UNSUPPORTED_WORK_DIR" \
    "5 expected-unsupported" \
    "async-threading" \
    "borrowed-return-reference" \
    "external-crate" \
    "unsafe-rust"

echo
echo "[from-rust-downstream] From-rust downstream consumer canaries passed in $TMP_DIR"
