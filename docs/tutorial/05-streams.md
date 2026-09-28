---
feature_stage: stable
feature_stage_surfaces:
  - reactive-stateful-surfaces
---

# 5. Streams and Reactivity

## Finite streams (pipelines)

```runa
~ data = from_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
~ big = data |> filter(|x| x > 5) |> map(|x| x * 10)
@ print(show(collect(big)))  -- [60, 70, 80, 90, 100]
```

The `~` rune declares streams. `|>` composes operators. `from_list` makes a finite
stream: all its values are known.

## Live streams (subjects)

```runa
~ clicks = subject()
clicks <- "button1"
clicks <- "button2"
clicks <- "button3"

@ print("count: " + show(clicks.count)) -- count: 3
@ print("latest: " + show(clicks.latest)) -- latest: button3
```

Subjects are push-based streams. `<-` sends values. `.count` and `.latest` inspect state.

Subscriptions and derived streams are live: they see every value, including
values sent after they were created, and `<-` returns only after every
subscriber has handled the value:

```runa
~ clicks = subject()
~ clicks | c -> { @ print("clicked " + c) }
~ firsts = clicks |> take(2)
clicks <- "a"          -- clicked a
clicks <- "b"          -- clicked b
clicks <- "c"          -- clicked c
@ print(show(collect(firsts)))  -- [a, b]
complete(clicks)
```

A subject or stream is shared: passing it to a function or binding it to another
name refers to the same stream. After `complete(clicks)`, sending to it is an
error.

## Scoped lifecycle

```runa
| scope Monitor {
    ~ sensor = subject()
    sensor <- 22.5
    sensor <- 23.1
    sensor <- 21.8

    ~ alerts = sensor |> filter(|t| t > 23.0)
    @ print("alerts: " + show(collect(alerts)))
}
@ teardown("Monitor")   -- stops what the scope owns
```

A scope owns the subscriptions and derived streams created inside it;
`@ teardown("Monitor")` stops them. No manual unsubscribe per subscription.
Named scopes are also the required owner for live subscriptions created inside
ordinary functions. If a function wants to start `~ stream | ...` or `for x in
stream { ... }` over a live stream, that work must live inside a named `| scope`.

```runa
> install_monitor(readings) -> () {
    | scope Monitor {
        ~ readings | x -> { @ print(show(x)) }
    }
}
```

Function-local live subscriptions without a named scope are rejected. See
[docs/stream-lifetimes.md](../stream-lifetimes.md) for the lifetime contract and
[streams.md](../reference/streams.md#live-stream-semantics) for delivery rules.

## Stream operators

`map`, `filter`, `scan`, `take`, `skip`, `tap`, `merge`, `start_with`,
`concat`, and `catch` work on finite and live streams. `zip`, `window`,
`distinct`, `flat_map`, `pairwise` and the other operators in
[streams.md](../reference/streams.md) work on finite streams; `collect` a live
stream first.

## Next

[6. Effects and Actors](06-effects.md)
