#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_step() {
    echo
    echo "[differential] $*"
    "$@"
}

RUNA_BIN="${RUNA_BIN:-./target/release/runa}"
STRESS_COUNT="${FUTURUNA_STRESS_COUNT:-64}"
SEEDS_FILE="${FUTURUNA_STRESS_SEEDS_FILE:-tests/differential/stress_gen_seeds.txt}"
CORPUS_DIR="${FUTURUNA_DIFFERENTIAL_CORPUS:-tests/differential/corpus}"
OUT_DIR="${FUTURUNA_DIFFERENTIAL_OUT:-${TMPDIR:-/tmp}/futuruna-differential}"

if [[ ! -x "$RUNA_BIN" ]]; then
    run_step cargo build --release
fi

if [[ ! -f "$SEEDS_FILE" ]]; then
    echo "[differential] missing seeds file: $SEEDS_FILE" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"

if [[ -d "$CORPUS_DIR" ]] && find "$CORPUS_DIR" -type f -name '*.runa' -print -quit | grep -q .; then
    run_step "$RUNA_BIN" test --roundtrip "$CORPUS_DIR"
else
    echo "[differential] no differential corpus cases in $CORPUS_DIR"
fi

while IFS= read -r raw_seed || [[ -n "$raw_seed" ]]; do
    seed="${raw_seed%%#*}"
    seed="$(printf '%s' "$seed" | tr -d '[:space:]')"
    if [[ -z "$seed" ]]; then
        continue
    fi
    run_step "$RUNA_BIN" stress-gen "$STRESS_COUNT" --seed "$seed" --save-failures "$OUT_DIR"
done < "$SEEDS_FILE"

echo
echo "[differential] completed. Failure artifacts, if any, are in $OUT_DIR"
