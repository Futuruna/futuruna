---
type: source
source_type: repo-doc
status: summarized
source_path: "docs/differential-testing.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - testing
  - compiler
  - differential
related:
  - "[[compiler-differential-testing]]"
  - "[[differential-testing-flow]]"
  - "[[test-surface]]"
  - "[[verification-lanes]]"
---

# Differential Testing

This source note summarizes `docs/differential-testing.md`.

Futuruna has a dedicated differential lane for finding compiler bugs before
users report them. The canonical command is:

```bash
./scripts/differential.sh
```

The lane has two parts:

- replay checked-in minimized repros from `tests/differential/corpus/`
- run `runa stress-gen` over stable seeds from `tests/differential/stress_gen_seeds.txt`

## Reproducibility Contract

`runa stress-gen` accepts a fixed `--seed` and can write failure artifacts with
`--save-failures`. A saved failure includes both the generated `.runa` program
and replay metadata: base seed, case index, derived case seed, failure reason,
and replay commands.

## Promotion Workflow

When the lane finds a real bug:

- minimize the generated `.runa` program
- add the minimized repro to `tests/differential/corpus/`
- keep the original stress seed when it still adds useful search coverage
- fix the compiler and make the repro part of routine verification

## Operational Knobs

The script can be scaled or redirected with:

- `RUNA_BIN`
- `FUTURUNA_STRESS_COUNT`
- `FUTURUNA_STRESS_SEEDS_FILE`
- `FUTURUNA_DIFFERENTIAL_CORPUS`
- `FUTURUNA_DIFFERENTIAL_OUT`

## Wiki Implication

This is Futuruna's search lane. It complements [[expectation-suites]], authored
canaries, downstream canaries, and proof-backed checking by exploring valid
program combinations that humans are unlikely to write by hand.
