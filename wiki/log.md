---
type: log
title: "Futuruna Wiki Log"
created: 2026-07-18
updated: 2026-09-24
tags:
  - wiki
  - log
---

# Futuruna Wiki Log

## 2026-09-24 maintenance | Repository structure

- Grouped workflow instructions, skills, templates, attachments, and raw-source
  scaffolding under `wiki/`; the vault root remains the repository root.
- Qualified links where note names overlap with maintained documentation.
- Added [[docs/repository-layout|Repository layout]] and [[docs/README|Documentation]].
- Retained the original migration paths and artifact recovery information in
  `docs/repository-migrations.json`.

## 2026-07-18 ingest | Second-wave Futuruna repo docs

- Source: `docs/differential-testing.md`, `docs/canary-suite.md`, selected `docs/reference/`, and selected `docs/milestones/`
- Summary: [[wiki/sources/differential-testing|differential-testing]], [[wiki/sources/canary-suite|canary-suite]], [[language-reference]], [[milestone-docs]]
- Pages created: [[language-surface]], [[differential-testing-flow]]
- Pages updated: [[repo-docs]], [[test-surface]], [[verification-lanes]], [[compiler-pipeline]], [[hot]], [[index]]
- Key insight: the docs now distinguish stable language core, preview language surfaces, authored canaries, differential search, and milestone history instead of flattening all assurance material into one bucket.

## 2026-07-18 board | Milestone Kanban

- Added [[board]] as the strategic milestone tracker, above td-epic level
- Lanes: Now (shipping), Next (queued), Later (planned), Done (shipped)
- Linked from [[index]] and [[wiki/meta/dashboard|meta/dashboard]]; called out in [[hot]]

## 2026-07-18 policy | Futuruna compatibility policy

- Added `docs/compatibility-policy.md`
- Defined compatibility categories, feature stages, deprecation expectations, and bug-fix exceptions
- Synced contributor-facing entry points and added source note [[wiki/sources/compatibility-policy|compatibility-policy]]

## 2026-07-18 policy | Surface feature stages

- Added `docs/feature-stages.md`
- Surfaced stage information in `docs/reference/README.md`, `README.md`, `docs/state-and-roadmap.md`, and `runa --help`
- Added source note [[wiki/sources/feature-stages|feature-stages]]

## 2026-07-18 policy | Compatibility guides

- Added `docs/compatibility-guides/README.md` and rolling guide `docs/compatibility-guides/0.1.x.md`
- Updated contributor/review entry points so stable-surface changes should update the current guide
- Added source note [[compatibility-guides]]

## 2026-07-18 autoresearch | Hardening Futuruna into a Professional Language

- Rounds: 2
- Sources found: 5
- Pages created: [[research-hardening-futuruna-into-a-professional-language]], [[kotlin-evolution-and-compatibility]], [[rust-testing-and-stability]], [[swift-source-compatibility-and-governance]], [[alive2-translation-validation]], [[compiler-fuzzing-csmith-and-csmithedge]], [[compatibility-discipline]], [[ecosystem-canaries]], [[compiler-differential-testing]], [[translation-validation]], [[Kotlin]], [[Rust]], [[Swift]]
- Synthesis: [[research-hardening-futuruna-into-a-professional-language]]
- Key finding: professional language projects combine explicit compatibility policy, real-project ecosystem testing, and selective formal validation instead of relying on any one mechanism.

## 2026-07-18

- Initialized the Futuruna Obsidian vault at repo root
- Added hybrid repository/research wiki structure under `wiki/`
- Seeded core navigation, current-state, compiler, proof, and verification notes
- Added `wiki/_templates/` and `.obsidian/snippets/vault-colors.css`
- Added `WIKI.md`, onboarding/dashboard notes, comparisons/canvases/attachments scaffolding, and shared plugin config so the vault behaves like the claude-obsidian overlay pattern
- Ingested the core repo docs into linked source notes for roadmap, proof kernel, verified bootstrap, mint gate, and canary coverage
- Expanded the seed notes so the wiki now reflects the actual assurance stack and proof trust boundary
