# Single-parent employment deduction

The canonical taxpayer and active spouse calculations include the extra
employment deduction in LL § 9 J(3)–(4). Eligibility requires entitlement to
**and receipt of extra børnetilskud** for the relevant quarter. Being unmarried,
having children, or receiving ordinary child benefits does not establish this.
The model takes benefit facts, not the deduction printed on the tax report.

## Rules and provenance

| Income year | Annual rate | Full-year cap, DKK |
| --- | ---: | ---: |
| 2023 | 6.25% | 24,400 |
| 2024 | 6.25% | 25,300 |
| 2025 | 11.5% | 48,300 |
| 2026 | 11.5% | 50,600 |

The basis and ordinary employment-deduction eligibility condition come from the
existing LL § 9 J calculation. The extra deduction is the capped full-year
amount multiplied by eligible quarters divided by four. The original bill's
explanatory notes explicitly describe a quarter of the total annual deduction;
this is the basis for applying the cap **before** quarter allocation. Fractions
are retained until the final whole-krone projection, avoiding double rounding.

Sources checked September 21, 2026:

- [Current LL § 9 J(3)–(4)](https://www.retsinformation.dk/eli/lta/2025/1500),
  and [the 2023 provision](https://www.retsinformation.dk/eli/lta/2023/42).
- [L 194 (2011–12), explanatory notes on quarter allocation](https://www.retsinformation.dk/eli/ft/201112L00194).
- [Official 2018–2024 regulated limits](https://svmn.dk/tal-og-metode/satser/tidsserier/centrale-beloebsgraenser-i-skattelovgivningen-2018-2024).
- [SKAT's 2025–2026 rates, caps and eligibility guidance](https://skat.dk/borger/fradrag/arbejdsrelaterede-fradrag/beskaeftigelses-og-jobfradrag).

## Input: unknown is not zero

`lønmodtager.ligningsfradrag.enlig_forsørger` has three choices:

- `EkstraBørnetilskudUoplyst`: unknown, and the generated template default.
- `IntetEkstraBørnetilskud`: explicitly confirmed no extra benefit for the year.
- `OplystEkstraBørnetilskud`: quarter facts and an explicit statement that the
  year's information is complete. Quarters omitted from a complete record have
  no eligible receipt. An empty complete record therefore gives zero.

For example, this synthetic JSON field value records one eligible quarter:

```json
{
  "$variant": "OplystEkstraBørnetilskud",
  "oplysninger_for_året_komplette": true,
  "kvartaler": [{
    "indkomstår": 2026,
    "kvartal": 1,
    "berettiget": true,
    "modtaget": true,
    "kildereference": "synthetic-benefit-decision"
  }]
}
```

Each row refers to the benefit's quarter, not the payment date, and not to an
individual child. Quarter numbers must be unique, in 1–4, in the calculation
year, and have a nonblank local source reference. A reference does not establish
the document's authenticity. Do not put personal identifiers in it.

The same field is required under `ægtefælle.fakta.lønmodtager.ligningsfradrag`
when `MedÆgtefælle` is active. Year-end marriage alone does not decide earlier
quarters' entitlement. Unknown or incomplete information fails the canonical
[validity assessment](personskat-validity.md), leaving the comparison amount
null. Ineligible but valid quarter facts are not an input error.

## Output and a working result

`enligforsørgerfradrag` exposes eligibility, quarters, basis, rate, cap, amount
and input errors. `helårsfradrag_kroner` is the hypothetical full-year amount
before quarter eligibility; only `fradrag_kroner` is the applied deduction.
The latter is included once in `skat.øvrige_ligningsmæssige_fradrag_kroner` and
the deduction total. It is separate from ordinary and senior employment
deductions. The spouse trace is `ægtefælle.grundlag.enligforsørgerfradrag`.

For a synthetic 2026 Copenhagen taxpayer born in 1990, salary 600,000 DKK,
no church tax and no other income or deductions, two eligible quarters give
25,300 DKK of extra deduction. At the model's 23.39% municipal rate, tax falls
by 5,917.67 DKK from 208,725.64 to 202,807.97 DKK. The deduction is not itself
the tax saving. This is a regression case, not a personal assessment.

## Migration and limits

This adds a required input field to the research model's Preview contract.
Regenerate JSON/XLSX templates and copy confirmed facts; do not edit the old
fingerprint or automatically replace missing information with the `Intet`
choice. Source-level `PersonskatLigningsfradragInput` constructors need the new
field too. The fictional no-deductions helper and examples explicitly assert
`IntetEkstraBørnetilskud`; that assertion is not evidence for a private case.

The whole-krone projection discards positive fractions, following the existing
ordinary LL § 9 J model. The cited sources establish eligibility, rates, caps
and allocation, not independent confirmation of the administrative rounding
rule. That confirmation and the existing foreign-employer fact-routing gap
remain launch follow-ups. Future years are not extrapolated. Service and
handyman deductions and complete model conformance remain separate work.

Focused checks:

```sh
runa check examples/danish-income-tax/ligningsloven-par9j-enlig.runa
runa run tests/personskat_single_parent_test.runa
cargo test --quiet --test personskat_validity -j 1
```
