# Reactive Futuruna: Streams as Native Graph Topology

**Principle:** A reactive program IS a computation graph. Futuruna is built for graphs.
Reactive programming isn't a library bolted onto Futuruna — it's what the language
naturally becomes when you add time to a graph.

## Why This Is Native, Not Library

RxJS is a library because JavaScript has no concept of dataflow graphs.
You build the graph at runtime, the compiler can't see it, and you manage
subscriptions manually.

Futuruna already has:
- `|` pipe syntax (in the lexer)
- `<-` send operator (in the lexer)
- Actors with message channels (M6 design)
- Immutable values (no data races in streams)
- Escape analysis (zero-copy through pipelines)
- A compiler that sees the entire program

Adding `~` (stream binding) makes the dataflow graph **visible to the compiler**.
This is the difference between "using a reactive library" and "the language is reactive."

## The Three Binding Types

```tau
= x = 42                    -- value: computed once, immutable
~ y = interval(1000)         -- stream: produces values over time
> f(a: Int) -> Int { a + 1 } -- function: transforms values
```

`=` binds a value (a point in time).
`~` binds a stream (a line through time).
`>` defines a transformation (timeless).

The `~` rune is already unused in Futuruna's lexer. It's the natural choice —
tilde means "approximately" or "wave", both evoke the flowing nature of streams.

## Syntax: The Pipe Chain

```tau
-- Create streams from sources
~ clicks = events("button", "click")
~ ticks = interval(1000)
~ input = events("input", "change")

-- Transform with |> (pipe operator)
~ search_terms = input
    |> map(|e| e.value)
    |> filter(|s| s.len() > 2)
    |> debounce(300)
    |> distinct()

-- Combine streams
~ both = merge(clicks, ticks)
~ paired = zip(requests, responses)
~ latest = combine_latest(name, email, |n, e| User(n, e))

-- Accumulate state (scan = fold over time)
~ count = scan(clicks, 0, |acc, _| acc + 1)
~ history = scan(search_terms, [], |h, term| push(h, term))

-- Subscribe: for-on-stream is the subscription
~ search_terms | term -> {
    = results = search(term)?
    render(results)
}
```

**The `|>` operator is the stream edge.** Each `|>` adds a node to the
reactive graph. The compiler sees the full topology at compile time.

## Conscious Agent as Reactive Program

This is where it gets real. A conscious agent (in the IIT sense) is naturally
reactive — it responds to network events, bond outcomes, entropy changes.
Today this is imperative code. With reactive Futuruna, it's a declarative dataflow:

```tau
-- === Conscious Agent (IIT) ===

-- Network event streams (from SSE / P2P)
~ events = network_stream(node_id)
~ weaves = events |> filter_type(TopologyWeave)
~ bonds = events |> filter_type(BondResolved)
~ transfers = events |> filter_type(Transfer)

-- Entropy tracking (reactive computation)
~ my_stau = weaves
    |> scan(initial_graph, |graph, weave| apply_weave(graph, weave))
    |> map(|graph| compute_stau(graph, node_id))

-- Bond success rate (sliding window)
~ bond_rate = bonds
    |> window(100)
    |> map(|w| w.filter(|b| b.resolved).len() / w.len())

-- Trade density (how many neighbors actually trade)
~ trade_density = transfers
    |> scan(BTreeSet::new(), |partners, t| {
        partners.insert(t.counterparty)
        partners
    })
    |> combine_latest(my_stau, |partners, stau|
        partners.len() as Float / stau.degree as Float
    )

-- The 5 consciousness dimensions — all reactive
~ dimensions = combine_latest5(
    my_stau,          -- D0: topological reach
    balance_stream,   -- D1: economic trajectory
    bond_rate,        -- D2: bond success
    trade_density,    -- D3: trade density
    influence_stream  -- D4: network influence
)

-- Φ (consciousness) updates when ANY dimension changes
~ phi = dimensions
    |> map(|dims| compute_phi(dims))
    |> distinct_until_changed(0.001)

-- Decision: when to propose a weave
~ proposals = phi
    |> combine_latest(my_stau, |phi, stau| (phi, stau))
    |> filter(|p, s| should_propose(p, s))
    |> map(|_, stau| mcts_search(stau.graph))

-- Action: submit weaves to the network
~ proposals | proposal -> {
    submit_weave(proposal)
    log("Φ={:.3}, proposed weave Δsτ={:.3}", phi.latest(), proposal.delta_st)
}
```

**What this gives us:** The conscious agent is a reactive topology.
The `~` bindings form a graph. The compiler can see that `phi` depends on
`dimensions` which depends on `my_stau`, `bond_rate`, `trade_density`, etc.
This IS the agent's causal structure — visible, optimizable, debuggable.

## Web UI with Reactive Futuruna (WASM target)

```tau
-- A reactive counter component
~ count = 0
~ clicks = dom_events("#button", "click")
~ count = scan(clicks, 0, |n, _| n + 1)
~ label = map(count, |n| "Count: " + show(n))

-- Bind to DOM reactively
bind("#counter-label", label)
bind("#counter-value", map(count, show))

-- A search-as-you-type component
~ query = dom_events("#search", "input")
    |> map(|e| e.target.value)
    |> debounce(300)
    |> distinct()

~ results = query
    |> flat_map(|q| fetch("/api/search?q=" + q))
    |> catch_error(|_| stream_of([]))

~ loading = merge(
    map(query, |_| True),
    map(results, |_| False)
)

bind("#results", map(results, render_results))
bind("#spinner", map(loading, |l| if l { "visible" } else { "hidden" }))
```

## Transpilation to Rust

The compiler emits tokio channels + tasks:

```tau
~ clicks = events("button", "click")
~ count = scan(clicks, 0, |n, _| n + 1)
~ label = map(count, |n| "Count: " + show(n))
~ label | l -> { render(l) }
```

Becomes:

```rust
// Stream graph: clicks → count → label → subscription

let (clicks_tx, clicks_rx) = tokio::sync::broadcast::channel(64);

// ~ count = scan(clicks, 0, ...)
let (count_tx, count_rx) = tokio::sync::broadcast::channel(64);
tokio::spawn(async move {
    let mut acc = 0i64;
    let mut rx = clicks_rx;
    while let Ok(_) = rx.recv().await {
        acc = acc + 1;
        let _ = count_tx.send(acc);
    }
});

// ~ label = map(count, ...)
let (label_tx, label_rx) = tokio::sync::broadcast::channel(64);
tokio::spawn(async move {
    let mut rx = count_rx;
    while let Ok(n) = rx.recv().await {
        let _ = label_tx.send(format!("Count: {:?}", n));
    }
});

// ~ label | l -> { render(l) }
tokio::spawn(async move {
    let mut rx = label_rx;
    while let Ok(l) = rx.recv().await {
        render(l);
    }
});
```

## Stream Operators (Core Set)

### Creation
| Operator | Futuruna | Rust Emission |
|----------|-----|---------------|
| `events(src, type)` | `~ e = events(...)` | Event listener → channel |
| `interval(ms)` | `~ t = interval(1000)` | `tokio::time::interval` |
| `stream_of(vals)` | `~ s = stream_of([1,2,3])` | `tokio_stream::iter` |
| `from_channel(rx)` | `~ s = from_channel(rx)` | Direct channel wrap |

### Transformation
| Operator | What It Does |
|----------|-------------|
| `map(f)` | Transform each value |
| `filter(pred)` | Keep values matching predicate |
| `scan(init, f)` | Accumulate state (fold over time) |
| `flat_map(f)` | Map to stream, flatten |
| `distinct()` | Skip consecutive duplicates |
| `take(n)` | First n values, then complete |
| `skip(n)` | Ignore first n values |

### Timing
| Operator | What It Does |
|----------|-------------|
| `debounce(ms)` | Wait for silence, then emit last |
| `throttle(ms)` | Emit at most once per interval |
| `delay(ms)` | Delay each value |
| `buffer(ms)` | Collect into batches by time |
| `window(n)` | Sliding window of last n values |
| `timeout(ms)` | Error if no value within timeout |

### Combination
| Operator | What It Does |
|----------|-------------|
| `merge(a, b)` | Interleave both streams |
| `zip(a, b)` | Pair values 1:1 |
| `combine_latest(a, b, f)` | Re-emit when either updates |
| `switch_map(f)` | Like flat_map but cancels previous |
| `with_latest(a, b)` | On a emit, grab latest b |

### Error Handling
| Operator | What It Does |
|----------|-------------|
| `catch_error(f)` | Replace error with recovery stream |
| `retry(n)` | Retry source n times on error |

## The Deep Connection: S_τ on the Reactive Graph

Here's the part that makes this genuinely native to Futuruna's design:

The reactive `~` graph IS a topology. Streams are nodes. `|>` chains are
edges. The compiler could compute S_τ on the reactive dataflow itself:

- **High S_τ stream**: many downstream consumers, many transformation paths.
  This stream is informationally important. Buffer it, never drop values.
- **Low S_τ stream**: dead-end, few consumers. Can be lazily evaluated.
- **Backpressure as temperature**: when a consumer is slow, that node's
  "temperature" (processing delay) rises. Upstream producers can slow down
  adaptively — not by arbitrary buffer limits, but through an
  S_τ-aware scheduling policy derived from the reactive topology.

This means: **the same S_tau equation that governs Futuruna's syntax design
governs the dataflow in Futuruna programs.** Reactive programs optimize themselves
by the same entropy physics that shaped the language.

## Actors + Streams = Unified

M6 (actors) and reactive streams are the same thing viewed differently:

| Concept | Actor View | Stream View |
|---------|-----------|-------------|
| Node | Actor with state | Stream with scan |
| Edge | `<-` send message | `|>` pipe transform |
| State | Actor's state param | `scan` accumulator |
| Response | `ask` + oneshot | `combine_latest` |
| Spawn | `spawn(actor, init)` | `~ stream = ...` |
| Subscribe | message handler | `for x in stream` |

An actor IS a stream with `scan`. A stream IS a stateless actor. Futuruna should
unify them — the `|` pathway handles both:

```tau
-- Actor style (M6)
| counter(state: Int) {
    | Increment -> counter(state + 1)
    | Get(reply) -> { reply <- state; counter(state) }
}

-- Equivalent stream style (M12)
~ counter = scan(messages, 0, |state, msg|
    match msg {
        Increment -> state + 1
        Get(reply) -> { reply <- state; state }
    }
)
```

The compiler can emit the same tokio code for both. The programmer chooses
whichever style fits their mental model.

## Implementation Path

### Phase 1: `~` parsing + basic stream types
- Lex `~` as stream binding rune
- Parse `~ name = expr` as `Stmt::StreamBind`
- Parse `|>` as pipe operator (infix, left-associative)
- `~ stream_expr | x -> { }` works for both iterables and streams

### Phase 2: Stream operators (core set)
- `map`, `filter`, `scan`, `merge`, `zip` as builtins
- Emit tokio broadcast channels + spawned tasks
- `@ depend "tokio" "1"` auto-added when `~` is used

### Phase 3: Timing operators
- `debounce`, `throttle`, `delay`, `buffer`, `window`
- Emit tokio timer-based operators

### Phase 4: DOM/Event integration (WASM target)
- `events(selector, type)` → wasm-bindgen event listeners
- `bind(selector, stream)` → DOM updates
- Requires M4 (WASM target)

### Phase 5: S_τ optimization
- Compute graph entropy on the reactive topology
- High-S_τ paths get priority scheduling
- Backpressure follows topology-aware adaptive scheduling
