# M29: Intermediate Representation (FIR)

**Tagline:** "The compiler gets a brain between thinking and speaking."

## Goal

Introduce FIR (Futuruna Intermediate Representation) — a typed,
ownership-annotated tree between the AST and Rust code emission.
Today RustCodegen walks the AST directly and makes ownership/type
decisions during string concatenation. FIR separates analysis from emission.

## Context

`RustCodegen` in `runa.rs` has 139+ fields because it discovers metadata
while walking the AST. The same struct does:
- Type variant tracking
- Ownership inference (4 counting functions)
- Borrow analysis
- Effect tracking
- Code emission

This makes it impossible to test individual concerns, add new backends,
or optimize across function boundaries.

## Design

### What FIR is NOT

FIR is not SSA, not a bytecode, not a full MIR. It's the AST with
decisions attached: every expression annotated with its resolved type
and ownership mode (move/clone/borrow/copy).

### What FIR IS

```rust
struct FirExpr {
    kind: FirExprKind,    // mirrors ExprKind but with resolved info
    span: Span,           // from AST
    ty: FirTy,            // resolved type
    ownership: Ownership, // move | clone | borrow | copy
}

enum Ownership {
    Move,      // single use, transfer ownership
    Clone,     // multi-use, clone before use
    Borrow,    // read-only reference
    Copy,      // Copy type, free to duplicate
    Inout,     // mutable reference
}
```

### Incremental approach

Rather than building FIR all at once, we can extract one concern at a
time from RustCodegen:

1. **Extract type metadata** into a shared `TypeRegistry`
2. **Extract ownership analysis** into a standalone pass
3. **Define FIR types** that carry analysis results
4. **Lower AST → FIR** using TypeRegistry + ownership pass
5. **Emit Rust from FIR** (stateless walk)

Each step compiles and passes tests before the next.

## Sub-steps

### Sub-step 1: TypeRegistry extraction

**Change:** Move type/constructor/variant metadata from RustCodegen fields
into a shared `TypeRegistry` struct. Both TypeChecker and RustCodegen
consume it. Eliminates duplicate builtin lists.

**Test:** All 107 tests pass. TypeRegistry populated correctly.

### Sub-step 2: Ownership pass extraction

**Change:** Consolidate the 4 `count_*_uses` functions into a single
`OwnershipAnalysis` struct that computes move/clone/borrow decisions
per variable per function. Run it as a pre-pass before emission.

**Test:** `runa emit` produces identical Rust output. All 107 tests pass.

### Sub-step 3: FIR types + lowering

**Change:** Define `FirExpr`, `FirStmt`, `FirProgram`. Implement
`lower(ast: &[Stmt], registry: &TypeRegistry, ownership: &OwnershipAnalysis) -> FirProgram`.

**Test:** Round-trip: AST → FIR → Rust matches AST → Rust (old path).

### Sub-step 4: Emit from FIR

**Change:** New `emit_from_fir(fir: &FirProgram) -> String` that walks
FIR nodes and produces Rust. Replace old emission path.

**Test:** All 107 tests produce byte-identical output.

## Checklist

- [x] `TypeRegistry` struct extracted from RustCodegen (10 fields, 94 references redirected)
- [ ] TypeChecker and RustCodegen share TypeRegistry (deferred — needs lib.rs export)
- [ ] Ownership analysis consolidated into standalone pass
- [ ] FIR types defined
- [ ] AST → FIR lowering implemented
- [ ] FIR → Rust emission implemented
- [ ] Old emission path removed or gated behind flag
- [ ] All 107 tests pass through new path

## Files Modified

| File | Change |
|------|--------|
| `src/bin/runa.rs` | TypeRegistry extraction, ownership consolidation, FIR types + lowering + emission |
| `src/lib.rs` | TypeRegistry shared with TypeChecker |

## Verification

```bash
cargo build --release
runa test                    # 69 happy-path + 12 negative = 81
cargo test --lib             # 26 unit tests
runa emit program.runa       # Identical Rust output through FIR path
```
