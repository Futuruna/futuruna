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

@ print(show(Math.square(5)))  -- 25
@ print(show(Math.cube(3)))    -- 27
```

`@ export` marks what's public. `@ import Name from ./path` brings it in with qualified access.
For reusable helper files, mark import-safe libraries with
`-- library-hygiene: importable` and keep `runa lint-library --imports` green as
described in [../library-hygiene.md](../library-hygiene.md).

## Add dependencies

```bash
runa add ../shared-lib                              # Local directory
runa add https://github.com/org/tax-rules           # Git repository (default branch)
runa add https://github.com/org/tax-rules --rev v1.2  # Branch, tag or commit
```

`runa add` records the dependency in `runa.toml`:

```toml
[dependencies]
shared-lib = { path = "../shared-lib" }
tax-rules = { git = "https://github.com/org/tax-rules", rev = "v1.2" }
```

For a Git repository it downloads that revision (a shallow fetch of one
commit) into the per-user Futuruna cache and pins the exact commit in
`runa.lock`. Paths in both files are relative to `runa.toml`; commit both
files. Git sources are `https://`, `ssh://` or `git@host:path` URLs, or a local
repository path ending in `.git`.

Import a dependency's modules by its name:

```runa
@ import shared-lib/rates        -- shared-lib/rates.runa or shared-lib/src/rates.runa
@ import tax-rules               -- tax-rules/lib.runa (or src/lib.runa)
```

Only `runa add` and `runa fetch` contact the network. Every other command,
including the editor, resolves imports from the commit locked in `runa.lock`.
After cloning a project, or when `runa.lock` names a commit you have not
downloaded, run:

```bash
runa fetch            # download the commits pinned in runa.lock
runa fetch --update   # re-resolve each git `rev` (or default branch) and rewrite runa.lock
```

If a dependency is missing or its lock entry no longer matches `runa.toml`,
imports from it report an error that asks you to run `runa fetch`.

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
with `@ depend`, or uses a feature that adds them, validating it needs Cargo to
download and build those crates, which runs their build scripts. `check` does
that only when asked:

```bash
runa check --build-deps src/main.runa
```

Without the flag such a program fails the check with an error naming the flag.
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
