---
feature_stage: experimental
feature_stage_surfaces:
  - reactive-stateful-surfaces
---

# Reactive Streams

Reactive streams are syntax, not a library. `~` declares them, `|>` composes them, `~ + |` consumes them.

A stream is either **finite** or **live**:

- A **finite stream** holds values that are all known: `from_list(...)`, `~[...]`, and
  operators applied to finite streams.
- A **live stream** receives values over time: a subject, or an operator applied to a
  live stream.

Both execution modes (`runa file.runa` and `runa run`) implement the rules on this page.

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
| `flat_map` | `(Stream(a), a -> Stream(b)) -> Stream(b)` | Map and flatten (finite streams) |
| `enumerate` | `Stream(a) -> Stream((Int, a))` | Attach index to each element (finite streams) |

```runa
~ doubled = nums |> map(|x| x * 2)
~ indexed = nums |> enumerate
```

### Filtering

| Operator | Signature | Description |
|----------|-----------|-------------|
| `filter` | `(Stream(a), a -> Bool) -> Stream(a)` | Keep elements where predicate is true |
| `take` | `(Stream(a), Int) -> Stream(a)` | Take first N elements, then complete |
| `skip` | `(Stream(a), Int) -> Stream(a)` | Skip first N elements |
| `distinct` | `Stream(a) -> Stream(a)` | Remove consecutive duplicates (finite streams) |

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
| `zip` | `(Stream(a), Stream(b)) -> Stream((a, b))` | Pair elements by position (finite streams) |
| `combine_latest` | `(Stream(a), Stream(b)) -> Stream((a, b))` | Combine with latest value from each (finite streams) |

```runa
~ merged = merge(odds, evens)
~ pairs = zip(names, scores)
```

`merge` alternates the values both sources already hold, then follows the order in
which new values arrive. It completes when both sources have completed and ends with
the first error either source ends with.

### Windowing

| Operator | Signature | Description |
|----------|-----------|-------------|
| `window` | `(Stream(a), Int) -> Stream(List(a))` | Sliding window of size N (finite streams) |

```runa
~ windows = nums |> window(3)
-- emits: [[1,2,3], [2,3,4], [3,4,5]]
```

### Terminal operations

These read the values a stream holds now and return a single value.

| Operator | Signature | Description |
|----------|-----------|-------------|
| `collect` | `Stream(a) -> List(a)` | The values as a list |
| `count` | `Stream(a) -> Int` | Count elements |
| `sum` | `Stream(Int) -> Int` | Sum all elements |
| `last` | `Stream(a) -> a` | Last element; raises `last: empty list` when empty |
| `any` | `(Stream(a), a -> Bool) -> Bool` | Any element matches? |
| `all` | `(Stream(a), a -> Bool) -> Bool` | All elements match? |

```runa
= total = nums |> sum
= element_count = nums |> count
= has_big = nums |> any(|x| x > 100)
```

### Side Effects & Error Recovery

| Operator | Signature | Description |
|----------|-----------|-------------|
| `tap` | `(Stream(a), a -> ()) -> Stream(a)` | Calls the function for each element and passes the element on |
| `catch` | `(Stream(a), String -> Stream(a)) -> Stream(a)` | When the stream ends with an error, continues with the recovery stream |

```runa
~ result = raw_data
    |> tap(|x| @ print("saw: " + show(x)))       -- observe without consuming
    |> catch(|e| from_list([fallback_value]))   -- recover mid-pipeline
    |> map(transform)
```

`tap` observes values passing through without consuming the stream. `catch` receives
the error message and replaces the error ending with the values of the recovery
stream. Both return a stream for further chaining — they are not terminals.

### Prepending & Concatenation

| Operator | Signature | Description |
|----------|-----------|-------------|
| `start_with` | `(Stream(a), a) -> Stream(a)` | Prepend a value to the front of a stream |
| `concat` | `(Stream(a), Stream(a)) -> Stream(a)` | All values of the first stream, then those of the second |

```runa
~ nums = from_list([2, 3, 4])
~ with_one = nums |> start_with(1)         -- [1, 2, 3, 4]
~ both = concat(from_list([1, 2]), from_list([3, 4]))  -- [1, 2, 3, 4]
```

`concat` holds values of the second stream until the first stream completes.

### Pairing & Tuple Access

| Operator | Signature | Description |
|----------|-----------|-------------|
| `pairwise` | `Stream(a) -> Stream((a, a))` | Emit consecutive pairs (finite streams): `[1,2,3]` becomes `[(1,2),(2,3)]` |
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
| `first` | `Stream(a) -> a` | First element; raises `first: empty list` when empty |
| `reduce` | `(Stream(a), b, (b, a) -> b) -> b` | Terminal fold: reduce stream to a single value |

```runa
~ nums = from_list([10, 20, 30])
= head = nums |> first                           -- 10
= total = nums |> reduce(0, |acc, x| acc + x)    -- 60
```

### Operators on live streams

On a live stream, `map`, `filter`, `scan`, `take`, `skip`, `tap`, `merge`,
`start_with`, `concat`, and `catch` build a live stream that follows its source.
The terminal operations read the values the stream holds now.

`flat_map`, `enumerate`, `distinct`, `zip`, `window`, `combine_latest`, `pairwise`,
`debounce`, `throttle`, `delay`, `buffer`, `timeout`, `switch_map`, `sample`, and
`take_until` are defined only for finite streams. Applying one of them to a live
stream is an error; read the values with `collect(stream)` first.

Other functions that take a list do not accept a live stream; use `collect(stream)`.

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

Subjects are live streams you can push values into.

### Creation
```runa
~ clicks = subject()              -- empty; keeps every value
~ temp = subject(20.0)            -- starts with 20.0; keeps every value
~ history = subject(0, 10)        -- starts with 0; keeps the last 10 values
```

`subject()` and `subject(initial)` keep every value they emit, so they grow with each
value. `subject(initial, keep)` keeps only the last `keep` values (`keep` is an `Int`
of at least 0); use it for long-lived subjects. A stream derived from a live source
keeps as many values as its source; `merge` and `concat` keep as many as the larger
of their sources, or every value when either source keeps every value.

### Pushing values
```runa
clicks <- "event1"
clicks <- "event2"
temp <- 25.0
```

### Properties
```runa
clicks.count       -- number of values the stream has emitted, including the initial value
temp.latest        -- most recent value; an error when the stream has no values
```

`.count` and `.latest` work on every live stream, whatever it keeps.

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

`as_stream` returns the same stream without write access: `<-`, `complete`, and
`error` on it are errors. Streams derived from a subject are read-only too.

### Completing a subject
```runa
complete(my_subject)                   -- end normally
error(my_subject, "something failed")  -- end with an error message
```

## Live Stream Semantics

These rules define live streams in both execution modes.

1. **Streams are shared.** A subject or live stream is a reference. Binding it to
   another name, passing it to a function, storing it in a value, or importing it
   refers to the same stream: `<-` through any of them reaches every subscriber.
2. **Delivery is synchronous.** `subject <- value` returns after every subscription
   and every derived stream of the subject has handled the value, including streams
   derived from those streams. Subscriptions and derived streams receive each value in
   the order they were created.
3. **Values stay in order.** A value sent to a stream while that stream is still
   delivering an earlier value is delivered after the earlier value has reached every
   subscriber. Every subscriber sees a stream's values in the same order.
4. **Late subscribers catch up.** A subscription or derived stream created after
   values were sent first receives the values the stream keeps, in order, then the
   stream's end if it has ended, then every later value.
5. **A stream ends once.** `complete(s)` ends a subject normally; `error(s, message)`
   ends it with an error. Every subscription's `Complete` arm runs once, or its
   `Err(e)` arm runs once with `e` bound to the message. Derived streams end with their
   source (`take` also completes after its last value).
6. **An ended subject accepts nothing.** `<-`, `complete`, or `error` on a subject
   that has ended is a runtime error naming the ending.
7. **Errors are handled or fatal.** When a stream ends with an error, a subscription
   without an `Err(e)` arm, a `for` loop over the stream, and every terminal operation
   on it (`collect`, `count`, `sum`, ...) fail with the error message.
8. **Every value is handled.** A value that matches none of a subscription's value
   arms is a runtime error, as for `match`.
9. **Nothing is left pending.** Because delivery is synchronous, every value sent has
   been handled when the program ends.

```runa
~ s = subject()
~ s | x -> { @ print("A got " + show(x)) }
s <- 1                                  -- A got 1
~ s | x -> { @ print("B got " + show(x)) }   -- B got 1 (catches up)
s <- 2                                  -- A got 2, then B got 2
complete(s)
@ print(show(collect(s)))               -- [1, 2]
```

## Scopes (Lifecycle Management)

Scopes own the subscriptions and derived streams created inside them:

```runa
| scope Dashboard {
    ~ readings = subject()
    ~ alerts = readings |> filter(|r| r.severity > 3)

    ~ alerts
        | a -> { notify(a.message) }
        | Err(e) -> { log_error(e) }
}
-- @ teardown("Dashboard") stops these subscriptions
```

A scope's subscriptions and derived streams keep receiving values after the scope's
statements have run, until `@ teardown("ScopeName")`. After teardown they receive
nothing more; the scope's bindings keep the values they hold. Named scopes are also
the explicit lifetime owner for live subscriptions started inside ordinary functions.
See [docs/stream-lifetimes.md](../stream-lifetimes.md) for the full contract.

---

## Subscriptions (`~ + |`)

Subscriptions are the terminal consumption mechanism for streams. They use `~` to open the subscription and `|` arms to handle stream events: values, errors, and completion.

### Why not `for`?

`for x in stream { ... }` over a live stream is a subscription with one value arm:
its body runs for each value, and an error ending is a runtime error. `~ + |` adds
error and completion handling. Use `for` for lists and ranges; use `~ + |` for
streams.

### The three stream events

Every stream produces three kinds of events:

| Event | Meaning | Arm |
|-------|---------|-----|
| Value | A new value arrived | `\| x -> ...` (any pattern) |
| Error | The stream ended with an error | `\| Err(e) -> ...` (`e` is the message) |
| Complete | The stream ended normally | `\| Complete -> ...` |

A finite stream delivers its values and then completes.

### Syntax forms

Arms may follow on the same line or on the lines below; arms on following lines are
indented past the `~`.

#### Single handler (most common)
```runa
~ stream | x -> { handle(x) }
```

One arm, handles each value. If the stream ends with an error, the program stops
with that error.

#### With error handling
```runa
~ stream
    | x -> { handle(x) }
    | Err(e) -> { recover(e) }
```

`Err(e)` runs once when the stream ends with an error; `e` is the error message.

#### Full lifecycle
```runa
~ stream
    | x -> { handle(x) }
    | Err(e) -> { recover(e) }
    | Complete -> { finalize() }
```

`Complete` runs once when the stream ends normally: after the last value of a finite
stream, or when `complete(subject)` is called.

#### Pipeline ending in subscription
```runa
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
    | Err(e) -> { log(e) }
```

Pipe operators transform the stream; `|` arms subscribe to the result.

#### Pipeline with mid-stream recovery AND terminal handling
```runa
~ api_data
    |> catch(|e| from_list([fallback]))    -- recover mid-pipeline
    |> map(transform)
    | result -> { save(result) }              -- consume at terminal
    | Err(e) -> { alert(e) }                 -- only uncaught errors reach here
```

`catch` recovers within the pipeline. `| Err(e)` handles errors that survive the pipeline.

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

-- Two independent subscriptions, run in this order for each value
~ temps | t -> { display(t) }
~ temps | t -> { log_to_file(t) }
```

And you can subscribe to an inline pipeline without naming it:

```runa
~ sensor |> filter(valid) |> map(to_celsius)
    | t -> { display(t) }
```

### Scoped subscriptions

Inside a `| scope` block, subscriptions stop at `@ teardown` of the scope:

```runa
| scope WeatherApp {
    ~ readings = subject()
    ~ alerts = readings |> filter(|r| r.severity > 3)

    ~ alerts
        | a -> { notify(a.message) }
        | Err(e) -> { log_error(e) }
        | Complete -> { @ print("stream ended") }

    ~ readings
        | r -> { update_dashboard(r) }
}
@ teardown("WeatherApp")   -- both subscriptions stop here
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
stream. This keeps every live subscription owned by a name that `@ teardown` can
stop.

Futuruna does not expose subscription handles. Factor stream setup by returning a
stream expression, then subscribe inside the caller's named scope. Use
`@ teardown("ScopeName")` to stop the scope's subscriptions. See
[Stream Lifetimes](../stream-lifetimes.md) for the full ownership contract.

### Replacing `for` on streams

| Loop | Subscription |
|-------------------|---------------------|
| `for x in stream { body }` | `~ stream \| x -> { body }` |
| `for x in stream { body }` with error handling | `~ stream \| x -> { body } \| Err(e) -> { handle }` |
| `for x in list { body }` | `for x in list { body }` (unchanged) |

### Design rationale

`~` means "what flows" — subscription is where flow meets action. `|` arms use the same `| pattern -> { body }` syntax as `match`. Stream events are just another thing to pattern match on.

Pipeline operators and subscription arms are complementary: `catch` recovers within the pipeline (stream continues), `| Err` handles errors at the terminal (subscription level), `tap` observes mid-pipeline, `| x ->` consumes at the terminal.

---

## Actors

An actor holds a state value and handles messages one at a time.

```runa
> actor counter(state: Int) {
    | Inc -> state + 1
    | Add(n) -> state + n
}

= c = spawn(counter, 0)
c <- Inc                      -- handled before the next statement
= now = ask(c, Add(10))       -- 11: handles the message, returns the new state
```

1. **Actors are shared.** `spawn(actor, initial)` creates an actor. Binding the
   handle to another name, passing it to a function, or storing it in a value refers
   to the same actor: a message sent through any copy updates the one state.
2. **`<-` handles the message.** `actor <- message` runs the first handler whose
   pattern matches; its result becomes the new state. The send returns after the
   message has been handled.
3. **One message at a time.** A message sent to an actor while it is handling
   another message (for example from a subscription its handler triggers) is handled
   right after the current one, in send order.
4. **`ask` returns the new state.** `ask(actor, message)` handles the message like
   `<-` and returns the state it produced. Asking an actor that is handling a message
   is a runtime error, because the answer could never arrive.
5. **Every message has a handler.** A message no handler matches is a runtime error.
6. **Nothing is left pending.** Every message sent has been handled when the program
   ends.
