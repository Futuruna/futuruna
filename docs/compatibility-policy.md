# Futuruna Compatibility Policy

During development toward 0.2.3, prefer a correct, clear, coherent language over
backward compatibility. There are no known live projects outside this repository
depending on the language. A better design may replace an existing contract,
including one marked Stable, without a deprecation cycle, legacy alias, or
compatibility mode.

Changing a contract still requires deliberate review and permanent coverage.
Update the implementation, tests, reference material, examples, and repository
programs together. Preserve the intended meaning of authored legal models when
adapting their syntax or APIs; a language redesign does not authorize changing
their legal assumptions or results.

## Identify the affected contract

Classify changes so reviewers can assess their consequences:

- **Source:** syntax, name resolution, imports, types, and accepted programs.
- **Behavior:** interpretation, native execution, builtins, runtime errors,
  ordering, and equality.
- **Verification:** invariants and what verification results establish.
- **Artifacts and integration:** generated workbooks, reports, journals, native
  interfaces, and emitted program behavior.
- **Internals:** compiler representations, helper names, diagnostic wording,
  performance, and generated Rust layout.

Exact emitted Rust is generally an implementation detail. Reviewed artifact
goldens define explicit expectations where exact output matters; update them
only after reviewing the resulting contract. See the
[artifact and codegen contracts](artifact-codegen-contracts.md).

## Feature stages

[Feature stages](feature-stages.md) and their
[machine-readable assignments](feature-stages.json) describe current maturity:

- **Experimental:** available for exploration; its design may change or be
  removed.
- **Preview:** intended for practical feedback, with a design still being
  refined.
- **Stable:** a documented, defended contract for the current language.

Stable is a quality expectation, not a freeze on language design during this
development phase. Existing coverage remains authoritative until a deliberate
contract change replaces it. Do not infer stability merely because an
undocumented behavior happens to work.

## Make a coherent change

1. Read the closest contract, implementation, example, and test.
2. Choose the behavior that best serves the language. Do not retain an inferior
   design solely because an earlier version accepted it.
3. Implement the change across the affected frontend, interpreter, native
   backend, verification, and tooling paths. Make unsupported cases explicit.
4. Update repository callers and maintained documentation to use the current
   contract. Remove obsolete branches and aliases when they no longer serve a
   purpose.
5. Add regression coverage for the new behavior and relevant failure boundaries.
   A changed golden or a deleted failing assertion alone does not establish
   correctness.
6. Run the semantic-change ratchet in [CONTRIBUTING](../CONTRIBUTING.md), including
   Mint and the deeper lanes appropriate to the change.

## Describe the current language

Reference material and examples should explain how Futuruna works now. Fix
them directly rather than adding historical sidebars, migration diaries, or
obsolete alternatives. Review descriptions explain the rationale and affected
contracts; Git preserves the history.

The existing compatibility-guide CI check accepts a concrete reason for not
adding a versioned guide entry. For a deliberate redesign, use its PR field to
identify the current references and repository programs updated with the change.
No new historical guide entry is required by this policy.

## Preserve correctness and evidence

Freedom to break compatibility does not permit silent reinterpretation of saved
facts or evidence. When a schema or contract changes, update its identity and
validation so incompatible inputs are rejected or explicitly regenerated.
Preserve provenance, source/compiler identity, and the distinction between
facts, assumptions, interpretations, and unknowns.

A review should state the resulting contract, the affected execution paths,
the updated references and callers, the permanent coverage, and the exact
checks run. Record any remaining gap as tracked work rather than implying it
has been fixed.
