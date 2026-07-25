---
feature_stage: stable
feature_stage_surfaces:
  - differential-generative-testing
---

# Differential Testing

Futuruna now has a dedicated differential lane for catching compiler bugs before
users do.

The canonical command is:

```bash
./scripts/differential.sh
```

It does two things:

1. Replays checked-in minimized repros from `tests/differential/corpus/`.
2. Runs `runa stress-gen` with a stable seed list from
   `tests/differential/stress_gen_seeds.txt`.
3. Generates import-aware seed cases under a temporary output directory and
   runs import hygiene, compiled execution, check-codegen, and exact compiled
   stdout expectations against each generated case.

## Stable Contract

The production-ready contract for this lane is intentionally bounded:

- `runa stress-gen` accepts a count, `--seed`, and `--save-failures`.
- A fixed seed and count must produce a reproducible stream of generated cases
  on the supported toolchain.
- Generated failures must write replayable source and metadata artifacts under
  the configured failure directory.
- `./scripts/differential.sh` is the canonical blocking lane for replaying the
  checked-in corpus, stable stress seeds, authored import corpus, and generated
  import-aware cases.
- Skips must be explicit. Generic roundtrip skips for imported helper files are
  not counted as parity evidence; import entrypoints instead run through
  compiled execution, check-codegen, import hygiene, and exact run expectations.

The internal generator grammar, generated source text shape, number of checked
cases, and corpus contents may grow as compiler bugs are found.

The corpus also contains an import-aware subcorpus under
`tests/differential/corpus/imports/`. Because generic roundtrip intentionally
skips `@ import` entrypoints, the differential script runs that subcorpus with
compiled execution and `test --check-codegen` so nested and qualified local
imports get deeper replay coverage beyond the authored downstream canary lane.

The generated import-aware cases are derived from the same stable seed list.
For each seed, the script writes a small four-file import graph under
`$FUTURUNA_DIFFERENTIAL_GENERATED_IMPORT_DIR` or
`$FUTURUNA_DIFFERENTIAL_OUT/generated-imports`:

- an exported ADT/accessor module
- a nested flat-import shared module
- a qualified policy module
- a consumer entrypoint with `-- expect-command: run` and exact
  `-- expect-stdout:` markers

That makes the generated lane exercise more than single-file stress programs:
flat imports, qualified imports, exported values/types/functions, list/map
helpers, compiled execution, Rust codegen, and exact output matching all have to
stay green.

## Reproducible Stress Generation

`runa stress-gen` accepts:

```bash
runa stress-gen 100 --seed 42 --save-failures /tmp/futuruna-diff
```

- `--seed` fixes the random program stream.
- `--save-failures` writes failing programs and replay metadata to disk.

Saved failure artifacts include:

- `<stem>.runa` with the generated program
- `<stem>.txt` with the base seed, case index, derived case seed, failure reason,
  and replay commands

## Promoting a Found Bug

When the differential lane finds a real compiler bug:

1. Minimize the saved `.runa` file.
2. Add the minimized repro to `tests/differential/corpus/`.
3. Keep the original stress seed in `tests/differential/stress_gen_seeds.txt` if it
   still covers useful search space.
4. Fix the compiler and make the minimized repro part of routine verification.

## Environment Knobs

`./scripts/differential.sh` respects:

- `RUNA_BIN`
- `FUTURUNA_STRESS_COUNT`
- `FUTURUNA_STRESS_SEEDS_FILE`
- `FUTURUNA_STRESS_RUN_TIMEOUT_SECONDS`
- `FUTURUNA_DIFFERENTIAL_CORPUS`
- `FUTURUNA_DIFFERENTIAL_OUT`
- `FUTURUNA_DIFFERENTIAL_GENERATED_IMPORT_DIR`

Use those to scale local runs up or down without editing the script. The
compiled spot-check timeout defaults to 10 seconds so loaded machines do not
turn ordinary small generated programs into false differential failures.
