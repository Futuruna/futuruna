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
- Change references in XML: 7

Working decision: use `2021/1284` as the current consolidated source for live
encoding, while preserving `2019/799` as source lineage because the valid
consolidation explicitly builds on it. The 2019 source remains useful for
historical audit and diffing, but it should not be treated as the live basis for
calculating a current taxpayer's tax.

## Current Implementation Status

- Folder created at `examples/danish-income-tax/`.
- `source-status.runa` exists and checks with `runa check`.
- `kapitel-01-indkomst.runa` exists and checks with `runa check`.
- `kapitel-02-statsskat.runa` exists and checks with `runa check`.
- `skatteaar-parametre.runa` exists and checks with `runa check`.
- `loenmodtager_beregning.runa` exists and checks with `runa check`.
- `loenmodtager-fixtures.runa` exists and checks/runs with `runa run`.
- `personskatteloven-audit.runa` exists and checks/runs with `runa run`.
- The current `.runa` slices encode source validity, source lineage, the
  §§ 1-4 b income taxonomy, the §§ 5-9 state-tax skeleton, 2024/2025 tax-year
  parameter packs, first wage-earner fixtures, and first audit signals.
- The chapter files follow the repeating structure: official legal text in a
  multiline block, then the corresponding Futuruna rules.
- Existing Danish Constitution examples show the intended style: original legal
  text in multiline source blocks, followed by Futuruna types, constants, and
  typed `|` legal rules.
- Typed `|` rule heads, `under` conditions, and `exception` rules are already
  present in the language test corpus and should be used for legal formulations.
- Website integration is intentionally deferred until there is at least one
  checked Personskatteloven `.runa` file to display.

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

- Encode the state income tax composition in §§ 5-9 as executable rules.
- Separate legal structure from annual parameter packs: rates, thresholds,
  personal allowances, municipal tax, church tax, and other tax-year data.
- Build calculation fixtures for ordinary wage-earner cases before handling
  complex cases.
- Gather complementary official sources for:
  arbejdsmarkedsbidrag, municipal income tax, church tax, personal allowance,
  ligningsmæssige fradrag, kildeskat, and annual rate/threshold adjustments.
- Add the first audit file for source drift, missing dependencies, tax cliffs,
  delegated powers, and category boundary problems.
- Add a website research page once the foundation file checks and the project
  has a stable first slice to show.

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
  income, bundskat, topskat, municipal tax, church tax, and a marked § 9
  personfradrag gap.

M5 - Audit suite

- Status: first slice implemented.
- Output: audit files that intentionally search for tension, missing inputs,
  discontinuities, and source drift.
- Done when: audits can fail loudly without blocking legal reformulation work.
- Current slice: source-status rejection, § 9 personfradrag gap, 2026 reform
  parameter gap, topskat threshold activation, AM-law dependency,
  municipal-tax-law dependency, and spouse/negative-capital gaps are executable
  audit signals.

M6 - Website integration

- Status: not started.
- Output: research page under the website showing the Personskatteloven corpus,
  source status, milestones, and selected audits.
- Done when: the website renders the checked corpus and clearly marks whether it
  is a calculation-ready slice or a research/audit slice.
