# Rune-Prefix Disambiguation and LLM Token Processing

**Thesis:** Futuruna's rune-prefixed syntax minimizes the conditional entropy
of code for autoregressive language models, making it theoretically the
easiest programming language to learn, predict, and generate.

## 1. The Problem with Existing Syntax

Autoregressive LLMs predict tokens left-to-right: `P(t_i | t_1, ..., t_{i-1})`.
When reading source code, the model must resolve *what kind of statement* it is
processing before it can accurately predict what follows. In most languages,
this disambiguation is deferred across many tokens.

### Disambiguation delay by language

| Language | Example line                              | Tokens before statement kind is known |
|----------|-------------------------------------------|---------------------------------------|
| Python   | `class Foo(Bar):`                         | 1 (`class`)                           |
| Python   | `x = await foo(bar)`                      | 3 (`x`, `=`, `await`)                |
| Rust     | `pub async fn foo<T: Bar>(x: T) -> R`    | 3+ (`pub`, `async`, `fn`)            |
| Rust     | `let mut x: Vec<i32> = vec![1, 2, 3];`   | 2 (`let`, `mut`)                     |
| Haskell  | `instance Monad Maybe where`              | 1 (`instance`)                        |
| Haskell  | `f x y = case x of ...`                  | 3+ (function vs binding ambiguous)    |
| C++      | `template<typename T> static const ...`   | 4+ (cascading specifiers)             |
| **Futuruna** | `> foo(x: Int) -> Int`               | **1** (the `>` rune)                  |
| **Futuruna** | `# Point { x: Float, y: Float }`     | **1** (the `#` rune)                  |
| **Futuruna** | `~ temp = sensor |> map(to_c)`      | **1** (the `~` rune)                  |

In Futuruna, **every line is disambiguated at token position 1**. The rune
partitions the entire grammar into 7 disjoint syntactic modes before the model
reads anything else.

## 2. Information-Theoretic Framing

### 2.1 Conditional Entropy Reduction

Let `S` be the random variable for statement kind (type decl, function def,
binding, stream, rule, meta, verification) and let `t_1` be the first token
of a line.

For Futuruna:
```
H(S | t_1) = 0 bits
```

The rune is a deterministic function of statement kind. There is zero residual
uncertainty about what grammatical mode the line operates in.

For Python, `t_1` could be an identifier (assignment? function call? type
annotation?), a keyword (`def`, `class`, `if`, `for`, `async`, `with`, ...),
or a decorator (`@`). Many identifiers are ambiguous:
```
H(S | t_1) > 0 bits  (often significantly so)
```

### 2.2 Freed Capacity

A transformer has finite representational capacity per layer. Capacity spent
on syntactic disambiguation is capacity *not* spent on semantic understanding.

If the model can determine at token 1 that `>` means "function definition,"
all subsequent attention heads can focus on:
- Parameter names and types (semantics)
- Return type (contract)
- Relationship to other functions (architecture)

Rather than simultaneously maintaining hypotheses about whether the line is
an assignment, a macro invocation, a trait impl, etc.

### 2.3 Connection to d_eff

Futuruna's measured d_eff = 3 means its three cognitive axes (rune / type flow /
block structure) are **independent**. For an LLM, independence means each axis
provides non-redundant information. The rune tells you *what* (axis 1), the
type arrow tells you *how it connects* (axis 2), and the braces tell you
*where it nests* (axis 3). No axis is predictable from the others.

In languages with d_eff = 1, these axes collapse: knowing the keyword often
predicts the block structure and even the type flow. The model receives the
same information three times in different clothing, wasting token budget.

## 3. Additional Structural Advantages

### 3.1 Left-Margin Architecture Map

In a Futuruna file, reading only column 1 of every line produces a complete
structural skeleton of the program:

```
#  ← type
#  ← type
>  ← function
>  ← function
=  ← binding
~  ← stream
|  ← rule
?  ← verification
```

This is an implicit table of contents. An LLM (or a human skimming code)
can build a mental model of the program's architecture from the left margin
alone — where the types live, where the logic is, where the reactive
topology is defined, what invariants are checked.

No other language offers this. In Python, the left margin is indentation
(nesting depth, not statement kind). In Rust, it's a mix of `pub`, `fn`,
`let`, `struct`, `impl`, `use`, `mod`, `type`, `const`, `static`, `trait`,
`enum`, `match`, `if`, `for`, `while`, `loop`, `return`, `async`, `unsafe`
— 20+ keywords competing for the same position with no categorical grouping.

### 3.2 Token Efficiency

The rune replaces multi-token keyword sequences with a single character:

| Traditional syntax | Tokens | Futuruna | Tokens |
|---|---|---|---|
| `public static void` | 3 | `>` | 1 |
| `async def` | 2 | `>` | 1 |
| `data class` | 2 | `#` | 1 |
| `sealed interface` | 2 | `#` | 1 |
| `let mut` | 2 | `=` | 1 |
| `assert!` / `debug_assert!` | 1-2 | `?` | 1 |

Fewer tokens per statement means more program fits in a fixed context window.
For LLMs with 8K-128K token limits, this is a direct multiplier on the amount
of code the model can reason about simultaneously.

### 3.3 Grammar-Constrained Decoding

Modern LLM tooling (Guidance, Outlines, LMQL) supports grammar-constrained
generation — restricting the model's output to syntactically valid programs.

For Futuruna, the top-level grammar constraint is trivial:

```
line ::= '#' type_decl
       | '>' func_decl
       | '|' rule_or_match
       | '=' binding
       | '~' stream_decl
       | '@' meta_effect
       | '?' verification
```

Seven production rules at the top level. The constraint automaton starts
with a 7-way branch on a single token. For traditional languages, the
top-level grammar has dozens of overlapping productions that require
multi-token lookahead to disambiguate.

This makes Futuruna uniquely suited to constrained code generation.

### 3.4 Language-Independent Structure

The runes are symbols (`#`, `>`, `|`, `=`, `~`, `@`, `?`), not English words.
A model does not need to know what "function," "class," or "let" mean in
English to parse Futuruna's structure. The categorical signal is carried by
single Unicode characters that have no natural-language polysemy.

This has two implications:
- Models pre-trained on multilingual data can leverage Futuruna structure
  without English bias
- The rune system transfers across human languages — a Japanese, Arabic,
  or Swahili speaker reads the same structural markers as an English speaker

### 3.5 Semantic Chunking for Retrieval

When building RAG (retrieval-augmented generation) systems over codebases,
the standard problem is: how do you chunk source code into semantically
meaningful units? Most approaches require AST parsing or heuristic splitting.

In Futuruna, every rune-prefixed block is a self-contained semantic unit.
Chunking is a single regex: split on lines matching `^[#>|=~@?]` at the
top indentation level. No parser needed. Each chunk carries its category
in its first byte.

### 3.6 Miller's 7 +/- 2

The seven runes are not an arbitrary count. George Miller's (1956) famous
observation that human working memory holds 7 +/- 2 chunks means that
a programmer can hold the *entire rune vocabulary* in working memory
simultaneously. There is no rune they need to look up. The full categorical
structure of the language is always mentally available.

Most programming languages have 30-80 keywords. No human holds all of
C++'s 97 keywords in working memory. The excess must be offloaded to
documentation, IDE tooltips, or the LLM's parametric memory.

Seven runes is the sweet spot: enough to partition all of programming
(types, functions, rules, bindings, streams, meta, verification) while
remaining within the cognitive capacity of a single human mind.

## 4. Predictions

### 4.1 Few-Shot Learning

The rune system should produce the largest advantage in **few-shot** and
**low-data** regimes. Why:

- With 7 runes partitioning all syntax, a model needs approximately 7 examples
  (one per rune) to learn the grammar's top-level structure
- Traditional languages require hundreds of examples to statistically
  disambiguate overlapping keyword/identifier patterns
- The rune acts as an explicit **grammar mode tag** that other languages
  encode implicitly through context

**Prediction 1:** An LLM given N examples of Futuruna code will achieve
higher code completion accuracy than the same model given N examples of
Python/Rust/Haskell, for small N (N < 50).

### 4.2 Generation Accuracy

Since each line begins with a rune that constrains all subsequent tokens,
the search space for code generation is partitioned early:

- After generating `#`, the model only needs to consider type-declaration syntax
- After generating `>`, only function-definition syntax
- After generating `~`, only reactive-stream syntax

**Prediction 2:** Futuruna code generation will exhibit lower error rates
per line than equivalent programs in traditional languages, because each
line's first token eliminates 6/7 of the grammatical search space.

### 4.3 Fine-Tuning Efficiency

**Prediction 3:** Fine-tuning an LLM on Futuruna will require fewer
training tokens to reach a given perplexity target than fine-tuning on
a comparably-sized corpus of Rust or Python.

The argument: each Futuruna token carries more syntactic information
(higher mutual information between adjacent tokens due to rune constraints),
so each training example teaches the model more about the language's structure.

### 4.4 Error Localization

When an LLM generates incorrect code, the rune prefix makes errors easier
to detect and localize:

- A misplaced `#` (type where a function should be) is visible at token 1
- In Python, a wrong statement kind might only become apparent 10+ tokens in

**Prediction 4:** Both LLMs and human reviewers will detect structural
errors in Futuruna faster than in traditional languages.

## 5. The S_tau Connection

This is not a coincidence. Futuruna's syntax was optimized for S_tau (causal
entropic force): maximum freedom of future action from each token state.

High S_tau means: from any given token, there are many distinct valid
continuations, but they are **structured** — the rune channels them into
coherent grammatical modes rather than chaotic ambiguity.

For an autoregressive model, this is ideal:
- High entropy in the *language* (many valid programs can be written)
- Low entropy in the *conditional prediction* (given the rune, the next
  tokens are highly constrained)

This is precisely the signature of a well-designed communication channel
(Shannon 1948): maximize information rate while minimizing decoding error.
Futuruna's syntax, derived from S_tau optimization, accidentally (or
inevitably) optimizes the same quantity that makes a language easy for
any sequential token processor — biological or artificial.

## 6. Experimental Design (Future Work)

To validate these predictions empirically:

### Experiment A: Few-Shot Completion
- Take a base LLM (no Futuruna in training data)
- Provide N examples of {Futuruna, Python, Rust, Haskell} (N = 1, 5, 10, 25, 50)
- Measure code completion accuracy on held-out test cases
- Metric: exact-match accuracy, token-level perplexity

### Experiment B: Fine-Tuning Curve
- Fine-tune the same base model on equal-sized corpora of each language
- Plot perplexity vs training tokens
- Measure tokens-to-threshold for each language

### Experiment C: Generation Correctness
- Prompt a model to generate complete programs from specifications
- Compare compile/run success rate across languages
- Control for program complexity (AST node count)

### Experiment D: Error Detection
- Inject structural errors into generated code
- Measure time-to-detection for LLM-based and human reviewers
- Compare across languages

## 7. Broader Implications

If validated, these results would suggest that **programming language design
has been leaving information-theoretic efficiency on the table**. Languages
evolved from human traditions (C's heritage, ML's heritage, Lisp's heritage)
without optimizing for the token-sequential processing that both LLMs and
human working memory perform.

Futuruna's approach — deriving syntax from measurement rather than tradition —
may represent a new design methodology: **languages optimized for the
information-processing architecture of their readers**, whether human or
machine.

The seven runes are, in this framing, not just a syntactic convention.
They are an **optimal prefix code** for programming language statement kinds:
7 symbols, each a single token, each deterministically identifying one of
7 grammatical modes. No existing language achieves this.

## References

- Shannon, C. E. (1948). A Mathematical Theory of Communication.
- Miller, G. A. (1956). The Magical Number Seven, Plus or Minus Two.
- Tononi, G. (2004). An Information Integration Theory of Consciousness.
- Wissner-Gross, A. D., & Freer, C. E. (2013). Causal Entropic Forces.
- Vaswani, A., et al. (2017). Attention Is All You Need.
- Chen, M., et al. (2021). Evaluating Large Language Models Trained on Code. (Codex)
- Austin, J., et al. (2021). Program Synthesis with Large Language Models.
