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
research model has been ruled out. The public `beregn_personskat` entry checks
the main person's and any active spouse's tax year before evaluating tax.
Years outside the current 2023–2026 boundary produce a case diagnostic naming
the field, supplied year, supported years and
[parameter source model](skatteaar-parametre.runa), with no result for that case.
Other cases in the same batch may still succeed. Do not change a correct year
merely to pass the check: that year needs its own source-backed model. This is
a model-coverage limit, not a finding about tax liability.

Other unsupported inputs that fail before a result can be constructed still
produce CLI diagnostics. Native checks now pass for the state-tax source module
and the [focused employment-deduction audit](beskaeftigelsesfradrag.md), but this
does not establish native support for the full Personskat calculation. Remaining
code-generation limitations mean this boundary is exercised through `runa call`.
The year guard uses `assert_with_message` and therefore requires a compiler built
after that builtin was added, not the original 0.2.0 binary.

Low-level year-parameter tables expose `*_opslag` helpers with `Some(...)` or
`None`. Their existing value helpers stop when a required parameter is absent;
they do not invent a zero rate. A legally phased-out rate of zero remains a
known value. A known rate does not establish an indexed threshold for an
uncovered future year. These helpers do not expand the canonical 2023–2026
input boundary, and guarded low-level calculation helpers still require their
stated preconditions.

## Union-fee taxpayer status

This is an individual assessment, not a company tax return. With active
union-fee records, `Ll13JuridiskPerson` fails the control at
`lønmodtager.ligningsfradrag.faglige_kontingenter.skatteyderstatus`, including
under the active spouse prefix. The deduction summary is invalid and the final
comparison amount is withheld; diagnostic component amounts are not usable tax.
An empty fee section stays neutral. The shared LL §13 model still supports
legal persons, and the individual self-employed branch is not removed.

The status describes the taxpayer, not whoever paid the bill. An employer's
payment does not turn an employee into a legal person or establish entitlement
to the uncapped branch. Check the actual taxpayer status and any employment
income treatment; do not switch status just to obtain a matching deduction.
See [DJV C.A.4.3.1.3](https://info.skat.dk/data.aspx?oid=2061770), including the
company-paid membership example, and the source text in
[the LL §13 model](ligningsloven-kontingenter-gaver.runa).

## Foreign-employment allocation

The required `lønmodtager.ligningsfradrag.arbejdsfradrag_udland` input starts
unknown. A documented common condition can rule out the exclusion for all
relevant employment; otherwise [source/period allocation](beskaeftigelsesfradrag.md)
must reconcile to the model's independently derived work-deduction basis.
Unknown facts, mismatched sums and an attempted blanket exclusion fail the
canonical validity check, including for an active spouse.

Only the affected employment income is excluded from the basis for ordinary,
job, senior and single-parent employment deductions. AM, personal income and
the LL §9 L pension basis are unchanged. The internal all-or-nothing Boolean
remains `false` because the source-specific exclusion has already been applied;
it is no longer an assumption that the person's foreign facts are false.
See [LBK 1500/2025, §§9 J–9 L](https://www.lovtidende.dk/api/pdf/250970) and
[L 238, 2017–18, notes to §1 no. 3, pp. 15–16](https://www.ft.dk/ripdf/samling/20171/lovforslag/l238/20171_l238_som_fremsat.pdf).

The component result exposes the before/excluded/after amounts and controls.
Source truth, treaty residence, foreign relief and complete tax coverage remain
separate questions. [External rounding observations](skatdk-fradrag-oere-ekstern.md)
now support the whole-øre projection before upward whole-krone rounding in
selected 2025/2026 cases, with two explicit disagreements and unverified older
years. This is not a claim of complete administrative conformance.
The [part-year entry](personskat-par14.calculate.runa) also preserves and
reconciles the exclusion before recomputing annual deductions; check its own
`input_gyldigt` rather than using a diagnostic scalar total.

## Updating existing clients

The [spouse loss-credit correction](underskud-modtagersats.md) also uses the
recipient's rates, including for conversion back to unused losses. Three
fictional 2025 calculator cases cover different municipalities and church
membership. No new public input fact is required; direct `.runa` constructors
of `LønmodtagerPar13Forhold` must supply the recipient's derived §13 rate.

The [spouse personal-allowance correction](personfradrag-samordning.md) uses
the donor's allowance amounts after own-tax offsets and the recipient's rates,
not the donor's tax-credit amounts. This also corrects ordinary low-income
own-tax offsets. Six fictional 2025 calculator observations exercise the
canonical source-fact boundary. The additional outgoing allowance-basis record
changes the Preview schema fingerprint: regenerate templates even though no
new personal input fact is required. Read the linked rounding and coverage
limits before treating the result as full administrative conformance.

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

Regenerate templates for the required foreign-employment input too. Preserve
unknowns until the facts are available, and update source allocations when a
pension or wage scenario changes the work-deduction basis. Fictional scenario
helpers are not suitable defaults for a real person's treaty-residence facts.

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
