# Bounded Rule Exploration with `? explore`

Status: accepted RFC; syntax and typed query analysis are Experimental

This document specifies a first-class Futuruna analysis for asking a bounded
question of an encoded rule model and receiving every meaningfully distinct
answer.

The companion [implementation workbook](bounded-rule-exploration-workbook.md)
teaches the contract through a small program and the Danish personal-income-tax
model. The compiler parses and type-checks the compact search clauses, which
normalize into the mandatory transition IR. The current development slice
includes a capped exact-finite reference invocation and the first
macOS-supervised single-worker durable stream over that same transition
evaluator: checked source probes,
candidate-first evaluation, authenticated frontier deltas, bounded published
checkpoints, pause/resume, and explicit bounded atomic terminal sealing.
An explicit durable-only `--case-graph full` request now publishes a bounded,
total current-evidence search decision DAG. The internal executable mechanism engine now
has two narrow profiles: one checked top-level endpoint containing one `if`, and one
checked endpoint making one direct positional call to a checked helper that
executes one `if`. Both fresh-replay each confirmed matching case against its
canonical transition frame, journal replay-confirmed signature blocks, and
publish resumable count-only mechanism checkpoints with `scope_open`,
`incidence_open`, or `matching_closed` status. Checked numeric `show` roots can
also reuse their canonical replayed `Int` values to publish exact or lower-bound
distinct-mechanism counts in requested half-open bins. This V1 runtime owns and
revalidates the checked root AST and refuses every external Futuruna import
before a mechanism stream is opened; import support requires a future frozen
module graph that preserves module boundaries and origins. General multi-event
and rule-attempt instrumentation, mechanism-DAG publication and public
mechanism-aware terminal schemas remain later slices. An explicitly positional
experimental CLI selector now exposes the nested-helper stream as count-only
checkpoints after normalization to the same observation IR; the ordinary exact
profile reports mechanisms as deferred when none is requested and admitted.
Symbolic/SMT closure,
typed output rows, result continuations, detached observation, parallel
workers, and chunked terminal publication also remain later implementation
slices. No result group is treated as a mechanism.

## Summary

Ordinary execution answers:

> What do these rules produce for these facts?

Bounded exploration answers:

> Which permitted before-to-after transitions make this property hold or fail,
> and which mechanisms explain those transitions?

An exploration names one pure Boolean rule, declares the complete input world,
defines or derives a typed before-to-after transition, and states what makes
two answers the same. The question may compare the after state with the before
state or state an absolute property of the after state while both remain
available for evaluation and replay. Futuruna searches the reachable rule
graph, confirms every reported result through normal execution, and
distinguishes a complete answer from a partial or undecidable search.

The transition generator and a separately requested mechanism observation have
different jobs. The generator declares the intervention or comparison that
supplies `before` and `after`; it does not claim that an encoded rule caused an
income, municipality or policy version to change. When requested and admitted,
mechanism replay describes both endpoint computations and how their encoded
rule-graph paths differ, if they differ. The transition and any observed
signature are joined without losing their exact supporting `CaseId` evidence;
the requested mechanism scope may nevertheless remain open.

The compact source form is:

```runa
? explore QUERY_NAME {
    over BOOLEAN_RULE_CALL
    find violations | matches

    bounds {
        BOUND_OR_CONSTRAINT...
    }

    boundaries on INTEGER_INPUT by POSITIVE_STEP

    probes {
        schedule [PROBE_SELECTOR...]
        lift matches on boundary_axis across [OUTER_INPUT_NAME...]
        at_most POSITIVE_CASE_COUNT cases
        retain configuration [INPUT_NAME...]
        retain output [OUTPUT_FIELD...]
        trace mechanisms | no_mechanism_trace
    }

    output {
        key [KEY_FIELD...]
        extrema [EXTREMA_NAME = INT_EXPR...]
        having varies(EXTREMA_NAME)
        show [SHOWN_FIELD...]
        representative first | maximize EXPR | minimize EXPR
    }
}
```

`over` defines the question. `bounds` defines the world. `boundaries` is compact
syntax for the initial Relative movement being examined; without it the compact
form normalizes to Identity. An optional `probes` block defines a finite,
deterministic initial scheduling plan inside the same exploration run without
changing that world or the answer.
`output.key` defines the raw answer groups; optional `extrema` summarize them
and `having` chooses which closed groups are emitted without changing the
matching case population.

The `output { ... }` form is a CLI-only projection. A query may
instead name one declared output product and receive the terminal result in an
analysis-only continuation:

```runa
? explore QUERY_NAME {
    over BOOLEAN_RULE_CALL
    find violations | matches

    bounds {
        BOUND_OR_CONSTRAINT...
    }

    boundaries on INTEGER_INPUT by POSITIVE_STEP

    output as ROW_TYPE {
        key [KEY_FIELD...]
        extrema [EXTREMA_NAME = INT_EXPR...]
        having varies(EXTREMA_NAME)
        show [SHOWN_FIELD...]
        representative first | maximize EXPR | minimize EXPR
    }

    then REPORT_NAME -> CONTINUATION
}
```

`output as ROW_TYPE` may be used without `then`. `then` requires a typed
output row. The continuation receives one status-safe
`ExplorationReport(ROW_TYPE)` after enumeration, replay and canonical sorting;
it cannot affect the search. This typed form is specified Experimental syntax,
not syntax accepted by the current compiler.

## Goals

The feature MUST:

- discover answers without a user-authored list of suspected thresholds;
- search only an explicit, typed and finite declared assignment space;
- normalize every explored assignment to a typed before state, after state and
  conservative transition context;
- distinguish assignment cases, semantic transitions, output keys and dynamic
  mechanisms instead of using one ambiguous case count;
- reuse canonical rule dispatch, imports, defaults and exceptions;
- enumerate distinct projected answers rather than raw solver assignments;
- preserve the exact relation between admissible cases, matches, projected
  findings and observed mechanisms;
- replay every emitted answer through normal Futuruna semantics;
- report whether the answer is complete, partial, unknown or unsupported;
- expose progress as a durable observable evidence stream that can be paused,
  inspected and resumed without weakening completion claims;
- keep execution status, case coverage and mechanism-evidence closure visibly
  separate;
- keep counts meaningful by requiring an explicit result identity;
- retain enough stable rule and source provenance to build replay-derived
  mechanism signatures without letting explanations define the answer set;
- fail closed when exact reasoning is unavailable.

## Non-goals

The first version does not:

- infer a useful legal question from an entire corpus;
- invent bounds for integers, strings, lists or personal facts;
- treat every assignment as a distinct legal mechanism;
- let the Futuruna question itself run effects, persistence, user streams,
  actors, time or randomness while forming or solving a query; the host may
  durably journal and publish exact search evidence, and an explicit downstream
  `then` continuation may use ordinary effects;
- approximate unsupported semantics and still call the result complete;
- provide population prevalence from a model-space search;
- claim that an encoded model is legally authoritative merely because its
  bounded search is complete.

## Terms

**Question rule**
: The pure Boolean rule call named by `over`.

**Declared assignment space**
: The Cartesian product `U` of the independently varied bound domains, before
  boundary eligibility and `where` constraints are applied. In normalized
  transition semantics these axes are role-tagged and extended by any
  independent after axes to form the declared generator-coordinate space
  `U_D`.

**Constructible transition space**
: The subset `U_C` of `U_D` whose structural endpoint contracts can construct a
  total typed `(context, before, after)` transition. A coordinate in
  `U_D \ U_C` retains its `CaseId` and closes as structurally excluded without
  a `TransitionId`.

**Admissible universe**
: The subset `D_C` of the constructible transition space `U_C` whose endpoint
  and cross-edge validity constraints hold. The compact notation `D` may be used
  when the transition-case scope is unambiguous.

**Assignment**
: One complete choice of all searched inputs.

**Generator axis descriptor**
: The structural identity `(role, role_field_index, bound_index)` of one varied
  coordinate in a report or snapshot. The serialized source name is a
  presentation label, not an identity or graph reference.

**Transition case**
: One canonical generator coordinate together with its structural,
  endpoint-validity and question-polarity classification. A coordinate in
  `U_C` additionally has one normalized `(context, before, after)` transition;
  a coordinate in `U_D \ U_C` is structurally excluded before such a transition
  exists. `CaseId` is injective over `U_D` and remains the identity used by exact
  search evidence and resume frontiers.

**Before state**
: The canonical typed value observed at the source endpoint of one transition.

**After state**
: The canonical typed value observed at the target endpoint of one transition.
  Its expression may be relative to the before state, may be a parallel
  evaluation over shared context, or may be independent of the before value.
  Independence is claimed only when checked dependency analysis proves it.

**Transition context**
: The canonical bounded facts needed to distinguish the meaning of a
  before-to-after edge but not represented in its endpoint values. Version one
  retains every explicitly declared Context field without relevance
  projection; a future projection may remove a field only with an exact
  irrelevance proof.

**State identity**
: `StateId`, the hash of the canonical State schema identity and canonical
  state value. A declared schema identity includes its resolved,
  occurrence-sensitive type owner and exact checked product layout. Endpoint
  role is not part of the identity, so one state may be the target of one edge
  and the source of another.

**Transition identity**
: `TransitionId`, the directional extensional identity of one typed
  `Context + Before -> After` value. It hashes the canonical Context then State
  schema identities, canonical context, before `StateId` and after `StateId`.
  It excludes transition mode, after-construction recipes and DAG topology,
  query identity, generator coordinates and mechanism execution paths.
  Reversing the endpoints changes the identity; computation differences remain
  separate mechanism evidence.

**Finding**
: One distinct emitted `output.key` for which at least one admissible
  transition case has the requested polarity and whose closed group satisfies the
  optional `having` filter.

**Case region**
: An exact subset of the declared generator-coordinate space `U_D` represented
  by a normalized path union or ordered decision subgraph. A shared node is
  interpreted with its incoming path context rather than as a context-free
  Cartesian region.

**Search decision DAG**
: The reduced ordered multi-terminal decision diagram classifying transition
  cases as excluded, matching, nonmatching or open. Implemented snapshot v6
  and exact-answer v5 artifacts expose this object under `graph.case_graph`
  when its immutable report request authorizes publication.

**Semantic transition graph**
: The directed graph whose nodes are canonical states and whose edges are
  canonical transitions, with exact support back to assignment cases. This is
  the domain-level case graph; it is not the search decision DAG. The in-process
  exact accumulator now constructs collision-checked `StateId`, directional
  `TransitionId` and `CaseId -> TransitionId` support for accepted constructible
  singleton transactions. The durable journal and public
  `semantic_transition_graph` serialization remain pending final-contract
  slices.

**Mechanism signature**
: A canonical replay-derived differential execution signature for one fixed
  query and mechanism-observation specification. Equal result values or equal
  top-level rules do not by themselves establish equal mechanisms.

**Mechanism fiber**
: The preimage of one complete mechanism signature within a named traced
  population. Signature fibers partition that population; the supports of
  individual rules or branch atoms may overlap.

**Boundary pair**
: The lower value `x` and following value `x + step` on one integer axis.

**Representative**
: The replayed assignment shown for one result key when several assignments
  share that key.

**Output row**
: One declared concrete product whose fields are exactly the output key fields
  followed by the shown fields.

**Analysis continuation**
: The optional query-scoped expression receiving the terminal typed report
  after search, replay and sorting have finished.

**Probe plan**
: An optional finite, deterministic scheduling plan declared by a query. It
  chooses which concrete cases to classify first inside the exploration run;
  it does not restrict `U`, `D`, `M` or `R` and is not a closure method.

**Exploration run**
: One durable attempt to close a fixed program, query, domain, report request
  and evaluator contract. Operational slices may pause and resume it without
  changing those identities.

**Evidence event**
: One immutable, run-bound commitment of newly validated case, region,
  representative, mechanism or frontier evidence. Discovery hints and worker
  proposals are not evidence events until the coordinator validates them.

**Journal head**
: The latest durable hash-chain head for an exploration run. It commits to
  operational event order and is the cursor used for resume and follower gap
  detection. Equivalent runs may have different journal heads when workers
  finish in a different order.

**Evidence root**
: The canonical, arrival-order-independent hash of normalized accepted
  semantic evidence and the exact open frontier. Equivalent evidence produces
  the same root regardless of scheduling, pause history or worker completion
  order.

**Exploration snapshot**
: A versioned projection derived from one journal head and its canonical
  evidence root: current closure states, exact counts or lower bounds,
  confirmed rows, open frontier and operational metadata. The full protocol
  may also include explicitly authorized case/mechanism graph views when the
  implementation supports them. Current executable `snapshot.v6` supports an
  explicitly requested total current-evidence search decision DAG and reports mechanism
  evidence as `unavailable_deferred`. Snapshot materialization is an optional,
  separately admitted observer phase; it is neither the resume checkpoint nor
  a prerequisite for a valid pause. A snapshot is never the canonical terminal
  result; sealing produces a distinct terminal document.

**Journal-only checkpoint**
: The authoritative append-only journal pause returned when snapshot
  materialization is not admitted before the invocation deadline. The current
  typed artifact is `JournalOnlyCheckpoint` for exact snapshots or
  `MechanismJournalOnlyCheckpoint` for mechanism count views; invocation-v1
  JSON names either artifact kind `journal_checkpoint`, reports the selected
  observer view (`snapshot` or `mechanism_checkpoint`) as `deferred`, and has no
  view blob, canonical payload, checkpoint cursor or publication cursor.
  This is operational view deferral, not evidence, graph-capacity status or a
  change to run identity.

**Snapshot-unavailable receipt**
: A separate, cursor-bound observer publication emitted when the snapshot phase
  was admitted but that publication attempt reported capacity. The journal
  records `SnapshotUnavailablePublished`, invocation-v1 uses artifact kind
  `snapshot_unavailable`, and its canonical JSON line is capped at 4 KiB. It
  carries hashes and bounded progress only: no configuration, answer rows,
  search-DAG prefix or arbitrary diagnostic text. It services that cursor's
  observer boundary without claiming that a later attempt can never fit and
  without changing semantic evidence. This is distinct from both transient
  `journal_checkpoint` deferral and search-DAG `capacity_limited` status inside
  an otherwise complete snapshot.

**Probe-complete**
: Every selector and adaptive decision required by the declared finite probe
  plan has reached its declared stopping condition. This is an observable
  milestone in the same run, not a terminal answer; it says nothing by itself
  about answer, case, value, complement or mechanism closure.

**Paused**
: No work is being dispatched, every accepted event is durable, and the exact
  open frontier is committed at the journal head and evidence root. A paused
  run is resumable and is not terminal semantic evidence.

**Terminal seal**
: An immutable lifecycle record committing one terminal outcome, the last
  journal head, canonical evidence root and the hash of the terminal payload
  derived from that evidence. `Completed` is the seal kind reserved for
  complete closure; terminal partial, unknown, unsupported, error and
  cancellation outcomes use distinct non-complete seal kinds.

**Complete**
: Every answer-defining population and every explicitly requested case/value
  view has closed by final solver `UNSAT`, exact finite exhaustion, exact
  region coverage or another recorded exact method. Mechanism evidence has a
  separate status and never makes an otherwise complete answer unsupported.

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

The transition-aware grammar recognizes `explore`, `over`, `find`, `bounds`,
`where`, `transition`, endpoint-local `before` and `after`, `boundaries`,
`output`, `as`, `key`, `extrema`, `having`, `varies`, `show` and
`representative` contextually inside this form, and reuses `then` for terminal
continuation delivery. These contextual words do not become global keywords.

Existing proof forms remain unchanged. In particular, bare:

```runa
? explore
```

continues to prove an invariant named `explore`. Only `? explore {` and
`? explore NAME {` begin an exploration declaration.

### Clause order

The transition-aware complete form requires this order:

1. exactly one `over` clause;
2. exactly one `find` clause;
3. exactly one `bounds` block;
4. one normalized transition, written explicitly or synthesized from compact
   source syntax;
5. zero or one `boundaries` clause supplying a Relative axis and accelerator;
6. zero or one `probes` block;
7. exactly one `output` block, optionally naming a row type with `as`;
8. zero or one `then` continuation.

Clause order is fixed so diagnostics, formatting and source review remain
predictable. Compact syntax may omit a source `transition` clause: no boundary
normalizes to Identity and a boundary normalizes to Relative. Both spellings
produce the same non-optional transition IR and use the same evaluator.

`then` is legal only after `output as ROW_TYPE`. The `output { ... }` spelling
has CLI-only behavior; both output spellings project the same canonical result
contract.

### Probe scheduling plans

A query may declare a finite initial scheduling plan between `boundaries` and
`output`:

```runa
probes {
    schedule [boundary_candidates, boundary_endpoints, frontier_midpoints]
    lift matches on boundary_axis across [municipality, church_tax, distance_km]
    at_most 256 cases
    retain configuration [municipality, church_tax, distance_km, income]
    retain output [income_before, income_after, loss_ore]
    no_mechanism_trace
}
```

This syntax is specified Experimental syntax and is not accepted by the
current compiler. The block lowers to a `ProbePlan`; it does not name a file.
The selected `runa explore` invocation MUST provide the private run-state path.
Keeping storage out of source lets the same checked question run in a local
scratch directory, a private case workspace or an automation cache without
changing its proposition or semantic query identity.

The first selector set is deliberately small:

- `boundary_candidates` visits guarded singleton supports proposed by the
  checked source-event extractor and `BoundaryPlan`;
- `boundary_endpoints` visits the canonical lower and upper eligible endpoint
  cases of each normalized open support; and
- `frontier_midpoints` adaptively bisects normalized open supports, choosing
  the largest support first and breaking all ties by canonical axis and
  `CaseId` order.

Selectors run in source order, never classify one `CaseId` twice, and operate
only inside the declared finite universe. `at_most` is mandatory and positive.
It is a semantic stopping condition of the probe plan, not an Explore search
budget: reaching it may make the probe plan complete while leaving almost all
of the exploration space open. The plan records every adaptive choice and the
classification that caused the next choice, so the same program, domain and
plan reproduce the same transcript prefix.

`lift matches on boundary_axis across [a, b, ...]` is an optional compositional
scheduling operation. When an evaluated case matches, it keeps that case's
lower boundary-axis value, ranges the listed independently varied outer inputs
over their declared domains, recomputes derived facts, and enqueues the
resulting in-domain `CaseId`s in canonical order. Endpoint and constraint
eligibility remain unevaluated. This expresses the useful
hypothesis "a boundary observed for one profile may also matter for other
profiles" without transferring any conclusion. The originating observed
`CaseId` and every generated candidate are distinct transcript records; a
generated candidate is explicitly `unevaluated` until it is selected and
classified normally. Neither match, nonmatch, exclusion, output nor mechanism
evidence is inherited from the origin. The operation is rejected if a listed
name is not an independently varied non-boundary input.

`retain configuration` is the exhaustive allow-list of query-local names whose
values may be written beside a `CaseId`. `retain output` similarly authorizes
named key or shown fields. The run journal always records the minimum evidence
needed to validate a classification: the canonical `CaseId`, match/nonmatch/
exclusion classification, boundary-axis lower and upper endpoint values, and
the exact Boolean outcome or exclusion reason. It MUST NOT serialize other
configuration fields, hidden model inputs or values merely because the
evaluator had them in memory. `trace mechanisms` is a separate explicit
authorization for the optional replay-derived trace; otherwise mechanism
trace content is absent.

Changing only a probe plan or its retention authorization changes
`probe_plan_hash`, but not `query_hash`, `U`, `D`, `M`, `R` or the meaning of a
later terminal result. The run-state path, time limit and checkpoint
frequency belong to the invocation and are excluded from every semantic hash.

## The Question Rule

In compact syntax, `over` accepts exactly one call to a named Boolean rule:

```runa
over one_more_never_hurts(household, income, step)
```

Each argument in version one MUST be a distinct bare identifier. Its type comes
from the corresponding rule parameter, and the identifier becomes a
query-local source alias for a checked State or Context field.
Literals, field access, nested calls, named arguments and repeated identifiers
are rejected in `over`; place fixed or derived expressions in `bounds` instead.
The call also establishes the root of the reachable dependency slice.
Overloaded rule identities MUST resolve unambiguously by scope, name and arity.

The explicit form calls one named Boolean rule with the three typed contextual
products:

```runa
over one_more_never_hurts(before, after, context)
```

`before` and `after` have the same declared product type; `context` has its
declared product type and may be `()`. This three-argument shape is the
initial explicit-transition contract. A wrapper rule may ignore `before` for
an absolute after-state question, but both endpoints remain defined. Output
expressions use the same contextual products. The compact bare-argument
restriction does not apply inside those products and MUST NOT be generalized
into implicit endpoint guessing. Alias resolution happens during normalization;
it does not create a second flat evaluator.

The question rule and every operation reachable from required answer, case and
value roots MUST be pure, total and exactly supported by the selected analysis
backend. Mechanism-only roots use their independent evidence status.

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

restricts the admissible universe. The expression MUST be a pure Boolean
expression over already declared names.

For a boundary query, every `where` constraint is checked for both endpoints
after substituting the boundary axis with `x` and `x + step`. A runtime error is
not an invalid case; it is an exploration error.

The intended explicit-transition surface makes validity scope visible:

```runa
where before before_is_valid(before, context)
where after after_is_valid(after, context)
where transition permitted_change(before, after, context)
```

These lower to `Before`, `After` and cross-edge `Transition` constraints. A
compact boundary `where` lowers to one checked template applied at both
endpoints (`BothEndpoints` in typed IR); a compact no-boundary `where` lowers to
`Before`, which is equivalent at the Identity after state. An independent
self-edge is admissible unless an explicit transition constraint excludes it.
The compiler never guesses such an exclusion from unequal source ordinals or a
presentation goal.

Constraints are part of the public claim. A complete result is complete only
for assignments satisfying them. The report prints the constraints and any
statically computed domain cardinalities.

### Closed universe rule

Dependency slicing may prove that a rule input or model field cannot affect the
question, key, extrema, group filter, shown values, representative metric or
validity constraints. Only then may it be omitted from the bounds.

Every remaining free input MUST have a finite domain. Futuruna never fills a
missing legal or personal fact from a default merely to make a query run.

## Transition Semantics

The semantic unit of Explore is a directed transition, not an isolated row.
Search and semantic identity are nevertheless different layers. Let `U_D` be
the finite declared generator-coordinate space, including context, before and
independent-after axis ordinals. Structural endpoint contracts first select
`U_C ⊆ U_D`: the coordinates capable of constructing a total typed endpoint
inside every required endpoint domain. For example, a boundary successor that
is outside its declared domain—or would overflow `Int` before construction—is
closed as structurally excluded in `U_D \ U_C` without evaluating derived
facts. Let `c(u)` be edge-local context, `b(u)` the complete before state, and
`a(u)` the after state generated at an eligible coordinate `u`. Normalization
is total on `U_C`:

```text
tau : U_C -> Transition
tau(u) = (c(u), b(u), a(u))
U_T = image(tau)
A(c, b) = { a(u) | u in U_C, c(u) = c, b(u) = b }
```

Every coordinate in `U_D` has a `CaseId`, including a structurally excluded
coordinate, but only `U_C` has semantic transition support and a
`TransitionId`. Two eligible coordinates remain distinct `CaseId` values even
if normalization gives them the same semantic edge. The normalized transition
generator SHOULD be one-to-one by default; an intentional many-to-one mapping
retains exact support and deduplicates only at the `TransitionId` layer.

Endpoint and cross-edge validity classify generator coordinates, while their
images define the distinct semantic-transition populations:

```text
D_C = { u in U_C |
        V_before(c(u), b(u)) and
        V_after(c(u), a(u)) and
        V_edge(c(u), b(u), a(u)) }

M_C = { u in D_C | Q(tau(u)) }
D_T = image(tau restricted to D_C)
M_T = image(tau restricted to M_C)
```

`Q(c, b, a)` may compare `a` with `b`, as an income-cliff question does, or
may state an absolute property of `a`. The before state remains available in
either form. An absolute predicate does not erase the transition that supplied
the after state.

Three normalized constructors cover the initial architecture:

- `Identity` has `A(c, b) = {b}`. It is the normalized form of a point search
  and may also be declared explicitly. It supplies no nontrivial differential
  mechanism.
- `Relative` has `A(c, b) = {F(c, b)}`. Fields not assigned by `F` are copied
  from `b` under checked frame semantics. A boundary query lowers to this form:
  the selected axis advances by `step`, invariant endpoint facts are framed,
  and derived endpoint facts are recomputed rather than copied accidentally.
- `Independent` has at least one after field drawn from a finite domain whose
  choices do not depend on the before value. Other after fields may still frame
  or derive from `b`, so `A(c, b)` remains the general notation. A singleton
  independent field is the simplest case; a finite alternative-policy or
  municipality domain produces one edge per alternative.

“Universal” is reserved for quantification over an independent after domain,
for example requiring a property for every alternative. It is a grouped view
over concrete edges, not an extra transition constructor and not permission to
omit the before endpoint.

Endpoint model facts belong in the state value. Action, step, policy-change and
other edge-local parameters belong in transition context. Context participates
in question evaluation, output and `TransitionId`, but not in `StateId`.
Version one retains every declared Context field. A future projected context
is sound only when dependency analysis proves that removed facts cannot change
endpoint validity, polarity, outputs or observed mechanisms.

For one fixed pair of canonical State and Context schemas:

```text
StateSchemaId = H(resolved_state_owner, canonical_state_layout)
ContextSchemaId = H(resolved_context_owner, canonical_context_layout)
TransitionTypeId = H(ContextSchemaId, StateSchemaId)
StateId(b) = H(StateSchemaId, canonical_value(b))
TransitionId(c, b, a) =
    H(TransitionTypeId, canonical_context(c), StateId(b), StateId(a))
```

The one State schema types both endpoints. For a declared State or Context
product, `resolved_*_owner` is the checked declaration identity, not its source
spelling. Every nominal node nested in `canonical_*_layout` is resolved and
encoded by the same declaration/intrinsic identity rule; equal-looking field
types from different owners therefore cannot alias. The transient global
position of a retained declaration in one checked program is not part of this
extensional identity. Synthetic compact products instead use their
normalization version and recursively resolved checked layout, and Unit has its
own tag.

`StateId` is role-neutral and `TransitionId` is directional. One canonical
state may therefore be shared as an after node of one transition and a before
node of another. Equal typed Context, Before and After values produce the same
`TransitionId` even when different modes, normalized recipes, generator
coordinates or mechanism paths produced them. Those intensional distinctions
remain in query identity, `CaseId` support and mechanism evidence rather than
splitting one semantic edge. `CaseId` remains the query-local canonical ordinal
coordinate in `U_D`, used for scheduling, frontiers and exact classification.
Only a `CaseId` in `U_C` has a `TransitionId` and semantic-edge support.
The normalized generator MUST either establish a one-to-one
`CaseId -> TransitionId` mapping or perform explicit semantic-edge
deduplication before reporting a distinct-transition count. Two generator
paths MUST NOT silently double-count one edge. The support relation is retained
when several case coordinates denote the same edge.

The implemented exact accumulator now derives the three typed 32-byte schema
IDs in the composition above, retains their canonical preimages for collision
checks, instantiates `StateId` and `TransitionId` from those IDs, and interns
exact support for every constructible singleton transaction it accepts,
including validity exclusions and nonmatches. That crate-private index is not
yet journaled or emitted by snapshot v6 or exact-answer v5, and certified search
regions are not silently promoted into semantic-edge support. Public graph
serialization and any complete distinct-transition count therefore remain
pending.

### Transition syntax and canonical typed IR

The explicit source surface is a contextual `transition { ... }` clause.
`before` and `after` are reserved for endpoint roles; the terminal analysis
continuation is consequently spelled `then`. The source role-tags bound fields
explicitly:

```runa
# IncomeState(
    household: Household,
    income: Int
)

# IncomeChange(step: Int)

| one_more_never_hurts(
    before: IncomeState,
    after: IncomeState,
    context: IncomeChange
) -> available(after) >= available(before)

over one_more_never_hurts(before, after, context)
find violations

bounds {
    before.household in households
    before.income in range(0, 1_500_001)
    context.step = 1
}

transition as IncomeState context IncomeChange {
    relative
    after.income = before.income + context.step
}
```

`state_schema` and `context_schema` are named, already declared product types;
context may instead be `()`. This makes the ordinary named rule signature
authorable before the query. `before.FIELD` and `context.FIELD` declarations
must cover each corresponding product field exactly once with the declared
type. Type declaration field order, not incidental bound order, is canonical
within each role. The schema identity closes the resolved checked declaration
owner as well as that layout, so two same-spelled product declarations at
different occurrences do not alias. `after` has exactly the state type. Every
after field is then assigned one checked source: frame from the same-named
before field, derive from before, context and explicitly projected
`after.OTHER` fields, or range over an independent finite domain. Types must
agree exactly. There is no implicit decision about whether an untagged
explicit bound belongs to state or context.

These shorter forms illustrate the three normalized modes:

```runa
transition as StateType context () {
    identity
}

transition as StateType context ChangeType {
    relative
    after.income = before.income + context.step
    after.tax = tax_for(before.household, after.income)
    after.available = after.income - after.tax
}

transition as StateType context ComparisonContext {
    independent
    after.municipality in municipalities
}
```

Explicit endpoint references are `before.FIELD`, `after.FIELD` and
`context.FIELD`. Relative fields not assigned in the clause are frame-copied.
Inside a derived assignment, the partial after product may be observed only
through `after.OTHER`. Each such projection resolves to the declared State
field index and one checked dependency binding. Source order is not evaluation
order: the compiler validates the complete indexed DAG, rejects self-edges,
cycles and unknown fields, and the evaluator executes each node once when all
of its declared predecessors are available.
Independent after axes are finite and have canonical State-field and value
order.
The question rule, key, extrema, shown expressions and representative metric
receive typed contextual access to both endpoints. In compact syntax, bare
names are source aliases with a checked endpoint role. The implementation MUST
NOT guess an endpoint from an unclassified bare expression.

The normalized typed query IR makes the transition non-optional and represents
after construction compositionally per state field:

```text
AfterDependency {
    field_index,
    binding_name
}

AfterFieldSource =
    FrameBefore
  | Derived {
        expression: checked_expr_over_before_context_and_declared_predecessors,
        after_dependencies: [AfterDependency]
    }
  | IndependentDomain(finite_typed_domain)

AfterMembership {
    after_field_index,
    before_dimension_index,
    preconstruction: RelativeIntStep { step }
}

ExploreTransitionIr {
    normalization_version,
    mode,
    state_schema: ClosedCanonicalProductSchema,
    context_schema: ClosedCanonicalProductSchema | Unit,
    after_fields: [StateField -> AfterFieldSource],
    after_membership: [AfterMembership],
    compact_aliases: [SourceAlias -> (role, field_index)],
    boundary_hint?
}
```

For explicit syntax the canonical type IDs resolve the named declared
products. Compact syntax mints versioned internal product identities from its
checked field aliases, types and canonical role order; those identities are not
source-visible.

The transition owns after-endpoint membership. Each `after_membership` closes a
required endpoint check against the indexed canonical Before-axis domain and
retains the checked preconstruction needed to decide structural eligibility
before fallible derived evaluation. The initial preconstruction is a positive
fixed relative integer step. `boundary_hint` only repeats checked accelerator
metadata and can be removed without changing the constructible/admitted
universes, identities or answer.

Identity means every after field is `FrameBefore`. Relative uses only framed
and derived fields and therefore produces one after state for each `(c, b)`.
Independent has at least one `IndependentDomain`; unchanged fields may still
frame from before and other fields may be derived. This supports a comparison
such as “which municipality gives lower tax, if any differs” without a second
architecture: frame the profile and income, vary `after.municipality`, and
compare each concrete after state with its before state.

Compact normalization is deterministic. Without `boundaries`, every checked
query-local state alias becomes a before-state field, context is empty and the
transition is Identity. With `boundaries on x by step`, the checked step value
belongs to Context, Relative derives
`after.x = before.x + context.step`, and every other state field is framed or
recomputed through the normalized after-construction DAG. Derived after nodes
are topologically checked compiler-owned lets over Before, Context and their
explicitly named predecessor nodes. Each node is evaluated once. The private
let environment exposes only those declared predecessors through checked
`after.FIELD` projections (and compiler-owned compact aliases). Unresolved
fields are statically unreachable, runtime placeholders never become state,
and evaluation does not follow source order.

When an explicit transition and `boundaries` coexist, the boundary clause is a
validated endpoint-membership contract. It MUST be entailed by exactly one
Relative after-field derivation and adds the same canonical `after_membership`
obligation as compact syntax: only coordinates whose derived endpoint belongs
to the declared Before-axis domain denote transitions. It cannot add generator
axes, add arbitrary validity predicates or override endpoint construction. A
mismatch is a compile-time diagnostic. The separately retained
`boundary_hint` only accelerates candidate scheduling and proof; deleting that
hint leaves the normalized after DAG and membership obligation unchanged. In
compact syntax the boundary clause synthesizes the Relative update and the
same membership obligation during normalization.
Whether a boundary synthesizes a compact transition or validates an explicit
one is resolved before canonical IR construction. Source origin may be retained
for spans and diagnostics, but it does not select another evaluator.

Transition construction does not itself choose what a mechanism replay
observes. The query-and-report request carries a separate optional typed
observation contract:

```text
MechanismObservationIr {
    endpoint_template: CheckedCallableId,
    state_type,
    context_type,
    observation_type,
    dependency_roots,
    normalization_version
}
```

The checked template has the pure total shape
`observe(endpoint: State, context: Context) -> Observation`. Replay evaluates
that one template once in the before state and once in the after state, with
isolated endpoint runtime and trace state. Its complete identity forms `h` and
participates in the mechanism request and stream identities. It is never
inferred from the two-state Boolean question, from equal result values or from
two positional `show` expressions. Without an admitted template, transition
classification may still close but differential mechanism evidence is
unavailable. The general public spelling for selecting this observation
remains deferred. Any restricted mechanism experiment must still produce and
validate this same observation IR; positional shown-root pairing is not an
alternate mechanism contract.

Every generator axis carries the structural descriptor
`(role, role_field_index, bound_index)`, where role is `Context`, `Before` or
`AfterIndependent`. Its source name is a presentation label only and MUST NOT
be used to resolve the axis, order coordinates or interpret a graph node.
Canonical order sorts first by that role order, then product-field index, then
closed bound index. Values retain canonical domain order. Declared products use
their checked product-field indices; compiler-minted products use the indices
fixed by their normalization version. Report and snapshot dimension entries
serialize all three structural fields alongside the optional display name, and
decision nodes refer to the dimension-array index. This descriptor order
participates in query, domain and stream identities; presentation spelling does
not define `CaseId`. Fixed and derived configuration facts carry the same
role/field/bound ownership descriptor so their State or Context slot is
unambiguous, but they are not varied axes and add no CaseId coordinate. Validity
constraints are typed with `Before`, `After`, `BothEndpoints` or `Transition`
scope. The present `ExploreBoundaryIr` becomes a Relative accelerator hint,
not the definition of Explore. Program, query, mechanism-target and resume
identities bind the transition mode, schemas, frame rule, axis roles and axis
descriptor order. These intensional query identities deliberately bind mode
and after-construction recipes even though extensional `TransitionId` does not.
A journal whose transition or evaluator identity differs fails closed instead
of resuming under different semantics.

Every implementation slice MUST construct the canonical Context, Before and
After products before evaluating scoped constraints, the question or outputs.
A compact bare-name reference is evaluated only by projecting its closed alias
from that frame; it never causes a second assignment environment to be built.
A smaller slice may defer graph publication, mechanism observation or an
accelerator, but it MUST NOT introduce a lower/upper or flat evaluator beside
the canonical transition evaluator.

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
contract and optimization opportunity. Semantically it is compact sugar
for a Relative transition: `after.x = before.x + step`, with every other
endpoint field framed and dependent values recomputed. It does not replace the
Boolean rule.

Without a `boundaries` clause or an explicit non-identity transition,
`? explore` is an ordinary bounded match or violation search over complete
assignments, normalized as Identity.

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
to close through solver `UNSAT`, exact finite exhaustion or an exact coverage
certificate. An incomplete extractor cannot silently narrow the search.
Candidate provenance MUST come from the checked resolver's canonical callable
and declaration identities, not from name matching over raw syntax. Every
reachable construct outside the supported extraction fragment is retained as
an explicit residual. Even when no such residual remains, candidate extraction
alone supplies no proof about the complement.

### Exact symbolic finite differences

For a unit-step resource-cliff query, separate the boundary axis from the
remaining configuration `c`. Let `V(c, x)` mean that the endpoint is valid and
let `N(c, x)` be the exact modeled net resource value. The searched relation
is:

```text
Cliff(c, x) =
    V(c, x) and V(c, x + 1) and N(c, x + 1) < N(c, x)
```

For a declared step `d`, replace `x + 1` with `x + d`. This formulation exposes
the useful object directly: the finite difference
`Delta_d N(c, x) = N(c, x + d) - N(c, x)`. A proof that the difference is
nonnegative on a region closes that region as nonmatching without evaluating
every integer point. A proof that it is negative closes the region as matching.

The first symbolic boundary backend is deliberately narrower than general
Futuruna execution. It accepts the reachable fragment that can be normalized
to exact Presburger or semilinear relations: integer addition and comparison,
multiplication by constants, constant divisors and remainders, finite dispatch
and tables, and piecewise `min`, `max`, clamp and rounding behavior with exact
integer semantics. Variable-by-variable multiplication, approximate floating
arithmetic, unbounded recursion and effects are outside this proof fragment.
They may still be handled by another exact backend or by finite singleton
fallback; otherwise their residual region remains open.

For each exact region of the non-boundary configuration, the backend partitions
the boundary axis at reachable branch and arithmetic events. Between events,
it reasons over interval/congruence cells and derives exact lower and upper
bounds for `Delta_d N`. Congruence is essential: integer division and rounding
can make a derivative periodic even when no source branch changes. Mixed cells
are split further, sent to an exact SMT query, or left for fallback.

The resulting proof-carrying plan has the conceptual shape:

```text
BoundaryPlan {
    program_hash,
    query_hash,
    axis_dimension_index,
    axis_descriptor,
    axis_label,
    step,
    candidates: [guarded singleton support + source event labels],
    certified_intervals: [semilinear support + classification + certificate],
    open_intervals: [semilinear support + reason],
    proof: coverage and disjointness certificate
}
```

`axis_descriptor` is the role/field/bound identity and `axis_label` is display
text only. Here an "interval" may be split into exact congruence classes.
Candidate and
interval supports are guarded by the other dimensions, so a municipality-
specific event is not widened to every municipality. Their disjoint union with
the open supports MUST equal every eligible lower endpoint in the declared
space. The plan and every certificate are valid only for the recorded resolved
program and query hashes; a source change invalidates them.

Source event labels identify which stable rule, guard or arithmetic site
produced a candidate support. They are useful mechanism hypotheses and can be
retained on the case-to-mechanism incidence graph, but they are not themselves
dynamic mechanism signatures and do not prove that the labelled event caused
a cliff. Replay or a separate signature-invariance certificate supplies that
evidence.

Exact counts come from weighted cardinalities of the classified singleton and
semilinear supports, not from the number of evaluations or retained examples.
An example or display cap therefore cannot change an exact count. A search cap
that leaves any `open_intervals` can only report a lower bound with partial,
unknown or unsupported closure, as appropriate.

## Output and Result Identity

Every exploration requires an output key:

```runa
output {
    key [income_before = income]
    ...
}
```

The output block contains exactly one `key`, zero or one `extrema` list, zero
or one `having` filter, zero or one `show` list, and exactly one
`representative` policy, in that order. `having` requires `extrema`; version
one supports only `having varies(EXTREMA_NAME)`.

The existing form:

```runa
output {
    ...
}
```

defines the CLI projection and creates no source-level value.

The specified typed form names one visible declared product:

```runa
# SupportCliffRow(
    income_before: Int,
    income_after: Int,
    household: Household,
    available_before: Int,
    available_after: Int,
    loss: Int
)

output as SupportCliffRow {
    key [income_before = income]
    show [
        income_after = income + step,
        household,
        available_before = available_resources(household, income),
        available_after = available_resources(household, income + step),
        loss = loss_after_next_step(household, income, step)
    ]
    representative maximize loss
}
```

`SupportCliffRow` MUST resolve unambiguously to one declared, concrete,
non-generic product. It cannot be a sum type, alias, collection or synthesized
anonymous record.

Its fields MUST exactly equal the concatenation of `key` and `show`, including
field names, source order and inferred types. No additional fields, missing
fields, reordering or implicit conversions are allowed. Names MUST be unique
across the combined key, extrema and shown fields. Extrema summaries are report
metadata, not fields of the representative row. The representative objective
does not add a row field unless it is itself named in `key` or `show`.

The compiler constructs a row only from a canonically replayed representative.
Rows use the same canonical key ordering as the CLI and JSON result. Naming a
row type does not change result identity or answer-set semantics.

The key answers:

> What counts as one distinct finding?

For searched assignment `x`, let `K(x)` be its key tuple. Without `having`, the
explorer returns one result for every distinct key having at least one
assignment of the requested polarity. A `having` clause may suppress a closed
key group from the row projection; it never reclassifies the group's cases.

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

A continuation receives only the declared row fields, the allowlisted report
identity and the status-specific public stop reason or diagnostics as report
data. Hidden assignments, raw SMT models, solver handles, absolute paths,
timing data and replay-runtime state are not passed to it. Exposing another
searched or replayed value through the report requires adding it deliberately
to `key` or `show`. The continuation may independently compute from its public
row and ordinary program declarations, but that is new program logic rather
than access to the hidden solver assignment or its provenance.

A `then` block is nevertheless an explicit data sink: its user-authored
effects may print, persist or transmit the values it was given. The ordinary
effect and privacy review therefore applies to the continuation.

### Group extrema and `having varies`

Some policy questions ask for one optimum per conditioning group, but only
when the policy outcome actually differs inside that group. This is a grouped
projection, not a new search predicate:

```runa
output {
    key [tax_year]
    extrema [tax_øre = tax_due(municipality, profile)]
    having varies(tax_øre)
    show [
        lowest_tax_municipality = municipality,
        representative_tax_øre = tax_øre
    ]
    representative minimize tax_øre
}
```

Every extrema expression MUST be pure and infer exactly as `Int`. Extrema are
evaluated in source order for every matching configuration after its key is
known. Their aliases are then available to `show` and to the representative
objective. They do not enter the key and therefore do not split a group.

For each raw key group and each extrema field, a closed report records the
exact minimum, maximum, nonnegative spread, group support, and the number of
cases tied at each endpoint. It also records canonical minimum and maximum
witness CaseIds and fresh-replays those witnesses. A representative replay is
not evidence for a different extrema endpoint. Tie-support cardinalities come
from the closed exact case relation; mechanism evidence remains separate.

`having varies(name)` emits a group exactly when that field's minimum is less
than its maximum. On an open prefix, observing two different values proves
only a lower bound that the group will be emitted; observing equal values does
not prove suppression. Version one therefore publishes grouped extrema rows
only after projection and aggregation closure.

Let `R_raw` be all matching key groups, `R` the emitted groups, `M_emit` the
matching configurations whose key is emitted, and `M_suppressed` the matching
configurations whose key is suppressed. A closed report conserves:

```text
|R_raw| = |R| + |R_raw - R|
|M| = |M_emit| + |M_suppressed|
sum(row support for k in R) = |M_emit|
```

`D`, `M`, coverage, the search decision DAG and an authorized matching-case ledger all
remain pre-`having`. Thus an invariant control may truthfully report
`M = 98`, `R = 0`, one suppressed group and no row. The public report MUST
carry whether its group filter was `all` or `varies(name)` so identical-looking
rows cannot erase the projection contract.

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

Named extrema and `show` expressions are evaluated in clause/source order and
may be referenced where their clause makes them available. Duplicate names,
forward references and cycles are rejected.

The report states that a representative is one case for the key. It MUST NOT
imply that every hidden assignment has identical shown values.

## The Exploration Relations and Graphs

The semantic truth of an exploration is its transition relation: which
generated before-to-after edges are admissible, which admissible edges match,
what the declared outputs evaluate to and, where requested, what a fresh replay
observes. A flat row list is only one projection of that relation. Futuruna may
represent closed parts of it with three linked but independently scoped
graphs:

1. a query-relative **search decision DAG** saying exactly which generated
   transition cases match, do not match, are excluded or remain unresolved;
2. a **semantic transition graph** whose canonical state nodes and directed
   edges show the before-to-after cases represented by that search evidence;
   and
3. a query-and-observation-relative **mechanism DAG** saying which stable
   rules, branches and operations formed each replay-derived mechanism
   signature.

The search DAG answers **which generated coordinates have what
classification**. The transition graph answers **what state change each case
denotes**. The mechanism graph answers **how the observed change was
computed**. None is allowed to redefine another, and none is required for a
projection-only report that has closed its declared result keys by another
exact method.

The final transition-aware artifact contract names the search/classification
DAG `search_decision_dag` and the state/edge graph
`semantic_transition_graph`. The implemented snapshot-v6 and exact-answer-v5
artifacts can currently expose only the former, under `graph.case_graph`; that
object is always the search decision DAG and is never evidence that a semantic
transition graph was serialized. Public state/edge serialization remains a
pending slice over the already canonical in-memory identities and exact
singleton support.

### Canonical search decision DAG

Let the independently varied bound dimensions, in canonical structural
descriptor order, be
`A_0, ..., A_n-1`. After transition lowering, their Cartesian product is the
declared generator-coordinate space:

```text
U_D = A_0 x ... x A_n-1
```

The search decision DAG represents one total evidence classification over the
canonically ordered transition generator axes of `U_D`:

```text
F(x) = excluded
     | eligibility_open(reason)
     | admissible_nonmatch
     | admissible_match
     | admissible_open(reason)
```

`excluded` means either structural membership in `U_C` is known to fail or, for
a constructible coordinate, validity membership in `D_C` is known to fail.
`eligibility_open` means those memberships are not yet both known.
`admissible_nonmatch` and `admissible_match` are closed classifications inside
`D`. `admissible_open` means membership in `D` is known but question polarity
is not. Mechanism availability is never a case terminal.

For the exact regions carried by a partial artifact, require the disjoint
conservation invariant:

```text
U_D = E_closed + N_closed + M_closed + O_eligibility + O_polarity
```

where `+` denotes disjoint union. Admissibility closure means
`O_eligibility` is empty and therefore `D` is exact. Polarity closure means
both open sets are empty and therefore `M` is exact. Each open region retains
its own reason; aggregate status does not erase that frontier.

The graph is a reduced ordered multi-terminal decision diagram. Its
`dimension_index` points into the report's ordered structural axis-descriptor
array; it is never resolved from a display name:

```text
CaseNode {
    dimension_index,
    arcs: [exact_ordinal_set -> child]
}
```

If any declared domain is empty, `U_D` is empty. Its unique canonical
representation is the distinguished `empty_space` root with zero decision
nodes, zero classified paths and cardinality zero. The nonempty-arc rules below
apply only when `U_D` is nonempty.

The following rules are normative:

- decision dimensions are only role-tagged independently varied `in` bindings,
  including explicit independent-after domains;
- fixed facts and framed or derived after fields are not new dimensions;
- dimensions follow the normalized Context/Before/AfterIndependent generator
  order by `(role, role_field_index, bound_index)`, and values follow canonical
  domain ordinal order;
- outgoing arc sets are nonempty, disjoint and exhaustive for that dimension;
- child dimension indices strictly increase along every edge;
- arcs to the same child are coalesced into one canonically normalized ordinal
  set, and arcs have a canonical order;
- equal terminals are interned;
- nodes at the same dimension with identical complete value-to-child mappings
  are merged;
- a node is removed only when every value of its dimension reaches the same
  child; and
- omitted dimensions therefore mean proven independence, never existential
  omission.

Several disconnected paths may share one suffix or terminal. They denote the
exact union of those paths, not the Cartesian product of their marginal values.
For example, sharing a leaf between `(Copenhagen, false, 100)` and
`(Laesoe, true, 200)` does not assert either crossed combination. Non-contiguous
integer support remains a union of exact ranges and is never widened across a
gap.

Counts use weighted path multiplicity, not node or leaf counts. An arc
covering `s` values contributes `s * suffix_count(child)`; skipped dimensions
contribute their full domain cardinalities. Because a shared node may be
reached after different skipped dimensions, cardinality belongs to a path
context or weighted edge, not intrinsically to the shared node. Counts use
checked arbitrary-size nonnegative integers.

These reduction rules make the graph canonical for a fixed dimension and
domain-value order, up to report-local node renaming. A heuristic or learned
decision tree may be derived for presentation, but it is not the proof object
and may not change membership, counts or status.

### Semantic transition graph

The pending optional public semantic graph will materialize the state-and-edge
meaning of a named closed or partially closed transition population. The exact
runtime currently retains only the private singleton support described above:

```text
StateNode {
    state_id,
    state_schema_id,
    state_type_label,
    canonical_value_or_authorized_projection
}

TransitionEdge {
    transition_id,
    transition_type_id,
    before_state_id,
    after_state_id,
    canonical_context_or_authorized_projection,
    supporting_case_ids_or_exact_region,
    classification,
    observations
}
```

Its request names a transition-bearing population—constructible, admissible,
matching or selected representatives—and its disclosure projection. State
values and context are private unless the report request authorizes them. An
edge is published only after both endpoint identities and its support are
validated. Structurally excluded coordinates and open search regions remain in
the search decision DAG; they are not invented as endpoint nodes with guessed
values.

One state node may participate in many incoming and outgoing edges. Two edges
with equal endpoint state values remain distinct when their canonical context
or typed State/Context schema identity differs. Transition mode, recipe/DAG
topology and mechanism path do not split an otherwise equal edge. Conversely,
if explicit semantic-edge
deduplication proves several generated cases have one `TransitionId`, the edge
retains their exact support rather than discarding multiplicity. State-node
count, distinct-edge count and supporting-case count are therefore different
populations. Graph node counts MUST NOT be used as substitutes for exact
weighted support counts.

Semantic identity has its own reduction closure. For a named case-coordinate
scope `X`, exact distinct-edge and state-node counts require both:

1. `X` is closed under the requested admissible, matching or representative
   prerequisite; and
2. `tau`, `StateId` and `TransitionId` have been evaluated for every coordinate
   in `X`, or an exact certificate proves their image and deduplication over a
   whole region.

A closed search support count alone need not close an image-cardinality count;
a symbolic region may cover millions of `CaseId`s without yet proving how many
distinct state or transition identities they denote. Observed IDs are lower
bounds while this identity-reduction frontier is open. Conversely, omitting or
capacity-limiting graph serialization does not weaken an already closed scalar
edge or node count.

The semantic graph is an explanatory projection, not the coverage proof. It
may be omitted or reported `capacity_limited` while the search decision DAG and
scalar transition-case counts remain exact. A partial semantic graph reports
lower-bound node and edge populations under its own materialization frontier.
Mechanism incidence attaches replay evidence to the supported directed edge;
equal endpoint values never manufacture a shared mechanism signature.

### Differential mechanism DAG

The resolved program supplies a stable semantic-site catalogue: declarations,
rule candidates, dispatch choices and control-flow decisions identified by an
analysis-program identity plus structural AST paths. This catalogue describes
possible behavior and need not itself be acyclic. Source spans and author
labels are display annotations rather than identity.

A mechanism request fixes a `MechanismObservationIr` specification `h`: one checked
endpoint-observation template, the value roots whose dependencies
matter, the before/after pairing, and the signature-normalization version. It
also names a requested target population `S_req`: either the canonical
representative set `Rep(R)` or all matching transition cases `M_C`. Let
`T_trace` be the transitions actually traced, with `T_trace` a subset of
`S_req`. Fresh replay defines a deterministic, canonical and total map on that
scope:

```text
Sigma_(q,h) : T_trace -> Signature
```

`S_req` is not known until its prerequisite closes: representative scope needs
projection plus representative-policy closure, and matching scope needs
polarity closure. The durable implementation may nevertheless replay a case
already confirmed inside the eventual target and publish its signature as a
`scope_open` lower bound. It rejects every unconfirmed rank; only prerequisite
closure turns the accumulated known support into exact `S_req`. Changing an
unrelated case view does not change `h`. Changing the observation roots or
endpoint-pairing contract creates a different mechanism request and signature
identity.

Each endpoint replay constructs a finite dynamic occurrence DAG. Occurrence
nodes are annotated with stable semantic-site IDs, an event kind and canonical
causal, data or control-dependency edges. Repeated visits to one site remain
distinct occurrences when multiplicity matters; edges follow replay order, so
interning cannot introduce cycles. A differential signature consists of the
two endpoint-local DAGs plus a fail-closed partial correspondence between
outcome-free stable occurrence slots. Its logical vertices remain
endpoint-qualified even when a compact representation stores a matched pair
together. The uncoloured union of before and after dependency edges is not the
mechanism graph and need not be acyclic when execution order reverses. Empty
endpoint DAGs are valid. Common normalized subgraphs may be hash-consed across
signatures.

Local visit or invocation ordinals may participate in the correspondence only
when the matching semantic group has compatible multiplicity at both
endpoints. Otherwise an earlier endpoint-only repetition could silently shift
the ordinals, so pairing is unavailable rather than guessed.

#### Implemented first nested trace profile

The first private nested profile is implemented narrowly. The paired lower and
upper shown expressions MUST resolve to the same checked top-level function.
That common endpoint call is the implicit trace root. Its execution at each
endpoint MUST contain exactly one nested, direct, positional activation of one
other checked top-level function, and that helper MUST execute exactly one `if`
decision exactly once. The helper call alone contributes one outcome-free
activation frame; the endpoint root is not repeated as a frame. The resulting
endpoint trace contains exactly one actual `IfDecision` from the canonical
shown-value evaluation.

The trace sink MUST observe that canonical evaluation in place. It MUST NOT
evaluate the condition, selected body or helper call separately to reconstruct
an outcome. The frozen-profile selector rejects source slices containing
short-circuit Boolean operators, `match`, rule dispatch, recursion, named
arguments, more than one nested activation, repeated activation of the event-
bearing helper, or more than one dynamic-control event; that source cannot
authorize a mechanism stream. The admitted expression subset is limited to
variables, literals, unit, non-short-circuit binary and unary operations, the
two direct endpoint calls, the one direct helper call, the selected `if`, and
one-expression blocks containing those forms. Lists, tuples, fields, indexing,
lambdas, pipes, effects, extra applications and every other expression form are
rejected during plan construction, including when they occur in an unreachable
branch. Runtime checks remain a defense against artifact/execution divergence;
they MUST never represent an unsupported or integrity-failed event by silently
omitting it.

The mechanism runtime MUST execute the immutable root syntax owned by the
checked artifact, after revalidating both its complete root digest and root
`ModuleId`; a caller-owned statement buffer is not replay authority. Every
checked declaration MUST have root origin, and a recursive AST walk MUST reject
plain, qualified and hash imports before the run store is opened. Import support
requires a frozen module graph whose nodes retain immutable AST, origin
directory and local module identity and whose edges bind parent module plus
import occurrence to a target. A flattened import sequence is not equivalent
because it can change function hoisting and binding initialization order.

The runtime MUST also authenticate the actual closure before assigning any
checked callable or body site. Fresh initialization mints an opaque interpreter-
local capability only when a root top-level closure is unambiguously located at
its checked declaration occurrence; the capability binds both
`AnalysisProgramId` and `CheckedCallableId`. The implicit endpoint and nested
helper MUST each match the plan target. A missing or unequal capability is a
hard replay-integrity failure: no mechanism observation or permanent-untraced
evidence may be committed. By contrast, an operational resource limit leaves
the same mechanism rank open and uncommitted for retry.

The older direct-`if` profile MAY continue to reconstruct its checked condition
to retain its wider pure-expression subset, but it MUST authenticate the actual
canonical show endpoint closure immediately before doing so. A shadowing value
or different same-named closure is an integrity failure and MUST NOT authorize
reconstructed mechanism evidence.

`DynamicControlV1` currently retains the complete supported executed control
trace beneath each endpoint root; it performs no relevance pruning. In the
frozen profile that complete trace has one occurrence. Before relevance
pruning or repeated event-bearing invocations can be admitted, the
correspondence contract needs a call anchor that remains sound when an earlier
endpoint-only invocation is retained on one side or pruned from another.
Recorder-local visit and invocation ordinals MUST be assigned against the
complete executed trace, never renumbered after pruning, and pairing MUST stay
unavailable where the checked call anchor and compatible multiplicity do not
establish correspondence.

Before and after endpoints have isolated runtime and trace state: activation
stacks, visit ordinals and invocation ordinals never flow from one endpoint to
the other. A future endpoint-value cache may stand in for execution only when
the cached entry carries identity-bound result and complete trace provenance
for the same analysis program, mechanism request, checked call anchor and
canonical inputs. A value-only cache cannot mint mechanism evidence. This
first profile is matching-only: its requested mechanism population is all
confirmed matching configurations (`S_req = M`), not representatives or
nonmatching cases. General target scopes remain part of the RFC but outside
this executable slice.

The focused executable fixture has four declared incomes and three valid lower
boundary cases. It closes with three exact signatures—`Else/Else`,
`Else/Then`, and `Then/Then`, each with support one—and zero known target cases
left untraced. Each signature contains the same checked helper activation and
`if` site; only the endpoint outcome pair differs. Recovery after the first
committed mechanism block reconstructs identical reduced evidence before the
remaining two blocks are minted.

A separate source-boundary fixture supplies the same helper through a plain
import and proves that plan construction refuses the live module graph with the
frozen-module-graph requirement. The refusal happens before store creation or
mechanism minting; unchanged live files are not treated as immutable evidence.

A transition mechanism compares the encoded computations for its before and
after endpoints and retains the dynamic data/control slice relevant to the
roots fixed by `h`. It explains how the encoded rule graph's relevant execution
differs across the declared edge; it does not by itself establish a real-world
temporal or causal transition. Candidate atoms include:

- a selected default, exception or rule clause changing;
- an `if` or `match` arm changing;
- an operation being reached at only one endpoint; and
- an exactly supported discrete arithmetic regime changing.

Raw input values and loss amounts are observations attached to a signature;
they are not automatically part of structural mechanism identity. Otherwise
every configuration would become a different mechanism. Conversely, reaching
the same top-level rule is not sufficient when different relevant branches or
dependencies changed.

Mechanism tracing consumes the normalized transition and one checked
endpoint-observation template. A restricted implementation slice may admit
only a narrow set of checked templates, such as the Personskat transition
helper, but it evaluates them against the same canonical before/after frame.
When sound pairing is unavailable, mechanism evidence is reported as
unavailable rather than guessed.

The exact transition-case-to-signature incidence is retained separately as
another ordered decision DAG over the same generator axes and canonical order
as the search decision DAG. Once `S_req` is exact, its terminals are `outside_scope`,
`signature(id)` and, for a known target case not yet traced,
`known_target_untraced(reason)`. This representation can encode correlated and
disconnected fibers without widening them into Cartesian rectangles. The
signature regions are disjoint and their union is exactly `T_trace` when mechanism
incidence is closed.

A case normally traverses several mechanism atoms, so atom supports may overlap
and their counts MUST NOT be summed. One complete canonical signature per case
does define a partition. If a case exposes several separately named mechanism
roots, their canonical sorted set is the complete signature used for that
partition.

For a closed traced scope, the exact signature fibers satisfy:

```text
T_trace = disjoint_union {
    t in T_trace | Sigma_(q,h)(t) = sigma
} for sigma in Gamma_trace
```

By contrast, the support of an individual atom `a` is
`{t in T_trace | a occurs in Sigma_(q,h)(t)}`; those supports may overlap.

Mechanism evidence has its own scope and closure:

- `scope_open` means `S_req` is not yet exact because its case or
  representative prerequisite is open;
- `incidence_open` means `S_req` is exact but `T_trace` is a strict subset and the
  known target remainder is retained explicitly;
- `representatives_closed` means `S_req = Rep(R)` and `T_trace = S_req`;
- `matching_closed` means `S_req = M_C` and `T_trace = S_req`;
- `unavailable` makes no automatic mechanism claim and counts no synthetic
  “unknown mechanism” signature.

Complete case classification does not imply complete mechanism incidence, and
incomplete or unavailable mechanism evidence does not change an otherwise
exact exploration status.

### Cases and mechanisms are independent quotients

For matching transition cases, `K(t)` is the declared result key. On the traced
population `T_trace`, `Sigma_(q,h)(t)` is the mechanism signature. They induce two generally
incomparable equivalence relations where both are defined:

```text
t ~key u        when K(t) = K(u)
t ~mechanism u  when Sigma_(q,h)(t) = Sigma_(q,h)(u)
```

One result key may contain several mechanisms. One mechanism may span several
keys, loss values and disconnected case regions. Their exact joint view is the
observed incidence set:

```text
{ (K(t), Sigma_(q,h)(t)) | t in T_trace }
```

It is never replaced with the Cartesian product of all keys and all
mechanisms. A representative policy selects one case for each key class; it
does not select or prove representatives for every mechanism class.

### Mechanism fibers and cardinality

Where `h` contains the query-independent observation mapping and is defined
over a typed program input space `X_h`, the potential fiber of a mechanism
signature `sigma` is:

```text
fiber_h(sigma) = { x in X_h | Sigma_h(x) = sigma }
```

That global fiber may be empty, finite, infinite or not decidable by an
available backend. A bounded query reports only the observed preimage inside
its named traced scope `T_trace`, which is a subset of finite `D_C`. Every such bounded
fiber is finite, even when the same mechanism applies to infinitely many
inputs outside the query.

Reports therefore say, for example, “17,421 matching configurations within
these bounds have this mechanism signature.” They do not claim that the
mechanism has exactly 17,421 transition cases globally. A reachable program
mechanism with no cases in `D_C` is unobserved under this exploration, not absent from the
program.

#### Capped support summaries

A per-mechanism cap must say which quantity it caps. Futuruna distinguishes:

- **support cardinality**: how many configurations belong to the signature
  fiber in the named scope;
- **materialized cases**: how many complete case rows were retained; and
- **displayed examples**: how many retained cases are printed.

For a requested support threshold `c`, the engine may compute the complete
saturating summary:

```text
capped_c(n) = exact(n)    when n < c
              at_least(c) when n >= c
```

`at_least(c)` is a complete answer to that deliberately coarsened view. It is
not an exact count and is not evidence of infinity. `proven_infinite` is a
separate certificate available only under a future unbounded-domain contract
with an actual infinity proof. A cap reached for a bounded query means only
that the finite support inside `D` is at least `c`.

The same lower-bound shape can also arise from an unclosed frontier, but its
closure metadata is then different. For example, 37 confirmed matches plus a
solver-unknown region yields `lower_bound(37)` with exploration status
`unknown`; a timeout yields the same bound with status `partial`. If case
polarity is closed but mechanism tracing is not, `|M|` may be exact while each
observed signature has only a lower-bound support and additional signatures
  may still exist. Count certainty says what is proved; layer status says why
more precision is absent.

A report can therefore state independently:

```text
distinct mechanism signatures: X
matching configurations: Y exact | at least Y
materialized concrete cases: E
mechanism signatures saturated at support cap c: Z
```

A future unbounded report may additionally carry
`mechanism_signatures_proven_infinite: I`; bounded v1 omits that field rather
than emitting a misleading zero.

Version one fresh-replays every case in the mechanism target, streams those
assignments into a reduced weighted incidence DAG without retaining every row,
and keeps at most an independently named example cap `e` per signature. An
example-retention cap never changes exact counts or closure. If execution stops
assigning target cases, mechanism incidence remains open and the distinct-
signature count is only a lower bound. Only the future explicit proof-backed
mode may assign one replayed signature to a proven homogeneous region.

A requested full closed incidence DAG necessarily reveals exact bounded
support counts through weighted paths, so it always uses `exact` support mode.
A `saturating` support request is an alternative coarsened aggregate: it omits
or redacts full incidence and exposes only `exact(n)` below the cap or
`at_least(c)` at the cap. Example retention remains independently capped in
either mode.

Because complete signature fibers partition `T_trace`, their exact or lower-bound
support summaries may be combined with the usual bound arithmetic. Individual
rule/branch atom supports still overlap and remain non-additive.

Truly infinite exploration domains would require a separate contract with
symbolic fibers, infinite-cardinality classes and non-list result forms. They
are not silently admitted by this bounded feature.

### Derived views

Projected results, configuration ledgers, coverage, partitions and case-value
histograms are deterministic views of named closed case and value relations.
They do not launch narrower hidden explorations and do not require mechanism
evidence.

- `results` contains one deterministic representative per result key;
- a configuration ledger contains one row per exposed matching `CaseId`;
- coverage derives from exact admissible and matching region counts;
- a partition groups exact case regions by declared fields; and
- a histogram sums an exact per-case value relation over a named population.

For a histogram field `H` over population `P`, `H(x)` must be total and
replay-closed for every `x` in `P`. Its bins are disjoint and exhaustive, and:

```text
count(bin_j) = |{ x in P | H(x) is in bin_j }|
sum_j count(bin_j) = |P|
```

Mechanism multiplicities cannot substitute for this value relation because
one mechanism signature may span several loss values.

A **mechanism-bin histogram** is a different, explicitly mechanism-scoped
view. For a numeric observation `H`, bin `B`, and the bounded signature fiber

```text
fiber_trace(sigma) = {
    t in T_trace | Sigma_(q,h)(t) = sigma
},
```

define:

```text
mechanisms_in_bin(B) =
    |{ sigma in Gamma_trace |
        exists t in fiber_trace(sigma), H(t) is in B }|
```

This counts distinct query-relative dynamic signatures represented in the bin,
not cases, projected findings, rule atoms or legal provisions. One signature
may contain cases whose `H` values fall in several bins, so it is counted once
in each such bin. Mechanism-bin counts therefore overlap and MUST NOT be summed
to obtain `|Gamma_trace|`.

A legal-mechanism family such as "the § 9 C phase-out" is a coarser authored or
derived grouping than a dynamic trace signature. One family may contain
several signatures because different dispatch, `min`/`max`, rounding or control
paths were relevant. A family-bin histogram is valid only as a separately
named quotient; its counts cannot be substituted for signature-bin counts.

`mechanisms_in_bin` is exact only when the target scope `S_req` is exact,
signature incidence is closed with `T_trace = S_req`, and `H` is total and
replay-closed for every case in that target. With open incidence, confirmed
signature/bin witnesses give only a lower bound; with unresolved loss values
or membership, the affected bin remains unknown rather than receiving zero.
Example-retention and display caps do not change this certainty, while a cap on
signature enumeration does.

`empty` means no admissible case exists. `none` means admissible cases exist but
none match. `some` and `all` compare matching with admissible membership.
Incomplete global closure yields `undetermined` even when some confirmed cases
are already known.

The source syntax for requesting additional graph views is intentionally left
open until the graph artifact has executable corpus experience. The durable
CLI currently exposes exactly one case-graph choice: `--case-graph full`.
The engine represents it in an explicit `ReportRequest`: the report-request
digest binds `full` versus `omit`, the retention-authorization digest binds
case-classification disclosure, and the snapshot/terminal schema digests bind
the lowerer and serialization contracts with their fixed limits. All are
immutable run identity.
An omitted-graph run therefore cannot be resumed with full disclosure enabled,
or vice versa. The baseline request omits the full search decision DAG and ledger,
requests no case-level view, and permits only representative provenance when
available. A future source form may construct a larger request, but canonical
output never silently expands it.

`output.key` continues to define result identity. Adding a case/value view
MUST NOT change the question, universe or case classification. Changing the
mechanism observation roots is an explicit change to `h`, not an incidental
effect of requesting a histogram.

A full search decision DAG and a configuration ledger reveal different case-level data:
the graph can reveal nonmatches and exclusions, while the ledger can reveal
per-match values. Both require explicit case-level disclosure authorization
and are omitted by default. Full matching-case mechanism incidence likewise
requires explicit opt-in. Default result provenance remains
representative-scoped.

### Limits

The limit classes have different meanings:

| Limit | Contract |
|---|---|
| Source `bounds` and `where` | Defines the proposition by defining `D` |
| Source `probes.at_most` | Defines completion of the initial scheduling plan; never restricts `D` or closes an unprobed case |
| Invocation time, work or artifact budget | Commits the current evidence and exact open frontier, then pauses the run |
| Answer/case/value classification cap | Stops required closure at a resumable open frontier; current answer evidence is `partial`, which an admitted snapshot reports |
| Mechanism-only tracing budget | Leaves mechanism status `scope_open` or `incidence_open` without changing exploration status |
| Noncanonical presentation or example budget | Limits console rows, per-mechanism examples or explanations without truncating an authorized canonical graph or ledger |

Timeouts, result caps and graph-resource budgets are never translated into
hidden solver constraints. Increasing a search budget may refine an open
region into closed regions, but it cannot reclassify a previously closed
region; such a contradiction is an error. Semantic query identity excludes
operational budgets. The artifact-content hash changes when its exact evidence
or open frontier changes, while run metadata records the budget that produced
it.

“Show at most five example cases per mechanism” is therefore a presentation
limit; a requested canonical ledger remains complete and untruncated. “Stop
mechanism tracing after five signatures” leaves mechanism evidence open, but
does not make an otherwise closed answer/case/value result partial. A budget
that stops case classification or required value derivation does. Only
changing `bounds` changes the world being claimed.

Timeout, work, and pressure controls pause between whole semantic commits. The
first in-memory mechanism reducer additionally binds fixed cumulative resource
ceilings into stream identity: retained signatures and their nodes/edges,
their nested activation-path steps, keyed support fibers and intervals, and
retained examples. Incidence materialization is separately charged for rank
intervals times traversed dimensions before constructing its DAG. Reaching a
ceiling rejects the next complete mechanism block or observer view before
append and leaves the existing run resumable; it never truncates a signature,
silently drops incidence, or changes case-answer closure. A later
storage-backed reducer can advertise a different contract digest and higher
ceilings.

### Durable observable run

Explore execution is a durable, observable stream of monotone evidence, not a
single opaque call whose only meaningful product appears when the process
exits. The stream is an execution protocol outside the pure Futuruna question;
it is not a source-language `Stream`, cannot be reached by `over`, and cannot
feed observer effects back into evaluation.

Three status axes remain independent:

| Axis | States | Meaning |
|---|---|---|
| Run lifecycle | `running`, `paused`, `sealed` | Whether this durable run may accept more work |
| Answer closure | `partial`, `complete`, `unknown`, `unsupported`, `error` | What has been established about requested answer/case/value layers at the current committed cursor |
| Mechanism closure | the mechanism-evidence status contract | What has been replayed or proved about the requested mechanism population |

A paused run normally has `partial` answer evidence at its committed journal
cursor; an admitted snapshot makes that state observable, but a journal-only
pause is equally resumable. `paused` is not a terminal answer status. `unknown`
or `unsupported` may either pause a run with an explicit open obligation that
another exact backend can attempt, or seal a non-successful run when no
permitted continuation exists. Only a `Completed` seal asserts complete answer
closure.

Answer closure may become `complete` while requested mechanism replay is still
running or paused. That does not reopen the answer, but the observable run may
continue producing mechanism evidence. A mechanism-only operational cap leaves
the run resumable even though its answer evidence is complete. The terminal
seal requires that no permitted work remain under the selected run contract:
every completion-blocking frontier is closed, or its independent layer has a
terminal `unavailable` outcome. An operationally capped open mechanism frontier
is not terminal merely because answer closure succeeded.

A fixed implementation ceiling is reported separately from operational host
pressure. In particular, a V1 mechanism reducer ceiling returns a typed
`mechanism_limit` at the still-open rank and states that unchanged resume cannot
advance; it requires a later storage-backed mechanism contract rather than
blindly retrying the same observation as transient `resource_pressure`.

The first durable record is `RunOpened`. It binds `run_id` to the selected
`program_hash`, `analysis_program_hash`, `query_hash`, `domain_hash`,
`report_request_hash`, `probe_plan_hash`, evaluator contract and canonical
schema versions, including the full canonical case space. It MUST NOT bind the
post-certificate residual support or shard width as if either existed at
genesis. Jobs, time limits, checkpoint cadence, filesystem paths, shard width
and observer attachments are operational slice metadata. They may differ
between resumptions and MUST NOT change the question or invalidate already
accepted evidence.

After proof preparation, a validated `CoveragePlanAccepted` record may bind one
proof-set identity, its certified closed supports, exact residual support and a
sharding epoch. Later plans may refine only the then-open frontier. Accepting a
new plan obeys the same conservation invariant as every other semantic commit;
it cannot rematerialize a proof-closed region or make operational sharding part
of the semantic run identity.

The journal distinguishes semantic evidence from control and discovery
records. Its minimum logical record classes are:

- probe decisions and candidate discoveries, which explain scheduling but do
  not close any case;
- evaluated singleton classifications and validated exact region
  certificates, which close their stated disjoint support;
- representative and extrema witness replays, which close only their stated
  selection obligations;
- replay-confirmed mechanism-signature observations, which extend only the
  named mechanism target;
- committed open-frontier transitions and snapshot cursors; and
- lifecycle records for open, pause, resume and terminal seal.

Worker output is a proposal, not a journal event. The coordinator checks its
run identity, exact support, disjointness, classification and proof or replay
receipt before accepting it. Accepted evidence never retracts or changes a
closed fact. A contradiction fails closed instead of rewriting history.
Scheduling hypotheses, cost estimates and provisional discoveries may be
superseded because they are explicitly not semantic evidence.

Mechanism observations use two complementary commitments. The ordered journal
retains each canonical block, complete referenced signature definitions, and
validation receipts. The normalized answer layer instead keys a fact by the
checked mechanism request and complete semantic outcome, with an exact
compressed case support as its subject. It deliberately excludes CaseId from
the key, validation receipts, arrival order, and batch boundaries. Replaying
one wide block or the same disjoint cases in many blocks must therefore produce
the same mechanism evidence root, while the journal still proves how the work
arrived. Before matching scope closes, only already classified matching support
may authorize a mechanism observation; those facts are immediately visible as
`scope_open` lower bounds. Exact case closure seals that same support as the
target without enumerating its ranks.

The first mechanism-aware slice scheduler runs only after the checked probe
milestone and gives confirmed mechanism backlog strict priority over another
case classification. Replay and classification are separately admitted atomic
subjects, `MechanismCaseIdRank(rank)` and `CaseIdRank(rank)`, under the same
resource envelope. After one matching classification, its mechanism rank must
therefore cross a replay boundary before classification can expand again. A
failed or operationally capped replay commits neither the rank nor a prefix;
the same rank remains first after resume. When neither frontier has work, the
coordinator publishes `matching_closed` count evidence and pauses at the
typed `mechanism_observation_closed_terminal_unavailable` frontier rather than
invoking the exact-only finalizer. The stop states that unchanged resume cannot
advance until a mechanism-aware terminal publication contract exists.

Before the probe milestone, a mechanism checkpoint is not defined. A
journal-only pause there is therefore a complete resume boundary but not
observer-view debt. After the milestone, a journal-only pause does create view
debt, and resume must service it before admitting another semantic work unit.

The private mechanism-enabled identity reuses `SnapshotPublished` and
`SnapshotUnavailablePublished`, but its snapshot schema digest dispatches
those records to `futuruna.explore.mechanism-checkpoint.v1` rather than the
exact-only snapshot-v6 schema. The canonical count-only checkpoint binds its
pre-publication cursor, run and journal/evidence heads, checked request and
observation identities, probe/classification progress, closure status,
confirmed target/traced/untraced populations, distinct signature certainty and
requested bin counts. It discloses no signature definitions, CaseIds, examples
or incidence DAG. Exact case closure may supply an unforgeable closure-gated
target-support token to this projection without first materializing a target
DAG. If the full bounded document cannot fit, an independently capped
capacity receipt commits the same cursor and progress; it is valid only when
canonical reconstruction proves the
full document exceeded its fixed limit. Recovery rerenders the appropriate
document from the pre-event reducer state and requires byte equality before
applying the journal record. Exact-only snapshot identity and bytes remain
unchanged.

Every semantic commit MUST conserve the frontier exactly. If `C_new` is the
newly accepted disjoint closed support, then:

```text
previous_open_frontier = C_new disjoint_union next_open_frontier
```

`C_new` may be empty for a control-only commit. It may neither overlap prior
closed support nor omit a case from both sides. Cardinality conservation alone
is necessary but insufficient; the support identities themselves must prove
the equality.

The durable transition need not repeat both growing frontier bodies. The
current exact implementation stores the previous authenticated frontier
commitment, bounded `C_new` delta and proposed next commitment. Replay applies
the delta to the persistent canonical interval tree, derives the successor and
checks both commitments plus support conservation before advancing. Cached
subtree cardinalities and hashes make scheduling, subtraction and commitment
delta-proportional; full interval materialization is confined to bounded wire
or publication boundaries. This is the ledger analogue of storing a state
root plus transaction, rather than copying the entire chain state into every
block.

Storage MAY batch logical records into immutable content-addressed chunks.
Each accepted record advances the order-sensitive journal head from its prior
head and canonical record bytes. Parallel workers may prepare disjoint chunks
concurrently, but only the coordinator assigns committed sequence and advances
that head. Separately, every semantic commit derives an evidence root from the
canonical normalized evidence set and exact frontier; control records do not
change it. Thus arrival order and pause history may change the journal head but
never the evidence root or final answer identity. This is a Merkle-style
evidence journal, not a distributed blockchain: it needs neither mining nor
consensus, and it avoids forcing every worker through one global serialization
point. Disposable indexes and rendered graphs may be rebuilt from the accepted
journal and cannot add coverage.

Event partitioning and producer method are likewise not semantic identities.
If the same normalized classification is accepted for ranks `[a, c)`, the
evidence map has the same canonical state whether one worker proposed
`[a, c)`, two workers proposed `[a, b)` and `[b, c)`, or an evaluator and a
certificate established disjoint pieces. The journal retains those distinct
provenance records, but the evidence map unions equal semantic facts over
normalized exact support. Operational chunks, shards and commit batch sizes
MUST NOT become evidence leaves. Implementations therefore update an
authenticated semantic map incrementally; re-hashing or copying the complete
accepted set on every commit is not an acceptable long-run algorithm.

Content addressing provides integrity, gap detection and reconstruction inside
the owner-local trust boundary; it does not authenticate a hostile owner or
make singleton classifications independently verifiable. Certificates and
fresh publication replays remain the semantic validation mechanisms.

The full observer protocol permits a versioned snapshot to be derived at a
committed cursor with exact counts or labelled lower bounds, confirmed rows,
provisional discoveries, requested histograms, open frontier and run metadata.
The current executable slice first commits an authoritative journal pause, and
materializes snapshot v6 only when a separate bounded observer phase is
admitted before the invocation deadline. If admission is denied, invocation-v1
returns a typed journal-only checkpoint with snapshot status `deferred`; no
snapshot blob or graph is minted. Reopening such a pause services the pending
snapshot before dispatching more semantic work and stops at a typed
`snapshot_catch_up` boundary. The artifact still says whether that attempt
materialized the view, published a bounded `snapshot_unavailable` capacity
receipt, or remained deferred, so repeated time-boxed search slices cannot
silently outrun observation forever.

When the view phase is admitted, the baseline snapshot marks the search decision DAG
`not_requested`. A run created with `--case-graph full` instead publishes
either the entire canonical total current-evidence DAG or typed
`capacity_limited` evidence; it never publishes a node or rank prefix. The DAG
covers the full declared universe: closed support ends at `excluded`,
`admissible_nonmatch` or `admissible_match`, and its exact current open
complement ends at `eligibility_open(search_budget_exhausted)`. Thus totality
describes the current evidence partition, not completed exploration.
Operational snapshot deferral is not case-graph capacity evidence, does not
alter semantic evidence, and does not change the immutable graph request or
run identity. Mechanism-DAG publication remains absent and mechanism evidence
is `unavailable_deferred`. The underlying relations and closure evidence are
monotone; serialized graph node identifiers, reduced layout, provisional
representative choices and presentation order need not be. Privacy and
disclosure rules apply to every event and admitted snapshot exactly as they do
to the final artifact.

Every published event envelope names `run_id`, a monotonic committed sequence,
previous and new journal heads, resulting evidence root, record kind and
canonical payload hash. Followers accept only contiguous committed envelopes,
detect a gap by sequence or journal head, and reconnect from their last
accepted cursor. Transport EOF, observer detachment or loss of a live
connection is not run completion. Only a valid terminal seal ends the logical
stream.

Committing one transition follows `prepare -> durable install -> apply ->
publish`. Preparation is pure and binds a complete, canonically encoded replay
payload to the current cursor, writer-fence generation, proposed next frontier,
next evidence root and proposed journal head. The coordinator first installs
any referenced immutable evidence blobs, then the event itself, and only then
advances live in-memory state or exposes it to followers. A storage failure
cannot leave the authoritative in-memory cursor ahead of the durable journal.
Recovery decodes and reduces these complete payloads; a hash-only envelope is
not a resume artifact.

An invocation time cap, an orderly interrupt or an explicit pause first stops
new dispatch, then drains or revokes owned work and commits all accepted
evidence plus the exact open frontier. The append-only journal is already the
authoritative resume checkpoint. The current executable slice then tries once
to admit bounded snapshot work. When admitted, it renders and installs a
snapshot at the running cursor, appends
`SnapshotPublished(checkpoint_cursor, blob_digest)`, and finally appends
`Paused(reason, journal_head, evidence_root)`; the receipt exposes checkpoint,
publication and final paused cursors. If the admitted publisher instead reports
capacity, it installs the bounded cursor/progress-only receipt, appends
`SnapshotUnavailablePublished`, then pauses through the same two-record suffix.
That publication says only that this attempt was unavailable; it is not a
partial snapshot or a permanence claim. When the deadline has expired or
resource admission is unavailable, it appends `Paused` directly and returns
only the final paused cursor in `JournalOnlyCheckpoint`. It never consumes the
reserved host headroom merely to manufacture the view.

A hard kill may omit any uncommitted publication or pause suffix, but
resumption starts from the last fully committed journal head and evidence root
and treats uncommitted worker output as absent. Resume validates the journal and
immutable run identities, appends `Resumed(previous_journal_head)`, and
continues only the still-open frontier. It never reruns a closed singleton or
expands a proof-closed region merely to reconstruct progress.

A per-case evaluator step or collection limit is different from a time-slice
boundary. If one whole `CaseId` deterministically reaches a limit fixed by the
run's evaluator contract, reopening the unchanged run would only reach the
same limit again. The invocation stop therefore identifies that rank as still
open and reports it as blocked under the current contract; an admitted snapshot
reflects the same open evidence. Neither form MUST promise that an unchanged
resume will progress. Continuing requires a new compatible evaluator-budget
refinement protocol, or a new run whose different evaluator identity is
explicit.

Declared probes receive initial scheduling priority in this same stream. Their
observations become ordinary singleton evidence after exact evaluation, while
their candidate and lifting edges remain scheduling provenance. Reaching
`probe_plan_complete` closes only that finite scheduling obligation. The run
then proceeds directly into certificate refinement, solver closure and finite
residual evaluation unless the invocation explicitly requests a pause after
the probe milestone. Adaptive witness probes may recur later as the open
frontier changes; they remain hints until evaluation or proof closes support.

Seal construction avoids a self-referential hash. Let `J_e` be the last
accepted journal head, `E_e` its canonical evidence root, and
`A = render(E_e)` the canonical semantic result payload with run-history,
execution-metadata and seal envelopes omitted.
The terminal record commits its outcome kind, `J_e`, `E_e`, `hash(A)` and
completion or stop method; appending that record produces the distinct terminal
journal head `J_t`. The terminal document names `J_e`, `J_t`, `E_e` and
`hash(A)` and can therefore validate without containing its own hash. Final
answer equality depends on canonical payload/evidence, not the order-sensitive
journal heads.

`Completed` is the terminal-record kind permitted only when the required
frontier is empty and complete answer closure validates. Other terminal
outcomes use distinct seal kinds and cannot be mistaken for `Completed`. No
snapshot, discovered key, probe milestone or process exit may be described as
final before the appropriate seal validates. A sealed run accepts no further
semantic evidence. Only a sealed terminal report is eligible for downstream
`then` delivery; observing or pausing a live run never constructs a typed
terminal report and never invokes the continuation.

### Resource envelope and crash-safe progress

Parallelism is an operational scheduling choice, never part of query identity.
`jobs = N` is an upper bound on concurrently resident evaluators, not a promise
to keep `N` workers running. The engine MAY use fewer workers whenever its
resource envelope requires that headroom. The requested ceiling, observed peak
worker residency, effective worker high-water mark, orderly pressure stops and
checkpoint cadence belong in run metadata; none changes `program_hash`,
`query_hash`, `domain_hash`, `D`, `M`, `R` or any closed classification.

An automatic resource envelope MUST be conservative before it has measured one
representative worker. It begins with one evaluation worker, reserves at least
20 percent of installed processor capacity and at least 20 percent of physical
memory for the host. The memory reserve also has a 1-GiB absolute floor.
Consequently automatic admission budgets CPU and RAM independently beneath an
80-percent operational ceiling; spare capacity in one resource cannot
compensate for an over-budget value in the other. This is a scheduler and
reservation ceiling, not a kernel-enforced promise against momentary CPU or RSS
overshoot. Whole evaluator workers round the usable CPU budget down, so a
future six-core multi-worker scheduler admits at most four one-core evaluators.
After calibration it may raise concurrency one worker at a time, bounded by
both the processor reserve and a memory estimate that includes evaluator state,
thread stacks, artifact buffers and a safety margin. It stops launching new
work immediately when memory pressure rises or swap activity grows, and drains
back toward one worker before considering another increase. A user-supplied
worker count may lower or cap this policy but MUST NOT disable the memory
reserve in an automatic run.

Snapshot and case-DAG materialization are a separate admitted work subject,
not free work performed after the semantic permit ends. The current
single-worker publisher assigns that phase a fixed 256 MiB accounted
working-set envelope and admits it only through the same fresh telemetry,
normal-pressure and 80-percent CPU/RAM policy. It never spends the CPU reserve
or the memory reserve, which is at least 20 percent of physical RAM and never
less than 1 GiB. In the current conservative cold phase, the worker charge is
`max(2 GiB, ceil(total_memory / 4))`, so it dominates the 256 MiB view
envelope. Snapshot admission therefore receives one bounded opportunity at a
pause boundary; deadline expiry or denied authority produces a journal-only
checkpoint rather than borrowing the reserve. On resume, that pending view gets
the next separately admitted opportunity before semantic dispatch. A future
calibrated scan-mode or multi-worker publisher MUST introduce a distinct
materialized-view charge before admitting this phase; it cannot reuse the
current cold-charge dominance argument. An admitted attempt that reports
capacity publishes the fixed-size `snapshot_unavailable` observer receipt; it
does not become journal-only deferral and does not block later semantic work.

Automatic worker admission uses reservations, not process counts alone. Let
`W` be the calibrated per-worker memory charge, including a safety factor, and
let `R` be the required host-memory reserve. A live sample may compute a total
memory ceiling from `available_memory + resident_worker_rss - R`, because the
resident RSS is already absent from available memory; it MUST then charge `W`
for every target lease, including idle residents. Adding the resident count to
raw available-memory slots without that RSS reconciliation double-counts
capacity and is invalid. CPU accounting follows the same rule: a live total
ceiling may add measured resident-worker CPU back to idle capacity, then must
charge one evaluator core per target and preserve the host CPU reserve. Both
ceilings are capped by the live capacity sample. Missing aggregate worker RSS
or CPU attribution cannot justify scale-up while workers are resident.

For a normal, fresh sample this can be summarized as:

```text
N_memory = floor(max(0, available + owned_worker_rss - memory_reserve) / W)
N_cpu    = floor(max(0, idle_millicores + owned_worker_millicores
                        - cpu_reserve_millicores) / 1000)
N_live   = floor(max(0, live_capacity_millicores
                        - cpu_reserve_millicores) / 1000)
N_safe   = min(requested_ceiling, N_memory, N_cpu, N_live)
```

The equation computes a ceiling, not an immediate target: generations,
stability, acknowledgement and pressure rules below still govern movement
toward it. Missing or incoherent terms make `N_safe = 0` for automatic
admission. Every not-yet-stopped owned process remains a charged commitment,
including draining workers and granted-but-not-yet-resident leases. If those
commitments exceed `N_safe`, the coordinator stops new dispatch and drains to
zero; a promised shutdown is not reusable capacity.

Every target-lease change creates a new lease generation. A resident-worker
observation acknowledges only the generation and monotonic sample epoch it
names; an older zero-worker observation cannot finish a later drain, and a
partial acknowledgement cannot authorize another increase. Automatic
parallelism grows additively by one only after all target leases in the current
generation are observed resident and the complete stability window has passed.
Warning pressure reduces the target immediately; critical pressure, unknown
required telemetry, or a growing monotonic swap-out counter reduces it to zero.
Counter reset or sampler restart begins a new stability epoch and discards the
old shard/window evidence rather than treating the reset as zero growth.

Automatic admission requires known installed processor and physical-memory
capacity as well as live headroom; either half alone is insufficient. Before
granting another lease, the host sampler must show both that current idle CPU
capacity preserves the processor reserve after other workloads and that
current available memory remains above its reserve after charging the worker.
Unknown pressure or headroom grants no automatic lease; an implementation may
offer an explicit, recorded one-worker override on a platform without such
telemetry, but it is not `auto`. Known headroom below either reserve likewise
grants no new lease. A normal-pressure streak used for scale-up includes
elapsed CPU and memory stability as well as completed shards, so two very fast
checkpoints cannot stand in for the declared observation window.

Compiler/code-generation, solver construction and residual evaluation are
separate heavy phases. The default scheduler MUST NOT overlap a memory-heavy
compile with resident evaluation workers merely because processor capacity is
available. Implementations that prove a platform-specific joint memory bound
may opt into overlap explicitly and record that decision in run metadata.
Compile admission itself requires a fresh normal-pressure sample and reserves a
conservative compiler charge. Its generation begins only after the zero-worker
observation that completed the preceding drain. A worker observed resident in
that generation aborts compile admission. Ending compilation invalidates the
old evaluator memory calibration and requires a post-compile sample from a new
epoch before calibration or scanning can begin.

Evaluation workers SHOULD be isolated child processes with one semantic shard
at a time, bounded internal thread creation, coordinator-owned process groups,
and a lightweight pressure watchdog. Admission margins reduce risk but are not
hard containment: where the host offers reliable per-process memory or job
limits the coordinator SHOULD apply them as a second boundary. On platforms
without reliable containment, the watchdog stops dispatch at warning pressure
and terminates the owned worker group at critical pressure before accepting
more evidence.

The first single-worker implementation applies that process boundary to one
whole invocation slice. On macOS the parent owns the sampler and the child
alone owns the run-state writer fence. A private inherited pipe validates the
expected parent/process-group liveness shape and carries bounded monitor
heartbeats; it is not a cryptographic authentication boundary against a
hostile local launcher. Parent death, a stalled sampler or a missed heartbeat
makes the child terminate its own process group. The parent also holds an armed
group guard, checks both leader reaping and group disappearance, and never
reports `contained` after an unchecked signal attempt. It samples process-group
RSS and interval CPU plus host pressure with an early guard below the
80-percent operational upper bound. The fresh child additionally tracks live
Rust allocation requests from process start and installs that cap after the
liveness relationship is validated, while a reserved margin covers untracked
stacks, FFI, mappings and allocator overhead. This reduces crash risk and
prevents an orphaned unsupervised slice, but remains a circuit breaker rather
than a kernel-enforced RSS or instantaneous CPU quota. Descendants that
deliberately escape the owned process group are outside this first
implementation and evaluator subprocess workers therefore remain forbidden.
Full parallel execution moves the coordinator back to the parent and places
only bounded, replayable semantic shards in child groups.

Durable output has a separate storage envelope. Before a resumable run, the
coordinator checks free space on the selected run-state filesystem, reserves
room for both committed chunks and one streamed final-assembly generation, and
stops before crossing a fixed host-space reserve. Chunk-size observations may
tighten that estimate but never remove the reserve. Unknown free space, a full
filesystem, or an unbounded authorized record shape cannot justify an
automatic long run. Final assembly streams canonical chunks in order instead
of retaining the complete ledger in RAM.

Parallel work is divided into deterministic, disjoint canonical shards of the
remaining open frontier. Certified regions and structurally closed cases are
never expanded back into singleton work merely to fit the resumable journal;
the exact residual-support identity is part of the run-state contract. A shard
writes no authoritative evidence in place. The coordinator
validates a completed shard's exact support and identities, writes it as a new
owner-only sibling temporary file, flushes and syncs it, atomically renames it
to an immutable content-addressed chunk, then syncs the journal directory. A
lease likewise uses one small immutable attempt record before dispatch. The
durable journal is the validated set of contract-bound attempt and chunk
entries, not a full manifest rewritten after every transition. A periodic
compact index may accelerate resume but is disposable and cannot add coverage.
Re-hashing or rewriting the growing committed ledger for every shard is
forbidden: it would make million-case execution quadratic. A canonical
manifest and final artifact are streamed once from the validated disjoint
chunk set; complete publication requires a durable receipt for that final
manifest generation. A crash may therefore
discard at most currently uncommitted shards; it cannot create a false closed
region, duplicate case or hole hidden behind `complete`. Checkpoints that may
contain configurations or outputs use an explicit private run-state directory
outside the repository and never default to a temporary directory whose
contents disappear on reboot.

The unified run journal, including singleton work performed during probes, is
a trusted owner-local execution cache rather than an independently verifiable
proof. It is bound to the exact program, query, domain, probe plan, evaluator,
report request and artifact-schema identities; stale, malformed, conflicting,
overlapping or non-private state fails closed. This trust boundary permits
completed singleton work to survive a crash without replaying every case,
while published representatives are freshly replayed and region certificates
are independently checked. Deployments that cannot trust the local owner
boundary MUST discard or fully replay the journal before publication.

Resource pressure is an orderly operational stop when the coordinator can
commit the journal: the pause preserves every established layer status and the
exact open frontier. If the separately governed view phase is admitted, a
snapshot also records the resource-pressure stop; otherwise the receipt returns
`JournalOnlyCheckpoint` with resource-admission deferral. Answer closure is
normally `partial`, but may already be `complete` while a separate mechanism
frontier remains open. An external kill or machine failure emits no new
snapshot; the next invocation validates and resumes the last durable journal
head and evidence root.
Neither event is evidence about the unexplored complement.

### Non-normative theoretical basis

The case representation follows reduced ordered decision diagrams: fixed
variable order plus redundant-node elimination and isomorphic-subgraph sharing
gives a canonical logical representation for that order. Futuruna generalizes
the Boolean terminals and binary choices to typed finite domains and several
classification terminals. See Bryant,
[Graph-Based Algorithms for Boolean Function Manipulation](https://people.eecs.berkeley.edu/~russell/classes/cs289/f04/readings/Bryant%3A1986.pdf).

Margrave's policy analysis gives a particularly close domain precedent.  It
represents access-control decisions with multi-terminal BDDs, uses shared
subgraphs and eliminated variables to describe sets of concrete scenarios,
and constructs change-impact queries from paired decisions of two policy
versions.  A Futuruna boundary query is the corresponding self-comparison
under an input perturbation: it relates `P(x)` to `P(x + step)` inside one
resolved program.  See Fisler et al.,
[Verification and Change-Impact Analysis of Access-Control Policies](https://web.cs.wpi.edu/~kfisler/Pubs/icse05.pdf),
and the
[Margrave command reference](https://docs.racket-lang.org/margrave/).

Margrave also separates query construction from scenario presentation through
`SHOW ONE`, `SHOW NEXT` and `SHOW ALL`, and can ask which included rule atoms
are realized in satisfying scenarios.  That distinction supports Futuruna's
separate semantic closure, retained-example and display limits.  Realized rule
atoms are analogous to overlapping mechanism-atom supports, not to the
disjoint signature fibers defined here: Futuruna therefore keeps the exact
search decision DAG, semantic transition graph, replay-derived mechanism
signatures and their incidence as linked but distinct objects.  Margrave's
`CEILING` bounds scenario size; it is
not evidence that an unenumerated family is infinite, just as a Futuruna
example or tracing cap is never an infinity proof.

The mechanism representation draws on program dependence graphs and dynamic
slicing: static structure records possible data/control dependence, while a
replay-specific slice records what actually influenced a selected observation.
See Ferrante, Ottenstein and Warren,
[The Program Dependence Graph and Its Use in Optimization](https://bears.ece.ucsb.edu/class/ece253/papers/ferrante87.pdf),
and Agrawal and Horgan,
[Dynamic Program Slicing](https://www.cs.purdue.edu/homes/xyzhang/spring07/Papers/p246-agrawal.pdf).

The separation between one result and its alternative derivations is also
related to provenance semantics, where output identity and the symbolic ways
that contributed to it remain distinct. See Green, Karvounarakis and Tannen,
[Provenance Semirings](https://web.cs.ucdavis.edu/~green/papers/pods07.pdf).
These precedents motivate the data structures; Futuruna's exact semantics are
the invariants specified above.

## First-class Result Continuation

A typed output may bind its terminal report:

```runa
then report -> publish_support_report(report)
```

The binder is immutable and scoped only to the continuation expression. It is
not a top-level binding left behind after the command. Query-local bounds,
hidden assignments and replay interpreter state are not in scope.

The continuation evaluates in a fresh ordinary environment containing the
resolved program declarations and this one report binding. It does not inherit
mutable state from solving or from any representative replay.

The binder has type `ExplorationReport(Row)`. Its normative source-level shape
is:

```runa
# ExplorationIdentity(
    run_id: String,
    query: String,
    query_hash: String,
    program_hash: String
)

# ExplorationReport(row) =
    ExplorationComplete(
        identity: ExplorationIdentity,
        findings: List(row)
    )
  | ExplorationPartial(
        identity: ExplorationIdentity,
        confirmed: List(row),
        stop_reason: String
    )
  | ExplorationUnknown(
        identity: ExplorationIdentity,
        confirmed: List(row),
        reason: String
    )
  | ExplorationUnsupported(
        identity: ExplorationIdentity,
        diagnostic: String
    )
  | ExplorationError(
        identity: ExplorationIdentity,
        diagnostics: List(String)
    )
```

These are compiler-provided Experimental types; a model does not redeclare
them. The prefixed constructors avoid collision with the stream event
`Completed`.

Only `ExplorationComplete` exposes a list named `findings`.
`ExplorationPartial` and `ExplorationUnknown` expose only rows already replayed
and confirmed, under the deliberately different name `confirmed`.
`ExplorationUnsupported` and `ExplorationError` expose no row list and cannot
be mistaken for a complete result.

`ExplorationError` represents a terminal solving, decoding or replay error
after a query has type-checked. Parse and type errors occur before a report
exists and therefore cannot invoke `then`.

The continuation MUST type-check to `()`. It may call ordinary functions and
explicit effects. Automatic delivery is **at most once per `run_id`**, not an
impossible exactly-once guarantee across arbitrary external systems. After the
terminal report is durable, the coordinator records a separate durable
delivery claim before invoking `then` and a success receipt afterward. A
reopened sealed run with an existing claim does not invoke it again. A crash
between claim and receipt leaves `continuation_outcome_unknown`; it is not
retried automatically. An explicit operator retry carries the same `run_id` as
an idempotency key, and the continuation is responsible for making any
external effect idempotent. These delivery records are operational outbox
state, not new semantic evidence in the sealed exploration. A source or type
error prevents report construction and therefore prevents the continuation
from running.

Continuation analysis and execution are downstream of the search. The
continuation cannot contribute dependencies, bounds, constraints, solver
terms, output keys, representative objectives or replay behavior.

## Formal Semantics

Let:

- `U_D` be the canonical finite declared generator-coordinate space;
- `U_C ⊆ U_D` be the coordinates for which structural endpoint contracts can
  construct a total typed transition;
- `tau : U_C -> Transition`, with `tau(u) = (c(u), b(u), a(u))`, be the total
  transition-normalization map on that constructible subset;
- `U_T = image(tau)` be the distinct declared semantic transitions;
- `D_C ⊆ U_C` be the coordinates satisfying before, after and cross-edge
  validity;
- `D_T = image(tau restricted to D_C)` be the distinct admissible transitions;
- `P(tau(u))` be the Boolean question rule;
- `Q(tau(u))` be `not P(tau(u))` for `find violations`, otherwise
  `P(tau(u))`;
- `M_C = { u | u in D_C and Q(tau(u)) }` be the matching transition cases;
- `M_T = image(tau restricted to M_C)` be the distinct matching transitions;
- `CaseId(u)` be the injective query-local generator identity for every
  `u in U_D`;
- `StateId(b(u))` and `StateId(a(u))` be the role-neutral endpoint identities;
- `TransitionId(tau(u))` be the directional semantic-edge identity;
- `K(u)` be the output-key projection;
- `E_i(u)` be the optional integer extrema measures;
- `S(u)` be the shown-value projection;
- `O(u)` be the optional representative objective value;
- `R_raw = { K(u) | u in M_C }` be the raw matching key groups;
- `F_group(k)` be the declared post-aggregation group filter, which is always true
  without `having` and is `min E_i[W(k)] < max E_i[W(k)]` for
  `having varies(E_i)`;
- `R = { k | k in R_raw and F_group(k) }` be the emitted result keys;
- `h` be a fixed mechanism-observation specification;
- `S_req` be the requested mechanism target, either `Rep(R)` or `M_C`;
- `T_trace` be the actually traced subset of `S_req`;
- `Sigma_(q,h)(u)` be the deterministic canonical mechanism signature for
  every `u in T_trace`; and
- `Gamma_trace = { Sigma_(q,h)(u) | u in T_trace }` be the observed complete
  mechanism signatures for that traced scope.

For brevity, the remainder of the RFC may write `U`, `D` and `M` for `U_D`,
`D_C` and `M_C`; “configuration” means one declared generator coordinate, not a
state or graph node. `CaseId` is injective over `U_D`; `K` need not be. The
cardinalities therefore obey:

```text
|R| <= |R_raw| <= |M_C| <= |D_C| <= |U_C| <= |U_D|
|M_T| <= |D_T| <= |U_T|
|U_T| <= |U_C|, |D_T| <= |D_C|, |M_T| <= |M_C|
|Gamma_trace| <= |T_trace| <= |S_req|
```

`|D_C|` is the admissible-transition-case count, `|M_C|` is the matching-
transition-case count, `|D_T|` and `|M_T|` count distinct semantic edges,
`|R_raw|` is the raw group count and `|R|` is the emitted distinct-result-key
count. None may be substituted for another merely because they happen to agree
for one query.

For a named case-coordinate scope `X` contained in `U_C`:

```text
E_X = { TransitionId(tau(u)) | u in X }
V_X = { StateId(b(u)), StateId(a(u)) | u in X }
```

`|E_X|` is a distinct semantic-transition count, while `|X|` is its weighted
supporting-case count. `|E_X| <= |X|`, with equality under the default
one-to-one generator. State nodes, transition edges, result keys and mechanism
signatures are otherwise incomparable populations. A mechanism partition is a
quotient only on traced transition cases: `u ~_h v` exactly when
`Sigma_(q,h)(u) = Sigma_(q,h)(v)`.

The lossless matched value relation is:

```text
J = {
    (CaseId(u), TransitionId(tau(u)), StateId(b(u)), StateId(a(u)),
     K(u), S(u), O(u))
    | u in M_C
}
```

Mechanism evidence is the separately scoped relation:

```text
I_trace = {
    (CaseId(u), TransitionId(tau(u)), K(u), Sigma_(q,h)(u))
    | u in T_trace
}
```

Raw result groups are the image of `K`, while returned result keys additionally
satisfy `F_group`; observed mechanism classes are the image of
`Sigma_(q,h)` on `T_trace`.
Neither image determines the other. Their joint view is
the exact incidence `{(K(u), Sigma_(q,h)(u)) | u in T_trace}`, not
`R x Gamma_trace`. Only `S_req = M_C` and `T_trace = S_req` justify claims about
every matching mechanism.

Partial case evidence additionally conserves the declared space as:

```text
U_D = E_closed + N_closed + M_closed + O_eligibility + O_polarity
```

with disjoint union. `E_closed` contains both coordinates proved structurally
excluded in `U_D \ U_C` and constructible coordinates proved validity-excluded
in `U_C \ D_C`; `N_closed` is known in `D_C - M_C`, and `M_closed` is known in
`M_C`. Exact `D_C` requires `O_eligibility = empty`; exact `M_C` requires both
open sets to be empty.

For every returned key `k`, a representative is selected from:

```text
W(k) = { u | u in D_C, Q(tau(u)), and K(u) = k }
```

Representative closure for key `k` requires a selected `u_k in W(k)`, no
`v in W(k)` that is strictly better under the declared `first`, `maximize` or
`minimize` policy, and minimum canonical `CaseId(v)` among objective ties.
Projection closure alone does not establish this invariant.

When a report explicitly exposes the configuration ledger, its complete
payload is the canonically sorted sequence:

```text
[L(u) | u in M_C, ordered by CaseId(u)]
```

where `L(u)` contains the authorized case and transition identities, key,
shown values and replay confirmation for that exact transition case. `O(u)` remains internal
unless it aliases an authorized key/show field, as `loss` does in the example,
or a future report request explicitly authorizes objective publication.

For group projection `G(u)`, let:

```text
groups(D_C, G) = { G(u) | u in D_C }
D_g = { u | u in D_C and G(u) = g }
M_g = { u | u in M_C and G(u) = g }
R_raw,g = { K(u) | u in M_g }
R_g = { k | k in R_raw,g and F_group(k) }
```

An `any` group is satisfied when `M_g` is nonempty. An `all` group is satisfied
when `D_g` is nonempty and `M_g = D_g`. An exact partition leaf exposes counts
for `D_g`, `M_g`, `R_raw,g` and `R_g` without changing `D_C`, `M_C`, `R_raw`
or `R`.

When `output as Row` is present, let `RowOf(u)` be the declared row constructor
applied to the key fields followed by the shown fields of the selected replayed
representative. The typed complete payload is the canonically sorted list:

```text
[RowOf(u_k) | k in R]
```

The continuation observes this result; it cannot alter `U_D`, `U_C`, `D_C`,
`M_C`, any identity, `K`, `E`, `F_group`, `S`, `O`, `W`, `R_raw`, `R`, `h`,
`S_req`, `T_trace`, `Sigma_(q,h)`, any graph or any graph-derived view.

A complete exploration guarantees:

```text
k is returned
if and only if
there exists an admissible transition case u with K(u) = k and Q(tau(u)),
and the closed group satisfies F_group(k)
```

## Probe Scheduling inside the Run

Probes are a formal scheduling strategy within an exploration run, not a
separate low-confidence analysis and not a prerequisite artifact. A query with
a `probes` block normally gives that finite plan first priority immediately
after `RunOpened`; when the plan reaches its stopping condition, the same run
continues directly into region certification, solver refinement and residual
evaluation. `--pause-after probes` preserves an explicit human inspection
point without imposing a forced second invocation on every run.

| Run state at invocation entry | Required behavior |
|---|---|
| No run state | Create `RunOpened`, initialize the full exact frontier and begin the declared probe plan |
| Valid paused head with an incomplete probe plan | Append `Resumed`, continue the remaining probe schedule, then proceed unless explicitly paused |
| Valid paused head with a complete probe plan | Append `Resumed` and continue the exact open frontier without rerunning closed probe cases |
| Valid unsealed running head with a live writer lease | Refuse a second writer; observers may still attach read-only |
| Valid unsealed running head whose writer is provably gone and lease is fenced | Advance the lease generation, append `Recovered(previous_journal_head)`, and continue from the last committed evidence root/frontier |
| Valid sealed head | Expose the immutable terminal result; reject attempts to append semantic evidence |
| Stale, corrupt, identity-mismatched or conflicting state | Fail closed without overwriting the last valid head |

Only one coordinator may own the writer lease for a run. Recovery requires
both loss of the old storage lock and a new fenced lease generation; elapsed
time alone never proves that a writer is dead. Other processes may observe
committed events and snapshots but cannot dispatch work or advance the journal
head.

### Probe records

Probe records live in the main run journal and contain at least:

- the run identities and `probe_plan_hash` already bound by `RunOpened`;
- canonical ordered dimension descriptors `(role, role_field_index,
  bound_index)`, structural boundary-axis reference and step, declared selector
  order, semantic case cap and deterministic selector/tie-break version;
- `active` or `complete`, the current scheduling cursor, the number of distinct
  cases classified and the remaining probe-plan obligation;
- an ordered adaptive-decision transcript recording each scheduling reason,
  the pre-decision frontier identity, selected `CaseId`, observed
  classification and resulting next-frontier identity;
- every lifted scheduling edge, including its observed origin `CaseId`, fixed
  boundary-axis value, generated candidate `CaseId` and `evaluated` or
  `unevaluated` state; and
- one validated observation for every selected case: `CaseId`, only the
  authorized named configuration values, actual lower and upper endpoint
  values and eligibility states, classification, exact question and authorized
  output values, exclusion reason, scheduling reason, replay receipt and any
  separately authorized mechanism trace.

Both matches and nonmatches are retained; boundary-ineligible or constraint-
failing selections are retained as exclusions. Omitting any class would make
the adaptive transcript irreproducible and could bias later scheduling.
Endpoint values are the actual values considered for that case. An excluded
endpoint records its value and ineligibility rather than pretending that the
model ran there; unavailable question and output fields are explicitly absent.

`domain_hash` identifies the normalized independent domains, fixed and derived
facts, constraints, canonical `CaseId` dimension descriptors and order,
structural boundary-axis reference, step and endpoint-eligibility rule. Axis
names are retained only as presentation labels. `probe_plan_hash` identifies the ordered selectors,
lift operations, deterministic adaptive rules, semantic stopping cap,
authorized field allow-lists and mechanism-trace choice. Run-state path, jobs,
time limit, checkpoint cadence and observation mode are not hashed.

`probe_plan_complete` means exactly that every finite selector and generated
lift candidate is exhausted or the unique-classification count reached
`at_most`. Reaching `at_most 256 cases` may therefore complete the probe plan
while millions of cases remain open. It establishes neither final solver
`UNSAT`, BoundaryPlan coverage, admissibility or polarity closure, an exact
global count, a complete candidate set nor mechanism-incidence closure.

### Checkpointing, privacy and trust

An exactly evaluated probe observation enters the same singleton case evidence
as any later evaluation. Its scheduling reason or source-event label remains a
priority hint. An unevaluated lifted record enters only the candidate queue and
origin graph. No classification, output, loss or mechanism evidence crosses a
lifting edge, and neither candidate exhaustion nor probe-plan completion closes
the complement. Certified regions enter only through their independent
coverage, disjointness and certificate validation.

Ordinary pause/resume trusts committed probe observations under the same
owner-local, hash-bound run-journal boundary as other singleton chunks; it does
not duplicate every evaluation on each invocation. Resume validates the entire
manifest chain, immutable identities and frontier conservation. Public
representatives and extrema witnesses still receive their required fresh
replay before exposure. A future import of probe evidence from another run or
trust boundary MUST freshly replay every imported observation before it can
seed this run.

The `retain` clauses authorize disclosure in journal events and snapshots, not
only final presentation. Fields outside those allow-lists stay absent, and
mechanism traces require their own opt-in. The engine never invents a private
run-state path, writes sensitive observations into the repository or build
cache, or emits an unrestricted journal to stdout. New state SHOULD use
owner-only permissions where supported.

### Probe milestone and snapshots

Probe-plan progress, semantic-answer closure and stream lifecycle remain
separate fields. When snapshot materialization is admitted, its payload
describes the pre-publication `running` cursor and the surrounding invocation
receipt carries the final `paused` cursor:

| Snapshot phase | Probe milestone | Answer snapshot | Meaning |
|---|---|---|---|
| `probes` | active | `partial` | Probe scheduling is selecting cases from the exact open frontier |
| `case_search` | complete | `partial` | The probe milestone passed and exact closure continues |
| `finalization` | complete | `partial` | Case classification closed but required terminal replay remains open |
| `complete` | complete | `complete` | Semantic answer frontiers are closed; publication/sealing may still be pending |

Only the terminal document and its valid seal make the stream lifecycle
`sealed`. Thus a checkpoint whose phase is `complete` is not itself a terminal
artifact.

The positive `at_most` count is part of `ProbePlan`; reaching it is a normal
milestone, not an answer result. A time, work, storage or explicit
`--pause-after probes` limit commits the journal frontier and exits with the
partial-result code while required frontier remains. It attempts the optional
snapshot phase only under a separate resource permit. It never emits an
answer-not-started phase: validated probe classifications already belong to the
same case relation being explored, whose current answer evidence is `partial`.

When view admission succeeds, human output at an inspection pause includes all
three durable cursors:

```text
Run: PAUSED
Run state: /private/path/income-cliffs.run
Stop: probe milestone reached
Final cursor: #42 <paused-journal-head> (paused)
Probe milestone: COMPLETE
Singleton cases evaluated this slice: <n>
Total cases closed this slice: <n>

Artifact blob: <snapshot-sha256>
Checkpoint cursor: #40 <running-journal-head>
Publication cursor: #41 <publication-journal-head>
Canonical observable checkpoint:
{...}
```

If the invocation deadline has expired or the view phase is not admitted, the
journal pause remains complete and human output instead says:

```text
Run: PAUSED
Run state: /private/path/income-cliffs.run
Stop: <typed stop>
Final cursor: #<n> <paused-journal-head> (paused)
Probe milestone: COMPLETE | INCOMPLETE

Artifact: journal-only checkpoint
Observable snapshot: deferred (<time-limit or resource-admission reason>)
```

There is then no snapshot blob, checkpoint cursor or publication cursor.

If the view phase was admitted but the bounded publisher reported capacity,
human output instead names an observable snapshot as `unavailable` and prints
the canonical `futuruna.explore.snapshot-unavailable.v1` receipt. The receipt
has its own blob, checkpoint cursor and `SnapshotUnavailablePublished` cursor;
it contains no configuration, answer, or graph prefix.

A machine observer currently receives a
`futuruna.explore.invocation.v1` receipt; the invocation schema remains v1 for
all pause artifact forms. An admitted full view embeds the exact content-addressed
`futuruna.explore.snapshot.v6` document plus three distinct cursors: the running
cursor described by the checkpoint, the following `SnapshotPublished` cursor,
and the final paused cursor. This two-record suffix avoids a circular hash while
proving that the returned bytes were durably named before pause.

A deferred view instead returns typed `JournalOnlyCheckpoint`; JSON uses
`artifact.kind = "journal_checkpoint"`, `snapshot.status = "deferred"`, and a
reason kind of `time_limit` or `resource_admission`. It has no `blob_digest`,
`canonical_payload`, checkpoint cursor or publication cursor. The final paused
cursor is sufficient for exact resume. Deferral is operational metadata only:
it does not add evidence, change the evidence root or run identity, or mean that
an authorized search decision DAG exceeded a capacity limit. Invocation-local stop
details stay in the receipt; an embedded snapshot keeps `invocation_stop` and
`pause_reason` null. Pausing never constructs `ExplorationReport(Row)` and never
executes `then`.

An admitted capacity outcome is not deferred. Invocation-v1 uses artifact kind
`snapshot_unavailable`, embeds the separately framed canonical receipt, and
names its blob, checkpoint and publication cursors. Its reason kind is
`capacity`; diagnostic detail remains in the invocation envelope rather than
the canonical receipt. This publication leaves the evidence root unchanged and
does not assert that a future attempt can never fit.

The next invocation resumed from that journal-only suffix attempts the pending
view before further search. It pauses again with stop kind `snapshot_catch_up`:
with the materialized checkpoint, a bounded `snapshot_unavailable` receipt if
the admitted publisher reports capacity, or honestly with another
`journal_checkpoint` if its own deadline or current resource admission still
cannot admit the view. This catch-up does not evaluate a CaseId.

Once classification closes, ordinary slicing pauses at
`classification_closed_finalization_pending`. Explicit `--finalize` admits one
atomic-v1 replay/publication unit. It either publishes and seals
`futuruna.explore.exact-answer.v5`, or commits another pause with typed
`FinalizationLimit` details (`finalization_limit` in JSON) when the witness set,
complete raw-group preflight, replay manifest, requested search decision DAG, or single
JSON blob does not fit. That pause may carry an admitted snapshot or a
journal-only checkpoint under the same view-admission rule. If the immutable
request includes the search decision DAG, the finalizer refuses to seal unless the graph
is included and both its admissibility and polarity closures are closed.
Repeating that unchanged v1 invocation reaches the same capability limit; it
does not make chunked progress. A time limit is a work-boundary soft deadline
inside the library API; the CLI supervisor may interrupt an atomic unit and
replay safely from the last committed event. Resumable inner batches and
chunked terminal blobs remain future protocol work.

Atomic-v1 currently requires the full raw-group snapshot to fit the v6 bounded
group/value envelope (256 groups, 16,384 recursive value nodes and 4 MiB of
semantic value payload), permits at most 65,536 selected replay witnesses, caps
retained replay bodies at 32 MiB, caps rendered row JSON at 48 MiB, and caps the
complete terminal JSON blob at 64 MiB. These limits constrain this finalizer
implementation, not the exact case cardinality proved by the run.
Projection labels are independently bounded to 65,536 labels per projection
kind and 1 MiB per label, with a cumulative 4 MiB UTF-8 byte cap across key,
extrema and shown labels. The cumulative cap is part of the snapshot and
terminal schema identity. Before a run is created, all checked presentation
strings copied into either document—query and axis names, named domain sources,
fact and boundary names, plus projection/having labels—must also fit an exact
8 MiB cumulative canonical-JSON string budget and 262,144 total occurrences.
The occurrence cap bounds retained per-entry metadata even for many tiny or
repeated names. Repeated serialized occurrences are charged repeatedly; both
caps are bound into the snapshot and terminal serialization-schema identities.
That presentation binding does not promote an axis name into structural axis
identity; `(role, role_field_index, bound_index)` remains authoritative.

Case-graph materialization has its own fixed all-or-nothing envelope: at most
256 axes, 65,536 uniform rank runs, 131,072 DAG nodes, 262,144 arcs, 262,144
ordinal intervals and 64 MiB of conservative lowerer-accounted work, followed
by an 8 MiB limit for the canonical nested
`futuruna.explore.case-graph.v1`
JSON object. An admitted pause snapshot reports the first exceeded graph
resource with its fixed `maximum` and an honest `required_at_least`; these are
publication limits, not case-space bounds or fabricated totals. A
graph-requested terminal reaches `FinalizationLimit` instead of sealing when
any one is exceeded.

## Enumeration, Graph Construction and Replay

The exact finite backend is the reference implementation for finite `D`. It
streams canonical cases, evaluates admissibility and polarity, records closed
case evidence and ends with exact finite exhaustion. A solver backend may
instead produce exact regions, projected keys or cases and close them with
final `UNSAT` or exact counting. Both feed the same backend-neutral evidence
and graph builders.

Key blocking establishes `R_raw`, not `|M|`; closed extrema aggregation and the
group filter establish emitted `R`. Exact `D` and `M`, a full search decision DAG,
configuration coverage or an exposed ledger require canonical case closure by
finite exhaustion, exact regions, model counting or an equivalent exact
method. Neither `|R|` nor `|R_raw|` can stand in for `|M|`.

Representatives are finalized only after the policy optimum and canonical
tie-break are closed for each exposed key. `first` means the least canonical
`CaseId`, not the first solver model or enumeration discovery. Objective ties
use the same order.

Every public row then replays through a fresh ordinary interpreter. The replay
MUST confirm question polarity, key, shown values and objective. A row is
eligible for `results` or a typed `confirmed` payload only after representative
selection is closed for that key: canonical `first` is proven least, or the
`maximize`/`minimize` optimum and canonical tie-break are proven final.
When grouped extrema are exposed, canonical minimum and maximum witness cases
for every measure MUST also replay and confirm their key and endpoint value;
the representative alone cannot certify another endpoint.
Discovered keys with provisional witnesses may be reported separately as lower
bounds, but their changing shown values are not canonical result rows. Mechanism
tracing, when requested and supported, uses this same fresh replay layer for
both exact and solver-produced cases; enumeration internals and solver branch
terms never become public provenance. A replay disagreement is an internal
correctness error, not a partial success.

The case decision graph is reduced only from canonical closed/open evidence,
never incrementally in backend discovery order. Mechanism signatures are
interned only after canonical replay. This keeps equivalent exact and solver
runs observationally identical apart from completion method and performance.

For a supported boundary relation, the preferred exact refinement order is:

1. normalize the reachable value and validity slice to a guarded semilinear
   form;
2. construct the branch/event partition and its mechanism-labelled candidate
   supports;
3. certify whole interval/congruence cells from exact finite-difference bounds;
4. ask SMT about only the residual cells and use each model or failed proof to
   refine the partition (CEGAR);
5. enumerate singleton lower endpoints only in the remaining finite residual.

This order is an optimization, not a change in semantics. Every closed region
enters the same case evidence as exhaustive enumeration would, every residual
remains explicit, and the singleton fallback supplies a completeness path for
finite pure total expressions that the symbolic accelerator cannot compress.
The fallback may be expensive, but it prevents an incomplete event extractor
from becoming an unsound candidate filter.

A symbolic implementation may alternate safely:

1. choose one canonical witness from the uncovered region;
2. replay it and obtain its classification and optional mechanism signature;
3. prove an exact region with the same case classification;
4. subtract that region from the open frontier; and
5. repeat until the frontier is empty or the run stops honestly.

In version one, the witness signature covers only the freshly replayed witness;
`matching_closed` requires fresh replay of every member of `M`. A future
proof-backed mechanism mode may assign one signature to an exact homogeneous
region, but it must record a separate signature-invariance proof kind and
witness rather than presenting extrapolation as replay. Mechanism-first
candidate extraction may choose search order, but it may not define the
universe or claim `matching_closed` without evidence covering all of `M`.

At a committed cursor, exposed snapshot rows and evidence may be canonicalized
for observation without sealing the run. After execution reaches a terminal
sealed state, Futuruna finalizes the canonical report, constructs
`ExplorationReport(Row)` and invokes the selected continuation. Only
`complete` requires every requested answer/case/value layer to be closed.
Terminal partial and unknown reports contain only selection-closed confirmed
rows, and mechanism evidence may independently be closed, open or unavailable.
A routine time slice, resource pause or orderly interrupt commits a resumable
journal checkpoint, not `ExplorationPartial`, and does not invoke `then`. It
emits snapshot v6 only when the separate materialized-view phase is admitted;
otherwise the invocation reports a journal-only checkpoint.

## Completion Status

The answer projection at a snapshot or terminal report uses one of five
statuses:

| Status | Meaning |
|---|---|
| `complete` | Every answer/case/value population required by the selected report contract is closed |
| `partial` | A limit or interruption stopped required answer/case/value closure; any emitted findings are confirmed |
| `unknown` | The solver could not decide the remaining query |
| `unsupported` | Exact lowering and exhaustive fallback were unavailable for a required answer/case/value path |
| `error` | Validation failed before a report, or solving/replay disagreed with execution |

Only a sealed terminal report has a typed variant. An explicitly abandoned or
otherwise terminal partial run may be sealed as `ExplorationPartial`; an
ordinary resumable pause may not. The variants preserve the same distinction:

| Status | Typed report payload |
|---|---|
| `complete` | `ExplorationComplete(..., findings)` |
| `partial` | `ExplorationPartial(..., confirmed, stop_reason)` |
| `unknown` | `ExplorationUnknown(..., confirmed, reason)` |
| `unsupported` | `ExplorationUnsupported(..., diagnostic)` |
| `error` | `ExplorationError(..., diagnostics)` |

CLI `error` covers two phases. Parse, type and query-validation failures happen
before a typed report exists, so they emit diagnostics but do not invoke
`then`. `ExplorationError` represents only a terminal post-typecheck solving,
decoding or replay error for which Futuruna can construct the typed report.

Closure is recorded per semantic layer:

| Layer | Closed when |
|---|---|
| `projection` | Every result key in `R` is known |
| `admissibility` | No `eligibility_open` region remains, so `D` is exact |
| `polarity` | Admissibility is closed and no `admissible_open` region remains, so `M` is exact |
| `representatives` | Every exposed key has a policy-optimal case and canonical tie-break proof |
| `rows` | Every publicly exposed row has replayed and serialized |
| `views` | Every requested ledger, coverage, partition or histogram population is derived from closed evidence |

A projected-key report may close `projection` without claiming exact `|D|` or
`|M|`. A report exposing a full search decision DAG, exact case counts, configuration
ledger or case-population histogram also requires `admissibility` and
`polarity`. Mechanism status is recorded separately as `unavailable`,
`scope_open`, `incidence_open`, `representatives_closed` or
`matching_closed`; it is not aggregated into the exploration status. An exact
case report may therefore truthfully say that mechanism evidence is
representative-only or unavailable.

`complete` requires all of:

1. every relevant input is fixed, derived or proven finite;
2. every operation reachable from required answer, case and value roots has
   exact supported semantics;
3. all validity constraints are applied at every required endpoint;
4. every answer, case and value population required by the selected report is
   closed;
5. every emitted representative replays successfully;
6. every exposed extrema endpoint witness replays successfully;
7. every exposed configuration replays successfully;
8. closure comes from final `UNSAT`, exact finite exhaustion, exact region
   coverage or another recorded exact method; and
9. no timeout, search-result cap, resource limit or solver `unknown` prevented
   a required layer from closing.

A partial or unknown zero-result search MUST NOT say that no case exists. It
says only that no case has been confirmed so far.

Open case regions retain their individual reason categories. For the one CLI
status, `error` overrides every other outcome; otherwise a required open region
selects `unsupported` before `unknown` before `partial`. `complete` requires no
open region in any answer/case/value layer required by the `ReportRequest`.
Mechanism status remains orthogonal to this aggregation.

A human row-display or per-mechanism example limit is not a search-result cap.
Truncating presentation while preserving the complete canonical artifact
leaves status `complete`; stopping enumeration through `--max-results`
produces `partial` unless closure was independently established.

## Fail-closed Requirements

A complete result is unavailable when a path required for the answer, case
classification or requested value relation contains unsupported:

- recursion;
- effects, I/O, persistence, actors, streams, time or randomness;
- higher-order values or calls;
- partial non-Boolean rule dispatch;
- unbounded or cyclic domain construction;
- unsupported arithmetic or collections;
- model decoding;
- solver absence or solver `unknown` when no exact finite or other exact
  backend closes the required layers.

The tool reports the first source-linked boundary and the remaining supported
coverage. It MUST NOT introduce uninterpreted functions, arbitrary defaults or
approximate arithmetic and then claim completeness.

An unsupported mechanism-only trace or endpoint pairing instead makes
`mechanism_evidence` unavailable; it does not downgrade a closed answer.

## CLI Contract

Exploration uses a dedicated analysis command. A capped non-resumable
invocation accepts `--case-limit`. The current macOS-supervised durable
invocation accepts `--run-state`, `--time-limit`/`--max-minutes`,
`--pause-after probes`, explicit `--case-graph full`, explicit `--finalize`, and
`--json`. The first count-only mechanism experiments additionally accept an
all-or-none `nested-if-v1` or `rule-dispatch-v1` profile and two zero-based
`output.show` indexes:

```bash
runa explore model.runa
runa explore model.runa --query income_cliffs
runa explore model.runa --query income_cliffs --case-limit 100000
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --time-limit 10m
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --pause-after probes --json
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --time-limit 10m --case-graph full --json
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --time-limit 10m --finalize --json
runa explore examples/danish-income-tax/mechanism-stream-smoke.runa --query nested_mechanism_stream_smoke --run-state /private/path/nested-if-smoke.run --pause-after probes --mechanism-profile nested-if-v1 --mechanism-before-show 0 --mechanism-after-show 1 --json
runa explore examples/danish-income-tax/mechanism-rule-dispatch-smoke.runa --query rule_dispatch_mechanism_stream_smoke --run-state /private/path/rule-dispatch-smoke.run --time-limit 10s --mechanism-profile rule-dispatch-v1 --mechanism-before-show 0 --mechanism-after-show 1 --json
```

The accepted protocol also reserves the following future surfaces. They are
not commands implemented by the current slice:

```bash
runa explore model.runa --query income_cliffs --json --output result.json
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --follow
runa explore follow --run-state /private/path/income-cliffs.run --jsonl
runa explore model.runa --query income_cliffs --max-results 100
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --jobs auto
runa explore model.runa --query income_cliffs --run-state /private/path/income-cliffs.run --jobs 2
```

- `runa check` validates exploration declarations.
- `runa fmt` formats them.
- `runa run`, `build` and ordinary `verify` do not launch them.
- Multiple named declarations require `--query`.
- A sole named or anonymous root declaration may run without `--query`.
- Imported declarations are not selected implicitly.
- `--run-state PATH` names the private owner-local resumable search directory;
  it is distinct from the final `--output` artifact. The engine never invents
  this potentially sensitive durable path. Current durable supervision is
  macOS-only. Other platforms fail before launching the durable child rather
  than running without containment. The future authored `probes` syntax and
  parallel runs also require a run-state path.
- Repeating the same command with the same `--run-state` validates the journal
  and resumes its exact open frontier. A changed immutable identity is an error,
  not an implicit new run or probe refresh.
- `--mechanism-profile nested-if-v1|rule-dispatch-v1`,
  `--mechanism-before-show INDEX`, and
  `--mechanism-after-show INDEX` are an all-or-none experimental selector. The
  indexes must be distinct zero-based shown-field positions. This profile
  requires `--run-state`, binds its checked mechanism request into sequence-zero
  identity, drains confirmed mechanism replay before classifying another case,
  and publishes count-only checkpoints. `rule-dispatch-v1` requires both roots
  to call the same global family directly and records the ordinary dispatcher's
  reached `HeadMismatch` / `GuardFalse` / `BodyFalse` / `Applicable` candidates
  plus its terminal selection. It currently rejects `--case-graph
  full` and `--finalize`; signature definitions, incidence DAGs and terminal
  mechanism publication remain private or unavailable. Fully closed mechanism evidence
  therefore pauses with `mechanism_observation_closed_terminal_unavailable`
  and exit `2`; unchanged resume cannot seal or advance.
- `--case-graph full` is available only with `--run-state` and explicitly
  authorizes disclosure and retention of the complete current case
  classification graph. Full versus omitted is bound through the report
  request, retention authorization and schema contracts at run creation; it
  cannot be toggled on resume. Without the option, graph status is
  `not_requested` and no search decision DAG is published.
- `--time-limit DURATION` (with current whole-minute alias `--max-minutes`)
  caps one active invocation slice. The library orderly-pauses at a work
  boundary when it reaches the deadline; the outer supervisor may contain a
  child that does not exit within its grace interval, preserving the last
  committed cursor. A time limit does not alter query identity or seal a
  partial answer. If it leaves no time to admit the optional snapshot phase,
  the invocation returns a journal-only checkpoint. In atomic-v1 finalization
  it remains a work-boundary soft deadline whose uncommitted suffix may be
  retried safely.
- `--pause-after probes` asks for the inspection point explicitly. Without it,
  reaching the probe milestone falls through into ordinary exact refinement in
  the same run.
- `--finalize` opts into bounded atomic-v1 finalization once classification
  closes. It requires `--run-state` plus `--time-limit`/`--max-minutes` and
  cannot be combined with `--pause-after probes`. It either seals the bounded
  answer or returns typed `finalization_limit` details with another journal
  checkpoint and, when separately admitted, snapshot v6.
  When `--case-graph full` belongs to the run identity, the requested graph
  must be included and closed; capacity-limited graph materialization refuses
  the seal rather than silently dropping the view.
  Repeating the unchanged v1 invocation reaches the same capability limit;
  productive chunked continuation remains future protocol work.
- On a durable invocation, current `--json` emits one
  `futuruna.explore.invocation.v1` receipt. It raw-embeds an admitted canonical
  snapshot, bounded snapshot-unavailable receipt, or terminal answer; a
  journal-only pause instead has artifact kind `journal_checkpoint`,
  `snapshot.status = "deferred"`, and no canonical payload or blob. The
  admitted capacity receipt has distinct kind `snapshot_unavailable`, status
  `unavailable`, reason kind `capacity`, and its own content-addressed payload.
  A mechanism-profile receipt adds `execution_profile`, uses artifact kind
  `mechanism_checkpoint` or `mechanism_checkpoint_unavailable`, and embeds the
  count-only mechanism schema. Its journal-only form uses
  `mechanism_checkpoint.status = "deferred"` rather than calling that view a
  snapshot.
  It is not JSONL. Non-resumable and plan JSON remain unavailable.
- Future `--follow`/`--jsonl` surfaces will expose committed deltas without
  changing ordinary `--json` from one invocation receipt into a stream.
- Future `--jobs auto` and `--jobs N` use the resource envelope as a ceiling,
  not a demand. More than one worker requires `--run-state`; the current
  executable durable slice remains single-worker.
- Without `--run-state`, a query without probes is a one-worker,
  non-resumable, human-only invocation of the same canonical transition
  evaluator. It cannot provide
  pause/resume, follower reconnect, crash recovery or the invocation-v1 JSON
  receipt.
- A completed non-resumable search exits successfully. A durable search exits
  successfully only after a valid `Completed` seal, whether coverage is
  `empty`, `none`, `some` or `all`.
- A resource limit preventing required answer/case/value closure produces a
  nonzero partial result; a mechanism-only limit changes only mechanism status.
- Invalid, unsupported, unknown and replay-error outcomes are distinguishable
  in both the report or diagnostic and the process result.

Finding a violation is the command's purpose, not a process failure.

The process exit contract is:

| Exit | Command outcome |
|---:|---|
| `0` | A completed non-resumable result, or a valid durable `Completed` seal, regardless of finding count or coverage outcome |
| `1` | Invalid invocation, parse/type/query validation failure, stale/corrupt/conflicting run state, terminal `error`, writer-lease conflict or artifact-write failure |
| `2` | A durable nonterminal checkpoint, including a typed stop that may require changed resources, evaluator identity or a future finalizer; or an explicitly sealed terminal `partial` report |
| `3` | A terminal sealed `unknown` report |
| `4` | A terminal sealed `unsupported` report |
| `5` | The canonical exploration report was finalized, but its `then` continuation failed |

A complete search with zero matches deliberately shares exit `0` with a
complete search having matches. `empty` versus `none`, and zero versus nonzero
findings, are semantic results carried by the human or JSON report rather than
shell failures. This avoids treating a successfully proved absence or a found
counterexample as an operational error. Automation that branches on result
cardinality MUST read the versioned report. Exit `2` is an honest committed
checkpoint, not evidence that the remaining frontier is empty or that an
unchanged retry can progress. Automation MUST read the typed `stop` plus the
artifact kind and final cursor before deciding whether and how to resume. A
materialized checkpoint additionally supplies its checkpoint and publication
cursors; a journal-only checkpoint deliberately does not.

When a sealed typed report exists, nonzero terminal statuses still emit or
preserve that canonical report. A nonterminal exit `2` emits either an admitted
snapshot or a journal-only checkpoint. Parse, type and query-validation
failures happen before either exists. Exit `5` takes precedence as the command
outcome when a continuation fails, while the artifact retains its original
exploration status and counts.

The specified continuation is analysis-only:

- `runa check` type-checks it and `runa fmt` formats it, but neither executes
  it;
- `runa run`, `build` and ordinary `verify` never execute it or launch its
  exploration;
- only the explicitly selected root query may execute its continuation;
- imported and unselected query continuations never execute;
- attempting to use the `then` binder outside its continuation is an
  analysis-scope diagnostic.

The canonical human or JSON report is finalized before the continuation runs.
An `--output` result artifact is written before post-processing. If the
continuation fails, the command exits nonzero with a distinct continuation
diagnostic, but the exploration status, hashes and already written canonical
artifact remain unchanged. `--run-state` is never an alias for `--output`: the
former is the private append-only execution journal, while the latter is
written only from a sealed terminal report.

A continuation failure is not an `ExplorationError` variant: that report
already existed before the continuation started. The failure is a separate
command outcome layered on the preserved report.

When JSON uses stdout, stdout is reserved for the single versioned JSON receipt
or future terminal document. Console output from the continuation is isolated
to stderr, so post-processing cannot corrupt the JSON transport.

## Structured Result Contract

On the current durable path, `--json` emits one versioned
`futuruna.explore.invocation.v1` document. Its `stop`, `final_cursor`, and
per-slice counters are operational receipt data. Its exact profile has four
artifact forms. An admitted pause uses kind `checkpoint`, raw-embeds
`futuruna.explore.snapshot.v6`, and supplies the blob digest, byte framing,
checkpoint cursor and publication cursor. A denied or deadline-exhausted view
uses kind `journal_checkpoint`, contains `snapshot.status = "deferred"` and its
operational reason, and has no canonical payload or blob. An admitted publisher
that reports capacity uses kind `snapshot_unavailable`, raw-embeds the bounded
`futuruna.explore.snapshot-unavailable.v1` receipt, and supplies the same three
cursors as a full snapshot publication. A sealed receipt uses kind
`terminal_result` and raw-embeds the current experimental
`futuruna.explore.exact-answer.v5` semantic answer. The invocation schema stays
`futuruna.explore.invocation.v1` for all four forms. The nested-`if` mechanism
profile adds an `execution_profile` object containing its two selected show
indexes. Its admitted count view uses kind `mechanism_checkpoint` and embeds
`futuruna.explore.mechanism-checkpoint.v1`; bounded rendering capacity uses
`mechanism_checkpoint_unavailable`. A journal-only mechanism pause keeps kind
`journal_checkpoint`, has `mechanism_checkpoint.status = "deferred"`, and does
not carry a canonical payload. Non-resumable and plan JSON are not
implemented. JSONL following and the expanded public
`futuruna.explore.v1` terminal report below remain specified future surfaces.

A journal-only artifact has this shape inside the invocation receipt:

```json
{
  "kind": "journal_checkpoint",
  "snapshot": {
    "status": "deferred",
    "reason": {"kind": "time_limit"}
  }
}
```

`reason.kind` may instead be `resource_admission` with a typed detail. This
deferral says only that the materialized view was not admitted. It is not an
open-evidence claim, a `graph.case_graph.status = "capacity_limited"` result,
or an identity change; the receipt's final paused cursor names the
authoritative resume state.

An admitted capacity receipt has this artifact shape:

```json
{
  "kind": "snapshot_unavailable",
  "snapshot": {
    "status": "unavailable",
    "reason": {"kind": "capacity", "detail": "<invocation diagnostic>"}
  },
  "blob_digest": "<sha256>",
  "canonical_byte_framing": "json_line_lf",
  "checkpoint_cursor": {"sequence": 40},
  "publication_cursor": {"sequence": 41},
  "canonical_payload": {
    "schema": "futuruna.explore.snapshot-unavailable.v1",
    "snapshot": {"status": "unavailable", "reason": {"kind": "capacity"}}
  }
}
```

The canonical payload omits the diagnostic, configuration, answer and graphs;
it is a replay-verified operational receipt for that cursor, not a partial
snapshot or a claim about later attempts.

Every snapshot or terminal exploration document contains the identities and
closure/count evidence available to that artifact. Hypotheses never appear
under confirmed results and never contribute to an evidence-backed lower bound
before validation. Optional case-level sections appear only when authorized.
Only a terminal seal can make a completed terminal claim.

The executable v6 pause snapshot is deliberately a bounded observation, not a
full terminal artifact. Configuration values share one identity-bound node and
semantic-byte budget. Results use a canonical raw-key prefix with independent
group, recursive-value, semantic-byte and rendered-JSON caps. The document
reports observed versus scanned raw groups, truncation and exact versus
lower-bound count status, so preview limits never masquerade as semantic case
limits. Its `graph.case_graph` envelope is `not_requested`, `included`, or
`capacity_limited`. This implemented field contains the search decision DAG;
it does not contain semantic state/edge serialization. An included graph is a
complete total current-evidence DAG;
capacity evidence names the fixed resource, `maximum`, and
`required_at_least`, with both the graph object and graph hash absent. Until
general mechanism replay is wired into the public snapshot-v6 contract,
`mechanism_evidence.status` is `unavailable_deferred` rather than an inferred
count.

Snapshot-v6 configuration dimensions already serialize `bound_index`, `role`
and `role_field_index` beside `name`; fixed and derived facts carry the same
structural ownership fields. Its nested case DAG refers only to
`dimension_index`. Consumers use those indices and descriptors for identity;
the names are presentation labels. Exact-answer-v5 does not repeat this
configuration object. It commits the checked program, query, domain, report,
disclosure policy and complete case universe indirectly through
`answer_scope_hash`.

An embedded materialized checkpoint is the exact pre-publication JSON-line
blob, including one trailing LF in storage. The invocation envelope raw-embeds
its JSON object and declares `canonical_byte_framing: "json_line_lf"`;
consumers append that single LF when reproducing `blob_digest`. A journal-only
checkpoint has neither field. Terminal answer framing is `json_document`.
Decimal counts are encoded losslessly and are never routed through a
floating-point JSON value representation.

The following is an expanded, explicitly authorized target-v1 example. It is
not the snapshot-v6 or exact-answer-v5 shape emitted by the current runtime:

```json
{
  "schema": "futuruna.explore.v1",
  "schema_version": 1,
  "run": {
    "run_id": "...",
    "lifecycle": "sealed",
    "journal_head_before_seal": "sha256:...",
    "evidence_root": "sha256:...",
    "sequence": 42
  },
  "query": "support_cliffs",
  "query_hash": "...",
  "analysis_program_hash": "...",
  "report_request_hash": "...",
  "program_hash": "...",
  "status": "complete",
  "polarity": "violations",
  "report_request": {
    "search_decision_dag": "full",
    "configuration_ledger": {"population": "matching_configurations"},
    "coverage": [
      {"name": "cases", "basis": {"kind": "cases"}},
      {
        "name": "households_with_any_cliff",
        "basis": {
          "kind": "groups",
          "fields": ["household"],
          "require": "any"
        }
      }
    ],
    "mechanisms": {
      "scope": "matching_configurations",
      "observation": {
        "roots": ["question", "show:loss"],
        "endpoint_pairing": {
          "kind": "paired_calls",
          "lower_callsite_id": "site:available-before",
          "upper_callsite_id": "site:available-after"
        },
        "normalization_version": 1
      },
      "observation_spec_hash": "...",
      "support_cardinality": {"mode": "exact"},
      "examples_per_signature": 100
    },
    "case_views": [{"name": "households", "fields": ["household"]}],
    "histograms": [
      {
        "name": "loss_distribution",
        "population": "matching_configurations",
        "field": "loss",
        "unit": "resource units",
        "bin_edges": [0, 10000, 15000]
      }
    ]
  },
  "bounds": {
    "dimensions": [
      {
        "name": "household",
        "bound_index": 0,
        "role": "before",
        "role_field_index": 0,
        "domain": {
          "kind": "values",
          "type": "Household",
          "cardinality": 2
        }
      },
      {
        "name": "income",
        "bound_index": 1,
        "role": "before",
        "role_field_index": 1,
        "domain": {
          "kind": "range",
          "start": 90000,
          "end_exclusive": 110000,
          "cardinality": 20000
        }
      }
    ],
    "fixed": [
      {
        "name": "step",
        "bound_index": 2,
        "role": "context",
        "role_field_index": 0,
        "value": 1
      }
    ],
    "constraints": []
  },
  "boundary": {
    "axis": "income",
    "axis_dimension_index": 1,
    "step": 1
  },
  "projection": {
    "key_fields": ["income_before"]
  },
  "counts": {
    "cartesian_assignments": {"value": 40000, "certainty": "exact"},
    "admissible_configurations": {"value": 39998, "certainty": "exact"},
    "matching_configurations": {"value": 2, "certainty": "exact"},
    "distinct_result_keys": {"value": 1, "certainty": "exact"},
    "mechanism_signatures": {
      "value": 2,
      "certainty": "exact",
      "scope": "matching_configurations"
    }
  },
  "mechanism_evidence": {
    "requested_scope": "matching_configurations",
    "status": "matching_closed",
    "observation_spec_hash": "...",
    "target_cases": {"value": 2, "certainty": "exact"},
    "traced_cases": {"value": 2, "certainty": "exact"},
    "materialized_examples": 2,
    "displayed_examples": 1,
    "support_cap": null,
    "saturated_signatures": {
      "value": 0,
      "certainty": "exact",
      "scope": "matching_configurations"
    },
    "reason": null
  },
  "coverage": [
    {
      "name": "cases",
      "basis": {"kind": "cases"},
      "outcome": "some",
      "closure": "closed",
      "eligible_units": {"value": 39998, "certainty": "exact"},
      "satisfied_units": {"value": 2, "certainty": "exact"}
    },
    {
      "name": "households_with_any_cliff",
      "basis": {"kind": "groups", "fields": ["household"], "require": "any"},
      "outcome": "all",
      "closure": "closed",
      "eligible_units": {"value": 2, "certainty": "exact"},
      "satisfied_units": {"value": 2, "certainty": "exact"}
    }
  ],
  "configuration_ledger": {
    "included": true,
    "population": "matching_configurations",
    "closure": "closed",
    "confirmed_rows": {"value": 2, "certainty": "exact"},
    "case_fields": ["household", "income"],
    "rows": [
      {
        "case": {"household": "Single", "income": 99999},
        "key": {"income_before": 99999},
        "shown": {
          "income_after": 100000,
          "household": "Single",
          "available_before": 109999,
          "available_after": 100000,
          "loss": 9999
        },
        "objective": {"source_field": "loss", "value": 9999},
        "replay": "confirmed"
      },
      {
        "case": {"household": "Couple", "income": 99999},
        "key": {"income_before": 99999},
        "shown": {
          "income_after": 100000,
          "household": "Couple",
          "available_before": 114999,
          "available_after": 100000,
          "loss": 14999
        },
        "objective": {"source_field": "loss", "value": 14999},
        "replay": "confirmed"
      }
    ]
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
        "metric": "loss",
        "objective_value": 14999,
        "selection_closure": "closed"
      },
      "provenance": {
        "scope": "representative",
        "mechanism_signature_id": "mechanism:couple"
      },
      "replay": "confirmed"
    }
  ],
  "graph": {
    "artifact_graph_hash": "...",
    "search_decision_dag": {
      "included": true,
      "closure": {
        "admissibility": "closed",
        "polarity": "closed"
      },
      "root": "case:income",
      "nodes": [
        {
          "id": "case:income",
          "dimension_index": 1,
          "arcs": [
            {
              "ordinal_intervals": [[0, 9999], [10000, 19999]],
              "to": "case:nonmatch"
            },
            {"ordinal_intervals": [[9999, 10000]], "to": "case:match"},
            {"ordinal_intervals": [[19999, 20000]], "to": "case:excluded"}
          ]
        }
      ],
      "terminals": [
        {"id": "case:excluded", "classification": "excluded"},
        {"id": "case:nonmatch", "classification": "admissible_nonmatch"},
        {"id": "case:match", "classification": "admissible_match"}
      ]
    },
    "mechanism_graph": {
      "scope": "matching_configurations",
      "closure": "matching_closed",
      "observation_spec_hash": "...",
      "nodes": [
        {
          "id": "event:threshold-change",
          "site_id": "site:support-income-threshold",
          "kind": "branch_change",
          "dependencies": [],
          "before": "then",
          "after": "else"
        },
        {
          "id": "event:single-arm",
          "site_id": "site:support-household-match",
          "kind": "selected_arm",
          "dependencies": [],
          "endpoint_role": "before_only",
          "arm": "Single"
        },
        {
          "id": "event:couple-arm",
          "site_id": "site:support-household-match",
          "kind": "selected_arm",
          "dependencies": [],
          "endpoint_role": "before_only",
          "arm": "Couple"
        },
        {
          "id": "event:single-observation",
          "site_id": "site:next-step-observation",
          "kind": "observation_root",
          "dependencies": ["event:threshold-change", "event:single-arm"]
        },
        {
          "id": "event:couple-observation",
          "site_id": "site:next-step-observation",
          "kind": "observation_root",
          "dependencies": ["event:threshold-change", "event:couple-arm"]
        }
      ],
      "signatures": [
        {
          "id": "mechanism:single",
          "roots": ["event:single-observation"],
          "support": {"kind": "exact", "value": 1},
          "materialized_examples": 1,
          "examples_truncated": false
        },
        {
          "id": "mechanism:couple",
          "roots": ["event:couple-observation"],
          "support": {"kind": "exact", "value": 1},
          "materialized_examples": 1,
          "examples_truncated": false
        }
      ],
      "incidence_graph": {
        "root": "incidence:household",
        "nodes": [
          {
            "id": "incidence:household",
            "dimension_index": 0,
            "arcs": [
              {"ordinal_intervals": [[0, 1]], "to": "incidence:single-income"},
              {"ordinal_intervals": [[1, 2]], "to": "incidence:couple-income"}
            ]
          },
          {
            "id": "incidence:single-income",
            "dimension_index": 1,
            "arcs": [
              {
                "ordinal_intervals": [[0, 9999], [10000, 20000]],
                "to": "incidence:outside"
              },
              {
                "ordinal_intervals": [[9999, 10000]],
                "to": "incidence:single"
              }
            ]
          },
          {
            "id": "incidence:couple-income",
            "dimension_index": 1,
            "arcs": [
              {
                "ordinal_intervals": [[0, 9999], [10000, 20000]],
                "to": "incidence:outside"
              },
              {
                "ordinal_intervals": [[9999, 10000]],
                "to": "incidence:couple"
              }
            ]
          }
        ],
        "terminals": [
          {"id": "incidence:outside", "classification": "outside_scope"},
          {"id": "incidence:single", "signature_id": "mechanism:single"},
          {"id": "incidence:couple", "signature_id": "mechanism:couple"}
        ]
      }
    }
  },
  "case_views": [
    {
      "name": "households",
      "fields": ["household"],
      "closure": "closed",
      "partitions": [
        {
          "key": {"household": "Single"},
          "admissible_configurations": {"value": 19999, "certainty": "exact"},
          "matching_configurations": {"value": 1, "certainty": "exact"},
          "coverage": "some",
          "distinct_result_keys": {"value": 1, "certainty": "exact"}
        },
        {
          "key": {"household": "Couple"},
          "admissible_configurations": {"value": 19999, "certainty": "exact"},
          "matching_configurations": {"value": 1, "certainty": "exact"},
          "coverage": "some",
          "distinct_result_keys": {"value": 1, "certainty": "exact"}
        }
      ]
    }
  ],
  "histograms": [
    {
      "name": "loss_distribution",
      "population": "matching_configurations",
      "field": "loss",
      "unit": "resource units",
      "closure": "closed",
      "underflow": {
        "upper_exclusive": 0,
        "count": {"value": 0, "certainty": "exact"}
      },
      "bins": [
        {
          "lower_inclusive": 0,
          "upper_exclusive": 10000,
          "count": {"value": 1, "certainty": "exact"}
        },
        {
          "lower_inclusive": 10000,
          "upper_exclusive": 15000,
          "count": {"value": 1, "certainty": "exact"}
        }
      ],
      "overflow": {
        "lower_inclusive": 15000,
        "count": {"value": 0, "certainty": "exact"}
      },
      "total": {"value": 2, "certainty": "exact"}
    }
  ],
  "presentation": {
    "rows_truncated": false
  },
  "execution": {
    "requested_jobs": "auto",
    "effective_worker_high_water": 2,
    "peak_worker_resident_bytes": 734003200,
    "shard_cases": 1024,
    "committed_shards": 17,
    "resumed_from_run_state": true,
    "pressure_scale_downs": 0,
    "heavy_phase_overlap": false
  },
  "completion": {
    "method": "smt-unsat",
    "stop_reason": null,
    "closure": {
      "projection": "closed",
      "admissibility": "closed",
      "polarity": "closed",
      "representatives": "closed",
      "rows": "closed",
      "views": "closed"
    }
  },
  "seal": {
    "kind": "Completed",
    "journal_head_before_seal": "sha256:...",
    "terminal_journal_head": "sha256:...",
    "evidence_root": "sha256:...",
    "payload_hash": "sha256:...",
    "method": "smt-unsat"
  },
  "diagnostics": []
}
```

This is an expanded, explicitly authorized example: its `report_request`
asks for case-level data and matching-case mechanism replay. A baseline v1
request omits `configuration_ledger`, `search_decision_dag`, `case_views` and
`histograms`, and reports at most representative provenance when available.
Each serialized dimension is identified by `(role, role_field_index,
bound_index)`; `name` and `boundary.axis` are presentation labels, while graph
nodes and the boundary use dimension-array indices for structural references.
The half-open `ordinal_intervals` encode `[start, end_exclusive)` in canonical
domain-ordinal order; they are not source values.

`mechanism_evidence` is always present even when `mechanism_graph` is omitted.
It records requested scope, `representatives_closed`, `matching_closed`,
`scope_open`, `incidence_open` or `unavailable`, the observation identity when
one exists, and a source-linked reason for open or unavailable evidence.
When it is `unavailable`, `mechanism_signatures` is absent or has `unknown`
certainty; it is never reported as exact zero.

The schema reuses canonical typed JSON values from
`futuruna.calculate.v1`. Result rows sort lexicographically by key fields in
source order, configuration rows by case coordinates in canonical structural
axis-descriptor order, and case-view leaves by group fields, always using
canonical value order.
Counts have `exact`, `lower_bound` or `unknown` certainty; incomplete coverage
is `undetermined`, including zero confirmed matches.

`execution` is non-semantic run metadata. Worker counts, timings, pressure
samples, shard size and resume history may differ between executions that
produce byte-identical canonical evidence. Paths, host names and raw process
details are never included in the public artifact.

In a complete artifact, included ledger rows equal `matching_configurations`,
`results` rows equal `distinct_result_keys`, and every histogram total equals
its named population. Every search-decision-DAG path resolves to exactly one
terminal, and a closed mechanism-incidence DAG assigns every case in `T_trace`
to exactly one existing signature. Physical node IDs are report-local; canonicality is
defined by the logical ordered graphs and their artifact-content hash. Learned
explanations remain a separate optional view and cannot rewrite exact status,
coverage, graph or counts.

Canonical JSON never silently truncates requested rows. Human truncation is
separate; an answer/case/value search cap instead makes the result `partial`,
while a mechanism-only cap changes only mechanism status. Timing, raw SMT
models, absolute paths and unrequested hidden inputs are excluded. Unknown
additive fields are ignored; an unknown major schema is rejected.

`query_hash` covers the normalization version, normalized question, polarity,
closed occurrence-resolved State and Context schemas, transition mode,
structural role/field/bound axis descriptors,
after-construction DAG, scoped constraints, projection, extrema definitions,
group filter, representative policy and projected field names, order and
types. Axis selection and uniqueness use structural descriptors; labels remain
contract-bound schema/presentation metadata and are never used as selectors.
Compact aliases are hashed by their closed role/field mapping. The frontend
syntax tag is not part of normalized transition IR, although the checked query
artifact still binds its occurrence-resolved source declaration and program.
Unlike extensional `TransitionId`, this query identity binds mode and after
recipes because changing how the declared search generates an edge changes the
proposition and resumable work.
`analysis_program_hash` covers the normalized declarations and stable semantic
sites reachable from every semantic root: domains, fixed and derived facts,
constraints, the question, key, extrema, group filter, shown values,
representative objective and mechanism observation roots. It is independent
of which reachable sites a particular replay slice retains, and excludes
continuation code and nominal row-type spelling. Stable site IDs are
namespaced by this hash rather than the full program hash;
`observation_spec_hash` identifies `h` separately.

`report_request_hash` covers the complete serialized definitions of authorized
case/value views, populations, group quantifiers, histogram fields/units/bin
edges, mechanism scope, full observation specification, support-count mode and
example-retention cap. `artifact_graph_hash` covers
the normalized case evidence, exact open frontier, normalized mechanism
occurrence/signature DAG content and incidence actually present in this
artifact. It is an evidence/content hash and therefore changes when a larger
budget closes more regions. Operational budgets are excluded from semantic
identities and recorded as run metadata.

For a query declaring probes, `domain_hash` covers the normalized
domain-and-`CaseId`
portion of `query_hash`: the closed product schemas, role-tagged independently
varied domains with their role/field/bound descriptors, fixed and derived facts,
after-field sources and dependency DAG, constraints, canonical dimension order
and endpoint membership. A
boundary optimizer hint contributes only through the canonical transition and
membership semantics it asserts or synthesizes, not through its source
spelling.
`probe_plan_hash` covers the ordered probe selectors and lift operations,
deterministic adaptive/tie-break versions, semantic case cap, retained field
allow-lists and mechanism-trace authorization. Neither hash includes the
run-state path or invocation-level budgets. Both are bound by `RunOpened` and
carried by authorized journal events and snapshots; they are not new answer
identities and do not alter `futuruna.explore.v1` result equality.

`program_hash` remains the identity of the complete resolved program and may
therefore change when a row declaration or continuation changes. The identity
passed to the continuation is byte-for-byte the same identity emitted in the
canonical report. A continuation cannot rewrite any identity.

`output { ... }` and `output as Row` project the same canonical result shape;
the typed form adds a source-level check over the key and shown payload and may
be followed by a `then` continuation.

## Human Result Contract

Human output begins with the answer and its scope. The expanded example below
corresponds to the same explicit case/value/mechanism `ReportRequest` as the
JSON example; baseline output omits sections that were not authorized:

```text
Exploration: support_cliffs
Status: COMPLETE

Question: where does next_step_never_hurts fail?
Search: every Household value; income 90,000–109,999; step 1
Result identity: one answer per income

Admissible configurations: 39,998
Matching configurations: 2 / 39,998 (SOME)
Households with at least one cliff: 2 / 2 (ALL)
Different income steps found: 1
Distinct mechanism signatures: 2
Mechanism coverage: 2 / 2 matching configurations (CLOSED)

99,999 -> 100,000
Representative household: Couple
Loss after the next unit: 14,999

Loss distribution over 2 matching configurations:
  below 0: 0
  [0, 10,000): 1
  [10,000, 15,000): 1
  15,000 and above: 0

Exact partition view `households`: 2 admissible groups
Matching-configuration ledger: 2 replay-confirmed rows
```

It then prints representatives, authorized objective values, changed rule
branches from each representative replay, source references, fixed facts,
constraints and exclusions. A hidden objective used only for selection is not
published.

The primary finding count is always `distinct_result_keys`, described using the
key field names. Graph-derived views name their populations and closure scope
explicitly. The tool does not print bare `findings: N`, `mechanisms: N`,
`coverage: all` or `histogram: N` wording when it would hide whether the unit
is a configuration, result key, mechanism signature or group.

If human output omits rows for readability, it says how many of the exact total
are shown and where the complete artifact was written. Such presentation
truncation never changes search status or count certainty.

## Provenance

The mechanism graph is constructed from common fresh-runtime replay, never
from solver internals or an exact backend's enumeration path. Stable semantic
site IDs are namespaced by `analysis_program_hash` and include module/declaration
identity, operation or rule-candidate identity, and a structural AST child
path. Source spans, line numbers, labels and legal-source metadata annotate
those sites but do not define identity.

Every traced result records its case or representative scope, the mechanism
signature ID, attached source references and replay confirmation. Differential
branch observations are included only when lower and upper endpoint
computations can be paired soundly. Otherwise the artifact explicitly records
that differential mechanism evidence is unavailable.

Structural boundary extraction MAY suggest candidates or enrich an already
confirmed explanation. It does not define `D`, `M`, `R`, the search decision DAG or
mechanism completeness. The mechanism graph records differential execution
evidence; it does not by itself establish legal causality or prove the source
interpretation.

Representative scope describes only the selected case for each key. An exact
count of distinct mechanism signatures among all matches requires fresh replay
and incidence closure over all `M`. A mechanism node's supporting cases may
overlap with those of another node, so only complete signature classes are
counted as a partition.

The first-class typed report does not promote representative traces or source
attachments into the declared row. Canonical provenance remains in the human
and JSON artifacts. A continuation that needs a mechanism as typed data must
expose that mechanism deliberately through `key` or `show`; it cannot recover
hidden mechanisms from the report identity.

## Feature Stage

The syntax, CLI and JSON contract begin as **Experimental**. `output { ... }`
is the CLI-only source form; `output as` and `then` belong to that same
`solver-backed-exploration` surface and remain specified, not implemented. The solver-assisted
`runa verify` surface is Preview, but exploration introduces a new language and
operational contract that needs real corpus experience before promotion.

Implementation adds a separate `solver-backed-exploration` feature-stage
surface. The RFC, feature-stage documents and CLI help identify the status.

The implementation uses a distinct exploration AST and typed query IR rather
than overloading `Stmt::Prove`. Every canonical AST/FIR walker, formatter,
typechecker, import-hygiene pass, semantic-interface classifier, LSP path and
compiler-pass coverage matrix MUST classify the new node explicitly.

The exploration node has no ordinary runtime or native-codegen behavior. Its
optional continuation is type-checked as analysis-only code and is scheduled
only by `runa explore` after a report exists.

## Implementation Slices

1. Freeze transition grammar, compact-source normalization, answer-set semantics and
   diagnostic expectations.
2. Add AST, parser, formatter, spans and traversal coverage for explicit
   transition fields, optional `output as` and `then`.
3. Add query-local before/after/context scope, purity checks, typed domain and
   endpoint-state elaboration, exact output product validation and isolated
   continuation type-checking.
4. Add a non-optional normalized Explore transition IR independent of Z3, with
   framed, independent and indexed-DAG derived after fields resolved from
   `after.OTHER`; retain the continuation outside the solver-semantic IR.
5. Add backend-neutral `CaseId`, `StateId`, `TransitionId`, key, value, count
   and per-layer closure evidence. State/Context schemas close resolved declared
   owners, while `TransitionId` remains extensional and independent of recipe
   or mechanism identity.
6. Add exact finite exhaustion as the reference producer of canonical
   transition cases, `D_C`, `M_C` and `R` evidence.
7. Build one crate-private reduced ordered search-decision-DAG core and validate
   strict role/field/bound axis descriptors, canonical arc coalescing, graph
   expansion equivalence, path-count conservation and open-frontier handling.
8. Publish the separately scoped semantic transition graph from validated
   identity/support evidence; add deterministic
   representative selection, exact objectives and graph-derived ledgers,
   coverage, partitions and histograms.
9. Add stable analysis-program/declaration/decision identities, explicit
   mechanism observation identity and an optional fresh-interpreter replay
   trace sink.
10. Add concrete replay from one checked endpoint template in both states,
    sound differential pairing, normalized occurrence-DAG signatures, a second
    ordered edge/signature incidence DAG and independent mechanism closure.
11. Lower polarity, domains, constraints and canonical rule dispatch to SMT as
    another producer of the same evidence; add projection blocking and final
    `UNSAT` closure without exposing solver structure.
12. Add `runa explore`, human output, the exit-code contract and explicit
    privacy-authorizing `ReportRequest`; publish separately named canonical
    search-decision and semantic-transition graph fields.
13. Construct the status-safe `ExplorationReport(Row)` and execute only the
    selected continuation in a fresh environment, with hash isolation, JSON
    channel isolation and artifact-preserving failure behavior.
14. Add boundary-axis validation and a hash-bound `BoundaryPlan`: guarded
    mechanism-labelled candidates, semilinear interval/congruence certificates,
    explicit open residuals, SMT/CEGAR refinement and finite singleton
    fallback.
15. Add typed `ProbePlan` lowering, deterministic candidate/endpoint/midpoint
    and cross-profile lift scheduling inside the unified run journal, same-run
    pause/resume, privacy allow-lists and observable probe milestones. Reserve
    fresh validation replay for evidence imported across a trust boundary.
16. Extend exact lowering through the required Personskat rule slice.
17. Run the Personskat query without manually supplied tax thresholds and
    inspect its case regions, mechanism signatures and derived distributions.
18. Publish feature stages, reference, tutorial and agent guidance.
19. Run focused proof tests, mint, the relevant canary and differential lanes.

## End-to-end Acceptance

Two queries establish different guarantees.

### Current Personskat calibration evidence

The development preview has supplied three empirical timing and result points for
the encoded 2026 model:

- the first known § 9 C boundary across all 98 municipalities and four selected
  church-tax/commute profiles per municipality evaluated 392 transitions,
  found `M = 392`, and completed in 1,351.44 seconds;
- all 50 known § 9 C boundaries across two municipalities and the same four
  profiles evaluated 400 transitions, found `M = 400`, and completed in
  477.67 seconds;
- all 50 source-derived § 9 C boundaries across all 98 municipalities and the
  same four witness profiles evaluated 19,600 transitions, found
  `M = 19,600`, and completed in 146.19 seconds after specializing the exact
  finite-difference path.

These are observed exact-finite executions of their declared candidate domains,
not closure evidence for the full income axis. For annual income through
3,000,000 DKK, the naive Cartesian scan is about 1.176 billion transitions
(`98 * 4 * 3,000,000`). Restricting evaluation to the 50 source-derived § 9 C
steps produced 19,600 candidates (`98 * 4 * 50`) in the completed witness run.
Its exact case-loss histogram was 9,800 cases in 50.00–99.99 DKK, 770 in
100.00–149.99 DKK, and 9,030 in 150.00–199.99 DKK. That closes the declared
witness matrix, not every reachable Personskat mechanism or every valid commute
distance.

The distances 60 km and 130 km are therefore calibration witnesses, not
semantic bounds discovered by Explore. Source specialization of the encoded
commute formulas exposes a broader non-monotone distance topology: no cliff at
0–24 km; all municipality/church/boundary profiles cliff at 25–282 km; and the
first conditional cell at 283 km, where the first § 9 C boundary is neutral for
the 25 outer-municipality profiles in both church states. Further exact source
bounds show no outer-municipality cliff after 659 km and no ordinary-municipality
cliff after 1,367 km for this 203-workday slice. These are proof candidates for
the boundary-certification backend, not yet a closed public Explore artifact.

The source-derived obligation is to extract every reachable event support and
prove `Delta_1 N >= 0` on its complement, with exact endpoint validity. The
empirical obligation is then to evaluate or certify the candidates, replay the
reported cases and retain any SMT/CEGAR or singleton residual. Until both
obligations close under the recorded program hash, the RFC makes no claim that
the 50 steps are the full Personskat result.

Likewise, "the § 9 C phase-out" currently names one coarse legal family, while
two different `min` arms are hypotheses for two dynamic trace signatures.
Neither is yet an exact mechanism-bin count. Such a count requires the
query-relative signature target and incidence, plus every loss value used for
bin membership, to close through provenance replay or an explicit proof.

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
- reports admissible configurations, matching configurations and distinct
  income-transition keys as three separately named exact counts;
- constructs a lossless search decision DAG whose expansion reproduces every
  admissible classification and preserves municipality/income correlations;
- can expose every replay-confirmed matching configuration when its ledger is
  authorized, without confusing that population with projected findings;
- derives an exact loss histogram and municipality coverage from the same
  admissibility- and polarity-closed case/value evidence rather than hidden
  sub-explorations, only when those case-level views were authorized;
- reports the number of distinct observed mechanism signatures only with an
  explicit matching-case or representative scope and matching closure;
- permits disconnected thresholds and municipalities to reference one shared
  mechanism signature without widening their exact case regions;
- retains and classifies every additional replay-confirmed transition;
- reports unavailable differential evidence honestly when endpoint calls
  cannot be paired; and
- reaches `complete` only after every answer/case/value population required by
  the selected report contract is closed, while mechanism evidence reports its
  own closure independently.

Changing an encoded threshold changes the discovered transitions in both
queries without any edit to either exploration query.

### Typed result continuation

The same queries can use CLI-only `output` blocks or opt into the typed form:

- the declared row product exactly matches key plus show;
- a complete result exposes every replayed, sorted row as `findings`;
- partial and unknown results expose only confirmed rows;
- unsupported and error results expose no list that can masquerade as
  complete;
- only the selected root continuation is automatically delivered, at most once
  per `run_id`, with crash ambiguity and explicit idempotent retry reported;
- ordinary execution, imported queries and unselected queries run none;
- changing only `then` leaves `query_hash` unchanged;
- hidden assignments and raw solver state are unavailable to the continuation;
- an auxiliary configuration ledger is not injected into the typed report;
  receiving configurations as `findings` requires an output key `K` that is
  injective on `M`;
- JSON stdout remains one valid document;
- continuation failure leaves the canonical exploration artifact intact.
