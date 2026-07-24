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

That means:

- scope exit cancels scope-owned live subscriptions
- scope exit cancels scope-owned derived stream forwarders
- `@ teardown("ScopeName")` cancels them early
- post-teardown sends should not keep invoking the torn-down subscribers
- post-teardown settled reads of scope-owned derived streams should not hang on
  stale barrier links

This contract is part of why stateful canaries and lifecycle tests exist in the
verification stack.

## Relationship To Library Hygiene

Importable library files should not rely on top-level live subscriptions.

Top-level subscriptions are script-lifetime behavior, not import-safe library
surface. Use [docs/library-hygiene.md](library-hygiene.md) and
`runa lint-library` to keep that boundary explicit.

## Current Status

This surface is still [Preview](feature-stages.md), but the ownership rule
itself is deliberate:

- named scopes own live subscription lifetimes
- named scopes own derived async stream operator tasks created inside them
- detached function-local live subscriptions are rejected

Future work may add more advanced explicit ownership forms, but implicit
detached background subscriptions are not the direction.
