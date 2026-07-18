# M32: Constraint-Based Type Inference

**Tagline:** "Types flow through the program."

**Status:** In progress.

## Goal

Add type variables and unification so unannotated expressions get inferred
types. Currently, unannotated function params and unresolvable vars get
FirTy::Unknown. After M32, `> id(x) { x }` gets type `forall a. a -> a`.

## Sub-steps

### Sub-step 1: Type variables + unification engine

**Change:** Add `FirTy::Var(usize)` for fresh type variables. Implement
union-find unification with occurs check.

**Test:** Unify(Var(0), Int) resolves Var(0) to Int.

### Sub-step 2: Constraint generation

**Change:** During lowering, generate equality constraints from expressions:
- `a + b` where a: Var(0) → constraint Var(0) = Int
- `if c then a else b` → constraint typeof(a) = typeof(b)
- `f(x)` where f: A → B → constraint typeof(x) = A

**Test:** Constraints generated correctly for simple programs.

### Sub-step 3: Solve + substitute

**Change:** After lowering a function body, solve all constraints via
unification. Walk FIR and substitute resolved type variables.

**Test:** `> double(x) { x + x }` infers x: Int without annotation.

## Checklist

- [ ] FirTy::Var(usize) type variables
- [ ] TypeInference struct with fresh_var(), unify(), resolve()
- [ ] Occurs check prevents infinite types
- [ ] Constraint generation during lowering
- [ ] Substitution pass after solving
- [ ] Tests for inference on unannotated params
