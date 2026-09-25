# Senior employment deduction in the 2026 calculation

The canonical taxpayer and spouse calculations now include the extra senior
employment deduction separately from ordinary employment, job and pension
deductions. It uses the existing birth date and the same model-derived LL § 9 J
income basis and eligibility condition as the ordinary deduction. Do not enter
the report's deduction amount as another input.

For 2026, the encoded rate is 1.4%, capped at 6,100 DKK. Eligibility covers the
two income years before the income year in which pension age is reached—not a
rolling two-year interval ending on the birthday. The shared, versioned Social
Pension Act § 1 a model supplies that year. There is no senior deduction before
2026; later-year rates are not silently extrapolated.

Sources checked on September 21, 2026:

- [Act 482 of 2024, § 2(4) and commencement in § 8(4)](https://www.retsinformation.dk/eli/lta/2024/482).
- [SKAT eligibility and rate guidance](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag).
- [Official regulated annual limits](https://svmn.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/ligningsloven).

The proposed five-year extension is not encoded. SKAT's guidance was checked
again on September 25, 2026 and still says it has not been enacted.
The [minister's August 10 reply](https://www.ft.dk/samling/20252/almdel/BEU/bilag/72/3167643.pdf)
also describes intended reintroduction, not enactment. For reproducibility, the
source module retains the enacted two-year provision and identifies the
verified 2026 limit.

## Result and migration

`skat.seniorbeskæftigelsesfradrag_kroner` shows the additional deduction;
`skat.beskæftigelsesfradrag_kroner` continues to mean the ordinary deduction.
The shared `Ligningslov9J9K9LResult.seniorbeskæftigelsesfradrag` retains the
age, eligibility, basis, rate, cap and validity trace. The spouse's field is
inside `ægtefælle.skat.ligningsfradrag` when a spouse calculation is present.

For a synthetic 2026 Copenhagen taxpayer born January 1, 1960, salary 600,000 DKK,
no church tax and no other income or deductions, the source-backed correction
adds 6,100 DKK to deductions. At 23.39% municipal tax this reduces modeled tax
by 1,426.79 DKK, from 208,725.64 to 207,298.85 DKK. The deduction is not itself
the tax saving. This is a synthetic regression, not a personal assessment.

No input fields were added. The additive Preview output changes the contract
fingerprint, so regenerate templates rather than editing stale fingerprints.
Continue to check [the canonical validity assessment](personskat-validity.md).

## Remaining boundaries

The current model truncates the positive percentage product to whole øre,
then rounds that amount **up** to whole kroner, retaining the annual cap.
It does not simply discard fractional kroner. For example, a 2026 basis of
100,001 DKK gives a modeled senior deduction of 1,401 DKK, not 1,400 DKK.
This convention is inferred from selected public-calculator observations;
the legal sources above establish the rate, cap and eligibility, not a separate
administrative rounding rule. A known one-krone senior disagreement remains
explicit in the [rounding evidence](skatdk-fradrag-oere-ekstern.md#kendte-afvigelser--ikke-match).
The capped regression's tax delta does not depend on fractional rounding.

This does not extend the canonical model's year/jurisdiction support. It reuses
the current ordinary employment-deduction basis, including modeled self-employed
income and [source-specific foreign-employment allocation](beskaeftigelsesfradrag.md);
it is not a new audit of every income category. The
[single-parent deduction](ligningsloven-par9j-enlig.md) is composed separately
and requires benefit facts. [Service and handyman deductions](boligjob.md) use
their own invoice facts and retain explicit coverage boundaries.

## Trace the rule and its qualifications

The source anchor now attaches the enacted provision, the dated 2026 rate
guidance, the rounding assumption and the known limitation as separate typed
roles. Use the compiler that passed the
[runtime check](../../website/public/ai-setup.md#tax-audit-runtime-check):

```sh
"$RUNA_BIN" meta --json examples/danish-income-tax/ligningsloven-par9j-senior.runa
"$RUNA_BIN" meta --json --role rate_source examples/danish-income-tax/ligningsloven-par9j-senior.runa
"$RUNA_BIN" check examples/danish-income-tax/ligningsloven-par9j-senior.runa
"$RUNA_BIN" tests/personskat_senior_test.runa
```

Read `assumption` and `warning` alongside `source` and `rate_source`; metadata
does not turn an observed convention into law. The
[metadata regression](../../tests/tax_supplementary_metadata.test.mjs) checks
the actual index, not merely the presence of comments. No rate, eligibility or
tax formula changed in this metadata/guidance correction.
