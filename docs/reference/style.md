---
feature_stage: mixed
feature_stage_surfaces:
  - style-and-modeling-guidance
  - source-meta-comment-tooling
  - typed-program-references
  - exploratory-audit-tooling
  - annotations
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

When the source is saved in the project, put the text in a section as a
checked quote instead: see [Annotations describe code](#annotations-describe-code).

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
didn't think to check. Rule names never imply a relationship. With Z3 it also
reports clauses that can never fire and same-tier guards that overlap with
different results (see [runes.md](runes.md#auditing-clause-reachability)). The
`?` rune proves what you expect; the audit discovers structural cases to review.

## `----` blocks are for source text, `--` is for code comments

```runa
----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

-- Code that models § 3
| lovgivende_magt() -> IForening(Kongen, Folketinget)
```

The `----` block quotes the source. The `--` comment explains the code. Don't mix them.
Outside a section a `----` block is a block comment that nothing checks. Inside
an `@ section` every `----` block is an `@ quote` of a saved document, and
`runa check` compares it with the file.

## Annotations describe code

Experimental. Four annotations attach facts to the code below them:

| Annotation | Stands before | Means |
|---|---|---|
| `@ section <name> {` ... `}` | top-level items (a block) | the items inside implement one passage of a source |
| `@ quote(<document>)` | a `----` block inside a section | the block is verbatim text of a saved document |
| `@ cite(<paragraphs>)` | a rule clause, an exception, a RuleScope member or a function | the clause implements these paragraphs of its section's quotes |
| `@ meta(<value>)` | a section, a data type, a binding, a rule clause, a RuleScope member or a function | attaches a typed metadata value |
| `@ meta(<value>) for <name>` | nothing: it stands alone at top level | attaches the value to the declaration `<name>`, written apart from it |

Each annotation stands on its own line directly above its item. Comments may
stand between them; a blank line may not. Several annotations on one item are
written one per line, and declaration annotations such as `@ calculate` or
`@ field` may stand between them and the item. An annotation that describes
nothing it may describe is an error that says where it may stand.

Annotations change no program: the lexer reads each one whole and emits no
tokens for it, so statements, content hashes and calculation contracts are the
same with or without them. `runa call`, `runa build` and the WebAssembly engine
run the model exactly as if they were absent.

The `--@label`, `--@begin`, `--@end`, `--@cite`, `--@source` and `--@meta`
comment directives of earlier versions are errors that name their replacement;
`runa fmt --migrate` rewrites them (see [Migrating](#migrating-from-the-comment-directives)).

### Sections and quotes

A section groups the code that implements one passage of a source, with the
passage's text:

```runa
# SourceInfo(file: String, identifier: String)
# impl Document for SourceInfo {}
# LegalMetaRole(a) = Source(value: a) | Guidance(value: a)
# impl MetaRole for LegalMetaRole {}
# impl Meta for LegalMetaRole {}

= grundloven = SourceInfo(
    file = "sources/grundloven.txt",
    identifier = "LOV nr 169 af 05/06/1953"
)

@ section grundlov_par3 {

@ quote(Source(value = grundloven))
----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

@ cite(§ 3)
| lovgivende_magt() -> IForening(Kongen, Folketinget)

}
```

- **A section opens no scope.** Every name declared inside is a top-level name
  of the file, and code outside calls it as before. `runa fmt` does not indent
  a section's body.
- **The name is written once**, after `@ section`: a lowercase identifier, with
  the `{` at the end of the line. The section closes at a `}` alone at the
  start of a line once every brace opened inside it is closed. A section stands
  only at top level; sections do not nest. A second section with the same name
  in one file is an error. A section may share its name with a rule it holds.
- **`refof(section grundlov_par3)`** is a `ProgramReference`
  (`ProgramSectionReference(name)`) to the section, checked against the
  sections of the file it stands in.

**Documents.** `@ quote` names a *document*: a binding whose type implements
the standard marker trait `Document`, or a `MetaRole` value holding one
(`Source(value = grundloven)`). `Document` adds no behaviour; the value must
have a `file = "..."` field, the path of the saved text relative to the project
root (the directory of `runa.toml`, else of the root source file). A model
declares this once per type: `# impl Document for SourceInfo {}`. A name that is
not a binding, a value whose type does not implement `Document` and a document
without `file` are errors at the quote.

**Roles.** When the argument is a role value (`Source(value = ...)`,
`Guidance(value = ...)`), that role is an attachment of the section: the
document is named once, with its role, at its quote, and is not listed again in
the section's metadata.

**Quotes.** The `----` block holds only the document's text, verbatim. A
section that quotes several documents has one `@ quote` per document, and
quotes may stand anywhere in the section, so a long passage can sit beside the
rules for each paragraph. The section's quote is its `@ quote` blocks in order.
Inside a section every `----` block needs an `@ quote` above it; outside a
section `@ quote` is an error and a `----` block is a block comment.

**The quote check.** `runa check` reads each quoted document's file and finds
every paragraph of the quote (lines separated by a blank line) in it. Both
sides are compared after turning no-break spaces into spaces, removing soft
hyphens and zero-width joiners and collapsing whitespace, so line breaks in
the model need not match the file. A missing file, or a paragraph the file
does not hold verbatim, is an error at that paragraph's first line:

```text
this quoted paragraph is not in `sources/grundloven.txt` (document `grundloven`) verbatim: § 3. Den lovgivende magt er hos kongen ...
```

The check runs on every `runa check`, for the checked file and every file it
imports, with files relative to the project root of the checked model. Saved
documents are not part of the check cache's key, so an edited source file is
always compared again. A paragraph is compared whole: a quote that shortens a
sentence with "[…]" fails.

**Text without a saved document is not a quote.** A quote must be comparable
with a file. Source text the project has not saved stays a `----` block
comment, outside the section (directly above it is the convention), and nothing
can cite it:

```runa
----
§ 6. Kongen skal høre til den evangelisk-lutherske kirke.
----

@ section grundlov_par6 {

| kongens_trossamfund() -> EvangeliskLuthersk

}
```

A section without quotes is still a section: explanations and needs report it
(as a section of checks, for example). Only a section that quotes a document
is a *law section*.

### Citations

A clause names the paragraphs of its section's quotes it implements:

```runa
@ cite(§ 9, stk. 3)
| excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
```

- Paragraphs are separated by `;`. Each is `§ N` (with a letter, as `§ 8 a`)
  followed by optional `, stk. N`, `, nr. N` and `, N. pkt.` (a sentence).
  `in <document>` names the document by its binding when two quoted documents
  hold the same paragraph number: `§ 1 in bek_19_2024`.
- `@ cite` stands above a top-level rule clause, an `exception` clause, a
  member inside a RuleScope body (at the member's indentation) or a function
  (`> f(...)`). A clause takes one `@ cite`. A citation on a rule's first
  clause covers its other clauses that cite nothing; an exception with its own
  citation is cited by it.
- `runa check` finds each paragraph in the section's quotes, read the way the
  quote marks them: a line starting `§ N.` opens a paragraph, `Stk. N.` a
  subsection, `N)` a numbered item. `§ 8 a` and `§ 8 A` are the same
  paragraph; a citation is shown as written. A paragraph the quotes do not
  hold is an error that lists what they do hold.
- **Sentences.** A subsection splits into sentences after a full stop followed
  by a space and a capital letter or an ordinal (`1. og 2. pkt. gælder ...`),
  except after the abbreviations `kr.`, `pct.`, `stk.`, `nr.`, `pkt.`, `jf.`,
  `m.v.`, `f.eks.` and `bl.a.`. An error about a `pkt.` lists the sentences
  found, so a wrong split is visible.
- **No paragraph and its part.** A bare `§ 13` is the whole paragraph, so
  `@ cite(§ 13; § 13, stk. 2)` is an error ("`§ 13` already contains
  `§ 13, stk. 2`; cite `§ 13` or its parts"), as is naming one passage twice.
  An explanation never shows the same quoted lines twice.
- A citation outside a section, or in a section that quotes no document, is an
  error.

**Citing a passage as amended.** When a section quotes an amending act as well
as the consolidated act it amends, a passage the amending act changes is cited
`as amended`:

```runa
@ quote(Source(value = personskatteloven_aendring_2024))
----
§ 4 I personskatteloven, jf. lovbekendtgørelse nr. 1284 af 14. juni 2021, som ændret senest ved § 1 i lov nr. 482 af 22. maj 2024, foretages følgende ændringer:

1. § 8 a, stk. 1, 1. pkt., affattes således:

Skat af aktieindkomst, der ikke overstiger et grundbeløb på 51.500 kr. ...
----

@ cite(§ 8 a, stk. 1, 1. pkt. as amended)
| low_rate_part_ore() -> min_int(abs(taxable_share_income_ore()), threshold_ore())
```

In a quote, a line `§ N I <act>, jf. lovbekendtgørelse nr. ... foretages
følgende ændring(er):` opens amending paragraph `§ N` (LexDania's `§ N Stk. 1.`
rendering is read too), and a line `N. <passage>, <verb>` inside it is
instruction `nr. N` for `<passage>`. The verbs are `affattes således:` (the
passage reads as the text after the instruction, up to the next one),
`ændres »x« til: »y«` (the base passage with its only `x` replaced by `y`) and
`ophæves` (repealed: citing it is an error). `runa check` requires exactly one
quoted instruction for the passage (with several, name one:
`as amended by personskatteloven_aendring_2024 § 4, nr. 1`), requires the
amending paragraph's `jf. lovbekendtgørelse nr. <n> af <date>` to match the
base document's identifier when the section quotes the base, and for `ændres`
requires `x` to occur once in the base passage. Conversely, a plain citation
of a passage an instruction in the section amends, or of a paragraph or
subsection containing it, is an error: a superseded wording is never shown as
the cited law. A provision of the amending act itself is cited plainly with
`in`: `§ 9, stk. 2 in personskatteloven_aendring_2024`. Insertions and
renumbering instructions are not read.

Explanations and needs show a cited paragraph's quoted text; for a passage
cited `as amended`, the wording in force and `as_amended_by` (see
[Explanations](calculations.md#explanations)).

### Metadata

`@ meta(<value>)` attaches one ordinary typed Futuruna value to the item below
it without changing evaluation. The value is a binding's name
(`@ meta(tax_case_guide)`) or any pure ground value written inline
(`@ meta(Assumption(value = assumption_ask_tax_paid_and_room))`). Its root type
must explicitly implement the standard marker trait `Meta`. Several `@ meta`
lines attach several values. Roles, sources, warnings, fields and other
structure belong in the value, not in the annotation, so the annotation grammar
cannot grow into a second schema language. The metadata index resolves the
value through Futuruna's type information, so tools can query metadata by role
or by domain type.

- On a section (above `@ section`), the values describe the whole section;
  explanations list them with the roles of its quotes as the section's
  attachments.
- On a rule clause, a RuleScope member or a function, the value describes that
  item and gives it a code span. An `Assumption` written on a clause is
  reported only when that clause fires. Metadata of a calculation entry stands
  above its `@ calculate` line.
- On a data type or a binding, the value describes the declaration.
- `@ meta` cannot stand above a `----` block: a quoted document's role is
  written on the quote.

**Metadata written apart.** A model may keep the metadata of declarations in a
file of its own, for example one that describes the input types the domain
files declare:

```runa
@ import ./domain

= age_note = AgeNote(text = "Age at the end of the year.")
@ meta(age_note) for Child
```

`@ meta(<value>) for <name>` stands alone at top level and attaches to nothing
below it. `<name>` is a checked name, never a string: a type, rule, function
or section declared in the file or a file it imports. A name that names
nothing in scope is an error (`` `@ meta(...) for Chlid`: no type, rule,
function or section named `Chlid` is declared in this file or a file it
imports ``), and so is a name that names two things, such as a section and
the rule it holds. It means exactly what `@ meta(<value>)` on the declaration
means: the value is resolved where the line is written and indexed under the
declaration's name, so field metadata, entry metadata, needs and the contract
read it unchanged.

A declaration's metadata is written in one place: on the declaration, or in
one `for` (consecutive `for` lines for one name, with only comments between,
are one place). Metadata written on a declaration and in a `for`, or in two
`for`s anywhere in the program, is an error that names both places, so a
reader always finds all of a declaration's metadata in one place.
`runa meta --json` says where each value is written (`target.declared_in`,
`written`: `on_declaration` or `for`, and `written_line`), and an editor hover
on a declaration lists its metadata and where each value is written.

Known gap: a `for` gives a rule or function no code span, so explanations do
not attach its metadata to the rule's clauses. Metadata that explanations
should attach to a clause is written with `@ meta` on the clause.

When one item needs many related metadata values, attach one typed aggregate.
Every constructor nested inside a pure ground aggregate retains its Futuruna
type and a stable value path. Tools can therefore select the values they
understand while the annotation stays short and domain-neutral.

```runa
# CalculationMeta(fields: List(CalculationField))
# impl Meta for CalculationMeta {}

= amount_meta = CalculationMeta(fields = [
    amount_field,
    currency_field,
    period_field
])

@ meta(amount_meta)
@ calculate
> calculate_amount(input: AmountInput) -> Amount { ... }
```

`runa meta --type CalculationField` finds `amount_meta` because it contains
typed `CalculationField` descendants. JSON output reports those descendants as
`typed_values` with paths such as `$.fields[0]`. This behavior is generic: the
aggregate and nested types are ordinary user-defined Futuruna types. Only the
marker traits `Meta`, `MetaRole` and `Document` and the annotations are
standardized.

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
onto every reachable occurrence. Attach that metadata with `@ meta` on the
nested type. An exact root-input reference or absolute `pathof(...)`
declaration overrides a projected declaration; equal-specificity collisions
fail closed.

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

@ meta(evidence)
| calculate_tax(income: Int) -> income / 2
```

`refof(rule_name)`, `refof(TypeName)`, `refof(TypeName::field::nested_field)`
and `refof(section name)` produce typed `ProgramReference` values.
Rules, functions, bindings, types, structural fields, RuleScope members and
sections are checked against local and plain imported declarations. Misspelled
or ambiguous targets fail `runa check`, and definition navigation follows the
reference. Creating a reference does not execute its target and does not add a
rule dependency; only actual calls define the program's dependency graph.

When the aggregate also needs role-bearing values, define a sum type whose
variants each have one named `value` field and implement the standard
`MetaRole` marker. This moves the role map into ordinary Futuruna data instead
of extending the annotation syntax or relying on a magic constructor name.

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

@ meta(shape_meta)
| circles_rule(shape: Shape) -> shape == Circle
```

`Meta` and `MetaRole` are empty marker traits and add no runtime behavior. Every
applied variant of a `MetaRole` type must have exactly one named `value` field.
Its constructor is exposed as lower snake case, so `DependencySource` becomes
`dependency_source`. A role type may also implement `Meta` when one role value
is itself the root attachment, as in `@ meta(Assumption(value = ...))`.
Multiline tuples accept line breaks and a trailing comma. The former
`MetaAttachment(role = ..., value = ...)` shape remains readable, not canonical
source.

### The metadata index

`runa meta --type Shape file.runa` finds references whose value has type
`Shape` or whose typed aggregate contains a `Shape`. `runa meta --role warning
file.runa` finds warning references, and the filters can be combined. Pure
ground bindings also expose a static value, typed descendant paths, and
definition location. Nested attachments additionally expose their normalized
role, canonical value binding, attachment path, and value path. A `@ meta`
value may name a binding in the current file or in any recursively reachable
plain import; aggregate resolution follows referenced bindings transitively
through nested imports. A section's quote roles are references of the section
like its `@ meta` values. Dynamic or unresolved values and misplaced
annotations produce metadata diagnostics; `runa meta` and metadata-aware
consumers such as `runa schema` fail closed on those diagnostics.

Audit and indexing tools should use `runa meta --json file.runa`. The versioned
`futuruna.meta.v2` document contains typed references, an anchor per section
and per item with `@ meta`, code spans and their declared types, rules,
bindings, and functions, plus metadata diagnostics. Every anchor names its
`target`: `kind` (`section`, `type`, `binding`, `function`, `rule` or
`member`), `name` and `declared_in` (the file that declares it), and says
where its metadata is written: `written` (`on_declaration` or `for`) and
`written_line`. A section is a code span; so is a function, rule clause
or member with `@ meta`. Each ground reference retains its Futuruna rendering
in `value` and exposes a structural `data` tree. Constructors include their
parent Futuruna type. Constructors, named arguments, lists, tuples, and
primitive values are separate nodes, while `typed_values` indexes every nested
constructor by type and value path. An audit can therefore inspect an
`AuditWarning` field or a `SourceInfo` URL without parsing Futuruna display
text. The additive `attachments` index makes nested `MetaRole` variants
directly queryable by role without parsing the structural tree. It also
reports the participating `meta_role_types`. Expression-level forms such as
`match` arms are not span symbols. `--type` and `--role` apply to JSON output
as well, so a warning sweep can use `runa meta --json --role warning file.runa`
without parsing presentation text.

A section anchor reports `text_begin_marker_line` and `text_end_marker_line`
for the first quote's opening and the last quote's closing `----`, and
`text_start_line` and `text_end_line` for the quoted content between them; its
`text` is the section's quotes, each document's text after a `[<file>]` line.
A section without quotes has no text range.

Pass a directory to sweep a complete source tree recursively:

```sh
runa meta --json --type Shape --role source examples/
runa meta --json --role warning examples/danish-income-tax/
```

Directory output uses the separate `futuruna.meta.collection.v1` schema. Its
`files` array contains filtered `futuruna.meta.v2` documents in stable path
order, while its top-level counts distinguish files scanned from files returned.
Corpus diagnostics repeat the source file beside every line and message, so an
audit can report invalid metadata without losing its location. A collection
with metadata diagnostics is still emitted as JSON and exits unsuccessfully.

### Migrating from the comment directives

`runa fmt --migrate <files or directories>` rewrites a model's comment
directives as annotations in place, then formats it. It is idempotent, and
`runa fmt --migrate --check` lists the files that need migration without
writing them. Files are migrated against the program they belong to (the
directory of the nearest `runa.toml`, else the file's own directory):

1. `--@label:X::meta:B--` with a `----` quote and a `--@begin:X--` ...
   `--@end:X--` span becomes `@ meta(B)` and `@ section X { ... }` around the
   span, with the quote moved to the section's start. Each `[sources/<file>]`
   part of the quote becomes its own `@ quote(<binding>)`, where `<binding>`
   is the program's only binding whose `file` is `<file>`.
2. When `B` is used by this section only and holds `MetaRole` values whose
   `value` is a quoted document, those values move onto the quotes
   (`@ quote(Guidance(value = ...))`) and leave `B`. A `B` left with one value
   is written inline as `@ meta(<value>)`; an empty `B` is removed.
3. `--@label:X::meta:B--` naming the declaration `X` (with or without a span
   around that declaration only) becomes `@ meta(B)` on `X` when `X` is
   declared in the same file, and `@ meta(B) for X` in place when it is
   declared in another file.
4. `--@cite:...--` becomes `@ cite(...)` at the clause's indentation;
   `in sources/<file>` becomes `in <binding>`. A citation naming a paragraph
   and a part of it keeps the paragraph and drops the part; each drop is
   reported, since it changes what an explanation quotes.
5. `--@source::X::meta:B--` is read as `--@label`. A direct role chain
   (`::source:B`) becomes `@ meta(B)`, and the dropped role is reported.

Quoted text with no `[sources/...]` document stays a `----` block comment
directly above the section. A type whose values are quoted documents gains
`# impl Document for <Type> {}` after its declaration.

The migration leaves a file unchanged and lists why when it cannot rewrite it:
a `[sources/<file>]` that no binding (or more than one) declares, a
`--@begin` without its `--@end` or spans that close out of order, an indented
span, a quote that names documents but has no span, an anchor with two quotes,
or a directive it does not read. It cannot know which passages a model applies
as amended; after it, `runa check` reports every plain citation of a passage
that a quoted instruction amends.

## Types are the domain vocabulary

Define types before using them. Name constructors after domain concepts, not implementation.

```runa
-- YES: the domain speaks
# Magtholder = Hos(institution: Institution) | IForening(a: Institution, b: Institution)

-- NO: the programmer speaks
# PowerHolder = Single(inst: Institution) | Joint(inst1: Institution, inst2: Institution)
```

When using `@ sprog da`, write identifiers in Danish — including æ, ø, å. The Futuruna lexer uses Unicode-aware `is_alphabetic()`, so Danish characters work natively in identifiers, type names, and constructors. Rust codegen preserves them (Rust supports non-ASCII identifiers since 1.53).

[Danish source files](basics.md#danish-source-files) lists the Danish
keywords and builtin names and how they resolve.

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
