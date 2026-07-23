# Contributing

Futuruna is only useful if contributors can change it without silently changing
the language under users' feet. The rule here is simple: semantic changes must
raise the safety floor, not just pass today.

## Semantic Change Ratchet

A semantic change is any change that can alter what a Futuruna program means,
emits, or proves. In practice that includes parser, typechecker, interpreter,
codegen, ownership, stdlib builtin behavior, proof/verify, and runtime changes.

Before a semantic change is submitted for review:

1. Add permanent coverage.
   Bug fixes and semantic changes must land with at least one durable guard:
   an ordinary regression in `tests/`, an authored canary in
   `tests/canary/`, a minimized repro in `tests/differential/corpus/`, or an
   internal phase validator/snapshot in Rust tests.
2. Run the baseline gate.
   `./scripts/mint.sh` is the minimum contract for compiler/runtime behavior
   changes.
3. Run the relevant deeper lane.
   Use the lane that matches the risk you touched:
   - `./scripts/canary.sh core` or a relevant tier for user-shaped workflow
     changes.
   - `./scripts/differential.sh` for parser, type inference, lowering,
     ownership, codegen, or bugs found through stress generation.
   - targeted `cargo test --quiet ...` and `runa verify ...` coverage for
     proof/verify changes, in addition to `./scripts/mint.sh`.
4. Explain the contract.
   The review request must say what semantic contract changed, what permanent
   coverage was added, and which commands were run.
5. Park follow-up debt explicitly.
   If you leave a shortcut, workaround, or known gap behind, file a `td-*` task
   before merge. Do not leave semantic debt implicit.

## Review Expectations

Semantic/compiler/runtime pull requests should be reviewable without guesswork.
The PR description should include:

- the user-visible or compiler-internal semantic contract that changed
- the exact regression, canary, differential corpus case, or snapshot added
- the exact verification commands that were run
- any skipped lane, with a concrete reason
- any parked follow-up work as linked `td-*` tasks

Every real semantic bug should become permanent coverage somewhere in the tree.
Never merge a semantic fix that relies only on a verbal explanation.

## Non-Semantic Changes

Pure docs, comments, or clearly non-behavioral refactors do not need the full
semantic ratchet. Keep the diff honest and run the smallest relevant checks.
