# Application demonstrations

- [URL shortener](link-shortener/shortener.runa)
- [Log analyzer](log-analyzer/analyzer.runa)

These examples demonstrate language features rather than establish a production
support contract. Follow each file's instructions and keep generated output in
an ignored working directory as described in the [repository map](../../docs/repository-layout.md).

## Validation status

During the September 2026 repository cleanup, the link shortener passed
`runa check`. The log analyzer had an existing `alert_level` rule-dispatch/type
inference failure (`E0425`), tracked as `td-b9d595`. Both passed `runa fmt --check`.
