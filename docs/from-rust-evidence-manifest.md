# FRSS-v0 Evidence Manifest

This manifest maps each supported FRSS-v0 source-shape claim to exact
Rust-vs-Futuruna stdout evidence. It is part of the preview-to-production audit
for `runa from-rust`; a Rust shape is not part of FRSS-v0 unless it appears
here or in a linked manifest with a blocking lane.

Authoritative lanes:

- `runa from-rust --test examples/from-rust/`
- `./scripts/from-rust-downstream-canary.sh`
- `./scripts/from-rust-differential.sh`

`from-rust --verify` CLI tests prove stable user-facing summaries. They do not
by themselves promote a source shape; supported-source promotion requires exact
stdout parity in one of the lanes above.

## Supported Source Shapes

| ID | Supported source shape | Exact-match evidence | Lane |
|----|------------------------|----------------------|------|
| FRSS-SRC-001 | Flat single Rust file with top-level supported items and deterministic `main` stdout | `examples/from-rust/t01_basics.rs`; all downstream supported fixtures | broad example corpus; downstream canary |
| FRSS-SRC-002 | `std` imports used by checked fixtures without crate/module translation | `tests/from-rust/downstream/supported/event_rollup.rs`; `tests/from-rust/downstream/supported/inventory_report.rs`; `examples/from-rust/stress_hashmap.rs` | downstream canary; broad example corpus |
| FRSS-SRC-003 | Functions, parameters, returns, nested calls, and ordinary top-level helper calls | `examples/from-rust/t01_basics.rs`; `examples/from-rust/t02_if_else.rs`; generator `numeric_branch_matrix` | broad example corpus; differential lane |
| FRSS-SRC-004 | Primitive integer/float/bool/string values, arithmetic, comparisons, boolean operators, division, and modulo | `examples/from-rust/t01_basics.rs`; `examples/from-rust/t02_if_else.rs`; `examples/from-rust/t10_multiline.rs`; generator `numeric_branch_matrix` | broad example corpus; differential lane |
| FRSS-SRC-005 | Local `let` bindings, `let mut`, reassignment, compound assignment, and branch-visible accumulator rebinding | `examples/from-rust/t19_while_loops.rs`; `tests/from-rust/downstream/supported/conditional_loop_aggregation.rs`; generator `enum_loop_rebinding` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-006 | `if`/`else` control flow and expression-like conditional results | `examples/from-rust/t02_if_else.rs`; `examples/from-rust/t10_multiline.rs`; generator `numeric_branch_matrix` | broad example corpus; differential lane |
| FRSS-SRC-007 | `match` over primitive, enum, option/result, and checked reference-pattern shapes | `examples/from-rust/t05_enums.rs`; `examples/from-rust/t09_nested_match.rs`; `examples/from-rust/adversarial_5_patterns.rs`; `tests/from-rust/downstream/supported/text_command_parser.rs` | broad example corpus; downstream canary |
| FRSS-SRC-008 | `for` loops over checked `Vec`, slice, and reference iteration shapes | `examples/from-rust/t06_lists.rs`; `examples/from-rust/t12_iter_patterns.rs`; `tests/from-rust/downstream/supported/invoice_totals.rs`; `tests/from-rust/downstream/supported/event_rollup.rs` | broad example corpus; downstream canary |
| FRSS-SRC-009 | `while` loops and loop-carried scalar state | `examples/from-rust/t19_while_loops.rs`; `examples/from-rust/t10_multiline.rs` | broad example corpus |
| FRSS-SRC-010 | Struct declarations, named-field construction, field access, borrowed helper parameters, and nested struct values | `examples/from-rust/t04_structs.rs`; `tests/from-rust/downstream/supported/invoice_totals.rs`; `tests/from-rust/downstream/supported/customer_nested_orders.rs`; generator `nested_order_totals` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-011 | Enum declarations with unit, tuple, and checked named variants | `examples/from-rust/t05_enums.rs`; `examples/from-rust/t17_state_machine.rs`; `examples/from-rust/adversarial_3_error_handling.rs`; `tests/from-rust/downstream/supported/config_validation.rs` | broad example corpus; downstream canary |
| FRSS-SRC-012 | Recursive ADTs and recursive `Box<T>` enum constructors | `examples/from-rust/t09_nested_match.rs`; `examples/from-rust/t15_nested_data.rs`; `examples/from-rust/t18_parser.rs`; `examples/from-rust/adversarial_5_patterns.rs` | broad example corpus |
| FRSS-SRC-013 | Inherent impl methods, method calls, `Self` constructors, functional lowering of checked `&mut self` field pushes, and narrow recursive `Option<&T>` search | `examples/from-rust/adversarial_1_ownership.rs` | broad example corpus |
| FRSS-SRC-014 | `String` and `&str` construction, `.to_string()`, cloning, concatenation, length, split/join-style workflows, prefix/suffix checks, trim/lowercase/replace/classification | `examples/from-rust/t03_strings.rs`; `examples/from-rust/t14_string_processing.rs`; `tests/from-rust/downstream/supported/text_normalization_report.rs`; generator `text_transform_matrix` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-015 | Checked formatting and stdout macros: `println!`, `eprintln!`, `format!`, `{:?}`, and `{:.N}` forms represented by fixtures | `examples/from-rust/t03_strings.rs`; `examples/from-rust/adversarial_4_closures.rs`; `tests/from-rust/downstream/supported/text_command_parser.rs`; generator `text_transform_matrix` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-016 | `Vec<T>` literals, pushes, indexing, `.len()`, nested vectors, and vector-of-struct workflows | `examples/from-rust/t06_lists.rs`; `examples/from-rust/t12_iter_patterns.rs`; `tests/from-rust/downstream/supported/customer_nested_orders.rs`; generator `nested_order_totals` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-017 | `Option<T>` construction and matching, including `Option<String>` input workflows | `examples/from-rust/t07_option_result.rs`; `examples/from-rust/stress_result_chain.rs`; `tests/from-rust/downstream/supported/config_validation.rs`; generator `option_result_pipeline` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-018 | `Result<T, E>` construction, matching, simple `?` chains, early `Err` returns, and checked integer `parse().map_err(...)?` remapping | `examples/from-rust/t11_chained_result.rs`; `examples/from-rust/adversarial_3_error_handling.rs`; `tests/from-rust/downstream/supported/error_row_pipeline.rs`; generator `option_result_pipeline` | broad example corpus; downstream canary; differential lane |
| FRSS-SRC-019 | Deterministic map reports with `BTreeMap<String, i64>`, get/insert/update workflows, and sorted key-visible stdout | `tests/from-rust/downstream/supported/event_rollup.rs`; `tests/from-rust/downstream/supported/inventory_report.rs`; generator `btree_rollup_report` | downstream canary; differential lane |
| FRSS-SRC-020 | Checked map/set-like example-corpus workflows where stdout is deterministic | `examples/from-rust/stress_hashmap.rs`; `examples/from-rust/real_world_1.rs`; `examples/from-rust/real_world_3.rs` | broad example corpus |
| FRSS-SRC-021 | Closures, closure arguments, higher-order functions, and checked `Box<dyn Fn>`/`impl Fn` composition shapes represented by examples | `examples/from-rust/t08_closures.rs`; `examples/from-rust/adversarial_2_generics.rs` | broad example corpus |
| FRSS-SRC-022 | Narrow generic trait fixture: `Functor` associated-type calls over `Option`/`Result`, generic higher-order functions, generic struct constructors, and generic inherent methods | `examples/from-rust/adversarial_2_generics.rs` | broad example corpus |
| FRSS-SRC-023 | Checked iterator/stateful subset: tuple-key `sort_by`, Fibonacci-style `scan(...).collect()`, and `entry(...).or_insert_with(Vec::new).push(...)` grouping | `examples/from-rust/adversarial_4_closures.rs` | broad example corpus |
| FRSS-SRC-024 | Consumer-shaped workflows across config validation, invoice arithmetic, event/report aggregation, parser/text transformations, nested order data, error rows, inventory reports, and text normalization | `tests/from-rust/downstream/supported/*.rs` | downstream canary |
| FRSS-SRC-025 | Generated seed-stable coverage for the six FRSS-v0 families listed in `tests/from-rust/differential/search-manifest.tsv` | generators `numeric_branch_matrix`, `option_result_pipeline`, `nested_order_totals`, `btree_rollup_report`, `text_transform_matrix`, `enum_loop_rebinding` | differential lane |

## Unsupported Boundary Evidence

These shapes are not supported-source claims. They stay in the manifest so the
contract remains explicit about what users can rely on failing closed.

| Unsupported category | Boundary | Fail-closed evidence | Lane |
|----------------------|----------|----------------------|------|
| `borrowed-return-reference` | General functions that return borrowed references outside the checked recursive owned-tree search shape | `tests/from-rust/downstream/unsupported/borrowed_return_reference.rs` | downstream canary |
| `associated-types` | Associated types outside the checked `Functor` fixture | `tests/from-rust/downstream/unsupported/associated_type_trait.rs` | downstream canary |
| `impl-trait` | `impl Trait` signatures outside the checked `impl Fn` composition shape | `tests/from-rust/downstream/unsupported/impl_trait_iterator.rs` | downstream canary |
| `unsupported-map-err` | `Result::map_err` forms outside checked integer parse remapping | `tests/from-rust/downstream/unsupported/unsupported_map_err.rs` | downstream canary |
| `stateful-iterator-chain` | Iterator/map state machines outside checked `sort_by`, `scan`, and `entry(...).or_insert_with(...).push(...)` shapes | `tests/from-rust/downstream/unsupported/stateful_iterator_scan.rs` | downstream canary |
| `reference-tuple-match` | Tuple-of-references matches outside the checked two-reference simplification subset | `tests/from-rust/downstream/unsupported/reference_tuple_match.rs` | downstream canary |
| `async-threading` | `async`, `.await`, and thread-spawning Rust | `tests/from-rust/downstream/unsupported/async_function.rs`; `tests/from-rust/downstream/unsupported/thread_spawn.rs` | downstream canary |
| `unsupported-effect` | File I/O, environment/process state, runtime I/O, networking, wall-clock time, and randomized hashing through `std` APIs | `tests/from-rust/downstream/unsupported/effectful_std_api.rs` | downstream canary |
| `unsafe-rust` | Unsafe blocks | `tests/from-rust/downstream/unsupported/unsafe_block.rs` | downstream canary |
| `unsupported-macro` | Macro names outside the checked stdout/vector formatting subset, including unproven control/assertion macros | `tests/from-rust/downstream/unsupported/unsupported_macro.rs`; `tests/from-rust/downstream/unsupported/unchecked_assert_macro.rs` | downstream canary |
| `unsupported-format-spec` | Formatting placeholders outside `{}`, `{:?}`, and `{:.N}` checked forms | `tests/from-rust/downstream/unsupported/unsupported_format_spec.rs` | downstream canary |
| `unsupported-module` | Rust `mod` declarations and module trees outside the flat single-file boundary | `tests/from-rust/downstream/unsupported/external_module_declaration.rs` | downstream canary |
| `unsupported-rust-item` | Top-level Rust item forms outside the checked item subset | `tests/from-rust/downstream/unsupported/unsupported_item_union.rs` | downstream canary |
| `unsupported-rust-expr` | Rust expression forms with no checked lowering | `tests/from-rust/downstream/unsupported/unsupported_expr_fallback.rs` | downstream canary |
| `external-crate` | Non-stdlib `use` or `extern crate` inputs | `tests/from-rust/downstream/unsupported/external_crate_use.rs` | downstream canary |

## Audit Notes

This pass found no remaining supported-source claim in
`docs/from-rust-contract.md` without exact-match evidence in the manifest above.
Claims that would require broader Rust package translation, arbitrary macro
expansion, general lifetime/reference preservation, arbitrary trait machinery,
general iterator state machines, unsafe semantics, async/threading semantics,
effectful runtime APIs, or Rust module trees remain unsupported and are either
covered by the fail-closed table or outside the syntactically detectable source
shape surface.

`from-rust --verify` translator-bug summaries are tracked separately by
`td-ed2a52`; they are user-workflow evidence, not supported-source shape
evidence.
