# M33: Trait Resolution

**Tagline:** "Types that implement behaviors."

**Status:** In progress.

## Goal

Add trait-aware checking to the TypeChecker so missing impl methods
and bad method signatures are caught at the Futuruna level, not by rustc.

## Sub-steps

### Sub-step 1: Trait registry in TypeChecker

**Change:** TypeChecker collects `# trait` declarations: name → required methods
with signatures. TypeChecker collects `# impl` blocks: trait + type → methods.

**Test:** Trait and impl registered correctly.

### Sub-step 2: Impl completeness check

**Change:** After collecting all declarations, verify each impl block provides
all methods required by its trait. Report missing methods as Diagnostics.

**Test:** Missing method produces Futuruna error, not rustc error.

### Sub-step 3: Error test for missing impl method

**Change:** Add negative test in tests/errors/.

**Test:** `runa test tests/errors` catches the new error.

## Checklist

- [ ] TypeChecker collects trait declarations (name → methods)
- [ ] TypeChecker collects impl blocks (trait + type → methods)
- [ ] Missing method check after declarations collected
- [ ] Diagnostic with span for missing methods
- [ ] Negative test for incomplete impl
