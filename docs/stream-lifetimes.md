---
feature_stage: stable
feature_stage_surfaces:
  - reactive-stateful-surfaces
---

# Stream Lifetimes

This document defines the current lifetime contract for live Futuruna stream
consumption.

It should be read alongside:

- [docs/reference/streams.md](reference/streams.md)
- [docs/reference/runes.md](reference/runes.md)
- [docs/feature-stages.md](feature-stages.md)
- [docs/library-hygiene.md](library-hygiene.md)

## The Core Rule

Live stream consumption must have an explicit owner.

In Futuruna today, that owner is a named `| scope`.

That means:

- top-level subscriptions are script-lifetime work
- subscriptions inside a named scope are scope-lifetime work
- ordinary functions must not silently create detached live subscriptions

This rule exists to keep stream lifetime explicit instead of letting compiled
async subscriptions outlive the function that created them.

## What Counts As Live Stream Consumption

These forms create live stream work when they target a subject or derived async
stream:

```runa
~ readings
    | x -> { @ print(show(x)) }

for x in readings {
    @ print(show(x))
}

~ projected = readings |> filter(|x| x > 30) |> map(|x| x + 1)
```

In compiled async mode, those forms become background receive loops or stream
forwarder tasks. They are not just local list iteration.

The validation predicate is `is_live_stream_expr_for_validation` in
`src/bin/runa.rs`. An expression counts as live when it is:

- a variable bound to a `subject()` or to an async-stream binding, or
- an `as_stream(...)` of a live source, or
- a derived operator chain over a live source — currently `map`, `filter`,
  `scan`, `take`, `skip`, `tap`, `merge`, `start_with`, `concat` — including
  the desugared `|>` form.

Adding a new stream operator that spawns a forwarder task means adding it to
this list; otherwise the contract leaks.

## Snapshot Reads (Allowed Without Scope)

A "snapshot" read is any stream observation that does **not** spawn a
background task. These are allowed in ordinary functions because their cost is
bounded and they do not outlive the call.

Snapshot reads include:

```runa
> latest_label(s) -> String { "latest: " + s.latest }
> count_so_far(s) -> Int { s.count }
> first_seen(s) -> Option(Int) { s.first }
> total(xs: List(Int)) -> Int { xs |> sum }
```

The boundary is intentional and narrow: anything that goes through one of the
derived-operator names listed above is *not* a snapshot, even when its result
"looks" terminal. If you want a terminal value from a derived pipeline, route
it through a scope first.

## Allowed Ownership Shapes

### 1. Top-level script ownership

At the top level of a program, a subscription belongs to the script itself:

```runa
~ readings = subject()

~ readings
    | x -> { @ print(show(x)) }

readings <- 1
readings <- 2
```

This is appropriate for tests, demos, and top-level application entrypoints.

### 2. Named scope ownership

Inside a named scope, the scope owns the subscription lifetime:

```runa
> install_dashboard() -> () {
    | scope Dashboard {
        ~ readings = subject()

        ~ readings
            | x -> { @ print("dashboard: " + show(x)) }
    }
}
```

When the scope is torn down or drops, its live subscriptions are cancelled.
Derived streams built inside that scope are also scope-owned: their forwarder
tasks stop, and later settled reads observe the frozen scope-local stream
history instead of waiting on a dead pipeline.

This is the preferred shape for:

- UI/component-like lifetimes
- monitoring sessions
- scoped actor/subject orchestration
- temporary subscriptions that must stop deterministically

## Rejected Shapes

Ordinary functions may not start live subscriptions outside a named scope:

```runa
> install_bad(readings) -> () {
    ~ readings | x -> { @ print(show(x)) }   -- compile error
}

> loop_bad(readings) -> () {
    for x in readings {                       -- compile error
        @ print(show(x))
    }
}
```

The compiler rejects these forms because they would otherwise create detached
background tasks whose lifetime is not owned by the function.

## What To Do In Functions Instead

If a function needs to work with stream data, prefer one of these shapes:

### Return a stream

```runa
> alert_stream(readings) {
    readings |> filter(|x| x > 30)
}
```

The caller decides where and how to subscribe. Avoid hiding derived stream work
behind local bindings inside ordinary functions; either return the derived
stream expression directly or place the pipeline inside a named scope.

### Consume a snapshot or terminal result

```runa
> latest_label(readings) -> String {
    "latest: " + readings.latest
}

> current_total(xs) -> Int {
    xs |> sum
}
```

This keeps the function synchronous and bounded.

### Require a named scope at the call site

```runa
| scope Monitor {
    ~ readings = subject()

    ~ readings
        | x -> { @ print("monitor: " + show(x)) }
}
```

The caller chooses the lifetime boundary explicitly.

## Teardown Semantics

Named scopes are the lifetime owner for the live subscriptions they create.

### Ordering

Scope-end teardown runs in this order:

1. **Cancel scope-owned subscriptions.** Active `~ stream | x -> { ... }` arms
   stop receiving values. In-flight handler bodies finish their current value
   and then drop.
2. **Cancel scope-owned derived operator handles.** Every forwarder task
   spawned by `map` / `filter` / `scan` / etc. inside the scope is cancelled.
   The derived stream is now frozen at its last forwarded value.
3. **Unregister barrier expectations.** Any `__fut_settle` barriers that were
   pinned to scope-owned streams release without waiting on dead pipelines.
4. **Drop the scope guard.** The runtime structure that tracked (1)–(3) is
   dropped; a re-entry of the same scope name starts fresh.

This ordering is what keeps post-teardown sends safe (they reach (1) which is
already cancelled) and what keeps post-teardown settled reads bounded (they
hit (3) which has released).

### Triggers

- normal scope exit at end-of-block
- `@ teardown("ScopeName")` from anywhere — cancels the named scope early
- a diagnostic that aborts the scope body

### What it does not do

- it does not retroactively "un-send" values that subscribers have already
  observed
- it does not cancel actor tasks spawned with `spawn(...)`; actors have their
  own lifetime owned by their handle, not by the scope
- it does not cancel top-level subscriptions made before the scope opened

This contract is part of why stateful canaries and lifecycle tests exist in
the verification stack.

## Relationship To Library Hygiene

Importable library files should not rely on top-level live subscriptions.

Top-level subscriptions are script-lifetime behavior, not import-safe library
surface. Use [docs/library-hygiene.md](library-hygiene.md) and
`runa lint-library` to keep that boundary explicit.

## Crossing Function and Scope Boundaries

The contract gives a single rule for each direction:

| Direction | Rule |
|---|---|
| Live stream **into** an ordinary function (parameter) | allowed; the function may snapshot it but may not subscribe or derive from it |
| Live stream **out** of an ordinary function (return) | allowed; the function returns the stream expression and the caller decides where to subscribe |
| Live stream **into** a named scope (closed-over) | allowed; the scope may subscribe and derive freely |
| Live stream **out** of a named scope (return) | **not currently supported** — see "Returned-stream ownership" below |
| Subscription **across** a scope boundary | a subscription is owned by the scope it is *created in*, regardless of where the source stream came from |

The asymmetry is intentional: parameters can carry live streams in, but
ordinary functions cannot start a subscription on them, so no detached task is
created at the call site.

## Open Design Decisions

### Explicit subscription handles

**Status: deferred.** Tracked as `td-b48d46`.

Futuruna does **not** currently have a first-class subscription handle type.
There is no supported form like:

```runa
> install(readings) -> Subscription {
    ~ readings | x -> { @ print(show(x)) }
}

= handle = install(readings)
@ cancel(handle)
```

That design is deliberately deferred. A first-class handle would need a clear
contract for at least:

- ownership transfer: who must keep the handle alive, and what happens when it
  is dropped
- cancellation ordering relative to in-flight handler bodies and derived
  stream forwarders
- whether handles are `Send`, clonable, or single-owner values
- whether imported libraries may return handles without becoming
  script-lifetime code
- how `@ teardown("ScopeName")` composes with separately returned handles

Until those questions are answered, the production rule is:

- return a stream expression when code wants to factor a pipeline
- open the subscription in a named scope chosen by the caller
- use `@ teardown("ScopeName")` for explicit early cancellation

Example:

```runa
> alerts(readings) {
    readings |> filter(|x| x > 30)
}

| scope Monitor {
    ~ alerts(readings)
        | x -> { @ print("alert: " + show(x)) }
}
```

This keeps the lifetime owner visible in source and keeps importable helpers
from smuggling background work through ordinary function calls.

### Function-as-scope

**Status: decided: stay with explicit named scopes.**

The current contract requires an explicit `| scope Name { ... }` even when
the function body is the obvious lifetime container for the subscription.
That explicit form is the language contract for now. Ordinary function frames
do **not** implicitly own live subscriptions, and Futuruna does not currently
support anonymous `| scope { ... }` blocks.

Rationale:

- the lifetime owner should be visible at the call site and in diagnostics
- function calls are too easy to treat as ordinary helpers, especially in
  importable library code, so letting them spawn owned async work would hide a
  material effect behind a normal call
- named scopes already line up with teardown (`@ teardown("Name")`), generated
  guard names, and scope-field access such as `Dashboard.label`
- the ergonomic cost is a small explicit name, while the safety benefit is a
  stable ownership boundary that users and canaries can reason about

If real programs show that naming scopes is noisy but ownership should remain
explicit, the next candidate is a deliberately designed anonymous-scope form.
That is not part of the current contract.

## Current Status

This surface is [Stable](feature-stages.md), and the ownership rule is part of
the production contract:

- named scopes own live subscription lifetimes
- named scopes own derived async stream operator tasks created inside them
- detached function-local live subscriptions are rejected
- snapshot reads (`.latest`, `.count`, terminal reductions, etc.) are allowed
  in ordinary functions because they do not spawn background work

Future work may add more advanced explicit ownership forms (see "Open Design
Decisions" above), but implicit detached background subscriptions are not the
direction.
