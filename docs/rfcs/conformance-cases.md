# Conformance cases generated from the law

Status: Experimental (`runa cases`, Futuruna 0.7.0).

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
    [--seed cases/aktiesparekonto-2026.json ...] [--scenario tests/aktiesparekonto_year.scenario.runa ...]
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
  `boundary-above` (found for a threshold), or `scenario` (exported from a
  scenario or seed).
- `covers`: every rule clause the case's run fired, in the order of the
  catalogue (by file, then line), read from the run (below), never from what
  the case was searched for. A `boundary-below` case of a `>=` guard does not
  fire that clause, and says so.
- `input` is a complete, ready input of the entry's contract: `runa call`
  decodes it without a diagnostic. `expected` is the `result` `runa call`
  gives for it, the whole result (an `AskYearRejected` with its issues is a
  result like any other).

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
   diagnostic, the entry returns a result, and its fired clauses include the
   target. Its `covers` and `expected` come from that run.

Without a witness (a guard outside the encoding, or `solver-unknown`), step 2
runs without a target: the neighbourhood of each base is tried in a fixed
order (each `null` input path filled from another base or with the minimal
value of its type, each non-null option set to `null`, each sum-typed input
switched to each other variant, each Bool flipped) and a candidate is kept
when the target fires.

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

A base that does not decode, or whose run fails, is reported on stderr and
not used. A clause that a base already fires is covered by that base; the
search runs only for clauses no case covers yet, so the suite stays small.

### Boundaries

A threshold is a top-level conjunct of a clause's lowered guard that compares
two integer terms (`<`, `<=`, `>`, `>=`; amounts of a unit type are
integers), or two `Date`s (which `smt_date_order` writes over their fields;
they are compared by day number, `runa_days`, and both are kept valid
dates). For threshold *k* of clause *i* and each offset `d` in `-1, 0, +1`,
Z3 is asked for a model of: dispatch reaches clause *i* (every earlier
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
  needs that leaf's provenance; a value that only a *new* list element (a
  second deposit, a dividend of another ISIN) could produce is not reached.
  These clauses are reported `no-entry-input` with their witness: the place
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
  leaf's provenance. `Interpreter` gains an opt-in capture of an entry
  function's arguments for scenario export.
- `src/bin/runa_conformance_cases/mod.rs`: the command — bases, lift, search,
  boundaries, deduplication, case files and `coverage.json`.
- `runa cases` in the CLI, `docs/reference/calculations.md`
  ("Generate conformance cases") and a release note.
