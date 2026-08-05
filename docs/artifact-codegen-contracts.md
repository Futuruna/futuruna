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
| Pure/core generated Rust behavior and reviewed artifact fixtures | Stable | For stable pure/core source, generated Rust must compile on the supported Rust toolchain and preserve documented Futuruna behavior. Exact text is stable only where covered by an artifact expectation fixture; helper names and private layout remain internal. |
| Artifact expectation fixtures | Snapshot-stable | Files under `tests/expect/artifact/` and their golden files are reviewed emitted-artifact contracts. Any diff is a compatibility-facing change that must be intentional and documented. |
| `runa build` native binary output | Stable command behavior, unstable path internals | The command produces a runnable native binary or reports a compiler bug if generated Rust does not compile. Cache paths, temporary Rust filenames, and internal build layout are not public contract. |
| `runa lib` Rust-facing library output | Stable | `@ export` functions and types are emitted as public Rust items using the documented type mapping; unexported helpers remain private; no binary `fn main` is emitted. The generated source is supported either as a Rust module file or as `src/lib.rs` in a user-owned Cargo library crate with matching Cargo dependencies supplied by the consumer. When dependencies are required, `runa lib` must list Cargo.toml-ready entries in generated-source comments and stderr. Exact helper layout remains internal. |
| `runa wasm` output | Preview | WASM export validation and build are supported, but package file layout, JS glue shape, and required toolchain policy are not stable yet. |
| Helper names, private modules, formatting, internal prelude layout | Unstable internal | These may change without a compatibility cycle unless they appear in an artifact expectation fixture or a specific doc promises them. |

## Pure/Core Rust Codegen Stable Contract

Pure/core Rust codegen is stable for the covered contract when all of these
hold:

1. Mint stays green for interpreted tests, compiled tests, check-codegen,
   roundtrip, expectations, and canaries.
2. The core canary tier remains 0-skip for run, check-codegen, and roundtrip.
3. Artifact expectations and permanent codegen fixtures cover representative
   emitted Rust shapes: top-level bindings, exported functions/types,
   collections, ownership-sensitive borrowing, and documented stdlib helpers.
   Ownership-sensitive direct branch/list reuse is also covered by a
   source-derived translation check that rejects tampered emitted Rust missing
   the required clone.
4. Every exact emitted shape that Futuruna promises has a golden artifact
   fixture, or the docs explicitly say the shape is internal.
5. Any emitted-artifact diff is reviewed as source, behavioral, or
   artifact-facing compatibility work instead of being treated as incidental
   compiler churn.

The current pure/core stable promise is intentionally narrow: pure/core programs
that use stable source syntax and documented stdlib behavior must compile and
preserve behavior. It does not freeze exact private helper names, internal
module layout, cache paths, temporary filenames, Rust interop escape-hatch
helper details, or WASM package shape. `runa lib` has its own Rust-facing
library contract above.

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
| `tests/expect/artifact/emit_pure_core_contract.runa` | A small pure/core program has a reviewed full emitted-Rust snapshot, including top-level global lowering and the synchronous binary entry point. The generated program body runs on a named worker with a bounded 64 MiB stack; the outer `main` joins it and resumes panic payloads. |
| `tests/expect/artifact/collection_helper_eval_contract.runa` | Collection helpers and expression-valued builtin arguments bind callback/input expressions once before repeated runtime use. |
| `tests/expect/artifact/ownership_branch_string_contract.runa` | Ownership-sensitive `String` branch lowering clones a reused local when the same binding is needed after an `if` arm; the same class is backed by source-derived translation-check tests for branch and list reuse. |
| `tests/expect/artifact/stateful_async_runtime_contract.runa` | Async/stateful lowering emits an async main, stream event/runtime scaffolding, scope-owned stream task registration, subscription receivers, and settled snapshot reads. |
| `tests/expect/artifact/lib_export_contract.runa` | `runa lib` emits exported functions/types as public Rust items, keeps private helpers private, and does not emit a binary `main`; package-layout and dependency-guidance behavior are covered by `./scripts/rust-interop-canary.sh`. |
