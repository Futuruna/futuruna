# Reader's Mind: Mapping Income Cliffs

This document holds the reader's point of view for [I mapped where one more
krone leaves you poorer](personskat-income-cliffs.md). The article should work
for someone who knows that Futuruna can express and run formal rules, but has
never seen the Danish personal-tax model or a finite rule-space search.

## The reader

The reader arrives with three questions:

1. Is the first Copenhagen result a curiosity, or does the effect repeat?
2. What rule can make one additional krone leave someone worse off?
3. How can an executable model turn a legal rule into a map of answers?

The reader should leave with exact answers, a visible evidence boundary and a
method they can reuse for another law, contract or compliance system.

## The reading journey

| Section | The reader enters knowing | The reader asks or feels | Risk to guard against | What the section delivers | The reader leaves knowing |
|---|---|---|---|---|---|
| Opening | Futuruna can calculate formal rules | "Is this one odd case or a pattern?" | Reading a bounded result as a claim about every taxpayer | The map, domain, calculation count and full result range | Futuruna found 490 cliffs among 490 declared transitions, with modeled losses from 69.23 to 170.02 DKK |
| The answer in one table | The headline result is broad | "What exactly was searched?" | Adding overlapping layers and reporting 492 cases | The national cross-section, two staircases and overlap-aware total | The 392 first-step cases and 98 additional anchor steps form 490 distinct transitions and 980 full calculations |
| The largest cliff | The pattern repeats | "Show me the strongest case" | Confusing a deduction reduction with an equal cash loss | A four-row witness, metric and arithmetic | One Læsø profile gains 1 DKK gross, pays 171.02 DKK more modeled tax and loses 170.02 DKK in modeled after-tax resources |
| One mechanism, repeated | The arithmetic reconciles | "Why does this happen?" | Treating an ordinary progressive bracket as a cliff | The statute, whole-thousand application and two deduction branches | The low-income commuting addition falls at 50 whole-thousand boundaries; commute length changes the size of the step |
| What changes across the country | The legal mechanism is known | "How much do local facts matter?" | Treating standardized profiles as real commuters or municipality as the only cause | Four matched national profile ranges and two full anchor ranges | Municipal and church rates, commute length and enhanced-rate eligibility move the modeled loss within the reported range |
| Turning the law inside out | The reader knows what varied | "How did Futuruna find every witness?" | Suggesting a hidden solver or special search syntax | A finite data pipeline and two small Futuruna excerpts | Lists create cases, the full model evaluates both sides, a filter keeps cliffs, and folds select extrema and ties |
| What the map establishes | The method is visible | "How complete is this?" | Calling the map exhaustive beyond its declared domain | Executable coverage, distinctness, validity, extrema and scope | Every declared transition is checked, while the national layer covers only the first boundary and the model excludes stated facts and systems |
| Run the map | The reader understands and trusts the claim | "Can I inspect and reproduce it?" | Leaving evidence behind prose | Direct links, exact commands and measured runtime | The model, executable map and workbook are available; the measured interpreter run took 4 minutes 23 seconds |
| The next questions | One exploration is concrete | "What else can executable law reveal?" | Ending with a recap instead of possibility | New directions and an invitation | The same method can search other thresholds, interactions, exceptions and counterexamples |

## Concepts introduced in order

- **Income cliff:** a local boundary where a small income increase removes
  more modeled value than it adds.
- **Modeled after-tax resources:** gross wage income minus modeled final
  personal tax, calculated in øre for this exploration.
- **Low-income commuting addition:** an addition to the ordinary commuting
  deduction, not a cash payment of the same amount.
- **Whole-thousand step:** a phase-out change triggered when another complete
  1,000 DKK above the threshold is reached.
- **Nationwide first-step cross-section:** the same income boundary evaluated
  across 98 municipal rows, two church-tax statuses and two commute profiles.
- **Anchor staircase:** one standardized profile followed through all 50
  phase-out boundaries.
- **Finite exploration:** declare cases, calculate each with the same rules,
  keep those that answer the question and check the coverage.

Each term appears where the reader first needs it. The article does not ask
the reader to learn the Personskat type graph before seeing the result.

## The evidence boundary

The reader can distinguish four layers:

1. Official sources state the 2026 thresholds, rates, municipal rows and
   phase-out formula.
2. An official worked calculation applies the formula in complete
   whole-thousand steps.
3. Futuruna runs both sides of every declared transition through the current
   full personal-tax model using fixed, visible assumptions.
4. The executable map checks 490 distinct transitions, keeps all 490 cliffs
   and proves the reported extrema inside that domain.

The national cross-section is complete for the first boundary and the stated
profile axes. Only the Copenhagen and Læsø anchors cover all 50 boundaries.
The result is model evidence, not an official assessment or individual tax
advice.

## What the reader can do afterward

After reading, someone with Futuruna's smallest basics can:

- explain why the effect is a discontinuity rather than an ordinary marginal
  tax rate
- reproduce the arithmetic for the 170.02 DKK witness
- state the 490-transition domain without double-counting the anchor cases
- distinguish official legal inputs from consequences calculated by Futuruna
- read the list construction and cliff filter
- inspect or run the executable map
- frame a new exploration by naming fixed facts, varied facts, metric,
  validity conditions and finite domain

## Final editorial test

The article is ready when these answers are visible without outside context:

| Question | Answer present |
|---|---|
| Is the original case the only one? | No; all 490 declared transitions are cliffs |
| Is it the largest? | No; the original Copenhagen track reaches 69.47 DKK, while the map reaches 170.02 DKK |
| What creates the cliffs? | The whole-thousand phase-out of the low-income addition to the commuting deduction |
| What was searched? | A 392-case national first-step cross-section plus 98 non-duplicating steps from two complete anchors |
| What is the strongest witness? | A standardized Læsø/church-tax/130-km profile loses 170.02 DKK at 342,499→342,500 DKK |
| What is exhaustive? | Every transition inside the declared 490-case map, with the stated commuting-input validity checks |
| What comes from official sources? | Thresholds, rates, municipal data, eligibility and whole-step application |
| What comes from Futuruna? | The composed personal-tax results, comparisons, counts and extrema |
| How was it found? | Existing list, model-evaluation, filtering, folding and executable-check capabilities |
| Can I verify it? | Yes; the model, executable map, workbook and commands are linked |

The final reading experience should feel surprising at first, inevitable once
the mechanism is visible, and useful by the end.
