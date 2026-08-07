# Semantic Module Interfaces

Status: unstable internal compiler contract

## Problem

Futuruna currently parses imports into one program and emits one Rust unit. A
change in any imported file therefore invalidates the whole source graph, even
when the change preserves everything an importer can observe during semantic
analysis.

Incremental compilation needs to distinguish three identities:

1. **Content hash**: the exact implementation source of one module.
2. **Interface hash**: the local semantic surface exposed by that module.
3. **Dependency hash**: the local interface plus all transitively imported
   interfaces.

A body-only edit changes the content hash. It does not change the interface or
dependency hashes when all inferred and declared signatures remain equal. A
signature, layout, effect, import-resolution, calculation-contract, or metadata
contract change updates the interface hash and propagates through dependency
hashes.

## Interface Contents

The versioned `futuruna.semantic-interface.v1` representation contains:

- flat, qualified, content-addressed, Cargo, and Rust imports;
- resolved module paths where module resolution applies;
- exported names;
- functions, rules, methods, actor handlers, and raw-Rust callable signatures;
- declaration-order parameter names and types, including `inout`;
- explicit or inferred return types and declared effects;
- ADT constructors and fields, conditional type layouts, effects, traits,
  implementations, and RuleScope members;
- typed top-level bindings, streams, reactive-scope members, and invariants;
- semantic annotations;
- calculation entry names and their canonical schema hashes;
- normalized metadata labels, references, attached values, source-text hashes,
  and code-span symbols.

Source locations and ordinary implementation bodies are excluded. Metadata
source text and statically attached metadata values are included because audit
tools expose them as data.

The canonical representation uses ordered collections and JSON serialization
before SHA-256 hashing. Equivalent interfaces therefore produce identical
hashes in separate compiler processes.

## Dependency Hashing

Imports form a directed graph. Strongly connected components are hashed as one
unit so cyclic imports terminate deterministically and a public change in any
member invalidates every member of the cycle. The component graph is acyclic;
its hashes are then composed from imported component hashes.

This also handles diamonds without duplicate or order-dependent traversal. A
leaf interface change invalidates both branches and their common importer. A
leaf body edit with the same interface invalidates none of those semantic
dependents.

## Artifact Keys

The next incremental compiler layer should key a module artifact by at least:

```text
compiler fingerprint
module content hash
transitive semantic dependency hash
target and compilation mode
```

The content hash ensures the edited module itself is rebuilt. The dependency
hash ensures it is rebuilt when an imported semantic contract changes. An
imported implementation-only edit can reuse the module's typed and generated
artifact while the changed dependency receives a new implementation artifact.

## Validation

`runa interface <file.runa>` is an unstable internal introspection command that
emits the complete graph and all three hashes. Regression coverage requires:

- byte-identical output across independent processes;
- stable interface hashes for body-only edits;
- changed interface hashes for named-parameter, type-layout, inferred-return,
  effect, import-resolution, calculation, and metadata changes;
- deterministic invalidation through diamonds and cycles;
- a full Personskatteloven graph experiment before cache behavior is promoted.

This RFC defines the invalidation boundary. Persistent typed FIR and stable Rust
module artifacts are a separate implementation step.
