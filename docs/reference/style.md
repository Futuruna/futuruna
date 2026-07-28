---
feature_stage: preview
feature_stage_surfaces:
  - style-and-modeling-guidance
  - exploratory-audit-tooling
---

# Futuruna Style Guide

## The rune decides

Every line starts with a rune. The rune is not decoration — it classifies what the statement *is*. Choosing the wrong rune is a category error, not a style preference.

```
#  what exists       — the world has this shape
>  what happens      — given input, produce output
|  what must be true — this holds, unconditionally
=  what is           — this name means this value
~  what flows        — data moves through time
@  where proofs stop — side effects, imports, the boundary
?  prove it          — verify or halt
```

## `|` is for truth. `>` is for computation.

If the answer is always the same regardless of how you ask, it's a `|` fact.
If the answer depends on input, it's a `>` function.

```runa
-- YES: a fixed truth
| folkekirke() -> EvangeliskLuthersk

-- NO: wrapping a fixed truth in a function
> folkekirke() -> Trossamfund {
    EvangeliskLuthersk
}
```

```runa
-- YES: outcome depends on input
> er_myndig(monark: Monark) -> Bool {
    if monark.alder >= 18 { True } else { False }
}

-- NO: declaring a computed result as a fact
| er_myndig() -> True
```

The test: if you could replace the function body with a single constructor and nothing would change, it should be `|`.

## Model what the source says, not what you think about it

A model of a law, a spec, or a protocol should contain what the source *establishes*. Not your interpretation, not your commentary.

```runa
-- The source text goes in a ---- block
----
§ 6. Kongen skal høre til den evangelisk-lutherske kirke.
----

-- The model captures what § 6 establishes
| kongens_trossamfund() -> EvangeliskLuthersk

-- What § 6 does NOT say is also worth modeling
| troskrav_gælder_ikke_tronfølger()
```

What the source delegates, you delegate. What the source is silent on, you are silent on.

```runa
-- § 9 says "fastsættes ved lov" — the rule IS the delegation
| regentskab_fastsættes_ved_lov()

-- NOT this — don't invent a function for something the source leaves to others
> regentskab_regler() -> String {
    "Fastsættes ved lov"
}
```

## Don't invent constructors the source doesn't have

If the text says "as heir", don't add "as king" because it seems logical. The model tracks the source, not your extensions.

```runa
-- § 8 mentions oath given as tronfølger only
# ForsikringsStatus = IkkeAfgivet | AfgivetSomTronfølger

-- NOT this — AfgivetSomKonge has no basis in the text
# ForsikringsStatus = IkkeAfgivet | AfgivetSomTronfølger | AfgivetSomKonge
```

## Absence is a fact

When a rule applies to X but explicitly does NOT apply to Y, the non-application is worth stating. Silence in the source is information.

```runa
-- § 7 says "det samme gælder tronfølgeren" — explicit extension
-- § 6 has no such extension — that absence is constitutional

| troskrav_gælder_ikke_tronfølger()
```

## `?` proves `|`. `runa audit` finds what you missed.

Write `|` invariants with captured values and boolean predicates. The `?` rune checks them.

```runa
= par5_uden = regent_i_andre_lande(IkkeGivet)
| par5_default_nej: par5_uden -> par5_uden == False

? par5_default_nej
```

`runa audit` analyzes the topology of all `|` rules — it finds asymmetries, gaps, tensions, and paradoxes you didn't think to check. The `?` rune proves what you expect. The audit discovers what you don't.

## `----` blocks are for source text, `--` is for code comments

```runa
----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

-- Code that models § 3
| lovgivende_magt() -> IForening(Kongen, Folketinget)
```

The `----` block quotes the source. The `--` comment explains the code. Don't mix them.

## Types are the domain vocabulary

Define types before using them. Name constructors after domain concepts, not implementation.

```runa
-- YES: the domain speaks
# Magtholder = Hos(institution: Institution) | IForening(a: Institution, b: Institution)

-- NO: the programmer speaks
# PowerHolder = Single(inst: Institution) | Joint(inst1: Institution, inst2: Institution)
```

When using `@ sprog da`, write identifiers in Danish — including æ, ø, å. The Futuruna lexer uses Unicode-aware `is_alphabetic()`, so Danish characters work natively in identifiers, type names, and constructors. Rust codegen preserves them (Rust supports non-ASCII identifiers since 1.53).

```runa
-- YES: real Danish
# Tronfølger(alder: Heltal, trossamfund: Trossamfund)
> ændre_rigets_område(samtykke: Samtykke) -> Boolsk { ... }
| årpenge_fastsættes_ved_lov()

-- NO: ASCII approximations
# Tronfoelger(alder: Heltal, trossamfund: Trossamfund)
> aendre_rigets_omraade(samtykke: Samtykke) -> Boolsk { ... }
| aarpenge_fastsaettes_ved_lov()
```

## One pattern per concern

If two paragraphs follow the same pattern, make that visible.

```runa
-- § 5 and § 11 both use the same samtykke pattern
> regent_i_andre_lande(samtykke: Samtykke) -> Bool { ... }
> årpenge_i_udlandet(samtykke: Samtykke) -> Bool { ... }

-- Prove the symmetry in the audit
| samtykke_symmetri: par5_uden -> par5_uden == par11_uden
```

Structural agreement between rules is a finding. `runa audit` detects it automatically — but naming the pattern makes it explicit.

## Don't narrate, don't over-print

The model is the artifact. Verification output should be minimal.

```runa
-- YES: the proof speaks for itself
? all -> {
    @ print("All invariants hold.")
} else {
    @ print("Some invariants failed.")
}

-- NO: printing a running commentary
@ print("--- § 5: Regent i andre lande ---")
@ print("Uden samtykke: " + show(regent_i_andre_lande(IkkeGivet)))
@ print("Med samtykke: " + show(regent_i_andre_lande(Givet)))
@ print("")
@ print("--- § 6: Troskrav ---")
...
```

If you need to see values, write `?` proofs with pass/fail blocks, not print statements.

## Structure for multi-file models

- **Source files** (kapitel-01.runa, kapitel-02.runa): types, `|` facts, `>` functions. No `@ print`. No verification. The law and nothing else.
- **Audit file** (grundlov.audit.runa): collects all rules, defines `|` invariants, runs `?` proofs, fed to `runa audit`.
- **Source text in `----` blocks**: the actual text being modeled, verbatim. The code that follows must be traceable to the text above it.
