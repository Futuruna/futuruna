#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_step() {
    echo
    echo "[wasm-canary] $*"
    "$@"
}

find_wasm_pack() {
    if command -v wasm-pack >/dev/null 2>&1; then
        command -v wasm-pack
        return 0
    fi

    local cargo_wasm_pack="${HOME:-}/.cargo/bin/wasm-pack"
    if [[ -x "$cargo_wasm_pack" ]]; then
        echo "$cargo_wasm_pack"
        return 0
    fi

    return 1
}

RELEASE_RUNA="${RUNA_BIN:-./target/release/runa}"

if ! WASM_PACK="$(find_wasm_pack)"; then
    if [[ "${FUTURUNA_WASM_CANARY_REQUIRED:-0}" == "1" ]]; then
        echo "[wasm-canary] FAIL: wasm-pack not found" >&2
        echo "[wasm-canary] Install with: cargo install wasm-pack" >&2
        exit 1
    fi
    echo "[wasm-canary] SKIP: wasm-pack not found; install with: cargo install wasm-pack"
    exit 0
fi

export PATH="$(dirname "$WASM_PACK"):$PATH"

if [[ ! -x "$RELEASE_RUNA" ]]; then
    run_step cargo build --release
fi

RAW_TARGETS=("$@")
if [[ ${#RAW_TARGETS[@]} -eq 0 ]]; then
    RAW_TARGETS=(tests/canary)
fi

CANARIES=()
for target in "${RAW_TARGETS[@]}"; do
    if [[ -f "$target" ]]; then
        if grep -q '^[[:space:]]*-- wasm-build-canary' "$target"; then
            CANARIES+=("$target")
        fi
    elif [[ -d "$target" ]]; then
        while IFS= read -r file; do
            if grep -q '^[[:space:]]*-- wasm-build-canary' "$file"; then
                CANARIES+=("$file")
            fi
        done < <(find "$target" -type f -name '*.runa' | sort)
    else
        echo "[wasm-canary] target not found: $target" >&2
        exit 1
    fi
done

if [[ ${#CANARIES[@]} -eq 0 ]]; then
    echo "[wasm-canary] no WASM build canaries matched; add '-- wasm-build-canary' to a .runa fixture" >&2
    exit 1
fi

for file in "${CANARIES[@]}"; do
    run_step "$RELEASE_RUNA" wasm "$file"
done

echo
echo "[wasm-canary] WASM build canaries passed for: ${CANARIES[*]}"
