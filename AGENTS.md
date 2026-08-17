# Futuruna Agent Guide

Futuruna is a programming language for ordinary programs and formal rule
models. This file is the shared operating contract for agents working in this
repository. Keep it short and stable: route to maintained sources instead of
copying volatile details or CLI help here.

## Read the Current Sources

- Start with `README.md` for the product overview.
- Use `docs/reference/README.md` and `docs/tutorial/` for language behavior and
  examples.
- Check `docs/feature-stages.md` or `docs/feature-stages.json` before claiming a
  feature is Stable, Preview, or Experimental. Apply
  `docs/compatibility-policy.md` when changing a public surface.
- Follow `CONTRIBUTING.md` for the semantic-change ratchet and test selection.
  `docs/mint-gate.md` and the other test-lane documents define the gates.
- Use `website/public/ai-setup.md` for installation, first-use, privacy, and
  legal-modeling setup. `docs/first-run-contract.md` defines the stable new-user
  path.
- Read `Cargo.toml`, `Cargo.lock`, and the current source and tests for
  implementation truth.
- For Futuruna setup, `.runa` authoring, rule modeling, calculations, Rust
  interop, or compiler semantics, use the repo skill at
  `.agents/skills/futuruna/SKILL.md` (`$futuruna` in Codex).
- For counterexamples, thresholds, income cliffs, extrema, and other finite
  rule-space analysis, start with
  `examples/danish-income-tax/exploration-workbook.md` and its executable
  `personskat-income-cliffs.audit.runa` companion. Futuruna expresses the
  search with list literals/`range`, `map`/`flat_map`, `filter`, `foldl`, and a
  named invariant checked with `?`.

## Authority and Artifact Boundaries

Current code, tests, contracts, and maintained reference documentation on the
checked-out branch define product behavior.

- Treat tracked contracts such as `Cargo.lock`, `docs/feature-stages.json`, and
  reviewed expectation goldens as authoritative inputs.
- Ignore local and generated state unless the task explicitly targets it.

## Start and Scope Work

1. Run `td usage --new-session` at conversation start or after a context reset.
2. Inspect `git status --short --branch` before editing. Preserve unrelated and
   user-authored changes; stage only the intended paths.
3. Read the closest contract, example, and test before changing behavior.
4. Make the smallest coherent change. Avoid duplicating durable documentation
   into agent instructions or skills.

Use `td` for project work:

```bash
td ready
td show <id>
td start <id>
td log "message"
td handoff <id>
td review <id>
```

Do not edit `.todos/` directly. Completed implementation goes through
`td review`; a different session uses `td approve` or `td reject`.

## Safety and Privacy

- Validate exact targets before file operations. Use non-interactive flags only
  after the target is known; never normalize recursive deletion as routine
  cleanup.
- Do not rewrite Git refs or history, or delete repository artifacts, unless the
  task explicitly requires it and the exact target has been independently
  checked.
- Ask before installing software, changing global configuration, publishing,
  deploying, or mutating external services unless the user's request already
  grants that exact authority.
- Never inspect, copy, upload, or commit ignored personal PDFs, tax records,
  generated workbooks, or case results unless the user explicitly places them
  in scope. Keep private case work outside the checkout.
- Futuruna legal and tax models are research software, not individual advice.
  Preserve source text and provenance; distinguish facts, assumptions,
  interpretations, and unknowns; never invent personal or legal facts.

## Choose Proportional Checks

- Documentation or instruction-only work: run the narrowest syntax/link/content
  checks plus `git diff --check`.
- `.runa` changes: run targeted `runa fmt --check`, `runa check`, and the closest
  behavior scenario or test.
- Rust implementation changes: run `cargo fmt --check` and focused Rust tests
  while iterating.
- Compiler, runtime, proof, stdlib, or other semantic changes: add permanent
  coverage, run `./scripts/mint.sh`, and run the risk-matched deeper lane from
  `CONTRIBUTING.md`.
- Do not run every expensive suite for unrelated or documentation-only work.
  Never weaken a required gate merely because it is slow.

Report the exact commands run, their results, skipped gates with reasons, and
remaining uncertainty.

## Communicate for the User

- Lead with the useful answer, artifact, or next action. Keep internal planning,
  tool choreography, and background context out of user-facing copy unless they
  materially affect the user's decision.
- Explain Futuruna from the user's present goal. Prefer a small working example
  and a clear next step over an exhaustive feature tour.
- Mention limitations when they affect correctness, safety, compatibility, or
  expectations; do not turn every answer into a catalogue of non-goals.
- Preserve the user's requested wording and voice in public-facing text. Fix or
  reinterpret it only within the scope the user authorized.

## Finish Tracked Implementation

Before completing an authorized implementation task:

1. Inspect the final diff and `git status`.
2. Run the proportional checks.
3. Record progress with `td log`, then `td handoff` and `td review`.
4. Commit only the scoped files.
5. Verify the current branch/upstream and inspect the exact ahead range. Do not
   publish unrelated commits or refs.
6. From a clean worktree, run `git pull --rebase`, recheck the ahead range, then
   `git push`.
7. Verify `git status --short --branch` shows the branch up to date.

Tracked implementation requested for delivery is not complete until its commit
is pushed. Read-only audits, reviews, and explanations do not acquire write or
push authority from this section.
