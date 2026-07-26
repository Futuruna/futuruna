# From-Rust Validation Contract

`runa from-rust` is still experimental translational tooling. The current
contract is a validation contract, not a frozen source-compatibility promise:
checked-in supported fixtures must translate, parse, run in the Futuruna
interpreter, and produce exactly the same stdout as the original Rust program.

The blocking lane is:

```bash
runa from-rust --test examples/from-rust/
```

CI runs this lane after mint. A fixture without an explicit directive is part of
the supported subset and must match Rust output exactly.

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
- small real-world examples: JSON-like values, expression evaluation, and a mini
  type checker

This subset is intentionally evidence-based: adding a Rust shape to the
supported set means adding or unmarking a fixture and keeping the lane green.

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
- `reference-tuple-match`: matches over tuples of references that need
  reference-pattern simplification

Current expected-unsupported categories include:

- nested boxed enum patterns and reference-pattern simplification

## Promotion Rule

Do not promote `runa from-rust` beyond experimental while the corpus still has
expected-unsupported fixtures. Promotion requires shrinking or splitting those
fixtures into exact output matches, or into stable unsupported diagnostics for
Rust shapes Futuruna intentionally does not translate.
