# Artifact And Codegen Contracts

This document defines which emitted Rust and generated artifacts Futuruna treats
as public contract, which surfaces are preview contracts, and which details are
internal compiler implementation.

Read this with [compatibility-policy.md](compatibility-policy.md) and
[feature-stages.md](feature-stages.md).

## Contract Tiers

| Surface | Stage | Contract |
|---------|-------|----------|
| `runa emit` command behavior | Stable | For accepted source, the command prints generated Rust to stdout, reports command metadata on stderr, and exits nonzero for parse/type/compiler errors. |
| Pure/core generated Rust artifact shape | Preview | For stable pure/core source, generated Rust must compile on the supported Rust toolchain and preserve documented Futuruna behavior. Exact text is not stable unless covered by an artifact expectation fixture. |
| Artifact expectation fixtures | Snapshot-stable | Files under `tests/expect/artifact/` and their golden files are reviewed emitted-artifact contracts. Any diff is a compatibility-facing change that must be intentional and documented. |
| `runa build` native binary output | Stable command behavior, unstable path internals | The command produces a runnable native binary or reports a compiler bug if generated Rust does not compile. Cache paths, temporary Rust filenames, and internal build layout are not public contract. |
| `runa lib` Rust-facing export shape | Preview | `@ export` functions and types are emitted as public Rust items using the documented type mapping; unexported helpers remain private; no binary `fn main` is emitted. Exact helper layout remains internal. |
| `runa wasm` output | Preview | WASM export validation and build are supported, but package file layout, JS glue shape, and required toolchain policy are not stable yet. |
| Helper names, private modules, formatting, internal prelude layout | Unstable internal | These may change without a compatibility cycle unless they appear in an artifact expectation fixture or a specific doc promises them. |

## Pure/Core Rust Codegen Promotion Rule

Pure/core Rust codegen can move out of Preview only when all of these hold:

1. Mint stays green for interpreted tests, compiled tests, check-codegen,
   roundtrip, expectations, and canaries.
2. The core canary tier remains 0-skip for run, check-codegen, and roundtrip.
3. Artifact expectations cover representative emitted Rust shapes: top-level
   bindings, exported functions/types, collections, ownership-sensitive
   borrowing, and documented stdlib helpers.
4. Every exact emitted shape that Futuruna promises has a golden artifact
   fixture, or the docs explicitly say the shape is internal.
5. Any emitted-artifact diff is reviewed as source, behavioral, or
   artifact-facing compatibility work instead of being treated as incidental
   compiler churn.

## Contributor Rules

When changing codegen:

- If behavior changes, add or update run/roundtrip/canary coverage.
- If generated Rust shape changes only internally, do not promise the new shape
  in docs.
- If generated Rust shape is intentionally public, add or update an artifact
  expectation in `tests/expect/artifact/`.
- If a golden artifact changes, say whether this is a compatibility-preserving
  implementation change, a preview-contract adjustment, or a stable
  compatibility break.
- If a helper name or layout becomes user-facing, move it from "unstable
  internal" into a named contract above before relying on it.

## Current Artifact Fixtures

| Fixture | Contract |
|---------|----------|
| `tests/expect/artifact/emit_pure_core_contract.runa` | A small pure/core program has a reviewed full emitted-Rust snapshot, including top-level global lowering and the binary `main` shape. |
| `tests/expect/artifact/lib_export_contract.runa` | `runa lib` emits exported functions/types as public Rust items, keeps private helpers private, and does not emit a binary `main`. |
