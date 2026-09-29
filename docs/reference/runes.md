---
feature_stage: mixed
feature_stage_surfaces:
  - core-language-syntax
  - typed-calculation-contracts
  - rust-escape-hatches
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
# Chain(a) = End | Link(head: a, tail: Chain(a))
```

`Option(a) = None | Some(a)` and `Result(a, e) = Ok(a) | Err(e)` are part of
the prelude. Their constructors (`None`, `Some`, `Ok`, `Err`), `Pair`, and the
built-in list constructors `Nil` and `Cons` belong to those types: another type
may not declare a variant with one of these names. A variant may also not reuse
a built-in type name such as `Int` or `String`; Futuruna has no type aliases.

A field of a multi-variant type can be read directly only when every variant
declares it with the same type. Otherwise, match on the variant first. Inside
an arm whose pattern names the variant, the matched variable is refined to that
variant and its fields are readable:

```runa
# Income = Salary(amount: Int) | Pension(amount: Int, supplement: Int)
> total(i: Income) -> Int {
    match i {
        | Salary -> i.amount
        | Pension -> i.amount + i.supplement
    }
}
```

Here `i.amount` is also readable outside a match, because both variants declare
`amount: Int`; `i.supplement` is not.

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

A method belongs to its type, not to the global namespace. The first parameter
(without type annotation) receives the value the method is called on, so two
types may each declare a method with the same name.

Methods declared in a type body and in `# impl Trait for Type` blocks are
called the same way:

- `x.m(args)` calls the method `m` of the type of `x`.
- A free call `m(x, args)` of a method name dispatches on the type of its first
  argument, exactly like `x.m(args)`.
- A declared global function always wins for free calls. With
  `> fee(x: Int) -> Int` declared, `fee(10)` calls that function even when a
  type has a `fee` method; `Case(1).fee()` still calls the method of `Case`.
- A free call whose first argument's type has no such method is a runtime
  error naming the method and the type.

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
names do not leak globally. Scoped dispatch matches both name and arity, so a
same-named member with a different parameter count does not hide an ordinary
global rule. Pure zero-argument rule members are memoized for the duration of
one root member evaluation; repeated dependencies therefore retain value
semantics without repeatedly evaluating the same rule cascade.

The same product body may contain ordinary `>` methods. Methods share the
product instance and can call rule members with `tax_due()` or `self.tax_due()`.
Fields are also available in product methods, so `person.gross_income` works in
both `|` rule members and `>` methods. A `|` rule member and `>` method cannot
use the same member name.

A rule member or method is called on any expression whose value is the
product: a variable, a nested field (`x.p.rate()`), a call result
(`head(members).rate()`), or a lambda or loop parameter
(`map(members, |p| p.rate())`). When the receiver's type is known, the checker
resolves the member before execution; otherwise the call is resolved from the
runtime value.

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
        "Hello, " + self.display()    -- default implementation
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

An impl names a declared trait and a declared (or built-in) type. It provides
every trait method that has no default body and no other methods. Each method
restates its trait signature: the same number of parameters, `self` in the
same position, and the parameter and result types the trait declares (`Self`
stands for the implementing type). Any difference is a type error at the impl.

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

An ordinary function name has one declaration in each authored lexical scope,
regardless of parameter count. Duplicate declarations are errors before
execution. Separate modules and nested scopes may use the same name, and a
local declaration may override a name supplied by the prelude or a prefix
import. Use `|` rules for a family of clauses and cases.

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
> map_list(xs: Chain(a), f: a -> b) -> Chain(b) {
    match xs {
        | End -> End
        | Link(h, t) -> Link(f(h), map_list(t, f))
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

Actors have a state parameter and message handlers. Each handler returns the new state. An actor handles one message at a time; see [Actors](streams.md#actors) for sending, `ask`, and sharing handles.

### Module
```runa
> module Math {
    > square(x: Int) -> Int { x * x }
    > cube(x: Int) -> Int { x * x * x }
}
```

Modules can be nested. Contents are accessed via `Math.square(5)`.

A module instance owns its initialized bindings. Initializers run once, including
private bindings and bindings that are never read. Reading a binding or calling
a module function reuses that instance. A module declared inside a function is
instantiated each time that declaration executes and captures the surrounding
values it uses.

Captures retain their types, including type parameters of the enclosing
function and compound values such as `List(a)` or `Result(a, b)`. Each call
owns a separate instance. A module function returned as a value keeps that
instance alive:

```runa
> remember(value: a) -> Int -> a {
    > module Saved {
        = item = value
        > read(ignored: Int) -> a { item }
    }
    Saved.read
}
= reader = remember("retained")
@ print(reader(0))
```

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

Rules are evaluated by priority tier: exceptions, guarded defaults, ordinary
clauses, then an unguarded default. Within one tier, source order is
authoritative and the first applicable rule wins. Put the more specific of two
overlapping guards first. `under` adds a guard condition. `exception <label>`
places the rule in the exception tier for the same head. The label (here
`heatwave`) names the exception for readability and debugging; it does not
affect priority. Write both the label and the named call:
`| exception reduced rate(x) -> 10 under x < 10`. For a rule with no arguments,
use `rate()`. Missing labels, parenthesized heads, and heads that are not named
rule calls are parsing errors. An exception overrides an existing rule: the
same scope (or its plain imports) must declare an ordinary clause or default
with the same name and arity. An exception whose head names no such rule, for
example a misspelled rule name, is a type error at that head.

An `under` guard must return `Bool`. A known non-Boolean guard is rejected at
its declaration, even if the rule is never called. A dynamically supplied
non-Boolean guard fails evaluation; it is not treated as `False` and cannot
select a fallback rule. Clauses in one family (same scope, name, and arity)
must have compatible result types. Known conflicting results are diagnosed
before execution. Unresolved generic results do not establish compatibility
or totality.

A rule with safely known Boolean results returns `False` when no clause
applies, including `| eligible(age) -> True under age >= 18` without a
parameter annotation. Invalid guards still stop evaluation. This runtime
fallback does not establish parameter types or authorize a proof: verification
and exploration retain their separate checking requirements.

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

Scopes own the live subscriptions and derived streams created inside them.
They keep receiving values after the block's statements run, until
`@ teardown("WeatherStation")`. Named scopes are also the explicit owner
required for live subscriptions started inside ordinary functions. See
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

### Top-level initialization order

Unique top-level bindings may refer to bindings declared later, directly or
through functions, rules, and RuleScope members. Futuruna initializes the
required dependencies first in both interpreted and compiled execution.
Declarative `@ comptime` and `@ export` annotations remain attached when a
binding moves with its dependencies.

```runa
| answer() -> base + 1

= result = answer()
= base = 41
```

Rebound names keep source-order semantics. A cycle between unique top-level
bindings is rejected with the complete initialization path.

### Monadic bind (early return)
```runa
= value <- parse_int("42")
```

If the expression returns `Ok(v)` or `Some(v)`, binds `v` and continues. If `Err(e)` or `None`, returns immediately (early return). Equivalent to Rust's `?` operator.

The expression must be a `Result` or an `Option`, or a call to an effect
operation, which resumes with a plain value. `<-` on any other value is a type
error; bind a plain value with `= name = expression`.

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

The `|` arms handle three stream events: values, errors, and completion. Arms on the lines after `~` are indented past it. Use `for` for lists and ranges; use `~ + |` for streams.

See [streams.md](streams.md) for the full stream API and subscription reference.
For lifetime ownership rules around function-local subscriptions, see
[docs/stream-lifetimes.md](../stream-lifetimes.md).

### Subject creation (push-based streams)
```runa
~ clicks = subject()              -- keeps every value
~ temp = subject(20.0)            -- starts with 20.0, keeps every value
~ history = subject(0, 10)        -- starts with 0, keeps the last 10 values
```

### Push values into subjects
```runa
clicks <- "click1"
clicks <- "click2"
temp <- 25.0
```

`<-` returns after every subscription and derived stream has handled the value.

### Subject properties
```runa
clicks.count       -- number of values the subject has emitted
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

Effect invocations use supported builtin names. Unknown names such as
`@ println(...)` or `@ log(...)` are errors. Assertions are ordinary calls:
write `assert(condition)` or `assert_with_message(condition, "message")`
without `@`. Declared algebraic operations also use ordinary call syntax, as
shown in the handler example above.

### Import (multi-file)
```runa
@ import ./utils                    -- flat import: merge all definitions
@ import Utils from ./utils         -- qualified: access via Utils.function()
@ import #a1b2c3 from ./utils       -- content-addressed import
```

Top-level plain imports form one merged declaration scope. They are resolved
in import order before the importing file's declarations, regardless of where
the directives appear. Imported functions override injected prelude defaults;
local functions override imported functions. Imported initializers and later
calls use that same final function and rule context. Unique bindings across the
merged scope follow the dependency order described above. A canonical source
is imported once per namespace, including when several dependencies import it.
Standalone executable statements in an imported file are not run; its binding
initializers are part of the merged program.

Checking an importing file also checks imported function bodies. Errors identify
the imported file and source position; editors link the import-site diagnostic
to that original location. Qualified modules resolve private helpers and their
own dependencies within the module, without inheriting the caller's local names.
Qualified bindings and their helper calls use the module's declaration scope;
a same-named value in the importer does not replace a module binding.
Separate qualified aliases own separate instances. Repeating the same alias
and source within one parent namespace reuses its existing instance. Bindings
inside an instance follow the dependency order described above.

### Use (Rust items)
```runa
@ use std::collections::HashMap
@ use std::io::*
```

Use `@ import` for Futuruna modules.

### Depend (Cargo dependencies)
```runa
@ depend "serde" "1" ["derive"]
@ depend "tokio" "1"
```

A dependency is a crates.io package: a name, a version such as `"1"` or
`"0.10.2"`, and optionally a list of features. Path, git and inline-table
sources are rejected. `runa build` and `runa run` let Cargo download and build
these crates; `runa check` does so only with `--build-deps`.

### Export (visibility)
```runa
@ export
> public_function() -> Int { 42 }
```

Marks the next definition as public. Without `@ export`, definitions are private.

### Calculate (typed external input)
```runa
@ calculate("Danish personal income tax")
| calculate_tax(input: TaxInput) -> TaxResult(annual_tax = annual_tax(input))
```

Marks one typed rule or function as a discoverable calculation boundary for
`runa schema`, `runa template`, and `runa call`. This annotation does not perform
an effect or change rule semantics. Its optional single string labels the whole
calculation; nested input labels and questions remain field metadata. See
[calculations.md](calculations.md).

### Comptime (compile-time evaluation)
```runa
> generate_lookup(n: Int) -> Int { n * 2 }

@ comptime
= table = generate_lookup(1000)
```

Put `@ comptime` on its own line before the binding. The expression is evaluated
at compile time and inlined as a constant.

The compiler can also fold ordinary pure expressions automatically. This
speculative evaluation is bounded and cannot perform host effects, including
file access, output, randomness, or effects reached through callbacks and
module initializers. A value that requires runtime initialization stays at
runtime; unavailable values are never substituted with placeholders.

Explicit `@ comptime` evaluation is pure as well: an expression that reaches
a host effect (output, input, files, the environment, clocks, network or
processes) is a compile-time error at that call. Its dependencies must have
compile-time values too; the compiler reports an error when a required value
is available only at runtime.

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
- **`runa verify`** — translates supported claims to SMT-LIB2 and invokes Z3
  to prove them over their runtime input domain

For `Int`, verification includes checked `i64` arithmetic: a reachable overflow
or zero divisor is a counterexample to the guarantee. Division truncates toward
zero, and remainder has the dividend's sign. Short-circuited operations and
unselected branches do not have to be defined. Unsupported helpers and Float
claims remain explicitly unverified; exact real arithmetic cannot substitute
for floating-point evaluation.

`runa verify` is Preview. PROVED means that for every value of the invariant's
free variables (Int ranges over the 64-bit values) the predicate is true and no
Int operation it evaluates overflows or divides by zero. Claims involving Float
are reported as unsupported, never proved.

### Verifying rule dispatch

For CI, `runa verify` exits 0 only when at least one invariant exists and every
invariant is PROVED. A counterexample, unsupported claim, unknown result,
missing or failed solver, or empty invariant set exits 1.

`runa verify` can translate pure, total, non-recursive `|` rule groups directly,
including rules inside a product RuleScope. Conditions and exceptions use the
same precedence as execution; there is no need to restate the rule cascade as a
separate `>` function.

```runa
# TaxCase(income: Int) {
    | rate_percent() -> 25
    | rate_percent() -> 30 under income > 500000
    | exception low_income rate_percent() -> 20 under income < 100000
    | tax_due() -> income * rate_percent() / 100
}

= high_income_case = TaxCase(income = 600000)
| high_income_tax: high_income_case.tax_due() -> high_income_case.tax_due() == 180000
```

Verification uses the same merged declaration scope and binding dependencies
as execution, including when a plain import appears after a local declaration.

Plain imports are resolved recursively for verification. An exception declared
by an importing file therefore extends the imported rule group and keeps its
normal exception priority. Within one priority tier, imported declarations
come before declarations in the importing file and the first applicable rule
wins, just as it does during execution.

The solver path fails closed with a diagnostic for partial non-Boolean rules,
recursive dispatch, higher-order parameters, effects, and other expressions
outside its current first-order subset. Rule return sorts are resolved by the
exact RuleScope (when present), name, and arity; same-named overloads never
share a sort, and conflicting or unresolved result sorts fail closed. `runa
verify` remains a Preview surface.
