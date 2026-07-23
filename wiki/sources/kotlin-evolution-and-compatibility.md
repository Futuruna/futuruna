---
type: source
source_type: official-docs
status: summarized
created: 2026-07-18
updated: 2026-07-18
author: "JetBrains / Kotlin"
date_published: 2026-03
url: "https://kotlinlang.org/docs/kotlin-evolution-principles.html"
urls:
  - "https://kotlinlang.org/docs/kotlin-evolution-principles.html"
  - "https://kotlinlang.org/docs/compatibility-guide-23.html"
confidence: high
tags:
  - source
  - kotlin
  - compatibility
  - language-evolution
key_claims:
  - "Kotlin treats language evolution as a balance of modernizing the language, maintaining a user feedback loop, and keeping updates comfortable."
  - "Kotlin publishes explicit compatibility guides that classify source, binary, and behavioral incompatibilities."
  - "Kotlin uses pre-stable feature statuses such as Experimental, Alpha, and Beta to gather production feedback before stabilization."
related:
  - "[[Kotlin]]"
  - "[[compatibility-discipline]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Kotlin Evolution And Compatibility

This source cluster captures Kotlin's public discipline for language change and compatibility.

## What It Contributes

- A clear statement that language evolution must keep the language modern, maintain a feedback loop, and keep upgrades comfortable.
- A concrete compatibility vocabulary: source, binary, and behavioral incompatibility.
- A staged path from preview to stable, with explicit instability for features that are still collecting production feedback.

## Relevant Details

- Kotlin says incompatible changes should be announced in advance, surfaced as compiler warnings where possible, and accompanied by migration help before the break lands.
- Kotlin explicitly uses preview statuses such as Experimental, Alpha, and Beta for battle-testing features in real code before they are frozen.
- Kotlin publishes versioned compatibility guides that list concrete changes, their affected component, the incompatibility type, and the deprecation cycle.

## Futuruna Implication

Futuruna should stop treating compatibility as an implicit social promise. It should publish a compatibility policy with named categories and feature stages, then attach every intentional break to that policy.

