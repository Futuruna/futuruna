---
feature_stage: mixed
feature_stage_surfaces:
  - core-cli-workflow
  - first-run-project-initialization
  - importable-local-libraries
  - package-and-project-tooling
  - solver-assisted-verification
  - exploratory-audit-tooling
---

# 7. Building a Project

## Create a project

```bash
runa init my-app
cd my-app
```

This creates:
```
my-app/
  runa.toml
  src/main.runa
```

## runa.toml

```toml
[package]
name = "my-app"
version = "0.1.0"
entry = "src/main.runa"

[dependencies]
```

## Multi-file imports

```runa
-- src/math.runa
@ export
> square(x: Int) -> Int { x * x }

@ export
> cube(x: Int) -> Int { x * x * x }
```

```runa
-- src/main.runa
@ import Math from ./math

@ print(show(Math.square(5))) -- 25
@ print(show(Math.cube(3))) -- 27
```

`@ export` marks what's public. `@ import Name from ./path` brings it in with qualified access.
For reusable helper files, mark import-safe libraries with
`-- library-hygiene: importable` and keep `runa lint-library --imports` green as
described in [../library-hygiene.md](../library-hygiene.md).

## Tests

`runa test` runs every `.runa` file in `tests/`. `runa init` does not create
that directory; add it together with a first test:

```bash
mkdir tests
```

```runa
-- tests/math_test.runa
@ import Math from ../src/math

| square_of_five: Math.square(5) -> Math.square(5) == 25
? square_of_five
```

Run `runa test` from the project root. A named invariant checked with `?`
fails the test when it does not hold.

## Add dependencies

```bash
runa add ../shared-lib          # Local path
runa add https://github.com/...  # Git repository
```

This updates `runa.toml` and generates `runa.lock` for reproducible builds.

## Build and run

```bash
runa run src/main.runa     # Compile + execute
runa build src/main.runa   # Compile to native binary → ./main
runa check src/main.runa   # Full frontend + generated Rust validation
runa check --frontend src/main.runa # Fast frontend-only feedback
runa emit src/main.runa    # Show generated Rust
runa test                  # Run all tests/*.runa
```

Full `check` validates the generated Rust. When a program declares Rust crates
with `@ depend`, or uses a feature that adds them, it invokes Cargo and may
download missing dependencies. To use only cached dependencies, run:

```bash
CARGO_NET_OFFLINE=true runa check src/main.runa
```

Cargo reports an error if the required dependencies are unavailable offline.
`check --frontend` provides frontend feedback without invoking Rust validation;
it does not establish that the generated program compiles. Source filenames
such as `2026-policy.runa` are supported: generated Cargo package names are
normalized independently of the user's filename.

## Tooling

```bash
runa fmt .                 # Format all .runa files
runa fmt --check .         # Check formatting (CI mode)
runa lsp                   # Start language server for editor integration
runa audit src/main.runa   # Discover invariant gaps automatically
runa verify src/main.runa  # Prove invariants via Z3
```

## What's next?

- [Language Reference](../reference/basics.md) — full syntax and semantics
- [Standard Library](../reference/stdlib.md) — 100+ built-in functions
- [Examples](../../examples/) — real programs
- [Research](../research.md) — the science behind the syntax
