---
feature_stage: preview
feature_stage_surfaces:
  - typed-program-references
---

# RFC: Typed program references

Status: Preview

## Summary

`refof(...)` creates ordinary typed Futuruna data that identifies a declaration
or structural member in the checked program. It replaces opaque declaration
names in metadata without turning metadata into executable code or a second
dependency graph.

```runa
# SourceInfo(url: String)
# Evidence(source: SourceInfo, target: ProgramReference)
# impl Meta for Evidence {}

= section_source = SourceInfo(url = "https://example.invalid/law")
= section_evidence = Evidence(
    source = section_source,
    target = refof(calculate_tax)
)

--@label:calculate_tax::meta:section_evidence--
| calculate_tax(income: Int) -> income / 2
```

The standard prelude defines:

```runa
# ProgramReference =
    ProgramSymbolReference(name: String)
  | ProgramTypeReference(name: String)
  | ProgramMemberReference(root_type: String, path: String)
```

## Syntax and resolution

Version 1 accepts three forms:

```runa
refof(calculate_tax)
refof(TaxInput)
refof(TaxInput::household::children::age)
```

A lower-case symbol target must resolve unambiguously to a rule, function, or
binding. A type target must resolve to a declared type. Member paths are checked
through declared fields, transparent optional/reference wrappers, list and set
elements, map values, and explicit sum variants. RuleScope rules and functions
may be addressed as terminal members:

```runa
refof(TaxCase::result)
```

Plain imports participate in the same lookup. Unknown targets, invalid paths,
and targets that collide across declaration categories are compile errors. A
RuleScope capture and callable may legally share a name in executable code,
where projection and call syntax distinguish them; the corresponding bare
member reference is deliberately rejected as ambiguous.

`refof` values survive interpretation, native compilation, and metadata JSON as
the same prelude constructors. LSP definition navigation follows local and plain
imported symbols, types, fields, variants, and RuleScope members.

## Relationship to `pathof`

`pathof(Input::field)` returns the canonical `String` `"field"`. It is intended
for external data paths whose serialized contract is textual.

`refof(Input::field)` returns
`ProgramMemberReference(root_type: "Input", path: "field")`. It is intended for
typed provenance, evidence, warnings, and other metadata that points back into
the Futuruna program. Existing `pathof` expressions and string metadata remain
compatible.

## Semantics

A program reference is a pure value. Constructing one does not call, project,
or evaluate its target. It does not add an edge to rule dependency analysis.
Futuruna continues to derive executable dependencies solely from actual rule and
function calls.

The reference variants distinguish symbols, types, and members in portable
data. The compiler retains the more specific resolution fact, such as rule
versus function or field versus scoped rule, for validation and navigation
without duplicating those semantics in a serialized metadata schema.

## Non-goals

- `refof` is not reflection and cannot dynamically invoke its target.
- It does not replace imports, exports, rule calls, or `@ depends_on` behavior.
- It does not make comments semantic; meta comments still attach one ordinary
  typed value to a source label.
- Version 1 does not address qualified-module references or local lexical
  variables.
