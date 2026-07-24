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

## Next

[5. Streams and Reactivity](05-streams.md)
