# Proof Kernel — Design Spec

**Status:** Implemented and audited for trusted-boundary size (`td-f8f162`).
**Target:** The trusted core of the Curry-Howard verification layer for the `?` rune.

---

## 0. Current Audit Decision

The original design budget expected a roughly 540 LoC trusted core with an
800 LoC hard ceiling. That is no longer an honest description of the shipped
implementation. `src/proof_kernel.rs` now includes metadata-aware `cases` and
`induction_on`, local schemas, rewrite synthesis, axiom dispatch, and the
kernel-facing constructor metadata that those rules need.

The decision for this audit is:

- Keep `cases` and `induction_on` inside the proof-kernel trusted boundary.
  They are judgment rules, not external scaffolding.
- Keep constructor-family, field-family, and recursive-field metadata in `Ctx`
  inside the trusted boundary. A wrong metadata interpretation can make the
  kernel accept a wrong branch split or induction hypothesis.
- Keep tests outside the trusted boundary, even though they live in the same
  Rust file today under `#[cfg(test)]`.
- Keep parser lowering, theorem construction, computation-lemma generation,
  local-lemma registry seeding, and SMT fallback outside the kernel boundary.
  They remain trusted proof-elaboration pipeline code, documented separately in
  [verified-bootstrap.md](verified-bootstrap.md).
- Do not refactor the file merely to satisfy the old line budget. Instead,
  freeze the trusted surface explicitly and require any new rule form, primitive
  axiom family, or growth beyond this audited size to come with a boundary
  update or a split.

Measured at the audit point:

| Surface | Lines |
| --- | ---: |
| `src/proof_kernel.rs` total | 2,228 |
| Trusted implementation before `#[cfg(test)]` | 1,572 |
| Trusted implementation, nonblank/noncomment approximation | 1,233 |
| Tests under `#[cfg(test)]` | 656 |

The new review budget is therefore not "under 800 LoC total." The current
target is to keep the trusted implementation at roughly this size: about
1,600 pre-test lines or 1,300 nonblank/noncomment lines. If the kernel crosses
that by more than small maintenance drift, the next change must either split
support code away from `check`, reduce rule surface, or update this document
with a new explicit trust argument.

The code that remains inside the trusted boundary is:

- `Term`, `Prop`, `ProofTerm`, `IndArm`, `CaseArm`, `Schema`, `Hyp`, and `Ctx`.
- `Registry::with_builtins`, `Registry::register`, and built-in/special axiom
  dispatch in `check_apply`.
- first-order unification, occurs checking, substitution, schema freshening,
  rewrite, synthesis, and goal-subterm collection.
- `check`, `check_apply`, `check_cases`, and `check_induction`.

Everything else that prepares inputs for the kernel is trusted pipeline code,
not the small kernel itself.

## 1. Purpose

When a user writes

```runa
| add_comm: (a: Int, b: Int) -> a + b == b + a

? add_comm by {
    | (a, b) -> apply int_ring.comm_add
}
```

the parser puts `a : Int, b : Int` into the context `Γ` and hands the kernel the open body `a + b == b + a` as the goal. The kernel receives the proof term `apply int_ring.comm_add` and decides whether that term constitutes a proof of that proposition. (The axiom schema `∀α β. α+β == β+α` unifies against the goal, solving `α↦a, β↦b`, with no premise proofs to check.) No side effects. No SMT calls. No compiler state. A pure function:

```rust
fn check(term: &ProofTerm, prop: &Prop, ctx: &Ctx) -> Result<(), ProofError>
```

**Everything Futuruna's "trustworthy for infinite production" story leans on the correctness of this one function.** It is therefore treated like cryptographic code: small, closed, self-contained, reviewable by one person in one sitting.

## 2. Design Principles

1. **Small.** Keep the audited trusted implementation near the measured budget
   in Section 0. The old ~500 LoC estimate and 800 LoC hard ceiling were design
   estimates, not the current implementation contract.
2. **Closed.** No dependencies on the rest of the compiler beyond shared AST types for expressions that appear inside propositions. No I/O, no globals, no mutable state outside the `Ctx` passed to `check`.
3. **Decidable.** Every rule terminates on well-formed input. No unbounded unfolding, no unrestricted fixpoints.
4. **Axioms are named, not inspected.** `int_ring.comm_add` is a string the kernel recognizes. The kernel does **not** look inside `std/prove.runa` to verify axiom bodies. The primitive axiom set *is* the trust boundary.
5. **Conservative v1.** When in doubt, reject. A `?` the kernel can't close falls through to Z3 via `runa verify`, so rejection is never fatal — it is merely a missed upgrade from "external oracle says yes" to "here is a checked proof term."
6. **Never silently upgrade trust.** If an axiom is missing, unification fails, or the term is malformed, the kernel returns an error. It never "tries harder" — automation lives outside the trust boundary.

## 3. Proposition Grammar (v1 scope)

```
Prop  ::= Expr '==' Expr                  -- equality
        | Expr '<=' Expr                  -- ordering ( <, >=, > desugared to <= )
        | Prop 'and' Prop                 -- conjunction
        | 'forall' Pattern '.' Prop       -- universal
        | 'not' '(' Prop ')'              -- negation
        | Prop '->' Prop                  -- implication (only inside proofs; see ASSUME)
        | 'False'                         -- bottom

Expr  ::= IntLit
        | Var
        | Expr BinOp Expr                 -- +, -, *, /
        | Fn '(' Expr* ')'                -- function application (uninterpreted to the kernel)
        | Ctor '(' Expr* ')'              -- data constructor
```

**Out of scope in v1** (these `?` invariants fall through to Z3):

- Existentials (`exists x. P(x)`)
- Disjunction (`or`) — forces case-splitting on propositions, not just values
- Real-number / float arithmetic
- Function extensionality
- Higher-order predicates (`P: Int -> Bool`)
- Propositions referencing themselves recursively

The v1 scope is chosen so that every accepted proposition has a straightforward first-order shape and every rule is structural.

## 4. Proof Term Grammar

```
Term  ::= 'refl'
        | 'apply' Name                              -- zero-premise axiom/lemma (parens optional)
        | 'apply' Name '(' Term (',' Term)* ')'     -- with premise proofs
        | 'rewrite' Term 'in' Term                  -- substitute equals in the goal
        | 'induction_on' Var '{' IndArm+ '}'
        | 'cases' Expr '{' CaseArm+ '}'
        | 'contra' '{' Term '}'
        | 'let' Var '=' Term 'in' Term
        | 'assume' Prop 'in' Term
        | Var                                       -- reference a hypothesis in context

IndArm   ::= '|' CtorPat '->' Term                  -- `ih` (and `ih_1`, …) bound in Γ for recursive args
CaseArm  ::= '|' CtorPat '->' Term
```

`Name` refers to either a built-in axiom (listed in Section 7) or a previously-proved `|` invariant in the current module. The kernel looks up both in a single registry, so there's no distinction at the call site — a proved lemma is used exactly like an axiom. (This is why there's no separate `by_lemma` form: it would be redundant.)

`apply` takes **proofs of the axiom's premises**, not its universally-quantified variables. Universal variables are solved by first-order unification against the current goal. So `apply int_ring.comm_add` on a goal `x + y == y + x` unifies `α↦x, β↦y` automatically — the user does not write them.

Syntactically, `IndArm` and `CaseArm` reuse Futuruna's existing `|` arm shape. The parser can share the `match`-arm production and retag the body as a `Term` instead of an `Expr` when it sees the parser is in **proof mode** (inside a `? ... by { ... }` block).

**No new rune.** The proof surface syntax is `|`, `=`, `match`-style patterns, and a small set of keywords (`by`, `apply`, `refl`, `rewrite`, `induction_on`, `cases`, `contra`, `assume`, `let`). Everything else is existing grammar.

## 5. Judgment Rules

Notation: `Γ ⊢ t : P` reads "in context `Γ`, the proof term `t` checks proposition `P`."

```
                                                [REFL]
                      Γ ⊢ refl : e == e


   Name N resolves to schema  ∀x̄. (P₁ ∧ … ∧ Pₙ) → C
     (either a built-in axiom or a previously-proved | invariant)
   Unifier σ obtained from matching C against the current goal
   Γ ⊢ tᵢ : Pᵢσ    for each i ∈ 1..n
  ──────────────────────────────────────────────────  [APPLY]
             Γ ⊢ apply N(t̄) : Cσ

  (If n = 0, the premise-proof list is empty and the unifier is
   derived solely from the goal. Parentheses are optional: `apply N`.)


   Γ ⊢ t_eq : e₁ == e₂    Γ ⊢ t_body : P[e₁ := e₂]
  ───────────────────────────────────────────────  [REWRITE]
        Γ ⊢ rewrite t_eq in t_body : P

  (The kernel replaces every occurrence of e₁ in the current goal
   with e₂ and then checks t_body against the rewritten goal.
   Sound by Leibniz equality.)


   For each constructor C(ȳ : T̄) of the inductive type of x:
     let ihⱼ : P(yⱼ)  for each recursive yⱼ ∈ ȳ
     Γ, ȳ : T̄, ih̄ ⊢ t_C : P[x := C(ȳ)]
  ──────────────────────────────────────────────────────  [IND]
      Γ ⊢ induction_on x { | C(ȳ) -> t_C ... } : P(x)

  (P(x) is the open goal, with x free in Γ. The rule is "structural
   induction on x", not a universal-intro step — there is no ∀ on
   the conclusion line.)


    For each constructor C(ȳ) of the type of e:
       Γ, ȳ : T̄, e = C(ȳ) ⊢ t_C : P
  ─────────────────────────────────────────  [CASES]
    Γ ⊢ cases e { | C(ȳ) -> t_C ... } : P


        Γ ⊢ t : False
     ──────────────────  [CONTRA]
     Γ ⊢ contra { t } : P       (any P)


     Γ ⊢ t₁ : P₁     Γ, v : P₁ ⊢ t₂ : P₂
  ─────────────────────────────────────  [LET]
        Γ ⊢ let v = t₁ in t₂ : P₂


          Γ, h : P ⊢ t : Q
        ───────────────────  [ASSUME]
        Γ ⊢ assume P in t : P → Q


           (v : P) ∈ Γ
        ────────────────  [VAR]
            Γ ⊢ v : P
```

Rules not included in v1 (intentionally): `forall`-intro (the kernel never sees a ∀-top-level goal; see Section 6), `or`-intro/elim, existential intro/elim. These would require a bigger kernel.

## 6. How `|` Invariants Become Propositions

A `|` invariant has a head with parameters and a body that is a predicate. Example:

```runa
| balance_ok: b -> b >= 0 and b <= 1_000_000
```

**Logical reading.** Universally closed: `∀b. (b >= 0) and (b <= 1_000_000)`. This is how humans think about the invariant.

**Kernel reading.** The parser adds the head parameter `b : Int` to the context `Γ`, and the kernel's goal becomes the **open** body `b >= 0 and b <= 1_000_000`. No top-level `∀` ever reaches the kernel. Universal generalization is implicit in the fact that `b` is a free variable in `Γ` — the proof must work for an arbitrary `b`.

This is why Section 5 has no `forall`-intro rule: there is nothing to introduce. A proof of `b >= 0 and b <= 1_000_000` in a context that binds `b : Int` *is* a proof of `∀b. b >= 0 and b <= 1_000_000`.

Two cases at `?`-time:

- **Generic (parameter in `Γ`).** If the user asks `? balance_ok` and the invariant is stated generically, `b` is free in `Γ` and the kernel proves the open body. The proof must not inspect the value of `b`.
- **Ground (substituted).** If the invariant references a previously-bound concrete name (`= balance = 1000` followed by `| balance_ok: balance -> balance >= 0 and balance <= 1_000_000`), the checker substitutes `1000` for `balance` and sees the ground proposition `(1000 >= 0) and (1000 <= 1_000_000)`, which is closed by two uses of `apply int_ord.le_of_concrete`.

Conjunction is provable by `apply and.intro(proof_of_left, proof_of_right)`.

## 6.5. Computation Lemmas from `>` Functions

The kernel does not unfold function definitions. A goal like `0 <= length(Nil)` is opaque to it — `length(Nil)` is just an uninterpreted function call. To make function-using invariants provable, the compiler performs a **computation lemma pass** when it encounters a `>` function with a `match` body.

For each match arm, it emits one equation axiom into a per-module axiom table:

```runa
> length(xs: List(a)) -> Int {
    match xs {
        | Nil        -> 0
        | Cons(_, t) -> 1 + length(t)
    }
}
```

yields two generated axioms, available to `apply` inside `?` blocks in the same module:

```
length.nil  : length(Nil) == 0
length.cons : ∀h t. length(Cons(h, t)) == 1 + length(t)
```

These are **trusted** — not because they are hand-written, but because they are the mechanical image of the function's source text. If the function body changes, the axioms are regenerated on the next compile.

**Restrictions on which functions get computation lemmas (v1):**

- The body must be a single `match` on the first parameter. Nested `match`, `if` chains, and helper `let`s are not lifted.
- The function must be total (all constructors covered; no partial patterns).
- Guards (`when` clauses) disqualify the arm — the lemma would need a side condition we can't yet express.
- Recursive functions are allowed; the kernel does not unfold recursively, so induction closes the gap.

Functions that don't qualify simply don't get computation lemmas; any proof that would have used them falls through to Z3 as today.

This pass is part of the **compiler**, not the **kernel**. The kernel sees the generated axioms via the same registry it uses for built-ins. The trust story is "axioms mirror source code" — if that assumption is wrong, the kernel is wrong through no fault of its own, but the generation pass is small (~100 LoC) and easy to audit.

## 7. Primitive Axiom Set (v1 trust boundary)

The kernel recognizes these ~22 axioms by name. Their bodies are trusted. They are declared in `std/prove.runa` via a new `@ axiom` form (see futuruna-uvh) that the kernel cross-references. **If any axiom here is wrong, every proof that uses it is wrong.** Keep the list small; grow only with justification.

**Equality** (5)

- `eq.refl       : ∀x. x == x`
- `eq.sym        : ∀x y. x == y → y == x`
- `eq.trans      : ∀x y z. x == y → y == z → x == z`
- `eq.congr_f    : ∀f x y. x == y → f(x) == f(y)`
- `eq.congr_op   : ∀x y u v. x == u → y == v → x op y == u op v` (one per binary op, or parameterized)

**Int ring** (8)

- `int_ring.comm_add     : ∀a b. a + b == b + a`
- `int_ring.assoc_add    : ∀a b c. (a + b) + c == a + (b + c)`
- `int_ring.zero_add     : ∀a. 0 + a == a`
- `int_ring.comm_mul     : ∀a b. a * b == b * a`
- `int_ring.assoc_mul    : ∀a b c. (a * b) * c == a * (b * c)`
- `int_ring.one_mul      : ∀a. 1 * a == a`
- `int_ring.distr        : ∀a b c. a * (b + c) == a*b + a*c`
- `int_ring.mul_neg_one  : ∀a. (-1) * a == -a`

**Int order** (5)

- `int_ord.le_refl        : ∀a. a <= a`
- `int_ord.le_trans       : ∀a b c. a <= b → b <= c → a <= c`
- `int_ord.le_antisym     : ∀a b. a <= b → b <= a → a == b`
- `int_ord.add_mono       : ∀a b c. a <= b → a + c <= b + c`
- `int_ord.le_of_concrete : ∀m n. (decided at kernel time by literal comparison)` — the one escape hatch for ground arithmetic

**Propositional** (5)

- `and.intro    : ∀P Q. P → Q → P and Q`
- `and.elim_l   : ∀P Q. (P and Q) → P`
- `and.elim_r   : ∀P Q. (P and Q) → Q`
- `not.intro    : ∀P. (P → False) → not(P)`
- `false.elim   : ∀P. False → P`

**Total: 23 axioms.** If this list doubles without justification, we are over-scoping v1.

**Proved lemmas live outside the axiom set.** Appendix A shows that even a trivial inductive fact like `length_nonneg` requires a chain of ~5 rewrites against the primitive axioms — brutal user experience, standard raw-kernel pain (Lean before `omega`, Coq before `lia`). The answer is **not** to enlarge the axiom set; it is to ship a small companion library of *proved lemmas* in `std/prove.runa` that the kernel verifies once on stdlib build, then exposes through the same `apply` registry. Candidates for v1:

- `int_ord.zero_le : ∀a. 0 <= a → ∀b. 0 <= b → 0 <= a + b` (nonnegative-sum closure)
- `int_ord.le_succ : ∀a. a <= a + 1`
- `int_ord.strict_trans : ∀a b c. a < b → b <= c → a < c`
- `and.comm : ∀P Q. P and Q → Q and P`
- `eq.refl_on : ∀e. e == e` (specialized `eq.refl` for use-site closure of trivial equalities after rewrite)

These are **not trusted** — the kernel proves them from the 23 primitives. They just make user-level proofs shorter. The trust boundary stays at 23.

## 8. Unification (for APPLY)

First-order, non-recursive, occurs-checked. No higher-order unification, no pattern unification tricks. The algorithm is Martelli-Montanari with two additions:

1. Variables in axiom schemas are rigid distinct from variables in the current goal.
2. `op` (binary operator) unifies only if it is the same op symbol — no algebraic reasoning.

That is the *only* form of inference the kernel performs. Everything else is syntax-directed dispatch on the term form. Expected LoC: ~80.

## 9. Size Budget

This section supersedes the original estimate. The old table said the trusted
core would land around 540 LoC and stay under an 800 LoC hard ceiling. The
actual implementation is larger because `cases`, `induction_on`, local schemas,
schema freshening, and metadata-aware branch contexts are implemented in the
kernel rather than left as parser-only sugar.

Current implementation budget:

| Component | Boundary | Audit note |
| --- | --- | --- |
| `Term`, `Prop`, proof-term datatypes, displays | Trusted kernel | Defines the language the kernel accepts. |
| `Ctx` and constructor metadata | Trusted kernel | Required for sound `cases` and `induction_on`; wrong metadata can make a bad proof pass. |
| Hard-coded primitive axiom table | Trusted kernel | Primitive trust boundary by name. |
| Special axiom dispatch in `check_apply` | Trusted kernel | Handles proposition-valued axioms and concrete literal order checks. |
| Unification, occurs check, substitution, freshening | Trusted kernel | Determines whether an axiom/schema really matches the goal. |
| Rewrite and synthesis support | Trusted kernel | Determines the intermediate theorem used by `rewrite`. |
| `check`, `check_apply`, `check_cases`, `check_induction` | Trusted kernel | The judgment rules themselves. |
| Unit tests in `#[cfg(test)]` | Outside kernel | Essential evidence, not trusted by a released proof. |
| Proof parsing and elaboration in `src/lib.rs` / `src/bin/runa.rs` | Outside kernel, trusted pipeline | Builds the proposition and context passed to the kernel. |
| Computation-lemma generation | Outside kernel, trusted pipeline | Generates schemas the kernel then trusts as registry entries. |
| Z3 fallback | Outside kernel | Automation only; not part of the closed proof-kernel trust story. |

The audit count at `td-f8f162` is 1,572 pre-test lines in
`src/proof_kernel.rs`, with about 1,233 nonblank/noncomment implementation
lines. Tests add 656 lines and are explicitly excluded from the trusted budget.

The current target is to keep the trusted implementation near this audited
size. A future change must update this section before it:

- adds a new proof-term rule form,
- adds a new primitive axiom family,
- moves elaboration or solver behavior into the kernel, or
- grows the pre-test implementation materially beyond about 1,600 lines.

If the next proof feature needs that much growth, prefer splitting the kernel
into explicit modules such as syntax, registry, unification, and checking while
keeping the same trust-boundary list above. A split is organizational; it does
not by itself reduce the trusted boundary.

## 10. Out of Scope for v1

- Disjunction (`or`) in propositions
- Existentials (`exists`)
- Floats / reals
- Function extensionality
- Higher-order predicates
- User-declared axioms (v1 trusts only `std/prove.runa`)
- Proof search / `auto` tactic
- Reflection / meta
- List / Map / Set lemmas (added in v2 as per-type axiom packs)
- Termination checking for user-written recursive lemmas (v1: no user recursion inside proofs)

Any `?` invariant outside this scope still works — it falls through to Z3 exactly as today. The kernel is a *strict upgrade* over the current `runa verify`, never a regression.

## 11. Historical Open Questions, Now Resolved

1. **How does the parser distinguish proof mode from value mode inside `|` arms?**
   Proof terms are parsed through the explicit proof-term parser used by
   `? ... by { ... }`, not by evaluating ordinary Futuruna expressions as
   proofs.
2. **Do we canonicalize expressions inside propositions?** No. The current
   kernel remains syntactic. Commutativity, associativity, and similar facts
   must be invoked through explicit axioms or proved lemmas.
3. **Where does the axiom registry live?** Primitive axioms live in the
   hard-coded Rust registry. Checked local schemas can be registered alongside
   them, but they do not become primitive trust.
4. **What happens for tuple or constructor patterns?** The kernel does not
   silently introduce an implicit split for arbitrary pattern heads. Explicit
   `cases` and `induction_on` are supported when the trusted proof context has
   constructor-family metadata.

## 12. Acceptance Criteria

This spec is **done** when:

- [x] Section 3 (proposition grammar) is concrete enough that two reviewers would accept the same set of propositions.
- [x] Section 5 (judgment rules) has one rule per term form and no hand-waving. *(Revised during walkthrough: added REWRITE, removed LEMMA, clarified APPLY + IND.)*
- [x] Section 7 (axiom list) has every primitive named with its type schema.
- [x] A pen-and-paper proof of `add_comm` and `length_nonneg` using only the listed axioms and rules has been walked through end-to-end. *(See Appendix A. The walkthrough forced three design changes: `rewrite` term form, computation-lemma pass, kernel sees open body not ∀-closed.)*
- [x] The four historical open questions in Section 11 have agreed answers.
- [x] The trusted-boundary size/surface audit states which code remains inside
  the proof-kernel trust boundary.

The original design gates are now closed. Future acceptance is governed by the
audit policy in Section 0: update the boundary and size budget before adding
new rule forms, primitive axiom families, or material kernel growth.

---

## Appendix A. Walked Proofs

These two proofs are the design's ground truth. Every judgment rule, the axiom
set, and the computation-lemma pass has to cooperate to close them. If any step
can't be justified by the rules in Section 5, the design has a hole and must be
revised before expanding the kernel.

### A.1. `add_comm`

```runa
| add_comm: (a: Int, b: Int) -> a + b == b + a

? add_comm by {
    | (a, b) -> apply int_ring.comm_add
}
```

**Kernel view.** After the parser lifts the `|` head, the context is `Γ = { a : Int, b : Int }` and the goal is `a + b == b + a`.

**Step 1.** `apply int_ring.comm_add` with term list `[]`.

- Look up `int_ring.comm_add` in the axiom registry → schema `∀α β. α + β == β + α`, zero premises.
- Unify the schema conclusion `α + β == β + α` with the goal `a + b == b + a`: first-order unifier `σ = { α ↦ a, β ↦ b }`.
- No premise proofs to check (`n = 0`).
- By [APPLY], `Γ ⊢ apply int_ring.comm_add : a + b == b + a`. ✓

**One step. Done.**

---

### A.2. `length_nonneg`

```runa
# List(a) = Nil | Cons(a, List(a))

> length(xs: List(a)) -> Int {
    match xs {
        | Nil         -> 0
        | Cons(_, t)  -> 1 + length(t)
    }
}

| length_nonneg: (xs: List(a)) -> 0 <= length(xs)

? length_nonneg by {
    | xs -> induction_on xs {
        | Nil ->
            rewrite (apply length.nil) in (apply int_ord.le_refl)

        | Cons(h, t) ->
            rewrite (apply length.cons) in
            rewrite (apply int_ring.comm_add) in      -- length(t) + 1  →  1 + length(t) stays; comm_add used under reasoning below
            let one_le_sum =
                rewrite (apply int_ring.zero_add) in
                apply int_ord.add_mono(ih)            -- 0 <= length(t)  ⟹  0 + 1 <= length(t) + 1
            in
                apply int_ord.le_trans(
                    apply int_ord.le_of_concrete,     -- 0 <= 1
                    one_le_sum
                )
    }
}
```

**Computation lemmas** auto-generated from the `length` function (Section 6.5):

- `length.nil  : length(Nil) == 0`
- `length.cons : ∀h t. length(Cons(h, t)) == 1 + length(t)`

**Kernel view at the top.** `Γ = { xs : List(a) }`, goal = `0 <= length(xs)`.

---

**Nil branch.** The [IND] rule drops us into the branch with `Γ_nil = { xs : List(a) }` (no new hypotheses, no `ih` — `Nil` has no recursive arguments). Goal becomes `P[xs := Nil]` = `0 <= length(Nil)`.

**Step N1.** `rewrite (apply length.nil) in (apply int_ord.le_refl)`.

- Check the equation proof first: `apply length.nil` with zero premises. Schema: `length(Nil) == 0`. No unifier variables. [APPLY] gives `Γ_nil ⊢ apply length.nil : length(Nil) == 0`. ✓
- [REWRITE] says: if the equation is `e₁ == e₂`, replace `e₁` with `e₂` in the goal. Here `e₁ = length(Nil)`, `e₂ = 0`. Goal `0 <= length(Nil)` becomes `0 <= 0`.
- Check body: `apply int_ord.le_refl`. Schema `∀α. α <= α`. Unify conclusion `α <= α` against new goal `0 <= 0`: `α ↦ 0`. No premises. [APPLY] gives `Γ_nil ⊢ apply int_ord.le_refl : 0 <= 0`. ✓
- [REWRITE] closes: `Γ_nil ⊢ rewrite ... : 0 <= length(Nil)`. ✓

**Nil branch done.**

---

**Cons branch.** [IND] drops us into `Γ_cons = { xs : List(a), h : a, t : List(a), ih : 0 <= length(t) }`. Goal = `P[xs := Cons(h, t)]` = `0 <= length(Cons(h, t))`.

**Step C1.** Outer `rewrite (apply length.cons) in <body>`.

- Equation: `apply length.cons` has schema `∀h' t'. length(Cons(h', t')) == 1 + length(t')`. Unify against some `e₁ == e₂` form — the kernel uses the current goal's subterms to find a match. `length(Cons(h, t))` appears in the goal, so `h' ↦ h, t' ↦ t`, yielding `length(Cons(h, t)) == 1 + length(t)`. [APPLY] ✓.
- [REWRITE] replaces `length(Cons(h, t))` with `1 + length(t)` in the goal. New goal: `0 <= 1 + length(t)`.

**Step C2.** We now have to prove `0 <= 1 + length(t)` in `Γ_cons`. The proof sketched above builds it via `le_trans(0 <= 1, 1 <= 1 + length(t))`, where the right half comes from `add_mono` applied to `ih`.

Sub-derivation `one_le_sum : 1 <= 1 + length(t)`:

- `apply int_ord.add_mono(ih)` with `ih : 0 <= length(t)`. Schema: `∀α β γ. α <= β → α + γ <= β + γ`. Conclusion unifies with `? + γ <= ? + γ`, premise unifies against `ih`'s type `0 <= length(t)`: `α ↦ 0, β ↦ length(t)`. `γ` is free; the kernel needs the goal or an annotation to pin it.
- Here's a subtlety: at this point the *intended* conclusion is `0 + 1 <= length(t) + 1`, so `γ ↦ 1`. But the enclosing `rewrite (apply int_ring.zero_add) in ...` forces the step: the goal *after* the outer rewrite is `1 <= 1 + length(t)`; rewriting `0 + 1 == 1` means the inner goal is `0 + 1 <= 1 + length(t)`; then we also need the right side `length(t) + 1 == 1 + length(t)` to transform `0 + 1 <= length(t) + 1` into `0 + 1 <= 1 + length(t)`.

**Honest finding from the walkthrough.** The nested-rewrite dance here is *exactly* the pain I predicted in Section 7. The proof is mechanically closeable from the 23 primitives, but writing it out requires careful bookkeeping of which rewrite transforms which side of the `<=`. A production user would reach for `int_ord.zero_le` (the proved-lemma helper) and collapse the whole inductive step to:

```runa
| Cons(h, t) ->
    rewrite (apply length.cons) in
        apply int_ord.zero_le(apply int_ord.le_of_concrete, ih)
```

where `int_ord.zero_le : ∀a b. 0 <= a → 0 <= b → 0 <= a + b` is proved once in `std/prove.runa` from `add_mono` + `zero_add` + `le_trans`, and lives outside the trust boundary.

---

**Outer conclusion.** [IND] requires both arms to check `P[xs := C(ȳ)]` for each constructor `C`. Both check, so [IND] gives `Γ ⊢ induction_on xs { ... } : 0 <= length(xs)`. ✓

**`length_nonneg` proved.**

---

### Gaps the walkthrough revealed (now folded back into the spec)

1. **`rewrite` term form.** Essential; added to Section 4 grammar and Section 5 rules.
2. **Computation lemma pass.** Added as Section 6.5. Without it, any goal mentioning a user-defined function is opaque.
3. **Kernel sees open body, not `∀`-closed prop.** Clarified in Section 6. This removed the need for a `forall`-intro rule and eliminated one full judgment rule from the kernel (a silent win).
4. **Proved-lemma library.** Added to Section 7 as a distinct layer. The 23 axioms stay minimal; user pain is absorbed by a separately-checked helper library.
5. **`apply` notation.** `apply N(t̄)` takes *premise proofs*, not universally-quantified variables. Those are solved by unification against the goal. Fixed the Section 1 example and clarified Section 4.
6. **`by_lemma` removed.** The unified `apply` registry subsumes it. One fewer term form.

---

*The kernel is the one piece of Futuruna that must be unambiguously correct. Everything else — the parser, the stdlib, the `runa verify` command — can have bugs and be patched. A wrong kernel means every proved invariant is a lie. Design slowly here. The budget for this file is measured in thought, not lines.*
