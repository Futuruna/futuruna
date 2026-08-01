# Exploring Futuruna's Syntax

## Start With Law

Futuruna is a programming language for law. Its purpose is to let definitions,
rules, defaults, exceptions, calculations, tests, effects, and ordinary program
logic live in one execution space.

That purpose comes before the syntax experiment described on this page.
Futuruna does not need an optimizer to prove that it should exist. It must prove
itself by making real laws, contracts, policies, and rule-bound systems easier
to express, execute, inspect, and audit.

The optimizer was a design instrument. It helped explore a question:

> Can the first character of a statement orient the reader without limiting
> what the rest of the statement can express?

The answer led toward Futuruna's seven runes. The experiment is an interesting
part of the language's history, but it is not a claim that Futuruna is the one
optimal programming language.

## The Design Problem

Rule systems and ordinary programs are usually separated.

A logic language is comfortable describing relationships and asking for
answers, but less comfortable building an application around those answers. A
legal language can model defaults and exceptions precisely, but may stop at the
edge of its legal domain. A general-purpose language can build almost anything,
but usually represents law through nested conditionals, framework conventions,
and comments that the compiler cannot understand.

Futuruna tries to keep those capabilities together. A small, illustrative
model can contain types, layered rules, a normal function, values, an invariant,
and an effect:

```runa
# Person(income: Int, age: Int)

| tax_rate(person: Person) -> 20
| tax_rate(person: Person) -> 40 under person.income > 500000
| exception allowance tax_rate(person: Person) -> 0 under person.income < 50000

> tax_due(person: Person) -> Int {
    person.income * tax_rate(person) / 100
}

= taxpayer = Person(650000, 42)
= due = tax_due(taxpayer)

| non_negative_tax: due -> due >= 0
? non_negative_tax: amount -> {
    @ print("Tax due: " + show(amount))
}
```

This is not an attempt to reduce law to arithmetic. Real legal models need
authority, dates, definitions, dependencies, provenance, and competing
interpretations. The example shows only the language-design point: the rule
system and the ordinary program are not foreign to each other.

## Runes as Semantic Modes

Most Futuruna statements begin with one of seven runes:

| Rune | Question | Semantic territory |
|------|----------|--------------------|
| `#` | What exists? | Types, effects, traits, implementations |
| `>` | What happens? | Functions, actors, modules |
| `\|` | What should hold or apply? | Rules, alternatives, invariants, handlers |
| `=` | What is? | Values, bindings, established facts |
| `~` | What flows? | Streams and temporal behavior |
| `@` | Where is the boundary? | Effects, imports, metadata, foreign code |
| `?` | What needs evidence? | Checks and verification demands |

The rune is a semantic fly-in. Before reading the whole line, a person or tool
knows which part of the language to consider.

This does not mean that every rune maps to exactly one parser construct. The
runes mark semantic territories, not individual nodes in an abstract syntax
tree. `|`, for example, can introduce a rule, an exception, a match arm, an
invariant, or an effect handler. Those forms are different, but they all qualify
a proposition or select among alternatives.

That compression is useful only while the territory remains coherent. The
design standard is simple:

> Every use of a rune should fit one stable sentence that explains what the
> rune means.

If a construct fits only because the grammar had nowhere else to put it, the
rune has become a bucket. It should be clarified, moved, or removed. If users
can predict the family of valid continuations from the rune and local context,
the shared syntax is doing useful work.

## The Optimization Experiment

The syntax exploration represented programs as transitions between broad token
categories: statement starts, identifiers, types, operators, delimiters,
literals, and similar structural roles. A candidate design could then be viewed
as a graph describing which categories tended to follow which others.

An NSGA-II evolutionary search explored candidate transition graphs against
three proxy objectives:

- **Optionality:** how many meaningfully different continuations remain
  reachable from a position.
- **Distinguishability:** how different one syntactic context looks from
  another.
- **Structural independence:** whether different signals in the syntax carry
  non-redundant information.

These objectives pull in different directions. Maximum optionality alone
produces noise: everything can follow everything. Maximum distinction alone
can produce a rigid language made of isolated forms. The search looked for
non-dominated trade-offs rather than collapsing the objectives into one score.

In optimization terminology, those trade-offs form a Pareto frontier. Here,
that phrase has a narrow meaning: within the chosen representation, objectives,
corpora, and search process, no measured objective can improve without another
measured objective becoming worse. It does not mean that a language is
universally optimal.

The recurring result that influenced Futuruna was strong differentiation at
the beginning of statements. Candidate structures scored well when the opening
token identified a statement family while later syntax remained available for
types, expressions, and block structure. That result made line-initial runes
worth trying.

The optimizer did not emit Futuruna's grammar. It did not decide that law has
exactly seven concepts. It did not establish that `|` should mean rules or that
`?` should demand verification. Those were human language-design decisions,
shaped by the practical goal of combining rule-based and ordinary programming.

## What the Experiment Does Not Establish

The metrics measure properties of a model of syntax. They do not directly
measure whether programmers understand a legal model, make fewer mistakes, or
produce better software.

The experiment does not prove that:

- seven runes are the only possible design;
- Futuruna is clearer than every other language;
- a Pareto-frontier candidate is optimal outside the measured objectives;
- the runes improve human comprehension without user studies;
- the runes improve AI generation without comparative model evaluations; or
- mathematical language borrowed from another field transfers its authority
  to programming-language design.

Changing the token categories, source corpus, thresholds, objectives, or search
space can change the result. That is normal for an exploratory optimization
experiment. The output is a set of design leads, not a law of nature.

## Why the Front-Rune System Stayed

The runes survived because they are useful in programs, not because they scored
well in an experiment.

They create a compact visual distinction between a legal rule and the function
that applies it. They let a default, a guarded condition, and a named exception
remain close together. They make effects visibly different from pure
calculations. They allow checks and verification demands to appear beside the
model they examine.

Most importantly, they let several programming domains share a file without
making every line look interchangeable. The syntax provides a point of entry:
when a line starts with `|`, the reader enters the space of rules and qualified
alternatives; when it starts with `>`, the reader enters the space of
transformations and processes.

That is the practical hypothesis behind the design.

## The AI Hypothesis

The front-rune system may also help language models. A rune gives the model an
early, low-cost signal about the kind of statement it is generating. It may
help separate rule generation from value binding, effects, type construction,
or verification.

This is plausible, but presently speculative. Transformer attention heads do
not become correctly oriented merely because a syntax looks structured to a
human. The claim should be tested by comparing error rates, repair rates,
context use, and audit quality across equivalent tasks and languages.

The stronger reason Futuruna may help AI is less mysterious: the language gives
important legal concepts explicit forms. An LLM does not have to simulate a
default rule through an informal convention if the target language already has
default and exception semantics. Better representational tools can produce
better generated artifacts, regardless of whether the runes confer an
additional model-specific advantage.

## How the Syntax Should Be Judged

The next evidence should come from use:

1. Can legal practitioners trace an output back to the rules and source text
   that produced it?
2. Can programmers combine those rules with ordinary computation without
   building a second integration layer?
3. Can the auditor expose conflicts, gaps, unreachable rules, circularity, and
   surprising consequences with minimal counterexamples?
4. Can unfamiliar readers predict what a rune permits before consulting the
   grammar?
5. Do people and AI systems make fewer structural mistakes when translating
   the same source material?

If the runes fail those tests, an optimization score cannot rescue them. If
they pass, the optimizer becomes what it should be: an interesting footnote to
a useful design.

## Conclusion

Futuruna's syntax began with an unusual experiment, but the experiment is not
the pitch.

The pitch is that law should be expressible as something people and machines
can run, test, combine with ordinary software, and audit. The seven runes are a
compact attempt to make that possible without splitting the work across
separate languages.

That is a practical proposition. It can be tested against real statutes,
contracts, policies, programs, and users. That is where Futuruna should earn
its claims.
