# M31: Type Annotation Pass

**Tagline:** "Every expression knows its type."

**Status:** In progress.

## Goal

Make the LoweringCtx resolve FirTy on every FIR node instead of
FirTy::Unknown. This eliminates the heuristic sets (string_typed_vars,
float_typed_vars) and gives the emitter real type information.

## Sub-steps

### Sub-step 1: Type environment in LoweringCtx

**Change:** Add a type environment (`BTreeMap<String, FirTy>`) to LoweringCtx.
Function params with annotations populate it. Var lookups resolve from it.
Literals get their obvious types. BinOps infer from operands.

**Test:** FIR nodes for typed programs have non-Unknown FirTy.

### Sub-step 2: Ty → FirTy conversion

**Change:** Convert Futuruna `Ty` to `FirTy` so function signatures
feed into the type environment.

**Test:** `> add(a: Int, b: Int) -> Int` produces FirTy::Int on body.

### Sub-step 3: Type-aware emission

**Change:** emit_fir_expr uses FirTy to make decisions currently handled
by heuristic sets (string concat via format!, float division, etc.)

**Test:** String + String emits format!(), not +.

## Checklist

- [ ] Type environment in LoweringCtx
- [ ] Ty → FirTy conversion
- [ ] Literals resolve to concrete FirTy
- [ ] Function params populate type env
- [ ] BinOp type inference from operands
- [ ] String concat detection via FirTy (not heuristic)
- [ ] Tests verify FirTy on all node types
