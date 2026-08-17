# Reader's Mind: Income Cliffs

This document holds the reader's point of view for [Can earning one more
krone leave you with less?](personskat-income-cliffs.md). It keeps the article
useful to someone who knows only that Futuruna can express and run formal
rules.

## The reader

The reader is curious about what executable law can reveal. They do not need
to know Danish tax terminology, the structure of the Personskat model or the
details of Futuruna's list functions. They care about three things from the
start:

1. Is the result real, and what exactly does it claim?
2. Why does one additional krone make this modeled person worse off?
3. How did Futuruna find the case, and can I use the same method?

The reader should finish with an exact answer to all three questions and a
clear sense of where the result ends.

## The reading journey

| Section | The reader enters knowing | The reader asks or feels | Risk to guard against | What the section delivers | The reader leaves knowing |
|---|---|---|---|---|---|
| Opening | Futuruna can model rules | "Can the answer really be yes?" | Reading one local result as a claim about every taxpayer or every extra krone | The immediate answer, year, amount and meaning of an income cliff | The answer is yes for one stated boundary in the current 2026 model: 1 DKK more gross wage income produces 69.47 DKK less after-tax resources |
| The result | There is one modeled income cliff | "Show me the numbers" | Confusing a 297 DKK reduction in a deduction with 297 DKK lost in cash | A four-row comparison, the metric and the arithmetic | The four amounts reconcile: gross income rises by 1 DKK, modeled final tax rises by 70.47 DKK and after-tax resources fall by 69.47 DKK |
| The case | The arithmetic is clear | "Whose case is this?" | Applying the result to a different municipality, household, commute or tax profile | Every fixed personal and tax fact | Every fixed fact is visible, and only gross wage income changes |
| What changed? | The profile and result are known | "Which rule creates the cliff?" | Treating an ordinary progressive bracket as a cliff, or treating the model output as an individual assessment | The rule, official sources, whole-step example and staircase explanation | The low-income addition to the commuting deduction falls in whole-thousand steps, and the official rule text and ministry example sit beside the explanation |
| Turning the rules inside out | The legal mechanism is understood | "How did Futuruna discover it?" | Assuming a special solver or unexplained search language is required | A plain-language search pipeline and two small Futuruna excerpts | Ordinary Futuruna lists create candidates, the full tax calculation evaluates each pair, a filter keeps the cliffs, and `find` and `foldl` select useful results |
| What the search establishes | The method is visible | "How complete is the answer?" | Mistaking a complete search of 50 declared boundaries for a complete search of Danish tax law | The exact domain, checked validity, result and limits | All 50 declared adjacent pairs were checked for one fixed profile; this is exhaustive inside that domain and no broader |
| Run the exploration | The reader trusts the result and method | "Can I inspect or reproduce it?" | Looking only at prose when the executable calculation is available | Direct links, commands and a realistic runtime expectation | The audit, full model, workbook and two commands are directly available; the full run can take a few minutes |
| What else can we ask? | One exploration is concrete | "What can I discover next?" | Searching broadly without defining facts, validity, metric or domain | Reusable questions and five facts to define before searching | The reader can formulate a disciplined next question about thresholds, extrema, counterexamples, contracts or combined rules |

## Concepts introduced in order

The article introduces only the ideas needed for the next question:

- **Income cliff:** a local boundary where a small income increase removes a
  larger value.
- **After-tax resources:** modeled gross wage income minus modeled final
  personal tax, measured in øre for this exploration.
- **Low-income addition:** an addition to the ordinary commuting deduction,
  not cash paid directly to the taxpayer.
- **Whole-thousand step:** the phase-out changes when a complete 1,000 DKK
  step above the threshold is crossed.
- **Finite exploration:** create a declared set of candidate facts, calculate
  each case with the same rules, compare the outcomes and keep the cases that
  answer the question.
- **`Heltal`:** Futuruna's Danish name for an integer.

Each term appears beside the place where the reader needs it. The article
does not require a separate glossary before the result makes sense.

## The evidence boundary

The reader can distinguish four layers without being burdened by them:

1. Official sources state the 2026 threshold, rates and phrase *for each
   1,000 DKK*.
2. The ministry's worked example applies the phase-out in complete
   whole-thousand steps.
3. Futuruna applies that method inside its current full personal-tax model for
   the fixed profile.
4. The executable exploration checks 50 declared boundary pairs and selects
   the 342,499→342,500 DKK case, where the modeled loss is 69.47 DKK.

This supports a precise public claim. It does not turn the article into an
individual tax assessment, and it does not imply that every increase in
income leaves a person worse off.

## What the reader can do afterward

After reading, someone with Futuruna's smallest basics can:

- explain the difference between an ordinary progressive bracket and a local
  income cliff
- reproduce the headline arithmetic from the result table
- identify the fixed facts and the exact metric being compared
- read the two small Futuruna excerpts without learning a new search syntax
- inspect or run the executable audit
- frame a new exploration by naming what varies, what stays fixed, what is
  compared, which cases are valid and which finite domain is searched

## Final editorial test

The article is ready for the reader when all of these answers are visible
without outside context:

| Question | Answer present |
|---|---|
| What happened? | 1 DKK more gross wage income leaves the modeled profile 69.47 DKK worse off at one boundary |
| To whom? | A fully stated 2026 Copenhagen commuter profile |
| Why? | A whole-thousand phase-out step reduces the low-income commuting addition by 297 DKK; the full modeled calculation raises final tax by 70.47 DKK |
| Is this generally true? | No; it is local, profile-specific and limited to the stated metric |
| What comes from official sources? | The threshold, rates, amendment and whole-step application |
| What comes from Futuruna? | The composed tax result and exhaustive comparison of the 50 declared pairs |
| How was it found? | Existing list, calculation, filtering, selection and executable-check capabilities |
| Can I verify it? | Yes; the model, audit, workbook and commands are linked |
| What can I try next? | A clearly bounded search for thresholds, extrema, counterexamples or interactions |

The final reading experience should feel like a careful discovery: surprising
at first, understandable by the middle and useful by the end.
