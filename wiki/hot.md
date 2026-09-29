---
type: meta
title: "Hot Cache"
updated: 2026-07-18
tags:
  - wiki
  - hot
---

# Recent Context

## Key Facts

- Futuruna has an active mint gate and multiple authored canary tiers.
- The production compiler is tested, not formally verified; there is no proof kernel.
- Milestones are tracked in [[board]] as an Obsidian Kanban board with Now / Next / Later / Done lanes; milestones sit above td epics.
- Research synthesis: [[research-hardening-futuruna-into-a-professional-language]]
- Core external lessons: compatibility discipline, ecosystem canaries, compiler differential testing, and narrow translation validation.
- The repo compatibility policy covers stability stages and bug-fix exceptions.
- The stage matrix is surfaced in docs and `runa --help`.
- `runa expect` plus `tests/expect/` check exact compiler expectations (diagnostics, run/fail behavior, phase markers).
- The reference docs split core basics/runes/stdlib as stable and streams/Rust compatibility as preview.
- The differential lane is a replay/minimize/promote loop, not just random stress.
- The canary suite has documented authored, downstream, external, expectation, and WASM lanes.
- Source notes: [[wiki/sources/state-and-roadmap|state-and-roadmap]], [[wiki/sources/mint-gate|mint-gate]], [[wiki/sources/canary-matrix|canary-matrix]], [[wiki/sources/differential-testing|differential-testing]], [[wiki/sources/canary-suite|canary-suite]], [[language-reference]], [[milestone-docs]].

## Active Threads

- Burn down downstream consumer compiler bugs
- Keep expanding authored canaries and downstream-style validation
- Turn more canonical repo docs into linked wiki notes
- Convert the hardening research into concrete Futuruna roadmap tasks and policy docs
- Grow expectation suites for diagnostics, phase snapshots, and minimized compiler regressions
- Keep preview language surfaces moving toward production readiness with explicit contracts and canary-backed coverage
