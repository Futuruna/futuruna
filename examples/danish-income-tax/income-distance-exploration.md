# Income cliffs across income and commuting distance

[`personskat-income-distance-unit.explore.runa`](personskat-income-distance-unit.explore.runa)
asks whether a one-krone pay increase or a one-kilometre commute increase can
reduce annual salary minus tax. It uses the canonical Personskat model, not a
simplified tax formula or an LLM evaluator.

Explore is [Experimental](../../docs/feature-stages.md). The recorded answer
below belongs to its preserved model, executable and checkpoint; it does not
certify later revisions or arbitrary tax reports. For tax-report intake, start
with [Danish tax-audit readiness](../../docs/tax-audit-readiness.md). For broader
query design, use the [exploration workbook](exploration-workbook.md).

## Recorded full-grid answer

The run completed on **2026-09-15**. Its exact domain is annual salary
**0–400,000 DKK** and daily round-trip commuting distance **0–200 km**,
inclusive, with separate **+1 DKK** and
**+1 km** interventions. All other inputs remain explicitly conditioned below.

| Classification | Candidates |
| --- | ---: |
| Harmless | 160,391,400 |
| Income-loss cases | 8,800 |
| Excluded outward edges | 400,202 |
| Unknown | 0 |
| Total | 160,800,402 |

There are **50 salary boundaries**, from **342,499 → 342,500** through
**391,499 → 391,500 DKK** at 1,000-DKK intervals. Each has 176 losing distances,
25–200 km. Annual losses range from **1.87 to 144.09 DKK**. No distance-only
loss occurs in this encoded model and fixed context; travel costs/time are not
part of the metric. The 50/100/150-km profiles detect every cliff boundary but
miss the exact maximum at 26 of the 50 boundaries; adding 200 km still misses
21 maxima.

The final accounting covers 2,454 disjoint chunks and 63,128 regions.
Regional certificates cover 157,523,602 candidates; concrete sweeps cover
3,276,800. Every chunk's exclusions reconcile to the declared outward edges:
201 salary edges and 400,001 distance edges, with no additional in-bounds
exclusions. All 8,800 findings join successful canonical mechanism incidences
and source-derived arithmetic explanations, with zero unexplained cases and
zero øre residual. The explanation reduction is hand-transcribed deterministic
BigInt arithmetic, not an independent causal proof or pruning authority.

Completed checkpoint **276485 / 8,317 segments** and its final report,
executable, publication cursor, full journal, output and audit records are
preserved locally in
`/Users/andreasrudolph/futuruna-explore-checkpoints/complete-unit-grid-seq276485.q7IKqH/`.
Start with `result.md`, `boundary-comparison.md` and `restoration.md` there.
At completion, all eleven artifacts were caught up; byte-equal copy comparisons, whole-artifact
commitment checks, physical journal hash/chain checks, and finding/region/
exclusion/support/explanation joins passed. The final invocation exited 0.
These are the recorded checks for that run, not checks of later model or
compiler revisions.

Recovery explicitly trusted checkpoint 258991: 2,404 regional proof events and,
with the separate result-row option, 26,400 saved records eligible for row-local
reuse. This inherits earlier regional assumptions and is not independent
mathematical re-verification; new work retained normal checks. Journal recovery
took 45m59.379s, while final byte-integrity checks took seconds. True state
snapshot/tail-only restoration remains unimplemented. That run did not pass
the deferred broad compiler/release gates tracked under `td-511340`.

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

The query expresses the exact product:

```runa
vary bruttoløn_kroner in range(0, 400001)
vary afstand_km in range(0, 201)
vary retning in range(0, 2)
```

The intervention is `(income_delta, distance_delta) = (1 - retning, retning)`.
It never changes both dimensions at once. The query rejects successors outside
the endpoint bounds and retains canonical model validity at both endpoints.
When changing a maximum, update both its end-exclusive source bound and its
inclusive successor-admission bound.

The query fixes Copenhagen, no church tax, birth date 1990-01-01, adult
allowance status, 203 workdays and a single wage earner. Other income, pensions,
capital, property, spouse and special-regime inputs are explicitly empty through
the shared adapter. Work travel has no employer-paid travel or reimbursement.
These are assumptions defining one profile, not facts inferred about a user
or a representative sample of all taxpayers.

| Population | Exact size before model-validity exclusions |
|---|---:|
| Raw directed candidates | 160,800,402 |
| Outward-pointing boundary edges | 400,202 |
| In-bounds salary edges | 80,400,000 |
| In-bounds distance edges | 80,000,200 |
| All in-bounds edges | 160,400,200 |

The two finding summaries group by intervention. A salary cliff and a
distance-induced loss must not become an indistinguishable aggregate.

## Understanding the earliest cliff

For the fixed profile above, a raise from **342,499 to 342,500 DKK** reduces
annual salary minus final tax at every integer daily round-trip distance from
**25 through 200 km**. These are the recorded full-grid run's own 176 cases.

| Daily round-trip commute | Annual loss from the 1-DKK raise |
|---|---:|
| 25 km | 2.11 DKK |
| 50 km | 50.06 DKK |
| 75 km | 98.24 DKK |
| 100 km | 144.08 DKK |
| 150 km | 144.08 DKK |
| 200 km | 144.09 DKK |

The maximum **at this boundary** is 144.09 DKK, attained at 18 distances,
first at 99 km.

The [encoded low-income supplement](ligningsloven_fradrag.runa) uses whole-
thousand income phase-out steps. At this boundary the excess over 341,500 DKK
changes from 999 to 1,000 DKK: the supplement falls from 64% to 62.72%, and
its cap from 30,800 to 30,184 DKK. The resulting tax increase outweighs the
extra krone. Longer commutes increase the underlying deduction until the cap
binds; integer rounding retains the small differences along the loss plateau.
This explains the run's encoded research model, not an independently
validated interpretation of current law or individual tax advice.

In the recorded run, all 176 mechanism replays succeeded, with zero unavailable
explanations.
Their four structural groups cover **25–98 km (74 cases)**, **99–119 km (21)**,
**120 km (1)** and **121–200 km (80)**. Every group has zero before/after
differential nodes and edges despite these numeric losses. A shared execution
path is therefore not authority to close an unseen income or distance range.

## What can safely close a route?

A repeated execution mechanism is a useful explanation and scheduling hint.
It is **not** proof that unseen values cannot contain a cliff: arithmetic,
rounding, eligibility or another rule can change the outcome inside the same
apparent path.

The implementation separates three operations:

1. Checked source boundaries nominate promising regions. Nomination changes
   scheduling, not whether an edge is admitted, harmless or selected.
2. A regional proof must establish its conclusion over the whole claimed
   region. Ranked covers preserve exact geometry and account separately for
   admitted harmless edges, exclusions and unresolved work.
3. Anything unproved remains residual work. An uncertain branch, unsupported
   operation, possible overflow or loose rounding bound cannot become a
   no-cliff conclusion. Findings receive concrete case identities and mechanism
   replay.

Certificates bind the checked program and query to the covered region.
Strict recovery checks the saved proof obligations as well as byte integrity.
A stored digest is not a mathematical proof; the explicit assumption mode
below is a different trust choice.

Implementation and permanent regressions live together in
[product regions](../../src/explore/relational_product_region.rs),
[region covers](../../src/explore/relational_region_cover.rs),
[regional proofs](../../src/explore/relational_region_proof.rs) and
[durable journals](../../src/explore/relational_durable_journal.rs).
Read those current sources for supported proof fragments and capacity limits,
not an old experiment's schema numbers or test transcript.

## Run a bounded exploration

On the development machine with 8 GiB RAM and six CPU cores, retain the
resource governor and use one build job if a build is necessary. Preparation,
native compilation, recovery and publication also consume resources; a short
epoch may finish without publishing any classifications.

For a small engine demonstration, use
[`relational-explore-income-distance.runa`](../relational-explore-income-distance.runa)
with `--query income_distance_demo`. Its allowance rule is synthetic, not a
tax approximation. A narrower canonical example is
[`personskat-income-distance-boundary.explore.runa`](personskat-income-distance-boundary.explore.runa);
neither closes the full grid.

For the full query, replace the paths below with distinct writable private
directories and use an existing compatible compiler:

```sh
./target/release/runa explore \
  examples/danish-income-tax/personskat-income-distance-unit.explore.runa \
  --query personskat_income_distance_unit_2026 \
  --run-state /path/to/private/unit-grid.run \
  --output /path/to/private/unit-grid.result \
  --time-limit 3m --json
```

Repeat the same command to resume. A changed query or incompatible compiler
identity requires separate state; do not rewrite a saved identity or overwrite
the preserved completed checkpoint. A paused empty finding set is not an
exact-empty answer. Check each count's status, admission exclusions, findings,
mechanisms and publication closure.

Concrete work is batched and journaled in bounded slices. Do not extrapolate a
small synthetic example's speed to the canonical model or assume that a faster
hash check removes journal reconstruction and calculation costs.

### macOS boot identity

The [sampler](../../src/explore/resource_sampler.rs) uses
`kern.bootsessionuuid`, not a wall-clock boot timestamp, to identify one boot.
It brackets observations with that identity and refuses mismatches. Real
generation changes and decreasing cumulative swap counters retain the
governor's backoff/error behavior. A resource pause is not a tax finding.

### Opt-in bounded native batches

`FUTURUNA_EXPLORE_NATIVE_WORKERS=2` requests two native classification
subprocesses inside one admitted work quantum. The default, unset or `1`,
is serial. This is an Experimental operational option, not a measured speedup
promise or permission to bypass the host reserves.

Two-process mode requires supervised containment and its larger resource
reservation. If the governor cannot admit it, work pauses. The host validates
the input and both responses before accepting the batch; a failed batch falls
back to checked execution. First-batch parity and finding/mechanism replay
remain required. See the
[epoch configuration](../../src/explore/relational_public.rs),
[native classifier](../../src/explore/relational_native_classifier.rs) and
[resource governor](../../src/explore/resource_governor.rs) for the current
settings and limits.

## Explicit trusted-checkpoint recovery

For a previously verified local run, an operator may explicitly accept the old
regional conclusions instead of repeating their mathematical proof searches:

```bash
runa explore examples/danish-income-tax/personskat-income-distance-unit.explore.runa \
  --query personskat_income_distance_unit_2026 \
  --run-state /path/to/existing-state --output /path/to/existing-output \
  --assume-verified-checkpoint NEXT_SEQUENCE:LOWERCASE_SHA256 \
  --time-limit 10m --json
```

Replace the anchor with an independently retained checkpoint's `next_sequence`
and journal head, not a digest blindly obtained from an untrusted directory.
The sequence must be positive and the head exactly 64 lowercase hexadecimal
digits. This option cannot create a run: the anchor must match an installed
segment boundary in the existing, hash-validated journal. Program, query,
analysis, event-chain and cover geometry checks remain mandatory.

Only recorded region-cover conclusions inside that prefix are assumed. Any
newer journal tail, other proof recipes, and all new work retain their normal
checks. Assumed covers do not populate reusable proof caches. Recovery still
reads and folds the journal; this is not a constant-time snapshot restore or
a persisted independently checkable solver proof.

This is an explicit trust tradeoff, not independent re-verification. The CLI
warns at startup, the human report names the assumption, JSON reports include
`run.recovery_assumption`, and the publication manifest includes
`recovery_assumption`. These record the pinned prefix and number of regional
proof events assumed. Preserve that provenance with exported results. Without
the option, recovery remains strict; a later successful strict recovery can
publish a report without the assumption.

For an additional, separate trust decision, add `--assume-verified-result-rows`
alongside that checkpoint pin. This allows row-local result publication to
reuse the recorded evaluated values inside the authenticated prefix rather
than evaluating the same expressions again after their warm receipts have
expired. The existing checkpoint option alone remains regional-only. Missing
or mismatched pins fail closed; unpinned/new rows retain ordinary evaluation.
Typed evidence, selected-case membership, existing projection consistency and
all byte/model/query identity checks remain mandatory. Grouped, Choice and
other non-row-local result execution paths retain their existing checks.

The CLI warns about this extra scope, and reports/manifests add
`recovery_assumption.row_local_result_records_trusted`. This is the number of
pinned evidence records authorized for row-local reuse, not the number of
evaluations actually skipped; a record may belong to another execution path
or already be published. This process-local index retains only content IDs,
never duplicates row payloads, never enters the ordinary verified-receipt
cache, and is not extended by new work. The saved values are explicitly
trusted, not independently reverified. This still does not provide snapshot
or tail-only restoration.

## Interpreting and reproducing the answer

The completed grid is exhaustive only over its declared profile and preserved
encoded model, with its disclosed recovery assumptions. Mechanism explanations
do not independently establish that the law was modeled correctly. Distances
are not travel times, and the metric does not include transport costs.

Keep the query, imported source revision, executable, state, output and
recovery provenance together. A later tax-model change calls for a separately
identified run; do not relabel the saved result as a current-model validation.

The source regressions exercise exhaustive small-product oracles, unit losses,
rounding, exclusions, exact region accounting, forged certificates and cold
recovery. They are model/engine tests, not independent validation of a personal
årsopgørelse. Follow [CONTRIBUTING.md](../../CONTRIBUTING.md) for proportional
checks when changing implementation.
