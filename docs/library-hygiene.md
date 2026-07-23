# Library Import Hygiene

Importable Futuruna library files should be explicit about being import-safe.

The canonical checker is:

```bash
runa lint-library tests
```

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

## Why This Exists

The goal is not to ban real library surfaces. The goal is to stop library files
from quietly accumulating smoke/demo behavior that only breaks once another file
imports them.

Use ordinary scripts, canaries, or examples for runnable top-level flows. Use
importable library files for reusable exported surfaces.
