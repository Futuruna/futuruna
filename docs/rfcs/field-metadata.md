---
feature_stage: experimental
feature_stage_surfaces:
  - calculation-field-declarations
---

# RFC: Field metadata on declarations

Status: Experimental

## Summary

A calculation's input fields carry interview metadata: a label, a question,
help, a unit, sources, and the entry that provides the field. Until now a model
wrote that metadata in one list per calculation, keyed by `pathof(...)`, away
from the fields it describes. A type used at several paths repeated the same
label at every path, and moving a field meant editing two places.

`@ field(...)` writes the metadata where the field is declared:

```runa
# AskOwner(
    @ field(label = "Fully tax liable in Denmark", help = "Fuld skattepligt efter kildeskattelovens § 1.")
    fully_tax_liable_in_denmark: Bool
)

@ field(label = "Buy or sell")
# Side = BuySide | SellSide

# Date(
    @ field(label = "year") year: Int,
    @ field(label = "month") month: Int,
    @ field(label = "day") day: Int
)

# Trade(@ field(label = "Trade date") date: Date, @ field(label = "Event id") id: String)
```

`runa schema` emits the same `field_metadata` contract as before. For an input
holding `trades: List(Trade)`, the declarations above give
`trades.date.year` the label `Trade date: year` and `trades.id` the label
`Event id`, wherever `Trade` is used.

## Goals

- Say each fact once, next to the field it describes: a type used at ten paths
  is labelled once.
- Keep the contract unchanged: the same `field_metadata` entries, paths and
  wire format, so SDKs, workbooks and needs read declared metadata exactly as
  they read listed metadata.
- Keep one precedence rule that a reader can apply by hand.

## Non-goals

- Changing the contract format or the schema fingerprint scheme.
- Replacing the anchored metadata list. It remains the way to write metadata
  that depends on where a type is used, metadata for one calculation entry,
  metadata kept in a separate module, and groups of fields that share sources.
- Deriving labels from field names. A field without metadata keeps the
  deterministic humanized fallback.

## Syntax

`@ field(...)` is a declaration annotation like `@ calculate(...)`: written on
the line before (or directly before) the thing it describes.

- **On a field.** Before a named field inside `# Type(...)` or inside a variant
  payload of `# Type = A(...) | B(...)`. It describes that field wherever the
  type occurs in a calculation input.
- **On a type.** Before a `#` data type declaration. It describes every input
  field whose value has that type: the field of an enum type, the `$variant`
  column of a sum type with payloads, the collection or optional column of a
  record type. List, set, optional and map wrappers are transparent, as in
  `pathof`.

The arguments are named:

| Key | Value | Contract |
|---|---|---|
| `label` | required `String` | `label` |
| `question` | `String` | `question` |
| `help` | `String` | `help` |
| `unit` | `String` | `unit` |
| `sources` | list of source values, such as `[Source(value = law)]` | `sources` |
| `entry`, `subject` | `String`, given together | `provided_by` |

Values are plain strings, not `Some(...)`: a key that does not apply is left
out. A value may be any pure ground expression, resolved like a metadata
binding in the program's flat import scope. A `sources` item is a value of a
sum type that implements `MetaRole`; its variant names the role and its sole
`value` field is the source, exactly as in anchored metadata attachments.

### Why this syntax

Futuruna has two annotation forms: `@ name` before a declaration (`@ export`,
`@ comptime`, `@ calculate("title")`) and `--@label:...::meta:binding--`
comments that attach a typed metadata value to a label. The comment form names
its target by a string label and lives outside the code it describes; that is
the distance this RFC removes. `@ calculate("title")` already labels the
declaration that follows it, so `@ field(label = "...")` labels the field that
follows it, with the same reading order and no new punctuation. The keys are
the contract's own field names, so the source and `runa schema` read alike.

The metadata needs no declared record type (`CalculationField`,
`CalculationMeta`, `impl Meta`) and no `path`: the position is the path.

## Precedence

For each input path, the most specific metadata wins, whole:

1. **Exact path for one calculation:** anchored metadata whose `path` is a
   string, `pathof(...)`, or `refof(...)` rooted at the calculation's input
   type.
2. **A type's field, wherever the type occurs:** `@ field` on a field, or
   anchored `refof(Type::field)` metadata.
3. **A type's default:** `@ field` on a type.

The winner supplies every key; keys are not merged across levels, so the
contract entry for a path comes from exactly one declaration, named by its
`anchor` and `binding`. Two declarations at the winning level for one path are
an error naming both. `@ field` on a field and `refof(Type::field)` for the
same field are the same level and therefore conflict.

### Labels of record fields

A required record field has no column of its own: its members are spread into
columns. Metadata may name such a field (with `@ field` or an exact path); its
label is not emitted as an entry but gives context to its members. A label
written for a type or a type's field (levels 2 and 3) does not know where the
type is used, so below a labelled record field it reads
`<record field label>: <member label>`, nesting outward. A label written for an
exact path (level 1) already names its context and stays as written.

```runa
# Date(@ field(label = "year") year: Int, @ field(label = "month") month: Int, @ field(label = "day") day: Int)
# Payment(@ field(label = "Payment date") paid: Date)
```

`paid.year` is labelled `Payment date: year`. Only the label composes; a record
field's help, unit and other keys describe no column and are not emitted.

## Diagnostics

`runa check` reports, at the declaration:

- an unknown key, with the list of keys;
- a key given twice, an unnamed argument, a missing `label`, `entry` without
  `subject` or the reverse;
- two `@ field` on one field or one type (conflicting declarations at the same
  level);
- `@ field` that is not followed by a field or a data type, on a positional
  field, on type parameters, an opaque type, a rule-scope type or a `WHEN`
  type evolution;
- a value whose names do not resolve, a text key whose value is not a
  `String`, and `sources` that is not a list.

`runa schema`, `template` and `call` additionally report a value that is not
pure ground data, a `sources` item that is not a `MetaRole` value, two
declarations at the winning level for one path, and, for both forms, a
provided field whose `subject` is not an input path holding the providing
entry's input type. Metadata for a field that does not exist cannot be written
with `@ field`; anchored exact paths keep their unknown-path errors.

## Contract equivalence

Declared metadata produces the same `field_metadata` entries as the same
metadata listed by path: `path`, `label`, `question`, `help`, `unit`,
`sources` (role, binding, type, value, data) and `provided_by` are equal.
Provenance differs, because the metadata is written somewhere else:

- `anchor` is the declaring type, `binding` is `Type`, `Type::field` or
  `Type::Variant::field`;
- a source's `label` is the declaring type and its `attachment_path` is
  `<binding>.sources[i]`;
- the calculation's `metadata` list no longer contains the removed metadata
  list binding.

The schema fingerprint covers provenance, so it changes once when a model moves
its metadata. `tests/calculate_cli.rs` checks a model in both forms for equal
field metadata.

Declarations are presentation: they are not part of a type's content hash, its
semantic interface or its runtime behaviour.

## Alternatives considered

- **Replace the anchored list entirely.** The list remains necessary for
  metadata that depends on where a type is used (for example help that applies
  to one calculation only), for metadata in a separate module, and for groups
  of questions sharing sources. Keeping it as the exact-path level gives one
  precedence rule instead of a second override syntax.
- **Merge keys across levels.** An exact path could add `help` to a declared
  `label`. Rejected: a reader would have to look in several places to know a
  field's metadata, and the contract's `binding` could no longer name its
  source.
- **Doc comments as labels.** Comments describe the model for its authors;
  interview text is a product surface with its own wording and translation.
