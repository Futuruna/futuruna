---
feature_stage: mixed
feature_stage_surfaces:
  - pure-core-rust-artifacts
  - rust-interop
---

# Rust Compatibility

Futuruna compiles to Rust. You write value semantics; the compiler handles ownership.

## Ownership Inference

Futuruna is to Rust as Kotlin is to Java. You never write `&T`, lifetimes, or `.clone()`.

### What the compiler infers

| Pattern | Futuruna | Generated Rust |
|---------|----------|---------------|
| Read-only parameter | `data: List(String)` | `data: &Vec<String>` |
| Borrowed string | `prefix: String` | `prefix: &String` |
| Single use | `= x = make_thing()` | `let x = make_thing();` (move) |
| Multiple use | `= x = val; f(x); g(x)` | `let x = val.clone(); f(&x); g(&x)` |
| In-place mutation | `xs: inout List(Int)` | `xs: &mut Vec<i64>` |

### Escape analysis

The compiler performs whole-program escape analysis:
- **Single-use** variables move (zero copies)
- **Multi-use** variables clone once, then borrow
- **Read-only** parameters auto-borrow as `&T`

### The `inout` keyword

For in-place mutation without ownership transfer:

```runa
> sort_vec(xs: inout List(Int)) -> () {
    @ rust { xs.sort(); }
}

= data = [5, 3, 1, 4, 2]
sort_vec(data)
@ print(show(data))    -- [1, 2, 3, 4, 5]
```

Compiles to `fn sort_vec(xs: &mut Vec<i64>)`. The caller's binding is mutated directly.

## Rust Escape Hatch

When Futuruna's abstractions don't cover a case, embed raw Rust:

```runa
@ rust {
    fn fast_sort(x: &mut [f64]) {
        x.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    }
}
```

The block is inserted verbatim into the generated Rust. Handles nested braces, strings, and comments.

## Using Rust Crates

### Declare a dependency
```runa
@ depend "serde" "1"
@ depend "tokio" "1"
```

Adds the crate to the generated `Cargo.toml`.

### Import Rust items
```runa
@ use std::collections::HashMap
@ use std::io::*
@ use serde::{Serialize, Deserialize}
```

Emits a Rust `use` statement in the generated code.
For Futuruna modules, use `@ import`.

## Build Modes

| Command | What it does |
|---------|-------------|
| `runa file.runa` | Interpret directly (no Rust compilation) |
| `runa run file.runa` | Compile to a native binary and execute it |
| `runa emit file.runa` | Print the generated Rust source |
| `runa build file.runa` | Transpile to Rust and compile to native binary |
| `runa lib file.runa` | Emit as a Rust library (no `fn main`, exported names get `pub`) |
| `runa verify file.runa` | Translate invariants to SMT-LIB2 and invoke Z3 |
| `runa hashes file.runa` | Show content-addressed hashes for all definitions |
| `runa registry file.runa` | Generate `<file>.registry.json` next to the source file |

### Build output

Native `run` and `build` artifacts use a dependency-complete compiler cache.
The cache records the root source, every transitive plain, qualified, and hash
import, its resolved import edges and manifest-resolution contexts, prelude mode,
and exact Futuruna compiler. An unchanged graph reuses the validated binary
before type checking or Rust code generation. Any source, import-resolution,
manifest, prelude-mode, or compiler change causes a miss. Programs with Cargo
dependencies also retain their generated Cargo project under `.runa-build/` for
Cargo's own incremental compilation.

`runa check` uses the same graph validation and caches only successful checks.
Its Rust metadata lane additionally fingerprints `rustc` and retains a
persistent rustc incremental workspace for changed graphs. Set
`FUTURUNA_COMPILER_CACHE_DIR` to choose a cache root,
`FUTURUNA_DISABLE_COMPILER_CACHE=1` to bypass it, or
`FUTURUNA_COMPILER_CACHE_TRACE=1` to report cache hits and misses on standard
error.

For a shorter edit loop, `runa check --frontend file.runa` stops after parsing,
import-aware type checking, calculation-contract checks, and Futuruna compiler
validation diagnostics. It deliberately skips complete Rust generation and
`rustc` or Cargo validation, and reports that reduced assurance in its success
message. Use ordinary `runa check` as the authoritative pre-commit and CI gate.

`runa check` also keeps `rustc` incremental state per canonical source root,
prelude mode, and Rust toolchain. Exact Futuruna check results still invalidate
when the Futuruna compiler changes, while compatible Rust backend work is reused
across compiler rebuilds. Byte-identical generated Rust is left untouched so its
filesystem identity remains useful to `rustc` and Cargo's incremental engines.
Successful backend validation is also cached by the complete generated Rust
hash and Rust toolchain fingerprint. A compiler rebuild or source-only metadata
change therefore reruns Futuruna analysis but does not ask `rustc` to prove an
identical backend artifact again. Programs containing raw `@ rust` blocks do not
use this content-only validation cache because those blocks may read external
files or compile-time environment values.

Cache validation includes the exact SHA-256 of the `runa` executable. That
digest is reused across CLI processes only while the executable's canonical
path, size, modification time, and platform file identity remain unchanged.
Replacing or rewriting the executable recomputes the digest before any
compiler or calculation artifact can be accepted; malformed fingerprint cache
entries are ignored.

For the precise compatibility boundary around emitted Rust, native build
artifacts, `runa lib`, and WASM package output, see
[../artifact-codegen-contracts.md](../artifact-codegen-contracts.md). The short
rule is: generated Rust behavior for stable pure/core programs is a contract;
exact emitted text, helper names, and private layout are internal unless an
artifact expectation fixture or doc explicitly promises them.

## Rust-Facing Library Contract

`runa lib file.runa` emits a Rust source file intended to be compiled into a
Rust library or included as a Rust module.

Stable today:

- exported Futuruna ADTs become public Rust structs/enums
- exported struct fields are public
- exported Futuruna functions become public Rust functions
- private Futuruna helpers remain private Rust items
- no binary `fn main` is emitted
- `String`, `List`, `Option`, `Result`, and exported ADTs use the documented
  type mapping below
- read-only non-copy parameters may be borrowed in Rust signatures, for example
  `Packet` as `&Packet`, `String` as `&String`, and `List(Int)` as `&Vec<i64>`
- Rust consumers can compile `runa lib` output that references external crates
  through `@ depend`, explicit `@ use` declarations, external-crate stdlib
  builtins, or raw `@ rust` blocks, as long as the consuming Cargo project
  provides the matching dependencies

Stable package layouts:

1. Include the generated file as a Rust module inside an ordinary Cargo package:

   ```bash
   runa lib library.runa > src/futuruna_lib.rs
   ```

   ```rust
   mod futuruna_lib;

   fn main() {
       let value = futuruna_lib::exported_function();
   }
   ```

2. Place the generated file at `src/lib.rs` inside a user-owned Cargo library
   crate, then depend on that crate from another Cargo package:

   ```bash
   mkdir -p generated/src
   runa lib library.runa > generated/src/lib.rs
   ```

   ```toml
   # generated/Cargo.toml
   [package]
   name = "futuruna_generated"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   # Add crates required by @ depend or dependency-backed builtins, for example:
   regex = "1"
   ```

   ```toml
   # app/Cargo.toml
   [dependencies]
   futuruna_generated = { path = "../generated" }
   ```

   ```rust
   fn main() {
       let value = futuruna_generated::exported_function();
   }
   ```

`@ depend` records the Cargo dependencies needed by generated Rust. In
`runa lib` mode, Futuruna emits source only; the consuming Cargo package owns
its `Cargo.toml` and must provide matching dependency entries.

When `runa lib` sees required Cargo dependencies, it prints a Cargo.toml-ready
dependency list to stderr and writes the same guidance into the generated Rust
header:

```rust
// Required Cargo dependencies for consumers:
//   regex = "1"
// Add these entries to the consuming Cargo.toml [dependencies].
```

If a Rust consumer omits one of those entries, Cargo will fail while compiling
the generated source. Treat that as a consumer package setup issue: copy the
listed entries into the consuming crate's `[dependencies]`.

Blocking evidence:

- `tests/expect/artifact/lib_export_contract.runa` snapshots the public/private
  emitted shape
- `./scripts/rust-interop-canary.sh` emits
  `tests/canary/interop/rust_consumer_lib.runa` with `runa lib`, compiles it
  with `rustc`, and runs an ordinary Rust consumer that calls exported structs,
  enums, borrowed params, lists, `Option`, and `Result`
- the same script emits
  `tests/canary/interop/rust_consumer_external_crate_lib.runa` with `runa lib`,
  compiles it inside an ordinary Cargo consumer with `CARGO_NET_OFFLINE=true`,
  and verifies `@ depend`, `@ use`, a raw Rust helper using `regex`, and the
  `regex_find_all` builtin
- the same canary also places generated output at `src/lib.rs` in a user-owned
  Cargo library crate and compiles a separate downstream Cargo package that
  depends on that generated library by path
- the same canary intentionally builds a Cargo consumer without the required
  `regex` dependency and verifies the generated source and `runa lib` stderr
  carried actionable dependency guidance before Cargo reports the missing crate

Outside the stable contract:

- exact helper names and private generated layout
- automatic manifest/package generation for `runa lib` consumers
- richer FFI patterns beyond ordinary Rust source inclusion and Cargo
  dependencies supplied by the consumer crate
- `runa from-rust`, which has a separate stable FRSS-v0 single-file validation
  contract rather than being part of the Rust-facing library contract; see
  [../from-rust-contract.md](../from-rust-contract.md) for its separate
  validation lane

## Type Mapping

| Futuruna | Rust | Copy? |
|----------|------|-------|
| `Int` | `i64` | Yes |
| `Float` | `f64` | Yes |
| `Bool` | `bool` | Yes |
| `Char` | `char` | Yes |
| `()` | `()` | Yes |
| `String` | `String` | No (Clone) |
| `List(a)` | `Vec<A>` | No (Clone) |
| `Option(a)` | `Option<A>` | No (Clone) |
| `Result(a, e)` | `Result<A, E>` | No (Clone) |
| `Pair(a, b)` | `Pair<A, B>` (struct with `fst`, `snd`) | No (Clone) |
| `a -> b` | `impl FnMut(A) -> B` or `impl FnOnce(A) -> B` | — |

Generic type variables (lowercase) are uppercased in Rust output: `a` becomes `A`, `b` becomes `B`.

**Copy vs Clone:** The escape analysis uses this distinction. Copy types (Int, Float, Bool, Char, unit) are freely duplicated — no clone needed. Clone types (String, List, structs, Option, Result) are cloned when used more than once and moved when used exactly once. You never write `.clone()` — the compiler inserts it.

The compiler chooses `FnOnce` when a closure parameter is used exactly once as a direct call and not inside a nested lambda. Otherwise it uses `FnMut`. This means closures that move captured non-Copy values (called once) get `FnOnce`, while closures called in loops or multiple times get `FnMut`.

## Multi-File Projects

### Flat import
```runa
@ import ./utils
```

Merges all definitions from `utils.runa` into the current scope. Transitive dependencies are loaded automatically.

### Qualified import
```runa
@ import Utils from ./utils
```

Definitions accessed as `Utils.function()`. Only `@ export`-marked definitions are visible.

### Content-addressed import
```runa
@ import #a1b2c3d4 from ./utils
```

Import by structural hash. Same hash = same code, regardless of filename. Inspired by Unison.

## Actors (Concurrency)

Actors compile to tokio tasks:

```runa
> actor counter(state: Int) {
    | Increment -> state + 1
    | Decrement -> state - 1
    | Reset -> 0
}
```

Generated Rust:
- A message enum with variants for each handler pattern
- An async `_run()` function with a receive loop
- A `_spawn()` helper that creates an mpsc channel and spawns the task
