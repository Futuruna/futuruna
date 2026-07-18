# 6. Effects and Actors

## Algebraic effects

Effects make side effects explicit and composable:

```runa
-- Declare an effect
# effect Logger {
    > log(msg: String) -> ()
}

-- Use it in a function
> process(data: String) -> String with Logger {
    log("processing: " + data)
    to_upper(data)
}

-- Handle it
= result = | handle Logger {
    | log(msg) -> { @ print("[LOG] " + msg); resume(()) }
} in process("hello")

@ print(result)  -- "HELLO"
```

Different handlers = different behaviors. Same code, testable in isolation.

## Actors

```runa
> actor counter(state: Int) {
    | Increment -> state + 1
    | Decrement -> state - 1
    | Reset -> 0
}

= c = spawn(counter, 0)
c <- Increment
c <- Increment
c <- Increment
= val = ask(c, Increment)
@ print(show(val))  -- 4
```

Actors encapsulate mutable state behind message passing. No `Arc<Mutex<T>>`.

## The escape hatch

For the 1-5% that Futuruna can't express natively:

```runa
@ rust {
    fn fast_sort(x: &mut [f64]) {
        x.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    }
}
```

Raw Rust, embedded directly. Use sparingly.

## Next

[7. Building a Project](07-project.md)
