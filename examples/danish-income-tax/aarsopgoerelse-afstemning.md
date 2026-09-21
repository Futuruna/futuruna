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

Keep the PDFs, transcriptions, and results in a private directory **outside the
checkout**. Replace `PRIVATE_WORK_DIR` below with its actual path. From the repo:

```sh
runa template examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --format json --output PRIVATE_WORK_DIR/report-cases.json
# Fill the generated cases with report observations, preserving $futuruna.
runa call examples/danish-income-tax/aarsopgoerelse-afstemning.calculate.runa --input PRIVATE_WORK_DIR/report-cases.json --output PRIVATE_WORK_DIR/report-results.json
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
offsets or payment dates. It leaves restskat and negative post-correction
settlements incomplete.
[Kildeskatteloven §§ 60, 62 and 62 A](https://www.retsinformation.dk/eli/lta/2024/460).

Coverage is 2023–2026 and the supported municipal parameter table. The mode
does not independently recompute individual tax lines, resolve other spouse
mechanisms, or prove a complete household solution. The typed interpreter
(`runa call`) is the supported execution path for this review; native `runa
check`/codegen currently encounters guarded-rule errors in the shared tax
parameter modules (tracked as `td-438812`). Do not claim native parity from a
successful frontend or interpreter check.
