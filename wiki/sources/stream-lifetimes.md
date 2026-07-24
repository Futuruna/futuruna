---
type: source
status: summarized
source_kind: repo-doc
source_path: "docs/stream-lifetimes.md"
created: 2026-07-18
updated: 2026-07-18
tags:
  - source
  - docs
  - streams
  - lifecycle
  - scopes
related:
  - "[[feature-stages]]"
  - "[[verification-lanes]]"
  - "[[test-surface]]"
  - "[[repo-docs]]"
---

# Stream Lifetimes

This source note summarizes `docs/stream-lifetimes.md`.

## What It Adds

- one canonical lifetime contract for live stream consumption
- explicit named-scope ownership for live subscriptions
- scope-owned derived stream forwarders and barrier cleanup on teardown
- a clear rejection of detached function-local live subscriptions
- guidance for what functions should do instead: return streams, consume
  snapshots, or require a caller-owned scope

## Most Important Reading

- top-level subscriptions are script-lifetime work
- named scopes are the current lifetime owner for live subscriptions
- derived async streams built in a named scope freeze after teardown instead of
  continuing hidden forwarder work
- ordinary functions must not silently create detached live subscriptions
- `@ teardown("ScopeName")` is part of the explicit ownership story, not a
  best-effort cleanup hack

## Best Companion Notes

- [[feature-stages]]
- [[verification-lanes]]
- [[test-surface]]
- [[state-and-roadmap]]
