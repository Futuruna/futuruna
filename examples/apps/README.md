# Application demonstrations

These application examples were moved from `project-examples/`. Formatting and
location comments were updated; all four emit the same Rust as their originals.

- [Warehouse inventory](inventory/inventory.runa)
- [URL shortener](link-shortener/shortener.runa)
- [Log analyzer](log-analyzer/analyzer.runa)
- [Task tracker](task-tracker/tracker.runa)

They demonstrate language features rather than establish a production-support
contract. Follow each file's instructions and keep database/build output in an
ignored working directory as described in the [repository map](../../docs/repository-layout.md).

## Validation status

Checked with `runa 0.2.0` during the September 2026 repository cleanup:

| Demonstration | `runa check` |
| --- | --- |
| Link shortener | Pass |
| Task tracker | Pass |
| Inventory | Existing native Rust type-mismatch errors (`E0308`) |
| Log analyzer | Existing `alert_level` rule-dispatch/inference failure (`E0425`) |

The two failures also reproduce from the original files. They are tracked as
`td-b9d595`; these examples remain available as references while their native
execution is repaired. All four pass `runa fmt --check` after cleanup.
