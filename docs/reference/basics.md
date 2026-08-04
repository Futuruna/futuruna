---
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
---

# Futuruna Basics

Core language syntax: literals, types, operators, control flow, and closures.

## Literals

### Numbers
```runa
42          -- Int (i64)
3.14        -- Float (f64)
-7          -- negative Int
```

### Booleans
```runa
True        -- capitalized
False
```

### Strings
```runa
"hello"                     -- basic string
"line\nnewline"             -- escape sequences: \n \t \\ \"

"""
multi-line string
preserves newlines
"""

"""Result: {{x + 5}}"""    -- interpolation with {{ expr }}
```

Interpolation desugars to `"Result: " + show(x + 5)`.

### Characters
```runa
'a'         -- single character (Char type, compiles to Rust char)
```

### Lists
```runa
[1, 2, 3]               -- list literal
[]                       -- empty list
```

### Unit
```runa
()                       -- unit value and unit type
```

## Types

### Primitives
| Type | Rust equivalent | Example |
|------|----------------|---------|
| `Int` | `i64` | `42` |
| `Float` | `f64` | `3.14` |
| `String` | `String` | `"hello"` |
| `Bool` | `bool` | `True` |
| `Char` | `char` | `'a'` |
| `()` | `()` | `()` |

### Composite
| Type | Rust equivalent | Example |
|------|----------------|---------|
| `List(a)` | `Vec<A>` | `[1, 2, 3]` |
| `Option(a)` | `Option<A>` | `Some(42)`, `None` |
| `Result(a, e)` | `Result<A, E>` | `Ok(42)`, `Err("fail")` |
| `Pair(a, b)` | `Pair<A, B>` (struct with `fst`, `snd` fields) | `Pair(1, "x")` |

Tuple literals may span lines and may end in a trailing comma:

```runa
= sources = (
    primary_source,
    amendment_source,
)
```

Pair construction and field access:
```runa
= p = Pair(1, "hello")
@ print(show(p.fst))          -- 1
@ print(show(p.snd))          -- "hello"
```

### Function types
```runa
Int -> Bool              -- function from Int to Bool
(Int, Int) -> String     -- two-argument function
a -> b                   -- generic function type
```

### Generic type variables
Lowercase single letters are type variables: `a`, `b`, `c`, etc. They become uppercased Rust generics (`A`, `B`, `C`). Uppercase names like `T` pass through unchanged.

## Operators

### Arithmetic (precedence low to high)
| Op | Meaning |
|----|---------|
| `+`, `-` | addition, subtraction |
| `*`, `/`, `%` | multiplication, division, modulo |

### Comparison
| Op | Meaning |
|----|---------|
| `==`, `!=` | equality, inequality |
| `<`, `>`, `<=`, `>=` | ordering |

### Logical
| Op | Meaning |
|----|---------|
| `&&` | logical AND |
| `\|\|` | logical OR |
| `not(x)` | logical NOT (function) |

### Special operators
| Op | Meaning | Example |
|----|---------|---------|
| `\|>` | pipe-forward | `x \|> f` becomes `f(x)` |
| `<-` | send/push | `subject <- value` |
| `?.` | safe call | `expr?.field` (None propagation) |
| `?:` | elvis | `expr ?: default` (unwrap with fallback) |

The pipe operator inserts the left side as the first argument:
```runa
x |> f           -- f(x)
x |> f(a, b)     -- f(x, a, b)
x |> f |> g      -- g(f(x))
```

## Control Flow

### if/else
```runa
if condition { then_expr }
if condition { then_expr } else { else_expr }
if x > 0 { "positive" } else if x == 0 { "zero" } else { "negative" }
```

### match
```runa
match expr {
    | Pattern1 -> body1
    | Pattern2 if guard -> body2
    | _ -> default_body
}
```

The `|` before each arm is optional. Patterns can destructure ADTs:
```runa
match shape {
    | Circle(r) -> 3.14 * r * r
    | Rectangle(w, h) -> w * h
}

match point {
    | Point(x: xval, y: _) -> xval    -- named field destructuring
}
```

### for loop
```runa
for item in collection {
    @ print(show(item))
}
```

Works with `List`, `Stream`, and subjects.

## Closures

```runa
|x| x * 2                       -- single parameter
|x, y| x + y                    -- multiple parameters
|x: Int, y: Float| x + y        -- with type annotations
```

Closures capture their enclosing environment.

## Built-in Functions (Quick Reference)

For the complete standard library with all ~70 builtins, see [stdlib.md](stdlib.md).

### Display
| Function | Signature | Description |
|----------|-----------|-------------|
| `show` | `a -> String` | Convert any value to string |

### List operations
| Function | Signature | Description |
|----------|-----------|-------------|
| `length` | `List(a) -> Int` | List length |
| `head` | `List(a) -> a` | First element |
| `tail` | `List(a) -> List(a)` | All but first |
| `push` | `(List(a), a) -> List(a)` | Append element |
| `concat` | `(List(a), List(a)) -> List(a)` | Concatenate |
| `reverse` | `List(a) -> List(a)` | Reverse |
| `map` | `(List(a), a -> b) -> List(b)` | Map function |
| `filter` | `(List(a), a -> Bool) -> List(a)` | Filter |
| `foldl` | `(List(a), b, (b, a) -> b) -> b` | Left fold |
| `range` | `(Int, Int) -> List(Int)` | Range `[start, end)` |

### Math
| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `Int -> Int` | Absolute value |
| `sqrt` | `Float -> Float` | Square root |
| `pow` | `(Float, Float) -> Float` | Exponentiation |
| `round` | `Float -> Int` | Round to nearest |
| `floor` | `Float -> Int` | Floor |
| `max_int` | `(Int, Int) -> Int` | Maximum |
| `min_int` | `(Int, Int) -> Int` | Minimum |
| `clamp` | `(Int, Int, Int) -> Int` | Clamp to range |
| `to_float` | `Int -> Float` | Convert to float |

### String
| Function | Signature | Description |
|----------|-----------|-------------|
| `string_length` | `String -> Int` | Unicode scalar length |
| `starts_with` | `(String, String) -> Bool` | Prefix check |

### Option/Result
| Function | Signature | Description |
|----------|-----------|-------------|
| `unwrap_or` | `(Option(a), a) -> a` | Unwrap with default |
| `is_some` | `Option(a) -> Bool` | Check if Some |
| `is_none` | `Option(a) -> Bool` | Check if None |

### Logic
| Function | Signature | Description |
|----------|-----------|-------------|
| `not` | `Bool -> Bool` | Logical NOT |
| `assert` | `Bool -> ()` | Runtime assertion |
| `identity` | `a -> a` | Identity function |

## Comments

```runa
-- Line comment

----
Block comment
can span multiple lines
----
```
