# Canary Matrix

This is the authored coverage map for Futuruna's canary suite.

Tracked in `td-39b478` under the broader mint epic `td-f7f0d2`.

Target shape:
- `core`: 15 blocking canaries with interpreter, compiled, codegen, and roundtrip parity
- `stateful`: 10 canaries for subjects, actors, lifecycle, and effectful workflows
  (7 implemented, including one adversarial cross-surface workflow)
- `extended`: 10 canaries for JSON, regex, DB, HTTP, WASM, and import-heavy programs
- `storage`: dedicated persisted-storage runtime canaries for compiled SQLite
  behavior that needs isolated temp databases
- `interop`: dedicated Rust-facing library consumer canaries for `runa lib`
  output and Cargo integration
- `regressions`: every user bug class distilled into a broader authored workflow

## Implemented

| Path | Tier | Coverage |
|------|------|----------|
| `tests/canary/core/word_metrics_test.runa` | `core` | lists, maps, sets, tuples |
| `tests/canary/core/stream_windows_test.runa` | `core` | streams, windows, reductions |
| `tests/canary/core/recursive_inventory_test.runa` | `core` | recursive ADTs, top-level bindings, maps, sets |
| `tests/canary/core/loop_rebind_pipeline_test.runa` | `core` | `while`, `for`, rebinding, streams, list accumulation |
| `tests/canary/core/closure_scoreboard_test.runa` | `core` | tuples, closures, map folding, partitioning |
| `tests/canary/core/ownership_text_pipeline_test.runa` | `core` | strings, chunking, closure captures, ownership-sensitive transforms |
| `tests/canary/core/persistent_tree_diff_test.runa` | `core` | recursive ADTs with named list children, shared-subtree workflows, flattening, deterministic diffs |
| `tests/canary/core/collection_join_mesh_test.runa` | `core` | nested `zip`, `enumerate`, partitioning, stable ordering |
| `tests/canary/core/proof_guarded_pipeline_test.runa` | `core` | ordinary data pipeline with explicit proof-backed invariants |
| `tests/canary/core/top_level_mesh_test.runa` | `core` | top-level values, free functions, recursive helper composition |
| `tests/canary/stateful/subject_funnel_test.runa` | `stateful` | subjects, projection, aggregation, transitions |
| `tests/canary/stateful/subject_window_alerts_test.runa` | `stateful` | subjects, rolling windows, threshold alerts, deterministic summaries |
| `tests/canary/stateful/actor_job_queue_test.runa` | `stateful` | actors, ordered queue transitions, deterministic completion summaries |
| `tests/canary/stateful/lifecycle_projection_test.runa` | `stateful` | scoped projections, teardown, post-teardown stability, deterministic snapshots |
| `tests/canary/stateful/effect_retry_flow_test.runa` | `stateful` | effect handlers, actor-backed retry state, branching outcomes, deterministic audit summaries |
| `tests/canary/stateful/stream_subject_bridge_test.runa` | `stateful` | live stream-to-subject bridging, derived subscriptions, deterministic downstream aggregation |
| `tests/canary/stateful/stateful_adversarial_workflow_test.runa` | `stateful` | subjects, scoped derived streams, teardown, effect handlers, actor-backed audit state, deterministic adversarial invariants |
| `tests/canary/extended/json_report_pipeline_test.runa` | `extended` | JSON parse/build, nested arrays, list/map/set aggregation, deterministic report emission |
| `tests/canary/extended/regex_classifier_test.runa` | `extended` | regex matching, extraction, replacement, tag collection, deterministic triage summaries |
| `tests/canary/extended/db_reconciliation_test.runa` | `extended` | SQLite-backed ingest, reconciliation, deterministic query/report summaries |
| `tests/canary/extended/http_handler_contract_test.runa` | `extended` | request routing, response shaping, deterministic handler contract summaries |
| `tests/canary/extended/import_mesh_test.runa` | `extended` | transitive flat imports, qualified imports, content-addressed imports, deterministic cross-module summaries |
| `tests/canary/extended/wasm_export_surface_test.runa` | `extended` | WASM-facing exported primitive/string/list-numeric/option functions, private helpers, zero-arg exports |
| `tests/canary/interop/rust_consumer_lib.runa` | `interop` | `runa lib` output consumed from plain Rust via `rustc`, exported structs/enums/functions, borrowed params, lists, `Option`, `Result` |
| `tests/canary/interop/rust_consumer_external_crate_lib.runa` | `interop` | `runa lib` output consumed from an offline Cargo project with `@ depend`, `@ use`, raw Rust helpers, regex-backed stdlib codegen, exported APIs, `src/lib.rs` package layout through a downstream path dependency, and missing-dependency guidance in generated source/stderr |
| `tests/canary/storage/persist_tx_commit_savepoint_test.runa` | `storage` | compiled persisted transaction commit plus nested savepoint release |
| `tests/canary/storage/persist_tx_rollback_fail_test.runa` | `storage` | intentionally failing transactional scope used to prove rollback through a follow-up fixture |
| `tests/canary/storage/persist_tx_rollback_check_test.runa` | `storage` | compiled persisted readback proving rollback left only the committed baseline row |

## Planned Next

### Core

### Stateful

### Extended

### Regressions
- `td-4e7e91`: distill every user-found semantic bug into an authored workflow canary when it reflects real usage
- Intake rule: user bug classes belong here only when the distilled fixture is
  still a realistic multi-subsystem workflow; narrow compiler probes stay in
  ordinary regression tests
- `tests/canary/regressions/module_shadowing_regression_test.runa`: imported
  top-level bindings, computed getters, module-qualified access, and local
  shadowing
- `tests/canary/regressions/verify_process_audit_test.runa`: `process_run`
  tuples, tuple accessors, substring, and list-valued invariant display
- `tests/canary/regressions/unicode_string_semantics_test.runa`: non-ASCII
  string length, substring, char_at, index_of, and `length` function values use
  Unicode scalar values in interpreter and compiled execution
- Keep minimized one-off compiler probes in ordinary regression tests, not here
