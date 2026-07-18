# M30: Split RustCodegen into Passes

**Tagline:** "One thing at a time."

**Status:** In progress.

## Goal

Extract the declaration scanning phase from `emit_program` into a standalone
`scan_declarations()` method. This makes the pipeline explicit:

```
scan_declarations(stmts) → populated TypeRegistry + metadata
  ↓
emit_functions(stmts) → Rust function code
  ↓  
emit_main(stmts) → Rust main() code
```

## Context

`emit_program` is ~1,275 lines doing scanning, registration, and emission
in one interleaved pass. M29 extracted TypeRegistry and OwnershipAnalysis
as data structures, but the code that populates them is still inside
emit_program. This milestone extracts that code.

## Sub-steps

### Sub-step 1: Extract scan_declarations

**Change:** Move the import resolution + type registration + effect inference
phases out of emit_program into `scan_declarations(&mut self, stmts) -> Vec<Stmt>`.
Returns the resolved statement list (with imports merged). emit_program calls it
first, then does emission only.

**Test:** All 143 tests pass. `runa emit` output identical.

### Sub-step 2: Extract borrow_analysis pass

**Change:** Move the borrow_only_params computation into a standalone method
`compute_borrow_flags(&mut self, stmts)` called after scan_declarations.

**Test:** All 143 tests pass.

### Sub-step 3: Pipeline function

**Change:** Create `compile_program(stmts) -> String` that calls passes in order:
1. scan_declarations
2. compute_borrow_flags
3. emit_program (now emission-only)

**Test:** `runa emit` uses compile_program. All 143 tests pass.

## Checklist

- [x] scan_declarations extracted (~500 lines: imports, type registration, async detection, Rc computation)
- [x] compute_borrow_flags extracted (fixed-point borrow analysis)
- [x] emit_program calls passes explicitly: scan → borrow → emit
- [x] runa emit --fir uses scan_declarations directly
- [x] 4 scan tests (TypeRegistry population, struct detection, user functions)
- [x] All 147 tests pass

## Verification

```bash
cargo build --release
runa test                  # 69 + 12 = 81
cargo test --lib           # 26
cargo test --bin runa      # 36+
runa emit program.runa     # Identical output
```
