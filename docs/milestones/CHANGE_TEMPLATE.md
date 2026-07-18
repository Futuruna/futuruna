# M??: Title

**Tagline:** "..."

## Goal

What this change achieves and why it matters. 2-3 sentences max.

## Context

What exists today that this change improves or replaces.
Link to relevant code (file:line), existing behavior, or prior milestones.

## Design

How the change works. Key decisions and their rationale.
If there are alternatives considered, note why they were rejected.

## Sub-steps

Break the milestone into incremental pieces. Each sub-step MUST include
its own test before moving to the next. No sub-step is done until its
test passes.

### Sub-step 1: ...

**Change:** What to implement.

**Test:** How to prove it works — a specific command, expected output,
or test file. The test should be committed alongside the change.

### Sub-step 2: ...

(repeat)

## Checklist

- [ ] Sub-step 1 + test
- [ ] Sub-step 2 + test
- [ ] ...
- [ ] All existing tests still pass (`runa test`)
- [ ] Milestone test file committed (`tests/m??_test.runa` or `tests/errors/m??_*.runa`)

## Files Modified

| File | Change |
|------|--------|
| `src/lib.rs` | ... |

## Tests

Each sub-step adds or updates tests. List them here as they land:

| Test | What it proves |
|------|---------------|
| `tests/...` | ... |

## Verification

Final verification after all sub-steps complete:
- `cargo build --release` — clean
- `runa test` — all pass (old + new)
- Manual smoke test commands

## Notes

Anything discovered during implementation. Gotchas, surprises,
decisions deferred. Update this section as work progresses.
