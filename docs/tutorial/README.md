---
feature_stage: mixed
feature_stage_surfaces:
  - core-language-syntax
  - source-meta-comment-tooling
  - typed-program-references
  - exploratory-audit-tooling
---

# Build a Rule-Driven Tax Program

This tutorial builds a small, synthetic tax policy from types and rules, then
uses it in a scenario, traces it to typed metadata, and audits an actual
same-rule contradiction. It is a language example, not Danish tax law.

You will use three files already included in the repository:

- `examples/tutorial_tax.runa` contains the reusable model.
- `examples/tutorial_tax.scenario.runa` contains one concrete case.
- `examples/tutorial_tax.audit.runa` contains an intentional same-rule contradiction.

Core language syntax is **Stable**. The metadata index is **Preview**, and the
exploratory audit command is **Experimental**.

## 1. Run One Rule

A rule starts with `|`. A named exception with a satisfied `under` condition
takes precedence over the default rule.

```runa
# Child(age: Int)
# Person(annual_income: Int, children: List(Child))

| tax_due(person: Person) -> person.annual_income / 4
| exception two_children tax_due(person: Person) -> person.annual_income / 5 under length(person.children) >= 2

= parent = Person(
    annual_income = 500000,
    children = [Child(age = 5), Child(age = 8)]
)

@ print(show(tax_due(parent)))
```

Save that as `tax.runa`, then run it:

```bash
runa run tax.runa
```

The output is `100000`: the exception selects the 20% rate for this family.

## 2. Give Rules a Shared Case

Real models quickly gain several rules that need the same facts. A product
RuleScope keeps those facts together and lets its rule members call each other:

```runa
# TaxCase(person: Person) {
    | rate_percent() -> 25
    | exception two_children rate_percent() -> 20 under length(person.children) >= 2
    | tax_due() -> person.annual_income * rate_percent() / 100
}
```

Constructing `TaxCase` makes the rules active for one concrete person. Calling
`family_tax.tax_due()` follows the rule cascade through `rate_percent()`:

```runa
= family_tax = TaxCase(person = parent)
= due = family_tax.tax_due()
```

The constructor uses named arguments, and each member has direct access to
`person`. This avoids threading the same parameter through every rule.

## 3. Attach Traceable Metadata

Metadata stays ordinary, typed Futuruna data. The meta comment attaches one
value to a label, while the matching `begin` and `end` markers identify the
code covered by that label.

```runa
# SourceInfo(title: String, url: String)
# TutorialRuleMetadata(source: SourceInfo, target: ProgramReference)
# impl Meta for TutorialRuleMetadata {}

= source = SourceInfo(
    title = "Synthetic family tax policy",
    url = "https://futuruna.com/docs/tutorial"
)

= metadata = TutorialRuleMetadata(
    source = source,
    target = refof(TaxCase::tax_due)
)

--@label:family_tax_rule::meta:metadata--
----
Synthetic tutorial policy: income is taxed at 25%, except that a person with
at least two children pays 20%.
----

--@begin:family_tax_rule--
# TaxCase(person: Person) {
    | rate_percent() -> 25
    | exception two_children rate_percent() -> 20 under length(person.children) >= 2
    | tax_due() -> person.annual_income * rate_percent() / 100
}
--@end:family_tax_rule--
```

`ProgramReference` and `refof(TaxCase::tax_due)` make the target structural and
checked against the program. Index the canonical example with:

```bash
runa meta examples/tutorial_tax.runa
```

The metadata does not change the calculation. It gives tools a typed route from
an explanation or source to the code that implements it.

## 4. Prove a Concrete Scenario

A `.scenario.runa` file imports the reusable model, supplies facts, and proves
the expected result:

```runa
@ import ./tutorial_tax

= parent = Person(
    annual_income = 500000,
    children = [Child(age = 5), Child(age = 8)]
)

= family_tax = TaxCase(person = parent)
= due = family_tax.tax_due()

| two_child_tax_is_100000: due -> due == 100000

? two_child_tax_is_100000(due)
@ print(show(due))
```

Run the checked-in scenario:

```bash
runa run examples/tutorial_tax.scenario.runa
```

The proof rune `?` fails the run if the expected amount stops being true.

## 5. Audit an Actual Contradiction

The reusable tax model is coherent. To exercise the experimental auditor, this
separate fixture adds two exception branches that answer the same question for
the same family:

```runa
@ import ./tutorial_tax

= parent = Person(
    annual_income = 500000,
    children = [Child(age = 5), Child(age = 8)]
)

= family_tax = TaxCase(person = parent)

| exception family_policy reduced_rate_applies() -> family_tax.rate_percent() == 20 under length(parent.children) >= 2
| exception audit_only_income_exclusion reduced_rate_applies() -> False under parent.annual_income >= 500000
```

Run the exploratory audit:

```bash
runa audit examples/tutorial_tax.audit.runa
```

Both conditions hold: the parent has two children and earns `500000`. The
`family_policy` branch returns `True`, while the audit-only exclusion returns
`False`. Because both are active exceptions for the same zero-argument rule,
the report identifies one **contradiction**.

Source order still gives execution a deterministic result, but the audit exposes
the incompatible equal-priority outcomes. Separate rules are never paired by
their names: a semantic contradiction must occur within one rule identity and
one concrete evaluation environment.

## Keep Going

Use the [language reference](https://futuruna.com/docs) for exact syntax, open
the [playground](https://futuruna.com/playground) for quick experiments, or
continue through the longer tutorial sequence with [Hello,
Futuruna](https://github.com/Futuruna/futuruna/blob/main/docs/tutorial/01-hello.md).
