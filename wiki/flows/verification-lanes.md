---
type: flow
status: active
created: 2026-07-18
updated: 2026-07-18
tags:
  - flow
  - verification
related:
  - "[[mint-ratchet]]"
  - "[[test-surface]]"
---

# Verification Lanes

## Primary Gate

`./scripts/mint.sh` is the main blocking gate for “Futuruna is mint”.

## Coverage Shape

- Rust test suite
- release build
- interpreted Futuruna suite
- compiled Futuruna suite
- codegen checks
- roundtrip parity
- selected example/program checks

## Canary Shape

- `tests/canary/core/`
- `tests/canary/stateful/`
- `tests/canary/extended/`
- `tests/canary/regressions/`

## Practical Reading

When something breaks externally, ask:

1. did the mint gate miss it?
2. did the canary surface miss the usage shape?
3. is the failure due to heuristic codegen rather than a stated invariant?

