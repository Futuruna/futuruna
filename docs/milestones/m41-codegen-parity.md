# M41: Codegen Parity — All Tests Compile

**Tagline:** "If it runs in the interpreter, it compiles to Rust."

**Status:** In progress.

## Goal

Fix all remaining `runa test --check-codegen` failures so every test file
that runs in the interpreter also produces valid Rust. Make --check-codegen
blocking in CI.

Baseline: 51 pass, 15 fail, 16 skipped.
Target: 66 pass, 0 fail, 16 skipped.

## Verification

```bash
runa test --check-codegen    # 0 failures
```
