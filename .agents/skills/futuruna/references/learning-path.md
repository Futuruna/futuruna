# Learning Futuruna Effectively

Use this path when the learner or model has not seen Futuruna before. Teach the
language through its organizing idea, a small working program, and progressively
richer examples. Do not begin with a full syntax or command inventory.

Repository paths in this reference are relative to the repository root.

## Contents

- [Start With the Mental Model](#start-with-the-mental-model)
- [Teach the Core Syntax in This Order](#teach-the-core-syntax-in-this-order)
- [Reinforce the Main Idioms](#reinforce-the-main-idioms)
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
