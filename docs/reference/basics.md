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

Ordinary strings recognize `\n`, `\t`, `\r`, `\\`, and `\"`. Character literals
recognize `\n`, `\t`, `\r`, `\\`, and `\'`. Unknown escapes are errors at the
backslash. Write `"a\\qb"` to include a literal backslash before `q`.
The literal portions of triple-quoted strings preserve backslashes as written;
expressions inside `{{ ... }}` use normal Futuruna string and character syntax.

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

Arithmetic uses `Int` or `Float` operands. For `+`, `-`, `*`, and `/`, mixing
`Int` and `Float` converts the integer to `Float` and produces a `Float`.
Remainder (`%`) requires two `Int` values or two `Float` values. Use `to_float`
when an explicit conversion is needed. `+` also concatenates when either operand
is a `String`; use `concat` to combine lists.

### Comparison
| Op | Meaning |
|----|---------|
| `==`, `!=` | equality, inequality |
| `<`, `>`, `<=`, `>=` | ordering |

Primitive equality requires operands of the same type: `5 == 5.0` is a type
error; `to_float(5) == 5.0` compares two floats. Lists, tuples, and data
constructors support structural equality. Ordering compares numbers, two
strings, or two characters. Numeric ordering permits `Int`/`Float` mixing with
the same conversion as arithmetic. Floating-point conversion can lose integer
precision, so keep exact integer calculations in `Int`.

### Logical
| Op | Meaning |
|----|---------|
| `&&` | logical AND |
| `\|\|` | logical OR |
| `not(x)` | logical NOT (function) |

Logical operators require `Bool` operands. `!x` is also Boolean negation;
unary `-x` requires a number. Invalid operand types known to the frontend are
reported at the source expression before program effects run.

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

An `if` condition must return `Bool`. Numbers, strings, lists, and `()` are not
converted to Boolean values.

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

## Danish Source Files

A file whose first declaration is `@ sprog da` is written in Danish. Comments,
blank lines and a leading byte-order mark may precede the declaration, and a
comment may follow it on the same line. `@sprog da` and `@ language da` are the
same declaration. Language codes are case-insensitive: `da` or `dansk` selects
Danish, `en` or `english` selects English. Any other code, and a language
declaration after another declaration, is an error. A file without the
declaration is English.

The language of a file selects its keywords and the Danish names of builtins.
It never renames what the author declares, and it never changes how values
behave.

### Keywords

| Danish | English | Danish | English |
|--------|---------|--------|---------|
| `skel` | `match` | `hvis` | `if` |
| `ellers` | `else` | `med` | `with` |
| `undtagelse` | `exception` | `omfang` | `scope` |
| `på` | `on` | `aktør` | `actor` |
| `start` | `spawn` | `effekt` | `effect` |
| `modul` | `module` | `importer` | `import` |
| `brug` | `use` | `træk` | `trait` |
| `hvor` | `where` | `lad` | `let` |
| `gør` | `do` | `så` | `then` |
| `returner` | `return` | `håndter` | `handle` |
| `genoptag` | `resume` | `udfør` | `perform` |
| `Sandt`, `sandt` | `True` | `Falskt`, `falskt` | `False` |
| `og` | `and` | `eller` | `or` |

`og` binds more tightly than `eller`; both work in Boolean expressions and
between rule goals. English keywords, `&&` and `||` remain available. A keyword
may also be used as a parameter or field name (`start`, `under`); the name keeps
the Danish spelling. `indud` marks an `inout` parameter, `delt` a `shared` type,
and `@ eksport` / `@ afhæng` are `@ export` / `@ depend`.

### Danish names for builtins, types and constructors

A Danish file may call a builtin by its Danish name, and write builtin types
and the constructors of `Option` and `Result` in Danish:

| Danish | English | Danish | English |
|--------|---------|--------|---------|
| `Heltal` | `Int` | `Kommatal` | `Float` |
| `Tekst` | `String` | `Boolsk` | `Bool` |
| `Tegn` | `Char` | `Liste` | `List` |
| `Naturligt` | `Nat` | `Intet` | `None` |
| `Noget` | `Some` | `Fejl` | `Err` |

A Danish name refers to the builtin only when neither the file nor a module it
imports with a plain `@ importer` declares that name. Declared names — functions,
rules, bindings, parameters, fields, types and constructors — always mean what
the author declared, in every file:

```runa
@ sprog da
# Kontrol = Godkendt | Fejl          -- a user constructor named Fejl
> vis(s: Sag) -> Tekst { ... }       -- a user function named vis
@ print(vis(sag))                     -- calls the user function
@ print("""Beløb: {{beløb}}""")       -- interpolation always uses the builtin `show`
```

Here `Fejl` is the declared `Kontrol` constructor, and calculation schemas and
results publish it as `"Fejl"`. Where no user declaration exists, `Fejl(…)`,
`Noget(…)` and `Intet` construct the built-in `Result` and `Option` values,
which are always published and shown under their English names `Err`, `Some`
and `None`. A name after `.` is always a field or member name, never a builtin.

Names cross language boundaries unchanged: an English file that imports a Danish
module calls its functions by their declared Danish names (`tæl(1)`), and a
Danish file that imports an English module calls `tag(…)` or `hale(…)` as
declared there. English files do not know the Danish names; `hale([1, 2])` in an
English file is an undefined name unless the program declares it.

Diagnostics in a Danish file name builtins, types and keywords the way the
source line spells them (`fold`, not `foldl`; `returner`, not `return`).

| Danish | English |
|--------|---------|
| `vis` | `show` |
| `skriv` | `print` |
| `vis_heltal` | `show_int` |
| `vis_kommatal` | `show_float` |
| `kvrod` | `sqrt` |
| `potens` | `pow` |
| `til_kommatal` | `to_float` |
| `afrund` | `round` |
| `gulv` | `floor` |
| `længde` | `length` |
| `tekst_længde` | `string_length` |
| `opdel` | `split` |
| `saml` | `join` |
| `indeholder` | `contains` |
| `starter_med` | `starts_with` |
| `ender_med` | `ends_with` |
| `erstat` | `replace` |
| `til_store` | `to_upper` |
| `til_små` | `to_lower` |
| `deltekst` | `substring` |
| `tegn_ved` | `char_at` |
| `indeks_af` | `index_of` |
| `formater_kommatal` | `format_float` |
| `fortolk_heltal` | `parse_danish_int` |
| `fortolk_kommatal` | `parse_danish_float` |
| `tekst_tegn` | `string_chars` |
| `hoved` | `head` |
| `hale` | `tail` |
| `nte` | `nth` |
| `vend` | `reverse` |
| `tilføj` | `push` |
| `område` | `range` |
| `afbild` | `map` |
| `filtrer` | `filter` |
| `fold` | `foldl` |
| `sorter` | `sort` |
| `sorter_efter` | `sort_by` |
| `nogen` | `any` |
| `alle` | `all` |
| `flad_afbild` | `flat_map` |
| `par` | `zip` |
| `numerer` | `enumerate` |
| `tag_mens` | `take_while` |
| `spring_mens` | `drop_while` |
| `sum_liste` | `sum_list` |
| `unikke` | `distinct` |
| `tæl_efter` | `count_by` |
| `opdel_efter` | `partition` |
| `stykker` | `chunked` |
| `abonner` | `subscribe` |
| `læs_fil` | `read_file` |
| `skriv_fil` | `write_file` |
| `tilføj_fil` | `append_file` |
| `fil_eksisterer` | `file_exists` |
| `læs_linjer` | `read_lines` |
| `miljø_var` | `env_var` |
| `json_fortolk` | `json_parse` |
| `json_hent` | `json_get` |
| `json_tekst` | `json_string` |
| `json_tal` | `json_number` |
| `json_sand` | `json_bool` |
| `json_liste` | `json_array` |
| `json_udsend` | `json_emit` |
| `json_objekt` | `json_object` |
| `kort_nyt` | `map_new` |
| `kort_indsæt` | `map_insert` |
| `kort_hent` | `map_get` |
| `kort_hent_eller` | `map_get_or` |
| `kort_indeholder` | `map_contains` |
| `kort_fjern` | `map_remove` |
| `kort_nøgler` | `map_keys` |
| `kort_værdier` | `map_values` |
| `kort_poster` | `map_entries` |
| `kort_længde` | `map_len` |
| `kort_flet` | `map_merge` |
| `kort_fra` | `map_from` |
| `sæt_nyt` | `set_new` |
| `sæt_indsæt` | `set_insert` |
| `sæt_indeholder` | `set_contains` |
| `sæt_fjern` | `set_remove` |
| `sæt_længde` | `set_len` |
| `sæt_til_liste` | `set_to_list` |
| `sæt_forening` | `set_union` |
| `sæt_fælles` | `set_intersect` |
| `sæt_forskel` | `set_diff` |
| `sæt_fra_liste` | `set_from_list` |
| `fra_liste` | `from_list` |
| `tag` | `take` |
| `spring` | `skip` |
| `indsaml` | `collect` |
| `tæl` | `count` |
| `vindue` | `window` |
| `sidste` | `last` |
| `kombiner_seneste` | `combine_latest` |
| `flet` | `merge` |
| `første` | `first` |
| `reducer` | `reduce` |
| `start_med` | `start_with` |
| `sammenkæd` | `concat` |
| `parvis` | `pairwise` |
| `spørg` | `ask` |
| `delt` | `shared` |
| `ikke` | `not` |
| `find_alle` | `findall` |

### Numbers, sorting and printed values

`fortolk_heltal` and `fortolk_kommatal` (English `parse_danish_int` and
`parse_danish_float`) read Danish number text and return
`Result(Int, String)` and `Result(Float, String)`. Surrounding whitespace is
ignored. The text is an optional sign, an integer part written either as plain
digits or with `.` between groups of three digits, and — for `fortolk_kommatal`
only — an optional `,` followed by digits:

```runa
@ sprog da
fortolk_heltal("1.250.000")     -- Ok(1250000)
fortolk_kommatal("1.234,75")    -- Ok(1234.75)
fortolk_kommatal("1,5")         -- Ok(1.5)
fortolk_heltal("1,5")           -- Err("`1,5` is not a Danish integer")
fortolk_kommatal("1.5")         -- Err("`1.5` is not a Danish decimal number")
```

`parse_int` and `parse_float` read the language-neutral format with a decimal
point in every file. `sorter` orders strings by Unicode code point, and `vis`
prints values exactly as `show` does (`true`, `None`, `Some(3)`, `1.5`). Neither
depends on the language of the file, so a value sorts and prints the same
wherever it is used. Order Danish text with `sorter_efter` and an explicit key,
and format amounts for Danish readers with `formater_kommatal` and `erstat`.
