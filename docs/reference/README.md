---
feature_stage: mixed
feature_stage_surfaces:
  - core-language-syntax
  - documented-stdlib
  - pure-core-rust-artifacts
  - reactive-stateful-surfaces
  - rust-interop
  - style-and-modeling-guidance
  - typed-program-references
  - typed-calculation-contracts
---

# Futuruna Language Reference

Complete reference documentation for the Futuruna programming language.

## Reading Order

| Document | What it covers | Current stage |
|----------|---------------|---------------|
| [basics.md](basics.md) | Literals, types, operators, control flow, closures | Stable |
| [runes.md](runes.md) | The seven runes (`#` `>` `|` `=` `~` `@` `?`) — all top-level statement forms | Stable |
| [stdlib.md](stdlib.md) | Complete standard library (~70 builtins): math, strings, lists, collections, I/O, JSON, HTTP, database | Stable |
| [streams.md](streams.md) | Reactive streams, subjects, subscriptions, named-scope lifetime ownership, actors, and effects | Stable |
| [rust-compatibility.md](rust-compatibility.md) | Stable pure/core Rust codegen behavior, type mapping, build modes, and stable `runa lib` Rust interop contract | Mixed |
| [style.md](style.md) | Style, legal modeling, and audit workflow guidance | Preview |
| [calculations.md](calculations.md) | Typed calculation contracts and JSON, TOML, and XLSX workflows | Preview |

Start with **basics.md** for syntax fundamentals, then **runes.md** for the full language. Use **stdlib.md** as a lookup reference. Consult **streams.md** and **rust-compatibility.md** when working with reactive/stateful code, pure/core Rust codegen behavior, or Rust interop. Use **calculations.md** to expose a typed rule as a human- or machine-filled calculation.

For the broader stage matrix across language/runtime and `runa` command
surfaces, see [../feature-stages.md](../feature-stages.md). The machine-readable
metadata lives in [../feature-stages.json](../feature-stages.json).
