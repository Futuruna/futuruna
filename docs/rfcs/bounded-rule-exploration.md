# Bounded Rule Exploration with `? explore`

Status: accepted architectural direction; Experimental breaking replacement

This RFC defines Futuruna Explore as a finite, provenance-aware relational
query over typed state transitions. The relation contract in this opening
section is normative for new implementation. It replaces the earlier
Cartesian `over`/`bounds`/`boundaries`/`transition`/`probes`/`output` design;
that syntax has no compatibility claim and MUST NOT be retained as a second
public Explore language.

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
before-to-after state transitions satisfy a question, which coherent model
profiles support them, and—when requested—which replay-derived mechanisms those
transitions share.

An Explore declaration has six semantic stages:

1. `from` constructs a finite dependent relation of canonical
   `(Context, Before)` source rows.
2. `to` constructs a finite successor set for each source row.
3. scoped `where` clauses classify constructible cases for admission.
4. `find` selects all admitted cases, matches, or violations.
5. named `results` blocks derive typed views.
6. named `mechanisms` requests replay explicit endpoint observations and expose
   typed signature-incidence relations to later result views.

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

### Canonical source form

The target source form is:

```runa
? explore QUERY_NAME {
    from {
        AUXILIARY = PURE_VALUE
        ITEM in EXACT_FINITE_EXPRESSION
        context = CONTEXT_EXPRESSION
        before = BEFORE_EXPRESSION
    }

    to after = SINGLE_SUCCESSOR_EXPRESSION
    -- or: to after in EXACT_FINITE_SUCCESSOR_EXPRESSION

    where before BEFORE_PREDICATE
    where after AFTER_PREDICATE
    where transition TRANSITION_PREDICATE

    find all
    -- or: find matches of BOOLEAN_EXPRESSION
    -- or: find violations of BOOLEAN_EXPRESSION

    results SOURCE_VIEW from sources {
        group by [context = context]
        aggregate [source_rows = count_distinct(before)]
        select [context, source_rows]
    }

    results CASE_VIEW {
        each case
        measure [NAME = EXPRESSION]
        select [NAME = EXPRESSION]
    }

    mechanisms REQUEST for selected from ENDPOINT_OBSERVER
    -- or: mechanisms REQUEST for admitted from ENDPOINT_OBSERVER

    results MECHANISM_VIEW from mechanisms REQUEST {
        group by [NAME = EXPRESSION]
        aggregate [NAME = CLOSED_GROUP_REDUCER]
        select [NAME]
    }
}
```

After `find`, named `results` and `mechanisms` clauses form a typed dependency
DAG. A declaration MUST place a node after every node it references. An
unqualified `results NAME { ... }` is shorthand for `from selected`.
`results NAME from sources` reads the normalized `(Context, Before)` source
relation independently of successor, admission and FIND closure. Its current
surface is grouped and exposes only `context` and `before`; auxiliary producer
bindings remain authenticated construction lineage rather than source-row
columns.
`results NAME from mechanisms REQUEST` reads the request's authorized incidence
rows. A mechanism may target `selected` cases, all `admitted` cases, or `view
NAME chosen` rows from an already resolved case view. A selected or chosen-view
target is question-scoped. An admitted target is admission-scoped: changing
only FIND MUST NOT change the identity of the admitted population or its replay
target. The checker MUST reject forward references, missing targets, type
mismatches and cycles, including a mechanism targeting a view that depends on
that same mechanism.

The incidence relation first carries the complete raw replay assignment
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

The current first execution path supports selected and chosen-view targets.
Until the admission-scoped target seal and admitted-case materializer land, a
separate `find all` question is the exact executable spelling for a complete
admitted mechanism landscape because `S_Q = D_A` in that query. This is a
staged implementation limit, not a competing semantic definition.

The grammar is intentionally one language, not a compatibility adapter:

- every query is named;
- `from`, `to`, zero or more scoped `where` clauses and `find` are required in
  that order;
- zero or more named result and mechanism nodes follow in dependency order;
- there is exactly one semantic `before` binding and one semantic `context`
  binding;
- `to after = EXPR` is singleton successor syntax;
- `to after in EXPR` requires a checked exact-finite collection;
- Before and After have the same closed state type;
- source, successor, admission, selection, view and observer expressions are
  checked and pure; and
- the old `over`, `bounds`, `boundaries`, authored `probes`, transition-mode,
  `observe mechanisms with`, `output`, `output as` and `then` clauses are not
  part of this contract.

Bindings in `from` are ordered. `name = expression` contributes one value;
`name in finite_expression` performs a dependent finite expansion analogous to
SQL `LATERAL`. Each expression may refer only to earlier bindings. Auxiliary
bindings are authenticated producer lineage, not hidden case-identity fields.
Alpha-renaming a resolved local binder does not change semantics.

A mechanism observer has checked shape `(State, Context) -> Observation`.
Static totality proof is sufficient but not required: over a finite closed
target, one complete fresh replay at every Before and After endpoint is also a
definedness witness. A failed deterministic replay becomes a typed unavailable
terminal. Invocation time, cancellation and host-resource pauses remain open;
a fixed replay-ABI evaluation or trace capacity is instead typed unavailable,
because retrying the same ABI cannot cross the same deterministic boundary.
Mechanism and downstream result certainty MUST degrade whenever any target
lacks a complete signature.

An exact-finite producer may be a list, end-exclusive range, finite enum,
dependent join, indexed relation or certified symbolic cell. Its checked
contract MUST expose a canonical element schema, set normalization, a resumable
enumerator and a closable frontier. Separate independent `in` bindings form a
product. Authors SHOULD instead construct coherent typed profiles when facts
are correlated; structurally impossible profiles do not belong in the source
relation merely to be filtered later.

The checked query artifact MUST also expose a `RelationId`-scoped, **FROM-only**
source-construction coverage manifest. Its dependency digest covers the checked
`from` producer closure and the closed Context/Before schemas. It does not
absorb `to`, admission, `find`, result-view or mechanism-observer dependencies.
It recursively walks Context and Before field paths, including variant and
nested-record segments, until it reaches closed leaves. An open, recursive or
unsupported composition becomes an explicit gap at the affected path boundary;
it is never silently omitted.

For each Context and Before path, and each reachable immutable producer input,
the manifest records whether it is a varied finite dimension, derived from
declared dimensions, explicitly conditioned to a singleton or source
restriction, covered by an exact irrelevance certificate, or an acknowledged
model-coverage gap. Literals and referenced immutable top-level constants
inside ordinary producer helpers remain visible conditioning. The manifest is
derived from the checked producer closure and does not add a second source DSL
or invent an undeclared dimension.

The exact-irrelevance category is proof-gated. Its presence in the manifest
algebra does not claim that an irrelevance producer exists for every query; in
the absence of producer-issued exact evidence the compiler MUST NOT label a
path irrelevant. Declared support remains intact, while an unmodeled or
unproved path is a coverage gap. A gap forbids any broader population claim,
although an exact result over the smaller declared relation remains exact over
that relation.

Admission, `find` and mechanism-observer input coverage MUST be separate sibling
artifacts, scoped respectively by `AdmissionId`, `QuestionId` and
`MechanismRequestId`. They are not yet emitted. When implemented, they may
reuse the same coverage vocabulary, but they do not rewrite the FROM-only
manifest or `RelationId`-scoped source facts when a predicate or observer
changes.

Source conditioning and admission are different. Restricting a producer in
`from` declares a smaller world and changes `RelationId`. A scoped `where`
predicate classifies an already constructible case and changes `AdmissionId`,
not `RelationId`. A backend MAY push an admission predicate into enumeration,
but the optimization MUST preserve both identities and every population count.

The successor is an ordinary checked relation:

```text
successors(context, before) -> finite set of after states
```

It may contain zero, one or many After values. Duplicate canonical successors
under one source collapse. If two interventions reaching the same After value
are semantically different, their typed action identity belongs in Context;
producer provenance alone cannot keep them as different cases.

### Formal algebra

For one checked query:

```text
R       = distinct canonical (context, before) rows produced by FROM
C_R     = { (context, before, after)
          | (context, before) in R,
            after in successors(context, before) }
D_A     = { case in C_R | admission_A(case) }
S_Q     = D_A                                      when FIND ALL
S_Q     = { case in D_A | predicate_Q(case) }      when FIND MATCHES
S_Q     = { case in D_A | not predicate_Q(case) }  when FIND VIOLATIONS
V_case  = a named view over S_Q
M_Q(T)  = differential signature incidence for question-scoped target T
M_A(D_A)= differential signature incidence for the admitted population
S(q,m)  = { case in target(q) | M_q(case) = complete signature m }
P(q,m)  = projection_(Context, Before)(S(q,m))
A(q,m,s)= { after | (s.context, s.before, after) in S(q,m) }
V_mech  = a named view over the incidence relation produced by M_Q or M_A
```

Source and successor collections have set semantics. Equal canonical
`(Context, Before)` rows collapse and union exact producer support. Equal After
values under one source do the same. Producer-path counts remain useful
diagnostics, but they are not case counts.

`S(q,m)` is the complete case-support fiber, `P(q,m)` is its distinct starter
support and `A(q,m,s)` is the dependent successor fiber at starter `s`.
Projection can collapse several supported cases onto one starter, so these
populations have separate closure and count evidence. In particular,
`|S(q,m)| = sum(s in P(q,m), |A(q,m,s)|)`; neither a starter count nor marginal
field bounds can be substituted for the case count.

An optimizer MAY evaluate one representative for a region whose members are
proved behaviorally equivalent, but it MUST retain an exact, disjoint support
certificate for that region. Such a quotient reduces evaluator and mechanism
discovery work without changing source, case, affected-profile or incidence
populations. A concrete representative without that certificate has support
one; it is never extrapolated to unvisited profile combinations.

`where before`, `where after` and `where transition` clauses are one normalized
pure conjunction. Clause order and repeated identical conjuncts are diagnostic
source facts, not inputs to `AdmissionId`.

Result-view grain is explicit:

- `each case` preserves `CaseId` as row identity;
- `group all` forms one closed group;
- `group by [FIELD...]` forms canonical value groups;
- `measure` computes named exact scalars per input row;
- `aggregate` consumes a closed group, including `count_distinct`;
- `having` filters only after its required group reducers close;
- `select` is the public projection and privacy allow-list; and
- `choose one|all minimizing|maximizing` or `choose pareto` declares cardinality
  and objective semantics explicitly.

An observed optimum, group, Pareto member or distinct aggregate is provisional
until its required input and view frontiers close.

### Semantic identities

The following layers MUST remain distinct:

**`RelationId`**
: Identity of the checked FROM+TO relation: stable model and type owners,
  Context/State schemas, normalized ordered producer definitions, intrinsic
  finite membership, successor semantics, set normalization and lineage
  contract. It excludes query/view/request names, admissions, `find`, result
  views, mechanism requests, schedules, worker counts, limits and journal order.

**`AdmissionId`**
: `H(RelationId, normalized scoped admissions)`.

**`QuestionId`**
: `H(AdmissionId, normalized FIND predicate or ALL form, polarity)`.

**`ViewId`**
: Identity of one typed input relation—`RelationId` sources, `QuestionId`
  selection or `MechanismRequestId` incidence—plus grain, fields, measures,
  aggregates, filters, choice, deterministic ordering and privacy schema.

**`MechanismRequestId`**
: Identity of one `QuestionId`, explicit selected or view-chosen target,
  canonical endpoint observer, reachable dependency closure and signature
  normalization. A view-chosen target seals the resolved `ViewId`.

**Durable-evidence identity**
: Immutable relation, admission, question, requested view/mechanism DAG,
  evaluator, retention authorization and journal/serialization contracts.

**Operational records**
: Run-state path, worker count, time and resource limits, scheduler decisions,
  pressure events, pauses and resumes. They never redefine the bounded question.

Query, view and mechanism request names are unique source addresses, not
semantic hash inputs. Renaming a node and updating its references preserves its
semantic identity.

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
bindings are factors or dependent fibers; singleton Context/Before
construction is a deterministic map and never a cardinality multiplier. A
runtime fiber is cached by exactly the earlier binding values named by its
dependency edges, not by the complete prefix. TO starts from distinct source
rows after projection normalization, so auxiliary producer assignments cannot
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
- `S_C`: cases selected by a `QuestionId`.

For `find all`, `S_C = D_C`. Inside one RelationId the corresponding extensional
transition counts are conservation equalities: `U_T = U_C`, `D_T = D_C` and
`S_T = S_C`. Cross-relation reports MAY additionally count global distinct
TransitionIds but MUST name that larger scope.

A count is `lower_bound(n)` while any required source, per-source successor,
classification or view frontier remains open. It is `exact(n)` only after those
frontiers close. Raw-signature counts have an independent request-relative
incidence frontier; structural-mechanism and execution-profile counts also
depend on quotient assignment and closure. Before any confirmed replay evidence
the honest value at each grain may be unknown rather than zero.

For a mechanism request, successful replay defines a partial explanation map
`mu: target cases -> signature`. The support fiber of signature `m` is
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
enlarge the same support fiber.

Every published starter region MUST retain the source manifest's distinction
between varied, derived, explicitly conditioned, proved-irrelevant and
unsupported dimensions. If a request fixes `commune = Copenhagen`, that
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
answers where the subject was reached among this request's selected, admitted
or explicitly named target population. It is not by itself a global trigger
predicate or weakest precondition for the underlying rule.

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
For any structural subject `x`, let `C_x` be its true target-case support,
`I_x` the disjoint case atoms already proved to contain `x`, and `O_x` the
concrete atoms already known to be possible support because they have not been
proved unable to contain `x`. `I_x` and `O_x` below denote the unions of those
atoms' case-set denotations, not merely their cell descriptors.

An open target may additionally have an opaque undiscovered-target obligation
`omega`. That token is not itself a case set. Treat the upper bound as the
abstract value `Uhat_x = (O_x, omega)` with a concretization `gamma(Uhat_x)`:
the family of concrete case relations consistent with the known outer atoms
and the still-undiscovered target. The sound open statement is therefore

```text
I_x subseteq C_x
C_x in gamma(Uhat_x)
P_x^- = distinct projection_(Context, Before)(I_x)
P_x   = distinct projection_(Context, Before)(C_x)

P_x^- subseteq P_x
```

Projection and deduplication happen after case-space bounds are derived. An
exact case-atom weight therefore does not by itself establish an exact starter
weight. When `omega` is absent and `O_x` is a concrete envelope, define
`P_x^+ = distinct projection_(Context, Before)(O_x)` and the corresponding
per-source `A_x^+`; projection monotonicity then gives
`P_x^- subseteq P_x subseteq P_x^+` and
`A_x^-(source) subseteq A_x(source) subseteq A_x^+(source)`. These are
correlated regions or checked predicates, not independent per-field boxes.
When `omega` is present, projection is lifted through `gamma`: the upper
starter region, successor fibers and their counts are `top`/unknown unless a
separate checked concrete envelope exists. An empty currently discovered
residual does not discharge that obligation.

A shared `StructuralNodeId` view intentionally unions support across every
structural mechanism containing that node. A presentation which asks for the
node *inside one displayed mechanism* MAY instead derive the intersection
subject `(StructuralMechanismId, StructuralNodeId, facet)` (and analogously for
an edge). That contextual view does not change either structural identity. For
a complete fixed execution graph it may coincide with the whole mechanism's
support; the explicit intersection becomes important when explaining a shared
node across several mechanisms.

The total support of a node or edge and its route-conditioned fibers are
different views over the same case authority. Let `c` be a checked structural
condition such as an owning `StructuralMechanismId`, one incident
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
its fibers are not necessarily disjoint: one case may contain several incident
edges or paths reaching the same shared node. Their starter projections can
overlap even when their case fibers do not. Implementations MUST therefore
deduplicate a requested union and MUST NOT sum edge/path-conditioned counts
without a checked partition proof. Route conditions remain support-overlay
keys; they never clone or rename the structural node.

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

The compact all-subject result MUST NOT eagerly build one complete union after
another: evicting between subjects bounds retained accumulation but still lets
one ubiquitous node allocate `O(target cases)`. Its constant-size row SHOULD
instead use an authenticated factorized summary over the contributing
signature fibers and shared residual. Disjoint fiber weights give case bounds
directly. Before cross-fiber starter deduplication, the largest single-fiber
starter count is a safe lower bound and the sealed target starter count is a
safe upper bound. Automatic publication scans at most 256 canonical fiber
summaries per subject; a capped scan widens bounds and MUST NOT fall back to a
full union. The row MUST label the starter projection `not_materialized`; a
factorized summary root is not a materialized correlated-content root. The row
also publishes authenticated inner and outer **fiber-expression identities**
whose coordinate contract is
`SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>`. The inner expression
commits the contributing signature-fiber union. The outer expression commits
that inner expression plus the shared possible-support residual and any opaque
target obligation; it normalizes to the inner identity only when neither can
add support. These identities preserve correlation semantics without exposing
typed values or authorizing cells. The row separately names an
authorization-neutral `projection_plan_id`, not a public cell job.

Exact correlated materialization is a separate, content-addressed projection
job whose identity is derived from that plan plus checked publication
authorization. Publication v9 schedules that job only for an explicit,
single-subject consumer:

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

The qualified artifact uses subject-starter record schema v2 and adds the
route to its cursor identity. The optional cursor field is omitted for an
unqualified consumer, whose checked ID, projection roots, v1 records and
publication-v9 bytes remain unchanged. Thus a route consumer is an additive
artifact on an existing closed v9 publication, not a migration of total
subject support.

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

The compact mechanism-support result still closes independently; each authored
typed subject artifact has its own resumable cursor and closure at
`starters/<consumer>.ndjson`. Its header binds the request, target, exact
subject/facet, projection plan/job, authorization and structural/support roots;
bounded typed pages retain the correlated values above; its closure certifies
the exact case count, distinct-starter count and content root. Factorized rows
for the complete structural catalog continue to label their inline correlated
projection `not_materialized`: authoring one selected consumer does not turn
every node and edge into an eager artifact. Path-conditioned selectors remain
future work.

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
`(Context, Before) -> After` root exact.

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
an exact materialization of the possible-starter region, and it MUST NOT make
the projected region exact.

Distributed merges union stable terminal, signature-fiber and membership facts
idempotently and reject conflicting terminals or quotient roots. They recompute
outer bounds from the merged unresolved partition; they do not add serialized
lower bounds or union independently projected scalar summaries. A compact
structural-quotient closure binds the raw incidence root, quotient version,
signature-to-quotient root and structural membership root. A support-view
closure additionally binds its case/starter projection roots.

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
starter projections, projection receipts and count intervals to the signature.
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

Lifecycle and answer evidence are orthogonal:

```text
lifecycle: running <-> paused; {running, paused} -> sealed
answer:    partial | complete | unknown | unsupported | error
```

Pause occurs only after accepted evidence and the remaining frontier are
durable. Resume continues from that frontier without renaming cases. `complete`
requires every answer-defining frontier to be closed by finite exhaustion,
certified region coverage or another exact method. A separately requested
mechanism or view frontier may remain open and MUST report its own status.

There is no authored probe block, probe-complete state or global probe phase.
Endpoints, source events, midpoints, region proofs and singleton evaluations
are ordinary work nodes with scheduler priorities. Scheduling policy is
observable operational provenance and is absent from RelationId, AdmissionId,
QuestionId and ViewId.

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

Human and machine reports MUST identify the authenticated journal head they
project, the query identity ladder, declared source coverage and restrictions,
and a separate closure status for base counts, every named result view and
every mechanism request. Population-sized selected configurations and
incidence relations MUST be exportable as bounded records (for example
NDJSON); a renderer MUST NOT require one in-memory JSON array merely to save an
otherwise durable exact answer. Only fields authorized by a result view's
`select` schema may enter its public configuration export.

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
`SourceKey` and `SuccessorKey` join it to conditioned starter fibers.

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
- preserve model validity, exclusions and unknowns without silently dropping
  cases;
- keep cases, questions, views and mechanism evidence independently addressable;
- discover thresholds as findings or optimizer events rather than requiring an
  authored threshold list;
- pause, inspect and resume without weakening exactness claims;
- use canonical rule evaluation and fresh endpoint replay for evidence;
- expose closure-aware counts and deterministic named views; and
- expose unavailable reasoning or replay explicitly and never promote it to an
  exact count or closed downstream claim.

The feature does not infer a useful question or real-world population, invent
unbounded personal facts, treat one rule name as a complete mechanism, or turn
model completeness into legal authority. The broad tax examples are research
questions over the checked-in encoded model, not individual advice.

### Implementation slices

Implementation proceeds in this dependency order:

1. Replace the public frontend with the canonical named FROM/TO/WHERE/FIND
   grammar. Delete compatibility parsing for the old surface.
2. Close one typed relational IR with stable model-owner, schema, RelationId,
   SourceKey, SuccessorKey and CaseId semantics.
3. Add AdmissionId and QuestionId classification independently of relation
   identity and presentation.
4. Define resumable concrete producers and first-class certified support cells,
   including exact partition, cardinality, provenance and materialization
   contracts for source and per-source successor frontiers.
5. Make the authenticated journal and indexed evidence state incremental: no
   whole-state clone or full-relation rebuild may be required per accepted
   event, and completed work need not remain in the open frontier.
6. Add named result views, reducers, deterministic choice, endpoint replay and
   exact CaseId/TransitionId/signature incidence as one checked dependency DAG;
   every layer accepts either concrete rows or certified cells.
7. Add the initial optimizer portfolio: dependency slicing, endpoint/delta
   reuse, affine/interval/congruence certificates, guard-driven partitioning
   and canonical concrete residue; then add decision-diagram, Presburger and
   SMT proof backends through the same certificate interface.
8. Publish closure-aware human/JSON/snapshot artifacts and privacy-safe saved
   evidence queries.
9. Exercise small genuinely multidimensional Personskat relations, then widen
   toward annual income through 1,500,000 DKK only after the relation and proof
   frontier behave correctly.
10. Add permanent focused coverage and run the required semantic-change gates
    after the architecture and output contract are coherent.

### End-to-end acceptance

The replacement is accepted when:

- canonical singleton and multivalued successor examples parse, type-check and
  lower to one relational IR;
- old authored probes and old compact syntax fail with targeted diagnostics;
- dependent source domains vary by earlier bindings and duplicate paths do not
  inflate CaseIds or counts;
- `RelationId`-scoped FROM coverage recursively reports every Context/Before
  field path and reachable immutable producer input as varied, derived,
  conditioned, proof-backed exactly irrelevant or an explicit coverage gap,
  so a broad profile claim cannot hide literals or top-level constants inside
  helpers;
- admission, `find` and observer input coverage remains separately bound to
  its owning identity and cannot rename or broaden the source manifest;
- exact behavior quotients preserve disjoint weighted profile/case support,
  while uncertified representatives never stand in for unvisited profiles;
- explicit source conditioning changes RelationId while equivalent optimizer
  pushdown of `where` does not;
- changing only admission, `find`, views, mechanism requests, names, scheduler
  or resource limits preserves the identities of the layers beneath it;
- pause/resume and discovery-order permutations preserve stable CaseIds,
  evidence roots, any materialized completed content roots and exact results;
- a zero-, one- and many-successor source all close correctly;
- `find all`, matches and violations classify the same admitted relation under
  distinct QuestionIds;
- case views, view-chosen mechanism targets and post-mechanism histogram views
  form an acyclic typed dependency graph;
- every published count or optimum carries an honest closure status;
- Carl/John-style distinct transitions may share one complete mechanism
  signature without losing either support;
- a shared structural node retains one `StructuralNodeId` while distinct
  correlated origin starters enlarge its request-, target- and facet-conditioned
  support; confirmed inner support establishes a lower bound, a concrete outer
  envelope permits `interval(lower, upper)`, an opaque target obligation reports
  `unknown(lower)`, and only closed starter and successor obligations permit
  exact correlated support, without multiplying marginal Before-field bounds;
- an empty exact result is complete, not unknown;
- resource pressure pauses durably rather than crashing or fabricating closure;
  and
- the broad Personskat run is not started merely to obtain an early number
  before these contracts and best-effort proof optimizations are in place.
