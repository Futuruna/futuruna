---
type: source
source_type: papers
status: summarized
created: 2026-07-18
updated: 2026-07-18
author: "Yang, Chen, Eide, Regehr; Even-Mendoza, Cadar, Donaldson"
date_published: 2022-07
url: "https://users.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf"
urls:
  - "https://users.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf"
  - "https://link.springer.com/article/10.1007/s10664-022-10146-1"
confidence: high
tags:
  - source
  - compiler
  - fuzzing
  - differential-testing
key_claims:
  - "Csmith found hundreds of previously unknown compiler bugs and showed that fixed suites alone are inadequate quality control."
  - "Differential testing works when generators stay within well-defined semantics."
  - "CsmithEdge shows that refreshing generators and increasing test diversity continues to find bugs even in mature compilers."
related:
  - "[[compiler-differential-testing]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Compiler Fuzzing: Csmith And CsmithEdge

This source cluster captures the strongest practical case for compiler fuzzing and differential testing.

## What It Contributes

- A direct argument that fixed test suites are not enough for compiler quality control.
- A methodology for generating only well-defined programs so output mismatches are meaningful.
- A reminder that generators can saturate and need to evolve to keep finding bugs in mature compilers.

## Relevant Details

- The original Csmith paper reports more than 325 bugs and states that every tested compiler both crashed and silently miscompiled valid inputs.
- The paper argues that fixed suites are inadequate quality control and that differential testing without undefined behavior is a strong oracle for wrong-code bugs.
- CsmithEdge expands the generator space and reports additional bugs in GCC, LLVM, and MSVC, showing that richer, less idiomatic test programs keep paying off after older generators saturate.

## Futuruna Implication

Futuruna's current differential lane is the right direction, but it should evolve toward typed, language-aware generators plus reducers and saved repro corpora instead of remaining a one-off stress tool.

