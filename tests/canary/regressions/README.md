Authored regression canaries live here.

This tier is for user-found bug classes that deserve a broader workflow canary,
not just a minimized compiler probe.

Put a bug here when all of these are true:

- the original report reflects a realistic Futuruna workflow rather than a tiny
  isolated parser/codegen edge case
- the distilled fixture still mixes multiple subsystems
- the failure would matter to ordinary users, not only to compiler developers

Keep the bug in ordinary tests instead when:

- a narrow Rust/unit/codegen regression captures it precisely
- the bug only exists as a one-line crash or parse rejection with no broader
  workflow pattern
- the authored canary would mostly duplicate an existing core/stateful canary

Good regression canaries should say which bug class they distill and should
freeze the broader behavior users expected, not only the exact minimized input
that happened to fail first.
