---
feature_stage: stable
feature_stage_surfaces:
  - documented-stdlib
---

# Standard Library

Built into the compiler — no imports needed. Every function here is available in every `.runa` file. User-defined functions with the same name shadow builtins.

---

## Display

| Function | Signature | Description |
|----------|-----------|-------------|
| `show` | `a -> String` | Convert any value to its string representation |
| `print` | `String -> ()` | Print to stdout (use via `@ print(...)`) |

```runa
@ print(show(42))           -- "42"
@ print(show([1, 2, 3]))    -- "[1, 2, 3]"
```

---

## Math

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `Int -> Int` | Absolute value |
| `sqrt` | `Float -> Float` | Square root |
| `pow` | `(Float, Float) -> Float` | Exponentiation |
| `exp` | `Float -> Float` | Natural exponential (e^x) |
| `ln` | `Float -> Float` | Natural logarithm |
| `round` | `Float -> Int` | Round to nearest integer |
| `floor` | `Float -> Int` | Floor (round down) |
| `to_float` | `Int -> Float` | Convert integer to float |
| `max_int` | `(Int, Int) -> Int` | Maximum of two integers |
| `min_int` | `(Int, Int) -> Int` | Minimum of two integers |
| `max_f` | `(Float, Float) -> Float` | Maximum of two floats |
| `min_f` | `(Float, Float) -> Float` | Minimum of two floats |
| `clamp` | `(Int, Int, Int) -> Int` | Clamp value to range `[lo, hi]` |

```runa
= x = abs(-7)               -- 7
= r = sqrt(16.0)            -- 4.0
= p = pow(2.0, 10.0)        -- 1024.0
= n = round(3.7)            -- 4
= f = floor(3.7)            -- 3
= c = clamp(15, 0, 10)      -- 10
= big = max_int(3, 7)       -- 7
```

---

## String Operations

String positions and lengths use Unicode scalar values, matching Rust `char`
iteration. They are not UTF-8 byte offsets and not grapheme clusters:
`string_length("å🙂b") == 3`, `char_at("å🙂b", 1) == "🙂"`, and
`index_of("å🙂b", "b") == 2`. When `length` is applied to a `String`, it has
the same scalar-count behavior as `string_length`; for lists, `length` still
counts elements. `substring` clamps negative starts/lengths to zero and stops at
the string end. `char_at` returns `""` when the index is out of range.

| Function | Signature | Description |
|----------|-----------|-------------|
| `string_length` | `String -> Int` | Number of Unicode scalar values in string |
| `split` | `(String, String) -> List(String)` | Split by separator |
| `join` | `(List(String), String) -> String` | Join with separator |
| `trim` | `String -> String` | Remove leading/trailing whitespace |
| `contains` | `(String, String) -> Bool` | Substring test |
| `starts_with` | `(String, String) -> Bool` | Prefix test |
| `ends_with` | `(String, String) -> Bool` | Suffix test |
| `replace` | `(String, String, String) -> String` | Replace all occurrences |
| `to_upper` | `String -> String` | Convert to uppercase |
| `to_lower` | `String -> String` | Convert to lowercase |
| `substring` | `(String, Int, Int) -> String` | Extract by scalar start index and scalar length |
| `char_at` | `(String, Int) -> String` | Single Unicode scalar value by index |
| `index_of` | `(String, String) -> Int` | Find substring scalar position (-1 if absent) |
| `format_float` | `(Float, Int) -> String` | Format float with N decimal places |
| `parse_int` | `String -> Int` | Parse string to integer (0 on failure) |
| `parse_float` | `String -> Float` | Parse string to float (0.0 on failure) |
| `string_chars` | `String -> List(String)` | Explode into Unicode scalar values |

```runa
= parts = split("a,b,c", ",")         -- ["a", "b", "c"]
= joined = join(["x", "y"], "-")       -- "x-y"
= clean = trim("  hello  ")            -- "hello"
= has = contains("hello world", "world")  -- true
= upper = to_upper("hello")            -- "HELLO"
= sub = substring("the cat sat", 4, 5)  -- "cat s"
= ch = char_at("hello", 0)             -- "h"
= pos = index_of("hello", "ll")        -- 2
= fmt = format_float(3.14159, 2)        -- "3.14"
= n = parse_int("42")                  -- 42
= chars = string_chars("abc")           -- ["a", "b", "c"]
= replaced = replace("foo bar foo", "foo", "baz")  -- "baz bar baz"
```

---

## List Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `length` | `List(a) -> Int` | List length |
| `head` | `List(a) -> a` | First element |
| `tail` | `List(a) -> List(a)` | All but first |
| `push` | `(List(a), a) -> List(a)` | Append element |
| `concat` | `(List(a), List(a)) -> List(a)` | Concatenate two lists |
| `reverse` | `List(a) -> List(a)` | Reverse a list |
| `map` | `(List(a), a -> b) -> List(b)` | Transform each element |
| `filter` | `(List(a), a -> Bool) -> List(a)` | Keep elements matching predicate |
| `foldl` | `(List(a), b, (b, a) -> b) -> b` | Left fold |
| `range` | `(Int, Int) -> List(Int)` | Range `[start, end)` |

```runa
= xs = [3, 1, 4, 1, 5]
= len = length(xs)                     -- 5
= first = head(xs)                     -- 3
= rest = tail(xs)                      -- [1, 4, 1, 5]
= bigger = push(xs, 9)                 -- [3, 1, 4, 1, 5, 9]
= both = concat([1, 2], [3, 4])        -- [1, 2, 3, 4]
= rev = reverse(xs)                    -- [5, 1, 4, 1, 3]
= doubled = map(xs, |x| x * 2)        -- [6, 2, 8, 2, 10]
= big = filter(xs, |x| x > 2)         -- [3, 4, 5]
= total = foldl(xs, 0, |acc, x| acc + x)  -- 14
= nums = range(1, 5)                   -- [1, 2, 3, 4]
```

`head([])` is a runtime error. `tail([])` returns `[]`.

Out-of-range direct indexing (`xs[i]`) and `nth(xs, i)` are runtime errors.

---

## Collection Operations (Kotlin-inspired)

Higher-order operations on lists. All work in both interpreter and compiled mode.

| Function | Signature | Description |
|----------|-----------|-------------|
| `sort` | `List(a) -> List(a)` | Sort by string representation (lexicographic) |
| `sort_by` | `(List(a), a -> b) -> List(a)` | Sort by key function |
| `any` | `(List(a), a -> Bool) -> Bool` | True if any element matches |
| `all` | `(List(a), a -> Bool) -> Bool` | True if all elements match |
| `find` | `(List(a), a -> Bool) -> Option(a)` | First matching element |
| `flat_map` | `(List(a), a -> List(b)) -> List(b)` | Map then flatten |
| `zip` | `(List(a), List(b)) -> List(Pair(a, b))` | Pair elements from two lists |
| `enumerate` | `List(a) -> List(Pair(Int, a))` | Index-value pairs |
| `take_while` | `(List(a), a -> Bool) -> List(a)` | Take while predicate holds |
| `drop_while` | `(List(a), a -> Bool) -> List(a)` | Drop while predicate holds |
| `sum_list` | `List(Int) -> Int` | Sum of integer list |
| `distinct` | `List(a) -> List(a)` | Remove duplicates (preserves order) |
| `count_by` | `(List(a), a -> Bool) -> Int` | Count elements matching predicate |
| `partition` | `(List(a), a -> Bool) -> (List(a), List(a))` | Split by predicate |
| `chunked` | `(List(a), Int) -> List(List(a))` | Split into chunks of size N |
| `subscribe` | `(List(a), a -> ()) -> ()` | Iterate and apply callback |

```runa
= xs = [5, 2, 8, 1, 9, 3]

= sorted = sort(xs)                       -- [1, 2, 3, 5, 8, 9]
= words = sort(["banana", "apple", "cherry"])  -- ["apple", "banana", "cherry"]
-- Note: sort is lexicographic. sort([10, 2, 3]) gives [10, 2, 3] because "10" < "2".
-- Use sort_by for numeric sorting: sort_by([10, 2, 3], |x| x) sorts numerically.
= has_big = any(xs, |x| x > 7)            -- true
= all_pos = all(xs, |x| x > 0)            -- true
= found = find(xs, |x| x > 4)             -- Some(5)
= total = sum_list(xs)                     -- 28
= uniq = distinct([1, 2, 2, 3, 1])        -- [1, 2, 3]
= evens = count_by(xs, |x| x % 2 == 0)   -- 2
= pairs = zip(["a", "b"], [1, 2])          -- [("a", 1), ("b", 2)]
= indexed = enumerate(["x", "y", "z"])     -- [(0, "x"), (1, "y"), (2, "z")]
= prefix = take_while(xs, |x| x > 1)      -- [5, 2, 8]
= chunks = chunked([1,2,3,4,5], 2)         -- [[1,2], [3,4], [5]]
= halves = partition(xs, |x| x > 4)        -- ([5, 8, 9], [2, 1, 3])
```

---

## Tuple Accessors

| Function | Signature | Description |
|----------|-----------|-------------|
| `fst` | `(a, b) -> a` | First element of a tuple/pair |
| `snd` | `(a, b) -> b` | Second element of a tuple/pair |
| `trd` | `(a, b, c) -> c` | Third element of a tuple |

```runa
= p = (10, "hello")
= x = fst(p)          -- 10
= y = snd(p)          -- "hello"
= r = (10, "hello", true)
= z = trd(r)          -- true
```

---

## Map Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `map_new` | `() -> Map(k, v)` | Create empty map |
| `map_insert` | `(Map(k, v), k, v) -> Map(k, v)` | Return new map with key→value added |
| `map_get` | `(Map(k, v), k) -> Option(v)` | Lookup key, return Some(v) or None |
| `map_get_or` | `(Map(k, v), k, v) -> v` | Lookup key with default if missing |
| `map_contains` | `(Map(k, v), k) -> Bool` | Check if key exists |
| `map_remove` | `(Map(k, v), k) -> Map(k, v)` | Return new map without key |
| `map_keys` | `Map(k, v) -> List(k)` | All keys as a list |
| `map_values` | `Map(k, v) -> List(v)` | All values as a list |
| `map_entries` | `Map(k, v) -> List((k, v))` | All entries as list of tuples |
| `map_len` | `Map(k, v) -> Int` | Number of entries |
| `map_merge` | `(Map(k, v), Map(k, v)) -> Map(k, v)` | Merge maps (second overwrites first) |
| `map_from` | `List((k, v)) -> Map(k, v)` | Create map from list of tuples |

```runa
= m = map_new()
= m = map_insert(m, "name", "Alice")
= m = map_insert(m, "age", "30")
@ print(map_get_or(m, "name", "?"))       -- Alice
@ print(map_contains(m, "age"))           -- true
@ print(map_len(m))                       -- 2
= keys = map_keys(m)                      -- ["name", "age"]
```

---

## Set Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `set_new` | `() -> Set(a)` | Create empty set |
| `set_insert` | `(Set(a), a) -> Set(a)` | Return new set with value added (dedup) |
| `set_contains` | `(Set(a), a) -> Bool` | Check membership |
| `set_remove` | `(Set(a), a) -> Set(a)` | Return new set without value |
| `set_len` | `Set(a) -> Int` | Number of elements |
| `set_to_list` | `Set(a) -> List(a)` | Convert to list |
| `set_union` | `(Set(a), Set(a)) -> Set(a)` | Union of two sets |
| `set_intersect` | `(Set(a), Set(a)) -> Set(a)` | Intersection |
| `set_diff` | `(Set(a), Set(a)) -> Set(a)` | Difference (first minus second) |
| `set_from_list` | `List(a) -> Set(a)` | Create set from list (deduplicates) |

```runa
= s = set_from_list(["red", "green", "blue", "red"])
@ print(set_len(s))                       -- 3
@ print(set_contains(s, "green"))         -- true
= s2 = set_insert(s, "yellow")           -- 4 elements
= colors = set_to_list(s2)               -- list of all colors
```

---

## Stream Operators

These operators work on reactive streams (declared with `~`). They complement the core stream operators (`map`, `filter`, `scan`, `merge`, `zip`, etc.) documented in [streams.md](streams.md).

| Function | Signature | Description |
|----------|-----------|-------------|
| `tap` | `(Stream(a), a -> ()) -> Stream(a)` | Side-effect observation: calls fn for each element, returns stream unchanged |
| `catch` | `(Stream(a), Err -> Stream(a)) -> Stream(a)` | Error recovery: in sync mode, pass-through (no errors in Vec) |
| `first` | `Stream(a) -> a` | First element; raises `first: empty list` when empty |
| `reduce` | `(Stream(a), b, (b, a) -> b) -> b` | Terminal fold: reduce stream to a single value |
| `start_with` | `(Stream(a), a) -> Stream(a)` | Prepend a value to the front of a stream |
| `concat` | `(Stream(a), Stream(a)) -> Stream(a)` | Concatenate two streams sequentially |
| `pairwise` | `Stream(a) -> Stream((a, a))` | Emit consecutive pairs: `[1,2,3]` becomes `[(1,2),(2,3)]` |

```runa
~ nums = from_list([1, 2, 3, 4, 5])

~ observed = nums |> tap(|x| @ print("saw: " + show(x)))  -- prints each, returns stream
= head = nums |> first                                     -- 1
= total = nums |> reduce(0, |acc, x| acc + x)              -- 15
~ prefixed = nums |> start_with(0)                          -- [0, 1, 2, 3, 4, 5]
~ both = concat(from_list([1, 2]), from_list([3, 4]))       -- [1, 2, 3, 4]
~ pairs = nums |> pairwise                                  -- [(1,2), (2,3), (3,4), (4,5)]
~ firsts = pairs |> map(|p| fst(p))                         -- [1, 2, 3, 4]
```

---

## Option / Result

| Function | Signature | Description |
|----------|-----------|-------------|
| `unwrap_or` | `(Option(a), a) -> a` | Unwrap with default |
| `is_some` | `Option(a) -> Bool` | True if Some |
| `is_none` | `Option(a) -> Bool` | True if None |

```runa
= val = unwrap_or(Some(42), 0)     -- 42
= val2 = unwrap_or(None, 0)        -- 0
= check = is_some(Some("hi"))      -- true
```

For error propagation, use monadic bind:

```runa
> safe_divide(a: Int, b: Int) -> Result(Int, String) {
    if b == 0 { Err("division by zero") }
    else { Ok(a / b) }
}

> compute() -> Result(Int, String) {
    = x <- safe_divide(10, 2)    -- unwraps Ok, early-returns Err
    = y <- safe_divide(x, 0)     -- early-returns Err("division by zero")
    Ok(x + y)
}
```

---

## Logic

| Function | Signature | Description |
|----------|-----------|-------------|
| `not` | `Bool -> Bool` | Logical NOT |
| `assert` | `Bool -> ()` | Runtime assertion (panics on false) |
| `identity` | `a -> a` | Identity function |

```runa
= flag = not(true)          -- false
assert(2 + 2 == 4)          -- passes
= x = identity(42)          -- 42
```

---

## File I/O

File operations are invoked with the `@` rune (effect boundary).

| Function | Signature | Description |
|----------|-----------|-------------|
| `read_file` | `String -> String` | Read entire file contents |
| `write_file` | `(String, String) -> ()` | Write/overwrite file |
| `append_file` | `(String, String) -> ()` | Append to file |
| `file_exists` | `String -> Bool` | Check if file exists |
| `read_lines` | `String -> List(String)` | Read file as list of lines |
| `env_var` | `String -> String` | Read environment variable |

```runa
@ write_file("output.txt", "hello world")
= exists = file_exists("output.txt")       -- true
= content = read_file("output.txt")        -- "hello world"
= lines = read_lines("output.txt")         -- ["hello world"]
@ append_file("output.txt", "\nline 2")
= home = env_var("HOME")                   -- "/Users/..."
```

---

## Process Execution

Process execution is argv-based, not shell-based.

| Function | Signature | Description |
|----------|-----------|-------------|
| `process_run` | `List(String) -> (Int, String, String)` | Run argv without a shell, returning exit code, stdout, stderr |

```runa
= result = process_run(["git", "status", "--short"])
= code = result.0
= out = result.1
= err = result.2
```

`process_run` does not invoke a shell. Pass the executable and each argument as separate list elements.

---

## JSON

JSON values are represented as `String` (serialized JSON text). Auto-adds `serde_json` dependency on first use.

| Function | Signature | Description |
|----------|-----------|-------------|
| `json_parse` | `String -> String` | Validate and return JSON string |
| `json_get` | `(String, String) -> String` | Access object field (returns JSON text) |
| `json_string` | `String -> String` | Extract string value (unquoted) |
| `json_number` | `String -> Float` | Extract number |
| `json_bool` | `String -> Bool` | Extract boolean |
| `json_array` | `String -> List(String)` | Extract array elements as JSON strings |
| `json_emit` | `String -> String` | Pass through (identity for JSON) |
| `json_object` | `List(List(String)) -> String` | Build JSON from key-value pairs |

```runa
= raw = "{\"name\": \"Alice\", \"age\": 30, \"tags\": [\"dev\", \"runa\"]}"
= parsed = json_parse(raw)
= name = json_string(json_get(parsed, "name"))    -- "Alice"
= age = json_number(json_get(parsed, "age"))       -- 30.0
= tags = json_array(json_get(parsed, "tags"))       -- ["\"dev\"", "\"runa\""]

-- Build JSON
= obj = json_object([["city", "\"Copenhagen\""], ["temp", "22"]])
-- {"city":"Copenhagen","temp":22}
```

---

## HTTP

HTTP client and server. Auto-adds `ureq` (client) and `tiny_http` (server) dependencies.

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_get` | `String -> String` | GET request, return response body |
| `http_post` | `(String, String) -> String` | POST with body, return response |
| `http_serve` | `(Int, (String, String, String) -> (Int, String, String)) -> ()` | Start HTTP server |
| `http_respond` | `(Int, String, String) -> (Int, String, String)` | Build response tuple |
| `http_request_path` | `Request -> String` | Extract request URL path |
| `http_request_method` | `Request -> String` | Extract HTTP method |
| `http_request_body` | `Request -> String` | Extract request body |

### Client

```runa
= body = http_get("https://httpbin.org/get")
@ print(body)

= response = http_post("https://httpbin.org/post", "{\"key\": \"value\"}")
@ print(response)
```

### Server

```runa
@ http_serve(8080, |path, method, body| {
    if path == "/hello" {
        http_respond(200, "text/plain", "Hello from Futuruna!")
    } else {
        http_respond(404, "text/plain", "Not found")
    }
})
```

The handler receives three string arguments: request path, HTTP method, and request body. Return a response tuple via `http_respond(status, content_type, body)`.

---

## Database (SQLite)

SQLite access via `rusqlite`. Auto-adds dependency. Connection is thread-safe (`Arc<Mutex<Connection>>`).

| Function | Signature | Description |
|----------|-----------|-------------|
| `db_open` | `String -> Db` | Open SQLite database (`:memory:` for in-memory) |
| `db_exec` | `(Db, String) -> ()` | Execute DDL/DML (CREATE, INSERT, UPDATE, DELETE) |
| `db_query` | `(Db, String) -> List(List(String))` | Query all rows |
| `db_query_row` | `(Db, String) -> List(String)` | Query single row |
| `db_insert` | `(Db, String) -> Int` | Insert and return last row ID |
| `db_close` | `Db -> ()` | Close database connection |

```runa
= db = db_open(":memory:")

@ db_exec(db, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
@ db_exec(db, "INSERT INTO users (name, age) VALUES ('Alice', 30)")
@ db_exec(db, "INSERT INTO users (name, age) VALUES ('Bob', 25)")

= rows = db_query(db, "SELECT name, age FROM users")
for row in rows {
    @ print(show(row))    -- ["Alice", "30"], ["Bob", "25"]
}

= one = db_query_row(db, "SELECT name FROM users WHERE age = 30")
@ print(show(one))        -- ["Alice"]

@ db_close(db)
```

---

## Concurrency

| Function | Signature | Description |
|----------|-----------|-------------|
| `spawn` | `(Actor, a) -> ActorHandle` | Create actor with initial state |
| `ask` | `(ActorHandle, Msg) -> a` | Send message, get response |
| `shared` | `a -> shared(a)` | Wrap value in `Arc` for thread-safe sharing |

Actors are defined with `> actor`, messages sent with `<-`:

```runa
> actor counter(state: Int) {
    | Increment -> state + 1
    | Decrement -> state - 1
    | Reset -> 0
}

= c = spawn(counter, 0)
c <- Increment
c <- Increment
= val = ask(c, Increment)
@ print(show(val))            -- 3
```

---

## Comptime

| Function | Signature | Description |
|----------|-----------|-------------|
| `struct_type` | `(String, List(Field)) -> TypeDef` | Generate struct type at compile time |
| `enum_type` | `(String, List(String)) -> TypeDef` | Generate enum type at compile time |
| `field` | `(String, String) -> Field` | Build a field descriptor |

```runa
@ comptime = MyPoint = struct_type("MyPoint", [field("x", "Float"), field("y", "Float")])
@ comptime = Color = enum_type("Color", ["Red", "Green", "Blue"])
```

These are comptime-only functions — they generate real Rust types (structs/enums) at compile time.
