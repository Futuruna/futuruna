# RFC: Constructor names scoped to their type

Stage: Experimental.

## Rule

Two sum types in one scope may declare the same constructor name:

```runa
# Event = Trade(id: String, amount: Int) | Closed
# AskYearEvent = Trade(id: String, amount: Int, year: Int) | Closed
```

A constructor belongs to its type. Where a name is declared by more than one
type, every use of it needs one owner, taken from the type the context
expects, or written explicitly as `Type.Constructor`:

```runa
= a: Event = Trade(id = "a", amount = 1)
= b = AskYearEvent.Trade(id = "b", amount = 2, year = 2026)
```

A name declared by one type in scope needs no context, as before.

## Resolution order

1. `Type.Constructor`, in an expression or a pattern, names its owner.
2. A pattern takes the type of the value it matches.
3. An argument takes its parameter's declared type; a field value its field's
   type; a function body, match arm, `if` branch and block result the expected
   result type; a list element the list's element type (from the context or a
   sibling element whose type is known).
4. One side of `==`, `!=`, `<`, `<=`, `>`, `>=` takes the type of the other
   side.
5. A rule argument takes the type the rule's clauses give that parameter.

An owner is never chosen by arity or by field names: adding a field to one
type must not change what code elsewhere means.

A subset declaration `# T = A.X` reuses `A`'s constructor `X`; it is the same
constructor, not a second owner.

## Diagnostics

A shared name with no owner from its context is an error that names the
owners and the explicit spellings:

| Situation | Message |
| --- | --- |
| Unannotated binding | ``constructor `Closed` belongs to `Event` and `AskYearEvent`; write `Event.Closed` or `AskYearEvent.Closed`, or give `x` a type`` |
| Argument of a callable that accepts any type | ``…; `show` accepts a value of any type, so write …`` |
| List with no element type | ``…; this list has no element type, so write …`` |
| Equality with no typed side | ``…; neither side of `==` has a known type, so write …`` |
| Pattern on a value of unknown type | ``constructor pattern `Trade` belongs to `Event` and `AskYearEvent`, and the matched value has no known type; write …`` |
| Rule whose clauses type the parameter with both owners | ``…; the clauses of rule `f` take `Event` and `AskYearEvent` here, so write …`` |
| Expected type is not an owner | ``constructor `Trade` belongs to `Event` and `AskYearEvent`, not to `Other` `` |
| `X.C` where `X` is both a qualified import and a type | ``` `X` names both a qualified import and a type, so `X.C` is ambiguous; rename the import``` |
| Otherwise | ``…; nothing here gives the value a type, so write …`` |

Fields of a shared constructor are checked against the chosen owner, and
messages name it as `Owner.Constructor`.

Types inside one inline module still declare distinct constructor names, and
an exploration cannot yet run a program whose types share a constructor name;
both are reported as errors.

## Implementation

After checking, an elaboration pass gives every shared constructor except the
first declared one an internal name, using the owner the checker chose for
each occurrence, and writes `Type.Constructor` as the constructor alone. The
interpreter, native code and the solver keep their name-keyed registries.
Every place a constructor name leaves the program maps it back to the name its
declaration wrote: displayed values and `show`, collection order, calculation
JSON (`$variant` stays the declared name in each type's position), contracts
and `runa schema`, solver models, and content hashes of declarations.

A program without shared names and without `Type.Constructor` is not
elaborated; its outputs and schema hashes are unchanged.

## Why not arity- or field-based resolution

Choosing the owner whose signature matches the arguments makes meaning depend
on the shape of unrelated declarations: adding a field to `AskYearEvent.Trade`
would silently move a `Trade(...)` elsewhere from one type to the other, or
make it ambiguous. The expected type is already what a reader uses to read
the code, so it is the only owner the language uses.
