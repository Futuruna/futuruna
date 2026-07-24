#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

run_step() {
    echo
    echo "[expect] $*"
    "$@"
}

RELEASE_RUNA="${RUNA_BIN:-./target/release/runa}"

needs_build=false
if [[ ! -x "$RELEASE_RUNA" ]]; then
    needs_build=true
elif ! "$RELEASE_RUNA" help 2>&1 | grep -q "runa expect"; then
    needs_build=true
elif find src Cargo.toml Cargo.lock -newer "$RELEASE_RUNA" -print -quit | grep -q .; then
    needs_build=true
fi

if [[ "$needs_build" == true ]]; then
    run_step cargo build --release
fi

TARGETS=("$@")
if [[ ${#TARGETS[@]} -eq 0 ]]; then
    TARGETS=(tests/expect)
fi

for target in "${TARGETS[@]}"; do
    run_step "$RELEASE_RUNA" expect "$target"
done

echo
echo "[expect] Futuruna expectation suites passed for: ${TARGETS[*]}"
