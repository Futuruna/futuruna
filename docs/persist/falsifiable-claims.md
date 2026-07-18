# Persist Design: Falsifiable Claims & Pressure Tests

Every claim below is stated as a testable proposition. If any fails, the design must change before implementation begins. Claims are ordered from foundational (must hold) to aspirational (nice-to-have).

---

## Claim 1: "| scope is sufficient as a transaction boundary"

**The proposition:** No new syntax is needed for transactions. The existing `| scope Name { ... }` construct, when it contains `assert` or `retract` on persisted types, becomes a transaction automatically.

### Pressure tests

**F1.1 — The naked loop problem.**
```runa
for item in items {
    assert OrderItem(order_id, item.product_id, item.quantity)
}
```
No scope. Crash at item 5 of 10. Five orphan OrderItems committed, five lost. The user didn't know they needed a scope. **Is silent auto-commit on bare asserts a foot-gun?**

Verdict: This matches SQLite's default (every statement is its own transaction). The alternative — requiring all asserts inside scopes — is too restrictive for simple scripts. But the compiler SHOULD emit a warning: `warn: multiple asserts without enclosing scope — each auto-commits independently`. This is a lint, not an error.

**F1.2 — Scope without persist ops.**
```runa
| scope dashboard {
    ~ readings = subject()
    -- stream subscriptions, no asserts
}
```
Does this scope emit BEGIN/COMMIT? **No.** The compiler must detect whether the scope body contains persist ops (transitively, including through function calls). A scope with no persist ops is a lifecycle scope only, zero transaction overhead.

Detection problem: If the scope calls `create_order()` and that function contains asserts, the scope IS transactional. But does the compiler know at codegen time what functions contain asserts? It must — this requires a **persist-analysis pass** that marks functions as "contains persist effects".

**F1.3 — Scope exit semantics are unambiguous.**
Three exit paths:
1. Normal exit → COMMIT
2. `abort` → ROLLBACK
3. Runtime error → ROLLBACK via Drop

What about `return`? If a function contains a scope, and the scope contains a `return`:
```runa
> process(x: Int) -> Int {
    | scope work {
        assert Record(x, "start")
        if x < 0 { return -1 }    -- exits function, exits scope
        assert Record(x, "done")
    }
    0
}
```
The `return -1` exits the scope AND the function. Does the Record("start") commit or rollback? **It must ROLLBACK** — the scope exited abnormally (not through its closing brace). This requires the TxGuard Drop to default to ROLLBACK unless explicitly committed:

```rust
struct TxGuard { committed: bool, /* ... */ }
impl Drop for TxGuard {
    fn drop(&mut self) {
        if !self.committed {
            // ROLLBACK — scope exited without reaching COMMIT point
        }
    }
}
```
COMMIT is set explicitly at the scope's final statement. Any other exit path (return, abort, panic) triggers ROLLBACK via Drop. **This is the correct design.**

**Status: HOLDS**, with two additions:
- Lint warning for bare asserts outside scopes
- Persist-analysis pass to determine which functions contain persist effects


---

## Claim 2: "`abort` triggers ROLLBACK cleanly"

**The proposition:** A new keyword `abort` inside a transactional scope causes immediate ROLLBACK and exits the scope without executing remaining statements.

### Pressure tests

**F2.1 — What IS `abort`?**
Options:
- A keyword (like `return`, `break`)
- A builtin function (like `print`)
- A panic (like Rust's `panic!()`)

Panic is wrong — it kills the program. `abort` should exit the scope, not the process. It's closest to `break` for a labeled block. The Rust codegen:

```rust
'scope_name: {
    let _tx = TxGuard::begin(&db);
    // ...
    if !invariant {
        break 'scope_name;   // TxGuard::drop fires → ROLLBACK
    }
    _tx.commit();             // explicit commit at end
}
```

`abort` = `break 'scope_name`. The Drop safety net does the ROLLBACK. **This is clean.**

**F2.2 — `abort` outside a scope.**
```runa
> process(x: Int) -> Int {
    abort   -- what happens?
}
```
If abort is scope-bound (like `break` outside a loop), this is a compile error. The type checker catches it: `error: abort used outside transactional scope`. **Yes — abort is scope-local.**

**F2.3 — `abort` with a value.**
```runa
| scope checkout {
    assert Order(1, "pending")
    if not(valid) { abort Err("invalid") }
    assert Order(1, "confirmed")
}
```
Should `abort` carry a return value? If the scope is an expression that produces a value (like `match`), then `abort` should carry the error case. But scopes are currently statements, not expressions. Keep it simple: **`abort` takes no value.** It's a bare control flow keyword. If you need error info, bind it before aborting:

```runa
| scope checkout {
    -- ...
    if not(valid) {
        @ print("checkout failed: invalid order")
        abort
    }
}
```

**F2.4 — `abort` in nested scope.**
```runa
| scope outer {
    assert Order(1, "pending")
    | scope inner {
        assert OrderItem(1, 1, "Widget", 3)
        abort   -- which scope does this abort?
    }
    assert Order(1, "confirmed")
}
```
`abort` aborts the **innermost enclosing scope** (like `break` in nested loops). The inner scope ROLLBACKs TO SAVEPOINT. The outer scope continues. `Order(1, "pending")` survives, `OrderItem` does not.

BUT — does execution continue after the inner scope? The inner scope aborted, which is not a normal exit. The outer scope needs to know. Two options:
1. Inner abort propagates (outer also fails) — too aggressive
2. Inner abort is handled, outer continues — requires try/catch semantics

**Resolution:** A scope that aborts is like a scope that errors. The outer scope can continue ONLY if it explicitly handles the abort. But Futuruna doesn't have try/catch.

**Simplest correct answer: inner abort propagates to outer.** If you want partial failure handling, use separate sequential scopes, not nested ones:

```runa
| scope create_order {
    assert Order(1, "pending")
}
-- order committed

| scope add_items {
    assert OrderItem(1, 1, "Widget", 3)
    -- if this fails, the order still exists
}
```

**Status: HOLDS**, with decisions:
- `abort` = keyword, compiles to `break 'scope_label`
- Scope-local only (compile error outside scope)
- No value carried
- In nested scopes: propagates upward (inner abort = outer abort)
- Alternative: sequential scopes for partial failure isolation


---

## Claim 3: "Functions participate in enclosing transactions"

**The proposition:** A function that contains `assert`/`retract` participates in whatever transaction is active at call time. No enclosing transaction = auto-commit per statement. Enclosing scope = all ops are part of that transaction.

### Pressure tests

**F3.1 — Same function, different behavior.**
```runa
> create_order(user_id: Int) -> Int {
    = oid = 42
    assert Order(oid, user_id, "pending")
    assert OrderItem(oid, 101, 2)
    oid
}

-- Call 1: no scope → two auto-commits
= id1 = create_order(1)

-- Call 2: inside scope → one transaction
| scope checkout {
    = id2 = create_order(2)
    assert Payment(id2, 500, "charged")
}
```

Is this confusing? Call 1 auto-commits each assert independently. Call 2 batches everything. **The function behaves differently based on call context.**

Verdict: This is exactly how SQL works. A stored procedure's DML is part of whatever transaction called it. It's the expected behavior for database-aware developers. But it MUST be documented: "Functions are transaction-transparent. They join the enclosing transaction if one exists."

**F3.2 — The invisible dependency.**
A function's documentation says "creates an order." A developer calls it inside a scope expecting atomicity. But the function also calls `@ print(...)` which is a side effect that can't be rolled back. The asserts roll back, but the print already happened.

This is fundamental: **transactions only cover persist operations.** IO (`@ print`, `@ write_file`, `http_post`) is not transactional. This must be explicitly stated: "Only `assert`/`retract` on persisted types participate in transactions. Other effects are irrevocable."

**F3.3 — The actor boundary.**
```runa
| scope batch {
    for order in orders {
        processor_actor <- Process(order)   -- actor does asserts
    }
}
```
The actor runs on a different Tokio task. Its asserts happen on a different code path. They are NOT part of the enclosing scope's transaction. Each actor message handler is its own transaction context.

**This is correct but potentially surprising.** The compiler should warn (or error) when `assert`/`retract` happens inside an actor handler that might be called from a transactional scope. Or simpler: document that actor boundaries are transaction boundaries.

**Status: HOLDS**, with documentation requirements:
- Functions are transaction-transparent (join enclosing transaction)
- Only persist ops are transactional (IO is irrevocable)
- Actor boundaries are transaction boundaries (each handler is independent)


---

## Claim 4: "watch events are reliable"

**The proposition:** `watch(Type)` emits events for every assert/retract, and subscribers see a consistent stream.

### Pressure tests

**F4.1 — The premature notification problem. CRITICAL.**

`sqlite3_update_hook` fires on every INSERT/UPDATE/DELETE **immediately**, even inside an uncommitted transaction. If the transaction later ROLLBACKs, subscribers already received events for data that was never committed.

```runa
~ orders = watch(Order)
~ orders | o -> { @ print("New order: " + show(o)) }

| scope risky {
    assert Order(1, "pending")   -- watch fires: "New order: ..."
    = x = 10 / 0                 -- crash → ROLLBACK
    -- Order(1) was never committed, but subscriber already printed it
}
```

**This is a real bug in the naive design.** The subscriber saw a phantom order.

**Fix:** Buffer watch events inside the TxGuard. Only flush to the broadcast channel on COMMIT. Discard on ROLLBACK.

```rust
struct TxGuard {
    db: Arc<Mutex<Connection>>,
    committed: bool,
    pending_events: Vec<FactEvent>,  // buffered, not sent yet
    tx: broadcast::Sender<FactEvent>,
}

impl TxGuard {
    fn commit(mut self) {
        self.db.lock().unwrap().execute("COMMIT", []).unwrap();
        self.committed = true;
        // NOW flush events
        for event in self.pending_events.drain(..) {
            self.tx.send(event).ok();
        }
    }
}

impl Drop for TxGuard {
    fn drop(&mut self) {
        if !self.committed {
            self.db.lock().unwrap().execute("ROLLBACK", []).unwrap();
            self.pending_events.clear();  // discard phantom events
        }
    }
}
```

**This changes the architecture:** We can't use `sqlite3_update_hook` directly for watch. Instead, each `assert`/`retract` codegen must also push to the TxGuard's pending_events. The hook is replaced by explicit event creation.

**F4.2 — Auto-commit asserts (no scope).**
```runa
assert User(1, "Alice", "alice@example.com", True)
~ users = watch(User)
```
No scope = auto-commit. The event should fire immediately after the implicit single-statement transaction. No buffering needed.

This means watch event timing depends on whether there's an enclosing scope:
- Inside scope: buffered until COMMIT
- No scope: immediate

This is correct and matches transactional semantics. But the implementation has two code paths.

**F4.3 — Watch ordering.**
If two asserts happen in one scope:
```runa
| scope batch {
    assert Order(1, "pending")
    assert Order(2, "pending")
}
```
On COMMIT, both events flush. Are they ordered? **Yes** — the pending_events Vec preserves insertion order. Subscribers see Order(1) then Order(2).

**Status: FAILS without buffering fix.** The naive `sqlite3_update_hook` approach breaks on ROLLBACK. Must buffer events in TxGuard and flush only on COMMIT.


---

## Claim 5: "Concurrent actors can write safely"

**The proposition:** Multiple actors can assert/retract into the same persisted type. SQLite serializes their writes.

### Pressure tests

**F5.1 — The deadlock scenario. CRITICAL.**
```runa
> actor worker_a(n: Int) {
    | Process -> {
        | scope work {
            assert Result(1, "from_a")
            = response = ask(worker_b, Check)  -- blocks waiting for B
        }
        n + 1
    }
}

> actor worker_b(n: Int) {
    | Check -> {
        | scope verify {
            assert Result(2, "from_b")   -- needs write lock, A holds it
        }
        n
    }
}
```

Actor A holds a transaction (write lock on SQLite). Inside that transaction, A sends a synchronous message to actor B. B's handler tries to write, needs the write lock, blocks. A waits for B's response. B waits for A's lock. **Deadlock.**

**Fix options:**
1. **Lint:** Warn on `ask()` inside transactional scope. "Synchronous actor messages inside transactions may deadlock."
2. **Separate connections:** Each actor gets its own SQLite connection. SQLite's busy_timeout prevents permanent deadlock (one will timeout and ROLLBACK). But this changes the connection model from shared to per-actor.
3. **Design rule:** "Never `ask` inside a scope." Use `send` (fire-and-forget) instead.

**Recommendation:** Option 1 (lint) + Option 2 (separate connections with busy_timeout). The lint catches the obvious cases. Separate connections prevent permanent deadlock.

```rust
// Each actor handler opens its own connection with busy_timeout
let conn = Connection::open("app.db")?;
conn.busy_timeout(Duration::from_secs(5))?;
conn.execute_batch("PRAGMA journal_mode=WAL")?;
```

WAL mode is essential here — it allows concurrent readers even while one connection writes.

**F5.2 — The lost update.**
```
Actor A reads: Account(1, balance=1000)
Actor B reads: Account(1, balance=1000)
Actor A writes: Account(1, balance=800)   -- deducted 200
Actor B writes: Account(1, balance=700)   -- deducted 300
-- Final: 700. Should be 500. A's deduction lost.
```

This is the classic read-then-write race. SQLite's serialized writes don't prevent it because the reads happened outside the write locks.

**Fix:** Read-modify-write must happen inside a single scope:
```runa
| scope deduct {
    = rows = findall(b, Account(1, _, b))
    = balance = head(rows)
    retract Account(1, _, _)
    assert Account(1, "Alice", balance - 200)
}
```

The scope serializes the whole read-modify-write. But ONLY if using `IMMEDIATE` transactions (not `DEFERRED`). With deferred, the read doesn't acquire a lock.

**Decision:** Transactional scopes should use `BEGIN IMMEDIATE` (not `BEGIN DEFERRED`). This acquires the write lock at scope entry, preventing concurrent read-modify-write races.

**F5.3 — Connection pool or single connection?**

The current DB codegen uses `Arc<Mutex<Connection>>` (single shared connection). For persist with actors, this becomes a bottleneck. Options:
- Single connection with Mutex: serialized, no deadlock possible (but blocks actors)
- Connection-per-actor: parallel reads, serialized writes, deadlock possible
- Connection pool: best throughput, complex

**Start with single connection.** It's simplest, deadlock-free (Mutex ensures one writer at a time), and correct. Upgrade to connection-per-actor in a later milestone if performance requires it.

**Status: HOLDS with single connection model.** The deadlock scenario only exists with multiple connections. Single `Arc<Mutex<Connection>>` prevents it by serializing all DB access. The cost is throughput — acceptable for embedded use.


---

## Claim 6: "Schema hashing detects all changes"

**The proposition:** A hash of field names + types + order detects any schema change that requires migration.

### Pressure tests

**F6.1 — Adding an Option field (nullable).**
```runa
-- v1: # User(id: Int, name: String)
-- v2: # User(id: Int, name: String, bio: Option(String))
```
Hash changes. But SQLite can handle this with `ALTER TABLE ADD COLUMN bio TEXT`. No data migration needed — existing rows get NULL. **Should this require a migration rule?**

No. The compiler should recognize "added nullable field at end" as an auto-migration:
```
[persist] User: schema changed (v1 -> v2)
  + bio: Option(String)  [nullable — auto-migrated via ALTER TABLE]
```

But "added non-nullable field" REQUIRES a migration rule (what value for existing rows?).

**F6.2 — Field reordering.**
```runa
-- v1: # User(id: Int, name: String, email: String)
-- v2: # User(id: Int, email: String, name: String)
```
Hash changes. But the SQLite data is identical (columns are named). This is a false positive.

**Fix:** Hash should be ORDER-INDEPENDENT for the purpose of detecting data-breaking changes. But field order matters for positional construction (`User(1, "alice", "alice@x.com")` changes meaning). So reordering IS a breaking change for the language, even if SQLite doesn't care.

**Decision:** Reordering changes the hash AND requires a migration rule (because Futuruna uses positional construction).

**F6.3 — Renaming a field.**
```runa
-- v1: # User(id: Int, name: String)
-- v2: # User(id: Int, full_name: String)
```
The compiler sees: field removed (`name`), field added (`full_name`). It can't distinguish rename from drop+add. Migration rule required. **Correct behavior.**

**Status: HOLDS**, with one refinement: auto-migrate `Option` field additions at the end.


---

## Claim 7: "retract with patterns maps cleanly to DELETE WHERE"

### Pressure tests

**F7.1 — Ambiguous bindings.**
```runa
= name = "Alice"
retract User(_, name, _, _)
```
Is `name` a bound variable (delete WHERE name = 'Alice') or an unbound pattern variable (match anything, bind it)? In Datalog, a variable that's already bound in the enclosing scope is used as a filter. An unbound variable is a wildcard.

**This matches Datalog semantics.** Bound = filter, unbound = wildcard. The compiler knows which variables are in scope.

**F7.2 — Pattern retract deletes too much.**
```runa
retract User(_, _, _, False)    -- all inactive users gone
```
This could delete thousands of rows. No confirmation, no limit. **Is this safe?**

It's as safe as `DELETE FROM user WHERE active = 0` in SQL. The developer wrote it, the developer means it. But inside a scope, it's atomic and rollbackable. Outside a scope, it's permanent.

**Lint opportunity:** Warn on retract patterns with more than N wildcards: "retract with 3+ wildcards may delete many rows. Consider wrapping in a scope."

**F7.3 — Retract on non-persisted facts.**
```runa
| parent("alice", "bob")
retract parent("alice", "bob")
```
The `parent` fact is in-memory (no `@ persist`). Retract must work on in-memory fact tables too (mutate the Vec). This is a separate code path from the SQL path.

**Status: HOLDS.** Bound variables filter, unbound wildcards match anything. Two code paths: SQL for persisted, Vec mutation for in-memory.


---

## Claim 8: "Scope + verify = transactional proof"

**The proposition:** Invariants checked with `?` inside a scope can abort the transaction, making the database self-verifying.

### Pressure tests

**F8.1 — Bare `?` inside scope.**
```runa
| scope transfer {
    retract Account(1, _, 1000)
    assert Account(1, "Alice", 800)
    | positive: 800 -> 800 >= 0
    ? positive                        -- bare ? — halts on fail
}
```
Bare `?` halts the program on failure. Inside a scope, does it ROLLBACK first? **Yes** — the halt triggers Drop on the TxGuard, which ROLLBACKs. Then the program halts. The database is consistent.

But this is aggressive — the whole program dies. Better pattern: `? positive else { abort }`. This ROLLBACKs the scope but the program continues.

**Decision:** Document that bare `?` inside a scope ROLLBACKs then halts. Recommend `? ... else { abort }` for recoverable failures.

**F8.2 — Invariant references uncommitted data.**
```runa
| scope transfer {
    retract Account(1, _, 1000)
    assert Account(1, "Alice", 800)

    = balances = findall(b, Account(_, _, b))
    | all_positive: min(balances) -> min(balances) >= 0
    ? all_positive else { abort }
}
```
`findall` inside the scope must see the uncommitted state (Account(1) = 800, not 1000). This requires read-your-writes within the transaction. **SQLite provides this on the same connection.**

**Status: HOLDS.** The `?` rune already has the right semantics. Inside a scope, Drop ensures ROLLBACK. `findall` sees uncommitted data within the same connection.


---

## Claim 9: "assert can return auto-generated IDs"

**The proposition:** `= oid = assert Order(user_id, total, "pending")` returns the auto-incremented ID.

### Pressure tests

**F9.1 — Grammar change.**
Currently `assert` would be a statement (like `@ print`). Making it an expression that returns a value means `assert` in expression position. This is a parser change.

Two options:
- `assert` is always an expression (returns `()` for non-auto types, returns the ID for auto types)
- `assert` is a statement; use `= id = last_insert_id()` for auto IDs

**Decision:** Start with assert-as-statement + `last_insert_id()` builtin. It's simpler, no grammar changes, and SQLite's `last_insert_rowid()` is exactly this. The syntactic sugar `= id = assert ...` can come later.

**Status: DEFERRED.** Start with `last_insert_id()` builtin. Add assert-as-expression later if the pattern is common enough.


---

## Claim 10: "Index inference from rules is sufficient"

**The proposition:** The compiler reads rule topology and creates indexes for the columns that appear as bound variables in queries.

### Pressure tests

**F10.1 — Over-indexing.**
Every rule creates an index. A file with 50 rules over the same type creates 50 indexes. Each index slows down writes.

**Fix:** Deduplicate. Multiple rules filtering on the same column position → one index. The compiler collects all filtered positions per table and creates only unique indexes.

**F10.2 — Under-indexing.**
```runa
= results = findall(name, User(_, name, email, _)), email == target_email
```
The `email == target_email` is in a separate filter, not in the pattern position. Does the compiler detect this? It would need to analyze the filter expression, not just the pattern variables.

**Decision for M26:** Only infer indexes from pattern positions in rules and findall. Filter-expression analysis is an optimization for later.

**F10.3 — Compound indexes.**
```runa
| user_orders(uid, status) -> Order(_, uid, _, status)
```
This filters on both `user_id` AND `status`. One compound index `(user_id, status)` is better than two single indexes. But compound index inference is complex.

**Decision for M26:** Single-column indexes only. Compound indexes later.

**Status: HOLDS for basic cases.** Dedup, single-column only, pattern-position only. Good enough for M26.


---

## Claim 11: "Complex nested types serialize to JSON transparently"

**The proposition:** Non-primitive fields (List, Map, nested structs, ADTs with data) serialize to JSON TEXT columns. Deserialization reconstructs the Futuruna value.

### Pressure tests

**F11.1 — Round-trip fidelity.**
```runa
# Item(name: String, price: Int)
# Order(id: Int, items: List(Item))
@ persist Order

assert Order(1, [Item("Widget", 500), Item("Gadget", 300)])
= orders = findall(items, Order(1, items))
= first = head(head(orders))    -- Item("Widget", 500)
```
The items field goes through: `List(Item)` → JSON string → SQLite TEXT → JSON string → `List(Item)`. Does it round-trip perfectly?

**Potential failures:**
- Float precision loss (JSON uses IEEE 754, but Futuruna Floats are f64 — same. OK.)
- Map ordering (JSON objects are unordered, Futuruna Maps are ordered by key. Must sort on deserialize.)
- ADT variants with data: `Some(42)` → `{"Some":42}` → `Some(42)`. Must know the type to deserialize correctly.

**The type problem:** JSON `{"Some":42}` could be `Option(Int)`, `Option(Float)`, or a custom ADT. The deserializer needs the Futuruna type definition to reconstruct correctly. This means **deserialize is type-directed** — the codegen knows the column's Futuruna type and generates the right deserialization code.

```rust
// Codegen for deserializing Order.items (List(Item)):
let items_json: String = row.get("items")?;
let items: Vec<Item> = serde_json::from_str(&items_json)?;
```

This requires `#[derive(Serialize, Deserialize)]` on `Item`. The codegen already emits serde derives when `@ depend "serde"` is present. For persist types, **serde is auto-added as a dependency**.

**F11.2 — Nested struct with Option field.**
```runa
# Address(street: String, city: String, zip: Option(String))
# Customer(id: Int, name: String, address: Address)
@ persist Customer

assert Customer(1, "Alice", Address("123 Main", "Copenhagen", None))
```
JSON: `{"street":"123 Main","city":"Copenhagen","zip":null}`. Deserialize: `Address("123 Main", "Copenhagen", None)`. **Works** — serde handles Option/None ↔ null natively.

**F11.3 — You can't query into JSON columns.**
```runa
| copenhagen_customers(name) -> Customer(_, name, addr), addr.city == "Copenhagen"
```
The `addr` column is a JSON blob. SQLite can't execute `WHERE addr.city = 'Copenhagen'` on a TEXT column. This rule MUST deserialize every row and filter in Futuruna.

**Solved: `json_extract`.** SQLite's `json_extract()` queries into JSON columns directly. The compiler sees `addr.city` on a persisted type and generates `WHERE json_extract(address, '$.city') = 'Copenhagen'`. No flattening needed. SQLite 3.38+ supports expression indexes on `json_extract` for performance. The rules don't bend.

For `@ store` mode: irrelevant — full scan + Futuruna filter after deserialization, nested access works natively.

**Status: HOLDS**, with type-directed deserialization, serde auto-dependency, and json_extract for nested field queries on `@ persist` types.


---

## Claim 12: "Object store eliminates migrations"

**The proposition:** `@ store Type` persists the entire struct as a JSON blob. Schema changes don't require migration because JSON is flexible.

### Pressure tests

**F12.1 — Added field.**
```runa
-- v1: # Config(key: String, mode: String)
-- v2: # Config(key: String, mode: String, theme: String)
```
Old row JSON: `{"key":"app","mode":"dark"}`. Missing `theme`. Deserialize to `Config("app", "dark", ???)`.

What's the zero value for String? `""`. For Int? `0`. For Bool? `False`. For Option? `None`.

**For Option fields, this is perfect** — missing = None. For required fields, the zero value might be wrong. The developer should use Option for fields that may not exist in old data.

**Recommendation:** Warn when a non-Option field is added to a stored type: `warn: new non-Option field 'theme' on stored type Config — old rows will get zero value "". Consider using Option(String).`

**F12.2 — Removed field.**
```runa
-- v1: # Config(key: String, mode: String, theme: String)
-- v2: # Config(key: String, mode: String)
```
Old row JSON still has `"theme":"dark"`. Serde's `#[serde(deny_unknown_fields)]` would fail. **Must NOT use deny_unknown_fields.** Use `#[serde(default)]` — unknown fields are silently ignored.

**F12.3 — Renamed field.**
```runa
-- v1: # Config(key: String, mode: String)
-- v2: # Config(key: String, display_mode: String)
```
Old JSON has `"mode"`, new struct expects `"display_mode"`. Serde sees missing field + unknown field. `display_mode` gets zero value, `mode` is ignored. Data silently lost.

**This is the one case where object store breaks.** Renames are invisible data loss. Options:
1. Document it: "renames in stored types lose old data. Use `@ migrate` or dump-and-reload."
2. Add `@ rename mode -> display_mode` hint (too complex for the value).
3. Accept it: renames are rare, and the dump workflow handles them.

**Decision:** Document it. Renames on `@ store` types require dump-and-reload.

**F12.4 — Changed field type.**
```runa
-- v1: # Config(key: String, timeout: String)
-- v2: # Config(key: String, timeout: Int)
```
Old JSON: `"timeout":"30"`. New struct expects Int. Serde fails to deserialize `"30"` as i64.

**This breaks.** Type changes in stored types require migration. Options:
- `@ migrate` rule (same as fact store)
- Dump-and-reload
- Use a version field: `# Config(key: String, version: Int, ...)` and branch on version

**Decision:** Type changes on `@ store` types still need `@ migrate` or dump-and-reload. The "no migration" claim only applies to additive changes (new fields, removed fields).

**Refined claim:** Object store eliminates migrations for **additive schema changes** (added fields, removed fields). Renames and type changes still require explicit migration.

**Status: PARTIALLY HOLDS.** Additive changes are free. Renames silently lose data (document this). Type changes break (need migration). Still much better than fact store, where ANY change needs migration.


---

## Claim 13: "Dump-as-migration is a sufficient escape hatch"

**The proposition:** `runa dump` exports all persisted data as valid Futuruna `|` facts. This output can be version-controlled, edited, transformed, and re-imported.

### Pressure tests

**F13.1 — Large datasets.**
1 million rows dumped as `| User(...)` statements = ~100 MB of text. Is this practical? For embedded databases (the target), this is fine. SQLite databases above 100 MB are unusual in embedded contexts.

For truly large datasets: `runa dump --format sqlite` could export as raw SQLite file copy (instant, but not human-readable). Two formats: readable (facts) and fast (binary).

**F13.2 — Complex types in dump format.**
```runa
| Order(1, [Item("Widget", 500), Item("Gadget", 300)], Shipped)
```
Is this valid Futuruna syntax? Yes — list literals, constructor calls, and ADT variants all parse today. The dump format is just Futuruna source.

But what about Maps?
```runa
| Config("theme", map_from([["mode", "dark"], ["font", "mono"]]))
```
`map_from` is a function call, not a literal. The dump needs to emit Map values as `map_from(...)` calls. This is slightly ugly but works.

**F13.3 — Re-import idempotency.**
If you dump and re-import without changing anything, you get duplicate facts. The re-import needs to either:
1. Clear the table first (`retract Type(_, _, ...)` all rows)
2. Use INSERT OR REPLACE (upsert by primary key)
3. The dump file includes cleanup: `retract Type(_, _, ...)` before facts

Option 3 is cleanest — the dump file is self-contained:
```runa
-- Auto-generated dump. Safe to re-import.
retract User(_, _, _, _)
| User(1, "Alice", "alice@example.com", True)
| User(2, "Bob", "bob@example.com", True)
```

**Status: HOLDS**, with two refinements: dump includes `retract all` before facts, and Maps emit as `map_from(...)`.


---

## Summary of Design Changes Required

| # | Claim | Status | Required change |
|---|-------|--------|-----------------|
| 1 | Scope as transaction boundary | HOLDS | Add persist-analysis pass + lint for bare asserts |
| 2 | abort triggers ROLLBACK | HOLDS | abort = keyword, compiles to break 'scope; propagates in nesting |
| 3 | Functions join enclosing transaction | HOLDS | Document transaction-transparency; actor = transaction boundary |
| 4 | watch events are reliable | **FAILS** | Must buffer events in TxGuard, flush on COMMIT only |
| 5 | Concurrent actor writes | HOLDS* | Single connection model; upgrade to per-actor later |
| 6 | Schema hashing | HOLDS | Auto-migrate nullable field additions |
| 7 | Pattern retract | HOLDS | Bound vars = filter, unbound = wildcard; lint for broad retract |
| 8 | Scope + verify | HOLDS | Works naturally; document bare ? vs ? else { abort } |
| 9 | assert returns auto ID | DEFERRED | Use last_insert_id() builtin first |
| 10 | Index inference | HOLDS | Single-column, pattern-only, deduped |
| 11 | Nested types serialize to JSON | HOLDS | Type-directed deserialization, auto serde dep, warn on JSON queries |
| 12 | Object store eliminates migrations | PARTIAL | Additive changes free; renames lose data; type changes still break |
| 13 | Dump-as-migration escape hatch | HOLDS | Dump includes retract-all; Maps emit as map_from(); two formats |

Critical fixes:
- **Claim 4 (watch buffering)** — naive design sends phantom events on ROLLBACK. Must buffer.
- **Claim 12 (object store)** — "no migration" only holds for additive changes. Renames and type changes still need migration or dump-and-reload. Must document honestly.

---

## Implementation Milestones

### M26a: `@ store Type` — Object Store (Quick Win)

**What:** Simplest possible persistence. Entire struct → JSON blob, keyed by first field. No column mapping, no type mapping, no schema migration. Get persistence working first.

**Why start here:** Object store is 3x simpler than fact store. It proves the `assert`/`retract` syntax, the DB connection lifecycle, and the `runa dump` workflow. Fact store (columnar) builds on top of it.

**Compiler changes:**
- Lexer/parser: recognize `@ store Typename` as an effect statement
- New `stored_types: BTreeSet<String>` in codegen context
- Auto-add `serde` + `serde_json` + `rusqlite` dependencies
- Derive `Serialize`/`Deserialize` on stored structs (with `#[serde(default)]` for flexibility)
- On startup: open/create DB, CREATE TABLE IF NOT EXISTS (id column + data TEXT column)
- `assert` on stored type → INSERT OR REPLACE (serialize to JSON)
- `retract` on stored type → DELETE WHERE id = ?
- `findall` on stored type → SELECT all, deserialize, filter in Futuruna

**Tests (tests/store_basic_test.runa):**
```
1. @ store on a struct → table exists after run
2. assert adds a row (findall sees it)
3. retract removes a row (findall doesn't see it)
4. Rerun same program → no duplicate data (INSERT OR REPLACE)
5. Complex fields (List, Map) round-trip through JSON
6. Nested struct field round-trips
7. Option field: None ↔ null
8. ADT enum field: variant name ↔ string
```

**Depends on:** Nothing (foundational)

---

### M26b: `@ persist Type` — Fact Store (Columnar)

**What:** Parse `@ persist` effect. Generate CREATE TABLE with typed columns. Store schema hash.

**Compiler changes:**
- Lexer/parser: recognize `@ persist Typename` as an effect statement
- New `persisted_types: BTreeSet<String>` in codegen context
- On startup codegen: open/create SQLite DB, CREATE TABLE IF NOT EXISTS, insert schema hash into `_schema` table
- Type mapping function: Futuruna type → SQLite column type (Int→INTEGER, Float→REAL, etc.)
- Complex fields (List, nested struct, Map) → TEXT with JSON serialization
- Warn on rules that query into JSON-serialized columns

**Tests (tests/persist_schema_test.runa):**
```
1. @ persist on a struct → table exists after run
2. Rerun same program → no error (IF NOT EXISTS)
3. Int/Float/String/Bool fields → correct SQLite types
4. Option(String) field → nullable TEXT column
5. Two persisted types → two tables
6. Non-persisted type → no table (still const array)
7. Schema hash stored in _schema table
8. List field → JSON TEXT column, round-trips
9. Nested struct field → JSON TEXT column, round-trips
```

**Depends on:** M26a (shares DB connection lifecycle, assert/retract parsing)

---

### M26c: `assert` / `retract` — Mutation Keywords

**What:** New keywords for both store and persist modes. Also works on in-memory facts.

**Compiler changes:**
- Lexer: `assert` and `retract` as keywords
- Parser: parse as statement (Stmt::Assert / Stmt::Retract)
- Type checker: verify type exists, verify arity matches
- Codegen (stored): INSERT OR REPLACE / DELETE with JSON (via M26a)
- Codegen (persisted): INSERT INTO / DELETE FROM with typed params (via M26b)
- Codegen (in-memory): Vec push / Vec retain
- Wildcard `_` in retract: omit from WHERE clause (persisted), skip match position (in-memory)

**Tests (tests/persist_mutation_test.runa):**
```
1. assert adds a row (findall sees it) — persisted
2. retract with full tuple removes exact match — persisted
3. retract with wildcard(s) removes partial match — persisted
4. retract Type(_, _, _, False) removes all matching
5. assert on non-persisted type adds to in-memory facts
6. retract on non-persisted type removes from in-memory facts
7. assert duplicate primary key → error (persisted) / duplicate (in-memory)
8. retract non-existent row → no error (0 rows affected)
9. assert on @ store type → JSON blob stored
10. retract on @ store type → row deleted by key
```

**Depends on:** M26a (or can be developed in parallel — mutation syntax is mode-independent)

---

### M26d: `findall` on Persisted/Stored Facts — Query

**What:** When findall targets a persisted or stored type, generate SELECT instead of iterating const tables.

**Compiler changes:**
- In findall codegen: check if target type is in `persisted_types` or `stored_types`
- If stored: SELECT all, deserialize JSON, filter in Futuruna (full scan)
- If persisted: generate `db.prepare("SELECT ...")` with WHERE clause
- Bound variables in pattern → WHERE clause parameters (persisted only)
- Return as Vec of tuples/structs (same shape as in-memory path)

**Tests (tests/persist_query_test.runa):**
```
1. findall on persisted type returns all rows
2. findall with one bound var → filtered results (persisted)
3. findall with multiple bound vars → AND conditions (persisted)
4. findall after assert sees new row
5. findall after retract doesn't see deleted row
6. Rule over persisted type works (| expensive(name) -> Product(_, name, p), p > 1000)
7. findall on stored type returns all (full scan + filter)
8. Mixed: rule joining persisted + non-persisted facts
```

**Depends on:** M26a, M26b, M26c

---

### M26e: Transactional `| scope` — ACID

**What:** Scopes containing persist ops become transactions. BEGIN/COMMIT/ROLLBACK. abort keyword. Nested scopes use SAVEPOINT.

**Compiler changes:**
- Persist-analysis pass: mark functions that contain assert/retract (transitive)
- Scope codegen: detect if body (transitively) contains persist ops
- If transactional: emit TxGuard with BEGIN IMMEDIATE on entry
- COMMIT at scope's final statement
- Drop → ROLLBACK (safety net)
- `abort` keyword: lexer + parser + codegen as `break 'scope_label`
- Nested transactional scope: SAVEPOINT / RELEASE / ROLLBACK TO

**Tests (tests/persist_transaction_test.runa):**
```
1. Scope with two asserts: both visible after scope
2. Scope with abort: neither visible
3. Scope with runtime error: neither visible (ROLLBACK via Drop)
4. Read-your-writes: findall inside scope sees uncommitted assert
5. ? inside scope: invariant pass → COMMIT
6. ? inside scope with else abort: invariant fail → ROLLBACK
7. Nested scope success: both commit
8. Nested scope inner abort: inner rolls back, outer propagates
9. Scope without persist ops: no BEGIN/COMMIT overhead
10. Function with asserts called inside scope: joins transaction
11. Function with asserts called outside scope: auto-commits
12. abort outside scope: compile error
```

**Depends on:** M26a, M26c, M26d

---

### M26f: `runa dump` / `runa load` — Data Portability

**What:** Export all persisted/stored data as valid Futuruna source. Import it back.

**Compiler changes:**
- New CLI command: `runa dump file.runa` → outputs `|` facts + `retract` preamble
- New CLI command: `runa load file.runa < dump.runa` → runs dump as source
- Dump format: `retract Type(_, _, ...)` then `| Type(...)` for each row
- Stored types: serialize nested values as Futuruna literals (list literals, map_from, constructors)
- Persisted types: emit flat `| Type(field1, field2, ...)` statements

**Tests (manual / integration):**
```
1. runa dump produces valid Futuruna source
2. Dump + load round-trips without data loss
3. Dump includes retract-all preamble (idempotent reload)
4. Complex nested types in @ store dump correctly (lists, maps)
5. Dump of empty table produces retract + no facts
```

**Depends on:** M26a, M26c

---

### M26g: `@ migrate` — Schema Evolution

**What:** Detect schema changes via hash comparison. Apply migration rules. `@ migrate drop` for dev mode.

**Compiler changes:**
- Parse `@ migrate Type(old_pattern) -> Type(new_pattern)` as effect statement
- On startup: compare stored hash in _schema vs current type hash
- If mismatch: find migration rule matching old arity
- Execute migration: SELECT all old rows, transform, recreate table, INSERT new rows
- No migration rule + non-auto-migratable change → compile error with suggestion
- Auto-migrate: adding Option field at end (ALTER TABLE ADD COLUMN) for `@ persist`
- `@ migrate Type drop` → DROP TABLE + CREATE TABLE
- `@ store` types: mostly skip (JSON flexes), only needed for type changes/renames

**Tests (tests/persist_migration_test.runa):**
```
1. Add non-nullable field with migration rule: rows transformed
2. Add Option field at end: auto-migrated (no rule needed)
3. Remove field with migration rule: rows transformed
4. Schema mismatch without migration rule: error with suggestion
5. Chained migration: old arity matches correct rule
6. @ migrate Type drop: table recreated empty
7. No schema change: no migration runs
```

**Depends on:** M26b

---

### M26h: `watch(Type)` — Change Streams

**What:** Connect persist/store mutations to broadcast channels. Buffer events in transactions.

**Compiler changes:**
- `watch(Type)` builtin: returns a broadcast Receiver for the type
- Each persisted/stored type gets a `broadcast::Sender` in codegen context
- assert/retract codegen: push FactEvent to TxGuard.pending_events (if in scope) or directly to broadcast (if bare)
- TxGuard.commit(): flush pending_events to broadcast
- TxGuard.drop() (rollback): discard pending_events
- FactEvent type: `# FactEvent(kind: EventKind, fact: T)`

**Tests (tests/persist_watch_test.runa):**
```
1. watch fires on bare assert (no scope)
2. watch fires on bare retract
3. watch inside scope: events arrive AFTER commit
4. watch with ROLLBACK (abort): no events emitted
5. watch |> filter works (standard stream ops)
6. watch |> map works
7. Multiple subscribers receive same events
```

**Depends on:** M26a, M26c, M26e

---

## Milestone Dependency Graph

```
M26a (object store)
  |
  +---> M26b (fact store / columnar) ---> M26g (migrations)
  |
  +---> M26c (assert/retract keywords)
  |       |
  |       +---> M26d (findall queries)
  |       |       |
  |       |       +---> M26e (transactions)
  |       |               |
  |       |               +---> M26h (watch streams)
  |       |
  |       +---> M26f (dump/load)
  |
  +---> M26g (migrations — only needs M26b)
```

**Critical path:** M26a → M26c → M26d → M26e → M26h

**Quick win path:** M26a → M26c → M26f (object store + mutation + dump = usable persistence in 3 milestones)

**Independent:** M26b (columnar) and M26g (migrations) can develop in parallel once M26a lands.

---

## Test Matrix

| Test file | Milestone | Tests | Key verification |
|-----------|-----------|-------|-----------------|
| store_basic_test.runa | M26a | 8 | Object store, JSON round-trip, nested types |
| persist_schema_test.runa | M26b | 9 | Columnar tables, type mapping, hash stored |
| persist_mutation_test.runa | M26c | 10 | INSERT/DELETE, wildcards, all three modes |
| persist_query_test.runa | M26d | 8 | SELECT generation, WHERE clauses, mixed |
| persist_transaction_test.runa | M26e | 12 | ACID, abort, nested, read-your-writes, ? |
| persist_dump_test (integration) | M26f | 5 | Round-trip, idempotent, complex types |
| persist_migration_test.runa | M26g | 7 | Hash diff, transform, auto-migrate, drop |
| persist_watch_test.runa | M26h | 7 | Events on commit, silent on rollback, ops |
| **Total** | | **66** | |

Every test is a falsifiable statement: "this program produces this output." If it doesn't, the implementation is wrong.
