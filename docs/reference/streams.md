---
feature_stage: preview
feature_stage_surfaces:
  - reactive-stateful-surfaces
---

# Reactive Streams

Reactive streams are syntax, not a library. `~` declares them, `|>` composes them, `~ + |` consumes them.

## Creating Streams

### From a list
```runa
~ nums = from_list([1, 2, 3, 4, 5])
~ letters = from_list(["a", "b", "c"])
```

### From a range
```runa
~ nums = from_list(range(1, 11))    -- [1, 2, 3, ..., 10]
```

### From a subject (push-based)
```runa
~ clicks = subject()
clicks <- "click1"
clicks <- "click2"
```

## Stream Operators

All stream operators take the stream as the first argument. Use directly or with `|>`.

### Transformation

| Operator | Signature | Description |
|----------|-----------|-------------|
| `map` | `(Stream(a), a -> b) -> Stream(b)` | Transform each element |
| `flat_map` | `(Stream(a), a -> Stream(b)) -> Stream(b)` | Map and flatten |
| `enumerate` | `Stream(a) -> Stream((Int, a))` | Attach index to each element |

```runa
~ doubled = nums |> map(|x| x * 2)
~ indexed = nums |> enumerate
```

### Filtering

| Operator | Signature | Description |
|----------|-----------|-------------|
| `filter` | `(Stream(a), a -> Bool) -> Stream(a)` | Keep elements where predicate is true |
| `take` | `(Stream(a), Int) -> Stream(a)` | Take first N elements |
| `skip` | `(Stream(a), Int) -> Stream(a)` | Skip first N elements |
| `distinct` | `Stream(a) -> Stream(a)` | Remove consecutive duplicates |

```runa
~ big = nums |> filter(|x| x > 3)
~ first3 = nums |> take(3)
~ after2 = nums |> skip(2)
```

### Accumulation

| Operator | Signature | Description |
|----------|-----------|-------------|
| `scan` | `(Stream(a), b, (b, a) -> b) -> Stream(b)` | Running fold, emitting each accumulator |

```runa
~ running_sum = nums |> scan(0, |acc, x| acc + x)
-- emits: [1, 3, 6, 10, 15]
```

### Combination

| Operator | Signature | Description |
|----------|-----------|-------------|
| `merge` | `(Stream(a), Stream(a)) -> Stream(a)` | Interleave two streams |
| `zip` | `(Stream(a), Stream(b)) -> Stream((a, b))` | Pair elements by position |
| `combine_latest` | `(Stream(a), Stream(b)) -> Stream((a, b))` | Combine with latest value from each |

```runa
~ merged = merge(odds, evens)
~ pairs = zip(names, scores)
```

### Windowing

| Operator | Signature | Description |
|----------|-----------|-------------|
| `window` | `(Stream(a), Int) -> Stream(List(a))` | Sliding window of size N |

```runa
~ windows = nums |> window(3)
-- emits: [[1,2,3], [2,3,4], [3,4,5]]
```

### Terminal operations

These consume the stream and return a single value.

| Operator | Signature | Description |
|----------|-----------|-------------|
| `count` | `Stream(a) -> Int` | Count elements |
| `sum` | `Stream(Int) -> Int` | Sum all elements |
| `last` | `Stream(a) -> a` | Last element |
| `any` | `(Stream(a), a -> Bool) -> Bool` | Any element matches? |
| `all` | `(Stream(a), a -> Bool) -> Bool` | All elements match? |

```runa
= total = nums |> sum
= count = nums |> count
= has_big = nums |> any(|x| x > 100)
```

### Side Effects & Error Recovery

| Operator | Signature | Description |
|----------|-----------|-------------|
| `tap` | `(Stream(a), a -> ()) -> Stream(a)` | Side-effect observation: calls fn for each element, returns stream unchanged |
| `catch` | `(Stream(a), Err -> Stream(a)) -> Stream(a)` | Error recovery: in sync mode, pass-through (no errors in Vec) |

```runa
~ result = raw_data
    |> tap(|x| @ print("saw: " + show(x)))    -- observe without consuming
    |> catch(|e| from_list([fallback_value]))   -- recover mid-pipeline
    |> map(transform)
```

`tap` observes values passing through without consuming the stream. `catch` recovers from errors mid-pipeline, replacing the failed portion with a recovery stream. Both return the stream for further chaining — they are NOT terminals.

### Prepending & Concatenation

| Operator | Signature | Description |
|----------|-----------|-------------|
| `start_with` | `(Stream(a), a) -> Stream(a)` | Prepend a value to the front of a stream |
| `concat` | `(Stream(a), Stream(a)) -> Stream(a)` | Concatenate two streams sequentially |

```runa
~ nums = from_list([2, 3, 4])
~ with_one = nums |> start_with(1)         -- [1, 2, 3, 4]
~ both = concat(from_list([1, 2]), from_list([3, 4]))  -- [1, 2, 3, 4]
```

### Pairing & Tuple Access

| Operator | Signature | Description |
|----------|-----------|-------------|
| `pairwise` | `Stream(a) -> Stream((a, a))` | Emit consecutive pairs: `[1,2,3]` becomes `[(1,2),(2,3)]` |
| `fst` | `(a, b) -> a` | Return first element of a tuple/pair |
| `snd` | `(a, b) -> b` | Return second element of a tuple/pair |

```runa
~ nums = from_list([1, 2, 3, 4])
~ pairs = nums |> pairwise               -- [(1,2), (2,3), (3,4)]
~ firsts = pairs |> map(|p| fst(p))      -- [1, 2, 3]
~ seconds = pairs |> map(|p| snd(p))     -- [2, 3, 4]
```

### Additional Terminal Operations

| Operator | Signature | Description |
|----------|-----------|-------------|
| `first` | `Stream(a) -> a` | Return the first element of a stream (or Unit if empty) |
| `reduce` | `(Stream(a), b, (b, a) -> b) -> b` | Terminal fold: reduce stream to a single value |

```runa
~ nums = from_list([10, 20, 30])
= head = nums |> first                           -- 10
= total = nums |> reduce(0, |acc, x| acc + x)    -- 60
```

## Pipe Operator (`|>`)

The pipe operator inserts the left side as the first argument of the right side:

```runa
x |> f           -- f(x)
x |> f(a, b)     -- f(x, a, b)
x |> f |> g      -- g(f(x))
```

Chains compose naturally:
```runa
~ result = from_list(range(1, 101))
    |> filter(|x| x % 2 == 0)
    |> map(|x| x * x)
    |> take(10)
```

## Subjects (Push-Based Streams)

Subjects are mutable streams you can push values into.

### Creation
```runa
~ clicks = subject()              -- empty subject
~ temp = subject(20.0)            -- with initial value (BehaviorSubject)
~ history = subject(0, 10)        -- replay last 10 values (ReplaySubject, positional)
```

### Pushing values
```runa
clicks <- "event1"
clicks <- "event2"
temp <- 25.0
```

### Properties
```runa
clicks.count       -- number of values in the subject
temp.latest        -- most recent value
```

### Using subjects as streams
Subjects work anywhere a stream does:
```runa
~ data = subject()
data <- 1
data <- 2
data <- 3

~ doubled = data |> map(|x| x * 2)
= total = data |> sum           -- 6
```

### Converting to read-only stream
```runa
~ stream = as_stream(my_subject)
```

### Completing a subject
```runa
complete(my_subject)               -- mark as done
error(my_subject, "something failed")  -- terminate with error
```

## Scopes (Lifecycle Management)

Scopes group stream operations with lifecycle management:

```runa
| scope Dashboard {
    ~ readings = subject()
    ~ alerts = readings |> filter(|r| r.severity > 3)

    ~ alerts
        | a -> { notify(a.message) }
        | Err(e) -> { log_error(e) }
}
-- Dashboard exits -> all subscriptions torn down, zero leaks
```

When a scope ends, its subjects, streams, and subscriptions are cleaned up.
Named scopes are also the explicit lifetime owner for live subscriptions started
inside ordinary functions. See [docs/stream-lifetimes.md](../stream-lifetimes.md)
for the full contract.

---

## Subscriptions (`~ + |`)

Subscriptions are the terminal consumption mechanism for streams. They use `~` to open the subscription and `|` arms to handle stream events: values, errors, and completion.

### Why not `for`?

`for` is pull-based with no error/completion handling. `~ + |` maps directly to the three stream events and compiles to a tokio broadcast receive loop. Use `for` for lists and ranges; use `~ + |` for streams.

### The three stream events

Every stream produces exactly three kinds of events:

| Event | Meaning | Tokio mapping |
|-------|---------|---------------|
| Value | A new value arrived | `Ok(x)` from `rx.recv().await` |
| Error | Something went wrong | `Err(RecvError::Lagged(n))` |
| Complete | Stream has ended | `Err(RecvError::Closed)` |

The `~ + |` arms correspond directly to these three events.

### Syntax forms

#### Single handler (most common)
```runa
~ stream | x -> { handle(x) }
```

One arm, handles each value. Errors silently terminate the subscription (same as current `for` behavior, but explicit). Use this when you don't care about errors.

#### With error handling
```runa
~ stream
    | x -> { handle(x) }
    | Err(e) -> { recover(e) }
```

Two arms. `Err(e)` catches stream errors. The subscription continues after handling the error (the error is not fatal to the subscription).

#### Full lifecycle
```runa
~ stream
    | x -> { handle(x) }
    | Err(e) -> { recover(e) }
    | Complete -> { finalize() }
```

Three arms. `Complete` fires when the stream ends normally (all senders dropped, or `complete(subject)` called). This is the full form — every stream event is handled.

#### Pipeline ending in subscription
```runa
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
    | Err(e) -> { log(e) }
```

Pipe operators transform the stream; `|` arms subscribe to the result. The pipeline composes transformations, the subscription consumes them.

#### Pipeline with mid-stream recovery AND terminal handling
```runa
~ api_data
    |> catch(|e| from_list([fallback]))    -- recover mid-pipeline
    |> map(transform)
    | result -> { save(result) }              -- consume at terminal
    | Err(e) -> { alert(e) }                 -- only uncaught errors reach here
```

`catch` recovers within the pipeline (the stream continues). `| Err(e)` handles errors that survive the pipeline.

### Binding and subscription are separate statements

A `~` statement is either a **binding** or a **subscription**, never both:

```runa
-- Binding: ~ name = expr
~ temps = sensor |> map(to_celsius)

-- Subscription: ~ expr | arms
~ temps
    | t -> { display(t) }
    | Err(e) -> { log(e) }
```

The `=` determines binding vs subscription. `~ name = expr` binds. `~ expr | arms` subscribes.

This means you can subscribe to the same stream multiple times:

```runa
~ temps = sensor |> map(to_celsius)

-- Two independent subscriptions
~ temps | t -> { display(t) }
~ temps | t -> { log_to_file(t) }
```

And you can subscribe to an inline pipeline without naming it:

```runa
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
```

### Scoped subscriptions

Inside a `| scope` block, subscriptions are torn down when the scope exits:

```runa
| scope WeatherApp {
    ~ readings = subject()
    ~ alerts = readings |> filter(|r| r.severity > 3)

    -- Both subscriptions die when WeatherApp scope exits
    ~ alerts
        | a -> { notify(a.message) }
        | Err(e) -> { log_error(e) }
        | Complete -> { @ print("stream ended") }

    ~ readings
        | r -> { update_dashboard(r) }
}
-- Scope exits -> all subscriptions cancelled, channels closed
```

### Function boundaries

Ordinary functions may not start live subscriptions unless a named scope owns
them:

```runa
> install_bad(readings) -> () {
    ~ readings | x -> { @ print(show(x)) }   -- compile error
}

> install_ok(readings) -> () {
    | scope Monitor {
        ~ readings | x -> { @ print(show(x)) }
    }
}
```

The same rule applies to `for x in stream { ... }` when `stream` is a live
subject or derived async stream. This keeps compiled async subscriptions from
quietly outliving the function that created them.

### Migrating from `for`

| Before (for loop) | After (subscription) |
|-------------------|---------------------|
| `for x in stream { body }` | `~ stream \| x -> { body }` |
| `for x in stream { if err { handle } }` | `~ stream \| x -> { body } \| Err(e) -> { handle }` |
| `for x in list { body }` | `for x in list { body }` (unchanged) |

`for` remains correct for lists and ranges. Only stream consumption migrates to `~ + |`.

### Design rationale

`~` means "what flows" — subscription is where flow meets action. `|` arms use the same `| pattern -> { body }` syntax as `match`. Stream events are just another thing to pattern match on. The three arms map 1:1 to tokio broadcast `recv()` outcomes — no wrappers, no indirection.

Pipeline operators and subscription arms are complementary: `catch` recovers within the pipeline (stream continues), `| Err` handles errors at the terminal (subscription level), `tap` observes mid-pipeline, `| x ->` consumes at the terminal.
