# Income cliffs across income and commuting distance

The full-domain query declaration is
[`personskat-income-distance-unit.explore.runa`](personskat-income-distance-unit.explore.runa).
It uses the canonical Personskat calculation, not a simplified tax formula.
It passes frontend checking and now has a regression proving endpoint totality
across the full declared ranges, including mechanisms. The full-model query
now executes governed epochs and writes a durable classified prefix. Exact
full-grid closure remains unfinished. Explore remains **Experimental**.

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
