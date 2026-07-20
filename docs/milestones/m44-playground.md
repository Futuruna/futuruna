# M44: Web Playground

**Tagline:** "Try Futuruna without installing anything."

## Goal

Compile the interpreter (lib.rs) to WASM. Embed in the Dioxus website
with a code editor and output panel. Programs run client-side.

## Approach

1. lib.rs already has `eval_source()` — expose via wasm-bindgen
2. CodeMirror editor with Futuruna syntax highlighting
3. "Run" button → eval_source() → display output
4. Preloaded examples (one per rune)
5. Share links (source in URL fragment)
