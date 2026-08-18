#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[differential] $*"
    "$@"
}

RUNA_BIN="${RUNA_BIN:-./target/release/runa}"
STRESS_COUNT="${FUTURUNA_STRESS_COUNT:-64}"
SEEDS_FILE="${FUTURUNA_STRESS_SEEDS_FILE:-tests/differential/stress_gen_seeds.txt}"
CORPUS_DIR="${FUTURUNA_DIFFERENTIAL_CORPUS:-tests/differential/corpus}"
IMPORT_CORPUS_DIR="${FUTURUNA_DIFFERENTIAL_IMPORT_CORPUS:-${CORPUS_DIR}/imports}"
OUT_DIR="${FUTURUNA_DIFFERENTIAL_OUT:-${TMPDIR:-/tmp}/futuruna-differential}"
GENERATED_IMPORT_DIR="${FUTURUNA_DIFFERENTIAL_GENERATED_IMPORT_DIR:-${OUT_DIR}/generated-imports}"

if [[ ! -x "$RUNA_BIN" ]]; then
    run_step cargo build --release
fi

if [[ ! -f "$SEEDS_FILE" ]]; then
    echo "[differential] missing seeds file: $SEEDS_FILE" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"

seed_slug() {
    printf '%s' "$1" | tr -c 'A-Za-z0-9_' '_'
}

bucket_label() {
    local weight="$1"
    local warm_floor="$2"
    local hot_floor="$3"
    if (( weight >= hot_floor )); then
        printf 'hot'
    elif (( weight >= warm_floor )); then
        printf 'warm'
    else
        printf 'cold'
    fi
}

generate_import_case() {
    local seed="$1"
    local case_dir="$2"
    local slug
    slug="$(seed_slug "$seed")"
    local base_a=$((seed % 7 + 2))
    local base_b=$(((seed / 7) % 7 + 4))
    local base_c=$(((seed / 49) % 7 + 6))
    local delta=$(((seed / 343) % 5 + 1))
    local hot_floor=$(((seed / 2401) % 15 + 14))
    local warm_floor=$(((hot_floor + 1) / 2))
    local weight_a=$((base_a * 3 + delta))
    local weight_b=$((base_b * 3 + delta))
    local weight_c=$((base_c * 3 + delta))
    local bucket_a
    local bucket_b
    local bucket_c
    bucket_a="$(bucket_label "$weight_a" "$warm_floor" "$hot_floor")"
    bucket_b="$(bucket_label "$weight_b" "$warm_floor" "$hot_floor")"
    bucket_c="$(bucket_label "$weight_c" "$warm_floor" "$hot_floor")"
    local labels="${bucket_a}:alpha:${weight_a}|${bucket_b}:beta:${weight_b}|${bucket_c}:gamma:${weight_c}"
    local probe_weight=$((3 + 3 + 3 + delta))
    local probe="probe:${probe_weight}"

    mkdir -p "$case_dir"

    cat >"${case_dir}/generated_types.runa" <<'EOF'
-- library-hygiene: importable
-- roundtrip-skip: generated import helper, no expected output
--
-- Generated differential import helper: exported ADT plus accessors.

@ export
# Job = Job(String, Int, List(Int))

@ export
> job_name(job: Job) -> String {
    match job {
        | Job(name, _, _) -> name
    }
}

@ export
> job_score(job: Job) -> Int {
    match job {
        | Job(_, score, _) -> score
    }
}

@ export
> job_notes(job: Job) -> List(Int) {
    match job {
        | Job(_, _, notes) -> notes
    }
}
EOF

    cat >"${case_dir}/generated_shared.runa" <<EOF
-- library-hygiene: importable
-- roundtrip-skip: generated import helper, no expected output
--
-- Generated differential import helper seed ${seed}: nested flat imports and
-- exported pure top-level values.

@ import ./generated_types

@ export
> make_job(name: String, score: Int) -> Job {
    Job(name, score, [score, score + ${delta}])
}

@ export
> job_weight(job: Job) -> Int {
    job_score(job) + sum(job_notes(job))
}

@ export
> render_probe(job: Job) -> String {
    job_name(job) + ":" + show(job_weight(job))
}

@ export
= generated_probe = render_probe(make_job("probe", 3))
EOF

    cat >"${case_dir}/generated_policy.runa" <<EOF
-- library-hygiene: importable
-- roundtrip-skip: generated import helper, no expected output
--
-- Generated differential import helper seed ${seed}: qualified policy module.

@ import ./generated_types
@ import ./generated_shared

@ export
= warm_floor = ${warm_floor}

@ export
= hot_floor = ${hot_floor}

@ export
> bucket(job: Job) -> String {
    = weight = job_weight(job)
    if weight >= hot_floor {
        "hot"
    } else if weight >= warm_floor {
        "warm"
    } else {
        "cold"
    }
}
EOF

    cat >"${case_dir}/generated_import_${slug}_test.runa" <<EOF
-- Generated import-aware differential case for seed ${seed}. This stresses
-- nested flat imports, a qualified import, exported ADTs/functions/values,
-- list aggregation, map/fold use, and exact output assertions.
-- expect-command: run
-- expect-stdout: ${labels}
-- expect-stdout: probe=${probe}

@ import ./generated_shared
@ import Policy from ./generated_policy

> label(job: Job) -> String {
    Policy.bucket(job) + ":" + job_name(job) + ":" + show(job_weight(job))
}

= jobs = [
    make_job("alpha", ${base_a}),
    make_job("beta", ${base_b}),
    make_job("gamma", ${base_c})
]

= labels = map(jobs, label)
= labels_text = join(labels, "|")

@ print(labels_text)
@ print("probe=" + generated_probe)
EOF
}

if [[ -d "$CORPUS_DIR" ]] && find "$CORPUS_DIR" -type f -name '*.runa' -print -quit | grep -q .; then
    run_step "$RUNA_BIN" test --roundtrip "$CORPUS_DIR"
else
    echo "[differential] no differential corpus cases in $CORPUS_DIR"
fi

if [[ -d "$IMPORT_CORPUS_DIR" ]] && find "$IMPORT_CORPUS_DIR" -type f -name '*.runa' -print -quit | grep -q .; then
    # `runa test --roundtrip` intentionally skips @ import entrypoints, so
    # import-aware corpus cases use compiled execution plus codegen checks.
    run_step "$RUNA_BIN" test --run "$IMPORT_CORPUS_DIR"
    run_step "$RUNA_BIN" test --check-codegen "$IMPORT_CORPUS_DIR"
fi

rm -rf "$GENERATED_IMPORT_DIR"
mkdir -p "$GENERATED_IMPORT_DIR"

while IFS= read -r raw_seed || [[ -n "$raw_seed" ]]; do
    seed="${raw_seed%%#*}"
    seed="$(printf '%s' "$seed" | tr -d '[:space:]')"
    if [[ -z "$seed" ]]; then
        continue
    fi
    generate_import_case "$seed" "$GENERATED_IMPORT_DIR/seed-$(seed_slug "$seed")"
    run_step "$RUNA_BIN" stress-gen "$STRESS_COUNT" --seed "$seed" --save-failures "$OUT_DIR"
done < "$SEEDS_FILE"

if find "$GENERATED_IMPORT_DIR" -type f -name '*.runa' -print -quit | grep -q .; then
    run_step "$RUNA_BIN" fmt --check "$GENERATED_IMPORT_DIR"
    run_step "$RUNA_BIN" lint-library --imports "$GENERATED_IMPORT_DIR"
    while IFS= read -r case_dir; do
        case_entry="$(find "$case_dir" -maxdepth 1 -type f -name 'generated_import_*_test.runa' -print -quit)"
        if [[ -z "$case_entry" ]]; then
            echo "[differential] missing generated import entrypoint in $case_dir" >&2
            exit 1
        fi
        run_step "$RUNA_BIN" test "$case_dir"
        run_step "$RUNA_BIN" test --run "$case_dir"
        run_step "$RUNA_BIN" test --check-codegen "$case_dir"
        run_step "$RUNA_BIN" expect "$case_entry"
    done < <(find "$GENERATED_IMPORT_DIR" -mindepth 1 -maxdepth 1 -type d | sort)
fi

echo
echo "[differential] completed. Failure artifacts, if any, are in $OUT_DIR"
