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
| Does my own-source calculation agree with the report? | [Canonical validity assessment](../examples/danish-income-tax/personskat-validity.md) | Supported model calculations after input checks, with explicit coverage reservations. |
| What explains my green-check credit? | [Compact green-check review](../examples/danish-income-tax/groen-check.md) or [Personskat-integrated calculation](../examples/danish-income-tax/personskat-groen-check.md) | The compact review uses supplied income; the integrated entry derives it from Personskat, inserts the credit once and excludes it from refund percentage compensation. Missing or unsupported facts withhold the integrated settlement. |
| Do my benefits affect the extra commuting deduction? | [Danish benefit review](../examples/danish-income-tax/personskat-dagpenge.md) | Separates personal income without wage AM from LL §9 C income; ordinary statutory benefits and explicit exclusions also compose into Personskat. |
| How do SU and a part-time job enter my tax? | [SU and student-job review](../examples/danish-income-tax/personskat-su.md) | Ordinary Danish grants enter personal income without wage AM or employment deductions; loan disbursements remain outside income. Corrections and foreign circumstances are explicitly outside this compact route. |
| How do folkepension and supplements enter my tax? | [Folkepension review](../examples/danish-income-tax/personskat-folkepension.md) | Separates taxable pension/elder cheque from covered tax-free supplements, without treating public pension as wages or private-pension payouts. It does not calculate benefit entitlement or income-tested benefit amounts. |

No LLM evaluates these rules at runtime. A person or assistant supplies and
classifies source facts; Futuruna executes the arithmetic and logic. Do not
turn inferred report residuals into supposedly independent input facts.

Saved conditional results can also be read through the
[local Danish result viewer](../examples/danish-income-tax/aarsopgoerelse-afstemning.md#læs-resultatet-på-dansk).
It retains every check, necessary amount, caveat and failed-case diagnostic;
it is a read-only presentation, not a second tax calculator or an authenticity
check of saved output.

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

The latest targeted conformance additions include
[pension deduction rounding](../examples/danish-income-tax/skatdk-pensionsfradrag-ekstern.md)
and [ordinary interest deductions](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md).
The latter matched six independently observed 2025 cases through total tax;
no interest formula correction was needed. These fictional observations do not
establish universal or historical conformance.

## Limits to keep visible

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
