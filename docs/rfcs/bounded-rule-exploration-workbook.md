# Turning Rules Inside Out with `? explore`

Status: implementation workbook for the Experimental feature

Futuruna normally answers:

> What do these rules produce for these facts?

`? explore` asks the reverse question:

> For which permitted facts does this property hold or fail?

You name the property and define a finite world. Futuruna finds the values. You
do not supply a list of suspected thresholds.

The normative contract lives in
[Bounded Rule Exploration with `? explore`](bounded-rule-exploration.md). The
compiler parses and type-checks the five search clauses today. Search
execution, reporting, typed output rows and result continuations are subsequent
Experimental implementation slices described below. In particular,
`output as` and `after` are specified here but are not accepted by the current
compiler yet.

## Five search clauses, plus an optional continuation

| Clause | Meaning |
|---|---|
| `over` | The Boolean rule Futuruna should investigate |
| `find` | Whether Futuruna searches for cases where the rule fails or holds |
| `bounds` | Every value each relevant input may take |
| `boundaries` | Which integer input is compared with its following value |
| `output` | What counts as one finding and which case should be shown |
| `after` | Optional code receiving the terminal typed report after search |

`after` is not another search input. It cannot change the world or the answer.
It runs only after enumeration, replay and sorting.

The existing `output { ... }` form remains CLI-only. The specified
`output as Row` form lets Futuruna code consume the result afterward.

`find violations` looks for cases where the rule is false. `find matches` looks
for cases where it is true. Bounds define the world. Boundaries define the
movement. The output key defines what one answer means.

## 1. Begin with a property

Consider a synthetic support rule. Support disappears when income reaches
100,000. The rule is intentionally simple so the exploration contract is easy
to see.

```runa
# Household = Single | Couple

> support(household: Household, income: Int) -> Int {
    if income < 100000 {
        match household {
            | Single -> 10000
            | Couple -> 15000
        }
    } else {
        0
    }
}

> available_resources(household: Household, income: Int) -> Int {
    income + support(household, income)
}

> loss_after_next_step(
    household: Household,
    income: Int,
    step: Int
) -> Int {
    available_resources(household, income) -
        available_resources(household, income + step)
}

| next_step_never_hurts(
    household: Household,
    income: Int,
    step: Int
) ->
    available_resources(household, income + step) >=
        available_resources(household, income)
```

The question rule states the property we hope is true. It does not contain a
suspected failing income.

## 2. Define the complete world

The first query asks where the property fails:

```runa
? explore support_cliffs {
    over next_step_never_hurts(household, income, step)
    find violations

    bounds {
        household in values(Household)
        income in range(90000, 110000)
        step = 1
    }

    boundaries on income by step

    output {
        key [income_before = income]
        show [
            income_after = income + step,
            household,
            available_before = available_resources(household, income),
            available_after =
                available_resources(household, income + step),
            loss = loss_after_next_step(household, income, step)
        ]
        representative maximize loss
    }
}
```

Nothing in the query mentions `99999`. Futuruna receives an income interval and
the property; it finds the failing step.

### Ways to define a bound

```runa
x in [a, b, c]
```

means exactly those values.

```runa
x in named_values
```

means every distinct value in a pure finite list or set. This is the right form
for a dated table or an explicitly curated legal domain.

```runa
x in range(start, end)
```

means every integer from `start` through `end - 1`. The solver can keep the
interval symbolic; Futuruna does not need to allocate a list containing every
integer.

```runa
x in values(Type)
```

means every possible value of a type the compiler can prove finite.

For example:

```runa
household in values(Household)
church_tax in values(Boolsk)
```

`values(Household)` contains `Single` and `Couple`.
`values(Boolsk)` contains both Boolean values.

`values(Int)` is an error because integers have no finite complete set. A
product containing an integer field is also rejected unless that field is
exposed as a separately bounded query input. Recursive and collection-bearing
types require an explicit finite domain.

`values(Type)` means every value of one finite declared type. Use a named list
instead when the domain consists of recorded instances, such as dated municipal
parameters.

```runa
year = 2026
case = make_case(household, year)
```

fixes or derives a value rather than adding another independent dimension.

```runa
where case_is_valid(case, income)
```

restricts the declared world. For a boundary query, validity is required at
both `income` and `income + step`.

Futuruna rejects a query when a relevant input remains unbounded. It never
invents a value merely to make the exploration run.

## 3. Understand the boundary

```runa
boundaries on income by step
```

says that `income` is the transition axis. With `step = 1`, every considered
pair is:

```text
income -> income + 1
```

Both endpoints must remain inside the income domain.

The Boolean rule still defines the property. The boundary clause identifies
the axis, checks endpoint coverage and lets Futuruna focus its explanation and
solver work.

The compiler may notice thresholds, comparisons, integer division, rounding,
caps and rule-branch changes. Those observations can make the search faster.
They do not narrow the answer. The complete bounded Boolean question remains
the source of truth.

## 4. Decide what one answer means

The most important output line is:

```runa
key [income_before = income]
```

Both `Single` and `Couple` fail at the same income step. Because the key contains
only income, the exploration returns one finding:

```text
Exploration: support_cliffs
Status: COMPLETE

Different income steps where next_step_never_hurts fails: 1

99,999 -> 100,000
Representative household: Couple
Available before: 114,999
Available after: 100,000
Loss after the next unit: 14,999
```

The representative is `Couple` because the query asks for the greatest loss
inside that income-step group.

Change only the key:

```runa
key [household, income_before = income]
```

and the answer becomes two findings:

```text
Single, 99,999 -> 100,000
Couple, 99,999 -> 100,000
```

Both counts are correct. They answer different questions.

The search space determines what Futuruna examines. The output key determines
what Futuruna counts.

### Shown values and representatives

`show` controls what enters the result. Hidden solver assignments are not
printed automatically.

If a shown expression depends on variables omitted from the key, choose one
case explicitly:

```runa
representative first
representative maximize loss
representative minimize final_tax_øre
```

The choice is made separately within every key group. Equal objective values
use canonical domain order so repeated runs produce the same result.

### Use the result inside Futuruna

The specified typed form starts with the exact row to receive:

```runa
# SupportCliffRow(
    income_before: Int,
    income_after: Int,
    household: Household,
    available_before: Int,
    available_after: Int,
    loss: Int
)
```

Then change only the output header from the earlier query and append a
continuation:

```runa
output as SupportCliffRow {
    key [income_before = income]
    show [
        income_after = income + step,
        household,
        available_before = available_resources(household, income),
        available_after = available_resources(household, income + step),
        loss = loss_after_next_step(household, income, step)
    ]
    representative maximize loss
}

after report -> publish_support_report(report)
```

The row fields are exactly `key` followed by `show`. Futuruna rejects an extra,
missing, reordered or differently typed field.

A consumer must handle the status explicitly:

```runa
> publish_support_report(
    report: ExplorationReport(SupportCliffRow)
) -> () {
    match report {
        | ExplorationComplete(_, findings) -> {
            @ print("Complete rows: " + show(findings))
        }
        | ExplorationPartial(_, confirmed, reason) -> {
            @ print(
                "Partial: " + show(length(confirmed)) +
                " confirmed rows; " + reason
            )
        }
        | ExplorationUnknown(_, confirmed, reason) -> {
            @ print(
                "Unknown: " + show(length(confirmed)) +
                " confirmed rows; " + reason
            )
        }
        | ExplorationUnsupported(_, diagnostic) -> {
            @ print("Unsupported: " + diagnostic)
        }
        | ExplorationError(_, diagnostics) -> {
            @ print("Exploration error: " + show(diagnostics))
        }
    }
}
```

A histogram writer can replace the first `print` with an ordinary function
receiving `findings`. Partial or unknown rows can still be visualized, but they
must remain visibly labelled as incomplete.

`report` is not a global variable. It exists only inside `after`, because the
solver result exists only during the selected `runa explore` command. To keep
the result after the process ends, use the CLI artifact or an explicit effect
inside the continuation. Ordinary `run` and `build` never launch this work.

The canonical report is finalized before `after` runs. Changing only the
continuation leaves `query_hash` unchanged, although the full `program_hash`
may change. A continuation failure leaves that report intact and becomes a
separate nonzero command outcome. In JSON mode the canonical document keeps
stdout to itself; continuation console output is isolated to stderr.

## 5. Read completion status before reading the count

A count is meaningful only together with its status and key.

| Status | What it lets you say |
|---|---|
| `COMPLETE` with zero findings | The property has no counterexample in the declared world |
| `COMPLETE` with findings | Every distinct output key in the declared world was returned |
| `PARTIAL` | Search stopped before closure; shown findings are confirmed, but more may remain |
| `UNKNOWN` | The remaining symbolic question could not be decided |
| `UNSUPPORTED` | Exact analysis was unavailable for a reachable construct |
| `ERROR` | Validation failed before a report, or solver and execution disagreed |

The typed report preserves that distinction:

| Status | Typed report payload |
|---|---|
| `COMPLETE` | `ExplorationComplete(..., findings)` |
| `PARTIAL` | `ExplorationPartial(..., confirmed, reason)` |
| `UNKNOWN` | `ExplorationUnknown(..., confirmed, reason)` |
| `UNSUPPORTED` | `ExplorationUnsupported(..., diagnostic)` |
| `ERROR` | `ExplorationError(..., diagnostics)` |

Only the complete variant calls its rows `findings`. That type-level
difference prevents a partial count from quietly becoming a final published
count. Unsupported and error reports expose no row list.

An invalid declaration fails before `report` exists and therefore never calls
`after`. The typed `ExplorationError` variant is reserved for a terminal
solving, decoding or replay error after the query has type-checked.

A partial report says:

```text
Different income steps found so far: 37
Completeness has not been established.
```

It never presents 37 as the final total.

`COMPLETE` requires every domain to be finite, every relevant operation to have
exact supported semantics, every result key to be enumerated, every shown case
to replay successfully, and the remaining query to end in `UNSAT` or exact
finite exhaustion.

## 6. See what the engine proves

For the synthetic query, the formal task is:

```text
Find every distinct income for which there exists an allowed household such
that next_step_never_hurts(household, income, 1) is false.
```

After finding income `99999`, Futuruna blocks that key—not merely the complete
`Couple` assignment—and asks whether another income remains. Final `UNSAT`
establishes that no unseen income key exists in the declared interval.

Each shown result is then run through ordinary Futuruna execution. A solver
answer that does not replay identically is rejected as an implementation error.

This is why `? explore` is more than shorter syntax for `range`, `map` and
`filter`: it owns projection, closure and replay.

If the query has `after`, Futuruna next constructs the appropriate
`ExplorationReport(Row)` from the already replayed and sorted public rows and
calls the continuation once. Nothing the continuation does can reopen or alter
the solver question.

## 7. Apply the pattern to Danish personal income tax

The Personskat exploration starts with one Boolean rule that calls the
canonical model at two adjacent gross incomes.

The helper constructing `PersonskatInput` must state its fixed facts plainly:

- tax year 2026;
- adult and single;
- 203 workdays;
- a standardized eligible commute distance;
- residence supplied by the dated 2026 residence-profile dataset, including
  each municipality and the separately qualifying small-island variants;
- no capital or share income;
- no pension, property-tax, spouse or foreign-contribution inputs;
- no carried tax position or special tax arrangement.

The residence profile derives the model's outer-municipality or qualifying-
small-island fact. It is not an independent Boolean that the explorer may pair
with an unrelated municipality.

`PersonskatResidenceProfile` is the typed model input for that pairing.
`personskat_residence_profiles_2026` is its dated, source-backed finite list and
must include both the ordinary municipality residences and the separately
qualifying island residences before the broad query can claim that scope.

The question is:

```runa
| personskat_next_krone_never_hurts(
    residence_profile: PersonskatResidenceProfile,
    pays_church_tax: Boolsk,
    daily_commute_km: Heltal,
    gross_income_kroner: Heltal,
    step_kroner: Heltal
) ->
    personskat_net_resources_øre(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner + step_kroner
    ) >= personskat_net_resources_øre(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner
    )
```

The broad query is:

```runa
? explore personskat_income_cliffs {
    over personskat_next_krone_never_hurts(
        residence_profile,
        pays_church_tax,
        daily_commute_km,
        gross_income_kroner,
        step_kroner
    )
    find violations

    bounds {
        residence_profile in personskat_residence_profiles_2026
        pays_church_tax in values(Boolsk)
        daily_commute_km in range(0, 201)
        gross_income_kroner in range(0, 1_000_001)
        step_kroner = 1

        where personskat_profile_is_supported(
            residence_profile,
            pays_church_tax,
            daily_commute_km
        )
    }

    boundaries on gross_income_kroner by step_kroner

    output {
        key [income_before_kroner = gross_income_kroner]
        show [
            income_after_kroner =
                gross_income_kroner + step_kroner,
            net_loss_øre = personskat_net_loss_øre(
                residence_profile,
                pays_church_tax,
                daily_commute_km,
                gross_income_kroner,
                step_kroner
            ),
            residence_profile,
            pays_church_tax,
            daily_commute_km
        ]
        representative maximize net_loss_øre
    }
}
```

The query above remains CLI-only. The specified typed form can feed its
replayed rows directly into a histogram pipeline by declaring the matching
product:

```runa
# PersonskatIncomeCliffRow(
    income_before_kroner: Heltal,
    income_after_kroner: Heltal,
    net_loss_øre: Heltal,
    residence_profile: PersonskatResidenceProfile,
    pays_church_tax: Boolsk,
    daily_commute_km: Heltal
)
```

Its output and continuation are:

```runa
output as PersonskatIncomeCliffRow {
    key [income_before_kroner = gross_income_kroner]
    show [
        income_after_kroner = gross_income_kroner + step_kroner,
        net_loss_øre = personskat_net_loss_øre(
            residence_profile,
            pays_church_tax,
            daily_commute_km,
            gross_income_kroner,
            step_kroner
        ),
        residence_profile,
        pays_church_tax,
        daily_commute_km
    ]
    representative maximize net_loss_øre
}

after report -> write_personskat_income_histogram(report)
```

With the income-only key above, the histogram contains one row per distinct
income boundary and plots that boundary's maximum modeled loss across the
declared profiles. It is not the distribution of every profile-boundary
observation. The function should publish this complete boundary distribution
only from `ExplorationComplete`. It may show confirmed partial rows for
diagnostics, but must label them as incomplete. A separate query whose key also
contains the profile fields is required to histogram every profile-boundary
observation. Leaving the original `output { ... }` unchanged keeps the query
entirely CLI-driven.

There is no list of known income steps in this query. The dated residence list
defines the legal geography, church status covers every Boolean value, commute
and income have explicit ranges, and every other fact is fixed by the
documented profile builder.

With the key:

```runa
key [income_before_kroner = gross_income_kroner]
```

the result answers:

> At which different earnings levels does at least one permitted profile lose
> resources after earning the next krone?

It does not count the same income step once per municipality, church-tax state
or commute distance.

To count affected profile-step combinations, run a second query whose key also
contains residence profile, church-tax state and commute distance. That count
is a different result, not a larger number of legal thresholds.

The expected known sequence from the encoded § 9 C phase-out contains 50 such
earnings steps, beginning with `342499 -> 342500` and ending with
`391499 -> 391500`. Those values belong in acceptance evidence, not in the
query's candidate domain.

A full Personskat search may discover additional earnings steps caused by
other encoded rules or exact rounding. Such results are discoveries to replay,
classify and explain. They must not be discarded merely because the known
§ 9 C reference run contains 50.

The public result therefore separates:

- distinct earnings steps;
- the greatest replayed loss and its representative profile for each step;
- the changed rule branches in that representative's exact replay;
- profile-step combinations from a separate projection when that count is
  requested.

A representative trace can explain a result, but it is not automatically a
complete inventory of every mechanism among the hidden profiles sharing that
income key. In version one, an exhaustive mechanism count requires a typed
mechanism value in the model and a separate query that includes it in the key;
it cannot be inferred from the income-step count.

## 8. Build the feature in executable checkpoints

### Checkpoint 1: syntax and types

- Parse, format and type-check the synthetic query.
- Preserve every existing `?` proof form, including an invariant named
  `explore`.
- Preserve existing CLI-only `output { ... }` while adding the optional
  `output as Row` and `after report ->` forms.
- Diagnose missing, duplicate and out-of-order clauses.

### Checkpoint 2: domains

- Support explicit lists and pure named finite collections.
- Preserve end-exclusive `range` semantics without materializing large ranges.
- Enumerate `values(Type)` only when every inhabitant is provably finite.
- Diagnose the exact unbounded field in rejected types.
- Reject unbounded relevant inputs and cyclic derived values.

### Checkpoint 3: answer-set semantics

- Find the synthetic failing step without receiving `99999`.
- Return one result for `key [income]`.
- Return two results for `key [household, income]`.
- Enumerate until final `UNSAT`.
- Return a complete empty result for a property holding throughout its bounds.

### Checkpoint 4: representatives and replay

- Select the larger `Couple` loss deterministically.
- Preserve deterministic ties.
- Replay every key, shown value and objective through normal execution.
- Treat any disagreement as an error.

### Checkpoint 5: honest interruption

- Stop a search through a test resource limit.
- Retain already replayed results.
- Report `PARTIAL` and never imply that the current count is final.
- Exercise solver `UNKNOWN`, unsupported lowering and invalid-query statuses.

### Checkpoint 6: result contracts

- Add `runa explore FILE [--query NAME]`.
- Add deterministic human output.
- Add versioned `futuruna.explore.v1` JSON.
- Exclude timing, absolute paths, raw SMT models and hidden inputs from the
  canonical result.

### Checkpoint 7: typed result continuation

- Validate one declared concrete row product against key plus show.
- Construct rows only after representative replay and canonical sorting.
- Expose complete, partial, unknown, unsupported and error as distinct sum
  variants.
- Run only the selected root continuation, once, in a fresh environment.
- Keep continuation code out of the query hash and solver IR.
- Preserve the canonical artifact on continuation failure.
- Keep JSON stdout isolated from continuation effects.
- Prove that hidden assignments are unavailable to post-processing.

### Checkpoint 8: Personskat

- Lower a narrow fixed-profile Personskat query.
- Rediscover the known § 9 C sequence without threshold candidates in source.
- Replay the established first loss through `beregn_personskat`.
- Run the broad declared query and accept its discovered total rather than
  forcing the known sequence to be the entire answer.
- Group by earnings step, label representative rule provenance, and obtain
  profile-step observations through a separate projection.

### Checkpoint 9: permanent confidence

- Add parser, formatter, type, diagnostic, solver, projection and replay tests.
- Add a small solver-backed exploration canary.
- Add a differential case for key blocking and deterministic representatives.
- Add Personskat conformance evidence.
- Run the compiler semantic-change gates required by `CONTRIBUTING.md`.

## 9. Report a legal-model exploration responsibly

A complete exploration is complete for:

- the checked-in model revision;
- the declared domains and fixed facts;
- the supported reachable semantics;
- the output key named by the query.

It does not establish that the model contains every relevant legal rule, that
the source interpretation is correct, or that the bounded profiles represent
the population.

The useful public claim has four parts:

1. the exact question;
2. the exact declared world;
3. the meaning of one result key;
4. the completion and replay status.

That is enough for Futuruna to make an unusually strong statement without
making a larger one than the evidence supports.

An `after` continuation is an explicit publication sink, not additional
evidence. It is not passed hidden profiles or assignments, but its ordinary
program code may compute from the public rows and visible declarations. Such a
computation is new logic, not recovered solver provenance, and it cannot turn a
partial report into a complete one. Review its effects with the same care as
any exporter, especially for private legal or tax data.
