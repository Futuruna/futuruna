# From-Rust Validation Contract

`runa from-rust` is preview translational tooling inside the validation
boundary documented here.

The named current contract is **FRSS-v0**, the From-Rust Single-File Supported
Subset version 0. FRSS-v0 is a versioned preview compatibility contract: it is
specific enough for users, tests, and release notes to point at a known Rust
source subset, but it is not yet a production-ready or stable arbitrary-Rust
source-compatibility promise.

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
output, and leaves replay artifacts on failure.

## FRSS-v0 Contract Summary

| Field | FRSS-v0 contract |
|-------|------------------|
| Stage | Preview |
| Source shape | One deterministic Rust source file with ordinary functions, local values, ADTs, checked impl/generic shapes, checked stdlib collection/string forms, and a `main` whose observable contract is stdout. |
| Package shape | No Cargo workspace, module tree, build script, proc macro, or external crate translation. `std` imports used by checked fixtures are allowed. |
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

## FRSS-v0 Category Matrix

| Category | Supported in FRSS-v0 | Required evidence | Outside FRSS-v0 |
|----------|----------------------|-------------------|-----------------|
| Source and package boundary | Single-file Rust programs using checked `std` shapes and deterministic `main` stdout. | Exact match in the example corpus, downstream canary, or generated differential lane. | External crates, `extern crate`, multi-file modules, proc macros, build scripts, Cargo-manifest generation, or crate-level translation. |
| Control flow and bindings | Functions, local bindings, `if`/`else`, `match`, `for`, `while`, local value mutation, checked conditional accumulator rebinding, and compound-assignment shapes. | Exact stdout parity in supported fixtures; generated numeric/branch differential coverage. | Process state, nondeterminism, or control flow tied to unsupported runtime effects. |
| Data and ADTs | Structs, enums, recursive ADTs, `Box<T>` enum constructors, nested structs/vectors, simple inherent impl dispatch, checked recursive owned-tree search. | Example adversarial fixtures, downstream nested customer/order fixtures, generated nested-data cases. | General reference-preserving/lifetime translation or arbitrary borrowed-reference returns. |
| Strings and formatting | Checked `String`, `&str`, trim/lowercase/replace/classification, `format!`, print/vector/assert macro shapes, and deterministic stdout formatting used by fixtures. | String examples, downstream text fixtures, generated text matrix. | Broad macro expansion, formatting/layout promises for emitted Futuruna, or unchecked formatting traits. |
| Collections | Checked `Vec`, maps/sets, deterministic `BTreeMap` reports, list indexing in fixtures, and checked grouping/rollup patterns. | Example stress fixtures, downstream event/inventory fixtures, generated `BTreeMap` rollup. | Nondeterministic `HashMap` stdout ordering or arbitrary collection/iterator state machines. |
| Error handling | `Option`, `Result`, simple `?` chains, early `return Err(...)`, and checked integer `parse().map_err(...)?` remapping. | Error-handling examples, downstream error-row pipeline, generated parse pipeline. | `Result::map_err` forms outside the checked parse-error remapping subset. |
| Generic and trait subset | Narrow checked `Functor` associated-type shape for `Option` and `Result`, generic higher-order functions, `impl Fn` composition, generic struct constructors, and generic inherent methods. | Checked generic trait fixture. | General associated types, arbitrary trait machinery, and `impl Trait` signatures outside the checked `impl Fn` composition shape. |
| Iterator/stateful subset | Checked tuple-key `sort_by`, Fibonacci-style `scan(...).collect()`, and `entry(...).or_insert_with(Vec::new).push(...)` map grouping. | Checked closure/iterator fixture. | General iterator state-machine translation. |

## FRSS-v0 Lane Mapping

| Lane | Command | FRSS-v0 role |
|------|---------|--------------|
| Broad example corpus | `runa from-rust --test examples/from-rust/` | Keeps the checked in-tree FRSS-v0 corpus exact-matching Rust stdout. |
| Mint downstream canary | `./scripts/from-rust-downstream-canary.sh` | Runs consumer-shaped FRSS-v0 programs from a fresh temporary directory and proves expected-unsupported boundaries stay fail-closed. |
| Mint generated differential lane | `./scripts/from-rust-differential.sh` | Generates deterministic FRSS-v0 programs, exact-matches Rust vs translated Futuruna stdout, and leaves replay artifacts on failure. |

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

FRSS-v0 follows the project compatibility policy for a preview surface:

- Expanding FRSS-v0 requires a reviewed fixture or generated lane case before
  the docs claim the new source shape is supported.
- Narrowing or removing a documented FRSS-v0 shape requires either a bug-fix
  rationale or a new contract version such as FRSS-v1. User-visible preview
  changes should be recorded in the compatibility guide's preview notes.
- Unsupported diagnostic category changes require updating the permanent
  expected-unsupported fixture that proves the boundary.
- Generated Futuruna source formatting, helper names, and internal layout remain
  internal unless a future artifact expectation explicitly promotes them.
- Production promotion requires satisfying the Production Promotion Checklist
  below in one reviewed change. Renaming FRSS-v0 or passing one lane is not
  enough to call `runa from-rust` production-ready.

## FRSS-v0 Compatibility Boundary

FRSS-v0 is a single-file Rust-to-Futuruna validation boundary. It is
intended for pure/core Rust programs that use ordinary functions, ADTs, local
value mutation, deterministic collections, and the explicitly documented
checked-in shapes above. A supported fixture must be deterministic, must not
depend on process state, files, networking, threads, or wall-clock time, and
must exact-match stdout against the original Rust program.

FRSS-v0 non-goals are arbitrary crate translation, macro expansion beyond the
small checked-in print/vector/assert shapes, async/threading, unsafe semantics,
proc macros, generic trait machinery outside the checked `Functor` fixture,
general lifetime/reference-preserving translation, general iterator
state-machine translation, and nondeterministic `HashMap` stdout ordering.
Those shapes must either stay out of the supported corpus or fail closed with a
stable unsupported diagnostic.

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
  `println!`/`eprintln!`/`format!`/`vec!`/`panic!`/`todo!`/
  `unimplemented!`/`assert!`/`assert_eq!` subset
- `unsupported-format-spec`: placeholders inside checked `println!`,
  `eprintln!`, and `format!` macro names outside the supported `{}`, `{:?}`,
  and `{:.N}` formatting subset, including named, width, padding, alignment,
  and radix/hex formatting
- `unsupported-rust-item`: top-level Rust items outside the checked
  `fn`/`struct`/`enum`/`const`/`static`/`type`/`impl`/`trait`/`use`/`mod`
  item subset
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
external crate imports, unsupported macro names such as `print!`, and
unsupported format specs such as `{:x}`, unsupported top-level Rust items such
as `union`, unsupported expression fallbacks such as `break` statements, and
effectful `std` APIs such as `std::fs`, `std::env`, `std::process`,
`std::time::SystemTime`, `std::net`, and `std::collections::hash_map::RandomState`
must remain fail-closed until those shapes are deliberately promoted.

## Preview Evidence

As of 2026-07-18, the preview boundary is backed by:

- `runa from-rust --test examples/from-rust/`: 35 exact stdout matches
- `./scripts/from-rust-downstream-canary.sh`: 9 downstream supported exact
  matches from a fresh temporary directory
- the same downstream canary: 15 expected-unsupported fail-closed fixtures
  covering the stable unsupported diagnostic categories listed above
- `./scripts/from-rust-differential.sh`: 6 generated supported-subset exact
  matches covering loops/branches, `Option`/`Result` parse validation, nested
  data, deterministic map reporting, string transformation, and enum/reference
  conditional rebinding
- `runa from-rust --verify <file.rs>`: stable success/failure summary lines for
  supported matches, recognized unsupported categories, translator parse
  failures, Rust compile/run failures, translated Futuruna parse failures, and
  stdout divergence. Focused CLI coverage currently exercises supported
  matches, Rust parse failure, Rust compile failure, `unsafe-rust`,
  `async-threading`, `unsupported-effect`, `unsupported-macro`,
  `unsupported-format-spec`, `unsupported-rust-expr`, and help text. The
  output-mismatch summary remains part of the stable contract, but
  there is no current minimized source-level fixture for it after syntactic
  macro-name and format-spec divergences were moved to fail-closed diagnostics.
  Translated Futuruna parse failure has a stable summary contract, but no
  minimized source-level fixture currently reaches that path.

This evidence promotes the checked-in validation boundary to preview. It does
not promote arbitrary Rust crate translation, broad macro expansion, full
lifetime preservation, general iterator state machines, unsafe or async
semantics, or generated Cargo manifests.

The downstream supported lane now includes the first production-corpus growth
increment toward the checklist below: nested data, error handling, deterministic
collection/reporting, and text transformation fixtures run from the same clean
temporary-directory canary as the preview corpus. This is stronger production
evidence, not a production-ready stage claim.

The generated supported-subset differential lane is the first implementation of
the production checklist's differential requirement. It searches an enumerated
set of deterministic Rust programs inside FRSS-v0 and writes repro-ready
source, output, manifest, and replay artifacts on failure.
It is intentionally not evidence for arbitrary Rust crate translation.

## Preview Promotion Checklist

A zero-XFAIL example corpus is necessary but not sufficient for keeping `runa
from-rust` in preview. The preview claim depends on all of the following
evidence staying green and reviewed together:

1. The broad example corpus passes with exact stdout parity:
   `runa from-rust --test examples/from-rust/`.
2. The mint-blocking downstream canary passes from a fresh temporary directory:
   `./scripts/from-rust-downstream-canary.sh`.
3. The downstream canary covers at least four distinct consumer families:
   config validation, money or invoice arithmetic, deterministic event/report
   aggregation, and text/parser-style transformation.
4. The downstream canary includes expected-unsupported fixtures for every
   preview non-goal that has a reasonably syntactic Rust marker, including
   general borrowed-reference returns, unchecked associated-type or `impl
   Trait` shapes, unsupported iterator state machines, unsupported tuple
   reference patterns, effectful `std` APIs, async/threading, unsafe blocks,
   and external crate or proc-macro-like entrypoints.
5. Supported fixtures stay deterministic, single-file, and pure/core: no file
   I/O, networking, process state, wall-clock time, ambient environment, or
   nondeterministic stdout ordering.
6. Any promoted Rust shape lands with a fixture and either a compatibility-guide
   note or an explicit statement that the shape is still preview tooling scope
   rather than a stable source-compatibility promise.
7. `docs/feature-stages.md`, `docs/feature-stages.json`, and this contract stay
   synchronized with the preview boundary.

Preview would mean "intended for real use inside this documented validation
boundary." It would not mean arbitrary Rust crate translation, macro expansion
beyond the checked small forms, async runtime translation, unsafe semantics,
proc macro support, full generic trait machinery, full lifetime/reference
preservation, general iterator state-machine translation, generated Cargo
manifests, or stable formatting/layout of the emitted Futuruna source.

The downstream canary is production evidence for the current validation
boundary, not a promise that arbitrary Rust crates translate.

## Production Promotion Checklist

Preview is not enough for a production-ready `runa from-rust` claim. Promotion
from preview to production-ready requires a reviewed change that proves all of
the following at the same time:

1. FRSS-v0, or its successor, is frozen as a stable compatibility contract for
   the release line, including the exact Rust syntax families, stdlib shapes,
   ownership simplifications, deterministic collection behavior, and
   unsupported boundaries that users can rely on.
2. Every supported source shape has at least one exact Rust-vs-Futuruna stdout
   fixture in either `examples/from-rust/` or the downstream canary, and every
   newly promoted shape adds coverage before the contract expands.
3. Every unsupported boundary that can be detected syntactically fails closed
   before Futuruna parse/run, with a stable diagnostic category and a permanent
   expected-unsupported fixture. Unsupported shapes must not silently translate
   into wrong Futuruna.
4. The mint-blocking downstream lane includes a larger production corpus with
   real consumer-style programs across parsing/validation, reporting,
   transformations, nested data, error handling, and deterministic collection
   workflows. At least one lane must run from a clean external-style fixture
   directory rather than relying only on in-tree examples.
5. A generated Rust-subset differential lane or equivalent proof-backed checker
   searches within the named supported subset and minimizes any divergence into
   a permanent fixture before promotion.
6. `runa from-rust --verify` keeps stable success/failure output for the
   production subset, and CLI/help/docs explain how users distinguish supported
   source, expected unsupported source, and translator bugs.
7. The compatibility guide records the production contract, including how future
   source-subset breaks, diagnostic category changes, and fixture removals are
   handled under `docs/compatibility-policy.md`.
8. `docs/feature-stages.md`, `docs/feature-stages.json`, this contract, the
   README, and the production-readiness scorecard move together in the same
   reviewed change.

Production-ready still would not mean arbitrary Rust crate translation unless
that broader crate-level contract is explicitly documented, canaried, and
promoted separately.

## Production Readiness Audit

Audit date: 2026-07-18.

Result: FRSS-v0 stays preview. The current evidence is strong enough for the
documented preview boundary, but it does not yet prove the production checklist
above. The missing items are now explicit `td` blockers instead of implicit
readiness debt.

| Checklist item | Current evidence | Audit result |
|----------------|------------------|--------------|
| 1. Freeze a stable release-line contract | FRSS-v0 is named and versioned, but `docs/feature-stages.md`, `docs/feature-stages.json`, the README state, and the compatibility guide still intentionally describe it as preview. | Blocked until the final promotion packet moves all public stage metadata together. |
| 2. Fixture evidence for every supported source shape | The broad example corpus, downstream canary, and generated differential lane provide exact-match fixtures, but there is no source-shape-to-fixture manifest proving every documented syntax, stdlib, ownership, collection, generic, and formatting claim. | Blocked by `td-f6df85`. |
| 3. Fail closed for every syntactically detectable unsupported boundary | The downstream unsupported corpus covers 15 permanent fail-closed fixtures, including ownership, generics, iterator state machines, tuple-reference matches, effectful `std` APIs, async/threading, unsafe, external crates, macros, format specs, item fallbacks, and expression fallbacks. The audit found a remaining syntactic non-goal without production-grade coverage: external Rust module declarations. | Blocked by `td-0f2bd8`. |
| 4. Larger downstream production corpus | The mint-blocking downstream lane runs 9 supported consumer-style fixtures from a fresh temporary directory across config validation, invoice arithmetic, event/report aggregation, text/parser transformations, nested data, error handling, inventory reporting, and normalization. | Satisfied for the current checklist. Keep growing with every promoted shape. |
| 5. Production search or proof-backed differential checking | `./scripts/from-rust-differential.sh` gives replay artifacts for 6 deterministic generated cases, but it is still an enumerated corpus rather than a source-shape-manifest-driven or seed-stable search/minimization lane. | Blocked by `td-d47bff`. |
| 6. Stable `from-rust --verify` user workflow | Stable summary lines exist for supported matches, recognized unsupported categories, Rust parse/compile/run failures, translated Futuruna parse failures, and output divergence. CLI coverage exercises many categories, but translated-parse-failed and mismatch currently lack minimized source-level fixtures after previous divergences moved to fail-closed diagnostics. | Blocked by `td-ed2a52`. |
| 7. Compatibility guide records the production contract | The compatibility guide records preview hardening and support expansions, but not a production contract or future stable break policy for FRSS. | Blocked until the final promotion packet after the evidence blockers above. |
| 8. Feature-stage metadata, README, contract, and scorecard move together | All current public stage metadata correctly keeps `runa from-rust` in preview. | Blocked until the final promotion packet after the evidence blockers above. |

Current production blockers:

- `td-f6df85`: add an FRSS-v0 source-shape evidence manifest.
- `td-0f2bd8`: fail closed on external Rust module declarations and other
  multi-file package boundaries.
- `td-d47bff`: promote the from-rust differential lane from six enumerated
  cases to production search/minimization evidence.
- `td-ed2a52`: cover or revise the `from-rust --verify` translator-bug paths
  for translated parse failures and output divergence.
