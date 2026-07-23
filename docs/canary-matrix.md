# Canary Matrix

This is the authored coverage map for Futuruna's canary suite.

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
| `tests/canary/stateful/subject_funnel_test.runa` | `stateful` | subjects, projection, aggregation, transitions |

## Planned Next

### Core
- `ownership_text_pipeline_test.runa`: strings, captures, list transforms, chunking
- `proof_guarded_pipeline_test.runa`: ordinary data pipeline with explicit proof-backed invariants
- `persistent_tree_diff_test.runa`: recursive ADTs, structural sharing style workflows, flattening
- `collection_join_mesh_test.runa`: tuples, `zip`, `enumerate`, `partition`, stable ordering
- `top_level_mesh_test.runa`: top-level values, free functions, recursive helper composition

### Stateful
- `subject_window_alerts_test.runa`: subject streams, windows, threshold alerts, aggregation
- `actor_job_queue_test.runa`: actor state transitions, ordering, retries
- `lifecycle_projection_test.runa`: scopes, teardown, subject-derived projections
- `effect_retry_flow_test.runa`: handler variation, early return, result propagation
- `stream_subject_bridge_test.runa`: subject ingestion, stream transforms, stable snapshots

### Extended
- `json_report_pipeline_test.runa`: JSON parsing, object extraction, list aggregation
- `regex_classifier_test.runa`: regex extraction, replacement, result bucketing
- `db_reconciliation_test.runa`: DB writes, reads, and deterministic summaries
- `http_handler_contract_test.runa`: request routing and response tuples
- `wasm_export_surface_test.runa`: wasm-visible functions and deterministic output
- `import_mesh_test.runa`: local module graph, top-level bindings, helper composition

### Regressions
- Distill every user-found semantic bug into an authored workflow canary when it reflects real usage
- Keep minimized one-off compiler probes in ordinary regression tests, not here
