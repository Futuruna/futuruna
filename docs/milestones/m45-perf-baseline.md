# M45: Performance Baseline

**Tagline:** "Measure twice, optimize once."

## Goal

Establish compile-time and runtime performance baselines so regressions
are detectable. No optimization work — just measurement.

## Metrics

- Compile time: `runa build examples/weather_demo.runa` wall clock
- Interpreter time: `runa examples/weather_demo.runa` wall clock
- Test suite time: `runa test` wall clock
- Binary size: compiled weather_demo binary size
- Memory: peak RSS during compilation

## Approach

Add `runa bench` command that runs standard benchmarks and reports metrics.
Store results in `.runa-bench/` for comparison across commits.
