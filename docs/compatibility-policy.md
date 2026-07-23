# Futuruna Compatibility Policy

This document defines what Futuruna treats as compatibility, how language
surfaces move from experimental to stable, and when a bug fix may break old
behavior without a staged deprecation.

For the current stage assignments of major language and tooling surfaces, see
[docs/feature-stages.md](feature-stages.md).

The goal is simple: users should not have to guess which changes are ordinary
progress, which are intentional compatibility breaks, and which surfaces are
still explicitly unstable.

## 1. Compatibility Categories

Futuruna tracks compatibility by **surface**, not by vague intuition.

### Source compatibility

Source compatibility means an existing `.runa` program still parses,
type-checks, and is accepted by the language under the same intended source
constructs.

Examples of source breaks:

- syntax changes
- changed name resolution or import rules
- changes that reject programs previously accepted by parser or typechecker
- changed meaning of stable surface syntax in a way that requires source edits

### Behavioral compatibility

Behavioral compatibility means a stable Futuruna program that still compiles
continues to have the same documented observable meaning:

- interpreter behavior
- compiled/native behavior
- runtime error behavior
- deterministic ordering and equality/display contracts where those are part of
  the documented language semantics
- standard-library builtin semantics

Examples of behavioral breaks:

- `substring(s, start, length)` changing meaning
- `head([])` or `xs[i]` changing from one documented contract to another
- stable collection ordering changing without being explicitly declared unstable

### Verification compatibility

Verification compatibility means documented stable proof or verify surfaces keep
their contract:

- proof term forms
- theorem/invariant surface syntax
- documented kernel-backed capabilities
- stable `runa verify` meanings where a contract is explicitly documented

This category exists because Futuruna is not only a runtime/compiler project.
Proof and verification behavior can also break users.

### Artifact and codegen compatibility

Artifact compatibility means emitted or produced artifacts remain compatible
with documented promises.

Default rule:

- Futuruna **does not** guarantee the exact emitted Rust text, helper names,
  internal module layout, or formatting.
- Futuruna **does** care about codegen as a compatibility surface when the
  emitted artifact behavior is part of the documented language contract.

In practice:

- exact emitted Rust shape is unstable unless a doc explicitly promises it
- emitted program behavior is part of behavioral compatibility
- build outputs, ABI, and integration surfaces only become stable if they are
  documented as such

### Diagnostic and internal compatibility

These surfaces are explicitly lower-stability unless documented otherwise:

- exact wording of diagnostics
- internal FIR/AST layouts
- internal Rust helper names
- compiler implementation structure
- performance characteristics

Changes here are still expected to be reviewed carefully, but they are not
treated as stable public contract by default.

## 2. Feature Stages

Every user-visible Futuruna surface should be thought of as being in one of
three states.

### Experimental

Experimental means:

- the feature is available for exploration
- the team is still learning the right design
- source and behavior may change or be removed without a deprecation cycle
- changes must still be called out in docs or release notes if users are likely
  to notice them

Experimental is appropriate for:

- new syntax
- new proof surface
- unstable runtime/library APIs
- codegen or tooling affordances that are not yet part of the public contract

### Preview

Preview means:

- the feature is intended for real-world feedback
- the design is expected to settle, but is not frozen yet
- incompatible changes should be announced and migration guidance should be
  provided where possible
- removal or large redesign is still allowed if the current shape is clearly
  wrong

Preview is where Futuruna should battle-test important new surfaces before
calling them stable.

### Stable

Stable means:

- the surface is part of Futuruna's public contract
- source and behavioral breaks require explicit compatibility handling
- contributors should assume users may rely on it in production or long-lived
  code

Stable does **not** mean "never change." It means changes must be classified,
justified, documented, and covered.

## 3. Default Stability Rules

Unless a doc says otherwise:

- core language syntax in public docs is treated as stable
- documented builtin semantics are treated as stable
- the proof kernel rule set and documented proof surface are treated as stable
  once published as current behavior
- exact emitted Rust text is not stable
- diagnostics wording is not stable
- internal compiler structures are not stable

If a surface is not clearly documented, contributors should bias toward either:

1. documenting it before treating it as stable, or
2. explicitly marking it experimental/preview instead of assuming stability by
   accident

## 4. Deprecation And Migration Expectations

For stable source or behavior changes, the preferred path is:

1. document the incompatibility
2. add or update permanent coverage for the old and new contract boundaries
3. provide a migration path when one exists
4. warn before hard-breaking when the compiler can do so at reasonable cost
5. record the change in the compatibility guide or release notes

When practical, stable source breaks should get at least one release cycle of:

- warning, alias, or compatibility mode
- migration note
- explicit test coverage around the transition

Not every change can be warned for. When it cannot, the review and
compatibility note must say why.

## 5. Bug-Fix Exceptions

Some old behavior is simply wrong enough that staged deprecation is not the
right answer.

Futuruna may bypass staged deprecation for a stable surface when the old
behavior is any of:

- unsound
- nondeterministic against a documented determinism promise
- wrong-code generation
- security-sensitive
- data-corrupting
- contradicting an already documented language contract

When this happens, the change still must:

1. state that it is a compatibility exception
2. explain why the previous behavior is treated as a bug, not a supported
   alternative
3. land with permanent regression/canary/differential coverage
4. include migration guidance if user code is likely to be affected

The rule is not "stable users can always be broken immediately." The rule is
"some bugs are too wrong to preserve, but the break still has to be explicit."

## 6. Contributor Requirements For Compatibility Changes

Any pull request that changes a stable surface should say:

- which compatibility category it touches
- whether the change is source, behavioral, verification, or artifact-facing
- whether the surface is stable, preview, or experimental
- whether this is a normal staged change or a bug-fix exception
- what migration or warning path exists, if any
- what permanent coverage was added

`CONTRIBUTING.md` defines the mechanical ratchet. This document defines the
classification model reviewers should apply when reading those changes.

## 7. What This Policy Does Not Yet Automate

This policy is a contract first. Some enforcement still needs follow-up work:

- feature stability metadata is not yet surfaced consistently in tooling/docs
- Futuruna does not yet publish a versioned compatibility guide per release
- artifact compatibility is only partially formalized beyond emitted behavior

Those gaps should be tracked explicitly in `td`, not left as implicit policy
debt.

## 8. Practical Reading

When a change is proposed, ask:

1. Which compatibility category does it touch?
2. Is the affected surface experimental, preview, or stable?
3. Is this a normal staged change or a bug-fix exception?
4. Where is the migration story documented?
5. What permanent coverage now defends the new contract?

If those answers are unclear, the change is not ready to merge.
