# Futuruna Syntax Research

The research that derived Futuruna's syntax from measurement — computational
experiments, data, and theoretical analysis.

## Computational Experiments

Three standalone Rust programs compute the information-theoretic metrics
reported in the paper (`paper/paper-futuruna.tex`) and discover the
Pareto-optimal syntactic designs that led to Futuruna's seven-rune structure.

### `syntax_pareto.rs` (1,249 lines)
**NSGA-II evolutionary search over the 3D Pareto frontier.**

Optimizes S_τ (optionality) × JSD (clarity) × Φ (integration) simultaneously.
Starting from English, Lojban, and Chinese transition matrices, mutates and
recombines syntactic structures across 200 generations. Discovers that 85/122
Pareto-optimal designs share the START→OP transition (statement runes).

### `syntax_frontier2.rs` (869 lines)
**Deep analysis of the frontier.**

- Phase transition mapping: obligation level vs d_eff
- Interpolation paths between natural languages
- Programming language evaluation (Rust, Haskell, Prolog, Python, Scala, Kotlin, Lisp, C)
- Robustness analysis under random perturbation
- Equality frontier (mean vs min S_τ, Gini coefficient)

### `syntax_stau.rs` (754 lines)
**S_τ evaluation and novel syntax discovery.**

- Computes S_τ on POS transition graphs for real English (SVO)
- Compares synthetic word orders (SOV, VSO, V2, ergative, polysynthetic, free)
- 5D syntactic dimension profiles
- Novel syntax discovery via edge perturbation optimization
- Eigenstate decomposition with d_eff/Φ computation

### Running

These are standalone Rust files with no external dependencies (only `std`).

```bash
cd research
rustc -O syntax_pareto.rs -o syntax_pareto && ./syntax_pareto
rustc -O syntax_frontier2.rs -o syntax_frontier2 && ./syntax_frontier2
rustc -O syntax_stau.rs -o syntax_stau && ./syntax_stau
```

## Theory

### [`llm-prefix-disambiguation.md`](llm-prefix-disambiguation.md)
**Why Futuruna is optimal for autoregressive token processors.**

Theoretical analysis of rune-prefix disambiguation and LLM code processing.
Every line is disambiguated at token position 1 — `H(S|t_1) = 0 bits` —
freeing model capacity for semantics rather than syntactic hypothesis tracking.
Connects S_τ optimization to Shannon channel capacity: the same property
that maximizes optionality for humans also minimizes prediction cost for
machines. Includes four testable predictions and experimental designs.

## Data

The `data/` directory contains POS (part-of-speech) bigram transition counts:

| File | Source |
|------|--------|
| `pos_tags.tsv` | 22 POS categories (the token transition matrix dimensions) |
| `pos_bigrams.tsv` | English bigram counts (Brown corpus) |
| `pos_bigrams_lojban.tsv` | Lojban bigram counts |
| `pos_bigrams_chinese.tsv` | Chinese bigram counts |

## Key Results

The paper's Table 2 and Table 3 numbers come from `syntax_frontier2.rs`.
The NSGA-II claim (85/122 frontier members require START→OP) comes from `syntax_pareto.rs`.
The three eigenvalues (λ: 2.28, 1.66, 0.96) come from `syntax_pareto.rs`.
