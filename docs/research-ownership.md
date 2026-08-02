# Invisible Ownership

Every program creates values, passes them around, changes some of them, and
eventually releases the memory behind them. A language cannot make those
questions disappear. It can only decide who has to answer them.

Rust asks the programmer and compiler to answer together. The source code says
when a value moves, when it is borrowed, when it is cloned, and sometimes how
long a reference must remain valid. Futuruna chooses a narrower source model:
the programmer writes value flow, while the compiler decides how that flow
should become ownership-correct Rust.

That is what **invisible ownership** means. Ownership is not absent. It is
present in the generated program rather than in the ordinary Futuruna syntax.

## The Memory Question

Consider a value used twice:

```runa
= clause = "A tenant may terminate the agreement"
= words = word_count(clause)
@ print(clause)
```

Several implementation questions are hiding inside these three lines:

- Does `word_count` take the string away from its caller?
- Does it borrow the string temporarily?
- Is the string copied, or is its storage shared?
- If either side can mutate it, what does the other side observe?
- When can the allocation be released?

Those are ownership questions. They matter because the wrong answer can create
a dangling pointer, a double free, a data race, or an unnecessary copy.

For a long time, mainstream languages mostly placed this burden in one of two
places. C exposes allocation and deallocation directly; C++ adds deterministic
destruction and RAII, but the programmer still reasons about pointers and
object lifetimes. Managed languages usually place more of the burden on a
garbage collector, which discovers unreachable objects while the program runs.
The first route offers direct control. The second removes a large class of
memory-lifetime mistakes. Both make a real tradeoff.

## From Rust to Hylo

[Rust](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html) made a
third route practical for general systems programming: memory is managed by a
static ownership system checked by the compiler, without requiring a tracing
garbage collector.

Rust makes the important distinctions visible. Passing a value can move it.
`&T` borrows it for reading, `&mut T` borrows it exclusively for mutation, and
`.clone()` asks for another owned value. References carry lifetimes so the
compiler can prove that they do not outlive what they point to. Most lifetime
relationships are inferred or covered by [lifetime
elision](https://doc.rust-lang.org/reference/lifetime-elision.html); explicit
annotations are needed when the relationship cannot be inferred from the
signature.

This is an excellent design for control. Rust can store references in data
structures, return borrowed views, and express low-level relationships
precisely. The cost is that ownership and borrowing become part of the
programmer's working vocabulary.

Futuruna's direct influence at this point is
[Hylo](https://hylo-lang.org/introduction/), formerly called Val. Hylo is built
around **mutable value semantics**: values behave as independent values, while
temporary access can still permit efficient in-place mutation.

The 2022 paper [*Mutable Value
Semantics*](https://research.google/pubs/mutable-value-semantics/) describes the
strict form of this idea. References are second-class: they arise implicitly at
function boundaries, but cannot be stored in variables or object fields. Hylo
uses access conventions such as
[`inout`](https://hylo-lang.org/docs/user/language-tour/functions-and-methods/)
to say that a function may temporarily modify a caller's value without turning
references and lifetimes into ordinary source-level data.

Futuruna adopts that semantic boundary and the `inout` idea. It does not adopt
Hylo's compiler or exact syntax. Futuruna takes a different implementation
route: it transpiles to Rust and infers the moves, borrows, clones, and sharing
operations that the Rust program needs.

## The Futuruna Choice

The ordinary Futuruna model rests on a few constraints:

1. **Values are the default.** A parameter describes the value a function
   receives, not a pointer shape the caller must manage.
2. **References are not source-level data.** Ordinary Futuruna code does not
   store a reference in a field or return a borrowed value tied to an input
   lifetime.
3. **Mutation across a function boundary is explicit.** An `inout` parameter
   says that the caller's value may change.
4. **Shared mutable state gets an owner.** Actors and other dedicated handles
   put mutation behind an interface instead of exposing freely aliased memory.
5. **Rust remains available at the boundary.** Interop and low-level work can
   use `@ rust {}` when the value model is not the right tool.

The first constraint gives the language its simple surface. The others are
what make that simplicity possible. Futuruna can omit lifetime syntax because
its normal data model does not let a reference escape and become a long-lived
value.

For example:

```runa
> word_count(text: String) -> Int {
    length(split(text, " "))
}

= clause = "A tenant may terminate the agreement"
= words = word_count(clause)
@ print(clause)
```

The source reads as value flow. Because `word_count` only inspects `text`, the
compiler can emit a Rust function that accepts a shared borrow and a call that
passes `&clause`. The Futuruna programmer does not choose or spell that borrow.

## What the Compiler Infers

The compiler analyzes how each binding and function parameter is used. The
result is carried into Rust generation as one of several ownership strategies:

| Source situation | Generated Rust strategy | Typical cost |
|------------------|-------------------------|--------------|
| A primitive such as `Int` or `Bool` | Copy | No allocation |
| One consuming use of an owned value | Move | No copy |
| A read-only parameter | Shared borrow, `&T` | No copy |
| An `inout` parameter | Exclusive borrow, `&mut T` | In-place mutation |
| Several consuming uses | Clone where another owner must survive | Depends on the value |
| A recursive algebraic data type | `Rc` in synchronous code, `Arc` in async code | Reference-count update on shared edges |
| An explicit `shared T` | `Arc<T>` | Atomic reference-count update |

This is more than counting textual appearances. The analysis knows which
built-ins borrow their arguments, propagates read-only parameter decisions
through calls between named functions, and treats mutually exclusive branches
independently. A value consumed once in either side of an `if` need not be
cloned merely because both branches mention it.

The analysis is conservative. When it cannot justify a borrow or a unique move,
it preserves the source-level value behavior by cloning, or by relying on a
representation that is already shared. That choice should be correct before it
is clever.

Rust then checks the generated program. This is an important division of
responsibility: Futuruna's analysis chooses a representation, while `rustc`
enforces Rust's ownership rules. Rust compilation is strong evidence of memory
safety for the emitted code, but it does not prove that every clone was optimal
or that the transpiler preserved every intended high-level behavior. Those are
separate compiler-correctness questions.

## Mutation as a Semantic Choice

Ownership and mutation are related, but they are not the same question.
Futuruna hides most ownership choices while keeping mutation visible:

```runa
> push_squares(xs: inout List(Int), n: Int) -> () {
    for i in range(0, n) {
        push(xs, i * i)
    }
}

= values = [0]
push_squares(values, 5)
```

`inout` says something useful to a human reader: this function may change
`values`. It does not ask the reader to reason about a pointer or lifetime. The
compiler lowers the parameter to `&mut Vec<i64>` and passes temporary exclusive
access at the call site.

For `inout shared T`, the generated Rust uses copy-on-write through
`Arc::make_mut`. Mutation remains local to the caller's logical value even when
the previous representation was shared. This can require a copy, but it
preserves value independence.

This is the central lesson inherited from Hylo: make the semantic effect
visible and leave the physical access mechanism to the implementation.

## When References Become Data

Not every Rust design fits a language without stored references. Futuruna does
not secretly infer arbitrary lifetime graphs. It asks the program to represent
some relationships differently.

| Reference-oriented shape | Value-oriented Futuruna shape |
|---------------------------|--------------------------------|
| Slice pointing into a document | Source value plus `Span(start, stop)` |
| Parent or cross-link in a graph | Index or stable identifier |
| Iterator borrowing a collection | Owned iterator state, or collection plus cursor position |
| Long-lived shared mutable object | Actor or purpose-built handle |
| Borrowed result tied to an input | Owned result, index, or operation performed inside the call |

A parsed document, for example, can keep its source text and store spans rather
than slices:

```runa
# Span(start: Int, stop: Int)
# Document(source: String, clauses: List(Span))
```

This removes a lifetime relationship from the type. It does not remove all
work. Spans must be validated, indices can become logically stale, reconstructing
an owned view may allocate, and an actor introduces messaging overhead. These
are representation tradeoffs, not free victories over the borrow checker.

The benefit is a firm boundary: ordinary data remains self-contained. A value
does not quietly keep another value alive through an invisible source-level
reference.

## Sharing and Concurrency

Value semantics do not require deep-copying everything.

For recursive algebraic data types, Futuruna emits reference-counted recursive
edges. Synchronous programs use `Rc`; programs with async features use `Arc`.
That makes persistent tails and subtrees cheap to share:

```runa
# IntList = Nil | Cons(Int, IntList)

= tail = Cons(30, Cons(40, Nil))
= branch_a = Cons(10, tail)
= branch_b = Cons(20, tail)
```

At the Futuruna level, `branch_a` and `branch_b` are independent immutable
values. In generated Rust, their recursive edges can point to the same
allocation. Cloning such an edge updates a reference count instead of copying
the entire tail.

This mechanism is suited to finite recursive values, not arbitrary mutable
pointer graphs. A general cyclic graph is normally represented with indexed
nodes. Code that specifically needs intrusive pointers, custom allocation, or
lock-free mutation belongs in a Rust library or a raw Rust boundary.

For concurrency, Futuruna steers shared mutation toward actors. An actor owns
its state and receives messages; callers hold a handle rather than a mutable
reference into that state. The generated runtime can still use Rust channels,
reference counts, locks, and task handles internally. The point is that those
mechanisms do not become the application's source-level ownership protocol.

## The Honest Costs

Invisible ownership is a trade, not a universal improvement over explicit
ownership.

1. **Conservative clones can cost time and memory.** Read-only inference removes
   common copies, but complex flows can still produce clones that a Rust expert
   would avoid.
2. **Reference counting is not free.** `Rc` and especially atomic `Arc` updates
   have a cost, even when they are much cheaper than a deep copy.
3. **Borrowed views are deliberately limited.** A Rust API can return a slice
   tied to its input. Ordinary Futuruna code must return an owned value, return
   coordinates, or perform the operation before temporary access ends.
4. **Some data structures change shape.** Indices and spans are often clear and
   robust, but they are not interchangeable with pointers in every algorithm.
5. **Actors trade aliasing for messaging.** That is useful for concurrent
   workflows, but excessive actor use is the wrong choice for a hot local loop.
6. **The inference is compiler machinery, not magic.** Its safety is backed by
   Rust's checks; its performance and semantic correctness still need tests,
   generated-code inspection, and benchmarks.

The `@ rust {}` escape hatch marks the edge of the model. It is appropriate for
FFI, specialized allocation, lock-free structures, unusual borrowed APIs, and
performance-sensitive integration with Rust crates. Raw Rust is still checked
by Rust, but any `unsafe` code inside it carries the same obligations as
`unsafe` code in a Rust project.

## The Claim and the Evidence

Futuruna does not claim to make every Rust ownership pattern inferable. That
would combine Rust's full expressive freedom with none of its visible
constraints, which is not a credible promise.

The narrower claim is this:

> For ordinary rule, data, and application programming, Futuruna can present a
> value-oriented language without source-level borrow syntax, then generate
> Rust that moves, borrows, clones, or shares those values as required.

Rust handles difficult lifetime relationships by making them expressible and
checkable. Futuruna handles many of them by making stored references
inexpressible in its ordinary model. What remains is represented as values,
indices, spans, shared immutable structure, actors, or explicit Rust interop.

The repository keeps this claim inspectable rather than attaching it to a
fixed test count that will immediately go stale:

```bash
# Inspect the ownership decisions in generated Rust
runa emit tests/ownership_arena.runa

# Compile and execute the test corpus
runa test --run

# Require generated Rust to pass rustc across the supported test corpus
runa test --check-codegen
```

The focused cases include `tests/ownership_arena.runa` for arena and index-based
data, `tests/ownership_self_ref.runa` for self-reference alternatives,
`tests/ownership_async.runa` for actor-oriented state,
`tests/ownership_hard_patterns.runa` for difficult value flows, and
`tests/rc_sharing_test.runa` for reference-counted structural sharing.

That evidence should grow with the language. The promise should remain the
same: simple source semantics, visible mutation, honest boundaries, and Rust
doing the final ownership check.
