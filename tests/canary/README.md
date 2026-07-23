Authored Futuruna canaries live here.

Purpose:
- exercise realistic, user-shaped programs rather than tiny feature probes
- stay owned by this repo rather than importing downstream projects
- catch semantic regressions that ordinary unit tests miss

Layout:
- `core/` is the blocking authored lane: interpreter, compiled, codegen, and roundtrip should all stay green
- `stateful/` covers subjects, actors, lifecycle, and other richer runtime flows
- `extended/` is reserved for heavier canaries such as JSON, DB, HTTP, WASM, and import-heavy programs
- `regressions/` is where user bug classes get distilled into broader workflow canaries

Rules:
- prefer small but realistic programs over toy probes
- every canary should mix multiple subsystems
- failures should be obvious from precise invariants or exact output
- when a user bug reveals a broader workflow pattern, distill it into a canary here
