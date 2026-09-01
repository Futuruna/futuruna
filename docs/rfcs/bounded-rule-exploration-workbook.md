# Turning Rules Inside Out with `? explore`

Status: steering workbook for the Experimental relational replacement

The normative contract is
[Bounded Rule Exploration with `? explore`](bounded-rule-exploration.md). This
workbook turns it into the shortest coherent implementation path. Superseded
Cartesian and probe-era notes were removed; Git history preserves them without
leaving a second apparent design in this document.

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

The architecture is:

```text
finite dependent FROM relation
        |
        v
finite TO successor fibers
        |
        v
RelationId + stable CaseIds + authenticated open frontier
        |
        | WHERE
        v
AdmissionId
        |
        | FIND
        v
QuestionId + selected cases
        | \
        |  +-----------------------------+
        v                                v
case ViewIds --chosen target--> MechanismRequestIds
                                           |
                                           v
                              mechanism-incidence ViewIds
```

There is no public probe phase. Candidate endpoints, source events, midpoints,
region certificates and singletons are prioritized work nodes in one resumable
stream.

## The smallest complete source example

This sketch deliberately explores a multidimensional profile relation rather
than a fixed person:

```runa
? explore income_cliffs {
    from {
        profile in coherent_profiles(profile_space)
        income in supported_income_coordinates(
            profile,
            range(0, 1_500_000)
        )
        context = SalaryChange(amount_kroner = 1)
        before = state_for(profile, income)
    }

    to after = apply_salary_change(before, context)

    where before supported(before)
    where after supported(after)
    where transition permitted(before, after, context)

    find violations of resources_never_fall(before, after)

    results cliffs from selected {
        each case
        measure [
            loss_ore = resources_ore(before) - resources_ore(after)
        ]
        select [case_id, context, profile = before.profile, before, after, loss_ore]
    }

    results case_summary from selected {
        group all
        aggregate [
            cases = count_distinct(case_id),
            affected_profiles = count_distinct(before.profile)
        ]
        select [cases, affected_profiles]
    }

    mechanisms cliff_paths for selected from assess_policy

    results mechanism_summary from mechanisms cliff_paths {
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
    }

    results loss_bins from mechanisms cliff_paths {
        group by [
            bin_start_ore = floor_to_bin(
                resources_ore(before) - resources_ore(after),
                5_000
            )
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
    }
}
```

The quotient bindings in that sketch are the structural result surface. An
authored `from mechanisms` incidence row exposes typed `signature_id`,
`structural_mechanism_id` and `execution_profile_id` values. A row is not
evaluated until its durable signature-to-structure assignment exists, and the
exact result-input seal binds both the raw incidence root and structural
quotient root. Consequently `count_distinct(signature_id)` is explicitly a
**raw-signature** summary, while a true mechanism count uses
`count_distinct(structural_mechanism_id)`.

An ungrouped replay view names the same input explicitly and uses incidence
grain rather than case grain:

```runa
results raw_cliff_paths from mechanisms cliff_paths {
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
promising helper name. Its source-coverage manifest classifies every Context and
Before field, plus every reachable producer input, as a varied finite
dimension, a value derived from earlier dimensions, an explicit singleton or
source restriction, an exactly irrelevant input, or a reported model-coverage
gap. A constant buried inside `state_for` is therefore visible conditioning,
not an invisible default. This is derived evidence about the ordinary source
program; it does not require another clause in the query language.

Relevance is an execution optimization, not permission to erase population.
If church status is proved irrelevant to the question and mechanism observer,
the search may evaluate one behavior representative while retaining the exact
support of both statuses. The completed case and affected-profile counts still
range over the declared relation. Intersections that are relevant only in
combination remain separate decision cells, which is where quirky profiles can
emerge without a blind Cartesian evaluation.

The exact finite range is end-exclusive, so lower endpoints end at 1,499,999
and the singleton successor may reach 1,500,000. The number is a declared
question bound, not a suspected threshold. The initial implementation work and
small experiments must not launch this whole range merely to get an early
number.

The first honest runnable Personskat audit may be a **conditioned bootstrap**:
choose one source-backed coherent profile explicitly, vary lower annual income
coordinates over `0..<200_000` DKK and let the final `+1 DKK` successor reach
exactly 200,000 DKK. Its source-coverage manifest must report every singleton
and restriction. Completion then means exact over that declared one-profile
relation; it does not mean exact over all persons, municipalities or profile
constructors. This is a legitimate first audit of the durable pipeline, not a
population result. The bootstrap binds `before in range(0, 200_000)` directly
after its singleton context. It does not introduce an auxiliary `income`
dimension merely to copy it into `before`: that redundant dependent singleton
would add roughly one source fiber, receipt and traversal edge per income
without adding a case or proving anything new.

The first **population audit** at the same income horizon is a separate
milestone. It ranges over a declared genuinely multidimensional coherent
profile relation, publishes its coverage manifest and closes every source,
successor, admission and FIND frontier. Either audit may produce an exact empty
cliff relation in the current model. Empty is useful evidence only when all
frontiers required by that audit close; it is never inferred from seeing no
sampled case. A separate tiny synthetic fixture deliberately contains a shared
nonempty mechanism so the integration milestone exercises case-to-mechanism
incidence and a post-mechanism view even when Personskat below 200,000 DKK is
empty.

Those components are now connected for the conditioned bootstrap; its first
attempt nevertheless stopped during preparation before semantic replay. The
conditioned bootstrap may use concrete enumeration if it fits the resource
envelope. The population audit remains gated on the proof portfolio so it does
not blindly multiply profiles by income where exact cells are available.

`to after in successors(before, context)` is the multivalued form. A household
planning query uses it when one coherent source has zero, one or many candidate
After plans. This is the decisive generalization beyond a fixed mixed-radix
product.

## What each clause owns

| Clause | Owns | Does not own |
|---|---|---|
| `from` | the finite source world and producer lineage | case validity or findings |
| `to` | the finite successor relation | the rule mechanism explaining an endpoint |
| scoped `where` | admission classification | source identity |
| `find` | selected all/matching/violating cases | presentation or mechanism identity |
| `results NAME from sources` | a grouped view over canonical `(Context, Before)` source rows | successors, admission, findings or auxiliary lineage |
| `results NAME from selected` | a named typed case projection/reduction; `from selected` may be omitted | base cases or classifications |
| `mechanisms NAME` | explicit endpoint replay and signature incidence | optimum or grouping proof |
| invocation limits | safe scheduling and pause behavior | semantic query identity |

No profile fact is implicitly fixed. When a question intentionally conditions
on Copenhagen, filtering the profile producer declares that smaller source
world and changes `RelationId`. A `where before` condition instead classifies
cases in the already declared relation and changes `AdmissionId`. An optimizer
may push the predicate physically but may not move it semantically.

Result grain follows the input relation. A selected-case view may use `each
case`; a raw mechanism-request view may use `each incidence`. A source view
currently uses `group all` or `group by` over canonical `context` and `before`;
auxiliary producer bindings are lineage rather than source-row columns. Any
input may use grouped grain, and only closed-group grains admit `aggregate`.
The implemented aggregate reducer is explicit
`NAME = count_distinct(EXPR)`. The checker rejects a case/incidence grain
mismatch rather than silently changing row identity.

Group, measure and aggregate declarations introduce ordered intermediate names;
`select` declares the public output schema and may project an earlier value with
the same bare name. Duplicate intermediate or selected output names are
rejected.

## The identity ladder

The implementation should make each boundary concrete before building the next:

```text
RelationId
  -> AdmissionId
    -> QuestionId
      -> ViewId(case input)
      -> MechanismRequestId
        -> ViewId(mechanism-incidence input)
```

- `RelationId` seals normalized FROM+TO semantics, stable model/type owners,
  schemas, set normalization and lineage. It excludes names, WHERE, FIND,
  views, mechanisms and execution policy.
- `AdmissionId` adds the normalized scoped WHERE conjunction.
- `QuestionId` adds FIND all/matches/violations.
- `ViewId` adds its typed input relation and view semantics.
- `MechanismRequestId` adds its question, explicit target, endpoint observer
  and signature normalization.

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

The same certified leaf can feed two deliberately different mechanism indexes:
the full case-space support keyed by `(MechanismRequestId, SignatureId)`, and a
derived distinct `(Context, Before)` starter-support projection. The latter
must retain dependent/correlated cells or an equivalent checked predicate;
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

The concrete fallback remains output-sensitive and ultimately `O(N)` endpoint,
admission and question evaluations, plus source-prefix work. It is the
correctness floor. Current hot paths are:

| Current operation | Current time | Retained or peak memory |
|---|---:|---:|
| keyed source/case/classification/work/evidence insert | normally `O(log X)` plus payload validation | one canonical record; singleton successor and provenance collections stay inline and promote only when they actually branch |
| resume a depth-`d` source prefix | `O(d)` fiber evaluations and set normalization | current prefix plus one materialized fiber |
| enumerate one configured quantum | `O(k)` evaluator steps after the fiber opens | `O(k)` unapplied events; production fused work cold-starts at `k = 1`, then adapts toward five seconds with `k <= 256` |
| reconstruct the verified case-chunk partition | `O(B)` once per cold journal replay; later slices/chunks/runs use indexed lookup in the replay-derived opaque authority | one `O(B)` partition/binding cache, currently `B = 782` for the 200k audit |
| accept one classified chunk | `O(r log C)` keyed validation/append for `r <= 256` homogeneous runs, including causal root and opposite-obligation evidence-index lookups, plus bounded addressed-chunk reverification | `O(r)` exact-key undo log (`<= 9r + 3`); no accumulated support-catalog clone, proof scan or whole-partition rebuild |
| derive public classified counts | `O(C log C + M log C)` over the case-root-reachable support topology and facts | `O(C + M)` topology/key indexes while all semantic cells and evidence remain borrowed; no support snapshot or payload clone |
| close classified support | `O((C + O + M) log (C + O + M))` full validation plus canonical hashing at each crash-safe seal boundary | derived key/ID validation sets only; no journal or support snapshot clone |
| accept one concrete selected run | `O(k log N)` collision/classification preflight and merge for `k <= 256` cases | `O(k)` batch-local relation delta; no relation/admission/FIND prefix clones |
| finish source traversal | `O(P + S + E)` ordered reachability/root validation over prefixes, sources and traversal edges | current `O(P + S + E)` reachability scratch; the terminal receipt itself is a fixed 212-byte body |
| relation/admission/FIND closure | `O(N)` coverage validation | relation rows plus `O(A + Q)` decisions |
| result evidence or projection-record insert | `O(log R)` in canonical indexes | one bounded record plus its reverse/index entry |
| publish an ungrouped or choice-bearing row view | `O(R)` deterministic reducer/projection rebuild, once per process resume | current `O(R)` ephemeral row-state reducer plus cached bounded records; durable terminal state is constant-size |
| resume bounded result projection publication | one `O(P)` cold prefix validation, then `O(delta)` for each newly durable suffix; sealed evidence-root checks are `O(1)` | one `(validated length, prefix root)` cursor per active view; no prefix-root array |
| publish a grouped no-choice view, including `count_distinct` | `O(R log G + sum(aggregate_count * group_size * log group_size))` after fresh equality-checking all `R` durable rows | `O(R + G)` borrowed references and up to `O(R)` exact distinct members per aggregate pass; no second owned contribution catalog or per-row binding map |
| optimize one group | `O(K)` | `O(K)` objective candidates |
| current Pareto choice | `O(K^2 * objectives)` worst case | up to `O(K)` survivors |
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

Coverage has two independent axes. Exact exhaustion or accepted proof closes
the explicitly declared relation. The checked source-coverage manifest
separately answers whether that relation spans every profile field and
constructor choice relevant to a broader claim such as “all encoded persons.”
A coverage gap or explicit singleton qualifies that breadth claim but does not
turn an exact count over the smaller declared relation into a lower bound.
Conversely, a broad manifest does not prove a mapped projection injective or a
result cell uniform.

Closure is layered and cannot be inferred downstream:

1. the relation is extensionally complete only after source enumeration and
   every discovered source's successor fiber are sealed;
2. admission is exact only after every CaseId has one decision;
3. FIND is exact only after every admitted CaseId has one selection decision;
4. certified support is exact only when every declared active obligation leaf
   has accepted evidence and all partition/refinement fronts are closed; and
5. full analysis is exact only after every named result is published and every
   mechanism target has one terminal replay outcome or typed unavailability.

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
yielded it. The unified scheduler now catches selected-result and
selected-mechanism work up to the current authenticated prefix before granting
one more base quantum. Result specs/evidence and mechanism target/replay events
therefore appear while FIND is open; only input/target seals, publications and
global closure wait for exact FIND closure. One quirky profile case may be
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

The canonical frontend and closed IR represent the ordered dependent
FROM/TO/WHERE/FIND query and its post-FIND analysis dependency DAG. The branch
also contains content-stable relation/admission/question catalogs, lazy source
and successor evaluators, a readiness-driven indexed work DAG, compact exact
traversal closure, SupportCells and proof obligations, the streaming semantic
journal fold, bounded work compaction, readiness-driven selected-case result
and mechanism scheduling, bounded result-projection publication, a unified
semantic DAG scheduler, the canonical safe-subset codec, immutable segmented
byte store, durable journal coordinator, resource-admitted slice loop, prepared
warm epoch and public `runa explore` product path.

The four-transition `relational-explore-stream-smoke.runa` query has closed
through the current publication-v7 path with four exact cases, two selected
cases, one shared structural mechanism and execution profile, two incidences,
and all eight planned artifacts caught up to the same journal prefix. The
first proof-bearing case-image artifact now installs
root injectivity and optional exact cardinality atomically, with checked durable
restoration and proper-prefix recovery for both resolver completions; positive
weighted SupportCells still cannot feed result reducers, and chosen-view
mechanisms remain deferred. The unified
scheduler now joins readiness-driven selected results, mechanism-incidence
results, selected-target replay and base enumeration. The production checked
interpreter supplies fresh Before/After rule and branch traces from the
artifact-owned program snapshot. Mechanism traces now cross the durable store
as bounded, prefix-resumable artifacts with checked private restoration.
The public selector now routes relational syntax only through this path and
fails closed rather than falling back to the v0 Cartesian, ordinal or probe-era
executor.

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
static checks but has not yet supplied semantic evidence.
This is an exact result for the declared coarse relation, not a certificate for
every one-krone transition or every profile. The authenticated external run
closed at journal sequence 130 and head
`81eaaa3bbd0089501dc9a2af7574762a7df8d0b0474673ddb223073265a6cf32`.

The complementary conditioned mechanism-landscape entrypoint is now authored
with the same 2,000-edge relation, `find all`, raw admitted-edge and successful
incidence views, typed unavailable terminals, closure-qualified per-signature
support, 1,000-DKK income bins and 50-DKK modeled net-change bins. These
mechanism views become exact for the complete admitted target only after their
frontier closes without replay-unavailable edges. Because `find all` gives
`selected = admitted`, this first implementation
can replay every admitted edge without prematurely adding the admission-scoped
target syntax. The complete architecture still reserves `for admitted` as an
AdmissionId-scoped target for combined questions.

Before starting that nonempty 2,000-edge replay, the durable mechanism path is
being normalized: one complete signature definition is journaled once, and
case incidences reference it with compact transition/replay receipts. Warm
adjacent endpoint proposals are already reused, so a linear `N`-edge landscape
needs `N + 1` endpoint evaluations inside an uninterrupted epoch. This removes
both avoidable evaluation and avoidable trace-payload repetition before a
longer run. No through-1,500,000-DKK stream should start at this checkpoint.

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

The scheduler wiring is not the hard part of that next proof. The conditioned
Personskat WHERE and FIND expressions cross checked helper and rule calls. An
`AdmissionId` or `QuestionId` commits their meaning, but a digest alone is not a
replayable theorem. The symbolic fast path therefore needs a canonical checked
classification-rule-graph capsule under the support plan: specialized fixed
context, hash-consed reachable rules, and a paired Before/After delta DAG. A
proof artifact must replay against that capsule before it can mint uniform or
partitioned admission/FIND evidence. Literal predicates are a useful narrow
producer recipe; they are not evidence for the Personskat calls. Until the
capsule and its proof producer exist, the exact 200,000-candidate count may sit
beside honest concrete lower bounds and the fused evaluator remains the
canonical completion path.

Once positive selected fragments are certified, downstream adoption should be
incremental. First expose exact logical admission/FIND totals, then support the
one algebraically safe weighted result: `group all` with checked
`count_distinct(case_id)`, whose value is the disjoint selected-population
cardinality. General grouping, measures and mechanism incidence require typed
uniform values or further fragment refinement. Fragment count is never case
count, and symbolic closure must not fabricate extensional CaseIds, row-set
roots or mechanism-incidence roots.

### Checked classification capsule

The minimal replayable proof program should have two identities. A
`ClassificationGraphRoot` commits the canonical typed semantics and permits
process-local reuse. A `ClassificationCapsuleId` additionally commits the
checked program, relation/admission/question IDs, support-plan/root-cell IDs,
fixed-value specialization and provenance. Proof artifacts bind the capsule,
not a runtime function name, AST address, dispatch-key spelling or digest by
itself.

The V1 graph needs typed input/constant/constructor/projection nodes; ordered
integer and Boolean operations; `if`/`match`; pure closed function calls; and
the exact checked rule-family dispatch order and miss behavior. Local immutable
bindings become DAG edges. Recursive SCCs, effects, open captures, dynamic or
higher-order calls, unresolved members, arbitrary collection traversal and
arithmetic whose overflow/rounding semantics are not proved remain explicit
residuals. A residual means only concrete evaluation is required.

Fixed context may be specialized only from an exact singleton support witness.
Before and After use the same parameterized observation DAG in two lanes:
context-only nodes run once, equal branch predicates share an arm, and endpoint
results may be cached by complete canonical dependency tuples. This is physical
acceleration only; caches are bounded and absent from evidence roots, and fresh
mechanism replay remains authoritative.

Proof replay deterministically rebuilds the capsule from the exact checked
query, requires complete identity equality, abstractly evaluates the obligation
cell, rechecks totality/overflow/dispatch/match conditions, and reproduces the
typed conclusion before a private gateway can mint evidence. Start with exact
constants, integer interval/congruence/quasi-affine values, Boolean truth sets
and exact constructor tags. A varying truth set proposes certified cuts; it
does not classify the parent. With graph size `G` and `P` retained proof
fragments, the direct bound is `O(G * P + P log P)` time and `O(G + P)` memory;
a successful global abstract proof is `O(G)`, while worst-case `P = Theta(N)`
must fall back to concrete evaluation.

Implementation order is: expose a producer-owned checked-query/semantic-seal
seam; lower the acyclic canonical capsule; use its exact paired interpreter as
optional concrete acceleration; add abstract scheduling cuts; then add replayed
typed admission and staged FIND proofs, followed by certified partitions. This
keeps the first optimization useful even when a full Personskat theorem remains
out of reach.

### Ordered exact fallback without population-sized state

An exact injective mapped image of an ordered integer range with a singleton
successor has a stronger fallback than retaining one journal-state row per
coordinate. First partition the root structurally into bounded ordinal chunks.
Within a chunk, evaluate adjacent endpoints in order and partition that chunk
again into maximal homogeneous rejected runs, admitted/not-selected runs, and
selected fragments. Selected fragments remain concretely materializable for
views and mechanism replay; the other runs carry typed admission/FIND evidence
and exact cardinality without fabricating extensional `CaseId`s.

Each accepted sweep batch is bound to the checked query, support-plan root,
root cell, mapped-image materializer, starting cursor and ending ordinal. A
pause commits only a completely covered prefix. The next batch must begin at
that exact cursor, so replay cannot skip or overlap coordinates. A private
checked-executor gateway gives a concrete exhaustive batch the same evidence
boundary as individual classifications; later a classification-capsule proof
may discharge the identical chunk obligation without evaluation.

For `N` adjacent edges the black-box lower bound remains `N + 1` distinct
endpoint observations, which the adjacent-value memo already attains. If `B`
is the number of chunks, `R` the number of homogeneous outcome runs and `X` the
selected cases retained for downstream work, retained semantic state becomes
`O(B + R + X)` plus one bounded chunk, rather than necessarily `O(N)`. The
honest worst case remains `R + X = Theta(N)`. Binary search and probes can rank
where to work next, but cannot certify skipped coordinates unless interval or
rule-graph reasoning proves the skipped cell uniform.

## Implementation checkpoints

### 1. Canonical frontend and closed relation IR

- Parse only named FROM/TO/WHERE/FIND Explore queries.
- Accept singleton and exact-finite successor relations.
- Type and purity-check ordered dependent bindings.
- Reject the old compact syntax rather than adapting it.
- Close stable model owners, schemas and RelationId independent of views.

### 2. Content-stable relation frontier

- Discover and deduplicate source rows.
- Open, advance and seal each source's successor frontier independently.
- Preserve provenance support without adding cases.
- Make partial snapshots canonical and `finish` fail on an open frontier.
- Resume under a new journal schema without ordinal CaseIds or a probe barrier.

### 3. Admission and question layers

- Normalize scoped WHERE conjunctions into AdmissionId.
- Normalize FIND all/matches/violations into QuestionId.
- Key classifications by semantic layer plus CaseId.
- Reuse the same relation evidence for another authorized admission/question.

### 4. Named result dependency DAG

- Implement case views over selected cases.
- Add deterministic grouping, measures, aggregates, `having`, selection and
  all-ties/Pareto choice.
- Resolve names to IDs and reject cycles.
- Keep classification independent of projections and privacy authorization.

### 5. Named mechanism requests

- Validate one typed pure `(State, Context) -> Observation` endpoint callable.
- Accept either a static totality proof or complete finite target replay;
  persist explicit unavailable terminals and degrade signature/result certainty
  whenever replay cannot close.
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
- Add post-mechanism views such as distinct signatures per 50-DKK loss bin.
- Keep optimum proofs and mechanism explanations separate.

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
- Confirm that irrelevant dimensions close as irrelevant.
- Confirm that every compressed behavior cell preserves exact disjoint profile
  and case support.
- For each audit, stream exact/open case and profile evidence and, where
  selected cases exist, replay-derived mechanisms and 50-DKK loss bins.
- Accept an authenticated exact-empty Personskat result at either scope; use
  the nonempty mechanism fixture, not a planted tax threshold, to cover the
  mechanism-incidence path independently.
- Widen income and profile domains organically while watching frontier shape.
- Start the through-1,500,000-DKK stream only after the implementation avoids
  per-profile/per-krone waste where the model admits exact cells.

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
  starters/selected_cliff_node.ndjson
  graphs/case-support.ndjson
  graphs/case-transitions.ndjson
```

The `starters/` lane is explicit and single-subject. Publication v9 schedules
one artifact only for an authored `starters NAME from mechanisms REQUEST ...`
consumer whose `using values from VIEW` reference names a checked lossless
selected-input `each case` view authorizing CaseId, Context, Before and After.
Its absence is `not_materialized`, not an empty-support claim. Adding another
consumer to a completed run is an additive publication operation: it leaves
the existing journal, analysis roots, and prior artifacts untouched while
adding its content-addressed artifact and updating cursor/manifest state.

`graphs/case-transitions.ndjson` is the corresponding case-side semantic
artifact. It is automatically authorized only when one checked selected-input
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
mechanism request reports its own `open`, `exact`, `unknown` or `unavailable`
frontier plus evidence/result roots. An NDJSON row is one bounded authorized
projection or incidence, so exporting many concrete configurations does not
require building one giant array. Grouped views use the same NDJSON envelope,
one authenticated projection record per line, so a large histogram also stays
bounded and resumable.

An authorized starter-support view publishes correlated `(Context, Before)`
cells, their inner/outer support status and optional searchable marginals. It
is separate from the mechanism definition stream: changing the explored
relation, target or open frontier may change starter support without changing
the causal structure of an already known signature. Raw source values remain
subject to the view's explicit privacy projection.

An explicitly **raw-signature** support view remains useful for replay
diagnostics:

```runa
results raw_signature_starter_support from mechanisms paths {
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

The compact all-subject sidecar uses a factorized summary rather than eagerly
building every correlated union. Disjoint signature-fiber weights give its
case bounds; the largest inspected single-fiber starter count and sealed target
starter projection give honest starter bounds until deduplication. Automatic
publication inspects at most 256 canonical signature-fiber summaries per
subject. Crossing that schema-fixed cap records a capped scan and wider bounds;
it never falls back to constructing the full union. Every row says
`starter_projection.status: not_materialized` and references a stable,
content-addressed projection plan. It also carries authenticated inner/outer
fiber-expression identities over
`SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>`: the factorized inner
union and the same union extended by the shared residual or opaque target
obligation. Equal identities mean the expression bounds have collapsed; they
do not mean typed cells were serialized. The plan is authorization-neutral and
does not authorize starter cells; a selected export derives its job identity
from the plan plus checked publication authorization. The target generic job
uses exact key cursors and bounded merge pages so pause/resume never requires
retaining the complete union. Exact starter-set evidence remains distinct from
exact conditional-successor evidence.

Publication v9 specializes that job for one explicitly selected structural
mechanism, activation/differential node, or activation/differential edge:

```runa
starters selected_cliff_node
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
using values from cliffs
```

It writes `starters/<name>.ndjson` in pages of at most 64 members, adaptively
shortens a page to the byte limit, and uses a k-way merge whose peak memory is
proportional to the contributing raw signatures plus one page. V1 has no
wildcard or list selector; multiple subjects require multiple named consumers.
The declaration belongs to the appendable publication-consumer graph, not the
analysis DAG, so copying a node ID from an existing structural sidecar and
resuming does not re-explore any cases. A fixed-fan-in external merge and
path-conditioned selectors remain future scaling work. The whole structural
catalog MUST NOT be eagerly materialized merely because it exists.

Publication schema v9 splits each mechanism request and authored projection into independently
resumable artifacts. `mechanisms/<name>.ndjson` is the answer lane: its compact
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

`mechanisms/<name>.structural.ndjson` is the quotient-and-support lane. It
streams raw-signature-to-structural assignments, the exact structural closure,
then one lazy support summary for each canonical whole mechanism and each
node/edge activation or differential-participation subject. Every summary
names its origin-preimage case and distinct-starter bounds, factorized support
root, bounded signature-prefix root, shared residual, upper-bound provenance
and authorization-neutral projection-plan ID. Exact starter-key saturation is
kept distinct from an exact correlated successor fiber: the automatic row never
claims or emits a correlated root, even when its scalar starter count is exact.
The normative conservative envelope remains
`conservative_target_projection_upper`. The
artifact publishes a factorized summary (`cells_serialized: false`) and marks
the correlated projection `not_materialized`. Its flat cursor counts subjects,
never case-by-node pairs; automatic publication never constructs a full
subject union. The final support receipt binds the target, raw incidence and
structural quotient roots.

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
v7 artifacts contains replay-only state or context values.

The automatic `graphs/case-support.ndjson` artifact is the complementary
public reflection of the authenticated case/support prefix. It has two honest
projection shapes rather than pretending every scheduler path creates the same
proof artifact. A partitioned classified run emits a root, then complete chunk
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
case-by-node table. Publication v9 treats this new lane as an additive
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
tagged with the journal prefix that authorized it. Group aggregates, `having`,
choice results and mechanism histograms cannot pretend to be exact early: they
publish lower-bound/provisional observations while open and an authenticated
projection closure when their declared input closes. Thus observability does
not require delaying every useful case until global exhaustion, and early
output never turns a partial prefix into a completed answer.

The human completion line is derived from that manifest, for example:

> Exact over the declared conditioned profile: 11 structural mechanisms explain
> 3,000 concrete cliff cases; 2 raw-signature support frontiers reached their
> declared support cap. See `views/loss_bins_50_dkk.ndjson` and
> `views/cliffs.ndjson`.

The numbers above are illustrative. “Exact” may appear only when the named
frontiers actually close; otherwise the same sentence must say `at least`,
`open`, `unknown` or `unavailable` at the affected layer. The selected
configurations file contains only fields authorized by the view's `select`
clause. Retention limits govern examples, not exact scalar counts.

## Acceptance signal

Before allowing a long Personskat semantic slice, the cumulative small durable
gate requires:

- canonical frontend-to-relational-driver selection with no Cartesian route;
- append, pause, immutable-segment flush, reopen and semantic streaming replay;
- stable CaseIds and byte-identical roots across at least one pause/resume;
- exact relation/admission/FIND closure, including an exact-empty terminal
  path;
- selected-case result publication from the sealed question;
- governor-controlled dispatch and resource-pressure pause without host
  instability; and
- a publication coverage manifest that exposes every fixed bootstrap fact.

The deliberately nonempty
`examples/relational-explore-stream-smoke.runa` fixture has now demonstrated
the public frontend-to-publication happy path: four declared transitions, two
selected cases, one shared replay-derived raw signature, one shared structural
mechanism and execution profile, one two-case incidence aggregate, and exact
two-case/two-starter support closure. All eight planned artifacts caught up at
journal sequence 107 and head
`fb37a53cac23fd1c4cee5da2508824f694ca11091d7696e60acf4fafbcba3d46`.
Its case graph contains one classification-summary root, three exact outcome
regions, the two authorized selected cases and one exact closure; those CaseIds
equal the two mechanism terminal CaseIds. Reopening the identical query and
directories appends zero semantic events and zero publication lines while
preserving the journal head, analysis roots and artifact digests. It exercises
the same stream without pretending to be a tax audit. An induced resource-pressure
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
- all three FIND forms over one admitted relation;
- at least two distinct cases sharing one replay-derived mechanism signature;
- a case view, a view-targeted mechanism request and a post-mechanism aggregate
  view in one acyclic dependency DAG;
- honest partial snapshots and an exact empty or nonempty terminal result; and
- resource-pressure pause without host instability.

Only after the second signal should profile breadth or the income horizon
widen. The desired large-run output is not merely a case count: it is a
closure-aware count of selected cases, affected profile projections and
structural mechanisms, with raw-signature and execution-profile counts retained
separately, named views such as the 50-DKK structural-mechanism histogram, and
resumable authenticated evidence handles.

The first compact answer envelope therefore stays bounded: it leads with the
selected before-to-after population, reports each mechanism request's
closure-aware structural count, and exposes the sealed target's distinct
starter count separately from cases. The latter is request-wide support, not a
per-node count. Per-mechanism, node and edge starter conditions remain
correlated authenticated support overlays referenced through the publication
manifest. Report v5 may inline a schema-capped prefix of a small exact grouped
view directly from its authenticated projection journal; its evidence roots
and truncation cursor make that preview auditable. The operational artifact
index names the corresponding full NDJSON plus every case/mechanism/starter
artifact and its catch-up state. Bulk case rows and large grouped histograms
remain streamable NDJSON rather than one in-memory answer array.
