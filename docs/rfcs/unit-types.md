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
# unit Ore "DKK øre" = DKK / 100
# unit MicroDkk "millionths of a krone" = DKK / 1000000
# unit Micro "millionths of a share or unit" = 1 / 1000000
# unit Bp "basis points"
```

`# unit Name "description"` declares a unit of measure for whole numbers. The
description is what the unit counts; it is written once and the calculation
contract repeats it wherever a field has the unit. `unit` is contextual: it
stays an ordinary name everywhere else (a `CalculationField` keeps its `unit`
member). A unit may not reuse a built-in type name or the name of a declared
type or constructor.

`= Base / n` declares the unit's scale: `n` of the unit make one `Base`. The
base is a name of its own (a currency code such as `DKK`, or another quantity)
or `1` for a plain number; `n` is a whole number of at least 1, and `= Base`
alone means `= Base / 1`. The base is never a declared type, constructor or
unit: every unit of one base says how many of it make one base, so no unit is
defined through another and a scale is read off one line. A unit without a
scale (`Bp` above) is a quantity of its own and converts into nothing.

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
| `List(U) == List(V)`, `Some(u) == Some(v)` | error: the units inside differ |

A number written in the source (`0`, `-5`, `100 * 100`) has no unit of its
own: it takes the unit its place requires. `amount > 0`, `amount + 100`, an
`Ore` field given `0`, a function returning `0` for `-> Ore`,
`if c { 0 } else { amount }` and `[a] == [0]` need no conversion. This is the
only implicit step, and it cannot mix two quantities: the mistake units catch
is a named value of one kind used as another, and a literal is never that.

`abs`, `min_int`, `max_int`, `clamp`, `sum` and `sum_list` keep the unit of
their arguments; every argument has the same unit.

A callback's untyped parameters have the types the call gives them, so its
body is judged as any other body:

- the element parameter of `map`, `filter`, `foldl` and the other collection
  functions has the collection's element type;
- `foldl`'s accumulator has the type of the start value. A start written as a
  number has the unit its step brings in (`foldl(xs, 0, |t, x| t + x.amount)`
  sums `Ore`, also when the step is `if c { t + x.amount } else { t }`), and
  is otherwise a plain `Int`, so `foldl(xs, 0, |t, x| t + x.amount +
  x.quantity)` adds `Ore` to a plain `Int` and is an error;
- a closure bound to a name (`= f = |x| x + a`) is judged again at each call
  with the types of its arguments, so `f(quantity)` adds `Micro` to `Ore` and
  is an error. An argument written as a number gives its parameter no type.

Like the rest of the checker, the rules judge operands whose types are known.
An operand of unknown type is not evidence of a mistake.

## Conversions

`Ore(n)` gives a plain `Int` the unit; `Int(amount)` takes it off. Both leave
the value unchanged; neither converts between units.

Two units of one base convert by the ratio of their scales, when one is a
whole multiple of the other. The conversion names the unit it converts to;
the unit of the amount is its type:

| Conversion | `MicroDkk` amount `m`, `Ore` amount `o` |
|---|---|
| `to(MicroDkk, o)` | `o * 10000`, exact |
| `to_nearest(Ore, m)` | `m / 10000`, halves away from zero |
| `to_floor(Ore, m)` | `m / 10000`, toward negative infinity |
| `to_ceiling(Ore, m)` | `m / 10000`, toward positive infinity |
| `to_toward_zero(Ore, m)` | `m / 10000`, toward zero |

A conversion that divides states its rounding in its name, and one that
multiplies has none to state, so a rounding is never silent and never says
more than happens: `to(Ore, m)` is an error that lists the four roundings, and
`to_nearest(MicroDkk, o)` is an error that asks for `to`. Converting a value to
its own unit, between bases, between units whose scales are not whole multiples
of each other (`DKK / 3` and `DKK / 2`), from a unit without a scale or from a
number written in the source is an error. A program that declares its own
function `to` (or `to_nearest`, ...) keeps it.

A conversion between units that are not one base's (shares and kroner, say)
is a rule of the model, written once in a function that uses `Int(...)` and
the unit:

```runa
> market_value(quantity: Micro, price: Ore) -> Ore {
    price * (Int(quantity) / 1000000) + price * (Int(quantity) % 1000000) / 1000000
}
```

The checker records each conversion with its factor and rounding. Before the
program runs, elaboration (the step that also gives shared constructors their
owners) writes each as the `Int` arithmetic it stands for, keeping its source
position: `to(MicroDkk, o)` is `MicroDkk(Int(o) * 10000)`, and
`to_nearest(Ore, m)` divides with quotient and remainder, so no intermediate
exceeds the amount, and moves the quotient one away from zero when the
remainder is at least half the factor. The interpreter, `runa call`,
`runa verify` and generated Rust therefore run one definition of each
rounding, and the solver proves properties of it like any other arithmetic.

## Runtime, wire and proofs

A unit value is an `Int` everywhere outside the checker:

- the interpreter stores the number; `Ore(n)` and `Int(x)` return their
  argument, and a parameter or rule head typed `Ore` accepts an `Int` value;
- `runa call` reads and writes the same JSON numbers;
- `runa verify` and Rust generation erase units before encoding
  (`erase_unit_types`): unit types become `Int` and `Ore(n)`/`Int(x)` become
  their argument, so the solver and the generated Rust see exactly the checked
  `Int` arithmetic the interpreter runs. Generated Rust also declares
  `pub type Ore = i64;`.

## Contract

`runa schema` marks a unit-typed field additively: its type stays
`{"kind": "primitive", "name": "Int"}` and gains `"unit": "Ore"`. The contract
lists the units its types use with their descriptions and, for a unit that
declares one, its scale:

```json
"units": [
  {"name": "Micro", "description": "millionths of a share or unit", "scale": {"of": "1", "per": 1000000}},
  {"name": "Ore", "description": "DKK øre", "scale": {"of": "DKK", "per": 100}}
]
```

`per` of the unit make one `of`; `of` is the base as declared, `"1"` for a
plain number. A consumer converts a unit value to its base by dividing by
`per` exactly (an `Ore` amount of `12345` is `123.45` DKK) and between two
units of one base by the ratio of their `per`. A unit without a scale has no
`scale` member.

A field metadata entry of a unit-typed field takes its `unit` text from the
declaration. Writing `unit = Some(...)` for such a field would say the unit a
second time and is an error. Consumers that ignore unknown members read the
same contract as before; `runa sdk ts` brands the field type from `unit`
(`Ore` is `number & { readonly __unit: "Ore" }`).
