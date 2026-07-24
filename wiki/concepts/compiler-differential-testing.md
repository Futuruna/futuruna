---
type: concept
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - concept
  - testing
  - fuzzing
  - compiler
related:
  - "[[compiler-fuzzing-csmith-and-csmithedge]]"
  - "[[differential-testing]]"
  - "[[differential-testing-flow]]"
  - "[[test-surface]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Compiler Differential Testing

Compiler differential testing generates valid programs, runs them through multiple compiler paths or configurations, and treats mismatched behavior as evidence of a bug.

## Effective Form

- generate only well-defined programs or control undefined behavior tightly
- compare interpreter vs compiled output, or one backend vs another, or optimization levels against each other
- save seeds and minimized repros
- turn every real mismatch into a permanent regression

## Why It Still Matters

Even mature compilers keep shipping wrong-code bugs. Fixed suites and hand-written regressions are not enough because they under-sample strange but valid feature combinations.

## Futuruna Implication

Futuruna has a documented differential lane in [[differential-testing]]. The
professional shape is now explicit: replay checked-in minimized repros, run
seeded stress generation, and promote real failures into permanent corpus cases.
The next improvement is reducer quality and broader language-aware generation.

## Primary Sources

- [[compiler-fuzzing-csmith-and-csmithedge]]
