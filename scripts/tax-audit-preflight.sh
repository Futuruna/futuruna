#!/usr/bin/env bash
# Small behavior check of the selected binary, not an installation or tax audit.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNA_BIN="${RUNA_BIN:-$ROOT_DIR/target/release/runa}"
if ! command -v "$RUNA_BIN" >/dev/null 2>&1; then
    echo "[tax-audit] Cannot execute RUNA_BIN=$RUNA_BIN. No build was started." >&2
    exit 1
fi

PROBE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-tax-preflight.XXXXXX")"
# Preserve this tiny synthetic evidence on failure; never touch user documents.
echo "[tax-audit] Binary: $RUNA_BIN"
echo "[tax-audit] Synthetic evidence: $PROBE_DIR"
"$RUNA_BIN" --version
export FUTURUNA_CALCULATION_JOBS=1
FAILURES=0

fail_probe() {
    echo "[tax-audit] FAIL: $*" >&2
    FAILURES=$((FAILURES + 1))
}

# These probes contain only fixed ASCII strings without whitespace. This is
# deliberately NOT a general JSON parser or a tool for processing tax records.
compact() { tr -d '[:space:]' < "$1"; }

make_variant() {
    local template="$1" field="$2" replacement="$3" destination="$4"
    if [[ "$(grep -Ec "\"$field\": 0([,]|$)" "$template")" != 1 ]]; then
        echo "[tax-audit] Unexpected synthetic template layout; stopping." >&2
        exit 1
    fi
    sed "s/\"$field\": 0/$replacement/" "$template" > "$destination"
}

expect_success() {
    local name="$1" model="$2" input="$3" result="$4"
    if ! "$RUNA_BIN" call "$model" --input "$input" > "$PROBE_DIR/$name.out" 2> "$PROBE_DIR/$name.err"; then
        fail_probe "$name did not calculate. See $PROBE_DIR/$name.err and .out."
        return
    fi
    local actual
    actual="$(sed -E 's/"schema_hash": "[0-9a-f]{64}"/"schema_hash": "dynamic"/' "$PROBE_DIR/$name.out" | tr -d '[:space:]')"
    if [[ "$actual" != '{"$futuruna":{"schema":"futuruna.calculate.output.v1","schema_hash":"dynamic","entry":"calculate"},"results":[{"case_id":"case-1","result":'"$result"'}],"diagnostics":[]}' ]]; then
        fail_probe "$name returned unexpected output. See $PROBE_DIR/$name.out."
        return
    fi
    echo "[tax-audit] PASS: $name"
}

expect_failure() {
    local name="$1" model="$2" input="$3" diagnostic="$4" status=0
    "$RUNA_BIN" call "$model" --input "$input" > "$PROBE_DIR/$name.out" 2> "$PROBE_DIR/$name.err" || status=$?
    if [[ "$status" != 1 ]] || ! grep -Fq "$diagnostic" "$PROBE_DIR/$name.out" "$PROBE_DIR/$name.err"; then
        fail_probe "$name did not reject the input as expected. See $PROBE_DIR/$name.err and .out."
        return
    fi
    local output
    output="$(compact "$PROBE_DIR/$name.out")"
    if [[ "$name" == duplicate-* ]]; then
        [[ -z "$output" ]] || { fail_probe "$name emitted calculation output for ambiguous input."; return; }
    elif [[ "$output" != *'"results":[]'* || "$output" == *'"result":'* || "$output" != *'"case_id":"case-1"'* ]]; then
        fail_probe "$name did not isolate the failed case from successful results."
        return
    fi
    echo "[tax-audit] PASS: $name"
}

RUNTIME_MODEL="$ROOT_DIR/tests/fixtures/calculation/audit-runtime.calculate.runa"
MODULE_MODEL="$ROOT_DIR/tests/fixtures/calculation/audit-modules.calculate.runa"
"$RUNA_BIN" template "$RUNTIME_MODEL" --format json --output "$PROBE_DIR/runtime.json"
"$RUNA_BIN" template "$MODULE_MODEL" --format json --output "$PROBE_DIR/modules.json"

expect_success arithmetic "$RUNTIME_MODEL" "$PROBE_DIR/runtime.json" '{"label":"known","quotient":100}'
expect_success inline-modules "$MODULE_MODEL" "$PROBE_DIR/modules.json" '{"count":2,"scoped":10}'

make_variant "$PROBE_DIR/runtime.json" divisor '"divisor": -1' "$PROBE_DIR/division.json"
expect_failure division-by-zero "$RUNTIME_MODEL" "$PROBE_DIR/division.json" 'integer division'
make_variant "$PROBE_DIR/runtime.json" year '"year": 2027' "$PROBE_DIR/year.json"
expect_failure undefined-rule "$RUNTIME_MODEL" "$PROBE_DIR/year.json" 'label/1'
make_variant "$PROBE_DIR/modules.json" value '"value": -1' "$PROBE_DIR/list.json"
expect_failure undefined-list-rule "$MODULE_MODEL" "$PROBE_DIR/list.json" 'amounts/1'

# Both orders matter: a permissive reader can select either the first or last.
make_variant "$PROBE_DIR/runtime.json" year '"year": 2027, "year": 0' "$PROBE_DIR/duplicate-first.json"
expect_failure duplicate-first "$RUNTIME_MODEL" "$PROBE_DIR/duplicate-first.json" 'duplicate JSON object member'
make_variant "$PROBE_DIR/runtime.json" year '"year": 0, "year": 2027' "$PROBE_DIR/duplicate-last.json"
expect_failure duplicate-last "$RUNTIME_MODEL" "$PROBE_DIR/duplicate-last.json" 'duplicate JSON object member'

if [[ "$FAILURES" != 0 ]]; then
    echo "[tax-audit] STOP: $FAILURES compatibility check(s) failed. Do not audit with this binary/model pairing." >&2
    echo "[tax-audit] Use a verified binary built from this checkout, then rerun this check. See website/public/ai-setup.md." >&2
    exit 1
fi
echo "[tax-audit] All 7 runtime compatibility checks passed. This is not tax-law, document, or complete model validation."
