# M42: Error Recovery — Multiple Errors Per Parse

**Tagline:** "Show five errors, not one."

**Status:** DONE.

## Result

Parser now continues after errors by synchronizing to the next statement
boundary (rune character at column 1). Collects up to 10 errors before
stopping. Both `runa check` and `runa run` report all errors at once.

Example:
```
$ runa broken.runa
3 parse errors:
3:11: unexpected newline
5:20: expected a type name, got `{`
10:1: Futuruna uses `=` to bind values, not `let`.
  Try: = w
```

## Implementation

- `parse_program()` catches errors from `parse_statement()` instead of `?` propagation
- `synchronize()` skips tokens until it finds a statement boundary:
  rune characters (#, >, |, =, ~, @, ?) at column 1, or keywords (for, while, if)
- Up to 10 errors collected, then stops
- Successfully parsed statements before/between errors are preserved
- Type checker already supported multiple errors via `Vec<Diagnostic>`
- 14 negative tests pass (including multi-error recovery test)
