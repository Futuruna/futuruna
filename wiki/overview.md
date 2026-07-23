---
type: overview
title: "Futuruna Vault Overview"
created: 2026-07-18
updated: 2026-07-18
tags:
  - wiki
  - overview
related:
  - "[[current-state]]"
  - "[[repo-map]]"
---

# Futuruna Vault Overview

Futuruna is both:

- a compiler and runtime in Rust
- a language design and proof project
- a growing quality system built around mint gates, canaries, audits, and proof-backed slices

This vault is meant to make those threads easier to navigate without replacing the source tree.

## What Lives Where

- `docs/` holds the canonical project docs
- `tests/` holds executable language and compiler coverage
- `research/` holds exploratory design and analysis
- `paper/` holds publication material
- `wiki/` is the Obsidian knowledge layer that cross-links the moving parts

## Current Emphasis

- semantic hardening of codegen and runtime behavior
- authored canary coverage
- proof-kernel and verified-bootstrap expansion
- downstream-user bug burn-down

## Start Here

- [[current-state]]
- [[verification-lanes]]
- [[proof-kernel]]
- [[test-surface]]

