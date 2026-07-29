# Personskatteloven as Futuruna

Status: foundation  
Last updated: 2026-07-18
TD epic: `td-56cf8d`

This folder is the working home for encoding Danish personal income tax law in
Futuruna. The aim is not only to display the law as source code, but to make the
rules executable enough to calculate ordinary tax cases and strict enough to
audit tensions, cliffs, missing definitions, and delegated dependencies.

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
- XML end date observed on 2026-07-18: `2026-06-23`
- Change references in XML: 7

Current source-refresh finding:

- The tracked Retsinformation XML sources were re-fetched on 2026-07-18.
- The official XML `Status` fields remained unchanged: the working/dependency
  sources still report `Valid`, while `2019/799` reports `Historic`.
- Every tracked `Valid` source now has an XML `EndDate` horizon before
  2026-06-26, so `source-status.runa` distinguishes formal legal validity from
  current-day automation freshness.
- `AktuelSkatteberegning` still accepts formally valid sources; the new
  `DagsaktuelAutomatiskBeregning` purpose rejects sources whose metadata horizon
  does not cover `20260626`.
- `scripts/refresh-danish-tax-source-status.py --today 20260626 --fail-on-drift`
  fetches official XML for every `Retskilde(...)` record and reports semantic
  drift between Retsinformation and the encoded source model. On 2026-07-18 it
  checked 19 records with 0 drift and 0 fetch/parse errors.

Current § 13 amendment/dependency sources:

- Person-tax reform amendment:
  `https://www.retsinformation.dk/eli/lta/2024/482`
  - XML status on 2026-07-18: `Valid`
  - § 1, nr. 14 repeals Personskatteloven § 13, stk. 5, 4. pkt.
  - § 8, stk. 4 gives § 1 effect from income year 2026.
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
    wage-earner calculator.
  - SKM rates page used for current basis points and caps:
    `https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/ligningsloven`
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
- `skatdk-2026-ekstern.scenario.runa` exists and checks/runs with `runa run`.
- `delaar-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `omregning-skatteloft-ekstern.scenario.runa` exists and checks/runs with
  `runa run`.
- `husholdning-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `husholdning-benefit-cliffs.audit.runa` exists and checks/runs with
  `runa run`.
- `aktieindkomst-pension.audit.runa` exists and checks/runs with `runa run`.
- `aktieindkomst-slutopgoerelse.scenario.runa` exists and checks/runs with
  `runa run`.
- `slutopgoerelse.scenario.runa` exists and checks/runs with `runa run`.
- `indeholdelse-afregning.scenario.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven-bomber.audit.runa` exists and checks/runs with `runa run`.
- `personskatteloven.audit.runa` exists and checks/runs with `runa run`.
- `pengebeloeb.runa` exists and checks/runs with `runa run`.
- Website research page exists at `/research/personskatteloven` and renders
  source status, milestone status, selected audit signals, and the checked
  `.runa` corpus.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy including amount-level § 4 a pension deduction from
  positive share income, the §§ 5-9 state-tax skeleton including amount-level
  § 6 spouse negative net-capital offset and § 7 stk. 5 spouse
  positive-capital threshold/negative-capital offset, the §§ 10-13
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
  Ligningsloven ordinary wage-earner deduction
  dependency slices, 2024/2025/2026 tax-year parameter packs, grouped
  wage-earner calculation-domain records, first wage-earner scenarios, a first
  § 14 partial-year wage-earner scenario, a first fictional household scenario,
  a first § 8 a share-income final-settlement scenario with § 67 dividend-tax
  credit splitting,
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

- Futuruna already supports named constructor arguments (`Field = value`) for
  named-field records and scoped-rule constructors. Wide legal/domain records
  should use named construction at fixture and boundary-assembly points when
  positional arguments would hide legal meaning, especially for boolean flags,
  statutory rate rows, tax-credit baskets, and settlement/date cases.
- A readability sweep now uses named construction for the broad executable
  Danish-income-tax records found by scan, including boolean-heavy fixtures,
  statutory rate rows, remittance calendar/history facts, and audit inputs;
  short date-like triples and compact arithmetic helpers can remain positional
  where that is still idiomatic.
- `opkraevningsloven.runa` now splits the former 11-field remittance input into
  `OpkrævningAfregningsperiode`, `OpkrævningTilsvarHistorik`,
  `OpkrævningBankkalender`, `OpkrævningBetaling`, and a small composed
  `OpkrævningASkatAmAfregningInput`.
- `indeholdelse-afregning.scenario.runa` owns the executable remittance facts
  and assertions. The source-law module keeps the original legal text and the
  corresponding rules.
- `Par13KompleksBeregningInput` now composes named subdomains instead of a
  25-field positional record: income basis, tax-value rates, offset-tax pools,
  spouse-transfer facts, stk. 5 limitation facts, and same-business loss facts.
  The calculator rules project from those domain objects, and the scenario/audit
  fixtures name those facts before composing the calculator input.
- `Par13ModregningSag` uses product-scoped `|` rules for the § 13 ordered
  tax-value offset chain, keeping the carried remainder after § 6, § 7, § 7 a,
  and § 8 a, stk. 2 inside the same legal case while preserving public wrapper
  rule names for downstream calculator/audit files.
- `Par4aPensionsfradragSag` uses product-scoped `|` rules for the § 4 a,
  stk. 4 amount layer. The scope keeps positive share income, the requested
  pension deduction, notice to the tax administration, no-double-deduction
  status, capped deduction, remaining share income, and disallowed amount
  together while stable wrapper rules expose the calculation to audits.
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
- `AktieindkomstSlutopgørelseCase` uses product-scoped `|` rules for the
  § 8 a/§ 67 annual-settlement case. The scope keeps the wage-earner breakdown,
  monthly A-skat, share income, spouse share-income threshold facts, and
  withheld dividend tax together while methods derive the effective
  progression threshold, final low-layer share tax, high-layer tax entering
  final tax, § 60 credit basket, and final annual-settlement result.
- `AktieindkomstUdbytteskatStk3Sag` uses product-scoped `|` rules for
  Personskatteloven § 8 a, stk. 3. It keeps tax year, total share income, and
  dividend tax withheld under Kildeskatteloven § 65 together while defaults and
  exceptions derive the low-rate comparison amount, any over-withheld amount
  credited in slutskatten, the negative-share-income full credit, and the
  remaining final dividend-tax payment.
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
  calculator surface.
- `LønmodtagerBeregning` now composes the ordinary wage-earner calculation from
  named domain records: income basis, Ligningsloven deductions, tax before
  person allowance, person allowance tax value, tax after person allowance before
  skatteloft, and final tax after § 19 relief. The existing flat
  `LønmodtagerBreakdown` remains as the reporting/API projection so website and
  scenario consumers do not have to learn every internal calculation layer.
- `loenmodtager_beregning.runa` now separates state income tax, municipal/church
  income tax, total income tax, and the final total including AM contribution.
  This keeps Personskatteloven § 14's "helårsskat efter §§ 6-9" from
  accidentally consuming an AM-inclusive cash-flow total.
- `LønmodtagerPar14Input` is the current right-sized § 14 boundary for ordinary
  wage-earner partial-year cases. It carries tax-liability change status, the
  delårs wage-earner input, and tax-liability days together, instead of passing
  those scalars through every helper rule.
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

- Deepen the executable Kildeskatteloven settlement path around ordinary
  taxpayers: restskat supplement/timing, overskydende skat compensation,
  amended annual statement posture, and § 62 C minimum thresholds.
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
  Sømandsbeskatningsloven §§ 5-8, and the 2026 repeal in LOV nr. 482/2024.
  § 4 a's first amount-level audit now covers pensionsfradrag in positive
  share income: the deduction is capped at positive share income, requires
  notice to the tax administration, is blocked if already deducted in personal
  income, and is unavailable without positive share income.
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
  itemized deductions beyond the ordinary §§ 9 J/9 K wage-earner deductions,
  and annual rate/threshold adjustments.
- Expand audit coverage for source drift, missing dependencies, tax cliffs,
  delegated powers, category boundary problems, and multi-step daisy-chain
  effects. The first dedicated bomb-audit file now covers § 6 spouse negative
  net-capital offset, § 7 stk. 5 spouse capital-threshold/negative-capital
  offset, § 8 a share-income
  spouse-threshold/negative-offset interactions, § 8 b CFC tax outside
  personfradrag/skatteloft, § 13 passive business-loss lock-in, § 14
  annualisation, and § 19 skatteloft interactions. A separate household benefit
  audit now covers Børne- og ungeydelse aftrapning plus an official-source
  mellemskat/topskat wording tension, and a first boligsikring § 22 threshold
  cliff; remaining probes should target broader housing-support calculators and
  cross-law allowance or collection timing chains.
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
  CFC income are represented as typed legal categories with original text
  preserved.

M2 - State tax computation skeleton

- Status: first slice implemented.
- Output: `.runa` files for §§ 5-9.
- Done when: the legal structure of bundskat, mellemskat, topskat, toptopskat,
  abolished/zeroed taxes, aktieindkomstskat, CFC tax, and
  municipal-equivalent state tax is encoded.
- Current slice: 2026 LOV nr. 482/2024 state-tax structure is represented:
  mellemskat under § 7, topskat under § 7 a, toptopskat under § 8, and
  udligningsskat/sundhedsbidrag are no longer active components from 2026.
  The § 6 slice now computes the amount-level spouse negative net-capital
  offset before bundskat basis calculation.
  The § 7 mellemskat slice now covers positive net capital income over the
  regulated 2026 threshold, including an executable spouse doubled-threshold
  case and the § 7 stk. 5 rule that negative net capital is offset against the
  spouse's positive net-capital income before the spouse's effective
  grundbeløb is increased.

M3 - Tax-year parameter packs

- Status: first slice implemented.
- Output: sourceable annual tables for rates, thresholds, allowances, and
  municipality-specific inputs.
- Done when: the same legal rules can run for at least two tax years by swapping
  parameter packs.
- Current slice: 2024, 2025, and 2026 national parameters from
  Skattestyrelsen/SKM plus Copenhagen and Gentofte municipal/church-tax inputs
  from Skatteministeriet, and a 2026-only Langeland municipal row from
  Skatteministeriet's `kommuneskattesatser_2026.xlsx`. The 2026 pack covers
  mellemskat, topskat, toptopskat, personfradrag, aktieindkomst, skatteloft,
  municipal rates, church-tax rates, published skatteloftsnedslag, and
  grundskyldspromille. Parameter completeness is year+municipality specific, so
  Langeland is not treated as supported for 2024/2025 until those rows are
  source-backed.

M4 - Ordinary taxpayer calculator

- Status: first slice implemented.
- Output: fixtures and executable examples for wage income, capital income,
  deductions, municipal tax, church tax, and AM contribution.
- Done when: normal cases produce reproducible tax breakdowns with source-backed
  assumptions.
- Current slice: 2025 Copenhagen and Gentofte wage-earner fixtures produce
  deterministic AM contribution, personal income after AM, ordinary taxable
  income after derived Ligningsloven §§ 9 J/9 K wage-earner deductions,
  bundskat, topskat, municipal tax, church tax, § 10 personfradrag, § 12
  personfradrag tax values, after-personfradrag totals, and a § 13
  ordinary-positive-income boundary. Separate § 13 calculator breakdown fixtures
  now cover spouse-transfer deficit, LL § 33 A relief, 2026 post-PBL-repeal
  transfer, and same-business loss carry-forward cases. 2026 Copenhagen
  wage-earner fixtures now exercise mellemskat, topskat, and toptopskat under
  the LOV nr. 482/2024 reform thresholds, and a 2026 Copenhagen positive
  net-capital fixture exercises the mellemskat capital addition. The wage-earner
  breakdown now includes `LønmodtagerSkatteloftResult`, so § 19 personal and
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
  now import the Ligningsloven dependency slice instead of being manual zeroes.
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
  `aktieindkomst-slutopgoerelse.scenario.runa` now composes Personskatteloven
  § 8 a with Kildeskatteloven § 67 for the fictional primary wage-earner:
  150.000 kr. share income with the spouse's unused share-income threshold stays
  in the 27 pct. final-tax layer, while the high-tax variant splits 21.438 kr.
  final low-layer tax from 29.652 kr. high-layer tax entering slutskat and
  leaves 7.900 kr. restskat after 19.062 kr. dividend-tax credit. The source-law
  module now also covers § 8 a, stk. 3 as a separate scoped rule case for
  over-withheld dividend tax and negative-share-income full credit.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, covered normal-fixture
  personfradrag, covered 2026 state-tax reform layers, covered § 13 deficit
  mechanics, mellemskat positive-net-capital and spouse-threshold activation,
  § 7 stk. 5 spouse negative-capital offset/effective-grundbeløb activation,
  ordinary wage and special-case AM-law coverage, ordinary Ligningsloven §§ 9 J/9 K
  wage-earner-deduction coverage, ordinary municipal/church-tax legal coverage,
  covered Kildeskatteloven ordinary A-income/withholding/e-skattekort posture,
  covered BEK 839 forskudskort generation, covered BEK 1094 2026
  indeholdelsesprocent derivation, covered Kildeskatteloven slutopgørelse
  balance/restskat timing/system-date-driven § 61 stk. 4/stk. 6 rateplans/
  overskydende skat compensation/dividend-tax credit posture, covered § 8 a
  share-income final-settlement scenarios with § 67 dividend-tax credit
  splitting plus § 8 a, stk. 3 over-withheld/negative-share-income dividend-tax
  credits, covered fictional
  household scenario, covered Børne- og ungeydelse
  benefit-cliff/source-tension audit plus a first boligsikring § 22 threshold
  cliff, covered external Skat.dk 2026
  ordinary wage-earner fixture, topskat threshold activation,
  covered § 4 a pension deduction from positive share income,
  covered § 14 annualization and first wage-earner calculator integration plus
  the Den juridiske vejledning external annualisation example,
  covered first bomb-audit probes for § 6/§ 7/§ 8 a/§ 8 b/§ 13/§ 14/§ 19 daisy-chain tensions,
  covered § 19 skatteloft including the 2026
  44,57 pct. personal ceiling, 42 pct. positive-capital ceiling, and
  calculator-level wage-earner integration for both paths, including
  source-backed Langeland 2026 high-municipal-rate personal and positive-capital
  relief fixtures and the published 1,24 pct. SKM `Nedslag pct.` differential,
  covered § 20 regulation/rounding, covered § 26 transition
  compensation, covered § 28 territorial exclusion, covered AM-law special cases,
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
  first § 4 a pension/share-income audit,
  first § 8 a/§ 67 share-income annual-settlement scenario,
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
  value and offset order are executable, spouse deficit transfer and negative
  personal income carry-forward are fixture-tested, foreign/pension spouse
  transfer limitations are executable, and same-business loss carry-forward
  amounts are fixture-tested. § 13's first dependent-source validation now
  covers PBL § 16 through 2025, the 2026 repeal, LL § 33 A relief, and seamen
  relief. Complex § 13 calculator breakdown fixtures now cover spouse transfer,
  LL § 33 A relief, 2026 PBL repeal, and same-business carry-forward. Remaining
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
  one-off amount handling. §§ 15-18 are explicit repealed markers, § 19 computes
  personal and positive-capital tax ceiling excess and relief, both personal and
  positive-capital § 19 relief now flow into the ordinary wage-earner breakdown
  for supported tax years and municipalities, the 2026 Langeland fixture proves
  a source-backed high municipal-tax ceiling case and matches the published
  1,24 pct. `Nedslag pct.`, and § 20 computes 2010-level amount regulation with
  round-up to the nearest 100 kroner.

M9 - Final provisions and transition compensation

- Status: first slice implemented.
- Output: `.runa` file for §§ 21-28 plus audit coverage for effective date,
  transition compensation, ministerial delegation, and territorial exclusion.
- Done when: repealed tail provisions, effect from income year 1987, § 26
  compensation and offset order, § 27 delegation, and § 28 exclusion of the
  Faroe Islands and Greenland are executable.
- Current slice: §§ 21-24 a, 25 a, 25 b, and 27 a are explicit repeal markers,
  § 25 applies from 1987, § 26 computes negative transition difference as
  compensation and applies it in statutory order, § 27 is encoded as delegated
  implementation/administration authority, and § 28 excludes the Faroe Islands
  and Greenland.
