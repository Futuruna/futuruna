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

resolve_target() {
    case "$1" in
        all)
            echo "tests/canary"
            ;;
        core|stateful|extended|regressions)
            echo "tests/canary/$1"
            ;;
        *)
            echo "$1"
            ;;
    esac
}

if [[ ! -x "$RELEASE_RUNA" ]]; then
    run_step cargo build --release
fi

RAW_TARGETS=("$@")
if [[ ${#RAW_TARGETS[@]} -eq 0 ]]; then
    RAW_TARGETS=(core stateful extended regressions)
fi

TARGETS=()
for raw_target in "${RAW_TARGETS[@]}"; do
    target="$(resolve_target "$raw_target")"
    if [[ -d "$target" ]] && find "$target" -type f -name '*.runa' -print -quit | grep -q .; then
        TARGETS+=("$target")
    fi
done

if [[ ${#TARGETS[@]} -eq 0 ]]; then
    echo "[canary] no canary targets matched" >&2
    exit 1
fi

for target in "${TARGETS[@]}"; do
    run_step "$RELEASE_RUNA" fmt --check "$target"
    run_step "$RELEASE_RUNA" test --run "$target"
    run_step "$RELEASE_RUNA" test --check-codegen "$target"
    run_step "$RELEASE_RUNA" test --roundtrip "$target"
done

echo
echo "[canary] Authored Futuruna canaries passed for: ${TARGETS[*]}"
