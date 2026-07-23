# WIKI.md — Futuruna LLM Wiki Overlay

This repository root is also an Obsidian vault. The wiki is superimposed on top of the project so Claude/Codex can use the same workflow shape as `/Users/andreasrudolph/claude-obsidian`, but directly against Futuruna’s docs, source, tests, and research.

## What This Gives You

- persistent wiki pages in `wiki/`
- raw source staging in `.raw/`
- reusable note templates in `_templates/`
- attachments in `_attachments/`
- Obsidian dashboard and Bases view under `wiki/meta/`
- direct compatibility with `/wiki`, `/save`, `/autoresearch`, `/canvas`, and related skills

## Main Structure

```text
.raw/          immutable source staging
wiki/          synthesized knowledge layer
_templates/    note templates
_attachments/  images and PDFs referenced by notes
docs/          canonical project documentation
tests/         executable coverage and canaries
research/      exploratory material
src/           compiler/runtime implementation
```

## How To Use It

1. Open `/Users/andreasrudolph/futuruna` directly as an Obsidian vault.
2. Drop external material to ingest into `.raw/`.
3. Use skills against this repo root:
   - `/wiki`
   - `ingest [filename]`
   - `what do you know about [topic]?`
   - `/save`
   - `/autoresearch [topic]`
   - `lint the wiki`
4. Keep durable synthesis in `wiki/`, not in chat.

## Important Constraint

The repo’s root `CLAUDE.md` remains the agent instruction file for project work. This `WIKI.md` is the vault reference file for the Obsidian overlay.

## Start Here

- [[wiki/getting-started|Getting Started]]
- [[wiki/index|Wiki Index]]
- [[wiki/meta/dashboard|Dashboard]]
- [[wiki/overview|Overview]]

