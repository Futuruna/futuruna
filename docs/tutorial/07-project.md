---
feature_stage: preview
feature_stage_surfaces:
  - core-cli-workflow
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

## Add dependencies

```bash
runa add ../shared-lib          # Local path
runa add https://github.com/...  # Git repository
```

This updates `runa.toml` and generates `runa.lock` for reproducible builds.

## Build and run

```bash
runa run src/main.runa     # Compile + execute
runa build src/main.runa   # Compile to native binary → ./my-app
runa check src/main.runa   # Type-check without running
runa emit src/main.runa    # Show generated Rust
runa test                  # Run all tests/*.runa
```

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
