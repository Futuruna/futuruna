# M29: Intermediate Representation (FIR)

**Tagline:** "The compiler gets a brain between thinking and speaking."

**Status:** Complete.

## Goal

Introduce FIR (Futuruna Intermediate Representation) — a typed,
ownership-annotated tree between the AST and Rust code emission.
Separate analysis from emission so concerns are independently testable.

## What was delivered

### TypeRegistry (29 fields extracted from RustCodegen)

Shared static metadata: types, variants, effects, rules, exports,
comptime, store. RustCodegen went from **138 → 36 fields** (74% reduction).

### OwnershipAnalysis

`OwnershipAnalysis { var_uses, consuming_uses }` with three entry points:
- `analyze()` — borrow-aware, for function bodies
- `analyze_simple()` — basic, for rule bodies and top-level
- `analyze_stmt_refs()` — for main-level statements

Wraps the 4 counting functions into a clean API.

### FIR types

```
FirExpr { kind: FirExprKind, span: Span, ty: FirTy }
FirExprKind::Var(name, VarMode) — Move | Clone | Borrow | Copy | Deref | RuleClone
FirStmt, FirDefn, FirProgram, FirMatchArm, FirEffHandler, FirHandler
FirTy — Int | Float | String | Bool | List | Named | Arrow | Unknown
```

### AST → FIR lowering (LoweringCtx)

Walks AST, resolves VarMode per variable reference from OwnershipAnalysis:
Deref > RuleClone > Copy > Clone > Move priority. Every ExprKind and
Stmt variant has a corresponding FIR lowering.

### FIR → Rust emission

Stateless `emit_fir_expr()` and `emit_fir_stmt()` produce Rust source
from FIR nodes. All core expression types handled:
Var, Lit, BinOp, UnOp, If, App, Field, Index, List, Tuple, Lambda,
Try, Match, Pipe, Block, Conjunction, Effect, Handle.

### CLI integration

`runa emit --fir` shows FIR pipeline output alongside old path for comparison.
`emit_via_fir()` handles functions, bindings, expressions, for loops,
and silently skips declarations (handled by TypeRegistry).

## What is deferred to M30

- TypeChecker sharing TypeRegistry (needs lib.rs refactor)
- Migrating old emit path to FIR for remaining features (builtins, async, actors, effects)
- Removing old emit path entirely

## Tests

| Category | Count | What |
|----------|-------|------|
| FIR type construction | 6 | VarMode, FirTy, FirProgram basics |
| AST → FIR lowering | 7 | Var/BinOp/fn lowering, span preservation, ownership modes |
| FIR → Rust emission | 12 | Var modes, literals, BinOp, App, If, Pipe, List |
| FIR pipeline integration | 10 | End-to-end, for loops, match, lambda, field access, clone |
| **Total runa unit tests** | **36** | |

Full suite: 26 lib + 36 runa + 69 happy + 12 error = **143 tests passing**.

## Verification

```bash
cargo build --release                    # Clean
cargo test --lib                         # 26 pass
cargo test --bin runa                    # 36 pass
./target/release/runa test              # 69 + 12 pass
./target/release/runa emit --fir file.runa  # Side-by-side comparison
```
