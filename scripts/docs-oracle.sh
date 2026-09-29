#!/usr/bin/env bash
# Run every documentation example that states its expected output.
#
# A ```runa block is checked when each top-level `@ print(...)` line ends with
# a `-- expected output` comment and the block prints nowhere else; the
# program must exit successfully and print exactly those lines. Blocks of one
# page share a project directory: a
# block whose first line is `-- path/name.runa` is written to that path, so
# multi-file examples import each other. Blocks with `@ rust` run natively
# (`runa run`); all others run in the interpreter. Each checked block that
# `runa fmt` changes is run again after formatting and must print the same.
#
# Usage: scripts/docs-oracle.sh [markdown files...]
# Defaults to docs/tutorial/*.md and docs/reference/*.md.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUNA="${RUNA_BIN:-$ROOT_DIR/target/release/runa}"
if [[ ! -x "$RUNA" ]]; then
    echo "docs-oracle: $RUNA is missing; run cargo build --release or set RUNA_BIN" >&2
    exit 1
fi

if [[ $# -eq 0 ]]; then
    set -- docs/tutorial/*.md docs/reference/*.md
fi

RUNA="$RUNA" python3 - "$@" <<'PY'
import os, re, shutil, subprocess, sys, tempfile

runa = os.environ["RUNA"]
print_line = re.compile(r"^@ print\(.*\)\s+--\s+(.*\S)\s*$")
file_header = re.compile(r"^--\s+([\w./-]+\.runa)\s*$")


def blocks(markdown):
    lines = open(markdown, encoding="utf-8").read().split("\n")
    index = 0
    while index < len(lines):
        if lines[index].strip() == "```runa":
            start = index + 1
            end = start
            while end < len(lines) and lines[end].strip() != "```":
                end += 1
            yield start + 1, lines[start:end]
            index = end
        index += 1


def expected_output(body):
    expected = []
    for line in body:
        if line.startswith("@ print("):
            match = print_line.match(line)
            if not match:
                return None
            expected.append(match.group(1))
        elif "@ print(" in line:
            return None
    return expected or None


def run(path, native, cwd):
    command = [runa, "run", path] if native else [runa, path]
    environment = dict(os.environ, FUTURUNA_DISABLE_COMPILER_CACHE="1")
    try:
        result = subprocess.run(command, cwd=cwd, capture_output=True, text=True,
                                timeout=300, env=environment)
    except subprocess.TimeoutExpired:
        return None, "timed out"
    return result.returncode, result.stdout + ("\n" + result.stderr if result.returncode else "")


failures = []
checked = 0
for markdown in sys.argv[1:]:
    project = tempfile.mkdtemp(prefix="futuruna-docs-oracle-")
    try:
        cases = []
        for line_number, body in blocks(markdown):
            header = file_header.match(body[0]) if body else None
            relative = header.group(1) if header else f"block-{line_number}.runa"
            path = os.path.normpath(os.path.join(project, relative))
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write("\n".join(body) + "\n")
            expected = expected_output(body)
            if expected is not None:
                native = any(line.lstrip().startswith("@ rust") for line in body)
                cases.append((line_number, relative, expected, native))
        for line_number, relative, expected, native in cases:
            checked += 1
            location = f"{markdown}:{line_number}"
            status, output = run(relative, native, project)
            actual = [line.rstrip() for line in (output or "").split("\n")] if status == 0 else None
            while actual and actual[-1] == "":
                actual.pop()
            if actual != expected:
                failures.append(location)
                print(f"FAIL {location} ({'native' if native else 'interpreter'})")
                print("  expected: " + " | ".join(expected))
                print("  actual:   " + (" | ".join(actual) if actual is not None
                                        else f"exit {status}: {output.strip()[:400]}"))
                continue
            formatted = relative + ".fmt.runa"
            formatted_path = os.path.join(project, formatted)
            shutil.copyfile(os.path.join(project, relative), formatted_path)
            subprocess.run([runa, "fmt", formatted_path], capture_output=True)
            if open(formatted_path, encoding="utf-8").read() != open(os.path.join(project, relative), encoding="utf-8").read():
                formatted_status, formatted_output = run(formatted, native, project)
                if formatted_status != status or formatted_output != output:
                    failures.append(location + " (after fmt)")
                    print(f"FAIL {location}: output changed after runa fmt")
                    continue
            os.remove(formatted_path)
            print(f"ok   {location}")
    finally:
        shutil.rmtree(project, ignore_errors=True)

print(f"\ndocs-oracle: {checked - len(failures)} of {checked} documented outputs match")
if failures:
    for failure in failures:
        print(f"  - {failure}")
    sys.exit(1)
PY
