# Exploring Law with Futuruna

Futuruna can turn an encoded rule model inside out. Instead of supplying one
set of facts and asking for its result, define a finite set of possible facts,
run every case through the same canonical rules, and ask which results satisfy
your question.

The working pattern is:

```text
candidate facts -> canonical rules -> typed results -> filter -> rank -> prove
```

The proposed first-class north star developed in section 9 is:

```text
finite profile and successor relations + authenticated frontier ->
content-stable cases -> classify -> named views + explicit-target provenance
```

This workbook uses Futuruna's existing lists, `range`, `map`, `flat_map`,
`filter`, `foldl`, invariants, and proofs. The complete executable example is
[personskat-income-cliffs.audit.runa](personskat-income-cliffs.audit.runa). It
is deliberately a handwritten finite audit using ordinary collections, not a
completed run of the Experimental first-class `? explore` surface described in
section 9.

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
varied, derived, or intentionally fixed. A fact is omitted only after exact
irrelevance proof or as an explicitly reported model-coverage limit.

Section 9 gives that distinction its general form: the successor relation
declares the intervention, the question classifies its Before/After edge, and
separate endpoint replay explains the encoded rule difference. The calibration
below supplies evidence for that design; it is not the design contract itself.

## 2. Define the metric and separate fixed facts from search dimensions

Vary every supported fact named by the question. Derive dependent facts, and
fix only facts on which the question intentionally conditions so that every
result has a precise meaning. Do not fix a profile fact merely because varying
it makes the search larger: either give it a finite domain, derive it, prove it
irrelevant to the requested roots and omit it, or state that the exploration
does not cover it.

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

The input builder contains the fixed facts. The mapping contains the varying
fact and comparison. The canonical calculation remains the single source of
the tax result.

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

The following is an adaptation skeleton: define the domain values, result type,
and model-specific functions for the question first. Use nested `flat_map` to
explore combinations. Each outer value returns a list;
`flat_map` joins those lists into one search space, while the innermost `map`
creates the cases:

```runa
= search_cases = flat_map(years, |year: Heltal| {
    flat_map(municipalities, |municipality: Municipality| {
        map(incomes, |income: Heltal| {
            SearchCase(
                year = year,
                municipality = municipality,
                income = income
            )
        })
    })
})

| search_space_has_expected_size: search_cases ->
    length(search_cases) ==
        length(years) * length(municipalities) * length(incomes)

? search_space_has_expected_size
```

Then apply the same pipeline:

```runa
= results = map(search_cases, evaluate_case)
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
typed `valid_profiles` or `feasible_successors(before, context)` relation over a
large product followed by silent rejection. Derived facts do not add cases;
excluded combinations retain their classification when they are part of the
declared world; and an unsupported part of the model must remain visibly
outside the claim.

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

A fixed profile remains valuable as a calibration shard. It is not the
semantic north star. The full result should be able to reveal that a profile
dimension is decisive, irrelevant, or relevant only in combination with other
facts.

This section records the next Experimental language direction. The source
blocks below are design sketches, not syntax accepted by the current compiler.
The current normative contract and exact implementation status remain in
[Bounded Rule Exploration with `? explore`](../../docs/rfcs/bounded-rule-exploration.md),
its [implementation workbook](../../docs/rfcs/bounded-rule-exploration-workbook.md),
and [feature stages](../../docs/feature-stages.md). Those sources must be
revised before this proposed surface becomes the compiler contract.

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

It may return the unchanged state, one derived successor, or several finite
alternatives. The current `identity`, `relative`, and `independent` labels are
useful inferred IR and optimizer properties, but they are not clean user-level
categories. A household reallocation, for example, can both derive fields from
Before and branch over several alternatives. Successor domains have set
semantics: duplicate generated values for the same canonical source key do not
inflate case counts.

Source deduplication, successor deduplication and canonical value order are part
of the relation contract; discovery or storage order is not. Equal canonical
source rows collapse unless an explicit source key distinguishes them. If two
choices reaching the same After state are meaningfully different interventions,
their action identity belongs in Context; otherwise they collapse within that
source. Distinct `CaseId` coordinates may still support one extensional
`TransitionId` when their canonical Context/Before/After values are equal.
Variable successor cardinality therefore needs a dependent decision structure
with stable per-source support, not merely a Cartesian axis bolted onto the
existing generator.

Likewise, a boundary is normally a finding, not a source-supplied search hint.
The 1-DKK update belongs to the income-cliff question; a suspected threshold
does not. The compiler can recognize an affine successor, source events and
other structure without making `boundaries` the semantic definition of the
query.

One possible relational spelling is:

```runa
? explore personskat_income_cliffs_2026 {
    from {
        context = SalaryChange(amount_kroner = 1)
        source in declared_personskat_source_rows_2026(
            profiles = coherent_personskat_profile_rows_2026(
                municipalities = supported_municipalities_2026(),
                church_tax_statuses = supported_church_tax_statuses_2026(),
                households = supported_household_profiles_2026(),
                commutes = supported_commute_profiles_2026(),
                income_compositions = supported_income_compositions_2026(),
                pensions = supported_pension_profiles_2026()
            ),
            gross_salary_kroner = range(0, 1_500_000)
        )
        before = personskat_state_from_source(source)
    }

    to after = before with {
        gross_salary_kroner =
            before.gross_salary_kroner + context.amount_kroner
    }

    where before personskat_supported(before)
    where after personskat_supported(after)
    where transition salary_change_permitted(before, after, context)

    find violations of modeled_after_tax_resources_never_fall(
        before,
        after,
        context
    )

    results cliffs {
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

    mechanisms for selected from assess_personskat
}
```

`coherent_personskat_profile_rows_2026(...)` is a dependent relation, not an
instruction to cross those catalogs blindly. It joins and derives them into
whole typed profile rows; each `source` row then pairs one coherent profile with
one lower salary endpoint. Neither relation is prefiltered because its rows are
already expected to be interesting. A materialized list is a sound first
implementation when it exposes stable schema, cardinality, canonical order and
lineage. The target relational IR should retain component descriptors so the
decision structure and relevance analysis can share or eliminate profile
columns without treating the whole profile as one opaque axis.

Fixed facts are optional, visible conditioning on that relation. For example,
a Copenhagen calibration could add:

```runa
where before before.profile.municipality == Municipality.Copenhagen
where before before.profile.church_tax_status == ChurchTaxStatus.NotMember
```

Those conditions narrow the declared question and its identity. Without them,
the exploration ranges over the whole declared coherent profile relation;
there are no hidden “fixed profile facts.”

The end-exclusive source range supplies lower salary endpoints through
1,499,999 DKK; the successor reaches 1,500,000 DKK. `to after` constructs that
comparison. Scoped `where` clauses distinguish unsupported or invalid cases
from valid nonmatches. `find` states the question. `results cliffs` defines a
named view of the evidence, and the mechanism root names the endpoint
computation whose Before/After executions are compared.

The smallest final-architecture sequence is:

1. Seal the source and successor schemas, program/model identity, normalized
   producer definitions and lineage contract in `RelationId`. If a complete
   materialized relation is supplied at open, its canonical content hash is
   also sealed; an incrementally enumerated relation instead authenticates its
   discovered content and open producer frontier in the evidence journal.
2. Enumerate source rows and each row's dependent successors in canonical set
   semantics. Derive content-stable `SourceKey`, `SuccessorKey` and `CaseId`
   values from canonical content, never discovery order or a temporary
   ordinal. The stream can pause and publish lower bounds before enumeration
   closes without renaming committed cases.
3. Classify each discovered case against admission and `find` independently of presentation.
   A complete exploration with zero result views is valid.
4. Materialize zero or more named `ViewId` projections over that classified
   relation without changing its cases or classification evidence.
5. Replay an explicit mechanism request against either `selected` cases or the
   `chosen` cases of a named view; the latter target seals that `ViewId`.

The spelling remains open to refinement; this ordering does not.

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
coherent profiles, or the declaration must classify incoherent coordinates as
excluded. It must never silently pair unrelated facts and then call the product
a population of people.

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
CaseId(Carl, 199999) -> TransitionId(T1) --\
                                           > MechanismSignatureId(Σ)
CaseId(John,   9999) -> TransitionId(T2) --/
```

`T1` and `T2` stand for extensional typed Context/Before/After identities; a
person label appears in a transition identity only when it is genuinely part of
the modeled state. Case coordinates are never collapsed merely because their
displayed fields look alike.

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
operational provenance. They do not change the declared world or answer. The
current Experimental durable implementation still contains a phase-zero probe
milestone and related names; removing that global barrier is implementation
work, not a reason to preserve it in the language.

The append-only journal remains authoritative for recovery. Every constructible
case commits its `CaseId`, canonical Context/Before/After transition and
classification atomically, together with only authorized case-local values
evaluated in that transaction. Extrema, representatives, general result views
and mechanism replay have their own closure records. Bounded snapshots are
materialized views, not the source of truth. The blockchain analogy remains
useful only in this precise sense: the run appends content-addressed evidence
about a finite world and can resume at its authenticated head; it is not mining
cases or running distributed consensus.

### SQL-like views over graph-backed evidence

Explore should borrow SQL's separation of relational stages without inheriting
SQL's bag semantics, null rules or nondeterministic limits:

| Explore concept | Relational role |
|---|---|
| `from` | finite source relation |
| `to after` | dependent finite successor relation |
| scoped `where` | Before, After and transition admission |
| `find matches`, `find violations`, `find all` | selected transition relation |
| `group by` or `group all` | finding equivalence |
| `measure` | named exact per-case scalars |
| grouping reducers | exact group minima, maxima, spread, support and ties |
| `having` | group filter applied after its required closure |
| `select` | public projection and privacy allow-list |
| `choose` | explicit one-row, all-ties or frontier cardinality policy |

The closest SQL analogy for `to after in successors(before, context)` is a
`LATERAL` join or `CROSS APPLY`: the finite successor relation is evaluated for
each source row and may return a different number of rows. That is the crucial
generalization beyond a Cartesian list of profile switches. `results` blocks
are named `SELECT`-like views; mechanism replay is the extra provenance layer
ordinary SQL does not provide.

The current `output.key` hides a semantic `GROUP BY` inside a presentation
block. The next surface should make grouping, measurement and representative
choice explicit. `find all` is equally important: “which municipality minimizes
tax?” is an optimization over admissible alternatives, not an artificial
always-true Boolean witness search.

The exact case relation remains primary. Named views are projections over it;
no mandatory grouping key should force a choice between hiding profile
multiplicity and emitting an unreadable row for every profile field. `each
case` preserves `CaseId` as logical row identity, so two cases remain distinct
even when every selected display value is equal.

Five layers should remain separate:

- **exploration-relation identity**: resolved program and model, state and
  context schemas, typed source domains and relations, canonical successor
  semantics and endpoint membership, admission, and the normalized question
  with its selected polarity;
- **mechanism-request identity**: explicit canonical endpoint observation
  roots, target scope and signature normalization; a representative-scoped
  request also references its `ViewId`;
- **view identity**: grouping, measures, group filters, selected public fields,
  ordering, choice and privacy policy;
- **durable-evidence identity**: immutable relation and mechanism requests plus
  evidence-retention authorization, bound to evaluator, journal and
  serialization-schema contracts; and
- **operational records**: each invocation's run-state path, time and resource
  limits and workers, plus scheduler and pause events accumulated in the
  journal across resumes.

This separation allows one durable body of evidence to support another
authorized result view without pretending that a new grouping changed the
finite world. A derived view artifact has its own identity over
`(EvidenceRoot, ViewId, report schema)`; it does not mutate the underlying
`RunId`. Retention and privacy may limit which later views can be materialized,
but presentation should not define case or mechanism identity. This is a design
correction: the current Experimental query hash still closes output and
grouping fields together with the exploration request.

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

    results lowest_tax {
        group all
        measure [tax_ore = tax_due(after)]
        having varies(tax_ore)
        select [municipality = after.tax_municipality, tax_ore]
        choose all minimizing tax_ore
    }

    mechanisms for view lowest_tax chosen from assess_personskat
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

If the encoded model has no municipality-dependent result, exact closure proves
zero spread and publishes no “best municipality” recommendation.

The household question uses a finite dependent successor relation rather than
pretending every labor and pension choice is independent:

```runa
? explore household_reallocation {
    from {
        before = current_household
        context in candidate_household_actions(before, planning_limits)
    }

    to after = apply_household_action(before, context)

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
        planning_limits.resource_tolerance
    )

    results tradeoffs {
        group all
        measure [
            disposable_ore = household_disposable(after),
            spouse_hours = after.spouse.hours,
            own_hours = after.self.hours,
            spouse_pension_ore = after.spouse.pension_ore
        ]
        select [action = context, after, disposable_ore]
        choose pareto [
            maximize disposable_ore,
            minimize spouse_hours,
            minimize own_hours,
            maximize spouse_pension_ore
        ]
    }

    mechanisms for view tradeoffs chosen from assess_household_plan
}
```

This query can expose the trade-off frontier; it cannot infer what the couple
“should” prefer. The one-sided floor permits plans that improve resources while
rejecting plans more than the stated tolerance below Before. Candidate actions
derive earnings from hours and wage assumptions, conserve declared household
transfers, assign pension payer and ownership explicitly, and do not vary
correlated amounts independently. Materially different payment or transfer
paths remain in Context even when they reach the same After value. Feasibility
and whether objectives are lexicographic, weighted or Pareto are explicit parts
of the question. General dependent relations and Pareto result views are design
goals, not current compiler capabilities. `choose pareto` is set-valued: it
returns every nondominated selected row. A member observed during an open search
is provisional; only group closure can prove that no unseen case dominates it.

Every `mechanisms` clause names both its target and an explicit canonical
endpoint observation. `for selected` means every case selected by `find` before
view processing. `for view NAME chosen` waits for that view's closure, targets
its chosen rows, and seals the referenced `ViewId` into mechanism-request
identity. Presentation fields and measures do not infer the observation root.
Here `assess_household_plan` must expose the modeled resources, hours, pension
and tax dependencies needed by the question and objectives. A future
convenience inference is possible only after its normalization is specified; it
is not implicit in this design.

The named observer resolves to one checked pure callable of shape
`(State, Context) -> Observation`, evaluated independently at Before and After
and total over the declared endpoint scope. Its reachable rule and call closure,
not every theoretical value of those types, is sealed into mechanism-request
identity. Failure to prove that contract makes mechanism evidence unavailable;
it does not silently fall back to tracing selected display fields.

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
                       (future until endpoint replay is implemented)
```

The search decision structure is a DAG. Each dynamic mechanism occurrence
structure is a DAG. The role-neutral state-transition graph is not necessarily
acyclic: general queries may contain self-edges or both `A -> B` and `B -> A`.
Call it a transition or case graph unless a monotone rank proves acyclicity.
The layered evidence incidence `CaseId -> TransitionId -> MechanismSignatureId`
is acyclic even when the state graph is not.

Different cases may denote one semantic transition, and different transitions
may share one mechanism. For a fixed question and observation request, exact
support retains the incidence triple—or equivalent exact fibers—rather than
only a bare transition-to-signature edge. One result group may contain several
mechanisms; one mechanism may span several groups, profiles, incomes, loss
values and disconnected regions. Neither a graph's node count nor a displayed
row count substitutes for a population count.

For the broad income-cliff result, report at least four distinct populations:

- matching profile-by-income transition cases;
- distinct semantic transitions;
- distinct affected profile configurations, because one profile configuration
  may support several cliffs; and
- distinct complete differential mechanism signatures.

In the proposed income view, `before.profile` is a declared canonical product
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
complete, request-relative normalized signature roots—or a separately declared
coarser authored family—not raw mechanism-DAG node count or a shared rule name.
One case may traverse several shared mechanism atoms, so atom supports overlap
and are not additive. These configuration counts are model-space support, not
population estimates.

The seven current case and transition populations remain useful:

- `U_D`: declared generator cases;
- `U_C`: constructible transition cases;
- `U_T`: distinct declared transitions;
- `D_C`: admissible transition cases;
- `D_T`: distinct admissible transitions;
- `M_C`: matching transition cases; and
- `M_T`: distinct matching transitions.

The current fixed-product slice knows `U_D` exactly when the run opens. A
general dependent relation also reports `U_S`, its distinct canonical source
rows, and a source/successor-enumeration frontier. If every relation exposes
exact cardinality and order statically, `U_S` and `U_D` are exact at open;
otherwise their observed values are lower bounds until that enumeration
frontier closes. Content-stable source and successor keys keep already emitted
`CaseId` values unchanged. `U_C`, `U_T`, `D_C`, `D_T`, `M_C` and `M_T` become
exact only after their own required frontiers close.

The current names use `M` for the selected matching or violating polarity. The
general algebra should call these `S_C` and `S_T`: for `find all`, the selected
relation is the admissible relation, so `S_C = D_C` and `S_T = D_T`. Mechanism
counts have their own incidence frontier. With no confirmed mechanism evidence
yet, the honest count may be unknown rather than zero.

Intermediate extrema require directional honesty too: an observed maximum is a
lower bound on the final maximum, an observed minimum is an upper bound on the
final minimum, and group winners or Pareto frontiers remain provisional until
their required relation and view frontiers close.

A 50-DKK mechanism-bin view counts distinct dynamic signatures having support
in each loss interval, not the number of cases in that interval. The same
signature may occur in several bins, so bin counts need not sum to the global
mechanism count. A bin is exact only when selected-case membership, complete
signature incidence, loss measurement and bin membership have all closed. A
cap can justify “at least N”; it never proves infinity. Every support inside a
finite Explore world is finite.

Support may be `lower_bound(n)` while its incidence frontier is open,
`at_least(c)` when an actual support-counting cap saturates, or `exact(n)` when
counting closes. A cap on retained examples alone does not degrade an exact
scalar support count: the report may retain only `c` examples while still
counting every incidence. Report the number of counting-capped signatures
separately. Thus the useful summary is “X signatures, Y classified cases, Z
signatures with censored support,” not “Z infinite mechanisms.” A cap must not
stop signature assignment for later cases; if it does, signature count and
incidence remain open rather than merely censored.

### What a finished answer publishes

A paused or sealed exploration bundle should make the result speak at three
resolutions:

- a closure-aware summary of declared, constructible, admissible and selected
  source rows, cases and transitions, affected `ProfileKeyId` values, complete
  mechanism signatures, and saturated-support signatures;
- named relational views such as `cliffs`, `affected_profiles` and
  `mechanism_loss_bins_50_dkk`, each with its own exactness, schema, grouping
  key and privacy authorization; and
- the authenticated journal/evidence root plus typed snapshot and JSON export
  handles needed to resume a paused run or audit and analyze either form.

An eventual report can therefore honestly say “Y concrete profile-transition
cases across P profiles share X complete mechanisms; Z signatures have support
of at least c because their materialization cap was reached,” followed by an
exact or lower-bound 50-DKK histogram and links to the corresponding case and
incidence views. JSON is a reproducible materialization of named evidence, not
the authority from which the graphs are reconstructed.

### Experimental implementation boundary

Keep proposed language direction separate from executable evidence:

| Surface | Status on the feature branch |
|---|---|
| Typed finite Context/Before/After transitions over the currently supported exact-finite subset; macOS-supervised single-worker durable pause/resume; case and transition counts; search decision DAG; semantic transition graph | Implemented Experimental slice |
| Checked `observe mechanisms with CALLABLE` declaration | Parsed and sealed; endpoint replay is not executed |
| Relational `from`/`to`/`results`, inferred transition categories, `find all`, dependent finite successors, separate view identity, and removal of the probe lifecycle | Proposed next contract |
| Mechanism replay and DAG, exact signature incidence, Pareto views, arbitrary saved-graph queries, symbolic/SMT closure, and broad multidimensional Personskat closure | Not implemented |

The existing fixed Cartesian axes give every case a straightforward canonical
ordinal. A dependent relation must replace that assumption with canonical
source and successor keys, explicit enumeration closure and exact support
mapping before it can replace the current generator. This is a real IR and
search-DAG design task, not syntax sugar.

### Final pieces to lock before the broad run

The architecture now has a strong center, but the following pieces must be made
coherent before a nationwide through-1,500,000-DKK exploration is a sensible
execution target:

1. Center the RFC, IR and compiler surface on one typed finite successor
   relation; remove public probes, infer transition properties and add
   `find all`.
2. Give coherent profile relations and dependent successors canonical schemas,
   identities, cardinalities, order and lineage.
3. Separate relation, mechanism, view, durable-evidence and operational layers
   while preserving explicit privacy and retention authorization; make
   grouping, measures, all-ties choice and Pareto choice deterministic.
4. Replay explicit canonical endpoint observations and publish exact
   case/transition/signature incidence.
5. Close large cells by proving regional transition images with relevance,
   affine, interval, congruence and later SMT certificates, then allow new
   authorized views over the saved evidence without reclassifying its world.

The next experiments should use small but genuinely multi-dimensional complete
profile relations, not another single fixed person or only the already-known
341,500-DKK, 60-km and 130-km anchors. Widen income and profile dimensions while
checking what the observable run discovers and which frontier remains. A
dimension with no encoded effect should close as irrelevant rather than being
forced to produce an interesting answer.

There is therefore no planned 1.5-million-income execution merely to obtain an
early number. The milestone is a profile-and-income relation plus proof-closure
experiment. The desired signal is that widening the declared world adds a
small number of source events, mechanism signatures and certified cells rather
than requiring one full Personskat evaluation for every krone-profile pair.

Residual work is admitted and reserved beneath independent operational ceilings
of 80 percent of installed CPU and 80 percent of physical RAM, with host-pressure
trips free to reduce capacity or pause dispatch. These are safety policies, not
hard instantaneous quota guarantees. Limits, worker count, timing and scheduling
order are invocation facts; they never define a different bounded question.

## Reading the result responsibly

The hand-written audit is executable evidence about the checked-in Futuruna
model and its narrow declared domain. The proposed broad exploration becomes
evidence only after its surface is implemented and its declared world is
actually run or exactly closed. Neither is an individual tax determination.
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
