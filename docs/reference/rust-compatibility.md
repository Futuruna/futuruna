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

## Build Modes

| Command | What it does |
|---------|-------------|
| `runa run file.runa` | Interpret directly (no Rust compilation) |
| `runa emit file.runa` | Print the generated Rust source |
| `runa build file.runa` | Transpile to Rust and compile to native binary |
| `runa lib file.runa` | Emit as a Rust library (no `fn main`, exported names get `pub`) |
| `runa verify file.runa` | Translate invariants to SMT-LIB2 and invoke Z3 |
| `runa hashes file.runa` | Show content-addressed hashes for all definitions |
| `runa registry file.runa` | Generate `.registry.json` mapping names to content hashes |

### Build output

Compiled output goes to `.runa-build/` (a Cargo project). Incremental compilation caches binaries in `runa-cache/` and skips unchanged files.

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
