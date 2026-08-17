# Philosophy of Futuruna

Futuruna begins with a practical ambition: one language should be able to
express law and ordinary programs in the same execution space without becoming
verbose, fragmented, or cluttered.

The seven front runes are the mechanism. The idea behind them is
**partitioned syntax**.

A broad language needs many possible forms. But if every form competes inside
one shared grammar, adding capabilities also adds keywords, contextual rules,
special delimiters, and opportunities for collision. Futuruna partitions that
space before the rest of the line begins.

The result is a useful duality:

> More choices for the author, less uncertainty for the reader.

## Partitioned Syntax

Consider a language with several semantic categories of statements: types,
functions, rules, values, streams, effects, and verification demands.

Without a dedicated category channel, those forms share one statement-entry
grammar. The language can still distinguish them with words such as `struct`,
`fn`, `rule`, `let`, and `assert`, but the distinction must be encoded as part
of the ordinary syntax of each line.

Futuruna instead reserves the first character as a semantic entry point:

| Rune | Semantic entry point |
|------|----------------------|
| `#` | What exists |
| `>` | What happens |
| `\|` | What should hold or apply |
| `=` | What is |
| `~` | What flows |
| `@` | Where effects and boundaries begin |
| `?` | What needs evidence |

Each rune selects its own local grammar. Let `L_i` be the set of valid
continuations belonging to rune `r_i`. The complete statement language is:

```text
L_front = r1 L1 ∪ r2 L2 ∪ ... ∪ rN LN
```

Because the front runes are distinct, their grammatical namespaces cannot
collide:

```text
ri Li ∩ rj Lj = ∅    when i ≠ j
```

The parser can select the correct namespace after one character. More
importantly, two namespaces can reuse similar continuation shapes without
becoming ambiguous. The distinction has already been made.

This is not unrestricted optionality, where every token can follow every other
token. It is optionality divided into named, non-conflicting regions.

## The Optionality Gain

Assume that, under a fixed readability or complexity budget, one local grammar
can comfortably support `M` recognizable forms.

Without another discriminator, those `M` surface forms must be shared by all
semantic categories. With `N` independent front-rune namespaces, every
namespace can have its own set of forms:

```text
Unpartitioned capacity = M
Partitioned capacity   = M1 + M2 + ... + MN
```

If every namespace supports the same `M` forms:

```text
Partitioned capacity = N × M
Capacity gain        = N
```

For Futuruna's seven runes, the maximum local grammar multiplier is seven. This
does not make Futuruna seven times more computationally expressive. It means
that, under the same local syntactic budget, seven semantic categories can
reuse grammatical space without competing for the same surface forms.

That is the simple mathematical argument behind the front-rune system.

## The Shannon View

The same idea can be expressed through Shannon entropy.

Let `R` be the rune at the start of a statement, and let `p_i` be the
probability of rune `r_i`. The information carried by observing the rune is:

```text
H(R) = -Σ p_i log2(p_i)
```

If all `N` runes are used equally:

```text
H(R) = log2(N)
```

With seven equally likely runes:

```text
log2(7) ≈ 2.81 bits
```

The first character can therefore provide up to 2.81 bits of immediate
semantic orientation. Before the rune is chosen, the author has seven
categories available. After it is read, the reader, parser, editor, or AI no
longer has to infer which category the line belongs to.

If rune usage is uneven, the effective number of active namespaces is:

```text
N_effective = 2 ^ H(R)
```

This is more honest than assuming that all seven runes always contribute
equally. A codebase dominated by two runes has less effective partitioning than
one that meaningfully uses the full language.

If `C` represents the continuation selected after a rune, the total choice in a
statement is:

```text
H(R, C) = H(R) + H(C | R)
```

The rune contributes category choice. The continuation contributes choice
inside that category. Futuruna separates the two channels instead of forcing
both decisions to emerge from the same undifferentiated syntax.

## More Choice, Less Ambiguity

Optionality usually appears to conflict with clarity. A language with more
features has more forms to remember, more keywords to recognize, and more
interactions between constructs.

Partitioning changes that relationship.

For the author, a front rune opens a semantic namespace with its own valid
continuations. For the reader, that same rune immediately closes six other
namespaces. The author receives more structured choices while the reader's
search space becomes smaller.

In parser terminology, the runes give the top-level statement grammars
disjoint first sets. Top-level dispatch requires one symbol of lookahead. In
human terms, the first character says what kind of thought follows.

This is why the front-rune system is more than character-level compression. Its
main value is not that `>` is shorter than `function`. Its main value is that
`>` reserves an independent grammatical space for things that happen.

## High Paradigm Coverage

Futuruna is intended to cover several programming paradigms without looking
like several languages bolted together.

The runes provide stable entry points for that coverage:

- `#` contains domain models, algebraic data types, traits, and declared
  effects.
- `>` contains ordinary functions, actors, and modules.
- `|` contains logic rules, legal defaults and exceptions, match alternatives,
  handlers, and invariants.
- `=` contains values, bindings, and established facts.
- `~` contains streams and temporal behavior.
- `@` contains effects, metadata, imports, and foreign-code boundaries.
- `?` contains checks and verification demands.

These are not seven isolated mini-languages. They share values, types,
functions, and execution. A legal rule can call an ordinary function. A
function can use a modeled type. A stream can carry the result of a rule. A
verification demand can inspect the same values the program executes.

That shared execution space is what gives Futuruna high paradigm coverage. The
language can absorb a new capability without necessarily inventing a new
top-level syntax family. If the capability belongs coherently inside an
existing semantic territory, it can reuse that rune's grammatical namespace.

This keeps the surface language compact while allowing its capabilities to
grow.

## Compression Without Collision

Conventional keywords already perform some of the work of runes. `fn`, `let`,
`struct`, and `assert` are category markers. In that sense, they are verbose
front runes.

Futuruna's difference is consistency. The category channel is:

- mandatory for most statements;
- fixed at the beginning of the line;
- one character wide;
- drawn from a small semantic alphabet; and
- independent from the syntax that follows.

At the character level, this compresses a category into one symbol. At the
grammar level, the larger gain is that syntax can be reused inside separate
namespaces without collision.

At the language-model token level, one character is not automatically one
token, and shorter source is not automatically easier source. Those are
empirical questions. The formal claim is narrower: distinct front runes make
the statement categories immediately and unambiguously distinguishable.

## The Coherence Constraint

Partitioned syntax works only when each partition remains coherent.

The `|` rune is deliberately broad. Rules, defaults, exceptions, match arms,
handlers, and invariants are different constructs, but they all qualify a
proposition or select among alternatives. That common idea gives the rune a
stable semantic territory.

The design test is:

> Can every use of a rune be explained by one stable sentence?

If the answer is no, the feature has not found a semantic home. Putting it
under the least inconvenient rune would increase feature count while reducing
clarity. It should instead be redesigned, explicitly distinguished, or left
out.

High paradigm coverage is valuable only when it comes from coherent reuse. A
rune must not become a miscellaneous bucket.

## Law and Ordinary Programming

This philosophy matters particularly for law.

Executable law needs more than rules. It needs types for legal concepts,
defaults and exceptions, ordinary calculations, dates and flows, effects at
system boundaries, and demands for tests or evidence. Splitting those needs
across separate languages creates translation boundaries precisely where an
audit needs continuity.

Futuruna keeps them together:

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

The runes do not merely label different lines. They allow rule-based and
ordinary programming to remain visibly distinct while operating over the same
types and values.

## Humans, Parsers, and AI

Partitioned syntax gives three different consumers the same early signal.

**The parser** receives deterministic top-level dispatch after one symbol.

**The human reader** receives the statement category before reading its name,
arguments, or body.

**An AI system** receives an early category marker that may constrain the
continuations it should consider. It is plausible that this improves
generation and repair, but that remains an empirical hypothesis. It should be
tested against equivalent tasks in keyword-based languages rather than assumed
from the entropy argument alone.

The mathematical result concerns the source grammar. Any cognitive or
language-model advantage beyond that still requires evidence.

## What This Does Not Claim

Partitioned syntax does not make Futuruna more Turing-complete than
another general-purpose language. It does not prove that seven is the only
correct number of categories. It does not guarantee that every rune is equally
useful, or that every feature placed under a rune belongs there.

Keyword-based languages can construct the same kind of partition. They may
choose multi-character words because those words are more familiar or
self-describing. Futuruna chooses a compact, fixed-position alphabet because it
values visual distinction, grammar reuse, and the ability to mix paradigms.

The `N × M` gain also depends on a fixed local complexity budget. With unlimited
keywords, arbitrary lookahead, and no concern for human comfort, an
unpartitioned grammar can continue growing. The relevant constraint is not
computability. It is how much local syntactic complexity a person or tool can
comfortably navigate.

## Conclusion

The philosophy of Futuruna is not that punctuation is magical.

It is that a broad programming language can remain compact and legible when its
optionality is partitioned before complexity begins.

The front rune gives the author a semantic point of entry. It gives the grammar
an independent namespace. It gives the reader an immediate category signal.
With `N` runes and `M` comfortable forms inside each namespace, the local
grammar can represent up to `N × M` non-conflicting category/form pairs. In
Shannon's terms, the rune carries up to `log2(N)` bits of category information.

That is partitioned syntax: more choices for the author, less uncertainty
for everyone who reads what comes next.
