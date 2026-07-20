# M45: Performance Baseline

**Tagline:** "Measure twice, optimize once."

**Status:** DONE.

## Result

`runa bench` runs standard benchmarks and reports metrics:

```
── Interpreter ──
  Weather Demo (all 7 runes)                    0.9ms
  Cocktails (24 Datalog recipes)               26.5ms
  Self-hosting Lexer (~300 lines)              71.9ms

── Parse + Type-check ──
  Weather Demo (all 7 runes)                  0.113ms
  Cocktails (24 Datalog recipes)              0.135ms
  Self-hosting Lexer (~300 lines)             0.464ms

── Codegen (emit Rust) ──
  Weather Demo (all 7 runes)                    1.7ms  (121 → 230 lines)
  Cocktails (24 Datalog recipes)                7.0ms  (177 → 173 lines)
  Self-hosting Lexer (~300 lines)              58.6ms  (348 → 730 lines)

── Test Suite ──
  runa test (interpreter, all)                  294ms  (61 files)

── From-rust Transpiler ──
  Transpile all .rs files                     266.8ms  (27 files, 1352 lines)

── Binary Size ──
  runa compiler binary                          6.2 MB
```

## Benchmarks

- **Interpreter**: 5 runs, median. 0.9ms for weather demo, 71ms for lexer.
- **Parse**: 10 runs, median. Sub-millisecond for all programs.
- **Codegen**: 5 runs, median. Includes ownership analysis + Rust emission.
- **Test suite**: All 61 clean test files interpreted.
- **Transpiler**: All 27 from-rust .rs files transpiled via syn.
- **Binary**: Release build of the runa compiler.
