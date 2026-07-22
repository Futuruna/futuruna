# Project: Self-Hosting Futuruna

Goal: Make the Futuruna compiler compile itself.

## Status: In Progress

## Critical Blockers (in order)

| # | Milestone | Status | What |
|---|-----------|--------|------|
| 1 | **M24: Map + Set** | DONE | 22 builtins, Map(K,V)→HashMap, Set(T)→HashSet, 64/64 tests |
| 2 | **M25: Qualified Imports** | DONE (already existed) | M3b was complete — qualified imports, inline modules, nested modules, privacy |
| 3 | **M26: Port Lexer + Parser** | TODO | First self-hosted compiler components |
| 4 | **M27: Port Interpreter** | TODO | Self-hosted eval |
| 5 | **M28: Port Type Checker** | TODO | Self-hosted type checking |
| 6 | **M29: Port Codegen** | TODO | Self-hosted Rust emission (incl. escape analysis) |
| 7 | **M30: Bootstrap** | TODO | Compiler compiles itself |

## Design Decisions

- **Maps**: `Map(K, V)` type → `HashMap<K, V>` in Rust codegen
- **Sets**: `Set(T)` type → `HashSet<T>` in Rust codegen
- **Escape analysis**: TBD — Prolog rules (M23) vs `@ rust {}` escape hatch
- **Compiler split**: lexer.runa, parser.runa, interpreter.runa, codegen.runa, main.runa

## Verified Bootstrap

Self-hosting and verified bootstrap are related, but they are not the same milestone.

- Self-hosting means Futuruna can compile Futuruna.
- Verified bootstrap means the compiler stages we rely on are themselves justified by proofs or translation checks.

The current proof-carrying bootstrap plan and trust boundary live in [docs/verified-bootstrap.md](../../docs/verified-bootstrap.md). The short version is that Futuruna can already prove tiny compiler passes inside Futuruna, but the production compiler is not yet verified.

## Analysis

See `research/language-review/adversarial-review.md` for honest assessment.
