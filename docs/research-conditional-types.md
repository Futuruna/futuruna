# Conditional Type Evolution — Design Note

**Insight:** Laws are never edited. They are amended by new instruments that
modify the effect of existing types and rules. The original text stays frozen.

## The Problem

```runa
-- grundlov.runa (1953, frozen)
# Rigsdel = Danmark | Færøerne | Grønland
| grundloven_gælder_for(r: Rigsdel) -> true
```

In 2030, a treaty adds Iceland to the realm. You don't edit grundlov.runa.
You write a new file:

```runa
-- traktat-2030.runa
@ import ./grundlov

-- The triggering legal event
| traktat_underskrevet("Island-traktaten", "2030-06-01")

-- Type evolution: Rigsdel now includes Iceland
# Rigsdel WHEN traktat_underskrevet("Island-traktaten", _)
    = Rigsdel | Iceland
```

After this amendment:
- `grundloven_gælder_for(Iceland)` → true (automatically!)
- `findall(r, grundloven_gælder_for(r))` → [Danmark, Færøerne, Grønland, Iceland]
- The original grundlov.runa is untouched

## What This Means

1. **Types are living documents.** They evolve through legal events.
2. **Amendments are additive.** New files extend, never modify.
3. **Provenance is traceable.** Each type extension references the legal basis.
4. **Rollback is possible.** Retract the triggering fact → type reverts.
5. **Temporal queries.** "What was Rigsdel before the treaty?" is answerable.

## How It Differs From Catala

Catala has rule-level exceptions: "this rule applies EXCEPT when..."
Futuruna would have TYPE-level evolution: "this type INCLUDES X WHEN..."

Catala can override a rule's output. Futuruna can change a type's membership.
Since rules are typed (`r: Rigsdel`), changing the type automatically changes
which rules apply to which values. The rules themselves never change.

## Open Questions

- Syntax: `# Type WHEN condition = ...` vs `# Type += Variant WHEN ...`
- Scoping: does the evolution apply globally or only in the importing file?
- Conflict: what if two amendments contradict each other?
- Retraction: if the triggering fact is retracted, does the type shrink?
- Verification: can `runa audit` detect that an amendment creates contradictions?
