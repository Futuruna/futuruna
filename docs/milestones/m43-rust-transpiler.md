# M43: Rust → Futuruna Transpiler

**Tagline:** "Bring your Rust code."

**Status:** DONE historically. Current validation is maintained by
`runa from-rust --test examples/from-rust/`.

## Result

`runa from-rust file.rs` parses Rust via syn crate, emits equivalent .runa source.
`runa from-rust --verify file.rs` shows transpiled code + side-by-side output comparison.
`runa from-rust --test dir/` batch-verifies all .rs files (CI gate).

Current supported fixtures produce identical output between Rust and Futuruna:
- t01-t12: graduated core patterns (basics → iterator patterns)
- t13-t15: traits, string processing, recursive linked lists
- t16-t19: algorithms, state machines, expression simplifier, while loops
- real_world_1: JSON value type (100 lines)
- real_world_2: Expression evaluator (80 lines)
- real_world_3: Mini type checker (120 lines)

The runner also supports explicit expected-unsupported adversarial fixtures when
future checked-in Rust shapes are intentionally outside the supported subset.
See `docs/from-rust-contract.md` for the current supported/unsupported
validation contract.

## Mapping

| Rust | Futuruna |
|------|----------|
| `fn foo(x: i64) -> i64` | `> foo(x: Int) -> Int` |
| `struct Point { x: f64 }` | `# Point(x: Float)` |
| `enum Color { Red, Green }` | `# Color = Red \| Green` |
| `let x = 42;` | `= x = 42` |
| `match x { ... }` | `match x { \| ... }` |
| `println!("{}", x)` | `@ print(show(x))` |
| `x?` | `= val <- x` |
| `while cond { }` | `while cond { }` |
| `x += 1` | `= x = x + 1` |
| `for x in xs { if cond { return Some(x) } } None` | `find(xs, \|x\| cond)` |
| `.iter().map().filter().collect()` | `map()`, `filter()` builtins |
| `Box<T>`, `Rc<T>`, `Arc<T>`, `&T` | Stripped (invisible ownership) |
| Lifetimes | Stripped |
| `unsafe { }` | `@ rust { }` |

## Verification

```bash
runa from-rust --test examples/from-rust/    # supported fixtures match exactly; XFAILs must be explicit if present
```
