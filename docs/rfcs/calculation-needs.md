---
feature_stage: experimental
feature_stage_surfaces:
  - calculation-needs
---

# RFC: Calculation needs

Status: Experimental

## Summary

A calculation case may leave facts out. `runa call --partial` evaluates such a
case and answers either with the result, or with the facts it still needs:

- `Complete`: the result, and the provided input paths evaluation never read;
- `Needs`: every fact and entry that blocks the result, each with its concrete
  input path, the field's interview metadata and the rule waiting for it.

A client answers some of the needs, writes the answers into the same case at
their paths, and calls again. The engine keeps no state between rounds: the
partial case is the whole state of the interview. Every round either completes
or adds at least one answer.

```text
round 1: {}                         -> Needs [taxpayer.married]
round 2: {married: true}            -> Needs [taxpayer.spouse.share_income_ore]
round 3: {married: true, spouse...} -> Complete(result)
```

If a partial case returns `Complete(r)`, every completion of its unknown paths
returns `r`, or stops with an arithmetic-overflow error; no completion returns
a different result.

Without `--partial` nothing changes: every case is complete, a left-out
required field is an input error, and a left-out optional field is `None`.

## Goals

- Ask only for what evaluation actually reaches: a fact that cannot change the
  result at that point is not asked.
- Never return `Complete(r)` unless every completion of the case returns `r`.
- Keep the result machine-stable, so clients can generate types for it.
- Keep the calculation pure: no callbacks, no state between rounds.

## Non-goals

- Deciding relevance by solving: a need is a fact the evaluation reached, not a
  proof that two of its values give different results. Relevance by solver is
  later work.
- Native and compiled programs. Partial evaluation is an interpreter feature of
  `runa call`; native code and generated Rust take complete input only.
- Spreadsheet and TOML input. A blank cell and an omitted TOML key cannot tell
  "none" from "not asked yet", so partial cases are JSON.

## Kinds of missing

| Input at a path | Meaning |
|---|---|
| a value | the answer |
| `{}` for a record | the record exists; its fields are not known yet |
| `null` for an optional field | the answer "none" (no spouse) |
| the field left out | not asked yet: **unknown** |

Decoding a partial case:

- a left-out field, required or optional, decodes to an unknown value carrying
  its concrete path;
- a left-out field whose type is a single-constructor record decodes like `{}`:
  the record must exist, so only its fields are unknown, each at its own path;
- `null` is `None`, as in complete cases;
- `{}` for a record decodes to that record with every field unknown;
- a sum type without `$variant` is unknown at `path.$variant`; once the variant
  is given, its left-out fields are unknown at their own paths. An object with
  payload fields but no `$variant` is an input error;
- a left-out list, set or map is unknown at its path; a given collection has a
  known length and its elements decode like any other value.

Paths are relative to the case's `input`: record fields join with `.`, list
and set elements use `[index]`, map values use `.key`, and a sum type's tag is
`.$variant`. Examples: `taxpayer.married`, `accounts[2].closing_cash_ore`,
`taxpayer.outside_endavu.$variant`.

Every other input rule stays: unknown fields, wrong types, unknown variants and
template placeholders are input errors in partial cases too.

## Evaluation

Carrying an unknown value is always allowed: binding it, passing it to a rule
or function, storing it in a record or list. Reading it propagates:

- an operator on an unknown operand yields an unknown value carrying the union
  of the paths its operands depend on;
- `&&` and `||` use Kleene logic: `False && unknown` is `False`,
  `True || unknown` is `True`, otherwise the result is unknown;
- `if` with an unknown condition evaluates both branches; when both give the
  same known value the `if` is decided, otherwise its result is unknown with
  the condition's paths and the branches' paths;
- `match` on an unknown scrutinee does the same over every arm the scrutinee
  could take;
- rule dispatch tries clauses in priority order. A clause whose guard is
  unknown blocks every clause below it, since it might fire. If every clause
  that might still fire gives the same known value, and the call cannot miss,
  the call is decided; otherwise it is unknown with the blocking guards' paths;
- builtins follow a strictness table: each argument is either needed whole
  (`show`, arithmetic helpers, string functions), needed only as a structure
  (`length` needs the list, not its elements), or passed through (the elements
  given to a higher-order builtin's function). A builtin with an unknown needed
  argument gives an unknown result. A higher-order builtin over a list of
  unknown length gives an unknown result with the list's path. What a
  builtin's function returns is read the same way: `map` and `foldl` carry
  it, `flat_map` needs its list structure, and every other builtin (a
  predicate's Bool, a sort key) needs it whole; an unknown answer makes the
  builtin's result unknown;
- a loop (`for`, `while`) over an unknown list or condition stops the case
  with that need;
- field access on a record reads only that field; field access on an unknown
  value (a sum type whose variant is unknown) gives an unknown result.

An operation that can fail for some completion (a division or remainder by an
unknown divisor, indexing with an unknown index, a value rule that might match
no clause, a branch that fails while its condition is unknown) records its
paths as *fallible*. A case whose result is known but whose evaluation recorded
fallible paths returns `Needs` for those paths instead of `Complete`: a
completion that fails is not a completion that returns `r`.

Arithmetic overflow on unknown operands is not recorded as fallible (see Open
questions and the Guarantee).

### Bounded speculation

Anything evaluated only because a condition is unknown is *speculative*: both
branches of an `if`, the arms of a `match`, and the clauses explored below an
unknown guard. Speculation is bounded so that a partial case always
terminates and cannot grow exponentially:

- speculative evaluation counts its steps against a speculation budget per
  case (100 000 evaluation steps), separate from and inside the calculation's
  ordinary step budget. Once the budget is spent, every speculative
  evaluation of the case gives an unknown result with its condition's paths
  without exploring further;
- a call to a rule or named function that is already on the call stack,
  entered while a speculative evaluation is active, is not evaluated: its
  result is unknown with the paths of every active speculation's condition.
  `| f(n) -> f(n - 1) under n > 0` with `n` unknown therefore needs `n`
  instead of recursing forever.

Both cuts are conservative: they may ask a fact that deeper exploration would
have shown irrelevant, and they never turn a `Needs` into a wrong `Complete`.

## Strict mode

Strict mode stops the case at the first read of an unknown value, through the
calculation's existing failure channel, as a distinct outcome: `Needs` with
that read's paths. It never propagates, so its `Complete(r)` holds by
construction: no unknown value was read. Strict mode is the test oracle for
propagation (`runa call --partial=strict`).

## Result

`runa call --partial` writes an output envelope with schema
`futuruna.calculate.partial-output.v1`. Each case's `result` is one of two
variants.

```json
{ "case_id": "case-1",
  "result": {
    "$variant": "Complete",
    "result": { "annual_tax": 1200 },
    "unused_paths": ["taxpayer.phone"] } }
```

```json
{ "case_id": "case-1",
  "result": {
    "$variant": "Needs",
    "needs": [
      { "kind": "fact",
        "path": "taxpayer.married",
        "label": "Married",
        "question": "Were you married at the end of the year?",
        "help": "Cohabiting spouses only.",
        "unit": "yes/no",
        "sources": [ { "label": "...", "role": "Source", "binding": "pskl_8a" } ],
        "waiting_rule": "transferable_threshold" },
      { "kind": "entry",
        "path": "taxpayer.spouse_share_income_ore",
        "entry": "share_income",
        "subject": "taxpayer.spouse",
        "label": "Spouse's share income",
        "waiting_rule": "transferable_threshold" } ] } }
```

`Complete`:

- `result`: the calculation's output in its canonical JSON encoding;
- `unused_paths`: the provided input paths evaluation never read, sorted. A
  provided path is a leaf the case supplies: a scalar, a `null`, an empty
  collection, or a variant tag at `path.$variant`. Reading a record field reads
  that field's leaves; a record or list consumed as a whole (compared, shown,
  matched, given to a builtin) reads every leaf below it.

`Needs`:

- `needs`: every fact and entry that blocks the result, sorted by `path`, each
  path once;
- `kind`: `fact` when the client answers the path itself, `entry` when the
  field's metadata says another calculation entry provides it;
- `path`: the concrete input path to answer, with list indices;
- `label`, `question`, `help`, `unit`, `sources`: the contract's
  `field_metadata` for the path with list indices removed
  (`accounts[2].closing_cash_ore` takes the metadata of
  `accounts.closing_cash_ore`; a member of an optional record such as
  `taxpayer.spouse.cohabiting_at_year_end` has its own entry); each is omitted
  when the metadata does not give it. `sources` are the contract's source
  references, unchanged;
- `waiting_rule`: the rule or function the fact matters to. When the path is
  first read, the rule families and named functions being evaluated form a
  stack (for a rule family, the clause being tried). A frame's *section* is
  the section (between `--@begin:<section>--` and `--@end:<section>--`) its
  clause, or function body, is declared in, when the section's
  `--@label:<section>` anchor has a `----` quote (an anchor that only carries
  metadata, such as an entry's input guide, makes no section here); a *law
  section* is one whose quote holds a `[sources/...]` document. `waiting_rule` is the
  innermost frame in a law section; without one, the innermost frame in any
  section (a section of input checks, say); without one, the innermost rule
  family, and the calculation entry when no rule was evaluating (a value
  that reached the output unread). A validation helper or the entry itself
  tells a reader little; the rule of the law the fact matters to does;
- `section`, `paragraphs`: the section of `waiting_rule`'s frame and the
  paragraphs cited (`--@cite`, see [Explanations](explanations.md)) on the
  clause being tried, or on the rule's first clause when that clause cites
  none, or on the function; omitted when the frame has no section or nothing
  is cited;
- `entry`, `subject` (entry needs only): the calculation entry that provides
  the value, and the input path of the subject it runs for.

A case with an input error or a calculation failure in a known part of the
evaluation produces a diagnostic, as in complete mode. A `Needs` result is not
a diagnostic; `runa call --partial` exits successfully when every case
returned `Complete` or `Needs`.

## Commands

- `runa call --partial` (or `--partial=propagate`) propagates;
  `--partial=strict` is the oracle; `--partial=check` runs both and the
  completion test and reports every violation as a diagnostic.
- `runa schema --format partial-result` prints the JSON Schema of a partial
  case's `result`.
- Partial cases are read from JSON and still need `input_status: "ready"`:
  readiness acknowledges the supplied facts, not completeness.
- The contract's `field_metadata` carries `provided_by: {entry, subject}` for
  entry-provided fields; it is omitted otherwise, so existing schema
  fingerprints do not change.

## Entry needs

A field may be the result of another calculation for another subject: the
spouse's share income is the spouse's own `share_income` entry. Field metadata
marks it with two optional fields beside `path` and `label`:

```runa
= spouse_share_income_field = CalculationField(
    path = pathof(TaxCase::taxpayer::spouse_share_income_ore),
    label = "Spouse's share income",
    entry = Some("share_income"),
    subject = Some("taxpayer.spouse")
)
```

`entry` names the providing calculation and `subject` the input path of the
person or object it runs for. Both are given or neither. An unknown value of
such a field that blocks the result becomes a need of kind `entry`. The client
runs that entry through the same loop and writes its whole result at `path`.

The field's type must be the providing entry's result type, so that the
entry's result can be written at `path` unchanged. `runa check` (and every
command that extracts the contract) reports an error when the named entry is
not a calculation of the same program, or when the field's contract type
differs from that entry's output type. There is no projection of one field of
the entry's result: a model that needs one figure of another entry declares
that entry with that figure as its result.

Spouses transfer unused amounts both ways, so two entries that need each other
must be one joint household entry; a client resolver tracks the stack of
(entry, subject) pairs and reports a repeat as a model error.

## Guarantee

If a partial case returns `Complete(r)`, every completion of its unknown paths
returns `r`, or stops with an arithmetic-overflow error; no completion returns
a different result. Two checks hold it in place:

- **the completion test**: for a case returning `Complete(r)`, completions of
  its unknown paths are generated from the contract types (boundary and
  seeded pseudo-random values; every variant of a sum type; empty and short
  lists) and each must return `r` through the ordinary, complete calculation,
  or stop with an arithmetic-overflow error. Any other error, and any other
  result, fails the test;
- **the strict-mode oracle**: whenever strict mode returns `Complete(r)`,
  propagation returns `Complete(r)`.

Both run on every partial case of the Futuruna test suites and of the models
that use them.

## Open questions

- **Overflow.** Arithmetic on unknown operands could overflow for extreme
  completions. Recording every arithmetic operation as fallible would block
  `Complete` whenever an unused intermediate amount depends on an unknown
  fact, so overflow is not recorded, and the guarantee admits an
  overflow error as the outcome of a completion.
- **Speculation budget.** The budget of 100 000 steps per case is a fixed
  constant. A model whose relevant facts sit behind deeper speculation is
  asked more facts than it needs, never fewer.
- **Left-out records.** A left-out required record decodes like `{}` (every
  field unknown) rather than as one unknown value, so needs name its fields,
  not the record. A left-out optional record stays one unknown value: whether
  it exists is the first question.
- **Sources.** Needs carry the contract's structured source references, not
  rendered citation strings; the client renders them.
- **Waiting rule.** It is the rule family that first read the path, which for
  a path read in several rules is the first in evaluation order.
- **Speculative failure.** A branch that fails while its condition is unknown
  makes the result unknown and its condition's paths fallible; the failure
  itself is not reported until the condition is known.
