# Futuruna Elementa

**A UI framework built on the hexagonal decomposition of application concerns.**

Elementa decomposes every application into three core domains and three connectors,
mapped onto Futuruna's rune system and native reactive streams. The target is WASM
via Rust transpilation.

---

## The Hexagonal Model

```
          Iconia
         (symbols)
        /          \
     Style       Animation
      /              \
  Structura ------- Logica
    (layout)  Flow  (code)
```

Three vertices. Three edges. Six concerns. Zero mixing.

---

## Core Domains

### Structura — What Exists (`#`)

Pure layout. The skeleton of the application with no style, no logic, no color.
Structura defines **what is on screen and how it is spatially composed**.

In today's world this is HTML, JSX, SwiftUI body, Compose column/row — all of which
leak logic and style into layout. Structura doesn't.

```
# View { children: List[View] }
# Stack { axis: Axis, gap: Spacing, children: List[View] }
# Text { content: String }
# Slot { name: Symbol, child: View }
# Input { kind: InputKind, bind: Binding[String] }

-- A screen is pure structure
# screen search_screen {
  Stack(axis: vertical) {
    Slot(name: :header)
    Slot(name: :results)
    Slot(name: :footer)
  }
}
```

**Handoff target:** Designer, layout architect. No code knowledge required.

### Iconia — What Is (`=`)

Ground-truth symbols. SVGs, raster images, icon sets, illustrations — named and
catalogued. No style applied, no animation attached. Pure visual atoms.

```
= icon_search   : Svg("assets/icons/search.svg")
= icon_menu     : Svg("assets/icons/menu.svg")
= icon_back     : Svg("assets/icons/arrow_back.svg")
= avatar_default : Raster("assets/img/default_avatar.png")
= logo          : Svg("assets/brand/logo.svg")
```

**Handoff target:** Icon designer, brand team. Just assets with names.

### Logica — What Happens (`>`)

Pure behavior. Functions, actors, data transformations. Logica has no knowledge of
layout or visuals. It operates on typed data and produces typed data.

```
> search(query: String) -> Result[List[Item], ApiError] {
  = raw <- api.get("/search", { q: query })
  raw.items |> filter(|i| i.score > 0.5) |> sort_by(.score)
}

> format_price(cents: Int) -> String {
  = dollars = cents / 100
  = remainder = cents % 100
  "{dollars}.{remainder:02}"
}
```

**Handoff target:** Backend developer, data engineer. No UI knowledge required.

---

## Connectors

### Style — Between Iconia and Structura (`|`)

Rules that bind icons into layout. Padding, margins, spacing, borders, semantic
color tokens, typography scales. Style is a **constraint language**, not a property
bag. It says "what should be true" about the visual relationship between symbols
and structure.

```
| style header_bar {
  padding: 12 16
  gap: 8
  bg: token.surface
  fg: token.on_surface
  elevation: 2
}

| style icon_button {
  size: 24 24
  hit_area: 48 48
  tint: token.primary
  radius: 12
}

| style body_text {
  font: token.body_large
  color: token.on_surface
  line_height: 1.5
}

-- Semantic tokens, not raw values
| tokens light {
  surface: #FFFFFF
  on_surface: #1C1B1F
  primary: #6750A4
  on_primary: #FFFFFF
}

| tokens dark {
  surface: #1C1B1F
  on_surface: #E6E1E5
  primary: #D0BCFF
  on_primary: #381E72
}
```

Style is **never** in the same file as logic. It never contains conditionals,
loops, or function calls. It is pure declarative constraint.

**Handoff target:** Design systems engineer, theme designer.

### Animation — Between Iconia and Logica (`~`)

Reactive streams over visuals. Animation is what happens when logic wants to
bring an icon to life. It is a **temporal transformation** — a stream of values
applied to visual properties over time.

```
-- Tweens: point-to-point transitions
~ fade_in(target: View) -> Stream[Opacity] {
  target.opacity |> from(0.0) |> to(1.0) |> ease(cubic_out) |> dur(200ms)
}

-- Loops: continuous motion
~ pulse(icon: Svg) -> Stream[Scale] {
  icon.scale |> from(1.0) |> to(1.1) |> ease(sine_in_out) |> dur(600ms) |> yoyo
}

-- Sequences: choreographed motion
~ stagger_in(items: List[View]) -> Stream[Unit] {
  items |> each_with_index(|view, i| {
    delay(i * 50ms) |> then(fade_in(view))
  }) |> merge
}

-- Physics-based: spring dynamics
~ spring_press(target: View) -> Stream[Scale] {
  target.scale |> spring(stiffness: 300, damping: 15) |> to(0.95)
}
```

Animation is always a `Stream`. It composes with pipe operators. It runs on
`requestAnimationFrame` in the WASM runtime. Logic triggers it; icons receive it.
Animation never knows about layout.

**Handoff target:** Motion designer. Pure temporal streams, no business logic.

### Flow — Between Logica and Structura (`~` + `|`)

State machine transitions between layouts. Flow defines **which screen the user
is on and how they got there**. It is the connective tissue between what the app
does (Logica) and what the app shows (Structura).

Flow is currently the most tangled concern in every framework — scattered across
routers, navigation stacks, conditional renders, and state management. Elementa
makes it a first-class, visible, isolated concern.

```
~ flow app {
  -- States are layout references
  | state splash
  | state home
  | state search
  | state detail(item: Item)
  | state settings

  -- Transitions: trigger, target, animation
  | splash -> home     : after(2s)           via fade_in
  | home -> search     : on(tap :search_btn) via slide_left
  | home -> settings   : on(tap :menu_btn)   via slide_right
  | home -> detail     : on(select_item)     via slide_up
  | search -> home     : on(back)            via slide_right
  | detail -> home     : on(back)            via slide_down
  | settings -> home   : on(back)            via slide_right
}
```

At a glance you can see: every state, every transition, every trigger, every
animation. A UX designer can read this. A formal verifier (`?`) can prove
reachability. No state is invisible.

**Handoff target:** UX designer, product manager. Readable state machine.

---

## Rendering Architecture: The Hybrid Path

Elementa uses a **hybrid rendering model**:

### DOM Layer (Structura + Text)
- Structura compiles to semantic DOM elements (`div`, `section`, `input`, etc.)
- Text rendering uses the browser's native text layout (excellent, battle-tested)
- Accessibility is inherited: screen readers, tab order, ARIA — all native
- Style rules compile to scoped CSS custom properties and a minimal layout engine

### Canvas/WebGPU Layer (Iconia + Animation)
- Icons and animations render on a composited Canvas or WebGPU surface
- SVG icons are rasterized at the correct resolution and animated on the GPU
- Physics-based animations run at 60/120fps without touching the DOM
- Lottie-style complex animations are first-class

### Compositing Bridge
- A thin compositor overlays the Canvas layer on the DOM layer
- Hit-testing merges both layers (DOM events + canvas hit regions)
- The bridge handles z-ordering between DOM content and canvas visuals
- Structura `Slot` elements define where canvas-rendered content appears

```
┌─────────────────────────────┐
│  Canvas/WebGPU Layer        │  ← Iconia + Animation
│  (icons, animations, GPU)   │
├─────────────────────────────┤
│  Compositing Bridge         │  ← hit-test merge, z-order
├─────────────────────────────┤
│  DOM Layer                  │  ← Structura + Style + Text
│  (layout, text, a11y)       │
└─────────────────────────────┘
```

### Why Hybrid?

- **Not pure DOM:** DOM can't do 60fps GPU animations or custom rendering
- **Not pure Canvas:** Canvas can't do accessible text, native inputs, or SEO
- **Hybrid:** Best text engine in the world (the browser) + best render pipeline
  available (WebGPU) + full accessibility

---

## Futuruna Advantages

### Runes Are the Separation

The seven runes enforce the hexagonal decomposition at the **syntax level**.
You can't accidentally mix Style into Logica because they use different runes.
The parser itself is the architecture enforcer.

### Reactive Streams Are Native

Animation and Flow are both time-varying transformations. In every other framework,
you import a library for this. In Futuruna, `~` is a first-class rune. Streams
compose with `|>`. They schedule on the reactive runtime. No library needed.

### Verification Is Built In

The `?` rune can verify Flow state machines:
```
? flow app {
  -- Every state is reachable from splash
  | reachable(splash, *)
  -- No dead-end states (every state has an exit)
  | no_deadlocks
  -- Back always returns to previous state
  | back_returns
}
```

### Transpilation Pipeline

```
Futuruna (.runa)
    ↓ runa emit
Rust (.rs)
    ↓ cargo build --target wasm32-unknown-unknown
WASM (.wasm)
    ↓ wasm-bindgen / wasm-pack
Browser runtime
```

The existing pipeline gets us 90% of the way. What's new is the runtime that
bridges WASM to DOM + Canvas.

---

## File Convention

```
app/
  structura/        -- Layout definitions (#)
    search.runa
    home.runa
    detail.runa
  iconia/           -- Icon and asset bindings (=)
    icons.runa
    illustrations.runa
  style/            -- Style rules and tokens (|)
    tokens.runa
    components.runa
  animation/        -- Animation streams (~)
    transitions.runa
    micro.runa
  flow/             -- Flow state machines (~ + |)
    app.runa
    auth.runa
  logica/           -- Pure logic (>)
    search.runa
    auth.runa
    cart.runa
```

Each directory is a single concern. Each file is handoffable to a specialist.
The compiler enforces that runes in each directory match their domain.

---

## Open Questions

1. **Component composition.** How do the six concerns compose into reusable
   components? A "SearchBar" needs structure, style, animation, and logic.
   How is the binding expressed without collapsing the separation?

2. **Responsive layout.** Structura needs breakpoint-aware variants. Is this
   a Style concern (constraint rules that vary by viewport) or a Flow concern
   (different layouts are different states)?

3. **Forms and two-way binding.** Input elements need Logica ↔ Structura
   bidirectional data flow. Is this Flow, or a fourth connector?

4. **Server-side rendering.** The hybrid model assumes a browser. For SSR,
   the Canvas layer doesn't exist. How does Iconia degrade?

5. **Design tool integration.** Can Structura + Style + Iconia files be
   round-tripped to/from Figma or similar tools?

6. **Performance budget.** Two rendering layers means two compositing passes.
   What's the overhead, and when does pure-DOM fallback make sense?

---

## Status

This document is a design sketch. Nothing is implemented yet.

Next steps:
- Prototype Structura → DOM compilation
- Prototype Animation → Canvas/rAF runtime
- Define the compositing bridge API
- Build a single demo app (search screen) using all six concerns
