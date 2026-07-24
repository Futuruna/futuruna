---
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
---

# 3. Functions and Lambdas

## Functions

```runa
> add(a: Int, b: Int) -> Int { a + b }

> factorial(n: Int) -> Int {
    if n <= 1 { 1 }
    else { n * factorial(n - 1) }
}

@ print(show(factorial(10)))  -- 3628800
```

Functions are defined with `>`. Parameters have type annotations. The body is the return value (no `return` keyword).

## Lambdas

```runa
= double = |x: Int| x * 2
= items = [1, 2, 3, 4, 5]
= doubled = map(items, double)
@ print(show(doubled))  -- [2, 4, 6, 8, 10]
```

Lambdas use `|params|` syntax. They capture variables from their environment.

## Higher-order functions

```runa
> apply_twice(f: Int -> Int, x: Int) -> Int {
    f(f(x))
}

@ print(show(apply_twice(|x| x + 1, 10)))  -- 12
```

## Pipe operator

```runa
= result = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    |> filter(|x| x > 3)
    |> map(|x| x * 2)
    |> take(3)

@ print(show(result))  -- [8, 10, 12]
```

`|>` passes the left side as the first argument to the right side. Clean data pipelines without nesting.

## Ownership (invisible)

Futuruna infers ownership automatically. You never write `&`, `&mut`, lifetimes, or `.clone()`. The compiler's escape analysis decides:
- Single use → move (zero cost)
- Multiple uses → clone (safe)
- Read-only parameter → borrow (efficient)

```runa
> process(data: List(String)) -> List(String) {
    -- data is automatically borrowed or moved as needed
    map(data, |s| to_upper(s))
}
```

## Next

[4. Rules and Verification](04-rules.md)
