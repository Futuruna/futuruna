# The Danish Constitution in Futuruna

**Danmarks Riges Grundlov encoded as a Futuruna program**

## Why This Works

Every previous attempt to formalize constitutions has failed in the same way:
the target language forces law into a shape it doesn't have. Prolog can encode
rules but not types. Haskell can encode types but not defaults. Catala can
encode defaults but not general computation. SQL can encode data but not logic.

Futuruna is the first language where law doesn't require translation — it IS the
natural notation:

| Legal concept | Futuruna feature | Example |
|---|---|---|
| Legal entity (Monarch, Parliament, Court) | `# Type` ADT | `# Statsmagt = Lovgivende \| Udovende \| Dommende` |
| General rule | `\| head -> value` | `\| regeringsform() -> IndskraenketMonarki` |
| Conditional rule | `\| rule under condition` | `\| statskirke(kirke) under kirke == EvangeliskLuthersk` |
| Exception to a rule | `\| exception label rule` | `\| exception tronfølgelov succession(...)` |
| Chapter/section scope | `\| scope Name { }` | `\| scope Kapitel_I { }` |
| Legal reference | Function call | `troelfolgeloven_1953()` |
| Annotation/citation | `@ comment` | `@ ref("Grundloven § 2")` |
| Legal definition | `> fn` | `> magtdeling() -> Magtfordeling { }` |

## The Key Insight

A constitution is a **default logic system with typed entities**:

1. It defines **types** — what kinds of things exist (powers, institutions, rights)
2. It states **defaults** — what is true unless overridden ("the government is a constitutional monarchy")
3. It creates **exceptions** — specific rules that override general ones
4. It **scopes** these rules into chapters

This is exactly what Futuruna's `|` rune pathway does. The constitution doesn't need
to be *translated* into Futuruna — it needs to be *transcribed*. The structure is
already there in the legal text. Futuruna just makes it explicit.

## What This Proves

If the Danish Constitution can be written in Futuruna:

1. **Futuruna is the first general-purpose legal programming language** — not a DSL like
   Catala, but a full PL that happens to be natural for law
2. **Default logic + types = legal reasoning** — the combination that no other
   language provides in a single coherent syntax
3. **Legal verification becomes possible** — the type checker can find gaps
   (uncovered cases), contradictions (overlapping exceptions), and ambiguities
   (underspecified defaults)
4. **The constitution becomes executable** — you can query it: "who holds legislative
   power?", "what is the state church?", "can a woman inherit the throne?"

## Structure

Each chapter of the Grundlov becomes a `.runa` file:

```
grundlov.runa              -- Preamble + top-level types
kapitel-01.runa            -- Kap. I: Statsform (§1-§4)
kapitel-02.runa            -- Kap. II: Kongen (§5-§11) [planned]
...
kapitel-11.runa            -- Kap. XI: Ikrafttræden (§89) [planned]
```

## Language Features Needed

### Already in Futuruna
- [x] Default logic (`| rule under condition`)
- [x] Exceptions (`| exception label rule`)
- [x] Scoped rules (`| scope Name { }`)
- [x] ADTs with named fields (`# Type(field: Type)`)
- [x] Pattern matching (`match x { | A -> ... }`)
- [x] Methods on types
- [x] Annotations (`@ ref(...)`)

### May Want to Add
- [ ] **Date literals** — `1953-03-27` instead of `Date(1953, 3, 27)`
- [ ] **String enums / symbols** — `:denmark` instead of `"Danmark"`
- [ ] **Cross-file scoping** — `@ import Kapitel_02 from ./kapitel_02`
- [ ] **Legal assertion syntax** — `| assert complete(succession_rules)` for gap checking
- [ ] **Paragraph numbering as metadata** — `@ § 2` on rule groups

## Status

- [x] Chapter I (§1-§4) — encoded
- [ ] Chapter II (§5-§11) — planned
- [ ] Remaining chapters
- [ ] Verification queries
- [ ] Completeness analysis
