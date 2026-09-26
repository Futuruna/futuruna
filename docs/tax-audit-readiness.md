# Danish tax-audit readiness

Evidence snapshot: **26 September 2026**; named commits below identify earlier
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
| Where does my rental operating result belong? | [Property-income routing](../examples/danish-income-tax/personskat-ejendomsdrift.md) | Nonzero amounts outside the selected PSL4(1)(6) branch withhold comparison; exclusion is not tax exemption or automatic reclassification. |
| Does a financial payment belong in this income branch? | [Financial-post routing](../examples/danish-income-tax/personskat-finansielle-poster.md) | Nonzero income outside the selected PSL4(1)(5a)/(5b) branch withholds comparison; legitimate zero expense deductions remain distinct. |
| Did Danish tax liability begin or end during the year? | [Part-year intake and assessment](../examples/danish-income-tax/personskat-delaar.md) | Explicit liability periods and source treatment; the final part-year assessment, not a nested ordinary amount, controls comparison. Fewer months worked do not themselves establish part-year liability. |
| How do ferry or flight tickets combine with a short road commute? | [Ferry/flight guide](../examples/danish-income-tax/personskat-faerge-og-fly.md) | Deducts the unused daily 24-km threshold from documented ticket costs, preserving source expense and exact-øre trace. Requires correctly grouped travel days. |
| What explains my green-check credit? | [Compact green-check review](../examples/danish-income-tax/groen-check.md) or [Personskat-integrated calculation](../examples/danish-income-tax/personskat-groen-check.md) | The compact review uses supplied income; the integrated entry derives it from Personskat, inserts the credit once and excludes it from refund percentage compensation. Missing or unsupported facts withhold the integrated settlement. |
| Do my benefits affect the extra commuting deduction? | [Danish benefit review](../examples/danish-income-tax/personskat-dagpenge.md) | Separates personal income without wage AM from LL §9 C income; ordinary statutory benefits and explicit exclusions also compose into Personskat. |
| How do SU and a part-time job enter my tax? | [SU and student-job review](../examples/danish-income-tax/personskat-su.md) | Ordinary Danish grants enter personal income without wage AM or employment deductions; loan disbursements remain outside income. Corrections and foreign circumstances are explicitly outside this compact route. |
| How do folkepension and supplements enter my tax? | [Folkepension review](../examples/danish-income-tax/personskat-folkepension.md) | Separates taxable pension/elder cheque from covered tax-free supplements, without treating public pension as wages or private-pension payouts. It does not calculate benefit entitlement or income-tested benefit amounts. |
| How do disability, senior and early public pensions enter my tax? | [Public early-pension review](../examples/danish-income-tax/personskat-socialpension.md) | Distinguishes modern pensions and taxable/exempt old-regime components; keeps ATP/SUPP contributions separate without a second income deduction. Payment classification does not establish benefit entitlement or year-end green-check status. |

No LLM evaluates these rules at runtime. A person or assistant supplies and
classifies source facts; Futuruna executes the arithmetic and logic. Do not
turn inferred report residuals into supposedly independent input facts.

Current-year property-tax age assertions now reconcile against the taxpayer's
and current cohabiting spouse's respective canonical birth dates. A fictional
35-year-old previously obtained a valid comparison with false retirement relief;
the new guard withholds that comparison and reports the model-derived expected
date/status without altering source facts. Both ownership directions and the
year-end boundary are covered by [focused cases](../tests/personskat_property_age.test.mjs).
Typed field guidance carries ESL §25 and social-pension §1a sources.
The owner's explicitly own 2024 rebate basis now also checks that same birth
date, assessed for 2024 rather than the current calculation year. Before this
guard, a false historical retirement claim for a 1990-born owner reduced a
valid annual comparison by 1,900 DKK through the historical tax-growth cap.
Historical partner identities, an acquired spouse's rebate basis and survivor
facts are not automatically joined to the current household. The regression
covers those distinct routes, active-spouse own history, a property now rented
out and identifying the erroneous row among multiple properties. This does
not validate every historical EVSL §9 benefit qualification; see the
[scope and migration](../examples/danish-income-tax/personskat-validity.md#folkepensionsalder-og-ejendomsskat).

Canonical spouse intake now distinguishes unavailable facts from no spouse.
Fresh templates select `ÆgtefællegrundlagUoplyst`; this withholds independent
comparison, including the period/documented annual basis in PSL14 and integrated
green-check settlement. Shared typed guidance reaches those wrappers and routes
missing documents to conditional report review without inventing spouse income.
The [focused regression](../tests/tax_spouse_input.test.mjs) checks actual
templates, projected sources, supported controls and unresolved inputs.
This does not authenticate an asserted known relationship or implement additional
marriage/separation rules. Existing tax formulas and conditional reconciliation
are unchanged; [fresh templates are required](../examples/danish-income-tax/personskat-validity.md#manglende-ægtefælleoplysninger-er-ikke-ingen-ægtefælle).

Church-tax intake now refers to the income year, with shared taxpayer/spouse
guidance and official source traces. Canonical `lønmodtager.kirkeskat` now
distinguishes unknown, no-year, whole-year and part-year status. Unknown and
part-year cases mechanically withhold the independent comparison for the
taxpayer or an active spouse, including the part-year composition. This does
not authenticate an asserted whole-year status or implement membership
proration. The compact conditional report input remains separate and retains
a full-year upper bound, not exact period-specific entitlement. See the
[coverage boundary and Preview input migration](../examples/danish-income-tax/personskat-validity.md#kirkeskat-gælder-indkomståret-ikke-status-i-dag).

The [church-status regression](../tests/tax_church_input.test.mjs) checks real
templates, source-linked input guidance, supported whole-year controls and
rejected legacy/malformed inputs. It also checks that an unresolved taxpayer
or spouse in a separately documented annual basis cannot bypass the final
part-year assessment. [Green-check coverage](../tests/personskat_green_check.test.mjs)
retains the same boundary through credit calculation and settlement. These are
model and intake-contract checks, not independent period-tax conformance or
validation of an AI's reading of a real document. Fresh templates are required;
personal records are not automatically migrated.

The first-use overview now distinguishes required source classification,
template placeholders and enumerated validity checks from complete coverage.
The [setup guide](../website/public/ai-setup.md) offers JSON input without a full
workbook and requires unresolved facts to remain unresolved. Ordinary wage help
is shared by taxpayer and spouse through typed metadata, with explicit
pension/ATP boundaries and source traces, guarded against renewed drift by the
[generated-contract test](../tests/tax_salary_metadata.test.mjs). This improves
input guidance; it does not prove that an AI classifies an arbitrary report
correctly or that every missing fact is mechanically detectable.

The [salary source-mapping correction](../examples/danish-income-tax/fra-bilag-til-input.md#lønlinjer-kræver-klassifikation)
closes a contradictory helper path: an arbitrary line labelled personal income
previously became ordinary wage with no remaining factual requirements. The
generic helper now requests classification; a separate source-confirmed route
checks year, sign and precision while retaining its factual obligations. Neither
route authenticates a document or submits a canonical tax case. Focused
[behavior](../tests/personskat_salary_mapping_test.runa) and
[metadata](../tests/tax_salary_mapping_metadata.test.mjs) regressions protect the
boundary, source references and linked code spans. Canonical tax formulas and
input contracts are unchanged; old source mappings require review, not automatic
renaming. This is not evidence of reliable AI interpretation of unseen reports.

The [read-only input navigator](../scripts/calculation-input-guide.py) makes the
large generated contract usable one branch at a time, including type choices,
collection/optional context and exact field/ancestor metadata. It preserves
referenced sources and keeps unannotated fields visible; pagination and separate
root-metadata access are explicit. The [focused regression](../scripts/tests/test_calculation_input_guide.py)
checks a fresh canonical Personskat schema, taxpayer/spouse wage help and expense
warnings, as well as malformed inputs. This improves access to the interview
contract; it neither fills facts nor validates legal coverage, schema authenticity
or arbitrary AI document interpretation. See the [navigation instructions](reference/calculations.md#inspect-one-input-branch).

The [senior](../examples/danish-income-tax/ligningsloven-par9j-senior.md) and
[single-parent](../examples/danish-income-tax/ligningsloven-par9j-enlig.md)
deduction guides now describe the current øre-truncation/krone-ceiling convention
instead of the superseded claim that fractional kroner are discarded. Their
typed metadata distinguishes legal sources, annual rates, allocation evidence,
rounding assumptions and warnings. Single-parent benefit/quarter fields retain
those sources for both taxpayer and spouse in the generated contract, covered
by the [metadata regression](../tests/tax_supplementary_metadata.test.mjs).
No tax formula changed. The metadata changes the input contract fingerprint;
regenerate templates without treating placeholders as facts. Selected calculator
observations still do not establish universal or historical rounding conformance.

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

Revised reports can now select an explicit
[corrected-payment comparison](../examples/danish-income-tax/aarsopgoerelse-afstemning.md#ændret-rapport-beløb-til-betaling-eller-udbetaling).
A smaller annual refund can require repayment; reduced restskat can refund an
earlier payment. The new optional observation keeps that direction separate from
annual surplus/restskat, checks exact øre before whole-krone projection and
preserves missing values. The [focused regression](../tests/tax_report_payment.test.mjs)
covers both directions, wrong-direction one-øre boundaries, incomplete facts,
overlapping fields, generated interview metadata, indexed legal-source/code-span
links and actual viewer output. Legacy routes remain available. These are
fictional arithmetic cases, not independently verified collection/interest law or
unseen-document AI intake. Fresh report templates are required; this does not
change canonical Personskat tax or resolve the section-55 credit-floor question.

The conditional report contract now also carries explicit questions, units,
sign conventions and source traces on all six name/amount fields in its three
post lists, not only on the parent collections. The
[row-input regression](../tests/tax_report_input_metadata.test.mjs) checks the
generated contract and fictional signed refund/restskat observations, including
wrong signs, units, duplicate rows, unknown completeness and rejected localized
amount text. This improves AI intake guidance; it does not parse documents or
prove their classification. Metadata changes require fresh report templates.

Canonical `beregn_personskat` output now has its own
[local Danish summary](../examples/danish-income-tax/personskat-validity.md#læs-dit-gemte-resultat-på-dansk).
It leads with validity, withholds diagnostic amounts for invalid cases, retains
all assessment controls/caveats and every case diagnostic, and shows selected
main-person amounts with exact units. It does not validate every nested result,
authenticate the saved file, compare with observed report amounts, or summarize
the separate spouse/settlement calculations. No tax formula changed.

The pension walkthrough now distinguishes the assessed person's comparison
amount from the household's tax change. A [four-case fictional couple](../examples/danish-income-tax/pension-og-fradrag.md#din-skat-og-husstandens-skat-er-ikke-det-samme)
has unchanged own tax but a 3,833 DKK modeled spouse saving when private
contributions increase by 10,000 DKK. The household's modeled cash reduction
is 6,167 DKK, not 10,000 DKK. All four gated main-person amounts are required;
missing spouse facts cannot be replaced by fixed inferred transfers. The
[runnable example](../examples/danish-income-tax/pension-par-demo.mjs) preserves
all assessments and warnings, while [focused coverage](../tests/pension_couple_demo.test.mjs)
checks mirrored inputs, unchanged pay and withheld comparisons. These are
canonical model observations, not new external tax conformance. Arbitrary
role-swapping of personal cases, including pair-level losses/relief, is not
implemented or implied. No tax formula or input contract changed.

The separate part-year entry has a final nullable comparison assessment.
Its initial addition used the existing validity conditions. A fictional valid 2025 case gives
103,831.01 DKK after PSL14 but 94,331.44 DKK in its ordinary intermediate result;
a source mismatch leaves that intermediate valid while invalidating the final
part-year result. The [regression](../tests/personskat_partyear_validity.test.mjs)
guards the correct output level and generated source-linked warning. That
initial addition changed no tax formula, raw amount or existing validity
condition; the later final-settlement guard is described below. This does not infer
tax liability, establish external conformance or cover arbitrary mixed periods;
the ordinary entry still needs the correct intake route chosen explicitly.

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

The [ordinary-wage intake regression](../tests/personskat_salary_input.test.mjs)
reproduces an accepted unsupported offset: -10,000 DKK in the ordinary wage
field reduced modeled tax on 100,000 DKK of SU by 4,066.92 DKK without repayment
or correction-year evidence. The comparison now requires a nonnegative original
ordinary wage total for each person, before derived spouse-business wage
adjustments. Raw signed facts remain diagnostic; genuine negative income in
other supported branches is not prohibited. Shared generated guidance separates
correction deltas from documented corrected annual totals and distinguishes
original-year corrections from later-arising repayment losses. This is a
coverage safeguard, not a finding that every wage repayment is nondeductible,
and not automatic correction/reopening or document classification. Positive
totals can still be misclassified. See the [scope and migration](../examples/danish-income-tax/personskat-validity.md#lønrettelser-og-negative-beløb).

That regression also found a composition omission: a part-year result accepted
zero tax despite the period's canonical wage rejection. The wrapper inherits
canonical calculation controls, including active spouse controls, in both bases.
The temporary path-based stage exception has now been replaced with typed
`kontrolgrundlag.beregning`/`afregning` groups. Source-credit checks remain in
the calculation group even when their path is `årsopgørelse`.

The [settlement-stage regression](../tests/personskat_settlement_stage.test.mjs)
reproduced accepted wrong payment direction and wrong year in the final
part-year assessment. The final comparison and `input_gyldigt` now require the
selected settlement to match the recomputed part-year tax and existing KSL
fact checks. Tax arithmetic and source facts are unchanged; a valid final
restskat can coexist with an ordinary intermediate refund. Canonical and
green-check assessments use the same explicit groups, preserving green-check
payment validation after its credit. The
[small boundary regression](../tests/personskat_partyear_input_controls_test.runa)
checks group membership independently of control names. The Danish viewer
validates and displays both groups while retaining older saved-output support.
This is source-model consistency, not new independent payment-law conformance.
Output contracts changed: generate fresh templates and recalculate from reviewed
facts. See the [part-year guide](../examples/danish-income-tax/personskat-delaar.md#afregning-efter-den-endelige-delårsskat).

The [annual credit guidance](../examples/danish-income-tax/personskat-skattekreditter.md)
corrects an actual generated question that asked for paid B-skat, although the
encoded KSL60(1)(b,c) credit basis is the required tax-bill/section-68 amount.
Shared typed metadata now covers kroner and øre, distinguishing withheld A-skat/AM,
assessed amounts, net voluntary section-59 payments and section-55 refunds.
Historical machine keys and tax formulas remain unchanged; fresh schemas/templates
are required. The [focused regression](../tests/tax_credit_input.test.mjs)
checks the canonical generated contract, projection through a small nested-list
contract and six fictional annual calculations, plus one rejected missing amount.
Using 30,000 DKK paid instead of 40,000 DKK assessed raises modeled restskat by
10,000 DKK without changing tax. Such wrong positive scalars still pass the
existing model checks: this is improved source-to-field guidance, not automatic
document classification, credit authentication or a new completeness gate.

The subsequent [credit-sign regression](../tests/tax_credit_signs.test.mjs)
reproduced accepted negative section-55 refunds in both unit routes. A -1,000
DKK input became an extra credit rather than a subtraction, understating restskat
by 2,000 DKK relative to a positive refund. Canonical intake now requires all
12 raw credit/refund fields to follow their nonnegative gross-amount convention.
A calculation-stage `årsopgørelse.kreditter` control withholds comparison while
preserving source values and raw diagnostic arithmetic. Shared metadata and
[guidance](../examples/danish-income-tax/personskat-skattekreditter.md#fortegn-og-rettelser)
distinguish report signs and correction deltas from supported annual totals.
The bounded matrix covers all 24 field/unit negative edges and zero/positive
controls; four canonical scenarios cover the refund reversal and a B-skat edge.
Existing credit guidance and six positive-scalar mapping scenarios remain covered.
This does not authenticate amounts or establish a refund-versus-gross-credit cap.
Correction histories and the possible negative-net-credit floor remain separate
`td-145088`; part-year/green-check execution was not rerun for this change.

The [refund source-mapping regression](../tests/personskat_refund_mapping_test.runa)
then reproduced a separate helper defect: a fictional prior annual refund mapped
straight to `tilbagebetalt_par55_øre`, with no remaining fact requirements.
That helper now requests the decision/payment evidence and refund classification.
An explicit section-55 route preserves exact øre, checks year/nonnegative amount
and retains source-classification/no-double-subtraction requirements. Six focused
invariants cover ambiguous labels and signs, zero, exact øre, wrong years,
incomplete source identity and an unchanged adjacent credit mapping.
The [metadata regression](../tests/tax_refund_mapping_metadata.test.mjs) checks
actual `runa meta --json` output: both helpers are linked to typed statutory
provenance and the warning. The [migration guide](../examples/danish-income-tax/personskat-skattekreditter.md#tidligere-udbetaling-er-ikke-automatisk--55)
keeps prior annual refunds in the separate payment-reconciliation route.
This is not automatic classification of PDFs or a new canonical credit gate;
tax formulas, canonical input types and the unresolved net-credit floor are unchanged.

The [employer bank-pension year route](../examples/danish-income-tax/personskat-bankpension-aar.md)
now separates unresolved treatment, ordinary payment-year treatment and a
documented authority approval of the previous wage-withholding year. Previously
three fictional unresolved taxpayer/spouse cases were accepted. Annual intake
now withholds them and can apply a documented approval without rewriting the
actual payment year. The approval's shared gross budget, year/date consistency
and source references are checked; the existing employer-net and private shared
rate-cap calculations consume the approved year. Generated guidance is shared
by taxpayer/spouse. The model does not issue or authenticate approvals, invent
a deadline for “reasonable time”, or establish administrative conformance.
Special-plan/correction combinations and cross-file duplicate evidence remain
outside this route. Fresh templates and reviewed facts are required.

The [pension re-payment regression](../tests/personskat_pension_redeposit.test.mjs)
addresses a reproduced acceptance gap: a correction Boolean previously sufficed
without original-payment or refund evidence. PBL22E now has optional typed
source facts, calculated 30-day/19-January deadlines and shared refund-amount
checks. Timely bank re-payments can retain the original deduction year without
rewriting the actual payment year. Ordinary insurance due-year precedence and
ordinary late re-payment treatment remain distinct. Missing, contradictory,
late-refund or overallocated facts withhold comparison for taxpayer/spouse.
A fictional 40,000 DKK private correction produces the same 196,612.54 DKK tax
as the ordinary control; splitting that refund does not create another allowance.
The [guide](../examples/danish-income-tax/personskat-pensionskorrektion.md)
and shared generated metadata explain gross/AM amounts, original remainders,
source identities and migration. [Component checks](../tests/personskat_pension_redeposit_test.runa)
cover calendar edges and year selection. These are source-model checks, not
new independent administrative observations, payroll correction, general
refund-chain support or full special-plan conformance. Those combinations
remain explicit follow-up `td-9597b1`; approved bank-year handling remains
`td-fa5e6d`. Fresh templates and review of affected facts are required.

The [pension payment-year regression](../tests/personskat_pension_payment_year.test.mjs)
reproduces accepted contradictory deadline facts and ordinary plans using §15A
timing choices. In a fictional insurance case due in 2024 and paid timely in
2025, an inapplicable choice moved a 40,000 DKK deduction into 2025 and lowered
the accepted tax by 15,332 DKK. Annual intake now withholds comparisons for
these contradictions, including spouses, without rewriting facts or changing
valid bank/insurance year rules. Shared generated guidance correctly limits
the 1 April bankday extension and keeps source traces for both persons.
The [guide and migration](../examples/danish-income-tax/pension-og-fradrag.md#betalingsår-og-forfaldsår)
distinguish year consistency from actual date evidence, special bank approvals
and correction rules. These are fictional source-model checks, not new
independent tax-administration observations.

The [financial-income routing safeguard](../examples/danish-income-tax/personskat-finansielle-poster.md)
addresses two reproduced omissions: a fictional 15,000 DKK investment-certificate
gain and a 9,000 DKK payment from an unlisted intermediary each left annual tax
unchanged at 211,944.54 DKK with a valid comparison. Canonical intake now requires
nonzero income to be covered by the selected PSL4(1)(5a)/(5b) branch; underlying
law exclusions and source facts remain intact. This is a composition boundary,
not a finding that every excluded amount is taxable or an automatic alternative
classification. The precise financial-list control propagates to the taxpayer,
spouse and both part-year bases. Lawful zero deductions under nr7 and covered
personal reclassification under stk6 remain supported.
[Focused fictional cases](../tests/personskat_financial_route.test.mjs) distinguish
these outcomes and check projected source-linked guidance for both persons.
Fresh schemas/templates are required after the metadata change; input types
are unchanged. These cases do not establish document-reading accuracy or
independent official-calculator conformance.

The [property-income routing safeguard](../examples/danish-income-tax/personskat-ejendomsdrift.md)
closes another reproduced silent omission: a fictional 20,000 DKK commercial
property profit was excluded by PSL4(1)(6), yet the annual comparison remained
valid and unchanged from the no-property baseline. Nonzero excluded gains or
losses now invalidate annual composition with a precise taxpayer/spouse input
path, including both bases of PSL14. The root-law component keeps its correct
non-applicability and source facts. Six source-linked interview fields now
follow the reusable type instead of appearing only on the main input.
[Focused coverage](../tests/personskat_property_route.test.mjs) distinguishes
supported amounts, confirmed zero, misplaced amounts and separately supplied
ordinary business facts. No automatic document classification, new expense
entitlement or independent official-calculator conformance is established.
Metadata changes require fresh templates; input types are unchanged.

The [honorarium intake safeguard](../examples/danish-income-tax/personskat-honorar.md)
closes a reproduced silent omission: 50,000 DKK entered in the fee list but
classified as employment/self-employment previously contributed zero while
the annual comparison remained valid. The component still correctly reports
non-applicability of AMBL2(1)(2); annual composition now withholds comparison,
including for a calculated spouse and the part-year flow. Correct-route fee
arithmetic is unchanged. [Focused coverage](../tests/personskat_work_income_route.test.mjs)
checks misplaced and mixed routes, duplicate identifiers, year mismatch and
projected source guidance. This does not classify documents, detect duplicates
under different identifiers.

The [honorarium expense route](../examples/danish-income-tax/personskat-honorar.md)
now separates documented ordinary costs from gross AM income, with explicit
unknown/complete facts and per-cost source guidance. A fictional 50,000 DKK fee
with 10,000 DKK costs previously had no explicit expense route: netting the fee
incorrectly lowered AM by 800 DKK. The separate cost now reduces personal income
without changing AM or employment/job deduction bases. Duplicate identifiers
across fee rows and unsupported facts withhold comparison. The coverage guard
also reaches spouse and part-year composition; it does not cap a legal deduction.
Loss-making activities and special costs remain `td-e2c310`: C.C.1.2.3 flags
SKM2025.490.ØLR against source-income restriction, so the model does not assume
the old restriction settles their treatment. New required expense facts need
fresh templates. [Focused checks](../tests/personskat_honorar_expenses.test.mjs)
are canonical model evidence, not independent official-calculator conformance.

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

A separate [pension-bonus group-life route](../examples/danish-income-tax/personskat-gruppeliv.md#gruppeliv-betalt-af-pensionsbonus)
now accepts documented premiums funded by bonus on a deductible pension,
without treating them as payroll, new pension deposits or pension payouts.
It adds personal income without AM or employment/job/extra-pension deductions.
Unknown financing, payroll financing, bonus from the insurance itself, invalid
source facts, duplicate identifiers and year mismatch withhold comparison;
taxpayer and spouse receive the same source-backed interview fields and a
targeted explanation. The new variant requires a fresh contract/template.
This does not classify an aggregate annual-report line automatically or resolve
all insurance ownership/bonus cases. The [component](../tests/personskat_group_bonus_test.runa)
and [canonical regression](../tests/personskat_group_bonus.test.mjs) distinguish
legal source classification from the two recorded 2025 public-form observations
of non-AM group-life income without work deductions.

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

The subsequent [four married-household observations](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md#ægtepar-særskilte-observationer-for-begge-personer)
did find a canonical composition error: with 20,000 DKK positive capital income
and a spouse's larger negative capital income, bundskat omitted the PSL6(3)
offset and total tax was 2,402 DKK too high. Both exact-øre and legacy whole-krone
composition now call the existing source rule, preserving original capital
income for municipal tax and PSL11. The [offline regression](../tests/personskat_married_interest.test.mjs)
checks each person's gated amount, bundskat, municipal tax and used interest
credit against the separately observed specifications. It also checks the
zero-income spouse's outgoing loss, personal allowance and unused PSL11 credit.
After the correction all eight separately assessed people match the recorded
tax amounts to the øre, without changing the expected amounts or source facts.
The [small boundary regression](../tests/personskat_bundskat_spouse_test.runa)
covers partial/full offset, the cohabitation condition and unchanged own facts.
This is annual-tax conformance evidence, not an independent check of part-year
capital contexts and scaling.
Input types and rates are unchanged; affected saved annual results need to be
recalculated, without changing the source facts.

The adjacent [part-year capital regression](../tests/personskat_partyear_capital.test.mjs)
reproduces two composition errors: an accepted bundskat ratio omits the spouse
offset, and the PSL11 explanation reports a single-person basis even while the
canonical calculation applies a spouse-adjusted credit. Each period now keeps
its own canonical spouse capital context. All four bundskat scaling consumers
use the existing PSL6(3) rule; original own income remains available for the
other taxes. In a fictional 2025 arrival-year case with separately documented
annual amounts, the ratio changes from 204000/408000 to 194000/393000 and the
comparison from 72,547.66 DKK to 72,286.83 DKK. In the negative-capital case,
the displayed period/annual credits become 3,200/6,400 DKK instead of 4,000/4,000 DKK,
consistent with the already-used amounts. Single-person and no-cohabitation
controls remain unchanged. The test also checks source-linked generated input
help: spouse annual amounts must not be guessed or copied from the period.
These are source-model checks, not independent official part-year observations
or broad couple coverage. See the
[input and migration guide](../examples/danish-income-tax/personskat-delaar.md).

The [adjacent share-income regression](../tests/personskat_partyear_shares.test.mjs)
reproduces four accepted but inconsistent share-tax cases. The part-year path
now keeps source reconciliation before PSL8a spouse offsets separate from the
net share-tax context, including transferred thresholds. Previously an unused
spouse threshold could leave 8,775 DKK of low-rate tax uncounted, while a fully
offset 20,000 DKK share amount could incorrectly retain 5,400 DKK tax. The
documented annual basis must also agree on spouse presence, year-end cohabitation
and the spouse's tax year; contradictory facts withhold the comparison instead
of producing a usable zero or ordinary total. Generated source guidance
explains this distinction. These fictional 2025 arrival-year composition
checks do not establish independent part-year conformance, spouse identity,
or all share-loss/foreign-income combinations.

The [share-source conversion regression](../tests/personskat_partyear_share_conversion.test.mjs)
then reproduced another accepted discrepancy: applying the generic day factor
to a fictional 100,000 DKK dividend raised total tax by 5,020.89 DKK. Changed
documented/actual share amounts were accepted too. Each own share source must
now retain its amount in the selected PSL14 basis; a conflicting conversion
withholds comparison instead of rewriting facts. Per-source checks also reject
opposite conversions that cancel in the aggregate. The source-choice metadata
explains the share-income exception, including the actual-income election.
Documented annual low-rate share tax is additionally checked against its
canonical annual result rather than the same reconstruction on both sides.
These are source-backed consistency checks, not independent administrative
part-year conformance or proof of all spouse/foreign share-income treatments.

The [employer-pension/ATP observations](../examples/danish-income-tax/skatdk-arbejdsgiverpension-ekstern.md)
add two 2025 official-form matches (ordinary ATP below the employment-deduction
cap, and lifetime pension plus ATP) and four unresolved employer-rate pension
disagreements. All four differ by 5,520 DKK in extra pension deduction.
Three high-wage profiles differ by 1,297.20 DKK in tax; a new low-wage profile
also differs by 6,150 DKK in employment deduction and 2,742.45 DKK in tax.
A combined private/employer profile reaches the shared 65,500 DKK cap but
the public form's extra deduction reflects only the private contribution.
Two separate over-cap submissions returned validation errors, not tax results;
they show that the server uses the employer-rate field for the shared cap.
The guide preserves their inputs separately, without claiming excess-payment
conformance. Public HTTP form submissions, restored inputs and selected client
submit handlers were inspected; a fresh graphical-browser reproduction and
explanation remain open (`td-0e5d15`). No formula was changed to fit the
observations. Canonical caveats now mention the employment-deduction gap too;
the source demo retains its original rate/ATP disagreement.
A further September 26 HTTP probe preserved the low-wage rate profile while
selecting the two observed open-panel form flags. It retained the same deduction
and tax discrepancy, with the submitted values restored unchanged. This did not
resolve the gap or establish graphical-browser equivalence; it is not another
independent tax profile or a reason to repeat the same probe.
The [offline regression](../tests/tax_employer_pension_conformance.test.mjs)
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
