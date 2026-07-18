# M28: Negative Tests + CLI Polish

**Tagline:** "A test suite that proves what doesn't work."

## Goal

Add tests that verify the compiler produces correct error messages for
bad input, and polish the CLI to handle edge cases gracefully. Today all
69 tests are happy-path — zero tests verify error output.

## Context

M27 delivered structured Diagnostics with spans and underlines. Now we
need tests that prove these errors fire correctly, and CLI polish to
prevent user confusion (unknown flags silently ignored, no --version, etc.)

## Sub-steps

### Sub-step 1: Negative test infrastructure

**Change:** Add `tests/errors/` directory. Each test is a `.runa` file
with a comment header declaring expected behavior:
```runa
-- expect-error: undefined function
= x = nonexistent()
```

The test runner discovers these, runs each file, asserts non-zero exit
and that stderr contains the expected substring.

**Test:** `runa test tests/errors` runs the negative tests and reports pass/fail.

### Sub-step 2: Parse error tests (10+)

**Change:** Write .runa files that trigger specific parse errors.

**Test:** Each file produces the expected error message.

### Sub-step 3: Type error tests (10+)

**Change:** Write .runa files that trigger type checker errors.

**Test:** Each file produces the expected error message with correct span.

### Sub-step 4: CLI polish

**Change:**
- `--version` flag
- Error on unknown flags (not silently treat as filename)
- `--quiet` flag

**Test:** `runa --version` prints version. `runa frobnicate` prints error.

## Checklist

- [ ] `tests/errors/` directory with negative test infrastructure
- [ ] Test runner support for error tests (expect-error comments)
- [ ] 10+ parse error test files
- [ ] 10+ type error test files
- [ ] `--version` flag
- [ ] Error on unknown commands
- [ ] All existing tests still pass
- [ ] Negative tests integrated into `runa test`

## Files Modified

| File | Change |
|------|--------|
| `src/bin/runa.rs` | Test runner + CLI flags |
| `tests/errors/*.runa` | New negative test files |

## Tests

| Test | What it proves |
|------|---------------|
| `tests/errors/*.runa` | Each error case produces expected diagnostic |

## Verification

```bash
cargo build --release
runa test                    # All 69 happy-path tests pass
runa test tests/errors       # All negative tests pass
runa --version               # Prints version
runa frobnicate              # Prints "unknown command"
```
