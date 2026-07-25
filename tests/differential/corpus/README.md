# Differential Corpus

Place minimized `.runa` repros here when `runa stress-gen` or external bug reports
find interpreter-vs-compiled mismatches.

The differential lane replays this corpus with:

```bash
./target/release/runa test --roundtrip tests/differential/corpus
```

Import-aware cases live under `imports/`. The consumer entrypoints in that
directory are ordinary roundtrip programs; helper libraries are marked with
`-- library-hygiene: importable` and `-- roundtrip-skip:` so the lane compares
the downstream consumer output while still resolving nested and qualified
imports through those helpers.

Because `runa test --roundtrip` intentionally skips `@ import` entrypoints,
`scripts/differential.sh` also runs the import subcorpus with:

```bash
./target/release/runa test --run tests/differential/corpus/imports
./target/release/runa test --check-codegen tests/differential/corpus/imports
```

This keeps import-aware downstream coverage in the deeper differential lane
rather than relying only on `scripts/downstream-canary.sh`.

The script also generates import-aware seed cases under
`$FUTURUNA_DIFFERENTIAL_GENERATED_IMPORT_DIR`, defaulting to
`$FUTURUNA_DIFFERENTIAL_OUT/generated-imports`. Each stable stress seed gets a
four-file local import graph: exported ADT/accessors, nested flat-import shared
helpers, a qualified policy module, and a consumer entrypoint. The generated
entrypoint carries `-- expect-command: run` plus exact `-- expect-stdout:`
markers, so compiled output is checked as well as import hygiene, compiled
execution, and check-codegen.

Guideline:

- keep each file as small as possible
- include only positive programs whose stdout should match in interpreted and compiled mode
- add a short comment at the top describing the original bug

Seed cases:

| File | Bug class preserved |
| --- | --- |
| `integer_modulo_after_float_helper.runa` | integer `%` lowering after an Int-returning helper that performs Float work |
| `map_entries_pair_lowering.runa` | `map_entries`/`Pair` lowering and tuple-field access parity |
| `string_list_helper_reuse.runa` | read-only string/list helper chains should not consume reused values |
| `list_literal_reuse_clone.runa` | list literals must clone reused values before later reads |
| `imports/import_mesh_consumer.runa` | nested flat imports, qualified imports, exported ADTs/functions/values, and named HOF callbacks in the differential lane |
