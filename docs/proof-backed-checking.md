# Proof-Backed Compiler Checking

Goal: use Futuruna's proof machinery where it reduces real compiler trust, not
where it turns the language into a general theorem prover.

Futuruna should keep a hybrid assurance model:

- proof-backed checking for small, semantics-critical compiler slices
- translation validation for passes that are easier to check than prove
- mint, canaries, differential testing, and snapshots for the rest

The proof layer is valuable when it shrinks a trusted boundary. It is a liability
when it expands the language surface without making a concrete compiler claim
more trustworthy.

## Selection Rules

A compiler pass is a good candidate for proof-backed or translation-checked
treatment when it is:

- narrow enough to model independently
- semantics-preserving by contract
- historically bug-prone or hard to test exhaustively
- close to a trust boundary where mistakes can silently validate wrong behavior
- useful for both law-style invariants and ordinary compiler correctness

A pass is a poor first candidate when it depends on broad runtime effects,
ownership side effects, async scheduling, external I/O, or emitted Rust borrow
checker behavior. Those surfaces still need strong tests, but they are bad
places to start a small trusted proof story.

## Ranked Candidate Passes

### 1. Proof elaboration and computation-lemma generation

Current status: trusted compiler machinery.

Why this is first:

- the kernel checks proof terms, but the compiler still decides which theorem is
  handed to the kernel
- computation lemmas such as `lower.sadd` are trusted equations generated from
  Futuruna function bodies
- a bug here can make the kernel prove the wrong proposition while every proof
  term still looks valid

Target:

- add an independent checker for generated computation lemmas
- validate that each generated lemma corresponds exactly to one source
  `match` arm or simple function body
- snapshot the generated lemma set for the verified bootstrap fixture and proof
  canaries

This is the first slice that directly shrinks the trusted proof boundary.

### 2. Import normalization and library boundary preservation

Current status: partially guarded by snapshots and downstream canaries.

Why it matters:

- recent downstream failures came from lost transitive imports and leaked
  imported top-level smoke code
- the pass is structural and should preserve an explicit module/export contract
- it is easier to translation-check than to prove in the kernel today

Target:

- define the source import graph, normalized module graph, and exported symbol
  set as checkable artifacts
- validate that normalized output preserves reachable exported declarations and
  excludes script-only top-level smoke code
- keep behavioral downstream canaries as the end-to-end guard

This should be translation-checked first, not kernel-proved first.

### 3. Pure expression/FIR lowering

Current status: defended by tests, typed lowering regressions, and roundtrip
execution.

Why it matters:

- many historical bugs were not parser bugs; they were lowering/type/codegen
  mismatches
- pure expressions have clear interpreter semantics and avoid async/effect
  complications
- this slice can grow from arithmetic and ADTs into lists, tuples, maps, and
  pattern matches

Target:

- define a small checked source/target model for pure expressions
- prove or translation-check representative lowering rules
- connect each proved model slice to concrete regression fixtures in the real
  compiler

This is the natural successor to the current verified bootstrap fixture.

### 4. Ownership and borrow-sensitive Rust emission

Current status: high-risk conventional codegen.

Why it is not first:

- the user-visible failures are important, but the model is Rust-specific and
  tangled with clone insertion, borrow lifetimes, and emitted code shape
- a proof-first approach would likely be too large before the smaller core
  passes are under control

Target for now:

- keep this under mint, canaries, focused regressions, and generated Rust
  compile checks
- later introduce translation validation around simple ownership-preserving
  expression fragments

### 5. Stateful streams and async scope ownership

Current status: preview runtime surface guarded by canaries.

Why it is not first:

- stream correctness includes scheduling, teardown, barriers, and task lifetime
  behavior
- these are exactly the wrong ingredients for the first small proof-backed pass

Target for now:

- keep scope-lifetime contracts explicit
- keep stateful canaries and deterministic tests broad
- only model small algebraic stream laws once runtime semantics are stable

## First Concrete Slice

The first implementation slice is:

> translation-check generated computation lemmas against the source functions
> they claim to describe.

Minimum useful version:

1. collect generated computation lemmas for a file
2. record the source function arm that produced each lemma
3. independently lower that arm into the proof-kernel term fragment
4. compare the independently lowered proposition with the generated schema
5. fail loudly if the generated lemma has no source arm, misses an arm, or
   changes the meaning of the arm

This does not prove all of Futuruna. It removes one dangerous assumption from
the proof pipeline: that compiler-generated lemmas accurately mirror source
code.

Implementation status:

- explicit proof registry construction now uses a checked computation-lemma
  collection path
- the checker rejects generated lemmas with no eligible source arm
- the checker rejects missing generated lemmas for eligible source arms
- the checker rejects generated schemas that differ from the source-derived arm
  schema
- focused regressions cover the verified bootstrap fixture and tampered
  generated lemma sets

## Non-Goals

Do not pursue these as immediate work:

- proving all Rust codegen
- making Futuruna a general-purpose theorem prover
- adding proof syntax unless a concrete compiler slice requires it
- treating Z3 success as part of the small trusted kernel story
- proving async stream scheduling before the runtime contract is fully stable

## Success Criteria

This lane is succeeding when:

- every new proof feature names the trust boundary it shrinks
- proof-related compiler machinery has independent checks or snapshots
- the verified bootstrap keeps growing through realistic compiler models
- user-facing proof claims stay honest about what is trusted
- unproved surfaces remain covered by mint, canaries, differential testing, and
  focused regressions
