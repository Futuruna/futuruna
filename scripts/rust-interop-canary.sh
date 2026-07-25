#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[rust-interop] $*"
    "$@"
}

fail() {
    echo "[rust-interop] error: $*" >&2
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

RELEASE_RUNA="$(resolve_runa_bin)"
if ! command -v "$RELEASE_RUNA" >/dev/null 2>&1; then
    run_step cargo build --release
    RELEASE_RUNA="$ROOT_DIR/target/release/runa"
fi

if [[ "$RELEASE_RUNA" == "$ROOT_DIR/target/release/runa" ]] \
    && find src Cargo.toml Cargo.lock -newer "$RELEASE_RUNA" -print -quit | grep -q .; then
    run_step cargo build --release
fi

RUSTC_BIN="${RUSTC_BIN:-rustc}"
if ! command -v "$RUSTC_BIN" >/dev/null 2>&1; then
    fail "rustc not found"
fi

FIXTURE="${1:-tests/canary/interop/rust_consumer_lib.runa}"
[[ -f "$FIXTURE" ]] || fail "fixture not found: $FIXTURE"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-rust-interop.XXXXXX")"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

run_step "$RELEASE_RUNA" fmt --check "$FIXTURE"

echo
echo "[rust-interop] $RELEASE_RUNA lib $FIXTURE > $TMP_DIR/futuruna_lib.rs"
"$RELEASE_RUNA" lib "$FIXTURE" > "$TMP_DIR/futuruna_lib.rs"

cat > "$TMP_DIR/consumer.rs" <<'RS'
mod futuruna_lib;

use futuruna_lib::{Mode, Packet};

fn main() {
    let packet = futuruna_lib::make_packet(7, "mint".to_string());
    assert_eq!(packet.id, 7);
    assert_eq!(packet.label, "mint");
    assert_eq!(futuruna_lib::packet_label(&packet), "mint".to_string());

    let direct = Packet {
        id: 8,
        label: "direct".to_string(),
    };
    assert_eq!(futuruna_lib::packet_label(&direct), "direct".to_string());

    assert_eq!(futuruna_lib::mode_name(Mode::Fast), "fast".to_string());
    assert_eq!(futuruna_lib::mode_name(Mode::Slow), "slow".to_string());

    let scores = vec![1, 2, 3, 4];
    assert_eq!(futuruna_lib::sum_scores(&scores), 10);

    let text = "alpha beta".to_string();
    assert_eq!(
        futuruna_lib::split_words(&text),
        vec!["alpha".to_string(), "beta".to_string()]
    );

    assert_eq!(futuruna_lib::maybe_label(true), Some("yes".to_string()));
    assert_eq!(futuruna_lib::maybe_label(false), None);
    assert_eq!(futuruna_lib::result_label(true), Ok("ok".to_string()));
    assert_eq!(futuruna_lib::result_label(false), Err("bad".to_string()));

    println!("rust interop canary passed");
}
RS

run_step "$RUSTC_BIN" --edition=2021 "$TMP_DIR/consumer.rs" -o "$TMP_DIR/consumer"

echo
echo "[rust-interop] $TMP_DIR/consumer"
"$TMP_DIR/consumer" | tee "$TMP_DIR/consumer.out"
grep -Fxq "rust interop canary passed" "$TMP_DIR/consumer.out" \
    || fail "Rust consumer did not print the expected success line"

echo
echo "[rust-interop] Rust-facing Futuruna library canary passed for: $FIXTURE"
