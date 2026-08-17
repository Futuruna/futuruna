# Exploring Law with Futuruna

Futuruna can turn an encoded rule model inside out. Instead of supplying one
set of facts and asking for its result, define a finite set of possible facts,
run every case through the same canonical rules, and ask which results satisfy
your question.

The working pattern is:

```text
candidate facts -> canonical rules -> typed results -> filter -> rank -> prove
```

This workbook uses Futuruna's existing lists, `range`, `map`, `flat_map`,
`filter`, `foldl`, invariants, and proofs. The complete executable example is
[personskat-income-cliffs.audit.runa](personskat-income-cliffs.audit.runa).

## 1. Formulate the question

Begin with a relationship the model can evaluate, not a broad request for an
interesting answer.

The flagship question is:

> Across the encoded 2026 § 9 C phase-out steps for a stated tax profile, does
> increasing gross income by exactly 1 DKK ever reduce after-tax resources?

For gross income `g`, exact final tax `tax(g)`, and values measured in øre, the
searched condition is:

```text
g * 100 - tax(g) > (g + 1) * 100 - tax(g + 1)
```

That formulation identifies all of the pieces the program needs:

- the quantity that varies: gross income
- the comparison: two adjacent incomes, 1 DKK apart
- the result function: the canonical `beregn_personskat` rule
- the metric: gross income in øre minus exact final tax in øre
- the witness: a pair for which the metric falls

Other exploration questions have the same shape. “What is the least tax in
this search space?” asks for a minimum. “Can total tax be negative?” filters
for results below zero. “Does this rule always preserve a value?” searches for
a counterexample to the preservation condition.

## 2. Define the metric and fix the facts

Only vary facts named by the question. Hold the rest fixed and visible so that
every result has a precise meaning.

The income-cliff audit fixes tax year 2026, Copenhagen municipality, a single
adult without church tax, and an ordinary commute of 60 km per workday for 203
workdays. It supplies no capital income, share income, pension, property tax,
foreign social contributions, carried tax positions, or special tax
arrangements. Gross income is the only changing fact.

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

The audit stores both inputs, both outputs, the changed rule component, and
validity in one typed result:

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
the relevant amount changes at 1,000-DKK boundaries. The audit therefore
constructs the income immediately before each of the 50 boundaries:

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
- For several dimensions, construct their Cartesian product with nested
  `flat_map` and finish the innermost dimension with `map`.

## 4. Evaluate the canonical rules

Map each candidate to the canonical model. Do not reproduce selected formulas
inside the search: the point is to exercise the same rule graph used for an
ordinary calculation.

The income audit calculates both sides of every transition:

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

Run the complete exploration from the repository root:

```bash
./target/release/runa check examples/danish-income-tax/personskat-income-cliffs.audit.runa
./target/release/runa examples/danish-income-tax/personskat-income-cliffs.audit.runa
```

## 8. Extend to multi-dimensional questions

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
deduction eligibility, contract clause choices, effective dates, and exception
constructors. Add a dimension only when it belongs to the question, and prove
the resulting search-space size so an omitted branch cannot quietly narrow the
answer.

For a larger worked product search, see
[personskatteloven-konfiskatorisk.audit.runa](personskatteloven-konfiskatorisk.audit.runa),
which constructs combinations with nested `flat_map`, evaluates each case,
filters findings, and selects extrema with `foldl`.

## Reading the result responsibly

This exploration is executable evidence about the checked-in Futuruna model,
the declared search space, and the stated fixed facts. It is not an individual
tax determination. Verify the facts and the encoded phase-out interpretation
before relying on a witness outside research or model review.

Boundary and commuting sources for this example:

- [Ligningsloven, LBK nr. 1500 af 24 November 2025](https://www.retsinformation.dk/eli/lta/2025/1500)
- [LOV nr. 616 af 30 June 2026, § 1](https://www.retsinformation.dk/eli/lta/2026/616)
- [BEK nr. 1333 af 20 November 2025, §§ 1-3](https://www.retsinformation.dk/eli/lta/2025/1333)
- [Skatteministeriet: beløbsgrænser for 2025-2026](https://skm.dk/tal-og-metode/satser/regulering-af-beloebsgraenser/beloebsgraenser-i-skattelovgivningen-der-reguleres-efter-personskattelovens-20-2025-2026)
- [Skattestyrelsen: Kørselsfradrag (befordringsfradrag)](https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag)

The imported Personskat model carries source metadata for the remaining
components of the final-tax calculation.
