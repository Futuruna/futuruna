# RFC: AnswerGuide

> **Status: Superseded (before 0.6.0).** The integrator-written guide (paths
> to the integrator's groups) put integrator knowledge into Futuruna. 0.6
> instead generates each entry's AnswerGuide from the law and answers an
> entry with one `resolve` per round; see
> [The generated AnswerGuide](../reference/calculations.md#the-generated-answerguide)
> and [Answering an entry](../reference/calculations.md#answering-an-entry).
> The text below is kept as history.

Status: Proposed for Futuruna 0.6. Builds on [Calculation needs](calculation-needs.md),
[Needs in the language](needs-in-the-language.md) and
[Field metadata on declarations](field-metadata.md). Code marked `-- proposed`
does not exist yet; other code is quoted from the file and line named above it.

## Summary

A partial case says which facts are missing, not who answers them. That
depends on the integrator, so it does not belong in the law. The integrator
declares it in an **AnswerGuide**: input paths mapped to its own groups of
resolvers. **`answer`** uses it:

```runa
-- proposed
= person_tax = answer(tax, case, endavu_guide, from_case(complete))
```

There are two kinds of path:

- **Required.** Every path the guide does not name. The integrator builds the fact
  into the case, and a need there is a defect in its data.
- **Resolved.** Every path the guide names. The named group's resolver
  answers it, once per round, with all of that group's needs.

The generated TypeScript requires exactly the required facts and one resolver per
group. A new model field therefore fails the backend's build; it does not
reach a customer as a silent gap.

Questions stay on the fact's `@ field`. `@ calculate` is renamed `@ entry`,
and `interview` becomes `answer`.

## Principle

The integrator is never exposed to more than it alone can define:

1. which of its data is which fact (the required facts);
2. the answers to the needs it can fill (one resolver per group).

Everything generic belongs to a Futuruna runtime library, and everything
derivable from the model is generated:

| Who | What |
|---|---|
| model | what a fact is and how a person is asked for it (label, question, help, unit, sources, key); which entry provides a field; how an amount finer than its unit is treated |
| integrator | its groups, which paths each group resolves, the resolvers |
| generated | the need-able paths, each need's group, the facts type, resolver types, unit converters, wire codecs |
| runtime | `undefined` → unknown, the round loop, dispatch by group, writing answers, the required-fact check, running another entry, element keys |

## Problem

**Who answers a fact is implicit and scattered.**

1. **The law model holds it as metadata.** tax-service attaches the customer's
   questions to `tax_report` (`src/tax_report.calculate.runa:137-138`):

   ```runa
   @ meta(tax_case_guide)
   @ calculate("Endavu investor tax report")
   ```

   Which facts are asked is said in a comment (`src/input_guide.runa:7-13`):

   ```runa
   -- Fields the customer answers, because Endavu's data does not hold them, also
   -- carry the question to ask; each group of questions names the sources it
   -- rests on. [...] On an Endavu account the same fields come from
   -- Endavu's data and are not asked.
   ```

   The questions are the model's. "Endavu's data does not hold them" is a
   statement about one integrator, made inside the law.

2. **The model declares record types for it**, which Futuruna recognises only
   by the shape of their fields (calculations.md:214-219;
   `src/sources.runa:20-27`):

   ```runa
   # CalculationField(path: String, label: String, help: String?)
   # CalculationGuide(fields: List(CalculationField))
   # CustomerQuestion(path: String, label: String, question: String, help: String)
   # CustomerQuestions(a) = CustomerQuestions(questions: List(CustomerQuestion), attachments: a)
   ```

3. **The backend holds it as code.** endavu-service-proof `66e98411b`,
   `src/infrastructure/investment/taxEngine/aktiesparekontoYearCase.ts:210-216`:

   ```ts
   /**
    * The fact behind each input the engine can need. Exhaustive over the SDK's patterns, so a fact the model adds
    * does not compile until it is named here.
    */
   const FACT_OF_INPUT: {
     readonly [P in keyof aktiesparekontoYearSdk.AktiesparekontoYearInputPatterns]: AktiesparekontoYearFact;
   } = {
   ```

   The map has 76 entries (lines 217-295). Which of these facts the backend
   holds is prose: PROOF.md:75-82, "These facts are left out of the case, so
   the engine reports them as Needs".

4. **The SDK registry holds it**, where a need that no pattern covers silently
   becomes a question (calculations.md:1397-1398; `sdk_ts_runtime.ts:1721-1722`,
   `1761-1762`).

**Nothing checks that every fact has someone to answer it.** These figures are
derived by a script from `runa schema` (v0.5.0, tax-service `4284501a`)
applying the table under *Which paths can become a need*; they are not tool
output.

- `TaxCase` has 107 paths at which a need can arise. The 26
  `CustomerQuestion`s name 26 of them. For the other 81, nothing says who
  answers.
- `FACT_OF_INPUT` is exhaustive over the SDK's patterns, not over the paths
  that can become needs. The 81 need-able paths of `aktiesparekonto_year` are
  65 TypeScript patterns. The map names those 65 and 11 that can never be
  needs:
  - the required records `owner`, `account` and `carriedForward`;
  - six list elements, such as `events[]`;
  - two sum-typed fields, whose need is at `.$variant`.
- The interview test answers every need from a complete case. It states the
  rounds, not who answers them (`tests/interviews.scenario.runa:12`):

  ```runa
  = spouse_interview = interview(|c| query tax_report(c), demo_case(demo_outside, unknown), married)
  ```

**Two hand-kept lists, and missing facts that are not questions.**
`CustomerQuestions` (Runa) and `FACT_OF_INPUT` (TypeScript) are both
hand-kept lists over an entry's input paths, and neither knows of the other.
The backend's list ends in a response (tax-service-pitch
`end-to-end/index.html:128`: "Nothing in endavu-service answers a need or
calls `resolve()`: the needs end in the backoffice response").

tax-service `docs/business-scenarios.md` names missing facts that are not the
customer's to answer:

- the giving institute's § 11, stk. 2 statement (gap 2, lines 119-122);
- a fund's tax character off SKAT's ABIS list (gap 6, lines 138-139).

A third, the closing cause, is gone from the model since (gap 1, lines
109-118), because nothing but a report field read it. Yet PROOF.md:29 calls
every need "the customer's questions".

## Design

### The guide: path → group

```runa
-- proposed: the answer prelude
# AnswerGuide(i, g) = AnswerGuide(fields: List(EntryField(i, g)))
# EntryField(i, g) = EntryField(path: Path(i), by: g)
```

The prelude joins every program that builds an `AnswerGuide` or calls
`answer`, as the needs prelude does. In the types above, `i` is the entry's
input type. `g` is the integrator's group type: a sum of constructors without
fields. This is the PO's sketch `# DataSource = SpouseData | HousingSituation |
Children`, moved to the caller's side:

```runa
-- proposed
# Origin = PriceFeed | Customer
= endavu_tax_report = AnswerGuide(fields = [
    EntryField(path = pathof(TaxCase::year_end_prices), by = PriceFeed),
    EntryField(path = pathof(TaxCase::taxpayer::spouse), by = Customer),
    …
])
```

- **Required by default.** A required fact must be present: a need there is a defect
  in the integrator's data. Required facts have no resolver, so a group name for
  them would carry nothing.
- **A named path may be missing.** The case builder still fills it whenever
  it has it, and the group's resolver is asked only for what is missing. This
  is why `endavu_ask_year` (below) names 68 of its 81 paths while the backend
  builds nearly all of them in one query: naming a path says "this one may be
  missing, and here is who to ask", not "this one is not built".
- **Resolved, most specific item first.** A named path and every path below it
  belong to its group, unless a more specific item names a path below. This is
  the rule of field metadata (calculations.md:191-192) and of today's resolver
  patterns (calculations.md:1357-1360).
- **What a resolver does inside is the integrator's business.** It may read a
  database, ask a person, call a third party or run another entry. Futuruna
  only sees whether it answered.
- **Checks.** `runa check` reports:
  - two items with one path;
  - an unused group constructor, which would force an unused resolver;
  - a group type that is not a plain sum of constructors;
  - as a warning, an item every one of whose paths is named more specifically.
- **Paths** are typed `pathof` paths of the input, as in a query test. Each
  form covers what is below it:
  - a field: `taxpayer::spouse`;
  - a list, with its elements' fields: `events`;
  - an element field: `opening_positions::isin`;
  - an alternative's field: `events::Trade::amount_øre`;
  - a tag: `events::$variant`;
  - the whole input: `pathof(T)` (new).

  Indices and keys are errors, because who answers does not depend on which
  element. Where it seems to, the fact is resolved, and the required data fills
  the elements it has.

**A group nobody can fill is one line.** Some groups can only leave their needs
open. `Snapshots` is one: the builder has already read every snapshot that
exists. Its resolver is still required, and the runtime gives it a name:

```ts
// proposed
const resolvers: AktiesparekontoYearResolvers = { Snapshots: leaveOpen, GivingInstitute: leaveOpen, … };
```

This is the boring option, and the right one. Making such keys optional
would bring back today's silent fall-through (calculations.md:1397-1398).
Leaving the paths unnamed would make them required, so a missing snapshot would
become a data error instead of an open need. A required `leaveOpen` puts the
decision "nobody can fill this yet" in one visible line. When a source
appears, that line is the one that changes.

### Questions stay in the model

`@ field(question = …, help = …)` stays on the fact's declaration
(calculations.md:74-80). Every need carries it, with label, help, sources and
`waiting`.

- **The wording defines the fact legally.** A mis-phrased question yields a
  wrong legal fact that the engine cannot detect. That is model knowledge, and
  the integrator must not have to write it.
- **It is true for every integrator**, so it is said once. The app may
  translate or restyle it.

tax-service's `CustomerQuestions` therefore disappears. Its 26 questions and
helps move verbatim onto the declarations. What it said about one integrator
is now the guide's: which paths go to `Customer`.

### Which paths can become a need

These come from the decoding table of calculation-needs.md (*Kinds of
missing*, and lines 383-386 on left-out records):

| Value at the path | A need at |
|---|---|
| scalar, unit, named scalar, enum, `Date` | the path (a `Date` as one fact) |
| optional | the path (existence), and below |
| list, set, map | the path (the whole collection), and the elements' fields |
| sum type with payloads | `path.$variant`, and each alternative's fields |
| required record; list element that is a record | never the record; its fields |
| list element that is a sum type | `path[].$variant`, and its fields |
| list element that is a scalar | never (calculations.md:916-918) |

This set, *T(E)* for an entry *E*, is generated. In a recursive input type it
is cut where a type repeats, and the path above the cut must be named. Under a
guide, the required paths are *T(E)* minus the named subtrees. The generated required
type requires exactly those.

### `answer`

```runa
-- proposed: the answer prelude, continued
# Answers(i) = Answers(from: i, paths: List(Path(i)))
# Asking(i, g) = Asking(group: g, paths: List(Path(i)))
# Round(i, g) = Round(paths: List(Path(i)), groups: List(Asking(i, g)))
# Outcome(o, i, g) = Answered(result: o) | Unanswered(open: List(Asking(i, g)), case: i)
# Answering(o, i, g) = Answering(rounds: List(Round(i, g)), outcome: Outcome(o, i, g))
```

`answer(e, case: I, guide: AnswerGuide(I, g), resolve: (g, List(Need(I)), I) -> Answers(I)) -> Answering(O, I, g)`
is a built-in form like `query`.

- Its first argument names an `@ entry`.
- The guide must be a top-level binding of a ground value, as a metadata
  binding is (calculations.md:83-84), so that its paths are checked against
  `I` with the program.

Each round:

1. Query the entry on the case, as `query e(case)` does, with the strict oracle
   and the completion test under `runa test`.
2. Stop with `Answered(result)` on `Complete`.
3. Stop with an error on a need at a required path:

   ```text
   -- proposed
   tests/answers.scenario.runa:20: `answer(tax_report, …)`, round 1: `income_year` is required (the AnswerGuide `endavu_tax_report` names no group for it), but the case leaves it unknown
   ```

4. Call `resolve(group, needs, case)` once per group, with all of the round's
   needs of that group.
5. Write the answers with `fill`. An answer for a path the group was not given
   is an error.
6. Stop with `Unanswered(open, case)` when the round answered nothing.

`answer` terminates for the reason `interview` does
(needs-in-the-language.md:184-185). `Round.paths` is today's
`Interview.rounds` entry. `Round.groups` lets a test say "the backend holds X;
the customer is asked Y, in this order".

**Today's oracle form** becomes a resolver, plus a guide that resolves
everything:

```runa
-- proposed
> from_case(complete: i) -> (g, List(Need(i)), i) -> Answers(i) {
    |group, needs, case| Answers(from = complete, paths = map(needs, |n| n.path))
}
# WholeCase = WholeCase
> whole_case_guide() -> AnswerGuide(i, WholeCase) { ... }   -- one item: pathof(root), by = WholeCase
```

**The PO's sketch, completed.** `answer` takes an entry, like `query`, so
`tax` is marked too. Answering any rule stays the open question of
needs-in-the-language.md:431-439.

```runa
-- proposed
# SpouseStatus(cohabiting: Bool, income: Int)
# Spouse(status: SpouseStatus)
# Person(income: Int, spouse: Spouse)

@ entry("The spouse status")
| spouse_status(spouse: Spouse) -> spouse.status

@ entry("Tax")
| tax(person: Person) -> person.income + spouse_status(person.spouse).income

# Source = Customer
= person_guide = AnswerGuide(fields = [EntryField(path = pathof(Person::spouse), by = Customer)])
= person_tax = answer(tax, Person(income = 500, spouse = unknown), person_guide, from_case(complete_person))
```

### Composition and running another entry

These are two different things, and the design keeps them apart.

**An entry calling another entry is an ordinary call.** `tax` calls
`spouse_status`. Under a query the two compose without new rules
(needs-in-the-language.md:240-244). Kleene propagation brings the callee's
needs into the caller's `Needs`, at the caller's paths
(`spouse.status.income`). The guide treats them like any other path of `tax`.

**Running another entry separately is something a resolver may do.** In this
case the input holds a result, and the law says which entry gives it
(calculations.md:826-831):

```runa
# TaxCase(
    taxpayer: Taxpayer,
    @ field(label = "Spouse's share income", entry = refof(share_income), subject = pathof(TaxCase::taxpayer::spouse))
    spouse_share_income_ore: Int
)
```

That statement stays the law's. Whether the resolver runs `share_income`,
reads a stored figure or asks someone is its business. The runtime gives it a
helper for the first: `runEntry(need, facts, resolvers)` in TypeScript, and
`answer` on the providing entry in Futuruna. The other entry's needs belong to
that second `answer`, not to the caller. The `(entry, subject)` stack still
catches entries that need each other (calculations.md:1403-1405).

### Where a guide lives

- **No guide in the law.** An `AnswerGuide` binding in a file that declares an
  `@ entry`, or that such a file imports, is a check error.
- **No `answer` in the law.** `answer`, like `query`, may not be reached from
  an `@ entry` (needs-in-the-language.md:276-280).
- **Recognised by type.** A guide file is called `*.guide.runa` by convention
  only.
- **The guide does not name its entry.** The entry is named where the guide is
  used: `answer(e, …)`, `runa call --entry e --guide g`, `runa sdk ts --guide
  e=g`. One guide can serve two entries with the same input.
- **Its output.** With a guide, each JSON need gains `"by": "<group>"`. The
  engine is unchanged, and the entry's contract and schema hash are unchanged.

### TypeScript

```ts
// proposed: generated by runa sdk ts … --guides guides/endavu.guide.runa --guide aktiesparekonto_year=endavu_ask_year
export type AktiesparekontoYearGroup = "Snapshots" | "GivingInstitute" | "Bookings" | "PreviousYear" | "Backoffice" | "Customer";
/** Every path the guide does not name is required; a resolved fact may be given when known. */
export type AktiesparekontoYearFacts = { readonly incomeYear: number; readonly account: { readonly accountId: AccountId; … }; … };
export type AktiesparekontoYearGuidedNeed = AktiesparekontoYearNeed & { readonly by: AktiesparekontoYearGroup };
export type AktiesparekontoYearResolvers = {
  readonly [G in AktiesparekontoYearGroup]: Resolver<NeedsOf<AktiesparekontoYearGuidedNeed, G>, AktiesparekontoYearInputPatterns>;
};
export function aktiesparekontoYearAnswer(transport: EngineTransport, facts: AktiesparekontoYearFacts,
  resolvers: AktiesparekontoYearResolvers): Promise<Answering<AskYearOutcome, AktiesparekontoYearGuidedNeed, AktiesparekontoYearGroup>>;
```

- **The facts type is the backend's checklist.** `undefined` stands for not
  known, as today (calculations.md:1293-1296). A required fact left out does not
  compile, including one the model has just added. `MissingFactError` covers
  what the type cannot see.
- **Resolvers are keyed by group, and every key is required.** Each one keeps
  today's batched shape (sdk_ts_runtime.ts:1589-1592). It answers each need
  with `given(need, value)`, typed by the need's pattern, or leaves it `open`.
- **The outcome** is `Answered`, or `Unanswered` with `open` keyed by group.
  `open.Customer` then holds questions, and `open.Snapshots` holds facts a
  source did not have.
- **Amounts are converted by generated code.** A unit that declares its scale
  (`# unit Øre = DKK / 100`, calculations.md:346-353) gets `øre.fromDecimal`
  and `øre.toDecimal`. Refusing or rounding a finer amount is the model's
  decision, as it already is for the figures the model computes: tax-service's
  truncating rules state ASKL-ØRE-TRUNCATION (`src/ask.runa:147-151`,
  `src/money.runa:32-36`). Until a unit can declare its policy, the converter
  refuses. The proof's adapter does that by hand today
  (`aktiesparekontoYearCase.ts:63-69`, `wholeUnits`).
- **Pattern-keyed resolvers and `resolve()` go.**

Two things are still on the integrator's side, and should not be:

- **Unit scales.** tax-service declares none (`src/money.runa:11-13`), and
  there is no syntax yet for the finer-amount policy.
- **Rendering `waiting`.** This is the proof's `actOf`
  (`aktiesparekontoYearCase.ts:324-328`). It belongs in the runtime.

### `@ entry`

`@ calculate("title")` becomes `@ entry("title")`. The language already calls
the thing an entry in every other place:

- "the `@ calculate` entry that provides the field" (calculations.md:79);
- a need's `kind: "entry"`;
- `query` "prefixes a call of a calculation entry" (needs-in-the-language.md:104);
- the contract's `"entry"`;
- `--entry` ("Select an @ calculate entry", `src/bin/runa.rs:768`).

"calculate" is a verb standing where the language names a thing. That thing
"does not change rule evaluation and is not an effect" (calculations.md:14-15).

## The tax-service example

### The law side

These go:

- `src/input_guide.runa`;
- `src/sources.runa:17-39`;
- in `src/tax_report.calculate.runa`, line 18 (`@ import ./input_guide`) and
  line 137 (`@ meta(tax_case_guide)`).

**Deferred:** `@ field(at = …)` is not in 0.6; it moves to a later release.
Until then a use-site label keeps its 0.5 form, an `@ meta` record that names
the field by `pathof` (as the Danish examples' use-site records do).

The two use-site labels become `@ field(at = …)`. This is the exact-path
precedence level (calculations.md:194-198), written without a record type
(field-metadata.md:111-112):

```runa
-- proposed: src/aktiesparekonto.calculate.runa:287-297 (ekapital_ask.calculate.runa:14-24 alike)
@ field(at = pathof(AskYearCase::year_end_prices::price_micro_dkk), label = "Price on 31 December", help = price_help)
@ entry("Aktiesparekonto tax for one account and income year")
> aktiesparekonto_year(c: AskYearCase) -> AskYearOutcome {
```

The 26 questions move onto `src/facts.runa`, each with its label, help, and
its group's `attachments` as `sources`. The text below is generated from
`src/input_guide.runa` by script:

```runa
-- proposed: src/facts.runa (excerpt)
@ import ./sources

= tax_liability_sources = [Source(value = aktiesparekontoloven)]
= spouse_sources = [Source(value = aktieavancebeskatningsloven), Source(value = personskatteloven)]

# SpouseFacts(
    @ field(label = "Living together with the spouse at the end of the year", question = "Were you and your spouse living together (samlevende) at the end of the income year?", help = "Only then does an excess loss on listed shares move between the spouses (aktieavancebeskatningsloven § 13 A, stk. 3), an unused grundbeløb raise the other spouse's (personskatteloven § 8 a, stk. 4) and a negative share income be set off in the other spouse's positive one (stk. 6).", sources = spouse_sources)
    cohabiting_at_year_end: Bool,
    @ field(label = "Spouse's share income", question = "What was your spouse's share income (aktieindkomst) for the income year?", help = "The spouse's own aktieindkomst (personskatteloven § 4 a), before any transfer between the spouses; negative if it is negative.", sources = spouse_sources)
    share_income_øre: Øre,
    @ field(label = "Spouse's net result on listed shares", question = "What was the net amount of your spouse's dividends, gains and losses on listed shares (nettobeløbet af ægtefællens samlede udbytter, gevinster og tab) for the income year?", help = "Shares admitted to trading on a regulated market or a multilateral trading facility, after the spouse's own losses brought forward and before any transfer between the spouses (aktieavancebeskatningsloven § 13 A, stk. 3); negative when the spouse has a loss left over.", sources = spouse_sources)
    listed_share_net_øre: Øre
)

# Taxpayer(
    @ field(label = "Endavu user id") user_id: String,
    @ field(label = "Fully tax liable in Denmark", question = "Are you fully tax liable in Denmark (fuldt skattepligtig til Danmark)?", help = "Only a person who is fully tax liable under kildeskattelovens § 1 can open an Aktiesparekonto and deposit on it (aktiesparekontoloven § 1 and § 9). An owner with limited tax liability (begrænset skattepligtig efter kildeskattelovens § 2) is taxed under aktiesparekontoloven § 16, which is not modelled: such a case is rejected.", sources = tax_liability_sources)
    fully_tax_liable_in_denmark: Bool,
    outside_endavu: OutsideEndavu,
    @ field(label = "Spouse", question = "Were you married (gift) at the end of the income year?", help = "If you were, the spouse's facts follow, all for the income year and before any transfer between the spouses.", sources = spouse_sources)
    spouse: SpouseFacts?
)
```

The other 21 go the same way:

- on `OutsideEndavu` (its type and `other_share_income_øre`);
- on `Account.kind` and `Account.acquisitions_reported_to_skat`;
- on five fields of `OpeningPosition`;
- on five of `Event.Trade` and six of `Event.DividendPaid`;
- on `CarriedForward.listed_share_loss_øre`.

Seven labels exist today, on the field or on its type (`Isin`, `Side`). A
field-level `@ field` restates the type's label, because the most specific
metadata wins whole.

The same liability fact in the Aktiesparekonto entry, `AskOwner`, has no
question (`src/aktiesparekonto.calculate.runa:27-30`). So the backend's need
for it says `"question": null` (PROOF.md:266). The model adds the question
once, and every integrator has it.

### The guides

```runa
-- proposed: guides/endavu.guide.runa
@ import ../src/tax_report.calculate
@ import ../src/aktiesparekonto.calculate

-- Everything not named is in Endavu's records. The customer's facts are
-- held on Endavu's accounts and asked for accounts with other providers.
# TaxReportOrigin = PriceFeed | Customer
= endavu_tax_report = AnswerGuide(fields = [
    EntryField(path = pathof(TaxCase::year_end_prices), by = PriceFeed),
    EntryField(path = pathof(TaxCase::accounts::ask_ending::Closed::prices), by = PriceFeed),
    EntryField(path = pathof(TaxCase::taxpayer::fully_tax_liable_in_denmark), by = Customer),
    EntryField(path = pathof(TaxCase::taxpayer::outside_endavu), by = Customer),
    EntryField(path = pathof(TaxCase::taxpayer::spouse), by = Customer),
    EntryField(path = pathof(TaxCase::accounts::kind), by = Customer),
    EntryField(path = pathof(TaxCase::accounts::acquisitions_reported_to_skat), by = Customer),
    EntryField(path = pathof(TaxCase::opening_positions::isin), by = Customer),
    EntryField(path = pathof(TaxCase::opening_positions::quantity_micro), by = Customer),
    EntryField(path = pathof(TaxCase::opening_positions::cost_basis_øre), by = Customer),
    EntryField(path = pathof(TaxCase::opening_positions::market_value_øre), by = Customer),
    EntryField(path = pathof(TaxCase::opening_positions::acquisition_reported), by = Customer),
    EntryField(path = pathof(TaxCase::events::Trade::date), by = Customer),
    EntryField(path = pathof(TaxCase::events::Trade::isin), by = Customer),
    EntryField(path = pathof(TaxCase::events::Trade::side), by = Customer),
    EntryField(path = pathof(TaxCase::events::Trade::quantity_micro), by = Customer),
    EntryField(path = pathof(TaxCase::events::Trade::amount_øre), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::date), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::isin), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::decided), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::gross_øre), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::danish_tax_withheld_øre), by = Customer),
    EntryField(path = pathof(TaxCase::events::DividendPaid::foreign_tax_withheld_øre), by = Customer),
    EntryField(path = pathof(TaxCase::carried_forward::listed_share_loss_øre), by = Customer)
])

-- PROOF.md:75-87, as data. Everything not named is built from the repositories.
# AskYearOrigin = Snapshots | GivingInstitute | Bookings | PreviousYear | Backoffice | Customer
= endavu_ask_year = AnswerGuide(fields = [
    EntryField(path = pathof(AskYearCase::owner::fully_tax_liable_in_denmark), by = Customer),     -- PROOF.md:77
    EntryField(path = pathof(AskYearCase::account::opening_cash_micro_dkk), by = Snapshots),       -- PROOF.md:78-79
    EntryField(path = pathof(AskYearCase::account::closing_cash_micro_dkk), by = Snapshots),
    EntryField(path = pathof(AskYearCase::opening_positions), by = Snapshots),
    EntryField(path = pathof(AskYearCase::year_end_prices), by = Snapshots),
    EntryField(path = pathof(AskYearCase::account::ending::Closed::prices), by = Snapshots),
    EntryField(path = pathof(AskYearCase::account::transfer_in), by = GivingInstitute),            -- PROOF.md:80
    EntryField(path = pathof(AskYearCase::events), by = Bookings),                                 -- PROOF.md:82
    EntryField(path = pathof(AskYearCase::events::$variant), by = Backoffice),                     -- PROOF.md:81
    EntryField(path = pathof(AskYearCase::instruments::kind), by = Backoffice),                    -- gap 6
    EntryField(path = pathof(AskYearCase::carried_forward), by = PreviousYear)                     -- PROOF.md:84-87
])
```

Counted with the same script:

| Entry and guide | Need-able paths | Resolved | Required |
|---|---|---|---|
| `tax_report`, today | 107 | 26 asked through `CustomerQuestions` | 81, unstated |
| `tax_report`, `endavu_tax_report` (24 items) | 107 | 26 `Customer`, 6 `PriceFeed` | 75, required by the facts type |
| `aktiesparekonto_year`, `endavu_ask_year` (11 items) | 81 | 68, in six groups | 13 |

### The tests

`tests/interviews.scenario.runa` becomes `tests/answers.scenario.runa`:

```runa
-- proposed
@ import ../guides/endavu
= married = demo_case(demo_outside, demo_spouse)
= spouse_answered = answer(tax_report, demo_case(demo_outside, unknown), endavu_tax_report, from_case(married))

| customer_asked_for_the_spouse_then_their_facts: spouse_answered -> map(spouse_answered.rounds, |r| r.groups) == [
    [Asking(group = Customer, paths = [pathof(TaxCase::taxpayer::spouse)])],
    [Asking(group = Customer, paths = [
        pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end),
        pathof(TaxCase::taxpayer::spouse::listed_share_net_øre),
        pathof(TaxCase::taxpayer::spouse::share_income_øre)
    ])]
]
| spouse_answers_end_in_the_complete_result: spouse_answered -> spouse_answered.outcome == Answered(result = tax_report(married))
```

The rounds are today's (`tests/interviews.scenario.runa:14-21`). What the test
adds is who answers them, checked against the guide the backend's SDK is
generated from.

## The backend's view

Three things in the backend change:

- **`FACT_OF_INPUT` goes** (`aktiesparekontoYearCase.ts:214-296`). The
  generated facts type now fails the build on a new input. The need's `by` and
  label name the need.
- **PROOF.md:75-82 becomes `endavu_ask_year`.**
- **The hand-written unit code** (lines 35-100) goes once the units declare
  scales. `inDkk` (lines 72-77) stays: that a `Money` is in DKK is the
  backend's own fact.

What remains is what the backend alone can define:

```ts
// proposed
const facts: AktiesparekontoYearFacts = {
  incomeYear: facts.incomeYear,
  owner: { fullyTaxLiableInDenmark: facts.ownerFullyTaxLiableInDenmark },  // resolved by Customer: may be undefined
  account: { accountId: accountId(facts.accountId.value),
             openingCashMicroDkk: facts.openingCash && microDkk.fromDecimal(inDkk(facts.openingCash, "opening cash")), … },
  …
};
const answering = await engine.aktiesparekontoYearAnswer(facts, {
  Snapshots: leaveOpen, GivingInstitute: leaveOpen, Bookings: leaveOpen,
  PreviousYear: leaveOpen, Backoffice: leaveOpen, Customer: leaveOpen,
});
```

Every resolver above is `leaveOpen`, which is today's behaviour, now
stated. When a source becomes worth reading on demand, only its resolver
changes, and it is still called once per round with all of its needs.

tax-service commit `bec731b` shows what that batching is worth.
`aktiesparekonto_year` started without its events was resolved with 2 resolver
calls over 3 rounds, and one `events[]` call received all 18 detail needs of
six events. A resolver per need would have been called once for the list and
once for each of the 18.

## Rename table

The rule: where a name says "calculation" for the entry concept, it says
"entry". Where it names the computation a call performs, it stays.

| Today | 0.6 | Where |
|---|---|---|
| `@ calculate`, `@ calculate("title")` | `@ entry`, `@ entry("title")` | annotation; `src/lib.rs:597` |
| `interview`, `interview_from`, `Interview(o, i)` | `answer`, `Answering(o, i, g)` | prelude, `src/query.rs:149-163` |
| answering from a complete case | `from_case`, `whole_case_guide()` | prelude |
| `CustomerQuestion(s)`, `InterviewGuide` (tax-service) | `@ field(label, question, help, sources)` on declarations; groups in an `AnswerGuide` | `src/sources.runa:26-32` |
| `CalculationField`, `CalculationGuide` (tax-service) | `@ field(at = …)` (deferred past 0.6: meanwhile an `@ meta` record naming the field by `pathof`); `EntryField(path, by)` is the guide's item | `src/sources.runa:20-21` |
| `CalculationField`, `CalculationMeta`, `CalculationMetaRole` (docs examples) | `@ field(at = …)` (deferred past 0.6); `EntryMeta`, `EntryMetaRole` where a record example stays | calculations.md:51-53, 110-147 |
| `resolve()`, pattern-keyed `Resolvers` | `<entry>Answer(transport, facts, resolvers)`, resolvers by group | `sdk_ts_runtime.ts:1611`, `1692` |
| replies `answer(need, value)`, `ask`; `Resolution` (`Complete`/`Questions`) | `given(need, value)`, `open`; `Answering` (`Answered`/`Unanswered`) | `sdk_ts_runtime.ts:1514-1521`, `1631` |
| `PreparedCalculation` | `PreparedEntry` | `src/calculate.rs:7131` |
| CLI help and diagnostics saying `@ calculate` or "calculation entry" | `@ entry`, "entry" | `src/bin/runa.rs:729`, `734`, `768`, `2352`, `2359`; `src/calculate/sdk_ts.rs:286`, `295`; `src/calculate.rs:2207`, `2466`, `3091`; `src/query.rs:608`, `981` |
| "calculation entry" in docs; page "Typed Calculations" | "entry"; "Entries" | calculations.md:12, RFCs, README |

These stay, because they name the computation: `runa call`, `Calculated<R>`,
`CalculationError`, `CalculationAssumption` and `FUTURUNA_CALCULATION_*`.

## Migration

- **`@ calculate`.** `runa fmt --migrate` rewrites it to `@ entry`. In 0.6
  `@ calculate` is still read, with a warning, and 0.7 removes it. The
  contract and schema hash do not change.
- **`interview`.** `runa fmt --migrate` makes three rewrites and lists any
  other use:
  - `interview(|c| query e(c), partial, complete)` becomes
    `answer(e, partial, whole_case_guide(), from_case(complete))`;
  - `x.rounds` becomes `map(x.rounds, |r| r.paths)`;
  - `x.result == v` becomes `x.outcome == Answered(result = v)`.
- **Unchanged:** `@ field(question = …)`, and field records under `@ meta`.
- **SDK.** `resolve()` is removed. Without `--guide`, the SDK has calls and
  queries only. `engine-package/test/resolve.test.mjs` names `events` for one
  group and keeps its assertion.
- **tax-service, by hand:**
  - remove `src/input_guide.runa` and `src/sources.runa:17-39`;
  - move the 26 questions onto declarations, and add one to `AskOwner`;
  - replace the two `CalculationGuide`s with `@ field(at = …)` once it ships
    (deferred past 0.6); until then keep them as `@ meta` records naming the
    field by `pathof`;
  - declare the unit scales;
  - write `guides/endavu.guide.runa`;
  - move the interviews to `answer`.

  The schema hashes change for the moved metadata and for the scales.
- **Backend proof:**
  - regenerate the SDK with the guide;
  - build `AktiesparekontoYearFacts` with the generated converters;
  - delete `FACT_OF_INPUT`.

## Alternatives rejected

- **Four origin kinds** (required, fetched, asked, another entry). What a
  resolver does inside is the integrator's business, and declaring it gives
  Futuruna nothing to check or use. `Unanswered.open`, keyed by group, already
  keeps the customer apart from a source.
- **Questions in the guide.** The wording defines the fact legally, and a
  mis-phrased question yields a wrong legal fact that the engine cannot
  detect. The integrator would write model knowledge, once per integrator.
- **Questions in the app.** As above, and worse: the text would not be
  versioned with the model it defines. The app translates or restyles what
  the need carries.
- **Origins in the law, or a `DataSource` enum on `@ field`.** The same fact
  is required by one integrator and asked by another (PROOF.md:77). A law module
  that names `Ledger` names one backend.
- **The guide declared on the entry** (`@ guide(g)`, or `@ meta(g) for e`).
  Either the law names its caller, or which guide applies depends on imports.
  Two guides for one entry (a backend's and `whole_case_guide()`) could not
  coexist.
- **A group name for required paths.** It would carry nothing. Leaving required paths
  unnamed makes a new field required by default.
- **Groups by key or data** (`opening_positions[kind = ExternalDepot]`). These
  cannot be checked. The fact is resolved, and the required data fills the
  elements it has.
- **One-need-at-a-time resolvers.** See commit `bec731b`: a backend that sees
  18 needs one at a time cannot fetch them in one query.
- **Futuruna effects in the entry.** An entry is pure (calculations.md:17-22).
  `query`, the one observer of unknown-ness, must stand outside what it
  observes (needs-in-the-language.md:273-283).
- **`CustomerQuestions` as a list on the entry.** It is recognised only by
  shape (calculations.md:214-219) and overrides the declarations whole. It
  mixes the law's questions with one integrator's choice, and nothing checks
  it.
- **`@ needs` as the annotation's name.** An entry is also called with complete
  cases, and `Needs` is already a variant name.
- **A completeness audit with a recorded ceiling.** With required as the default,
  coverage is complete by construction. What can go wrong is a required fact left
  out, which is a compile error in the facts type and a runtime error in
  `answer`. Neither needs a search.

## Open questions

- **The wire ids** (`futuruna.calculate.v1`, `.input.v2`, `.output.v1`,
  `.partial-output.v1`, `.compact.v1`, `.xlsx.input.v9`, `.xlsx.output.v2`).
  Renaming them breaks every saved document; keeping them leaves the old word
  on the wire.
- **The `.calculate.runa` convention** (tax-service's files, CLI examples,
  `src/bin/runa.rs:2385`). `runa` does not depend on it.
- **The finer-amount policy for incoming amounts.** The model already decides
  it for computed figures (ASKL-ØRE-TRUNCATION). For a unit converter, it is
  open whether the unit or the field declares it, and with what syntax.
- **A list from two sources.** `opening_positions` holds Endavu's and the
  customer's positions. The list is required and the customer's fields are
  resolved. Should a guide be able to say "elements added by the customer"?
- **`later`.** Should a round's `later` facts go to their groups in the same
  round?
- **`answer` on any rule**, as the PO's sketch suggests
  (needs-in-the-language.md:431-439).
- **Deprecation length.** `@ calculate` is read for one release, unless the PO
  wants longer.
