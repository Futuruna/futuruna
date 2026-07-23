---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/compatibility-guides/README.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - compatibility
  - release-notes
related:
  - "[[compatibility-policy]]"
  - "[[feature-stages]]"
  - "[[repo-docs]]"
---

# Compatibility Guides

This source note summarizes the release-facing compatibility guide discipline in
`docs/compatibility-guides/`.

## What It Adds

- a dedicated release-facing ledger for stable breaks, deprecations, bug-fix exceptions, and notable preview changes
- a current rolling guide at `docs/compatibility-guides/0.1.x.md`
- a rule that stable-surface changes should update the current guide instead of living only in PR text

## Why It Matters

The compatibility policy defines categories and feature stages. The
compatibility guides make that policy durable over time for users.

## Best Companion Notes

- [[compatibility-policy]]
- [[feature-stages]]
- [[compatibility-discipline]]

