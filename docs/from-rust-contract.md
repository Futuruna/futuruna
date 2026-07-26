# From-Rust Validation Contract

`runa from-rust` is still experimental translational tooling. The current
contract is a validation contract, not a frozen source-compatibility promise:
checked-in supported fixtures must translate, parse, run in the Futuruna
interpreter, and produce exactly the same stdout as the original Rust program.

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
- the checked-in generic trait fixture, including the narrow `Functor`
  associated-type shape for `Option` and `Result`, generic higher-order
  functions, `impl Fn` composition, generic struct constructors, and generic
  inherent methods
- the checked-in nested pattern fixture, including recursive `Box<T>` enum
  constructors and ordered matches over two references lowered into nested
  Futuruna matches
- consumer-shaped single-file workflows for config parsing/validation, invoice
  totals, and event rollups
- the downstream canary's clean-directory config validation and deterministic
  event-rollup fixtures
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

The runner still compiles and runs the Rust program, attempts the translation,
and reports the observed parse failure or output divergence as `XFAIL`. If an
expected-unsupported fixture starts matching, the runner reports `XPASS` and
fails so the directive can be removed and the supported subset can grow.

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

The current checked-in corpus has no expected-unsupported fixtures. Future
adversarial examples may still use the directive when they intentionally
describe an unsupported Rust shape and fail closed with one of the stable
diagnostics above.

The downstream canary also keeps one expected-unsupported fixture outside the
example corpus to prove the boundary is enforced: returning a borrowed
reference from a general function must remain `borrowed-return-reference` until
that ownership shape is deliberately promoted.

## Preview Promotion Checklist

A zero-XFAIL example corpus is necessary but not sufficient for promoting
`runa from-rust` beyond experimental. Promotion to preview requires all of the
following evidence to be reviewed together:

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
   note or an explicit statement that the shape is still experimental tooling
   scope rather than a stable source-compatibility promise.
7. `docs/feature-stages.md`, `docs/feature-stages.json`, and this contract are
   updated in the same reviewed change if the stage moves from experimental to
   preview.

Preview would mean "intended for real use inside this documented validation
boundary." It would not mean arbitrary Rust crate translation, macro expansion
beyond the checked small forms, async runtime translation, unsafe semantics,
proc macro support, full generic trait machinery, full lifetime/reference
preservation, general iterator state-machine translation, generated Cargo
manifests, or stable formatting/layout of the emitted Futuruna source.

The downstream canary is production evidence for the current validation
boundary, not a promise that arbitrary Rust crates translate.
