# M27: Error Reporting Overhaul

**Tagline:** "Errors that point, explain, and suggest."

## Goal

Make compiler errors precise, contextual, and helpful. Today errors are
string-formatted with line:col prefixes, AST nodes carry no span info,
134 `.unwrap()` calls can produce uncontextualized panics, and
`parse_int("")` silently returns `0`. After M27, every error points to
the exact source location with multi-line context and underlines.

## Context

Current error infrastructure:
- `display_error()` in `lib.rs:7835` — parses `"LINE:COL: message"` strings,
  shows source line with single `^` caret
- `TypeChecker::error()` in `lib.rs:8023` — tries to find symbol position by
  searching source text for backtick-quoted names (fragile, finds first occurrence)
- `TypeChecker::errors: Vec<String>` — plain string accumulator, no structure
- Parser returns `Result<_, String>` with `"LINE:COL: message"` format
- No `NO_COLOR` support — hardcoded ANSI escape codes everywhere
- No error context stack — single-line diagnostics with no breadcrumbs

Token already carries `.line` and `.col` (1-based), set by the Lexer.
Parser has `line_starts: Vec<usize>` and `char_offset(line, col)`.
These provide the foundation for span derivation without AST changes.

## Design

### Sub-step 1: Foundation types + improved display (DONE)

Added to `lib.rs` PART 2b:

- `Span { start: usize, end: usize }` — byte offsets into source text
  - `Span::dummy()` for nodes not yet tracked
  - `Span::merge()` for compound expressions
  - `start_line_col()` / `end_line_col()` to convert to human-readable
- `Diagnostic { span, message, severity, notes, context }` — structured error
  - `Diagnostic::error()`, `error_at()` — constructors
  - `.with_note()`, `.with_context()` — builder pattern
  - `.display(source, filename, use_color)` — renders with underlines
- `should_use_color()` — checks `NO_COLOR` env var and `TERM=dumb`
- `display_error_in(source, error, filename)` — filename-aware error display
- `line_col_to_span()` — converts legacy `LINE:COL` format to Span
- Updated all 10 `display_error()` call sites in `runa.rs` to use
  `display_error_in()` with actual filename

### Sub-step 2: TypeChecker uses Diagnostic (TODO)

Replace `errors: Vec<String>` with `errors: Vec<Diagnostic>`.
TypeChecker methods push structured diagnostics with span info
derived from the statement being checked (since Stmt will get spans).

Add error context stack: push "in function `X`" when entering a Defn::Fn,
pop when leaving. Each Diagnostic inherits current context.

### Sub-step 3: Fix silent failures (TODO)

- `parse_int("")` → return default `0` but emit a warning, OR change
  to return `Result` (breaking change — needs migration path)
- `parse_float("")` → same
- Audit all builtins in interpreter (lib.rs ~5500-6200) for swallowed errors

### Sub-step 4: Eliminate dangerous unwraps (TODO)

Audit categories:
- **Database builtins** (`db_open().expect(...)`) — replace with Result propagation
- **File I/O** (`fs::write().unwrap()`) — replace with error message
- **JSON** (`serde_json::to_string().unwrap()`) — safe (can't fail on valid input)
- **Parser internals** (`tokens.last().unwrap()`) — safe (EOF always appended)

Target: zero `.unwrap()` in user-facing code paths.

### Sub-step 5: AST spans (TODO — largest change)

Two approaches considered:

**A. ExprKind rename** — Rename `Expr` → `ExprKind`, create new
`struct Expr { kind: ExprKind, span: Span }`. Cleanest design but
touches ~1,366 match sites across both files.

**B. Parallel span storage** — Keep Expr as-is, store spans in a
side table keyed by AST node ID. Less invasive but fragile.

Decision: Approach A (ExprKind rename) is the right long-term choice.
Defer to a dedicated sub-step since it's purely mechanical but large.

### Sub-step 6: Parser captures spans (TODO — after sub-step 5)

In every parser production, capture `start_token = self.peek()` before
parsing and `end_token = self.tokens[self.pos-1]` after. Derive span
from token line/col converted to byte offsets via `char_offset()`.

## Checklist

- [x] `Span` struct with `dummy()`, `merge()`, `start_line_col()`, `end_line_col()`
- [x] `Diagnostic` struct with `error()`, `error_at()`, `.with_note()`, `.with_context()`, `.display()`
- [x] `should_use_color()` — `NO_COLOR` + `TERM=dumb` support
- [x] `display_error_in()` — filename-aware, uses Diagnostic rendering
- [x] All `display_error()` call sites updated to `display_error_in()` with filename
- [x] `byte_offset_to_line_col()` utility
- [x] `line_col_to_span()` for legacy format conversion
- [x] TypeChecker uses `Vec<Diagnostic>` instead of `Vec<String>`
- [x] Error context stack in TypeChecker (`push_context`/`pop_context`)
- [x] Context breadcrumbs on function, actor, and module checking
- [x] `check_with_diagnostics()` returns structured `Vec<Diagnostic>`
- [x] `run_type_check()` helper in runa.rs — displays structured errors with color support
- [x] LSP `diagnostic_to_lsp()` — converts Diagnostic to LSP JSON with proper ranges
- [x] Fix `parse_int("")` — now warns to stderr, suggests monadic bind
- [x] Fix `parse_float("")` — same
- [x] Audit and fix dangerous `.unwrap()` calls (12 fixed in compiler code paths, generated code deferred)
- [x] `Expr` → `ExprKind` rename + new `Expr { kind, span }` struct (736 renames + ~600 construction/match fixes)
- [x] `From<ExprKind> for Expr` + `From<ExprKind> for Box<Expr>` for ergonomic migration
- [x] Parser captures real spans: atoms (Var, Lit), operators (BinOp, Pipe, Try), field access, function calls
- [x] TypeChecker `error_at_expr()` uses AST spans directly — multi-char underlines
- [ ] Same for `Stmt` → `StmtKind` (deferred — Stmt has fewer match sites, lower priority)
- [ ] Same for `Pat` → `PatKind` (deferred — same)

## Files Modified

| File | Change |
|------|--------|
| `src/lib.rs` | Added PART 2b (Span, Diagnostic, should_use_color), updated display_error, TypeChecker → Diagnostic, context stack, parse_int/parse_float warnings |
| `src/bin/runa.rs` | All display_error → display_error_in with filename, run_type_check helper, diagnostic_to_lsp for LSP |

## Tests

| Test | What it proves |
|------|---------------|
| `tests::span_dummy_is_zero` | Span::dummy() has zero offsets and is_dummy() returns true |
| `tests::span_merge_covers_both` | Merging two spans produces min-start to max-end |
| `tests::span_merge_with_dummy_returns_other` | Dummy span doesn't pollute merges |
| `tests::byte_offset_to_line_col_first_line` | Offset→line:col on single-line source |
| `tests::byte_offset_to_line_col_multiline` | Offset→line:col across newlines |
| `tests::span_start_line_col` | Span→line:col for start and end |
| `tests::diagnostic_error_no_span` | Diagnostic without location |
| `tests::diagnostic_error_at_span` | Diagnostic with span carries it |
| `tests::diagnostic_error_at_dummy_has_no_span` | Dummy span → None in Diagnostic |
| `tests::diagnostic_with_note_and_context` | Builder pattern adds notes + context |
| `tests::diagnostic_display_no_color` | No ANSI in output, shows file:line:col, underline |
| `tests::diagnostic_display_with_context_breadcrumbs` | Context trail appears in output |
| `tests::no_color_env_disables_color` | should_use_color() exists and returns bool |
| `tests::typechecker_undefined_function_produces_diagnostic` | Undefined fn → Diagnostic with message |
| `tests::typechecker_wrong_arity_produces_diagnostic` | Wrong arity → Diagnostic with counts |
| `tests::typechecker_context_breadcrumb_on_function_error` | Error inside fn has "in function `X`" context |
| `tests::typechecker_diagnostic_has_span` | Type error Diagnostics carry span info |

Run with: `cargo test --lib`

## Verification

```bash
cargo build --release              # Compiles clean
cargo test --lib                   # 17 unit tests pass
./target/release/runa test         # All 69 .runa tests pass
NO_COLOR=1 ./target/release/runa check examples/weather_demo.runa  # No ANSI codes
```

## Notes

- The ExprKind rename (sub-step 5) is ~1,366 mechanical changes. Consider
  using a script or doing it in a dedicated session.
- `find_symbol_in_source` is fragile — it finds the FIRST occurrence of
  a symbol name, which may be the wrong one. AST spans (sub-step 5+6)
  will eliminate this entirely.
- The `Span` uses byte offsets, not line:col, because byte offsets
  compose better (merge two spans = min start, max end) and are cheaper.
  Line:col is derived on demand for display.
