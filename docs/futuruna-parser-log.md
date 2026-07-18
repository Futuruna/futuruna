# Self-Hosting Parser — Design Log

**Goal:** Write Futuruna's parser in Futuruna itself. The lexer is done
(`examples/lexer.runa`, ~350 lines). The parser is the second bootstrapping
step. When the parser can parse itself, Futuruna crosses the threshold from
"language with a Rust host" to "language that sustains itself."

---

## Architecture

### The core constraint: immutability

The Rust parser uses `&mut self` with a mutable position cursor:

```rust
// Rust parser — mutable state
fn parse_expr(&mut self) -> Result<Expr, String> {
    let tok = self.advance();  // mutates self.pos
    ...
}
```

Futuruna is immutable by default. The parser must **thread state through
return values** — the same pattern the self-hosting lexer uses:

```
-- Futuruna parser — functional state threading
> parse_expr(st: PState) -> PExpr {
    = tok = peek(st)
    = st2 = advance(st)
    ...
    PExpr(st2, EVar(tok.text))
}
```

Every parse function takes a `PState`, returns a result struct carrying
the **new** `PState` alongside the parsed node. No mutation. The state
flows forward like a river.

### State and result types

```
# PState(toks: List(Token), pos: Int)

-- One result type per AST category
# Consumed(st: PState, tok: Token)
# PExpr(st: PState, expr: Expr)
# PStmt(st: PState, stmt: Stmt)
# PTy(st: PState, ty: Ty)
# PPat(st: PState, pat: Pat)
# PList(st: PState, items: List(Expr))
# PStmts(st: PState, stmts: List(Stmt))
```

### Navigation helpers

```
-- Peek at current token without consuming
> peek(st: PState) -> Token {
    nth(st.toks, st.pos)
}

-- Return new state advanced by one position
> advance(st: PState) -> PState {
    PState(st.toks, st.pos + 1)
}

-- Advance and return both new state and consumed token
> consume(st: PState) -> Consumed {
    Consumed(PState(st.toks, st.pos + 1), nth(st.toks, st.pos))
}

-- Skip newlines (TkSemi tokens)
> skip_semis(st: PState) -> PState {
    if peek(st).kind == TkSemi { skip_semis(advance(st)) }
    else { st }
}

-- Expect a specific token kind, error if wrong
> expect(st: PState, kind: TokenKind) -> Consumed {
    = tok = peek(st)
    if tok.kind == kind { consume(st) }
    else {
        @ print("Parse error " + show(tok.line) + ":" + show(tok.col)
                + ": expected " + show_kind(kind)
                + ", got " + show_kind(tok.kind))
        Consumed(st, tok)
    }
}
```

### Statement dispatch — the seven runes

This is the heart of the parser. Each rune routes to its own handler:

```
> parse_statement(st: PState) -> PStmt {
    = st = skip_semis(st)
    = tok = peek(st)
    match tok.kind {
        TkHash  -> parse_type_decl(advance(st))     -- # what exists
        TkGt    -> parse_definition(advance(st))     -- > what happens
        TkPipe  -> parse_rule(advance(st))           -- | what should be true
        TkEq    -> parse_binding(advance(st))        -- = what is
        TkTilde -> parse_stream(advance(st))         -- ~ what flows
        TkAt    -> parse_annotation(advance(st))     -- @ where proofs stop
        TkOp if tok.text == "?" ->
                   parse_prove(advance(st))          -- ? prove it
        TkKW if tok.text == "for" ->
                   parse_for(advance(st))
        _       -> {
            = e = parse_expr(st)
            PStmt(e.st, SExpr(e.expr))
        }
    }
}
```

### Expression parsing — Pratt precedence climbing

```
> parse_expr(st: PState) -> PExpr {
    parse_expr_prec(st, 0)
}

> parse_expr_prec(st: PState, min_prec: Int) -> PExpr {
    = atom = parse_atom(st)
    expr_loop(atom.st, atom.expr, min_prec)
}

-- Infix/postfix loop: check operator, recurse if precedence allows
> expr_loop(st: PState, lhs: Expr, min_prec: Int) -> PExpr {
    = tok = peek(st)
    if tok.kind == TkOp && op_prec(tok.text) >= min_prec {
        = st2 = advance(st)
        = rhs = parse_expr_prec(st2, op_prec(tok.text) + 1)
        expr_loop(rhs.st, EBinOp(tok.text, lhs, rhs.expr), min_prec)
    }
    else if tok.kind == TkDot {
        = c = consume(advance(st))
        expr_loop(c.st, EField(lhs, c.tok.text), min_prec)
    }
    else if tok.kind == TkLParen {
        = args = parse_arg_list(st)
        expr_loop(args.st, EApp(lhs, args.items), min_prec)
    }
    else { PExpr(st, lhs) }
}

-- Atoms: the leaves of expression trees
> parse_atom(st: PState) -> PExpr {
    = tok = peek(st)
    match tok.kind {
        TkIdent  -> PExpr(advance(st), EVar(tok.text))
        TkType   -> PExpr(advance(st), EVar(tok.text))
        TkInt    -> PExpr(advance(st), ELit(LInt(parse_int(tok.text))))
        TkFloat  -> PExpr(advance(st), ELit(LFloat(parse_float(tok.text))))
        TkString -> PExpr(advance(st), ELit(LStr(tok.text)))
        TkBool   -> PExpr(advance(st), ELit(LBool(tok.text == "True")))
        TkLParen -> parse_paren_or_tuple(st)
        TkLBrace -> parse_block_expr(st)
        TkLBracket -> parse_list_literal(st)
        TkPipe   -> parse_lambda(st)
        TkKW if tok.text == "if" -> parse_if(st)
        TkKW if tok.text == "match" -> parse_match(st)
        _        -> PExpr(advance(st), EUnit)
    }
}
```

### Program loop — accumulator pattern

Same pattern as the lexer's `tok_loop`: accumulate into a list, recurse.

```
> parse_program(st: PState) -> PStmts {
    parse_program_loop(skip_semis(st), [])
}

> parse_program_loop(st: PState, acc: List(Stmt)) -> PStmts {
    if peek(st).kind == TkEof { PStmts(st, acc) }
    else {
        = parsed = parse_statement(st)
        parse_program_loop(skip_semis(parsed.st), push(acc, parsed.stmt))
    }
}
```

---

## AST Types

Prefixed constructors to avoid name clashes across types (Futuruna
flattens constructors to the top level — no namespace qualification).

### Literals

```
# Literal = LInt(val: Int) | LFloat(val: Float) | LStr(val: String) | LChar(val: String) | LBool(val: Bool)
```

### Expressions

```
# Expr = EVar(name: String) | ELit(lit: Literal) | EApp(func: Expr, args: List(Expr)) | ELambda(params: List(FParam), body: Expr) | EBinOp(op: String, lhs: Expr, rhs: Expr) | EUnOp(op: String, operand: Expr) | EIf(cond: Expr, then_br: Expr, else_br: Expr) | EMatch(scrutinee: Expr, arms: List(MArm)) | EBlock(stmts: List(Stmt)) | EField(obj: Expr, field: String) | EIndex(obj: Expr, idx: Expr) | EList(elems: List(Expr)) | ETuple(elems: List(Expr)) | EEffect(name: String, args: List(Expr)) | ETry(inner: Expr) | EUnit
```

Note: `Expr` and `Stmt` are mutually recursive (`EBlock` holds `List(Stmt)`,
`Stmt` holds `Expr`). M25 (transparent Rc) handles this — both types get
`Rc` wrapping for their recursive fields.

### Patterns

```
# Pat = PWild | PVar(name: String) | PLit(lit: Literal) | PCon(name: String, args: List(Pat)) | PAs(inner: Pat, alias: String)
```

### Types

```
# Ty = TyName(name: String) | TyApp(con: Ty, args: List(Ty)) | TyArrow(from: Ty, to: Ty) | TyRef(inner: Ty) | TyOptional(inner: Ty) | TyUnit | TyHole
```

### Statements

```
# Stmt = SDefn(defn: Defn) | SType(td: TypeDecl) | SRule(rule: Rule) | SUse(path: String) | SImport(path: String) | SDepend(crate_name: String, version: String) | SRust(code: String) | SAnnot(name: String, args: List(Expr)) | SBind(pat: Pat, ty: Option(Ty), val: Expr) | SFor(var: String, iter: Expr, body: List(Stmt)) | SStreamBind(name: String, val: Expr) | SInvariant(name: String, subject: Expr, pred: Expr) | SProve(name: String) | SExpr(expr: Expr)
```

### Definitions

```
# Defn = DFn(name: String, params: List(FParam), ret_ty: Option(Ty), body: Expr) | DActor(name: String, state: FParam, handlers: List(MArm)) | DModule(name: String, body: List(Stmt))
```

### Rules

```
# Rule = RClause(head: Expr, body: Option(Expr)) | RDefault(head: Expr, value: Expr, cond: Option(Expr)) | RScope(name: String, body: List(Stmt))
```

### Type declarations

```
# TypeDecl = TdADT(name: String, params: List(FParam), variants: List(Vnt)) | TdTrait(name: String, methods: List(Defn)) | TdImpl(trait_name: String, for_type: String, methods: List(Defn))
```

### Supporting types

```
# FParam(name: String, ty: Option(Ty), is_inout: Bool)
# MArm(pat: Pat, guard: Option(Expr), body: Expr)
# Vnt(name: String, fields: List(VField), positional: Bool)
# VField(name: String, ty: Ty)
```

---

## Milestones

### P1: Foundation — AST types + state helpers

**Scope:** Define all AST types above. Implement `PState`, navigation
helpers (`peek`, `advance`, `consume`, `expect`, `skip_semis`), and
`expect_ident`. Demo: tokenize a small program, create `PState`,
navigate and print tokens.

**Output:** `examples/parser.runa` with `@ import ./lexer`

**Test:** `runa run examples/parser.runa` succeeds, prints tokens via
navigation helpers.

**Lines:** ~150

### P2: Expression parser

**Scope:** Pratt precedence climbing. Parse atoms (variables, literals,
parenthesized, blocks, lists, tuples, lambdas, if/else, match) and
infix/postfix operators (binary ops, field access, function application,
indexing, pipe-forward, try `?`).

**Test:** Parse `1 + 2 * 3`, `f(x, y)`, `p.x`, `if a { b } else { c }`,
`match x { Some(v) -> v, None -> 0 }`, `|x| x + 1`, `[1, 2, 3]`.
Print AST for each.

**Lines:** ~300

### P3: Type + pattern parser

**Scope:** Parse type annotations (`Int`, `List(Int)`, `Int -> String`,
`Option(T)`, `T?`). Parse patterns (`_`, `x`, `42`, `Some(x)`,
`Point(x: a, y: b)`).

**Test:** Parse `> f(x: Int, y: String) -> Bool { ... }` — params get
type annotations, return type parses correctly. Match arms with
constructor patterns work.

**Lines:** ~150

### P4: Statement parser — all 7 runes

**Scope:** Implement `parse_statement` dispatch and all rune handlers:
- `#` → `parse_type_decl` (ADTs, traits, impls)
- `>` → `parse_definition` (functions, actors, modules)
- `|` → `parse_rule` (clauses, defaults, invariants, scopes)
- `=` → `parse_binding` (simple bindings, monadic binds)
- `~` → `parse_stream` (stream bind, stream subscribe)
- `@` → `parse_annotation` (use, import, depend, print, rust blocks)
- `?` → `parse_prove` (all 6 forms)
- `for` loops

Implement `parse_program` loop.

**Test:** Parse `examples/lexer.runa` — the self-hosting lexer. This
covers ADTs, functions, if/else, match, recursion, lambdas, for loops,
bindings, imports, and effects. If the parser can parse the lexer,
it can parse most of Futuruna.

**Lines:** ~300

### P5: Self-parse — the bootstrapping proof

**Scope:** The parser parses itself. `parse_program(tokenize(read_file("examples/parser.runa")))` produces a valid AST.

**Test:** Round-trip: tokenize parser → parse → count AST nodes → print
summary. No crashes, all statements parsed.

This is the proof that Futuruna is self-sustaining.

**Lines:** ~50 (test harness)

### P6: Verification suite

**Scope:** `examples/parser-audit.runa` with `|` invariants and `?` proofs,
mirroring `examples/lexer-audit.runa`.

Invariants:
- Parsed AST is non-empty for non-empty input
- Every function definition produces a `DFn` node
- Every `#` line produces a `TdADT` or trait/impl node
- Binding count matches `=` rune count in tokens
- Round-trip: parse(tokenize(source)) preserves structure

**Lines:** ~100

---

## Total estimate

~1,000-1,100 lines of Futuruna for the parser + ~100 for the audit.
The self-hosting lexer is 350 lines, so the parser is roughly 3x larger
— which matches the Rust compiler's ratio (~700 lines lexer, ~2,400
lines parser).

## Constructs the parser uses (self-parse requirements)

The parser itself will use these Futuruna features, which it must
therefore be able to parse:

- `# Name = Variant(fields) | Variant(fields)` — ADTs with variants
- `# Name(field: Type)` — named-field structs
- `> func(param: Type) -> RetType { body }` — typed functions
- `= name = value` — bindings
- `@ import ./lexer` — imports
- `@ print(...)` — effects
- `if/else` chains — conditionals
- `match expr { Pat -> body }` — pattern matching
- `Binary operators` — +, ==, !=, &&, ||, >=, <
- `Function calls` — f(x, y)
- `Field access` — st.pos, tok.kind
- `Lambdas` — |x| expr
- `for` loops — iteration over lists
- `List literals` — []
- `push()` — list append
- `String operations` — concatenation, show()
- `Recursion` — tail-recursive loops (TCE applies)

All of these are in the P1-P4 scope. No exotic features needed.

---

## Adversarial Review

### Critique 1: The parser doesn't need all 7 runes in its own source

The design doc opens with "statement dispatch — the seven runes" as if
that's the parser's identity. But the parser *itself* only uses 4 runes:

| Rune | Parser uses it? | Why |
|------|----------------|-----|
| `#`  | Yes | ADT definitions for AST types |
| `>`  | Yes | Every parse function |
| `=`  | Yes | Bindings everywhere |
| `@`  | Yes | `@ import ./lexer`, `@ print(...)` |
| `\|` | No  | Invariants belong in the audit file |
| `?`  | No  | Proofs belong in the audit file |
| `~`  | No  | No streams in a parser |

The parser must be able to *parse* all 7, but it should only *use* the
ones it needs. Don't shoehorn `|` invariants or `?` proofs into the
parser file just to demonstrate them. That's the audit file's job.

**Verdict:** Keep the parser pure. `# > = @` plus `for`, `if/else`,
`match`. The audit file (P6) gets `| ?`. This is not a weakness —
it's discipline.

### Critique 2: Seven result structs is too many

`PExpr`, `PStmt`, `PTy`, `PPat`, `PList`, `PStmts`, `Consumed` — that's
7 wrapper types just for state threading. Each one exists only because
Futuruna can't return `(PState, T)` as a generic pair.

But do we actually need all of them?

- `Consumed` — yes, `consume()` and `expect()` need to return (state, token)
- `PExpr` — yes, expression parsers return (state, Expr)
- `PStmt` — yes, statement parsers return (state, Stmt)
- `PTy` — yes, type parser returns (state, Ty)
- `PPat` — yes, pattern parser returns (state, Pat)
- `PList` — **no** — argument list parsing can return `PExpr` with `EList` as the node, or we can use a dedicated struct but it's the same cost
- `PStmts` — **no** — `parse_program` can just return `List(Stmt)` directly since the final state is discarded

Cut `PList` and `PStmts`. Use `PExpr` with `EList` wrapper for arg lists.
For `parse_program`, return `List(Stmt)` — the caller doesn't need the
final parser state.

**Verdict:** 5 result types: `Consumed`, `PExpr`, `PStmt`, `PTy`, `PPat`.
Acceptable cost of immutability without generics.

### Critique 3: Print-and-continue error handling is wrong

The design says "V1 uses print-and-continue." This is actively dangerous.
A parse error at token 5 will cascade into garbage AST for the remaining
900 tokens. The output would be meaningless.

But full error recovery (Result ADT, propagation at every call site) is
verbose without monadic sugar for custom types.

**The right v1 answer:** Assume valid input. The self-hosting parser's
job is to parse well-formed Futuruna — itself and the lexer. If input is
malformed, crash. That's honest. Error recovery is a v2 concern after
the bootstrapping proof works.

In practice: `expect()` can call `@ print` and return a sentinel.
But for v1, the test programs are always valid.

**Verdict:** Don't handle errors. Don't pretend to. Parse valid input
correctly, crash on invalid input. Honest > polite.

### Critique 4: The AST mirrors the Rust AST too slavishly

The design defines 16 Expr variants, 14 Stmt variants, 7 Ty variants,
5 Pat variants, etc. — a 1:1 port of the Rust compiler's AST. But half
of those variants are unused for self-parse:

**Expr variants the parser never produces for self-parse:**
`ETry`, `ETuple`, `EIndex`, `EEffect` (as expression), `EUnOp`, `EHandle`,
`EConjunction`

**Stmt variants the parser never produces for self-parse:**
`SStreamBind`, `SStreamSub`, `SSend`, `SMonadicBind`, `SRust`, `SDepen`,
`SUse`, `SInvariant`, `SProve`

The question is: should we define all of them anyway? Two arguments:

*For:* The parser should be general-purpose — able to parse any `.runa`
file, not just itself. Defining all variants upfront means we never have
to restructure the AST later.

*Against:* Untested code is broken code. Defining 30 variants but only
testing 15 means half the parser is faith-based.

**Verdict:** Define all variants (they're just type definitions, cheap).
But only IMPLEMENT parsers for what we can test. In the milestones,
parse the lexer first (P4 test), then self-parse (P5). Variants that
neither file uses can be stubbed with a TODO comment and implemented
when a test exercises them.

### Critique 5: The milestone ordering is wrong

P1 (types) → P2 (expressions) → P3 (types+patterns) → P4 (statements)

You can't test expression parsing without statement parsing. Where do
you put `1 + 2`? In a binding: `= x = 1 + 2`. But bindings are P4.
You can't test function parsing without expressions (the body). But
functions are P4 too.

The lexer was testable in isolation because chars → tokens is simple.
The parser's output (AST) is complex and everything is interconnected.

**Better approach: thin vertical slices.**

Each slice delivers a working end-to-end parser for a growing subset:

1. **Slice 1:** `= x = 42` — binding + literal. Proves the pipeline works.
2. **Slice 2:** `= y = 1 + 2 * 3` — binding + Pratt precedence. Proves expressions.
3. **Slice 3:** `> f(x: Int) -> Int { x + 1 }` — function + params + types + block.
4. **Slice 4:** `# Color = Red | Green | Blue` — ADT declarations.
5. **Slice 5:** `for`, `if/else`, `match`, lambdas — control flow.
6. **Slice 6:** `@ import`, `@ print` — annotations.
7. **Slice 7:** Parse `lexer.runa` end-to-end.
8. **Slice 8:** Parse `parser.runa` end-to-end (self-parse).

Each slice is testable the moment it's written. No waiting for P4 to
test P2.

**Verdict:** Replace P1-P4 with vertical slices. Each slice adds both
AST types AND parsers AND a test, all in one step.

### Critique 6: Constructor names — ugly but right

`EVar`, `ELit`, `EApp` look like C macros. But Futuruna flattens all
constructors into one namespace — `Var` would clash between `Expr::Var`
and `Pat::Var`. Prefixes are necessary.

Short prefixes (`E`, `S`, `P`, `Ty`, `L`) are better than long ones
(`ExprVar`, `StmtBind`) because they appear hundreds of times in match
arms. Visual noise matters in a 1000-line file.

**Verdict:** Keep `EVar`, `SBind`, `PVar`, `TyName`, `LInt`. They earn
their brevity.

### Critique 7: What Futuruna opportunities are we *actually* missing?

The design is "translate the Rust parser to Futuruna." That's pragmatic
but leaves Futuruna's unique strengths on the table:

**Tail-call elimination (TCE).** The recursive loops (`expr_loop`,
`parse_program_loop`) get compiled to `loop { ... continue }` by TCE
automatically. The Rust parser uses `while` loops. Futuruna's recursive
style is cleaner AND has the same performance. This is a real win —
the parser demonstrates that recursion is a first-class control flow
strategy in Futuruna, not a performance compromise.

**Rc structural sharing (M25).** AST subtrees get O(1) clone via Rc
automatically. When the parser builds `EBinOp(op, lhs, rhs)`, the `lhs`
is shared, not deep-copied. This is a genuine advantage over languages
where AST construction has hidden O(n) costs.

**Pipe-forward at the top level.** The entry point should be beautiful:
```
= source = read_file("examples/lexer.runa")
= ast = source |> tokenize |> make_state |> parse_program
```
One clean pipeline. This is how Futuruna programs should read.

**The `|` rune for grammar invariants in the audit.** After parsing,
the audit file can assert structural properties of the AST:
```
| all_fns_have_bodies: ast ->
    all(fns_in(ast), |d| match d { DFn(_, _, _, body) -> body != EUnit })
```
Most self-hosting parsers can't do this — their test is "it doesn't
crash." Futuruna can prove structural properties. That's unique.

**What to skip:**
- Logic programming for grammar rules (parser generators are a different
  tool — this would be losing focus)
- Content-addressed AST nodes (M11 — interesting but v2)
- Stream-based token consumption (cute but worse than direct access)
- Monadic parser combinators (we don't have the type system for it)

**Verdict:** Lean into TCE, Rc sharing, pipe-forward, and `|` invariants
in the audit. Don't chase parser-generator novelty.

### Critique 8: The real test isn't "it doesn't crash"

P5 says "parse the parser, count AST nodes, print summary." That's weak.
The real proof is structural:

- Every `>` in the source produces a `DFn` in the AST
- Every `#` produces a `TdADT`
- Every `=` produces a `SBind`
- Function parameter counts match
- AST depth is bounded (no infinite recursion in the tree)

The audit file (P6) should verify these properties. The self-parse test
(P5) should at minimum verify that the number of parsed top-level
statements matches expectations.

**Verdict:** P5 needs a real structural check, not just "no crash."

---

## Revised Milestones

Based on the adversarial review, replace the linear P1-P6 with vertical
slices and honest scope:

### S1: Skeleton — state, navigation, literals, bindings

Parse `= x = 42` and `= name = "hello"`. This forces:
- AST types: `Literal`, `Expr` (just `EVar`, `ELit`), `Stmt` (just `SBind`), `Pat` (just `PVar`)
- Result types: `PState`, `Consumed`, `PExpr`, `PStmt`
- Navigation: `peek`, `advance`, `consume`, `expect`, `skip_semis`
- A `show_expr` pretty-printer (can't verify without seeing output)
- Test: tokenize + parse + print AST for 3 binding statements

~150 lines. Testable immediately.

### S2: Expressions — Pratt climbing + calls + fields

Parse `= y = 1 + 2 * 3` and `= z = f(x, y)` and `= w = p.name`.
Adds:
- `EBinOp`, `EApp`, `EField`, `EList`, `EUnit`
- `parse_expr_prec`, `expr_loop`, `parse_atom`, `parse_arg_list`
- Operator precedence table
- Test: verify parse tree for `1 + 2 * 3` has correct nesting

~200 lines.

### S3: Functions + types + patterns

Parse `> f(x: Int) -> Int { x + 1 }` and `match val { Some(v) -> v }`.
Adds:
- `Ty` (TyName, TyApp, TyArrow), `Pat` (PWild, PCon, PLit), `FParam`
- `PTy`, `PPat` result types
- `Defn` (DFn), `MArm`
- `parse_definition`, `parse_type`, `parse_pattern`, `parse_block_expr`
- `if/else`, `match`, lambda parsing
- Test: parse a function with typed params, return type, and body

~250 lines.

### S4: ADTs + annotations + for loops

Parse `# TokenKind = TkKW | TkIdent | ...` and `@ import ./lexer`.
Adds:
- `TypeDecl` (TdADT), `Vnt`, `VField`
- `SAnnot`, `SImport`, `SFor`
- `parse_type_decl`, `parse_annotation`, `parse_for`
- `parse_program` loop
- **Test: parse `examples/lexer.runa` end-to-end.** This is the first
  real stress test. The lexer uses ADTs, functions, match, if/else,
  recursion, lambdas, for loops, bindings, imports, and effects.

~200 lines.

### S5: Self-parse — the bootstrapping proof

Parse `examples/parser.runa` with itself.
```
= source = read_file("examples/parser.runa")
= ast = source |> tokenize |> make_state |> parse_program
@ print("Self-parse: " + show(length(ast)) + " statements")
```

Structural check: count of `DFn` nodes matches `>` rune count in source.

~50 lines (test harness, integrated into parser.runa itself).

### S6: Audit — `parser-audit.runa`

Verification suite using `|` invariants and `?` proofs. This is where
the remaining runes shine — not in the parser, but in its verification.

~100 lines.

---

## Open questions (post-review)

1. **Mutual recursion (Expr ↔ Stmt):** Must verify M25 handles two types
   that reference each other. Test before committing to the design.

2. **Long ADT lines:** Expr with 16 variants is ~400 chars. Acceptable
   for now. If the formatter ever supports multi-line ADTs, revisit.

3. **Stream/actor/effect parsing:** Not needed for self-parse. Implement
   when a test program exercises them. Don't write untested code.
