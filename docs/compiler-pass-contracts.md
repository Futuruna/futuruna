# Compiler Pass Contracts

These contracts cover passes that have caused first-hour bugs because they sit at
feature cross-products: imports, invariants/proofs, ownership-sensitive values,
FIR, and emitted Rust. The checked metadata for this table lives in
`src/bin/runa.rs` and is verified by `compiler_pass_contracts_*` tests.

| Pass | Input phase | Output phase | Contract | Representative evidence |
| --- | --- | --- | --- | --- |
| Ownership/use analysis | Parsed AST after declaration scan and import expansion | Per-scope ownership counts consumed by AST and FIR Rust emitters | Visit every expression-bearing AST child, including `Invariant` and `Prove`; mark non-Copy branch reuse for clone/borrow; do not treat imported script flow as root runtime code. | `tests/expect/artifact/ownership_branch_string_contract.runa`, `tests/canary/core/ownership_text_pipeline_test.runa`, `scripts/compiler-cross-product-canary.sh` |
| Import resolution | Parsed AST with caller `source_dir` | Normalized public import/export graph plus imported declarations | Resolve nested plain imports relative to the imported file, restore parent `source_dir`, expose only exported qualified members, and keep private/script symbols from leaking. | `tests/expect/imports/import_normalization_contract.runa`, `tests/differential/corpus/imports/import_mesh_consumer.runa`, `scripts/compiler-cross-product-canary.sh` |
| FIR lowering | Resolved AST after type registry and ownership analysis | FIR tree with resolved types and ownership modes | Traverse every FIR expression-bearing statement through canonical immutable/mutable visitors; preserve `VarMode`; keep module-qualified/imported symbols scoped in FIR snapshots. | `tests/expect/phase/fir_cross_binding.runa`, `tests/expect/phase/fir_module_qualified.runa` |
| Rust codegen | Resolved AST/FIR plus import, type, ownership, and borrow metadata | Rust source that compiles and preserves Futuruna value semantics | Emit clone/borrow behavior for read-only String/list/map flows, keep Pair/map_entries tuple contracts stable, and suppress private import/script leakage in generated artifacts. | `tests/expect/artifact/ownership_branch_string_contract.runa`, `tests/expect/phase/rust_map_entries_pairs.runa`, `tests/codegen_integration_regression_test.runa`, `scripts/compiler-cross-product-canary.sh` |

Contract rows are not just documentation. Each row must keep at least one
coverage marker and one representative fixture in the checked metadata. If a pass
contract points at a missing fixture or at a marker that no longer exists in the
AST/FIR coverage matrix, the focused contract tests fail.
