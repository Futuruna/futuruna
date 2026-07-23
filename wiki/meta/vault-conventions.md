---
type: meta
title: "Vault Conventions"
created: 2026-07-18
updated: 2026-07-18
tags:
  - wiki
  - meta
related:
  - "[[index]]"
  - "[[repo-map]]"
---

# Vault Conventions

This vault is layered on top of the Futuruna repository root.

## Important Constraint

The repo already has a root `CLAUDE.md` for agent instructions. This vault does not replace or overwrite that file.

## Working Model

- source of truth for implementation remains the repo
- source of truth for durable synthesis can live in `wiki/`
- raw imports belong in `.raw/`
- the vault should link to existing docs instead of copying them blindly

## Note Rules

- use frontmatter with `type`, `created`, `updated`, and `tags`
- use wikilinks like `[[current-state]]`
- log major vault operations in [[log]]
- keep [[hot]] short and overwrite it instead of journaling there

## Suggested Usage

- document stable architecture and design threads in `wiki/modules/`, `wiki/decisions/`, and `wiki/thesis/`
- file synthesized answers under `wiki/questions/`
- use `wiki/sources/` to summarize important docs, papers, or bug reports

