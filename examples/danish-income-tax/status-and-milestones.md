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
- Historic mark in XML: `2021-06-16`

Current working source:

- Retsinformation: `https://www.retsinformation.dk/eli/lta/2021/1284`
- XML endpoint checked: `https://www.retsinformation.dk/eli/lta/2021/1284/dan/xml`
- Title: `Bekendtgørelse af lov om indkomstskat for personer m.v. (personskatteloven)`
- XML status on 2026-07-18: `Valid`
- Signed: `2021-06-14`
- In force from: `2021-06-16`
- XML end date observed on 2026-07-18: `2026-06-19`
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
- Sømandsbeskatningsloven:
  `https://www.retsinformation.dk/eli/lta/2023/1181`
  - XML status on 2026-07-18: `Valid`
  - §§ 5-8 are the seamen relief exception in § 13, stk. 5.

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
- `skatteaar-parametre.runa` exists and checks with `runa check`.
- `loenmodtager_beregning.runa` exists and checks with `runa check`.
- `loenmodtager-fixtures.runa` exists and checks/runs with `runa run`.
- `personskatteloven-audit.runa` exists and checks/runs with `runa run`.
- Website research page exists at `/research/personskatteloven` and renders
  source status, milestone status, selected audit signals, and the checked
  `.runa` corpus.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy, the §§ 5-9 state-tax skeleton, the §§ 10-13
  personfradrag/underskud slice, the §§ 14-20 omregning/skatteloft/regulering
  slice, the §§ 21-28 concluding provisions slice, 2024/2025 tax-year parameter
  packs, first wage-earner fixtures, and first audit signals.
- The chapter files follow the repeating structure: official legal text in a
  multiline block, then the corresponding Futuruna rules.
- Existing Danish Constitution examples show the intended style: original legal
  text in multiline source blocks, followed by Futuruna types, constants, and
  typed `|` legal rules.
- Typed `|` rule heads, `under` conditions, and `exception` rules are already
  present in the language test corpus and should be used for legal formulations.
- Website integration is active and should be updated whenever a checked
  Personskatteloven `.runa` slice becomes part of the displayed corpus.

## Now

- Create the source foundation for Personskatteloven using official
  Retsinformation references.
- Encode source suitability and source lineage explicitly, so historic sources
  cannot silently drive current tax calculation.
- Start with chapter structure and the core income categories:
  skattepligtig almindelig indkomst, personlig indkomst, kapitalindkomst,
  aktieindkomst, and CFC-indkomst.
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
  statutes and trusted calculation examples, especially for AM contribution,
  municipal/church taxation, and Kildeskatteloven. § 13's first dependent-source
  slice now covers Pensionsbeskatningsloven § 16, Ligningsloven § 33 A,
  Sømandsbeskatningsloven §§ 5-8, and the 2026 repeal in LOV nr. 482/2024.
- Decide how § 14 partial-year annualization and § 19 skatteloftsnedslag should
  flow into the ordinary wage-earner calculator once there are trusted
  partial-year and high-rate fixtures.
- Separate legal structure from annual parameter packs: rates, thresholds,
  personal allowances, municipal tax, church tax, and other tax-year data.
- Build calculation fixtures for ordinary wage-earner cases before handling
  complex cases.
- Gather complementary official sources for:
  arbejdsmarkedsbidrag, municipal income tax, church tax, personal allowance,
  ligningsmæssige fradrag, kildeskat, and annual rate/threshold adjustments.
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
- Done when: the legal structure of bundskat, topskat, abolished/zeroed taxes,
  aktieindkomstskat, CFC tax, and municipal-equivalent state tax is encoded.

M3 - Tax-year parameter packs

- Status: first slice implemented.
- Output: sourceable annual tables for rates, thresholds, allowances, and
  municipality-specific inputs.
- Done when: the same legal rules can run for at least two tax years by swapping
  parameter packs.
- Current slice: 2024 and 2025 national parameters from Skattestyrelsen plus
  Copenhagen and Gentofte municipal/church-tax inputs from Skatteministeriet.

M4 - Ordinary taxpayer calculator

- Status: first slice implemented.
- Output: fixtures and executable examples for wage income, capital income,
  deductions, municipal tax, church tax, and AM contribution.
- Done when: normal cases produce reproducible tax breakdowns with source-backed
  assumptions.
- Current slice: 2025 Copenhagen and Gentofte wage-earner fixtures produce
  deterministic AM contribution, personal income after AM, ordinary taxable
  income, bundskat, topskat, municipal tax, church tax, § 10 personfradrag,
  § 12 personfradrag tax values, after-personfradrag totals, and a § 13
  ordinary-positive-income boundary.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, covered normal-fixture
  personfradrag, covered § 13 deficit mechanics, 2026 reform parameter gap,
  topskat threshold activation, covered § 14 annualization, covered § 19
  skatteloft, covered § 20 regulation/rounding, covered § 26 transition
  compensation, covered § 28 territorial exclusion, AM-law dependency,
  municipal-tax-law dependency, and § 13 foreign/pension/business amount
  limitations are executable audit signals, including 2025 PBL § 16 behavior,
  2026 repeal behavior, LL § 33 A relief, and seamen-relief exceptions.

M6 - Website integration

- Status: first slice implemented.
- Output: research page under the website showing the Personskatteloven corpus,
  source status, milestones, and selected audits.
- Done when: the website renders the checked corpus and clearly marks whether it
  is a calculation-ready slice or a research/audit slice.
- Current slice: `/research/personskatteloven` links the valid and historic
  sources, renders the milestone log, embeds the checked §§ 1-28 `.runa`
  corpus, marks the limited wage-earner fixture slice as calculation-ready, and
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
  relief. Remaining work is calculator integration for complex § 13 cases rather
  than the first amount formulas themselves.

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
