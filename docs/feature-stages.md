# Futuruna Feature Stages

This document makes the current stage of major Futuruna surfaces visible to
users and contributors. The same contract is exposed mechanically in
[docs/feature-stages.json](feature-stages.json) and through:

```bash
runa feature-stages --json
```

It should be read alongside [docs/compatibility-policy.md](compatibility-policy.md):

- the compatibility policy defines what the stages mean
- this document says which current surfaces are in which stage
- `docs/feature-stages.json` gives tools a stable schema for querying the same
  assignments

For bounded exploration, publication v20 and compact report v11 are current.
One exact-finite relation and admission can now feed zero or more named finds,
with explicit result/mechanism consumers and per-question case-graph layers.
Case/support schema v4 is a discovery-ordered append-only stable-key `add`
stream: sparse accepted chunks and regions publish immediately, selected
materializations and authorized cases append later, and exact closure emits a
canonical key-sorted active-set `seal` independent of discovery order. The
classified root-prefix promotion remains operational and is not a graph update.
The concrete plural stream is resumable. Nonempty plural question sets now have
a shared compact classified-sweep implementation: one pass binds the full
canonical ordered question set and run-length encodes packed joint decisions;
per-question counts and projections read that shared evidence, while selected
cases are materialized once for the any-selected union. Native v2 and regional-
proof accelerators remain exact-one and never nominate a primary find. Nested
`choose` now lowers at the checked boundary to a canonical choice relation with
its own membership-only `ChoiceId`; the downstream display keeps a separate
`ViewId`, and mechanisms target the `ChoiceId`. Choice objectives may use
candidate, partition, and measure values, but fail closed if they depend on an
aggregate or `SELECT` display alias. Choice membership has its own journaled
frontier and authenticated content root. After closure, the display iterates
only canonical Choice members and applies its row-local `SELECT` only there; it does
not rescan FIND candidates or repeat the choice policy. Aggregate-backed Choice
display projection fails closed until the member relation carries sufficient
closed-group evidence. A
focused audit paused at transition 17, reopened, and completed one shared 300-
transition sweep with independent exact selected counts of 20 and 10 and one
20-case any-selected materialization; it used neither accelerator.
Compact support observations implement independently domain-separated
inner/outer expression bounds for correlated case support `S` and distinct-
starter projection `P = distinct_sources(S)`, together with explicit starter-
set and correlated-support statuses. Publication v13 and earlier are
implementation history, not compatibility targets. The existing four-case/two-
find smoke has closed through pause/resume; the broad Personskat run remains
later work. The accepted target `? explore` → `? analyze` → `? publish`
contract in the bounded-exploration RFC is steering syntax beyond the current
nested Experimental implementation and has no compatibility promise.

## Language And Runtime Surfaces

| Surface | Stage | Notes |
|---------|-------|-------|
| Core language syntax documented in `docs/reference/basics.md` and `docs/reference/runes.md` | Stable | Changes here are source-compatibility changes unless docs explicitly mark a subsection otherwise. |
| Documented stdlib builtin semantics in `docs/reference/stdlib.md` | Stable | Behavioral changes require compatibility handling or an explicit bug-fix exception. |
| Pure/core generated Rust behavior and reviewed artifact fixtures | Stable | Generated Rust for stable pure/core source must compile on the supported Rust toolchain and preserve documented behavior. Exact emitted text is stable only for named artifact fixtures; helper names and private layout remain internal. See `docs/artifact-codegen-contracts.md`. |
| Importable local libraries and downstream consumer shape | Stable | Local flat and qualified imports, exported values/types/functions, downstream consumer entrypoints, and import-safe helper files are part of the public contract. The contract is backed by the downstream canary lane, import-hygiene linting, import-consumer expectations, `emit --imports` normalization snapshots, and import-aware differential corpus coverage. |
| Exact helper names, private generated layout, and internal compiler layouts | Unstable internal | Not a public compatibility surface unless a doc or artifact expectation explicitly promises it. |
| Explicit kernel proof terms and documented proof-kernel rule forms | Stable | The small kernel-backed proof term surface is part of the published contract. |
| `runa verify` theorem elaboration, solver fallback, and broader verification automation | Preview | Useful and supported, but still evolving as the proof trust boundary and automation pipeline change. |
| Relational bounded `? explore` declarations and exact-finite execution | Experimental | The current nested syntax constructs one ordered finite dependent `from` relation, its finite transition relation, and scoped admission once, then evaluates zero or more named `find` questions (`all`, `matches`, or `violations`) over that shared world. Semantically identical finds alias one `QuestionId`; authored names and order remain publication metadata. Explicit result and mechanism consumers bind to named relations, so there is no public primary or ambient `selected` question. A nested FIND-backed `choose` lowers to an independently journaled membership relation with a `ChoiceId`, open frontier, counts, canonical members and exact `ChoiceContentRoot`; its display projection has a separate `ViewId`. Choice-target mechanisms consume the closed Choice member ordinals and seal against `ChoiceId` plus `ChoiceContentRoot`, without a display identity or result root. Report v11 and publication v20 expose this layer; answer-index schema v2 links each Choice to its display and mechanism consumers. Choice objectives fail closed on aggregate/`SELECT` aliases, and aggregate-backed Choice displays fail closed until the member relation carries sufficient closed-group evidence. A Choice display iterates only the exact canonical members and applies row-local `SELECT` only to those members; excluded FIND candidates never enter its evidence, input count, seal, or projection. Every result descriptor is bounded and self-describing: it names its input, grain, selected schema, grouping keys, honest row/projection counts, evidence roots, and resumable NDJSON artifact without inlining the population. Publication cursors bind the immutable find-address plan and each installed artifact's presentation digest, so renames require a fresh output tree while later additive readers remain possible. The authenticated append-only journal and NDJSON artifacts form a bounded resumable stream. Case/support schema v4 publishes first-arrival stable-key `add` records for accepted sparse chunks and regions without waiting for lower ordinals or selected materialization; materialization and authorized-case records append later, and its exact `seal` commits a canonical key-sorted active-set root. Prefix promotion remains operational. Nonempty plural question sets now share one compact classified pass whose artifacts bind the full canonical ordered `QuestionId` set and run-length encode packed joint admission/selection decisions. Per-question counts and support projections derive from that shared stream, and selected case rows are materialized once for the any-selected union. Native v2 and regional-proof accelerators remain exact-one; zero-question runs use the ordinary concrete fallback. A focused audit paused at transition 17, reopened, and completed one shared 300-transition sweep with independent exact selected counts of 20 and 10 and one 20-case any-selected materialization, without native or regional acceleration. A four-case/two-find smoke on the existing plural path closed through pause/resume with `all_cases = exact(4)`, `interesting = exact(2)`, five state nodes, four U/D transitions, M(`all_cases`) = 4, M(`interesting`) = 2, and one structural mechanism explaining both interesting cases. This is kernel evidence, not a Personskat result: no broad Personskat exploration has run. There is no public probe phase. Admission-scoped mechanism target execution, arbitrary path-conditioned starter selectors, parallel workers, general late symbolic materialization, and broad multidimensional Personskat closure remain deferred. |
| Reactive/stateful surfaces such as streams, subjects, actors, and effect-heavy workflows | Stable | User-facing and documented, with explicit named-scope ownership for live subscriptions, compiled stateful canaries, adversarial workflow coverage, and async emitted-Rust artifact expectations. Generic roundtrip/check-codegen skips for live async files are explicit and not treated as pass evidence. |
| Rust interop and Rust-facing library integration behavior | Stable | `runa lib` output, exported Rust-facing API shape, `@ use`, `@ depend`, raw `@ rust` helpers, dependency guidance for generated Cargo consumers, external-crate generated code, and the documented module/Cargo-library consumer layouts are covered by blocking canaries. Exact helper/private layout and automatic manifest generation remain outside the stable contract; `runa from-rust` has a separate stable FRSS-v0 single-file validation boundary. |
| Typed `@ calculate` boundaries and `futuruna.calculate.v1` contracts | Preview | One typed rule or function boundary can derive a versioned contract and canonical value model without changing normal rule semantics. An optional string labels the whole calculation; typed field-target metadata adds per-input labels, interview questions, help, units, and source traces while canonical paths remain machine keys. `pathof(...)` checks exact paths, while type-anchored `refof(Type::member)` metadata is projected across reusable nested domain models with deterministic override rules. The declaration, schema, and adapters are being hardened through production corpus use. |

## Tooling And Command Surfaces

| Command family | Stage | Notes |
|---------------|-------|-------|
| `runa run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes`, `lib`, `feature-stages` | Stable | These are core workflow and Rust-facing library commands. Their documented behavior is part of the normal public surface. `emit --imports` is the stable public import/export graph snapshot under the importable-library contract. `feature-stages` is stable through the versioned JSON schema. |
| `runa init <name>` first-run scaffold | Stable | The documented first-run scaffold contract is stable: create `runa.toml`, create `src/main.runa`, and produce a project that immediately passes `check`, `fmt --check`, `run`, and `build`. Broader package/project workflow behavior remains preview unless another contract names it. |
| `runa lint-library` | Stable | Import-hygiene tooling for authored library surfaces, including import-graph checks and helper-call-chain impurity rejection. |
| `runa stress-gen` and `./scripts/differential.sh` | Stable | Differential replay and generative compiler testing are stable as a quality gate: checked-in replay corpus, stable seed lists, failure artifacts, and generated import-aware pressure are part of the documented hardening contract. Generator internals and corpus volume may still grow. |
| `runa add`, `wasm`, `lsp` | Preview | Useful project and integration tooling, but still subject to package, interface, or behavior refinement. |
| `runa expect`, `bench` | Preview | Used by the compiler hardening loop; expectation corpus shape, fixture volume, and benchmark reporting can still evolve. |
| `runa meta` | Preview | Resolves canonical `--@label:LABEL::meta:BINDING--` attachments whose root type explicitly implements `Meta`; indexes typed role variants from types implementing `MetaRole`; retains legacy role-chain and `MetaAttachment` compatibility; exposes ground structural values and recursively typed descendant paths; indexes raw-text/code spans and symbols; and supports type/role sweeps over files or recursive source trees with versioned `--json` output. Output shape may still evolve as audit and explanation tooling grows. |
| `runa schema`, `template`, `call` | Preview | Inspect typed calculation contracts, generate JSON/TOML/XLSX inputs (including related worksheets for repeated values), validate fingerprints and values, and invoke isolated batches. All adapters decode through the same canonical contract. |
| `runa verify` | Preview | The command is supported, but the elaboration and automation path is not yet a frozen contract. |
| `runa audit` | Experimental | Structural findings use rule identity, active resolution branches, typed value domains, proof references, and type-member coverage. Contradictions require conflicting active branches of the same rule at the same priority; names never imply semantic relationships. Treat output shape and behavior as early and subject to redesign. |
| `runa explore` | Experimental | Opens or resumes one authenticated journal under explicit `--run-state`, advances a time/resource-bounded slice, and incrementally publishes authorized artifacts to a separate `--output` tree; a completed resume is a no-op. One invocation constructs the relation and admission once and evaluates every unique named find, including zero or plural questions, while aliases reuse the same semantic work. Report v11 and publication v20 expose canonical question IDs plus authored find names, per-question closure, explicit result/mechanism consumers, U/D/M(q) graph support, independently resumable Choice relations distinct from display ViewIds, and answer-index schema v2 for every named result—including ungrouped, open, and exact-empty views. Choice candidates may accumulate before question closure; exact membership and `ChoiceContentRoot` wait for the question seal. Choice-target mechanisms then replay canonical Choice members and seal without a presentation ViewId before any dependent display projection runs. Population-sized configurations remain in separately resumable NDJSON. Publication resume also authenticates the immutable find-address plan and per-artifact presentation metadata; renaming an address requires a fresh output tree without changing the semantic journal. Case/support schema v4 is independently resumable: discovery-ordered stable-key `add` records expose sparse accepted chunks and regions immediately, later selected materializations and cases append separately, and exact closure adds a canonical key-sorted `seal`; internal prefix promotion is not published. Nonempty plural classified runs share one compact pass bound to the complete canonical question set, with packed joint decisions compressed as runs, per-question projected counts/support, and one selected-case materialization for the any-selected union. Native v2 and regional-proof acceleration remain exact-one; zero-question runs use bounded ordinary execution. A focused audit paused at transition 17, reopened, and completed one shared 300-transition sweep with independent exact selected counts of 20 and 10 and one 20-case any-selected materialization, without either accelerator. The existing concrete four-case/two-find smoke paused and resumed to exact counts of four and two with all fourteen artifacts caught up; a separate four-case choice-target smoke selected two tied winners and published an exact `{cases: 2, mechanisms: 1}` summary. No broad Personskat exploration was executed. There is no public probe phase or separate observer/tail command yet. Admission-scoped target execution, fixed-fan-in external merging, parallel workers, symbolic closure, and the broad Personskat audit remain deferred. Invocation CPU/RAM limits are operational policy and currently preserve at least 20 percent for the host; the containment supervisor is presently macOS-only. |
| `runa from-rust`, `from-rust --verify` | Stable | Production-ready inside FRSS-v0, the checked single-file validation boundary in `docs/from-rust-contract.md`. Supported FRSS-v0 fixtures exact-match Rust stdout, recognized unsupported boundaries fail closed with stable diagnostic categories, and `from-rust --verify` has stable summary lines for supported matches, recognized unsupported categories, and translator/runtime failures. This is still not arbitrary Rust crate translation, module-tree translation, generated Cargo manifests, broad macro expansion, full lifetime/reference preservation, unsafe/async/effectful APIs, or general iterator state-machine translation. |
| `runa registry` | Experimental hidden surface | Internal metadata helper; not documented as a public workflow command. |

## Structured Metadata

Every user-facing page under `docs/reference/` and `docs/tutorial/`, plus
selected cross-cutting contract pages listed in the JSON metadata, carries
frontmatter with:

```yaml
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
```

Aggregate pages that intentionally cover more than one stage use
`feature_stage: mixed` and list the concrete surface ids they cover.

The JSON contract uses schema `futuruna.feature-stages.v1`. Consumers should
read `schema_version` and treat unknown fields as additive. The current top-level
keys are:

| Key | Meaning |
|-----|---------|
| `stage_values` | Human-readable definitions for stage labels, including the `mixed` document roll-up label. |
| `surfaces` | Public language, runtime, artifact, verification, integration, documentation, and tooling surfaces. |
| `commands` | CLI commands and the stage/surface each command belongs to. |
| `documents` | Per-page stage metadata for reference and tutorial pages. |

## How To Use This Document

When documenting or reviewing a change:

1. find the affected surface here
2. apply the rules from [docs/compatibility-policy.md](compatibility-policy.md)
3. if the surface is missing, either add it here or explicitly mark it as
   experimental/preview in the relevant doc instead of assuming stability
4. update `docs/feature-stages.json` and affected page frontmatter in the same
   change when a stage assignment changes
