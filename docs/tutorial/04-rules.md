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
@ print(show(descendants))  -- ["bob", "charlie", "diana"]
```

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
