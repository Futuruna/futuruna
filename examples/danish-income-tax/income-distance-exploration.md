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

Two practical bounds are now explicit:

- Advisory slab nomination stops after 16,384 attempts. Unnominated chunks
  remain in the exact residual schedule.
- The eager 256-rank partition accelerator stops before allocating more than
  65,536 descriptors. The full grid would need 628,127. This keeps its descriptor
  payload below 8 MiB within the default 16-MiB journal-entry limit; larger
  searches retain the ordinary bounded concrete path.

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

For the next *useful cliff search*, replace point-level full-grid accounting
with compact batching and lazy/hierarchical regional partitions, then finish
proof lowering for the actual rule families. Then let checked
boundaries prioritize unit-resolution neighborhoods in **both** dimensions,
and discharge the rest with regional proofs or concrete evaluation. Coarse
scans can inform the order but cannot replace the unit-edge coverage obligation.

## Delivered boundary and remaining work

The current product prover handles constructors, projections, typed source and
context bindings, acyclic pure calls, checked affine arithmetic, supported
integer division and decisive Boolean branches. The canonical Personskat graph
still contains rule-family dispatch and collection/rounding behavior outside
that proof fragment. Such unsupported optimizations must leave concrete
residuals. The original endpoint-totality refusal has now been removed by
supporting bounded `flat_map` summaries; a permanent test proves both endpoint
roles over the unchanged full canonical query. The first real-model epoch has
now classified cases, but this does **not** establish practical full-grid closure.

The outstanding work is tracked explicitly:

- `td-f699c8`: bounded finite-list callback proofs for canonical endpoint
  preparation; implemented and verified through real execution and cold resume.
- `td-7ba30c`: paged/hierarchical product partitions with bounded retained state.
- `td-966941`: checked rule-dispatch and required collection/rounding proof
  lowering, followed by measured canonical Personskat regional closure.

The broader Explore feature is not finished by this slice. The finish line is
an exact, resumable unit-grid answer with every exclusion accounted for,
replay-derived mechanisms for findings, and measured operation within the host
resource policy—not merely a small number of discovered mechanisms.

This changes Experimental optimization behavior and artifacts, not core source
syntax or tax semantics. Regional proof schema is now 4, scheduler policy 4 and
journal schema 29 / codec 24. Old codec-23 journals are rejected explicitly; keep their
artifacts for historical evidence and start fresh state with this compiler.

## Verification

Permanent tests compare both intervention directions against an independently
enumerated Cartesian oracle, including boundary rejections, isolated one-unit
losses and integer-rounding cliffs. A uniform affine grid closes all 800 cases
using four regional certificates rather than point classification. Candidate
and canonical schedules preserve exact finding identities and evidence roots;
product prefixes are encoded, cold-replayed and resumed. Geometry tests cover
equal-size axes, nonzero starts, intersected slabs, nomination caps and the
full-grid eager-metadata refusal. Forged coordinate-kind certificates cannot
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
  without appending semantic events. Existing codec-24 state remains readable.

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

This prefix retained about **174 MiB** of run state. The eager-partition cap
currently sends this large grid through point-level records and one-subject
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
