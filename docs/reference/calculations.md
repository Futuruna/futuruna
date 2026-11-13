---
feature_stage: mixed
feature_stage_surfaces:
  - typed-calculation-contracts
  - calculation-needs
  - typescript-sdk
---

# Typed Calculations

`@ calculate` marks an ordinary typed rule or function as an external
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

Members of optional composites are metadata targets too, at any depth:
`taxpayer.spouse.cohabiting_at_year_end`,
`pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end)`, and
`refof(Spouse::cohabiting_at_year_end)` projected through `spouse: Spouse?` name
the same member, with the same precedence and duplicate rules. A path into a
member the type does not declare is an error. The optional field itself stays
one input field and can carry its own metadata.

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

For models with repeated field-source provenance, opt into a lossless compact
schema export:

```sh
runa schema model.calculate.runa --format compact-json --output schema.json
```

The distinct wire schema `futuruna.calculate.compact.v1` stores each exact source
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

A call prepares its calculation entry once: the runtime rule-dispatch metadata
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
`futuruna::calculate::PreparedCalculation` is that prepared entry.

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
connecting it.

`runa audit FILE --json` works for every file. It also carries the structural
audit as `topology` (`summary` counts of rules, invariants, `contradictions`,
`tensions`, `asymmetries` and `invariant_gaps`; `findings` with `kind`,
`severity`, `title`, `rules` and `explanation`). For a file without
`@ calculate`, the entry sections (`entry`, `label`, `input_type`,
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
runa template model.calculate.runa --format json --output cases.json
runa template model.calculate.runa --format toml --output cases.toml
runa template model.calculate.runa --format xlsx --output cases.xlsx
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
shows the `@ calculate` title, and every related collection sheet combines that
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
runa template model.calculate.runa --input cases.xlsx --output refreshed.xlsx
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
runa call model.calculate.runa --input cases.json
runa call model.calculate.runa --input cases.toml --output results.json
runa call model.calculate.runa --input cases.xlsx --output results.xlsx
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
runa call model.calculate.runa --partial --input partial.json
runa call model.calculate.runa --partial=strict --input partial.json
runa call model.calculate.runa --partial=check --input partial.json
runa schema model.calculate.runa --format partial-result
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
      "label": "Closing balance",
      "question": "What was the balance on the closing day?",
      "unit": "øre",
      "sources": [ { "label": "§ 1", "role": "Source", "binding": "law" } ],
      "waiting_rule": "account_tax" } ] }
```

`unused_paths` lists the provided input paths evaluation never read. A
need's `path` is concrete, with list indices; its `label`, `question`,
`help`, `unit` and `sources` come from the field metadata of the same path
without indices, and `waiting_rule` names the rule that first read it: the
innermost rule of a law section on the stack when there is one, with its
`section` and cited `paragraphs`, else the innermost rule. The
output envelope's schema is `futuruna.calculate.partial-output.v1`, and
`runa schema --format partial-result` prints the JSON Schema of `result`.

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
metadata (`entry = Some("share_income")`, `subject =
Some("taxpayer.spouse")`). Its need has kind `entry`: the client runs that
entry for the subject and writes the entry's result at `path`. The field's
type must be the entry's result type; `runa check` reports a missing entry or
a different type.

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

## Explanations

Experimental. `runa call --explain` returns with every case result an
`explanation`: for each leaf of the result, the rule clause that produced it,
the law section and cited paragraphs of that clause, and the rules and inputs
it was computed from.

```sh
runa call model.calculate.runa --input cases.json --explain
runa call model.calculate.runa --input cases.json --explain=full
runa call model.calculate.runa --input partial.json --partial --explain
runa schema model.calculate.runa --format explanation
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
                       "attachments": [ { "role": "Source", "binding": "aktiesparekontoloven", "value": { "...": "..." } } ],
                       "quote": "..." } } } }
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
walked from any value. `sections` holds each law section an explained clause
sits in: its anchor's location, its `[sources/...]` documents, its metadata
attachments and the quote.

A rule clause names the paragraphs it implements with a citation comment
above it, inside the section:

```runa
--@cite:§ 9, stk. 3--
| excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
```

Paragraphs are separated by `;`; `nr. N` names a numbered item, and
`in <source>` the quoted document when two hold the same number. A citation on
a rule's first clause covers its other clauses that have none. `runa check`
finds each cited paragraph in the section's quote, where a line starting
`§ N.` opens a paragraph, `Stk. N.` a subsection and `N)` an item, and rejects
one the quote does not hold. Without a citation, an explanation shows the
section.

Explaining never changes a result: the `result` of every case is the one
`runa call` gives without `--explain`. `--explain` writes JSON only, and is
refused with `--partial=check`; with `--partial` a `Complete` result is
explained and a `Needs` result is not. Explanations are an interpreter
feature. The design is [Explanations](../rfcs/explanations.md).

## TypeScript SDK

Experimental. `runa sdk ts` generates a typed TypeScript client from the
contracts of every `@ calculate` entry in one file:

```sh
runa sdk ts model.calculate.runa --out sdk/
```

The output is five ES modules whose only runtime dependency is
[`zod`](https://zod.dev):

| File | Contents |
|------|----------|
| `types.ts` | One type per contract type, in the canonical JSON encoding: records as objects, sum types as unions tagged by `$variant` (positional payloads in `$values`), optional values as `T \| null`, lists and sets as arrays, `Map(String, T)` as an object. Every field carries its label, question, help, unit and source labels as TSDoc. |
| `schema.ts` | A Zod validator for every type, and `SCHEMA_HASH` per entry. Records reject unknown fields. `Int` is `number` and validates as a safe integer: a value beyond ±(2^53 − 1) is rejected, never rounded. |
| `partial.ts` | `Partial<Type>` for every input type, in which any field may be left out and a record may be `{}`, with the same field TSDoc as `types.ts`; the `Complete` and `Needs` results of a partial call; `Need` typed by the entry's input paths; and `setAt(case, path, value)`. |
| `client.ts` | The `EngineTransport` interface, a call and a query function per entry, and the `resolve` loop. |
| `index.ts` | Re-exports the other modules. |

Each file starts with a header naming the source file and the schema hash of
every entry. The output depends only on the contracts, so regenerating an
unchanged model rewrites identical files. A contract type whose name collides
with a generated name (`Need`, `Complete`, `Needs`, or `Partial`, `Schema` and
entry-derived names) is an error; rename the type.

### Calls

The SDK reaches an engine only through a transport. Both methods receive the
JSON document `runa call --input` reads, holding one case marked ready, and
return the document `runa call` writes, as an object or as JSON text:

```ts
interface EngineTransport {
  call(entry: string, input: unknown): Promise<unknown>;
  callPartial(entry: string, input: unknown): Promise<unknown>;
  callExplained?(entry: string, input: unknown, full: boolean): Promise<unknown>;
  callPartialExplained?(entry: string, input: unknown, full: boolean): Promise<unknown>;
}
```

For an entry `tax_report` with input `TaxCase`:

- `taxReport(transport, input: TaxCase)` validates the input, calls
  `transport.call`, checks the envelope (schema hash, entry and output schema,
  where the document has an envelope), and validates the result;
- `taxReportQuery(transport, input: PartialTaxCase)` does the same through
  `transport.callPartial` and returns `Complete` with `result` and
  `unused_paths`, or `Needs` with `needs`;
- `taxReportExplained(transport, input, depth?)` and
  `taxReportQueryExplained(transport, input, depth?)` do the same through
  `callExplained` and `callPartialExplained` (`--explain`, or
  `--explain=full` for depth `"full"`) and return `{ result, explanation }`,
  typed by `Explanation`.

A failure throws `CalculationError` with `kind` `input`, `output`, `envelope`
or `diagnostics`, and the validation issues or the engine's diagnostics.

### Partial cases and needs

In a partial case a value is the answer, `{}` is a record whose fields are not
known yet, `null` is the answer "none", and a field left out is unknown. A sum
type without `$variant` has an unknown variant.

A need names a concrete input path: record fields join with `.`, list elements
are `[index]`, map values are `.key`, and a sum type's tag is `.$variant`, as in
`accounts[2].closing_cash_ore` or `taxpayer.outside_endavu.$variant`. Each
need carries the field's `label`, `question`, `help`, `unit` and `sources`
where the contract gives them, and the `waiting_rule`. A need of kind `entry`
also names the `entry` that provides the value and the `subject` it runs for.

`setAt(case, path, value)` returns a new case with `value` written at a
concrete path; `taxReportSetAt` is the same, with the value typed by the path.
`TaxReportInputPaths` maps every concrete path to the type written there, and
`TaxReportInputPatterns` the same by pattern, with `[]` for any list index and
`*` for any map key.

### Resolving needs

`resolve(query, initialCase, resolvers)` runs the interview loop. It queries the
case, gives each need to the resolver registered for its path pattern, writes
the answers into the case and queries again:

```ts
const resolvers: TaxReportResolvers = {
  "accounts[].closing_cash_ore": (need) => answer(ledger.closingCash(need.path)),
  "taxpayer.married": () => ask,
};
const outcome = await resolve((c) => taxReportQuery(transport, c), {}, resolvers);
if (outcome.$variant === "Complete") report(outcome.result);
else store(outcome.partialCase, outcome.questions);
```

A resolver returns `answer(value)`, `ask` for a question only the customer can
answer, or the resolution of another entry from `context.runEntry(query,
case, resolvers)`. A need without a resolver is a question. The loop stops
with `Complete`, or with `Questions` (the questions, entry needs waiting on
another case's questions, and the partial case) when a round adds no answer.
It keeps the stack of `(entry, subject)` pairs it runs: a pair that repeats
throws `ResolveCycleError`, since entries that need each other belong in one
joint entry. A path the engine still needs after it was answered throws
`ResolveError`.
