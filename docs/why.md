# Why Futuruna

## A Programming Language for Law

I have spent a long time thinking about how to make a programming language that
could encode the rule of law. Others have pondered the same question, but by my
estimation, whatever has been made still seems unmade for humans. I think that
this time it might be different.

Futuruna is not a programming language that *only* encodes law. It is a
programming language in which law can be encoded naturally, in combination with
ordinary programming and in the same execution space.

That distinction matters. A law is not only a collection of rules. It contains
definitions, calculations, dates, procedures, defaults, exceptions, evidence,
and consequences. A useful language for law must let those things meet without
forcing the legal model into one language and the surrounding program into
another.

## Prolog: Rules Instead of Instructions

If you are a programming-language nerd - which I am not - you might have tried
Prolog, which I have. It is not easy.

Prolog is innovative in what it achieves. It lets you describe rules rather
than only instructions, and the language itself becomes a search engine for
answers. That seems analogous to law. But Prolog was not made for law in that
way, and God help you if you try to make it carry an entire legal system. It
often feels more like a language for logic-programming and database wizards.

Again, the language *is* innovative. Futuruna would not exist without ideas
that Prolog helped establish. It simply does not fit the whole bill.

## Catala: Law as Code

Then there is a niche newcomer that has inspired me heavily: Catala, sometimes
described as Catala Law.

Catala really does encode laws as code. Its focus is also its limitation: it is
made specifically for legal rules. Ordinary programming is outside its central
domain. I do not speak ill of that ambition - it is a serious and important
project - but a specialized legal language becomes difficult to combine with
broader software ambitions.

Catala follows a pattern that appears naturally in legislation. You describe a
default case and then add conditions and exceptions that modify it. Ordinary
imperative programming often approaches the same problem from the other
direction: guard every special condition first until the final branch contains
what was the law's opening proposition.

You should be able to choose your point of entry. Futuruna lets you express a
default and layer exceptions onto it, or use ordinary functions and explicit
control flow when that is clearer. In my experience, its ergonomics make those
two approaches feel like parts of one language rather than separate worlds.

## Why Build a Language in the Age of LLMs?

You might wonder why anyone would make a programming language today, when an
LLM can generate code in almost any language you ask for.

The problem is that an LLM can only express what its target language can
represent. If you ask it to encode law in a language without first-class ways
to express defaults, exceptions, rules, provenance, and audit demands, the LLM
must simulate those ideas through conventions. It will produce nested branches,
scattered metadata, comments that the runtime cannot check, and duplicated
logic. Then it will do its best to repair the structure it just created while
you keep spending tokens.

You are asking a bicycle to pedal faster until it reaches the Moon, even though
it cannot fly.

To make an LLM do more with code, you have to make the code capable of saying
more. So I did.

Welcome to Futuruna. I hope you enjoy the language and start experimenting with
it yourself.

## The Quick Fly-In

Futuruna uses a simple but unusual approach: you usually put a rune at the
front of the line you are about to write. The rune tells you which semantic
territory the line belongs to. That makes it possible to mix classical
programming with rule-based programming while keeping the different kinds of
statements visually distinct.

I know. Who would have thought? Is it a bad idea, and is that why nobody else
has done it?

The syntax was explored with an NSGA-II optimizer that compared candidate
structures using measures of optionality, distinguishability, and structural
independence. The experiment repeatedly favored strong categories at the start
of statements. That was useful evidence and an interesting way to explore a
design space. It was not proof that Futuruna is universally optimal, and it is
not the reason the language should exist.

The language stands or falls on a practical question: do the runes make real
programs and legal models easier to write, read, combine, and audit?

The seven runes act as semantic modes. `#` introduces what exists, `>` what
happens, `|` what should hold or which alternative applies, `=` what is, `~`
what flows, `@` where effects and boundaries begin, and `?` where evidence is
demanded. You can explore [the seven runes in the language
reference](/docs).

I also suspect that Futuruna could work exceptionally well with AI. A
line-initial rune may orient a model toward the kind of statement it is about
to produce before it generates the rest of the line. That is a hypothesis, not
an established result. Time will tell.

## Rust Underneath

Futuruna is not made completely from scratch. It transpiles to Rust, an
extremely fast and memory-safe systems programming language.

Futuruna removes several obstacles that Rust beginners commonly struggle with,
especially explicit ownership decisions in ordinary value-oriented code. The
generated program still goes through the Rust compiler, which remains the final
safety check. When low-level control is necessary, Futuruna can cross into Rust
explicitly.

This gives Futuruna native-code performance, access to a wide range of Rust
libraries, and a mature compilation target without requiring every Futuruna
programmer to begin by mastering lifetimes and borrow annotations.

## Welcome

Okay, maybe I am a programming-language nerd, or have somewhat become one.

I invite you to try Futuruna. Start encoding laws, contracts, policies, and
other rule-bound documents, then audit them. Or build ordinary programs that
mix calculations and rules in a compact, readable form.

Let your imagination run wild, and let Futuruna run with it.
