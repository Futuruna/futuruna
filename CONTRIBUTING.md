# Contributing

Futuruna favors a correct, clear, coherent language over preserving older
choices. During development toward 0.2.3, better designs may break compatibility.
Update the implementation, tests, references, and repository programs together;
semantic changes must raise the safety floor, not just pass today.

The classification model for those changes lives in
`docs/compatibility-policy.md`. Use it when deciding whether a change is a
source, behavioral, verification, or artifact-facing compatibility change, and
whether the affected surface is experimental, preview, or stable.

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
   coverage was added, which compatibility category it touched, and which
   commands were run.
5. Update the current references and programs.
   Describe the resulting language directly, without adding historical or
   migration notes. For tracked stable-surface changes, use the PR template's
   compatibility field to identify those updates and explain why a versioned
   guide entry is unnecessary under the current development policy.
6. Park follow-up debt explicitly.
   If you leave a shortcut, workaround, or known gap behind, file a `td-*` task
   before merge. Do not leave semantic debt implicit.

## Large Corpus Loop

For repeated checks of large `.runa` corpora, use the persistent optimized
compiler profile:

```bash
./scripts/runa-corpus.sh check path/to/model.calculate.runa
./scripts/test-corpus.sh --test calculate_cli test_name
```

Ordinary `cargo test` remains the right loop while changing the Rust compiler.
The corpus profile deliberately trades a slower one-time compiler build for
faster repeated compiler and interpreter execution.

## Review Expectations

Semantic/compiler/runtime pull requests should be reviewable without guesswork.
The PR description should include:

- the user-visible or compiler-internal semantic contract that changed
- the exact regression, canary, differential corpus case, or snapshot added
- the current references and repository callers updated with the contract
- the exact verification commands that were run
- any skipped lane, with a concrete reason
- any parked follow-up work as linked `td-*` tasks

Every real semantic bug should become permanent coverage somewhere in the tree.
Never merge a semantic fix that relies only on a verbal explanation.

## Non-Semantic Changes

Pure docs, comments, or clearly non-behavioral refactors do not need the full
semantic ratchet. Keep the diff honest and run the smallest relevant checks.

For repository layout, documentation links, and generated-file hygiene, run:

```sh
python3 scripts/repository-hygiene.py
git diff --check
```

Follow the [repository map](docs/repository-layout.md) when adding or moving
files. Keep generated executables and runtime databases out of Git; preserve
reference paths through a recorded migration and update their consumers.

For VS Code syntax grammar or language-configuration changes, run
`npm test --prefix editors/vscode` after the dependency setup in the
[editor guide](editors/vscode/README.md). These tests exercise the actual
TextMate tokenizer, including multiline state and scope precedence; CI runs
them in its independent editor job.

## Releases

Public tags, crates.io publication, release binaries, checksums, and macOS
signing follow the maintainer [release runbook](docs/releasing.md). Do not push a
version tag as a substitute for completing its release preflight.
