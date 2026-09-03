# Bounded Rule Exploration with `? explore`, `? analyze`, and `? publish`

Status: accepted architectural direction; Experimental breaking replacement

This RFC defines Futuruna Explore as a finite, provenance-aware relational
language over typed state transitions. `? explore` declares the bounded world,
its admission rule and zero or more named questions. Query-local `derive`
declarations name pure computed values and endpoint observations. A separate
`? analyze` declaration builds an order-independent typed DAG of views, choices
and replay-derived explanations over those questions. `? publish` attaches
authorized materializations without becoming part of that semantic DAG.

The contract in this opening section is normative for new implementation. It
replaces the earlier Cartesian
`over`/`bounds`/`boundaries`/`probes`/`output` design. The current nested
FROM/TO/WHERE/named-FIND plus `results`/`mechanisms` surface is the executable
lowering checkpoint toward the separated Analyze/Publish syntax, not a second
semantic language. Neither Experimental spelling has a compatibility claim.

Normative here means “the implementation target,” not “already executable.”
The frontend, closed relational IR, executor, journal and result DAG are moving
through the slices below and are not all verification-backed yet. User-visible
artifacts MUST continue to report the actually implemented boundary honestly.

The companion [implementation workbook](bounded-rule-exploration-workbook.md)
turns this contract into executable checkpoints. The income-tax steering
workbook at
[examples/danish-income-tax/exploration-workbook.md](../../examples/danish-income-tax/exploration-workbook.md)
records the motivating multidimensional questions and the expected result
shape.

## Normative relation contract

Ordinary execution evaluates one state. Explore asks which explicitly bounded
before-to-after state transitions satisfy zero or more questions, which
coherent model profiles support them, and—when requested by an analysis—which
replay-derived mechanisms those transitions share.

The target language has three declaration layers:

1. `? explore` constructs one finite transition relation, names reusable pure
   `derive` values, applies one total admission predicate and names any number
   of total questions over the same admitted cases.
2. `? analyze ... from explore ...` declares an order-independent acyclic graph
   of explicit `view`, `choice` and `explain ... using` nodes over that Explore
   contract.
3. `? publish ... from analyze ...` attaches authorized readers and output
   addresses to one checked analysis graph.

`observe support`, `materialize support cases`, `materialize support starters`
and `materialize transitions` are checked publication readers. They are
declared only in `? publish`, outside the Analyze semantic DAG.
Adding, removing or renaming such a reader MUST NOT rename the RelationId,
AdmissionId, any QuestionId, ViewId, ChoiceId or MechanismRequestId, the
analysis graph, journal contract, or evidence already minted for the analysis.
The journal contract always contains the checked StateSchemaId,
ContextSchemaId and TransitionTypeId whether or not a reader is attached.

Cases precede mechanisms in semantic dependency. Execution MAY interleave
source discovery, classification, view reduction and replay for different
cases; no global phase barrier follows from the algebra.

For every successful replay, the first case that reveals a content-new
signature MUST intern one mechanism definition and add one incidence; later
cases with the same signature MUST add incidences without cloning that
definition. Until the target frontier closes, signature support is a lower
bound. A certified uniform support cell MAY increase support by its exact
weight without materializing one CaseId per member. A scheduler MAY prioritize
open case regions adjacent to signature, admission or finding changes, but
mechanism novelty MUST NOT itself mint cases, transfer coverage between
different RelationIds or alter the final extensional result. Thus case and
mechanism DAGs co-evolve operationally while remaining separated by their
typed incidence relation and closure rules.

“Case graph” and “mechanism DAG” describe evidence projections, not additional
source-language constructs. The semantic base is the finite transition
relation plus case-to-mechanism incidence. A transition projection may contain
cycles; only the Analyze dependency graph and an individual replay-derived
mechanism occurrence graph are required to be acyclic. This distinction keeps
the useful graph architecture without overfitting the language to graph
terminology or to income-cliff traversal.

### Canonical target form

The target source and analysis form is:

```runa
? explore personskat_changes {
    from {
        given tax_year = 2026
        vary profile in coherent_personskat_profiles(tax_year)
        vary income_kroner in range(0, 200_000)
        let context = SalaryChange(amount_kroner = 1)
        let before = person_state(profile, income_kroner, tax_year)
    }

    transition after = apply_salary_change(before, context)

    derive policy_assessment {
        before = assess_personskat(before, context)
        after = assess_personskat(after, context)
    }
    derive disposable_change_ore =
        policy_assessment.after.disposable_ore -
        policy_assessment.before.disposable_ore

    admit supported when policy_assessment.before.supported
        && policy_assessment.after.supported

    find supported_cases = all
    find cliffs = violations of (disposable_change_ore >= 0)
    -- or: find improvements = matches of (disposable_change_ore > 0)
}

? analyze personskat_income_cliffs from explore personskat_changes {
    view supported_case_summary from find supported_cases {
        group all
        aggregate [cases = count_distinct(case_id)]
        select [cases]
        updates closure_gated
    }

    view cliff_cases from find cliffs {
        each case
        measure [loss_ore = -disposable_change_ore]
        select [case_id, context, before, after, loss_ore]
        updates monotone
    }

    choice worst_cliffs from find cliffs {
        partition all
        measure [loss_ore = -disposable_change_ore]
        choose all maximizing loss_ore
        updates revisable
    }

    view worst_cliff_cases from choice worst_cliffs {
        each case
        select [case_id, context, before, after, loss_ore]
        updates closure_gated
    }

    explain cliff_paths from find cliffs using policy_assessment

    view mechanism_loss_bins from explain cliff_paths {
        measure [loss_ore = -disposable_change_ore]
        group by [loss_bin_ore = bin(loss_ore, 5_000)]
        aggregate [mechanisms = count_distinct(structural_mechanism_id)]
        select [loss_bin_ore, mechanisms]
        updates closure_gated
    }
}

? publish personskat_income_cliff_evidence
from analyze personskat_income_cliffs {
    emit view supported_case_summary
    emit view cliff_cases
    emit view mechanism_loss_bins

    observe support cliff_node
        from explain cliff_paths
        for node differential "<StructuralNodeId>"
        within mechanism "<StructuralMechanismId>"

    materialize support cases cliff_node_cases
        from explain cliff_paths
        for node differential "<StructuralNodeId>"
        using values from view cliff_cases

    materialize support starters cliff_node_starters
        from support cases cliff_node_cases

    materialize transitions full_case_graph
        from explore personskat_changes all cases
}
```

`Transition<Context, State>` is the closed record type containing typed
`context`, `before` and `after` fields. A `transition` declaration creates the
reserved value `transition` at that type for every constructible case. It is a
one-step relation, not a temporal behavior. A singleton `after =` and an
exact-finite `after in` lower to the same successor relation. When successor
dimensions must remain visible, the finite form may use an ordered block:

```runa
transition after {
    vary option in candidate_options(before, context)
    vary route in candidate_routes(before, context, option)
    let plan = plan_for(option, route)
    yield apply_plan(before, context, plan)
}
```

The block uses the same dependent `vary`/`let` discipline and has exactly one
typed `yield`. It is syntax for one set-normalized conditional successor fiber,
not a second transition model.

Bindings in `from` are ordered and have explicit coverage roles:

- `given name = expression` contributes one explicitly conditioned immutable
  value;
- `vary name in finite_expression` expands one exact finite factor or dependent
  fiber; and
- `let name = expression` deterministically derives one value and never acts as
  an independent cardinality multiplier.

`given` is source-independent: its expression may use sealed immutable module
values and pure helpers, but no binding in the same `from` relation. `vary` and
`let` expressions may refer only to earlier bindings. Exactly one resulting
`context` and one `before` binding feed the transition. Auxiliary bindings are
authenticated construction lineage, not hidden case-identity fields.
Alpha-renaming a resolved local binder does not change semantics.

A query-local `derive` names a reusable, referentially transparent computed
column. Its right-hand side may depend only on relation bindings, the typed
transition, earlier derives and checked immutable definitions. Derives are
ordered after transition construction; self-reference and forward reference
are errors. They cannot perform effects, enumerate hidden cases or observe
scheduler state. The paired endpoint form shown above must
apply one checked observer shape to Before and After; `explain ... using
policy_assessment` reruns that observer freshly with tracing rather than
substituting cached values. A derive's normalized dependency closure is
incorporated at each semantic use site; alpha-renaming it does not change
identity. A use in `admit`, `find`, `view`, `choice` or `explain` must satisfy
that site's totality and coverage obligations.

`admit NAME when BOOLEAN_EXPRESSION` is one total Boolean classification over all
constructible cases. The name is a source address; its normalized expression
and RelationId define AdmissionId. `admit all when true` spells an unrestricted
admission. Each named `find` independently reads that admitted relation:

```runa
find NAME = all
find NAME = matches of TOTAL_BOOLEAN_EXPRESSION
find NAME = violations of TOTAL_BOOLEAN_EXPRESSION
```

An Explore declaration may contain any number of named `find` clauses. They
share one RelationId and AdmissionId; each name resolves to the QuestionId
derived from its predicate and polarity. Semantically different forms have
different IDs, while two identical normalized finds are aliases of one content
ID. Predicate evaluation is `true`, `false`, or an
integrity error—not three-valued negation. `matches` selects only `true` and
`violations` selects only `false`. An error selects neither, leaves the
question unable to close exactly and is never negated into a violation.

An `? analyze NAME from explore NAME` declaration contains a typed dependency
DAG scoped to that Explore contract. Its semantic nodes are explicit:

- `view NAME from INPUT` derives a relation and public projection from exactly
  one named input;
- `choice NAME from find NAME` applies an explicit partition, measure,
  eligibility, one/all-ties/argmin/argmax or Pareto policy and produces a
  case-bearing chosen relation; and
- `explain NAME from TARGET using DERIVATION` requests fresh endpoint replay for
  a named find or choice and produces typed signature incidence.

Inputs are always qualified by relation kind (`find`, `choice`, or `explain`);
the Analyze header supplies the one Explore namespace and there is no implicit
`from selected`. Declarations may appear in any source order. The checker resolves all names first,
canonicalizes the node and edge sets independently of declaration order, and
rejects missing references, type/grain mismatches and cycles. Recursive
analysis nodes are outside this contract.

A `choice` is not embedded in or fed by a display view. Its input QuestionId
describes the candidate case relation; ChoiceId adds partition, measure,
eligibility, objective, tie/cardinality and deterministic ordering semantics.
A later display view may read that choice. An explanation may target it directly
without pretending the choice is a display transformation.

The row schemas are closed and typed. A `find` row is the complete canonical
case row: CaseId, SourceKey, SuccessorKey, typed transition and every
addressable query derive. A `choice` row preserves its input row and adds its
named partition keys and measures; those additions are semantic columns in
ChoiceId, not public fields. An `explain` row preserves its resolved find or
choice target row and adds TransitionId, raw-signature and available quotient
incidence IDs. This row-preserving rule lets a mechanism view group or measure
by the case facts whose replay produced the incidence without a hidden join.

Addressability does not make every derive an identity dependency. Each find,
choice, explain or view records only the normalized DerivationIds actually
referenced by its own predicate, partition, measures, observer or projection,
plus the dependencies inherited through its explicit input. Adding an unused
derive therefore renames no consumer. Only a `view ... select` creates a public
value schema. Publication may further restrict authorization or retention but
cannot infer hidden columns or widen that schema.

Choice clauses have one evaluation order independent of source declaration
order: compute total measures per candidate row, form exact partitions, apply
the optional partition-level `having`, then apply `choose` or `pareto`.
`having varies(M)` is true exactly when the closed partition contains at least
two distinct exact values of measure `M`. A false result produces an exact empty
choice for that partition; an open or unavailable measure cannot manufacture a
winner or a no-difference conclusion.

For a `pareto` choice, candidate `x` dominates candidate `y` iff `x` is no worse
than `y` on every declared objective and strictly better on at least one, using
each objective's declared direction. Every objective must be total and exactly
comparable over the partition. The choice retains every undominated CaseId.
Distinct cases with equal objective vectors do not dominate one another and are
all retained unless a separate, explicit equivalence quotient is introduced.

Every view and choice has a checker-derived public update mode, optionally
asserted by an `updates` clause:

- `monotone` emits only additions whose membership cannot later be revoked;
- `revisable` emits explicit add/retract revisions; and
- `closure_gated` emits no public rows until every closure-sensitive input is
  exact.

An incorrect authored assertion is a type/checking error. Positive per-case
projection may be monotone. `having`, exact distinct aggregates, extrema,
argmin/argmax, all-ties and Pareto membership are revisable while their inputs
remain open or closure-gated when only final output is requested. An
append-only journal records revisions; it MUST NOT make a revisable public
relation appear append-only by leaving retracted rows active.

Every public relation has a canonical row key: CaseId for `each case`, the
declared canonical group key for grouped views, `(partition key, CaseId)` for a
choice, and the typed incidence key for explanations. `add(key, row)` is
idempotent only for the same row hash; `retract(key, prior_row_hash)` is valid
only for the currently active row. One journal transaction may atomically
retract old rows and add replacements. `seal(active_set_root, count)` must equal
the canonical fold of all preceding valid events and forbids later semantic
updates to that node. This keyed fold is the entire public revision algebra;
there is no separate `supersede` operation.

Publication readers in a separate `? publish NAME from analyze NAME` declaration
are resolved only after the semantic DAG and are excluded from its identity.
`observe support` requests a value-free compact support slice. `materialize
support cases` publishes the correlated `S` members, including source/successor
identity. `materialize support starters` publishes only the deduplicated `P =
distinct_sources(S)` members. `materialize transitions` requests an
independently resumable transition materialization. Publication declarations
are order-independent readers, and their names are output addresses rather than
semantic node IDs.

### Current Experimental executable boundary

The separated target syntax above is not yet a complete implementation claim.
Publication v15 and report v8 execute the nested checkpoint with ordered
`from` bindings, `transition after`, scoped `where`, zero or more
`find NAME = all|matches|violations` declarations, explicit
`results ... from find NAME`, `mechanisms ... from find NAME using OBSERVER`,
and `mechanisms ... from view VIEW chosen using OBSERVER` consumers,
`observations`, `starters` and `transitions`.
The existing engine's `results` maps toward target `view`; embedded `choose`
maps toward target `choice`; and `mechanisms` maps toward target `explain ...
using`. Its placement-order dependency rule is implementation history, not the
target order-independent analysis semantics.

Most importantly, the v14 executable `starters NAME ... using values from VIEW`
consumer publishes correlated `S` case-support members, not deduplicated `P`
starters. That spelling and the `starters/` artifact path are explicitly
transitional historical names. They remain documented below so current bytes
are interpreted honestly; the target frontend replaces them with `support
cases`. A future true `support starters` consumer publishes one SourceKey per
member and cannot alias the case-support artifact.

The current concrete execution path evaluates every unique named question over
one shared relation and admission, while identical normalized aliases reuse one
QuestionId and predicate evaluation. Certified sweep and regional-proof
accelerators are still exact-one and fall back to that concrete stream for
zero/plural question sets without choosing a primary. A chosen-view mechanism
target must name an earlier FIND-backed result containing `choose`. Execution
waits for that exact result to publish, scans its authenticated projection in
bounded resumable chunks, and admits each CaseId with its exact projection
ordinal as durable provenance. The target seal binds the immutable result root,
and only those chosen cases enter the ordinary bounded incidence scheduler. A
complete admitted mechanism landscape can now be another named
`find NAME = all` in the same relation as a cliff question. These are staged
implementation limits, not competing semantic definitions.

In the target language an `observe support` declaration addresses exactly one
request-relative compact
support slice. Its subject is one whole structural mechanism, one activation or
differential-participation structural node, or one activation or
differential-participation structural edge. A node or edge may additionally be
qualified `within mechanism`; a whole-mechanism subject MUST NOT be. The
selector is value-free and does not authorize Context, Before, After, CaseId or
starter-cell publication. The complete forms are:

```runa
observe support WHOLE
    from explain REQUEST
    for mechanism "<StructuralMechanismId>"

observe support NODE_OR_EDGE
    from explain REQUEST
    for node activation "<StructuralNodeId>"
    -- or: for node differential "<StructuralNodeId>"
    -- or: for edge activation "<StructuralEdgeId>"
    -- or: for edge differential "<StructuralEdgeId>"
    -- optional for a node or edge only:
    within mechanism "<StructuralMechanismId>"
```

The declaration name is an output address, not semantic slice identity. The
checked demand ID binds the resolved `MechanismRequestId`, subject, facet and
optional enclosing mechanism, but excludes the name and declaration position.
Aliases of the same slice share one registration and one observation stream.
The demand-set ID deduplicates and sorts those IDs, so declaration order,
aliases and renames do not change it. Both identities remain outside
RelationId, AdmissionId, QuestionId, MechanismRequestId and the analysis-graph
digest; attaching a reader MUST NOT rename or reopen the core DAG.

The explanation incidence relation first carries the complete raw replay assignment
(`signature_id`). Once that raw signature has a validated structural quotient,
the target result algebra also exposes its `structural_mechanism_id` and
`execution_profile_id`. A user-facing count named `mechanisms` MUST count
distinct `structural_mechanism_id` values. Distinct raw signatures and execution
profiles are separately useful audit populations and MUST be named accordingly;
neither is a substitute for the structural-mechanism count. The current
authored incidence-row executor exposes `signature_id`; structural mechanism
and profile counts are available in the run report and structural artifacts,
but authored result expressions over the two quotient IDs remain an
implementation target.

The target grammar is intentionally one language, not a compatibility adapter:

- every Explore and Analyze declaration is named;
- `from`, `transition`, one total `admit` and zero or more named `find` clauses
  define an Explore relation and its questions;
- `given`, `vary` and `let` make source-conditioning, finite variation and
  derived construction distinguishable in syntax and coverage;
- `derive` expressions are reusable, checked and pure;
- explicit views, choices and explanations form a separately named,
  order-independent acyclic Analyze graph;
- checked observation/publication readers live outside semantic Explore and do
  not enter the Analyze graph;
- there is exactly one semantic `before` binding and one semantic `context`
  binding;
- `after = EXPR` is singleton successor syntax;
- `after in EXPR` requires a checked exact-finite collection;
- Before and After have the same closed state type;
- transition, admission, question, view, choice and observer expressions are
  checked, pure and total on their declared finite domains; and
- the old `over`, `bounds`, `boundaries`, authored `probes`, transition-mode,
  transitional `where`/unnamed `find`/`results`/`mechanisms`/`starters`,
  `observe mechanisms with`, `output`, `output as` and `then` clauses are not
  part of the final contract.

Global parsing, type checking and checked name/type resolution precede Explore
or Analyze selection; a failure in those program-wide authorities is global.
Successful checking eagerly creates one lightweight artifact slot per named
declaration. The caller selects an Explore relation/question or Analyze graph
before any request-scoped proof is attempted. Access to that slot then lazily
materializes its expensive checked artifact, identity ladder and, when it is
explanation-bearing, endpoint-totality obligations and any available static
certificates. A declaration-local artifact or proof-strategy failure is retained on that slot and does not poison a
valid sibling slot, although selecting the failed slot reports its failure.
This ordering avoids proving every declaration merely to select one without
weakening global program checking.

A mechanism observer has checked shape `(State, Context) -> Observation`.
Every mechanism request carries one target-relative endpoint-totality
obligation: the observer must return an `Observation` independently at every
distinct Before and After endpoint in the exact target population. That one
obligation may close by either of two evidence strategies:

1. a static certificate discharges the reachable observer call/rule closure
   over sound finite over-approximations of the target's Before and After
   marginals, including partial dispatch, overflow, zero division, effects,
   recursion and unresolved calls; or
2. after the finite target closes, an extensional certificate binds one
   successful canonical evaluation receipt for every distinct required
   endpoint. Endpoint reuse is allowed, but a sample or open prefix is not a
   totality proof.

The `RelationId` retains the exact correlated `(Context, Before)` relation and
conditional successor fibers. A static certificate may deliberately prove a
larger Cartesian marginal population; an extensional certificate proves only
the exact closed target. Both bind the same obligation and neither is mislabeled
as starter support or successor materialization. The proof strategy is physical
evidence and never changes MechanismRequestId or AnalyzeGraphId.

Fresh traced evaluation remains the sole source of concrete endpoint traces and
signatures. A statically certified request may replay immediately. An
uncertified request may also replay concrete endpoints and publish lower-bound
incidence, but it cannot become exact until its extensional certificate closes.
A deterministic semantic failure before static certification yields
`unavailable(observer_partial)` for that obligation and prevents exact
explanation/support claims; it is never negated into a mechanism fact. The same
failure after a valid static certificate is an integrity error: the event is
rejected and the journal remains at its preceding valid semantic root.
Cancellation and transient host pressure leave the work frontier open and may
pause the run. A deterministic instrumentation or capacity boundary likewise
yields typed unavailable evidence and prevents exactness when a required
endpoint lacks a complete signature.

An exact-finite `vary` producer may be a list, end-exclusive range, finite enum,
dependent join, indexed relation or certified symbolic cell. Its checked
contract MUST expose a canonical element schema, set normalization, a resumable
enumerator and a closable frontier. Separate independent `vary` bindings form a
product. Authors SHOULD instead construct coherent typed profiles when facts
are correlated; structurally impossible profiles do not belong in the source
relation merely to be filtered later.

The checked Explore artifact MUST expose two `RelationId`-scoped coverage
components. `SourceCoverageId` covers the checked `given`/`vary`/`let` producer
closure and recursively walks Context and Before field paths.
`SuccessorCoverageId` covers transition construction, every transition-block
`vary`/`let`/`yield` dependency and recursively walks After field paths. Both
include variant and nested-record segments until closed leaves. An open,
recursive or unsupported composition becomes an explicit gap at the affected
path boundary; it is never silently omitted. Together they cover the closed
Context/Before/After schemas without confusing source support with a dependent
successor fiber.

For each covered Context, Before or After path, and each reachable immutable producer input,
the applicable manifest records whether it is a varied finite dimension, derived from
declared dimensions, explicitly conditioned to a singleton or source
restriction, covered by an exact irrelevance certificate, or an acknowledged
model-coverage gap. Literals and referenced immutable top-level constants
inside ordinary producer helpers remain visible conditioning. The manifest is
derived from the checked source or transition producer closure and does not add
a second source DSL or invent an undeclared dimension.

The exact-irrelevance category is proof-gated. Its presence in the manifest
algebra does not claim that an irrelevance producer exists for every query; in
the absence of producer-issued exact evidence the compiler MUST NOT label a
path irrelevant. Declared support remains intact, while an unmodeled or
unproved path is a coverage gap. A gap forbids any broader population claim,
although an exact result over the smaller declared relation remains exact over
that relation.

Admission and question input coverage MUST be separate sibling artifacts,
scoped respectively by `AdmissionId` and each `QuestionId`. Every Analyze node
also has one generic `AnalysisNodeCoverageId` covering only dependencies added
by that view, choice or explanation observer. These components reuse the same
vocabulary but do not rewrite relation coverage when an admission, question,
view, choice or observer changes.

Every executable question or Analyze graph has a composed `CoverageBundleId`:

```text
CoverageBundleId = H(
    SourceCoverageId,
    SuccessorCoverageId,
    AdmissionCoverageId,
    sorted applicable QuestionCoverageIds,
    sorted reachable AnalysisNodeCoverageIds
)
```

The bundle is the authority for population-scope language in a report. An exact
answer over the declared finite relation remains exact even when one component
contains a coverage gap, but the report MUST expose that gap and MUST NOT turn
the result into a broader claim about an unmodeled real-world population. A
view, choice or explanation binds exactly the coverage components reachable
from its typed input edges plus its own AnalysisNodeCoverageId; unrelated
questions or analysis nodes do not contaminate its bundle.

This composed bundle is the normative target. Publication v15 currently emits
the RelationId-scoped source manifest and carries admission/question/request
identities, but does not yet emit every sibling coverage artifact or the
composed bundle. Current artifacts MUST describe that boundary rather than
claiming the target coverage contract.

Source conditioning and admission are different. A `given` value or restricted
`vary` producer declares a smaller world and changes `RelationId`. `admit`
classifies an already constructible case and changes `AdmissionId`, not
`RelationId`. A backend MAY push admission into enumeration, but the
optimization MUST preserve both identities and every population count.

The successor is an ordinary checked relation:

```text
successors(context, before) -> finite set of after states
```

It may contain zero, one or many After values. Duplicate canonical successors
under one source collapse. If two interventions reaching the same After value
are semantically different, their typed action identity belongs in Context;
producer provenance alone cannot keep them as different cases.

### Formal algebra

For one checked Explore declaration `e`, its admission `a`, each named question
`q`, and one Analyze graph:

```text
R_e       = distinct canonical (context, before) rows produced by FROM
C_e       = { transition(context, before, after)
            | (context, before) in R_e,
              after in successors(context, before) }
eval_a(c) = True | False | IntegrityError
D_a       = { c in C_e | eval_a(c) = True }
eval_q(c) = True | False | IntegrityError
Q_q       = D_a                                      when FIND q = ALL
Q_q       = { c in D_a | eval_q(c) = True }          when FIND q = MATCHES
Q_q       = { c in D_a | eval_q(c) = False }         when FIND q = VIOLATIONS
V_v       = relational view v over one explicit typed input
K_k       = choice relation k over one input QuestionId
M_r(T)    = differential signature incidence for explain request r and target T
S(r,m)    = { case in target(r) | M_r(case) = complete signature m }
P(r,m)    = projection_(Context, Before)(S(r,m))
A(r,m,s)  = { after | (s.context, s.before, after) in S(r,m) }
```

`IntegrityError` is disjoint from both Boolean values. It contributes a durable
error/closure obligation and cannot be interpreted as `False`; consequently it
can never mint a violation. Exact AdmissionId or QuestionId closure requires a
terminal classification for every member of its finite input or a certificate
covering the same complete set.

Source and successor collections have set semantics. Equal canonical
`(Context, Before)` rows collapse and union exact producer support. Equal After
values under one source do the same. Producer-path counts remain useful
diagnostics, but they are not case counts.

`S(r,m)` is the complete correlated case-support relation over source and
successor coordinates, `P(r,m)` is only its distinct `(Context, Before)`
starter-set projection, and `A(r,m,s)` is the dependent After-successor fiber
beneath one starter `s`. `S` therefore retains the correlation which `P`
deliberately projects away; `A` is a fiber of `S`, not an independently widened
After marginal. Projection can collapse several supported cases onto one
starter, so these populations have separate closure and count evidence. In
particular,
`|S(r,m)| = sum(s in P(r,m), |A(r,m,s)|)`; neither a starter count nor marginal
field bounds can be substituted for the case count.

An optimizer MAY evaluate one representative for a region whose members are
proved behaviorally equivalent, but it MUST retain an exact, disjoint support
certificate for that region. Such a quotient reduces evaluator and mechanism
discovery work without changing source, case, affected-profile or incidence
populations. A concrete representative without that certificate has support
one; it is never extrapolated to unvisited profile combinations.

The normalized total `admit` expression is the sole admission definition.
Reordering pure conjunctions or repeating an identical conjunct may remain
diagnostic source provenance but cannot change `AdmissionId`.

Result-view grain is explicit:

- `each case` preserves `CaseId` as row identity;
- `group all` forms one closed group;
- `group by [FIELD...]` forms canonical value groups;
- `measure` computes named exact scalars per input row;
- `aggregate` consumes a closed group, including `count_distinct`;
- `having` filters only after its required group reducers close;
- `select` is the public projection and privacy allow-list.

Choice is deliberately absent from view algebra. `choice` consumes a QuestionId
and separately declares its measures, partition, eligibility,
`one|all minimizing|maximizing` or `pareto` cardinality and objective semantics
under ChoiceId.

An observed optimum, group, Pareto member or distinct aggregate is provisional
until its required input and view frontiers close. The node's checked update
mode determines whether that prefix is exposed as revisable evidence or withheld
behind closure; final denotation is independent of scheduling and declaration
order.

The Analyze DAG is a finite, non-recursive stratified relational program.
Positive projection, join and total filtering may advance monotonically from
accepted facts. Negated selection, exact distinct aggregation, `having`, choice
and other closure-sensitive operators cannot derive a final negative or optimum
from mere absence in an open prefix. They consume the required input seal or
publish explicit revisions according to their update mode. No Prolog-style
negation-as-failure or implicit backtracking is part of the semantics.

### Semantic identities

The following layers MUST remain distinct:

**`RelationId`**
: Identity of the checked `from` plus `transition` relation: stable model and
  type owners, Context/State/Transition schemas, normalized ordered
  `given`/`vary`/`let` definitions and roles, intrinsic finite membership,
  successor semantics, set normalization and lineage contract. It excludes
  declaration names, admission, every `find`, Analyze nodes, schedules, worker
  counts, limits and journal order.

**`DerivationId`**
: Identity of one query-local `derive` declaration's normalized dependencies,
  pure expression and reachable immutable definition closure. DerivationId is
  a computed-column identity, not a relation, population, evidence root or
  publication authorization. Each use-site identity also binds the resolved
  DerivationId and its use-site domain obligations.

**`AdmissionId`**
: `H(RelationId, normalized total admit expression and dependency closure)`.
  The authored admission name is excluded.

**`QuestionId`**
: `H(AdmissionId, normalized total FIND predicate or ALL form, polarity)`.
  Every named `find` resolves to a QuestionId; names are excluded, so identical
  normalized questions alias the same ID. No question owns or renames the
  shared relation or admission.

**`ViewId`**
: Identity of one explicit QuestionId, ChoiceId or MechanismRequestId-incidence
  input plus grain, fields, measures, aggregates, filters, deterministic
  ordering, public update mode and display/privacy schema. It never embeds or
  feeds choice semantics.

**`ChoiceId`**
: Identity of an input QuestionId plus normalized measures, partition,
  eligibility, choice kind, objectives, direction, one/all-ties or Pareto
  cardinality, comparison semantics and deterministic tie ordering. A display
  view over the chosen relation has its own ViewId; changing that projection
  cannot silently rename the choice, and changing the choice cannot masquerade
  as a display-only edit.

**`MechanismRequestId`**
: Identity of one explicit QuestionId or ChoiceId target,
  canonical endpoint observer, reachable dependency closure and signature
  normalization. The target-language `explain ... using` declaration mints
  this identity. An explanation targeting a choice seals ChoiceId, not a
  mutable display snapshot.

**`AnalyzeGraphId`**
: Identity of the canonical set of reachable ViewId, ChoiceId and
  MechanismRequestId nodes plus their typed dependency edges and semantic root
  targets. Source order and declaration names are excluded. Publication readers
  and their authorization are separate and cannot rename this graph.

**`CoverageBundleId`**
: Identity of the applicable SourceCoverageId, SuccessorCoverageId and
  AdmissionCoverageId plus the canonical sets of reachable QuestionCoverageIds
  and AnalysisNodeCoverageIds. It scopes what a result may say about the
  declared population; it is not an extensional content root.

**`EndpointTotalityObligationId`**
: Identity of one MechanismRequestId's observer-totality claim over its target
  endpoint population. The obligation belongs to the Analyze graph; the method
  used to discharge it does not.

**`EndpointTotalityEvidenceId`**
: Evidence identity of one accepted static or extensional discharge. It binds
  the obligation, strategy/schema version, covered endpoint-domain roots,
  canonical proof or evaluation-receipt root and checked obligation count. The
  current implementation's `EndpointTotalityCertificateId` is the static
  subtype. No evidence ID renames MechanismRequestId or AnalyzeGraphId.

**Durable-evidence identity**
: Immutable relation, admission, questions, requested Analyze DAG,
  accepted endpoint-totality evidence, CoverageBundleId, evaluator, retention
  authorization and journal/serialization contracts.

**`EvidenceToken`**
: Opaque identity of one cohesive semantic snapshot. It binds the journal
  contract, analysis graph, coverage bundle, accepted semantic evidence
  root and exact semantic frontier roots. It is a read-consistency token, not a
  claim of freshness, closure or materialization.

**`ResumeCursor`**
: Operational continuation capability for one authenticated journal/checkpoint
  and work/materializer position. It is not interchangeable with EvidenceToken
  and is excluded from semantic answer identity.

**Operational records**
: Run-state path, worker count, time and resource limits, scheduler decisions,
  pressure events, pauses, resumes and resume/materialization cursors. They
  never redefine the bounded question or serve as evidence of exactness.

The selected explanation-bearing Analyze graph MUST seal the canonical sorted
set of `(MechanismRequestId, EndpointTotalityObligationId)` pairs. Its
`RelationalAnalysisPlanRoot` commits that graph, its reachable
Explore/Admission/Question/View/Choice identities and the chosen execution
strategy. The journal contract stores the checked AnalyzeGraphId and obligation
set. Genesis therefore never depends on which valid proof backend will
eventually discharge an obligation. Accepted static or extensional totality
evidence enters the semantic evidence root. Resume MUST revalidate the journal
contract, registered plan/root and every proof artifact on which already
accepted semantic evidence depends. Operational limits and scheduler policy
remain outside semantic identities; proof strategy adds no field to raw
signatures or structural mechanism identity.

Explore, admission, question, Analyze-node and publication-reader names are
unique source addresses, not semantic hash inputs. Renaming a node and updating
its references preserves its semantic identity.

`RelationId` seals semantics, not materialization strategy. A
`RelationFrontierRoot` commits the currently discovered canonical rows,
provenance and every open/sealed producer frontier without asserting
completion. Once all source and successor frontiers seal, a distinct
`RelationContentRoot` commits the completed extensional relation. An eager
relation and an incremental producer with equal semantics share one
`RelationId` and MUST converge to the same completed content root.

A certified symbolic partition may close a relation or a downstream answer
without materializing every `CaseId`. Its arrival-order-independent
`EvidenceRoot` commits the exact support expressions, disjoint-union proofs,
classification or observer certificates and remaining open obligations. A
symbolic implementation MUST NOT claim the extensional `RelationContentRoot`
unless it has actually derived the canonical extensional set commitment. Thus
two implementations with the same materialized relation converge to one
content root, while an unmaterialized exact answer may legitimately expose an
evidence root and exact counts without pretending to have enumerated that set.

The source planner is factorized by the checked dependency graph. Finite
`vary` bindings are factors or dependent fibers; `given` and `let`
Context/Before construction is a deterministic map and never a cardinality
multiplier. A runtime fiber is cached by exactly the earlier binding values named by its
dependency edges, not by the complete prefix. Transition construction starts
from distinct source rows after projection normalization, so auxiliary producer assignments cannot
duplicate cases. Interval splits may be lifted through an unchanged Cartesian
factor product, and into a mapped case image only when injectivity or an
equivalent disjoint-image proof has been accepted.

The stable row identities are:

```text
SourceKey    = H(RelationId, canonical Context, canonical Before)
SuccessorKey = H(RelationId, SourceKey, canonical After)
CaseId       = H(RelationId, SourceKey, SuccessorKey)
```

They do not contain discovery rank or a mixed-radix ordinal. One canonical
Context/Before/After triple has one CaseId within a RelationId. A global
`TransitionId` identifies the same extensional typed triple independently of
RelationId. Consequently CaseId-to-TransitionId is injective inside one
relation; the same TransitionId may recur across relations. Distinct transitions
may share a mechanism signature without collapsing their cases.

### Counts and closure

The primary relation populations are:

- `U_S`: distinct constructible source rows;
- `U_C`: distinct constructible source/successor cases;
- `D_C`: cases admitted by an `AdmissionId`; and
- `S_C(q)`: cases selected by each `QuestionId q`.

The full semantic graph calls each selected/question-matching layer `M(q)`;
within one question `M_C(q)` is the same population as `S_C(q)`. It records the
shared `U` and `D` support relations and one explicit `(TransitionId, CaseId)`
`M(q)` relation per QuestionId. This makes the scope of every transition count
auditable without cloning the relation or deriving it from an arbitrary
retained case cap.

For `find q = all`, `S_C(q) = D_C`. Inside one RelationId the corresponding extensional
transition counts are conservation equalities: `U_T = U_C`, `D_T = D_C` and
`S_T(q) = S_C(q)`. Cross-relation reports MAY additionally count global distinct
TransitionIds but MUST name that larger scope.

A count is `lower_bound(n)` while any required source, per-source successor,
classification or view frontier remains open. It is `exact(n)` only after those
frontiers close. Raw-signature counts have an independent request-relative
incidence frontier; structural-mechanism and execution-profile counts also
depend on quotient assignment and closure. Before any confirmed replay evidence
the honest value at each grain may be unknown rather than zero.

For a mechanism request, successful replay defines a partial explanation map
`mu: target cases -> signature`. The case-support fiber of signature `m` is
`S_m = { case | mu(case) = m }`. One confirmed incidence proves
`lower_bound(1)` for that fiber; `exact(1)` additionally requires every target
case or certified weighted cell to be terminal and no second member of `S_m`.
Implementations SHOULD represent evolving support as disjoint concrete
witnesses, disjoint certified uniform cells and a residual frontier. Count
knowledge forms a narrowing interval over extended natural numbers: accepted
evidence may increase its lower bound, coverage/refutation proofs may decrease
its upper bound, and equality yields `exact(n)`. Distributed merges MUST use
stable case IDs or disjoint cell proofs rather than adding overlapping lower
bounds.

Every case is a `(Context, Before, After)` triple, so a signature also has a
request-relative starter projection and a per-starter successor fiber:

```text
P_m          = { (context, before) | exists after: (context, before, after) in S_m }
A_m(source)  = { after | (source.context, source.before, after) in S_m }
|S_m|        = sum(source in P_m, |A_m(source)|)
```

`P_m` is the complete raw signature's starter support: the initial worlds from
which that replay explanation can occur under the declared transition. It MUST
remain separate from the normalized signature definition. Structural mechanism
support is the deduplicated union of the raw-signature fibers assigned to that
`StructuralMechanismId`. Structure says what happened; starter support says
where that same structure was observed or proved possible. A support annotation
is therefore scoped by the mechanism request and raw signature or structural
subject. Starter values MUST NOT be hashed into a mechanism node merely to
encode support; a value which changes rule selection will already change the
checked differential signature, while a value which does not may legitimately
enlarge the same case-support fiber.

Every published starter-support projection MUST retain the source manifest's
distinction between varied, derived, explicitly conditioned, proved-irrelevant
and unsupported dimensions. If a request fixes `commune = Copenhagen`, that
singleton is evidence about the question's conditioning, not proof that the
mechanism cannot occur in another commune. Only variation or a checked theorem
over a wider declared source relation can establish such an exclusion.

Starter support MUST preserve dependencies between dimensions. Independent
per-field minima and maxima are only a lossy summary: their Cartesian box may
contain starting states which never reach the mechanism. The proof-carrying
form is a union of correlated typed support cells or an equivalent checked
predicate. During an open run it may be represented by inner and outer
approximations `P_m^- subseteq P_m subseteq P_m^+`; the unresolved starter
frontier is `P_m^+ \\ P_m^-`. Marginal bounds may be published for navigation,
but MUST be labeled as projections and MUST NOT be multiplied to obtain a case
count without a product/disjointness proof.

Every validated structural mechanism, internal mechanism-DAG node and edge
definitionally induces a request- and target-conditioned starter activation
support (its source preimage). An implementation MAY materialize or publish
that view on demand; the support semantics do not depend on eager storage. It
answers where the subject was reached in the mechanism request's explicitly
named `find` or `choice` target population. Targeting every admitted case uses
a named `find NAME = all`; admission itself is not an Analyze input. The
support is not by itself a global trigger predicate or weakest precondition for
the underlying rule.

Two coordinate systems MUST remain distinguishable. The starter support above
is an **origin preimage** over the exploration's original `(Context, Before)`:
it answers which whole starting worlds eventually reached the subject. A node
MAY additionally expose **local-entry support** over the checked values and
bindings present at that node's evaluation frame. Local-entry support can make
the immediate guard or threshold easier to inspect, but it requires retained
frame evidence and is not interchangeable with the origin preimage. Neither
support belongs in `StructuralNodeId`; both are conditioned overlays, and
neither alone proves a universal trigger condition without a separate coverage
or preimage proof.

Support facets MUST be named rather than conflated. Activation support says
that a node was traversed. Differential-participation support says that its
Before/After presence or outcome participates in the differential signature.
A future causal-responsibility or sufficiency relation would require stronger
counterfactual evidence. A rule visited with the same outcome at both endpoints
may belong to the first support without belonging to either stronger relation.
All of these node supports can overlap whenever one case traverses more than
one node, so they are useful for explaining starter conditions and shared
submechanisms but are not additive case or mechanism counts. Exact counting
remains grounded in disjoint complete-signature fibers or certified partitions
of them.

More precisely, let `r` be a mechanism request, `t` one of its named target
populations, `f` a declared support facet, `pi(m)` the structural-mechanism
quotient of complete signature `m`, and `N_f(pi(m))` the nodes participating in
that facet. Complete-signature leaves remain the disjoint authority from which
the coarser supports are derived:

```text
S(r,t,g)   = disjoint_union(m where pi(m) = g, S(r,t,m))
S_f(r,t,n) = disjoint_union(m where n in N_f(pi(m)), S(r,t,m))
P(r,t,g)   = distinct projection_(Context, Before)(S(r,t,g))
P_f(r,t,n) = distinct projection_(Context, Before)(S_f(r,t,n))
A_f(r,t,n,s) = { after | (s.context, s.before, after) in S_f(r,t,n) }

|S_f(r,t,n)| = sum(s in P_f(r,t,n), |A_f(r,t,n,s)|)
```

The same `n` notation covers a structural node or edge with the corresponding
membership relation. A whole structural mechanism has facetless support;
activation and differential participation qualify its internal nodes and
edges, not two aliases of the same mechanism-level set. The unions of
confirmed complete-signature leaves are
disjoint in case space even while a request is open because signature
assignment is functional; the unresolved target residual simply remains
outside those unions. Starter projections are not necessarily disjoint: two
different cases, or two different signatures containing the node, may project
onto the same starting world. A subject's exact starter count therefore
requires projection and deduplication evidence even when all contributing
signature case counts are exact. The stable overlay key is the request, its
resolved target identity, and a structural subject; a node/edge subject embeds
its named facet, while a whole-mechanism subject is facetless. The eventual
target seal is closure evidence attached to that same stream, not a new view
identity. An overlay MUST NOT clone the subject merely because another starter
reaches it.

The bounds on a subject are set bounds before they are scalar count bounds.
For any structural subject `x`, let `S_x` be its true correlated target-case
support, `S_x^-` the union of disjoint case atoms already proved to contain
`x`, and `S_x,known^+` the union of concrete atoms still able to contain `x`.
Those symbols denote case-set denotations over `(Context, Before, After)`, not
merely their cell descriptors and not independent endpoint marginals.

An open target may additionally have an opaque undiscovered-target obligation
`omega`. That token is not itself a case set. Treat the upper bound as the
abstract value `Uhat_x = (S_x,known^+, omega)` with a concretization
`gamma(Uhat_x)`:
the family of concrete case relations consistent with the known outer atoms
and the still-undiscovered target. The sound open statement is therefore

```text
S_x^- subseteq S_x
S_x in gamma(Uhat_x)
P_x^- = distinct projection_(Context, Before)(S_x^-)
P_x   = distinct projection_(Context, Before)(S_x)
A_x^-(source) = { after | (source.context, source.before, after) in S_x^- }
A_x(source)   = { after | (source.context, source.before, after) in S_x }

P_x^- subseteq P_x
A_x^-(source) subseteq A_x(source)
```

Projection and deduplication happen after case-space bounds are derived. An
exact case-atom weight therefore does not by itself establish an exact starter
weight. When `omega` is absent, write `S_x^+ = S_x,known^+`, define
`P_x^+ = distinct projection_(Context, Before)(S_x^+)` and derive the
corresponding per-source `A_x^+` from that same correlated case relation.
Projection monotonicity then gives
`S_x^- subseteq S_x subseteq S_x^+`,
`P_x^- subseteq P_x subseteq P_x^+` and
`A_x^-(source) subseteq A_x(source) subseteq A_x^+(source)`. These are
correlated regions or checked predicates, not independent per-field boxes.
When `omega` is present, projection is lifted through `gamma`: the upper
starter-support projection, successor fibers and their counts are `top`/unknown
unless a separate checked concrete envelope exists. An empty currently
discovered residual does not discharge that obligation.

A shared `StructuralNodeId` view intentionally unions support across every
structural mechanism containing that node. A presentation which asks for the
node *inside one displayed mechanism* MAY instead derive the intersection
subject `(StructuralMechanismId, StructuralNodeId, facet)` (and analogously for
an edge). That contextual view does not change either structural identity. For
a complete fixed execution graph it may coincide with the whole mechanism's
support; the explicit intersection becomes important when explaining a shared
node across several mechanisms.

The total case support of a node or edge and its route-conditioned case-support
fibers are different views over the same case authority. Let `c` be a checked
structural condition such as an owning `StructuralMechanismId`, one incident
`StructuralEdgeId`, or one canonical structural path segment. Then

```text
S_f(r,t,x | c) = { case in S_f(r,t,x)
                 | the case's validated structural assignment satisfies c }
P_f(r,t,x | c) = distinct projection_(Context, Before)(S_f(r,t,x | c))
A_f(r,t,x | c,s) = { after | (s.context, s.before, after)
                              in S_f(r,t,x | c) }
```

`S_f(r,t,x)` is the total subject support, while the conditioned relations
explain which incident edge, enclosing mechanism or path brought cases to that
subject. A complete family of route conditions unions to the total support, but
its case-support fibers are not necessarily disjoint: one case may contain
several incident edges or paths reaching the same shared node. Their starter
projections can overlap even when their case-support fibers do not.
Implementations MUST therefore deduplicate a requested union and MUST NOT sum
edge/path-conditioned counts without a checked partition proof. Route
conditions remain support-overlay keys; they never clone or rename the
structural node.

The resulting scalar populations must be named by grain:

- `distinct_starters(x) = |P_f(r,t,x)|` counts origin `(Context, Before)` rows;
- `cases(x) = |S_f(r,t,x)|` counts supported cases and, within one
  `RelationId`, the same number of extensional transitions; and
- `subject_incidences = |{ (case, x) | case in S_f(r,t,x) }|` counts
  case-to-subject memberships. Summing per-subject case counts computes this
  overlapping incidence population, not distinct cases and not mechanisms.

This disjoint formula relies on the complete canonical signature assignment
being a function. If a future explanation layer admits several alternative or
minimal causal explanations for one case, those alternatives form an
overlapping incidence relation. They MUST NOT replace the complete-signature
partition authority or be summed as though they were disjoint.

Storage SHOULD remain factorized at the complete-signature leaves. For each
`(request, signature)`, retain a source-key map to distinct successor keys (or
certified uniform case cells); this directly supplies the signature's case
count, distinct starter count and successor fibers. A validated structural
quotient contributes immutable inverted indexes from structural mechanism,
node and edge IDs plus facet to the signatures containing them. Node/edge
support is then a virtual union of only its relevant signature leaves. Repeating
a node inside one execution does not add another supported case; its occurrence
multiplicity belongs to the execution profile. This avoids both an
`O(cases * mechanism_nodes)` incidence table and an `O(nodes * all_signatures)`
scan at publication.

Any fully unioned subject projection is a derived accelerator, not retained
authority. Such caches MUST be bounded or evictable: asking once for every
node and edge must not gradually materialize an `O(cases * subjects)` table in
memory. Eviction changes only cold-query cost. A later observation rebuilds
the same case, starter and conditional-successor roots from the immutable
signature fibers. Large exports MAY instead page or stream that union without
installing it in the hot cache.

The structural-definition catalog MUST publish stable, value-free slice
descriptors for every whole mechanism and every activation/differential node
and edge. A descriptor is only an address. Publication MUST NOT turn the
catalog into an eager all-subject result or build one complete union after
another: evicting between subjects bounds retained accumulation but still lets
one ubiquitous node allocate `O(target cases)`.

Every discovered structural mechanism automatically registers its facetless
total-support slice in the **automatic core scheduler**. Importing a structural
assignment or terminal signature fiber dirties only the affected mechanism;
several changes may coalesce before its next observation. Advancing some other
mechanism's frontier MUST NOT rewrite or re-observe older points. Each accepted
point is an immutable description of its exact journal prefix and contains an
authenticated factorized summary over contributing signature fibers and the
shared residual. Disjoint fiber weights give case bounds directly. Before
cross-fiber starter deduplication, the largest inspected single-fiber starter
count is a safe lower bound and the sealed target starter count is a safe upper
bound. One point scans at most 256 canonical fiber summaries; a capped scan
widens bounds and MUST NOT fall back to a full union.

The point MUST label its starter projection `not_materialized`; a factorized
summary root is not a materialized correlated-content root. Its compact schema
publishes two independently domain-separated inner/outer root pairs:

- `case_support.inner_root` and `case_support.outer_root` denote `S_x^-` and
  the corresponding concrete `S_x^+` or opaque upper-support expression. Their
  coordinate contract is
  `SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>`, so each source's
  `A_x(source)` remains a dependent successor fiber rather than an After
  marginal.
- `starter_support.inner_root` and `starter_support.outer_root` denote the
  canonical distinct-source projection `P_x^-` and the projection expression
  over the corresponding concrete or opaque case-support upper. The latter is
  `P_x^+` only when it denotes a concrete envelope. Both are expression
  identities even when no starter rows are materialized; they MUST NOT alias
  the case-support roots or be replaced by per-field minima/maxima.

An outer root may commit a shared possible-support residual and an opaque target
obligation; it is then a stable expression identity, not a claim that a finite
outer set has been enumerated. Within each domain the outer expression may
normalize to the inner identity only when neither residual can add support.
The point also publishes `starter_set_status` and
`correlated_support_status`. The former may become `exact_starter_set` once
`P_x^- = P_x^+`; the latter may become `exact_correlated_support` only when
`S_x` and every dependent `A_x(source)` are closed. These statuses are explicit
and MUST NOT be inferred from root equality or scalar counts alone. The roots
preserve correlation semantics without exposing typed values or authorizing
cells. The point separately names an authorization-neutral
`projection_plan_id`, not a public cell job.

Once request support closes, the automatic scheduler performs a lazy canonical
seal sweep. The core support receipt is withheld until its registered, observed
and sealed slice counts all equal the exact structural-mechanism count. Only
this automatic registry, its observation count and its automatic chain root
authorize that receipt. Explicit readers MUST NOT change or delay it.

Node/edge slices and route-conditioned slices enter a separate **explicit
extension scheduler** through checked `observations` demands. Registration is
anchored to the exact durable support cursor and frontier root, freezes the
current structural-assignment prefix, and backfills that prefix in canonical
pages of at most 256 assignments. A partial backfill is never observable. On
completion, subject/route and matching-signature watchers make the slice ready;
later assignment or terminal evidence dirties only ready incident slices. A
demand registered while support is open may emit open points and later one
sealed successor. A demand registered after durable support closure may emit a
sealed point first; it MUST NOT invent an open predecessor. A whole-mechanism
demand which names an automatically registered slice aliases that core slice
instead of installing a second scheduler entry.

Registration, bounded backfill and point acceptance are separate journaled
facts with authenticated prior/next scheduler summaries. Replay MUST reproduce
the same disposition, phase, cursor range, summary roots and point. Explicit
completion is invocation-local reader completion: it does not reopen the core
analysis or enter its semantic closure roots. All automatic and explicit points
share one append-only request observation chain, while per-lane scheduler roots
and counts remain independently reportable. No lane constructs a
`cases * DAG subjects` table.

Exact correlated materialization is a separate, content-addressed projection
job whose identity is derived from that plan plus checked publication
authorization. In the target language, the correlated relation and its
deduplicated projection have deliberately different consumers:

```runa
materialize support cases cliff_node_cases
    from explain cliff_paths
    for node differential "<StructuralNodeId>"
    using values from view cliff_cases

materialize support starters cliff_node_starters
    from support cases cliff_node_cases
```

`materialize support cases` enumerates `S`; `materialize support starters`
projects only `P` from that exact correlated source. They have different member
schemas, counts, roots, cursors and materialization statuses even when every
starter happens to have one successor.

Publication v15 does not implement that target split. It retains the explicit
single-subject `starters` consumer introduced in publication v9 as transitional
historical executable syntax:

```runa
starters cliff_node_cases
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
using values from cliff_cases
```

For a node or edge, an optional enclosing-mechanism route refines that same
subject without changing its structural identity:

```runa
starters cliff_node_cases_in_path
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
within mechanism "<StructuralMechanismId>"
using values from cliff_cases
```

Semantically, total subject support is the deduplicated union of all indexed
signature fibers for the subject. The qualified form intersects that signature
index with the named structural mechanism's signature index before paging the
same correlated `SourceKey -> Set<SuccessorKey>` fibers. The selector MUST be
bound into consumer, projection-plan, job, cursor/checkpoint and public-record
identity, while RelationId, QuestionId, MechanismRequestId and the structural
subject IDs remain unchanged. Two route slices may overlap; their counts MUST
NOT be added without a checked disjointness proof.

The qualified v13 artifact uses the unified subject-starter record schema v3
and adds the route to its cursor identity. Despite that historical schema and
consumer name, each member is an `S` case-support member carrying one
source/successor pair; it is the implementation precursor of target `support
cases`. A genuine target `support starters` member contains only the
deduplicated `SourceKey`. The optional cursor field is omitted for an
unqualified consumer. Publication v9 first established the additive consumer
model; under the current Experimental v15 plan, either historical form still
attaches without renaming or reopening the core analysis, but neither v9 bytes
nor the misleading `starters` spelling are a compatibility target.

The selector is exactly one structural mechanism, activation/differential
node, or activation/differential edge, optionally refined by one enclosing
mechanism for node/edge subjects. This selector surface deliberately has no
wildcard, list, or arbitrary path predicate: a declaration cannot accidentally
authorize a DAG-wide case-by-subject export. `using values from` is mandatory
and must name
a prior compatible lossless selected-input, each-case view which directly
exposes `case_id`, `context`, `before` and `after` without aggregation,
`having`, or choice. For a chosen-view mechanism request, that receipt
authorizes the selected population from which the same `QuestionId`'s chosen
target was derived; the choosing view itself need not be lossless.

The declaration lowers into a publication-consumer graph beside, not inside,
the answer-defining analysis DAG. Its checked ID binds its authored name,
request, structural subject/facet, optional enclosing mechanism and authorizing
semantic ViewId. The
canonical consumer-set identity is declaration-order-independent. Adding a
new consumer to a completed stream leaves RelationId, QuestionId,
MechanismRequestId, the analysis-graph root and journal head unchanged; cursor
reconciliation may append only a new content-addressed subject artifact and
must reject removal or rebinding of an already owned artifact.

Each published member retains its raw signature ID, `CaseId`, `SourceKey`,
typed `Context` and `Before`, `SuccessorKey`, and typed `After`. The job
merge-deduplicates the selected support slice's canonical signature fibers into immutable
pages of at most 64 members, using a key-based source/successor cursor and a
page-boundary-independent semantic root. Before append, the publisher
adaptively shortens a candidate page until its encoded NDJSON record fits the
configured `max_line_bytes`; if one member alone cannot fit, publication fails
explicitly rather than dropping or truncating that member. Its checkpoint
authenticates the exact input authority, output prefix and completed page
manifest. The current per-mechanism k-way merge retains one candidate per
contributing raw signature plus the current page, so peak memory is
`O(contributing signatures + 64 members)`, not the subject's case count. A
fixed-fan-in external merge remains a future scaling step for mechanisms with
very many contributing signatures.

The compact observation lane and the typed materializer close independently;
each authored v13 typed subject artifact has its own resumable cursor and
closure at the historical path `starters/<consumer>.ndjson`. Its header binds the request, relation, admission,
question and resolved target identities; exact subject/facet and optional
route; checked State, Context and transition schema identities; FROM-coverage
manifest digest; projection plan/job; value authorization; and the separate
case-support and starter-projection roots. Bounded typed pages retain the
correlated values above; the header labels materialization `pending`, and each
page's `exhausted` flag makes resumable progress explicit. Its closure labels
materialization `materialized` and certifies the exact case count,
distinct-starter count, typed content root, `starter_set_status` and
`correlated_support_status`. Compact scheduled observation points continue to
label `starter_support.materialization` as `not_materialized`: authoring one
selected consumer does not turn every node and edge into an eager artifact.
Arbitrary path-conditioned selectors remain future work.

Those signature leaves form a disjoint target-partition atom set: an atom is
either one concrete CaseId singleton or one certified uniform `SupportCell`,
never both for the same support. Materialized witnesses sampled from a uniform
cell are examples or projection caches and MUST NOT contribute support weight
again. Thus the concrete source/successor fiber map is complete authority on an
extensional branch, while on a symbolic branch it is only a cache beside the
atom partition and its receipts.

Open support bounds are derived from facts, never stored as freely mergeable
scalars. `inner(X)` contains assigned signature leaves and uniform cells proved
to contain subject `X`; `outer(X)` additionally includes the factorized
unresolved target frontier not yet proved unable to contain `X`. Case-count
knowledge and distinct-starter-count knowledge are separately typed as
`unknown(lower)`, `interval(lower, upper)` or `exact(n)` because projection may
collapse cases. A saturated counting cap is an interval with an unproved upper
side and MAY be rendered `at_least(lower), censored`; `at_least` is not a
fourth evidence kind. The stream MAY attach `possible_subjects` constraints to
a residual cell, but MUST NOT clone the whole residual frontier into every
node. One shared residual partition/root is referenced by every subject view;
only an explicit export may materialize its projection. A support view exists
before target closure: the undiscovered remainder is an opaque obligation, so
its counts are `unknown(confirmed_lower)` even when all currently known cases
have replayed. Attaching the exact target seal to that same view removes the
opaque remainder and permits finite intervals. An `Unavailable` replay terminal
is not proof that any structural subject was absent, so it remains in the
shared possible-support residual and prevents
unsupported exactness; its reason also remains visible in the request's
separate unavailable count. Exact structural/node/edge starter support requires
an exact target seal, validated structural membership and an exact deduplicated
starter projection. A residual need not prevent an exact starter *count* when
checked projection evidence proves that it cannot introduce a new starter—for
example, every starter in the sealed target is already in the confirmed inner
projection. The same residual can still prevent exact case support and exact
conditional After fibers. Raw mechanism-incidence closure alone does not close
this later quotient/support layer.

Proof states MUST therefore distinguish `exact_starter_set` from
`exact_correlated_support`. Target-starter saturation may establish the former
while unresolved cases can still add successors beneath already-known
starters. Only closure of those successor obligations can label the correlated
`(Context, Before) -> Set<After>` case-support root exact. Public records MUST
carry that distinction in the separate `starter_set_status` and
`correlated_support_status` fields rather than asking consumers to infer it
from counts.

The concrete residual SHOULD remain factorized as pending cases, unavailable
cases and a manifest of complete signature fibers whose structural assignment
is not yet validated. Each factor and signature fiber is incrementally
authenticated by a canonical map. Accepting a structural assignment then
removes one signature-manifest entry regardless of that fiber's case count; it
MUST NOT rebuild or delete every case from a flattened residual merely to
produce a new root. Case weights are additive across these disjoint factors.
Distinct-starter weights are not: an exact starter projection is a separate,
resumable union obligation over the relevant source fibers. Until that
obligation closes, a view may publish a conservative finite upper bound from
the sealed target's distinct starters. That value MUST be labeled a
`conservative_target_projection_upper`: it is an envelope around `P_x^+`, not
an exact materialization of the possible starter-support set, and it MUST NOT
make the projected set exact.

Distributed merges union stable terminal, signature-fiber and membership facts
idempotently and reject conflicting terminals or quotient roots. They recompute
outer bounds from the merged unresolved partition; they do not add serialized
lower bounds or union independently projected scalar summaries. A compact
structural-quotient closure binds the raw incidence root, quotient version,
signature-to-quotient root and structural membership root. A support-view
closure additionally binds both inner/outer case-support roots, both
inner/outer starter-projection roots, and the two explicit support statuses.

Operational support-frontier checkpoints SHOULD be sparse—at pause, explicit
checkpoint or report boundaries—rather than emitted after every case. Their
branch revisions and cursors belong to resumability, not semantic answer
identity. Final structural-quotient and support closures are semantic evidence;
checkpoint cadence is not. A checkpoint MUST name the exact raw target-
discovery cursor it has imported. Replay catches up only through that cursor,
not through whatever larger raw prefix happens to be visible later, and then
rederives the claimed frontier root. Large catch-up is split into bounded
cursor quanta; `SupportClosed` requires the complete target cursor and a
matching durable frontier checkpoint, so finalization cannot hide one
uninterruptible `O(target cases)` pass.

Support geometry is independent of scalar cardinality. A result MAY describe a
fiber as a union of typed cells that are bounded on some dimensions and
invariant or unresolved on others. In the current finite Explore contract, an
axis reaching its declared boundary is censored support, never proof of an
unbounded or infinite fiber. A future parameterized theorem layer may report an
unbounded axis or infinite cardinality only with a verifier-accepted region or
injective-family certificate; observing more cases or reaching a cap is
insufficient.

A genuine support-counting cap may publish `at_least(c)` and MUST identify the
censored signatures. A retained-example or display cap does not weaken an exact
scalar count. No cap proves infinity: every support in a finite Explore relation
is finite. If a cap stops signature assignment itself, both signature count and
incidence remain open.

A user-facing mechanism-loss histogram counts distinct
`StructuralMechanismId` values with support in each declared interval, not
cases or raw replay signatures. The same structural mechanism may occur in
several intervals, so bin counts need not sum to the global structural-
mechanism count. A separate raw-signature or `ExecutionProfileId` histogram MAY
show execution-sensitive variation but MUST say so in its field and view name.
A structural bin is exact only after selected membership, loss measurement,
raw-signature assignment and incidence, structural quotient assignment and bin
membership all close.

### Durable observable execution

The append-only journal is the recovery authority. It commits content-addressed
source rows, successors, classification records keyed by their semantic IDs,
view reductions, mechanism incidences, proof receipts and exact frontier
transitions. One transaction MAY co-commit a case and classifications for the
current question, but the records remain separately identified so a later
authorized AdmissionId or QuestionId can classify the same CaseId without
rewriting it.

Mechanism structure, incidence and support region are likewise separate record
families. A signature-definition record contains only normalized causal
structure. An incidence assigns one exact case to that signature. A
request-local mechanism-support record may then bind disjoint case cells,
separate inner/outer `S` case-support roots, separate inner/outer `P`
starter-projection roots, dependent `A(source)` fiber semantics, projection
receipts, explicit starter-set/correlated-support statuses and count intervals
to the signature.
The mechanism-support frontier belongs to the analysis layer: it MUST NOT
reopen the base relation/classification support catalog after that catalog has
sealed, and base-support closure MUST NOT require a future mechanism assignment.
This avoids a lifecycle cycle while allowing uniform-mechanism cell proofs to
arrive incrementally after FIND has identified the target population.

A complete mechanism signature definition MUST be content-addressed and
interned once per mechanism request. Each successful case incidence then binds
the request, case, transition, endpoint trace roots, signature and replay
receipt without embedding another copy of an already interned definition.
Journal replay MUST collision-check the referenced definition before accepting
the compact incidence. Chunking and pause/resume MAY split either artifact but
the semantic journal MUST preserve definition-before-reference order. A public
answer view MAY publish a content-addressed descriptor, incidence and exact
closure while an independently cursored canonical-payload sidecar is still
catching up, provided its manifest exposes that incomplete payload frontier and
never claims reconstructable definition availability early. A sidecar prefix
MUST be bounded by descriptors already committed in the companion answer view;
it cannot make an undisclosed signature payload observable first. Once caught
up after request closure, the sidecar MUST close with the exact signature count
and incidence root so its completeness is independently distinguishable from an
open prefix. This normalization is semantic storage hygiene, not merely
compression: the durable mechanism DAG is shared structure, while case
incidence is a separate edge relation.

The journal head authenticates operational event order. The evidence root is a
canonical arrival-order-independent commitment to accepted semantic evidence
and the exact open frontier. Snapshots, JSON, search DAGs, transition graphs and
reports are authorized materialized views, not recovery authority.

Every cohesive public read MUST carry an opaque `EvidenceToken` which binds one
semantic snapshot:

```text
EvidenceToken = H(
    JournalContractId,
    AnalyzeGraphId,
    CoverageBundleId,
    RelationalCoreEvidenceRoot,
    canonical semantic node/frontier roots
)
```

All rows and linked artifacts presented as one report or observation revision
MUST either carry the same EvidenceToken or declare an explicit earlier-token
dependency. “Latest” reads from several files are not a cohesive snapshot. A
later token means more refined accepted evidence, not necessarily an exact or
materialized answer; the orthogonal statuses below remain authoritative.

`ResumeCursor` is a separately typed operational capability binding the journal
head, checkpoint root, work position and any materializer cursor needed to
continue. It is never accepted where EvidenceToken is required and never proves
semantic closure. Conversely, EvidenceToken identifies what was known, not
where a worker should resume. Presentation pagination cursors are a third type
and confer neither authority.

Journal frames are typed as semantic evidence, resumability checkpoints or
presentation records. Source/case membership, classifications, support proofs,
result evidence and mechanism incidence are semantic. Work selection,
readiness, completion references and materializer cursors are checkpoints.
Retained examples are presentation. All three may be authenticated by the same
ordered chain, but only the first class contributes to answer identity.

No answer-defining terminal frame may grow in proportion to the population it
closes. Membership and projection evidence is appended in bounded records or
chunks; exhaustion and publication then use compact typed roots plus exact
counts. In particular, source exhaustion commits the prior fiber/source/edge
sets rather than embedding their identity arrays, and a result view journals
bounded row, group-header and chosen-row projection records before a compact
result-root closure. Mechanism signatures, endpoint traces and replay evidence
likewise use bounded canonical artifact records followed by a compact typed
closure; a trace-size byte string is not a valid single frame. Replay MUST
rederive those commitments from the preceding records before accepting the
seal.

Fresh mechanism replay MUST represent checked call paths as a bounded
parent-linked activation trie and MUST let occurrence roots and dependencies
refer to bounded local IDs until validation. Producer IDs are operational only:
normalization MUST retain the complete activation trie, including eventless
and endpoint-only calls, validate parent order/depth and contiguous
invocation/visit ordinals, assign prefix-first content-canonical path and
occurrence ranks, and only then derive endpoint and signature identity. Those
eventless activation nodes are execution anchors: removing them before pairing
could make the same ordinal identify different actual invocations at Before
and After. A canonical or durable form MUST encode each activation node once
rather than repeating the full path in every occurrence. Relevance slicing and
structural quotienting happen only after exact endpoint pairing and preserve
the anchor multiplicities in the execution profile. Internal pure higher-order
builtin calls MUST bind callbacks to their exact checked argument site and
callable/rule target; an expected activation frame that is not consumed fails
closed.

Every cold `RuleFamily` activation MUST consume an exact prefix of the checked
family's exception, conditional-default, clause and unconditional-default
candidates in authoritative dispatch order. Each retained attempt binds the
next full checked candidate identity and one structurally possible outcome. A
`Selected` closure is valid only for the immediately preceding `Applicable`
attempt; `NoApplicableRule` is valid only after the complete checked family has
been attempted without an applicable candidate. A skipped suffix after
selection is deliberately absent. The trace producer MUST reject an activation
that exits with a pending candidate or without one coherent selection.

Within one fresh endpoint replay, a non-evicting scope-local semantic memo MAY
retain a completed rule-selection occurrence together with its value. A later
equivalent source call MUST still enter a fresh checked activation and emit its
own selection occurrence, but MAY make that occurrence depend directly on the
original completed selection instead of re-evaluating or cloning the original
causal subtree. Such references MUST remain inside the same dynamic scope and
endpoint trace, preserve the same rule family and typed selection outcome,
point directly to the cold original rather than form a reuse chain, and be
installed only after the cold selection completed without trace or evaluation
failure. `RuleSelection` denotes the certified semantic selection outcome of
an invocation, including an invocation resolved by this reference. Memo keys,
hit counters and runtime addresses are operational and MUST NOT enter canonical
mechanism bytes; the deterministic reference topology remains part of the
replay ABI.

The exact execution-occurrence DAG and the structural mechanism graph are
different objects. The former is replay evidence and retains every invocation,
visit and dependency occurrence. The latter is a declared quotient of already
validated Before/After evidence: checked call-site/callee skeletons, semantic
event sites, event kinds, endpoint presence/outcomes and endpoint-coloured
dependency topology define structural nodes; invocation ordinals, visit
ordinals, repeated helper/loop visits and memo scheduling contribute exact
multiplicity to an execution profile instead of manufacturing new policy
mechanisms. Quotient construction MUST preserve and verify the Before/After
node, root and edge totals through class and edge multiplicities.

Structural ownership is context-sensitive without being invocation-sensitive.
An activation context hashes its invocation-erased parent context and checked
call-site/callee frame. Thus the same helper event beneath two different policy
roles may share a `StructuralNodeId` while the complete mechanisms retain
different node-to-context ownership facts. Repeated invocations of the same
static context contribute profile multiplicity rather than new context IDs.
After exact V3 pairing, activation contexts which neither own nor ancestrally
locate a retained event are sliced from structural identity; every such
eventless anchor remains explicit in raw membership and the execution profile.
An empty-event trace therefore has an empty policy mechanism rather than one
mechanism per incidental helper-call skeleton.

This equivalence is versioned semantic identity, not transparent compression.
Until its validator and canonical encoding are complete, the current exact
occurrence signature remains authority and a quotient is only a derived
diagnostic/view. The intended identities are:

```text
StructuralMechanismId = hash(versioned counted structural topology)
ExecutionProfileId    = hash(StructuralMechanismId, exact endpoint multiplicities)
```

The profile is an exact aggregate multiplicity account, not a replacement for
the raw execution topology. A separate membership commitment assigns every
canonical raw activation and event occurrence exactly once. The replay V3
signature remains the authority for invocation-specific ancestry and order.

User-facing distinct-mechanism counts SHOULD use `StructuralMechanismId` so the
same checked policy decision does not become a second mechanism merely because
it was invoked eleven times instead of ten. Count-sensitive execution analysis
MAY use `ExecutionProfileId`. Exact raw replay receipts remain separately
available for audit. Memo-reference contraction additionally requires explicit
producer-certified provenance; it MUST NOT be guessed from a dependency edge
that merely resembles a memo reference.

Every published nonempty structural mechanism definition MUST name its quotient
version and one canonical representative incidence, chosen by the lowest
canonical `(CaseId, SignatureId)` pair in its closed or current support. That
witness links to the authorized Before/After case and replay receipt so an
auditor can inspect one concrete explanation. It is an example, not the support
proof, and it does not enter StructuralMechanismId; a later lower-sorting
incidence may revise the representative while leaving mechanism identity
unchanged.

To share node support across structural mechanisms, `StructuralNodeId` MUST NOT
be a graph-local ordinal or include its enclosing `StructuralMechanismId`. It
hashes the quotient version, endpoint roles/presence, checked site and callee
skeleton, event kind/outcomes, and the quotient's declared local dependency
signature. `StructuralEdgeId` similarly hashes the quotient version, endpoint
colour/relation and its dependent/dependency node IDs; parallel raw occurrences
become execution-profile multiplicity. If a quotient deliberately chooses
graph-contextual node identity instead, its support key is
`(StructuralMechanismId, LocalNodeId)` and the implementation MUST NOT claim
cross-mechanism node sharing.

Terminal derivation and replay validation MUST operate over borrowed sealed
catalog entries, or consume already validated builders into their snapshots.
Accepting a compact closure MUST NOT require a second owned copy of every input
row, contribution, provenance set or analysis layer merely to recompute the
same root. An explicit caller-requested full snapshot or compatibility view may
materialize such a copy, but it is not part of ordinary close, resume or report
publication.

The first acceptance of a result publication MAY reconstruct its exact output
once, but the live catalog then retains only a compact, process-local validation
witness binding the publication, spec, evidence, projection, result roots and
exact counts. Terminal validation MUST use that immutable witness rather than
reconstructing the output again. The witness is not semantic or recovery state:
a typed snapshot restore remints it through the same full validation boundary.

Closing one mechanism request likewise retains only a compact replay-derived
receipt: request ID, incidence root, exact counts, exact downstream input seal
and the frozen publication-discovery end. The mechanism payload remains solely
in its live builder until final analysis closure consumes that builder into the
one closed snapshot. The receipt is a request-level write barrier: later target
or artifact payload MUST fail before mutation, equal close replay MAY remain
idempotent, and final analysis closure MUST remint and compare the full receipt
before consuming the builder.

The core stream exposes three distinct commitments: `RelationalCoreEvidenceRoot`
for relation/classification/support evidence, `RelationalCheckpointRoot` for
the work frontier and latest materializer cursors, and the ordered journal
head. A certified core may close when every declared support obligation is
proved even if no extensional relation root exists. The stricter extensional
close additionally requires complete concrete relation, admission and FIND
catalogs and publishes their content roots. Neither core close claims that
requested result or mechanism layers are complete; those join overall closure
only through their own sealed roots.

The frontier records at least:

- whether source enumeration is open or sealed;
- for every discovered source, whether its successor enumeration is open or
  sealed;
- admission and selection classification obligations;
- result-view reducer and choice obligations; and
- mechanism replay and incidence obligations.

Each yielded prefix, source row and case has an immutable readiness node.
Downstream work depends on readiness, not on closure of the still-producing
enumerator. An implementation MUST therefore be able to classify, reduce and
replay a discovered case while its surrounding source or successor frontier
remains open; the semantic DAG must not become a hidden batch barrier.
Content IDs are not scheduling watermarks: later CaseIds or signature IDs may
sort below earlier discoveries. An implementation MAY rebuild discovery-order
indexes from journal order for efficient catch-up, but those indexes and their
cursors MUST remain operational state outside arrival-order-independent answer
roots. Exact closure always checks the canonical member set and count.

Execution, semantic knowledge and materialization are three orthogonal status
axes in the target contract:

```text
execution.status:
    running | paused(reason) | stopped(reason)

semantic.status, per relation/question/view/choice/explanation/support claim:
    open | exact | unavailable(reason) | error(reason)

materialization.status, per publication consumer:
    not_requested | pending | caught_up
    | capacity_limited | unmaterialized | error

count.status, per reported cardinality:
    unknown | lower_bound(n) | interval(lower, upper) | exact(n)
```

`paused(reason)` means open resumable work is durably retained and therefore
MUST carry a ResumeCursor. Time limits, resource pressure and ordinary user
interruption take this route after the current atomic event. `stopped(reason)`
means the execution exposes no continuation capability because all requested
obligations are terminal, the operator explicitly abandoned them, or an
unrecoverable integrity failure ended the execution. It does not imply `exact`.
Likewise an exact symbolic answer may remain
`not_requested` or `unmaterialized`, and a fully written prefix may remain
semantically `open`. `caught_up` means only that an artifact matches its named
EvidenceToken; it does not mean that the underlying semantic node is exact.
Lower/upper bounds and unavailable residuals accompany the semantic status
rather than being compressed into execution or materialization status.

Pause occurs only after accepted evidence and the remaining frontier are
durable. Resume continues from that frontier without renaming cases. A later
Publish attachment starts new materialization work over the same semantic graph
and does not retroactively turn an earlier stopped execution into a cursor.
`exact`
requires every answer-defining frontier to be closed by finite exhaustion,
certified region coverage or another exact method. Each separately requested
view, choice, explanation and support relation reports its own semantic status;
each public consumer reports materialization independently.

Publication v13's historical lifecycle `running <-> paused -> sealed` and
answer values `partial | complete | unknown | unsupported | error` must be read
through these axes: `sealed` is execution terminality, `complete` corresponds
to semantic exactness only where the claimed relation's closure receipt says
so, and `partial` denotes useful open evidence. Those historical labels do not
override the target vocabulary.

There is no authored probe block, probe-complete state or global probe phase.
Endpoints, source events, midpoints, region proofs and singleton evaluations
are ordinary work nodes with scheduler priorities. Scheduling policy is
observable operational provenance and is absent from RelationId, AdmissionId,
QuestionId, ViewId, ChoiceId and AnalyzeGraphId.

### Certified support cells and physical optimization

The exact evidence unit MAY be one concrete case or one certified `SupportCell`.
A support cell denotes a finite set rather than a representative sample. It
contains a canonical support expression, exact cardinality or explicitly open
cardinality, a resumable materializer and the semantic dependency digest of the
program it covers. A split certificate proves that its children are disjoint
and their union equals the parent. Duplicate producer assignments and
set-normalized rows remain different populations; a cell therefore records the
mapping or injectivity evidence needed to justify every published row, case and
profile count.

Admission, FIND, result reducers and mechanism incidence MUST be able to
consume certified cells without first expanding every member. A uniform
classification, measure or mechanism signature applies to a cell only with an
exact certificate over every member. Otherwise the cell remains open, is split,
or falls back to concrete evaluation. Retained examples are materializations
from support; their display count is never substituted for support cardinality.

Splitting support does not prove the parent claim uniform. It creates a
content-addressed obligation-refinement edge from the parent obligation,
through the accepted cell partition, to exactly one same-claim obligation for
each child. The parent is then superseded in the active proof frontier; the
child obligations may close with different conclusions. Closure means that
every active obligation leaf is proved, not that every historical parent has
one uniform conclusion. This distinction is what permits an outcome DAG to
discover several mechanisms or classifications inside one initially broad
cell without losing exact coverage.

The obligation DAG has explicit roots. An unattached obligation is an error,
not a newly inferred question, and every retained obligation must be reachable
from a declared root. Direct evidence and refinement are mutually exclusive
for one parent: accepting both could retain a uniform parent conclusion that a
child later contradicts. Materialization cursors are authenticated resume
state, but they do not enter `EvidenceRoot`; equal proof frontiers reached by
different worker schedules therefore converge to the same evidence identity.

Solver choice is physical policy rather than query syntax. Implementations MAY
combine checked dependency slicing, partial and incremental evaluation,
interval/affine/congruence interpretation, categorical decision diagrams,
integer polyhedral or Presburger counting, LP/MILP bounds, SMT refinement and
batched concrete evaluation. Each backend closes only the obligations covered
by a checked certificate. Binary search is exact only inside a region with a
proved monotonicity or equivalent boundary certificate; otherwise midpoint is
merely a scheduling split and cannot establish coverage.

The required fallback is canonical concrete evaluation. Consequently the
architecture gains compression on structured rule graphs without claiming a
sublinear solution for arbitrary finite predicates, and a proof backend can be
added or replaced without changing RelationId or the authored question.

The first streamed regional-certificate slice is deliberately narrower than
that complete portfolio. Before concrete classification starts for the next
canonical case-partition child, the scheduler may ask the producer-owned
classification capsule to prove that the whole child is either rejected or
admitted but not selected. A certificate is authority-bound to the immutable
checked query and capsule, the support plan, the accepted partition and child,
the exact coordinate interval, the normalized formula and its conclusion.
Selected, mixed, unsupported, overflow-risk and partially checkpointed children
remain on the concrete path.

The certificate keeps starter and case coordinates distinct. Its subject is
the existing mapped case child, while its preimage names the original finite
source factor and the checked producer chain through source assignments, the
correlated `(Context, Before)` row image, the successor fiber and the case
image. The derived correlated-starter-region identity denotes that complete
image restriction; it is not a Cartesian product of independently widened
field bounds. The certificate need not materialize starter rows, but no public
projection may relabel its scalar scheduling axis as starter support.

One journal event re-verifies that certificate under the matching producer
authority and atomically installs the child cardinality and classification
evidence, leaf seal, materialization cursor and contiguous classified progress.
Cold replay without that exact authority, or with a capsule, subject or proof
mismatch, fails before semantic mutation. The public case/support projection is
therefore a hybrid ordered prefix: concrete children retain their classified
runs and sparse selected materializations, while a certified zero-selected
child contributes one proof-backed uniform region and no invented `CaseId` or
extensional content root.

For a fixed canonical child size `K`, a one-axis relation of `N` cases needs at
most `ceil(N/K)` formula proofs plus concrete work only for residual children.
If capsule normalization costs `G` and concrete evaluation costs `E`, the first
slice is `O(ceil(N/K) * G + R * E)` for `R` residual cases, with bounded proof
state per child and `O(N/K)` retained progress. Later adaptive certified cells
may reduce the proof count further without changing this certificate or
journal contract.

Time, CPU, memory and worker limits are invocation policies. Resource pressure
MUST reduce dispatch or pause before host stability is endangered. The initial
Personskat experiments use an 80-percent installed-CPU ceiling and the smaller
of 80 percent of physical RAM or the operator's explicit 6-GiB process-group
envelope, with host-pressure trips free to lower either. On the current 8-GiB
host a 512-MiB runway makes 5.5 GiB the soft dispatch/heap trip, while at least
1 GiB of host-available memory remains required. These sampled containment
policies do not define a different query and are not instantaneous kernel quota
guarantees.

Legacy ordinal, probe-phase and snapshot schemas MUST fail closed when opened by
the relational runtime. They are not silently migrated into new identities.

Human and machine reports MUST identify the cohesive EvidenceToken and
authenticated journal head they project, the Explore/Analyze identity ladder,
composed CoverageBundleId, declared coverage gaps/restrictions, and separate
semantic and materialization statuses for base counts, every named view,
choice, explanation and publication reader. Population-sized selected configurations and
incidence relations MUST be exportable as bounded records (for example
NDJSON); a renderer MUST NOT require one in-memory JSON array merely to save an
otherwise durable exact answer. Only fields authorized by a view's
`select` schema may enter its public configuration export.

Publication v15 gives every mechanism request two support-observation
artifacts. `mechanisms/<request>.support-observations.ndjson` is the one shared
append-only point stream for automatic and explicit slices.
`mechanisms/<request>.support-observation-demands.ndjson` is the durable demand
ledger: it publishes registration evidence, the checked name-independent
demand-set identity and every authored alias pointing back to its stable slice
in the shared stream. Both artifacts are value-free and MUST distinguish
`contains_typed_values: false` from an empty typed result. The structural
sidecar publishes assignments, structural closure and at most one automatic
support receipt; it MUST NOT duplicate those point records or emit one support
row for every structural node and edge at closure.

Publication v15 implements independently domain-separated inner/outer
expression bounds for correlated case support `S` and distinct-starter
projection `P = distinct_sources(S)`, plus explicit `starter_set_status` and
`correlated_support_status`. Publication v12 is implementation history, not a
compatibility target. Typed-region execution and the broad Personskat run
remain later work.

Every support-observation point and typed subject header MUST expose enough
audit lineage to interpret its source coordinates without following an
implementation-private object graph: `mechanism_request_id`, `relation_id`, the
applicable `admission_id` and `question_id`, `target_id`, structural
`subject`/`facet` and optional `route`, `state_schema_id`, `context_schema_id`,
`transition_type_id`, and `source_coverage_manifest_digest`. The compact
answer's record for each mechanism request MUST directly name its
structural-definition artifact, support-observation stream, demand ledger and
any declared typed-subject materializations (or a bounded manifest handle
containing those links). Discoverability fields are references only: they
neither inline values nor enter `StructuralMechanismId`, `StructuralNodeId` or
`StructuralEdgeId`.

Report v8 MUST expose the same partitions rather than one ambiguous total. It
reports the total shared point count/root; automatic point, registered, dirty,
observed and sealed counts plus the automatic chain root; and explicit demand
registrations, point count, registered, ready, pending-backfill, dirty,
unsealed, observed and sealed slice counts. Automatic whole-mechanism aliases
remain durable demand registrations but are identified as overlaps rather than
new explicit scheduler slices. A report MUST NOT derive a case count from any
of these operational point or slice counts.

The automatic case/support graph MUST reflect the proof shape the scheduler
actually closed; it MUST NOT wait for a bounded chunk partition that the
selected execution path never mints. A partitioned classified run publishes
its root, complete chunk packages, homogeneous outcome regions, selected-run
materializations and authorized cases. A closed concrete or otherwise
layer-composed exact classification may instead publish one classification
summary root, the mutually exclusive rejected, admitted/not-selected and
admitted/selected regions, authorized selected cases, and an exact closure.
That closure names the classification authority, selected-population
authority and authenticated support prefix. Both shapes join the mechanism
incidence graph through authorized `CaseId`s and preserve the same exact case
membership; neither may fabricate a partition, case identity or exact count
merely to make the public graphs uniform.

The case/support graph is not the semantic case graph. When a checked
selected-input `each case` view directly exposes `case_id`, `context`,
`before`, and `after`, publication MUST additionally expose the selected
semantic transition relation as a bounded resumable edge list or an explicit
typed capacity frontier. Every materialized edge record binds its `CaseId`,
source and successor keys, role-neutral Before/After
`StateId`s, directional `TransitionId`, checked schema identities, and the
authorized typed Context/Before/After values. Endpoint StateIds are the graph
nodes, and the existing within-relation CaseId-to-TransitionId injectivity
still applies; this is not a Cartesian product with mechanism nodes. The same
`CaseId` and `TransitionId` join this artifact to mechanism incidence, while
`SourceKey` and `SuccessorKey` join it to conditioned starter-support
projections and successor fibers.

Open rows follow the authenticated selected-discovery order because canonical
CaseId order is not prefix-stable while FIND is still discovering members.
The exact closure separately commits the canonical selected set, graph-content
root, distinct state count, distinct transition count, and selected-question
seal. Thus scheduling may change presentation order without changing graph
identity. Adding this publication lane to an already completed Experimental
stream MUST leave prior artifacts unchanged while appending its independent
artifact and updating publication cursor/manifest state; it MUST NOT
re-evaluate a case, replay a mechanism, or change the journal head. The typed
graph is confidential output under the same explicit value authorization as
its authorizing view.

The explicit `transitions NAME from all cases` consumer is a separate full
graph, not an alias for that selected typed edge list. It publishes only
StateIds, TransitionIds, endpoint StateId links, and canonical U/D/M
`(TransitionId, CaseId)` support. Every support member also binds its
`SourceKey` and `SuccessorKey`, giving an authenticated route back to the
relation-scoped `(Context, Before)` starter and its per-starter After fiber.
This route does not redefine the global semantic `TransitionId`, which already
binds its canonical Context/Before/After triple, and the coordinate hashes do
not reveal those typed values. The graph MUST NOT implicitly publish Context,
Before or After values; those remain behind the existing checked value
authorization.
`SuccessorDiscovered` adds U support, an admitted
`AdmissionClassified` adds D support, and a selected `QuestionClassified` adds
M support. A structurally excluded successor emits no transition at all.

The journal preflights relation identity, state/transition collisions and
classification containment before mutating each affected layer. Replay folds
the same events into CaseId-, StateId- and TransitionId-ordered authenticated
sets, so append order cannot change the graph root. Publication is paged from
those indexes after stable extensional closure and never clones the whole
graph into one terminal array. If the bounded publication capacity is
exceeded it emits `capacity_limited`. If a consumer is attached to a
proof-closed run whose cases were not extensionally retained and no existing
authenticated materializer can satisfy the demand, it emits
`unmaterialized`; it MUST NOT silently rerun the question or change its answer
identity. A fresh safely bounded run MAY choose concrete traversal as an
operational strategy for satisfying the declared materialization obligation.

On supported Unix hosts, every publication invocation MUST create or tighten
the operator-selected output root and its owned subdirectories to mode `0700`
and every cursor, manifest, artifact and atomic temporary to mode `0600`,
including an existing owned namespace being resumed. Failure to establish
those permissions is a publication error. Owner-only filesystem access reduces
accidental local disclosure; it is not anonymization and does not make a typed
bundle safe to publish or share.

Projection V2 collision-checks and counts at most 65,536 selected edges in
memory. If discovery proves at least 65,537, the file retains that stable
65,536-edge prefix and appends `capacity_limited`; it MUST NOT claim an exact
selected set, content root, state count or transition count. This terminal is
an honest bounded result, not an empty graph or a completed exploration. A
later disk-backed or incrementally authenticated projection version may raise
or remove the operational cap without changing the semantic distinction above;
the V2 bound itself is schema behavior and cannot change in place.

### Goals and non-goals

The feature MUST:

- search a declared finite relation of coherent typed states and successors;
- distinguish explicitly conditioned `given` inputs, finite `vary` dimensions
  and deterministic `let` construction in both syntax and coverage;
- expose every case as one typed `Transition<Context, State>` value and allow
  reusable checked pure `derive` expressions over it;
- preserve model validity, exclusions and unknowns without silently dropping
  cases;
- require total admission and question classification, so evaluation failure is
  never negated into a violation;
- let several named questions share one RelationId and AdmissionId without
  cloning the explored world;
- keep relations, admissions, questions, views, choices and mechanism evidence
  independently addressable;
- define views, choices and explanations in one separately named,
  order-independent acyclic Analyze graph with explicit dependency edges;
- discover thresholds as findings or optimizer events rather than requiring an
  authored threshold list;
- pause, inspect and resume without weakening exactness claims;
- use canonical rule evaluation and fresh endpoint replay for evidence;
- expose closure-aware counts and deterministic named views with checked
  monotone, revisable or closure-gated update behavior;
- compose source, admission, question and observer coverage for every public
  population claim;
- keep execution, semantic and materialization status independent, and bind
  cohesive reads with EvidenceToken rather than a resume cursor;
- name correlated `support cases` separately from deduplicated
  `support starters`; and
- expose unavailable reasoning or replay explicitly and never promote it to an
  exact count or closed downstream claim.

The feature does not infer a useful question or real-world population, invent
unbounded personal facts, treat one rule name as a complete mechanism, or turn
model completeness into legal authority. The broad tax examples are research
questions over the checked-in encoded model, not individual advice.

### Perspective-based acceptance gate

Before a target-language revision is accepted, it MUST express the three
steering scenarios—multidimensional income cliffs, lowest-tax municipality iff
the alternatives actually differ, and household work/pension Pareto
planning—without hidden profile facts or scenario-specific syntax. Reviewers
MUST apply four independent perspectives:

- the policy author can state the finite world, intervention, admission and
  question without referring to execution strategy;
- the language implementer can lower every declaration to typed,
  content-identified Explore and Analyze nodes without consuming display rows
  as semantic input;
- the auditor can reconstruct scope, composed coverage, cardinality status and
  the boundary between comparison proof and causal explanation; and
- the stream operator can pause/resume work and fold authorized
  add/retract/seal updates while keeping EvidenceToken, ResumeCursor, semantic
  closure and materialization status distinct.

A revision fails this gate if any perspective must invent an implicit
population, comparison group, totality assumption, update rule or meaning of
“complete.” Passing the gate validates the general contract; it does not claim
that every scenario is executable in the current Experimental implementation.

### Implementation slices

Implementation proceeds in this dependency order:

1. Replace the public frontend with `derive`, target `? explore` using
   `given`/`vary`/`let`, typed `transition`, total `admit` and named `find`, plus
   separate target `? analyze`. Delete compatibility parsing for both older
   surfaces once their target replacements execute.
2. Close one typed relational IR with stable model-owner, schema, DerivationId,
   RelationId, SourceKey, SuccessorKey, CaseId and Transition value semantics.
3. Add total AdmissionId classification and any number of independently named
   QuestionIds over the shared admission, with integrity errors unable to mint
   matches or violations.
4. Add the order-independent typed Analyze DAG: explicit ViewId nodes,
   separately identified ChoiceId nodes and `explain ... using` mechanism
   requests; reject cycles and implicit input relations.
5. Define resumable concrete producers and first-class certified support cells,
   including exact partition, cardinality, provenance and materialization
   contracts for source and per-source successor frontiers.
6. Make the authenticated journal and indexed evidence state incremental: no
   whole-state clone or full-relation rebuild may be required per accepted
   event, and completed work need not remain in the open frontier. Add cohesive
   EvidenceToken reads, separately typed ResumeCursors and orthogonal execution,
   semantic, materialization and count statuses.
7. Add view reducers, deterministic choices, endpoint replay and exact
   CaseId/TransitionId/signature incidence; every semantic layer accepts either
   concrete rows or certified cells. Seal one request-scoped endpoint-totality
   obligation per explanation in AnalyzeGraphId. Accept either a static
   certificate over a sound endpoint-domain approximation or an extensional
   certificate over every endpoint in the exact closed target; store accepted
   evidence in the semantic root without renaming the graph.
8. Emit separate source, successor, admission, question and generic
   Analysis-node coverage artifacts and their composed CoverageBundleIds.
9. Add the initial optimizer portfolio: dependency slicing, endpoint/delta
   reuse, affine/interval/congruence certificates, guard-driven partitioning
   and canonical concrete residue; then add decision-diagram, Presburger and
   SMT proof backends through the same certificate interface.
10. Publish closure-aware human/JSON/snapshot artifacts and privacy-safe saved
    evidence queries, including explicit public update events and distinct
    `support cases`/`support starters` consumers.
11. Exercise small genuinely multidimensional Personskat relations, then widen
   toward annual income through 1,500,000 DKK only after the relation and proof
   frontier behave correctly.
12. Add permanent focused coverage and run the required semantic-change gates
    after the architecture and output contract are coherent.

### End-to-end acceptance

The replacement is accepted when:

- `given`, `vary` and `let` examples parse, type-check, retain their coverage
  roles and lower singleton and multivalued transitions to one relational IR;
- the reserved typed `transition` exposes exactly canonical Context, Before and
  After, and duplicate producer paths do not inflate SourceKeys, CaseIds or
  counts;
- reusable pure `derive` closures are content-identified, cannot observe
  scheduler/effect state and are total over every accepted use-site domain;
- one total `admit` and several named `find` declarations share RelationId and
  AdmissionId while resolving stable content QuestionIds, including aliasing
  identical normalized finds; a classification
  integrity error selects neither a match nor a violation and prevents exact
  closure;
- old authored probes, old compact syntax and transitional nested
  FROM/TO/WHERE/FIND plus `results`/`mechanisms`/`starters` fail with targeted
  diagnostics after their target replacements land;
- the RelationId-scoped source and successor coverage components recursively
  report every Context/Before/After field path and reachable immutable producer
  input as `given`, `vary`, `let`, proof-backed exactly irrelevant or an
  explicit gap, so a broad profile claim cannot hide literals or top-level
  constants inside helpers;
- admission, each question and every Analyze node retain separately scoped
  coverage, and every view/choice/explanation/report binds the exact composed
  CoverageBundleId reachable from its inputs;
- Analyze declarations resolve independently of source order, equivalent node
  reorderings preserve AnalyzeGraphId, and missing references, grain/type
  mismatches and cycles fail before journal genesis;
- ViewId contains no choice policy, ChoiceId contains the complete objective and
  tie/cardinality semantics, a later display-only view cannot rename the choice,
  and explanations may target QuestionId or ChoiceId explicitly;
- every explanation-bearing checked graph seals one endpoint-totality
  obligation per request; exact explanation requires either a valid static
  certificate or an extensional certificate covering every endpoint in the
  exact closed target, while an open replay prefix proves only its members;
- semantic evaluation failure after static certification is an integrity error,
  while an uncertified deterministic failure is typed unavailable, and
  cancellation and transient host pressure preserve an open resumable frontier
  and deterministic instrumentation/capacity limits remain explicitly typed
  unavailable evidence that prevents unsupported exactness;
- exact behavior quotients preserve disjoint weighted profile/case support,
  while uncertified representatives never stand in for unvisited profiles;
- explicit `given`/`vary` source conditioning changes RelationId while
  equivalent optimizer pushdown of `admit` does not;
- changing only admission, one named question, views, choices, explanations,
  names, scheduler or resource limits preserves the identities of unaffected
  layers beneath and beside it;
- pause/resume and discovery-order permutations preserve stable CaseIds,
  evidence roots, EvidenceTokens for equal semantic snapshots, any materialized
  completed content roots and exact results; ResumeCursor remains operational
  and cannot be substituted for EvidenceToken;
- a zero-, one- and many-successor source all close correctly;
- named `find = all`, `matches` and `violations` classify the same admitted
  relation under distinct QuestionIds;
- case views, choices, explanation targets and post-explanation histogram views
  form an acyclic typed dependency graph;
- monotone nodes never retract, revisable nodes emit authenticated add/retract
  transactions under the canonical keyed fold, and closure-gated nodes publish no row
  before their required input seals;
- execution termination never implies semantic exactness, exact symbolic
  evidence never implies materialization, and every public node/consumer carries
  all three applicable status axes;
- checked observation demands deduplicate aliases by name-independent slice
  identity, preserve every upstream identity, and replay the same registration,
  bounded backfill and observation-point evidence before and after core closure;
- automatic whole-mechanism observations alone authorize structural support
  closure, while explicit node/edge readers can finish independently without a
  `cases * DAG subjects` table;
- publication readers attach without changing AnalyzeGraphId or upstream
  semantic identities; target `support cases` emits `S` source/successor members,
  target `support starters` emits only deduplicated `P` SourceKeys, and neither
  count/root/schema can be substituted for the other;
- every published count or optimum carries an honest semantic closure status,
  public update mode and cohesive EvidenceToken;
- Carl/John-style distinct transitions may share one complete mechanism
  signature without losing either support;
- a shared structural node retains one `StructuralNodeId` while distinct
  correlated origin starters enlarge its request-, target- and facet-conditioned
  support; confirmed inner support establishes a lower bound, a concrete outer
  envelope permits `interval(lower, upper)`, an opaque target obligation reports
  `unknown(lower)`, and only closed starter and successor obligations permit
  exact correlated support, without multiplying marginal Before-field bounds;
- each published subject-support point keeps `S` case-support roots distinct
  from `P` starter-projection roots, derives `A(source)` only as a successor
  fiber of `S`, reports `starter_set_status` separately from
  `correlated_support_status`, and links its request/relation/target/schema and
  composed coverage authority without putting values into structural identity;
- an empty exact result is complete, not unknown;
- resource pressure pauses durably rather than crashing or fabricating closure;
  and
- the broad Personskat run is not started merely to obtain an early number
  before these contracts and best-effort proof optimizations are in place.
