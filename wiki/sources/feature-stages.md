---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/feature-stages.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - stability
  - feature-stages
related:
  - "[[compatibility-policy]]"
  - "[[compatibility-discipline]]"
  - "[[repo-docs]]"
---

# Feature Stages

This source note summarizes `docs/feature-stages.md`.

## What It Adds

- a current stage matrix for major language/runtime surfaces
- a current stage matrix for major `runa` command families
- an explicit distinction between stable public behavior and unstable internal artifact details

## Most Important Reading

- core syntax and documented stdlib behavior are surfaced as stable
- explicit kernel proof terms are surfaced as stable
- `runa verify` automation is surfaced as preview
- streams/stateful surfaces and Rust interop are surfaced as preview
- the current preview contract for streams includes explicit named-scope
  ownership for live subscriptions
- `runa audit` and `runa from-rust` are surfaced as experimental

## Best Companion Notes

- [[compatibility-policy]]
- [[compatibility-discipline]]
- [[state-and-roadmap]]
