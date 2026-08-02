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

@ calculate
| calculate(input: Input) -> Result(annual_tax = annual_tax(input))
```

A calculation has exactly one explicitly typed input parameter. A rule result
must infer to one concrete serializable type; a function must declare its result
type. The input is one named domain type. Conditions, defaults, exceptions,
matches, rule scopes, and ordinary rule dependencies continue to work normally.

Use plain `@ calculate`. The annotation does not accept a prompt string. Labels,
help, units, and sources belong in typed meta comments.

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

## Generate Input

```sh
runa template model.calculate.runa --format json --output cases.json
runa template model.calculate.runa --format toml --output cases.toml
runa template model.calculate.runa --format xlsx --output cases.xlsx
```

JSON is the canonical value model. TOML omits absent optional record fields.
XLSX flattens nested named records into columns, gives booleans and nullary enums
constrained choices, and puts each `List`, string-keyed `Map`, or `Set` field in
a separate related worksheet. Integer template cells are text-formatted so all
`i64` values remain exact.

`cases` is the first visible worksheet and contains scalar fields for the named
input record. Every collection row
uses `case_id` and `item_id`; nested collection sheets add `parent_id`. List rows
use one-based `position`, map rows use `key`, and set rows have neither. Leave a
collection sheet without matching rows to supply an empty collection. Hidden
`_futuruna`, `_tables`, and `_columns` sheets record the contract fingerprint,
generated topology, and column types; do not edit them. Optional composite fields
and complex alternatives remain canonical JSON cells when they cannot be
represented without ambiguity.

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

The full format and evolution contract is in
[Typed calculation contracts](../rfcs/typed-calculation-contracts.md).
