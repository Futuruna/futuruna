---
feature_stage: preview
feature_stage_surfaces:
  - typed-calculation-contracts
---

# Typed Calculations

`@ calculate` marks an ordinary typed rule or function as an external
calculation boundary. It does not change rule evaluation and is not an effect.

```runa
# Input(monthly_income: Int, status: FilingStatus)
# Result(annual_tax: Int)

@ calculate("Household tax calculation")
| calculate(input: Input) -> Result(annual_tax = annual_tax(input))
```

A calculation has exactly one explicitly typed input parameter. A rule result
must infer to one concrete serializable type; a function must declare its result
type. The input is one named domain type. Conditions, defaults, exceptions,
matches, rule scopes, and ordinary rule dependencies continue to work normally.

Use plain `@ calculate` when the rule name is sufficient, or provide one
human-readable title with `@ calculate("Household tax calculation")`. The title
names the whole calculation and appears in its schema and generated workbook.
It does not label every nested input field. Field labels, interview questions,
help, units, and sources belong in typed field metadata.

## Describe Human Input

Keep the calculation boundary machine-stable and attach presentation metadata to
individual canonical input paths. A field binding is an ordinary pure ground
record. Its typed field shape and canonical `path` give it calculation meaning;
the anchor label names the calculation entry and its single `meta` binding
contains the fields and their provenance.

```runa
# CalculationField(path: String, label: String, question: String?, help: String?, unit: String?)
# SourceInfo(url: String, section: String)
# CalculationMetaRole(a) = Source(value: a)
# CalculationMeta(a) = CalculationMeta(fields: List(CalculationField), attachments: a)
# impl MetaRole for CalculationMetaRole {}
# impl Meta for CalculationMeta {}

= tax_source = SourceInfo(
    url = "https://example.invalid/tax",
    section = "income"
)
= monthly_income_field = CalculationField(
    path = pathof(Input::monthly_income),
    label = "Monthly income before tax",
    question = Some("What do you earn before tax each month?"),
    help = Some("Use the gross amount before deductions."),
    unit = Some("currency/month")
)
= monthly_income_meta = CalculationMeta(
    fields = [monthly_income_field],
    attachments = (
        Source(value = tax_source),
    )
)

--@label:calculate_tax::meta:monthly_income_meta--
```

`path` uses the exact canonical path emitted by the calculation layout. Root
fields use paths such as `monthly_income`; a field in a related list table uses
a path such as `children.age`, while `children` can describe the collection
itself. `input.monthly_income` and `TaxInput.monthly_income` are accepted and
normalized to the canonical path. Unknown paths and duplicate metadata for one
path are errors.

Prefer `pathof(InputType::field::nested_field)` for new metadata. It is checked
against the declared or plainly imported type graph during `runa check`, has
type `String`, and lowers to the same canonical string stored in the contract.
`List`, `Set`, optional, and string-keyed `Map` value traversal is transparent:
`pathof(Input::children::age)` lowers to `"children.age"`. Sum types require an
explicit constructor segment, such as
`pathof(Input::income::WageIncome::amount)`. Their discriminator uses the
terminal form `pathof(Input::income::$variant)`. A misspelled segment is
reported at that segment. Literal paths remain supported for generated data and
backward compatibility.

When metadata belongs to a nested domain type and should be reusable wherever
that type occurs, declare its `path` field as `ProgramReference` and use
`refof(Type::member)`:

```runa
# ChildField(path: ProgramReference, label: String)
# ChildMeta(fields: List(ChildField))
# impl Meta for ChildMeta {}

= child_meta = ChildMeta(fields = [
    ChildField(path = refof(Child::age), label = "Child age")
])

--@label:Child::meta:child_meta--
```

For an input containing `primary: Child` and `children: List(Child)`, this one
declaration produces `primary.age` and `children.age`. Projection crosses
records, alternatives, lists, sets, and string-keyed map values.
Exact `String`/`pathof(...)` metadata and `refof(...)` rooted at the calculation
input override projected metadata. Duplicate declarations at the winning
specificity are errors.

Optional composites remain one canonical JSON field in calculation contract
v1, so their internal members are not independent projected metadata targets.
Target the optional JSON field exactly when it needs a label or question.

The record must use named fields `path`, `label`, `question`, `help`, and `unit`.
`path` is a `String` for exact metadata or a structural `ProgramReference` for
reusable metadata, and `label` is a required string. The other fields may be
strings, optional strings, or omitted by a record type that does not declare them.
Role-bearing values such as `Source` attachments remain typed metadata and are
copied into that field's source trace.

Scale the same ordinary typed aggregate to any number of fields and attach that
object once with the `meta` role. The calculation consumer finds field-shaped
typed descendants recursively; other metadata consumers remain free to query
the same object by its own nested types.

```runa
= tax_input_meta = CalculationMeta(
    fields = [
        monthly_income_field,
        filing_status_field,
        children_field
    ],
    attachments = (
        Source(value = tax_source),
    )
)

--@label:calculate_tax::meta:tax_input_meta--
```

Typed metadata aggregates compose hierarchically. A role-bearing attachment on
an outer aggregate applies to every calculation field below that aggregate. An
attachment inside a nested aggregate applies only to fields below that nested
aggregate. This lets a calculation attach one root metadata object while each
domain component retains its own source, warning, and guidance trace.

The anchor and its typed aggregate may live in a recursively imported metadata
module. Calculation tooling collects only imported anchors whose label names an
actual calculation entry or its source span, so unrelated dependency metadata
cannot alter the contract. Imported fields use the same exact-path validation
and duplicate checks as local fields.

The emitted field binding names include stable aggregate paths such as
`tax_input_meta.fields[0]`. Direct `field` references remain accepted for source
compatibility; new code should attach one typed `meta` object. A `field`
reference that is neither a field record nor an aggregate containing field
records is an error; a generic `meta` object without calculation fields is
simply ignored by the calculation field consumer. Nested variants of types
implementing `MetaRole` are copied into the calculation and field source traces.
Each trace keeps
the canonical source `binding` and adds the aggregate `attachment_path`.

Field metadata appears structurally in `runa schema` and participates in the
schema fingerprint. In XLSX, the human label is the visible column header; the
exact canonical path and all presentation data remain in the hidden `_columns`
or `_tables` sheet. A field without explicit metadata receives a deterministic
humanized path as its visible fallback. Header notes always expose the canonical
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
runa schema model.calculate.runa
runa schema model.calculate.runa --entry calculate_tax
```

The command emits `futuruna.calculate.v1` JSON with input and output types,
reachable type definitions, linked metadata, and a SHA-256 schema fingerprint.
Use `--entry` when a file declares more than one calculation.

Linked metadata may reference ground bindings in the calculation file or in a
recursively reachable plain import. The contract includes stable metadata type
and value data in its fingerprint. Definition file paths and line numbers stay
in `runa meta --json` and do not make a portable calculation contract
machine-dependent.

## Reuse Validated Contracts

`schema`, `template`, and `call` persist successful contract validation in a
content-addressed local cache. The key includes the root source, every
transitive plain, qualified, and content-hash import, prelude mode, cache format,
and the exact compiler executable. An edit anywhere in that graph or a compiler
rebuild therefore causes a miss. Parse or type errors are never cached, and a
corrupt entry is ignored and rebuilt.

The default cache is under the operating system's user cache directory. Set
`FUTURUNA_CALCULATION_CACHE_DIR` to choose another root, set
`FUTURUNA_DISABLE_CALCULATION_CACHE=1` for an uncached validation run, or set
`FUTURUNA_CALCULATION_CACHE_TRACE=1` to report `hit`, `miss`, or `disabled` on
standard error. The cache contains contracts only; calculation inputs and
results are not stored.

## Audit Calculation Reachability

```sh
runa audit model.calculate.runa --entry calculate_tax
runa audit model.calculate.runa --entry calculate_tax --json
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
connecting it. Running `runa audit` without `--entry` keeps the original
topology audit for invariant gaps, asymmetries, and tensions.

## Generate Input

```sh
runa template model.calculate.runa --format json --output cases.json
runa template model.calculate.runa --format toml --output cases.toml
runa template model.calculate.runa --format xlsx --output cases.xlsx
```

JSON is the canonical value model. TOML omits absent optional record fields.
XLSX flattens nested named records into columns, gives booleans and nullary enums
constrained choices, expands finite payload alternatives into a `$variant`
choice plus typed variant-qualified columns, and puts each `List`, string-keyed
`Map`, or `Set` field in a separate related worksheet. Integer template cells
are text-formatted so all `i64` values remain exact.

`cases` is the first visible worksheet and contains scalar fields for the named
input record. Its first row visibly shows the `@ calculate` title, and every
related collection sheet combines that title with the collection's field label.
Explicit field labels replace machine paths only in visible headers; hidden
topology retains the paths. Every collection row
uses `case_id` and `item_id`; nested collection sheets add `parent_id`. List rows
use one-based `position`, map rows use `key`, and set rows have neither. Leave a
collection sheet without matching rows to supply an empty collection. Hidden
`_futuruna`, `_tables`, and `_columns` sheets record the contract fingerprint,
generated topology, and column types; do not edit them. Optional composite fields
and recursive or opaque leaves remain canonical JSON cells when they cannot be
expanded to a finite unambiguous layout. Cells and child rows belonging to an
inactive alternative are rejected.

Every template records the entry and schema fingerprint. A source type change
makes an old template stale; invocation reports the expected and actual hashes
instead of coercing the data.

## Invoke Cases

```sh
runa call model.calculate.runa --input cases.json
runa call model.calculate.runa --input cases.toml --output results.json
runa call model.calculate.runa --input cases.xlsx --output results.xlsx
```

Each case is decoded through the same Futuruna contract and evaluated in an
isolated interpreter. Valid cases can produce results even when another case has
a diagnostic. The command exits unsuccessfully when any diagnostics remain.

Calculation workbooks must be `.xlsx`. VBA projects and formulas in the input
sheet are rejected. Unknown columns, duplicate or empty case identifiers,
duplicate item identifiers, list positions, or map keys, orphaned parent rows,
non-exact integers, invalid enum choices, missing required fields, and malformed
canonical JSON are rejected before their case runs. A bad related row invalidates
that case while other valid cases still run.

An XLSX result contains a compact `results` summary, a lossless
`result_values` table, and case-scoped `diagnostics`. `result_values` uses RFC
6901 JSON Pointer paths and canonical JSON scalar text; long values occupy
ordered chunks rather than being truncated at Excel's cell-size limit.

The full format and evolution contract is in
[Typed calculation contracts](../rfcs/typed-calculation-contracts.md).
