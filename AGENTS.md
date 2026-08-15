# Futuruna Agent Guide

Futuruna is a programming language for ordinary programs and formal rule
models. This file is the shared operating contract for agents working in this
repository. Keep it short and stable: route to maintained sources instead of
copying feature lists, test counts, milestones, or CLI help here.

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
  implementation truth. Do not infer the current architecture from an old
  commit, milestone, binary, or generated artifact.
- For Futuruna setup, `.runa` authoring, rule modeling, calculations, Rust
  interop, or compiler semantics, use the repo skill at
  `.agents/skills/futuruna/SKILL.md` (`$futuruna` in Codex).

## Authority and Artifact Boundaries

Current code, tests, contracts, and maintained reference documentation on the
checked-out branch outrank historical or derivative material.

- `docs/milestones/`, historical design sketches, `wiki/`, `WIKI.md`, commit
  messages, reflogs, and the `archive/semantic-lineage` tag are context, not a
  present-tense specification.
- `.raw/`, `research/`, `paper/`, and legacy examples may be primary evidence
  for a task that explicitly targets them, but they do not override current
  language contracts.
- Root executables such as `parser`, `parser-audit`, `parser-e2e`, `test_build`,
  `ownership_linked_list`, and `store_v3_dump` are retained artifacts, not
  portable build products or current entry points.
- Ignore local/generated state such as `target/`, `website/target/`,
  `.runa-build/`, `deploy/`, `outputs/`, `.wrangler/`, runtime database files,
  `.DS_Store`, and editor state unless the task explicitly targets it.
- Do not ignore tracked contracts merely because they look generated.
  `Cargo.lock`, `docs/feature-stages.json`, and reviewed expectation goldens are
  authoritative tracked inputs.

This repository has undergone deliberate history rewrites. Treat archival and
local-only refs plus external recovery artifacts as read-only. Do not restore,
delete, move, publish, expire, garbage-collect, or rewrite them without explicit
task-specific authorization. Resolve current refs with Git; do not reuse copied
object IDs or trust a stale `.git/info/refs`. Never publish
`codex/semantic-interface-cache` unless the user explicitly names that ref.

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
- Do not use `git reset --hard`, `git clean`, stash manipulation, reflog expiry,
  aggressive GC, ref deletion, or history rewriting unless explicitly required
  by the task and independently checked.
- Ask before installing software, changing global configuration, publishing,
  deploying, or mutating external services unless the user's request already
  grants that exact authority.
- Never inspect, copy, upload, or commit ignored personal PDFs, tax records,
  generated workbooks, or case results unless the user explicitly places them
  in scope. Keep private case work outside the checkout. Do not recover deleted
  personal material from Git history or recovery stores unless an explicitly
  authorized privacy or recovery task requires it.
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
  tool choreography, cleanup history, and repository archaeology out of
  user-facing copy unless they materially affect the user's decision.
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
