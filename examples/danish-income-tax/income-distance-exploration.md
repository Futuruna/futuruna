# Income cliffs across income and commuting distance

The full-domain query declaration is
[`personskat-income-distance-unit.explore.runa`](personskat-income-distance-unit.explore.runa).
It uses the canonical Personskat calculation, not a simplified tax formula.
It passes frontend checking and now has a regression proving endpoint totality
across the full declared ranges, including mechanisms. The full-model query
now executes governed epochs and writes a durable classified prefix. Exact
full-grid closure remains unfinished. Explore remains **Experimental**.

The latest original-grid checkpoint accounts for **105,422,482 candidates
(65.5611%)**, with **55,377,920 still unclassified**. The latest productive
window added 38,993,920 candidates through checked regional proofs; see
[the next salary frontier](#continuation-to-the-next-salary-frontier).

The first **all-distance boundary window is now closed exactly**: 176 income
cliffs, zero commute-increase losses, 2,230 harmless transitions and six
outward-boundary exclusions. Its largest annual loss is **144.09 DKK**. See
[the completed boundary output](#completed-all-distance-boundary-output).
That six-salary window does not close the full income range.

For a quick working example without the full tax corpus, run
[`relational-explore-income-distance.runa`](../relational-explore-income-distance.runa)
with `--query income_distance_demo`. Its explicitly synthetic one-point
allowance produces two cliffs—one in each direction—and lets the engine prove
544 of its 800 edges harmless while evaluating the remaining 256 concretely.
It is a search-engine demonstration, not a tax approximation.

## The question

For the explicitly conditioned 2026 wage-earner profile, does either of these
single changes reduce annual salary minus final tax?

- Increase annual salary by **1 DKK**, holding commuting distance fixed.
- Increase daily round-trip commuting distance by **1 km**, holding salary fixed.

The endpoint bounds are **0–400,000 DKK** and **0–200 km**, inclusive. The
distance cap is an operational starting choice, not a legal threshold. These
are distance changes, not commute-time changes. Transport costs and the value
of travel time are not included. The score stays in integer øre, so a loss
smaller than a krone is still a finding.

Existing syntax already expresses the exact product:

```runa
vary bruttoløn_kroner in range(0, 400001)
vary afstand_km in range(0, 201)
vary retning in range(0, 2)
```

The intervention is `(income_delta, distance_delta) = (1 - retning, retning)`.
It never changes both dimensions at once. The query rejects successors outside
the endpoint bounds and retains canonical model validity at both endpoints.
When changing a maximum, update both its end-exclusive source bound and its
inclusive successor-admission bound. Fixed facts and units are documented in
the query; they are not a representative sample of all taxpayers.

| Population | Exact size before model-validity exclusions |
|---|---:|
| Raw directed candidates | 160,800,402 |
| Outward-pointing boundary edges | 400,202 |
| In-bounds salary edges | 80,400,000 |
| In-bounds distance edges | 80,000,200 |
| All in-bounds edges | 160,400,200 |

The two finding summaries group by intervention. A salary cliff and a
distance-induced loss must not become an indistinguishable aggregate.

## What can safely close a route?

A repeated execution mechanism is a useful explanation and scheduling hint.
It is **not** proof that unseen values cannot contain a cliff: arithmetic,
rounding, eligibility or another rule can change the outcome inside the same
apparent path.

The implementation separates three operations:

1. Checked source events can propagate an income or distance coordinate through
   typed record fields and callable arguments. Each usable boundary nominates
   adjacent **slabs** in the independent product, using the exact binding-to-factor
   correspondence. It does not classify those slabs.
2. For supported pure-function classification graphs, a bounded affine/interval
   evaluator can prove that an entire canonical rank chunk is admitted and has
   no selected cases. It conservatively encloses that chunk in a source-coordinate
   box, retaining income/distance correlations in arithmetic. The proof must hold
   for the whole box, but its weight is only the exact original rank count.
3. Anything unproved remains exact concrete work. An uncertain branch, nonlinear
   product, unsupported dispatch, possible overflow, or lossy rounding bound does
   not become a no-cliff conclusion. Positive findings still receive concrete
   case identities and mechanism replay.

Regional certificates bind the checked query, classification graph, source
image, canonical chunk and admission/question identities. Journal replay
recomputes the theorem; a stored digest alone cannot authorize pruning.
Discovery order does not change a closed answer's evidence roots.

## Resource-aware first attempt

The development machine has 8 GiB RAM and six CPU cores. Keep the governed
Explore supervisor enabled and use one build job. Do not launch a full-grid
exhaustion as an unattended performance experiment yet.

The old native measurement of roughly 120–135 cases/second would put even
classification alone for this product around **14–16 days** if every edge
needed concrete work. That is a rough extrapolation from the earlier workload,
not a benchmark or completion estimate for this new query. Preparation,
retained evidence, replay and publication also cost time and memory.

The execution uses a bounded two-level page layout:

- Advisory slab nomination stops after 16,384 attempts. Unnominated chunks
  remain in the exact residual schedule.
- A canonical directory contains at most 4,096 pages. Page width starts at 256
  ranks and doubles until that directory fits, with a hard ceiling of 65,536
  unit transitions per page. The declared grid therefore has **2,454 pages**,
  not 628,127 retained fine-grained descriptors. The directory is eager but
  bounded; concrete slices within a page are generated on demand.
- Concrete classification slices and native batches remain at most **256**
  transitions. Each slice is durably resumable. Equal harmless/rejected
  outcomes coalesce across the page; selected runs are additionally split at
  page-relative 256-rank boundaries so finding materialization stays bounded.
  Those splits are independent of operational slice size and pause timing.
- The physical journal frame is bounded to 8 MiB and its segment to 16 MiB.
  A maximal 65,536-run alternating single-question page must fit even without
  compression; the codec regression checks that worst case. This changes small
  physical buffer limits, not the host-wide CPU/RAM governor. Larger question
  vectors still have to satisfy the codec's byte limit.

This two-level accelerator covers roots up to 268,435,456 raw candidates;
larger roots conservatively retain the ordinary fallback. It is not an
arbitrary-depth lazy partition tree. Page width is derived from population and
fixed implementation bounds, never from tax thresholds or observed outcomes.

The affine evaluator also caps axes, evaluation work, call depth and retained
value trees. These are optimization limits, not permission to omit cases.

Use governed epochs to check source/case cardinality evidence, concrete edges
and pause/reopen behavior. Cold native preparation can exceed a short epoch;
the initial failed attempts are recorded below. A successful preparation proof
does not itself produce tax findings:

```sh
./target/release/runa explore \
  examples/danish-income-tax/personskat-income-distance-unit.explore.runa \
  --query personskat_income_distance_unit_2026 \
  --run-state /private/your-explore-unit.run \
  --output /private/your-explore-unit.result \
  --time-limit 3m --json
```

Use distinct writable private directories. Repeat the same command to resume;
changing the query requires fresh run state. A paused empty finding set is not
an exact-empty answer. Check each count's status and all admission exclusions.

For the next *useful cliff search*, measure the new compact page path on the
canonical model, then finish proof lowering for the actual rule families. Let checked
boundaries prioritize unit-resolution neighborhoods in **both** dimensions,
and discharge the rest with regional proofs or concrete evaluation. Coarse
scans can inform the order but cannot replace the unit-edge coverage obligation.

## Delivered boundary and remaining work

The current product prover handles constructors, projections, typed source and
context bindings, acyclic pure calls, checked affine arithmetic, supported
integer division and decisive Boolean branches. Checked root-scoped, typed,
acyclic rule families and strict pure local blocks now enter that graph too.
Scoped dispatch, exhaustive constructor matches without a wildcard, complex
rule heads and collection/rounding behavior in the canonical Personskat graph
still exceed that proof fragment. Such unsupported optimizations must leave
concrete residuals. The original endpoint-totality refusal has now been removed by
supporting bounded `flat_map` summaries; a permanent test proves both endpoint
roles over the unchanged full canonical query. The first real-model epoch has
now classified cases, but this does **not** establish practical full-grid closure.

The outstanding work is tracked explicitly:

- `td-f699c8`: bounded finite-list callback proofs for canonical endpoint
  preparation; implemented and verified through real execution and cold resume.
- `td-7ba30c`: bounded two-level page partitions and native batches; implemented,
  with focused tests and full-size synthetic closure passing; the real-model
  page measurement is recorded below.
- `td-966941`: checked rule-dispatch and required collection/rounding proof
  lowering, followed by measured canonical Personskat regional closure.

The broader Explore feature is not finished by this slice. The finish line is
an exact, resumable unit-grid answer with every exclusion accounted for,
replay-derived mechanisms for findings, and measured operation within the host
resource policy—not merely a small number of discovered mechanisms.

This changes Experimental optimization behavior and artifacts, not core source
syntax or tax semantics. Page-partition schema is now 4, classified-page schema
4, slice schema 3, scheduler policy 5 and journal schema 30 / codec 25; regional
proof schema remains 4. Codec-24 and earlier journals are rejected explicitly.
Keep their artifacts for historical evidence and start fresh state in distinct
directories. There is no in-place migration: the historical prefix below is not
silently imported into a new page-based answer. Cold resume is verified within
the new format.

## Verification

Permanent tests compare both intervention directions against an independently
enumerated Cartesian oracle, including boundary rejections, isolated one-unit
losses and integer-rounding cliffs. A uniform affine grid closes all 800 cases
using four regional certificates rather than point classification. Candidate
and canonical schedules preserve exact finding identities and evidence roots;
product prefixes are encoded, cold-replayed and resumed. Geometry tests cover
equal-size axes, nonzero starts, intersected slabs, nomination caps and the
full-grid bounded page directory. Forged coordinate-kind certificates cannot
replay even after their structural hashes are recomputed.

### Initial foundation checks recorded on 2026-09-06

| Command / check | Observed result |
|---|---|
| `cargo fmt --all --check` and `git diff --check` | Passed. |
| `cargo test --lib --jobs 1 -- income_distance product_ checked_explore_source_event relational_region_proof --test-threads=1` | 31 passed, including the authored demo and cold replay. |
| `cargo build --release --bin runa --jobs 1` | Passed; existing unused-method warnings in the CLI. |
| `runa fmt --check` for the new real query, shared adapter, retained 350k query and synthetic demo | Passed. |
| `runa check examples/relational-explore-income-distance.runa` | Passed, including generated Rust. |
| `runa check --frontend` for the real unit query and retained 350k query | Passed (30.4s and 21.5s respectively); this does not validate generated Rust. |
| `runa check examples/danish-income-tax/personskat-income-distance-unit.explore.runa` | Failed in generated Rust: 7,340 errors, chiefly missing canonical total/miss-safe `RuleDispatch` contracts. Not a passing model/backend gate. |
| Full Personskat `runa explore` command above, 3-minute limit | Refused before searching in 17.49s; peak resident memory 1,259,012,096 bytes (about 1.17 GiB). Endpoint-totality proof could not establish a finite-list callback result. No tax cases classified. |
| `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 ./scripts/mint.sh` | Stopped at its Rust test stage: 691 passed, 17 failed. Every failure was reproduced on untouched base commit `7bb4bc37`; later mint stages were not reached. |
| `CARGO_BUILD_JOBS=1 ./scripts/canary.sh core` | Formatting passed; compiled execution passed 10/12. The compact and multiline newline fixtures failed generated-Rust compilation; subsequent codegen/roundtrip stages were not reached. |
| `runa verify tests/verify_test.runa` | All five invariants proved by Z3. This is the existing verifier regression, not proof of Danish tax outcomes. |

Here `runa` denotes `./target/release/runa`. The differential lane was not run:
core parsing, type inference, lowering, ownership and codegen were not changed.
Baseline gate failures are recorded under `td-ce0146`; they were not bypassed
or weakened to make this feature appear green.

The synthetic CLI invocation used `--query income_distance_demo`, distinct
private run-state/output directories, `--time-limit 3m --json`, and the existing
resource governor. It completed at journal sequence **156** with 800 exact
admitted cases, zero rejected cases, two selected cases, two closed structural
mechanisms and zero unavailable explanations. Each loss is 400 øre: `(10, 3)`
to `(11, 3)` for salary, and `(10, 3)` to `(10, 4)` for distance. The unit/oracle
test verifies that three certificates cover 544 cases and the remaining chunk
is evaluated concretely; the published case-support graph independently agrees.
A cold reopen with the final release binary appended zero batches/events and
retained sequence 156, the exact journal head and all counts.

An earlier 45-second invocation on the busy build host was CPU-paced for
42.716 seconds and paused at sequence zero. It supplied no semantic evidence.
That pause is a resource-control result, not a finding of no cliffs.

The real query's preparation refusal identifies the canonical declaration
`personskat_søbl4_påkrævede_kilder` (arity 5), AST path `[1, 2, 0]`:
`flat_map callback must return an exact finite List`. No run-state or output
directory was created. This is a proof-support limitation, not a finding that
the tax function is partial or that cliffs do not exist.

A temporary finding-only variant retained the identical source, admission
validity and finding predicate while omitting the optional mechanism consumer.
It avoided that immediate refusal, but cold native preparation exceeded its
one-minute limit plus the 30-second outer grace. The supervisor stopped it at
91.43s; observed process-group resident memory was 2,202,058,752 bytes, below
the 5,905,580,032-byte guard. It also classified no tax cases and created no
run state. This diagnostic variant is not shipped as a working alternative;
it only shows that separating findings from explanations deserves investigation.

A final three-minute retry on the idle build host reached native classifier
compilation but also ended at the outer deadline, after 211.24s. It created no
run-state/output directory and classified no tax cases. Observed process-group
resident memory at containment was 1,237,843,968 bytes; the host still had
1,615,183,872 bytes available, above its 1-GiB reserve. Extending that epoch
therefore did not establish a working shortcut. Preparation throughput and
cache reuse need a measured checkpoint of their own before promising useful
short real-model runs.

### Endpoint-proof continuation on 2026-09-06

The `flat_map` proof now accepts exact or summarized finite input lists and
callback results. Exact input concatenations add result-length bounds; summary
inputs multiply input and callback bounds. Exact positional values stay exact
until a variable-length result requires a summary. Empty inputs skip callbacks,
possible empty outputs do not justify `head`, and possible callback errors
still refuse totality. The existing 4,096-item proof boundaries apply to both
input and output; length arithmetic and retained abstract values remain bounded.
This extends Experimental proof acceptance without changing runtime list
semantics, tax source, query domains, admission, or certificate encoding.

Checks (using the existing shared Cargo target cache and one build job):

- `cargo test --lib --jobs 1 -- flat_map_ personskat_unit_income_distance_endpoint_totality --test-threads=1`
  passed all four tests in 231.00s in the debug build. The large test loads the
  actual full-domain Personskat query and validates its mechanism certificate
  and analysis-plan authorization, not a reduced tax formula.
- After final input/output-limit guards and cold-runtime coverage,
  `cargo test --lib --jobs 1 -- flat_map_ --test-threads=1` passed four tests in
  0.24s. These exercise positive and adversarial list summaries, ordering,
  nonemptiness, overflow/capacity refusal, and independently reconstructed
  interpreter mechanism traces for every endpoint of a small fixture.
- `cargo test --lib --jobs 1 -- endpoint_totality flat_map_ --skip personskat_ --test-threads=1`
  passed 56 tests in 1.49s, including certificate/observation binding, codec,
  cold replay, arithmetic, effects, recursion and resource boundaries. The
  large canonical regression above is separate, not silently omitted.
- `cargo build --release --bin runa --jobs 1` passed in 5m37s. The rebuilt
  `runa verify tests/verify_test.runa` proved all five existing invariants.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 ./scripts/mint.sh` ran all 714 library
  tests: **697 passed, 17 failed**, in 598.18s after compilation. Both canonical
  Personskat endpoint regressions passed. The failure names are identical to
  the 17 already reproduced on untouched baseline `7bb4bc37`; later mint stages
  were not reached. This is still a red required gate, tracked by `td-ce0146`.
- `cargo fmt --all --check`, `git diff --check`, and `runa fmt --check` on the
  full-domain query passed. The differential lane was not rerun because this
  continuation changes proof acceptance, not parsing, type inference, lowering,
  ownership or generated-code semantics. Focused proof/replay tests and the
  existing Z3 verifier check cover the directly affected deeper lane.
- The published synthetic 800-edge run reopened under the rebuilt binary with
  `--time-limit 45s --json`: 798 not selected, two selected, all closures exact.
  It retained sequence 156, two journal segments, and journal head
  `fe011ba2b7af79237f1099a291ffb9fa2efe01c9abb74d5aab7ceaf4546356fc`,
  without appending semantic events. Codec-24 state remained readable at that
  endpoint-proof revision (`0ce37766`); the subsequent paging revision requires
  fresh codec-25 state, as explained above.

These are endpoint-proof and replay results, not classified tax cases or an
exact answer for the full income/commuting grid.

### First canonical full-domain execution

Using the rebuilt compiler, the actual query ran with mechanisms enabled,
unchanged validity checks, private run-state/output paths, `--time-limit 10m
--json`, and `FUTURUNA_EXPLORE_TRACE=1` for phase diagnosis. It exited normally
after 601.89s, paused at the runtime limit. Preparation took 77.824s; native
classifier construction then took 197.272s and installed a compiled evaluator
from 9,276 checked declarations. `/usr/bin/time -l` reported maximum resident
set size 2,249,539,584 bytes; the supervisor retained its existing limits and
recorded three CPU pauses totaling 3.380s.

The engine proved the declared source/case population exactly **160,800,402**.
The paused prefix classified **12,198** candidates: **12,168** admitted and
not selected, plus **30** rejected. Selected count is a **lower bound of zero**,
not an exact-empty answer. Relation, finding and analysis closure remain open.
Its durable journal had 70 segments, next sequence 287,957, and head
`2aa53f28940e7f2959f97d05085581953c5d5522b8512d376baa75af18809593`.

This prefix retained about **174 MiB** of run state. At that revision, the
eager-partition cap sent this large grid through point-level records and one-subject
native calls instead of compact classified sweeps. That measured representation
cannot scale to the whole grid within the available disk. `td-7ba30c` must
restore compact batching and bounded regional partitions before attempting
long exhaustion. This was a preparation/durability checkpoint, not a clean
throughput benchmark: verbose tracing was enabled and the required compiler
test gate began during its final execution portion, after memory usage fell.

A cold reopen of the same query and directories, without verbose tracing and
with `--time-limit 3m --json`, also exited normally after 181.06s. It recovered
the durable prefix and advanced to sequence **311,009**, head
`52e4c32d2e5f4d531846b1b731e003befe7883d2e83f80776bc8114f608a6560`.
Classified candidates grew to **13,187**: **13,154** admitted/not-selected and
**33** rejected. These are still lower bounds within the unchanged exact
160,800,402-candidate universe; no finding or closure is inferred from zero
selected cases so far. The resumed state has 77 segments and occupies about
188 MiB. Reported maximum RSS was 1,392,541,696 bytes; CPU pacing paused once
for 1.124s. Further point-only exhaustion is intentionally deferred while the
compact full-grid partition path is implemented.

### Bounded page continuation on 2026-09-06

The two-level layout above replaces the fine-descriptor cutoff. It retains a
bounded directory, not an arbitrary-depth lazy tree. A page proof must still
hold throughout its exact region, and unsupported pages remain concrete work.
Selected runs have canonical 256-coordinate cuts, so a long selected page
cannot overflow the existing bounded finding-materialization path.

Using the shared target cache and one build job:

- `cargo test --lib --jobs 1 -- paged_ canonical_page_directory maximum_alternating_page full_income_distance_grid income_distance product_ relational_region_proof --skip personskat_ --test-threads=1`
  passed **24 tests** in 10.32s. Coverage includes full-grid page geometry,
  thresholds/capacity/overflow, repaired-hash child forgery, a 65,536-run
  alternating codec payload, a 512-point cold resume at point 17, uniform
  selection across differently sized slices, and the existing independent
  small-product oracles. Selected materializations retain all 512 distinct
  cases in two bounded runs. An earlier lane also passed the actual canonical
  endpoint regression; its sole malformed test-source newline was corrected
  and retested, not a compiler failure.
- The already-built library test binary ran
  `relational_public::regional_stream_acceptance_tests relational_journal_codec relational_durable_journal --test-threads=1`:
  **35 passed, one failed** in 15.66s. That failure is the same baseline plural
  publication test recorded above.
- `cargo build --release --bin runa --jobs 1` passed in 5m39s. The rebuilt
  `runa verify tests/verify_test.runa` proved 5/5 invariants. Rust formatting and
  `git diff --check` passed.
- `CARGO_BUILD_JOBS=1 nice -n 15 ./scripts/canary.sh core` passed formatting for
  12 fixtures and compiled execution for 10/12. The same two baseline newline
  fixtures failed generated-Rust compilation; later canary stages were not
  reached.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 nice -n 15 ./scripts/mint.sh` stopped
  at its library test stage: **702 passed, 17 failed** in 880.55s. The failure
  set is unchanged from the independently reproduced baseline; later mint
  stages were not reached. The required gate remains red under `td-ce0146`.
  No gate or host-wide reserve was disabled. The differential lane was not
  rerun because this change affects proof/partition/replay behavior, not
  parsing, type inference, lowering, ownership or generated-code semantics.

A full-size **synthetic affine** CLI fixture used the same 160,800,402 raw-rank
product, with all successors admitted deliberately. It is an engine check,
not Personskat and not the canonical endpoint-exclusion policy. The first two
attempts stopped at sequence zero with `resource_reserve_backoff`; independent
host samples showed CPU idle as low as 6–14 percent during macOS indexing and
storage-management activity. Those attempts produced no classification evidence.

A same-state retry with the ordinary governor and `nice -n 15`, `--time-limit
3m --json`, completed in **29.56s**. All **160,800,402** candidates were exactly
admitted and not selected, with zero selected and rejected; all closures were
exact. The case/support seal independently reports **2,454** classified pages
and **2,454** certified regions, zero authorized individual case records, and
exact logical coverage 160,800,402. The run retained about **6.6 MiB** of state
and 4.9 MiB of output. Reported maximum RSS was **95,076,352 bytes**. This was
a governed correctness/resource checkpoint on a busy host with the low-priority
test gate also active, not a clean throughput benchmark.

Cold reopen with `--time-limit 45s --json` completed in **5.13s**, appended zero
semantic events, and preserved sequence **22,112**, one segment, and head
`1a5c61294d64a92557989f90f3671e8aa0fef65276121e6b9eeec9bb555ddd70`.
Reported maximum RSS was 80,412,672 bytes; CPU pacing paused once for 1.128s.
This proves that a full-size exact answer can be compact and replayable when
the classification theorem is supported. Canonical tax-model closure remains
the separate requirement below.

Most real-query pages contain some outward-pointing successors, so admission
is mixed even before tax validity is considered. The existing regional prover
requires uniformly admitted, not-selected regions. Canonical scaling therefore
needs proof-derived subregions or exact mixed-admission accounting as well as
rule-dispatch/collection/rounding support (`td-966941`). Merely recognizing a
repeated tax mechanism cannot discharge those obligations.

The first actual Personskat invocation with fresh codec-25 state and a
10-minute limit exited normally after **493.38s**, paused for
`resource_reserve_backoff` at sequence zero. It recorded 37 CPU pauses totaling
55.197s and maximum RSS 1,429,585,920 bytes. That attempt produced no semantic
classification evidence.

A same-state three-minute retry exited normally after **173.85s** at its
runtime limit. It proved the exact **160,800,402** source/case population and
appended 12 semantic batches / 31 events. The checkpoint had one segment, head
`8602163e98fabd53ecd9b1316b04cf9c69cd68cf399a87f7beba020c02328142`,
and about **284 KiB** of state. Maximum RSS was 1,403,699,200 bytes; two CPU
pauses totaled 2.249s. No classified page was complete, so every published
classification count remained a lower bound of zero and all closures remained
open. A pending concrete slice is resumable evidence, but does not count as a
completed page in the published totals. This is not a new exact-empty tax answer.

A further cold resume, with a ten-minute limit and phase/slice tracing,
recovered that checkpoint and evaluated **8,307 new unit transitions** in
63 concrete slices, with a measured maximum of **256** transitions per slice.
It appended 126 events, then exited normally after **316.57s** for
`resource_reserve_backoff`. The checkpoint advanced to sequence **157**, head
`0e4ff1a0c92d1ce11d7f6c30f3be7ac3602eb117c88ea0013fddfe77e5833d11`,
18 segments and about **392 KiB** of retained state. Preparation took 109.395s;
the cached native evaluator was reused in 227ms. Maximum RSS was
1,382,547,456 bytes; 13 CPU pauses totaled 21.287s. The slice count comes from
successful append traces, not a completed-page report. The first 65,536-point
page remains unfinished and published classification counts remain lower bounds
of zero. This is concrete evidence of bounded real-model batching and compact
partial retention, not proof that the full tax grid is cliff-free.

Cold reopen of that larger pending prefix, with a four-minute limit, recovered
the same 18 segments, sequence 157 and journal head without appending events.
It exited normally after 111.49s for `resource_reserve_backoff`; preparation
took 94.177s and maximum RSS was 1,373,880,320 bytes. Two CPU pauses totaled
3.379s. This verifies recovery of the real partial-page checkpoint, but the
resource pause supplies no additional classified cases. The next implementation
step is canonical regional proof support, not an unattended point-only sweep.

## Checked rule and local-block bridge

The next bounded implementation (`td-f461df`, within the still-open
`td-966941`) translates checked root-scoped rule families to the existing
acyclic `Call`/`If` graph. It preserves exception, conditional-default, clause
and unconditional-default order. A false Boolean clause tries the next
candidate; a false exception or default returns immediately. A missing numeric
fallback is not invented. Recursive families, open captures, scoped dispatch,
unresolved parameter types and complex head patterns remain concrete residuals.
The supported heads are exact scalar literals, wildcards and checked variable
binders, with compatible checked type annotations.

Pure local blocks use lexical binder identities, including shadowing. Every
initializer and preceding expression is retained as an eagerly evaluated call
argument. This matters for correctness: discarding an unused division or
overflowing addition could otherwise certify a region in which the actual
program fails. Effects, mutation and unsupported binding patterns remain
residual. This extends Experimental proof acceptance, not the meaning of
ordinary rules or tax calculations. It needs no new query syntax or graph-node
encoding. The checked graph/capsule identities bind the changed lowering;
use fresh run state when a compiler change alters those identities, rather
than assuming that old certificates are portable.

Six focused permanent tests compare checked-graph execution and regional
closure with independent exhaustive interpretation. They cover an 800-edge
affine product closed by four certificates, two isolated one-unit losses,
false-clause backtracking versus false exceptions/defaults, alpha-renaming and
local shadowing, and unused division-by-zero/overflow that must prevent closure.
The unchanged canonical-query regression also checks that its unit successor
enters the graph. Neither test population substitutes for real tax-grid closure.

A **rule-only bridge** release, before adding local-block lowering, ran the
unchanged full query with fresh state and `--time-limit 10m --json`, tracing,
the ordinary governor and `nice -n 15`. It stopped normally at the runtime
limit after **602.35s**, retaining **18,470** concrete unit transitions in
**107** pending slices. The checkpoint has sequence **243**, **30** segments,
head `bc6cc19fa9e6b64071761f3c8cda1c88d6f2e251158b1bc22da554f3bf98f6ea`,
and about **464 KiB** of state / 84 KiB of output. No classified page finished;
published classification counts remain lower-bound zero, and every closure is
open. No real-model harmless-region certificate was produced.

Preparation took 76.192s and native compilation 218.019s. Measured slice phases
spent **227.072278s** classifying versus **2.669715s** constructing transitions:
**98.838%** of those two phases was classification. Reported maximum RSS was
2,353,594,368 bytes; three host-CPU pauses totaled 3.376s. This was a busy-host
diagnostic with some low-priority test overlap, not a clean throughput benchmark.
A one-second native-child profile confirmed real canonical computation and
substantial copying/allocation: 223 of 751 top-of-stack samples were in
`memmove`, alongside allocator, clone and destructor costs. The host was waiting
for that native child, not repeatedly interpreting the tax model. Reducing
repeated result construction is therefore a useful complementary performance
route to investigate; the sample alone proves no achievable speedup.

The main remaining requirement is unchanged: replayable proofs for actual
canonical negative regions, exact mixed-admission partitioning, and exact
residual evaluation around every unresolved branch and rounding boundary.
Recognizing a mechanism, or making concrete evaluation faster, is not evidence
that an unvisited region is harmless.

The final **rule-plus-local-block** release was also run on the unchanged full
query with fresh state and a ten-minute limit. It paused normally after
601.37s, with **1,980** evaluated transitions in **37** pending slices,
sequence **103**, five segments, head
`6cc2b0afcbf2b6adaeecd635e9bb9df24475b1410411acf790fe11df6897d400`,
and about 324 KiB of state / 84 KiB of output. Preparation took **238.786s**,
native compilation 293.942s, and 65 CPU pauses totaled 97.842s. Maximum reported
RSS was 2,215,575,552 bytes. Classification took 36.936729s and materialization
0.364229s. These timings include the loaded host and other low-priority checks;
the extended proof fragment is not a demonstrated canonical-model speedup.
Its first surfaced canonical FIND residual is the exhaustive spouse-constructor
match in `personskat_aktieavance_parresultat`: the existing match normalizer
still requires an irrefutable last arm. Published classification counts remain
lower-bound zero, with no finished page or closed tax-region certificate.

Cold resume of that final-build checkpoint, with a five-minute limit, recovered
sequence 103 and evaluated **12,032 new transitions** in 79 slices. It paused
normally after **301.30s**, at sequence **261**, 23 segments, head
`f79a1024ffdf50965ca420993109245d5725a5f0a65336cca7fda2497d98ec0e`,
with about **444 KiB** of state / 84 KiB of output. This checkpoint retains
**14,012** evaluated transitions across the two epochs, still inside its first
unfinished page. Preparation took 121.518s, native-cache reuse 270ms, and one
CPU pause 1.125s; maximum RSS was 1,506,082,816 bytes. The new slices spent
130.598166s classifying and 1.519851s materializing. All published classification
counts remain lower bounds and all closures remain open. This is successful
real-model cold recovery and continuation, not an exhaustive cliff answer.

A one-second-budget cold-open attempt on the earlier rule-only checkpoint was
stopped by outer containment at 32.38s, before recovery. No uncommitted evidence
was accepted, but this is **not** a successful cold-replay test. Preparation
needs its own realistic time allowance on this model.

Checks for this bridge (shared Cargo cache, one build job/test thread):

- `cargo test --lib --jobs 1 -- checked_rule_dispatch_ --test-threads=1`:
  six passed in 4.96s. The canonical
  `personskat_unit_income_distance_endpoint_totality_certifies_without_execution`
  test passed separately in 386.16s. Endpoint totality proves evaluability,
  not admission validity for all generated tax inputs.
- `cargo build --release --bin runa --jobs 1`: passed in 8m50s;
  `cargo fmt --all --check`, `git diff --check`, and canonical query
  `runa fmt --check` passed. `runa verify tests/verify_test.runa` proved 5/5.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 nice -n 15 ./scripts/mint.sh`:
  **708 passed, 17 failed** in 918.91s in the library lane. The failure set
  matches the independently reproduced baseline recorded above; later mint
  stages were not reached. The six new tests also passed in this full run.
- `CARGO_BUILD_JOBS=1 nice -n 15 ./scripts/canary.sh core`: formatting 12/12,
  compiled execution 10/12, with the same two newline code-generation failures.
- `RUNA_BIN=./target/release/runa CARGO_BUILD_JOBS=1 nice -n 15 ./scripts/differential.sh`:
  ordinary corpus roundtrip 5/5 with no skips, then imported execution 3/4.
  `import_mesh_consumer.runa` fails generated Rust with `Plan` versus
  `Policy::Plan` at two calls. Running
  `cargo run --quiet --bin runa --jobs 1 -- check tests/differential/corpus/imports/import_mesh_consumer.runa`
  on untouched base `7bb4bc37` independently reproduced both errors.
  Later differential stress and generated-import stages were not reached.

The required gates remain red under `td-ce0146`; this is a scoped Experimental
proof extension submitted for review, not a claim that Explore is fully shipped
or that the broad Personskat audit is complete. No stable source/runtime
semantics changed, so no stable compatibility-guide entry is required.

## Checked exhaustive-match bridge

The next proof extension (`td-1a07b6`, under the still-open `td-966941`)
recognizes exhaustive matches over Boolean values and closed, monomorphic
declared ADTs. It reconciles every declared variant with the exact checked
constructor owner, ordinal, layout and fields; a catalogue missing a constructor
is not treated as a smaller universe. Unguarded whole-variant patterns establish
coverage. Earlier guards retain first-match order, but a guarded case alone does
not cover that variant. Named field projections are supported; partial matches,
refutable or nested field patterns, conditional type evolution, type composition
and unsupported schemas remain residual work.

This also fixes a proof-safety gap in existing wildcard matches: the matched
expression must still execute even when the result ignores it. Matches now share
the strict call-argument sequence used for local blocks, preserving unused
division-by-zero and overflow failures. No tax rule, public query syntax or
graph-node encoding changes. Changed graph/capsule identities still require
compatible replay state; old certificates are not silently upgraded.

Five permanent tests cover exact constructor metadata, Boolean/record matches,
guard order, lexical alpha-renaming, missing or refutable coverage, and strict
evaluation in both one-axis and product searches. Independent exhaustive
interpretation and cold journal replay agree with the accelerated runs: the
800-edge affine fixture closes with four certificates, while its isolated bonus
removal produces exactly two witnesses, one along each intervention axis.
Alternating constructors exercise both guarded and fallback graph execution.
The mechanism observer in this fixture is intentionally simple: endpoint-totality
proof has a separate, narrower match fragment and does not yet accept the
exhaustive Boolean helper used by the FIND expression. Classification support
does not imply broader observer-totality support.

The unchanged full-unit Personskat endpoint/graph regression passed in
**457.36s** (debug, alongside low-priority builds). The trace progressed beyond
the spouse match into `personskat_aktieavance_uden_par37_til40`, whose
`filter(input.særlige_aktiver, ...)` is the next surfaced `DynamicDispatch`
residual. This propagates through the pair calculation to the final observation;
the complete FIND lane is still residual. The trace identifies the next barrier,
not a completed regional certificate or a runtime speedup. Bounded collection
reasoning, particularly propagating this query's explicitly empty asset inputs,
is the next candidate extension (`td-a744af`). The observer-totality Boolean-match
difference is separately tracked as `td-4e1cde`.

These are synthetic compiler/proof results, not a full Personskat answer.
Canonical negative-region closure and mixed-admission partitioning remain
requirements of the parent task.

Checks for the exhaustive-match extension used the shared Cargo target, one
build job/test thread and `nice -n 15`:

- `cargo test --lib --jobs 1 -- checked_exhaustive_matches_ --test-threads=1`:
  five passed in 3.27s.
- `cargo test --lib --jobs 1 -- checked_rule_dispatch_ --test-threads=1`:
  six passed in 3.72s.
- The built library test binary, with `FUTURUNA_EXPLORE_TRACE=1`, ran
  `personskat_unit_income_distance_endpoint_totality_certifies_without_execution --test-threads=1 --nocapture`:
  one passed in 457.36s as described above.
- `cargo build --release --bin runa --jobs 1`: passed in 9m39s.
  `cargo fmt --all --check`, `git diff --check`, and
  `runa fmt --check examples/danish-income-tax/personskat-income-distance-unit.explore.runa`
  passed. `runa verify tests/verify_test.runa` proved 5/5.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 nice -n 15 ./scripts/mint.sh`:
  **713 passed, 17 failed** in 766.99s in the library lane, including all five
  new tests passing. The complete failure-name set exactly matches the prior
  bridge and its independently reproduced baseline. Later mint stages were
  not reached; the required gate remains red under `td-ce0146`.
- `RUNA_BIN=./target/release/runa CARGO_BUILD_JOBS=1 nice -n 15 ./scripts/differential.sh`:
  ordinary roundtrip 5/5 with no skips; imported execution 3/4. A separate
  `runa check tests/differential/corpus/imports/import_mesh_consumer.runa`
  confirms the same two `E0308` `Plan`/`Policy::Plan` errors reproduced on
  untouched `7bb4bc37` above. Later stress and generated-import stages were
  not reached. Differential is the selected deeper lowering lane; core canary
  was not rerun for this proof-only extension, which does not change ordinary
  parser/runtime/codegen behavior.

No new full-grid concrete epoch was started for this bridge: the known FIND
residual would still force point-by-point work. The earlier 14,012-transition
checkpoint is preserved unchanged; its partial progress is not promoted into
an exact answer. The implementation follows the Futuruna semantic-change
ratchet, with ordinary runtime behavior unchanged and only Experimental
proof acceptance/artifacts affected; no stable compatibility-guide entry is
needed.

## Output-first correlated classification

The next implementation reuses the checked endpoint abstract interpreter for
classification boxes, including canonical collection callbacks, empty inputs,
strict evaluation and scoped rules. It carries rational affine correlations,
bounded rounding error and integer congruences through arithmetic. In
particular, converting a whole-krone amount to øre retains its 100-øre step:
independent rounding uncertainty cannot invent an impossible fractional step.
All overflow and division safety obligations remain in force.

The stream tries a bounded cache of source-coordinate boxes before exact
native/interpreted fallback. Uniform decisions are applied only to actual
host-produced cases inside that box. The host still records the ordinary
ordered classification transcript; these ephemeral facts are **not** new
regional journal certificates. Cache misses, unsupported operations and
uncertain predicates remain exact residual work. The cache holds at most 16
tiles and supports at most eight independent integer axes. Its tile widths
and isolated upper endpoints are operational choices, not tax thresholds.

The canonical 2026 experiment now obtains these results with the unchanged
full-query model and fixed facts:

| Starting income (DKK) | Starting commute (km) | Intervention | Checked result |
| --- | --- | --- | --- |
| 1,000..1,100 | 50 | +1 DKK | All 101 valid edges harmless |
| 1,000..1,100 | 0..199 | +1 km | All 20,200 valid edges harmless |
| 349,499 | 50 | +1 DKK | Valid harmful edge |

These are checked-box results, not a full-grid journal closure. The three
box evaluations took 12.79s, 12.97s and 12.28s in the debug experiment; including
canonical frontend preparation, the experiment took 460.62s. Before preserving
integer congruences, the two non-singleton boxes were inconclusive.

For concrete output before attempting the full grid, the separate
[boundary window](personskat-income-distance-boundary.explore.runa) searches
all 201 distances and both interventions at the six starting incomes
342,497..342,502 DKK. This deliberately targets a documented mechanism
boundary; it cannot close the complementary income ranges. It contains 2,412
candidates, including six explicitly rejected outward distance edges.

The first published witness is a **2.11-DKK annual loss** at a **25-km** daily
round-trip commute when salary increases from **342,499 to 342,500 DKK**.
The saved loss is exactly **211 øre**, with the salary intervention `(1 DKK,
0 km)`. Both endpoints pass canonical model validity. This is a result from
the new boundary run, not an inference from a sampled mechanism signature.
This initial witness has since become part of the exact completed boundary
output recorded below.

The encoded supplement reduction uses whole-thousand steps in
[`ll9c_lavindkomst_aftrapningstrin_1000`](ligningsloven_fradrag.runa). This is why
an unchanged branch path is not sufficient to dismiss an income edge:
discrete arithmetic can change a deduction within that same path.

```sh
runa explore examples/danish-income-tax/personskat-income-distance-boundary.explore.runa \
  --query personskat_income_distance_boundary_2026 \
  --run-state /private/your-boundary.run --output /private/your-boundary.result \
  --time-limit 10m --json
```

Run the model epoch separately from large builds on an 8-GiB machine. The
initial native build took 433.74s; a cached reopen reused it in 255ms. An
overlapping build caused `resource_reserve_backoff` before classification,
not a negative answer. The resource reserve was preserved.
With the updated executable running alone, preparation took 76.55s and the
new compiler-specific native build took 197.10s. Classification and mechanism
publication then began normally. The full optimized `runa` build took 7m42s;
its only warning was the pre-existing pair of unused code-generation helpers.

Iteration checks deliberately follow the user's output-first instruction:

- `cargo test --lib adjacent_ -- --test-threads=1 --nocapture`: four passed
  in 0.07s, including the two new rounding/box checks.
- `cargo test --lib adjacent_box_outcomes_and_residuals_share_exact_ordered_accounting -- --nocapture`:
  passed in 2.36s. Both scheduled and canonical execution agree with an
  independent 400-case oracle: 30 rejected, 368 harmless and two losses.
- The explicitly invoked `canonical_2026_box_output --ignored --nocapture`
  experiment produced the canonical results above.
- `cargo fmt --check`, `git diff --check`, and
  `runa fmt --check examples/danish-income-tax/personskat-income-distance-boundary.explore.runa`
  passed.

Mint, differential, canary and the full Futuruna suite are deferred at the
user's explicit request while pursuing real output. This does not mark the
previous gate debt resolved or claim that the full Explore feature is finished.

## Completed all-distance boundary output

The unchanged boundary query completed with compiler commit `caf62976` on
2026-09-06. All relation, finding, result and mechanism layers closed exactly.
The query has six starting salaries, **342,497..342,502 DKK**, all integer
daily round-trip distances **0..200 km**, and separate **+1 DKK / +1 km**
interventions. Salary successors can reach 342,503 DKK. Fixed facts and the
integer-øre metric remain those of the full query above.

| Accounting | Exact count |
| --- | ---: |
| Directed candidates classified | 2,412 |
| Valid in-bounds transitions | 2,406 |
| Income cliffs | 176 |
| Commute-increase losses | 0 |
| Harmless transitions | 2,230 |
| Outward 200→201-km exclusions | 6 |
| Other model-validity exclusions | 0 |
| Findings with successful mechanism replay | 176 |
| Structural mechanisms / raw signatures | 4 / 4 |
| Unavailable explanations | 0 |

Every cliff occurs at **342,499→342,500 DKK**, one at each integer commute
from **25 through 200 km**. All 1,200 in-bounds commute-increase edges are
harmless within this six-salary window; this is not a full-income-range result.

| Daily round-trip commute | Annual loss |
| --- | ---: |
| 25 km | 2.11 DKK |
| 50 km | 50.06 DKK |
| 75 km | 98.24 DKK |
| 100 km | 144.08 DKK |
| 150 km | 144.08 DKK |
| 200 km | 144.09 DKK |

The maximum is **14,409 øre**, first attained at 99 km and attained at 18
distances in total. The encoded low-income commuting supplement steps down
at this income boundary. The supplement cap explains the plateau at longer
distances; integer rounding leaves one-øre differences within it. These are
research-model results, not independently validated legal conclusions or
individual advice.

All four structural assignments report zero before/after differential nodes
and edges. That describes unchanged execution structure, **not** unchanged
amounts: these 176 numeric losses are direct evidence that path similarity
alone cannot close unvisited income or distance routes.

The final journal sequence is **19,384**, with head
`648c327732bc9f0f274132a587d8b7fc21f3457f518621ee7844e3ad0047397d`.
The selected-case result root is
`692ddb6e3cffc2717b4f6a810dd2db74eb2a2e43aa74a38534f018a1cc6453e8`.
The final governed completion epoch took about 25 minutes after earlier saved
epochs; it is not a total cold-to-completion benchmark. State and output
together occupied about 583 MiB, dominated by shared mechanism definitions.

This run also exposed a concrete finalization cost: the first selected-result
projection quantum took **494.082 seconds**, re-evaluating selected measures
before accepting their deterministic projection. `td-ddc8bc` tracks bounded,
resumable rehydration or a checked reuse gateway. Evidence agreement must stay
intact; repeating every expensive calculation in one uninterruptible quantum
is not a scalable publication strategy.

## Preserve shared scalar results across branches

The checked-box interpreter now gives a total pure scalar result an exact
call identity when every argument and scoped capture is itself symbolically
identified. This lets copied nonlinear results reconcile even when a branch
join has lost their affine formula. Equal interval bounds alone never imply
identity. Strict evaluation, partial-call rejection and overflow checks still
precede any such identity.

Spare slots in the existing eight-symbol correlation budget can represent
these shared results, allowing cancellation inside larger expressions such
as `(f(x) + 2) - (f(x) + 1)`. No numerical formula for `f` is invented.
Capacity exhaustion falls back to a bounded opaque identity/interval and can
lose precision, not coverage. This mode is confined to ephemeral box
classification; ordinary endpoint-proof values and certificate roots retain
their previous behavior. No source syntax or journal format changes.

The actual canonical measurement now proves all three admission predicates
for each of these boxes:

| Starting income (DKK) | Starting commute (km) | Intervention |
| --- | --- | --- |
| 342,497 | 0..199 | +1 DKK |
| 342,497 | 0..199 | +1 km |
| 342,000..342,498 | 0..199 | +1 DKK |

The last box contains 99,800 salary edges. Previously, the first high-income
box left both endpoint-validity predicates unknown. **The loss predicate is
still unknown on all three boxes**: they do not become harmless regions or
regional journal certificates. The earlier two low-income harmless boxes and
the singleton cliff remain provable. The six box calls took 8.77–9.41 seconds
each in the debug measurement; total canonical preparation and measurement
took 300.77 seconds. This is not a controlled speed comparison.

Only focused checks were run for this patch, with one shared build job:

- `cargo test --lib symbolic_scalar_calls_preserve_aliases_not_equal_enclosures -- --test-threads=1 --nocapture`:
  passed in 0.11s after renaming the fixture's conflicting `Pair` constructor.
  It covers shared calls, independent equal enclosures, translated copies and
  distinct scoped captures. The initial fixture failed checked dependency
  closure before exercising the new proof logic.
- The existing library-test binary ran
  `adjacent_box_outcomes_and_residuals_share_exact_ordered_accounting --nocapture`:
  passed in 0.81s, retaining the independent 400-case oracle.
- The same binary ran `canonical_2026_box_output --ignored --nocapture` once
  for the actual-model measurement above; it is excluded from routine tests.
- `cargo fmt --check` and `git diff --check` passed.

Mint, canary, differential and the full suite remain deferred by the user's
explicit output-first instruction. The library was compiled for these checks;
another optimized CLI rebuild is deferred until the next production epoch.
The completed boundary output above was produced by `caf62976`, before this
additional proof-precision patch.

## Resumable row-local result publication

The `losses` result now publishes one canonical row per governed quantum.
Previously, its first projection step re-evaluated every selected measure
before returning any publication progress—the 494.082-second step measured
above. A pause can now preserve each accepted row independently.

The checked driver keeps at most 4,096 small, process-local evaluation
receipts. A receipt is created only after evaluating the row's result
expressions against its checked case; it avoids repeating that work when the
same driver later projects the row. Eviction affects speed, not results.
Receipts are never restored from disk.

On a cold resume, the accepted projection prefix is checked for exact
canonical order and agreement with the sealed row evidence. The next
unpublished row is re-evaluated and must match its complete recorded measure
and SELECT values before publication. Already accepted projection rows are
durable completed work; their tax calculations are not executed again. This
is reuse of authenticated execution progress, not a claim that hashes alone
prove the semantics of an arbitrarily rewritten journal.

This path applies to `each case` results from a find when every SELECT is
row-local and there is no grouping, aggregate, choice or HAVING dependency.
Grouped and deferred results retain the existing reducer and its cold-rebuild
cost. Final result closure still uses the existing exact reduction check;
source syntax, journal encoding and result-root definitions are unchanged.
The full-grid numerical proof and coverage limitations above remain open.

The permanent four-case edge check
`row_local_publication_reuses_warm_receipts_and_resumes_each_cold_row` passes
in 0.09 seconds. It verifies one row per projection quantum, identical final
roots for warm execution and a fresh journal replay after each output row,
and rejection of both a changed accepted projection and a changed unpublished
measure/SELECT with repaired record and journal hashes. Warm execution makes
20 result-expression calls total; repeated cold resumes make 35, rechecking
only each next unpublished row rather than the entire population.

The existing
`answer_index_describes_ungrouped_grouped_and_exact_empty_results` edge check
also passes (0.33 seconds). No mint, canary, differential or full-suite run
was added: those remain explicitly deferred for the output-first work.

The optimized CLI then completed an external two-transition canonical-model
measurement, with the same conditioned profile and result/mechanism consumers:
starting salary 342,499 DKK, commute 50 km, and separate +1 DKK / +1 km
interventions. Both transitions were valid, the salary edge lost exactly
**5,006 øre**, and the commute edge was harmless. All result and mechanism
layers closed, with one successfully explained finding and no exclusions or
unavailable explanations. This reproduces a witness in the completed boundary
window; it does not add coverage of the complementary income ranges.

The row-local projection quantum and its final result-close quantum each
reported **0 ms** at millisecond resolution. This confirms the actual-model
path executes without a bulk rehydration delay; it is not a controlled speedup
comparison against the earlier 176-row cold publication. Preparation took
75.745 seconds, and this small run used ordinary execution without building
another native classifier. Its final journal sequence is **3,992**, head
`7944cd7f8b172565e36f47e2d3fb10da4dc0d91e0146a024d67183ff8d2b98bc`,
and its selected-case result root is
`84a4645b3593a938cd35a43f3b7f742190795d4351658d1898f67a0cf403f3ea`.
State and output together occupied about 317 MiB, predominantly mechanism
definitions; row-local publication does not solve that separate storage cost.

The one required optimized build was
`env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target CARGO_BUILD_JOBS=1 nice -n 15 cargo build --release --bin runa --jobs 1`:
it passed in 5m48s with only the two pre-existing unused CLI-helper warnings.
The executable ran `explore publication-edge.explore.runa --query personskat_publication_edge_2026 --run-state state --output output --time-limit 20m --json`
with `FUTURUNA_EXPLORE_TRACE=1`, returning success and complete output.
The external fixture needed its import spacing formatted; `runa fmt` and then
`runa fmt --check` passed. A separate `runa check` was omitted because this
actual execution already performs checked frontend preparation. `cargo fmt
--check` and `git diff --check` passed for the implementation.

## Keep rounding uncertainty fractional

Two bounded precision changes address the high-income loss predicate without
changing the canonical tax calculation or its integer-øre outputs:

- After checked evaluation proves an integer is one exact constant, optional
  box classification discards any older, wider affine rounding enclosure for
  that value. This also applies to cached-value delivery. It does not skip
  evaluation, overflow checks or division-by-zero obligations, and ordinary
  endpoint-totality mode retains its prior values.
- The affine companion now carries error numerators under its existing exact
  denominator. Nested division no longer rounds an already fractional error
  bound outward to a whole integer at every intermediate step. Integer
  congruences also establish when division is exact and adds no rounding
  uncertainty. The representation has the same fields and bounded size; all
  arithmetic remains checked `i128`, with loss of precision on capacity
  exhaustion rather than permission to omit cases.

The measured starting salary box, 342,000..342,498 DKK at 50 km with +1 DKK
edges, initially had a net-change enclosure of **−5,157..5,272 øre**. Exact
constant normalization alone tightened this to **−99..224 øre**: still
unknown, not evidence of a cliff or a harmless region. It proved the narrower
342,001..342,010-DKK box harmless, with net change **42..93 øre**. The
342,497-DKK salary edge across 25..50 km initially remained unknown at
**−7..136 øre**, motivating fractional error bounds.

Focused permanent checks:

```sh
env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target \
  CARGO_BUILD_JOBS=1 nice -n 15 cargo test --lib rounding_ -- --test-threads=1 --nocapture
```

All five matched edge checks passed in 1.78 seconds: signed adjacent rounding,
constant quotient regions and their true thousand-step cliff, nested rounding,
exact positive/negative rescaling, and isolated unit cliffs retained as exact
product residuals. The standalone constant-quotient check also passed in
0.06 seconds before the fractional-bound change. Mint, canary, differential
and full-suite runs remain deferred at the user's request.

The ignored `canonical_2026_box_output` experiment accepts test-only
`FUTURUNA_EXPLORE_BOXES` JSON containing arrays of three inclusive bound pairs
(income, commuting distance, intervention). It checks those bounds against
the unchanged full query and prints the exact final comparison enclosure.
This is measurement control, not public Explore syntax or new proof authority.
At this measurement stage, the optional classifier decisions did not create
durable regional certificates or close portions of the saved full-grid journal.

The final canonical measurement with both changes produced:

| Starting income (DKK) | Starting commute (km) | Intervention | Box size | Proved net change (øre) | Decision |
| --- | --- | --- | ---: | --- | --- |
| 342,497 | 25..50 | +1 DKK | 26 | 2..127 | Valid, harmless |
| 342,000..342,498 | 25..50 | +1 km | 12,974 | 24,463..24,907 | Valid, harmless |
| 342,499 | 50 | +1 DKK | 1 | −5,006 exactly | Valid, cliff |
| 342,000..342,498 | 50 | +1 DKK | 499 | −98..223 | Valid, loss decision unknown |

The first two regions contain 13,000 distinct harmless directed edges. The
last probe overlaps one salary edge in the first region; this diagnostic
table is not an additive whole-query coverage ledger. Each result follows
from the full checked canonical model on the box, not sampled endpoints or
matching mechanism paths. Bounds are inclusive; commute successors in the
second row reach 51 km. The net-change intervals are safe enclosures, not
claims that every value in the interval occurs.

The four box calls took 8.97–10.21 seconds each in the debug measurement;
including shared canonical preparation, the explicitly invoked experiment
took 292.68 seconds. This used the existing library-test binary with
`FUTURUNA_EXPLORE_BOXES='[[[342000,342498],[50,50],[0,0]],[[342497,342497],[25,50],[0,0]],[[342000,342498],[25,50],[1,1]],[[342499,342499],[50,50],[0,0]]]'`
and `canonical_2026_box_output --ignored --nocapture`. The earlier baseline
and constant-only measurements took 293.22 and 313.95 seconds respectively;
these are diagnostic runs, not a controlled throughput comparison. No extra
optimized CLI rebuild was performed for this precision patch.

These measurements motivated `td-ed3104`: retain the checked AST box proof
as an honestly typed, replayable regional certificate (described below).
The original journal producer requires complete lowered graph lanes, while the
canonical model still has residual collection/dispatch lanes that this box
interpreter can handle. A residual AST expression must not be disguised as a
graph node. Mixed admission and the remaining wide salary rounding dependency
also remain open under `td-966941`; the full 0..400,000-DKK query is not closed.

## Replayable checked-source-box certificates

The checked box interpreter can now issue a distinct **version 5 regional
certificate** when an entire canonical product enclosure is proved valid and
not selected. This connects the numerical box proofs above to durable
classified-support accounting. It does not change the tax model or add query
syntax.

The existing version 4 graph certificate retains its encoding, hash inputs,
and replay-authority identity. Version 5 explicitly identifies a checked
source-box derivation instead of pretending residual source expressions are
classification-graph nodes. Both routes remain bound to the checked query,
admission and question, support plan, exact canonical child, and exact case
cardinality. A conservative rectangular enclosure may include extra source
points, but the accepted count is only the weight of the canonical rank slice.

The source-box proof commits to the checked program, source coordinates,
admission/selection decisions, and strict evaluation obligations. Its digest
is not proof authority: journal acceptance and cold replay freshly derive the
same theorem from the checked program and require the complete artifact to
match. Replay uses the recorded proof route, so making a newer producer
available does not silently replace an older certificate's derivation.
Mixed admission, selected points, unknown comparisons and unsupported
evaluation remain residual work; a matching mechanism path cannot close them.

The permanent edge
`checked_box_child_certificate_replays_without_graph_nodes_or_pointwise_cases`
proves an 800-case collection-based product through the source-box route while
the graph-only producer remains residual. It checks exact rank weights,
support projection without concrete cases, journal encoding and cold replay,
and rejection of altered bounds, outcomes, program identity and derivation
roots even after repairing artifact identities. It passes in 0.18 seconds.
The unchanged graph-based
`product_child_certificate_replays_exact_rank_weight_and_typed_starter_chain`
passes in 0.08 seconds. These are focused certificate edges, not a full
compiler validation claim; mint and deeper suites remain deferred as requested.

The optimized CLI then closed the canonical-model slice with starting incomes
**342,000..342,498 DKK**, starting daily round-trip commute **25..50 km**, and
the **+1 km** intervention. The other conditioned facts, canonical assessment,
validity predicates, integer-øre metric and result/mechanism consumers were
unchanged. Commute successors reach 51 km, within the full query's 200-km cap.

All **12,974** candidates were admitted and not selected, with **zero
exclusions or losses**. The case/support graph contains **51 regional
certificates**: fifty exact 256-case children and one 174-case child. There
were **zero concrete classification sweeps** and no observer-memo entries or
calls. All four analysis layers closed and all 11 published artifacts caught
up to the final journal. This is actual saved negative-region coverage for
this query, not merely an optional classifier decision.

State occupied approximately **180 KiB**, output **216 KiB**. The 51 certificate
quanta reported **214.506 seconds** in total. Frontend preparation took
**78.767 seconds** and the eagerly built native fallback took **208.365
seconds**, although no concrete sweep subsequently needed it. Trace/report
file timestamps give an approximate end-to-end span of **544 seconds**;
this is not a controlled benchmark. The governor reported four CPU-pacing
pauses totaling 4.498 seconds. The optimized compiler build itself took
7m25s, separately from this run, using one low-priority build job.

Final journal sequence: **519**, in **12 segments**, head
`aecbe5511204f694858845a2dcbd939fba35d45b1d7f1063651a58ae5ba39e93`.
The actual command was:

```sh
env FUTURUNA_EXPLORE_TRACE=1 nice -n 15 \
  /Users/andreasrudolph/futuruna-explore-spec/target/release/runa explore \
  /tmp/futuruna-checked-box-output.zbifuK/checked-box.explore.runa \
  --query personskat_checked_box_2026 \
  --run-state /tmp/futuruna-checked-box-output.zbifuK/state \
  --output /tmp/futuruna-checked-box-output.zbifuK/output \
  --time-limit 20m --json
```

The external fixture passed `runa fmt --check`. Its first launch with only a
relative filename failed import resolution before exploration; the absolute
path above succeeded. No separate `runa check` was run because actual
execution performs the checked frontend preparation. The build required no
cache deletion: low disk space interrupted the first focused test build, then
space recovered externally and subsequent work proceeded serially.

A separate cold process then reopened the same state/output with the same
command and a `10m` limit. It completed in approximately **192 seconds**,
including **81.385 seconds** of preparation; native cache reuse took 226 ms.
It added **zero semantic events and zero batches**. A direct comparison of
the two JSON reports confirmed identical checkpoint, exact counts, coverage,
all published artifacts, and analysis closure root. The empty `losses` result
root is `bdb698e513727c90fba077fdfff6601d5db5b68b674dc043c588d3d894ba7404`.
Cold replay re-derives the checked-box certificates; it does not restore an
operational classification cache or classify each of the 12,974 points.

The focused Rust command was
`env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target CARGO_BUILD_JOBS=1 nice -n 15 cargo test --lib checked_box_child_certificate_replays_without_graph_nodes_or_pointwise_cases -- --test-threads=1 --nocapture`.
The legacy edge used the resulting library-test binary directly with
`product_child_certificate_replays_exact_rank_weight_and_typed_starter_chain --test-threads=1 --nocapture`.
`cargo fmt --check` and `git diff --check` passed. An initial edge assertion
expected a semantic mismatch where V5 correctly rejected an invalid shape
earlier; the expectation was corrected before the passing run. No broad gate
or per-point canonical oracle was added for this change.

The full 0..400,000-DKK / 0..200-km query is still open. Its canonical pages
mix inward transitions with outward upper-bound edges; they require exact
subregion or mixed-admission accounting. Wide salary comparisons also still
need tighter rounding dependence. This separately bound slice is not silently
imported as coverage of the older full-grid checkpoint.

## Exact mixed-admission covers of ranked pages

Version 6 regional certificates add a bounded tree of source-factor splits
inside an existing canonical page. They can close a page whose leaves are
either rejected or admitted/not-selected, without pretending that the parent
has one uniform admission outcome. Versions 4 and 5 retain their existing
encoding and replay recipes.

Factor restrictions retain original tuple coordinates. Their rank endpoints
are translated by exact mixed-radix prefix counts, not by enumerating incomes
or kilometres. Thus splitting the intervention and the 200-km endpoint does
not require one leaf per income. For the original grid's first 65,536-rank
page, the geometry alone accounts for 32,768 salary steps, 32,605 inward
commuting steps and 163 outward commuting steps. Geometry is not tax evidence:
the checked model must still prove each leaf's admission and FIND result.

Search is bounded to 31 tree nodes. Every accepted leaf is freshly proved;
an unknown predicate or a selected loss leaves the page open. The journal
stores the split recipe, leaf outcomes, exact weights and derivation roots.
Cold replay reconstructs the partition and re-proves every leaf. Public
support rows report each leaf separately. A shared mechanism signature is
still never permission to skip a region.

A saved concrete prefix can be reconciled with this cover: every existing run
is intersected with the proved leaves by exact rank counting and must agree
with their outcomes. Only after atomic proof acceptance is the pending
accumulator retired; the original slice events remain in the journal.
Disagreement refuses the proof and preserves the prefix. A failed search is
not repeated after each subsequent concrete slice in the same attached run.

The focused `ranked_box_mixed_cover_journals_exact_leaf_counts_and_cold_replay`
check passes with a 41-case saved prefix, a first page containing 250
admitted/not-selected and six rejected cases, public leaf rows, codec round
trip and cold replay. Initial integration attempts exposed a remaining
single-region projection assumption and test-fixture wiring errors; these
were corrected before the passing 0.14-second run. Broad gates remain deferred
at the user's explicit request.

The final focused library binary also passed `ranked_box_` (four checks,
0.51 seconds), `checked_box_child_certificate_replays` (one, 0.05 seconds)
and `product_child_certificate_replays` (one, 0.03 seconds), each with
`--test-threads=1 --nocapture` under `nice -n 15`. The optimized build used
`env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target CARGO_BUILD_JOBS=1 nice -n 15 cargo build --release --bin runa`
and finished in 4m29s with the two pre-existing unused CLI-helper warnings.
`cargo fmt --check` and `git diff --check` passed. No full mint, canary or
differential lane was run.

The first actual full-grid resume with V6 did **not** close page zero. Its
bounded search declined, retaining the old concrete prefix and appending only
one checked transition (sequence 261 to 263; 14,013 pending transitions, still
zero public classified lower bound). Model preparation took 61.856 seconds;
the eager native evaluator build added 155.039 seconds. The captured report is
`/tmp/futuruna-mixed-cover-output.hK6DTI/report.json`; this attempt must not be
reported as certified full-grid coverage.

A three-box diagnostic localized one blocker to known-zero parameters in
`penge_mindste_beløb(a, b)`, whose body is `if a < b { a } else { b }`.
For `a = 0` and nonnegative `b`, literal-only branch refinement retained a
spurious positive alternative in the else branch. That made the negative
share-tax conservation check uncertain even with no share income. Zero income
at every 0..200-km commute was proved valid/harmless; income 0..163 at zero
commute and income 1..163 at all commutes remained uncertain. These are proof
precision findings, not tax-rule changes or newly discovered invalid inputs.

V6 now permits branch refinement from a checked parameter whose abstract
integer bounds are a singleton. It does not treat a range as constant, evaluate
new calls during refinement, or change the V4/V5 recipe. The small
`cover_branch_constants` reproduction passed in 0.07 seconds; the mixed-cover
replay and V5 compatibility edges then passed in 0.13 and 0.05 seconds.

Proof-focused CLI runs can set
`FUTURUNA_EXPLORE_DISABLE_NATIVE_CLASSIFIER=1` to skip the native sidecar build.
Regional proofs and the checked interpreter remain enabled, as does the
resource governor. The default stays unchanged. This is an operational choice,
not a weaker admission check or a change to the query's answer identity.

### Actual mixed-cover output

The canonical model's low-income strip now closes completely: starting incomes
0..10 DKK inclusive, every round-trip commute 0..200 km inclusive, and both
separate unit interventions. The endpoint horizon remains 400,000 DKK / 200 km,
so salary 10 → 11 is admitted. The exact result is 4,422 candidates, 4,411
admitted/not-selected and 11 rejected outward 200 → 201-km steps. There are
zero losses or unclassified cases in this strip.

This actual run used 18 certified-region quanta, no concrete sweeps and no
observer evaluations. All four layers closed and all 11 published artifacts
caught up. Public case support occupies 61 records, with separate weights for
the mixed-cover leaves. Preparation took 60.711 seconds and regional quanta
151.791 seconds in total; native classification was disabled. The saved state
is 92 KiB and output 172 KiB on the measured filesystem. The six-minute run
finished normally at sequence 222, nine segments, journal head
`1054db47248a8b9ffae1dcc0abb55d1c5a8a8f6cbfb240ab2114331ab37518f2`.

Evidence is under `/tmp/futuruna-mixed-cover-output.hK6DTI/`: the derived query
`low-income-strip.explore.runa`, `strip-report.json`, `strip-trace.log`,
`strip-findings.md`, `strip-state/` and `strip-output/`. Its inherited block
comment describes the parent full grid; the executable income range is
`range(0, 11)`. This is a separately bound query, not silently imported coverage
of the original full-grid checkpoint.

The second original-grid attempt confirmed the known-parameter fix makes
low-income model admission provable. It still failed to close page zero because
the salary-rounding comparison remained unknown. It was interrupted after
retaining additional concrete progress; no public full-grid coverage is claimed.
One loaded-model diagnostic proved the full 0..163-income / 0..199-km inward
commuting box harmless and the 0..10-income / 0..200-km salary box harmless.
These diagnostic boxes are not themselves journal certificates.

### Preserve rounded dependence through checked clamps

A second precision fix recognizes an exact checked integer clamp: one
exception returns the sole argument under `x > c` (or `>=`, `<`, `<=`), and
the fallback returns that same literal `c`. Both heads must bind the whole
argument. After ordinary strict dispatch and totality succeed, the analyzer
can preserve the argument's correlation on the clamp's identity side. It does
not recognize tax-rule names, assume equal enclosures denote equal values, or
ignore extra exceptions. Unrecognized shapes remain conservative.

The V6 producer tries the previous checked-box recipe first. Only an unknown
outcome uses this stronger fallback, preserving the derivation roots of
previously accepted V6 leaves. V4/V5 continue to use their existing precision.
The focused `cover_checked_clamp` edge passed in 0.11 seconds, including
altered constants, guards, an extra exception and a negative-crossing input.
Initial compile/fixture failures were corrected before this passing run. The
mixed journal/cold-replay edge also passed in 0.11 seconds after the change.

On the actual canonical model, the complete salary box with starting incomes
0..163 DKK and distances 0..200 km then proved all admissions and no loss in
7.381 seconds (after model loading). The after-minus-before net-income bound
is 0..100 øre: an affine slope of 92 øre per DKK combined with the exact
whole-krone congruence of AM-tax rounding. This replaces the previous loose
-1,200..1,400-øre comparison; no tax rule was changed.

Two wider boxes in the same loaded-model diagnostic also proved harmless:
starting incomes 0..50,000 at every 0..200-km salary step (6.543 seconds), and
the same incomes at every 0..199-km inward commuting step (6.804 seconds).
The latter difference is exactly zero. These enclosures respectively cover
10,050,201 and 10,000,200 transitions, but remain diagnostic evidence until
bound into the requested query's saved support. Logs are in
`/tmp/futuruna-mixed-cover-output.hK6DTI/clamp-boxes-run.log`.

### First saved page in the original full grid

The original query and checkpoint now have actual regional coverage. Page zero
closed with three leaves: 32,768 valid harmless salary steps, 32,605 valid
harmless commuting steps, and 163 rejected outward commuting steps. These sum
to its exact 65,536 candidates. It includes all distances for incomes 0..162,
then distances 0..4 for income 163. The saved concrete prefix was reconciled
without replacing history, and no new concrete sweep was needed for this page.
The certified-region quantum took 14.914 seconds.

The original state at `/tmp/futuruna-rule-regions.zIAuRQ/personskat-blocks-state`
is now sequence 330, 33 durable segments, journal head
`c3268c6b82dafdd4d8537bdc13cd84da8682d9dbd26b757f3d986fcf762b335b`.
All 11 artifacts in the adjacent `personskat-blocks-output` caught up. Its
case-support graph has five records: root, page and three regional leaves.
State/output occupy 496/88 KiB on this filesystem. The certificate is
`5d2d06742d9aae9336853d6283150484d788b8c5050a6a208527ec5797963278`.

This actual run used the optimized build completed in 4m31s and the command:

```sh
env FUTURUNA_EXPLORE_TRACE=1 FUTURUNA_EXPLORE_DISABLE_NATIVE_CLASSIFIER=1 \
  nice -n 15 /Users/andreasrudolph/futuruna-explore-spec/target/release/runa explore \
  examples/danish-income-tax/personskat-income-distance-unit.explore.runa \
  --query personskat_income_distance_unit_2026 \
  --run-state /tmp/futuruna-rule-regions.zIAuRQ/personskat-blocks-state \
  --output /tmp/futuruna-rule-regions.zIAuRQ/personskat-blocks-output \
  --time-limit 3m --json
```

Preparation took 63.946 seconds. The run then tried high-income endpoint page
2453, failed to close that page and retained one concrete transition there.
That quantum took 92.359 seconds including bounded proof search and fallback.
The run paused normally at its runtime limit. This next page, and the full
query, remain open; the public 65,536 classified count is a lower bound. No
no-cliff claim is made for the remaining 160,734,866 candidates.

The report, trace and readable findings are `original-page-report.json`,
`original-page-trace.log` and `original-page-findings.md` under
`/tmp/futuruna-mixed-cover-output.hK6DTI/`. The final focused compile/run was
`env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target CARGO_BUILD_JOBS=1 nice -n 15 cargo test --lib cover_checked_clamp -- --test-threads=1 --nocapture`;
the resulting test binary ran `ranked_box_mixed_cover_journals --test-threads=1 --nocapture`
under `nice -n 15`. Both checks passed in 0.11 seconds each. Formatting and
diff checks passed. Broad gates and another canonical cold replay were
deliberately deferred in favor of actual output, at the user's request.

### Checked integer dependencies across rounding

The next V6 precision fallback retains a bounded integer-expression DAG while
the existing checked interpreter evaluates the model. It preserves shared
subexpressions through addition, subtraction, constant multiplication and
constant division. Division truncates toward zero, including negative values
and divisors. Unrecognized expressions remain independent, unbounded integer
symbols unless their checked expression identities are identical. Equal
numeric enclosures do not establish identity. Only the original source axes
receive domain bounds; branch-local intermediate bounds are not asserted
globally.

An optional installed `z3` process may strengthen an unknown, direct ordered
integer FIND comparison **only after every admission has been proved true**.
It asks whether any assignment violates the desired non-selection predicate.
Only a fresh successful `unsat` response grants that refinement. A `sat`
response is merely a possible counterexample in an overapproximation, not an
authenticated case or an income-cliff finding. Missing Z3, unsupported terms,
timeouts, `unknown`, process failures and malformed output leave the region
residual. No tax formula is copied into a separate solver model.

Each obligation has a 10-second solver timeout, a 128-MB solver memory limit,
at most 4,096 reachable terms and a one-MiB script limit. The capture graph is
bounded at 65,536 nodes. Workers run serially under the calling process's
priority and the existing Explore containment policy. Z3 is an additional
trusted solver in this Experimental fallback, not a proof checked by the small
Futuruna kernel. No dependency is installed automatically.

The checked-box derivation commits the exact generated obligation and its
non-selection polarity. The digest alone confers no authority: saved cover
leaves are freshly proved on replay, including a fresh solver response when
needed. Reopening solver-backed evidence therefore requires that obligation
to succeed again; lack of a solver or a timeout cannot silently authorize it.
Existing closed V4/V5/V6 derivations keep their prior recipes and roots. The
new fallback only strengthens previously unknown comparisons.

Unknown checked affine guards also nominate adjacent source-coordinate cuts.
These are search hints, not evidence. The cover first separates tiny
categorical axes, prefers checked cuts isolating declared upper endpoints,
then uses balanced interior guard cuts. Every child still needs its own exact
rank geometry and classification proof; replay follows the recorded partition
rather than rerunning the search heuristic. Commuting boundaries are derived
from checked expressions, not a hardcoded list of tax thresholds.

The motivation is measured on the canonical model. For starting salaries
399,900..399,999 DKK at every 0..200-km commute, the earlier interval comparison
was unknown, with a -2,049,038..2,049,163-øre enclosure. The captured integer
obligation instead proved all **20,100 salary transitions** harmless in
5.92 seconds with about 29 MB maximum RSS. At a fixed zero-km commute, its
100-transition counterpart proved harmless in 2.00 seconds. Conversely, the
wide commuting-step relaxation returned `sat`, which was **not** reported as
a real loss. These diagnostic obligations are retained under
`/tmp/futuruna-high-income-regions.1ShEd7/`; they are not themselves saved
coverage of the original full-grid journal.

The focused `checked_integer_cover_replays_unsat_and_rejects_forged_roots`
edge passed in 0.26 seconds: both FIND polarities, all 400 valid rounded
transitions, two excluded endpoints, fresh cover replay and rejection of a
forged prior derivation root. The separate signed-division, solver-polarity and
distinct-opaque-expression edge passed in 0.11 seconds. Fixture setup and a
fixed-partition-shape assertion were corrected before the passing cover run;
the assertion now checks exact outcome totals rather than a particular search
tree shape. A subsequent scheduling-only refinement prioritizes checked
upper-endpoint cuts. Broad gates remain deferred at the user's request.

### Original full-grid checkpoint after integer dependency proofs

The subsequent production run increased saved original-query coverage from
65,536 to **433,810 candidates**: **432,530 admitted, harmless transitions**
and **1,280 excluded endpoints**. It closed pages 0..5 and 2453 with 28
regional leaves. No new concrete sweeps were needed; the run's observer memo
had zero entries, hits, misses and inserts. This is actual journal coverage,
not the diagnostic solver measurements above.

The previously blocked page 2453 covers original ranks
`[160759808, 160800402)`. Its ten-leaf certificate accounts for **40,292
harmless transitions and 302 exclusions**: 20,096 salary steps, 20,196 commute
steps, 201 outward salary endpoints and 101 outward commuting endpoints.
Its enclosing coordinates start at income 399,900 DKK and extend through
400,000 DKK; exact rank clipping omits the first four commuting distances at
income 399,900. The salary leaf uses the shared integer-dependency obligation;
the commuting leaves split at checked guards. Its certificate is
`d9ab19f7ab9df2557c0076952e5228c714faf1fe19b64e50994f47a0ebb5d6cb`.
The existing page-0 certificate replayed unchanged, and new leaves were
freshly reverified before acceptance.

The original state now has sequence **375**, 39 durable segments and head
`c1f42d5f34077ae8ef20a081c1c1df1f4f20d8382552152d91c24efb761fee91`.
All 11 output artifacts caught up, with 36 case-support graph records. State
and output occupy 520 and 120 KiB respectively on this filesystem. The run
appended 20 semantic batches containing 45 events and paused normally at its
runtime limit. All analysis layers remain open: **160,366,592 candidates are
still unclassified**. Zero selected cases in this prefix is not a full-grid
no-cliff result. The separately completed 176-cliff boundary experiment above
has a different query identity and is not silently counted in this journal.

The optimized build completed in 5m58s. The actual invocation was:

```sh
env FUTURUNA_EXPLORE_TRACE=1 FUTURUNA_EXPLORE_DISABLE_NATIVE_CLASSIFIER=1 \
  /usr/bin/time -l nice -n 15 \
  /Users/andreasrudolph/futuruna-explore-spec/target/release/runa explore \
  examples/danish-income-tax/personskat-income-distance-unit.explore.runa \
  --query personskat_income_distance_unit_2026 \
  --run-state /tmp/futuruna-rule-regions.zIAuRQ/personskat-blocks-state \
  --output /tmp/futuruna-rule-regions.zIAuRQ/personskat-blocks-output \
  --time-limit 5m --json \
  > /tmp/futuruna-high-income-regions.1ShEd7/original-integer-report.json \
  2> /tmp/futuruna-high-income-regions.1ShEd7/original-integer-trace.log
```

Preparation took 90.006 seconds. The high-endpoint proof quantum took 75.915
seconds; subsequent low-income page quanta took 19.4..20.5 seconds each.
Measured process time was **606.30 seconds wall, 266.86 user, 8.56 system**,
with maximum RSS 1,207,877,632 bytes and zero reported swaps. The declared
five-minute runtime budget is not a five-minute wall-time measurement; the
additional elapsed time has not been diagnosed. Host CPU pacing remained
enabled and recorded one 1.121-second pause. Formatting and diff checks passed;
mint and deeper suites remain deliberately unrun under the user's explicit
output-first instruction. These are research results of the encoded model,
not an independent verification of Danish tax law.

### Reusing checked superset boxes across canonical pages

V7 regional covers add a scoped leaf: the exact page restriction still owns
its original coordinate count, but its classification may follow from a
larger, explicitly recorded source box. Acceptance verifies that every
coordinate in the leaf's enclosure belongs to that box, with the same
finite/singleton binding shape, and that the box is inside the declared
source domains. A producer-owned checked derivation must establish the
claimed rejected or admitted/not-selected outcome on the whole box. An
artifact digest or coincident mechanism signature cannot establish it.

The proof producer retains at most 64 such scope results per checked query
snapshot in memory. Clones share that cache; it cannot be filled from decoded
artifacts. A fresh process starts empty and proves each needed scope again,
including any required integer-solver obligation. Within that process, one
checked theorem may justify many contained pages without reevaluating the
same canonical model. Eviction can cause extra proof work, not missing
coverage. Unknown or failed scope proofs supply no classification authority.

The current widening heuristic uses source-relative 32,768-coordinate tiles
for sufficiently wide independent integer axes, retaining narrower context
and intervention ranges and isolating declared upper endpoints. The tile
size is an operational choice, not a tax threshold or a new query step.
Checked guard cuts can guide subsequent partitioning; a scope that cannot
close a leaf retains local proof or concrete fallback. Replay follows the
recorded scope, never this widening heuristic. At most 64 binding slots can
be encoded per scope, and the existing 31-node cover bound is unchanged.

This changes only the Experimental regional-proof artifact surface. New
scoped leaves use regional version 7; unscoped covers remain version 6.
V4/V5/V6 decoding, local replay recipes and identities are retained. Original
query, case, page and partition identities are unchanged. Older executables
that do not understand V7 must reject those new receipts; use an updated
executable to resume a journal containing scoped leaves.

The focused edge command was:

```sh
env CARGO_TARGET_DIR=/Users/andreasrudolph/futuruna-explore-spec/target \
  CARGO_BUILD_JOBS=1 nice -n 15 \
  cargo test --lib scoped_cover_pages -- --test-threads=1 --nocapture
```

It passed in 1.55 seconds after a 1m13s compile, with 747 other tests filtered
out. Three tiny pages from a 600,006-coordinate fixture matched independent
exact outcome counts; adjacent pages shared the same larger source theorem.
Freshly constructed classifiers reproduced the proofs; journal codec
roundtrip and cold replay preserved the saved head. Repaired artifact
identities did not authorize forged roots, non-containing scopes,
out-of-domain scopes or incorrect finite/singleton shapes. No full suite ran.

### Original full-grid output with shared scopes

The next actual five-minute-budget run accounts for **13,672,082 original
candidates**, up by **13,238,272** from the integer-proof checkpoint. This is
**13,637,871 admitted, harmless transitions and 34,211 exclusions**. Pages
0..207 and 2453 are closed, with 676 regional leaves and 886 case-support
records. The low-income prefix is exactly ranks `[0,13631488)`: all incomes
0..33,908 DKK at all distances and both interventions, followed by income
33,909 DKK at distances 0..34 for both interventions. The high endpoint page
remains the previously proved, disjoint rank interval.

No new concrete sweeps were needed, and the observer memo again had zero
entries and activity. The run appended 808 semantic batches / 1,818 events,
covering 202 more pages. Original V6 receipts, including the high-income
integer obligation, replayed successfully. The shared-scope path's first new
page (6) took 11.090 seconds; pages 7..200 then took approximately one
millisecond each to certify, using the retained larger checked theorems.
This timing covers the proof quantum, not the entire publication pipeline.

The next income tile, 32,768..65,535 DKK, crosses additional checked branches.
Some wide scope obligations returned `sat` in the relaxation; those were not
reported as losses. The cover used local proofs where necessary. Page 201
took 63.495 seconds and pages 202..207 took 14.25..14.52 seconds each.
Adaptive narrowing of shared scopes is therefore the next measured
opportunity; the one-krone and one-kilometre source grid is unchanged.

The saved original state is now sequence **2193**, 44 durable segments, head
`6a02592a4c4a92b4c2648c200b800a57f800232616bffbbb06934399d539b397`.
All 11 output artifacts caught up. State/output occupy approximately 1.1/1.0
MiB on this filesystem. The result remains a lower bound: **147,128,320
candidates are unclassified**, all analysis layers remain open, and zero
selected cases in this prefix is not a global no-cliff finding. The separately
completed 176-cliff boundary result still has its own query identity.

The optimized build took 4m58s. The invocation matched the previous original
query command (`nice -n 15`, native classifier disabled, governor retained,
`--time-limit 5m --json`) with report and trace redirected to
`/tmp/futuruna-shared-scope-output.frTahz/original-scoped-report.json` and
`original-scoped-trace.log`; `original-scoped-findings.md` there gives the
readable checkpoint. Preparation took 68.623 seconds. Measured total time
was **307.95 seconds wall, 272.29 user, 8.74 system**, maximum RSS
1,544,749,056 bytes and zero reported swaps. Exit status was zero, paused at
the runtime limit. Formatting and diff checks passed; mint/deeper gates and
a standalone full canonical replay were deliberately deferred under the
user's output-first instruction. The next real continuation can both replay
these new scoped receipts and extend the answer.

### Bounded refinement of reusable scopes

When a wide shared box cannot classify the requested leaf and cannot suggest
a useful checked split, scope nomination now tries source-relative widths
32,768, 16,384, 8,192, 4,096, 2,048, 1,024 and 512. It considers at most seven
nominations and performs at most three fresh shared-scope derivations per
cover node; previously retained scope results can be consulted without
reevaluating the model. The unchanged 64-entry cache now keeps recently used
scope results, including unknown results, so repeated wide failures do not
continually displace useful shared proofs. Exact local proof and concrete
fallback remain available when the bounded nominations fail.

This is an operational search refinement, not a proof-rule or artifact-format
change. Every accepted scoped leaf still records its precise V7 domain and
passes the same checked-domain, containment, outcome and derivation checks.
Old receipts replay their recorded scopes regardless of the current search
widths. Formatting and diff checks were selected for this heuristic-only
change; no additional test suite was started. Its real effect must be
measured on a continuation of the saved original query.

The actual continuation succeeded: original coverage increased by **9,437,184
candidates** to **23,109,266** — **23,051,580 admitted harmless transitions**
and **57,686 exclusions**. It closed another 144 pages with 576 semantic
batches / 1,296 events and no concrete sweeps or observer memo activity.
Pages 0..351 plus 2453 are now accounted for, with 1,972 regional leaves and
2,326 case-support graph records. The low prefix is exactly ranks
`[0,23068672)`: all coordinates at incomes 0..57,383 DKK, then income
57,384 at distances 0..151, for both interventions. There remain
**137,691,136 unclassified candidates**; all analysis layers are open.

The previous V7 checkpoint reopened successfully before new work. Scope
refinement found reusable 32,768..49,151 and 49,152..57,343-DKK boxes for
the previously unresolved leaves. Page 208 established its new cover in
48.341 seconds; page 209 took 2 milliseconds, and other cached pages took
roughly 2..4 milliseconds for the proof quantum. Page 301 straddled a tile
boundary and used local proof in 14.439 seconds; page 302 established further
shared scopes in 21.609 seconds. Page 351 also crossed a tile boundary and
took 14.230 seconds. These are measured proof quanta, not end-to-end page
publication times.

The original journal is now sequence **3489**, 48 durable segments, head
`47be83c213e32db9f88faa90bd21d3cd9ef8fe351b72e56f2b66853765d6c48c`.
All 11 artifacts caught up; state/output occupy approximately 1.7/2.4 MiB.
The optimized build took 5m00s. The same original query invocation used
`--time-limit 5m`, `nice -n 15`, native classification disabled and the
existing governor, writing `original-refined-report.json` and
`original-refined-trace.log` under `/tmp/futuruna-refined-scope-output.5Theqy/`.
The readable companion there is `original-refined-findings.md`.
Preparation took 69.877 seconds; measured total time was **315.09 seconds
wall, 275.49 user, 8.07 system**, maximum RSS 1,538,932,736 bytes, zero
reported swaps. Exit status was zero, paused at the runtime limit. Formatting
and diff checks passed; no new test run was added for this scheduling-only
change. As before, zero selected cases in the original prefix is not a
global no-cliff result, and the separate 176-cliff boundary evidence is not
silently counted as original-query coverage.

### Retaining uncertain checked clamps

A subsequent unchanged 8-minute continuation reached **23,633,554**
classified candidates: **23,574,564 admitted harmless transitions** and
**58,990 exclusions**, leaving **137,166,848** candidates unclassified.
Pages 0..359 plus 2453 are accounted for (2,044 regional leaves, 2,406
case-support records). All 11 artifacts caught up to sequence **3561**, 51
durable segments, head
`6e3e7ffd52db6eda64c9470d3ac8be1f1e96e0b459c8d66eea6ff42c868ca7a0`.
The outer wall governor then stopped the child with exit 1 during page 360;
no uncommitted evidence was accepted. Measured time was 511.73 seconds wall,
471.30 user, 11.39 system, maximum RSS 1,344,323,584 bytes and zero swaps.
The empty JSON report is not a final result; the trace and readable findings
are under `/tmp/futuruna-next-income-output.dVxmt2/`.

The failure localized a missing dependency: a salary box at
58,689..58,851 DKK remained unknown even at zero commute kilometres, while
58,689..58,770 closed. The prior checked clamp refinement recovered an input
only on a known identity side. Crossing a clamp made the before/after results
independent opaque integers, allowing spurious counterexamples in the solver
relaxation. Such SAT results are not modeled income-cliff witnesses.

The new fallback recognizes the same exact checked single-argument min/max
rule shapes after ordinary strict dispatch and totality. It records the
original input, constant and min/max operation as an integer conditional in
the bounded dependency DAG. Identities come from that expression, never from
equal result enclosures or tax-specific names. Intermediate/branch-local
bounds are not asserted globally, and only fresh UNSAT can close a box.

Previously successful recipes run unchanged. The new precision is attempted
only after a definite SAT from the old integer relaxation, not after timeout,
unavailability or unsupported input. A definite SAT cannot turn into UNSAT
for the identical obligation merely with more time, so previously closed
V4/V5/V6/V7 receipts keep their recipe and root. The new dependency identity
and exact solver script are committed by the ordinary derivation root.

The focused installed-solver edge passed in 0.85 seconds (1m22s test build):
`cargo test --lib checked_clamp_dependencies_cross_zero_preserve_polarity_and_legacy_roots -- --ignored --nocapture --test-threads=1`.
It checks crossing-zero max/min rules, strict and inclusive guards, negative
constants, both FIND polarities, malformed look-alikes with real jumps,
fresh producer replay and unchanged identity-side roots. It ran serially at
`nice -n 15` with the shared target and `CARGO_BUILD_JOBS=1`. Formatting and
diff checks passed. Mint and deeper lanes remain deliberately deferred under
the user's output-first instruction; the real original-grid continuation is
the next measurement, not implied by this small edge.

That measurement did **not** extend coverage. The 5m42s optimized build
reopened the original checkpoint, but the same salary box remained SAT in
the stronger relaxation, even at zero kilometres. The unchanged 8-minute
invocation stopped at the outer wall deadline (exit 1): 512.66 seconds wall,
468.50 user, 10.34 system; maximum RSS 1,553,940,480 bytes and zero swaps.
Sequence 3561 and all its counts remain intact. Trace, empty JSON report and
readable findings are under `/tmp/futuruna-clamp-scope-output.qrI3OS/`.
The single-argument clamp edge is therefore useful but insufficient; an
explicit one-box diagnostic is tracing the remaining opaque expression.

The one-box diagnostic identified the missing form exactly: the two free
values still used by the final solver obligation came from
`søbl5_positivt_beløb`, whose checked body is `if x > 0 { x } else { 0 }`.
The 2,986-byte obligation and opaque-call trace are retained as
`zero-km-clamp.smt2` and `zero-km-diagnostic.log` in that same directory.
The canonical box proof took 9.378 seconds; loading the full model through
the debug diagnostic dominated its 307.92-second total (including a 15.58s
incremental build), maximum RSS 2,297,757,696 bytes, zero swaps. Admissions
were all true; selection remained unknown. The diagnostic's test exit 0
does not mean the box was proved harmless.

Clamp dependency retention now also recognizes this conditional expression
inside either a rule or an ordinary function. It saves the checked input
from before branch narrowing, requires the same binder and literal bound in
the guard/results, and permits only single-expression branch blocks. Every
reachable branch still completes ordinary strict evaluation before the fact
is used. Recognition depends on checked structure, not the helper's name.
The expanded focused edge passed in 2.14 seconds after a 26.90-second
incremental build, covering all three spellings and the same negative,
polarity and fresh-root checks. An optimized original-grid continuation will
measure whether this resolves the remaining canonical dependency.

The next actual run did prove previously unknown salary scopes: income
32,768..65,535 / distance 0..24, 49,152..65,535 / 25..48, and
57,344..65,535 / 49..60, plus the local 58,689..58,851 / 61..66 box.
However, the 61..72-km box remained unknown and page 360 did not finish.
These transient proofs are **not additional committed coverage**. The outer
wall deadline again stopped the unchanged 8-minute run (exit 1); sequence
3561 and its counts remain unchanged. The second optimized build took
5m43s; preparation took 79.980s; the run measured 512.66 seconds wall,
467.75 user, 10.45 system, maximum RSS 1,554,579,456 bytes and zero swaps.
The `original-conditional-clamp-*` files in the same external directory
retain the trace, empty report and readable findings. A single diagnostic
of the remaining commuting-dependent box follows; no full suite was run.

That diagnostic found exactly two free terms in the final 7,027-byte solver
obligation: before/after `par13_absolut_underskud`, written
`if x < 0 { 0 - x } else { 0 }`. The other opaque min-call results in the
trace were not reachable from this final obligation. The box proof took
9.893s; total time was 259.27s wall, 250.43 user, 3.59 system, maximum RSS
1,491,222,528 bytes, zero swaps. Its files are `commuting-clamp.smt2` and
`commuting-diagnostic.log` in the same external directory.

The conditional recognizer now also covers `if x < c { c - x } else { 0 }`
and `if x > c { x - c } else { 0 }`, including inclusive guards. These
are positive-part clamps of checked differences. Original operands are
retained before branch narrowing; extending the subtraction outside its
branch requires checked arithmetic to prove it total over the whole box.
Otherwise refinement declines. The same focused edge, expanded for both
directions, negative constants and wrong-direction refusal, passed in
3.26s after a 27.11s incremental build. Original page 360 is still open
pending the next actual continuation; these diagnostics do not add coverage.

The positive-difference run proved the page's remaining salary regions:
local 58,689..58,851 / 25..72 and / 73..119, and a reusable
32,768..65,535 / 120..200 scope. The outer wall deadline stopped it before
the commuting side completed, so sequence 3561 and its counts remained
unchanged. Build 5m46s, preparation 80.277s; run 512.01s wall, 472.04 user,
9.85 system, maximum RSS 1,538,113,536 bytes, zero swaps. CPU pacing logged
one 1.123s pause; memory did not cause the stop. The external
`original-difference-clamp-*` files preserve this attempt.

The next actual-output continuation uses a 15-minute work window to allow
replay plus the difficult page to finish. This changes only the operational
epoch duration: the query, journal, CPU/RAM governor, solver timeout and
proof acceptance conditions remain unchanged. No extra build or test run is
needed for that change.

The 15-minute continuation committed **five new pages (360..364)**, adding
**327,680 candidates**. The original result is now **23,961,234 classified**
(14.9012%), comprising **23,901,428 admitted harmless transitions** and
**59,806 exclusions**; **136,839,168 candidates remain unclassified**.
Pages 0..364 plus 2453 are accounted for, with 2,099 regional leaves and
2,466 case-support records. The low prefix is `[0,23920640)`: all incomes
0..59,503, then income 59,504 / distances 0..15, for both interventions.

Page 360's 11-leaf certificate is
`9688b8d277aa09dbfb1dc5a46ce8875b20a29268753c47b2f0c56961ede5f140`.
It accounts for 65,373 harmless transitions and 163 exclusions. Proof quanta
were 302.620s for page 360, then 120.135s, 44.557s, 22.211s and 34.803s
for pages 361..364. There were no concrete sweeps or selected observer work
in the trace. New local obligations were re-derived during certificate
acceptance; a separate cold reopen is deferred to the next productive run.

The committed journal is sequence **3606**, **56 durable segments**, head
`4661c2b0a9b968e58810b7ec68afd603ffa1f7ebac1e0a312f71f2faef35fc5a`.
All 11 artifacts caught up; state/output are approximately 1.7/2.6 MiB.
Preparation took 73.029s; the invocation measured **931.92s wall, 873.58
user, 18.10 system**, maximum RSS **1,386,053,632 bytes**, zero swaps.
The outer deadline stopped page 365 with exit 1 and no uncommitted evidence
accepted. Its empty JSON report is not the authoritative result; the
manifest and `original-difference-clamp-15m-findings.md` under
`/tmp/futuruna-clamp-scope-output.qrI3OS/` retain the committed answer.

The next bottleneck is solver timeout cost at the 59,504..59,666-DKK /
25..119-km salary box, not disk pressure. The whole original answer remains
open; zero selected cases here is not a global no-cliff claim. The separate
176-cliff boundary audit remains separate. Formatting, diff checks and the
focused clamp edge passed; mint/deeper lanes remain deferred as requested.

A 4.4-MiB, recursively byte-compared checkpoint/output/experiment snapshot
is also retained outside temporary storage at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq3606.b9kjjb/`.
Its README identifies it as research/checkpoint data, not disposable debug
data. The working `/tmp` paths remain untouched; no cleanup was needed.

### Reducing repeated solver timeouts

A fresh diagnostic of the stalled 59,504..59,666-DKK / 25..119-km salary
box found a search-strategy bottleneck. Installed Z3 4.15.4's default
arithmetic engine returned Unknown after 10.04s. The **identical statement**
with `smt.arith.solver=2` returned **UNSAT in 0.14s**, using 27.5 MB maximum
RSS. Only the invocation strategy changed; the 10-second/128-MB statement
caps, signed integer arithmetic and strict proof acceptance stayed intact.
Wider freshly derived scopes of 32,768, 4,096 and 512 incomes still timed
out with both strategies. These diagnostics add no original-grid coverage.

The implementation attempts at most **one new shared scope per node**
before its exact local box. Cached
facts at all seven dyadic widths remain available. This is an operational
search change, not new query semantics or a new proof recipe: canonical
statement bytes/digests and recorded V7 scope replay are unchanged.
Scripts, measurements and readable findings are retained under
`/tmp/futuruna-frontier-solver.RbZkSU/`. The next productive continuation
will re-derive the saved receipts and measure actual coverage; the full
result remains open. No cleanup was needed.

The first actual continuation exposed a compatibility limitation in globally
replacing the solver strategy: an older saved obligation now timed out.
Replay stopped safely with `SelectionTruthVariesOverAxis` before accepting
new work. Sequence 3606, its head and all counts remained unchanged.
Preparation was 98.301s; the run ended after 122.71s wall, maximum RSS
1,291,845,632 bytes, zero swaps. CPU pacing accounted for 16.805s.

The revised policy preserves the original engine for legacy arithmetic.
Clamp-bearing traces may first try simplex for **at most one second**;
otherwise the original engine receives the identical statement, including
its unchanged 10-second/128-MB limits. Both attempts share the existing
12-second outer proof guard and run serially. Thus a fast clamp proof can
avoid a long timeout while saved proofs retain the original strategy.
This is same-statement solver scheduling, never a timeout-triggered switch
to a stronger semantic recipe. The focused arithmetic edge additionally
checks identical receipt digests across strategies and expired-guard refusal.

The corrected continuation replayed all saved receipts and committed
**page 365**, adding **65,536 candidates**. Its 10-leaf certificate is
`497d3c23fbcd4126e50098fcabe0a5cb771cdbc075fcf4841c7e229adb82218a`:
65,373 admitted harmless and 163 excluded transitions. That proof quantum
took 137.612s. The original answer now accounts for **24,026,770 candidates
(14.9420%)**: **23,966,801 admitted harmless**, **59,969 excluded** and
**136,773,632 still unclassified**. Zero selected remains only a lower
bound, not a global no-cliff result. The separate 176-cliff audit is unchanged.

The low prefix is `[0,23986176)`: all incomes 0..59,666, then income 59,667
at distances 0..20 for both interventions. Pages 0..365 plus the preserved
upper-endpoint page 2453 comprise 367 pages, 2,109 regional leaves and
2,477 case-support records. Independent chunk-row sums match the manifest.

The run paused normally (exit 0, `runtime_limit`) at **sequence 3624**,
**58 segments**, head
`9936b197c42d7388067c4e29ef44ce30788e89477435258ba304b0f1fadafce0`.
All 11 artifacts caught up. Its nonempty JSON report is
`/tmp/futuruna-frontier-solver.RbZkSU/original-prefix-15m-report.json`.
Preparation took 89.686s; total **921.74s wall, 834.15s user, 16.86s system**,
maximum RSS **1,474,166,784 bytes**, zero swaps. CPU pacing paused 31.365s.

The next page, 366, made exactly 31 cover probes before exhausting the
current bounded tree. It then saved one concrete classification in a
partial slice; this does **not** increase published support coverage yet.
The combined failed-cover/one-member quantum took 395.163s. The existing
driver can reconcile such a partial slice with a fresh checked cover on
resume, so no checkpoint reset is needed. The next task is a resource-safe
cover-budget improvement, not another blind full-grid run. This measurement
clears the previous stall but does not establish a broad throughput speedup.

The revised arithmetic edge passed in 0.16s; the scoped replay edge passed
in 1.72s. Formatting/diff checks and the optimized build passed (6m13s,
existing unrelated warning only). Mint/deeper lanes remain deferred under
the output-first instruction. State/output are approximately 1.8/2.6 MiB;
a byte-compared snapshot plus these measurements is retained at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq3624.it0NQt/`.
No files were deleted; about 47 GiB remained free.

### Letting a bounded cover finish after a concrete prefix

The page-366 trace ended after exactly 31 probes, with further commuting and
outward-edge branches still pending. The producer and journal decoder now
share a **63-node limit**: at most 32 leaves in a full binary cover. The
search remains finite and bounded; source domains, node semantics, solver
statements, acceptance checks and CPU/RAM limits have not changed.

This is an Experimental artifact-capacity extension, not a new query or a
new proof recipe. Existing V4/V5/V6/V7 receipts retain their identities and
replay paths. The V6/V7 encoding grammar is unchanged, but an older CLI with
the 31-node decoder limit will correctly reject newly written larger covers;
use the updated CLI when resuming them.

The focused
`expanded_cover_reconciles_partial_slice_and_cold_replays_at_bound` test
passed in **0.15s**, following a 13.14s incremental test build. A 288-case
fixture exercises its first 256-case page with exactly **63 nodes / 32
leaves**, 128 harmless cases and 128 exclusions. It preserves one concrete
member, reconciles it through the fresh cover, encodes and decodes the real
journal, cold-replays with a fresh producer, and rejects a 64-node cover.
Formatting and diff checks passed; mint/deeper suites remain deferred under
the user's output-first instruction.

The next actual continuation keeps the original sequence-3624 checkpoint,
including its partial slice. A 30-minute operational window amortizes the
observed roughly six-minute preparation/replay cost. It does not increase
CPU/RAM limits or the per-obligation solver caps. Results will be retained
under `/tmp/futuruna-cover63-output.gxRFa9/`; no additional original-grid
coverage is implied by the focused fixture or this scheduling change.

The actual continuation reconciled the saved one-member slice and committed
**pages 366..371**, adding **393,216 candidates** through 82 checked leaves:
392,238 admitted harmless and 978 excluded transitions. Page 366 needed
17 leaves (33 nodes), confirming that the former 31-node cap was too small.
The six proof quanta took 463.973, 49.627, 102.211, 43.301, 370.561 and
337.745 seconds respectively. These differing costs are measured local
progress, not a whole-grid throughput guarantee.

The original answer now accounts for **24,419,986 candidates (15.1865%)**:
**24,359,039 admitted harmless**, **60,947 excluded** and **136,380,416
still unclassified**. Zero selected remains only a lower bound. The separate
176-cliff boundary audit is not counted here. Independent chunk-row sums
match the manifest: 373 pages (0..371 and 2453), 2,191 regional leaves and
2,565 case-support records. The low covered prefix is `[0,24379392)`:
all incomes 0..60,644, then income 60,645 at distances 0..50 for both
interventions. The upper-endpoint certificate remains unchanged.

The outer wall deadline stopped the unfinished page-372 proof (exit 1);
the empty JSON report adds no evidence. The committed answer is intact at
**sequence 3671**, **64 durable segments**, head
`a3cde82250495d0cb8b35c5c48a1e8020dd361c22b1736fc3e93d15353160029`.
Later scheduler events seen in the trace were not durably committed. All
11 published artifacts caught up to the committed prefix. Preparation took
81.821s; the invocation measured **1,832.89s wall, 1,751.88s user, 35.00s
system**, maximum RSS **1,304,969,216 bytes**, zero swaps. The stop was a
time limit, not memory pressure. Query semantics, proof statements, CPU/RAM
limits and per-obligation solver caps stayed unchanged.

State/output are approximately 1.8/2.7 MiB. A recursively byte-compared
snapshot with logs and exact commands is retained outside temporary storage
at `/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq3671.hycPFk/`.
This is research/checkpoint data, not disposable debug data. No files were
deleted; about 47 GiB remained free. The focused edge, formatting, diff check
and optimized build passed; mint/deeper lanes remain deferred as requested.
The full original answer remains open.

### Native fallback and a wider arithmetic proof

A 45-minute continuation restored the existing default compiled residual
classifier, without changing the original query or checkpoint. Preparation
took 77.145s and the compiler-hash-keyed native setup took 209.381s. Saved
receipts, including the larger covers, cold-replayed successfully. The run
then committed **pages 372..377**, adding **393,216 candidates** through
90 checked leaves: 392,238 harmless and 978 excluded. All six pages closed
through proofs; no new concrete classification was needed. Native residual
throughput therefore remains unmeasured, and proof time is still the observed
bottleneck on this frontier.

The committed original answer is **24,813,202 classified (15.4311%)**:
**24,751,277 harmless**, **61,925 excluded**, **135,987,200 unclassified**.
Zero selected is only a lower bound; the separate 176-cliff audit is unchanged
and not included in these counts. Independent chunk sums match the manifest:
379 pages (0..377 and 2453), 2,281 regional leaves, 2,661 records. The low
covered prefix `[0,24772608)` includes all incomes 0..61,622, then income
61,623 at distances 0..80 for both interventions.

The outer wall deadline stopped unfinished page 378 (exit 1). Its uncommitted
work is not accepted evidence, and the empty JSON report is not the answer.
The durable result is **sequence 3725**, **70 segments**, head
`922e030be13aea315f4facd35ebb6ec1fb63b350ac1614666a90d40f69f6acd4`.
All 11 artifacts caught up. Total **2,731.45s wall, 2,666.33s user, 64.25s
system**, maximum RSS **1,425,752,064 bytes**, zero swaps; CPU pacing paused
4.507s. No CPU/RAM or solver caps were raised. A byte-compared stable snapshot
with exact commands and logs is retained at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq3725.k7vNU4/`.
No files were deleted; about 47 GiB remained free.

After that worker stopped, one targeted arithmetic diagnostic found a more
promising route than faster pointwise evaluation. The retained
32,768..65,535-DKK / 25..119-km salary obligation previously timed out after
10 seconds with both arithmetic engines. Adding general order/difference
lemmas for its exact integer divisions and clamps yielded **UNSAT in 0.03s**,
using 28,590,080 bytes maximum RSS, with the same statement limits. These
facts expose the coupled endpoint changes: when rounded AM contribution rises
by one whole krone, post-AM income rises by zero. Bounding those changes
independently loses that useful relationship.

The diagnostic preserves the original problem and adds 144 entailed facts
over 72 same-operation/parameter pairs; it does not add model assumptions or
source-axis bounds. A focused counterexample search over unbounded signed
inputs, arbitrary clamp threshold, both min/max and divisors 1, 2, 100 and
10,000 also returned UNSAT in 0.05s. Sign crossings are included; truncation
toward zero is not incorrectly treated as floor division.

This is **not yet an engine change or new grid coverage**. The next task is
bounded, checked-DAG-derived preprocessing that preserves the original
obligation, old receipt identities and solver fallback, followed by the
closest permanent edge and a productive same-checkpoint run. Exact commands,
scripts, results and caveats are in
`/tmp/futuruna-hybrid-cover-output.8DBiKZ/findings.md`. No compiler rebuild or
broad test lane was added; mint/deeper gates remain deferred as requested.

### Checked paired-integer preprocessing

The arithmetic emitter now derives those order/difference facts directly from
its checked DAG, alongside the immutable original obligation. It pairs only
positive constant divisions or clamps with exactly the same kind and threshold.
The facts hold for signed inputs, including sign crossings; opaque values and
branch-local interval bounds supply no extra assumptions. Generation is bounded
by 128 candidate operations, 256 matched pairs and 1 MiB total input, with the
existing 4,096-node reachable-DAG limit. Exceeding a limit declines augmentation,
not the original proof attempt.

The old one-second simplex prefix runs first. If unresolved, the paired attempt
may use only the remaining time through 1.5 seconds; the original default
engine retains its ten-second opportunity inside the twelve-second outer
guard. Attempts remain serial and retain the 128-MB statement cap. The original
statement bytes and receipt digest are unchanged: fresh augmented UNSAT proves
that statement because every added fact follows from its own definitions.
This is equivalent preprocessing, not a new semantic precision or receipt
format. Legacy non-clamp arithmetic follows its unchanged solver path.

The two permanent `paired_integer_` edges passed in 0.17s, covering signed
validity, preserved SAT witnesses, generation limits, canonical identity and
fresh-DAG receipt replay. Formatting/diff checks and the optimized build passed
(346.39s wall, 2,081,390,592 bytes maximum RSS, zero swaps). Mint/deeper lanes
remain deferred under the user's output-first instruction. The 30-minute
continuation used the same original sequence-3725 checkpoint, with native
residual setup disabled for this proof-focused measurement.

The actual run cold-replayed the saved receipts and committed **50 new pages
(378..427)** through **495 checked leaves**. It added **3,276,800 candidates**:
**3,268,649 harmless** and **8,151 excluded**. No concrete sweeps or selected
observer work occurred. Fresh salary scopes covered 32,768..65,535 and
65,536..98,303 DKK across all 201 distances; only the exact accepted page
members are counted, not every member of those wider scopes.

The original answer now accounts for **28,090,002 candidates (17.4689%)**:
**28,019,926 harmless**, **70,076 excluded**, **132,710,400 unclassified**.
Zero selected remains a lower bound, not a global no-cliff result. The separate
176-cliff boundary audit is unchanged and not included in these counts.
Independent chunk sums match the manifest: 429 pages (0..427 and 2453),
2,776 regional leaves and 3,206 case-support records. The low prefix
`[0,28049408)` covers all incomes 0..69,773, then income 69,774 at distances
0..129 for both interventions. The upper-endpoint certificate is unchanged.

The run paused normally with **exit 0 / `runtime_limit`** at **sequence 4175**,
**107 segments**, head
`0eb311caaf63653059b365eac564791296d47368e184ab16624993aa70fbf1ef`.
All 11 artifacts caught up. Preparation took 78.432s; total **1,808.23s wall,
1,704.85s user, 50.61s system**, maximum RSS **1,435,058,176 bytes**, zero
swaps. CPU pacing paused 1.115s. The 50 proof quanta totalled 1,192.441s,
with median 24.931s and range 12.336..135.285s. This is measured local
progress, not a whole-grid completion forecast.

State/output are approximately 2.1/4.0 MiB. A recursively byte-compared
snapshot with exact commands, JSON report and logs is retained at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq4175.zHJ8qR/`.
No files were deleted; about 47 GiB remained free. A remaining cost is repeated
local commute probes whose smaller scopes are already proved. Reusing those
checked partitions is a next investigation, not authority to skip an unproved
region. The full original goal and the deferred mint/deeper gates remain open.

### Assembling pages from already-proved regions

The cover producer now tries one cache-only geometry pass before repeating
local proof search. Its input consists only of immutable scope/proof pairs
from the current checked producer. Every accepted leaf must fit completely
inside a rejected or harmless theorem's scope. Cached boundaries may suggest
splits, but cannot classify a parent or fill a gap. Exact ranked intersections
recompute the page's members and counts, including clipped fringes.

The pass is bounded to 64 cached scopes and the existing 63-node/32-leaf cover
limit, without backtracking or new solver calls. If the cached pieces do not
close the entire page, this pass is discarded and the existing prover keeps
its original opportunity. It does not change the query, model, source bounds,
resource limits, V4/V5/V6/V7 grammar or canonical statement identities. A cold
reopen still re-derives the recorded scopes before they can supply authority.

The new cache-geometry edge and existing V7 codec/cold-replay edge passed in
2.05s; formatting, diff checks and the optimized build passed. Mint/deeper
suites remain deferred as requested. The same-checkpoint 45-minute run
cold-replayed the sequence-4175 receipts and committed **585 new pages
(428..1012)** through **7,404 checked leaves**. It added **38,338,560
candidates**: **38,243,190 harmless** and **95,370 excluded**. No concrete
sweeps or selected observer work occurred; default native fallback was ready
but not needed for the committed pages.

Of those pages, **543 used the cache-only pass**. Their classification quanta
totalled **1.412s**, ranging from 1 to 6 milliseconds per page. The remaining
42 pages required ordinary proof work, totalling **1,759.431s**, with a range
of 14.944..209.022s. These timings exclude startup, old-proof replay and
publication. They demonstrate cheap reuse of proved regions, not a promise
that fresh boundaries or the whole remaining grid will be equally cheap.

Independent chunk sums match the terminal manifest: **66,428,562 classified**
= **66,263,116 harmless** + **165,446 excluded**, with **94,371,840 still
unclassified**. Zero selected is still a lower bound. The separate completed
176-cliff boundary audit is unchanged and not included. The original graph
contains 1,014 pages (0..1012 and 2453), 10,180 leaves and 11,195 records.
Its gap-free low prefix `[0,66387968)` covers all incomes 0..165,143, then
income 165,144 at distances 0..39 for both interventions. The upper-endpoint
certificate remains unchanged.

The worker reached its **outer wall deadline**, exiting **1** after
**2,732.96s wall, 2,632.41s user and 78.93s system**. The final JSON report is
empty; the durable manifest is the result source. Saved **sequence 9440** has
**149 segments**, head
`b905d3b7a7a540ff59109e9714031d1b5f016e66949ed54fcad9fcb832a036c0`;
all 11 artifacts are caught up. The later in-memory sequence 9447 and
unfinished page 1013 are not counted. Maximum RSS was **1,493,417,984 bytes**,
zero swaps; CPU pacing paused 11.284s. At containment, the process group used
554,663,936 bytes and host available memory was 1,960,853,504 bytes, above the
unchanged 1-GiB guard floor. No resource limit was relaxed.

All workers stopped before a recursively byte-compared snapshot was saved at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq9440.1mOfZL/`.
It contains state, published output, commands, logs and the empty report.
State/output occupy about 4.7/11.5 MiB. No files were deleted; about 47 GiB
remained free. The next productive resume must recover and cold-replay this
checkpoint, then continue page 1013. Its repeated local commute probes around
25..118 and 72..118 km are measured remaining costs; their smaller proved
children do not authorize skipping the parent. Full-grid closure and the
deferred broader validation gates remain open.

### Continuation to the next salary frontier

The next run used the same source, query, binary and sequence-9440 checkpoint,
with a 60-minute cap and unchanged CPU/RAM/solver limits. Preparation took
76.178s; the native fallback reused its cached executable in 0.232s, without
compilation. By the observation at 17m11s, all saved receipts had cold-replayed,
including the new cached-cover recipes. The interrupted page-1012 completion
bookkeeping recovered and new work resumed at page 1013. No verification-only
epoch, rebuild or additional test suite was run.

It committed **595 new pages (1013..1607)** through **8,921 checked leaves**:
**38,993,920 candidates**, comprising **38,896,920 harmless** and **97,000
excluded**. **591 pages** used cache-only covers, totalling **1.828s** of
classification work (2..6ms each). Four fresh closures took **526.890s**
(97.419..157.761s each). These timings exclude preparation, cold replay,
publication and the later unsuccessful proof search.

Independent chunk sums agree with the final manifest: **105,422,482
classified (65.5611%)** = **105,160,036 harmless** + **262,446 excluded**.
**55,377,920 remain unclassified**; zero selected is still only a lower bound.
The separate completed 176-cliff boundary audit is unchanged and not counted
here. The original graph contains 1,609 pages (0..1607 and 2453), 19,101
regional leaves and 20,711 records. Its gap-free low prefix `[0,105381888)`
covers every starting income **0..262,143 DKK**, every declared distance and
both interventions, including the outward exclusions. The existing
upper-endpoint certificate remains unchanged.

The next page, **1608**, exhausted the 63-node cover budget while salary
proofs repeatedly returned `Unknown`, even after distance and income splits.
Its final `ClassifiedSweep` quantum took **1,525.887s** and included only
**one concrete member** after the unsuccessful cover attempt. The recorded
slice has no completed page artifact, so that member is not added to the
published closure counts above. No selected observer work occurred. The
262,144-DKK boundary is an operational scope boundary, not a discovered tax
cliff; a timeout is neither a no-cliff proof nor a cliff witness.

The run paused with **exit 0 / `runtime_limit`** after **3,083.69s wall,
2,965.89s user and 54.28s system**, below its 60-minute cap. Maximum RSS was
**1,463,894,016 bytes**, zero swaps; CPU pacing paused 5.677s. The JSON report
and manifest agree on **sequence 14804**, **154 segments**, head
`0808deebffa3b4653a63d7823e90fa54d35d4199670dd3c66c3b2f212844a1cc`.
All 11 artifacts are caught up, and all workers stopped before preservation.

A recursively byte-compared snapshot, including the one-member resume slice,
state, output, exact command, report and trace, is retained at
`/Users/andreasrudolph/futuruna-explore-checkpoints/unit-grid-seq14804.nKJXxK/`.
State/output occupy about 7.4/21.2 MiB; approximately 47 GiB remained free.
Nothing was deleted. The next useful step is to inspect an exact failing
salary obligation and a nearby passing child with the already-built bounded
diagnostic, before another long run. A read-only inspection also found a
literal-only min/max recognizer limitation, but that is not yet the diagnosed
cause of these salary timeouts. No speculative precision or model change has
been made; mint/deeper gates remain deferred under the output-first request.
