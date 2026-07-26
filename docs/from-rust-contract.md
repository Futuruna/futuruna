# From-Rust Validation Contract

`runa from-rust` is preview translational tooling inside the validation
boundary documented here. The current contract is a validation contract, not a
frozen source-compatibility promise: checked-in supported fixtures must
translate, parse, run in the Futuruna interpreter, and produce exactly the same
stdout as the original Rust program.

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
with stable diagnostics. A fixture without an explicit directive is part of the
supported subset and must match Rust output exactly.

## Supported Fixture Subset

The current supported examples cover:

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
  totals, event rollups, and text/parser transformations
- the downstream canary's clean-directory config validation and deterministic
  event-rollup fixtures, plus enum/reference loop aggregation with conditional
  accumulator rebinding
- small real-world examples: JSON-like values, expression evaluation, and a mini
  type checker

This subset is intentionally evidence-based: adding a Rust shape to the
supported set means adding or unmarking a fixture and keeping the lane green.

## Current Compatibility Boundary

The current boundary is a single-file Rust-to-Futuruna validation lane. It is
intended for pure/core Rust programs that use ordinary functions, ADTs, local
value mutation, deterministic collections, and the explicitly documented
checked-in shapes above. A supported fixture must be deterministic, must not
depend on process state, files, networking, threads, or wall-clock time, and
must exact-match stdout against the original Rust program.

Current non-goals are arbitrary crate translation, macro expansion beyond the
small checked-in print/vector/assert shapes, async/threading, unsafe semantics,
proc macros, generic trait machinery outside the checked `Functor` fixture,
general lifetime/reference-preserving translation, general iterator
state-machine translation, and nondeterministic `HashMap` stdout ordering.
Those shapes must either stay out of the supported corpus or fail closed with a
stable unsupported diagnostic.

## Expected Unsupported Fixtures

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
the directive can be removed and the supported subset can grow.

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
- `unsafe-rust`: `unsafe` blocks outside the validation subset
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
external crate imports must remain fail-closed until those shapes are
deliberately promoted.

## Preview Evidence

As of 2026-07-18, the preview boundary is backed by:

- `runa from-rust --test examples/from-rust/`: 35 exact stdout matches
- `./scripts/from-rust-downstream-canary.sh`: 5 downstream supported exact
  matches from a fresh temporary directory
- the same downstream canary: 10 expected-unsupported fail-closed fixtures
  covering the stable unsupported diagnostic categories listed above

This evidence promotes the checked-in validation boundary to preview. It does
not promote arbitrary Rust crate translation, broad macro expansion, full
lifetime preservation, general iterator state machines, unsafe or async
semantics, or generated Cargo manifests.

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
   reference patterns, async/threading, unsafe blocks, and external crate or
   proc-macro-like entrypoints.
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

1. The supported Rust source subset is versioned as a stable compatibility
   contract, including the exact Rust syntax families, stdlib shapes,
   ownership simplifications, deterministic collection behavior, and
   unsupported boundaries that users can rely on for the release line.
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
   searches within the documented supported subset and minimizes any divergence
   into a permanent fixture before promotion.
6. `runa from-rust --verify` has stable success/failure output for the
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
