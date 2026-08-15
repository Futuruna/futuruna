<p align="center">
  <a href="https://futuruna.com">
    <img src="website/assets/logo.svg" width="128" alt="Futuruna logo">
  </a>
</p>

<h1 align="center">Futuruna — Law Programming</h1>

<p align="center">
  A programming language for expressing, running, testing, and auditing law.
</p>

<p align="center">
  <a href="Cargo.toml"><code>v0.1.0</code></a>
  ·
  <a href="LICENSE">MIT License</a>
</p>

## Welcome to Futuruna

Futuruna is a programming language designed to encode complex law while
remaining strong in ordinary programming structures and algorithms.

It combines different programming paradigms through partitioned syntax, using
a front rune for each approach category. Futuruna is designed for humans and
AI systems alike.

## AI Setup

I recommend setting up and exploring Futuruna with an agentic development
environment such as Codex, Claude Code, ChatGPT Work, or Claude Cowork, backed
by a strong model—for example, ChatGPT Sol or later, Claude Fable or later, or
Grok 4.7 or later.

Give the agent this repository and ask it to build Futuruna, run the mint gate,
and guide you through your first program.

## Skills, Integration and Examples

AI-assisted use is a first-class design goal for Futuruna. The repository
includes an [AI bootstrap script](website/public/ai-bootstrap.sh), and its
typed calculation interface gives AI assistants and humans the same explicit
contract.

When you encode a law, contract, or compliance policy as typed rules, Futuruna
can generate an Excel workbook (`.xlsx`) from the calculation model. The
workbook carries the model's fields, labels, interview questions, help text,
units, validation choices, and source links. An AI assistant can use those
questions to interview you, help map your answers into the workbook, and
submit the completed case to Futuruna. Futuruna remains the deterministic
calculator: it validates the facts, runs the rules, and returns the result and
rule trace.

See [Typed Calculations](docs/reference/calculations.md) for the workbook,
schema, and invocation workflow.

### Danish Personal Income Tax

The [Danish income-tax corpus](examples/danish-income-tax/) is an active
research encoding of Denmark's personal-income-tax calculation. It combines
Personskatteloven with the supporting laws and source material that its public
[`beregn_personskat`](examples/danish-income-tax/personskat.calculate.runa)
calculation depends on. You can use the generated Personskat workbook to audit
how your own annual tax assessment stacks up against the encoded model.

Three anonymized annual assessments for 2023, 2024, and 2025 have already been
reproduced to the øre from typed source facts. Representative direct JSON and
generated XLSX cases produced identical complete result trees. Read the
[Personskat research overview](examples/danish-income-tax/website-overblik.md)
for the method, evidence, and current scope.

This is an active research project, so I am especially interested in feedback
and results from independent audits. I believe Futuruna can help make law more
transparent, just, and fair by encoding rules in an open, inspectable, and
auditable form. Let us walk into that future together—and please be kind in
your [issue reports](https://github.com/Futuruna/futuruna/issues). :)

> [!IMPORTANT]
> Futuruna is research software, not individual legal or tax advice, and not
> every possible tax situation has been proven covered. Verify important
> results against official sources and qualified professionals.

## The Basics

Futuruna defines seven statement categories:

| Front rune | Category |
| --- | --- |
| `#` | Types |
| `>` | Functions |
| `\|` | Rules |
| `~` | Streams |
| `=` | Assignments |
| `?` | Proofs |
| `@` | Effects |

Each front rune has its own syntax rules. That may sound more complicated at
first, but the separation creates non-competing syntax within each category
space and lowers the complexity of programs in practice.

The result is more expressive power per character: high optionality for
authors, with less uncertainty for readers.

## Why Futuruna — The Manifest

[Read the full “Why Futuruna” article.](docs/why.md)

## MIT License

Copyright (c) 2026 Andreas Rudolph

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
