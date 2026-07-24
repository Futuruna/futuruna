# 5. Streams and Reactivity

## Cold streams (pipelines)

```runa
~ data = from_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
~ big = data |> filter(|x| x > 5) |> map(|x| x * 10)
@ print(show(collect(big)))  -- [60, 70, 80, 90, 100]
```

The `~` rune declares streams. `|>` composes operators. Streams are lazy — nothing runs until collected or subscribed.

## Hot streams (subjects)

```runa
~ clicks = subject()
clicks <- "button1"
clicks <- "button2"
clicks <- "button3"

@ print("count: " + show(clicks.count))    -- 3
@ print("latest: " + show(clicks.latest))  -- "button3"
```

Subjects are push-based streams. `<-` sends values. `.count` and `.latest` inspect state.

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
-- Monitor scope ends here — all streams automatically cleaned up
```

Scopes control when streams live and die. No manual unsubscribe. No memory leaks.
Named scopes are also the required owner for live subscriptions created inside
ordinary functions. If a function wants to start `~ stream | ...` or `for x in
stream { ... }` over a live subject/derived async stream, that work must live
inside a named `| scope`.

```runa
> install_monitor(readings) -> () {
    | scope Monitor {
        ~ readings | x -> { @ print(show(x)) }
    }
}
```

Detached function-local live subscriptions are rejected instead of silently
outliving the function that created them. See
[docs/stream-lifetimes.md](../stream-lifetimes.md) for the current contract.

## Stream operators

20+ operators: `map`, `filter`, `scan`, `merge`, `zip`, `take`, `skip`,
`distinct`, `flat_map`, `debounce`, `throttle`, `delay`, `buffer`,
`timeout`, `switch_map`, `sample`, `reduce`, `pairwise`, and more.

## Next

[6. Effects and Actors](06-effects.md)
