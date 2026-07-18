# Eliminating Borrow: Can Futuruna Make Ownership Invisible?

**Status:** Exploration — mapping the design space


## The Problem

Rust's borrow checker is a pain that Rust users *choose* because the alternative
(GC pauses, data races, use-after-free) is worse. Futuruna's implicit promise: you
get Rust's guarantees without the annotation burden.

Current Futuruna already eliminates ~80%:
- Escape analysis → compiler decides move/clone/borrow
- Immutable by default → no aliasing conflicts
- `inout` → safe in-place mutation without `&mut` annotations

But three pain points remain:
1. **`shared T`** — programmer must know when sharing is needed (15% of code)
2. **`@ rust { }`** — escape hatch for patterns the compiler can't handle (5%)
3. **Cycles and self-referential data** — no solution in any ownership system

Can we get to 100%? Let's map the landscape.

## What Other Languages Have Done

### Koka: Perceus Reference Counting (2021)

**Key insight:** Precise reference counting with *reuse analysis*. The compiler
inserts `drop` and `dup` operations, then optimizes:
- Single-owner values: zero overhead (same as move)
- Last-use of a constructor: *reuse* the memory for the new value (in-place update)
- Functional code that looks like copying is actually mutation under the hood

**What Futuruna could steal:** The reuse analysis. When Futuruna sees:
```tau
> map(xs: List(T), f: T -> U) -> List(U) {
    match xs {
        | Nil -> Nil
        | Cons(head: h, tail: t) -> Cons(head: f(h), tail: map(t, f))
    }
}
```
If `xs` has refcount 1, reuse the Cons cell in-place. Zero allocation for the
common case. This is *better* than hand-written Rust (which would clone or
require `&mut`).

**What it doesn't solve:** Still needs RC for the genuinely shared case. And
RC has cycle problems.

### Hylo (née Val): Mutable Value Semantics (2022)

**Key insight:** References are second-class. You can never *store* a reference.
`inout` is the only way to pass mutable access, and `inout` parameters can't
escape the function.

The Law of Exclusivity: at any point, a value is either:
- Owned by one binding (can read and write)
- Borrowed by one `inout` parameter (temporary write access)
- Projected by multiple readers (shared read access)

No lifetime annotations needed because references can't be stored.

**What Futuruna already has:** `inout` (M10). Futuruna is already 60% Hylo.

**What's missing:** Independence analysis — proving a value has no aliases.
Futuruna currently doesn't track this, so `inout` is opt-in rather than automatic.

### Lobster: Backwards Lifetime Analysis (2019)

**Key insight:** Instead of forward-propagating ownership (Rust), work backwards
from drop points. For each value, find where it's last used, insert the drop
there. Then check: can we prove it's not aliased? If yes → move. If no → RC.

This is essentially what Futuruna's escape analysis does, but Lobster goes further:
it handles cross-function analysis by specializing function bodies for different
ownership contexts (owned vs borrowed caller).

**What Futuruna could steal:** Function specialization. If `process(x)` is called
from two sites — one where `x` is the last use, one where `x` is used again
after — emit two versions: one that moves, one that borrows. Monomorphization
already does this for types; do it for ownership too.

### Mojo: Borrowed by Default (2023)

**Key insight:** Most function parameters are read-only. Make `borrowed` the
default calling convention. Only opt into `owned` or `inout` when you need it.

**Futuruna parallel:** This is what M5 (whole-program `&T` inference) plans to do.
But Mojo shows you can go further: the *default* for every parameter is borrow,
and the compiler inserts copies only when necessary.

### Lean 4: Proof-Carrying RC (2021)

**Key insight:** RC + compile-time proof that refcount is 1 → eliminate the
RC overhead. Lean's compiler uses "borrowed" annotations internally but the
programmer never sees them. The compiler proves uniqueness where possible and
falls back to RC where it can't.

**This is the synthesis Futuruna wants.**

## The Synthesis: Three-Layer Invisible Ownership

Instead of asking the programmer to choose between levels, the compiler
chooses automatically. The three levels become compiler strategies, not
programmer annotations:

### Layer A: Static Ownership (covers ~85%)

Escape analysis + independence analysis + function specialization.

The compiler proves statically that each value has exactly one owner at each
point. This covers:
- Single-use values → move
- Read-only access → borrow (compiler emits `&T`)
- Last use in scope → move to callee
- `inout`-style patterns → detected automatically when a value is modified
  and not aliased

**What's new vs current Futuruna:** Independence analysis (prove no aliases exist)
and function specialization (different callers get different ownership strategies).

### Layer B: Optimized RC (covers ~14%)

For values the compiler *can't* statically prove unique — but where the
programmer wrote nothing special:

```tau
= config = load_config()
= server = start_server(config)
= logger = start_logger(config)   -- config used twice: genuinely shared
```

The compiler sees `config` used in two consuming positions. Instead of
forcing `shared T`, it automatically inserts reference counting. With
Perceus-style reuse analysis, this has near-zero overhead for the common case.

**The key move:** `shared` becomes an *optimization hint*, not a requirement.
The compiler would choose RC anyway — `shared` just tells it "I know this is
shared, you can skip the analysis."

### Layer C: Cycle Collection (covers ~1%)

For genuinely cyclic data (doubly-linked lists, parent-child with back-pointers,
arbitrary graphs), the compiler detects potential cycles at the type level
(recursive type where a field can point back to the same type through a chain)
and uses a cycle-collecting strategy:
- Trial deletion (Python-style) on the RC'd subset
- Or: arena allocation for the cycle-containing type

The programmer writes:
```tau
# TreeNode(value: T, children: List(TreeNode(T)), parent: TreeNode(T))
```

The compiler sees the cycle (`TreeNode → parent → TreeNode`) and automatically
uses arena + indices instead of pointers. The programmer never knows.

## The S_τ Connection

Here's where entropy theory helps. The compiler's ownership analysis IS an
S_τ computation on the data flow graph:

- **Nodes** = variable bindings
- **Edges** = data flow (assignment, function call, return)
- **S_τ of a node** = how many distinct future code paths can use this value

High S_τ values → many possible futures → needs RC (shared).
Low S_τ values → one future path → move.
Medium S_τ values → known pattern → borrow.

This isn't just an analogy. The escape analysis algorithm literally computes
"how many future uses does this variable have?" — which is a degenerate case
of S_τ with τ=1 (one-step lookahead). Extending to τ>1 means:

**Cross-function flow analysis.** A variable passed to a function that stores
it in a struct that's returned to a caller that passes it to another function...
this is a random walk on the data flow graph. The entropy of that walk tells
the compiler whether the value will be shared.

The compiler doesn't need to solve the halting problem. It needs a
*conservative approximation* of S_τ on the data flow graph:
- S_τ = 0: dead code, value unused → drop immediately
- S_τ low (1 path): move semantics, zero cost
- S_τ medium (known bounded paths): borrow or clone, static analysis sufficient
- S_τ high (many/unbounded paths): RC, potentially with cycle detection

**The local-global parallel:** Locally intelligent actions are
globally good (no externalities). In Futuruna's ownership: locally optimal memory
decisions (the compiler's per-function analysis) should produce globally optimal
memory behavior (no leaks, no dangling refs, no unnecessary copies). The
compiler's escape analysis is a *local* computation; extending it with S_τ-style
flow analysis makes it *semi-global* — each function "knows" how its values
propagate through the call graph.

## What This Eliminates

| Pain point | Current Futuruna | After synthesis |
|-----------|------------|-----------------|
| `&T` / `&mut T` annotations | Never visible | Never visible |
| Lifetime `'a` annotations | Never visible | Never visible |
| `.clone()` guessing | Escape analysis | Automatic (move or RC, compiler decides) |
| `shared T` keyword | Programmer must write | Optional hint (compiler auto-detects) |
| `Arc<Mutex<T>>` patterns | Actors (M6) | Actors (unchanged) |
| Cyclic data structures | `@ rust {}` required | Auto arena or cycle-collecting RC |
| Self-referential structs | `@ rust {}` required | Auto arena with indices |
| Complex borrowing patterns | `@ rust {}` required | Function specialization + RC fallback |

**The `@ rust {}` escape hatch remains** — but only for FFI and raw
performance tuning. No ownership reason to use it.

## The Honest Limits

1. **RC overhead is real.** Even with Perceus reuse, atomic RC (for concurrent
   code) costs ~2ns per inc/dec. For hot inner loops, this matters. The compiler
   should use static ownership for the hot path and RC only for cold paths.

2. **Cycle detection has cost.** Trial deletion is O(cycle size) and requires
   a GC-like pause (albeit very short). Arena allocation avoids this but changes
   the performance characteristics (bulk deallocation, not incremental).

3. **Whole-program analysis is slow.** Cross-function S_τ computation could
   make compilation expensive. Mitigation: module-level analysis with
   conservative assumptions at module boundaries. Content-addressed modules (M11)
   help — unchanged modules keep their cached analysis.

4. **Soundness proof is hard.** Rust's borrow checker is sound by construction
   (Oxide/Stacked Borrows). A system that combines static ownership + RC + cycle
   detection needs a soundness proof for the *combination*. This is real work.

5. **Escape analysis can't see through trait objects.** Dynamic dispatch
   (`dyn Trait`) hides the concrete type and its ownership needs. Solution:
   RC for all trait objects (they're already heap-allocated in Rust).

## Innovation Futuruna Could Pioneer

### 1. Ownership-polymorphic functions

Instead of monomorphizing only on types, monomorphize on ownership:

```tau
> process(data: List(Int)) -> Int { ... }
```

The compiler emits up to 3 versions:
- `process_owned(data: Vec<i64>)` — caller gives up ownership
- `process_borrowed(data: &[i64])` — caller retains ownership
- `process_shared(data: Arc<Vec<i64>>)` — shared reference

Each call site gets the cheapest version that works. This is what Lobster does
for simple cases; Futuruna could do it systematically with whole-program analysis.

### 2. Entropy-adaptive memory strategy

Use S_τ on the module's data flow graph to choose the memory strategy at
compile time:

- Modules with low-entropy data flow (pipelines, transforms) → pure static
  ownership, zero RC overhead
- Modules with high-entropy data flow (event systems, caches, registries) →
  RC with reuse analysis
- Modules with cyclic data (graph algorithms, DOM trees) → arena allocation

The programmer never chooses. The *topology* of their code determines the
strategy. This is entropy theory's core principle applied to compilation: the structure
of the code reveals its nature.

### 3. Algebraic effects for ownership

Instead of special-casing ownership, make it an effect:

```tau
# effect Own {
    > borrow(x: T) -> T      -- temporarily access
    > consume(x: T) -> T     -- take ownership
    > share(x: T) -> T       -- create shared reference
}
```

The compiler provides a default handler that chooses the optimal strategy.
Advanced users can override with custom handlers for specific allocation
patterns (arena, pool, slab).

This probably won't work in practice (effects have runtime cost, ownership
must be zero-cost), but it's interesting to think about: ownership as an
algebraic effect with a compile-time handler.

## Concrete Next Steps

If we want to pursue this:

1. **Independence analysis** — extend escape analysis to prove "this value
   has no aliases." Required for auto-`inout` and for proving RC isn't needed.
   This is the single highest-value feature.

2. **Auto-RC fallback** — when escape analysis can't prove uniqueness, silently
   insert `Rc<T>` (single-thread) or `Arc<T>` (multi-thread). Make `shared`
   optional. This eliminates 14% of the remaining pain.

3. **Function specialization on ownership** — emit borrow vs owned versions.
   This eliminates unnecessary clones at call boundaries.

4. **Cycle detection at the type level** — scan type definitions for potential
   back-edges. Auto-arena or auto-weak-ref for those types.

5. **Benchmark against Rust** — the real test is: does invisible ownership
   produce code that's within 5% of hand-written Rust? If yes, mission
   accomplished. If not, identify the hot paths and optimize those specifically.

## The Vision

A Futuruna programmer should never think about ownership. Not because it's hidden
behind a GC (that's Go's answer — simple but costly), but because the
*compiler thinks about it for them*. The compiler has more context than the
programmer (whole-program data flow), more patience (can try multiple
strategies), and more precision (can specialize per call site).

The borrow checker isn't wrong. It's just *misplaced*. It belongs in the
compiler, not in the source code. Futuruna moves it there.
