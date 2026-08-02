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

## Generate Input

```sh
runa template model.calculate.runa --format json --output cases.json
runa template model.calculate.runa --format toml --output cases.toml
runa template model.calculate.runa --format xlsx --output cases.xlsx
```

JSON is the canonical value model. TOML omits absent optional record fields.
XLSX flattens nested named records into columns, gives booleans and nullary enums
constrained choices, and stores lists and complex alternatives as canonical JSON
cells. Integer template cells are text-formatted so all `i64` values remain exact.

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
non-exact integers, invalid enum choices, missing required fields, and malformed
canonical JSON are rejected before their case runs.

The full format and evolution contract is in
[Typed calculation contracts](../rfcs/typed-calculation-contracts.md).
