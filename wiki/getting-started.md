---
type: meta
title: "Getting Started"
created: 2026-07-18
updated: 2026-09-24
tags:
  - meta
  - onboarding
status: evergreen
related:
  - "[[index]]"
  - "[[overview]]"
  - "[[dashboard]]"
---

# Getting Started with the Futuruna Wiki Overlay

This vault is a superimposed knowledge layer on top of the Futuruna project. It is meant to let implementation work, research, proof work, and external bug reports accumulate into a persistent wiki instead of disappearing into chat history.

## Three-Step Quick Start

### 1. Open the repo as a vault

Open the Futuruna repository root directly in Obsidian, rather than the `wiki/`
subdirectory. Links to `docs/`, `src/`, and research material use the same vault.

The repo already ships with shared vault config for:

- graph view focused on `wiki/`
- the wiki color snippet
- the ITS Dataview/Image snippets used by the reference vault
- Bases enabled for the dashboard

### 2. Put material in the right place

- project docs already live in `docs/`, `research/`, `paper/`, and `tests/`
- external material to ingest should go in `wiki/.raw/`
- reusable images and PDFs for notes should go in `wiki/_attachments/`

### 3. Use the wiki workflows

Repository copies of the workflows live in `wiki/skills/`. Their paths are
relative to the repository root. When using a separately installed generic
wiki skill, follow these repository-specific locations for sources, templates,
and attachments.

- `ingest [filename]`
- `what do you know about [topic]?`
- `/save`
- `/autoresearch [topic]`
- `lint the wiki`

## Manual Obsidian Steps Still Worth Doing

- install or enable the community plugins you actually want, especially Dataview, Templater, and Obsidian Git
- set Templater's template folder to `wiki/_templates` if you use it; shared
  attachment settings already point to `wiki/_attachments`
- if you want REST-backed vault automation, install Local REST API and wire MCP as described in [[wiki/workflows|Wiki workflows]]

## Good Navigation Pages

- [[index]]
- [[dashboard]]
- [[current-state]]
- [[compiler-pipeline]]
- [[wiki/modules/proof-kernel|proof-kernel]]
- [[verification-lanes]]

## What This Vault Is Best For

- compiler and runtime design synthesis
- proof/bootstrap trust-boundary tracking
- canary and verification coverage mapping
- downstream bug intake and durable lessons
