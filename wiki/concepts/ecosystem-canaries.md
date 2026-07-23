---
type: concept
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - concept
  - testing
  - ecosystem
related:
  - "[[Rust]]"
  - "[[Swift]]"
  - "[[test-surface]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Ecosystem Canaries

Ecosystem canaries are curated real projects that a language/compiler builds or tests continuously to catch regressions in user-shaped code, not just synthetic fixtures.

## Why They Matter

Compiler teams miss bugs when they only test self-authored examples and minimized regressions. Real projects stress import shapes, dependency graphs, ownership patterns, and API evolution paths that toy suites underrepresent.

## Strong Pattern

- small always-on curated projects in CI
- larger periodic sweeps outside the fastest blocking lane
- explicit maintainership or pinned revisions
- clear expectations about what counts as a compatibility regression

## Futuruna Implication

Futuruna should keep authored in-repo canaries, but also add a true downstream library-consumer lane with curated external-style projects or repo-split exemplars that behave like users.

## Primary Sources

- [[rust-testing-and-stability]]
- [[swift-source-compatibility-and-governance]]

