# Why Futuruna

## The Question

You open a file in an unfamiliar language. Within seconds, before you understand a single line, something in you either relaxes or tightens.

That feeling has a number. We measured it.

Think about what happens when you scan code in most languages. Every line starts the same way: a keyword, then an identifier, then braces. `fn add(...)`, `struct Point {...}`, `let x = ...`, `if cond {...}` — the same visual shape every time. To know what a line *does*, you have to read the first word. The syntax tells you nothing for free.

What if the first character told you?

`#` defines a type. `>` defines a function. `|` states a rule. `=` binds a value. `~` declares a stream. `@` marks a side effect. `?` demands verification.

Seven characters. Each one answers the question *what kind of statement is this?* before you read a single word. We call them *runes*. This turns out to change something measurable about how the syntax works — something we can quantify. But first, see what it looks like.

---

## See It First

All seven runes in one program:

```runa
# Condition = Sunny | Cloudy | Stormy
# Weather(day: String, temp: Float, condition: Condition)

> describe(w: Weather) -> String {
    match w.condition {
        | Sunny -> show(w.temp) + " C, sunny"
        | Stormy -> show(w.temp) + " C, storm"
        | Cloudy -> show(w.temp) + " C, cloudy"
    }
}

| advisory(w) -> "all clear"
| advisory(w) -> "heat warning" under w.temp > 35.0
| exception storm advisory(w) -> "danger" under w.condition == Stormy

= today = Weather("today", 22.0, Sunny)
= alert = advisory(today)

@ print(today.day + ": " + describe(today) + " -- " + alert)

~ forecast = from_list([today, Weather("tomorrow", 40.0, Sunny),
    Weather("in 2 days", 18.0, Cloudy), Weather("in 3 days", 10.0, Stormy)])
    |> filter(|w| advisory(w) != "all clear")

= warning_count = count(forecast)
| has_warnings: warning_count -> warning_count > 0

? has_warnings: n -> {
    @ print("Upcoming warnings (" + show(n) + "):")
    ~ forecast | w -> {
        @ print("  " + w.day + ": " + advisory(w) + " -> " + describe(w))
    }
} else {
    @ print("No warnings -- all clear ahead")
}
```

Output:

```
today: 22.0 C, sunny -- all clear
Upcoming warnings (2):
  tomorrow: heat warning -> 40.0 C, sunny
  in 3 days: danger -> 10.0 C, storm
```

Read the first character of each line. `#` defines types. `>` defines a function with `|` match arms inside. Three `|` lines define `advisory` as rules with override logic — the default is "all clear", high temperature overrides to "heat warning", storms override everything. No if/else chain, no Prolog. `=` binds values. `@` prints. `~` declares a reactive stream. `?` checks an invariant and captures the verified value.

Four days go in. Two warnings come out. Day three (18°C, cloudy) triggers no override rule, so the default "all clear" holds and the filter removes it. The logic rules did the work; the stream carried the result.

---

## The Measurement

Take any language. Collect a corpus of code. Classify every token into one of 22 categories — keyword, identifier, type name, operator, brace, and so on. Count how often each category follows each other. Normalize to probabilities. The result is a 22 × 22 grid of numbers: a complete picture of which tokens tend to follow which.

This grid — 484 numbers — encodes an enormous amount about how a language feels to use. We computed three quantities from it.

**Optionality.** From any position in the code, how many meaningfully different continuations exist? If you can go many directions, the syntax has high optionality — many doors open. If everything funnels to the same few patterns, optionality is low — the syntax has already decided for you. We measure this by computing the entropy (the mathematical spread) of reachable states after three transition steps. The idea comes from physics: Wissner-Gross and Freer (2013) showed that systems which keep the most futures open tend to behave intelligently. We call this measure S_tau.

**Clarity.** Optionality alone produces noise. A language where every token can follow every other has maximum optionality and zero readability — every position feels the same. Clarity measures how *distinguishable* different positions are. Can you tell whether you are inside a type declaration or a function body just from the local token context? We measure this using Jensen-Shannon divergence, which computes the average statistical distance between what different positions look like. High clarity means each place in the code has a distinctive feel. Low clarity means everything blurs together.

**Integration.** This is the one that matters most.

When you scan a line of code, your mind tracks several things at once: what kind of statement is this? What types flow through it? How deep am I in the block structure? Each is a question. In most languages, the syntax answers only one of these passively — block depth, conveyed by indentation. The rest require reading. You decode `fn` to learn it is a function. You decode the type annotation to learn what flows. Each decoded answer costs time and attention.

In a language with high integration, the syntax answers multiple questions through *independent* visual cues — cues that carry non-redundant information. Knowing the first character tells you the statement kind; that tells you nothing about the types, which tells you nothing about the block depth. Three questions, three answers, arriving on three separate channels.

Think of a cockpit. Early aircraft had no instruments — you flew by feel and by looking outside. Then came the altimeter: one question answered at a glance. Then the heading indicator: two. Then the full panel — airspeed, attitude, altitude, each in a distinct visual form at a distinct position. A pilot scans them and absorbs three independent facts in parallel, before consciously reading any number. The reason instrument panels work is not that they show more data. It is that each instrument answers a *different* question through a *different* visual channel. Your brain processes them in parallel.

The mathematical tool for counting independent channels is called principal component analysis — it takes a table of correlated data and finds the independent axes hidden inside. Apply it to the rows of the transition grid, and the number of independent axes is what we call **d_eff** — the number of questions the syntax answers for free. The degree to which no single axis dominates is **Phi** (integration), from Integrated Information Theory (Tononi, 2004). High Phi means many independent channels. Phi = 0 means the syntax is a one-note instrument: every line has the same shape, and you must read the words to know what they do.

But here is the crucial point: a 22 × 22 matrix is a finite-bandwidth channel. It can only carry so many independent signals before they drown in noise. The number turns out to be three. Not because of human cognition — because of the mathematics of what 22 token categories can encode. Three independent pathways exhaust the structure available in the transition space. Every existing language uses at most two of them. One remains unclaimed.

### The results

| Language | Optionality (S_tau) | Clarity (JSD) | Integration (Phi) | d_eff |
|----------|--------------------:|---------------:|------------------:|------:|
| Prolog | 2.891 | 0.688 | 0.937 | 2 |
| Haskell | 3.012 | 0.671 | 0.883 | 2 |
| Scala | 3.115 | 0.589 | 0.412 | 1 |
| Python | 2.743 | 0.749 | 0.621 | 1 |
| Rust | 2.987 | 0.634 | 0.000 | 1 |
| Kotlin | 3.045 | 0.612 | 0.000 | 1 |
| Lisp | 2.456 | 0.523 | 0.312 | 1 |
| C | 2.834 | 0.601 | 0.189 | 1 |
| **Futuruna** | **3.537** | **0.784** | **0.980** | **3** |

Rust has Phi = 0.000. The most carefully designed systems language in mainstream use answers *zero* questions for free. Every line starts with a keyword that flows into an identifier that flows into braces — the same visual shape whether you are defining a function, a struct, a module, or an if-block. Rust programmers know this feeling: you open a 500-line file and scroll through blocks that all *look the same* until you slow down and read the first word of each line.

The highest-scoring existing language is Prolog — the only one where clause heads flow differently from clause bodies. Two dimensions. But nobody reaches three.

Futuruna dominates every measured language on all three axes simultaneously. More options than Scala. Clearer than Python. More integrated than Prolog. The only design that reaches the third dimension.

---

## Why Three

Why three dimensions, not four or five? Three independent arguments converge on the same number.

### The channel has finite bandwidth

The transition matrix is 22 × 22 — 22 token categories, 484 numbers. When you apply principal component analysis to its rows, you get eigenvalues that measure the strength of each independent axis. For the Pareto-optimal rune-based designs, three eigenvalues are significant:

- λ₁ = 2.28 (statement kind)
- λ₂ = 1.66 (type flow)
- λ₃ = 0.96 (block composition)

The fourth drops below significance. This is not a cognitive limit — it is an **information-theoretic** one. A 22 × 22 matrix encoding token-to-token transitions is a finite-bandwidth channel. Three independent signals is what that channel can sustain before the signal drowns in noise. Every token must play a role in at least one axis. With 22 token categories, you only have so many degrees of freedom to create genuinely independent pathways. Three axes exhaust the independent structure available in the transition space.

### Seven categories, three pathways

The paper asks: *what are the kinds of things a program can say?* Seven categories emerge — what exists, what happens, what must be true, what is, what flows, where proofs stop, prove it. Seven runes. But these seven map onto only **three structurally distinct routes** through the token graph:

- `>` → IDENT → `(` ... `)` (the definition pathway)
- `|` → IDENT → `→` (the clause pathway)
- `#` → TYPE → `=` (the type pathway)

The other runes (`=`, `~`, `@`, `?`) ride along these existing pathways or create minor variations — they enrich the semantic vocabulary without establishing new *independent* axes through the token graph. A hypothetical fourth axis would need a fourth genuinely independent token chain, and there is no semantic category left that is not already served by one of the three.

### The search found no fourth dimension

We ran NSGA-II — an evolutionary search that optimizes multiple goals at once — over the space of all possible 22 × 22 transition matrices. Population 500, 200 generations, optimizing for optionality, clarity, and integration simultaneously. The result: **122 designs** on the Pareto frontier — designs where you cannot improve any metric without worsening another. **85 achieved d_eff >= 3.** All 85 share one structural feature: statements begin with an operator character, not a keyword.

**None achieved d_eff = 4.** Not because nobody tried — the evolutionary search explored the full design space. Four independent dimensions do not appear on the Pareto frontier. Forcing a fourth independent pathway costs too much on the other metrics: you need so many distinct token transitions that you destroy optionality (too constrained) or clarity (too fragmented). Three is where the trade-offs balance.

No frontier member achieves d_eff = 3 without statement runes. Every member with them achieves d_eff >= 3.

### Why three is also the cognitive sweet spot

Imagine a grid of streets. In a city laid out in one dimension — a single road — a wanderer keeps revisiting the same places. In a flat grid of two dimensions, a wanderer still revisits every intersection eventually. But add a third dimension — overpasses, underpasses, vertical movement — and the wanderer escapes: there is finally enough room to explore without endlessly retreading old ground. This is a real theorem in mathematics (Polya, 1921): three is the smallest number of dimensions where exploration becomes genuinely open-ended.

Human working memory holds 3–5 independent chunks (Cowan, 2001). Three cognitive axes — statement kind, type flow, block composition — hit the sweet spot. Enough structure to separate concerns; few enough to hold the entire model in your head at once. The coincidence is suggestive: the information-theoretic ceiling of the transition matrix, the epistemic decomposition of program statements, the mathematical threshold for open-ended exploration, and the cognitive capacity of working memory all land on the same number.

### The three axes

The three independent axes found in the rune-based transition grid:

- **Axis 1 — Statement kind.** The rune at the start of each line. Your first cognitive act: *what kind of statement is this?*
- **Axis 2 — Type flow.** Type signatures (`Int -> Bool -> String`) flow independently of statement kind. This axis exists in Haskell. In rune-based syntax, it fully decouples from Axis 1 because runes, not keywords, carry the statement's identity.
- **Axis 3 — Block composition.** How braces nest. In C-family languages this axis is tangled with Axis 1 — keywords like `fn` and `class` initiate both statement kind *and* block structure. Runes separate them.

### The bottleneck

Why don't existing languages reach three? They all share a structural pattern.

In Rust, the keywords `fn`, `struct`, `impl`, `let`, and `if` are *semantically* different but *syntactically* identical. They all flow into an identifier followed by braces. The transition grid cannot tell them apart, so the independent dimensions collapse into one. In Kotlin and Scala, every token type connects to every other with roughly equal probability. Maximum syntactic flexibility produces maximum cognitive uniformity.

The constraint that creates cognitive structure is not richness. It is *differentiation*. Keywords overload two functions: they signal statement kind *and* initiate blocks. The rune decouples these. The decoupling creates independence. The independence creates the third dimension.

---

## The Seven Runes

If runes unlock the third dimension, how many do you need? The Pareto frontier constrains the answer: seven to nine produce the optimal balance. Seven is the sweet spot. They were not chosen by taste. They fell out of a question: *what are the kinds of things a program can say?*

| Rune | Question | What it does | Verification target |
|------|----------|-------------|---------------------|
| `#` | What exists? | Types, effects, traits, impls | Z3 datatypes |
| `>` | What happens? | Functions, actors, modules | Z3 functions |
| `\|` | What must be true? | Rules, match arms, handlers | Z3 assertions |
| `=` | What is? | Bindings, ground truth | Z3 constants |
| `~` | What flows? | Reactive streams, time | Temporal logic |
| `@` | Where do proofs stop? | IO, imports, meta | Proof boundary |
| `?` | Prove it. | Verification demands | Solver invocation |

Seven categories. Together they cover a natural decomposition of what programs express — what exists, what happens, what must hold, what is observed, what changes over time, where formal reasoning ends, and where it is demanded.

### The | headline

Today, if you want logic programming, you learn Prolog. Legal rules with exceptions? Catala. Algebraic effects? Koka. Pattern matching? Rust or Haskell. Four languages for four instances of the same idea: *case analysis under different regimes*.

In Futuruna, they are one rune:

```runa
-- Prolog-style inference
| taxable(person) -> resident(person), has_income(person)

-- Catala-style default logic with exceptions
| tax_rate(person) -> 0.20
| tax_rate(person) -> 0.40 under person.income > 50000
| exception tax_rate(person) -> 0.00 under person.income < 12000

-- Algebraic effect handler
| handle Console {
    | say(msg) -> { @ print(msg); resume(()) }
    | ask(prompt) -> resume("default")
} in greet("World")

-- Pattern matching
match shape {
    | Circle(r) -> 3.14 * r * r
    | Rectangle(w, h) -> w * h
}
```

Four capabilities. One character. The reason they unify is structural: logic clauses analyze cases over a search space. Pattern matching analyzes cases of a value. Effect handlers analyze cases of a side effect. Legal rules analyze cases of a regulation. The regimes differ. The operation does not.

---

## What One Rune Replaces

| Rune | Unifies | Scattered across |
|------|---------|-----------------|
| `#` | Types + Effects + Traits | `class`, `struct`, `enum`, `interface`, `trait`, `type` |
| `>` | Functions + Actors + Modules | `fn`, `def`, `func`, `mod`, `actor` |
| `\|` | Logic + Matching + Effects + Law | Prolog (`:-`), Rust (`match`), Koka (`handle`), Catala (`rule`) |
| `=` | Binding + Monadic bind | `let`, `val`, `var`, Haskell `<-` |
| `~` | Reactive streams | RxJS, Reactor, Akka Streams (libraries, not language) |
| `@` | IO + Imports + Build + FFI | `println!`, `use`, `import`, `build.gradle`, FFI |
| `?` | Tests + Asserts + Formal proofs | `assert!`, `#[test]`, external Z3 toolchain |

### Ownership dissolved

Here is a function in Rust:

```rust
fn process(data: &[String], prefix: &str) -> Vec<String> {
    data.iter()
        .filter(|s| s.starts_with(prefix))
        .map(|s| s.clone())
        .collect()
}
```

The programmer had to decide: `&[String]` or `Vec<String>`? `&str` or `String`? `s.clone()` or `s.to_string()`? Every decision is an ownership annotation.

Here is the same function in Futuruna:

```runa
> process(data: List(String), prefix: String) -> List(String) {
    filter(data, |s| starts_with(s, prefix))
}
```

No `&`. No lifetime. No `.clone()`. The compiler sees that `data` is read-only and emits `&[String]`. It sees `prefix` is only borrowed and emits `&str`. The generated Rust is identical to what a careful programmer would write. In an adversarial comparison of 12 ownership patterns that commonly trip Rust newcomers, Futuruna produces valid Rust for all 12 — winning on 5, tying on 7, losing on none.

### Verification as syntax

The `?` rune scales from runtime check to formal proof without changing the source:

```runa
| conservation: supply -> supply >= 0 && supply <= max_supply
? conservation
```

- **`runa run`** — evaluates the predicate with current values
- **`runa build`** — emits `debug_assert!()` in the compiled binary
- **`runa verify`** — translates the invariant into the standard format for theorem provers and invokes Z3 to prove it holds for *all* inputs

One line of code. Three levels of assurance. The code never changes.

---

## What the Type System Sees

The US Constitution encoded in Futuruna. Not a summary — the actual legal structure, expressed as types, rules, and invariants.

```runa
# Branch = Legislative | Executive | Judicial
# Chamber = Senate | House
# PowerHolder = ExclusiveTo(branch: Branch) | ExclusiveChamber(chamber: Chamber) | SharedPower
# PresidingOfficer = VicePresident | ChiefJustice
# CongressionalPower = Taxation | Borrowing | CommerceRegulation | Naturalization
    | Coinage | Counterfeiting | PostOffices | CopyrightPatent | InferiorCourts
    | Piracy | DeclareWar | RaiseArmy | MaintainNavy | MilitaryRules
    | CallMilitia | OrganizeMilitia | DistrictSeat | NecessaryAndProper
```

Then the verifier asks questions:

```runa
? bodies_separated -> { @ print("? Accuser != Trier:              VERIFIED") }
? legislative_is_limited -> { @ print("? Legislative IS enumerated:     VERIFIED") }
? executive_is_unlimited -> { @ print("? Executive NOT enumerated:      VERIFIED") }
? no_vp_conflict -> { @ print("? VP excluded from pres. trial:  VERIFIED") }
? clause_superseded -> { @ print("? Three-Fifths superseded:       VERIFIED") }
```

The compiler discovers what took scholars centuries:

- **ChiefJustice is the exception** in the PresidingOfficer domain. The VP presides over all impeachment trials — except when the VP's own boss is on trial.
- **Legislative powers are enumerated** (18 specific grants). Executive powers are not. The text says "the executive Power" without listing what that includes.
- **The Three-Fifths Clause is superseded** but never deleted. The type system finds the ghost.
- **The pardon power cannot reach impeachment** — but a successor can pardon the underlying crime. The type system flags the gap.
- **No religious test for office** — verified structurally, not by reading prose.

50+ invariants checked. Every VERIFIED result is a structural proof. Every FAILED result is a genuine gap or tension in the original text. The type system does not interpret the Constitution. It *audits* it.

---

## What d_eff Feels Like

What does it feel like when the syntax answers three questions for free?

Consider traffic signs. Imagine a city with no signs — just white text painted on the road surface. "STOP." "YIELD." "SPEED LIMIT 50." Every instruction looks the same until you read the words. That is Phi = 0: zero channels of pre-attentive information. You must decode every message.

Mount them on poles as round white signs with black text. Now you know something is a sign before you read it. One channel: sign versus not-sign.

Add color. Red means prohibition, blue means information, yellow means caution. You react to the color before you read the word. Two channels.

Add shape. Octagons for stop, triangles for yield, rectangles for information. Now shape, color, and text arrive on three parallel channels. You know what *kind* of sign it is, what *category* of instruction it carries, and what the *specific* message says — all before conscious reading begins. Three channels.

Traffic signs work this way because the human visual system processes shape, color, and symbol independently. Programming language syntax faces the same constraint: a programmer scanning unfamiliar code is a driver entering an unfamiliar intersection.

Rust is white text on asphalt. Futuruna is the shaped, colored sign.

---

## How It Works

A single-file Rust compiler (`runa.rs`, ~10,000 lines). Lexer, parser, interpreter, type checker, and Rust transpiler in one file.

| Command | What it does |
|---------|-------------|
| `runa run file.runa` | Interpret directly |
| `runa emit file.runa` | Show generated Rust |
| `runa build file.runa` | Compile to native binary |
| `runa wasm file.runa` | Compile to WebAssembly |
| `runa check file.runa` | Parse + type-check (fast) |
| `runa verify file.runa` | Prove invariants via Z3 |

### Ownership without annotation

Futuruna is to Rust as Kotlin is to Java. You write value semantics; the compiler handles ownership. It traces how each variable is used across the entire program: single-use variables move, multi-use variables clone once then borrow, read-only parameters auto-borrow as `&T`. You never write `&T`, never annotate a lifetime, never call `.clone()`. The escape hatch (`@ rust {}`) covers the remaining 5%.

### The verification pipeline

The runes are not decoration. They are the interface to the verifier. Each maps directly to a category that theorem provers already understand:

- `#` types → declared as algebraic datatypes
- `>` functions → declared as logical functions
- `=` bindings → declared as constants
- `|` invariants → the verifier tries to find a counterexample; if it cannot, the invariant is proven

The syntax *is* the verification interface.

### Transpiles to Rust

The generated Rust inherits zero-cost abstractions, memory safety, and the entire Cargo ecosystem. Native binaries. WebAssembly. No runtime overhead.

---

## Honest Limitations

- **The metrics are proxies.** Optionality, clarity, and integration measure syntactic texture, not programmer productivity. Three cognitive axes *should* improve the experience of reading code, but this has not been tested in user studies. The metrics are a map, not the territory.

- **Corpus dependence.** Transition grids depend on which code represents each language. Systems code and web code produce different numbers. The structural claim (d_eff = 3) rests on the rune mechanism, not the specific metric values.

- **Verification scope.** The `runa verify` pipeline handles integer arithmetic, algebraic data types, and first-order functions. Higher-order functions and floating-point require extensions not yet shipped.

- **Maturity.** Futuruna is a working compiler with 47 passing tests, not a production language. The escape hatch covers the gaps.

The deepest limitation is the first. We measured the structure of token transitions and found something real. Whether the third cognitive dimension translates to the lived experience of writing code is an empirical question this work opens but does not close.

The strongest version of this contribution is not "Futuruna is better" but "measurement reveals structure that intuition missed." If someone builds a better language using different measurements, the methodology wins.

---

## Where the Ideas Come From

**Integrated Information Theory** (Tononi, 2004) provides the framework for integration (Phi). Originally developed to formalize consciousness in neural systems — what makes a system more than the sum of its parts. The question it asks of a brain — "how many independent channels of information does this system integrate?" — turns out to be exactly the right question to ask of a syntax.

**Causal entropic forces** (Wissner-Gross & Freer, 2013) provide optionality (S_tau). Physical systems that maximize the diversity of their accessible futures spontaneously exhibit intelligent behavior. We show the same measure characterizes syntactic freedom: a language that keeps many doors open at each position gives the programmer more room to think.

**Catala** (Merigoux et al., 2021) pioneered legal default logic in a programming language. Futuruna absorbs this into `|`. **Koka** (Leijen, 2017) proved algebraic effects are tractable. **Unison** (Chiusano, 2019) showed content-addressed code eliminates dependency hell. **Zig** demonstrated compile-time evaluation eliminates macros. **Hylo** explored mutable value semantics. Each contributed one idea. Futuruna combines them under a syntax derived from measurement rather than taste.

---

## Conclusion

We measured the cognitive structure of programming language syntax and found that every existing language occupies at most two dimensions. A third dimension is available. It requires a constraint that is not the one intuition suggests.

Not richness. Not flexibility. Not multi-paradigm abundance.

Differentiation.

One character at the start of each line that says: *this is what kind of thing I am about to tell you.* Seven characters, covering every category of program statement we have found. Each one maps to a verification target. Each one creates a distinct cognitive pathway.

The language that results unifies things that have never coexisted: logic programming and algebraic effects and legal default logic and reactive streams, all under a syntax that sits on the Pareto frontier — the boundary where you cannot improve one metric without worsening another. It compiles to native Rust with inferred ownership. It provides formal verification through the same runes that structure the code. And it occupies a region of the design space that no existing language has reached.

Remember the feeling. You open a file and within seconds you know whether you are in friendly territory. That feeling has a name now. It is d_eff — the number of questions the syntax answers before you read a single word.

Every existing language leaves at least one of those questions unanswered.
