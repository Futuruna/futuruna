---
type: concept
status: developing
created: 2026-07-18
updated: 2026-07-18
tags:
  - concept
  - verification
  - compiler
related:
  - "[[alive2-translation-validation]]"
  - "[[verified-bootstrap]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Translation Validation

Translation validation checks that a specific compiler transformation preserves meaning for a specific input program or pass result, instead of trying to prove the whole compiler correct at once.

## Why It Is Attractive

- narrower scope than full compiler verification
- useful on high-risk passes first
- can often run continuously on existing compiler tests
- exposes both implementation bugs and semantic ambiguity

## Limits

- usually bounded or scoped to selected transformations
- does not replace testing, compatibility discipline, or broad runtime coverage
- still needs a reasonably precise semantics for the checked fragment

## Futuruna Implication

Futuruna’s proof ambitions become more practical if paired with a translation-validation pilot for one narrow IR or codegen slice. This is a more professional near-term move than claiming whole-compiler proof too early.

## Primary Sources

- [[alive2-translation-validation]]
- [[verified-bootstrap]]

