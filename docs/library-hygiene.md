---
feature_stage: stable
feature_stage_surfaces:
  - importable-local-libraries
  - library-hygiene-tooling
---

# Library Import Hygiene

Importable Futuruna library files must be explicit about being import-safe when
they participate in the stable local-library consumer contract.

The stable checker is:

```bash
runa lint-library tests
```

For a consumer surface, also check the actual import graph:

```bash
runa lint-library --imports tests/downstream
```

For a compiler-facing snapshot of the public import/export boundary, use:

```bash
runa emit --imports path/to/consumer.runa
```

That report is intentionally narrow: it lists the normalized public export sets
and ADT constructors for flat and qualified imports. The exact expectation
`tests/expect/imports/import_normalization_contract.runa` keeps mixed
plain-plus-qualified imports and multiple aliases from regressing.

## Marker

Mark importable library files with:

```runa
-- library-hygiene: importable
```

The checker still recognizes the older helper marker
`-- roundtrip-skip: library file, no expected output`, but new files should use
the explicit import-hygiene marker.

## What Is Allowed

Importable library files may define or export:

- functions, actors, modules, types, traits, effects, rules
- pure top-level values
- pure top-level stream bindings
- imports, dependencies, and declarative annotations such as `@ export`

Plain and qualified imports flatten importable library declarations into the
consumer. Pure top-level values are allowed because exported helpers may depend
on private constants or derived tables.

## What Is Rejected

Importable library files should not execute top-level script flows such as:

- `@ print(...)`
- bare top-level expression statements
- `for` / `while`
- top-level `send`
- top-level stream subscriptions
- top-level `? name` proof execution
- top-level `assert`, `retract`, `abort`
- top-level bindings whose expressions obviously perform import-time side effects
  such as `read_file`, `write_file`, `http_get`, `db_exec`, `process_run`, or
  similar impure builtins
- top-level bindings or stream bindings that reach those side effects through
  local helper function call chains

When `runa lint-library --imports <file|dir>` checks a consumer, every file
reached through plain or qualified imports must be marked importable and must
pass these same rules. Content-hash imports are different: they select one
declaration by hash, not the imported file's top-level script body.

Normal `runa run`, `runa check`, and `runa emit` remain backward-compatible for
ordinary scripts. The import-graph hygiene check is the blocking gate for
authored downstream/canary consumers and for files that claim the importable
library contract.

## Why This Exists

The goal is not to ban real library surfaces. The goal is to stop library files
from quietly accumulating smoke/demo behavior that only breaks once another file
imports them.

Use ordinary scripts, canaries, or examples for runnable top-level flows. Use
importable library files for reusable exported surfaces.
