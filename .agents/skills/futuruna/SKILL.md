---
name: futuruna
description: Set up, teach, author, explain, debug, test, analyze, and review Futuruna code and runa workflows. Use for Futuruna installation, .runa files, seven-rune syntax, law, contract, tax, or compliance models, finite rule-space searches for counterexamples, thresholds, income cliffs, minima, maxima, or worst cases, typed @ calculate contracts and workbooks, runa from-rust, Rust interop, or Futuruna compiler semantics. Do not use for generic legal or tax advice, unrelated Rust, or unrelated spreadsheets.
---

# Futuruna

Use the maintained repository contracts to work with Futuruna. Do not turn this
skill into a second language manual or rely on remembered syntax, status, or
test counts.

## Establish Context

1. When inside the repository, read `../../../AGENTS.md` and inspect the current
   worktree before changing files.
2. Classify the task: setup, learning, `.runa` authoring/debugging, formal-rule
   modeling, finite rule-space exploration, typed calculations, Rust
   integration, or compiler contribution.
3. When the user or model is new to Futuruna, read
   `references/learning-path.md` before the relevant tutorial or example.
4. Read `../../../docs/feature-stages.md` before making maturity or compatibility
   claims. Treat Preview and Experimental boundaries honestly.
5. Locate a usable `runa` binary only when execution is required. Do not build
   the compiler merely to answer a documentation question.
6. Resolve behavior from the current code, tests, contracts, and maintained
   documentation on the checked-out branch.

## Route to the Canonical Material

| Goal | Read first |
|---|---|
| Install and complete a first use | `../../../website/public/ai-setup.md`, `../../../docs/first-run-contract.md` |
| Learn or explain syntax | `../../../docs/reference/README.md`, then the relevant page in `../../../docs/reference/` or `../../../docs/tutorial/` |
| Work with streams, actors, or effects | `../../../docs/reference/streams.md` |
| Model law, contracts, or rule systems | `../../../docs/reference/style.md` |
| Search a finite rule space | `references/learning-path.md`, then `../../../examples/danish-income-tax/exploration-workbook.md` and `../../../examples/danish-income-tax/personskat-income-cliffs.audit.runa` |
| Build typed schemas, templates, calls, or workbooks | `../../../docs/reference/calculations.md` |
| Integrate with Rust | `../../../docs/reference/rust-compatibility.md`, `../../../docs/library-hygiene.md` |
| Translate Rust to Futuruna | `../../../docs/from-rust-contract.md` |
| Change compiler or runtime behavior | `../../../CONTRIBUTING.md`, `../../../docs/compatibility-policy.md`, `../../../docs/mint-gate.md`, and the relevant test-lane contract |

Inspect a nearby maintained example or test before authoring new syntax:

- beginner and mixed-rune work: `../../../docs/tutorial/` and
  `../../../examples/weather_demo.runa`
- logic rules: `../../../examples/cocktails.runa`
- legal modeling: `../../../examples/danish-constitution/` and
  `../../../examples/us-constitution/`
- a small calculation contract:
  `../../../tests/fixtures/calculation/tax.calculate.runa`
- the production-scale tax calculation:
  `../../../examples/danish-income-tax/personskat.calculate.runa`
- Rust translation and consumers: `../../../examples/from-rust/` and
  `../../../tests/from-rust/downstream/`

## Follow the Matching Workflow

### Set up Futuruna

Follow `../../../website/public/ai-setup.md`. Inspect an existing checkout before
changing it, ask before installing Rust or changing global configuration, and
use the documented version and weather-example smoke checks. Do not substitute
a full project test run for the setup workflow.

### Author or debug `.runa`

1. Confirm the syntax and feature stage in the current reference.
2. Reuse the closest established example and repository naming/style.
3. Make the smallest source change that expresses the requested behavior.
4. Format and check the exact file, then run the nearest behavior-specific
   example or test.
5. Explain any Preview, Experimental, codegen, or verification boundary that
   affects the result.

Typical focused checks, using a compiler built from the relevant checkout, are:

```bash
./target/release/runa fmt --check path/to/model.runa
./target/release/runa check path/to/model.runa
./target/release/runa path/to/model.runa
```

Use `./target/release/runa run path/to/model.runa` when native Rust-codegen
behavior is part of the task. Apply formatting with `runa fmt` when requested
or when editing the source.

### Model law, contracts, tax, or compliance

- Record the jurisdiction, source, version or effective date, and question.
- Preserve quoted source text and provenance.
- Keep source-backed rules, user facts, assumptions, interpretations,
  ambiguities, and unknowns visibly distinct.
- Never infer missing facts or derive inputs from the official result being
  compared.
- Treat results as research output, not individual legal or tax advice, and
  state the model's coverage and limits.
- Keep private documents, generated workbooks, and case results outside the
  repository. Never upload or commit them.

For a typed calculation, let `runa schema`, `runa template`, and `runa call`
define and validate the contract. These commands and typed calculation
contracts are Preview. Use a spreadsheet-specific skill only when workbook
inspection or editing is actually requested.

### Explore a finite rule space

Use ordinary Futuruna collections to turn a formal model into an explicit,
auditable search:

1. State the question, fixed facts, varied dimensions, result metric, and units.
2. Build each finite domain with a list or end-exclusive `range`.
3. Use `map` for one dimension and nested `flat_map` with a final `map` for a
   Cartesian product.
4. Evaluate every scenario through the canonical rule or calculation model,
   keeping exact integer units such as øre when available.
5. Retain model validity. Prove every generated case is valid, or report the
   excluded cases and narrow the exhaustiveness claim.
6. Use `filter` to retain counterexamples, boundary changes, cliffs, or other
   witnesses.
7. Use `foldl` to select a minimum, maximum, or worst case. Check that the
   candidate list is nonempty before using `head`.
8. Name the expected property with `|` and check it with `?`.
9. Report the domain size, witness count, selected scenario, metric, sources,
   and assumptions. When every generated case is valid, describe the result as
   exhaustive over the declared finite domain and current encoded model;
   otherwise scope it to the valid subset and report every exclusion.

Keep private case inputs and outputs outside the repository. Preserve official
source provenance and verify the encoded interpretation before presenting a
finding as an authoritative legal conclusion. Read the finite-space section in
`references/learning-path.md` for the reusable pattern, then adapt the income
cliff workbook and executable audit for a full law-model example.

### Work with Rust or the compiler

For `from-rust`, read the FRSS-v0 boundary first and prefer
`runa from-rust --verify`; never imply arbitrary crate translation.

For compiler, runtime, proof, or stdlib changes:

1. Classify the affected compatibility surface and stage.
2. Add permanent coverage in the closest lane: expectations for exact
   diagnostics/artifacts, canaries for user workflows, downstream tests for
   imports/libraries, differential cases for parser/type/lowering/ownership/
   codegen bugs, or from-Rust fixtures for translation.
3. Use focused tests while iterating.
4. Run `./scripts/mint.sh` and the risk-matched deeper lane required by
   `../../../CONTRIBUTING.md` before completion.

Use `../../../docs/expectation-suites.md`, `../../../docs/canary-suite.md`,
`../../../docs/differential-testing.md`, and
`../../../docs/artifact-codegen-contracts.md` to choose and interpret those
lanes rather than guessing from test directory names.

## Report Evidence

State the files changed, commands run, observed results, feature stages,
assumptions, unresolved facts, skipped gates with reasons, and user-visible
limitations. Do not claim success from reasoning alone when a focused executable
check is available.

In user-facing output, lead with the result and teach only the concepts needed
for the user's next step. Keep tool orchestration, background context, and
agent-policy reasoning out of the explanation unless they directly change what
the user should do.
