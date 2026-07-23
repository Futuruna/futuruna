---
type: concept
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - concept
  - compatibility
  - language-evolution
related:
  - "[[Kotlin]]"
  - "[[Rust]]"
  - "[[Swift]]"
  - "[[mint-ratchet]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Compatibility Discipline

Compatibility discipline is the practice of naming what kinds of breakage exist, staging intentional change, and making upgrades predictable for users.

## What Professional Projects Do

- classify compatibility explicitly, often as source, binary, and behavioral compatibility
- stage new features behind named unstable or preview statuses
- warn before breaking where possible
- publish compatibility guides or migration notes per release
- separate bug fixes from silent semantic drift

## Why It Matters For Futuruna

Futuruna already has a quality ratchet, but it still lacks a first-class compatibility policy. Without one, every behavior change gets debated from scratch, and users have no stable mental model for what an upgrade can break.

## Immediate Futuruna Move

Define:

- compatibility categories
- feature statuses
- deprecation / migration cycle
- criteria for "bug fix now" versus "warn now, break later"

## Primary Sources

- [[kotlin-evolution-and-compatibility]]
- [[rust-testing-and-stability]]
- [[swift-source-compatibility-and-governance]]

