---
feature_stage: experimental
---

# Unit types for whole numbers

Status: Experimental, in progress.

## Problem

Amounts in rule models are `Int` with the unit in the field name
(`opening_cash_ore`, `quantity_micro`) and again in field metadata
(`unit = Some("DKK øre")`). Nothing stops a model from adding øre to
millionths of a share, and a contract consumer sees a bare number.

## Declaration

```runa
# unit Ore "DKK øre"
# unit Micro "millionths of a share or unit"
```

`# unit Name "description"` declares a unit of measure for whole numbers. The
description is what the unit counts; it is written once and the calculation
contract repeats it wherever a field has the unit. `unit` is contextual: it
stays an ordinary name everywhere else (a `CalculationField` keeps its `unit`
member). A unit may not reuse a built-in type name or the name of a declared
type or constructor.

Units exist for `Int` only. The models that need them count exact amounts
(øre, millionths, basis points); a `Float` unit would also need rules for
mixing with `Int` and rounding. It can be added later without changing this
design.

## Rules

For a unit `U`, another unit `V` and a plain `Int` value `n`:

| Expression | Type |
|---|---|
| `U + U`, `U - U`, `U % U`, `-U` | `U` |
| `U * n`, `n * U`, `U / n`, `U % n` | `U` |
| `U / U` | `Int` (a ratio) |
| `U` compared with `U` | `Bool` |
| `U * U`, `n / U`, `n % U` | error: no squared or inverse units |
| `U` with `V` in any operator | error |
| `U + n`, `U - n`, `U` compared with `n` | error |

A number written in the source (`0`, `-5`, `100 * 100`) has no unit of its
own: it takes the unit its place requires. `amount > 0`, `amount + 100`, an
`Ore` field given `0`, a function returning `0` for `-> Ore` and
`if c { 0 } else { amount }` need no conversion. This is the only implicit
step, and it cannot mix two quantities: the mistake units catch is a named
value of one kind used as another, and a literal is never that.

`Ore(n)` gives a plain `Int` the unit; `Int(amount)` takes it off. Both leave
the value unchanged. A rule that changes unit is a function that uses them, so
the rule is written down once:

```runa
> market_value(quantity: Micro, price: Ore) -> Ore {
    price * (Int(quantity) / 1000000) + price * (Int(quantity) % 1000000) / 1000000
}
```

`abs`, `min_int`, `max_int`, `clamp`, `sum` and `sum_list` keep the unit of
their arguments; every argument has the same unit.

Like the rest of the checker, the rules judge operands whose types are known.
An operand of unknown type is not evidence of a mistake.

## Runtime, wire and proofs

A unit value is an `Int` everywhere outside the checker:

- the interpreter stores the number; `Ore(n)` and `Int(x)` return their
  argument, and a parameter or rule head typed `Ore` accepts an `Int` value;
- `runa call` reads and writes the same JSON numbers;
- `runa verify` and Rust generation erase units before encoding
  (`erase_unit_types`): unit types become `Int` and conversions become their
  argument, so the solver and the generated Rust see exactly the checked `Int`
  arithmetic the interpreter runs. Generated Rust also declares
  `pub type Ore = i64;`.

## Contract

`runa schema` marks a unit-typed field additively: its type stays
`{"kind": "primitive", "name": "Int"}` and gains `"unit": "Ore"`. The contract
lists the units its types use with their descriptions:

```json
"units": [{"name": "Ore", "description": "DKK øre"}]
```

A field metadata entry of a unit-typed field takes its `unit` text from the
declaration. Writing `unit = Some(...)` for such a field would say the unit a
second time and is an error. Consumers that ignore unknown members read the
same contract as before; `runa sdk ts` brands the field type from `unit`
(`Ore` is `number & { readonly __unit: "Ore" }`).
