# Explanations of calculation results

Status: Experimental (`runa call --explain`, Futuruna 0.3.1).

## Problem

An auditor reading a Futuruna result, or a backend storing it, must be able
to see for any amount which rule produced it, which exception applied, which
paragraph of the law that rule implements, and the law's own words. Before
this change the link from a rule to its law text existed only in the source:
a law section is an anchor `--@label:<section>::meta:<binding>--` followed by
a `----` quote block (`[sources/<file>]` lines and verbatim law text), and the
rules between `--@begin:<section>--` and `--@end:<section>--` implement it. A
section usually quotes several paragraphs, so "the section" is too coarse to
cite.

## What an explanation is

`runa call --explain` adds an `explanation` to every case result:

- `values`: every leaf of the result (a number, string, Bool, or the tag of a
  variant without fields) by its output path, in the path syntax of needs
  (`outcome.statement.excess_levy_ore`, `accounts[1].kind.$variant`). Each
  says what produced it (`by`): a rule clause, the input unchanged (with its
  input path), a computation outside any rule, or a constant of the model;
  and what it was computed from (`from`): the nearest rules and inputs
  behind it, each with its value.
- A rule clause names the rule, its RuleScope, whether its default or one of
  its exceptions applied (with the exception's label), its `file:line`, the
  law section around it, and the paragraphs it cites with their quoted text.
- `sections`: each law section an explained clause sits in, with its
  location, its `[sources/...]` documents, its metadata attachments
  (`Source`, `Guidance`, `Assumption`, ...) and the whole quote.
- `--explain=full` adds `derivation`: every rule node a reported value was
  computed from, by node number, each with its own clause and contributors.
  The default reports at most 32 direct contributors per value and counts the
  rest in `from_omitted`.

`runa schema --format explanation` prints the JSON Schema. `--partial
--explain` explains every `Complete` result; a `Needs` result has nothing to
explain. `--explain` writes JSON only and is refused with `--partial=check`.

## Mechanism: traced values and a provenance graph

Provenance lives on values, not on a trace of the evaluation. While an
explained case runs, a value that came from the input or from a rule clause
is a `Value::Traced`: the plain value and a node of the case's provenance
graph.

- Decoding tags every input leaf with an `Input` node holding its path.
- When a rule clause returns, every leaf of its value becomes a `Rule` node
  naming the clause, linked to the node the leaf had inside the clause.
- An operator or builtin over traced operands gives a `Computed` node over
  theirs. An identity operation (`x + 0`, `x * 1`) passes its operand's node
  through, so a value moved by arithmetic that changes nothing keeps its
  origin. Builtins that only move values (`map`, `filter`, `foldl`'s
  accumulator, `head`, `pair`) keep the elements' traces; builtins that read
  values (`sum_list`, `max_int`, `day_of_year`) compute from all of them.
- A clause that returns a single value claims it, even when the value is an
  input or another rule's result passed on unchanged: the auditor sees the
  rule that decided the amount, with the input or the inner rule in `from`. A
  clause that returns a record or a list claims only the members it computed;
  a member it passes on from the input or another rule keeps that origin, so
  an input carried into the result is reported with its input path.
- Nodes are interned per case: the same computation from the same nodes is
  one node, so a fold over many items, or a rule called many times with the
  same arguments, adds no node beyond the first.

The alternative, recording an execution trace and slicing it afterwards,
needs the whole trace in memory and a separate dependency analysis; tagging
values gives the dependency directly and costs memory only for distinct
provenance.

### Never changing a result

Every site that decides on a value reads it untraced: `if` and `match`
conditions, rule guards, patterns that are not plain variables, builtins that
read a scalar, predicate callbacks. Equality, hashing, ordering and display
see through a trace. A traced value therefore computes exactly what the
plain value computes, and the encoded result is the same with and without
`--explain`; tests compare the two byte for byte on the tax engine's cases
and interviews.

### Cost when off

Traced values exist only in explained cases. With the mode off, the
interpreter's creation sites test one `Option`, and the sites that read
values have one more `match` arm. A test build counts traced values and
nodes; a test shows an unexplained case creates none.

## Paragraph citations: `--@cite`

A rule clause names the paragraphs it implements in a comment on the lines
above it:

```runa
--@cite:§ 9, stk. 3--
| excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> share_of(positive_part(excess_days_ore), levy_bp) / levy_days_per_year()
```

Several paragraphs are separated by `;` (`§ 14; § 15, stk. 2`); `nr. N` names
a numbered item; `in <source>` names the quoted document when two documents
of one quote hold the same paragraph number. A citation on the first clause
of a rule covers its other clauses unless they carry their own.

`runa check` reads a citation's paragraphs from the quote of the section
around the clause, the way the quote marks them: a line starting `§ N.` opens
a paragraph (its first subsection), `Stk. N.` a later subsection, `N)` a
numbered item, and a `[sources/...]` line the next document. A citation
naming a paragraph the quote does not hold is an error that lists what the
quote holds:

```text
citation `§ 9, stk. 4`: § 9, stk. 4 does not appear in the quote of this section; the quote holds § 9, stk. 1, 2, 3 (section `ask_ceiling`)
```

A citation outside a law section, not directly above a rule clause (comments
may come between), or twice on one clause is an error too. An explanation
shows a cited paragraph's quoted lines verbatim; a clause without a citation
shows its section.

Why a comment marker: the section, its quote and its code span are already
`--@` comments, so the citation sits in the same annotation layer, is read by
the same scanner, and changes neither the grammar, the formatter nor
evaluation. It attaches to any clause form, exceptions and RuleScope members
included, which an `@` annotation (a statement before a declaration) does not
reach. Metadata bindings (`Metadata(attachments = ...)`) describe a whole
section; a paragraph is a property of one clause.

## Engine and SDK

The WebAssembly engine's `Model.callExplained(entry, input, full?)` and
`Model.callPartialExplained(entry, input, full?)` return the documents of
`runa call --explain` and `runa call --partial --explain`. `runa sdk ts`
types the explanation (`Explanation`, `ExplainedValue`, ...) and generates
`<entry>Explained` and `<entry>QueryExplained`, which need the transport's
`callExplained` and `callPartialExplained`.

## Limits

- Explanations are an interpreter feature; compiled programs do not explain.
- A value outside any rule built only from literals (an empty sum in a
  function) is `Constant`.
- A single value a rule returns unchanged from a deeper rule or the input is
  reported for the outer rule, with the deeper rule or the input in `from`.
- Map and set elements are explained by their output path but not traced
  into map keys or set membership.
