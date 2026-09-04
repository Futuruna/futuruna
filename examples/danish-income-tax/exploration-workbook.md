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

The **target language** now makes that architecture visible: `given` states
conditioning, `vary` introduces a finite dimension, `let` constructs local
values including Before, and `transition` declares the typed After relation.
`derive` names pure computed facts and endpoint observations.
A total `admit` decision classifies every constructed transition. Zero or more
named `find` relations then ask semantic questions without creating a
privileged relation called `selected`. Views, partitioned choices and causal
explanations live in separate `? analyze` declarations, while `? publish`
attaches authorized artifacts. Time limits, workers,
memory limits, journal locations and publication paths remain invocation
controls rather than query semantics.

> **Syntax status.** Named `find` declarations and explicit
> `results ... from find NAME`, `mechanisms ... from find NAME`, and
> `mechanisms ... from view NAME chosen` consumers are now carried through the
> Experimental frontend and durable runtime on this branch. The separate
> `? analyze` and `? publish` blocks in section 9 remain accepted **target
> syntax**, not a claim that every sketched clause is executable today. The
> collection audit below is executable ordinary Futuruna. No Experimental
> surface here is yet a compatibility promise.

The first closed plural stream is now concrete evidence rather than a design
sketch. The four-case `relational-explore-stream-smoke.runa` run used one
relation and admission for `find all_cases = all` and
`find interesting = matches of ...`. After a deliberately short cold slice
paused at sequence zero, the same journal resumed and closed with
`all_cases = exact(4)`, `interesting = exact(2)`, one structural mechanism
explaining both interesting cases, and fourteen caught-up artifacts. Its saved
`views/selected_cases.ndjson` contains the two typed cases;
`views/mechanism_summary.ndjson` reports `mechanisms = 1` and
`explained_cases = 2`; and `graphs/case_graph.ndjson` closes with five state
nodes, four universe/admitted cases, and question-specific matched layers of
four and two cases. This is a kernel audit, not a Personskat result.

The chosen-target dependency is also executable rather than sketched. The
four-case
[relational-explore-chosen-mechanism-smoke.runa](../relational-explore-chosen-mechanism-smoke.runa)
result chose the two tied maximum-score cases (`before = 2` and `before = 3`),
admitted exactly those two CaseIds into `winner_path`, and closed with one
shared structural mechanism.
Its downstream grouped result is exactly `{ cases: 2, mechanisms: 1 }`; all
declared artifacts caught up to the final journal head. A focused crash test
paused after the first Choice candidate while its FIND input was still open,
reopened the identical frontier, and resumed to the same exact closure. The
scheduler addresses the independently authenticated Choice relation
incrementally by member ordinal; it does not wait for, rematerialize, or copy a
display result before mechanism replay. This remains a kernel audit, not a
Personskat result.

The compact answer is now shaped for the policy result rather than only for
engine inspection. Report v11 and publication v19 list every named result,
including an ungrouped configuration ledger and an exact-empty view, with its
resolved input, row grain, ordered typed columns, group keys, exact or open row
count, evidence roots, and resumable NDJSON path. The manifest joins those
views to their FIND and mechanism identities without guessing from names and
without embedding the population. A completed Personskat run can therefore
state the exact cliff-case and structural-mechanism counts, show the grouped
50-DKK loss-bin rows, and point to the complete saved configurations from one
bounded index. This is output-contract readiness, not a new Personskat result.
Reopening an already closed ordinary stream now returns `complete` before the
time-limit or host-permit loop, with zero new semantic events; any retained
certified source-summary proof still receives its required checked runtime
rebind before that shortcut is allowed, and every retained explicit support
reader must have finished backfill, dirty-prefix observation and sealing.
An exact-empty result still has a real owner-only zero-byte NDJSON file at its
advertised path, so `caught_up` never disguises an absent configuration
artifact.

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
mechanism landscape. The present mechanism request explicitly replays the
`cliff_cases` find only. Interesting matching edges can seed finer subrelations without
forcing the first stream to pay for every krone; admission changes and
mechanism boundaries need their own discovery signals. A finer successor step
is a separately checked relation with a different identity; coarse evidence
can nominate its bounds but does not count as its coverage unless a future
checked bridge proves that transfer.

The current-contract execution of this target closed exactly on 2026-09-04:
all 2,000 transitions were admitted and classified, none satisfied
`cliff_cases`, and the selected mechanism and 50-DKK loss-bin layers therefore
closed at exact zero. This is the first Personskat result produced by the
current first-class `? explore` path. Its declared profile and source-coverage
gaps remain part of the answer, so the result is not generalized beyond this
coarse conditioned relation. Selected identities and timings are recorded in
section 9 below; the external manifest is the complete index of artifact and
evidence roots.

An earlier version of the same file declared all 200,000 adjacent 1-DKK
transitions. Its durable prefix remains useful performance evidence, but the
new 100-DKK relation has a different checked identity and must use a fresh run
state. The pipeline now has classified slices, sparse selected-case
realization, proof-specialized source results and a query-bound compiled
residual classifier; the coarse audit is the first Personskat target for that
complete path.

The newly chosen **first broad audit** is another separately identified
relation. For every profile in its declared coherent profile world it ranges
over lower salaries `0, 1_000, ..., 1_499_000 DKK` and compares each with its
`+1_000 DKK` successor through exactly 1,500,000 DKK. That is **1,500 edges per
profile** whose endpoint evaluator can reuse **1,501 endpoints per profile**.
Closing it would answer the coarse endpoint and mechanism questions exactly
for that grid; it would not enumerate or certify the 1,500,000 adjacent 1-DKK
transitions per profile. No run has started, no result count exists, and the
milestone is not a claim that the target Explore/Analyze/Publish syntax is
currently executable.

The current architecture also has its first proof-first stream seam. For each
next canonical child, a producer-owned certificate may close a whole
zero-selected region before the concrete classifier touches its members. The
certificate is bound to the exact checked classification capsule and durable
journal authority; unsupported, mixed or selected regions fall back to the
existing concrete sweep. This is a general Explore optimization, not a
Personskat-specific shortcut, and it does not by itself claim that the present
Personskat capsule can normalize every tax expression.

Most importantly, the proof subject is not “an income interval” standing in
for a profile. The case child remains a mapped case population, while a
separate correlated-starter-region identity binds the same exact coordinate
slice through source assignments, the constructed `(Context, Before)` row,
its singleton successor fiber and the final case image. It may avoid eagerly
materializing those starter rows, but it never widens their fields into an
independent box. This preserves the architecture needed for later mechanism
support: starter conditions and conditional successor fibers remain distinct
from raw case counts even when a scalar income axis makes their cardinalities
equal in this narrow slice.

The complementary all-admitted mechanism question is still authored as the
focused fixture
[personskat-mechanism-landscape-200k.explore.runa](personskat-mechanism-landscape-200k.explore.runa).
It names `find admitted_cases = all` and explicitly targets that find for
mechanism replay. This separate file remains useful as a narrow fixture, but it
is no longer the desired broad-audit topology. The combined audit should name
both `cliff_cases` and `admitted_cases` in one Explore declaration: relation,
endpoint and admission evidence are then generated once, while each find keeps
its own `QuestionId`, closure and downstream mechanism target.

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

The first broad execution milestone asks the same kind of comparative question
at 1,000-DKK resolution over the declared `0..1,500,000 DKK` horizon. It is
purposefully a mechanism-and-endpoint survey before an exhaustive one-krone
cliff proof. A mechanism or net-loss change can nominate a finer separately
identified query, but coarse coverage never transfers silently to that finer
relation.

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
coarse edge in the named cliff finding is a useful refinement neighborhood and
mechanism witness. An edge outside that finding does not prove that no 1-DKK subedge inside
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

The focused executable landscape fixture still uses a separate
`find admitted_cases = all` question, where positive membership equals
admission by definition. That file remains useful as a narrow calibration,
but separation is no longer an engine requirement: one Explore declaration
can name both `find cliff_cases = ...` and `find admitted_cases = all`, then
explicitly explain either relation. Each target receives its own question seal
and durable materialization over the shared relation and admission journal.

Calling the first query “mechanism discovery” without this qualification would
be misleading. A mechanism request explicitly targeting an exact-empty cliff
find produces exactly zero requested mechanisms; it does not prove that the
interval contains no legal mechanism change. A sibling all-admitted find can
request that landscape in the same shared traversal. The efficient next design reuses
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
explicitly reported model-coverage limit. The coverage schema does not itself
mint that proof: until an exact irrelevance producer supplies a certificate,
an already declared fact remains in support and an unmodeled or unproved path
is an explicit gap. Search cost alone is never a reason to turn a dimension
into a hidden constant.

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

This section records the accepted Experimental language direction. Its source
blocks use the target grammar described above; they are not accepted by the
current parser and are not evidence that an evaluator has executed or closed
the declared world. The evolving normative contract and current implementation
checkpoint remain in
[Bounded Rule Exploration with `? explore`](../../docs/rfcs/bounded-rule-exploration.md),
its [implementation workbook](../../docs/rfcs/bounded-rule-exploration-workbook.md),
and [feature stages](../../docs/feature-stages.md).

### One finite successor relation

Explore is best understood as a provenance-aware relational query over typed
state transitions:

```text
given conditions + finite varied dimensions + derived Before
    -> typed transition -> finite dependent After states
    -> total admission decision
    -> zero or more named question relations
    -> separate views, partitioned choices, and explanations
```

The general transition contract is:

```text
successors(context, before) -> finite set of after states
```

The algebra fixes the semantic order without imposing a batch execution order:

```text
R          = distinct source rows produced by from { given/vary/let }
C_R        = { (context, before, after) | (context, before) in R,
                                           after in transition(context, before) }
D_A        = { case in C_R | admission_A(case) }
Q_n        = { case in D_A | named_find_n(case) }
V_case     = a named relational view over one Q_n
K_(Q,p)    = a choice proved within each declared partition p of Q_n
M(target)  = differential signature incidence for a named find or choice
V_evidence = a named relational view over the incidence relation produced by M
```

The target identity ladder gives the relation, admission, each question, each
analysis node and each explanation request separate content identities. The
current implementation's corresponding names include `RelationId`,
`AdmissionId`, `QuestionId`, `ViewId` and `MechanismRequestId`. Cases must exist
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
does not. The checked planner can recognize its currently supported affine
structure, and a future typed adapter can contribute source events, without
making `boundaries` the semantic definition of the query.

The target relational spelling is deliberately explicit about which values are
conditioned, enumerated and derived:

```runa
? explore personskat_income_cliffs_2026 {
    from {
        given tax_year = 2026
        given salary_step_kroner = 1

        vary profile in coherent_personskat_profiles(
            tax_year,
            PersonskatProfileSpace(
                municipalities = supported_municipalities(tax_year),
                church_tax_statuses = supported_church_tax_statuses(tax_year),
                households = supported_household_profiles(tax_year),
                commutes = supported_commute_profiles(tax_year),
                income_compositions = supported_income_compositions(tax_year),
                pensions = supported_pension_profiles(tax_year)
            )
        )
        vary gross_salary_before_kroner in supported_income_coordinates(
            profile,
            range(0, 1_500_000)
        )

        let context = SalaryChange(amount_kroner = salary_step_kroner)
        let before = personskat_state(
            tax_year,
            profile,
            gross_salary_before_kroner
        )
    }

    transition after = apply_salary_change(before, context)

    derive policy_assessment {
        before = assess_personskat(before, context)
        after = assess_personskat(after, context)
    }
    derive resources_before_ore = policy_assessment.before.resources_ore
    derive resources_after_ore = policy_assessment.after.resources_ore

    admit supported when policy_assessment.before.supported
        && policy_assessment.after.supported
        && salary_change_permitted(before, after, context)

    find cliffs = violations of (
        resources_after_ore >= resources_before_ore
    )
    find admitted_edges = all
}

? analyze personskat_income_cliff_answer
from explore personskat_income_cliffs_2026 {
    view admitted_summary from find admitted_edges {
        group all
        aggregate [cases = count_distinct(case_id)]
        select [cases]
        updates closure_gated
    }

    view cliff_cases from find cliffs {
        each case
        measure [loss_ore = resources_before_ore - resources_after_ore]
        select [
            case_id,
            profile = before.profile,
            gross_salary_before_kroner = before.gross_salary_kroner,
            gross_salary_after_kroner = after.gross_salary_kroner,
            loss_ore
        ]
        updates monotone
    }

    view cliff_summary from find cliffs {
        group all
        aggregate [
            cases = count_distinct(case_id),
            starters = count_distinct(source_key),
            affected_profiles = count_distinct(before.profile)
        ]
        select [cases, starters, affected_profiles]
        updates revisable
    }

    explain cliff_paths from find cliffs using policy_assessment

    view mechanism_summary from explain cliff_paths {
        group all
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            explained_cases = count_distinct(case_id),
            explained_starters = count_distinct(source_key)
        ]
        select [
            mechanisms,
            raw_signatures,
            execution_profiles,
            explained_cases,
            explained_starters
        ]
        updates revisable
    }

    view mechanism_loss_bins_50_dkk from explain cliff_paths {
        group by [
            bin_start_ore = floor_to_bin(
                resources_before_ore - resources_after_ore,
                5_000
            )
        ]
        aggregate [
            mechanisms = count_distinct(structural_mechanism_id),
            raw_signatures = count_distinct(signature_id),
            execution_profiles = count_distinct(execution_profile_id),
            cases = count_distinct(case_id),
            starters = count_distinct(source_key)
        ]
        select [
            bin_start_ore,
            mechanisms,
            raw_signatures,
            execution_profiles,
            cases,
            starters
        ]
        updates revisable
    }
}

? publish personskat_income_cliff_evidence
from analyze personskat_income_cliff_answer {
    emit view admitted_summary
    emit view cliff_cases
    emit view cliff_summary
    emit view mechanism_summary
    emit view mechanism_loss_bins_50_dkk
}
```

`given` is visible conditioning and `vary` is finite enumeration, possibly
dependent on earlier bindings. `let` introduces a pure local abbreviation and
`derive` computes a typed fact; neither silently adds a dimension. The
`transition` clause owns the intervention and its finite successor fiber.
For every constructed transition, `admit` must terminate with one total typed
Boolean decision. A false decision is a reported rejection. A semantic failure
to produce either Boolean is an integrity error, selects nothing, and prevents
exact admission closure; cancellation or resource pressure instead pauses with
the affected frontier still open. Neither route is a filtered-away case.

`find cliffs` and `find admitted_edges` are ordinary named question relations.
There is no implicit `selected` relation. This lets one declared world support
both the cliff question and an admitted-edge mechanism landscape without
changing relation identity or pretending harmless edges are cliffs.

The separate `? analyze` declaration consumes those named semantic relations.
Its `view`, `choice` and `explain` nodes form a typed analysis DAG but do not
change which cases exist or satisfy a finding. An explanation request names a
finding or a choice plus a checked observer. It never requires an author to
paste a post-run raw mechanism hash back into semantic Explore source. A
discovered identity may be used by an external inspection or publication
request, where it is an operational address rather than part of the question.

The final mechanism-bin view is downstream of endpoint replay and quotient
assignment. `mechanisms` counts distinct `structural_mechanism_id` values;
`raw_signatures` and `execution_profiles` retain execution-sensitive
populations separately. An explanation-incidence row preserves the complete
resolved find/choice row—including its addressable Explore derives—and adds
`transition_id`, raw `signature_id`, `structural_mechanism_id` and
`execution_profile_id`. The inherited case row already carries `case_id`,
`source_key`, Context, Before and After. Exact closure commits both the raw
incidence and structural quotient roots, so
`count_distinct(signature_id)` remains a raw-signature histogram rather than a
mechanism histogram.

`aggregate count_distinct(...)` is a closed-group reducer; unlike retained
examples, it cannot claim exactness until its raw incidence, structural
assignment and declared result-input frontiers have closed.

`coherent_personskat_profiles(...)` is a dependent relation, not an
instruction to cross those catalogs blindly. It joins and derives them into
whole typed profile rows. The second `vary` is genuinely lateral:
`supported_income_coordinates(profile, ...)` may return a different finite
set for each profile because income composition, hours, pension facts or model
support can constrain the meaningful salary coordinates. Each resulting
`before` pairs one coherent profile with one supported lower endpoint. A
materialized list is a sound first implementation when it exposes stable
schema, finite closure, canonical order and lineage. The target relational IR
should retain producer dependencies so the decision structure and relevance
analysis can share evaluation or compress equal behavior without dropping any
declared profile column, SourceKey or support count.

Source bindings are ordered. `given` contributes one source-independent value
from sealed module facts or pure helpers. `let` contributes one value derived
from earlier source bindings, and `vary` performs a dependent finite expansion
in the style of SQL `LATERAL`. `vary` and `let` can see only earlier bindings;
`given` cannot depend on another binding in the same `from` block. The block must
ultimately bind one semantic `context` and `before`; the transition binds one
typed `after` per successor and retains every meaningful action value in case
identity.
Auxiliary bindings remain authenticated construction lineage, not extra hidden
fields. Several independent `vary` bindings do form a product, but
coherent-profile helpers let the author express a join instead of generating
nonsense combinations and filtering them afterward.
The closed IR resolves local binders to ordered indices, so alpha-renaming an
auxiliary spelling such as `profile` does not rename an otherwise identical
`RelationId`.

There is no Personskat-only producer primitive. Any checked pure expression of
an exact-finite collection type can appear after `vary ... in`. The closed IR records its
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

Nor is the name `coherent_personskat_profiles` itself proof that the query
is broad. The checked exploration bundle needs `RelationId`-scoped source and
successor coverage components. Source coverage audits the `from` helper's
reachable producer closure and recursively names Context and Before field
paths and every reachable immutable producer input as one of: varied finite
dimension, derived fact, explicit conditioning, proof-backed exact
irrelevance, or reported coverage gap. A Copenhagen literal or immutable
top-level constant buried in `personskat_state_2026` therefore appears as
conditioning even though it was not written as `given`. An
unsupported nested path remains an explicit gap; coverage never fills it
by inventing a dimension. It is generated from the ordinary checked program
and does not require authors to repeat the profile schema in a second
Explore-only language.

Successor coverage separately audits transition-block dependencies and every
After field. Coverage of facts consumed by admission and named finds requires
sibling AdmissionId/QuestionId components; each view, choice or explanation
observer gets one generic AnalysisNodeCoverageId for only the dependencies it
adds. Editing one consumer must not rewrite source or successor facts. A result
CoverageBundleId composes exactly the components reachable from that result.
Until an exact irrelevance producer exists for a path, this workbook makes no
claim that the mere availability of an `exact irrelevance` category proves the
path irrelevant.

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
the validity of an already constructed case, state them as `given` inputs to
the finite producer, for example:

```runa
given municipality = Municipality.Copenhagen
given church_tax_status = ChurchTaxStatus.NotMember
vary profile in coherent_personskat_profiles_where(
    profile_space,
    municipality = municipality,
    church_tax_status = church_tax_status
)
```

That source restriction changes `RelationId` because it declares a smaller
finite world. By contrast, `admit` classifies every constructed transition and
belongs to `AdmissionId`. Optimizers may push a safe admission predicate into
producer execution, but that physical shortcut must not change either identity
or its admitted/rejected population counts. Without explicit source
conditioning, the relation ranges over the whole declared coherent profile
relation; there are no hidden “fixed profile facts.” Conjunct order and repeated
identical terms remain available for diagnostics but normalize away from
`AdmissionId`; resolved total predicate semantics define the admission.

In this target snippet, the end-exclusive source range and
`salary_step_kroner = 1` supply lower salary endpoints through 1,499,999 DKK and a final successor
at 1,500,000 DKK. That spells the eventual exhaustive one-krone question; it is
not the planned first broad run. The broad milestone has its own RelationId,
uses lower endpoints `0, 1_000, ..., 1_499_000 DKK`, and applies a
`+1_000 DKK` transition, yielding 1,500 edges and 1,501 reusable endpoints per
profile. `transition` constructs either declared comparison. `admit`
distinguishes rejected cases and integrity failures from admitted nonmatches.
Named `find` clauses state the questions. `view cliff_cases` projects one
question relation, and `explain cliff_paths` names the endpoint computation
whose Before/After executions are compared. The downstream loss-bin view waits
for that explanation's incidence relation. No broad milestone run has begun,
and this target spelling is not presented as currently executable syntax.

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
3. Seal total admission in `AdmissionId`, then seal each named find under its
   own QuestionId. Classify every discovered case independently of
   presentation. Changing only admission or a question does not rename the case
   universe. A complete Explore declaration with no analysis is valid.
4. In a separate `? analyze`, materialize zero or more named view and
   partitioned-choice projections without changing cases or classifications.
5. Replay an explicitly named explanation request against a named find or
   the proven members of a named choice. The request name is an address, while
   its semantic identity comes from its target, observer and normalization
   contract.
6. Materialize zero or more explanation-incidence views. Exact
   distinct-signature aggregation waits for that explanation frontier to
   close; it never feeds back into its own target.

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
authoritative journal and exact open frontier, every enabled candidate or
proof producer joins the same dependency-linked work graph. Current v1
actively dispatches regional certificates and concrete/classified fallback,
and now orders the fixed canonical chunks candidate-first. Its live candidate
inputs are the lower and upper finite-range endpoints plus boundaries from
direct checked affine admission/FIND guards. Those source coordinates reach
the case partition only through a conservative exact single-axis lift whose
coordinate interval matches a bare or product-factor case interval. Both
adjacent sides of an interior boundary are nominated. Every other canonical
chunk remains implicit residual exact work, so candidate exhaustion is never a
completeness claim.

Source-event cuts and proof-certificate piece boundaries or authorized
midpoints remain typed future producer seams. The current stream does not
populate either source, and its live proof-strategy call supplies no
certificate obligation. Product-rank and plural-axis shapes therefore use
endpoint-first plus canonical residual order rather than guessing a lift.
The regional prover can still certify an already selected canonical child; it
does not currently mine that proof for another candidate boundary. Likewise,
Personskat helper/rule calls are not direct affine guard atoms, so without a
checked rule-graph normalizer they contribute no hidden tax-threshold
nominations—the honest order is range endpoints followed by residual chunks.
Completed out-of-order chunks occupy durable sparse ordinal slots, allowing a
selected run and its mechanism work to emerge before lower ordinals finish. A
separate bounded checkpoint promotes only the next occupied ordinal into the
contiguous root cursor; cold resume first continues any authenticated partial
concrete slice, then skips accepted slots and reconstructs the same candidate
and residual order.

The result and mechanism lanes consume those sparse selected cases today. The
current flat case/support artifact still publishes only its contiguous promoted
prefix; the intended open case graph needs stable-key sparse updates before it
can expose later chunks without reordering already appended records. Exact
closure is already canonical and remains gated on the complete prefix.

The dependency is on content readiness, not producer closure. As soon as one
coherent profile transition is yielded, its immutable CaseId readiness token
can unlock admission, FIND, a case view and mechanism replay even while that
profile's successor enumerator—or the wider profile producer—remains open.
This is how Carl's and John's cases can emerge and converge on a shared
mechanism during one observable stream instead of waiting behind a hidden
enumerate-everything phase.

The implementation scheduler now follows that contract directly. At each
durable base prefix it catches selected-case result evidence, post-mechanism
incidence-result evidence and direct FIND-target mechanism work up to the
currently known discovery ordinals, then grants one more base quantum. Those
consumers can stream while FIND remains open. A chosen-result mechanism target
instead waits for its choosing result to become exact and published, then
walks that immutable projection in bounded chunks; each target event binds its
CaseId to the exact projection ordinal and the target seal binds the result
root. The ordinals are replay-built scheduling indexes only—CaseId remains a
content hash and answer roots remain arrival-order independent. Exact FIND
closure is required only where a downstream operation actually needs an exact
input, not as a universal gate in front of useful evidence.

Execution, semantic certainty and artifact availability are orthogonal:

```text
execution.status:       running | paused(reason) | stopped(reason)
semantic.status:        open | exact | unavailable(reason) |
                        error(reason)                             (per relation)
materialization.status: not_requested | pending | caught_up |
                        capacity_limited | unmaterialized | error (per artifact)
count.status:           unknown | lower_bound(n) |
                        interval(lower, upper) | exact(n)
```

A time limit, resource pressure or user interruption pauses after the latest
committed evidence and carries a ResumeCursor. `stopped(reason)` carries no
continuation capability: all requested obligations are terminal, explicitly
abandoned, or ended by an unrecoverable integrity failure. Resuming a paused
run continues from the same frontier. The scheduler
may always choose the most informative ready node, and it may revisit source
analysis as newly closed regions expose new opportunities. There is no authored
`probes` block, no probe-complete semantic state, no `--pause-after probes`, and
no probe plan in query identity.

There is no ambiguous global `partial` or unqualified `complete`. A named
finding may be semantically exact while one explanation remains open and a
JSON artifact is still catching up. Conversely, publication can be caught up
to a paused evidence prefix while the semantic answer remains open.
`stopped(reason)` says why no worker is continuing; it does not turn an
unavailable proof into an exact answer. Every public count carries its own status, and every report
states the declared universe to which that status applies.

The report also carries an opaque EvidenceToken committing the JournalContract,
semantic graph, coverage and canonical evidence/projection roots for one
cohesive cross-artifact read. It excludes journal head, sequence and work position. A
separately typed resume cursor identifies the next operational work position;
neither token is accepted where the other is required. The cursor is
operational state; the EvidenceToken is semantic read authority. Neither is
part of `RelationId`, a question predicate or structural mechanism identity.

Scheduling policy and each scheduling decision are observable operational
provenance. Every emitted coordinator batch starts with an authenticated
policy-versioned reason checkpoint that fingerprints the complete ordered work
batch. It may survive alone as an attempted dispatch after a crash; it says
exactly what was selected without claiming that work completed, and resume can
select the still-open work again. It changes the journal head, not the semantic
evidence roots or declared answer. The v0
Cartesian/probe executor and its CLI, codec, snapshot and resource subjects
have been removed rather than retained as a second lifecycle. Opening its
`run-opened`/`fence-v1`/`blob-v1`/`event-v1` state namespace now fails closed
and directs the operator to a fresh run directory; it can never be mistaken
for an empty relational journal.

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
authenticated head. Distributed workers, if used, receive canonical disjoint
chunks of the same checked query and return query-bound evidence chunks. The
coordinator verifies their coverage and commitments, then merges them
idempotently into the same evidence root regardless of arrival order. Workers
do not mine cases, choose between competing chains or run consensus.

### SQL-like views over graph-backed evidence

Explore should borrow SQL's separation of relational stages without inheriting
SQL's bag semantics, null rules or nondeterministic limits:

| Explore concept | Relational role |
|---|---|
| `given NAME = EXPR` | visible conditioning in the declared source world |
| `vary NAME in FINITE_RELATION` | a finite, possibly dependent source or successor dimension |
| `let NAME = EXPR` | pure local abbreviation without added cardinality |
| `derive NAME = EXPR` or paired endpoint block | typed pure computed fact without added cardinality |
| `transition after = EXPR`, `after in FINITE_RELATION`, or an ordered block with one `yield` | finite successor relation and intervention |
| total `admit NAME when PREDICATE` | admitted/rejected classification of every constructed case |
| `find NAME = all/matches/violations` | one named semantic question relation over admitted cases |
| `? analyze ... from explore ...` | a separate typed result/explanation DAG |
| `view NAME from RELATION` | grouped or row-grain projection over a named relation |
| `partition all/by` | explicit comparison population for a choice proof |
| `choice NAME from find NAME` | partitioned argmin, argmax, all-ties or Pareto relation, independent of display views |
| `explain NAME from find/choice NAME using DERIVATION` | differential-signature incidence relation |
| `each case` | one logical row per finding `CaseId` |
| `each incidence` | one logical row per mechanism-incidence triple |
| `group by` or `group all` | closed groups over either input relation |
| `measure` | named exact per-input-row scalars |
| `aggregate` | closed-group reducers; currently `count_distinct` |
| `having varies(NAME)` | retain a closed group/choice partition only when its named measure has at least two distinct exact values |
| `select` | public projection and privacy allow-list |

The closest SQL analogy for a `vary` inside `transition` is a `LATERAL` join or
`CROSS APPLY`: the finite successor dimension is evaluated for each source row
and may return a different number of rows. Dependencies stay visible in source
order rather than disappearing inside an apparently independent Cartesian
product. `view` blocks are named `SELECT`-like projections; `choice` states the
comparison proof explicitly, and `explain` is the provenance layer ordinary
SQL does not provide.

Named views, choices and explanations form a typed dependency DAG after the
semantic Explore declaration. A view consumes a named find, choice or
explanation-incidence relation; a choice consumes a find directly; and an
explanation consumes a find or choice. Names resolve to semantic IDs, and the
checker rejects cycles. Structural-mechanism histograms genuinely
depend on quotient assignment; they cannot be faked by renaming a
raw-signature aggregate.

Finding inputs use `each case`; explanation inputs use `each incidence`.
Either may use `group all` or `group by [...]`. Mismatching row grain and input
relation is a type error, and an exact aggregate or choice requires closure of
its declared input. A `partition` is not a display group: it identifies the
complete comparison set within which minimum, maximum, all-ties or Pareto
membership is proved.

Entries in `group by`, `measure` and `select` use `name = expression`; a bare
`name` is shorthand for `name = name`. The implemented closed aggregate form is
`name = count_distinct(expression)`. Group, measure and aggregate declarations
introduce unique intermediate names. Select output names are also unique, while
an earlier intermediate may be projected by using the same bare name in
`select`. Measures are evaluated per input row in declaration order, aggregates
consume closed groups, and later view clauses can refer to earlier names without
making evaluation order or alias resolution implicit.

The superseded `output.key` form hid a semantic `GROUP BY` inside presentation.
The target `view`/`choice` surface makes grouping, measurement, aggregation,
comparison population and cardinality policy explicit. A named `find
alternatives = all` relation is equally important: “which municipality minimizes
tax?” is an optimization over admissible alternatives, not an artificial
always-true Boolean witness search.

The exact case relation remains primary. Named finds are semantic
subrelations and views are projections over them;
no mandatory grouping key should force a choice between hiding profile
multiplicity and emitting an unreadable row for every profile field. `each
case` preserves `CaseId` as logical row identity, so two cases remain distinct
even when every displayed value is equal. The analogous raw explanation view
uses `each incidence`, preserving the authorized `(CaseId, TransitionId,
SignatureId)` incidence row before any grouping.

The target keeps these layers separate:

- **base relation identity (`RelationId`)**: stable model/type owners, state and
  context schemas, ordered finite source producers, canonical dependent
  successor semantics, endpoint membership and lineage contracts;
- **admission identity (`AdmissionId`)**: one `RelationId` plus scoped Before,
  After and transition validity predicates;
- **question identity (`QuestionId`)**: one `AdmissionId` plus one normalized
  named find predicate and polarity, including the predicate-free `all` relation;
- **analysis identity (`AnalyzeGraphId`)**: one Explore contract plus its checked
  view, choice and explanation dependency DAG;
- **view identity (`ViewId`)**: one QuestionId, ChoiceId or
  MechanismRequestId-incidence input plus grouping, measures, aggregates,
  selected public fields, ordering, update mode and privacy policy;
- **choice identity (`ChoiceId`)**: one QuestionId plus measures, comparison
  partitions, eligibility, objective, cardinality and tie policy, independent
  of any display view;
- **explanation-request identity (`MechanismRequestId`)**: one named find
  or choice target, canonical endpoint observation roots and signature
  normalization;
- **durable-evidence identity**: immutable relation, admission, named finds,
  analysis DAG and explanation requests plus evidence-retention authorization,
  bound to evaluator, journal and serialization-schema contracts; and
- **operational records**: each invocation's run-state path, time and resource
  limits and workers, plus scheduler and pause events accumulated in the
  journal across resumes.

`SourceKey`, `SuccessorKey` and `CaseId` derive from `RelationId` and canonical
row content. They therefore survive a new admission predicate, finding or
analysis. Admission classifications are keyed by `(AdmissionId, CaseId)` and
question classifications by `(QuestionId, CaseId)`. This is what lets one
authenticated transition relation answer another authorized question without
pretending that the underlying cases changed.

Concrete execution constructs each case and evaluates its endpoint and
admission once, then fans the admitted case out to the unique semantic
questions. For `N` admitted cases and question-predicate costs `f_i`, this is
`O(N * sum(f_i))` question work, not `q` repetitions of source construction,
successor evaluation, endpoint observation and admission. Authored aliases of
one normalized question add no evaluation work. Honest extensional evidence
can require `Omega(Nq)` decision bits for arbitrary unrelated predicates; no
algorithm can erase information that is genuinely independent. The compact
classified sweep therefore compresses only what the checked transcript proves
repetitive. Every admitted coordinate carries one packed bit vector in
canonical `QuestionId` order, rejection is a separate admission outcome, and
adjacent coordinates coalesce only when their complete joint outcome is equal.
For chunk `c` with `R_c` joint runs, retained classification state is
`O(sum_c R_c * ceil(q/8))` decision bytes plus per-question scalar counts,
rather than a resident table of case records crossed with questions. Its
honest worst case is still `R_c = N_c`.

One rankable case partition and one transcript are shared by all named
questions. Admission evidence is installed once per run; selection evidence
is installed for every `(run, QuestionId)` before that leaf seals, so each
question can close and count independently from the same support DAG. A run
selected by several questions is concretely revisited once and its bounded row
payload is shared by those questions. Native-classifier v2 and regional
certificates remain exact-one accelerators; plural sweeps use the checked
interpreter backend without choosing a primary or first-authored question.
The focused implementation audit uses 300 integer transitions and two
overlapping finds selecting the final 20 and final 10 cases. It pauses inside
the first chunk after 17 evaluations, reopens the journal, and completes with
exact counts `20` and `10` after exactly 300 total transition classifications,
not 600. The joint RLE has four chunk-local runs (`0..256`, `256..280`,
`280..290`, `290..300`); the two questions reference two and one selected runs
respectively, while their concrete union is materialized once as 20 cases.
Neither the native nor regional exact-one accelerator participates.
The non-fused work DAG exposes at most one missing question leaf per case and
quantum. Singleton-source fusion charges the number of question decisions to
a fixed event budget, shrinks the member batch to fit, and disables itself when
one member would be too wide. Adding questions therefore cannot silently turn
one scheduler quantum into an unbounded event allocation.

Explore, find, analysis, view, choice and explanation names are unique
source addresses, not raw hash literals. Renaming an address and updating its
references preserves semantic identity; changing its normalized relation,
predicate, partition, observer or privacy contract does not. The transitional
runtime may call positive membership `selected`, but every source-level and
public consumer addresses a named find. There is no ambient selected set and
no primary or first-find fallback.

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
        given before = initial_person
        let context = TaxMunicipalityComparison()
    }

    transition after {
        vary municipality in supported_tax_municipalities(before.tax_year)
        let change = TaxMunicipalityChange(municipality = municipality)
        yield apply_tax_municipality_change(before, change)
    }

    derive policy_assessment {
        before = assess_personskat(before, context)
        after = assess_personskat(after, context)
    }
    derive annual_tax_ore = policy_assessment.after.annual_tax_ore

    admit supported when policy_assessment.before.supported
        && policy_assessment.after.supported
        && tax_municipality_change_permitted(before, after)

    find alternatives = all
}

? analyze lowest_tax_municipality_answer
from explore lowest_tax_municipality {
    view tax_options from find alternatives {
        each case
        select [case_id, municipality = after.tax_municipality, annual_tax_ore]
        updates monotone
    }

    view municipality_summary from find alternatives {
        group all
        aggregate [
            alternative_cases = count_distinct(case_id),
            distinct_annual_tax_values = count_distinct(annual_tax_ore)
        ]
        select [alternative_cases, distinct_annual_tax_values]
        updates closure_gated
    }

    choice lowest_tax from find alternatives {
        partition all
        measure [annual_tax_ore]
        having varies(annual_tax_ore)
        choose all minimizing annual_tax_ore
        updates closure_gated
    }

    view lowest_tax_options from choice lowest_tax {
        each case
        select [case_id, municipality = after.tax_municipality, annual_tax_ore]
        updates closure_gated
    }

    explain municipality_paths from choice lowest_tax using policy_assessment
}

? publish lowest_tax_municipality_evidence
from analyze lowest_tax_municipality_answer {
    emit view tax_options
    emit view municipality_summary
    emit view lowest_tax_options
}
```

This is deliberately a tax-jurisdiction substitution: it frames the person's
other facts, changes `tax_municipality`, and recomputes every tax-year and
municipality-derived parameter. It is not a relocation model. A relocation
query needs a different successor relation that regenerates or branches over
commute, property, residence-category, island and other residence-dependent
facts, and projects a key that distinguishes those After states.
`partition all` says that every admitted municipality alternative belongs to
one global comparison population. After that partition closes, `having
varies(annual_tax_ore)` is true exactly when it contains at least two distinct annual
tax values; false yields an authenticated exact-empty choice rather than a
recommendation. `choose all minimizing` returns every CaseId tied at the
minimum; choosing one display representative would be a different, explicitly
named policy. If this Explore instead varied people, the analysis would need
`partition by [starter = source_key]` to prove one optimum per starting person.
Here `initial_person` is intentional conditioning expressed by the question
“given this person”; it is not an Explore-wide assumption that profiles are
fixed.

The current executable nested `results { ... choose ... }` checkpoint now
lowers this same membership concept to a canonical `ChoiceId` and a separate
display `ViewId`. Choice candidates, the exact input seal, canonical member
prefix and `ChoiceContentRoot` are journaled independently. Mechanisms consume
those members by ordinal and close against the choice root without a `ViewId`;
only then does the display iterate those exact members and apply its row-local
`SELECT` only to them. It does not revisit excluded FIND candidates or repeat the choice
policy. Adding or changing an unused `SELECT` field therefore cannot rename or
roll back what was chosen.
Until explicit `choice` syntax lands, choice objectives may name candidate,
partition, and measure values; aggregate or `SELECT` aliases fail closed rather
than leaking display policy into membership identity. Aggregate-backed displays
over Choice also fail closed until member evidence can support them directly.

If the encoded model has no municipality-dependent result, exact closure proves
zero spread and publishes no “best municipality” recommendation. The
closure-gated summary distinguishes that result (`alternative_cases > 0`,
`distinct_annual_tax_values = 1`) from an exact empty candidate relation
(`alternative_cases = 0`).

The household question uses a finite dependent successor relation rather than
pretending every labor and pension choice is independent:

```runa
? explore household_reallocation {
    from {
        given before = current_household
        given limits = planning_limits
        let context = HouseholdPlanningRequest(limits = limits)
    }

    transition after {
        vary spouse_hours_per_week in candidate_spouse_hours_per_week(before, context)
        vary own_hours_per_week in candidate_own_hours_per_week(
            before,
            context,
            spouse_hours_per_week
        )
        vary pension_plan in candidate_pension_plans(
            before,
            context,
            spouse_hours_per_week,
            own_hours_per_week
        )
        let plan = HouseholdPlan(
            spouse_hours_per_week = spouse_hours_per_week,
            own_hours_per_week = own_hours_per_week,
            pension = pension_plan
        )
        yield apply_household_plan(before, plan)
    }

    derive household_assessment {
        before = assess_household_plan(before, context)
        after = assess_household_plan(after, context)
    }
    derive resources_before_ore = household_assessment.before.disposable_ore
    derive resources_after_ore = household_assessment.after.disposable_ore
    derive spouse_hours_per_week_after = after.spouse.hours_per_week
    derive own_hours_per_week_after = after.self.hours_per_week
    derive spouse_pension_after_ore = after.spouse.pension_ore

    admit feasible when household_assessment.before.supported
        && household_assessment.after.supported
        && legally_and_practically_feasible(
            before,
            after,
            context
        )

    find within_resource_floor = matches of (
        resources_after_ore
            >= resources_before_ore - context.limits.resource_tolerance_ore
    )
}

? analyze household_reallocation_answer
from explore household_reallocation {
    view feasible_plans from find within_resource_floor {
        each case
        select [case_id, request = context, plan = after, resources_after_ore]
        updates monotone
    }

    choice tradeoffs from find within_resource_floor {
        partition by [starter = source_key]
        measure [
            disposable_ore = resources_after_ore,
            spouse_hours_per_week = spouse_hours_per_week_after,
            own_hours_per_week = own_hours_per_week_after,
            spouse_pension_ore = spouse_pension_after_ore
        ]
        pareto [
            maximize disposable_ore,
            minimize spouse_hours_per_week,
            minimize own_hours_per_week,
            maximize spouse_pension_ore
        ]
        updates closure_gated
    }

    view tradeoff_plans from choice tradeoffs {
        each case
        select [case_id, request = context, plan = after, resources_after_ore]
        updates closure_gated
    }

    explain household_paths from choice tradeoffs using household_assessment
}

? publish household_reallocation_evidence
from analyze household_reallocation_answer {
    emit view feasible_plans
    emit view tradeoff_plans
}
```

The municipality and household relations are also the smallest permanent
support-projection fixtures: one starter may yield several successor cases for
one explained mechanism. A later mechanism-selected Publish attachment must
therefore demonstrate `|support cases S| > |support starters P|` while both
relations close exactly; equal counts in a singleton-transition cliff fixture
are not enough to test the distinction.

This query can expose the trade-off frontier; it cannot infer what the couple
“should” prefer. The one-sided floor permits plans that improve resources while
rejecting plans more than the stated tolerance below Before. This example
deliberately exercises a real successor fiber: the single canonical source may
have zero, one or many finite After rows, and its successor frontier closes
independently. The `candidate_*` producers encode finite structural domains and
dependencies only; legal and practical eligibility remains in `admit feasible`.
The ordered `vary` bindings make dependency explicit:
`own_hours_per_week` may depend on `spouse_hours_per_week`, and `pension_plan`
may depend on both.
The final plan derives earnings from hours and wage assumptions, conserves
declared household transfers, and assigns pension payer and ownership without
varying correlated amounts independently. A materially different payment route
must be represented in the typed After plan. If route identity is not state,
vary it in `from`, encode it in typed Context and let the successor depend on
that Context; a transition-block local cannot hide in producer provenance and
survive as another case.
`current_household` is likewise an explicit “given this household” condition;
another question may enumerate a coherent household-profile relation instead.
Feasibility and whether objectives are lexicographic, weighted or Pareto are
explicit parts of the question. General dependent relations and partitioned
Pareto choices are target semantics; their target syntax is not wired end to
end yet. `pareto` is set-valued and returns every nondominated CaseId in each
declared starter partition. Candidate `x` dominates `y` exactly when it is no
worse on every declared objective and strictly better on at least one. Equal
objective vectors do not dominate each other, so all distinct tied CaseIds
remain. Every objective must be total and exactly comparable, and only
partition closure can prove that no unseen case dominates a member.

Every target `explain` node declares a unique request name, a named relation
target and an explicit canonical endpoint observation. The name makes several
requests using the same observer independently addressable without defining
semantic identity. `from find NAME` targets every case in that question relation;
`from choice NAME` waits for the choice's partition closure, targets its proven
members, and seals the referenced `ChoiceId` into explanation-request identity.
Presentation fields and measures do not infer the observation root. Here
`household_assessment` resolves to `assess_household_plan`, which must expose
the modeled resources, hours, pension and tax dependencies needed by the
question and objectives. A future convenience
inference is possible only after its normalization is specified; it is not
implicit in this design.

The named observer resolves to one checked pure callable of shape
`(State, Context) -> Observation`, evaluated independently at Before and After.
Its reachable rule and call closure, not every theoretical value of those
types, is sealed into explanation-request identity. The request carries one
totality obligation over every distinct Before and After endpoint in its exact
target. A static certificate may discharge that obligation over sound finite
over-approximations before replay. Alternatively, after the finite target
closes, a complete set of successful canonical endpoint-evaluation receipts may
discharge it extensionally. A sample or open prefix is never enough.

Fresh traced evaluation remains the sole source of concrete traces and
signatures. A semantic evaluation failure after a valid static certificate is
an integrity error. Without such a certificate, deterministic partiality is
typed unavailable and prevents exact explanation; operational instrumentation
or capacity limits do the same. Signature counts are exact only when the target,
totality obligation and replay frontiers close with no unavailable endpoint;
otherwise they remain unknown or lower bounds. The engine never silently falls
back to tracing selected display fields.

Mechanism replay over `choice NAME` explains the chosen transitions; it
does not prove that they minimize a measure or lie on the Pareto frontier. That
proof comes from exact question-relation closure plus the choice's measures and
partition semantics. A request for comparative causal
explanation must instead target all find alternatives or name a separate
checked group-comparison observer. Structural-mechanism counts and optimum
proofs therefore remain distinct even when they are published together; raw
signature/profile counts are separately named audit measures.

The currently executable relational surface makes the question address
explicit: `results ... from find NAME`, `choose ...` inside a result block,
and either `mechanisms ... from find NAME using OBSERVER` or
`mechanisms ... from view NAME chosen using OBSERVER`. There is no ambient
`selected` relation; `results ... from selected` is rejected and must name its
FIND input.

### Perspective-based scenario acceptance

A target-syntax scenario is ready for implementation only when four independent
perspectives can recover its contract without repairing it in scheduler code or
publication configuration:

- **Policy author:** can point to every conditioned input in `given`, every
  finite search dimension in `vary`, every computed fact in `let` or `derive`,
  the intervention in `transition`, the total eligibility decision in `admit`,
  and the policy proposition in each named `find` relation. For minima or
  Pareto questions, the author can also state exactly which alternatives each
  `partition` compares and why the requested observer explains the chosen
  outcome.
- **Language implementer:** can assign a typed grain and stable identity to
  every declaration, build one acyclic Explore/Analyze dependency graph, state
  when each node may revise or close, and implement the syntax without a
  scenario-specific special case. Choice consumes semantic question rows rather
  than a display projection; explanation names its checked observer; Publish
  cannot change membership or choice.
- **Auditor/result consumer:** can restate the declared universe and transition
  from the checked query contract, distinguish rejected from unavailable cases,
  tell whether each finding, choice and explanation is open or exact, and
  distinguish cases, distinct starters and structural mechanisms. The auditor
  can use one EvidenceToken to read mutually consistent artifacts and a
  separately typed ResumeCursor to continue a paused run, without treating
  either token or a mechanism hash as policy input.
- **Stream operator:** can pause and resume bounded work, apply public
  add/retract/seal events deterministically, distinguish execution, semantic,
  materialization and count status, and enforce resource/publication policy
  without changing semantic identities or completeness claims.

If any perspective must infer a hidden fixed profile, an implicit comparison
group, an unreported filter, or what an unqualified “complete” means, the
scenario fails acceptance even if its evaluator happens to terminate.
The dated cross-scenario verdict and the issues corrected before it passed are
recorded in the
[implementation workbook](../../docs/rfcs/bounded-rule-exploration-workbook.md#pbr-round-2-record-2026-09-03).

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

and every signature `m` has a case-support fiber
`S_m = { case | mu(case) = m }`. The first witness proves
`lower_bound(1)`. Ten distinct incidences prove `lower_bound(10)`. The result is
`exact(10)` only when the complete target frontier is terminal and no unresolved
case or weighted cell can add another member. A support-counting cap of 100
therefore means `lower_bound(100), censored`; it does not mean “probably
infinite.”

The progressively refined correlated case-support object is richer than one
scalar:

```text
S(m)       = disjoint concrete witnesses
             + disjoint certified uniform cells with exact weights
             + a residual unresolved frontier

case_count(m) = unknown | interval(lower, upper) | exact(n)
shape(S(m))   = a union of correlated typed regions
```

Lower bounds grow as incidences or disjoint uniform cells arrive. Finite upper
bounds shrink as remaining regions are assigned elsewhere or proved empty.
When both meet, the count is exact. Stable CaseIds and disjoint-cell receipts
make query-bound authenticated evidence chunks mergeable across resumed or
distributed workers without double counting. This is one canonical merge
algebra under the same question and coverage authority, not a consensus
protocol.

A mechanism may occupy a bounded income interval while being invariant across
several commune or household dimensions; geometrically its support is a union
of cells or cylinders in the declared product space. In a finite Explore query
those regions still have finite weight. Touching the authored boundary means
the support is censored there, not that it is unbounded. A future parameterized
theorem layer may claim an unbounded direction or `infinite_proven` only by
supplying a checked region or injective family of cases. This separates honest
unknown frontiers from genuine mathematical infinity.

The starting context is not incidental metadata. Because a case is
`(Context, Before, After)`, every complete signature has a correlated
case-support relation `S`, its distinct starter projection `P`, and a dependent
successor fiber `A`:

```text
S(m) = { (context, before, after) | the case has signature m }
P(m) = distinct projection_(Context, Before)(S(m))
A(m, source) = { after | (source.context, source.before, after) in S(m) }
```

`S` retains source/successor correlation, `P` contains only distinct source
rows, and `A(m, source)` is the After fiber of `S` beneath one source rather
than an independently widened After marginal. The case count is the sum of the
successor-fiber weights over distinct starters. This avoids conflating “how
many starting worlds?” with “how many transitions?”, since one household state
can have several explored successor choices.

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
case_support     S = project_(FacetedSubject, CaseId, OriginSource, After)(I join R_(q,t))
starter_support  P = project_(FacetedSubject, OriginSource)(S)
after_fiber      A(subject, origin) = { after | exists case_id:
                                        (subject, case_id, origin, after) in S }
```

These are set semantics after canonical `CaseId`/`SuccessorKey`
deduplication, so for one subject
`|S_subject| = sum_(origin in P_subject) |A(subject, origin)|`. Case counts may
be added across disjoint signature fibers belonging to that subject. Starter
counts may not: the same `OriginSource` can support several signatures and is
unioned once. Supports of different nodes or edges can overlap as well, so
their counts are never summed into a mechanism total.

When both sides are concrete, an open exploration knows a powerset interval
`S^- subseteq S subseteq S^+`: the inner relation grows as evidence arrives,
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

So the useful answer to “does every mechanism node have its own starter-support
bounds?” is **yes, as a conditioned support overlay, not as part of the
node**:

```text
stable structural subject: StructuralMechanismId | StructuralNodeId | StructuralEdgeId
case-support overlay:      (request, target, subject, facet) -> S^- subseteq S subseteq S^+
starter projection:        P^- = distinct_sources(S^-) ... P^+ = distinct_sources(S^+)
successor fibers:          source -> A^-(source) subseteq A(source) subseteq A^+(source)
```

`S` is the correlated case relation, `P` is the correlated set of starting
`(Context, Before)` worlds obtained by distinct projection, and `A` keeps the
possible `After` worlds beneath each starter. This is why the result cannot be
summarized safely as only “income 190,000--200,000, any commune”: income,
commune, household and other starter coordinates may constrain one another.
The same structural node may consequently keep the same identity while its
support overlay grows, narrows or differs between exploration questions.

That node overlay is its **total** support across all enclosing structural
mechanisms and routes. A narrower explanation may condition it on an enclosing
`StructuralMechanismId`, one incident `StructuralEdgeId`, or a canonical path
segment:

```text
cases(node | route)    = { case in cases(node) | its structural assignment satisfies route }
starters(node | route) = distinct projection_(Context, Before)(cases(node | route))
afters(node | route, source) = { after | (source, after) in cases(node | route) }
```

A complete route cover unions to the total node support, but its case-support
fibers need not partition it. One case can contain several qualifying incident
edges or paths, and distinct route case-support fibers can project onto the
same starter. Their counts therefore require set union and deduplication;
adding them without a checked partition proof produces an overlapping
route-incidence count.

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
different explicitly named `find` or `choice` targets without changing
identity. An all-admitted overlay is expressed by naming `find NAME = all`;
admission itself is not a mechanism-request target.
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
an invented finite interval. The journal can accept immutable observation
points for that open state under the stable request/target/subject identity.
Publication v18 projects those points into a resumable request-local sidecar;
it does not wait for closure and then invent an all-subject report. The
eventual request-level support closure yields a sealed successor for every
registered mechanism slice; it does not mint a replacement slice identity.

Activation is also weaker than causation. A node may be visited with the same
outcome Before and After, participate differentially by changing presence or
outcome, or be established by stronger future counterfactual evidence as
responsible or sufficient for the selected result. Those are separate support
facets. The first mechanism graph should publish activation and differential
participation honestly; it must not silently label either one “the cause.”
A whole structural mechanism has one facetless support. The activation versus
differential distinction belongs to its internal node and edge support views.

Starter-support bounds are therefore proof-carrying, correlated set regions.
For example, the union

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

The order of derivation is important. First bound the subject's correlated
target-case relation `S`, then project and deduplicate its starting worlds into
`P`; the `A(source)` columns remain fibers of `S`:

```text
S^- (proved cases)  subseteq  S (true cases)  subseteq  S^+ (possible cases)
        |                        |                         |
 distinct-source projection     |                         |
        v                        v                         v
P^- (proved starters) subseteq P (true starters) subseteq P^+ (possible starters)

A^-(source), A(source), A^+(source) are the corresponding fibers of S^-, S, S^+.
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
view is rebuilt from the same authenticated signature leaves. Publication v18
does not eagerly rebuild those unions or emit closure-time support rows for
every node and edge. The structural-definition catalog publishes stable
support-slice descriptors for mechanisms and activation/differential node and
edge facets. A descriptor is an address, not an observation or a count; only a
scheduled slice contributes a factorized summary to the append-only
mechanism-support observation sidecar.

Every discovered structural mechanism automatically registers its facetless
total-support slice. Importing an assignment or terminal fiber marks only the
affected mechanism dirty, in a canonical ordered set. Several changes may
therefore coalesce into one next point for that mechanism, while already
accepted points remain immutable historical descriptions of their exact
journal prefixes. A global frontier advance caused solely by another mechanism
does not force every older mechanism to be re-observed. Once request support
closes, a lazy final sweep schedules one sealed successor for every registered
mechanism slice; the compact closure receipt is withheld until the registered,
observed and sealed slice counts all equal the exact structural-mechanism count.
This automatic whole-mechanism registry is the **core lane**. Selected node and
edge slices use a separate **explicit extension lane**, so attaching one does
not enlarge the core frontier or delay its closure receipt. A descriptor which
neither lane requests remains only an address. In particular, publishing the
DAG never creates a resident `cases × DAG subjects` table.

The Experimental checked syntax registers one compact slice explicitly:

```runa
observations selected_cliff_node_support
from mechanisms cliff_paths
for node differential "<StructuralNodeId>"
within mechanism "<StructuralMechanismId>"
```

The last line is optional and is valid only for a node or edge. The subject may
also be `mechanism "<StructuralMechanismId>"`, `node activation ...`, or the
corresponding activation/differential edge form. Declaration names are output
addresses, not slice identity: two names requesting the same request, subject,
facet and optional enclosing mechanism share one checked demand ID and one
scheduler registration. The unique demand-set identity is independent of
declaration order. Adding or renaming an observation does not rename the
relation, question, mechanism request or analysis DAG.

An explicit demand may be present before core support closes or attached after
core analysis has already closed. Registration fixes the current structural
assignment prefix, then backfills it in canonical pages of at most 256
signature assignments; a partial backfill is never observable. Backfill also
installs subject/route and signature watchers, including for a slice with no
matching fiber yet. Later assignment or terminal evidence therefore dirties
only ready slices incident to that evidence, rather than rescanning all
demands. If a demand first becomes observable after durable support closure,
its first accepted observation may be `sealed`; no invented open predecessor
is needed. These frontend and scheduler semantics are integrated through the
journal, stream driver and publication plan; focused executable verification
is the next gate before a larger audit run.

Those descriptors are authorization-neutral. Publication v18 retains the
explicit single-subject typed materializer introduced in publication v9 when a
checked lossless selected-input, each-case view exposes `case_id`, `context`,
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

The first form is the deduplicated total `S` case support. The second intersects
the node/edge signature index with the named mechanism's signature index. It
binds the route into the publication plan and resume cursor without renaming
the structural subject, request, question or analysis DAG.

The route-qualified typed artifact uses the historically named subject-starter
record schema v3. Its members are nevertheless `S` case-support members, each
carrying one source/successor pair; the deduplicated source-only population is
`P`. An unqualified consumer omits the optional route coordinate and retains
its v1 consumer-local identity and record shape. In the historical
publication-v9 contract this allowed a qualified consumer to be appended
without republishing the core exploration. Publication v18 preserves the
semantic separation, but has its own Experimental publication plan and cursor.

The subject may instead be one structural mechanism or an
activation/differential node or edge. It is deliberately singular: v1 has no
wildcard or list which could fan out into a hidden `cases × DAG` export. For a chosen
target, the authorizing view covers the selected population from which the same
`QuestionId`'s chosen subset was derived; the choosing view need not itself
expose all four values. The declaration is an appendable publication consumer,
so it can be added after copying an ID from the completed structural-definition
sidecar without changing or replaying the core exploration journal.

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
close independently of this resumable typed-publication lane. The structural
catalog carries stable descriptors for **total** mechanism, node and edge
support slices. Every mechanism-total summary is scheduled automatically;
node/edge summaries appear only when an explicit `observations` declaration
schedules and journals that stable slice. An authored typed projection remains
a separate
`starters/<name>.ndjson` consumer and does not turn a descriptor or compact
observation into inline `(Context, Before) -> Set<After>` case-support values.
Mechanism-route conditioning reuses that same bounded typed projection;
arbitrary path predicates and local-entry regions remain separate future work.

The shared frontier is itself factorized into pending cases, unavailable cases,
and complete signature fibers which do not yet have a validated structural
assignment. These components have canonical incremental roots. When an
assignment arrives, one signature descriptor leaves the unresolved manifest;
the stream does not rewrite all of that signature's cases. Exact node case
bounds follow from the disjoint signature weights. Exact node starter-support
projections remain a separate resumable operation, because starters can overlap
across signatures; until it catches up, the sealed target's distinct starters
provide a safe but deliberately conservative `target_projection_upper`. It is
not the materialized outer node starter-support set. If all sealed target
starters are already in the confirmed inner set, the distinct-starter count can
nevertheless be exact while unresolved cases still keep the case count and
conditional After fibers open.

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

For a `find NAME = all` question, positive membership is the admissible relation, so
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

A paused or stopped target bundle should make the result speak without requiring
the reader to reconstruct the query from hashes. It publishes:

- a normalized query contract covering every `given`, `vary`, `let`, `derive`,
  `transition`, `admit` and named `find` relation, including units,
  cardinality and declared coverage limits;
- a closure-aware summary of declared, constructible, admitted, rejected and
  unavailable source rows, cases and transitions, affected `ProfileKeyId` values,
  structural mechanisms, raw signatures, execution profiles, and
  saturated-support signatures;
- named relational views such as `cliffs`, `affected_profiles` and
  `mechanism_loss_bins_50_dkk`, each with its own exactness, schema, grouping
  key and privacy authorization; and
- orthogonal execution, semantic, materialization and count statuses, one
  opaque EvidenceToken for consistent reads, and a separate ResumeCursor when
  resumable work remains.

The three most easily confused counts stay visibly different:

- a **case** is one constructible `(Context, Before, After)` transition in this
  relation; admission and each named find define separately counted subsets;
- a **starter** is one distinct `(Context, Before)` source row and may own zero,
  one or several admitted successor cases; and
- a **structural mechanism** is one normalized differential explanation shared
  by any number of cases and starters.

For an exact result, illustrative public wording is:

> **Question.** Find 100-DKK salary transitions whose modeled after-tax
> resources decrease. **Declared universe.** Exactly 2,000 lower endpoints
> `0, 100, ..., 199,900` DKK for one explicitly conditioned 2026 profile, each
> with a +100-DKK successor through 200,000 DKK. This is not a claim about other
> profiles or 1-DKK transitions. **Answer — exact.** 16
> finding cases from 16 distinct starters share 2 structural mechanisms under
> the recorded explanation equivalence version. **Execution:**
> `stopped(completed)`.
> **Semantic:** finding exact; explanation exact. **Materialization:** 10/10
> artifacts caught up. **Evidence:** `TOKEN`.

For a useful paused prefix, the wording is instead:

> **Execution:** paused safely after 20 minutes. **Semantic:** open. Of the
> exactly 2,000 declared transitions, 512 are classified; at least 3 finding
> cases from at least 3 starters and at least 2 structural mechanisms are
> confirmed. No exact-total or exact-empty claim is available yet.
> **Materialization:** caught up to EvidenceToken `TOKEN`. **ResumeCursor:**
> `CURSOR`; only that cursor continues the run.

For one shared explanation:

> Mechanism M7 is shared by at least 84 cases from exactly 61 distinct starters. Their
> input values and execution multiplicities may differ; fresh Before/After
> replays normalize to the same differential rule-and-branch topology. Starter
> support is exact. Correlated case/successor support is open, so 84 is a lower
> bound rather than an inferred total.

The concrete numbers above are examples of wording, not new Personskat audit
results. A real report may also say that `Z` raw-signature supports are at
least `c` because their support-counting cap was reached, followed by an exact
or lower-bound 50-DKK **structural-mechanism** histogram and links to the
corresponding case, starter and incidence views. JSON is a reproducible
materialization of named evidence, not the authority from which the graphs are
reconstructed.

### Experimental implementation boundary

Keep accepted architecture separate from executable evidence. The feature
branch now connects the canonical frontend, relational lowering, content-stable
identities, durable append-only journal, pause/resume scheduler, named selected
views, fresh endpoint mechanism replay, incidence views and crash-safe NDJSON
publication through the public `runa explore` command.

An earlier single-question checkpoint of
`relational-explore-stream-smoke.runa` closed four sources and cases, two
positive transitions, one shared raw mechanism signature, and one structural
mechanism/execution profile. Its eight publication-v7 artifacts reached
sequence 107. That snapshot remains implementation history; the current
plural 4/2/1 execution and its fourteen publication-v17 artifacts are recorded
at the start of this workbook. Both are runtime evidence for the resumable
stream and case/mechanism graph, not Personskat evidence.

The conditioned
`personskat_income_cliffs_conditioned_100_dkk_grid_200k_2026` query is now a
checked public query over 2,000 coarse income coordinates and 21 exposed
configuration fields. Its source views publish the conditioned profile even
when `selected` is exactly empty; its mechanism observer maps coordinates back
to concrete kroner; and preparation retains the compiler-proven transitive
declaration slice needed by this query.

Global parse, type and checked-resolution failures remain program-wide. After
those authorities succeed, preparation eagerly records one lightweight slot
per named query; query-name selection happens over those slots first. Only
access to the selected slot lazily mints its expensive checked artifact,
identity ladder and request-scoped certificates. A query-local artifact or
certificate failure is retained on that slot without poisoning a valid sibling
query.

Endpoint replay currently has a precise but deliberately narrower authorization
seam than the target contract. Today's implementation accepts only a
query-relative static `RelationalEndpointTotalityCertificate` before its
observer may run. That certificate binds the request and relation identities,
the Before and After marginal-overapproximation roots, the normalized abstract-
proof root and its obligation count. It is the current static subtype of
endpoint-totality evidence, not the identity of the explanation or its only
future proof strategy. `RelationId` continues to bind the exact correlated
`(Context, Before) -> Set<After>` relation; the proof roots record possibly
larger endpoint marginals over which static totality was established.

The target AnalyzeGraph and journal genesis seal the
`(MechanismRequestId, EndpointTotalityObligationId)` pair, not a certificate.
Either accepted static evidence or complete extensional endpoint receipts may
later discharge that obligation and enter the semantic evidence root without
renaming the graph or genesis contract. During the transition, the executable
plan additionally requires and authenticates the matching static certificate;
that is a runtime authorization limit of this implementation slice, not a
second semantic graph contract. The first semantic event registers the complete
plan/root, and resume validates the contract, obligation and any proof artifact
on which accepted evidence depends. A semantic endpoint failure after valid
static authorization is an integrity error; operational instrumentation or
capacity exhaustion remains separately reportable as unavailable replay.

Status matters here: the certificate carrier, query-relative abstract prover,
per-query failure isolation and plan/journal/replay authorization seam are now
implemented. The focused `endpoint_totality` library lane covers guarded and
unguarded arithmetic, transitive helpers, overflow, rule dispatch, runtime-
equality parity, deterministic identity, sibling-query isolation, codec
restoration and replay authorization. An earlier certificate-only 200,000-DKK
attempt enumerated no cases and exposed evaluator stack growth and a lost
complementary-guard residual. After those corrections,
`personskat_200k_landscape_endpoint_totality_certifies_without_execution` now
mints and validates the request-scoped certificate and authorizes analysis-plan
construction for the conditioned 2,000-edge request. It still enumerates no
cases and performs no journal, scheduler, endpoint-replay, incidence or
publication work; this is admission evidence for those query-relative proof
domains, not an exploration result or broad Personskat profile closure.

The completed 2,000- and 10,500-edge Personskat streams from 2026-08-31
described below predate the obligation-bearing target graph and the current
static-proof authorization seam. Their results remain historical evidence
under their then-current declared relations and journal contracts, but their
journals and artifacts cannot be resumed or accepted as authority by the
current contract. The current obligation-bearing execution recorded later in
this section has its own genesis, checked identity and totality evidence; it
supersedes those runs as implementation evidence for today's contract without
rewriting their historical claims.

The historical first complete Personskat stream closed on 2026-08-31. Its
result was authenticated and exact for its then-current declared relation:

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
definition sidecar supplies stable mechanism/node/edge support-slice
descriptors and the request-local observation sidecar supplies bounded
factorized summaries for every discovered mechanism total. The checked
`observations` syntax can additionally name selected node/edge slices without
requesting their typed starter rows; runtime/publication verification of that
new extension lane remains pending.
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

The selected cases form two complete mechanism case-support fibers. Their
starter-support projections lie at 100 km and 150 km respectively; in each,
the lower salaries are
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
`S: SourceKey -> Set<SuccessorKey>` expressions are equal. Thus each
mechanism's `P` starter-support projection is an **exact request-conditioned
correlated set**, not a range inferred from its case count and not part of the
mechanism's stable identity.

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

The public values needed to read those two case-support fibers were already
authorized by the checked `cliffs` selected-case view. In the historical
publication-v8 experiment, that authorization automatically scheduled a separate
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

The historical closure-time compact node rows exposed why node starter-support
projection bounds are a distinct operation. That publication-v8/v9 structural
catalog contained **8,053 nodes** and **20,720 edges**, with activation and
differential-participation support views for a total of **57,548**
request-target-conditioned subject rows. For example, a shared activation node
supported by both signatures had **16 exact cases** but only the honest starter
interval **8..16** in its factorized row. The lower bound was the largest
contributing signature's exact starter set; the upper bound was the sealed
target's 16 starters. Materializing and deduplicating that node's correlated
cross-signature union would decide the exact value. The publisher therefore
did not relabel the exact case count as an exact starter count or manufacture a
Cartesian profile box.

Publication v9 closed that product boundary with the explicit single-subject
`starters` declaration above. It could materialize the shared node's exact typed
case-support fiber on demand while the other 8,052 nodes and 20,720 edges stayed
factorized. Publication must never eagerly serialize all of those case-support
fibers merely because the structural DAG is published.

Publication v18 keeps that typed consumer but removes the eager closure-time
all-subject `structural_subject_support` row enumeration. Structural
definitions now carry stable slice descriptors; only scheduled slices
receive compact, append-only support observations. Thus the old 57,548-row
measurement remains historical evidence for the support algebra, not the shape
or cost of the current publication.

The existing `graphs/case-support-<question-id-hex>.ndjson` lane remains a
different useful object: one artifact per canonical question projects the
shared classification/support partition proving how searched regions became
excluded, admitted, matched and selected. Aliases share the QuestionId-addressed
artifact; no authored find is primary. It is not the semantic state graph
`Before -> Transition -> After`. That semantic graph and the mechanism
starter-support projections can share identifiers and typed case evidence, but
neither should be misnamed as the other.

Publication now gives that semantic graph its own bounded lane at
`graphs/case-transitions.ndjson`. A checked lossless selected-case view is the
value authority. Each selected row retains the actual Context, Before and
After alongside CaseId, SourceKey, SuccessorKey, role-neutral endpoint
StateIds, directional TransitionId and the checked schema identities. The
closure counts cases, distinct state nodes and distinct transitions and binds
the canonical selected-case set rather than treating journal discovery order
as graph identity. Mechanism incidences join through CaseId/TransitionId;
mechanism/node/edge starter-support projections and successor fibers join
through SourceKey/SuccessorKey. This is
the concrete bridge between the case graph and mechanism DAG, without a
`cases x mechanism subjects` expansion.

That convenient selected typed edge list is deliberately not the complete
case graph. The SQL-like trailing declaration

```runa
transitions income_cliff_case_graph from all cases
```

requests a distinct identity-only artifact at
`graphs/income_cliff_case_graph.ndjson`. Its semantic journal index exists
regardless of whether the consumer is declared: successor discovery grows U,
admission grows D, and the question-selected population grows M. Records are
canonical StateId/TransitionId nodes plus U/D/M CaseId support; Context,
Before and After values do not leak through this declaration. Each support row
does retain `SourceKey` and `SuccessorKey`: that authenticated route preserves
which `(Context, Before)` starter and which per-starter After fiber produced a
case in this relation. It does not redefine the global semantic `TransitionId`,
which already binds canonical Context/Before/After, or disclose those typed
values. The previously described `S_C` selected count and this graph's `M_C`
count name the same question-relative population.

This distinction is important for resumability. A fresh small graph request
may make concrete traversal the cheapest materialization strategy, while a
late request on a symbolically closed journal must either use an already
authenticated materializer or say `unmaterialized`. It may not restart the
question invisibly. Exact graphs page directly from authenticated ID-ordered
indexes; oversized graphs terminate honestly as `capacity_limited` rather
than cloning an O(N) terminal value or treating a cap as an exact case count.

The historical closed 10,500-case commuter journal would have been the natural
first attachment audit, but it was minted under an earlier journal contract
and the current reader correctly rejects its prior-head identity. There is
therefore no authenticated publication-v9 case-transition artifact for those
16 historical selected cases. The design did not convert that journal or
manufacture a new 341xxx/342xxx input fixture to obtain a convenient answer.

A fresh audit under the then-current publication-v9 contract subsequently
closed the same authored 10,500-edge relation from a new journal. Its lifecycle,
relation, FIND frontier and analysis frontier were all exact under that
contract:

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

The historical publication-v9 semantic case graph exists at
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
unions and honest unknown/interval/exact counts. Publication v18 implements
independently domain-separated inner/outer expression bounds for `S`, the
correlated `SourceKey -> Set<SuccessorKey>` case-support contract, and
`P = distinct_sources(S)`. Its append-only observation points also carry
explicit `starter_set_status` and `correlated_support_status`; neither status
is inferred from scalar counts or root equality. Publication v12 is
implementation history, not a compatibility target. The exact whole-fiber
typed-region v1 is now the bounded publication/navigation implementation;
reduced decision-DAG compression and the broad Personskat run remain later
work. The structural-definition catalog supplies
stable slice descriptors rather than eagerly publishing one closure-time row
for every subject. An explicit typed `starters` consumer remains the separate
authorized materializer for one mechanism, node facet or edge facet; for node
and edge subjects it may bind one enclosing mechanism and derive the
route-conditioned intersection from the existing signature indexes. Neither
the descriptor nor the observation implicitly exposes typed starter content,
and no support root becomes part of a structural subject ID.

Publication v9 implemented the one-enclosing-mechanism selector. A focused
two-mechanism/shared-node fixture proved that total-node support contained two
cases but one deduplicated starter, while each `node | mechanism` slice
contained one case and the same one starter. Their plan and fiber identities
were distinct, and paging retained the two different successors beneath that
shared Source. A separate CLI attachment fixture closed a small journal first,
added a qualified node consumer afterward, and observed zero new semantic
batches/events, unchanged relation/question/analysis/journal identities, a
route-bound v2 starter artifact, and a byte-identical no-op resume. This remains
historical implementation evidence for the typed projection architecture, not
a new Personskat execution or the current compact-observation layout.

What remains is arbitrary path-conditioned selection and reduced, field-level
decision-DAG compression of the current human-readable whole-fiber index. The
index belongs to authorized publication/navigation, not to the semantic
mechanism DAG: it summarizes canonical support-case fibers without changing
their roots, counts or structural identities. Its dimension metadata
preserves the query's `varied`, `derived`, `conditioned`, certified
`irrelevant` or `coverage_gap` classification separately from the values seen
in this support slice. In particular, a singleton seen on a varied commune
axis remains varied, while Copenhagen fixed by the query remains conditioned.
An undemanded node/edge slice descriptor remains only an address; opaque roots
and per-field marginals do not become readable correlated support by
implication.

The starting context is part of every case, not incidental metadata. Write a
starter as `Source = (Context, Before)` and a case as one supported
`Source -> After` transition. The starter support of a mechanism (or one of its
nodes/edges) is the inverse image of its supported cases back onto `Source`.
This is why one starter can contribute several cases when its After fiber has
several successors, and why case count and distinct-starter count are separate
grains. When a node is viewed inside one enclosing mechanism, its starter
support is a subrelation of that mechanism's support; a shared node's total
support may instead union overlapping routes from several mechanisms.

These starter-support set bounds are request-conditioned overlays, not fields
of the stable structural node. Write their authoritative finite fiber relation
as `F : (Context, Before) -> Set<After>`: `domain(F)` is the distinct starter
projection and the graph of `F` is correlated case support. It carries
confirmed inner support, a concrete outer envelope or an opaque open
obligation, and independently closed case/starter/successor counts. Income
ranges, commune lists and other per-field bounds are useful projections for
browsing, but cannot replace `F`: interval widths and marginal products would
invent starting profiles and counts that were never observed or proved.

The first region format should therefore be deliberately plain: one exact
whole-`SourceKey` fiber at a time, in canonical order, with an exact filter back
to the typed support pages. A configured cap stops only between fibers and
closes with the canonical projection job, source closure-record address and
resume coordinate for the uncovered suffix. Since the source pages stream
independently, that reference may point forward until the source artifact
closes. It never widens that suffix into a Cartesian box; if even one fiber
cannot fit, it falls back before emitting a partial one. Counts remain
those of deduplicated canonical keys or a checked disjoint partition, not the
sum of displayed region widths. V1 measures each candidate against a
protocol-fixed 1 MiB synthetic maximum-width publication envelope and binds
that byte policy into both summary and artifact identity. An invocation whose
operational line limit is below the fixed cap is rejected before filesystem
mutation rather than changing the deterministic represented prefix.

The result reports three independent facts: semantic inner/outer support and
its starter/correlated closure statuses; whether the region derivation is an
`exact_partition` or a `confirmed_subset`; and whether compression is
`complete` or `capped`. The artifact is navigation-only in either case. An
opaque outer obligation stays opaque rather than being fabricated as a finite
outer region. Once this exact v1 is trustworthy, equal suffixes may be
hash-consed into a reduced ordered decision DAG with canonical Context/Before
field order and dependent After-set terminals. That compression is a smaller
index over the same `F`, not new evidence.

### Prefix-native support observations

The resumable stream records these overlays as an append-only observation
chain, not as a closure-time report invented by the renderer. Each point binds
one stable support-slice ID, the exact durable three-lane support cursor and
frontier root, a compact factorized summary root, its lifecycle status and the
point it supersedes. Replay reconstructs the summary from that prefix and
rejects a mismatching claim. The journal chain is therefore the recovery and
ordering authority; the public NDJSON artifact is only a resumable projection
of already accepted points.

The target point schema makes all three support objects explicit without
materializing typed values:

```text
case_support: {
    inner_root: S^- expression,
    outer_root: concrete S^+ or opaque upper-support expression,
    coordinates: SourceKey<(Context, Before)> -> Set<SuccessorKey<After>>
}
starter_support: {
    inner_root: P^- = distinct_sources(S^-),
    outer_root: distinct-source projection of the upper S expression,
    materialization: not_materialized,
    starter_set_status: open | exact_starter_set
}
correlated_support_status: open | exact_correlated_support
```

The two root pairs are independently domain-separated: a `P` projection root
is not an alias for an `S` relation root. `A(source)` is read only as the
successor fiber beneath that source in `S`; there is no independent After box.
An opaque target obligation may be committed by an outer expression, so its
root remains stable even while its set/count status is unknown. Root equality
alone never substitutes for either explicit status.

Each point and typed-subject header also carries the audit lineage needed to
interpret those roots: `mechanism_request_id`, `relation_id`, applicable
`admission_id` and `question_id`, `target_id`, structural `subject`/`facet` and
optional `route`, `state_schema_id`, `context_schema_id`,
`transition_type_id`, and `source_coverage_manifest_digest`. These are
support-overlay coordinates and references; typed starter values stay
authorization-gated and structural mechanism/node/edge identities stay
value-free.

Publication v18 emits one flat observation artifact per mechanism request at
`mechanisms/<request>.support-observations.ndjson`. The structural sidecar now
contains structural assignments, the quotient closure and, only after every
automatically registered mechanism slice seals, an optional constant-size
support-closure receipt. It no longer enumerates one factorized
`structural_subject_support` row per mechanism, node and edge at closure. Its
first assignment links to the first observation for that assignment's own
whole-mechanism slice; the observation need not have global ordinal zero. The
structural-definition catalog publishes stable slice descriptors for
whole-mechanism totals and total node/edge activation or
differential-participation support. Every whole-mechanism total is scheduled
automatically in the core lane. A checked `observations` declaration schedules
one total or route-conditioned node/edge slice in the explicit extension lane.
Multiple declaration names for the same stable slice coalesce into one
registration; an explicit whole-mechanism request aliases its automatic slice
rather than duplicating it. Undemanded descriptors remain addresses only.

A second compact request-local ledger at
`mechanisms/<request>.support-observation-demands.ndjson` records each unique
durable registration and its exact checkpoint/scheduler receipt. The manifest
maps every authored name—including aliases—to its name-independent demand ID,
stable slice ID and latest point in the shared observation artifact.

An open point reports `unknown(lower_bound = n)` for an open target. Its inner
case-support fiber contains only inspected, imported signature support; its
outer case-support fiber also names the shared pending, unavailable and
structurally unassigned residual. Later checkpoints can mint monotone
refinements without pretending that an observed prefix is final. Only changes
to a mechanism's own indexed support make its automatic slice dirty, and
multiple changes before its turn coalesce. Earlier points remain valid
historical prefix evidence rather than
being rewritten when unrelated mechanisms advance. Core support closure is
forbidden until automatic dirty work is durable. Closure then enters a lazy,
canonical sweep over the automatic mechanism slices, and analysis closure is
forbidden until each has a sealed successor. At that successor, exact or
interval counts follow from the closed factorized support algebra rather than
from an arbitrary case-display cap. Explicit registry, backfill, dirty and
unsealed roots are authenticated outer scheduler state, but configured-demand
completion is separate from this core closure.

Let `U_updates` be accepted updates which affect one mechanism's support index,
`M_mechanisms` the number of structural mechanisms and `O_points` the accepted
open observation points. The incremental indexed path costs
`O(U_updates log M_mechanisms + O_points * 256)` in the current
hard-bounded signature scan. Final sealing costs
`O(M_mechanisms(log M_mechanisms + 256))`, amortized
across `M_mechanisms` bounded quanta; support closure neither seeds nor
allocates an `O(M_mechanisms)` pending set in one quantum. The design does not
perform an `O(cases × mechanisms)` join or rescan all mechanisms on every
frontier advance. The same coordinate type already names whole mechanisms,
node or edge activation, differential participation and route-conditioned
intersections. Explicit node/edge demand registration therefore schedules
selected graph regions without changing the core journal identities or eagerly
multiplying `cases × nodes`. Each registration catches up to its fixed durable
structural prefix in quanta of at most 256 assignments, then incident-only
watchers carry later evidence forward. This is also why a separate probe
language is unnecessary: an early observation is a durable, resumable prefix
of the real exploration.

The explicit `starters` declaration remains a different consumer. It uses a
named checked value view to materialize one authorized typed
`S` case-support relation `(Context, Before) -> Set<After>`, optionally for a
node or edge within one enclosing mechanism. That relation supplies its
distinct `P` starter projection and dependent `A(source)` fibers. Support
observations stay compact and value-free; adding or reading them does not
implicitly publish private starter configurations.

The compact public answer now exposes this distinction directly. Relational
stream JSON v9 names every named question's closure-aware before-to-after case count and,
for every mechanism request, the structural-mechanism, successful replay and
unavailable-replay counts plus the exact sealed target's distinct starter
count and evidence roots. Each request record must also directly link its
structural-definition artifact, support-observation stream, demand ledger and
declared typed-subject materializations—or one bounded manifest handle which
resolves those links—so the support layer is discoverable from the answer.
Human output leads with the same answer before operational checkpoint
telemetry. Small exact grouped results are rendered inline from their
authenticated projection journals, while the operational publication index
names every complete or still-catching-up NDJSON artifact without loading bulk
configurations into the answer. The manifest remains the full materialization
index for authorized case rows, mechanism DAGs, compact support observations
and any explicitly authorized typed starter-support projections; the durable
journal remains recovery authority.
The sealed target starter count is request-wide; it is not relabeled as an
individual mechanism or node's starter count. Those correlated subject
case-support fibers and starter projections remain in their own authenticated
result layer.

The focused landscape query is authored, but deliberately not launched:
`personskat_mechanism_landscape_conditioned_100_dkk_grid_200k_2026` uses the
same 2,000-edge relation and admission predicates with
`find admitted_cases = all`. Its named
views retain every admitted edge, every successful case/signature incidence,
typed replay-unavailable terminals, closure-qualified structural support,
distinct structural mechanisms, raw signatures and edges per 1,000-DKK income
bin, and the same separate grains per mathematically floored 50-DKK modeled
net-change bin. The authored mechanism counts now group
`structural_mechanism_id`; their neighboring `raw_signatures` fields retain the
replay-sensitive count explicitly. These views are exact for the complete
admitted target only if their frontier closes without
replay-unavailable edges. As a separate fixture it has a separate journal; the
intended combined audit instead places this question beside `cliff_cases` in
one shared journal. Compact signature/receipt journaling now
exists, but this all-admitted mechanism request remains deliberately
deprioritized: it would
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

The checked rule trace now certifies the dispatch proof before that evidence
can enter a signature. A cold activation advances through the exact checked
family roster in exception, conditional-default, clause and
unconditional-default order. `Selected` must close the last `Applicable`
attempt, while `NoApplicableRule` requires the whole family to have failed;
early runtime aborts cannot masquerade as an empty selection. Scope-local and
producer-certified global memo hits take a separate path: each keeps a fresh
activation and may reference only a validated cold selection in the same
endpoint, so repeated hits form a star rooted at the original proof rather
than a chain of increasingly indirect claims.

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

Native classification now carries the frontend's checked `RuleDispatch` ABI
snapshot across dependency slicing. That snapshot deliberately separates
return/parameter type evidence from terminal-miss evidence: a value-typed
partial dispatch may be compiled inside the isolated classifier, but a genuine
miss aborts that process and causes the coordinator to discard and re-evaluate
the whole batch through the checked interpreter. A certified global predicate
miss and a `RuleScope` predicate miss remain `False`. A third case must not be
conflated with either: when a clause head matches and its Boolean body is
`False`, generated code records that match, tries later clauses, lets an
applicable unconditional default win, and otherwise returns `False`. Only a
true zero-candidate unsafe miss reaches the process trap. This mirrors the
checked interpreter's `matched_false_clause` dispatch algebra and removes the
Personskat-specific failure without weakening global miss safety.

The first current-contract Personskat audit closed on 2026-09-04. The exact
native classifier was compiled from the compiler-proven 9,275-statement
dependency slice and completed the declared 2,000 coordinates. Its first
checked one-coordinate parity canary took 25.381 seconds; the native scheduler
then ramped to 256-coordinate batches, whose final full chunks took roughly
1.9--2.1 seconds each. Source checking, slice construction, native generation,
compilation, classification and publication completed in 618.869 seconds. The
stream closed at sequence 86, journal head
`1f07c035374a7316a262dcb8a646086d039cfe12f5276720e13a499c18e91fd8`,
with all 13 declared artifacts caught up. Its checked program identity is
`f525a3697ccd952b4ff053772ea9794d79995028b6628d1190885973958d0ff8`;
the local operator-selected evidence bundle is
`personskat-200k-coarse-v1.run` with sibling
`personskat-200k-coarse-v1.result`.

The exact result is 2,000 sources, 2,000 cases and 2,000 admitted transitions;
all 2,000 were classified as not selected by `cliff_cases`. Consequently the
selected case count, structural-mechanism count, raw-signature count,
execution-profile count and explained-case count are exactly zero; the 50-DKK
loss-bin view is exactly empty. Relation, FIND, mechanism and analysis
frontiers are closed.
This is a result only for the declared `+100 DKK`, 0-through-200,000-DKK
relation and its conditioned 2026 Copenhagen/no-church-tax profile. The source
coverage manifest still reports schema-composition gaps, so neither the empty
answer nor the zero-mechanism graph may be generalized to other profiles,
1-DKK transitions or Danish tax law as a whole.

The ordinary interpreter remains the atomic whole-batch fallback, and the
first successful native batch must agree with it before any native outcome is
trusted. Resource containment is a passive ceiling around this path, not a
separate exploration phase. The content-addressed executable can be reused by
an unchanged checked query, so later resumes need not repay `rustc`; the later
checkpoint below distinguishes that executable reuse from the still-repeated
pre-cache code-generation analysis.

A fresh publication-v17 run of the 10,500-case commuter relation then supplied
an intentionally interruptible current-head prefix. Source and case geometry
closed immediately at **10,500 exact**, including three exact 3,500-income
profile groups. Native classification reached **lower_bound(7,168)** admitted
cases and discovered **lower_bound(8)** selected cases; one of those selected
cases had reached complete raw-signature incidence. Those are prefix facts
only; relation, question, structural quotient and analysis closure remain open.

That prefix exposed a warm-stream ownership bug before an hours-long run was
allowed to continue. Each roughly 15-second `run_slice` rebuilt the
`RelationalStreamDriver`; its mechanism driver therefore lost the process-local
structural-quotient artifact, rederived the entire large mechanism DAG, and
journaled only the next 32-KiB artifact chunk. The run was safely interrupted
after durable sequence **1,209**, head
`5bcdd3b6be60446e37cc41aff396c6aaa080cb63627e78cce122ad70a8bbe178`.
Its authenticated prefix remains resumable. The direct fix is to retain the
query-bound structural artifact across slices in one warm epoch while a cold
resume still rederives and validates it once. No further semantic run should
resume until that cache-ownership gate lands; repeated full derivation per
chunk is algorithmic waste, not useful exploration.

The focused warm/cold regression then proved one structural derivation across
successive warm artifact chunks and exactly one authenticated rederivation
after a cold reopen. Resuming the same journal advanced it to sequence
**2,633**, head
`12634184c72d2a0c324d52d3606bfe609dd65b0de8dee5010febbd2e1f41e2aa`.
The stream now commits **lower_bound(1)** structural mechanism, one execution
profile and five explained-case incidences for the still-open eight-case
selected prefix. It paused safely with `resource_swap_growth`; none of those
lower bounds may yet be read as final counts.

That pause reason is not attributable evidence about this Explore worker.
macOS exposes `Swapouts` as a boot-global counter, so any host process can
advance it. The outer containment supervisor already samples this process
group's RSS, available-memory floor, critical pressure and throttling while
enforcing the operator's 6-GiB envelope as a stricter 5.5-GiB group trip after
its 512-MiB untracked reserve. Under that validated containment only, swap
growth should remain visible advisory telemetry instead of draining a
roughly 1.2-GiB worker and ending its slice. Standalone governor use must keep
the conservative hard backoff, and no actual RSS/headroom/pressure stop is
weakened.

That resume exposed the next operational bottleneck before another invocation.
The native-classifier executable cache is currently consulted only after full
classifier synthesis and Rust code generation. A cache hit therefore repays
roughly the whole checked compiler analysis merely to reproduce the bytes used
as its lookup key. Resume should instead derive an early cache address from the
checked classifier identity plus Futuruna-compiler and `rustc` fingerprints,
then retain the existing protocol identity handshake and checked-interpreter
fallback. This is an operational cache change only: it must not enter query,
journal or result identity.

With both warm structural reuse and the early native-classifier cache in place,
the same publication-v17 stream has now reached exact semantic and publication
closure. Its authenticated checkpoint is sequence **5,325**, head
`47e1f09df411dc5db6981d620efadb6dc6a473fc4ff4d6318b79872525921bc6`;
the journal remains at 96 immutable segments and all 22 artifacts are caught up
to that prefix. The final exact counts are:

- **10,500** sources, cases, admissions and FIND-classified transitions;
- **0** rejected, **16** selected and **10,484** non-selected transitions;
- **2** affected starting profiles and **16** affected starting states;
- **2** raw signatures, structural mechanisms and execution profiles;
- **16** successful mechanism incidences and **0** unavailable cases; and
- one exact 50-DKK loss bin beginning at 5,000 øre, containing both mechanisms,
  both signatures, all 16 cases and both affected profiles.

The cases are the eight lower salaries
`342,400, 343,400, ..., 349,400 DKK` at 100 km and the same eight salaries at
150 km, each followed by the declared 100-DKK promotion. The 50-km profile has
no selected case. Modeled disposable-income loss is **8,161--8,185 øre**. The
current structural mechanism IDs are `d9dfd021...` at 100 km and `2b0db46e...`
at 150 km; each has exactly one raw signature, eight cases and eight distinct
starters. These IDs are discovered result addresses, not authored policy names.

The compact human-usable evidence is outside the checkout under
`/Users/andreasrudolph/futuruna-explore-runs/personskat-350k-commuter-current-v1.result`:
`views/cliffs.ndjson` holds all 16 typed configurations,
`views/mechanism_starter_support.ndjson` holds the exact per-mechanism counts,
`views/mechanism_loss_bins_50_dkk.ndjson` holds the histogram row, and
`graphs/case-transitions.ndjson` is the case graph. `manifest.json` binds these
to the terminal journal head. The final resume spent roughly four minutes on
checked reopen and publication catch-up while appending **zero semantic
events**: sequence and head did not move from the already closed journal.

The two unqualified checked-in convenience `starters` declarations still name
mechanism IDs discovered under the earlier publication-v9 structural quotient.
Under the current closed quotient those subjects are absent. Publication now
reports each one as an authenticated exact-empty closed slice, including
`structural_subject_membership = absent_from_closed_structural_catalog`, rather
than failing the otherwise complete result or inventing members.

The two current IDs were then attached as `_v17` additive consumers. That
resume left the journal at the same sequence and head—again appending **zero
semantic events**—and added four caught-up artifacts. Each starter stream is
three NDJSON records and 28,755 bytes: a header, one page with eight typed
members and an exact closure. The closures independently certify eight cases,
eight distinct starters, equal inner/outer correlated-support roots and
`exact_starter_set`. The materialized values confirm that `d9dfd021...` is the
100-km fiber and `2b0db46e...` is the 150-km fiber, each over the eight salaries
listed above. They are saved as
`starters/cliff_mechanism_100_km_v17.ndjson` and
`starters/cliff_mechanism_150_km_v17.ndjson` in the external result bundle.

This exactness is deliberately scoped. Source coverage v3 reports
`has_gaps = true`: four composed Before/profile subjects, including municipality
and age-status projections, remain `schema_composition_unavailable`. The three
declared commuter groups and their 3,500 salary coordinates are exact, but this
manifest is not yet a certificate that all other profile dimensions were
exhaustively classified. Carrying structured exact-finite profile factors
through the coverage algebra is therefore the next semantic gate before a
broader profile audit or the planned 1,500,000-DKK stream.

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

The architecture now has a strong center. Source construction and downstream
consumers have separate checked identities, and the v0 Cartesian/probe path is
physically gone rather than folded into the `RelationId`-scoped source graph.
The following positive proof and consumption work still matters before the
first broad `0..1,500,000 DKK` audit—1,000-DKK transitions, 1,500 edges and
1,501 reusable endpoints per profile—is a sensible execution target:

1. Make an exact `SupportCell`, rather than only one materialized `CaseId`, a
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
   the ordinary concrete evaluator. Static formatting and soundness review was
   clean through the terminal result/mechanism ownership boundaries at that
   checkpoint; the live Personskat slice still has not run.

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

   The current implementation's request-scoped static endpoint-totality
   certificate is now its closed pre-run authorization boundary, not an
   observer name or an inferred claim that a callable is pure. The target
   graph/genesis binds the underlying obligation and can later accept complete
   extensional evidence without changing identity. The exact 200,000-DKK
   certificate audit exercised this deliberately small static proof language:

   1. path-sensitive integer intervals and predicates that carry guards such as
      `nævner > 0` into `/` and `%`, while proving `i64` overflow absent;
   2. exact checked rule dispatch and `RuleScope` receiver captures, including
      binding constructor fields to the scope's parameter-binder identities;
   3. exact finite-list reasoning for the 98-row 2026 municipal table, so
      filtering for Copenhagen proves the guarded `head` has one element;
   4. lazy, checked-identity top-level bindings, selecting the 2026 table without
      turning unused year tables into proof obligations;
   5. non-strict higher-order collection semantics, especially proving that
      callbacks to `map`, `foldl`, `all` and `any` do not run on exact-empty
      inputs; and
   6. the empty map value and `map_new`, which is evaluated eagerly in the empty
      commuting-history fold even though its `map_get_or`/`map_insert` callback
      is unreachable.

   Exact constructors, fields, matches, local immutable bindings and bounded
   lists support those six capabilities. Effects, unresolved dispatch and an
   abstractly reachable recursive edge continue to fail closed. Conversely,
   recursion, indexing, floating pension calculations and map mutation found
   only behind the fixed no-pension, no-KGL, no-LL33A and empty-commuting paths
   are pruned only after the exact constructor, branch or empty-list proof makes
   them unreachable; syntactic presence alone is neither rejection nor proof.

   These were the audited requirements for the fixed-profile certificate. The
   focused canary now mints the certificate over both endpoint proof domains
   and authorizes plan construction; focused plan/replay coverage preserves the
   authorization and integrity-error seam. This closes only pre-run admission:
   no journal, scheduler, replay, result or mechanism-landscape execution ran,
   and no broader Personskat conclusion follows from it. Uniform or split
   evidence may likewise be minted only after its own exact seam closes; a
   semantic ID or literal-only recipe is never substituted for the checked
   graph. The next experimental boundary is a fresh obligation-bearing genesis,
   discharged by the currently supported static certificate, followed by one
   bounded, observable semantic stream slice—not a 1.5-million-income launch.

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
   contributes its exact logical count. Candidate-first batches may accept
   canonical chunks out of ordinal order into pre-sized sparse slots. Each such
   batch is durable evidence about that exact child, but the covered root
   prefix advances only through a separate checkpoint for the next occupied
   ordinal. Stopping after either event therefore has an honest resumable
   meaning without calling a sparse suffix a prefix. At closure this preserves
   the `2,000 = rejected + non-cliff + cliff` conservation law and changes the
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
   in reverse order; success fills only that child's canonical sparse slot and
   does not move the root cursor. This preserves the
   all-or-nothing journal boundary without copying the complete accumulated
   support catalog once or twice per child. The causal append boundary also
   declares fresh roots without scanning retained refinements, and derives the
   opposite cardinality/injectivity obligation ID before consulting its reverse
   evidence index. Admission of the next chunk therefore does not rescan all
   earlier proof records. The authenticated partition event also retains the
   opaque verified partition authority it reminted from that durable evidence.
   Classified slices, completed chunks and later selected-run materialization
   index that replay-derived value directly; cold replay rebuilds all eight chunk
   descriptors once, not once per slice or positive run. A distinct bounded
   `RelationalClassifiedPrefixAdvanced` checkpoint validates the next occupied
   slot's partition, ordinal, child, interval and artifact digest, then appends
   exactly one `(chunk ordinal, artifact, endpoint)` binding to the typed
   classified-progress chain. The root-relative materialization cursor advances
   in that same checkpoint as its operational mirror. It never cascades across
   several ready slots, so every crash prefix is replayable and bounded.
   Generic cursors cannot choose or advance this branch.
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
   semantic gain. The live scheduler changes only visitation order: range
   endpoints and safely lifted direct affine-guard boundaries nominate chunks
   first, while all other chunks remain an implicit exact residual cover.
   Accepted chunk IDs are reconstructed from the journal, so resume neither
   repeats a completed candidate nor depends on a process-local cursor. An
   authenticated partial concrete accumulator owns its chosen chunk until the
   whole-child artifact is accepted. With slice checkpoints the cold
   controller may grow
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
   question-relative projection attached to the same resumable publisher as
   `graphs/case-support-<question-id-hex>.ndjson`. A classified partition publishes root → chunk
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

   An earlier focused single-question oracle executed this path. Its sealed
   journal contains four exact cases, two selected cases, one shared mechanism
   signature and two mechanism incidences. Reopening the unchanged journal
   appended the previously missing seven-record classification-summary graph:
   one root, three exact outcome regions, two authorized case nodes and one
   closure. A second reopen appended zero semantic events, graph lines or
   source ordinals and preserved both graph and mechanism-file digests. This is
   runtime evidence for the general stream and publication architecture, not a
   Personskat finding. The plural 4/2/1 run recorded at the start of this
   workbook supersedes it as the current-source oracle.

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
   path to certified larger cells and explicitly named `find ... = all`
   mechanism targets.
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

The historical pre-certificate bootstrap completed under its then-current
contract. It ran
`personskat-income-cliffs-200k.explore.runa` over one conditioned profile and
2,000 coordinates representing lower annual salaries `0, 100, ..., 199_900`
DKK, with a deterministic `+100 DKK` successor including the transition into
exactly 200,000 DKK. It closed with 2,000 admitted-not-selected transitions,
zero rejected transitions and zero selected mechanisms. This is an exact claim
only about that declared coarse endpoint relation; it is not an exact-empty
certificate for every 1-DKK transition.

The discovery policy follows from the case and mechanism graphs rather than
becoming a second hand-authored audit. Matching coarse edges, admission changes
and mechanism-signature changes nominate candidate neighborhoods for a finer
query. The current cliff-only file can supply the first two signals but an
exact-empty `cliff_cases` target supplies no signature-change signal. The
all-admitted question instead compares every admitted coarse endpoint and
reuses each endpoint trace across neighboring edges. In the intended combined
query its signature-incidence stream can nominate candidate bounds without
changing the cliff question's meaning or rebuilding the shared relation.

A `+1 DKK` successor is nevertheless a separately declared and checked
relation, not a child whose proof obligations are silently discharged by the
`+100 DKK` journal. Its result is globally exact only when that finer relation
is itself completely covered, or when a future explicit checked bridge proves
which coarse regions transfer. Resolution is an operational search strategy;
coverage remains tied to the declared relation identity.

The first widening now lives in
[`personskat-income-cliffs-200k-profiles.explore.runa`](personskat-income-cliffs-200k-profiles.explore.runa).
It keeps the same income horizon while replacing the conditioned profile with
a genuinely multidimensional coherent profile relation. This comes before the
all-admitted mechanism-landscape audit: widening starter structure validates
profile grouping and sparse selected-only mechanism/support paths without first
tracing every admitted edge. The all-admitted question remains a later
shared-relation consumer when signature-change discovery justifies its replay
cost. This is still a system audit rather than a claim that the model covers
every real Danish taxpayer; profiles outside the declared relation remain out
of scope.

The bounded form is deliberately algebraic rather than a bag of independent
switches. One exact finite `Personskat200kProfile` has only church-tax status
and the relevant two-case age-status enum, so `values(Profile)` contains four
coherent members. The adult and under-18 representative birth dates are both
explicit conditioned fields in `Before.scope`; the profile selects the
applicable date inside evaluation. Crossing those four profiles with 2,000
income coordinates creates **8,000 exact cases** and four exact profile groups
of 2,000. `Context` contains only the 100-DKK promotion. Source-image proof
certifies the profile's direct identity path into `Before.profile`, the income
coordinate's affine `* 100` path, and native execution transports both
authenticated ordinals.

The first complete run closed across two bounded resumable epochs. Its final
manifest reports:

- lifecycle `complete`, with both relation and analysis closure `exact`;
- exactly 8,000 sources, 8,000 cases, 8,000 classified and admitted cases, and
  zero rejected cases;
- exactly four declared profile groups, each with 2,000 distinct salary
  coordinates;
- exactly zero selected income-cliff cases, zero affected profiles and zero
  affected starting states;
- exactly zero structural mechanisms, execution profiles and raw signatures,
  so the 50-DKK mechanism/loss histogram is exact-empty; and
- a source-coverage v4 manifest with 78 entries and no gaps.

The durable journal closed at sequence 164 in 11 segments, with head
`1035a470e7cac1af71e48bb896d897fd4eb7bdfcfd12bc259f101856b4f42fb3`.
The private generated run and result directories use the basename
`personskat-200k-four-profiles-100dkk-v1`; they remain outside the repository.
This exact-empty result applies only to the declared 100-DKK endpoint edges
from 0 through 200,000 DKK. It does not prove the absence of a narrower
within-bin cliff, a cliff above 200,000 DKK, or a cliff in an undeclared
profile.

The first attempted epoch also exposed a general proof issue before creating a
journal. The endpoint-totality BDD treated `status == Fyldt18EllerGift` and
`status == Under18Ugift` as unrelated predicates, despite the source proof
showing that those are the only reachable constructors. Rewriting the tax rule
as a `match` would have changed its mechanism trace and hidden the Explore bug.
Instead, totality now canonicalizes nullary constructors and seeds rule
dispatch with the disjunction of every reachable constructor. Exhaustive enum
guards therefore close, while a missing guard remains a typed partial-dispatch
refusal. Focused positive and negative regressions cover that distinction.

Both the historical single-profile relation and the completed four-profile
relation had no selected coarse cliff in that horizon, and every declared
source, successor, classification obligation and downstream empty frontier
closed under its respective contract. The separate small synthetic query with
an intentional shared mechanism supplied the nonempty integration signal, so
this quiet Personskat range does not leave mechanism incidence and
post-mechanism grouping structurally unexercised.

There is now one precise first broad execution milestone, rather than a vague
1.5-million-income run merely intended to obtain an early number. It crosses a
declared coherent profile relation with 1,500 `+1,000 DKK` edges from zero
through 1,500,000 DKK and reuses the 1,501 endpoints per profile. It is a
profile-and-income endpoint/mechanism audit plus proof-closure experiment, not
an exhaustive 1-DKK cliff audit. The desired signal is that widening the
declared world adds a small number of source events, mechanism signatures and
certified cells rather than requiring one evaluation for every
krone-profile pair. This audit has not run, and the current executable surface
is not claimed to implement the target contract needed to run it.

The intended execution portfolio is proof-first rather than worker-first:

1. checked interval/delta reasoning closes every homogeneous region it can
   prove;
2. a semantics-equivalent compiled classifier evaluates only the residual
   coordinates and returns ordered classification outcomes; and
3. the ordinary interpreter replays selected cases to derive mechanism and
   incidence evidence.

The coordinator alone validates canonically derived `CaseId` values, folds the
canonical transcript and appends evidence. A compiled kernel, local worker pool
or distributed worker is therefore an execution detail, never an alternative
source of truth. Parallel work is admitted only as disjoint, authenticated
evidence chunks bound to the same checked query and coverage obligations; the
coordinator verifies and idempotently merges those chunks into one evidence
root. This keeps result identity independent of worker placement and arrival
order. It is map/reduce-style acceleration, not mining or consensus.

Resource control is a passive operational failsafe, not an Explore research
thread. Work runs beneath the operator's 80-percent CPU/RAM policy and 6-GiB
process ceiling; unsafe pressure causes a checkpointed pause at the next work
boundary, after which the same journal can resume. Resource limits, worker
count and scheduling order never alter the bounded question or its evidence.
Normal runs should not repeatedly poll or report resource state beyond what is
needed to enforce that guard; only a tripped guard belongs in the user-visible
result.

## Reading the result responsibly

The hand-written audits are executable evidence about the checked-in Futuruna
model and their narrow declared domains. The completed 8,000-case exploration
is an exact Personskat result for its declared world, not a universal survey of
Personskat profiles or one-kroner transitions. The first-class surface remains
Experimental. Neither result is an individual tax determination.
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
