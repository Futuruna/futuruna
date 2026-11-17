# RFC: AnswerGuide

Status: Proposed for Futuruna 0.6. Builds on [Calculation needs](calculation-needs.md),
[Needs in the language](needs-in-the-language.md) and
[Field metadata on declarations](field-metadata.md).

Code quoted from today's system is copied from the file and line named above it.
Code under a **Proposed** heading or marked `-- proposed` is new syntax or new
behaviour; nothing of it exists yet.

## Summary

A partial case says *which* facts are missing (its needs). It does not say
where the answer to each one comes from: the integrator's own data, a person
who is asked, or another calculation run separately. That depends on who
integrates the model, so it does not belong in the law. This RFC gives the
integrator one declaration for it, the **AnswerGuide**, and one verb that uses
it, **`answer`**:

```runa
-- proposed
= person_tax = answer(tax, case, endavu_guide, from_case(complete))
```

`answer(entry, case, guide, resolve)` type-checks only when the guide gives an
origin to every input path of the entry that can become a need. An incomplete
guide is a check error that lists each path without an origin and the rules
that read it. At run time `answer` asks the entry round by round, calls each
resolver once per round with all of that round's needs of its group, and
checks that the case holds what the guide says the integrator holds.

`@ calculate` is renamed `@ entry`, `interview` becomes `answer`, the
interview question moves out of `@ field` into the guide, and the generated
TypeScript SDK dispatches needs to resolvers keyed by the guide's groups,
exhaustively.

## Principle

The integrator is never exposed to more than it alone can define. That is
two things:

1. **which of its data is which fact**: the facts it holds, built into the
   case (`Held`), and
2. **the answers to the needs it can fill**: one resolver per origin group,
   called with that group's needs.

Everything generic belongs to a Futuruna runtime library, and everything that
can be derived from the model is generated. Every choice below is checked
against this; *What the integrator still carries* lists what does not pass yet.

| Who | What |
|---|---|
| the model (law) | what a fact is (label, help, unit, sources, key), which entry provides a field, which rules read which facts, how an amount finer than its unit is treated |
| the integrator | its groups, which paths it holds, the question it puts to its people, the resolvers |
| generated from model and guide | the need-able paths, the origin of every need, the held case type, the resolver types, unit converters, wire codecs |
| the runtime library | `undefined` → unknown, the round loop, dispatch by group, writing answers into the case, the held-fact check, keys of list elements, errors |

## Problem

### Where an answer comes from is implicit and scattered

Today the same question, "who provides this fact when it is missing?", is
answered in four places, none of which says it in so many words.

1. **In the law model, as metadata.** tax-service attaches the questions the
   customer answers to the `tax_report` entry. `src/tax_report.calculate.runa:18`
   imports them and line 137 attaches them:

   ```runa
   @ meta(tax_case_guide)
   @ calculate("Endavu investor tax report")
   > tax_report(tc: TaxCase) -> TaxReportOutcome {
   ```

   `src/input_guide.runa:3-13` says what the list means, in a comment:

   ```runa
   -- The questions of the tax report's interview. What each input field means
   -- and its unit is written on its declaration (`@ field`, src/facts.runa);
   -- `runa schema`, templates and workbooks show those labels.
   --
   -- Fields the customer answers, because Endavu's data does not hold them, also
   -- carry the question to ask; each group of questions names the sources it
   -- rests on. These are the tax liability, the declaration about holdings and
   -- share income outside Endavu, the spouse, the accounts with other providers
   -- (`ExternalDepot`) with their holdings, trades and dividends, and the listed
   -- share loss brought forward. On an Endavu account the same fields come from
   -- Endavu's data and are not asked.
   ```

   "Endavu's data does not hold them" is a statement about one integrator,
   written in a module of the law.

2. **In the types the model declares for it.** `src/sources.runa:17-39`
   declares two record families by hand, recognised by Futuruna only by the
   shape of their fields (calculations.md:214-219):

   ```runa
   -- Metadata for one input path of one calculation, where it depends on where
   -- a type is used; the rest is written on the declarations (`@ field`). A
   -- field's unit comes from its type.
   # CalculationField(path: String, label: String, help: String?)
   # CalculationGuide(fields: List(CalculationField))

   -- An input field the customer answers because Endavu's data does not hold it:
   -- the question to ask and what counts. A group of questions carries
   -- the sources it rests on as `attachments`.
   # CustomerQuestion(path: String, label: String, question: String, help: String)
   # CustomerQuestions(a) = CustomerQuestions(questions: List(CustomerQuestion), attachments: a)
   ```

3. **In the backend, as code.** The proof's adapter maps every input pattern
   of `aktiesparekonto_year` to a fact of its own domain
   (endavu-service-proof `src/infrastructure/investment/taxEngine/aktiesparekontoYearCase.ts:222-228`):

   ```ts
   /**
    * The fact behind each input the engine can need. Exhaustive over the SDK's patterns, so a fact the model adds
    * does not compile until it is named here.
    */
   const FACT_OF_INPUT: {
     readonly [P in keyof aktiesparekontoYearSdk.AktiesparekontoYearInputPatterns]: AktiesparekontoYearFact;
   } = {
   ```

   The map has 77 entries (lines 229-308). Which of those facts the backend
   holds and which it leaves out is written in prose, in PROOF.md:73-81
   ("These facts are left out of the case, so the engine reports them as
   Needs"), not in code.

4. **In the SDK's resolver registry.** `resolve()` sends each need to the most
   specific registered pattern, and a need no pattern covers silently becomes a
   question (calculations.md:1397-1398: "A need no registered pattern covers
   is a question."; `src/calculate/sdk_ts_runtime.ts:1721-1722` skips it, and
   1761-1762 files it under `questions`).

### Nothing checks that an interview can complete

- Nothing checks that `CustomerQuestions` and the facts Endavu holds together
  cover every fact `tax_report` can need. Of the 108 input paths of `TaxCase`
  at which a need can arise (counted from `runa schema` by the rules in
  [Which paths can become a need](#which-paths-can-become-a-need)), the 26
  `CustomerQuestion`s name 26. For the other 82 nothing says where the answer
  comes from.
- `FACT_OF_INPUT` is exhaustive, but over the SDK's *patterns*, not over the
  paths that can become needs. 11 of its 77 keys can never name a need: the
  required records `owner`, `account` and `carriedForward`, the list elements
  `instruments[]`, `openingPositions[]`, `yearEndPrices[]`, `events[]`,
  `account.ending.prices[]` and `account.transferIn.foreignDividends[]`, and
  the sum-typed fields `account.transferIn.givingInstitute` and
  `account.ending.receivingInstitute`, whose need is at `.$variant`
  (derived by script from `runa schema src/aktiesparekonto.calculate.runa` with
  v0.5.0; not a measurement of any tool). The compiler forces the backend to
  name facts that cannot be missing.
- The interview test in tax-service (`tests/interviews.scenario.runa:12`)
  answers every need from a complete case, so it states the order of the
  questions but not who answers them:

  ```runa
  = spouse_interview = interview(|c| query tax_report(c), demo_case(demo_outside, unknown), married)
  ```

### Two hand-kept lists of the same kind

`CustomerQuestions` (Runa, for `tax_report`) and `FACT_OF_INPUT` (TypeScript,
for `aktiesparekonto_year`) are both hand-kept lists keyed by an entry's input
paths, saying where the answer to a need comes from. One lives in the law
model and is read by Futuruna as field metadata; the other lives in the
backend and is read by nobody but the adapter. Neither knows of the other,
and the end-to-end explainer records where the backend's list ends
(tax-service-pitch `end-to-end/index.html:128`): "Nothing in endavu-service
answers a need or calls `resolve()`: the needs end in the backoffice
response."

### Not all missing facts are questions

The business scenarios show missing facts that are not the customer's to
answer (tax-service `docs/business-scenarios.md`): the closing cause of an
account (gap 1, lines 107-112: "The backend holds the closing day but not why
the account closed"), the giving institute's § 11, stk. 2 statement (gap 2,
lines 113-116), a fund's tax character off SKAT's ABIS list (gap 6, lines
131-132). PROOF.md:29 calls every need "the customer's questions". A
backend that cannot tell "we failed to hold X" from "ask the customer Y"
cannot route either.

## Design

### The AnswerGuide

**Proposed.** A guide is an ordinary typed value, built from types of a
prelude that joins every program that builds an `AnswerGuide` or calls
`answer` (as the needs prelude joins a program that asks a query,
needs-in-the-language.md, *Decisions*, "The prelude"):

```runa
-- proposed: the answer prelude
# AnswerGuide(i, g) = AnswerGuide(fields: List(EntryField(i, g)))
# EntryField(i, g) = EntryField(path: Path(i), origin: Origin(g))
# Origin(g) =
    Held(by: g) |
    Fetched(by: g) |
    Asked(by: g, question: String, help: Option(String)) |
    Entry(by: g)
```

`i` is the entry's input type and `g` a sum type the integrator declares (its
groups). The writer never spells these parameters: they are inferred from the
paths and values.

`Asked` carries no legal sources. The sources a fact rests on are what the
fact is, so they are the model's: `@ field(sources = ...)` on the declaration,
and every need already carries the rules that wait for it with their cited
paragraphs (`waiting`). An integrator choosing paragraphs to show with a
question would be defining law it does not own.

An **origin** says what happens when a fact is missing from a case, which is
exactly when there is a need:

| Origin | Means | When a need arises at its path |
|---|---|---|
| `Held(by = G)` | The integrator builds this fact into the case. | It is a defect in the integrator's data: `answer` stops with an error naming the path and `G`. |
| `Fetched(by = G)` | The integrator reads it when the entry needs it, from a source that is expensive, remote or scattered. | `G`'s resolver is called with it, batched with the round's other needs of `G`. |
| `Asked(by = G, question, help)` | A person is asked. `G` names who or through which channel (the customer, a caseworker). | `G`'s resolver is called with it; a person who has not answered yet leaves it open. |
| `Entry(by = G)` | The integrator runs the entry the law names for this field, separately, on the field's subject. | `G`'s resolver is called with it and runs that entry. |

The four origins follow the rule the backend proof arrived at (PROOF.md:383-386):
"Build the case up front from what the backend holds, so the engine's needs
are exactly the customer's questions. Use resolvers for sources that are
expensive, remote or scattered, where reading only what the engine asks for
saves real work, and for asking the customer". `Held` is "build what you
hold", `Fetched` and `Asked` are "resolve what you don't", and the guide says
which is which per path, where today it is prose.

`Asked` does not forbid the case from holding the fact. A fact the integrator
holds for some customers and asks of others (the owner's tax liability, held
when the customer profile declares Danish tax residence, PROOF.md:75) is
`Asked`: the case builder writes it when it can, and when it cannot, the need
goes to the customer. The guide speaks about needs; the generated held case
type (see *TypeScript*) requires exactly the `Held` paths and accepts any
other fact the integrator happens to have.

**Groups.** The integrator declares its groups as a sum type of constructors
without fields, on its own side. This is the PO's first sketch,
`# DataSource = SpouseData | HousingSituation | Children`, moved out of the law:

```runa
-- proposed
# AskYearSource = Ledger | Snapshots | GivingInstitute | Bookings | PreviousYear | Backoffice | Customer
```

`runa check` holds the groups to three rules:

- a group is used with one kind of origin only (`Held`, `Fetched`, `Asked` or
  `Entry`), so a resolver of the group receives needs of one kind: an asked
  group's needs all carry a question, a fetched group's none;
- every constructor of the group type is used by some item; an unused group
  would make the generated TypeScript demand a resolver that is never called;
- the group type is a plain sum of constructors without fields.

**Covering.** An item covers its own path. `Held`, `Fetched` and `Entry` also
cover every path below it, unless a more specific item names that path: the
most specific item wins, as for field metadata (calculations.md:191-192, "For
each input path the most specific metadata wins, whole") and for resolver
patterns (calculations.md:1357-1360). `Asked` covers its own path only,
because a question is about one fact: one question text shown for every field
below a record would claim to ask what it does not. "Its own path" includes
the need of the field itself: the existence of an optional, the whole list,
the tag of a sum type or enum, a `Date` as one fact.

Two items with the same path are an error naming both, as two `@ field` at one
level are.

**`Entry` is only for fields the law marks.** The law says, intrinsically,
that another entry provides a field and for which subject:
`@ field(entry = refof(share_income), subject = pathof(TaxCase::taxpayer::spouse))`
(calculations.md:822-844). Whether the integrator runs that entry, holds its
result, or asks a person for the figure is the integrator's choice. So a guide
may give such a field any origin, and may give `Entry` to no other field:
`runa check` reports `Entry` on a path whose field has no `entry` in its
`@ field`, naming the field. A fact the integrator computes by other means
(the previous year's carried-forward balances, an account's engine outcome in
the eKapital report, `src/ekapital_ask.runa:54-55`) is `Fetched`: the law does
not say which entry provides it, so the guide cannot claim one.

### Path forms

Guide paths are typed paths of the entry's input, written with `pathof` in a
`Path(i)` place, exactly as in a query test (needs-in-the-language.md, *Typed
paths*):

| Form | Example | Covers |
|---|---|---|
| a field | `pathof(TaxCase::taxpayer::fully_tax_liable_in_denmark)` | that fact |
| an optional | `pathof(TaxCase::taxpayer::spouse)` | whether it exists; with `Held`, `Fetched`, `Entry` also its fields |
| a list | `pathof(TaxCase::events)` | the whole list; with `Held`, `Fetched`, `Entry` also every field of every element |
| a list element's field | `pathof(TaxCase::opening_positions::isin)` | that field in every element (a list step is transparent) |
| an alternative's field | `pathof(TaxCase::events::Trade::amount_ore)` | that field of `Trade` elements only |
| a tag | `pathof(TaxCase::taxpayer::outside_endavu::$variant)` | which alternative |
| the root (**new**) | `pathof(TaxCase)` | with `Held`, `Fetched`, `Entry`: every path no other item names |

- **No indices, no keys.** A guide speaks of every element of a list: an
  index (`accounts[1]`) in a guide path is a check error. The origin of a fact
  does not depend on which element it is in; where it seems to (an external
  depot's holdings are asked, an Endavu account's are held), the fact is
  `Asked` and the case builder fills the elements it holds.
- **The root path** `pathof(T)`, with no segment, is new. It lets a guide say
  "everything else" once. Its value is the empty path.
- A `pathof` rooted at another type than the entry's input is a type error,
  as in a query.

### Which paths can become a need

A need arises where evaluation reads a value the case leaves unknown. Two
things decide whether a path can ever be one: whether the partial case can
leave it unknown (the input type), and whether the entry reads it (the rules).

**From the type.** The decoding table of calculation-needs.md (*Kinds of
missing*) and its open question on left-out records (lines 383-386) give the
paths at which a need can be written:

| Value at the path | A need at |
|---|---|
| a scalar, a unit or named scalar, an enum, a `Date` | the path (a `Date` as one fact, never its parts) |
| an optional | the path (existence), and the paths below it |
| a list, set or map | the path (the whole collection), and the fields of its elements |
| a sum type with payloads | `path.$variant`, and each alternative's fields |
| a required record | never the record (it decodes as `{}`); its fields |
| an element of a list that is a record | never the element; its fields |
| an element that is a sum type | `path[].$variant`, and its fields |
| an element that is a scalar | never (calculations.md:916-918: it cannot be written in a partial case) |

Call this set *T(E)* for an entry *E*. It is finite except for recursive input
types; there the walk stops at a type already on the path, and only an item at
or above the recursion covers what lies below (an error otherwise, naming the
type).

**From the rules.** A path of *T(E)* that no code reachable from *E* reads
can never be a need: there is nothing to wait for it. `runa check` computes,
for each path, the rules that read it, by following input values through the
entry's reachable code: a value computed from an input value carries that
input's paths, through bindings, calls, records built and taken apart, list
operations and closures. Where the analysis cannot follow a value (a value
passed whole to a builtin that accepts any type, compared with `==`, or
returned in the result), every path below it counts as read there. This is a
sound over-approximation: it may report a path as read that no case ever
waits for, never the reverse.

The **need-able paths** of *E* are the paths of *T(E)* that are read. For each
one the analysis keeps the readers, chosen as `waiting` chooses them at run
time (calculation-needs.md, *Decisions*, "Every consumer, not the first
reader"; calculations.md:788-801): the rules and functions in a law section
that consume the value, cited ones first, each with its section and cited
paragraphs.

Why both halves:

- *T(E)* alone is exact for what the partial case can express and cheap, but it
  asks the writer for origins of facts the law never reads, and it knows
  nothing to say about *why* a fact is needed.
- A precise set ("paths that some partial case actually waits on") is
  undecidable in general: it depends on which branches a case can take.
- The read analysis is what makes the error useful: it names the law that
  waits, which is what the PO asked the error to show.

A consequence, wanted: the need-able set depends on the rules. A law change
that starts reading a fact makes the integrator's `answer` a check error until
the guide gives it an origin; that is the moment a new question could first
reach a customer.

### `answer`

**Proposed.** `answer` asks an entry about a case, round by round, with a
guide and a resolver, and returns the rounds and the outcome:

```runa
-- proposed: the answer prelude, continued
# Answers(i) = Answers(from: i, paths: List(Path(i)))
# Asking(i, g) = Asking(group: g, paths: List(Path(i)))
# Round(i, g) = Round(paths: List(Path(i)), groups: List(Asking(i, g)))
# Outcome(o, i, g) = Answered(result: o) | Unanswered(open: List(Asking(i, g)), case: i)
# Answering(o, i, g) = Answering(rounds: List(Round(i, g)), outcome: Outcome(o, i, g))
```

```text
answer(e, case: I, guide: AnswerGuide(I, g), resolve: (g, List(Need(I)), I) -> Answers(I)) -> Answering(O, I, g)
```

where `e` names an `@ entry` with input `I` and result `O`. Like `query`,
`answer` is a built-in form: its first argument is the name of an entry, not
a value, and `runa check` rejects a name that is not one.

**The type rule.** `answer(e, case, guide, resolve)` is well typed only when
`guide` gives every need-able path of `e` an origin. `guide` must therefore be
known when the program is checked: a top-level binding of a ground value, as a
metadata binding is (calculations.md:83-84). An incomplete guide is a check
error at the `answer`, listing each uncovered path with its readers:

```text
-- proposed message; the readers shown are illustrative
tests/answers.scenario.runa:14: `answer(tax_report, …)`: the AnswerGuide `customer_questions_only` gives no origin to 82 paths of TaxCase that can become a need:
  income_year
    read by year_parameters (section parameters)
    read by …
  taxpayer.user_id
    read by …
  accounts
    read by …
  …
Give each an origin in the guide, or one item above them (`EntryField(path = pathof(TaxCase), origin = Held(by = …))`).
```

The error lists every path; a writer who places them one by one learns the
interview. Completeness is therefore a property of the `answer` call, checked
with the program, not a report a CI job has to read.

There is no ceiling to record. The clause audit records one because it runs a
solver that may not decide (calculations.md:536-557, `not_run`,
`gap_not_checked`); this check is a walk over the input type and over the
entry's reachable code, which always finishes.

**At run time**, each round:

1. queries the entry on the case, as `query e(case)` does, with the strict
   oracle and the completion test under `runa test`
   (needs-in-the-language.md, *Tests run the guarantee*);
2. stops with `Answered(result)` on `Complete`;
3. looks up each need's origin (the most specific item covering its path);
4. stops with an error on a need whose origin is `Held`:

   ```text
   -- proposed message
   tests/answers.scenario.runa:20: `answer(tax_report, …)`, round 1: the AnswerGuide `endavu_tax_report` says `income_year` is held by `Endavu`, but the case leaves it unknown
   ```

5. calls `resolve(group, needs, case)` once for each other group that has
   needs this round, with all of them, in the order the groups first appear
   among the round's needs;
6. writes each group's answers into the case with `fill(case, answers.from,
   answers.paths)`; an answer for a path the group was not given, or a path at
   which `answers.from` holds no value, is an error naming both;
7. stops with `Unanswered(open, case)` when the round answered nothing:
   `open` holds every need left, by group.

It terminates for the reason `interview` does (needs-in-the-language.md:184-185):
every round that does not stop answers at least one path.

`Round.paths` is the round's needs in the engine's order (today's
`Interview.rounds` entry); `Round.groups` is the same needs by group, in the
order the resolvers were called. A test states "the backend holds X; the
customer is asked Y, in this order" by building X into the case, giving Y an
`Asked` origin and comparing `groups`.

**Resolvers in Futuruna** are functions. A resolver gives its answers as a
case value that holds them and the paths it answers, so that answers of
different types at different paths stay typed: they are written by `fill`,
which already knows how. Today's oracle form, answering every need from a
complete case, becomes one resolver of the prelude:

```runa
-- proposed: the answer prelude, continued
> from_case(complete: i) -> (g, List(Need(i)), i) -> Answers(i) {
    |group, needs, case| Answers(from = complete, paths = map(needs, |n| n.path))
}
```

A test that answers groups differently writes its own:

```runa
-- proposed
> endavu_answers(customer_says: TaxCase) -> (TaxReportOrigin, List(Need(TaxCase)), TaxCase) -> Answers(TaxCase) {
    |group, needs, case| match group {
        | Customer -> Answers(from = customer_says, paths = map(needs, |n| n.path))
        | PriceFeed -> Answers(from = case, paths = [])
        | Endavu -> Answers(from = case, paths = [])
    }
}
```

For a test that does not care where answers come from, the prelude offers a
guide that fetches everything from one group:

```runa
-- proposed: the answer prelude, continued
# WholeCase = WholeCase
> whole_case_guide() -> AnswerGuide(i, WholeCase) { ... }   -- one item: the root path, Fetched(by = WholeCase)
```

so that today's `interview(|c| query e(c), partial, complete)` is
`answer(e, partial, whole_case_guide(), from_case(complete))`.

**The PO's sketch**, completed. The sketch marked only `spouse_status` as an
entry; `answer`, like `query`, takes an entry (a rule without `@ entry` has no
contract: no stable input type, labels or questions), so `tax` is marked too.
Answering any rule is the open question of needs-in-the-language.md:431-439.

```runa
-- proposed
# SpouseStatus(cohabiting: Bool, income: Int)
# Spouse(status: SpouseStatus)
# Person(income: Int, spouse: Spouse)

@ entry("The spouse status")
| spouse_status(spouse: Spouse) -> spouse.status

@ entry("Tax")
| tax(person: Person) -> person.income + spouse_status(person.spouse).income

# Source = Payroll | Customer

= person_guide = AnswerGuide(fields = [
    EntryField(path = pathof(Person::income), origin = Held(by = Payroll)),
    EntryField(path = pathof(Person::spouse::status::income), origin = Asked(by = Customer,
        question = "What did your spouse earn this year?", help = None))
])

= person_tax = answer(tax, Person(income = 500, spouse = unknown), person_guide, from_case(complete_person))
```

The need-able paths of `tax` are `income` and `spouse.status.income`; 
`spouse.status.cohabiting` is in *T(tax)* but nothing `tax` reaches reads it,
so `person_guide` is complete without it. `answer(spouse_status, …)` with the
same guide is a check error: `spouse_status` returns the whole status, so it
reads `status.cohabiting` too.

### Composition and the `Entry` origin are different things

These two must not be confused, and the RFC keeps them apart on purpose.

**An entry calling another entry is an ordinary call.** In the sketch, `tax`
calls `spouse_status(person.spouse)`. Under a query this composes without new
rules (needs-in-the-language.md:240-244): the unknowns inside `person.spouse`
have their paths from the outer case, evaluation propagates them with Kleene
logic, and the callee's needs arrive in the caller's `Needs` at the caller's
paths, with the callee's rules as the ones waiting. For the guide nothing is
special: the paths are `tax`'s own input paths, the read analysis follows the
call like any other, and the guide covers them like any other. No `Entry`
origin is involved, and none may be: `spouse.status.income` has no `entry` in
its `@ field`.

**The `Entry` origin is a fact the integrator obtains by running another
calculation separately.** The input *holds a result*, not the facts behind it:

```runa
-- quoted: calculations.md:826-831
# TaxCase(
    taxpayer: Taxpayer,
    @ field(label = "Spouse's share income", entry = refof(share_income), subject = pathof(TaxCase::taxpayer::spouse))
    spouse_share_income_ore: Int
)
```

`tax_report` does not call `share_income`; it reads a number. The law says
that number is what `share_income` returns for the spouse. Who runs it is the
integrator's business: run it now, on another case, with its own guide
(`Entry(by = SpouseAssessment)`), or read a stored result (`Held` or
`Fetched`), or ask (`Asked`). The callee's needs never become the caller's:
they are needs of another `answer` the resolver runs, on the subject's case.
The SDK's stack of `(entry, subject)` pairs still catches two entries that need
each other (calculations.md:1403-1405).

### Where a guide lives

Origins belong to the caller (decided). The law's own modules never declare
them, and `runa check` enforces it:

- **No guide in the law.** A binding whose value is an `AnswerGuide` may not
  be declared in a file that declares an `@ entry` or that such a file imports,
  directly or through other imports. The error names the guide, the file and
  the entry that imports it:

  ```text
  -- proposed message
  src/input_guide.runa:95: `tax_case_guide` is an AnswerGuide, but src/tax_report.calculate.runa (entry `tax_report`) imports this file: a guide belongs to the caller; write it in a file no entry imports
  ```

- **No `answer` in the law.** `answer`, like `query` and `unknown`, may not be
  reached from an `@ entry` (needs-in-the-language.md:276-280); the check
  reports it with the call chain.
- **Recognised by type.** A guide is a binding of type `AnswerGuide`; no
  annotation and no file name is needed. A guide file conventionally ends in
  `.guide.runa` and imports the entries it guides, as a scenario file ends in
  `.scenario.runa`; `runa` does not depend on the name.

A guide does **not** name its entry. The entry is named where the guide is
used: in `answer(e, …)`, in `runa call --entry e --guide g`, in
`runa sdk ts --guide e=g`. One guide can serve two entries of the same input
type, and completeness is checked for each use. Saying the entry in the guide
too would say it twice, and the two could disagree.

### What moves where

| Today | Meaning | 0.6 |
|---|---|---|
| `@ field(label, help, unit, sources, key)` | what a fact is, true for every integrator | stays on `@ field` |
| `@ field(entry, subject)` | which entry provides a fact, for which subject | stays on `@ field` |
| `@ field(question = ...)` | whether and how a person is asked | `Asked(question = ...)` in the integrator's guide |
| `CalculationField(path, label, help)` under `@ meta` on the entry (tax-service `CalculationGuide`) | what a fact means at one use of a shared type, for one entry | `@ field(at = pathof(...), label, help)` before the `@ entry` |
| `CustomerQuestion(path, label, question, help)` | label: what the fact is; question, help: how the customer is asked | label on the declaration's `@ field`; question and help in `Asked` |
| `CustomerQuestions(attachments = ...)` | the sources the facts of a group of questions rest on | `@ field(sources = ...)` on the facts' declarations |

**`@ field` keeps label, help, unit, sources and key.** They say what a fact
is; a workbook, a schema, a need and the generated TSDoc show them to every
reader whoever integrates the model.

**The question leaves `@ field`.** Whether a person is asked at all is the
integrator's choice (the owner's tax liability is held for some customers,
PROOF.md:75), and so is whom: the customer is asked "Were you married?", a
caseworker "Was the taxpayer married?". A question in the law claims every
integrator asks it.

**Use-site labels stay in the law, as `@ field(at = ...)`.** tax-service's two
`CalculationGuide` records say what a fact means where a shared type is used
narrowly (`src/aktiesparekonto.calculate.runa:287-291`):

```runa
-- The year-end prices and a closed account's closing-day prices share
-- `AskYearPrice`; only here is the price the one on 31 December.
= ask_year_case_guide = CalculationGuide(fields = [
    CalculationField(path = pathof(AskYearCase::year_end_prices::price_micro_dkk), label = "Price on 31 December", help = Some(price_help))
])
```

"The price on 31 December" is true for every integrator; it is meaning, not
origin. Moving it into a guide would hide it from `runa schema`, templates and
unguided needs, and every second integrator would write it again. It moves to
an `@ field` with an exact path, standing before the entry:

```runa
-- proposed
@ field(at = pathof(AskYearCase::year_end_prices::price_micro_dkk), label = "Price on 31 December", help = price_help)
@ entry("Aktiesparekonto tax for one account and income year")
> aktiesparekonto_year(c: AskYearCase) -> AskYearOutcome {
```

`@ field(at = ...)` is the exact-path level of the existing precedence
(calculations.md:194-198, level 1), written without a record type to declare,
for the reason field-metadata.md:111-112 gives for `@ field` itself: "The
metadata needs no declared record type (`CalculationField`,
`CalculationMeta`, `impl Meta`) and no `path`". `at` is allowed only on an
`@ field` before an `@ entry`, and its path must be rooted at the entry's input
type. Field records under `@ meta` keep working for metadata kept in another
module (field-metadata.md:61-64).

This departs from the sketch in which `CalculationField(path, label, help)`
becomes the guide's `EntryField`. The name `EntryField` is kept for the guide's
item, which is an entry field with its origin; the label and help go where
they are true for everyone. See *Alternatives rejected*.

### Needs with their origin

**Proposed.** With a guide, each need carries its origin. In JSON
(`runa call FILE --entry e --guide g --partial`, where `FILE` declares the
guide `g`):

```json
{ "kind": "fact",
  "path": "owner.fully_tax_liable_in_denmark",
  "label": "Fully tax liable in Denmark",
  "help": "Fuld skattepligt efter kildeskattelovens § 1 and not resident elsewhere under a tax treaty (aktiesparekontoloven § 1).",
  "sources": [ "... from the declaration's @ field ..." ],
  "origin": { "$variant": "Asked", "by": "Customer",
              "question": "Are you fully tax liable in Denmark (fuldt skattepligtig til Danmark)?" },
  "waiting": [ "..." ] }
```

The envelope names the guide (`"guide": { "name": "endavu_ask_year", "hash": "…" }`).
A need's `question` member, read from `@ field` today, is filled from `@ field`
only for an unguided call during the deprecation (see *Migration*); with a
guide the question is in `origin` and nowhere else, so a need has a question
exactly when someone is asked.

The guide is compiled once into a total table from need-able path to origin
(the most-specific rule resolved at compile time). `runa call --guide`, the
TypeScript SDK and `answer` all read that table; `runa audit FILE --entry e
--guide g --json` prints it (`answer_guide.paths`: each need-able path, its
origin, the item that gave it, and its readers), so a reader can see which
paths fall to a broad item such as the root. The guide does not change the
entry's contract or its schema hash: a case document is as valid as before,
whoever answers it.

### TypeScript

**Proposed.** The integrator's TypeScript surface is the two things of the
[Principle](#principle) and nothing else: a function from its data to the
held facts, and one resolver per group. `runa sdk ts
src/aktiesparekonto.calculate.runa --guides guides/endavu.guide.runa --guide
aktiesparekonto_year=endavu_ask_year --out sdk/` generates, besides today's
modules, for each guided entry:

```ts
// proposed: generated sdk/guide.ts
/** The groups of the AnswerGuide `endavu_ask_year` for `aktiesparekonto_year`. */
export type AktiesparekontoYearGroup = "Ledger" | "Snapshots" | "GivingInstitute" | "Bookings" | "PreviousYear" | "Backoffice" | "Customer";

/**
 * The facts the integrator holds, by the guide: every `Held` path is required (inside a list element or an
 * optional, required when the element or the optional is there); every other fact may be given when it is known.
 * Leaving out a held fact does not compile.
 */
export type AktiesparekontoYearHeld = { readonly incomeYear: number; readonly account: { readonly accountId: AccountId; … }; … };

/** A need of `aktiesparekonto_year` with the origin the guide gives its path. */
export type AktiesparekontoYearGuidedNeed =
  | (AktiesparekontoYearNeed & { readonly origin: { readonly kind: "fetched"; readonly by: "Snapshots" | "GivingInstitute" | "Bookings" | "PreviousYear" } })
  | (AktiesparekontoYearNeed & { readonly origin: { readonly kind: "asked"; readonly by: "Backoffice" | "Customer"; readonly question: string; readonly help?: string } });

/** One resolver per group the guide fetches, asks or runs an entry for. A held group has none. */
export type AktiesparekontoYearResolvers = {
  readonly Snapshots: FetchResolver<NeedsOf<AktiesparekontoYearGuidedNeed, "Snapshots">, AktiesparekontoYearInputPatterns>;
  readonly GivingInstitute: FetchResolver<…>;
  readonly Bookings: FetchResolver<…>;
  readonly PreviousYear: FetchResolver<…>;
  readonly Backoffice: AskResolver<…>;
  readonly Customer: AskResolver<…>;
};

/** Answer `aktiesparekonto_year` from the held facts, with the guide `endavu_ask_year`. */
export function aktiesparekontoYearAnswer(
  transport: EngineTransport,
  held: AktiesparekontoYearHeld,
  resolvers: AktiesparekontoYearResolvers,
): Promise<Answering<AskYearOutcome, AktiesparekontoYearGuidedNeed, AktiesparekontoYearGroup>>;
```

- **The held facts are a generated type.** The case builder's only job is to
  say which of its data is which fact. It returns `AktiesparekontoYearHeld`,
  with `undefined` for a fact it does not have where the type allows it; the
  runtime turns `undefined` into unknown (as the SDK does today,
  calculations.md:1293-1296). A held fact the builder forgot is a compile
  error; `HeldMissingError` remains only for data the type cannot see (an
  element of a list the builder filled at run time from an untyped source).
- **Resolvers are keyed by group, and every key is required.** Leaving out a
  group the guide uses is a compile error, so no need can fall through to a
  silent question, as one that no pattern covers does today. The resolver
  shape is today's batched one (sdk_ts_runtime.ts:1589-1592): called once per
  round with all of its group's needs, it returns the function that answers
  each one. A resolver does not see the partial case or the round; it sees its
  needs, their keys and its own data.
- **Replies are typed by origin.** A fetched group replies `given(need,
  value)` or `notFound`; an asked group `given(need, value)` (a stored answer)
  or `ask`; an entry group `runEntry(need, held, resolvers)`, typed by the
  providing entry's own guided SDK. `given` is today's `answer(need, value)`,
  renamed because `answer` is now the loop. The value is typed by the need's
  pattern, in the model's units (next point).
- **Amounts in the field's unit are converted by generated code.** An
  integrator holds kroner; the model computes in øre or millionths. Converting
  is derivable from the model once a unit declares its scale (`# unit Ore = DKK
  / 100`, supported today, calculations.md:346-353), so the SDK generates it:
  `ore.fromDecimal("1740.25")`, `microDkk.fromDecimal(amount)`. What to do
  with an amount finer than the unit (refuse, or round, and how) is the
  model's decision, not the adapter's; until a model can declare it, the
  generated converter refuses, which is what the proof's adapter does today by
  hand (`aktiesparekontoYearCase.ts:74-81`, `wholeUnits`).
- **The outcome says who is missing what.** `Answering` is `Answered` with the
  result, its assumptions and the rounds, or `Unanswered` with `open` keyed by
  group, so an asked group's open needs (questions for a person) and a fetched
  group's (a source that had nothing) arrive apart.
- **Everything else is the runtime's**: the round loop, the origin of each need
  (attached from the generated table by the need's wire path, which names the
  alternative of a sum type, `account.ending.Closed.cause`, so two
  alternatives of one TypeScript pattern can have different origins), writing
  answers into the case, the `(entry, subject)` stack, the held-fact check. The
  WebAssembly engine and `runa call` without `--guide` are unchanged.
- **Pattern-keyed resolvers go.** The guide says what the patterns said, and
  checks it; keeping both would be two conventions for one thing. `resolve()`
  is replaced by the generated `<entry>Answer` and the runtime's `answer`.

### What the integrator still carries

Checked against the [Principle](#principle), these remain on the integrator's
side in this design, and should not:

- **Unit scales are not declared in tax-service.** `src/money.runa:11-13`
  declares `# unit Ore "DKK øre"`, `# unit MicroDkk "millionths of a krone"`
  and `# unit Micro "millionths of a unit"` without a scale, so no converter
  can be generated until the model adds `= DKK / 100` and its kin. The model's
  policy for an amount finer than its unit has no syntax yet (open question).
- **Help that explains the law to the customer.** Several `Asked` helps moved
  from `src/input_guide.runa` restate paragraphs ("Only a person who is fully
  tax liable under kildeskattelovens § 1 can open an Aktiesparekonto …"). The
  wording to a customer is the integrator's; the legal content is the
  model's, and each need already carries it (`waiting`, the field's `help`).
  tax-service should split them when it moves them.
- **Rendering `waiting` for a response.** The proof maps `waiting` to its own
  citation shape (`aktiesparekontoYearCase.ts:337-341`, `actOf`). It is generic
  and belongs in the runtime; whether the port carries citations at all is
  decided elsewhere.
- **Reading results in kroner.** The proof's `fromOre` (lines 108-111) is the
  inverse conversion; the same generated converters give `ore.toDecimal`.

Nothing else in the design asks the integrator for model knowledge: the
need-able paths, the origin of each need, the keys of list elements and the
check that the guide is complete are all derived.

### `@ entry`

`@ calculate` is renamed `@ entry`; the title carries over:
`@ entry("Aktiesparekonto tax for one account and income year")`.

The language and its tools already call the thing an entry:
`@ field(entry = refof(...))` names "the `@ calculate` entry that provides the
field" (calculations.md:79); a need has `kind: "entry"`, `entry` and `subject`;
`query` "prefixes a call of a calculation entry (`@ calculate`) by name"
(needs-in-the-language.md:104); the contract's first member is `"entry"`;
every command selects one with `--entry` (`src/bin/runa.rs:768`, "Select an
@ calculate entry"); the SDK exports `<entry>Entry` and `SCHEMA_HASH` keyed by
entry name. "calculate" is a verb standing where the language names a thing,
and the thing is not an action: calculations.md:14-15, "It does not change
rule evaluation and is not an effect". With `@ entry`, the annotation says what
every other place already says, and `answer(e, …)`, `query e(…)` and
`@ field(entry = …)` all name the same kind of thing by the same word.

## The tax-service example

### The law side

`src/sources.runa:17-32` and its three `impl Meta` lines for them (37-39) go:
the model declares no record types for interview metadata.
`src/input_guide.runa` goes. `src/tax_report.calculate.runa` loses
`@ import ./input_guide` (line 18) and `@ meta(tax_case_guide)` (line 137).

The use-site labels become `@ field(at = ...)`:

```runa
-- proposed: src/aktiesparekonto.calculate.runa:287-297
-- The year-end prices and a closed account's closing-day prices share
-- `AskYearPrice`; only here is the price the one on 31 December.
@ field(at = pathof(AskYearCase::year_end_prices::price_micro_dkk), label = "Price on 31 December", help = price_help)
@ entry("Aktiesparekonto tax for one account and income year")
> aktiesparekonto_year(c: AskYearCase) -> AskYearOutcome {
    ask_year_outcome(c.account.account_id, ask_year_tax_case(c))
}
```

```runa
-- proposed: src/ekapital_ask.calculate.runa:14-24
-- The opening year's help is about the year alone; the month and day keep
-- the labels the declaration composes.
@ field(at = pathof(AskReportCase::accounts::opened::year), label = "Account opened: year", help = "At the first institute when the account was received from another; 2019 or later.")
@ entry("Endavu eKapital Aktiesparekonto report")
> aktiesparekonto_report(rc: AskReportCase) -> AskReportOutcome {
    ekapital_ask_report(rc)
}
```

The 26 `CustomerQuestion` labels, and the sources their groups attach, say
what the facts are, so they move to the declarations in `src/facts.runa`
(which then imports `./sources`). Each group's attachments become one binding
there, under the group's name (`= spouse_sources = [Source(value =
aktieavancebeskatningsloven), Source(value = personskatteloven)]`, from
`src/input_guide.runa:45-48`), given to each fact of the group:

| Declaration in `src/facts.runa` | Label | `sources` |
|---|---|---|
| `Taxpayer.fully_tax_liable_in_denmark` | Fully tax liable in Denmark | `tax_liability_sources` |
| `OutsideEndavu` (the type: its `$variant`) | Declaration about holdings and share income outside Endavu | `outside_endavu_sources` |
| `OutsideEndavu.OutsideDeclared.other_share_income_ore` | Other share income | `outside_endavu_sources` |
| `Taxpayer.spouse` | Spouse | `spouse_sources` |
| `SpouseFacts.cohabiting_at_year_end` | Living together with the spouse at the end of the year | `spouse_sources` |
| `SpouseFacts.share_income_ore` | Spouse's share income | `spouse_sources` |
| `SpouseFacts.listed_share_net_ore` | Spouse's net result on listed shares | `spouse_sources` |
| `Account.kind` | Account type | `external_holding_sources` |
| `Account.acquisitions_reported_to_skat` | Purchases on the account are reported to Skattestyrelsen | `external_holding_sources` |
| `OpeningPosition.isin` | ISIN (today from the type `Isin`) | `external_holding_sources` |
| `OpeningPosition.quantity_micro` | Quantity on 1 January | `external_holding_sources` |
| `OpeningPosition.cost_basis_ore` | Remaining acquisition sum | `external_holding_sources` |
| `OpeningPosition.market_value_ore` | Market value on 31 December of the previous year | `external_holding_sources` |
| `OpeningPosition.acquisition_reported` | Acquisition known to Skattestyrelsen | `external_holding_sources` |
| `Event.Trade.date` | Trade date (there today) | `external_holding_sources` |
| `Event.Trade.isin` | ISIN (today from the type `Isin`) | `external_holding_sources` |
| `Event.Trade.side` | Buy or sell (today from the type `Side`) | `external_holding_sources` |
| `Event.Trade.quantity_micro` | Quantity traded | `external_holding_sources` |
| `Event.Trade.amount_ore` | Acquisition or disposal sum, with `help = trade_amount_help` | `external_holding_sources` |
| `Event.DividendPaid.date` | Payment date (there today) | `external_dividend_sources` |
| `Event.DividendPaid.isin` | ISIN (today from the type `Isin`) | `external_dividend_sources` |
| `Event.DividendPaid.decided` | Date the dividend was adopted or decided (there today) | `external_dividend_sources` |
| `Event.DividendPaid.gross_ore` | Gross dividend | `external_dividend_sources` |
| `Event.DividendPaid.danish_tax_withheld_ore` | Danish dividend tax withheld | `external_dividend_sources` |
| `Event.DividendPaid.foreign_tax_withheld_ore` | Foreign tax withheld | `external_dividend_sources` |
| `CarriedForward.listed_share_loss_ore` | Listed share loss brought forward | `loss_brought_forward_sources` |

19 labels are new; 7 exist today with the same text, on the field or its
type, and gain sources (a field-level `@ field` restates the type's label,
since the most specific metadata wins whole). `Event.Trade.amount_ore` today
gets `trade_amount_help` from the `CustomerQuestion` at
`src/input_guide.runa:64`; the same help is already on
`AskYearEvent.Trade.amount_micro_dkk`, so it moves to the declaration and the
question keeps none.

Today these labels and sources reach `tax_report`'s needs and workbook only
through the level-1 override that `CustomerQuestion` happens to be; after the
move every entry whose input holds these types shows them, guided or not.

### The guides

**Proposed.** `guides/endavu.guide.runa` holds Endavu's guides. For
`tax_report`, the questions and their help are those of
`src/input_guide.runa`, moved unchanged (generated from that file by script,
so the text is verbatim):

```runa
-- guides/endavu.guide.runa (proposed)
--
-- Where Endavu gets each fact of the tax report when a case leaves it out.
-- The questions are those of src/input_guide.runa, moved unchanged; the
-- labels and sources moved to the declarations (src/facts.runa).

@ import ../src/tax_report.calculate

-- Endavu: its own records, built into the case. PriceFeed: prices, read
-- when the engine needs them. Customer: the customer, asked.
# TaxReportOrigin = Endavu | PriceFeed | Customer

= endavu_tax_report = AnswerGuide(fields = [
    -- Everything not named below is in Endavu's records.
    EntryField(path = pathof(TaxCase), origin = Held(by = Endavu)),
    EntryField(path = pathof(TaxCase::year_end_prices), origin = Fetched(by = PriceFeed)),
    EntryField(path = pathof(TaxCase::accounts::ask_ending::Closed::prices), origin = Fetched(by = PriceFeed)),
    -- Tax liability (src/input_guide.runa:18-25).
    EntryField(path = pathof(TaxCase::taxpayer::fully_tax_liable_in_denmark), origin = Asked(by = Customer,
        question = "Are you fully tax liable in Denmark (fuldt skattepligtig til Danmark)?",
        help = Some("Only a person who is fully tax liable under kildeskattelovens § 1 can open an Aktiesparekonto and deposit on it (aktiesparekontoloven § 1 and § 9). An owner with limited tax liability (begrænset skattepligtig efter kildeskattelovens § 2) is taxed under aktiesparekontoloven § 16, which is not modelled: such a case is rejected."))),
    -- Holdings and share income outside Endavu (src/input_guide.runa:27-36).
    EntryField(path = pathof(TaxCase::taxpayer::outside_endavu::$variant), origin = Asked(by = Customer,
        question = "Will you tell us about all the shares and investment certificates you hold outside Endavu and all your other share income (aktieindkomst) for the year?",
        help = Some("OutsideDeclared: the accounts with other providers (ExternalDepot) in the case are all your holdings outside Endavu, and your other share income is given. OutsideNotDeclared: they are not known, and the share income tax is indicative. Holdings elsewhere count because the acquisition sum is that of your whole holding in the company (aktieavancebeskatningsloven § 26, stk. 2), and the tax is on your total aktieindkomst (personskatteloven § 8 a)."))),
    EntryField(path = pathof(TaxCase::taxpayer::outside_endavu::OutsideDeclared::other_share_income_ore), origin = Asked(by = Customer,
        question = "How much other share income (aktieindkomst, personskatteloven § 4 a) did you have in the year that is not on any account in this case?",
        help = Some("For example dividends and gains on unlisted shares. Leave out everything on your Endavu accounts and on the accounts with other providers you have declared. Negative for a net loss; 0 if there is none."))),
    -- The spouse (src/input_guide.runa:38-49).
    EntryField(path = pathof(TaxCase::taxpayer::spouse), origin = Asked(by = Customer,
        question = "Were you married (gift) at the end of the income year?",
        help = Some("If you were, the spouse's facts follow, all for the income year and before any transfer between the spouses."))),
    EntryField(path = pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end), origin = Asked(by = Customer,
        question = "Were you and your spouse living together (samlevende) at the end of the income year?",
        help = Some("Only then does an excess loss on listed shares move between the spouses (aktieavancebeskatningsloven § 13 A, stk. 3), an unused grundbeløb raise the other spouse's (personskatteloven § 8 a, stk. 4) and a negative share income be set off in the other spouse's positive one (stk. 6)."))),
    EntryField(path = pathof(TaxCase::taxpayer::spouse::share_income_ore), origin = Asked(by = Customer,
        question = "What was your spouse's share income (aktieindkomst) for the income year?",
        help = Some("The spouse's own aktieindkomst (personskatteloven § 4 a), before any transfer between the spouses; negative if it is negative."))),
    EntryField(path = pathof(TaxCase::taxpayer::spouse::listed_share_net_ore), origin = Asked(by = Customer,
        question = "What was the net amount of your spouse's dividends, gains and losses on listed shares (nettobeløbet af ægtefællens samlede udbytter, gevinster og tab) for the income year?",
        help = Some("Shares admitted to trading on a regulated market or a multilateral trading facility, after the spouse's own losses brought forward and before any transfer between the spouses (aktieavancebeskatningsloven § 13 A, stk. 3); negative when the spouse has a loss left over."))),
    -- Accounts with other providers, their holdings and trades: on an Endavu
    -- account these are in the case and never asked (src/input_guide.runa:51-69).
    EntryField(path = pathof(TaxCase::accounts::kind), origin = Asked(by = Customer,
        question = "Is this an account with Endavu, or a custody account (depot) with another bank or broker?",
        help = Some("FritDepot and Aktiesparekonto are Endavu accounts. ExternalDepot is a custody account with another provider, declared by you: its holdings and trades count because the acquisition sum is that of your whole holding in a company (aktieavancebeskatningsloven § 26, stk. 2)."))),
    EntryField(path = pathof(TaxCase::accounts::acquisitions_reported_to_skat), origin = Asked(by = Customer,
        question = "Does the bank or broker of this account report your purchases on it to Skattestyrelsen?",
        help = Some("A loss on listed shares and on investment certificates is deductible only if Skattestyrelsen has received the identity, number, acquisition date and acquisition sum of the purchase in time, or from a provider that reports them (aktieavancebeskatningsloven § 14 and § 19 D). Endavu fills this in for its own accounts."))),
    EntryField(path = pathof(TaxCase::opening_positions::isin), origin = Asked(by = Customer,
        question = "What is the ISIN of a security you held on this account on 1 January?",
        help = Some("The 12-character code on the account statement, one answer per security."))),
    EntryField(path = pathof(TaxCase::opening_positions::quantity_micro), origin = Asked(by = Customer,
        question = "How many shares or units of this security did you hold on the account on 1 January?",
        help = Some("The number on the account statement of 1 January."))),
    EntryField(path = pathof(TaxCase::opening_positions::cost_basis_ore), origin = Asked(by = Customer,
        question = "What is the remaining acquisition sum (anskaffelsessum) of these shares on 1 January?",
        help = Some("What the shares you still held cost, including costs, after the part of the acquisition sum that went to shares sold earlier (gennemsnitsmetoden, aktieavancebeskatningsloven § 26). Used for shares whose gain is taxed when sold (realisationsprincippet, § 23)."))),
    EntryField(path = pathof(TaxCase::opening_positions::market_value_ore), origin = Asked(by = Customer,
        question = "What were these shares or units worth at the end of the previous year?",
        help = Some("The value on 31 December of the previous year. Used for investment certificates whose gain is taxed every year (lagerprincippet, aktieavancebeskatningsloven § 23, stk. 8) and for an Aktiesparekonto."))),
    EntryField(path = pathof(TaxCase::opening_positions::acquisition_reported), origin = Asked(by = Customer,
        question = "Did Skattestyrelsen receive the purchase of these shares or units in time, from the bank or broker or from you?",
        help = Some("With the identity, number, acquisition date and acquisition sum. Without it a loss on them is not deductible (aktieavancebeskatningsloven § 14 and § 19 D)."))),
    EntryField(path = pathof(TaxCase::events::Trade::date), origin = Asked(by = Customer,
        question = "On which day was the trade made?",
        help = Some("A trade in the income year on an account with another provider. Trades are taken in date order: a sale takes its proportional part of the acquisition sum of the whole holding at that time (aktieavancebeskatningsloven § 26, stk. 2)."))),
    EntryField(path = pathof(TaxCase::events::Trade::isin), origin = Asked(by = Customer,
        question = "What is the ISIN of the security traded?",
        help = Some("The 12-character code on the trade note."))),
    EntryField(path = pathof(TaxCase::events::Trade::side), origin = Asked(by = Customer,
        question = "Did you buy or sell?",
        help = Some("BuySide for a purchase, SellSide for a sale."))),
    EntryField(path = pathof(TaxCase::events::Trade::quantity_micro), origin = Asked(by = Customer,
        question = "How many shares or units did you buy or sell?",
        help = Some("The number on the trade note."))),
    EntryField(path = pathof(TaxCase::events::Trade::amount_ore), origin = Asked(by = Customer,
        question = "What did you pay for the purchase, or receive for the sale, in Danish kroner?",
        help = None)),
    -- Dividends on accounts with other providers (src/input_guide.runa:71-83).
    EntryField(path = pathof(TaxCase::events::DividendPaid::date), origin = Asked(by = Customer,
        question = "On which day was the dividend paid?",
        help = Some("A dividend in the income year on an account with another provider: the payment date on the dividend note."))),
    EntryField(path = pathof(TaxCase::events::DividendPaid::isin), origin = Asked(by = Customer,
        question = "What is the ISIN of the security that paid the dividend?",
        help = Some("The 12-character code on the dividend note."))),
    EntryField(path = pathof(TaxCase::events::DividendPaid::decided), origin = Asked(by = Customer,
        question = "On which day was the dividend adopted or decided?",
        help = Some("Not later than the payment date. For an account with another provider, the date only sets a reporting deadline, which is not Endavu's; it does not change the tax."))),
    EntryField(path = pathof(TaxCase::events::DividendPaid::gross_ore), origin = Asked(by = Customer,
        question = "How much was the dividend (udbytte) before any tax was withheld?",
        help = Some("Aktieudbytte counts in aktieindkomst (personskatteloven § 4 a, stk. 1). Converted to DKK."))),
    EntryField(path = pathof(TaxCase::events::DividendPaid::danish_tax_withheld_ore), origin = Asked(by = Customer,
        question = "How much Danish dividend tax (udbytteskat efter kildeskattelovens § 65) was withheld from the dividend?",
        help = Some("It counts as payment of the tax on your share income (personskatteloven § 8 a, stk. 1 to 3). 0 when none was withheld, as for a foreign share."))),
    EntryField(path = pathof(TaxCase::events::DividendPaid::foreign_tax_withheld_ore), origin = Asked(by = Customer,
        question = "How much tax did the foreign state withhold from the dividend?",
        help = Some("0 for a Danish share. Converted to DKK."))),
    -- Listed share loss brought forward (src/input_guide.runa:85-93).
    EntryField(path = pathof(TaxCase::carried_forward::listed_share_loss_ore), origin = Asked(by = Customer,
        question = "How much loss on listed shares from earlier income years do you have left to deduct?",
        help = Some("Loss on shares admitted to trading on a regulated market or a multilateral trading facility that could not be deducted in earlier years and is carried forward (aktieavancebeskatningsloven § 13 A, stk. 2). As a positive amount; 0 if there is none.")))
])
```

What changed beyond the move: the guide holds no law. The labels and the
sources are on the declarations; what is left is who answers and how the
customer is asked. The two `Fetched` prices items and the root `Held` item
are new: they say what `src/input_guide.runa:12-13` said in a comment ("On an
Endavu account the same fields come from Endavu's data").

For `aktiesparekonto_year`, the backend's guide states what PROOF.md:73-81
says in prose. The question wording for the backoffice is the backend's to
write; the texts below are examples:

```runa
-- guides/endavu.guide.runa, continued (proposed)
# AskYearSource = Ledger | Snapshots | GivingInstitute | Bookings | PreviousYear | Backoffice | Customer

= endavu_ask_year = AnswerGuide(fields = [
    EntryField(path = pathof(AskYearCase), origin = Held(by = Ledger)),
    -- Built only from a customer profile that declares Danish tax residence (PROOF.md:75).
    EntryField(path = pathof(AskYearCase::owner::fully_tax_liable_in_denmark), origin = Asked(by = Customer,
        question = "Are you fully tax liable in Denmark (fuldt skattepligtig til Danmark)?",
        help = None)),
    -- Taken only from a snapshot built exactly on the day (PROOF.md:76-77).
    EntryField(path = pathof(AskYearCase::account::opening_cash_micro_dkk), origin = Fetched(by = Snapshots)),
    EntryField(path = pathof(AskYearCase::account::closing_cash_micro_dkk), origin = Fetched(by = Snapshots)),
    EntryField(path = pathof(AskYearCase::opening_positions), origin = Fetched(by = Snapshots)),
    EntryField(path = pathof(AskYearCase::year_end_prices), origin = Fetched(by = Snapshots)),
    EntryField(path = pathof(AskYearCase::account::ending::Closed::prices), origin = Fetched(by = Snapshots)),
    -- The giving institute's statement under § 11, stk. 2 (PROOF.md:78).
    EntryField(path = pathof(AskYearCase::account::transfer_in), origin = Fetched(by = GivingInstitute)),
    -- Bookings in another currency are left out (PROOF.md:81).
    EntryField(path = pathof(AskYearCase::events), origin = Fetched(by = Bookings)),
    -- Bookings the builder cannot classify (PROOF.md:80).
    EntryField(path = pathof(AskYearCase::events::$variant), origin = Asked(by = Backoffice,
        question = "What kind of booking is this?", help = None)),
    -- Not held (docs/business-scenarios.md, gap 1).
    EntryField(path = pathof(AskYearCase::account::ending::Closed::cause), origin = Asked(by = Backoffice,
        question = "Did the owner close the account, or did it close on the owner's death?",
        help = None)),
    -- Funds off SKAT's ABIS list for the year (docs/business-scenarios.md, gap 6).
    EntryField(path = pathof(AskYearCase::instruments::kind), origin = Asked(by = Backoffice,
        question = "What is this fund's tax character for the income year?",
        help = None)),
    -- The engine's own figures for the year before (PROOF.md:83-86).
    EntryField(path = pathof(AskYearCase::carried_forward), origin = Fetched(by = PreviousYear))
])
```

`carried_forward` is `Fetched`, not `Entry`: it holds four figures of the
previous year's outcome, not the result of an entry the law names
(`AskCarriedForward` has no `@ field(entry = ...)`, and an entry-provided
field must have the providing entry's whole result type, calculation-needs.md:343-349).

The sources of these facts are the model's: `AskOwner`'s `@ field` help
cites its paragraphs (`src/aktiesparekonto.calculate.runa:27-28`), each need
carries the rules that wait for it, and a source to show with the closing
cause or a fund's tax character belongs on those declarations, not here.

### The tests

**Proposed.** `tests/interviews.scenario.runa` becomes
`tests/answers.scenario.runa`; its spouse and closing-price parts read:

```runa
@ import ../fixtures/demo
@ import ../guides/endavu

= married = demo_case(demo_outside, demo_spouse)

-- The customer is asked whether there is a spouse, then the spouse's three
-- facts; everything else is in the case.
= spouse_answered = answer(tax_report, demo_case(demo_outside, unknown), endavu_tax_report, from_case(married))

| customer_asked_for_the_spouse_then_their_facts: spouse_answered -> map(spouse_answered.rounds, |r| r.groups) == [
    [Asking(group = Customer, paths = [pathof(TaxCase::taxpayer::spouse)])],
    [Asking(group = Customer, paths = [
        pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end),
        pathof(TaxCase::taxpayer::spouse::listed_share_net_ore),
        pathof(TaxCase::taxpayer::spouse::share_income_ore)
    ])]
]
| spouse_answers_end_in_the_complete_result: spouse_answered -> spouse_answered.outcome == Answered(result = tax_report(married))

-- The prices on the day an Aktiesparekonto closed: fetched once, from the price feed.
= ask_closed = ask_closed_case(ask_closed_prices)
= prices_answered = answer(tax_report, ask_closed_case(unknown), endavu_tax_report, from_case(ask_closed))

| closing_day_prices_fetched_once: prices_answered -> map(prices_answered.rounds, |r| r.groups) == [
    [Asking(group = PriceFeed, paths = [pathof(TaxCase::accounts[1]::ask_ending::Closed::prices)])]
]
```

The rounds are today's (`tests/interviews.scenario.runa:14-21` and 58-60);
what the test adds is who answers them, checked against the guide the backend
generates its SDK from.

### What the check reports against today's model

Counts from `runa schema` of v0.5.0 on tax-service `origin/main` (`ba2eb2c`),
by a script applying the decoding table above; the read analysis does not exist
yet, so these are the type-level sets *T(E)*, an upper bound of what the check
would list:

| Guide | Entry | Paths that can become a need (*T(E)*) | Without an origin |
|---|---|---|---|
| today's `CustomerQuestions`, each as `Asked` | `tax_report` | 108 | 82: every path not asked, from `income_year` and `taxpayer.user_id` to all of `accounts`, `instruments`, `year_end_prices` and four of the five `carried_forward` balances |
| `endavu_tax_report` above | `tax_report` | 108 | 0 |
| none | `aktiesparekonto_year` | 82 | 82 |
| `endavu_ask_year` above | `aktiesparekonto_year` | 82 | 0 |

In TypeScript patterns, where the alternatives of a sum share a pattern, the 82
paths of `aktiesparekonto_year` are 66 patterns. `FACT_OF_INPUT` names those
66 and 11 more that cannot be needs (listed under *Problem*).

Unused items are reported as warnings by `runa check`, not errors: an item none
of whose covered paths is need-able (shadowed by more specific items, or
covering only paths the entry never reads). An unused item does not make an
`answer` incomplete, and a law that stops reading a fact should tell the
integrator, not break its build. Expected for `endavu_tax_report`: none of
the items above covers only unread paths, but that can only be known once the
read analysis runs.

## The backend's view

**What disappears.** `FACT_OF_INPUT` (77 entries,
`aktiesparekontoYearCase.ts:226-309`) goes. Its first job, failing the build
when the model adds an input the backend has not thought about, moves into
`runa check`: building the engine package runs `runa sdk ts --guide
aktiesparekonto_year=endavu_ask_year`, which is the type rule of `answer` for
that pair, and lists the new path with the rule that reads it. Its second job,
naming the need for the backoffice in the backend's terms, is done by the
group and the label the need carries. The prose of PROOF.md:73-81 becomes the
guide. The proof's `elementKey` (lines 318-325) stays: keys are about which
element, not where an answer comes from.

**What the adapter reads.** **Proposed**, replacing `toNeed`
(`aktiesparekontoYearCase.ts:327-343`):

```ts
function toNeed(need: aktiesparekontoYearSdk.AktiesparekontoYearGuidedNeed, facts: AktiesparekontoYearFacts): AktiesparekontoTaxNeed {
  return {
    // "Customer" | "Backoffice" | "Snapshots" | "GivingInstitute" | "Bookings" | "PreviousYear": exhaustive in the SDK's type
    source: need.origin.by,
    ...elementKey(need, facts),
    label: need.label,
    question: need.origin.kind === "asked" ? need.origin.question : undefined,
    help: need.help,
    waiting: (need.waiting ?? []).map((waiting) => ({
      rule: waiting.rule,
      section: waiting.section,
      law: (waiting.paragraphs ?? []).map((reference) => ({ ...actOf(reference.act), paragraph: reference.paragraph })),
    })),
  };
}
```

**What the case builder becomes.** Its job shrinks to the first half of the
[Principle](#principle): which of the backend's data is which fact. It
returns the generated `AktiesparekontoYearHeld` instead of a hand-annotated
`PartialAskYearCase` (`aktiesparekontoYearCase.ts:121-172`), so a held fact
it forgets is a compile error rather than a need that reaches the
backoffice. The unit code of the adapter (lines 36-111: `wholeUnits`,
`toMicroDkk`, `toOre`, `toMicro`, `fromOre`) is replaced by the generated
converters once tax-service declares its units' scales; what stays is the
backend's own fact that a `Money` is in DKK (`inDkk`, lines 83-88). **Proposed**:

```ts
const held: AktiesparekontoYearHeld = {
  incomeYear: facts.incomeYear,
  owner: { fullyTaxLiableInDenmark: facts.ownerFullyTaxLiableInDenmark },  // Asked: may be undefined
  account: {
    accountId: accountId(facts.accountId.value),
    openingCashMicroDkk: facts.openingCash && microDkk.fromDecimal(inDkk(facts.openingCash, "opening cash")),
    …
  },
  …
};
const answering = await engine.aktiesparekontoYearAnswer(held, {
  Snapshots: () => () => notFound,        // the builder read every snapshot that exists
  GivingInstitute: () => () => notFound,  // no § 11, stk. 2 statement is held yet
  Bookings: () => () => notFound,         // no rate for another currency is held yet
  PreviousYear: () => () => notFound,     // the service chains the years before calling
  Backoffice: () => () => ask,
  Customer: () => () => ask,
});
if (answering.$variant === "Answered") return toStatement(answering.result, …);
const { Customer = [], Backoffice = [], ...gaps } = answering.open;  // questions for people, and sources that had nothing
```

Every resolver above is today's behaviour, now stated; the proof does not
resolve (end-to-end/index.html:128) and nothing forces it to. What the guide
buys the backend: `open.Customer` is the customer's questions, `open.Backoffice`
a caseworker's, and the `Snapshots`, `GivingInstitute`, `Bookings` and
`PreviousYear` entries are data gaps, apart. When a source becomes worth
reading on demand, its resolver changes and nothing else: the round still
calls it once with all of its needs (tax-service commit `bec731b`: one
`events[]` call received all 18 detail needs of six events; a resolver per
need would have been called once for the list and once for each of the 18).

## Rename table

One rule: where a name says "calculation" for the entry concept, it says
"entry". Where "calculation" names the computation a call performs, it stays.

| Today | 0.6 | Kind | Where |
|---|---|---|---|
| `@ calculate`, `@ calculate("title")` | `@ entry`, `@ entry("title")` | annotation | everywhere; `src/lib.rs:597` lists it |
| `interview(ask, partial, complete)` | `answer(entry, case, guide, resolve)` | prelude | `src/query.rs:159-161` |
| `interview_from` | (internal to `answer`) | prelude | `src/query.rs:163` |
| `Interview(o, i)` | `Answering(o, i, g)` | prelude type | `src/query.rs:149` |
| today's complete-case answering | `from_case(complete)`, `whole_case_guide()` | prelude | new |
| `CalculationField(path, label, help)` (tax-service type) | `@ field(at = …, label, help)` for meaning; `EntryField(path, origin)` is the guide's item | model / prelude | `src/sources.runa:20` |
| `CalculationGuide` (tax-service type) | removed | model | `src/sources.runa:21` |
| `CustomerQuestion`, `CustomerQuestions`, `InterviewGuide` (tax-service types) | `EntryField` with `Asked(question, help)` in an `AnswerGuide`; labels and `attachments` to the declarations' `@ field(label, sources)` | model → integrator and law | `src/sources.runa:26-32` |
| `@ field(question = …)` | `Asked(question = …)` | metadata key | calculations.md:77 |
| `CalculationField`, `CalculationMeta`, `CalculationMetaRole` in the docs' examples | `@ field(at = …)`; `EntryMeta`, `EntryMetaRole` where a record example stays | docs | calculations.md:51-53, 110-147, 230-244 |
| `resolve(query, initialCase, resolvers)` | `<entry>Answer(transport, held, resolvers)` and runtime `answer` | TypeScript | `sdk_ts_runtime.ts:1692` |
| `answer(need, value)` (reply) | `given(need, value)` | TypeScript | `sdk_ts_runtime.ts:1521` |
| `Resolution` (`Complete` / `Questions`) | `Answering` (`Answered` / `Unanswered`) | TypeScript | `sdk_ts_runtime.ts:1631` |
| `Resolvers` keyed by pattern | `<Entry>Resolvers` keyed by group | TypeScript | `sdk_ts_runtime.ts:1611` |
| `PreparedCalculation` | `PreparedEntry` | Rust API | `src/calculate.rs:7131` |
| "Select an @ calculate entry" | "Select an @ entry" | CLI help | `src/bin/runa.rs:768` |
| "Emit a typed @ calculate contract as JSON" | "Emit an entry's contract as JSON" | CLI help | `src/bin/runa.rs:729` |
| "Generate a TypeScript SDK for every calculation entry" | "… for every entry" | CLI help | `src/bin/runa.rs:734` |
| "source has no `@ calculate` entry" | "source has no `@ entry`" | diagnostic | `src/bin/runa.rs:2359` |
| "unknown calculation entry `{}`; available entries: {}" | "unknown entry `{}`; …" | diagnostic | `src/bin/runa.rs:2352` |
| "{source} declares no `@ calculate` entry" | "{source} declares no `@ entry`" | diagnostic | `src/calculate/sdk_ts.rs:286` |
| "calculation entry `{}` is declared twice" | "entry `{}` is declared twice" | diagnostic | `src/calculate/sdk_ts.rs:295` |
| "`@ calculate` must be followed by a top-level typed rule or function" | "`@ entry` must be …" | diagnostic | `src/calculate.rs:2207`, `2466` |
| "calculation `{}` field `{}` is provided by `{}`, which is not a calculation entry of this program; `entry` names an `@ calculate` function" | "entry `{}` field `{}` is provided by `{}`, which is not an entry of this program; `entry` names an `@ entry` rule or function" | diagnostic | `src/calculate.rs:3091` |
| "`query` asks a calculation entry (`@ calculate`); `{entry}` is not one" | "`query` asks an entry (`@ entry`); …" | diagnostic | `src/query.rs:608` |
| "no calculation entry `{entry}` in {} or the files it imports" | "no entry `{entry}` in …" | diagnostic | `src/query.rs:981` |
| "calculation entry", "calculation boundary" in docs | "entry" | docs | calculations.md, needs RFCs, README |
| page title "Typed Calculations" | "Entries" | docs | calculations.md:12 |

Kept, because they name the computation, not the entry: `runa call`,
`Calculated<R>`, `CalculationError`, `CalculationAssumption`,
`FUTURUNA_CALCULATION_CACHE_DIR`, `FUTURUNA_CALCULATION_JOBS`,
`FUTURUNA_CALCULATION_TRACE`, "a calculation is pure". Already "entry" and
unchanged: `--entry`, the contract's `"entry"`, `<entry>Entry`, `EntryNeed`,
`kind: "entry"`, `@ field(entry = …)`.

Not decided here (open questions): the wire schema identifiers
`futuruna.calculate.*`, and the `.calculate.runa` file-name convention.

## Migration

- **`@ calculate` → `@ entry`.** `runa fmt --migrate` rewrites it, keeping the
  title. In 0.6 `@ calculate` is still read, with a warning that names the
  migration; 0.7 removes it. The contract, its schema hash and every case
  document are unchanged: the annotation's name is not in the contract.
- **`interview` → `answer`.** `runa fmt --migrate` rewrites
  `interview(|c| query e(c), partial, complete)` to
  `answer(e, partial, whole_case_guide(), from_case(complete))`, a use
  `x.rounds` to `map(x.rounds, |r| r.paths)`, and `x.result == v` to
  `x.outcome == Answered(result = v)`. Any other use is listed
  (`file:line: left ...`), as the 0.5 migration lists what it cannot rewrite.
- **`@ field(question = ...)`.** No guide exists to move it into, so the
  migration does not move it. In 0.6 a question in `@ field` is still read,
  shown in unguided needs and the workbook, and warned about with the path and
  the advice to give it an `Asked` origin in a guide; with a guide, only the
  guide's question is shown. 0.7 removes the key.
- **Field records under `@ meta` on an entry** keep working. The docs teach
  `@ field(at = ...)` for one entry; the migration does not rewrite records,
  because their types are the model's own.
- **The SDK.** Pattern-keyed `resolve()` is removed; an SDK generated without
  `--guide` has calls and queries but no answering loop. tax-service's
  `engine-package/test/resolve.test.mjs` is rewritten with a guide that fetches
  `events` from one group; its assertion (one call receives every detail need)
  stays.
- **tax-service, by hand:** remove `src/input_guide.runa` and
  `src/sources.runa:17-32,37-39`; add the 19 labels and the group sources to
  `src/facts.runa`; declare the units' scales in `src/money.runa:11-13`;
  replace the two `CalculationGuide`s with `@ field(at = ...)`; write
  `guides/endavu.guide.runa`; move `tests/interviews.scenario.runa` to
  `answer`. The schema hash of `tax_report` changes once (its field metadata
  loses the questions and gains labels and sources from new provenance);
  those of `aktiesparekonto_year` and `aktiesparekonto_report` change once for
  the provenance of the moved use-site label; all three change once more when
  the units gain their scales.
- **The backend proof:** regenerate the SDK with the guide; build
  `AktiesparekontoYearHeld` with the generated converters instead of the
  hand-written unit code; delete `FACT_OF_INPUT`; name needs by `origin.by`.

## Alternatives rejected

- **Origins in the law** (`@ field(asked = True)`, `@ field(source = Ledger)`).
  The same fact is held by one integrator and asked by another (the owner's
  tax liability, PROOF.md:75), and a law module that names `Ledger` names one
  backend. Decided; the RFC builds on it.
- **A `DataSource` enum on fields** (`# DataSource = SpouseData |
  HousingSituation | Children`, used in `@ field`). It is the first
  alternative with a type: the law would still say where answers come from,
  for one caller. The PO's grouping survives as the integrator's group type.
- **Guide declared on the entry** (`@ guide(g)` on the entry, or
  `@ meta(g) for tax_report` from the integrator's file). On the entry, the
  law names its caller. Attached from outside, which guide applies would depend
  on which files a program imports, and a program could not hold two guides
  for one entry: a test comparing the backend's guide with a caseworker's, or
  `whole_case_guide()` beside the backend's. An argument of `answer` is checked
  where it is used and says which guide by name.
- **`EntryField(path, label, help, origin)`, the use-site label in the
  guide.** "Price on 31 December" is true for every integrator; in a guide it
  would vanish from `runa schema`, templates and unguided needs, and a second
  integrator would write it again. It stays in the law as `@ field(at = ...)`.
- **A `HeldOrAsked` origin** for facts held for some customers. `Asked`
  already means "when it is missing, ask"; the case builder writes it when it
  holds it.
- **Legal sources on `Asked`** (as `CustomerQuestions(attachments = ...)` has
  them today). Which paragraphs a fact rests on is the model's knowledge; on
  the integrator's side it would be law the integrator defines, against the
  [Principle](#principle). They go to `@ field(sources = ...)`, and the
  paragraphs that wait for a need come with it in `waiting`.
- **A hand-written case builder type**, as the proof annotates
  `PartialAskYearCase` today. Which paths the integrator must fill follows
  from the guide, so the type is generated (`<Entry>Held`).
- **Origins by key or by data** (`opening_positions[account kind =
  ExternalDepot]`). The guide is checked per path; an origin that depends on
  the case cannot be checked complete. The fact is `Asked`, and the builder
  fills the elements it holds.
- **One-need-at-a-time resolvers.** tax-service commit `bec731b` measured the
  batched shape on `aktiesparekonto_year` started without its events: 2
  resolver calls over 3 rounds, one of them receiving all 18 detail needs. A
  resolver per need is called per need, and a backend cannot fetch 18 rows by
  key in one query when it sees them one at a time.
- **Runtime resolution via Futuruna effects** (an entry performing
  `@ ask_customer(...)`). An entry is pure (calculations.md:17-22) and
  `query` is the one observer of unknown-ness, which must stand outside every
  evaluation it observes (needs-in-the-language.md:273-283). An effect in the
  entry would let a result depend on who was asked, and the guarantee "if a
  partial case returns `Complete(r)`, every completion returns `r`" would no
  longer be about the case alone.
- **Keeping `CustomerQuestions` as model metadata.** It works today only
  because a record with fields `path`, `label`, `question`, `help` is picked
  up as level-1 field metadata (calculations.md:214-219); it puts one
  integrator's questions in the law, overrides the declarations' labels whole,
  and nothing checks it is complete.
- **`@ needs` as the annotation's name.** Needs are one behaviour of an entry:
  it is called with complete cases too, and `Needs` is already a variant and
  a member name. "entry" is the word every other place uses.
- **Completeness as an audit report with a recorded ceiling.** A report is
  read when someone reads it; a type error stops the program that relies on
  the guide. The check needs no ceiling, because it runs no search.
- **The type-level set alone (*T(E)*) as the need-able paths.** Exact and
  cheap, but it demands origins for facts the law never reads, and it cannot
  say which rule waits for a path. **The precise set** (paths some case waits
  on) is undecidable.

## Open questions

- **Wire schema identifiers.** `futuruna.calculate.v1`,
  `futuruna.calculate.input.v2`, `futuruna.calculate.output.v1`,
  `futuruna.calculate.partial-output.v1`, `futuruna.calculate.compact.v1` and
  `futuruna.calculate.xlsx.input.v9`/`output.v2` say "calculate". Renaming
  them breaks every saved document and workbook; keeping them leaves the old
  word on the wire. Not decided here.
- **The `.calculate.runa` convention.** tax-service's entry files, the help
  text's examples (`runa schema model.calculate.runa`) and the SDK's usage
  line (`src/bin/runa.rs:2385`) use it. `runa` does not depend on it. Renaming
  it (`.entry.runa`) moves user files.
- **The read analysis.** It is new. Its precision through higher-order code
  (`map`, `flat_map`, closures) decides how many paths it reports as read that
  no case waits for, and its cost is paid on every `runa check` of a program
  with an `answer`. It must be measured on tax-service before the type rule
  depends on it.
- **A list assembled from two origins.** `opening_positions` of `tax_report`
  holds Endavu's positions and the customer's external ones. The list has one
  origin (`Held` above); the customer's elements reach it through the case
  builder. Whether a guide should say "elements added by the customer" is
  open.
- **The model's policy for amounts finer than a unit.** A unit can declare its
  scale today; nothing lets a model say whether a finer amount is refused or
  rounded, and how. Until it can, generated converters refuse. Where the
  declaration goes (the unit, or the field) is open.
- **Questions in several languages.** `Asked.question` is one string.
- **`later` and asked groups.** A round's `later` facts could be asked in the
  same round to save the customer a second visit; whether `answer` should, and
  whether that changes the rounds a test states, is open.
- **`answer` on any rule**, as the PO's sketch suggests: the open question of
  needs-in-the-language.md:431-439 applies unchanged.
- **Citations in the backend port.** Whether the port carries citations is
  decided elsewhere; this RFC does not depend on it. Explanations stay in
  Futuruna.
- **How long the old forms are read.** `@ calculate` and `@ field(question)`
  are read with a warning in 0.6 and removed in 0.7; whether one release is
  enough for tax-service and the engine package is the PO's call.
