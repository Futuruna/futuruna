# Conformance cases generated from the law

Status: Experimental (`runa cases`, Futuruna 0.8.0).

## Problem

A backend team implements a law in its own language (Endavu's TypeScript)
and keeps that implementation. The Futuruna model of the same law should be
the specification and the test oracle: a suite of input/expected-result
pairs that any implementation must pass, generated from the law, with a
statement of what the suite covers and what it does not. No WASM and no
Futuruna runtime runs in production; the cases are plain JSON.

Hand-written scenarios (`*.scenario.runa`) test what their authors thought
of. They do not say which rule clauses no test reaches, nor test the
thresholds of the law on both sides. The clause audit of `runa audit` knows
every clause and decides with Z3 whether each can fire, but it answers about
one rule family with its arguments free, not about the entry's input.

This is different from [external conformance cases](../conformance-cases.md),
which compare the model with an independently observed answer. Here the
model is the oracle and the implementation is under test.

## The command

```bash
runa cases src/aktiesparekonto.entry.runa --entry aktiesparekonto_year --out cases/generated \
    [--seed cases/aktiesparekonto-2026.json ...] [--scenario tests/aktiesparekonto_year.scenario.runa ...] \
    [--rejected AskYearRejected ...]
```

For each rule clause the entry reaches, `runa cases` looks for an entry input
that makes the clause fire, and for each integer or date threshold in a
clause's guard, inputs just below, at and just above it. It writes one file
per case, `<out>/<id>.case.json`, and `<out>/coverage.json`, and prints one
line:

```text
runa cases: 187 of 211 rule clauses covered; 41 cases (12 clause, 26 boundary, 3 scenario) in cases/generated
```

Case files of an earlier run that this run does not write again are removed
(only `*.case.json` in `--out`), so the directory always holds exactly the
current suite. The exit status is 0 when the cases were written, whatever the
coverage; a team that wants a coverage gate reads `coverage.json`.

### Case file

```json
{
  "id": "ask_year_ended_on.deemed_closed.clause",
  "entry": "aktiesparekonto_year",
  "kind": "clause",
  "covers": [
    { "rule": "ask_year_ended_on", "clause": "deemed_closed",
      "location": "aktiesparekonto.entry.runa:306", "cites": ["§ 3, stk. 4"] }
  ],
  "input": { "...": "the entry's complete input, as runa call takes it" },
  "expected": { "...": "the entry's complete result, as runa call gives it" }
}
```

- `kind`: `clause` (found for a clause), `boundary-below`, `boundary-at`,
  `boundary-above` (found for a threshold), `scenario` (exported from a
  scenario or seed), or `rejected`: any of these whose result is a rejection
  (below).
- `covers`: every rule clause the case's run fired, in the order of the
  catalogue (by file, then line), read from the run (below), never from what
  the case was searched for. A `boundary-below` case of a `>=` guard does not
  fire that clause, and says so. A rejected case covers only the clauses its
  rejection rests on.
- `input` is a complete, ready input of the entry's contract: `runa call`
  decodes it without a diagnostic, and it is in the input domain (below).
  `expected` is the `result` `runa call` gives for it, the whole result (an
  `AskYearRejected` with its issues is a result like any other).

### The input domain: only what an integrator can send

An integrator sends a case through the SDK `runa sdk ts` generates, whose
validators accept less than `runa call` decodes: an `Int` (of a unit or a
named scalar type too) is a JavaScript number, exact only within
±(2^53 − 1) (`Number.isSafeInteger`), and a `Date` is an ISO `YYYY-MM-DD`
string on the calendar, years 0000 to 9999. `runa call` takes any 64-bit
integer and any `Date` record, because a law may check its dates itself
(`date_valid(d)`); a case with `2026-04-31` or an amount of 10^16 exercises
that check, but no integrator can send it. Every other check of the SDK's
validators (a finite `Float`, one `Char`, a set without duplicates, no
unknown fields) the decoder makes as well.

`src/calculate/input_domain.rs` states the domain once on the Rust side:
the SDK generator's `Date` shape (`is_date`), the SMT functions the
witnesses use (`runa_int_in_domain`, `runa_valid_date`) and the check of a
case (`CalculationContract::input_domain_error`, `output_domain_error`); a
test holds it to the runtime's validators (`IntSchema`, `isoDateParts`).

- **At the source.** Where an argument leaf of the base's call *is* an input
  leaf unchanged (an integer, or an input date, which provenance traces as
  one fact), the witness query asserts that input's domain: an integer input
  a safe integer, and the year, month and day of an input date (each an
  argument leaf, or the input's value where no argument is that part) a day
  on the calendar. So `days_in_month(year, month)` for a leap February
  reached from an input date gets a year on the calendar, not −404. A leaf
  computed from inputs is not bounded: its inputs may be in the domain when
  it is not. When the query has no model in the domain but has one without
  it, the witness is outside the domain.
- **Every case.** A run whose input or result is outside the domain is no
  case: its result counts as a failure, so no search is satisfied by it and
  nothing it fires counts as covered (its rule calls stay, for the search to
  move on from it). This holds for scenario calls and seeds too.
- **Reported.** A clause that some run outside the domain would have covered,
  or whose only witness lies outside it, is uncovered with reason
  `outside-input-domain`. A threshold side whose model lies only outside the
  domain (an amount just above 2^53 − 1, the day after the last of a month
  for `d.day <= days_in_month(d.year, d.month)`), or that only a run outside
  it reached (an amount in øre whose input in millionths of a krone is beyond
  2^53 − 1), has no case, and the threshold's `detail` says `outside the
  input domain` with Z3's arguments or the value outside it.

### Rejections: coverage means the effect is observable

An entry that refuses a case returns a result that shows little of what the
rules computed before it gave up: an `AskYearRejected` with its issues says
nothing of the tax rules that ran first. A case whose result is a rejection
therefore covers a clause only when the rejection rests on it.

**Which results are rejections.** Futuruna has no entry-level declaration of
a rejection (`@ entry` takes only a title, and issues are a law's own type,
`Issue` in the tax service). The language's own notion of failure is the
prelude's `Result(a, e) = Ok(a) | Err(e)`: an entry whose result type is
`Result` rejects with `Err`, with no flag. An entry with its own outcome type
names its rejection variants with `--rejected VARIANT` (repeatable; `runa
cases` refuses a name that is not a variant of the result type).
`coverage.json` lists them under `rejections`.

**What a rejection rests on** is read from provenance. The explanation traces
values, not the branches that chose them: an issue built in
`if condition { [Issue(...)] }` carries no trace of `condition`. So a
rejected input runs a second time as a *decided case*: an explained run in
which the value an `if` or `match` branch gives has every leaf computed from
the condition (or the matched value) that chose the branch as well, and a
branch that gives a rejection variant records those conditions
(`Interpreter::begin_decided_case`, `explain_branch`). The clauses the
rejection rests on are the rule nodes reached backwards from the result's
leaves and those conditions, through computed values, what each clause
computed and the guards that chose it (`ExplainState::rejection_clauses`).
The first run's call log and argument provenance are kept for the lift: in
the decided run an input leaf passed through a branch is no longer the
input leaf unchanged.

Two more reads are traced only in a decided case, because an explanation
does not show them: `&&` and `||` are computed from the operands they read
(`date_valid(d) && date_in_bounds(d)`), and a clause that returns `None` is
traced like any other value (`match year_parameters(y) { | None ->
AskYearRejected(...) }`). Tracing changes no value, so a decided run whose
result differs from the first run's is not read, and the case then covers
nothing.

Consequences:

- Rejected cases are kept, `kind: "rejected"`, and counted apart
  (`coverage.json` `cases.rejected`, and the printed line).
- A clause search is satisfied by a rejected run only when the rejection
  rests on the target clause; a boundary only when it rests on a clause of
  the threshold's rule (the boundary of a validation rule is observable, that
  of a tax rule run before the entry gave up is not).
- A clause that fired only in rejected runs, off the rejection's path, is
  uncovered with reason `rejected`.

### Clause identity and stable ids

A clause is named by its rule and its key within the rule:

- `rule`: the family's qualified name (`ask_year_ended_on`,
  `ShareIncomeCase.tax_øre`).
- `clause`: an exception's label (`deemed_closed`); any other clause
  `clause-<n>`, its position among the family's clauses in source order
  (1-based). Moving a family's clauses around or reformatting the file does
  not change a key; inserting a clause before an unlabelled one does, which
  is a change of the law the suite should show.

Case ids are built from what the case is for, never from a hash of values:

| kind | id |
| --- | --- |
| clause | `<rule>.<clause>.clause` |
| boundary | `<rule>.<clause>.t<k>.boundary-below` / `-at` / `-above`, `k` the threshold's position in the guard |
| seed kept as is | `seed.<case_id>` (`seed.minimal` for the generated one), kind `clause` |
| scenario | `scenario.<file stem>.<n>`, `n` the call's position in the scenario run |

A `/` or whitespace in an id becomes `_`. Ids are unique; the file name is
the id.

### Coverage report

`coverage.json`:

```json
{
  "entry": "aktiesparekonto_year",
  "summary": "187 of 211 rule clauses covered",
  "rejections": ["AskYearRejected"],
  "cases": { "clause": 12, "boundary": 26, "scenario": 3, "rejected": 9 },
  "clauses": [
    { "rule": "...", "clause": "...", "location": "...", "cites": [],
      "covered_by": ["seed.u-1001-2026-ask-1", "..."] },
    { "rule": "...", "clause": "...", "location": "...", "cites": [],
      "covered_by": [], "reason": "not-analysable",
      "detail": "guard not translatable: parameter `transfer_in` has type `Option(AskTransferIn)`, ..." }
  ],
  "thresholds": [
    { "rule": "...", "clause": "...", "threshold": 1, "guard": "(>= deposits ceiling)",
      "below": "<case id>|null", "at": "...", "above": "...", "detail": "..." }
  ]
}
```

The clause set (`M`) is the clause audit's: every candidate of every rule
family the entry's runtime closure reaches, through imports
(`analyze_entry_clause_reachability`), in file and line order. Boolean
clauses whose body is the literal `false` only mark a matched head, select
nothing and are not counted, as in the audit. `N` counts the clauses some
written case covers. Each uncovered clause carries one reason:

- `unreachable`: the audit proved it can never fire (its guard is
  unsatisfiable, or earlier clauses always apply first). No case can exist;
  the finding is the audit's.
- `not-analysable`: its guard is outside the SMT encoding (the audit's
  reason is the `detail`), and the search without a witness found no input.
- `solver-unknown`: Z3 timed out or answered unknown on its witness.
- `no-entry-input`: Z3 found arguments that make the clause fire (the
  `witness`, by parameter), but no entry input reached the rule with them:
  the arguments are derived from the input through lists, `foldl` or
  `scan`, or the entry's validation rejects every input that would produce
  them first. The witness tells a human what a scenario has to set up.
- `rejected`: it fired, but only in inputs the entry rejects, and no
  rejection rested on it.
- `outside-input-domain`: only an input outside the input domain fires it
  (an `Int` beyond ±(2^53 − 1), a day not on the calendar): a run of such an
  input fired it, or Z3's witness exists only outside the domain (it is the
  `witness`).

## How a case is found

The clause audit encodes one rule family at a time: its arguments (and a
RuleScope's fields) are free SMT constants, its clauses' guards are
formulas over them. It does not encode the entry's own computation (lists,
`foldl`, `scan`, open collections, functions over records of lists), which
is where an Aktiesparekonto's rule arguments come from. So a case is found in
two levels, and the engine has the last word:

1. **Witness (Z3, per family).** For clause *i*: a model of
   `fires_i` = every earlier translatable guard false, guard *i* defined and
   true; the model's value of each argument is the witness. It is solved
   relative to a call of the family in a base's run (step 2): every leaf of
   the arguments that call passed is a soft constraint (`assert-soft`) to keep
   its value, weighing 50 when no input leaf contributes to it (nothing could
   move it) and 1 otherwise. So the witness moves as few values as the guard
   needs (the deposit, not the ceiling derived from the year), and the case
   reads as the base with that change. Arguments no run passed are pinned to
   plain values (`0`, `false`, `""`), as the audit's overlap witnesses are.
2. **Lift (engine, per input).** A *base* input is run with provenance on.
   Every call of the clause's family is logged with its argument values; the
   explanation graph says, for each scalar leaf of an argument, which input
   paths it was computed from and whether it *is* an input leaf unchanged.
   Each argument leaf whose witness value differs from the logged one is
   moved to it:
   - a leaf that is an input leaf: that input path takes the witness value;
   - a leaf computed from integer input leaves (an amount in øre from one in
     millionths of a krone, a sum of deposits): one contributing input path
     is searched, by secant steps and then bisection, until the run's leaf
     equals the witness value;
   - an `Option` argument that is `None` (no leaf to trace) or a constructor
     the witness changes: the input paths of the same type that hold `null`
     (or that variant) are tried in order.
   The changed input is run again; the steps repeat, at most four rounds,
   while the clause has not fired and the input changed.
3. **Confirm (engine).** The candidate is kept only if it decodes without a
   diagnostic, it and its result are in the input domain, the entry returns a
   result, and its fired clauses include the target. Its `covers` and
   `expected` come from that run.

Without a witness (a guard outside the encoding, or `solver-unknown`), step 2
runs without a target: the neighbourhood of each base is tried in a fixed
order (each `null` input path filled from another base or with the minimal
value of its type, each non-null option set to `null`, each sum-typed input
switched to each other variant, each Bool flipped) and a candidate is kept
when the target fires.

### New list elements

A rule over an event or a position takes its arguments from one element of
a list (`events[3].kind.amount_micro_dkk`, read from the arguments' input
paths). Moving that element often breaks what else rests on it (the only
deposit turned into a withdrawal). When the lift finds no case, the element
is added anew, innermost list first, for at most two lists per call:

1. A copy of the element is appended to the base, with its own key
   (`@ field(key = True)`): for a list the case gives whole, the case's id
   (`fee.large.clause`, a suffix `-2` if taken), so a rerun writes the same
   element; for an open collection (`open = True`) only a key the witness
   gives (an argument leaf read from the element's key), never an invented
   one, and none the collection already has. A key that is not text, or an
   open entry the witness gives no key for, gives no element.
2. The run's call that reads the new element is lifted to the witness as in
   step 2 above.
3. Every leaf of the new element the witness did not set (scalars, lists,
   dates) is put back to the plainest value of its type, one at a time, where
   the case still holds, so the element is as minimal as stays valid.
4. It is kept only when the run decodes, is not rejected, and fires the
   clause (or, for a boundary, has the threshold's values).

### Bases: complete, valid inputs

Every case is a base with a few leaves moved, so every field the clause does
not constrain keeps a value that already passed the entry's validation, and
the case reads as "this seed, with the deposit raised to 50 000 kr". Bases,
in this order:

1. **Scenario inputs** (`--scenario FILE`, repeatable): the scenario runs as
   `runa test` runs it, and every call of the entry function it makes is
   captured with its argument, encoded with the contract
   (`encode_partial_input`); arguments that hold `unknown` are skipped. Each
   distinct one is also written as a `scenario` case, so a clause no witness
   reaches but a human-written scenario does is covered by a human case.
2. **Seed envelopes** (`--seed FILE`, repeatable): the `ready` cases of a
   `runa call` input document, kept as `seed.<case_id>` when they cover a
   clause no earlier case covers.
3. **The minimal input**: every field at the plainest value of its type
   (`0`, `false`, `""`, `null`, `[]`, the first variant without fields, else
   the first; a `Date` 2000-01-01), kept as `seed.minimal` likewise.

The minimal input leaves every list empty, open collections included: it
never invents a key (a day, an ISIN) the law has not asked for. A law that
then looks an entry up fails the run ("no entry whose `day` is …"), and that
run is not a case; deriving keys from the law's own computed days would put
values into the input that no integrator supplied, so they would describe
the search, not the law.

A base that does not decode, or whose run fails, is reported on stderr and
not used. A clause that a base already fires is covered by that base; the
search runs only for clauses no case covers yet, so the suite stays small.

### Boundaries

A threshold is a top-level conjunct of a clause's lowered guard that compares
two integer terms (`<`, `<=`, `>`, `>=`; amounts of a unit type are
integers), or two `Date`s (which `smt_date_order` writes over their fields;
they are compared by day number, `runa_days`, and both are kept valid
dates, `runa_valid_date`). For threshold *k* of clause *i* and each offset
`d` in `-1, 0, +1`, Z3 is asked for a model of: dispatch reaches clause *i* (every earlier
translatable guard false, guard *i* defined), every other conjunct of guard
*i* holds, and `lhs - rhs = d`, where `lhs` is the side that reads the rule's
arguments and `rhs` the bound (when both sides read arguments, the guard's
left and right). For `deposits > ceiling` below, at and above are deposits
of `ceiling - 1`, `ceiling`, `ceiling + 1`, so only `above` fires the
clause; for `opened > Date(2024, 7, 1)` they are 30 June, 1 July and 2 July
2024. The model is anchored and lifted like a witness. Confirming a boundary
case reads the call log, not the fired clauses: some call of the family must
have the model's values in every argument leaf the comparison reads. A
boundary the search does not confirm is listed in `thresholds` with `null` and
the reason.

### Deduplication and determinism

Cases are compared by their input. A candidate equal to a written case is not
written again; the earlier case keeps its id and kind. Order of work: scenario
bases, seeds, then clause targets in catalogue order, then thresholds in
catalogue order; within one, bases in the order above. Nothing is random.
Z3 runs with the same scripts and the same soft constraints, so the same law,
seeds and scenarios give the same cases with the same Z3; across Z3 versions
a witness may differ where the law leaves values free, and then only the
values the guard does not decide differ, while ids and coverage stay. JSON is
written with sorted keys and a trailing newline.

## Limits

- **Clauses outside the SMT encoding** (a parameter of a type with a list, a
  call SMT cannot type) get no witness; only the neighbourhood search and the
  scenarios can cover them.
- **`foldl`, `scan`, lists, open collections.** No formula connects an
  argument to the entry's lists. The lift moves one input leaf at a time and
  needs that leaf's provenance; a new list element copies one that some
  base's run already passed to the rule. A value that needs a new entry of an
  open collection under a key no argument reads (a dividend of another ISIN)
  is not reached, and a new element that unbalances what the entry checks
  (a deposit the closing balance does not show) is rejected. These clauses
  are reported `no-entry-input` or `rejected` with their witness: the place
  for a scenario.
- **Validation.** A witness may be one the entry's validation rejects before
  the rule runs (an account closed before it opened). The search then fails
  honestly; it never claims unsatisfiability at entry level, which nothing
  here decides.
- **Floats** are outside the encoding, like in the audit.
- **Memoised calls**: a rule call answered from the interpreter's memo is not
  logged again; the first call with those arguments is.

## What is reused and what is new

Reused:

- `src/bin/runa_rule_reachability/mod.rs`: the audit's program setup
  (`resolve_smt_program`, `lower_smt_namespaces`,
  `RuleDispatchRegistry::for_solver`, `SmtRuleLowerer`, `Encoder`),
  `family_signature`, `lower_family`, `Encoder::preamble`, the per-candidate
  definitions (`fires_i`, `none_before_i`, guard definedness), `run_solver`,
  `smt_responses`, and its verdicts (`UnsatisfiableGuard`, `Shadowed`, not
  analysed) for the `unreachable`/`not-analysable` reasons.
- `src/bin/runa.rs`: `initialized_calculation_interpreter` and
  `reached_rule_families` (the families an entry reaches),
  `select_calculation_contract`, `type_check_artifacts`.
- `src/calculate.rs`: `PreparedEntry`, `CalculationWorker` (decode, scalar
  checks, run, `encode_output`), `CalculationContract::encode_partial_input`
  for scenario arguments, the contract's types for minimal values.
- `src/explain.rs`: `FiredClauses` (what a case covers) and the provenance
  graph (`Node::Input`, `Node::Computed`, `Node::Rule::from`) with
  `tag_input`'s input paths.
- `src/calculate/explain.rs`: `ExplainRenderer::clause_info` for a clause's
  location and cited paragraphs.

New:

- `src/bin/runa_rule_reachability/mod.rs`: `clause_case_targets` — the
  catalogue with, per clause, its witness model and its thresholds' boundary
  models (structured model values, not display strings), sharing `analyze`'s
  setup through `AuditProgram`.
- `src/explain.rs`/`src/lib.rs`: an opt-in rule-call log on `Interpreter`
  (`begin_rule_call_log`/`finish_rule_call_log`), written at the dispatch
  site that records a fired clause, with the selected clause and the
  argument values.
- `src/calculate.rs`: `PreparedEntry::probe` — one input, run with
  provenance, giving the result or the diagnostic, the fired clauses with
  their locations and citations, and the logged calls with each argument
  leaf's provenance; for a rejected result, the clauses the rejection rests
  on, from a decided run. `Interpreter` gains an opt-in capture of an entry
  function's arguments for scenario export.
- `src/explain.rs`/`src/lib.rs`: decided cases (`begin_decided_case`,
  `explain_branch` at `if` and `match`, `ExplainState::rejection_clauses`).
- `src/bin/runa_conformance_cases/mod.rs`: the command — bases, lift, search,
  boundaries, deduplication, case files and `coverage.json`.
- `src/calculate/input_domain.rs`: the input domain, shared with the SDK
  generator.
- `runa cases` in the CLI, `docs/reference/calculations.md`
  ("Generate conformance cases") and a release note.
