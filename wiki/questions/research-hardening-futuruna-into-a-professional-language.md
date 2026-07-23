---
type: synthesis
title: "Research: Hardening Futuruna into a Professional Language"
created: 2026-07-18
updated: 2026-07-18
tags:
  - research
  - compiler
  - quality
  - strategy
status: developing
related:
  - "[[kotlin-evolution-and-compatibility]]"
  - "[[rust-testing-and-stability]]"
  - "[[swift-source-compatibility-and-governance]]"
  - "[[alive2-translation-validation]]"
  - "[[compiler-fuzzing-csmith-and-csmithedge]]"
  - "[[compatibility-discipline]]"
  - "[[ecosystem-canaries]]"
  - "[[compiler-differential-testing]]"
  - "[[translation-validation]]"
  - "[[Kotlin]]"
  - "[[Rust]]"
  - "[[Swift]]"
sources:
  - "[[kotlin-evolution-and-compatibility]]"
  - "[[rust-testing-and-stability]]"
  - "[[swift-source-compatibility-and-governance]]"
  - "[[alive2-translation-validation]]"
  - "[[compiler-fuzzing-csmith-and-csmithedge]]"
---

# Research: Hardening Futuruna into a Professional Language

## Overview

The research converges on one answer: Futuruna becomes professional by combining compatibility discipline, layered testing, curated ecosystem canaries, and narrow formal validation where it pays off. Mature language projects do not rely on one magic mechanism. They stack release/process policy, broad regression infrastructure, real-project coverage, and selective proof or validation techniques. (Source: [[kotlin-evolution-and-compatibility]]) (Source: [[rust-testing-and-stability]]) (Source: [[swift-source-compatibility-and-governance]]) (Source: [[alive2-translation-validation]])

## Key Findings

- Professional language teams name compatibility explicitly instead of leaving it implicit. Kotlin documents source, binary, and behavioral incompatibility, stages features as preview or stable, and attaches breaks to deprecation cycles. (Source: [[kotlin-evolution-and-compatibility]])
- Stable upgrades need an operational model, not just good intentions. Rust frames this as "stability without stagnation" and backs it with release trains plus a large structured test harness. (Source: [[rust-testing-and-stability]])
- Self-authored tests are not enough. Rust and Swift both test against real public projects, with small always-on CI subsets and larger ecosystem sweeps outside the fastest lane. (Source: [[rust-testing-and-stability]]) (Source: [[swift-source-compatibility-and-governance]])
- Fixed regression suites are necessary but inadequate quality control for compilers. Csmith showed that valid, differential, randomly generated programs uncover wrong-code bugs that mature compilers' standard suites miss. CsmithEdge showed that generator diversity must keep evolving after old fuzzers saturate. (Source: [[compiler-fuzzing-csmith-and-csmithedge]])
- Formal methods are most useful when scoped. Alive2 shows that bounded translation validation on selected transformations can find real optimizer bugs and clarify semantics without requiring a fully verified compiler. (Source: [[alive2-translation-validation]])
- Public evolution governance matters. Swift and Kotlin both make feature staging, decision ownership, and compatibility impact legible to contributors and users. (Source: [[swift-source-compatibility-and-governance]]) (Source: [[kotlin-evolution-and-compatibility]])

## What Futuruna Should Do Next

1. Publish a Futuruna compatibility policy.
   Define source, runtime/behavioral, and artifact/codegen compatibility. Define feature states such as experimental, preview, and stable. Define when a compiler bug fix is allowed to break code immediately versus when it must warn first. (Source: [[kotlin-evolution-and-compatibility]]) (Source: [[rust-testing-and-stability]])

2. Add a real downstream ecosystem lane.
   Keep the in-repo canaries, but add curated downstream library-consumer projects and import-heavy exemplars that run outside the core mint gate and partially inside CI. (Source: [[rust-testing-and-stability]]) (Source: [[swift-source-compatibility-and-governance]])

3. Upgrade Futuruna's differential testing from stress tool to infrastructure.
   Use typed program generators, saved seeds, reduction, and explicit corpora of historical failures. Bias generation toward semantic stress shapes that users actually hit: imports, ownership, top-level bindings, effects, actors, and codegen-sensitive transformations. (Source: [[compiler-fuzzing-csmith-and-csmithedge]])

4. Build a compiletest-like expectation surface.
   Keep `mint.sh`, but add more structured suites for expected diagnostics, run/fail behavior, snapshots, and pass-specific expectations instead of encoding all behavior as ad hoc full-program tests. (Source: [[rust-testing-and-stability]])

5. Pilot translation validation on one narrow Futuruna slice.
   Do not wait for whole-compiler proof. Pick one risky transformation or IR/codegen handoff and check source/target equivalence there. (Source: [[alive2-translation-validation]]) (Source: [[verified-bootstrap]])

6. Separate language evolution from ordinary patch flow.
   Maintain a lightweight evolution/governance process for user-visible language changes, especially anything affecting syntax, semantics, diagnostics, or compatibility. (Source: [[swift-source-compatibility-and-governance]]) (Source: [[kotlin-evolution-and-compatibility]])

## Key Entities

- [[Kotlin]]: model for compatibility categories, preview statuses, and release-by-release compatibility guides.
- [[Rust]]: model for structured test infrastructure, release trains, and ecosystem regression testing.
- [[Swift]]: model for source compatibility suites and visible language governance.

## Key Concepts

- [[compatibility-discipline]]: compatibility is a named contract, not a vibe.
- [[ecosystem-canaries]]: real user code belongs in the quality surface.
- [[compiler-differential-testing]]: generated valid programs find bugs fixed suites miss.
- [[translation-validation]]: selective semantic checking is a practical formal-methods bridge.

## Contradictions

- [[kotlin-evolution-and-compatibility]] emphasizes comfortable updates and deprecation cycles, but it also says some compiler bug fixes should land quickly even if they are technically incompatible. The professional reading is not "never break"; it is "classify and justify the break."
- [[alive2-translation-validation]] shows strong value from formal checking, but it is bounded and scoped. [[compiler-fuzzing-csmith-and-csmithedge]] shows why broad testing still matters even when formal techniques exist. The credible strategy is hybrid assurance, not formal methods alone.

## Open Questions

- Which Futuruna compatibility categories should be first-class: source, runtime behavior, emitted Rust shape, binary artifacts, or all of them?
- Which specific Futuruna transformation is the best first translation-validation target?
- How should Futuruna expose experimental versus stable features in user-facing syntax and tooling?

## Sources

- [[kotlin-evolution-and-compatibility]]: JetBrains / Kotlin docs, 2026.
- [[rust-testing-and-stability]]: Rust Project and Rust Foundation docs, 2025-2026.
- [[swift-source-compatibility-and-governance]]: Swift Project docs, 2026.
- [[alive2-translation-validation]]: Alive2 project and PLDI 2021 paper.
- [[compiler-fuzzing-csmith-and-csmithedge]]: PLDI 2011 and Empirical Software Engineering 2022.

