# Reactive Futuruna: Subjects, Lifecycle, and the Weather App

> Status: design overview. Code blocks marked `tau` sketch the application
> shape and use operators (`poll`, `debounce`, named arguments) that are not part
> of the language. The authoritative semantics of subjects, live streams,
> subscriptions and actors are in [Reactive Streams](reference/streams.md); the
> lifetime contract is [Stream Lifetimes](stream-lifetimes.md): live
> subscriptions are top-level script work or owned by an explicit named
> `| scope Name { ... }`. Returned subscription handles and `subscribe()`-style
> disposables are not part of the language.

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
~ weather = subject()              -- empty, keeps every value
~ count = subject(0)               -- starts with 0, keeps every value
~ history = subject(0, 5)          -- starts with 0, keeps the last 5 values

-- Push with <-
weather <- Sunny(temp: 22.0)
count <- count.latest + 1

-- Subscribe with the current ~ + | terminal form
~ weather | w -> {
    @ print("Weather changed: " + show(w))
}

-- Derive from subjects (subjects ARE streams)
~ hot_days = weather |> filter(|w| w.temp > 30.0)
```

**Why `<-` and not `.next()`:** Futuruna already has `<-` for actor sends. A subject
is an actor with no logic — it forwards what it receives. Same operator, same
mental model: `<-` returns after the value has been handled by every
subscriber (subjects) or by the actor's handler (actors).

### Subject Variants

| Futuruna | Keeps | A late subscriber first receives |
|-----|------|----------|
| `subject()` | every value | every value sent so far |
| `subject(val)` | every value, starting with `val` | `val` and every value sent since |
| `subject(val, n)` | the last `n` values | the last `n` values |

### `.latest` — Synchronous Access

```tau
~ temp = subject(20.0)
temp <- 25.0

-- .latest gives the most recent value
= current = temp.latest    -- 25.0

-- Use in expressions directly
if temp.latest > 30.0 {
    @ print("It's hot!")
}
```

This is the bridge between `~` (time) and `=` (moment). `.latest` collapses
a stream to its current point.

## Lifecycle: Scoped Streams

RxJS lifecycle is manual: `subscription.unsubscribe()`. Angular added
`takeUntilDestroyed()`. React has `useEffect` cleanup. All of these are
bolted-on afterthoughts.

Futuruna has named scope blocks. **Live streams die when their owning named
scope dies.**

### `| scope` for View Lifecycle

```tau
-- A view is a named scope. When the scope exits, all streams inside are torn down.
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

**Ownership:** every subscription and derived stream created in a scope is
registered under the scope's name; `@ teardown("WeatherDashboard")` stops all of
them at once. Ordinary functions may snapshot streams or return stream
expressions, but they may not start live subscriptions unless a named scope owns
them.

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

### Explicit Teardown

Named scopes are the explicit lifetime owner. Manual subscription handles are
a deferred design, not part of the language:

```tau
| scope WeatherPanel {
    ~ weather | w -> { render(w) }
}

-- Later:
@ teardown("WeatherPanel")
```

A `subscribe()` that returns a disposable handle is intentionally not part of
the language: it would let ordinary helpers hide running subscriptions behind
returned values. Return streams from helpers and subscribe inside the caller's
named scope.

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

-- Complete a subject (no more values; later sends are errors)
complete(countdown)

-- Or end it with an error instead (subscribers' Err arms receive the message)
-- error(countdown, "timeout exceeded")

-- Detect completion in pipelines
~ safe = weather
    |> catch_error(|e| {
        @ print("Weather fetch failed: " + show(e))
        stream_of(FallbackWeather)
    })
    |> take_until(app_shutdown)
```

### Completion Propagation

When a source completes, derived streams complete too, and every subscription's
`Complete` arm runs once:

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
@ print("  ~ + |           — subscription inside named scopes")
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
| Get current | `s.getValue()` | `s.latest` | Most recent value; an error when there is none |
| Complete | `s.complete()` | `complete(s)` | Function, not method — composable |
| Error | `s.error(e)` | `error(s, e)` | Same |
| Subscribe | `s.subscribe(fn)` | `~ s \| x -> { }` | Dedicated syntax (`~ + |`) — structurally sound |
| Unsubscribe | `sub.unsubscribe()` | `@ teardown("Name")` of the owning scope | One name stops every subscription it owns |
| Delivery | synchronous, re-entrant | synchronous, in order | A value sent during delivery waits for the current one |
| BehaviorSubject | `new BehaviorSubject(0)` | `subject(0)` | Starts with the initial value |
| ReplaySubject | `new ReplaySubject(5)` | `subject(0, 5)` | Keeps the last 5 values |

## How Lifecycle Differs from RxJS/Angular/React

| Framework | Teardown mechanism | Problem |
|-----------|-------------------|---------|
| **RxJS** | `subscription.unsubscribe()` | Manual. Forget one → memory leak |
| **Angular** | `takeUntilDestroyed()`, `DestroyRef` | Bolted onto DI system, easy to forget |
| **React** | `useEffect` cleanup return | Closure footgun, stale closures |
| **Svelte** | `onDestroy()` | Manual callback |
| **Futuruna** | `@ teardown("Name")` of the owning named `\| scope Name { }` | One call stops everything the scope owns. The compiler rejects unowned subscriptions in functions |

The key insight: ownership is a name. Every live subscription is owned by the
script or by a named scope, and the compiler rejects subscriptions started in
ordinary functions without one.

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
1. `@ teardown("WeatherPanel")` → filter and subscription stop
2. `SettingsPanel` scope runs → fresh streams, fresh subscriptions
3. No stale subscriptions remain.

## Execution Model

Subjects, live streams and actors are shared runtime cells in both execution
modes (`src/live_streams.rs` in the interpreter, `src/live_runtime.rs` in
compiled programs). Delivery is synchronous: `count <- 5` returns after every
subscription and derived stream has handled 5, so output order is the same in
`runa file.runa` and `runa run`. A scope registers what it owns under its name;
`@ teardown("Panel")` unsubscribes it.

## Feature Summary

| Feature | Form |
|---------|------|
| Derived stream | `~ x = source \|> map(f)` |
| Subject | `~ s = subject()`, `subject(v)`, `subject(v, n)` |
| Push | `s <- value` |
| Current value | `s.latest`, `s.count` |
| Termination | `complete(s)`, `error(s, message)` |
| Subscription | `~ s \| x -> { ... } \| Err(e) -> { ... } \| Complete -> { ... }` |
| Lifetime | named `\| scope Name { }` and `@ teardown("Name")` |
| Actor | `> actor name(state: T) { \| Msg -> ... }`, `spawn`, `<-`, `ask` |

The actor unification is the insight: `<-` works on subjects and actors with one
meaning — the value is handled before the send returns.
