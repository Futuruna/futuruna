---
type: flow
status: active
created: 2026-07-18
updated: 2026-07-18
tags:
  - flow
  - testing
  - compiler
  - differential
related:
  - "[[differential-testing]]"
  - "[[compiler-differential-testing]]"
  - "[[verification-lanes]]"
  - "[[test-surface]]"
---

# Differential Testing Flow

The differential lane is the search loop for Futuruna compiler bugs.

## Routine Run

```bash
./scripts/differential.sh
```

This replays minimized corpus cases and runs seeded stress generation.

## Found Failure

1. Save the generated program and metadata with `--save-failures`.
2. Reproduce the failure from the saved replay command.
3. Minimize the `.runa` program.
4. Commit the minimized case under `tests/differential/corpus/`.
5. Keep or add the seed when it still explores useful nearby space.
6. Fix the compiler and keep the minimized case in routine verification.

## Lane Boundary

Use this lane for unknown-bug search. Use [[expectation-suites]] for exact
diagnostic or phase contracts, canaries for realistic authored workflows, and
downstream canaries for local-library consumer behavior.
