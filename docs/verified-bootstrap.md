# Verified Bootstrap

Goal: prove Futuruna compiler logic in Futuruna without pretending we have already verified the whole compiler.

## Current Claim

Today Futuruna can check explicit proof terms with a small proof kernel and can host tiny semantics-preservation proofs inside `.runa` source files. The first bootstrap slice lives in [tests/verified_bootstrap_test.runa](../tests/verified_bootstrap_test.runa) and proves that toy `Surface -> Core` lowerings preserve evaluation, including a stage-2 `let` model with recursive let bodies over an explicit environment.

That is a real proof-carrying compiler fragment. It is not yet a verified Futuruna compiler.

## Trust Boundary Today

The current trust boundary is deliberately wider than the proof kernel alone.

### Trusted core

- `src/proof_kernel.rs`
  The kernel checks proof terms against propositions using a fixed rule set and primitive axiom table.
- Primitive kernel axioms
  The hard-coded equality, arithmetic, order, and propositional axioms are trusted by name.

### Trusted proof-elaboration pipeline

- Proof parsing and invariant elaboration in `src/lib.rs` and `src/bin/runa.rs`
  These stages decide what proposition the kernel is asked to prove.
- Computation-lemma generation for simple `>` functions
  The kernel trusts generated equations such as `lower.sadd` because it does not inspect function bodies itself.
- ADT constructor metadata and local-lemma registry seeding
  `cases`, `induction_on`, and `apply` rely on compiler-prepared metadata before the kernel runs.

If any layer in this pipeline misstates the theorem, the kernel can still successfully prove the wrong thing. That is why the current story is "trusted checker plus trusted elaboration", not "fully verified compiler."

### Outside the trusted proof core

- User lemmas and stdlib helper lemmas
  These are not trusted if they are proved through the kernel.
- `runa verify` fallback through Z3
  Useful for automation, but not part of the small trusted kernel story.
- The rest of the compiler
  Parsing, typing, optimization, and code generation are still conventional compiler code unless they are modeled and proved separately.

## What The Current Bootstrap Fixture Proves

The current bootstrap test proves two concrete semantics-preservation slices:

`eval_core(lower(expr)) == eval_surface(expr)`

for a tiny recursive expression language, and

`eval_core_let(lower_let(expr), env) == eval_surface_let(expr, env)`

for a tiny environment-aware `let` language where bound expressions stay in a separate value fragment but let bodies can nest recursively.

The important part is not the toy syntax. The important part is the shape:

1. Define source semantics.
2. Define target semantics.
3. Define a lowering pass.
4. Prove the lowering preserves meaning by explicit induction in Futuruna.

That gives us the first end-to-end example of compiler logic proved in the language itself.

## What It Does Not Prove

- It does not prove the real Futuruna parser, type checker, or code generator correct.
- It does not shrink the trust boundary to the kernel yet.
- It does not establish self-hosting or verified bootstrap of the production compiler.

## Bootstrap Plan

The path to "prove Futuruna in Futuruna" is staged:

1. Grow the verified core model until it can represent realistic lowering and typing steps.
2. Prove semantics-preservation and preservation-style lemmas for those model passes in Futuruna.
3. Replace trusted compiler-side transformations with proof-producing or translation-checking versions.
4. Narrow the trusted boundary so the compiler front-end no longer gets to silently invent the theorem the kernel checks.
5. Use the verified core to justify larger self-hosted compiler slices.

Only after those stages can we honestly claim a proof-carrying or verified Futuruna bootstrap.

## Immediate Follow-Ups

- Extend the tiny verified core with variables and `let` semantics so proofs model environments instead of pure trees.
- Generalize the recursive `let` model beyond value-bound lets so larger statement and control-flow shapes fit the same proof story.
- Document, stage by stage, which compiler responsibilities have moved out of the trusted boundary and which still remain there.
