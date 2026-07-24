---
feature_stage: stable
feature_stage_surfaces:
  - core-language-syntax
---

# 2. Types and Pattern Matching

## Structs

```runa
# Point(x: Float, y: Float)

= p = Point(3.0, 4.0)
@ print("x = " + show(p.x))
@ print("y = " + show(p.y))
```

Structs are defined with `#` and constructed by name. Fields are accessed with dot notation.

## Enums (Algebraic Data Types)

```runa
# Shape = Circle(Float) | Rectangle(Float, Float) | Triangle(Float, Float, Float)

> area(s: Shape) -> Float {
    match s {
        | Circle(r) -> 3.14159 * r * r
        | Rectangle(w, h) -> w * h
        | Triangle(a, b, c) -> {
            = s = (a + b + c) / 2.0
            sqrt(s * (s - a) * (s - b) * (s - c))
        }
    }
}

@ print(show(area(Circle(5.0))))
@ print(show(area(Rectangle(3.0, 4.0))))
```

Pattern matching with `|` arms is how you work with enums. The compiler checks for exhaustiveness — miss a variant and you get an error.

## Option and Result

```runa
> safe_divide(a: Int, b: Int) -> Result(Int, String) {
    if b == 0 { Err("division by zero") }
    else { Ok(a / b) }
}

-- Monadic bind: unwrap or early-return
> compute(x: String, y: String) -> Result(Int, String) {
    = a <- parse_int(x)
    = b <- parse_int(y)
    = c <- safe_divide(a, b)
    Ok(c * 2)
}
```

The `<-` operator unwraps `Ok`/`Some` or returns early on `Err`/`None`. No callback pyramids.

## Next

[3. Functions](03-functions.md)
