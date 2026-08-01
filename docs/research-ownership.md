# Invisible Ownership

## Abstract

Rust proved that memory safety without garbage collection is achievable — but at the cost of explicit lifetime annotations, borrow syntax, and an infamously steep learning curve. We ask: can a compiler infer ownership, borrowing, and cloning automatically from value semantics alone?

We tested this by systematically attacking Futuruna's ownership inference with every pathological pattern that forces Rust into explicit lifetimes: self-referential structs, arena allocators, intrusive linked lists, async state machines with borrows across yield points, and 72 adversarial borrow-checker patterns. The result: **76 patterns compile to valid Rust with zero ownership annotations**. Three compiler bugs were discovered and fixed during the process.

This page describes the inference algorithm, presents every adversarial pattern with real code, documents the honest limits, and explains why the escape hatch exists for the remaining 1-5%.

---

## The Criticism We're Answering

Two independent reviewers raised the same concern:

> *"The escape analysis and borrow inference work for the test suite. But systems programming creates pathological ownership patterns — self-referential structs, arena allocators, intrusive linked lists, async state machines with borrows across yield points. These are the cases that forced Rust into explicit lifetimes."*

> *"While inferring &[String] over Vec&lt;String&gt; for read-only parameters is elegantly solvable, the boundary conditions of ownership — specifically cyclic data structures, self-referential structs, and complex interior mutability — are notoriously difficult to infer purely from usage without annotations."*

Both reviewers are right that these patterns are hard. Our claim is not that we solved them — it's that we **eliminated the need for them** through a different design point.

---

## The Hylo Influence

Futuruna did not invent the idea of keeping ownership and lifetime syntax out
of the programmer's way. Its clearest direct influence here is
[Hylo](https://hylo-lang.org/introduction/), formerly called Val, a systems
programming language built around mutable value semantics.

The 2022 paper [*Mutable Value
Semantics*](https://research.google/pubs/mutable-value-semantics/) describes the
central boundary: references are second-class. They can be created implicitly
at function boundaries, but they cannot be stored in variables or object
fields. Hylo applies this boundary so programs can combine value semantics with
efficient in-place mutation without exposing lifetime annotations.

Futuruna directly adopts that part of the design. Values have value semantics,
`inout` grants temporary mutable access, and references are not part of the
source-level data model. A Futuruna program cannot store a reference in a
structure or return one as a value.

From there, Futuruna takes a different implementation path. Hylo is its own
language and compiler. Futuruna transpiles to Rust, using branch-aware escape
analysis and auto-borrow inference to decide when the generated Rust should
move, borrow, or clone a value. For the remaining low-level cases, Futuruna
provides the `@ rust {}` escape hatch.

---

## The Design Philosophy

Rust and Futuruna make different tradeoffs:

| | Rust | Futuruna |
|---|---|---|
| **References** | First-class (can be stored, returned, composed) | Second-class (compiler emits them, programmer never sees them) |
| **Mutation** | Mutable by default, with careful exclusion rules | Immutable by default; `inout` for explicit mutation |
| **Shared mutable state** | `Arc<Mutex<T>>` with careful lifetime management | Actors own state exclusively; messages are the only interface |
| **Self-referential data** | `Pin<Box<T>>` + unsafe, or crates like ouroboros | Indices into flat arrays — integers are Copy |
| **Lifetimes** | Explicit `'a` annotations when the compiler can't infer | Never visible — the compiler either infers or clones |
| **When inference fails** | Programmer adds annotations | Programmer uses `@ rust {}` escape hatch |

The question becomes: **does second-class-reference programming actually work for real programs?**

---

## The Inference Algorithm

Futuruna's ownership inference has three layers that compose to produce Rust-level efficiency without annotations.

### Layer 1: Escape Analysis

For every variable in every scope, the compiler counts *consuming uses* — places where ownership would be required in Rust:

```
For each binding = x = expr:
  1. Count consuming uses of x in the remaining scope
  2. Classify:
     - 0 uses → drop immediately
     - 1 consuming use → move (zero cost)
     - 2+ consuming uses → clone at all but the last
  3. Copy types (Int, Float, Bool, Char) → never clone
```

The counting is **branch-aware**: a variable used once in the `if` branch and once in the `else` branch counts as 1 consuming use (maximum across branches), not 2. This prevents unnecessary cloning when a value is moved on whichever branch executes.

### Layer 2: Auto-Borrow Inference

For every function parameter, the compiler asks: *is this parameter only read, never consumed?*

```
analyze_borrow_only_params(params, body):
  for each param p:
    if p is never consumed AND never returned AND never pattern-matched:
      → emit as &T (borrow)
    else if ALL consuming uses are field access only:
      → emit as &T (field access works on references)
    else:
      → emit as T (owned)
```

At call sites, the compiler automatically emits `&var` for borrow-only parameters. The programmer writes `process(data)`. The compiler emits `process(&data)`.

This cascades: if function `get_x` borrows its parameter, and function `sum` calls `get_x(p) + get_y(p)`, then `sum` sees zero consuming uses of `p` — so `sum` also borrows its parameter. One level of inference propagates through the entire call graph.

### Layer 3: Ref-Match (Structural Borrowing)

Accessor functions that pattern-match on a parameter can still borrow if all destructured fields are Copy:

```runa
# Pair(fst: Int, snd: Int)

> pair_first(p: Pair) -> Int {
    match p { | Pair(a, _) -> a }
}
```

The compiler proves: `Pair` has all-Copy fields (`Int, Int`), the return type is Copy, and the type has no recursive (boxed) fields. Therefore the match can operate on `&Pair`:

```rust
fn pair_first(p: &Pair) -> i64 {
    match p { Pair { fst: a, .. } => (*a) }
}
```

The `(*a)` dereference is inserted automatically. The programmer never sees `&` or `*`.

---

## The Adversarial Patterns

We systematically tested every pattern the reviewers named, plus 60+ additional patterns from the Rust borrow-checker literature. Each pattern compiles to valid Rust, verified by `rustc`.

### Arena Allocators

**The Rust pain:** Arenas need lifetime annotations. `fn get<'a>(&'a self, idx: usize) -> &'a Node` ties the returned reference to the arena's lifetime.

**The Futuruna approach:** Index-based arena. Nodes store integer indices to children, not pointers. Integers are Copy — no lifetimes needed.

```runa
# Node(value: String, left: Int, right: Int)

> node_value(n: Node) -> String { match n { | Node(v, _, _) -> v } }
> node_left(n: Node) -> Int { match n { | Node(_, l, _) -> l } }
> node_right(n: Node) -> Int { match n { | Node(_, _, r) -> r } }

-- Build a tree as a flat array (arena pattern)
= arena = [Node("root", 1, 2), Node("left", 3, -1),
           Node("right", -1, -1), Node("leaf", -1, -1)]

-- Navigate by index — no lifetimes
= root = arena[0]
= left_child = arena[node_left(root)]
```

The tree traversal, depth calculation, and DFS all work with zero annotations. The generated Rust uses `Vec<Node>` with safe indexing — no `unsafe`, no lifetimes, no `Pin`.

**Key insight:** The arena pattern works *identically* in Rust (many Rust arena crates use index-based access). Futuruna just makes it the default — and since there are no references to worry about, the programmer doesn't need to think about why.

### Self-Referential Structs

**The Rust pain:** A struct containing both owned data and a reference into that data (e.g., a parsed document with token slices) requires `Pin<Box<T>>` + unsafe, or the ouroboros crate.

**The Futuruna approach:** Store offsets instead of references.

```runa
# Span(start: Int, stop: Int)

> get_token(source: String, spans: List(Span), idx: Int) -> String {
    = span = spans[idx]
    = start = span_start(span)
    = stop = span_stop(span)
    substring(source, start, stop - start)
}

= source = "hello world"
= spans = [Span(0, 5), Span(6, 11)]
= tok0 = get_token(source, spans, 0)   -- "hello"
= tok1 = get_token(source, spans, 1)   -- "world"
```

The same pattern handles iterators/cursors (carry `(items, position)` through function calls instead of storing `&'a Vec<T>`) and graph nodes with parent pointers (store `parent_idx: Int` instead of `*mut Node`).

**What this costs:** Reconstructing a substring from offsets is O(1) in both approaches (Rust slice indexing vs Futuruna's `substring`). The performance is equivalent.

### Doubly-Linked Lists

**The Rust pain:** Doubly-linked lists are the canonical example of data that requires `Rc<RefCell<Node>>` or unsafe raw pointers. Each node needs both forward and backward references — ownership can't flow in both directions.

**The Futuruna approach:** Flat array with prev/next indices.

```runa
# DLNode(label: String, prev: Int, next: Int)

> dl_value(n: DLNode) -> String { match n { | DLNode(v, _, _) -> v } }
> dl_prev(n: DLNode) -> Int { match n { | DLNode(_, p, _) -> p } }
> dl_next(n: DLNode) -> Int { match n { | DLNode(_, _, nx) -> nx } }

= dl = [DLNode("alpha", -1, 1), DLNode("beta", 0, 2), DLNode("gamma", 1, -1)]

-- Traverse forward
= n0 = dl[0]                    -- alpha
= n1 = dl[dl_next(n0)]          -- beta
= n2 = dl[dl_next(n1)]          -- gamma

-- Traverse backward
= n1b = dl[dl_prev(n2)]         -- beta
= n0b = dl[dl_prev(n1b)]        -- alpha
```

Forward and backward traversal with zero ownership annotations. The same approach handles intrusive linked lists (multiple index arrays over shared data, each providing a different ordering).

### Structural Sharing

**The Rust pain:** Two lists sharing a common tail require `Rc<List>`.

**The Futuruna approach:** Clone the shared data. Safe, correct, possibly more memory.

```runa
# IntList = Nil | Cons(Int, IntList)

= shared_tail = Cons(30, Cons(40, Nil))
= branch_a = Cons(10, shared_tail)
= branch_b = Cons(20, shared_tail)
```

The compiler sees `shared_tail` used in two consuming positions and emits `.clone()` for the first. Both branches get independent copies. This is semantically identical to sharing (the data is immutable) but uses more memory for large structures.

**Honest cost:** For a shared tail of n elements, this is O(n) extra memory and O(n) extra time. Rust's `Rc<List>` is O(1). For small to medium data, the cost is negligible. For large immutable trees with heavy sharing (persistent data structures), this is a real performance gap. Future work: Perceus-style reuse analysis could detect single-owner cases and avoid the copy.

### Async State Machines

**The Rust pain:** Borrows across `.await` points are the #1 source of lifetime pain in async Rust. The compiler must prove that borrowed data outlives the future that references it.

**The Futuruna approach:** Actors own their state exclusively. Messages transfer ownership. There are no borrows across yield points because there are no borrows.

```runa
> actor accumulator(total: Int) {
    | Add(n) -> total + n
    | Sub(n) -> total - n
    | Reset -> 0
}

= acc = spawn(accumulator, 0)
acc <- Add(10)
acc <- Add(20)
acc <- Sub(5)
= result = ask(acc, Add(0))     -- 25
```

The generated Rust uses tokio channels and message enums — standard async Rust, but the programmer never writes `Arc<Mutex<T>>`, never manages `Send` bounds, never fights the async borrow checker.

**Shared configuration** across multiple actors/tasks uses value copying. Since config is immutable, copying is semantically identical to `Arc<Config>` — but without atomic reference counting overhead:

```runa
# Config(url: String, retries: Int)

= cfg = Config("postgres://localhost/db", 3)
= r1 = process_with_config(cfg, 1)   -- clone
= r2 = process_with_config(cfg, 2)   -- clone
= r3 = process_with_config(cfg, 3)   -- move
```

### Higher-Order Closures

**The Rust pain:** Closures that capture non-Copy values and are used in multiple places need `Box<dyn Fn>` or careful Clone bounds. Composing closures (`compose(f, g)`) moves both into the composed closure, making the originals unusable.

**The Futuruna approach:** All closures are Clone (the compiler adds `+ Clone` to every `impl FnMut` bound). The escape analysis counts closure callee positions as consuming uses.

```runa
> make_adder(n: Int) -> Int -> Int { |x| x + n }
> make_multiplier(n: Int) -> Int -> Int { |x| x * n }
> compose(f: Int -> Int, g: Int -> Int) -> Int -> Int { |x| f(g(x)) }

= add5 = make_adder(5)
= mul3 = make_multiplier(3)
= add5_then_mul3 = compose(mul3, add5)

-- All three closures still usable:
@ print(show(add5(10)))              -- 15
@ print(show(mul3(10)))              -- 30
@ print(show(add5_then_mul3(10)))    -- 45
```

The compiler sees `add5` used twice (passed to `compose` AND called directly), emits `.clone()` for the first use, and the original survives. This pattern was a compiler bug we discovered and fixed during this research.

### Builder Pattern

**The Rust pain:** Builder patterns need `&mut self -> Self` chains or separate Builder/Built types.

**The Futuruna approach:** Functional rebinding. Each `with_*` function takes an owned struct and returns a new one.

```runa
# Request(method: String, path: String, body: String)

> with_method(r: Request, m: String) -> Request {
    Request(m, req_path(r), req_body(r))
}

= req = Request("GET", "", "")
= req = with_method(req, "POST")
= req = with_path(req, "/api/data")
= req = with_body(req, "{key: value}")
```

Each rebinding moves the previous value into the function (single consuming use → no clone). The generated Rust is identical to what a Rust programmer would write by hand.

### Recursive ADTs with Non-Copy Fields

Recursive types with String fields test the safety guard: the compiler must NOT attempt ref-match on types with boxed fields (matching on `&Box<T>` would require moving out of a shared reference).

```runa
# Expr = Lit(Int) | Add(Expr, Expr) | Mul(Expr, Expr) | Var(String)

> eval_expr(e: Expr, x_val: Int) -> Int {
    match e {
        | Lit(n) -> n
        | Add(a, b) -> eval_expr(a, x_val) + eval_expr(b, x_val)
        | Mul(a, b) -> eval_expr(a, x_val) * eval_expr(b, x_val)
        | Var(_) -> x_val
    }
}
```

The compiler correctly detects boxed fields in `Expr` (the recursive `Expr` arguments in `Add` and `Mul`), disables ref-match, and emits owned parameters. The expression tree `(x + 2) * (x + 3)` evaluates correctly at x=5 (56) and x=10 (156).

---

## The 72 Adversarial Borrow Patterns

Beyond the structural patterns above, we maintain five dedicated adversarial test files that attack the escape analysis with patterns from the Rust borrow-checker literature:

| Test File | Patterns | What It Tests |
|-----------|----------|---------------|
| `borrow_checker_test.runa` | 12 | Use-after-move, branch independence, aliasing, borrow-then-consume, match arm independence, recursive ownership, closure capture, struct field access, string building, mutual recursion, use-after-call, deep pattern match |
| `borrow_adversarial2.runa` | 12 | Same-var double-pass `f(x, x)`, closures capturing non-Copy, conditional return, loop+reuse, chained accessors, nested match, FnOnce vs FnMut, triple field access, string accumulation, Option matching |
| `borrow_adversarial3.runa` | 12 | Consume-then-use, multi-read after consume, lambda captures, for-loop ownership, match+use-after, string aliasing, multi-call same arg, nested calls, conditional consume, list of strings |
| `borrow_adversarial4.runa` | 12 | Identity consuming, strings in multiple concats, ADT with String fields, unit enum match, for-loop string use, nested consuming calls, struct multi-access, deep string ops, recursive string |
| `borrow_adversarial5.runa` | 12 | Triple aliasing, both-branch use, struct double-pass, list multi-op, nested match on ADT, HOF chain, string builder, pipeline captures, Boolean match, string foldl |

**Result:** All 60 patterns compile to valid Rust. Plus 16 patterns in the 5 ownership stress tests. **Total: 76 patterns, 0 escape hatches needed.**

### Comparison Table

| # | Pattern | Rust Requires | Futuruna Emits | Winner |
|---|---------|---------------|----------------|--------|
| T1 | Use-after-move | `.clone()` or `&T` | Auto-borrow `&String` | **Futuruna** (zero clones) |
| T2 | Branch independence | Manual analysis | Branch-aware counting | **Futuruna** (no annotation) |
| T3 | Aliasing `= y = x; use x` | `x.clone()` at binding | `x.clone()` at binding | Tie |
| T4 | Borrow then consume | Careful ordering | Auto-borrow `&val` | **Futuruna** (zero clones) |
| T5 | Match arm independence | Clone in all-but-last | Auto-borrow `&fallback` | **Futuruna** (zero clones) |
| T8 | Struct field access | Manual `&Pair` + `*a` deref | Ref-match cascade | **Futuruna** (zero clones) |
| T13 | Same-var double-pass `f(x, x)` | `.clone()` | `.clone()` | Tie |
| T14 | Closure capturing non-Copy | `move` + Clone | `move` + auto-clone | Tie |

**Summary:** Futuruna wins on 5/12 core patterns (auto-borrow + ref-match eliminates clones), ties on 7/12, loses on 0/12.

---

## Bugs Discovered

This research uncovered three compiler bugs, all fixed:

### Bug 1: Auto-Comptime on Closures

`= add5 = make_adder(5)` was incorrectly evaluated at compile time. The interpreter returned a closure value, which `value_to_rust_literal` couldn't represent, emitting `const add5: () = todo!(...)`.

**Fix:** After evaluation, check if the result can be represented as a Rust literal. Skip auto-comptime for closures, actors, and other non-representable values.

### Bug 2: Closure Callee Not Counted as Consuming

```runa
= add5 = make_adder(5)
= composed = compose(f, add5)   -- consuming use 1
@ print(show(add5(10)))          -- use after move!
```

The escape analysis counted `add5` as an argument to `compose` (1 consuming use) but did NOT count the callee position in `add5(10)` as consuming. With only 1 counted use, no clone was emitted.

**Fix:** In `count_consuming_uses_borrow_aware`, when the callee of an `App` is a variable not in the known function map (i.e., a local closure variable, not a top-level function definition), count it as a consuming use.

### Bug 3: Closures Not Clone

Even after correct counting, `.clone()` on `impl FnMut(i64) -> i64` fails — Rust's `impl Fn*` bounds don't include `Clone` by default.

**Fix:** All closure types now emit `impl FnMut(...) -> T + Clone`. This is sound because Futuruna values are always cloneable (the compiler auto-clones as needed), so all captured values in closures are Clone.

---

## The Honest Limits

### What costs more than hand-written Rust

1. **Structural sharing.** `Rc<List>` shares in O(1). Futuruna clones in O(n). For persistent data structures with heavy tail-sharing, this is a real performance gap.

2. **Shared immutable config.** `Arc<Config>` costs one atomic increment per share. Futuruna copies the entire struct. For small config objects, copying is faster (no atomic). For large objects shared thousands of times, `Arc` wins.

3. **Over-cloning in complex control flow.** The compiler takes the max count across branches, but can't prove a branch is unreachable. Some clones may be unnecessary.

### What requires `@ rust {}`

1. **Raw pointer manipulation** — inherently unsafe in any language
2. **FFI with C libraries** — external ABIs need explicit type mapping
3. **Custom allocators** — fine-grained memory control by definition
4. **Lock-free data structures** — atomic operations are inherently low-level
5. **General cyclic graphs** — back-edges create ownership cycles (index-based works for trees; raw pointer graphs need unsafe)

### What we explicitly do not handle

1. **Stored references.** You cannot create `struct { data: String, slice: &str }` in Futuruna. This is by design, not a bug. Futuruna follows Hylo's mutable-value-semantics boundary: if you cannot store a reference, you cannot create a stored-reference lifetime problem that outlives the current call.

2. **Interior mutability.** No `RefCell<T>`, no `Cell<T>`. Mutation goes through `inout` (local, scoped) or actors (concurrent, message-based). This eliminates the aliasing problem but means some Rust patterns must be restructured.

3. **Lifetime polymorphism.** Rust functions can be generic over lifetimes (`fn foo<'a, 'b>(...)`). Futuruna has no concept of lifetimes, so these patterns are handled by the compiler's choice of move vs clone vs borrow.

---

## Theoretical Connection: S_τ and Ownership

The ownership inference is, mathematically, a degenerate case of the S_τ (causal entropic force) computation that governs Futuruna's syntax design.

S_τ measures the freedom of future action from a given state. Applied to data flow:

- **S_τ = 0:** Variable is unused (dead code) → drop immediately
- **S_τ low (1 path):** Variable flows to exactly one consumer → move (zero cost)
- **S_τ medium (bounded paths):** Variable used in multiple consuming positions → clone
- **S_τ high (unbounded paths):** Variable escapes to unknown code → reference counting

The compiler's escape analysis computes S_τ with τ=1 (one-step lookahead): how many distinct future consumers does this variable have? Extending to τ>1 would enable cross-function flow analysis — each function "knows" how its values propagate through the call graph.

This is not implemented, but the theory predicts what a fully-optimized ownership inference would look like: an S_τ computation on the data flow graph, where the entropy of each variable's future determines whether it should be moved, borrowed, cloned, or reference-counted.

---

## Reproducibility

Every pattern on this page is a runnable test in the Futuruna repository:

```bash
# Run all 63 tests (including 5 ownership stress tests)
./target/release/runa test

# Verify generated Rust compiles (rustc validation)
./target/release/runa check tests/ownership_arena.runa
./target/release/runa check tests/ownership_self_ref.runa
./target/release/runa check tests/ownership_linked_list.runa
./target/release/runa check tests/ownership_async.runa
./target/release/runa check tests/ownership_hard_patterns.runa

# See the generated Rust for any test
./target/release/runa emit tests/ownership_arena.runa
```

The adversarial borrow tests are in `tests/borrow_checker_test.runa`, `tests/borrow_adversarial2.runa` through `tests/borrow_adversarial5.runa`.

---

## Conclusion

Ownership inference with unknown limits is a valid criticism of any system that claims to eliminate Rust's annotation burden. Our response is not that the limits are gone — it's that **the limits are now known, tested, and documented**.

The 76 adversarial patterns establish an empirical boundary. Everything inside that boundary — arena allocators, self-referential data, doubly-linked lists, async state machines, higher-order closures, recursive ADTs — works with zero annotations. Everything outside it — raw pointer manipulation, FFI, custom allocators, general cyclic graphs — has the `@ rust {}` escape hatch.

The honest question is not "can Futuruna handle every Rust pattern?" but "how often do you need the escape hatch?" In our test suite of 63 programs and all examples, the answer is: **zero times for ownership reasons**.

The escape hatch is for FFI and performance tuning. Not for borrow-checker fights.
