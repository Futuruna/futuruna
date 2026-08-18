# Bounded Rule Exploration with `? explore`

Status: accepted RFC; syntax and typed query analysis are Experimental

This document specifies a first-class Futuruna analysis for asking a bounded
question of an encoded rule model and receiving every meaningfully distinct
answer.

The companion [implementation workbook](bounded-rule-exploration-workbook.md)
teaches the contract through a small program and the Danish personal-income-tax
model. The compiler now parses and type-checks the declaration. Search
execution, concrete replay, reporting, and the dedicated CLI command remain the
next implementation slices.

## Summary

Ordinary execution answers:

> What do these rules produce for these facts?

Bounded exploration answers:

> For which permitted facts does this property hold or fail?

An exploration names one pure Boolean rule, declares the complete input world,
optionally identifies an adjacent transition axis, and states what makes two
answers the same. Futuruna searches the reachable rule graph, confirms every
reported result through normal execution, and distinguishes a complete answer
from a partial or undecidable search.

The core form is:

```runa
? explore QUERY_NAME {
    over BOOLEAN_RULE_CALL
    find violations | matches

    bounds {
        BOUND_OR_CONSTRAINT...
    }

    boundaries on INTEGER_INPUT by POSITIVE_STEP

    output {
        key [KEY_FIELD...]
        show [SHOWN_FIELD...]
        representative first | maximize EXPR | minimize EXPR
    }
}
```

`over` defines the question. `bounds` defines the world. `boundaries` defines
the movement being examined. `output.key` defines what one answer means.

## Goals

The feature MUST:

- discover answers without a user-authored list of suspected thresholds;
- search only an explicit, typed and finite declared universe;
- reuse canonical rule dispatch, imports, defaults and exceptions;
- enumerate distinct projected answers rather than raw solver assignments;
- replay every emitted answer through normal Futuruna semantics;
- report whether the answer is complete, partial, unknown or unsupported;
- keep counts meaningful by requiring an explicit result identity;
- retain enough rule and source provenance to explain a result;
- fail closed when exact reasoning is unavailable.

## Non-goals

The first version does not:

- infer a useful legal question from an entire corpus;
- invent bounds for integers, strings, lists or personal facts;
- treat every assignment as a distinct legal mechanism;
- run effects, persistence, streams, actors, time or randomness in a query;
- approximate unsupported semantics and still call the result complete;
- provide population prevalence from a model-space search;
- claim that an encoded model is legally authoritative merely because its
  bounded search is complete.

## Terms

**Question rule**
: The pure Boolean rule call named by `over`.

**Declared universe**
: Every assignment permitted by the bound domains and `where` constraints.

**Assignment**
: One complete choice of all searched inputs.

**Finding**
: One distinct `output.key` for which at least one admissible assignment has
  the requested polarity.

**Boundary pair**
: The lower value `x` and following value `x + step` on one integer axis.

**Representative**
: The replayed assignment shown for one result key when several assignments
  share that key.

**Complete**
: Every result key in the declared universe has been returned and closure has
  been established by final solver `UNSAT` or exact finite exhaustion.

## Source Syntax

### Named and anonymous queries

The durable form is named:

```runa
? explore income_cliffs {
    ...
}
```

One anonymous query is allowed in a root file:

```runa
? explore {
    ...
}
```

An anonymous query can run only when it is the sole exploration selected from
the root file. A file containing multiple explorations MUST name all of them.
Imported exploration declarations are not executed implicitly.

The parser recognizes `explore`, `over`, `find`, `bounds`, `where`,
`boundaries`, `output`, `key`, `show` and `representative` contextually inside
this form. They do not become global keywords.

Existing proof forms remain unchanged. In particular, bare:

```runa
? explore
```

continues to prove an invariant named `explore`. Only `? explore {` and
`? explore NAME {` begin an exploration declaration.

### Clause order

The first version requires this order:

1. exactly one `over` clause;
2. exactly one `find` clause;
3. exactly one `bounds` block;
4. zero or one `boundaries` clause;
5. exactly one `output` block.

Clause order is fixed so diagnostics, formatting and source review remain
predictable.

## The Question Rule

`over` accepts exactly one call to a named Boolean rule:

```runa
over one_more_never_hurts(household, income, step)
```

Each argument in version one MUST be a distinct bare identifier. Its type comes
from the corresponding rule parameter, and the identifier becomes query-local.
Literals, field access, nested calls, named arguments and repeated identifiers
are rejected in `over`; place fixed or derived expressions in `bounds` instead.
The call also establishes the root of the reachable dependency slice.
Overloaded rule identities MUST resolve unambiguously by scope, name and arity.

The question rule and every reachable operation used for a complete result
MUST be pure, total and exactly supported by the selected analysis backend.

Multiple questions are composed in an ordinary wrapper rule. `over [a, b]` is
not supported because a list does not define whether its elements are combined
with AND, OR or another relationship.

### Polarity

The `find` clause is mandatory:

```runa
find violations
```

returns assignments for which the Boolean rule is false.

```runa
find matches
```

returns assignments for which the Boolean rule is true.

Explicit polarity keeps these two questions distinct:

- Where does an expected property fail?
- Can a desired or unusual condition occur?

A complete zero-result violation search is a bounded proof that the property
holds. A complete zero-result match search is a bounded proof that the
condition does not occur.

## Bounds

Every relevant input introduced by `over` MUST be bounded, fixed or derived.
An unresolved input is a compile error. The explorer MUST NOT silently declare
an unbound input as an unconstrained integer.

The bounds block supports domain clauses, derived bindings and validity
constraints:

```runa
bounds {
    household in values(Household)
    municipality in municipalities_2026
    income in range(0, 1_000_001)
    step = 1
    case = build_case(household, municipality)
    where case_is_valid(case, income)
}
```

### Explicit values

```runa
x in [a, b, c]
```

searches exactly the distinct values produced by the list.

```runa
x in named_values
```

accepts a pure, ground, finite list or set with the required element type.
Named collections are the correct way to expose authoritative datasets such as
dated municipal parameter rows.

Membership has set semantics. Duplicate values do not create duplicate
assignments. Deterministic ordering preserves the first occurrence for lists
and uses canonical value order for sets.

### Integer ranges

```runa
income in range(0, 1_000_001)
```

uses Futuruna's existing end-exclusive `range(start, end)` meaning. The domain
contains values from `start` through `end - 1`.

The exploration IR represents a range symbolically when possible. A million
integer values MUST NOT require materializing a million-element list merely to
ask an SMT solver about the interval.

Both endpoints and the cardinality use checked integer arithmetic. A reversed
or overflowing range is an error. An empty well-formed domain yields a complete
search with zero findings when every other requirement is supported.

### Every value of a finite type

```runa
household in values(Household)
```

means every inhabitant of a type the compiler can prove finite and enumerate
exactly.

The first complete implementation of `values(Type)` supports:

- Boolean values;
- finite non-recursive sum types;
- tuples and products whose fields are all finite;
- finite payload variants whose payload types are all finite;
- optional and result types when every contained type is finite;
- imported types satisfying the same rules.

Enumeration follows declaration order for variants and field order for product
values, recursively. This order is part of deterministic representative
selection, not part of the answer set.

It rejects:

- `Int`, `Float`, `String` and other unbounded primitives;
- recursive types;
- lists, maps, sets, functions, streams and effects;
- any product or variant containing an unbounded field.

A diagnostic points to the first unbounded path and suggests an explicit list
or separately bounded input:

```text
cannot enumerate values(FilingStatus):
  Paper.copies has unbounded type Int
provide an explicit finite list or expose copies as a bounded query input
```

If an implementation slice supports only nullary alternatives, it MUST expose
the narrower spelling `variants(Type)`. It MUST NOT call the result
`values(Type)` while omitting payload-bearing inhabitants.

No reflective scan may treat every top-level binding of a type as its domain.
Types describe possible values; named datasets describe recorded instances.

### Fixed and derived values

```runa
year = 2026
case = standard_case(municipality, church_tax)
```

creates query-local values rather than independent search dimensions. Derived
values may depend only on earlier bounded, fixed or derived values. Cycles are
rejected.

A singleton list is equivalent but less direct:

```runa
year in [2026]
```

### Validity constraints

```runa
where input_is_valid(case, income)
```

restricts the declared universe. The expression MUST be a pure Boolean
expression over already declared names.

For a boundary query, every `where` constraint is checked for both endpoints
after substituting the boundary axis with `x` and `x + step`. A runtime error is
not an invalid case; it is an exploration error.

Constraints are part of the public claim. A complete result is complete only
for assignments satisfying them. The report prints the constraints and any
statically computed domain cardinalities.

### Closed universe rule

Dependency slicing may prove that a rule input or model field cannot affect the
question, key, shown values, representative metric or validity constraints.
Only then may it be omitted from the bounds.

Every remaining free input MUST have a finite domain. Futuruna never fills a
missing legal or personal fact from a default merely to make a query run.

## Boundary Queries

```runa
boundaries on income by step
```

declares one ordered integer transition axis.

For a lower value `x` and positive ground step `d`, the pair is:

```text
(x, x + d)
```

Both values MUST belong to the declared domain. Version one supports one
integer boundary axis and one positive fixed integer step.

The Boolean question rule remains the source of truth for what makes the pair
interesting. A typical rule receives the same `step` value and compares model
results at `income` and `income + step`:

```runa
| one_more_never_hurts(profile: Profile, income: Int, step: Int) ->
    net(profile, income + step) >= net(profile, income)
```

The boundary clause gives the explorer the result axis, endpoint-validity
contract and optimization opportunity. It does not replace the Boolean rule.

Without a `boundaries` clause, `? explore` is an ordinary bounded match or
violation search over complete assignments.

### Structural boundary extraction

The compiler MAY derive candidate change points from reachable:

- comparisons and guards;
- defaults, exceptions and rule-dispatch changes;
- integer division and remainder;
- rounding;
- `min`, `max`, clamps and caps;
- finite table selections;
- other operations with an exact documented boundary rule.

Derived candidates accelerate solving and attach explanations. They MUST NOT
define the answer set. Completeness still requires the full bounded predicate
to close through solver `UNSAT` or exact finite exhaustion. An incomplete
extractor cannot silently narrow the search.

## Output and Result Identity

Every exploration requires an output key:

```runa
output {
    key [income_before = income]
    ...
}
```

The output block contains exactly one `key`, zero or one `show` list, and
exactly one `representative` policy, in that order.

The key answers:

> What counts as one distinct finding?

For searched assignment `x`, let `K(x)` be its key tuple. The explorer returns
one result for every distinct key having at least one assignment of the
requested polarity.

With:

```runa
key [income]
```

many profiles failing at the same income transition produce one finding.

With:

```runa
key [income, municipality, church_tax, commute_km]
```

each profile-transition combination is a separate finding. Both queries are
valid, but their counts answer different questions.

To count affected profiles, run a separate projection whose key contains the
profile dimensions. Do not derive that count from the number of income keys.

The explorer blocks each previously returned key tuple, not the complete
solver assignment. Hidden assignment counts MUST NOT be labelled as boundaries
or findings.

Key fields MUST have finite, first-order, canonical equality and serialization.
Functions, effects, streams, open scopes and raw solver terms are rejected.

### Shown values

```runa
show [
    income_after = income + step,
    loss_øre = loss(profile, income, step),
    municipality
]
```

controls the human and structured result payload. Unrequested hidden solver
variables are not dumped. `show` is therefore also a privacy boundary.

### Representatives

Every output block MUST state exactly once how a representative is selected,
even when every shown value follows from the key:

```runa
representative first
representative maximize loss_øre
representative minimize final_tax_øre
```

`first` uses canonical domain order. `maximize` and `minimize` require an exact
ordered scalar in version one, initially `Int`. Selection occurs independently
inside each key's equivalence class. Equal objective values use canonical
domain order so results remain deterministic.

Named `show` expressions are evaluated in source order and may be referenced by
the representative expression. Duplicate names, forward references and cycles
are rejected.

The report states that a representative is one case for the key. It MUST NOT
imply that every hidden assignment has identical shown values.

## Formal Semantics

Let:

- `D` be the finite assignments allowed by domains and constraints;
- `P(x)` be the Boolean question rule;
- `Q(x)` be `not P(x)` for `find violations`, otherwise `P(x)`;
- `K(x)` be the output-key projection.

The answer set is:

```text
R = { K(x) | x in D and Q(x) }
```

For every returned key `k`, a representative is selected from:

```text
W(k) = { x | x in D, Q(x), and K(x) = k }
```

A complete exploration guarantees:

```text
k is returned
if and only if
there exists an admissible bounded assignment x with K(x) = k and Q(x)
```

## Solver Enumeration and Replay

A solver-backed implementation follows this semantic loop:

1. Solve the bounded query for one matching assignment.
2. Decode its output key.
3. Constrain the query to that key and select its required representative.
4. Replay the representative through ordinary Futuruna execution.
5. Record the replayed key, shown fields, metric and provenance.
6. Add a blocking clause for the complete key tuple.
7. Repeat until no unseen key remains.

Final `UNSAT` closes the projected answer set. It does not merely say that the
last attempted complete assignment was absent.

Every emitted representative MUST replay with identical question polarity,
key, shown values and objective. A solver/runtime disagreement is an internal
correctness error, not a partial success.

An exact finite backend may implement the same answer-set semantics without
SMT. Backend choice is not observable except through recorded completion
method and performance.

## Completion Status

The result uses one of five statuses:

| Status | Meaning |
|---|---|
| `complete` | Every projected answer in the declared universe was found |
| `partial` | A limit or interruption stopped closure; any emitted findings are confirmed |
| `unknown` | The solver could not decide the remaining query |
| `unsupported` | Exact lowering and exhaustive fallback were unavailable |
| `error` | The query was invalid or solving/replay disagreed with execution |

`complete` requires all of:

1. every relevant input is fixed, derived or proven finite;
2. every reachable operation has exact supported semantics;
3. all validity constraints are applied at every required endpoint;
4. every output key is enumerated;
5. every emitted representative replays successfully;
6. closure comes from final `UNSAT` or exact finite exhaustion;
7. no timeout, output cap, resource limit or solver `unknown` occurred.

A partial or unknown zero-result search MUST NOT say that no case exists. It
says only that no case has been confirmed so far.

## Fail-closed Requirements

A complete result is unavailable when a relevant path contains unsupported:

- recursion;
- effects, I/O, persistence, actors, streams, time or randomness;
- higher-order values or calls;
- partial non-Boolean rule dispatch;
- unbounded or cyclic domain construction;
- unsupported arithmetic or collections;
- model decoding;
- solver absence or solver `unknown`.

The tool reports the first source-linked boundary and the remaining supported
coverage. It MUST NOT introduce uninterpreted functions, arbitrary defaults or
approximate arithmetic and then claim completeness.

## CLI Contract

Exploration uses a dedicated analysis command:

```bash
runa explore model.runa
runa explore model.runa --query income_cliffs
runa explore model.runa --query income_cliffs --json
runa explore model.runa --query income_cliffs --json --output result.json
runa explore model.runa --query income_cliffs --timeout 60s
runa explore model.runa --query income_cliffs --max-results 100
```

- `runa check` validates exploration declarations.
- `runa fmt` formats them.
- `runa run`, `build` and ordinary `verify` do not launch them.
- Multiple named declarations require `--query`.
- A sole named or anonymous root declaration may run without `--query`.
- Imported declarations are not selected implicitly.
- A complete search exits successfully whether it finds zero or many results.
- A resource limit preventing closure produces a nonzero partial result.
- Invalid, unsupported, unknown and replay-error outcomes are distinguishable.

Finding a violation is the command's purpose, not a process failure.

## Structured Result Contract

`--json` emits a versioned `futuruna.explore.v1` document. It contains at
least:

```json
{
  "schema": "futuruna.explore.v1",
  "schema_version": 1,
  "query": "support_cliffs",
  "query_hash": "...",
  "program_hash": "...",
  "status": "complete",
  "polarity": "violations",
  "bounds": {
    "dimensions": [
      {
        "name": "household",
        "domain": {
          "kind": "values",
          "type": "Household",
          "cardinality": 2
        }
      },
      {
        "name": "income",
        "domain": {
          "kind": "range",
          "start": 90000,
          "end_exclusive": 110000,
          "cardinality": 20000
        }
      }
    ],
    "fixed": [{"name": "step", "value": 1}],
    "constraints": []
  },
  "boundary": {
    "axis": "income",
    "step": 1
  },
  "projection": {
    "key_fields": ["income_before"]
  },
  "results": [
    {
      "key": {"income_before": 99999},
      "shown": {
        "income_after": 100000,
        "household": "Couple",
        "available_before": 114999,
        "available_after": 100000,
        "loss": 14999
      },
      "representative": {
        "strategy": "maximize",
        "metric": "loss"
      },
      "provenance": {
        "scope": "representative",
        "reached_operation_ids": ["support", "available_resources"],
        "changed_branch_ids": ["support:income_threshold"]
      },
      "replay": "confirmed"
    }
  ],
  "summary": {
    "distinct_keys": 1
  },
  "completion": {
    "method": "smt-unsat",
    "stop_reason": null
  },
  "diagnostics": []
}
```

The schema reuses canonical typed JSON values from
`futuruna.calculate.v1`. Results sort lexicographically by key fields in source
order, using each type's canonical value order. Timing, raw SMT models,
absolute paths and hidden inputs are excluded from canonical output. Unknown
additive fields are ignored; an unknown major schema is rejected.

## Human Result Contract

Human output begins with the answer and its scope:

```text
Exploration: support_cliffs
Status: COMPLETE

Question: where does next_step_never_hurts fail?
Search: every Household value; income 90,000–109,999; step 1
Result identity: one answer per income

Different income steps found: 1
99,999 -> 100,000
Representative household: Couple
Loss after the next unit: 14,999
```

It then prints representatives, objective values, changed rule branches from
each representative replay, source references, fixed facts, constraints and
exclusions.

The primary count is always `distinct_keys`, described using the key field
names. The tool does not print a bare `findings: N` when that wording would
hide what is being counted.

## Provenance

Every result records:

- the query and program content identities;
- the exact bounds and constraints;
- the output key and representative policy;
- the operation and rule identities reached by the representative;
- control-flow or dispatch branches changed in the representative when known;
- attached source references available through typed metadata;
- concrete replay confirmation.

Structural boundary extraction MAY add a causal explanation. It does not turn
the execution record into legal advice or prove the source interpretation.

When multiple assignments share one key, this provenance describes only the
selected representative. It is not an exhaustive inventory of mechanisms among
the hidden assignments. Version one can count mechanisms exhaustively only
when the model exposes mechanism identity as a typed value and a separate query
includes it in `output.key`; automatic provenance-derived mechanism projection
is deferred.

## Compatibility and Feature Stage

The syntax, CLI and JSON contract begin as **Experimental**. The existing
solver-assisted `runa verify` surface is Preview, but exploration introduces a
new language and operational contract that needs real corpus experience before
promotion.

Implementation adds a separate `solver-backed-exploration` feature-stage
surface. The RFC, feature-stage documents and CLI help identify the status.

The implementation uses a distinct exploration AST and typed query IR rather
than overloading `Stmt::Prove`. Every canonical AST/FIR walker, formatter,
typechecker, import-hygiene pass, semantic-interface classifier, LSP path and
compiler-pass coverage matrix MUST classify the new node explicitly. The node
has no ordinary runtime or native-codegen behavior.

## Implementation Slices

1. Freeze grammar, answer-set semantics and diagnostic expectations.
2. Add AST, parser, formatter, spans and traversal coverage.
3. Add query-local scope, purity checks and typed domain elaboration.
4. Add an Explore IR independent of Z3.
5. Add exact finite semantic fixtures.
6. Lower polarity, domains, constraints and canonical rule dispatch to SMT.
7. Add output-key projection, blocking and final-`UNSAT` closure.
8. Add deterministic representative selection and exact objectives.
9. Add concrete replay and mismatch rejection.
10. Add `runa explore`, human output and `futuruna.explore.v1`.
11. Add boundary-axis validation and safe structural acceleration.
12. Extend exact lowering through the required Personskat rule slice.
13. Run the Personskat query without manually supplied tax thresholds.
14. Publish feature stages, reference, tutorial and agent guidance.
15. Run focused proof tests, mint, the relevant canary and differential lanes.

## End-to-end Acceptance

Two queries establish different guarantees.

### Narrow § 9 C conformance

The fixed-profile query:

- contains no `341500`, `342499`, thousand-step candidate construction or
  expected result count;
- calls the canonical Personskat result path;
- automatically rediscovers the known 50-step § 9 C sequence;
- replays every result through `beregn_personskat`;
- has each representative replay identify the § 9 C phase-out branch;
- reaches `complete` only after no unseen income key remains.

### Broad Personskat discovery

The broad declared-profile query:

- bounds every relevant standardized profile input explicitly;
- contains no expected result count;
- groups by income transition rather than repeated profile assignments;
- retains and classifies every additional replay-confirmed transition;
- reports only representative-level mechanism provenance;
- reaches `complete` only after no unseen result key remains.

Changing an encoded threshold changes the discovered transitions in both
queries without any edit to either exploration query.
