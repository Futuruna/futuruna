# M41: Codegen Parity — All Tests Compile

**Tagline:** "If it runs in the interpreter, it compiles to Rust."

**Status:** DONE.

## Result

65 pass, 0 fail, 18 skipped. All codegen failures eliminated.
`--check-codegen` is now a CI gate — any regression fails the build.

## Key Fixes

- TCE boxed deref (Rc-wrapped recursive ADT fields in tail-call match arms)
- Print hoisting (consuming calls separated from field accesses in println!)
- HashMap type inference prescan (map_insert determines value type)
- Lambda scope isolation (params don't inherit outer string/float types)
- Effect handlers use &self (enables Clone on closures in HOFs)
- show() uses __futuruna_show_any (Debug format with .0 stripping)
- Prolog value-returning rules return "" on no match
- findall/search query variables treated as bound, not captured

## Verification

```bash
runa test --check-codegen    # 65 passed, 0 failed, 18 skipped
runa test --roundtrip        # 49 matched, 0 diverged
```
