# Futuruna

**The first programming language designed from consciousness theory and entropy theory.**

File extension: `.runa` | Compiler: `runa` | Website: [futuruna.com](https://futuruna.com)

## Current State

Futuruna has a strong and tested core, but the whole language is not yet a
single production-ready surface. Core syntax, documented stdlib behavior, core
CLI commands, and pure interpreter-vs-compiled parity are the strongest
surfaces today. Stateful/reactive workflows, storage, proof-backed compiler
checking, WASM artifacts, Rust interop, and
experimental tooling are still preview or research-grade.

The current production-readiness table lives in
[docs/production-readiness-scorecard.md](docs/production-readiness-scorecard.md).
It tracks each area, state, next milestones, effort, and impact. The short
version is: keep the core mint, then promote preview surfaces one gate at a
time with explicit skip accounting and canary/downstream evidence.

## Why Futuruna Exists

Every programming language before Futuruna was designed by tradition and taste. Futuruna was designed by **measurement**.

Using Integrated Information Theory (IIT) and Shannon entropy analysis on token transition graphs, we discovered that optimal programming language syntax requires **three independent cognitive axes** (d_eff=3). No existing language achieves this - Rust, Kotlin, and Scala collapse to d_eff=1; Haskell and Prolog reach d_eff=2.

The key innovation that unlocks d_eff=3: **statement runes** - starting every statement with an operator instead of a keyword. This creates syntactically orthogonal pathways that the human mind processes on independent channels.

### The Theory

- **IIT (Integrated Information Theory)**: Phi measures how much a system is more than the sum of its parts. Futuruna's syntax maximizes Phi across its token pathways.
- **S_tau (Causal Entropic Forces)**: Measures freedom of future action. An NSGA-II multi-objective search over syntactic designs optimized for S_tau, JSD, and Phi simultaneously.
- **Shannon Entropy**: The token transition matrix is analyzed for information-theoretic properties - high entropy means the syntax carries maximum information per token.

Futuruna sits on the Pareto frontier that no existing language reaches.

## The Seven Runes

Every statement begins with one of seven runes - a closed partition of what a program can say:

| Rune | Meaning | What it does |
|-------|---------|-------------|
| `#` | What exists | Types, effects, traits, impls |
| `>` | What happens | Functions, actors, modules |
| `\|` | What should be true | Rules, match arms, handlers |
| `=` | What is | Bindings, ground truth |
| `~` | What flows | Reactive streams, temporal behavior |
| `@` | Where proofs stop | Meta/effects: print, use, import, comptime |
| `?` | Prove it | Solver/verification invocation |

## The Three Axes

The eigenspace decomposition reveals three independent cognitive channels:

1. **Axis 1 - Statement Kind**: which rune starts the line creates an independent dimension
2. **Axis 2 - Type Flow**: TYPE -> ARROW chains (Haskell/ML-style type signatures)
3. **Axis 3 - Block Composition**: brace nesting creates structural depth

## Quick Example

```
-- A reactive weather advisor using all 7 runes

# Condition = Sunny | Cloudy | Rainy | Stormy       -- # what exists
# Weather(temp: Float, condition: Condition)

> advise(w: Weather) -> String {                     -- > what happens
    match w.condition {
        | Sunny -> "Perfect day for a bike ride"     -- | what should be true
        | Rainy -> "Bring an umbrella"
        | Stormy -> "Stay indoors"
    }
}

= forecast = Weather(temp: 22.0, condition: Sunny)  -- = what is

~ readings = from_list([forecast])                   -- ~ what flows
    |> map(|w| advise(w))

@ print(show(readings))                              -- @ where proofs stop

? forecast.temp > -50.0 and forecast.temp < 60.0     -- ? prove it
```

## Quick Start

```bash
cargo build --release
./target/release/runa run examples/weather_demo.runa
```

### Commands

```bash
runa run program.runa       # Compile + execute
runa emit program.runa      # Show generated Rust
runa build program.runa     # Compile to native binary
runa wasm program.runa      # Compile to WebAssembly (via wasm-pack)
runa check program.runa     # Parse + type-check (fast feedback)
runa verify program.runa    # Verify invariants via Z3
runa hashes program.runa    # Show content-addressed hashes
runa test                   # Run all tests/*.runa (interpreted)
runa test --run             # Run all tests/*.runa (compiled)
runa test --roundtrip tests # Compare interpreter vs compiled output
runa expect tests/expect    # Run compiletest-style compiler expectations
runa stress-gen 100 --seed 42 --save-failures /tmp/futuruna-diff
./scripts/mint.sh           # Canonical "is Futuruna mint?" gate
./scripts/canary.sh         # Authored multi-feature canary programs
./scripts/expectations.sh   # Narrow diagnostics/run-fail/phase expectations
./scripts/differential.sh   # Reproducible differential and generative lane
```

See [docs/state-and-roadmap.md](docs/state-and-roadmap.md) for the current high-level map, [docs/production-readiness-scorecard.md](docs/production-readiness-scorecard.md) for the readiness table, [docs/mint-gate.md](docs/mint-gate.md) for the exact mint contract, [docs/canary-suite.md](docs/canary-suite.md) for the curated canary lane, [docs/canary-matrix.md](docs/canary-matrix.md) for the authored coverage map, [docs/expectation-suites.md](docs/expectation-suites.md) for compiletest-style expectations, [docs/artifact-codegen-contracts.md](docs/artifact-codegen-contracts.md) for emitted Rust and artifact boundaries, and [docs/differential-testing.md](docs/differential-testing.md) for the deeper proactive lane.

Current stage visibility lives in [docs/feature-stages.md](docs/feature-stages.md): core syntax and documented stdlib behavior are treated as stable, while `runa verify`, stateful/reactive surfaces, Rust interop, and several advanced tooling paths are still preview or experimental.

Release-facing compatibility history lives in
[docs/compatibility-guides/](docs/compatibility-guides/). Stable breaks,
deprecations, and bug-fix exceptions should show up there rather than only in
PR descriptions.

## What Futuruna Unifies

| Capability | Prolog | Rust | Haskell | Catala | Futuruna |
|-----------|--------|------|---------|--------|----------|
| Logic programming | Yes | No | No | No | Yes |
| Typed functions | No | Yes | Yes | Partial | Yes |
| Pattern matching | Yes | Yes | Yes | Partial | Yes |
| Algebraic effects | No | No | No | No | Yes |
| Default logic (law) | No | No | No | Yes | Yes |
| Reactive streams | No | No | No | No | Yes |
| Zero-cost ownership | No | Yes | No | No | Yes |
| Content-addressed code | No | No | No | No | Yes |
| d_eff | 2 | 1 | 2 | 2 | **3** |

## Architecture

The compiler is a ~10,000-line single-file Rust program (`src/bin/runa.rs`) containing lexer, parser, interpreter, and Rust transpiler. Futuruna programs transpile to Rust, inheriting its entire ecosystem (any crate via `@ use`) and zero-cost abstractions (no GC, deterministic drops).

The ownership model follows the Kotlin-to-Java philosophy: the programmer writes value semantics, the compiler emits ownership-correct Rust. Escape analysis, borrow inference, and independence analysis eliminate ~95% of manual ownership annotations.

## Examples

- `examples/weather_demo.runa` - Reactive weather station using all 7 runes
- `examples/danish-constitution/` - The entire Danish Constitution encoded in Futuruna
- `tests/` - 30+ test programs covering every language feature

## VS Code / Cursor Extension

See `editors/vscode/` for syntax highlighting and the Futuruna Axes color theme.

## Documentation

- `CONTRIBUTING.md` - Contributor ratchet for semantic/compiler changes
- `docs/compatibility-policy.md` - Compatibility categories, feature stages, and bug-fix exception policy
- `docs/artifact-codegen-contracts.md` - Emitted Rust, native artifact, `runa lib`, and WASM stability boundaries
- `docs/feature-stages.md` - Current stable/preview/experimental stage matrix for major surfaces and commands
- `docs/production-readiness-scorecard.md` - Evidence-backed readiness ratings and promotion plan
- `docs/expectation-suites.md` - Compiletest-style diagnostics, run/fail, and phase expectation lane
- `docs/compatibility-guides/` - Release-facing compatibility ledger for stable changes, deprecations, and bug-fix exceptions
- `docs/state-and-roadmap.md` - Where Futuruna stands now and the next three milestones
- `docs/language-sketch.md` - Language design and three-axis analysis
- `docs/differential-testing.md` - Reproducible differential and generative testing
- `docs/canary-suite.md` - Authored canaries for realistic user-shaped workflows
- `docs/canary-matrix.md` - Tiered authored coverage map and planned build-out
- `docs/mint-gate.md` - Canonical verification gate for keeping Futuruna mint
- `docs/stream-lifetimes.md` - Explicit lifetime contract for live stream subscriptions
- `docs/verified-bootstrap.md` - Current proof bootstrap claim and trust boundary
- `docs/milestones.md` - Compiler milestones (M1-M15)
- `docs/ownership-design.md` - Memory model (Kotlin-to-Rust philosophy)
- `docs/reactive-design.md` - Reactive streams as native graph topology
- `paper/paper-futuruna.tex` - Academic paper

## License

See LICENSE file.
