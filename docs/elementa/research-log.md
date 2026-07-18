# Elementa Research Log

Ongoing research notes, references, and decisions for the Elementa framework.

---

## Genesis

### The Problem

Every UI framework in existence conflates at least three of these six concerns:

| Concern | What it is | Who owns it |
|---------|-----------|-------------|
| Structura | Layout skeleton | Designer / architect |
| Iconia | Symbols, images, assets | Icon / brand designer |
| Logica | Behavior, data | Developer |
| Style | Visual rules between icons and layout | Design systems engineer |
| Animation | Temporal streams over visuals | Motion designer |
| Flow | State transitions between layouts | UX designer / PM |

**Examples of conflation in existing frameworks:**

- **React:** JSX = Structura + Logica + Flow (conditional renders). CSS-in-JS = Style + Animation. No separation possible.
- **SwiftUI:** `body` = Structura + Style + Flow. `withAnimation` = Flow + Animation. Modifiers = Style + Structura.
- **Flutter:** Widget tree = Structura + Style + Animation (implicit animations). Navigator = Flow + Logica.
- **CSS itself:** `margin` = Style. `transition` = Flow. `@keyframes` = Animation. Three concerns in one language.
- **Compose:** Column/Row = Structura. Modifier = Style + Animation. NavHost = Flow + Logica.

### The Insight

These six concerns form a **hexagonal graph** — three domains at the vertices,
three connectors at the edges. Each connector touches exactly two domains and
no others. This is not an arbitrary decomposition; it reflects how different
**specialists** think about an application:

- A layout designer never thinks about API calls
- A motion designer never thinks about padding rules
- A backend developer never thinks about spring physics

The framework should respect these cognitive boundaries.

### Why Futuruna?

Futuruna's rune system already carves syntax along cognitive axes (that's the
IIT d_eff=3 result). The mapping is natural:

- `#` (what exists) → Structura
- `=` (what is) → Iconia
- `>` (what happens) → Logica
- `|` (what should be true) → Style
- `~` (what flows) → Animation, Flow
- `@` (meta) → imports, WASM target config
- `?` (prove it) → Flow verification, style constraint checking

Native reactive streams (`~`) are the key differentiator. Animation and Flow are
both streams — time-varying transformations that compose. No other language has
this as a first-class syntactic construct.

### Rendering Decision: Hybrid

Three paths were considered:

| Path | Pros | Cons |
|------|------|------|
| Pure DOM | Accessible, text-native, proven | Can't do 60fps GPU animation, inherits CSS Cronenberg |
| Pure Canvas/WebGPU | Total control, clean model | Must rebuild text layout, accessibility, inputs from scratch |
| **Hybrid** | Browser text + GPU animation + native a11y | Two compositing layers, more complex bridge |

Chose **Hybrid**. The browser's text layout engine is genuinely world-class and
rebuilding it would be years of work for worse results. But the browser's
animation/rendering pipeline is not — Canvas/WebGPU is strictly better for
icons, animations, and custom visuals.

### Name

**Elementa** — from Latin "elementa" (first principles, fundamental components).
The framework decomposes applications into their elements.

---

## Research Threads

### Existing Hybrid Renderers
- **Wry / Tauri:** Webview + Rust backend. Similar hybrid spirit, but the
  separation is "web vs native", not the six-concern decomposition.
- **Makepad:** Custom GPU renderer with DOM fallback. Closer to pure Canvas.
  Worth studying their WASM + WebGPU pipeline.
- **Dioxus:** Rust + WASM with virtual DOM. React model in Rust. Conflates
  concerns the same way React does.
- **Leptos:** Fine-grained reactivity in Rust/WASM. Reactive signals are
  similar to `~` streams. Worth studying their signal implementation.
- **Xilem:** Rust UI with incremental computation. Architecture-focused.
  Their "view tree diff" approach is relevant to Structura → DOM updates.

### Canvas + DOM Compositing
- Mapbox GL does this: WebGL map underneath, DOM overlays on top.
  Hit-testing across layers is the hard part.
- Google Docs moved from DOM to Canvas for text rendering (opposite direction).
  Their motivation: performance and consistency. Our motivation is different —
  we keep DOM for text, Canvas for icons/animation.
- VS Code uses a Canvas-based editor with DOM overlays for widgets.

### Reactive Animation Systems
- **Framer Motion:** Declarative animation in React. Good API design, but
  fundamentally limited by being a library bolted onto a framework that
  doesn't support streams.
- **Rive:** Binary animation runtime. Closest to "animation as its own domain."
  Renders to Canvas. Their mental model matches our Animation connector.
- **Lottie:** JSON animation format from After Effects. Proves that animation
  can be authored separately from code and layout.
- **Motion Canvas:** Programmatic animation tool. TypeScript-based. Uses
  generator functions for sequencing — similar in spirit to `~` streams.

### WASM UI Precedent
- Current WASM UI frameworks (Yew, Leptos, Dioxus, Sycamore) all reproduce
  the React/component model in Rust. None attempt a new decomposition.
- WASM + WebGPU is stabilizing. Chrome, Firefox, Safari all ship WebGPU.
  The timing is right for a GPU-accelerated UI framework.

### Design Tool Interop
- Figma plugin API allows reading/writing design files programmatically.
  Structura + Style + Iconia could potentially round-trip.
- Figma's internal model: frames (Structura), fills/strokes (Style),
  components (composition), auto-layout (constraint-based Style).
  The mapping is surprisingly clean.

---

## Decisions Made

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Hybrid rendering (DOM + Canvas/WebGPU) | Best of both worlds, accessibility for free |
| 2 | Six-concern hexagonal decomposition | Matches specialist cognitive boundaries |
| 3 | Rune-enforced separation | Parser prevents concern mixing |
| 4 | WASM target via Rust transpilation | Existing runa pipeline, zero-cost abstractions |

## Decisions Pending

| # | Question | Options | Blocking |
|---|----------|---------|----------|
| 1 | Component composition model | a) Manifest file linking concerns, b) Convention-based name matching, c) New rune | Demo app |
| 2 | Responsive layout mechanism | a) Style constraints, b) Flow states, c) Structura variants | Layout prototype |
| 3 | Form binding model | a) Flow, b) New connector, c) Logica with special bindings | Input prototype |
| 4 | Exact DOM↔Canvas compositing API | Study Mapbox, VS Code approaches | Rendering prototype |
