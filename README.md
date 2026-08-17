<p align="center">
  <a href="https://futuruna.com">
    <img src="website/assets/logo.svg" width="128" alt="Futuruna logo">
  </a>
</p>

<h1 align="center">Futuruna - Law Programming</h1>

<p align="center">
  <a href="Cargo.toml">Version 0.1.0</a>
  ·
  <a href="https://futuruna.com">futuruna.com</a>
  ·
  <a href="LICENSE">MIT License</a>
</p>

## Welcome to Futuruna

Futuruna is a programming language, able to encode complex law, but preserving strength in normal programming structures and algorithms.

It allows combining different paradigms through partitioned syntax, using front-runes for each approach-category. Designed for humans and AIs alike.

## AI Setup

I recommend setting up Futuruna using Codex, Claude Code, ChatGPT Work, Claude Cowork and the strong models it provides (ChatGPT Sol or above, Claude Fable or above, Grok 4.7 or above)

## Skills, Integration and Examples

Futuruna comes with AI integration as a first-class integration.

When you encode rules, you are able to generate an Excel sheet of the rule model for the law, contract or compliance you set out to formalize. The AI can then interview you back, helping you fill out this sheet, so that you can check how your own case stacks up.

Futuruna has encoded the entire Danish Income Tax Law, and you should be able to audit your own Annual Tax Report using Futuruna's code. This is an active research project, so I am very interested in feedback and results from such audits. I truly believe Futuruna can help make a more transparent, just and fair world, encoding the law in an unbiased and auditable fashion. Let us walk into that future, together (be kind in your issue reports) :)

## The Basics

Futuruna can define types (#), functions (>), rules (|), streams (~), assignments (=), proofs (?), and effects (@).

Each front rune has their own rules for syntax, which might sound like it gets complicated, but as you will quickly experience, it creates non-competing syntax within each category space and actually lowers complexity of programs. More punch per character through high optionality for the authors yet less uncertainty for the readers.

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

Work through the [guided tutorial](https://futuruna.com/docs/tutorial), or use
the [full language documentation](https://futuruna.com/docs) as a reference.

## Why Futuruna - The Manifest

([The Why Futuruna Article](https://futuruna.com/why))

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
