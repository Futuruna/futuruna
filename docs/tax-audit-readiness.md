# Danish tax-audit readiness

Futuruna supports assisted review of Danish tax reports and source-based
calculations for the canonical 2023–2026 input boundary. Typed calculations are
[Preview](feature-stages.md); the tax models are research software, not individual
advice or certification of arbitrary årsopgørelser.

## Useful entry points now

Use the checked-out [setup guide](../website/public/ai-setup.md#tax-audit-runtime-check)
and the exact compiler that passes its runtime check. Choose the question before
generating a large input contract:

| Question | Start here | Result boundary |
| --- | --- | --- |
| Do my report's figures fit together, without my spouse's documents? | [Conditional report review](../examples/danish-income-tax/aarsopgoerelse-afstemning.md) | Arithmetic and necessary transfers, not verified spouse facts. |
| Does a calculation from my own source facts agree with the report? | [Canonical input and validity](../examples/danish-income-tax/personskat-validity.md) | Supported model calculation with explicit coverage reservations. |
| What changes if I pay more or less into a pension? | [Pension and deductions](../examples/danish-income-tax/pension-og-fradrag.md) | Modeled tax and available-cash differences; distinguish own tax from household effects. |
| Which invoice expenses can I examine? | [Service and handyman deductions](../examples/danish-income-tax/boligjob.md) | Supported invoice conditions and limits, not document authentication. |
| Did Danish tax liability start or end during the year? | [Part-year intake](../examples/danish-income-tax/personskat-delaar.md) | Separate liability-period calculation; months worked do not establish tax-liability dates. |
| What explains a green-check credit? | [Compact review](../examples/danish-income-tax/groen-check.md) or [integrated calculation](../examples/danish-income-tax/personskat-groen-check.md) | Supplied income versus a credit derived from Personskat; do not count the same credit twice. |

Read the relevant source-input guide when a report contains:

- [Fees/B-income](../examples/danish-income-tax/personskat-honorar.md),
  [group-life payments](../examples/danish-income-tax/personskat-gruppeliv.md),
  [rental operating income](../examples/danish-income-tax/personskat-ejendomsdrift.md)
  or [financial posts](../examples/danish-income-tax/personskat-finansielle-poster.md).
- [Interest](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md),
  [gifts](../examples/danish-income-tax/personskat-gaver.md),
  [maintenance](../examples/danish-income-tax/personskat-underholdsbidrag.md)
  or [ferry/flight commuting expenses](../examples/danish-income-tax/personskat-faerge-og-fly.md).
- [Statutory benefits](../examples/danish-income-tax/personskat-dagpenge.md),
  [SU](../examples/danish-income-tax/personskat-su.md),
  [folkepension](../examples/danish-income-tax/personskat-folkepension.md)
  or [public early/disability pensions](../examples/danish-income-tax/personskat-socialpension.md).
- [Pension corrections](../examples/danish-income-tax/personskat-pensionskorrektion.md)
  or [documented employer bank-pension year approval](../examples/danish-income-tax/personskat-bankpension-aar.md).

## From report to calculation

A person or assistant reads and classifies source facts; Futuruna executes the
arithmetic and logic. There is no LLM evaluator or automatic PDF importer.

1. Keep personal documents, inputs and results outside the checkout. Preserve
   each source line's document, year, identifier, original amount, sign and unit.
2. Generate a fresh schema/template from the selected model. Use its questions,
   choices, units and source traces; the
   [input navigator](reference/calculations.md#inspect-one-input-branch) can show
   one branch at a time without loading the whole workbook into an AI prompt.
   Never bypass a fingerprint mismatch by editing the saved hash.
3. Record the source-to-field mapping and its factual justification. The
   [worked intake example](../examples/danish-income-tax/fra-bilag-til-input.md)
   distinguishes wage/pension/ATP, own interest share and duplicate observations.
   A label or positive integer alone does not prove correct classification.
4. Keep unknown facts unknown. Template zeros and empty lists are not evidence
   of absence. If a required amount cannot represent uncertainty, stop the
   independent calculation until it is clarified. Missing spouse documents
   can instead lead to conditional report review.
5. Keep report observations separate from independent inputs. Do not derive
   missing facts from the tax amount being checked or feed conditionally inferred
   spouse transfers back as verified facts.
6. Distinguish tax liability from payment balance. Follow the
   [tax-credit guide](../examples/danish-income-tax/personskat-skattekreditter.md)
   for withholding, assessed amounts and refunds; use the
   [corrected-payment route](../examples/danish-income-tax/aarsopgoerelse-afstemning.md#ændret-rapport-beløb-til-betaling-eller-udbetaling)
   when a revised report asks for a payment or repayment.

## Read validity before amounts

For an independent calculation, read the final result's `vurdering`:

- `UgyldigtBeregningsgrundlag` withholds
  `slutskat_til_sammenligning_øre`. Other amounts are diagnostic, not an
  acceptable alternative comparison.
- `BeregnetMedForbehold` means the enumerated model checks passed, not that
  the documents are authentic, all facts are present or all applicable law is
  covered. Preserve the returned caveats and failed-case diagnostics.
- A conditional report result such as `BetingetAfstemt` is not an independent
  approval of the tax assessment or the inferred facts.

For part-year calculations, use the outer assessment, not the ordinary or
annual intermediate result. Calculation and final-settlement controls are
separate; a valid tax amount does not itself establish a payment instruction.
The [canonical Danish viewer](../examples/danish-income-tax/personskat-validity.md#læs-dit-gemte-resultat-på-dansk)
and [conditional-report viewer](../examples/danish-income-tax/aarsopgoerelse-afstemning.md#læs-resultatet-på-dansk)
have distinct supported result contracts.

## Limits to keep visible

- **Unseen-document intake is not validated.** Three personal examples and
  focused fictional regressions do not establish reliable interpretation of
  arbitrary reports by arbitrary AIs. Consistent, correctly typed inputs can
  still misclassify or omit a real source fact.
- **Employer-rate pension conformance is unresolved.** The
  [recorded 2025 observations](../examples/danish-income-tax/skatdk-arbejdsgiverpension-ekstern.md)
  include extra-pension and employment-deduction disagreements (`td-0e5d15`).
  Do not relabel contributions to force a match or call a personal report wrong
  on that basis.
- **Rounding is not universally matched.** Three observed 2026 one-krone
  deduction differences remain unexplained; earlier-year administrative
  rounding also needs independent evidence (`td-68c9d3`, `td-3f1c08`).
  [Exact profiles and differences](../examples/danish-income-tax/skatdk-fradrag-oere-ekstern.md#kendte-afvigelser--ikke-match)
  remain recorded. Do not hide them with a tolerance or altered source facts.
- **Coverage depends on the selected route.** Check the
  [canonical coverage guide](../examples/danish-income-tax/personskat-validity.md),
  especially unknown/part-year church membership, special DIS positions,
  unsupported corrections and facts outside the chosen income branch.
  Part-year spouse/share/foreign combinations and pension special-plan
  corrections are not established by ordinary annual examples.
- **Some expense and settlement questions remain open.** Honorarium losses
  and special costs need the [fee guide's coverage review](../examples/danish-income-tax/personskat-honorar.md);
  special Boligjob invoice cases need the [invoice guide](../examples/danish-income-tax/boligjob.md).
  The possible negative-net-credit floor and correction histories remain
  separate from sign checks (`td-145088`).
- **The full native Personskat path is incomplete** (`td-124b83`).
  `schema`/`template`/`call` is the supported tested tax-input workflow;
  native checks of smaller components do not validate the full graph.
- **Large contracts still have a cost.** Compact JSON reduces exported
  repetition, not all internal memory or cold-start work. Prefer focused input
  navigation and small question-specific entries; a full workbook can remain
  wide. Use one calculation worker on resource-constrained machines.
- **Source delivery is not public release.** The original v0.2.0 binary
  predates required safety fixes. The selected compiler must pass the runtime
  check, but that alone does not prove tax correctness or complete model
  compatibility.

## Correctness evidence and law navigation

Law text, effective-year rules and typed source links belong with the
[executable corpus](../examples/danish-income-tax/personskat.calculate.runa)
and its imports. The [source registry](../examples/danish-income-tax/source-status.runa)
distinguishes recorded legal status from metadata freshness. Read these sources,
not an old milestone's completion estimate, when investigating a rule.
[`runa meta`](reference/style.md#meta-comments-are-typed-anchors) exposes the
typed sources and their code spans; the generated calculation contract carries
field guidance. Provenance makes an interpretation inspectable, not infallible.

The [recorded interest](../examples/danish-income-tax/skatdk-rentefradrag-ekstern.md),
[private-pension](../examples/danish-income-tax/skatdk-pensionsfradrag-ekstern.md),
[employment-deduction](../examples/danish-income-tax/skatdk-arbejdsfradrag-ekstern.md)
and [rounding](../examples/danish-income-tax/skatdk-fradrag-oere-ekstern.md)
observations preserve fictional inputs, independently observed expectations,
sources and reproduction commands. The
[employer-pension record](../examples/danish-income-tax/skatdk-arbejdsgiverpension-ekstern.md)
keeps matches and disagreements distinct. These are evidence records, not
general certificates for personal returns.

Regression tests retain expected behavior and counterexamples. Follow
[CONTRIBUTING.md](../CONTRIBUTING.md) for proportional gates; a skipped test is
not a pass, and a past compiler gate does not certify a later model revision.
Change history belongs in Git and task records; required migration guidance
belongs in the [compatibility guide](compatibility-guides/0.2.x.md).

## Before broader launch

Use an approved, pinned candidate with aligned binaries, models and setup
guidance, and complete the [release gates](releasing.md). Exercise the actual
report-to-input workflow, not only already-correct calculation inputs: verify
source mappings, follow-up questions, units, signs, missing facts and the final
validity-first output. Keep the current coverage and conformance limitations
visible. Do not describe a source checkout or a passing example as a complete
personal-tax audit certification or a public release.
