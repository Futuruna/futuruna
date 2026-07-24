---

kanban-plugin: board
theme: Futuruna Milestones
tags:
  - futuruna
  - milestones
  - board
  - theme
  - td

---

## Now

- [ ] ### Mint Hardening for Downstream Users
	**td path:** [[td-f7f0d2]] — Keep Futuruna mint for users (epic)

	**Milestone meaning**
	The mint gate (`./scripts/mint.sh`) is the contract Futuruna offers to anyone who imports a `.runa` library or compiles a `.runa` program against a real Cargo backend. The work right now is not language features — it is closing the long tail of downstream-consumer compiler bugs that surface only when programs are imported, exported, and compiled instead of run from a single file. Authored canaries and downstream fixtures already catch most regressions; this milestone is the steady-state hardening that lets us call any feature "stable" without quotes.

	**Exit criteria**
	- Every open downstream-consumer bug under this epic has either a fix at HEAD or a regression fixture in `tests/downstream/` plus a follow-up issue
	- `./scripts/mint.sh` runs end-to-end on a clean clone with no auto-comptime noise that would mask real failures
	- Compatibility-policy categories (stable / preview / experimental) are accurate for every surface a downstream consumer can touch
	- A user can take a tagged `.runa` library, `runa add` it, and consume it without a compiler bug filed within 24 hours

	**References**
	- [[mint-gate]]
	- [[compatibility-policy]]
	- [[canary-matrix]]


- [ ] ### Stream Subscription Lifetime Contract
	**td path:** [[td-17811d]] — Align stream subscription lifetimes with explicit scopes (epic)
	**Open under epic:** [[td-1e74be]] (contract doc), [[td-b48d46]] (returned-handle design)

	**Milestone meaning**
	Runtime ownership of streams is now coherent: scopes own derived async operator handles, function-local subscriptions outside a scope are rejected at compile time, and a lifecycle canary proves teardown actually freezes derived streams. What is missing is the *written contract* — a doc that pins what a `| scope` guarantees, how returned streams should work, and where the boundary sits between snapshot reads and live subscriptions. Without that contract the implementation is ahead of the spec, and any future relaxation (e.g. function-as-scope sugar) has nothing to validate against.

	**Exit criteria**
	- A reference doc defines: scope-ownership rules, snapshot vs live distinction, derived-operator handle ownership, teardown ordering, and the rules for crossing function/scope boundaries
	- Returned-stream design (td-b48d46) is named in the doc as an open question and tracked separately
	- Validation diagnostics in `runa.rs:19776` cite the contract doc
	- Function-as-scope question is surfaced in the contract doc as an open design question with no forced answer; resolution tracked in [[td-fb9cf4]]

	**References**
	- [[stream-lifetimes]]
	- [[reactive-design]]


- [ ] ### M26b — Persist Phase B
	**td path:** [[td-c0a7a1]] (epic)
	**Subtasks:** [[td-8b2887]] typed columns · [[td-7b097f]] assert/retract · [[td-f4f433]] findall · [[td-c4282c]] scope-as-transaction · [[td-13a997]] watch · [[td-25667e]] migrate
	**Phase A shipped:** object store with `assert` / `retract` / scoped DBs

	**Milestone meaning**
	Phase A made `@ store Type` real: a struct becomes a JSON-backed SQLite blob with `assert` / `retract` mutation, and `| scope` is the natural transaction boundary. Phase B turns that into a database-from-language: `findall` over persisted facts, true scoped transactions (commit on scope-end, rollback on diagnostic), `watch(Type)` change streams that flow into the existing reactive runtime, and `@ persist Type` for typed-column storage instead of JSON blobs. The runes already cover all the database concepts — Phase B finishes the work without inventing new syntax.

	**Exit criteria**
	- `findall(x, persisted_pred(x))` returns matching rows from the store
	- `| scope` boundaries map to SQLite transactions; rollback on uncaught error or `?` failure
	- `watch(Type)` produces a `~ Stream(Type)` that fires on `assert` / `retract`
	- `@ persist Type` lowers struct fields to typed columns (Int → INTEGER, etc.)
	- `@ migrate { ... }` handles schema evolution between persist versions
	- An end-to-end example (e.g. inventory or task-tracker) runs against persisted state across process restarts

	**References**
	- [[research-persist]]
	- `docs/persist/research-persist.md`


## Next

- [ ] ### Proof-Backed Checking Expansion
	**td path:** [[td-a97ed9]] — Harden proof-backed compiler checking (epic)
	**Open under epic:** [[td-4d7e81]] (expectation corpus), [[td-15746e]] (golden snapshots), [[td-48e5d9]] (cross-binding phase samples)

	**Milestone meaning**
	`runa expect` and `tests/expect/` give us pinned diagnostics, run/fail behavior, and phase-specific markers — the compiletest-equivalent for a small language. The lane works; what it lacks is breadth. To make proof-backed checking load-bearing we need a wider corpus: more diagnostic shapes, phase snapshots across binding/module boundaries, and golden-file support so structural FIR snapshots can be compared without writing brittle string matchers.

	**Exit criteria**
	- `tests/expect/diagnostics/` covers every error category from M16/M27 (parse, type, ownership, scope, exhaustiveness, comptime)
	- Golden-file snapshot mode lands in `runa expect`
	- Phase snapshots include cross-binding and cross-module samples, not just isolated functions
	- A regression in any user-visible phase output is caught by the expectation lane before it lands on main

	**References**
	- [[expectation-suites]]
	- [[proof-backed-checking]]


- [ ] ### Canary Matrix Tail
	**td path:** [[td-39b478]] — Build an authored Futuruna canary suite (epic)
	**Open tail:** [[td-4ceb72]] (WASM build canary lane), [[td-770bee]] (check-codegen external-crate gap), [[td-acc049]] follow-ups

	**Milestone meaning**
	The core wave is shipped: 5 stateful canaries, 6 extended canaries, 3 downstream-consumer families, all tracked in `docs/canary-matrix.md`. The remaining tail is targeted: an automated WASM build canary lane so `runa wasm` regressions are caught structurally, and closing the check-codegen gap where external-crate breakages slip through because the lane skips local imports. After this the canary matrix is at steady-state expansion (one per real bug) rather than backlog burn-down.

	**Exit criteria**
	- WASM build canary lane exists and runs on every commit; failure surfaces in mint
	- `runa check-codegen` covers local multi-file import consumers (closes td-770bee)
	- Canary matrix lists "owners" or "next reviewer" for each lane so it stays current
	- New compiler bugs land with a canary fixture or a tracked follow-up — no fixture-less bug fixes

	**References**
	- [[canary-matrix]]
	- [[test-surface]]


- [ ] ### M27 — Error Reporting Overhaul
	**Milestone meaning**
	Type-checker and codegen errors today are functional but inconsistent: some have rich span info, some only line numbers, multi-error recovery is partial, and there is no common "did you mean..." or quick-fix surface. M27 unifies the diagnostic shape across all phases and gives the LSP something stable to render.

	**Exit criteria**
	- All compiler diagnostics carry source spans; line/col is no longer the bottom of the chain
	- Multi-error recovery: a parse error in one function does not hide type errors in others
	- "Did you mean" suggestions for undefined identifiers and constructors
	- LSP surfaces all of the above as code actions where applicable

	**References**
	- `docs/milestones/m27-error-reporting.md`


- [ ] ### M28 — Negative Tests + CLI Polish
	**Milestone meaning**
	`tests/errors/` exists with a baseline of 15 expect-error fixtures. M28 expands negative coverage to the levels expected of a real compiler — 10+ parse errors, 10+ type errors, 5+ runtime errors, 1000-line stress program — and finishes the small CLI ergonomics that downstream users hit first: `--version`, helpful unknown-flag messages, integration of negative tests into `runa test`.

	**Exit criteria**
	- 10+ parse-error fixtures, 10+ type-error fixtures, 5+ runtime-error fixtures
	- A 1000-line stress program in the test suite that catches quadratic blowups
	- `runa --version` prints from `Cargo.toml`; unknown flags suggest the closest match
	- `runa test` discovers and runs `tests/errors/` automatically

	**References**
	- `docs/milestones/m28-negative-tests-cli.md`


- [ ] ### M33 — Trait Resolution and Method Dispatch
	**Milestone meaning**
	Trait declarations and impls parse and lower today, but the registry and resolution machinery is incomplete. M33 makes traits a first-class abstraction layer — bounds on type parameters, multi-impl resolution, auto-derive for primitive types — so generic code over abstract behavior stops being a partial story.

	**Exit criteria**
	- Trait registry collects every `# trait T` and validates impls cover required methods
	- `x.method()` resolves through the trait hierarchy with sensible ambiguity errors
	- `> sort(xs: List(T)) where T: Ord` works end-to-end
	- Built-in trait auto-deriving for Copy / Eq / Show on all-primitive structs
	- Existing `tests/traits_test.runa` is expanded with adversarial dispatch cases

	**References**
	- `docs/milestones/m33-trait-resolution.md`


- [ ] ### Match Exhaustiveness Broaden
	**Milestone meaning**
	The exhaustiveness checker (`src/lib.rs:12480`) handles top-level constructor enumeration on a single ADT. Anything more — Bool, nested patterns, subset types, guard-only catch-alls, tuple destructuring — falls through silently. This matters for a language that leans on constitutional modeling and `EXCEPT` subset types, where a forgotten case is the entire point of the language being able to find it. Adding `Pat::Tuple` and `Pat::Or` is cheap, and the boundary regressions are six small fixtures.

	**Exit criteria**
	- Bool exhaustiveness flags `match b { | true -> ... }` as missing `false`
	- Nested constructor exhaustiveness: `match Some(Result)` requires all four combinations
	- Subset types: matching on `Skandinavien` knows the parent's variants are scoped down
	- Guard-only arms do not count as catch-all
	- `Pat::As` over a wildcard counts as catch-all
	- `Pat::Tuple` and `Pat::Or` land with grammar + checker support
	- Six expect-error fixtures pin the boundary cases above

	**References**
	- [[runes]]


- [ ] ### M30 — Split RustCodegen into Passes
	**Milestone meaning**
	Codegen today is a large single walk that mixes declaration collection, type inference, ownership analysis, and Rust emission. M29 (FIR) and M31 / M32 (annotation, inference) carved out the typed core. M30 finishes the split: declaration / import / type / ownership / FIR / emit, each a separate pass over a typed AST. After this, every pass is testable in isolation, and adding new analyses (effects, borrow patterns, escape) does not require touching emission.

	**Exit criteria**
	- Six named passes, each with a clear input and output type
	- `compile(ast: &[Stmt]) -> String` is a thin orchestrator
	- Duplicate type metadata between `TypeChecker` and codegen is centralized
	- Phase snapshots (M16) cover each pass independently

	**References**
	- `docs/milestones/m30-passes.md`


## Later

- [ ] ### M34 — Package Manager v2
	**Milestone meaning**
	M22 shipped `runa init`, `runa add`, and `runa.toml` with local path and git deps. M34 adds reproducibility: a lock file, semver resolution, transitive dep handling, and offline builds. This is the gate to publishing real `.runa` libraries that other projects can pin against.

	**Exit criteria**
	- `runa.lock` generation and consumption
	- Semver resolution against `version = "^1.2"` constraints
	- Transitive dep resolution with conflict reporting
	- `runa deps` shows the resolved tree
	- `runa build --offline` uses only locked / cached deps


- [ ] ### M35 — Stdlib Expansion
	**Milestone meaning**
	Real-world programs need regex, datetime, randomness, and `sleep`. M14 covered strings, file I/O, JSON, HTTP, DB, collections. M35 closes the rest of what a typical Kotlin / Python program reaches for in the first hour.

	**Exit criteria**
	- Regex builtins (match, find, replace, split) in interpreter + codegen
	- DateTime builtins (now, parse, format) with consistent timezone handling
	- Random builtins (float, choice, seeded)
	- `sleep(ms)` async-aware


- [ ] ### M36 — WASM Target Completion
	**Milestone meaning**
	M4 partial gave us the bones of WASM output. M36 finishes it: `#[wasm_bindgen]` on exported functions, full type mapping, end-to-end `runa wasm`, WASI target, and graceful suppression of incompatible builtins (file I/O, HTTP server, DB).

	**Exit criteria**
	- Browser-runnable WASM example exporting a typed function
	- WASI target via `runa wasm --wasi`
	- Incompatible builtins emit clear compile errors instead of runtime crashes


- [ ] ### M37 — Getting-Started Tutorial + Docs
	**Milestone meaning**
	A new user should reach "I built something with this" in under thirty minutes. M37 is the curated path: install, hello, a small real program, deploy. Existing `docs/` is reference-shaped, not learning-shaped.

	**Exit criteria**
	- 30-minute getting-started flow with runnable code
	- Hosted docs site (could land alongside M40)
	- Curated example progression from `hello` to the showcase project-examples


- [ ] ### M38 — CI/CD Pipeline
	**Milestone meaning**
	Mint and differential CI exist. M38 adds release automation: tagged versions, binary artifacts for macOS / Linux / Windows, automated changelog generation, and crates.io / brew-tap publishing.

	**Exit criteria**
	- Tagged release produces signed binaries for three platforms
	- Changelog generated from commit messages or td issues
	- Brew tap or equivalent for one-line install


- [ ] ### M39 — VS Code Marketplace
	**Milestone meaning**
	The extension lives in `editors/vscode/` with syntax highlighting, theme, and LSP. M39 publishes it.

	**Exit criteria**
	- Extension on the VS Code marketplace under a stable publisher
	- Auto-updates work
	- LSP version compatibility is documented


- [ ] ### M40 — Website + Playground
	**Milestone meaning**
	`website/` is a Dioxus WASM app with the research hub. M40 finishes the playground (in-browser compile + run) and polishes the routes for the public release.

	**Exit criteria**
	- In-browser playground compiles and runs `.runa` programs against a WASM-built compiler
	- All public routes have polished content
	- Site deploys on every main commit


- [ ] ### Self-Hosting Next Slice
	**Milestone meaning**
	The lexer (`examples/lexer.runa`) and parser are written in Futuruna. The next two pieces — type checker and interpreter — would let us drop the Rust host for the front half of the compiler. This is a deliberate stress test of the language: if Futuruna can write its own type checker without escape hatches, the abstraction story is honest.

	**Exit criteria**
	- A Futuruna-implemented type checker passes the same fixtures as the Rust one on a representative subset
	- A Futuruna-implemented interpreter runs `examples/weather_demo.runa` byte-identically
	- The escape hatch (`@ rust { }`) is not used in either


- [ ] ### Decide Function-as-Scope for Stream Lifetimes
	**td path:** [[td-fb9cf4]]

	**Milestone meaning**
	Today every function that hosts a live stream subscription must wrap the body in `| scope Name { ... }`. The forced-naming ceremony is the hot spot. Three points on the spectrum: (A) keep the status quo — explicit named scope required; (B) allow anonymous `| scope { ... }` — drops forced naming, keeps the brace; (C) function-as-scope — function frame implicitly counts as a scope. The current design analysis sits in `docs/stream-lifetimes.md` under "Open Design Decisions > Function-as-scope" but does not commit to a direction. This card exists to make sure the question is held until decided.

	**Exit criteria**
	- A decision is recorded in `docs/stream-lifetimes.md` — A, B, C, or a hybrid — with rationale tied to actual ergonomic friction or aesthetic argument
	- If the answer is anything other than (A), a follow-up implementation task is filed
	- The decision references real cases (downstream consumers, internal demos) rather than speculation

	**References**
	- [[stream-lifetimes]]


## Done

- [x] ### M1 — Structs + For Loops + One-Step Build
	**Status:** `runa run/build` work end-to-end; structs, dot access, typed lambdas, recursive method dispatch all green.

- [x] ### M2 — Error Handling + Standard Library
	**Status:** `?` operator, Result/Option, string/Vec methods, mutable accumulators, prelude all shipped.

- [x] ### M3 — Modules, Imports, Dependencies
	**Status:** `@ import`, `@ depend`, auto Cargo.toml — multi-file programs work.

- [x] ### M5 — Borrow Inference
	**Status:** Escape analysis + auto-borrow inference. 76 adversarial patterns documented in `docs/research-ownership.md`.

- [x] ### M6 — Actor Concurrency
	**Status:** Tokio-channel-backed actors with `spawn`, `<-`, `ask`. Codegen adversarial regression tests live in `runa.rs`.

- [x] ### M7 — Algebraic Effects
	**Status:** Koka-style effect declarations + handlers, with `resume` and effect inference.

- [x] ### M8 — Monadic Sugar
	**Status:** `<-` early-return on Result/Option chains. `monadic_test.runa` covers the cases.

- [x] ### M9 — Comptime
	**Status:** `@ comptime` for compile-time evaluation; auto-comptime for top-level bindings.

- [x] ### M10 — Mutable Value Semantics
	**Status:** Hylo-inspired `inout` for in-place mutation without breaking value semantics elsewhere.

- [x] ### M11 — Content-Addressed Modules (partial)
	**Status:** `runa hashes` and `runa registry` ship; full Unison-style identity tracking deferred.

- [x] ### M12 — Reactive Streams (Cold)
	**Status:** Pipe operator, cold pipelines, fusion. Native dataflow without runtime overhead.

- [x] ### M13 — Scopes, Subjects, Lifecycle (Hot)
	**Status:** `| scope`, `subject()`, scoped projections, derived-operator scope ownership all shipped.

- [x] ### M14 — Standard Library
	**Status:** Strings (16 builtins), file I/O (6), JSON (8 via serde_json), HTTP (ureq + tiny_http), DB (rusqlite), collections (16+22 map/set).

- [x] ### M15 — Multi-Core Streams
	**Status:** Trust the topology — Tokio threads, no manual concurrency.

- [x] ### M16 — Pre-Codegen Type Checking
	**Status:** `runa check` is the fast type-check path; catches arity, undefined names, exhaustiveness baseline.

- [x] ### M17 — Timing Operators
	**Status:** debounce, throttle, delay, buffer, timeout, switch_map, sample. Plus tap, catch, reduce.

- [x] ### M18 — `runa fmt`
	**Status:** Format single files or directories; `--check` mode for CI.

- [x] ### M19 — LSP + Editor Integration
	**Status:** JSON-RPC LSP over stdio. VS Code extension consumes it.

- [x] ### M20 — Async Stream Operators + Fusion
	**Status:** Async-aware operators, fusion across pipeline stages.

- [x] ### M21 — `runa audit`
	**Status:** Automated gap discovery — symmetric pairs, asymmetries, tensions, paradoxes from the `|` rule topology.

- [x] ### M22 — Package Manager v1
	**Status:** `runa init`, `runa add` (path + git), `runa.toml`. Lock files / semver are M34.

- [x] ### M23 — Datalog+ Logic Programming
	**Status:** Facts, rules, `findall`, `search`, transitive closure, `not`, wildcards, type-constrained rules.

- [x] ### M24 — Map + Set Collections
	**Status:** 22 map/set builtins in interpreter and codegen.

- [x] ### M25 — Transparent Rc
	**Status:** Auto-Rc for structural sharing on recursive ADTs.

- [x] ### M26a — Persist Phase A
	**Status:** `@ store Type` with `assert` / `retract`, scoped DBs (file-stem default, explicit `in "scope"` shared).

- [x] ### M29 — Intermediate Representation (FIR)
	**Status:** AST → FIR lowering, FIR → Rust emission, end-to-end pipeline. `runa emit --fir` flag.

- [x] ### M31 — Type Annotation Pass
	**Status:** Every FIR expression carries a resolved type. Replaces heuristic sets like `string_typed_vars`.

- [x] ### M32 — Type Inference
	**Status:** Constraint generation + union-find unification. Generic ADTs with real type parameters.

- [x] ### Targeted Semantic Audits Epic
	**td path:** [[td-a27991]] (closed)
	**Status:** Audit waves over high-risk language surfaces shipped under this epic; durable regressions and follow-ups left behind.




%% kanban:settings
```
{"kanban-plugin":"board","show-checkboxes":false,"lane-width":340,"show-relative-date":true,"new-card-insertion-method":"prepend-compact","list-collapse":[false,false,false,false]}
```
%%
