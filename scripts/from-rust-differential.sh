#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[from-rust-diff] $*"
    "$@"
}

fail() {
    echo "[from-rust-diff] error: $*" >&2
    echo "[from-rust-diff] artifacts kept in $OUT_DIR" >&2
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

SOURCE_SHAPE_MANIFEST="$ROOT_DIR/tests/from-rust/differential/search-manifest.tsv"
BASE_CASE_COUNT=6
GENERATOR_COUNT=6
DEFAULT_SEARCH_SEEDS="17 29 43"
SEARCH_SEEDS_RAW="${FUTURUNA_FROM_RUST_DIFF_SEEDS:-$DEFAULT_SEARCH_SEEDS}"
read -r -a SEARCH_SEEDS <<<"$SEARCH_SEEDS_RAW"

write_case_manifest() {
    cp -f "$SOURCE_SHAPE_MANIFEST" "$OUT_DIR/manifest.tsv"
}

write_case_index() {
    cat >"$OUT_DIR/cases.tsv" <<'CASES'
case	generator	seed	source_shape_id
numeric_branch_matrix.rs	numeric_branch_matrix	base	frss-control-flow
option_result_pipeline.rs	option_result_pipeline	base	frss-option-result
nested_order_totals.rs	nested_order_totals	base	frss-nested-data
btree_rollup_report.rs	btree_rollup_report	base	frss-deterministic-map
text_transform_matrix.rs	text_transform_matrix	base	frss-strings-formatting
enum_loop_rebinding.rs	enum_loop_rebinding	base	frss-enum-rebinding
CASES

    local seed
    for seed in "${SEARCH_SEEDS[@]}"; do
        cat >>"$OUT_DIR/cases.tsv" <<CASES
numeric_branch_matrix_seed_${seed}.rs	numeric_branch_matrix	${seed}	frss-control-flow
option_result_pipeline_seed_${seed}.rs	option_result_pipeline	${seed}	frss-option-result
nested_order_totals_seed_${seed}.rs	nested_order_totals	${seed}	frss-nested-data
btree_rollup_report_seed_${seed}.rs	btree_rollup_report	${seed}	frss-deterministic-map
text_transform_matrix_seed_${seed}.rs	text_transform_matrix	${seed}	frss-strings-formatting
enum_loop_rebinding_seed_${seed}.rs	enum_loop_rebinding	${seed}	frss-enum-rebinding
CASES
    done
}

write_coverage_metrics() {
    local generated_per_shape=$((1 + ${#SEARCH_SEEDS[@]}))
    local total_cases=$((BASE_CASE_COUNT + GENERATOR_COUNT * ${#SEARCH_SEEDS[@]}))
    {
        printf 'metric\tvalue\n'
        printf 'source_shape_families\t%s\n' "$GENERATOR_COUNT"
        printf 'base_cases\t%s\n' "$BASE_CASE_COUNT"
        printf 'search_seeds\t%s\n' "${SEARCH_SEEDS[*]:-none}"
        printf 'generated_cases\t%s\n' "$total_cases"
        printf '\n'
        printf 'source_shape_id\tgenerated_cases\n'
        printf 'frss-control-flow\t%s\n' "$generated_per_shape"
        printf 'frss-option-result\t%s\n' "$generated_per_shape"
        printf 'frss-nested-data\t%s\n' "$generated_per_shape"
        printf 'frss-deterministic-map\t%s\n' "$generated_per_shape"
        printf 'frss-strings-formatting\t%s\n' "$generated_per_shape"
        printf 'frss-enum-rebinding\t%s\n' "$generated_per_shape"
    } >"$OUT_DIR/coverage.tsv"
}

write_replay_script() {
    {
        printf '#!/usr/bin/env bash\n'
        printf 'set -euo pipefail\n'
        printf 'cd %q\n' "$ROOT_DIR"
        printf '%q from-rust --test %q\n' "$RELEASE_RUNA" "$CASE_DIR"
    } >"$OUT_DIR/replay.sh"
    chmod +x "$OUT_DIR/replay.sh"
}

write_minimization_notes() {
    cat >"$OUT_DIR/minimize.md" <<'NOTES'
# From-Rust Differential Failure Triage

1. Re-run `./replay.sh` from this artifact directory to confirm the failure.
2. Open `cases.tsv` to identify the generator, seed, and FRSS source-shape id.
3. Copy the failing `cases/*.rs` file to a scratch file and delete code until
   the Rust program still compiles/runs and `runa from-rust --test` still fails.
4. If the minimized source is inside FRSS-v0, check it in as a permanent
   supported fixture under `tests/from-rust/downstream/supported/` or
   `examples/from-rust/`.
5. If the minimized source is outside FRSS-v0, check it in under
   `tests/from-rust/downstream/unsupported/` with a
   `// runa-from-rust: expect-unsupported ...` directive and add or update the
   fail-closed diagnostic before treating the search failure as resolved.
6. Update `tests/from-rust/differential/search-manifest.tsv` and
   `docs/from-rust-contract.md` if the fix changes the supported source-shape
   claim or the generated search family.
NOTES
}

write_cases() {
    mkdir -p "$CASE_DIR"

    cat >"$CASE_DIR/numeric_branch_matrix.rs" <<'RUST'
// Generated supported-subset case: loops, branches, and arithmetic.

fn classify(value: i64) -> String {
    if value < 0 {
        "neg".to_string()
    } else if value == 0 {
        "zero".to_string()
    } else if value % 2 == 0 {
        "even".to_string()
    } else {
        "odd".to_string()
    }
}

fn score(values: Vec<i64>) -> i64 {
    let mut total = 0;
    for value in values {
        if value > 0 {
            total += value;
        }
    }
    total
}

fn main() {
    let values = vec![-2, 0, 3, 4, 7];
    println!("score={}", score(values.clone()));
    println!("a={}", classify(values[0]));
    println!("b={}", classify(values[1]));
    println!("c={}", classify(values[3]));
}
RUST

    cat >"$CASE_DIR/option_result_pipeline.rs" <<'RUST'
// Generated supported-subset case: Option/Result parse pipeline.

#[derive(Clone, Debug)]
enum ParseProblem {
    Missing(String),
    Invalid(String),
    TooLarge(String),
}

fn parse_count(name: &str, raw: Option<String>) -> Result<i64, ParseProblem> {
    match raw {
        Some(value) => {
            let n: i64 = value
                .parse()
                .map_err(|_| ParseProblem::Invalid(name.to_string()))?;
            if n <= 20 {
                Ok(n)
            } else {
                Err(ParseProblem::TooLarge(name.to_string()))
            }
        }
        None => Err(ParseProblem::Missing(name.to_string())),
    }
}

fn describe(result: Result<i64, ParseProblem>) -> String {
    match result {
        Ok(value) => format!("ok:{}", value),
        Err(ParseProblem::Missing(name)) => format!("missing:{}", name),
        Err(ParseProblem::Invalid(name)) => format!("invalid:{}", name),
        Err(ParseProblem::TooLarge(name)) => format!("large:{}", name),
    }
}

fn main() {
    println!("{}", describe(parse_count("apples", Some("12".to_string()))));
    println!("{}", describe(parse_count("pears", None)));
    println!("{}", describe(parse_count("figs", Some("many".to_string()))));
    println!("{}", describe(parse_count("plums", Some("30".to_string()))));
}
RUST

    cat >"$CASE_DIR/nested_order_totals.rs" <<'RUST'
// Generated supported-subset case: nested structs and vectors.

#[derive(Clone, Debug)]
struct Line {
    label: String,
    count: i64,
    cents: i64,
}

#[derive(Clone, Debug)]
struct Invoice {
    id: String,
    lines: Vec<Line>,
}

fn line(label: &str, count: i64, cents: i64) -> Line {
    Line {
        label: label.to_string(),
        count,
        cents,
    }
}

fn add_line(lines: Vec<Line>, label: &str, count: i64, cents: i64) -> Vec<Line> {
    let mut out = lines.clone();
    out.push(line(label, count, cents));
    out
}

fn invoice(id: &str, lines: Vec<Line>) -> Invoice {
    Invoice {
        id: id.to_string(),
        lines,
    }
}

fn line_total(line: &Line) -> i64 {
    line.count * line.cents
}

fn invoice_total(invoice: &Invoice) -> i64 {
    line_total(&invoice.lines[0]) + line_total(&invoice.lines[1])
}

fn main() {
    let mut lines = Vec::new();
    lines = add_line(lines, "notebook", 3, 250);
    lines = add_line(lines, "pencil", 5, 80);
    let invoice = invoice("INV-7", lines);

    println!("id={}", invoice.id.clone());
    println!("lines={}", invoice.lines.len());
    println!("first={}", invoice.lines[0].label.clone());
    println!("total={}", invoice_total(&invoice));
}
RUST

    cat >"$CASE_DIR/btree_rollup_report.rs" <<'RUST'
// Generated supported-subset case: deterministic BTreeMap rollup.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Reading {
    lane: String,
    amount: i64,
}

fn reading(lane: &str, amount: i64) -> Reading {
    Reading {
        lane: lane.to_string(),
        amount,
    }
}

fn add_reading(readings: Vec<Reading>, lane: &str, amount: i64) -> Vec<Reading> {
    let mut out = readings.clone();
    out.push(reading(lane, amount));
    out
}

fn rollup(readings: &Vec<Reading>) -> BTreeMap<String, i64> {
    let mut totals = BTreeMap::new();
    for reading in readings {
        let current = totals.get(&reading.lane).unwrap_or(&0);
        totals.insert(reading.lane.clone(), *current + reading.amount);
    }
    totals
}

fn main() {
    let mut readings = Vec::new();
    readings = add_reading(readings, "north", 4);
    readings = add_reading(readings, "south", 2);
    readings = add_reading(readings, "north", 5);
    readings = add_reading(readings, "east", 7);

    let totals = rollup(&readings);
    println!("count={}", readings.len());
    println!("east={}", totals.get("east").unwrap_or(&0));
    println!("north={}", totals.get("north").unwrap_or(&0));
    println!("south={}", totals.get("south").unwrap_or(&0));
}
RUST

    cat >"$CASE_DIR/text_transform_matrix.rs" <<'RUST'
// Generated supported-subset case: string normalization and classification.

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();
    lower.replace(" ", "_")
}

fn classify(raw: &str) -> String {
    let token = normalize(raw);
    if token.starts_with("warn") {
        format!("warn:{}", token)
    } else if token.ends_with("_ok") {
        format!("ok:{}", token)
    } else {
        format!("note:{}", token)
    }
}

fn main() {
    println!("{}", normalize("  Hello World  "));
    println!("{}", classify("WARN Disk"));
    println!("{}", classify("batch ok"));
    println!("{}", classify("status pending"));
}
RUST

    cat >"$CASE_DIR/enum_loop_rebinding.rs" <<'RUST'
// Generated supported-subset case: enum matching plus conditional rebinding.

#[derive(Clone, Debug)]
enum Signal {
    Add(i64),
    Drop(i64),
    Label(String),
}

fn signal_label(signal: &Signal) -> String {
    match signal {
        Signal::Add(value) => format!("add {}", value),
        Signal::Drop(value) => format!("drop {}", value),
        Signal::Label(name) => format!("label {}", name),
    }
}

fn positive_add_total(signals: &Vec<Signal>) -> i64 {
    let mut total = 0;
    for signal in signals {
        let label = signal_label(signal);
        if label.starts_with("add") {
            total += 1;
        }
    }
    total
}

fn main() {
    let mut signals = Vec::new();
    signals.push(Signal::Add(10));
    signals.push(Signal::Drop(4));
    signals.push(Signal::Label("ready".to_string()));
    signals.push(Signal::Add(3));

    println!("signals={}", signals.len());
    println!("adds={}", positive_add_total(&signals));
    println!("first={}", signal_label(&signals[0]));
    println!("last={}", signal_label(&signals[3]));
}
RUST
}

validate_search_seeds() {
    local seed
    for seed in "${SEARCH_SEEDS[@]}"; do
        [[ "$seed" =~ ^[0-9]+$ ]] || fail "search seed must be a non-negative integer: $seed"
    done
}

write_search_cases() {
    validate_search_seeds

    local seed
    for seed in "${SEARCH_SEEDS[@]}"; do
        write_numeric_branch_matrix_search_case \
            "$CASE_DIR/numeric_branch_matrix_seed_${seed}.rs" \
            "$seed"
        write_option_result_pipeline_search_case \
            "$CASE_DIR/option_result_pipeline_seed_${seed}.rs" \
            "$seed"
        write_nested_order_totals_search_case \
            "$CASE_DIR/nested_order_totals_seed_${seed}.rs" \
            "$seed"
        write_btree_rollup_report_search_case \
            "$CASE_DIR/btree_rollup_report_seed_${seed}.rs" \
            "$seed"
        write_text_transform_matrix_search_case \
            "$CASE_DIR/text_transform_matrix_seed_${seed}.rs" \
            "$seed"
        write_enum_loop_rebinding_search_case \
            "$CASE_DIR/enum_loop_rebinding_seed_${seed}.rs" \
            "$seed"
    done
}

write_numeric_branch_matrix_search_case() {
    local file="$1"
    local seed="$2"
    local negative=$((-(seed % 7 + 1)))
    local positive=$((seed % 11 + 3))
    local even=$(((seed % 5 + 1) * 2))
    local bonus=$((seed % 13 + 5))

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: control flow and arithmetic.
// seed: $seed

fn classify(value: i64) -> String {
    if value < 0 {
        "neg".to_string()
    } else if value == 0 {
        "zero".to_string()
    } else if value % 2 == 0 {
        "even".to_string()
    } else {
        "odd".to_string()
    }
}

fn score(values: Vec<i64>) -> i64 {
    let mut total = 0;
    for value in values {
        if value > 0 {
            total += value;
        } else {
            total += 1;
        }
    }
    total
}

fn main() {
    let values = vec![$negative, 0, $positive, $even, $bonus];
    println!("seed=$seed");
    println!("score={}", score(values.clone()));
    println!("a={}", classify(values[0]));
    println!("b={}", classify(values[1]));
    println!("c={}", classify(values[2]));
    println!("d={}", classify(values[3]));
}
RUST
}

write_option_result_pipeline_search_case() {
    local file="$1"
    local seed="$2"
    local ok=$((seed % 17 + 2))
    local large=$((seed % 19 + 30))

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: Option/Result parse pipeline.
// seed: $seed

#[derive(Clone, Debug)]
enum ParseProblem {
    Missing(String),
    Invalid(String),
    TooLarge(String),
}

fn parse_count(name: &str, raw: Option<String>) -> Result<i64, ParseProblem> {
    match raw {
        Some(value) => {
            let n: i64 = value
                .parse()
                .map_err(|_| ParseProblem::Invalid(name.to_string()))?;
            if n <= 25 {
                Ok(n)
            } else {
                Err(ParseProblem::TooLarge(name.to_string()))
            }
        }
        None => Err(ParseProblem::Missing(name.to_string())),
    }
}

fn describe(result: Result<i64, ParseProblem>) -> String {
    match result {
        Ok(value) => format!("ok:{}", value),
        Err(ParseProblem::Missing(name)) => format!("missing:{}", name),
        Err(ParseProblem::Invalid(name)) => format!("invalid:{}", name),
        Err(ParseProblem::TooLarge(name)) => format!("large:{}", name),
    }
}

fn main() {
    println!("seed=$seed");
    println!("{}", describe(parse_count("ok", Some("$ok".to_string()))));
    println!("{}", describe(parse_count("missing", None)));
    println!("{}", describe(parse_count("invalid", Some("bad-$seed".to_string()))));
    println!("{}", describe(parse_count("large", Some("$large".to_string()))));
}
RUST
}

write_nested_order_totals_search_case() {
    local file="$1"
    local seed="$2"
    local count_a=$((seed % 4 + 2))
    local cents_a=$(((seed % 5 + 1) * 100))
    local count_b=$((seed % 6 + 1))
    local cents_b=$(((seed % 7 + 2) * 50))

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: nested structs and vectors.
// seed: $seed

#[derive(Clone, Debug)]
struct Line {
    label: String,
    count: i64,
    cents: i64,
}

#[derive(Clone, Debug)]
struct Invoice {
    id: String,
    lines: Vec<Line>,
}

fn line(label: &str, count: i64, cents: i64) -> Line {
    Line {
        label: label.to_string(),
        count,
        cents,
    }
}

fn add_line(lines: Vec<Line>, label: &str, count: i64, cents: i64) -> Vec<Line> {
    let mut out = lines.clone();
    out.push(line(label, count, cents));
    out
}

fn invoice(id: &str, lines: Vec<Line>) -> Invoice {
    Invoice {
        id: id.to_string(),
        lines,
    }
}

fn line_total(line: &Line) -> i64 {
    line.count * line.cents
}

fn invoice_total(invoice: &Invoice) -> i64 {
    line_total(&invoice.lines[0]) + line_total(&invoice.lines[1])
}

fn main() {
    let mut lines = Vec::new();
    lines = add_line(lines, "alpha-$seed", $count_a, $cents_a);
    lines = add_line(lines, "beta-$seed", $count_b, $cents_b);
    let invoice = invoice("INV-$seed", lines);

    println!("id={}", invoice.id.clone());
    println!("lines={}", invoice.lines.len());
    println!("first={}", invoice.lines[0].label.clone());
    println!("second={}", invoice.lines[1].label.clone());
    println!("total={}", invoice_total(&invoice));
}
RUST
}

write_btree_rollup_report_search_case() {
    local file="$1"
    local seed="$2"
    local north=$((seed % 7 + 2))
    local south=$((seed % 5 + 3))
    local east=$((seed % 11 + 1))
    local north2=$((seed % 13 + 4))

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: deterministic BTreeMap rollup.
// seed: $seed

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Reading {
    lane: String,
    amount: i64,
}

fn reading(lane: &str, amount: i64) -> Reading {
    Reading {
        lane: lane.to_string(),
        amount,
    }
}

fn add_reading(readings: Vec<Reading>, lane: &str, amount: i64) -> Vec<Reading> {
    let mut out = readings.clone();
    out.push(reading(lane, amount));
    out
}

fn rollup(readings: &Vec<Reading>) -> BTreeMap<String, i64> {
    let mut totals = BTreeMap::new();
    for reading in readings {
        let current = totals.get(&reading.lane).unwrap_or(&0);
        totals.insert(reading.lane.clone(), *current + reading.amount);
    }
    totals
}

fn main() {
    let mut readings = Vec::new();
    readings = add_reading(readings, "north", $north);
    readings = add_reading(readings, "south", $south);
    readings = add_reading(readings, "east", $east);
    readings = add_reading(readings, "north", $north2);

    let totals = rollup(&readings);
    println!("seed=$seed");
    println!("count={}", readings.len());
    println!("east={}", totals.get("east").unwrap_or(&0));
    println!("north={}", totals.get("north").unwrap_or(&0));
    println!("south={}", totals.get("south").unwrap_or(&0));
}
RUST
}

write_text_transform_matrix_search_case() {
    local file="$1"
    local seed="$2"

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: strings and formatting.
// seed: $seed

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();
    lower.replace(" ", "_")
}

fn classify(raw: &str) -> String {
    let token = normalize(raw);
    if token.starts_with("warn") {
        format!("warn:{}", token)
    } else if token.ends_with("_ok") {
        format!("ok:{}", token)
    } else {
        format!("note:{}", token)
    }
}

fn main() {
    println!("seed=$seed");
    println!("{}", normalize("  Hello Seed $seed  "));
    println!("{}", classify("WARN Disk $seed"));
    println!("{}", classify("batch $seed ok"));
    println!("{}", classify("status $seed pending"));
}
RUST
}

write_enum_loop_rebinding_search_case() {
    local file="$1"
    local seed="$2"
    local add_a=$((seed % 8 + 1))
    local drop_a=$((seed % 5 + 1))
    local add_b=$((seed % 9 + 2))

    cat >"$file" <<RUST
// Generated seed-stable FRSS search case: enum matching and rebinding.
// seed: $seed

#[derive(Clone, Debug)]
enum Signal {
    Add(i64),
    Drop(i64),
    Label(String),
}

fn signal_label(signal: &Signal) -> String {
    match signal {
        Signal::Add(value) => format!("add {}", value),
        Signal::Drop(value) => format!("drop {}", value),
        Signal::Label(name) => format!("label {}", name),
    }
}

fn add_count(signals: &Vec<Signal>) -> i64 {
    let mut total = 0;
    for signal in signals {
        let label = signal_label(signal);
        if label.starts_with("add") {
            total += 1;
        } else {
            total += 0;
        }
    }
    total
}

fn main() {
    let mut signals = Vec::new();
    signals.push(Signal::Add($add_a));
    signals.push(Signal::Drop($drop_a));
    signals.push(Signal::Label("seed-$seed".to_string()));
    signals.push(Signal::Add($add_b));

    println!("seed=$seed");
    println!("signals={}", signals.len());
    println!("adds={}", add_count(&signals));
    println!("first={}", signal_label(&signals[0]));
    println!("third={}", signal_label(&signals[2]));
    println!("last={}", signal_label(&signals[3]));
}
RUST
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

AUTO_OUT=0
if [[ -n "${FUTURUNA_FROM_RUST_DIFF_OUT:-}" ]]; then
    OUT_DIR="$FUTURUNA_FROM_RUST_DIFF_OUT"
    mkdir -p "$OUT_DIR"
else
    OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-from-rust-diff.XXXXXX")"
    AUTO_OUT=1
fi

CASE_DIR="$OUT_DIR/cases"
OUTPUT_FILE="$OUT_DIR/from-rust-test.out"
rm -rf "$CASE_DIR"
mkdir -p "$CASE_DIR"

cleanup() {
    local status=$?
    if [[ "$status" -eq 0 && "$AUTO_OUT" -eq 1 && "${FUTURUNA_FROM_RUST_DIFF_KEEP:-0}" != "1" ]]; then
        rm -rf "$OUT_DIR"
    else
        echo "[from-rust-diff] artifacts kept in $OUT_DIR" >&2
    fi
}
trap cleanup EXIT

write_case_manifest
write_cases
write_search_cases
write_case_index
write_coverage_metrics
write_replay_script
write_minimization_notes

echo
echo "[from-rust-diff] $RELEASE_RUNA from-rust --test $CASE_DIR"
if ! "$RELEASE_RUNA" from-rust --test "$CASE_DIR" >"$OUTPUT_FILE" 2>&1; then
    cat "$OUTPUT_FILE" >&2
    fail "supported-subset differential lane failed"
fi

EXPECTED_MATCHES=$((BASE_CASE_COUNT + GENERATOR_COUNT * ${#SEARCH_SEEDS[@]}))
cat "$OUTPUT_FILE" >&2
grep -Fq "From-rust: $EXPECTED_MATCHES matched" "$OUTPUT_FILE" \
    || fail "differential lane did not report expected summary: From-rust: $EXPECTED_MATCHES matched"

echo
echo "[from-rust-diff] Search coverage: $GENERATOR_COUNT source-shape families, $EXPECTED_MATCHES generated cases, seeds: ${SEARCH_SEEDS[*]:-none}"
echo "[from-rust-diff] Supported-subset from-rust differential lane passed."
