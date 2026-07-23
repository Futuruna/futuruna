---
type: meta
title: "Hot Cache"
updated: 2026-07-18
tags:
  - wiki
  - hot
---

# Recent Context

## Last Updated

2026-07-18. The vault now includes an autoresearch package on how professional language/compiler projects harden themselves over time, grounded in Kotlin, Rust, Swift, Alive2, Csmith, and CsmithEdge.

## Key Recent Facts

- Futuruna has an active mint gate and multiple authored canary tiers.
- The proof kernel is real and already proving bootstrap slices, but the production compiler is not yet fully verified.
- Recent work has focused on downstream-user compiler bugs, semantic parity, and more realistic test coverage.
- The wiki now has dedicated source notes for `state-and-roadmap`, `proof-kernel`, `verified-bootstrap`, `mint-gate`, and `canary-matrix`.
- New research synthesis: [[research-hardening-futuruna-into-a-professional-language]]
- Core external lessons: compatibility discipline, ecosystem canaries, compiler differential testing, and narrow translation validation.
- Futuruna now has a first-class repo compatibility policy covering stability stages and bug-fix exceptions.
- Futuruna now also surfaces the current stage matrix in docs and `runa --help` instead of leaving stages implicit.

## Recent Changes

- Created: [[overview]], [[repo-map]], [[current-state]], [[compiler-pipeline]], [[proof-kernel]], [[test-surface]], [[verification-lanes]], [[mint-ratchet]], [[repo-docs]], [[vault-conventions]]
- Created: [[getting-started]], [[dashboard]], [[comparisons/_index]], [[WIKI]]
- Created indexes for modules, decisions, dependencies, flows, concepts, entities, thesis, gaps, questions, sources, and meta
- Added shared Obsidian config for `bases`, `canvas`, and optional community plugins
- Added Obsidian CSS snippet `vault-colors`
- Ingested core repo docs into [[state-and-roadmap]], [[proof-kernel-spec]], [[verified-bootstrap-doc]], [[mint-gate]], and [[canary-matrix]]
- Added thesis note [[verified-bootstrap]] and expanded the main seed notes around it
- Added source notes [[kotlin-evolution-and-compatibility]], [[rust-testing-and-stability]], [[swift-source-compatibility-and-governance]], [[alive2-translation-validation]], and [[compiler-fuzzing-csmith-and-csmithedge]]
- Added concepts [[compatibility-discipline]], [[ecosystem-canaries]], [[compiler-differential-testing]], and [[translation-validation]]
- Added entities [[Kotlin]], [[Rust]], and [[Swift]]
- Added repo policy doc `docs/compatibility-policy.md` and source note [[compatibility-policy]]
- Added `docs/feature-stages.md`, source note [[feature-stages]], and a feature-stage block in CLI help

## Active Threads

- Burn down downstream consumer compiler bugs before they become issue churn
- Keep expanding authored canaries and downstream-style validation
- Shrink the proof trust boundary over time
- Turn more canonical repo docs into linked wiki notes instead of leaving them as unconnected files
- Convert the new hardening research into concrete Futuruna roadmap tasks and policy docs
