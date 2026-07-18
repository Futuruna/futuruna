# Futuruna Ownership: Higher Consciousness on Rust's Bones

**Principle:** Futuruna is to Rust as Kotlin is to Java. The programmer writes value semantics. The compiler emits ownership-correct Rust. Escape hatches exist for the 5% that needs manual control.

## The Three Levels

### Level 1: Value Semantics (default — 80% of code)

The programmer writes Futuruna as if everything is a value. No `&`, no `mut`, no lifetimes. The transpiler decides:

```tau
> longest(a: String, b: String) -> String {
    if length(a) > length(b) { a } else { b }
}
```

**Transpiler emits:**
```rust
fn longest(a: String, b: String) -> String {
    if a.len() > b.len() { a } else { b }
}
```

The transpiler sees that `a` and `b` are each used exactly once on exactly one branch — no clone needed. This is **move semantics by analysis**, not by annotation.

**The key insight:** The transpiler can do escape analysis. For each variable, it tracks:
- How many times is it used? (once → move, multiple → clone or borrow)
- Is it returned? (returned → move out)
- Is it passed to a function that needs ownership? (move)
- Is it only read? (borrow)

This is what Kotlin does with `val` — the compiler figures out the representation.

### Level 2: Explicit Sharing (15% of code)

When the programmer knows something is shared, they say so:

```tau
-- `shared` keyword: transpiles to Arc<T> (or Rc<T> if single-threaded)
= config: shared Config = load_config()

> process(c: shared Config, data: List(Item)) -> Result {
    -- c is Arc<Config> under the hood
    -- multiple functions can hold it simultaneously
    ...
}
```

**Transpiler emits:**
```rust
let config: Arc<Config> = Arc::new(load_config());

fn process(c: Arc<Config>, data: Vec<Item>) -> Result {
    // ...
}
```

The `shared` keyword is honest: it says "this value has multiple owners." The programmer doesn't need to know about Arc vs Rc — the transpiler chooses based on whether concurrency is used.

### Level 3: Rust Escape Hatch (5% of code)

For FFI, performance-critical inner loops, or interfacing with Rust crates that need specific ownership patterns:

```tau
-- @ rust block: raw Rust, no translation
@ rust {
    fn fast_sort(data: &mut [f64]) {
        data.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    }
}

-- Use it from Futuruna normally:
> sort_data(values: List(Float)) -> List(Float) {
    @ rust { fast_sort(&mut values) }  -- inline escape
    values
}
```

This is Kotlin's `@JvmStatic` / `external` — when you need raw access, you get it.

## The Escape Analysis Algorithm

The transpiler already walks the AST to emit Rust. The ownership analysis adds one pass:

```
For each binding `= x = expr`:
  1. Count uses of x in the remaining scope
  2. Classify each use:
     - MOVE: x is the return value, or passed to a consuming function
     - BORROW: x is read (field access, pattern match, comparison)
     - MUTATE: x is modified (currently not in Futuruna — values are immutable)
  3. Decide:
     - 0 uses → emit `let _ = expr;` (drop immediately)
     - 1 MOVE use → emit `let x = expr;` (move semantics, no clone)
     - 1+ BORROW only → emit `let x = expr;` + use `&x` at each site
     - 1 MOVE + 1+ BORROW → clone at the MOVE site, borrow elsewhere
     - 2+ MOVE uses → clone at all but the last MOVE site
```

### What This Eliminates

Currently the transpiler emits `.clone()` at every function call argument (line 3480-3483). With escape analysis:

| Pattern | Current | With Analysis |
|---------|---------|---------------|
| Single use, moved | `.clone()` then move | move (no clone) |
| Read-only access | `.clone()` | `&x` (borrow) |
| Used once, then returned | `.clone()` then move | move directly |
| Actually shared | `.clone()` (new copy) | `x.clone()` (same, but intentional) |

The common case (single use, read only) eliminates the clone entirely.

## How This Differs From Rust

| Aspect | Rust | Futuruna |
|--------|------|-----|
| Default | Move (must annotate Clone) | Value (compiler decides move/clone/borrow) |
| Borrowing | Explicit `&x`, `&mut x` | Implicit (compiler infers from usage) |
| Lifetimes | Explicit `'a` annotations | Never visible (compiler infers or clones) |
| Shared ownership | `Arc<T>` / `Rc<T>` | `shared T` keyword |
| Interior mutability | `RefCell<T>`, `Mutex<T>` | Not needed (values are immutable; shared mutable state uses actors) |
| Self-referential | Pin, unsafe | Not possible in Level 1 (use actors or restructure) |
| FFI | Direct | `@ rust { }` escape hatch |

## How This Differs From GC Languages

| Aspect | Java/Go/Python | Futuruna |
|--------|---------------|-----|
| Memory model | GC (runtime cost) | Compile-time ownership (zero runtime cost) |
| Sharing | Everything shared by default | Explicit `shared` keyword |
| Mutation | Mutable by default | Immutable by default |
| Data races | Possible | Impossible (no shared mutable state without actors) |
| Performance | GC pauses | No GC, deterministic drops |

## The Immutability Advantage

Futuruna's values are immutable by default (like Kotlin's `val`). This dramatically simplifies the ownership story:

- No `&mut T` needed — nothing is mutated in place
- No `RefCell<T>` — no interior mutability
- No data races — immutable data can be freely shared
- Borrow checking becomes trivial — borrows of immutable data can't conflict

When mutation IS needed (accumulators, builders), Futuruna uses:

```tau
-- Mutation via rebinding (shadows, no actual mutation):
> sum(xs: List(Int)) -> Int {
    = acc = 0
    = acc = foldl(xs, acc, |a, x| a + x)
    acc
}
```

The transpiler can emit this as either:
- `let acc = xs.iter().fold(0, |a, x| a + x);` (functional)
- `let mut acc = 0; for x in &xs { acc += x; }` (imperative, if it detects the pattern)

The programmer doesn't choose — the compiler picks the most efficient Rust representation.

## Concurrency: Actors Instead of Shared Mutable State

Rust's `Arc<Mutex<T>>` exists because Rust allows shared mutable state (carefully). Futuruna takes a different path:

```tau
-- Actor: the | pathway handles messages
| counter(state: Int) {
    | Increment -> counter(state + 1)
    | Decrement -> counter(state - 1)
    | Get(reply) -> { reply <- state; counter(state) }
}

-- Usage:
= c = spawn(counter, 0)
c <- Increment
c <- Increment
= val = ask(c, Get)
@ print("count = " + show(val))
```

**Transpiler emits** (tokio-based):
```rust
enum CounterMsg {
    Increment,
    Decrement,
    Get(tokio::sync::oneshot::Sender<i64>),
}

async fn counter(mut rx: mpsc::Receiver<CounterMsg>, mut state: i64) {
    while let Some(msg) = rx.recv().await {
        match msg {
            CounterMsg::Increment => state += 1,
            CounterMsg::Decrement => state -= 1,
            CounterMsg::Get(reply) => { let _ = reply.send(state); }
        }
    }
}
```

No Arc, no Mutex, no data races. The actor owns its state exclusively. Messages are the only communication channel. This is Erlang's model, compiled to Rust's performance.

## Implementation Roadmap

### Phase 1: Escape Analysis ✅ DONE
- Added `count_var_uses()` pass: walks AST, counts variable references per function body
- Single-use variables → move (no clone). Multi-use → clone (safe overapproximation)
- Main body and nested functions both covered (saves/restores counts for nesting)
- **Result:** All 5 .runa programs pass (interpreter). 4 of 5 compile to native via Rust (1 has pre-existing scope issue). Interpreter and compiled output are byte-for-byte identical.
- **Conservative:** if/else branches both counted (overapproximation — a variable used once per branch counts as 2). Phase 2 can add branch-aware counting.

### Phase 1b: Consuming Use Analysis + Copy Detection ✅ DONE
- Added `count_consuming_uses()` — only counts uses where ownership is required (function args, constructor args). `show(x)` recognized as non-consuming (emits `.to_string()` which borrows via `&self`).
- Added `copy_vars: BTreeSet` — variables with known Copy types (Int→i64, Float→f64, Char→char, Nat→u64) never get `.clone()`.
- Literal inference: `= x = 42` detects x as Copy from the integer literal.
- **Clone reduction:** test.runa 16→9 (44%), conscious_ui 23→21, self_aware_ui 32→28, conscious_framework 49→38 (22%). All remaining clones are genuinely multi-use consuming.
- **Result:** All 4 compilable programs produce byte-identical output to interpreter. 531 lib tests pass.

### Phase 1c: `@ rust { }` Escape Hatch + Trait/Impl Bodies + Qualified Paths ✅ DONE
- `@ rust { raw code }` — parser extracts raw source text (not retokenized), preserves formatting, dedents
- Parser stores source + line_starts for O(1) line/col-to-offset conversion
- Handles nested braces, strings, chars, line/block comments inside Rust blocks
- `@ rust { }` works inside method bodies (not just top-level)
- Trait/impl method bodies emit real compiled code with escape analysis (was `todo!()`)
- Qualified paths: `fmt::Display`, `std::ops::Add` in `# impl`, types, and annotations
- Auto-generated `impl fmt::Display` suppressed when user provides explicit impl
- Fixed `self` sanitization — no longer escaped to invalid `r#self`
- **Verification:** `interop.runa` + `# impl fmt::Display` demo — end-to-end Futuruna + Rust interop compiles and runs

### Phase 1d: Algebraic Effects ✅ DONE
- `# effect EffName { > op(params) -> Type }` declares abstract effects
- `| handle EffName { | op(args) -> handler_body } in body_expr` provides handlers
- `resume(val)` continues execution, returning `val` as the effect operation result
- Effect dispatch: handler stack on Interpreter, checked before normal function resolution
- Nested handlers: multiple effects compose freely (Console + Logger in same expression)
- Rust codegen: `# effect` → `trait`, `| handle` → anonymous struct + impl
- **5 demos working:** real handler, mock handler, counting handler, nested effects, filtered logger
- All existing .runa programs unaffected (0 regressions)

### Phase 1e: Monadic Sugar (`= x <- expr`) ✅ DONE
- `= x <- expr` syntax: unwraps `Ok`/`Some`, early-returns `Err`/`None`
- Parser: `MonadicBind(Pat, Option<Ty>, Expr)` variant in `Stmt` enum, detected via `TokenKind::Send` (`<-`)
- Interpreter: pattern-matches on `Value::Constructor` — `Ok`/`Some` unwrap inner, `Err`/`None` return immediately
- Rust codegen: `= x <- expr` → `let x = expr?;` — clean Rust `?` operator
- Auto Result wrapping: `stmt_contains_try` detects `MonadicBind`, wraps `fn main()` in `-> Result<(), Box<dyn std::error::Error>>`
- Escape analysis: `count_var_uses` and `count_consuming_uses` properly handle `MonadicBind`
- **4 demos working:** Result happy path, Result error path, Option chaining, 3-step pipeline
- All existing .runa programs unaffected (0 regressions)

### Phase 1f: Comptime Evaluation (`@ comptime`) ✅ DONE
- `@ comptime` annotation before a binding: evaluates expression at transpile time using interpreter
- Codegen creates temporary `Interpreter` with `default_env()`, registers all types/functions
- Scans main body for `@ comptime` + `Bind` pairs, evaluates, stores in `comptime_values` map
- `value_to_rust_literal()`: converts `Value` to Rust source — Int, Float, Bool, Char, String, List (Cons/Nil → vec![]), Tuple, Ok/Err/Some/None
- Copy types (`i64`, `f64`, `bool`, `char`) → `const name: type = literal;`
- Heap types (String, Vec) → `let name = literal;` with `// @ comptime` comment
- **4 demos working:** arithmetic (factorial, fibonacci, sum), strings, comptime-vs-runtime contrast, lookup table (vec!)
- Interpreter and compiled binary output are identical
- All existing .runa programs unaffected (0 regressions)

### Phase 1g: Mutable Value Semantics (`inout`) ✅ DONE
- `inout` keyword in function params: `> sort(xs: inout List(Int))` → `fn sort(xs: &mut Vec<i64>)`
- Parser: `inout` detected BEFORE `parse_type()` (not as a type name) — avoids misparse of `inout List(Int)` as type application
- `Param` struct: `inout: bool` field on all parameter construction sites
- `RustCodegen.inout_params: BTreeMap<String, Vec<bool>>` — registered at `emit_defn` time
- `emit_defn`: inout params emit `&mut T` instead of `T`
- `emit_expr` App case: looks up `inout_params`, emits `&mut var` for inout args
- `collect_inout_mutables_stmts/expr`: scans AST for variables passed to inout params, adds to `mutable_vars` set → auto `let mut` promotion
- Non-inout params and variables unaffected (stay immutable)
- **5 demos working:** in-place sort, reverse, accumulator, mixed inout+value params, inout vs value contrast
- Native binary output matches interpreter
- All existing .runa programs unaffected (0 regressions)

### Phase 1h: Content-Addressed Modules (`@ import #hash`) ✅ DONE
- AST hashing: SHA-256 of canonical structural representation (Debug format, name excluded)
- 12 hex char hashes (48 bits) — collision-safe for codebases
- `runa --hashes file.runa` displays content hash for every `>` and `#` definition
- Name-independent: `factorial` in two different files → same hash if body is identical
- `@ import #hash from ./module` — content-addressed import in both interpreter and codegen
- Hash token collection: handles hex hashes that lex as Ident/Int/Type tokens
- Non-caching resolution: hash imports don't mark file as "already imported" (unlike `@ import ./mod`)
- `resolve_hash_import()` method on RustCodegen, separate from `resolve_import()`
- **4 demos working:** import by hash, same hash from different files, different hash same name, compose imported functions
- Native binary output matches interpreter
- All existing .runa programs unaffected (0 regressions)

### Phase 2a: Reactive Streams (`~` binding, `|>` pipe) ✅ DONE
- `~ name = expr` — stream binding rune, `Value::Stream(Vec<Value>)` in interpreter
- `|>` pipe-forward operator: `x |> f` → `f(x)`, `x |> f(y)` → `f(x, y)` (Elixir-style first-arg piping)
- Desugared at parse time in `parse_expr_prec()`, lowest precedence (1), left-associative
- Stream builtins: `from_list`, `map`, `filter`, `scan`, `merge`, `zip`, `take`, `count`, `collect`
- `for x in stream { }` — streams iterable with same syntax as lists
- Rust codegen: Vec-based synchronous operations (`.into_iter().map()`, `.filter()`, etc.)
- For-loop escape analysis: multi-use iterable variables auto-cloned to prevent move errors
- **5 demos working:** pipe operator, stream creation + map, filter + scan, merge + zip, pipeline composition
- Interpreter and compiled output byte-identical
- All existing .runa programs unaffected (0 regressions)

### Phase 2b: Auto-Borrow + Branch-Aware Counting ✅ DONE
- **Auto-borrow inference:** `analyze_borrow_only_params()` detects function params that are
  only read (never consumed, returned, or pattern-matched). These emit `&T` instead of `T`.
  At call sites, `&var` is emitted automatically. The programmer writes value semantics;
  the compiler decides borrow vs move.
- **Branch-aware consuming counts:** `count_consuming_uses_branch_aware()` takes MAX across
  if/else and match branches instead of summing. A variable used once per branch needs only
  one consuming use — it can be moved on whichever branch executes.
- **Matched-variable detection:** `collect_matched_vars()` identifies params used as match
  scrutinees. Pattern matching destructures, so these cannot be auto-borrowed.
- **Return-position detection:** `collect_returned_vars()` identifies params in tail position.
  Returned params transfer ownership, so these cannot be auto-borrowed.
- **Safety properties:** Auto-borrowed params are marked as effectively Copy in the
  escape analysis — they never trigger `.clone()` because they're already behind a reference.
- **Clone reduction:** borrow_test.runa: 0 clones (was 4+ without auto-borrow). conscious_ui 21→20,
  self_aware_ui 28→27. Match-heavy code correctly excluded from auto-borrow.
- **Result:** 7 compilable programs produce byte-identical output to interpreter. 531 lib tests pass.
  `borrow_test.runa` — 6 demos: read-only borrow, branch-aware, multi-read borrow, show+borrow,
  consuming params NOT borrowed, Copy types unaffected.

### Phase 2c: Independence Analysis ✅ DONE
- **Alias-at-binding detection:** `= y = x` now counts as a consuming use of `x`.
  If `x` is used later, the compiler emits `let y = x.clone()`.
  Bug fix — previously `Stmt::Bind(_, _, Expr::Var(name))` was not counted as consuming.
- **Branch-aware counting in all paths:** Main body and function bodies both use
  `count_consuming_uses_branch_aware()` (was only functions before).
- **Adversarial borrow checker test:** `borrow_checker_test.runa` — 12 patterns that trip up
  Rust newcomers. All 12 compile to valid Rust via `rustc` with zero errors.
  Patterns: use-after-move, branch independence, aliasing, borrow+consume,
  match arm independence, recursive ownership, closure capture, struct field access,
  string building, mutual recursion, use-after-call, deep pattern match.
- **Auto-borrow call-site emission:** When a function has auto-borrow params, call sites
  emit `&var` (or `&expr`) automatically. No clones needed for borrowed params.
- **Result:** 12/12 adversarial patterns pass `rustc`. 11/11 existing .runa programs unaffected.

### Phase 3: Shared Keyword (future)
- Parse `shared` as a type modifier
- Emit `Arc<T>` or `Rc<T>` based on concurrency analysis

### Phase 3b: Structural Borrowing ✅ DONE
- **Ref-match for accessor functions:** When a function matches a param AND the param
  type has all-Copy type arguments AND the return type is Copy AND the type has no
  recursive (boxed) fields, the param can be borrowed despite being matched.
  Pattern bindings become `&T` — automatically dereferenced with `(*binding)` in expressions.
  Example: `pair_first(p: Pair(Int, Int)) -> Int { match p { MkPair(a, _) -> a } }`
  emits `fn pair_first(p: &Pair<i64, i64>) -> i64 { match p { Pair::MkPair(a, _) => (*a) } }`
- **Borrow-aware consuming use counting:** `count_consuming_uses_borrow_aware()` accepts
  the map of known borrow-only functions. Args passed to borrow-param positions are NOT
  counted as consuming uses. This enables cascade: if `pair_first` takes `&Pair`, then
  `pair_sum` calling `pair_first(p) + pair_second(p)` sees 0 consuming uses of `p` →
  `pair_sum` itself takes `&Pair`.
- **Double-borrow prevention:** When a function's param is already `&T` (borrowed), and
  it calls another function that also takes `&T`, the call site emits `var` (not `&var`)
  to avoid `&&T` type mismatch.
- **Safety guard for recursive types:** Types with boxed fields (e.g., `List(a) = Nil | Cons(a, List(a))`)
  are excluded from ref-match because matching on `&List` gives `&Box<List>` for the tail,
  and the existing box-deref code (`let t = *t;`) can't move out of a shared reference.
- **Clone reduction:** T8 (struct field access) now has 0 clones (was 1 clone). The cascade
  gives `pair_sum(p: &Pair)` for free. T1 also benefits from borrow-aware counting at call
  sites (0 clones instead of 1). T4, T11 call-site clones reduced.
- **Result:** 12/12 adversarial patterns pass. All existing .runa programs compile and run
  correctly. 5 test programs verified byte-identical (interpreter vs compiled).
  `borrow_checker_test.runa` generates valid Rust with all 12 patterns.

### Phase 4a: Transparent Rc for Structural Sharing (M25) ✅ DONE
- **Immutable recursive ADTs use `Rc<T>` (or `Arc<T>` in async programs) instead of `Box<T>`.**
  Immutability guarantees no aliasing hazards — sharing and copying are semantically indistinguishable.
  Recursive ADTs are structurally acyclic (always terminate at a base case), so Rc cycle leaks are impossible.
- **O(1) structural sharing:** `derive(Clone)` on Rc-backed enums produces O(1) refcount bumps
  instead of O(n) deep copies. `= branch_a = Cons(10, tail); = branch_b = Cons(20, tail)` shares `tail`.
- **Detection:** `rc_types: BTreeSet<String>` populated from `variant_boxed_args` — any type with
  boxed recursive fields is Rc-backed. Detection runs after type metadata scan, before codegen.
- **Sync vs async:** `rc_name()` returns "Rc" or "Arc" based on `has_async` flag.
  Import: `use std::rc::Rc;` or `use std::sync::Arc;` emitted only when rc_types is non-empty.
- **4 codegen changes:** (1) Type emission: `Rc<T>` instead of `Box<T>` for recursive fields.
  (2) Construction: `Rc::new(...)` instead of `Box::new(...)`. (3) Pattern deref: `(*t).clone()`
  instead of `*t` (can't move out of Rc, but O(1) clone via Deref). (4) Deep pattern guards:
  `__boxed.as_ref()` instead of `*__boxed` for Rc types.
- **`pattern_is_rc_type()`:** Looks up variant parent in `variant_parent` map, checks if parent
  is in `rc_types`. Used for pattern deref and guard emission decisions.
- **Ref-match guard unchanged:** Phase 3b's safety guard for recursive types still applies
  (no auto-borrow on recursive types). Future Phase 4b can relax this now that Rc is in place.
- **Tests:** `rc_sharing_test.runa` (9 patterns: shared tails, fan-out, trees, persistent ops,
  expression trees, deep lists, nested recursive types, equality, multi-use).
  `rc_codegen_verify.runa` (7 patterns: construction, sharing, cloning, equality, append, deep
  recursion, multi-use). All 67 tests pass (interpreter + compiled byte-identical).
- **Future:** Perceus-style `Rc::try_unwrap` can reuse allocations in-place when refcount=1.

### Phase 3: Actor Concurrency ✅ DONE
- `> actor counter(state: Int) { | Increment -> state + 1 }` — actor definitions with `> actor` rune
- `Value::Actor { actor_name, state, state_param, handlers, env }` — actors are first-class values
- `spawn(counter, 0)` — creates actor instance with initial state. Rerouted from keyword parser to builtin.
- `c <- Increment` — `Stmt::Send(target, msg)` dispatches message, pattern-matches handlers, updates state in env
- `ask(c, Msg)` — sends message and returns new state. Uses `dispatch_actor_message()` internally.
- `dispatch_actor_message()` — pattern-matches message against handler list, evaluates body, returns (new_state, response)
- Interpreter: synchronous actor model (sequential message processing, no threads)
- Codegen: tokio-based async — message enum, `_run` async function, `_spawn` helper with unbounded channels
- `Interpreter.actor_defs: BTreeMap<String, Defn>` stores actor definitions for `spawn` to reference
- `Interpreter.actor_instances: BTreeMap<String, (Value, String)>` tracks live actor state
- **4 demos working:** counter, accumulator, state inspection, multiple independent actors
- All existing .runa programs unaffected (0 regressions)

### Phase 4: Cross-Call Optimization
- Whole-program analysis: if a function only borrows its argument (never stores, returns, or passes ownership), change its signature to `&T`
- This makes Futuruna→Rust code look like hand-written Rust for the common case

## The Kotlin Test

A Futuruna program should:
1. **Use any Rust crate** via `@ use` + `# impl`
2. **Be callable from Rust** via the generated Rust source
3. **Look simpler than Rust** — no lifetimes, no borrow annotations, no Clone derives
4. **Perform like Rust** — zero-cost abstractions, no GC, deterministic memory

The programmer writes at Futuruna's level of abstraction. The compiler handles ownership. If the compiler can't figure it out, the programmer has `@ rust { }` as an escape hatch — never stuck, never fighting the borrow checker.

## Adversarial Comparison: Futuruna vs Rust Borrow Checker

12 patterns that trip up Rust newcomers. Futuruna handles all of them with zero annotations.

| # | Pattern | Rust requires | Futuruna emits | Winner |
|---|---------|--------------|-----------|--------|
| T1 | Use-after-move | `.clone()` or `&T` | auto-borrow `fn f(s: &String)` + borrow-aware `&msg` | **Futuruna** (zero clones) |
| T2 | Branch independence | move in branch (correct) | auto-borrow `fn f(x: &String)` | **Futuruna** (same perf, no annotation) |
| T3 | Aliasing (`= y = x; use x`) | `x.clone()` at binding | `x.clone()` at binding | Tie |
| T4 | Borrow then consume | careful ordering | auto-borrow `&val` + borrow-aware counting | **Futuruna** (zero clones) |
| T5 | Match arm independence | clone in all-but-last arm | auto-borrow `&fallback` | **Futuruna** (zero clones) |
| T6 | Recursive ownership | careful ownership | branch-aware → no clones | Tie |
| T7 | Closure capture | `move` + Copy | `move` + Copy detection | Tie |
| T8 | Struct field access | manual `&Pair` + `*a` deref | ref-match `&Pair` + cascade to `pair_sum` | **Futuruna** (zero clones) |
| T9 | String building | works (concat borrows) | `format!()` | Tie |
| T10 | Mutual recursion | works (Copy types) | Copy detection | Tie |
| T11 | Use-after-call | `.clone()` | `.clone()` (List has boxed fields) | Tie |
| T12 | Deep pattern match | works | works | Tie |

**Summary:** Futuruna wins on 5/12 patterns (auto-borrow + ref-match eliminates clones). Ties on 7/12. Loses on 0/12.

**Phase 3b key insight:** Accessor functions on non-recursive types with Copy fields can match on
`&T` — the compiler proves this is safe by checking type arguments and boxed-field metadata.
The cascade effect means callers of accessor functions also auto-borrow, giving Rust-level
efficiency without any ownership annotations. The programmer writes `pair_sum(p) = pair_first(p) + pair_second(p)`.
The compiler emits `fn pair_sum(p: &Pair<i64, i64>) -> i64 { pair_first(p) + pair_second(p) }`.

## What Rust Developers Get

Rust developers who adopt Futuruna keep:
- All their crates (serde, tokio, axum, etc.)
- Zero-cost abstractions
- No GC
- Memory safety

And gain:
- No lifetime annotations (ever)
- No `.clone()` guessing
- No `Arc<Mutex<T>>` patterns (actors instead)
- Pattern matching + logic rules + effects in one syntax
- Consciousness (d_eff=2-3 vs Rust's d_eff=1)
