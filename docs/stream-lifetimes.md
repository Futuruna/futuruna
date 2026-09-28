---
feature_stage: stable
feature_stage_surfaces:
  - reactive-stateful-surfaces
---

# Stream Lifetimes

This document defines the lifetime contract for live Futuruna stream
consumption.

It should be read alongside:

- [docs/reference/streams.md](reference/streams.md) — the semantics of subjects,
  live streams, subscriptions and actors
- [docs/reference/runes.md](reference/runes.md)
- [docs/feature-stages.md](feature-stages.md)
- [docs/library-hygiene.md](library-hygiene.md)

## The Core Rule

Live stream consumption must have an explicit owner.

In Futuruna, that owner is either the script itself or a named `| scope`.

That means:

- top-level subscriptions are script-lifetime work
- subscriptions inside a named scope are scope-lifetime work: they stop at
  `@ teardown("Name")`
- ordinary functions must not create live subscriptions that no name owns

## What Counts As Live Stream Consumption

These forms keep receiving values after the statement that created them, when
they target a subject or a stream derived from one:

```runa
~ readings
    | x -> { @ print(show(x)) }

for x in readings {
    @ print(show(x))
}

~ projected = readings |> filter(|x| x > 30) |> map(|x| x + 1)
```

A subscription or `for` loop runs its body for every later value. A derived
stream (`projected`) receives every later value of its source.

The compiler's validation predicate is `is_live_stream_expr_for_validation` in
`src/bin/runa.rs`. An expression counts as live when it is:

- a variable bound to a `subject()` or to a live derived stream, or
- an `as_stream(...)` of a live source, or
- a live operator over a live source — `map`, `filter`, `scan`, `take`,
  `skip`, `tap`, `merge`, `start_with`, `concat`, `catch` (the
  `LIVE_STREAM_OPERATORS` table shared by the interpreter and the compiler) —
  including the `|>` form.

## Snapshot Reads (Allowed Without Scope)

A "snapshot" read observes a stream once and keeps nothing running. Snapshot
reads are allowed in ordinary functions because they end with the call.

Snapshot reads include:

```runa
> latest_label(s) -> String { "latest: " + s.latest }
> count_so_far(s) -> Int { s.count }
> total(xs: List(Int)) -> Int { xs |> sum }
> seen(s) -> List(Int) { collect(s) }
```

The boundary is intentional and narrow: anything that goes through one of the
live operators listed above is *not* a snapshot, even when its result "looks"
terminal. If you want a terminal value from a derived pipeline, build the
pipeline in a scope first.

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

Inside a named scope, the scope owns the subscription:

```runa
> install_dashboard(readings) -> () {
    | scope Dashboard {
        ~ readings
            | x -> { @ print("dashboard: " + show(x)) }
    }
}
```

The subscription keeps receiving values after `install_dashboard` returns,
until `@ teardown("Dashboard")`. Derived streams built inside the scope are
also scope-owned: after teardown they stop following their source, and reads
such as `collect(Dashboard.projected)` see the values they held at teardown.

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

The compiler rejects these forms because no name would own the subscription,
so nothing could stop it.

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

This keeps the function bounded.

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

Named scopes are the lifetime owner for the live subscriptions and derived
streams created while their body runs.

### What teardown does

`@ teardown("ScopeName")` stops every subscription, `for` loop and derived
stream link the scope owns:

1. Scope-owned subscriptions and `for` loops receive no further values.
2. Scope-owned derived streams stop following their sources and keep the values
   they held.
3. Sends to the sources keep working: other subscribers still receive them.

A scope entered several times (for example by calling a function that contains
it) owns everything created in every entry; one teardown stops all of it.

Delivery is synchronous (see
[streams.md](reference/streams.md#live-stream-semantics)), so a value sent
before the teardown has been handled completely by the time teardown runs, and
a value sent after it never reaches the stopped subscriptions.

### What it does not do

- it does not retroactively "un-send" values that subscribers have already
  observed
- it does not stop actors created with `spawn(...)`; an actor lives as long as
  a handle to it exists
- it does not stop top-level subscriptions or subscriptions owned by other
  scopes
- it does not remove the scope's bindings: `Dashboard.values` stays readable

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
| Live stream **into** an ordinary function (parameter) | allowed; the function shares the stream: it may send to a subject and take snapshots, but may not subscribe or derive from it outside a named scope |
| Live stream **out** of an ordinary function (return) | allowed; the function returns the stream expression and the caller decides where to subscribe |
| Live stream **into** a named scope (closed-over) | allowed; the scope may subscribe and derive freely |
| Subscription **across** a scope boundary | a subscription is owned by the scope it is *created in*, regardless of where the source stream came from |

The asymmetry is intentional: parameters can carry live streams in, but
ordinary functions cannot start a subscription on them, so no unowned
subscription is created at the call site.

## Open Design Decisions

### Explicit subscription handles

**Status: deferred.**

Futuruna does **not** have a first-class subscription handle type. There is no
supported form like:

```runa
> install(readings) -> Subscription {
    ~ readings | x -> { @ print(show(x)) }
}

= handle = install(readings)
@ cancel(handle)
```

A first-class handle would need a clear contract for at least:

- ownership transfer: who must keep the handle alive, and what happens when it
  is dropped
- whether handles are clonable or single-owner values
- whether imported libraries may return handles without becoming
  script-lifetime code
- how `@ teardown("ScopeName")` composes with separately returned handles

Until those questions are answered, the rule is:

- return a stream expression when code wants to factor a pipeline
- open the subscription in a named scope chosen by the caller
- use `@ teardown("ScopeName")` to stop it

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
from hiding subscriptions behind ordinary function calls.

### Function-as-scope

**Status: decided: stay with explicit named scopes.**

The contract requires an explicit `| scope Name { ... }` even when the
function body is the obvious lifetime container for the subscription.
Ordinary function frames do **not** own live subscriptions, and Futuruna does
not support anonymous `| scope { ... }` blocks.

Rationale:

- the lifetime owner should be visible at the call site and in diagnostics
- function calls are too easy to treat as ordinary helpers, especially in
  importable library code, so letting them start owned subscriptions would
  hide a material effect behind a normal call
- named scopes line up with teardown (`@ teardown("Name")`) and scope-field
  access such as `Dashboard.label`
- the ergonomic cost is a small explicit name, while the benefit is a stable
  ownership boundary that users and canaries can reason about

## Summary

- named scopes own live subscriptions and derived streams created inside them
- unowned function-local live subscriptions are rejected
- snapshot reads (`.latest`, `.count`, `collect`, terminal reductions) are
  allowed in ordinary functions because they keep nothing running

See [feature stages](feature-stages.md) for the stability of this surface.
