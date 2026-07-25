# Futuruna Persist: The Database That Falls Out of the Language

## The Observation

Futuruna already contains every concept a database needs. No new runes are required. The mapping is structural, not metaphorical:

| Database concept | Futuruna construct | Rune | Status |
|---|---|---|---|
| Table definition | `# Product(id: Int, name: String, price: Int)` | `#` | Exists |
| Row / record | `\| Product(1, "Laptop", 8999)` | `\|` | Exists (Datalog facts) |
| Query / view | `\| expensive(name) -> Product(_, name, p), p > 1000` | `\|` | Exists (Datalog rules) |
| SELECT | `findall(name, Product(_, name, _))` | `\|` | Exists |
| CHECK constraint | `? price_positive` | `?` | Exists |
| Trigger / change feed | `~ watch(Product)` | `~` | New |
| Transaction | `\| scope atomic { assert ...; assert ... }` | `\|` | Needs design |
| Persistence | `@ persist Product` | `@` | New |
| Migration | `@ migrate Product(old) -> Product(new)` | `@` | New |
| INSERT / DELETE | `assert Product(...)` / `retract Product(...)` | New keywords | Needs design |

Three new things: `@ persist`, `assert`/`retract`, and `watch`. Everything else exists today.

---

## Part 1: The Persist Effect

### Syntax

```runa
# User(id: Int, name: String, email: String, active: Bool)
@ persist User
```

`@ persist` is an effect — it crosses the IO boundary, consistent with every other `@` usage in the language (`@ print`, `@ db_exec`, `@ write_file`). The compiler sees it and changes the storage backing for that fact set from in-memory const tables to SQLite.

### What It Generates

Without persist (current behavior):
```rust
const USER_FACTS: &[(i64, &str, &str, bool)] = &[
    (1, "Alice", "alice@example.com", true),
    (2, "Bob", "bob@example.com", false),
];
```

With persist:
```rust
// On startup: CREATE TABLE IF NOT EXISTS user (
//   id INTEGER PRIMARY KEY,
//   name TEXT NOT NULL,
//   email TEXT NOT NULL,
//   active INTEGER NOT NULL
// )
// Seed: INSERT OR IGNORE for each | fact in source
// findall: SELECT ... WHERE ...
```

### Persist Modes

```runa
@ persist User                              -- file: ./{name}.db (default)
@ persist User in "data/production.db"      -- explicit path
@ persist User in :memory:                  -- in-memory SQLite (testing)
@ persist User preload                      -- load all to RAM, write-through
```

### Type Mapping

| Futuruna type | SQLite type | Notes |
|---|---|---|
| `Int` | `INTEGER` | |
| `Float` | `REAL` | |
| `String` | `TEXT` | |
| `Bool` | `INTEGER` | 0/1 |
| `Option(T)` | nullable T | None → NULL |
| ADT enum | `TEXT` | Variant name as string |
| `List(T)` | `TEXT` | JSON-serialized |

ADT enums with data (e.g., `Some(42)`) serialize to JSON: `{"Some": 42}`. Simple enums serialize to their variant name: `"Active"`, `"Inactive"`.

---

## Part 2: Assert and Retract

### The Problem

Datalog facts are currently static — declared at compile time, immutable at runtime. A database needs INSERT and DELETE.

### Syntax

```runa
assert User(4, "Dave", "dave@example.com", True)
retract User(3, "Charlie", "charlie@example.com", False)
```

`assert` adds a fact. `retract` removes a fact. These are the only two mutation operations. There is no UPDATE — update is `retract` old + `assert` new. This is deliberate: it matches Datalog semantics (set of tuples), keeps the mental model minimal, and maps cleanly to event sourcing.

### Pattern Retract

Full-tuple retract (exact match):
```runa
retract User(3, "Charlie", "charlie@example.com", False)
-- DELETE FROM user WHERE id=3 AND name='Charlie' AND email='...' AND active=0
```

Pattern retract (wildcard):
```runa
retract User(3, _, _, _)
-- DELETE FROM user WHERE id=3
```

This is powerful. `retract User(_, _, _, False)` deletes ALL inactive users. The Datalog wildcard `_` maps to "don't care" in the WHERE clause.

### Codegen

For persisted facts:
```rust
// assert User(4, "Dave", "dave@example.com", true)
db.execute("INSERT INTO user VALUES (?, ?, ?, ?)", params![4, "Dave", "dave@example.com", 1])?;

// retract User(3, _, _, _)
db.execute("DELETE FROM user WHERE id = ?", params![3])?;
```

For non-persisted facts (in-memory):
```rust
// assert: push to Vec
USER_FACTS.lock().unwrap().push((4, "Dave".into(), "dave@example.com".into(), true));

// retract: retain where not matching
USER_FACTS.lock().unwrap().retain(|row| row.0 != 3);
```

---

## Part 3: Transactions

This is the hard part.

### The Core Question

When you write:

```runa
retract User(3, "Charlie", "charlie@example.com", False)
assert User(3, "Charlie", "charlie@example.com", True)
```

...and the program crashes between the two lines, what state is the database in? Charlie is deleted. The re-insert never happened. Data lost.

SQL solves this with transactions: `BEGIN; DELETE; INSERT; COMMIT;`. Either both happen or neither does. Futuruna needs the same guarantee but without SQL.

### Design: `| scope` Becomes Transactional

The `| scope` construct already exists for lifecycle management. When a scope contains `assert` or `retract` on persisted facts, it becomes a transaction automatically:

```runa
| scope reactivate_charlie {
    retract User(3, "Charlie", "charlie@example.com", False)
    assert User(3, "Charlie", "charlie@example.com", True)
}
-- Both happen or neither. COMMIT at scope exit. ROLLBACK on error.
```

No new keyword. No `BEGIN`/`COMMIT`. The scope IS the transaction boundary. This falls out of existing syntax.

### Why Scope Works

A scope already has:
1. **A clear entry and exit** — the braces define the boundary
2. **A name** — for debugging and error messages
3. **Lifecycle semantics** — cleanup on exit (Drop in Rust)

Adding transactional semantics means:
- On scope entry: `BEGIN IMMEDIATE` (acquires write lock upfront — prevents read-modify-write races)
- On scope exit (normal): `COMMIT`
- On scope exit (error/panic/abort): `ROLLBACK` via Drop safety net

The Rust codegen already emits scope guards with Drop. Transaction rollback IS a Drop operation.

### Read-Your-Writes

Within a transaction scope, do queries see uncommitted changes?

**Yes.** SQLite deferred transactions provide this. Within the same connection, writes are visible to subsequent reads even before COMMIT. This is essential for:

```runa
| scope transfer {
    retract Account(1, "Alice", 1000)
    assert Account(1, "Alice", 800)
    retract Account(2, "Bob", 500)
    assert Account(2, "Bob", 700)

    -- Verify within the transaction:
    = alice = findall(balance, Account(1, _, balance))
    = bob = findall(balance, Account(2, _, balance))
    -- alice sees [800], bob sees [700] — the uncommitted state
}
```

If the verification fails, you can abort before commit. The scope becomes both the transaction boundary AND the verification boundary.

### Nested Transactions (Savepoints)

What happens when scopes nest?

```runa
| scope outer {
    assert Order(1, "pending")

    | scope inner {
        assert OrderItem(1, 1, "Widget", 3)
        assert OrderItem(1, 2, "Gadget", 1)
        -- If this scope fails: only OrderItems roll back
        -- The Order(1, "pending") survives
    }

    assert Order(1, "confirmed")
}
```

SQLite has `SAVEPOINT` for this. The codegen:

```rust
// | scope outer
db.execute("BEGIN TRANSACTION")?;
db.execute("INSERT INTO order_ VALUES (1, 'pending')")?;

// | scope inner
db.execute("SAVEPOINT inner")?;
db.execute("INSERT INTO order_item VALUES (1, 1, 'Widget', 3)")?;
db.execute("INSERT INTO order_item VALUES (1, 2, 'Gadget', 1)")?;
db.execute("RELEASE SAVEPOINT inner")?;  // success

db.execute("UPDATE order_ SET status = 'confirmed' WHERE id = 1")?;
db.execute("COMMIT")?;  // outer success
```

On inner failure:
```rust
db.execute("ROLLBACK TO SAVEPOINT inner")?;
// outer continues — can handle the error or propagate
```

### Scope + Verification = Transactional Proof

The killer feature: combine `| scope` (transaction) with `?` (verification) inside a transaction:

```runa
| scope bank_transfer {
    retract Account(1, "Alice", alice_balance)
    retract Account(2, "Bob", bob_balance)
    assert Account(1, "Alice", alice_balance - 200)
    assert Account(2, "Bob", bob_balance + 200)

    -- Invariant: total money is conserved
    = total = findall(b, Account(_, _, b))
    | conservation: sum_list(total) -> sum_list(total) == alice_balance + bob_balance
    ? conservation else {
        -- Invariant violated: abort the transaction
        @ print("ERROR: money not conserved, rolling back")
        abort
    }
}
```

`abort` inside a scope triggers ROLLBACK. The transaction only commits if the invariant passes. **The database checks its own constraints before committing, using the same verification system you use for everything else.**

No stored procedures. No triggers. No separate constraint language. The Futuruna you already know IS the constraint language.

### What About Errors?

Three ways a transaction scope can end:

1. **Normal exit** — COMMIT
2. **`abort`** — explicit ROLLBACK
3. **Runtime error** (division by zero, assertion failure) — ROLLBACK via Drop

```runa
| scope careful {
    assert User(99, "Test", "test@example.com", True)
    = x = 10 / 0  -- runtime error
    -- Scope exits via error → ROLLBACK
    -- User(99) was never committed
}
```

### Composability: Functions Inside Transactions

Can you call a function that does asserts?

```runa
> create_order(user_id: Int, items: List(Item)) -> Int {
    = order_id = next_id(Order)
    assert Order(order_id, user_id, "pending")
    for item in items {
        assert OrderItem(order_id, item.product_id, item.quantity)
    }
    order_id
}

| scope checkout {
    = oid = create_order(1, cart_items)
    assert Payment(oid, total, "charged")
    -- Everything commits together: order + items + payment
}
```

**Yes.** The function's asserts participate in the enclosing scope's transaction. SQLite's transaction is connection-scoped, not call-scoped. Any `assert`/`retract` executed while a transaction is active belongs to that transaction.

If there's no enclosing scope, each `assert`/`retract` is auto-committed (implicit single-statement transaction). This matches SQLite's default behavior and keeps simple code simple.

### Concurrent Transactions

Two actors asserting into the same persisted fact set:

```runa
> actor processor_a(n: Int) {
    | Process(order) -> {
        | scope atomic {
            assert Order(order.id, "processed_by_a")
        }
        n + 1
    }
}

> actor processor_b(n: Int) {
    | Process(order) -> {
        | scope atomic {
            assert Order(order.id, "processed_by_b")
        }
        n + 1
    }
}
```

SQLite serializes transactions via its internal locking. Two concurrent scopes:
1. First scope acquires the write lock, executes, commits
2. Second scope waits, then acquires, executes, commits

This is correct but serialized. For high-throughput concurrent writes, SQLite is the bottleneck. This is a known limitation — Futuruna persist is not PostgreSQL. It's embedded, application-level persistence.

For read-heavy workloads: SQLite WAL mode allows concurrent readers with one writer. The codegen should enable WAL by default:
```rust
db.execute_batch("PRAGMA journal_mode=WAL")?;
```

---

## Part 4: Watch — Connecting Persistence to Streams

### Syntax

```runa
~ order_feed = watch(Order)
~ big_orders = order_feed |> filter(|event| event.row.total > 5000)
~ big_orders | event -> {
    @ print("Large order change: " + show(event))
}
```

`watch(Type)` returns a `~ Stream(ChangeEvent(Type))` that emits whenever a persisted fact is asserted or retracted.

### Mechanism

**Not** `sqlite3_update_hook` — that callback fires immediately, even inside uncommitted transactions. If the transaction ROLLBACKs, subscribers see phantom events. Instead, watch events are generated explicitly by `assert`/`retract` codegen and **buffered** inside the TxGuard:

- **Inside a scope (transaction):** Events accumulate in `TxGuard.pending_events`. On COMMIT, all events flush to the broadcast channel. On ROLLBACK, events are discarded. Subscribers never see uncommitted data.
- **Outside a scope (auto-commit):** Events fire immediately after the implicit single-statement transaction completes. No buffering needed.

```rust
struct TxGuard {
    db: Arc<Mutex<Connection>>,
    committed: bool,
    pending_events: Vec<Box<dyn FnOnce() + Send>>,
}

impl TxGuard {
    fn commit(mut self) {
        self.db.lock().unwrap().execute("COMMIT", []).unwrap();
        self.committed = true;
        for emit in self.pending_events.drain(..) {
            emit();
        }
    }
}

impl Drop for TxGuard {
    fn drop(&mut self) {
        if !self.committed {
            self.db.lock().unwrap().execute("ROLLBACK", []).unwrap();
            self.pending_events.clear(); // discard phantom events
        }
    }
}
```

This connects the `@` boundary (SQLite writes) to the `~` system (broadcast subjects) with transactional correctness. The existing stream infrastructure handles the rest — `filter`, `map`, `scan`, `|>` all work on watch streams identically to any other stream.

### Event Shape

What does `watch` emit?

```runa
# ChangeEvent(User)(
#   op: ChangeOp,
#   row: User,
#   matched_fields: [String],
# )
# ChangeOp = Asserted | Retracted
```

```runa
~ changes = watch(User)
~ changes | event -> {
    match event.op {
        | Asserted -> @ print("New user: " + show(event.row))
        | Retracted -> @ print("Removed by " + show(event.matched_fields) + ": " + show(event.row))
    }
}
```

`matched_fields` is empty for `assert`. For `retract User(42, _, _)`, it is `["id"]`; for `retract User(_, "Ada", _)`, it is `["name"]`. Events are generated by Futuruna's assert/retract lowering, buffered with the owning transaction, flushed only after COMMIT, and discarded on rollback.

---

## Part 5: Migrations

### Schema Hashing

Every `# Type(...)` with `@ persist` gets a hash stored in a `_schema` table:

```sql
CREATE TABLE IF NOT EXISTS _schema (
    type_name TEXT PRIMARY KEY,
    field_hash TEXT NOT NULL,
    field_count INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1
);
```

On startup, the compiler compares stored hashes against current `#` definitions.

### Migration Rules

```runa
# User(id: Int, name: String, email: String, active: Bool)
@ persist User

-- Transform old shape (3 fields) → new shape (4 fields)
@ migrate User(id, name, active) -> User(id, name, name + "@migrated.com", active)
```

The `@ migrate` rule is:
1. A pattern match on the OLD tuple shape (by arity)
2. An arrow `->` to the NEW tuple shape
3. A pure function — can compute derived values

### Migration Execution

On `runa build` (or first `runa run` after schema change):

```
[persist] User: schema changed (v1 → v2)
  - User(id: Int, name: String, active: Bool)
  + User(id: Int, name: String, email: String, active: Bool)
  Applying migration rule... 1,247 rows transformed.
  Schema updated to v2.
```

If no migration rule exists for a detected change:
```
error: schema changed for User but no @ migrate rule found
  - User(id: Int, name: String, active: Bool)
  + User(id: Int, name: String, email: String, active: Bool)

  Add: @ migrate User(id, name, active) -> User(id, name, "", active)
  Or:  @ migrate User drop   (WARNING: deletes all data)
```

The compiler refuses to run with a schema mismatch and no migration. This prevents silent data corruption.

### Chained Migrations

Over time, types evolve through multiple versions:

```runa
-- Current schema
# User(id: Int, name: String, email: String, active: Bool, role: String)
@ persist User

-- Historical migrations (applied in order by arity)
@ migrate User(id, name) -> User(id, name, "", True, "user")                           -- v1→v5
@ migrate User(id, name, active) -> User(id, name, "", active, "user")                  -- v2→v5
@ migrate User(id, name, email, active) -> User(id, name, email, active, "user")        -- v4→v5
```

The compiler detects the stored arity, finds the matching migration rule, applies it. Old migrations can be removed once all deployments are past that version.

### Safe vs Unsafe Changes

**Automatic (no migration needed):**
- Add new `@ persist` type → CREATE TABLE
- Add field with `default` annotation → ALTER TABLE ADD COLUMN

**Requires `@ migrate`:**
- Add field without default (what value for existing rows?)
- Remove field (data loss — must be explicit)
- Rename field (ambiguous — is it rename or drop+add?)
- Change field type (needs conversion function)
- Reorder fields

**Destructive (requires `@ drop`):**
- Remove `@ persist` entirely → DROP TABLE

---

## Part 6: Identity and Keys

### Default: First Field Is Primary Key

```runa
# User(id: Int, name: String, email: String)
@ persist User
-- id is PRIMARY KEY (first field)
```

This is convention over configuration. It matches how 99% of tables are designed. The first field is the identity.

### Auto-Increment

```runa
# Order(id: Int auto, user_id: Int, total: Int, status: String)
@ persist Order

assert Order(1, 500, "pending")  -- id auto-assigned, skip first field
```

`auto` on the key field enables `AUTOINCREMENT`. Assert calls omit the auto field.

### Compound Keys

```runa
# OrderItem(order_id: Int, product_id: Int, quantity: Int) key(order_id, product_id)
@ persist OrderItem
```

`key(...)` overrides the default first-field convention.

### Unique Constraints

```runa
# User(id: Int, name: String, email: String unique, active: Bool)
@ persist User
```

`unique` on a field adds a UNIQUE constraint. Duplicate assert fails (and triggers ROLLBACK if inside a scope).

---

## Part 7: Query Optimization

### Index Inference

The compiler reads all rules that reference persisted facts and infers which columns need indexes:

```runa
| user_by_email(name) -> User(_, name, email, _)
-- Compiler sees: lookup on position 2 (email) → CREATE INDEX
```

```runa
| active_users(id, name) -> User(id, name, _, True)
-- Compiler sees: filter on position 3 (active) → CREATE INDEX
```

```runa
| user_orders(name, total) -> User(uid, name, _, _), Order(_, uid, total, _)
-- Compiler sees: join on User.id ↔ Order.user_id → INDEX on Order.user_id
```

No `CREATE INDEX` declarations. The rules ARE the usage patterns. The compiler generates indexes from them.

### Query Planning

`findall` with bound variables compiles to WHERE clauses:

```runa
= alice_orders = findall(total, Order(_, 1, total, _))
-- SELECT total FROM order_ WHERE user_id = 1

= big_pending = findall(id, Order(id, _, total, "pending")), total > 1000
-- SELECT id FROM order_ WHERE status = 'pending' AND total > 1000
```

Rules compile to SQL views or inline queries:

```runa
| expensive_products(name, price) -> Product(_, name, price), price > 1000
-- CREATE VIEW expensive_products AS SELECT name, price FROM product WHERE price > 1000
-- OR inline: findall compiles to SELECT with JOIN/WHERE
```

The compiler chooses between:
1. **In-memory iteration** — for non-persisted facts (current behavior)
2. **SQL query** — for persisted facts
3. **Hybrid** — for rules joining persisted + non-persisted facts

---

## Part 8: The Full Picture

A complete example using every feature:

```runa
-- Schema
# User(id: Int auto, name: String, email: String unique, active: Bool)
# Order(id: Int auto, user_id: Int, total: Int, status: String)
# OrderItem(order_id: Int, product_id: Int, quantity: Int) key(order_id, product_id)

-- Persistence
@ persist User
@ persist Order
@ persist OrderItem

-- Seed data (inserted on first run, idempotent via primary key)
| User(1, "Alice", "alice@example.com", True)
| User(2, "Bob", "bob@example.com", True)

-- Derived rules (= SQL views, computed from persisted facts)
| active_users(id, name) -> User(id, name, _, True)
| user_total(name, total) -> User(uid, name, _, True), total = sum(findall(t, Order(_, uid, t, _)))
| big_spenders(name) -> user_total(name, total), total > 10000

-- Invariants (= CHECK constraints, verified before and after transactions)
| all_totals_positive: Order(_, _, total, _) -> total > 0
| no_orphan_orders: Order(_, uid, _, _) -> User(uid, _, _, _)
? all_totals_positive
? no_orphan_orders

-- Live query (= trigger, connected to stream system)
~ order_feed = watch(Order)
~ large_orders = order_feed |> filter(|o| o.total > 5000)
~ large_orders | o -> {
    @ print("ALERT: large order " + show(o))
}

-- Transaction (= BEGIN/COMMIT, scope is the boundary)
| scope checkout {
    = oid = assert Order(1, 2500, "pending")
    assert OrderItem(oid, 101, 2)
    assert OrderItem(oid, 205, 1)

    -- Verify within transaction
    = items = findall(q, OrderItem(oid, _, q))
    | has_items: length(items) -> length(items) > 0
    ? has_items else { abort }

    -- Promote to confirmed
    retract Order(oid, _, _, "pending")
    assert Order(oid, 1, 2500, "confirmed")
}

-- Migration (for when schema evolves)
@ migrate User(id, name, active) -> User(id, name, name + "@migrated.com", active)
```

Zero SQL. Every line is a Futuruna rune you already know. The database fell out of the language.

---

## Part 9: What This Is Not

Being honest about the boundaries.

**This is not PostgreSQL.** It's embedded SQLite. Single-process. File-based. No network protocol. No concurrent multi-process writes. This is for application databases — local state, configuration, user data, event logs, embedded analytics.

**This is not an ORM.** There is no object-relational impedance mismatch because there are no objects. Facts are tuples. Rules are queries. Types are schemas. The mapping is structural, not behavioral.

**This is not automatic.** Migrations require explicit rules. Index inference covers common patterns but not all. Complex analytical queries might hit performance limits.

**This is not magic.** Under the hood, it's SQLite with generated SQL. The innovation is that the language surface hides ALL of it. You never write SQL. You never manage connections. You never think about VARCHAR vs TEXT. You write Futuruna and the database emerges.

---

## Part 10: Implementation Path

### Quick win (3 milestones → usable persistence):

| Step | Feature | What | Effort |
|------|---------|------|--------|
| M26a | `@ store Type` | Object store: struct → JSON blob, keyed by first field | Medium |
| M26c | `assert` / `retract` | New keywords → INSERT/DELETE for store, persist, and in-memory | Medium |
| M26f | `runa dump` / `runa load` | Export all data as `\|` facts, import back | Small |

This gives you persistence, mutation, and backup in three milestones. No schema migration needed (JSON flexes).

### Full relational (5 more milestones):

| Step | Feature | What | Effort |
|------|---------|------|--------|
| M26b | `@ persist Type` | Fact store: typed columns, schema hash | Medium |
| M26d | `findall` on persisted facts | SELECT with WHERE instead of const table iteration | Medium |
| M26e | Transactional `\| scope` | BEGIN IMMEDIATE / COMMIT / ROLLBACK, abort keyword | Medium |
| M26g | `@ migrate` | Schema diffing, migration rules, `@ migrate drop` | Medium |
| M26h | `watch(Type)` | Change streams (buffered in TxGuard, flushed on COMMIT) | Medium |

See `falsifiable-claims.md` for detailed per-milestone test plans (66 tests total).

---

## Part 11: Complex Nested Types — The Depth Problem

### The Question

What happens when a persisted type contains non-primitive fields?

```runa
# Item(product_id: Int, name: String, quantity: Int, price: Int)
# OrderStatus = Pending | Shipped | Delivered | Cancelled
# Order(id: Int, customer: String, items: List(Item), status: OrderStatus)
@ persist Order
```

`Order` has a `List(Item)` field and an ADT enum field. Neither maps to a single SQLite column.

### Three Possible Answers

**Option A: JSON serialization for complex fields.**
The type mapping table already says `List(T) → TEXT (JSON-serialized)`. Extend this to all non-primitive fields:

| Field type | SQLite | Representation |
|---|---|---|
| `Int`, `Float` | INTEGER, REAL | Native |
| `String` | TEXT | Native |
| `Bool` | INTEGER | 0/1 |
| `Option(T)` | nullable T | NULL for None |
| Simple ADT enum | TEXT | Variant name: `"Pending"` |
| ADT with data | TEXT | JSON: `{"Some": 42}` |
| `List(T)` | TEXT | JSON array |
| Nested struct | TEXT | JSON object |
| `Map(K,V)` | TEXT | JSON object |

The `items` column stores `[{"product_id":1,"name":"Widget","quantity":3,"price":500},...]`.

**Advantage:** Simple, works for any nesting depth, no extra tables.
**Disadvantage:** Can't query into complex fields with SQL. `findall` on `items` requires deserializing every row.

**Option B: Automatic normalization.**
The compiler creates a separate `order_item` table with foreign keys. Automatically generates JOINs for findall.

**Advantage:** Full queryability.
**Disadvantage:** Massive complexity (foreign keys, cascade, join codegen). This is ORM territory — exactly what we said we're not building.

**Option C: Two persist modes.**
Let the developer choose:

```runa
# Order(id: Int, customer: String, items: List(Item), status: OrderStatus)
@ persist Order                -- columnar: flat fields as columns, complex as JSON
@ persist Order as document    -- document: entire struct as one JSON blob
```

**Decision: Option A (JSON for complex fields) as the default.** It's honest, simple, and covers 90% of use cases. If you need queryable sub-structures, use separate persisted types with explicit relationships:

```runa
# Order(id: Int, customer: String, status: OrderStatus)
# OrderItem(order_id: Int, product_id: Int, name: String, quantity: Int, price: Int)
@ persist Order
@ persist OrderItem

-- Now you can query items directly:
| big_items(name, price) -> OrderItem(_, _, name, _, price), price > 1000
```

This is what you'd do in SQL anyway. The language doesn't hide the relational modeling — it makes it natural.

### What About Nested Structs?

```runa
# Address(street: String, city: String, zip: String)
# Customer(id: Int, name: String, address: Address)
@ persist Customer
```

The `address` column stores `{"street":"123 Main","city":"Copenhagen","zip":"1000"}`. Field access like `customer.address.city` works in Futuruna (deserialized), but NOT in SQL WHERE clauses.

If you need to query by city, flatten:
```runa
# Customer(id: Int, name: String, street: String, city: String, zip: String)
@ persist Customer
-- Now: | in_copenhagen(name) -> Customer(_, name, _, "Copenhagen", _)
```

### Querying Into Nested Fields — `json_extract`

The "flatten or accept full scans" trade-off is a false dilemma. SQLite's `json_extract()` lets rules query into JSON columns directly:

```runa
# Address(street: String, city: String, zip: String)
# Customer(id: Int, name: String, address: Address)
@ persist Customer

-- This rule queries into the nested Address:
| copenhagen_customers(name) -> Customer(_, name, addr), addr.city == "Copenhagen"
```

The compiler sees `addr.city` on a persisted type's JSON column and generates:
```sql
SELECT name FROM customer WHERE json_extract(address, '$.city') = 'Copenhagen'
```

No flattening. No extra tables. The rule stays clean. The compiler knows `address` is a JSON TEXT column of type `Address`, knows `Address` has a `city` field at path `$.city`, and emits `json_extract`.

For indexing (SQLite 3.38+):
```sql
CREATE INDEX idx_customer_city ON customer(json_extract(address, '$.city'));
```

The index inference pass (M26 wave 2) can detect rules that query into JSON fields and generate expression indexes automatically.

For `@ store` mode, this is irrelevant — queries are full-scan + Futuruna filter after deserialization, so nested access works natively with zero SQL.

---

## Part 12: Two Persistence Modes

The analysis above reveals two distinct use cases that deserve different treatment.

### Mode 1: Fact Store (default) — `@ persist Type`

Flat struct → SQLite columns. Each field becomes a column. Complex fields serialize to JSON.

```runa
# User(id: Int, name: String, email: String, active: Bool)
@ persist User

| User(1, "Alice", "alice@example.com", True)

-- Query by field:
| active_users(name) -> User(_, name, _, True)
```

- Fields are individually queryable via SQL WHERE
- Schema is strict — changes require migration
- Best for: structured data you filter/join by field
- Trade-off: flat structs only, migration overhead

### Mode 2: Object Store — `@ store Type`

Whole struct → JSON blob, keyed by first field.

```runa
# Config(key: String, settings: Map(String, String), tags: List(String))
@ store Config

assert Config("theme", map_from([["mode", "dark"], ["font", "mono"]]), ["ui", "display"])
```

SQLite table:
```sql
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    data TEXT NOT NULL    -- JSON: {"key":"theme","settings":{"mode":"dark",...},"tags":["ui","display"]}
)
```

- Entire struct serialized/deserialized as one unit
- No per-field SQL queries — findall does full scan + Futuruna filter
- **No migration needed** — JSON is schema-flexible. New fields get defaults, removed fields are ignored
- Versioned deserialization: the struct definition IS the schema, JSON adapts
- Best for: configuration, complex nested data, prototyping, rapid iteration

### Why Two Modes

| Concern | `@ persist` (fact store) | `@ store` (object store) |
|---|---|---|
| Schema changes | Migration required | Automatic (JSON flex) |
| Query by field | SQL WHERE (fast) | Full scan + filter (slow) |
| Nested types | JSON columns (opaque) | Natural (whole-object) |
| Implementation | Complex (type mapping) | Simple (serialize/deserialize) |
| Best for | Queryable structured data | Config, documents, prototypes |

The object store is dramatically simpler to implement. It might even be the right **first** implementation — get persistence working, then add the columnar mode later.

### Object Store Auto-Versioning

The struct definition IS the schema. A hash of field names + types is computed at compile time and stored in a `__store_meta` table. On startup:

1. **No entry** → first run, record hash, proceed
2. **Hash matches** → same schema, proceed silently
3. **Hash mismatch** → schema changed, apply strategy:

**Default strategy** (zero-fill): keep data, update hash. Old rows deserialize with zero values for new fields (`""`, `0`, `False`, `None`).

```runa
# Config(key: String, mode: String, theme: String)  -- added theme
@ store Config
-- Old row {"key":"app","mode":"dark"} → theme gets ""
```

**delete_on_change strategy**: export old data to `.{scope}.dump.runa`, drop table, start fresh.

```runa
# Config(key: String, mode: String, theme: String)
@ store Config delete_on_change
-- Old data → .myapp.dump.runa, table wiped
```

The dump file is valid Futuruna source (one `| Type(json)` per row), so you can inspect, edit, or re-import it.

### Full `@ store` Grammar

```
@ store TypeName                              -- auto-version, zero-fill on change
@ store TypeName delete_on_change               -- auto-version, dump on change
@ store TypeName in "scope"                   -- explicit scope (shared DB)
@ store TypeName delete_on_change in "scope"    -- both
```

Status: **implemented** (M26a).

---

## Part 13: Dump as Migration Strategy

### The Insight

Instead of complex schema-to-schema transformation rules, offer a simpler escape hatch: dump everything as Futuruna source code, change the schema, re-import.

### `runa dump`

```bash
$ runa dump app.runa
```

Output (valid Futuruna source):
```runa
-- Dump of app.db
-- Schema: User v3, Order v2

| User(1, "Alice", "alice@example.com", True)
| User(2, "Bob", "bob@example.com", True)
| User(3, "Charlie", "charlie@example.com", False)

| Order(1, 1, 2500, "confirmed")
| Order(2, 2, 800, "pending")
| Order(3, 1, 1200, "shipped")
```

This is not a special format. It's `|` facts — the same syntax you write by hand. You can:
- Version-control it (`git diff` shows exactly what changed)
- Edit it in a text editor
- Transform it with a script
- Feed it back: `runa load app.runa < dump.runa`

### Dump-Based Migration Workflow

When schema changes are complex enough that `@ migrate` rules are annoying:

```bash
# 1. Dump current data
runa dump app.runa > backup-v2.runa

# 2. Edit schema in app.runa (change # Type definitions)

# 3. Option A: Write a transform script
runa run transform.runa < backup-v2.runa > backup-v3.runa

# 3. Option B: Just re-seed (if you have seed data in source)
rm app.db
runa run app.runa    # recreates from | facts in source

# 3. Option C: Let @ migrate rules handle it
runa run app.runa    # detects schema change, applies migration
```

### `@ migrate Type drop` — The Nuclear Option

For development and prototyping, sometimes you just want to start fresh:

```runa
@ migrate User drop    -- DROP TABLE + CREATE TABLE on schema change
```

This destroys all data and recreates from seed facts. Explicit, loud, and appropriate for dev mode.

### Three Migration Strategies (Choose Per Type)

```runa
# User(id: Int, name: String, email: String, role: String)

-- Strategy 1: Explicit transform (production)
@ persist User
@ migrate User(id, name, email) -> User(id, name, email, "user")

-- Strategy 2: Dump and reload (manual migration)
@ persist User
-- No @ migrate → compiler suggests: runa dump app.runa > backup.runa

-- Strategy 3: Drop and reseed (development)
@ persist User
@ migrate User drop
```

For the object store (`@ store`), migration is usually unnecessary — JSON flexes. But `runa dump` still works for backup.

### Dump Format for Object Store

```runa
-- Dump of configs
| Config("theme", {"mode": "dark", "font": "mono"}, ["ui", "display"])
| Config("auth", {"provider": "oauth", "timeout": "30"}, ["security"])
```

Even nested JSON re-emits as valid Futuruna literals, because Futuruna already has Map and List literals.

---

## Open Questions

### Resolved

1. **`abort` semantics**: Keyword, scope-local, compiles to `break 'scope_label`. TxGuard Drop fires ROLLBACK. Compile error if used outside scope. (See falsifiable-claims.md, Claim 2.)

2. **`assert` return value**: Deferred. Use `last_insert_id()` builtin for auto-increment IDs. Assert-as-expression may come later. (See falsifiable-claims.md, Claim 9.)

3. **watch event timing**: Events buffered in TxGuard, flushed on COMMIT, discarded on ROLLBACK. Not via `sqlite3_update_hook`. (See falsifiable-claims.md, Claim 4.)

4. **Backup format**: `runa dump app.runa` outputs all persisted facts as `|` statements. Valid Futuruna source. See Part 13.

### Open

5. **How does `findall` distinguish persisted vs in-memory?** The compiler knows which types have `@ persist`. It generates different code paths per type. But what about rules that join persisted + non-persisted facts?

6. **Should `@ persist` work on ADTs with variants?** E.g., `# Shape = Circle(Float) | Rectangle(Float, Float)`. This would need a discriminator column + data column. Possible but complex. Probably: serialize the whole variant as JSON (like object store for the variant portion).

7. **What about `@ persist` across multiple files?** If `kapitel-01.runa` defines `# Paragraf(...)` and `kapitel-02.runa` also asserts `Paragraf(...)` facts, they should share the same table. This requires the import system to coordinate persistence.

8. **Bulk operations**: `assert` in a loop is N individual INSERTs. Should the compiler batch them? Detecting `for x in xs { assert ... }` and wrapping in an implicit transaction would be a useful optimization.

9. **Object store deserialization defaults**: When a JSON row is missing a field that the current struct requires, what's the zero value? `""` for String, `0` for Int, `False` for Bool, `None` for Option — but what about ADT enums? First variant? This needs a rule.

10. **`@ store` vs `@ persist` — are both needed from day one?** Object store is simpler to implement. Fact store is more powerful. Implement `@ store` first as the quick win?

11. **Blob/file persistence**: Probably not needed. SQLite can store BLOBs but it's not great at it. For files/images, store the path in the database, file on disk. Futuruna already has `@ write_file` / `read_file`. No special blob support planned.

---

## Part 14: Store DB Scoping

### The Problem

A hardcoded DB path (e.g., `.store.db`) means all programs on a machine share the same database. This fails in:
- **Multi-program environments**: `weather.runa` and `inventory.runa` trample each other's data
- **Web playground**: Multiple users' programs all writing to one DB
- **Testing**: Test runs leave artifacts that interfere with real data

### The Solution

DB name is derived from the source file stem by default, overridable with an explicit scope.

**Default** (per-program isolation):
```runa
-- In weather.runa:
# Forecast(city: String, temp: Float)
@ store Forecast
-- DB: .weather.store.db
```

**Explicit scope** (shared DB across files):
```runa
-- In kapitel-01.runa:
# Paragraf(nr: Int, tekst: String)
@ store Paragraf in "grundlov"
-- DB: .grundlov.store.db

-- In kapitel-02.runa:
# Paragraf(nr: Int, tekst: String)
@ store Paragraf in "grundlov"
-- DB: .grundlov.store.db  (same DB — shared)
```

### DB Naming Convention

| Source | Scope | DB File |
|---|---|---|
| `weather.runa` | (none) | `.weather.store.db` |
| `inventory.runa` | (none) | `.inventory.store.db` |
| `kapitel-01.runa` | `"grundlov"` | `.grundlov.store.db` |
| `tests/store_test.runa` | (none) | `.store_test.store.db` |

The dot prefix hides the file from casual `ls` while keeping it discoverable with `ls -a`.

### Playground / Embedded Use

For the web playground or embedded contexts, the host can set the scope programmatically. The compiled binary just opens whatever path is in the generated code. A playground wrapper would:
1. Generate a session-specific scope (e.g., hash of session ID)
2. Inject it via `@ store T in "session_abc123"`
3. Clean up the DB file when the session ends

### Implementation

- `RustCodegen.store_scope: Option<String>` — set from explicit `@ store T in "scope"`
- `RustCodegen.source_name: Option<String>` — set from source file stem
- Priority: explicit scope > source name > fallback `.store.db`
- Status: **implemented**
