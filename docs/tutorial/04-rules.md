---
feature_stage: preview
feature_stage_surfaces:
  - solver-assisted-verification
---

# 4. Rules and Verification

## Facts (Datalog)

The `|` rune declares things that must be true:

```runa
| parent("alice", "bob")
| parent("bob", "charlie")
| parent("alice", "diana")

-- Rules with backtracking
| ancestor(a, b) -> parent(a, b)
| ancestor(a, b) -> parent(a, mid), ancestor(mid, b)

-- Query
= descendants = findall(c, ancestor("alice", c))
@ print(show(descendants)) -- [bob, diana, charlie]
```

The first argument to `findall` or `search` must be a single variable; tuple,
list, constant, and computed templates are rejected. Interpreted `findall`
returns distinct values in first-discovery order. Values
with different types remain distinct even if `show` prints them alike. Reaching
the recursive search depth limit reports incomplete evaluation as an error;
a partial answer list must not be treated as complete.

Query goals run from left to right. A goal passes each distinct complete set of
named bindings to the remaining goals before searching for another set. It does
not re-run a derived rule separately for each output variable. An existence
test stops at its first successful witness; only enumeration needs every
answer. If no witness has been found and a search exceeds its depth limit,
negation also reports incomplete evaluation rather than claiming absence.

A parameter annotation supplies a type, not a collection of possible answers.
For `Int`, `String`, and other non-enum types, bind the result through facts or
earlier goals. If a successful clause leaves the result unbound, enumeration
reports incomplete evaluation rather than an empty answer list. Nullary enum
types supply a finite domain: unbound arguments are searched in declaration
order, with repeated names constrained to the same value.

An unannotated, variable-only fact does not choose a concrete argument type.
Repeated names require the same value, including its type:

```runa
| same(value, value)
assert(same(1, 1))
assert(same("law", "law"))
assert(not(same(1, "1")))
assert(findall(value, same(7, value)) == [7])
```

Native builds support these polymorphic facts over scalar values. A bound
argument can supply a query result's type through a repeated head variable;
an entirely unbound fact does not provide a finite collection of answers.
Generic matching of structured values and aliases reused at different concrete
types still have native lowering limits.

Native builds support recursive Boolean clause queries over scalar values,
including negation, correlated bindings, conjunctions, and alternatives. Some
derived queries involving structured values, captured values, exception priority, or scoped
rules still require interpreted execution; unsupported native searches must
produce a diagnostic rather than an empty answer list.

Use lowercase names such as `x`, `y`, and `z` for logic variables. Uppercase
names denote declared constructors; undeclared names such as Prolog-style
`X` and `Y` are rejected before evaluation. Declared constructors can still
appear as values or patterns in rules.

## Default logic (Catala-style)

```runa
# Weather(city: String, temp: Float)

-- Default rule
| advisory(w) -> "all clear"
-- Conditional override
| advisory(w) -> "heat warning" under w.temp > 35.0
-- Exception (highest priority)
| exception heatwave advisory(w) -> "DANGER" under w.temp > 45.0
```

Exception beats conditional, conditional beats default. Legal/regulatory logic made explicit.

## Invariants and verification

```runa
= balance = 500

-- Define an invariant
| balance_ok: balance -> balance >= 0 && balance <= 1000000

-- Check it at runtime
? balance_ok -> { @ print("Balance OK") } else { @ print("VIOLATION") }
```

Three assurance levels from the same `?` line:
- `runa run` — evaluates at runtime
- `runa build` — emits `debug_assert!()` in compiled binary
- `runa verify` — translates to SMT-LIB2, proves with Z3

The verifier understands pure, total, non-recursive rule cascades directly. A
RuleScope does not need a duplicate helper function for Z3:

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

```bash
runa verify tax.runa
```

The generated solver model preserves exception priority and first-applicable
source order inside each priority tier. Recursive, partial non-Boolean,
higher-order, or effectful rule groups are rejected from this Preview solver
path with a diagnostic instead of being approximated.

## Next

[5. Streams and Reactivity](05-streams.md)
