# Reactive Futuruna: Subjects, Lifecycle, and the Weather App

**The gap:** `~ stream = source |> map(f) |> filter(p)` gives us derived streams —
cold, pull-based, pipeline-only. Real applications need two more things:

1. **Subjects** — streams you can push into (hot, imperative entry points)
2. **Lifecycle** — scoped teardown, so streams die when their context dies

RxJS solved both. Futuruna can do it better because Rust already has `Drop`.

## Subjects: The `<-` Bridge

A Subject is a stream you can write to. RxJS has `Subject`, `BehaviorSubject`,
`ReplaySubject`. Futuruna unifies this with the `<-` operator we already have from actors:

```tau
-- subject(initial?) creates a pushable stream
~ weather = subject()              -- no initial value (like Subject)
~ count = subject(0)               -- with initial value (like BehaviorSubject)
~ history = subject([], replay: 5) -- replays last 5 to new subscribers (like ReplaySubject)

-- Push with <-
weather <- Sunny(temp: 22.0)
count <- count.latest() + 1

-- Subscribe with for (same as derived streams)
~ weather | w -> {
    @ print("Weather changed: " + show(w))
}

-- Derive from subjects (subjects ARE streams)
~ hot_days = weather |> filter(|w| w.temp > 30.0)
```

**Why `<-` and not `.next()`:** Futuruna already has `<-` for actor sends. A subject
IS an actor with no logic — it just forwards what it receives. Same operator,
same mental model. The send operator creates the bridge between imperative
code and reactive pipelines.

### Subject Variants

| Futuruna | RxJS | Behavior |
|-----|------|----------|
| `subject()` | `new Subject()` | No initial value, hot |
| `subject(val)` | `new BehaviorSubject(val)` | Has `.latest()`, emits current to new subscribers |
| `subject(val, replay: n)` | `new ReplaySubject(n)` | Buffers last n values for late subscribers |

### `.latest()` — Synchronous Access

BehaviorSubjects in RxJS have `.getValue()`. In Futuruna:

```tau
~ temp = subject(20.0)
temp <- 25.0

-- .latest() gives the current value synchronously
= current = temp.latest()    -- 25.0

-- Use in expressions directly
if temp.latest() > 30.0 {
    @ print("It's hot!")
}
```

This is the bridge between `~` (time) and `=` (moment). `.latest()` collapses
a stream to its current point.

## Lifecycle: Scoped Streams

RxJS lifecycle is manual: `subscription.unsubscribe()`. Angular added
`takeUntilDestroyed()`. React has `useEffect` cleanup. All of these are
bolted-on afterthoughts.

Futuruna has blocks. Blocks have scope. **Streams die when their scope dies.**

### `| scope` for View Lifecycle

```tau
-- A view is a scope. When the scope exits, all streams inside are torn down.
| scope WeatherDashboard {

    -- Sources (subjects — pushed from outside)
    ~ raw_weather = subject()
    ~ user_location = subject("Copenhagen")

    -- Derived (torn down automatically when scope exits)
    ~ forecasts = raw_weather
        |> filter(|w| w.location == user_location.latest())
        |> map(|w| format_forecast(w))

    ~ alerts = raw_weather
        |> filter(|w| w.severity > 3)
        |> debounce(5000)

    -- Subscriptions (also torn down with scope)
    ~ forecasts | f -> {
        render("#forecast", f)
    }

    ~ alerts | a -> {
        notify(a.message)
    }
}
-- WeatherDashboard exits → every ~ binding unsubscribes, channels close, tasks cancel
```

**The Rust bridge:** Each `~` in a scope compiles to a `tokio::broadcast::channel`
+ a `JoinHandle`. The scope holds a `Vec<JoinHandle>`. When the scope drops,
all handles are aborted. Zero manual cleanup. Rust's `Drop` does what RxJS
needs `takeUntil` hacks for.

### `| scope` Nesting (Component Trees)

```tau
| scope App {

    ~ route = subject("/")

    | scope Header {
        ~ title = route |> map(route_to_title)
        ~ title | t -> { render("#title", t) }
    }

    | scope MainContent {

        -- Child scopes. Each torn down independently.
        | scope WeatherPanel {
            ~ weather = poll(fetch_weather, 30000)  -- poll every 30s
            ~ weather | w -> { render("#weather", w) }
        }

        | scope NewsPanel {
            ~ news = poll(fetch_news, 60000)
            ~ news | n -> { render("#news", n) }
        }
    }
}
-- App exits → Header, MainContent, WeatherPanel, NewsPanel all torn down
-- Navigate away from MainContent → only WeatherPanel + NewsPanel torn down
```

### Explicit Teardown (Current Contract)

Current Futuruna uses named scopes as the explicit lifetime owner. Manual
subscription handles are a deferred design, not a supported surface today:

```tau
| scope WeatherPanel {
    ~ weather | w -> { render(w) }
}

-- Later:
@ teardown("WeatherPanel")
```

Historical sketches sometimes described `subscribe()` returning a disposable
handle. That route is intentionally not part of the current contract because it
would let ordinary helpers hide background work behind returned values. The
supported shape is to return streams from helpers and subscribe inside the
caller's named scope.

## `poll()` — Interval + Async Fetch

RxJS has `timer()` + `switchMap()`. Futuruna makes polling a first-class pattern:

```tau
-- poll(async_fn, interval_ms) → stream that calls fn every interval
~ weather = poll(fetch_weather, 30000)

-- With backoff on error:
~ weather = poll(fetch_weather, 30000, backoff: exponential)

-- With immediate first fetch:
~ weather = poll(fetch_weather, 30000, immediate: true)
```

`poll` is sugar for:
```tau
~ ticks = interval(30000)
~ weather = ticks |> flat_map(|_| from_async(fetch_weather))
```

But `poll` handles errors, retries, and cancellation of in-flight requests
(like `switchMap` — new tick cancels pending fetch).

## `complete()` and `error()` — Stream Termination

Streams aren't infinite. They end.

```tau
~ countdown = subject(10)

-- Complete a subject (no more values)
complete(countdown)

-- Error a subject (propagate failure)
error(countdown, "timeout exceeded")

-- Detect completion in pipelines
~ safe = weather
    |> catch_error(|e| {
        @ print("Weather fetch failed: " + show(e))
        stream_of(FallbackWeather)
    })
    |> take_until(app_shutdown)
```

### Completion Propagation

When a source completes, derived streams complete too:

```tau
~ nums = from_list([1, 2, 3, 4, 5])     -- completes after 5
~ doubled = nums |> map(|x| x * 2)    -- completes when nums completes
~ sum = nums |> scan(0, |a, x| a + x) -- emits [1, 3, 6, 10, 15], then completes

~ sum | x -> {
    @ print(show(x))
}
@ print("Stream complete!")  -- runs after sum completes
```

## The Weather App: Everything Together

A showcase that uses every major Futuruna feature — ADTs, default logic, streams,
subjects, lifecycle, pipe operators, pattern matching, error handling.

```tau
-- weather.runa: Futuruna showcase — reactive weather advisor
-- Features: ADTs, default logic, streams, subjects, lifecycle, pipes

-- ============================================================================
-- Types: the shape of weather
-- ============================================================================

# Condition = Sunny | Cloudy | Rainy | Stormy | Snowy | Windy

# Weather(
    temp: Float,
    condition: Condition,
    wind_kph: Float,
    humidity: Float,
    uv: Int
)

# Severity = Mild | Moderate | Severe | Extreme

# Advisory(
    activity: String,
    warning: String,
    severity: Severity,
    gear: List(String)
)

# FetchError = Timeout | NetworkDown | BadResponse(code: Int)

-- ============================================================================
-- Default logic: what to do today (Catala-style layered rules)
-- ============================================================================

-- Base rule: default advice for any weather
| advise(w: Weather) -> Advisory(
    activity: "Go outside and enjoy the day",
    warning: "",
    severity: Mild,
    gear: []
)

-- Condition-specific overrides
| advise(w) -> Advisory(
    activity: "Perfect day for a bike ride or outdoor café",
    warning: "",
    severity: Mild,
    gear: ["sunglasses"]
) under w.condition == Sunny and w.temp > 15.0 and w.temp < 35.0

| advise(w) -> Advisory(
    activity: "Good day for a museum or indoor market",
    warning: "Expect wet streets",
    severity: Moderate,
    gear: ["umbrella", "waterproof jacket"]
) under w.condition == Rainy

| advise(w) -> Advisory(
    activity: "Stay home, read a book, make soup",
    warning: "Dangerous conditions outside",
    severity: Severe,
    gear: ["stay indoors"]
) under w.condition == Stormy

| advise(w) -> Advisory(
    activity: "Build a snowman or go skiing",
    warning: "Roads may be icy",
    severity: Moderate,
    gear: ["warm coat", "boots", "gloves"]
) under w.condition == Snowy and w.temp > -10.0

-- Temperature extremes override everything
| exception heatwave
  advise(w) -> Advisory(
    activity: "Stay in shade, drink water, avoid exertion",
    warning: "HEAT WARNING: dangerously hot",
    severity: Extreme,
    gear: ["water bottle", "hat", "sunscreen SPF50"]
) under w.temp > 35.0

| exception coldsnap
  advise(w) -> Advisory(
    activity: "Do not go outside unless necessary",
    warning: "COLD WARNING: risk of hypothermia",
    severity: Extreme,
    gear: ["thermal layers", "face covering"]
) under w.temp < -15.0

-- Wind compounds severity
| exception gale
  advise(w) -> Advisory(
    activity: "Secure outdoor furniture, stay indoors",
    warning: "GALE WARNING: " + show(w.wind_kph) + " km/h winds",
    severity: Extreme,
    gear: ["stay indoors"]
) under w.wind_kph > 90.0

-- UV override on otherwise nice days
| exception uv_danger
  advise(w) -> Advisory(
    activity: advise(w).activity,  -- keep the base activity
    warning: "UV index " + show(w.uv) + " — limit sun exposure",
    severity: Severe,
    gear: push(advise(w).gear, "sunscreen SPF50")
) under w.uv > 8

-- ============================================================================
-- Mock weather data (simulating API responses over time)
-- ============================================================================

> mock_weather_feed() -> List(Weather) {
    [
        Weather(temp: 22.0, condition: Sunny,  wind_kph: 12.0, humidity: 45.0, uv: 6),
        Weather(temp: 18.0, condition: Cloudy, wind_kph: 20.0, humidity: 60.0, uv: 3),
        Weather(temp: 14.0, condition: Rainy,  wind_kph: 35.0, humidity: 85.0, uv: 1),
        Weather(temp: 38.0, condition: Sunny,  wind_kph:  8.0, humidity: 30.0, uv: 11),
        Weather(temp: -18.0, condition: Snowy, wind_kph: 45.0, humidity: 70.0, uv: 1),
        Weather(temp: 25.0, condition: Windy,  wind_kph: 95.0, humidity: 50.0, uv: 5),
        Weather(temp:  8.0, condition: Stormy, wind_kph: 80.0, humidity: 95.0, uv: 0),
        Weather(temp: 20.0, condition: Sunny,  wind_kph: 10.0, humidity: 40.0, uv: 5)
    ]
}

-- ============================================================================
-- Reactive pipeline: streams + subjects + lifecycle
-- ============================================================================

| scope WeatherApp {

    -- Subject: user can change location (pushed from UI)
    ~ location = subject("Copenhagen")

    -- Stream: weather readings arriving over time
    -- In production: ~ raw = poll(fetch_weather, 30000, immediate: true)
    ~ raw = from_list(mock_weather_feed())

    -- Pipe: transform raw readings into advisories
    ~ advisories = raw
        |> map(|w| (w, advise(w)))
        |> filter(|pair| pair.1.severity != Mild)

    -- Pipe: extract just the severe/extreme ones
    ~ urgent = advisories
        |> filter(|pair| pair.1.severity == Severe or pair.1.severity == Extreme)

    -- Scan: track how many alerts we've issued (running state)
    ~ alert_count = urgent
        |> scan(0, |count, _| count + 1)

    -- Scan: rolling average temperature
    ~ avg_temp = raw
        |> scan((0.0, 0), |acc, w| (acc.0 + w.temp, acc.1 + 1))
        |> map(|acc| acc.0 / acc.1)

    -- Subject: manual override (operator can push an emergency)
    ~ emergency = subject()

    -- Merge: combine computed alerts with manual overrides
    ~ all_alerts = merge(
        urgent |> map(|pair| pair.1.warning),
        emergency
    )

    -- ========================================================================
    -- Subscriptions (all torn down when WeatherApp scope exits)
    -- ========================================================================

    -- Main display: every reading gets advice
    ~ advisories | pair -> {
        = w = pair.0
        = a = pair.1
        @ print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
        @ print("  " + show(w.condition) + "  " + show(w.temp) + "°C")
        @ print("  → " + a.activity)
        if a.warning != "" {
            @ print("  ⚠ " + a.warning)
        }
        @ print("  Gear: " + show(a.gear))
    }

    -- Alert ticker
    ~ all_alerts | msg -> {
        @ print("[ALERT] " + msg)
    }

    -- Stats (fires each time a new reading comes in)
    ~ avg_temp | avg -> {
        @ print("  📊 Running avg: " + show(avg) + "°C")
    }

    ~ alert_count | n -> {
        @ print("  📊 Alerts issued: " + show(n))
    }
}
-- Scope exits → all channels closed, all tasks cancelled, zero leaks

-- ============================================================================
-- What just happened (no emoji in the code above — these are in prose)
-- ============================================================================

@ print("")
@ print("=== What this demonstrated ===")
@ print("  # ADTs          — Weather, Condition, Severity, Advisory")
@ print("  | default logic — layered rules with 'under' + 'exception'")
@ print("  ~ streams       — reactive pipelines with |> composition")
@ print("  ~ subjects      — push-based streams (location, emergency)")
@ print("  | scope         — automatic lifecycle teardown")
@ print("  > functions     — pure transforms in the pipeline")
@ print("  match/if        — pattern matching on conditions")
@ print("  scan            — stateful accumulation over time")
@ print("  merge           — combining independent event sources")
@ print("  for-in-stream   — subscription as iteration")
```

## The Production Version (What Changes)

The mock becomes real with three line changes:

```tau
-- Mock → Real: swap the source
-- ~ raw = from_list(mock_weather_feed())
~ raw = poll(fetch_weather, 30000, immediate: true)

-- The async fetch function
> fetch_weather() -> Result(Weather, FetchError) {
    = resp <- http_get("https://api.weather.com/v1/" + location.latest())
    = json <- parse_json(resp.body)
    Ok(Weather(
        temp:      json["temp"].as_float(),
        condition: parse_condition(json["condition"].as_string()),
        wind_kph:  json["wind_kph"].as_float(),
        humidity:  json["humidity"].as_float(),
        uv:        json["uv"].as_int()
    ))
}
```

Everything else — the rules, the pipelines, the lifecycle — stays identical.
The architecture doesn't change when you swap mock for real. That's the point.

## How Subjects Differ from RxJS

| Concept | RxJS | Futuruna | Why better |
|---------|------|-----|------------|
| Create subject | `new Subject<T>()` | `~ s = subject()` | `~` rune makes it visually stream |
| Push value | `s.next(val)` | `s <- val` | Same operator as actors — one mental model |
| Get current | `s.getValue()` | `s.latest()` | Only on `subject(initial)` — type-safe |
| Complete | `s.complete()` | `complete(s)` | Function, not method — composable |
| Error | `s.error(e)` | `error(s, e)` | Same |
| Subscribe | `s.subscribe(fn)` | `~ s \| x -> { }` | Dedicated syntax (`~ + |`) — structurally sound |
| Unsubscribe | `sub.unsubscribe()` | Scope exit (automatic) | Rust's Drop = no memory leaks |
| takeUntil | `s.pipe(takeUntil(d$))` | `s \|> take_until(d)` | Same, but scope makes it rarely needed |
| BehaviorSubject | `new BehaviorSubject(0)` | `subject(0)` | Initial value = behavior, no initial = plain |
| ReplaySubject | `new ReplaySubject(5)` | `subject([], replay: 5)` | Named parameter, obvious |

## How Lifecycle Differs from RxJS/Angular/React

| Framework | Teardown mechanism | Problem |
|-----------|-------------------|---------|
| **RxJS** | `subscription.unsubscribe()` | Manual. Forget one → memory leak |
| **Angular** | `takeUntilDestroyed()`, `DestroyRef` | Bolted onto DI system, easy to forget |
| **React** | `useEffect` cleanup return | Closure footgun, stale closures |
| **Svelte** | `onDestroy()` | Manual callback |
| **Futuruna** | `\| scope { }` block exit | Automatic. Compiler enforces. Zero leaks possible |

The key insight: **Rust already solved this problem** with `Drop`. A `| scope`
is a struct that holds `Vec<JoinHandle>`. When it drops, handles abort. The
compiler guarantees it. No discipline required.

### Nested Scope = Component Tree

```
| scope App
├── ~ route = subject("/")
├── | scope Sidebar
│   ├── ~ menu_items = route |> map(route_to_menu)
│   └── ~ menu_items | item -> { render(item) }
├── | scope Content
│   ├── | scope WeatherPanel        ← navigating away tears this down
│   │   ├── ~ weather = poll(fetch, 30s)
│   │   ├── ~ alerts = weather |> filter(severe?)
│   │   └── ~ alerts | a -> { notify(a) }
│   └── | scope SettingsPanel       ← navigating here creates fresh scope
│       ├── ~ prefs = load_prefs()
│       └── ~ prefs | p -> { render_form(p) }
└── | scope Footer
    └── ~ status = poll(health_check, 60s)
```

Navigate from Weather to Settings:
1. `WeatherPanel` scope drops → poll cancelled, filter cancelled, subscription cancelled
2. `SettingsPanel` scope created → fresh streams, fresh subscriptions
3. Zero manual cleanup. Zero leaked intervals. Zero stale closures.

## Transpilation Strategy

### Subject → Rust

```tau
~ count = subject(0)
count <- 5
~ count | x -> { @ print(show(x)) }
```

Becomes:

```rust
// ~ count = subject(0) → broadcast channel with initial value
let (count_tx, _) = tokio::sync::broadcast::channel::<i64>(64);
let count_latest = Arc::new(Mutex::new(0i64));

// count <- 5 → send + update latest
{
    let val = 5i64;
    *count_latest.lock().unwrap() = val;
    let _ = count_tx.send(val);
}

// ~ count | x -> { ... } → spawned subscriber task
let _sub_handle = tokio::spawn({
    let mut rx = count_tx.subscribe();
    async move {
        while let Ok(x) = rx.recv().await {
            println!("{}", x);
        }
    }
});
```

### Scope → Rust

```tau
| scope Panel {
    ~ data = poll(fetch, 30000)
    ~ data | d -> { render(d) }
}
```

Becomes:

```rust
{
    let mut _scope_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // ~ data = poll(fetch, 30000)
    let (data_tx, _) = tokio::sync::broadcast::channel(64);
    _scope_handles.push(tokio::spawn({
        let tx = data_tx.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_millis(30000));
            loop {
                interval.tick().await;
                match fetch().await {
                    Ok(val) => { let _ = tx.send(val); }
                    Err(_) => {} // TODO: error handling
                }
            }
        }
    }));

    // ~ data | d -> { render(d) }
    _scope_handles.push(tokio::spawn({
        let mut rx = data_tx.subscribe();
        async move {
            while let Ok(d) = rx.recv().await {
                render(d);
            }
        }
    }));

    // Scope cleanup: abort all tasks when block exits
    // (In practice, wrapped in a struct with Drop impl)
    struct _ScopeGuard(Vec<tokio::task::JoinHandle<()>>);
    impl Drop for _ScopeGuard {
        fn drop(&mut self) {
            for h in &self.0 { h.abort(); }
        }
    }
    let _guard = _ScopeGuard(_scope_handles);

    // ... scope body runs ...

}  // _guard drops here → all tasks aborted
```

## What This Means for Futuruna Milestones

M12 as currently spec'd covers `~` and `|>` (the cold/derived side). This
design adds the hot/imperative side:

| Feature | M12 (current) | M12+ (this doc) |
|---------|--------------|-----------------|
| `~ x = derived` | Yes | Yes |
| `\|>` pipe | Yes | Yes |
| `map`, `filter`, etc | Yes | Yes |
| `for x in stream` | Yes | Yes |
| `subject()` | No | **Yes** |
| `s <- val` (push) | No (actor only) | **Yes** (unified with actors) |
| `.latest()` | No | **Yes** |
| `complete(s)` / `error(s, e)` | No | **Yes** |
| `\| scope { }` teardown | No | **Yes** |
| `poll(fn, ms)` | No | **Yes** |
| `take_until(signal)` | No | **Yes** |
| Nested scope lifecycle | No | **Yes** |

The actor unification is the insight: **subjects ARE actors, actors ARE subjects.**
`<-` works on both. Scope teardown works on both. One mechanism, two views.
