#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

log() {
    echo "[compiler-cross-product] $*"
}

fail() {
    echo "[compiler-cross-product] error: $*" >&2
    echo "[compiler-cross-product] artifacts kept in $OUT_DIR" >&2
    exit 1
}

run_step() {
    log "$*"
    "$@" || fail "command failed: $*"
}

assert_contains() {
    local label="$1"
    local axes="$2"
    local haystack="$3"
    local needle="$4"
    if ! grep -Fq "$needle" <<<"$haystack"; then
        echo "[compiler-cross-product] output for $label:" >&2
        echo "$haystack" >&2
        fail "case $label axes=[$axes] missing expected output: $needle"
    fi
}

RUNA_BIN="${RUNA_BIN:-./target/release/runa}"
if [[ ! -x "$RUNA_BIN" ]]; then
    run_step cargo build --release
fi

OUT_DIR="${FUTURUNA_COMPILER_CROSS_PRODUCT_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/futuruna-compiler-cross-product.XXXXXX")}"
CASE_DIR="$OUT_DIR/cases"
DEPS_DIR="$CASE_DIR/deps"
mkdir -p "$DEPS_DIR"

cat >"$OUT_DIR/manifest.tsv" <<'MANIFEST'
case	axes	surfaces	expected_marker
import_ownership_lambda	nested-plain-import,qualified-import,invariant,prove,string-branch-reuse,match,lambdas,list,map	check,run,check-codegen,roundtrip,emit-rust	compiler cross product import_ownership_lambda passed
branch_lambda_match_roundtrip	invariant,prove,string-branch-reuse,match,lambdas,list	check,run,check-codegen,roundtrip	compiler cross product branch_lambda_match_roundtrip passed
MANIFEST

cat >"$DEPS_DIR/base.runa" <<'RUNA'
-- library-hygiene: importable
-- roundtrip-skip: generated dependency fixture for compiler cross-product canary

@ export
# Phase = Solid(String) | Liquid(String) | Gas

@ export
> phase_tag(phase: Phase) -> String {
    match phase {
        | Solid(label) -> "Solid:" + label
        | Liquid(label) -> "Liquid:" + label
        | Gas -> "Gas"
    }
}

@ export
> make_phase(predicted: String, regret: Int) -> Phase {
    if string_length(predicted) > 0 {
        if regret > 0 {
            Liquid(predicted)
        } else {
            Solid(predicted)
        }
    } else {
        Gas
    }
}
RUNA

cat >"$DEPS_DIR/policy.runa" <<'RUNA'
-- library-hygiene: importable
-- roundtrip-skip: generated dependency fixture for compiler cross-product canary

@ import ./base

@ export
> policy_label(predicted: String, regret: Int) -> String {
    phase_tag(make_phase(predicted, regret))
}

@ export
> preserve_phase(predicted: String, regret: Int) -> String {
    = before_phase = if string_length(predicted) > 0 { "Solid" } else { "Gas" }
    = after_phase = if regret > 0 { "Liquid" } else { before_phase }
    before_phase + "->" + after_phase
}
RUNA

cat >"$CASE_DIR/import_ownership_lambda_test.runa" <<'RUNA'
-- Generated compiler cross-product case.
-- axes: nested-plain-import, qualified-import, invariant, prove,
--       string-branch-reuse, match, lambdas, list, map

@ import ./deps / base
@ import ./deps / policy
@ import Policy from ./deps / policy

> bump(counts: Map(String, Int), key: String) -> Map(String, Int) {
    map_insert(counts, key, map_get_or(counts, key, 0) + 1)
}

> render_words(words: List(String)) -> String {
    = decorate = |word| Policy.policy_label(word, string_length(word)) + ":" + preserve_phase(word, 1)
    join(map(words, decorate), "|")
}

> phase_counts(words: List(String)) -> Map(String, Int) {
    = empty: Map(String, Int) = map_new()
    foldl(words, empty, |acc, word| bump(acc, phase_tag(make_phase(word, 1))))
}

= words = ["alpha", "beta", "gamma"]
= report = render_words(words)
= counts = phase_counts(words)
= phase_path = preserve_phase("alpha", 1)
= fallback_path = Policy.preserve_phase("beta", 0)

| report_mentions_liquid: report -> contains(report, "Liquid:alpha")
| branch_reuse_ok: phase_path -> phase_path == "Solid->Liquid"
| fallback_reuse_ok: fallback_path -> fallback_path == "Solid->Solid"
| count_alpha_ok: counts -> map_get_or(counts, "Liquid:alpha", 0) == 1

? report_mentions_liquid -> {
    @ print("proof report_mentions_liquid passed")
}

? branch_reuse_ok
? fallback_reuse_ok
? count_alpha_ok

@ print("axes=nested-plain-import+qualified-import+invariant+prove+string-branch-reuse+match+lambdas+list+map")
@ print(report)
@ print(phase_path + "|" + fallback_path)
@ print("compiler cross product import_ownership_lambda passed")
RUNA

cat >"$CASE_DIR/branch_lambda_match_roundtrip_test.runa" <<'RUNA'
-- Generated compiler cross-product case.
-- axes: invariant, prove, string-branch-reuse, match, lambdas, list

# Mode = Hot(String) | Cold(String)

> choose_mode(name: String, score: Int) -> Mode {
    if score > 3 {
        Hot(name)
    } else {
        Cold(name)
    }
}

> mode_label(mode: Mode) -> String {
    match mode {
        | Hot(name) -> "hot:" + name
        | Cold(name) -> "cold:" + name
    }
}

> reuse_branch_label(name: String, score: Int) -> String {
    = before = if string_length(name) > 0 { "seen" } else { "empty" }
    = after = if score > 3 { "kept" } else { before }
    before + "/" + after
}

> render_modes(names: List(String)) -> String {
    = decorate = |name| mode_label(choose_mode(name, string_length(name))) + ":" + reuse_branch_label(name, string_length(name))
    join(map(names, decorate), "|")
}

= rendered = render_modes(["a", "mint", "core"])

| rendered_ok: rendered -> rendered == "cold:a:seen/seen|hot:mint:seen/kept|hot:core:seen/kept"

? rendered_ok -> {
    @ print("proof branch_lambda_match_roundtrip passed")
}

@ print(rendered)
@ print("compiler cross product branch_lambda_match_roundtrip passed")
RUNA

case_path="$CASE_DIR/import_ownership_lambda_test.runa"
roundtrip_case_path="$CASE_DIR/branch_lambda_match_roundtrip_test.runa"
case_label="import_ownership_lambda"
case_axes="nested-plain-import,qualified-import,invariant,prove,string-branch-reuse,match,lambdas,list,map"
roundtrip_case_label="branch_lambda_match_roundtrip"
roundtrip_case_axes="invariant,prove,string-branch-reuse,match,lambdas,list"

log "generated manifest: $OUT_DIR/manifest.tsv"
log "case $case_label axes=[$case_axes]"
log "case $roundtrip_case_label axes=[$roundtrip_case_axes]"

run_step "$RUNA_BIN" fmt --check "$CASE_DIR"
run_step "$RUNA_BIN" check "$case_path"
run_step "$RUNA_BIN" check "$roundtrip_case_path"

run_output="$("$RUNA_BIN" run "$case_path")" \
    || fail "run failed for $case_label axes=[$case_axes]"
assert_contains "$case_label" "$case_axes" "$run_output" "proof report_mentions_liquid passed"
assert_contains "$case_label" "$case_axes" "$run_output" "Liquid:alpha:Solid->Liquid|Liquid:beta:Solid->Liquid|Liquid:gamma:Solid->Liquid"
assert_contains "$case_label" "$case_axes" "$run_output" "Solid->Liquid|Solid->Solid"
assert_contains "$case_label" "$case_axes" "$run_output" "compiler cross product import_ownership_lambda passed"

roundtrip_run_output="$("$RUNA_BIN" run "$roundtrip_case_path")" \
    || fail "run failed for $roundtrip_case_label axes=[$roundtrip_case_axes]"
assert_contains "$roundtrip_case_label" "$roundtrip_case_axes" "$roundtrip_run_output" "proof branch_lambda_match_roundtrip passed"
assert_contains "$roundtrip_case_label" "$roundtrip_case_axes" "$roundtrip_run_output" "cold:a:seen/seen|hot:mint:seen/kept|hot:core:seen/kept"
assert_contains "$roundtrip_case_label" "$roundtrip_case_axes" "$roundtrip_run_output" "compiler cross product branch_lambda_match_roundtrip passed"

run_step "$RUNA_BIN" test --check-codegen "$CASE_DIR"
roundtrip_output="$("$RUNA_BIN" test --roundtrip "$CASE_DIR" 2>&1)" \
    || fail "roundtrip failed for generated cross-product corpus"
assert_contains "roundtrip" "generated-cross-product" "$roundtrip_output" "Roundtrip: 1 matched"

emit_output="$("$RUNA_BIN" emit "$case_path")" \
    || fail "emit-rust failed for $case_label axes=[$case_axes]"
assert_contains "$case_label" "$case_axes" "$emit_output" "fn preserve_phase"
assert_contains "$case_label" "$case_axes" "$emit_output" "before_phase.clone()"
assert_contains "$case_label" "$case_axes" "$emit_output" "fn render_words"

log "Generated compiler cross-product canaries passed in $OUT_DIR"
