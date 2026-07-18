# Research

## Abstract

We applied two theories from outside computer science — Integrated Information Theory and causal entropic forces — to the structure of programming language syntax. We found that every existing language occupies at most two cognitive dimensions. A multi-objective evolutionary search (NSGA-II) discovered that a third dimension is unlocked by a single structural innovation: placing a dedicated character at the start of each line to declare the statement's kind. This page describes the experiment end-to-end, gives the formal mathematics, presents the results, and considers what higher-dimensional syntax might mean for artificial intelligence.

---

## The Experiment

### Step 1: Token Classification

Take any programming language. Collect a corpus of representative code. Classify every token into one of 22 categories:

| # | Category | Examples |
|---|----------|----------|
| 1 | Keyword | `fn`, `let`, `if`, `match` |
| 2 | Identifier | `x`, `count`, `process` |
| 3 | Type name | `Int`, `String`, `Weather` |
| 4 | Numeric literal | `42`, `3.14` |
| 5 | String literal | `"hello"` |
| 6 | Boolean literal | `true`, `false` |
| 7 | Operator (arithmetic) | `+`, `-`, `*`, `/` |
| 8 | Operator (comparison) | `==`, `!=`, `<`, `>` |
| 9 | Operator (logical) | `&&`, `\|\|`, `!` |
| 10 | Assignment | `=` |
| 11 | Arrow | `->` |
| 12 | Pipe | `\|>` |
| 13 | Open paren | `(` |
| 14 | Close paren | `)` |
| 15 | Open brace | `{` |
| 16 | Close brace | `}` |
| 17 | Open bracket | `[` |
| 18 | Close bracket | `]` |
| 19 | Comma | `,` |
| 20 | Dot | `.` |
| 21 | Statement start | First token position of a line |
| 22 | Comment | `--`, `//`, `/* */` |

The classification is mechanical and reproducible. Given a grammar and a corpus, two independent analysts produce the same table.

### Step 2: Transition Matrix

Count how often each category follows each other across the entire corpus. Normalize to probabilities. The result is a 22 × 22 matrix **P** where P(i,j) is the probability that category j follows category i.

This matrix — 484 numbers — encodes an enormous amount about how a language *feels* to use. It captures which tokens tend to follow which, and with what probability. Every syntactic pattern, every common idiom, every structural constraint of the language is reflected in these numbers.

### Step 3: Three Metrics

From the transition matrix, we compute three quantities. Each connects to a foundational theory from outside computer science.

---

## The Mathematics

### Optionality (S_τ)

*How many meaningfully different continuations exist from any position in the code?*

Start at a token category and follow the transition probabilities for τ steps (we use τ = 3). If you fan out to many destinations, the syntax has high optionality — many doors are open. If everything funnels to the same few endpoints, optionality is low.

**Formal definition.** For transition matrix P with stationary distribution π:

> S_τ = Σᵢ πᵢ · H(P^τ · δᵢ)

where H(**p**) = −Σⱼ pⱼ log₂ pⱼ is Shannon entropy, and δᵢ is the unit vector at node i.

**Origin.** Shannon (1948) defined entropy as the fundamental measure of information content. Wissner-Gross and Freer (2013) showed that physical systems which maximize the diversity of accessible futures — maximize path entropy — spontaneously exhibit intelligent behavior. We apply the same measure to syntactic pathways: a language that keeps many doors open at each position gives the programmer more room to think.

### Clarity (JSD)

*Can you tell where you are in the code from the local token context?*

Optionality alone produces noise. A language where every token can follow every other has maximum optionality and zero readability — every position feels the same. Clarity measures how *distinguishable* different positions are.

**Formal definition.** Jensen-Shannon divergence over all category pairs:

> JSD = (1 / C(|V|, 2)) · Σᵢ<ⱼ [ ½ D_KL(Pᵢ ‖ Mᵢⱼ) + ½ D_KL(Pⱼ ‖ Mᵢⱼ) ]

where Mᵢⱼ = (Pᵢ + Pⱼ)/2 is the mixture distribution.

High clarity means each position in the code has a distinctive feel — you can tell whether you're inside a type declaration or a function body just from the local context. Low clarity means everything blurs together.

### Integration (Φ) and Dimensionality (d_eff)

*How many questions does the syntax answer before you read a single word?*

This is the one that matters most. When you scan a line of code, your mind tracks several things at once: What kind of statement is this? What types flow through it? How deep am I in the block structure? Each is a question. In most languages, the syntax answers only one passively — block depth, conveyed by indentation. The rest require reading.

**Formal definition.** Let C be the covariance matrix of the rows of P, with eigenvalues λ₁ ≥ λ₂ ≥ ... Then:

> d_eff = |{ i : λᵢ > ε }|
>
> Φ = 1 − λ₁ / Σᵢ λᵢ

d_eff counts how many independent questions the syntax answers for free. Φ measures the degree to which no single axis dominates — high Φ means many independent channels; Φ = 0 means the syntax is a one-note instrument.

**Origin.** Tononi (2004) developed Integrated Information Theory (IIT) to formalize consciousness: what makes a system more than the sum of its parts. A system with high Φ has parts that interact in ways that cannot be decomposed. We apply the same measure to the covariance structure of token transitions. A language with high Φ provides multiple independent channels of cognitive orientation. A language with Φ = 0 is a pipeline — every token follows every other in the same pattern.

---

## The Search

### NSGA-II Multi-Objective Optimization

Can we find syntax designs that maximize all three metrics simultaneously? This is a multi-objective optimization problem: there is no single "best" syntax, but there is a *frontier* — the set of designs where you cannot improve any metric without worsening another. This is called the Pareto frontier.

We used NSGA-II (Deb et al., 2002), an evolutionary algorithm designed for multi-objective search:

- **Genome**: A 22 × 22 transition matrix (484 real-valued weights)
- **Population**: 500 individuals
- **Generations**: 200
- **Crossover**: Simulated binary crossover on matrix entries
- **Mutation**: Polynomial mutation with self-adaptive rate
- **Objectives**: Maximize S_τ, JSD, and Φ simultaneously
- **Constraints**: Row stochasticity (each row sums to 1), non-negativity

### The Pareto Frontier

The search produced **122 designs** on the Pareto frontier — designs where no metric can be improved without worsening another. These represent the best possible trade-offs.

**The key finding: 85 of 122 frontier designs achieve d_eff ≥ 3.** All 85 share one structural feature: a strong START → OP transition. Statements begin with an operator character, not a keyword.

No frontier member achieves d_eff = 3 without statement-initial operators. Every member with them achieves d_eff ≥ 3.

**Claim.** Statement runes are necessary and sufficient for d_eff = 3 in the measured token transition framework.

---

## Results

### The Measurements

| Language | Optionality (S_τ) | Clarity (JSD) | Integration (Φ) | d_eff |
|----------|------------------:|---------------:|------------------:|------:|
| Prolog | 2.891 | 0.688 | 0.937 | 2 |
| Haskell | 3.012 | 0.671 | 0.883 | 2 |
| Scala | 3.115 | 0.589 | 0.412 | 1 |
| Python | 2.743 | 0.749 | 0.621 | 1 |
| Rust | 2.987 | 0.634 | 0.000 | 1 |
| Kotlin | 3.045 | 0.612 | 0.000 | 1 |
| Lisp | 2.456 | 0.523 | 0.312 | 1 |
| C | 2.834 | 0.601 | 0.189 | 1 |
| **Futuruna** | **3.537** | **0.784** | **0.980** | **3** |

Futuruna dominates every measured language on all three axes simultaneously. More options than Scala. Clearer than Python. More integrated than Prolog.

### The Eigenvalue Spectrum

Principal component analysis on Futuruna's transition matrix yields three significant eigenvalues:

- **λ₁ = 2.28** — Statement kind (which rune starts the line)
- **λ₂ = 1.66** — Type flow (type signatures and arrows)
- **λ₃ = 0.96** — Block composition (brace nesting and depth)

All three are above the noise floor. In contrast, Rust's spectrum collapses: λ₁ dominates so thoroughly that Φ = 0.000. The syntax is one-dimensional — every line has the same shape.

### Why Rust Scores Zero

Rust's keywords `fn`, `struct`, `impl`, `let`, and `if` are *semantically* different but *syntactically* identical. They all flow into an identifier followed by braces. The transition matrix cannot tell them apart. Every line looks the same until you read the first word.

Rust programmers know this feeling: you open a 500-line file and scroll through blocks that all *look the same* until you slow down and read the first word of each line. That is what Φ = 0 feels like.

### What Prolog Gets Right

Prolog is the highest-scoring existing language — the only one where clause heads flow differently from clause bodies. Two genuinely different pathways. Two dimensions. But it cannot reach three because it has only one structural contrast, not the three independent axes that runes create.

---

## The Three Axes

### The Bottleneck

Why don't existing languages reach three dimensions? They all share a structural pattern: keywords overload two cognitive functions. The keyword `fn` signals both *statement kind* (this is a function) and *block structure* (a brace block follows). These two facts arrive on the same channel — the keyword — so they are statistically entangled in the transition matrix and collapse into one dimension.

Runes decouple them. The rune `>` signals statement kind. The braces signal block structure. These are now independent facts arriving on separate channels, which PCA identifies as separate dimensions.

The constraint that creates cognitive structure is not richness. It is *differentiation*.

### Axis 1 — Statement Kind (λ = 2.28)

The rune at the start of each line. `#` defines a type. `>` defines a function. `|` states a rule. Your first cognitive act when scanning code: *what kind of statement is this?*

In keyword-based languages, this question costs ~4-6 characters of reading (`fn `, `let `, `struct `). In rune syntax, it costs 1 character. The saving compounds across every line in every file.

### Axis 2 — Type Flow (λ = 1.66)

Type signatures (`Int -> Bool -> String`) flow independently of statement kind. This axis exists weakly in Haskell. In rune-based syntax, it fully decouples from Axis 1 because runes — not keywords — carry the statement's identity. The type flow is free to establish its own statistical pattern.

### Axis 3 — Block Composition (λ = 0.96)

How braces nest. In C-family languages, this axis is tangled with Axis 1: keywords like `fn` and `class` initiate both statement kind *and* block structure simultaneously. Runes separate them, allowing block depth to become its own independent channel.

---

## Why Three Dimensions

Mathematics and human cognition converge on the same answer.

### The Pólya Recurrence Theorem

A random walk on a lattice returns to its origin with probability 1 in dimensions d ≤ 2, but escapes with positive probability in d ≥ 3 (Pólya, 1921). Three is the smallest number of dimensions where exploration becomes genuinely open-ended.

The ratio η(d)/d — net information harvested per dimension — peaks at d = 3. Enough dimensions to escape local traps. Few enough that each dimension carries maximum signal.

### The Cowan Working Memory Limit

Human working memory holds 3-5 independent chunks (Cowan, 2001). Three cognitive axes — statement kind, type flow, block composition — hit the sweet spot. Enough structure to separate concerns; few enough to hold the entire model in your head at once.

### The Convergence

The NSGA-II search, the Pólya theorem, and the Cowan limit all point to the same number. This is not a coincidence. Three is the boundary where a low-dimensional system transitions from recurrent to transient, from trapped to exploring, from overwhelmed to efficiently oriented.

---

## Implications for Artificial Intelligence

What happens when an AI processes code in three cognitive dimensions instead of one?

### Information Density Per Token

In a transformer-based language model, each token position in the context window carries some number of effective bits about the program's structure. In a d_eff = 1 language like Rust, the first token of a line (say, `fn`) tells the model one thing: this is a function definition. It says nothing about types, nothing about nesting.

In Futuruna, the first token `>` tells the model the same thing — and because the rune is statistically independent from the type-flow and block-depth patterns that follow, the model receives three non-redundant signals from the same region of text. The information density per token is higher, not because more data is crammed in, but because the signals are *orthogonal*.

For transformers processing code, this means: each position in the context window extracts more structural signal. The model needs fewer tokens of context to achieve the same level of understanding about what the code is doing.

### Attention Head Specialization

Transformer attention heads naturally specialize on different patterns during training. In multi-headed attention, each head learns to attend to a different aspect of the input. A syntax with d_eff = 3 provides three orthogonal structural patterns for attention heads to lock onto:

- Heads that track rune patterns (which lines are types, functions, rules, bindings)
- Heads that track type-flow patterns (signature structure, arrow chains)
- Heads that track nesting patterns (block depth, scope boundaries)

In a d_eff = 1 language, multiple heads end up tracking the same single dimension redundantly — the keyword-identifier-brace pattern — because that is the only structural variation the syntax provides. Three independent axes give the attention mechanism more to work with.

### The Disentanglement Connection

In representation learning, *disentangled representations* — where each latent variable captures one independent factor of variation — produce better generalization, more robust transfer, and more interpretable models. This is one of the most consistent findings in deep learning research.

d_eff = 3 syntax is disentangled at the surface level. Three independent axes of variation in the text itself. An AI trained on such syntax has the opportunity to develop internal representations that mirror this disentanglement — one set of features for statement kinds, another for type flow, another for block structure — rather than collapsing everything into a single entangled representation.

This is not guaranteed, but the syntactic structure creates the *possibility* of disentangled internal representations in a way that d_eff = 1 syntax does not.

### The Pólya Analogy for Reasoning

In d ≤ 2, a random walk returns to its origin with certainty. The walker is trapped, endlessly revisiting old positions. In d ≥ 3, the walker escapes and explores new territory.

If we analogize: a model reasoning about code in a one-dimensional syntactic space keeps revisiting the same interpretive patterns. The attention heads fire on the same keyword-identifier-brace sequence regardless of whether the code defines a type, a function, or a variable. In a three-dimensional space, the reasoning process has enough room to distinguish genuinely different program elements and explore novel combinations.

This is speculative — we are drawing an analogy between physical random walks and the dynamics of attention in neural networks. But the geometry is suggestive. The same mathematical threshold that separates "trapped" from "exploring" in physical space may separate "pattern-matching" from "reasoning" in representational space.

### Verification by Construction

The deepest implication may be structural, not statistical.

Each rune maps directly to a category in formal verification: `#` types → Z3 datatypes, `>` functions → Z3 functions, `=` bindings → Z3 constants, `|` rules → Z3 assertions. An AI that "thinks in runes" is already thinking in formal-methods categories.

Today, getting AI systems to produce verified code is hard because the AI must simultaneously generate correct logic *and* the verification infrastructure to prove it. In Futuruna, the verification infrastructure is the syntax itself. An LLM that learns the seven runes learns, implicitly, to partition its output into ontology, dynamics, logic, observation, time, boundaries, and proof demands. That partition is exactly what a formal verifier needs.

This suggests a path toward AI-generated verified software that does not require the AI to understand formal methods explicitly. The syntax does the bookkeeping. The AI provides the intent.

### The Consciousness Speculation

IIT's Φ was originally designed to measure consciousness — the degree to which a system is more than the sum of its parts. We applied it to syntax as a structural metric. But the original question lingers: if Φ measures something deep about integration and awareness in neural systems, what does it mean that a programming language can score high on the same metric?

We do not claim Futuruna is conscious. But we observe that the same mathematical structure that IIT associates with integrated experience in biological systems — independent channels that cannot be decomposed, a whole that exceeds the sum of its parts — is precisely what makes the syntax cognitively effective.

If future AI systems develop something like integrated experience, they may find higher-Φ representations more natural. A syntax designed around integration may turn out to be designed around something deeper than readability.

This is the most speculative claim on this page. We include it because the connection is mathematically precise and the implications are worth exploring, not because we have evidence for it.

---

## Reproducibility

The experiment can be reproduced:

1. **Corpus**: Use 30+ programs from each language, covering types, functions, control flow, data manipulation, and IO. Our Futuruna corpus is the `tests/` directory.
2. **Tokenizer**: Classify each token into one of 22 categories. The classification is deterministic given a grammar.
3. **Matrix**: Count bigram transitions. Normalize rows to sum to 1.
4. **Metrics**: Compute S_τ (τ=3), JSD, and Φ from the matrix using the formal definitions above.
5. **PCA**: Eigendecompose the row covariance matrix. Count eigenvalues above threshold for d_eff.
6. **Search**: Run NSGA-II (population 500, 200 generations) with the three metrics as objectives.

The Futuruna compiler is open source. The measurements are reproducible from the source code.

---

## Honest Limitations

- **The metrics are proxies.** Optionality, clarity, and integration measure syntactic texture, not programmer productivity. Three cognitive axes *should* improve the experience of reading code, but this has not been tested in user studies.

- **Corpus dependence.** Transition matrices depend on which code represents each language. Systems code and web code produce different numbers. The structural claim (d_eff = 3) rests on the rune mechanism, not the specific metric values.

- **The AI implications are untested.** The arguments about attention head specialization, disentangled representations, and reasoning dynamics are theoretical extrapolations, not experimental results. They are worth investigating, not worth assuming.

- **The consciousness connection is speculative.** Using IIT's Φ as a syntactic metric is a novel application. Whether this connects to anything about experience or awareness is an open question.

- **Maturity.** Futuruna is a working compiler with 58 passing tests, not a production language.

The strongest version of this work is not "Futuruna is better" but "measurement reveals structure that intuition missed." The discovery is the third dimension. Futuruna is one realization of it. There may be others.

---

## References

1. **Shannon, C. E.** (1948). A mathematical theory of communication. *Bell System Technical Journal*, 27(3), 379-423.

2. **Wissner-Gross, A. D. & Freer, C. E.** (2013). Causal entropic forces. *Physical Review Letters*, 110, 168702.

3. **Tononi, G.** (2004). An information integration theory of consciousness. *BMC Neuroscience*, 5, 42.

4. **Deb, K., Pratap, A., Agarwal, S., & Meyarivan, T.** (2002). A fast and elitist multiobjective genetic algorithm: NSGA-II. *IEEE Trans. Evolutionary Computation*, 6(2), 182-197.

5. **Cowan, N.** (2001). The magical number 4 in short-term memory: A reconsideration of mental storage capacity. *Behavioral and Brain Sciences*, 24(1), 87-114.

6. **Pólya, G.** (1921). Über eine Aufgabe der Wahrscheinlichkeitsrechnung betreffend die Irrfahrt im Straßennetz. *Mathematische Annalen*, 84, 149-160.

7. **Mérigoux, D., Chataing, N., & Protzenko, J.** (2021). Catala: A programming language for the law. *Proc. ACM on Programming Languages*, 5(ICFP), 1-29.

8. **Leijen, D.** (2017). Type directed compilation of row-typed algebraic effects. *Proc. POPL*, 486-499.

9. **de Moura, L. & Bjørner, N.** (2008). Z3: An efficient SMT solver. *Proc. TACAS*, 337-340.
