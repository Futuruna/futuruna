[![Futuruna logo](https://futuruna.com/apple-touch-icon.png)](https://futuruna.com)

# Futuruna

**A programming language for law.**

Write laws, contracts, and policies you can run, test, and audit.

[Version 0.1.1](Cargo.toml) · [futuruna.com](https://futuruna.com) ·
[MIT License](LICENSE)

## What Futuruna does

Futuruna turns legal rules into programs that people and AI assistants can
inspect together. It keeps rules, defaults, exceptions, calculations, and
ordinary programming in one execution space.

Use Futuruna to:

- encode laws, contracts, and policies as explicit rule models;
- run examples and test the behavior of those rules;
- check invariants and audit decisions against their source-backed model; and
- combine formal rules with normal functions, collections, effects, and streams.

## Set up Futuruna with AI

Use the [Claude app](https://claude.com/download/) or
[ChatGPT app](https://chatgpt.com/download/). For a terminal workflow, use
[Claude Code](https://code.claude.com/docs/en/quickstart) or the
[Codex CLI](https://learn.chatgpt.com/docs/codex/cli).

Give the AI this instruction:

> Read https://futuruna.com/ai-setup.md and set up Futuruna for me.

The guide helps your AI install Futuruna locally, verify the download, and run a
working example. If no ready-made download exists for your computer, the AI
installs it with Cargo or builds Futuruna there instead.

## The seven runes

Futuruna uses seven front runes to make the role of each declaration visible:

| Rune | Role |
| --- | --- |
| `#` | Types |
| `>` | Functions |
| `\|` | Rules and exceptions |
| `=` | Values |
| `~` | Streams |
| `@` | Effects |
| `?` | Checks and proofs |

Here is a complete synthetic tax policy: income is taxed at 25%, except that a
person with at least two children pays 20%.

```runa
# Child(age: Int)
# Person(annual_income: Int, children: List(Child))

| tax_due(person: Person) -> person.annual_income / 4
| exception two_children tax_due(person: Person) -> person.annual_income / 5 under length(person.children) >= 2

= parent = Person(
    annual_income = 500000,
    children = [Child(age = 5), Child(age = 8)]
)

@ print(show(tax_due(parent))) -- 100000
```

## Learn and explore

- [Guided tutorial](https://futuruna.com/docs/tutorial)
- [Language documentation](https://futuruna.com/docs)
- [Why Futuruna](https://futuruna.com/why)
- [Browser playground](https://futuruna.com/playground)
- [Feature stages](docs/feature-stages.md)

Futuruna includes active research models for law and tax. Treat those models as
auditable research software, preserve their sources and assumptions, and do not
use them as individual legal or tax advice.

## License

Futuruna is available under the [MIT License](LICENSE).
