---
type: source
source_type: repo-doc-batch
status: summarized
source_paths:
  - "docs/reference/README.md"
  - "docs/reference/basics.md"
  - "docs/reference/runes.md"
  - "docs/reference/stdlib.md"
  - "docs/reference/streams.md"
  - "docs/reference/rust-compatibility.md"
  - "docs/reference/style.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - language
  - reference
related:
  - "[[language-surface]]"
  - "[[feature-stages]]"
  - "[[stream-lifetimes]]"
  - "[[compatibility-policy]]"
---

# Language Reference

This source note summarizes the selected `docs/reference/` pages plus the style
guide.

The reference splits Futuruna's language surface into stable fundamentals and
preview surfaces:

- `basics.md`, `runes.md`, and `stdlib.md` are listed as stable.
- `streams.md` and `rust-compatibility.md` are listed as preview.

For the full stage matrix, the reference points to [[feature-stages]].

## Stable Core

`basics.md` defines literals, primitive and composite types, operators, control
flow, closures, comments, and the quick built-in surface. It establishes the
current core mental model: value-shaped source code that maps to Rust values,
with `Pair`, `Option`, `Result`, lists, functions, and generic type variables.

`runes.md` is the classification layer. Every statement starts with one of the
seven runes:

- `#` for types, effects, traits, and impls
- `>` for functions, actors, and modules
- `|` for rules, invariants, handlers, scopes, and match arms
- `=` for bindings and monadic bind
- `~` for streams and subscriptions
- `@` for imports, IO, dependencies, exports, comptime, and Rust escape hatches
- `?` for proving or checking invariants

`stdlib.md` documents the built-in surface: display, math, strings, lists,
collections, tuples, maps, sets, stream helpers, options/results, logic, file
IO, process execution, JSON, HTTP, SQLite, concurrency, and comptime type
generation. The notable edge contracts include `head([])` and out-of-range
indexing as runtime errors, while `tail([])` returns `[]`.

## Preview Surfaces

`streams.md` defines `~` stream bindings, push-based `subject()` values,
operators, subscriptions with `|` arms, and scope-owned lifetimes. Its most
important contract is that ordinary functions may not start hidden live
subscriptions unless a named scope owns them. See [[stream-lifetimes]].

`rust-compatibility.md` defines the Rust-facing story: ownership inference,
`inout`, raw Rust blocks, Cargo dependencies, Rust `use` imports, build modes,
Futuruna import forms, type mapping, and actors. It also points at
`docs/artifact-codegen-contracts.md` for the exact boundary: generated Rust
behavior for stable pure/core programs is contractual, while exact helper names
and private layout are internal unless a doc or expectation fixture promises
them.

## Modeling Style

`style.md` treats runes as semantic categories, not decoration. It is especially
strict for law/spec modeling:

- model what the source establishes, not commentary
- use `|` for fixed truths and `>` for computation
- preserve source vocabulary in types and constructors
- use `?` for proofs rather than print-heavy narration
- keep source files, audit files, and source-text blocks separate

## Wiki Implication

The language reference gives the vault a clear stable-vs-preview split:
language basics, runes, and stdlib are the core production surface; streams and
Rust compatibility still need explicit contracts and canary-backed hardening
before the docs should be read as fully stable promises.
