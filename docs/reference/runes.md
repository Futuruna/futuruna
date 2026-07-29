---
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
---

# The Seven Runes

Every statement in Futuruna begins with a rune — a single character that declares what the statement *is*.

| Rune | Question | What it does |
|------|----------|-------------|
| `#` | What exists? | Types, effects, traits, impls |
| `>` | What happens? | Functions, actors, modules |
| `\|` | What must be true? | Rules, invariants, handlers, scopes |
| `=` | What is? | Bindings, monadic bind |
| `~` | What flows? | Reactive streams, subjects |
| `@` | Where do proofs stop? | IO, imports, dependencies, meta |
| `?` | Prove it. | Verification demands |

---

## `#` -- What exists

Defines the shape of data: types, algebraic effects, traits, and implementations.

### Struct (single-variant product type)
```runa
# Point(x: Float, y: Float)
# Weather(city: City, temp: Float, condition: Condition, wind_kph: Float)
```

Construction uses **positional** arguments:
```runa
= p = Point(1.0, 2.0)
= w = Weather(Copenhagen, 22.0, Sunny, 10.0)
```

Fields are accessed with dot notation: `w.temp`, `w.condition`.

### Enum (multi-variant algebraic data type)
```runa
# Color = Red | Green | Blue
# Shape = Circle(radius: Float) | Rectangle(width: Float, height: Float)
# Option(a) = None | Some(a)
# List(a) = Nil | Cons(head: a, tail: List(a))
```

### ADT with methods
```runa
# Color = Red | Green | Blue {
    > name(c) -> String {
        match c {
            | Red -> "red"
            | Green -> "green"
            | Blue -> "blue"
        }
    }
}
```

Methods are standalone functions. The first parameter (without type annotation) receives the ADT type.

### Product types with rule members
```runa
# TaxCase(person: Person, rates: Rates) {
    | taxable_income() -> person.gross_income
    | tax_due() -> taxable_income() * rates.percent / 100

    > label() -> String { "tax:" + show(tax_due()) }
}

= tax = TaxCase(Person(1000), Rates(25))
= due = tax.tax_due()
= label = tax.label()
```

When a product type body contains `|` entries, those entries are rule members
of the product value. This is the RuleScope model: a pure calculation object
whose constructor inputs are visible inside scoped rules. Rule members can call
sibling rule members, ordinary global functions/rules, and use `under` /
`exception` with the same priority semantics as top-level rules. Rule member
names do not leak globally.

The same product body may contain ordinary `>` methods. Methods share the
product instance and can call rule members with `tax_due()` or `self.tax_due()`.
Fields are also available in product methods, so `person.gross_income` works in
both `|` rule members and `>` methods. A `|` rule member and `>` method cannot
use the same member name.

RuleScope is different from `| scope Name { ... }`: `| scope` owns reactive
lifecycle work such as subjects, streams, subscriptions, and teardown. A
RuleScope has no mutation or lifecycle ownership.

### Effect declaration
```runa
# effect Console {
    > say(msg: String) -> ()
    > ask(prompt: String) -> String
}
```

Defines abstract operations that callers can intercept via `| handle`.

### Trait declaration
```runa
# trait Printable {
    > display(self) -> String
}

# trait Greetable {
    > greet(self) -> String {
        "Hello, " + display(self)    -- default implementation
    }
}
```

### Impl block
```runa
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

---

## `>` -- What happens

Defines transformation: functions, actors, and modules.

### Function
```runa
> add(a: Int, b: Int) -> Int { a + b }

> greet(name: String) -> String {
    "Hello, " + name + "!"
}
```

Parameters can omit type annotations (inferred). Return type after `->`.

### Function with effects
```runa
> process(item: String) -> String with Console, Logger {
    say("Processing: " + item)
    log("info", "processed " + item)
    item
}
```

The `with` clause declares which effects the function may perform.

### Function with inout (mutable value semantics)
```runa
> sort_vec(xs: inout List(Int)) -> () {
    @ rust { xs.sort(); }
}
```

`inout` parameters are passed as `&mut T`. The caller's value is mutated in place.

### Generic function
```runa
> map_list(xs: List(a), f: a -> b) -> List(b) {
    match xs {
        | Nil -> Nil
        | Cons(h, t) -> Cons(f(h), map_list(t, f))
    }
}
```

Lowercase type variables (`a`, `b`) become Rust generics.

### Actor
```runa
> actor counter(state: Int) {
    | Increment -> state + 1
    | Decrement -> state - 1
    | Reset -> 0
}
```

Actors have a state parameter and message handlers. Each handler returns the new state. Compiles to a tokio task with an mpsc channel.

### Module
```runa
> module Math {
    > square(x: Int) -> Int { x * x }
    > cube(x: Int) -> Int { x * x * x }
}
```

Modules can be nested. Contents are accessed via `Math.square(5)`.

---

## `|` -- What must be true

Declares rules, invariants, effect handlers, and scopes. The most versatile rune.

### Logic rules (Prolog-style)
```runa
| taxable(person) -> resident(person), has_income(person)
```

### Default rules with overrides (Catala-style)
```runa
| advisory(w) -> "all clear"
| advisory(w) -> "heat warning" under w.temp > 35.0
| exception heatwave advisory(w) -> "danger" under w.temp > 45.0
```

Rules are evaluated top-down. `under` adds a guard condition. `exception <label>` overrides all other rules for the same head when its condition holds. The label (here `heatwave`) names the exception for readability and debugging — it does not affect evaluation.

### Named invariants (verification targets)
```runa
| name: subject_expr -> predicate_expr
```

Defines a named predicate that `?` can check. The subject expression is the value being tested; the predicate expression must return `Bool`.

```runa
= balance = 1000
= max_supply = 1000000
| balance_bounded: balance -> balance >= 0 && balance <= max_supply
```

The name before `:` is the invariant name. The expression between `:` and `->` is the subject (captured by `? name: val`). The expression after `->` is the predicate.

### Effect handlers
```runa
= result = | handle Console {
    | say(msg) -> { @ print("[console] " + msg); resume(()) }
    | ask(prompt) -> { @ print("[console] " + prompt); resume("default") }
} in greet("World")
```

Intercepts effect operations from the `in` body. `resume(value)` continues execution with the given return value.

### Scope blocks (lifecycle management)
```runa
| scope WeatherStation {
    ~ readings = subject()
    readings <- 42
    @ print(show(readings))
}
```

Scopes group statements with lifecycle management. Subjects, streams, and
live subscriptions within a scope are cleaned up when the scope ends.
Named scopes are also the explicit owner required for live subscriptions
started inside ordinary functions. See
[docs/stream-lifetimes.md](../stream-lifetimes.md).

### Match arms
Inside a `match` expression, `|` introduces each arm (see basics.md for match syntax).

---

## `=` -- What is

Binds a name to a value. Ground truth at a point in time.

### Simple binding
```runa
= x = 42
= name = "hello"
= result = add(20, 22)
```

### With type annotation
```runa
= x: Int = 42
= name: String = "hello"
```

### Monadic bind (early return)
```runa
= value <- parse_int("42")
```

If the expression returns `Ok(v)` or `Some(v)`, binds `v` and continues. If `Err(e)` or `None`, returns immediately (early return). Equivalent to Rust's `?` operator.

```runa
> add_parsed(a_str: String, b_str: String) -> Result(Int, String) {
    = a <- parse_int(a_str)
    = b <- parse_int(b_str)
    Ok(a + b)
}
```

---

## `~` -- What flows

Declares reactive streams and subscribes to them. Values that change over time.

The `~` rune has two forms:
1. **Binding** (`~ name = expr`) — creates a stream
2. **Subscription** (`~ expr | arms`) — consumes a stream with event handling

### Stream binding
```runa
~ nums = from_list([1, 2, 3, 4, 5])
~ doubled = map(nums, |x| x * 2)
~ big = nums |> filter(|x| x > 3)
```

### Subscription (`~ + |`)
```runa
-- Subscribe to a stream with value handling
~ nums | x -> { @ print(show(x)) }

-- With error handling
~ nums
    | x -> { @ print(show(x)) }
    | Err(e) -> { @ print("error: " + show(e)) }

-- Full lifecycle (value + error + completion)
~ nums
    | x -> { @ print(show(x)) }
    | Err(e) -> { @ print("error: " + show(e)) }
    | Complete -> { @ print("stream ended") }

-- Pipeline ending in subscription
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
    | Err(e) -> { log(e) }
```

The `|` arms handle three stream events: values, errors, and completion. This replaces `for` loops on streams. Use `for` for lists/ranges; use `~ + |` for streams.

See [streams.md](streams.md) for the full stream API and subscription reference.
For lifetime ownership rules around function-local subscriptions, see
[docs/stream-lifetimes.md](../stream-lifetimes.md).

### Subject creation (push-based streams)
```runa
~ clicks = subject()              -- empty subject
~ temp = subject(20.0)            -- with initial value
~ history = subject(0, 10)        -- replay subject (buffer last 10)
```

### Push values into subjects
```runa
clicks <- "click1"
clicks <- "click2"
temp <- 25.0
```

### Subject properties
```runa
clicks.count       -- number of values pushed
temp.latest        -- most recent value
```

---

## `@` -- Where proofs stop

The boundary between the verified world and effects. Every `@` says: formal reasoning cannot reach here.

### Print (IO)
```runa
@ print("hello")
@ print("value: " + show(x))
```

### Import (multi-file)
```runa
@ import ./utils                    -- flat import: merge all definitions
@ import Utils from ./utils         -- qualified: access via Utils.function()
@ import #a1b2c3 from ./utils       -- content-addressed import
```

### Use (Rust items)
```runa
@ use std::collections::HashMap
@ use std::io::*
```

Use `@ import` for Futuruna modules.

### Depend (Cargo dependencies)
```runa
@ depend "serde" "1"
@ depend "tokio" "1"
```

### Export (visibility)
```runa
@ export
> public_function() -> Int { 42 }
```

Marks the next definition as public. Without `@ export`, definitions are private.

### Comptime (compile-time evaluation)
```runa
@ comptime = table = generate_lookup(1000)
```

The expression is evaluated at compile time and inlined as a constant.

### Rust escape hatch
```runa
@ rust {
    fn fast_sort(x: &mut [f64]) {
        x.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    }
}
```

Inline raw Rust code. Handles nested braces, strings, and comments correctly.

---

## `?` -- Prove it

Interrogates what other runes declared. Checks invariants defined with `|`.

### How it works

1. Define an invariant with `|`:
   ```runa
   | balance_ok: balance -> balance >= 0 && balance <= max_supply
   ```
2. Check it with `?`:
   ```runa
   ? balance_ok
   ```

### The six forms

Without `else`, failure **halts** the program. With `else`, failure is **handled** and execution continues.

```runa
-- Form 1: Bare check (halt on failure)
? balance_ok

-- Form 2: Pass block (halt on failure)
? balance_ok -> {
    @ print("Balance verified")
}

-- Form 3: Capture + pass (halt on failure)
? balance_ok: val -> {
    @ print("Balance is " + show(val))
}

-- Form 4: Else block (no halt)
? balance_ok else {
    @ print("Balance violated!")
}

-- Form 5: Pass + else (no halt)
? balance_ok -> {
    @ print("OK")
} else {
    @ print("FAIL")
}

-- Form 6: Full form — capture + pass + else (no halt)
? balance_ok: val -> {
    @ print("Verified: " + show(val))
} else {
    @ print("Violation: " + show(val))
}
```

The `: val` capture binds the **subject value** (the data being checked), not the boolean result.

### Verify all invariants
```runa
? all                                          -- check all, halt on any failure
? all -> { @ print("All OK") }                -- with pass block
? all -> { @ print("OK") } else { @ print("Some failed") }   -- with both
```

### Three assurance levels

The same `?` line works at three levels of assurance:
- **`runa run`** — evaluates the predicate with current values at runtime
- **`runa build`** — emits `debug_assert!()` in the compiled binary
- **`runa verify`** — translates to SMT-LIB2 and invokes Z3 to prove for all inputs
