---
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
---

# Futuruna Basics

Core language syntax: literals, types, operators, control flow, and closures.

New to the language? The [guided tutorial](https://futuruna.com/docs/tutorial)
builds a small rule-driven program, runs a concrete scenario, attaches typed
metadata, and audits an actual same-rule contradiction.

## Literals

### Numbers
```runa
42          -- Int (i64)
3.14        -- Float (f64)
-7          -- negative Int
```

Write thousands without commas or underscores: `1000`. Digit separators such
as `1_000` are unsupported. Commas separate arguments and list
items, so `[1,000, 2]` means `[1, 0, 2]`. Leading zeroes do not change an
integer's decimal value. There is no date literal: `2024-01-31` is subtraction
and evaluates to `1992`. Use a string such as `"2024-01-31"` for date text, or
an explicitly defined date record with the validation your model requires.

`%` is a binary remainder operator, not a percentage suffix. Represent a rate
as `25` percent or `0.25` as a fraction, and keep that unit consistent in the
model. A trailing `%` continues onto the next line because it needs a right
operand.

`Int` is a signed 64-bit integer. Addition, subtraction, multiplication,
division, remainder, and negation fail if the result cannot be represented.
Integer division or remainder by zero also fails. These operations do not
wrap or substitute zero in either interpreted or compiled execution. Division
truncates toward zero; a nonzero remainder has the dividend's sign.

Ordinary execution stops with an error. Typed calculations report the invalid
value as a case diagnostic; see [calculation errors](calculations.md).

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
An ordinary string such as `"Hello {name}"` keeps its braces literally. Use
`"""Hello {{name}}"""` for interpolation. Single quotes enclose one character,
not a string.

When either operand of `+` is a string, the other value is converted to text:
`"10" + 5` produces `"105"`. This is concatenation, not numeric addition. Parse
numeric input explicitly before doing arithmetic.

### Characters
```runa
'a'         -- single character (Char type, compiles to Rust char)
```

### Lists
```runa
[1, 2, 3]               -- list literal
[]                       -- empty list
```

Use `concat(left, right)` to concatenate lists. The `+` operator does not
concatenate lists.

Compare a list with `[]` using `==` or `!=`, in either operand order. The
empty list takes its element type from the other operand, including for
string lists and nested lists.

List operations are ordinary function calls: `length(items)` and
`map(items, |item| item * 2)`. A list does not expose `.length`, `.len()` or
`.map(...)` members.

### Unit
```runa
()                       -- unit value and unit type
```

## Layout and Multiline Syntax

A newline normally ends a statement. It is a continuation instead when the
grammar makes continuation unambiguous:

- inside parentheses `(...)` or brackets `[...]`;
- after an incomplete token such as `=`, `->`, `,`, or an operator; or
- before a continuation token such as `|>`, `.`, `under`, or `else`.

Use a newline or `;` between adjacent expressions. For example, `= x = 5 5`
is an error; write `= x = 5; 5` if you intend two statements. Explicit rune
boundaries and compact forms such as `{ @ print("hello") 5 }` remain valid.
Unrecognized characters in code are errors, rather than ignored text.
Strings, character literals, triple-quoted templates and `----` quotation
blocks must have their closing delimiters. Legal quotations and strings may
still contain punctuation such as `§`, curly quotation marks and dashes.

This lets compact and multiline forms mean the same thing:

```runa
= compact = calculate(case, [1, 2, 3]) |> cap(100)

= multiline =
    calculate(
        case,
        [
            1,
            2,
            3,
        ],
    )
    |> cap(100)
```

Delimited sequences accept a trailing comma. This applies consistently to
function and rule parameters, calls, constructors, type fields and arguments,
patterns, lists, tuples, closures, proof arguments, effect handlers, and grouped
`@ use` imports. The formatter keeps one item per line when a sequence is
already multiline.

Block braces `{...}` contain statements, so their newlines remain statement
boundaries. Explicit list-shaped brace syntax, such as grouped `@ use {...}`
imports, handles its own comma-separated items. Continue a block expression by
leaving an incomplete token at the end of the line:

```runa
> total(base: Int, adjustment: Int) -> Int {
    base +
    adjustment
}
```

For algebraic data types, put `|` after a variant when another variant follows.
That makes the continuation explicit without confusing the next variant with a
top-level rule:

```runa
# Shape =
    Circle(radius: Float) |
    Rectangle(
        width: Float,
        height: Float,
    )
```

Do not rely on continuation inference for a line-leading `+`, `-`, or `||`.
Put these operators at the end of the preceding line, or wrap the whole
expression in parentheses, to make the intended continuation explicit.
For example, `= net = 100` followed by an indented `- 30` keeps `net` at
`100`: the second line is a separate expression statement whose result is
discarded. Indentation alone does not join these statements.

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
| `Pair(a, b)` | `(A, B)` | `Pair(1, "x")` |

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
@ print(show(p.fst)) -- 1
@ print(show(p.snd)) -- hello
```

A nonexistent field is an error. Known receiver types are checked before
execution; an untyped receiver is checked when the field is accessed. A real
field whose value is `()` remains a valid field.

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
