# RFC: Needs in the language

Status: Proposed (Futuruna 0.5). Builds on [Calculation needs](calculation-needs.md).

## Problem

[Calculation needs](calculation-needs.md) lets a case leave facts out and
answers `Complete` or `Needs`, but only at the JSON boundary
(`runa call --partial`). A model cannot say in Futuruna what its interview
is, so the interview is tested outside the language. tax-service's
`scripts/needs_acceptance.py` (about 100 lines of Python) runs each scenario
of `cases/needs/*.json` round by round through `runa call --partial=check`,
answers each need from a complete JSON case, and compares the rounds with
expected path strings:

```json
"rounds": [
  ["taxpayer.spouse"],
  ["taxpayer.spouse.cohabiting_at_year_end",
   "taxpayer.spouse.listed_share_net_ore",
   "taxpayer.spouse.share_income_ore"]
]
```

What that costs:

- **Two languages for one model.** The scenarios are JSON files of about 400
  lines each, copied from `cases/demo-2026.json`; the model's own builders
  (`fixtures/builders.runa`) cannot make them.
- **Paths are strings.** Renaming `listed_share_net_ore` breaks the scenario
  when the script runs, not when the model is checked.
- **The guarantee is checked only where the script asks for it.** The strict
  oracle and the completion test run because the script passes
  `--partial=check`; a scenario run another way skips them.
- **Entry-provided fields are named by string.** `@ field(entry = "share_income",
  subject = "taxpayer.spouse")` is checked only when a contract is extracted.

## Design

Five additions, all on the side of the code that *asks*, none inside the
calculation that is asked:

1. `unknown`, a value that is not known yet;
2. `query entry(case)`, which evaluates a calculation entry on a case that
   holds unknowns and returns `Query(O, I)`;
3. typed paths `Path(I)`, written with `pathof`;
4. `fill` and `interview` in the standard library;
5. `runa test` running the strict oracle and the completion test on every
   query.

### `unknown`

`unknown` is an expression that takes the type of its place, as a number
literal takes its unit ([Unit types](unit-types.md)) and a constructor its
owner ([Scoped constructors](scoped-constructors.md)):

```runa
= spouse_not_asked = demo_u1001(unknown)          -- married_to: Option(SpouseFacts)
= spouse_facts_not_asked = demo_u1001(Some(unknown))   -- SpouseFacts: each field unknown
```

It means exactly what a left-out field means in a partial JSON case, by the
same decoding table (calculation-needs.md, *Kinds of missing*):

| `unknown` placed as | Means |
|---|---|
| a scalar, `Option`, list, set or map | not asked yet, as one unknown |
| a sum type | the variant is unknown (`path.$variant`) |
| a single-constructor record | the record exists, each field unknown |

`Some(unknown)` in an `Option(SpouseFacts)` place therefore says "married;
nothing about the spouse known yet", as `{}` does in JSON.

**Placement only.** `unknown` may stand where a value is placed and its type
is known from the place: an argument of a declared parameter, a field value,
a list element of known element type, a typed binding, a declared result, an
`if` or `match` branch. It may not be read: as an operand, a scrutinee, a
guard, an argument of a builtin that accepts any type (`show`), or one side
of `==`. A place with no type is an error that says how to give it one
(`= x: Int = unknown`).

**No path until asked.** An unknown carries no path until a `query` places
it: its path is its position in the query's argument, with list indices. A
case built by builders that read the unknown before the query (a builder
summing an unknown list of events) stops at that read with a runtime error
naming the read and the line of the `unknown`; the writer then states that
field directly.

`unknown` is a reserved word.

### `query`

```runa
query tax_report(c)          -- : Query(TaxReportOutcome, TaxCase)
```

`query` prefixes a call of a calculation entry (`@ calculate`) by name; its
argument has the entry's input type. It evaluates the entry as
`runa call --partial` evaluates a partial case: the unknowns of the argument
get their paths, evaluation propagates them with Kleene logic and bounded
speculation, and the result is

```runa
# Query(o, i) = Complete(result: o, unused: List(Path(i))) | Needs(needs: List(Need(i)))
```

`Complete.unused` and `Needs.needs` are the JSON result's `unused_paths` and
`needs`, at typed paths. `Need(i)` is the need of the partial-result schema:
its path, kind, label, question, help, unit, sources, and who waits for it
(below). It is not a second definition: the Futuruna type and
`runa schema --format partial-result` are generated from the one Rust
definition that writes the JSON, so a member added to needs reaches both.

`query` is a contextual keyword: only `query <entry>(...)` is a query, and
`query` stays an ordinary name elsewhere.

### Typed paths

`Path(i)` is a path into a value of type `i`, with list indices. `pathof`
writes one:

```runa
pathof(TaxCase::taxpayer::spouse::share_income_ore)
pathof(TaxCase::accounts[1]::closing_cash_ore)
```

- Elements of a list are indexed with `[n]`; without an index a list step is
  transparent, as today. `Option` steps are transparent, sum alternatives
  explicit (`outside_endavu::OutsideDeclared::other_share_income_ore`), and
  `::$variant` names a tag.
- `pathof` takes its type from its place, like `unknown`: `String` where a
  string is expected (`CalculationField(path = pathof(...))`, unchanged),
  `Path(TaxCase)` where a path is. One side of `==` gives the other its type,
  so `need.path == pathof(TaxCase::taxpayer::spouse)` compares typed paths. A
  `pathof` with no type from its place stays `String`, as today.
- A path to a field that does not exist is a check error, as today. A path
  rooted at another type than the query's input is a type error.
- `show(path)` is the JSON path string (`taxpayer.spouse.share_income_ore`).

### `fill` and `interview`

Two standard-library functions answer needs the way `needs_acceptance.py`
answers them:

```runa
> fill(partial: i, complete: i, paths: List(Path(i))) -> i
```

`fill` writes into `partial`, at each path, the value of `complete` there. A
record is answered as existing with its fields unknown (one question at a
time); every other value (a scalar, `None`, a variant with its payload, a
list) is answered whole. This is the script's `answer` rule, so expected
rounds carry over unchanged.

```runa
# Interview(o, i) = Interview(rounds: List(List(Path(i))), result: o)

> interview(ask: i -> Query(o, i), partial: i, complete: i) -> Interview(o, i) {
    interview_from(ask, partial, complete, [])
}

> interview_from(ask: i -> Query(o, i), case: i, complete: i, rounds: List(List(Path(i)))) -> Interview(o, i) {
    = q = ask(case)
    match q {
        | Complete -> Interview(rounds = rounds, result = q.result)
        | Needs -> {
            = asked = map(q.needs, |n| n.path)
            interview_from(ask, fill(case, complete, asked), complete, concat(rounds, [asked]))
        }
    }
}
```

`interview` is ordinary Futuruna; the query is passed as a function
(`|c| query tax_report(c)`). It terminates because every `Needs` round names
at least one path that `fill` then answers (calculation-needs.md, *Summary*).

### Tests run the guarantee

Under `runa test`, every `query` that a test evaluates runs as
`--partial=check` does: propagation, the strict-mode oracle, and, for a
`Complete(r)`, the completion test, which generates completions of the
case's unknowns from their types and runs the entry on each through ordinary
evaluation. A violation fails the test at the line of the `query`, for
example:

```text
tests/interviews.scenario.runa:9: query tax_report, round 2: the propagated result is Complete, but the
  completion taxpayer.spouse.cohabiting_at_year_end = false returns a different result
```

The writer writes nothing for this; it is how a query is tested. Outside
`runa test` (`runa run`, a program serving an interview) a query propagates
only, like `runa call --partial`.

### Who waits

A `Need` carries the attribution of [Calculation needs](calculation-needs.md)
as that RFC defines it; this RFC adds no rule of its own. As revised for 0.4
(the Provenance change), a need lists every rule or function in a law section
that consumes the value on the evaluated paths, cited ones first, each with
its section, cited paragraphs and act (`waiting: [{rule, section, paragraphs,
act}]`), so a helper such as a rounding function is not reported as the law
that waits. In Futuruna the rule and section are `ProgramReference`s, so a
test can say which law asks. The Provenance change's acceptance case, the
owner's tax liability waited on by the § 1, § 9 and § 16 rules of
`ask_ownership`, reads (with a fixture `liable_case(fully_tax_liable: Bool)`):

```runa
= liability = query tax_report(liable_case(unknown))
| liability_asked_by_ownership_rules: liability -> match liability {
    | Needs -> any(liability.needs, |n| any(n.waiting, |w| w.section == Some(refof(section ask_ownership))))
    | Complete -> False
}
```

(`refof(section ...)` is from [One annotation system](one-annotation-system.md).)

### Composition of entries

An entry uses another entry for a subject by calling it:

```runa
@ calculate("Household share income tax")
| household_tax(h: Household) -> HouseholdTax(
    own = share_income(h.own),
    spouse = share_income(h.spouse)
)
```

Under a query this composes without new rules. The unknowns inside
`h.spouse` got their paths from the outer case (`spouse.share_income_ore`),
so the needs of the inner entry arrive in the outer `Needs` at outer paths,
with the inner entry's rules as the ones waiting. One query interviews both
people.

When the client holds the other calculation's result rather than its facts
(the spouse's tax was computed in another account), the field stays an
entry-provided field, and its metadata becomes typed:

```runa
@ field(label = "Spouse's share income", entry = refof(share_income), subject = pathof(TaxCase::taxpayer::spouse))
spouse_share_income_ore: Ore
```

`runa check` now verifies what contract extraction verified: `entry` is a
calculation, its input type is the type at `subject`, and its output type is
the field's type. The contract's `provided_by` is the same strings. A need of
kind `Entry` carries `entry: ProgramReference` and `subject: Path(i)`; in a
test, `fill` answers it from the complete case like a fact.

Two entries that need each other are still one joint entry (calculation-
needs.md, *Entry needs*).

### Unknowns stay uninspectable

There is no `is_unknown`, no `known(x)`, and no pattern that matches an
unknown. The guarantee, "if a partial case returns `Complete(r)`, every
completion returns `r`", holds because every operation in the evaluation is a
function of the values the completion would supply. A test of unknown-ness
inside the evaluation would let a partial case take a branch no completion
takes and return a `Complete` no completion returns.

`query` is the one observer of unknown-ness, so it must stand outside every
evaluation it can observe. Hence:

- **`query` and `unknown` may not be reached from a calculation entry.**
  `runa check` reports a `query` or an `unknown` in code that any
  `@ calculate` entry can call, with the call chain: "`query` in
  `answer_or_zero`, reached from the calculation `tax_report` through
  `fd_tax`: a calculation's result must not depend on which facts are known".
- **`query` takes an entry, not any function.** An entry has a contract, so
  its needs have labels, questions and sources, and its paths are rooted at a
  declared input type.

### Where it runs

`query` runs in the interpreter, which `runa test`, `runa run` and the
WebAssembly engine use. Native code and generated Rust take complete input
only, as partial evaluation does today:

- `runa build` and Rust emission report each `query` and `unknown` in the
  program they compile as an error: "`query` runs in the interpreter; native
  code takes complete input".
- `runa test --run` runs a test that reaches a `query` in the interpreter
  and marks it `interpreted (query)` in its report; the other tests run
  natively as before. No test is skipped.

## The spouse interview, as a test

Before: `cases/needs/spouse.json` (the u-1001-2026 input of
`cases/demo-2026.json` without `taxpayer.spouse`, and the rounds shown under
Problem) and the loop of `scripts/needs_acceptance.py`:

```python
for _ in range(len(scenario["rounds"]) + 1):
    result = call(envelope, "--partial=check")
    if result["$variant"] == "Complete":
        break
    paths = [need["path"] for need in result["needs"]]
    rounds.append(paths)
    for path in paths:
        set_at(case["input"], path, answer(complete_case["input"], path))
ok = result["$variant"] == "Complete" and rounds == scenario["rounds"] \
    and result["result"] == expected
```

After: `tests/interviews.scenario.runa`. `demo_u1001(married_to)` is a
fixture that writes the u-1001-2026 case of `cases/demo-2026.json` with the
builders, with the spouse as its parameter; `spouse(60000, 0)` is the
existing builder for a cohabiting spouse with 60.000 kr. share income and a
listed-share net of 0, the demo case's spouse.

```runa
@ import ../fixtures/builders
@ import ../fixtures/demo

= married = demo_u1001(spouse(60000, 0))
= spouse_interview = interview(|c| query tax_report(c), demo_u1001(unknown), married)

| spouse_asked_first_then_their_facts: spouse_interview -> spouse_interview.rounds == [
    [pathof(TaxCase::taxpayer::spouse)],
    [
        pathof(TaxCase::taxpayer::spouse::cohabiting_at_year_end),
        pathof(TaxCase::taxpayer::spouse::listed_share_net_ore),
        pathof(TaxCase::taxpayer::spouse::share_income_ore)
    ]
]
| spouse_interview_ends_in_the_complete_result: spouse_interview -> spouse_interview.result == tax_report(married)

? spouse_asked_first_then_their_facts
? spouse_interview_ends_in_the_complete_result
```

Every round's `query` ran the strict oracle and the completion test; the
script's `--partial=check` is now how `runa test` evaluates a query. The
other two scenarios (`closing-day-prices.json`, `outside-endavu.json`) move
the same way.

A single round needs no helper:

```runa
= unmarried = query tax_report(demo_u1001(None))
| unmarried_needs_nothing: unmarried -> match unmarried {
    | Complete -> unmarried.result == tax_report(demo_u1001(None))
    | Needs -> False
}
```

## What the writer writes and what the reader sees

The writer states an interview where the model's other scenarios are: the
case from the same builders, the question as `unknown` in the place it
belongs, the expected rounds as checked paths. A renamed field breaks the
test at `runa check`; a case the builders cannot express is a builder away.
The guarantee is tested on every query without being asked for.

The reader of a test sees the interview in the model's terms: the paths are
the input type's fields, and a failure names the round, the query's line and
the completion that disproves it. The reader of a result sees nothing new:
`runa call --partial`, the WebAssembly engine and the SDK return the same
documents, and the `Need` type in Futuruna is the schema they are generated
from.

## Migration

- `pathof` keeps its type `String` wherever nothing asks for a path;
  existing metadata does not change.
- `@ field(entry = "...", subject = "...")` and anchored `entry = Some(...)`,
  `subject = Some(...)` become `refof(...)` and `pathof(...)`.
  `runa fmt --migrate` rewrites them when exactly one calculation input
  reaches the field (the root of the `pathof`); otherwise it lists them.
- A binding or parameter named `unknown` is renamed by `runa fmt --migrate`
  (to `unknown_`) and reported.
- tax-service writes `fixtures/demo.runa` (the demo case through the
  builders), moves its three interviews to `tests/interviews.scenario.runa`,
  and deletes `scripts/needs_acceptance.py` and `cases/needs/`. Its README
  section on interviews points to the test file.

## Alternatives rejected

- **`is_unknown` / `known(x)`.** Breaks the guarantee, as shown above.
- **`query` inside an entry**, so that one entry queries another and acts on
  its `Needs`. The outer entry could then return a value chosen because a
  fact is missing, a value no completion returns. A direct call composes the
  two entries and the outer query reports both entries' needs.
- **`unknown` as a type** (`Partial(a)`, `Unknown(a)`). Every field of every
  input type would change, and the model would be written twice: once for
  complete and once for partial values. `unknown` is a value of the place's
  own type, as a left-out JSON field is.
- **`None` for "not asked yet".** `None` is an answer ("no spouse");
  calculation-needs.md keeps the two apart, and so does the language.
- **String paths in `Need`.** A renamed field would fail at run time.
  `Path(i)` with `pathof` fails at `runa check`.
- **`Query(o)` without the input type.** `fill(partial, complete, paths)`
  could then be given paths of another input; the second parameter types it.
- **Running JSON scenarios from `runa test`.** Keeps the cases out of the
  builders and the paths as strings; it moves the script, not the problem.
- **An opt-in `?`-form for the oracle** (`? checked query ...`). A query
  tested without its guarantee proves less than it appears to; the check is
  not optional in tests.

## Open questions

- **Cost of the completion test.** Every query in `runa test` generates
  completions and runs the entry on each. A suite with hundreds of queries
  may need a per-query budget, reported
  when it cuts the generation short.
- **Explained queries.** A `query_explained` (or `explain` on a `Complete`)
  would let a test assert *why* a result holds; not part of this RFC.
- **`later`.** If [Calculation needs](calculation-needs.md) reports facts
  reachable once the listed needs are answered (`later`), `Need` gains it and
  `interview` could assert the full question plan; whether a round's `later`
  must equal the next round's needs is open.
- **Unknown reads before a query.** A builder that reads an unknown stops
  the test; propagating instead would hide where the case was built wrong.
  The stop is chosen, but a builder library may want a way to defer such
  derived fields.
- **List elements by key.** Paths index list elements; when needs carry an
  element's key (calculation-needs.md), `pathof` could address an element by
  key (`opening_positions[isin = "IE0000000003"]`) instead of position.
