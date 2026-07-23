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

Futuruna already has a differential lane. The next professional step is to make it more language-aware, more reducer-friendly, and more tied to replayable corpora instead of treating it as a loose stress tool.

## Primary Sources

- [[compiler-fuzzing-csmith-and-csmithedge]]

