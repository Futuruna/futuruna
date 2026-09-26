# Futuruna editor support

The extension starts `runa lsp` for diagnostics, completion, hover and
go-to-definition. Configure `futuruna.serverPath` when the compiler is not on
your path. The compiler provides the language behavior; the TextMate grammar
provides syntax colors, including quoted source blocks and template expressions.

Use Node.js 22 or newer to run the grammar regressions from this directory:

```sh
npm ci --ignore-scripts
npm test
```

The tests run the actual VS Code TextMate tokenizer and Oniguruma engine, with
versions pinned in `package-lock.json`. They check scopes across line boundaries,
not just whether a regular expression matches a sample. The `Editor grammar` CI
job runs them independently of the Rust compiler gate. The compiler's LSP process
tests live in `tests/lsp_protocol.rs` and run through the ordinary Cargo lane.

The grammar recognizes both canonical `True`/`False` and accepted lowercase
aliases. Editor completion suggests the canonical spellings. Syntax colors
do not establish that a program parses, type-checks or evaluates successfully.
