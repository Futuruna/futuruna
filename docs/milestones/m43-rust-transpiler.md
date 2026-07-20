# M43: Rust → Futuruna Transpiler

**Tagline:** "Bring your Rust code."

## Goal

Parse Rust source (via syn crate) and emit equivalent .runa source.
"Paste your Rust, see it in Futuruna" — the killer onboarding story.

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
| Lifetimes, `&`, `&mut` | Stripped (invisible ownership) |
| `unsafe { }` | `@ rust { }` (preserve) |
