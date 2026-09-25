# Danish tax-audit readiness

Evidence snapshot: **25 September 2026**; named commits below identify earlier
closeouts, with subsequent focused model corrections described separately.
This records the original launch findings and their disposition, not a claim
that arbitrary Danish tax returns are fully verified. Typed calculations remain
[Preview](feature-stages.md); the tax corpus remains research software.

## Useful entry points now

Use the checked-out [setup guide](../website/public/ai-setup.md#tax-audit-runtime-check)
and the exact compiler that passes its runtime check. Then choose the question:

| Question | Entry point | What it establishes |
| --- | --- | --- |
| Do my report's figures fit together without spouse documents? | [Conditional report review](../examples/danish-income-tax/aarsopgoerelse-afstemning.md) | Selected arithmetic, necessary transfers and checked limits; not independently verified spouse facts. |
| What changes if I pay more or less into a pension? | [Pension walkthrough](../examples/danish-income-tax/pension-og-fradrag.md) | Modeled tax and available-cash differences from explicitly supplied facts; unknown facts can withhold comparisons. |
| Could an invoice qualify for a service/handyman deduction? | [Invoice review](../examples/danish-income-tax/boligjob.md) | Supported factual conditions, allocation and annual limits; not automatic document authentication. |
| Are my recurring donations capped correctly when paid in instalments? | [Gift-agreement review](../examples/danish-income-tax/personskat-gaver.md) | Ordinary LL §12 payments share one limit per identified agreement before the common income ceiling. Missing/conflicting agreement facts or unconfirmed ordinary payment history withhold comparisons. |
| Does child maintenance belong in my calculation or my child's? | [Maintenance recipient guide](../examples/danish-income-tax/personskat-underholdsbidrag.md) | Separates payer, assessed child and adult alimony; inconsistent child-recipient birth dates withhold comparison, including for spouses. It does not authenticate identities. |
| Does my own-source calculation agree with the report? | [Canonical validity assessment](../examples/danish-income-tax/personskat-validity.md) | Supported model calculations after input checks, with explicit coverage reservations. |
| How do ferry or flight tickets combine with a short road commute? | [Ferry/flight guide](../examples/danish-income-tax/personskat-faerge-og-fly.md) | Deducts the unused daily 24-km threshold from documented ticket costs, preserving source expense and exact-øre trace. Requires correctly grouped travel days. |
| What explains my green-check credit? | [Compact green-check review](../examples/danish-income-tax/groen-check.md) or [Personskat-integrated calculation](../examples/danish-income-tax/personskat-groen-check.md) | The compact review uses supplied income; the integrated entry derives it from Personskat, inserts the credit once and excludes it from refund percentage compensation. Missing or unsupported facts withhold the integrated settlement. |
| Do my benefits affect the extra commuting deduction? | [Danish benefit review](../examples/danish-income-tax/personskat-dagpenge.md) | Separates personal income without wage AM from LL §9 C income; ordinary statutory benefits and explicit exclusions also compose into Personskat. |
| How do SU and a part-time job enter my tax? | [SU and student-job review](../examples/danish-income-tax/personskat-su.md) | Ordinary Danish grants enter personal income without wage AM or employment deductions; loan disbursements remain outside income. Corrections and foreign circumstances are explicitly outside this compact route. |
| How do folkepension and supplements enter my tax? | [Folkepension review](../examples/danish-income-tax/personskat-folkepension.md) | Separates taxable pension/elder cheque from covered tax-free supplements, without treating public pension as wages or private-pension payouts. It does not calculate benefit entitlement or income-tested benefit amounts. |
| How do disability, senior and early public pensions enter my tax? | [Public early-pension review](../examples/danish-income-tax/personskat-socialpension.md) | Distinguishes modern pensions and taxable/exempt old-regime components; keeps ATP/SUPP contributions separate without a second income deduction. Payment classification does not establish benefit entitlement or year-end green-check status. |

No LLM evaluates these rules at runtime. A person or assistant supplies and
classifies source facts; Futuruna executes the arithmetic and logic. Do not
turn inferred report residuals into supposedly independent input facts.

The first-use overview now distinguishes required source classification,
template placeholders and enumerated validity checks from complete coverage.
The [setup guide](../website/public/ai-setup.md) offers JSON input without a full
workbook and requires unresolved facts to remain unresolved. Ordinary wage help
is shared by taxpayer and spouse through typed metadata, with explicit
pension/ATP boundaries and source traces, guarded against renewed drift by the
[generated-contract test](../tests/tax_salary_metadata.test.mjs). This improves
input guidance; it does not prove that an AI classifies an arbitrary report
correctly or that every missing fact is mechanically detectable.

The ordinary-interest fields now share source-backed guidance for taxpayer and
spouse as well: own share, positive expense amounts, no duplicated sources and
separate income/expense totals. The [generated-input regression](../tests/tax_interest_input.test.mjs)
also checks existing sign validation and recorded ordinary netting results.
It does not detect duplicated documents behind a supplied aggregate; see the
[interest input boundary](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md#vejledning-i-det-genererede-input).

The [worked source-to-input rehearsal](../examples/danish-income-tax/fra-bilag-til-input.md)
adds explicitly fictional source excerpts, an authored field mapping with line
references, generated guidance and canonical output. It separates private
pension from payroll pension, employee ATP from total ATP, and repeated bank/
report observations from additional interest. Two resolved profiles use recorded
independent 2025 expectations. Employer rate pension/ATP has model checks
but an unresolved external disagreement, recorded below.
Unknown ATP is submitted with its explicit variant and withholds the comparison;
unknown interest share is held before submission because the amount field cannot
represent unknown. The [regression](../tests/tax_source_input.test.mjs) also
rejects contradictory source totals and unrepresentable fractional kroner in
this fixed example. This is not a general document importer or an evaluation of
arbitrary AIs reading real reports. No tax formula or contract type changed.

Saved conditional results can also be read through the
[local Danish result viewer](../examples/danish-income-tax/aarsopgoerelse-afstemning.md#læs-resultatet-på-dansk).
It retains every check, necessary amount, caveat and failed-case diagnostic;
it is a read-only presentation, not a second tax calculator or an authenticity
check of saved output.

Canonical `beregn_personskat` output now has its own
[local Danish summary](../examples/danish-income-tax/personskat-validity.md#læs-dit-gemte-resultat-på-dansk).
It leads with validity, withholds diagnostic amounts for invalid cases, retains
all assessment controls/caveats and every case diagnostic, and shows selected
main-person amounts with exact units. It does not validate every nested result,
authenticate the saved file, compare with observed report amounts, or summarize
the separate spouse/settlement calculations. No tax formula changed.

## Original findings: source closeout

“Implemented” below concerns the named defect, not all tax-law coverage.

| Finding | Source disposition | Evidence and remaining boundary |
| --- | --- | --- |
| F1: successful CLI output could hide invalid tax facts | Implemented | Model-owned validity/comparison gate (`306dfe14`), [guide](../examples/danish-income-tax/personskat-validity.md), [tests](../tests/personskat_validity.rs). Invalid/incomplete facts can withhold the comparison amount. Passing checks still does not certify complete legal coverage. |
| F2: missing 2026 senior employment deduction | Implemented | `a43b23e0`, [age/cohort checks](../tests/personskat_senior_test.runa), later [external observations](../examples/danish-income-tax/skatdk-fradrag-oere-ekstern.md). One senior rounding disagreement remains explicitly open below. |
| F3: missing single-parent fact route | Implemented | `e7ec6af7`, [source/eligibility guide](../examples/danish-income-tax/ligningsloven-par9j-enlig.md), [boundary checks](../tests/personskat_single_parent_test.runa). Benefit-period facts are required; being unmarried is not sufficient. |
| F4: no service/handyman route | Ordinary route implemented; special cases remain outside coverage | `72ab5171` and `571bf081`, [invoice contract and limitations](../examples/danish-income-tax/boligjob.md), [canonical tests](../tests/personskat_boligjob.rs). Insurance allocation, advance payments and earlier work are not silently accepted as zero entitlement. |
| F5: overwhelming workbook topology | Partially addressed | Sparse collection sheets and refresh (`219666d5`), guarded by [XLSX tests](../tests/calculate_cli.rs). The September 21 measurement fell to 59 sheets/54 visible; the main case sheet was still 763 columns. These are dated measurements, not current-size guarantees. Root-column usability remains `td-fe4b1f`; prefer compact question-specific entries. |
| F6: expensive schema export | Export size addressed; internal cost not eliminated | Lossless, explicit `compact-json` (`75cda03e`), [contract](reference/calculations.md), [reconstruction tests](../tests/calculate_cli.rs). The September 22 snapshot shrank from 124.26 MB pretty JSON to 6.80 MB compact. This does not prove lower extraction memory or remove cold-start cost. |
| F7: unsupported year produced an internal-looking error | Implemented | `cd6feb06`, [case-local regressions](../tests/personskat_validity.rs). Unsupported years identify the input and supported 2023–2026 boundary before tax evaluation; supported cases survive a mixed batch. |
| F8: hosted setup and delivered source differ | Public delivery still pending | Read-only check on September 25: [hosted guide](https://futuruna.com/ai-setup.md) lacked the runtime check, spouse-free review and pension walkthrough. [Latest public release](https://github.com/Futuruna/futuruna/releases/tag/v0.2.0) was still v0.2.0, published September 19. Source delivery is not binary or website publication. |

## Further correctness work already delivered

The [payroll group-life correction](../examples/danish-income-tax/personskat-gruppeliv.md)
adds the documented gross premium to employment/job deduction bases while
retaining net personal income, no second AM charge and no extra pension
deduction for this insurance. A reproduced 2025 low-wage case previously
accepted a deduction 123 DKK too low and tax 28.91 DKK too high. Unknown gross
now withholds the canonical comparison for both this route and the bounded
PBL19/PBL56 aggregate, including spouses. The new optional source field requires
a fresh template; generated help distinguishes payroll from pension-bonus
funding and warns against overlapping pension/report totals. Two recorded
anonymous public-form observations and focused [canonical coverage](../tests/personskat_group_life.test.mjs)
test the correction; this is not automatic report classification or coverage
of bonus-financed premiums.

The [labour-hire age regression](../tests/personskat_labour_hire_age.test.mjs)
closes an input-consistency gap: a separate labour-hire age can no longer
contradict the canonical taxpayer's birth date while retaining a comparison.
The guard covers spouses and the ordinary-tax election; source-backed field
help now identifies the single assessed person. Correct youth/adult rates are
unchanged. This does not authenticate identity or establish external tax
conformance.

The adjacent hydrocarbon investigation did **not** reproduce an accepted
canonical comparison with a contradictory age: the existing special-DIS
coverage gate already withholds the comparison, even for consistent facts.
The [coverage regression](../tests/personskat_dis_coverage.test.mjs) now protects
that boundary, while generated help and main/spouse diagnostics explain the
unsupported annual composition explicitly. Component readiness and a zero
component tax are not annual-tax approval. No hydrocarbon age guard or tax
formula was changed; see the [coverage explanation](../examples/danish-income-tax/personskat-validity.md#særlige-dis-skattepligtspositioner).

The [Boligjob supplier-age regression](../tests/boligjob_supplier_age.test.mjs)
corrects a cross-year eligibility error: delayed payment no longer makes work
by a private supplier under 18 at work-year-end deductible. The claimant's own
age condition remains separate. Compact and canonical checks cover taxpayer,
spouse and typed supplier guidance; see the
[invoice guide](../examples/danish-income-tax/boligjob.md#private-udførere-arbejdsår-er-ikke-altid-fradragsår).
This tests the source-backed year distinction, not supplier identity or invoice
authenticity, and does not resolve the existing rounding/coverage limitations.

Employee-expense work-use shares below zero or above 100% now invalidate the
facts instead of silently producing a valid zero deduction. The control covers
taxpayer and spouse while retaining zero use and ordinary ineligibility as
valid facts. [Focused component boundaries](../tests/personskat_employee_expenses_test.runa)
and [canonical/schema regression](../tests/personskat_employee_expenses.test.mjs)
cover the validity gate and shared input guidance. The
[input guide](../examples/danish-income-tax/personskat-validity.md#øvrige-lønmodtagerudgifter)
also warns against reusing net box-58 deductions as raw expenses or applying the
work-use share twice. This is a validity and input-guidance correction, not
verification of an arbitrary asset's depreciation schedule or entitlement.

Permanent coverage also protects fail-closed arithmetic/undefined rules,
duplicate JSON rejection, pension payout timing/completeness, employer pension
and ATP routing, foreign employment allocation, individual union-fee caps, and
recipient-rate spouse allowance/loss transfers. See the
[compatibility record](compatibility-guides/0.2.x.md),
[pension tests](../tests/personskat_pension_timing.rs),
[foreign-allocation tests](../tests/personskat_foreign_canonical.rs), and
[report tests](../tests/tax_report_reconciliation.rs).

The canonical commuting checks now reject every negative bridge-crossing
count and travel-day counts exceeding the income year's calendar length,
including for an active spouse. The [commuting input guide](../examples/danish-income-tax/personskat-validity.md#commuting-input-checks)
distinguishes these validity checks from actual travel evidence and coverage
of the extra low-income deduction.

The ferry/flight correction is guarded by [component examples and boundaries](../tests/personskat_ferry_test.runa)
and [canonical cases](../tests/personskat_ferry.test.mjs), including a spouse and
the annual low-income supplement. For 100 days with 16 road km and 120 DKK
tickets per day, the previous 12,000 DKK deduction becomes 10,216 DKK in 2025
or 9,464 DKK in 2026. Current official guidance supplies these daily examples;
this does not independently establish annual rounding or heterogeneous-day
allocation. The rural-threshold interpretation is explicit in the guide.
Verification: ten component assertions pass in interpreted and native execution;
seven canonical cases pass, with eight projected question/help/source checks
covering both taxpayer and spouse. Percentage-supplement truncation remains the
existing model convention, not a newly verified official rounding result.

Recurring-gift regression checks reproduce and fix an over-deduction: two
6,000 DKK payments under one 10,000 DKK annual agreement previously produced
12,000 DKK before the general ceiling. The [component checks](../tests/personskat_recurring_gifts_test.runa)
and [eight canonical cases](../tests/personskat_recurring_gifts.test.mjs) now
verify agreement-level caps, separate agreements and invalid-fact propagation,
including spouses and projected input help. This is a source-backed model
correction, not independent official-calculator conformance. New required
agreement facts require [template migration](../examples/danish-income-tax/personskat-gaver.md#eksisterende-input-skal-gennemgås-igen).

Child-maintenance recipient checks now reject a birth-date contradiction
between an assessed person and a child entered as that person's maintenance
recipient. Before correction, a fictional parent was incorrectly accepted with
1,397 DKK of the child's income. [Focused checks](../tests/personskat_maintenance_recipient.test.mjs)
cover correct child/payer/alimony routes and main/spouse contradictions. This
is input-consistency evidence, not identity authentication; see the
[role and migration guide](../examples/danish-income-tax/personskat-underholdsbidrag.md).

The latest targeted conformance additions include
[pension deduction rounding](../examples/danish-income-tax/skatdk-pensionsfradrag-ekstern.md)
and [ordinary interest deductions](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md).
The latter matched six independently observed 2025 cases through total tax;
no interest formula correction was needed. These fictional observations do not
establish universal or historical conformance.

The [employer-pension/ATP observations](../examples/danish-income-tax/skatdk-arbejdsgiverpension-ekstern.md)
add two 2025 official-form matches (ordinary ATP below the employment-deduction
cap, and lifetime pension plus ATP) and two unresolved employer-rate pension
disagreements. The latter differ by 5,520 DKK in extra pension deduction and
1,297.20 DKK in tax, not a rounding tolerance. Public HTTP form submissions
and restored inputs were inspected; a fresh graphical-browser reproduction
and explanation remain open (`td-0e5d15`). No formula was changed to fit the
observations. Canonical caveats and the source demo expose the disagreement;
the [offline regression](../tests/tax_employer_pension_conformance.test.mjs)
keeps matches, disagreements and an invalid ATP control distinct.

## Limits to keep visible

- Employer-rate pension conformance with the anonymous 2025 form remains
  unresolved (`td-0e5d15`, above). This is not evidence that an actual annual
  assessment is wrong, nor grounds to move contributions to another field.
- Commuting phaseout income now composes ordinary statutory benefits and the
  annual AMBL §§4–5 business result (`td-b8013e`). Generic A-kasse facts alone
  do not establish the statutory classification. Unknown facts withhold a
  comparison where they could alter the deduction; afterløn, other benefits,
  foreign benefits and reperiodization are not added by this change. Focused
  source-model regressions are not independent official-calculator conformance
  for all benefit/business profiles.
- Three observed 2026 **one-krone deduction differences**, at exact-one-øre
  intermediate boundaries, remain unexplained (`td-68c9d3`).
  [The amounts and profiles are recorded](../examples/danish-income-tax/skatdk-fradrag-oere-ekstern.md#kendte-afvigelser--ikke-match).
  Do not introduce a tolerance, adjust source facts or declare a taxpayer's
  return wrong to hide them. Earlier-year administrative rounding also needs
  independent evidence (`td-3f1c08`).
  The canonical result's `vurdering.forbehold` now includes this limitation;
  amounts and validity are unchanged. It is not a tolerance or an explanation
  of every small difference.
- The full Personskat native-codegen path remains incomplete (`td-124b83`).
  `template`/`call` is the tested canonical tax-input path. Native checks of
  smaller components do not establish native support for the entire graph.
  The personal-allowance §§10–13 import now has explicit closed branch coverage;
  [focused regressions](../tests/personfradrag_domain_test.runa) preserve the
  historical base amounts, supported annual amounts and missing-year rejection,
  alongside native spouse-allowance transfer checks. No tax rate or compiler
  totality policy was changed by this repair.
- The full workbook's width and special invoice cases remain the explicit
  limits in F4/F5, not reasons to withhold the compact supported workflows.
- The original v0.2.0 download predates required safety fixes. Passing the small
  runtime preflight is necessary, not sufficient evidence of tax-law correctness.
  Publication requires a separately approved release/deployment and its
  [release gates](releasing.md).

## Verification policy for this snapshot

The last compiler change (`18ffb04b`) completed mint, differential and core
canary gates. Later model corrections have their own focused canonical and
component checks; that earlier full gate is not a claim to have rerun every
test on every later model revision. Known skips and ignored tests are not passes.

The original closeout added documentation and a cheap setup regression.
Subsequent commuting corrections have targeted model tests, not a new full
compiler-suite run. Compiler changes still require the
[contributor ratchet](../CONTRIBUTING.md); model or guide changes require checks
proportionate to what actually changed.

Before public launch, align the guide and binaries with an approved candidate,
verify that candidate under the release runbook, and keep the above coverage
limits visible. Do not describe this snapshot as a complete personal-tax audit
certification or a public release.
