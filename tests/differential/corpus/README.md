# Differential Corpus

Place minimized `.runa` repros here when `runa stress-gen` or external bug reports
find interpreter-vs-compiled mismatches.

The differential lane replays this corpus with:

```bash
./target/release/runa test --roundtrip tests/differential/corpus
```

Guideline:

- keep each file as small as possible
- include only positive programs whose stdout should match in interpreted and compiled mode
- add a short comment at the top describing the original bug
