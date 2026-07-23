# Canary Matrix

This is the authored coverage map for Futuruna's canary suite.

Tracked in `td-39b478` under the broader mint epic `td-f7f0d2`.

Target shape:
- `core`: 15 blocking canaries with interpreter, compiled, codegen, and roundtrip parity
- `stateful`: 10 canaries for subjects, actors, lifecycle, and effectful workflows
- `extended`: 10 canaries for JSON, regex, DB, HTTP, WASM, and import-heavy programs
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

## Planned Next

### Core

### Stateful
- `td-ff3e1e`: `effect_retry_flow_test.runa`
- `td-e77c7f`: `stream_subject_bridge_test.runa`

### Extended
- `td-9fdd94`: `json_report_pipeline_test.runa`, `regex_classifier_test.runa`, `db_reconciliation_test.runa`, `http_handler_contract_test.runa`, `wasm_export_surface_test.runa`, and `import_mesh_test.runa`

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
- Keep minimized one-off compiler probes in ordinary regression tests, not here
