# Exploring Law with Futuruna

Futuruna can turn an encoded rule model inside out. Instead of supplying one
profile and asking for its result, declare a finite dependent relation of
coherent profiles and permitted successors, run every case through the same
canonical rules, and ask which transitions and mechanisms satisfy the
question. Profile facts are columns of that source relation: they are varied,
derived from earlier columns, or visibly conditioned by the question. They are
never silently fixed by Explore.

The accepted Experimental first-class direction developed in section 9 is:

```text
finite profile and successor relations + authenticated frontier ->
content-stable cases -> classify -> named views + explicit-target provenance
```

The existing executable calibration uses the same underlying discipline with
ordinary collections:

```text
candidate facts -> canonical rules -> typed results -> filter -> rank -> prove
```

This workbook uses Futuruna's existing lists, `range`, `map`, `flat_map`,
`filter`, `foldl`, invariants, and proofs. The complete executable example is
[personskat-income-cliffs.audit.runa](personskat-income-cliffs.audit.runa). It
is deliberately a handwritten finite audit using ordinary collections, not a
completed run of the Experimental first-class `? explore` surface described in
section 9.

The first relational execution target is separately authored as
[personskat-income-cliffs-200k.explore.runa](personskat-income-cliffs-200k.explore.runa).
Its current first-pass relation has 2,000 integer coordinates, each denoting
100 DKK, for one conditioned Copenhagen profile with no church tax or optional
input deductions. Its concrete edges are therefore `0 -> 100`, `100 -> 200`,
..., `199_900 -> 200_000` DKK. This is an exact coarse endpoint-cliff screen,
not an exact answer about all 200,000 possible 1-DKK raises and not yet a full
mechanism landscape. The present mechanism request replays selected coarse
cliffs only. Interesting selected edges can seed finer subrelations without
forcing the first stream to pay for every krone; admission changes and
mechanism boundaries need their own discovery signals. A finer successor step
is a separately checked relation with a different identity; coarse evidence
can nominate its bounds but does not count as its coverage unless a future
checked bridge proves that transfer.

An earlier version of the same file declared all 200,000 adjacent 1-DKK
transitions. Its durable prefix remains useful performance evidence, but the
new 100-DKK relation has a different checked identity and must use a fresh run
state. The pipeline now has classified slices, sparse selected-case
realization, proof-specialized source results and a query-bound compiled
residual classifier; the coarse audit is the first Personskat target for that
complete path.

The complementary all-admitted mechanism question is authored as
[personskat-mechanism-landscape-200k.explore.runa](personskat-mechanism-landscape-200k.explore.runa).
It imports the completed audit helpers without making the imported cliff query
selectable, repeats the same finite relation and admission contract, and uses
`find all`. Consequently its selected population is exactly its admitted
population, so the existing `mechanisms ... for selected` surface honestly
replays every admitted coarse edge. It has its own question identity and fresh
journal; it is not a reinterpretation of the zero-cliff result.

Read the workbook in two layers. Sections 1-8 build and check the executable
calibration audit. Section 9 extracts the general Explore architecture from
that evidence; start there when evaluating the language design rather than the
hand-picked fixture.

## 1. Formulate the question

Begin with a relationship the model can evaluate, not a broad request for an
interesting answer.

The smallest teaching slice asks a deliberately narrow calibration question:

> Across the encoded 2026 § 9 C phase-out steps for a stated tax profile, does
> increasing gross income by exactly 1 DKK ever reduce after-tax resources?

The first-class Explore question is broader:

> Across a declared finite relation of supported 2026 person profiles and
> income levels, which 1-DKK salary increases reduce after-tax resources, which
> profiles are affected, and which replay-derived mechanisms are shared by
> those otherwise different cases in the checked-in encoded model?

For the fixed-profile calibration, gross income `g`, exact final tax `tax(g)`,
and values measured in øre give the searched condition:

```text
g * 100 - tax(g) > (g + 1) * 100 - tax(g + 1)
```

The profile-aware form is:

```text
modeled_after_tax_resources(profile, g + 1)
    < modeled_after_tax_resources(profile, g)
```

The first streamed coarse endpoint screen intentionally evaluates the relation

```text
g in {0, 100, 200, ..., 199_900}
modeled_after_tax_resources(profile, g + 100)
    < modeled_after_tax_resources(profile, g)
```

This reduces the concrete population from 200,000 to 2,000 transitions. A
selected coarse edge is a useful refinement neighborhood and mechanism
witness. A nonselected coarse edge does not prove that no 1-DKK subedge inside
it is harmful, because several rule changes may cancel over the wider
interval. Likewise, the endpoint-difference mechanism DAG for one 100-DKK edge
may contain several rule changes inside that interval; refinement separates
them when a narrower causal account matters. Exactness is always stated
relative to the declared relation.

There are therefore two different coarse-grid questions:

1. **Endpoint-cliff screen:** select edges whose net modeled resources fall.
   This is what the current executable query closes.
2. **Mechanism landscape:** compare authenticated endpoint traces for every
   admitted coarse edge, group shared differential signatures and refine where
   a signature, admission status or rule-event boundary changes—even when the
   100-DKK net result is harmless.

The first executable landscape uses a separate `find all` query. In that
question `selected = admitted` by definition, which lets the already-closed
selected-target machinery do the right work without calling harmless edges
cliffs. The eventual general combined-query spelling should be `mechanisms ...
for admitted from ...`: that target belongs to the admission identity and
would let one cliff question both select violations and explain its entire
admitted background. Adding it correctly requires admission-scoped target
seals and durable materialization, not merely another parser keyword, so it is
not a prerequisite for the first exact landscape.

Calling the first query “mechanism discovery” without this qualification would
be misleading. With `mechanisms ... for selected`, an exact-empty cliff result
also produces exactly zero requested mechanisms; it does not prove that the
interval contains no legal mechanism change. The efficient next design reuses
each grid endpoint trace across its two neighboring edges and stores shared
trace/DAG nodes content-addressably, rather than independently replaying both
endpoints for every edge.

The first bounded reuse step is now implemented: one warm mechanism runtime
retains exactly one immutable, complete endpoint-trace proposal keyed by the
checked observation and canonical state/context values. On a linear grid, the
previous edge's After proposal is therefore the next edge's Before proposal.
No evaluator state is shared and memory cannot grow with the population. A
later durable endpoint-graph artifact can carry the same reuse across process
resumes and branching profile fibers.

Evaluation reuse alone is not enough. The first replay artifact format embeds
the complete two-endpoint signature definition in every case incidence even
though the catalog interns that definition afterward. Before starting the
2,000-edge landscape, the durable path is being factored so one canonical
signature definition is journaled once and each later incidence carries only
its concrete transition and collision-checkable replay receipt. This changes
storage from roughly one trace payload per edge toward trace payloads per
distinct signature plus small `O(edges)` receipts, while preserving append-only
resume and independent replay validation.

The narrow formulation identifies all of the pieces the executable audit
needs:

- the quantity that varies: gross income
- the profile facts held fixed for that calibration
- the comparison: two adjacent incomes, 1 DKK apart
- the result function: the canonical `beregn_personskat` rule
- the metric: gross income in øre minus exact final tax in øre
- the witness: a pair for which the metric falls

The broader question promotes profile facts from scenery to search dimensions.
Income, municipality, church-tax status, commute facts, family configuration,
income kinds, deductions and pension facts may all affect whether a cliff
exists, how large it is and which rule path explains it. Every relevant fact is
varied, derived, or explicitly conditioned. A fact is omitted only after exact
irrelevance proof or as an explicitly reported model-coverage limit.

Section 9 gives that distinction its general form: the successor relation
declares the intervention, the question classifies its Before/After edge, and
separate endpoint replay explains the encoded rule difference. The calibration
below supplies evidence for that design; it is not the design contract itself.

## 2. Define the metric and make conditioning explicit

For the broad Explore feature, every supported profile fact named by the
question belongs in the finite source relation. Give it a finite domain, derive
it from earlier bindings, or intentionally condition the source on a stated
value. A fact may be omitted only after an exact irrelevance proof or as an
explicitly reported model-coverage limit. Search cost alone is never a reason
to turn a dimension into a hidden constant.

Use four distinct labels when describing a run:

- **dimension**: a value enumerated by the finite source relation;
- **derived fact**: a value determined by earlier source bindings;
- **conditioning fact**: a visible restriction that deliberately asks a
  smaller question; and
- **coverage gap**: a model-supported fact omitted without an irrelevance
  proof, which narrows the claim and must be reported.

A complete fixed profile is therefore a useful calibration case, not Explore's
default source shape.

The concrete replay in section 7 fixes tax year 2026, Copenhagen municipality,
a single adult without church tax, and an ordinary commute of 60 km per workday
for 203 workdays. It supplies no capital income, share income, pension, property
tax, foreign social contributions, carried tax positions, or special tax
arrangements. Gross income is the only changing fact in that teaching slice.

The executable companion is already wider. At its first boundary it covers all
98 municipal parameter rows, both church-tax statuses and two selected commute
distances; it then follows all 50 known steps for two anchor profiles. Its 490
unique transitions are a mixed calibration map, not a one-profile scan and not
the broad first-class profile relation.

Those choices make one replayable calibration fixture; they are not the
first-class discovery target. The broad target is a finite relation of coherent
profiles, with municipality parameters and other dependent facts derived rather
than crossed as unrelated switches. Section 9 defines that relation.

For readability, the following type and sections 4-5 use compact single-profile
pseudocode for the Copenhagen teaching slice. They are not verbatim excerpts of
the wider companion, whose actual transition type carries `profil` and further
branch evidence and whose input builder receives both profile and income.

The metric is deliberately small:

```runa
| personskat_indkomstklint_netto_øre(
    bruttoløn_kroner: Heltal,
    resultat: PersonskatBeregningResultat
) -> bruttoløn_kroner * 100 - resultat.slutskat_øre
```

Use the most exact value exposed by the canonical model. Here that is
`slutskat_øre`, not a rounded whole-krone projection. Name the unit in every
derived field that could otherwise be misunderstood.

The compact teaching slice stores both inputs, both outputs, the changed rule
component, and validity in one typed result:

```runa
# PersonskatIndkomstovergang(
    indkomst_før_kroner: Heltal,
    indkomst_efter_kroner: Heltal,
    lavindkomsttillæg_før_kroner: Heltal,
    lavindkomsttillæg_efter_kroner: Heltal,
    slutskat_før_øre: Heltal,
    slutskat_efter_øre: Heltal,
    netto_før_øre: Heltal,
    netto_efter_øre: Heltal,
    nettotab_øre: Heltal,
    ligningsfradrag_input_gyldige: Boolsk
)
```

A witness should carry enough information to replay and explain it. Keeping
only an income number would hide the tax change and the rule component that
made the case interesting.

## 3. Choose a finite search space

An exhaustive result is always relative to its declared search space. Make
that space finite, purposeful, and inspectable.

The public 2026 guidance describes the low-income commuting supplement as
being reduced between 341,500 and 391,500 DKK. In the current encoded model,
the relevant amount changes at 1,000-DKK boundaries. For its two complete
anchor staircases, the audit therefore constructs the income immediately before
each of the 50 boundaries:

```runa
= personskat_indkomstklint_grænser =
    map(range(1, 51), |n: Heltal| 341499 + n * 1000)
```

`range(1, 51)` contains the integers from 1 through 50 because the upper bound
is excluded. The resulting list begins at 342,499 and ends at 391,499 DKK.
Each candidate is paired with the following krone.

This boundary-aware search evaluates 50 adjacent pairs instead of blindly
evaluating every krone in the interval. It is exhaustive for the identified
boundaries in this encoded rule. It is not a claim about incomes, facts, or
legal mechanisms outside the declared space.

Use the structure of the rule to design other focused spaces:

- For a threshold, evaluate the values immediately before, at, and after it.
- For an enumeration, include every constructor that the question permits.
- For a bounded amount without known boundaries, choose and state the step
  size, then refine around any interesting transition.
- For several genuinely independent dimensions, construct their Cartesian
  product with nested `flat_map`; use dependent relations for correlated facts.

## 4. Evaluate the canonical rules

Map each candidate to the canonical model. Do not reproduce selected formulas
inside the search: the point is to exercise the same rule graph used for an
ordinary calculation.

The compact fixed-profile mapping calculates both sides of every transition:

```runa
= personskat_indkomstovergange = map(
    personskat_indkomstklint_grænser,
    |indkomst_før_kroner: Heltal| {
        = indkomst_efter_kroner = indkomst_før_kroner + 1
        = resultat_før = beregn_personskat(
            personskat_indkomstklint_input(indkomst_før_kroner)
        )
        = resultat_efter = beregn_personskat(
            personskat_indkomstklint_input(indkomst_efter_kroner)
        )
        = netto_før_øre = personskat_indkomstklint_netto_øre(
            indkomst_før_kroner,
            resultat_før
        )
        = netto_efter_øre = personskat_indkomstklint_netto_øre(
            indkomst_efter_kroner,
            resultat_efter
        )

        PersonskatIndkomstovergang(
            indkomst_før_kroner = indkomst_før_kroner,
            indkomst_efter_kroner = indkomst_efter_kroner,
            lavindkomsttillæg_før_kroner =
                resultat_før.ligningsfradrag.befordring.lavindkomsttillæg_kroner,
            lavindkomsttillæg_efter_kroner =
                resultat_efter.ligningsfradrag.befordring.lavindkomsttillæg_kroner,
            slutskat_før_øre = resultat_før.slutskat_øre,
            slutskat_efter_øre = resultat_efter.slutskat_øre,
            netto_før_øre = netto_før_øre,
            netto_efter_øre = netto_efter_øre,
            nettotab_øre = netto_før_øre - netto_efter_øre,
            ligningsfradrag_input_gyldige =
                resultat_før.ligningsfradrag.alle_input_gyldige &&
                resultat_efter.ligningsfradrag.alle_input_gyldige
        )
    }
)
```

In this calibration only, the input builder contains the explicitly conditioned
profile and the mapping varies income. The broad Explore source instead emits
coherent profile rows and income coordinates as finite dimensions. In both
forms, the canonical calculation remains the single source of the tax result.

## 5. Filter witnesses and counterexamples

Retain the validity signal for the inputs the exploration varies. This audit
checks the commuting-deduction inputs on both sides of each transition; the
other neutral inputs remain explicit fixed assumptions. Then express the
question as a predicate over the typed results:

```runa
= personskat_ligningsfradrag_gyldige_indkomstovergange = filter(
    personskat_indkomstovergange,
    |overgang: PersonskatIndkomstovergang|
        overgang.ligningsfradrag_input_gyldige
)

= personskat_indkomstklinter = filter(
    personskat_ligningsfradrag_gyldige_indkomstovergange,
    |overgang: PersonskatIndkomstovergang|
        overgang.netto_efter_øre < overgang.netto_før_øre
)
```

`personskat_indkomstklinter` is now the answer set. Each element is both a
witness to the existential question and a counterexample to the claim that net
resources never fall when gross income rises within this search space.

The same pattern answers opposite kinds of questions:

```runa
= violations = filter(results, |result| not(required_condition(result)))
= satisfying_cases = filter(results, |result| desired_condition(result))
```

An empty `violations` list supports a universal claim over the searched cases.
A non-empty `satisfying_cases` list supports an existence claim. Preserve the
full records so each answer remains replayable. Do not silently remove invalid
or unsupported scenarios before claiming exhaustiveness: prove that every
generated case is valid, or report the excluded cases and narrow the claim.

## 6. Rank minima, maxima, and worst cases

Use `foldl` when the useful answer is an extremum rather than the entire list.
Guard the empty case before seeding the fold with `head`:

```runa
= worst_income_cliff = if length(personskat_indkomstklinter) > 0 {
    Some(foldl(
        tail(personskat_indkomstklinter),
        head(personskat_indkomstklinter),
        |worst: PersonskatIndkomstovergang,
         candidate: PersonskatIndkomstovergang|
            if candidate.nettotab_øre > worst.nettotab_øre {
                candidate
            } else {
                worst
            }
    ))
} else {
    None
}
```

The result is `Some(witness)` when the search finds a case and `None` when it
does not. A zero-witness search remains a valid answer instead of causing
`head` to fail.

Change only the comparison to answer a different ranking question:

- `candidate.nettotab_øre > worst.nettotab_øre` finds the greatest loss.
- `candidate.indkomst_før_kroner < first.indkomst_før_kroner` finds the
  earliest cliff.
- `candidate.slutskat_efter_øre < lowest.slutskat_efter_øre` finds the least
  final tax among admissible cases.
- Compare a named ratio or cross-multiplied numerator to rank effective
  marginal rates without losing precision to early division.

Keep the complete candidate as the accumulator. The winning record then
contains both the optimizing value and the facts needed to understand it.

## 7. Prove the search and interpret the witness

Prove the shape of the search as well as its answer:

```runa
| searched_50_boundaries: personskat_indkomstklint_grænser ->
    length(personskat_indkomstklint_grænser) == 50

| every_commuting_input_is_valid:
    personskat_ligningsfradrag_gyldige_indkomstovergange ->
    length(personskat_ligningsfradrag_gyldige_indkomstovergange) ==
        length(personskat_indkomstovergange)

| found_an_income_cliff: personskat_indkomstklinter ->
    length(personskat_indkomstklinter) > 0

| selected_cliff_has_maximum_loss: worst_income_cliff ->
    match worst_income_cliff {
        | Some(worst) -> all(
            personskat_indkomstklinter,
            |candidate: PersonskatIndkomstovergang|
                candidate.nettotab_øre <= worst.nettotab_øre
        )
        | None -> length(personskat_indkomstklinter) == 0
    }

? searched_50_boundaries
? every_commuting_input_is_valid
? found_an_income_cliff
? selected_cliff_has_maximum_loss
```

These checks make four different claims: the intended candidates were
generated, the targeted commuting inputs were accepted as valid on both sides,
at least one witness satisfies the searched relationship, and the selected
witness has no smaller loss than any other cliff.

The executable audit also locks down a concrete replay. In the current 2026
model and fixed profile, the transition from 342,499 to 342,500 DKK produces:

| Quantity | Before | After | Change |
|---|---:|---:|---:|
| Gross income | 342,499.00 DKK | 342,500.00 DKK | +1.00 DKK |
| Low-income commuting supplement | 14,826 DKK | 14,529 DKK | -297 DKK |
| Exact final tax | 99,967.63 DKK | 100,038.10 DKK | +70.47 DKK |
| After-tax resources | 242,531.37 DKK | 242,461.90 DKK | **-69.47 DKK** |

The witness proves that this adjacent pair is an income cliff in the encoded
model under the fixed facts. It also points directly to the changed commuting
supplement for source and implementation review.

Run the complete executable audit from the repository root:

```bash
./target/release/runa check examples/danish-income-tax/personskat-income-cliffs.audit.runa
./target/release/runa examples/danish-income-tax/personskat-income-cliffs.audit.runa
```

## 8. Extend to multi-dimensional and relational questions

The following adaptation skeleton starts with a dependent relation. Define the
domain values, result type and model-specific functions for the question first.
Each outer value returns a finite list; `flat_map` joins those fibers into one
search space, while the innermost `map` creates canonical case values:

```runa
= search_cases = flat_map(years, |year: Heltal| {
    flat_map(supported_profiles(year), |profile: PersonskatProfile| {
        map(supported_income_states(year, profile), |income_state: IncomeState| {
            SearchCase(
                year = year,
                profile = profile,
                income_state = income_state
            )
        })
    })
})

= expected_search_case_count = foldl(years, 0, |year_count: Heltal, year: Heltal|
    year_count + foldl(
        supported_profiles(year),
        0,
        |profile_count: Heltal, profile: PersonskatProfile|
            profile_count + length(supported_income_states(year, profile))
    )
)

= relational_search_cases = distinct(search_cases)

| producer_paths_have_expected_size: search_cases ->
    length(search_cases) == expected_search_case_count

| producer_paths_are_unique: relational_search_cases ->
    length(relational_search_cases) == expected_search_case_count

? producer_paths_have_expected_size
? producer_paths_are_unique
```

This count is a sum of dependent fiber sizes. Only when year, municipality and
income are genuinely independent does it reduce to the familiar product
`length(years) * length(municipalities) * length(incomes)`. Multiplication is a
special case, not the default mental model for a legal profile. The first proof
counts producer paths; the second additionally proves they are duplicate-free.
If convergence is intentional, `length(relational_search_cases)` is the exact
set count and the larger path count is only provenance diagnostics.

Then apply the same pipeline:

```runa
= results = map(relational_search_cases, evaluate_case)
= valid_results = filter(results, |result| result.valid)
= witnesses = filter(valid_results, |result| searched_relationship(result))
= worst = if length(witnesses) > 0 {
    Some(foldl(tail(witnesses), head(witnesses), keep_worse))
} else {
    None
}
```

If the model can mark cases invalid, retain that status and prove that every
generated case is valid. Otherwise report the count and facts of every
exclusion; a filtered subset is not the complete declared search space.

Good dimensions include tax year, municipality, family status, income type,
deduction eligibility, pension facts, contract clause choices, effective dates,
and exception constructors. Add a dimension when it belongs to the question,
and prove the resulting search-space size so an omitted branch cannot quietly
narrow the answer.

A Cartesian product is correct only for genuinely independent axes. Real legal
profiles are often a finite relation instead: one choice may determine another,
make only some successors legal, or supply a dated parameter record. Prefer a
typed `supported_profiles(year)` or `feasible_successors(before, context)`
relation over a large product followed by silent rejection. Derived facts do
not add cases; excluded combinations retain their classification when they are
part of the declared world; and an unsupported part of the model must remain
visibly outside the claim.

For income cliffs, the useful object is not merely an income list. It is the
relation between a complete supported profile, an income coordinate, and its
1-DKK successor. That relation lets the result answer both “where is there a
cliff?” and “which modeled profile configurations share the replay-derived rule
difference that explains the computed drop?”

For a larger worked product search, see
[personskatteloven-konfiskatorisk.audit.runa](personskatteloven-konfiskatorisk.audit.runa),
which constructs combinations with nested `flat_map`, evaluates each case,
filters findings, and selects extrema with `foldl`.

## 9. North star: one bounded, observable exploration

The executable audit above is a useful baseline, but its scope is handcrafted.
It supplies 50 already-known phase-out boundaries, fixes two commute distances,
and evaluates only selected profile slices. Its concrete witnesses are exact
for that declared audit. They do not show that Futuruna discovered the relevant
income regions, discovered which profile facts matter, or found every other
mechanism in the broader encoded Personskat model.

The first-class target is therefore not “scan income for one fixed person.” It
is:

> Across a declared finite relation of coherent supported profiles and income
> levels, which permitted 1-DKK salary transitions reduce disposable
> resources, which concrete profiles support each transition, and which shared
> rule mechanisms explain the losses in the checked-in encoded model?

An explicitly conditioned fixed profile remains valuable as a calibration
shard. It is not the semantic north star or an implicit default. The full
result should be able to reveal that a profile dimension is decisive,
irrelevant, or relevant only in combination with other facts.

This section records the accepted Experimental language direction. The source
blocks below use the canonical frontend grammar; they are not evidence that the
relational evaluator has executed or closed the declared world. The normative
contract and current implementation checkpoint remain in
[Bounded Rule Exploration with `? explore`](../../docs/rfcs/bounded-rule-exploration.md),
its [implementation workbook](../../docs/rfcs/bounded-rule-exploration-workbook.md),
and [feature stages](../../docs/feature-stages.md).

### One finite successor relation

Explore is best understood as a provenance-aware relational query over typed
state transitions:

```text
finite coordinates
    -> Context + Before -> finite successors(Before, Context)
    -> admissible transitions
    -> matching, violating, or all transitions
    -> grouped, measured, chosen, and projected result views
```

The general transition contract is:

```text
successors(context, before) -> finite set of after states
```

The algebra fixes the semantic order without imposing a batch execution order:

```text
R          = distinct source rows (context, before) produced by FROM
C_R        = { (context, before, after) | (context, before) in R,
                                           after in successors(context, before) }
D_A        = { case in C_R | admission_A(case) }
S_Q        = { case in D_A | selection_Q(case) }
V_case     = a named relational view over S_Q
M(target)  = differential signature incidence for a named case target
V_evidence = a named relational view over the incidence relation produced by M
```

`RelationId`, `AdmissionId`, `QuestionId`, `ViewId` and
`MechanismRequestId` name those layers respectively. Cases must exist
extensionally before a mechanism can claim their support, but the observable
scheduler may enumerate, classify and replay different committed cases in an
interleaved stream. This resolves the apparent “cases or mechanisms first?”
choice: cases come first in semantic dependency; neither requires a global
phase barrier in execution.

The practical rhythm is a feedback loop:

```text
open case/cell frontier
    -> classify one transition or certify one cell
    -> replay one eligible Before/After pair
    -> intern a new signature or append an incidence to a known signature
    -> prioritize unresolved admission/finding/signature boundaries
    -> return to the case frontier
```

The first witness of a new raw signature gives that signature fiber a concrete
case-support lower bound of one. Further cases grow the incidence fiber; a uniform-cell
proof may grow it by the cell's exact weight without minting every member as a
CaseId. The count becomes exact only when every case or weighted cell in the
mechanism target is terminal. Raw-signature novelty can change scheduling priority,
but it cannot manufacture a case, transfer evidence from another `RelationId`,
or change the final answer. This is how the case DAG and mechanism DAG swing
back and forth while remaining independently auditable.

It may return the unchanged state, one derived successor, or several finite
alternatives. Identity, relative and independent behavior can be inferred as
IR/optimizer properties; they are not user-level transition modes. A household
reallocation, for example, can both derive fields from Before and branch over
several alternatives. Successor domains have set
semantics: duplicate generated values for the same canonical source key do not
inflate case counts.

Source deduplication, successor deduplication and canonical value order are part
of the relation contract; discovery or storage order is not. Equal canonical
`(Context, Before)` rows collapse and union their exact producer support. Equal
After values under one source do the same, so one canonical triple has one
`CaseId` within a `RelationId`. There is no opaque source-key escape hatch. If
two choices reaching the same After state are meaningfully different
interventions, their typed action identity belongs in Context; otherwise they
are the same case. Equal extensional transitions from different relations may
still share a global `TransitionId` while retaining relation-scoped CaseIds.
Variable successor cardinality therefore needs a dependent decision structure
with stable per-source support, not an ordinal Cartesian generator.

Likewise, a boundary is normally a finding, not a source-supplied search hint.
The 1-DKK update belongs to the income-cliff question; a suspected threshold
does not. The compiler can recognize an affine successor, source events and
other structure without making `boundaries` the semantic definition of the
query.

One possible relational spelling is:

```runa
? explore personskat_income_cliffs_2026 {
    from {
        profile in coherent_personskat_profiles_2026(
            PersonskatProfileSpace2026(
                municipalities = supported_municipalities_2026(),
                church_tax_statuses = supported_church_tax_statuses_2026(),
                households = supported_household_profiles_2026(),
                commutes = supported_commute_profiles_2026(),
                income_compositions = supported_income_compositions_2026(),
                pensions = supported_pension_profiles_2026()
            )
        )
        coordinate in supported_income_coordinates_2026(
            profile,
            range(0, 1_500_000)
        )
        context = SalaryChange(amount_kroner = 1)
        before = personskat_state_2026(profile, coordinate)
    }

    to after = apply_salary_change(before, context)

    where before personskat_supported(before)
    where after personskat_supported(after)
    where transition salary_change_permitted(before, after, context)

    find violations of modeled_after_tax_resources_never_fall(
        before,
        after,
        context
    )

    results cliffs from selected {
        each case
        measure [
            loss_ore = modeled_after_tax_resources_ore(before) -
                modeled_after_tax_resources_ore(after)
        ]
        select [
            profile = before.profile,
            gross_salary_before_kroner = before.gross_salary_kroner,
            gross_salary_after_kroner = after.gross_salary_kroner,
            loss_ore
        ]
    }

    results case_summary from selected {
        group all
        aggregate [
            cases = count_distinct(case_id),
            affected_profiles = count_distinct(before.profile)
        ]
        select [cases, affected_profiles]
    }

    mechanisms cliff_paths for selected from assess_personskat

    results mechanism_summary from mechanisms cliff_paths {
        group all
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            explained_cases = count_distinct(case_id)
        ]
        select [
            mechanisms,
            raw_signatures,
            execution_profiles,
            explained_cases
        ]
    }

    results mechanism_loss_bins_50_dkk from mechanisms cliff_paths {
        group by [
            bin_start_ore = floor_to_bin(
                modeled_after_tax_resources_ore(before) -
                    modeled_after_tax_resources_ore(after),
                5_000
            )
        ]
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            cases = count_distinct(case_id)
        ]
        select [
            bin_start_ore,
            mechanisms,
            raw_signatures,
            execution_profiles,
            cases
        ]
    }
}
```

The final `results` block is the target structural result spelling downstream
of mechanism replay and quotient assignment. `mechanisms` counts distinct
`structural_mechanism_id` values; `raw_signatures` and `execution_profiles`
retain execution-sensitive populations separately. An authored
mechanism-incidence row joins `case_id`, `transition_id`, raw `signature_id`,
`structural_mechanism_id`, `execution_profile_id`, Context, Before and After.
The row waits for its durable quotient assignment, and exact closure commits
both the raw incidence and structural quotient roots. Consequently,
`count_distinct(signature_id)` remains a raw-signature histogram rather than a
mechanism histogram.

`aggregate count_distinct(...)` is a closed-group reducer; unlike retained
examples, it cannot claim exactness until its raw incidence, structural
assignment and declared result-input frontiers have closed.

`coherent_personskat_profiles_2026(...)` is a dependent relation, not an
instruction to cross those catalogs blindly. It joins and derives them into
whole typed profile rows. The next `in` is genuinely lateral:
`supported_income_coordinates_2026(profile, ...)` may return a different finite
set for each profile because income composition, hours, pension facts or model
support can constrain the meaningful salary coordinates. Each resulting
`before` pairs one coherent profile with one supported lower endpoint. A
materialized list is a sound first implementation when it exposes stable
schema, finite closure, canonical order and lineage. The target relational IR
should retain producer dependencies so the decision structure and relevance
analysis can share evaluation or compress equal behavior without dropping any
declared profile column, SourceKey or support count.

Bindings in `from` are ordered. `name = expression` contributes one value;
`name in finite_expression` performs a dependent finite expansion in the style
of SQL `LATERAL`. Each expression can see only earlier bindings. The block must
ultimately bind exactly one semantic `context` and one semantic `before`;
auxiliary bindings such as `source` remain authenticated construction lineage,
not extra hidden fields in case identity. Several independent `in` bindings do
form a product, but coherent-profile helpers let the author express a join
instead of generating nonsense combinations and filtering them afterward.
The closed IR resolves local binders to ordered indices, so alpha-renaming an
auxiliary spelling such as `profile` does not rename an otherwise identical
`RelationId`.

There is no Personskat-only producer primitive. Any checked pure expression of
an exact-finite collection type can appear after `in`. The closed IR records its
resolved expression, element schema, dependencies on earlier bindings,
set-normalization, canonical enumerator and lineage contract. A list is the
smallest implementation; a range, indexed relation or certified symbolic cell
may implement the same finite interface without changing query semantics.

Names such as `initial_person`, `current_household`, `profile_space` and
`planning_limits` in these sketches denote checked immutable module or scenario
values, not ambient mutable configuration. Their resolved declaration closure
is sealed into producer semantics, and their realized Before or Context value
is content-addressed in the source row. A future external parameter mechanism
must materialize typed values under the same rule; changing an unstated process
variable must never change the explored world behind an unchanged identity.
`profile_space` declares finite catalogs and bounds; it is not a single profile
whose remaining facts Explore silently holds constant.

Nor is the name `coherent_personskat_profiles_2026` itself proof that the query
is broad. The checked exploration bundle needs a source-coverage manifest that
audits the helper's reachable producer closure. For every Context and Before
field and reachable immutable producer input it records one of: varied finite
dimension, derived fact, explicit conditioning, exact irrelevance, or reported
coverage gap. A Copenhagen constant buried in `personskat_state_2026` would
therefore appear as conditioning even though it was not written beside the
`from` clause. The manifest is generated from the ordinary checked program; it
does not require authors to repeat the profile schema in a second Explore-only
language.

This also gives relevance optimization the right contract. If two church-tax
statuses are proved to induce identical behavior for this question and
observer, the evaluator may operate on one shared decision cell, but the cell
retains the exact disjoint support of both profile configurations. Case and
affected-profile counts still cover the declared world. If a rare combination
changes behavior only jointly with commute or household facts, it cannot be
merged away and becomes its own cell. That is the algorithmic route to finding
quirky profiles without blindly replaying every point in the raw product.

`apply_salary_change` is an ordinary checked pure function. It frames every
fact that the salary intervention does not change, replaces the declared gross
salary component, and derives any state fields whose definition depends on that
component. Keeping state construction in an ordinary function avoids giving
Explore a bespoke record-update language and makes the same transition usable
outside a search.

Conditioned facts are optional, visible source restrictions. If Copenhagen and
non-membership of the national church define the calibration world rather than
the validity of an already constructed case, restrict the finite producer in
`from`, for example:

```runa
all_profiles = coherent_personskat_profiles_2026(profile_space)
profile in filter(
    all_profiles,
    |candidate: PersonskatProfile2026|
        candidate.municipality == Municipality.Copenhagen &&
        candidate.church_tax_status == ChurchTaxStatus.NotMember
)
```

That source restriction changes `RelationId` because it declares a smaller
finite world. By contrast, scoped `where before`, `where after` and
`where transition` clauses classify constructed cases as admissible and belong
to `AdmissionId`. Optimizers may push a safe admission predicate into producer
execution, but that physical shortcut must not change either identity or its
population counts. Without explicit source conditioning, the relation ranges
over the whole declared coherent profile relation; there are no hidden “fixed
profile facts.”

Several scoped `where` clauses are a pure conjunction. Source order and repeated
identical conjuncts remain available for diagnostics but normalize away from
`AdmissionId`; scope plus resolved predicate semantics define the admission.

The end-exclusive source range supplies lower salary endpoints through
1,499,999 DKK; the successor reaches 1,500,000 DKK. `to after` constructs that
comparison. Scoped `where` clauses distinguish unsupported or invalid cases
from valid nonmatches. `find` states the question. `results cliffs from
selected` defines a named view of the evidence, and the mechanism root names
the endpoint computation whose Before/After executions are compared. The
downstream loss-bin view waits for that request's incidence relation.

The smallest final-architecture sequence is:

1. Seal the source and successor schemas, program/model identity, normalized
   producer definitions and lineage contract in `RelationId`. Representation
   strategy is absent: an eager list and an incremental producer with the same
   semantics have the same identity. A `RelationFrontierRoot` commits the open
   prefix and exact producer frontier; after closure a separate canonical
   `RelationContentRoot` commits the complete rows. Eager and incremental
   execution must converge to the same completed root.
2. Enumerate source rows and each row's dependent successors in canonical set
   semantics. Derive content-stable `SourceKey`, `SuccessorKey` and `CaseId`
   values from canonical content, never discovery order or a temporary
   ordinal. The stream can pause and publish lower bounds before enumeration
   closes without renaming committed cases.
3. Seal scoped admission in `AdmissionId`, then seal `find` in `QuestionId`.
   Classify each discovered case under those identities independently of
   presentation. Changing only admission or the question does not rename the
   case universe. A complete exploration with zero result views is valid.
4. Materialize zero or more named `ViewId` projections over that classified
   relation without changing its cases or classification evidence.
5. Replay an explicitly named mechanism request against either `selected`
   cases or the `chosen` cases of a named view; the latter target seals that
   `ViewId`. The request name is an address, while its semantic identity comes
   from its question, target, observer and normalization contract.
6. Materialize zero or more mechanism-incidence views whose source seals the
   resolved `MechanismRequestId`. Exact distinct-signature aggregation waits for
   that request frontier to close; it never feeds back into its own target.

This is the canonical dependency order. Future surface extensions must
preserve it rather than reintroducing an implicit execution phase.

### Explore coherent profiles, not a bag of switches

> Fixed is a scope choice; omitted is a proof obligation.

The money-shot is the joint profile search. Candidate profile dimensions for a
2026 Personskat exploration include, where the encoded model supports a finite
domain:

- residence and municipality, including distinct legally relevant residence
  variants rather than only a municipal tax-rate row;
- church-tax status;
- commute facts such as workdays, distance, purpose and special transport;
- age, family and spouse configuration;
- wage, capital, share and other income categories;
- pension contributions and deduction eligibility; and
- carried positions, property facts and special regimes that remain reachable
  from the question and mechanism roots.

These are not automatically independent axes. Municipal parameters derive from
tax year and municipality. Some residence variants imply legal facts. Spouse
and pension choices can constrain one another. The source relation must produce
coherent structural profiles. It must never pair impossible variants and then
call the product a population of people. `AdmissionId` may classify a coherent
constructed profile as unsupported or legally invalid for the requested model;
it is not a cleanup stage for structurally nonsensical source rows.

Prefer domain variants such as `Single | Couple(...)` and
`NoCommute | Commute(...)` over independent flags that can describe impossible
combinations. Derive redundant eligibility and parameter facts; use dependent
finite joins for correlated choices; reserve scoped validity constraints for
the remaining admissibility rules. “All profiles” always means all profiles in
the declared supported relation, not all real people or population prevalence.

Conversely, a dimension should not be fixed merely to save work. It may be
omitted only when the question intentionally conditions on it, the model does
not support it and the limitation is reported, or transitive relevance proves
that it cannot affect source or successor construction, endpoint membership,
admission, selection, grouping, `having`, requested measures or fields, choice,
or mechanisms.

Suppose, hypothetically, Carl's `199,999 -> 200,000` transition and John's
`9,999 -> 10,000` transition both increase gross income but reduce the modeled
after-tax resource metric through the same replay-derived rule change. They
remain different cases and normally different semantic transitions. Profile
equality or difference belongs to case support—for example the incidence from
each `CaseId` to its declared `ProfileKeyId`—not to mechanism identity. Their
shared replay signature is what unifies the mechanism while preserving both
supports. The names and amounts are illustrative configurations, not observed
people or Personskat findings:

```text
Carl configuration --supports--> CaseId(C1) -> TransitionId(T1) --\
                                                                  > MechanismSignatureId(Σ)
John configuration --supports--> CaseId(C2) -> TransitionId(T2) --/
```

`T1` and `T2` stand for extensional typed Context/Before/After identities; a
person label appears in a transition identity only when it is genuinely part of
the modeled state. `C1` and `C2` are content-derived hashes, not encodings of
the display names in this diagram. Case coordinates are never collapsed merely
because their displayed fields look alike.

The mechanism does not create the salary intervention. The transition
generator establishes
`after.gross_salary_kroner > before.gross_salary_kroner`; the cliff predicate
finds `modeled_after_tax_resources_ore(after) <
modeled_after_tax_resources_ore(before)`; replay explains how the encoded rule
execution changed between those endpoints. Sharing signature `Σ` must not
collapse the two cases or invent crossed profiles. Income and loss values
remain observations attached to the supporting cases unless the declared
signature normalization proves that a numeric regime or delta is
mechanism-defining; different numbers alone do not force a split. Conversely,
reaching the same named provision is not enough to merge signatures when other
relevant branches or dependencies differ.

### No probe phase: schedule the open work graph

An earlier design exposed probes as a special initial plan and lifecycle
milestone. That distinction is no longer useful. Once the run has an
authoritative journal and exact open frontier, source-event candidates,
endpoints, midpoints, region proofs and singleton evaluations are simply work
nodes with different priority and dependencies.

The dependency is on content readiness, not producer closure. As soon as one
coherent profile transition is yielded, its immutable CaseId readiness token
can unlock admission, FIND, a case view and mechanism replay even while that
profile's successor enumerator—or the wider profile producer—remains open.
This is how Carl's and John's cases can emerge and converge on a shared
mechanism during one observable stream instead of waiting behind a hidden
enumerate-everything phase.

The implementation scheduler now follows that contract directly. At each
durable base prefix it catches selected-case result evidence, post-mechanism
incidence-result evidence and selected-mechanism target/replay work up to the
currently known discovery ordinals, then grants one more base quantum. The
ordinals are replay-built scheduling indexes only—CaseId remains a content
hash and answer roots remain arrival-order independent. Exact FIND closure is
required only to seal those downstream inputs and publish exact counts; it is
not a gate in front of useful evidence.

Lifecycle and answer closure are related but distinct:

```text
lifecycle:              running <-> paused; {running, paused} -> sealed
answer status:          partial | complete | unknown | unsupported | error
classification frontier: open | closed
mechanism frontier:     not requested | open | closed | unavailable
```

A time limit, resource pressure or user interruption pauses after the latest
committed evidence. Resuming continues from the same frontier. The scheduler
may always choose the most informative ready node, and it may revisit source
analysis as newly closed regions expose new opportunities. There is no authored
`probes` block, no probe-complete semantic state, no `--pause-after probes`, and
no probe plan in query identity.

A requested case answer may become `complete` while separately requested
mechanism replay is still open. `partial` is also the normal status of useful
evidence in a running or paused stream. Sealing says no more work will be
appended; it may preserve a terminal `partial` or `unknown` answer, but neither
status disguises an open frontier as exact closure.

Scheduling policy and each scheduling decision may remain observable
operational provenance. They do not change the declared world or answer. Some
v0 probe-era modules still exist as migration residue on the feature branch.
They are not an executable fallback for the accepted relational syntax: the
command must fail closed until its new work frontier is wired, and the residue
is removed rather than preserved as another lifecycle.

The append-only journal remains authoritative for recovery. Every constructible
case commits its `CaseId`, canonical Context/Before/After transition and
classification atomically, together with only authorized case-local values
evaluated in that transaction. Extrema, representatives, general result views
and mechanism replay have their own closure records. Bounded snapshots are
materialized views, not the source of truth.

Audit-sized closure is also streamed rather than collected into one terminal
blob. Source exhaustion commits fixed-size typed set roots and exact counts for
all prior fiber receipts, source keys and traversal edges. Result publication
appends bounded row/group/chosen-row projection records and then a compact
counts-and-roots seal. A pause between any two records is therefore a valid
resume point, including late in a 200,000-row result.

The blockchain analogy remains useful only in this precise sense: the run
appends content-addressed evidence about a finite world and can resume at its
authenticated head; it is not mining cases or running distributed consensus.

### SQL-like views over graph-backed evidence

Explore should borrow SQL's separation of relational stages without inheriting
SQL's bag semantics, null rules or nondeterministic limits:

| Explore concept | Relational role |
|---|---|
| `from` | finite source relation |
| `to after` | dependent finite successor relation |
| scoped `where` | Before, After and transition admission |
| `find matches`, `find violations`, `find all` | selected transition relation |
| `results NAME from sources` | grouped view over canonical `(context, before)` source rows |
| `results NAME from selected` | named view over selected cases; `from selected` may be omitted |
| `mechanisms NAME ...` | named differential-signature incidence relation |
| `results NAME from mechanisms REQUEST` | named post-replay incidence view |
| `each case` | one logical row per selected `CaseId` |
| `each incidence` | one logical row per mechanism-incidence triple |
| `group by` or `group all` | closed groups over either input relation |
| `measure` | named exact per-input-row scalars |
| `aggregate` | closed-group reducers; currently `count_distinct` |
| `having varies(NAME)` | retain groups whose named measure varies after closure |
| `select` | public projection and privacy allow-list |
| `choose` | explicit one-row, all-ties or frontier cardinality policy |

The closest SQL analogy for `to after in successors(before, context)` is a
`LATERAL` join or `CROSS APPLY`: the finite successor relation is evaluated for
each source row and may return a different number of rows. That is the crucial
generalization beyond a Cartesian list of profile switches. `results` blocks
are named `SELECT`-like views; mechanism replay is the extra provenance layer
ordinary SQL does not provide.

Named views and mechanism requests form a typed dependency DAG after `find`.
An unqualified `results NAME { ... }` is shorthand for `from selected`.
Source-backed views are independent of admission and FIND closure and expose
only canonical `context` and `before`; auxiliary producer bindings remain
lineage, and `after`/`case_id` do not yet exist at that relational stage.
Mechanisms may target selected cases or the chosen rows of an already resolved
case view; a post-replay view may read a mechanism request's incidence rows.
Names resolve to semantic IDs, and the checker rejects dependency cycles such
as a mechanism targeting a view that itself depends on that mechanism. This is
the result-DAG extension needed for structural-mechanism histograms; those
views additionally depend on structural quotient assignment and cannot be
faked by renaming a raw-signature aggregate. Forcing every view to pretend it
reads raw cases would hide a real semantic dependency.

Selected-case inputs use `each case`; mechanism-incidence inputs use `each
incidence`. Source inputs currently require `group all` or `group by [...]`;
selected and incidence inputs may use those grouped forms as well.
Mismatching the row grain and input relation is a type error, and `aggregate`
is available only for a closed group.

Entries in `group by`, `measure` and `select` use `name = expression`; a bare
`name` is shorthand for `name = name`. The implemented closed aggregate form is
`name = count_distinct(expression)`. Group, measure and aggregate declarations
introduce unique intermediate names. Select output names are also unique, while
an earlier intermediate may be projected by using the same bare name in
`select`. Measures are evaluated per input row in declaration order, aggregates
consume closed groups, and later view clauses can refer to earlier names without
making evaluation order or alias resolution implicit.

The superseded `output.key` form hid a semantic `GROUP BY` inside presentation.
The accepted named `results` surface makes grouping, measurement, aggregation
and representative choice explicit. `find all` is equally important: “which
municipality minimizes tax?” is an optimization over admissible alternatives,
not an artificial always-true Boolean witness search.

The exact case relation remains primary. Named views are projections over it;
no mandatory grouping key should force a choice between hiding profile
multiplicity and emitting an unreadable row for every profile field. `each
case` preserves `CaseId` as logical row identity, so two cases remain distinct
even when every selected display value is equal. The analogous raw replay view
uses `each incidence`, preserving the authorized `(CaseId, TransitionId,
SignatureId)` incidence row before any grouping.

Seven layers should remain separate:

- **base relation identity (`RelationId`)**: stable model/type owners, state and
  context schemas, ordered finite source producers, canonical dependent
  successor semantics, endpoint membership and lineage contracts;
- **admission identity (`AdmissionId`)**: one `RelationId` plus scoped Before,
  After and transition validity predicates;
- **question identity (`QuestionId`)**: one `AdmissionId` plus the normalized
  `find` expression and polarity, including the predicate-free `find all`;
- **view identity (`ViewId`)**: one typed input-relation identity—normally a
  `QuestionId` selected relation or `MechanismRequestId` incidence relation—
  plus grouping, measures, aggregates, group filters, selected public fields,
  ordering, choice and privacy policy;
- **mechanism-request identity (`MechanismRequestId`)**: one `QuestionId`, an
  explicit selected or view-chosen target, canonical endpoint observation
  roots and signature normalization; a view-scoped target references its
  `ViewId`;
- **durable-evidence identity**: immutable relation, question, requested view
  DAG and mechanism requests plus evidence-retention authorization, bound to
  evaluator, journal and serialization-schema contracts; and
- **operational records**: each invocation's run-state path, time and resource
  limits and workers, plus scheduler and pause events accumulated in the
  journal across resumes.

`SourceKey`, `SuccessorKey` and `CaseId` derive from `RelationId` and canonical
row content. They therefore survive a new admission predicate, a switch from
`matches` to `violations`, or an additional view. Admission classifications are
keyed by `(AdmissionId, CaseId)` and selection classifications by
`(QuestionId, CaseId)`. This is not only a hash detail: it is what lets one
authenticated transition relation answer another authorized question without
pretending that the underlying cases changed.

Explore query names, `results NAME` block names and mechanism request names are
unique source addresses, not semantic hash inputs. Renaming a view and updating
references preserves its `ViewId`; the grouping, expressions, field names,
choice and privacy schema do not. Likewise a view-scoped mechanism request
seals the resolved `ViewId`, never the raw view spelling.

This separation allows one durable body of evidence to support another
authorized question or result view without pretending that a new predicate or
grouping changed the finite world. A derived view artifact has its own identity
over `(EvidenceRoot, ViewId, report schema)`; it does not mutate the underlying
`RunId`. Retention and privacy may limit which later views can be materialized,
but presentation should not define case or mechanism identity. This is a design
correction now being applied at the checked-artifact boundary. Until those
layers are minted and revalidated separately, execution fails closed rather
than folding presentation back into relation identity.

### Three questions, one algebra

The income-cliff question is a deterministic successor plus a violation search,
as in the sketch above. Two other questions test whether the abstraction is
general.

The municipality question branches from one initial state to every supported
tax-municipality alternative and asks for a global optimum:

```runa
? explore lowest_tax_municipality {
    from {
        before = initial_person
        context in tax_municipality_changes(municipalities_2026)
    }

    to after = apply_tax_municipality_change(before, context)

    where before personskat_supported(before)
    where after personskat_supported(after)
    find all

    results lowest_tax from selected {
        group all
        measure [tax_ore = tax_due(after)]
        having varies(tax_ore)
        select [municipality = after.tax_municipality, tax_ore]
        choose all minimizing tax_ore
    }

    mechanisms municipality_paths for view lowest_tax chosen from assess_personskat
}
```

This is deliberately a tax-jurisdiction substitution: it frames the person's
other facts, changes `tax_municipality`, and recomputes every tax-year and
municipality-derived parameter. It is not a relocation model. A relocation
query needs a different successor relation that regenerates or branches over
commute, property, residence-category, island and other residence-dependent
facts, and projects a key that distinguishes those After states.
`choose all minimizing` returns the complete tied argmin set; choosing one
display representative would be a different, explicitly named policy.
Here `initial_person` is intentional conditioning expressed by the question
“given this person”; it is not an Explore-wide assumption that profiles are
fixed.

If the encoded model has no municipality-dependent result, exact closure proves
zero spread and publishes no “best municipality” recommendation.

The household question uses a finite dependent successor relation rather than
pretending every labor and pension choice is independent:

```runa
? explore household_reallocation {
    from {
        before = current_household
        context = HouseholdPlanningRequest(limits = planning_limits)
    }

    to after in candidate_household_successors(before, context)

    where before household_supported(before)
    where after household_supported(after)
    where transition legally_and_practically_feasible(
        before,
        after,
        context
    )

    find matches of resources_not_below_floor(
        before,
        after,
        context.limits.resource_tolerance
    )

    results tradeoffs from selected {
        group all
        measure [
            disposable_ore = household_disposable(after),
            spouse_hours = after.spouse.hours,
            own_hours = after.self.hours,
            spouse_pension_ore = after.spouse.pension_ore
        ]
        select [request = context, plan = after, disposable_ore]
        choose pareto [
            maximize disposable_ore,
            minimize spouse_hours,
            minimize own_hours,
            maximize spouse_pension_ore
        ]
    }

    mechanisms household_paths for view tradeoffs chosen from assess_household_plan
}
```

This query can expose the trade-off frontier; it cannot infer what the couple
“should” prefer. The one-sided floor permits plans that improve resources while
rejecting plans more than the stated tolerance below Before. This example
deliberately exercises a real successor fiber: the single canonical source may
have zero, one or many finite After rows, and its successor frontier closes
independently. `candidate_household_successors` derives earnings from hours and
wage assumptions, conserves declared household transfers, assigns pension
payer and ownership explicitly, and does not vary correlated amounts
independently. A materially different payment route must be represented in the
typed After plan, or split into typed Context sources when route identity is not
state; it cannot hide in producer provenance and survive as another case.
`current_household` is likewise an explicit “given this household” condition;
another question may enumerate a coherent household-profile relation instead.
Feasibility and whether objectives are lexicographic, weighted or Pareto are
explicit parts of the question. General dependent relations and Pareto result
views are part of the accepted checked surface, but their relational evaluator
and reducers are not wired end to end yet. `choose pareto` is set-valued: it
returns every nondominated selected row. A member observed during an open
search is provisional; only group closure can prove that no unseen case
dominates it.

Every `mechanisms` clause declares a unique request name, a target and an
explicit canonical endpoint observation. The name makes several requests using
the same observer independently addressable without defining semantic
identity. `for selected` means every case selected by `find` before view
processing. `for view NAME chosen` waits for that view's closure, targets its
chosen rows, and seals the referenced `ViewId` into mechanism-request identity.
Presentation fields and measures do not infer the observation root. Here
`assess_household_plan` must expose the modeled resources, hours, pension and
tax dependencies needed by the question and objectives. A future convenience
inference is possible only after its normalization is specified; it is not
implicit in this design.

The named observer resolves to one checked pure callable of shape
`(State, Context) -> Observation`, evaluated independently at Before and After.
Its reachable rule and call closure, not every theoretical value of those
types, is sealed into mechanism-request identity. A static totality proof may
close definedness early, but it is not realistic to require that proof for all
arithmetic in a large legal model. Exact finite target closure supplies the
other sound route: every target endpoint is freshly replayed and receives
either a complete trace or an explicit durable unavailable terminal. Signature
counts are exact only when the target and terminal frontiers close with no
unavailable replay; otherwise they remain unknown or lower bounds. The engine
never silently falls back to tracing selected display fields.

Mechanism replay over `view NAME chosen` explains the chosen transitions; it
does not prove that they minimize a measure or lie on the Pareto frontier. That
proof comes from exact selected-relation closure plus the view's closed
reducers, `having` and `choose` semantics. A request for comparative causal
explanation must instead target all selected alternatives or name a separate
checked group-comparison observer. Structural-mechanism counts and optimum
proofs therefore remain distinct even when they are published together; raw
signature/profile counts are separately named audit measures.

### The result is graph-backed, but not every graph is a DAG

The primary semantic result is a relation between cases, typed transitions,
classification, result views and replay-derived mechanism evidence. Its useful
projections are:

```text
finite generator coordinates
        |
        v
search decision DAG  -------> exact coverage and weighted case counts
        |
        | CaseId-to-TransitionId support
        v
transition graph     -------> shared states and directed semantic edges
        |
        | case-scoped (CaseId, TransitionId, SignatureId) incidence
        v
mechanism DAG        -------> changed rules, dispatch and branches
                       (fresh replay-derived causal structure)
```

The search decision structure is a DAG. Each dynamic mechanism occurrence
structure is a DAG. The role-neutral state-transition graph is not necessarily
acyclic: general queries may contain self-edges or both `A -> B` and `B -> A`.
Call it a transition or case graph unless a monotone rank proves acyclicity.
The layered evidence incidence `CaseId -> TransitionId -> MechanismSignatureId`
is acyclic even when the state graph is not.

Within one `RelationId`, CaseId-to-TransitionId is injective: equal canonical
Context/Before/After triples already collapsed to one case. The same global
`TransitionId` may recur in another RelationId, and many different transitions
may share one mechanism. For a fixed question and observation request, exact
support retains the incidence triple—or equivalent exact fibers—rather than
only a bare transition-to-signature edge. One result group may contain several
mechanisms; one mechanism may span several groups, profiles, incomes, loss
values and disconnected regions. Neither a graph's node count nor a displayed
row count substitutes for a population count.

For the broad income-cliff result, report at least four independent
populations:

- matching profile-by-income transition cases;
- distinct affected profile configurations, because one profile configuration
  may support several cliffs;
- distinct structural mechanisms (`StructuralMechanismId`); and
- complete raw differential signatures plus `ExecutionProfileId` counts,
  explicitly labeled as execution-sensitive audit populations rather than
  mechanism totals.

The distinct-transition count may still be published as a conservation check,
but inside one relation it must equal the case count rather than masquerading as
another population.

In the income view, `before.profile` is a declared canonical product
that excludes `gross_salary_kroner` and computed outputs.
`ProfileProjectionId` seals its resolved schema and ordered profile fields, and
`ProfileKeyId = H(ProfileProjectionId, canonical profile value)` identifies one
equivalence class for that view. This is not a universal identity for a person.
More generally, an “affected profile” count always declares its projection or
equivalence key; it is never inferred from a convenient subset of displayed
fields.

When requested and authorized, the authenticated core retains exact
case/transition/signature incidence in raw or losslessly compressed form.
Profile support, income regions, loss-bin views and replayable examples are
separate named views with their own retention and capacity policies; they need
not retain every raw value for every signature. “Distinct mechanisms” means
distinct request-relative `StructuralMechanismId` values. Raw signature roots
and execution profiles remain available for audit, but neither is renamed a
mechanism count. A mechanism-DAG node count or shared rule name is a third,
different grain. One case may traverse several shared mechanism subjects, so
their supports overlap and are not additive. These configuration counts are
model-space support, not population estimates.

### Mechanism support is a fiber, not a sample count

For one fixed mechanism request, replay induces an explanation map

```text
mu: target CaseId -> MechanismSignatureId
```

and every signature `m` has a support fiber
`S_m = { case | mu(case) = m }`. The first witness proves
`lower_bound(1)`. Ten distinct incidences prove `lower_bound(10)`. The result is
`exact(10)` only when the complete target frontier is terminal and no unresolved
case or weighted cell can add another member. A support-counting cap of 100
therefore means `lower_bound(100), censored`; it does not mean “probably
infinite.”

The progressively refined support object is richer than one scalar:

```text
support(m) = disjoint concrete witnesses
             + disjoint certified uniform cells with exact weights
             + a residual unresolved frontier

count(m)   = unknown | interval(lower, upper) | exact(n)
shape(m)   = a union of typed regions, with per-dimension extent
```

Lower bounds grow as incidences or disjoint uniform cells arrive. Finite upper
bounds shrink as remaining regions are assigned elsewhere or proved empty.
When both meet, the count is exact. Stable CaseIds and disjoint-cell receipts
make this lattice mergeable across resumed or distributed workers without
double counting.

A mechanism may occupy a bounded income interval while being invariant across
several commune or household dimensions; geometrically its support is a union
of cells or cylinders in the declared product space. In a finite Explore query
those regions still have finite weight. Touching the authored boundary means
the support is censored there, not that it is unbounded. A future parameterized
theorem layer may claim an unbounded direction or `infinite_proven` only by
supplying a checked region or injective family of cases. This separates honest
unknown frontiers from genuine mathematical infinity.

The starting context is not incidental metadata. Because a case is
`(Context, Before, After)`, every complete signature has a starter support

```text
starters(m) = projection_(Context, Before)(support(m))
afters(m, source) = { after | (source.context, source.before, after) in support(m) }
```

and the case count is the sum of the successor-fiber weights over distinct
starters. This avoids conflating “how many starting worlds?” with “how many
transitions?”, since one household state can have several explored successor
choices.

Algebraically, this is a fibered relational-support lattice rather than a
scalar property attached to a DAG node. For request `q` and target `t`, let
`R_(q,t)` be the authenticated set relation
`Signature x CaseId x OriginSource x After`. `OriginSource` is explicitly the
relation-scoped canonical `SourceKey` for the exploration's original
`(Context, Before)`, never a node-local evaluator frame. Let `I` be the
structural incidence relation `FacetedSubject x Signature`, where a subject is
a mechanism or an activation/differential-participation view of a node or
edge. Then

```text
subject_cases    K = project_(FacetedSubject, CaseId, OriginSource, After)(I join R_(q,t))
starter_support  P = project_(FacetedSubject, OriginSource)(K)
after_fiber      A(subject, origin) = { after | exists case_id:
                                        (subject, case_id, origin, after) in K }
```

These are set semantics after canonical `CaseId`/`SuccessorKey`
deduplication, so for one subject
`|K_subject| = sum_(origin in P_subject) |A(subject, origin)|`. Case counts may
be added across disjoint signature fibers belonging to that subject. Starter
counts may not: the same `OriginSource` can support several signatures and is
unioned once. Supports of different nodes or edges can overlap as well, so
their counts are never summed into a mechanism total.

When both sides are concrete, an open exploration knows a powerset interval
`K^- subseteq K subseteq K^+`: the inner relation grows as evidence arrives,
the outer relation shrinks as frontiers close, and projection is applied
independently to both relations. An opaque undiscovered-target obligation is
instead the abstract top/unknown upper element, not a case relation that may be
projected. Its starter and successor upper supports remain unknown until a
concrete envelope or target seal replaces it. This is the formal reason that
case bounds cannot be relabelled as starter bounds and that correlations
between starter fields must survive projection.

These starter conditions belong to a request-relative support layer around the
mechanism, not inside the mechanism's structural identity. If Carl and John
have distinct canonical `SourceKey`s and the same normalized complete
signature—not merely a superficially similar rule path—their two starting
states enlarge one signature's support. If the model gives them identical
`(Context, Before)` values, they are one modeled starter; real-world person
identity is not inferred unless the source schema models it. If a profile fact
selects a different exception or rule, the differential trace changes and the
cases separate into different complete signatures. Internal rule nodes and
edges can likewise expose activation-support views, but one case can visit many
such nodes, so those sub-supports overlap and must never be added as case
counts.

So the useful answer to “does every mechanism node have its own starter
subbounds?” is **yes, as a conditioned support overlay, not as part of the
node**:

```text
stable structural subject: StructuralMechanismId | StructuralNodeId | StructuralEdgeId
conditioned overlay:       (request, target, subject, facet) -> P^- subseteq P subseteq P^+
successor fibers:          source -> A^-(source) subseteq A(source) subseteq A^+(source)
```

`P` is the correlated set of starting `(Context, Before)` worlds and `A` keeps
the possible `After` worlds beneath each starter. This is why the result cannot
be summarized safely as only “income 190,000--200,000, any commune”: income,
commune, household and other starter coordinates may constrain one another.
The same structural node may consequently keep the same identity while its
starter overlay grows, narrows or differs between exploration questions.

That node overlay is its **total** support across all enclosing structural
mechanisms and routes. A narrower explanation may condition it on an enclosing
`StructuralMechanismId`, one incident `StructuralEdgeId`, or a canonical path
segment:

```text
cases(node | route)    = { case in cases(node) | its structural assignment satisfies route }
starters(node | route) = distinct projection_(Context, Before)(cases(node | route))
afters(node | route, source) = { after | (source, after) in cases(node | route) }
```

A complete route cover unions to the total node support, but its fibers need
not partition it. One case can contain several qualifying incident edges or
paths, and distinct route fibers can project onto the same starter. Their
counts therefore require set union and deduplication; adding them without a
checked partition proof produces an overlapping route-incidence count.

The three useful scalar grains are consequently separate:

- `distinct_starters`: deduplicated origin `(Context, Before)` rows;
- `cases` (and, inside one RelationId, extensional transitions): distinct
  CaseIds reaching the subject; and
- `subject_incidences`: distinct `(CaseId, structural subject)` memberships.

Summing node or edge case counts computes the last, because one case commonly
reaches many subjects. It does not recover the case population and cannot be
used as a mechanism count.

The region must also say *why* each starter dimension has its extent. A
singleton commune fixed by the query is `conditioned`, not an inferred
mechanism prerequisite. A dimension varied across its declared support is
`explored`; a field determined by other source coordinates is `derived`; one
removed by proof is `irrelevant`; one absent from the model is a `coverage
gap`. Without these labels, a perfectly exact subbound could still invite the
wrong legal or tax interpretation.

More precisely, that node view is a **target-conditioned starter activation
support**, or source preimage. In an income-cliff request it describes the
starters of selected cliff cases whose explanation reaches the node; it does
not claim to characterize every taxpayer state in which the rule could execute.
The same structural node can consequently have different support overlays for
`selected`, `admitted` or another named target without changing identity.
Each overlay also retains the conditional successor fiber
`afters(node, source)`, because one starting world may have several explored
transitions. Every validated node has this support definitionally, while
materializing it can remain optional and searchable on demand.

There are really two useful maps around an internal node. Its **origin
preimage** maps back to the exploration's original `(Context, Before)` and
answers which complete taxpayer situations eventually reached the node. Its
optional **local-entry support** records the checked bindings at the instant
the node was entered and can reveal a nearby guard such as an income threshold.
The first follows immediately from case-to-signature incidence; the second
requires retained, occurrence-indexed frame evidence:

```text
LocalEntry subseteq CaseId x OriginSource x ActivationOccurrence x LocalFrame
```

The activation occurrence prevents repeated visits to a node from collapsing;
retaining `OriginSource` prevents equal local frames from merging distinct
starters. Origin preimage and local entry must stay distinct: neither changes
the node identity, and an observed local-entry range is not automatically the
rule's universal legal trigger condition.

This overlay is defined, and its confirmed starter witnesses can accumulate,
before its target has sealed. At that point undiscovered target cases remain an
opaque obligation, so the internal count state is `unknown(lower)` rather than
an invented finite interval. The current compact publisher emits subject rows
only after target/support closure; a future observer may expose that same open
state under the stable request/target/subject identity. The eventual target
seal narrows that view; it does not mint a replacement identity.

Activation is also weaker than causation. A node may be visited with the same
outcome Before and After, participate differentially by changing presence or
outcome, or be established by stronger future counterfactual evidence as
responsible or sufficient for the selected result. Those are separate support
facets. The first mechanism graph should publish activation and differential
participation honestly; it must not silently label either one “the cause.”
A whole structural mechanism has one facetless support. The activation versus
differential distinction belongs to its internal node and edge support views.

“Subbounds” are therefore best understood as proof-carrying, correlated support
regions. For example, the union

```text
(children = 0, income in 199_900..200_000)
union
(children >= 1, income in 189_900..200_000)
```

cannot safely be replaced by the independent box
`children >= 0, income in 189_900..200_000`: that box invents starter states.
The stream maintains an inner region already witnessed or proved to reach the
mechanism and an outer region which may still reach it. Their difference is the
unresolved starter frontier. Per-field income, commune or household bounds are
useful searchable projections of this region, but they are not the counting
authority unless a checked product/disjointness proof says they are.

The order of derivation is important. First bound the subject's target cases,
then project and deduplicate their starting worlds:

```text
proved subject cases  subseteq  true subject cases  subseteq  possible subject cases
        |                              |                              |
        v                              v                              v
 proved starters       subseteq       true starters       subseteq   possible starters
```

The vertical arrows are distinct `(Context, Before)` projections with an
`After` fiber retained beneath every starter. A hundred possible transitions
may project to one possible starting household, while two case cells with exact
weights may overlap after projection. Consequently, case bounds cannot simply
be relabeled as starter bounds.

A shared structural node has one cross-mechanism support view. When a graph
browser shows that node inside one particular mechanism, it may also expose the
intersection “this node in this mechanism.” That is a derived contextual view,
not a new node identity; in a fixed complete execution graph its starter
support may simply equal the enclosing mechanism's support.

An unavailable replay is reported separately and remains in the shared outer
frontier: inability to obtain its signature is not proof that any particular
node was absent. Pending replay and a successful signature whose structural
quotient is not yet validated remain there for the same reason. This residual
is referenced once for the request rather than copied into every node's stored
incidence.

Facet-aware inverted indexes connect a structural mechanism, node or edge to
only the complete-signature leaves that contain it. Querying all nodes therefore
does not rescan every signature for every node; the work is proportional to the
shared unresolved frontier plus the relevant leaves and the requested output.

The fully deduplicated starter/successor union for a requested node is a bounded,
evictable hot view, not another durable incidence table. Visiting every node in
the graph must not leave `cases × nodes` projections resident in memory. A cold
view is rebuilt from the same authenticated signature leaves. The compact
all-node result does not eagerly rebuild those unions: it publishes a
factorized summary from signature-fiber weights and labels the correlated
starter projection `not_materialized`. That keeps a ubiquitous root node from
allocating memory proportional to every case merely because the DAG summary is
being printed.

The compact row names only an authorization-neutral projection plan.
Publication v9 turns one explicitly selected plan into one typed artifact when
a checked lossless selected-input, each-case view exposes `case_id`, `context`,
`before` and `after` without aggregation, `having` or choice:

```runa
starters selected_cliff_node
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
using values from cliffs
```

For a shared node or edge, one enclosing mechanism may be selected explicitly:

```runa
starters selected_cliff_node_in_path
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
within mechanism "<StructuralMechanismId>"
using values from cliffs
```

The first form is the deduplicated total support. The second intersects the
node/edge signature index with the named mechanism's signature index. It binds
the route into the publication plan and resume cursor without renaming the
structural subject, request, question or analysis DAG.

The route-qualified artifact uses subject-starter record schema v2. An
unqualified consumer omits that optional coordinate and retains its existing
v1 ID, roots, cursor bytes and records, so a qualified consumer can be appended
to a closed publication-v9 output rather than forcing a republish.

The subject may instead be one structural mechanism or an
activation/differential node or edge. It is deliberately singular: v1 has no
wildcard or list which could fan out into a hidden `cases x DAG` export. For a chosen
target, the authorizing view covers the selected population from which the same
`QuestionId`'s chosen subset was derived; the choosing view need not itself
expose all four values. The declaration is an appendable publication consumer,
so it can be added after copying an ID from the completed structural sidecar
without changing or replaying the core exploration journal.

The content-addressed job merge-deduplicates canonical signature fibers with a
key-based `(SourceKey, SuccessorKey)` cursor. Every typed member retains its raw
signature ID, `CaseId`, `SourceKey`, `(Context, Before)`, `SuccessorKey` and
`After`, and the artifact has its own authenticated closure. A candidate page
contains at most 64 members and is shortened adaptively until its encoded
NDJSON record fits publication `max_line_bytes`. If one member alone is too
large, publication fails explicitly rather than omitting or truncating it. The
current per-mechanism k-way merge keeps one candidate per contributing raw
signature plus the current page, so memory is
`O(contributing signatures + 64 members)` rather than proportional to the
final union. It is not yet a fixed-fan-in external merge; that remains a future
scaling option for very high signature fan-in. The compact mechanism result can
close independently of this resumable publication lane. The current compact
catalog implements factorized **total** support summaries for mechanism, node
and edge subjects. Those rows retain `not_materialized` correlated content even
when one typed subject projection is authored: selection schedules a separate
`starters/<name>.ndjson` artifact and must not emit the whole DAG eagerly.
Mechanism-route conditioning reuses that same bounded projection; arbitrary
path predicates and local-entry regions remain separate future work.

The shared frontier is itself factorized into pending cases, unavailable cases,
and complete signature fibers which do not yet have a validated structural
assignment. These components have canonical incremental roots. When an
assignment arrives, one signature descriptor leaves the unresolved manifest;
the stream does not rewrite all of that signature's cases. Exact node case
bounds follow from the disjoint signature weights. Exact node starter regions
remain a separate resumable projection, because starters can overlap across
signatures; until it catches up, the sealed target's distinct starters provide
a safe but deliberately conservative `target_projection_upper`. It is not the
materialized outer node region. If all sealed target starters are already in
the confirmed inner region, the distinct-starter count can nevertheless be
exact while unresolved cases still keep the case count and conditional After
fibers open.

That last state is reported as `exact_starter_set`, not
`exact_correlated_support`: an unavailable case may add another After successor
beneath a starter already counted. The latter label requires the successor
obligation to close as well.

Support-frontier checkpoints are consequently sparse stream landmarks—pause,
explicit checkpoint and report boundaries—not one new semantic record per
case. Each checkpoint names the exact raw target-discovery cursor imported by
that bounded quantum. Replay stops at that cursor even if more raw cases have
since become visible, then verifies the same frontier root. The final support
closure requires the complete cursor and a matching durable checkpoint, so a
200,000-case run cannot defer all support construction to one uninterruptible
last step. Cursor cadence remains operational; only the eventual structural
and support closure roots enter the answer identity.

The relational result reports set populations, not the number of paths through
the producer. A producer-coordinate count may remain useful diagnostics, but
duplicates that converge on the same canonical row contribute provenance, not
extra people or cases. The primary populations are:

- `U_S`: distinct constructible `(Context, Before)` source rows;
- `U_C`: distinct constructible source/successor cases;
- `D_C`: cases admitted by an `AdmissionId`; and
- `S_C`: cases selected by that question's `find`.

For `find all`, the selected relation is the admissible relation, so
`S_C = D_C`. The corresponding transition counts are conservation equalities
`U_T = U_C`, `D_T = D_C` and `S_T = S_C` within one RelationId. If every
relation exposes exact cardinality and order statically, `U_S` and `U_C` may be
exact at open; otherwise their observed values are lower bounds until the
source and every dependent successor frontier close. Content-stable source and
successor keys keep already emitted `CaseId` values unchanged. Admission and
selection counts become exact only after their own required frontiers close.
Raw-signature counts have a separate request-relative incidence frontier;
structural-mechanism and execution-profile counts additionally require the
quotient-assignment frontier. With no confirmed replay evidence yet, each
honest count may be unknown rather than zero.

Intermediate extrema require directional honesty too: an observed maximum is a
lower bound on the final maximum, an observed minimum is an upper bound on the
final minimum, and group winners or Pareto frontiers remain provisional until
their required relation and view frontiers close.

For an arbitrary black-box resource function over `N` adjacent whole-krone
coordinates, exact cliff discovery has an adversarial lower bound of
`Omega(N)` observations: an unevaluated edge may be the only cliff. The
important first optimization is therefore endpoint reuse, not probing folklore.
For `after(n) = before(n + 1)`, the checked adjacent-value memo now reduces the
ordinary scan from four observation calls per edge to exactly `N + 1` distinct
Personskat evaluations in a warm uninterrupted epoch when all edges pass
admission. For the current 2,000-edge grid that predicts 2,001 misses and 5,999
hits; the retired 200,000-edge stress relation predicted 200,001 misses and
599,999 hits. Runtime telemetry must confirm that prediction. A cold resume can repeat its boundary
endpoint, and selected mechanism replay deliberately performs fresh traced
Before/After evaluations. The memo is dependency-certified, bounded to 2,048
entries and 16 MiB, and is acceleration only; it is not evidence or resume
authority.

Mechanism replay has a different sharing problem. Replay ABI v1 copied the
complete checked call path into every control occurrence, so its retained and
canonical cost was proportional to the sum of event depths even when thousands
of events shared one path. Replay ABI v2 introduced a parent-linked activation
trie, but its occurrence-oriented normalization could still erase eventless
calls. Replay ABI v3 commits the complete activation trie once—including
trace-empty and endpoint-only calls—then lets occurrences and dependency edges
refer to bounded local IDs. Prefix-first canonical ranks retain zero-based
invocation positions, so an eventless call cannot shift a later Before/After
pair. Relevance slicing begins only after this exact anchor layer; multiplicity
then belongs to the execution profile rather than the structural mechanism.
The quotient keeps an invocation-erased activation context for every retained
event owner: its parent context plus checked call-site/callee frame. This avoids
merging the same helper event when it occurs beneath two different policy
roles. Purely eventless side branches no longer manufacture policy mechanisms,
but their anchors remain one-for-one members of raw replay evidence and their
exact counts remain in the execution profile.
The safety limits bind actual activation nodes, event nodes, dependency edges
and depth—not repeated presentations of the same prefix. Public projection may
expand a path on demand, but durable identity stays compact. Retired v1/v2
terminals belong to their old question identities and are never resumed as v3
evidence.

Structural quotient admission uses three independent fail-closed lanes rather
than one small byte number for unrelated costs: at most 64 MiB of authenticated
raw-signature source, 1 Gi-unit of deterministic logical derivation work, and
128 MiB of canonical structural artifact output. These work units are a stable
conservative accounting policy, not a claim about sampled resident memory; the
outer Explore supervisor still enforces the process resource ceiling. The
recorded Personskat shape—about 30.1 MiB, 33,198 activation occurrences,
105,718 endpoint occurrences and 108,922 edges—charges 612,550,656 units and
therefore fits with explicit headroom. Live production and journal
rederivation use the same policy constructor, so resume cannot silently apply
a different envelope.

Pure higher-order builtins follow the same rule. An internal `all`, `map`,
`filter`, fold or related callback receives the producer-minted callback site
and exact checked target beneath the builtin activation. Named functions and
rule families must consume that frame; inline lambdas must match their checked
parameter and body digest. Runtime names or closure spellings never invent a
mechanism edge.

Binary search becomes sound only inside a cell whose monotonicity or
piecewise-affine boundaries have been certified. Stronger speedups must come
from such proof cells, relevance quotients, a semantics-equivalent compiled
evaluator, or deterministic parallel evaluation; none may silently skip an
uncertified coordinate.

A 50-DKK mechanism-bin view counts distinct `StructuralMechanismId` values
having support in each loss interval, not the number of cases or raw complete
signatures in that interval. Complete signatures remain the disjoint support
leaves and may be exposed as a separate execution-sensitive count. The same
structural mechanism may occur in several bins, so bin counts need not sum to
the global mechanism count. A bin is exact only when selected-case membership,
complete-signature incidence, structural quotient assignment, loss measurement
and bin membership have all closed. A cap can justify “at least N”; it never
proves infinity. Every support inside a finite Explore world is finite.

Support may be `lower_bound(n)` while its incidence frontier is open,
`at_least(c)` when an actual support-counting cap saturates, or `exact(n)` when
counting closes. A cap on retained examples alone does not degrade an exact
scalar support count: the report may retain only `c` examples while still
counting every incidence. Report the number of counting-capped signatures
separately. Thus the useful summary is “X signatures, Y selected cases, Z
signatures with censored support,” not “Z infinite mechanisms.” A cap must not
stop signature assignment for later cases; if it does, signature count and
incidence remain open rather than merely censored.

### What a finished answer publishes

A paused or sealed exploration bundle should make the result speak at three
resolutions:

- a closure-aware summary of declared, constructible, admissible and selected
  source rows, cases and transitions, affected `ProfileKeyId` values,
  structural mechanisms, raw signatures, execution profiles, and
  saturated-support signatures;
- named relational views such as `cliffs`, `affected_profiles` and
  `mechanism_loss_bins_50_dkk`, each with its own exactness, schema, grouping
  key and privacy authorization; and
- the authenticated journal/evidence root plus typed snapshot and JSON export
  handles needed to resume a paused run or audit and analyze either form.

An eventual report can therefore honestly say “Y concrete profile-transition
cases across P profiles share X structural mechanisms, represented by R raw
signatures and E execution profiles; Z raw-signature supports are at least c
because their support-counting cap was reached,” followed by an exact or
lower-bound 50-DKK **structural-mechanism** histogram and links to the
corresponding case and incidence views. JSON is a reproducible materialization
of named evidence, not the authority from which the graphs are reconstructed.

### Experimental implementation boundary

Keep accepted architecture separate from executable evidence. The feature
branch now connects the canonical frontend, relational lowering, content-stable
identities, durable append-only journal, pause/resume scheduler, named selected
views, fresh endpoint mechanism replay, incidence views and crash-safe NDJSON
publication through the public `runa explore` command.

The smallest nonempty oracle has closed through that complete path:
`relational-explore-stream-smoke.runa` produced exactly four sources and cases,
two selected transitions (`1 -> 2` and `3 -> 4`), one shared raw mechanism
signature with both case incidences, and one structural mechanism/execution
profile. Its target-conditioned support closed at two cases and two distinct
starters. All eight publication-v7 artifacts caught up to journal sequence 107
and head
`fb37a53cac23fd1c4cee5da2508824f694ca11091d7696e60acf4fafbcba3d46`;
an identical reopen appended zero semantic events and zero publication lines.
This is executable evidence for the resumable stream and case/mechanism graph,
not Personskat evidence.

The conditioned
`personskat_income_cliffs_conditioned_100_dkk_grid_200k_2026` query is now a
checked public query over 2,000 coarse income coordinates and 21 exposed
configuration fields. Its source views publish the conditioned profile even
when `selected` is exactly empty; its mechanism observer maps coordinates back
to concrete kroner; and preparation retains the compiler-proven transitive
declaration slice needed by this query.

The first complete Personskat stream closed on 2026-08-31. Its authenticated
result is exact for the declared relation:

- sources, cases, admitted and FIND-classified transitions: **2,000 exact**;
- rejected transitions: **0 exact**;
- selected harmful 100-DKK endpoint transitions: **0 exact**;
- requested differential mechanisms and incidences: **0 exact**, because the
  request targets selected transitions;
- analysis lifecycle: **complete**, with all seven public artifacts caught up
  to journal sequence 130 and head
  `81eaaa3bbd0089501dc9a2af7574762a7df8d0b0474673ddb223073265a6cf32`.

The local external evidence bundle is named
`personskat-100dkk-grid-200k-20260831-v1.state`; its published views and
manifest use the sibling `.output` name in the operator-selected run directory.
The checked program identity is
`eead131eb615a7ad65f45a82b3334a98be989e6df327f0b20109dffe97309b1f`.
These external artifacts are execution evidence, not tracked source fixtures.

The next selected-only audit query is now authored and checked at
`personskat-income-cliffs-350k-commuter.explore.runa`. Its evidence-informed
350,000-DKK horizon is a round stopping point, not a candidate threshold: the
relation generates every 100-DKK edge in the complete zero-to-350,000 prefix
without naming or arithmetically deriving any expected boundary. It crosses
that prefix with a deliberately small 50/100/150-km commuter lattice, giving
10,500 declared transitions. Those distances are experiment coordinates, not
the old 60/130-km witnesses and not a claim of complete distance coverage.

The query also applies the comparative-state architecture directly. `Before`
contains the full typed starting profile and gross salary; `Context` contains
only the 100-DKK promotion intervention; `After` preserves the profile and
applies that intervention. Birth date is explicit rather than hidden in a
no-pension helper. A per-signature result view counts distinct
`(Context, Before)` starters separately from cases, while the structural
sidecar supplies the deeper factorized mechanism/node/edge starter subbounds.
Copenhagen, tax year, church status and the remaining no/standard facts are
visibly `conditioned`; commute distance and salary are `explored`. Targeted
format and static checking pass.

Two deliberately short invocations have now exercised this question without
producing a tax result. The first stopped before case evaluation because
journal restore reconstructed only direct finite dependencies and lost the
distance dimension behind the derived profile/Before bindings. Mint and restore
now use the same transitive dimension-lineage derivation. The resumed invocation
passed that boundary and then stopped atomically when the single FIND expression
exceeded its one-million-step exact-evaluation allowance. No partially
classified case was accepted. Relational expressions now retry only a typed
`ExpressionSteps` exhaustion through bounded `1M -> 2M -> 4M` allowances and
retain four million as the hard per-expression ceiling.

That second stop exposed a performance distinction before a longer run could
wastefully begin. Dependency lineage alone cannot prove that this `3 x 3,500`
auxiliary-factor product maps injectively into its derived `(Context, Before)`
image, and the original compact partitioner understood only one varying
factor. The compiled V2 first accelerated only already checked concrete source
leaves. It reads their authenticated finite integer binding values,
reconstructs the authored profile, Before, Context and After inside a
query-bound executable, and returns ordered WHERE/FIND proposals; the
coordinator remains the only case/evidence producer and checks the first native
unit against the interpreter.

The compact multidimensional path is now implemented rather than inferred from
lineage. The checked producer may issue a fail-closed separating-projection
certificate only when every independent integer factor is recoverable through
a distinct exact Context/Before constructor-field path by a nonzero affine map
whose complete domain is free of `i64` overflow. The source and case proof
artifacts bind that compiler certificate and its minted injectivity evidence.
An exact product is partitioned into mixed-radix `ProductRank` intervals of at
most 256 members, with the last authored finite factor varying fastest. V1
single-factor identities remain unchanged. Unsupported expressions, dependent
factors or missing separation continue through ordinary concrete execution;
the proof gate is never relaxed. One narrow release-mode `runa` compile and an
independent static proof-boundary review passed. That review found and repaired
one missing exact comparison between the rederived checked projection
certificate and the retained query artifact before any execution evidence was
accepted.

The earlier concrete-leaf V2 continuation produced a real open prefix for the
`3 x 3,500` query. After the one-time sidecar compilation, a cached two-minute
epoch advanced the durable classification from 82 to **3,006** admitted,
FIND-classified, not-selected transitions, with zero rejected and zero selected
so far. The useful semantic interval was about 76 seconds, or roughly 38
concrete transitions per second. This is an open prefix, not an empty-cliff
result and not yet evidence about the expected higher-income neighborhood.

The same experiment exposed the next stream-level bottleneck before a longer
run was attempted. That prefix occupies about 58 MiB in 21 immutable segments
and contains roughly 54,000 semantic events. A following 75-second cold resume
made no durable semantic progress because reopening reconstructs the folded
state from the genesis prefix. A general append-only evidence chain still
needs an authenticated state checkpoint: it binds an exact journal sequence
and head plus the complete canonical fold state, and cold recovery loads it
before verifying only the later immutable suffix. The compact product path now
offers the narrower solution for this exact query: a fresh run can classify 42
rank chunks instead of rebuilding roughly 18 ordinary events per case. The old
ordinary-case journal remains valid evidence and is not silently converted to
the new plan.

A fresh compact run now supplies execution evidence for that path. The checked
source-image and case proofs both certified exactly **10,500** logical
transitions and the planner divided them into **42** `ProductRank` chunks. In a
three-minute epoch (about 44 seconds of preparation and 136 seconds of semantic
budget), five complete chunks closed: **1,280** admitted, FIND-classified,
not-selected cases, with zero rejected and zero selected cases in that prefix.
The remaining 37 chunks and all downstream target/mechanism conclusions stay
open, so this is not evidence that the query contains no cliff. The compact
journal reached only 47 semantic events in four immutable segments, and the
published state plus graph was about 48 KiB rather than one record chain per
logical case. Its public case-support graph contains the exact root plus five
chunk/region pairs.

The first bounded compact attempt also found a stream/publication edge case:
an epoch could end before the initial flat source projection existed, and the
publisher misreported that ordinary `not ready` state as a pending-cursor
mismatch. Publication now leaves the cursor untouched and returns `NotReady`
until the first source record is available; a same-root retry then proceeds
from source ordinal zero. This is a lifecycle correction, not a relaxation of
the evidence or cursor checks.

Subsequent bounded epochs have now completed that same compact journal. The
first exact multidimensional Personskat result for this feature is:

- sources, cases, admitted and FIND-classified transitions: **10,500 exact**;
- rejected transitions: **0 exact**;
- selected harmful 100-DKK endpoint transitions: **16 exact**;
- admitted but not selected transitions: **10,484 exact**;
- declared starter lattice: **3 exact profile groups**, each with **3,500**
  income coordinates;
- raw differential signatures, structural mechanisms and execution profiles:
  **2 exact** of each;
- successful mechanism incidences: **16 exact**; replay-unavailable cases:
  **0 exact**; and
- analysis lifecycle: **complete**, with all 11 artifacts in its saved
  publication-v7 bundle caught up at journal sequence 5,249 and head
  `df347602b5e8f1c3b847ea762ec852387dd67d155b14d184f402b5d3ef17e304`.

The selected cases form two correlated starter fibers. At 100 km and again at
150 km, the lower salaries are
`342,400, 343,400, ..., 349,400 DKK`; every successor adds the declared 100 DKK.
The modeled disposable-income losses range from **81.61 to 81.85 DKK**. No
50-km starter is selected. All 16 cases therefore fall in the same declared
50-DKK loss bin beginning at 50 DKK, whose exact row counts two structural
mechanisms, 16 cases and two affected starting profiles. The raw-signature and
execution-profile counts are also two in this run, but remain separate grains.
These are results of the checked-in
research model over this declared coarse lattice, not an individual tax
determination or a claim about unsearched 1-DKK edges and profiles.

This result makes the mechanism/starter distinction concrete. Raw signature
`3c853240...` maps to structural mechanism `49212c80...` and has exactly eight
cases and eight distinct `(Context, Before)` starters, all at 100 km. Raw
signature `e5757173...` maps to structural mechanism `1df2079a...` and has the
same exact counts at 150 km. For both mechanism subjects the target is closed,
the shared residual is empty and the authenticated inner and outer
`SourceKey -> SuccessorKey set` expressions are equal. Thus each mechanism's
starter subbound is an **exact request-conditioned correlated support**, not a
range inferred from its case count and not part of the mechanism's stable
identity.

The same separation applies below a mechanism. Structural mechanism, node and
edge IDs stay stable; request, target and support facet select a correlated
`(Context, Before) -> Set<After>` overlay, and an enclosing-mechanism route may
refine that overlay further. Every grain keeps case and starter bounds separate
and preserves successors as conditional fibers beneath each starter. A shared
node's total support is the deduplicated union of its
route-conditioned supports, while `node | mechanism` is their route-aware
intersection inside the same target incidence. Because two mechanisms can share
starters or successors, marginal counts cannot be summed or multiplied into a
case population without a disjointness proof.

The public values needed to read those two fibers were already authorized by
the checked `cliffs` selected-case view. In the historical publication-v8
experiment, that authorization automatically scheduled a separate
`mechanisms/cliff_paths.starters.ndjson` artifact and deterministically joined
the mechanism support keys back to typed relation values. Each whole-mechanism
group retains raw signature ID, `CaseId`, `SourceKey`, typed
`(Context, Before)`, `SuccessorKey` and typed `After`; its authenticated closure
independently certifies case and deduplicated-starter counts. The compact
structural row can still label its *inline* projection `not_materialized`
because it references this separately authorized artifact rather than
duplicating its values.

That v8 projection has now been materialized from the already closed journal,
without re-executing a single semantic case: the resumed invocation appended
**0 semantic events** at the same sequence 5,249 and journal head. The fresh
publication closed all **12** artifacts. Its starter artifact is an
eight-record, 43,319-byte NDJSON stream: one request header; for each of the two
structural mechanisms, one header, one eight-member page and one exact closure;
then one request closure. The request closure certifies **2 exact structural
mechanisms**, **16 exact cases** and a sum of **16 exact distinct
mechanism-local starters** under artifact closure root
`cc38244ed2d14a52ba340e9027e9132e4240cd2e7f120b9b3823520ffddbee91`.
The two mechanism closures independently certify eight cases and eight
starters, and every typed member recovers the expected 100-DKK promotion,
100-km or 150-km commute, salary endpoints and stable case/signature identity.
That request-level starter scalar is explicitly the sum of two mechanism-local
counts, not a generally valid global distinct-starter count: different
structural mechanisms may have overlapping starter projections.

The compact node rows expose why node starter subbounds are a distinct
operation. The published structural catalog contains **8,053 nodes** and
**20,720 edges**, with activation and differential-participation support views
for a total of **57,548** request-target-conditioned subject rows. For example,
a shared activation node supported by both signatures has **16 exact cases**
but only the honest starter interval **8..16** in its factorized row. The lower
bound is the largest contributing signature's exact starter set; the upper
bound is the sealed target's 16 starters. Materializing and deduplicating that
node's correlated cross-signature union would decide the exact value. The
publisher therefore does not relabel the exact case count as an exact starter
count or manufacture a Cartesian profile box.

Publication v9 closes that product boundary with the explicit single-subject
`starters` declaration above. It can materialize the shared node's exact typed
fiber on demand, while the other 8,052 nodes and 20,720 edges stay factorized.
Publication must never eagerly serialize all of those case fibers merely
because the structural DAG is published.

The existing `graphs/case-support.ndjson` remains a different useful object: it
is the classification/support partition proving how searched regions became
excluded, admitted, matched and selected. It is not the semantic state graph
`Before -> Transition -> After`. That semantic graph and the mechanism starter
fibers can share identifiers and typed case evidence, but neither should be
misnamed as the other.

Publication now gives that semantic graph its own bounded lane at
`graphs/case-transitions.ndjson`. A checked lossless selected-case view is the
value authority. Each selected row retains the actual Context, Before and
After alongside CaseId, SourceKey, SuccessorKey, role-neutral endpoint
StateIds, directional TransitionId and the checked schema identities. The
closure counts cases, distinct state nodes and distinct transitions and binds
the canonical selected-case set rather than treating journal discovery order
as graph identity. Mechanism incidences join through CaseId/TransitionId;
mechanism/node/edge starter fibers join through SourceKey/SuccessorKey. This is
the concrete bridge between the case graph and mechanism DAG, without a
`cases x mechanism subjects` expansion.

The historical closed 10,500-case commuter journal would have been the natural
first attachment audit, but it was minted under an earlier journal contract
and the current reader correctly rejects its prior-head identity. There is
therefore no authenticated publication-v9 case-transition artifact for those
16 historical selected cases. The design did not convert that journal or
manufacture a new 341xxx/342xxx input fixture to obtain a convenient answer.

A fresh current-contract audit has now closed the same authored 10,500-edge
relation from a new journal. Its lifecycle, relation, FIND frontier and analysis
frontier are all exact:

- sources, cases, admitted and FIND-classified transitions: **10,500 exact**;
- rejected transitions: **0 exact**; not selected: **10,484 exact**;
- harmful selected transitions: **16 exact**;
- raw signatures, structural mechanisms and execution profiles: **2 exact**
  of each; and
- successful mechanism incidences: **16 exact**, with **0 replay-unavailable**
  cases.

This independently reproduces the substantive coarse-lattice result without
treating the old bundle as authority. The selected starters are eight salaries
at 100 km and the same eight at 150 km: `342,400, 343,400, ..., 349,400 DKK`,
each followed by the declared 100-DKK promotion. No 50-km starter is selected.
The modeled loss is **8,161--8,185 øre**. The single 50-DKK loss bin beginning
at 5,000 øre contains **2 exact structural mechanisms**, 2 raw signatures, 16
cases and two affected starting profiles. These are exact only for the declared
50/100/150-km by 100-DKK lattice and its conditioned profile facts; they do not
rule out a narrower endpoint cliff or generalize to unsearched profiles.

The publication-v9 semantic case graph now exists at
`graphs/case-transitions.ndjson`. Its 18 records are one header, 16 selected
edges and one exact closure. It authenticates **16 distinct CaseIds**, **16
distinct directional TransitionIds** and **32 distinct role-neutral StateIds**
under content root
`502a33ce4473e11c90260bfe485d8270b49c80cd8c7d47d45cd4ba803bae1c7f`.
All selected-view CaseIds equal the graph CaseId set and the mechanism-incidence
CaseId set; graph and mechanism TransitionId sets also agree exactly. Thus the
case graph and mechanism DAG are joined by authenticated identities rather than
by matching displayed salaries after the fact.

The two current structural mechanisms have different request-relative IDs from
the historical experiment. Mechanism
`209897c89a8a3393f66e0ff631314a50eeb787dec688e9ee618944cc862a9d9d`
has exactly eight cases and eight distinct starters, all at 100 km. Mechanism
`e993d97f96fb14ed7717649a633a98c727f7e828f01f38491a4e34d0b6f110af`
has exactly eight cases and eight distinct starters, all at 150 km. Their shared
support residual is empty. Source coverage labels commute distance and salary
as varied finite dimensions, the composed profile/Before fields as derived
from those dimensions, and Copenhagen, year, church status, promotion amount
and the remaining fixed facts as conditioned restrictions. This is the
executable form of the rule above: a mechanism owns a request-conditioned
starter support overlay without absorbing those starter values into its
structural identity.

After the first exact closure, two trailing single-subject `starters`
consumers were added for those discovered IDs. Resuming the same journal
performed **0 semantic events** and appended only six publication records:
header, one bounded eight-member page and exact closure for each mechanism.
The closures independently certify eight cases and eight deduplicated
`(Context, Before)` starters while retaining each typed `After` successor.
The journal remained at sequence **5,236** and head
`4906f0f56f10091984c7766beca8027c4ec68bcc38175ff3e14dcb078f8814b2`.
One further identical resume appended **0 semantic events and 0 publication
records**, proving both computation and publication are resumably caught up.

The fresh run stayed inside its automatic one-worker, 80%-CPU and 6-GiB outer
memory envelope and reached exact closure within one 20-minute invocation.
The durable artifact timestamps span about **3 minutes 51 seconds**; that is an
artifact-production window, not a claimed end-to-end wall-clock benchmark.
The closed journal occupies about 154 MiB and its published bundle about 524
MiB. Because the graph and starter lanes contain typed state, context and
successor values, the external state/output trees are treated as confidential
and restricted to owner-only access. They remain execution evidence outside
the checkout, not tracked fixtures.

The final source-result closure did not replay the 10,500 tax transitions. A
checked ProductRank grouped-distinct theorem proved the lattice as three exact
groups of 3,500 members and evaluated one representative per group only for
their public keys. Compact proof artifacts are capped at 256 groups. On every
cold invocation they are rebound to the current compiler theorem and their
bounded representatives before a replayed terminal analysis can be trusted;
otherwise the stream fails closed rather than accepting self-hashed group
geometry from an old journal.

The mechanism-support implementation has the matching authenticated
origin-preimage foundation today: request/target/subject/facet identity,
`SourceKey` starter sets, conditional `SuccessorKey` fibers, lazy signature
unions and honest unknown/interval/exact counts. Compact subject rows publish
authenticated inner/outer fiber-expression identities for the correlated
`SourceKey -> SuccessorKey set` contract while explicitly marking their inline
typed content `not_materialized`. Publication v9 can turn one closed exact
mechanism, node-facet or edge-facet expression into the separate authorized
typed starter artifact described above without inventing a Cartesian product.
For node and edge subjects it may bind one enclosing mechanism and derive the
route-conditioned intersection from the existing signature indexes.
The compact row remains `not_materialized` because the selected artifact is a
separate appendable consumer, not inline content.

Publication v9 now implements the one-enclosing-mechanism selector. A focused
two-mechanism/shared-node fixture proves that total-node support contains two
cases but one deduplicated starter, while each `node | mechanism` slice contains
one case and the same one starter. Their plan and fiber identities are distinct,
and paging retains the two different successors beneath that shared Source. A
separate CLI attachment fixture closes a small journal first, adds a qualified
node consumer afterward, and observes zero new semantic batches/events,
unchanged relation/question/analysis/journal identities, a route-bound v2
starter artifact, and a byte-identical no-op resume. This is implementation
evidence for the projection architecture, not a new Personskat execution.

What remains is arbitrary path-conditioned selection and a human-readable
region compression which labels dimensions as conditioned, explored, derived,
proved irrelevant or coverage gaps. Compact unselected rows correctly remain
`not_materialized` rather than implying that opaque roots or per-field
marginals are readable correlated subbounds.

The starting context is part of every case, not incidental metadata. Write a
starter as `Source = (Context, Before)` and a case as one supported
`Source -> After` transition. The starter support of a mechanism (or one of its
nodes/edges) is the inverse image of its supported cases back onto `Source`.
This is why one starter can contribute several cases when its After fiber has
several successors, and why case count and distinct-starter count are separate
grains. When a node is viewed inside one enclosing mechanism, its starter
support is a subrelation of that mechanism's support; a shared node's total
support may instead union overlapping routes from several mechanisms.

These starter subbounds are request-conditioned overlays, not fields of the
stable structural node. Their authoritative shape is the correlated relation
`(Context, Before) -> Set<After>`, with confirmed inner support, an outer
envelope or open obligation, and independently closed case/starter/successor
counts. Income ranges, commune lists and other per-field bounds are useful
projections for browsing, but cannot replace the relation: multiplying them
would invent starting profiles that were never observed or proved reachable.

The compact public answer now exposes this distinction directly. Relational
stream JSON v5 names the closure-aware selected before-to-after case count and,
for every mechanism request, the structural-mechanism, successful replay and
unavailable-replay counts plus the exact sealed target's distinct starter
count and evidence roots. Human output leads with the same answer before
operational checkpoint telemetry. Small exact grouped results are rendered
inline from their authenticated projection journals, while the operational
publication index names every complete or still-catching-up NDJSON artifact
without loading bulk configurations into the answer. The manifest remains the
full materialization index for authorized case rows, mechanism DAGs and
conditioned starter-support bounds; the durable journal remains recovery
authority.
The sealed target starter count is request-wide; it is not relabeled as an
individual mechanism or node's starter count. Those correlated subject fibers
remain in their own authenticated result layer.

The next root query is now authored, but deliberately not launched:
`personskat_mechanism_landscape_conditioned_100_dkk_grid_200k_2026` uses the
same 2,000-edge relation and admission predicates with `find all`. Its named
views retain every admitted edge, every successful case/signature incidence,
typed replay-unavailable terminals, closure-qualified structural support,
distinct structural mechanisms, raw signatures and edges per 1,000-DKK income
bin, and the same separate grains per mathematically floored 50-DKK modeled
net-change bin. The authored mechanism counts now group
`structural_mechanism_id`; their neighboring `raw_signatures` fields retain the
replay-sensitive count explicitly. These views are exact for the complete
admitted target only if their frontier closes without
replay-unavailable edges. A fresh journal is required
because this is a different question. Compact signature/receipt journaling now
exists, but this `find all` query remains deliberately deprioritized: it would
request endpoint replay for all 2,000 admitted edges, whereas the selected-only
multidimensional query can first validate positive mechanism and starter
support with a sparse replay target.

The first fresh one-case mechanism calibration on 2026-08-31 now closes. It is
deliberately not a landscape result: the hidden diagnostic query contains one
source and one `0 -> 1` audit step only. Both endpoint traces stayed below the
65,536-event replay ceiling (`52,787` Before events and `52,931` After events),
the stream minted one complete differential signature, and the selected target,
case/signature incidence and raw-signature count all closed at `exact(1)` with no
unavailable case. The improvement came from a producer-certified,
endpoint-local causal memo: a hot global rule dispatch retains a fresh
activation but depends directly on the same-endpoint cold rule-selection event
for the identical checked family and canonical arguments. The memo is cleared
at every endpoint boundary, never evicts a live causal anchor, charges the cold
dispatch's exact step cost on a hit, and fails closed while any dynamic
`RuleScope` is active. It admitted 1,408 checked families and observed 319--323
global causal reuses plus 1,271--1,297 existing scope-local reuses across the two
endpoints. This is the first real mechanism artifact from the Personskat path;
it is not evidence about an income cliff or about any wider income/profile
support.

The `exact(1)` raw-signature count in that historical calibration is
specifically the replay-ABI-v2 complete execution-occurrence-signature count.
With only one case its cardinality is necessarily also a lower bound of one for
any derived structural quotient, but it does not validate the user-facing
structural-mechanism count. ABI v3 now replaces that raw format because v2 could
erase eventless activation anchors; the v2 run remains historical diagnostic
evidence and is not reinterpreted or resumed as v3 structural input.

The trace-only first quotient diagnostic ran over that same v2 one-case replay.
Exact Before/After pairing formed a union of 52,975 occurrence
identities; erasing invocation and visit ordinals, then refining the separately
coloured Before/After outgoing dependency multisets, reached a fixed point in
seven rounds with 52,456 structural occurrence classes. It conserved all
52,787/52,931 endpoint nodes, 3/3 roots and 54,378/54,544 edges. Exact/static
activation-path counts were 16,703/16,495, and the largest endpoint occurrence
multiplicity of one class was 72.

This is a useful negative optimization result rather than a final graph: local
invocation/visit erasure alone removes only 519 union occurrences. The
diagnostic computes causal-subgraph or unfolding equivalence from outgoing
dependencies; it is not full directed-graph isomorphism, its dense class
ordinals are not durable IDs, and it safely skips rather than publishes a
partial result if its bounded refinement cannot finish. The small reduction
shows that a user-facing structural mechanism graph must next quotient or slice
larger causal regions without retaining the entire static activation ancestry
as identity. It is not a reason to launch the 2,000-edge landscape yet.

The first calibration also found the next scale boundary before a 2,000-edge run.
The one signature's canonical definition is about 30.1 MB and its uncompressed
presentation has 320,364 structured records. Analysis and incidence are exact,
but the first publication slice emitted only 1,786 definition records before
its bounded output quantum, so the mechanism artifact is intentionally not yet
caught up. Repeatedly resuming solely to expand that presentation would be
algorithmic waste.

Publication v4 now removes that result-path blockage. A fresh one-case v8 run
closed its compact mechanism answer in three records (descriptor, incidence and
exact closure; 2,480 bytes), and closed the new
`mechanism_starter_support` view in two records (one exact group plus closure;
1,191 bytes). The group reports one distinct `(Context, Before)` starter and
one case for its one signature. The case-support graph also caught up. In the
same invocation the independent definition sidecar wrote a bounded 169-record,
8,386,031-byte prefix of its 1,226 chunks and honestly remained open, while the
semantic query lifecycle and compact answer were already complete. The local
external evidence bundle is
`personskat-mechanism-single-case-debug-20260831-v8.state` with sibling
`.output`; it is diagnostic execution evidence, not a tracked fixture or a
wider tax result.

The exact native classifier was compiled from a compiler-proven 9,275-statement
dependency slice. Its first installation was the expensive step; the resumed
epoch regenerated the content hash in about 4.5 seconds, reused the cached
executable, classified the remaining coordinates and closed every downstream
empty frontier. The ordinary interpreter remains the atomic whole-batch
fallback, and the first native batch must agree with it before native results
are trusted. Resource containment is a passive ceiling around this path, not a
separate exploration phase.

This result is not an exact-empty certificate for 1-DKK transitions or other
profiles. It also revealed one provenance debt in the audit source: the
21-field configuration does not expose birth date, while
`personskat_ingen_pension()` fixes it to 1990-01-01. The completed result is
therefore for that concrete helper-conditioned birth date. The next revision
must promote birth date into the source relation; doing so changes the checked
identity and correctly requires a new run state. Until then the profile must
not be described as completely field-exposed.

The first expensive fused transition of each cold run is now a one-member
calibration quantum. Once its complete TO/admission/FIND batch is appended, the
coordinator immediately installs that prefix instead of waiting for the normal
4-MiB segment threshold. Later fused batches target about five seconds, shrink
immediately when slower, grow at most twofold, remain capped at 256 members and
will not start after a learned one-member estimate no longer fits the remaining
slice time plus a 250-ms reserve. Warm slices keep that operational estimate;
cold recovery deliberately recalibrates. None of these timings or batch sizes
enters a semantic identity or result. Only after the next slice installs such a
prefix can a later invocation be described as a continuation or resume.

A static peak-memory audit then found that exact semantic state was being
duplicated at several representation boundaries even though no extra evidence
was gained. The implementation now shares constructor-field payloads across
case values and cold replay, keeps the common singleton successor fiber out of
a `BTreeMap`, stores one- and two-member provenance sets inline, derives the
terminal analysis root over borrowed builders and moves those builders into the
closed snapshot, and publishes grouped views without `choice` by reducing over
borrowed durable contributions. Fresh expression evaluation and full durable
record equality checks remain mandatory. In the retired 200,000-edge stress
relation the last path retained one population-sized vector of references only
on the general extensional fallback. The current conditioned profile summary
has a narrower exact theorem:
the source-image certificate proves one Context value crossed with an injective
`0..<2_000` Before factor, while the checked result shape contains only direct
Context group keys and `count_distinct(before)`. Its durable artifact therefore
commits the one checked group-value tuple, exact source-population root and
cardinality in constant space. It neither constructs 2,000 source-result rows
nor invents a representative `SourceKey`. Ungrouped, choice-bearing and
unrecognized grouped views intentionally keep the general reducer path.
Durable staged values still represent an arbitrary all-`None` array canonically
as its logical length, and the evidence-ID collision index stores membership
rather than a never-read copy of the wide row identity. Public classification
counts now traverse a borrowed case-root support view instead of cloning the
support catalog merely to render a report. Exact support closure also validates
and hashes the borrowed catalog, retaining only key/ID validation sets, and
advances crash-safely through obligation-frontier seal, catalog seal, then the
authenticated closed root. This both removes the terminal journal/support copy
and fixes the formerly unreachable catalog-seal transition. Result publication
now performs its full reconstruction once, releases the invocation-owned
projection copy before closure construction, and retains only a compact
process-local witness for later terminal checks; typed restore remints that
witness through the full boundary. A closed mechanism request similarly keeps
only a compact receipt beside the live builder, rejects later request payload,
and remints the receipt before the builder is moved into the final snapshot.
These are implementation and static-review checkpoints, not Personskat execution
evidence; the first governed slice must still measure the real classifier slope
before a longer continuation is admitted.

The post-change static memory verdict deliberately separates experiment from
blind completion. The retired `0..<200_000` 1-DKK relation remains a **no-go**
for brute-force continuation. The current `0..<2_000` coarse relation is the
next bounded execution target. Its source-result contribution is
proof-specialized and constant-size, while its exhaustive residual classifier
remains linear in coordinates; the ungrouped `cliffs` view plus mechanism graph
scale with the number of genuine selected cases. Whole-state compatibility
helpers such as extensional snapshot finish remain outside the production
slice/report route because invoking them would reintroduce population-sized
clones.

The old Cartesian/probe executor is not a compatibility target. Pareto
evaluation, proof-region closure and broad multidimensional Personskat closure
remain later implementation work. No result from the earlier v0 artifact
schemas is evidence for this contract.

### Final pieces to lock before the broad run

The architecture now has a strong center, but the following pieces must be made
coherent before a nationwide through-1,500,000-DKK exploration is a sensible
execution target:

1. Finish the checked reachable dependency closure behind each layered identity
   and derive the source-coverage manifest from that same closure; physically
   unhook the v0 Cartesian/probe path.
2. Make an exact `SupportCell`, rather than only one materialized `CaseId`, a
   first-class evidence unit. Cells carry canonical finite support, exact or
   open image cardinality, disjoint-union partition proof and a resumable
   concrete materializer. Producer-assignment counts never become row or case
   counts without injectivity or exact image evidence. A partition replaces a
   typed parent obligation with one same-claim child obligation per cell; it
   does not falsely certify the broad parent uniform when its children have
   different cliff or mechanism outcomes.
   Root obligations are explicit, every child obligation must be reachable,
   and direct parent evidence is mutually exclusive with refinement. Resume
   cursors stay outside the semantic evidence root. The income refiner splits
   one interval factor while carrying the remaining commune/profile product
   unchanged; it may lift that split into the mapped case population only with
   accepted injectivity or a general disjoint-image proof.
   The first narrow production slice at this seam is now implemented. Support-plan
   registration seeds `SupportCellReady` plus canonical root resolvers. A
   checked verifier replays the assignment/source/successor/case producer chain
   and always issues mapped-case injectivity for recognized producer shapes. It
   additionally issues exact mapped-case cardinality only for an exact
   independent Context/Before product with a singleton successor. The
   conditioned `0..<2_000` query has exactly that stronger shape. Its journal
   stores the canonical artifact, structurally restores it on decode,
   re-verifies it against the installed plan on apply, and atomically remints
   the declared injectivity and cardinality evidence. The support scheduler now
   accepts this source-population certificate before classified or ordinary
   source work; a proper-prefix crash resumes from the missing semantic event
   or resolver completion in canonical order.
   Generic injectivity-only artifacts remain durable evidence rather than being
   discarded for lack of a count. Bare proof-receipt hashes remain rejected.
   The slice report can therefore say `cases = Exact(2000)` while concrete
   admission and FIND are still lower bounds.

   A first plan-owned uniform-admission producer now proves only direct checked
   Boolean literals (including the empty conjunction) and rejects all other
   shapes. Its root evidence and concrete admission events are cross-checked at
   journal application in both arrival orders, so a contradiction cannot poison
   a resumable authenticated prefix. The public report repeats the consistency
   check as defense in depth. Personskat calls remain deliberately unsupported
   by this literal proof and therefore use the exhaustive bounded classifier.

   This is exact candidate-transition support, not an income-cliff answer.
   General split lifting and weighted result/mechanism consumption remain the
   positive symbolic frontier. The main stream driver now selects the bounded
   classifier for its recognized one-axis partition; unsupported shapes retain
   the ordinary concrete evaluator. Static formatting and soundness review is
   clean through the terminal result/mechanism ownership boundaries; the
   post-change focused build and live Personskat slice have not yet run.

   The first semantic blocker is now removed. The checked query owns a
   canonical, name-free classification graph plus a separately authenticated
   runtime-shape adapter; a request capsule binds both to the support plan,
   provenance and exact relation/admission/question identities. Its bounded
   exact evaluator is threaded through classified chunks and reuses complete
   pure-call results across adjacent Before/After endpoints. Unsupported
   checked sites or operations residualize the whole classification lane and
   fall back atomically to the ordinary checked evaluator.

   The present 350,000-DKK commuter query marks that boundary precisely. Its
   finite FROM construction and deterministic TO successor fit capsule V1, but
   the Before/After validity predicates and FIND cross the multi-statement
   Personskat observation, and the transition predicate includes structured
   profile equality. Admission/FIND therefore residualize and capsule execution
   delegates the complete batch. The published exact 10,500-case result above
   came through the query-bound native V2 classifier and checked parity canary;
   it is not evidence that capsule V1 executes the Personskat rule graph.

   The meaningful next capsule extension is a sealed checked-observer leaf. It
   would bind the exact checked observation call into capsule identity, evaluate
   and cache it independently for Before and After, and leave the surrounding
   projections and comparisons in the canonical graph. This creates the desired
   adjacent-endpoint reuse without pretending that merely lowering a local
   binding has made all deeper Personskat collections and dispatch executable
   in capsule V1.

   The remaining symbolic blocker is narrower. The capsule-bound one-axis
   proof producer can replay exact scalar quasi-affine/Boolean graphs and
   acyclic calls, but its proof artifact is not yet a durable scheduler/journal
   event and richer Personskat rule-family behavior may still residualize.
   Uniform or split evidence may be minted only after that exact capsule-bound
   replay seam is installed; a semantic ID or literal-only recipe is never
   substituted for the checked graph.

   Positive consumption also has a narrow first rung. Exact population totals
   can become public without inventing cases, but concrete result expressions
   and mechanism replay consume real `CaseId` values, not one representative
   per cell. The bounded classifier therefore retains every positive selected
   run as a proof fragment and a sparse producer materializes exactly the cases
   in those runs. The certified population root remains the independent
   completeness authority; a downstream seal is valid only when the canonical
   materialized selected-run cover and its concrete CaseIds agree with that
   proof's exact cardinality. This is sparse relative to the candidate space,
   not a claim that an exploration with many genuine findings can avoid
   retaining those findings. `.len()` of fragments or evidence records is
   never silently substituted for a case count.

   The concrete bounded fallback also uses the support DAG instead of
   rebuilding population-sized state. Its exact injective income/case image is
   partitioned into bounded ordinal chunks. A chunk sweep evaluates adjacent
   endpoints once in income order, then refines the chunk into exhaustive
   rejected intervals, admitted/non-cliff intervals and concrete cliff cases.
   Only cliff cases need full result and mechanism replay; interval evidence
   contributes its exact logical count. Batches advance one authenticated
   materializer cursor with no gaps or overlaps, so stopping after any batch is
   a real covered prefix. For the current coarse relation this preserves the
   `2,000 = rejected + non-cliff + cliff` conservation law and changes the
   expected retained cost from one row
   per income toward chunks plus outcome changes plus actual cliffs. Alternating
   outcomes still degrade honestly to linear size.

   This is not permission to infer an interval from matching probes. Without a
   checked abstract proof, every coordinate in a compacted concrete interval
   was evaluated. The classification capsule now supplies the replayable typed
   program for such a proof; scheduler/journal integration must still verify a
   capsule-bound proof artifact before it may skip evaluations. That later
   acceleration does not change the support DAG or public counting rules.

   The first exact implementation of that fallback now fixes one canonical
   partition of the conditioned `0..<2_000` mapped case image into eight
   children of at most 256 coordinates. The compact partition artifact is
   structural scheduling evidence, not a case count. For each child, the
   checked evaluator visits every coordinate and records one of exactly three
   outcomes: rejected, admitted/not selected, or admitted/selected. Adjacent
   equal outcomes coalesce into maximal run cells. Each run carries its own
   exact cardinality plus admission and, when admitted, FIND evidence; every
   proper sub-run also carries injectivity restricted from the accepted mapped
   image. Therefore the closed count is the conserved sum of run
   cardinalities, never the number of chunks, runs or retained examples.

   Whole classified-chunk acceptance is one atomic semantic journal event.
   Replay first matches the exact durable root injectivity evidence, accepted
   canonical partition/refinement and exact child injectivity evidence, then re-verifies
   the artifact. It installs the run partition, typed evidence and leaf seals
   through a bounded append transaction whose exact-key undo log is at most
   `9 * run_count + 3` entries. Any late validation error restores those keys
   in reverse order; success appends only the new chunk. This preserves the
   all-or-nothing journal boundary without copying the complete accumulated
   support catalog once or twice per child. The causal append boundary also
   declares fresh roots without scanning retained refinements, and derives the
   opposite cardinality/injectivity obligation ID before consulting its reverse
   evidence index. Admission of the next chunk therefore does not rescan all
   earlier proof records. The authenticated partition event also retains the
   opaque verified partition authority it reminted from that durable evidence.
   Classified slices, completed chunks and later selected-run materialization
   index that replay-derived value directly; cold replay rebuilds all eight chunk
   descriptors once, not once per slice or positive run. A typed classified-progress
   chain then binds the exact contiguous `(chunk ordinal, artifact, endpoint)`
   prefix; the root-relative materialization cursor advances last only as its
   operational mirror. Generic cursors cannot choose or advance this branch.
   No semantic half-child is visible. To make the operational time limit honest
   even when one 256-coordinate child is expensive, the runtime seam is a
   second, typed layer: nonempty contiguous within-child slice checkpoints.
   Their boundary-independent transcript is a coordinate hash chain
   `t(i+1) = H(t(i), coordinate, source key, case id, outcome)`. A checkpoint
   binds its predecessor and before/after roots, advances only typed partial
   progress, and installs no run evidence. The final slice deterministically
   coalesces equal outcomes across slice boundaries, byte-compares the derived
   canonical whole-child artifact, and only then performs the existing atomic
   semantic acceptance. Thus adaptive slice sizes and host timing can change
   the journal's checkpoint history without changing final run IDs, support
   roots, case DAG, or answer. One evaluator coordinate remains the minimum
   non-preemptible unit; an installed checkpoint resumes exactly, while an
   unflushed provisional tail may be repeated after a hard power loss.

   Keeping the fixed 256-coordinate semantic partition is important. Making
   the partition itself adaptive would make mathematical roots depend on host
   scheduling; shrinking it to one would create 2,000 proof children with no
   semantic gain. With slice checkpoints the cold controller may grow
   `1, 2, 4, ... 256` within the first child, then reuse its learned quantum
   across the remaining canonical children.
   The mirror cursor remains outside the evidence root, while the outcome
   transcript, progress binding and run cells are authenticated evidence. The
   retained size is `O(B + R + X)` for `B` canonical chunks, `R` outcome changes and
   `X` concretely selected cases, with an honest `O(N)` worst case when outcomes
   alternate. Evaluation remains `Theta(N)` for an uncertified black-box
   predicate; adjacent endpoint reuse aims for `N + 1` distinct Personskat
   observations rather than changing that lower bound.

   The companion proof/reduction seams are now connected to the audit stream.
   A plan-bound source-image proof recognizes the
   exact singleton-Context by finite-Before producer, proves its canonical
   `SourceRowImage` injective with cardinality 2,000, and derives a stable
   source-population root from the exact typed evidence IDs, and the support
   scheduler emits that proof before population work. The classification-count
   reducer traverses only the case-root-reachable support subtree and exposes
   rejected, admitted/nonselected and selected lower bounds for an open prefix,
   upgrading them to exact counts only after every reachable leaf is sealed;
   unrelated auxiliary source proof cells cannot poison those totals. The
   public slice report consumes both authorities without adding overlapping
   support and concrete observations. It also proves a useful constant-space
   theorem for `conditioned_profile_summary`: singleton independent Context plus
   an exact one-factor Before image means every Context-only group expression
   has one value, the group has 2,000 members, and direct
   `count_distinct(before)` is exactly 2,000. The result layer must retain
   that certified-source recipe and its own input/root domain; evaluating
   ordinal zero may compute the one group value, but it must never turn that
   row into a weighted representative or fabricate a 2,000-key extensional
   source seal.

   The sparse producer and its authenticated journal/driver bridge now
   re-evaluate and retain every concrete `CaseId` in one positive selected run
   of at most 256 coordinates. The positive certified-selected closure compares
   that exact concrete set with the independent proof cardinality; the
   within-child slice protocol resumes partial classifier work; and the compact
   certified-source result is retained, decoded and rebound to the installed
   analysis/source authorities without replaying 2,000 result rows. The
   artifact is also part of the result-layer snapshot and catalog root, so
   terminal validation, cold snapshot restoration and public input counts use
   its logical `N` instead of misreading zero physical rows as an empty result.
   Selected-run admission likewise validates one batch-local relation overlay,
   all claimed identities and all admission/FIND conflicts before merging the
   three durable catalogs in canonical coordinate order. Peak transaction
   memory is therefore `O(k)` for `k <= 256`, not another copy of every earlier
   selected case. A static trace of the current exact first-audit path gives
   `Theta(N)` checked endpoint/classification work, eight fixed semantic chunks,
   and retained `O(B + R + X)` state for `B` chunks, `R` homogeneous outcome
   runs and `X` genuine cliffs; the certified source summary remains `O(1)`
   physical rows while carrying logical count 2,000. Dense answers still
   honestly retain `O(X)` cases, but no transaction has to double that prefix.
   Once a result input closes, its canonical evidence root is cached because
   no later row insertion is legal. Bounded projection publication validates
   only the newly durable suffix against the incremental prefix root; a cold
   process pays one linear revalidation, while warm publication is linear in
   the records it actually appends rather than quadratic in the growing prefix.
   First publication still reconstructs and validates the exact output, then a
   compact witness binds its immutable publication/spec/evidence/projection/
   result identities and exact count shape. Selected, source and incidence
   drivers drop their completed output caches before that closure is derived;
   terminal analysis checks do not materialize the result again. The witness is
   deliberately absent from snapshots and semantic hashes and is fully reminted
   on typed restore.
   Their shared schema/hash generations are frozen together for the first
   durable audit. None of this is yet Personskat evidence until the focused
   build and governed run succeed.

   Mechanism replay distinguishes genuinely retryable invocation pauses from
   fixed replay-ABI capacity. The production interpreter's evaluation-step and
   trace ceilings are deterministic ABI capabilities: crossing one records a
   typed unavailable terminal and degrades mechanism certainty. It cannot
   return an open retry that restarts the same endpoint at the same ceiling
   forever. Time, cancellation and host-resource pauses still mint no terminal
   and remain resumable.

   The first one-case Personskat replay has now localized that capacity problem
   without widening the income range. Its Before trace retained 65,536 events
   and then failed closed on event 65,537. Diagnostic counting allowed the
   already-running evaluation to finish without retaining further nodes: the
   endpoint attempted 252,573 decision events across 73,014 activation paths.
   Of 71,163 cold global rule-family dispatches, only 2,132 distinct encodable
   `(checked family, canonical arguments)` keys occurred; 292 calls had
   arguments outside that diagnostic encoding, and the separate scope-local
   memo already reused 1,849 selections. For example, `dato_gyldig` was
   dispatched 10,035 times for 11 argument keys, `dato_dage_i_måned` 10,420
   times for 12, and `dato_nøgle` 7,720 times for 11. These are performance
   measurements, not a mechanism or tax result.

   That evidence rejects a blind trace-cap increase. The next replay reduction
   is a producer-certified, endpoint-local causal memo for effect-free global
   rule families with no dynamic captures. One cold dispatch retains its value
   and completed selection node; every equal later call consumes a fresh
   checked activation and emits a fresh selection which points directly to the
   cold selection. The references form a star rather than a chain. The cache is
   bounded and non-evicting for that endpoint so an event index cannot become a
   dangling causal reference, and it is discarded before the other endpoint or
   another case. Families outside the checked plan continue through ordinary
   evaluation. This should turn repeated rule calculation into a causal DAG
   before considering a larger ABI ceiling.

   The first audit's public materialization is intentionally not yet the final
   graph browser. It already streams authorized selected configurations to
   `views/cliffs.ndjson`, publishes closure-aware case, raw-signature and
   structural-mechanism counts, and
   exports a searchable signature DAG plus case/transition/signature incidence
   while the mechanism frontier is still open. Replay rederives a compact
   interleaved signature/reason/terminal discovery chain. Each signature event
   expands before any dependent terminal into a header; canonical Before then
   After node, outcome, root and dependency-edge records; and the original
   bounded canonical-byte chunks for independent reconstruction. The cursor is
   `(event ordinal, definition-part ordinal, closure emitted)`. A bounded batch
   freezes its event end and optional exact closure root before bytes are
   appended, so recovery cannot borrow later discoveries to validate an older
   torn suffix. One closure record is emitted only after the final event and an
   authenticated closed-mechanism root. That authority is now a constant-sized
   receipt containing the request/root/counts, exact incidence-result row-set
   seal and frozen discovery end. The journal does not retain a second closed
   incidence snapshot: definitions and reasons stay in the live builder until
   final analysis close moves them once, while terminal discovery stays in the
   existing operational sidecar. An equal close replay remints and compares the
   full receipt; any later target or artifact event for that request is rejected
   before mutation.

   Every graph record is signature- and endpoint-addressed, contains no replay
   state/context value, and has a deterministic definition-local ordinal. A
   lazy validated byte-offset/cumulative-edge index is built only for an
   addressed signature; no fresh process scans every known definition to
   resume one line. The internal case/support DAG now also has a pure public
   projection attached to the same resumable publisher as
   `graphs/case-support.ndjson`. A classified partition publishes root → chunk
   → region → selected materialization → authorized case. A run that closes
   without ever minting such a partition publishes root → exact classification
   region → authorized case instead, with one closure naming the actual
   classification, support-prefix and selected-population authorities. This
   second shape is essential: a uniform proof or complete concrete path must
   not leave a completed exploration's graph permanently waiting for an
   artifact its scheduler branch cannot create. Neither projection exposes
   coordinates, state/context values, materializer identities or proof
   payloads. Publication v3 freezes an exact flat source end before a
   result-view or case/support batch appends, so torn-tail recovery cannot
   validate later rows under the older pending checkpoint.

   The focused current-source oracle has now executed this path. Its sealed
   journal contains four exact cases, two selected cases, one shared mechanism
   signature and two mechanism incidences. Reopening the unchanged journal
   appended the previously missing seven-record classification-summary graph:
   one root, three exact outcome regions, two authorized case nodes and one
   closure. A second reopen appended zero semantic events, graph lines or
   source ordinals and preserved both graph and mechanism-file digests. This is
   runtime evidence for the general stream and publication architecture, not a
   Personskat finding.

   The graph's deterministic artifact IDs and roots are audit commitments, not
   hiding commitments. This conditioned audit contains intentionally declared
   model inputs, but a run over private facts must keep its output directory
   private even when raw values and unauthorized `CaseId`s are absent: a
   low-entropy fact may still be testable against a commitment. External/cloud
   publication requires an explicit release policy or projection-local/hiding
   identifiers; the resumable local evidence chain alone is not anonymization.

   The first governed command has one crisp proof-bearing checkpoint. Its empty
   run-state and output directories are private (`0700`) and distinct. After a
   focused rebuild, a report may state `sources = Exact(2000)` and
   `cases = Exact(2000)` only when the retained source-image and case-image
   certificates remint from that exact journal prefix. Later classified counts
   name only sealed support leaves until closure. A watchdog refusal leaves
   both directories empty, while a genesis pause at sequence zero is a real
   pause but contains no semantic evidence. Neither may be described as an
   empty or partial audit result.

   Historical evidence from the retired 1-DKK relation remains useful for
   performance only; its checked identity cannot resume the coarse-grid run.
   Its first real Personskat stream prefix came from an optimized five-minute
   invocation that reopened the durable journal and paused cleanly at sequence 75,
   head `c3caf2914cc0f6cd44b6df32d5fcf9baa467473f32ded237249ddf29aecd45f0`.
   The checked source and case images are both exact at 200,000 transitions;
   the classified prefix is a lower bound of 256 admitted cases, with zero
   rejected, zero selected and 256 admitted-not-selected. This is evidence
   only for salaries 0 through 255 DKK. It is not an exact-empty cliff result:
   relation, FIND and analysis frontiers remain open.

   Publication caught up to that retired prefix. The conditioned-profile view
   contains
   one exact group covering all 200,000 declared income coordinates, and the
   case/support graph contains its root, the first exact 256-case chunk and one
   homogeneous admitted-not-selected region. Cliff, mechanism-summary and
   50-DKK loss-bin files are presently empty because no selected case has yet
   entered those layers. The manifest reports seven caught-up artifacts rather
   than confusing an open empty prefix with a closed empty answer.

   That retired prefix is also useful pre-memo performance evidence.
   Release-mode preparation fell from roughly 113 seconds to 16 seconds, but the semantic slice classified
   only 256 coordinates in roughly 284 seconds. A linear continuation at that
   rate would take about 61 hours on the current one-worker path, so it must not
   be launched as the intended audit. The report additionally exposed that the
   producer-planned observer memo was disabled. That eligibility gap was first
   repaired, but the initial resumed measurement then exposed a second, deeper
   bug: entering an ordinary local statement block invalidated the installed
   memo as though the program's static declarations had changed. Declaration-
   free blocks now retain it; functions, rules, invariants and runtime type
   registration still invalidate it.

   The corrected one-minute continuation closed the second canonical chunk and
   paused at sequence 110, head
   `118e72ca4d224e9b93ce5c69e16c1fc24ae7fad06e8d9526a4ed76f6abc9806f`.
   The exact source/case population remains 200,000; the classified lower bound
   is now 512 admitted-not-selected cases, with no selected or rejected case in
   that prefix. This is still not an exact-empty result.

   The adjacent-endpoint reuse signal is decisive. The first cold one-member
   proposal took about 318 ms; the next two-member and three-member proposals
   took about 328 ms and 514 ms total, or roughly 164--171 ms per warm adjacent
   case instead of 1.1 seconds. That is about a 6--7x reduction. A purely serial
   concrete continuation of that old 200,000-edge relation is consequently
   still measured in hours (roughly nine hours of evaluator work before
   operational overhead), so it is not the next run. The first useful reduction
   is semantic: the new coarse endpoint-screen relation contains only 2,000 exact
   100-DKK edges. Its unresolved WHERE/FIND residual is compiled into a
   query-identity-bound helper, while the coordinator alone materializes cases,
   folds transcripts and writes the journal. Any unavailable helper batch is
   discarded wholesale and evaluated by the checked interpreter. In parallel,
   proof-backed interval/delta closure can later certify homogeneous regions
   without evaluating every krone; source-event hints may propose splits but
   never certify skipped members.

   The semantic classifier already begins with one coordinate, adapts toward a
   five-second bounded quantum and never evaluates more than 256 coordinates in
   one proposal. The first attempt to flush every proposal proved too eager:
   tiny 1/2/4-member ramp-up proposals paid the full filesystem sync cost. The
   semantic stream therefore flushes the first expensive proposal and then on
   a bounded cadence. The public CLI now keeps one epoch warm, divides the
   invocation into roughly 15-second `run_slice` micro-slices, and publishes
   newly durable result/graph suffixes plus an atomic manifest after each slice
   before continuing. Publication no longer waits only for the outer invocation
   to return. One indivisible semantic quantum can still exceed the cadence;
   that is a remaining cursor granularity issue, not permission to publish
   uncommitted state. None of this changes a CaseId, support cell, raw signature,
   structural mechanism, journal root or final answer; it makes the existing
   evidence observable while the stream is alive.
3. Persist the content-addressed source/successor work DAG and its canonical
   open frontier with incremental indexed state, so one accepted event does not
   clone or re-sort the accumulated exploration and pause/resume does not
   rescan closed work.
4. Finish separating base relation, admission, question, view, mechanism request,
   durable-evidence and operational layers while preserving explicit privacy
   and retention authorization; carry the frontend's checked
   case-view/mechanism/evidence-view dependency DAG into closed IR and execution;
   make grouping, measures, distinct aggregates, all-ties choice and Pareto
   choice deterministic. Every layer consumes concrete singleton cells and
   certified larger cells through one interface.
5. Extend the smoke-proven concrete-case endpoint replay and exact incidence
   path to certified larger cells and admission-scoped mechanism targets.
6. Add the optimizer as a proof portfolio: checked relevance slicing and
   before/after delta reuse first; affine, interval and congruence closure plus
   guard-driven splits next; categorical decision diagrams, integer-polyhedral
   counting and bounded SMT refinement where their fragments fit; canonical
   concrete evaluation for every residual. Binary search is a proof only
   inside a certified monotone cell.
7. Publish exact support carried by every behavior quotient, so the first broad
   result can distinguish explored dimensions, explicit conditioning, proved
   irrelevance and genuine model gaps. Publish a symbolic `EvidenceRoot`
   without claiming the extensional `RelationContentRoot` until concrete case
   materialization actually exists.

The next experiments should use small but genuinely multi-dimensional complete
profile relations, not another single fixed person or only the already-known
341,500-DKK, 60-km and 130-km anchors. Widen income and profile dimensions while
checking what the observable run discovers and which frontier remains. A
dimension with no encoded effect should close as irrelevant rather than being
forced to produce an interesting answer.

The checked bootstrap is now complete. It ran
`personskat-income-cliffs-200k.explore.runa` over one conditioned profile and
2,000 coordinates representing lower annual salaries `0, 100, ..., 199_900`
DKK, with a deterministic `+100 DKK` successor including the transition into
exactly 200,000 DKK. It closed with 2,000 admitted-not-selected transitions,
zero rejected transitions and zero selected mechanisms. This is an exact claim
only about that declared coarse endpoint relation; it is not an exact-empty
certificate for every 1-DKK transition.

The discovery policy follows from the case and mechanism graphs rather than
becoming a second hand-authored audit. Selected coarse edges, admission changes
and mechanism-signature changes nominate candidate neighborhoods for a finer
query. The current query can supply the first two signals but, because mechanism
replay targets `selected`, its exact-empty result supplies no signature-change
signal. The authored `find all` mechanism-landscape query instead compares all
admitted coarse endpoints and reuses each endpoint trace across neighboring
edges. Its signature-incidence stream can nominate candidate bounds without
changing the cliff query's meaning.

A `+1 DKK` successor is nevertheless a separately declared and checked
relation, not a child whose proof obligations are silently discharged by the
`+100 DKK` journal. Its result is globally exact only when that finer relation
is itself completely covered, or when a future explicit checked bridge proves
which coarse regions transfer. Resolution is an operational search strategy;
coverage remains tied to the declared relation identity.

After the conditioned mechanism-landscape audit, the next widening keeps the
same income horizon but replaces the conditioned context with a genuinely
multidimensional coherent profile relation. It must additionally publish
profile counts and 50-DKK loss-bin views. This is still a system audit rather
than a claim that the model covers every real Danish taxpayer; any
profile-model gaps remain explicit output.

The current conditioned Personskat relation has no selected coarse cliff in
that horizon, and every declared source, successor, classification obligation
and downstream empty frontier closed. The separate small synthetic query with
an intentional shared mechanism supplied the nonempty integration signal, so
this quiet Personskat range does not leave mechanism incidence and
post-mechanism grouping structurally unexercised.

There is therefore no planned 1.5-million-income execution merely to obtain an
early number. The milestone is a profile-and-income relation plus proof-closure
experiment. The desired signal is that widening the declared world adds a
small number of source events, mechanism signatures and certified cells rather
than requiring one full Personskat evaluation for every krone-profile pair.

The intended execution portfolio is proof-first rather than worker-first:

1. checked interval/delta reasoning closes every homogeneous region it can
   prove;
2. a semantics-equivalent compiled classifier evaluates only the residual
   coordinates and returns ordered classification outcomes; and
3. the ordinary interpreter replays selected cases to derive mechanism and
   incidence evidence.

The coordinator alone mints `CaseId` values, folds the canonical transcript and
appends evidence. A compiled kernel, local worker pool or distributed worker is
therefore an execution detail, never an alternative source of truth. This
keeps the same result identity while allowing later map/reduce acceleration.

Resource control is a passive operational failsafe, not an Explore research
thread. Work runs beneath the operator's 80-percent CPU/RAM policy and 6-GiB
process ceiling; unsafe pressure causes a checkpointed pause at the next work
boundary, after which the same journal can resume. Resource limits, worker
count and scheduling order never alter the bounded question or its evidence.
Normal runs should not repeatedly poll or report resource state beyond what is
needed to enforce that guard; only a tripped guard belongs in the user-visible
result.

## Reading the result responsibly

The hand-written audit is executable evidence about the checked-in Futuruna
model and its narrow declared domain. The broad exploration becomes
evidence only after its declared world is actually run or exactly closed; the
first-class surface is implemented experimentally, but that is not a broad
Personskat result. Neither is an individual tax determination.
Verify the facts and the encoded phase-out interpretation before relying on a
witness outside research or model review.

Boundary and commuting sources for this example:

- [Ligningsloven, LBK nr. 1500 af 24 November 2025](https://www.retsinformation.dk/eli/lta/2025/1500)
- [LOV nr. 616 af 30 June 2026, § 1](https://www.retsinformation.dk/eli/lta/2026/616)
- [BEK nr. 1333 af 20 November 2025, §§ 1-3](https://www.retsinformation.dk/eli/lta/2025/1333)
- [Skatteministeriet: beløbsgrænser for 2025-2026](https://skm.dk/tal-og-metode/satser/regulering-af-beloebsgraenser/beloebsgraenser-i-skattelovgivningen-der-reguleres-efter-personskattelovens-20-2025-2026)
- [Skattestyrelsen: Kørselsfradrag (befordringsfradrag)](https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag)

The imported Personskat model carries source metadata for the remaining
components of the final-tax calculation.
