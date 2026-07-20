# M42: Error Recovery — Multiple Errors Per Parse

**Tagline:** "Show five errors, not one."

## Goal

Parser currently stops at the first error. Professional languages (Rust, Go,
TypeScript) collect multiple errors and report them all. Critical for editor
experience — the LSP should show all problems, not just the first.

## Approach

Add synchronization points: on error, skip to the next statement boundary
(next rune character at column 0), then continue parsing. Collect up to 10
errors before stopping.
