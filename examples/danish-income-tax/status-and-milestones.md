# Personskatteloven as Futuruna

Status: active implementation; first-slice corpus complete
Last updated: 2026-07-18
TD epic: `td-56cf8d`
Current focus issue: `td-2d84ec`

This folder is the working home for encoding Danish personal income tax law in
Futuruna. The aim is not only to display the law as source code, but to make the
rules executable enough to calculate ordinary tax cases and strict enough to
audit tensions, cliffs, missing definitions, and delegated dependencies.

Current project priority: finish the source-backed Personskatteloven
implementation first. Audit files remain important as validation gates for
implemented slices, but deeper exploratory audits should wait until the main law
model is materially complete.

Latest mainline slices: § 1/§ 2 now compose ordinary taxable income as an
amount-level result from personal income, capital income, excluded share income,
excluded CFC income and ligningsmæssige fradrag, and the wage-earner calculator
delegates its taxable-income base to that result; the 2026 § 7/§ 7 a/§ 8 reform
parameters for mellemskat, topskat and toptopskat now carry LOV nr. 482/2024
source provenance and derive statutory 2010-level thresholds through § 20
regulation, with § 7 a topskat and § 8 toptopskat exposed as personal-income
amount result objects; § 6 bundskat rates now carry the
2021 base text and the 2022/2023/2024 amendment source chain as a typed
rate result; § 7 spouse positive-net-capital tax now has an
executable allocation layer for stk. 10-11 and a stk. 12 equal-basis tie-break
rule; § 7 a now has post-level amount rules for included and excluded
pension-like payments; § 13, stk. 2 now has amount-level spouse transfer where
remaining deficit is first deducted from the spouse's taxable income and then
converted to tax value against the spouse's §§ 6, 7, 7 a, § 8 and § 8 a, stk. 2
tax basket, wired through the wage-earner calculator; § 13, stk. 4 now has
amount-level spouse and carry-forward offset ordering for negative personal
income; § 8 a, stk. 2
high-layer share-income tax now flows through the wage-earner § 5/§ 9
state-tax path, § 8 a, stk. 6 now has a pair-level both-negative spouse
share-income threshold allocation, and § 8 a negative share-income annual
settlement now offsets the taxpayer's slutskat, spouse slutskat, and then
carries the remainder forward; § 9
now has amount-level state-personfradrag reduction ordering for the split
§ 8/sundhedsbidrag and § 6/bundskat tax values plus non-state § 8 c,
municipal-tax and church-tax personfradrag reductions, wired through the wage-earner
calculator; § 10, stk. 3 now has amount-level spouse transfer of unused
personfradrag state-tax value into the receiving spouse's § 9 state-tax basket,
and the public wage-earner model now delegates to `LønmodtagerBeregningSag` so
ordinary fixtures and special tax postures both use the same scoped § 10
eligibility rules; § 11 negative net-capital relief now runs through the
ordinary wage-earner calculator as a named `LønmodtagerPar11NedslagResultat`,
with the public input carrying net capital income while the model derives
positive and negative capital views for the relevant statutory branches;
§ 14 now has a reusable statutory skatteberegning result that chooses between
stk. 1/stk. 3 helårsomregning and stk. 2 period reduction, and the partial-year
wage-earner path delegates its final § 14 amount to that object;
§ 10 now reflects LOV 1564/2023's removal of the separate under-18 basis from
income year 2023 onward, and § 4, stk. 1, nr. 6 has a source-backed post-LOV
615/2026 category fixture for the ejendomsværdiskattelov reference;
§ 26, stk. 7 now composes § 7 spouse capital-threshold and
capital-tax allocation rules into the transition-compensation nr. 3 amount, and
§ 26 now has an annual compensation-settlement result that composes
source-derived yearly parameters with the statutory tax-offset order plus
pair-level stk. 4 spouse difference results, pair-level stk. 5 and stk. 8
net-capital offset results, and a pair-level annual path where stk. 6
bundfradrag transfer feeds the nr. 2 line item;
§ 8 c's 2023-2026 published limited-taxpayer rate now has a
`Par8cSatsResultat` result that keeps the statutory rounded-down municipal
average method, the Skatteministeriet source posture, and the applied
basispoint rate together; § 8 b's CFC tax rate now delegates through a
`SelskabsskattelovPar17Stk1SatsResultat` that keeps the tracked
Selskabsskatteloven source line for 2024 and 2025+, ordinary 22 pct.
selskabsskat, 3 percentage-point kulbrinte supplement, and applied CFC rate
together.

Distance to full implementation: the first-slice legal corpus for §§ 1-28 is in
place, and the ordinary wage-earner/slutopgørelse path is already calculation
useful. The remaining work is turning posture-only clauses and edge cases into
amount-level, source-backed rules. Treat this as past the structural phase and
well into implementation, but not yet close to a complete statutory model.

## Source Status

Primary prompt source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2019/799`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2019/799/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-18: `Historic`
- XML end date observed on 2026-07-18: `2026-06-23`
- Historic mark in XML: `2021-06-16`

Current working source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2021/1284`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2021/1284/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-18: `Valid`
- Signed: `2021-06-14`
- In force from: `2021-06-16`
- XML end date observed on 2026-07-18: `2026-07-01`
- Tracked amendment sources now include `2022/252`, `2023/610`,
  `2023/1564`, `2024/108`, `2024/482`, `2024/1691` and `2026/615`.

Current source-refresh finding:

- The tracked Retsinformation XML sources were re-fetched on 2026-07-18.
- The official XML `Status` fields remained unchanged: the working/dependency
  sources still report `Valid`, while `2019/799` reports `Historic`.
- Every tracked `Valid` source now has an XML `EndDate` horizon before
  2026-07-02, so `source-status.runa` distinguishes formal legal validity from
  current-day automation freshness.
- `AktuelSkatteberegning` still accepts formally valid sources; the new
  `DagsaktuelAutomatiskBeregning` purpose rejects sources whose metadata horizon
  does not cover `20260702`.
- `scripts/refresh-danish-tax-source-status.py --today 20260702 --fail-on-drift`
  fetches official XML for every `Retskilde(...)` record and reports semantic
  drift between Retsinformation and the encoded source model. On 2026-07-18 it
  checked 24 records with 0 drift and 0 fetch/parse errors.

Current Personskatteloven amendment sources:

- Bundskat 2022 amendment:
  `https://www.retsinformation.dk/eli/lta/2022/252`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,09 pct. and 4,09 pct. for
    income year 2022 and later.
- Bundskat 2023 amendment:
  `https://www.retsinformation.dk/eli/lta/2023/610`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,06 pct. and 4,06 pct. for
    income year 2023 and later.
- Personfradrag under 18:
  `https://www.retsinformation.dk/eli/lta/2023/1564`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 2 removes the separate § 10, stk. 2 under-18 basis and inserts a
    single 39.350 kr. 2010-level basis; the rule model keeps the old basis only
    for pre-2023 historical queries.
- Bundskat 2024 amendment:
  `https://www.retsinformation.dk/eli/lta/2024/108`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 1 replaces § 6, stk. 2 with 12,01 pct. and 4,01 pct. for
    income year 2024 and later.
- Person-tax reform amendment:
  `https://www.retsinformation.dk/eli/lta/2024/482`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 14 repeals Personskatteloven § 13, stk. 5, 4. pkt.
  - § 8, stk. 4 gives § 1 effect from income year 2026.
- Iværksætterpakken amendment:
  `https://www.retsinformation.dk/eli/lta/2024/1691`
  - XML status on 2026-07-18: `Valid`
  - § 4 updates § 8 a share-income thresholds for income years 2025-2027 and
    later.
- Property-category amendment:
  `https://www.retsinformation.dk/eli/lta/2026/615`
  - XML status on 2026-07-18: `Valid`
  - § 12 changes § 4, stk. 1, nr. 6's ejendomsværdiskattelov reference to
    nr. 1-4, 8 and 9.

Current § 8 b dependency source:

- Historic Selskabsskatteloven source:
  `https://www.retsinformation.dk/eli/lta/2022/1241`
  - XML status on 2026-07-18: `Historic`
  - Used for the 2024 § 8 b rate path that predated LBK nr. 279/2025.
- Current Selskabsskatteloven source:
  `https://www.retsinformation.dk/eli/lta/2025/279`
  - XML status on 2026-07-18: `Valid`
  - § 17, stk. 1 sets the ordinary selskabsskat rate at 22 pct. and the
    kulbrinte supplement at 3 percentage points; Personskatteloven § 8 b uses
    the ordinary 22 pct. rate for CFC income.

Current § 13 amendment/dependency sources:

- Pensionsbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2024/1243`
  - XML status on 2026-07-18: `Valid`
  - § 16, stk. 1, 4. pkt. is the historic PBL cap reference used by
    Personskatteloven § 13 through income year 2025.
- Ligningsloven:
  `https://www.retsinformation.dk/eli/lta/2025/1500`
  - XML status on 2026-07-18: `Valid`
  - § 33 A is the foreign-wage relief exception in § 13, stk. 5.
  - §§ 9 J and 9 K are the ordinary employment/job-deduction slice used by the
    wage-earner calculator; § 9 L is now modeled for extra pension deductions
    and § 26 nr. 5 transition-compensation input. The § 26 path now has
    2012-2019 Ligningsloven deduction parameter coverage for the first
    transition-compensation calculation layer, and § 26 year packs now derive
    their § 20 regulation number, § 7 top-tax threshold and § 8 health
    contribution rate instead of taking those legal-year facts as fixture
    literals.
  - SKM rates page used for current basis points and caps:
    `https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/ligningsloven`
  - SKM `Skatteberegning - hovedtrækkene i personbeskatningen` pages for
    2014, 2016, 2017 and 2018 are used for the historical § 26-relevant
    Ligningsloven deduction rates and pre-2018 absence of §§ 9 K/9 L.
- Sømandsbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2023/1181`
  - XML status on 2026-07-18: `Valid`
  - §§ 5-8 are the seamen relief exception in § 13, stk. 5.

Current AM-contribution dependency sources:

- Arbejdsmarkedsbidragsloven:
  `https://www.retsinformation.dk/eli/lta/2020/121`
  - XML status on 2026-07-18: `Valid`
  - §§ 1-7 cover the first ordinary and special-case AM-contribution slice:
    ordinary wage remuneration/naturalier, § 3 exclusions, self-employed bases
    with and without virksomhedsordning, library-fee compensation, and
    collection-reference posture.
- AM youth exemption amendment:
  `https://www.retsinformation.dk/eli/lta/2025/96`
  - XML status on 2026-07-18: `Valid`
  - § 1 adds 0 pct. AM contribution through the income year in which the
    person turns 17, with effect from January 1, 2026 under § 7, stk. 4.

Current municipal/church-tax and withholding dependency sources:

- Kommuneskatteloven:
  `https://www.retsinformation.dk/eli/lta/2019/935`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 5 and 6 are the first ordinary municipal-income-tax slice used by
    the wage-earner calculator.
- Folkekirkens økonomi:
  `https://www.retsinformation.dk/eli/lta/2023/424`
  - XML status on 2026-07-18: `Valid`
  - § 18 is the first ordinary church-tax membership/rate slice used by the
    wage-earner calculator.
- Folkekirkens økonomi amendment:
  `https://www.retsinformation.dk/eli/lta/2025/1772`
  - XML status on 2026-07-18: `Valid`
  - § 2, nr. 4-6 touches § 18 and is tracked as a dependency for current
    church-tax wording.
- Kildeskatteloven:
  `https://www.retsinformation.dk/eli/lta/2024/460`
  - XML status on 2026-07-18: `Valid`
  - §§ 41, 43, 46 and 48 are the first ordinary A-income and A-tax withholding
    slice used to distinguish final annual tax from payroll withholding. § 48
    now covers e-skattekort retrieval posture, main-card period allowances,
    bikort with no allowance, frikort/no-card behavior, optional higher
    withholding percentage, and base rounding to whole 10-kroner amounts.
    §§ 58, 60-62, 62 A, 62 C and 67 now cover the first final-settlement slice:
    B-skat installment calendar projection, crediting, restskat/overskydende
    skat balance, spouse offsetting, restskat percentage supplement and timing
    posture, system-date-driven § 61 stk. 4/stk. 6 restskat rateplans,
    overskydende skat compensation and refund posture, amended annual statement
    interest posture, minimum-rate thresholds, and dividend-tax credit posture.
- Bekendtgørelse om kildeskat:
  `https://www.retsinformation.dk/eli/lta/2025/839`
  - XML status on 2026-07-18: `Valid`
  - §§ 2, 5, 8, 9, 12 and 13 are the first forskudsopgørelse-to-skattekort
    generation slice used to turn a forskudsskat and unrounded withholding
    percentage into card allowance, rounded withholding percentage, and
    possible B-tax overflow.
- Forskudsregistrering/indeholdelsesprocent 2026:
  `https://www.retsinformation.dk/eli/lta/2025/1094`
  - XML status on 2026-07-18: `Valid`
  - § 6 is the first annual source-backed derivation of the 2026
    indeholdelsesprocentsats: skattekommunens laveste skatteprocentsats plus
    positive mellemskat/topskat/toptopskat rates computed with two decimals.
- Forskudsregistrering/indeholdelsesprocent 2026 amendment:
  `https://www.retsinformation.dk/eli/lta/2025/1828`
  - XML status on 2026-07-18: `Valid`
  - Amends BEK 1094 § 1, stk. 2, and is tracked as a current dependency;
    it does not change § 6.
- Opkrævningsloven:
  `https://www.retsinformation.dk/eli/lta/2024/1040`
  - XML status on 2026-07-18: `Valid`
  - §§ 1, 2, 4, 5 and 7 are the first payment-deadline/remittance/rate slice
    for withheld A-skat and AM-bidrag: ordinary monthly deadline,
    large-withholder deadline, region/municipality exception, provisional
    assessment posture, corrected underpayment, late-payment interest posture,
    and the § 7 stk. 2 annual-rate formula from Nationalbank July/August/
    September kassekreditrente inputs.
  - The 2025 and 2026 § 7 stk. 2 settlement-rate fixtures now use
    Skattestyrelsen's published `SKM2024.619.SKTST` and
    `SKM2025.720.SKTST` rate sources:
    `https://info.skat.dk/data.aspx?oid=2436822` and
    `https://info.skat.dk/data.aspx?oid=2459995`.
  - The § 7, stk. 1 late-payment supplement source drift is resolved through
    LOV 1694/2024, LOV 1783/2025 and BEK 1793/2025: the live supplement is
    0,85 procentpoint from January 1, 2026, so the 2026 late-payment monthly
    rate fixture is 0,95 pct. before daily accrual mechanics.
  - Den juridiske vejledning 2026-1, A.B.4.7.2, supplies the administrative
    daily-interest convention: the renteår is the calendar year, so the day
    divisor is 365 or 366 in leap years:
    `https://info.skat.dk/data.aspx?oid=2168585&chk=220619`.

Current external validation sources:

- Skattestyrelsen calculator:
  `https://www.tastselv.skat.dk/fskbrgn2/Skprofil.aspx?indkomstaar=2026`
  - Supporting public page:
    `https://skat.dk/en-us/individuals/preliminary-income-assessment/calculate-your-pay`
  - Retrieved on 2026-07-18.
  - Calculator version observed in the profile form: `26.2.2.3`.
  - First fixture: enlig Copenhagen taxpayer, born 01.01.1980, no church tax,
    no spouse/children/self-employment, 600.000 kr. in `Lønindkomst mv.`
    (`tbAFYfnr201`), all other tax-information fields blank/default.
  - Observed result used in `skatdk-2026-ekstern.scenario.runa`: final tax
    including AM contribution 208.725,64 kr., forskudsskat to A/B-tax
    collection 160.725,64 kr., trækprocent 36 pct., and monthly tax-card
    allowance 8.164 kr.
- Den juridiske vejledning 2026-1, C.F.1.6.2.1:
  `https://info.skat.dk/data.aspx?oid=1977388`
  - Used in `omregning-skatteloft-ekstern.scenario.runa` for the official
    § 14 annualisation example: 27-day tax-liability period, one-off income not
    annualised, recurring items annualised and rounded to whole kroner, yielding
    444.077 kr. personal income, -38.052 kr. capital income, 60.865 kr.
    ligningsmæssige fradrag, and 345.160 kr. taxable income.
  - Used in `kapitel-04-omregning-skatteloft.runa` for the § 14 stk. 2
    election posture that the oplysningsskema election belongs to the year
    where full tax liability ceases or begins, and that reversal must be stated
    by 30 June in the second calendar year after the income year.
- Skatteministeriet, "Oversigt over kommuneskatter":
  `https://skm.dk/tal-og-metode/satser/oversigt-over-kommuneskatter`
  - Used in `omregning-skatteloft-ekstern.scenario.runa` for the
    `kommuneskattesatser_2026.xlsx` Langeland row: 26,30 pct. municipal tax and
    1,24 pct. published `Nedslag pct.`.
- Beskæftigelsesministeriet, "Boligstøtte", satser for 2026:
  `https://bm.dk/satser/satser-for-2026/boligstoette`
  - Used in `husholdning-benefit-cliffs.audit.runa` for boligsikring § 22/§ 23
    rates: 170.300 kr. income threshold, 44.900 kr. child-threshold increment
    for the 2nd-4th child, 28.700 kr. minimum own payment, and 50.412 kr.
    annual maximum boligsikring.
- Styrelsen for Arbejdsmarked og Rekruttering, "Boligsikring":
  `https://star.dk/ydelser/boligstoette-boernetilskud-og-hjaelp-i-saerlige-tilfaelde/boligstoette/boligsikring`
  - Used in `husholdning-benefit-cliffs.audit.runa` for the official
    calculation posture that only children from the 2nd through 4th child
    increase the § 22 boligsikring income threshold.

Working decision: use `2021/1284` as the current consolidated source for live
encoding, while preserving `2019/799` as source lineage because the valid
consolidation explicitly builds on it. The 2019 source remains useful for
historical audit and diffing, but it should not be treated as the live basis for
calculating a current taxpayer's tax. For provisions modified by later valid
amendment acts, such as § 13's 2026 PBL § 16 repeal, the amendment act must be
encoded as a temporal rule on top of the consolidation.

## Current Implementation Status

- Folder created at `examples/danish-income-tax/`.
- `source-status.runa` exists and checks/runs with `runa run`; it now models
  Retskilde records with named metadata fields and separates formal legal
  validity from current-day XML metadata freshness.
- `scripts/refresh-danish-tax-source-status.py` exists and self-tests; the live
  run checks all `Retskilde(...)` records against official Retsinformation XML
  before source metadata is refreshed by hand.
- `kapitel-01-indkomst.runa` exists and checks with `runa check`.
- `kapitel-02-statsskat.runa` exists and checks with `runa check`.
- `kapitel-03-personfradrag.runa` exists and checks with `runa check`.
- `kapitel-04-omregning-skatteloft.runa` exists and checks with `runa check`.
- `kapitel-05-afsluttende-bestemmelser.runa` exists and checks with
  `runa check`.
- `arbejdsmarkedsbidragsloven.runa` exists and checks with `runa check`.
- `kommuneskatteloven.runa` exists and checks with `runa check`.
- `folkekirkens-oekonomi.runa` exists and checks with `runa check`.
- `kildeskatteloven.runa` exists and checks with `runa check`.
- `kildeskattebekendtgoerelsen.runa` exists and checks/runs with `runa run`.
- `forskudsregistrering_2026.runa` exists and checks/runs with `runa run`.
- `slutopgoerelse.runa` exists and checks/runs with `runa run`.
- `opkraevningsloven.runa` exists and checks/runs with `runa run`.
- `ligningsloven_fradrag.runa` exists and checks with `runa check`.
- `skatteaar-parametre.runa` exists and checks with `runa check`.
- `loenmodtager_beregning.runa` exists and checks with `runa check`.
- `loenmodtager-fixtures.scenario.runa` exists and checks/runs with `runa run`.
- `loenmodtager-par11.audit.runa` exists and checks/runs with `runa run`.
- `loenmodtager-par13-spouse.audit.runa` exists and checks/runs with
  `runa run`.
- `skatdk-2026-ekstern.scenario.runa` exists and checks/runs with `runa run`.
- `delaar-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `omregning-skatteloft-ekstern.scenario.runa` exists and checks/runs with
  `runa run`.
- `husholdning-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `husholdning-benefit-cliffs.audit.runa` exists and checks/runs with
  `runa run`.
- `aktieindkomst-pension.audit.runa` exists and checks/runs with `runa run`.
- `aktieindkomst-slutopgoerelse.runa` exists and checks with `runa check`.
- `aktieindkomst-slutopgoerelse.scenario.runa` exists and checks/runs with
  `runa run`.
- `slutopgoerelse.scenario.runa` exists and checks/runs with `runa run`.
- `indeholdelse-afregning.scenario.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-bomber.audit.runa` exists and checks/runs with `runa run`.
- `personskatteloven-konfiskatorisk.audit.runa` exists and checks/runs with
  `runa run`; its bounded year/municipality grid is now declared as
  constructor-shaped `|` facts and enumerated with `findall`.
- `personskatteloven.audit.runa` exists and checks with `runa check`; focused
  `.audit.runa` entrypoints are preferred for per-slice execution while the
  umbrella audit stays broad.
- `pengebeloeb.runa` exists and checks/runs with `runa run`.
- Website research page exists at `/research/personskatteloven` and renders
  source status, milestone status, selected audit signals, and the checked
  `.runa` corpus.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy including amount-level § 1 ordinary taxable-income
  composition across the separate § 2 categories, amount-level § 3 personal-income inclusion
  and deduction totals, amount-level § 4 net capital-income inclusion,
  deductible capital costs, positive/negative net-capital projections and
  personal-income reclassification, plus amount-level § 4 a share-income
  inclusion, stk. 2 exclusions, stk. 3 personal-income reclassification,
  negative share-income preservation and pension deduction from positive share
  income, and amount-level § 4 b CFC-income aggregation with positive § 8 b
  tax base projection, the §§ 5-9 state-tax skeleton including amount-level
  § 6 spouse negative net-capital offset and § 7 stk. 5 spouse
  positive-capital threshold/negative-capital offset, § 12 unused
  personfradrag tax-value allocation across the § 9 state-tax basket, the §§ 10-13
  personfradrag/underskud slice, the §§ 14-20 omregning/skatteloft/regulering
  slice, the §§ 21-28 concluding provisions slice, ordinary and special-case
  AM-law,
  municipal-income-tax, church-tax, Kildeskatteloven A-income/withholding,
  BEK 839 forskudskort generation, BEK 1094 2026 indeholdelsesprocent,
  Kildeskatteloven §§ 60-62/62 A/62 C/67 slutopgørelse balance,
  restskat timing, date-derived B-skat rate windows, B-skat minimum-rate
  completion plans, system-date-driven § 61 stk. 4/stk. 6 restskat rateplans
  with exact and mixed large/small installment splits, date-derived § 62 A interest spans, and overskydende-skat
  compensation posture,
  Opkrævningsloven payment deadlines and § 7 late-payment rate posture,
  shared money/rounding posture for whole kroner, ten-kroner floors,
  basispoint rounding, and øre-level fractions,
  a source-backed pre-2026 § 7 topskat amount result with regulated
  bundfradrag, regulated positive-capital grundbeløb, PBL § 16 additions,
  personal/capital split and wage-earner calculator reuse,
  a source-backed 2026 reform parameter/result layer deriving mellemskat,
  topskat, toptopskat, and the mellemskat positive-capital grundbeløb from the
  amendment's 2010-level amounts through § 20 while preserving the LOV nr.
  482/2024 source branch for each layer,
  Ligningsloven ordinary wage-earner deduction
  dependency slices, § 26 historical year-parameter derivation for 2012-2019,
  2024/2025/2026 tax-year parameter packs, grouped
  wage-earner calculation-domain records, first wage-earner scenarios, a first
  § 14 partial-year wage-earner scenario, a first fictional household scenario,
  a first § 8 a share-income final-settlement scenario with § 67 dividend-tax
  credit splitting and § 8 a stk. 2 composition through the § 9/§ 12 state
  personfradrag allocation slot,
  § 14 stk. 2 election/reversal control flow for full-tax-liability entry or
  exit, including the 30 June second-calendar-year reversal deadline and the
  continued mandatory annualisation path for § 10 stk. 6 limited-taxability
  cases,
  calculator-level nonzero § 8 b CFC tax and § 8 c municipal-equivalent
  limited-taxpayer tax through a grouped `LønmodtagerSkatteforhold` path,
  a source-backed external Skat.dk 2026 wage-earner scenario, complex § 13
  calculator fixtures, and first audit signals.
- The chapter files follow the repeating structure: official legal text in a
  multiline block, then the corresponding Futuruna rules.
- Existing Danish Constitution examples show the intended style: original legal
  text in multiline source blocks, followed by Futuruna types, constants, and
  typed `|` legal rules.
- Typed `|` rule heads, `under` conditions, and `exception` rules are already
  present in the language test corpus and should be used for legal formulations.
- Website integration is active and should be updated whenever a checked
  Personskatteloven `.runa` slice becomes part of the displayed corpus.
- Executable scenario tests use `.scenario.runa` filenames. Cross-cutting audit
  suites use `.audit.runa` filenames.
- New source-law modules should avoid embedding scenario assertions where the
  test facts are better expressed as `.scenario.runa` files. Existing local
  smoke fixtures can be migrated as their surrounding legal slices are revised.

## Implementation Completion Snapshot

As of 2026-07-18, the corpus should be treated as a source-backed first-slice
full-statute implementation plus an ordinary-taxpayer calculator prototype, not
as a complete Personskatteloven calculator.

- Structural/source coverage is high: §§ 1-28 are represented in chapter files,
  and the core dependency laws needed for ordinary wage-earner calculation have
  executable first slices.
- Ordinary wage-earner calculation coverage is useful but not complete: current
  scenarios exercise wage income, AM contribution, ordinary wage-earner
  deductions, municipal/church tax, state-tax components, personfradrag,
  selected § 13 deficit paths including spouse/current-year negative personal
  income and carried-forward negative personal-income ordering through the
  reusable § 13 complex calculator, § 14 annualisation/election cases, § 19
  cases, withholding/card generation, and first final-settlement paths.
- Full legal calculation coverage is still materially incomplete: some rules are
  still posture/category coverage rather than amount-level calculations, several
  dependent statutes are first-slice only, and special regimes or edge cases are
  represented by selected scenarios rather than comprehensive calculation paths.
- Working estimate: roughly 63-73% complete as an executable research corpus,
  and roughly 48-58% complete as a production-grade calculator for
  Personskatteloven plus its necessary dependencies.
- Current priority: close source-backed calculation gaps in the law itself.
  Audits should validate newly implemented slices; deeper exploratory "bomb"
  audits, including source-derived confiscatory restskat search expansion in
  `td-f318b1`, are deferred until the main implementation is substantially
  complete.
- Completion posture: the next sessions should prefer converting remaining
  posture/category rules into amount-level legal calculations over adding new
  exploratory audit search spaces.

## File Layout

The corpus is intentionally split across multiple `.runa` files. The split is
legal and operational, not arbitrary: Personskatteloven is grouped by chapter,
tax-year data lives in parameter modules, executable normal-person calculations
live in calculator/fixture modules, and dependent statutes such as
Arbejdsmarkedsbidragsloven live in their own source-cited files.

Each legal file should remain a repeating source sequence:

1. official legal text in a multiline block,
2. an optional note only when the code cannot make the legal choice clear on
   its own,
3. idiomatic Futuruna rules, preferably typed `|` rules with `under` and
   `exception` where the law has conditions or carve-outs.

Imports are preferred over large monolithic files. This lets each slice be
checked independently, lets audit modules compose across laws, and keeps the
website integration able to show verified progress without waiting for the
whole statute to be calculation-complete.

## Domain Model Review

Wide records are not automatically a problem. Parameter packs and result
breakdowns are expected to be wide because they represent a table row or a
reporting surface. A record becomes suspect when unrelated facts are passed down
only so subrules can project one or two fields from it.

Current decision:

- Futuruna supports named arguments (`name = value`) for named-field records,
  scoped-rule constructors, ordinary functions, rule calls, and scoped-rule
  member calls. Wide legal/domain records and boolean-bearing legal predicates
  should use named calls at fixture and boundary-assembly points when
  positional arguments would hide legal meaning.
- `Par1AlmindeligSkattepligtigIndkomstSag` uses product-scoped `|` rules for
  § 1/§ 2 taxable-income composition. It keeps personal income, capital income,
  share income outside ordinary taxable income, CFC income outside the §§ 6-8 a
  taxable-income base, and ligningsmæssige fradrag in one result; the ordinary
  wage-earner calculator now delegates its taxable-income base to that result
  instead of carrying a local formula.
- The confiscatory audit work tightened Futuruna's language/runtime support:
  typed `|` rule-head parameters that name a `RuleScope` type now keep that
  receiver type through checking, and named constructors inside nested
  collection lambdas no longer leak the internal named-argument marker into
  generated closure captures.
- Futuruna now treats constructor-shaped rule facts as proper ground facts for
  `findall` in both interpreted and compiled execution. This lets audit search
  spaces be declared as legal/domain facts instead of duplicated hand-written
  lists.
- The confiscatory audit now records the current distinction between
  current-year annual tax and Kildeskatteloven payment burden: the bounded
  search finds no current-year tax over 100% of the positive income base, but
  does find payment-burden cases over 100% when transferred restskat m.v. is
  included.
- A readability sweep now uses named construction and named function/rule calls
  for the broad executable Danish-income-tax records and boolean-heavy calls
  found by scan, including statutory rate rows, remittance calendar/history
  facts, household scenario assembly, and audit inputs; short date-like triples
  and compact arithmetic helpers can remain positional where that is still
  idiomatic.
- `opkraevningsloven.runa` now splits the former 11-field remittance input into
  `OpkrævningAfregningsperiode`, `OpkrævningTilsvarHistorik`,
  `OpkrævningBankkalender`, `OpkrævningBetaling`, and a small composed
  `OpkrævningASkatAmAfregningInput`.
- `indeholdelse-afregning.scenario.runa` owns the executable remittance facts
  and assertions. The source-law module keeps the original legal text and the
  corresponding rules.
- `Par13KompleksBeregningInput` now composes named subdomains instead of a
  25-field positional record: income basis, tax-value rates, offset-tax pools,
  spouse-transfer facts, negative-personal-income facts, stk. 5 limitation
  facts, and same-business loss facts. The calculator rules project from those
  domain objects, and the scenario/audit fixtures name those facts before
  composing the calculator input.
- `Par13ModregningSag` uses product-scoped `|` rules for the § 13 ordered
  tax-value offset chain, keeping the carried remainder after § 6, § 7, § 7 a,
  and § 8 a, stk. 2 inside the same legal case while preserving public wrapper
  rule names for downstream calculator/audit files.
- `Par13ÆgtefælleSkatModregningSag` uses product-scoped `|` rules for the
  § 13, stk. 2 spouse tax-value step after spouse taxable-income deduction. It
  keeps the remaining transferred deficit, tax-value rate, spouse tax basket,
  used tax value, deficit amount covered by that tax value, and remaining
  carry-forward amount together instead of passing a loose remainder through the
  wage-earner path.
- `Par13NegativPersonligModregningSag` and
  `Par13FremførtNegativPersonligModregningSag` keep the § 13, stk. 4
  negative-personal-income offset order inside explicit legal cases: current
  year spouse personal income first, then own positive capital and spouse
  positive capital, while carried-forward negative personal income starts in
  the spouses' positive capital income before own and spouse personal income.
  `beregn_par13_kompleks` now returns both result objects instead of only the
  old single-person rest amount.
- `Par4aPensionsfradragSag` uses product-scoped `|` rules for the § 4 a,
  stk. 4 amount layer. The scope keeps positive share income, the requested
  pension deduction, notice to the tax administration, no-double-deduction
  status, capped deduction, remaining share income, and disallowed amount
  together while stable wrapper rules expose the calculation to audits.
- `PersonfradragPar10Sag` uses product-scoped `|` rules for § 10 eligibility.
  It keeps tax year, age status, tax-liability posture, partial-year election
  date, and reversal date together so the Kildeskatteloven § 2 full-year case,
  partial-year election deadline, omvalg deadline, sailor-tax exclusion,
  residence-permit applicant exclusion, and researcher-tax exclusion are derived
  from one legal case.
- `PersonfradragStatsskatNedsættelseSag` uses product-scoped `|` rules for
  § 12 stk. 2. It keeps the § 9 state-tax basket and state personfradrag tax
  value together so the unused value after § 6, § 7, § 7 a, § 8, and
  § 8 a stk. 2 is derived inside one legal case instead of passed through a
  chain of loose remainders. The ordinary wage-earner calculator now projects
  its state-tax component fields through this allocator, using tax-year-aware
  mapping because § 7 is topskat before 2026 and mellemskat from 2026 onward.
- `Par9StatsskatPersonfradragSag` uses product-scoped `|` rules for the
  amount-level § 9 reduction order. It keeps the state-tax basket, the § 8
  personfradrag tax value, and the § 6 personfradrag tax value together so
  each value first reduces its own tax component and then falls through the
  remaining § 9 taxes in the statutory order.
- `Par9IkkeStatPersonfradragResultat` covers § 9's "tilsvarende måde" sentence
  for § 8 c tax, municipal income tax, and church tax. The wage-earner
  calculator now delegates those final reductions to this result instead of
  subtracting scalar personfradrag values in place.
- `Par10Stk3ÆgtefælleStatsskatSag` uses product-scoped `|` rules for unused
  personfradrag tax-value transfer to a spouse. It keeps the unused § 8 and
  § 6 tax values, the receiving spouse's § 9 state-tax basket, and the
  year-end cohabitation condition together, then delegates the reduction order
  to `Par9StatsskatPersonfradragResultat` so the amount actually transferred,
  used, left unused, or barred by missing cohabitation stays in one domain
  result without duplicating § 9 mechanics.
- `Par7ReformMellemskatSag` uses product-scoped `|` rules for the 2026
  mellemskat amount layer. It keeps the § 20-regulated personal threshold, the
  § 20-regulated positive-capital grundbeløb, personal income, net-capital
  income, personal/capital split, and resulting tax together so wage-earner
  calculator rules no longer duplicate reform thresholds as loose parameters.
- `ReformPersonligStatsskatSag` uses product-scoped `|` rules for the 2026
  personal-only reform layers. It keeps the § 7 a topskat/§ 8 toptopskat lag,
  statutory 2010-level threshold, § 20-regulated threshold, personal income,
  excess personal base, rate, and kroner tax together so the wage-earner
  calculator and audits can inspect those taxes as named legal results instead
  of opaque scalar calls.
- `Par26Stk7TopskatÆgtefællerSag` uses product-scoped `|` rules for the
  § 26, stk. 7 transition-compensation bridge. It keeps both spouses'
  personal/PBL bases, net-capital incomes, regulated § 26 nr. 3 threshold,
  ordinary § 7 threshold, and § 7 capital allocation together so the old-vs-new
  top-tax difference is derived as a named result instead of a loose scalar.
- `Par26KompensationAfregningSag` composes the annual § 26 line-item
  calculation with the § 26, stk. 1 tax-offset order, so fixtures can prove the
  whole compensation-settlement path instead of separately calling
  `par26_forskelsbeløb_beregning_for_skatteår` and `par26_modregning_resultat`.
- `Par26ÆgtefælleForskelsbeløbParSag` applies the § 26, stk. 4 spouse rule in
  both directions, so a married couple's positive/negative transition
  differences produce named post-stk. 4 amounts and compensation for each
  spouse without caller-side direction selection.
- `Par26Stk5KapitalParSag` applies the § 26, stk. 5 spouse net-capital rule in
  both directions, so a couple's positive/negative net-capital income produces
  named post-stk. 5 amounts and offset totals before the nr. 2 transition
  amount is calculated.
- `Par26Stk6BundfradragParSag` applies the § 26, stk. 6 spouse bundfradrag
  transfer in both directions, so a couple's personal-plus-positive-capital
  income produces named missing-threshold amounts, threshold increases, and
  effective nr. 2 thresholds before the transition amount is calculated.
- `Par26ForskelsbeløbParÅrsSag` composes the annual § 26 year parameters with
  the stk. 6 pair threshold result and stk. 4 spouse difference offset, so
  fixtures can prove the effective nr. 2 threshold changes the actual annual
  line item before the post-stk. 3 spouse offset is applied.
- `Par26Stk8KapitalParSag` applies the § 26, stk. 8 spouse net-capital rule in
  both directions, so a couple's positive/negative net-capital income produces
  named post-stk. 8 amounts and offset totals before the nr. 8 transition
  amount is calculated.
- `Par11NegativKapitalNedslagSag` uses product-scoped `|` rules for § 11.
  It keeps tax year, the taxpayer's net-capital income, the spouse's
  net-capital income, samliv status, and municipal/§ 8 c tax-liability posture
  together so the own negative amount, spouse positive offset, spouse unused
  threshold, effective threshold, reduction base, § 11 stk. 2 rate result, and
  final reduction are derived from one legal case.
  `Par11NedslagModregningSag` separately keeps
  the statutory reduction order across §§ 6, 7, 7 a, 8, § 8 a stk. 2, § 8 c,
  municipal tax, and church tax.
- `Par6ÆgtefælleKapitalModregningSag` uses product-scoped `|` rules for the
  § 6, stk. 3 amount layer. It keeps marriage/samliv status, one spouse's
  positive net-capital income, and the other spouse's negative net-capital
  income together so the offset amount, reduced positive capital basis, and
  residual negative amount are derived from one legal case rather than passed as
  loose scalars.
- `Par7ÆgtefælleKapitalStk5Sag` uses product-scoped `|` rules for the § 7,
  stk. 5 capital-threshold layer. It keeps the taxpayer's net-capital income,
  the spouse's positive net-capital income, the regulated grundbeløb, and
  samliv status together so negative capital is offset before the spouse's
  effective threshold is increased.
- `Par7KapitalskatFordelingSag` keeps the § 7, stk. 10-11 allocation of
  spouse positive-net-capital tax together as a result object. It derives
  whether one or both spouses are over the grundbeløb and assigns the combined
  capital tax to the single over-threshold spouse or splits it by the statutory
  ratio. `par7_højeste_beregningsgrundlag` separately models the stk. 8/stk. 12
  identity rule, including the equal-basis tie-break by largest
  ligningsmæssige deductions.
- `Par8cSkatSag` uses product-scoped `|` rules for the § 8 c municipal
  equivalent tax. It keeps tax year, limited-taxpayer posture, taxable ordinary
  income, and the § 10 stk. 5 personfradrag amount together so coverage,
  taxable base, published rate, personfradrag tax value, and final § 8 c tax
  are derived from one legal case. `Par8cSatsResultat` separately keeps the
  statutory method, published source posture, source rate, and applied
  basispoint rate together so the calculation does not depend on a bare 25 pct.
  constant.
- `LønmodtagerSkatteforhold` groups non-ordinary taxpayer facts for the
  wage-earner calculator: share income and spouse share-income posture, CFC
  income, § 8 c posture, and § 10 personfradrag tax-liability/election facts.
  `LønmodtagerBeregningSag` uses product-scoped `|` rules to compose that tax
  position with the ordinary wage-earner base, so nonzero § 8 a high-layer
  share-income tax, § 8 b CFC tax, and § 8 c municipal-equivalent tax now flow
  through § 5 state-tax aggregation, § 10 personfradrag eligibility, § 13
  deficit tax-value posture, and § 19's municipal-or-§ 8 c rate input without
  adding more scalar fields to `LønmodtagerInput`. The public
  `beregn_lønmodtager` path now uses this scoped model with standard tax
  conditions, so standard fixtures and special tax-condition fixtures no longer
  diverge through separate model constructors.
- `slutopgoerelse.runa` keeps the year-end balance as `KildeskatSlutopgørelseInput`
  plus a statutory `KildeskatPar60Kreditter` credit basket. This is a better
  domain boundary than passing A-skat, AM-bidrag, B-skat, dividend-tax credits,
  voluntary payments, and special credit categories through every subrule.
- `KildeskatRestskatOpkrævningInput` now uses
  `KildeskatRestskatUdskrivningspostur` instead of a cluster of timing booleans.
  The enum models the statutory issuance posture directly and rules derive the
  § 61 branches from it, so impossible flag combinations cannot become fixtures.
- `KildeskatBSkatRateVindue` now groups the B-skat installment calendar
  projection for restskat collection. The preferred restskat input now composes
  `KildeskatDato` and `KildeskatRestskatSystemdatoer`, deriving both the
  statutory issuance posture and the first remaining B-skat rate instead of
  passing those as scenario literals. The lower-level input remains as a
  compatibility helper for focused edge-case audits.
- `KildeskatRestskatMinimumsplan` is the date-aware boundary for the § 61,
  stk. 5 command that January-or-later restskat paid over remaining B-skat rates
  must still be paid in at least three rates. It keeps the ordinary B-skat count,
  missing supplemental rate count, divisible fixture amount, supplemental due
  dates, total rate count, and § 58 last-timely-payment day together instead of
  scattering late-year calendar assumptions across audit rules.
- `KildeskatRestskatRateplan` and `KildeskatRestskatBeløbsdeling` now expose
  the executable installment schedule layer for § 61. The model records
  statutory branch, first/last due date, rate count, last-timely-payment day,
  and exact-vs-mixed large/small installment splits. `KildeskatRestskatSystemdatoer`
  now carries separate source-backed system-start dates for late stk. 4
  three-rate collection and stk. 6 residual collection, including the January
  18.300 kr. over nine remaining B-skat rates case with three 2.034 kr. rates
  and six 2.033 kr. rates.
- `KildeskatRestskatTreRateSag` uses product-scoped `|` rules for the § 61
  stk. 4/stk. 6 three-rate case. The scope keeps the shared derived
  `opkrævning`, system-start dates, installment count, and amount splitting
  together while the public wrapper rules keep the surrounding file stable.
- `aktieindkomst-slutopgoerelse.runa` now owns `AktieindkomstSlutopgørelseCase`
  as reusable product-scoped `|` rules for the § 8 a/§ 67 annual-settlement
  case. The scope keeps the wage-earner breakdown, monthly A-skat, share
  income, spouse share-income threshold facts, and withheld dividend tax
  together while methods derive the effective progression threshold, final
  low-layer share tax, high-layer tax entering final tax before and after § 12
  state personfradrag allocation, § 60 credit basket, and final annual-settlement
  result. The low-wage/high-share scenario now audits that unused state
  personfradrag tax value can reduce the
  § 8 a, stk. 2 amount before Kildeskatteloven final settlement.
  The source-law module now keeps § 8 a stk. 1/stk. 2 share-income tax rates,
  source paragraph, final-tax posture, and slutskat-entry posture in one typed
  rate result.
- `AktieindkomstNegativSlutopgørelseCase` keeps § 8 a, stk. 5-6 negative
  share-income settlement separate from the positive-share case. It derives
  spouse positive-share offset, negative tax, whole dividend-tax credit for
  negative share income, own-slutskat offset, spouse-slutskat offset, and the
  carried-forward remainder while crediting only the amount actually usable in
  the taxpayer's current-year § 60 basket.
- `AktieindkomstUdbytteskatStk3Sag` uses product-scoped `|` rules for
  Personskatteloven § 8 a, stk. 3. It keeps tax year, total share income, and
  dividend tax withheld under Kildeskatteloven § 65 together while defaults and
  exceptions derive the low-rate comparison amount, any over-withheld amount
  credited in slutskatten, the negative-share-income full credit, and the
  remaining final dividend-tax payment.
- `Par7aUdligningsskatPostOpgørelseSag` and
  `Par7aUdligningsskatBeløbsSag` use product-scoped `|` rules for
  Personskatteloven § 7 a. They keep the enumerated pension and pension-like
  payments as amount-carrying posts, exclude invalidity pension, efterløn,
  fleksydelse, førtidspension, and mandatory foreign security schemes before
  calculating the stk. 1 amount, and feed that source-derived amount into the
  existing stk. 3, stk. 4-5, and stk. 6 udligningsskat calculation. The
  historical phase-out rate now carries the § 7 a, stk. 5 source paragraph and
  phased-out posture in one typed result.
- `AktieindkomstÆgtefællerBeggeNegativeSag` uses product-scoped `|` rules for
  Personskatteloven § 8 a, stk. 6. It keeps both spouses' negative share income
  and samliv status together so the double share-income threshold is split
  proportionally when both spouses are negative, and each spouse's negative tax
  is calculated from the allocated threshold instead of granting the double
  threshold twice.
- `KonfiskatoriskCase` uses product-scoped `|` rules for the effective-rate
  audit. It keeps tax year, municipality, wage, positive net-capital income,
  share income, spouse share-income posture, church-tax membership, and
  transferred restskat m.v. together while deriving the wage-earner input,
  § 8 a final-settlement result, positive income denominator, current-year
  tax basis points, and broader payment-burden basis points from one case.
- `KildeskatPar62ARenteDatoInput` and
  `KildeskatPar62AForsinketUdbetalingsdatoInput` keep § 62 A issue and payout
  scheduling date-based. They derive the old "påbegyndte måneder" helper input
  from legal dates, which keeps scenario files from smuggling calendar math in
  as precomputed integers.
- `OpkrævningPar7OffentliggjortSats` is the annual published-rate domain row
  for settlement examples. The Nationalbank July/August/September formula stays
  executable, while live Kildeskatteloven fixtures use the Skattestyrelsen
  source row instead of synthetic monthly-rate literals.
- `opkrævning_par7_stk1_forsinkelsestillæg_basispoint` now captures the
  2026 source-chain amendment from 0,7 to 0,85 procentpoint as a temporal
  exception, and `opkrævning_par7_stk1_månedlig_forsinkelsesrente_basispoint`
  derives the combined monthly late-payment rate from the published § 7, stk. 2
  row plus that supplement.
- `OpkrævningDato`, `OpkrævningPar7DagligRenteInput`,
  `OpkrævningPar7DagligRenteKontekst`, and
  `OpkrævningPar7DagligRenteBeregning` are now the right-sized domain boundary
  for § 7 daily late-payment interest. The context carries principal, latest
  timely payment date, actual payment date, calendar-year rate row, supplement,
  day-count convention, and rentedage together, avoiding loose date/rate
  parameters being passed down merely so subrules can project them.
- `OpkrævningPar7DagligRenteÅrsdel` and
  `OpkrævningPar7DagligRenteTværårBeregning` extend that boundary across
  New Year. Each segment carries its own calendar-year published-rate row,
  supplement, year divisor, rentedage, and `PengeØreBeregning`, so a cross-year
  payment cannot accidentally price 2025 days with a 2026 rate or divisor.
- `pengebeloeb.runa` is now the shared precision boundary for positive clamps,
  minimum amounts, ten-kroner floors, basispoint rounding, basispoint-to-kroner
  multiplication, and øre-fraction rounding. Statutory modules retain their
  local legal names but delegate the common arithmetic posture to this file.
- `LønmodtagerSkatteloftResult` is now the right-sized § 19 boundary inside the
  ordinary wage-earner breakdown. It carries the derived `Par19SkatteloftInput`,
  excess basis points, and kroner relief together for both personal and
  positive-capital skatteloft paths instead of adding loose scalar fields to the
  calculator surface. The § 19 personal and positive-capital ceilings now also
  project from typed `Par19SkatteloftResultat` values that preserve whether the
  rate comes from LBK nr. 1284/2021 § 19 stk. 1/stk. 2 or the 2026 LOV nr.
  482/2024 § 1 nr. 15 rewrite.
- `LønmodtagerBeregning` now composes the ordinary wage-earner calculation from
  named domain records: income basis, Ligningsloven deductions, tax before
  person allowance, § 5 state-tax aggregate before person allowance, person
  allowance tax value, tax after person allowance before skatteloft, and final
  tax after § 19 relief. The existing flat
  `LønmodtagerBreakdown` remains as the reporting/API projection so website and
  scenario consumers do not have to learn every internal calculation layer.
- `LønmodtagerPar13Sag` is the current right-sized § 13 boundary inside the
  ordinary wage-earner calculator. It keeps the taxpayer input and immediate
  spouse-transfer facts together while internal `|` rules derive the taxable
  deficit, tax-value rate, statutory tax-value offset order, deficit amount
  covered by own tax offset, remaining deficit, spouse deduction, spouse tax
  value offset after the income deduction, and carry-forward remainder. The
  focused `loenmodtager-par13-spouse.audit.runa` case verifies that the
  remaining 8.283 kr. deficit after spouse income deduction is further reduced
  by 1.000 kr. of spouse tax-value offset to 4.028 kr. carry-forward. Scenario
  fixtures stay as plain taxpayer/spouse facts plus assertions; later-year
  priority between a spouse's own prior deficits and another spouse's carried
  deficits should be modeled as a separate carry-forward-year case, not folded
  into first-year fixture setup.
- `LønmodtagerPar11NedslagResultat` is the current right-sized § 11 boundary
  inside the ordinary wage-earner calculator. `LønmodtagerInput` now carries
  `nettokapitalindkomst_kroner`; helper rules derive positive net-capital income
  for progressive positive-capital branches and negative net-capital income for
  § 11. The focused `loenmodtager-par11.audit.runa` case verifies that a 2026
  København wage-earner with -40.000 kr. net capital receives a 3.200 kr. § 11
  reduction before final tax.
- `loenmodtager_beregning.runa` now separates state income tax, municipal/church
  income tax, total income tax, and the final total including AM contribution.
  This keeps Personskatteloven § 14's "helårsskat efter §§ 6-9" from
  accidentally consuming an AM-inclusive cash-flow total.
- `LønmodtagerPar14Input` is the current right-sized § 14 boundary for ordinary
  wage-earner partial-year cases. It carries tax-liability change status, the
  delårs wage-earner input, and tax-liability days together, instead of passing
  those scalars through every helper rule.
- `Par14SkatteberegningResultat` is now the reusable statutory § 14 amount
  boundary. It carries helårsindkomst, helårsskat efter §§ 6-9, the stk. 1/stk. 3
  proportional delårsskat, the stk. 2 period-reduced tax, the governing election
  posture, and the final `skat_efter_par14_kroner`.
- `Par14Beløbspost` now captures whether a § 14 amount is recurring or one-off.
  This keeps the official annualisation example from forcing one-off income
  through the same annualisation path as wage, interest, or A-kasse amounts.
- `KildeskattebekendtgørelseForskudskortInput` now composes a named
  `KildeskattebekendtgørelseForskudskortIndkomstgrundlag` instead of passing
  annual basis, period basis, and excluded basis as loose scalars. The source
  backed Skat.dk fixture showed why this matters: generated tax-card values
  must use the AM-reduced A-tax basis for ordinary wages, not the gross wage
  amount. `KildeskatESkattekortInput` now names that field
  `a_skat_grundlag_kroner` so downstream rules do not smuggle gross A-income
  semantics into withholding calculations.
- `parameterpakke_komplet` now depends on a year+municipality coverage rule
  rather than a broad municipality predicate. This keeps the parameter-pack
  domain honest as new municipalities are added for selected years; Langeland is
  currently source-backed for 2026 only.
- Domain review pass: exact duplicate scalar helpers for positive amounts,
  minimum amounts, and kroner-by-basispoint calculations now route through
  `pengebeloeb.runa`. Wider statutory input records stay explicit where their
  fields are enumerated legal facts rather than accidental parameter plumbing.
- The household scenario helpers are a future candidate for a compact scenario
  input object if more household scenarios are added. With one scenario, the
  current explicit facts remain easier to audit than a new wrapper layer.

Review candidates to revisit deliberately, not as broad churn:

- `KildeskatESkattekortInput` and the generated-card result may eventually
  share smaller card-period and withholding-percentage objects, but the current
  BEK 1094 slice keeps the annual 2026 percentage derivation as its own domain
  object rather than forcing a broader card refactor prematurely.
- `ArbejdsmarkedsbidragUdvidetLønmodtagerInput` and
  `ArbejdsmarkedsbidragVirksomhedsordningInput` are wide, but they still mirror
  dense statutory enumerations closely enough that premature grouping could hurt
  source traceability.

## Now

- Close the Personskatteloven implementation gaps before deeper audits. The next
  work should identify the remaining posture-only/first-slice legal areas and
  turn the highest-value ones into source-backed calculation rules.
- Continue deepening dependency laws such as Kildeskatteloven, AM-law,
  municipal/church tax, Ligningsloven, and Opkrævningsloven only where they
  unblock Personskatteloven calculation completeness or validate a newly
  implemented legal slice.
- Keep validation audits close to the implementation. Exploratory daisy-chain,
  confiscatory, household-benefit, minimum-retained-income up to 2 mio. kr., or
  loophole searches belong after the main law model is more complete.
- Keep reviewing domain boundaries as each slice grows. Encapsulate repeated
  legal facts when they are genuine statutory objects, but avoid broad refactors
  that would make source traceability weaker.
- Preserve original Danish legal text in multiline comment/source blocks above
  every Futuruna translation.
- Model ordinary legal statements primarily as `|` rules, using `under` for
  conditions and `exception` for overrides.
- Allow verification and audit files to break while the model is being
  reformulated; then repair them as milestone work rather than weakening the
  legal encoding.

## Next

- Deepen the first-pass full-statute corpus from structural coverage into
  calculation coverage where official fixtures and dependent statutes make that
  safe.
- Replace remaining source-dependency placeholders with complementary official
  statutes and trusted calculation examples, especially for municipal/church
  settlement edge cases, direct Nationalbank raw-data ingestion beyond the
  Skattestyrelsen-published Opkrævningsloven § 7 annual rate, and remaining AM
  edge cases beyond the first source-explicit special-case slice. The AM-law
  slice now covers ordinary wage remuneration,
  taxable benefits, § 3 exclusions, self-employed bases with and without
  virksomhedsordning, library-fee compensation, the 2026 youth exemption, and
  collection-reference posture. The first municipal/church
  slice now covers ordinary municipal tax on Personskatteloven taxable income
  and church tax for Folkekirken members. The first Kildeskatteloven slice now
  covers ordinary wage A-income, withholding duty, e-skattekort card types,
  main-card period allowances, bikort without allowance, optional higher
  withholding percentage, base rounding, and the statutory 55 pct. no-card
  fallback. The first BEK 839 slice now generates skattekort values from
  forskudsskat plus an unrounded withholding percentage. The first BEK 1094
  slice now derives the 2026 unrounded withholding percentage from municipal,
  church, bundskat, mellemskat, topskat and toptopskat components. The
  fictional household scenario now computes monthly A-skat and cash-flow payroll
  output both from supplied e-skattekort allowance/procent inputs and generated
  BEK 839 card values using the BEK 1094-derived percentage. The first
  Opkrævningsloven slice now covers ordinary and large-withholder A-skat/AM
  payment deadlines, late payment posture, provisional assessment posture, and
  the § 7 stk. 2 annual-rate formula from July/August/September Nationalbank
  kassekreditrente inputs plus the Skattestyrelsen-published 2026 annual rate
  and the 2026 source-chain amendment to the § 7, stk. 1 late-payment
  supplement.
  The first Kildeskatteloven slutopgørelse slice now covers § 60 crediting,
  § 61 restskat plus percentage supplement, timing posture, B-skat/restskat
  rateplans with large/small installment splits, including system-date-driven
  late stk. 4 and stk. 6 three-rate plans, § 62
  overskydende skat plus compensation/refund posture, § 60 spouse offsetting,
  § 58 B-skat calendar projection, § 62 A amended annual statement interest
  posture with date-derived month counts, § 62 C minimum thresholds, and § 67
  dividend-tax credit posture; the
  fictional household's generated-card annual settlement currently yields
  3.541 kr. overskydende skat and 3.541 kr. payout under the source-derived
  § 7 rate fixture.
  The first bomb-audit probes now formalize nine daisy-chain tensions: § 6
  spouse negative net-capital offset can lower the other spouse's bundskat
  basis; § 7 stk. 5 negative net capital can both offset the spouse's positive
  net-capital income and raise the spouse's effective positive-capital
  threshold, removing 6.375 kr. mellemskat in the probe; § 14 helårsomregning
  can increase the state-tax component for the 180-day
  wage-earner case by 3.293 kr.; a high municipal rate can lower the state-tax
  component through § 19 while still increasing total tax; the 2026 personlige
  skatteloft sits 10,83 percentage points below the full
  mellemskat/topskat/toptopskat progression stack in the Copenhagen probe; §
  8 a unused spouse share-income threshold can remove the high share-income
  tax bracket; § 8 a mandatory negative share-income spouse offset is not
  neutral in the family-net probe; § 8 b CFC tax sits outside both § 9
  personfradrag and § 19 skatteloft in the executable model; and § 13 can lock
  passive business losses to same-business carry-forward while active
  participation releases the same amount into current other-income deduction.
  A separate confiscatory effective-rate audit now searches 8.064 bounded
  year/municipality/income/payment configurations. It finds no encoded
  current-year `årsskat` above current positive wage/capital/share income in
  that grid, while it finds 360 configurations above 100 pct. when transferred
  restskat m.v. under Kildeskatteloven is included as a payment burden. The
  highest current-year `årsskat` rate in the grid is 52,63 pct. in a 2026
  Langeland high-wage church-tax case; the highest payment burden is 215,91
  pct. in a 2024 Copenhagen low-income share-income case with 150.000 kr.
  transferred restskat m.v.
  A first cross-law household benefit audit now covers Børne- og ungeydelse:
  the fictional three-child household has 48.216 kr. annual benefit before
  aftrapning and no reduction at the current wage levels, while a parent 100
  kr. over the 2026 mellemskat-linked threshold loses 2 kr. of that parent's
  own half and one fully phased-out parent leaves the other parent's 24.108 kr.
  half intact. The same audit also flags a source-wording tension: the current
  Retsinformation § 1 a and Borger.dk posture point to the mellemskat-linked
  reduction base for 2026, while the Skatteministeriet rate page still describes
  the reduction income as the topskat basis.
  The same audit now covers a first boligsikring § 22 cliff: at 80.000 kr.
  annual housing expense and 215.000 kr. household income, one child gives no
  child increment to the income threshold and leaves an 8.046 kr. income
  deduction, while the second child raises the threshold enough to remove that
  deduction in the encoded 2026 slice.
  § 13's first dependent-source slice now covers
  Pensionsbeskatningsloven § 16, Ligningsloven § 33 A,
  Sømandsbeskatningsloven §§ 5-8, the 2026 repeal in LOV nr. 482/2024, and
  LOV nr. 482/2024's reform insertion of § 8/toptopskat into the § 13
  modregningsrækkefølge.
  § 4 a's amount-level audit now covers included and excluded share-income
  posts, § 19 B-to-§ 17 personal-income reclassification, negative share-income
  preservation and pensionsfradrag in positive share income: the deduction is
  capped at positive share income, requires notice to the tax administration,
  is blocked if already deducted in personal income, and is unavailable without
  positive share income.
- Add more trusted external differential fixtures after the first § 14/§ 19
  external slice. The ordinary 2026 Copenhagen wage-earner path now has a
  source-backed Skat.dk calculator fixture for final tax and generated tax-card
  values. The official § 14 guidance example now verifies annualisation
  rounding and one-off income handling, and the source-backed Langeland 2026
  high-municipal-rate fixture now compares § 19 personal relief against the
  published 1,24 pct. municipal `Nedslag pct.` while exercising both personal
  and positive-capital relief inside the wage-earner calculator.
- Separate legal structure from annual parameter packs: rates, thresholds,
  personal allowances, municipal tax, church tax, and other tax-year data.
- Build calculation fixtures for ordinary wage-earner cases before handling
  complex cases.
- Gather complementary official sources for:
  municipal and church-tax settlement/allocation, personal allowance, automated
  Opkrævningsloven § 7 input lookup from Nationalbank data, date-exact B-tax
  remaining-rate selection, date-exact § 62 A issue/payout scheduling, remaining
  AM edge cases, other
  itemized deductions beyond the ordinary §§ 9 J/9 K wage-earner deductions and
  the first § 9 L extra-pension slice,
  and annual rate/threshold adjustments.
- Keep existing audits running as implementation validation, but defer expanded
  source-drift, delegated-power, confiscatory, household-benefit, and
  daisy-chain searches until the main Personskatteloven calculation model has
  fewer first-slice gaps. The existing audit files remain useful guardrails; they
  should not lead the next milestone.
- Extend the website page as more of the corpus becomes calculation-ready.

## Later

- Encode spouse rules, partial-year taxation, pension interaction, share income,
  CFC income, business income, property-related income, and special regimes.
- Build a normal-person income tax calculator backed by the Futuruna rules and
  tax-year parameter packs.
- Add differential checks against official examples or trusted calculators where
  legally safe and sourceable.
- Extend the Retsinformation update automation with optional reviewed patch
  generation after the fetch/detect/report workflow has been used a few times.
- Expand audits into legal "bomb" discovery: confiscatory effective rates,
  cliff effects, hidden delegations, obsolete provisions, incoherent categories,
  and temporal contradictions between consolidated law and annual parameters.
- Integrate the mature corpus into the website alongside the Danish
  Constitution research pages.

## Milestones

M0 - Source foundation

- Status: first slice implemented.
- Output: this status log plus a checked source-status `.runa` file.
- Done when: the project records current and historic source posture and has a
  passing Futuruna file that prevents historic law from being used for live tax
  calculation.

M1 - Income taxonomy

- Status: first slice implemented.
- Output: chapter/foundation `.runa` file for §§ 1-4 b.
- Done when: ordinary income, personal income, capital income, share income, and
  CFC income are represented as typed legal categories and amount-level result
  records with original text preserved.
- Current slice: § 1/§ 2 ordinary taxable-income composition is executable as a
  named result over personal income, capital income, share income, CFC income,
  and ligningsmæssige fradrag. The fixture proves § 4 a share income remains
  outside ordinary taxable income and § 4 b CFC income remains outside the
  §§ 6-8 a taxable-income base while reclassified § 4/§ 4 a amounts feed
  personal income.

M2 - State tax computation skeleton

- Status: first slice implemented.
- Output: `.runa` files for §§ 5-9.
- Done when: the legal structure of bundskat, mellemskat, topskat, toptopskat,
  abolished/zeroed taxes, aktieindkomstskat, CFC tax, and
  municipal-equivalent state tax is encoded.
- Current slice: 2026 LOV nr. 482/2024 state-tax structure is represented:
  § 5 now sums typed state-tax component posts into an amount-level
  `Par5StatsskatResultat`, filtering inactive components by tax year so
  udligningsskat/sundhedsbidrag are ignored from 2026 and mellemskat/
  toptopskat are ignored before the 2026 reform. Mellemskat under § 7,
  topskat under § 7 a, and toptopskat under § 8 now derive their 2026
  thresholds from typed `ReformStatsskatParameterResultat` values carrying the
  LOV nr. 482/2024 § 1 nr. 2-4/nr. 5/nr. 6 source branch, then regulate the
  amendment's 2010-level amounts through § 20 before feeding the calculator,
  with § 7 a and § 8 now exposed as named personal-income amount results; CFC
  tax under § 8 b can feed this state-tax
  component model, with § 8 b consuming the amount-level § 4 b CFC-income
  result before applying a structured Selskabsskatteloven § 17, stk. 1 rate
  result.
  The § 6 slice now computes the amount-level spouse negative net-capital
  offset before bundskat basis calculation, and its rate accessor now projects
  a source-backed `BundskatSatsResultat` covering the 2021 LBK rate plus the
  2022/2023/2024 statutory reductions to 12,09/12,06/12,01 pct. and the
  corresponding 4,09/4,06/4,01 pct. no-municipal-liability rates.
  The § 7 mellemskat slice now covers positive net capital income over the
  § 20-regulated 2026 threshold, including an executable spouse doubled-threshold
  case and the § 7 stk. 5 rule that negative net capital is offset against the
  spouse's positive net-capital income before the spouse's effective
  grundbeløb is increased. It now also exposes the § 7 stk. 10-11 allocation of
  the combined spouse capital tax and the stk. 12 tie-break for equal stk. 7
  beregningsgrundlag.
  The historical § 7 a udligningsskat slice now computes the amount-level
  tax from the regulated 2010-level grundbeløb, the stk. 3 corrected-personal-
  income cap, the stk. 6 spouse grundbeløb increase with the 121.000 kr.
  regulated cap, and the 2011-2018 phase-out rates.
  The historical § 8 sundhedsbidrag slice now computes amount-level tax from
  skattepligtig indkomst, the 2010-2019 rate phase-out, and the stk. 2
  municipal/§ 8 c liability condition. Both historical rate ladders now expose
  source-backed rate results with percentage, basispoints, and phased-out
  posture while preserving the scalar basispoint accessors used by the
  calculators.
  The ordinary wage-earner domain model now carries § 7 a udligningsskat and
  § 8 sundhedsbidrag as explicit state-tax component slots before and after
  personfradrag allocation, and feeds those slots into the § 5 aggregate.
  It also exposes ordinary zero-default § 8 a aktieindkomstskat, § 8 b
  CFC-indkomstskat, and § 8 c kommunal-lignende statsskat slots through the
  same § 5 aggregate boundary, so later nonzero special-income integration has
  a stable domain home.

M3 - Tax-year parameter packs

- Status: first slice implemented.
- Output: sourceable annual tables for rates, thresholds, allowances, and
  municipality-specific inputs.
- Done when: the same legal rules can run for at least two tax years by swapping
  parameter packs.
- Current slice: 2024, 2025, and 2026 national parameters now return
  `NationalParametreResultat` values with Skattestyrelsen source branches.
  Copenhagen and Gentofte municipal/church-tax inputs for 2024-2026, plus the
  2026-only Langeland municipal row, now return `KommunaleParametreResultat`
  values with Skatteministeriet source branches. Combined
  `SkatteårParameterpakkeResultat` values carry both national and municipal
  source provenance before projecting the existing plain parameter pack. The
  2026 pack covers mellemskat, topskat, toptopskat, personfradrag,
  aktieindkomst, skatteloft, municipal rates, church-tax rates, published
  skatteloftsnedslag, and grundskyldspromille. Parameter completeness is
  year+municipality specific, so Langeland is not treated as supported for
  2024/2025 until those rows are source-backed.

M4 - Ordinary taxpayer calculator

- Status: first slice implemented.
- Output: fixtures and executable examples for wage income, capital income,
  deductions, municipal tax, church tax, and AM contribution.
- Done when: normal cases produce reproducible tax breakdowns with source-backed
  assumptions.
- Current slice: 2025 Copenhagen and Gentofte wage-earner fixtures produce
  deterministic AM contribution, personal income after AM, ordinary taxable
  income through the § 1/§ 2 `Par1AlmindeligSkattepligtigIndkomstResultat`
  after derived Ligningsloven §§ 9 J/9 K wage-earner deductions,
  bundskat, topskat, municipal tax, church tax, § 10 personfradrag, § 12
  personfradrag tax values and § 9/§ 12 state-tax allocation,
  after-personfradrag totals, and a § 13 ordinary-positive-income boundary.
  Separate § 13 calculator breakdown fixtures now cover spouse-transfer deficit,
  LL § 33 A relief, 2026 post-PBL-repeal
  transfer, and same-business loss carry-forward cases. 2026 Copenhagen
  wage-earner fixtures now exercise mellemskat, topskat, and toptopskat under
  the LOV nr. 482/2024 reform thresholds, with topskat and toptopskat routed
  through named personal reform results, and a 2026 Copenhagen positive
  net-capital fixture exercises the mellemskat capital addition. The wage-earner
  model now routes state income tax before personfradrag through the
  `Par5StatsskatResultat` aggregate, so the ordinary calculator consumes the
  same § 5 active-component filtering as the source-law module instead of a
  parallel scalar sum. The public wage-earner model now delegates through
  `LønmodtagerBeregningSag` with standard tax conditions, and the audit model
  verifies that a Kildeskattelov §§ 48 E/48 F researcher-income posture carries
  § 10's personfradrag exclusion all the way through final tax. Its breakdown
  now includes `LønmodtagerSkatteloftResult`,
  so § 19 personal and
  positive-capital skatteloft input, excess basis points, and kroner relief are
  part of ordinary 2025/2026 calculator output. The 2026 Copenhagen
  positive-net-capital fixture now applies the 42 pct. positive-capital ceiling,
  reducing final tax by 106 kr.; current Copenhagen/Gentofte personal-income
  fixtures are explicitly under the personal ceiling. 2026 Langeland
  wage-earner fixtures now exercise source-backed high-municipal-rate § 19
  relief: 124 basis points and 2.316 kr. personal relief on a 900.000 kr. wage
  case, plus 381 basis points and 449 kr. positive-capital relief on a
  650.000 kr. wage plus 110.000 kr. capital-income case. The ordinary wage-earner
  AM contribution now imports Arbejdsmarkedsbidragsloven instead of
  using a local arithmetic shortcut, and the AM-law module now has
  source-backed special-case fixtures for § 3 exclusions, self-employed bases,
  library-fee compensation, and the 2026 youth exemption. Ordinary municipal
  income tax and church tax now import Kommuneskatteloven and Folkekirkens
  økonomi instead of using
  local arithmetic shortcuts. Ordinary Ligningsloven employment/job deductions
  now import the Ligningsloven dependency slice instead of being manual zeroes,
  and the dependency slice now also models § 9 L extra pension deductions for
  direct use by Personskatteloven § 26 nr. 5.
  A first fictional household scenario now computes a 2026 Copenhagen married
  renter household with 50.000 kr./month primary wage income and 20.000
  kr./month spouse wage income. A first household benefit-cliff audit covers
  Børne- og ungeydelse for the three-child scenario and a first boligsikring
  § 22 child-threshold cliff while explicitly marking broader housing-support
  and other deduction discovery as outside the current executable slice.
  Kildeskatteloven now marks the primary wage as A-income,
  proves that A-skat must be withheld, computes the statutory 55 pct.
  withholding if no e-skattekort, bikort or frikort has been received, and
  computes monthly A-skat/cash-flow payroll output when e-skattekort
  allowance/procent inputs are supplied. BEK 839 now generates the household's
  main-card monthly allowances from forskudsskat and BEK 1094-derived
  withholding percentage inputs, producing a separate generated-card payroll
  view.
  Opkrævningsloven now provides source-backed payment-deadline/remittance rules
  plus the § 7 annual-rate formula and date-exact daily late-payment interest
  context, with fixtures separated into `indeholdelse-afregning.scenario.runa`
  where they are scenario facts. The § 13
  complex calculator input now uses domain objects for income basis, tax-value
  rates, offset taxes, spouse transfer, stk. 5 limits, and same-business loss
  facts. `slutopgoerelse.scenario.runa` now also computes the fictional
  household's generated-card overskydende skat compensation and payout, plus a
  low-withholding restskat path with supplement and next-year transfer posture.
  Kildeskatteloven now also exposes the § 58 B-skat calendar as a rate-window
  domain object, § 62 A interest fixtures, and a restskat minimum-rate tension
  plus completion plan when remaining B-skat rates are too few, and separates
  late § 61 stk. 4 and stk. 6 system-start dates in executable restskat
  rateplans.
  `delaar-scenarier.scenario.runa`
  now runs a 2026 Copenhagen § 14 partial-year wage-earner case, annualizing
  180 days of wage income and applying the reduced §§ 6-9 state-income-tax
  result while keeping AM outside the § 14 helårsskat component.
  `aktieindkomst-slutopgoerelse.runa` now composes Personskatteloven § 8 a
  with Kildeskatteloven § 67 as reusable calculation rules; the corresponding
  scenario file supplies fictional wage-earner fixtures:
  150.000 kr. share income with the spouse's unused share-income threshold stays
  in the 27 pct. final-tax layer, while the high-tax variant splits 21.438 kr.
  final low-layer tax from 29.652 kr. high-layer tax entering slutskat and
  leaves 7.900 kr. restskat after 19.062 kr. dividend-tax credit. The scenario
  now builds a `Par5StatsskatResultat` for slutskat-bound state-tax components,
  excluding the final low-layer § 8 a tax while including the high-layer
  `Aktieindkomstskat` amount before § 12 personfradrag allocation. The
  source-law module now keeps source-backed § 8 a stk. 1/stk. 2 rate results
  for the 28/27/42 pct. rates and also covers § 8 a, stk. 3 as a separate
  scoped rule case
  for over-withheld dividend tax and negative-share-income full credit, and
  § 8 a, stk. 6 for both-negative spouse share-income cases where the double
  threshold is split proportionally. It now also covers negative-share-income
  final settlement: a 120.000 kr. negative-share case is fully absorbed by own
  slutskat, while a 900.000 kr. negative-share case offsets 208.726 kr. in own
  slutskat, 50.000 kr. in spouse slutskat, and carries 95.454 kr. forward. A
  paired settlement case now derives the spouse's own annual settlement before
  applying the § 8 a, stk. 5 negative-tax credit: 150.000 kr. spouse positive
  share income is first offset against a 900.000 kr. negative-share case, the
  remaining negative tax offsets 208.726 kr. in own slutskat and 71.005 kr. in
  spouse slutskat, and 11.449 kr. is carried forward.
  Personskatteloven § 8 c now computes the municipal-equivalent tax for covered
  limited-taxpayer postures, using the Skatteministeriet-published 25 pct.
  2026 rate and the same personfradrag reduction posture as § 10 stk. 5.
  Personskatteloven § 8 b now keeps the Selskabsskatteloven § 17, stk. 1
  historic/current source line, 22 pct. ordinary selskabsskat rate, 3
  percentage-point kulbrinte supplement, and applied CFC rate in one result
  object.
  `LønmodtagerBeregningSag` now exercises a nonzero 2026 CFC/§ 8 c
  tax-position path: 500.000 kr. CFC income feeds 110.000 kr. § 8 b tax into
  the § 5 aggregate, § 8 c replaces ordinary municipal income tax for the
  limited-taxpayer posture, § 10 stk. 5 personfradrag reduces the § 8 c amount,
  and § 19 uses the 25 pct. § 8 c rate in place of a municipality rate.
  Personskatteloven § 11 now computes negative net-capital-income reduction
  with spouse threshold pooling, spouse positive-net-capital offset before
  threshold increase, source-backed stk. 2 rate provenance, statutory tax-order
  reduction, and unused spouse transfer.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, covered § 1/§ 2 taxable-income
  composition from separate income categories, covered normal-fixture
  personfradrag, covered § 10 stk. 5-6 choice/deadline/exclusion posture,
  covered § 10 stk. 3 spouse transfer of unused personfradrag state-tax value,
  covered § 11 negative net-capital reduction with source-backed stk. 2 rate
  provenance, spouse threshold pooling, spouse positive-capital offset,
  statutory reduction order, and unused spouse transfer, covered § 9/§ 12 split
  state personfradrag tax-value reduction
  order, § 9 non-state personfradrag reduction, and wage-earner component
  projection,
  covered source-backed 2024-2026 national/municipal parameter-pack provenance,
  covered 2026 state-tax reform parameter source branches, covered § 19
  personal and positive-capital skatteloft source branches across the LBK text
  and LOV nr. 482/2024 rewrite, covered § 20 regulation-number source branches
  and corrected 2020-2024 regulation figures against SKM's historical table,
  covered § 6 source-backed bundskat-rate
  provenance across the 2022/2023/2024 amendment chain,
  covered § 13 deficit mechanics,
  § 13 stk. 4 negative-personal-income spouse and carry-forward offset order,
  mellemskat positive-net-capital and spouse-threshold activation,
  § 7 stk. 5 spouse negative-capital offset/effective-grundbeløb activation,
  § 7 stk. 10-11 spouse capital-tax allocation, and § 7 stk. 12 tie-break,
  historical § 7 a udligningsskat amount calculation with post-level stk. 1
  included/excluded pension-like payments plus stk. 3 and stk. 6 spouse-threshold
  cases and source-backed phase-out rate provenance,
  historical § 8 sundhedsbidrag amount calculation with liability, zero-rate
  boundary cases, and source-backed phase-out rate provenance,
  wage-earner domain-model projection of explicit § 7 a/§ 8, positive § 8 a
  high-layer share-income tax, and § 8 b/§ 8 c state-tax slots through § 5
  aggregation and § 9 personfradrag allocation,
  ordinary wage and special-case AM-law coverage, ordinary Ligningsloven §§ 9 J/9 K
  wage-earner-deduction coverage plus § 9 L/§ 26 nr. 5 validation coverage,
  ordinary municipal/church-tax legal coverage,
  covered Kildeskatteloven ordinary A-income/withholding/e-skattekort posture,
  covered BEK 839 forskudskort generation, covered BEK 1094 2026
  indeholdelsesprocent derivation, covered Kildeskatteloven slutopgørelse
  balance/restskat timing/system-date-driven § 61 stk. 4/stk. 6 rateplans/
  overskydende skat compensation/dividend-tax credit posture, covered § 8 a
  source-rate provenance and share-income final-settlement scenarios with § 67
  dividend-tax credit
  splitting plus § 8 a, stk. 3 over-withheld/negative-share-income dividend-tax
  credits and § 8 a, stk. 6 both-negative spouse threshold allocation, covered
  § 8 b CFC tax source-rate provenance from the historic/current
  Selskabsskatteloven § 17, stk. 1 source line,
  § 8 c municipal-equivalent limited-taxpayer tax with
  personfradrag reduction, published 2023-2026 rate source/method provenance,
  and non-covered boundary case, covered fictional
  household scenario, covered Børne- og ungeydelse
  benefit-cliff/source-tension audit plus a first boligsikring § 22 threshold
  cliff, covered external Skat.dk 2026
  ordinary wage-earner fixture, topskat threshold activation,
  covered § 4 a share-income aggregation, exclusions, personal-income
  reclassification and pension deduction from positive share income,
  covered § 14 annualization and first wage-earner calculator integration plus
  the Den juridiske vejledning external annualisation example,
  covered first bomb-audit probes for § 6/§ 7/§ 8 a/§ 8 b/§ 13/§ 14/§ 19 daisy-chain tensions,
  covered § 19 skatteloft including the 2026
  44,57 pct. personal ceiling, 42 pct. positive-capital ceiling, and
  calculator-level wage-earner integration for both paths, including
  source-backed Langeland 2026 high-municipal-rate personal and positive-capital
  relief fixtures and the published 1,24 pct. SKM `Nedslag pct.` differential,
  covered § 20 regulation/rounding, covered § 26 transition
  compensation including a composed annual settlement path, pair-level stk. 4
  spouse difference offset, and stk. 7 spouse top-tax allocation for nr. 3,
  covered § 28 territorial exclusion, covered AM-law special cases,
  covered shared Pengebeløb rounding and øre-fraction posture,
  covered Opkrævningsloven payment-deadline/remittance posture and § 7
  rate-derivation fixture plus 2026 late-payment supplement source-chain
  amendment and date-exact daily interest context, covered B-skat installment
  calendar/rate-window projection, covered § 7 cross-calendar-year daily
  interest split, covered § 62 A interest fixtures, exposed and scheduled
  restskat remaining B-skat-rate minimum tension and system-start residual
  restskat collection,
  § 13 foreign/pension/business amount limitations are executable audit signals,
  including 2025 PBL § 16 behavior, 2026 repeal behavior, LL § 33 A relief,
  seamen-relief exceptions, and calculator-level § 13 integration signals over
  the domain-object calculator input.

M6 - Website integration

- Status: first slice implemented.
- Output: research page under the website showing the Personskatteloven corpus,
  source status, milestones, and selected audits.
- Done when: the website renders the checked corpus and clearly marks whether it
  is a calculation-ready slice or a research/audit slice.
- Current slice: `/research/personskatteloven` links the valid and historic
  sources, renders the milestone log, embeds the checked §§ 1-28 `.runa`
  corpus plus `.scenario.runa` executable scenarios and the `.audit.runa`
  audit suite, marks the shared Pengebeløb rounding posture, the limited
  wage-earner fixture slice, a source-backed external Skat.dk 2026 ordinary
  wage-earner fixture, the § 14/§ 19 external differential scenario, plus ordinary and
  special-case AM-law coverage, ordinary Ligningsloven deductions, Kildeskatteloven
  A-income/withholding/e-skattekort/slutopgørelse/restskat timing and system-start rateplan posture,
  BEK 839 generated-card path, BEK 1094 2026 indeholdelsesprocent derivation,
  first § 1/§ 2 taxable-income composition from the separate income categories,
  first § 4 a pension/share-income audit,
  first § 8 a/§ 67 share-income annual-settlement scenario including negative
  share-income carry-forward,
  first § 8 c limited-taxpayer municipal-equivalent tax calculation and
  published-rate source/method provenance,
  first § 11 negative net-capital reduction order and spouse-transfer audit,
  first § 9/§ 12 split state personfradrag tax-value reduction-order audit and
  wage-earner component projection,
  § 26 transition-compensation audit including stk. 7 spouse top-tax allocation,
  first § 6/§ 7/§ 8 a/§ 8 b/§ 13/§ 14/§ 19 bomb-audit probes, the Børne- og ungeydelse
  household benefit-cliff/source-tension probe plus a boligsikring § 22
  threshold-cliff probe,
  Opkrævningsloven payment-deadline, § 7 rate-derivation, date-exact daily
  interest-context, and cross-calendar-year interest-split slices, the B-skat
  calendar projection, system-date-driven § 61 stk. 4/stk. 6 rateplans,
  minimum-rate completion plan, § 62 A interest
  fixtures, § 14 partial-year wage-earner
  scenario, § 14 official guidance example, and personal plus positive-capital § 19 skatteloft inside the
  wage-earner breakdown, including the 2026 Langeland high-rate municipality
  fixture and published SKM `Nedslag pct.` differential, as calculation-ready, and marks the full statute model as
  research/audit-only.

M7 - Personfradrag and deficit layer

- Status: first slice implemented.
- Output: `.runa` file for §§ 10-13 plus calculator/audit integration for
  ordinary positive-income wage-earner cases.
- Done when: personfradrag amount selection, § 12 tax-value calculation,
  after-personfradrag fixture totals, and § 13 deficit boundary signals are
  executable.
- Current slice: adult 2025 personfradrag is pulled from the official
  tax-year parameter pack, state/municipal/church tax values are calculated,
  Copenhagen and Gentofte fixtures settle after personfradrag, § 13 deficit tax
  value and offset order are executable, § 10 stk. 5-6 eligibility for
  Kildeskatteloven § 2 taxpayers is modeled with choice/reversal deadlines and
  explicit sailor/residence-permit/researcher exclusions, § 10 stk. 3
  spouse transfer of unused state-personfradrag tax value is amount-modeled for
  the § 9 state-tax basket and year-end cohabitation condition, § 11 negative
  net-capital reduction covers source-backed stk. 2 rate provenance, spouse
  threshold pooling, spouse positive-capital offset, statutory tax-order
  reduction, and unused spouse transfer, § 9/§ 12
  split state personfradrag tax-value reduction across the state-tax basket and
  § 9 non-state personfradrag reduction for § 8 c/kommunal/kirkelig tax are
  now wired into the wage-earner calculator, spouse
  deficit transfer, § 13 stk. 4 negative personal income offsets through spouse
  personal income and both spouses' positive capital income, and carried-forward
  negative personal income ordering are fixture-tested,
  foreign/pension spouse transfer limitations are executable, and same-business
  loss carry-forward amounts are fixture-tested. § 13's first dependent-source validation now
  covers PBL § 16 through 2025, the 2026 repeal, LL § 33 A relief, and seamen
  relief. Complex § 13 calculator breakdown fixtures now cover spouse transfer,
  LL § 33 A relief, 2026 PBL repeal, and same-business carry-forward. The
  ordinary wage-earner calculator now also exposes a `LønmodtagerPar13UnderskudResult`
  for negative taxable-income cases, keeps municipal/church tax from becoming
  negative, applies the § 13 tax-value offset to state tax before § 9/§ 12
  personfradrag, and carries the unused tax value in the breakdown. Remaining
  work is broader calculator integration with complete 2026 parameters and
  external differential fixtures rather than the first § 13 amount formulas
  themselves.

M8 - Omregning, skatteloft, and regulation

- Status: first slice implemented.
- Output: `.runa` file for §§ 14-20 plus audit coverage for annualization,
  tax-ceiling relief, and statutory regulation rounding.
- Done when: partial-year annualization, repeal markers, personal/capital
  tax-ceiling rates, calculated ceiling relief, and § 20 rounded regulated
  amounts are executable.
- Current slice: § 14 converts partial-year income to whole-year equivalents
  rounded to whole kroner and reduces whole-year tax proportionally. A first
  ordinary wage-earner § 14 integration now annualizes a delårs wage-earner
  input and uses the state income-tax component after §§ 6-9, rather than the
  AM-inclusive total. The official guidance example now proves recurring-vs.
  one-off amount handling, and § 14 stk. 2 now has an executable election result
  for oplysningsskema election, timely reversal by 30 June in the second
  calendar year after the income year, late reversal, and the § 10 stk. 6
  limited-taxability path where the stk. 2 election is not available.
  §§ 15-18 are explicit repealed markers, § 19 computes
  personal and positive-capital tax ceiling excess and relief, both personal and
  positive-capital § 19 relief now flow into the ordinary wage-earner breakdown
  for supported tax years and municipalities, the 2026 Langeland fixture proves
  a source-backed high municipal-tax ceiling case and matches the published
  1,24 pct. `Nedslag pct.`, and § 20 now returns source-backed regulation-number
  results for statutory 2009-2013 values, SKM historical 2014-2024 values, and
  SKM 2025-2026 published values. The § 20 table now uses SKM's 2020-2024
  figures of 114,3, 116,9, 118,3, 121,8 and 126,1 before computing 2010-level
  amount regulation with round-up to the nearest 100 kroner.

M9 - Final provisions and transition compensation

- Status: first slice implemented.
- Output: `.runa` file for §§ 21-28 plus audit coverage for effective date,
  transition compensation, ministerial delegation, and territorial exclusion.
- Done when: repealed tail provisions, effect from income year 1987, § 26
  compensation and offset order, § 27 delegation, and § 28 exclusion of the
  Faroe Islands and Greenland are executable.
- Current slice: §§ 21-24 a, 25 a, 25 b, and 27 a are explicit repeal markers,
  § 25 applies from 1987, § 26 computes negative transition difference as
  compensation and applies it in statutory order, § 26 stk. 9 now regulates the
  2010-level thresholds through the § 20 rounding rule before deriving stk. 2
  line items from statutory bases, § 26 stk. 4-6 and stk. 8 now compute
  samlevende-ægtefælle offsets for positive/negative transition differences
  including pair-level post-stk. 4 compensation amounts, pair-level post-stk. 5
  and post-stk. 8 negative/positive net-capital interaction, and pair-level
  nr. 2 bundfradrag transfer with the § 48 F exception. An annual pair-level
  `Par26ForskelsbeløbParÅrsSag` now feeds the stk. 6 effective thresholds into
  the actual nr. 2 line items before the stk. 4 spouse difference offset.
  `Par26ForskelsbeløbParÅrsKomponentSag` now accepts raw annual personal-income,
  net-capital-income, and ligningsmæssige-fradrag components for both spouses,
  applies stk. 5 before the nr. 2 base, stk. 8 before the nr. 8 base using the
  § 11 threshold, and then delegates to the annual pair calculation.
  § 26 stk. 9 can now derive
  source-backed 2012-2019
  threshold packs from the official § 20 `reguleringstal`, § 26 nr. 5 can derive
  2012, 2017 and 2019 Ligningsloven §§ 9 J/9 K/9 L fradrag and the 4,25 pct.
  baseline from source-backed inputs, § 26 stk. 7 now applies § 7 stk. 5 and
  stk. 10-11 spouse capital rules when deriving nr. 3 for transition
  compensation, and `Par26KompensationAfregningResult` now composes an annual
  compensation calculation with the statutory tax-offset order, § 27 is encoded as delegated
  implementation/administration authority, and § 28 excludes the Faroe Islands
  and Greenland. Remaining § 26 depth is mostly integration work: broader
  historic compensation fixtures, dependent-year settlement parameter wiring,
  and eventual wiring into a full historic tax-settlement calculator.
