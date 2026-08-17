# Can earning one more krone leave you with less?

Yes—at one boundary in Futuruna's current 2026 Danish income-tax model. For
the commuter described below, gross annual wage income rises by 1 DKK while
after-tax resources fall by **69.47 DKK**.

This does not mean that earning more generally leaves people worse off. It is
a local income cliff: a point where a small increase crosses a rule boundary
and removes a larger value. The threshold and reduction rules come from
official Danish sources; 69.47 DKK is the result produced by Futuruna's full
modeled tax calculation for the stated facts.

## The result

| Measure | Before | After | Change |
|---|---:|---:|---:|
| Gross annual wage income | 342,499.00 DKK | 342,500.00 DKK | **+1.00 DKK** |
| Low-income addition to commuting deduction | 14,826 DKK | 14,529 DKK | **-297 DKK** |
| Modeled final tax | 99,967.63 DKK | 100,038.10 DKK | **+70.47 DKK** |
| After-tax resources | 242,531.37 DKK | 242,461.90 DKK | **-69.47 DKK** |

Here, *after-tax resources* means modeled gross wage income minus modeled
final personal tax. Both are calculated in øre. It does not include housing
support, child benefits, consumption taxes, commuting costs or other household
cash flows.

The arithmetic is simple once the full tax calculation has been made:

```text
+1.00 DKK gross income - 70.47 DKK additional tax = -69.47 DKK
```

The interesting question is why one additional krone can produce 70.47 DKK
of additional tax.

## The case

The calculation holds everything fixed except annual gross wage income:

- tax year 2026
- Copenhagen municipality
- adult, aged 18 or older
- single, without a spouse and without church tax
- an ordinary commute of 60 kilometres in total per workday for 203 workdays
- no capital or share income, pension, property tax, foreign social
  contributions, tax amounts carried from earlier years or special tax
  arrangements

Futuruna calculates both incomes through `beregn_personskat`, the model's
ordinary full personal-tax calculation. Only gross wage income changes; every
other fact above stays fixed.

## What changed?

People with lower incomes can receive an addition to their ordinary commuting
deduction under section 9 C of the Danish Tax Assessment Act
(*Ligningsloven*). For 2026, [Skattestyrelsen states that the addition is
reduced between 341,500 and 391,500
DKK](https://skat.dk/borger/fradrag/koerselsfradrag/koerselsfradrag-befordringsfradrag),
and that the maximum addition is 30,800 DKK.

The [consolidated section 9 C, subsection
4](https://www.retsinformation.dk/eli/lta/2025/1500) reduces the addition's
percentage by 1.28 percentage points and its maximum by 2 percent for each
1,000 DKK above the income threshold. [Law no. 616 of 30 June
2026](https://www.retsinformation.dk/eli/lta/2026/616) raised the 2026
commuting rates and doubled the maximum addition for the year.

The words *for each 1,000 DKK* matter. An official [Skatteministeriet 2020
worked
example](https://skm.dk/tal-og-metode/satser/skatte-og-afgiftsberegning/skatteberegningseksempel-for-et-aegtepar-i-2020)
applying the same phase-out formula calculates income above the threshold in
whole numbers of 1,000 DKK. In that example, an excess of 45,961 DKK becomes
45 steps, not 45.961 steps.

Futuruna encodes the phrase in the same whole-step way. The 69.47 DKK result
depends on that reading: if the reduction were spread smoothly through each
1,000-DKK interval, it would not produce the same one-krone step.

That creates a staircase:

- at 342,499 DKK, income is 999 DKK above the 2026 threshold, so no complete
  1,000-DKK step has been crossed
- at 342,500 DKK, income is 1,000 DKK above the threshold, so the first step
  applies
- for this commute, that step reduces the low-income addition by 297 DKK
- a deduction is not cash: after the entire tax calculation is run again, the
  extra 1 DKK of wage income and the 297 DKK smaller deduction together
  increase modeled final tax by 70.47 DKK

This is different from an ordinary progressive tax bracket. When a higher
rate begins at a threshold, that rate normally applies only to the income
above the threshold; the income already earned is not taxed again, so total
after-tax income still rises. A cliff appears here because crossing the
boundary removes part of an existing deduction in one step.

The low-income addition takes another step down at each following
whole-thousand boundary. The result is local and repeated, not a claim that
every higher income leaves this person worse off.

## Turning the rules inside out

An ordinary tax calculation starts with one set of facts and asks for one
result:

```text
one person's facts -> Danish tax rules -> one tax result
```

An exploration turns that relationship around. It creates many exact sets of
facts, sends every one through the same rules, and keeps the results that
answer a question:

```text
candidate facts -> Danish tax rules -> compare results -> keep the cliffs
```

Futuruna performs this search with ordinary lists and `range`, `map`, `filter`,
`find` and `foldl`. Here, `=` names candidate values and results, `|` states a
condition that must hold, and `?` asks Futuruna to check it.

The 50 incomes immediately before the known phase-out steps are generated in
one line:

```runa
= personskat_indkomstklint_grænser =
    map(range(1, 51), |n: Heltal| 341499 + n * 1000)
```

`Heltal` is Futuruna's Danish name for an integer. `range(1, 51)` contains the
integers 1 through 50 because the final value is excluded.

The list begins at 342,499 and ends at 391,499 DKK. Each income is paired with
the following krone. The search therefore performs 100 full personal-tax
calculations: one before and one after every boundary.

After each pair has been calculated, the question itself becomes a small
filter:

```runa
= personskat_indkomstklinter = filter(
    personskat_ligningsfradrag_gyldige_indkomstovergange,
    |overgang: PersonskatIndkomstovergang|
        overgang.netto_efter_øre < overgang.netto_før_øre
)
```

The search then uses `find` to select the exact 342,499→342,500 case and
`foldl` to select a case with the largest loss. Executable `?` checks confirm
that all 50 pairs were covered, both cases in every pair passed the model's
commuting-deduction input checks, the selected amounts match and no found loss
is larger.

## What the search establishes

The executable search checks that:

- exactly 50 declared boundaries were searched
- both sides of all 50 pairs passed the model's commuting-deduction input
  checks
- all 50 pairs are income cliffs in this fixed 2026 model
- the 342,499→342,500 case has a 69.47 DKK loss, and no found case has a
  larger loss

This is an exhaustive result for the 50 declared boundary pairs and the fixed
facts above. It is not an exhaustive search of every Danish income, household
or benefit rule. It is also not an individual tax assessment. A real case must
use the person's actual facts and the current official calculation.

Those limits matter. The statute says *for each 1,000 DKK*, the ministry's
example applies that wording in whole steps, and Futuruna shows the
consequence when that mechanism works alongside the rest of the modeled tax
system.

## Run the exploration

The complete source is the
[executable income-cliff audit](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-income-cliffs.audit.runa).
It calls the full
[`personskat.calculate.runa`](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat.calculate.runa)
model. The [exploration
workbook](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/exploration-workbook.md)
teaches the same method so you can adapt it to another question.

From a Futuruna checkout:

```bash
runa check --frontend examples/danish-income-tax/personskat-income-cliffs.audit.runa
runa examples/danish-income-tax/personskat-income-cliffs.audit.runa
```

The second command performs 100 full personal-tax calculations and can take a
few minutes. Its output shows the checked conditions and the case with the
largest loss.

## What else can we ask?

Once a law is executable, we can ask more than *what happens in this one
case?*

We can search for the least tax inside a clearly stated income range. We can
ask whether any modeled case produces negative tax. We can find the first
threshold where a deduction changes, the most expensive exception in a
contract, or a counterexample to a rule we thought would always preserve a
value. We can combine tax with housing support and other transfers—provided
we define the household facts and the meaning of disposable resources before
we search.

A useful exploration begins with an exact question:

- What may vary?
- What stays fixed?
- What result are we comparing?
- Which cases are valid?
- What domain did we search completely?

Law often uses calm words such as *gradually*. A formal rule model lets us ask:
gradually, exactly how? The value lies in making the path from legal text to
human consequence visible, executable and open to inspection.

Careful legal exploration gives the law precise questions and preserves the
reasons for every answer.

If you find a legal edge worth exploring, I would love to hear about it at
[research@futuruna.com](mailto:research@futuruna.com).
