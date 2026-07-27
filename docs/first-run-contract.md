---
feature_stage: stable
feature_stage_surfaces:
  - first-run-project-initialization
  - core-cli-workflow
---

# First-Run Contract

This is the stable new-user path Futuruna keeps green.

The blocking proof is:

```bash
./scripts/first-run-canary.sh
```

That script is part of `./scripts/mint.sh`, so the first-run contract is a core
mint gate, not an optional smoke test.

## Stable Path

For a freshly built `runa` binary, this flow must work without source edits:

```bash
runa init hello
cd hello
runa check src/main.runa
runa fmt --check src/main.runa
runa run src/main.runa
runa build src/main.runa
```

The contract covers:

- `runa init hello` creates `hello/runa.toml` and `hello/src/main.runa`
- `runa.toml` records the package name and `entry = "src/main.runa"`
- the generated source checks successfully
- the generated source is already formatted
- `runa run src/main.runa` prints `Hello from hello!`
- `runa build src/main.runa` produces a runnable native binary for the entry
- `runa feature-stages --json` reports the versioned stage schema and stable
  core CLI metadata
- a local qualified import/library consumer checks, formats, runs, builds, and
  can access exported functions and values
- every `runa` block in `docs/tutorial/01-hello.md` checks, formats, and runs

## Diagnostic Contract

First-hour failures should stop at Futuruna-level diagnostics whenever the
compiler has enough information to explain the problem. These cases are locked
by `runa expect tests/expect`, and selected first-hour mistakes are also
checked directly by `./scripts/first-run-canary.sh`. The covered cases include:

- common syntax mistakes such as `=>` where `->` is required
- undefined names and wrong arity
- missing local imports
- private qualified import member access that should suggest `@ export` instead
  of leaking rustc privacy errors
- malformed `@ depend` declarations
- obvious type mistakes such as annotated literal mismatches, heterogeneous list
  literals, and mismatched literal `if` branches
- named-scope diagnostics for live streams inside functions

## Non-Goals

This contract does not freeze cache directories, temporary Rust filenames, or
private generated Rust helper layout. It does not claim every possible semantic
type error is caught before codegen; newly discovered first-hour Rust compiler
leaks should be reduced into expectation cases or canaries.
