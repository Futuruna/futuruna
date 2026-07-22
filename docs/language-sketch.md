# Futuruna: A Three-Dimensional Programming Language

**Design derived from Pareto-optimal syntactic analysis (#241-245)**
**Iterated through Futuruna Design Lab (tau-lang binary)**

## Design Principles (from the data)

The NSGA-II search over programming language token transitions discovered that
the optimal PL syntax requires:

1. **1-3 tunnels** — obligatory transitions that create structural certainty
2. **3 guided channels** — strong-but-not-absolute flow directions
3. **7-9 junctions** — moderate choice points that create texture
4. **1 hub** — a universal connector (identifiers)
5. **d_eff = 3** — three independent cognitive axes

## The Honest Status

| Design | S_τ(3) | JSD | Φ | d_eff | Status |
|--------|--------|-----|---|-------|--------|
| **Futuruna-v1** (realistic) | 3.256 | 0.613 | 0.890 | 2 | Beats all existing PLs on composite |
| **Futuruna-v3** (bold) | 3.418 | 0.622 | 0.885 | 2 | Highest S_τ, best base for tuning |
| **Futuruna-tuned** | 3.522 | 0.721 | 0.955 | 2 | Best realistic design achievable |
| **Futuruna-d3** (evolved) | 3.537 | 0.784 | 0.980 | **3** | Requires statement runes |
| Best existing PL | 3.115 | 0.749 | 0.937 | 2 | Scala on S_τ, Python on JSD, Prolog on Φ |

**Key finding:** d_eff=2 with Φ=0.955 is achievable with conventional syntax
(keywords, braces, type annotations). d_eff=3 requires one specific innovation:
**statement runes** — starting statements with operators instead of keywords.

## The Three Axes (from d_eff=3 analysis)

The eigenspace decomposition of the d_eff=3 member (λ: 2.28, 1.66, 0.96)
reveals three independent cognitive channels:

- **Axis 1 — Statement Kind** (which rune starts the line)
  - `>` definitions, `|` logic clauses, `#` type declarations, `@` annotations
  - This is the BIGGEST innovation: START→OP creates an independent dimension

- **Axis 2 — Type Flow** (TYPE → ARROW chains)
  - `Int -> Bool -> String` — Haskell/ML-style type signatures
  - COLON introduces types (partially) but also operators (dual use)
  - TYPE→ARROW at 71% — everything tends toward function types

- **Axis 3 — Block Composition** (BRACE nesting + DOT termination)
  - `{ ... { ... } ... }` — how blocks nest and compose
  - DELIM→BRACE at 75% — brackets open block contexts

## Concrete Syntax

### Statement Runes (Axis 1 — the d_eff=3 innovation)

Every statement begins with an operator rune that declares its nature:

```tau
-- > introduces a definition (function, value)
> add(a: Int, b: Int) -> Int {
    a + b
}

-- | introduces a logic clause (Prolog's soul)
| member(X, Cons(head: X, tail: _))
| member(X, Cons(head: _, tail: Tail)) -> member(X, Tail)

-- # introduces a type declaration
# List(T) = Nil | Cons(T, List(T))
# Tree(T) = Leaf(value: T) | Branch(left: Tree(T), right: Tree(T))

-- @ introduces an annotation or effect
@ pure
@ memoize
> fibonacci(n: Int) -> Int {
    match n {
        | 0 -> 0
        | 1 -> 1
        | n -> fibonacci(n - 1) + fibonacci(n - 2)
    }
}

-- = introduces a binding (like let)
= x: Int = 42
= name: String = "hello"
```

### Type Flow (Axis 2)

Types are first-class with strong `TYPE → ARROW` flow:

```tau
-- Function types (Haskell-like)
> compose(f: A -> B, g: B -> C) -> A -> C {
    |x| g(f(x))
}

-- Type-level functions
# Functor(F) = {
    > map(f: A -> B, fa: F(A)) -> F(B)
}

-- Colon introduces type context
> greet(name: String) -> String {
    "Hello, " + name
}
```

### Block Composition (Axis 3)

Blocks nest freely with clear visual boundaries:

```tau
-- Module blocks
> module Collections {

    # Tree(T) = Leaf(value: T) | Branch(left: Tree(T), right: Tree(T))

    > map(tree: Tree(A), f: A -> B) -> Tree(B) {
        match tree {
            | Leaf(x) -> Leaf(f(x))
            | Branch(l, r) -> Branch(map(l, f), map(r, f))
        }
    }

    -- Logic and functions compose:
    | balanced(Leaf(_))
    | balanced(Branch(l, r)) -> {
        balanced(l),
        balanced(r),
        abs(depth(l) - depth(r)) <= 1
    }
}
```

### Complete Example: All Three Axes

```tau
-- A small collection library showing all three dimensions

# Option(T) = None | Some(value: T)

# List(T) = Nil | Cons(head: T, tail: List(T))

-- Logic rules for structural properties
| empty(Nil)
| nonempty(Cons(_, _))

-- Functions on lists
> head(list: List(T)) -> Option(T) {
    match list {
        | Nil -> None
        | Cons(x, _) -> Some(x)
    }
}

> filter(list: List(T), pred: T -> Bool) -> List(T) {
    match list {
        | Nil -> Nil
        | Cons(h, t) -> {
            = rest: List(T) = filter(t, pred)
            if pred(h) { Cons(h, rest) } else { rest }
        }
    }
}

-- Higher-order with logic: find all solutions
> findall(pred: T -> Bool, candidates: List(T)) -> List(T) {
    filter(candidates, pred)
}

-- Rules can reference functions
| sorted(Nil)
| sorted(Cons(_, Nil))
| sorted(Cons(a, Cons(b, rest))) -> {
    a <= b,
    sorted(Cons(b, rest))
}

@ test
> test_filter() {
    = nums: List(Int) = [1, 2, 3, 4, 5]
    = evens: List(Int) = filter(nums, |x| x % 2 == 0)
    assert(evens == [2, 4])
}
```

## What Each Rune Creates

| Rune | Meaning | Axis | Verification lens | Think of... |
|-------|---------|------|-------------------|-------------|
| `#` | What exists | 1+2 | Z3 datatypes (state space) | "This is shaped like" — types, effects, traits, impls |
| `>` | What happens | 1 | Z3 functions (transitions) | "I am creating" — functions, actors, modules |
| `\|` | What should be true | 1 | Z3 assertions (invariants) | "This case" — logic clauses, match arms, handlers, scopes |
| `=` | What is | 1 | Z3 constants (ground truth) | "Right now" — local bindings |
| `~` | What flows | 1 | TLA+ temporal logic (behavior) | "Over time" — reactive streams |
| `@` | Where proofs stop | 1 | Proof boundary (effects) | "Meta-level" — effects, imports, annotations |
| `?` | Prove it | 1 | Solver invocation (verification) | "Demand proof" — verify invariants |

## Why Runes Create d_eff=3

In conventional languages (Kotlin, Scala), keywords like `fn`, `class`, `val`
all flow into similar continuations (identifiers, braces). They're
*syntactically similar* despite *semantically different*. This is why
Kotlin/Scala collapse to d_eff=1 — everything connects to everything.

Runes are *syntactically orthogonal*:
- `>` flows to IDENT (function name) then PAREN (args) — definition flow
- `|` flows to IDENT (pattern) then ARROW (body) — clause flow
- `#` flows to TYPE then OP (=) — type definition flow

Three different START→OP→... chains create three genuinely independent
pathways through the token graph. The eigenspace captures this as three
non-degenerate eigenvalues.

## Comparison With Existing Languages

### vs Prolog (d_eff=2, Φ=0.937)
Prolog has clauses (Axis 1) and data (lists, tuples) but no types (Axis 2)
and no blocks (Axis 3). Futuruna adds typed logic + block composition.

### vs Rust (d_eff=1, Φ=0)
Rust has types (Axis 2) and blocks (Axis 3) but no logic (Axis 1). All
token types serve similar structural roles → one dimension. Futuruna adds
logic clauses + statement runes to create independent axes.

### vs Kotlin/Scala (d_eff=1, Φ=0)
Multi-paradigm richness creates uniformity: every token can go everywhere.
Futuruna's runes create *constraints* that make the paradigms syntactically
distinct. The constraints ARE the consciousness.

### vs Haskell (d_eff=2, Φ=0.883)
Haskell has type flow + pattern matching but uniform whitespace syntax
collapses visual distinctiveness. Futuruna adds runes for visual axis
independence + explicit blocks for the third dimension.

## Implementation Strategy

1. **Parser:** Recursive descent, rune-dispatched (the first token on
   each line determines the parsing strategy — very simple)

2. **Type system:** Bidirectional type checking, Hindley-Milner core
   with explicit annotations at definition boundaries

3. **Logic engine:** Tabling (like SLG resolution in XSB Prolog) for
   termination guarantees on recursive rules

4. **Compilation:**
   - Logic rules → tabled resolution engine
   - Typed functions → LLVM IR (or Cranelift for fast compilation)
   - Blocks → closure conversion with stack allocation

5. **Effect system:** Logic rules are implicitly non-deterministic.
   Functions are pure by default. `@` annotations declare effects.

## Default Logic: Law as a First-Class Citizen (#246)

Catala achieves Φ=0.921 through keyword chains for legal reasoning. Futuruna can
absorb this capability without any new axes — default logic fits naturally
within the `|` rune pathway.

### The Syntax: `under` and `exception`

```tau
-- Prolog-style inference: who is taxable?
| taxable(person) -> resident(person), has_income(person)
| resident(person) -> person.address.country == "DK"

-- Catala-style default logic: what's the rate?
-- Rules are tried in order; later rules override earlier ones
| tax_rate(person) -> 0.20
| tax_rate(person) -> 0.40 under person.income > 50000
| exception tax_rate(person) -> 0.00 under person.income < 12000

-- Function that uses both logic AND law
> compute_tax(person: Person) -> Money {
    if taxable(person) {
        person.income * tax_rate(person)
    } else {
        0
    }
}
```

### Scoped Default Logic (Catala's Scopes)

```tau
-- | scope creates a legal reasoning context
| scope IncomeTax {

    -- Type declarations work inside scopes
    # TaxBracket = Low | Medium | High

    -- Default definitions (lowest priority)
    | bracket(person) -> Low
    | rate(bracket: TaxBracket) -> 0.20

    -- Condition-guarded overrides
    | bracket(person) -> Medium under person.income > 50000
    | bracket(person) -> High under person.income > 150000
    | rate(High) -> 0.45

    -- Named exceptions (can be referenced from other scopes)
    | exception student_exemption
      bracket(person) -> Low under person.is_student

    -- Computed definition using both logic and functions
    > tax(person: Person) -> Money {
        person.income * rate(bracket(person))
    }
}
```

### Why This Works Without Breaking d_eff=3

The `under` and `exception` keywords live WITHIN the `|` pathway — they don't
create new transition patterns at the token level, they enrich the existing
Axis 1 (statement kind). The three axes remain:

1. **Axis 1 — Statement Kind** (rune): `>` computation, `|` logic+law, `#` types
2. **Axis 2 — Type Flow**: TYPE→ARROW chains
3. **Axis 3 — Block Composition**: BRACE nesting

Catala's KW→KW chains (Φ boost) happen inside `|` blocks, adding legal
reasoning vocabulary without inflating the number of hubs. Measured:
Futuruna-Law variant shows Φ=0.899 (up from 0.885) while preserving d_eff=2.

### What Futuruna Unifies

| Capability | Catala | Prolog | Rust | Futuruna |
|-----------|--------|--------|------|-----|
| Default logic with exceptions | Yes | No | No | **Yes** |
| Prolog-style inference | No | Yes | No | **Yes** |
| Typed functions | Partial | No | Yes | **Yes** |
| Pattern matching | Partial | Yes | Yes | **Yes** |
| Type-level programming | No | No | Yes | **Yes** |
| Block composition | No | No | Yes | **Yes** |
| d_eff | 2 | 2 | 1 | **3** |

**Futuruna is the first language where law IS logic IS computation** — and the
syntax is Pareto-optimal for human cognition.

## Named Fields (#250)

All constructor fields **must** be named. No positional-only fields. No guessing.
Leave no one guessing — Kotlin is better for it, and so is Futuruna.

```tau
# Circle(radius: Float)
# Rectangle(width: Float, height: Float)

# Shape = Circle(radius: Float)
       | Rectangle(width: Float, height: Float)

# List(T) = Nil | Cons(head: T, tail: List(T))
```

### Why This Doesn't Break d_eff=3

Named fields use `IDENT : TYPE` inside parentheses — the exact same token flow
as function parameters. This is Axis 2 (type flow). No new syntactic pathway is
created; we're reusing the `name: Type` pattern that already exists in `>` definitions.

Token transition: `LPAREN → IDENT → COLON → TYPE → COMMA → ...`
This is identical to the parameter parsing path. Same axis, same transitions.

Unnamed fields produce a clear error:
```
Parse error: constructor fields must be named — write `x: Type` instead of just a type
```

### Named Field Access (dot notation)

```tau
= c: Shape = Circle(radius: 5.0)
= r: Float = c.radius               -- dot access by field name

-- Also works in destructuring and pattern matching:
match shape {
    | Circle(radius: r) -> r * r * 3.14159
    | Rectangle(width: w, height: h) -> w * h
}

-- Positional destructuring also works (binds in declaration order):
match shape {
    | Circle(r) -> r * r * 3.14159
    | Rectangle(w, h) -> w * h
}
```

### Construction

```tau
-- Positional (arguments in declaration order):
= c = Circle(5.0)

-- Named (explicit):
= c = Circle(radius: 5.0)

-- Named can be reordered:
= r = Rectangle(height: 10.0, width: 5.0)
```

### Transpilation to Rust

Named-field variants emit Rust struct variants:

```rust
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle(f64, f64, f64),   // positional → tuple variant
}
```

## Sealed Interfaces (#250)

Kotlin's sealed interfaces do two things: restrict implementations (closed
hierarchy) and provide shared behavior. Futuruna's `#` ADTs already seal the hierarchy.
What's missing is **methods that live on the type**.

### The Design: Method Blocks on `#` Types

```tau
# Shape = Circle(radius: Float) | Rectangle(width: Float, height: Float) {

    -- Methods: > inside # block
    > area(self) -> Float {
        match self {
            | Circle(radius: r) -> 3.14159 * r * r
            | Rectangle(width: w, height: h) -> w * h
        }
    }

    > describe(self) -> String {
        "Shape with area " + show(self.area())
    }

    -- Abstract method (no body): must be provided by... whom?
    -- In Futuruna, the answer is: you can't leave it abstract.
    -- The type is sealed. All variants are known. Just match.
    -- This is the key difference from Kotlin: no separate impl per variant.
}
```

### Why Not Per-Variant Impls?

In Kotlin, each subclass of a sealed interface provides its own `override`:

```kotlin
sealed interface Shape {
    fun area(): Double
}
data class Circle(val radius: Double) : Shape {
    override fun area() = Math.PI * radius * radius
}
```

This is the OOP dispatch model: behavior is scattered across classes.

Futuruna takes the **functional dispatch model**: behavior lives in one place via
pattern matching. This is better for sealed types because:

1. **All cases visible at once** — you see the full truth table, not fragments
2. **Exhaustiveness is natural** — `match` already checks it
3. **Adding a variant** forces updating every method (compile error), same as Kotlin
4. **No expression problem** — adding methods is trivial (just add another `>` in the block)

### Shared Behavior (Default Methods)

Methods in a `#` block can call other methods on `self`:

```tau
# Http = Get(url: String) | Post(url: String, body: String) {

    > url(self) -> String {
        match self {
            | Get(url: u) -> u
            | Post(url: u, body: _) -> u
        }
    }

    -- Default method using other methods:
    > is_same_host(self, other: Http) -> Bool {
        host(self.url()) == host(other.url())
    }
}
```

### Composition: Extending Sealed Types

For the Kotlin use case of sealed interface hierarchies, Futuruna uses **composition
via type unions**:

```tau
# Solid = Circle(radius: Float) | Rectangle(width: Float, height: Float)
# Hollow = Ring(inner: Float, outer: Float) | Frame(width: Float, height: Float, border: Float)

-- Union type: all variants from both
# Shape = Solid | Hollow

-- Or more explicitly with shared methods:
# Shape = Circle(radius: Float)
       | Rectangle(width: Float, height: Float)
       | Ring(inner: Float, outer: Float) {

    > area(self) -> Float {
        match self {
            | Circle(radius: r) -> 3.14159 * r * r
            | Rectangle(width: w, height: h) -> w * h
            | Ring(inner: i, outer: o) -> 3.14159 * (o * o - i * i)
        }
    }
}
```

### Why This Doesn't Break d_eff=3

Method blocks use `{ > ... }` inside `#` declarations. This is:
- Axis 1 (statement kind): `>` inside `#` — definition within type
- Axis 2 (type flow): return types, parameter types — same as always
- Axis 3 (block composition): `{ }` nesting — same as always

No new token transition pathways. The `>` rune already flows to IDENT→PAREN.
Placing it inside a `#` block is just Axis 3 composition. The eigenvalue
structure is preserved because we're combining existing axes, not creating new ones.

### The Kotlin Comparison

| Feature | Kotlin sealed interface | Futuruna `#` with methods |
|---------|------------------------|---------------------|
| Closed hierarchy | `sealed interface` keyword | `#` ADT (always sealed) |
| Named fields | `data class(val x: T)` | `Variant(x: T)` (required) |
| Dot access | `obj.x` | `obj.x` |
| Methods | `override fun` per subclass | `> method(self)` with `match` |
| Default methods | `fun` in interface body | `> method(self)` calling other methods |
| Exhaustive match | `when` + compiler check | `match` + compiler check |
| Multiple inheritance | multiple sealed interfaces | composition via type unions |
| d_eff | 1 | 3 |

The trade-off: Kotlin lets you put logic *near* the data class definition.
Futuruna puts all cases *together* in one `match`. For sealed types (where you know
all variants), Futuruna's approach is strictly better — you never have to hunt through
files to find all implementations.

## Traits and Impl Blocks (#251)

Rust's trait system is the gold standard for zero-cost abstractions. Futuruna adopts
traits and impl blocks to enable **Rust ecosystem interop** — write Futuruna, transpile
to Rust, use any crate.

### Trait Declaration

```tau
# trait Printable {
    > display(self) -> String
}

# trait Greetable {
    > greet(self) -> String {
        "Hello, " + display(self)     -- default body
    }
}
```

Traits use `# trait Name { }` — stays within Axis 1 (`#` rune), same pathway as
`# effect`. Methods inside use `>` rune, consistent with all other function definitions.

### Impl Blocks

```tau
# impl Printable for Color {
    > display(self) -> String {
        match self {
            | Red -> "Red"
            | Green -> "Green"
            | Blue -> "Blue"
        }
    }
}
```

`# impl Trait for Type { }` — again Axis 1. The `for` keyword bridges trait name
to concrete type.

### Use Declarations

```tau
@ use std::collections::HashMap
@ use std::io::{Read, Write}
```

`@ use path` — meta-level imports via the `@` rune. Supports `::` paths and `{group}`
imports. Transpiles directly to Rust `use` statements.
Futuruna module loading belongs under `@ import`; legacy `@ use grundlov::*`-style module imports remain as deprecated compatibility.

### Why This Enables Rust Interop

The Kotlin model: write Kotlin, use Java libraries seamlessly. Futuruna does the same
with Rust via `runa --emit rust`:

1. `# trait` → `trait` in Rust
2. `# impl` → `impl` in Rust
3. `@ use` → `use` in Rust
4. Named fields → struct variants in Rust enums

A Futuruna developer can `@ use serde::Serialize` and `# impl Serialize for MyType`,
then `runa --emit rust` produces valid Rust that compiles with `serde` in `Cargo.toml`.

### d_eff Preservation

Traits and impls are `# → KW → IDENT` — the same Axis 1 pathway as `# Name = ...`
and `# effect Name { }`. No new cognitive axis created. The three eigenvalues are
unchanged because these are new token *values* flowing through existing *transitions*.

## Open Questions

1. **Backtracking scope:** When a `|` clause fails, how far does backtracking
   extend? Only within `match`? Into calling functions? The Prolog answer
   (global backtracking) creates problems; the Mercury answer (determinism
   declarations) adds complexity.

2. **Logic-function boundary:** Can `|` rules call `>` functions? Can `>`
   functions call `|` rules? If yes, what are the semantics when a rule
   produces multiple solutions inside a function?

3. **Module system:** How do `> module { }` blocks interact with the type
   and logic systems? Can rules be scoped to modules?

4. **Performance:** The rune-dispatch model is efficient to parse but the
   logic-function interop needs careful compiler design.

5. **Default logic semantics:** When `under` guards conflict, what is the
   resolution order? Catala uses textual order (later overrides earlier) with
   explicit `exception` labels. Should Futuruna use the same? Or specificity-based
   (more specific conditions win)? The `exception` keyword creates a naming
   mechanism — can exceptions reference each other across scopes?

6. **Legal verification:** Can `| scope` blocks be formally verified for
   completeness (every input has a defined output) and consistency (no
   contradictory exceptions)? This is exactly what law needs and what Catala
   was designed for. The type system should help here.

## Name

**Futuruna** — from τ (tau):
- τ (tau) is the planning horizon in S_τ — the language maximizes freedom of action
- τ is 2π — the full circle, all three axes rotating together
- The first programming language designed from consciousness theory (IIT) and entropy theory
