# RFC: One annotation system

Status: Proposed (Futuruna 0.5). Replaces the `--@` comment directives.

## Problem

Futuruna attaches facts about code in two ways that do not know each other.

- **Typed annotations** stand before a declaration and are parsed with it:
  `@ export`, `@ comptime`, `@ calculate("title")`, `@ field(label = ...)`.
- **Comment directives** are comments that a separate scanner reads and that
  name their targets by string: `--@label:<name>::meta:<binding>--` attaches a
  metadata value to the name `<name>`, a `----` block after such an anchor is
  that name's quote, `[sources/<file>]` lines inside the quote name its
  documents, `--@begin:<name>--` and `--@end:<name>--` mark the code that
  implements it, and `--@cite:§ 9, stk. 3--` names the paragraphs one clause
  implements ([Explanations](explanations.md#paragraph-citations-cite)).

Because the second layer is made of strings and adjacency, the language cannot
check most of what it says:

- A section's name is written three times (`--@label`, `--@begin`,
  `--@end`); a typo in one silently makes a different section, or none.
- A quote belongs to whatever anchor happens to be on the line above it. A
  blank line or a moved comment detaches it without an error.
- A quoted document is a file path string inside the quote, and the same
  document is listed again as a `Source(...)` or `Guidance(...)` attachment
  of the section's metadata. Nothing checks that the two lists agree, and
  nothing in the compiler checks that the quote is verbatim: tax-service runs
  `scripts/verify_quotes.py` for that.
- `--@cite` exists only because `@` cannot stand above a rule clause or a
  RuleScope member (explanations.md: "an `@` annotation ... does not reach").
- "label" means two things: in `--@label:ask_ceiling` it is the name of an
  anchor, in `@ field(label = "Trade date")` it is text shown to a person.
- A citation may say the same paragraph twice: `--@cite:§ 13; § 13, stk. 2--`
  (tax-service `src/ask.runa:406`) shows stk. 2 twice, because a bare `§ 13`
  already quotes the whole paragraph.

tax-service carries 50 `--@label` anchors, 50 `--@begin`/`--@end` pairs, 71
`--@cite` lines and 86 `[sources/...]` lines (`src/` and `tests/`).

## Design

Everything that describes code is an `@` annotation, parsed with the item it
stands before, with typed arguments. The comment directives are removed.

### The annotations

| Annotation | Stands before | Means |
|---|---|---|
| `@ section <name> { ... }` | top-level items (a block) | the items inside implement one passage of a source; `<name>` names the passage |
| `@ quote(<document>)` | a `----` block, inside a section | the block is verbatim text of `<document>` |
| `@ cite(<paragraphs>)` | a rule clause, an exception, a RuleScope member, a function | the clause implements these paragraphs of its section's quotes |
| `@ meta(<value>)` | any declaration, clause, member or section | attaches a typed metadata value to it |
| `@ field(...)` | a field or a data type | interview metadata, unchanged ([Field metadata](field-metadata.md)) |
| `@ calculate("title")` | a rule or function | a calculation entry, unchanged |
| `@ export`, `@ comptime` | a declaration | unchanged |

The list is closed: an unknown `@ name` is an error, as today. Structure a
model wants to record beyond it is a typed value in `@ meta(...)`, so the
annotation grammar does not grow into a second schema language.

`@ print`, `@ import`, `@ use`, `@ depend` and `@ rust` are statements, not
annotations; they keep their meaning.

**Placement.** An annotation stands on its own line directly before its item.
Comments may stand between them; a blank line may not. Several annotations on
one item are written one per line, in any order. An annotation that is
followed by nothing it can describe is an error that says what it may stand
before. The parser attaches annotations to items, so `@ cite` reaches every
clause form: a top-level clause, an `exception` clause, and a member inside a
RuleScope body `# AskYear(...) { ... }`.

**One meaning of "label".** `label` is only ever display text (`@ field`,
`@ calculate`'s title). Code is named by identifiers: a section by the name
after `@ section`, a declaration by its own name.

### Sections

```runa
@ section ask_ceiling {

-- items: types, rules, functions, bindings, quotes

}
```

- A section is a block of top-level items. It opens no scope: every name
  declared inside is a top-level name of the file, exactly as without the
  section, and code outside calls it as before.
- `runa fmt` does not indent a section's body. A section groups declarations
  around a passage of law; indenting would move every line of a model one
  level right and suggest a scope that does not exist.
- Section names have their own namespace, unique per program. A section may
  share its name with a rule it holds (tax-service's `ask_eligible` section
  holds the rule `ask_eligible`). `refof(section ask_ceiling)` is a
  `ProgramReference` to it.
- Sections do not nest and stand only at top level.
- The name is written once. Explanations and needs report it as `section`, as
  today.

### Quotes

```runa
@ quote(Source(value = aktiesparekontoloven))
----
§ 9. Personer, der er fuldt skattepligtige, kan foretage indskud på
...
----
```

- The argument is a **document**: a binding whose type implements the
  standard marker trait `Document`, or a `MetaRole` value whose `value` is
  one. `Document`, like `Meta` and `MetaRole`, adds no behaviour; it requires
  the type to have a field `file: String`, a path relative to the project
  root (the directory of `runa.toml`, or of the root source file without
  one). tax-service declares this once: `# impl Document for SourceInfo {}`.
- When the argument is a role value (`Source(value = ...)`,
  `Guidance(value = ...)`), the role is an attachment of the section. The
  document is written once, at the quote, with its role; it is not listed
  again in the section's metadata.
- The `----` block holds only the document's text, verbatim and unindented.
  There is no `[sources/...]` line: a section that quotes four documents has
  four quotes, each naming its document.
- `runa check` reads the file and finds every paragraph of the quote (lines
  separated by a blank line) in it, comparing after collapsing whitespace and
  removing soft hyphens, as `verify_quotes.py` does. A missing file or a
  paragraph the file does not hold is an error at that paragraph's line.
  `runa call`, `runa build` and the WebAssembly engine do not read the files:
  a quote's text is part of the program.
- A section's quote is the concatenation of its `@ quote` blocks in order.
  Quotes may stand anywhere in the section, so a long passage can sit next to
  the rules for it, paragraph by paragraph.
- Inside a section, every `----` block is a quote and needs `@ quote(...)`.
  Outside sections a `----` block stays a block comment. `@ quote` outside a
  section is an error.

### Citations

```runa
@ cite(§ 9, stk. 3)
| excess_levy_ore(excess_days_ore: Ore, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
```

The argument is the citation syntax of today: paragraphs separated by `;`,
each `§ N` with optional `, stk. N` and `, nr. N`, and now `, N. pkt.` (a
sentence). `in <document>` names the document by its binding
(`§ 1 in bek_19_2024`), where today it is a file path string. The checker
resolves each paragraph in the section's quotes as it does today, and the
rules of `--@cite` carry over: a citation on a rule's first clause covers the
clauses after it that cite nothing, an exception with its own citation is
cited by it, a function may be cited.

A sentence is found by splitting a quoted subsection after a full stop that
is followed by a space and a capital letter and that does not end an
abbreviation (`kr.`, `pct.`, `stk.`, `nr.`, `pkt.`, `jf.`, `m.v.`). An error
about a `pkt.` lists the sentences the checker found, so a split it got wrong
is visible.

One rule is new: **a citation may not name a paragraph and a part of it.** A
bare `§ 13` is the whole paragraph, so `@ cite(§ 13; § 13, stk. 2)` is an
error ("`§ 13` already contains `§ 13, stk. 2`; cite `§ 13` or its parts").
An explanation then never shows the same quoted lines twice.

#### Citing a passage as amended

A consolidated act (a *lovbekendtgørelse*) is often older than the law in
force. tax-service's share income rates are personskatteloven § 8 a, stk. 1
and 2, as reworded by LOV nr. 1691 af 30/12/2024 § 4: the consolidated LBK
1284/2021 still says 48.300 kr., and the amending act's text has no `§ 8 a`
paragraph marker, only an instruction "1. § 8 a, stk. 1, 1. pkt., affattes
således:" followed by the new sentence. Today the rules that apply the rates
(`low_rate_part_ore`, `high_rate_part_ore`, `tax_magnitude_ore` in
`src/frit_depot.runa`) carry no citation, because any citation of § 8 a
would quote the superseded figure.

A citation names such a passage with `as amended`:
`@ cite(§ 8 a, stk. 1, 1. pkt. as amended)` (the model's full form is below).

**Reading an amending act.** In a quote, the checker reads the amending
act's own markers, as the official rendering writes them: a line
`§ N I <act>, jf. <consolidation>, ... foretages følgende ændring(er):` opens
amending paragraph `§ N` (it has no full stop after the number), and a line
`N. <passage>, <verb>` inside it opens instruction `nr. N`, whose target is
`<passage>` and whose verb is one of:

| Verb | The passage as amended is |
|---|---|
| `affattes således:` | the text after the instruction, up to the next instruction |
| `ændres »x« til: »y«` | the passage's text in the base quote with its only `x` replaced by `y` |
| `ophæves` | gone: citing it as amended is an error |

**What the checker verifies** for `<passage> as amended`:

1. The section quotes exactly one instruction whose target is `<passage>`.
   With several (two successive amendments), the citation names the one it
   means: `§ 8 a, stk. 1, 1. pkt. as amended by personskatteloven_aendring_2024 § 4, nr. 1`.
2. That instruction stands under its amending paragraph line in the quote,
   so the reader can be told which act and paragraph made the change.
3. The amending paragraph's `jf. lovbekendtgørelse nr. <n> af <date>` has
   the number and year of the base document's identifier, when the section
   quotes the base: an instruction for personskatteloven cannot be used to
   amend a quote of kildeskatteloven.
4. For `ændres »x« til: »y«`, the passage is in the base quote and `x` occurs
   in it exactly once.

And the converse: **a plain citation of a passage that an instruction quoted
in the section amends is an error**, as is a plain citation of a paragraph or
subsection containing it:

```text
citation `§ 8 a, stk. 1, 1. pkt.`: LOV nr. 1691 af 30/12/2024 § 4, nr. 1 (quoted in section `share_income`) rewords this passage; cite `§ 8 a, stk. 1, 1. pkt. as amended`
citation `§ 8 a, stk. 1`: contains § 8 a, stk. 1, 1. pkt., reworded by LOV nr. 1691 af 30/12/2024 § 4, nr. 1; cite its parts
```

So a superseded figure is never shown as the cited law. A provision of the
amending act itself, such as its commencement rule, is cited plainly with
`in`: `§ 9, stk. 2 in personskatteloven_aendring_2024`.

**What the reader sees.** A cited paragraph in an explanation or a need
carries, besides its act and text, `as_amended_by` (the amending act, its
paragraph and instruction number, and the instruction line). Its `text` is
the wording in force, not the consolidated one:

```json
{ "paragraph": "§ 8 a, stk. 1, 1. pkt.",
  "act": "Personskatteloven, LBK nr. 1284 af 14/06/2021",
  "as_amended_by": {
    "act": "LOV nr. 1691 af 30/12/2024", "paragraph": "§ 4, nr. 1",
    "instruction": "1. § 8 a, stk. 1, 1. pkt., affattes således:" },
  "text": "Skat af aktieindkomst, der ikke overstiger et grundbeløb på 51.500 kr. (2010-niveau) for indkomståret 2025, et grundbeløb på 57.800 kr. (2010-niveau) for indkomståret 2026 og et grundbeløb på 63.400 kr. (2010-niveau) for indkomståret 2027 og senere indkomstår, beregnes som en endelig skat på 27 pct." }
```

A section's whole quote, as explanations list it, marks a quoted base passage
that an instruction in the same section amends as superseded, with that
instruction, so the 48.300 kr. sentence is shown as replaced. (The field
names follow the explanation schema of [Explanations](explanations.md), which
adds them additively.)

In the model the amending act's quote gains its paragraph line, copied from
the source file, and the three rate rules gain citations:

```runa
@ quote(Source(value = personskatteloven_aendring_2024))
----
§ 4 I personskatteloven, jf. lovbekendtgørelse nr. 1284 af 14. juni 2021, som ændret senest ved § 1 i lov nr. 482 af 22. maj 2024, foretages følgende ændringer:

1. § 8 a, stk. 1, 1. pkt., affattes således:

Skat af aktieindkomst, der ikke overstiger et grundbeløb på 51.500 kr.
[…]

2. § 8 a, stk. 2, 1. pkt., affattes således:

Skat af aktieindkomst, der overstiger et grundbeløb på 51.500 kr. (2010-niveau)
[…]
----
[…]
@ cite(§ 8 a, stk. 1, 1. pkt. as amended)
| low_rate_part_ore() -> min_int(abs(taxable_share_income_ore()), threshold_ore())
@ cite(§ 8 a, stk. 2, 1. pkt. as amended)
| high_rate_part_ore() -> abs(taxable_share_income_ore()) - low_rate_part_ore()
@ cite(§ 8 a, stk. 1, 1. pkt. as amended; § 8 a, stk. 2, 1. pkt. as amended)
| tax_magnitude_ore() -> share_of(low_rate_part_ore(), parameters.share_income_low_bp) + share_of(high_rate_part_ore(), parameters.share_income_high_bp)
```

### Metadata

`@ meta(<value>)` attaches a value whose type implements `Meta` to the item
below it. The value is any pure ground expression, resolved like a metadata
binding today, so it may be a binding name (`@ meta(tax_case_guide)`) or the
value itself (`@ meta(Assumption(value = assumption_ask_tax_paid_and_room))`).
Several `@ meta` lines attach several values. `runa meta` indexes them by
type and role as it indexes anchored metadata today.

- On a calculation entry or a data type, `@ meta` replaces
  `--@label:<name>::meta:<binding>--`: field metadata lists keep their
  exact-path precedence ([Field metadata](field-metadata.md#precedence)).
- On a section, its values describe the whole section; explanations list them
  with the roles of its quotes as the section's attachments.
- On a clause, a value describes that clause only. An `Assumption` written on
  the clause it qualifies is reported only when that clause fires, which is
  finer than the per-section assumptions of
  [Calculation needs](calculation-needs.md).

## What becomes impossible to get wrong

| Today | With one annotation system |
|---|---|
| `--@begin:ask_celing--` makes a new, empty section | the name is written once |
| `--@end` missing, or closing another section | a block; the parser matches it |
| a quote detached from its anchor by a blank line | `@ quote` is attached to its block; a `----` block in a section without `@ quote` is an error |
| `[sources/08-DJV.md]` naming a file that does not exist | `@ quote(juridisk_vejledning_ask_indskud)` is a checked name; its file must exist |
| a quoted paragraph that drifted from its source | `runa check` compares every paragraph with the file |
| a document quoted but not listed as a source, or listed but not quoted | the role is written on the quote |
| a metadata anchor whose name no longer matches the renamed entry | `@ meta` stands on the entry |
| `--@cite` on a RuleScope member written as a comment the parser ignores | `@ cite` is part of the member |
| `§ 13; § 13, stk. 2` quoting stk. 2 twice | an error |
| `§ 1 in sources/b.txt` with a mistyped path | `§ 1 in bek_19_2024` is a checked name |
| a rule of an amended passage cited with the consolidated, superseded text, or left uncited | `as amended` shows the wording in force; a plain citation of an amended passage is an error |
| an amendment cited for a passage it does not amend | the instruction's target must be the cited passage |
| "label": an anchor or display text | display text only |

A leftover `--@label`, `--@begin`, `--@end`, `--@cite` or `--@source` comment
is an error that names its replacement and `runa fmt --migrate`, so no
citation or attachment is lost by being silently read as a comment.

## The ceiling section, before and after

tax-service `src/ask.runa:155-306`, before (`[…]` marks lines left out of
this excerpt; the quoted law text is copied verbatim from the model):

```runa
= assumption_ask_tax_paid_and_room = AssumptionInfo(id = "ASKL-9-TAX-PAID", […])

= ask_ceiling_meta = Metadata(
    attachments = (
        Source(value = aktiesparekontoloven),
        Guidance(value = juridisk_vejledning_ask_indskud),
        Guidance(value = juridisk_vejledning_ask_beskatning),
        Guidance(value = ekapital_ask),
        Assumption(value = assumption_ask_tax_paid_and_room),
    )
)

--@label:ask_ceiling::meta:ask_ceiling_meta--
----
[sources/01-aktiesparekontoloven-LBK-281-2025.txt]

§ 9. Personer, der er fuldt skattepligtige, kan foretage indskud på
aktiesparekontoen, i det omfang den samlede værdi af aktiesparekontoen inklusive
årets nettoindskud ikke overstiger et grundbeløb på 87.450 kr. (2010-niveau),
jf. dog stk. 2.
[…]

[sources/08-DJV-C.B.5.2-indskud.md]
[…]
[sources/08-DJV-C.B.5.3-beskatning.md]
[…]
[sources/07g-eKapital-ASK-aktiesparekonto-2026.md]
[…]
----

--@begin:ask_ceiling--
[…]
--@cite:§ 9, stk. 3--
| excess_levy_ore(excess_days_ore: Ore, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
[…]
--@cite:§ 9; § 9, stk. 2; § 9, stk. 3--
> ask_cash_step(run: AskCashRun, e: Event, ceiling_ore: Ore) -> AskCashRun {
[…]
--@end:ask_ceiling--
```

After `runa fmt --migrate` (which also drops the overlapping parts of
`ask_cash_step`'s citation, see Migration):

```runa
= assumption_ask_tax_paid_and_room = AssumptionInfo(id = "ASKL-9-TAX-PAID", […])

@ meta(Assumption(value = assumption_ask_tax_paid_and_room))
@ section ask_ceiling {

@ quote(Source(value = aktiesparekontoloven))
----
§ 9. Personer, der er fuldt skattepligtige, kan foretage indskud på
aktiesparekontoen, i det omfang den samlede værdi af aktiesparekontoen inklusive
årets nettoindskud ikke overstiger et grundbeløb på 87.450 kr. (2010-niveau),
jf. dog stk. 2.
[…]
----

@ quote(Guidance(value = juridisk_vejledning_ask_indskud))
----
[…]
----
[…] (two more quotes: juridisk_vejledning_ask_beskatning, ekapital_ask)

[…]
@ cite(§ 9, stk. 3)
| excess_levy_ore(excess_days_ore: Ore, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
[…]
@ cite(§ 9)
> ask_cash_step(run: AskCashRun, e: Event, ceiling_ore: Ore) -> AskCashRun {
[…]

}
```

What changed: the name `ask_ceiling` is written once instead of three times;
the 8-line `ask_ceiling_meta` binding is gone, because each document is named
once, with its role, at its quote, and the assumption stands on the section;
the four file paths are replaced by checked names; `ask_cash_step`'s citation
says § 9 once instead of quoting stk. 2 and stk. 3 twice; and a RuleScope
member such as `taxable_base_ore` in `AskYear` (`src/ask.runa:406`) carries
`@ cite(§ 13)` as part of the member.

## What the writer writes and what the reader sees

The writer writes each fact once, where it applies: the passage's name at the
section, a document and its role at its quote, a paragraph at its clause, an
assumption at the section or clause it qualifies. An editor follows
`@ quote(aktiesparekontoloven)` to the document's declaration and
`refof(section ask_ceiling)` to the section, and a rename of the entry cannot
detach its metadata.

The reader of a result sees what explanations and needs show today, from one
source instead of a comment scanner: `section` is the section's name, a cited
paragraph's text is quoted verbatim and now checked against the file, and the
section's documents carry their binding and role, so the act (title and
identifier, the `Document` value's other fields) reaches the explanation
without parsing a file header. The contract changes additively: explanation
sections list `documents` (binding, file, role) beside the existing quote.
`runa meta --json` writes `futuruna.meta.v2`: attachments name their target
item (a declaration, clause, member or section) instead of a label string, and
code spans are sections.

## Migration

`runa fmt --migrate <files or directories>` rewrites a model in place and is
idempotent. It parses the comment directives with today's scanner and writes:

1. `--@label:X::meta:B--` with a `----` block after it and a
   `--@begin:X--` ... `--@end:X--` span becomes `@ meta(B)` and
   `@ section X { ... }` around the span, with the quote moved into the
   section's start. Each `[sources/<file>]` part becomes its own
   `@ quote(<binding>)`, where `<binding>` is the program's only `Document`
   binding whose `file` is `<file>`.
2. When `B` is used by this section only and holds `MetaRole` values whose
   `value` is a quoted document, those values move onto the quotes
   (`@ quote(Guidance(value = ...))`) and leave `B`. A `B` left with one
   attachment is written inline as `@ meta(<value>)`; an empty `B` is
   removed.
3. `--@label:X::meta:B--` before the declaration `X` (with or without a
   `--@begin:X--` span around that declaration only, as for an entry's input
   guide) becomes `@ meta(B)` on `X`, with no section.
4. `--@cite:...--` becomes `@ cite(...)` in place, at the clause's
   indentation. `in sources/<file>` becomes `in <binding>`. A citation naming
   a paragraph and a part of it keeps the paragraph and drops the part; each
   drop is reported, since it changes what the explanation quotes.
5. `--@source::X::meta:B--` (the older spelling) is read as `--@label`.

The migration stops on a file without writing it, and lists what it cannot
rewrite: a `[sources/<file>]` with no `Document` binding (or two), a span
whose begin and end do not match, a quote with no span, `B` shared by several
sections (rule 2 is then skipped and the duplicate is reported by
`runa check`). For tax-service the first case does not occur:
`verify_quotes.py` already requires every quoted file to be a `SourceInfo` in
`src/sources.runa`; the model adds `# impl Document for SourceInfo {}`.

The migration cannot know which passages a model applies as amended. After
it, `runa check` reports every plain citation of a passage that a quoted
instruction amends; in tax-service there is none, since the § 8 a rate rules
are uncited today. The writer adds the amending paragraph line to the quote
of `personskatteloven_aendring_2024` and the three `as amended` citations by
hand, as shown above.

After migration tax-service deletes `scripts/verify_quotes.py`; `runa check`
does its work. Futuruna's own docs, examples and tutorial (`style.md`,
`calculations.md`, `tutorial/README.md`, the Explanations and Field metadata
RFCs) are migrated in the same change.

## Alternatives rejected

- **Keep the directives and lint them.** A linter could match begin and end
  names, but two grammars would remain, the names would still be strings, and
  `@` would still not reach clauses. The lint would be a second parser of the
  same comments.
- **`@ begin(X)` / `@ end(X)` statements.** Typed spelling of the same pairs:
  the name still appears twice and pairs still have to be matched by name.
- **Sections as modules (`> module`).** A module scopes names and changes how
  code outside calls them. A section is a passage of law, not a namespace;
  the rules inside stay what they are.
- **Indented section bodies.** Re-indents every model and suggests a scope.
- **Quote text as a string argument** (`@ quote(doc, """...""")`): legal text
  would need escaping, and `{{` would interpolate.
- **Keeping `[sources/...]` lines inside the quote.** The only non-verbatim
  lines of a verbatim block, and a path string the checker cannot resolve to
  a declaration.
- **Roles only in section metadata, documents only in quotes.** The document
  is then written twice, and the lists can disagree.
- **Typed citation arguments** (`@ cite(par(9, stk = 3))`): a lawyer reviews
  `§ 9, stk. 3`; the legal form is checked against the quote anyway.
- **User-defined annotations.** Any structure a model needs is a typed value
  under `@ meta`, indexed by type and role.
- **Quoting a hand-consolidated § 8 a.** Text that appears in no source cannot
  be checked verbatim, and the reader loses who changed the law and when.
- **Citing only the amending act** (`§ 4, nr. 1 in personskatteloven_aendring_2024`).
  The reader would see an instruction, not the paragraph in force, and the
  checker could not tell that the rule applies § 8 a.
- **Resolving amendments silently** (a plain `§ 8 a, stk. 1, 1. pkt.` shows
  the amended text when an instruction is quoted). The code reader would not
  see that the rule rests on two acts; `as amended` costs two words.

## Open questions

- **Verbatim granularity.** A paragraph is compared whole. A quote that
  shortens a sentence with "[…]" fails; should an elision marker be allowed,
  and shown as such in explanations?
- **Act identity.** [Calculation needs](calculation-needs.md) attaches the act
  of a cited paragraph from the source file's header. With typed documents the
  `Document` value carries the identifier too; one of the two must be the
  authority, and the other checked against it.
- **Documents outside the project.** `file` is relative to the project root.
  A dependency's sources would need its own root.
- **When an amendment applies.** LOV 1691/2024 § 9, stk. 2 gives § 4 effect
  from income year 2025. The citation states the wording; which income years
  a rule applies it to stays the model's parameters. Should a citation also
  name the commencement provision, and the checker require it?
- **Further instruction verbs.** Insertions (`indsættes ... som nyt stk.`)
  and renumbering (`Stk. 5 bliver herefter stk. 7`) change the numbering of
  the base act; this RFC supports rewording, replacement and repeal only.
- **Sentence splitting.** The `pkt.` rule above is a heuristic over Danish
  legal text; a quote where it fails needs a way to mark sentence boundaries.
