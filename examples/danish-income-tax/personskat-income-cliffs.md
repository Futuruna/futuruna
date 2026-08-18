# I mapped where one more krone leaves you poorer

Can one extra krone leave you worse off in only one unusual tax profile, or
does the same edge repeat across Denmark? I asked Futuruna to map it.

The search covered all 98 official 2026 municipal tax rows at the first
phase-out boundary, with and without church tax, at two standardized commuting
distances. It also followed two profiles through every step of the phase-out.
In total, Futuruna compared **490 adjacent income transitions** through **980
full personal-tax calculations**.

All **490 profile-boundary transitions were cliffs**. Inside this map, earning
one additional krone reduced modeled after-tax resources by between
**69.23 DKK and 170.02 DKK**.

An income cliff is a local boundary where a small increase removes more value
than it adds. It is not the ordinary marginal effect of a progressive tax
rate. Here, it comes from the low-income addition to the commuting deduction
falling in whole-thousand steps.

## The answer in one table

| Search layer | What varies | Adjacent transitions | Full tax calculations | Result |
|---|---|---:|---:|---:|
| Nationwide first step | 98 municipalities × church tax yes/no × 60/130 km total daily commute | 392 | 784 | 392 cliffs |
| Additional staircase steps | Steps 2–50 for Copenhagen/no church/60 km and Læsø/church/130 km | 98 | 196 | 98 cliffs |
| Complete map | The first anchor steps are already in the nationwide layer | **490** | **980** | **490 cliffs** |

The nationwide layer answers how the first legal step changes across municipal
rates, church tax and two commute profiles. The two staircases answer a
different question: does the same mechanism repeat through all 50 steps?

The two anchor profiles share their first transition with the nationwide
layer, so the union contains 490 distinct cases rather than 492.

The Copenhagen case that prompted the wider search was therefore neither
unique nor the largest. Its 50-step track ranged from 69.23 to 69.47 DKK. The
broader map found losses more than twice that size.

## The largest cliff in the map

The largest modeled loss was **170.02 DKK**. It occurred **41 times** inside
the map. One such case was:

- municipality: **Læsø**
- church tax: **yes**
- commute: **130 km in total per workday**, for 203 workdays
- gross annual wage income: **342,499 → 342,500 DKK**

| Measure | Before | After | Change |
|---|---:|---:|---:|
| Gross annual wage income | 342,499.00 DKK | 342,500.00 DKK | **+1.00 DKK** |
| Low-income addition to commuting deduction | 30,800 DKK | 30,184 DKK | **-616 DKK** |
| Modeled final tax | 88,526.60 DKK | 88,697.62 DKK | **+171.02 DKK** |
| Modeled after-tax resources | 253,972.40 DKK | 253,802.38 DKK | **-170.02 DKK** |

Here, *modeled after-tax resources* means gross wage income minus modeled final
personal tax, calculated in øre. It does not include the cost of commuting,
housing support, child benefits, consumption taxes or other household cash
flows.

The arithmetic is simple once the full tax calculation has been made:

```text
+1.00 DKK gross income - 171.02 DKK additional tax = -170.02 DKK
```

Losing a deduction is not the same as losing that amount in cash. The full
personal-tax calculation determines what the smaller deduction is worth under
the stated municipal and church-tax rates.

The commuting addition fell by the same 616 DKK at every Læsø step. Exact
rounding inside the full calculation moved the after-tax loss between 170.00
and 170.02 DKK; it did not turn those steps into different legal mechanisms.

## One mechanism, repeated

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
commuting rates and doubled the maximum addition for the year. The baseline
2026 kilometre rates are set by [Executive Order no. 1333 of 20 November
2025](https://www.retsinformation.dk/eli/lta/2025/1333), before the increases
made by Law no. 616.

The words *for each 1,000 DKK* matter. An official [Skatteministeriet 2023
worked
calculation](https://skm.dk/tal-og-metode/satser/skatte-og-afgiftsberegning/skatteberegningseksempel-for-en-ugift-skatteyder-i-2023)
labels the phase-out input as income above the threshold in whole numbers of
1,000 DKK. Futuruna encodes the phrase in the same whole-step way.

That produces 50 boundaries:

```text
342,499 → 342,500 DKK
343,499 → 343,500 DKK
...
391,499 → 391,500 DKK
```

At each boundary, one complete 1,000-DKK step enters the phase-out. One krone
of extra gross income is added, but part of an existing deduction disappears
at once. For these fixed wage-only profiles, gross wage moves the phase-out
income one-for-one; other kinds of income in the law remain fixed at zero.

The two commute profiles exercise different sides of the same rule:

- At 60 km per workday, the addition is controlled by a percentage of the
  ordinary commuting deduction. Each step removes about 296–297 DKK for an
  ordinary municipality and 328–329 DKK where the enhanced outer-municipality
  rate applies.
- At 130 km per workday, the addition has reached its maximum. Each step
  removes exactly 616 DKK from that maximum.

This is why the wider map matters. It is not finding 490 unrelated tricks. It
is showing one legal mechanism under many standardized tax profiles, then
following two of those profiles down the full staircase.

## What changes across the country

The first boundary produced these ranges across all 98 municipal tax rows:

| Standardized profile | Cases | One smallest-loss witness | One largest-loss witness |
|---|---:|---:|---:|
| 60 km, no church tax | 98 | 69.47 DKK — Copenhagen | 86.26 DKK — Odsherred |
| 60 km, church tax | 98 | 71.41 DKK — Rudersdal | 90.52 DKK — Læsø |
| 130 km, no church tax | 98 | 144.08 DKK — Copenhagen | 162.01 DKK — Odsherred |
| 130 km, church tax | 98 | 148.09 DKK — Rudersdal | 170.02 DKK — Læsø |

The size changes because municipal rates, church tax, commute length and the
enhanced outer-municipality commuting rate change the value of the deduction
that disappears. The nationwide cross-section holds the income boundary fixed
so these profiles can be compared on the same step. The rates come from
[Skatteministeriet's official 2026 municipal-tax
table](https://skm.dk/tal-og-metode/satser/oversigt-over-kommuneskatter).

The two complete tracks show how the mechanism behaves over the whole
phase-out:

| Standardized anchor | Steps checked | Modeled loss range |
|---|---:|---:|
| Copenhagen, no church tax, 60 km | 50 | 69.23–69.47 DKK |
| Læsø, church tax, 130 km | 50 | 170.00–170.02 DKK |

These are held-constant model profiles, not claims about two real commuters.
In particular, 130 km in Læsø is a standardized tax-jurisdiction stress case,
not an assertion about a plausible road route on an island. Actual ferry,
route and documented-expense facts can invoke other rules.

## Turning the law inside out

An ordinary tax calculation starts with one set of facts and asks for one
result:

```text
one person's facts → Danish tax rules → one tax result
```

This exploration turns the relationship around:

```text
source-defined boundaries
× declared tax profiles
→ calculate both sides with the full model
→ keep net_after < net_before
→ select the smallest, largest and tied witnesses
```

The nationwide profiles are ordinary Futuruna lists. This is the construction
from the executable map:

```runa
= personskat_indkomstklint_kirkestatusser = [Falskt, Sandt]
= personskat_indkomstklint_pendlerafstande = [60, 130]
= personskat_indkomstklint_nationale_profiler = flat_map(
    kommunale_parametre_2026,
    |parametre: KommunaleParametre| {
        flat_map(
            personskat_indkomstklint_kirkestatusser,
            |betaler_kirkeskat: Boolsk| {
                map(
                    personskat_indkomstklint_pendlerafstande,
                    |daglige_befordringskilometer: Heltal|
                        personskat_indkomstklint_profil(
                            parametre,
                            betaler_kirkeskat,
                            daglige_befordringskilometer
                        )
                )
            }
        )
    }
)
```

Each profile is evaluated immediately before and after the boundary through
the same full `beregn_personskat` function. The question itself is a filter:

```runa
= personskat_indkomstklinter = filter(
    personskat_ligningsfradrag_gyldige_indkomstovergange,
    |overgang: PersonskatIndkomstovergang|
        overgang.netto_efter_øre < overgang.netto_før_øre
)
```

This is closer to property-based testing than to an ordinary tax calculator:
declare a finite domain, run the real composition, and retain every
counterexample. Futuruna uses `foldl` and `filter` to select extrema and ties,
while executable `?` statements check coverage, distinct keys and valid
commuting inputs.

The computer, mercifully, has more patience for 980 tax calculations than I
do.

## What the map establishes

The executable audit checks that:

- all 98 official 2026 municipal rows are present once and supported by the
  model
- the exact 25 municipalities named for the enhanced rate are derived from
  the municipality field
- all 490 transition keys are distinct
- both sides of all 490 transitions pass the commuting-deduction input checks
- **490 of 490** transitions are income cliffs
- the reported minimum and maximum are true extrema inside the declared map
- the maximum occurs in exactly **41** transitions

This is exhaustive for the declared map, not for every Danish taxpayer or
every possible interaction in Danish law. The national layer covers the first
phase-out boundary; only the two anchor profiles cover all 50 boundaries.

The map derives enhanced-rate eligibility from the [25 named
municipalities](https://skat.dk/borger/fradrag/koerselsfradrag/yderligere-information-om-koerselsfradrag).
The same rule separately names ten small-island residence variants. Those
remain outside this map because a municipal tax row alone cannot tell us
whether someone lives on a particular island.

The fixed facts are tax year 2026; adult; single; no spouse; no capital or
share income, pension, property tax, foreign social contributions, tax amounts
carried from earlier years or special tax arrangements; and ordinary travel
to an income-producing workplace. All facts remain fixed except the declared
municipality, church-tax status, commute profile and adjacent wage amounts.

Futuruna produces a result from its current encoded model. It does not replace
an official assessment or individual legal and tax advice.

## Run the map

The complete source is the [executable income-cliff
map](https://github.com/Futuruna/futuruna/blob/main/examples/danish-income-tax/personskat-income-cliffs.audit.runa).
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

The complete map performs 980 full personal-tax calculations. On the measured
development machine, the interpreter completed it in **4 minutes 23 seconds**;
runtime will vary by machine and competing work.

## The next questions

This map looks at one known discontinuity in current-year personal tax. The
same method can search for thresholds, exceptions and counterexamples across
other encoded rules: combined tax and benefits, contract clauses, compliance
requirements, or places where two individually reasonable rules interact in
an unreasonable way.

The cliff itself is small enough to hide inside the calm words *for each 1,000
DKK*. The unusual part is that we can ask the law for every witness in a
declared domain, inspect each answer, and rerun the map when the law changes.

One mechanism is enough to show the method. The rest of the rule system is
waiting to be questioned.

I suspect the more interesting maps will appear where two rule systems meet.

If you find a legal edge worth exploring, I would love to hear about it at
[research@futuruna.com](mailto:research@futuruna.com).
