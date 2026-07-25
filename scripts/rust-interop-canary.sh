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

CARGO_BIN="${CARGO_BIN:-cargo}"
if ! command -v "$CARGO_BIN" >/dev/null 2>&1; then
    fail "cargo not found"
fi

BASIC_FIXTURE="${1:-tests/canary/interop/rust_consumer_lib.runa}"
EXTERNAL_FIXTURE="${RUST_INTEROP_EXTERNAL_FIXTURE:-tests/canary/interop/rust_consumer_external_crate_lib.runa}"
[[ -f "$BASIC_FIXTURE" ]] || fail "fixture not found: $BASIC_FIXTURE"
[[ -f "$EXTERNAL_FIXTURE" ]] || fail "fixture not found: $EXTERNAL_FIXTURE"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-rust-interop.XXXXXX")"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

run_basic_consumer_canary() {
    local fixture="$1"
    local work_dir="$TMP_DIR/basic"
    mkdir -p "$work_dir"

    run_step "$RELEASE_RUNA" fmt --check "$fixture"

    echo
    echo "[rust-interop] $RELEASE_RUNA lib $fixture > $work_dir/futuruna_lib.rs"
    "$RELEASE_RUNA" lib "$fixture" > "$work_dir/futuruna_lib.rs"

    cat > "$work_dir/consumer.rs" <<'RS'
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

    run_step "$RUSTC_BIN" --edition=2021 "$work_dir/consumer.rs" -o "$work_dir/consumer"

    echo
    echo "[rust-interop] $work_dir/consumer"
    "$work_dir/consumer" | tee "$work_dir/consumer.out"
    grep -Fxq "rust interop canary passed" "$work_dir/consumer.out" \
        || fail "Rust consumer did not print the expected success line"

    echo
    echo "[rust-interop] Rust-facing Futuruna library canary passed for: $fixture"
}

run_external_crate_consumer_canary() {
    local fixture="$1"
    local work_dir="$TMP_DIR/external"
    mkdir -p "$work_dir/src"

    run_step "$RELEASE_RUNA" fmt --check "$fixture"

    echo
    echo "[rust-interop] $RELEASE_RUNA lib $fixture > $work_dir/src/futuruna_lib.rs"
    "$RELEASE_RUNA" lib "$fixture" > "$work_dir/src/futuruna_lib.rs"

    cat > "$work_dir/Cargo.toml" <<'TOML'
[package]
name = "futuruna-rust-interop-external-canary"
version = "0.1.0"
edition = "2021"

[dependencies]
regex = "1"
TOML

    cat > "$work_dir/src/main.rs" <<'RS'
mod futuruna_lib;

fn main() {
    let probe = futuruna_lib::make_pattern_probe("\\d+".to_string(), "A12 B007 C".to_string());
    assert_eq!(probe.pattern, "\\d+");
    assert_eq!(probe.text, "A12 B007 C");

    assert_eq!(futuruna_lib::external_match_count(&probe), 2);
    assert_eq!(
        futuruna_lib::external_first_match(&probe),
        Some("12".to_string())
    );
    assert_eq!(
        futuruna_lib::external_replace_all(&probe, "#".to_string()),
        "A# B# C".to_string()
    );
    assert_eq!(
        futuruna_lib::external_builtin_matches("AA 12 BB 007".to_string()),
        vec!["AA".to_string(), "BB".to_string()]
    );
    assert_eq!(
        futuruna_lib::classify_external(&probe.pattern, &probe.text),
        "matched:2".to_string()
    );

    println!("rust external crate interop canary passed");
}
RS

    echo
    echo "[rust-interop] (cd $work_dir && CARGO_NET_OFFLINE=true $CARGO_BIN run --release --quiet)"
    (
        cd "$work_dir"
        CARGO_NET_OFFLINE=true "$CARGO_BIN" run --release --quiet
    ) | tee "$work_dir/consumer.out"
    grep -Fxq "rust external crate interop canary passed" "$work_dir/consumer.out" \
        || fail "Rust external crate consumer did not print the expected success line"

    echo
    echo "[rust-interop] Rust external-crate Futuruna library canary passed for: $fixture"
}

run_package_crate_consumer_canary() {
    local fixture="$1"
    local work_dir="$TMP_DIR/package"
    local lib_dir="$work_dir/futuruna_generated"
    local app_dir="$work_dir/package_consumer"
    mkdir -p "$lib_dir/src" "$app_dir/src"

    run_step "$RELEASE_RUNA" fmt --check "$fixture"

    echo
    echo "[rust-interop] $RELEASE_RUNA lib $fixture > $lib_dir/src/lib.rs"
    "$RELEASE_RUNA" lib "$fixture" > "$lib_dir/src/lib.rs"

    cat > "$lib_dir/Cargo.toml" <<'TOML'
[package]
name = "futuruna_generated"
version = "0.1.0"
edition = "2021"

[dependencies]
regex = "1"
TOML

    cat > "$app_dir/Cargo.toml" <<'TOML'
[package]
name = "futuruna-rust-interop-package-consumer"
version = "0.1.0"
edition = "2021"

[dependencies]
futuruna_generated = { path = "../futuruna_generated" }
TOML

    cat > "$app_dir/src/main.rs" <<'RS'
use futuruna_generated::PatternProbe;

fn main() {
    let probe = futuruna_generated::make_pattern_probe("\\d+".to_string(), "A12 B007 C".to_string());
    assert_eq!(futuruna_generated::external_match_count(&probe), 2);
    assert_eq!(
        futuruna_generated::external_first_match(&probe),
        Some("12".to_string())
    );

    let direct = PatternProbe {
        pattern: "[A-Z]+".to_string(),
        text: "AA bb CC".to_string(),
    };
    assert_eq!(futuruna_generated::external_match_count(&direct), 2);
    assert_eq!(
        futuruna_generated::external_replace_all(&direct, "X".to_string()),
        "X bb X".to_string()
    );
    assert_eq!(
        futuruna_generated::classify_external(&probe.pattern, &probe.text),
        "matched:2".to_string()
    );

    println!("rust package crate interop canary passed");
}
RS

    echo
    echo "[rust-interop] (cd $app_dir && CARGO_NET_OFFLINE=true $CARGO_BIN run --release --quiet)"
    (
        cd "$app_dir"
        CARGO_NET_OFFLINE=true "$CARGO_BIN" run --release --quiet
    ) | tee "$app_dir/consumer.out"
    grep -Fxq "rust package crate interop canary passed" "$app_dir/consumer.out" \
        || fail "Rust package crate consumer did not print the expected success line"

    echo
    echo "[rust-interop] Rust package-layout Futuruna library canary passed for: $fixture"
}

run_basic_consumer_canary "$BASIC_FIXTURE"
run_external_crate_consumer_canary "$EXTERNAL_FIXTURE"
run_package_crate_consumer_canary "$EXTERNAL_FIXTURE"
