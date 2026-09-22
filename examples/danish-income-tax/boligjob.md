# Audit service and handyman deductions from invoices

The canonical calculation includes a fact-based LL § 8 V route for service and
green home-improvement deductions. It does not accept the tax report's deduction
as an unrestricted balancing input. The same component is available separately
through `boligjob.calculate.runa`, so an invoice can be reviewed without filling
the entire personal-income-tax contract.

This remains research software. A person must classify the actual work against
the sources and supply supported facts. The executable model applies arithmetic
and explicit conditions; it does not ask an LLM to decide eligibility at runtime.

## Annual limits and dates

| Deduction year | Service cap per person, DKK | Green handyman cap, DKK |
| --- | ---: | ---: |
| 2023 | 6,600 | 0 |
| 2024 | 11,900 | 0 |
| 2025 | 17,500 | 8,600 |
| 2026 | 18,300 | 9,000 |

The two caps are separate and shared across all the person's homes. Material,
travel and other non-labour invoice costs do not consume or increase either
deduction. Labour inputs include VAT and are in exact integer øre.

The deduction generally belongs to the payment year. A payment within the first
two months after the work year belongs to the work year instead, including
February 29 in a leap year. Later payment does not make ineligible work eligible:
for example, paying in 2025 for roof insulation performed in 2024 still gives
no green handyman deduction. The new service categories also require work from
2025. Appliance repairs are a time-limited category through 2027; that does not
extend the calculation's verified annual-cap coverage past 2026.

Work from April 1, 2022 through 2026 can be reviewed when paid in the supported
payment range (2023–2027); the annual calculation still covers only 2023–2026.
The lower work-date boundary follows BEK 391/2022 § 14(2), checked September 22,
2026. Work before that date needs the earlier regulations and remains outside
this model's coverage, not automatically ineligible. For example, ordinary
cleaning performed in December 2022 and paid in March 2023 can enter the 2023
deduction, whereas payment in February 2023 belongs to 2022 and contributes
nothing to the 2023 calculation. This does not add a calculation for 2022.

Sources checked September 21, 2026:

- [LL § 8 V and the category list in annex 1](https://www.retsinformation.dk/eli/lta/2025/1500).
- [Payment, documentation and reporting rules, BEK 391/2022](https://www.retsinformation.dk/eli/lta/2022/391).
- [SKAT's 2023 service declaration and cap](https://skat.dk/media/ntnj5lsw/02048_januar_2023-t.pdf).
- [Skattestyrelsen's 2024 cap confirmation](https://sktst.dk/nyheder-og-pressemeddelelser/arsopgoerelsen-for-2024-aabner-den-24-marts).
- [Current general conditions and 2025–2026 caps](https://skat.dk/borger/fradrag/servicefradrag/generelle-betingelser).
- [Service categories](https://skat.dk/borger/fradrag/servicefradrag/servicefradrag),
  [green handyman categories](https://skat.dk/borger/fradrag/servicefradrag/haandvaerkerfradrag),
  and [the legal guidance](https://info.skat.dk/data.aspx?oid=2061725).

## Facts, allocations and exclusions

The canonical field is `lønmodtager.ligningsfradrag.boligjob`, also present for
an active spouse. Its three alternatives distinguish unknown information,
confirmed no expenses, and a complete set of expense facts. Missing facts must
not be converted to no expenses just to obtain a calculation.

Each post contains a documented payment/worksheet line, the work and payment
dates, a labour/material split, work category, supplier, property facts,
payment method, reporting, and explicit exclusion facts. `Ll8VSvar` preserves
unknown separately from yes and no. For example, known cash payment gives a
valid, ineligible expense; unknown payment method makes the calculation
incomplete. Eligibility being false is not the same as invalid source facts.

Use stable local references for the invoice **and its work line**, the actual
payment and the property. Do not embed CPR numbers. Duplicate identities or
duplicate invoice-line/payment references invalidate the annual input. This
checks the supplied records, not authenticity or identity across differently
labelled source documents.

`andele` allocates the paid labour among all participants. Their total cannot
exceed that labour, person keys must be unique, and each claimant identifies
their own key. A claimant who did not pay needs a spouse/cohabitant payer with
shared finances. When both spouses submit the same payment, the common facts
and full allocation must agree. The model does not optimise or invent an
allocation. Unequal shares are allowed; unused personal limits are not simply
transferred between people.

The rules also check age at year end, Danish tax status, residence/ownership,
supplier registration, electronic payment, evidence, reporting, public subsidy,
other deductions and the worker's relationship to the home. A rented holiday
home excludes service deductions; it does not automatically exclude eligible
green work. Private individuals can supply eligible services but not the green
handyman route. Foreign-property and cross-border status must be stated, not
inferred from a municipality or a supplier's name.

The category names refer to the listed work, not every activity involving the
named object. In particular:

- `Ll8VSkorsten` means eligible repair/improvement, not arbitrary construction.
- `Ll8VAfmonteringBrændeovn` requires the stated removal/closure conditions.
- `Ll8VSolcellerOgHusstandsmøller` excludes the business-tax route; a standalone
  household battery is not automatically covered.
- `Ll8VHvidevarereparation` means actual repair in the home of a covered
  appliance, not a diagnostic fee or repair of any electrical device.
- Ordinary ventilation servicing is not installation, repair or improvement.
- A subscription is not itself a documented eligible service performed.

If these distinctions are unresolved, choose `Ll8VUafklaretYdelse` and retain
the uncertainty. The model cannot validate a description it has not received.

## Run and read the result

Keep invoice documents and filled templates outside the checkout:

```sh
runa template examples/danish-income-tax/boligjob.calculate.runa --format json --output PRIVATE_WORK_DIR/boligjob.json
runa call examples/danish-income-tax/boligjob.calculate.runa --input PRIVATE_WORK_DIR/boligjob.json --output PRIVATE_WORK_DIR/boligjob-results.json
```

In the component output, inspect `input_gyldigt` and each post's conditions and
deduction year before using the amounts. In the full tax output the trace is
`ligningsfradrag.boligjob`; an active spouse's trace is under
`ægtefælle.grundlag.ligningsfradrag.boligjob`. The deduction is composed once
into other itemised deductions. Check the root
[validity assessment](personskat-validity.md) before comparing tax totals.

In the synthetic 2026 Copenhagen regression, a 600,000 DKK salary, no church tax
and no other optional deductions gives baseline tax of 208,725.64 DKK. Eligible
labour sufficient to reach both caps adds 27,300 DKK of deductions and reduces
modeled tax to 202,340.17 DKK: a 6,385.47 DKK tax difference, not a 27,300 DKK
tax credit. Shared-payment tests also check each spouse's separate allocation.

The component retains capped category totals in øre. Its whole-krone projection
for the existing tax interface discards fractions separately per category,
after combining that category's expenses. Independent confirmation of the
administrative rounding rule is still a launch follow-up; the integer-cap
regressions do not depend on fractional rounding.

## Migration and unfinished coverage

This adds a required field to the Preview calculation input. Regenerate
templates and migrate confirmed facts, not fingerprints. Existing fictional
no-deduction constructors explicitly use `IngenBoligjobudgifter`; that is not an
appropriate automatic choice for private cases.

Insurance allocation, advance payments, work before April 1, 2022 and annual limits
after 2026 are not yet supported. Such active input must be reported as outside
coverage, not as proof that the legal deduction is zero. The model records
reporting as a fact; it neither submits a claim nor checks amendment deadlines.
Property rights, foreign-tax status, registration and work classification remain
source facts to substantiate, not automated registry or document verification.
An overall launch/conformance sign-off remains separate.

Focused regression commands:

```sh
runa check examples/danish-income-tax/boligjob.calculate.runa
runa run tests/personskat_boligjob_rules_test.runa
cargo test --quiet --test personskat_boligjob -j 1
cargo test --quiet --test personskat_validity -j 1
```
