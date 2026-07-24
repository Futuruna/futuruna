---
type: source
status: active
source_kind: repo-docs
tags:
  - source
  - docs
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[current-state]]"
  - "[[verification-lanes]]"
  - "[[proof-kernel]]"
  - "[[verified-bootstrap]]"
---

# Repo Docs

The current repo docs are the canonical written contracts for Futuruna. The wiki should summarize, connect, and operationalize them rather than fork them.

## Core Assurance And Proof Sources

- [[state-and-roadmap]]
  High-level project map: assurance stack, trust boundary, and next milestones.
- [[compatibility-policy]]
  The compatibility contract for source, behavior, verification, artifact surfaces, and feature stages.
- [[compatibility-guides]]
  The release-facing ledger for stable changes, deprecations, and bug-fix exceptions.
- [[feature-stages]]
  The current stage matrix for major language/runtime surfaces and `runa` command families.
- [[mint-gate]]
  The blocking "Futuruna is mint" contract.
- [[canary-matrix]]
  The authored workflow coverage map.
- [[canary-suite]]
  The operational contract for authored, downstream, external, expectation,
  and WASM canary lanes.
- [[differential-testing]]
  The reproducible stress-generation and minimized-corpus lane.
- [[verified-bootstrap-doc]]
  The honest statement of what native proof-backed compiler work means today.
- [[proof-kernel-spec]]
  The kernel design boundary and its v1 logic fragment.
- [[language-reference]]
  The stable/preview split across core language reference pages.
- [[milestone-docs]]
  Historical milestone docs and their current staleness boundary.

## How These Feed The Vault

- [[current-state]] and [[overview]] explain where Futuruna stands now.
- [[verification-lanes]] and [[test-surface]] explain how quality is enforced operationally.
- [[proof-kernel]] and [[verified-bootstrap]] track the formal-methods thread without overclaiming.
- [[mint-ratchet]] records the contributor discipline layer that keeps these docs live.
- [[compatibility-discipline]] and [[compatibility-policy]] make the user-facing change contract explicit.
- [[compatibility-guides]] make that contract cumulative over time instead of PR-local.
- [[feature-stages]] makes those stages visible in day-to-day docs and tooling entry points.
- [[language-surface]] turns the reference docs into a stable-vs-preview map.
- [[differential-testing-flow]] operationalizes replay/minimize/promote behavior.

## Best Next Ingests

- `docs/artifact-codegen-contracts.md`
- `docs/downstream-test-surface-audit.md`
- `docs/library-hygiene.md`
- high-value research notes under `research/`
