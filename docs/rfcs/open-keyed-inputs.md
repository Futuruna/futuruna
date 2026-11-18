---
feature_stage: experimental
feature_stage_surfaces:
  - calculation-needs
---

# RFC: Open keyed inputs

Status: Experimental (Futuruna 0.7)

## Problem

The law asks for facts whose key it computes. Aktiesparekontoloven § 6 deems
the assets distributed "the first day after" a delisting, or "20 February the
year after" a change of status: the account's balance on a day the law
derives. The integrator cannot know which days to supply before the law has
asked.

In 0.6 a keyed list (`balances: List(Balance)`, `Balance` keyed by `day`) is
read like any list. An element the case gives with unknown fields becomes a
need with its key (`balances[1].value`, keyed `day = "2027-02-20"`), but an
element the case does not give is simply absent: `find` or `filter` over the
list gives `None` or an empty list, a silent `0`, and no need. Nothing tells
the integrator that the law wanted 20 February.

## Design

### Declaring an open collection

A record field of type `List(T)`, where `T` declares a key field
(`@ field(key = True)`), is declared open on the field:

```runa
# Balance(@ field(key = True, label = "Day") day: String, @ field(label = "Value") value: Int)
# Case(@ field(label = "Delisted on") delisted: String,
       @ field(label = "Balances", open = True) balances: List(Balance))
```

`open` is a marker like `key`: `@ field(open = True)` alone needs no label and
keeps the field the metadata of its type. The key identifies an entry; the
field says the case never gives the collection whole. Openness belongs to the
field, not the type: the same `Balance` may be closed elsewhere (a statement
that lists every balance it has).

`runa check` holds it: `open` is written `True` or `False`, on a field (not a
type), whose type is a `List` of a type with a key field.

### Reading it: `lookup`

```runa
| deemed(c: Case) -> Int { lookup(c.balances, c.delisted).value }
```

`lookup(collection, key)` is the entry of a keyed list whose key is `key`. Its
type says what is true:

- on an open field (`lookup(c.balances, day)`, the field read written as its
  first argument) it is the entry, `Balance`: the entry exists in the law's
  world, and the integrator gives it;
- on any other keyed list it is `Option(Balance)`: `Some(entry)` or `None`,
  as `find` by the key would give.

The key has the key field's type. A list whose element type declares no key
field is a check error.

What `lookup` does on an open collection:

| The case | Partial (`--partial`, `query`, `answer`) | Complete |
|---|---|---|
| gives the entry | the entry | the entry |
| gives an entry whose key is unknown, before it | unknown, with that key's path | — |
| does not give it | unknown, a need at `balances[day="2027-02-20"]` | an error naming the field and the key |

A missing entry is also recorded as *fallible*: a case that stays without it
fails in the complete calculation, so a partial case never returns `Complete`
past it, even when the entry's value is never read.

An open field may be left out of a case in every mode: it then has no entries
given yet. It is never unknown as a whole.

### No whole-collection reads

The case never holds all of an open collection, so no read of it as a whole
has a true result. `runa check` refuses every read of an open field except as
the first argument of `lookup`: `filter(c.balances, …)`, `length(c.balances)`,
`for b in c.balances`, binding `= bs = c.balances`, and passing it to a
function all are check errors naming the field and `lookup`. Where the
receiver's type is not known, a field of that name is refused too.

Refusing at check time, rather than making the read unknown at run time, says
it where the law is written: a whole-collection read could only ever be
unknown, so it is a mistake, not a need.

### The need

```json
{ "kind": "fact",
  "path": "balances[day=\"2027-02-20\"]",
  "keys": [ { "path": "balances[day=\"2027-02-20\"]", "field": "day", "value": "2027-02-20" } ],
  "label": "Balances",
  "waiting": [ { "rule": "value_on" } ] }
```

- `path` names the entry: the collection's path, then `[field=value]` with the
  key field and its value as JSON (a string quoted and escaped, an `Int` as
  its digits). Its metadata is the collection field's (`balances`).
- `keys` carries the key as for any element a path lies in: here the entry
  itself. An entry of an open collection inside a keyed element carries both.
- The answer is the whole entry, a `Balance`.

A path's grammar is: fields joined by `.`, then per step `[index]` or
`[field=JSON]`.

### Writing the answer

- **`fill` and `answer` / `from_case`**: `fill(partial, complete, paths)` at
  `balances[day="2027-02-20"]` takes the entry with that key from `complete`
  and adds it to the partial case's collection (or replaces the entry with
  that key). The entry is answered whole: the need asked for the entry.
- **The TypeScript SDK**: the need's pattern is `balances[]` (`[]` stands for a
  list index or a key), its answer the element type, so
  `given(need, { day: "2027-02-20", value: 1740 })` writes a whole new entry.
  `setAt` adds it to the collection, setting the key from the path when the
  value leaves it out and throwing a `PathError` when the value gives another
  key. `<entry>InputPatterns` has `balances[]` and the entries' fields
  (`balances[].value`), never `balances`: an open collection is not asked
  whole. The field is optional in the input type.

```ts
const resolve: DeemedResolve = async (needs) => {
  const days = needs.filter((n) => n.pattern === "balances[]").map((n) => n.keys?.[0]?.value);
  const balances = await bank.balancesOn(account, days);
  return (need) => {
    switch (need.pattern) {
      case "balances[]": {
        const day = need.keys?.[0]?.value;
        const value = day === undefined ? undefined : balances.get(day);
        return value === undefined ? open : given(need, { day, value });
      }
      default:
        return open;
    }
  };
};
```

### Interactions

- **Kleene propagation.** `lookup` needs its key whole (an unknown key gives an
  unknown result with the key's paths) and the list's structure. It scans the
  entries' keys as `find` scans answers: an entry whose key is unknown, before
  the match or with no match, makes the result unknown with that key's path,
  since it may be the entry; in an open collection those paths are fallible.
  Strict mode stops at the first unknown key or the missing entry.
- **query / answer / from_case.** A query's case is encoded with the entries it
  holds; the need's path is a `Path(i)` like any other and `fill` writes the
  entry.
- **The completion test** runs only on `Complete` results, which a missing
  entry never gives (it is fallible).
- **The clause audit and solvers** see `lookup` as an uninterpreted call: they
  report what they cannot translate, as for other interpreter-only builtins.
- **Native code.** `lookup` runs in the interpreter, as `query` does:
  `runa build` and `runa run` refuse it with an error at the call, `runa check`
  skips Rust generation for a program that calls it, and `runa test --run` runs
  its tests in the interpreter. Native code takes complete input, and an open
  collection is never complete.

## Alternatives

- **`find` by the key field** (`find(c.balances, |b| b.day == day)`). It would
  need the engine to recognise a predicate as a key comparison; a predicate
  that compares anything else could not become a need with a key, and the
  closure says the key a second time. `lookup` says it once.
- **A `Map(Day, Int)` input.** Map keys are paths already, but the entry would
  lose its record (a day's balance and its currency) and the existing
  `@ field(key = True)`, which needs and the SDK use for list elements. One way
  to say keyed entries is kept.
- **A type (`Open(Balance)`) instead of a marker.** It would make openness
  travel through function parameters, but every pass that knows `List` would
  need a second collection type, and the type would claim openness everywhere
  the record is used. Openness is a property of how the case gives the field.
- **Whole-collection reads as unknown at run time.** Any such read could only
  give an unknown result, never answerable; a check error says so where the law
  is written.
- **`Option(T)` from `lookup` on an open collection.** Its `None` could never
  happen, so a law would carry a branch that claims something false.

## Open questions

- **An entry that does not exist.** A day without a balance is answered with an
  entry (a `0`, or an optional field). An open collection whose entries may
  truly be absent ("the price on a day the security was not listed") has no
  answer "there is none" yet.
- **Reads through a whole record.** `show(c)`, `c == d` and a record pattern
  binding the open field read the collection as the case gives it; only field
  reads are refused.
- **`pathof` of an entry.** `pathof(Case::balances[day = "…"])` is not written
  yet; a test compares an entry need by its `keys`.
- **Two entries with one key.** `lookup` takes the first; the input is not
  refused.
- **Maps.** An open `Map` input could follow the same marker if a law needs
  keyed values without a record.
