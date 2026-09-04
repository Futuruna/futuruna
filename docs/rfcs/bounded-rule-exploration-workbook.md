# Turning Rules Inside Out with Explore

Status: steering workbook for the Experimental relational replacement

The normative semantic contract is
[Bounded Rule Exploration with `? explore`](bounded-rule-exploration.md). This
workbook sharpens the intended authoring surface and turns it into the shortest
coherent implementation path. The exact target spellings below are steering
syntax. The current parser now accepts `from { given/vary/let }`,
`transition after`, zero or more named `find` questions, and explicit named
question inputs for `results` and `mechanisms`, including an exact chosen result
as a mechanism target. Its scoped `where` plus
publication-v17 `results`/`mechanisms`/`observations`/`starters` and
`transitions` forms remain transitional implementation spellings that lower
toward this model.
They are not a second public dialect and carry no compatibility requirement.
The old Cartesian and probe-era syntax remains rejected rather than adapted.

## What we are building

Futuruna normally answers:

> What do these rules produce for these facts?

Explore asks:

> Across this finite relation of coherent Before states and permitted After
> successors, which transitions match or violate the question, and which
> replay-derived mechanisms do those cases share?

An income cliff is one instance. The same algebra supports a municipality
comparison, a household work/pension reallocation, a policy-version comparison
or any other finite before/after relation. The implementation must therefore be
general at the source, case, question, view and mechanism boundaries even while
Personskat supplies the first ambitious result.

One Explore contract owns one finite relation and one admission. It may then
name several questions over that same admitted relation. Analysis and
publication are separate consumers:

```text
EXPLORE CONTRACT
  finite dependent FROM + TRANSITION successor fibers
      -> RelationId + stable CaseIds + authenticated frontier
      -> named DERIVE nodes
      -> ADMIT -> AdmissionId
                    |-- named FIND cliffs       -> QuestionId(cliffs)
                    |-- named FIND all_options  -> QuestionId(all_options)
                    `-- named FIND ...          -> QuestionId(...)

ANALYZE CONTRACT
  QuestionId -------> ViewId
       |------------> ChoiceId -------------> ViewId
       |                 `-- EXPLAIN target
       `-------------------- EXPLAIN target -> MechanismRequestId
                                                `-----------> ViewId

PUBLISH CONTRACT
  explicit view, compact-support, support-case, support-starter and graph readers
  (outside RelationId, AdmissionId, QuestionId, ChoiceId and the analysis DAG)
```

There is no public probe phase. Regional certificates and concrete/classified
fallback already advance as prioritized work in one resumable stream. The
candidate planner can derive endpoints, source events, certificate-authorized
midpoints and lifted cuts; connecting those producers to the same scheduler is
a performance follow-up, and their residual intervals must remain explicit
until certified or materialized. There is also no implicit `selected` relation
in the target surface: every view, choice and explanation names the `find`
relation it consumes.

## The smallest complete target example

This sketch deliberately explores a multidimensional profile relation rather
than a fixed person. `given` declares an explicit singleton or bounded input,
`vary` expands an exact finite domain, and `let` derives a value without
pretending it is another independent dimension:

```runa
? explore income_cliffs {
    from {
        given profile_space = personskat_profile_space_2026
        vary profile in coherent_profiles(profile_space)
        vary income in supported_income_coordinates(
            profile,
            range(0, 1_500_000)
        )
        let context = SalaryChange(amount_kroner = 1)
        let before = state_for(profile, income)
    }

    transition after = apply_salary_change(before, context)

    derive policy_assessment {
        before = assess_policy(before, context)
        after = assess_policy(after, context)
    }
    derive loss_ore = policy_assessment.before.resources_ore -
        policy_assessment.after.resources_ore

    admit supported when policy_assessment.before.supported
        && policy_assessment.after.supported
        && permitted(before, after, context)

    find cliffs = violations of resources_never_fall(
        policy_assessment.before,
        policy_assessment.after
    )
    find all_options = all
}

? analyze income_cliff_answers from explore income_cliffs {
    view admitted_summary from find all_options {
        group all
        aggregate [cases = count_distinct(case_id)]
        select [cases]
        updates closure_gated
    }

    view cliffs from find cliffs {
        each case
        select [case_id, context, profile = before.profile, before, after, loss_ore]
        updates monotone
    }

    view case_summary from find cliffs {
        group all
        aggregate [
            cases = count_distinct(case_id),
            affected_profiles = count_distinct(before.profile)
        ]
        select [cases, affected_profiles]
        updates revisable
    }

    choice worst_cliffs from find cliffs {
        partition all
        measure [loss_ore]
        choose all maximizing loss_ore
        updates revisable
    }

    view worst_cliff_cases from choice worst_cliffs {
        each case
        select [case_id, before, after, loss_ore]
        updates closure_gated
    }

    explain cliff_paths from find cliffs using policy_assessment

    view mechanism_summary from explain cliff_paths {
        group all
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            explained_cases = count_distinct(case_id)
        ]
        select [
            mechanisms,
            raw_signatures,
            execution_profiles,
            explained_cases
        ]
        updates revisable
    }

    view loss_bins from explain cliff_paths {
        group by [
            bin_start_ore = floor_to_bin(loss_ore, 5_000)
        ]
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            cases = count_distinct(case_id)
        ]
        select [
            bin_start_ore,
            mechanisms,
            raw_signatures,
            execution_profiles,
            cases
        ]
        updates revisable
    }
}

? publish income_cliff_evidence from analyze income_cliff_answers {
    emit view admitted_summary
    emit view cliffs
    emit view case_summary
    emit view mechanism_summary
    emit view loss_bins

    observe support one_mechanism_support
        from explain cliff_paths
        for mechanism "<StructuralMechanismId>"

    materialize support cases one_mechanism_cases
        from explain cliff_paths
        for mechanism "<StructuralMechanismId>"
        using values from view cliffs

    materialize support starters one_mechanism_starters
        from support cases one_mechanism_cases
}
```

This is a semantic sketch, not a claim that the current parser accepts every
token shown. The implementation now accepts `given`, `vary`, `let`,
`transition after`, and zero or more named finds directly. It spells `admit`
as scoped `where`, `view` as in-query `results`, `choice` as `choose` inside a
result, `explain` as `mechanisms`, and publication readers as trailing in-query
declarations. Result consumers and direct mechanism consumers name their find
explicitly; a mechanism may instead name an earlier FIND-backed result's exact
chosen cases. There is no implicit selected question. First-class `derive`, `ChoiceId`,
separate `? analyze`, and separate `? publish` remain the target.
New examples should use one surface intentionally and label transitional files
as such; they must never imply two supported public grammars.

The endpoint derivation is named because admission, questions and views may all
consume the same checked values. Physically evaluating a pure endpoint once is
an optimization; semantically the resolved expression and dependencies are part
of every downstream identity that uses it. `explain ... using
policy_assessment` remains a fresh traced Before/After evaluation of the
derivation's resolved callable. A cached derived value can establish
classification, but it cannot stand in for causal replay.

The quotient bindings in the analysis sketch are the structural result
surface. An authored mechanism-incidence row exposes typed `signature_id`,
`structural_mechanism_id` and `execution_profile_id` values. A row is not
evaluated until its durable signature-to-structure assignment exists, and the
exact result-input seal binds both the raw incidence root and structural
quotient root. Consequently `count_distinct(signature_id)` is explicitly a
**raw-signature** summary, while a true mechanism count uses
`count_distinct(structural_mechanism_id)`.

An ungrouped replay view names the explanation explicitly and uses incidence
grain rather than case grain:

```runa
view raw_cliff_paths from explain cliff_paths {
    each incidence
    select [
        case_id,
        transition_id,
        signature_id,
        structural_mechanism_id,
        execution_profile_id
    ]
}
```

The profile producer owns structural coherence. It can join municipality,
church status, household form, commute facts, income composition and pension
facts without pretending that those catalogs are independent switches. The
income producer is lateral: each profile may have a different supported finite
coordinate set. `profile_space` names finite catalogs and bounds for that
producer; it is not a preselected person whose other facts Explore fixes.

The checked artifact must make that breadth inspectable rather than trusting a
promising helper name. Its `RelationId`-scoped source-construction manifest is
deliberately FROM-only: it follows the checked `from` producer closure and
recursively classifies canonical Context and Before field paths, plus every
reachable immutable producer input. Each path/input is a varied finite
dimension, a value derived from earlier dimensions, an explicit singleton or
source restriction, a proof-backed exactly irrelevant input, or a reported
model-coverage gap. A literal or immutable top-level constant buried inside
`state_for` is therefore visible conditioning, not an invisible default. An
unsupported nested composition becomes a gap at the affected path; it never
disappears and never causes Explore to invent a replacement dimension. This is
derived evidence about the ordinary source program and does not require another
clause in the query language.

A sibling `SuccessorCoverageId` follows the transition producer and recursively
classifies After paths, including ordered transition-block dependencies. It is
separate because a dependent successor fiber is not another source dimension;
the pair nevertheless covers the complete RelationId schema.

Exact irrelevance remains evidence-gated. The coverage algebra reserves that
classification, but the source manifest does not manufacture an irrelevance
certificate when no producer exists. A declared dimension retains its support;
an unmodeled or unproved path is a gap. Coverage of inputs used by admission and
`find` requires separate siblings under `AdmissionId` and `QuestionId`; each
view, choice and explanation gets one generic `AnalysisNodeCoverageId` for
dependencies it adds. Those components qualify only their own layer, cannot
mutate source/successor coverage and compose into the result's
`CoverageBundleId`.

Relevance is an execution optimization, not permission to erase population.
If church status is proved irrelevant to the question and mechanism observer,
the search may evaluate one behavior representative while retaining the exact
support of both statuses. The completed case and affected-profile counts still
range over the declared relation. Intersections that are relevant only in
combination remain separate decision cells, which is where quirky profiles can
emerge without a blind Cartesian evaluation.

In the target example above, the exact finite range is end-exclusive, so a
`+1 DKK` question has lower endpoints through 1,499,999 and a final successor
at 1,500,000. That is the eventual exhaustive one-krone cliff relation, not the
first broad execution milestone.

The first broad audit is deliberately a different, coarser RelationId. For
each declared coherent profile it uses lower endpoints
`0, 1_000, ..., 1_499_000 DKK` and the transition `g -> g + 1_000 DKK`, ending
at exactly 1,500,000 DKK. It therefore has exactly **1,500 edges per profile**
and **1,501 reusable endpoints per profile**. Exact closure of that relation
would be exact only for those 1,000-DKK endpoint transitions and their observed
mechanisms; it would neither enumerate nor certify the 1,500,000 adjacent
1-DKK cliffs per profile. The horizon is a declared question bound, not a
suspected threshold. This is a planned audit contract: no broad run has begun,
and this paragraph makes no claim that the target Explore/Analyze/Publish
surface can execute it yet.

The first honest runnable Personskat audit may be a **conditioned bootstrap**:
choose one source-backed coherent profile explicitly, vary lower annual income
coordinates over `0..<200_000` DKK and let the final `+1 DKK` successor reach
exactly 200,000 DKK. Its source-coverage manifest must report every singleton
and restriction. Completion then means exact over that declared one-profile
relation; it does not mean exact over all persons, municipalities or profile
constructors. This is a legitimate first audit of the durable pipeline, not a
population result. In the transitional parser the bootstrap binds `before in
range(0, 200_000)` directly after its singleton context. It does not introduce
an auxiliary `income` dimension merely to copy it into `before`: that redundant
dependent singleton would add roughly one source fiber, receipt and traversal
edge per income without adding a case or proving anything new.

An earlier **small population calibration** at the same income horizon remains
a separate useful step, but it is not the first broad audit defined above. It
ranges over a declared genuinely multidimensional coherent profile relation,
publishes its coverage manifest and closes every source, successor, admission
and FIND frontier. Either calibration may produce an exact empty cliff relation
in the current model. Empty is useful evidence only when all frontiers required
by that audit close; it is never inferred from seeing no sampled case. A
separate tiny synthetic fixture deliberately contains a shared nonempty
mechanism so the integration milestone exercises case-to-mechanism incidence
and a post-mechanism view even when Personskat below 200,000 DKK is empty.

Those components are now connected for the conditioned bootstrap; its first
attempt nevertheless stopped during preparation before semantic replay. The
conditioned bootstrap may use concrete enumeration if it fits the resource
envelope. Any multidimensional calibration remains gated on the proof portfolio
so it does not blindly multiply profiles by income where exact cells are
available.

`transition after in successors(before, context)` is the target multivalued
form. A household planning query uses it when one coherent source has zero, one
or many candidate After plans. The equivalent ordered `transition after {
vary ...; let ...; yield ... }` block exposes dependent successor bindings and
requires exactly one typed `yield`; it lowers to the same set-normalized finite
fiber. This is the decisive generalization beyond a fixed mixed-radix product.
The current parser accepts the simple forms `transition after =` and
`transition after in`; the ordered transition block remains to be implemented.

## What each clause owns

| Target clause | Owns | Does not own |
|---|---|---|
| `from { given/vary/let }` | the finite source world, dependencies and producer lineage | case validity or findings |
| `transition` | the exact-finite successor relation | the rule mechanism explaining an endpoint |
| `derive` | reusable checked pure values, including named Before/After assessments | admission, question or causal-trace authority by itself |
| `admit` | one shared admission classification | source identity or a particular question |
| `find NAME` | one named all/matching/violating case relation and its `QuestionId` | presentation, choice or mechanism identity |
| `? analyze` | a typed dependency DAG rooted in explicit named `find` relations, with checked `choice`, `explain` and `view` edges | source construction, direct admission targeting or publication scheduling |
| `view NAME from ...` | projection or closed aggregation and its public schema | winner membership in another choice |
| `choice NAME from find NAME` | partition, eligibility, objectives, tie policy and chosen case membership | display projection or causal explanation |
| `explain NAME from find/choice NAME using DERIVATION` | fresh endpoint replay and signature incidence | proof that an optimum or Pareto member wins |
| `? publish` readers | authorized materialization, compact support and graph attachments | a new answer-DAG node or broader semantic claim |
| invocation limits | safe scheduling and pause behavior | semantic query identity |

No profile fact is implicitly fixed. When a question intentionally conditions
on Copenhagen, a visible `given`, a restricted producer or a constant reachable
through its checked closure declares that smaller source world and changes
`RelationId`. An `admit` condition instead classifies cases in the already
declared relation and changes `AdmissionId`. An optimizer may push the predicate
physically but may not move it semantically.

`given` is source-independent conditioning: it may use sealed module values and
pure helpers, but not an earlier binding from the same `from` relation. Use
`let` when one singleton value is computed from earlier source bindings. Both
`vary` and `let` are ordered and may depend only on earlier bindings; this keeps
coverage from misreporting a varied-dependent value as a fixed condition.

Every analysis node names its input. `from find cliffs`, `from choice
worst_cliffs` and `from explain cliff_paths` are different typed relations;
there is no omitted `from` and no ambient `selected`. Case views use `each
case`; raw explanation views use `each incidence`. Auxiliary producer bindings
remain authenticated lineage rather than implicit source-row columns. The
checker rejects a case/incidence grain mismatch rather than silently changing
row identity.

`partition all` or `partition by` defines the competition domain for `choice`
without collapsing its case rows. `group all` or `group by` defines closed
aggregate rows for `view`; only that grouped grain admits `aggregate`. The
current implementation has one explicit aggregate reducer,
`NAME = count_distinct(EXPR)`. Group, measure and aggregate declarations
introduce ordered intermediate names. `select` declares a view's public output
schema. It does not participate in another node's `ChoiceId` merely because a
view later projects chosen rows.

## Design lineage: borrow contracts, not costumes

Explore is not SQL, Prolog, TLA+, Alloy, Zanzibar or a constraint solver with
Futuruna keywords. It borrows one strong idea from each while keeping one
language and one evidence model:

- [TLA+ action semantics](https://lamport.azurewebsites.net/pubs/spec-book-chap.pdf)
  motivates treating a question as a predicate over one typed Before/After
  step, without turning Explore into an unbounded temporal behavior language.
- [Alloy's bounded relational analysis](https://alloytools.org/spec.html)
  motivates an exact declared finite universe, explicit scope and concrete
  witnesses; unlike Alloy, Explore evaluates Futuruna's canonical rule model
  and preserves a durable open frontier.
- SQL's named relations and lateral joins, together with stratified Datalog,
  motivate explicit `from`, `find`, `view`, `choice` and `explain` edges. Explore
  uses set semantics, typed grains and closure-aware negation rather than SQL
  bags, nulls or Prolog negation-as-failure.
- [MiniZinc's separation of parameters and decision variables](https://docs.minizinc.dev/en/stable/spec.html)
  motivates the visible distinction between `given`, `vary` and `let`; the
  coherent dependent relation, not an implicit Cartesian variable box, remains
  the source of cases.
- [Zanzibar's relationship graph and consistency token](https://research.google/pubs/zanzibar-googles-consistent-global-authorization-system/)
  motivates stable content-addressed relationships and cohesive snapshot reads.
  Explore's EvidenceToken is likewise a consistency handle, never a proof of
  completeness and never a ResumeCursor.
- [Provenance semirings](https://www.cs.ucdavis.edu/~green/papers/pods07.pdf)
  and [Souffle provenance](https://souffle-lang.github.io/provenance) motivate
  factorized explanation/support structure: cases point to shared mechanism
  definitions instead of cloning one trace per witness.
- [DBSP](https://www.vldb.org/pvldb/vol16/p1601-budiu.pdf) and differential
  dataflow motivate incremental maintenance and explicit revisions. They do
  not relax the rule that an optimum, exact distinct aggregate or negative
  result needs its declared input closure.

The resulting category is best described as a **versioned, bounded relational
transition-query language with provenance-carrying, incrementally refined
results**. That description is a design test: a borrowed feature belongs only
when it strengthens boundedness, typing, provenance, incrementality or honest
closure. Superficial syntax resemblance is not a reason to add it.

### The minimum sufficient kernel

Broadening is useful for discovering possibilities; reduction decides what
survives. The target architecture keeps only five irreducible responsibilities:

1. **Explore declares the finite world:** explicit source roles, one typed
   Before-to-After relation, total admission and named questions.
2. **Analyze derives meaning:** typed views, choices and fresh causal
   explanations in one acyclic dependency graph.
3. **Publish exposes authorized evidence:** attachable authorization,
   retention, encoding and artifact locations that cannot redefine the
   question, analysis or the privacy schema already fixed by `view select`.
4. **The stream proves what is known:** stable identities, a resumable frontier,
   explicit coverage/bounds/closure and incidence from cases to shared
   mechanisms.
5. **Physical optimizers satisfy obligations:** enumeration, endpoint reuse,
   event-guided splitting, binary prioritization, decision cells, LP/SMT and
   distributed execution remain replaceable backends beneath one certificate
   interface.

Every proposed construct must pass the following reduction test:

| If it changes ... | It belongs in ... |
|---|---|
| which Context/Before/After cases can exist | Explore / RelationId |
| admission or the truth population of a named question | Explore / AdmissionId or QuestionId |
| comparison membership, causal replay or a derived answer relation | Analyze / ChoiceId, MechanismRequestId or ViewId |
| only authorization, encoding, retention or artifact location | Publish |
| only work order, parallelism, limits or pause policy | invocation/runtime state |

If a feature fits none of those rows, it is presumptively accidental. If one
concept spans two rows, split its identities before adding syntax. Probes,
implicit `selected`, raw discovered hashes in semantic source, consensus/mining
vocabulary and authored binary-search hints fail this test. Their useful parts
already live in scheduler policy, evidence identity or explicit publication.
Conversely, removing finite scope, total classification, stable case identity,
closure-aware results, composed coverage or case-to-mechanism incidence would
make the feature unable to support its promised claims; those are not optional
complexity.

## The identity ladder

The implementation should make each boundary concrete before building the next:

```text
RelationId
  -> AdmissionId
    | -> QuestionId(A)
    |      | -> ViewId(case input)
    |      | -> ChoiceId -> ViewId(chosen-case input)
    |      |       `-----> MechanismRequestId
    |      `-------------> MechanismRequestId
    |                         `-> ViewId(mechanism-incidence input)
    ` -> QuestionId(B), QuestionId(C), ...
```

- `RelationId` seals normalized FROM+TRANSITION semantics, stable model/type
  owners, schemas, set normalization and lineage. It excludes names, ADMIT, FIND,
  views, mechanisms and execution policy.
- `AdmissionId` adds the normalized ADMIT conjunction and every transitive
  derivation dependency it consumes.
- Each `QuestionId` adds one named FIND all/matches/violations expression and
  its transitive derivation dependencies. Several QuestionIds may share one
  AdmissionId; their declaration names remain addresses rather than identity.
- `ChoiceId` adds its explicit QuestionId input, partition, eligibility,
  required measures, objectives and one/all/Pareto cardinality. It excludes
  later display fields.
- `ViewId` adds its explicit typed input, grain, projection or grouping,
  aggregates and public schema. Adding a display-only field changes that view,
  not the `ChoiceId` whose members it projects.
- `MechanismRequestId` adds its explicit QuestionId or ChoiceId target, fresh
  endpoint replay derivation and signature normalization. It does not target a
  projection merely because that projection displays the same cases.

Names are source addresses. Resolved semantic IDs, not spellings, form
dependencies. Renaming a query/view/request and updating references must not
rename its semantic layer.

The checker consequently keeps two coordinates. Query-inclusive analysis IDs
address declarations and expression sites in the exact checked source. A
separate occurrence-sensitive model-owner key seals only the recursively
resolved nominal type/schema interface. Explore declarations, rule and method
bodies, observer logic and execution policy cannot rename that owner: reachable
transition logic enters `RelationId`, observer logic enters
`MechanismRequestId`, and scheduling remains operational state.

`RelationFrontierRoot` is distinct from `RelationId`: it commits discovered
rows, provenance and the exact open/sealed producer frontier without claiming
completion. When every source and successor frontier seals,
`RelationContentRoot` commits the completed extensional relation. Eager and
incremental implementations of the same relation share one RelationId and
converge to one completed content root.

`SupportObservationDemandId` and publication-consumer identities sit beside
this ladder rather than below it. A compact support demand
binds one resolved request, structural subject/facet and optional enclosing
mechanism while excluding the declaration name and position. The demand-set ID
is the sorted set of unique demand IDs, so aliases and declaration order do not
rename it. Neither ID enters the analysis-graph digest or any upstream semantic
identity; declarations are checked readers, not new rule-graph nodes.

Stable case identity contains no rank:

```text
SourceKey    = H(RelationId, Context, Before)
SuccessorKey = H(RelationId, SourceKey, After)
CaseId       = H(RelationId, SourceKey, SuccessorKey)
```

Duplicate producer paths union provenance. Duplicate successors under one
source collapse. One canonical Context/Before/After triple has one CaseId within
the relation. Carl at one income and John at another remain different cases and
transitions, while replay may map both to one complete mechanism signature.

An exact symbolic run also has an `EvidenceRoot`. It commits certified support
cells, their partition proofs and the remaining open obligations. It is not a
shortcut spelling for an extensional `RelationContentRoot`: that latter root is
published only when the canonical concrete relation has been materialized.
Exact scalar counts and closed grouped answers may therefore precede exhaustive
case serialization without weakening their proof status.

## Query contract and evidence checklist

Every authored question and every review of its output must answer the same five
questions. This checklist is part of the target product contract, not report
decoration:

1. **Query contract:** What exact finite relation does `from` plus `transition`
   declare? Which inputs are `given`, which are `vary`, which are `let`, what
   set-normalization occurs, and which named finds exist over the shared
   admission?
2. **Composed coverage:** What source, successor, admission, named-question,
   choice and explanation coverage has been proved? A downstream layer may
   narrow or qualify upstream coverage; it may never silently broaden it.
3. **Status:** What are the separate execution, semantic, materialization and
   count statuses? Which frontier or missing proof justifies `open`,
   `unavailable`, `lower_bound`, `interval` or `capacity_limited`? Empty and
   optimum answers are exact only after their required input closes, and
   `caught_up` says only that an artifact matches its EvidenceToken.
4. **Update:** Is this authenticated event `add`, `retract` or `seal`? Monotone
   nodes only add; revisable nodes may atomically retract old rows and add
   replacements; closure-gated nodes expose no rows before closure. The
   evidence journal is append-only even when the public relation it describes
   is revisable.
5. **Evidence token:** Does every row, aggregate, choice observation and support
   point carry or reference a compact token binding its journal contract,
   semantic graph, coverage and canonical evidence/projection roots—without an
   operational head, sequence or work position? Can the reader distinguish
   that cohesive-read token from the separately typed operational ResumeCursor
   and presentation-page cursor?

The source-construction manifest answers only the first coverage layer. A
composed answer envelope links the independently authenticated coverage and
status of RelationId, AdmissionId, the named QuestionId, optional ChoiceId or
MechanismRequestId, and the final ViewId. Publication policy may hide values;
it must not hide which semantic layer remains open.

The compact target vocabulary is:

```text
execution:       running | paused(reason, ResumeCursor) | stopped(reason)
semantic:        open | exact | unavailable(reason) | error(reason)
materialization: not_requested | pending | caught_up | capacity_limited |
                 unmaterialized | error
count:           unknown | lower_bound(n) | interval(lower, upper) | exact(n)
```

`paused` is exactly the resumable state; `stopped` has no ResumeCursor.
`caught_up` is relative to one EvidenceToken and never implies `exact`.

## The adaptive outcome DAG

The concrete executor is the universal fallback, witness checker and on-demand
materializer. It must not be the only evidence representation. The scalable
path is a factorized outcome DAG:

```text
checked finite support
    -> dependency slice and partial evaluation
    -> categorical / interval / congruence decision cells
    -> uniform admission + finding + measure + mechanism leaves
    -> request-local complete-signature support and starter projections
    -> unresolved cells split, prove with a stronger backend, or enumerate
```

Every `SupportCell` carries its exact support and cardinality, a resumable
materializer and a proof that any children form a disjoint complete partition.
Uniform results attach only after a certificate covers every member. A saved
example is one member of that cell, never evidence that all unvisited members
behave like it.

The same certified leaf can feed two deliberately different target-support
indexes:

```text
support_cases(q, m) = S(q,m)
    = { (Context, Before, After) cases in target(q) assigned to mechanism m }

support_starters(q, m) = P(q,m)
    = distinct_(Context, Before)(support_cases(q,m))
```

Support cases are correlated case/transition members and count CaseIds. Support
starters are the distinct source projection and may be strictly fewer whenever
one starting world has several supported After successors. Neither is the
rule's universal legal trigger population: both are conditioned on the explicit
explanation target. The full support-case index is keyed by
`(MechanismRequestId, SignatureId)`; the starter index is derived from it. The
latter must retain dependent/correlated cells or an equivalent checked predicate;
independent marginal bounds are display summaries, not a replacement support
cell. Projection can merge several After fibers and can destroy disjointness,
so it needs its own deduplication or projection certificate and its own
closure-aware count.

Keep those indexes in a mechanism-analysis support catalog rather than adding
post-FIND evidence back into the base support catalog. The base catalog must be
able to seal the selected population before mechanism replay; requiring it to
contain the later mechanism assignment would create a dependency cycle. The
analysis catalog can reuse `SupportCellId` and checked proof receipts while
maintaining its own inner cells, outer frontier, projection receipts and
case/starter count intervals.

Cell partitioning and claim proof are separate edges. Refining an obligation
replaces its parent in the active frontier with one same-claim obligation per
partition child; it does not pretend the parent had one uniform answer. The
children may close to different classifications, values or mechanism
signatures. Exact completion is therefore “all active obligation leaves are
proved,” while superseded parent obligations remain auditable history.

Root obligations are declared, never inferred from whichever records happen
to have arrived. Every obligation must be reachable from those roots. A parent
may either receive direct evidence or be refined, never both. Resume cursors
remain checkpoint data outside `EvidenceRoot`, so pausing at different physical
positions cannot rename otherwise identical mathematical evidence.

Use algorithms as a checked portfolio rather than surface syntax:

- dependency slicing, partial evaluation and before/after delta reuse first;
- affine, interval and congruence reasoning for cheap whole-cell closure;
- source guards and discovered events as preferred split points;
- binary search only within a proved monotone region;
- decision diagrams for categorical dispatch and low-dimensional integer
  polyhedra or Presburger counting for suitable arithmetic fragments;
- MILP and SMT for bounds, witnesses and residual refinement; and
- batched canonical evaluation for everything that resists proof.

This separates the semantic question from the physical optimizer. In the worst
case every cell becomes a singleton; on structured policy graphs one terminal
cell may cover a very large profile-and-income population exactly.

### Complexity and coverage contract

Use separate symbols for the logical plan and the materialized prefix:

- `B`, `E`: FROM stages and declared dependency edges;
- `P`: accepted concrete source-prefix traversal edges;
- `S`: distinct set-normalized source rows;
- `N`: distinct constructible cases;
- `A`, `Q`: admission and FIND decision records;
- `q`: canonical unique semantic questions in one classified sweep, and `R_c`:
  maximal joint-outcome runs retained for classified chunk `c`;
- `W`: retained work-DAG nodes after compaction;
- `C`, `O`, `M`: support cells, proof obligations and accepted proof evidence;
- `R`: evaluated result-input rows, `G`: result groups, `K`: candidates in one
  choice group;
- `I`: mechanism-incidence rows; and
- `J`, `D`: durable journal frames and immutable journal segments.

The checked support plan itself is `O(B + E + C)` space and does not allocate a
node per concrete prefix. Its dependency-key recipes identify the future cache
key for a finite fiber. The current concrete enumerator does **not** yet own
that cross-prefix cache: resuming a source work node replays its prefix and
reopens each required earlier fiber. For source depth `d`, that reconstruction
is `O(d)` fiber evaluations plus the cost of canonicalizing those fibers. A
bounded quantum reuses its current opened fiber, but different work nodes can
still repeat equivalent evaluation. Dependency-tuple memoization can reduce
that repeated work toward `O(F)`, where `F` is the number of distinct opened
dependency tuples; it is a required optimization before broad profile
widening, not a current complexity claim.

The current nonempty classified sweep remains output-sensitive. It performs
`O(N)` endpoint and admission evaluations plus source-prefix work, then
evaluates every unique semantic question for each admitted case. With question
predicate costs `f_i`, that work is `O(N sum_i f_i)`; authored aliases of the
same normalized question add no work. The sweep binds one canonical ordered
`QuestionId` set and represents an admitted outcome as a packed joint decision
mask in that order. Adjacent cases are run-length encoded only when their full
outcome agrees: rejection, or admission with the same complete mask.

For chunk `c`, compact retained decision state is
`O(R_c ceil(q / 8))`; across chunks it is
`O(sum_c R_c ceil(q / 8))` plus `O(q)` exact counters. This is a compression
opportunity, not a better worst-case bound: adversarial outcomes can force one
run per case and require `Omega(Nq)` decision bits. An admitted run carries one
admission-evidence item and one selection-evidence item per semantic question;
admission is not duplicated for question fan-out.

Cases selected by at least one question are materialized once as a shared
any-selected union payload. That payload records its canonical selected
`QuestionId` subset, and question-specific catalogs and public support are
projections of it rather than duplicate case rows. The native V2 classifier
and region-proof accelerators remain exact-one. They do not choose an authored
first question when `q > 1`; the shared concrete classifier handles the full
vector. A zero-question query keeps the concrete traversal fallback because
there is no decision vector to classify. This is the correctness floor.
Current hot paths are:

| Current operation | Current time | Retained or peak memory |
|---|---:|---:|
| keyed source/case/classification/work/evidence insert | normally `O(log X)` plus payload validation | one canonical record; singleton successor and provenance collections stay inline and promote only when they actually branch |
| resume a depth-`d` source prefix | `O(d)` fiber evaluations and set normalization | current prefix plus one materialized fiber |
| enumerate one configured quantum | `O(k)` evaluator steps after the fiber opens | `O(k)` unapplied events; production fused work cold-starts at `k = 1`, then adapts toward five seconds with `k <= 256` |
| reconstruct the verified case-chunk partition | `O(B)` once per cold journal replay; later slices/chunks/runs use indexed lookup in the replay-derived opaque authority | one `O(B)` partition/binding cache, currently `B = 782` for the 200k audit |
| accept one classified chunk | `O(rq log C)` worst-case keyed validation/append for `r <= 256` joint-outcome runs, including one admission item per run, one selection item per admitted run/question, causal roots and bounded addressed-chunk reverification | `O(rq)` exact-key undo/validation state; the durable outcome masks use `O(r ceil(q / 8))` plus `O(q)` counts, with no accumulated support-catalog clone, proof scan or whole-partition rebuild |
| derive public classified counts | `O(C log C + M log C)` over the case-root-reachable support topology and facts | `O(C + M)` topology/key indexes while all semantic cells and evidence remain borrowed; no support snapshot or payload clone |
| close classified support | `O((C + O + M) log (C + O + M))` full validation plus canonical hashing at each crash-safe seal boundary | derived key/ID validation sets only; no journal or support snapshot clone |
| accept one shared any-selected run | `O(kq log N)` worst-case collision/classification preflight and sparse per-question membership merge for `k <= 256` union-selected cases | one `O(k)` batch-local relation/admission payload plus selected-question masks and sparse per-question memberships; no duplicate case row per matching question and no relation/admission/FIND prefix clones |
| finish source traversal | `O(P + S + E)` ordered reachability/root validation over prefixes, sources and traversal edges | current `O(P + S + E)` reachability scratch; the terminal receipt itself is a fixed 212-byte body |
| relation/admission/FIND closure | `O(N + A + Q)` coverage validation across all unique questions | relation rows plus `O(A + Q)` decisions; worst-case `Q = Nq` |
| result evidence or projection-record insert | `O(log R)` in canonical indexes | one bounded record plus its reverse/index entry |
| publish an ungrouped or choice-bearing row view | `O(R)` deterministic reducer/projection rebuild, once per process resume | current `O(R)` ephemeral row-state reducer plus cached bounded records; durable terminal state is constant-size |
| resume bounded result projection publication | one `O(P)` cold prefix validation, then `O(delta)` for each newly durable suffix; sealed evidence-root checks are `O(1)` | one `(validated length, prefix root)` cursor per active view; no prefix-root array |
| publish a grouped no-choice view, including `count_distinct` | `O(R log G + sum(aggregate_count * group_size * log group_size))` after fresh equality-checking all `R` durable rows | `O(R + G)` borrowed references and up to `O(R)` exact distinct members per aggregate pass; no second owned contribution catalog or per-row binding map |
| optimize one group | `O(K)` | `O(K)` objective candidates |
| current Pareto choice | `O(K^2 * objectives)` worst case | up to `O(K)` survivors |
| register one explicit support-observation slice | `O(log D)` authenticated registration, followed by resumable backfill rather than an eager union | one registry entry, pending/dirty/unsealed membership and one subject/route watcher |
| backfill one explicit observation demand | `O(min(256, remaining assignments))` membership checks per quantum | matching signature watchers for that bounded page; no partially ready slice |
| accept new structural/support evidence | `O(log M + incident_slices * log D)` dirty-set updates | the affected automatic mechanism plus ready explicit incident slices; never all demands |
| work compaction | `O(W log W)` conservative scan/root derivation | bounded removal set; defaults trigger at 8,192 completed nodes and remove at most 4,096 |
| explicit full journal snapshot export | `O(P + S + N + A + Q + W + C + O + M + R + I)` | clones/materializes the published catalogs by request; terminal analysis close preflights and then moves its catalogs instead |
| durable append | `O(frame_bytes)` plus an occasional segment install/readback | at most one configured segment buffer; default 4 MiB |
| durable open/replay | `O(total durable bytes + J + D log D)` | folded semantic catalogs plus one segment/frame view; no second entry history in streaming mode |

`RelationCatalogBuilder` stores sources in a `BTreeMap<SourceKey, SourceDraft>`.
Each source uses an adaptive canonical successor map: no allocation while
empty, one boxed key/value for the ordinary singleton fiber, and a
`BTreeMap<SuccessorKey, SuccessorDraft>` only after a second distinct successor
actually branches the fiber. Separate successor- and case-claim maps detect
identity collisions and provide direct CaseId lookup. Provenance support uses
the analogous empty/one/two inline representation and promotes to `BTreeSet`
only at the third distinct member; iteration and hashing remain sorted.
Admission and FIND are `BTreeMap<CaseId, Decision>` catalogs. The work frontier
is a `BTreeMap<WorkNodeId, Record>` plus ordered open/runnable sets, reverse
dependency sets and incomplete-dependency counts, so runnable lookup and
dependency updates do not rescan the whole DAG. Completed leaf records are
removed only through a rederived authenticated compaction receipt.

Support evidence uses canonical maps for cells, partitions, obligations,
refinements, evidence, cursors and layer registrations, with ordered sets for
roots and active/open/proved leaves. Result evidence has a row-keyed canonical
map and an evidence-ID membership set for collision detection; it does not
repeat the wide row identity as an unused reverse-index value. All-`None`
staged SELECT/objective arrays retain only their logical length, while mixed
arrays remain dense and journal/hash iteration remains byte-identical. CaseId
is a content hash, not an arrival watermark: a later selected case may sort
below every earlier CaseId. The
journal fold therefore rebuilds a non-semantic selected-discovery vector beside
the canonical question map. Invocation-local ordinal cursors normally stream
only the new suffix in `O(k)`; after restart or an unusual fork they walk each
durable prefix position once. Mechanism targets, terminals and incidence-result
rows use the analogous discovery-order indexes. These indexes never enter a
question, incidence or answer root; exact seals still validate the canonical
sets and counts. Terminal publication freshly re-evaluates and equality-checks
durable inputs. Grouped views without `choice` then reduce over borrowed durable
contributions, so they do not rebuild a second contribution catalog or retain
per-row base bindings. Ungrouped and choice-bearing views still rebuild the
full ephemeral reducer. The analysis catalog is a `BTreeMap` in dependency-plan
order whose result and mechanism layers own their subordinate evidence
catalogs.

The production journal fold uses `new_streaming`: it retains the folded state,
sequence and head, but not a second `Vec` of every entry. The segmented store
keeps an `O(D)` descriptor list and one bounded segment buffer. Its defaults are
4 MiB per segment, 1 MiB per frame and 65,536 frames per segment; hard limits
remain defensive capability bounds, not recommended operating targets. In
contrast, the in-memory test/history constructor deliberately retains `O(J)`
entries.

Some roots remain full exports. Non-consuming relation, support, work and
analysis snapshots walk or clone their catalogs. Consuming terminal analysis
close first derives and validates the same root over borrowed builders, then
moves the already ordered semantic maps and vectors into their snapshots
instead of cloning them. Result publication no longer writes the complete
closed view as one terminal frame: it journals one bounded row/group/chosen-row
projection record at a time and then a compact counts-and-roots closure. The
ordinary stream report reduces classification counts through a borrowed
case-root support view. Exact support closure likewise validates and hashes the
borrowed builder, retaining only derived key/ID sets, so neither boundary
materializes a full support or journal snapshot. The obligation-frontier seal,
catalog seal and authenticated closed root are separate durable quanta in that
order. The
grouped/no-choice publisher performs one `O(R)` fresh verification pass after
process restart but retains only borrowed contribution/member references,
distinct sets and projected output. Ungrouped or choice-bearing publishers
still perform an `O(R)` owned reducer rebuild, so resource admission must charge
that peak whenever such a view is present.
The long-run target remains incremental authenticated indexes and
incrementally maintained reducers, not repeated `O(total state)` observer
work.

No optimizer erases the theoretical hard case. Arbitrary finite rule programs
can encode SAT-like search, exact model counting can be `#P`-hard, and a Pareto
frontier can contain every candidate. LP, MILP, Presburger counting, decision
diagrams, SMT/CEGAR, monotone binary search and SIMD evaluation are therefore
portfolio backends behind the same proof-obligation interface. Each is chosen
only for the fragment it can certify; none changes the meaning of Explore.

Coverage has independent layers. Exact exhaustion or accepted proof closes the
explicitly declared relation. The checked FROM-only source-construction
manifest separately answers whether that relation spans every recursive
Context/Before path and producer choice relevant to a broader claim such as
“all encoded persons.” Admission, `find` and observer input manifests are
identity-scoped siblings rather than extra source facts. A coverage gap or
explicit singleton qualifies the applicable breadth claim but does not turn an
exact count over the smaller declared relation into a lower bound. Conversely,
a broad manifest does not prove a mapped projection injective or a result cell
uniform.

Closure is layered and cannot be inferred downstream:

1. the relation is extensionally complete only after source enumeration and
   every discovered source's successor fiber are sealed;
2. admission is exact only after every CaseId has one decision;
3. FIND is exact only after every admitted CaseId has one selection decision;
4. certified support is exact only when every declared active obligation leaf
   has accepted evidence and all partition/refinement fronts are closed; and
5. an Analyze node is exact only after every semantic input and reducer it
   depends on closes; an explanation additionally requires complete target
   replay and totality evidence with no unavailable endpoint. Publish catch-up
   is a separate materialization status and never gates semantic exactness.

Before the corresponding seal, observed population counts are lower bounds.
An exact-empty certified population is already supported by the post-FIND
bridge and selected-result driver. Positive certified SupportCells are not yet
accepted as weighted result rows: the current result catalog rejects that path
until multiplicity, distinctness, grouping and choice have a checked reducer
algebra. Positive results therefore still require concrete CaseIds today.
Unknown or deferred mechanism evidence is likewise never counted as zero.

The smallest sound positive-cell reducer is now explicit. If the certified
selected population is a disjoint union of fragments `S_i` with exact weights
`w_i`, a grouped view may consume those fragments without CaseIds only when
every group, measure and non-identity distinct expression has typed uniform
value evidence on each fragment. Then group membership and
`count_distinct(case_id)` are `sum(w_i)`; `count_distinct(expr)` is the exact
set cardinality of the certified per-fragment constants; and `having varies`
compares those constants. This first algebra supports grouped, no-choice views.
`each case`, arbitrary nonuniform projection, Pareto/optimization, mixed
concrete-and-cell coverage and transition-ID distinctness must refine or
materialize instead. A mechanism analogue maps each disjoint cell and its
weight to a typed uniform mechanism signature, producing a compressed
case-to-mechanism incidence edge without inventing a CaseId or TransitionId.

The durable structures mirror those distinctions: a factor/dependency DAG for
construction, set-normalized relation catalogs for extensional membership, a
reusable support-partition DAG, a typed obligation/refinement DAG for each
question, and derived result/mechanism DAGs. The ordered journal head commits
recovery history; arrival-order-independent semantic roots commit accepted
answers; work and cursor roots commit resumability. None substitutes for the
others.

The proof portfolio is deliberately narrower in implementation than in design.
Today the support planner represents independent/dependent finite factors,
singleton maps, mapped images, successor sums and exact structural emptiness.
The first producer proves a direct one-independent-Integer-axis,
singleton-Unit-context, quasi-affine successor and direct Boolean FIND formula.
It can close an exact-empty selected population in `O(E + T log T)` time and
`O(E + T)` memory, independent of the range cardinality. Its constant-size
artifact is not authority by itself: revalidation against the same checked
query and support plan is the only route to verifier-gated proof receipts.
The more general strategy also derives split coordinates and structural
interval partitions, but split candidates carry no classification authority
by themselves.

The conditioned Personskat query is outside that first theorem: its Context is
structured and FIND crosses checked helper/rule calls and field projections.
Personskat therefore still needs a checked rule-graph normalizer and proof
producer for piecewise arithmetic, rounding, caps and rule dispatch. There is
no current LP, MILP, SMT, Presburger, decision-diagram or
proved-monotonicity backend wired to discharge those obligations, and binary
search without such a proof is only scheduling. Residual support must remain
open or fall back to exact enumeration. This is the principal algorithmic
blocker for the multidimensional 200,000-DKK population audit; it does not
block an explicitly conditioned concrete bootstrap if that run fits the
resource envelope.

The bootstrap binds the income coordinate directly as `before in
range(0, 200_000)` and does not restate facts already established by its
finite producers as runtime `where` predicates. That range and `to after =
before + 1` already imply the nonnegative lower bound, the 200,000-DKK endpoint
and the transition equation. Keeping those structural facts in the relation
contract avoids one auxiliary source layer and three vacuous predicate
evaluations per case. A future optimizer should prove and erase equivalent
redundant guards in general; the first audit need not manufacture them.

A checked-expression execution plan now computes the immutable top-level
binding closure once for each borrowed expression occurrence in the selected
query. Runtime lookup uses pointer equality only inside that query's lifetime
and fails closed for an expression outside the plan; addresses never enter an
identity or artifact. This removes repeated reconstruction of the full binding
name set and helper dependency graph from the concrete hot path. After the
checked declaration snapshot is completely registered, the interpreter also
enables its existing lazy rule-family dispatch metadata cache. That cache
retains only immutable tier/order/local-name metadata: it caches no values,
skips no rule body, and preserves fresh Before/After mechanism traces.

For the direct conditioned query, the remaining black-box lower bound is still
important. WHERE and FIND together invoke the complete observation four times
per edge, but the dependency-certified adjacent-value memo now reduces a warm
uninterrupted scan to exactly `N + 1` distinct endpoint evaluations. For this
audit, if all edges pass admission, that predicts 200,001 misses and 599,999
hits; runtime telemetry must confirm it. A cold resume may repeat its boundary
endpoint, while selected mechanism replay remains deliberately fresh and
traced. The memo plan commits the exact pure closed rule-family
identity, candidate set and semantic closure, retains at most 2,048 entries and
16 MiB, and never enters journal or answer authority. An arguments-only
name-based memo would still be unsound while a callable may read its call-site
environment.

The multidimensional Personskat experiment exposed a second distinction. A
query-bound compiled classifier can accelerate already checked concrete source
rows without proving that the source-assignment product is injective into
`(Context, Before)`. It receives only authenticated finite binding values,
reconstructs the authored singleton bindings and transition, and proposes
ordered WHERE/FIND tags; the coordinator still mints every semantic identity
and event. Compact classified support is a stronger layer. It may skip
extensional cases or count weighted cells only after a separate checked
assignment-to-source injectivity proof and a genuine mixed-radix product
partition. Merely observing that an auxiliary dimension reaches `Before` is
not such a proof: `x * 0`, modulo and truncation are immediate counterexamples.

The same experiment also showed why endpoint reuse should ultimately be an
explicit relational observation DAG rather than only an interpreter cache.
Admissions, FIND, measures and publication may all depend on one compact
observation of the same `(ObserverId, Context, State)` endpoint. A SQL-like
`APPLY`/lateral observation node would evaluate that value once, let every
downstream query node reference it, and optionally authenticate it across
resume when publication is authorized. This does not replace fresh traced
mechanism replay: classification observations establish values, while replay
establishes causal rule-graph evidence. Until that surface exists, the bounded
memo and compiled classifier are operational accelerators, not a second query
semantics.

The corresponding physical target should share values and prefixes, cache
fibers by dependency tuple, retain only open obligations in the active
frontier, append immutable evidence through the segmented store, update Merkle
indexes incrementally and maintain view reducers incrementally. Resource
ceilings then protect the host rather than compensating for avoidable repeated
evaluation or whole-state publication.

## The observable stream

The journal is the recovery authority only when codec-validated semantic
entries are installed through the framed durable store; snapshots and JSON are
projections. The branch now has the streaming semantic fold, canonical bounded
codec, immutable segmented byte store and a memory-bounded durable coordinator.
The coordinator replays entries incrementally, binds every append batch to its
exact head, publishes only installed segment cursors, and poisons an in-memory
fold that advances before a later encode/install failure. This is
crash-prefix-safe for the codec's accepted event subset, not yet for every
proof-bearing analysis event.

Support proof receipts are the sharpest codec boundary. Recomputing their hash
checks identity but does not prove that the named verifier accepted the proof.
The region proof producer now emits a replayable proof artifact and can rerun
the verifier under the exact checked query/plan, but the journal event schema
does not yet carry that artifact. The codec therefore rejects proof-bearing
support events with `ProofPolicyRequired` instead of constructing a receipt
from digests alone. Successful replay, standalone signature and permanent
unavailability payloads now use a separate bounded artifact protocol: one
typed header, at most one non-interleaved open artifact, bounded canonical byte
chunks and a compact closure. Reopen compares a freshly reproduced payload
against every durable prefix chunk before appending its suffix. Closure
reconstructs the private receipt and endpoint DAGs and revalidates their roots,
transition, case, signature and request contract without executing policy.
Chunk size affects transport and journal order, not the typed artifact or final
incidence identity. Support proof restoration remains a real trust requirement,
not serialization ceremony.

Result publication uses a subordinate bounded protocol. Row evidence is
journaled as it becomes ready; after exact input closure, deterministic
projection records are journaled individually in canonical ordinal order, and
a compact closure commits their root, exact counts and final result root. No
event embeds the complete `ClosedResultView`, so a 200,000-row answer cannot
fail merely because its final payload was one oversized frame. Replay rebuilds
the public closed view from those durable records without rerunning result
expressions.

Support-observation readers use the same crash-prefix discipline. Registration
records the stable slice, `Registered`/`AlreadyRegistered`/automatic-overlap
disposition, open-or-sealed phase, exact durable support cursor/frontier and
prior/next explicit scheduler roots. A separate backfill checkpoint advances a
canonical range of no more than 256 structural assignments and makes no partial
slice observable. Only after readiness may a point enter the shared
request-local observation chain. While support is open, later assignment or
terminal evidence dirties only incident ready slices; after support closes, a
pre-existing demand receives a final sealed point, while a late demand may
start sealed. Replay re-prepares and compares every registration, backfill and
point transition before mutation.

Once joined, a valid durable pause commits accepted evidence plus the exact
open frontier. Resume continues without renaming a source, successor or case.
The coordinator must treat the in-memory fold and buffered segment tail as
provisional: only a fully installed segment head may be published as durable.
If encoding or installation fails after the in-memory fold advanced, that fold
is discarded and rebuilt from the last installed prefix.

Preparation, semantic continuation and operational control are distinct
layers. `prepare_checked_relational_stream` closes the checked query, support
plan, publication plan and request-scoped mechanism catalog into a warm
`PreparedRelationalExplore`. `open_epoch` then acquires the exclusive journal
writer and reconstructs the durable frontier once. Repeated `run_slice` calls
advance bounded semantic quanta while retaining checked-expression memo state
and mechanism replay caches. Dropping the epoch is a cold stop: its RAM is
released, while the installed journal remains sufficient to reopen without
changing any identity or exactness claim. Preparation time is reported
separately and cannot itself mint cases or mechanisms.

The public CLI currently opens one such epoch and repeatedly runs short warm
micro-slices (normally about 15 seconds) while its invocation budget remains.
Every completed `run_slice` flushes its accepted journal prefix and attempts a
bounded publication suffix plus manifest refresh before the next slice begins;
after semantic closure, the loop can spend remaining time on publication-only
catch-up. Its `--time-limit` is a total invocation budget: measured preparation
time is subtracted before semantic dispatch. A persistent external control
endpoint for start, pause and status is future operational work; serialized
prepared heaps are deliberately not part of the recovery contract.

Its frames distinguish semantic evidence from checkpoints and presentation.
The journal head authenticates all accepted frame order; only semantic records
enter answer roots. Work/cursor state enters a separate checkpoint root, and
retained examples enter neither. This permits two schedules to converge on the
same mathematical answer without pretending they had the same execution
history.

One scheduler quantum may contain several journal events and is intentionally
not an atomic mega-frame. Every crash prefix of the event order must remain a
valid resumable state. Member evidence and child readiness are written before
the cursor that skips those members. A terminal member chunk then writes its
cursor barrier before the independently retryable exhaustion receipt, seal and
work completion. A crash before the cursor can repeat only idempotent evidence;
a crash after it resumes at exhaustion rather than rediscovering a case into a
sealed fiber. Immutable segments need no mutable `HEAD`: the last strictly
validated installed segment is the recoverable tail, while an unflushed buffer
is discarded.

Replay cost is linear in durable bytes and frames. The store scans the bounded
segment namespace and verifies segment continuity; the codec then decodes and
applies one entry at a time with `replay_streaming_entry`, retaining folded
catalogs rather than an `O(J)` decoded history. At source closure, three typed
set roots plus exact counts commit all fiber receipts, source keys and
traversal edges. The encoded receipt body is fixed at 212 bytes; replay
rederives it from the preceding traversal events before accepting closure.
Work closure similarly peels completed leaf layers through bounded
authenticated compaction receipts.

At the current core boundary, `finish_certified_core` means all declared
support obligations and required core work are closed; it can succeed without
an extensional `RelationContentRoot`. `finish_extensional` is deliberately
stricter and publishes concrete relation/admission/question content roots.
Neither name is shorthand for full analysis-DAG closure: requested result views
and mechanism incidence must later contribute their own sealed roots.

The frontier tracks:

- source discovery and source seal;
- successor discovery and a separate seal for every source;
- admission and question classifications;
- result-view reducers and choices; and
- mechanism replay and incidence.

A discovered prefix, source row or case publishes an immutable readiness token.
Downstream work depends on that token, not on completion of the producer that
yielded it. The unified scheduler now catches selected-result and direct
FIND-target mechanism work up to the current authenticated prefix before granting
one more base quantum. Their result evidence and mechanism target/replay events
therefore appear while FIND is open; only input/target seals, publications and
global closure wait for exact FIND closure. A choice-target mechanism is
different: it waits for the independently journaled choice relation to close,
then admits its members by ordinal in bounded resumable chunks without depending
on display publication. One quirky profile case may be
admitted, selected, shown and replayed while the same source or successor
enumerator continues finding other cases.

Useful evidence is expected before closure. Counts are lower bounds while their
required frontiers are open and exact only when those frontiers close. An
observed maximum is a lower bound on the final maximum; an observed minimum is
an upper bound on the final minimum. View winners and Pareto members are
provisional until their input closes. Unknown mechanism evidence is not zero.

The scheduler may quotient equal proved behavior, but the evidence retains the
exact source support represented by every cell. Mechanism discovery cost can
therefore follow the number of distinct reachable behaviors while case and
profile counts still follow the declared multidimensional world. A sampled
representative without a coverage/disjointness certificate is only a case, not
weighted evidence for the rest of its cell.

The primary per-relation populations are `U_S` source rows, `U_C` constructible
cases, `D_C` admitted cases and `S_C` selected cases. Within one RelationId the
extensional transition counts equal their case counts. Affected-profile and
structural-mechanism counts are separate declared quotients; raw replay
signatures and execution profiles remain separately named audit populations.

Retaining at most N examples does not weaken an exact scalar support count. A
real support-counting cap yields `at_least(N)` for that signature. If signature
assignment stops, signature count and incidence remain open. Finite support is
never reported as assumed infinite.

Time, CPU, RAM and worker limits are invocation facts. The resource governor
already defines the right operational policy: reserve at least one fifth of
installed/live CPU and physical RAM, reserve at least 1 GiB of memory, charge
at least one full CPU core and 512 MiB per worker, use at least a 2-GiB charge
for cold calibration/compile phases and admit zero new work when coherent host
telemetry is unavailable. Warning/critical pressure, OOM risk, swap growth or
lost headroom reduce leases or pause earlier. Scale-up requires a stable window
and durable shard evidence; limits never enter RelationId, CaseId or answer
roots.

Distributed execution, when added, partitions that same checked query into
canonical disjoint evidence chunks. Every returned chunk is bound to the same
RelationId, admission/question identities, coverage obligations and evaluator
contract; the coordinator validates its support and authenticated commitment
before an idempotent merge into the one evidence stream. Worker count, host and
arrival order cannot create another answer. This is ordinary deterministic
evidence-chunk production and merge under one authority, not mining, chain
competition or distributed consensus.

The governor's one-worker envelope has an explicit relational work subject
bound to `(expected_sequence, expected_journal_head)`. The outer run loop
consumes that permit before evaluation, appends the resulting head-bound batch
through the durable coordinator, and flushes an installed checkpoint on
time/resource/semantic pause or completion. This prevents a permit from being
reused after another semantic quantum advances the stream. Bounded quanta are
not themselves complete RAM accounting, so peak reducer and interpreter work
must still fit the conservative one-worker charge:

The production run loop also owns an operational expensive-base controller.
It starts cold fused singleton work at one complete member, immediately flushes
that first appended transition prefix, then targets approximately five-second
batches with a conservative ten-second prediction ceiling, twofold maximum
growth, immediate shrink and an absolute 256-member cap. After learning a
per-member rate it refuses a new quantum when one member plus a 250-ms flush
reserve no longer fits the slice. Warm epochs retain the estimate; cold replay
recalibrates. Batch size, timing samples and forced-checkpoint cadence are never
hashed, journaled or projected.

| Resource boundary | Required relational behavior |
|---|---|
| CPU | dispatch no more charged workers than the governor's safe ceiling; whole-core rounding may reserve more than 20 percent, never less |
| RAM | charge evaluator state, journal fold, open segment buffer, proof state and the peak terminal projection together; pause before the 80-percent ceiling |
| host pressure | stop dispatch at work boundaries, flush the durable tail and publish an honest paused frontier |
| time limit | stop after the current admitted quantum; never truncate a semantic event or invent closure |
| unknown telemetry | admit no new automatic work and preserve the last durable prefix |

The goal is high utilization without another host crash. “Use 80 percent” is a
ceiling on admitted owned work, not a target that overrides live pressure and
not a semantic change to the query.

The outer supervisor uses a 70-percent worker-group RAM trip and the authorized
80-percent total-host CPU ceiling. It also enforces an absolute 80-percent RAM
ceiling, a host-available-memory floor of 20 percent (at least 1 GiB), and an
untracked-overhead reserve of five percent (at least 512 MiB). Launch headroom
counts only free, inactive and speculative pages; active resident pages are not
promised to the evaluator. The inner governor's 30-second stable window is
currently one-time cold-slice overhead.

The former stop/wake race is removed. The parent receives a bounded binary
receipt for a start-gated worker in its own process group before acknowledging
work; an independently runnable guardian owns heartbeat loss and worker-group
kill/reap; and the parent pauses or resumes only that worker group through a
measured debt controller. This design passed a focused build and independent
static safety review, but not yet a live Personskat pause/resume experiment.
Memory floor, critical pressure, swap growth, telemetry loss and wall deadline
remain hard containment boundaries, and relational preparation shares the same
outer resource contract as the warm epoch. Keeping a prepared epoch alive can
amortize cold calibration; removing pressure checks merely to improve a tiny
benchmark cannot.

## Current branch checkpoint

The current transitional frontend and closed IR represent the ordered dependent
FROM/TO/WHERE/FIND query and its post-FIND analysis dependency DAG. This is the
one executable lowering path while the target Explore/Analyze/Publish surface
above is built; it is not a second public dialect. The branch
also contains content-stable relation/admission/question catalogs, lazy source
and successor evaluators, a readiness-driven indexed work DAG, compact exact
traversal closure, SupportCells and proof obligations, the streaming semantic
journal fold, bounded work compaction, readiness-driven selected-case result
and mechanism scheduling, bounded result-projection publication, a unified
semantic DAG scheduler, the canonical safe-subset codec, immutable segmented
byte store, durable journal coordinator, resource-admitted slice loop, prepared
warm epoch and public `runa explore` product path. The checked frontend and IR
also carry name-independent `observations` demands outside that DAG. The stream
now journals demand registration, bounded structural-prefix backfill and shared
support-observation points with separate automatic and explicit schedulers.

The four-transition `relational-explore-stream-smoke.runa` query now closes
through report v11 and publication v19 with two named questions over one shared
relation and admission. `all_cases` closes at exactly four cases;
`interesting` closes at exactly two. The latter two cases share one raw
signature, structural mechanism and execution profile. Its identity-only
semantic transition graph closes with five states, four universe cases, four
admitted cases and separate matched layers of four and two cases. A
30-second cold slice first paused with zero semantic events, then a two-minute
resume completed the same journal at sequence 179 and published all fourteen
artifacts. The
first proof-bearing case-image artifact now installs
root injectivity and optional exact cardinality atomically, with checked durable
restoration and proper-prefix recovery for both resolver completions; positive
weighted SupportCells still cannot feed result reducers. Exact FIND-backed
choice-target mechanisms now wait for the independent Choice relation to close,
admit its members by canonical choice ordinal in bounded resumable chunks, and
seal against ChoiceId and ChoiceContentRoot without a ViewId. This avoids making
display publication an authority for membership. The unified scheduler now joins
readiness-driven selected results, mechanism-incidence results, selected-target
replay and base enumeration. The production checked
interpreter supplies fresh Before/After rule and branch traces from the
artifact-owned program snapshot. Mechanism traces now cross the durable store
as bounded, prefix-resumable artifacts with checked private restoration.
The public selector now routes relational syntax only through this path and
fails closed rather than falling back to the v0 Cartesian, ordinal or probe-era
executor.

Publication v19 is the current artifact plan and report v11 is the current
compact report. Every named result now has a bounded self-describing entry even
when it is ungrouped, open, or exact-empty. The entry names its resolved input,
grain, ordered selected schema, group keys, output-row and projection-record
counts, evidence roots when published, and its independently resumable NDJSON
artifact. Population rows never enter the compact answer or manifest index.
Each mechanism request has one shared value-free observation
stream plus one demand ledger. Automatic whole-mechanism observations remain
the sole support-closure authority; explicitly requested mechanism/node/edge
slices register, backfill in pages of at most 256 assignments, follow
incident-only updates and can attach before or after analysis closure. This
source path is integrated; focused executable verification remains the next
gate before a broader Personskat run.

A reopened terminal journal is recognized before the runtime deadline or host
work-permit poll when no checked runtime proof rebind and no retained explicit
support-observation backfill, dirty prefix or unsealed slice remains. It
returns `complete` with zero appended semantic events and leaves every
caught-up artifact unchanged. A retained certified source summary still passes
its bounded compiler/runtime rebind gateway first; terminal recognition cannot
bypass that trust check—or pending work from an additive support reader—merely
to improve resume latency.

The v13 observation schema implements independently domain-separated
inner/outer expression bounds for correlated case support `S` and
distinct-starter projection `P = distinct_sources(S)`, with explicit
`starter_set_status` and `correlated_support_status`. Publication v12 is
implementation history, not a compatibility target. The exact whole-fiber
typed-region v1 is now the bounded navigation implementation; reduced
decision-DAG compression and the broad Personskat run remain later work.

The first checked policy target now uses 2,000 coordinates, each representing
100 DKK, rather than 200,000 one-krone edges. On 2026-08-31 its public stream
closed exactly for the conditioned Copenhagen profile: 2,000 sources/cases,
2,000 admitted and FIND-classified transitions, zero rejected transitions and
zero selected harmful endpoint transitions. Its selected-target mechanism
request consequently closed with zero requested signatures and incidences.

The next checked question widens profile structure before income scale.
`personskat-income-cliffs-350k-commuter.explore.runa` crosses the complete
zero-to-350,000 prefix of 100-DKK transitions with a 50/100/150-km commuter
lattice, for 10,500 declared cases. The horizon is evidence-informed, but no
suspected threshold or expected case is encoded in the relation. The typed
`Before` carries the whole starting profile and salary, `Context` carries only
the promotion, and the views count distinct `(Context, Before)` starters
separately from cases and signatures. This query has passed targeted format and
static checks at this checkpoint; its later authenticated 10,500-case runs are
recorded separately below. The sequence-130 external closure in the preceding
paragraph belongs to the conditioned 2,000-case question, not this commuter
relation. Every eventual result here remains exact only for its declared coarse
relation, never for every one-krone transition or every profile.

The current publication-v17 execution of that relation is now an authenticated
exact result rather than a prefix. It closed 10,500 sources, cases, admissions
and FIND classifications with zero rejections, 16 selected cliffs and 10,484
non-selected cases. Mechanism analysis closed with two raw signatures, two
structural mechanisms, two execution profiles, 16 successful incidences and
zero unavailable cases. Each mechanism has exactly eight cases and eight
distinct `(Context, Before)` starters; together they occupy the single declared
50-DKK loss bin beginning at 50 DKK. After attaching the two discovered
mechanism starter fibers, the saved bundle's 22 artifacts all match
journal sequence 5,325 and head
`47e1f09df411dc5db6981d620efadb6dc6a473fc4ff4d6318b79872525921bc6`.
This closes the declared relation, not the profile schema: source coverage v3
still reports four `schema_composition_unavailable` subjects. Structured
exact-finite profile composition therefore remains a prerequisite for broader
profile claims and for the planned 1,500,000-DKK audit.

The complementary conditioned mechanism-landscape entrypoint is now authored
with the same 2,000-edge relation, `find admitted_cases = all`, raw admitted-edge and successful
incidence views, typed unavailable terminals, closure-qualified per-signature
support, 1,000-DKK income bins and 50-DKK modeled net-change bins. These
mechanism views become exact for the complete admitted target only after their
frontier closes without replay-unavailable edges. Positive membership in that
question equals admission, so it can replay every admitted edge. The intended
combined audit places this question beside `cliff_cases` over one executable
relation; explanation names the relevant QuestionId and never acquires a
separate implicit `for admitted` surface.

Before starting that nonempty 2,000-edge replay, the durable mechanism path is
being normalized: one complete signature definition is journaled once, and
case incidences reference it with compact transition/replay receipts. Warm
adjacent endpoint proposals are already reused, so a linear `N`-edge landscape
needs `N + 1` endpoint evaluations inside an uninterrupted epoch. This removes
both avoidable evaluation and avoidable trace-payload repetition before a
longer run. No through-1,500,000-DKK stream should start at this checkpoint.

The first broad successor to these calibration runs is now fixed more
precisely: a `0..1,500,000 DKK` horizon sampled by `+1,000 DKK` transitions,
giving 1,500 edges and 1,501 reusable endpoints for every declared profile.
It is a coarse endpoint-and-mechanism audit, not exhaustive one-krone cliff
coverage. No such run has started, and the milestone does not assert current
frontend or runtime support for the target contract.

The implementation history below records the proof and fallback slices that
led to this checkpoint; superseded candidate counts and launch refusals there
are not current result claims.

A requirement-by-requirement static audit sharpened the positive-support seam,
and the first narrow vertical slice is now implemented. After support-plan
registration, production seeds `SupportCellReady` and canonical
`ResolveSupportObligation` nodes. A checked producer-chain verifier replays the
assignment, source-row image, successor and final case-image contracts. It can
prove generic final case-image injectivity without inventing a count, and it
issues exact case cardinality only for an exact independent Context/Before
product with a singleton successor. Finite auxiliary coordinates, dependent
joins and finite-successor expansions remain open.

The retained event contains the canonical proof artifact, not a trusted proof
receipt. Durable decode restores only its structural identity; journal apply
re-verifies it against the installed support plan and atomically remints the
declared root injectivity evidence plus exact cardinality when the stronger
specialization proves it. Generic producer proofs therefore remain useful even
without inventing a count. A crash after that evidence event but before either
resolver completion resumes by appending only the missing canonical
`DirectSupportEvidence` completions. Bare `SupportJournal::EvidenceAccepted`
frames remain codec-rejected. The public report can consequently expose the
conditioned audit's proof-backed `cases = Exact(200000)` before concrete
classification closes, while source, admission and FIND progress remain honest
lower bounds.

Uniform root admission evidence and concrete `AdmissionClassified` evidence
are cross-checked while the journal event is applied, in either arrival order.
A disagreement therefore cannot enter an authenticated resumable prefix; the
same public-report check remains as defense in depth. The first replayable
producer accepts only an empty/all-literal Boolean conjunction and fails closed
on calls, including the Personskat predicates.

This does not close positive symbolic selection. The root admission obligation
still needs a rule-graph-backed uniform/refinement resolver for Personskat;
admitted-only FIND must then activate, and weighted result/mechanism consumers
must accept certified fragments. General partition lifting and bounded
materialization fallback also remain. They must not be simulated with
representative CaseIds. These changes have passed static formatting and
soundness review but await the single focused build and first
resource-admitted Personskat slice.

The conditioned Personskat WHERE and FIND expressions cross checked helper and
rule calls. An `AdmissionId` or `QuestionId` commits their meaning, but a digest
alone is not a replayable theorem. The checked query now retains the first
canonical classification program for this purpose: a name-free, hash-consed
semantic graph plus a separately authenticated runtime-shape adapter. One
request capsule binds them to the checked program, relation/admission/question,
support plan/root cell, specialization and provenance. Its bounded exact
evaluator is threaded through concrete classified sweeps and falls back
atomically whenever any lane residualizes.

That fallback is the honest current boundary for the 350,000-DKK commuter
query. Its finite FROM bindings and deterministic TO successor fit capsule V1,
but both endpoint-validity predicates and FIND call the multi-statement checked
Personskat observation, while the transition predicate also compares a
structured profile value. Those Admission/FIND lanes therefore residualize and
the capsule evaluator delegates the complete batch. The published exact
10,500-transition result was classified by the query-bound native V2 sidecar,
with its checked parity canary, not by capsule V1. The useful next capsule
extension is a sealed checked-observer leaf: bind one exact checked observation
call into capsule identity, evaluate and cache it independently at Before and
After, then retain projections and comparisons in the canonical graph. Merely
lowering local bindings would still stop at the deeper Personskat residuals.

The remaining proof work is scheduler and replay authority plus wider checked
rule semantics. The first capsule-bound one-axis proof producer can normalize
exact scalar quasi-affine/Boolean graphs and acyclic calls, but it is not yet a
durable journal event and richer Personskat rule-family behavior can remain
residual. A proof artifact must replay against the exact capsule before it can
mint uniform or partitioned admission/FIND evidence. Literal predicates remain
useful scaffolding, never a substitute for the checked graph; until that replay
path is scheduled, concrete evaluation remains the canonical completion path.

Once positive selected fragments are certified, downstream adoption should be
incremental. First expose exact logical admission/FIND totals, then support the
one algebraically safe weighted result: `group all` with checked
`count_distinct(case_id)`, whose value is the disjoint selected-population
cardinality. General grouping, measures and mechanism incidence require typed
uniform values or further fragment refinement. Fragment count is never case
count, and symbolic closure must not fabricate extensional CaseIds, row-set
roots or mechanism-incidence roots.

### Checked classification capsule

The replayable classification program has two principal identities. A
`ClassificationGraphRoot` commits the canonical typed semantics and permits
process-local reuse. A separate `RuntimeShapeRoot` commits the current checked
constructor spelling/layout adapter without contaminating that name-free graph.
A `ClassificationCapsuleId` additionally commits both roots, the checked
program, relation/admission/question IDs, support-plan/root-cell IDs,
fixed-value specialization and provenance. Proof artifacts bind the capsule,
not a runtime function name, AST address, dispatch-key spelling or digest by
itself.

Mechanism, node and edge identities remain structural and stable across this
request binding. Their starter evidence is a separate overlay keyed by request,
target and support facet, with an enclosing-mechanism route when that grain is
requested. Each overlay preserves the correlated relation
`Source = (Context, Before) -> Set<After>`. Case and distinct-Source bounds are
separate, while successor support remains conditional beneath each Source;
none can be recovered by relabeling another count. For a shared node, total support is the deduplicated
union of its route-conditioned supports. `node | mechanism` is the corresponding
route-aware intersection of node and enclosing-mechanism incidence within the
same target evidence. Route fibers may overlap, so their counts are never added
unless disjointness is itself proved.

The implemented V1 graph has typed input/constant/constructor/projection and
variant-test nodes; ordered checked integer and Boolean operations;
`if`/conservative lazy `match`; and acyclic pure direct calls. Local immutable
bindings become DAG edges. Recursive SCCs, effects, open captures, dynamic or
higher-order calls, unresolved members, arbitrary collection traversal,
unretained nested pattern-field types and arithmetic whose overflow/rounding
semantics are not proved remain explicit residuals. Exact rule-family dispatch
and additional collection/value operations widen this vocabulary later. A
residual means only that the complete batch uses checked concrete evaluation.

Fixed context may be specialized only from an exact singleton support witness;
the first public binding deliberately remains unspecialized. Before and After
use the same parameterized observation DAG. Exact complete-call results are
cached by callable identity and complete canonical argument tuples across
adjacent endpoints and resumed slices. The FIFO cache is bounded by entry count,
per-entry logical bytes and total logical bytes. This is physical acceleration
only: caches are absent from evidence roots, and fresh mechanism replay remains
authoritative.

Proof replay deterministically rebuilds the capsule from the exact checked
query, requires complete identity equality, abstractly evaluates the obligation
cell, rechecks totality/overflow/call conditions, and reproduces the typed
conclusion before a private gateway can mint evidence. The first proof slice
accepts exact integer quasi-affine values and Boolean truth sets, including
acyclic scalar calls, under a fixed normalization budget; unsupported or
varying forms return concrete fallback. Constructor tags, certified cuts and
wider rule dispatch remain subsequent proof slices. With graph size `G` and
`P` retained proof fragments, the intended direct bound is
`O(G * P + P log P)` time and `O(G + P)` memory; a successful global abstract
proof is `O(G)`, while worst-case `P = Theta(N)` must fall back to concrete
evaluation.

The producer-owned semantic seam, canonical capsule, runtime adapter and exact
ordered concrete backend now form the first implementation slice. Next comes a
durable capsule-bound proof-artifact event and scheduler invocation, followed by
abstract cuts, replayed typed admission/staged FIND proofs and certified
partitions. This keeps the concrete optimization useful even when a full
Personskat theorem remains out of reach.

### Ordered exact fallback without population-sized state

An exact injective mapped image of an ordered integer range with a singleton
successor has a stronger fallback than retaining one journal-state row per
coordinate. First partition the root structurally into bounded ordinal chunks.
Within a chunk, evaluate adjacent endpoints in order and partition that chunk
again into maximal runs with the same full joint outcome: rejected, or admitted
with one packed decision bit for every canonical `QuestionId`. A case selected
by any bit remains concretely materializable once for views and mechanism
replay. Its shared payload records all selecting questions, from which each
question obtains its own projection; the other runs carry typed
admission/FIND evidence and exact cardinality without fabricating extensional
`CaseId`s.

Each accepted sweep batch is bound to the checked query, support-plan root,
root cell, mapped-image materializer, starting cursor and ending ordinal. A
pause commits only a completely covered prefix. The next batch must begin at
that exact cursor, so replay cannot skip or overlap coordinates. A private
checked-executor gateway gives a concrete exhaustive batch the same evidence
boundary as individual classifications; later a classification-capsule proof
may discharge the identical chunk obligation without evaluation.

For `N` adjacent edges the black-box lower bound remains `N + 1` distinct
endpoint observations, which the adjacent-value memo already attains. For `q`
unique questions it also performs `O(N sum_i f_i)` predicate work. If `B` is
the number of chunks, `R_c` the number of joint-outcome runs in chunk `c` and
`X_union` the cases selected by at least one question, retained semantic state
becomes
`O(B + sum_c R_c ceil(q / 8) + q + X_union)` plus one bounded chunk. This need
not allocate one case row per question, but the honest adversarial decision
floor is still `Omega(Nq)` bits when every case breaks the preceding joint run.
Each admitted run authenticates admission once and selection once per
question. Binary search and informative sampling may influence scheduler
priority, but cannot certify skipped coordinates unless interval or rule-graph
reasoning proves the skipped cell uniform.

## Implementation checkpoints

### 1. Canonical frontend and closed relation IR

- Parse `given`/`vary`/`let` and the simple singleton or exact-finite
  `transition after` relation directly. Carry the producer roles through typed
  IR, relation identity and source coverage rather than inferring them from
  cardinality.
- Add named `derive` values and one shared `admit` around the implemented zero
  or more named `find` relations. Delete the remaining scoped-`where` spelling
  when `admit` replaces it; do not maintain a compatibility adapter or
  document two public dialects.
- Accept singleton and exact-finite successor relations.
- Type and purity-check ordered dependent bindings.
- Reject the old compact syntax rather than adapting it.
- Close stable model owners, schemas and RelationId independent of admission,
  named questions, analysis and publication.

### 2. Content-stable relation frontier

- Discover and deduplicate source rows.
- Open, advance and seal each source's successor frontier independently.
- Preserve provenance support without adding cases.
- Make partial snapshots canonical and `finish` fail on an open frontier.
- Resume under a new journal schema without ordinal CaseIds or a probe barrier.

### 3. Admission and question layers

- Normalize one ADMIT conjunction into AdmissionId.
- Normalize every named FIND all/matches/violations relation into its own
  QuestionId, allowing several questions to share one AdmissionId.
- Key classifications by semantic layer plus CaseId.
- Reuse the same relation evidence for another authorized admission/question.
- Require every downstream node to name its question input; do not preserve an
  implicit `selected` shorthand in the target IR or surface.

### 4. Separate analysis DAG, ChoiceId and ViewId

- Parse `? analyze` separately from the Explore contract and resolve every view,
  choice and explanation against an explicit upstream semantic ID.
- Implement case views over named find and choice relations and incidence views
  over named explanations.
- Add deterministic `group`/aggregate views separately from non-collapsing
  `partition`/one/all/Pareto choices.
- Mint `ChoiceId` from membership-defining input, eligibility, partition,
  measures, objectives and tie policy. Mint `ViewId` from its explicit input and
  projection/reduction schema; display-only changes must not rename ChoiceId.
- Resolve names to IDs and reject cycles.
- Keep classification independent of projections and privacy authorization.

### 5. Named endpoint derivations and explanations

- Make checked pure endpoint derivations reusable by admission, named finds and
  analysis without repeating the expression at each clause.
- Validate one typed pure `(State, Context) -> Observation` endpoint callable.
- Lower `explain NAME from find/choice NAME using DERIVATION` to a
  MechanismRequestId whose target is the explicit QuestionId or ChoiceId.
- Accept either a static totality certificate or an extensional certificate
  binding successful canonical receipts for every distinct endpoint in the
  exact closed target; an open replay prefix is only lower-bound evidence.
- Persist explicit unavailable terminals and degrade signature/result certainty
  whenever a required endpoint cannot close.
- Replay Before and After in fresh isolated evaluation.
- Record checked activations as a parent-linked trie and occurrences as local
  references, then normalize both tables before identity. Never charge or
  serialize the same full activation prefix once per event.
- Treat pure higher-order builtin callbacks as checked nested calls: named or
  rule targets must consume their producer-owned callback frame, while inline
  lambdas must match the checked source body.
- Intern complete differential signatures and exact incidence.
- Derive structural mechanism/node/edge starter subbounds as request-, target-
  and facet-keyed correlated origin-support overlays
  `SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>`, never as fields of
  structural identity. Track confirmed inner support, a concrete outer envelope
  or opaque target obligation, and keep case-count, distinct-starter and
  conditional-successor closure independently typed.
- Automatically observe only each discovered whole-mechanism total slice for
  core support closure. Register checked explicit mechanism/node/edge slices in
  a separate scheduler, backfill their frozen structural prefix in pages of at
  most 256 assignments, and dirty only incident ready slices thereafter.
- Journal registration, backfill and immutable point evidence so demands can
  attach before or after analysis closure; allow a late demand's first point to
  be sealed and keep explicit completion outside the core closure receipt.
- Add post-mechanism views such as distinct signatures per 50-DKK loss bin.
- Keep optimum proofs and mechanism explanations separate.

### 5a. Separate publication and support readers

- Parse `? publish` as an additive consumer of named analysis nodes. It may
  expose views, compact support observations, typed support cases, projected
  support starters and case/support graphs; it cannot add an answer-DAG node.
- Treat `support cases` as the correlated target-conditioned `S` relation and
  `support starters` as its distinct `(Context, Before)` projection `P`.
- Keep value authorization explicit and singular. Do not infer typed values
  from a compact observation or introduce a wildcard `cases × DAG subjects`
  export.
- Derive a typed correlated-region index only as an additive publication reader
  over authorized canonical support-case fibers
  `F : (Context, Before) -> Set<After>`, never as an Analyze node or a new
  semantic count authority.
- Make the first index an exact whole-`SourceKey`-fiber v1. Stop a compression
  cap only between fibers and link the uncovered suffix to its canonical page,
  projection job and source closure-record address instead of widening it into
  marginal boxes.
- Report semantic inner/outer status, region derivation
  (`exact_partition | confirmed_subset`) and compression coverage
  (`complete | capped`) independently. The artifact remains navigation-only
  in either case, and an opaque outer obligation has no invented typed
  envelope.
- Preserve each dimension's varied, derived, conditioned, certified-irrelevant
  or coverage-gap provenance separately from the extent observed in one
  support slice. Counts come from canonical deduplicated keys or a checked
  partition receipt, never interval widths or products of marginals.
- Lower publication-v17 trailing `observations`, `starters` and `transitions`
  declarations only as transitional implementation spellings.

### 6. Proof-oriented search reduction

- Recognize source events and affine successors as scheduler information.
- Certify interval, congruence and relevance cells.
- Cache reconstructed fibers by the support plan's declared dependency tuple.
- Add a checked Personskat rule-graph normalization/proof producer; do not
  mistake the current affine query-guard splitter for policy-rule closure.
- Leave exact residual frontiers for singleton or later SMT refinement.
- Never promote candidate exhaustion into complement closure.

### 7. Personskat widening

- Run a conditioned one-profile bootstrap over lower annual income
  `0..<200_000` DKK. Publish every fixed field and source restriction in its
  coverage manifest, and describe the result only as exact over that declared
  subrelation.
- Then run a small genuinely multidimensional coherent profile relation at the
  same income horizon. Publish its source-coverage manifest; reject an
  undocumented hidden constant as evidence for the broad question.
- Where an exact irrelevance producer exists, confirm that irrelevant
  dimensions close with its certificate; otherwise keep them varied or report
  the coverage gap rather than inferring irrelevance from equal samples.
- Confirm that every compressed behavior cell preserves exact disjoint profile
  and case support.
- For each audit, stream exact/open case and profile evidence and, where
  selected cases exist, replay-derived mechanisms and 50-DKK loss bins.
- Accept an authenticated exact-empty Personskat result at either scope; use
  the nonempty mechanism fixture, not a planted tax threshold, to cover the
  mechanism-incidence path independently.
- Widen income and profile domains organically while watching frontier shape.
- Make the first broad audit a declared coherent profile relation crossed with
  lower salaries `0, 1_000, ..., 1_499_000 DKK` and `+1_000 DKK` successors
  through exactly 1,500,000 DKK: 1,500 edges and 1,501 reusable endpoints per
  profile.
- Report its closure only as exact over that coarse relation. Do not describe
  it as an exhaustive 1-DKK cliff audit, and do not start it until the target
  contract has an honest executable path.

### 8. Permanent confidence

- Add focused unit coverage while implementing binary/event partitioning and
  identity conservation.
- Once the feature and outputs are coherent, run formatter, focused behavior
  tests, mint and the required deeper semantic-change lane.
- Do not repeatedly run broad compilation or end-to-end Personskat exploration
  while the architecture is still moving.

## Publication shape

The durable journal directory is the resumable evidence artifact; a report is
a projection of one authenticated head. The head itself is the whole streamed
prefix commitment: reporting must not clone a 200,000-case catalog merely to
derive a cosmetic second root. Publication should therefore be a
small manifest plus streamable named-view files rather than one population-sized
JSON object:

In the target surface every artifact below is requested by a separate
`? publish` contract. Publication cannot alter the Explore relation, its named
questions, a ChoiceId or the analysis DAG. The current parser keeps equivalent
`results`, `observations`, `starters` and `transitions` declarations inside the
Explore block while publication v19 is completed. Nested `choose` now lowers
to a canonical membership-only `ChoiceId` plus a separate display `ViewId`;
an independently journaled choice relation commits its candidate seal, member
prefix and content root before the display iterates those exact members. The
display applies row-local `SELECT` only to them, without scanning excluded FIND candidates or
repeating choice policy. Choice objectives
currently fail closed on aggregate or `SELECT` aliases so display-only changes
cannot silently rename the choice relation; aggregate-backed Choice displays
also fail closed until the member relation carries sufficient closed-group
evidence. Those spellings are a transitional lowering boundary, not another
public language design.

```text
personskat-200k-result/
  manifest.json
  views/cliffs.ndjson
  views/case_summary.ndjson
  views/mechanism_summary.ndjson
  views/mechanism_starter_support.ndjson
  views/loss_bins_50_dkk.ndjson
  mechanisms/cliff_paths.ndjson
  mechanisms/cliff_paths.definitions.ndjson
  mechanisms/cliff_paths.structural.ndjson
  mechanisms/cliff_paths.structural-definitions.ndjson
  mechanisms/cliff_paths.support-observations.ndjson
  mechanisms/cliff_paths.support-observation-demands.ndjson
  starters/selected_cliff_node.ndjson
  starters/selected_cliff_node.regions.ndjson
  graphs/case-support-<question-id-hex>.ndjson
  graphs/case-transitions.ndjson
```

The `starters/` lane is explicit and single-subject. Publication v18 retains
the materializer first introduced in v9 and schedules one artifact only for an
authored transitional `starters NAME from mechanisms REQUEST ...` consumer
whose `using values from VIEW` reference names a checked lossless
single-question `each case` view authorizing CaseId, Context, Before and After.
Its absence is `not_materialized`, not an empty-support claim. Adding another
consumer to a completed run is an additive publication operation: it leaves
the existing journal, analysis roots, and prior artifacts untouched while
adding its content-addressed artifact and updating cursor/manifest state.

The automatically paired `.regions.ndjson` companion is a bounded navigation
index over that explicitly authorized artifact, not a second support relation.
V1 emits complete source fibers in canonical order and records an exact filter
back to their source/successor pages. A capped index closes with the last
covered SourceKey and a resume/fallback handle to the canonical projection.
The source pages stream independently, so such handles may be forward
references until the source artifact closes. The index never emits half a
fiber or treats missing records as wildcard support. Its dimension metadata
keeps query coverage provenance separate from observed support extent, and its
counts cite the canonical projection closure or checked disjoint-partition
receipt rather than region arithmetic. A protocol-fixed 1 MiB
maximum-width-envelope preflight is bound into the summary and artifact
identities and falls back before an oversized fiber. An invocation with a
smaller operational line limit is rejected before filesystem mutation rather
than changing that deterministic prefix. After closure, no-op resumes reuse
the validated final receipt instead of rebuilding typed regions. Later
compression may reduce the same exact fibers into an ordered, hash-consed
decision DAG with disjoint typed selectors; this changes navigation size only.

`graphs/case-transitions.ndjson` is the corresponding case-side semantic
artifact. It is automatically authorized only when one checked explicit-find
`each case` view directly exposes `case_id`, `context`, `before`, and `after`.
Its append-only rows follow journal selected-discovery order and contain the
CaseId, source/successor keys, canonical Before/After StateIds, directional
TransitionId, checked schema IDs, and typed Context/Before/After values. Its
exact closure commits the canonical selected-case set and graph-content root,
so file order is not semantic identity. Because it is derived from retained
journal cases, it can attach to a closed stream without endpoint evaluation or
mechanism replay. It is `O(selected cases)`, never `O(cases * mechanism
subjects)`, within V1's fixed 65,536-edge collision-checking capacity. Crossing
that capacity produces a typed `capacity_limited` terminal after the stable
retained prefix; it does not fabricate exact graph closure or allocate beyond
the cap.

`manifest.json` names the query and identity ladder, checked program, source
coverage restrictions/gaps, journal head and sequence, lifecycle/pause reason,
and closure-aware `U_S`, `U_C`, `D_C` and `S_C` counts. Every named view and
mechanism request reports semantic `open`, `exact`, `unavailable(reason)` or
`error(reason)` plus a separate count status and evidence/result roots. An NDJSON row is one bounded authorized
projection or incidence, so exporting many concrete configurations does not
require building one giant array. Grouped views use the same NDJSON envelope,
one authenticated projection record per line, so a large histogram also stays
bounded and resumable.

Report v10 partitions support observation state explicitly. The request layer
contains the total shared observation point count/root; automatic point,
registered, dirty, observed and sealed counts plus its closure-authority chain
root; and explicit registration, point, registered, ready, pending-backfill,
dirty, unsealed, observed and sealed counts. These are stream/scheduler facts,
not invented case counts. The demand ledger additionally distinguishes authored
aliases, unique checked demands, genuinely new explicit registrations and
whole-mechanism overlaps with the automatic registry.

An authorized starter-support view publishes correlated `(Context, Before)`
cells, their inner/outer support status and optional searchable marginals. It
is separate from the mechanism definition stream: changing the explored
relation, target or open frontier may change starter support without changing
the causal structure of an already known signature. Raw source values remain
subject to the view's explicit privacy projection.

An explicitly **raw-signature** support view remains useful for replay
diagnostics. In the target analysis surface it is:

```runa
view raw_signature_starter_support from explain paths {
    group by [signature_id]
    aggregate [
        starters = count_distinct((context, before)),
        cases = count_distinct(case_id)
    ]
    select [signature_id, starters, cases]
}
```

The tuple is the source-row identity inside the relation, so several After
successors from one starting world contribute one starter and several cases.
This is an exact concrete support result for each complete replay signature.
The structural form groups by `structural_mechanism_id` and may retain
`count_distinct(signature_id)` as a separate diagnostic. Correlated symbolic
cells, inner/outer frontiers and structural-node/edge support are now grounded
in the factorized mechanism-support catalog. Complete signatures retain the
disjoint CaseId authority; inverted structural membership derives each
mechanism/node/edge overlay, and its authenticated starter index retains a
conditional SuccessorKey fiber beneath every SourceKey. They MUST NOT be
simulated by multiplying marginal bounds or exploding every case across every
DAG node.

The overlay is keyed by `(request, target, structural subject, facet)`, not
stored inside structural node identity. Activation and differential
participation are separate node/edge facets; a whole mechanism is facetless.
Every validated structural mechanism, node and edge therefore has a bounded,
correlated origin-starter relation over `Source = (Context, Before)`. Each
overlay has correlated lower/upper starter support, per-starter successor
fibers and independently typed case/starter counts. Query-fixed values are
labeled `conditioned`, never misreported as inferred trigger requirements.

For a shared node, that key names the **total** origin-preimage support across
every structural mechanism and route which reaches it. A graph browser may
derive narrower fibers conditioned on an enclosing
`structural_mechanism_id`, an incident structural edge or a canonical path
segment. Each conditioned fiber retains the same
`OriginSource -> Set<SuccessorKey>` shape. The fibers union to the total only
under a complete route cover, and they can overlap when one case contains
several qualifying edges or paths. They are therefore navigation and
explanation views, not additive partitions unless a checked disjointness proof
says otherwise.

The published grains remain separate: `distinct_starters` counts deduplicated
origin `(Context, Before)` rows; `cases` counts supported CaseIds (and therefore
extensional transitions inside one RelationId); and a sum over node/edge rows
counts overlapping **case-to-subject incidences**. That incidence sum may be
much larger than the case population and is never a mechanism count.

Origin preimage is also distinct from optional local-entry support. The former
always refers back to the exploration's original `(Context, Before)` row. The
latter would expose retained values in the node's immediate evaluation frame
and requires occurrence-indexed frame evidence. Equal local frames may come
from different origin starters, so neither coordinate system may be substituted
for the other or hashed into structural identity.

Fully unioned subject projections are bounded, evictable hot views. Durable
authority remains the shared signature fibers plus one factorized residual, so
visiting every DAG node cannot accumulate a permanent `cases × nodes` table.
Sparse pause/report checkpoints bind operational cursors; final structural and
support closures bind the exact raw-signature set and correlated support roots.

Publication v18 replaces the old eager all-subject structural rows with
scheduled compact observations. The structural-definition catalog advertises
stable descriptors for every whole mechanism and activation/differential node
or edge, but a descriptor is only an address. Every discovered whole mechanism
automatically registers its facetless total slice. New evidence dirties only
that mechanism; changes may coalesce, and a lazy final sweep seals every
automatic slice. The core support receipt requires automatic registered,
observed and sealed counts to equal the exact structural-mechanism count.

Target-conditioned mechanism, node and edge slices are explicit readers. The
following is the current publication-v17 transitional spelling; its target home
is a separate `? publish` declaration:

```runa
observations selected_cliff_node_support
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
within mechanism "<StructuralMechanismId>"
```

`within mechanism` is optional and valid only for a node or edge. The subject
may instead be `mechanism "<StructuralMechanismId>"`, `node activation ...`,
or the corresponding activation/differential edge form. Names are output
aliases. The checked demand ID is derived from request, subject/facet and
optional route, while the sorted unique demand-set ID ignores names, aliases
and declaration order. Both remain outside the core analysis DAG.

The explicit scheduler anchors registration to the durable support prefix,
backfills no more than 256 canonical structural assignments per quantum, and
does not expose a partially caught-up slice. Ready slices install incident
watchers, so later assignments and terminals do not scan every demand. A demand
registered before closure can emit open points and a final sealed successor; a
demand attached after closure can begin sealed. Its progress never gates the
automatic support receipt. A whole-mechanism demand aliases an existing
automatic slice instead of duplicating its scheduler state.

Each observation is a constant-size, value-free factorized summary. Disjoint
signature-fiber weights give case bounds; the largest inspected single-fiber
starter count and sealed target starter projection give honest starter bounds
until deduplication. One point inspects at most 256 canonical signature-fiber
summaries; crossing that cap records wider bounds, never a full union. The point
keeps `starter_projection.status: not_materialized`, authenticated inner/outer
fiber-expression identities over
`SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>`, and an
authorization-neutral projection plan. Equal expression identities do not mean
typed cells were serialized. Exact starter-set evidence remains distinct from
exact conditional-successor evidence.

The explicit typed materialization job remains a different reader. Publication
v14 retains the single-subject transitional `starters` form introduced in v9:

```runa
starters selected_cliff_node
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
using values from cliffs
```

A node or edge consumer may additionally select one enclosing structural
mechanism route without changing the node/edge identity:

```runa
starters selected_cliff_node_in_path
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
within mechanism "<StructuralMechanismId>"
using values from cliffs
```

The unqualified form is total subject support. The qualified form intersects
the existing subject and mechanism signature indexes, then reuses the same
correlated `SourceKey -> Set<SuccessorKey>` merge. The route is bound into the
consumer, plan, job, cursor, checkpoint and public record identities; it never
renames the structural node or the core analysis DAG.

Qualified artifacts use the unified subject-starter record schema v3. The
optional route field is omitted from unqualified cursors and records.
Publication v9 established the additive-consumer principle; the current
Experimental v17 plan
preserves that separation without treating historical v9 bytes as a
compatibility target.

It writes `starters/<name>.ndjson` in pages of at most 64 members, adaptively
shortens a page to the byte limit, and uses a k-way merge whose peak memory is
proportional to the contributing raw signatures plus one page. V1 has no
wildcard or list selector; multiple subjects require multiple named consumers.
The declaration belongs to the appendable publication-consumer graph, not the
analysis DAG, so copying a node ID from an existing structural sidecar and
resuming does not re-explore any cases. A fixed-fan-in external merge and
arbitrary path-conditioned selectors remain future scaling work. The whole
structural catalog MUST NOT be eagerly materialized merely because it exists.

Publication schema v17 keeps each mechanism request and authored projection in
independently resumable artifacts. `mechanisms/<name>.ndjson` is the
answer lane: its compact
discovery events name a validated signature descriptor, typed unavailable
reason or case terminal, followed by the incidence closure when authorized.
First intern still precedes a terminal that refers to it, but one descriptor
advances the cursor immediately. Exact **raw-signature** counts and incidences
therefore do not wait behind presentation of a large definition; exact
structural-mechanism and execution-profile counts come from the separately
closed structural quotient.

`mechanisms/<name>.definitions.ndjson` is the audit-payload lane. In signature
discovery order it emits one header, bounded lowercase-hex chunks of the
existing canonical definition, and a completion record binding the digest,
length and chunk count. Once every final definition is complete it emits a
request-level closure binding the exact signature count and incidence root.
Its independent cursor is
`(signature_ordinal, definition_part_ordinal, closure_emitted)`; the answer
cursor is `(event_ordinal, closure_emitted)`. A sidecar's available signature
prefix is derived from signature descriptors already committed by its companion
answer cursor, so payload can never become observable before its descriptor.
Result views, the answer lane and the case-support graph are serviced before
definition sidecars. The manifest shows their independent source windows and
caught-up states, so an exact answer may be visible while reconstructable
definition bytes continue streaming.

`mechanisms/<name>.structural.ndjson` is the quotient-and-closure lane. It
streams raw-signature-to-structural assignments, the exact structural closure,
and at most one constant-size automatic support receipt. It no longer emits a
closure-time row for every mechanism, node and edge.

`mechanisms/<name>.support-observations.ndjson` is the single append-only point
stream shared by automatic whole-mechanism slices and explicitly demanded
slices. Each row records its stable slice, prefix status and value-free
factorized summary. The artifact reports both the total shared chain and the
separate automatic closure-authority partition.

`mechanisms/<name>.support-observation-demands.ndjson` is the per-request
demand ledger. It publishes durable registration claims, registration phase
and disposition, the name-independent demand-set ID, and every authored alias
with a lookup into the shared observation stream. Automatic whole-mechanism
overlaps are identified explicitly rather than counted as new explicit
scheduler slices. Both artifacts say `contains_typed_values: false` and
`cells_serialized: false`.

`mechanisms/<name>.structural-definitions.ndjson` is a separate,
self-describing DAG lane. It becomes available at structural quotient closure
without waiting for support closure, and publishes each normalized frame,
activation context, node, edge, mechanism and execution profile once in
canonical content-ID order. Typed, item-capped chunks carry dependency,
membership, root, ownership and multiplicity lanes; a catalog closure binds
their fixed section counts and a structural-definition catalog root. Support
rows reference these IDs and this artifact. It never republishes raw signature
payloads, cases or starter values, so its size follows the unique quotient
catalog rather than the raw trace population.

Structural derivation has three separate hard admissions: a 64-MiB
authenticated raw-source ceiling, a 1-Gi-unit conservative logical-work
ceiling, and a 128-MiB canonical-artifact ceiling enforced during encoding.
One policy constructor is shared by the live producer and journal rederiver.
The split is intentional: source size, transient quotient work and durable
output are different resources, and forcing all three beneath an 8-MiB output
quantum rejected the known 30.1-MiB Personskat definition before decoding it.
Logical work remains deterministic scheduling policy; sampled process RAM is
still governed independently by the outer 6-GiB/80-percent containment
envelope.

The signature descriptor validates the request scope, canonical encoding and
Before/After DAG once and publishes exact endpoint node/root/edge counts plus a
content-addressed reference to the sidecar. Definition resumes hash and slice
the canonical blob without rebuilding the verbose DAG index. The v3 mandatory
node/outcome/root/edge flattening is retired because it repeated full activation
paths and expanded the 30.1-MB one-case Personskat definition into 320,364
blocking records. A searchable per-node cell projection can return later as an
explicit on-demand export; it is not on the critical result path. None of the
v13 structural or compact observation artifacts contains replay-only state or
context values.

Each canonical question's automatic
`graphs/case-support-<question-id-hex>.ndjson` artifact is the complementary
public reflection of its projection from the shared authenticated case/support
prefix. Publication v18 gives every `QuestionId` its own key, path, cursor and
manifest descriptor; aliases share that artifact and no find is primary. The
artifact has two honest projection shapes rather than pretending every
scheduler path creates the same proof artifact. A partitioned classified run emits a root, then complete chunk
packages containing structural chunk and homogeneous region records,
selected-materialization records, and authorized cases. A completed run that
never mints a chunk partition emits an exact classification-summary root, the
three mutually exclusive rejected, admitted/not-selected and
admitted/selected regions, authorized selected cases, and a closure naming the
actual classification and selected-population authorities. In both shapes a
case record appears only when a checked ungrouped selected-case view explicitly
publishes the bare nominal `case_id: CaseId`.

Parent IDs make the projection searchable as root → chunk → region → selected
materialization → case or root → classification region → case without
publishing coordinates, state/context values, materializer identities or proof
payloads. A missing selected materialization holds back its whole partitioned
chunk, so an open file remains a valid append-only prefix. Partitioned counts
stay `lower_bound` until exact classified/materialized coverage conserves
against the root cardinality. A classification summary is emitted only after
the relation/admission/FIND layers compose to exact counts and the selected
seal's set commitment matches every authorized CaseId. The publisher must not
wait forever for a partition that the chosen proof path can never produce, and
must not synthesize one for presentation symmetry.

Those opaque artifact IDs and roots are deterministic audit commitments, not
hiding commitments. The graph does not serialize raw case values without a
checked `select`, but a low-entropy private input could still be tested against
a known commitment. A local output directory containing private queries must
therefore remain confidential. Any future shared or cloud publisher needs an
explicit release policy and, where non-disclosure is required, projection-local
identifiers or hiding commitments rather than reusing private content roots.

The separately resumed `graphs/case-transitions.ndjson` artifact closes the
other half of the model. `case-support` is the search/classification DAG;
`case-transitions` is the selected semantic edge list. Every edge carries one
authorized CaseId support and the canonical
`StateId(Before) -> TransitionId(Context) -> StateId(After)` identities plus
typed values. Mechanism incidence already uses the same CaseId and
TransitionId, while subject starter projections use the same SourceKey and
SuccessorKey. The three projections therefore meet without duplicating a
case-by-node table. Publication v18 treats this lane as an additive
artifact, preserving existing cursor/journal identity when it is attached to
a completed run. Its V1 in-memory collision index has a hard 65,536-edge
ceiling; a larger selected population closes the materialization attempt with
an explicit capacity frontier, not with false exactness or unbounded RAM.

Before appending a bounded mechanism batch, the publication cursor freezes the
authorized event end and, if analysis has closed, the exact incidence root.
Crash recovery may reproduce only that frozen source window even when a newer
journal prefix is already installed. Newer discovery events therefore cannot
authenticate an older torn suffix. Exactly one compact closure record follows
the final frozen event after `closed_mechanism` confirms the root and counts;
an open prefix has no synthetic closure marker.

Publication is a resumable materialized view of the journal, never a second
source of semantic truth. Its cursor is a checked journal sequence/head plus
per-artifact ordinal state; it is not a content-hash watermark, because a
later `CaseId` or signature can sort before an earlier one. On reopen the
publisher validates its last complete line and cursor, then copies only the
missing bounded records. A crash may leave publication behind the journal and
is repaired by replay; publication ahead of or contradictory to its committed
journal prefix fails closed. Before any result-view or case/support batch
appends bytes, v3 also freezes its exact flat source end. Torn-tail recovery
cannot consume rows that arrived only in a newer journal prefix, just as a
mechanism batch cannot cross its frozen event end. Ordinary result lines contain
only immutable record-owned facts; changing input/projection/mechanism frontier
metadata lives in the atomically refreshed manifest. Recovery can therefore
rederive an older complete or partial tail byte-for-byte using its frozen
checkpoint even after the live journal has advanced.

The warm invocation now reuses that external-resume correctness directly. The
epoch orchestrator divides the caller's total deadline into short operational
micro-slices (initially about 15 seconds), lets each semantic sub-slice end at a
real flushed journal boundary, publishes one bounded publication suffix plus an atomic
manifest, and resumes the same prepared epoch while time remains. After
semantic closure it spends the remaining deadline on publication-only catch-up.
This is sequential cooperation over one journal, not a concurrent publisher
callback or a second stream. Publication is therefore observable after each
micro-slice rather than only when the outer invocation returns. Latency remains
bounded below by one indivisible semantic quantum, so endpoint replay will
eventually need its own cursor if one replay can exceed the cadence.

For an `each case` view whose entire `select` list is row-local, the accepted
result-evidence record already contains the authorized projection values.
Those rows may therefore enter `cliffs.ndjson` while FIND is still open, each
tagged with the EvidenceToken whose semantic snapshot contains it; the
operational journal head is reported separately. Group aggregates, `having`,
choice results and mechanism histograms cannot pretend to be exact early. A
`revisable` node may publish authenticated add/retract events while open; a
`closure_gated` node publishes no row until its declared input closes. Both end
with an authenticated active-set seal. Thus observability does not require
delaying every useful case until global exhaustion, and early output never
turns a partial prefix into a completed answer.

The human completion line is derived from that manifest, for example:

> Exact over the declared conditioned profile: 11 structural mechanisms explain
> 3,000 concrete cliff cases; 2 raw-signature support frontiers reached their
> declared support cap. See `views/loss_bins_50_dkk.ndjson` and
> `views/cliffs.ndjson`.

The numbers above are illustrative. “Exact” may appear only when the named
frontiers actually close; otherwise the same sentence must say `at least`,
`open`, `unknown` or `unavailable` at the affected layer. A named question's
configuration file contains only fields authorized by the published view's
`select` clause. Retention limits govern examples, not exact scalar counts.

## Repeatable perspective-based scenario review

Before widening a domain or promoting a surface, review all three scenarios
through four independent perspectives. The policy author asks whether the
question says what it means. The language implementer asks whether it lowers to
one typed, acyclic and scenario-neutral graph. The auditor asks what the
evidence actually proves. The stream operator asks whether interruption,
revision, closure and publication can be handled without changing meaning.

| Scenario | Policy-author obligation | Language-implementer obligation | Auditor/result obligation | Stream-operator obligation |
|---|---|---|---|---|
| Income cliff | State `given` scope, every varied coherent profile/income dimension, the salary transition, named endpoint assessments, shared admission and a named violation find | Lower the same relation to stable cases, reusable derives, closure-aware questions and a fresh explanation request without tax-specific syntax | Verify exact or lower-bound cliff cases, affected starters/profiles, structural mechanisms and loss bins; never confuse support cases `S` with support starters `P` | Stream monotone case rows early, keep aggregates and explanation closure honest, and resume from durable frontier state |
| Lowest-tax municipality iff variation | State one given person, municipality as the varied alternative, a named all-options find, `partition all`, exact `varies` eligibility and complete tied argmin choice | Make `having varies` and tied argmin properties of ChoiceId over full question rows, independent of any display ViewId | Suppress a recommendation until the partition closes; distinguish authenticated no-difference from no candidates, and mechanism evidence from the proof of minimality | Publish the closure-gated summary and winners atomically while preserving all tied minima and exact-empty results |
| Household work/pension trade-off | State one given household, an exact-finite dependent successor fiber that keeps hours/transfers/pension ownership correlated, explicit feasibility admission, a named resource-floor find and declared Pareto objectives | Type dependent successor dimensions and Pareto dominance without flattening correlation or inventing a one-off planner | Verify nondominated plans only after frontier closure, the explicit preference model rather than an invented “should”, and explanations that do not widen household scope | Pause/resume the larger frontier under resource limits and apply revisable or closure-gated updates by their canonical row keys |

Repeat the following questions for each scenario:

- Can a reviewer find every fixed, varied and derived fact without opening an
  execution manifest or guessing what a helper silently fixed?
- Can two named finds share the finite relation and admission without copying
  the query or creating different CaseIds?
- Does every view, choice and explanation name its input, and does changing a
  display-only field leave QuestionId and ChoiceId unchanged?
- Can an interrupted stream state what is known now, what remains open and
  which evidence token authenticates the update?
- Are publication and support readers attachable without changing the question,
  and do they preserve the distinction between cases, starters, incidences and
  structural mechanisms?
- Can the same kernel express all three scenarios without authored probes,
  scenario-specific declarations or display rows becoming semantic inputs?

### PBR round 2 record (2026-09-03)

The review covered the target RFC and all three steering scenarios. It was a
contract review, not an execution claim. The first pass was allowed to fail:
that pass found ambiguous choice-local `having`/Pareto semantics, incomplete
successor and Analyze-node coverage, a display-dependent choice path,
overloaded stream statuses and tokens, an underspecified revision fold, a
non-row-preserving explanation schema, and competing static/extensional
totality identities.

The contract was reduced and corrected in place rather than gaining another
language layer. Choice now consumes full named-find rows; explanation preserves
its resolved target row and adds incidence; source, successor and reachable
Analyze coverage compose explicitly; updates use one keyed add/retract/seal
fold; EvidenceToken and ResumeCursor have disjoint jobs; and one endpoint-
totality obligation may be discharged by static or complete extensional
evidence without renaming the graph. Direct admission targeting was removed in
favor of an ordinary named `find NAME = all`.

The narrow recheck then produced no P0 or P1 blocker:

| Perspective | Income cliff | Municipality iff variation | Household Pareto | Reduction verdict |
|---|---|---|---|---|
| Policy author | Pass | Pass | Pass | No hidden fixed profile, implicit group or scenario-only clause |
| Language implementer | Pass | Pass | Pass | One typed row-preserving Explore/Analyze graph; no redundant identity layer |
| Auditor/result consumer | Pass | Pass | Pass | Scope, coverage, bounds, exact-empty results and mechanism evidence are distinguishable |
| Stream operator | Pass | Pass | Pass | Pause/resume, revisions, closure and publication preserve semantic identity |

Passing means the target contract is reconstructable and internally sufficient.
It does not mean every target block executes in the current Experimental
frontend. The remaining gap is implementation progress under this contract,
not another round of public concepts.

## Acceptance signal

Before allowing a long Personskat semantic slice, the cumulative small durable
gate requires:

- one finite relation and shared admission feeding explicit named question
  relations, with no Cartesian route or implicit `selected` dependency;
- named endpoint derivations reused by admission, questions and analysis while
  explanation still performs fresh traced replay;
- append, pause, immutable-segment flush, reopen and semantic streaming replay;
- stable CaseIds and byte-identical roots across at least one pause/resume;
- exact relation/admission/named-question closure, including an exact-empty
  terminal path;
- separate ViewId and ChoiceId behavior, including a display-only view change
  that leaves chosen membership and its explanation target unchanged;
- explicit-view result publication from the sealed question through a separate
  additive publication consumer;
- governor-controlled dispatch and resource-pressure pause without host
  instability; and
- a publication coverage manifest that exposes every fixed bootstrap fact and
  reports composed coverage, status, update kind and evidence token.

The deliberately nonempty
`examples/relational-explore-stream-smoke.runa` fixture has now demonstrated
the plural public frontend-to-publication path. One shared four-case relation
feeds `all_cases = exact(4)` and `interesting = exact(2)`; the latter produces
one shared replay-derived raw signature, structural mechanism and execution
profile, a two-case incidence aggregate and exact two-case/two-starter support
closure. The semantic case graph closes at five states with `U_C = D_C = 4`
and question-addressed matched layers `M_C = 4` and `M_C = 2`. A 30-second
cold slice paused before semantic work; resuming the same authenticated run
completed at journal sequence 179 and head
`ec7c602c6fa3548bab4f0221247f019c49f4c63577c5d76d00c2b174a592ab81`,
with all fourteen planned artifacts caught up. It exercises the same stream
without pretending to be a tax audit. An induced resource-pressure
pause remains a separate focused acceptance check; it need not be rediscovered
by repeatedly running the tiny happy-path query.
The failed 200,000-DKK preparation attempt demonstrates only the outer
host-pressure stop before semantic work; it does not satisfy the durable
resource-pressure pause/resume or coverage-publication checks.

Before treating the same horizon as a population audit, one durable fixture
must additionally demonstrate:

- a dependent multidimensional source relation;
- zero-, one- and many-successor fibers;
- stable CaseIds across pause/resume and discovery-order changes;
- exact deduplication and closure-aware `U_S`, `U_C`, `D_C` and `S_C`;
- all three FIND forms, including at least two named finds over one admitted
  relation;
- at least two distinct cases sharing one replay-derived mechanism signature;
- a case view, a partitioned choice, a choice-targeted explanation and a
  post-mechanism aggregate view in one acyclic analysis DAG;
- different exact target support-case and support-starter counts for one
  multivalued successor fixture;
- honest partial snapshots and an exact empty or nonempty terminal result; and
- resource-pressure pause without host instability.

Only after the second signal should profile breadth or the income horizon
widen. The desired large-run output is not merely a case count: it is a
closure-aware count of cases in the named question, affected profile projections
and structural mechanisms, with raw-signature and execution-profile counts
retained separately, named views such as the 50-DKK structural-mechanism
histogram, and resumable authenticated evidence handles.

The first compact answer envelope therefore stays bounded: it leads with the
named before-to-after question population, reports each mechanism request's
closure-aware structural count, and exposes the sealed target's distinct
starter count separately from cases. The latter is request-wide support, not a
per-node count. Per-mechanism, node and edge starter conditions remain
correlated authenticated support overlays referenced through the publication
manifest. Report v10 may inline a schema-capped prefix of a small exact grouped
view directly from its authenticated projection journal; its evidence roots
and truncation cursor make that preview auditable. The operational artifact
index names the corresponding full NDJSON plus every case, mechanism, compact
support-observation, demand-ledger and typed-starter artifact and its catch-up
state. Bulk case rows and large grouped histograms remain streamable NDJSON
rather than one in-memory answer array.
