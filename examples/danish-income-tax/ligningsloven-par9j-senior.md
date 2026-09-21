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

The proposed five-year extension is not encoded. SKAT's current guidance says
it has not been enacted. For reproducibility, the source module retains the
enacted two-year provision and identifies the verified 2026 limit.

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

The whole-krone projection follows the existing ordinary LL § 9 J model:
fractional positive deduction kroner are discarded. The source checks above
establish the rate, cap and eligibility, not a separate administrative rounding
rule. Independent confirmation of that projection remains a launch follow-up.
The capped regression's tax delta does not depend on fractional rounding.

This does not extend the canonical model's year/jurisdiction support or fix its
existing limitations in foreign-employer fact routing. It reuses the current
ordinary employment-deduction basis, including modeled self-employed income;
it is not a new audit of every income category. The
[single-parent deduction](ligningsloven-par9j-enlig.md) is composed separately
and requires benefit facts. [Service and handyman deductions](boligjob.md) use
their own invoice facts and retain explicit coverage boundaries.

Focused checks:

```sh
runa check examples/danish-income-tax/ligningsloven-par9j-senior.runa
runa run tests/personskat_senior_test.runa
runa examples/danish-income-tax/ligningsloven-par9l.scenario.runa
cargo test --quiet --test personskat_validity -j 1
```
