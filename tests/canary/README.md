Authored Futuruna canaries live here.

Purpose:
- exercise realistic, user-shaped programs that mix language features
- stay owned by this repo rather than importing downstream projects
- catch semantic regressions that ordinary unit tests miss

Rules:
- prefer small but realistic programs over tiny feature probes
- use invariants or precise output so failures are obvious
- each canary should stress more than one subsystem
- when a user bug reveals a broader workflow pattern, distill it into a canary here
