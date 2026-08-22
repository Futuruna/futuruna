# Turning Rules Inside Out with `? explore`

Status: implementation workbook for the Experimental feature

Futuruna normally answers:

> What do these rules produce for these facts?

`? explore` asks the reverse question:

> For which permitted facts does this property hold or fail?

You name the property and define a finite world. Futuruna finds the values. You
do not supply a list of suspected thresholds.

The normative contract lives in
[Bounded Rule Exploration with `? explore`](bounded-rule-exploration.md). The
compiler parses and type-checks the five search clauses today. The current
development slice contains the human-only capped exact-finite compatibility
executor and the first macOS-supervised durable observable coordinator:
immutable run identity, a checked source-probe phase, candidate-first
evaluation, authenticated frontier deltas, bounded canonical snapshots and
exact restart from an owner-supplied run-state directory. The append-only
journal pause is the authoritative resume checkpoint. Snapshot v5 and its
optional case DAG are a separately admitted materialized view: when deadline or
resource admission denies that phase, the invocation returns a typed
`JournalOnlyCheckpoint` at the final paused cursor without a snapshot blob or
canonical payload. If the phase is admitted but its publisher reports
capacity, a separate bounded `snapshot_unavailable` receipt makes that cursor
observable without publishing partial semantics or blocking later work.
Explicit `--finalize` can atomically replay, publish and
seal a small enough closed answer; a larger answer pauses at an honest
finalization limit for future chunking. An explicit durable
`--case-graph full` request now enables
bounded, all-or-nothing publication of a total current-evidence case DAG.
A private developer-only mechanism path now executes two deliberately narrow
profiles. The first pairs two positional shown calls to one checked top-level
function whose body is exactly one `if`; it can also populate private half-open
bin incidence from a checked numeric shown value. The second observes the
canonical shown-value evaluation in place when a common checked endpoint makes
one direct positional call to one checked helper and that helper executes one
`if`. Both paths fresh-replay confirmed matching cases, append complete
signature observations, and publish count-only mechanism checkpoints that
survive crash/reopen. Mechanism replay V1 owns an immutable checked root-module
snapshot and refuses every external Futuruna import before stream creation;
import support waits for a boundary-preserving frozen module graph rather than
rereading live files or flattening module initialization. The nested profile is
now driven by the same bounded invocation lifecycle as ordinary exact Explore:
source probes run first; each confirmed mechanism replay is individually
admitted under the 80% resource envelope; that backlog is drained before
another CaseId is classified; and every orderly stop publishes the existing
count checkpoint before its pause when view work is admitted. A pre-probe stop
is journal-only because the mechanism checkpoint intentionally requires the
completed probe milestone. The paths have no CLI selector, general
multi-event/rule-call
tracing, public bin surface, or mechanism-DAG/terminal publication yet. Those
surfaces, user-authored
`probes`, typed `output as` rows, `after`, detached following, parallel workers
and resumable chunked terminal publication are subsequent slices described
below. Ordinary snapshot v5 therefore still reports mechanism evidence as
unavailable and never infers a mechanism count from result groups. This is the
implemented artifact contract, not a claim that a graph-bearing policy
exploration has already been executed.

The smallest executable mechanism-stream fixture deliberately declares only
four incomes, of which three are eligible lower boundary endpoints around one
thresholding `if`. Its first post-probe checkpoint is `scope_open` with zero
confirmed evidence and an honestly `unknown` mechanism-signature count. After
case classification and fresh mechanism replay, selected members of the final
private canonical checkpoint are:

```json
{
  "status": "matching_closed",
  "target_cases": { "certainty": "exact", "value": "3" },
  "traced_cases": "3",
  "known_target_untraced": { "total": "0" },
  "mechanism_signatures": { "certainty": "exact", "value": "3" }
}
```

The fixture first publishes its `scope_open` probe checkpoint. It then commits
one exact classification, verifies that the newly confirmed mechanism rank has
priority over the next classification, and drops the live coordinator without
a pause. A fresh bounded invocation recovers that pending rank from the
authenticated journal, drains it first, alternates the remaining classification
and mechanism work, and publishes the `matching_closed` checkpoint. Reopening
again has no replay backlog. The excerpt is for readability; each committed
artifact is one cursor-bound canonical JSON line with the complete conservation
fields and hashes.

## Five search clauses, an optional probe plan, plus a continuation

| Clause | Meaning |
|---|---|
| `over` | The Boolean rule Futuruna should investigate |
| `find` | Whether Futuruna searches for cases where the rule fails or holds |
| `bounds` | Every value each relevant input may take |
| `boundaries` | Which integer input is compared with its following value |
| `probes` | An optional finite initial scheduling plan inside the same resumable run |
| `output` | What counts as one finding and which case should be shown |
| `after` | Optional code receiving the terminal typed report after search |

Neither `probes` nor `after` is another search input. A probe plan changes
scheduling, not the world or answer; `after` runs only from the terminal sealed
report after enumeration, replay and sorting.

The existing `output { ... }` form remains CLI-only. The specified
`output as Row` form lets Futuruna code consume the result afterward.

`find violations` looks for cases where the rule is false. `find matches` looks
for cases where it is true. Bounds define the world. Boundaries define the
movement. The output key defines what one answer means.

## 1. Begin with a property

Consider a synthetic support rule. Support disappears when income reaches
100,000. The rule is intentionally simple so the exploration contract is easy
to see.

```runa
# Household = Single | Couple

> support(household: Household, income: Int) -> Int {
    if income < 100000 {
        match household {
            | Single -> 10000
            | Couple -> 15000
        }
    } else {
        0
    }
}

> available_resources(household: Household, income: Int) -> Int {
    income + support(household, income)
}

> loss_after_next_step(
    household: Household,
    income: Int,
    step: Int
) -> Int {
    available_resources(household, income) -
        available_resources(household, income + step)
}

| next_step_never_hurts(
    household: Household,
    income: Int,
    step: Int
) ->
    available_resources(household, income + step) >=
        available_resources(household, income)
```

The question rule states the property we hope is true. It does not contain a
suspected failing income.

## 2. Define the complete world

The first query asks where the property fails:

```runa
? explore support_cliffs {
    over next_step_never_hurts(household, income, step)
    find violations

    bounds {
        household in values(Household)
        income in range(90000, 110000)
        step = 1
    }

    boundaries on income by step

    output {
        key [income_before = income]
        show [
            income_after = income + step,
            household,
            available_before = available_resources(household, income),
            available_after =
                available_resources(household, income + step),
            loss = loss_after_next_step(household, income, step)
        ]
        representative maximize loss
    }

}
```

Nothing in the query mentions `99999`. Futuruna receives an income interval and
the property; it finds the failing step.

### Ways to define a bound

```runa
x in [a, b, c]
```

means exactly those values.

```runa
x in named_values
```

means every distinct value in a pure finite list or set. This is the right form
for a dated table or an explicitly curated legal domain.

```runa
x in range(start, end)
```

means every integer from `start` through `end - 1`. The solver can keep the
interval symbolic; Futuruna does not need to allocate a list containing every
integer.

```runa
x in values(Type)
```

means every possible value of a type the compiler can prove finite.

For example:

```runa
household in values(Household)
church_tax in values(Boolsk)
```

`values(Household)` contains `Single` and `Couple`.
`values(Boolsk)` contains both Boolean values.

`values(Int)` is an error because integers have no finite complete set. A
product containing an integer field is also rejected unless that field is
exposed as a separately bounded query input. Recursive and collection-bearing
types require an explicit finite domain.

`values(Type)` means every value of one finite declared type. Use a named list
instead when the domain consists of recorded instances, such as dated municipal
parameters.

```runa
year = 2026
case = make_case(household, year)
```

fixes or derives a value rather than adding another independent dimension.

```runa
where case_is_valid(case, income)
```

restricts the declared world. For a boundary query, validity is required at
both `income` and `income + step`.

Futuruna rejects a query when a relevant input remains unbounded. It never
invents a value merely to make the exploration run.

## 3. Understand the boundary

```runa
boundaries on income by step
```

says that `income` is the transition axis. With `step = 1`, every considered
pair is:

```text
income -> income + 1
```

Both endpoints must remain inside the income domain.

The Boolean rule still defines the property. The boundary clause identifies
the axis, checks endpoint coverage and lets Futuruna focus its explanation and
solver work.

The compiler may notice thresholds, comparisons, integer division, rounding,
caps and rule-branch changes. Those observations can make the search faster.
They do not narrow the answer. The complete bounded Boolean question remains
the source of truth.

### Search the changes, prove the space between them

The attractive algorithm is not "run the tax model three million times." Hold
the other facts in a configuration `c`, call exact net resources `N`, and write
the unit-step question itself:

```text
Cliff(c, x) =
    Valid(c, x)
    and Valid(c, x + 1)
    and N(c, x + 1) < N(c, x)
```

Now the search target is the finite difference
`Delta N(c, x) = N(c, x + 1) - N(c, x)`. Most tax code is piecewise integer
arithmetic. Guards, thresholds, rule dispatch, caps, integer division and
rounding divide the income axis into cells. Inside a supported cell, Futuruna
can often prove an exact lower bound for `Delta N`; if that lower bound is zero
or greater, the entire cell contains no cliff.

The first useful symbolic fragment is Presburger/semilinear: addition,
ordering, constant rates and divisors, congruences, finite tables and finite
piecewise dispatch, all with exact integer semantics. Rounding is why cells may
be an interval split by congruence class rather than one plain interval.

The engine can then climb a refinement ladder:

1. collect guarded branch and arithmetic events from the reachable rule slice;
2. evaluate the event candidates;
3. certify the intervals and congruence classes between them from exact
   finite-difference bounds;
4. ask SMT only about the unresolved residual and refine from each witness
   (CEGAR); and
5. enumerate individual integer points only in whatever finite residual is
   still open.

That last step matters. Candidate extraction is a speedup only. If the proof
cannot close the space between candidates, the engine must expose an open
frontier or check the remaining singletons; it cannot silently call the
candidate list exhaustive.

The durable artifact is a hash-bound `BoundaryPlan`: guarded candidates with
resolved source-event labels, certified interval or congruence supports,
still-open supports and a coverage/disjointness proof tied to the resolved
program and query hashes. A source change invalidates the plan. Candidate
extraction records every unsupported reachable residual and is structurally
unable to close the complement. Its labels explain which source event proposed
a point; replay still decides which dynamic mechanism actually formed a cliff.

This is also why a result can say "35,491 matching configurations" without
storing 35,491 JSON rows. Exact counts are weighted cardinalities of classified
supports. A cap on retained or displayed examples changes only presentation.
A cap that stops classification leaves an open support and makes the count a
lower bound.

### Watch, pause and resume one exploration

A large Explore should not disappear into a process for several hours and then
produce its first meaningful state at exit. Treat it as one durable observable
run. It emits validated evidence as it learns, can be paused at a committed
frontier, and resumes from that exact frontier until the run is completely
sealed.

The accepted design lets an optional finite probe plan give the first scheduling
phase a deliberate shape. This authored `probes` block is not executable yet:

```runa
probes {
    schedule [boundary_candidates, boundary_endpoints, frontier_midpoints]
    lift matches on boundary_axis across [household]
    at_most 64 cases
    retain configuration [household, income]
    retain output [income_before, income_after, loss]
    no_mechanism_trace
}
```

Today the coordinator derives a checked finite source-probe manifest from the
query, falls back conservatively when source analysis cannot certify coverage,
evaluates every proposed candidate normally, and persists the matching query
configuration. The private run-state path belongs to the invocation:

```bash
runa explore support.runa \
  --query support_cliffs \
  --run-state /private/work/support-cliffs.run \
  --pause-after probes \
  --json
```

The explicit `--pause-after probes` asks Futuruna to stop at the inspection
point. Detached `--follow` is a later observer surface. The journal pause is
already sufficient to resume. Futuruna then gets one separately governed
opportunity to materialize the larger observer view. When that phase is
admitted, human mode shows all three cursors:

```text
Run: PAUSED
Run state: /private/work/support-cliffs.run
Stop: probe milestone reached
Final cursor: #42 <paused-journal-head> (paused)
Probe milestone: COMPLETE
Artifact blob: <snapshot-sha256>
Checkpoint cursor: #40 <running-journal-head>
Publication cursor: #41 <publication-journal-head>
```

That admitted snapshot is already useful: it shows aggregate match, nonmatch, exclusion
and closure counts; checked probe phase, candidate counts and commitments;
bounded configuration and representative-result prefixes; and exact or
lower-bound count labels. Individual nonmatch cases and candidate reasons
remain private journal material by default. A durable run created with
`--case-graph full` additionally authorizes its complete current case
classification DAG; mechanism-DAG views still await the general replay path.
Mechanism evidence is explicitly `unavailable_deferred`; result groups are
never relabelled as mechanisms. It is not final while the required frontier is
open. To continue after this inspection, replace `--pause-after probes` with a
positive `--time-limit` and rerun. Futuruna validates the journal identities,
journal head and evidence root, appends a resume record, and continues the open
frontier. With the required time limit and without the explicit pause option,
the first invocation passes directly from the probe milestone into proof and
residual refinement.

If the deadline has expired or snapshot work is not admitted, human mode
instead makes the missing view explicit:

```text
Run: PAUSED
Run state: /private/work/support-cliffs.run
Stop: probe milestone reached
Final cursor: #<n> <paused-journal-head> (paused)
Probe milestone: COMPLETE

Artifact: journal-only checkpoint
Observable snapshot: deferred (<time-limit or resource-admission reason>)
```

There is no snapshot blob, checkpoint cursor or publication cursor in this
form. Nothing has been lost from the resume state: the final journal cursor is
authoritative.

Machine mode wraps the exact stored checkpoint in
`futuruna.explore.invocation.v1`; the invocation schema stays v1 for both
forms. An admitted checkpoint uses artifact kind `checkpoint`, embeds
`futuruna.explore.snapshot.v5`, and names its checkpoint, publication and final
cursors. The checkpoint describes cursor 40; cursor 41 durably names its blob;
cursor 42 commits the pause. `canonical_byte_framing: "json_line_lf"` records
that reproducing the blob digest requires the checkpoint object plus one LF.

A deferred view uses artifact kind `journal_checkpoint` with
`snapshot.status = "deferred"` and a reason of `time_limit` or
`resource_admission`. It has no `blob_digest`, `canonical_payload`, checkpoint
cursor or publication cursor. This operational deferral is not new evidence,
does not change the evidence root or immutable run identity, and does not mean
that the requested case graph was `capacity_limited`.

If the snapshot phase is admitted but its bounded publisher reports capacity,
the outcome is observable rather than deferred. Futuruna publishes a separate
`futuruna.explore.snapshot-unavailable.v1` JSON line, appends
`SnapshotUnavailablePublished`, then pauses. Invocation-v1 uses artifact kind
`snapshot_unavailable`, `snapshot.status = unavailable`, and reason kind
`capacity`; the receipt has a blob plus checkpoint and publication cursors.
Its canonical payload is capped at 4 KiB and contains only cursor hashes and
bounded progress—never a partial configuration, answer, or graph. It reports
only this admitted attempt and does not claim that a later attempt can never
fit.

The next invocation services this pending observer view before doing more
search and stops with `snapshot_catch_up`. Its artifact is the admitted full
checkpoint, the bounded `snapshot_unavailable` receipt if the publisher reports
capacity, or another honest `journal_checkpoint` if that invocation still lacks
time or resource authority. No CaseId is evaluated during catch-up.

A time slice works the same way:

```bash
runa explore support.runa \
  --query support_cliffs \
  --run-state /private/work/support-cliffs.run \
  --time-limit 20m
```

At expiry the library stops dispatch, commits accepted work and the exact open
frontier, and pauses at its next work boundary. If time remains and the view is
admitted, it also publishes snapshot v5; otherwise it returns the journal-only
form above. If the child does not exit within the supervisor grace interval,
the parent contains it instead; the next invocation recovers the last durable
journal head and evidence root. An abrupt process or machine failure likewise
loses only the uncommitted suffix.

When case classification closes, the ordinary invocation pauses at
`classification_closed_finalization_pending`. A deliberate terminal attempt is
another bounded slice. The current CLI requires `--run-state` plus a positive
time limit and rejects `--finalize --pause-after probes`:

```bash
runa explore support.runa \
  --query support_cliffs \
  --run-state /private/work/support-cliffs.run \
  --time-limit 20m \
  --finalize \
  --json
```

Atomic-v1 fresh-replays the selected representative/extrema witnesses,
publishes `futuruna.explore.exact-answer.v4`, and seals only if the full
snapshot, retained replay manifest and terminal JSON fit its conservative
envelope. If not, the same invocation commits a journal pause with typed
`FinalizationLimit` details (`finalization_limit` in JSON); it carries a
snapshot only if that separate view phase is admitted. The exact closed
evidence is preserved for a future chunked finalizer. Repeating the unchanged
atomic-v1 invocation reaches the same capability limit rather than making
chunked progress. The in-process time limit is a work-boundary soft deadline
during this atomic unit. The CLI supervisor may interrupt it, and replay safely
resumes from the last committed event.

The present atomic guardrails are intentionally conservative: all raw groups
must fit the v5 complete raw-group preflight (at most 256 groups, 16,384
recursive value nodes and 4 MiB semantic payload), at most 65,536 selected
replay witnesses may be retained, replay observations may use at most 32 MiB
of canonical manifest bodies, rendered result rows may use at most 48 MiB,
and the complete single JSON document may use at most 64 MiB. These are
finalizer-capability limits, not limits on exact case counts. Reaching one
leaves classification evidence closed and the terminal frontier durable, but
unchanged atomic-v1 cannot advance it past that limit.
Projection labels are also bounded: at most 65,536 labels per projection kind,
at most 1 MiB per label, and at most 4 MiB of UTF-8 label bytes cumulatively
across key, extrema and shown labels. The cumulative cap belongs to snapshot
and terminal schema identity. Before run creation, every checked presentation
string copied into those artifacts—query and axis names, named domain sources,
fact and boundary names, plus projection/having labels—must additionally fit an
exact cumulative 8 MiB canonical-JSON string budget and 262,144 total
occurrences. The occurrence cap bounds retained metadata objects even for tiny
or repeated names. Repeated serialized occurrences are charged repeatedly, and
both limits are schema identity.

The complete snapshot/case-DAG materialization phase has a fixed 256 MiB
accounted working-set envelope. It is admitted under the same 80-percent CPU
and RAM policy and may never borrow the reserved 20 percent of CPU or the
memory reserve, which is at least 20 percent and never below 1 GiB. The current
single-worker run stays in the conservative cold phase, whose
`max(2 GiB, ceil(total RAM / 4))` charge dominates this view envelope. A future
calibrated scan-mode or multi-worker publisher needs a distinct snapshot
charge; it cannot reuse that cold-phase argument.

Case-DAG publication has a separate fixed envelope: at most 256 axes, 65,536
uniform rank runs, 131,072 nodes, 262,144 arcs, 262,144 ordinal intervals and
64 MiB of conservative lowerer-accounted work. The complete nested
`futuruna.explore.case-graph.v1` object must then fit in 8 MiB of canonical
JSON. An admitted requested pause snapshot publishes the entire graph or a typed
`capacity_limited` status with the resource, fixed `maximum`, and honest
`required_at_least`; it never emits a graph prefix. A requested terminal graph
must be included with closed admissibility and polarity. Otherwise finalization
pauses with phase `case_graph_publication` and does not seal.

The graph choice is deliberately durable identity, not a display toggle. Start
the graph-bearing run this way and repeat the option on every resume:

```bash
runa explore support.runa \
  --query support_cliffs \
  --run-state /private/work/support-cliffs-with-graph.run \
  --time-limit 20m \
  --case-graph full \
  --json
```

The report-request digest binds `full` versus `omit`; the retention-
authorization digest binds case-classification disclosure; and the snapshot
and terminal schema digests bind the fixed lowerer and JSON limits. All are
immutable run identity. Futuruna rejects trying to reopen an omitted-graph run
with the option, or a graph-bearing run without it.

The run journal retains every selected `CaseId`, match, nonmatch and exclusion;
the actual lower and upper boundary endpoint values; exact question and
authorized output values; scheduling reasons; proof receipts; and the adaptive
decision transcript. It stores only configuration and output names allowed by
`retain`. An optional mechanism trace needs its own `trace mechanisms`
authorization. Treat the directory as potentially sensitive case data and put
it in an explicitly chosen private location rather than the repository.

#### First domain-neutral execution checkpoint (2026-08-21)

`examples/explore-stream-lifecycle.explore.runa` is a deliberately tiny oracle
for the protocol rather than a policy model. Its compatibility execution
completed exactly: 10 declared assignments, 9 admissible boundary transitions,
2 matching configurations, and 2 result keys (`income_before` 4 and 7). This
establishes the semantic result expected from the durable run without using a
tax-specific income anchor.

The first durable `--pause-after probes --json` attempt did not launch a child.
The outer containment preflight measured 2,055,356,416 bytes available against
its 2,576,980,378-byte early-trip floor and failed closed before creating run
state. No guard was weakened. The next lifecycle attempt should reuse the same
ten-case oracle when host headroom is safe, then demonstrate probe pause,
resume, explicit finalization and `AlreadySealed` readback in order.
After the focused checks, one final retry still failed closed at 2,307,473,408
available bytes against the same floor; it likewise created no run state.

The same protocol path also has a deterministic cardinality-one lifecycle
test that does not depend on live host calibration. It passed checkpoint
rendering, `SnapshotPublished`, `Paused`, automatic `Resumed`, exact case
closure, atomic terminal publication/sealing, sealed replay, and byte-identical
`AlreadySealed` readback. Production still acquires the resource permits around
those shared semantic helpers; only the test omits live telemetry.

The `lift` line is a scheduling hypothesis, not a generalization. If one
household matches at income `99999`, it enqueues the same boundary-axis value
for the other declared household profiles. The journal links those candidates
to the observed origin `CaseId` but marks each one `unevaluated` until Futuruna
executes it. No classification, loss or mechanism evidence crosses that edge.
This is the compositional move we want for Personskat too: discover a plausible
income event cheaply on one profile, then systematically ask whether it appears
across the declared commune, church-tax and commute profiles.

`at_most 64 cases` makes this finite plan complete after at most 64 distinct
classifications. Those validated classifications are already ordinary exact
singletons in the case evidence relation; they are not a published case DAG or
a disposable pre-search sample. The rest of the frontier stays open until
`BoundaryPlan` certificates, exact SMT/CEGAR closure or singleton exhaustion
covers it. Finishing every
source-derived candidate also does not close the complement.

Stale identities, a corrupt journal, overlapping chunks or replay differences
fail closed and preserve the last valid journal head and evidence root. A
changed query or probe plan
starts a new run rather than rewriting old evidence. A pause with remaining
frontier exits `2` and commits a journal checkpoint; it emits snapshot v5 only
when materialized-view admission succeeds. Only the durable `Completed` seal
over an empty required frontier exits `0` as a final answer.

## 4. Decide what one answer means

The most important output line is:

```runa
key [income_before = income]
```

Both `Single` and `Couple` fail at the same income step. Because the key contains
only income, the exploration returns one finding. The expanded output below
also assumes an explicit report request authorizing case counts, the matching
ledger, partitions, a histogram and matching-case mechanism replay. Baseline
output does not publish those case-level views by default:

```text
Exploration: support_cliffs
Status: COMPLETE

Matching configurations: 2 / 39,998 admissible (SOME)
Households with at least one cliff: 2 / 2 (ALL)
Different income steps where next_step_never_hurts fails: 1

99,999 -> 100,000
Representative household: Couple
Available before: 114,999
Available after: 100,000
Loss after the next unit: 14,999

Matching-configuration ledger: 2 replay-confirmed rows
Partition view `households`: 2 exact groups
```

The representative is `Couple` because the query asks for the greatest loss
inside that income-step group.

Change only the key:

```runa
key [household, income_before = income]
```

and the answer becomes two findings:

```text
Single, 99,999 -> 100,000
Couple, 99,999 -> 100,000
```

Both finding counts are correct. They answer different questions. In the
closed case artifact, Futuruna additionally distinguishes 39,998 admissible
full configurations (`D`), two matching configurations (`M`) and one projected
income key (`R`). The exact relation is `|R| <= |M| <= |D|`.

The search space determines what Futuruna examines. The output key determines
what Futuruna counts as one finding. A requested configuration ledger is a
separate population; it never changes the output key.

Status, coverage and graph shape are also separate. This one result is
simultaneously `COMPLETE`, `SOME` over cases, `ALL` for household groups under
an “at least one” view. Household is irrelevant to whether the cliff occurs,
so a reduced membership DAG skips that dimension; it still changes the loss
value and the replayed `match` arm, so value and mechanism views retain it. A
partial or unknown search uses coverage `UNDETERMINED`; `EMPTY` means no
admissible cases, while `NONE` means admissible cases existed but none matched.

### The deeper result: a case graph and a mechanism graph

The result list is only one view. The semantic object is the relation joining
each declared case to admissibility, polarity, outputs and optional replay
evidence. An explicitly authorized artifact may project two linked graphs:

```text
declared configurations
        |
        v
case decision DAG  --------->  projected keys and distributions
        |
        | exact case-to-signature incidence
        v
shared mechanism DAG  ------>  changed rules and branches
```

The **case graph** answers where the property holds. Its decisions follow the
independently varied inputs in source order. Equal suffixes are shared, so a
single node can represent a large exact region. Sharing never forgets the path
that led there: two disconnected case paths pointing at one node remain the
union of those paths, not every crossed combination of their values.

Its terminals preserve what is and is not known: excluded, admissible match,
admissible nonmatch, eligibility still open, or polarity still open for an
already admissible region. That distinction lets Futuruna know `D` exactly
even when it cannot yet decide all of `M`. Missing mechanism evidence is never
smuggled into a case terminal.

This case-side design has a close precedent in Margrave policy analysis:
Margrave uses shared multi-terminal decision diagrams for concrete policy
scenarios, removes irrelevant request attributes, and compares two policies by
their paired decisions. A boundary exploration applies the same idea to one
program at `x` and `x + step`. Futuruna extends it from Boolean request atoms
to typed finite dimensions, explicit open frontiers, exact population counts,
and a separate replay-derived mechanism partition. See the
[Margrave policy-analysis paper](https://web.cs.wpi.edu/~kfisler/Pubs/icse05.pdf).
Margrave also keeps semantic query construction separate from presentation,
warns that materializing every scenario can be enormous, and lets a query ask
which included rule predicates are actually realized. Those are useful design
precedents for Futuruna's separate search closure, case disclosure and
mechanism-observation requests; they do not make a sampled mechanism support
exact. Its ability to infer a sufficient scenario-size ceiling is also the
right conceptual distinction for Futuruna: a proved sufficient search bound
can close a result, while a user-supplied work cap can only leave an open
frontier when it is reached.

The **mechanism graph** answers how the result arose for one fixed query and
observation specification. A fresh replay records stable rule, dispatch and
branch sites. For a boundary question, Futuruna compares the lower and upper
computations when it can pair them soundly, then interns equal differential
signatures. Cases at different incomes or in different municipalities may
therefore point to one shared mechanism. Asking a different observation
question can legitimately produce a different signature for the same input.

Result grouping and mechanism grouping are independent. If `K(x)` is the
output key and `Sigma_(q,h)(x)` is the mechanism signature on traced scope
`T`, then:

```text
same finding:    K(x) = K(y)
same mechanism:  Sigma_(q,h)(x) = Sigma_(q,h)(y)
```

One finding may hide several mechanisms, and one mechanism may span several
findings, loss amounts and disconnected case regions. Their relationship is
the observed set of `(key, mechanism)` pairs; it is never widened to every
possible key/mechanism combination.

This is why “three municipalities have a cliff at 199,999” and “the same
mechanism also causes a cliff at 399,999” can both be true without duplicating
the mechanism node. The case paths stay distinct and both reference the same
mechanism signature.

The link is itself exact: a second ordered decision DAG assigns each traced
case either one complete signature or “outside this mechanism scope.” It can
represent disconnected correlated regions; independent per-field ranges are
not multiplied together and therefore cannot invent municipality/income
combinations that were never cases.

So which comes first? Mathematically, neither graph does. The primary object is
the relation joining a canonical case to its classification, outputs and
optional mechanism signature. The case graph and mechanism classes are two
different quotients of that relation. Operationally, the static rule graph
comes first only as a vocabulary of possible sites. A concrete or symbolic
case supplies evidence about which sites actually formed a mechanism, and an
exact closure proof establishes how much of the case space that mechanism
covers.

The blockchain intuition is useful here, with one important correction. The
declared cases already exist mathematically at `RunOpened`; Futuruna does not
mint reality by discovering them. What it “mints” is validated evidence:
closed singleton or region classifications, replay-confirmed mechanism
signatures, selection-closed representatives and conserved frontier changes.
A candidate or interesting probe is only a proposal until evaluation or proof
accepts it.

Each accepted chunk is immutable and content-addressed. The coordinator keeps
two related roots. The **journal head** records committed arrival order and is
the point another invocation or follower picks up. The **evidence root** hashes
the normalized closed relations plus exact open frontier independently of
arrival order. Two workers may finish in opposite orders and produce different
journal histories but the same evidence root and answer. Parallel workers may
therefore prepare disjoint chunks without making scheduling part of result
identity. This is more precisely an event-sourced Merkle evidence DAG than a
literal blockchain: there is one run coordinator, no mining, token or
distributed consensus, and no reason to force all computation through one
serialized worker chain.

The canonical evidence state also ignores operational batching. Closing one
semantic interval in one event or the same interval as several disjoint pieces
must reduce to the same normalized support map and evidence root. Certificate,
probe and ordinary-evaluator provenance stays visible in the ordered journal;
it does not create different semantic leaves for an otherwise identical fact.
This requires an incremental authenticated interval map, not a full-set rehash
after every discovery.

The crash boundary is equally explicit: prepare a transition without mutating
the live cursor, durably install its referenced blobs and event, apply it to the
in-memory reducer, then publish it. Resume is a replay of complete typed event
payloads. A list of hashes without the supports and state transition they name
cannot be the checkpoint format.

Nor should every block copy the full accumulated frontier. The implementation
uses a persistent authenticated interval tree and records
`previous_frontier_commitment + newly_closed_delta + next_frontier_commitment`.
Replay derives the successor from the delta and rejects a commitment or
conservation mismatch. That makes a scattered candidate-first search pay for
the changed paths rather than repeatedly serializing and hashing its entire
history—the algorithmic property needed before a million-case stream is sane.

Observable snapshots now carry an identity-bound configuration manifest as
well as the answer view. Integer ranges are explicit. Enumerated/finite values
and fixed facts share one global recursive-node and semantic-byte budget;
anything that does not fit retains its cardinality or fact name, source shape
and an explicit omission reason. Result rows separately use a canonical raw-key
prefix with group, recursive-node, semantic-byte and rendered-JSON ceilings.
The snapshot reports how many raw groups were observed and scanned, whether the
scan closed, and whether every displayed count is exact or only a lower bound.
This gives a saved probe enough human context to say which bounds it matches
without turning an accidental domain or result dump into the output contract.
The current v5 identity fixes the configuration budget at 4,096 recursive
value nodes and 4 MiB of semantic value bytes. Its result preview selects at
most 256 canonical raw groups under 16,384 recursive value nodes, 4 MiB of
semantic payload and 8 MiB of rendered row JSON. These are disclosure limits,
not invented case-count limits: exact reducer counts may exceed the preview and
remain exact when their closure evidence permits it.

The hashes make accidental gaps and mutation detectable inside the trusted
owner-local directory; they do not authenticate a hostile owner or magically
prove that an evaluated case was correct. Exact certificates and fresh
publication replay remain the semantic checks.

The current executable v5 snapshot can derive the explicitly authorized case
DAG from that evidence; only mechanism-DAG publication remains deferred. Closed
facts and exact lower bounds only move forward, while graph reduction, node
numbering, provisional representatives and display order may change as more
evidence arrives. Once mechanism replay is implemented, a new replayed
signature can appear immediately as a confirmed mechanism lower bound, and the
scheduler can use that novelty to prioritize informative open regions without
pretending the mechanism inventory is exact before its target incidence closes.

This separation also gives a clean future map-reduce boundary without changing
the meaning of Explore. Map workers can own disjoint canonical CaseId-rank
shards and return immutable, run-identity-bound case regions plus replay
signatures. A reducer verifies disjoint coverage, joins the case regions into
the same ordered case DAG, and hash-conses equal signatures into one shared
mechanism DAG even when they were observed by different workers. Distribution
therefore changes where evidence is produced, not what counts as a case or a
mechanism. It remains a perspective beyond the first single-host 1.5-million
closure milestone; the current durable shard contract is designed not to block
it.

Mechanism counts are always scoped. A request names target `S_req`—canonical
representatives or all matching cases—and `T` is the subset actually traced.
Complete signatures partition `T`; closure additionally requires `T = S_req`.
Individual rule or branch atoms can overlap and their counts cannot be added.
The global typed input space can be infinite, so one mechanism may apply to
infinitely many possible inputs. This bounded query asks only for the finite
intersection with its declared world. The report may say
“35,491 matching configurations within these bounds share seven mechanism
signatures”; it may not turn that into a global cardinality claim.

You can also ask how many distinct mechanisms occur in each numeric loss bin.
For a replayed loss `H(x)` and traced signature fiber `fiber_T(sigma)`, the
count is:

```text
mechanisms_in_bin(B) =
    |{ sigma in Gamma_T |
        exists x in fiber_T(sigma), H(x) is in B }|
```

This is not a case histogram. It counts each query-relative dynamic signature
once in a bin, no matter how many of its cases land there. The same signature
may span several loss bins and is then counted in each, so the bin counts do
not add up to the global number of distinct signatures. A legal family label
is coarser still: one named provision can contain several dynamic signatures
because different dispatch, `min` arms or rounding paths were observed.

The bin count is exact only after the mechanism target, all signatures in it
and every required loss value have closed through replay or an explicit proof.
Before then, confirmed signature/bin witnesses form a lower bound; unresolved
signature or loss membership remains unknown. It is never silently counted as
zero.

If `M` or the canonical representative set is not yet closed, the mechanism
target itself is `scope_open`. Once the target is exact but some of it remains
untraced, incidence is `incidence_open`. This avoids pretending that an exact
“untraced remainder” is known before the population it belongs to is known.

If you want at most 100 cases per mechanism, decide whether 100 limits stored
examples or the precision of the support count. The useful default is:

```text
Mechanism signatures: 11 (closed)
Matching configurations: 35,491 (exact)
Concrete examples retained: 743
Example cap per mechanism: 100
Mechanisms whose examples were truncated: 7
```

The graph can count a huge finite region without serializing every case. If
counting itself is deliberately capped, each signature reports either an exact
number below 100 or `at least 100`. The number of such saturated signatures is
useful; it is exact over the requested target only when signature incidence is
closed, and otherwise a lower bound over observed signatures. It must not be
labeled “infinite”:
reaching a cap proves a lower bound, while infinity requires a separate proof.
Inside the current bounded feature every support is finite; global or symbolic
infinity belongs to a future unbounded contract.

A lower bound can instead mean unfinished search. If 37 cases are confirmed
and a solver cannot decide the remaining region, the count is `at least 37`
and the frontier stays open (`UNKNOWN`). A timeout pauses the run with
`PARTIAL` answer evidence and may materialize a snapshot when admitted; an
unsupported construct yields `UNSUPPORTED` and may seal only when no permitted
exact continuation remains. It is also possible for the case graph to close at
exactly 3,000 matches while mechanism tracing remains open: the total case
count is exact, but observed mechanism supports and even the number of
mechanisms are only lower bounds. Count certainty and layer closure must
therefore be read together.

Not every durable stop is automatically productive to resume. A time,
pressure or explicit milestone pause continues from the committed open
frontier. But if a particular whole case deterministically exhausts the step
or collection budget bound into this run's evaluator identity, retrying the
same run unchanged would stop at that same case forever. The invocation stop
must say that the rank is open and blocked under the current evaluator
contract; an admitted snapshot reflects the same evidence. Budget refinement
needs an explicit compatible protocol; otherwise it begins a differently
identified run.

Mechanism-guided symbolic search may use one replayed witness to prioritize the
next case region, but version one assigns that signature only to cases freshly
replayed. A future proof-backed mode can prove signature invariance over an
exact homogeneous region, add its weighted cardinality to a saturating counter
and retain only the requested examples. That proof kind must be explicit; one
guessed witness never silently stands for an unproved population.

Three limits must not be confused:

- `bounds` changes the world and therefore changes the proposition;
- an answer/case/value search budget leaves an explicit open region and makes
  the result `PARTIAL`;
- a mechanism-only tracing budget leaves mechanism evidence open without
  changing an otherwise complete result; and
- a display limit such as “show five cases per mechanism” changes only the
  view, not exact counts or completion.

The current exact implementation is case-first: classify the finite world,
then derive keys and representatives. Its first bounded terminal replay checks
only selected representative/extrema witnesses; it publishes no mechanism
incidence. The private mechanism-enabled experiment now adds canonical replay
for the deliberately constrained same-function, single-`if` profile and
commits its count-only checkpoint. Requested numeric `show` fields reuse their
canonical evaluated values and feed the durable half-open-bin incidence
relation; general dynamic mechanisms remain deferred. A
symbolic implementation may then alternate between finding one uncovered case,
replaying its mechanism, proving an exact case-classification region and
subtracting that region. In the first general mechanism-enabled version,
closure still replays every case in its target; a later proof-backed mode may
certify a shared signature for a whole region. Answer/case/value completion
requires no corresponding mechanism frontier.

The exploration can still be case-complete while mechanism evidence is only
representative-scoped or unavailable; those closure claims are reported
separately.

#### Honest DAG publication path

The implemented reducer retains three arrival-order-independent persistent
supports—excluded, admissible nonmatch and admissible match—and derives the
open support as their exact complement. Typed singleton and closed-region
events update those supports; recovery rebuilds them from the typed journal.
The reducer validates that the supports are pairwise disjoint, their counts
equal the scalar classification counts, and their union equals closed support.

For `--case-graph full`, a bounded mixed-radix rank-run lowerer turns those
supports into the existing canonical ordered decision DAG without enumerating
every case. It validates terminal multiplicities against the exact reducer
counts and gives every rank in the declared universe one terminal. Current
closed support ends at `excluded`, `admissible_nonmatch` or
`admissible_match`; the exact remainder ends at
`eligibility_open(search_budget_exhausted)`. That makes the snapshot graph
total over current evidence even while exploration closure is open.

Publication is all-or-nothing. Within the fixed lowerer and 8 MiB nested-JSON
limits, `graph.case_graph.status` is `included` and carries the complete graph,
its artifact hash, closures, polarity, terminal multiplicities and limits.
Otherwise status is `capacity_limited`, the graph and graph hash are absent,
and typed capacity evidence names what exceeded its maximum. Baseline runs keep
status `not_requested`. The request, retention authorization and schema limits
were bound before the run began, so publication cannot silently widen
disclosure or change limits during resume.

The mechanism DAG cannot be derived by the same shortcut. The private durable
path now has identity-bound trace authorization, typed signature/incidence
batches, an arrival-order-independent reducer, explicit untraced support and
count-only mechanism/bin checkpoints. Its executable producer remains limited
to one stable `if`: either directly in the paired function or inside exactly
one checked nested helper activation. The ordinary evaluator now carries
structural `ExprSiteId` context through that narrow function/`if` path, but not
through rule attempts, `match`, short circuits or general event graphs, and no
public mechanism DAG is assembled. Consequently, result groups remain separate
from mechanisms; the ordinary CLI still reports mechanisms as
`unavailable_deferred`, while the private stream may publish only the lower
bounds or exact counts its replayed incidence actually proves.

#### Implemented first nested mechanism slice

The first nested slice is implemented privately. The two paired shown
expressions resolve to the same checked top-level endpoint function, which
remains the implicit root. During each canonical endpoint evaluation that
function makes exactly one nested, direct, positional call to one checked
top-level helper, and the helper actually executes exactly one `if` decision
once. Only the helper contributes an activation-path frame. The trace contains
the `IfDecision` outcome produced by the canonical evaluation itself; neither
its condition nor either body is replayed separately.

The frozen-profile selector refuses source containing short-circuit Boolean
control, `match`, rule calls, recursion, named arguments, another nested
activation, repeated invocation of the event-bearing helper or multiple
dynamic-control events before a mechanism stream can be authorized. The
instrumented expression subset is deliberately explicit: variables, literals,
unit, non-short-circuit binary and unary operations, the two direct endpoint
calls, the one direct helper call, the selected `if`, and one-expression blocks
around those forms. Lists, tuples, fields, indexing, lambdas, pipes, effects,
extra applications and other expression forms cause plan refusal even in an
unreachable branch. Runtime checks remain defense in depth and cannot flatten
away an unsupported or integrity-failed event. `DynamicControlV1` retains the
complete supported executed control trace for each endpoint in this slice;
there is no relevance-pruning pass yet.

The evaluator revalidates and executes the artifact-owned immutable root syntax,
not a later-mutated caller buffer. Before a mechanism-enabled store is opened,
every checked declaration must belong to that root and the complete root AST is
recursively checked for plain, qualified or hash imports. Any external import
is refused until the runtime can retain immutable module nodes, origin
directories and occurrence-bound import edges. A flat imported statement list
is deliberately insufficient because it changes declaration-hoisting and
binding-initialization order.

Fresh initialization also attaches an interpreter-private capability to every
actual root top-level closure that can be matched unambiguously to its producer-
minted checked occurrence. Both the implicit endpoint and nested helper must
present that capability before the trace installs a checked body site. Missing
or unequal capabilities are hard replay failures and commit no mechanism
evidence. They are not permanent `observation_unsupported` cases. A resource
limit is different: it returns the same rank as operationally open, also
without a journal commit, so a later slice can retry it with a larger budget.
The older direct-`if` profile still reconstructs its checked condition so it
can retain its wider pure-expression subset, but it authenticates the actual
canonical show endpoint closure immediately beforehand. A shadowing value or
different same-named closure therefore fails before reconstructed evidence can
be minted.

That no-pruning rule deliberately postpones an identity problem rather than
hiding it. Repeated event-bearing calls currently need local invocation and
visit ordinals, but an earlier call reached at only one endpoint can shift a
later ordinal. Pruning first can create the same error by deleting the anchor
against which an ordinal was assigned. Before either repeated event-bearing
invocations or relevance pruning is enabled, the runtime contract must define
a checked call anchor and correspondence rule over the complete executed trace;
incompatible multiplicity must remain unpaired rather than be guessed.

Endpoint state is isolated: the lower and upper evaluations do not share a
trace stack or ordinal counters. Cached endpoint reuse is not mechanism
evidence unless the cache entry binds the same analysis program, mechanism
request, checked call anchor and canonical inputs and carries the complete
result/trace provenance. A cached value by itself is insufficient. This V1
slice traces confirmed matching cases only (`S_req = M`); representative and
nonmatching trace populations remain later profiles. These restrictions narrow
only the executable producer, not the general mechanism-DAG model above.

The nested producer is now joined to a bounded orchestration loop rather than
being exercised by a hand-written completion loop. After the checked probe
milestone, the scheduler chooses exactly one atomic work subject at a time:

1. if confirmed mechanism incidence is pending, admit
   `MechanismCaseIdRank(rank)` and fresh-replay that rank;
2. otherwise admit one ordinary `CaseIdRank(rank)` classification; and
3. when both frontiers are empty, publish the count-only mechanism checkpoint
   and pause at the still-explicit terminal-publication frontier.

The subject distinction is capability-bearing, not a label: authority to
classify rank 7 cannot be reused to mint mechanism evidence for rank 7. Each
new matching classification is therefore reflected into its mechanism graph
before the scheduler may expand the case frontier again. This bounds replay
backlog, makes novel signatures observable early, and preserves the same exact
resume rank when evaluation leaves a work unit open. An immutable V1 reducer
ceiling instead returns a typed `mechanism_limit`; it is not transient resource
pressure, and unchanged resume is not advertised as productive. Mechanism
checkpoint publication remains a separately admitted view phase; denial after
the probe milestone leaves a journal-only pause and a later invocation services
that view debt before advancing further. A pre-probe journal pause creates no
such debt because no mechanism checkpoint is defined at that cursor.

The executable four-income fixture yields three exact matching cases and three
exact signatures. Their endpoint outcomes are `Else/Else`, `Else/Then` and
`Then/Then`, each with support one. Every signature contains the same one-node
checked shape—one helper activation frame and its actual `IfDecision`—while the
outcome pair distinguishes the signatures. The experiment publishes the probe
checkpoint, commits the first classification, drops with its mechanism rank
pending, and resumes through the formal scheduler. The closing invocation
classifies the remaining three boundary configurations, traces all three
matching targets, and publishes exact three-case / three-signature counts with
no untraced remainder.
A second focused source-boundary experiment checks the same shape through a
plain-import helper and requires plan construction to fail with the frozen-
module-graph requirement. The rejection happens before a mechanism stream is
opened or evidence can be minted. Import-capable replay is intentionally held
for the frozen module graph rather than made dependent on whether a live file
happens to remain unchanged.

### Shown values and representatives

`show` controls what enters the result. Hidden solver assignments are not
printed automatically.

If a shown expression depends on variables omitted from the key, choose one
case explicitly:

```runa
representative first
representative maximize loss
representative minimize final_tax_øre
```

The choice is made separately within every key group. Equal objective values
use canonical domain order so repeated runs produce the same result.

### Grouped extrema: compare only when values differ

"Which municipality has the lowest tax, if municipalities differ?" is not a
tax-specific search primitive. It is a grouped result question. The `key`
names the conditioning facts, such as income and household profile, while
municipality remains a varied case dimension. A proposed generic result shape
is:

```runa
key [profile, income]
extrema [
    final_tax_øre = final_tax_øre(kommune, profile, income)
]
having varies(final_tax_øre)
representative minimize final_tax_øre
```

Each extrema measure is explicitly named and must type-check as `Int`; it is
not inferred from a shown field or from the representative objective. For each
raw key group `g`, let `C_g` be its matching configurations and let `H` be the
named measure. The closed summary contains:

```text
minimum(g) = min { H(x) | x in C_g }
maximum(g) = max { H(x) | x in C_g }
spread(g)  = maximum(g) - minimum(g)
minimum_tie_support(g) = |{ x in C_g | H(x) = minimum(g) }|
maximum_tie_support(g) = |{ x in C_g | H(x) = maximum(g) }|
```

`having varies(final_tax_øre)` retains exactly the groups whose spread is
positive. `representative minimize final_tax_øre` then selects a minimum case
inside each retained group, breaking equal minima by canonical domain order.
The deterministic representative does not hide ties: both tied-extremum
supports remain part of the group summary.

Keep the populations explicit. If `M` is the matching-case count, `G` is the
number of raw key groups before `having`, and `R` is the number of emitted
groups after it, then `R <= G`. Let `Q` be the number of matching cases in
qualifying groups and `S` the number in invariant, suppressed groups. Closed
aggregation must conserve the matching population:

```text
Q + S = M
```

Suppression is therefore not reclassification. A case in an invariant group
is still a match; its group merely does not answer the requested
"if there is a difference" result view. With an open case or value frontier,
an observed difference can confirm that a group varies, but a later value can
still change its extrema and tie supports. An apparently invariant group
cannot be suppressed conclusively until its population and measure values are
closed. Partial reports must keep those aggregates and `G`, `R`, `Q` and `S`
provisional or lower-bounded rather than present them as exact.

A compact permanent canary uses three municipalities and two profile keys. If
profile `P1` has taxes `[10,000, 10,000, 12,000]` øre and profile `P2` has
`[20,000, 20,000, 20,000]`, all six configurations remain matches. The raw
result has `M = 6` and `G = 2`. `P1` is emitted with minimum `10,000`, maximum
`12,000`, spread `2,000`, minimum-tie support two, maximum-tie support one and
the canonically first minimum municipality as representative. `P2` is
suppressed as invariant. Thus `R = 1`, `Q = 3`, `S = 3`, and `Q + S = M = 6`.

#### Empirical checkpoint: municipality-rate extrema

The executable 2026 query `laveste_kommuneskat_hvis_satserne_varierer`
completed over all 98 municipalities. Every municipality had parameters and
matched, while the single year key passed
`having varies(kommuneskat_basispoint)`:

```text
status = COMPLETE, coverage = ALL
U = D = M = 98
G = 1 raw group, R = 1 emitted group, suppressed groups = 0
Q = 98, S = 0
```

The exact minimum was 2,339 basis points, attained only by canonical witness
`[0]`; the exact maximum was 2,630 basis points, attained by 11 municipalities
with canonical witness `[32]`; and the spread was 291 basis points. The
deterministic minimum representative was Copenhagen at 2,339 basis points, and
the emitted key had exact support 98. Mechanism evidence was **unavailable**.

The control query `ingen_række_hvis_målingen_er_invariant` used the same 98
matching municipalities but measured the fixed year. It also completed with
`U = D = M = 98`, `G = 1` and coverage `ALL`, while producing `R = 0`, one
suppressed group, `Q = 0`, `S = 98` and no result row. This is executable
evidence that `having` changes only the group projection: the invariant cases
do not become nonmatches and case coverage remains `ALL`.

Together the positive and control runs demonstrate grouped extrema beyond tax
cliffs. This particular result compares the encoded 2026 municipal tax rates;
a query for the lowest total tax paid would use the same result algebra with a
different explicit integer measure.

This aggregation layer generalizes fixed loss-bin magnitude ranges into
data-derived per-group ranges. It operates over the closed case/value relation
and is independent of the optional mechanism DAG: mechanism evidence may be
unavailable while grouped extrema and their supports are exact.

### Use the result inside Futuruna

The specified typed form starts with the exact row to receive:

```runa
# SupportCliffRow(
    income_before: Int,
    income_after: Int,
    household: Household,
    available_before: Int,
    available_after: Int,
    loss: Int
)
```

Then change only the output header from the earlier query and append a
continuation:

```runa
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

after report -> publish_support_report(report)
```

The row fields are exactly `key` followed by `show`. Futuruna rejects an extra,
missing, reordered or differently typed field.

A consumer must handle the status explicitly:

```runa
> publish_support_report(
    report: ExplorationReport(SupportCliffRow)
) -> () {
    match report {
        | ExplorationComplete(_, findings) -> {
            @ print("Complete rows: " + show(findings))
        }
        | ExplorationPartial(_, confirmed, reason) -> {
            @ print(
                "Partial: " + show(length(confirmed)) +
                " confirmed rows; " + reason
            )
        }
        | ExplorationUnknown(_, confirmed, reason) -> {
            @ print(
                "Unknown: " + show(length(confirmed)) +
                " confirmed rows; " + reason
            )
        }
        | ExplorationUnsupported(_, diagnostic) -> {
            @ print("Unsupported: " + diagnostic)
        }
        | ExplorationError(_, diagnostics) -> {
            @ print("Exploration error: " + show(diagnostics))
        }
    }
}
```

A histogram writer can replace the first `print`, but with the income-only key
its typed `findings` population is one representative per income. A graph view
can instead aggregate every matching configuration in the authorized case
population. To receive those configurations as typed `findings`, make `K`
injective on the matching population by including every independently varied
case dimension. Partial or unknown rows remain visibly incomplete.

`report` is not a global variable. It exists only inside `after`, because the
solver result exists only during the selected `runa explore` command. To keep
the result after the process ends, use the CLI artifact or an explicit effect
inside the continuation. Ordinary `run` and `build` never launch this work.

The canonical report is finalized before `after` runs. Changing only the
continuation leaves `query_hash` unchanged, although the full `program_hash`
may change. A continuation failure leaves that report intact and becomes a
separate nonzero command outcome. In JSON mode the canonical document keeps
stdout to itself; continuation console output is isolated to stderr.

## 5. Read completion status before reading the count

A count is meaningful only together with its status and key.

| Status | What it lets you say |
|---|---|
| `COMPLETE` | Every answer/case/value population required by this report is closed |
| `PARTIAL` | Search stopped before required answer/case/value closure; shown findings are confirmed, but more may remain |
| `UNKNOWN` | The remaining symbolic question could not be decided |
| `UNSUPPORTED` | Exact analysis was unavailable on a required answer/case/value path |
| `ERROR` | Validation failed before a report, or solver and execution disagreed |

Coverage is a separate axis:

| Coverage | Exact meaning |
|---|---|
| `EMPTY` | No admissible case exists after bounds and constraints |
| `NONE` | Admissible cases exist, but none has the requested polarity |
| `SOME` | Some but not every admissible case matches |
| `ALL` | Every admissible case matches |
| `UNDETERMINED` | The required case population is not closed |

Only a sealed terminal result has a typed report; a routine time/resource pause
has an authoritative journal checkpoint, may also have an admitted observable
snapshot, and never invokes `after`. Terminal reports preserve the distinction:

| Status | Typed report payload |
|---|---|
| `COMPLETE` | `ExplorationComplete(..., findings)` |
| `PARTIAL` | `ExplorationPartial(..., confirmed, reason)` |
| `UNKNOWN` | `ExplorationUnknown(..., confirmed, reason)` |
| `UNSUPPORTED` | `ExplorationUnsupported(..., diagnostic)` |
| `ERROR` | `ExplorationError(..., diagnostics)` |

Only the complete variant calls its rows `findings`. That type-level
difference prevents a partial count from quietly becoming a final published
count. Unsupported and error reports expose no row list.

An invalid declaration fails before `report` exists and therefore never calls
`after`. The typed `ExplorationError` variant is reserved for a terminal
solving, decoding or replay error after the query has type-checked.

A partial report says:

```text
Different income steps found so far: 37
Completeness has not been established.
```

It never presents 37 as the final total.

`COMPLETE` requires every domain and operation reachable from required
answer/case/value roots to have exact semantics, every requested
answer/case/value population to close, every exposed row to replay, and the
remaining query to end in `UNSAT`, exact finite exhaustion or another recorded
exact closure method. Mechanism closure remains separate from case and key
closure.

## 6. See what the engine proves

For the synthetic query, the formal task is:

```text
Find every distinct income for which there exists an allowed household such
that next_step_never_hurts(household, income, 1) is false.
```

After finding income `99999`, Futuruna blocks that key—not merely the complete
`Couple` assignment—and asks whether another income remains. Final `UNSAT`
establishes that no unseen income key exists in the declared interval.

Each shown result is then run through ordinary Futuruna execution. A solver
answer that does not replay identically is rejected as an implementation error.

This is why `? explore` is more than shorter syntax for `range`, `map` and
`filter`: it owns projection, closure and replay.

If the query has `after`, Futuruna next constructs the appropriate
`ExplorationReport(Row)` from the already replayed and sorted public rows and
automatically delivers the continuation at most once per `run_id`. A durable
claim is written before delivery. A crash during an external effect therefore
leaves the outcome explicitly unknown and is not retried automatically; an
operator-requested retry carries the same `run_id` so the sink can be
idempotent. Nothing the continuation does can reopen or alter the solver
question.

## 7. Apply the pattern to Danish personal income tax

The Personskat exploration starts with one Boolean rule that calls the
canonical model at two adjacent gross incomes.

The helper constructing `PersonskatInput` must state its fixed facts plainly:

- tax year 2026;
- adult and single;
- 203 workdays;
- a standardized eligible commute distance;
- residence supplied by the dated 2026 residence-profile dataset, including
  each municipality and the separately qualifying small-island variants;
- no capital or share income;
- no pension, property-tax, spouse or foreign-contribution inputs;
- no carried tax position or special tax arrangement.

The residence profile derives the model's outer-municipality or qualifying-
small-island fact. It is not an independent Boolean that the explorer may pair
with an unrelated municipality.

`PersonskatResidenceProfile` is the typed model input for that pairing.
`personskat_residence_profiles_2026` is its dated, source-backed finite list and
must include both the ordinary municipality residences and the separately
qualifying island residences before the broad query can claim that scope.

The question is:

```runa
| personskat_next_krone_never_hurts(
    residence_profile: PersonskatResidenceProfile,
    pays_church_tax: Boolsk,
    daily_commute_km: Heltal,
    gross_income_kroner: Heltal,
    step_kroner: Heltal
) ->
    personskat_net_resources_øre(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner + step_kroner
    ) >= personskat_net_resources_øre(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner
    )
```

The broad query is:

```runa
? explore personskat_income_cliffs {
    over personskat_next_krone_never_hurts(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner,
        step_kroner
    )
    find violations

    bounds {
        residence_profile in personskat_residence_profiles_2026
        pays_church_tax in values(Boolsk)
        daily_commute_km in range(0, 201)
        gross_income_kroner in range(0, 1_000_001)
        step_kroner = 1

        where personskat_profile_is_supported(
            residence_profile,
            pays_church_tax,
            daily_commute_km
        )
    }

    boundaries on gross_income_kroner by step_kroner

    output {
        key [income_before_kroner = gross_income_kroner]
        show [
            income_after_kroner =
                gross_income_kroner + step_kroner,
            net_loss_øre = personskat_net_loss_øre(
                residence_profile,
                pays_church_tax,
                daily_commute_km,
                gross_income_kroner,
                step_kroner
            ),
            residence_profile,
            pays_church_tax,
            daily_commute_km
        ]
        representative maximize net_loss_øre
    }

}
```

The query above remains CLI-only. The specified typed form can feed its
replayed rows directly into a histogram pipeline by declaring the matching
product:

```runa
# PersonskatIncomeCliffRow(
    income_before_kroner: Heltal,
    income_after_kroner: Heltal,
    net_loss_øre: Heltal,
    residence_profile: PersonskatResidenceProfile,
    pays_church_tax: Boolsk,
    daily_commute_km: Heltal
)
```

Its output and continuation are:

```runa
output as PersonskatIncomeCliffRow {
    key [income_before_kroner = gross_income_kroner]
    show [
        income_after_kroner = gross_income_kroner + step_kroner,
        net_loss_øre = personskat_net_loss_øre(
            residence_profile,
            pays_church_tax,
            daily_commute_km,
            gross_income_kroner,
            step_kroner
        ),
        residence_profile,
        pays_church_tax,
        daily_commute_km
    ]
    representative maximize net_loss_øre
}

after report -> write_personskat_income_histogram(report)
```

With the income-only key above, the typed histogram contains one row per distinct
income boundary and plots that boundary's maximum modeled loss across the
declared profiles. It is not the distribution of every profile-boundary
observation. The function should publish this complete boundary distribution
only from `ExplorationComplete`. It may show confirmed partial rows for
diagnostics, but must label them as incomplete. A future explicitly authorized
case/value view can histogram every matching profile-boundary configuration
without changing the output key, provided admissibility, polarity and the
required per-case values are closed. Leaving the original `output { ... }`
unchanged keeps the query entirely CLI-driven.

There is no list of known income steps in this query. The dated residence list
defines the legal geography, church status covers every Boolean value, commute
and income have explicit ranges, and every other fact is fixed by the
documented profile builder.

With the key:

```runa
key [income_before_kroner = gross_income_kroner]
```

the result answers:

> At which different earnings levels does at least one permitted profile lose
> resources after earning the next krone?

It does not count the same income step once per municipality, church-tax state
or commute distance.

The closed case graph counts affected profile-step configurations separately
from projected income keys. A second query whose key also contains residence
profile, church-tax state and commute distance is required only when those
configurations must become projected or typed findings. That is a different
result identity, not a larger number of legal thresholds.

The expected known sequence from the encoded § 9 C phase-out contains 50 such
earnings steps, beginning with `342499 -> 342500` and ending with
`391499 -> 391500`. Those values belong in acceptance evidence, not in the
query's candidate domain.

### What the current scaling experiments establish

Three development-preview runs give us concrete calibration points:

| Empirical domain | Transitions | Matches `M` | Elapsed |
|---|---:|---:|---:|
| First § 9 C boundary, all 98 municipalities, four selected profiles each | 392 | 392 | 1,351.44 s |
| All 50 § 9 C boundaries, two municipalities, four selected profiles each | 400 | 400 | 477.67 s |
| All 50 § 9 C boundaries, all 98 municipalities, four selected profiles each, specialized exact path | 19,600 | 19,600 | 146.19 s |

The four profiles are the selected church-tax/commute combinations used by the
calibration query. These measurements say that every transition in those two
declared candidate domains was a cliff under the encoded model. They do not say
that those domains contain every cliff.

Scanning every one-kroner transition from zero through 3,000,000 DKK for 98
municipalities and four profiles would ask for about 1.176 billion transition
evaluations. The 50 source-derived § 9 C steps reduced that witness matrix to
19,600 transitions, and the specialized run completed it. The exact loss bins
for that declared matrix were 9,800 cases in 50.00–99.99 DKK, 770 in
100.00–149.99 DKK and 9,030 in 150.00–199.99 DKK. The reduction is dramatic,
but no proof yet establishes that those 50 income steps and the witness
distances 60/130 km cover every reachable Personskat event.

#### Empirical checkpoint: canonical native candidate matrix

On 2026-08-21, the focused native experiment
`personskat-income-cliffs-native-candidates.experiment.runa` evaluated the
canonical Personskat transition at all 50 source-derived § 9 C candidates for
København and Læsø, both church-tax states and the two calibration distances.
The prepared executable classified the 400 cases in 93.45 seconds of wall
time. Its raw ledger conserved the declared matrix exactly:

```text
400 declared candidate configurations
400 distinct canonical cliff observations
8 profiles × 50 boundary numbers, each boundary 1 through 50 exactly once
minimum loss: 6,923 øre
maximum loss: 17,002 øre
```

The exact case histogram for this declared matrix was:

| Loss bin in øre, `[from, to)` | Exact canonical cases |
|---|---:|
| `[5,000, 10,000)` | 200 |
| `[10,000, 15,000)` | 100 |
| `[15,000, 20,000)` | 100 |

The eight profile fibers each contained all 50 boundaries. Every 60-km case
fell in `[5,000, 10,000)`. The København 130-km cases fell in
`[10,000, 15,000)`, while the Læsø 130-km cases fell in
`[15,000, 20,000)`. This is exact classification of the authored candidate
matrix through the canonical tax calculation, not the specialized formula.

This checkpoint still establishes neither complement closure over the broad
income axis nor a dynamic mechanism count. The 50 locations are source-derived
scheduling evidence, and 60/130 km remain calibration profiles. The run does
show the intended optimized workflow: prepare the large canonical program once,
evaluate a small high-value candidate set cheaply, retain every matched
configuration, and leave the unproved complement visibly open for
quasi-affine certificates, SMT/CEGAR or residual exhaustion.

#### Empirical checkpoint: exact supports for branch/bin keys

On 2026-08-20, the exact-finite executor completed
`personskat_indkomstklint_specialiseret_mekanismehypoteser_i_50_dkk_bins` in
roughly 4.5 minutes of observed wall time. For this declared candidate matrix,
the report closed all four case/result populations:

```text
U = 19,600 declared configurations
D = 19,600 admissible configurations
M = 19,600 matching configurations
R = 3 projected branch/bin keys
```

The exact support of each projected key was:

| `beregningsgren_hypotese` | Loss bin in øre, `[from, to)` | Exact matching configurations |
|---|---:|---:|
| `lavindkomst-procentgren` | `[5,000, 10,000)` | 9,800 |
| `lavindkomst-maksimumsgren` | `[10,000, 15,000)` | 770 |
| `lavindkomst-maksimumsgren` | `[15,000, 20,000)` | 9,030 |

The supports conserve the closed matching population exactly:
`9,800 + 770 + 9,030 = 19,600 = M`. Because `U = D = M`, every admissible
configuration in this particular candidate matrix matched. The three rows are
therefore an exact case histogram under the declared projection, not three
mechanisms.

Mechanism evidence for this run was **unavailable**. The branch strings are
source-derived calculation hypotheses produced by the query, not signatures
from differential replay. That distinction is observable even inside the
60-km group: at the final phase-out endpoint both candidate supplement amounts
are zero, and the strict `procentbeløb < maksimum` test therefore selects the
`else`/maximum arm at the tie. The coarse label remains
`lavindkomst-procentgren`, so it cannot certify that every case in that row has
one dynamic branch signature.

This checkpoint is steering input, not a general proof or v1 mechanism
closure. It validates the reusable result shape—exact case multiplicity under
an arbitrary typed key, with mechanism evidence reported on a separate
closure axis. The bins and branch hypotheses belong to this query, not to the
executor. The same substrate should later support other projections and
objectives, such as returning the lowest-tax municipality only when the closed
municipality results actually differ, without adding tax-specific search
logic.

#### Empirical checkpoint: the first natural distance ramp

The canonical
`personskat_indkomstklint_to_ankre_afstand_32_første_grænse` run completed in
roughly 15 minutes. It declared two anchor municipalities, both church-tax
states, the first source-derived income event and the natural distance range
`0..<32`; it did not seed the search with 60 or 130 km. The closed report was:

```text
U = 128, D = 128, M = 28, R = 7, coverage = SOME
```

Distances `0..24` produced no match. Each emitted distance from 25 through 31
had exact support four, accounting for both municipalities times both
church-tax states. The canonically first representative was Copenhagen without
church tax:

| Daily distance (km) | Exact support | Representative net loss (øre) |
|---:|---:|---:|
| 25 | 4 | 211 |
| 26 | 4 | 374 |
| 27 | 4 | 562 |
| 28 | 4 | 772 |
| 29 | 4 | 959 |
| 30 | 4 | 1,169 |
| 31 | 4 | 1,356 |

The projected supports conserve the matching population: `7 * 4 = 28 = M`;
the other 100 admissible configurations are exact nonmatches. This organically
discovers the 24/25-km floor for this bounded first-event experiment. It does
not yet prove that floor for every income event or municipality. Mechanism
evidence was **unavailable**, so the result establishes case and value closure,
not a mechanism signature.

A control query then derived the canonical `PersonskatIndkomstovergang` once
per configuration and reused it in the question and outputs. It reproduced the
same closed counts, keys, supports and values in roughly 13.5 minutes rather
than roughly 15 minutes. That modest improvement is evidence that top-level
query duplication is not the dominant cost. The next substantial speedup must
come from the general execution/search substrate—pure-call reuse or partial
evaluation, compiled case evaluation, and candidate/certificate closure—not
from hand-specializing each Explore query.

#### Keep throughput, discovery and closure separate

A static accounting of the canonical 128-case query explains what the control
removed. This is a call-shape diagnosis, not a profiler measurement:

- the 128 question evaluations each construct one transition;
- the first representative for each of seven projected keys evaluates three
  transition-backed shown fields, adding 21 transitions; and
- the seven fresh replays each reconstruct the question transition and the
  three shown-field transitions, adding 28 transitions.

The original shape therefore invokes the transition 177 times and the full
Personskat calculation twice per transition, for 354 calls. The derived-fact
control has the static shape of 128 enumeration transitions plus seven replay
transitions, or 135 transitions. It removes genuine duplicate work, but still
performs a full lower/upper calculation for every configuration it classifies.

The general optimization architecture has three orthogonal axes:

1. **Singleton throughput** makes one exact case cheaper but does not close any
   unevaluated case. Prepare the dependency-closed interpreter program once,
   including dispatch groups keyed by scope, name and arity, tier order,
   argument permutation, head-match plans, local cleanup, resolved field and
   constructor identities and stable trace occurrences. Fresh enumeration and
   replay runtimes may share those immutable plans while retaining isolated
   mutable environments. Reuse pure calls only after proving their complete
   caller/global dependency set; Futuruna rule bodies may observe caller
   bindings, so arguments alone are not a sound cache key. For adjacent
   boundary cases, a bounded endpoint cache can reuse the proven result for
   `F(profile, x + step)` as the next `F(profile, x)` when every non-boundary
   dependency agrees. A checked native residual evaluator can then execute the
   same prepared slice in a tight finite loop and return compact canonical
   observations. It cannot simply use ordinary Rust code generation: Explore
   requires checked integer behavior, effect isolation, resource-guard
   reporting and deterministic case identities. The interpreter remains the
   fresh replay oracle until native provenance has equivalent evidence.
2. **Candidate discovery** changes which open singleton is inspected first.
   Source guards, dispatch changes, finite-table boundaries, division,
   remainder, rounding, `min`, `max` and constructor tests can produce guarded
   event candidates. Probes can find witnesses quickly, but evaluating a
   candidate closes only that point. A source label is a mechanism hypothesis,
   not a replay-derived mechanism signature, and an exhausted candidate list
   does not close its complement.
3. **Proof closure** avoids singleton evaluation. For the three-million-kroner
   income axis, the principal path is to normalize the reachable finite
   difference into guarded quasi-affine form—affine terms, constant division
   and remainder, finite lookups, conditionals and `min`/`max`—then partition
   intervals by guards and congruence classes. Exact sign bounds can certify a
   whole cell as matching or nonmatching; exact value bounds and cardinalities
   can populate case counts and loss bins without materializing every case.
   Hash-bound, disjoint certificates must cover the declared support and lower
   into the ordered case DAG. Case/value closure still does not imply mechanism
   closure; the latter needs replayed or equivalently proven trace incidence.

SMT/CEGAR belongs after quasi-affine normalization, over the residual cells.
Every satisfiable model is replayed and used to refine its cell. An
unsatisfiable result closes a region only where the lowering is complete and
semantically equivalent; unsupported residuals remain open or fall back to a
bounded checked singleton evaluator. Whole-model SMT and tax-specific formula
copies are not the core search strategy.

The candidate scheduler has now crossed that implementation guard at the
source-review level. Its mutable per-profile frontier is an indexed interval
partition; singleton and certified-region refinements are transactional and
logarithmic in the current cell count; candidate and fallback cursors advance
monotonically; and checked cost counters are maintained incrementally. Full
plan reconstruction and validation happen only at an explicit audit/export
boundary. This removes the earlier quadratic bookkeeping path, but it does not
make a large singleton residual desirable: the residual still costs one full
semantic classification per case and retains point evidence. The scheduler is
therefore ready to be connected to the executor, while quasi-affine
certificates remain the necessary route to broad closure.

#### Native residual throughput calibration

A deliberately small compiled control scanned 1,000 adjacent gross-income
transitions for one Copenhagen/no-church/60-km profile through the canonical
`personskat_indkomstklint_overgang` helper. Generating and compiling the full
Personskat artifact took 212.46 seconds and produced 256,889 lines of Rust; the
resulting binary classified the 1,000 transitions in 41.07 seconds. The scan
found no cliffs in that low-income prefix, as expected.

This is one throughput sample, not a stable benchmark and not Explore result
evidence. Its useful conclusion is nevertheless decisive for planning: at the
observed roughly 24 transitions per second, a serial three-million-transition
scan would project to about 34 hours before replay or mechanism tracing.
Endpoint reuse and parallel chunks could reduce that constant, but they would
not turn brute force into the preferred algorithm. Native evaluation remains a
valuable residual backend after proofs have made the residual small; it is not
a substitute for semilinear closure. The experiment also exposed a separate
code-generation ownership defect for a non-Copy loop-invariant value, now
tracked independently; the probe worked around it by constructing the profile
inside a helper.

#### Resource incident and revised execution discipline

An attempted broad oracle run later launched five native residual workers on a
six-core, 8-GiB machine. While those workers were resident, an optimized
compilation of the 256-thousand-line generated Personskat program was started
to test a faster evaluator. The machine exhausted practical headroom and
rebooted. The exact causal chain is not proven without a crash diagnostic, but
overlapping the memory-heavy LLVM phase with five evaluators was an avoidable
resource error. Because the chunk logs and experimental JSON were under
`/tmp`, the reboot also discarded that uncommitted evidence. The checked-in
fixtures survived; no lost temporary row is treated as result evidence.
After reboot, swap activity eventually stabilized but system indexing still
kept load well above the six available cores; the oracle remained paused rather
than treating recovered memory alone as permission to restart.

The corrected discipline is part of the normal observable-run design rather
than an ad hoc recovery mode. The current slice implements the supervised
single child and durable commit boundary; dynamic worker scaling and semantic
shards below remain the accepted multi-worker design. The same journal that
supplies the current bounded case snapshots—and, in a later slice, authorized
mechanism observations—is the journal that makes pause, pressure yielding and
crash recovery honest:

1. Code generation/compilation and case evaluation are mutually exclusive
   heavy phases by default.
2. A run begins with one representative worker and records its elapsed time and
   peak residency. It may add only one worker at a time, with explicit CPU and
   physical-memory reserves; a requested job count is a ceiling.
3. Rising memory pressure or swap activity prevents new launches and causes a
   fast scale-down. A pressure stop checkpoints an honest open frontier rather
   than pushing toward a nominal utilization target.
4. Only the still-open residual support is split into short, canonical,
   disjoint CaseId-rank shards; proof-closed regions are never expanded back
   into singleton work for checkpointing. Each
   completed shard is validated and atomically committed as an immutable
   content-addressed chunk, so a crash loses at most the short shards still in
   flight and a resumed run cannot hide a gap. Lease attempts are their own
   small immutable records. The journal directory is the authoritative entry
   set; it does not rewrite or re-hash a growing all-chunk manifest on every
   transition. A disposable compact index may speed recovery, while the
   canonical manifest and final artifact are streamed once after complete
   validated coverage.
5. Private checkpoints use an explicit durable path outside the checkout, not
   `/tmp`. Compiler artifacts and final Explore artifacts remain separate.
6. The unified journal, including probe singletons, is an explicit trusted
   owner-local cache, hash-bound to the whole evaluator and query contract.
   Stale, overlapping, conflicting or non-private chunks fail closed;
   published representatives are freshly replayed. An environment that cannot
   trust that local boundary must discard or fully replay the journal.

The optimization objective is therefore lexicographic: first minimize the
number of semantic evaluations through source events, certificates and solver
closure; then maximize residual throughput inside the measured safe resource
envelope. Percent CPU utilization is not the objective. Avoiding 1,499,950
unnecessary evaluations is a stronger optimization than keeping every core
busy performing them.

Within the residual envelope, the planned multi-worker controller is
deliberately asymmetric. It uses additive increase: one new worker only after
the previous target is fully resident, two more shards have committed, and the
same CPU/memory/swap epoch
has stayed normal for at least 30 seconds. It uses immediate decrease: warning
pressure revokes capacity, while critical pressure, unknown required telemetry
or a new swap-out stops dispatch and drains to zero. This is an AIMD-like
search for useful host capacity, but safety signals win immediately instead of
waiting for another shard. A long healthy run can approach its measured safe
ceiling; a busy interactive machine automatically yields capacity.

The accepted resource policy budgets installed CPU and physical RAM
independently beneath an 80-percent operational ceiling. The remaining 20
percent is host reserve, with a 1-GiB absolute memory floor; a future
whole-worker scheduler rounds CPU admission down. Thus that scheduler would
admit at most four one-core evaluators on this six-core host, while measured
memory, live pressure or other applications could lower the target all the way
to zero. The current executable slice remains one-worker. The 80-percent
figures are admission ceilings, not utilization targets or kernel-enforced
guarantees against momentary CPU/RSS overshoot, and not permission to ignore
swap growth or warning pressure.

Pause does not bypass that envelope. The small append-only journal transition
is the resume checkpoint; the potentially much larger snapshot and case DAG
form a separate materialized-view work subject with 256 MiB of accounted
working set. It receives one normal admission opportunity after semantic work
stops. If the deadline or current host sample denies it, Futuruna preserves the
20-percent/1-GiB reserve, appends the pause directly and reports
`JournalOnlyCheckpoint`. The next resumed invocation retries that observation
work before semantic dispatch and pauses at `snapshot_catch_up`; it is not lost
evidence and is not permission to relabel the graph as capacity-limited. If an
admitted attempt reports capacity, it instead publishes the bounded
`snapshot_unavailable` observer receipt, satisfying that cursor's observation
boundary without claiming that a later attempt can never fit.

The first executable slice is deliberately one worker, but it still runs in a
fresh macOS process group. Other platforms currently reject the durable path
rather than run without that supervisor. A parent sampler holds no writer
fence; the child alone advances run state. Their private pipe carries monitor
heartbeats, so parent death, suspension or a blocked telemetry helper stops
the child instead
of leaving an unsupervised exploration behind. The parent trips below the
upper bound, freezes and kills the group, reaps its leader and verifies that no
group member remains before reporting containment. RSS/pressure sampling is a
safety circuit breaker, not a kernel memory quota, and the CPU decision is a
conservative interval rather than an instantaneous scheduler cap. The fresh
child tracks live Rust allocation requests from process start and, after
liveness validation, rejects growth beyond a reserved heap budget. Thread
stacks, allocator overhead, direct FFI or mapped memory and subprocesses remain
outside that hard counter. The honest
claim is bounded requested Rust growth, reduced crash risk and durable
recovery—not mathematically impossible RSS or momentary CPU overshoot.

The accounting is reservation based. For memory, the sampler adds the exact
aggregate RSS of the coordinator-owned resident workers back to current
available memory, subtracts the host reserve, and divides the remainder by the
calibrated safety-charged worker peak. It does not add resident process count to
raw memory slots. CPU applies the analogous reconciliation using measured
worker CPU and live CPU capacity. If either attribution is unavailable, the
run may retain or reduce already safe work but cannot use that absence to scale
up. Each target change, compile phase and sampler restart advances a typed
generation so a delayed zero-worker or pre-compile sample cannot unlock later
work.

The run-state filesystem is governed independently from worker admission. A
million retained configurations can exhaust disk even when evaluation RSS is
flat, so a broad run needs a free-space reserve, a measured conservative
bytes-per-case/chunk projection, and space for one streamed final assembly.
Crossing that boundary yields a durable partial result with an open residual;
it never fills the host volume in pursuit of a complete status.

On this host a future multi-worker calibration therefore starts at one worker.
Although an 80-percent CPU ceiling rounds down to four one-core evaluators, it
does not exceed two evaluators until measured residency and several
pressure-stable shards justify successive one-worker increases. This is an
operational starting policy, not a semantic constant or a claim that two
workers are universally optimal.

The initial calibration shard is 256 cases. After measuring throughput, the
coordinator fixes the run's residual shard size to the next power of two near
30 seconds of work, clamped to 256–4,096 cases. At the observed roughly 24
cases/second this selects 1,024 cases, or about 40 seconds: short enough for
bounded crash loss without turning checkpoint overhead into the workload. Two
committed pressure-stable shards and at least 30 seconds without new swap-outs
are required before adding exactly one worker. CPU reserve caps this machine at
four evaluators even if memory remains green; measured memory may cap it lower.

Most importantly, the 1.5-million-income milestone remains a proof-closure
task. Resource-safe parallelism is for the small residual and for an optional
independent oracle; it does not rehabilitate exhaustive scanning as the primary
algorithm.

The first proof-closure milestone is therefore one fixed profile through
1,500,000 DKK, not a half-sized brute-force job. Its zero-evaluation cost plan
has `U = 1,500,001` declared incomes and 1,500,000 eligible adjacent pairs. A
100,000-case singleton cap would still leave at least 1,400,001 declared
assignments open. The candidate/certificate backend must instead close this
same declared bound by weighted regions, using native evaluation only for the
small residual. Once that plan closes, extending the upper bound to 3,000,000
should add source events and proof cells rather than another 1.5 million
mandatory evaluations.

The first execution is deliberately capped at **one** singleton
classification. Source preparation and interval certification still analyze
the whole declared axis, while the runtime may inspect at most one residual
case. Its search evidence distinguishes the outcomes immediately:

- a large `certified_region_closed_cases` count plus a small open residual
  demonstrates that the broad run has become a proof-closure problem;
- source candidates with little certified closure show that candidate
  discovery works but complement proof is still owed; and
- canonical search evidence shows that the optional proof path was unavailable
  and prevents an accidental 100,000-case fallback experiment.

Only after inspecting that one-case report is the case cap raised to cover the
measured residual. This is an experimental probe of the real execution path,
not a separate hand-authored oracle and not a claim of completion.

Do not run this broad query through the one-shot compatibility executor. The
first 1.5-million-axis measurement starts only after a pre-probe `RunOpened`
header, later proof/residual coverage plans, distinct journal and canonical
evidence roots, and resumable typed refinement chunks are wired. That makes the
first output part of the durable stream instead of a disposable calibration
whose work must be repeated. A one-case slice remains useful, but it is the
first committed stream slice—not a separate pre-architecture run.

The distance-ramp search deliberately removes the 60/130 km restriction. With
203 workdays, the encoded commute formula yields a non-monotone distance
partition:

- 0–24 km: no commute supplement and no § 9 C cliff;
- 25–282 km: every commune/church/boundary profile in the current slice cliffs;
- 283 km: conditional—the first boundary becomes neutral for outer
  municipalities while the other cases still cliff;
- after 659 km no outer-municipality cliffs remain, and after 1,367 km no
  ordinary-municipality cliffs remain.

Those endpoints came from exact source formulas and are inputs to certificate
checking, not replacement bounds authored into the final query. The final
query declares the broad valid axis; midpoint probes only choose which open
cell to inspect next, and only a certificate, SMT closure or singleton
exhaustion closes a cell.

The executable experiment ladder is
`examples/danish-income-tax/personskat-income-cliffs-distance-ramp.explore.runa`.
It uses natural power-of-two distance ramps rather than the discovered
breakpoints: first two communes over 0..<256 km, then all communes over that
same ramp, then all 50 source-derived income events over 0..<256, 0..<512 and
0..<2048 km. Their declared case counts are respectively 1,024, 50,176,
2,508,800, 5,017,600 and 20,070,400. Under the default 100,000-case
classification cap, the first two can close by finite exhaustion and the last
three must retain an explicit open suffix until region certificates or a
higher cap close them.

Keep the two evidence kinds separate:

- **empirical:** the three rows above are completed executions of the stated
  finite domains;
- **source-derived:** the 50 § 9 C steps are event candidates justified by the
  encoded rule structure;
- **still owed for a full result:** extract all reachable event supports, prove
  nonnegative finite differences between them (including rounding congruences),
  close any SMT/CEGAR or singleton residual, and replay the matches under the
  same program hash.

The current source evidence supports one coarse legal family, the § 9 C
phase-out, and the observed values suggest two possible dynamic signatures
corresponding to different `min` arms. Neither statement is yet an exact
mechanism-per-loss-bin count. That requires provenance replay to close the
query-relative signatures, their target incidence and the loss value for every
case in scope. Until then, the family is a source label and the two signatures
are hypotheses, not a completed mechanism histogram.

Until the last obligation closes, the honest result is a successful calibration
and a promising search plan, not a complete Personskat cliff inventory.

A full Personskat search may discover additional earnings steps caused by
other encoded rules or exact rounding. Such results are discoveries to replay,
classify and explain. They must not be discarded merely because the known
§ 9 C reference run contains 50.

The public result therefore separates:

- distinct earnings steps;
- matching profile-step configurations;
- the greatest replayed loss and its representative profile for each step;
- exact case regions and graph-derived loss distributions when admissibility,
  polarity and the requested value relation are closed and publication was
  explicitly authorized;
- changed rule branches in representative replay by default; and
- distinct mechanism signatures and their exact case incidence only when the
  matching mechanism scope is fully replayed.

A representative trace can explain a result, but it is not automatically a
complete inventory of every mechanism among the hidden profiles sharing that
income key. Mechanism signatures are a separate quotient of the same matching
case population. They cannot be inferred from the income-step count, and their
count is exact only when the report says that matching-case mechanism incidence
is closed.

## 8. Build the feature in executable checkpoints

### Checkpoint 1: syntax and types

- Parse, format and type-check the synthetic query.
- Preserve every existing `?` proof form, including an invariant named
  `explore`.
- Preserve existing CLI-only `output { ... }` while adding the optional
  `output as Row` and `after report ->` forms.
- Diagnose missing, duplicate and out-of-order clauses.

### Checkpoint 2: domains

- Support explicit lists and pure named finite collections.
- Preserve end-exclusive `range` semantics without materializing large ranges.
- Enumerate `values(Type)` only when every inhabitant is provably finite.
- Diagnose the exact unbounded field in rejected types.
- Reject unbounded relevant inputs and cyclic derived values.

### Checkpoint 2a: durable run and probe scheduling

- Parse and type-check a finite deterministic `ProbePlan` without changing the
  exploration's `query_hash` or declared case universe.
- Open one run-bound journal whose genesis commits the program, query, domain,
  full CaseId universe, report request, evaluator and probe-plan identities—but
  not a post-proof residual or shard width; keep `--run-state` distinct from
  final `--output`.
- Commit proof coverage, exact residual support and each sharding epoch in a
  later `CoveragePlanAccepted` record whose transition conserves the prior
  frontier exactly.
- Append immutable content-addressed evidence chunks and advance a hash-bound
  operational journal head while maintaining a separate arrival-order-
  independent evidence root over closed support and the exact open frontier.
- Derive observable snapshots and case/mechanism DAG views from committed
  cursors without treating provisional discoveries as closed evidence.
- Give the declared probe plan initial priority, fall through into exact
  refinement in the same run by default, and support an explicit
  `--pause-after probes` inspection point.
- Commit probe matches, nonmatches and exclusions as ordinary singleton
  evidence with their observed `CaseId`, authorized named configuration,
  actual boundary endpoints, classification, outputs, scheduling reason and
  adaptive transcript.
- Exercise cross-profile axis-value lifting while proving that generated cases
  stay `unevaluated` and inherit no evidence from their observed origin.
- Fail closed on stale identities, corrupt chains, overlapping chunks or replay
  disagreement while preserving the last valid head.
- Keep `probe_plan_complete` separate from answer closure and prove that
  candidate or probe exhaustion never closes the complement.

### Checkpoint 3: answer-set semantics

- Find the synthetic failing step without receiving `99999`.
- Return one result for `key [income]`.
- Return two results for `key [household, income]`.
- Close and distinguish `D`, `M` and `R`.
- Build a reduced ordered case DAG whose expansion reproduces the complete
  finite classification and whose path multiplicities conserve the counts.
- Preserve separate eligibility-open and admissible/polarity-open frontiers
  before closure.
- For supported boundary expressions, build a program-hash-bound
  `BoundaryPlan`, certify semilinear interval/congruence cells, refine the
  residual with SMT/CEGAR and fall back to exact singleton evaluation.
- Prove that retained-example and display caps leave weighted counts unchanged,
  while a classification cap retains an explicit open support.
- Return a complete empty result for a property holding throughout its bounds.

### Checkpoint 4: representatives and replay

- Select the larger `Couple` loss deterministically.
- Preserve deterministic ties.
- Replay every key, shown value and objective through normal execution.
- Treat any disagreement as an error.

### Checkpoint 5: mechanism reflection

- Assign stable analysis-program, declaration, rule-candidate and decision
  identities, plus an explicit mechanism observation identity.
- Trace through a fresh replay interpreter, not through enumeration or solver
  internals.
- Pair the synthetic before/after computations and intern their differential
  mechanism signatures. The `Single` and `Couple` signatures are distinct
  because they select different `match` arms, while sharing the threshold
  change subgraph.
- Show separately that disconnected case paths may share one complete
  signature without becoming a Cartesian product.
- Keep representative and all-matching mechanism closure separate from case
  completion.

### Checkpoint 6: honest interruption

- Pause through time and test resource limits, retain closed regions plus the
  exact open frontier, and resume without reclassifying accepted support.
- Distinguish lifecycle `paused` from answer-evidence status `partial`; admit
  snapshot materialization separately and keep a journal-only pause resumable;
  never construct a typed terminal report or execute `after` at a routine
  pause.
- Recover the last committed journal head and evidence root after a simulated
  unclean stop and ignore uncommitted worker proposals.
- Seal `Completed` only over an empty required frontier and a canonical final
  artifact hash; reject later semantic events.
- Prove that raising an operational budget refines only open regions and that
  contradictory evidence fails instead of retracting a closed fact.
- Prove that a display or per-mechanism example limit does not change status.
- Exercise solver `UNKNOWN`, unsupported lowering and invalid-query statuses.

### Checkpoint 7: result contracts

- Add `runa explore FILE [--query NAME]`.
- Add versioned event and snapshot envelopes plus read-only follow mode; keep
  ordinary `--json` a single document rather than implicit JSONL.
- Add exact pause/resume/time-slice exit behavior without constructing a typed
  `ExplorationReport` or executing `after` at pause.
- Keep invocation-v1 stable across admitted full snapshots, bounded
  `snapshot_unavailable` receipts, and typed `journal_checkpoint` artifacts;
  the latter has `snapshot.status = deferred` and no canonical payload or blob.
- Add deterministic human output.
- Add sealed versioned `futuruna.explore.v1` JSON with a `Completed` seal.
- Keep case graph, mechanism graph and their incidence referentially sound.
- Derive counts, coverage, partitions and histograms without rerunning queries.
- Publish case-level artifacts only through an explicit, recorded
  `ReportRequest`; keep baseline output privacy-safe.
- Exclude timing, absolute paths, raw SMT models and hidden inputs from the
  canonical result.

### Checkpoint 8: typed result continuation

- Validate one declared concrete row product against key plus show.
- Construct rows only after representative replay and canonical sorting.
- Expose complete, partial, unknown, unsupported and error as distinct sum
  variants.
- Deliver only the selected root continuation, at most once automatically per
  `run_id`, in a fresh environment; make crash ambiguity and explicit
  idempotent retry visible.
- Keep continuation code out of the query hash and solver IR.
- Preserve the canonical artifact on continuation failure.
- Keep JSON stdout isolated from continuation effects.
- Prove that hidden assignments are unavailable to post-processing.

### Checkpoint 9: Personskat

- Lower a narrow fixed-profile Personskat query.
- Rediscover the known § 9 C sequence without threshold candidates in source.
- Replay the established first loss through `beregn_personskat`.
- Run the broad declared query and accept its discovered total rather than
  forcing the known sequence to be the entire answer.
- Extend the income axis through 1,500,000 DKK using a certified boundary plan;
  do not promote the 50 § 9 C candidates to a full result until their
  complement and every residual region are closed.
- Group by earnings step while preserving every profile-step case region.
- Report exact loss distributions and mechanism signatures with their named
  populations and closure scopes.

### Checkpoint 10: permanent confidence

- Add parser, formatter, type, diagnostic, solver, projection and replay tests.
- Add a small solver-backed exploration canary.
- Add differential cases for graph expansion, canonical reduction, key
  blocking, deterministic representatives and exact-versus-SMT graph parity.
- Add Personskat conformance evidence.
- Run the compiler semantic-change gates required by `CONTRIBUTING.md`.

## 9. Report a legal-model exploration responsibly

A complete exploration is complete for:

- the checked-in model revision;
- the declared domains and fixed facts;
- the supported reachable semantics;
- the output key named by the query.

It does not establish that the model contains every relevant legal rule, that
the source interpretation is correct, or that the bounded profiles represent
the population.

The useful public claim has four parts:

1. the exact question;
2. the exact declared world;
3. the meaning of one result key;
4. the completion and replay status.

That is enough for Futuruna to make an unusually strong statement without
making a larger one than the evidence supports.

An `after` continuation is an explicit publication sink, not additional
evidence. It is not passed hidden profiles or assignments, but its ordinary
program code may compute from the public rows and visible declarations. Such a
computation is new logic, not recovered solver provenance, and it cannot turn a
partial report into a complete one. Review its effects with the same care as
any exporter, especially for private legal or tax data.
