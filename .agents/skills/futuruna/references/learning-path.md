# Learning Futuruna Effectively

Use this path when the learner or model has not seen Futuruna before. Teach the
language through its organizing idea, a small working program, and progressively
richer examples. Do not begin with a full syntax or command inventory.

Repository paths in this reference are relative to the repository root.

## Contents

- [Start With the Mental Model](#start-with-the-mental-model)
- [Teach the Core Syntax in This Order](#teach-the-core-syntax-in-this-order)
- [Reinforce the Main Idioms](#reinforce-the-main-idioms)
- [Explore Finite Rule Spaces](#explore-finite-rule-spaces)
- [Teach Through a Feedback Loop](#teach-through-a-feedback-loop)
- [Confirm Practical Understanding](#confirm-practical-understanding)

## Start With the Mental Model

Futuruna's main declaration forms are organized by a front-rune; ordinary
expressions and control flow live inside those forms. Teach the question each
rune answers before teaching individual forms.

| Rune | Question | Core role |
|---|---|---|
| `#` | What exists? | Domain types, effects, traits, and implementations |
| `>` | What happens? | Functions, actors, and modules |
| `\|` | What must be true? | Facts, rules, invariants, handlers, scopes, and match arms |
| `=` | What is? | Bind a name to a value or bind a successful result |
| `~` | What flows? | Streams, subjects, and subscriptions |
| `@` | Where do proofs stop? | Effects, imports, dependencies, metadata, and integration boundaries |
| `?` | Prove it. | Check declared invariants |

The front-runes create separate syntax spaces. When a learner is unsure how to
write something, first ask which question the statement answers, then consult
the matching section of `docs/reference/runes.md`.

Begin with one complete effect:

```runa
@ print("Hello, Futuruna!")
```

Then add one category at a time: a `#` type, a `>` function, an `=` binding,
and finally the `@ print` that exposes the result. The maintained version of
that progression lives in `docs/tutorial/01-hello.md`.

## Teach the Core Syntax in This Order

1. Literals, primitive and composite types, expressions, braces, and `--`
   comments from `docs/reference/basics.md`.
2. `#` product and sum types, positional construction, field access, and
   pattern matching.
3. `=` bindings and `>` functions, including expression-valued `if` and
   `match` blocks.
4. `|` facts, logic rules, defaults, exceptions, and named invariants.
5. `@` effects and imports so pure/domain logic stays distinguishable from IO
   and integration.
6. `?` checks after the learner can state an invariant clearly.
7. Teach `~` streams and actors when the task contains state or values changing
   over time. Teach declared algebraic effects when the task needs an abstract,
   handleable capability.

Use the tutorial in order when broad onboarding is requested:

| Step | Maintained lesson | Learner outcome |
|---|---|---|
| 1 | `docs/tutorial/01-hello.md` | Run a program and recognize the rune categories |
| 2 | `docs/tutorial/02-types.md` | Define domain vocabulary and match structured values |
| 3 | `docs/tutorial/03-functions.md` | Write functions, lambdas, and pipelines |
| 4 | `docs/tutorial/04-rules.md` | Express facts, rules, defaults, and invariants |
| 5 | `docs/tutorial/05-streams.md` | Model values that flow through time |
| 6 | `docs/tutorial/06-effects.md` | Isolate effects and understand actors |
| 7 | `docs/tutorial/07-project.md` | Organize and build a multi-file project |

Load only the lessons needed for the current task.

State command maturity when it matters: core rules and invariants are Stable,
while `runa verify` is Preview; project initialization's documented first-run
path is Stable, while commands such as `add` and `lsp` are Preview and `audit`
is Experimental.

## Reinforce the Main Idioms

- Let the rune decide. Choosing a front-rune is a semantic classification, not
  decoration.
- Use `#` types as the domain vocabulary. Prefer names from the modeled domain
  over implementation-shaped names.
- Use `|` for facts and rules that state what holds. Use `>` for a computation
  that produces an outcome from inputs.
- Use `=` for local meaning. Keep bindings small enough that the value behind a
  name remains clear.
- Keep direct IO and integration at explicit `@` boundaries. Use `# effect`,
  `with`, and `| handle` when effects must be abstract and interceptable.
  Prefer pure types, rules, and functions for the core model.
- Use `~` when time, subscription, or a changing sequence is genuinely part of
  the problem; ordinary collection processing does not need to become a stream.
- Define a named `|` invariant before checking it with `?`.
- For source-backed models, quote and identify the source, encode only what it
  establishes, and keep interpretation or missing information explicit.
- Separate reusable source rules from concrete scenarios, audit checks, and
  typed calculation entry points in larger models.

Read `docs/reference/style.md` for the maintained idioms and
`docs/reference/README.md` for the full reference map.

## Explore Finite Rule Spaces

Futuruna can search an explicit finite scenario space with its ordinary list
operations. The core flow is:

```text
finite inputs -> map/flat_map -> model results -> filter -> foldl -> invariant -> ?
```

Start every exploration by naming:

- the facts held fixed,
- each input allowed to vary and its finite domain,
- the exact outcome metric and unit,
- the predicate that makes a scenario a witness, and
- the ordering used to select a minimum, maximum, or worst case.

Use a list for meaningful discrete alternatives and `range(start, end)` for an
integer interval; the end is excluded. Use nested `flat_map` calls to enumerate
multiple dimensions and a final `map` to construct or evaluate each scenario.
When every generated scenario is supported and valid, evaluating all of them
makes the result exhaustive over the declared finite domain.

If the model can mark a scenario invalid or unsupported, keep that status in
the result. Prove every generated scenario is valid, or report the exclusions
and narrow the claim; never silently filter them away before saying a search
was exhaustive.

This complete synthetic example searches adjacent income steps for a cliff:

```runa
# IncomeStep(before_income: Int, after_income: Int, before_net: Int, after_net: Int)

> net(income: Int) -> Int {
    if income >= 105 { income - 10 } else { income }
}

> assess_step(before_income: Int) -> IncomeStep {
    = after_income = before_income + 1
    IncomeStep(
        before_income = before_income,
        after_income = after_income,
        before_net = net(before_income),
        after_net = net(after_income)
    )
}

> loss(step: IncomeStep) -> Int {
    step.before_net - step.after_net
}

= steps = map(range(100, 110), assess_step)
= cliffs = filter(steps, |step| step.after_net < step.before_net)
= worst_cliff = if length(cliffs) > 0 {
    Some(foldl(tail(cliffs), head(cliffs), |worst, step| {
            if loss(step) > loss(worst) { step } else { worst }
        }))
} else {
    None
}

| searched_every_input: steps -> length(steps) == 10
| income_cliff_found: cliffs -> length(cliffs) > 0

? searched_every_input
? income_cliff_found

@ print("worst cliff: " + show(worst_cliff))
```

Apply the same structure to common questions:

| Question | Search construction |
|---|---|
| Does a universal claim fail? | Filter for cases where the claim is false; each retained case is a counterexample. |
| Where does behavior change? | Evaluate adjacent values or source-defined boundary candidates and filter for a changed classification or outcome. |
| Can extra income reduce net resources? | Pair `before` with `after`, calculate both exactly, and retain cases where `net_after < net_before`. |
| What is the minimum, maximum, or worst result? | Filter eligible cases, then use `foldl` with an explicit comparison metric. |
| Which combination of facts causes the result? | Enumerate each dimension with nested `flat_map`, then preserve all varied facts in the result type. |

For a universal claim, a zero-length counterexample list supports the claim over
the searched domain. For an existence question, prove that the witness list is
nonempty. Guard `head` with `length(list) > 0` before selecting an extremum.

In law, tax, contract, and compliance work, retain the jurisdiction, effective
date, official sources, fixed facts, assumptions, and units beside the search.
Prefer exact model outputs such as øre over rounded display projections. Keep
private case material outside the repository, and state findings as results of
the current encoded model over the declared domain.

Use `examples/danish-income-tax/exploration-workbook.md` for the full working
method and `examples/danish-income-tax/personskat-income-cliffs.audit.runa` for
an executable multi-step income-cliff search.

## Teach Through a Feedback Loop

For each new concept:

1. Show one small, runnable example.
2. Ask the learner to predict which rune belongs and what the result means.
3. Format and check the file.
4. Run the example or its closest focused test.
5. Explain the observed result or diagnostic in terms of the rune category.
6. Extend the example by one concept, not an entire subsystem.

Progress from `examples/weather_demo.runa` for a compact mixed-rune program to
`examples/cocktails.runa` for logic rules, then to a calculation fixture or a
domain example only when the learner's goal calls for it.

## Confirm Practical Understanding

A learner or model is ready to work independently when it can:

- select the appropriate front-rune and explain why,
- locate the current syntax and feature-stage source instead of guessing,
- define domain types, functions, bindings, rules, effects, and invariants
  without mixing their roles,
- format, check, and run a focused program,
- adapt the closest maintained example rather than inventing a new dialect,
  and
- state assumptions and limitations that actually affect the result.
