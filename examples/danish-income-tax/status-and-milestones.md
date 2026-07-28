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
- XML end date observed on 2026-07-18: `2026-06-20`
- Historic mark in XML: `2021-06-16`

Current working source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2021/1284`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2021/1284/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-18: `Valid`
- Signed: `2021-06-14`
- In force from: `2021-06-16`
- XML end date observed on 2026-07-18: `2026-06-20`
- Change references in XML: 7

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
    posture, overskydende skat compensation and refund posture, amended annual
    statement interest posture, minimum-rate thresholds, and dividend-tax credit
    posture.
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

Working decision: use `2021/1284` as the current consolidated source for live
encoding, while preserving `2019/799` as source lineage because the valid
consolidation explicitly builds on it. The 2019 source remains useful for
historical audit and diffing, but it should not be treated as the live basis for
calculating a current taxpayer's tax. For provisions modified by later valid
amendment acts, such as § 13's 2026 PBL § 16 repeal, the amendment act must be
encoded as a temporal rule on top of the consolidation.

## Current Implementation Status

- Folder created at `examples/danish-income-tax/`.
- `source-status.runa` exists and checks with `runa check`.
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
- `husholdning-scenarier.scenario.runa` exists and checks/runs with `runa run`.
- `slutopgoerelse.scenario.runa` exists and checks/runs with `runa run`.
- `indeholdelse-afregning.scenario.runa` exists and checks/runs with
  `runa run`.
- `personskatteloven.audit.runa` exists and checks/runs with `runa run`.
- Website research page exists at `/research/personskatteloven` and renders
  source status, milestone status, selected audit signals, and the checked
  `.runa` corpus.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy, the §§ 5-9 state-tax skeleton, the §§ 10-13
  personfradrag/underskud slice, the §§ 14-20 omregning/skatteloft/regulering
  slice, the §§ 21-28 concluding provisions slice, ordinary and special-case
  AM-law,
  municipal-income-tax, church-tax, Kildeskatteloven A-income/withholding,
  BEK 839 forskudskort generation, BEK 1094 2026 indeholdelsesprocent,
  Kildeskatteloven §§ 60-62/62 A/62 C/67 slutopgørelse balance,
  restskat timing and overskydende-skat compensation posture,
  Opkrævningsloven payment deadlines,
  Ligningsloven ordinary wage-earner deduction
  dependency slices, 2024/2025/2026 tax-year parameter packs, first wage-earner
  scenarios, a first fictional household scenario, complex § 13 calculator
  fixtures, and first audit signals.
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
- `slutopgoerelse.runa` keeps the year-end balance as `KildeskatSlutopgørelseInput`
  plus a statutory `KildeskatPar60Kreditter` credit basket. This is a better
  domain boundary than passing A-skat, AM-bidrag, B-skat, dividend-tax credits,
  voluntary payments, and special credit categories through every subrule.
- `KildeskatRestskatOpkrævningInput` now uses
  `KildeskatRestskatUdskrivningspostur` instead of a cluster of timing booleans.
  The enum models the statutory issuance posture directly and rules derive the
  § 61 branches from it, so impossible flag combinations cannot become fixtures.
- `KildeskatBSkatRateVindue` now groups the B-skat installment calendar
  projection for restskat collection. The restskat input still carries the
  system/scenario fact "first remaining B-skat rate", while downstream checks
  read a single rate-window object instead of threading first-rate, last-rate,
  month, and deadline fields separately.

Review candidates to revisit deliberately, not as broad churn:

- `KildeskatESkattekortInput` and
  `KildeskattebekendtgørelseForskudskortInput` still may eventually share
  smaller card-period and withholding-percentage objects, but the current BEK
  1094 slice keeps the annual 2026 percentage derivation as its own domain
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
  settlement edge cases, automated Nationalbank input sourcing for
  Opkrævningsloven § 7 annual rates, date-exact B-tax first-remaining-rate
  selection, date-exact § 62 A issue/payout scheduling, and remaining AM
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
  kassekreditrente inputs.
  The first Kildeskatteloven slutopgørelse slice now covers § 60 crediting,
  § 61 restskat plus percentage supplement and timing posture, § 62
  overskydende skat plus compensation/refund posture, § 60 spouse offsetting,
  § 58 B-skat calendar projection, § 62 A amended annual statement interest
  posture, § 62 C minimum thresholds, and § 67 dividend-tax credit posture; the
  fictional household's generated-card annual settlement currently yields
  3.541 kr. overskydende skat and 3.541 kr. payout under the source-derived
  § 7 rate fixture.
  § 13's first dependent-source slice now covers
  Pensionsbeskatningsloven § 16, Ligningsloven § 33 A,
  Sømandsbeskatningsloven §§ 5-8, and the 2026 repeal in LOV nr. 482/2024.
- Decide how § 14 partial-year annualization and § 19 skatteloftsnedslag should
  flow into the ordinary wage-earner calculator once there are trusted
  partial-year and high-rate fixtures.
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
  delegated powers, and category boundary problems.
- Extend the website page as more of the corpus becomes calculation-ready.

## Later

- Encode spouse rules, partial-year taxation, pension interaction, share income,
  CFC income, business income, property-related income, and special regimes.
- Build a normal-person income tax calculator backed by the Futuruna rules and
  tax-year parameter packs.
- Add differential checks against official examples or trusted calculators where
  legally safe and sourceable.
- Add Retsinformation update automation: fetch current XML, detect source
  status changes, and produce semantic diffs against the encoded model.
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
  The § 7 mellemskat slice now covers positive net capital income over the
  regulated 2026 threshold, including an executable spouse doubled-threshold
  case.

M3 - Tax-year parameter packs

- Status: first slice implemented.
- Output: sourceable annual tables for rates, thresholds, allowances, and
  municipality-specific inputs.
- Done when: the same legal rules can run for at least two tax years by swapping
  parameter packs.
- Current slice: 2024, 2025, and 2026 national parameters from
  Skattestyrelsen/SKM plus Copenhagen and Gentofte municipal/church-tax inputs
  from Skatteministeriet. The 2026 pack covers mellemskat, topskat,
  toptopskat, personfradrag, aktieindkomst, skatteloft, and municipal rates.

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
  net-capital fixture exercises the mellemskat capital addition. The ordinary
  wage-earner AM contribution now imports Arbejdsmarkedsbidragsloven instead of
  using a local arithmetic shortcut, and the AM-law module now has
  source-backed special-case fixtures for § 3 exclusions, self-employed bases,
  library-fee compensation, and the 2026 youth exemption. Ordinary municipal
  income tax and church tax now import Kommuneskatteloven and Folkekirkens
  økonomi instead of using
  local arithmetic shortcuts. Ordinary Ligningsloven employment/job deductions
  now import the Ligningsloven dependency slice instead of being manual zeroes.
  A first fictional household scenario now computes a 2026 Copenhagen married
  renter household with 50.000 kr./month primary wage income and 20.000
  kr./month spouse wage income, while explicitly marking child benefits,
  housing support and other deduction discovery as outside the current
  executable slice. Kildeskatteloven now marks the primary wage as A-income,
  proves that A-skat must be withheld, computes the statutory 55 pct.
  withholding if no e-skattekort, bikort or frikort has been received, and
  computes monthly A-skat/cash-flow payroll output when e-skattekort
  allowance/procent inputs are supplied. BEK 839 now generates the household's
  main-card monthly allowances from forskudsskat and BEK 1094-derived
  withholding percentage inputs, producing a separate generated-card payroll
  view.
  Opkrævningsloven now provides source-backed payment-deadline/remittance rules
  plus the § 7 annual-rate formula, with fixtures separated into
  `indeholdelse-afregning.scenario.runa` where they are scenario facts. The § 13
  complex calculator input now uses domain objects for income basis, tax-value
  rates, offset taxes, spouse transfer, stk. 5 limits, and same-business loss
  facts. `slutopgoerelse.scenario.runa` now also computes the fictional
  household's generated-card overskydende skat compensation and payout, plus a
  low-withholding restskat path with supplement and next-year transfer posture.
  Kildeskatteloven now also exposes the § 58 B-skat calendar as a rate-window
  domain object, § 62 A interest fixtures, and a restskat minimum-rate tension
  when remaining B-skat rates are too few.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, covered normal-fixture
  personfradrag, covered 2026 state-tax reform layers, covered § 13 deficit
  mechanics, mellemskat positive-net-capital and spouse-threshold activation,
  ordinary wage and special-case AM-law coverage, ordinary Ligningsloven §§ 9 J/9 K
  wage-earner-deduction coverage, ordinary municipal/church-tax legal coverage,
  covered Kildeskatteloven ordinary A-income/withholding/e-skattekort posture,
  covered BEK 839 forskudskort generation, covered BEK 1094 2026
  indeholdelsesprocent derivation, covered Kildeskatteloven slutopgørelse
  balance/restskat timing/overskydende skat compensation/dividend-tax credit
  posture, covered fictional household scenario, topskat threshold activation,
  covered § 14 annualization, covered § 19 skatteloft including the 2026
  44,57 pct. ceiling, covered § 20 regulation/rounding, covered § 26 transition
  compensation, covered § 28 territorial exclusion, covered AM-law special cases,
  covered Opkrævningsloven payment-deadline/remittance posture and § 7
  rate-derivation fixture, covered B-skat installment calendar/rate-window
  projection, covered § 62 A interest fixtures, exposed restskat remaining
  B-skat-rate minimum tension,
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
  audit suite, marks the limited wage-earner fixture slice plus ordinary and
  special-case AM-law coverage, ordinary Ligningsloven deductions, Kildeskatteloven
  A-income/withholding/e-skattekort/slutopgørelse/restskat timing posture,
  BEK 839 generated-card path, BEK 1094 2026 indeholdelsesprocent derivation,
  Opkrævningsloven payment-deadline and § 7 rate-derivation slices, the B-skat
  calendar projection, and § 62 A interest fixtures as calculation-ready, and
  marks the full statute model as research/audit-only.

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
  and reduces whole-year tax proportionally, §§ 15-18 are explicit repealed
  markers, § 19 computes personal and positive-capital tax ceiling excess and
  relief, and § 20 computes 2010-level amount regulation with round-up to the
  nearest 100 kroner.

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
