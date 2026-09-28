---
feature_stage: mixed
feature_stage_surfaces:
  - core-language-syntax
  - documented-stdlib
  - pure-core-rust-artifacts
  - core-cli-workflow
  - style-and-modeling-guidance
---

# Common Mistakes When Coming From Other Languages

Use Futuruna's [basics](basics.md) and [rune reference](runes.md) when adapting
an example from another language. These are the small differences most likely
to change what a program means.

## Bindings, comparisons, and comments

Bind a value with `= name = value`; compare with `==`, including in `under`
guards and invariant predicates. Write line comments with `--` and source
quotation blocks with `---- ... ----`. `#` declares a type: `# TODO` is a valid
type declaration, so a compiler cannot assume that it was intended as a comment.

A function returns its last expression. Conditionals use `if ... { ... } else
if ... { ... } else { ... }`. Lambdas use `|x| expression`, and ranges use
end-exclusive `range(start, end)`.

## Nominal types and numeric wrappers

Futuruna has no type aliases. `# Money = Int` is a type error: a variant may
not reuse a built-in type name, and the declaration would otherwise be a
nominal type with a nullary variant named `Int`. To give a number a
domain-specific type, put it in a record and access its field explicitly:

```runa
# Money(amount: Int)
> amount_of(value: Money) -> Int { value.amount }
@ print(show(amount_of(Money(amount = 42)))) -- 42
```

Use `Int` directly when no wrapper is needed. A field's name does not establish
its currency, units, or permitted range; document and validate those separately.

## Rule coverage and exceptions

Annotate rule inputs where their types are known. If a value rule should return
a result for every input, provide an unguarded fallback that is justified by
the model:

```runa
| rate(age: Int) -> 25
| exception youth rate(age: Int) -> 10 under age < 18
```

The exception needs a label (`youth`) before the rule head. Combine guard
conditions with `and` or `&&`; commas separate goals inside a logic rule body.

Some rules are deliberately partial. Do not invent a fallback merely to make
the program run; represent missing information explicitly or restrict and
check the input domain. A direct Boolean result such as
`| adult(age: Int) -> age >= 18` makes both outcomes clear. A safely known
Boolean rule also returns `False` when no clause matches; this runtime behavior
does not establish the separate requirements for a proof.

Logic variables are lowercase (`x`, `parent`); uppercase names denote
constructors (`Some`, `None`, or an authored type's constructors).

## Effects and ordinary calls

Print with `@ print(value)`. `@ println`, `@ log`, and `@ assert` are not
supported effects. Use ordinary `assert(condition)` for a runtime assertion,
or declare an invariant with `| name: subject -> predicate` and invoke it with
`? name`.

Lists use functions such as `length(items)` and `map(items, |x| x + 1)`.
Do not assume that another language's `.len()` or `.map(...)` methods exist.

## Strings and numbers

- Ordinary strings keep braces literally: `"Hello {name}"`. Interpolation uses
  triple quotes and doubled braces: `"""Hello {{name}}"""`.
- A string operand makes `+` concatenate text: `"10" + 5` is `"105"`.
  Parse numeric text explicitly before doing arithmetic.
- Integer division truncates: `1 / 3 * 100` is `0`, while `1 * 100 / 3` is
  `33`. Choose units and rounding deliberately. Multiplying first can overflow,
  so the input domain and the intermediate value must fit `Int`.
- `%` means remainder. A percentage is a numeric value with a model-defined
  unit, such as `25` percent or the fraction `0.25`.
- There are no date literals: `2024-01-31` is subtraction and gives `1992`.
  Use date text or a validated record. Write thousands as `1000`; commas
  separate items and underscores are unsupported.
- A newline usually ends a statement. To subtract across lines, put the `-`
  before the newline or group the whole expression in parentheses. Indentation
  alone does not continue an expression.

## Check the program and its intended behavior

Use a compiler built from the checkout whose contracts you are reading:

```bash
runa fmt --check ./model.runa
runa check ./model.runa
runa ./model.runa
runa run ./model.runa
```

`runa check` validates the frontend and generated Rust without executing the
program; `--frontend` omits the Rust check. The last two commands run interpreted
and native code respectively. A successful run of one case does not establish
rule coverage or equivalence for all inputs. Test the applicable boundaries,
missing cases, and expected failures. Check the [feature stages](../feature-stages.md)
before relying on a particular execution or verification surface.

Both `model.runa` and `./model.runa` are accepted source paths. An explicit
relative path can make examples clearer; it is not a workaround required by
the current import resolver. See [CLI diagnostics](../cli-diagnostics.md) for
structured checks and their limits.
