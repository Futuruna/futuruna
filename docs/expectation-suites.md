# Compiletest-Style Expectation Suites

Futuruna now has a small compiletest-style lane for compiler behavior that
should be checked with exact expectations instead of broad end-to-end programs.

The canonical command is:

```bash
./scripts/expectations.sh
```

or directly:

```bash
./target/release/runa expect tests/expect
```

## Purpose

Use expectation cases when the contract is:

- a diagnostic should be emitted
- a command should pass or fail
- a compiler phase should contain a stable structural marker
- a minimized regression should stay small and precise

Do not use this lane for realistic user workflows. Those belong in
`tests/canary/` or `tests/downstream/`.

## Case Format

Each `.runa` case uses source comments as directives:

```runa
-- expect-command: check
-- expect-status: fail
-- expect-stderr: undefined function `missing`
= value = missing()
```

Supported directives:

- `-- expect-command: check|run|interp|emit-rust|emit-fir|verify`
- `-- expect-status: pass|fail`
- `-- expect-stdout: text that must appear on stdout`
- `-- expect-stderr: text that must appear on stderr`
- `-- expect-stdout-file: path/to/stdout.golden`
- `-- expect-stderr-file: path/to/stderr.golden`
- `-- expect-skip: reason`

If `expect-command` is omitted, the command defaults to `check`. If
`expect-status` is omitted, the status defaults to `pass`.

Golden file paths are resolved relative to the `.runa` case. They check the
entire selected output channel exactly, except CRLF and LF newlines are treated
the same. Use substring directives for small markers and golden files when the
reviewable artifact is the full diagnostic, FIR, emitted Rust, or run output.

## Layout

The starter layout is:

- `tests/expect/diagnostics/`
  Parse, type, validation, and compiler diagnostics.

- `tests/expect/run/`
  Minimal interpreted or compiled pass/fail behavior.

- `tests/expect/phase/`
  Phase-specific structural expectations such as FIR or emitted Rust markers.

Future subdirectories should be named by behavior, not by bug number. A bug
number can appear in the file name when that helps traceability.

## Lane Mapping

Expectation suites are the narrow compiler-contract lane:

- `mint`: runs `runa expect tests/expect` as a fast blocking check.
- `canary`: holds authored realistic programs that mix subsystems.
- `downstream-canary`: models library-consumer import usage.
- `differential`: searches for unknown semantic divergence.
- `proof-backed checking`: reduces trust in selected compiler transformations.

When a user reports a compiler bug, choose the smallest permanent lane that
matches the failure:

- exact diagnostic or pass/fail behavior: add `tests/expect/`
- realistic workflow regression: add `tests/canary/`
- library-consumer/import regression: add `tests/downstream/`
- unknown semantic drift: add or minimize into `tests/differential/`
- proof-elaboration trust issue: add proof-backed validation or a proof snapshot
