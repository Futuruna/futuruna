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
- closures and iterator patterns already represented by matching fixtures
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
- `associated-types`: associated types in traits or impl blocks
- `impl-trait`: `impl Trait` signatures
- `unsupported-map-err`: `Result::map_err` shapes outside the checked-in
  integer parse-error remapping subset
- `stateful-iterator-chain`: iterator/map state machines such as `scan`,
  `sort_by`, `entry`, and `or_insert_with`
- `reference-tuple-match`: matches over tuples of references that need
  reference-pattern simplification

Current expected-unsupported categories include:

- recursive ownership patterns that return borrowed nodes
- associated types, trait bounds, higher-rank generic closures, and `impl Trait`
- iterator state machines such as `scan`, `sort_by`, and map entry chains
- nested boxed enum patterns and reference-pattern simplification

## Promotion Rule

Do not promote `runa from-rust` beyond experimental while the corpus still has
expected-unsupported fixtures. Promotion requires shrinking or splitting those
fixtures into exact output matches, or into stable unsupported diagnostics for
Rust shapes Futuruna intentionally does not translate.
