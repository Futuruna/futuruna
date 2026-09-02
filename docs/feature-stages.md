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
| Relational bounded `? explore` declarations and exact-finite execution | Experimental | The public implementation declares an ordered finite dependent `from` relation, a singleton or finite `to` successor relation, scoped `where` admissions, `find all`/matches/violations, and a named result/mechanism dependency DAG. `RelationId`, `AdmissionId`, `QuestionId`, `ViewId`, `MechanismRequestId`, content-stable CaseIds and closure-aware frontiers bind the durable evidence. A `RelationId`-scoped, FROM-only source-coverage manifest recursively classifies Context/Before paths and reachable immutable producer inputs; unresolved provenance is an explicit gap, never inferred irrelevance. The supported selected- and same-question chosen-view-target paths run end to end through classification, named views, fresh Before/After endpoint replay, raw and structural mechanism incidence, target-conditioned prefix-native support observations, an authenticated journal, and bounded resumable NDJSON publication. Publication v11 emits one request-local append-only mechanism-support observation sidecar and automatically registers the total-support slice of every discovered structural mechanism. Assignment and terminal updates dirty only the affected mechanism, multiple updates may coalesce before its next point, and already accepted observations remain immutable historical prefix evidence. After request support closes, a lazy final sweep appends a sealed successor for every registered mechanism slice; the structural sidecar's constant-size receipt is withheld until registered, observed and sealed slice counts all equal the exact structural-mechanism count. Structural definitions also advertise stable descriptors for total activation/differential node and edge facets without automatically scheduling them. The coordinate type can represent a route-conditioned slice when a future explicit demand scheduler requests one. The structural sidecar carries assignments, quotient closure and the optional support receipt rather than closure-time all-subject `structural_subject_support` rows. A separate explicit single-subject `starters` consumer materializes one authorized typed correlated fiber for a structural mechanism or activation/differential node/edge; node/edge consumers may additionally intersect support with one enclosing structural mechanism while preserving node/edge identity. The same value authority automatically enables a separate selected semantic case-transition graph with typed Context/Before/After, canonical StateIds and TransitionIds. The trailing `transitions NAME from all cases` consumer instead requests a full identity-only State/Transition graph with authenticated U/D/M CaseId support, bounded canonical paging and explicit `capacity_limited`/`unmaterialized` terminals; it never implicitly exposes typed endpoint values. Transition schema IDs belong to the journal contract and the layered graph root to core journal evidence independently of consumers, while consumer names and paths remain additive publication state. Authored probes, a probe lifecycle and ordinal/rank CaseIds are not part of this model; v0 and publication-v9/v10 Experimental artifacts are implementation history, not compatibility targets. Admission-scoped mechanism targets, explicit node/edge observation-demand registration, arbitrary path-conditioned starter selectors, parallel workers, a general late symbolic materializer and broad multidimensional Personskat closure remain in progress. |
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
| `runa explore` | Experimental | Selects one named relational query, opens or resumes its authenticated journal under an explicit `--run-state`, executes one time/resource-bounded semantic slice, and incrementally publishes authorized views and case/mechanism artifacts to a separate `--output` tree. Identical completed resumes are no-ops. The selected- and same-question chosen-view-target paths, endpoint mechanism replay, post-mechanism views, structural mechanism/profile catalog, and prefix-native factorized support observations are implemented; large definition lanes have independent bounded cursors. Publication v11 writes accepted observation points to `mechanisms/<request>.support-observations.ndjson` while the request remains open. Every discovered structural mechanism automatically registers one total-support slice. Only evidence changes for that mechanism make it dirty, dirty changes coalesce before the next canonical observation, and historical points remain immutable. A lazy post-support-closure sweep later appends one sealed successor per registered mechanism; completion requires registered, observed and sealed slice counts to equal the structural-mechanism count. Stable descriptors in the structural-definition catalog expose total node/edge facet coordinates without scheduling or eagerly observing them; the same coordinate type can represent a route-conditioned slice for future explicit demand. The structural sidecar contains assignments, quotient closure and at most one constant-size support-closure receipt, not closure-time all-subject summaries. Separately, one authenticated `starters/<consumer>.ndjson` lane is scheduled for each explicit single-subject `starters` declaration. A node/edge declaration may add `within mechanism "<StructuralMechanismId>"`; the publisher intersects existing signature indexes and binds that route into its plan, cursor, pages and closure without changing upstream semantic identities. Its named compatible lossless selected-input each-case view authorizes `case_id`, `context`, `before` and `after`; its canonical k-way pager starts with at most 64 typed members, adaptively shortens a page to fit publication `max_line_bytes`, fails explicitly if one member alone is too large, and uses memory proportional to contributing signatures plus that page. The typed subject closure independently certifies exact case and distinct-starter counts; support observations never implicitly expose those values. That same checked value surface enables `graphs/case-transitions.ndjson`, an independently resumable selected edge list carrying CaseId, source/successor keys, StateIds, TransitionId, schema identities and typed Context/Before/After; its exact closure is set-rooted rather than order-defined. V2 collision-checks at most 65,536 retained edges and emits an explicit non-exact `capacity_limited` terminal if discovery exceeds that bound. Adding an explicit typed artifact to a completed semantic stream preserves the semantic journal while updating only its independent publication lane plus cursor/manifest state. A running invocation advances and publishes bounded internal micro-slices, so durable results can emerge before the outer invocation returns; a separate observer/tail command is not yet exposed. Admission-scoped targets, explicit node/edge observation-demand registration, arbitrary path-conditioned selectors and fixed-fan-in external merging remain deferred. There is no public probe phase, ordinal case-space contract, or compatibility promise for v0 or publication-v9/v10 artifacts. Invocation CPU/RAM limits are operational policy and currently preserve at least 20 percent of CPU and RAM for the host; the containment supervisor is presently macOS-only. Parallel workers, symbolic closure and the broad Personskat audit remain in progress. |
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
