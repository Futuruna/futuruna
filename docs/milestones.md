# Futuruna Language Milestones: From Prototype to Real

**Goal:** Make Futuruna a language you can actually build things with. The Kotlin-to-Rust
analogy isn't real until someone can write a web server, a CLI tool, or a WASM app
in Futuruna and ship it.

## Current State

**~10,000-line compiler** in `runa.rs`. Lexer, parser, interpreter, and Rust transpiler.
Kotlin-style nullability (`T?`, `?.`, `?:`) added — desugars to Option(T) match.
Catala-style default logic with exceptions: conditional `under` rules override
unconditional defaults, `| exception` overrides everything. Weather demo running.

**What works:** ADTs (recursive, auto-boxed), pattern matching, higher-order functions,
lambdas with closure capture, traits + impls, qualified paths (`fmt::Display`),
`@ rust { }` escape hatch (raw source preserved), `@ use` imports, escape analysis
(move/clone/borrow), Copy detection, interpreter + compiler with byte-identical output.

**What doesn't (yet):** No WASM target, no timing operators (debounce/throttle).

## M1: Structs + For Loops + One-Step Build

**The "write real code" milestone.** After this, Futuruna programs can define data, iterate,
and compile in one command.

- [x] **Standalone structs**: single-variant `# Point(x: Float, y: Float)` emits
  `struct Point { x: f64, y: f64 }` instead of single-variant enum. Dot access works.
- [x] **For loops**: `for x in xs { ... }` parses and emits Rust `for` loops.
  Works on Vec, ranges, and iterators.
- [x] **`runa --build`**: one command transpiles + compiles to native binary.
  `runa --build program.runa` -> `./program`. No manual rustc step.
- [x] **`runa --run`**: transpile + compile + execute in one step.
- [x] **Typed lambda parameters**: `|x: Int| x * 2` parses type annotations on lambda params.
- [x] **Fix method self-reference**: `t.size()` on recursive types works.
  Interpreter: method dispatch binds `self` to object, creates closure.
  Codegen: `&self` methods skip boxed unboxing (`let x = *x;`), auto-deref handles it.

**Test:** Write a Futuruna program that defines a struct, iterates over a list, and
compiles with `runa --build`. ✅ `hello.runa` — struct Point with dot access,
for loop over list, typed lambda, all byte-identical interpreter vs compiled.

## M2: Error Handling + Standard Library

**The "handle failure gracefully" milestone.** Real programs need Result, Option,
and a way to propagate errors.

- [x] **`?` operator**: `parse_int("42")?` compiles to Rust's `?`.
  `main()` auto-detects `?` usage and emits `-> Result<(), Box<dyn std::error::Error>>`.
- [x] **Result/Option integration**: Futuruna's `Result(T, E)` and `Option(T)` map
  directly to Rust's types. `Ok`, `Err`, `Some`, `None` pass through.
- [x] **Equality operator**: `=` in expressions emits `==` in Rust.
- [x] **String methods**: `.len()`, `.contains()`, `.split()`, `.trim()`,
  `.to_uppercase()`, `.starts_with()` — all pass through to Rust method calls.
  String literal args in method calls stay as `&str` (no `.to_string()`).
- [x] **Vec methods**: `.len()`, `.push()`, `.iter()`, `.map()`, `.filter()`
  — all pass through to Rust.
- [x] **Builtins**: `map(xs, f)` → `.into_iter().map(f)`, `filter(xs, f)`,
  `foldl(xs, init, f)`, `range(a, b)` → `(a..b)`, `push(xs, x)`.
  User-defined functions with same names take priority.
- [x] **Mutable accumulators in for loops**: `= total = total + i` inside
  for loops emits as mutation (auto `let mut` + assignment).
- [x] **Standard prelude**: auto-imported types (Option, Result, Pair) and functions
  (unwrap_or, is_some, is_none, max_int, min_int, clamp, identity).
  Embedded in runa.rs, parsed once, prepended to user's program.
  `--no-prelude` flag disables. User definitions shadow prelude.
  List ADT intentionally excluded (conflicts with List→Vec type mapping).

**Test:** `m2_full.runa` — parses ints with `?`, safe division, string methods,
Vec operations, range + for-loop accumulator, map with typed lambda. ✅

## M3: Modules, Visibility & Dependencies

**The "build real projects" milestone.** Programs can span multiple files,
control what's public, and use external crates.

**Design rationale — module ≠ scope:** Every language that merges modules
(compile-time visibility) with scopes (runtime lifecycle) creates accidental
complexity — React components are the worst case (module + scope + state in one,
causing stale closures and `useEffect` footguns). Languages that separate them
cleanly (Rust `mod` + `Drop`, Kotlin packages + `coroutineScope`, Elixir
`defmodule` + processes) work better. Futuruna follows:

| | `> module` | `| scope` |
|---|---|---|
| When | Compile time | Runtime |
| Controls | **Visibility** — who can see names | **Existence** — when streams live |
| Rune | `>` (what happens) | `|` (what should be true) |
| Rust analog | `mod` + `pub` | `Drop` guard |

Three orthogonal concerns: **module = who can see** (visibility),
**scope = when it lives** (lifecycle), **Subject vs Stream = who can write**
(mutability). Zero overlap.

### M3a: Multi-File Imports + Dependencies (DONE)

- [x] **`@ import`**: `@ import ./math` imports from another .runa file.
  Resolves relative paths, prevents cycles, merges definitions (functions,
  types, rust blocks) into the main program. Works in both interpreter and transpiler.
- [x] **`@ depend`**: `@ depend "serde_json" "1"` adds Cargo dependencies
- [x] **Auto Cargo.toml**: `runa --build` generates Cargo project in `.runa-build/`,
  manages dependencies, caches builds. No-deps programs still use fast `rustc` path.

**Test:** Multi-file Futuruna project with imported math/greet modules compiles and runs.
Dependency test uses serde_json via `@ depend` — auto-generated Cargo project,
downloaded crate, compiled and ran. Interpreter and compiled output byte-identical
for import tests. ✅ `/tmp/tau-m3/` (main.runa + math.runa + greet.runa + deps_test.runa)

### M3b: Module Visibility + Export (TypeScript model)

Private by default. `@ export` marks names that cross the module boundary.
File = module (like Rust, TypeScript). `> module` for inline sub-modules.

```tau
-- counter.runa (file IS the Counter module)

~ count = subject(0)                              -- private (default)

@ export ~ value = as_stream(count)               -- Stream(Int), read-only
@ export > increment() { count <- count.latest() + 1 }
@ export # Status = Active | Paused               -- export types too
```

From the importer's side:
```tau
@ import Counter from ./counter

for x in Counter.value { @ print(show(x)) }      -- fine: exported Stream
Counter.value <- 99                                -- compile error: Stream, not Subject
Counter.increment()                                -- fine: exported function
Counter.count                                      -- compile error: not exported
```

- [x] **`@ export` annotation** — `@ export > f()`, `@ export # T`, `@ export ~ s`.
  Uses the `@` rune because export is meta-level (same category as
  `@ import`, `@ use`, `@ depend`). Emits `pub fn` / `pub struct` / `pub enum`
  in Rust. Non-exported names emit without `pub` (private by default).
  Imported files: `@ export` annotations propagate — exported names from
  imported modules also get `pub` in the combined Rust output.
- [x] **File = module** — `counter.runa` defines the `Counter` module.
  `@ import Counter from ./counter` brings exported names into scope
  with qualified access (`Counter.add()`). Current `@ import ./file`
  still imports all definitions (backwards compatible). Qualified imports
  enforce privacy: only `@ export`-ed names are accessible, private
  names return `()` in interpreter. Codegen wraps imported file in
  `mod Name { use super::*; ... }` with `pub` on exported items.
  Module-qualified calls (`Name.func()`) emit `Name::func()` in Rust.
  Borrow inference works through module-qualified calls.
  ADT constructors of exported types are included in the module namespace.
- [x] **Inline sub-modules** — `> module Name { }` for grouping within a file.
  Sub-module bindings are private to the sub-module by default.
  Qualified access only: `Name.function()` (no unqualified leaking).
  Nested modules supported: `App.Utils.clamp(x, lo, hi)`.
  Codegen: `mod Name { use super::*; ... }` with `Name::func()` calls.
  Nested module paths emit chained `::` (e.g., `App::Utils::func()`).
- [x] **Scope opaqueness** — `| scope` bindings are accessible within the
  enclosing module (internal wiring) but never cross the module boundary.
  Scopes are excluded from `@ export` name collection and from codegen
  module wrapping. If a scope produces data the module wants to export,
  the module author wires it to an exported stream. Consumers see streams,
  not lifecycle.
- [x] **Futuruna as library**: `runa lib program.runa` emits Rust library
  code (no `fn main()`). Exported names get `pub`, private names stay private.
  Generated code is callable from Rust projects as a crate dependency.

**Test:** `export_lib.runa` + `module_test.runa` — 5 demos: qualified import
with exported/private functions and types, inline modules with qualified access,
nested modules with chained access, module with ADTs and constructors,
privacy enforcement (private names return `()`). Codegen emits `pub fn`/`pub enum`
for exported, plain `fn`/`enum` for private. `runa lib export_lib.runa` emits
clean Rust library (no main, `pub` on exports). Interpreter + compiled output
byte-identical. ✅

## M4: WASM + Deployment Targets

**The "ship it anywhere" milestone.** Futuruna programs compile to WASM, run in
browsers, deploy to edge.

- [ ] **`runa --target wasm`**: emits wasm-bindgen annotated Rust
- [ ] **`#[wasm_bindgen]` auto-annotation**: exported Futuruna functions get bindings
- [ ] **JS interop**: Futuruna types serialize to/from JS values
- [ ] **`runa --target wasi`**: WASI target for server-side WASM

**Test:** A Futuruna program that compiles to WASM and runs in a browser.

## M5: Borrow Inference + Performance

**The "faster than hand-written Rust" milestone.** The compiler sees more context
than the human and makes better ownership decisions.

- [x] **Whole-program `&T` inference**: functions that only read arguments get
  `&T` signatures automatically. Auto-borrow (Phase 2b) + borrow-aware counting
  (Phase 3b) cascade: accessor functions take `&T`, callers inherit.
- [x] **Branch-aware escape analysis**: variables used in only one branch of
  if/else don't get cloned. Branch-aware counting takes MAX across branches.
- [x] **`shared` keyword**: `shared T` in type annotations emits `Arc<T>`.
  `shared(expr)` wraps value in `Arc::new(expr)`. Interpreter: transparent (no-op).
  Arc implements Clone (cheap ref-count) and Deref (auto-access inner value).
- [x] **Move analysis across calls**: caller knows callee only borrows,
  avoids unnecessary clone at call site. `count_consuming_uses_borrow_aware()`
  skips borrow-param positions when counting consuming uses.
- [x] **Ref-match for accessor functions**: match on `&T` when type has all-Copy
  fields and no recursive (boxed) structure. Pattern bindings auto-deref `(*a)`.

**Test:** Benchmark Futuruna vs hand-written Rust on a real program. Futuruna should
produce equivalent or fewer allocations.

## M6: Actor Concurrency

**The "safe concurrency without Arc<Mutex<T>>" milestone.**

- [x] **Actor definitions**: `> actor counter(state: Int) { | Increment -> state + 1 }`
  Parser, interpreter, and Rust codegen. Actors are first-class values with
  encapsulated state and message handlers.
- [x] **`<-` send operator**: `c <- Increment` dispatches message, updates actor state
  in-place. Interpreter: synchronous dispatch. Codegen: `tx.send(Msg).unwrap()`.
- [x] **`ask` for request-response**: `= val = ask(c, Increment)` sends message
  and returns the new state. Interpreter: immediate. Codegen: oneshot channel.
- [x] **spawn**: `= c = spawn(counter, 0)` creates actor with initial state.
  Interpreter: `Value::Actor`. Codegen: tokio channel + spawned async task.

**Test:** `actor_test.runa` — 4 demos: counter (inc/dec/reset), accumulator (add/sub),
state inspection, multiple independent actors. All produce correct results.
Interpreter passes. ✅

## M7: Algebraic Effects (Koka's Lesson)

**The "effects are values, not magic" milestone.** Side effects become first-class,
composable, and handler-dispatched through the `|` pathway. Koka proved this is
tractable. Futuruna's three-axis syntax makes it *more* readable.

- [x] **`# effect` declarations**: `# effect Console { > print(msg: String) -> () }`
- [x] **Effect operations in functions**: `> greet() -> () with Console { say("hello") }`
- [x] **`| handle` blocks**: `| handle Console { | say(msg) -> body } in expr`
- [x] **`resume` for continuations**: `resume(val)` returns val as effect op result
- [x] **Nested handlers**: multiple effects handled independently, compose freely
- [x] **Rust codegen**: `# effect` emits Rust trait, `| handle` emits struct + impl
- [x] **Effect type inference**: compiler infers which effects a function performs
  by walking function bodies. Finds direct effect ops and transitive effects from
  callees. Iterates to fixed point. `| handle` blocks suppress propagation.
  Distinguishes handle-scope effects (concrete structs, `&mut`) from function
  params (already `&mut impl E`, auto-reborrow).
- [x] **Built-in effects**: Console (say, ask) in standard prelude. State(s) blocked
  by parser (no parameterized effects). User-defined effects shadow prelude's.
- [x] **Effect polymorphism**: HOFs accept effectful closures without knowing which
  effects are used. Closures inside `| handle` scopes capture the handler struct;
  lambdas that call effectful functions auto-forward handlers. `Ty::Arrow` emits
  `impl FnMut(...)` (not `Fn`) so effectful closures work. `()` on left of `->` skips
  to emit `FnMut()` not `FnMut(())`. Function params with Arrow types get `mut` prefix.
  Nested FnMut call fix: `f(f(x))` pre-binds inner call to temp to avoid double
  mutable borrow.
- [x] **Rust codegen threading**: `with Effect` → `__eff_E: &mut impl E` param added.
  Effect ops route through handler (`say(x)` → `__eff_Console.say(x)`).
  `| handle` creates struct+impl, passes `&mut` to callee. `resume(val)` → `val`.
  Caller-side forwarding: functions that call effectful functions auto-pass handlers.
  Handler capture: free variables in handler bodies detected, stored as struct fields,
  accessed via `self.field`. Type inference from binding expressions (literal → i64/String/etc).
  Distinguishes handle-scope effects (`&mut`) from function-param effects (reborrow).

**Test:** A Futuruna program that defines a custom effect, handles it two different ways
(real IO vs mock). DONE — `effects_test.runa` (5 demos, 2 effects, nested handlers).

## M8: Monadic Sugar (`<-` on `=` bindings) (Gleam's Lesson)

**The "no more callback pyramids" milestone.** `use`/`<-` syntax desugars
Result/Option/Effect chains into sequential-looking code.

- [x] **`= x <- expr` syntax**: desugars to `?` operator (unwrap or early-return)
- [x] **Works with Result**: `= user <- get_user(id)` early-returns on Err
- [x] **Works with Option**: `= val <- lookup(key)` early-returns on None
- [x] **Works with effects**: `= line <- readline()` suspends to effect handler.
  MonadicBind detects effect operation calls via `is_effect_op_call()` and omits `?`
  (effect ops return values directly, not wrapped in Result/Option). Fixed in all three
  MonadicBind codegen sites (emit_stmt, Expr::Block, emit_expr_as_return).
- [x] **Chainable**: multiple `<-` bindings compose naturally
- [x] **Rust codegen**: `= x <- expr` emits `let x = expr?;`, auto-wraps `fn main()` in `Result`

**Test:** A Futuruna program with 4+ chained `<-` bindings that compiles to clean Rust
with proper error propagation. DONE — `monadic_test.runa` (4 demos: Result happy/error
path, Option chaining, 3-step pipeline).

## M9: Comptime (Zig's Lesson)

**The "your functions run at compile time too" milestone.** Any pure function
can execute at compile time when inputs are known. No macro system needed.

- [x] **`@ comptime` annotation**: marks a binding for compile-time evaluation
- [x] **Const tables**: `@ comptime = table = generate_lookup(1000)` becomes `vec![...]` literal in Rust
- [x] **Comptime strings/ints/floats/bools/lists**: interpreter evaluates, codegen inlines result
- [x] **Rust `const` for Copy types**: `@ comptime = x = factorial(10)` → `const x: i64 = 3628800;`
- [x] **Auto-comptime**: pure functions with literal args auto-evaluate at compile time.
  Purity analysis finds functions with no effects, no `@ print`, no `| handle`.
  Iterates to fixed point for transitively impure functions.
  Chained: auto-comptime values feed into subsequent auto-comptime evaluations.
- [x] **Comptime types**: type-level computation (generate types from data).
  `struct_type(fields)` and `enum_type(variants)` return `TypeDef` values at compile
  time. Comptime pass converts them to real Rust types (structs/enums). Functions
  returning `TypeDef` are comptime-only (suppressed from Rust output). Supports
  unit-variant enums, field-carrying enums, named-field structs, and function-
  generated types. `field(name, type)` builds field descriptors.
- [x] **Comptime assertions**: `@ comptime assert(expr)` evaluates at compile time,
  exits build on failure. Handles Bool and Constructor("true"/"false") values.
  Assert expressions are not emitted as runtime code.

**Test:** `comptime_test.runa` (5 demos: arithmetic, strings, comptime-vs-runtime contrast,
lookup table, assertions). `comptime_types_test.runa` (4 demos: comptime struct from
field list, enum from string list, enum with fields, function-generated type). All
type generation produces real Rust types (structs/enums) that compile and run. ✅

## M10: Mutable Value Semantics (Hylo's Lesson)

**The "mutation is safe when you're alone" milestone.** Extends escape analysis
with independence tracking — if you're the only owner, mutation is zero-cost.

- [x] **Independence analysis**: `collect_aliased_vars` tracks which variables alias
  non-Copy sources (`= y = x`). Wired into both main and function body emission.
  Combined with consuming-use counting: aliased sources get cloned, independent values move.
- [x] **`inout` parameters**: `> sort(xs: inout List(Int))` mutates in place when independent
- [x] **Automatic promotion**: single-owner values auto-promote to mutable in Rust output
- [x] **Copy-on-write**: `inout` on `shared T` params automatically uses
  `Arc::make_mut` at call site — shared values transparently copy before
  mutation. Function signature unwraps Arc, caller uses copy-on-write.
- [x] **Benchmark parity**: `benchmark_test.runa` — 5 demos showing Futuruna emits hand-written
  quality Rust. Inout → `&mut Vec`, accumulators → `let mut` + assignment (works in
  function bodies, nested if/for), borrow inference → `&T` for read-only params with
  `for &x in xs` auto-deref, single-use → move (no clone), `push()` on `&mut Vec` →
  direct `.push()` call.

**Test:** `benchmark_test.runa` — 5 demos: in-place mutation via inout, function-level
accumulators (sum, product, count_positive with nested if), single-use moves,
borrow inference, main-level accumulators. DONE — emitted Rust matches hand-written
quality. `inout_test.runa` also passes (5 demos). All 12 .runa tests pass. ✅

## M11: Content-Addressed Modules (Unison's Lesson)

**The "identity is structure, not names" milestone.** Functions are addressed by
AST hash. No dependency conflicts. No build system. Identity is structure, not names.

- [x] **AST hashing**: every `>` and `#` definition gets a content hash (SHA-256,
  12 hex chars). `runa --hashes file.runa` displays all definition hashes.
  Name-independent: same body → same hash regardless of function name.
- [x] **`@ import #hash from ./module`**: import by hash for exact dependency
  resolution. Works in both interpreter and transpiler. No import-cycle caching
  (each hash import re-parses the file to find the matching definition).
- [x] **Name registry**: `runa registry file.runa` generates `.registry.json`
  mapping human-readable names to content hashes (like DNS for code).
  Registry files accumulate across modules. Names resolve to exact
  structural identities.
- [x] **Incremental compilation**: `runa --build` hashes generated Rust code
  and caches binaries in `runa-cache/`. Unchanged code skips recompilation.
  Hash comparison is at the whole-program level — any definition change
  triggers rebuild, unchanged programs use cached binary instantly.
- [x] **Distribution**: registry files (`.registry.json`) + source files +
  `@ import #hash from ./module` enable exact dependency resolution.
  No versioning needed — identity is structure. Share code as hash +
  dependency tree.

**Test:** Two Futuruna files (`hash_math.runa`, `hash_alt.runa`) with overlapping function names
but different bodies. `hash_test.runa` imports specific functions by hash from both files.
`factorial` has same hash in both files (identical body — Unison property). `square` has
different hashes (different body). Native binary output matches interpreter. ✅

## M12: Reactive Streams — Cold Pipelines (Native Dataflow)

**The "the language IS the reactive graph" milestone.** Not a library — the
compiler sees the stream topology, optimizes it, and the same S_τ equation
that governs causal entropic forces governs the dataflow in Futuruna programs.

- [x] **`~` stream binding**: `~ clicks = events("button", "click")` declares
  a reactive stream. `~` is to time what `=` is to a moment.
- [x] **`|>` pipe operator**: `stream |> map(f) |> filter(p) |> take(n)`
  builds the transformation graph. Left-associative, composes linearly.
  Desugars: `x |> f` → `f(x)`, `x |> f(y)` → `f(x, y)`.
- [x] **20 stream operators**: `map`, `filter`, `scan`, `merge`, `zip`,
  `take`, `skip`, `distinct`, `flat_map`, `sum`, `any`, `all`,
  `last`, `window`, `enumerate`, `count`, `collect`, `combine_latest`,
  `from_list`. Vec-based synchronous implementation in both interpreter and
  Rust codegen.
- [x] **`~ stream | x -> { }` subscription**: same syntax as iterables,
  iterates over stream elements. Escape analysis handles multi-use cloning.

**Test:** `reactive_test.runa` — 8 demos: pipe operator, map/filter/scan,
merge/zip, pipeline composition, skip/distinct/flat_map, aggregations (sum/any/
all/last), window/enumerate. Interpreter and compiled output byte-identical.
All existing .runa programs unaffected (0 regressions). ✅

**Design:** `research/language-design/reactive-design.md`

## M13: Scopes, Subjects & Lifecycle (Hot Streams)

**The "streams that live and die" milestone.** Cold pipelines (M12) process
data. Hot streams push events. Scopes control when streams start and stop.
This is where Futuruna becomes a reactive application framework — not just a
pipeline language.

**Design:** `research/language-design/reactive-subjects-lifecycle.md`

### M13a: Scope Execution

`| scope` blocks are parsed (since M12) but the interpreter ignores them.
Make them real: execute the body, provide a child environment, enable
scope-qualified access.

- [x] **Interpreter: execute scope body** — `Rule::Scope { name, body }` runs
  body statements in a child `Env`. Bindings inside are local. Functions
  defined with `>` inside `{ }` blocks work (Op(">") handling added).
- [x] **Scope-qualified access** — `WeatherApp.location` accesses a binding
  from inside a named scope. Dot access on `Value::Scope` looks up bindings.
- [x] **Nested scopes** — `| scope App { | scope Panel { ... } }` works.
  Inner scopes stored as `Value::Scope` in parent scope's bindings.
  Chained access: `App.Header.title` works.
- [x] **`teardown(ScopeName)`** — explicit scope destruction from outside.
  `@ teardown("ScopeName")` removes the scope binding from the environment.
  After teardown, scope fields are inaccessible (name resolves to unbound).
  Works via effect system — returns `__Teardown` marker that the interpreter
  handles with `env.remove()`.

**Test:** `scope_test.runa` — 6 demos: basic scope execution, nested scopes
with chained access, subjects with push/latest, subjects as streams with
pipe operators, streams inside scopes, functions inside scopes. All pass.
`lifecycle_test.runa` — 6 demos: teardown (scope removal), complete (subject
termination), error (stream error), replay subjects, actor `.state` access,
nested scope teardown. All pass. ✅

### M13b: Subjects (Push Streams)

Subjects are streams you can push into. They unify with actors: a subject IS
an actor with no logic, it just forwards. `<-` works on both.

- [x] **`subject()`** — hot stream with no initial value (like RxJS Subject).
  `Value::Subject(Vec<Value>)` — accumulates pushed values.
- [x] **`subject(val)`** — with initial value (like BehaviorSubject).
  `.latest` property returns most recent value. `.count` returns length.
- [x] **`subject(val, replay: n)`** — replay subject: buffers last n values for
  late subscribers (like ReplaySubject). In sync interpreter, all values are
  buffered (replay is implicit). Replay count metadata preserved for async/codegen.
- [x] **`stream <- val`** — push a value into a subject. Same `<-` operator
  as actor sends. One mental model. Subject items grow on each push.
- [x] **Subjects as streams** — all 20 stream operators (`map`, `filter`,
  `scan`, `|>`, `for x in subject`, etc.) work on subjects. Subjects
  ARE streams.
- [x] **`as_stream(subject)`** — type narrowing: `Subject(T)` → `Stream(T)`.
  Strips write access. `<-` on a `Stream` fails (not a Subject). This is
  how modules export read-only views of internal subjects (see M3b).
  Derived streams (`subject |> map(f)`) are already `Stream(T)` —
  `as_stream` is only needed to narrow the subject itself.
  Interpreter: `Subject(items)` → `Stream(items)`. Codegen: `.clone()`
  (Vec-based sync mode; tokio mode will strip `tx` handle).
- [x] **`complete(s)` / `error(s, e)`** — stream termination. `complete(subject)`
  converts Subject→Stream (strips write access, no more pushes). `error(subject, msg)`
  terminates with error message and converts to Stream. Both registered as builtins
  with full interpreter implementation.
- [x] **Actor-subject unification** — actors expose `.state` field for current state
  (actor-subject symmetry). Actors respond to `<-` (same as subjects). `scan` on a
  message stream IS an actor pattern. Both actors and subjects are first-class values
  with push semantics via `<-`.

### M13c: Scoped Lifecycle (Rust's Drop for Streams)

The killer feature: streams die when their scope dies. No manual unsubscribe.
No memory leaks. Rust's `Drop` does what RxJS needs `takeUntil` hacks for.

Scopes are **opaque from outside the module** (see M3b). Within the enclosing
module, scope-qualified access works (`Active.feed` is a stream that emits when
the scope is alive and goes silent when it's not). The module author wires scope
internals to exported streams; consumers never think about lifecycle.

```tau
> module Dashboard {
    ~ data = subject(0)                          -- private subject

    | scope Active(when: connected) {
        ~ feed = as_stream(data) |> map(transform)
    }

    @ export ~ output = Active.feed              -- stream, quiet when inactive
    @ export > push(v) { data <- v }
}
```

- [x] **Scope-owned streams** — `~` bindings inside `| scope` register with
  the scope's lifecycle. Scope exit → all streams torn down. Subscriptions
  (`~ subject | x -> { }`) inside scopes are tracked by the scope guard.
- [x] **Tokio emission** — `~` subjects emit `tokio::sync::broadcast::channel`.
  `for x in subject` emits spawned subscriber task (`tokio::spawn` +
  `while let Ok(x) = rx.recv().await`). `| scope` emits `_ScopeGuard` struct
  with `Drop` that aborts all subscription handles. Tokio dependency
  auto-added when subjects have async subscribers. `#[tokio::main(flavor =
  "current_thread")]` for deterministic ordering. `yield_now().await` after
  each send ensures subscribers process values in order.
  Programs without async subscribers stay synchronous (no tokio).
- [x] **`poll(fn, ms)`** — registered as builtin. Sync interpreter: calls fn
  once. Async codegen: emits interval + fn call (sync fallback for v1).
- [x] **`take_until(signal)`** — registered as builtin. Sync interpreter:
  returns stream unchanged. Async codegen: pass-through (scopes handle
  lifecycle automatically in practice).

**Test:** `lifecycle_async_test.runa` — 3 demos: subject push + async
subscription (values received after each send), scoped lifecycle (monitor
receives values then teardown stops it — post-teardown sends not received),
multiple subscribers on same subject (logger + doubler both receive, selective
teardown removes one while other continues). Compiled output uses
`tokio::sync::broadcast`, `tokio::spawn`, `_ScopeGuard` with `Drop`. ✅

### M13 Future (not blocking)

- [x] **Timing operators**: `debounce(ms)`, `throttle(ms)`, `delay(ms)`,
  `buffer(ms)`, `timeout(ms)` — completed in M17
- [ ] **Error handling on streams**: `catch_error(f)`, `retry(n)`

**Test:** `weather_demo.runa` — the first Futuruna program combining ALL 7 runes:
`#` ADTs (Weather, Condition, Severity, City, Alert), `>` functions (mock_feed),
`|` default logic with Catala-style exceptions (heatwave/coldsnap/gale override
conditional rules which override unconditional default), `=` bindings,
`~` subjects + streams with `|>` pipes (map, filter, count),
`| scope WeatherStation { }` with lifecycle, `?` verification (3 invariants
proven at runtime), `@` effects (print). All 7 readings produce correct alerts.
Conditional defaults fire before unconditional defaults (Catala semantics). ✅

## Priority Order

M1 is non-negotiable — without structs, for loops, and one-step build, Futuruna
is a demo, not a language. M2 makes it usable for real error-handling code.
M3a makes it multi-file. M3b completes the module system with TypeScript-style
visibility (private by default, `@ export`). M4 opens deployment. M5 and M6
deliver on the promise of being *better* than Rust, not just simpler.

M7-M11 are the **"learned from others"** milestones — each absorbs a major
insight from another language:
- M7 (Koka): effects as values — unifies IO, errors, async, state
- M8 (Gleam): monadic sugar — eliminates callback pyramids
- M9 (Zig): comptime — eliminates macros, shrinks escape hatch
- M10 (Hylo): mutable value semantics — mutation's speed, value's safety
- M11 (Unison): content-addressed code — identity is structure

M12 is the cold reactive foundation — pipelines, operators, `|>` composition.
M13 is where it becomes real — hot streams, scoped lifecycle, the Weather App.
M3b + M13 together solve the Subject/Observable problem: modules control
visibility (`@ export`), `as_stream()` narrows write access, scopes are
internal lifecycle machinery opaque to consumers. Three orthogonal axes.

The escape hatch (`@ rust { }`) means we can ship each milestone incrementally.
Anything Futuruna can't do yet, the programmer drops to Rust. Each milestone shrinks
the need for the escape hatch.

## M14: Standard Library — Shrink the Escape Hatch

**The "write real programs without `@ rust {}`" milestone.** The MiFIR exercise
showed that ~80% of a real-world program was `@ rust {}`. Each sub-milestone
wraps Rust capabilities in clean Futuruna APIs so programmers never see Rust.

**Strategy:** Builtins for universal primitives (string, file I/O), `.runa` stdlib
modules with internal `@ rust {}` for ecosystem wrappers (HTTP, DB, JSON).

### M14a: String Operations (Builtins)

The most universal gap. Every real program needs string manipulation beyond
`show()` and `+` concatenation. Added as compiler builtins (interpreter + codegen).

- [x] **`split(s, sep)`** → `List(String)` — split string by separator
- [x] **`join(parts, sep)`** → `String` — join list of strings with separator
- [x] **`trim(s)`** → `String` — remove leading/trailing whitespace
- [x] **`contains(s, sub)`** → `Bool` — substring test
- [x] **`starts_with(s, prefix)`** → `Bool` — prefix test
- [x] **`ends_with(s, suffix)`** → `Bool` — suffix test
- [x] **`replace(s, old, new)`** → `String` — replace all occurrences
- [x] **`to_upper(s)` / `to_lower(s)`** → `String` — case conversion
- [x] **`substring(s, start, end)`** → `String` — extract substring by index
- [x] **`char_at(s, idx)`** → `String` — single character by index
- [x] **`index_of(s, sub)`** → `Int` — find substring position (-1 if absent)
- [x] **`format_float(x, decimals)`** → `String` — format with precision
- [x] **`parse_int(s)`** → `Int` — string to integer
- [x] **`parse_float(s)`** → `Float` — string to float
- [x] **`string_chars(s)`** → `List(String)` — explode into characters

All 15 builtins work in both interpreter and compiled mode. User-defined
functions with the same names shadow builtins (e.g., `parse_int` returning
`Result` instead of the builtin's unwrap-or-0 behavior).

**Test:** `tests/string_stdlib_test.runa` — all 15 operations with edge cases. ✅

### M14b: File I/O (Builtins)

Basic file operations. No crate dependencies — uses Rust's `std::fs`.
Invoked with `@` rune (effect marker): `@ write_file(path, content)`.

- [x] **`read_file(path)`** → `String` — read entire file
- [x] **`write_file(path, content)`** → `()` — write/overwrite file
- [x] **`append_file(path, content)`** → `()` — append to file
- [x] **`file_exists(path)`** → `Bool` — check file existence
- [x] **`read_lines(path)`** → `List(String)` — read file as lines
- [x] **`env_var(name)`** → `String` — read environment variable

All 6 builtins work in both interpreter and compiled mode. I/O builtins
route through `@` (effect) in both parse paths and codegen.

**Test:** `tests/string_stdlib_test.runa` — combined with M14a tests. ✅

### M14c: JSON (Builtins)

JSON as compiler builtins rather than a separate module — no import needed.
JSON values represented as `String` (serialized JSON text) for simplicity.
Codegen auto-adds `serde_json` dependency. Interpreter uses `serde_json` directly.

- [x] **`json_parse(s)`** → `String` — parse and validate JSON string
- [x] **`json_get(val, key)`** → `String` — access object field (returns JSON text)
- [x] **`json_array(val)`** → `List(String)` — extract array elements
- [x] **`json_string(val)`** → `String` — extract string value (unquoted)
- [x] **`json_number(val)`** → `Float` — extract number
- [x] **`json_bool(val)`** → `Bool` — extract boolean
- [x] **`json_emit(val)`** → `String` — serialize to JSON string
- [x] **`json_object(pairs)`** → `String` — build JSON from key-value pairs

All 8 builtins work in both interpreter and compiled mode. Nested objects
supported (chain `json_get` calls). Auto-dependency injection: first JSON
builtin usage adds `serde_json = "1"` to generated Cargo.toml.

**Test:** `tests/json_test.runa` — parse, navigate, nested, extract, emit. ✅

### M14d: HTTP Builtins ✅

HTTP client and server as compiler builtins. Wraps `ureq` (client) and
`tiny_http` (server). Auto-dependency injection on first use.

- [x] **`http_get(url)`** → `String` — GET request, return body
- [x] **`http_post(url, body)`** → `String` — POST with body
- [x] **`http_serve(port, handler)`** → `()` — start HTTP server with
  `|path, method, body|` lambda handler (3 separate String args for type inference)
- [x] **`http_respond(status, content_type, body)`** → tuple for handler return
- [x] **`http_request_path(req)`** → `String` — extract request URL path
- [x] **`http_request_method(req)`** → `String` — extract HTTP method
- [x] **`http_request_body(req)`** → `String` — extract request body

**Test:** `tests/http_test.runa` — client GET. `examples/grundlov-server/` —
full server + client: 14 query types, typed JSON API, 11 end-to-end queries. ✅

### M14e: `std/db` Module ✅

Database access. Wraps `rusqlite` via auto-dep. Connection wrapped in `Rc<RefCell<>>` for escape analysis compatibility.

- [x] **`db_open(path)`** → `Db` — open SQLite database (`:memory:` for in-memory)
- [x] **`db_exec(db, sql)`** → `()` — execute DDL/DML (CREATE, INSERT, UPDATE, DELETE)
- [x] **`db_query(db, sql)`** → `List(List(String))` — query all rows, all column types
- [x] **`db_query_row(db, sql)`** → `List(String)` — query single row
- [x] **`db_insert(db, sql)`** → `Int` — insert, return last row ID
- [x] **`db_close(db)`** → `()` — close database

**Test:** `tests/db_test.runa` — in-memory DB: CREATE TABLE, INSERT, SELECT, UPDATE, DELETE, close. ✅

### M14f: Collection Builtins (Kotlin-inspired) ✅

Kotlin-inspired higher-order collection operations. All work in both interpreter
and compiled mode. Codegen uses Rust's native iterator adapters.

- [x] **`sort(list)`** → `List` — sort by string representation
- [x] **`sort_by(list, f)`** → `List` — sort by key function
- [x] **`any(list, f)`** → `Bool` — true if any element matches predicate
- [x] **`all(list, f)`** → `Bool` — true if all elements match predicate
- [x] **`find(list, f)`** → `Option` — first element matching predicate
- [x] **`flat_map(list, f)`** → `List` — map then flatten
- [x] **`zip(a, b)`** → `List(Tuple)` — pair elements from two lists
- [x] **`enumerate(list)`** → `List(Tuple(Int, _))` — index-value pairs
- [x] **`take_while(list, f)`** → `List` — take while predicate holds
- [x] **`drop_while(list, f)`** → `List` — drop while predicate holds
- [x] **`sum_list(list)`** → `Int` — sum of integer list
- [x] **`distinct(list)`** → `List` — remove duplicates (preserves order)
- [x] **`count_by(list, f)`** → `Int` — count elements matching predicate
- [x] **`partition(list, f)`** → `Tuple(List, List)` — split by predicate
- [x] **`chunked(list, n)`** → `List(List)` — split into chunks of size n
- [x] **`subscribe(stream, f)`** → `Unit` — iterate stream, apply callback

**Test:** `tests/collection_test.runa` — all 16 builtins in interpreter and compiled mode. ✅

### M14 Priority

| Sub | Module | Approach | Impact |
|-----|--------|----------|--------|
| **M14a** | String ops | Builtins | Eliminates ~40% of `@ rust {}` |
| **M14b** | File I/O | Builtins | Eliminates ~10% of `@ rust {}` |
| **M14c** | JSON | `.runa` + serde_json | Every API program needs this |
| **M14d** | HTTP | `.runa` + ureq/tiny_http | Client + server |
| **M14e** | Database | `.runa` + rusqlite | Data persistence |
| **M14f** | Collections | Builtins | Kotlin-style HOFs for lists |

## M15: Trust the Topology (Multi-Core Streams) ✓

**The "fix the codegen, not the language" milestone.** The programmer declares the dataflow
topology with `~` runes. Independent streams are causally independent (zero Φ). The runtime
schedules them across all CPU cores. No new operators, no heuristics, no annotations.

**Design insight:** We considered dataflow graph extraction, fan-in detection, entropy heuristics,
and effect-typed parallelism. All were rejected. The topology *is* the parallel execution plan.
Tokio's work-stealing scheduler handles optimal distribution. The compiler just needs to not
block it.

- [x] **Multi-threaded Tokio runtime:** Switch codegen from `current_thread` to default
  multi-threaded `#[tokio::main]`. Tokio auto-scales to `num_cpus` worker threads.
- [x] **Thread-safe DB builtins:** Upgrade `db_open` from `Rc<RefCell<Connection>>` to
  `Arc<Mutex<Connection>>`. All `.borrow()` calls become `.lock().unwrap()`. This was the
  only non-`Send` type in the entire codegen.
- [x] **Verify Send safety:** All emitted types (structs, enums, broadcast channels, JoinHandles,
  scope guards) are `Send + Sync` by construction — immutable by default, owned data, channel
  boundaries between actors.

**Why this works:** Every `~ subject | x -> { ... }` subscription already emits `tokio::spawn(async move { ... })`.
On `current_thread`, these interleaved on one core. On multi-threaded, Tokio's work-stealing
scheduler distributes them across all cores automatically. Independent streams feeding into
`zip` or `merge` naturally execute in parallel — the fan-in point is where they synchronize.

**What the programmer controls:** The shape of the stream graph. Nothing else. That's the point.

## M16: Pre-Codegen Type Checking ✓

**The "errors that make sense" milestone.** Type errors currently surface as
rustc errors on generated Rust — confusing even to experts. Pre-codegen
checking catches mistakes before codegen, reporting them as Futuruna errors
with function names and rune context.

- [x] **Undefined functions**: calling a name that's not a function, builtin, or constructor
- [x] **Wrong arity**: calling a function with the wrong number of arguments
- [x] **Undefined variables**: using a variable not in scope
- [x] **Constructor arity**: wrong number of fields in type constructors
- [x] **Builtin arity**: wrong number of args to builtins (split, show, etc.)
- [x] **Effect arity**: wrong number of args to effect operations
- [x] **Scope-aware checking**: variables in for-loops, match arms, lambdas,
  function params correctly scoped — no false positives on shadowed bindings
- [x] **Forward references**: functions and types can be used before definition
  (two-pass: collect declarations first, then check)
- [x] **Integrated into all modes**: `runa check`, `runa run`, `runa build`
  all run the type checker before codegen/interpretation
- [ ] **Undefined types**: type annotations referencing unknown type names
  (deferred — requires walking Ty nodes, not just Expr)

**Test:** All 45 existing tests pass with zero regressions. Programs with
deliberate errors (wrong arity, undefined functions, undefined variables,
wrong constructor fields) produce clear Futuruna-level diagnostics. ✅

## M17: Timing Operators + Stream Naming (M13 Future) ✓

**The "production-grade reactive" milestone.** Timing operators for reactive
streams. Clean names (no `s_` prefix) — the `~` rune already isolates stream
context, so the prefix was redundant information (d_eff collapsing from 3 to 2).

- [x] **`debounce(stream, ms)`**: suppress rapid events, emit only after quiet period.
  Sync: keeps only the last value (the debounced result).
- [x] **`throttle(stream, ms)`**: rate-limit to at most one event per interval.
  Sync: samples at proportional intervals.
- [x] **`delay(stream, ms)`**: shift stream events forward in time.
  Sync: pass-through (no real time in batch mode).
- [x] **`buffer(stream, ms)`**: collect events into time-windowed batches.
  Sync: collects all into one batch `[items]`.
- [x] **`timeout(stream, ms)`**: error if no event within deadline.
  Sync: pass-through (no real time).
- [x] **`switch_map(stream, f)`**: map + cancel previous inner subscription.
  Sync: keeps only the last inner stream result (models cancellation).
- [x] **`sample(stream, trigger)`**: emit latest value when trigger fires.
  Sync: picks values at trigger points proportionally.
- [x] **Clean naming**: all stream operators use clean names (no prefix).
  The `~` rune already isolates stream context, making the `s_` prefix redundant.

**Test:** `tests/timing_test.runa` — 8 demos: debounce (keeps last), throttle
(samples), delay (pass-through), buffer (batches), timeout (pass-through),
switch_map (last inner), sample (trigger-driven), pipeline composition. ✅

### M17b: Stream Operators — Tap, Catch, Reduce & Friends ✓

Seven new stream operators plus tuple accessors, completing the reactive toolkit.

- [x] **`tap(stream, fn)`** — side-effect observation, returns stream unchanged
- [x] **`catch(stream, fn)`** — error recovery (sync: pass-through, no errors in Vec)
- [x] **`first(stream)`** — return first element (or Unit if empty)
- [x] **`reduce(stream, init, fn)`** — terminal fold to single value
- [x] **`start_with(stream, value)`** — prepend a value to the front of a stream
- [x] **`concat(stream1, stream2)`** — concatenate two streams sequentially
- [x] **`pairwise(stream)`** — emit consecutive pairs as tuples: `[1,2,3]` → `[(1,2),(2,3)]`
- [x] **`fst(tuple)` / `snd(tuple)`** — tuple accessors (first and second element)

## M18: `runa fmt` ✅

**The "canonical formatting" milestone.** Auto-formatter for `.runa` files.
One true style, enforced by tooling.

- [x] **Line-based formatting**: normalizes indentation via brace-depth tracking
- [x] **`runa fmt file.runa`**: format in place
- [x] **`runa fmt --check`**: exit 1 if not formatted (CI mode)
- [x] **`runa fmt dir/`**: format all `.runa` files in directory (recursive)
- [x] **`@ rust { }` blocks**: reindented preserving relative structure
- [x] **Block comments**: `----` and `{- -}` preserved, indentation normalized
- [x] **Triple-quoted strings**: preserved verbatim
- [x] **Idempotent**: formatting an already-formatted file produces no changes
- [x] **Rune alignment**: consistent spacing after `#`, `>`, `|`, `=`, `~`, `@`, `?` (v2)
- [x] **Operator spacing**: normalize `a+b` → `a + b`, two-char ops (==, !=, ->, |>, etc.) (v2)
- [x] **Lambda-aware**: `|x|` and `|x, y|` lambda syntax preserved (not treated as `|` rune)

## M19: LSP + Editor Integration ✅

**The "IDE experience" milestone.** The VS Code extension has syntax highlighting
but no intelligence. LSP brings go-to-definition, error squiggles, and completions.

- [x] **`runa lsp`**: Language Server Protocol implementation (JSON-RPC over stdio, zero dependencies)
- [x] **Diagnostics**: real-time parse errors + type errors (M16) as editor squiggles
- [x] **Go-to-definition**: jump to function/type declarations (text-based heuristic using rune prefixes)
- [x] **Hover**: function signatures + builtin docs in markdown
- [x] **Completions**: rune-aware snippets + user symbols + ~70 builtins
- [x] **VS Code extension**: `vscode-languageclient` integration (`editors/vscode/extension.js`)

## M20: Async Stream Operators + Stream Fusion ✅

**The "real reactive" milestone.** Stream operators (`map`, `filter`, `scan`, etc.)
now work on async broadcast subjects, and sync pipelines are fused for zero intermediate allocations.

### Async Stream Operators
- [x] **`map(subject, f)`**: transform values via spawned forwarding task + new broadcast channel
- [x] **`filter(subject, f)`**: filter with async predicate evaluation
- [x] **`scan(subject, init, f)`**: running accumulator over async stream
- [x] **`take(subject, n)`**: first n values then stop forwarding
- [x] **`skip(subject, n)`**: skip first n values
- [x] **`tap(subject, f)`**: side-effect observation, passthrough
- [x] **`merge(s1, s2)`**: merge two async subjects into one channel
- [x] **Pipe chains**: `subject |> map(f) |> filter(g)` chains correctly (recursive detection)
- [x] **Type inference**: broadcast channels annotated with `i64` for Rust type inference
- [x] **Async mode detection**: subjects + any StreamSub triggers async runtime

### Stream Fusion
- [x] **Fused iterator chains**: `data |> map(f) |> filter(g) |> map(h)` compiles to single
  `.into_iter().map(f).filter(g).map(h).collect()` — zero intermediate Vecs
- [x] **Fusible ops**: map, filter, take, skip, flat_map, take_while, drop_while
- [x] **Chain detection**: recursive AST walk identifies 2+ chained fusible operations
- [x] **Correctness**: fused and non-fused produce identical results

## M21: `runa audit` — Automated Gap Discovery

**The "trust the topology" milestone for verification.** `runa audit` discovers invariant
gaps, rule asymmetries, and normative tensions automatically — without the user writing
any `?` proofs or `|` invariants. It reads the rule topology and reports what falls out.

- [x] **Rule collection**: parses all files including imports, registers all `|` rules
- [x] **Zero-arg evaluation**: evaluates all zero-arg Bool rules via the interpreter
- [x] **Entity prefix detection**: groups rules by entity (congress, president, states, etc.)
- [x] **Symmetric pair analysis**: finds same suffix across different entities, reports asymmetries
- [x] **Power without enforcement**: detects `X_can_Y = True` with no corresponding check/restriction
- [x] **Paradox detection**: finds `X_can_Y` / `X_cannot_Y` contradictions
- [x] **Automatic tension discovery**: concept-overlap analysis with normative direction (grant vs. restrict vs. duty)
- [x] **Unpaired entity rules**: detects rules for one entity with no counterpart for the natural pair
- [x] **Severity ranking**: findings ranked 0-100, sorted most interesting first
- [x] **Grouped output**: Paradoxes > Tensions > Asymmetries > Gaps > Consistent
- [x] **Rule chain display**: each finding shows the involved rules and their evaluated values
- [x] **Proof suppression**: `?` proofs in the source file are silenced during audit (clean output)

**Result on US Constitution (215 rules):** 51 interesting findings — 2 tensions, 1 asymmetry, 48 gaps — all discovered automatically from rule names and truth values alone.

---

## M22 — Package Manager ✅

**Goal:** Project scaffolding and local dependency management via `runa.toml`.

- [x] **`runa init [name]`** — scaffold new project: creates `runa.toml` + `src/main.runa`
- [x] **`runa.toml` manifest** — `[package]` (name, version, entry) + `[dependencies]` sections
- [x] **`runa add <path>`** — add local path dependency to `runa.toml` (with duplicate detection)
- [x] **Manifest-aware imports** — `@ import dep_name/module` resolves via `runa.toml` dependency paths
- [x] **Resolution search order** — tries `dep_path/module.runa`, then `dep_path/src/module.runa`
- [x] **Unified resolution** — interpreter, type checker, and codegen all use manifest-aware import
- [x] **Walk-up search** — `runa.toml` found by walking up from source directory (like Cargo.toml)
- [x] **Relative path computation** — `runa add` stores relative paths from manifest to dependency
- [x] **Git dependencies** — `runa add https://github.com/user/repo` clones to `~/.cache/futuruna/deps/`
- [x] **Git TOML format** — `dep = { git = "https://...", rev = "abc123" }` with optional rev pinning
- [x] **Shallow clone** — `--depth 1` for fast initial fetch
- [x] **Auto-fetch** — unpinned git deps fetch latest on each resolution
- [x] **Pattern exhaustiveness** — type checker detects non-exhaustive match expressions
- [x] **Missing variant reporting** — lists exactly which variants are uncovered
- [x] **Wildcard/variable escape** — catch-all patterns (`_`, `x`) skip the check
- [x] **Prelude types** — Option (Some/None), Result (Ok/Err), Bool (True/False) tracked

## M23 — Logic Programming (Datalog+) ✅

**Goal:** Complete Prolog-style logic programming within the `|` rune — facts, rules,
backtracking, negation, wildcards, and solution collection. The `|` rune becomes a full
Datalog+ engine while staying clean with the other six runes.

- [x] **Relational facts** — `| parent("alice", "bob")` as ground truth assertions
- [x] **Value-returning facts** — `| capital("Denmark") -> "Copenhagen"` (lookup tables, `Option<T>` return)
- [x] **Conjunction** — `| ancestor(a, b) -> parent(a, mid), ancestor(mid, b)` with comma-separated goals
- [x] **Existential search** — unbound variables in goals trigger backtracking over fact tables
- [x] **Negation as failure** — `not(goal)` in rule bodies; `| safe(x) -> not(dangerous(x))`
- [x] **Wildcard `_`** — anonymous variable in heads/bodies; `| childless(p) -> not(parent(p, _))`
- [x] **`findall(var, goal)`** — collect all solutions into a list; bridges `|` logic to `=` data
- [x] **Rule body `=` binding access** — rules reference outer literal bindings (auto-promoted to scope)
- [x] **Type propagation** — params inferred from Prolog call targets (`safe(x)` → `x: &str` from `dangerous`)
- [x] **Integer/float value facts** — `| port("http") -> 80` returns `Option<i64>`
- [x] **Codegen: fact tables** — `const PARENT_FACTS: &[(&str, &str,)]` with inline iteration
- [x] **Codegen: wildcard scans** — `parent(p, _)` → `PARENT_FACTS.iter().any(|f| f.0 == p)`
- [x] **Codegen: findall iteration** — `findall(c, parent("bob", c))` → `.filter().map().collect()`
- [x] **Codegen: `length` builtin** — `length(list)` → `.len() as i64`
- [x] **Aggregation via composition** — `length(findall(...))` for counting solutions

## M24 — Map and Set Collections ✅

**Goal:** Native `Map(K, V)` and `Set(T)` types with full builtin support — the critical
data structures needed for self-hosting the compiler (symbol tables, name tracking, dedup).

### Map builtins (11 functions)
- [x] **`map_new()`** — create empty map
- [x] **`map_insert(m, k, v)`** — return new map with k→v added (immutable semantics)
- [x] **`map_get(m, k)`** — return `Option<V>` (Some or None)
- [x] **`map_get_or(m, k, default)`** — return V directly, with default if missing
- [x] **`map_contains(m, k)`** — return Bool
- [x] **`map_remove(m, k)`** — return new map without k
- [x] **`map_keys(m)`** — return List of keys
- [x] **`map_values(m)`** — return List of values
- [x] **`map_entries(m)`** — return List of (K, V) tuples
- [x] **`map_len(m)`** — return Int
- [x] **`map_merge(m1, m2)`** — return new map with m2 entries overwriting m1
- [x] **`map_from(pairs)`** — create map from List of (K, V) tuples

### Set builtins (10 functions)
- [x] **`set_new()`** — create empty set
- [x] **`set_insert(s, v)`** — return new set with v added (dedup)
- [x] **`set_contains(s, v)`** — return Bool
- [x] **`set_remove(s, v)`** — return new set without v
- [x] **`set_len(s)`** — return Int
- [x] **`set_to_list(s)`** — convert to List
- [x] **`set_union(s1, s2)`** — return union
- [x] **`set_intersect(s1, s2)`** — return intersection
- [x] **`set_diff(s1, s2)`** — return s1 minus s2
- [x] **`set_from_list(items)`** — create set from List (deduplicates)

### Type mapping
- [x] **`Map(K, V)` → `HashMap<K, V>`** in generated Rust
- [x] **`Set(T)` → `HashSet<T>`** in generated Rust
- [x] **Preamble** — `use std::collections::{HashMap, HashSet}` auto-emitted
- [x] **Interpreter** — `Value::Map(Vec<(Value, Value)>)` and `Value::Set(Vec<Value>)`
- [x] **Type checker** — all 22 builtins registered with correct arities
- [x] **64/64 tests pass** — interpreter and compiled output match exactly

## M25 — Transparent Rc for Structural Sharing ✅

**Goal:** Eliminate O(n) cloning for immutable recursive ADTs by using `Rc<T>` (or `Arc<T>`
in async programs) instead of `Box<T>`. Immutability guarantees no aliasing hazards —
sharing and copying are semantically indistinguishable. Recursive ADTs are structurally
acyclic (always terminate at a base case), so Rc cycle leaks are impossible.

### Core changes
- [x] **`rc_types: BTreeSet<String>`** — detects recursive ADTs from `variant_boxed_args`
- [x] **`rc_name()`** — returns "Rc" or "Arc" based on `has_async` flag
- [x] **`pattern_is_rc_type()`** — checks if a pattern's constructor belongs to an Rc-backed type
- [x] **Type emission** — `Rc<T>` / `Arc<T>` instead of `Box<T>` for recursive fields (4 sites)
- [x] **Construction** — `Rc::new(...)` instead of `Box::new(...)`
- [x] **Pattern deref** — `(*t).clone()` instead of `*t` (O(1) via Deref + derive(Clone))
- [x] **Deep pattern guards** — `__boxed.as_ref()` instead of `*__boxed` for Rc types
- [x] **Import emission** — `use std::rc::Rc;` / `use std::sync::Arc;` when rc_types non-empty

### Properties
- [x] **O(1) structural sharing** — `derive(Clone)` on Rc-backed enums does refcount bump
- [x] **PartialEq/Debug/Display** — all work through Rc's Deref (value comparison, not pointer)
- [x] **Acyclic guarantee** — recursive ADTs always terminate at base case, no Rc cycles
- [x] **Sync/async split** — Rc for sync programs, Arc for async (existing `has_async` flag)

### Tests
- [x] **`rc_sharing_test.runa`** — 9 patterns (shared tails, fan-out, trees, persistent ops, expression trees, deep lists, nested recursive types, equality, multi-use)
- [x] **`rc_codegen_verify.runa`** — 7 verification patterns (construction, sharing, cloning, equality, append, deep recursion, multi-use)
- [x] **67/67 tests pass** — interpreter and compiled output byte-identical
