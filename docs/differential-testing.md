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
- `FUTURUNA_DIFFERENTIAL_CORPUS`
- `FUTURUNA_DIFFERENTIAL_OUT`

Use those to scale local runs up or down without editing the script.
