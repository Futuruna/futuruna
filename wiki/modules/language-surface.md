---
type: module
path: "docs/reference/"
status: active
language: markdown
purpose: "Canonical language surface reference and stage split for core syntax, runes, stdlib, streams, and Rust compatibility."
maintainer: "Futuruna"
last_updated: 2026-07-18
tags:
  - module
  - language
  - docs
created: 2026-07-18
updated: 2026-07-18
related:
  - "[[language-reference]]"
  - "[[feature-stages]]"
  - "[[compatibility-policy]]"
  - "[[stream-lifetimes]]"
---

# Language Surface

The reference docs define the user-facing Futuruna surface. They are split
between stable core language promises and preview surfaces that still need
contract hardening.

## Stable Reference Core

- `docs/reference/basics.md`: literals, types, operators, control flow,
  closures, and comments
- `docs/reference/runes.md`: the seven top-level statement categories
- `docs/reference/stdlib.md`: built-in functions and their documented edge
  behavior

## Preview Reference Surface

- `docs/reference/streams.md`: streams, subjects, subscriptions, and scoped
  lifetime management
- `docs/reference/rust-compatibility.md`: ownership inference, generated Rust
  behavior, build modes, imports, dependencies, and escape hatches

## Production-Readiness Read

Treat the stable core as the language area to keep mint and expectation-backed.
Treat streams and Rust compatibility as preview areas until their contracts are
fully reflected in [[stream-lifetimes]], `docs/artifact-codegen-contracts.md`,
canaries, and minimized expectation fixtures.

## Source Notes

- [[language-reference]]
- [[feature-stages]]
- [[compatibility-policy]]
