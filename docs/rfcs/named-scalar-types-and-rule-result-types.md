# RFC: Named scalar types and declared rule result types

Status: Part 1 implemented (Experimental, 0.5.0); Part 2 proposed. Builds on [Unit types](unit-types.md),
[Field metadata](field-metadata.md), [Scoped constructors](scoped-constructors.md)
and [One annotation system](one-annotation-system.md).

## Summary

Two additions, each saying a fact once, where it belongs, so that the type
tells the reader what a value is:

1. **Named scalar types.** `# Isin: String "International Securities
   Identification Number" where isin_shape, isin_check_digit` declares a type
   whose values are Strings that pass the checks. An `Isin` is a String
   wherever a String is expected, but a String (or an `AccountId`) is not an
   `Isin`. The type carries its description, its checks and, with `@ field`,
   the label and help of every field that holds it. The checks run at the
   calculation boundary and are reported as input errors with a path. At run
   time, on the wire and in proofs the value is the String; the TypeScript SDK
   brands it.
2. **Declared rule result types.** `| rate(year: Int): Ore -> 0` declares
   the family `rate` to take an `Int` and give an `Ore`. The declaring line
   is the family's first line. The other clauses write only names
   (`| rate(year) -> 2500 under year >= 2024`) and are checked against the
   declaration, the way a `>` body is checked against its `-> T`. A literal
   takes the declared unit, so `-> Ore(0)` becomes `-> 0`.

Units and named scalars are siblings built on one core. A unit is a
**dimension**: a different quantity, closed under its own arithmetic. A named
scalar is a **refinement**: a subset of its primitive. The difference decides
which of them can widen to the primitive and which can carry checks (see
[Units and named scalars](#units-and-named-scalars)).

---

# Part 1: Named scalar types

## Problem

tax-service has no type for an ISIN, an account id, an event id or a CPR
number. Each one is a `String`, so two things are missing.

**The label is written at every field.** `src/aktiesparekonto.calculate.runa`
and `src/facts.runa` write `@ field(label = "ISIN")` on 9 fields,
`@ field(label = "Event id", help = event_id_help, key = True)` on 10 variant
fields and `@ field(label = "Account id")` on 8. The `isin` fields of `Trade`,
`DividendPaid` and `OpeningPosition` in `src/facts.runa` have no label at all,
because the label was not repeated there. Their interview text comes from
exact paths in `src/input_guide.runa` instead:

```runa
-- src/aktiesparekonto.calculate.runa
# AskYearPrice(
    @ field(label = "ISIN", key = True) isin: String,
    @ field(label = "Price", help = price_help)
    price_micro_dkk: MicroDkk
)
[…]
@ field(label = "Event type", help = "Trade, DividendPaid, CashMovement, AccountCharge or PositionChange. Events on the same day are in the order they happened.")
# AskYearEvent =
    Trade(
        @ field(label = "Event id", help = event_id_help, key = True)
        id: String,
        @ field(label = "Trade date") date: Date,
        @ field(label = "ISIN") isin: String,
        […]
    ) |
    DividendPaid(
        @ field(label = "Event id", help = event_id_help, key = True)
        id: String,
        @ field(label = "Payment date") date: Date,
        @ field(label = "ISIN") isin: String,
        […]
```

**Nothing stops an account id from being used as an ISIN.** Every check sees
`String`:

```runa
-- src/validation.runa
> event_issues(income_year: Int, accounts: Map(String, Account), unknown_instrument: Bool, e: Event) -> List(Issue) {
    concat(
        concat(
            issue_when(not(map_contains(accounts, e.account_id)), UnknownAccount, e.id),
```

`map_contains(accounts, e.isin)` passes the check just as well, and is always
`False`.

**What a value must look like is checked inside the model, or not at all.**
The eKapital report (`src/ekapital_ask.runa`) checks its identifiers with
predicates it calls by hand and reports each failure as an issue of the
result:

```runa
| se_number_valid(se: String) -> string_length(se) == 8 && digits_only(se)
| report_id_valid(report_id: String) -> length_between(report_id, 4, 50)
| agreement_number_valid(agreement: String) -> length_between(agreement, 6, 30)
> cpr_valid(cpr: String) -> Bool {
    string_length(cpr) == 10 && digits_only(cpr) && (cpr == "0000000000" || cpr_day_month_valid(unwrap_or(ok_int(substring(cpr, 0, 2)), 0), unwrap_or(ok_int(substring(cpr, 2, 2)), 0)))
}
[…]
                report_issue_when(not(report_id_valid(a.report_id)), InvalidReportId, id),
[…]
                report_issue_when(not(cpr_valid(a.owner_cpr)), InvalidCprNumber, id),
                report_issue_when(not(agreement_number_valid(a.agreement_number)), InvalidAgreementNumber, id)
```

The field's help states the format in prose ("10 digits without a hyphen"). A
predicate states it again in code, and a third place calls the predicate. A
field that nobody remembers to check is not checked: no ISIN in the model is
checked.

## Design

### Declaration

```runa
@ field(label = "ISIN", help = "The 12-character code on the account statement, the trade note or the dividend note.")
# Isin: String "International Securities Identification Number (ISO 6166)" where isin_shape, isin_check_digit

@ field(label = "Account id")
# AccountId: String "the custodian's identifier of an account"

@ field(label = "Event id", help = "Unique within the case.")
# EventId: String "the custodian's identifier of a ledger event"

# Cpr: String "Danish civil registration number, 10 digits without a hyphen" where cpr_valid
# ReportId: String "the report's AktiesparekontoIndberetningEksternID" where length_between(4, 50)
# Month: Int "a month of the year, 1 to 12" where between(1, 12)

| isin_shape(text: String) -> string_length(text) == 12 && characters_in(substring(text, 0, 2), "ABCDEFGHIJKLMNOPQRSTUVWXYZ") && characters_in(text, "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789") && digits_only(substring(text, 11, 1))
```

`characters_in`, `digits_only` and `length_between` are tax-service's own
rules. `between` and `isin_check_digit` (the ISO 6166 check digit) would be
written once in the model in the same way.

`# Name: Primitive "description"` declares a named scalar type. `:` says what
it says everywhere else in Futuruna: the name on the left has the type on the
right. An `Isin` is a String. The primitive is `String` or `Int`. The
description says what a value is. It is required, as a unit's is, and the
contract and the SDK repeat it. The name follows the rules for unit names: it
may not reuse a built-in type name or the name of a declared type,
constructor or unit.

`where` lists the type's checks. `where` is an existing keyword (Danish
`hvor`) that already admits values in exploration queries. Each check is a
pure rule or function that returns `Bool` and whose first parameter takes the
primitive. Further arguments are written after the name, and the value is
passed first, as `|>` passes it: `length_between(4, 50)` checks
`length_between(value, 4, 50)`. Those arguments are pure ground expressions,
resolved like metadata values. A value belongs to the type when every check
is `True`. The checks run in the order written and the first `False` stops
them, so a later check may assume an earlier one: `isin_check_digit` reads
the twelfth character, and `isin_shape` has already shown it is a digit. A
check that fails while it runs (an index out of range, a division by zero) is
an error of the model, reported as one. It is not read as `False`.

A named scalar without `where` is a name and nothing more: `AccountId` keeps
account ids apart from ISINs but accepts any String.

### Rules

For a named scalar `S` over the primitive `P`, another named scalar `S2` over
`P` and a plain `P` value `p`:

| Expression | Type |
|---|---|
| an `S` where an `S` is expected | `S` |
| an `S` where a `P` is expected | accepted: an `Isin` is a String |
| `p` where an `S` is expected | error: write `Isin(p)` or `parse(Isin, p)` |
| an `S2` where an `S` is expected | error |
| a literal (`"DK0060534915"`, `7`) where an `S` is expected | `S`, checked by `runa check` |
| `S == S`, `S != S`, `S < S` and the other comparisons | `Bool` |
| `S` compared with a literal | `Bool`; a literal that fails a check is an error, since no `S` equals it |
| `S` compared with `p` | `Bool`, comparing the `P` values |
| `S` compared with `S2` | error |
| any other operator (`+`, `*`, `%`, string `+`) | applies to `P` and gives `P` |
| `if`, `match` or a list joining `S` with `S2` or `p`, when nothing expects a type | `P` |

So the mistake the type exists for, an account id where an ISIN is expected,
is an error. An ISIN used as text is not: `p.account_id + "/" + p.isin` in
`src/validation.runa` is a String, as it is today. No operator makes an `S`.
The only ways to get one are a literal, a conversion or the calculation
input, and every one of them runs the checks.

A literal takes the named type its place requires, as a number takes a unit.
`runa check` evaluates the checks on the literal, so
`= home: CountryCode = "dk"` is an error in the source, before anything runs.

Containers widen too: a `List(Isin)` is accepted where a `List(String)` is
expected, and so are `Set`, `Option` and both sides of `Map`. A type variable
bound by `S` alone is `S`, so `head(isins)` is an `Isin`. A type variable
bound by `S` and `P` together is `P`.

### Conversions

| Conversion | Meaning |
|---|---|
| `Isin(p)` | `p` as an `Isin`, after the checks; a failure is an undefined value, like a division by zero |
| `parse(Isin, p)` | `Result(Isin, String)`: `Ok` with the value, or `Err` naming the check that failed |
| `String(isin)`, `Int(id)` | the primitive value; rarely needed, since `S` is accepted where `P` is expected |
| `Isin(account_id)` | error: convert between two named scalars through `String(...)`, so that the step is visible |

`Isin("DK0060534915")` with a literal is checked by `runa check`. A program
that declares its own `parse` keeps it, as with `to`.

`parse` is the form for code that reads text, such as tax-service's
`src/codec.runa`:

```runa
> decode_year_end_price(raw: String) -> Result(YearEndPrice, String) {
    = text <- text_field(raw, "isin")
    = isin <- parse(Isin, text)
    = price_ore <- ore_field(raw, "price_ore")
    Ok(YearEndPrice(isin = isin, price_ore = price_ore))
}
```

### Where the checks run

- **`runa check`:** on every literal that takes a named scalar type.
- **The calculation boundary:** on every input value at a position of a named
  scalar type. This covers a field, a list or set element, a map key or value,
  a variant payload and a present optional value. It applies to `runa call`,
  `--partial`, `runa template` and workbook input. A failure is an input
  diagnostic at the value's path. It is reported together with the case's
  other input errors (missing and unknown fields, invalid enum choices), and
  the case is not run:

  ```json
  {"case_id": "demo", "path": "$.events[3].isin", "message": "`\"DK006053491\"` is not an `Isin`: `isin_shape` is false", "scalar": "Isin", "check": "isin_shape"}
  ```

  `scalar` and `check` are new, additive members of a diagnostic. In a
  partial case a missing value is still a need. A present value that fails
  is an input error, never a need.
- **Run time:** in `Isin(p)` and `parse(Isin, p)`.

Results are never checked. Every `S` value was made by one of the three paths
above, so a result is valid by construction.

### Field metadata

`@ field` before a named scalar declaration is the type's default (level 3 in
[Field metadata](field-metadata.md#precedence)). This is the same rule that
applies before a data type today. It describes every input field that holds
the type, also inside `List`, `Set`, `Option` and `Map`. The precedence
rule does not change: the most specific metadata wins whole. Exact paths
still win, so the three ISIN questions in `src/input_guide.runa`, each with
its own help ("on the trade note", "on the dividend note"), stay as they are.

A field whose `@ field` says exactly what its named scalar's `@ field` says
is an error. It says the same thing a second time, as `unit = ...` does on a
unit-typed field. A field that says something else overrides the type's
entry, as today: `@ field(label = "Owner's CPR number") owner_cpr: Cpr`.

**`key` stays on the field.** A key belongs to the record, not to the type of
its value. tax-service shows this: `isin` is the key of `AskYearPrice`,
`AskYearInstrument`, `AskOpeningPosition`, `Instrument` and `YearEndPrice`.
In `Trade`, `DividendPaid` and `PositionChange` it is an ordinary field and
`id` is the key. A key declared on `Isin` would make `Trade` a record with two
keys.

Today an `@ field` on a field is a whole metadata entry. A level-2 entry
written only to set `key` would therefore hide the type's label behind an
entry with no label, and that is an error. This RFC treats `key` as what the
contract already treats it as: structure, written on the record's type
definition and not in `field_metadata`. An `@ field` whose only argument is
`key` marks the key and takes no part in metadata precedence, so the field
keeps its type's label:

```runa
# AskYearPrice(
    @ field(key = True) isin: Isin,
    @ field(label = "Price", help = price_help)
    price_micro_dkk: MicroDkk
)
```

An `@ field` with `key` and presentation keys is a level-2 entry, as today.
A `key` field must be a `String` or an `Int`, and a named scalar over either
qualifies.

### Runtime, wire and proofs

A named scalar value is its primitive everywhere outside the checker.

- **The interpreter** stores the String or Int. Before the program runs,
  elaboration (the step that also writes unit conversions as `Int`
  arithmetic) writes `Isin(p)` and `parse(Isin, p)` as the checks they stand
  for, with their source positions. The interpreter, `runa call`,
  `runa verify` and generated Rust therefore run one definition of each
  check.
- **`runa call`** reads and writes the same JSON strings and numbers.
- **Proofs.** `runa verify`, `runa audit` and the solver erase `S` to `P`.
  For an argument, field or input of type `S`, they assume its checks where
  the checks are inside the translatable subset. A check outside the subset
  adds no assumption. That is sound: the solver then considers more values,
  not fewer. `Isin(p)` is an obligation, like a zero divisor. For an `Int`
  scalar this changes what the audit reports. Today
  [the audit](../reference/runes.md#auditing-clause-reachability) "does not
  know which inputs a model admits". A family over `month: Month` whose
  guards cover months 1 to 12, with no default, no longer has a gap at month
  13, because 13 is not a `Month`.
- **Generated Rust** declares `pub type Isin = String;`, as it declares
  `pub type Ore = i64;`.

The declaration's name, primitive and checks are meaning: they are part of
the type's content hash and of the module's semantic interface, because
changing a check changes which inputs are accepted. The description and
`@ field` are presentation, as for data types.

## The contract and the SDK

`runa schema` marks a field of a named scalar type additively, as it marks a
unit:

```json
"type": {"kind": "primitive", "name": "String", "scalar": "Isin"}
```

and lists the named scalars it uses:

```json
"scalars": [
  {"name": "Cpr", "of": "String", "description": "Danish civil registration number, 10 digits without a hyphen", "checks": ["cpr_valid"]},
  {"name": "Isin", "of": "String", "description": "International Securities Identification Number (ISO 6166)", "checks": ["isin_shape", "isin_check_digit"]},
  {"name": "ReportId", "of": "String", "description": "the report's AktiesparekontoIndberetningEksternID", "checks": ["length_between(4, 50)"]}
]
```

A check appears as it is written. The fingerprint covers the list. Consumers
that ignore unknown members read the same contract as before. This respects
the rule of [Typed calculation contracts](typed-calculation-contracts.md):
validity comes only from the types, "UI labels, units, examples, and source
citations never determine validity". A named scalar's checks are part of its
type, and its label is still only presentation.

`runa sdk ts` brands a named scalar as it brands a unit:

```ts
/**
 * International Securities Identification Number (ISO 6166).
 * The engine checks: isin_shape, isin_check_digit.
 */
export type Isin = string & { readonly __scalar: "Isin" };
```

`schema.ts` holds `IsinSchema` and the constructor `isin(text)`. For an
`Int` scalar the constructor throws a `RangeError` unless the number is a safe
integer, as `ore(n)` does. Wherever the contract says `Isin` (fields, results,
the answers of `<entry>SetAt` and the resolvers), a bare `string` or an
`AccountId` is a type error. An `Isin` is accepted where `string` is expected,
because TypeScript's intersection types widen exactly as the checker does.

The SDK cannot run Futuruna checks, so `isin(text)` brands without running
them, and the TSDoc says which checks the engine runs. When the engine
refuses a value, its diagnostic carries `scalar` and `check`. The SDK throws
`CalculationError` of kind `input` with the issue at the TypeScript path
(`events[3].isin`), the same kind and path as a validation failure that the
SDK finds itself.

## What the writer writes and what the reader sees

The writer writes, once per kind of value, what it is, what it looks like
and how it is asked for. A field says only what is particular to it:

```runa
@ field(label = "ISIN", help = "The 12-character code on the account statement, the trade note or the dividend note.")
# Isin: String "International Securities Identification Number (ISO 6166)" where isin_shape, isin_check_digit

@ field(label = "Event id", help = "Unique within the case.")
# EventId: String "the custodian's identifier of a ledger event"

# AskYearPrice(
    @ field(key = True) isin: Isin,
    @ field(label = "Price", help = price_help)
    price_micro_dkk: MicroDkk
)

@ field(label = "Event type", help = "Trade, DividendPaid, CashMovement, AccountCharge or PositionChange. Events on the same day are in the order they happened.")
# AskYearEvent =
    Trade(
        @ field(key = True) id: EventId,
        @ field(label = "Trade date") date: Date,
        isin: Isin,
        side: Side,
        […]
    ) |
    DividendPaid(
        @ field(key = True) id: EventId,
        @ field(label = "Payment date") date: Date,
        isin: Isin,
        […]
```

```runa
-- src/validation.runa
> event_issues(income_year: Int, accounts: Map(AccountId, Account), unknown_instrument: Bool, e: Event) -> List(Issue) {
    concat(
        concat(
            issue_when(not(map_contains(accounts, e.account_id)), UnknownAccount, e.id),
```

Here `map_contains(accounts, e.isin)` is an error. The reader of `isin: Isin`
knows what the value is without reading the field name. In the contract and
the SDK they see `Isin` with its description and checks. A malformed ISIN in
a case comes back as an input error at its path, before any rule runs.

For the eKapital report, `owner_cpr: Cpr` makes the model's
`report_issue_when(not(cpr_valid(a.owner_cpr)), InvalidCprNumber, id)` dead:
no `Cpr` fails `cpr_valid`. Whether a CPR belongs on the type or in the
report's issues is the writer's choice, and the choice changes the product.
With the type, a bad CPR stops the whole case at the boundary with a path
(`$.accounts[3].owner_cpr`). With the issue, the case runs and returns a
typed rejection that lists `InvalidCprNumber`. The rule of thumb is this: a
check belongs on the type when a value that fails it is not a value of that
kind at all, because a nine-digit "CPR number" is not a CPR number. A rule
that judges a valid value, such as a CPR whose owner was not fully tax
liable, stays in the model.

## Migration

Which `String` is an ISIN is the writer's decision. `runa fmt --migrate`
cannot make it. The steps:

1. Declare the named scalars with their descriptions, checks and type-level
   `@ field`.
2. Change the field and parameter types. tax-service has 20 `isin: String`
   and 32 `account_id: String` fields and parameters, plus the event ids,
   `issuer_country`, `owner_cpr`, `report_id` and `agreement_number`.
3. `runa check` then lists every place where a plain `String` flows into a
   named scalar. Each one is either already a named value whose type should
   change too, text being read (`parse(Isin, text)` in `src/codec.runa`), or
   a literal, which is checked.
4. `runa fmt --migrate` removes each field-level `@ field` that says exactly
   what its named scalar's `@ field` says. If such an `@ field` set `key`, it
   becomes `@ field(key = True)`. This step is mechanical.
5. The writer decides for each model-level validity check whether it moves to
   the type, as described above.

## Alternatives rejected

- **A transparent alias (`# Isin = String`).** It gives the name without the
  distinction, and `=` in a `#` declaration introduces constructors and
  subsets, not types.
- **A record wrapper (`# Isin(value: String)`).** It changes the wire to
  `{"value": "DK…"}`, puts `.value` at every use and boxes the value at run
  time.
- **One mechanism for units and named scalars.** Arithmetic has opposite
  needs in the two (see [Units and named scalars](#units-and-named-scalars)).
- **Strict named scalars that never widen.** Every use as text would need
  `String(isin)`: `p.account_id + "/" + p.isin`, the eKapital XML writer, every
  string function. Those conversions state nothing a reader needs. The
  mistake the type catches is narrowing, and narrowing stays explicit.
- **Declarative facets (`length = 12`, `pattern = "…"`) instead of
  predicates.** They cannot express the ISIN check digit or the CPR's day and
  month, and they would be a second, weaker language for what a `|` rule
  already says. Facets that an SDK could check locally are an
  [open question](#open-questions).
- **A message per check (`where cpr_valid "…"`).** The check's name and
  arguments say what failed (`length_between(4, 50) is false`). Text for a
  person is presentation and belongs in `help`, where it can be translated.
- **`key` on the type.** Disproved by tax-service: `isin` is the key of five
  records and an ordinary field of three.
- **Checking results.** Every value of a named scalar was checked when it was
  made.

## Decisions

Recorded while implementing Part 1 (Experimental in 0.5.0):

- **A check is the definition the declaration resolved.** Elaboration
  declares one function per check beside the type, in the declaring file:
  `__scalar_check_Isin_0(value)` is `isin_shape(value)`, with the check's
  name and arguments resolved there. `Isin(p)`, `parse(Isin, p)` and the
  calculation boundary call these functions, never the check by name in the
  file that converts, so a function of the same name in another file never
  stands in for the check. `runa check` runs a literal's checks with the
  declaring file's definitions too.
- **Elaboration trusts its own `Isin(v)`.** The `Isin(v)` that ends an
  elaborated conversion has no source position; the checker accepts a
  conversion without a position as already checked, and at runtime it leaves
  the value as it is.
- **`String` is a runtime identity**, like `Int`: `String(isin)` returns its
  argument.
- **In joins only `Int` literals take the scalar.** `if c { isin } else
  { "DK0060534915" }` joins to `String`; a String literal branch is text
  until something expects an `Isin`.
- **A check that cannot run at check time is an error at the literal**, not
  a silent pass.
- **The repeated-label rule ignores `key`.** A field's `@ field` that says
  what its type's says, apart from `key`, is an error; `@ field(key = True)`
  alone parses without a `label`, marks the key and keeps the type's
  metadata.
- **Boundary errors are input errors.** A value that fails a check is a
  diagnostic with `scalar` and `check`, reported beside the case's decoding
  errors; the TypeScript SDK throws `CalculationError` of kind `input` with
  `scalar`, `check` and the issue at the TypeScript path. `runa template`
  carries a draft input over as it is; `runa call` refuses a failing value.

## Open questions

- **Checks in the SDK.** Checks inside a small subset (length, character
  classes, ranges) could be compiled to Zod refinements, or the SDK could ask
  the WebAssembly engine. Until then `isin(text)` brands without checking.
- **Refinements of named scalars** (`# DanishIsin: Isin where starts_with_dk`).
  These follow from the subtyping above, but they need a rule for the
  contract's `of`.
- **Members on named scalars** (`isin.issuer_prefix()`), as product types
  have rule members.
- **Other primitives.** `Float` is left out for the reason units leave it
  out. A two-valued `Bool` scalar is an enum. `Date` is a record.
- **Bounds on units at the boundary.** A unit cannot carry checks (see
  below), but "this amount is never negative" is a fact about an input
  field. A field-level check could cover it without making it an invariant of
  the unit.
- **Combining a type's label with a path's question.** Metadata precedence is
  whole-entry, so a path that adds a `question` must repeat the type's
  `label`.

---

# Part 2: Declared rule result types

## Problem

A rule family has no place to say its type. The checker reads the type from
the first clause whose value has a known type (`infer_rule_return_types` in
`src/lib.rs`). Every clause writes its parameter types again. The writer
works around the missing declaration, and the reader reconstructs it.

**A constant clause needs a conversion to get a unit.** It is the only clause
of its family, and nothing else gives `0` a unit:

```runa
-- src/ask.runa
@ cite(§ 30, nr. 3)
| ask_danish_dividend_tax_expected_ore() -> Ore(0)
```

**The type is in another clause.** In the following family, the type appears
seven lines below the first clause, inside `Some(YearParameters(...))`:

```runa
-- src/parameters.runa
| year_parameters(income_year: Int) -> None
| year_parameters(income_year: Int) -> Some(YearParameters(
    income_year = 2025,
    share_income_threshold_ore = 6750000,
    […]
)) under income_year == 2025
```

In a RuleScope the default is `0` and the type comes from the exception's
`min_int(...)`:

```runa
-- src/frit_depot.runa, # ShareIncomeCase(...) { ... }
    @ cite(§ 13 A, stk. 3)
    | loss_to_spouse_ore() -> 0
    | exception spouse_absorbs_excess_loss loss_to_spouse_ore() -> min_int(excess_loss_ore(), spouse_listed_net_ore) under spouse_cohabiting && spouse_listed_net_ore > 0
```

**Parameter types are written at every clause:**

```runa
-- src/ask.runa
@ cite(§ 17, stk. 1)
| ask_period_end(income_year: Int, closed: Bool, closed_on: Date) -> Date(year = income_year, month = 12, day = 31)
@ cite(§ 17, stk. 2; § 12)
| exception closed ask_period_end(income_year: Int, closed: Bool, closed_on: Date) -> closed_on under closed
```

```runa
-- src/facts.runa
| days_in_month(year: Int, month: Int) -> 31
| days_in_month(year: Int, month: Int) -> 30 under month == 4 || month == 6 || month == 9 || month == 11
| days_in_month(year: Int, month: Int) -> 28 under month == 2 && leap_year(year) == False
| days_in_month(year: Int, month: Int) -> 29 under month == 2 && leap_year(year)
```

tax-service's `src/` has about 29 families with more than one clause, 83
clauses in all (counted by name). The 23 families that take parameters type
them on every clause, so 46 clauses restate the parameter types of the clause
above them.

**The name carries the type.** `ask_value_at_period_end_ore`,
`loss_to_spouse_ore` and `threshold_ore` put the unit in the name because the
reader cannot see the type anywhere else.

**An error lands on the wrong clause.** Because the first typable clause sets
the family's type, a mistake in that clause (say, a `Micro` where an `Ore` was
meant) is reported against the correct clauses that follow it.

## Design

### The declaring line

```runa
| days_in_month(year: Int, month: Int): Int -> 31
| days_in_month(year, month) -> 30 under month == 4 || month == 6 || month == 9 || month == 11
| days_in_month(year, month) -> 28 under month == 2 && leap_year(year) == False
| days_in_month(year, month) -> 29 under month == 2 && leap_year(year)
```

`: R` after a rule head declares the family's result type, and `name: Type`
in the head declares a parameter's type. Both are written on the family's
**declaring line**: its first line in source order, in the scope that holds
its ordinary clauses. The declaring line may not be an exception, because an
exception overrides a rule and does not declare one. A family is identified
by scope, name and arity, as dispatch identifies it.

The declaring line is usually the first clause. When the first clause cannot
carry the types, because it matches a pattern at a typed position, the
declaring line is a declaration alone:

```runa
| fib(n: Int): Int
| fib(0) -> 0
| fib(1) -> 1
| fib(n) -> fib(n - 1) + fib(n - 2)
```

A line with `: R` and no `->` is a declaration. A fact never has `: R`, so
the two cannot be confused. A declaration is not a clause: `@ cite` and a
clause's `@ meta` do not stand before it. Annotations that describe the
family do, namely `@ calculate`, `@ export` and a family `@ meta`. Those
already stand before the family's first clause today.

**`:` and not `->`.** In a `|` rule, `->` is followed by a value:
`| rate() -> 25`. The form `| rate(year: Int) -> Ore` would therefore read as
a clause whose value is `Ore`, and it would differ from the constant clause
`| rate(year: Int) -> Ore(0)` only by `(0)`. Elsewhere in Futuruna `:` is what
says "has the type": a field `isin: Isin`, a parameter `year: Int`, a binding
`= fee: Ore = 250`. A rule line now reads like a binding:
`| fee(): Ore -> 250`. `>` functions keep `-> T`, because there the arrow is
followed by a type and a braced body, so it is never ambiguous. Each rune
keeps its own arrow. A rule head always has parentheses (`rate()`), and that
separates it from a named invariant `| name: subject -> predicate`.

### Types once

- Types are written on the declaring line only. A later clause writes a name
  or a pattern at each position, and so does an exception, also in an
  importing module. A type on a later clause is an error that points to the
  declaring line.
- A name on a later clause must be the declaring line's name for that
  position. The audit's witnesses, explanations and the reader then call the
  parameter by one name.
- Each part is optional. A declaring line may type its parameters and leave
  the result to inference, or the reverse. Whatever is written is written
  there.
- One declaring line per family. A second one is an error that names the
  first. A declaration with no clause is an error. A family still needs an
  ordinary clause or default for its exceptions, as today.
- A family without `: R` is inferred as today. Declarations are not required.
  A one-clause rule's value already states its type
  (`| taxable_income() -> person.gross_income`), as `= x = expr` does, and
  requiring a declaration would make the writer say it twice.

### Checking the clauses

Each clause's value is checked against `R` as the expected type, as a `>` body
is checked against its `-> T`:

- a literal takes `R`'s unit or named scalar. `| loss_to_spouse(): Ore -> 0`
  needs no conversion, and `| home_country(): CountryCode -> "DK"` is checked
  by `runa check`;
- `None`, `[]` and a constructor take `R`: `-> None` is a `YearParameters?`,
  and a shared constructor gets its owner from `R`;
- a clause whose value has a known type other than `R` is an error at that
  clause, and the message names the declaration;
- a fact (`| parent("a", "b")`) and a goal body
  (`-> resident(p), has_income(p)`) give `Bool`, so they require `R = Bool`;
- a clause whose type the checker cannot know is checked when it runs, as a
  calculation's declared result is.

A family declared `: Bool` returns `False` when no clause applies. That is
the behaviour today when the checker can infer a safe `Bool`; with a
declaration it follows from the declaration. A family of any other type
still stops with `no value rule matched`.

### Interactions

- **Units.** `-> Ore(0)` becomes `-> 0`. Mixing literal and unit clauses
  ([unit types](unit-types.md#rules)) no longer depends on another clause
  supplying the unit.
- **Named scalars.** `| isin_of(e: Event): Isin? -> None` takes an `Isin`
  in each clause. A computed `String` is an error that asks for `Isin(...)`.
- **Scoped constructors.** The [resolution order](scoped-constructors.md#resolution-order)
  gains one entry and sharpens another. Under step 3, a clause's value takes
  the family's declared result type. Step 5 becomes: a rule argument takes the
  type the declaring line gives that parameter, or, without one, the type the
  clauses give it. A constructor pattern in a later clause's head takes its
  owner from the declared parameter type.
- **RuleScopes.** A member declares on its own first line inside the body.
  The declared type is what `case.loss_to_spouse()` has for callers, and it is
  the scope's interface. Fields stay visible in every clause. Dispatch still
  uses name and arity, so a member with another arity is another family with
  its own declaration.
- **Exceptions across modules.** An exception in an importing module is
  checked against the declaring line in the imported one.
- **Generics.** Lowercase type variables work as in `>` functions:
  `| first_or(xs: List(a), fallback: a): a -> fallback`.
- **Proofs.** `runa verify`, `runa audit` and Explore take the family's sort
  and its parameters' sorts from the declaration. Today they "fail closed" on
  unresolved result sorts. A declared family whose parameters are untyped
  today becomes translatable.
- **Contract.** For a `@ calculate` rule entry, the contract's output type is
  the declared `R`, not an inferred one. The JSON is the same.
- **Interfaces.** A declared `R` and declared parameter types are the family's
  entry in the semantic interface. Editing a clause can no longer change the
  interface. A changed clause that disagrees with the declaration is an error
  at that clause.

## What the writer writes and what the reader sees

```runa
-- src/ask.runa
@ cite(§ 30, nr. 3)
| ask_danish_dividend_tax_expected_ore(): Ore -> 0

@ cite(§ 17, stk. 1)
| ask_period_end(income_year: Int, closed: Bool, closed_on: Date): Date -> Date(year = income_year, month = 12, day = 31)
@ cite(§ 17, stk. 2; § 12)
| exception closed ask_period_end(income_year, closed, closed_on) -> closed_on under closed
```

```runa
-- src/parameters.runa
| year_parameters(income_year: Int): YearParameters? -> None
| year_parameters(income_year) -> Some(YearParameters(
    income_year = 2025,
    […]
)) under income_year == 2025
```

```runa
-- src/frit_depot.runa, # ShareIncomeCase(...) { ... }
    @ cite(§ 13 A, stk. 3)
    | loss_to_spouse_ore(): Ore -> 0
    | exception spouse_absorbs_excess_loss loss_to_spouse_ore() -> min_int(excess_loss_ore(), spouse_listed_net_ore) under spouse_cohabiting && spouse_listed_net_ore > 0
```

The writer gives each fact once: the parameter types and the result on one
line, and the cases below it. The reader finds a rule's type in one place,
the family's first line, and reads it as they read a binding. An error points
at the clause that disagrees with the declaration. Once a rule's type is
visible, the `_ore` suffix is no longer the only place the unit is written. A
model may drop the suffix, but that is a rename and not part of this change.

## Migration

`runa fmt --migrate` rewrites a family when the checker resolves one concrete
result type for it:

1. A family with more than one clause in its home scope, where every clause
   writes the same parameter types and names: the first clause that is not an
   exception keeps its types and gains `: R`, and the other clauses lose
   their types.
2. A family with a clause whose value is a conversion of a literal to a unit
   or named scalar (`-> Ore(0)`): the declaring line gains `: Ore` and the
   clause becomes `-> 0`.
3. A family whose clauses write different names or types at one position,
   whose first line is an exception, or whose first clause matches a pattern
   at a typed position is reported and not rewritten. The writer adds a
   declaration.
4. Every other one-clause family is left as it is.

In tax-service this covers the multi-clause families counted above, except
any that rule 3 reports, and `ask_danish_dividend_tax_expected_ore`. The
migration is idempotent and changes no behaviour: the declared types are the
types the checker inferred.

## Alternatives rejected

- **`| rate(year: Int) -> Ore`.** `->` in a rule is followed by a value. The
  declaration and the clause `-> Ore(0)` would differ by `(0)`, and a
  constructor name is itself a value.
- **A separate signature line for every declared family.** It adds a line to
  each family and doubles every one-line RuleScope member. The
  declaration-only line remains for the case that needs it, a first clause
  that matches a pattern.
- **A `>` signature with `|` clauses (`> rate(year: Int) -> Ore` without a
  body).** `>` says what happens. A bodiless top-level `>` would be a third
  meaning of `>`, and the reader would have to look at two runes for one
  rule.
- **Types on every clause, now with `: R` as well.** That is today's practice
  for parameters, extended to the result. It says the type once per clause,
  and the clauses can disagree.
- **A block (`| rate(year: Int): Ore { | ... }`).** A family's exceptions may
  stand in an importing module, and its clauses carry their own `@ cite`. A
  block would force them into one place.
- **Requiring a declaration on every family.** Most rules are one clause
  whose value states its type.

## Open questions

- **Required declarations at module boundaries.** A family that another module
  calls or overrides could be required to declare its type, so that the
  module's interface is written down and not inferred.
- **A declared parameter type as a search domain.** Logic queries over a
  declared family (`findall(x, eligible(x))`) could take a declared enum or
  subset type as the domain to enumerate.

---

# Units and named scalars

The two share one core:

| | Unit (`# unit Ore "DKK øre" = DKK / 100`) | Named scalar (`# Isin: String "…" where …`) |
|---|---|---|
| Is | a dimension: a different quantity | a refinement: a subset of its primitive |
| Primitive | `Int` | `String` or `Int` |
| In the checker | distinct type | distinct type |
| At run time, on the wire, in proofs, in Rust | `Int` | the primitive |
| Literal | takes the unit | takes the type; checked by `runa check` |
| Make one | `Ore(n)`, free | `Isin(p)` or `parse(Isin, p)`, checked |
| Take it off | `Int(x)` | `String(x)`, or implicitly where `P` is expected |
| Where `P` is expected | error | accepted |
| Operators | unit algebra: `U + U` is `U`, `U * n` is `U`, `U / U` is `Int` | none of its own; operators apply to `P` |
| Scale and conversions | `= Base / n`, `to`, `to_nearest`, … | none |
| Checks (`where`) | none | yes |
| Type-level `@ field` | no | yes |
| Contract | `"unit": "Ore"`, `units` | `"scalar": "Isin"`, `scalars` |
| SDK | `number & { readonly __unit: "Ore" }` | `string & { readonly __scalar: "Isin" }` |

The core is the same: a name that only the checker sees, a description the
contract and the SDK repeat, a brand, a literal that takes its type from its
place, and conversions named after the type. Elaboration erases both. One
implementation serves both kinds.

The difference is arithmetic, and it decides the rest:

- **A unit has arithmetic, so it cannot have checks.** A check is an
  invariant of a type only if every way to make a value of the type passes
  through it. `a - b` makes a new `Ore` from two `Ore`s without any
  conversion. A unit with `where non_negative` would therefore claim
  something its own arithmetic breaks, unless every operation were checked
  at run time. A named scalar has no operation that makes an `S`, so its
  checks hold for every value.
- **A unit cannot widen, and a named scalar can.** `Ore + n` must be an
  error: the mistake units catch is a plain number taken for an amount, so an
  `Ore` cannot be a plain `Int`. An ISIN, on the other hand, is a String, and
  a String function may read it. Narrowing is the dangerous direction for
  both, and both require it to be written.
- **The scale is not what separates them.** `# unit Bp "basis points"` has
  no scale and still has unit arithmetic. A named `Int` such as an account
  number has no arithmetic: `account + 1` gives a plain `Int`, not an account
  number. What makes a unit is its algebra.
- **A type-level label fits a named scalar and not a unit.** Every `Isin`
  field asks for an ISIN, so one label serves all of them. `Ore` fields hold
  cash, prices and tax, so one label would be wrong for most of them. A
  unit's description already supplies its fields' `unit` text.

`# unit` keeps its declaration and its contract members. Named scalars reuse
its machinery: erasure, the elaboration of conversions, branding and the
literal rule.
