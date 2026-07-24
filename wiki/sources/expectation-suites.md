---
type: source
source_type: repo-doc
status: summarized
source_path: "docs/expectation-suites.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - testing
  - compiler
related:
  - "[[test-surface]]"
  - "[[mint-gate]]"
  - "[[canary-matrix]]"
---

# Expectation Suites

This source note summarizes `docs/expectation-suites.md`.

Futuruna has a compiletest-style expectation lane for narrow compiler behavior.
The canonical command is `./scripts/expectations.sh`, and the direct command is
`./target/release/runa expect tests/expect`.

Use this lane for:

- diagnostics
- command pass/fail behavior
- phase-specific output markers
- minimized compiler regressions

Do not use this lane for realistic user workflows or library-consumer behavior.
Those belong in `tests/canary/` and `tests/downstream/`.

The core directives are:

- `-- expect-command: check|run|interp|emit-rust|emit-fir|verify`
- `-- expect-status: pass|fail`
- `-- expect-stdout: text`
- `-- expect-stderr: text`
- `-- expect-skip: reason`

This narrows the gap called out by [[rust-testing-and-stability]]: Futuruna now
has a first-class surface for exact compiler expectations instead of encoding
all behavior as ad hoc full-program tests.

