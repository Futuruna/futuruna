---
type: source
source_type: official-docs
status: summarized
created: 2026-07-18
updated: 2026-07-18
author: "Rust Project / Rust Foundation"
date_published: 2025-05
url: "https://rustc-dev-guide.rust-lang.org/tests/intro"
urls:
  - "https://rustc-dev-guide.rust-lang.org/tests/intro"
  - "https://rustc-dev-guide.rust-lang.org/tests/compiletest.html"
  - "https://rustc-dev-guide.rust-lang.org/tests/ecosystem.html"
  - "https://doc.rust-lang.org/beta/book/appendix-07-nightly-rust.html"
  - "https://rustfoundation.org/media/10-years-of-stable-rust-an-infrastructure-story/"
confidence: high
tags:
  - source
  - rust
  - testing
  - stability
key_claims:
  - "Rust combines a large structured compiler test harness with ecosystem testing against real projects."
  - "Rust frames upgrades around 'stability without stagnation' and a release-train model."
  - "Rust regression-tests releases against a significant fraction of the public crate ecosystem."
related:
  - "[[Rust]]"
  - "[[compatibility-discipline]]"
  - "[[ecosystem-canaries]]"
  - "[[research-hardening-futuruna-into-a-professional-language]]"
---

# Rust Testing And Stability

This source cluster captures how Rust combines compatibility policy, compiler test infrastructure, and ecosystem regression testing.

## What It Contributes

- A release-train model that makes stable upgrades routine instead of dramatic.
- A layered compiler test system built around `compiletest`, package tests, style checks, docs, performance, and ecosystem testing.
- A serious habit of regression testing against public crates and large open-source projects.

## Relevant Details

- Rust's compiler guide describes `compiletest` as the main compiler harness and organizes thousands of tests into suites with directives and expected outputs.
- The ecosystem testing guide describes both broad sweeps such as Crater and smaller always-on CI canaries such as `cargotest` and builders for large OSS projects.
- The Rust book defines the governance goal as "stability without stagnation": stable upgrades should be painless while still shipping features and fixes.
- The 2025 Rust Foundation infrastructure retrospective says every release has passed an exhaustive testsuite and has been regression-tested against a significant fraction of the public crate ecosystem.

## Futuruna Implication

Futuruna should keep its mint gate small, but it also needs a compiletest-like structured expectation surface and a real downstream ecosystem lane that is not limited to self-authored examples.

