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
- `Needs`: the facts and entries that block the result as it stands, each
  with its concrete input path, the field's interview metadata and the rules
  of the law waiting for it; and, as `later`, the facts evaluation read only
  behind an unknown condition, which answering the needs may make needs.

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
  given to a higher-order builtin's function). The map builtins (`map_from`,
  `map_get`, `map_get_or`, `map_contains`, `map_len`) need the keys whole and
  carry the values. A builtin with an unknown needed
  argument gives an unknown result. A higher-order builtin over a list of
  unknown length gives an unknown result with the list's path. What a
  builtin's function returns is read the same way: `map` and `foldl` carry
  it, `flat_map` needs its list structure, `any`, `all` and `find` read its
  answers in Kleene logic, and every other builtin (`filter`'s Bool, a sort
  key) needs it whole; an unknown answer makes the builtin's result unknown;
- `any` and `all` scan past an unknown answer: a `True` answer decides `any`
  and a `False` answer decides `all` even after unknown ones, otherwise the
  result is unknown with every unknown answer's paths. `find` is decided by a
  match with no unknown answer before it; a match after unknown answers gives
  an unknown result with their paths, since one of them might be the first;
- `sort` reads what its comparisons reach: items ordered by known parts (a
  key and a position before a record with unknown fields) sort into a known
  list that carries the unknown parts. A comparison that reaches an unknown
  part makes the result unknown with every unknown path of the list;
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
  entered under a speculative evaluation that began inside the activation it
  recurs from, is not evaluated: the recursion depends on an unknown
  condition, and its result is unknown with the paths of every active
  speculation's condition. `| f(n) -> f(n - 1) under n > 0` with `n` unknown
  therefore needs `n` instead of recursing forever. A recursion whose own
  conditions are known (a helper that splits a known list) runs as it would
  in every completion, also under an outer speculation, within the budget.

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
        "waiting": [
          { "rule": "transferable_threshold", "section": "pskl_8a",
            "paragraphs": [ { "paragraph": "§ 8 a, stk. 1",
                              "source": "sources/04-personskatteloven.txt",
                              "act": { "title": "Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)",
                                       "identifier": "LBK nr. 1284 af 14/06/2021",
                                       "url": "https://www.retsinformation.dk/eli/lta/2021/1284" } } ] } ] },
      { "kind": "entry",
        "path": "taxpayer.spouse_share_income_ore",
        "entry": "share_income",
        "subject": "taxpayer.spouse",
        "label": "Spouse's share income",
        "waiting": [ { "rule": "transferable_threshold", "section": "pskl_8a" } ] } ],
    "later": [
      { "kind": "fact",
        "path": "taxpayer.spouse.cohabiting_at_year_end",
        "label": "Cohabiting at year end",
        "waiting": [ { "rule": "spouse_transfer", "section": "pskl_8a" } ] } ] } }
```

`Complete`:

- `result`: the calculation's output in its canonical JSON encoding;
- `unused_paths`: the provided input paths evaluation never read, sorted. A
  provided path is a leaf the case supplies: a scalar, a `null`, an empty
  collection, or a variant tag at `path.$variant`. Reading a record field reads
  that field's leaves; a record or list consumed as a whole (compared, shown,
  matched, given to a builtin) reads every leaf below it.

`Needs`:

- `needs`: the facts and entries that block the result as it stands, sorted
  by `path`, each path once. They are not every fact the case will need:
  evaluation does not see past an unknown condition it cannot finish (see
  `later`), so answering the needs can reveal more, as round 2 above does;
- `later`: the facts evaluation read only behind an unknown condition, in a
  speculation (see Bounded speculation) whose branching site ended undecided
  because a branch failed, was cut, or another branch was undecided, and that
  are not among `needs`; sorted by `path`, each with the fields of a need.
  They may become needs once the needs are answered; a client may ask them
  in the same round. `later` is what bounded speculation reached, not every
  fact behind the condition, and is omitted when empty;
- `kind`: `fact` when the client answers the path itself, `entry` when the
  field's metadata says another calculation entry provides it;
- `path`: the concrete input path to answer, with list indices. A `Date`
  (the standard record of `year`, `month` and `day`) is one fact: an unknown
  year, month or day is a need at the date's path, answered with the whole
  date, and the needs of its parts are one;
- `keys`: for each list element the path lies in whose record type has a key
  field (a `String` or `Int` field declared `@ field(..., key = True)`, which
  the contract's definition of the type names as `key`) and whose key the
  case gives, outermost first: the element's `path`, the key `field` and its
  `value`. A backend maps `opening_positions[0]` to its ISIN without relying
  on positions; omitted when there is none;
- `label`, `question`, `help`, `unit`, `sources`: the contract's
  `field_metadata` for the path with list indices removed
  (`accounts[2].closing_cash_ore` takes the metadata of
  `accounts.closing_cash_ore`; a member of an optional record such as
  `taxpayer.spouse.cohabiting_at_year_end` has its own entry; a date takes
  the metadata of the date field); each is omitted when the metadata does not
  give it. `sources` are the contract's source references, unchanged;
- `waiting`: the rules of the law that wait for the fact, never empty. Every
  time evaluation reads the unknown value, or a value computed from it, the
  rule families and named functions being evaluated form a stack (for a rule
  family, the clause being tried); every such stack counts, also inside
  speculation. A frame's *section* is the section (between
  `--@begin:<section>--` and `--@end:<section>--`) its clause, or function
  body, is declared in, when the section's `--@label:<section>` anchor has a
  `----` quote (an anchor that only carries metadata, such as an entry's
  input guide, makes no section here); a *law section* is one whose quote
  holds a `[sources/...]` document; a frame is *cited* when its clause, or the
  rule's first clause when that clause cites none, or its function carries a
  `--@cite` (see [Explanations](explanations.md)). Each stack names one
  consumer: its innermost cited frame of a law section; without one, its
  innermost frame of a law section; then of any section (a section of input
  checks, say); then its innermost rule family. `waiting` lists the consumers
  of the best of these kinds any stack gives, each once, in the order the
  stacks were first seen: when some rule of the law cites paragraphs, an
  uncited helper (a rounding function, a validation check) that also read
  the value is not listed. Without any, it is the calculation entry (a value
  that reached the output unread). Each consumer has:
  - `rule`: the rule or function;
  - `section`: its section, omitted for a frame in no section;
  - `paragraphs`: the paragraphs its clause or function cites, each with its
    `paragraph` label (`§ 9, stk. 3`), the quoted `source` document it was
    found in, and that document's `act`; omitted when nothing is cited;
- `act`: the act a quoted document is, read from the document's header: a
  block of `Key: value` lines between a first line `---` and the next `---`,
  whose `Title:` gives `title` (required), `Document identifier:` gives
  `identifier` and `Human-readable URL:` (else `URL:`) gives `url`. The
  document is found at its `[sources/...]` path beside the model file, or in
  the nearest directory above it that holds it; a document without such a
  header has no `act`. In the WebAssembly engine the document must be among
  the model's provided sources (its header suffices);
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
- **Speculative failure.** A branch that fails while its condition is unknown
  makes the result unknown and its condition's paths fallible; the failure
  itself is not reported until the condition is known. What the failed
  branch read is reported in `later`.

## Decisions

- **`later` rather than a narrower promise alone.** A need is what blocks the
  result as it stands; that much is a promise evaluation keeps. The facts
  behind an unknown condition that evaluation did reach are already known to
  the engine when the case ends, at no extra cost, so they are reported
  separately instead of dropped: a client may ask them in the same round, and
  the separate list keeps `needs` exact. `later` claims only that a fact was
  read behind the condition, never that it will be needed.
- **Every consumer, not the first reader.** A fact read by a rounding helper
  first and by the rule of § 13 later is waited on by § 13; which rule reads
  it first is an accident of evaluation order. Listing every cited consumer
  names each paragraph the answer feeds (the owner's tax liability feeds § 1,
  § 9 and § 16), and dropping the uncited ones when a cited one exists keeps
  helpers out of what a reader is shown.
- **A date is one fact; other records are not.** Only the standard `Date` is
  merged: its parts are meaningless alone (a day without its month), a client
  answers them together, and SDKs already encode it as one ISO date. A general
  marker (`@ field(atomic = True)`) would add a language surface without a
  second record that needs it; a record of separately answerable facts (an
  address) is better asked field by field. A marker can follow when a second
  record needs the merge.
- **Keys are declared, not guessed.** The first `String` field, or a field
  named `id`, would often be right and sometimes silently wrong (a name
  before an ISIN); `@ field(key = True)` says it once, where the field is
  declared, and the checker holds it to one scalar field per record type.
