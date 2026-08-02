---
feature_stage: preview
feature_stage_surfaces:
  - typed-calculation-contracts
---

# RFC: Typed calculation contracts

Status: Preview

## Summary

`@ calculate` marks one typed Futuruna callable as a calculation boundary. The
callable remains an ordinary rule or function: its implementation, dependencies,
conditions, exceptions, and rule scopes keep their normal language semantics.
The marker lets tools derive one versioned input/output contract from Futuruna's
types and invoke it through JSON, TOML, or XLSX without adding spreadsheet
semantics to the language.

```runa
# TaxInput(monthly_income: Int, municipality: Municipality, children: List(Child))
# TaxResult(annual_tax: Int, effective_rate: Float)

@ calculate
| calculate_tax(input: TaxInput) -> TaxResult(
    annual_tax = annual_tax(input),
    effective_rate = effective_rate(input),
)
```

The same marker can precede a function with an explicit return type:

```runa
@ calculate
> calculate_tax(input: TaxInput) -> TaxResult {
    TaxResult(annual_tax = annual_tax(input), effective_rate = effective_rate(input))
}
```

## Goals

- Keep rules as the source of truth and expose only a deliberate typed boundary.
- Derive required fields, optional values, nested records, lists, and alternatives
  from Futuruna types.
- Give JSON, TOML, and XLSX one canonical contract and schema fingerprint.
- Reject stale templates and malformed values before evaluating legal or business
  rules.
- Preserve exact integers and reject active spreadsheet content by default.
- Carry source metadata into the contract without making comments semantic.

## Non-goals

- `@ calculate` is not an algebraic effect and does not permit ambient input.
- It does not replace `@ export` or define a Rust, WASM, or network ABI.
- It does not infer domain constraints that are absent from Futuruna types.
- UI labels, units, examples, and source citations never determine validity.
- XLSX formulas and macros are not an execution mechanism.

## Declaration contract

The marker is a prefix annotation and applies to the next substantive top-level
statement. Other annotations may appear between the marker and the callable.
Version 1 accepts:

- a top-level `|` rule group whose marked head has one variable parameter with an
  explicit type and a single, concrete inferred result type;
- a top-level `>` function with exactly one explicitly typed parameter and one
  explicit result type.

The input type must be closed and serializable. The result type must be closed,
serializable, and unambiguous. References, mutable references, function values,
type holes, actors, streams, and effect values are not serializable contract
types. A marked function may not declare effects. Direct effect expressions in a
marked callable are rejected. The calculation CLI also rejects runtime results
that do not conform to the declared result type.

A file may contain multiple calculations. Commands require `--entry NAME` when
selection is ambiguous. A missing marker, duplicate marker on the same callable,
misplaced marker, untyped input, unknown result, or requested unknown entry is a
Futuruna diagnostic.

`@ calculate(...)` is deliberately not syntax. Prompts belong to field metadata,
not to the calculation boundary.

## Contract document

`runa schema` emits `futuruna.calculate.v1`. Its stable semantic fields are:

- `schema` and `schema_version`;
- `entry` and the single parameter name;
- structured `input` and `output` type references;
- reachable named type `definitions`, including type parameters, variants,
  positional status, and fields;
- matching typed meta-comment references and source spans;
- `schema_hash`, a lowercase SHA-256 fingerprint of the semantic document with
  the hash field empty.

Object key order is not semantic. Definition, variant, and field order is
semantic because it determines constructor layout and generated input order.

## Canonical values

JSON defines the canonical value tree used by all adapters:

| Futuruna type | Canonical value |
| --- | --- |
| `Int`, `Float`, `Bool`, `String`, `Char` | JSON number, boolean, string, or one-character string |
| `T?` or `Option(T)` | `null` for `None`, otherwise the canonical `T` value |
| `List(T)` | JSON array |
| `Map(String, T)` | JSON object |
| `Set(T)` | JSON array with no duplicate Futuruna values |
| single-constructor named record | JSON object keyed by field name |
| sum type | object containing `$variant` and its named fields |
| positional alternative | object containing `$variant` and `$values` |
| unit | `null` |

Nullary alternatives still use `{ "$variant": "Name" }` canonically. An adapter
may present such an alternative as a dropdown, but must reconstruct the canonical
object before validation. Unknown fields, missing required fields, invalid
alternatives, lossy integer conversions, and duplicate set values fail closed.

## Invocation envelopes

JSON and TOML use the same logical envelope:

```json
{
  "$futuruna": {
    "schema": "futuruna.calculate.input.v1",
    "schema_hash": "...",
    "entry": "calculate_tax"
  },
  "cases": [
    { "case_id": "case-1", "input": {} }
  ]
}
```

Case identifiers are non-empty and unique. An output envelope uses
`futuruna.calculate.output.v1`, retains the contract hash and entry, and contains
`results` plus case-scoped `diagnostics`. One invalid case does not prevent valid
cases from being evaluated unless the envelope or schema itself is invalid.

## XLSX adapter

Generated workbooks contain:

- `_futuruna`: hidden adapter schema, entry, contract hash, and encoding metadata;
- `cases`: one row per case with a required `case_id` column;
- `_tables`: hidden collection topology, including worksheet names, parent
  paths, attachment paths, collection kinds, item types, and variant guards;
- `_columns`: hidden worksheet, table path, visible path, canonical value path,
  type, encoding, requiredness, choices, and variant guards for each generated
  input column;
- one worksheet for every relational collection path;
- generated output workbooks additionally contain `results` and `diagnostics`.

The first visible worksheet is `cases` for input templates and `results` for
generated outputs. Machine-only metadata worksheets remain hidden while still
being validated on every invocation.

Named records are flattened into dotted columns. Primitive values and nullary
enums use native cells; enum and boolean columns use constrained choices. A
finite payload-bearing sum type uses a `$variant` choice column plus
variant-qualified typed payload columns. Positional payload fields use `_0`,
`_1`, and so on in visible headers while rebuilding canonical `$values` arrays.
A closed `List(T)`, `Map(String, T)`, or `Set(T)` field is normalized into its
own worksheet instead of being embedded as JSON in `cases`, including a
collection owned by one sum alternative.

Every collection row has `case_id` and an adapter-local `item_id`. Nested
collections also have `parent_id`, which references an item in their generated
parent worksheet for the same case. Lists have a positive, one-based `position`
that is unique per parent and determines canonical array order. Maps have a
non-empty `key` that is unique per parent. Sets have neither position nor key;
their canonical values must still be unique. Empty collections are represented
by zero matching rows. Item records are flattened into columns on their
collection worksheet, and collections inside those records become further child
worksheets.

Recursive leaves, opaque named types, and optional composite leaves retain
canonical JSON cells as a bounded fallback. An item in a normalized collection
may therefore have a `value` JSON column when its item type cannot be flattened
safely. JSON remains the canonical runtime boundary; typed columns and relational
worksheets are only an input adapter and are rebuilt into the same canonical tree
before contract validation.

Inputs must be `.xlsx`, contain the generated metadata, and have the exact
generated headers. Formula cells, VBA projects, duplicate case or item
identifiers, duplicate list positions or map keys, orphaned parent references,
unknown columns, and stale hashes are rejected before invocation. A malformed
collection row invalidates its associated case without suppressing unrelated
valid cases. A populated payload column or collection row whose variant is not
selected is rejected. Integer cells must be exact `i64` values; floating-point
cells are never silently rounded into integers.

The normalized input workbook schema is
`futuruna.calculate.xlsx.input.v3`. Earlier workbooks are rejected rather than
silently interpreting their older topology or payload encoding.

## Metadata

Meta comments remain comments. For a contract, tooling includes resolved
references and spans whose symbols intersect the entry or reachable type names.
Roles such as `source`, `warning`, `label`, `unit`, and `help` are conventions.
They can drive explanations, audit indexes, workbook notes, and labels, but the
typed contract remains complete and valid when metadata is absent.

Future field-target metadata may enrich a path such as `TaxInput.monthly_income`.
It must use the generic typed meta-index and cannot add requiredness, alternatives,
or defaults that contradict the Futuruna type.

## Commands

```sh
runa schema model.calculate.runa --entry calculate_tax
runa template model.calculate.runa --entry calculate_tax --format json --output cases.json
runa template model.calculate.runa --entry calculate_tax --format toml --output cases.toml
runa template model.calculate.runa --entry calculate_tax --format xlsx --output cases.xlsx
runa call model.calculate.runa --entry calculate_tax --input cases.xlsx --output results.xlsx
```

`schema` writes JSON to standard output unless `--output` is supplied. `template`
infers the format from `--format` or the output extension. `call` infers input and
output adapters from their extensions; `--format` can select standard-output JSON.

## Evolution and compatibility

The contract schema and every adapter envelope carry independent version tags.
Readers reject unknown major versions. Additive contract fields may appear within
version 1. Any semantic type or endpoint change produces a different hash. A
template with a different hash is stale and reports both expected and actual
fingerprints; it is never coerced silently.

The declaration and CLI are Preview. Promoting them requires production corpus
usage, schema migration evidence, adversarial workbook tests, and differential
agreement between interpreted and generated calculations.
