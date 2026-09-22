# Read the calculation status before comparing tax totals

`beregn_personskat` returns `vurdering` alongside its existing breakdown. This
is a model-owned validity assessment, not a compiler guess based on Boolean
field names. The same checks apply to the taxpayer and an active spouse.

If `vurdering.status` is `UgyldigtBeregningsgrundlag`,
`vurdering.slutskat_til_sammenligning_øre` is `null`. Do not present any of the
other numerical fields as a reliable tax result. They remain available only to
diagnose the failed calculation. `vurdering.fejl` gives input paths and reasons;
`vurdering.kontroller` includes the passing checks too.

For example, a structurally valid date object containing all zeroes is not a
valid birthday. It now fails the `lønmodtager.pension.fødselsdato` check even if
the downstream arithmetic happens to produce a plausible tax amount. An invalid
spouse birthday is reported against
`ægtefælle.MedÆgtefælle.fakta.lønmodtager.pension.fødselsdato`.

The birthday is required even without pension contributions: it also determines
the AM contribution rate. From 2026 the youth exemption lasts through the year
of turning 17, while the ordinary rate applies throughout the year of turning
18. The preliminary pension income basis must preserve the same actual date as
the final wage calculation. See the [Danish age-boundary example](pension-og-fradrag.md#fødselsdato-er-ikke-kun-et-pensionsfelt).

`BeregnetMedForbehold` means the listed input checks passed. Its comparison
amount is the existing `slutskat_øre`, unchanged. It does **not** establish that
the underlying documents are true, all facts were supplied, or the complete
relevant law has been encoded. `samlet_modeldækning_bekræftet` remains `false`
and `forbehold` lists the remaining qualifications, including known deduction
coverage gaps. Only compare the modeled portion until coverage is established.

A successful CLI exit means execution and serialization succeeded, not that
the domain assessment is valid. JSON and XLSX expose the same status and nullable
comparison amount. Do not replace an absent comparison amount with zero.

## Which checks are combined

The canonical calculation combines explicit domain summaries for dates/year and
municipality, personal income, commuting and deductions, pension contributions
and payments, capital income, self-employed AM contributions, shares, foreign
social contributions, CFC, spouse business allocation, carried losses, debt
relief, negative share-tax credits, DIS relief, property tax, foreign relief and
settlement credits/selected payment reconciliation. These reuse the component
models' active-branch validation; a false eligibility decision is not itself
invalid input. Inactive spouse and optional payment branches are not required.

These are the enumerated checks, not a claim that every possible defect in the
research model has been ruled out. Unsupported inputs that fail before a result
can be constructed still produce CLI diagnostics; improving the unsupported-year
diagnostic is separate work. Native code-generation limitations in shared tax
modules also remain; this boundary is exercised through `runa call`.

## Updating existing clients

The validity assessment itself is an additive change to the research model's
Preview output. Subsequent deduction work also adds a required
[single-parent benefit input](ligningsloven-par9j-enlig.md): its template default
is unknown, not confirmed nonreceipt. Regenerate templates and migrate supported
facts instead of editing a stale workbook's hidden fingerprint. Existing clients
must check `vurdering` before using the legacy scalar totals for comparison.

The required [service/handyman input](boligjob.md) also starts as unknown.
Active invoice facts and spouse allocations must pass their component and
household consistency checks; a known ineligible expense is distinct from an
incomplete or unsupported calculation.

Pension payout completeness is explicit too. Both Boolean fields under
`lønmodtager.pension.udbetalingsoplysninger` start as `false` (unknown or
incomplete), never as confirmed absence. The current year's facts must be
complete before comparing tax. Prior history only needs further clarification
if it can still change the modeled LL §9 L deduction. A known eligible prior
payout may already establish the condition; otherwise the model evaluates the
same source rule with and without the current payout offset, including caps and
rounding. The output keeps `foregående_oplysninger_komplette` distinct from
`foregående_oplysninger_tilstrækkelige`: irrelevant unknowns remain unknown.
An active spouse and the separate part-year calculation have the same gate.
See the [Danish pension interview and migration guide](pension-og-fradrag.md).

The required `lønmodtager.pension.atp` input starts as `AtpUoplyst`, not
confirmed absence. Employer-reported ATP, public-benefit ATP, SUPP paid to ATP
and mandatory pension savings have different deduction bases. Complete source
rows must preserve the reported gross/net amounts and payment identities;
missing net amounts or duplicate payments with other pension routes withhold
comparison. An active spouse and part-year calculation use the same gate.
Regenerate older templates and review actual ATP facts, even without company
pension. The source rows and derived bases remain visible in `pension.atp_resultat`.

Never fill a missing birthday or other unknown fact with a plausible substitute.
When unavailable spouse facts prevent an independent household calculation, use
the [conditional report reconciliation](aarsopgoerelse-afstemning.md) instead.

The focused canonical regression is:

```sh
cargo test --quiet --test personskat_validity -j 1
```

For model-only iteration, `FUTURUNA_MODEL_TEST_RUNA` may name a verified existing
binary from the same compiler revision. Leave it unset for focused compiler
regressions to exercise Cargo's debug binary. The canonical
[mint gate](../../docs/mint-gate.md) builds the current optimized compiler first
and pins these model tests to that fresh artifact, including in CI; it never
trusts an inherited binary override.
