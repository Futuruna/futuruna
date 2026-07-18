# 1. Hello, Futuruna

## Install

```bash
git clone https://github.com/Futuruna/futuruna.git
cd futuruna
cargo build --release
# Add to PATH:
export PATH="$PWD/target/release:$PATH"
```

## Your first program

Create `hello.runa`:

```runa
@ print("Hello, Futuruna!")
```

Run it:

```bash
runa hello.runa
```

Output:
```
Hello, Futuruna!
```

## What just happened?

The `@` rune marks **effects** — things that interact with the outside world.
`print` is a built-in effect that writes to stdout.

Every line in Futuruna starts with one of **seven runes**:

| Rune | Meaning | Example |
|------|---------|---------|
| `#` | What exists (types) | `# Color = Red \| Green \| Blue` |
| `>` | What happens (functions) | `> add(a: Int, b: Int) -> Int { a + b }` |
| `\|` | What must be true (rules) | `\| parent("alice", "bob")` |
| `=` | What is (bindings) | `= x = 42` |
| `~` | What flows (streams) | `~ clicks = subject()` |
| `@` | Where proofs stop (effects) | `@ print("hello")` |
| `?` | Prove it (verification) | `? balance_ok` |

## A real program

```runa
-- Define a type
# Greeting(name: String, message: String)

-- Define a function
> greet(name: String) -> Greeting {
    Greeting(name, "Welcome to Futuruna!")
}

-- Use it
= g = greet("World")
@ print(g.name + ": " + g.message)
```

```bash
runa greeting.runa
```

Output: `World: Welcome to Futuruna!`

## Next

[2. Types and Pattern Matching](02-types.md)
