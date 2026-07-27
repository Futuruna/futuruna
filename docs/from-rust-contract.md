# From-Rust Validation Contract

`runa from-rust` is stable translational tooling inside the validation boundary
documented here.

The named current contract is **FRSS-v0**, the From-Rust Single-File Supported
Subset version 0. FRSS-v0 is a versioned production-ready compatibility
contract for deterministic single-file Rust programs in the documented
supported subset. It is intentionally narrower than arbitrary Rust
source-compatibility: crate translation, module trees, broad macro expansion,
unsafe/async/effectful APIs, general lifetime preservation, and general
iterator state-machine translation remain outside the stable promise unless a
future contract promotes them separately.

For Rust source inside FRSS-v0, checked-in supported fixtures must translate,
parse, run in the Futuruna interpreter, and produce exactly the same stdout as
the original Rust program. For source outside FRSS-v0 that matches a known
unsupported boundary, `runa from-rust` must fail closed with an unsupported
diagnostic before Futuruna parse/run instead of silently producing wrong
Futuruna.

The broad example-corpus lane is:

```bash
runa from-rust --test examples/from-rust/
```

The mint-blocking downstream canary is:

```bash
./scripts/from-rust-downstream-canary.sh
```

That script copies checked-in fixtures from `tests/from-rust/downstream/` into a
fresh temporary directory, exact-matches the supported consumer-shaped Rust
programs, and verifies that intentionally unsupported Rust stays fail-closed
with stable diagnostics. A fixture without an explicit directive is part of
FRSS-v0 and must match Rust output exactly.

The mint-blocking generated supported-subset differential lane is:

```bash
./scripts/from-rust-differential.sh
```

That script writes deterministic single-file Rust programs inside FRSS-v0 to a
temporary directory, exact-matches Rust stdout against the translated Futuruna
output, and leaves replay/minimization artifacts on failure. By default it
keeps the original six base cases and searches three stable seeds across the
six source-shape families in
`tests/from-rust/differential/search-manifest.tsv`.

## FRSS-v0 Contract Summary

| Field | FRSS-v0 contract |
|-------|------------------|
| Stage | Stable / production-ready for FRSS-v0 |
| Source shape | One deterministic Rust source file with ordinary functions, local values, ADTs, checked impl/generic shapes, checked stdlib collection/string forms, and a `main` whose observable contract is stdout. |
| Package shape | No Cargo workspace, Rust `mod` declarations or module tree, build script, proc macro, or external crate translation. `std` imports used by checked fixtures are allowed. |
| Runtime effects | Pure/core deterministic computation only: no file I/O, networking, process state, environment, threads, async, wall-clock time, randomness, or nondeterministic stdout ordering. |
| Success guarantee | Supported FRSS-v0 fixtures must compile/run as Rust, translate to Futuruna, parse/run as Futuruna, and exact-match Rust stdout. |
| Failure guarantee | Recognized unsupported boundaries must fail closed with a stable diagnostic category before Futuruna parse/run. |
| Non-guarantees | Arbitrary Rust crate translation, broad macro expansion, exact emitted Futuruna formatting/layout, generated Cargo manifests, full lifetime/reference preservation, full generic trait machinery, unsafe semantics, async/threading semantics, and general iterator state-machine translation. |

## FRSS-v0 Supported Categories

The current FRSS-v0 supported examples cover:

- functions, local bindings, loops, conditionals, and pattern matching
- structs, enums, recursive ADTs, and simple impl dispatch
- `String`, `Vec`, `Option`, `Result`, maps, and sets in the checked-in shapes
- simple `Result` `?` chains, including checked-in integer
  `parse().map_err(...)?` parse-error remapping and early `return Err(...)`
  guard flows
- closures and iterator patterns already represented by matching fixtures,
  including the checked-in stateful subset for tuple-key `sort_by`,
  Fibonacci-style `scan(...).collect()`, and
  `entry(...).or_insert_with(Vec::new).push(...)` map grouping
- recursive owned tree patterns in the checked-in ownership fixture, including
  `Vec<Box<T>>` children, inherent impl method calls that collide with builtin
  names, functional lowering for `&mut self` field pushes, and the narrow
  `Option<&T>` recursive search shape translated as value-returning `Option(T)`
- conditional accumulator rebinding in loop bodies for checked single-target
  assignment and compound-assignment shapes, lowered as Futuruna `if`
  expressions so values updated in a branch remain visible after the branch
- the checked-in generic trait fixture, including the narrow `Functor`
  associated-type shape for `Option` and `Result`, generic higher-order
  functions, `impl Fn` composition, generic struct constructors, and generic
  inherent methods
- the checked-in nested pattern fixture, including recursive `Box<T>` enum
  constructors and ordered matches over two references lowered into nested
  Futuruna matches
- consumer-shaped single-file workflows for config parsing/validation, invoice
  totals, event rollups, text/parser transformations, nested customer/order
  data, error-row pipelines, deterministic inventory reporting, and text
  normalization
- the downstream canary's clean-directory config validation and deterministic
  event-rollup fixtures, plus enum/reference loop aggregation with conditional
  accumulator rebinding
- small real-world examples: JSON-like values, expression evaluation, and a mini
  type checker

This subset is intentionally evidence-based: adding a Rust shape to FRSS-v0
means adding or unmarking a fixture and keeping the lane green.

The reviewed source-shape-to-fixture map lives in
[from-rust-evidence-manifest.md](from-rust-evidence-manifest.md). Public
supported-shape claims in this document should either appear there or be
removed from the contract.

## FRSS-v0 Category Matrix

| Category | Supported in FRSS-v0 | Required evidence | Outside FRSS-v0 |
|----------|----------------------|-------------------|-----------------|
| Source and package boundary | Single-file Rust programs using checked `std` shapes and deterministic `main` stdout, with supported items at the Rust file top level. | Exact match in the example corpus, downstream canary, or generated differential lane. | External crates, `extern crate`, Rust `mod` declarations or module trees, proc macros, build scripts, Cargo-manifest generation, or crate-level translation. |
| Control flow and bindings | Functions, local bindings, `if`/`else`, `match`, `for`, `while`, local value mutation, checked conditional accumulator rebinding, and compound-assignment shapes. | Exact stdout parity in supported fixtures; generated numeric/branch differential coverage. | Process state, nondeterminism, or control flow tied to unsupported runtime effects. |
| Data and ADTs | Structs, enums, recursive ADTs, `Box<T>` enum constructors, nested structs/vectors, simple inherent impl dispatch, checked recursive owned-tree search. | Example adversarial fixtures, downstream nested customer/order fixtures, generated nested-data cases. | General reference-preserving/lifetime translation or arbitrary borrowed-reference returns. |
| Strings and formatting | Checked `String`, `&str`, trim/lowercase/replace/classification, `format!`, stdout/vector macro shapes, and deterministic stdout formatting used by fixtures. | String examples, downstream text fixtures, generated text matrix. | Broad macro expansion, formatting/layout promises for emitted Futuruna, or unchecked formatting traits. |
| Collections | Checked `Vec`, maps/sets, deterministic `BTreeMap` reports, list indexing in fixtures, and checked grouping/rollup patterns. | Example stress fixtures, downstream event/inventory fixtures, generated `BTreeMap` rollup. | Nondeterministic `HashMap` stdout ordering or arbitrary collection/iterator state machines. |
| Error handling | `Option`, `Result`, simple `?` chains, early `return Err(...)`, and checked integer `parse().map_err(...)?` remapping. | Error-handling examples, downstream error-row pipeline, generated parse pipeline. | `Result::map_err` forms outside the checked parse-error remapping subset. |
| Generic and trait subset | Narrow checked `Functor` associated-type shape for `Option` and `Result`, generic higher-order functions, `impl Fn` composition, generic struct constructors, and generic inherent methods. | Checked generic trait fixture. | General associated types, arbitrary trait machinery, and `impl Trait` signatures outside the checked `impl Fn` composition shape. |
| Iterator/stateful subset | Checked tuple-key `sort_by`, Fibonacci-style `scan(...).collect()`, and `entry(...).or_insert_with(Vec::new).push(...)` map grouping. | Checked closure/iterator fixture. | General iterator state-machine translation. |

## FRSS-v0 Lane Mapping

| Lane | Command | FRSS-v0 role |
|------|---------|--------------|
| Broad example corpus | `runa from-rust --test examples/from-rust/` | Keeps the checked in-tree FRSS-v0 corpus exact-matching Rust stdout. |
| Mint downstream canary | `./scripts/from-rust-downstream-canary.sh` | Runs consumer-shaped FRSS-v0 programs from a fresh temporary directory and proves expected-unsupported boundaries stay fail-closed. |
| Mint generated differential lane | `./scripts/from-rust-differential.sh` | Searches seed-stable deterministic FRSS-v0 programs from `tests/from-rust/differential/search-manifest.tsv`, exact-matches Rust vs translated Futuruna stdout, and leaves replay/minimization artifacts on failure. |

## FRSS-v0 `from-rust --verify` Output Contract

`runa from-rust --verify <file.rs>` is the single-file interactive verifier for
FRSS-v0. It compiles and runs the Rust file, translates the file to Futuruna,
runs the translated Futuruna in the interpreter, and compares stdout.

The stable summary lines are written to stderr:

| Outcome | Exit | Stable summary |
|---------|------|----------------|
| Supported source matches | 0 | `from-rust verify: match <file> lines=<n>` |
| Recognized unsupported source | 1 | `from-rust verify: unsupported <category>: <message>` |
| Input read failure | 1 | `from-rust verify: read-failed <file>: <message>` |
| Rust parse failure | 1 | `from-rust verify: rust-parse error: <message>` |
| Rust compile failure | 1 | `from-rust verify: rust-compile-failed <file>: <message>` |
| Rust run failure | 1 | `from-rust verify: rust-run-failed <file>: <message>` |
| Missing `rustc` | 1 | `from-rust verify: rustc-unavailable: <message>` |
| Translated Futuruna parse failure | 1 | `from-rust verify: translated-parse-failed <file>: <message>` |
| Output divergence | 1 | `from-rust verify: mismatch <file> rust_lines=<n> futuruna_lines=<n>` |

The colored transpiled source, pretty output block, and side-by-side mismatch
diff are diagnostic display only. Tests and external tooling should key off the
stable summary lines above.

## FRSS-v0 Compatibility Policy

FRSS-v0 follows the project compatibility policy for a stable surface:

- Expanding FRSS-v0 requires a reviewed fixture or generated lane case before
  the docs claim the new source shape is supported. Stable support expansions
  should be recorded in the compatibility guide when they change the public
  contract.
- Narrowing or removing a documented FRSS-v0 shape requires either a bug-fix
  rationale or a new contract version such as FRSS-v1. User-visible preview
  history stays in the compatibility guide, but stable-contract breaks must be
  handled under the stable-surface policy.
- Unsupported diagnostic category changes require updating the permanent
  expected-unsupported fixture that proves the boundary and recording the
  compatibility impact when external tooling may key off the category.
- Generated Futuruna source formatting, helper names, and internal layout remain
  internal unless a future artifact expectation explicitly promotes them.
- Keeping FRSS-v0 production-ready requires the evidence lanes below to remain
  green together. Renaming FRSS-v0 or passing one lane is not enough to expand
  the stable claim.

## FRSS-v0 Compatibility Boundary

FRSS-v0 is a single-file Rust-to-Futuruna validation boundary. It is
intended for pure/core Rust programs that use ordinary functions, ADTs, local
value mutation, deterministic collections, and the explicitly documented
checked-in shapes above. A supported fixture must be deterministic, must not
depend on process state, files, networking, threads, or wall-clock time, and
must exact-match stdout against the original Rust program.

FRSS-v0 non-goals are arbitrary crate translation, macro expansion beyond the
small checked-in stdout/vector formatting shapes, async/threading, unsafe semantics,
proc macros, generic trait machinery outside the checked `Functor` fixture,
general lifetime/reference-preserving translation, general iterator
state-machine translation, Rust `mod` declarations or module trees, and
nondeterministic `HashMap` stdout ordering. Those shapes must either stay out
of the supported corpus or fail closed with a stable unsupported diagnostic.

## FRSS-v0 Unsupported Boundaries

The explicit unsupported boundaries for FRSS-v0 are represented by permanent
expected-unsupported fixtures.

Adversarial Rust examples may be kept in the same corpus with an explicit
top-of-file directive:

```rust
// runa-from-rust: expect-unsupported reason for the unsupported Rust shape
```

For expected-unsupported fixtures, the runner first checks for known
fail-closed diagnostics. If one is found, the fixture is reported as `XFAIL`
without requiring the Rust program to compile in the standalone `rustc` lane.
If the source translates, the runner compiles and runs the Rust program,
interprets the translated Futuruna, and reports an output match as `XPASS` so
the directive can be removed and FRSS-v0 can grow.

Expected-unsupported fixtures should fail closed before Futuruna parse/run
whenever the unsupported Rust shape is known. The runner reports these as stable
unsupported diagnostics:

- `borrowed-return-reference`: Rust functions that return borrowed references
  outside the checked recursive owned-tree search subset
- `associated-types`: associated types outside the checked `Functor` fixture
  shape
- `impl-trait`: `impl Trait` signatures outside the checked `impl Fn`
  composition shape
- `unsupported-map-err`: `Result::map_err` shapes outside the checked-in
  integer parse-error remapping subset
- `stateful-iterator-chain`: iterator/map state machines outside the checked-in
  stateful subset
- `reference-tuple-match`: matches over tuples of references outside the
  checked two-reference pattern simplification subset
- `async-threading`: `async` functions/blocks/awaits and thread-spawning code
  outside the deterministic pure/core subset
- `unsupported-effect`: effectful `std` APIs outside the deterministic
  pure/core subset, including file I/O, environment/process-state APIs,
  process control, runtime I/O, networking, wall-clock `SystemTime`/`Instant`,
  and randomized hashing through `RandomState`
- `unsafe-rust`: `unsafe` blocks outside the validation subset
- `unsupported-macro`: macro invocations outside the checked
  `println!`/`eprintln!`/`format!`/`vec!` subset. Unproven control/assertion
  macros such as `panic!`, `todo!`, `unimplemented!`, `assert!`, and
  `assert_eq!` stay unsupported until promoted with exact-match evidence.
- `unsupported-format-spec`: placeholders inside checked `println!`,
  `eprintln!`, and `format!` macro names outside the supported `{}`, `{:?}`,
  and `{:.N}` formatting subset, including named, width, padding, alignment,
  and radix/hex formatting
- `unsupported-module`: Rust `mod` declarations, including external
  `mod helper;` files and inline `mod helper { ... }` blocks, outside the flat
  single-file validation subset
- `unsupported-rust-item`: top-level Rust items outside the checked
  `fn`/`struct`/`enum`/`const`/`static`/`type`/`impl`/`trait`/`use` item
  subset
- `unsupported-rust-expr`: Rust expression forms with no checked lowering,
  including expression statements that would otherwise reach a translator
  fallback
- `external-crate`: non-stdlib `use` or `extern crate` inputs outside the
  single-file validation subset

The broad example corpus currently has no expected-unsupported fixtures. Future
adversarial examples may still use the directive when they intentionally
describe an unsupported Rust shape and fail closed with one of the stable
diagnostics above.

The downstream canary also keeps expected-unsupported fixtures outside the
example corpus to prove the boundary is enforced: returning a borrowed
reference from a general function, unchecked associated types, unchecked `impl
Trait`, unsupported `Result::map_err`, unsupported iterator state machines,
unsupported tuple-of-references matches, async/threading, unsafe blocks, and
external crate imports, unsupported macro names such as `print!`, unchecked
control/assertion macros such as `assert!`, and
unsupported format specs such as `{:x}`, unsupported top-level Rust items such
as `union`, unsupported expression fallbacks such as `break` statements, and
Rust module declarations such as `mod helper;`, and effectful `std` APIs such
as `std::fs`, `std::env`, `std::process`, `std::time::SystemTime`, `std::net`,
and `std::collections::hash_map::RandomState` must remain fail-closed until
those shapes are deliberately promoted.

## Production Evidence

As of 2026-07-18, the stable FRSS-v0 boundary is backed by:

- `runa from-rust --test examples/from-rust/`: 35 exact stdout matches
- `./scripts/from-rust-downstream-canary.sh`: 9 downstream supported exact
  matches from a fresh temporary directory
- the same downstream canary: 17 expected-unsupported fail-closed fixtures
  covering the stable unsupported diagnostic categories listed above
- `./scripts/from-rust-differential.sh`: 24 generated supported-subset exact
  matches by default, covering three stable search seeds plus base cases for
  loops/branches, `Option`/`Result` parse validation, nested data,
  deterministic map reporting, string transformation, and enum/reference
  conditional rebinding
- [from-rust-evidence-manifest.md](from-rust-evidence-manifest.md): reviewed
  map from every current supported source-shape claim to exact-match fixture or
  generated-lane evidence, plus the fail-closed unsupported boundary map
- `runa from-rust --verify <file.rs>`: stable success/failure summary lines for
  supported matches, recognized unsupported categories, translator parse
  failures, Rust compile/run failures, translated Futuruna parse failures, and
  stdout divergence. Focused CLI coverage currently exercises supported
  matches, Rust parse failure, Rust compile failure, `unsafe-rust`,
  `async-threading`, `unsupported-effect`, `unsupported-macro`,
  `unsupported-format-spec`, `unsupported-module`, `unsupported-rust-expr`,
  harness-level `translated-parse-failed`, harness-level `mismatch`, and help
  text. The harness cases keep translator-bug summary formatting stable without
  preserving a known-bad Rust source as a permanent supported or unsupported
  fixture.

This evidence promotes the checked-in validation boundary to stable and
production-ready. It does not promote arbitrary Rust crate translation, broad
macro expansion, full lifetime preservation, general iterator state machines,
unsafe or async semantics, effectful APIs, or generated Cargo manifests.

The downstream supported lane includes production-corpus growth across nested
data, error handling, deterministic collection/reporting, and text
transformation fixtures. These fixtures run from the same clean
temporary-directory canary as the rest of the FRSS-v0 corpus.

The generated supported-subset differential lane searches seed-stable
deterministic Rust programs inside the FRSS-v0 source-shape family manifest and
writes repro-ready source, output, manifest, coverage, replay, and
minimization artifacts on failure. It is intentionally not evidence for
arbitrary Rust crate translation.

## Stable Contract Maintenance Checklist

FRSS-v0 stays production-ready only if the following evidence remains green and
reviewed together:

1. The broad example corpus passes with exact stdout parity:
   `runa from-rust --test examples/from-rust/`.
2. The mint-blocking downstream canary passes from a fresh temporary directory:
   `./scripts/from-rust-downstream-canary.sh`.
3. The downstream canary covers consumer families across config validation,
   money or invoice arithmetic, deterministic event/report aggregation,
   text/parser-style transformation, nested data, error handling, and
   deterministic collection/reporting.
4. The downstream canary includes expected-unsupported fixtures for every
   stable non-goal that has a reasonably syntactic Rust marker, including
   general borrowed-reference returns, unchecked associated-type or `impl
   Trait` shapes, unsupported iterator state machines, unsupported tuple
   reference patterns, effectful `std` APIs, async/threading, unsafe blocks,
   Rust module declarations, unchecked macros, unsupported item/expression
   fallbacks, external crates, and proc-macro-like entrypoints.
5. Supported fixtures stay deterministic, single-file, and pure/core: no file
   I/O, networking, process state, wall-clock time, ambient environment, or
   nondeterministic stdout ordering.
6. Any promoted Rust shape lands with exact-match evidence and, when it expands
   the public contract, a compatibility-guide note under the stable-surface
   policy.
7. `docs/feature-stages.md`, `docs/feature-stages.json`, this contract, the
   README, the compatibility guide, and the production-readiness scorecard stay
   synchronized when the stable boundary changes.

Stable means "supported for real use inside this documented validation
boundary." It does not mean arbitrary Rust crate translation, macro expansion
beyond the checked small forms, async runtime translation, unsafe semantics,
proc macro support, full generic trait machinery, full lifetime/reference
preservation, general iterator state-machine translation, generated Cargo
manifests, or stable formatting/layout of the emitted Futuruna source.

The downstream canary is production evidence for the current validation
boundary, not a promise that arbitrary Rust crates translate.

## Production Readiness Audit

Audit date: 2026-07-18.

Result: FRSS-v0 is stable and production-ready for the documented single-file
validation boundary. The claim is intentionally narrow: arbitrary Rust crate
translation and the non-goals listed above remain outside the production
contract.

| Checklist item | Current evidence | Audit result |
|----------------|------------------|--------------|
| 1. Freeze a stable release-line contract | FRSS-v0 is named and versioned, and the feature-stage docs, JSON metadata, README, compatibility guide, contract, and scorecard now describe it as stable for the documented boundary. | Satisfied. Keep future boundary changes synchronized across the same files. |
| 2. Fixture evidence for every supported source shape | [from-rust-evidence-manifest.md](from-rust-evidence-manifest.md) maps every current supported source-shape claim to exact-match evidence from the broad example corpus, downstream canary, or generated differential lane. | Satisfied. Keep this manifest in the same reviewed change as any supported-shape expansion. |
| 3. Fail closed for every syntactically detectable unsupported boundary | The downstream unsupported corpus covers 17 permanent fail-closed fixtures, including ownership, generics, iterator state machines, tuple-reference matches, effectful `std` APIs, async/threading, unsafe, external crates, Rust module declarations, unchecked macros, format specs, item fallbacks, and expression fallbacks. | Satisfied. Keep adding expected-unsupported fixtures before documenting new non-goals. |
| 4. Larger downstream production corpus | The mint-blocking downstream lane runs 9 supported consumer-style fixtures from a fresh temporary directory across config validation, invoice arithmetic, event/report aggregation, text/parser transformations, nested data, error handling, inventory reporting, and normalization. | Satisfied. Keep growing with every promoted shape. |
| 5. Production search or proof-backed differential checking | `./scripts/from-rust-differential.sh` searches the checked-in six-family FRSS-v0 differential source-shape manifest with the original base cases plus three stable seeds by default, for 24 exact Rust-vs-Futuruna matches, and writes manifest, coverage, replay, and minimization artifacts. | Satisfied. Keep expanding the manifest as FRSS grows. |
| 6. Stable `from-rust --verify` user workflow | Stable summary lines exist for supported matches, recognized unsupported categories, Rust parse/compile/run failures, translated Futuruna parse failures, and output divergence. CLI coverage exercises supported success, Rust parse/compile failure, major unsupported categories, help text, and harness-level translated-parse-failed/mismatch translator-bug summaries. | Satisfied. Keep source-level fixtures for real future translator bugs when they appear. |
| 7. Compatibility guide records the production contract | The 0.1.x compatibility guide records the 2026-07-18 stable FRSS-v0 promotion and says how future source-subset breaks, diagnostic category changes, and fixture removals are handled under the stable-surface policy. | Satisfied. |
| 8. Feature-stage metadata, README, contract, and scorecard move together | This promotion packet moves `docs/feature-stages.md`, `docs/feature-stages.json`, this contract, the README, the compatibility guide, and the production-readiness scorecard together. | Satisfied. |

No current production blockers remain for FRSS-v0. Future support growth should
prefer a new fixture or generated lane case first, then update the manifest and
compatibility guide before broadening the stable claim.
