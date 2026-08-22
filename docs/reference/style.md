---
feature_stage: preview
feature_stage_surfaces:
  - style-and-modeling-guidance
  - source-meta-comment-tooling
  - typed-program-references
  - exploratory-audit-tooling
---

# Futuruna Style Guide

## The rune decides

Every statement starts with a rune. Continuation lines belong to that statement
and are indented by the formatter. The rune is not decoration — it classifies
what the statement *is*. Choosing the wrong rune is a category error, not a
style preference.

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

## Distinguish exclusion from silence

When a source explicitly excludes Y from a rule that applies to X, the
non-application is worth stating. Mere silence only proves that the source does
not state the rule for Y; it does not by itself establish the opposite rule or
a positive right.

Do not turn a clause that names the king into
`troskrav_gælder_ikke_tronfølger()`. That would convert silence about the heir
into an explicit exclusion. If a source really says that a requirement applies
only to the reigning king, the narrower fact may be modeled because the
exclusion then comes from the source itself.

## `?` proves `|`. `runa audit` finds what you missed.

Write `|` invariants with captured values and boolean predicates. The `?` rune checks them.

```runa
= par5_uden = regent_i_andre_lande(IkkeGivet)
| par5_default_nej: par5_uden -> par5_uden == False

? par5_default_nej
```

`runa audit` analyzes the topology of all `|` rules — it finds structural
asymmetries, proof gaps, resolution tensions, and same-rule contradictions you
didn't think to check. Rule names never imply a relationship. The `?` rune
proves what you expect; the audit discovers structural cases to review.

## `----` blocks are for source text, `--` is for code comments

```runa
----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

-- Code that models § 3
| lovgivende_magt() -> IForening(Kongen, Folketinget)
```

The `----` block quotes the source. The `--` comment explains the code. Don't mix them.

## Meta comments are typed anchors

Meta comments attach one ordinary typed Futuruna value to a label without
changing evaluation semantics. The comment is only an attachment point; roles,
sources, warnings, fields, and other structure belong in the referenced value.
The metadata index resolves that value through Futuruna's type information, so
tools can query metadata by role or by domain type.

```runa
# SourceInfo(url: Tekst, identifier: Tekst)
# LegalMetaRole(a) = Source(value: a)
# impl MetaRole for LegalMetaRole {}
# impl Meta for LegalMetaRole {}

= grundlov_par3_source = SourceInfo(
    url = "https://www.retsinformation.dk/eli/lta/1953/169",
    identifier = "§ 3"
)
= grundlov_par3_meta = Source(value = grundlov_par3_source)

--@label:grundlov_par3::meta:grundlov_par3_meta--
----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

--@begin:grundlov_par3--
| lovgivende_magt() -> IForening(Kongen, Folketinget)
--@end:grundlov_par3--
```

The canonical form is `--@label:LABEL::meta:BINDING--`. The binding's inferred
root type must explicitly implement the standard marker trait `Meta`. Direct
`::ROLE:BINDING` pairs and the older `--@meta::...` spelling remain accepted
for source compatibility, but they are not the style for new code. A shipped
example must use one `meta` reference so the comment grammar cannot grow into a
second schema language.

When one anchor needs many related metadata values, attach one typed aggregate
instead of adding references to the comment. Every constructor nested
inside a pure ground aggregate retains its Futuruna type and a stable value path.
Tools can therefore select the values they understand while the anchor stays
short and domain-neutral.

```runa
# CalculationMeta(fields: List(CalculationField))
# impl Meta for CalculationMeta {}

= amount_meta = CalculationMeta(fields = [
    amount_field,
    currency_field,
    period_field
])

--@label:calculate_amount::meta:amount_meta--
```

`runa meta --type CalculationField` finds `amount_meta` because it contains
typed `CalculationField` descendants. JSON output reports those descendants as
`typed_values` with paths such as `$.fields[0]`. This behavior is generic: the
aggregate and nested types are ordinary user-defined Futuruna types. Only the
two empty marker traits and the label attachment edge are standardized.

When metadata names a field in another declared schema, use a structural path
instead of repeating that field path as an opaque string:

```runa
# Household(children: List(Child), income: Income)
# Child(age: Int)
# Income = Wage(amount: Int) | Business(profit: Int)

= child_age_path = pathof(Household::children::age)
= wage_path = pathof(Household::income::Wage::amount)
= income_kind_path = pathof(Household::income::$variant)
```

`pathof` is a generic compile-time `String` value. The root type is checked but
omitted from the result, so these values lower to `"children.age"`,
`"income.Wage.amount"`, and `"income.$variant"`. Collection elements are
traversed implicitly; sum alternatives are explicit. Plainly imported types are
resolved through the same declaration graph. This gives metadata consumers a
stable serialized string without sacrificing source navigation or rename-time
checking. Existing string values remain valid.

Use `refof(NestedType::field)` instead when one metadata declaration should
follow a nested domain type through different calculation inputs. Calculation
contracts retain the reference's root type and project its checked member path
onto every reachable occurrence. Attach that metadata to the nested type label.
An exact root-input reference or absolute `pathof(...)` declaration overrides a
projected declaration; equal-specificity collisions fail closed.

When metadata points to a Futuruna declaration or member rather than an external
data field, use `refof` and declare the metadata field as `ProgramReference`:

```runa
# SourceInfo(url: String)
# Evidence(source: SourceInfo, target: ProgramReference)
# impl Meta for Evidence {}

= source_info = SourceInfo(url = "https://example.invalid/law")
= evidence = Evidence(
    source = source_info,
    target = refof(calculate_tax)
)

--@label:calculate_tax::meta:evidence--
| calculate_tax(income: Int) -> income / 2
```

`refof(rule_name)`, `refof(TypeName)`, and
`refof(TypeName::field::nested_field)` produce typed `ProgramReference` values.
Rules, functions, bindings, types, structural fields, and RuleScope members are
checked against local and plain imported declarations. Misspelled or ambiguous
targets fail `runa check`, and definition navigation follows the reference.
Creating a reference does not execute its target and does not add a rule
dependency; only actual calls define the program's dependency graph.

When the aggregate also needs role-bearing values, define a sum type whose
variants each have one named `value` field and implement the standard
`MetaRole` marker. This moves the role map into ordinary Futuruna data instead
of extending the comment syntax or relying on a magic constructor name.

```runa
# Shape = Circle | Triangle | Square
# ShapeMetaRole(a) = Source(value: a) | Warning(value: a)
# AuditWarning(message: Tekst)
# ShapeMeta(a) = ShapeMeta(attachments: a)
# impl MetaRole for ShapeMetaRole {}
# impl Meta for ShapeMeta {}

= comment_shape = Circle
= alternate_shape = Triangle
= shape_warning = AuditWarning(message = "Kredsen er ikke udtrykkeligt afgrænset")
= shape_meta = ShapeMeta(
    attachments = (
        Source(value = comment_shape),
        Source(value = alternate_shape),
        Warning(value = shape_warning),
    )
)

--@label:shape_rule::meta:shape_meta--
--@begin:shape_rule--
| circles_rule(shape: Shape) -> shape == Circle
--@end:shape_rule--
```

`Meta` and `MetaRole` are empty marker traits and add no runtime behavior. Every
applied variant of a `MetaRole` type must have exactly one named `value` field.
Its constructor is exposed as lower snake case, so `DependencySource` becomes
`dependency_source`. A role type may also implement `Meta` when one role value
is itself the root attachment. Multiline tuples accept line breaks and a
trailing comma. Direct `::ROLE:BINDING` pairs and the former
`MetaAttachment(role = ..., value = ...)` shape remain compatibility input,
not canonical source. The comment stays an attachment point; metadata structure
does not belong in its grammar.

Code spans use the compact `--@begin:LABEL--` and `--@end:LABEL--` forms.
The earlier `--@begin::LABEL--` and `--@end::LABEL--` spellings remain accepted.

`runa meta --type Shape file.runa` finds references whose binding has type
`Shape` or whose typed aggregate contains a `Shape`. `runa meta --role warning
file.runa` finds warning references, and the filters can be combined. Pure
ground bindings also expose a static value, typed descendant paths, and
definition location. Nested attachments additionally expose their normalized
role, canonical value binding, attachment path, and value path. A reference may
point to a binding in the current file or in any recursively reachable plain
import; aggregate resolution follows referenced bindings transitively through
nested imports. Dynamic or unresolved bindings produce metadata diagnostics;
the parser still treats every meta marker as a comment, while `runa meta` and
metadata-aware consumers such as `runa schema` fail closed on those diagnostics.

Audit and indexing tools should use `runa meta --json file.runa`. The versioned
`futuruna.meta.v1` document contains typed references, quoted source anchors,
linked code spans and their declared types, rules, bindings, and functions, plus
metadata diagnostics. Each ground reference retains its Futuruna rendering in
`value` and exposes a structural `data` tree. Constructors include their parent
Futuruna type. Constructors, named arguments, lists, tuples, and primitive
values are separate nodes, while `typed_values` indexes every nested constructor
by type and value path. An audit can therefore inspect an `AuditWarning` field or
a `SourceInfo` URL without parsing Futuruna display text. The additive
`attachments` index makes nested `MetaRole` variants directly queryable by role
without parsing the structural tree. It also reports the participating
`meta_role_types`; legacy `MetaAttachment` values remain readable during
migration. Expression-level forms such as
`match` arms are not span symbols. `--type` and `--role` apply to JSON output as
well, so a warning sweep can use `runa meta --json --role warning file.runa`
without parsing presentation text.

Raw-text anchors report `text_begin_marker_line` and `text_end_marker_line` for
the `----` delimiters. `text_start_line` and `text_end_line` identify only the
verbatim content between them, matching the way code spans separate marker and
content lines. Empty blocks have no content range, and an unterminated attached
block is reported as a metadata diagnostic.

Pass a directory to sweep a complete source tree recursively:

```sh
runa meta --json --type Shape --role source examples/
runa meta --json --role warning examples/danish-income-tax/
```

Directory output uses the separate `futuruna.meta.collection.v1` schema. Its
`files` array contains filtered `futuruna.meta.v1` documents in stable path
order, while its top-level counts distinguish files scanned from files returned.
Corpus diagnostics repeat the source file beside every line and message, so an
audit can report invalid metadata without losing its location. A collection
with metadata diagnostics is still emitted as JSON and exits unsuccessfully.

The original source spelling remains compatible and desugars `meta` to the
`source` role:

```runa
--@source::grundlov_par3::meta:grundlov_par3--
```

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
- **Calculation file** (personskat.calculate.runa): exposes one or more typed domain-object boundaries with `@ calculate`, suitable for `runa schema`, `runa template`, and `runa call`.
- **Scenario file** (loenmodtager.scenario.runa): contains concrete fictional or sourced input facts and executable `?` checks, fed to `runa run`.
- **Audit file** (grundlov.audit.runa): collects all rules, defines `|` invariants, runs `?` proofs, fed to `runa audit`.
- **Source text in `----` blocks**: the actual text being modeled, verbatim. The code that follows must be traceable to the text above it.
