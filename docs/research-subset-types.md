# Algebraic Subset Types — Design Sketch

**Core idea:** Types are sets of constructors. A subset type picks specific variants from a parent type. The same constructor value inhabits multiple types simultaneously.

## Syntax

```runa
-- Base type: full set of constructors
# GeoArea = Sweden | Danmark | Norway | Færøerne | Grønland | Iceland

-- Subset types: pick specific variants from the parent
# Skandinavien = GeoArea.Sweden | GeoArea.Danmark | GeoArea.Norway
# Rigsdel = GeoArea.Danmark | GeoArea.Færøerne | GeoArea.Grønland
# Norden = GeoArea.Sweden | GeoArea.Danmark | GeoArea.Norway | GeoArea.Færøerne | GeoArea.Grønland | GeoArea.Iceland

-- Multi-level: subset of a subset
# EUSkandinavien = Skandinavien.Sverige | Skandinavien.Danmark
```

## Type relationships

```
GeoArea ⊇ Norden ⊇ Skandinavien ⊇ EUSkandinavien
GeoArea ⊇ Rigsdel
Skandinavien ∩ Rigsdel = {Danmark}
```

`Danmark` is simultaneously a valid:
- `GeoArea`
- `Skandinavien`  
- `Rigsdel`
- `Norden`

## Rules with subset types

```runa
-- Applies to exactly the realm parts
| grundloven_gælder_for(r: Rigsdel) -> true

-- Applies to all of Scandinavia  
| skandinavisk_samarbejde(s: Skandinavien) -> true

-- Danmark satisfies BOTH
@ print(grundloven_gælder_for(Danmark))     -- true
@ print(skandinavisk_samarbejde(Danmark))    -- true

-- Sweden is Scandinavian but not a Rigsdel
@ print(skandinavisk_samarbejde(Sweden))     -- true
@ print(grundloven_gælder_for(Sweden))       -- false

-- findall respects subset boundaries
= nordiske = findall(n, skandinavisk_samarbejde(n))  
-- [Sweden, Danmark, Norway]

= rigsdele = findall(r, grundloven_gælder_for(r))
-- [Danmark, Færøerne, Grønland]
```

## Subtyping rules

1. **Widening (always safe):** `Skandinavien` → `GeoArea` ✓
   A Skandinavien value is always a valid GeoArea.

2. **Narrowing (may fail):** `GeoArea` → `Skandinavien` — requires runtime check
   Not all GeoAreas are Scandinavian. `Færøerne` is a GeoArea but not Skandinavien.

3. **Function params:** `f(x: Skandinavien)` accepts Danmark, Sweden, Norway.
   `f(x: GeoArea)` accepts any of the 6.

4. **Match exhaustiveness:** `match x: Skandinavien` only needs Sweden|Danmark|Norway arms.

## Implementation plan

### Layer 1: Parser (easy, ~30 lines)

In `parse_type_decl`, when parsing `# Name = Variants`:
- If a variant contains `.` (e.g., `GeoArea.Danmark`), it's a subset reference
- Parse `ParentType.Variant` as a qualified variant
- Store as a new TypeDecl variant: `SubsetType { name, parent, variants }`

```rust
// In enum TypeDecl, add:
SubsetType {
    name: String,
    parent: String,        // "GeoArea"  
    variants: Vec<String>, // ["Danmark", "Færøerne", "Grønland"]
}
```

### Layer 2: Type registry (easy, ~20 lines)

In Interpreter:
```rust
// Add to Interpreter fields:
pub subset_of: BTreeMap<String, String>,        // "Rigsdel" → "GeoArea"
pub subset_variants: BTreeMap<String, Vec<String>>, // "Rigsdel" → ["Danmark", ...]
```

When processing `SubsetType`:
1. Validate all variants exist in parent's `type_variants`
2. Register the subset in both maps
3. Register the subset's variants in `type_variants` (so findall works)

### Layer 3: Type constraint checking (easy, ~15 lines)

In `match_rule_head` for `__typed` params:
- Current: check if constructor is in `type_variants[type_name]`
- New: ALSO check parent types recursively

```rust
fn is_valid_for_type(&self, constructor: &str, type_name: &str) -> bool {
    // Direct membership
    if self.type_variants.get(type_name)
        .map_or(false, |vs| vs.contains(&constructor.to_string())) {
        return true;
    }
    // Check if this is a subset type, and the constructor is in the parent
    // (NO — subset types restrict, they don't widen. A Rigsdel check 
    //  should only accept Rigsdel variants, not all GeoAreas.)
    false
}
```

### Layer 4: Subtype coercion in function calls (medium, ~40 lines)

When calling `f(x: GeoArea)` with a `Skandinavien` value:
- The value `Danmark` is `Constructor("Danmark", [])` regardless of declared type
- No coercion needed at runtime — the constructor IS the same value
- Type checking just needs to verify: Skandinavien ⊆ GeoArea

This is the key insight: **constructors are untyped at runtime**. The type constraint is purely a compile-time / rule-time check. No boxing, no wrapping, no conversion.

### Layer 5: findall enumeration (easy, ~10 lines)

`findall(r, rule(r: Rigsdel))` should enumerate Rigsdel's variants.
Already works via `collect_all_values` harvesting `type_variants`.
Just needs `subset_variants` to also be registered in `type_variants`.

### Layer 6: Codegen (medium, ~50 lines)

In Rust codegen, subset types can be:

**Option A: Same enum, runtime validation**
```rust
type Rigsdel = GeoArea; // type alias
fn is_rigsdel(g: &GeoArea) -> bool {
    matches!(g, GeoArea::Danmark | GeoArea::Færøerne | GeoArea::Grønland)
}
```

**Option B: Newtype with TryFrom**
```rust
struct Rigsdel(GeoArea);
impl TryFrom<GeoArea> for Rigsdel { ... }
impl From<Rigsdel> for GeoArea { ... }
```

Option A is simpler and matches the interpreter semantics.

### Layer 7: Match exhaustiveness (hard, ~100 lines)

```runa
match r: Rigsdel {
    | Danmark -> "DK"
    | Færøerne -> "FO"
    | Grønland -> "GL"
}
```

This is exhaustive for Rigsdel (3 arms = 3 variants). But:
```runa
match r: GeoArea {
    | Danmark -> "DK"
    | _ -> "other"
}
```
Needs `_` because GeoArea has 6 variants.

The exhaustiveness checker needs to know which type the scrutinee has, and check against that type's variant set.

## What's NOT needed

- **No inheritance of methods.** Subset types don't inherit behavior.
- **No virtual dispatch.** The constructor is the same value.  
- **No boxing/unboxing.** Runtime representation is identical.
- **No coercion functions.** A Danmark is a Danmark.

## Estimated effort

| Layer | Effort | Lines |
|-------|--------|-------|
| Parser | Easy | ~30 |
| Type registry | Easy | ~20 |
| Type constraint | Easy | ~15 |
| Subtype coercion | Medium | ~40 |
| findall | Easy | ~10 |
| Codegen | Medium | ~50 |
| Exhaustiveness | Hard | ~100 |
| **Total** | | **~265 lines** |

The first 5 layers (~115 lines) get the core working for rules and findall.
Codegen and exhaustiveness are needed for compilation but not for the interpreter.

## Why this is powerful for legal modeling

Constitutional law is FULL of overlapping jurisdictions:
- Danmark is in the EU, Færøerne is not
- Danmark is Scandinavian, Grønland is not
- All three are in Rigsfællesskabet

With subset types, these overlaps are modeled naturally:
```runa
# EUMedlem = GeoArea.Danmark
# Skandinavien = GeoArea.Sweden | GeoArea.Danmark | GeoArea.Norway  
# Rigsdel = GeoArea.Danmark | GeoArea.Færøerne | GeoArea.Grønland

| eu_ret_gælder(r: EUMedlem) -> true
| grundloven_gælder(r: Rigsdel) -> true
| nordisk_samarbejde(s: Skandinavien) -> true

-- Danmark satisfies all three. Færøerne satisfies only grundloven.
-- Sweden satisfies only nordisk_samarbejde. These are provable facts.
```

The audit rune (`runa audit`) could discover:
- "Færøerne is a Rigsdel but not an EUMedlem — asymmetry"
- "No GeoArea satisfies all three rules simultaneously except Danmark"
- "Adding Finland to Norden would not affect Rigsdel rules"
