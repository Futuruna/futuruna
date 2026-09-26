# Application demonstrations

- [URL shortener](link-shortener/shortener.runa)
- [Log analyzer](log-analyzer/analyzer.runa)

These examples demonstrate language features rather than establish a production
support contract. Follow each file's instructions and keep generated output in
an ignored working directory as described in the [repository map](../../docs/repository-layout.md).

The SQLite-dependent inventory and task tracker demos were retired with the
built-in database feature. Their source remains available in Git history:
[inventory](https://github.com/Futuruna/futuruna/blob/075fe19d656f4c8f2a1fc1bda4e38b25b949d582/examples/apps/inventory/inventory.runa)
and [task tracker](https://github.com/Futuruna/futuruna/blob/075fe19d656f4c8f2a1fc1bda4e38b25b949d582/examples/apps/task-tracker/tracker.runa).
See the [database migration guidance](../../docs/compatibility-guides/0.2.x.md#database-feature-removal).

## Validation status

During the September 2026 repository cleanup, the link shortener passed
`runa check`. The log analyzer had an existing `alert_level` rule-dispatch/type
inference failure (`E0425`), tracked as `td-b9d595`. Both passed `runa fmt --check`.
