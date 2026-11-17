---
feature_stage: mixed
feature_stage_surfaces:
  - typed-calculation-contracts
  - calculation-field-declarations
  - calculation-needs
  - needs-in-the-language
  - answer-guide
  - typescript-sdk
  - annotations
---

# Entries

`@ entry` marks an ordinary typed rule or function as an external
calculation boundary. It does not change rule evaluation and is not an effect.

A calculation is pure: its entry, and every function and rule it reaches, must
not perform effects — no `@` effects and no host builtins such as `print`,
`read_file`, `write_file`, `env_var`, `now`, `http_get` or `process_run`. The
checker reports the first such call with its location, and `runa call` refuses
host effects at run time as well (for example in imported helpers). External
data enters only through the typed input.

```runa
# Input(monthly_income: Int, status: FilingStatus)
# Result(annual_tax: Int)

@ entry("Household tax calculation")
| calculate(input: Input) -> Result(annual_tax = annual_tax(input))
```

A calculation has exactly one explicitly typed input parameter. A rule result
must infer to one concrete serializable type; a function must declare its result
type. The input is one named domain type. Conditions, defaults, exceptions,
matches, rule scopes, and ordinary rule dependencies continue to work normally.

Use plain `@ entry` when the rule name is sufficient, or provide one
human-readable title with `@ entry("Household tax calculation")`. The title
names the whole calculation and appears in its schema and generated workbook.
It does not label every nested input field. Field labels, interview questions,
help, units, and sources belong in field metadata.

## Describe Human Input

Keep the calculation boundary machine-stable and give each input field its
human presentation: a label, an interview question, help, a unit, sources, and
the calculation that provides it. Write it with `@ field(...)` on the
declaration of the field it describes:

```runa
# SourceInfo(url: String, section: String)
# EntryMetaRole(a) = Source(value: a)
# impl MetaRole for EntryMetaRole {}

= tax_source = SourceInfo(url = "https://example.invalid/tax", section = "income")

# Input(
    @ field(label = "Monthly income before tax", question = "What do you earn before tax each month?", help = "Use the gross amount before deductions.", unit = "currency/month", sources = [Source(value = tax_source)])
    monthly_income: Int,
    status: FilingStatus
)

@ field(label = "Filing status", question = "Do you file alone or jointly?")
# FilingStatus = Single | Joint
```

`@ field(...)` before a named field, in a record or in a variant payload,
describes that field wherever its type occurs in a calculation input. Before a
`#` data type it describes every input field whose value has that type: the
field of an enum such as `status`, the `$variant` column of a sum with
payloads, the collection or optional column of a record. List, set, optional
and map wrappers are transparent, as in `pathof`.

| Key | Value |
|---|---|
| `label` | required `String` |
| `question`, `help`, `unit` | `String` |
| `sources` | list of source values such as `[Source(value = law)]`, whose type implements `MetaRole` |
| `entry`, `subject` | together: `entry = refof(<calculation>)`, the `@ entry` that provides the field, and `subject = pathof(<Input>::...)`, the input path it runs for, rooted at the calculation's input type (see [Needs](#needs) below). The older strings (`entry = "share_income"`, `subject = "taxpayer.spouse"`) are still read; `runa fmt --migrate` rewrites them |
| `key` | `Bool`: `True` makes this `String` or `Int` field of a record type the key that identifies an element of the type in a list (one per type); the contract's definition of the type names it as `key`, and a need inside such an element carries the element's key |

A key that does not apply is left out; values are plain strings (`key` a
plain `Bool`, `entry` a `refof`, `subject` a `pathof`), not `Some(...)`. A
value may be any pure ground expression, resolved like a metadata binding.
`runa check` reports an unknown or repeated key, a missing label, `entry`
without `subject`, two `@ field` on one field or type, an `@ field` that no
field or data type follows, a value that is not a `String` (or, for
`sources`, not a list; for `key`, not a `Bool`; for `entry`, not a `refof` of
a calculation), a providing entry that does not fit the field (see below),
and a `key` on a field that is not a `String` or `Int` of a record type, or a
second one in a type. Declarations are presentation only: they do not change a
type's meaning, content hash or semantic interface.

A required record field has no column of its own; its members are spread into
columns. Its `@ field` label gives those members context: a label written for a
type or for a type's field reads `<record field label>: <member label>` below a
labelled record field.

```runa
# Date(@ field(label = "year") year: Int, @ field(label = "month") month: Int, @ field(label = "day") day: Int)
# Trade(@ field(label = "Trade date") date: Date, @ field(label = "Event id") id: String)
```

For `trades: List(Trade)`, `trades.date.year` is labelled `Trade date: year`.
Only the label of a record field without a column is used, except for a
`Date`: a date is one fact, so the contract also keeps the date field's own
metadata (label, question, help) at `trades.date`, where a need for it is
asked.

### Metadata for one calculation

When metadata depends on where a type is used, such as help that holds for one
calculation only, attach typed field records to the entry with
`@ meta`. A field binding is an ordinary pure ground record. Its typed field
shape and canonical `path` give it calculation meaning; `@ meta` stands on the
entry, and its value contains the fields and their provenance.

```runa
# UseSiteField(path: String, label: String, question: String?, help: String?, unit: String?)
# SourceInfo(url: String, section: String)
# EntryMetaRole(a) = Source(value: a)
# EntryMeta(a) = EntryMeta(fields: List(UseSiteField), attachments: a)
# impl MetaRole for EntryMetaRole {}
# impl Meta for EntryMeta {}

= tax_source = SourceInfo(
    url = "https://example.invalid/tax",
    section = "income"
)
= monthly_income_field = UseSiteField(
    path = pathof(Input::monthly_income),
    label = "Monthly income before tax",
    question = Some("What do you earn before tax each month?"),
    help = Some("Use the gross amount before deductions."),
    unit = Some("currency/month")
)
= monthly_income_meta = EntryMeta(
    fields = [monthly_income_field],
    attachments = (
        Source(value = tax_source),
    )
)

@ meta(monthly_income_meta)
@ entry
> calculate_tax(input: Input) -> Int { ... }
```

`path` uses the exact canonical path emitted by the calculation layout. Root
fields use paths such as `monthly_income`; a field in a related list table uses
a path such as `children.age`, while `children` can describe the collection
itself. `input.monthly_income` and `TaxInput.monthly_income` are accepted and
normalized to the canonical path. Unknown paths and duplicate metadata for one
path are errors. A required record field such as `trades.date` is a valid path
too; only its label is used, as context for its members.

Prefer `pathof(InputType::field::nested_field)` for these paths. It is checked
against the declared or plainly imported type graph during `runa check`, has
type `String`, and lowers to the same canonical string stored in the contract.
`List`, `Set`, optional, and string-keyed `Map` value traversal is transparent:
`pathof(Input::children::age)` lowers to `"children.age"`. Sum types require an
explicit constructor segment, such as
`pathof(Input::income::WageIncome::amount)`. Their discriminator uses the
terminal form `pathof(Input::income::$variant)`. A misspelled segment is
reported at that segment. Literal paths remain supported for generated data and
backward compatibility.

Typed records can also follow a nested domain type wherever it occurs: declare
the record's `path` field as `ProgramReference`, use `refof(Type::member)`, and
write `@ meta` on the type, or `@ meta(child_meta) for Child` in a file of its
own when the metadata is kept apart from the type (see [Metadata written
apart](style.md#metadata)); next to the type, prefer `@ field`.

```runa
# ChildField(path: ProgramReference, label: String)
# ChildMeta(fields: List(ChildField))
# impl Meta for ChildMeta {}

= child_meta = ChildMeta(fields = [
    ChildField(path = refof(Child::age), label = "Child age")
])

@ meta(child_meta)
# Child(age: Int)
```

For an input containing `primary: Child` and `children: List(Child)`, this one
declaration produces `primary.age` and `children.age`. Projection crosses
records, alternatives, lists, sets, and string-keyed map values.

For each input path the most specific metadata wins, whole; keys are not
merged:

1. an exact path for this calculation: a `String` or `pathof(...)` path, or
   `refof(...)` rooted at the calculation input;
2. a type's field wherever the type occurs: `@ field` on the field, or
   `refof(Type::field)` projected metadata;
3. a type's default: `@ field` on the type.

Two declarations at the winning level for one path are an error naming both;
`@ field` on a field and `refof` metadata for that field are the same level.
The design and its reasons are in [Field metadata on
declarations](../rfcs/field-metadata.md).

Members of optional composites are metadata targets too, at any depth:
`taxpayer.spouse.cohabiting_at_year_end`,
`pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end)`,
`refof(Spouse::cohabiting_at_year_end)` projected through `spouse: Spouse?`, and
`@ field` on `Spouse`'s `cohabiting_at_year_end` name the same member, with the
same precedence and duplicate rules. A path into a member the type does not
declare is an error. The optional field itself stays one input field and can
carry its own metadata.

The record must use named fields `path`, `label`, `question`, `help`, and `unit`.
`path` is a `String` for exact metadata or a structural `ProgramReference` for
reusable metadata, and `label` is a required string. The other fields may be
strings, optional strings, or omitted by a record type that does not declare them.
Role-bearing values such as `Source` attachments remain typed metadata and are
copied into that field's source trace.

When the field's type is a declared [unit](runes.md#unit), its `unit` text comes
from the declaration's description; an authored `unit = Some(...)` for that
field is an error. See [Unit types](#unit-types).

Scale the same ordinary typed aggregate to any number of fields and attach that
object once with the `meta` role. The calculation consumer finds field-shaped
typed descendants recursively; other metadata consumers remain free to query
the same object by its own nested types.

```runa
= tax_input_meta = EntryMeta(
    fields = [
        monthly_income_field,
        filing_status_field,
        children_field
    ],
    attachments = (
        Source(value = tax_source),
    )
)

@ meta(tax_input_meta)
@ entry
> calculate_tax(input: Input) -> Int { ... }
```

Typed metadata aggregates compose hierarchically. A role-bearing attachment on
an outer aggregate applies to every calculation field below that aggregate. An
attachment inside a nested aggregate applies only to fields below that nested
aggregate. This lets a calculation attach one root metadata object while each
domain component retains its own source, warning, and guidance trace.

The metadata value may live in a recursively imported metadata module, and
`@ meta(<value>) for <name>` may attach it from such a module. Calculation
tooling collects only imported metadata on an actual entry or a
reachable input type, so unrelated dependency metadata cannot alter the
contract. Imported fields use the same exact-path validation and duplicate
checks as local fields.

The emitted field binding names include stable aggregate paths such as
`tax_input_meta.fields[0]`. A `@ meta` value without calculation fields is
simply ignored by the calculation field consumer. Nested variants of types
implementing `MetaRole` are copied into the calculation and field source traces.
Each trace keeps the canonical source `binding` (`""` for a value written
inline). The calculation's `metadata` list is sorted by label, role, binding
and value, without duplicates. Where in an aggregate a value sits is
reported by `runa meta` (`path`), not by the contract.

Field metadata appears structurally in `runa schema` and participates in the
schema fingerprint. In XLSX, the human label is the visible column header; the
exact canonical path and all presentation data remain in the hidden `_columns`
or `_tables` sheet. A field without explicit metadata receives a deterministic
humanized path as its visible fallback. When columns of one sheet share a
label (a type with `@ field` held at two paths), each is qualified by the
humanized ancestors where its path parts from the others'
(`Spouse / Business use`); a column with no such ancestor keeps the bare label,
and columns the ancestors cannot tell apart add their canonical path. A label no
other column of the sheet carries stays as it is, and workbooks written with
the bare shared labels still read. Header notes always expose the canonical
path and add the interview question, help, unit, and source bindings when
present. None of this prose changes requiredness, alternatives, validation, or
rule evaluation.

An AI client can therefore present the calculation title, ask each field's
`question`, map the answer to `path`, and submit canonical JSON or XLSX. The AI
gathers facts and explains the returned rule trace; Futuruna remains the
deterministic calculator. Futuruna does not parse PDFs or infer facts from source
documents: the AI or a human reads those documents and fills the generated
workbook. A visible machine path in a generated workbook means that field still
lacks explicit presentation metadata; it is a stable fallback, not the intended
final interview wording.

## Inspect The Contract

```sh
runa schema model.entry.runa
runa schema model.entry.runa --entry calculate_tax
```

The command emits `futuruna.entry.v1` JSON with input and output types,
reachable type definitions, linked metadata, and a SHA-256 schema fingerprint.
Use `--entry` when a file declares more than one calculation.

For models with repeated field-source provenance, opt into a lossless compact
schema export:

```sh
runa schema model.entry.runa --format compact-json --output schema.json
```

The distinct wire schema `futuruna.entry.compact.v1` stores each exact source
object once in `source_objects`, ordered source lists in `source_groups`, and
a `source_group` reference on each field that has sources. Source order,
repetitions, text, structured data and field associations are preserved. Root
metadata and type definitions are unchanged. `contract_schema` identifies the
expanded logical schema; `schema_hash` remains the fingerprint of that complete
expanded contract, so templates and workbooks retain the same fingerprint.

Default output and `--format json` remain expanded JSON. Compact schema JSON
is not a calculation input envelope or workbook adapter format. Exporting it
does not shrink the compiler's internal contract cache or workbook columns.
The Rust reader `calculate::compact::expand_compact_json` rejects duplicate and
unknown keys, corrupt or dangling references and inconsistent fingerprints.
Its required byte budget limits repeated serialized field-source bytes before
expansion, not total parsing or process memory. These hashes check consistency,
not source authenticity: consumers must still compare with a trusted model's
expected fingerprint.

Linked metadata may reference ground bindings in the calculation file or in a
recursively reachable plain import. The contract includes stable metadata type
and value data in its fingerprint. Definition file paths and line numbers stay
in `runa meta --json` and do not make a portable calculation contract
machine-dependent.

### Unit types

Experimental. A field, output or other type position whose type is a declared
[unit](runes.md#unit) keeps its primitive type and gains an additive `unit`
member naming the unit, and the contract lists the units it uses with their
descriptions and, for a unit declared with a scale (`= DKK / 100`), the scale:

```json
"type": { "kind": "primitive", "name": "Int", "unit": "Ore" }
```

```json
"units": [{ "name": "Ore", "description": "DKK øre", "scale": { "of": "DKK", "per": 100 } }]
```

`per` of the unit make one `of`: an `Ore` value of `12345` is `123.45` DKK.
`of` is the base the declaration names, or `"1"` for a plain number
(`= 1 / 1000000`); a unit without a scale has no `scale` member. Two units with
the same `of` convert exactly by the ratio of their `per`.

Values on the wire stay JSON integers, so consumers that ignore unknown members
read the same contract as before. The field metadata of a unit-typed field takes
its `unit` text from the declaration (`"DKK øre"`); writing `unit = Some(...)`
in the metadata of such a field states the unit a second time and is a check
error.

### Named scalar types

Experimental. A field, output or other type position whose type is a declared
[named scalar](runes.md#named-scalar) keeps its primitive type and gains an
additive `scalar` member, and the contract lists the named scalars it uses
with their descriptions and checks as written:

```json
"type": { "kind": "primitive", "name": "String", "scalar": "Isin" }
```

```json
"scalars": [{ "name": "Isin", "of": "String", "description": "International Securities Identification Number", "checks": ["isin_shape", "isin_check_digit"] }]
```

A `Map` may be keyed by a named scalar over `String`. Values on the wire stay
JSON strings and numbers. `runa call` (also `--partial`) runs the checks on
every input value at a position of a named scalar type: a field, a list or set
element, a map key or value, a variant payload and a present optional value.
A value that fails is an input diagnostic at its path, reported with the
case's other input errors, and the case does not run:

```json
{"case_id": "demo", "path": "$.events[3].isin", "message": "`\"DK006053491\"` is not an `Isin`: `isin_shape` is false", "scalar": "Isin", "check": "isin_shape"}
```

In a partial case a missing value is still a need; a present value that fails
is an input error. `runa template --input` runs the same checks on the values
it carries over and refuses a value `runa call` would refuse, with the same
message at the same path; a placeholder is not checked. Results are not checked: every value of a named scalar was
checked where it was made. The type's `@ field` is the field metadata of every
input field that holds it, unless the field or an exact path says otherwise.

### Inspect one input branch

For a large contract, the repository's read-only Python 3 helper can inspect a
fresh compact export without printing every field or expanding every source:

```sh
python3 scripts/calculation-input-guide.py schema.json
python3 scripts/calculation-input-guide.py schema.json lønmodtager
python3 scripts/calculation-input-guide.py schema.json lønmodtager.bruttoløn_kroner
python3 scripts/calculation-input-guide.py schema.json 'ægtefælle.$variant'
```

The first command lists the input's immediate children. Follow a returned `path`
to see its type, immediate children, explicit field metadata and referenced
source objects. Unannotated fields remain visible with
`has_explicit_metadata: false`; missing help does not mean an optional or
irrelevant field. Selected ancestor metadata, including branch-discriminator
guidance, stays in the view. Source-group order, repeated references and source
objects are preserved, not summarized into new legal claims.

`variant_guards` describes which alternative a payload belongs to; inspection
does not select that alternative for a person. `containers` identifies lists,
sets, string-keyed maps and optional values along the path. Collection paths
describe each element, not a particular row. Optional descendants apply only
when their enclosing value is present; their explicit metadata is listed like
any other field's, while workbook optional composites still use one JSON cell.
These are schema-navigation paths, **not JSON Pointers into case data**. Use the
normal generated template and input encoding when supplying facts.

The default page contains at most 20 children. Follow `page.next_offset` with
`--offset N` until it is `null`; `--limit` accepts 1–100. Whole-calculation
metadata is separate, not silently treated as field-local or discarded:

```sh
python3 scripts/calculation-input-guide.py schema.json --root-metadata --limit 10
python3 scripts/calculation-input-guide.py schema.json --root-metadata --limit 10 --offset 10
```

Review relevant root metadata and the model's coverage guide as well as field
guidance. A focused view is not evidence that unrelated conditions are satisfied.
The helper accepts regular compact-schema files up to 32 MiB and refuses output
over 1 MiB instead of truncating it; choose a deeper path or smaller page. It
rejects duplicate JSON keys, missing source references and nonexistent paths,
and preserves exact integer metadata. Positional constructor payload navigation
is explicitly unsupported; inspect its original type definition instead.

This repository helper is not a new `runa` command, input adapter, completeness
check or document importer. It does not authenticate the reported fingerprint,
check legal correctness, execute rules, fill placeholders or write files. Its
inspection output may evolve and cannot be submitted to `runa call`. Generate
the schema locally from the trusted current model; `runa call` remains the
authority for contract and case validation.

## Reuse Validated Contracts

`schema`, `template`, and `call` persist successful contract validation in a
dependency-validated local cache. Its source-graph snapshot includes the root
source, every transitive plain, qualified, and content-hash import, manifest
resolution contexts, resolved import edges, prelude mode, cache format, and the
exact compiler executable. Warm validation hashes the recorded files directly
and rechecks each import target instead of reparsing the graph. An edit anywhere
in that graph, a manifest change, a newly shadowing module, or a compiler rebuild
therefore causes a miss. Parse or type errors are never cached, and a corrupt
entry is ignored and rebuilt.

The default cache is under the operating system's user cache directory. Set
`FUTURUNA_CALCULATION_CACHE_DIR` to choose another root, set
`FUTURUNA_DISABLE_CALCULATION_CACHE=1` for an uncached validation run, or set
`FUTURUNA_CALCULATION_CACHE_TRACE=1` to report `hit`, `miss`, or `disabled` on
standard error. The cache contains contracts only; calculation inputs and
results are not stored.

A call prepares its entry once: the runtime rule-dispatch metadata
and an interpreter that has initialized the program (the prelude, the
declarations, and the top-level and metadata bindings the entry reaches). Every
case runs from that prepared state and starts from it alone: a fresh
environment over the initialized bindings, with the handler, rule-scope,
step-budget, and random state initialization left. A case that sends to an
actor or emits on, subscribes to, or tears down a subject the initialized
program holds, or that panics, has the program initialized again before the
next case, so no case sees the messages or values of another. A host that keeps
the prepared entry reuses it for every later call: the WebAssembly engine's
`Model` prepares an entry on its first call (full or partial), and later calls
run only their cases, with results byte-identical to a fresh call's. In Rust,
`futuruna::calculate::PreparedEntry` is that prepared entry.

Calculation batches with at least four ready cases run independent cases
concurrently and retain their original result and diagnostic order. The
prepared interpreter serves one worker; each further worker initializes its own
interpreter for the batch, under the same per-case isolation. Set
`FUTURUNA_CALCULATION_JOBS=1` for a serial parity or debugging run, or set an
explicit positive worker count. Zero and an unset value select the available
machine parallelism, capped at eight workers by default.
`FUTURUNA_CALCULATION_TRACE=1` reports per-case timings, the selected worker
count, initialization time (the time until the entry was prepared, close to
zero when it already was), and total calculation time on standard error.

For repeated work on a large calculation corpus, `scripts/runa-corpus.sh`
builds and reuses an optimized `target/corpus/runa` without changing the normal
debug profile. `scripts/test-corpus.sh` builds and runs selected Rust tests under
the same persistent optimized profile. This keeps ordinary compiler edits quick
to rebuild while making repeated schema, workbook, and calculation gates
substantially faster.

## Audit Calculation Reachability

```sh
runa audit model.entry.runa --entry calculate_tax
runa audit model.entry.runa --entry calculate_tax --json
```

Entry-specific audit mode reports the conservative runtime-symbol closure used
to initialize that calculation. The versioned
`futuruna.audit.reachability.v1` JSON contains required top-level bindings and
the loaded source graph together with registered global rule, function, method,
and RuleScope families carrying a `reachable` or `not_reached` status. This
makes disconnected modules and implementation families machine-queryable
without executing scenario proofs.

The report deliberately works at rule-family granularity. When a RuleScope is
reached, all members of that scope are marked reachable because the scope is the
encapsulation boundary. A `not_reached` item is a review candidate, not a proof
that dynamic or external invocation is impossible. Consumers should confirm a
candidate against the public input contract and source model before removing or
connecting it.

`runa audit FILE --json` works for every file. It also carries the structural
audit as `topology` (`summary` counts of rules, invariants, `contradictions`,
`tensions`, `asymmetries` and `invariant_gaps`; `findings` with `kind`,
`severity`, `title`, `rules` and `explanation`). For a file without
`@ entry`, the entry sections (`entry`, `label`, `input_type`,
`output_type`, `analysis`, `summary`, `reachable_symbols`, `required_bindings`,
`loaded_sources`, `callables`, `not_reached_candidates`) are `null`. With
`--json`, a file with several calculations needs `--entry`. Running `runa audit` without
`--entry` or `--json` prints the topology audit for invariant gaps,
asymmetries, and tensions.

Every mode also reports clause reachability and gaps (see
[Auditing clause reachability](runes.md#auditing-clause-reachability)). With a
selected calculation (the file's only one, or `--entry`) it covers every rule
family the entry reaches, imports included, plus the families with a clause in
the audited file; otherwise the families with a clause in the audited file. In
the JSON report it is the `clause_reachability` object: `status` is `ran` or
`not_run` (with a `reason`, for example a missing Z3); `scope` is
`{"kind": "reached_from_entry", "entry": …}` or
`{"kind": "declared_in_file", "file": …}`; `families` lists every checked
family with its `files` and number of `clauses`; `summary` counts `families`,
`clauses`, `files`, `analyzed`, `not_analyzed`, `overlap_not_checked`,
`gap_not_checked`, `unreachable_clauses`, `overlapping_guards`, and `gaps`, so
a CI check can require that something was checked; each entry of `findings`
has a `file` (that of the reported clause) and a `kind` of
`unreachable_clause` (with `cause`
`shadowed` or `unsatisfiable_guard` and the clauses that take its inputs in
`shadowed_by`), `overlapping_guards` (with the earlier, selected clause in
`overlaps`), or `gap` (with every clause of the family, in dispatch order, in
`clauses`). An overlap or gap carries a `witness` mapping each argument, and
each RuleScope field as `self.<field>`, to a solver value in source spelling
(`null` when the solver named none). Every clause location carries `file`,
`line`, `tier` (`exception`, `guarded_default`, `clause`, or
`unguarded_default`), and `source`. `not_analyzed` and `overlap_not_checked`
list clauses with the reason the solver encoding could not decide them;
`gap_not_checked` lists families (`family`, `reason`) whose coverage was not
decided, for example because one of their guards is not translatable. Findings
are informational and do not change the exit status of `runa audit`.

## Generate Input

```sh
runa template model.entry.runa --format json --output cases.json
runa template model.entry.runa --format toml --output cases.toml
runa template model.entry.runa --format xlsx --output cases.xlsx
```

JSON is the canonical value model. TOML omits absent optional record fields.
Generated cases start with `input_status = "draft"`. After reviewing the supplied
facts, explicitly set the case's status to `ready` before calling it. This
acknowledges intentional inputs, including zeros, empty collections and known
unknowns; it does not authenticate documents, establish completeness or certify
the model. A ready case can still fail ordinary input or model checks.

Every value the user must supply is written as a placeholder object such as
`{"$fill": "Int"}` or `{"$fill": "FilingStatus: one of Single, Married"}`;
only structure fixed by the contract (record fields, a sole variant) is spelled
out. A placeholder is never a valid value, so `runa call` refuses a case that
still contains one and lists every unfilled path, for example
`$.cases[0].input.monthly_income`, even when the case is marked `ready`.
Optional values and collections are placeholders too: write `null`, `[]` or
`{}` deliberately. `$fill` is reserved and cannot be used as a map key in
calculation input. XLSX templates leave unfilled cells blank; a blank required
cell is rejected, and converting a workbook with `template --input` keeps blank
required cells as placeholders. Spreadsheet collections are supplied as rows, so
a collection sheet without rows is an empty collection.

Changing some values does not automatically mark a case ready. Review
the status again after editing facts; offline files cannot detect whether a
previous review still applies. JSON/TOML carry the status alongside `case_id`,
outside the model's `input` record.

XLSX flattens nested named records into columns, gives booleans and nullary enums
constrained choices, expands finite payload alternatives into a `$variant`
choice plus typed variant-qualified columns, and puts each `List`, string-keyed
`Map`, or `Set` field in a separate related worksheet when it is reachable in a
supplied case. Inactive alternatives and descendants of empty collections do
not create sheets. A reachable empty collection still gets its entry sheet.
Integer template cells are text-formatted so all `i64` values remain exact.
String cells use `""` for intentional empty text; a cleared cell is absent.
Text starting with a double quote uses JSON string quoting. This also applies
to string values in collections and to map keys, including an empty key.

In new workbooks, enter `""` (two double quotes) for an empty string. Clearing
a cell still means an absent optional value or a missing required value.
Ordinary text stays unquoted. Text starting with a double quote uses JSON
string quoting: for example, `"\"quoted\""` represents the text `"quoted"`,
including its quotation marks. Generated templates apply this encoding
automatically, including list/set elements, map values, and map keys.
Input workbook version 8 records this distinction; versions 6 and 7 remain
readable with their original literal-text behavior. Refreshing an older
workbook writes version 8 without changing its decoded values.

`cases` is the first visible worksheet and contains scalar fields for the named
input record. `input_status` beside `case_id` offers `draft` and `ready`; it
applies to that case's inputs on all related sheets. Its first row visibly
shows the `@ entry` title, and every related collection sheet combines that
title with the collection's field label.
Explicit field labels replace machine paths only in visible headers; hidden
topology retains the paths. Every collection row
uses `case_id` and `item_id`; nested collection sheets add `parent_id`. List rows
use one-based `position`, map rows use `key`, and set rows have neither. Leave a
collection sheet without matching rows to supply an empty collection. Hidden
`_futuruna`, `_tables`, and `_columns` sheets record the contract fingerprint,
complete topology, and column types; `_sheets` records the collection sheets
actually generated. Do not edit these sheets or delete generated entry sheets.
Optional composite fields
and recursive or opaque leaves remain canonical JSON cells when they cannot be
expanded to a finite unambiguous layout. An optional composite's cell note lists
the label and question of each member with metadata. `_columns` records those
members' metadata in rows after the column rows: encoding `member`, no sheet,
the cell's `path`, and the member's `input_path`. Members never become columns
of their own. Cells and child rows belonging to an
inactive alternative are rejected.

When an AI or person fills a generated workbook, resolve `$variant` choices
before asking for their payload fields. Variant-qualified columns describe what
the contract can accept; they are required only when their alternative is
selected. Do not infer requiredness from a column merely being visible.

After changing a variant or adding parent rows, refresh the workbook to add any
newly reachable collection sheets:

```sh
runa template model.entry.runa --input cases.xlsx --output refreshed.xlsx
```

Refresh validates and preserves existing values and readiness, except that cases
needing newly reachable collection sheets return to draft. Those sheets start
empty, not as confirmed empty collections. Complete and review them before
calling the calculation. It does not invent required scalar facts: missing or
invalid payload fields must first be corrected. A normal `call` rejects a case
needing an ungenerated sheet instead of silently assuming an empty collection.
Deleted declared sheets, stale contracts, and malformed data fail refresh too.
Missing readiness in an older input becomes `draft`, never automatic approval.
Item IDs are adapter-local and may change during refresh; parent links are
rebuilt consistently.

JSON and TOML envelopes can also hydrate a workbook through `template --input`.
The generated sheets are the union needed by all supplied cases. Use
`template --all-tables --output complete.xlsx` only when you explicitly need
every collection sheet, including inactive alternatives. Scalar columns and
hidden contract metadata remain complete in both modes. This reduces worksheet
count, not the width of the root input sheet or the size of the contract itself.

New JSON/TOML templates use input envelope v2 and workbooks use input adapter v9.
Input v1 and complete v6/v7/v8 workbooks are readable, but cases without
`input_status` do not run: generate a fresh template, review the facts and mark
intentional cases ready. Do not rewrite the version or fingerprint to bypass
this.

Every template records the entry and schema fingerprint. A source type change
makes an old template stale; invocation reports the expected and actual hashes
instead of coercing the data.

## Invoke Cases

```sh
runa call model.entry.runa --input cases.json
runa call model.entry.runa --input cases.toml --output results.json
runa call model.entry.runa --input cases.xlsx --output results.xlsx
```

Each case is decoded through the same Futuruna contract and evaluated in an
isolated interpreter. Valid cases can produce results even when another case has
a diagnostic. The command exits unsuccessfully when any diagnostics remain.
Draft and unmarked cases produce readiness diagnostics without invoking the
calculation. An entirely unready batch does not initialize calculation workers;
ready cases in a mixed batch still run, with results and diagnostics retaining
their input order.

JSON and TOML cases report independent missing, unknown, and invalid fields
together, including fields inside collection elements and selected variants.
An invalid parent shape or unknown variant produces a diagnostic at that
boundary; the decoder does not guess child fields. Invalid cases are never
invoked. Diagnostics remain in case order with one or multiple workers, and
template hydration reports input errors before writing a workbook.

`call` rejects undefined calculation values: integer division/remainder by zero,
integer overflow, non-finite floating-point intermediates, failed assertions,
out-of-bounds list access, and unmatched value rules produce case diagnostics,
not substitute zero or empty values. Known Boolean predicate misses still return
`False`, including scoped predicates with captured fields; an error while
evaluating a predicate is a diagnostic, not `False`. Required top-level
initialization is checked too; if it fails, no case receives a result from it.
An unexpected recoverable runtime panic invalidates that worker, which is
reinitialized before another case. The boundary does not guarantee recovery
from process termination or resource exhaustion.

Calculation workbooks must be `.xlsx`. VBA projects and formulas in the input
sheet are rejected. Before any sheet is read, a workbook may unpack to at most
256 MiB in at most 10,000 archive entries, and each sheet's used range may span
at most 4,000,000 cells; larger workbooks are rejected with an error. JSON input documents and canonical JSON workbook cells
reject duplicate object member names at every depth, including escaped names
that decode to the same key. This also applies to template hydration: ambiguous
values are never normalized by choosing the first or last occurrence.

Unknown columns, duplicate or empty case identifiers,
duplicate item identifiers, list positions, or map keys, orphaned parent rows,
non-exact integers, invalid enum choices, missing required fields, and malformed
canonical JSON are rejected before their case runs. A bad related row invalidates
that case while other valid cases still run.

An XLSX result contains a compact `results` summary, a lossless
`result_values` table, and case-scoped `diagnostics`. `result_values` uses RFC
6901 JSON Pointer paths and canonical JSON scalar text; long values occupy
ordered chunks rather than being truncated at Excel's cell-size limit.

## Partial cases and needs

Experimental. `runa call --partial` evaluates cases that leave facts out and
answers each with its result or with the facts it still needs. A client
answers some needs, writes the answers into the same case at their paths and
calls again, until the case is `Complete`.

```sh
runa call model.entry.runa --partial --input partial.json
runa call model.entry.runa --partial=strict --input partial.json
runa call model.entry.runa --partial=check --input partial.json
runa schema model.entry.runa --format partial-result
```

In a partial case a value means the answer, `{}` for a record means the
record exists with its fields not known yet, `null` for an optional field
means "none", and a field left out is unknown. A sum type given as `{}` or
left out is unknown at `path.$variant`. Readiness, unknown fields, wrong
types and placeholders are checked as in complete cases. Partial cases are
JSON: a blank cell or omitted TOML key cannot tell "none" from "not asked
yet".

Each case's `result` is one of:

```json
{ "$variant": "Complete", "result": { "tax": 250 }, "unused_paths": ["phone"] }
```

```json
{ "$variant": "Needs",
  "needs": [
    { "kind": "fact",
      "path": "accounts[1].closing_ore",
      "keys": [ { "path": "accounts[1]", "field": "account_id", "value": "ASK-1" } ],
      "label": "Closing balance",
      "question": "What was the balance on the closing day?",
      "unit": "øre",
      "sources": [ { "label": "§ 1", "role": "Source", "binding": "law" } ],
      "waiting": [
        { "rule": "account_tax", "section": "taxation",
          "paragraphs": [ { "paragraph": "§ 13",
                            "source": "sources/law.txt",
                            "act": { "title": "Bekendtgørelse af aktiesparekontoloven",
                                     "identifier": "LBK nr. 281 af 13/03/2025",
                                     "url": "https://www.retsinformation.dk/eli/lta/2025/281" } } ] } ] } ],
  "later": [ { "kind": "fact", "path": "accounts[1].closing_day", "waiting": [ "..." ] } ] }
```

`unused_paths` lists the provided input paths evaluation never read. A
need's `path` is concrete, with list indices; its `label`, `question`,
`help`, `unit` and `sources` come from the field metadata of the same path
without indices. A `Date` is one fact: when its year, month or day is
unknown, the need is at the date's path, with the date field's own metadata,
and is answered with the whole date. `keys` names the list elements the path
lies in by their key, outermost first, when the element's type has a key
field (`@ field(label = "ISIN", key = True)`) that the case gives. A record
declares one key field; a sum type declares it in every variant, with the
same name and type (an event's `id` in each kind of event), so that an
element is keyed whatever its variant.

`waiting` lists the rules that wait for the value: of every rule and function
evaluation was in when it read the value (or a value computed from it), the
innermost one declared in a law section (an `@ section` that quotes a
document) that cites paragraphs; when none cites, the innermost of a law
section; else of any section (one that quotes nothing, such as a section of
input checks); else the innermost rule. Cited rules are listed alone when
there are any, each with its `section` and its `paragraphs`; a paragraph names
the file of its quoted document (`source`) and that document's `act`, read
from the header block at the top of the file (`Title:`, `Document
identifier:`, `Human-readable URL:` or `URL:` between `---` lines). A passage
cited `as amended` also carries `as_amended_by` (`act`, `paragraph`,
`instruction`): the amending act (its file's `Document identifier`, else the
document's `identifier`), its paragraph and instruction number, and the
instruction line.

`needs` are the facts that block the result as it stands; answering them can
reveal more, since evaluation does not look past an unknown condition it
cannot finish. `later` lists the facts evaluation did read behind such a
condition (in a branch that failed, was cut, or was undecided because of
another branch) and that are not needs: they may become needs once the needs
are answered. The output envelope's schema is
`futuruna.entry.partial-output.v1`, and `runa schema --format
partial-result` prints the JSON Schema of `result`.

Evaluation carries unknown values freely and decides wherever the known part
decides: `False && unknown` is `False`, an `if` or `match` whose possible
branches give one value is decided, and a rule whose clauses that might fire
all agree is decided. A clause with an unknown guard blocks the clauses below
it. Speculation (what runs only because a condition is unknown) is bounded:
after 100 000 steps per case, or at a recursive call under an unknown
condition, the result is unknown with the condition's paths. An operation
that may fail for some answer (division by an unknown, a rule that might
match no clause) keeps the case from completing until its facts are known.

A field another calculation provides carries `entry` and `subject` in its
metadata:

```runa
# TaxCase(
    taxpayer: Taxpayer,
    @ field(label = "Spouse's share income", entry = refof(share_income), subject = pathof(TaxCase::taxpayer::spouse))
    spouse_share_income_ore: Int
)
```

In a field record, the record declares `entry: ProgramReference?` and gives
`entry = Some(refof(share_income))`, `subject =
Some(pathof(TaxCase::taxpayer::spouse))` (`pathof` in a `String?` place stays
the path string). Its need has kind `entry`: the client runs that entry for
the subject and writes the entry's result at `path`. The contract and the
need carry the entry's name and the subject's input path
(`"entry": "share_income"`, `"subject": "taxpayer.spouse"`). `runa check`
reports, naming the field: an `entry` that is not an `@ entry` of
the program, a `subject` rooted at another type than the calculation's input,
a subject whose value (through optional, list, set and map wrappers) does not
have the entry's input type, and a field whose type is not the entry's result
type.

The string forms (`entry = "share_income"`, `subject = "taxpayer.spouse"`, and
`Some("...")` in a field record whose `entry` is a `String?`) are still read,
with the same checks. `runa fmt --migrate` rewrites them as `refof`/`pathof`
when exactly one calculation input reaches the field (that type is the root
of the `pathof`; a field record's `entry` type becomes `ProgramReference?`)
and lists the others with the reason, as `file:line: left ...`.

`--partial=strict` stops each case at the first unknown value it reads. Its
`Complete(r)` holds by construction and serves as the test oracle.
`--partial=check` runs both modes and the completion test: strict
`Complete(r)` must be propagating `Complete(r)`, and every generated
completion of a `Complete(r)` case must return `r` or stop with an
arithmetic-overflow error; each violation is a diagnostic. If a partial case
returns `Complete(r)`, every completion of its unknown paths returns `r` or
stops with an overflow error; no completion returns a different result.

Partial evaluation is an interpreter feature of `runa call`. Native and
compiled programs take complete input; `--partial` is refused elsewhere.

The full format and evolution contract is in
[Typed calculation contracts](../rfcs/typed-calculation-contracts.md). Partial
cases and their needs are specified in
[Calculation needs](../rfcs/calculation-needs.md).

## Needs in the language

Experimental. A program asks an entry about a case that holds
facts not known yet, in Futuruna, and a test states an interview round by
round. Queries run in the interpreter, as `runa call --partial` does.

```runa
# Spouse(income: Int, cohabiting: Bool)
# Household(income: Int, married: Bool, spouse: Option(Spouse))
# Result(tax: Int)

@ section transfer {
| threshold(h: Household) -> match h.spouse {
    | Some(s) -> if s.cohabiting { 200 - s.income } else { 100 }
    | None -> 100
}
}

@ entry
| household_tax(h: Household) -> Result(tax = h.income - threshold(h))

= with_spouse = Household(income = 500, married = True, spouse = Some(Spouse(income = 50, cohabiting = True)))
= first = query household_tax(Household(income = 500, married = True, spouse = unknown))
= asked = answer(household_tax, Household(income = 500, married = True, spouse = unknown), whole_case_guide(), from_case(with_spouse))

@ print(match first { | Needs -> map(first.needs, |n| n.path) == [pathof(Household::spouse)] | Complete -> False }) -- true
@ print(map(asked.rounds, |r| r.paths) == [[pathof(Household::spouse)], [pathof(Household::spouse::cohabiting), pathof(Household::spouse::income)]]) -- true
@ print(asked.outcome == Answered(result = household_tax(with_spouse))) -- true
```

**`unknown`** is a value not known yet. It means what a left-out field means
in a partial JSON case: for an optional, a scalar or a list it is one unknown
value; for a record, the record exists with each field unknown; for a sum
type, the variant is unknown. `Some(unknown)` of a record says "present,
nothing known yet". `unknown` takes the type of its place, as a number
literal takes its unit, so it stands only where a place gives it a type:

| Placed (accepted) | Read (a check error) |
|---|---|
| an argument: `case_of(unknown)` | an operand: `unknown + 1` |
| a field value: `Case(income = unknown)` | a side of a comparison: `unknown == 3` |
| a list element: `[1, unknown]` | a scrutinee: `match unknown { ... }` |
| a typed binding: `= later: Int = unknown` | a guard or condition: `\| n if unknown -> n`, `if unknown` |
| a declared result: `> later() -> Int { unknown }` | the argument of a builtin: `show(unknown)` |
| a branch: `if c { 1 } else { unknown }` | an untyped binding: `= x = unknown` |

An unknown scalar as a list element, a map value or the payload of `Some`
cannot be written in a partial case, so a query of such a case stops with an
error at the query's line. A builder that reads an `unknown` before a query
places it stops with an error naming the `unknown`'s line. `unknown` is a
reserved word; `runa fmt --migrate` renames a binding, parameter or function
called `unknown` to `unknown_`.

**`query entry(case)`** asks the entry `entry` (an
`@ entry` declared in the asking file or a file it imports) about
`case`. It returns

```text
Query(o, i) = Complete(result: o, unused: List(Path(i)), assumptions: List(QueryAssumption))
            | Needs(needs: List(Need(i)))
```

with what `runa call --partial` returns for the case: the entry is checked
and prepared as `runa call` prepares it, the case crosses as a partial JSON
case, and the result and needs cross back. `query` is a contextual keyword:
only `query name(...)` is a query, and `runa check` rejects a query of a
name that is not an entry. No calculation may reach a `query` or
an `unknown`: `runa check` reports one that any `@ entry` can
reach, with the call chain.

A `Complete` carries the assumptions its result rests on, as `runa call`
does: a **`QueryAssumption`** has `section`, `rule` and `binding`, each an
`Option(ProgramReference)`, so a test states which assumption a case rests on
with a checked name: `any(q.assumptions, |a| a.binding ==
Some(refof(assumption_closing_day)))`.

A **`Need(i)`** has the members of a need in the partial result: `kind`
(`FactNeed` or `EntryNeed`), `path: Path(i)`, `keys: List(NeedKey(i))`,
`entry: Option(ProgramReference)`, `subject: Option(Path(i))`, `label`,
`question`, `help`, `unit`, `sources: List(NeedSource)` and `waiting:
List(NeedWaiting)`. A waiting rule's `section` is a
`ProgramSectionReference`, so a test compares it with
`Some(refof(section transfer))`; `refof(section name)` names a section of the
file it stands in or of a file it imports, and a name two files declare is a
check error. These types and `fill` join the prelude of a program that asks
a query, places an `unknown`, or calls `fill`; the types of
[Answering an entry](#answering-an-entry) join it as well when the program
builds an `AnswerGuide` or calls `answer`.

**`Path(i)`** is a path into a value of type `i`. `pathof(T::...)` is a
`Path(T)` where a path is expected and a `String` elsewhere; it takes list
indices (`pathof(TaxCase::accounts[1]::closing_cash_ore)`) and names the
alternative of a sum type it passes through
(`taxpayer.outside_endavu.OutsideDeclared.other_share_income_ore`), as field
metadata does. Paths of one root are equal when they name the same place;
comparing a path with a path of another root type is a check error. A query
writes every path it returns in this form.

**`fill(partial, complete, paths)`** writes into `partial`, at each path, the
value of `complete` there: a record as existing with its fields unknown, a
variant by its tag, every other value (a `Date` too) whole.
[Answering an entry](#answering-an-entry) answers an entry round by round.

**What `runa test` checks per query.** Outside tests a query only
propagates. Under `runa test`, every query also runs the two checks of
`--partial=check`: the strict-mode oracle (if strict evaluation of the case
is `Complete(r)`, propagation must return `Complete(r)`) and the completion
test (every generated completion of a `Complete(r)` case, 32 per query, must
return `r` or stop with an arithmetic-overflow error). A violation fails the
test at the query, naming its round (the how-manyth time this query ran) and
the completion that disproves it:

```text
asks_income.runa:7: query tax_of, round 1: the propagated result is Complete, but the completion income = -1 returns a different result: {"tax":0} instead of {"tax":-10}
```

`runa build` and `runa run` refuse a program with a `query`, an `unknown` or
an `AnswerGuide`, naming each; `runa check` checks its frontend and reports
`interpreted (query)` (`interpreted (guide)` for a file of guides alone), and
`runa test --run` runs such a test in the interpreter and marks it
`interpreted (query)`. Specified in
[Needs in the language](../rfcs/needs-in-the-language.md).

## Answering an entry

Experimental. A partial case says which facts are missing, not who answers
them. That depends on the integrator, so it is not the law's to say. The
integrator says it in an **`AnswerGuide`**: input paths, each mapped to one of
its own groups of resolvers. `answer` uses a guide in Futuruna, and `runa call
--guide` and the TypeScript SDK use it outside.

```runa
-- household.guide.runa
@ import ./household.entry

# Origin = Registry | Customer

= household_guide = AnswerGuide(fields = [
    EntryField(path = pathof(Household::spouse), by = Customer),
    EntryField(path = pathof(Household::children::age), by = Registry)
])

= complete = Household(income = 500, married = True, spouse = Some(Spouse(income = 50, cohabiting = True)), children = [Child(name = "Ada", age = 4)])
= started: Household = Household(income = 500, married = True, spouse = unknown, children = [Child(name = "Ada", age = unknown)])
= answered = answer(household_tax, started, household_guide, from_case(complete))
```

The prelude of a program that builds a guide or calls `answer`:

```text
AnswerGuide(i, g) = AnswerGuide(fields: List(EntryField(i, g)))
EntryField(i, g)  = EntryField(path: Path(i), by: g)
Answers(i)        = Answers(from: i, paths: List(Path(i)))
Asking(i, g)      = Asking(group: g, paths: List(Path(i)))
Round(i, g)       = Round(paths: List(Path(i)), groups: List(Asking(i, g)))
Outcome(o, i, g)  = Answered(result: o) | Unanswered(open: List(Asking(i, g)), case: i)
Answering(o, i, g) = Answering(rounds: List(Round(i, g)), outcome: Outcome(o, i, g))
WholeCase         = WholeCase
```

`i` is the entry's input type and `g` the integrator's group type, a sum of
constructors without fields. A group is written as any constructor is: bare
(`Customer`) where that is unambiguous, which includes a guide bound with its
type (`= g: AnswerGuide(Household, Origin) = ...`), and `Origin.Customer`
where two group types in scope declare `Customer`. On the wire (`"by"`) and
in TypeScript a group is its bare name: a guide has one group type.

**Required and named.** A path the guide does not name is **required**: the
integrator builds the fact into every case, and a need there is a defect in
its data. A path the guide names may be missing: the case still carries it
whenever the integrator has it, and the group is asked only for what is
missing. A named path covers every path below it, unless a more specific item
names one below; a need belongs to the most specific item at or above its
path. The forms of a path, each covering what is below it, are a field
(`pathof(TaxCase::taxpayer::spouse)`), a list with its elements' fields
(`pathof(TaxCase::events)`), an element field
(`pathof(TaxCase::opening_positions::isin)`), an alternative's field
(`pathof(TaxCase::events::Trade::amount_øre)`), a tag
(`pathof(TaxCase::events::$variant)`) and the whole input (`pathof(TaxCase)`).
A need's list indices are ignored when it is matched with an item, and the
alternative it lies in is named.

**`answer(e, case, guide, resolve)`** answers the `@ entry` `e` about `case`,
round by round:

1. it asks `query e(case)`, with the strict-mode oracle and the completion
   test under `runa test`;
2. on `Complete` it ends with `Answered(result)`;
3. a need at a required path stops the program with an error naming the path,
   the guide and the round;
4. it calls `resolve(group, needs, case)` once for each group that has needs
   this round, with all of that group's needs (**batching**). The resolver
   returns `Answers(from, paths)`: the paths it answers and a case holding
   their values. An answer for a path the group was not asked is an error;
5. it writes the answers into the case with `fill` and asks again;
6. a round that answers nothing ends with `Unanswered(open, case)`: what each
   group left open, and the case as far as it got.

`rounds` records, for each round, the needs' paths and what each group was
asked, so a test states "the registry is asked for the ages, then the
customer for the spouse". What a resolver does inside (read a database, ask a
person, run another entry) is the integrator's business; `answer` only sees
which paths it answered.

**`from_case(complete)`** is a resolver that answers every need from
`complete`, and **`whole_case_guide()`** a guide whose one group, `WholeCase`,
resolves the whole input. Together they answer a case from a complete one,
round by round:
`answer(e, partial, whole_case_guide(), from_case(complete))`.
`runa fmt --migrate` rewrites a 0.5 `interview(|c| query e(c), partial,
complete)` to that form, with its `.rounds` and `.result == v`.

**Where a guide lives.** A guide is a top-level binding of a ground value,
`AnswerGuide(fields = [EntryField(path = pathof(...), by = Group), ...])`, so
that `runa check`, `runa call --guide` and `runa sdk ts --guide` read its
paths and groups without running the program. It is the integrator's, not the
law's: a guide in a file that declares an `@ entry`, or in a file such a file
imports, is a check error, and so is `answer` reached from an `@ entry`. A
guide file is called `*.guide.runa` by convention. The guide does not name its
entry; one guide serves every entry with its input type.

**Checks.** `runa check` reports:

- a guide that is not a ground `AnswerGuide(fields = [...])`, or that names
  no path;
- a path with a list index (who answers does not depend on which element),
  and paths into two input types;
- the same path named twice;
- a `by` that is not a constructor without fields, groups of two types, a
  group type that is not a plain sum of constructors, and a group constructor
  with no path (its resolver would resolve nothing);
- as a warning, an item every one of whose need-able paths is named more
  specifically (it resolves nothing);
- `answer` of a name that is not an `@ entry`, a guide argument that is
  neither a guide binding nor `whole_case_guide()`, and a guide whose paths
  are into another type than the entry's input.

**Composition and running another entry** are different things. An entry
that calls another entry makes an ordinary call: under a query the callee's
needs are the caller's, at the caller's paths, and the guide treats them like
any other path. Running another entry separately is something a resolver may
do, for a field whose metadata names the entry that provides it
(`@ field(entry = refof(share_income), subject = ...)`): the resolver may run
`answer` on that entry with a guide of its own, read a stored figure, or ask
someone. That second `answer`'s needs are its own, not the caller's.

**`runa call --guides FILE --guide NAME`** gives each need of a partial call
(`--partial`, `--partial=strict` or `--partial=check`), and each `later`
need, the group of the guide's most specific item for its path:

```sh
runa call household.entry.runa --partial --input partial.json \
  --guides household.guide.runa --guide household_guide
```

```json
{ "kind": "fact", "path": "children[0].age", "by": "Registry", "waiting": [ "..." ] }
```

`FILE` is checked as `runa check` checks it, and `NAME` is a guide it binds,
rooted at the entry's input type. A need at a required path keeps no `by` and is
a diagnostic of its case, as in `answer`. The entry's contract, its template
and its schema hash do not change; the partial-result schema documents `by`.

## Explanations

Experimental. `runa call --explain` returns with every case result an
`explanation`: for each leaf of the result, the rule clause that produced it,
the law section and cited paragraphs of that clause, and the rules and inputs
it was computed from.

```sh
runa call model.entry.runa --input cases.json --explain
runa call model.entry.runa --input cases.json --explain=full
runa call model.entry.runa --input partial.json --partial --explain
runa schema model.entry.runa --format explanation
```

```json
{ "case_id": "ask-over-ceiling-2026",
  "result": { "...": "..." },
  "explanation": {
    "values": {
      "outcome.statement.excess_levy_ore": {
        "value": 31545,
        "by": { "$variant": "Rule", "rule": "excess_levy_ore", "clause": "default",
                "location": "src/ask.runa:268", "section": "ask_ceiling",
                "cites": [ { "paragraph": "§ 9, stk. 3",
                             "source": "sources/01-aktiesparekontoloven-LBK-281-2025.txt",
                             "text": "Stk. 3. Foretages indskud i strid med stk. 1, ..." } ] },
        "from": [
          { "$variant": "Rule", "rule": "share_of", "clause": "default",
            "location": "src/money.runa:17", "value": 11514000 },
          { "$variant": "Rule", "rule": "levy_days_per_year", "clause": "default",
            "section": "ask_ceiling", "paragraphs": ["§ 9, stk. 3"], "value": 365 } ] } },
    "sections": {
      "ask_ceiling": { "location": "src/ask.runa:167",
                       "sources": ["sources/01-aktiesparekontoloven-LBK-281-2025.txt", "..."],
                       "documents": [ { "binding": "aktiesparekontoloven",
                                        "file": "sources/01-aktiesparekontoloven-LBK-281-2025.txt",
                                        "role": "Source" }, "..." ],
                       "attachments": [ { "role": "Source", "binding": "aktiesparekontoloven", "value": { "...": "..." } } ],
                       "quote": "[sources/01-aktiesparekontoloven-LBK-281-2025.txt]\n\n§ 9. Personer, ..." } } } }
```

`values` holds every leaf (number, string, Bool, or the tag of a variant
without fields) by its output path, in the path syntax of needs. `by` says
what produced it:

- `Rule`: the rule, its `scope` for a RuleScope member, `clause` (`default`,
  or `exception` with the `exception` label that applied, or `clause` for a
  logic clause), its `location`, its law `section`, and the paragraphs it
  `cites` with their quoted text;
- `Input`: the value is the input's at `path`, passed on unchanged into the
  result by functions and as a member of the records rules build (a rule
  that returns a single value claims it, with the input in `from`);
- `Computed`: an operator or builtin outside any rule computed it;
- `Constant`: a literal of the model.

`from` lists the nearest rules and inputs the value was computed from, each
with its value. `--explain` lists at most 32 and counts the rest in
`from_omitted`; `--explain=full` lists all and adds `derivation`, every rule
node reached with its own clause and contributors, so the whole tree can be
walked from any value. `sections` holds each section an explained clause
sits in, by name: the location of its `@ section` line, the files of the
documents it quotes (`sources`), each quoted document with its binding, file
and role (`documents`), its attachments (the roles of its quotes and its
`@ meta` values, sorted by role, binding and value) and its `quote`: its
`@ quote` blocks in order, each
document's text after a `[<file>]` line. When an amending act quoted in the
section amends a quoted base passage, `superseded` lists that passage with the
instruction (`as_amended_by`), so the replaced wording is shown as replaced. A
cited paragraph carries its document's `act` (`title`, `identifier`, `url`),
read from the document's header as for needs, and `as_amended_by` when it is
cited `as amended`; its `text` is then the wording in force.

A value a rule clause produced also says `because`: what the guards that
chose the clause were computed from, the guard of the clause that applied and
the guards of the clauses above it that were tried and did not. The guards are
still read untraced, so `because` changes nothing in the result; it lists the
rules and inputs behind the guard values, at most 32 without `full`. A
literal a `match` arm chose is computed from the matched value, so a `Bool`
from `is_some(x)` is computed from what `x` holds (a `None` has nothing to
show).

Assumptions are not part of the explanation: every case of `runa call`,
explained or not, carries them beside its result (see
[Assumptions](#assumptions)), and the explanation's reader finds them there.

A rule clause names the paragraphs it implements with `@ cite` on the line
above it, inside the section:

```runa
@ cite(§ 9, stk. 3)
| excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
```

Paragraphs are separated by `;`; `nr. N` names a numbered item, `N. pkt.` a
sentence, and `in <document>` the quoted document's binding when two hold the
same number. A citation on a rule's first clause covers its other clauses that
have none; an exception, a RuleScope member and a function (`> f(...)`) in a
section can be cited too. `runa check` finds each cited paragraph in the
section's quotes, where a line starting `§ N.` opens a paragraph, `Stk. N.` a
subsection and `N)` an item, and rejects one the quotes do not hold, a
citation that names a paragraph and a part of it, and a plain citation of a
passage that an amending act quoted in the section rewords (cite it
`§ 8 a, stk. 1, 1. pkt. as amended`). Without a citation, an explanation shows
the section. Sections, quotes, the quote check and citations are described in
[Annotations describe code](style.md#annotations-describe-code).

Explaining never changes a result: the `result` of every case is the one
`runa call` gives without `--explain`. `--explain` writes JSON only, and is
refused with `--partial=check`; with `--partial` a `Complete` result is
explained and a `Needs` result is not. Explanations are an interpreter
feature. The design is [Explanations](../rfcs/explanations.md).

## Assumptions

Every result of `runa call` carries the assumptions it rests on, beside it:
`assumptions` next to `result` in each case, and in a `--partial` call inside
`Complete`. They are the `Assumption` attachments of the rule clauses that
fired in the case (an `Assumption` written with `@ meta` on a clause or
RuleScope member), of the sections those clauses sit in, and of the entry and
its section; each once (by binding and value), in the order the model writes
them (by file, then line), each with its `section` (the law section it is
written in; absent outside any section), its `rule` (the rule, member or
function it is written on; absent when it is written on its section), its
`binding` when it is written as one, and its `value` as plain JSON. A case
with none leaves `assumptions` out;
a `Needs` result has none.

```json
{ "case_id": "closed",
  "result": { "room": 0 },
  "assumptions": [
    { "section": "closing", "binding": "closing_day",
      "value": { "id": "CLOSING-DAY", "statement": "The value on the closing day is the custodian's." } } ] }
```

So a model says each assumption once, on the rule it concerns, and a reader
knows each listed assumption matters to this result: an `Assumption` on an
exception is listed only when that exception applied; one on a section only
when a rule clause of the section fired. A function (`> f(...)`) running does
not count, since a function runs whenever it is called; write the assumption
on a rule clause that fires exactly when it applies. A model need not list
assumptions in its result type.

When an assumption matters only for some values of a clause's parameters,
write the condition on it instead of a second clause:

```runa
@ meta(Assumption(value = assumption_levy_base)) when excess_deposit_ore > 0
| levy_base_ore(excess_deposit_ore: Ore) -> excess_deposit_ore
```

The assumption is listed when the clause fires and the condition holds on
one of its firings. The condition is checked like the clause's guard (a
`Bool` over its parameters, and in a RuleScope member over the scope's
fields and members) and evaluated in the clause's environment when it
fires; it changes nothing the case computes or reads, and in a partial case
a condition that is not known yet holds. `when` qualifies only an
`Assumption` value on a rule clause or RuleScope member: other metadata
describes a declaration whatever its values, and a section or function does
not fire, so `when` there is an error.

Collecting them traces no value: every case records the rule clauses that
fire, one table lookup per clause that returns a value. The assumptions are
read once, when a worker has initialized the program: their values are the
initialized program's bindings, and no file is scanned for metadata during
a case.
`runa schema --format
result` prints the JSON Schema of a case, `--format partial-result` that of a
partial result; a result workbook (`--output x.xlsx`) lists them on its
`assumptions` sheet. The TypeScript SDK returns `{ result, assumptions }` from
every call (`Calculated<R>`), and `Complete` carries `assumptions`.

## TypeScript SDK

Experimental. `runa sdk ts` generates a typed TypeScript client from the
contracts of every `@ entry` in one file:

```sh
runa sdk ts model.entry.runa --out sdk/
```

The output is five ES modules for the file's entries, six when an entry is
answered by a guide ([Answering by a guide](#answering-by-a-guide)). They
import [`zod`](https://zod.dev) and the runtime package `@futuruna/runtime`
([The runtime package](#the-runtime-package)):

| File | Contents |
|------|----------|
| `types.ts` | One type per contract type, as TypeScript writes it (see [Types](#types)). Every field carries its label, question, help, unit and source labels as TSDoc. |
| `schema.ts` | A Zod validator for every type, `SCHEMA_HASH` per entry, and a wire codec (`<Type>Codec`) for every record and every sum type with payload variants. Records reject unknown fields. `Int` is `number` and validates as a safe integer: a value beyond ±(2^53 − 1) is rejected, never rounded. Each unit's constructor, with its decimal converters where its scale allows them. |
| `partial.ts` | `Partial<Type>` for every input record and sum type, in which any field may be left out and a record may be `{}`, with the same field TSDoc as `types.ts`; each entry's input path patterns (`<Entry>InputPatterns`, with the value written at each), its `Need` tagged by pattern, the `Complete`/`Needs` result, and `<entry>SetAt`. |
| `client.ts` | A call and a query function per entry, each also explained (`<entry>Explained`, `<entry>QueryExplained`), and the entry's descriptor (`<entry>Entry`: name, schema hash, codecs, path patterns). |
| `answer.ts` | Only with `--guide`: for each entry given a guide, its groups, its facts type, its resolvers by group, the guide's table and `<entry>Answer`. |
| `index.ts` | Re-exports the other modules and the runtime package. |

Each module starts with a header naming the source file, the runtime version
it imports and the schema hash of every entry. The output depends only on the
contracts and the guides, so regenerating an unchanged model rewrites
identical files. A contract type whose name collides with a generated name (a
runtime name such as `Need`, `Complete`, `Needs`, `Unknown` or `given`, or a
`Partial`, `Schema`, `Codec` or entry-derived name) is an error, and so are
two fields of one record or variant with the same camelCase name; rename one.

### The runtime package

`@futuruna/runtime` is what every SDK shares: the value validators and
`isoDate`, `none`, the wire codec, `setAt` and paths, needs, the calls through
a transport, answering by a guide, the unit converters and the error classes.
It depends on no model, so one copy serves the SDKs of every source file, and
`instanceof` on its error classes holds across them. Its source passes
typescript-eslint's `strictTypeChecked`.

`runa sdk runtime --out DIR` writes the package at `runa`'s own version:
`package.json`, the ES module `index.js` and its declarations `index.d.ts`.
`zod` (4.x) is its peer dependency, so the SDK and the runtime validate with
one `zod`. Every Futuruna release also attaches it, packed, as
`futuruna-runtime-<version>.tgz`.

The package that holds a generated SDK (the model package) depends on the
runtime as on any package, at the version of the `runa` that generated the
SDK; the SDK carries no copy of it. Either from the release, here 0.6.0:

```json
"dependencies": {
  "@futuruna/runtime": "https://github.com/Futuruna/futuruna/releases/download/v0.6.0/futuruna-runtime-0.6.0.tgz",
  "zod": "^4.0.0"
}
```

or vendored, from the `runa` that generates the SDK:

```sh
runa sdk runtime --out build/runtime
mkdir -p vendor && npm pack ./build/runtime --pack-destination vendor
npm install ./vendor/futuruna-runtime-0.6.0.tgz
```

which records `"@futuruna/runtime": "file:vendor/futuruna-runtime-0.6.0.tgz"`.
Check the tarball in with the SDK, so that building the model package needs
neither the network nor `runa`.

### Types

The SDK speaks TypeScript; the engine keeps its canonical JSON encoding, and
the SDK converts between them on every call:

| Futuruna | TypeScript | On the wire |
|----------|------------|-------------|
| field `opening_cash_ore` | `openingCashOre` | `opening_cash_ore` |
| `# CashDirection = Deposit \| Withdrawal` (no variant carries fields) | `"Deposit" \| "Withdrawal"` | `{ "$variant": "Deposit" }` |
| `# Event = Trade(…) \| CashMovement(…)` (payload variants) | a union tagged by `$variant`, positional payloads in `$values` | the same, with snake_case fields |
| `Date` | an ISO date `"2026-03-01"`, typed `IsoDate`: only `isoDate(text)` and the validators make one | `{ "year": 2026, "month": 3, "day": 1 }` |
| `Ore`, an `Int` with a [unit](runes.md#unit) | `Ore`, a branded number: `number & { readonly __unit: "Ore" }` | the same number |
| `T?` | `T \| null`; in a partial case `T \| None`, "none" written `none` | `null` |
| `List(T)`, `Set(T)` | arrays | the same |
| `Map(String, T)` | an object; keys are kept as they are | the same |

A sum type with payload variants keeps `$variant` as its tag. No Futuruna
field can be named `$variant`, so the tag never collides with a field, where
`kind` or `type` would (an account charge with a `kind: ChargeKind` field is
ordinary); narrowing works on it as on any literal property, and it is the
name the engine's paths and results use (`Complete`, `Needs`). A date is
validated as a day on the calendar from 0000 to 9999: `2026-02-30` is
rejected, and `isoDate("2026-02-30")` throws a `RangeError`; a bare string
such as `"yesterday"` is not an `IsoDate`.

A unit is declared once in `types.ts`, with its description as TSDoc, and
`schema.ts` holds its validator (`OreSchema`, typed `Ore` once parsed) and its
constructor `ore(n)`, which throws a `RangeError` unless `n` is a safe
integer. Wherever the contract says `Ore` (fields, results, the answers of
`<entry>SetAt` and the resolvers) a bare number or a `Micro` is a type error,
while a plain `Int` stays `number`.

A unit declared with a scale whose values are all finite decimals of its base
(`# unit Ore "DKK øre" = DKK / 100`, or `= 1 / 1000000`: a `per` built from
2s and 5s) also gets two exact converters, in integer arithmetic and never
through a float:

```ts
const price: Ore = ore.fromDecimal("1740.25"); // 174025
ore.toDecimal(result.taxOre);                   // "1740.25": how the integrator reads a figure in kroner
ore.toDecimal(ore(174000));                     // "1740.00", with as many decimals as one Ore has
```

`fromDecimal` takes the amount as text, a decimal number of the base, and
refuses an amount finer than one unit (`"1740.255"`), text that is not a
decimal number (`"1,5"`, `"1e3"`) and a result beyond a safe integer: it
throws a `CalculationError` of kind `amount`, whose `entry` is the unit and
whose message names the amount, as in `Ore: 1740.255 DKK is finer than one
Ore (1/100 DKK)`. Rounding an amount instead is the model's decision, which a
unit cannot declare yet. A unit whose scale is no finite decimal (`= DKK / 3`)
and a unit without a scale get no converters.

`toWire(codec, value)` and `fromWire(codec, value)` convert a value either way
with a codec from `schema.ts` (or `<entry>Entry.input` and `.output`), for
code that reads or writes `runa call` documents directly. A field the codec
does not know (a camelCase name on the wire, a snake_case name in TypeScript)
throws `WireError` with its path; in a call it is an `output` error. Any other
mismatch stays as it is, for the validators to reject.

### Calls

The SDK reaches an engine only through a transport. Each method receives the
JSON document `runa call --input` reads, holding one case marked ready, and
returns the document `runa call` writes, as an object or as JSON text:

```ts
interface EngineTransport {
  call(entry: string, input: unknown): Promise<unknown>;
  callPartial(entry: string, input: unknown): Promise<unknown>;
}
interface ExplainingTransport extends EngineTransport {
  callExplained(entry: string, input: unknown, full: boolean): Promise<unknown>;
  callPartialExplained(entry: string, input: unknown, full: boolean): Promise<unknown>;
}
```

For an entry `tax_report` with input `TaxCase`:

- `taxReport(transport, input: TaxCase)` validates the input, encodes it,
  calls `transport.call`, checks the envelope (schema hash, entry and output
  schema, where the document has an envelope), decodes and validates the
  result, and returns it with its assumptions: `{ result, assumptions }`
  (`Calculated<TaxReportOutcome>`; `assumptions` is `[]` when the engine
  lists none, each a `CalculationAssumption` as the engine writes it);
- `taxReportQuery(transport, input: PartialTaxCase)` does the same through
  `transport.callPartial` and returns `Complete` with `result`,
  `assumptions` and `unusedPaths`, or `Needs` with `needs`;
- `taxReportExplained(transport, input, depth?)` and
  `taxReportQueryExplained(transport, input, depth?)` do the same through
  `callExplained` and `callPartialExplained` (`--explain`, or
  `--explain=full` for depth `"full"`) and return `{ result, assumptions,
  explanation }` (a query: the `Complete` result and the explanation),
  typed by `Explanation`. They take an `ExplainingTransport`: passing a
  transport that cannot explain is a type error, not a runtime failure. The
  explanation speaks TypeScript too: its values
  are keyed by TypeScript paths of the result, an input it names is at its
  TypeScript path in the case, and `from_omitted` is `fromOmitted`. Leaf values
  are numbers, strings, booleans and an enum's variant name; a `Date` is one
  value, its ISO date, unless its parts came from different places, when each
  part is explained at `<date>.year`, `.month` and `.day`. The inputs and
  rules a clause's guards read (`because`) are at TypeScript paths as `from`
  is.

A failure throws `CalculationError` with `kind` `input`, `output`, `envelope`
or `diagnostics`, and the validation issues (at TypeScript paths) or the
engine's diagnostics (at paths of the input document). The error classes come
from the runtime, so `instanceof` holds across every SDK that shares it.

### Partial cases and needs

In a partial case a value is the answer, `{}` is a record whose fields are not
known yet, `none` is the answer "none" for an optional field, and a field left
out (or `undefined`) is unknown. A bare `null` is not a partial value: it is a
type error and fails validation, so "none" cannot come from a stray `?? null`.
On the wire `none` is `null`. A sum type with payload variants whose variant
is not known yet is `{}`, typed `Unknown<T>`, whose `$variant` is `undefined`:
a `switch` on `$variant` that does not handle it is not exhaustive. An enum or
a date is known whole or left out (inside a list or map, where nothing can be
left out, it may also be `Unknown<T>`). At an optional position, an object
whose only key is `$none` (with value `true`) is `none`.

A need names a concrete input path as TypeScript writes it: fields join with
`.` under their camelCase names, list elements are `[index]`, map values are
`.key`, and the tag of a sum type with payload variants is `.$variant`, as in
`accounts[2].closingCashOre` or `taxpayer.outsideEndavu.$variant`. An enum is
needed at its own path (`events[0].direction`) and a date as a whole
(`events[0].date`). `unusedPaths` and an entry need's `subject` are TypeScript
paths too; the engine's own paths (`runa call --partial`) stay snake_case.

Each need is tagged with its `pattern`, the path with `[]` for every list
index and `*` for every map key (`accounts[].closingCashOre`), and its `path`
is an `InputPath` of that pattern: a branded string only the validators make,
so a path such as `accounts[-1].closingCashOre` cannot be written by hand.
`TaxReportInputPatterns` maps every pattern to the value an answer writes
there; `TaxReportNeed` is the union of the needs of every pattern, so
narrowing `need.pattern` narrows the need. Each need carries the field's
`label`, `question`, `help`, `unit` and `sources` where the contract gives
them; `waiting`, the rules and functions in a law section that wait for the
value, the ones citing paragraphs first, each with its `section` and cited
`paragraphs` (paragraph, quoted source and act); and, inside a list whose
element type declares a key field (`@ field(key = True)`, on a record or on
every variant of a sum type), `keys`: the element's path, key field and value,
typed by the contract, so a need on `openingPositions[0].marketValue` names the
ISIN it is about. A need of kind
`entry` also names the `entry` that provides the value and the `subject` it
runs for. Besides `needs`, a `Needs` result may hold `later`: needs of the same
type for facts read only behind a condition not known yet, which answering
`needs` may turn into needs.

`taxReportSetAt(case, need, value)` returns a new case with `value` written at
the need's path; the value's type is the one at the need's pattern, and a need
not yet narrowed to one pattern takes no value. `setAt(case, path, value)` is
the same, untyped. `fromWirePath(codec, wireCase, path)` turns an engine path
into its TypeScript path.

```ts
function answerFromLedger(c: PartialTaxCase, need: TaxReportNeed): PartialTaxCase {
  switch (need.pattern) {
    case "accounts[].closingCashOre":
      return taxReportSetAt(c, need, ledger.closingCash(need.path));
    case "taxpayer.spouse":
      return taxReportSetAt(c, need, none);
    default:
      return c;
  }
}
```

### Answering by a guide

The integrator states, in an `AnswerGuide` ([RFC](../rfcs/answer-guide.md)),
which input paths may be missing and which of its groups resolves each; every
path the guide does not name is required. `--guides` names the file that binds the
guides, and each `--guide` the entry a guide answers:

```sh
runa sdk ts src/aktiesparekonto.entry.runa --out sdk/ask \
  --guides guides/endavu.guide.runa --guide aktiesparekonto_year=endavu_ask_year
```

`runa` checks the guides file as `runa check` does, then reads the guides
without running it. A guide whose paths are of another type than the entry's
input, or an item at or below which no need can arise, is an error. For each
entry given a guide, `answer.ts` holds:

```ts
export type AktiesparekontoYearGroup = "Snapshots" | "GivingInstitute" | "Bookings" | "PreviousYear" | "Backoffice" | "Customer";
export interface AktiesparekontoYearGroupPatterns { Snapshots: "account.openingCashMicroDkk" | …; … }
export type AktiesparekontoYearGuidedNeed<G extends AktiesparekontoYearGroup = AktiesparekontoYearGroup> = …;
export type AktiesparekontoYearFacts = { readonly incomeYear: number; readonly account: { … }; … };
export type AktiesparekontoYearResolvers = GroupResolvers<…>;
export const aktiesparekontoYearGuide: Guide<AktiesparekontoYearGroup> = { name: "endavu_ask_year", paths: { … } };
export function aktiesparekontoYearAnswer(transport, facts, resolvers, options?): Promise<Answering<…>>;
```

- **The required facts** (`<Entry>Facts`) are the backend's checklist: the partial
  case with the key of every required fact required and its value of the model's
  type, never `undefined`, so leaving a required fact out, or a fact the model
  adds, does not compile. An optional fact may be `none` where the model
  allows it. A builder whose source may lack a value must decide there what a
  missing value means. A fact the guide resolves may be left out, or given
  when known; its TSDoc names the group that resolves it.
- **The resolvers** (`<Entry>Resolvers`) are keyed by group, and every group
  is required. Each receives the needs of its group's patterns only
  (`<Entry>GroupPatterns`), so a `switch` on `need.pattern` can be exhaustive.
  `leaveOpen` is the resolver of a group nobody can fill yet: one visible line,
  the one that changes when a source appears.
- **The guide's table** (`<entry>Guide`) gives the group of every need-able
  path pattern the guide resolves, each alternative of a sum type named before
  its fields (`events[].Trade.amountOre`), as the guide's most specific item
  says. The runtime attaches it to each need as `by`; the engine, the contract
  and its schema hash are unchanged.

`<entry>Answer(transport, facts, resolvers)` queries the required facts, calls each
group's resolver once per round with all of that round's needs of the group,
the groups at the same time, writes the answers into the case and queries
again. A resolver fetches what its needs ask for in one batch and returns the
function that answers each need:

```ts
const accountId = (need: TaxReportNeed) => need.keys?.find((key) => key.field === "accountId")?.value;
const resolvers: TaxReportResolvers = {
  Ledger: async (needs) => {
    const accounts = await ledger.accounts(needs.map(accountId));
    return (need) => {
      const account = accounts.get(accountId(need));
      if (account === undefined) return open;
      switch (need.pattern) {
        case "accounts[].openingCashOre":
          return given(need, account.openingCashOre);
        case "accounts[].closingCashOre":
          return given(need, account.closingCashOre);
      }
    };
  },
  PriceFeed: leaveOpen,
  Customer: leaveOpen,
};
const answering = await taxReportAnswer(transport, facts, resolvers);
if (answering.$variant === "Answered") report(answering.result);
else store(answering.partialCase, answering.open.Customer ?? []);
```

The loop calls the returned function once per need, so every need gets a
reply made from that need, and none is left out. It returns
`given(need, value)`, `open` for a need the group cannot fill, or another
entry's outcome from `context.runEntry(need, run)`. `runEntry` runs the entry
an entry need names (a fact need is given one: `{ ...need, entry, subject }`)
through `run`, which passes its `options` on to that entry's answer, as
`(options) => shareIncomeAnswer(transport, facts, resolvers, options)`; an
`Answered` result is written at the need's path, and an `Unanswered` one waits
in `pending`. A reply names the need's path and its value is typed by the
need's pattern, so a resolver of several patterns narrows `need.pattern`
before it answers, and an answer cannot land at another need's path, as one
from a list of answers in another order (the backend's rows) would: a reply
for another path throws `AnswerError`. `context.partialCase` is the case of
the round, with every answer so far.

Answering ends `Answered`, with the result, its assumptions and the rounds,
or `Unanswered` when a round answers nothing: `open` holds the last round's
needs keyed by the group that left them open (`open.Customer` holds
questions, `open.Snapshots` facts a source did not have), with entry needs
waiting on another entry in `pending`, the last round's `later` facts and the
partial case. Each round records its needs' paths and each group's needs. A
need at a required path throws `MissingFactError` naming the paths, the guide and
the round, before any resolver runs; the types rule this out, so it catches
only what they cannot see, such as untyped data or an `as` cast. A pair
of `(entry, subject)` that repeats on the stack of entries being answered
throws `AnswerCycleError`, since entries that need each other belong in one
joint entry. A path the engine still needs after it was given throws
`AnswerError`.

Batches make second-order resolution cheap: a group that gives a list
(`events`) answers skeletons that give each element's key and variant and
leave its details out; the next round needs only the fields the law reads,
inside those elements, each need with the element's key in `keys`, and the
group of those fields (`events::Trade::amount`, named in the guide) fetches
them all in one call.

Without `--guide`, the SDK has the calls and queries only.
