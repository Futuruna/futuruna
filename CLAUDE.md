# Futuruna

A programming language designed from consciousness theory and entropy theory.
Syntax derived from measurement, not tradition. Transpiles to Rust.

## Quick Start

```bash
cargo build --release
./target/release/runa run examples/weather_demo.runa    # Interpret
./target/release/runa emit program.runa                 # Show generated Rust
./target/release/runa build program.runa                # Compile to native binary
./target/release/runa check program.runa                # Parse + type-check (fast)
```

## Architecture

**Single-file compiler**: `src/bin/runa.rs` (~12,200 lines of Rust)
- Lexer, parser, interpreter, type checker, Rust transpiler, and Rust→Futuruna transpiler
- 83 tests in `tests/`, all passing. 65/65 codegen, 49/49 roundtrip, 22/22 from-rust
- Bootstrapping: Rust hosts the compiler, Futuruna transpiles back to Rust

---

## The Seven Runes — with Idiomatic Examples

Every line starts with a rune. The rune classifies what the statement *is*.

### `#` — What exists (types, effects, traits, impls)

```runa
-- Struct: named fields, positional construction
# Weather(city: String, temp: Float, condition: Condition)

-- Enum: algebraic data type with variants
# Condition = Sunny | Cloudy | Rainy | Stormy

-- Recursive ADT (auto-boxed, uses Rc for structural sharing)
# Expr = Lit(Int) | Add(Expr, Expr) | Var(String)

-- Generic type
# Option(a) = None | Some(a)

-- ADT with methods (first param receives the type)
# Color = Red | Green | Blue {
    > name(c) -> String {
        match c { | Red -> "red" | Green -> "green" | Blue -> "blue" }
    }
}

-- Effect declaration
# effect Console {
    > say(msg: String) -> ()
    > ask(prompt: String) -> String
}

-- Subset types: pick variants from existing types
# GeoArea = Sweden | Danmark | Norway | Færøerne | Grønland
# Skandinavien = GeoArea.Sweden | GeoArea.Danmark | GeoArea.Norway
# Rigsdel = GeoArea.Danmark | GeoArea.Færøerne | GeoArea.Grønland

-- Type inclusion: bare type name = all variants
# AlleReligioner = Kristendom | Islam | Jødedom

-- EXCEPT: set subtraction
# AnerkendteKristne = Kristendom EXCEPT Sekterianisme

-- Type-constrained rules: type IS the law
| grundloven_gælder_for(del: Rigsdel) -> true
-- grundloven_gælder_for(Danmark) → true
-- grundloven_gælder_for(Sweden) → false

-- Trait + impl
# trait Printable { > display(self) -> String }
# impl Printable for Color {
    > color_display(c) -> String { match c { | Red -> "Red" | _ -> "Other" } }
}
```

Fields accessed with dot notation: `w.temp`, `w.condition`.

### `>` — What happens (functions, actors, modules)

```runa
-- Pure function
> add(a: Int, b: Int) -> Int { a + b }

-- Higher-order function
> apply_twice(f: Int -> Int, x: Int) -> Int { f(f(x)) }

-- Generic function (lowercase = type variable)
> map_list(xs: List(a), f: a -> b) -> List(b) {
    match xs {
        | Nil -> Nil
        | Cons(h, t) -> Cons(f(h), map_list(t, f))
    }
}

-- Actor (concurrent state machine via tokio channels)
> actor counter(state: Int) {
    | Increment -> state + 1
    | Decrement -> state - 1
    | GetCount -> state             -- ask pattern returns current state
}

-- Module
> module Math {
    > square(x: Int) -> Int { x * x }
}
```

Ownership is invisible: `&T` and `.clone()` are inferred by escape analysis. You never write lifetimes.

### `|` — What must be true (rules, invariants, handlers, scopes, match arms, facts)

The most versatile rune. Unifies logic programming, default logic, pattern matching, effect handling, and verification.

```runa
-- Prolog-style facts (Datalog)
| parent("alice", "bob")
| parent("bob", "charlie")

-- Value-returning fact
| capital("Denmark") -> "Copenhagen"

-- Rule with conjunction using 'and' keyword
| ancestor(a, b) -> parent(a, b)
| ancestor(a, b) -> parent(a, mid) and ancestor(mid, b)

-- Disjunction using 'or' keyword
| is_drink_base(x) -> is_citrus(x) or is_spirit(x)

-- Negation + wildcard
| childless(x) -> parent(x, _) and not(parent(_, x))

-- Constructor matching in facts (enum variants as ground terms)
# Color = Red | Green | Blue
| is_warm(Red)
-- is_warm(Red) → true, is_warm(Blue) → false

-- Type-constrained rules
| grundloven_gælder_for(del: Rigsdel) -> true
-- findall(del, grundloven_gælder_for(del)) → [Danmark, Færøerne, Grønland]

-- search: first-match query (returns Option)
= first_child = search(c, parent("alice", c))

-- Catala-style default logic with exceptions
| advisory(w) -> "all clear"
| advisory(w) -> "heat warning" under w.temp > 35.0
| exception heatwave advisory(w) -> "danger" under w.condition == Stormy

-- Named invariant (verification target)
= balance = 1000
| balance_ok: balance -> balance >= 0 && balance <= 1000000

-- Effect handler
= result = | handle Console {
    | say(msg) -> { @ print("[console] " + msg); resume(()) }
    | ask(prompt) -> resume("default")
} in greet("World")

-- Scope (lifecycle management for streams)
| scope WeatherStation {
    ~ readings = subject()
    readings <- Weather(Copenhagen, 22.0, Sunny)
    @ print(show(readings.latest))
}
```

Inside `match`, `|` introduces each arm:
```runa
match shape {
    | Circle(r) -> 3.14 * r * r
    | Rectangle(w, h) -> w * h
}
```

### `=` — What is (bindings, ground truth)

```runa
-- Simple binding
= x = 42
= name = "hello"
= result = add(20, 22)

-- With type annotation
= x: Int = 42

-- Monadic bind (early return on Err/None, like Rust's ?)
= value <- parse_int("42")

-- Practical: chain fallible operations
> add_parsed(a: String, b: String) -> Result(Int, String) {
    = a <- parse_int(a)
    = b <- parse_int(b)
    Ok(a + b)
}
```

Rebinding is idiomatic (shadow the name):
```runa
= req = Request("GET", "", "")
= req = with_method(req, "POST")
= req = with_path(req, "/api")
```

While loops (compiles to real Rust `while`, no stack overhead):
```runa
= n = 10
while n > 1 {
    if n % 2 == 0 { = n = n / 2 } else { = n = 3 * n + 1 }
}
```

### `~` — What flows (reactive streams, subjects, temporal behavior)

```runa
-- Create streams from data
~ nums = from_list([1, 2, 3, 4, 5])

-- Pipe operators compose stream transformations
~ big = nums |> filter(|x| x > 3) |> map(|x| x * 2)

-- Subject: push-based stream
~ clicks = subject()
clicks <- "click1"
clicks <- "click2"
@ print(show(clicks.count))       -- 2
@ print(show(clicks.latest))      -- "click2"

-- Subscription (~ + |) — use for streams, NOT for loops
~ nums | x -> { @ print(show(x)) }

-- Full lifecycle handling
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
    | Err(e) -> { log(e) }
    | Complete -> { @ print("done") }
```

Stream operators: `map`, `filter`, `scan`, `take`, `skip`, `tap`, `merge`, `flat_map`, `take_while`, `drop_while`, `debounce`, `throttle`, `delay`, `buffer`, `timeout`, `switch_map`, `sample`, `first`, `reduce`, `start_with`, `concat`, `pairwise`.

### `@` — Where proofs stop (IO, imports, meta, effects)

```runa
-- Output
@ print("hello")

-- Import (multi-file)
@ import ./utils                      -- flat merge
@ import Utils from ./utils           -- qualified: Utils.function()

-- Cargo dependencies
@ depend "serde" "1"
@ depend "tokio" "{ version = \"1\", features = [\"full\"] }"

-- Export (make next definition public)
@ export
> public_api() -> Int { 42 }

-- Compile-time evaluation
@ comptime = table = generate_lookup(1000)

-- Language mode (Danish identifiers)
@ sprog da

-- Rust escape hatch (for FFI, performance, the 1-5%)
@ rust {
    fn fast_sort(x: &mut [f64]) {
        x.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    }
}

-- File I/O
@ write_file("out.txt", content)
= data = read_file("in.txt")
```

### `?` — Prove it (verification demands)

```runa
-- Define invariant with |, check with ?
| balance_ok: balance -> balance >= 0 && balance <= max_supply

-- Six forms:
? balance_ok                                          -- bare (halt on fail)
? balance_ok -> { @ print("OK") }                    -- pass block
? balance_ok: val -> { @ print(show(val)) }           -- capture subject
? balance_ok else { @ print("FAIL") }                 -- else (no halt)
? balance_ok -> { @ print("OK") } else { @ print("FAIL") }  -- both
? balance_ok: val -> { @ print(show(val)) } else { @ print("violation") }  -- full

-- Check ALL invariants at once
? all -> { @ print("All OK") } else { @ print("Some failed") }
```

Three assurance levels from the same `?` line:
- `runa run` — evaluates predicate at runtime
- `runa build` — emits `debug_assert!()` in compiled binary
- `runa verify` — translates to SMT-LIB2, invokes Z3 to prove for all inputs

---

## All Commands

```bash
runa run file.runa           # Interpret directly
runa emit file.runa          # Show generated Rust code
runa build file.runa         # Compile to native binary
runa wasm file.runa          # Compile to WebAssembly
runa check file.runa         # Parse + type-check (fast, ~0.1s)
runa verify file.runa        # Prove invariants via Z3
runa audit file.runa         # Discover gaps, tensions, asymmetries automatically
runa hashes file.runa        # Content-addressed hashes
runa registry file.runa      # Generate name→hash registry
runa fmt file.runa           # Format source file
runa fmt .                   # Format all .runa files in directory
runa fmt --check file.runa   # Check formatting (CI mode, exit 1 if unformatted)
runa test                    # Run all tests/*.runa (interpreted)
runa test --run              # Run all tests (compiled, clean output)
runa test path/to/dir        # Run .runa files in a specific directory
runa init my-project         # Create new project with runa.toml
runa add ../shared/lib       # Add local path dependency
runa add https://github.com/user/repo  # Add git dependency
runa lsp                     # Start language server (JSON-RPC over stdio)
runa lib file.runa           # Emit Rust library (no main)
runa bench                   # Run performance benchmarks
runa from-rust file.rs       # Transpile Rust → Futuruna
runa from-rust --verify f.rs # Transpile + run both + compare outputs
runa from-rust --test dir/   # Batch verify all .rs files (CI gate)
```

### `runa audit` — Automated Gap Discovery

Analyzes the topology of all `|` rules in a file to discover what you missed:
- **Symmetric pairs**: rules that follow the same pattern (e.g., samtykke functions)
- **Asymmetries**: where a pattern holds for A but not B
- **Tensions**: overlapping concepts with conflicting normative direction
- **Powers without enforcement**: authorities declared but never checked
- **Paradoxes**: contradictions the model reveals

Usage pattern for constitutional/legal modeling:
```bash
# Source files define types + facts + functions (no verification, no prints)
# Audit file imports everything, defines invariants, runs proofs
runa audit examples/danish-constitution/grundlov.audit.runa
runa run examples/danish-constitution/grundlov.audit.runa
```

---

## Idiomatic Patterns

### Logic programming (Datalog+)

```runa
-- Facts as ground truth
| have("brandy")
| have("lime")
| needs("daiquiri", "light rum")
| needs("daiquiri", "lime")
| needs("daiquiri", "sugar")

-- Query with findall + filter
= cocktails = distinct(findall(c, needs(c, _)))
for c in cocktails {
    = missing = filter(findall(i, needs(c, i)), |i| not(have(i)))
    if length(missing) == 0 { @ print(c) }
}
```

### Constitutional / legal modeling

```runa
@ sprog da    -- Danish identifiers with æøå

----
§ 3. Den lovgivende magt er hos kongen og folketinget i forening.
----

# Magtholder = Hos(institution: Institution) | IForening(a: Institution, b: Institution)

| lovgivende_magt() -> IForening(Kongen, Folketinget)
| udøvende_magt() -> Hos(Kongen)
| dømmende_magt() -> Hos(Domstolene)
```

### Stream processing pipeline

```runa
| scope DataPipeline {
    ~ readings = subject()
    for w in from_list(mock_feed()) { readings <- w }

    ~ alerts = readings |> map(|w| advise(w))
    ~ important = alerts |> filter(|a| a.severity != Mild)

    ~ important | a -> {
        @ print(a.message + " [" + show(a.severity) + "]")
    }
}
```

### Error handling (monadic bind)

```runa
> safe_divide(a: Int, b: Int) -> Result(Int, String) {
    if b == 0 { Err("division by zero") }
    else { Ok(a / b) }
}

> compute(x: String, y: String) -> Result(Int, String) {
    = a <- parse_int(x)
    = b <- parse_int(y)
    = c <- safe_divide(a, b)
    Ok(c * 2)
}
```

### Actor concurrency

```runa
> actor accumulator(total: Int) {
    | Add(n) -> total + n
    | Sub(n) -> total - n
    | Reset -> 0
}

= acc = spawn(accumulator, 0)
acc <- Add(10)
acc <- Add(20)
= result = ask(acc, Add(0))    -- 30
```

---

## Theoretical Foundations

Futuruna is the first programming language whose syntax was derived from measurement.

### Integrated Information Theory (IIT)
- **Phi** measures how much a system is "more than the sum of its parts"
- Applied to token transition graphs: Futuruna's syntax maximizes Phi
- **d_eff** (effective dimensionality) = 3: three independent cognitive axes
- Existing languages collapse to d_eff=1 (Rust, Kotlin) or d_eff=2 (Haskell, Prolog)

### Shannon Entropy / Causal Entropic Forces
- **S_tau** measures freedom of future action from a given state
- NSGA-II search over 22×22 token transition matrices optimized for S_tau, JSD, and Phi
- Pareto-optimal syntax: Futuruna sits on the frontier no existing language reaches

### The Three Cognitive Axes
1. **Axis 1 — Statement Kind** (λ=2.28): which rune starts the line
2. **Axis 2 — Type Flow** (λ=1.66): TYPE -> ARROW chains
3. **Axis 3 — Block Composition** (λ=0.96): brace nesting and depth

Three is the information-theoretic ceiling of what a 22×22 transition matrix can sustain. The NSGA-II search found 122 Pareto-optimal designs; 85 achieved d_eff≥3, all sharing statement-initial operators. None achieved d_eff=4.

---

## Research Spaces

### Core Theory
- `docs/research.md` — Full experiment: token classification, transition matrices, NSGA-II search, measurements, eigenvalue spectrum, AI implications
- `docs/why.md` — Accessible explanation: the traffic sign analogy, cockpit instruments, what d_eff feels like
- `paper/paper-futuruna.tex` — Academic paper (14 pages, compiles with pdflatex)

### Ownership Research
- `docs/research-ownership.md` — Invisible ownership: 76 adversarial patterns, the inference algorithm (escape analysis → auto-borrow → ref-match), bugs discovered, theoretical connection to S_tau
- `docs/ownership-limits.md` — Tested limits: Category A (handled natively), B (eliminated by design), C (requires `@ rust {}`)
- `docs/ownership-design.md` — Memory model (Kotlin-to-Rust philosophy)
- `docs/borrow-elimination.md` — Invisible ownership exploration

### Constitutional Modeling
- `examples/danish-constitution/` — 12 files: all 89 paragraphs of the Danish Constitution encoded as types, facts, functions, and invariants. Full audit with ~180 invariants, tension discovery (troskrav vs diskriminationsforbud, dødvande-scenariet, umyndig-med-ed paradox)
- `examples/us-constitution/` — 21 files: Articles I-VII + Succession Act, 50+ invariants, structural proofs (separation of powers, enumerated vs general powers, Three-Fifths ghost)
- `docs/reference/style.md` — Style guide for legal/constitutional modeling (model the source, don't narrate, absence is a fact)

### Cocktail Datalog
- `examples/cocktails.runa` — 24 cocktail recipes as Datalog facts, mixability query, shopping list, versatility ranking via `findall`/`filter`/`not`

### Real-World Showcase Programs
- `project-examples/log-analyzer/analyzer.runa` — Log analysis pipeline: 20 entries, stream processing (scan/pairwise/reduce), Catala alert rules with escalation, actors, JSON reports, 10 invariants. Runs to completion.
- `project-examples/inventory/inventory.runa` — Warehouse inventory system: SQLite persistence, Map-based stock tracking, Catala rules (reorder/hazmat/escalation/shelf-life), actors, stream analytics, 16 invariants, 12 simulated orders. Runs to completion.
- `project-examples/link-shortener/shortener.runa` — URL shortener REST API: HTTP server on :3000, spam detection, hash-based codes, actors, stream analytics, 8 invariants. Blocks on serve.
- `project-examples/task-tracker/tracker.runa` — Task tracker pressure test: SQLite, actors, streams, Catala rules, scopes, 20 invariants. Runs to completion.

### Persist (Database-from-Language — M26 In Progress)
- `docs/persist/research-persist.md` — "The database that falls out of the language": `@ persist Type` backs Datalog facts with SQLite, `assert`/`retract` for mutation, `| scope` as transaction boundary, `watch(Type)` for change streams, `@ migrate` for schema evolution, index inference from rule topology
- Key insight: Futuruna's existing runes already map to all database concepts — `#` types = tables, `|` facts = rows, `|` rules = queries, `?` invariants = constraints, `~` streams = triggers. Only three new things needed: `@ persist`, `assert`/`retract`, `watch`
- **Two persistence modes:** `@ store Type` (object store, struct→JSON blob, implemented) vs `@ persist Type` (fact store, struct→typed columns, not yet)
- **DB scoping:** `@ store T` → DB named from source file stem (`.weather.store.db`); `@ store T in "scope"` → explicit shared DB (`.scope.store.db`)
- Status: M26a (object store with assert/retract/scoped DB) implemented; M26b+ (findall, transactions, persist, watch) not yet

### Website
- `website/` — Dioxus WASM app with research hub
- Routes: `/` (home), `/playground`, `/docs`, `/why`, `/research` (index), `/research/optimization`, `/research/danish-constitution`, `/research/danish-constitution-audit`, `/research/us-constitution`, `/research/ownership`
- Features: syntax-highlighted code blocks with hover tooltips, live Futuruna examples, markdown rendering

### Self-Hosting
- `examples/lexer.runa` — Self-hosting lexer (~300 lines), tokenizes all 7 runes
- `examples/lexer.audit.runa` — Verification suite for the lexer
- Status: lexer done, parser is next

---

## Key Files

| Path | Purpose |
|------|---------|
| `src/bin/runa.rs` | The compiler (~12,200 lines: lexer, parser, interpreter, type checker, codegen) |
| `src/bin/runa_adversarial.rs` | Adversarial borrow checker tests |
| `std/std.runa` | Standard library |
| `docs/reference/` | Language reference: [basics](docs/reference/basics.md), [runes](docs/reference/runes.md), [stdlib](docs/reference/stdlib.md) (~70 builtins), [streams](docs/reference/streams.md), [rust-compat](docs/reference/rust-compatibility.md), [style](docs/reference/style.md) |
| `docs/research.md` | Full research: measurements, NSGA-II, eigenvalues, AI implications |
| `docs/why.md` | Accessible intro: what d_eff feels like |
| `docs/language-sketch.md` | Language design, three-axis analysis, rune rationale |
| `docs/milestones/` | Milestones M1–M40: main doc + per-milestone detail files |
| `docs/ownership-design.md` | Memory model (Kotlin-to-Rust philosophy) |
| `docs/research-ownership.md` | Invisible ownership: 76 adversarial patterns |
| `docs/ownership-limits.md` | Tested limits and honest boundaries |
| `docs/reactive-design.md` | Reactive streams as native graph topology |
| `docs/persist/` | Database-from-language research (persist, transactions, migrations) |
| `examples/weather_demo.runa` | Showcase: all 7 runes in one program |
| `examples/cocktails.runa` | Cocktail Datalog: 24 recipes + queries |
| `examples/danish-constitution/` | Danish Constitution (12 files, ~180 invariants) |
| `examples/us-constitution/` | US Constitution (21 files, 50+ invariants) |
| `examples/lexer.runa` | Self-hosting lexer |
| `project-examples/` | Showcase programs: log-analyzer, inventory, link-shortener, task-tracker |
| `tests/` | 68 test programs covering every feature |
| `paper/paper-futuruna.tex` | Academic paper |
| `editors/vscode/` | VS Code extension (syntax highlighting + theme + LSP) |
| `website/` | Dioxus WASM website with research hub |

## Completed Milestones

**Core language:** M1 (structs, for loops, build), M2 (error handling, stdlib), M3a (multi-file imports), M3b partial (export/visibility), M5 (borrow inference, escape analysis), M6 (actor concurrency), M7 (algebraic effects), M8 (monadic sugar), M9 (comptime), M10 (mutable value semantics, inout), M11 partial (content-addressed modules), M12 (reactive streams, pipe operator), M16 (pre-codegen type checker)

**Streams & concurrency:** M13a (scope execution), M13b partial (subjects, push streams), M15 (multi-threaded Tokio, trust-the-topology), M17 (timing operators: debounce, throttle, delay, buffer, timeout, switch_map, sample), M20 (async stream ops + stream fusion)

**Standard library:** M14a (16 string builtins), M14b (6 file I/O), M14c (8 JSON via serde_json), M14d (HTTP via ureq/tiny_http), M14e (DB via rusqlite), M24 (22 map+set builtins)

**Tooling:** M18 (runa fmt), M19 (LSP), M21 (runa audit), M22 (package manager: init, add, runa.toml)

**Advanced:** M4 partial (WASM target), M23 (Datalog+ logic programming), M25 (transparent Rc for structural sharing)

## Conventions

- File extension: `.runa`
- Comments: `--` line comments, `{- block comments -}`
- Source text blocks: `----` ... `----` (for quoting legal text being modeled)
- Transpilation target: Rust (via `runa emit`)
- Naming: snake_case for functions/variables, PascalCase for types/constructors
- Unicode identifiers: æøå fully supported (lexer uses `is_alphabetic()`)
- Tests go in `tests/` as `.runa` files
- The escape hatch `@ rust { }` embeds raw Rust when needed (avoid Rust lifetimes — use owned types)
- Immutable by default; `inout` for in-place mutation; actors for shared mutable state
- Private by default; `@ export` for visibility across modules
- Multi-file models: source files have types/facts/functions, audit file has invariants/proofs


<!-- BEGIN TD INTEGRATION v:1 profile:minimal -->
## TD Issue Tracker

This project uses **td** for issue tracking. Run `td usage --new-session` at conversation start or after a context reset.

### Quick Reference

```bash
td ready              # Find available work
td show <id>          # View issue details
td start <id>         # Claim/start work
td log "message"      # Record progress
td handoff <id>       # Capture handoff context
td review <id>        # Submit completed work for review
td approve <id>       # Approve reviewed work from another session
td reject <id>        # Return reviewed work to open
```

### Rules

- Use `td` for all task tracking; do not use TodoWrite, TaskCreate, or markdown task lists for project work.
- Run `td usage --new-session` at conversation start or after a context reset.
- Use `td log` and `td handoff` for persistent work context.
- Completed implementation work should go through `td review`; a different session should use `td approve` or `td reject`.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END TD INTEGRATION -->
