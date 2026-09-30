---
type: source
status: active
source_kind: repo-docs
tags:
  - source
  - docs
created: 2026-07-18
updated: 2026-09-27
related:
  - "[[current-state]]"
  - "[[verification-lanes]]"
---

# Repo Docs

The current repo docs are the canonical written contracts for Futuruna. The wiki should summarize, connect, and operationalize them rather than fork them.

## Core Assurance Sources

- [[wiki/sources/state-and-roadmap|state-and-roadmap]]
  High-level project map: assurance stack, trust boundary, and next milestones.
- [[wiki/sources/compatibility-policy|compatibility-policy]]
  The compatibility contract for source, behavior, verification, artifact surfaces, and feature stages.
- [[wiki/sources/feature-stages|feature-stages]]
  The current stage matrix for major language/runtime surfaces and `runa` command families.
- [[wiki/sources/mint-gate|mint-gate]]
  The blocking "Futuruna is mint" contract.
- [[wiki/sources/canary-matrix|canary-matrix]]
  The authored workflow coverage map.
- [[wiki/sources/canary-suite|canary-suite]]
  The operational contract for authored, downstream, external, expectation,
  and WASM canary lanes.
- [[wiki/sources/differential-testing|differential-testing]]
  The reproducible stress-generation and minimized-corpus lane.
- [[language-reference]]
  The stable/preview split across core language reference pages.
- [[milestone-docs]]
  Milestone docs and their staleness boundary.

## How These Feed The Vault

- [[current-state]] and [[overview]] explain where Futuruna stands.
- [[verification-lanes]] and [[test-surface]] explain how quality is enforced operationally.
- [[mint-ratchet]] records the contributor discipline layer that keeps these docs live.
- [[compatibility-discipline]] and [[wiki/sources/compatibility-policy|compatibility-policy]] make the user-facing change contract explicit.
- [[wiki/sources/feature-stages|feature-stages]] makes those stages visible in day-to-day docs and tooling entry points.
- [[language-surface]] turns the reference docs into a stable-vs-preview map.
- [[differential-testing-flow]] operationalizes replay/minimize/promote behavior.

## Best Next Ingests

- `docs/artifact-codegen-contracts.md`
- `docs/library-hygiene.md`
- high-value research notes under `research/`
