# RunaQL — Query Resolution via Runes

A research direction for typed query resolution in Futuruna,
where ADT types define the query schema, pattern matching performs resolution,
and the seven runes map directly to API architecture layers.

## The Problem

Every API framework separates concerns into multiple artifacts:

```
REST:    domain -> controllers -> routes -> JSON -> OpenAPI spec     (5 artifacts, 3 languages)
GraphQL: domain -> schema.graphql -> resolvers -> codegen -> types   (5 artifacts, 2 languages)
Prolog:  facts -> rules -> queries -> results                        (1 language, no types)
```

REST has types but no schema language. GraphQL has schema but needs codegen.
Prolog has unification but no types. None of them are one thing.

## The Thesis

Futuruna's runes already ARE a query architecture:

| Architecture Layer | GraphQL Artifact   | Futuruna Rune | What It Means        |
|--------------------|--------------------|---------------|----------------------|
| Schema             | `schema.graphql`   | `#` types     | What you can ask     |
| Resolvers          | `resolvers.ts`     | `>` functions  | How answers are computed |
| Validation         | middleware         | `|` invariants | What must hold       |
| Configuration      | env/config         | `=` bindings   | Ground truth         |
| Subscriptions      | `Subscription` type | `~` streams   | What flows live      |
| Transport          | `server.ts`        | `@` effects    | Where proofs stop    |
| Verification       | tests (maybe)      | `?` proofs     | Prove it             |

One file. Seven runes. Seven layers. No codegen. No mapping. No glue.

## Core Idea: Types as Schema

A RunaQL schema is a Futuruna ADT:

```runa
# Query = GetUser(id: Int) | SearchUsers(name: String, role: Role) | ListRoles
```

This type IS the schema. Every variant is a query. Every field is a parameter.
The compiler enforces exhaustive handling — add a variant, you MUST add a resolver arm.
Forget an arm and it doesn't compile. GraphQL can't do this.

## Core Idea: Match as Resolution

Resolution is pattern matching:

```runa
> resolve(q: Query) -> String {
    match q {
        | GetUser(id) -> lookup_user(id)
        | SearchUsers(name, role) -> search(name, role)
        | ListRoles -> all_roles()
    }
}
```

No resolver registry. No middleware chain. No dependency injection.
The match IS the resolver. The compiler guarantees completeness.
This is the Prolog idea (query -> resolution) with the Haskell guarantee (exhaustive match).

## Core Idea: Invariants as Validation

```runa
| user_exists: lookup_user(1) -> is_some(lookup_user(1)) == True
? user_exists   -- verified before the server starts
```

Validation isn't middleware bolted onto the side. It's structural.
The `|` rune declares what must hold. The `?` rune proves it.
The server won't start if the invariants fail.

## The Three Levels

### Level 1: What Works Today (see examples/tax-server.runa)

- Define domain types with `#` (Status, brackets, deductions)
- Parse JSON -> typed ADT via string-matching functions
- Resolve via pure functions in `>` (tax rules, bracket logic)
- Validate with `|` invariants (fairness guarantees), verify with `?` before serving
- Track live analytics via `~` reactive streams (cumulative revenue)
- Serve via `@ http_serve`

This is already more coherent than GraphQL. One language, not schema + resolvers + types + codegen.
The gap: parsing JSON into ADT values requires hand-written `parse_X` functions per type.

### Level 2: Schema Exchange (needs compiler work)

Auto-derive schema from types:

```runa
-- Imagine: @ comptime generates this from # Query definition
> schema() -> String {
    type_schema(Query)  -- returns JSON schema of the ADT
}
```

What this enables:
- Clients introspect the server's type system at runtime
- Schema diffing for API versioning ("Query gained variant X, removed Y")
- Client-side validation before sending (does this query match the schema?)
- Auto-generation of parse functions from type definitions

What this needs:
- Runtime reflection on ADT types (list variants, list fields)
- `@ comptime` access to type metadata
- A standard schema serialization format (JSON Schema? Custom?)

### Level 3: Prolog-Style Resolution (deep research)

Queries as goals with unification:

```runa
-- Hypothetical syntax
? find X such_that magthaver(X) == Enkelt(_)
-- Returns: [Udoevende, Doemmende]

? find X, Y such_that magthaver(X) == Forening(Y, _)
-- Returns: [(Lovgivende, Kongen)]
```

What this enables:
- Compositional queries: "find all X where P(X) and Q(X)"
- Backwards reasoning: "what input produces this output?"
- Negation-as-failure for closed-world assumptions
- The full Prolog experience, but typed

What this needs:
- Unification engine (structural matching with logic variables)
- Goal stack with backtracking
- Integration with the existing `?` rune (verification becomes query)
- A way to enumerate ADT variant space

## Comparison

| Feature          | REST        | GraphQL          | Prolog       | RunaQL (today) | RunaQL (dream)     |
|------------------|-------------|------------------|--------------|----------------|--------------------|
| Schema           | OpenAPI     | SDL              | none         | `# ADT`        | `# ADT` + auto     |
| Type safety      | external    | codegen          | none         | native          | native             |
| Resolution       | controllers | resolver fns     | unification  | pattern match   | typed unification  |
| Validation       | middleware  | directives       | constraints  | `\|` invariants | `\|` invariants    |
| Subscriptions    | WebSocket   | Subscription     | —            | `~` streams     | `~` streams        |
| Verification     | tests       | tests            | proofs       | `?` rune        | `?` as query       |
| Exhaustiveness   | no          | no               | no           | yes (compiler)  | yes (compiler)     |
| Introspection    | /swagger    | `__schema`       | `listing/0`  | hand-written    | auto-derived       |

## Key Research Questions

1. **Can `@ comptime` derive schema from `#` types?**
   Today we hand-write `/schema`. Could the compiler generate it?
   This is probably the lowest-hanging fruit with highest impact.

2. **What does field selection look like in an ADT world?**
   GraphQL returns partial objects. ADTs are all-or-nothing.
   Possible: projection types, structural subtyping, or just "return JSON and let the client pick fields."
   The pragmatic answer may be: don't. Return the full typed result. Bandwidth is cheap. Type safety isn't.

3. **Can `|` invariants become runtime request validation?**
   Today they're compile-time/boot-time. Could they guard individual requests?
   E.g., `| valid_age: alder -> alder >= 0 && alder <= 200`
   Applied automatically when a query contains an `alder` field.

4. **Can `~` streams become live query subscriptions?**
   A subject per query type. Client subscribes, gets pushed results when data changes.
   The topology is natural — each subscription IS a stream.
   Needs: WebSocket or SSE transport in `@ http_serve`.

5. **What would typed unification look like?**
   Prolog unification is untyped. Futuruna types constrain the search space.
   `? find X : Statsmagt such_that er_delt_magt(X) == True`
   The type annotation bounds the search. The `?` rune already means "prove it."
   This is the deepest question. A typed logic programming layer.

## The Grundlov as Test Case

The Danish Constitution is the ideal test domain:

- **Fixed ontology** — powers, institutions, persons, modalities (stable types)
- **Rich invariants** — constitutional guarantees that MUST hold (testable)
- **Known paradoxes** — the Muslim King, section 77 vs 88 (tests resolution depth)
- **Compositional queries** — "is this person eligible to rule?" combines age + religion + oath
- **Small enough to encode fully** — 89 paragraphs, not thousands of tables
- **Meaningful enough to matter** — this is real law, not a toy

## Async Transport: tiny_http → axum

The current `@ http_serve` uses `tiny_http`, which is synchronous and single-threaded.
This is a bottleneck for any real server. The plan is to replace it with `axum`,
which is already tokio-native — matching the async runtime Futuruna already generates.

### Why axum fits

- Futuruna already generates `#[tokio::main]` with multi-threaded runtime
- Subjects are `tokio::sync::broadcast` channels — already async
- Actors use `tokio::spawn` + oneshot — already async
- axum handlers are async fns on tokio — zero impedance mismatch

### What this unlocks: requests as streams

With an async transport, HTTP requests can flow into `~` subjects:

```runa
~ requests = subject()

-- Stream operators compose request processing
~ results = requests
    |> filter(|r| r.path == "/tax")
    |> map(|r| handle_tax(r.body))

-- The topology IS the execution plan
-- map spawns tokio tasks — automatic parallelism
```

The missing piece today is the **reply mechanism**: getting a stream output
back to the waiting HTTP client. Two approaches:

**Approach A: Request-reply subject.** Each request carries a oneshot reply channel.
The stream pipeline computes the result and sends it back. This is what actors
already do with `ask` — generalize it to streams.

```runa
-- Hypothetical syntax
@ http_serve_async(8080, |req| {
    requests <- req              -- push into stream
    = response = await(req)      -- wait for stream pipeline to reply
    http_respond(200, "application/json", response)
})
```

**Approach B: Actor pool.** Multiple actors consume from a shared channel.
Each actor handles one request at a time, but N actors handle N concurrent requests.

```runa
> actor TaxWorker {
    | ComputeTax(body: String) -> String { handle_tax(body) }
}

-- Spawn a pool of workers
= pool = spawn_pool(TaxWorker, 8)

@ http_serve_async(8080, |path, method, body| {
    = result = ask(pool, ComputeTax(body))
    http_respond(200, "application/json", result)
})
```

### What stream-based HTTP enables beyond parallelism

- **Rate limiting**: `take(1000)` per window — back-pressure is a stream op
- **Request batching**: `window(10) |> map(batch_resolve)` — batch DB queries
- **Live dashboards**: push request stats to a dashboard subject via `tap`
- **Circuit breaking**: `catch` to fall back when a downstream service fails
- **Metrics for free**: `scan` accumulates request counts, latencies, error rates

These are all standard stream operators applied to HTTP. No new concepts needed.
The topology IS the middleware stack.

### Migration path

1. Add `@ depend "axum"` support to codegen (auto-dependency, like tiny_http today)
2. New builtin `@ http_serve_async` that generates an axum server
3. Handler receives a request struct, returns a response (async fn)
4. Keep `@ http_serve` as the simple synchronous option (tiny_http, good for demos)
5. Stream integration: subject push + reply channel in Level 2


## Design Principle

RunaQL is NOT "add a query language to Futuruna."

RunaQL is "recognize that Futuruna's runes already ARE a query architecture,
and make the pattern explicit, automatable, and interoperable."

The runes don't need to change. The infrastructure around them does.
