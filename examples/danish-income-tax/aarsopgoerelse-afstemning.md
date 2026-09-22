# Review a report without the spouse's tax documents

You can check an årsopgørelse without reconstructing the spouse's affairs.
The question is conditional: **what would have to be true for these reported
amounts to fit together?**

[`aarsopgoerelse-afstemning.calculate.runa`](aarsopgoerelse-afstemning.calculate.runa)
exposes `afstem_årsopgørelse`. It uses exact integer arithmetic and the existing
year-specific tax parameters. No LLM evaluates the rules. A human or assistant
transcribes the report; Futuruna calculates the checks and necessary conditions.
This research workflow uses the Preview typed-calculation interface.

## Two distinct kinds of review

| Review | Inputs | What the result establishes |
| --- | --- | --- |
| Independent calculation, `beregn_personskat` | Supported source facts, including relevant spouse facts | What the encoded rules calculate from those facts |
| Conditional reconciliation, `afstem_årsopgørelse` | Observed report lines and explicitly known conditions | Whether the selected arithmetic agrees, what transfers are necessary, and whether checked bounds contradict them |

Never feed the reconciliation's inferred amounts into `PersonskatInput` as
verified facts. Matching the report this way would be circular. Necessary
conditions are not a proof that a legally valid household with those facts
exists, and neither workflow certifies the underlying documents.

## Run the compact review

First run the [tax-audit runtime check](../../website/public/ai-setup.md#tax-audit-runtime-check)
against the compiler you will use. The original `v0.2.0` download predates
required calculation-safety fixes; the version string alone is insufficient.
Do not continue on a failed check. It uses only synthetic data, not your report.

Keep the PDFs, transcriptions, and results in a private directory **outside the
checkout**. Replace `PRIVATE_WORK_DIR` below with its actual path, and keep
`RUNA_BIN` set to the exact absolute path that passed the check. From the repo:

```sh
"$RUNA_BIN" template examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --format json --output PRIVATE_WORK_DIR/report-cases.json
# Fill the generated cases with report observations, preserving $futuruna.
"$RUNA_BIN" call examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --input PRIVATE_WORK_DIR/report-cases.json --output PRIVATE_WORK_DIR/report-results.json
```

The same contract supports `--format xlsx` and XLSX invocation. The optional
income bridge remains one canonical JSON field in that format. Field metadata
explains the units, signs, and completeness questions. Generated template values
are placeholders, not facts: use `null` for unknown optional values and `false`
for unconfirmed completeness. A known zero is different from an unknown value.

Transcribe only these selected observations:

- The income bridge in **whole DKK**: personal income + net capital income −
  ordinary deductions + other known income adjustments − reported taxable
  income. The residual is the required spouse loss deduction. If any bridge
  component is unknown, leave the whole bridge `null`; the printed spouse-loss
  line may independently be `null`.
- All tax lines in **øre**, excluding the four separately recorded spouse
  credits: state, municipal and church personal-allowance values, and negative
  capital-income credit. Taxes are positive; other credits are negative. Do not
  include subtotals or payments. Only mark the list complete after checking it.
- The four reported spouse credits as positive **øre** amounts or `null`, plus
  the person's own reported negative-capital credit when known.
- Prepaid tax, assessed tax, and surplus tax in **øre**. Payment corrections
  are signed øre: interest/allowances positive, prior refunds/offsets negative.
  The final refund is in **whole DKK**, after rounding down.

For a report with **restskat** instead of a refund, select the tax-owed route
below. Do not put a negative debt amount in the surplus-tax field.

Neither the spouse's municipality nor year-end tax cohabitation is mandatory.
When the spouse's municipality is unknown, the municipal transfer ceiling is
`null`; the model does not substitute the recipient's municipality. A known
failure of the cohabitation condition conflicts with a nonzero spouse transfer.

## Read the result

For a wholly synthetic income bridge of 400,000 − 10,000 − 45,000 − 333,000,
the necessary spouse loss deduction is **12,000 DKK**. That is the transferable
remainder required by this bridge, not the spouse's original loss or income.

If complete other tax lines sum to 102,000 DKK and reported assessed tax is
98,200 DKK, the four excluded spouse credits must total **3,800 DKK**. Their
individual allocation remains unknown unless separately observed. Missing
printed transfer lines do not prevent outputting this required sum.

The output distinguishes:

- `BetingetAfstemt`: the supplied comparisons agree and no checked bound is
  violated. Unverified legal conditions remain listed under `uafklaret`.
- `Modstrid`: an arithmetic comparison or checked necessary condition fails.
- `Ufuldstændig`: comparisons cannot be completed; available necessary amounts
  are still returned. Missing values are not silently zeroed.
- `UgyldigtRapportinput`: invalid numeric bounds, duplicate/empty post names,
  or negative ordinary deductions prevented reconciliation.
- `IkkeUnderstøttetÅrEllerKommune`: no supported parameter coverage.

Each check shows expected and observed amounts, unit, and **observed minus
expected** difference. Each condition shows the necessary amount, checked lower
and optional upper bound, and its explanation. An absent upper bound is
**unverified**, not unlimited entitlement. The output always sets
`uafhængig_skatteberegning_udført` to `false`. Domain statuses are result data;
successful CLI execution alone does not mean the report passed.

The allowance ceilings use the encoded PSL §§ 9–10 parameters. The capital
credit constraints use the shared PSL § 11 rate and household limit, less the
person's own reported credit when known. The inferred capital basis is a lower
bound, not a unique reconstruction of interest expenses. Loss transfer follows
the PSL § 13 income-capacity constraint; eligibility, prior own offsets and
cross-border adjustments remain unverified.
[Personskatteloven](https://www.retsinformation.dk/eli/lta/2021/1284).

An amended assessment's new surplus is not its additional refund: subtract any
reported earlier refund before comparing the whole-krone payout. This mode
checks those reported amounts, not the independent legality of interest rates,
offsets or payment dates. The refund route leaves negative post-correction
settlements incomplete; it does not silently turn them into a debt calculation.
[Kildeskatteloven §§ 60, 62 and 62 A](https://www.retsinformation.dk/eli/lta/2024/460).

## Reports with tax owed (restskat)

Set `betaling.restskat` to an object instead of `null` to reconcile the
**underlying restskat before interest and the percentage addition**. For example,
with reported assessed tax of 98,200 DKK and prepaid tax of 90,000 DKK:

```json
{
  "oplyst_restskat_øre": 820000,
  "tillæg_til_slutskat": [],
  "tillæg_til_slutskat_komplette": true
}
```

This is only the `betaling.restskat` object, not a complete input document.
It is a synthetic example. A complete empty addition list is a positive
confirmation that there are no additions, not a default for missing facts.
Use `false` when completeness is unknown; use `null` for an unknown reported
restskat amount. The expected amount remains visible when the underlying
figures are known, even if the reported restskat line is missing.

The check is:

`reported assessed tax + reported additions to that tax − prepaid tax`

Under KSL § 61(1), additions at this stage include carried restskat and the
specified pension tax. Record their positive øre amounts with unique names in
`tillæg_til_slutskat` **only when they are separate from the reported assessed
tax**. For instance, a separately reported 500 DKK carried amount gives a
principal of 8,700 DKK in this example. Do not double-count an included amount.
Keep the same current assessed-tax amount in `skat` and `betaling`; the existing
cross-section check still applies. These are report observations, not verified
legal classifications or permissions to add arbitrary balancing amounts.

Interest and percentage additions **on this year's restskat** do not belong in
that list. Neither do instalments, a prior refund, or a payment made after the
assessment. This route does not model those collection/reassessment movements.
Read the report's separately labelled principal line rather than substituting
the amount on a payment slip. For an amended assessment, this is the new total
principal, not necessarily the change from the preceding assessment.

While this route is selected, keep `oplyst_overskydende_skat_øre` and
`oplyst_udbetaling_kroner` as `null` and `korrektioner_til_udbetaling` empty.
These are inactive fields, not assumed zero observations. Supplying both routes
is rejected rather than silently dropping a payment observation. A known
negative principal contradicts the selected debt route, including when the
reported restskat is unknown. Zero is allowed for a reconciled zero balance.

`BetingetAfstemt` now means the **selected** arithmetic checks agree. In this
route it does not mean that an amount to collect, interest, rates, due dates,
minimum collection amounts, or earlier payments were checked. There is no
calculated “pay this now” output. The result keeps this limitation explicit.
The distinction follows [KSL § 61(1)–(2), § 62 and § 62 C](https://www.retsinformation.dk/eli/lta/2024/460/pdf),
checked against the official law text September 22, 2026. No new annual
interest rates or payment schedules are inferred.

### Existing templates

The optional `betaling.restskat` field changes the Preview model's contract
fingerprint. Regenerate the template and migrate supported observations; never
edit an old workbook's hidden fingerprint. `null` preserves the refund route,
including the earlier-refund correction. Existing JSON records may omit the
optional field after migration to the current envelope. Saved results are
historical evidence, not automatically recalculated with the new contract.

The focused regression checks both routes, including the one-øre debt boundary,
missing amounts/additions, mixed-route rejection and the existing refund cases:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --quiet --test tax_report_reconciliation
```

Coverage is 2023–2026 and the supported municipal parameter table. The mode
does not independently recompute individual tax lines, resolve other spouse
mechanisms, or prove a complete household solution. The typed interpreter
(`runa call`) is the supported execution path for this review; native `runa
check`/codegen currently encounters guarded-rule errors in the shared tax
parameter modules (tracked as `td-438812`). Do not claim native parity from a
successful frontend or interpreter check.
