# M44: Web Playground

**Tagline:** "Try Futuruna without installing anything."

**Status:** DONE. Live at website /playground.

## Result

Dioxus WASM application with:
- `futuruna::eval_source()` compiled to WASM — runs entirely client-side
- Code editor with syntax highlighting + hover tooltips for runes
- "Run" button → eval_source() → display output
- 6 preloaded examples (Weather, Hello, Streams, Rules, Fibonacci, Boot)
- Share links (deflate + base64url in URL fragment)
- Full playground page at /playground + embedded on homepage
- Delay support for streaming demos (|> delay(N))
