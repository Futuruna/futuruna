#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Passing machine lanes should stay structural; comptime failures still print.
export FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS="${FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS:-1}"

run_step() {
    echo
    echo "[first-run] $*"
    "$@"
}

fail() {
    echo "[first-run] error: $*" >&2
    exit 1
}

require_file() {
    [[ -f "$1" ]] || fail "expected file '$1'"
}

require_executable() {
    [[ -x "$1" ]] || fail "expected executable '$1'"
}

require_contains() {
    local file="$1"
    local expected="$2"
    grep -Fq "$expected" "$file" || fail "expected '$file' to contain: $expected"
}

require_not_contains() {
    local file="$1"
    local unexpected="$2"
    if grep -Fq "$unexpected" "$file"; then
        fail "expected '$file' not to contain: $unexpected"
    fi
}

run_expected_failure() {
    local label="$1"
    local expected="$2"
    shift 2

    local stdout="$TMP_DIR/${label}.stdout"
    local stderr="$TMP_DIR/${label}.stderr"

    echo
    echo "[first-run] expect-fail ${label}: $*"
    if "$@" >"$stdout" 2>"$stderr"; then
        cat "$stdout"
        cat "$stderr" >&2
        fail "expected ${label} to fail"
    fi
    cat "$stderr"

    require_contains "$stderr" "$expected"
    require_not_contains "$stderr" "check ok"
    require_not_contains "$stderr" "Cannot read"
    require_not_contains "$stderr" "error[E"
    require_not_contains "$stderr" "rustc"
    require_not_contains "$stderr" "panicked"
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

extract_runa_blocks() {
    local source="$1"
    local out_dir="$2"
    awk -v out_dir="$out_dir" '
        BEGIN { in_block = 0; block = 0 }
        /^```runa[[:space:]]*$/ {
            in_block = 1
            block += 1
            file = sprintf("%s/tutorial_01_block_%02d.runa", out_dir, block)
            next
        }
        /^```[[:space:]]*$/ && in_block {
            in_block = 0
            close(file)
            next
        }
        in_block { print > file }
        END { print block > (out_dir "/.block-count") }
    ' "$source"
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

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/futuruna-first-run.XXXXXX")"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

PROJECT_NAME="hello"

cd "$TMP_DIR"
run_step "$RELEASE_RUNA" init "$PROJECT_NAME"

require_file "$PROJECT_NAME/runa.toml"
require_file "$PROJECT_NAME/src/main.runa"
require_contains "$PROJECT_NAME/runa.toml" "name = \"$PROJECT_NAME\""
require_contains "$PROJECT_NAME/runa.toml" "entry = \"src/main.runa\""
require_contains "$PROJECT_NAME/src/main.runa" "Hello from $PROJECT_NAME!"

cd "$TMP_DIR/$PROJECT_NAME"
run_step "$RELEASE_RUNA" check src/main.runa
run_step "$RELEASE_RUNA" fmt --check src/main.runa

echo
echo "[first-run] $RELEASE_RUNA feature-stages --json"
"$RELEASE_RUNA" feature-stages --json > "$TMP_DIR/feature-stages.json"
require_contains "$TMP_DIR/feature-stages.json" "\"schema\": \"futuruna.feature-stages.v1\""
require_contains "$TMP_DIR/feature-stages.json" "\"id\": \"core-cli-workflow\""
require_contains "$TMP_DIR/feature-stages.json" "\"stage\": \"stable\""

echo
echo "[first-run] $RELEASE_RUNA run src/main.runa"
"$RELEASE_RUNA" run src/main.runa > "$TMP_DIR/init-run.out"
cat "$TMP_DIR/init-run.out"
grep -Fxq "Hello from $PROJECT_NAME!" "$TMP_DIR/init-run.out" \
    || fail "generated project did not print the expected greeting"

run_step "$RELEASE_RUNA" build src/main.runa
require_executable "main"

cat > src/greetings.runa <<'RUNA'
-- library-hygiene: importable

@ export
> greeting(name: String) -> String {
    "Hello, " + name + "!"
}

> secret_greeting(name: String) -> String {
    "secret:" + name
}

@ export
> probe() -> String {
    greeting("library")
}
RUNA

cat > src/import_smoke.runa <<'RUNA'
@ import Greeter from ./greetings

= msg = Greeter.greeting("import")
= probe = Greeter.probe()
= ok = msg == "Hello, import!" && probe == "Hello, library!"
? ok

@ print(msg)
RUNA

run_step "$RELEASE_RUNA" check src/import_smoke.runa
run_step "$RELEASE_RUNA" fmt --check src/import_smoke.runa

echo
echo "[first-run] $RELEASE_RUNA run src/import_smoke.runa"
"$RELEASE_RUNA" run src/import_smoke.runa > "$TMP_DIR/import-run.out"
cat "$TMP_DIR/import-run.out"
grep -Fxq "Hello, import!" "$TMP_DIR/import-run.out" \
    || fail "local import smoke did not print the expected greeting"

run_step "$RELEASE_RUNA" build src/import_smoke.runa
require_executable "import_smoke"

cat > src/bad_arrow.runa <<'RUNA'
> add(a: Int, b: Int) => Int { a + b }
RUNA

cat > src/missing_import.runa <<'RUNA'
@ import ./missing_first_hour_helper

= x = 1
RUNA

cat > src/private_import.runa <<'RUNA'
@ import Greeter from ./greetings

= hidden = Greeter.secret_greeting("import")
RUNA

run_expected_failure \
    "bad-arrow" \
    'Futuruna uses `->` for return types, not `=>`.' \
    "$RELEASE_RUNA" check src/bad_arrow.runa

run_expected_failure \
    "missing-import" \
    'cannot resolve import `./missing_first_hour_helper`' \
    "$RELEASE_RUNA" check src/missing_import.runa

run_expected_failure \
    "private-import" \
    'qualified import `Greeter` has no exported member `secret_greeting`' \
    "$RELEASE_RUNA" check src/private_import.runa

TUTORIAL_EXAMPLES="$TMP_DIR/tutorial-01"
mkdir -p "$TUTORIAL_EXAMPLES"
extract_runa_blocks "$ROOT_DIR/docs/tutorial/01-hello.md" "$TUTORIAL_EXAMPLES"
BLOCK_COUNT="$(cat "$TUTORIAL_EXAMPLES/.block-count")"
[[ "$BLOCK_COUNT" -ge 2 ]] || fail "expected at least two runa examples in docs/tutorial/01-hello.md"

for example in "$TUTORIAL_EXAMPLES"/*.runa; do
    run_step "$RELEASE_RUNA" check "$example"
    run_step "$RELEASE_RUNA" fmt --check "$example"
    echo
    echo "[first-run] $RELEASE_RUNA run $example"
    "$RELEASE_RUNA" run "$example" > "$example.out"
    cat "$example.out"
done

echo
echo "[first-run] First-run golden path passed."
