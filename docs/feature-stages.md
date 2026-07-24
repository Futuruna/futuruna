# Futuruna Feature Stages

This document makes the current stage of major Futuruna surfaces visible to
users and contributors.

It should be read alongside [docs/compatibility-policy.md](compatibility-policy.md):

- the compatibility policy defines what the stages mean
- this document says which current surfaces are in which stage

## Language And Runtime Surfaces

| Surface | Stage | Notes |
|---------|-------|-------|
| Core language syntax documented in `docs/reference/basics.md` and `docs/reference/runes.md` | Stable | Changes here are source-compatibility changes unless docs explicitly mark a subsection otherwise. |
| Documented stdlib builtin semantics in `docs/reference/stdlib.md` | Stable | Behavioral changes require compatibility handling or an explicit bug-fix exception. |
| Pure/core generated Rust artifact shape | Preview | Generated Rust for stable pure/core source must compile and preserve behavior; exact emitted text is stable only for named artifact fixtures. See `docs/artifact-codegen-contracts.md`. |
| Exact helper names, private generated layout, and internal compiler layouts | Unstable internal | Not a public compatibility surface unless a doc or artifact expectation explicitly promises it. |
| Explicit kernel proof terms and documented proof-kernel rule forms | Stable | The small kernel-backed proof term surface is part of the published contract. |
| `runa verify` theorem elaboration, solver fallback, and broader verification automation | Preview | Useful and supported, but still evolving as the proof trust boundary and automation pipeline change. |
| Reactive/stateful surfaces such as streams, subjects, actors, and effect-heavy workflows | Preview | User-facing and documented, with explicit named-scope ownership for live subscriptions, but still under active semantic hardening and canary expansion. |
| Rust interop and Rust-facing integration behavior | Preview | Supported, but still evolving around ownership, codegen, and artifact boundaries. |

## Tooling And Command Surfaces

| Command family | Stage | Notes |
|---------------|-------|-------|
| `runa run`, `check`, `emit`, `build`, `test`, `fmt`, `hashes` | Stable | These are core workflow commands. Their documented behavior is part of the normal public surface. |
| `runa lint-library` | Preview | Supported import-hygiene tooling for authored library surfaces. The policy is deliberate, but the exact lint coverage can still expand. |
| `runa lib`, `wasm`, `lsp`, `stress-gen` | Preview | Useful and supported, but still subject to format, interface, or behavior refinement. |
| `runa verify` | Preview | The command is supported, but the elaboration and automation path is not yet a frozen contract. |
| `runa audit` | Experimental | Treat output shape and behavior as early and subject to redesign. |
| `runa from-rust`, `from-rust --verify` | Experimental | Early translational tooling. Do not treat current behavior as a frozen compatibility contract. |

## How To Use This Document

When documenting or reviewing a change:

1. find the affected surface here
2. apply the rules from [docs/compatibility-policy.md](compatibility-policy.md)
3. if the surface is missing, either add it here or explicitly mark it as
   experimental/preview in the relevant doc instead of assuming stability

## Current Gaps

This stage matrix is intentionally lightweight. Futuruna does not yet:

- surface stage metadata mechanically in every doc page
- expose machine-readable stage metadata in tooling

Those are tracked follow-up tasks, not hidden assumptions.
