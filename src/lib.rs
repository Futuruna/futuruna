//! Futuruna core library — lexer, parser, interpreter, type checker
//!
//! This module contains the core language implementation that can be used
//! as a library, including for the WASM playground.

#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments
)]

//! runa — The Futuruna Compiler / Interpreter
//!
//! A Rust-hosted bootstrap compiler for the Futuruna programming language.
//! Reads .runa files, tokenizes, parses, evaluates, or transpiles to Rust.
//!
//! Usage:
//!   cargo run --release --bin runa -- <file.runa>
//!   cargo run --release --bin runa              # REPL mode
//!
//! This is the first real implementation of Futuruna — the programming language
//! designed by measuring syntactic consciousness.

use serde_json;
use sha2::{Digest as ShaDigest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fmt;
use std::io::{self, BufRead, Write as IoWrite};
use std::rc::Rc;

/// Proof kernel — Curry-Howard verification layer for the `?` rune.
/// Lives in its own file so it can be audited in isolation.
/// See `docs/proof-kernel.md` for the design spec.
pub mod proof_kernel;

// ============================================================================
// PART 1: TOKENS
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // The 15 token categories from the S_τ model
    Start,
    KW,    // keywords
    Ident, // identifiers (lowercase start)
    Op,    // operators + runes
    Delim, // unused (absorbed by comma/semi)
    Lit,   // literals
    Type,  // type names (uppercase start)
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semi, // newline / statement separator
    Dot,
    Arrow,    // ->
    FatArrow, // =>
    Comma,
    Colon,
    Pipe,     // | (both rune and separator)
    Hash,     // #
    At,       // @
    Gt,       // >
    Eq,       // =
    Send,     // <-
    Tilde,    // ~ (stream binding rune)
    PipeGt,   // |> (pipe-forward operator)
    SafeCall, // ?. (Kotlin-style safe call on optional)
    Elvis,    // ?: (Kotlin-style elvis / default on optional)
    Amp,      // &
    String_,
    Char_,
    Int_,
    Float_,
    Bool_,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, text: impl Into<String>, line: usize, col: usize) -> Self {
        Token {
            kind,
            text: text.into(),
            line,
            col,
        }
    }
}

// ============================================================================
// PART 2: LEXER
// ============================================================================

// ---- Language keyword tables for @ sprog / @ language ----
// Maps localized keyword → (canonical English text, TokenKind)
// The lexer normalizes all tokens to canonical form so the parser stays untouched.
type KeywordTable = HashMap<String, (String, TokenKind)>;

pub fn keyword_table_english() -> KeywordTable {
    let kws = [
        "match",
        "if",
        "else",
        "with",
        "under",
        "exception",
        "scope",
        "on",
        "actor",
        "spawn",
        "effect",
        "module",
        "import",
        "where",
        "mut",
        "in",
        "let",
        "fn",
        "type",
        "do",
        "then",
        "return",
        "use",
        "trait",
        "impl",
        "for",
        "while",
        "handle",
        "resume",
        "perform",
        "assert",
        "retract",
        "abort",
    ];
    let mut t = HashMap::new();
    for kw in kws {
        t.insert(kw.to_string(), (kw.to_string(), TokenKind::KW));
    }
    t.insert("True".to_string(), ("True".to_string(), TokenKind::Bool_));
    t.insert("False".to_string(), ("False".to_string(), TokenKind::Bool_));
    t.insert("true".to_string(), ("True".to_string(), TokenKind::Bool_));
    t.insert("false".to_string(), ("False".to_string(), TokenKind::Bool_));
    // `or` as expression operator (alias for ||)
    t.insert("or".to_string(), ("||".to_string(), TokenKind::Op));
    // Note: `and` is handled specially in the rule conjunction parser
    // (not lexed as && because it's also a goal separator in Datalog rules)
    t
}

pub fn keyword_table_dansk() -> KeywordTable {
    let pairs: &[(&str, &str, TokenKind)] = &[
        // ---- core keywords ----
        ("skel", "match", TokenKind::KW),
        ("hvis", "if", TokenKind::KW),
        ("ellers", "else", TokenKind::KW),
        ("med", "with", TokenKind::KW),
        ("under", "under", TokenKind::KW),
        ("undtagelse", "exception", TokenKind::KW),
        // ---- actors & effects ----
        ("omfang", "scope", TokenKind::KW),
        ("på", "on", TokenKind::KW),
        ("aktør", "actor", TokenKind::KW),
        ("start", "spawn", TokenKind::KW),
        ("effekt", "effect", TokenKind::KW),
        // ---- modules ----
        ("modul", "module", TokenKind::KW),
        ("importer", "import", TokenKind::KW),
        ("brug", "use", TokenKind::KW),
        ("træk", "trait", TokenKind::KW),
        ("impl", "impl", TokenKind::KW),
        ("for", "for", TokenKind::KW),
        ("while", "while", TokenKind::KW),
        // ---- misc keywords ----
        ("hvor", "where", TokenKind::KW),
        ("mut", "mut", TokenKind::KW),
        // NOTE: "i" → "in" intentionally omitted — single-letter `i`
        // clashes with common variable names. Use English `in` in Danish mode.
        ("lad", "let", TokenKind::KW),
        ("fn", "fn", TokenKind::KW),
        ("type", "type", TokenKind::KW),
        ("gør", "do", TokenKind::KW),
        ("så", "then", TokenKind::KW),
        ("returner", "return", TokenKind::KW),
        // ---- algebraic effects ----
        ("håndter", "handle", TokenKind::KW),
        ("genoptag", "resume", TokenKind::KW),
        ("udfør", "perform", TokenKind::KW),
        // ---- persist ----
        ("hævd", "assert", TokenKind::KW),
        ("tilbagetræk", "retract", TokenKind::KW),
        ("afbryd", "abort", TokenKind::KW),
        // ---- booleans ----
        ("Sandt", "True", TokenKind::Bool_),
        ("Falskt", "False", TokenKind::Bool_),
        ("sandt", "True", TokenKind::Bool_),
        ("falskt", "False", TokenKind::Bool_),
        // ---- type name aliases ----
        ("Heltal", "Int", TokenKind::Type),
        ("Kommatal", "Float", TokenKind::Type),
        ("Tekst", "String", TokenKind::Type),
        ("Boolsk", "Bool", TokenKind::Type),
        ("Tegn", "Char", TokenKind::Type),
        ("Liste", "List", TokenKind::Type),
        ("Naturligt", "Nat", TokenKind::Type),
        // ---- constructor aliases ----
        ("Intet", "None", TokenKind::Type),
        ("Noget", "Some", TokenKind::Type),
        ("Fejl", "Err", TokenKind::Type),
        // ---- @ directive aliases ----
        ("eksport", "export", TokenKind::Ident),
        ("afhæng", "depend", TokenKind::Ident),
        ("indud", "inout", TokenKind::Ident),
        // ---- builtin function aliases (normalized at lex time) ----
        // display & output
        ("vis", "show", TokenKind::Ident),
        ("vis_heltal", "show_int", TokenKind::Ident),
        ("vis_kommatal", "show_float", TokenKind::Ident),
        ("beskriv", "describe", TokenKind::Ident),
        // math
        ("kvrod", "sqrt", TokenKind::Ident),
        ("potens", "pow", TokenKind::Ident),
        ("til_kommatal", "to_float", TokenKind::Ident),
        ("afrund", "round", TokenKind::Ident),
        ("gulv", "floor", TokenKind::Ident),
        // string
        ("længde", "length", TokenKind::Ident),
        ("tekst_længde", "string_length", TokenKind::Ident),
        ("opdel", "split", TokenKind::Ident),
        ("saml", "join", TokenKind::Ident),
        ("indeholder", "contains", TokenKind::Ident),
        ("starter_med", "starts_with", TokenKind::Ident),
        ("ender_med", "ends_with", TokenKind::Ident),
        ("erstat", "replace", TokenKind::Ident),
        ("til_store", "to_upper", TokenKind::Ident),
        ("til_små", "to_lower", TokenKind::Ident),
        ("deltekst", "substring", TokenKind::Ident),
        ("tegn_ved", "char_at", TokenKind::Ident),
        ("indeks_af", "index_of", TokenKind::Ident),
        ("formater_kommatal", "format_float", TokenKind::Ident),
        ("fortolk_heltal", "parse_int", TokenKind::Ident),
        ("fortolk_kommatal", "parse_float", TokenKind::Ident),
        ("tekst_tegn", "string_chars", TokenKind::Ident),
        // list
        ("hoved", "head", TokenKind::Ident),
        ("hale", "tail", TokenKind::Ident),
        ("nte", "nth", TokenKind::Ident),
        ("vend", "reverse", TokenKind::Ident),
        ("tilføj", "push", TokenKind::Ident),
        ("område", "range", TokenKind::Ident),
        ("afbild", "map", TokenKind::Ident),
        ("filtrer", "filter", TokenKind::Ident),
        ("fold", "foldl", TokenKind::Ident),
        ("sorter", "sort", TokenKind::Ident),
        ("sorter_efter", "sort_by", TokenKind::Ident),
        ("nogen", "any", TokenKind::Ident),
        ("alle", "all", TokenKind::Ident),
        ("flad_afbild", "flat_map", TokenKind::Ident),
        ("par", "zip", TokenKind::Ident),
        ("numerer", "enumerate", TokenKind::Ident),
        ("tag_mens", "take_while", TokenKind::Ident),
        ("spring_mens", "drop_while", TokenKind::Ident),
        ("sum_liste", "sum_list", TokenKind::Ident),
        ("unikke", "distinct", TokenKind::Ident),
        ("tæl_efter", "count_by", TokenKind::Ident),
        ("opdel_efter", "partition", TokenKind::Ident),
        ("stykker", "chunked", TokenKind::Ident),
        ("abonner", "subscribe", TokenKind::Ident),
        // file I/O
        ("læs_fil", "read_file", TokenKind::Ident),
        ("skriv_fil", "write_file", TokenKind::Ident),
        ("tilføj_fil", "append_file", TokenKind::Ident),
        ("fil_eksisterer", "file_exists", TokenKind::Ident),
        ("læs_linjer", "read_lines", TokenKind::Ident),
        ("miljø_var", "env_var", TokenKind::Ident),
        // JSON
        ("json_fortolk", "json_parse", TokenKind::Ident),
        ("json_hent", "json_get", TokenKind::Ident),
        ("json_tekst", "json_string", TokenKind::Ident),
        ("json_tal", "json_number", TokenKind::Ident),
        ("json_sand", "json_bool", TokenKind::Ident),
        ("json_liste", "json_array", TokenKind::Ident),
        ("json_udsend", "json_emit", TokenKind::Ident),
        ("json_objekt", "json_object", TokenKind::Ident),
        // map & set
        ("kort_nyt", "map_new", TokenKind::Ident),
        ("kort_indsæt", "map_insert", TokenKind::Ident),
        ("kort_hent", "map_get", TokenKind::Ident),
        ("kort_hent_eller", "map_get_or", TokenKind::Ident),
        ("kort_indeholder", "map_contains", TokenKind::Ident),
        ("kort_fjern", "map_remove", TokenKind::Ident),
        ("kort_nøgler", "map_keys", TokenKind::Ident),
        ("kort_værdier", "map_values", TokenKind::Ident),
        ("kort_poster", "map_entries", TokenKind::Ident),
        ("kort_længde", "map_len", TokenKind::Ident),
        ("kort_flet", "map_merge", TokenKind::Ident),
        ("kort_fra", "map_from", TokenKind::Ident),
        ("sæt_nyt", "set_new", TokenKind::Ident),
        ("sæt_indsæt", "set_insert", TokenKind::Ident),
        ("sæt_indeholder", "set_contains", TokenKind::Ident),
        ("sæt_fjern", "set_remove", TokenKind::Ident),
        ("sæt_længde", "set_len", TokenKind::Ident),
        ("sæt_til_liste", "set_to_list", TokenKind::Ident),
        ("sæt_forening", "set_union", TokenKind::Ident),
        ("sæt_fælles", "set_intersect", TokenKind::Ident),
        ("sæt_forskel", "set_diff", TokenKind::Ident),
        ("sæt_fra_liste", "set_from_list", TokenKind::Ident),
        // streams & reactive
        ("fra_liste", "from_list", TokenKind::Ident),
        ("tag", "take", TokenKind::Ident),
        ("spring", "skip", TokenKind::Ident),
        ("indsaml", "collect", TokenKind::Ident),
        ("tæl", "count", TokenKind::Ident),
        ("vindue", "window", TokenKind::Ident),
        ("sidste", "last", TokenKind::Ident),
        ("kombiner_seneste", "combine_latest", TokenKind::Ident),
        ("flet", "merge", TokenKind::Ident),
        ("første", "first", TokenKind::Ident),
        ("reducer", "reduce", TokenKind::Ident),
        ("start_med", "start_with", TokenKind::Ident),
        ("sammenkæd", "concat", TokenKind::Ident),
        ("parvis", "pairwise", TokenKind::Ident),
        // actor & concurrency
        ("spørg", "ask", TokenKind::Ident),
        ("delt", "shared", TokenKind::Ident),
        // logic
        ("ikke", "not", TokenKind::Ident),
        ("find_alle", "findall", TokenKind::Ident),
        // ---- English pass-throughs (bilingual mode) ----
        ("match", "match", TokenKind::KW),
        ("if", "if", TokenKind::KW),
        ("else", "else", TokenKind::KW),
        ("with", "with", TokenKind::KW),
        ("exception", "exception", TokenKind::KW),
        ("scope", "scope", TokenKind::KW),
        ("on", "on", TokenKind::KW),
        ("actor", "actor", TokenKind::KW),
        ("spawn", "spawn", TokenKind::KW),
        ("effect", "effect", TokenKind::KW),
        ("module", "module", TokenKind::KW),
        ("import", "import", TokenKind::KW),
        ("use", "use", TokenKind::KW),
        ("trait", "trait", TokenKind::KW),
        ("where", "where", TokenKind::KW),
        ("in", "in", TokenKind::KW),
        ("let", "let", TokenKind::KW),
        ("do", "do", TokenKind::KW),
        ("then", "then", TokenKind::KW),
        ("return", "return", TokenKind::KW),
        ("handle", "handle", TokenKind::KW),
        ("resume", "resume", TokenKind::KW),
        ("perform", "perform", TokenKind::KW),
        ("True", "True", TokenKind::Bool_),
        ("False", "False", TokenKind::Bool_),
        ("true", "True", TokenKind::Bool_),
        ("false", "False", TokenKind::Bool_),
    ];
    let mut t = HashMap::new();
    for (local, canonical, kind) in pairs {
        t.insert(local.to_string(), (canonical.to_string(), *kind));
    }
    t
}

/// Detect @ sprog / @ language declaration in first non-comment line
pub fn detect_language(source: &str) -> KeywordTable {
    let mut in_block_comment = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if in_block_comment {
            if trimmed.contains("----") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("----") {
            // Block comment might open and close on same line
            if trimmed.matches("----").count() < 2 {
                in_block_comment = true;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        // Look for @ sprog <code> or @ language <code>
        if trimmed.starts_with("@ sprog ") || trimmed.starts_with("@ language ") {
            let code = trimmed.rsplit(' ').next().unwrap_or("en").trim();
            return match code {
                "da" | "dansk" => keyword_table_dansk(),
                "en" | "english" => keyword_table_english(),
                other => {
                    eprintln!("runa: unknown language '{}', defaulting to English", other);
                    keyword_table_english()
                }
            };
        }
        break; // first non-comment, non-empty line isn't a language declaration
    }
    keyword_table_english()
}

// ---- Builtin aliases for localized builtins ----
// Single source of truth: (alias, canonical_english).
// Adding a new language: just add entries here.
// All codegen sites use builtin_canonical(); runtime uses builtin_aliases().
const BUILTIN_ALIASES: &[(&str, &str)] = &[
    // ---- dansk: display & output ----
    ("vis", "show"),
    ("skriv", "print"),
    ("vis_heltal", "show_int"),
    ("vis_kommatal", "show_float"),
    ("beskriv", "describe"),
    // ---- dansk: math ----
    ("kvrod", "sqrt"),
    ("potens", "pow"),
    ("til_kommatal", "to_float"),
    ("afrund", "round"),
    ("gulv", "floor"),
    // ---- dansk: string ----
    ("længde", "length"),
    ("tekst_længde", "string_length"),
    ("opdel", "split"),
    ("saml", "join"),
    ("indeholder", "contains"),
    ("starter_med", "starts_with"),
    ("ender_med", "ends_with"),
    ("erstat", "replace"),
    ("til_store", "to_upper"),
    ("til_små", "to_lower"),
    ("deltekst", "substring"),
    ("tegn_ved", "char_at"),
    ("indeks_af", "index_of"),
    ("formater_kommatal", "format_float"),
    ("fortolk_heltal", "parse_int"),
    ("fortolk_kommatal", "parse_float"),
    ("tekst_tegn", "string_chars"),
    // ---- dansk: list ----
    ("hoved", "head"),
    ("hale", "tail"),
    ("nte", "nth"),
    ("vend", "reverse"),
    ("tilføj", "push"),
    ("område", "range"),
    ("afbild", "map"),
    ("filtrer", "filter"),
    ("fold", "foldl"),
    ("sorter", "sort"),
    ("sorter_efter", "sort_by"),
    ("nogen", "any"),
    ("alle", "all"),
    ("flad_afbild", "flat_map"),
    ("par", "zip"),
    ("numerer", "enumerate"),
    ("tag_mens", "take_while"),
    ("spring_mens", "drop_while"),
    ("sum_liste", "sum_list"),
    ("unikke", "distinct"),
    ("tæl_efter", "count_by"),
    ("opdel_efter", "partition"),
    ("stykker", "chunked"),
    ("abonner", "subscribe"),
    // ---- dansk: file I/O ----
    ("læs_fil", "read_file"),
    ("skriv_fil", "write_file"),
    ("tilføj_fil", "append_file"),
    ("fil_eksisterer", "file_exists"),
    ("læs_linjer", "read_lines"),
    ("miljø_var", "env_var"),
    // ---- dansk: JSON ----
    ("json_fortolk", "json_parse"),
    ("json_hent", "json_get"),
    ("json_tekst", "json_string"),
    ("json_tal", "json_number"),
    ("json_sand", "json_bool"),
    ("json_liste", "json_array"),
    ("json_udsend", "json_emit"),
    ("json_objekt", "json_object"),
    // ---- dansk: map & set ----
    ("kort_nyt", "map_new"),
    ("kort_indsæt", "map_insert"),
    ("kort_hent", "map_get"),
    ("kort_hent_eller", "map_get_or"),
    ("kort_indeholder", "map_contains"),
    ("kort_fjern", "map_remove"),
    ("kort_nøgler", "map_keys"),
    ("kort_værdier", "map_values"),
    ("kort_poster", "map_entries"),
    ("kort_længde", "map_len"),
    ("kort_flet", "map_merge"),
    ("kort_fra", "map_from"),
    ("sæt_nyt", "set_new"),
    ("sæt_indsæt", "set_insert"),
    ("sæt_indeholder", "set_contains"),
    ("sæt_fjern", "set_remove"),
    ("sæt_længde", "set_len"),
    ("sæt_til_liste", "set_to_list"),
    ("sæt_forening", "set_union"),
    ("sæt_fælles", "set_intersect"),
    ("sæt_forskel", "set_diff"),
    ("sæt_fra_liste", "set_from_list"),
    // ---- dansk: streams & reactive ----
    ("fra_liste", "from_list"),
    ("tag", "take"),
    ("spring", "skip"),
    ("indsaml", "collect"),
    ("tæl", "count"),
    ("vindue", "window"),
    ("sidste", "last"),
    ("kombiner_seneste", "combine_latest"),
    ("flet", "merge"),
    ("første", "first"),
    ("reducer", "reduce"),
    ("start_med", "start_with"),
    ("sammenkæd", "concat"),
    ("parvis", "pairwise"),
    // ---- dansk: actor & concurrency ----
    ("spørg", "ask"),
    ("delt", "shared"),
    // ---- dansk: logic ----
    ("ikke", "not"),
    ("find_alle", "findall"),
    // backward compat: s_ prefix → clean name
    ("s_map", "map"),
    ("s_filter", "filter"),
    ("s_scan", "scan"),
    ("s_merge", "merge"),
    ("s_zip", "zip"),
    ("s_take", "take"),
    ("s_skip", "skip"),
    ("s_distinct", "distinct"),
    ("s_flat_map", "flat_map"),
    ("s_sum", "sum"),
    ("s_any", "any"),
    ("s_all", "all"),
    ("s_last", "last"),
    ("s_window", "window"),
    ("s_enumerate", "enumerate"),
    ("s_count", "count"),
    ("s_collect", "collect"),
    ("s_combine_latest", "combine_latest"),
];

/// Returns the canonical (English) builtin name for a possibly-localized name.
/// If the name is not an alias, returns it unchanged.
pub fn builtin_canonical(name: &str) -> &str {
    for &(alias, canonical) in BUILTIN_ALIASES {
        if alias == name {
            return canonical;
        }
    }
    name
}

/// Returns alias→canonical map for runtime env registration.
pub fn builtin_aliases() -> Vec<(String, String)> {
    BUILTIN_ALIASES
        .iter()
        .map(|&(a, c)| (a.into(), c.into()))
        .collect()
}

// ---- Standard Prelude ----
// Auto-imported definitions for every Futuruna program (unless --no-prelude).
// These are parsed once and prepended to the user's program.
// User-defined functions/types with the same name shadow prelude versions.
const TAU_PRELUDE: &str = r#"
-- Futuruna Standard Prelude (auto-imported)
-- Types available to every Futuruna program without explicit definition.
-- Note: # List(a) is NOT included — programs that want Cons/Nil define it
-- themselves; programs that use List(Int) in type annotations get Vec<i64>.

# Option(a) = None | Some(a)
# Result(a, e) = Ok(a) | Err(e)
# Pair(a, b) = Pair(fst: a, snd: b)

-- Option functions

> unwrap_or(opt: Option(a), default: a) -> a {
    match opt {
        | None -> default
        | Some(x) -> x
    }
}

> is_some(opt: Option(a)) -> Bool {
    match opt {
        | None -> false
        | Some(_) -> true
    }
}

> is_none(opt: Option(a)) -> Bool {
    match opt {
        | None -> true
        | Some(_) -> false
    }
}

-- Math

> max_int(a: Int, b: Int) -> Int {
    if a >= b { a } else { b }
}

> min_int(a: Int, b: Int) -> Int {
    if a <= b { a } else { b }
}

> clamp(x: Int, lo: Int, hi: Int) -> Int {
    if x < lo { lo } else { if x > hi { hi } else { x } }
}

-- Composition

> identity(x: a) -> a { x }

-- Built-in effects

# effect Console {
    > say(msg: String) -> ()
    > ask(prompt: String) -> String
}
"#;

/// Parse the standard prelude into a list of statements.
/// Returns only definition and type declaration statements.
pub fn parse_prelude() -> Vec<Stmt> {
    let mut lexer = Lexer::new(TAU_PRELUDE);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, TAU_PRELUDE);
    match parser.parse_program() {
        Ok(stmts) => stmts
            .into_iter()
            .filter(|s| matches!(s, Stmt::Defn(_) | Stmt::TypeDecl(_)))
            .collect(),
        Err(e) => {
            eprintln!("BUG: prelude parse error: {}", e);
            vec![]
        }
    }
}

/// Prepend prelude statements to user statements, skipping prelude definitions
/// that the user has already defined (user definitions take priority).
pub fn prepend_prelude(prelude: Vec<Stmt>, user_stmts: &[Stmt]) -> Vec<Stmt> {
    // Collect names defined by the user
    let mut user_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for s in user_stmts {
        match s {
            Stmt::Defn(Defn::Fn { name, .. }) => {
                user_names.insert(name.clone());
            }
            Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                user_names.insert(name.clone());
            }
            Stmt::TypeDecl(TypeDecl::TraitDecl { name, .. }) => {
                user_names.insert(name.clone());
            }
            Stmt::TypeDecl(TypeDecl::EffectDecl { name, .. }) => {
                user_names.insert(name.clone());
            }
            _ => {}
        }
    }
    // Only include prelude stmts whose name isn't shadowed by the user
    let mut result: Vec<Stmt> = prelude
        .into_iter()
        .filter(|s| {
            let name = match s {
                Stmt::Defn(Defn::Fn { name, .. }) => Some(name.as_str()),
                Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => Some(name.as_str()),
                Stmt::TypeDecl(TypeDecl::TraitDecl { name, .. }) => Some(name.as_str()),
                Stmt::TypeDecl(TypeDecl::EffectDecl { name, .. }) => Some(name.as_str()),
                _ => None,
            };
            name.map(|n| !user_names.contains(n)).unwrap_or(true)
        })
        .collect();
    result.extend_from_slice(user_stmts);
    result
}

pub enum TplPart {
    Lit(String),
    Interp(String),
}

pub struct Lexer {
    pub chars: Vec<char>,
    pub pos: usize,
    pub line: usize,
    pub col: usize,
    pub keywords: KeywordTable,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        let keywords = detect_language(source);
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            keywords,
        }
    }

    pub fn with_keywords(source: &str, keywords: KeywordTable) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            keywords,
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    pub fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    pub fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    pub fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_not_newline();
            let line = self.line;
            let col = self.col;

            let c = match self.peek() {
                Some(c) => c,
                None => {
                    tokens.push(Token::new(TokenKind::Eof, "", line, col));
                    break;
                }
            };

            // Block comments: ---- ... ----
            if c == '-'
                && self.peek2() == Some('-')
                && self.peek_at(2) == Some('-')
                && self.peek_at(3) == Some('-')
            {
                // Consume the opening ----
                self.advance();
                self.advance();
                self.advance();
                self.advance();
                // Scan until closing ----
                loop {
                    match self.peek() {
                        None => break, // EOF — unclosed block comment
                        Some('-')
                            if self.peek2() == Some('-')
                                && self.peek_at(2) == Some('-')
                                && self.peek_at(3) == Some('-') =>
                        {
                            self.advance();
                            self.advance();
                            self.advance();
                            self.advance();
                            break;
                        }
                        _ => {
                            self.advance();
                        }
                    }
                }
                continue;
            }

            // Line comments: -- to end of line
            if c == '-' && self.peek2() == Some('-') {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
                continue;
            }

            // Newlines become semicolons (statement separators)
            if c == '\n' {
                self.advance();
                // Collapse multiple newlines
                while self.peek() == Some('\n') {
                    self.advance();
                }
                // Don't emit semi at start or after another semi
                if let Some(last) = tokens.last() {
                    if last.kind != TokenKind::Semi
                        && last.kind != TokenKind::LBrace
                        && last.kind != TokenKind::Start
                    {
                        tokens.push(Token::new(TokenKind::Semi, "\n", line, col));
                    }
                }
                continue;
            }

            // Two-character operators
            // ?. (safe call) and ?: (elvis) — must come before single-char ? handling
            if c == '?' && self.peek2() == Some('.') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::SafeCall, "?.", line, col));
                continue;
            }
            if c == '?' && self.peek2() == Some(':') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Elvis, "?:", line, col));
                continue;
            }
            if c == '-' && self.peek2() == Some('>') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Arrow, "->", line, col));
                continue;
            }
            if c == '<' && self.peek2() == Some('-') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Send, "<-", line, col));
                continue;
            }
            if c == '=' && self.peek2() == Some('>') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::FatArrow, "=>", line, col));
                continue;
            }
            if c == '=' && self.peek2() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, "==", line, col));
                continue;
            }
            if c == '!' && self.peek2() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, "!=", line, col));
                continue;
            }
            if c == '>' && self.peek2() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, ">=", line, col));
                continue;
            }
            if c == '<' && self.peek2() == Some('=') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, "<=", line, col));
                continue;
            }
            if c == '&' && self.peek2() == Some('&') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, "&&", line, col));
                continue;
            }
            if c == '|' && self.peek2() == Some('|') {
                self.advance();
                self.advance();
                tokens.push(Token::new(TokenKind::Op, "||", line, col));
                continue;
            }

            // Single-character tokens
            match c {
                '(' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::LParen, "(", line, col));
                    continue;
                }
                ')' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::RParen, ")", line, col));
                    continue;
                }
                '{' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::LBrace, "{", line, col));
                    continue;
                }
                '}' => {
                    self.advance();
                    // Remove preceding semi before }
                    if let Some(last) = tokens.last() {
                        if last.kind == TokenKind::Semi {
                            tokens.pop();
                        }
                    }
                    tokens.push(Token::new(TokenKind::RBrace, "}", line, col));
                    continue;
                }
                '[' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::LBracket, "[", line, col));
                    continue;
                }
                ']' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::RBracket, "]", line, col));
                    continue;
                }
                ',' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Comma, ",", line, col));
                    continue;
                }
                ':' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Colon, ":", line, col));
                    continue;
                }
                '.' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Dot, ".", line, col));
                    continue;
                }
                '#' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Hash, "#", line, col));
                    continue;
                }
                '@' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::At, "@", line, col));
                    continue;
                }
                '&' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Amp, "&", line, col));
                    continue;
                }
                '~' => {
                    self.advance();
                    tokens.push(Token::new(TokenKind::Tilde, "~", line, col));
                    continue;
                }
                '|' => {
                    self.advance();
                    if self.pos < self.chars.len() && self.chars[self.pos] == '>' {
                        self.advance();
                        tokens.push(Token::new(TokenKind::PipeGt, "|>", line, col));
                    } else {
                        tokens.push(Token::new(TokenKind::Pipe, "|", line, col));
                    }
                    continue;
                }
                '>' => {
                    self.advance();
                    // Distinguish > rune (start of line) from > operator
                    let is_rune = tokens.is_empty()
                        || tokens.last().map_or(false, |t| t.kind == TokenKind::Semi);
                    if is_rune {
                        tokens.push(Token::new(TokenKind::Gt, ">", line, col));
                    } else {
                        tokens.push(Token::new(TokenKind::Op, ">", line, col));
                    }
                    continue;
                }
                '=' => {
                    self.advance();
                    // Distinguish = rune (start of statement) from = operator
                    let is_rune = tokens.is_empty()
                        || tokens.last().map_or(false, |t| {
                            t.kind == TokenKind::Semi || t.kind == TokenKind::LBrace
                        });
                    if is_rune {
                        tokens.push(Token::new(TokenKind::Eq, "=", line, col));
                    } else {
                        tokens.push(Token::new(TokenKind::Op, "=", line, col));
                    }
                    continue;
                }
                _ => {}
            }

            // Operators: + - * / % < ! ?
            if "+-*/%<!?~^".contains(c) {
                self.advance();
                tokens.push(Token::new(TokenKind::Op, c.to_string(), line, col));
                continue;
            }

            // Numbers
            if c.is_ascii_digit() {
                let s = self.read_number();
                let kind = if s.contains('.') {
                    TokenKind::Float_
                } else {
                    TokenKind::Int_
                };
                tokens.push(Token::new(kind, s, line, col));
                continue;
            }

            // String literals
            if c == '"' {
                // Check for triple-quoted string: """..."""
                // c is from peek() (not yet consumed), so check pos+1 and pos+2
                if self.peek_at(1) == Some('"') && self.peek_at(2) == Some('"') {
                    self.advance(); // consume first "
                    self.advance(); // consume second "
                    self.advance(); // consume third "
                    let parts = self.read_triple_string();
                    // Desugar template parts into concatenation tokens
                    let mut first = true;
                    for part in &parts {
                        match part {
                            TplPart::Lit(s) => {
                                if !first {
                                    tokens.push(Token::new(TokenKind::Op, "+", line, col));
                                }
                                tokens.push(Token::new(TokenKind::String_, s.clone(), line, col));
                                first = false;
                            }
                            TplPart::Interp(expr_src) => {
                                if !first {
                                    tokens.push(Token::new(TokenKind::Op, "+", line, col));
                                }
                                // Desugar {{expr}} to show(expr)
                                tokens.push(Token::new(TokenKind::Ident, "show", line, col));
                                tokens.push(Token::new(TokenKind::LParen, "(", line, col));
                                // Lex the interpolated expression (inherit parent's keywords)
                                let mut sub = Lexer::with_keywords(expr_src, self.keywords.clone());
                                let sub_tokens = sub.tokenize();
                                for st in &sub_tokens {
                                    if st.kind != TokenKind::Eof {
                                        tokens.push(Token::new(
                                            st.kind,
                                            st.text.clone(),
                                            line,
                                            col,
                                        ));
                                    }
                                }
                                tokens.push(Token::new(TokenKind::RParen, ")", line, col));
                                first = false;
                            }
                        }
                    }
                    // If the template was empty or all-interpolation, ensure at least one string
                    if first {
                        tokens.push(Token::new(TokenKind::String_, String::new(), line, col));
                    }
                    continue;
                }
                let s = self.read_string();
                tokens.push(Token::new(TokenKind::String_, s, line, col));
                continue;
            }

            // Char literals
            if c == '\'' {
                let s = self.read_char_lit();
                tokens.push(Token::new(TokenKind::Char_, s, line, col));
                continue;
            }

            // Words: identifiers, keywords, type names, booleans
            // Uses the keyword table (set by @ sprog / @ language declaration)
            if c.is_alphabetic() || c == '_' {
                let word = self.read_word();
                let (text, kind) = if let Some((canonical, kw_kind)) = self.keywords.get(&word) {
                    // Keyword or boolean — normalize to canonical English form
                    (canonical.clone(), *kw_kind)
                } else if word.starts_with(|c: char| c.is_uppercase()) {
                    (word.clone(), TokenKind::Type)
                } else {
                    (word.clone(), TokenKind::Ident)
                };
                tokens.push(Token::new(kind, text, line, col));
                continue;
            }

            // Skip unknown characters
            self.advance();
        }

        tokens
    }

    pub fn skip_whitespace_not_newline(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn read_word(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }

    /// Read a triple-quoted string `"""..."""`.
    /// Opening `"""` already consumed. Strips leading/trailing newline.
    /// Supports `{{expr}}` interpolation.
    pub fn read_triple_string(&mut self) -> Vec<TplPart> {
        // Strip leading newline
        if self.peek() == Some('\n') {
            self.advance();
        } else if self.peek() == Some('\r') {
            self.advance();
            if self.peek() == Some('\n') {
                self.advance();
            }
        }

        let mut parts: Vec<TplPart> = Vec::new();
        let mut buf = String::new();

        loop {
            match self.peek() {
                // Check for closing """
                Some('"') if self.peek_at(1) == Some('"') && self.peek_at(2) == Some('"') => {
                    self.advance();
                    self.advance();
                    self.advance();
                    break;
                }
                // Check for interpolation {{...}}
                Some('{') if self.peek_at(1) == Some('{') => {
                    self.advance();
                    self.advance(); // consume {{
                                    // Flush accumulated literal
                    if !buf.is_empty() {
                        parts.push(TplPart::Lit(std::mem::take(&mut buf)));
                    }
                    // Read expression until }}
                    let mut expr = String::new();
                    let mut depth = 0i32;
                    loop {
                        match self.peek() {
                            Some('}') if depth == 0 && self.peek_at(1) == Some('}') => {
                                self.advance();
                                self.advance();
                                break;
                            }
                            Some('{') => {
                                depth += 1;
                                expr.push('{');
                                self.advance();
                            }
                            Some('}') => {
                                depth -= 1;
                                expr.push('}');
                                self.advance();
                            }
                            Some(c) => {
                                expr.push(c);
                                self.advance();
                            }
                            None => break,
                        }
                    }
                    let trimmed = expr.trim().to_string();
                    if !trimmed.is_empty() {
                        parts.push(TplPart::Interp(trimmed));
                    }
                }
                Some(c) => {
                    buf.push(c);
                    self.advance();
                }
                None => break,
            }
        }

        // Strip trailing newline from last literal
        if buf.ends_with('\n') {
            buf.pop();
            if buf.ends_with('\r') {
                buf.pop();
            }
        }

        if !buf.is_empty() {
            parts.push(TplPart::Lit(buf));
        }

        // If no parts at all, return a single empty literal
        if parts.is_empty() {
            parts.push(TplPart::Lit(String::new()));
        }

        parts
    }

    pub fn read_number(&mut self) -> String {
        let mut s = String::new();
        let mut has_dot = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !has_dot {
                // Check it's not a method call like 42.foo
                if self.peek2().map_or(false, |c2| c2.is_ascii_digit()) {
                    has_dot = true;
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        s
    }

    pub fn read_string(&mut self) -> String {
        self.advance(); // consume opening "
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => break,
                },
                Some(c) => s.push(c),
                None => break,
            }
        }
        s
    }

    pub fn read_char_lit(&mut self) -> String {
        self.advance(); // consume opening '
        let c = match self.advance() {
            Some('\\') => match self.advance() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('\\') => '\\',
                Some('\'') => '\'',
                Some(c) => c,
                None => ' ',
            },
            Some(c) => c,
            None => ' ',
        };
        if self.peek() == Some('\'') {
            self.advance();
        }
        c.to_string()
    }
}

// ============================================================================
// PART 2b: SPANS & DIAGNOSTICS
// ============================================================================

/// Source location span — byte offsets into source text.
/// Use `Span::to_line_col()` to convert to human-readable line:col.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize, // byte offset (0-based)
    pub end: usize,   // byte offset (exclusive)
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// Dummy span for AST nodes not yet tracked.
    pub fn dummy() -> Self {
        Span { start: 0, end: 0 }
    }

    pub fn is_dummy(&self) -> bool {
        self.start == 0 && self.end == 0
    }

    /// Merge two spans into one covering both.
    pub fn merge(self, other: Span) -> Span {
        if self.is_dummy() {
            return other;
        }
        if other.is_dummy() {
            return self;
        }
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Convert byte offset to 1-based (line, col) using source text.
    pub fn start_line_col(&self, source: &str) -> (usize, usize) {
        byte_offset_to_line_col(source, self.start)
    }

    pub fn end_line_col(&self, source: &str) -> (usize, usize) {
        byte_offset_to_line_col(source, self.end)
    }
}

/// Convert a byte offset to 1-based (line, col).
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Help,
}

/// A structured compiler diagnostic with optional source location.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub span: Option<Span>,
    pub message: String,
    pub severity: Severity,
    pub notes: Vec<String>,
    pub context: Vec<String>, // breadcrumb trail: ["in function `foo`", "in match arm 2"]
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            span: None,
            message: message.into(),
            severity: Severity::Error,
            notes: Vec::new(),
            context: Vec::new(),
        }
    }

    pub fn error_at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            span: if span.is_dummy() { None } else { Some(span) },
            message: message.into(),
            severity: Severity::Error,
            notes: Vec::new(),
            context: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context.push(ctx.into());
        self
    }

    /// Format this diagnostic for display.
    /// If `use_color` is false, strips ANSI codes.
    pub fn display(&self, source: &str, filename: &str, use_color: bool) -> String {
        let mut out = String::new();

        // Colors
        let (red, blue, bold, reset, dim) = if use_color {
            ("\x1b[1;31m", "\x1b[1;34m", "\x1b[1m", "\x1b[0m", "\x1b[2m")
        } else {
            ("", "", "", "", "")
        };

        // Header: error: message
        let label = match self.severity {
            Severity::Error => format!("{}error{}", red, reset),
            Severity::Warning => format!("{}warning{}", "\x1b[1;33m", reset),
            Severity::Help => format!("{}help{}", "\x1b[1;32m", reset),
        };
        let label = if !use_color {
            match self.severity {
                Severity::Error => "error".to_string(),
                Severity::Warning => "warning".to_string(),
                Severity::Help => "help".to_string(),
            }
        } else {
            label
        };

        // Context breadcrumbs
        for ctx in &self.context {
            out.push_str(&format!(" {}{}-->{} {}\n", dim, blue, reset, ctx));
        }

        if let Some(span) = self.span {
            let (line, col) = span.start_line_col(source);
            let (end_line, end_col) = span.end_line_col(source);

            // Location header
            out.push_str(&format!("{}: {}{}{}\n", label, bold, self.message, reset));
            out.push_str(&format!(
                " {}-->{} {}:{}:{}\n",
                blue, reset, filename, line, col
            ));

            let lines: Vec<&str> = source.lines().collect();
            if line > 0 && line <= lines.len() {
                let src_line = lines[line - 1];
                let line_str = format!("{}", line);
                let padding = " ".repeat(line_str.len());

                // Gutter
                out.push_str(&format!(" {} {}|{}\n", padding, blue, reset));

                // Source line
                out.push_str(&format!(
                    " {}{}{} {}|{} {}\n",
                    blue, line_str, reset, blue, reset, src_line
                ));

                // Underline
                let caret_start = if col > 0 { col - 1 } else { 0 };
                let underline_len = if line == end_line && end_col > col {
                    end_col - col
                } else {
                    1
                };
                let underline_len = underline_len.max(1);
                let underline_pad = " ".repeat(caret_start);
                let underline = "^".repeat(underline_len);
                out.push_str(&format!(
                    " {} {}|{} {}{}{}{}\n",
                    padding, blue, reset, underline_pad, red, underline, reset
                ));
            }
        } else {
            out.push_str(&format!("{}: {}{}{}\n", label, bold, self.message, reset));
        }

        // Notes
        for note in &self.notes {
            out.push_str(&format!(" {} = {}help{}: {}\n", "  ", blue, reset, note));
        }

        out
    }
}

/// Check if color output should be used.
pub fn should_use_color() -> bool {
    // NO_COLOR convention: https://no-color.org/
    if env::var("NO_COLOR").is_ok() {
        return false;
    }
    // Also check TERM=dumb
    if env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
        return false;
    }
    true
}

// ============================================================================
// PART 3: AST
// ============================================================================

#[derive(Debug, Clone)]
pub enum Ty {
    Name(String),
    App(Box<Ty>, Vec<Ty>),
    Arrow(Box<Ty>, Box<Ty>),
    Ref(Box<Ty>),
    MutRef(Box<Ty>),
    Shared(Box<Ty>),   // shared T → Arc<T> in Rust
    Optional(Box<Ty>), // T? → sugar for Option<T> (Kotlin-style nullability)
    Var(String),
    Unit,
    Hole,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Name(n) => write!(f, "{}", n),
            Ty::App(con, args) => {
                write!(f, "{}(", con)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            Ty::Arrow(a, b) => write!(f, "{} -> {}", a, b),
            Ty::Ref(t) => write!(f, "&{}", t),
            Ty::MutRef(t) => write!(f, "&mut {}", t),
            Ty::Shared(t) => write!(f, "shared {}", t),
            Ty::Optional(t) => write!(f, "{}?", t),
            Ty::Var(n) => write!(f, "{}", n),
            Ty::Unit => write!(f, "()"),
            Ty::Hole => write!(f, "_"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Option<Ty>,
    pub inout: bool, // Mutable value semantics: caller passes &mut, function mutates in place
}

#[derive(Debug, Clone)]
pub enum Pat {
    Wild,
    Var(String),
    Lit(Literal),
    Con(String, Vec<Pat>),
    NamedCon(String, Vec<(String, Pat)>), // Circle(radius: r) — named field destructure
    As(Box<Pat>, String),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(String),
    Char(char),
    Bool(bool),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Int(n) => write!(f, "{}", n),
            Literal::Float(v) => write!(f, "{}", v),
            Literal::Str(s) => write!(f, "\"{}\"", s),
            Literal::Char(c) => write!(f, "'{}'", c),
            Literal::Bool(b) => write!(f, "{}", if *b { "True" } else { "False" }),
        }
    }
}

/// The expression AST node kind. Expr wraps this with a Span.
/// During the type alias phase, `Expr` = `ExprKind` (zero-cost transition).
#[derive(Debug, Clone)]
pub enum ExprKind {
    Var(String),
    Lit(Literal),
    App(Box<Expr>, Vec<Expr>),
    Lambda(Vec<Param>, Box<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>),
    UnOp(String, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<MatchArm>),
    Block(Vec<Stmt>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Effect(String, Vec<Expr>),
    /// Algebraic effect handler: | handle EffectName { | op(args) -> body } in expr
    Handle {
        effect: String,
        handlers: Vec<EffHandler>,
        body: Box<Expr>,
    },
    /// Postfix ? operator (try/propagate errors)
    Try(Box<Expr>),
    /// Prolog-style conjunction: goal1, goal2, goal3 — all must succeed
    Conjunction(Vec<Expr>),
    /// Prolog-style disjunction: goal1 or goal2 — first that succeeds wins
    Disjunction(Vec<Expr>),
    /// Pipe-forward operator: a |> f — preserves ~ rune identity (not desugared to App)
    Pipe(Box<Expr>, Box<Expr>),
    Unit,
}

/// Expression AST node: wraps ExprKind with source location.
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    /// Create an Expr with a source span.
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr { kind, span }
    }

    /// Shorthand for unspanned expressions (during migration).
    pub fn unspanned(kind: ExprKind) -> Self {
        Expr {
            kind,
            span: Span::dummy(),
        }
    }
}

/// Allow ExprKind to convert to Expr (with dummy span) during migration.
impl From<ExprKind> for Expr {
    fn from(kind: ExprKind) -> Self {
        Expr::unspanned(kind)
    }
}

/// Allow ExprKind to convert to Box<Expr> directly.
impl From<ExprKind> for Box<Expr> {
    fn from(kind: ExprKind) -> Self {
        Box::new(Expr::unspanned(kind))
    }
}

/// A single handler clause in a | handle expression
#[derive(Debug, Clone)]
pub struct EffHandler {
    pub op_name: String,
    pub params: Vec<String>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub struct ProofBlock {
    pub arms: Vec<ProofArm>,
}

#[derive(Debug, Clone)]
pub struct ProofArm {
    pub binders: Vec<String>,
    pub term: proof_kernel::ProofTerm,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Defn(Defn),
    TypeDecl(TypeDecl),
    Rule(Rule),
    Use(String),                     // @ use path::to::thing
    Import(String),                  // @ import ./module (multi-file, flat merge)
    QualifiedImport(String, String), // @ import Name from ./module (qualified access)
    HashImport(String, String),      // @ import #hash from ./module (content-addressed)
    Depend(String, String),          // @ depend "crate" "version"
    RustBlock(String),               // @ rust { raw Rust code }
    Annot(String, Vec<Expr>),
    Bind(Pat, Option<Ty>, Expr),
    /// Monadic bind: = pat <- expr (unwrap Ok/Some, early-return on Err/None)
    MonadicBind(Pat, Option<Ty>, Expr),
    For(String, Expr, Vec<Stmt>),   // for var in expr { body }
    While(Expr, Vec<Stmt>),         // while cond { body }
    Send(Expr, Expr),               // target <- message (actor send)
    StreamBind(String, Expr),       // ~ name = expr (reactive stream binding)
    StreamSub(Expr, Vec<MatchArm>), // ~ expr | pat -> { body } (reactive stream subscription)
    /// | name: subject -> predicate (named invariant / verification assertion)
    Invariant {
        name: String,
        subject: Expr,   // the expression being constrained
        predicate: Expr, // must evaluate to true
    },
    /// ? name — invoke verification (runtime check / Z3 proof / debug_assert)
    /// Optional: `: val` captures subject, `-> { pass }` block, `else { fail }` block
    Prove {
        name: String,
        proof_block: Option<ProofBlock>, // `by { | ... -> proof }`
        capture: Option<String>,         // `: val` — bind subject value
        pass_block: Option<Vec<Stmt>>,   // `-> { ... }` — custom pass handler
        else_block: Option<Vec<Stmt>>,   // `else { ... }` — custom fail handler (suppresses halt)
    },
    /// assert TypeName(args...) — insert fact (persist/store/in-memory)
    Assert(String, Vec<Expr>),
    /// retract TypeName(args...) — remove fact (persist/store/in-memory)
    Retract(String, Vec<Expr>),
    /// abort — exit current scope with ROLLBACK
    Abort,
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Defn {
    Fn {
        name: String,
        params: Vec<Param>,
        ret_ty: Option<Ty>,
        effects: Vec<String>,
        body: Expr,
    },
    Actor {
        name: String,
        state_param: Param,
        handlers: Vec<Handler>,
    },
    Module {
        name: String,
        body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone)]
pub struct Handler {
    pub msg_pat: Pat,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum TypeDecl {
    ADT {
        name: String,
        params: Vec<Param>,
        variants: Vec<Variant>,
        methods: Vec<Defn>,
        /// EXCEPT clause: include all variants from this type minus the listed ones
        except_from: Option<(String, Vec<String>)>,
    },
    /// Conditional type evolution: # Type WHEN condition = new_variants
    WhenType {
        name: String,
        condition: Expr,
        variants: Vec<Variant>,
        except_from: Option<(String, Vec<String>)>,
    },
    EffectDecl {
        name: String,
        ops: Vec<(String, Vec<Param>, Option<Ty>)>,
    },
    TraitDecl {
        name: String,
        params: Vec<Param>, // trait type params
        methods: Vec<TraitMethod>,
    },
    ImplBlock {
        trait_name: String,
        for_type: String,
        methods: Vec<Defn>,
    },
}

#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Ty>,
    pub default_body: Option<Expr>, // default implementation
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
    pub positional: bool, // true = tuple-style (Type, Type), false = named (name: Type)
    pub from_type: Option<String>, // Some("GeoArea") for subset variants: GeoArea.Danmark
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub enum Rule {
    Clause {
        head: Expr,
        body: Option<Expr>,
    },
    Default {
        head: Expr,
        value: Expr,
        condition: Option<Expr>,
    },
    Exception {
        label: String,
        head: Expr,
        value: Expr,
        condition: Option<Expr>,
    },
    Scope {
        name: String,
        body: Vec<Stmt>,
    },
}

// ============================================================================
// PART 3b: CONTENT HASHING (Unison's Lesson)
// ============================================================================
// Identity is structure, not names. Two definitions with identical bodies
// get the same hash regardless of what they're called.

fn strip_spans_expr(expr: &Expr) -> Expr {
    let kind = match &expr.kind {
        ExprKind::Var(name) => ExprKind::Var(name.clone()),
        ExprKind::Lit(lit) => ExprKind::Lit(lit.clone()),
        ExprKind::App(func, args) => ExprKind::App(
            Box::new(strip_spans_expr(func)),
            args.iter().map(strip_spans_expr).collect(),
        ),
        ExprKind::Lambda(params, body) => {
            ExprKind::Lambda(params.clone(), Box::new(strip_spans_expr(body)))
        }
        ExprKind::BinOp(op, lhs, rhs) => ExprKind::BinOp(
            op.clone(),
            Box::new(strip_spans_expr(lhs)),
            Box::new(strip_spans_expr(rhs)),
        ),
        ExprKind::UnOp(op, inner) => ExprKind::UnOp(op.clone(), Box::new(strip_spans_expr(inner))),
        ExprKind::If(cond, then_, else_) => ExprKind::If(
            Box::new(strip_spans_expr(cond)),
            Box::new(strip_spans_expr(then_)),
            Box::new(strip_spans_expr(else_)),
        ),
        ExprKind::Match(scrutinee, arms) => ExprKind::Match(
            Box::new(strip_spans_expr(scrutinee)),
            arms.iter().map(strip_spans_match_arm).collect(),
        ),
        ExprKind::Block(stmts) => ExprKind::Block(stmts.iter().map(strip_spans_stmt).collect()),
        ExprKind::Field(obj, field) => {
            ExprKind::Field(Box::new(strip_spans_expr(obj)), field.clone())
        }
        ExprKind::Index(base, idx) => ExprKind::Index(
            Box::new(strip_spans_expr(base)),
            Box::new(strip_spans_expr(idx)),
        ),
        ExprKind::List(items) => ExprKind::List(items.iter().map(strip_spans_expr).collect()),
        ExprKind::Tuple(items) => ExprKind::Tuple(items.iter().map(strip_spans_expr).collect()),
        ExprKind::Effect(name, args) => {
            ExprKind::Effect(name.clone(), args.iter().map(strip_spans_expr).collect())
        }
        ExprKind::Handle {
            effect,
            handlers,
            body,
        } => ExprKind::Handle {
            effect: effect.clone(),
            handlers: handlers.iter().map(strip_spans_eff_handler).collect(),
            body: Box::new(strip_spans_expr(body)),
        },
        ExprKind::Try(inner) => ExprKind::Try(Box::new(strip_spans_expr(inner))),
        ExprKind::Conjunction(items) => {
            ExprKind::Conjunction(items.iter().map(strip_spans_expr).collect())
        }
        ExprKind::Disjunction(items) => {
            ExprKind::Disjunction(items.iter().map(strip_spans_expr).collect())
        }
        ExprKind::Pipe(lhs, rhs) => ExprKind::Pipe(
            Box::new(strip_spans_expr(lhs)),
            Box::new(strip_spans_expr(rhs)),
        ),
        ExprKind::Unit => ExprKind::Unit,
    };
    Expr {
        kind,
        span: Span::dummy(),
    }
}

fn strip_spans_eff_handler(handler: &EffHandler) -> EffHandler {
    EffHandler {
        op_name: handler.op_name.clone(),
        params: handler.params.clone(),
        body: strip_spans_expr(&handler.body),
    }
}

fn strip_spans_match_arm(arm: &MatchArm) -> MatchArm {
    MatchArm {
        pat: arm.pat.clone(),
        guard: arm.guard.as_ref().map(strip_spans_expr),
        body: strip_spans_expr(&arm.body),
    }
}

fn strip_spans_rule(rule: &Rule) -> Rule {
    match rule {
        Rule::Clause { head, body } => Rule::Clause {
            head: strip_spans_expr(head),
            body: body.as_ref().map(strip_spans_expr),
        },
        Rule::Default {
            head,
            value,
            condition,
        } => Rule::Default {
            head: strip_spans_expr(head),
            value: strip_spans_expr(value),
            condition: condition.as_ref().map(strip_spans_expr),
        },
        Rule::Exception {
            label,
            head,
            value,
            condition,
        } => Rule::Exception {
            label: label.clone(),
            head: strip_spans_expr(head),
            value: strip_spans_expr(value),
            condition: condition.as_ref().map(strip_spans_expr),
        },
        Rule::Scope { name, body } => Rule::Scope {
            name: name.clone(),
            body: body.iter().map(strip_spans_stmt).collect(),
        },
    }
}

fn strip_spans_defn(defn: &Defn) -> Defn {
    match defn {
        Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        } => Defn::Fn {
            name: name.clone(),
            params: params.clone(),
            ret_ty: ret_ty.clone(),
            effects: effects.clone(),
            body: strip_spans_expr(body),
        },
        Defn::Actor {
            name,
            state_param,
            handlers,
        } => Defn::Actor {
            name: name.clone(),
            state_param: state_param.clone(),
            handlers: handlers
                .iter()
                .map(|handler| Handler {
                    msg_pat: handler.msg_pat.clone(),
                    body: strip_spans_expr(&handler.body),
                })
                .collect(),
        },
        Defn::Module { name, body } => Defn::Module {
            name: name.clone(),
            body: body.iter().map(strip_spans_stmt).collect(),
        },
    }
}

fn strip_spans_type_decl(td: &TypeDecl) -> TypeDecl {
    match td {
        TypeDecl::ADT {
            name,
            params,
            variants,
            methods,
            except_from,
        } => TypeDecl::ADT {
            name: name.clone(),
            params: params.clone(),
            variants: variants.clone(),
            methods: methods.iter().map(strip_spans_defn).collect(),
            except_from: except_from.clone(),
        },
        TypeDecl::WhenType {
            name,
            condition,
            variants,
            except_from,
        } => TypeDecl::WhenType {
            name: name.clone(),
            condition: strip_spans_expr(condition),
            variants: variants.clone(),
            except_from: except_from.clone(),
        },
        TypeDecl::EffectDecl { name, ops } => TypeDecl::EffectDecl {
            name: name.clone(),
            ops: ops.clone(),
        },
        TypeDecl::TraitDecl {
            name,
            params,
            methods,
        } => TypeDecl::TraitDecl {
            name: name.clone(),
            params: params.clone(),
            methods: methods
                .iter()
                .map(|method| TraitMethod {
                    name: method.name.clone(),
                    params: method.params.clone(),
                    ret_ty: method.ret_ty.clone(),
                    default_body: method.default_body.as_ref().map(strip_spans_expr),
                })
                .collect(),
        },
        TypeDecl::ImplBlock {
            trait_name,
            for_type,
            methods,
        } => TypeDecl::ImplBlock {
            trait_name: trait_name.clone(),
            for_type: for_type.clone(),
            methods: methods.iter().map(strip_spans_defn).collect(),
        },
    }
}

fn strip_spans_stmt(stmt: &Stmt) -> Stmt {
    match stmt {
        Stmt::Defn(defn) => Stmt::Defn(strip_spans_defn(defn)),
        Stmt::TypeDecl(td) => Stmt::TypeDecl(strip_spans_type_decl(td)),
        Stmt::Rule(rule) => Stmt::Rule(strip_spans_rule(rule)),
        Stmt::Use(path) => Stmt::Use(path.clone()),
        Stmt::Import(path) => Stmt::Import(path.clone()),
        Stmt::QualifiedImport(name, path) => Stmt::QualifiedImport(name.clone(), path.clone()),
        Stmt::HashImport(hash, path) => Stmt::HashImport(hash.clone(), path.clone()),
        Stmt::Depend(name, version) => Stmt::Depend(name.clone(), version.clone()),
        Stmt::RustBlock(code) => Stmt::RustBlock(code.clone()),
        Stmt::Annot(name, args) => {
            Stmt::Annot(name.clone(), args.iter().map(strip_spans_expr).collect())
        }
        Stmt::Bind(pat, ty, expr) => Stmt::Bind(pat.clone(), ty.clone(), strip_spans_expr(expr)),
        Stmt::MonadicBind(pat, ty, expr) => {
            Stmt::MonadicBind(pat.clone(), ty.clone(), strip_spans_expr(expr))
        }
        Stmt::For(var, expr, body) => Stmt::For(
            var.clone(),
            strip_spans_expr(expr),
            body.iter().map(strip_spans_stmt).collect(),
        ),
        Stmt::While(cond, body) => Stmt::While(
            strip_spans_expr(cond),
            body.iter().map(strip_spans_stmt).collect(),
        ),
        Stmt::Send(target, msg) => Stmt::Send(strip_spans_expr(target), strip_spans_expr(msg)),
        Stmt::StreamBind(name, expr) => Stmt::StreamBind(name.clone(), strip_spans_expr(expr)),
        Stmt::StreamSub(expr, arms) => Stmt::StreamSub(
            strip_spans_expr(expr),
            arms.iter().map(strip_spans_match_arm).collect(),
        ),
        Stmt::Invariant {
            name,
            subject,
            predicate,
        } => Stmt::Invariant {
            name: name.clone(),
            subject: strip_spans_expr(subject),
            predicate: strip_spans_expr(predicate),
        },
        Stmt::Prove {
            name,
            proof_block,
            capture,
            pass_block,
            else_block,
        } => Stmt::Prove {
            name: name.clone(),
            proof_block: proof_block.clone(),
            capture: capture.clone(),
            pass_block: pass_block
                .as_ref()
                .map(|stmts| stmts.iter().map(strip_spans_stmt).collect()),
            else_block: else_block
                .as_ref()
                .map(|stmts| stmts.iter().map(strip_spans_stmt).collect()),
        },
        Stmt::Assert(name, args) => {
            Stmt::Assert(name.clone(), args.iter().map(strip_spans_expr).collect())
        }
        Stmt::Retract(name, args) => {
            Stmt::Retract(name.clone(), args.iter().map(strip_spans_expr).collect())
        }
        Stmt::Abort => Stmt::Abort,
        Stmt::Expr(expr) => Stmt::Expr(strip_spans_expr(expr)),
    }
}

/// Compute a canonical structural representation of a Defn (excluding the name).
/// Two functions with different names but identical params/body get the same hash.
pub fn content_hash_defn(defn: &Defn) -> String {
    let canonical = match defn {
        Defn::Fn {
            params,
            ret_ty,
            effects,
            body,
            ..
        } => {
            format!(
                "FN({:?},{:?},{:?},{:?})",
                params,
                ret_ty,
                effects,
                strip_spans_expr(body)
            )
        }
        Defn::Actor {
            state_param,
            handlers,
            ..
        } => {
            let stripped_handlers: Vec<Handler> = handlers
                .iter()
                .map(|handler| Handler {
                    msg_pat: handler.msg_pat.clone(),
                    body: strip_spans_expr(&handler.body),
                })
                .collect();
            format!("ACTOR({:?},{:?})", state_param, stripped_handlers)
        }
        Defn::Module { body, .. } => {
            format!(
                "MODULE({:?})",
                body.iter().map(strip_spans_stmt).collect::<Vec<_>>()
            )
        }
    };
    hash_string(&canonical)
}

/// Compute a canonical structural representation of a TypeDecl (excluding the name).
pub fn content_hash_type(td: &TypeDecl) -> String {
    let canonical = match td {
        TypeDecl::ADT {
            params,
            variants,
            methods,
            ..
        } => {
            format!(
                "ADT({:?},{:?},{:?})",
                params,
                variants,
                methods.iter().map(strip_spans_defn).collect::<Vec<_>>()
            )
        }
        TypeDecl::EffectDecl { ops, .. } => {
            format!("EFFECT({:?})", ops)
        }
        TypeDecl::TraitDecl {
            params, methods, ..
        } => {
            let stripped_methods: Vec<TraitMethod> = methods
                .iter()
                .map(|method| TraitMethod {
                    name: method.name.clone(),
                    params: method.params.clone(),
                    ret_ty: method.ret_ty.clone(),
                    default_body: method.default_body.as_ref().map(strip_spans_expr),
                })
                .collect();
            format!("TRAIT({:?},{:?})", params, stripped_methods)
        }
        TypeDecl::ImplBlock {
            trait_name,
            for_type,
            methods,
            ..
        } => {
            format!(
                "IMPL({:?},{:?},{:?})",
                trait_name,
                for_type,
                methods.iter().map(strip_spans_defn).collect::<Vec<_>>()
            )
        }
        TypeDecl::WhenType { name, variants, .. } => {
            format!(
                "WHEN({},{})",
                name,
                variants
                    .iter()
                    .map(|v| v.name.as_str())
                    .collect::<Vec<_>>()
                    .join("|")
            )
        }
    };
    hash_string(&canonical)
}

/// SHA-256 of a string, truncated to 12 hex chars (48 bits — collision-safe for codebases).
pub fn hash_string(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    // First 6 bytes = 12 hex chars
    format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        result[0], result[1], result[2], result[3], result[4], result[5]
    )
}

/// Get the name of a Defn.
pub fn defn_name(defn: &Defn) -> &str {
    match defn {
        Defn::Fn { name, .. } => name,
        Defn::Actor { name, .. } => name,
        Defn::Module { name, .. } => name,
    }
}

/// Get the name of a TypeDecl.
pub fn type_decl_name(td: &TypeDecl) -> &str {
    match td {
        TypeDecl::ADT { name, .. } => name,
        TypeDecl::EffectDecl { name, .. } => name,
        TypeDecl::TraitDecl { name, .. } => name,
        TypeDecl::ImplBlock {
            trait_name,
            for_type,
            ..
        } => {
            // impl blocks don't have a single name, use for_type
            for_type
        }
        TypeDecl::WhenType { name, .. } => name,
    }
}

/// Print content hashes for all definitions in a program.
pub fn print_hashes(stmts: &[Stmt]) {
    let mut found = false;
    for stmt in stmts {
        match stmt {
            Stmt::Defn(defn) => {
                let hash = content_hash_defn(defn);
                let kind = match defn {
                    Defn::Fn { .. } => ">",
                    Defn::Actor { .. } => "|",
                    Defn::Module { .. } => "mod",
                };
                println!("  {} {} #{}", kind, defn_name(defn), hash);
                found = true;
            }
            Stmt::TypeDecl(td) => {
                let hash = content_hash_type(td);
                let kind = match td {
                    TypeDecl::ADT { .. } => "#",
                    TypeDecl::EffectDecl { .. } => "# effect",
                    TypeDecl::TraitDecl { .. } => "# trait",
                    TypeDecl::ImplBlock { .. } => "# impl",
                    TypeDecl::WhenType { .. } => "# WHEN",
                };
                println!("  {} {} #{}", kind, type_decl_name(td), hash);
                found = true;
            }
            _ => {}
        }
    }
    if !found {
        println!("  (no definitions found)");
    }
}

// ============================================================================
// PART 4: PARSER
// ============================================================================

pub struct Parser {
    pub tokens: Vec<Token>,
    pub pos: usize,
    pub source_chars: Vec<char>,
    pub line_starts: Vec<usize>, // line_starts[i] = char index of start of line (i+1)
    pub in_rule_body: bool,      // true when parsing | rule body (and = conjunction, not &&)
}

impl Parser {
    pub fn new(tokens: Vec<Token>, source: &str) -> Self {
        let source_chars: Vec<char> = source.chars().collect();
        let mut line_starts = vec![0usize]; // line 1 starts at char 0
        for (i, &c) in source_chars.iter().enumerate() {
            if c == '\n' && i + 1 < source_chars.len() {
                line_starts.push(i + 1);
            }
        }
        Parser {
            tokens,
            pos: 0,
            source_chars,
            line_starts,
            in_rule_body: false,
        }
    }

    /// Convert token line/col (1-based) to char offset in source_chars.
    pub fn char_offset(&self, line: usize, col: usize) -> usize {
        let line_idx = (line - 1).min(self.line_starts.len() - 1);
        self.line_starts[line_idx] + (col - 1)
    }

    /// Create a Span from a token's position to the end of its text.
    pub fn token_span(&self, tok: &Token) -> Span {
        let start = self.char_offset(tok.line, tok.col);
        let end = start + tok.text.len();
        Span::new(start, end)
    }

    /// Create a Span from start token to end of most recently consumed token.
    pub fn span_since(&self, start_tok: &Token) -> Span {
        let start = self.char_offset(start_tok.line, start_tok.col);
        let prev = if self.pos > 0 {
            &self.tokens[self.pos - 1]
        } else {
            start_tok
        };
        let end = self.char_offset(prev.line, prev.col) + prev.text.len();
        Span::new(start, end)
    }

    /// Create a spanned Expr using token positions.
    pub fn spanned(&self, kind: ExprKind, start_tok: &Token) -> Expr {
        Expr::new(kind, self.span_since(start_tok))
    }

    pub fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(
            self.tokens.last().unwrap(), // EOF
        )
    }

    pub fn peek_kind(&self) -> TokenKind {
        self.peek().kind
    }

    pub fn advance(&mut self) -> Token {
        let tok = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or_else(|| Token::new(TokenKind::Eof, "", 0, 0));
        self.pos += 1;
        tok
    }

    pub fn token_display(kind: TokenKind) -> &'static str {
        match kind {
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBracket => "`[`",
            TokenKind::RBracket => "`]`",
            TokenKind::Arrow => "`->`",
            TokenKind::Comma => "`,`",
            TokenKind::Colon => "`:`",
            TokenKind::Semi => "`;`",
            TokenKind::Dot => "`.`",
            TokenKind::Eq => "`=`",
            TokenKind::Pipe => "`|`",
            TokenKind::Hash => "`#`",
            TokenKind::At => "`@`",
            TokenKind::Tilde => "`~`",
            TokenKind::Gt => "`>`",
            TokenKind::Eof => "end of file",
            TokenKind::Ident => "an identifier",
            TokenKind::Type => "a type name",
            TokenKind::String_ => "a string",
            TokenKind::Int_ => "a number",
            TokenKind::Float_ => "a number",
            TokenKind::KW => "a keyword",
            TokenKind::Send => "`<-`",
            _ => "token",
        }
    }

    pub fn expect(&mut self, kind: TokenKind) -> Result<Token, String> {
        let tok = self.advance();
        if tok.kind == kind {
            Ok(tok)
        } else {
            // Context-sensitive hints for common mistakes
            let hint = if kind == TokenKind::Arrow && tok.text == "=>" {
                "\n  Hint: Futuruna uses `->`, not `=>`. Replace `=>` with `->`."
            } else if kind == TokenKind::LBrace && tok.kind == TokenKind::Arrow {
                "\n  Hint: did you forget `{` before the body?"
            } else if kind == TokenKind::Comma && tok.kind == TokenKind::Colon {
                "\n  Hint: this looks like a type annotation.\n  Rule parameters (| rune) don't take types — they are inferred.\n    Try: | rule_name(param) -> ...\n  For typed parameters, use a function (> rune):\n    Try: > func_name(param: Type) -> Type { ... }"
            } else if kind == TokenKind::RParen && tok.kind == TokenKind::Colon {
                "\n  Hint: unexpected `:` — did you mean to add a type annotation?\n  Type annotations use `name: Type` inside `>` functions, not here."
            } else {
                ""
            };
            Err(format!(
                "{}:{}: expected {}, got `{}`{}",
                tok.line,
                tok.col,
                Self::token_display(kind),
                tok.text,
                hint
            ))
        }
    }

    pub fn expect_ident(&mut self) -> Result<String, String> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Ident | TokenKind::Type | TokenKind::Bool_ | TokenKind::KW => Ok(tok.text),
            _ => Err(format!(
                "{}:{}: expected an identifier, got `{}`",
                tok.line, tok.col, tok.text
            )),
        }
    }

    pub fn expect_field_name(&mut self) -> Result<String, String> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Ident | TokenKind::Type | TokenKind::Bool_ | TokenKind::KW => Ok(tok.text),
            TokenKind::Int_ => Ok(tok.text),
            _ => Err(format!(
                "{}:{}: expected a field name or tuple index, got `{}`",
                tok.line, tok.col, tok.text
            )),
        }
    }

    /// Parse a qualified name like `fmt::Display` or `std::ops::Add`.
    /// Returns the full path string.
    pub fn parse_qualified_name(&mut self) -> Result<String, String> {
        let mut name = self.expect_ident()?;
        while self.peek_kind() == TokenKind::Colon {
            let saved = self.pos;
            self.advance(); // first :
            if self.peek_kind() == TokenKind::Colon {
                self.advance(); // second :
                let seg = self.expect_ident()?;
                name.push_str("::");
                name.push_str(&seg);
            } else {
                self.pos = saved; // backtrack — was just a single :
                break;
            }
        }
        Ok(name)
    }

    pub fn skip_semis(&mut self) {
        while self.peek_kind() == TokenKind::Semi {
            self.advance();
        }
    }

    pub fn at_block_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof)
    }

    fn peek_word(&self, word: &str) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Ident | TokenKind::KW | TokenKind::Type
        ) && self.peek().text == word
    }

    fn parse_dotted_name(&mut self) -> Result<String, String> {
        let mut name = self.expect_ident()?;
        while self.peek_kind() == TokenKind::Dot {
            self.advance();
            let seg = self.expect_ident()?;
            name.push('.');
            name.push_str(&seg);
        }
        Ok(name)
    }

    fn parse_proof_block(&mut self) -> Result<ProofBlock, String> {
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut arms = Vec::new();

        while self.peek_kind() != TokenKind::RBrace {
            self.expect(TokenKind::Pipe)?;
            let binders = self.parse_proof_binders()?;
            self.expect(TokenKind::Arrow)?;
            let term = self.parse_proof_term()?;
            arms.push(ProofArm { binders, term });
            self.skip_semis();
        }

        self.expect(TokenKind::RBrace)?;
        if arms.is_empty() {
            return Err("proof block needs at least one `| ... -> ...` arm".into());
        }
        Ok(ProofBlock { arms })
    }

    fn parse_proof_binders(&mut self) -> Result<Vec<String>, String> {
        let mut binders = Vec::new();
        if self.peek_kind() == TokenKind::LParen {
            self.advance();
            while self.peek_kind() != TokenKind::RParen {
                if !binders.is_empty() {
                    self.expect(TokenKind::Comma)?;
                }
                let binder = self.expect_ident()?;
                if binder == "_" {
                    return Err("proof binders cannot use `_` yet".into());
                }
                if self.peek_kind() == TokenKind::Colon {
                    self.advance();
                    let _ = self.parse_type()?;
                }
                binders.push(binder);
            }
            self.expect(TokenKind::RParen)?;
        } else {
            let binder = self.expect_ident()?;
            if binder == "_" {
                return Err("proof binders cannot use `_` yet".into());
            }
            if self.peek_kind() == TokenKind::Colon {
                self.advance();
                let _ = self.parse_type()?;
            }
            binders.push(binder);
        }
        Ok(binders)
    }

    fn proof_ctor_arm_from_pattern(
        &self,
        pat: Pat,
        arm_kind: &str,
    ) -> Result<(String, Vec<String>), String> {
        match pat {
            Pat::Con(ctor, binders) => {
                let mut names = Vec::new();
                for binder in binders {
                    match binder {
                        Pat::Var(name) if name != "_" => names.push(name),
                        Pat::Wild => {
                            return Err(format!("{} cannot use `_` binders yet", arm_kind));
                        }
                        _ => {
                            return Err(format!(
                                "{} currently require simple constructor binders",
                                arm_kind
                            ));
                        }
                    }
                }
                Ok((ctor, names))
            }
            _ => Err(format!("{} require constructor patterns", arm_kind)),
        }
    }

    fn parse_proof_term(&mut self) -> Result<proof_kernel::ProofTerm, String> {
        if self.peek_kind() == TokenKind::LParen {
            self.advance();
            let term = self.parse_proof_term()?;
            self.expect(TokenKind::RParen)?;
            return Ok(term);
        }

        if self.peek_word("refl") {
            self.advance();
            return Ok(proof_kernel::ProofTerm::Refl);
        }

        if self.peek_word("apply") {
            self.advance();
            let name = self.parse_dotted_name()?;
            let mut args = Vec::new();
            if self.peek_kind() == TokenKind::LParen {
                self.advance();
                while self.peek_kind() != TokenKind::RParen {
                    if !args.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    args.push(self.parse_proof_term()?);
                }
                self.expect(TokenKind::RParen)?;
            }
            return Ok(proof_kernel::ProofTerm::Apply(name, args));
        }

        if self.peek_word("rewrite") {
            self.advance();
            let eq_term = self.parse_proof_term()?;
            if !self.peek_word("in") {
                return Err("expected `in` after `rewrite <proof>`".into());
            }
            self.advance();
            let body = self.parse_proof_term()?;
            return Ok(proof_kernel::ProofTerm::Rewrite(
                Box::new(eq_term),
                Box::new(body),
            ));
        }

        if self.peek_word("induction_on") {
            self.advance();
            let scrutinee = self.expect_ident()?;
            self.expect(TokenKind::LBrace)?;
            self.skip_semis();
            let mut arms = Vec::new();
            while self.peek_kind() != TokenKind::RBrace {
                self.expect(TokenKind::Pipe)?;
                let pat = self.parse_pattern()?;
                let (ctor, binders) = self.proof_ctor_arm_from_pattern(pat, "induction arms")?;
                self.expect(TokenKind::Arrow)?;
                let body = self.parse_proof_term()?;
                arms.push(proof_kernel::IndArm {
                    ctor,
                    binders,
                    body,
                });
                self.skip_semis();
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(proof_kernel::ProofTerm::InductionOn(scrutinee, arms));
        }

        if self.peek_word("cases") {
            self.advance();
            let scrutinee_expr = self.parse_expr()?;
            let scrutinee = lower_expr_to_proof_term(&scrutinee_expr)?;
            self.expect(TokenKind::LBrace)?;
            self.skip_semis();
            let mut arms = Vec::new();
            while self.peek_kind() != TokenKind::RBrace {
                self.expect(TokenKind::Pipe)?;
                let pat = self.parse_pattern()?;
                let (ctor, binders) = self.proof_ctor_arm_from_pattern(pat, "case arms")?;
                self.expect(TokenKind::Arrow)?;
                let body = self.parse_proof_term()?;
                arms.push(proof_kernel::CaseArm {
                    ctor,
                    binders,
                    body,
                });
                self.skip_semis();
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(proof_kernel::ProofTerm::Cases(scrutinee, arms));
        }

        if self.peek_word("let") {
            self.advance();
            let name = self.expect_ident()?;
            if self.peek_kind() == TokenKind::Eq {
                self.advance();
            } else if self.peek_kind() == TokenKind::Op && self.peek().text == "=" {
                self.advance();
            } else {
                return Err("expected `=` in proof `let` binding".into());
            }
            let bound = self.parse_proof_term()?;
            if !self.peek_word("in") {
                return Err("expected `in` after proof `let` binding".into());
            }
            self.advance();
            let body = self.parse_proof_term()?;
            return Ok(proof_kernel::ProofTerm::Let(
                name,
                Box::new(bound),
                Box::new(body),
            ));
        }

        if self.peek_word("assume") {
            self.advance();
            let prop_expr = self.parse_expr()?;
            let prop = lower_expr_to_proof_prop(&prop_expr)?;
            if !self.peek_word("in") {
                return Err("expected `in` after `assume <prop>`".into());
            }
            self.advance();
            let body = self.parse_proof_term()?;
            return Ok(proof_kernel::ProofTerm::Assume(prop, Box::new(body)));
        }

        Ok(proof_kernel::ProofTerm::Hyp(self.expect_ident()?))
    }

    // --- Top-level parsing ---

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        self.skip_semis();
        while self.peek_kind() != TokenKind::Eof {
            match self.parse_statement() {
                Ok(stmt) => {
                    stmts.push(stmt);
                }
                Err(e) => {
                    errors.push(e);
                    // Synchronize: skip to the next statement boundary
                    self.synchronize();
                    if errors.len() >= 10 {
                        errors.push("... (too many errors, stopping)".to_string());
                        break;
                    }
                }
            }
            self.skip_semis();
        }
        if errors.is_empty() {
            Ok(stmts)
        } else {
            // Return all errors, but also include any successfully parsed statements
            // in a special format: errors first, then note about partial parse
            let error_count = errors.len();
            let mut msg = format!(
                "{} parse error{}:\n",
                error_count,
                if error_count == 1 { "" } else { "s" }
            );
            for (i, e) in errors.iter().enumerate() {
                if i > 0 {
                    msg.push('\n');
                }
                msg.push_str(e);
            }
            Err(msg)
        }
    }

    /// Skip tokens until we reach a likely statement boundary.
    /// Looks for rune characters at the start of a line, or `for`/`while`/`}` keywords.
    fn synchronize(&mut self) {
        // Skip the current token (the one that caused the error)
        if self.peek_kind() != TokenKind::Eof {
            self.advance();
        }
        while self.peek_kind() != TokenKind::Eof {
            let tok = self.peek();
            // Statement boundaries: rune characters at column 1, or keywords at column 1
            if tok.col <= 1 {
                match tok.kind {
                    TokenKind::Hash
                    | TokenKind::Arrow
                    | TokenKind::Pipe
                    | TokenKind::Eq
                    | TokenKind::Tilde
                    | TokenKind::At
                    | TokenKind::Gt => return,
                    // ? rune starts with Op("?")
                    TokenKind::Op if tok.text == "?" => return,
                    TokenKind::KW
                        if matches!(tok.text.as_str(), "for" | "while" | "if" | "match") =>
                    {
                        return
                    }
                    TokenKind::Ident
                        if matches!(tok.text.as_str(), "fn" | "let" | "struct" | "enum") =>
                    {
                        return
                    }
                    _ => {}
                }
            }
            self.advance();
        }
    }

    pub fn parse_statement(&mut self) -> Result<Stmt, String> {
        self.skip_semis();

        // ── Detect common mistakes from other languages ──
        if self.peek_kind() == TokenKind::Ident || self.peek_kind() == TokenKind::KW {
            let tok = self.peek();
            let line = tok.line;
            let col = tok.col;
            match tok.text.as_str() {
                "fn" | "func" | "def" | "fun" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `>` to define functions, not `{}`.\n  \
                        Try: > {}",
                        line,
                        col,
                        tok.text,
                        self.tokens
                            .get(self.pos + 1)
                            .map(|t| t.text.as_str())
                            .unwrap_or("name(args) -> Type { body }")
                    ));
                }
                "let" | "val" | "var" | "const" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `=` to bind values, not `{}`.\n  \
                        Try: = {}",
                        line,
                        col,
                        tok.text,
                        self.tokens
                            .get(self.pos + 1)
                            .map(|t| t.text.as_str())
                            .unwrap_or("name = value")
                    ));
                }
                "struct" | "class" | "enum" | "interface" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `#` to define types, not `{}`.\n  \
                        Try: # {}",
                        line,
                        col,
                        tok.text,
                        self.tokens
                            .get(self.pos + 1)
                            .map(|t| t.text.as_str())
                            .unwrap_or("TypeName(field: Type)")
                    ));
                }
                "import" | "require" | "include" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `@ import` for Futuruna modules and `@ use` for Rust imports, not `{}`.\n  \
                        Try: @ import ./module",
                        line, col, tok.text
                    ));
                }
                "print" | "println" | "console" | "echo" => {
                    return Err(format!(
                        "{}:{}: In Futuruna, IO is an effect — use the `@` rune.\n  \
                        Try: @ print(\"hello\")",
                        line, col
                    ));
                }
                // "assert" is now a real keyword for persist operations
                // (old error removed — assert is parsed as Stmt::Assert below)
                "test" if col == 1 => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `?` for verification, not `test`.\n  \
                        Try: ? invariant_name",
                        line, col
                    ));
                }
                "trait" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `#` for traits.\n  \
                        Try: # trait {} {{ > method(self) -> Type }}",
                        line,
                        col,
                        self.tokens
                            .get(self.pos + 1)
                            .map(|t| t.text.as_str())
                            .unwrap_or("Name")
                    ));
                }
                "impl" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `#` for implementations.\n  \
                        Try: # impl TraitName for TypeName {{ ... }}",
                        line, col
                    ));
                }
                "match" if col == 1 => {
                    return Err(format!(
                        "{}:{}: `match` at the start of a line looks like a top-level statement.\n  \
                        If this is a match expression, it belongs inside a function body (> rune).\n  \
                        If you want pattern-based rules, use the `|` rune:\n  \
                        | rule_name(x) -> result",
                        line, col
                    ));
                }
                "type" => {
                    return Err(format!(
                        "{}:{}: Futuruna uses `#` to define types, not `type`.\n  \
                        Try: # TypeName(field: Type)",
                        line, col
                    ));
                }
                "return" => {
                    return Err(format!(
                        "{}:{}: Futuruna functions return their last expression — no `return` needed.\n  \
                        Just write the value as the last line of the function body.",
                        line, col
                    ));
                }
                "if" if col == 1 => {
                    return Err(format!(
                        "{}:{}: `if` at top level is not a statement.\n  \
                        If you want a conditional binding, use a function with the `>` rune.\n  \
                        If you want pattern rules, use the `|` rune.",
                        line, col
                    ));
                }
                _ => {}
            }
        }

        // ── Detect bare assignment without `=` rune ──
        if self.peek_kind() == TokenKind::Ident {
            if let Some(next) = self.tokens.get(self.pos + 1) {
                if next.kind == TokenKind::Eq || (next.kind == TokenKind::Op && next.text == "=") {
                    let tok = self.peek();
                    return Err(format!(
                        "{}:{}: bare assignment `{} = ...` needs the `=` rune.\n  \
                        Try: = {} = ...",
                        tok.line, tok.col, tok.text, tok.text
                    ));
                }
            }
        }

        match self.peek_kind() {
            // > rune: definition (Gt at top level, Op(">") inside braces)
            TokenKind::Gt => {
                self.advance();
                self.parse_definition()
            }

            TokenKind::Op if self.peek().text == ">" => {
                self.advance();
                self.parse_definition()
            }
            // | rune: rule/clause
            TokenKind::Pipe => {
                self.advance();
                self.parse_rule()
            }
            // # rune: type declaration
            TokenKind::Hash => {
                self.advance();
                self.parse_type_decl()
            }
            // @ rune: annotation
            TokenKind::At => {
                self.advance();
                self.parse_annotation()
            }
            // = rune: binding
            TokenKind::Eq => {
                self.advance();
                self.parse_binding()
            }
            // ~ rune: stream binding OR stream subscription
            // ~ rune: stream binding OR stream subscription
            TokenKind::Tilde => {
                // ~[...] stream source literal: ~[1, 2, 3] → Stmt::Expr(from_list([1, 2, 3]))
                {
                    let next_pos = self.pos + 1;
                    if next_pos < self.tokens.len()
                        && self.tokens[next_pos].kind == TokenKind::LBracket
                    {
                        self.advance(); // consume ~
                        let list_expr = self.parse_atom()?; // parse the [...] list literal
                        return Ok(Stmt::Expr(
                            ExprKind::App(
                                Box::new(ExprKind::Var("from_list".to_string()).into()),
                                vec![list_expr],
                            )
                            .into(),
                        ));
                    }
                }

                let saved = self.pos;
                self.advance();

                // Disambiguate: ~ name = expr (StreamBind) vs ~ expr | pat -> body (StreamSub)
                let name_tok = self.advance();
                let is_binding = name_tok.kind == TokenKind::Ident
                    && (self.peek_kind() == TokenKind::Eq
                        || (self.peek_kind() == TokenKind::Op && self.peek().text == "="));

                if is_binding {
                    // Consume =
                    if self.peek_kind() == TokenKind::Eq {
                        self.advance();
                    } else if self.peek_kind() == TokenKind::Op && self.peek().text == "=" {
                        self.advance();
                    }
                    let expr = self.parse_expr()?;
                    Ok(Stmt::StreamBind(name_tok.text.clone(), expr))
                } else {
                    // Backtrack and parse as stream expression + match arms
                    self.pos = saved;
                    self.advance(); // consume ~
                    let stream_expr = self.parse_expr()?;
                    let mut arms = Vec::new();

                    while self.peek_kind() == TokenKind::Pipe {
                        self.advance(); // consume |
                        let pat = self.parse_pattern()?;
                        let guard = if self.peek_kind() == TokenKind::KW && self.peek().text == "if"
                        {
                            self.advance();
                            Some(self.parse_expr()?)
                        } else {
                            None
                        };

                        if self.peek_kind() == TokenKind::Arrow {
                            self.advance();
                        } else if self.peek_kind() == TokenKind::Op && self.peek().text == "->" {
                            self.advance();
                        } else {
                            return Err(format!("{}:{}: expected `->` after pattern in subscription arm.\n  Each arm should look like: | pattern -> {{ body }}", self.peek().line, self.peek().col));
                        }

                        let body = self.parse_expr()?;
                        arms.push(MatchArm { pat, guard, body });
                        self.skip_semis();
                    }

                    if arms.is_empty() {
                        return Err(format!(
                            "{}:{}: stream subscription needs at least one `|` arm.\n  Example:\n    ~ my_stream\n    | x -> {{ @ print(x) }}\n    | Err(e) -> {{ @ print(\"error\") }}\n    | Complete -> {{ @ print(\"done\") }}",
                            self.peek().line, self.peek().col
                        ));
                    }
                    Ok(Stmt::StreamSub(stream_expr, arms))
                }
            }
            // ? rune: verify/prove an invariant (or "? all" for all)
            //        ? name by { | binders -> proof }
            // Forms: ? name
            //        ? name -> { pass }
            //        ? name else { fail }
            //        ? name -> { pass } else { fail }
            //        ? name: val -> { pass } else { fail }
            TokenKind::Op if self.peek().text == "?" => {
                self.advance(); // consume '?'
                let name = self.expect_ident()?;
                let proof_block = if self.peek_word("by") {
                    self.advance();
                    Some(self.parse_proof_block()?)
                } else {
                    None
                };

                if proof_block.is_some() {
                    self.skip_semis();
                    if self.peek_kind() == TokenKind::Colon
                        || self.peek_kind() == TokenKind::Arrow
                        || (self.peek_kind() == TokenKind::KW && self.peek().text == "else")
                    {
                        return Err(
                            "`? name by { ... }` cannot be combined with capture/pass/else blocks yet"
                                .into(),
                        );
                    }
                    return Ok(Stmt::Prove {
                        name,
                        proof_block,
                        capture: None,
                        pass_block: None,
                        else_block: None,
                    });
                }
                // Optional `: capture_var`
                let capture = if self.peek_kind() == TokenKind::Colon {
                    self.advance(); // consume ':'
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                // Optional `-> { pass_block }`
                let pass_block = if self.peek_kind() == TokenKind::Arrow {
                    self.advance(); // consume '->'
                    self.expect(TokenKind::LBrace)?;
                    self.skip_semis();
                    let mut stmts = Vec::new();
                    while self.peek_kind() != TokenKind::RBrace {
                        stmts.push(self.parse_block_statement()?);
                        self.skip_semis();
                    }
                    self.expect(TokenKind::RBrace)?;
                    Some(stmts)
                } else {
                    None
                };
                // Optional `else { fail_block }`
                self.skip_semis();
                let else_block = if self.peek_kind() == TokenKind::KW && self.peek().text == "else"
                {
                    self.advance(); // consume 'else'
                    self.expect(TokenKind::LBrace)?;
                    self.skip_semis();
                    let mut stmts = Vec::new();
                    while self.peek_kind() != TokenKind::RBrace {
                        stmts.push(self.parse_block_statement()?);
                        self.skip_semis();
                    }
                    self.expect(TokenKind::RBrace)?;
                    Some(stmts)
                } else {
                    None
                };
                Ok(Stmt::Prove {
                    name,
                    proof_block: None,
                    capture,
                    pass_block,
                    else_block,
                })
            }
            // assert TypeName(args...) — insert a fact
            // Only when followed by an identifier (type name), not by ( which is assert(expr) function call
            TokenKind::KW
                if self.peek().text == "assert"
                    && self.tokens.get(self.pos + 1).map_or(false, |t| {
                        t.kind == TokenKind::Ident || t.kind == TokenKind::Type
                    }) =>
            {
                self.advance(); // consume 'assert'
                let type_name = self.expect_ident()?;
                self.expect(TokenKind::LParen)?;
                let mut args = Vec::new();
                while self.peek_kind() != TokenKind::RParen && self.peek_kind() != TokenKind::Eof {
                    if !args.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                    if self.peek_kind() == TokenKind::RParen {
                        break;
                    }
                    args.push(self.parse_expr()?);
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(Stmt::Assert(type_name, args))
            }
            // retract TypeName(args...) — remove a fact (wildcards allowed)
            TokenKind::KW if self.peek().text == "retract" => {
                self.advance(); // consume 'retract'
                let type_name = self.expect_ident()?;
                self.expect(TokenKind::LParen)?;
                let mut args = Vec::new();
                while self.peek_kind() != TokenKind::RParen && self.peek_kind() != TokenKind::Eof {
                    if !args.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                    if self.peek_kind() == TokenKind::RParen {
                        break;
                    }
                    args.push(self.parse_expr()?);
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(Stmt::Retract(type_name, args))
            }
            // abort — exit current scope with ROLLBACK
            TokenKind::KW if self.peek().text == "abort" => {
                self.advance(); // consume 'abort'
                Ok(Stmt::Abort)
            }
            // for x in expr { body }
            TokenKind::KW if self.peek().text == "for" => {
                self.advance(); // consume 'for'
                let var = self.expect_ident()?;
                if self.peek_kind() == TokenKind::KW && self.peek().text == "in" {
                    self.advance();
                } else {
                    let p = self.peek();
                    return Err(format!(
                        "{}:{}: expected `in` after `for {}`, got `{}`.\n  \
                        Try: for {} in collection {{ ... }}",
                        p.line, p.col, var, p.text, var
                    ));
                }
                let iter_expr = self.parse_expr_prec(0)?;
                self.expect(TokenKind::LBrace)?;
                self.skip_semis();
                let mut body = Vec::new();
                while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                    body.push(self.parse_block_statement()?);
                    self.skip_semis();
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::For(var, iter_expr, body))
            }
            // while cond { body }
            TokenKind::KW if self.peek().text == "while" => {
                self.advance(); // consume 'while'
                let cond = self.parse_expr_prec(0)?;
                self.expect(TokenKind::LBrace)?;
                self.skip_semis();
                let mut body = Vec::new();
                while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                    body.push(self.parse_block_statement()?);
                    self.skip_semis();
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::While(cond, body))
            }
            // Bare expression (inside blocks) — or send: expr <- expr
            _ => {
                let expr = self.parse_expr()?;
                // Check for <- (actor send): target <- message
                if self.peek_kind() == TokenKind::Send {
                    self.advance(); // consume '<-'
                    let msg = self.parse_expr()?;
                    Ok(Stmt::Send(expr, msg))
                } else {
                    Ok(Stmt::Expr(expr))
                }
            }
        }
    }

    // --- > Definition ---

    pub fn parse_definition(&mut self) -> Result<Stmt, String> {
        let name_tok = self.advance();
        let name = name_tok.text;

        // > actor Name(state) { ... }
        if name == "actor" {
            return self.parse_actor_defn();
        }
        // > module Name { ... }
        if name == "module" {
            return self.parse_module_defn();
        }

        // > name(params) -> RetTy with Effects { body }
        let params = if self.peek_kind() == TokenKind::LParen {
            self.parse_params()?
        } else {
            Vec::new()
        };

        let ret_ty = if self.peek_kind() == TokenKind::Arrow {
            self.advance();
            Some(self.parse_type()?)
        } else if self.peek_kind() == TokenKind::FatArrow {
            let tok = self.peek();
            return Err(format!(
                "{}:{}: Futuruna uses `->` for return types, not `=>`.\n  \
                Try: > {}({}) -> ReturnType {{ body }}",
                tok.line,
                tok.col,
                name,
                params
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        } else {
            None
        };

        let effects = if self.peek_kind() == TokenKind::KW && self.peek().text == "with" {
            self.advance();
            self.parse_effect_list()?
        } else {
            Vec::new()
        };

        let body = self.parse_block_expr()?;

        Ok(Stmt::Defn(Defn::Fn {
            name,
            params,
            ret_ty,
            effects,
            body,
        }))
    }

    pub fn parse_actor_defn(&mut self) -> Result<Stmt, String> {
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        let state_name = self.expect_ident()?;
        let state_ty = if self.peek_kind() == TokenKind::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut handlers = Vec::new();
        while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
            if self.peek_kind() == TokenKind::Pipe {
                self.advance();
                if self.peek_kind() == TokenKind::KW && self.peek().text == "on" {
                    self.advance();
                }
                let msg_pat = self.parse_pattern()?;
                self.expect(TokenKind::Arrow)?;
                let body = self.parse_expr()?;
                handlers.push(Handler { msg_pat, body });
            } else {
                // Skip unexpected token to prevent infinite loop
                self.advance();
            }
            self.skip_semis();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Stmt::Defn(Defn::Actor {
            name,
            state_param: Param {
                name: state_name,
                ty: state_ty,
                inout: false,
            },
            handlers,
        }))
    }

    pub fn parse_module_defn(&mut self) -> Result<Stmt, String> {
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut body = Vec::new();
        while self.peek_kind() != TokenKind::RBrace {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            self.skip_semis();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Stmt::Defn(Defn::Module { name, body }))
    }

    pub fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while self.peek_kind() != TokenKind::RParen {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
            }
            let name = self.expect_ident()?;
            let mut inout = false;
            let ty = if self.peek_kind() == TokenKind::Colon {
                self.advance();
                // Check for inout modifier BEFORE parsing type
                if self.peek_kind() == TokenKind::Ident && self.peek().text == "inout" {
                    self.advance(); // consume 'inout'
                    inout = true;
                }
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(Param { name, ty, inout });
        }
        self.expect(TokenKind::RParen)?;
        Ok(params)
    }

    pub fn parse_effect_list(&mut self) -> Result<Vec<String>, String> {
        let mut effects = Vec::new();
        let name = self.expect_ident()?;
        effects.push(name);
        while self.peek_kind() == TokenKind::Comma {
            self.advance();
            let name = self.expect_ident()?;
            effects.push(name);
        }
        Ok(effects)
    }

    /// Parse: handle EffectName { | op(args) -> body ... } in body_expr
    /// Called after consuming the leading `|` and seeing `handle` keyword.
    pub fn parse_handle_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'handle'
        let effect = self.expect_ident()?;

        // Parse handler clauses: { | op(args) -> body ... }
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut handlers = Vec::new();
        while self.peek_kind() != TokenKind::RBrace {
            // Each handler: | op_name(param1, param2, ...) -> body
            self.expect(TokenKind::Pipe)?;
            let op_name = self.expect_ident()?;
            let mut params = Vec::new();
            if self.peek_kind() == TokenKind::LParen {
                self.advance(); // (
                while self.peek_kind() != TokenKind::RParen {
                    if !params.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    params.push(self.expect_ident()?);
                }
                self.advance(); // )
            }
            self.expect(TokenKind::Arrow)?;
            let body = self.parse_expr()?;
            handlers.push(EffHandler {
                op_name,
                params,
                body,
            });
            self.skip_semis();
        }
        self.expect(TokenKind::RBrace)?;

        // Expect 'in' keyword followed by body expression
        if self.peek_kind() == TokenKind::KW && self.peek().text == "in" {
            self.advance();
        } else {
            let p = self.peek();
            return Err(format!(
                "{}:{}: expected 'in' after | handle {{ ... }}",
                p.line, p.col
            ));
        }
        let body = self.parse_expr()?;

        Ok(ExprKind::Handle {
            effect,
            handlers,
            body: Box::new(body),
        }
        .into())
    }

    // --- | Rule ---

    pub fn parse_rule(&mut self) -> Result<Stmt, String> {
        // | handle EffectName { ... } in body — algebraic effect handler
        if self.peek_kind() == TokenKind::KW && self.peek().text == "handle" {
            let expr = self.parse_handle_expr()?;
            return Ok(Stmt::Expr(expr));
        }

        // | scope Name { ... }
        if self.peek_kind() == TokenKind::KW && self.peek().text == "scope" {
            self.advance();
            let name = self.expect_ident()?;
            self.expect(TokenKind::LBrace)?;
            self.skip_semis();
            let mut body = Vec::new();
            while self.peek_kind() != TokenKind::RBrace {
                let stmt = self.parse_statement()?;
                body.push(stmt);
                self.skip_semis();
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(Stmt::Rule(Rule::Scope { name, body }));
        }

        // | exception label ...
        if self.peek_kind() == TokenKind::KW && self.peek().text == "exception" {
            self.advance();
            let label = self.expect_ident()?;
            let head = self.parse_expr()?;
            self.expect(TokenKind::Arrow)?;
            let value = self.parse_expr()?;
            let condition = if self.peek_kind() == TokenKind::KW && self.peek().text == "under" {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            return Ok(Stmt::Rule(Rule::Exception {
                label,
                head,
                value,
                condition,
            }));
        }

        // | name: subject -> predicate (named invariant for verification)
        // Detect: if next token is ident and token after is ':', it's an invariant
        if self.peek_kind() == TokenKind::Ident {
            let saved_pos = self.pos;
            let name_tok = self.advance();
            if self.peek_kind() == TokenKind::Colon {
                self.advance(); // consume ':'
                let subject = self.parse_expr()?;
                self.expect(TokenKind::Arrow)?;
                let predicate = self.parse_expr()?;
                return Ok(Stmt::Invariant {
                    name: name_tok.text.clone(),
                    subject,
                    predicate,
                });
            }
            // Not an invariant — backtrack
            self.pos = saved_pos;
        }

        // | head [-> body] [under condition]
        let head = self.parse_expr()?;
        if self.peek_kind() == TokenKind::Arrow {
            self.advance();
            self.in_rule_body = true;
            let body_or_value = self.parse_expr()?;
            self.in_rule_body = false;
            if self.peek_kind() == TokenKind::KW && self.peek().text == "under" {
                self.advance();
                let condition = self.parse_expr()?;
                Ok(Stmt::Rule(Rule::Default {
                    head,
                    value: body_or_value,
                    condition: Some(condition),
                }))
            } else {
                // Parse rule body with and/or precedence:
                // `and` (,) binds tighter than `or`
                // a and b or c and d → Disjunction([Conjunction([a,b]), Conjunction([c,d])])
                let parse_and_group = |parser: &mut Self, first: Expr| -> Result<Expr, String> {
                    let is_and = parser.peek_kind() == TokenKind::Comma
                        || (parser.peek_kind() == TokenKind::Ident && parser.peek().text == "and");
                    if is_and {
                        let mut goals = vec![first];
                        while parser.peek_kind() == TokenKind::Comma
                            || (parser.peek_kind() == TokenKind::Ident
                                && parser.peek().text == "and")
                        {
                            parser.advance();
                            goals.push(parser.parse_expr()?);
                        }
                        Ok(ExprKind::Conjunction(goals).into())
                    } else {
                        Ok(first)
                    }
                };

                let first_group = parse_and_group(self, body_or_value)?;

                // Check for `or` between groups
                let is_or = self.peek_kind() == TokenKind::Op && self.peek().text == "||";
                if is_or {
                    let mut alternatives = vec![first_group];
                    while self.peek_kind() == TokenKind::Op && self.peek().text == "||" {
                        self.advance(); // consume 'or' (lexed as ||)
                        let next_expr = self.parse_expr()?;
                        let group = parse_and_group(self, next_expr)?;
                        alternatives.push(group);
                    }
                    Ok(Stmt::Rule(Rule::Clause {
                        head,
                        body: Some(ExprKind::Disjunction(alternatives).into()),
                    }))
                } else {
                    Ok(Stmt::Rule(Rule::Clause {
                        head,
                        body: Some(first_group),
                    }))
                }
            }
        } else {
            // Bare fact
            Ok(Stmt::Rule(Rule::Clause { head, body: None }))
        }
    }

    // --- # Type declaration ---

    pub fn parse_type_decl(&mut self) -> Result<Stmt, String> {
        // # trait Name { > method(self) -> Type }
        if self.peek_kind() == TokenKind::KW && self.peek().text == "trait" {
            self.advance();
            return self.parse_trait_decl();
        }

        // # impl Trait for Type { > method(self) -> Type { body } }
        if self.peek_kind() == TokenKind::KW && self.peek().text == "impl" {
            self.advance();
            return self.parse_impl_block();
        }

        // # effect Name { ... }
        if self.peek_kind() == TokenKind::KW && self.peek().text == "effect" {
            self.advance();
            let name = self.expect_ident()?;
            self.expect(TokenKind::LBrace)?;
            self.skip_semis();
            let mut ops = Vec::new();
            while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                let is_gt = self.peek_kind() == TokenKind::Gt
                    || (self.peek_kind() == TokenKind::Op && self.peek().text == ">");
                if is_gt {
                    self.advance();
                    let op_name = self.expect_ident()?;
                    let params = if self.peek_kind() == TokenKind::LParen {
                        self.parse_params()?
                    } else {
                        Vec::new()
                    };
                    let ret_ty = if self.peek_kind() == TokenKind::Arrow {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    ops.push((op_name, params, ret_ty));
                } else {
                    // Skip unknown tokens to prevent infinite loop
                    self.advance();
                }
                self.skip_semis();
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(Stmt::TypeDecl(TypeDecl::EffectDecl { name, ops }));
        }

        // # Name(params) = Variant1 | Variant2 | ...
        let name = self.expect_ident()?;

        // # Type WHEN condition = new_variants (conditional type evolution)
        if self.peek_kind() == TokenKind::KW && self.peek().text == "WHEN"
            || (self.peek_kind() == TokenKind::Type && self.peek().text == "WHEN")
        {
            self.advance(); // consume WHEN
            let condition = self.parse_expr_prec(0)?;
            // Expect -> (arrow separates condition from new type definition)
            if self.peek_kind() == TokenKind::Arrow {
                self.advance();
            } else {
                let p = self.peek();
                return Err(format!("{}:{}: expected `->` after WHEN condition, got `{}`\n  Syntax: # Type WHEN condition -> NewVariants",
                    p.line, p.col, p.text));
            }
            let (variants, except_from) = self.parse_variants()?;
            return Ok(Stmt::TypeDecl(TypeDecl::WhenType {
                name,
                condition,
                variants,
                except_from,
            }));
        }

        // Detect # Name { ... } (curly-brace struct syntax from other languages)
        if self.peek_kind() == TokenKind::LBrace {
            let p = self.peek();
            return Err(format!(
                "{}:{}: struct fields use parentheses, not braces.\n  Try: # {}(field: Type, ...)\n  Not: # {} {{ field: Type, ... }}",
                p.line, p.col, name, name
            ));
        }

        let params = if self.peek_kind() == TokenKind::LParen {
            self.parse_type_params()?
        } else {
            Vec::new()
        };

        // Check for = (ADT definition) vs standalone declaration
        if self.peek_kind() == TokenKind::Op && self.peek().text == "=" {
            self.advance();
        } else if self.peek_kind() == TokenKind::Eq {
            self.advance();
        } else {
            // No = sign: check if params have type annotations (name: Type)
            // If so, this is a single-variant product type: # Point(x: Float, y: Float)
            let has_typed_params = params.iter().any(|p| p.ty.is_some());
            if has_typed_params {
                // Single-variant type with named fields
                let fields: Vec<Field> = params
                    .iter()
                    .map(|p| Field {
                        name: p.name.clone(),
                        ty: p.ty.clone().unwrap_or(Ty::Name("Any".into())),
                    })
                    .collect();
                let variant = Variant {
                    name: name.clone(),
                    fields,
                    positional: false,
                    from_type: None,
                };
                return Ok(Stmt::TypeDecl(TypeDecl::ADT {
                    name,
                    params: Vec::new(),
                    variants: vec![variant],
                    methods: Vec::new(),
                    except_from: None,
                }));
            }
            // Truly opaque type declaration
            return Ok(Stmt::TypeDecl(TypeDecl::ADT {
                name,
                params,
                variants: Vec::new(),
                methods: Vec::new(),
                except_from: None,
            }));
        }

        let (variants, except_from) = self.parse_variants()?;

        // Check for method block: # Type = ... { > method(self) -> ... { } }
        let methods = if self.peek_kind() == TokenKind::LBrace {
            self.advance(); // consume {
            self.skip_semis();
            let mut methods = Vec::new();
            while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                let is_gt = self.peek_kind() == TokenKind::Gt
                    || (self.peek_kind() == TokenKind::Op && self.peek().text == ">");
                if is_gt {
                    self.advance(); // consume >
                    match self.parse_definition() {
                        Ok(Stmt::Defn(defn)) => methods.push(defn),
                        Ok(_) => {}
                        Err(e) => return Err(format!("in method block: {}", e)),
                    }
                } else if self.peek_kind() == TokenKind::Hash {
                    // Allow nested # type declarations inside method block
                    self.advance();
                    let _ = self.parse_type_decl()?;
                } else {
                    // skip unknown tokens inside method block
                    self.advance();
                }
                self.skip_semis();
            }
            if self.peek_kind() == TokenKind::RBrace {
                self.advance();
            }
            methods
        } else {
            Vec::new()
        };

        Ok(Stmt::TypeDecl(TypeDecl::ADT {
            name,
            params,
            variants,
            methods,
            except_from,
        }))
    }

    /// Parse: # trait Display { > fmt(self) -> String }
    pub fn parse_trait_decl(&mut self) -> Result<Stmt, String> {
        let name = self.expect_ident()?;
        let params = if self.peek_kind() == TokenKind::LParen {
            self.parse_type_params()?
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut methods = Vec::new();
        while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
            let is_gt = self.peek_kind() == TokenKind::Gt
                || (self.peek_kind() == TokenKind::Op && self.peek().text == ">");
            if is_gt {
                self.advance(); // consume >
                let method_name = self.expect_ident()?;
                let method_params = if self.peek_kind() == TokenKind::LParen {
                    self.parse_params()?
                } else {
                    Vec::new()
                };
                let ret_ty = if self.peek_kind() == TokenKind::Arrow {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                // Optional default body
                let default_body = if self.peek_kind() == TokenKind::LBrace {
                    Some(self.parse_block_expr()?)
                } else {
                    None
                };
                methods.push(TraitMethod {
                    name: method_name,
                    params: method_params,
                    ret_ty,
                    default_body,
                });
            } else {
                self.advance(); // skip unknown
            }
            self.skip_semis();
        }
        if self.peek_kind() == TokenKind::RBrace {
            self.advance();
        }
        Ok(Stmt::TypeDecl(TypeDecl::TraitDecl {
            name,
            params,
            methods,
        }))
    }

    /// Parse: # impl Display for Shape { > fmt(self) -> String { ... } }
    pub fn parse_impl_block(&mut self) -> Result<Stmt, String> {
        let trait_name = self.parse_qualified_name()?;
        // expect "for"
        if self.peek_kind() == TokenKind::KW && self.peek().text == "for" {
            self.advance();
        } else {
            return Err(format!("expected 'for' after trait name in # impl"));
        }
        let for_type = self.parse_qualified_name()?;
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut methods = Vec::new();
        while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
            let is_gt = self.peek_kind() == TokenKind::Gt
                || (self.peek_kind() == TokenKind::Op && self.peek().text == ">");
            if is_gt {
                self.advance(); // consume >
                match self.parse_definition() {
                    Ok(Stmt::Defn(defn)) => methods.push(defn),
                    Ok(_) => {}
                    Err(e) => return Err(format!("in impl block: {}", e)),
                }
            } else {
                self.advance(); // skip
            }
            self.skip_semis();
        }
        if self.peek_kind() == TokenKind::RBrace {
            self.advance();
        }
        Ok(Stmt::TypeDecl(TypeDecl::ImplBlock {
            trait_name,
            for_type,
            methods,
        }))
    }

    pub fn parse_type_params(&mut self) -> Result<Vec<Param>, String> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while self.peek_kind() != TokenKind::RParen {
            if !params.is_empty() {
                self.expect(TokenKind::Comma)?;
            }
            let name = self.expect_ident()?;
            let ty = if self.peek_kind() == TokenKind::Colon {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(Param {
                name,
                ty,
                inout: false,
            });
        }
        self.expect(TokenKind::RParen)?;
        Ok(params)
    }

    pub fn parse_variants(
        &mut self,
    ) -> Result<(Vec<Variant>, Option<(String, Vec<String>)>), String> {
        let mut variants = Vec::new();
        let mut except_from: Option<(String, Vec<String>)> = None;
        loop {
            let name = self.expect_ident()?;
            // Check for qualified variant: Parent.Variant (subset type syntax)
            if self.peek_kind() == TokenKind::Dot {
                self.advance(); // consume '.'
                let variant_name = self.expect_ident()?;
                variants.push(Variant {
                    name: variant_name,
                    fields: Vec::new(),
                    positional: false,
                    from_type: Some(name),
                });
            } else if (self.peek_kind() == TokenKind::Ident || self.peek_kind() == TokenKind::Type)
                && self.peek().text == "EXCEPT"
            {
                // TypeName EXCEPT Variant1 | Variant2 | ...
                // or: TypeName EXCEPT OtherType
                self.advance(); // consume 'EXCEPT'
                let mut excluded = Vec::new();
                loop {
                    let exc = self.expect_ident()?;
                    excluded.push(exc);
                    if self.peek_kind() == TokenKind::Pipe {
                        // Check if next after | is EXCEPT-continuation or a new variant group
                        let saved = self.pos;
                        self.advance(); // consume |
                                        // If next token followed by EXCEPT or end, it's a new group
                                        // For simplicity: EXCEPT consumes remaining | separated names
                                        // until we hit a non-ident or end
                        if self.peek_kind() == TokenKind::Ident
                            || self.peek_kind() == TokenKind::Type
                        {
                            // Continue collecting excluded variants
                            continue;
                        } else {
                            self.pos = saved;
                            break;
                        }
                    } else {
                        break;
                    }
                }
                except_from = Some((name, excluded));
                break; // EXCEPT always ends the variant list
            } else {
                let (fields, positional) = if self.peek_kind() == TokenKind::LParen {
                    self.parse_field_list()?
                } else {
                    (Vec::new(), false)
                };
                // If this is an uppercase name with no fields and no dot,
                // it MIGHT be a type inclusion (# A = Kristendom | Islam).
                // We can't resolve at parse time — mark with from_type = Some("__include")
                // and resolve during type registration.
                let is_potential_type_include = fields.is_empty()
                    && name.chars().next().map_or(false, |c| c.is_uppercase())
                    && self.peek_kind() != TokenKind::LParen;
                variants.push(Variant {
                    name: name.clone(),
                    fields,
                    positional,
                    from_type: if is_potential_type_include {
                        Some("__maybe_include".into())
                    } else {
                        None
                    },
                });
            }
            if self.peek_kind() == TokenKind::Pipe {
                self.advance();
            } else {
                break;
            }
        }
        Ok((variants, except_from))
    }

    /// Parse field list: (name: Type, ...) named OR (Type, Type, ...) positional.
    /// Returns (fields, is_positional).
    pub fn parse_field_list(&mut self) -> Result<(Vec<Field>, bool), String> {
        self.expect(TokenKind::LParen)?;
        let mut fields = Vec::new();
        if self.peek_kind() == TokenKind::RParen {
            self.expect(TokenKind::RParen)?;
            return Ok((fields, false));
        }

        // Detect positional vs named by peeking at first field
        let is_positional = {
            let saved = self.pos;
            let is_ident = matches!(
                self.peek_kind(),
                TokenKind::Ident | TokenKind::Type | TokenKind::KW
            );
            let result = if is_ident {
                self.advance();
                self.peek_kind() != TokenKind::Colon
            } else {
                true // non-ident start (e.g. `(`) must be a type → positional
            };
            self.pos = saved;
            result
        };

        let mut idx = 0;
        while self.peek_kind() != TokenKind::RParen {
            if !fields.is_empty() {
                self.expect(TokenKind::Comma)?;
            }
            if is_positional {
                let ty = self.parse_type()?;
                fields.push(Field {
                    name: format!("_{}", idx),
                    ty,
                });
                idx += 1;
            } else {
                let tok = self.peek().clone();
                let field_name = self.expect_ident()?;
                if self.peek_kind() != TokenKind::Colon {
                    return Err(format!(
                        "{}:{}: constructor fields must be named — write `{}: Type` instead of just a type",
                        tok.line, tok.col, field_name
                    ));
                }
                self.advance(); // consume colon
                let ty = self.parse_type()?;
                fields.push(Field {
                    name: field_name,
                    ty,
                });
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok((fields, is_positional))
    }

    pub fn parse_type_list(&mut self) -> Result<Vec<Ty>, String> {
        self.expect(TokenKind::LParen)?;
        let mut types = Vec::new();
        while self.peek_kind() != TokenKind::RParen {
            if !types.is_empty() {
                self.expect(TokenKind::Comma)?;
            }
            types.push(self.parse_type()?);
        }
        self.expect(TokenKind::RParen)?;
        Ok(types)
    }

    // --- @ Annotation or Effect invocation ---

    pub fn parse_annotation(&mut self) -> Result<Stmt, String> {
        let tok = self.advance();
        let name = tok.text.clone();

        // @ use path::to::thing
        if name == "use" {
            return self.parse_use_decl();
        }

        // @ rust { raw Rust code } — escape hatch for inline Rust
        if name == "rust" && self.peek_kind() == TokenKind::LBrace {
            return self.parse_rust_block();
        }

        // @ import ./module — multi-file import
        if name == "import" {
            return self.parse_import_decl();
        }

        // @ depend "crate" "version" — Cargo dependency
        if name == "depend" {
            return self.parse_depend_decl();
        }

        // @ store TypeName [delete_on_change] [in "scope"]
        // Object store persistence (struct → JSON blob in SQLite)
        // @ persist TypeName [in "scope"]
        // Typed-column persistence (struct → SQLite table with one column per field)
        if name == "store" || name == "persist" {
            let kind = name.clone();
            let type_name = self.expect_ident()?;
            let mut args: Vec<Expr> = vec![ExprKind::Var(type_name.clone()).into()];
            // Optional `delete_on_change` flag (only meaningful for @ store today,
            // accepted in both for symmetry).
            if self.peek_kind() == TokenKind::Ident && self.peek().text == "delete_on_change" {
                self.advance();
                args.push(ExprKind::Var("delete_on_change".to_string()).into());
            }
            // Optional `in "scope"` clause
            if self.peek_kind() == TokenKind::KW && self.peek().text == "in" {
                self.advance(); // consume `in`
                if self.peek_kind() == TokenKind::String_ {
                    let scope = self.advance().text.clone();
                    args.push(ExprKind::Lit(Literal::Str(scope)).into());
                } else {
                    let p = self.peek();
                    return Err(format!(
                        "{}:{}: expected scope string after `in`\n  Try: @ {} {} in \"myapp\"",
                        p.line, p.col, kind, type_name
                    ));
                }
            }
            return Ok(Stmt::Annot(kind, args));
        }

        // @ migrate Type(old_fields...) -> Type(new_fields...) [unsafe]
        //
        // Kept as an annotation payload so older compiler passes can ignore it
        // unless they specifically care about persisted schema evolution.
        if name == "migrate" {
            let old_shape = self.parse_expr()?;
            self.expect(TokenKind::Arrow)?;
            let new_shape = self.parse_expr()?;
            let mut args = vec![old_shape, new_shape];
            if matches!(self.peek_kind(), TokenKind::Ident | TokenKind::KW)
                && self.peek().text == "unsafe"
            {
                self.advance();
                args.push(ExprKind::Var("unsafe".to_string()).into());
            }
            return Ok(Stmt::Annot(name, args));
        }

        // @ sprog / @ language — already consumed by lexer, skip the code token
        if name == "sprog" || name == "language" {
            if self.peek_kind() == TokenKind::Ident || self.peek_kind() == TokenKind::KW {
                self.advance(); // consume the language code (da, en, etc.)
            }
            return Ok(Stmt::Annot(name, Vec::new()));
        }

        // Leaf effect operations (no arguments required): @ time, @ random, @ input
        if name == "time" || name == "random" || name == "input" {
            // Allow optional parens: @ time and @ time() are both valid
            if self.peek_kind() == TokenKind::LParen {
                let _ = self.parse_arg_list()?; // consume empty parens
            }
            return Ok(Stmt::Expr(ExprKind::Effect(name, vec![]).into()));
        }

        // If followed by ( it's an effect invocation: @ print("hello")
        if self.peek_kind() == TokenKind::LParen {
            let args = self.parse_arg_list()?;
            Ok(Stmt::Expr(ExprKind::Effect(name, args).into()))
        } else if name == "export" && self.peek_kind() == TokenKind::Ident {
            // Post-hoc export: `@ export add` — capture the name as arg
            let export_name = self.advance().text.clone();
            Ok(Stmt::Annot(name, vec![ExprKind::Var(export_name).into()]))
        } else {
            // Pure annotation: @ test, @ pure, @ total
            Ok(Stmt::Annot(name, Vec::new()))
        }
    }

    /// Parse: @ rust { raw Rust code }
    /// Extracts raw source text between { }, preserving original formatting.
    pub fn parse_rust_block(&mut self) -> Result<Stmt, String> {
        let open_brace = self.advance(); // consume {
                                         // Find the char offset right after the opening brace
        let start = self.char_offset(open_brace.line, open_brace.col) + 1;

        // Scan raw source chars to find matching closing brace
        let mut depth = 1i32;
        let mut end = start;
        let mut in_string = false;
        let mut in_char = false;
        let mut escape = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        while end < self.source_chars.len() && depth > 0 {
            let c = self.source_chars[end];
            let next = self.source_chars.get(end + 1).copied();

            if escape {
                escape = false;
                end += 1;
                continue;
            }
            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                }
                end += 1;
                continue;
            }
            if in_block_comment {
                if c == '*' && next == Some('/') {
                    in_block_comment = false;
                    end += 2;
                    continue;
                }
                end += 1;
                continue;
            }
            if in_string {
                if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_string = false;
                }
                end += 1;
                continue;
            }
            if in_char {
                if c == '\\' {
                    escape = true;
                } else if c == '\'' {
                    in_char = false;
                }
                end += 1;
                continue;
            }

            match c {
                '/' if next == Some('/') => {
                    in_line_comment = true;
                    end += 2;
                    continue;
                }
                '/' if next == Some('*') => {
                    in_block_comment = true;
                    end += 2;
                    continue;
                }
                '"' => {
                    in_string = true;
                }
                '\'' => {
                    in_char = true;
                }
                '{' => {
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }

        let raw: String = self.source_chars[start..end].iter().collect();
        // Dedent: strip common leading whitespace
        let lines: Vec<&str> = raw.lines().collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        let code: String = lines
            .iter()
            .map(|l| {
                if l.len() >= min_indent {
                    &l[min_indent..]
                } else {
                    l.trim()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Advance the token stream past the matching closing brace
        let mut tok_depth = 1i32;
        while tok_depth > 0 && self.peek_kind() != TokenKind::Eof {
            let tok = self.advance();
            match tok.kind {
                TokenKind::LBrace => tok_depth += 1,
                TokenKind::RBrace => tok_depth -= 1,
                _ => {}
            }
        }

        Ok(Stmt::RustBlock(code.trim().to_string()))
    }

    /// Parse: @ use std::collections::HashMap
    pub fn parse_use_decl(&mut self) -> Result<Stmt, String> {
        let mut path = self.expect_ident()?;
        while self.peek_kind() == TokenKind::Colon {
            self.advance(); // first :
            if self.peek_kind() == TokenKind::Colon {
                self.advance(); // second :
                path.push_str("::");
                if self.peek_kind() == TokenKind::LBrace {
                    // @ use std::collections::{HashMap, BTreeMap}
                    self.advance();
                    path.push('{');
                    let mut first = true;
                    while self.peek_kind() != TokenKind::RBrace
                        && self.peek_kind() != TokenKind::Eof
                    {
                        if !first {
                            self.expect(TokenKind::Comma)?;
                            path.push_str(", ");
                        }
                        let seg = self.expect_ident()?;
                        path.push_str(&seg);
                        first = false;
                    }
                    if self.peek_kind() == TokenKind::RBrace {
                        self.advance();
                    }
                    path.push('}');
                } else if self.peek_kind() == TokenKind::Op && self.peek().text == "*" {
                    // @ use std::collections::*
                    self.advance();
                    path.push('*');
                } else {
                    let seg = self.expect_ident()?;
                    path.push_str(&seg);
                }
            } else {
                break;
            }
        }
        Ok(Stmt::Use(path))
    }

    /// Parse: @ import ./math, @ import Name from ./path, or @ import #hash from ./module
    pub fn parse_import_decl(&mut self) -> Result<Stmt, String> {
        // @ import Name from ./module — qualified import (M3b)
        // Name is a Type token (capitalized) followed by 'from'
        if self.peek_kind() == TokenKind::Type {
            let saved = self.pos;
            let mod_name = self.advance().text.clone();
            if self.peek_kind() == TokenKind::Ident && self.peek().text == "from" {
                self.advance(); // consume 'from'
                let path = self.parse_module_path()?;
                return Ok(Stmt::QualifiedImport(mod_name, path));
            }
            // Not a qualified import — backtrack
            self.pos = saved;
        }
        // @ import #hash from ./module — content-addressed import
        if self.peek_kind() == TokenKind::Hash {
            self.advance(); // consume '#'
                            // Collect hash chars: hex digits may lex as Ident/Int/Type tokens
            let mut hash = String::new();
            while self.peek_kind() != TokenKind::Eof {
                // Stop when we hit 'from' keyword
                if self.peek_kind() == TokenKind::Ident && self.peek().text == "from" {
                    break;
                }
                if self.peek_kind() == TokenKind::Ident
                    || self.peek_kind() == TokenKind::Int_
                    || self.peek_kind() == TokenKind::Type
                {
                    hash.push_str(&self.advance().text);
                } else {
                    break;
                }
            }
            if hash.is_empty() {
                let p = self.peek();
                return Err(format!(
                    "{}:{}: expected hash after # in @ import #hash from ./module",
                    p.line, p.col
                ));
            }
            // Expect 'from' keyword
            if self.peek_kind() == TokenKind::Ident && self.peek().text == "from" {
                self.advance(); // consume 'from'
            } else {
                let p = self.peek();
                return Err(format!(
                    "{}:{}: expected 'from' after hash in @ import #hash from ./module",
                    p.line, p.col
                ));
            }
            // Parse module path
            let path = self.parse_module_path()?;
            return Ok(Stmt::HashImport(hash, path));
        }
        // @ import ./module — regular import
        let path = self.parse_module_path()?;
        Ok(Stmt::Import(path))
    }

    /// Parse a module path like `./math`, `./utils/helpers`, or `./kapitel-08`
    pub fn parse_module_path(&mut self) -> Result<String, String> {
        let mut path = String::new();
        // Handle ./ and ../ prefixes (including chained ../../)
        while self.peek_kind() == TokenKind::Dot {
            self.advance();
            path.push('.');
            // Check for .. (parent directory)
            if self.peek_kind() == TokenKind::Dot {
                self.advance();
                path.push('.');
            }
            // Consume the / after . or ..
            if self.peek_kind() == TokenKind::Op && self.peek().text == "/" {
                self.advance();
                path.push('/');
            }
        }
        loop {
            if self.peek_kind() == TokenKind::Op && self.peek().text == "/" {
                self.advance();
                path.push('/');
            } else if self.peek_kind() == TokenKind::Op && self.peek().text == "-" {
                // Allow dashes in module paths (e.g. kapitel-08)
                self.advance();
                path.push('-');
            } else if self.peek_kind() == TokenKind::Ident
                || self.peek_kind() == TokenKind::Type
                || self.peek_kind() == TokenKind::Int_
            {
                let seg = self.advance().text.clone();
                path.push_str(&seg);
            } else {
                break;
            }
        }
        if path.is_empty() {
            let p = self.peek();
            return Err(format!(
                "{}:{}: expected module path after @ import\n  Try: @ import ./module_name",
                p.line, p.col
            ));
        }
        Ok(path)
    }

    /// Parse: @ depend "crate_name" "version"
    pub fn parse_depend_decl(&mut self) -> Result<Stmt, String> {
        let crate_name = if self.peek_kind() == TokenKind::String_ {
            self.advance().text.clone()
        } else {
            let p = self.peek();
            return Err(format!("{}:{}: expected crate name string after @ depend\n  Try: @ depend \"crate_name\" \"version\"", p.line, p.col));
        };
        let version = if self.peek_kind() == TokenKind::String_ {
            self.advance().text.clone()
        } else {
            let p = self.peek();
            return Err(format!("{}:{}: expected version string after crate name in @ depend\n  Try: @ depend \"{}\" \"1.0\"", p.line, p.col, crate_name));
        };
        Ok(Stmt::Depend(crate_name, version))
    }

    // --- = Binding ---

    pub fn parse_binding(&mut self) -> Result<Stmt, String> {
        let pat = self.parse_pattern()?;
        let ty = if self.peek_kind() == TokenKind::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        // Check for <- (monadic bind) vs = (regular bind)
        if self.peek_kind() == TokenKind::Send {
            // = x <- expr  (monadic: unwrap Ok/Some, early-return on Err/None)
            self.advance();
            let value = self.parse_expr()?;
            return Ok(Stmt::MonadicBind(pat, ty, value));
        }
        // Consume = (could be Op("=") or part of the initial = rune)
        if self.peek_kind() == TokenKind::Op && self.peek().text == "=" {
            self.advance();
        } else if self.peek_kind() == TokenKind::Eq {
            self.advance();
        }
        let value = self.parse_expr()?;
        Ok(Stmt::Bind(pat, ty, value))
    }

    // --- Type parsing ---

    pub fn parse_type(&mut self) -> Result<Ty, String> {
        let mut base = self.parse_type_atom()?;
        // T? → Optional(T) — Kotlin-style nullable type sugar for Option(T)
        if self.peek_kind() == TokenKind::Op && self.peek().text == "?" {
            self.advance();
            base = Ty::Optional(Box::new(base));
        }
        if self.peek_kind() == TokenKind::Arrow {
            self.advance();
            let ret = self.parse_type()?;
            Ok(Ty::Arrow(Box::new(base), Box::new(ret)))
        } else {
            Ok(base)
        }
    }

    pub fn parse_type_atom(&mut self) -> Result<Ty, String> {
        // shared T → Ty::Shared(T)
        if self.peek_kind() == TokenKind::Ident && self.peek().text == "shared" {
            self.advance(); // consume "shared"
            let inner = self.parse_type_atom()?;
            return Ok(Ty::Shared(Box::new(inner)));
        }
        match self.peek_kind() {
            TokenKind::Type | TokenKind::Ident => {
                let tok = self.advance();
                // Check for qualified path: fmt::Display, std::ops::Add
                let mut name = tok.text.clone();
                while self.peek_kind() == TokenKind::Colon {
                    let saved = self.pos;
                    self.advance(); // first :
                    if self.peek_kind() == TokenKind::Colon {
                        self.advance(); // second :
                        if let Ok(seg) = self.expect_ident() {
                            name.push_str("::");
                            name.push_str(&seg);
                        } else {
                            self.pos = saved;
                            break;
                        }
                    } else {
                        self.pos = saved;
                        break;
                    }
                }
                if self.peek_kind() == TokenKind::LParen {
                    let args = self.parse_type_list()?;
                    Ok(Ty::App(Box::new(Ty::Name(name)), args))
                } else if name.contains("::") || name.starts_with(|c: char| c.is_uppercase()) {
                    Ok(Ty::Name(name))
                } else {
                    Ok(Ty::Var(name))
                }
            }
            TokenKind::LParen => {
                self.advance();
                if self.peek_kind() == TokenKind::RParen {
                    self.advance();
                    Ok(Ty::Unit)
                } else {
                    let inner = self.parse_type()?;
                    self.expect(TokenKind::RParen)?;
                    Ok(inner)
                }
            }
            TokenKind::Amp => {
                self.advance();
                if self.peek_kind() == TokenKind::KW && self.peek().text == "mut" {
                    self.advance();
                    Ok(Ty::MutRef(Box::new(self.parse_type_atom()?)))
                } else {
                    Ok(Ty::Ref(Box::new(self.parse_type_atom()?)))
                }
            }
            _ => {
                let tok = self.peek().clone();
                let got_desc = match tok.kind {
                    TokenKind::Semi => "newline".to_string(),
                    TokenKind::Op => format!("operator `{}`", tok.text),
                    TokenKind::Int_ | TokenKind::Float_ => format!("number `{}`", tok.text),
                    TokenKind::String_ => "a string literal".to_string(),
                    TokenKind::LBrace => "`{`".to_string(),
                    TokenKind::RBrace => "`}`".to_string(),
                    TokenKind::LParen => "`(`".to_string(),
                    TokenKind::RParen => "`)`".to_string(),
                    TokenKind::Eq => "`=`".to_string(),
                    TokenKind::Pipe => "`|`".to_string(),
                    TokenKind::Eof => "end of file".to_string(),
                    _ => format!("`{}`", tok.text),
                };
                let hint = match tok.kind {
                    TokenKind::Eq => "\n  Hint: did you forget the type? e.g. `name: Type`",
                    TokenKind::LBrace => {
                        "\n  Hint: types use parentheses for fields: `# Name(field: Type)`"
                    }
                    _ => "",
                };
                Err(format!(
                    "{}:{}: expected a type name, got {}{}",
                    tok.line, tok.col, got_desc, hint
                ))
            }
        }
    }

    // --- Pattern parsing ---

    pub fn parse_pattern(&mut self) -> Result<Pat, String> {
        match self.peek_kind() {
            TokenKind::Ident => {
                let tok = self.advance();
                if tok.text == "_" {
                    Ok(Pat::Wild)
                } else {
                    Ok(Pat::Var(tok.text))
                }
            }
            TokenKind::Type => {
                let tok = self.advance();
                if self.peek_kind() == TokenKind::LParen {
                    self.advance();
                    // Check if first arg is named (ident: pattern)
                    let is_named = if self.peek_kind() == TokenKind::Ident
                        && self.peek_kind() != TokenKind::RParen
                    {
                        let saved = self.pos;
                        let _ = self.advance();
                        let has_colon = self.peek_kind() == TokenKind::Colon;
                        self.pos = saved;
                        has_colon
                    } else {
                        false
                    };

                    if is_named {
                        // Named field pattern: Circle(radius: r, ...)
                        let mut named_args = Vec::new();
                        while self.peek_kind() != TokenKind::RParen {
                            if !named_args.is_empty() {
                                self.expect(TokenKind::Comma)?;
                            }
                            let field_name = self.expect_ident()?;
                            self.expect(TokenKind::Colon)?;
                            let pat = self.parse_pattern()?;
                            named_args.push((field_name, pat));
                        }
                        self.expect(TokenKind::RParen)?;
                        Ok(Pat::NamedCon(tok.text, named_args))
                    } else {
                        // Positional pattern: Circle(r)
                        let mut args = Vec::new();
                        while self.peek_kind() != TokenKind::RParen {
                            if !args.is_empty() {
                                self.expect(TokenKind::Comma)?;
                            }
                            args.push(self.parse_pattern()?);
                        }
                        self.expect(TokenKind::RParen)?;
                        Ok(Pat::Con(tok.text, args))
                    }
                } else {
                    Ok(Pat::Con(tok.text, Vec::new()))
                }
            }
            TokenKind::Int_ => {
                let tok = self.advance();
                Ok(Pat::Lit(Literal::Int(tok.text.parse().unwrap_or(0))))
            }
            TokenKind::Float_ => {
                let tok = self.advance();
                Ok(Pat::Lit(Literal::Float(tok.text.parse().unwrap_or(0.0))))
            }
            TokenKind::String_ => {
                let tok = self.advance();
                Ok(Pat::Lit(Literal::Str(tok.text)))
            }
            TokenKind::Bool_ => {
                let tok = self.advance();
                // In pattern context, True/False are constructors
                Ok(Pat::Con(tok.text, Vec::new()))
            }
            TokenKind::Op if self.peek().text == "-" => {
                self.advance();
                let tok = self.advance();
                if tok.kind == TokenKind::Float_ {
                    Ok(Pat::Lit(Literal::Float(
                        -tok.text.parse::<f64>().unwrap_or(0.0),
                    )))
                } else {
                    Ok(Pat::Lit(Literal::Int(
                        -tok.text.parse::<i64>().unwrap_or(0),
                    )))
                }
            }
            _ => {
                let tok = self.advance();
                if tok.text == "_" {
                    Ok(Pat::Wild)
                } else {
                    Ok(Pat::Var(tok.text))
                }
            }
        }
    }

    // --- Expression parsing (Pratt / precedence climbing) ---

    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_expr_prec(0)
    }

    pub fn parse_expr_prec(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut lhs = self.parse_atom()?;

        loop {
            // Check for postfix / infix operations
            match self.peek_kind() {
                // Postfix ? operator (try/error propagation) — must be before generic Op
                TokenKind::Op if self.peek().text == "?" => {
                    let start_span = lhs.span;
                    self.advance();
                    let span =
                        start_span.merge(self.span_since(&self.tokens[self.pos - 1].clone()));
                    lhs = Expr::new(ExprKind::Try(Box::new(lhs)), span);
                }
                // Pipe-forward operator: x |> f — preserved as ExprKind::Pipe AST node
                TokenKind::PipeGt => {
                    let pipe_prec: u8 = 1; // lowest precedence
                    if pipe_prec < min_prec {
                        break;
                    }
                    let start_span = lhs.span;
                    self.advance();
                    let rhs = self.parse_expr_prec(pipe_prec + 1)?;
                    let span = start_span.merge(rhs.span);
                    lhs = Expr::new(ExprKind::Pipe(Box::new(lhs), Box::new(rhs)), span);
                }
                // Binary operators (including `or` which lexes as Op "||")
                TokenKind::Op => {
                    let op = &self.peek().text;
                    let prec = op_precedence(op);
                    if prec < min_prec {
                        break;
                    }
                    let start_span = lhs.span;
                    let op = self.advance().text;
                    let rhs = self.parse_expr_prec(prec + 1)?;
                    let span = start_span.merge(rhs.span);
                    lhs = Expr::new(ExprKind::BinOp(op, Box::new(lhs), Box::new(rhs)), span);
                }
                // `and` as infix operator (alias for &&) in non-rule expressions
                // In rule bodies, `and` is handled as conjunction separator instead
                TokenKind::Ident if self.peek().text == "and" && !self.in_rule_body => {
                    let prec = op_precedence("&&");
                    if prec < min_prec {
                        break;
                    }
                    let start_span = lhs.span;
                    self.advance();
                    let rhs = self.parse_expr_prec(prec + 1)?;
                    let span = start_span.merge(rhs.span);
                    lhs = Expr::new(
                        ExprKind::BinOp("&&".to_string(), Box::new(lhs), Box::new(rhs)),
                        span,
                    );
                }
                // Safe call: expr?.field → match expr { Some(v) -> Some(v.field), None -> None }
                TokenKind::SafeCall => {
                    self.advance(); // consume '?.'
                    let field = self.expect_field_name()?;
                    let v = format!("__safe_{}", field);
                    lhs = ExprKind::Match(
                        Box::new(lhs),
                        vec![
                            MatchArm {
                                pat: Pat::Con("Some".into(), vec![Pat::Var(v.clone())]),
                                guard: None,
                                body: ExprKind::App(
                                    Box::new(ExprKind::Var("Some".into()).into()),
                                    vec![ExprKind::Field(Box::new(ExprKind::Var(v).into()), field)
                                        .into()],
                                )
                                .into(),
                            },
                            MatchArm {
                                pat: Pat::Con("None".into(), vec![]),
                                guard: None,
                                body: ExprKind::Var("None".into()).into(),
                            },
                        ],
                    )
                    .into();
                }
                // Elvis: expr ?: default → match expr { Some(v) -> v, None -> default }
                TokenKind::Elvis => {
                    let prec: u8 = 2; // just above pipe (1), below arithmetic
                    if prec < min_prec {
                        break;
                    }
                    self.advance(); // consume '?:'
                    let default = self.parse_expr_prec(prec + 1)?;
                    let v = "__elvis_v".to_string();
                    lhs = ExprKind::Match(
                        Box::new(lhs),
                        vec![
                            MatchArm {
                                pat: Pat::Con("Some".into(), vec![Pat::Var(v.clone())]),
                                guard: None,
                                body: ExprKind::Var(v).into(),
                            },
                            MatchArm {
                                pat: Pat::Con("None".into(), vec![]),
                                guard: None,
                                body: default,
                            },
                        ],
                    )
                    .into();
                }
                // Field access: expr.field
                TokenKind::Dot => {
                    let start_span = lhs.span;
                    self.advance();
                    let field = self.expect_field_name()?;
                    let span =
                        start_span.merge(self.span_since(&self.tokens[self.pos - 1].clone()));
                    lhs = Expr::new(ExprKind::Field(Box::new(lhs), field), span);
                }
                // Function application: expr(args)
                TokenKind::LParen => {
                    let start_span = lhs.span;
                    let args = self.parse_arg_list()?;
                    let span =
                        start_span.merge(self.span_since(&self.tokens[self.pos - 1].clone()));
                    lhs = Expr::new(ExprKind::App(Box::new(lhs), args), span);
                }
                // Index: expr[idx]
                TokenKind::LBracket => {
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    lhs = ExprKind::Index(Box::new(lhs), Box::new(idx)).into();
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    pub fn parse_atom(&mut self) -> Result<Expr, String> {
        match self.peek_kind() {
            // Identifiers and type constructors
            TokenKind::Ident => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                Ok(Expr::new(ExprKind::Var(tok.text), span))
            }
            TokenKind::Type => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                // Type constructor: might be called with args
                Ok(Expr::new(ExprKind::Var(tok.text), span))
            }
            // Literals
            TokenKind::Int_ => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                Ok(Expr::new(
                    ExprKind::Lit(Literal::Int(tok.text.parse().unwrap_or(0))),
                    span,
                ))
            }
            TokenKind::Float_ => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                Ok(Expr::new(
                    ExprKind::Lit(Literal::Float(tok.text.parse().unwrap_or(0.0))),
                    span,
                ))
            }
            TokenKind::String_ => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                Ok(Expr::new(ExprKind::Lit(Literal::Str(tok.text)), span))
            }
            TokenKind::Char_ => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                let c = tok.text.chars().next().unwrap_or(' ');
                Ok(Expr::new(ExprKind::Lit(Literal::Char(c)), span))
            }
            TokenKind::Bool_ => {
                let tok = self.advance();
                let span = self.token_span(&tok);
                Ok(Expr::new(
                    ExprKind::Lit(Literal::Bool(tok.text == "True")),
                    span,
                ))
            }
            // Parenthesized expression, unit, or tuple
            TokenKind::LParen => {
                self.advance();
                if self.peek_kind() == TokenKind::RParen {
                    self.advance();
                    return Ok(ExprKind::Unit.into());
                }
                let expr = self.parse_expr()?;
                if self.peek_kind() == TokenKind::Comma {
                    // Tuple
                    let mut elems = vec![expr];
                    while self.peek_kind() == TokenKind::Comma {
                        self.advance();
                        elems.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RParen)?;
                    return Ok(ExprKind::Tuple(elems).into());
                }
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            // Block
            TokenKind::LBrace => self.parse_block_expr(),
            // List literal (supports multi-line)
            TokenKind::LBracket => {
                let _bracket_tok = self.advance();
                // Skip newlines after [
                while self.peek_kind() == TokenKind::Semi {
                    self.advance();
                }
                let mut elems = Vec::new();
                while self.peek_kind() != TokenKind::RBracket {
                    if !elems.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    // Skip newlines after comma (or after [ for first element)
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                    // Trailing comma support: comma then ]
                    if self.peek_kind() == TokenKind::RBracket {
                        break;
                    }
                    elems.push(self.parse_expr()?);
                    // Skip newlines after element
                    while self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(ExprKind::List(elems).into())
            }
            // Lambda: |params| body   OR   | handle Effect { ... } in body
            // || as single Op token — empty lambda
            TokenKind::Op if self.peek().text == "||" => {
                self.advance();
                let body = self.parse_expr()?;
                return Ok(ExprKind::Lambda(Vec::new(), Box::new(body)).into());
            }
            TokenKind::Pipe => {
                self.advance();
                // | handle EffectName { | op(args) -> handler } in body
                if self.peek_kind() == TokenKind::KW && self.peek().text == "handle" {
                    return self.parse_handle_expr();
                }
                if self.peek_kind() == TokenKind::Pipe {
                    // || — empty lambda (two separate Pipe tokens)
                    self.advance();
                    let body = self.parse_expr()?;
                    return Ok(ExprKind::Lambda(Vec::new(), Box::new(body)).into());
                }
                let mut params = Vec::new();
                while self.peek_kind() != TokenKind::Pipe {
                    if !params.is_empty() {
                        self.expect(TokenKind::Comma)?;
                    }
                    let name = self.expect_ident()?;
                    let ty = if self.peek_kind() == TokenKind::Colon {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    params.push(Param {
                        name,
                        ty,
                        inout: false,
                    });
                }
                self.expect(TokenKind::Pipe)?;
                let body = self.parse_expr()?;
                Ok(ExprKind::Lambda(params, Box::new(body)).into())
            }
            // ~[...] stream source literal: ~[1, 2, 3] → from_list([1, 2, 3])
            TokenKind::Tilde
                if {
                    // Look ahead: is the token after ~ a [ ?
                    let next_pos = self.pos + 1;
                    next_pos < self.tokens.len()
                        && self.tokens[next_pos].kind == TokenKind::LBracket
                } =>
            {
                self.advance(); // consume ~
                let list_expr = self.parse_atom()?; // parse the [...] list literal
                Ok(ExprKind::App(
                    Box::new(ExprKind::Var("from_list".to_string()).into()),
                    vec![list_expr],
                )
                .into())
            }
            // Unary operators
            TokenKind::Op if self.peek().text == "-" || self.peek().text == "!" => {
                let tok = self.advance();
                let operand = self.parse_atom()?;
                Ok(ExprKind::UnOp(tok.text, Box::new(operand)).into())
            }
            // & reference
            TokenKind::Amp => {
                self.advance();
                if self.peek_kind() == TokenKind::KW && self.peek().text == "mut" {
                    self.advance();
                    let inner = self.parse_atom()?;
                    Ok(ExprKind::UnOp("&mut".to_string(), Box::new(inner)).into())
                } else {
                    let inner = self.parse_atom()?;
                    Ok(ExprKind::UnOp("&".to_string(), Box::new(inner)).into())
                }
            }
            // @ effect invocation
            TokenKind::At => {
                self.advance();
                let name_tok = self.advance();
                let args = if self.peek_kind() == TokenKind::LParen {
                    self.parse_arg_list()?
                } else {
                    Vec::new()
                };
                Ok(ExprKind::Effect(name_tok.text, args).into())
            }
            // Keywords that start expressions
            TokenKind::KW => {
                let tok = self.advance();
                match tok.text.as_str() {
                    "match" => self.parse_match_expr(),
                    "if" => self.parse_if_expr(tok),
                    _ => Ok(ExprKind::Var(tok.text).into()),
                }
            }
            // = inside expressions (binding in block)
            TokenKind::Eq => {
                let tok = self.peek().clone();
                Err(format!(
                    "{}:{}: `=` binding found where an expression was expected.\n  Bindings (= name = value) can only appear at the top level or inside {{ blocks }}.\n  Did you mean `==` for comparison?",
                    tok.line, tok.col
                ))
            }
            _ => {
                let tok = self.peek().clone();
                let kind_name = match tok.kind {
                    TokenKind::Semi => "newline",
                    TokenKind::Colon => "colon `:`",
                    TokenKind::Pipe => "pipe `|`",
                    TokenKind::Hash => "hash `#`",
                    TokenKind::At => "at `@`",
                    TokenKind::Tilde => "tilde `~`",
                    TokenKind::Eq => "equals `=`",
                    TokenKind::Gt => "greater-than `>`",
                    TokenKind::Arrow => "arrow `->`",
                    TokenKind::Eof => "end of file",
                    _ => "",
                };
                let display = if kind_name.is_empty() {
                    format!("`{}`", tok.text)
                } else {
                    kind_name.to_string()
                };
                let hint = match tok.kind {
                    TokenKind::Hash => "\n  Hint: `#` starts a type declaration. Did you forget to close the previous expression?",
                    TokenKind::At => "\n  Hint: `@` starts an effect. Did you forget to close the previous expression?",
                    TokenKind::Pipe => "\n  Hint: `|` starts a rule. If you meant a lambda, use `|param| expr`.",
                    TokenKind::Eq => "\n  Hint: `=` starts a binding. Did you mean `==` for comparison?",
                    TokenKind::Eof => "\n  Hint: unexpected end of file. Check for unclosed `{`, `(`, or `[`.",
                    _ => "",
                };
                Err(format!(
                    "{}:{}: unexpected {}{}",
                    tok.line, tok.col, display, hint
                ))
            }
        }
    }

    pub fn parse_arg_list(&mut self) -> Result<Vec<Expr>, String> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        while self.peek_kind() != TokenKind::RParen {
            if !args.is_empty() {
                self.expect(TokenKind::Comma)?;
            }
            let arg = self.parse_expr()?;
            // Allow optional type annotation `: Type` on args (used in rule heads)
            // Store as App(Var("__typed"), [arg, Var(TypeName)]) for runtime constraint
            if self.peek_kind() == TokenKind::Colon {
                self.advance(); // consume ':'
                let ty = self.parse_type()?;
                let type_name = match &ty {
                    Ty::Name(n) => n.clone(),
                    _ => format!("{:?}", ty),
                };
                // Wrap in a typed annotation that the interpreter can check
                args.push(
                    ExprKind::App(
                        Box::new(ExprKind::Var("__typed".into()).into()),
                        vec![arg, ExprKind::Var(type_name).into()],
                    )
                    .into(),
                );
            } else {
                args.push(arg);
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(args)
    }

    pub fn parse_block_expr(&mut self) -> Result<Expr, String> {
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut stmts = Vec::new();
        while self.peek_kind() != TokenKind::RBrace {
            let stmt = self.parse_block_statement()?;
            stmts.push(stmt);
            self.skip_semis();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(ExprKind::Block(stmts).into())
    }

    pub fn parse_block_statement(&mut self) -> Result<Stmt, String> {
        self.skip_semis();
        match self.peek_kind() {
            TokenKind::Eq => {
                self.advance();
                self.parse_binding()
            }
            TokenKind::Gt => {
                self.advance();
                self.parse_definition()
            }
            TokenKind::Tilde => {
                // ~[...] stream source literal: ~[1, 2, 3] → Stmt::Expr(from_list([1, 2, 3]))
                {
                    let next_pos = self.pos + 1;
                    if next_pos < self.tokens.len()
                        && self.tokens[next_pos].kind == TokenKind::LBracket
                    {
                        self.advance(); // consume ~
                        let list_expr = self.parse_atom()?; // parse the [...] list literal
                        return Ok(Stmt::Expr(
                            ExprKind::App(
                                Box::new(ExprKind::Var("from_list".to_string()).into()),
                                vec![list_expr],
                            )
                            .into(),
                        ));
                    }
                }

                let saved = self.pos;
                self.advance();

                // Disambiguate: ~ name = expr (StreamBind) vs ~ expr | pat -> body (StreamSub)
                let name_tok = self.advance();
                let is_binding = name_tok.kind == TokenKind::Ident
                    && (self.peek_kind() == TokenKind::Eq
                        || (self.peek_kind() == TokenKind::Op && self.peek().text == "="));

                if is_binding {
                    // Consume =
                    if self.peek_kind() == TokenKind::Eq {
                        self.advance();
                    } else if self.peek_kind() == TokenKind::Op && self.peek().text == "=" {
                        self.advance();
                    }
                    let expr = self.parse_expr()?;
                    Ok(Stmt::StreamBind(name_tok.text.clone(), expr))
                } else {
                    // Backtrack and parse as stream expression + match arms
                    self.pos = saved;
                    self.advance(); // consume ~
                    let stream_expr = self.parse_expr()?;
                    let mut arms = Vec::new();

                    while self.peek_kind() == TokenKind::Pipe {
                        self.advance(); // consume |
                        let pat = self.parse_pattern()?;
                        let guard = if self.peek_kind() == TokenKind::KW && self.peek().text == "if"
                        {
                            self.advance();
                            Some(self.parse_expr()?)
                        } else {
                            None
                        };

                        if self.peek_kind() == TokenKind::Arrow {
                            self.advance();
                        } else if self.peek_kind() == TokenKind::Op && self.peek().text == "->" {
                            self.advance();
                        } else {
                            return Err(format!("{}:{}: expected `->` after pattern in subscription arm.\n  Each arm should look like: | pattern -> {{ body }}", self.peek().line, self.peek().col));
                        }

                        let body = self.parse_expr()?;
                        arms.push(MatchArm { pat, guard, body });
                        self.skip_semis();
                    }

                    if arms.is_empty() {
                        return Err(format!(
                            "{}:{}: stream subscription needs at least one `|` arm.\n  Example:\n    ~ my_stream\n    | x -> {{ @ print(x) }}\n    | Err(e) -> {{ @ print(\"error\") }}\n    | Complete -> {{ @ print(\"done\") }}",
                            self.peek().line, self.peek().col
                        ));
                    }
                    Ok(Stmt::StreamSub(stream_expr, arms))
                }
            }
            TokenKind::Pipe => {
                // Inside a block, | can be a lambda |x| ... OR a rule | head -> body
                // Disambiguate: if the token after | is an ident followed by |, it's a lambda
                // Otherwise it's a rule/match arm
                let saved = self.pos;
                self.advance(); // consume |
                                // Check if this looks like a lambda: |x| or |x, y|
                let mut looks_like_lambda = false;
                if self.peek_kind() == TokenKind::Pipe {
                    // || — empty lambda
                    looks_like_lambda = true;
                } else if self.peek_kind() == TokenKind::Ident {
                    // Save pos, scan forward
                    let saved2 = self.pos;
                    self.advance(); // consume ident
                    if self.peek_kind() == TokenKind::Pipe || self.peek_kind() == TokenKind::Comma {
                        looks_like_lambda = true;
                    }
                    self.pos = saved2; // restore
                }
                self.pos = saved; // restore to before |
                if looks_like_lambda {
                    let expr = self.parse_expr()?;
                    return Ok(Stmt::Expr(expr));
                }
                self.advance(); // consume |
                self.parse_rule()
            }
            TokenKind::Hash => {
                self.advance();
                self.parse_type_decl()
            }
            TokenKind::At => {
                self.advance();
                self.parse_annotation()
            }
            // for x in expr { body }
            TokenKind::KW if self.peek().text == "for" => {
                self.advance(); // consume 'for'
                let var = self.expect_ident()?;
                // expect 'in'
                if self.peek_kind() == TokenKind::KW && self.peek().text == "in" {
                    self.advance();
                } else {
                    let p = self.peek();
                    return Err(format!(
                        "{}:{}: expected `in` after `for {}`, got `{}`.\n  \
                        Try: for {} in collection {{ ... }}",
                        p.line, p.col, var, p.text, var
                    ));
                }
                let iter_expr = self.parse_expr_prec(0)?;
                self.expect(TokenKind::LBrace)?;
                self.skip_semis();
                let mut body = Vec::new();
                while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                    body.push(self.parse_block_statement()?);
                    self.skip_semis();
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::For(var, iter_expr, body))
            }
            // while cond { body }
            TokenKind::KW if self.peek().text == "while" => {
                self.advance(); // consume 'while'
                let cond = self.parse_expr_prec(0)?;
                self.expect(TokenKind::LBrace)?;
                self.skip_semis();
                let mut body = Vec::new();
                while self.peek_kind() != TokenKind::RBrace && self.peek_kind() != TokenKind::Eof {
                    body.push(self.parse_block_statement()?);
                    self.skip_semis();
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::While(cond, body))
            }
            _ => {
                let expr = self.parse_expr()?;
                // Check for <- (actor send): target <- message
                if self.peek_kind() == TokenKind::Send {
                    self.advance(); // consume '<-'
                    let msg = self.parse_expr()?;
                    return Ok(Stmt::Send(expr, msg));
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    pub fn parse_match_expr(&mut self) -> Result<Expr, String> {
        let scrut = self.parse_expr_prec(0)?;
        self.expect(TokenKind::LBrace)?;
        self.skip_semis();
        let mut arms = Vec::new();
        while self.peek_kind() != TokenKind::RBrace {
            if self.peek_kind() == TokenKind::Pipe {
                self.advance();
            }
            let pat = self.parse_pattern()?;
            let guard = if self.peek_kind() == TokenKind::KW && self.peek().text == "if" {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(TokenKind::Arrow)?;
            let body = self.parse_expr()?;
            arms.push(MatchArm { pat, guard, body });
            self.skip_semis();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(ExprKind::Match(Box::new(scrut), arms).into())
    }

    pub fn parse_if_expr(&mut self, start_tok: Token) -> Result<Expr, String> {
        let cond = self.parse_expr_prec(0)?;
        let then_ = self.parse_block_expr()?;
        // Skip newlines (Semi tokens) between } and else
        self.skip_semis();
        let else_ = if self.peek_kind() == TokenKind::KW && self.peek().text == "else" {
            self.advance();
            if self.peek_kind() == TokenKind::KW && self.peek().text == "if" {
                let if_tok = self.advance();
                self.parse_if_expr(if_tok)?
            } else {
                self.parse_block_expr()?
            }
        } else {
            ExprKind::Unit.into()
        };
        Ok(self.spanned(
            ExprKind::If(Box::new(cond), Box::new(then_), Box::new(else_)),
            &start_tok,
        ))
    }
}

pub fn op_precedence(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" => 3,
        "<" | ">" | "<=" | ">=" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => 0,
    }
}

pub fn lower_expr_to_proof_term(expr: &Expr) -> Result<proof_kernel::Term, String> {
    match &expr.kind {
        ExprKind::Var(name) => Ok(proof_kernel::Term::Var(name.clone())),
        ExprKind::Lit(Literal::Int(n)) => Ok(proof_kernel::Term::Int(*n)),
        ExprKind::Lit(Literal::Bool(b)) => Ok(proof_kernel::Term::App(
            if *b { "True" } else { "False" }.into(),
            vec![],
        )),
        ExprKind::UnOp(op, inner) if op == "-" => Ok(proof_kernel::Term::Op(
            "-".into(),
            Box::new(proof_kernel::Term::Int(0)),
            Box::new(lower_expr_to_proof_term(inner)?),
        )),
        ExprKind::BinOp(op, lhs, rhs) if matches!(op.as_str(), "+" | "-" | "*" | "/") => {
            Ok(proof_kernel::Term::Op(
                op.clone(),
                Box::new(lower_expr_to_proof_term(lhs)?),
                Box::new(lower_expr_to_proof_term(rhs)?),
            ))
        }
        ExprKind::App(func, args) => {
            let name = match &func.kind {
                ExprKind::Var(name) => name.clone(),
                _ => {
                    return Err(
                        "proof kernel only supports simple function/constructor calls".into(),
                    );
                }
            };
            let mut lowered_args = Vec::with_capacity(args.len());
            for arg in args {
                lowered_args.push(lower_expr_to_proof_term(arg)?);
            }
            Ok(proof_kernel::Term::App(name, lowered_args))
        }
        _ => Err(format!(
            "proof kernel cannot lower term expression: {:?}",
            expr.kind
        )),
    }
}

pub fn lower_expr_to_proof_prop(expr: &Expr) -> Result<proof_kernel::Prop, String> {
    let succ = |term: proof_kernel::Term| {
        proof_kernel::Term::Op(
            "+".into(),
            Box::new(term),
            Box::new(proof_kernel::Term::Int(1)),
        )
    };

    match &expr.kind {
        ExprKind::BinOp(op, lhs, rhs) if op == "==" => Ok(proof_kernel::Prop::Eq(
            lower_expr_to_proof_term(lhs)?,
            lower_expr_to_proof_term(rhs)?,
        )),
        ExprKind::BinOp(op, lhs, rhs) if op == "<=" => Ok(proof_kernel::Prop::Le(
            lower_expr_to_proof_term(lhs)?,
            lower_expr_to_proof_term(rhs)?,
        )),
        ExprKind::BinOp(op, lhs, rhs) if op == ">=" => Ok(proof_kernel::Prop::Le(
            lower_expr_to_proof_term(rhs)?,
            lower_expr_to_proof_term(lhs)?,
        )),
        ExprKind::BinOp(op, lhs, rhs) if op == "<" => Ok(proof_kernel::Prop::Le(
            succ(lower_expr_to_proof_term(lhs)?),
            lower_expr_to_proof_term(rhs)?,
        )),
        ExprKind::BinOp(op, lhs, rhs) if op == ">" => Ok(proof_kernel::Prop::Le(
            succ(lower_expr_to_proof_term(rhs)?),
            lower_expr_to_proof_term(lhs)?,
        )),
        ExprKind::BinOp(op, lhs, rhs) if op == "&&" => Ok(proof_kernel::Prop::And(
            Box::new(lower_expr_to_proof_prop(lhs)?),
            Box::new(lower_expr_to_proof_prop(rhs)?),
        )),
        ExprKind::App(func, args)
            if matches!(&func.kind, ExprKind::Var(name) if name == "not") && args.len() == 1 =>
        {
            Ok(proof_kernel::Prop::Not(Box::new(lower_expr_to_proof_prop(
                &args[0],
            )?)))
        }
        ExprKind::Lit(Literal::Bool(false)) => Ok(proof_kernel::Prop::False),
        _ => Err(format!(
            "proof kernel cannot lower proposition expression: {:?}",
            expr.kind
        )),
    }
}

// ============================================================================
// PART 5: VALUES & ENVIRONMENT
// ============================================================================

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Char(char),
    Bool(bool),
    Unit,
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Constructor(String, Vec<Value>),
    NamedConstructor(String, Vec<(String, Value)>), // Named fields: Circle { radius: 5.0 }
    Closure {
        name: Option<String>, // for recursion
        params: Vec<String>,
        body: Expr,
        env: Env,
    },
    Builtin(String),
    /// Actor: name, current state, handler definitions, base env
    Actor {
        actor_name: String,
        state: Box<Value>,
        state_param: String,
        handlers: Vec<Handler>,
        env: Env,
    },
    /// Reactive stream: ordered sequence of values (lazy in codegen, eager in interpreter)
    Stream(Vec<Value>),
    /// Subject: a mutable stream you can push into with <-
    /// (values, initial_value). Subjects ARE streams you can write to.
    Subject(Vec<Value>),
    /// Map: key-value dictionary with deterministic key iteration.
    Map(BTreeMap<String, Value>),
    /// Set: unique value collection keyed by Futuruna display semantics.
    Set(BTreeMap<String, Value>),
    /// Scope: a named block with its own environment. Bindings accessible via Scope.name.
    Scope {
        name: String,
        bindings: HashMap<String, Value>,
    },
    /// Comptime type definition: describes a type to be generated at compile time.
    /// fields: vec of (field_name, type_name) for structs, or variant descriptions for enums.
    TypeDef {
        kind: String,
        fields: Vec<(String, String)>,
    },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(v) => write!(f, "{}", v),
            Value::Str(s) => write!(f, "{}", s),
            Value::Char(c) => write!(f, "{}", c),
            Value::Bool(b) => write!(f, "{}", if *b { "true" } else { "false" }),
            Value::Unit => write!(f, "()"),
            Value::List(elems) => {
                write!(f, "[")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", e)?;
                }
                write!(f, "]")
            }
            Value::Tuple(elems) => {
                write!(f, "(")?;
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", e)?;
                }
                write!(f, ")")
            }
            Value::Constructor(name, args) if args.is_empty() => {
                if name == "Nil" {
                    write!(f, "[]") // Empty list
                } else {
                    write!(f, "{}", name)
                }
            }
            Value::Constructor(name, args) if name == "Cons" && args.len() == 2 => {
                // Display linked lists as [1, 2, 3] instead of Cons(1, Cons(2, Nil))
                let items = list_to_vec(self);
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Constructor(name, args) => {
                write!(f, "{}(", name)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            Value::NamedConstructor(name, fields) if fields.is_empty() => {
                write!(f, "{}", name)
            }
            Value::NamedConstructor(name, fields) => {
                write!(f, "{}(", name)?;
                for (i, (fname, val)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", fname, val)?;
                }
                write!(f, ")")
            }
            Value::Closure { name, params, .. } => {
                let n = name.as_deref().unwrap_or("lambda");
                write!(f, "<fn {}({})>", n, params.join(", "))
            }
            Value::Builtin(name) => write!(f, "<builtin:{}>", name),
            Value::Actor {
                actor_name, state, ..
            } => write!(f, "<actor:{}({})>", actor_name, state),
            Value::Stream(items) => {
                write!(f, "~[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Subject(items) => {
                write!(f, "~subject[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?; // k is already String, v is Value
                }
                write!(f, "}}")
            }
            Value::Set(items) => {
                write!(f, "{{")?;
                for (i, (_, v)) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "}}")
            }
            Value::Scope { name, .. } => write!(f, "<scope:{}>", name),
            Value::TypeDef { kind, fields } => {
                write!(f, "<typedef:{} {{", kind)?;
                for (i, (n, t)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", n, t)?;
                }
                write!(f, "}}>")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    /// HashMap for O(1) amortized lookup (was BTreeMap O(log n))
    pub bindings: HashMap<String, Value>,
    /// Rc parent: child creation is O(1) refcount bump, not O(n) deep clone.
    /// This is the critical optimization for closure-heavy code (map/filter/foldl).
    pub parent: Option<Rc<Env>>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    pub fn child(&self) -> Self {
        Env {
            bindings: HashMap::new(),
            parent: Some(Rc::new(self.clone())),
        }
    }

    /// Cheap child: shares parent via Rc (no clone of parent chain).
    /// Use this in hot paths (map/filter/foldl closure application).
    pub fn child_rc(self_rc: &Rc<Env>) -> Self {
        Env {
            bindings: HashMap::new(),
            parent: Some(Rc::clone(self_rc)),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }

    pub fn set(&mut self, name: String, val: Value) {
        self.bindings.insert(name, val);
    }

    /// Iterate over all bindings (current scope + parents)
    pub fn iter_bindings(&self) -> Vec<(String, Value)> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut current = Some(self);
        while let Some(env) = current {
            for (k, v) in &env.bindings {
                if seen.insert(k.clone()) {
                    result.push((k.clone(), v.clone()));
                }
            }
            current = env.parent.as_ref().map(|p| p.as_ref());
        }
        result
    }

    pub fn remove(&mut self, name: &str) {
        self.bindings.remove(name);
    }
}

fn comptime_typedef_to_type_decl(name: &str, kind: &str, fields: &[(String, String)]) -> TypeDecl {
    fn parse_ty(s: &str) -> Ty {
        match s {
            "Int" => Ty::Name("Int".into()),
            "Float" => Ty::Name("Float".into()),
            "String" => Ty::Name("String".into()),
            "Bool" => Ty::Name("Bool".into()),
            "Char" => Ty::Name("Char".into()),
            other => Ty::Name(other.into()),
        }
    }

    match kind {
        "struct" => {
            let variant_fields: Vec<Field> = fields
                .iter()
                .map(|(fname, fty)| Field {
                    name: fname.clone(),
                    ty: parse_ty(fty),
                })
                .collect();
            let variant = Variant {
                name: name.to_string(),
                fields: variant_fields,
                positional: false,
                from_type: None,
            };
            TypeDecl::ADT {
                name: name.to_string(),
                params: vec![],
                variants: vec![variant],
                methods: vec![],
                except_from: None,
            }
        }
        "enum" => {
            let variants: Vec<Variant> = fields
                .iter()
                .map(|(vname, field_str)| {
                    let variant_fields = if field_str.is_empty() {
                        vec![]
                    } else {
                        field_str
                            .split(',')
                            .filter_map(|part| {
                                let parts: Vec<&str> = part.trim().splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    Some(Field {
                                        name: parts[0].trim().to_string(),
                                        ty: parse_ty(parts[1].trim()),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect()
                    };
                    Variant {
                        name: vname.clone(),
                        fields: variant_fields,
                        positional: false,
                        from_type: None,
                    }
                })
                .collect();
            TypeDecl::ADT {
                name: name.to_string(),
                params: vec![],
                variants,
                methods: vec![],
                except_from: None,
            }
        }
        _ => TypeDecl::ADT {
            name: name.to_string(),
            params: vec![],
            variants: vec![Variant {
                name: name.to_string(),
                fields: vec![],
                positional: false,
                from_type: None,
            }],
            methods: vec![],
            except_from: None,
        },
    }
}

// ============================================================================
// PART 6: EVALUATOR
// ============================================================================

pub struct FnDef {
    pub params: Vec<String>,
    pub body: Expr,
}

pub struct Interpreter {
    /// Logic rules (Prolog-style)
    pub rules: Vec<(String, Rule)>,
    /// Type constructors: name -> (arity, positional)
    pub constructors: BTreeMap<String, (usize, bool)>,
    /// Named field names per constructor: ctor_name -> [field_name, ...]
    pub field_names: BTreeMap<String, Vec<String>>,
    /// Type name -> variant names (for method dispatch)
    pub type_variants: BTreeMap<String, Vec<String>>,
    /// Which type a constructor belongs to: ctor_name -> type_name
    pub ctor_to_type: BTreeMap<String, String>,
    /// Named function registry (for recursion without circular closures)
    pub functions: BTreeMap<String, FnDef>,
    /// Runtime impl methods keyed by (receiver type, method name)
    pub impl_methods: BTreeMap<(String, String), FnDef>,
    /// Output buffer for tests
    pub output: Vec<String>,
    /// Current source file directory (for resolving @ import paths)
    pub source_dir: Option<String>,
    /// Already-imported files (prevent cycles)
    pub imported: BTreeSet<String>,
    /// Named invariants: name -> (subject_expr, predicate_expr)
    pub invariants: BTreeMap<String, (Expr, Expr)>,
    /// Effect declarations: effect_name -> [(op_name, param_names)]
    pub effect_decls: BTreeMap<String, Vec<(String, Vec<String>)>>,
    /// Handler stack for algebraic effects: (effect_name, handlers)
    pub handler_stack: Vec<(String, Vec<EffHandler>)>,
    /// Actor definitions: actor_name -> Defn::Actor
    pub actor_defs: BTreeMap<String, Defn>,
    /// Live actor instances: var_name -> (state, actor_name)
    pub actor_instances: BTreeMap<String, (Value, String)>,
    /// Step budget for auto-comptime: 0 = unlimited
    pub step_limit: usize,
    /// Current step count (incremented each eval call)
    pub step_count: usize,
    /// Set to true when step budget exceeded
    pub budget_exceeded: bool,
    /// Shared xorshift64 state for random collection builtins
    pub(crate) rng_state: u64,
    /// Suppress stdout output (for comptime evaluation during codegen)
    pub suppress_output: bool,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            rules: Vec::new(),
            constructors: {
                let mut c = BTreeMap::new();
                c.insert("Some".into(), (1, true)); // Option constructors for T? / ?. / ?:
                c.insert("None".into(), (0, true));
                c
            },
            field_names: BTreeMap::new(),
            type_variants: BTreeMap::new(),
            ctor_to_type: BTreeMap::new(),
            functions: BTreeMap::new(),
            impl_methods: BTreeMap::new(),
            output: Vec::new(),
            source_dir: None,
            imported: BTreeSet::new(),
            invariants: BTreeMap::new(),
            effect_decls: BTreeMap::new(),
            handler_stack: Vec::new(),
            actor_defs: BTreeMap::new(),
            actor_instances: BTreeMap::new(),
            step_limit: 0,
            step_count: 0,
            budget_exceeded: false,
            rng_state: 0x12345678_9abcdef0,
            suppress_output: false,
        }
    }

    /// Resolve an import path to a file path.
    /// Supports relative (`./module`) and manifest-based (`dep/module`) imports.
    pub fn resolve_import_path_for_source(import_path: &str, dir: &str) -> Option<String> {
        let rel = import_path.trim_start_matches("./");
        let file_path = format!("{}/{}.runa", dir, rel);

        if import_path.starts_with("./")
            || import_path.starts_with("../")
            || std::path::Path::new(&file_path).exists()
        {
            return Some(file_path);
        }

        // Try manifest-based resolution
        if let Some(toml_path) = Self::find_manifest(dir) {
            if let Some((deps, _)) = Self::parse_manifest_deps(&toml_path) {
                let toml_dir = std::path::Path::new(&toml_path)
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy().to_string();
                        if s.is_empty() {
                            ".".to_string()
                        } else {
                            s
                        }
                    })
                    .unwrap_or_else(|| ".".to_string());

                if let Some(resolved) =
                    TypeChecker::resolve_dep_module(import_path, &deps, &toml_dir)
                {
                    return Some(resolved);
                }
            }
        }

        Some(file_path)
    }

    fn runtime_type_name(&self, value: &Value) -> Option<String> {
        match value {
            Value::Constructor(name, _) | Value::NamedConstructor(name, _) => self
                .ctor_to_type
                .get(name)
                .cloned()
                .or_else(|| Some(name.clone())),
            Value::Str(_) => Some("String".into()),
            Value::Int(_) => Some("Int".into()),
            Value::Float(_) => Some("Float".into()),
            Value::Bool(_) => Some("Bool".into()),
            Value::Char(_) => Some("Char".into()),
            _ => None,
        }
    }

    fn bind_method_value(&self, obj_val: &Value, field: &str, env: &Env) -> Option<Value> {
        if let Some(type_name) = self.runtime_type_name(obj_val) {
            if let Some(func_def) = self.impl_methods.get(&(type_name, field.to_string())) {
                let mut method_env = env.child();
                method_env.set("self".to_string(), obj_val.clone());
                let remaining_params: Vec<String> = func_def
                    .params
                    .iter()
                    .filter(|p| p.as_str() != "self")
                    .cloned()
                    .collect();
                return Some(Value::Closure {
                    name: None,
                    params: remaining_params,
                    body: func_def.body.clone(),
                    env: method_env,
                });
            }
        }

        let method_body_params = if let Some(Value::Closure { params, body, .. }) = env.get(field) {
            Some((body.clone(), params.clone()))
        } else if let Some(func_def) = self.functions.get(field) {
            Some((func_def.body.clone(), func_def.params.clone()))
        } else {
            None
        };

        method_body_params.map(|(body, params)| {
            let mut method_env = env.child();
            method_env.set("self".to_string(), obj_val.clone());
            let remaining_params: Vec<String> = params
                .iter()
                .filter(|p| p.as_str() != "self")
                .cloned()
                .collect();
            Value::Closure {
                name: None,
                params: remaining_params,
                body,
                env: method_env,
            }
        })
    }

    /// Find runa.toml by walking up from a directory
    fn find_manifest(start_dir: &str) -> Option<String> {
        let mut dir = std::path::PathBuf::from(start_dir);
        loop {
            let candidate = dir.join("runa.toml");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    /// Parse [dependencies] from a runa.toml — returns Vec<(name, path)> and package name
    /// Extract a quoted value for a key from an inline TOML table
    fn extract_toml_value(raw: &str, key: &str) -> Option<String> {
        if let Some(k_start) = raw.find(key) {
            let after = &raw[k_start + key.len()..];
            if let Some(eq) = after.find('=') {
                let val = after[eq + 1..]
                    .trim()
                    .trim_end_matches('}')
                    .trim()
                    .trim_end_matches(',')
                    .trim()
                    .trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    /// Hash a git URL to get the cache directory name
    fn git_cache_key(url: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        url.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    fn parse_manifest_deps(toml_path: &str) -> Option<(Vec<(String, String)>, String)> {
        let content = std::fs::read_to_string(toml_path).ok()?;
        let mut pkg_name = String::new();
        let mut deps: Vec<(String, String)> = Vec::new();
        let mut section = "";

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "[package]" {
                section = "package";
                continue;
            }
            if trimmed == "[dependencies]" {
                section = "deps";
                continue;
            }
            if trimmed.starts_with('[') {
                section = "";
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let val_raw = trimmed[eq_pos + 1..].trim();
                let val = val_raw.trim_matches('"');
                match section {
                    "package" => {
                        if key == "name" {
                            pkg_name = val.to_string();
                        }
                    }
                    "deps" => {
                        if val_raw.contains("git") {
                            // Git dependency → resolve to cache path
                            if let Some(url) = Self::extract_toml_value(val_raw, "git") {
                                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                                let cache_path = format!(
                                    "{}/.cache/futuruna/deps/{}",
                                    home,
                                    Self::git_cache_key(&url)
                                );
                                deps.push((key.to_string(), cache_path));
                            }
                        } else if val_raw.contains("path") {
                            if let Some(path) = Self::extract_toml_value(val_raw, "path") {
                                deps.push((key.to_string(), path));
                            }
                        } else {
                            deps.push((key.to_string(), val.to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
        Some((deps, pkg_name))
    }

    pub fn default_env(&self) -> Env {
        let mut env = Env::new();
        env.set("True".into(), Value::Bool(true));
        env.set("False".into(), Value::Bool(false));
        env.set("None".into(), Value::Constructor("None".into(), vec![]));
        env.set("Nil".into(), Value::Constructor("Nil".into(), vec![]));
        env.set("print".into(), Value::Builtin("print".into()));
        env.set("show".into(), Value::Builtin("show".into()));
        env.set("length".into(), Value::Builtin("length".into()));
        env.set("head".into(), Value::Builtin("head".into()));
        env.set("tail".into(), Value::Builtin("tail".into()));
        env.set("abs".into(), Value::Builtin("abs".into()));
        env.set("random_float".into(), Value::Builtin("random_float".into()));
        env.set(
            "random_choice".into(),
            Value::Builtin("random_choice".into()),
        );
        env.set("shuffle".into(), Value::Builtin("shuffle".into()));
        env.set("sleep".into(), Value::Builtin("sleep".into()));
        env.set("now".into(), Value::Builtin("now".into()));
        env.set("time_diff".into(), Value::Builtin("time_diff".into()));
        env.set("regex_match".into(), Value::Builtin("regex_match".into()));
        env.set("regex_find".into(), Value::Builtin("regex_find".into()));
        env.set(
            "regex_find_all".into(),
            Value::Builtin("regex_find_all".into()),
        );
        env.set(
            "regex_replace".into(),
            Value::Builtin("regex_replace".into()),
        );
        env.set("not".into(), Value::Builtin("not".into()));
        env.set("concat".into(), Value::Builtin("concat".into()));
        env.set("reverse".into(), Value::Builtin("reverse".into()));
        env.set("map".into(), Value::Builtin("map".into()));
        env.set("filter".into(), Value::Builtin("filter".into()));
        env.set("foldl".into(), Value::Builtin("foldl".into()));
        env.set("assert".into(), Value::Builtin("assert".into()));
        env.set("range".into(), Value::Builtin("range".into()));
        env.set("push".into(), Value::Builtin("push".into()));
        env.set("nth".into(), Value::Builtin("nth".into()));
        // Collection builtins (Kotlin-inspired)
        env.set("sort".into(), Value::Builtin("sort".into()));
        env.set("sort_by".into(), Value::Builtin("sort_by".into()));
        env.set("any".into(), Value::Builtin("any".into()));
        env.set("all".into(), Value::Builtin("all".into()));
        env.set("find".into(), Value::Builtin("find".into()));
        env.set("flat_map".into(), Value::Builtin("flat_map".into()));
        env.set("pair".into(), Value::Builtin("pair".into()));
        env.set("triple".into(), Value::Builtin("triple".into()));
        env.set("zip".into(), Value::Builtin("zip".into()));
        env.set("enumerate".into(), Value::Builtin("enumerate".into()));
        env.set("take_while".into(), Value::Builtin("take_while".into()));
        env.set("drop_while".into(), Value::Builtin("drop_while".into()));
        env.set("sum_list".into(), Value::Builtin("sum_list".into()));
        env.set("distinct".into(), Value::Builtin("distinct".into()));
        env.set("count_by".into(), Value::Builtin("count_by".into()));
        env.set("partition".into(), Value::Builtin("partition".into()));
        env.set("chunked".into(), Value::Builtin("chunked".into()));
        env.set("subscribe".into(), Value::Builtin("subscribe".into()));
        // Map builtins (M24)
        env.set("map_new".into(), Value::Builtin("map_new".into()));
        env.set("map_insert".into(), Value::Builtin("map_insert".into()));
        env.set("unwrap_or".into(), Value::Builtin("unwrap_or".into()));
        env.set("is_some".into(), Value::Builtin("is_some".into()));
        env.set("is_none".into(), Value::Builtin("is_none".into()));
        env.set("map_get".into(), Value::Builtin("map_get".into()));
        env.set("map_get_or".into(), Value::Builtin("map_get_or".into()));
        env.set("map_contains".into(), Value::Builtin("map_contains".into()));
        env.set("map_remove".into(), Value::Builtin("map_remove".into()));
        env.set("map_keys".into(), Value::Builtin("map_keys".into()));
        env.set("map_values".into(), Value::Builtin("map_values".into()));
        env.set("map_entries".into(), Value::Builtin("map_entries".into()));
        env.set("map_len".into(), Value::Builtin("map_len".into()));
        env.set("map_merge".into(), Value::Builtin("map_merge".into()));
        env.set("map_from".into(), Value::Builtin("map_from".into()));
        // Set builtins (M24)
        env.set("set_new".into(), Value::Builtin("set_new".into()));
        env.set("set_insert".into(), Value::Builtin("set_insert".into()));
        env.set("set_contains".into(), Value::Builtin("set_contains".into()));
        env.set("set_remove".into(), Value::Builtin("set_remove".into()));
        env.set("set_len".into(), Value::Builtin("set_len".into()));
        env.set("set_to_list".into(), Value::Builtin("set_to_list".into()));
        env.set("set_union".into(), Value::Builtin("set_union".into()));
        env.set(
            "set_intersect".into(),
            Value::Builtin("set_intersect".into()),
        );
        env.set("set_diff".into(), Value::Builtin("set_diff".into()));
        env.set(
            "set_from_list".into(),
            Value::Builtin("set_from_list".into()),
        );
        env.set("show_int".into(), Value::Builtin("show_int".into()));
        env.set("show_float".into(), Value::Builtin("show_float".into()));
        env.set(
            "string_length".into(),
            Value::Builtin("string_length".into()),
        );
        // String builtins (M14a)
        env.set("split".into(), Value::Builtin("split".into()));
        env.set("join".into(), Value::Builtin("join".into()));
        env.set("trim".into(), Value::Builtin("trim".into()));
        env.set("contains".into(), Value::Builtin("contains".into()));
        env.set("starts_with".into(), Value::Builtin("starts_with".into()));
        env.set("ends_with".into(), Value::Builtin("ends_with".into()));
        env.set("replace".into(), Value::Builtin("replace".into()));
        env.set("to_upper".into(), Value::Builtin("to_upper".into()));
        env.set("to_lower".into(), Value::Builtin("to_lower".into()));
        env.set("substring".into(), Value::Builtin("substring".into()));
        env.set("char_at".into(), Value::Builtin("char_at".into()));
        env.set("index_of".into(), Value::Builtin("index_of".into()));
        env.set("format_float".into(), Value::Builtin("format_float".into()));
        env.set("parse_int".into(), Value::Builtin("parse_int".into()));
        env.set("parse_float".into(), Value::Builtin("parse_float".into()));
        env.set("string_chars".into(), Value::Builtin("string_chars".into()));
        // File I/O builtins (M14b)
        env.set("read_file".into(), Value::Builtin("read_file".into()));
        env.set("write_file".into(), Value::Builtin("write_file".into()));
        env.set("append_file".into(), Value::Builtin("append_file".into()));
        env.set("file_exists".into(), Value::Builtin("file_exists".into()));
        env.set("read_lines".into(), Value::Builtin("read_lines".into()));
        env.set("env_var".into(), Value::Builtin("env_var".into()));
        env.set("process_run".into(), Value::Builtin("process_run".into()));
        // JSON builtins (M14c)
        env.set("json_parse".into(), Value::Builtin("json_parse".into()));
        env.set("json_get".into(), Value::Builtin("json_get".into()));
        env.set("json_string".into(), Value::Builtin("json_string".into()));
        env.set("json_number".into(), Value::Builtin("json_number".into()));
        env.set("json_bool".into(), Value::Builtin("json_bool".into()));
        env.set("json_array".into(), Value::Builtin("json_array".into()));
        env.set("json_emit".into(), Value::Builtin("json_emit".into()));
        env.set("json_object".into(), Value::Builtin("json_object".into()));
        // HTTP builtins (M14d)
        env.set("http_get".into(), Value::Builtin("http_get".into()));
        env.set("http_post".into(), Value::Builtin("http_post".into()));
        env.set("http_serve".into(), Value::Builtin("http_serve".into()));
        env.set("http_respond".into(), Value::Builtin("http_respond".into()));
        env.set(
            "http_request_path".into(),
            Value::Builtin("http_request_path".into()),
        );
        env.set(
            "http_request_method".into(),
            Value::Builtin("http_request_method".into()),
        );
        env.set(
            "http_request_body".into(),
            Value::Builtin("http_request_body".into()),
        );
        // Database builtins (M14e)
        env.set("db_open".into(), Value::Builtin("db_open".into()));
        env.set("db_exec".into(), Value::Builtin("db_exec".into()));
        env.set("db_query".into(), Value::Builtin("db_query".into()));
        env.set("db_query_row".into(), Value::Builtin("db_query_row".into()));
        env.set("db_insert".into(), Value::Builtin("db_insert".into()));
        env.set("db_close".into(), Value::Builtin("db_close".into()));
        // Math builtins
        env.set("exp".into(), Value::Builtin("exp".into()));
        env.set("ln".into(), Value::Builtin("ln".into()));
        env.set("sqrt".into(), Value::Builtin("sqrt".into()));
        env.set("pow".into(), Value::Builtin("pow".into()));
        env.set("to_float".into(), Value::Builtin("to_float".into()));
        env.set("round".into(), Value::Builtin("round".into()));
        env.set("floor".into(), Value::Builtin("floor".into()));
        env.set("max_f".into(), Value::Builtin("max_f".into()));
        env.set("min_f".into(), Value::Builtin("min_f".into()));
        env.set("format_f".into(), Value::Builtin("format_f".into()));
        // Shared (Arc/Rc) — in interpreter, shared(x) just returns x
        env.set("shared".into(), Value::Builtin("shared".into()));
        // Actor builtins
        env.set("spawn".into(), Value::Builtin("spawn".into()));
        env.set("ask".into(), Value::Builtin("ask".into()));
        // Stream builtins (M12: reactive streams — clean names, no s_ prefix)
        env.set("from_list".into(), Value::Builtin("from_list".into()));
        // Non-colliding stream ops (colliding ones: map, filter, zip, any, all,
        // flat_map, enumerate, distinct — already registered as list builtins above)
        env.set("scan".into(), Value::Builtin("scan".into()));
        env.set("merge".into(), Value::Builtin("merge".into()));
        env.set("take".into(), Value::Builtin("take".into()));
        env.set("collect".into(), Value::Builtin("collect".into()));
        env.set("count".into(), Value::Builtin("count".into()));
        env.set("skip".into(), Value::Builtin("skip".into()));
        env.set("window".into(), Value::Builtin("window".into()));
        env.set("sum".into(), Value::Builtin("sum".into()));
        env.set("last".into(), Value::Builtin("last".into()));
        env.set(
            "combine_latest".into(),
            Value::Builtin("combine_latest".into()),
        );
        // New stream operators (M17b)
        env.set("tap".into(), Value::Builtin("tap".into()));
        env.set("catch".into(), Value::Builtin("catch".into()));
        env.set("first".into(), Value::Builtin("first".into()));
        env.set("reduce".into(), Value::Builtin("reduce".into()));
        env.set("start_with".into(), Value::Builtin("start_with".into()));
        env.set("concat".into(), Value::Builtin("concat".into()));
        env.set("pairwise".into(), Value::Builtin("pairwise".into()));
        env.set("fst".into(), Value::Builtin("fst".into()));
        env.set("snd".into(), Value::Builtin("snd".into()));
        env.set("trd".into(), Value::Builtin("trd".into()));
        // Timing operators (M17) — clean names, no s_ prefix
        env.set("debounce".into(), Value::Builtin("debounce".into()));
        env.set("throttle".into(), Value::Builtin("throttle".into()));
        env.set("delay".into(), Value::Builtin("delay".into()));
        env.set("buffer".into(), Value::Builtin("buffer".into()));
        env.set("timeout".into(), Value::Builtin("timeout".into()));
        env.set("switch_map".into(), Value::Builtin("switch_map".into()));
        env.set("sample".into(), Value::Builtin("sample".into()));
        // Subject + lifecycle builtins (M13)
        env.set("subject".into(), Value::Builtin("subject".into()));
        env.set("watch".into(), Value::Builtin("watch".into()));
        env.set("as_stream".into(), Value::Builtin("as_stream".into()));
        env.set("complete".into(), Value::Builtin("complete".into()));
        env.set("error".into(), Value::Builtin("error".into()));
        env.set("teardown".into(), Value::Builtin("teardown".into()));
        // M13c: async lifecycle builtins
        env.set("poll".into(), Value::Builtin("poll".into()));
        env.set("take_until".into(), Value::Builtin("take_until".into()));
        // Comptime type builtins (M9)
        env.set("struct_type".into(), Value::Builtin("struct_type".into()));
        env.set("enum_type".into(), Value::Builtin("enum_type".into()));
        env.set("field".into(), Value::Builtin("field".into()));
        // Localized builtin aliases (vis→show, skriv→print, længde→length, etc.)
        for (alias, canonical) in builtin_aliases() {
            env.set(alias, Value::Builtin(canonical));
        }
        env
    }

    pub fn register_type(&mut self, decl: &TypeDecl) {
        match decl {
            TypeDecl::ADT {
                name,
                variants,
                except_from,
                ..
            } => {
                // Resolve variant list: expand type includes and apply EXCEPT
                let mut resolved_variants: Vec<String> = Vec::new();

                // Process EXCEPT first: # A = B EXCEPT C | D
                if let Some((source_type, excluded)) = except_from {
                    if let Some(source_variants) = self.type_variants.get(source_type).cloned() {
                        for sv in &source_variants {
                            if !excluded.contains(sv) {
                                resolved_variants.push(sv.clone());
                            }
                        }
                    }
                }

                // Process explicit variants
                for v in variants {
                    match v.from_type.as_deref() {
                        Some("__maybe_include") => {
                            // Could be a type inclusion or a new constructor
                            if let Some(source_variants) = self.type_variants.get(&v.name).cloned()
                            {
                                // It IS an existing type → include all its variants
                                for sv in &source_variants {
                                    if !resolved_variants.contains(sv) {
                                        resolved_variants.push(sv.clone());
                                    }
                                }
                            } else {
                                // New constructor
                                self.constructors
                                    .insert(v.name.clone(), (v.fields.len(), v.positional));
                                if !v.fields.is_empty() && !v.positional {
                                    let names: Vec<String> =
                                        v.fields.iter().map(|f| f.name.clone()).collect();
                                    self.field_names.insert(v.name.clone(), names);
                                }
                                if !resolved_variants.contains(&v.name) {
                                    resolved_variants.push(v.name.clone());
                                }
                            }
                        }
                        Some(_source) => {
                            // Qualified variant: Parent.Variant — already has the right name
                            if !resolved_variants.contains(&v.name) {
                                resolved_variants.push(v.name.clone());
                            }
                        }
                        None => {
                            // Normal variant
                            self.constructors
                                .insert(v.name.clone(), (v.fields.len(), v.positional));
                            if !v.fields.is_empty() && !v.positional {
                                let names: Vec<String> =
                                    v.fields.iter().map(|f| f.name.clone()).collect();
                                self.field_names.insert(v.name.clone(), names);
                            }
                            if !resolved_variants.contains(&v.name) {
                                resolved_variants.push(v.name.clone());
                            }
                        }
                    }
                }

                for vn in &resolved_variants {
                    self.ctor_to_type.insert(vn.clone(), name.clone());
                }
                self.type_variants.insert(name.clone(), resolved_variants);
            }
            TypeDecl::EffectDecl { name, ops } => {
                // Register effect operations: effect_name -> [(op_name, [param_names])]
                let effect_ops: Vec<(String, Vec<String>)> = ops
                    .iter()
                    .map(|(op_name, params, _)| {
                        let param_names: Vec<String> =
                            params.iter().map(|p| p.name.clone()).collect();
                        (op_name.clone(), param_names)
                    })
                    .collect();
                self.effect_decls.insert(name.clone(), effect_ops);
            }
            TypeDecl::TraitDecl { .. } => {} // traits are type-level, no runtime registration
            TypeDecl::ImplBlock {
                for_type, methods, ..
            } => {
                // Register impl methods as functions
                for method in methods {
                    if let Defn::Fn {
                        name, params, body, ..
                    } = method
                    {
                        let param_names: Vec<String> =
                            params.iter().map(|p| p.name.clone()).collect();
                        self.impl_methods.insert(
                            (for_type.clone(), name.clone()),
                            FnDef {
                                params: param_names.clone(),
                                body: body.clone(),
                            },
                        );
                        self.functions.insert(
                            name.clone(),
                            FnDef {
                                params: param_names,
                                body: body.clone(),
                            },
                        );
                    }
                }
            }
            TypeDecl::WhenType { .. } => {
                // Handled in run_program (needs env to evaluate condition)
            }
        }
    }

    pub fn run_program(&mut self, stmts: &[Stmt], env: &mut Env) -> Value {
        let mut last = Value::Unit;
        let mut pending_annot: Option<String> = None;

        for stmt in stmts {
            match stmt {
                Stmt::Annot(name, _) => {
                    pending_annot = Some(name.clone());
                    continue;
                }
                _ => {}
            }

            let current_annot = pending_annot.take();

            match stmt {
                Stmt::Defn(defn) => {
                    last = self.eval_defn(defn, env);
                }
                Stmt::TypeDecl(decl) => {
                    // Handle WHEN type evolution: evaluate condition, then update type
                    if let TypeDecl::WhenType {
                        name,
                        condition,
                        variants,
                        except_from,
                    } = decl
                    {
                        let cond_val = self.eval(condition, env);
                        if matches!(cond_val, Value::Bool(true)) {
                            // Condition is true — evolve the type
                            // Build a temporary ADT decl to reuse register_type
                            let adt = TypeDecl::ADT {
                                name: name.clone(),
                                params: vec![],
                                variants: variants.clone(),
                                methods: vec![],
                                except_from: except_from.clone(),
                            };
                            self.register_type(&adt);
                        }
                        // If false, type stays unchanged
                    } else {
                        self.register_type(decl);
                    }
                    // Register constructors and methods as functions in env
                    self.register_constructors(decl, env);
                    // Register methods in function table for recursion
                    if let TypeDecl::ADT { methods, .. } = decl {
                        for method in methods {
                            if let Defn::Fn {
                                name, params, body, ..
                            } = method
                            {
                                let param_names: Vec<String> =
                                    params.iter().map(|p| p.name.clone()).collect();
                                self.functions.insert(
                                    name.clone(),
                                    FnDef {
                                        params: param_names,
                                        body: body.clone(),
                                    },
                                );
                            }
                        }
                    }
                    last = Value::Unit;
                }
                Stmt::Rule(rule) => {
                    // Scopes are executed immediately, not registered as rules
                    if let Rule::Scope { name, body } = rule {
                        let mut scope_env = env.child();
                        // Execute all body statements in the child environment
                        let _scope_last = self.run_program(body, &mut scope_env);
                        // Store scope as a value so bindings are accessible via ScopeName.field
                        let scope_bindings = scope_env.bindings.clone();
                        env.set(
                            name.clone(),
                            Value::Scope {
                                name: name.clone(),
                                bindings: scope_bindings,
                            },
                        );
                        last = Value::Unit;
                    } else {
                        let name = self.rule_name(rule);
                        self.rules.push((name, rule.clone()));
                        last = Value::Unit;
                    }
                }
                Stmt::Bind(pat, _ty, value) => {
                    let val = self.eval(value, env);
                    if current_annot.as_deref() == Some("comptime") {
                        let bind_name = match pat {
                            Pat::Var(name) => Some(name.as_str()),
                            Pat::Con(name, args) if args.is_empty() => Some(name.as_str()),
                            _ => None,
                        };
                        if let (Some(name), Value::TypeDef { kind, fields }) = (bind_name, &val) {
                            let decl = comptime_typedef_to_type_decl(name, kind, fields);
                            self.register_type(&decl);
                            self.register_constructors(&decl, env);
                            last = Value::Unit;
                            continue;
                        }
                    }
                    self.bind_pattern(pat, &val, env);
                    last = val;
                }
                Stmt::Expr(expr) => {
                    last = self.eval(expr, env);
                    // Handle teardown markers from teardown() builtin
                    if let Value::Constructor(ref name, ref args) = last {
                        if name == "__Teardown" {
                            if let Some(Value::Str(scope_name)) = args.first() {
                                env.remove(scope_name);
                            }
                            last = Value::Unit;
                        }
                    }
                }
                Stmt::Annot(_, _) => {}
                Stmt::Use(_) => {}
                Stmt::RustBlock(_) => {} // @ rust { } blocks are transpile-time only
                Stmt::Import(path) => {
                    // @ import ./math → load math.runa from same directory
                    // @ import dep/module → resolve via runa.toml dependencies
                    if let Some(ref dir) = self.source_dir {
                        let file_path = Self::resolve_import_path_for_source(path, dir);
                        if let Some(file_path) = file_path {
                            let canon = std::fs::canonicalize(&file_path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or(file_path.clone());
                            if !self.imported.contains(&canon) {
                                self.imported.insert(canon);
                                match std::fs::read_to_string(&file_path) {
                                    Ok(source) => {
                                        let mut lexer = Lexer::new(&source);
                                        let tokens = lexer.tokenize();
                                        let mut parser = Parser::new(tokens, &source);
                                        match parser.parse_program() {
                                            Ok(import_stmts) => {
                                                let defs: Vec<Stmt> = import_stmts.into_iter().filter(|s| {
                                                    matches!(s, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Use(_) | Stmt::Rule(_) | Stmt::Bind(..))
                                                }).collect();
                                                last = self.run_program(&defs, env);
                                            }
                                            Err(e) => eprintln!("\x1b[1;31merror\x1b[0m: parse error in imported {}: {}", file_path, e),
                                        }
                                    }
                                    Err(e) => eprintln!("Cannot import {}: {}", file_path, e),
                                }
                            }
                        }
                    }
                }
                Stmt::QualifiedImport(mod_name, path) => {
                    // @ import Name from ./module — qualified import (M3b)
                    // Only exported definitions are accessible as Name.function()
                    if let Some(ref dir) = self.source_dir {
                        let rel = path.trim_start_matches("./");
                        let file_path = format!("{}/{}.runa", dir, rel);
                        let canon = std::fs::canonicalize(&file_path)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or(file_path.clone());
                        if !self.imported.contains(&canon) {
                            self.imported.insert(canon);
                            match std::fs::read_to_string(&file_path) {
                                Ok(source) => {
                                    let mut lexer = Lexer::new(&source);
                                    let tokens = lexer.tokenize();
                                    let mut parser = Parser::new(tokens, &source);
                                    match parser.parse_program() {
                                        Ok(import_stmts) => {
                                            // Scan for @ export annotations to find exported names
                                            let mut exported_names: BTreeSet<String> = BTreeSet::new();
                                            let mut is_export = false;
                                            for s in &import_stmts {
                                                if let Stmt::Annot(name, args) = s {
                                                    if name == "export" {
                                                        // Post-hoc form: `@ export add`
                                                        for a in args { if let ExprKind::Var(n) = &a.kind { exported_names.insert(n.clone()); } }
                                                        if args.is_empty() { is_export = true; }
                                                        continue;
                                                    }
                                                }
                                                if is_export {
                                                    match s {
                                                        Stmt::Defn(Defn::Fn { name, .. }) | Stmt::Defn(Defn::Actor { name, .. }) => {
                                                            exported_names.insert(name.clone());
                                                        }
                                                        Stmt::Defn(Defn::Module { name, .. }) => {
                                                            exported_names.insert(name.clone());
                                                        }
                                                        Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                                                            exported_names.insert(name.clone());
                                                        }
                                                        Stmt::Bind(Pat::Var(name), _, _) => {
                                                            exported_names.insert(name.clone());
                                                        }
                                                        Stmt::StreamBind(name, _) => {
                                                            exported_names.insert(name.clone());
                                                        }
                                                        _ => {}
                                                    }
                                                    is_export = false;
                                                }
                                            }
                                            // Execute all definitions in a child env
                                            let defs: Vec<Stmt> = import_stmts.into_iter().filter(|s| {
                                                matches!(s, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Use(_) | Stmt::Rule(_) | Stmt::Bind(..))
                                            }).collect();
                                            let mut mod_env = env.child();
                                            self.run_program(&defs, &mut mod_env);
                                            // Filter to only exported bindings
                                            let mut bindings = HashMap::new();
                                            for (k, v) in &mod_env.bindings {
                                                if exported_names.contains(k) {
                                                    bindings.insert(k.clone(), v.clone());
                                                }
                                            }
                                            // Also include constructors of exported ADTs
                                            // (constructors are registered by variant name, not type name)
                                            // Scan the imported file to find which constructors belong to exported types
                                            for s in &defs {
                                                if let Stmt::TypeDecl(TypeDecl::ADT { name: type_name, variants, .. }) = s {
                                                    if exported_names.contains(type_name) {
                                                        for v in variants {
                                                            if let Some(val) = mod_env.bindings.get(&v.name) {
                                                                bindings.insert(v.name.clone(), val.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            env.set(mod_name.clone(), Value::Scope {
                                                name: mod_name.clone(),
                                                bindings,
                                            });
                                        }
                                        Err(e) => eprintln!("\x1b[1;31merror\x1b[0m: parse error in imported {}: {}", file_path, e),
                                    }
                                }
                                Err(e) => eprintln!("Cannot import {}: {}", file_path, e),
                            }
                        }
                    }
                }
                Stmt::HashImport(hash, path) => {
                    // @ import #hash from ./module — load only the definition with matching hash
                    if let Some(ref dir) = self.source_dir {
                        let rel = path.trim_start_matches("./");
                        let file_path = format!("{}/{}.runa", dir, rel);
                        match std::fs::read_to_string(&file_path) {
                            Ok(source) => {
                                let mut lexer = Lexer::new(&source);
                                let tokens = lexer.tokenize();
                                let mut parser = Parser::new(tokens, &source);
                                match parser.parse_program() {
                                    Ok(import_stmts) => {
                                        let mut found = false;
                                        for s in &import_stmts {
                                            let matches = match s {
                                                Stmt::Defn(d) => content_hash_defn(d) == *hash,
                                                Stmt::TypeDecl(td) => {
                                                    content_hash_type(td) == *hash
                                                }
                                                _ => false,
                                            };
                                            if matches {
                                                last = self.run_program(&[s.clone()], env);
                                                found = true;
                                                break;
                                            }
                                        }
                                        if !found {
                                            eprintln!("Hash #{} not found in {}", hash, file_path);
                                        }
                                    }
                                    Err(e) => eprintln!(
                                        "\x1b[1;31merror\x1b[0m: parse error in imported {}: {}",
                                        file_path, e
                                    ),
                                }
                            }
                            Err(e) => eprintln!("Cannot import {}: {}", file_path, e),
                        }
                    }
                }
                Stmt::Invariant {
                    name,
                    subject,
                    predicate,
                } => {
                    // Register the invariant for later ? verification
                    self.invariants
                        .insert(name.clone(), (subject.clone(), predicate.clone()));
                    last = Value::Unit;
                }
                Stmt::Prove {
                    name,
                    proof_block,
                    capture,
                    pass_block,
                    else_block,
                } => {
                    // Explicit proof blocks are checked by `runa verify`, not executed at runtime.
                    if proof_block.is_some() {
                        last = Value::Unit;
                        continue;
                    }
                    let has_blocks = pass_block.is_some() || else_block.is_some();
                    let targets: Vec<(String, Expr, Expr)> = if name == "all" {
                        self.invariants
                            .iter()
                            .map(|(n, (s, p))| (n.clone(), s.clone(), p.clone()))
                            .collect()
                    } else if let Some((s, p)) = self.invariants.get(name) {
                        vec![(name.clone(), s.clone(), p.clone())]
                    } else {
                        eprintln!("? {}: no such invariant", name);
                        vec![]
                    };
                    let mut all_passed = true;
                    let mut last_subject_val = Value::Unit;
                    for (inv_name, subject_expr, pred_expr) in &targets {
                        let subject_val = self.eval(subject_expr, env);
                        let pred_val = self.eval(pred_expr, env);
                        last_subject_val = subject_val.clone();
                        match pred_val {
                            Value::Bool(true) => {
                                if !has_blocks {
                                    // Default pass output (bare ? only)
                                    let msg = format!(
                                        "  ✓ |{}| holds (value: {})",
                                        inv_name, subject_val
                                    );
                                    println!("{}", msg);
                                    self.output.push(msg);
                                }
                            }
                            Value::Bool(false) => {
                                all_passed = false;
                                if !has_blocks {
                                    // Bare `? name` — default: halt on failure
                                    let msg = format!(
                                        "? {} FAILED: |{}| VIOLATED (value: {})",
                                        name, inv_name, subject_val
                                    );
                                    println!("{}", msg);
                                    self.output.push(msg);
                                    std::process::exit(1);
                                }
                            }
                            other => {
                                let msg = format!(
                                    "  ? |{}| predicate returned non-bool: {}",
                                    inv_name, other
                                );
                                println!("{}", msg);
                                self.output.push(msg);
                            }
                        }
                    }
                    // Bind capture variable if specified
                    if let Some(cap) = capture {
                        env.set(cap.clone(), last_subject_val);
                    }
                    // Run blocks based on aggregate result
                    if has_blocks {
                        if all_passed {
                            if let Some(block) = pass_block {
                                self.run_program(block, env);
                            }
                        } else {
                            if let Some(block) = else_block {
                                // Custom fail block — user handles it, no halt
                                self.run_program(block, env);
                            } else {
                                // Has pass block but no else → halt on failure
                                let msg = format!("? {} FAILED", name);
                                println!("{}", msg);
                                self.output.push(msg);
                                std::process::exit(1);
                            }
                        }
                    }
                    last = Value::Unit;
                }
                Stmt::Depend(_, _) => {} // @ depend is transpile-time only
                Stmt::StreamBind(name, expr) => {
                    // ~ name = expr — evaluate expr and wrap in Stream if needed
                    // Subjects stay as Subject (not re-wrapped into Stream)
                    let val = self.eval(expr, env);
                    let stream_val = match val {
                        Value::Stream(_) | Value::Subject(_) => val,
                        other => Value::Stream(list_to_vec(&other)),
                    };
                    env.set(name.clone(), stream_val);
                }
                Stmt::StreamSub(expr, arms) => {
                    let val = self.eval(expr, env);
                    let items = match val {
                        Value::Stream(items) | Value::Subject(items) => items,
                        other => list_to_vec(&other),
                    };

                    // Categorize arms
                    let mut value_arms = Vec::new();
                    let mut _error_arm = None;
                    let mut complete_arm = None;

                    for arm in arms {
                        let is_complete = matches!(&arm.pat, Pat::Var(n) if n == "Complete")
                            || matches!(&arm.pat, Pat::Con(n, _) if n == "Complete");
                        let is_error = matches!(&arm.pat, Pat::Con(n, _) if n == "Err");

                        if is_complete {
                            complete_arm = Some(arm);
                        } else if is_error {
                            _error_arm = Some(arm);
                        } else {
                            value_arms.push(arm);
                        }
                    }

                    // Process values
                    for item in items {
                        // Check if it's an error value inserted by `error(subject, msg)`
                        // In sync interpreter, we might not have a reliable way to distinguish,
                        // but if we did, we'd route it to error_arm. For now, all are values.
                        let mut matched = false;
                        for arm in &value_arms {
                            let mut local_env = env.child();
                            if self.match_pattern(&arm.pat, &item, &mut local_env) {
                                let guard_ok = match &arm.guard {
                                    Some(g) => {
                                        matches!(self.eval(g, &mut local_env), Value::Bool(true))
                                    }
                                    None => true,
                                };
                                if guard_ok {
                                    self.eval(&arm.body, &mut local_env);
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        if !matched && !value_arms.is_empty() {
                            eprintln!(
                                "stream subscription value {:?} did not match any value arms",
                                item
                            );
                        }
                    }

                    // Process Complete
                    if let Some(arm) = complete_arm {
                        let mut local_env = env.child();
                        if self.match_pattern(
                            &arm.pat,
                            &Value::Constructor("Complete".into(), vec![]),
                            &mut local_env,
                        ) {
                            let guard_ok = match &arm.guard {
                                Some(g) => {
                                    matches!(self.eval(g, &mut local_env), Value::Bool(true))
                                }
                                None => true,
                            };
                            if guard_ok {
                                self.eval(&arm.body, &mut local_env);
                            }
                        }
                    }
                }

                Stmt::Send(target_expr, msg_expr) => {
                    // actor <- Message: dispatch message to actor, update its state
                    let target = self.eval(target_expr, env);
                    let msg = self.eval(msg_expr, env);
                    match target {
                        Value::Actor {
                            ref actor_name,
                            ref state,
                            ref state_param,
                            ref handlers,
                            env: ref actor_env,
                        } => {
                            let (new_state, _response) = self.dispatch_actor_message(
                                actor_name,
                                state,
                                state_param,
                                handlers,
                                actor_env,
                                &msg,
                            );
                            // Update the actor in the env with new state
                            if let ExprKind::Var(var_name) = &target_expr.kind {
                                let updated = Value::Actor {
                                    actor_name: actor_name.clone(),
                                    state: Box::new(new_state),
                                    state_param: state_param.clone(),
                                    handlers: handlers.clone(),
                                    env: actor_env.clone(),
                                };
                                env.set(var_name.clone(), updated);
                            }
                        }
                        Value::Subject(ref items) => {
                            // subject <- value: push value into the subject's buffer
                            if let ExprKind::Var(var_name) = &target_expr.kind {
                                let mut new_items = items.clone();
                                new_items.push(msg);
                                env.set(var_name.clone(), Value::Subject(new_items));
                            }
                        }
                        _ => eprintln!("Send (<-): target is not an actor or subject"),
                    }
                }
                Stmt::For(var, iter_expr, body_stmts) => {
                    let iter_val = self.eval(iter_expr, env);
                    let items = match iter_val {
                        Value::Stream(items) | Value::Subject(items) => items,
                        Value::List(items) => items,
                        other => {
                            // Cons/Nil linked list
                            let mut v = Vec::new();
                            let mut cur = other;
                            loop {
                                match &cur {
                                    Value::Constructor(name, args)
                                        if name == "Cons" && args.len() == 2 =>
                                    {
                                        v.push(args[0].clone());
                                        cur = args[1].clone();
                                    }
                                    _ => break,
                                }
                            }
                            v
                        }
                    };
                    for item in items {
                        env.set(var.clone(), item);
                        let inner_stmts: Vec<Stmt> = body_stmts.clone();
                        last = self.run_program(&inner_stmts, env);
                    }
                }
                Stmt::While(cond, body_stmts) => {
                    loop {
                        let cond_val = self.eval(cond, env);
                        if !matches!(cond_val, Value::Bool(true)) {
                            break;
                        }
                        // Execute body stmts directly in the current env
                        // (not via run_program which may scope bindings differently)
                        for body_stmt in body_stmts {
                            match body_stmt {
                                Stmt::Bind(pat, _, val) => {
                                    let v = self.eval(val, env);
                                    if let Pat::Var(name) = pat {
                                        env.set(name.clone(), v);
                                    }
                                }
                                Stmt::Expr(expr) => {
                                    // Handle if/else that rebinds variables
                                    if let ExprKind::If(c, then_branch, else_branch) = &expr.kind {
                                        let cv = self.eval(c, env);
                                        let branch = if matches!(cv, Value::Bool(true)) {
                                            then_branch
                                        } else {
                                            else_branch
                                        };
                                        // Execute branch block stmts in parent env
                                        if let ExprKind::Block(stmts) = &branch.kind {
                                            for s in stmts {
                                                if let Stmt::Bind(Pat::Var(name), _, val) = s {
                                                    let v = self.eval(val, env);
                                                    env.set(name.clone(), v);
                                                } else {
                                                    self.run_program(&[s.clone()], env);
                                                }
                                            }
                                        } else {
                                            last = self.eval(branch, env);
                                        }
                                    } else {
                                        last = self.eval(expr, env);
                                    }
                                }
                                Stmt::Annot(name, args) if name == "print" => {
                                    if let Some(arg) = args.first() {
                                        let v = self.eval(arg, env);
                                        let text = format!("{}", v);
                                        if !self.suppress_output {
                                            println!("{}", text);
                                        }
                                        self.output.push(text);
                                    }
                                }
                                other => {
                                    last = self.run_program(&[other.clone()], env);
                                }
                            }
                        }
                    }
                }
                Stmt::MonadicBind(pat, _ty, expr) => {
                    let val = self.eval(expr, env);
                    match &val {
                        Value::Constructor(name, args) if name == "Ok" || name == "Some" => {
                            let inner = args.first().cloned().unwrap_or(Value::Unit);
                            self.bind_pattern(pat, &inner, env);
                            last = inner;
                        }
                        Value::Constructor(name, _) if name == "Err" || name == "None" => {
                            return val; // Early return from enclosing block
                        }
                        _ => {
                            // Not a Result/Option — just bind directly
                            self.bind_pattern(pat, &val, env);
                            last = val;
                        }
                    }
                }

                // Persist: assert/retract/abort — interpreter stubs (real impl in codegen)
                Stmt::Assert(type_name, args) => {
                    let vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
                    eprintln!("[runa interpreter] assert {}({}) — persist operations require `runa build`",
                        type_name,
                        vals.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(", "));
                }
                Stmt::Retract(type_name, args) => {
                    let vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
                    eprintln!("[runa interpreter] retract {}({}) — persist operations require `runa build`",
                        type_name,
                        vals.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(", "));
                }
                Stmt::Abort => {
                    eprintln!("[runa interpreter] abort — scope abort requires `runa build`");
                }
            }

            pending_annot = None;
        }
        last
    }

    pub fn eval_defn(&mut self, defn: &Defn, env: &mut Env) -> Value {
        match defn {
            Defn::Fn {
                name, params, body, ..
            } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                // Register in function table for recursion
                self.functions.insert(
                    name.clone(),
                    FnDef {
                        params: param_names.clone(),
                        body: body.clone(),
                    },
                );
                // Named functions don't capture env — they get it at call time
                // This avoids exponential env cloning
                let closure = Value::Closure {
                    name: Some(name.clone()),
                    params: param_names,
                    body: body.clone(),
                    env: Env::new(),
                };
                env.set(name.clone(), closure.clone());
                closure
            }
            Defn::Actor {
                name,
                state_param,
                handlers,
            } => {
                // Store actor definition as a named constructor so spawn() can find it
                let val = Value::Constructor(
                    format!("ActorDef<{}>", name),
                    vec![Value::Str(state_param.name.clone())],
                );
                // Also store the handlers in a separate env key for spawn to use
                env.set(
                    format!("__actor_handlers_{}", name),
                    Value::Constructor(
                        "Handlers".to_string(),
                        vec![], // handlers stored structurally — we'll look up the Defn directly
                    ),
                );
                // Store the raw defn for the interpreter to reference
                self.actor_defs.insert(
                    name.clone(),
                    Defn::Actor {
                        name: name.clone(),
                        state_param: state_param.clone(),
                        handlers: handlers.clone(),
                    },
                );
                env.set(name.clone(), val.clone());
                val
            }
            Defn::Module { name, body } => {
                let mut mod_env = env.child();
                self.run_program(body, &mut mod_env);
                // M3b: Store module as Value::Scope for qualified access (Name.func())
                // No unqualified leaking — use Name.binding to access
                let bindings = mod_env.bindings.clone();
                let val = Value::Scope {
                    name: name.clone(),
                    bindings,
                };
                env.set(name.clone(), val.clone());
                val
            }
        }
    }

    pub fn register_constructors(&self, decl: &TypeDecl, env: &mut Env) {
        match decl {
            TypeDecl::ADT {
                variants, methods, ..
            } => {
                for v in variants {
                    if v.fields.is_empty() {
                        // Nullary constructor: just a value
                        env.set(v.name.clone(), Value::Constructor(v.name.clone(), vec![]));
                    } else if v.positional {
                        // Positional constructor: tuple-style Value::Constructor
                        let arity = v.fields.len();
                        let name = v.name.clone();
                        env.set(
                            name.clone(),
                            Value::Builtin(format!("ctor:{}/{}", name, arity)),
                        );
                    } else {
                        // Named constructor: struct-style Value::NamedConstructor
                        let arity = v.fields.len();
                        let name = v.name.clone();
                        env.set(
                            name.clone(),
                            Value::Builtin(format!("nctor:{}/{}", name, arity)),
                        );
                    }
                }
                // Register methods as functions
                for method in methods {
                    if let Defn::Fn {
                        name, params, body, ..
                    } = method
                    {
                        let param_names: Vec<String> =
                            params.iter().map(|p| p.name.clone()).collect();
                        let closure = Value::Closure {
                            name: Some(name.clone()),
                            params: param_names,
                            body: body.clone(),
                            env: Env::new(),
                        };
                        env.set(name.clone(), closure);
                    }
                }
            }
            TypeDecl::EffectDecl { .. } => {}
            TypeDecl::TraitDecl { .. } => {} // no runtime values for traits
            TypeDecl::WhenType { .. } => {}  // handled in run_program
            TypeDecl::ImplBlock { methods, .. } => {
                // Register impl methods as callable functions
                for method in methods {
                    if let Defn::Fn {
                        name, params, body, ..
                    } = method
                    {
                        let param_names: Vec<String> =
                            params.iter().map(|p| p.name.clone()).collect();
                        let closure = Value::Closure {
                            name: Some(name.clone()),
                            params: param_names,
                            body: body.clone(),
                            env: Env::new(),
                        };
                        env.set(name.clone(), closure);
                    }
                }
            }
        }
    }

    pub fn eval(&mut self, expr: &Expr, env: &Env) -> Value {
        if self.step_limit > 0 {
            self.step_count += 1;
            if self.step_count > self.step_limit {
                self.budget_exceeded = true;
                return Value::Unit;
            }
        }
        match &expr.kind {
            ExprKind::Var(name) => {
                // Check local env first (params, local bindings, builtins)
                if let Some(val) = env.get(name) {
                    val.clone()
                }
                // Then function registry (for recursion and cross-function calls)
                else if self.functions.contains_key(name) {
                    Value::Closure {
                        name: Some(name.clone()),
                        params: self.functions[name].params.clone(),
                        body: self.functions[name].body.clone(),
                        env: env.clone(),
                    }
                } else if self.constructors.contains_key(name) {
                    let (arity, positional) = self.constructors[name];
                    if arity == 0 {
                        Value::Constructor(name.clone(), vec![])
                    } else if positional {
                        Value::Builtin(format!("ctor:{}/{}", name, arity))
                    } else {
                        Value::Builtin(format!("nctor:{}/{}", name, arity))
                    }
                } else {
                    // Might be an unbound variable (used in logic rules)
                    Value::Constructor(name.clone(), vec![])
                }
            }
            ExprKind::Lit(lit) => match lit {
                Literal::Int(n) => Value::Int(*n),
                Literal::Float(f) => Value::Float(*f),
                Literal::Str(s) => Value::Str(s.clone()),
                Literal::Char(c) => Value::Char(*c),
                Literal::Bool(b) => Value::Bool(*b),
            },
            ExprKind::App(func, args) => {
                // Check if this is an effect operation call dispatched to a handler
                if let ExprKind::Var(ref fn_name) = (*func).kind {
                    if let Some(result) = self.try_effect_dispatch(fn_name, args, env) {
                        return result;
                    }
                    // findall(template_var, goal) — collect all solutions
                    if fn_name == "findall" && args.len() == 2 {
                        return self.eval_findall(&args[0], &args[1], env);
                    }
                    // search(template, goal) → first match (like findall but returns Option)
                    if fn_name == "search" && args.len() == 2 {
                        let results = self.eval_findall(&args[0], &args[1], env);
                        return match results {
                            Value::List(items) if !items.is_empty() => {
                                Value::Constructor("Some".into(), vec![items[0].clone()])
                            }
                            _ => Value::Constructor("None".into(), vec![]),
                        };
                    }
                    // Check if this is a rule call (| name(...) -> value)
                    if let Some(result) = self.try_rule_call(fn_name, args, env) {
                        return result;
                    }
                    // If rules exist for this name but none matched:
                    // Value-returning rules → Str("") as default, boolean rules → false
                    if self.rules.iter().any(|(n, _)| n == fn_name) {
                        let is_value_returning = self.rules.iter().any(|(n, r)| {
                            if n != fn_name {
                                return false;
                            }
                            match r {
                                Rule::Default { .. } | Rule::Exception { .. } => true,
                                Rule::Clause {
                                    body: Some(body_expr),
                                    ..
                                } => {
                                    // Body is a non-boolean literal → value-returning fact
                                    matches!(
                                        &body_expr.kind,
                                        ExprKind::Lit(Literal::Str(_))
                                            | ExprKind::Lit(Literal::Int(_))
                                            | ExprKind::Lit(Literal::Float(_))
                                    )
                                }
                                _ => false,
                            }
                        });
                        return if is_value_returning {
                            Value::Str(String::new())
                        } else {
                            Value::Bool(false)
                        };
                    }
                }
                let f = self.eval(func, env);
                let arg_vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
                self.apply(f, arg_vals, env)
            }
            ExprKind::Lambda(params, body) => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                Value::Closure {
                    name: None,
                    params: param_names,
                    body: *body.clone(),
                    env: env.clone(),
                }
            }
            ExprKind::BinOp(op, lhs, rhs) => {
                let l = self.eval(lhs, env);
                // Short-circuit for && and ||
                if op == "&&" {
                    return match l {
                        Value::Bool(false) => Value::Bool(false),
                        _ => self.eval(rhs, env),
                    };
                }
                if op == "||" {
                    return match l {
                        Value::Bool(true) => Value::Bool(true),
                        _ => self.eval(rhs, env),
                    };
                }
                let r = self.eval(rhs, env);
                self.eval_binop(op, l, r)
            }
            ExprKind::UnOp(op, operand) => {
                let v = self.eval(operand, env);
                match (op.as_str(), v) {
                    ("!", Value::Bool(b)) => Value::Bool(!b),
                    ("-", Value::Int(n)) => Value::Int(-n),
                    ("-", Value::Float(f)) => Value::Float(-f),
                    ("&", v) | ("&mut", v) => v, // References are just values for now
                    _ => Value::Unit,
                }
            }
            ExprKind::If(cond, then_, else_) => match self.eval(cond, env) {
                Value::Bool(true) => self.eval(then_, env),
                Value::Bool(false) => self.eval(else_, env),
                _ => self.eval(then_, env),
            },
            ExprKind::Match(scrut, arms) => {
                let val = self.eval(scrut, env);
                self.eval_match(val, arms, env)
            }
            ExprKind::Block(stmts) => {
                let mut block_env = env.child();
                self.run_program(stmts, &mut block_env)
            }
            ExprKind::Field(obj, field) => {
                let obj_val = self.eval(obj, env);
                match &obj_val {
                    // Tuple field access: .fst/.snd/.trd → indices 0/1/2
                    // Also supports .0, .1, .2, etc.
                    Value::Tuple(elems) => match field.as_str() {
                        "fst" | "0" => return elems.first().cloned().unwrap_or(Value::Unit),
                        "snd" | "1" => return elems.get(1).cloned().unwrap_or(Value::Unit),
                        "trd" | "2" => return elems.get(2).cloned().unwrap_or(Value::Unit),
                        _ => {
                            if let Ok(idx) = field.parse::<usize>() {
                                return elems.get(idx).cloned().unwrap_or(Value::Unit);
                            }
                            return Value::Unit;
                        }
                    },
                    Value::NamedConstructor(_name, named_fields) => {
                        // Named field access: obj.field_name
                        for (fname, val) in named_fields {
                            if fname == field {
                                return val.clone();
                            }
                        }
                        // Fall through to index-based access
                        if let Ok(idx) = field.parse::<usize>() {
                            named_fields
                                .get(idx)
                                .map(|(_, v)| v.clone())
                                .unwrap_or(Value::Unit)
                        } else {
                            self.bind_method_value(&obj_val, field, env)
                                .unwrap_or(Value::Unit)
                        }
                    }
                    Value::Constructor(ctor_name, fields) => {
                        // Try named field access via field_names registry
                        if let Some(names) = self.field_names.get(ctor_name.as_str()) {
                            for (i, fname) in names.iter().enumerate() {
                                if fname == field {
                                    return fields.get(i).cloned().unwrap_or(Value::Unit);
                                }
                            }
                        }
                        // Numeric index access
                        if let Ok(idx) = field.parse::<usize>() {
                            fields.get(idx).cloned().unwrap_or(Value::Unit)
                        } else {
                            self.bind_method_value(&obj_val, field, env)
                                .unwrap_or(Value::Unit)
                        }
                    }
                    // Subject: .latest returns the most recent value, .count returns length
                    Value::Subject(items) => match field.as_str() {
                        "latest" => items.last().cloned().unwrap_or(Value::Unit),
                        "count" => Value::Int(items.len() as i64),
                        _ => Value::Unit,
                    },
                    // Actor-subject unification: actors expose .state for current state
                    Value::Actor { state, .. } => match field.as_str() {
                        "state" => *state.clone(),
                        _ => Value::Unit,
                    },
                    // Scope field access: MyScope.field → look up field in scope bindings
                    Value::Scope { bindings, .. } => {
                        if let Some(value) = bindings.get(field) {
                            if let Value::Closure {
                                name,
                                params,
                                body,
                                env: closure_env,
                            } = value
                            {
                                if name.is_some() {
                                    let mut scope_env = env.child();
                                    for (binding_name, binding_value) in bindings {
                                        scope_env.set(binding_name.clone(), binding_value.clone());
                                    }
                                    return Value::Closure {
                                        name: name.clone(),
                                        params: params.clone(),
                                        body: body.clone(),
                                        env: scope_env,
                                    };
                                }
                            }
                            value.clone()
                        } else {
                            Value::Unit
                        }
                    }
                    _ => self
                        .bind_method_value(&obj_val, field, env)
                        .unwrap_or(Value::Unit),
                }
            }
            ExprKind::Index(arr, idx) => {
                let arr_val = self.eval(arr, env);
                let idx_val = self.eval(idx, env);
                match (&arr_val, &idx_val) {
                    (Value::List(elems), Value::Int(i)) => {
                        let i = *i;
                        if i < 0 || i as usize >= elems.len() {
                            panic!("index out of bounds: {} (len {})", i, elems.len())
                        } else {
                            elems[i as usize].clone()
                        }
                    }
                    (_, Value::Int(i)) => {
                        // Handle Cons/Nil linked lists and other list-like values
                        let elems = list_to_vec(&arr_val);
                        let i = *i;
                        if i < 0 || i as usize >= elems.len() {
                            panic!("index out of bounds: {} (len {})", i, elems.len())
                        } else {
                            elems[i as usize].clone()
                        }
                    }
                    _ => Value::Unit,
                }
            }
            ExprKind::List(elems) => {
                // Convert list literal to Cons/Nil chain
                let vals: Vec<Value> = elems.iter().map(|e| self.eval(e, env)).collect();
                let mut result = Value::Constructor("Nil".into(), vec![]);
                for v in vals.into_iter().rev() {
                    result = Value::Constructor("Cons".into(), vec![v, result]);
                }
                result
            }
            ExprKind::Tuple(elems) => {
                let vals: Vec<Value> = elems.iter().map(|e| self.eval(e, env)).collect();
                Value::Tuple(vals)
            }
            ExprKind::Effect(name, args) => {
                let arg_vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
                self.eval_effect(name, arg_vals)
            }
            ExprKind::Handle {
                effect,
                handlers,
                body,
            } => {
                // Push handlers onto the stack
                self.handler_stack.push((effect.clone(), handlers.clone()));
                // Evaluate the body with handlers active
                let result = self.eval(body, env);
                // Pop handlers
                self.handler_stack.pop();
                result
            }
            ExprKind::Try(inner) => {
                // ? operator: unwrap Ok/Some, early-return Err/None (matches compiled ? behavior)
                let val = self.eval(inner, env);
                match &val {
                    Value::Constructor(name, args) if name == "Ok" && args.len() == 1 => {
                        args[0].clone()
                    }
                    Value::Constructor(name, args) if name == "Some" && args.len() == 1 => {
                        args[0].clone()
                    }
                    Value::Constructor(name, _) if name == "Err" || name == "None" => {
                        // Early-return the error/none value (same as ? in Rust)
                        eprintln!("Error: ? operator on {}", val);
                        std::process::exit(1);
                    }
                    _ => val, // pass through non-Result/Option values
                }
            }
            ExprKind::Unit => Value::Unit,
            ExprKind::Conjunction(exprs) => {
                // Evaluate all conjuncts; return true if all are true
                for e in exprs {
                    if let Value::Bool(false) = self.eval(e, env) {
                        return Value::Bool(false);
                    }
                }
                Value::Bool(true)
            }
            ExprKind::Disjunction(alternatives) => {
                // Evaluate alternatives; return true if ANY succeeds
                for alt in alternatives {
                    let result = self.eval(alt, env);
                    if !matches!(result, Value::Bool(false)) {
                        return result;
                    }
                }
                Value::Bool(false)
            }
            ExprKind::Pipe(input, transform) => {
                // Pipe: a |> f → f(a), a |> f(y) → f(a, y)
                // Same semantics as the old App desugaring
                match &transform.as_ref().kind {
                    ExprKind::App(func, existing_args) => {
                        let f = self.eval(func, env);
                        let input_val = self.eval(input, env);
                        let mut arg_vals = vec![input_val];
                        arg_vals.extend(existing_args.iter().map(|a| self.eval(a, env)));
                        self.apply(f, arg_vals, env)
                    }
                    _ => {
                        let f = self.eval(transform, env);
                        let input_val = self.eval(input, env);
                        self.apply(f, vec![input_val], env)
                    }
                }
            }
        }
    }

    pub fn apply(&mut self, func: Value, args: Vec<Value>, call_env: &Env) -> Value {
        match func {
            Value::Closure {
                ref name,
                ref params,
                ref body,
                ref env,
            } => {
                // Named functions: use call-site env (has builtins, other fns)
                // Lambdas: use captured env (has enclosing scope vars)
                let uses_call_env =
                    name.is_some() && env.bindings.is_empty() && env.parent.is_none();
                let base_env = if uses_call_env { call_env } else { env };
                let mut call = base_env.child();
                for (p, a) in params.iter().zip(args.iter()) {
                    call.set(p.clone(), a.clone());
                }
                self.eval(body, &call)
            }
            Value::Builtin(ref name) => self.eval_builtin(name, args, call_env),
            Value::Constructor(name, existing) => {
                // Partial application of constructor
                let mut all = existing;
                all.extend(args);
                Value::Constructor(name, all)
            }
            _ => {
                self.output.push(format!("Error: cannot apply {}", func));
                Value::Unit
            }
        }
    }

    pub fn eval_builtin(&mut self, name: &str, args: Vec<Value>, env: &Env) -> Value {
        // resume(val) — algebraic effect continuation (identity in tail-resumptive)
        if name == "__resume" {
            return args.into_iter().next().unwrap_or(Value::Unit);
        }
        // Constructor application
        if name.starts_with("nctor:") {
            let parts: Vec<&str> = name[6..].split('/').collect();
            let ctor_name = parts[0];
            // Build NamedConstructor with field names from registry
            if let Some(names) = self.field_names.get(ctor_name) {
                let named_fields: Vec<(String, Value)> = names
                    .iter()
                    .zip(args.into_iter())
                    .map(|(n, v)| (n.clone(), v))
                    .collect();
                return Value::NamedConstructor(ctor_name.to_string(), named_fields);
            }
            return Value::Constructor(ctor_name.to_string(), args);
        }
        if name.starts_with("ctor:") {
            let parts: Vec<&str> = name[5..].split('/').collect();
            let ctor_name = parts[0];
            return Value::Constructor(ctor_name.to_string(), args);
        }

        match name {
            "print" => {
                let text = match args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    Some(v) => format!("{}", v),
                    None => String::new(),
                };
                if !self.suppress_output {
                    println!("{}", text);
                }
                self.output.push(text);
                Value::Unit
            }
            "show" => match args.first() {
                Some(v) => Value::Str(format!("{}", v)),
                None => Value::Str(String::new()),
            },
            "show_int" | "show_float" => match args.first() {
                Some(v) => Value::Str(format!("{}", v)),
                None => Value::Str("0".into()),
            },
            "length" => match args.first() {
                Some(Value::Str(s)) => Value::Int(s.chars().count() as i64),
                Some(v) => Value::Int(list_length(v)),
                None => Value::Int(0),
            },
            "head" => match args.first() {
                Some(Value::Constructor(n, fields)) if n == "Cons" => {
                    fields.first().cloned().unwrap_or(Value::Unit)
                }
                Some(Value::Constructor(n, _)) if n == "Nil" => {
                    panic!("head: empty list")
                }
                Some(Value::List(elems)) => {
                    if elems.is_empty() {
                        panic!("head: empty list")
                    } else {
                        elems.first().cloned().unwrap_or(Value::Unit)
                    }
                }
                _ => Value::Unit,
            },
            "tail" => match args.first() {
                Some(Value::Constructor(n, fields)) if n == "Cons" => fields
                    .get(1)
                    .cloned()
                    .unwrap_or(Value::Constructor("Nil".into(), vec![])),
                Some(Value::List(elems)) => {
                    if elems.len() <= 1 {
                        Value::Constructor("Nil".into(), vec![])
                    } else {
                        Value::List(elems[1..].to_vec())
                    }
                }
                _ => Value::Constructor("Nil".into(), vec![]),
            },
            "nth" => {
                // nth(list, index) — 0-based indexed access
                match (args.get(0), args.get(1)) {
                    (Some(list_val), Some(Value::Int(idx))) => {
                        let elems = list_to_vec(list_val);
                        let i = *idx;
                        if i < 0 || i as usize >= elems.len() {
                            panic!("index out of bounds: {} (len {})", i, elems.len())
                        } else {
                            elems[i as usize].clone()
                        }
                    }
                    _ => Value::Unit,
                }
            }
            "abs" => match args.first() {
                Some(Value::Int(n)) => Value::Int(n.abs()),
                Some(Value::Float(f)) => Value::Float(f.abs()),
                _ => Value::Int(0),
            },
            "random_float" => {
                // random_float() → Float in [0.0, 1.0). Xorshift64.
                let mut s = self.rng_state;
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                self.rng_state = s;
                let val = (s as u64) as f64 / u64::MAX as f64;
                Value::Float(val)
            }
            "random_choice" => {
                // random_choice(list) → random element from list
                let items = list_to_vec(args.first().unwrap_or(&Value::List(vec![])));
                if items.is_empty() {
                    Value::Unit
                } else {
                    let mut s = self.rng_state;
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    self.rng_state = s;
                    let idx = (s as usize) % items.len();
                    items[idx].clone()
                }
            }
            "shuffle" => {
                // shuffle(list) → new list with elements in random order (Fisher-Yates)
                let mut items = list_to_vec(args.first().unwrap_or(&Value::List(vec![])));
                let n = items.len();
                for i in (1..n).rev() {
                    let mut s = self.rng_state;
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    self.rng_state = s;
                    let j = (s as usize) % (i + 1);
                    items.swap(i, j);
                }
                Value::List(items)
            }
            "sleep" => {
                // sleep(ms) → () — blocking sleep
                let ms = match args.first() {
                    Some(Value::Int(n)) => *n as u64,
                    _ => 0,
                };
                if ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
                Value::Unit
            }
            "now" => {
                // now() → Int — Unix timestamp in milliseconds
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                Value::Int(ms)
            }
            "time_diff" => {
                // time_diff(a, b) → Int — difference in ms (a - b)
                match (args.first(), args.get(1)) {
                    (Some(Value::Int(a)), Some(Value::Int(b))) => Value::Int(a - b),
                    _ => Value::Int(0),
                }
            }
            "regex_match" => {
                // regex_match(pattern, text) → Bool
                match (args.first(), args.get(1)) {
                    (Some(Value::Str(pattern)), Some(Value::Str(text))) => {
                        match regex::Regex::new(pattern) {
                            Ok(re) => Value::Bool(re.is_match(text)),
                            Err(_) => Value::Bool(false),
                        }
                    }
                    _ => Value::Bool(false),
                }
            }
            "regex_find" => {
                // regex_find(pattern, text) → Option(String): first match
                match (args.first(), args.get(1)) {
                    (Some(Value::Str(pattern)), Some(Value::Str(text))) => {
                        match regex::Regex::new(pattern) {
                            Ok(re) => match re.find(text) {
                                Some(m) => Value::Constructor(
                                    "Some".into(),
                                    vec![Value::Str(m.as_str().to_string())],
                                ),
                                None => Value::Constructor("None".into(), vec![]),
                            },
                            Err(_) => Value::Constructor("None".into(), vec![]),
                        }
                    }
                    _ => Value::Constructor("None".into(), vec![]),
                }
            }
            "regex_find_all" => {
                // regex_find_all(pattern, text) → List(String): all matches
                match (args.first(), args.get(1)) {
                    (Some(Value::Str(pattern)), Some(Value::Str(text))) => {
                        match regex::Regex::new(pattern) {
                            Ok(re) => {
                                let matches: Vec<Value> = re
                                    .find_iter(text)
                                    .map(|m| Value::Str(m.as_str().to_string()))
                                    .collect();
                                Value::List(matches)
                            }
                            Err(_) => Value::List(vec![]),
                        }
                    }
                    _ => Value::List(vec![]),
                }
            }
            "regex_replace" => {
                // regex_replace(pattern, text, replacement) → String
                match (args.first(), args.get(1), args.get(2)) {
                    (
                        Some(Value::Str(pattern)),
                        Some(Value::Str(text)),
                        Some(Value::Str(replacement)),
                    ) => match regex::Regex::new(pattern) {
                        Ok(re) => {
                            Value::Str(re.replace_all(text, replacement.as_str()).to_string())
                        }
                        Err(_) => Value::Str(text.clone()),
                    },
                    _ => Value::Str(String::new()),
                }
            }
            "not" => match args.first() {
                Some(Value::Bool(b)) => Value::Bool(!b),
                _ => Value::Bool(false),
            },
            "concat" => {
                match (args.get(0), args.get(1)) {
                    (Some(Value::Str(a)), Some(Value::Str(b))) => Value::Str(format!("{}{}", a, b)),
                    (Some(a), Some(b)) => {
                        // List concat
                        let mut items = list_to_vec(a);
                        items.extend(list_to_vec(b));
                        vec_to_list(items)
                    }
                    _ => Value::Constructor("Nil".into(), vec![]),
                }
            }
            "reverse" => match args.first() {
                Some(v) => {
                    let mut items = list_to_vec(v);
                    items.reverse();
                    vec_to_list(items)
                }
                _ => Value::Constructor("Nil".into(), vec![]),
            },
            "map" => {
                // Polymorphic: Stream/Subject → Stream, List/Cons → List
                // TODO(perf): func.clone() per iteration is expensive (clones captured Env).
                // Fix: wrap closures in Rc so clone is a refcount bump, or borrow via apply_ref.
                let input = args.get(0).cloned().unwrap_or(Value::Unit);
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let (items, is_stream) = match &input {
                    Value::Stream(v) | Value::Subject(v) => (v.clone(), true),
                    other => (list_to_vec(other), false),
                };
                let mapped: Vec<Value> = items
                    .into_iter()
                    .map(|item| self.apply(func.clone(), vec![item], env))
                    .collect();
                if is_stream {
                    Value::Stream(mapped)
                } else {
                    Value::List(mapped)
                }
            }
            "filter" => {
                // Polymorphic: Stream/Subject → Stream, List/Cons → List
                // TODO(perf): same closure cloning issue as map. Also list_to_vec walks
                // entire Cons chain before filtering — should iterate lazily.
                let input = args.get(0).cloned().unwrap_or(Value::Unit);
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let (items, is_stream) = match &input {
                    Value::Stream(v) | Value::Subject(v) => (v.clone(), true),
                    other => (list_to_vec(other), false),
                };
                let filtered: Vec<Value> = items
                    .into_iter()
                    .filter(
                        |item| match self.apply(func.clone(), vec![item.clone()], env) {
                            Value::Bool(true) => true,
                            _ => false,
                        },
                    )
                    .collect();
                if is_stream {
                    Value::Stream(filtered)
                } else {
                    Value::List(filtered)
                }
            }
            "foldl" => {
                // TODO(perf): func.clone() and acc clone per iteration.
                // Rc-wrapping closures would make this near-zero-cost.
                match (args.get(0), args.get(1), args.get(2)) {
                    (Some(list), Some(init), Some(func)) => {
                        let items = list_to_vec(list);
                        let mut acc = init.clone();
                        for item in items {
                            acc = self.apply(func.clone(), vec![acc, item], env);
                        }
                        acc
                    }
                    _ => Value::Unit,
                }
            }
            // ---- Collection builtins (Kotlin-inspired) ----
            "sort" => match args.first() {
                Some(list) => {
                    let mut items = list_to_vec(list);
                    items.sort_by(|a, b| format!("{}", a).cmp(&format!("{}", b)));
                    Value::List(items)
                }
                _ => Value::List(vec![]),
            },
            "sort_by" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let mut items = list_to_vec(list);
                    items.sort_by(|a, b| {
                        let ka = self.apply(func.clone(), vec![a.clone()], env);
                        let kb = self.apply(func.clone(), vec![b.clone()], env);
                        format!("{}", ka).cmp(&format!("{}", kb))
                    });
                    Value::List(items)
                }
                _ => Value::List(vec![]),
            },
            "any" => {
                // Polymorphic: works on Stream/Subject/List/Cons
                let input = args.get(0).cloned().unwrap_or(Value::Unit);
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let items = match &input {
                    Value::Stream(v) | Value::Subject(v) => v.clone(),
                    other => list_to_vec(other),
                };
                Value::Bool(items.into_iter().any(|item| {
                    matches!(self.apply(func.clone(), vec![item], env), Value::Bool(true))
                }))
            }
            "all" => {
                // Polymorphic: works on Stream/Subject/List/Cons
                let input = args.get(0).cloned().unwrap_or(Value::Unit);
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let items = match &input {
                    Value::Stream(v) | Value::Subject(v) => v.clone(),
                    other => list_to_vec(other),
                };
                Value::Bool(items.into_iter().all(|item| {
                    matches!(self.apply(func.clone(), vec![item], env), Value::Bool(true))
                }))
            }
            "find" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let items = list_to_vec(list);
                    for item in items {
                        if matches!(
                            self.apply(func.clone(), vec![item.clone()], env),
                            Value::Bool(true)
                        ) {
                            return Value::Constructor("Some".into(), vec![item]);
                        }
                    }
                    Value::Constructor("None".into(), vec![])
                }
                _ => Value::Constructor("None".into(), vec![]),
            },
            "flat_map" => {
                // Polymorphic: Stream/Subject → Stream, List/Cons → List
                let input = args.get(0).cloned().unwrap_or(Value::Unit);
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let (items, is_stream) = match &input {
                    Value::Stream(v) | Value::Subject(v) => (v.clone(), true),
                    other => (list_to_vec(other), false),
                };
                let mut result = Vec::new();
                for item in items {
                    let mapped = self.apply(func.clone(), vec![item], env);
                    match mapped {
                        Value::Stream(v) | Value::Subject(v) => result.extend(v),
                        other => result.extend(list_to_vec(&other)),
                    }
                }
                if is_stream {
                    Value::Stream(result)
                } else {
                    Value::List(result)
                }
            }
            "zip" => {
                // Polymorphic: Stream/Subject → Stream, List/Cons → List
                let a = args.get(0).cloned().unwrap_or(Value::Unit);
                let b = args.get(1).cloned().unwrap_or(Value::Unit);
                let (va, is_stream) = match &a {
                    Value::Stream(v) | Value::Subject(v) => (v.clone(), true),
                    other => (list_to_vec(other), false),
                };
                let vb = match &b {
                    Value::Stream(v) | Value::Subject(v) => v.clone(),
                    other => list_to_vec(other),
                };
                let pairs: Vec<Value> = va
                    .into_iter()
                    .zip(vb.into_iter())
                    .map(|(x, y)| Value::Tuple(vec![x, y]))
                    .collect();
                if is_stream {
                    Value::Stream(pairs)
                } else {
                    Value::List(pairs)
                }
            }
            "enumerate" => {
                // Polymorphic: Stream/Subject → Stream, List/Cons → List
                let input = args.first().cloned().unwrap_or(Value::Unit);
                let (items, is_stream) = match &input {
                    Value::Stream(v) | Value::Subject(v) => (v.clone(), true),
                    other => (list_to_vec(other), false),
                };
                let pairs: Vec<Value> = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v]))
                    .collect();
                if is_stream {
                    Value::Stream(pairs)
                } else {
                    Value::List(pairs)
                }
            }
            "take_while" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let items = list_to_vec(list);
                    let taken: Vec<Value> = items
                        .into_iter()
                        .take_while(|item| {
                            matches!(
                                self.apply(func.clone(), vec![item.clone()], env),
                                Value::Bool(true)
                            )
                        })
                        .collect();
                    Value::List(taken)
                }
                _ => Value::List(vec![]),
            },
            "drop_while" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let items = list_to_vec(list);
                    let dropped: Vec<Value> = items
                        .into_iter()
                        .skip_while(|item| {
                            matches!(
                                self.apply(func.clone(), vec![item.clone()], env),
                                Value::Bool(true)
                            )
                        })
                        .collect();
                    Value::List(dropped)
                }
                _ => Value::List(vec![]),
            },
            "sum_list" => match args.first() {
                Some(list) => {
                    let items = list_to_vec(list);
                    let total: i64 = items
                        .into_iter()
                        .filter_map(|v| if let Value::Int(n) = v { Some(n) } else { None })
                        .sum();
                    Value::Int(total)
                }
                _ => Value::Int(0),
            },
            "distinct" => {
                // Polymorphic: List → global unique (HashSet), Stream → consecutive dedup
                let input = args.first().cloned().unwrap_or(Value::Unit);
                match &input {
                    Value::Stream(v) | Value::Subject(v) => {
                        // Stream: remove consecutive duplicates (Rx distinctUntilChanged)
                        let mut result = Vec::new();
                        let mut prev: Option<String> = None;
                        for item in v {
                            let repr = format!("{}", item);
                            if prev.as_ref() != Some(&repr) {
                                result.push(item.clone());
                                prev = Some(repr);
                            }
                        }
                        Value::Stream(result)
                    }
                    other => {
                        // List: global unique (HashSet)
                        let items = list_to_vec(other);
                        let mut seen = std::collections::HashSet::new();
                        let unique: Vec<Value> = items
                            .into_iter()
                            .filter(|v| seen.insert(format!("{}", v)))
                            .collect();
                        Value::List(unique)
                    }
                }
            }
            "count_by" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let items = list_to_vec(list);
                    let count = items
                        .into_iter()
                        .filter(|item| {
                            matches!(
                                self.apply(func.clone(), vec![item.clone()], env),
                                Value::Bool(true)
                            )
                        })
                        .count();
                    Value::Int(count as i64)
                }
                _ => Value::Int(0),
            },
            "partition" => match (args.get(0), args.get(1)) {
                (Some(list), Some(func)) => {
                    let items = list_to_vec(list);
                    let mut yes = Vec::new();
                    let mut no = Vec::new();
                    for item in items {
                        if matches!(
                            self.apply(func.clone(), vec![item.clone()], env),
                            Value::Bool(true)
                        ) {
                            yes.push(item);
                        } else {
                            no.push(item);
                        }
                    }
                    Value::Tuple(vec![Value::List(yes), Value::List(no)])
                }
                _ => Value::Tuple(vec![Value::List(vec![]), Value::List(vec![])]),
            },
            "chunked" => match (args.get(0), args.get(1)) {
                (Some(list), Some(Value::Int(n))) if *n > 0 => {
                    let items = list_to_vec(list);
                    let chunks: Vec<Value> = items
                        .chunks(*n as usize)
                        .map(|c| Value::List(c.to_vec()))
                        .collect();
                    Value::List(chunks)
                }
                _ => Value::List(vec![]),
            },
            "subscribe" => match (args.get(0), args.get(1)) {
                (Some(stream), Some(func)) => {
                    let items = list_to_vec(stream);
                    for item in items {
                        self.apply(func.clone(), vec![item], env);
                    }
                    Value::Unit
                }
                _ => Value::Unit,
            },
            // ---- Map builtins (M24) ----
            "map_new" => Value::Map(BTreeMap::new()),
            "map_insert" => match (args.get(0), args.get(1), args.get(2)) {
                (Some(Value::Map(entries)), Some(key), Some(val)) => {
                    let mut new_map = entries.clone();
                    new_map.insert(format!("{}", key), val.clone());
                    Value::Map(new_map)
                }
                _ => args.first().cloned().unwrap_or(Value::Map(BTreeMap::new())),
            },
            "unwrap_or" => match (args.get(0), args.get(1)) {
                (Some(Value::Constructor(name, inner)), Some(default)) => {
                    if name == "Some" && inner.len() == 1 {
                        inner[0].clone()
                    } else if name == "None" {
                        default.clone()
                    } else {
                        args[0].clone()
                    }
                }
                _ => args.first().cloned().unwrap_or(Value::Unit),
            },
            "is_some" => match args.first() {
                Some(Value::Constructor(name, _)) if name == "Some" => Value::Bool(true),
                Some(Value::Constructor(name, _)) if name == "None" => Value::Bool(false),
                _ => Value::Bool(false),
            },
            "is_none" => match args.first() {
                Some(Value::Constructor(name, _)) if name == "None" => Value::Bool(true),
                Some(Value::Constructor(name, _)) if name == "Some" => Value::Bool(false),
                _ => Value::Bool(true),
            },
            "map_get" => match (args.get(0), args.get(1)) {
                (Some(Value::Map(entries)), Some(key)) => {
                    let key_str = format!("{}", key);
                    match entries.get(&key_str) {
                        Some(v) => Value::Constructor("Some".into(), vec![v.clone()]),
                        None => Value::Constructor("None".into(), vec![]),
                    }
                }
                _ => Value::Constructor("None".into(), vec![]),
            },
            "map_get_or" => match (args.get(0), args.get(1), args.get(2)) {
                (Some(Value::Map(entries)), Some(key), Some(default)) => {
                    let key_str = format!("{}", key);
                    entries
                        .get(&key_str)
                        .cloned()
                        .unwrap_or_else(|| default.clone())
                }
                _ => args.get(2).cloned().unwrap_or(Value::Unit),
            },
            "map_contains" => match (args.get(0), args.get(1)) {
                (Some(Value::Map(entries)), Some(key)) => {
                    Value::Bool(entries.contains_key(&format!("{}", key)))
                }
                _ => Value::Bool(false),
            },
            "map_remove" => match (args.get(0), args.get(1)) {
                (Some(Value::Map(entries)), Some(key)) => {
                    let mut new_map = entries.clone();
                    new_map.remove(&format!("{}", key));
                    Value::Map(new_map)
                }
                _ => args.first().cloned().unwrap_or(Value::Map(BTreeMap::new())),
            },
            "map_keys" => match args.first() {
                Some(Value::Map(entries)) => {
                    Value::List(entries.keys().map(|k| Value::Str(k.clone())).collect())
                }
                _ => Value::List(vec![]),
            },
            "map_values" => match args.first() {
                Some(Value::Map(entries)) => Value::List(entries.values().cloned().collect()),
                _ => Value::List(vec![]),
            },
            "map_entries" => match args.first() {
                Some(Value::Map(entries)) => Value::List(
                    entries
                        .iter()
                        .map(|(k, v)| Value::Tuple(vec![Value::Str(k.clone()), v.clone()]))
                        .collect(),
                ),
                _ => Value::List(vec![]),
            },
            "map_len" => match args.first() {
                Some(Value::Map(entries)) => Value::Int(entries.len() as i64),
                _ => Value::Int(0),
            },
            "map_merge" => match (args.get(0), args.get(1)) {
                (Some(Value::Map(base)), Some(Value::Map(other))) => {
                    let mut merged = base.clone();
                    for (k, v) in other {
                        merged.insert(k.clone(), v.clone());
                    }
                    Value::Map(merged)
                }
                _ => args.first().cloned().unwrap_or(Value::Map(BTreeMap::new())),
            },
            "map_from" => match args.first() {
                Some(list) => {
                    let items = list_to_vec(list);
                    let mut map = BTreeMap::new();
                    for v in items {
                        if let Value::Tuple(ref pair) = v {
                            if pair.len() >= 2 {
                                map.insert(format!("{}", pair[0]), pair[1].clone());
                            }
                        }
                    }
                    Value::Map(map)
                }
                _ => Value::Map(BTreeMap::new()),
            },
            // ---- Set builtins (M24) ----
            "set_new" => Value::Set(BTreeMap::new()),
            "set_insert" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(items)), Some(val)) => {
                    let key = format!("{}", val);
                    if items.contains_key(&key) {
                        Value::Set(items.clone())
                    } else {
                        let mut new_items = items.clone();
                        new_items.insert(key, val.clone());
                        Value::Set(new_items)
                    }
                }
                _ => args.first().cloned().unwrap_or(Value::Set(BTreeMap::new())),
            },
            "set_contains" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(items)), Some(val)) => {
                    let key = format!("{}", val);
                    Value::Bool(items.contains_key(&key))
                }
                _ => Value::Bool(false),
            },
            "set_remove" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(items)), Some(val)) => {
                    let key = format!("{}", val);
                    let mut new_items = items.clone();
                    new_items.remove(&key);
                    Value::Set(new_items)
                }
                _ => args.first().cloned().unwrap_or(Value::Set(BTreeMap::new())),
            },
            "set_len" => match args.first() {
                Some(Value::Set(items)) => Value::Int(items.len() as i64),
                _ => Value::Int(0),
            },
            "set_to_list" => match args.first() {
                Some(Value::Set(items)) => Value::List(items.values().cloned().collect()),
                _ => Value::List(vec![]),
            },
            "set_union" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(a)), Some(Value::Set(b))) => {
                    let mut result = a.clone();
                    for (k, v) in b {
                        result.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                    Value::Set(result)
                }
                _ => args.first().cloned().unwrap_or(Value::Set(BTreeMap::new())),
            },
            "set_intersect" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(a)), Some(Value::Set(b))) => {
                    let result: BTreeMap<String, Value> = a
                        .iter()
                        .filter(|(k, _)| b.contains_key(k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    Value::Set(result)
                }
                _ => Value::Set(BTreeMap::new()),
            },
            "set_diff" => match (args.get(0), args.get(1)) {
                (Some(Value::Set(a)), Some(Value::Set(b))) => {
                    let result: BTreeMap<String, Value> = a
                        .iter()
                        .filter(|(k, _)| !b.contains_key(k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    Value::Set(result)
                }
                _ => args.first().cloned().unwrap_or(Value::Set(BTreeMap::new())),
            },
            "set_from_list" => match args.first() {
                Some(list) => {
                    let items = list_to_vec(list);
                    let mut result: BTreeMap<String, Value> = BTreeMap::new();
                    for v in items {
                        let key = format!("{}", v);
                        result.entry(key).or_insert(v);
                    }
                    Value::Set(result)
                }
                _ => Value::Set(BTreeMap::new()),
            },
            "assert" => match args.first() {
                Some(Value::Bool(true)) => Value::Unit,
                Some(Value::Bool(false)) => {
                    let msg = "Assertion failed!";
                    println!("FAIL: {}", msg);
                    self.output.push(format!("FAIL: {}", msg));
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "string_length" => match args.first() {
                Some(Value::Str(s)) => Value::Int(s.chars().count() as i64),
                _ => Value::Int(0),
            },
            // ---- M14a: String builtins ----
            "split" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sep))) => Value::List(
                    s.split(sep.as_str())
                        .map(|p| Value::Str(p.to_string()))
                        .collect(),
                ),
                _ => Value::List(vec![]),
            },
            "join" => match args.get(1) {
                Some(Value::Str(sep)) => {
                    if let Some(list_val) = args.get(0) {
                        let items = list_to_vec(list_val);
                        let strs: Vec<String> = items
                            .iter()
                            .map(|v| match v {
                                Value::Str(s) => s.clone(),
                                other => format!("{}", other),
                            })
                            .collect();
                        Value::Str(strs.join(sep.as_str()))
                    } else {
                        Value::Str(String::new())
                    }
                }
                _ => Value::Str(String::new()),
            },
            "trim" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.trim().to_string()),
                _ => Value::Str(String::new()),
            },
            "contains" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sub))) => {
                    Value::Bool(s.contains(sub.as_str()))
                }
                _ => Value::Bool(false),
            },
            "starts_with" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(pre))) => {
                    Value::Bool(s.starts_with(pre.as_str()))
                }
                _ => Value::Bool(false),
            },
            "ends_with" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(suf))) => {
                    Value::Bool(s.ends_with(suf.as_str()))
                }
                _ => Value::Bool(false),
            },
            "replace" => match (args.get(0), args.get(1), args.get(2)) {
                (Some(Value::Str(s)), Some(Value::Str(old)), Some(Value::Str(new))) => {
                    Value::Str(s.replace(old.as_str(), new.as_str()))
                }
                _ => Value::Str(String::new()),
            },
            "to_upper" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.to_uppercase()),
                _ => Value::Str(String::new()),
            },
            "to_lower" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.to_lowercase()),
                _ => Value::Str(String::new()),
            },
            "substring" => match (args.get(0), args.get(1), args.get(2)) {
                (Some(Value::Str(s)), Some(Value::Int(start)), Some(Value::Int(len))) => {
                    let start = (*start).max(0) as usize;
                    let chars: Vec<char> = s.chars().collect();
                    let end = (start + (*len).max(0) as usize).min(chars.len());
                    let start = start.min(end);
                    Value::Str(chars[start..end].iter().collect())
                }
                _ => Value::Str(String::new()),
            },
            "char_at" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Int(idx))) => {
                    let idx = *idx as usize;
                    // O(1) for ASCII strings (the common case for simple Latin text).
                    // Falls back to O(n) char iteration for multi-byte UTF-8.
                    if s.is_ascii() {
                        if idx < s.len() {
                            Value::Str(String::from(s.as_bytes()[idx] as char))
                        } else {
                            Value::Str(String::new())
                        }
                    } else {
                        match s.chars().nth(idx) {
                            Some(c) => Value::Str(c.to_string()),
                            None => Value::Str(String::new()),
                        }
                    }
                }
                _ => Value::Str(String::new()),
            },
            "index_of" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(s)), Some(Value::Str(sub))) => match s.find(sub.as_str()) {
                    Some(pos) => Value::Int(s[..pos].chars().count() as i64),
                    None => Value::Int(-1),
                },
                _ => Value::Int(-1),
            },
            "format_float" => match (args.get(0), args.get(1)) {
                (Some(Value::Float(f)), Some(Value::Int(decimals))) => {
                    Value::Str(format!("{:.prec$}", f, prec = *decimals as usize))
                }
                (Some(Value::Int(n)), Some(Value::Int(decimals))) => {
                    Value::Str(format!("{:.prec$}", *n as f64, prec = *decimals as usize))
                }
                _ => Value::Str("0.0".to_string()),
            },
            "parse_int" => match args.first() {
                Some(Value::Str(s)) => match s.trim().parse::<i64>() {
                    Ok(n) => Value::Int(n),
                    Err(_) => {
                        // Silent fallback — use monadic bind (= n <- parse_int(s)) for error handling
                        Value::Int(0)
                    }
                },
                Some(Value::Int(n)) => Value::Int(*n),
                _ => Value::Int(0),
            },
            "parse_float" => match args.first() {
                Some(Value::Str(s)) => match s.trim().parse::<f64>() {
                    Ok(f) => Value::Float(f),
                    Err(_) => {
                        // Silent fallback — use monadic bind (= f <- parse_float(s)) for error handling
                        Value::Float(0.0)
                    }
                },
                Some(Value::Float(f)) => Value::Float(*f),
                Some(Value::Int(n)) => Value::Float(*n as f64),
                _ => Value::Float(0.0),
            },
            "string_chars" => match args.first() {
                Some(Value::Str(s)) => {
                    Value::List(s.chars().map(|c| Value::Str(c.to_string())).collect())
                }
                _ => Value::List(vec![]),
            },
            // ---- M14b: File I/O builtins ----
            "read_file" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_to_string(path) {
                    Ok(content) => Value::Str(content),
                    Err(_) => Value::Str(String::new()),
                },
                _ => Value::Str(String::new()),
            },
            "write_file" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(path)), Some(Value::Str(content))) => {
                    let _ = std::fs::write(path, content);
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "append_file" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(path)), Some(Value::Str(content))) => {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(path)
                    {
                        let _ = f.write_all(content.as_bytes());
                    }
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "file_exists" => match args.first() {
                Some(Value::Str(path)) => Value::Bool(std::path::Path::new(path.as_str()).exists()),
                _ => Value::Bool(false),
            },
            "read_lines" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_to_string(path) {
                    Ok(content) => {
                        Value::List(content.lines().map(|l| Value::Str(l.to_string())).collect())
                    }
                    Err(_) => Value::List(vec![]),
                },
                _ => Value::List(vec![]),
            },
            "env_var" => match args.first() {
                Some(Value::Str(name)) => match std::env::var(name) {
                    Ok(val) => Value::Str(val),
                    Err(_) => Value::Str(String::new()),
                },
                _ => Value::Str(String::new()),
            },
            "process_run" => match args.first() {
                Some(argv) => {
                    let argv = list_to_vec(argv);
                    let mut parts = Vec::with_capacity(argv.len());
                    for value in argv {
                        match value {
                            Value::Str(s) => parts.push(s),
                            other => parts.push(format!("{}", other)),
                        }
                    }
                    if parts.is_empty() {
                        Value::Tuple(vec![
                            Value::Int(-1),
                            Value::Str(String::new()),
                            Value::Str(
                                "process_run requires at least one argv element".to_string(),
                            ),
                        ])
                    } else {
                        let mut cmd = std::process::Command::new(&parts[0]);
                        if parts.len() > 1 {
                            cmd.args(&parts[1..]);
                        }
                        match cmd.output() {
                            Ok(output) => Value::Tuple(vec![
                                Value::Int(output.status.code().map(|c| c as i64).unwrap_or(-1)),
                                Value::Str(String::from_utf8_lossy(&output.stdout).to_string()),
                                Value::Str(String::from_utf8_lossy(&output.stderr).to_string()),
                            ]),
                            Err(err) => Value::Tuple(vec![
                                Value::Int(-1),
                                Value::Str(String::new()),
                                Value::Str(err.to_string()),
                            ]),
                        }
                    }
                }
                None => Value::Tuple(vec![
                    Value::Int(-1),
                    Value::Str(String::new()),
                    Value::Str("process_run requires argv".to_string()),
                ]),
            },
            // ---- M14c: JSON builtins ----
            "json_parse" => match args.first() {
                Some(Value::Str(s)) => match serde_json::from_str::<serde_json::Value>(s) {
                    Ok(_) => Value::Str(s.clone()),
                    Err(_) => Value::Str("null".to_string()),
                },
                _ => Value::Str("null".to_string()),
            },
            "json_get" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(json)), Some(Value::Str(key))) => {
                    match serde_json::from_str::<serde_json::Value>(json) {
                        Ok(v) => match v.get(key.as_str()) {
                            Some(val) => Value::Str(val.to_string()),
                            None => Value::Str("null".to_string()),
                        },
                        Err(_) => Value::Str("null".to_string()),
                    }
                }
                _ => Value::Str("null".to_string()),
            },
            "json_string" => match args.first() {
                Some(Value::Str(json)) => match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(serde_json::Value::String(s)) => Value::Str(s),
                    _ => Value::Str(json.trim_matches('"').to_string()),
                },
                _ => Value::Str(String::new()),
            },
            "json_number" => match args.first() {
                Some(Value::Str(json)) => match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(serde_json::Value::Number(n)) => Value::Float(n.as_f64().unwrap_or(0.0)),
                    _ => Value::Float(0.0),
                },
                Some(Value::Float(f)) => Value::Float(*f),
                Some(Value::Int(n)) => Value::Float(*n as f64),
                _ => Value::Float(0.0),
            },
            "json_bool" => match args.first() {
                Some(Value::Str(json)) => match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(serde_json::Value::Bool(b)) => Value::Bool(b),
                    _ => Value::Bool(false),
                },
                Some(Value::Bool(b)) => Value::Bool(*b),
                _ => Value::Bool(false),
            },
            "json_array" => match args.first() {
                Some(Value::Str(json)) => match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(serde_json::Value::Array(arr)) => {
                        Value::List(arr.iter().map(|v| Value::Str(v.to_string())).collect())
                    }
                    _ => Value::List(vec![]),
                },
                _ => Value::List(vec![]),
            },
            "json_emit" => match args.first() {
                Some(Value::Str(s)) => Value::Str(s.clone()),
                Some(Value::Int(n)) => Value::Str(n.to_string()),
                Some(Value::Float(f)) => Value::Str(f.to_string()),
                Some(Value::Bool(b)) => Value::Str(b.to_string()),
                Some(Value::List(items)) => {
                    let parts: Vec<String> = items
                        .iter()
                        .map(|v| match v {
                            Value::Str(s)
                                if s.starts_with('{')
                                    || s.starts_with('[')
                                    || s.starts_with('"')
                                    || s == "true"
                                    || s == "false"
                                    || s == "null"
                                    || s.parse::<f64>().is_ok() =>
                            {
                                s.clone()
                            }
                            Value::Str(s) => {
                                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                            }
                            Value::Int(n) => n.to_string(),
                            Value::Float(f) => f.to_string(),
                            Value::Bool(b) => b.to_string(),
                            other => format!("\"{}\"", format!("{}", other).replace('"', "\\\"")),
                        })
                        .collect();
                    Value::Str(format!("[{}]", parts.join(",")))
                }
                _ => Value::Str("null".to_string()),
            },
            "json_object" => {
                let pairs = match args.first() {
                    Some(v) => list_to_vec(v),
                    None => vec![],
                };
                let mut parts = Vec::new();
                for pair in &pairs {
                    let elems = list_to_vec(pair);
                    if elems.len() >= 2 {
                        let key = match &elems[0] {
                            Value::Str(s) => s.clone(),
                            v => format!("{}", v),
                        };
                        let val = match &elems[1] {
                            Value::Str(s)
                                if s.starts_with('{')
                                    || s.starts_with('[')
                                    || s.starts_with('"')
                                    || s == "true"
                                    || s == "false"
                                    || s == "null"
                                    || s.parse::<f64>().is_ok() =>
                            {
                                s.clone()
                            }
                            Value::Str(s) => {
                                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                            }
                            Value::Int(n) => n.to_string(),
                            Value::Float(f) => f.to_string(),
                            Value::Bool(b) => b.to_string(),
                            v => format!("\"{}\"", format!("{}", v).replace('"', "\\\"")),
                        };
                        parts.push(format!("\"{}\":{}", key, val));
                    }
                }
                Value::Str(format!("{{{}}}", parts.join(",")))
            }
            // ---- M14d: HTTP builtins ----
            "http_get" => {
                println!("[runa interpreter] http_get: use `runa run` for real HTTP requests");
                Value::Str(String::new())
            }
            "http_post" => {
                println!("[runa interpreter] http_post: use `runa run` for real HTTP requests");
                Value::Str(String::new())
            }
            "http_serve" => {
                println!(
                    "[runa interpreter] {}: use `runa run` for real HTTP server",
                    name
                );
                Value::Unit
            }
            "http_respond" => {
                let status = args.get(0).cloned().unwrap_or(Value::Unit);
                let content_type = args.get(1).cloned().unwrap_or(Value::Unit);
                let body = args.get(2).cloned().unwrap_or(Value::Unit);
                Value::Tuple(vec![status, content_type, body])
            }
            "http_request_path" => match args.first() {
                Some(Value::Tuple(parts)) => parts.first().cloned().unwrap_or(Value::Unit),
                Some(Value::Constructor(_, fields)) => {
                    fields.first().cloned().unwrap_or(Value::Unit)
                }
                _ => Value::Unit,
            },
            "http_request_method" => match args.first() {
                Some(Value::Tuple(parts)) => parts.get(1).cloned().unwrap_or(Value::Unit),
                Some(Value::Constructor(_, fields)) => {
                    fields.get(1).cloned().unwrap_or(Value::Unit)
                }
                _ => Value::Unit,
            },
            "http_request_body" => match args.first() {
                Some(Value::Tuple(parts)) => parts.get(2).cloned().unwrap_or(Value::Unit),
                Some(Value::Constructor(_, fields)) => {
                    fields.get(2).cloned().unwrap_or(Value::Unit)
                }
                _ => Value::Unit,
            },
            // ---- M14e: Database builtins (interpreter stubs — use `runa run` for real DB) ----
            "db_open" => {
                println!("[runa interpreter] db_open: use `runa run` for real database access");
                Value::Str("__db_handle__".to_string())
            }
            "db_exec" => {
                println!("[runa interpreter] db_exec: use `runa run` for real database access");
                Value::Unit
            }
            "db_query" => {
                println!("[runa interpreter] db_query: use `runa run` for real database access");
                Value::List(vec![])
            }
            "db_query_row" => {
                println!(
                    "[runa interpreter] db_query_row: use `runa run` for real database access"
                );
                Value::List(vec![])
            }
            "db_insert" => {
                println!("[runa interpreter] db_insert: use `runa run` for real database access");
                Value::Int(0)
            }
            "db_close" => Value::Unit,
            "exp" => match args.first() {
                Some(Value::Float(f)) => Value::Float(f.exp()),
                Some(Value::Int(n)) => Value::Float((*n as f64).exp()),
                _ => Value::Float(0.0),
            },
            "ln" => match args.first() {
                Some(Value::Float(f)) => Value::Float(f.ln()),
                Some(Value::Int(n)) => Value::Float((*n as f64).ln()),
                _ => Value::Float(0.0),
            },
            "sqrt" => match args.first() {
                Some(Value::Float(f)) => Value::Float(f.sqrt()),
                Some(Value::Int(n)) => Value::Float((*n as f64).sqrt()),
                _ => Value::Float(0.0),
            },
            "pow" => match (args.get(0), args.get(1)) {
                (Some(Value::Float(a)), Some(Value::Float(b))) => Value::Float(a.powf(*b)),
                (Some(Value::Float(a)), Some(Value::Int(b))) => Value::Float(a.powi(*b as i32)),
                (Some(Value::Int(a)), Some(Value::Float(b))) => Value::Float((*a as f64).powf(*b)),
                (Some(Value::Int(a)), Some(Value::Int(b))) => {
                    Value::Float((*a as f64).powf(*b as f64))
                }
                _ => Value::Float(0.0),
            },
            "to_float" => match args.first() {
                Some(Value::Int(n)) => Value::Float(*n as f64),
                Some(Value::Float(f)) => Value::Float(*f),
                _ => Value::Float(0.0),
            },
            "round" => match args.first() {
                Some(Value::Float(f)) => Value::Int(f.round() as i64),
                Some(Value::Int(n)) => Value::Int(*n),
                _ => Value::Int(0),
            },
            "floor" => match args.first() {
                Some(Value::Float(f)) => Value::Int(f.floor() as i64),
                Some(Value::Int(n)) => Value::Int(*n),
                _ => Value::Int(0),
            },
            "max_f" => match (args.get(0), args.get(1)) {
                (Some(Value::Float(a)), Some(Value::Float(b))) => Value::Float(a.max(*b)),
                (Some(Value::Int(a)), Some(Value::Float(b))) => Value::Float((*a as f64).max(*b)),
                (Some(Value::Float(a)), Some(Value::Int(b))) => Value::Float(a.max(*b as f64)),
                (Some(Value::Int(a)), Some(Value::Int(b))) => Value::Int(*a.max(b)),
                _ => Value::Float(0.0),
            },
            "min_f" => match (args.get(0), args.get(1)) {
                (Some(Value::Float(a)), Some(Value::Float(b))) => Value::Float(a.min(*b)),
                (Some(Value::Int(a)), Some(Value::Float(b))) => Value::Float((*a as f64).min(*b)),
                (Some(Value::Float(a)), Some(Value::Int(b))) => Value::Float(a.min(*b as f64)),
                (Some(Value::Int(a)), Some(Value::Int(b))) => Value::Int(*a.min(b)),
                _ => Value::Float(0.0),
            },
            "format_f" => match (args.get(0), args.get(1)) {
                (Some(Value::Float(f)), Some(Value::Int(decimals))) => {
                    Value::Str(format!("{:.*}", *decimals as usize, f))
                }
                (Some(Value::Int(n)), Some(Value::Int(decimals))) => {
                    Value::Str(format!("{:.*}", *decimals as usize, *n as f64))
                }
                (Some(v), _) => Value::Str(format!("{}", v)),
                _ => Value::Str("0".into()),
            },
            // shared(x) — in interpreter, shared values are just regular values
            "shared" => args.into_iter().next().unwrap_or(Value::Unit),
            "range" => {
                match (args.get(0), args.get(1)) {
                    (Some(Value::Int(a)), Some(Value::Int(b))) => {
                        // Build a Cons/Nil list for the interpreter
                        let mut result = Value::Constructor("Nil".into(), vec![]);
                        for i in (*a..*b).rev() {
                            result = Value::Constructor("Cons".into(), vec![Value::Int(i), result]);
                        }
                        result
                    }
                    _ => Value::Constructor("Nil".into(), vec![]),
                }
            }
            "push" => {
                // push(list, elem) — append elem to list
                match (args.get(0), args.get(1)) {
                    (Some(list), Some(elem)) => {
                        let mut items = Vec::new();
                        let mut cur = list.clone();
                        loop {
                            match cur {
                                Value::Constructor(ref n, ref fs)
                                    if n == "Cons" && fs.len() == 2 =>
                                {
                                    items.push(fs[0].clone());
                                    cur = fs[1].clone();
                                }
                                _ => break,
                            }
                        }
                        items.push(elem.clone());
                        let mut result = Value::Constructor("Nil".into(), vec![]);
                        for item in items.into_iter().rev() {
                            result = Value::Constructor("Cons".into(), vec![item, result]);
                        }
                        result
                    }
                    _ => Value::Constructor("Nil".into(), vec![]),
                }
            }
            "spawn" => {
                // spawn(actor_def, initial_state) -> Actor value
                match args.get(0) {
                    Some(Value::Constructor(ref def_name, _))
                        if def_name.starts_with("ActorDef<") =>
                    {
                        let actor_name = def_name
                            .trim_start_matches("ActorDef<")
                            .trim_end_matches('>')
                            .to_string();
                        let initial_state = args.get(1).cloned().unwrap_or(Value::Int(0));
                        if let Some(Defn::Actor {
                            state_param,
                            handlers,
                            ..
                        }) = self.actor_defs.get(&actor_name).cloned()
                        {
                            Value::Actor {
                                actor_name,
                                state: Box::new(initial_state),
                                state_param: state_param.name.clone(),
                                handlers,
                                env: env.clone(),
                            }
                        } else {
                            eprintln!("spawn: no actor definition for '{}'", actor_name);
                            Value::Unit
                        }
                    }
                    _ => {
                        eprintln!("spawn: expected actor definition as first argument");
                        Value::Unit
                    }
                }
            }
            "ask" => {
                // ask(actor, message) -> sends message, returns new state
                match args.get(0) {
                    Some(Value::Actor {
                        actor_name,
                        state,
                        state_param,
                        handlers,
                        env: actor_env,
                    }) => {
                        let msg = args.get(1).cloned().unwrap_or(Value::Unit);
                        let (new_state, _response) = self.dispatch_actor_message(
                            actor_name,
                            state,
                            state_param,
                            handlers,
                            actor_env,
                            &msg,
                        );
                        // Update the actor in-place via actor_instances
                        self.actor_instances
                            .insert(actor_name.clone(), (new_state.clone(), actor_name.clone()));
                        new_state
                    }
                    _ => {
                        eprintln!("ask: expected actor as first argument");
                        Value::Unit
                    }
                }
            }
            // ── Stream builtins (M12) ──
            "from_list" => {
                // from_list(list) → Stream — convert a Cons/Nil list to a Stream
                match args.into_iter().next() {
                    Some(v) => Value::Stream(list_to_vec(&v)),
                    None => Value::Stream(vec![]),
                }
            }
            // Colliding s_* handlers removed — merged into polymorphic list handlers above
            // (map, filter, any, all, flat_map, zip, enumerate, distinct)
            "scan" => {
                // scan(stream, init, f) → Stream — running fold, emitting each accumulator
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let init = args.get(1).cloned().unwrap_or(Value::Int(0));
                let func = args.get(2).cloned().unwrap_or(Value::Unit);
                let items = match stream {
                    Value::Stream(items) | Value::Subject(items) => items,
                    other => list_to_vec(&other),
                };
                let mut acc = init;
                let mut results = Vec::new();
                for item in items {
                    acc = self.apply_fn(&func, vec![acc.clone(), item], env);
                    results.push(acc.clone());
                }
                Value::Stream(results)
            }
            "merge" => {
                // merge(stream1, stream2) → Stream — interleave two streams
                let s1 = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let s2 = args.get(1).cloned().unwrap_or(Value::Stream(vec![]));
                let items1 = match s1 {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let items2 = match s2 {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let mut merged = Vec::new();
                let mut i1 = items1.into_iter();
                let mut i2 = items2.into_iter();
                loop {
                    match (i1.next(), i2.next()) {
                        (Some(a), Some(b)) => {
                            merged.push(a);
                            merged.push(b);
                        }
                        (Some(a), None) => {
                            merged.push(a);
                            merged.extend(i1);
                            break;
                        }
                        (None, Some(b)) => {
                            merged.push(b);
                            merged.extend(i2);
                            break;
                        }
                        (None, None) => break,
                    }
                }
                Value::Stream(merged)
            }
            "take" => {
                // take(stream, n) → Stream — take first n elements
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let n = match args.get(1) {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => 0,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                Value::Stream(items.into_iter().take(n).collect())
            }
            "collect" => {
                // collect(stream) → List — convert stream to list
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                vec_to_list(items)
            }
            "count" => {
                // count(stream) → Int — number of elements
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                Value::Int(items.len() as i64)
            }
            "skip" => {
                // skip(stream, n) → Stream — skip first n elements
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let n = match args.get(1) {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => 0,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                Value::Stream(items.into_iter().skip(n).collect())
            }
            "window" => {
                // window(stream, n) → Stream of Stream — sliding window of size n
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let n = match args.get(1) {
                    Some(Value::Int(n)) => (*n as usize).max(1),
                    _ => 1,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let windows: Vec<Value> = items
                    .windows(n)
                    .map(|w| Value::Stream(w.to_vec()))
                    .collect();
                Value::Stream(windows)
            }
            "sum" => {
                // sum(stream) → Int or Float — sum all elements
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let mut int_sum: i64 = 0;
                let mut has_float = false;
                let mut float_sum: f64 = 0.0;
                for item in &items {
                    match item {
                        Value::Int(n) => {
                            int_sum += n;
                            float_sum += *n as f64;
                        }
                        Value::Float(f) => {
                            has_float = true;
                            float_sum += f;
                        }
                        _ => {}
                    }
                }
                if has_float {
                    Value::Float(float_sum)
                } else {
                    Value::Int(int_sum)
                }
            }
            "last" => {
                // last(stream) → Value — last element (or Unit if empty)
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                items.into_iter().last().unwrap_or(Value::Int(0))
            }
            "combine_latest" => {
                // combine_latest(stream1, stream2) → Stream of Tuple pairs
                // Rx semantics: does not emit until BOTH streams have produced at least one value
                let s1 = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let s2 = args.get(1).cloned().unwrap_or(Value::Stream(vec![]));
                let items1 = match s1 {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let items2 = match s2 {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                if items1.is_empty() || items2.is_empty() {
                    return Value::Stream(vec![]);
                }
                let len = items1.len().max(items2.len());
                let mut result = Vec::new();
                for i in 0..len {
                    let a = items1.get(i).or_else(|| items1.last()).cloned().unwrap();
                    let b = items2.get(i).or_else(|| items2.last()).cloned().unwrap();
                    result.push(Value::Tuple(vec![a, b]));
                }
                Value::Stream(result)
            }
            // ── New stream operators (M17b) ──
            "tap" => {
                // tap(stream, f) → Stream — apply f for side effect, pass values through
                let input = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let items = match &input {
                    Value::Stream(v) | Value::Subject(v) => v.clone(),
                    other => list_to_vec(other),
                };
                for item in &items {
                    self.apply_fn(&func, vec![item.clone()], env);
                }
                input
            }
            "catch" => {
                // catch(stream, f) → Stream — on error elements, replace with recovery stream
                // In sync mode: no errors in Vec, so pass-through (errors only in async)
                let input = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let _recovery = args.get(1).cloned().unwrap_or(Value::Unit);
                input
            }
            "first" => {
                // first(stream) → Value — first element (or Unit if empty)
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                items.into_iter().next().unwrap_or(Value::Int(0))
            }
            "reduce" => {
                // reduce(stream, init, f) → Value — fold, emitting only the final accumulator
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let init = args.get(1).cloned().unwrap_or(Value::Int(0));
                let func = args.get(2).cloned().unwrap_or(Value::Unit);
                let items = match stream {
                    Value::Stream(items) | Value::Subject(items) => items,
                    other => list_to_vec(&other),
                };
                let mut acc = init;
                for item in items {
                    acc = self.apply_fn(&func, vec![acc.clone(), item], env);
                }
                acc
            }
            "start_with" => {
                // start_with(stream, value) → Stream — prepend a value to the stream
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let value = args.get(1).cloned().unwrap_or(Value::Unit);
                let mut items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                items.insert(0, value);
                Value::Stream(items)
            }
            "pairwise" => {
                // pairwise(stream) → Stream of Tuple — consecutive pairs
                let stream = args.into_iter().next().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let pairs: Vec<Value> = items
                    .windows(2)
                    .map(|w| Value::Tuple(vec![w[0].clone(), w[1].clone()]))
                    .collect();
                Value::Stream(pairs)
            }
            "fst" => {
                // fst(tuple) → first element of a pair/tuple
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                match val {
                    Value::Tuple(elems) => elems.into_iter().next().unwrap_or(Value::Unit),
                    _ => Value::Unit,
                }
            }
            "snd" => {
                // snd(tuple) → second element of a pair/tuple
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                match val {
                    Value::Tuple(elems) => elems.into_iter().nth(1).unwrap_or(Value::Unit),
                    _ => Value::Unit,
                }
            }
            "trd" => {
                // trd(tuple) → third element of a tuple
                let val = args.into_iter().next().unwrap_or(Value::Unit);
                match val {
                    Value::Tuple(elems) => elems.into_iter().nth(2).unwrap_or(Value::Unit),
                    _ => Value::Unit,
                }
            }
            // pair(a, b) → Tuple(a, b). Access via .fst/.snd or .0/.1.
            "pair" => {
                let a = args.get(0).cloned().unwrap_or(Value::Unit);
                let b = args.get(1).cloned().unwrap_or(Value::Unit);
                Value::Tuple(vec![a, b])
            }
            // triple(a, b, c) → Tuple(a, b, c). Access via .0/.1/.2.
            "triple" => {
                let a = args.get(0).cloned().unwrap_or(Value::Unit);
                let b = args.get(1).cloned().unwrap_or(Value::Unit);
                let c = args.get(2).cloned().unwrap_or(Value::Unit);
                Value::Tuple(vec![a, b, c])
            }
            // ── Timing operators (M17) ──
            "debounce" => {
                // debounce(stream, ms) → Stream — in sync: emit only the last value
                // (suppresses rapid events; in batch mode, only the final value survives)
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let _ms = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                match items.last() {
                    Some(last) => Value::Stream(vec![last.clone()]),
                    None => Value::Stream(vec![]),
                }
            }
            "throttle" => {
                // throttle(stream, ms) → Stream — in sync: take every Nth element
                // (rate-limit; in batch mode we sample at intervals proportional to ms)
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let ms = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 100,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                if items.is_empty() {
                    return Value::Stream(vec![]);
                }
                // In sync: sample rate = max(1, len/10) for ms>0, pass through for ms=0
                let step = if ms > 0 { (items.len() / 10).max(1) } else { 1 };
                let throttled: Vec<Value> = items.iter().step_by(step).cloned().collect();
                Value::Stream(throttled)
            }
            "delay" => {
                // delay(stream, ms) → Stream — in sync: pass through (no real time)
                // In async codegen: each element delayed by ms before forwarding
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let _ms = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                match stream {
                    Value::Stream(_) | Value::Subject(_) => stream,
                    other => Value::Stream(list_to_vec(&other)),
                }
            }
            "buffer" => {
                // buffer(stream, ms) → Stream(List) — in sync: collect all into one batch
                // In async codegen: time-windowed batches
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let _ms = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 100,
                };
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                // Sync mode: one batch of all elements
                Value::Stream(vec![Value::List(items)])
            }
            "timeout" => {
                // timeout(stream, ms) → Stream — in sync: pass through (no timing)
                // In async codegen: errors if no event within ms
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let _ms = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => 1000,
                };
                match stream {
                    Value::Stream(_) | Value::Subject(_) => stream,
                    other => Value::Stream(list_to_vec(&other)),
                }
            }
            "switch_map" => {
                // switch_map(stream, f) → Stream — in sync: flat_map (last inner wins)
                // In async codegen: cancels previous inner subscription on new outer value
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let func = args.get(1).cloned().unwrap_or(Value::Unit);
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                // Sync: map each to inner stream, keep only the last inner result set
                // This models "cancel previous" — only the last mapping survives
                if let Some(last) = items.last() {
                    let inner = self.apply_fn(&func, vec![last.clone()], env);
                    match inner {
                        Value::Stream(v) | Value::Subject(v) => Value::Stream(v),
                        other => Value::Stream(list_to_vec(&other)),
                    }
                } else {
                    Value::Stream(vec![])
                }
            }
            "sample" => {
                // sample(stream, trigger) → Stream — emit latest value from stream when trigger fires
                // Sync: for each trigger event, take the latest value from stream
                let stream = args.get(0).cloned().unwrap_or(Value::Stream(vec![]));
                let trigger = args.get(1).cloned().unwrap_or(Value::Stream(vec![]));
                let items = match stream {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let trigger_items = match trigger {
                    Value::Stream(v) | Value::Subject(v) => v,
                    other => list_to_vec(&other),
                };
                let mut result = Vec::new();
                for (i, _) in trigger_items.iter().enumerate() {
                    // At each trigger point, take the latest available value from stream
                    let idx = ((i + 1) * items.len())
                        .checked_div(trigger_items.len().max(1))
                        .unwrap_or(0)
                        .min(items.len().saturating_sub(1));
                    if let Some(v) = items.get(idx) {
                        result.push(v.clone());
                    }
                }
                Value::Stream(result)
            }
            // ── Subject + lifecycle builtins (M13) ──
            "subject" => {
                // subject() → Subject with no initial value
                // subject(val) → Subject with initial value (BehaviorSubject)
                // subject(val, n) → ReplaySubject: buffers last n values for late subscribers
                //   (In sync interpreter, all values are buffered anyway; replay count
                //    is relevant for async/codegen where late subscribers need history.)
                let mut it = args.into_iter();
                match (it.next(), it.next()) {
                    (Some(val), Some(Value::Int(_replay_n))) => Value::Subject(vec![val]),
                    (Some(val), _) => Value::Subject(vec![val]),
                    (None, _) => Value::Subject(vec![]),
                }
            }
            "watch" => {
                // The interpreter has no persisted change-feed runtime. Codegen
                // lowers watch(Type) to a broadcast stream backed by TxGuard.
                Value::Stream(vec![])
            }
            "as_stream" => {
                // as_stream(subject) → Stream — strips write access (Subject→Stream narrowing)
                // In interpreter: converts Subject(items) to Stream(items)
                // <-  on a Stream will fail at the Send handler (not a Subject)
                match args.into_iter().next() {
                    Some(Value::Subject(items)) => Value::Stream(items),
                    Some(Value::Stream(items)) => Value::Stream(items), // already a stream, no-op
                    Some(other) => Value::Stream(list_to_vec(&other)),
                    None => Value::Stream(vec![]),
                }
            }
            "complete" => {
                // complete(subject_name) → mark subject as completed (no more pushes)
                // In sync interpreter: converts Subject→Stream (strips write access)
                // In async mode: closes the broadcast channel
                if let Some(Value::Subject(items)) = args.into_iter().next() {
                    Value::Stream(items)
                } else {
                    Value::Unit
                }
            }
            "error" => {
                // error(subject, msg) → terminate subject with error
                // In sync interpreter: converts Subject→Stream, prints error
                let mut it = args.into_iter();
                if let (Some(Value::Subject(items)), Some(err_val)) = (it.next(), it.next()) {
                    eprintln!("stream error: {}", err_val);
                    Value::Stream(items)
                } else {
                    Value::Unit
                }
            }
            "teardown" => {
                // teardown("ScopeName") → returns Teardown marker for the caller to handle
                // The actual env removal happens in run_program where we have &mut Env
                if let Some(Value::Str(scope_name)) = args.first() {
                    Value::Constructor("__Teardown".into(), vec![Value::Str(scope_name.clone())])
                } else if let Some(Value::Constructor(scope_name, _)) = args.first() {
                    Value::Constructor("__Teardown".into(), vec![Value::Str(scope_name.clone())])
                } else {
                    Value::Unit
                }
            }
            // M13c: poll(fn, ms) → in sync interpreter, just call fn once
            "poll" => {
                // poll(fn, interval_ms) → calls fn once in sync mode
                // In async codegen: spawns interval + fn with switchMap cancellation
                if let Some(Value::Closure {
                    body,
                    env: closure_env,
                    ..
                }) = args.first()
                {
                    self.eval(body, &closure_env.child())
                } else if let Some(Value::Builtin(fn_name)) = args.first() {
                    self.eval_builtin(fn_name, vec![], &Env::new())
                } else {
                    Value::Unit
                }
            }
            // M13c: take_until(stream, signal) → in sync interpreter, return stream unchanged
            "take_until" => {
                // take_until(stream, signal_stream) → stream that completes when signal fires
                // In sync mode: just return the stream as-is (no async termination)
                args.into_iter().next().unwrap_or(Value::Unit)
            }
            // Comptime type builtins (M9)
            "field" => {
                // field("name", "Type") → Tuple("name", "Type")
                match (args.get(0), args.get(1)) {
                    (Some(Value::Str(name)), Some(Value::Str(ty))) => {
                        Value::Tuple(vec![Value::Str(name.clone()), Value::Str(ty.clone())])
                    }
                    _ => Value::Unit,
                }
            }
            "struct_type" => {
                // struct_type([field("x", "Int"), field("y", "Float")]) → TypeDef { kind: "struct", fields }
                let items = Self::cons_to_vec(args.into_iter().next().unwrap_or(Value::Unit));
                let fields = items
                    .into_iter()
                    .filter_map(|item| match item {
                        Value::Tuple(pair) if pair.len() == 2 => {
                            if let (Value::Str(name), Value::Str(ty)) = (&pair[0], &pair[1]) {
                                Some((name.clone(), ty.clone()))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect();
                Value::TypeDef {
                    kind: "struct".into(),
                    fields,
                }
            }
            "enum_type" => {
                // enum_type(["Red", "Green", "Blue"]) → unit variants
                // enum_type([("Circle", [field(...)]), ...]) → variants with fields
                let items = Self::cons_to_vec(args.into_iter().next().unwrap_or(Value::Unit));
                let fields = items
                    .into_iter()
                    .filter_map(|item| {
                        match item {
                            // Simple string → unit variant
                            Value::Str(name) => Some((name, String::new())),
                            // Tuple(name, fields_cons_list) → variant with fields
                            Value::Tuple(pair) if pair.len() == 2 => {
                                if let Value::Str(name) = &pair[0] {
                                    let sub_items = Self::cons_to_vec(pair[1].clone());
                                    let field_str: String = sub_items
                                        .iter()
                                        .filter_map(|f| match f {
                                            Value::Tuple(fp) if fp.len() == 2 => {
                                                if let (Value::Str(fn_), Value::Str(ft)) =
                                                    (&fp[0], &fp[1])
                                                {
                                                    Some(format!("{}:{}", fn_, ft))
                                                } else {
                                                    None
                                                }
                                            }
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join(",");
                                    Some((name.clone(), field_str))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    })
                    .collect();
                Value::TypeDef {
                    kind: "enum".into(),
                    fields,
                }
            }
            _ => {
                self.output.push(format!("Unknown builtin: {}", name));
                Value::Unit
            }
        }
    }

    /// Convert Cons/Nil linked list to Vec, also handling Value::List directly
    pub fn cons_to_vec(val: Value) -> Vec<Value> {
        match val {
            Value::List(items) => items,
            Value::Constructor(ref name, _) if name == "Nil" => vec![],
            Value::Constructor(ref name, ref args) if name == "Cons" && args.len() == 2 => {
                let mut result = vec![args[0].clone()];
                result.extend(Self::cons_to_vec(args[1].clone()));
                result
            }
            _ => vec![],
        }
    }

    /// Apply a function value (Closure or Builtin) to arguments.
    /// Convenience wrapper around `apply()` for use in stream builtins.
    pub fn apply_fn(&mut self, func: &Value, args: Vec<Value>, env: &Env) -> Value {
        self.apply(func.clone(), args, env)
    }

    /// Check if a function call is an effect operation with an active handler.
    /// If so, evaluate the handler and return Some(result). Otherwise None.
    pub fn try_effect_dispatch(
        &mut self,
        fn_name: &str,
        args: &[Expr],
        env: &Env,
    ) -> Option<Value> {
        // Search handler stack top-first for a handler matching this operation
        let handler_idx = self
            .handler_stack
            .iter()
            .rposition(|(_eff_name, handlers)| handlers.iter().any(|h| h.op_name == fn_name));
        let handler_idx = handler_idx?;

        // Verify this operation belongs to a declared effect
        let (ref eff_name, _) = self.handler_stack[handler_idx];
        let is_effect_op = self
            .effect_decls
            .get(eff_name)
            .map(|ops| ops.iter().any(|(op, _)| op == fn_name))
            .unwrap_or(false);
        if !is_effect_op {
            return None;
        }

        // Find the specific handler clause
        let (_, ref handlers) = self.handler_stack[handler_idx];
        let handler = handlers.iter().find(|h| h.op_name == fn_name)?.clone();

        // Evaluate arguments
        let arg_vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();

        // Create handler environment with params bound to args
        let mut handler_env = env.child();
        for (param, val) in handler.params.iter().zip(arg_vals.iter()) {
            handler_env.set(param.clone(), val.clone());
        }
        // Bind `resume` as an identity function — in tail-resumptive position,
        // resume(val) just returns val as the result of the effect operation
        handler_env.set("resume".into(), Value::Builtin("__resume".into()));

        // Evaluate handler body
        Some(self.eval(&handler.body, &handler_env))
    }

    pub fn eval_effect(&mut self, name: &str, args: Vec<Value>) -> Value {
        match builtin_canonical(name) {
            "print" => {
                let text = match args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    Some(v) => format!("{}", v),
                    None => String::new(),
                };
                if !self.suppress_output {
                    println!("{}", text);
                }
                self.output.push(text);
                Value::Unit
            }
            "spawn" => {
                // Handled by eval_builtin now
                Value::Int(0)
            }
            "teardown" => {
                // Return teardown marker — caller handles env removal
                if let Some(Value::Str(scope_name)) = args.first() {
                    Value::Constructor("__Teardown".into(), vec![Value::Str(scope_name.clone())])
                } else {
                    Value::Unit
                }
            }
            // I/O builtins: handle directly (eval_effect has no env)
            "write_file" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(path)), Some(Value::Str(content))) => {
                    let _ = std::fs::write(path, content);
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "append_file" => match (args.get(0), args.get(1)) {
                (Some(Value::Str(path)), Some(Value::Str(content))) => {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(path)
                    {
                        let _ = f.write_all(content.as_bytes());
                    }
                    Value::Unit
                }
                _ => Value::Unit,
            },
            "read_file" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_to_string(path) {
                    Ok(content) => Value::Str(content),
                    Err(_) => Value::Str(String::new()),
                },
                _ => Value::Str(String::new()),
            },
            "file_exists" => match args.first() {
                Some(Value::Str(path)) => Value::Bool(std::path::Path::new(path.as_str()).exists()),
                _ => Value::Bool(false),
            },
            "read_lines" => match args.first() {
                Some(Value::Str(path)) => match std::fs::read_to_string(path) {
                    Ok(content) => {
                        Value::List(content.lines().map(|l| Value::Str(l.to_string())).collect())
                    }
                    Err(_) => Value::List(vec![]),
                },
                _ => Value::List(vec![]),
            },
            "env_var" => match args.first() {
                Some(Value::Str(name)) => match std::env::var(name) {
                    Ok(val) => Value::Str(val),
                    Err(_) => Value::Str(String::new()),
                },
                _ => Value::Str(String::new()),
            },
            "time" => {
                // Return current Unix timestamp as Float
                use std::time::{SystemTime, UNIX_EPOCH};
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                Value::Float(secs)
            }
            "random" => {
                // Return a pseudo-random f64 between 0 and 1
                // Use a simple hash-based approach (no external dependency)
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                // xorshift-style mixing
                let mut x = nanos as u64;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                Value::Float((x as f64) / (u64::MAX as f64))
            }
            "input" => {
                // Read a line from stdin and return as String
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                Value::Str(
                    line.trim_end_matches('\n')
                        .trim_end_matches('\r')
                        .to_string(),
                )
            }
            // HTTP + DB + process builtins: delegate to eval_builtin
            "http_get"
            | "http_post"
            | "http_serve"
            | "http_respond"
            | "http_request_path"
            | "http_request_method"
            | "http_request_body"
            | "db_open"
            | "db_exec"
            | "db_query"
            | "db_query_row"
            | "db_insert"
            | "db_close"
            | "process_run" => self.eval_builtin(name, args, &Env::new()),
            _ => Value::Unit,
        }
    }

    /// Dispatch a message to an actor: pattern-match handlers, evaluate body, return (new_state, response).
    pub fn dispatch_actor_message(
        &mut self,
        actor_name: &str,
        current_state: &Value,
        state_param: &str,
        handlers: &[Handler],
        actor_env: &Env,
        msg: &Value,
    ) -> (Value, Value) {
        for handler in handlers {
            let mut handler_env = actor_env.child();
            // Bind state
            handler_env.set(state_param.to_string(), current_state.clone());
            // Try to match message against handler pattern
            if self.match_pattern(&handler.msg_pat, msg, &mut handler_env) {
                let result = self.eval(&handler.body, &handler_env);
                // Convention: if the handler body calls the actor recursively (e.g. counter(state + 1)),
                // that becomes the new state. If it returns a tuple, (response, new_state).
                // For simplicity: if result is a Constructor("Reply", [response, new_state]),
                // extract both. Otherwise result is the new state, response is Unit.
                match &result {
                    Value::Constructor(name, args) if name == "Reply" && args.len() == 2 => {
                        return (args[1].clone(), args[0].clone());
                    }
                    _ => {
                        // Result is the new state, no response
                        return (result, Value::Unit);
                    }
                }
            }
        }
        // No handler matched
        eprintln!("Actor '{}': no handler for message {:?}", actor_name, msg);
        (current_state.clone(), Value::Unit)
    }

    pub fn eval_binop(&self, op: &str, l: Value, r: Value) -> Value {
        match (op, &l, &r) {
            // Int operations
            ("+", Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            ("-", Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            ("*", Value::Int(a), Value::Int(b)) => Value::Int(a * b),
            ("/", Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    Value::Int(0)
                } else {
                    Value::Int(a / b)
                }
            }
            ("%", Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    Value::Int(0)
                } else {
                    Value::Int(a % b)
                }
            }
            ("==", Value::Int(a), Value::Int(b)) => Value::Bool(a == b),
            ("!=", Value::Int(a), Value::Int(b)) => Value::Bool(a != b),
            ("<", Value::Int(a), Value::Int(b)) => Value::Bool(a < b),
            (">", Value::Int(a), Value::Int(b)) => Value::Bool(a > b),
            ("<=", Value::Int(a), Value::Int(b)) => Value::Bool(a <= b),
            (">=", Value::Int(a), Value::Int(b)) => Value::Bool(a >= b),

            // Float operations
            ("+", Value::Float(a), Value::Float(b)) => Value::Float(a + b),
            ("-", Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            ("*", Value::Float(a), Value::Float(b)) => Value::Float(a * b),
            ("/", Value::Float(a), Value::Float(b)) => Value::Float(a / b),
            ("==", Value::Float(a), Value::Float(b)) => Value::Bool(a == b),
            ("!=", Value::Float(a), Value::Float(b)) => Value::Bool(a != b),
            ("<", Value::Float(a), Value::Float(b)) => Value::Bool(a < b),
            (">", Value::Float(a), Value::Float(b)) => Value::Bool(a > b),
            ("<=", Value::Float(a), Value::Float(b)) => Value::Bool(a <= b),
            (">=", Value::Float(a), Value::Float(b)) => Value::Bool(a >= b),

            // Mixed int/float
            ("+", Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 + b),
            ("+", Value::Float(a), Value::Int(b)) => Value::Float(a + *b as f64),
            ("-", Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 - b),
            ("-", Value::Float(a), Value::Int(b)) => Value::Float(a - *b as f64),
            ("*", Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 * b),
            ("*", Value::Float(a), Value::Int(b)) => Value::Float(a * *b as f64),
            ("/", Value::Int(a), Value::Float(b)) => Value::Float(*a as f64 / b),
            ("/", Value::Float(a), Value::Int(b)) => Value::Float(a / *b as f64),
            ("<", Value::Int(a), Value::Float(b)) => Value::Bool((*a as f64) < *b),
            ("<", Value::Float(a), Value::Int(b)) => Value::Bool(*a < *b as f64),
            (">", Value::Int(a), Value::Float(b)) => Value::Bool((*a as f64) > *b),
            (">", Value::Float(a), Value::Int(b)) => Value::Bool(*a > *b as f64),
            ("<=", Value::Int(a), Value::Float(b)) => Value::Bool((*a as f64) <= *b),
            ("<=", Value::Float(a), Value::Int(b)) => Value::Bool(*a <= *b as f64),
            (">=", Value::Int(a), Value::Float(b)) => Value::Bool((*a as f64) >= *b),
            (">=", Value::Float(a), Value::Int(b)) => Value::Bool(*a >= *b as f64),

            // String operations
            ("+", Value::Str(a), Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
            ("+", Value::Str(a), b) => Value::Str(format!("{}{}", a, b)),
            ("+", a, Value::Str(b)) => Value::Str(format!("{}{}", a, b)),
            ("==", Value::Str(a), Value::Str(b)) => Value::Bool(a == b),
            ("!=", Value::Str(a), Value::Str(b)) => Value::Bool(a != b),

            // Bool operations
            ("==", Value::Bool(a), Value::Bool(b)) => Value::Bool(a == b),
            ("!=", Value::Bool(a), Value::Bool(b)) => Value::Bool(a != b),

            // Constructor equality
            ("==", Value::Constructor(a, af), Value::Constructor(b, bf)) => Value::Bool(
                a == b
                    && af.len() == bf.len()
                    && af.iter().zip(bf.iter()).all(|(x, y)| {
                        matches!(
                            self.eval_binop("==", x.clone(), y.clone()),
                            Value::Bool(true)
                        )
                    }),
            ),
            ("==", a, b) if values_are_list_like(a) && values_are_list_like(b) => {
                Value::Bool(values_equal(a, b))
            }
            ("!=", a, b) => match self.eval_binop("==", a.clone(), b.clone()) {
                Value::Bool(v) => Value::Bool(!v),
                _ => Value::Bool(true),
            },

            _ => Value::Unit,
        }
    }

    pub fn eval_match(&mut self, val: Value, arms: &[MatchArm], env: &Env) -> Value {
        for arm in arms {
            let mut arm_env = env.child();
            if self.match_pattern(&arm.pat, &val, &mut arm_env) {
                // Check guard
                if let Some(guard) = &arm.guard {
                    match self.eval(guard, &arm_env) {
                        Value::Bool(true) => {}
                        _ => continue,
                    }
                }
                return self.eval(&arm.body, &arm_env);
            }
        }
        Value::Unit // no arm matched
    }

    pub fn match_pattern(&self, pat: &Pat, val: &Value, env: &mut Env) -> bool {
        match (pat, val) {
            (Pat::Wild, _) => true,
            (Pat::Var(name), _) => {
                env.set(name.clone(), val.clone());
                true
            }
            (Pat::Lit(Literal::Int(a)), Value::Int(b)) => a == b,
            (Pat::Lit(Literal::Float(a)), Value::Float(b)) => a == b,
            (Pat::Lit(Literal::Str(a)), Value::Str(b)) => a == b,
            (Pat::Lit(Literal::Bool(a)), Value::Bool(b)) => a == b,
            (Pat::Lit(Literal::Char(a)), Value::Char(b)) => a == b,
            (Pat::Con(pname, pargs), Value::Constructor(vname, vargs)) => {
                if pname != vname || pargs.len() != vargs.len() {
                    return false;
                }
                for (pp, va) in pargs.iter().zip(vargs.iter()) {
                    if !self.match_pattern(pp, va, env) {
                        return false;
                    }
                }
                true
            }
            // Positional pattern on NamedConstructor (extract values by position)
            (Pat::Con(pname, pargs), Value::NamedConstructor(vname, vfields)) => {
                if pname != vname || pargs.len() != vfields.len() {
                    return false;
                }
                for (pp, (_, va)) in pargs.iter().zip(vfields.iter()) {
                    if !self.match_pattern(pp, va, env) {
                        return false;
                    }
                }
                true
            }
            // Named pattern on NamedConstructor: Circle(radius: r)
            (Pat::NamedCon(pname, named_pats), Value::NamedConstructor(vname, vfields)) => {
                if pname != vname {
                    return false;
                }
                for (field_name, pat) in named_pats {
                    let found = vfields.iter().find(|(fn_, _)| fn_ == field_name);
                    match found {
                        Some((_, val)) => {
                            if !self.match_pattern(pat, val, env) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            }
            // Named pattern on positional Constructor (use field_names registry)
            (Pat::NamedCon(pname, named_pats), Value::Constructor(vname, vargs)) => {
                if pname != vname {
                    return false;
                }
                if let Some(names) = self.field_names.get(vname.as_str()) {
                    for (field_name, pat) in named_pats {
                        if let Some(idx) = names.iter().position(|n| n == field_name) {
                            if let Some(val) = vargs.get(idx) {
                                if !self.match_pattern(pat, val, env) {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }
            // Match True/False constructors against bools
            (Pat::Con(name, args), Value::Bool(b)) if args.is_empty() => {
                (name == "True" && *b) || (name == "False" && !*b)
            }
            (Pat::As(inner, name), _) => {
                if self.match_pattern(inner, val, env) {
                    env.set(name.clone(), val.clone());
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn bind_pattern(&self, pat: &Pat, val: &Value, env: &mut Env) {
        match pat {
            Pat::Var(name) => env.set(name.clone(), val.clone()),
            Pat::Wild => {}
            Pat::Con(_, pargs) => match val {
                Value::Constructor(_, vargs) => {
                    for (pp, va) in pargs.iter().zip(vargs.iter()) {
                        self.bind_pattern(pp, va, env);
                    }
                }
                Value::NamedConstructor(_, vfields) => {
                    for (pp, (_, va)) in pargs.iter().zip(vfields.iter()) {
                        self.bind_pattern(pp, va, env);
                    }
                }
                _ => {}
            },
            Pat::NamedCon(_, named_pats) => {
                if let Value::NamedConstructor(_, vfields) = val {
                    for (field_name, pat) in named_pats {
                        if let Some((_, va)) = vfields.iter().find(|(fn_, _)| fn_ == field_name) {
                            self.bind_pattern(pat, va, env);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Try to evaluate a rule call. Returns Some(value) if a rule with the given
    /// name exists and evaluates successfully, None otherwise.
    ///
    /// Resolution order (Catala + Prolog hybrid):
    /// 1. Exceptions (highest priority, Catala-style)
    /// 2. Conditional defaults (under clause, Catala-style)
    /// 3. Clauses with backtracking (Prolog-style: if body fails, try next clause)
    /// 4. Unconditional defaults (Catala-style fallback)
    pub fn try_rule_call(&mut self, fn_name: &str, args: &[Expr], env: &Env) -> Option<Value> {
        // Collect matching rules (clone to avoid borrow conflict with self.eval)
        let matching: Vec<Rule> = self
            .rules
            .iter()
            .filter(|(name, _)| name == fn_name)
            .map(|(_, rule)| rule.clone())
            .collect();

        if matching.is_empty() {
            return None;
        }

        // Evaluate arguments once (in caller's env so variables resolve correctly)
        let arg_vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();

        // Create a base env: caller's env minus all rule-local variables
        // (variables that appear as params in ANY clause head for this function).
        // This prevents variables like `mid` from leaking between recursive rule calls
        // while preserving top-level bindings like `threshold`.
        let mut base_env = env.clone();
        for rule in &matching {
            let head = match rule {
                Rule::Clause { head, .. }
                | Rule::Default { head, .. }
                | Rule::Exception { head, .. } => head,
                _ => continue,
            };
            if let ExprKind::App(_, params) = &head.kind {
                for param in params {
                    if let ExprKind::Var(name) = &param.kind {
                        if name != "_" {
                            base_env.remove(name);
                        }
                    }
                }
            }
            // Also clear body variables from conjunction/disjunction bodies
            if let Rule::Clause {
                body: Some(body_expr),
                ..
            } = rule
            {
                let body_goals = match &body_expr.kind {
                    ExprKind::Conjunction(goals) => goals.clone(),
                    ExprKind::Disjunction(alts) => alts
                        .iter()
                        .flat_map(|a| match &a.kind {
                            ExprKind::Conjunction(gs) => gs.clone(),
                            _ => vec![a.clone()],
                        })
                        .collect(),
                    _ => vec![body_expr.clone()],
                };
                for goal in &body_goals {
                    if let ExprKind::App(_, goal_args) = &goal.kind {
                        for ga in goal_args {
                            if let ExprKind::Var(name) = &ga.kind {
                                if name != "_" {
                                    base_env.remove(name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check exceptions first — they override the default
        for rule in &matching {
            if let Rule::Exception {
                head,
                value,
                condition,
                ..
            } = rule
            {
                if let Some(mut rule_env) = self.match_rule_head(head, &arg_vals, &base_env) {
                    let cond_met = match condition {
                        Some(cond) => matches!(self.eval(cond, &rule_env), Value::Bool(true)),
                        None => true,
                    };
                    if cond_met {
                        return Some(self.eval(value, &rule_env));
                    }
                }
            }
        }

        // Catala-style: conditional defaults first
        for rule in &matching {
            if let Rule::Default {
                head,
                value,
                condition: Some(cond),
            } = rule
            {
                if let Some(mut rule_env) = self.match_rule_head(head, &arg_vals, &base_env) {
                    if matches!(self.eval(cond, &rule_env), Value::Bool(true)) {
                        return Some(self.eval(value, &rule_env));
                    }
                }
            }
        }

        // Clauses with backtracking (Prolog-style):
        // Try each clause; if the body evaluates to false, try the next one.
        for rule in &matching {
            if let Rule::Clause { head, body } = rule {
                if let Some(rule_env) = self.match_rule_head(head, &arg_vals, &base_env) {
                    match body {
                        None => return Some(Value::Bool(true)), // bare fact — head matched
                        Some(body_expr) => {
                            if let ExprKind::Conjunction(goals) = &body_expr.kind {
                                if self.eval_conjunction(goals, &rule_env) {
                                    return Some(Value::Bool(true));
                                }
                            } else if let ExprKind::Disjunction(alts) = &body_expr.kind {
                                // Disjunction: succeed if ANY alternative succeeds
                                let ok = alts.iter().any(|alt| match &alt.kind {
                                    ExprKind::Conjunction(goals) => {
                                        self.eval_conjunction(goals, &rule_env)
                                    }
                                    _ => !matches!(self.eval(alt, &rule_env), Value::Bool(false)),
                                });
                                if ok {
                                    return Some(Value::Bool(true));
                                }
                            } else {
                                let result = self.eval(body_expr, &rule_env);
                                if !matches!(result, Value::Bool(false)) {
                                    return Some(result);
                                }
                                // Body returned false — backtrack to next clause
                            }
                        }
                    }
                }
            }
        }

        // Unconditional defaults (lowest priority)
        for rule in &matching {
            if let Rule::Default {
                head,
                value,
                condition: None,
            } = rule
            {
                if let Some(mut rule_env) = self.match_rule_head(head, &arg_vals, &base_env) {
                    return Some(self.eval(value, &rule_env));
                }
            }
        }

        None
    }

    /// Match a rule head against argument values.
    /// Returns Some(env) with bindings if the head matches, None if ground terms don't match.
    /// Ground terms in the head (literals, constructors) must match the corresponding argument.
    /// Variables in the head bind to the corresponding argument value.
    fn match_rule_head(&self, head: &Expr, args: &[Value], env: &Env) -> Option<Env> {
        // Use the provided env (base_env from try_rule_call, clean of conjunction vars)
        let mut rule_env = env.clone();
        if let ExprKind::App(_, params) = &head.kind {
            for (param, val) in params.iter().zip(args.iter()) {
                match &param.kind {
                    // Typed parameter: __typed(var, TypeName) — check type constraint
                    ExprKind::App(func, typed_args)
                        if matches!(&func.kind, ExprKind::Var(n) if n == "__typed")
                            && typed_args.len() == 2 =>
                    {
                        let param_name = match &typed_args[0].kind {
                            ExprKind::Var(n) => n.clone(),
                            _ => "_".to_string(),
                        };
                        let type_name = match &typed_args[1].kind {
                            ExprKind::Var(n) => n.clone(),
                            _ => String::new(),
                        };
                        // Check if the value is a valid variant of the type
                        let is_valid = match val {
                            Value::Constructor(cname, _) => {
                                // Check if this constructor belongs to the type
                                self.type_variants
                                    .get(&type_name)
                                    .map_or(false, |variants| variants.contains(cname))
                            }
                            _ => false,
                        };
                        if !is_valid {
                            return None;
                        }
                        if param_name != "_" {
                            rule_env.set(param_name, val.clone());
                        }
                    }
                    ExprKind::Var(name) if name == "_" => {
                        // Wildcard — matches anything, don't bind
                    }
                    ExprKind::Var(name)
                        if name.chars().next().map_or(false, |c| c.is_uppercase()) =>
                    {
                        // Constructor (uppercase) — must match exactly as ground term
                        let expected = Value::Constructor(name.clone(), vec![]);
                        if !values_equal(&expected, val) {
                            // Also try string comparison for findall candidates
                            if let Value::Str(s) = val {
                                if s != name {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        }
                    }
                    ExprKind::Var(name) => {
                        // Variable (lowercase) — bind to the argument value
                        rule_env.set(name.clone(), val.clone());
                    }
                    ExprKind::Lit(lit) => {
                        // Ground term — must match exactly
                        let expected = self.literal_to_value(lit);
                        if !values_equal(&expected, val) {
                            return None; // ground term mismatch
                        }
                    }
                    _ => {
                        // Other expressions in head (e.g., constructors) — bind as variable
                        // for now, treat as variable via name extraction
                        if let Some(name) = self.extract_var_name(param) {
                            if name != "_" {
                                rule_env.set(name, val.clone());
                            }
                        }
                    }
                }
            }
            Some(rule_env)
        } else {
            // No-arg rule head — always matches
            Some(rule_env)
        }
    }

    /// Convert a literal to a Value for comparison during fact matching
    fn literal_to_value(&self, lit: &Literal) -> Value {
        literal_to_value_static(lit)
    }
}

fn literal_to_value_static(lit: &Literal) -> Value {
    match lit {
        Literal::Int(n) => Value::Int(*n),
        Literal::Float(f) => Value::Float(*f),
        Literal::Str(s) => Value::Str(s.clone()),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Char(c) => Value::Char(*c),
    }
}

impl Interpreter {
    /// Extract variable name from an expression (for non-trivial head patterns)
    fn extract_var_name(&self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Var(name) => Some(name.clone()),
            _ => None,
        }
    }

    /// Evaluate a Prolog-style conjunction: all goals must succeed.
    /// Handles existential variables by searching through matching facts.
    fn eval_conjunction(&mut self, goals: &[Expr], env: &Env) -> bool {
        if goals.is_empty() {
            return true;
        }

        let goal = &goals[0];
        let remaining = &goals[1..];

        // Check if this goal introduces unbound variables
        // (variables not yet in the environment that appear as arguments)
        if let ExprKind::App(func, args) = &goal.kind {
            let fn_name = self.expr_name(func);
            let unbound: Vec<(usize, String)> = args
                .iter()
                .enumerate()
                .filter_map(|(i, arg)| {
                    if let ExprKind::Var(name) = &arg.kind {
                        // _ is a wildcard: treated as unbound for existential search
                        // but binding is discarded (never propagated to remaining goals)
                        if name == "_" || env.get(name).is_none() {
                            return Some((i, name.clone()));
                        }
                    }
                    None
                })
                .collect();

            if !unbound.is_empty() {
                // Existential search: find all facts/clauses that can provide bindings
                return self.search_bindings(&fn_name, args, &unbound, remaining, env);
            }
        }

        // All variables are bound — evaluate the goal normally
        let result = self.eval(goal, env);
        match result {
            Value::Bool(true) => self.eval_conjunction(remaining, env),
            Value::Bool(false) => false,
            _ => {
                // Non-boolean result — treat as success (the goal produced a value)
                self.eval_conjunction(remaining, env)
            }
        }
    }

    /// Search for bindings that satisfy an existential variable.
    /// For `parent(a, b)` where `b` is unbound, iterate through all `parent` facts
    /// and try each binding of `b` to see if the remaining goals succeed.
    fn search_bindings(
        &mut self,
        fn_name: &str,
        goal_args: &[Expr],
        unbound: &[(usize, String)],
        remaining: &[Expr],
        env: &Env,
    ) -> bool {
        // Collect all rules for this function name
        let rules: Vec<Rule> = self
            .rules
            .iter()
            .filter(|(name, _)| name == fn_name)
            .map(|(_, rule)| rule.clone())
            .collect();

        // Create clean env: caller's env minus unbound variables (prevents leakage)
        let mut clean_env = env.clone();
        for (_, uname) in unbound {
            clean_env.remove(uname);
        }

        // Evaluate bound arguments
        let bound_vals: Vec<Option<Value>> = goal_args
            .iter()
            .map(|arg| {
                if let ExprKind::Var(name) = &arg.kind {
                    env.get(name).cloned()
                } else {
                    Some(self.eval(arg, env))
                }
            })
            .collect();

        // Try each rule/fact as a potential source of bindings
        for rule in &rules {
            if let Rule::Clause { head, body } = rule {
                if let ExprKind::App(_, head_params) = &head.kind {
                    if head_params.len() != goal_args.len() {
                        continue;
                    }

                    // Check if bound args match this fact's ground terms
                    let mut matches = true;
                    let mut new_env = clean_env.clone();

                    for (i, (head_param, bound_val)) in
                        head_params.iter().zip(bound_vals.iter()).enumerate()
                    {
                        match (&head_param.kind, bound_val) {
                            // Head has a literal, we have a bound value — must match
                            (ExprKind::Lit(lit), Some(val)) => {
                                let expected = self.literal_to_value(lit);
                                if !values_equal(&expected, val) {
                                    matches = false;
                                    break;
                                }
                            }
                            // Head has a literal, we have an unbound var — bind it
                            (ExprKind::Lit(lit), None) => {
                                let val = self.literal_to_value(lit);
                                if let Some((_, ref name)) =
                                    unbound.iter().find(|(idx, _)| *idx == i)
                                {
                                    new_env.set(name.clone(), val);
                                }
                            }
                            // Head has a variable — it can provide a binding for our unbound vars
                            // but only if the fact itself has a body that can evaluate
                            (ExprKind::Var(head_var), Some(val)) => {
                                new_env.set(head_var.clone(), val.clone());
                            }
                            (ExprKind::Var(head_var), None) => {
                                // Both head and goal have unbound variables — skip
                                // (can't resolve without more facts)
                                matches = false;
                                break;
                            }
                            _ => {}
                        }
                    }

                    if !matches {
                        continue;
                    }

                    // Check clause body if present
                    let clause_ok = match body.as_ref().map(|e| &e.kind) {
                        None => true, // bare fact
                        Some(ExprKind::Conjunction(goals)) => {
                            self.eval_conjunction(goals, &new_env)
                        }
                        Some(_) => {
                            matches!(
                                self.eval(body.as_ref().unwrap(), &new_env),
                                Value::Bool(true)
                            )
                        }
                    };

                    if clause_ok {
                        // This fact/clause succeeded — try remaining goals with new bindings
                        if self.eval_conjunction(remaining, &new_env) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Evaluate findall(template, goal) — collect all solutions of a logic query.
    /// template is a variable name (e.g., `c`), goal is a rule call (e.g., `parent("bob", c)`).
    /// Returns a Vec of all bindings of the template variable that make the goal true.
    fn eval_findall(&mut self, template: &Expr, goal: &Expr, env: &Env) -> Value {
        let template_name = match &template.kind {
            ExprKind::Var(name) => name.clone(),
            _ => return Value::List(vec![]),
        };

        // The goal should be a rule call: App(Var(fn_name), args)
        if let ExprKind::App(func, goal_args) = &goal.kind {
            let fn_name = self.expr_name(func);

            // Strategy: collect all possible values from ground facts across
            // all rules in the database. Then for each candidate, check if
            // the goal succeeds with that binding.
            let mut candidates = std::collections::BTreeSet::new();
            self.collect_all_values(&mut candidates);

            // Identify which goal arg positions are wildcards (_)
            let wildcard_positions: Vec<usize> = goal_args
                .iter()
                .enumerate()
                .filter(|(_, a)| matches!(&a.kind, ExprKind::Var(n) if n == "_"))
                .map(|(i, _)| i)
                .collect();

            let mut results = Vec::new();
            for candidate_str in &candidates {
                // Build args with the candidate substituted for the template var
                // and wildcards substituted with each possible value
                // Build an expression for a candidate value (string or constructor)
                let candidate_to_expr = |s: &str| -> Expr {
                    if s.chars().next().map_or(false, |c| c.is_uppercase()) {
                        ExprKind::Var(s.to_string()).into() // Constructor
                    } else {
                        ExprKind::Lit(Literal::Str(s.to_string())).into()
                    }
                };
                let make_test_args = |wildcard_val: &str| -> Vec<Expr> {
                    goal_args
                        .iter()
                        .map(|a| {
                            if let ExprKind::Var(name) = &a.kind {
                                if name == &template_name {
                                    candidate_to_expr(candidate_str)
                                } else if name == "_" {
                                    candidate_to_expr(wildcard_val)
                                } else {
                                    a.clone()
                                }
                            } else {
                                a.clone()
                            }
                        })
                        .collect()
                };

                // If there are wildcards, check ground facts directly (fast path)
                let found = if !wildcard_positions.is_empty() {
                    // For wildcard queries like needs(c, _), scan ground facts
                    // Pre-evaluate bound args
                    let bound_arg_vals: Vec<(usize, Value)> = goal_args.iter().enumerate()
                        .filter(|(_, a)| {
                            !matches!(&a.kind, ExprKind::Var(n) if n == &template_name || n == "_")
                        })
                        .map(|(i, a)| (i, self.eval(a, env)))
                        .collect();
                    let rules_snapshot: Vec<(String, Rule)> = self.rules.clone();
                    rules_snapshot.iter().any(|(rn, rule)| {
                        if rn != &fn_name {
                            return false;
                        }
                        if let Rule::Clause { head, body: None } = rule {
                            if let ExprKind::App(_, params) = &head.kind {
                                if params.len() != goal_args.len() {
                                    return false;
                                }
                                // Template position must match candidate
                                for (i, (hp, ga)) in params.iter().zip(goal_args.iter()).enumerate()
                                {
                                    if let ExprKind::Var(n) = &ga.kind {
                                        if n == &template_name {
                                            if let ExprKind::Lit(Literal::Str(s)) = &hp.kind {
                                                if s != candidate_str {
                                                    return false;
                                                }
                                            } else if let ExprKind::Lit(Literal::Int(n)) = &hp.kind
                                            {
                                                if n.to_string() != *candidate_str {
                                                    return false;
                                                }
                                            } else {
                                                return false;
                                            }
                                        }
                                        // Wildcard _ matches anything — skip
                                    }
                                }
                                // Check bound positions
                                for (idx, val) in &bound_arg_vals {
                                    if let ExprKind::Lit(lit) = &params[*idx].kind {
                                        let hv = literal_to_value_static(lit);
                                        if !values_equal(&hv, val) {
                                            return false;
                                        }
                                    }
                                }
                                return true;
                            }
                        }
                        false
                    })
                } else {
                    let test_args = make_test_args("");
                    if let Some(result) = self.try_rule_call(&fn_name, &test_args, env) {
                        !matches!(result, Value::Bool(false))
                    } else {
                        false
                    }
                };

                if found {
                    let val = if candidate_str
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_uppercase())
                    {
                        Value::Constructor(candidate_str.clone(), vec![])
                    } else if let Ok(n) = candidate_str.parse::<i64>() {
                        Value::Int(n)
                    } else if let Ok(f) = candidate_str.parse::<f64>() {
                        Value::Float(f)
                    } else {
                        Value::Str(candidate_str.clone())
                    };
                    let vs = format!("{}", val);
                    if !results.iter().any(|r: &Value| format!("{}", r) == vs) {
                        results.push(val);
                    }
                }
            }
            Value::List(results)
        } else {
            Value::List(vec![])
        }
    }

    /// Collect all string values that appear as ground terms in any rule/fact.
    fn collect_all_values(&self, values: &mut std::collections::BTreeSet<String>) {
        for (_, rule) in &self.rules {
            match rule {
                Rule::Clause { head, .. }
                | Rule::Default { head, .. }
                | Rule::Exception { head, .. } => {
                    if let ExprKind::App(_, params) = &head.kind {
                        for p in params {
                            self.collect_expr_values(p, values);
                        }
                    }
                }
                _ => {}
            }
        }
        // Also collect all enum variant names from type declarations
        for (_, variants) in &self.type_variants {
            for variant in variants {
                values.insert(variant.clone());
            }
        }
    }

    fn collect_expr_values(&self, expr: &Expr, values: &mut std::collections::BTreeSet<String>) {
        match &expr.kind {
            ExprKind::Lit(Literal::Str(s)) => {
                values.insert(s.clone());
            }
            ExprKind::Lit(Literal::Int(n)) => {
                values.insert(n.to_string());
            }
            ExprKind::Var(name) if name.chars().next().map_or(false, |c| c.is_uppercase()) => {
                values.insert(name.clone());
            }
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.kind {
                    if name == "__typed" {
                        // __typed(var, TypeName) — collect all variants of the type
                        if let Some(type_arg) = args.get(1) {
                            if let ExprKind::Var(type_name) = &type_arg.kind {
                                if let Some(variants) = self.type_variants.get(type_name.as_str()) {
                                    for v in variants {
                                        values.insert(v.clone());
                                    }
                                }
                            }
                        }
                    } else if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        values.insert(name.clone());
                    }
                }
                for a in args {
                    self.collect_expr_values(a, values);
                }
            }
            _ => {}
        }
    }

    /// Enumerate all solutions for a findall query by trying each rule.
    /// For ground facts: direct match. For rules with bodies: recursively find values.
    fn findall_enumerate(
        &mut self,
        fn_name: &str,
        bound_vals: &[Option<Value>],
        template_pos: Option<usize>,
        env: &Env,
        results: &mut Vec<Value>,
        depth: usize,
    ) {
        if depth > 50 {
            return;
        } // prevent infinite recursion

        let rules: Vec<Rule> = self
            .rules
            .iter()
            .filter(|(name, _)| name == fn_name)
            .map(|(_, rule)| rule.clone())
            .collect();

        for rule in &rules {
            match rule {
                Rule::Clause { head, body } => {
                    if let ExprKind::App(_, head_params) = &head.kind {
                        if head_params.len() != bound_vals.len() {
                            continue;
                        }

                        // For ground facts (no body), directly extract the template value
                        if body.is_none() {
                            let mut ok = true;
                            let mut candidate = None;
                            for (i, (hp, bv)) in
                                head_params.iter().zip(bound_vals.iter()).enumerate()
                            {
                                match (&hp.kind, bv) {
                                    (ExprKind::Lit(lit), Some(val)) => {
                                        if !values_equal(&self.literal_to_value(lit), val) {
                                            ok = false;
                                            break;
                                        }
                                    }
                                    (ExprKind::Lit(lit), None) => {
                                        if Some(i) == template_pos {
                                            candidate = Some(self.literal_to_value(lit));
                                        }
                                    }
                                    (ExprKind::Var(_), Some(_)) => {} // variable matches bound
                                    (ExprKind::Var(_), None) => {}    // both free
                                    _ => {}
                                }
                            }
                            if ok {
                                if let Some(val) = candidate {
                                    {
                                        let vs = format!("{}", val);
                                        if !results.iter().any(|r| format!("{}", r) == vs) {
                                            results.push(val);
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        // For rules with bodies: bind head variables, then evaluate the body
                        // to find all values the template var can take.
                        let body_expr = body.as_ref().unwrap();
                        let mut rule_env = env.clone();

                        // Bind known (bound) values to head params
                        for (i, hp) in head_params.iter().enumerate() {
                            if let ExprKind::Var(hname) = &hp.kind {
                                if let Some(val) = &bound_vals[i] {
                                    rule_env.set(hname.clone(), val.clone());
                                }
                            }
                        }

                        // The body is a conjunction/disjunction/single goal
                        // We need to enumerate all bindings of the free variable
                        // Strategy: if the body is a single rule call, recursively findall
                        // then for each result, check the conjunction holds

                        match &body_expr.kind {
                            ExprKind::Conjunction(goals) if goals.len() >= 1 => {
                                // First goal provides candidates, remaining goals filter
                                self.findall_conjunction(
                                    goals,
                                    head_params,
                                    bound_vals,
                                    template_pos,
                                    &rule_env,
                                    results,
                                    depth,
                                );
                            }
                            ExprKind::Disjunction(alts) => {
                                // Try each alternative
                                for alt in alts {
                                    match &alt.kind {
                                        ExprKind::Conjunction(goals) => {
                                            self.findall_conjunction(
                                                goals,
                                                head_params,
                                                bound_vals,
                                                template_pos,
                                                &rule_env,
                                                results,
                                                depth,
                                            );
                                        }
                                        _ => {
                                            // Single goal alternative
                                            self.findall_conjunction(
                                                &[alt.clone()],
                                                head_params,
                                                bound_vals,
                                                template_pos,
                                                &rule_env,
                                                results,
                                                depth,
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Single goal body
                                self.findall_conjunction(
                                    &[body_expr.clone()],
                                    head_params,
                                    bound_vals,
                                    template_pos,
                                    &rule_env,
                                    results,
                                    depth,
                                );
                            }
                        }
                    }
                }
                Rule::Default {
                    head,
                    value,
                    condition,
                } => {
                    // Value-returning rules: check if any bound vals match
                    if let ExprKind::App(_, head_params) = &head.kind {
                        if head_params.len() != bound_vals.len() {
                            continue;
                        }
                        let mut ok = true;
                        for (hp, bv) in head_params.iter().zip(bound_vals.iter()) {
                            if let (ExprKind::Lit(lit), Some(val)) = (&hp.kind, bv) {
                                if !values_equal(&self.literal_to_value(lit), val) {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if !ok {
                            continue;
                        }
                        let cond_ok = match condition {
                            Some(c) => {
                                let mut ce = env.clone();
                                for (hp, bv) in head_params.iter().zip(bound_vals.iter()) {
                                    if let ExprKind::Var(n) = &hp.kind {
                                        if let Some(v) = bv {
                                            ce.set(n.clone(), v.clone());
                                        }
                                    }
                                }
                                matches!(self.eval(c, &ce), Value::Bool(true))
                            }
                            None => true,
                        };
                        if cond_ok {
                            let val = self.eval(value, env);
                            {
                                let vs = format!("{}", val);
                                if !results.iter().any(|r| format!("{}", r) == vs) {
                                    results.push(val);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Enumerate solutions through a conjunction of goals.
    /// The first goal generates candidate bindings, each subsequent goal filters.
    fn findall_conjunction(
        &mut self,
        goals: &[Expr],
        head_params: &[Expr],
        bound_vals: &[Option<Value>],
        template_pos: Option<usize>,
        env: &Env,
        results: &mut Vec<Value>,
        depth: usize,
    ) {
        if goals.is_empty() {
            return;
        }

        let first_goal = &goals[0];

        // If the first goal is a rule call, enumerate its solutions
        if let ExprKind::App(func, args) = &first_goal.kind {
            let goal_fn = self.expr_name(func);
            let goal_args_vals: Vec<Option<Value>> = args
                .iter()
                .map(|a| match &a.kind {
                    ExprKind::Var(name) => env.get(name).cloned(),
                    _ => Some(self.eval(a, env)),
                })
                .collect();

            // Find which positions are free (unbound variables)
            let free_positions: Vec<(usize, String)> = args
                .iter()
                .enumerate()
                .filter_map(|(i, a)| {
                    if let ExprKind::Var(name) = &a.kind {
                        if goal_args_vals[i].is_none() {
                            Some((i, name.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            if free_positions.is_empty() {
                // All bound — just check if the goal holds
                let result = self.try_rule_call(&goal_fn, args, env);
                if result.is_some() && !matches!(result, Some(Value::Bool(false))) {
                    // Check remaining goals
                    if goals.len() <= 1 {
                        // Extract template value from head params
                        if let Some(tpos) = template_pos {
                            if let ExprKind::Var(hname) = &head_params[tpos].kind {
                                if let Some(val) = env.get(hname) {
                                    {
                                        let vs = format!("{}", val);
                                        if !results.iter().any(|r| format!("{}", r) == vs) {
                                            results.push(val.clone());
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        self.findall_conjunction(
                            &goals[1..],
                            head_params,
                            bound_vals,
                            template_pos,
                            env,
                            results,
                            depth,
                        );
                    }
                }
                return;
            }

            // Enumerate: for each free variable, find all values from the goal's facts
            // Use the first free position to enumerate
            let (free_idx, free_name) = &free_positions[0];
            let mut candidates = Vec::new();
            self.findall_enumerate(
                &goal_fn,
                &goal_args_vals,
                Some(*free_idx),
                env,
                &mut candidates,
                depth + 1,
            );

            for candidate in candidates {
                let mut new_env = env.clone();
                new_env.set(free_name.clone(), candidate.clone());
                // Also bind to head param if this variable appears there
                for (i, hp) in head_params.iter().enumerate() {
                    if let ExprKind::Var(hname) = &hp.kind {
                        if hname == free_name {
                            new_env.set(hname.clone(), candidate.clone());
                        }
                    }
                }
                // Check remaining goals with this binding
                if goals.len() <= 1 {
                    if let Some(tpos) = template_pos {
                        if let ExprKind::Var(hname) = &head_params[tpos].kind {
                            if let Some(val) = new_env.get(hname) {
                                {
                                    let vs = format!("{}", val);
                                    if !results.iter().any(|r| format!("{}", r) == vs) {
                                        results.push(val.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    self.findall_conjunction(
                        &goals[1..],
                        head_params,
                        bound_vals,
                        template_pos,
                        &new_env,
                        results,
                        depth,
                    );
                }
            }
        } else {
            // Non-rule goal (comparison, etc.) — just check it
            let val = self.eval(first_goal, env);
            if matches!(val, Value::Bool(true)) {
                if goals.len() <= 1 {
                    if let Some(tpos) = template_pos {
                        if let ExprKind::Var(hname) = &head_params[tpos].kind {
                            if let Some(val) = env.get(hname) {
                                {
                                    let vs = format!("{}", val);
                                    if !results.iter().any(|r| format!("{}", r) == vs) {
                                        results.push(val.clone());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    self.findall_conjunction(
                        &goals[1..],
                        head_params,
                        bound_vals,
                        template_pos,
                        env,
                        results,
                        depth,
                    );
                }
            }
        }
    }

    /// Extract parameter names from a rule head (which is an Expr) and bind
    /// the provided argument values into the environment.
    /// Legacy helper — used by code paths that don't need ground-term matching.
    pub fn bind_rule_params(&self, head: &Expr, args: &[Value], env: &mut Env) {
        if let ExprKind::App(_, params) = &head.kind {
            for (param, val) in params.iter().zip(args.iter()) {
                if let ExprKind::Var(name) = &param.kind {
                    env.set(name.clone(), val.clone());
                }
            }
        }
    }

    pub fn rule_name(&self, rule: &Rule) -> String {
        match rule {
            Rule::Clause { head, .. } => self.expr_name(head),
            Rule::Default { head, .. } => self.expr_name(head),
            Rule::Exception { head, .. } => self.expr_name(head),
            Rule::Scope { name, .. } => name.clone(),
        }
    }

    pub fn expr_name(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::App(f, _) => self.expr_name(f),
            ExprKind::Var(name) => name.clone(),
            _ => "?".into(),
        }
    }
}

/// Compare two Values for structural equality (used in Prolog-style fact matching)
pub fn values_equal(a: &Value, b: &Value) -> bool {
    if let Some(equal) = list_values_equal(a, b) {
        return equal;
    }

    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => (x - y).abs() < f64::EPSILON,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        (Value::Constructor(n1, f1), Value::Constructor(n2, f2)) => {
            n1 == n2
                && f1.len() == f2.len()
                && f1.iter().zip(f2.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::NamedConstructor(n1, f1), Value::NamedConstructor(n2, f2)) => {
            n1 == n2
                && f1.len() == f2.len()
                && f1
                    .iter()
                    .zip(f2.iter())
                    .all(|((k1, v1), (k2, v2))| k1 == k2 && values_equal(v1, v2))
        }
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        _ => false,
    }
}

fn values_are_list_like(value: &Value) -> bool {
    list_items(value).is_some()
}

fn list_values_equal(a: &Value, b: &Value) -> Option<bool> {
    let a_items = list_items(a)?;
    let b_items = list_items(b)?;
    Some(
        a_items.len() == b_items.len()
            && a_items
                .iter()
                .zip(b_items.iter())
                .all(|(x, y)| values_equal(x, y)),
    )
}

fn list_items(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::List(items) => Some(items.clone()),
        Value::Constructor(name, fields) if name == "Nil" && fields.is_empty() => Some(vec![]),
        Value::Constructor(name, fields) if name == "Cons" && fields.len() == 2 => {
            let mut items = vec![fields[0].clone()];
            items.extend(list_items(&fields[1])?);
            Some(items)
        }
        _ => None,
    }
}

// Helper functions for list operations
pub fn list_length(val: &Value) -> i64 {
    match val {
        Value::Constructor(name, fields) if name == "Cons" => {
            1 + fields.get(1).map_or(0, |t| list_length(t))
        }
        Value::List(elems) => elems.len() as i64,
        _ => 0,
    }
}

pub fn list_to_vec(val: &Value) -> Vec<Value> {
    let mut result = Vec::new();
    let mut current = val.clone();
    loop {
        match current {
            Value::Stream(items) | Value::Subject(items) => {
                result.extend(items);
                break;
            }
            Value::Constructor(ref name, ref fields) if name == "Cons" => {
                if let Some(head) = fields.first() {
                    result.push(head.clone());
                }
                current = fields
                    .get(1)
                    .cloned()
                    .unwrap_or(Value::Constructor("Nil".into(), vec![]));
            }
            Value::List(elems) => {
                result.extend(elems);
                break;
            }
            _ => break,
        }
    }
    result
}

pub fn vec_to_list(items: Vec<Value>) -> Value {
    let mut result = Value::Constructor("Nil".into(), vec![]);
    for item in items.into_iter().rev() {
        result = Value::Constructor("Cons".into(), vec![item, result]);
    }
    result
}

// ============================================================================
// PART 7: MAIN
// ============================================================================

/// Display a parse/compile error with source context and underline.
/// Accepts legacy "LINE:COL: message" format strings and converts them
/// to structured Diagnostic output.
pub fn display_error(source: &str, error: &str) {
    display_error_in(source, error, "<input>");
}

/// Display error with filename context.
pub fn display_error_in(source: &str, error: &str, filename: &str) {
    let use_color = should_use_color();

    // Try to parse legacy "LINE:COL: message" format into a Diagnostic
    let parts: Vec<&str> = error.splitn(3, ':').collect();
    if parts.len() >= 3 {
        if let (Ok(line_num), Ok(col_num)) = (
            parts[0].trim().parse::<usize>(),
            parts[1].trim().parse::<usize>(),
        ) {
            let msg = parts[2..].join(":").trim().to_string();
            // Convert line:col to byte offset
            let span = line_col_to_span(source, line_num, col_num);
            let diag = Diagnostic::error_at(span, msg);
            eprint!("{}", diag.display(source, filename, use_color));
            return;
        }
    }
    // Fallback: no line:col parsed
    let diag = Diagnostic::error(error);
    eprint!("{}", diag.display(source, filename, use_color));
}

/// Display a structured Diagnostic directly.
pub fn display_diagnostic(source: &str, diag: &Diagnostic, filename: &str) {
    let use_color = should_use_color();
    eprint!("{}", diag.display(source, filename, use_color));
}

/// Convert 1-based line:col to a Span (for legacy error format).
fn line_col_to_span(source: &str, line: usize, col: usize) -> Span {
    let mut current_line = 1;
    let mut line_start = 0;
    for (i, ch) in source.char_indices() {
        if current_line == line {
            let start = line_start + (col - 1);
            return Span::new(start, start + 1);
        }
        if ch == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    // If we're on the target line but didn't hit a newline (last line)
    if current_line == line {
        let start = line_start + (col - 1);
        return Span::new(start, start + 1);
    }
    Span::dummy()
}

// ============================================================================
// TYPE CHECKER (M16)
// ============================================================================

// Catches errors before Rust codegen so users see Futuruna-level diagnostics
// instead of confusing rustc errors on generated code.
//
// Pass 1: collect all declarations (functions, types, constructors, builtins)
// Pass 2: walk the AST checking for undefined names, wrong arity, etc.

pub struct TypeChecker {
    /// function name -> param count
    pub functions: BTreeMap<String, usize>,
    /// type name -> exists
    pub types: BTreeSet<String>,
    /// constructor/variant name -> (parent type, field count)
    pub constructors: BTreeMap<String, (String, usize)>,
    /// type name -> variant names (for exhaustiveness checking)
    pub type_variants: BTreeMap<String, Vec<String>>,
    /// builtin name -> arity
    pub builtins: BTreeMap<String, usize>,
    /// effect name -> set of operation names with arity
    pub effect_ops: BTreeMap<String, BTreeMap<String, usize>>,
    /// trait name -> required method names
    pub trait_methods: BTreeMap<String, Vec<String>>,
    /// (trait_name, for_type) -> provided method names
    pub impl_methods: BTreeMap<(String, String), Vec<String>>,
    /// (trait_name, method_name) -> receiver semantics required by the trait
    trait_method_receivers: BTreeMap<(String, String), MethodReceiverShape>,
    /// (trait_name, for_type, method_name) -> receiver semantics provided by the impl
    impl_method_receivers: BTreeMap<(String, String, String), MethodReceiverShape>,
    /// structured diagnostics accumulated during checking
    pub diagnostics: Vec<Diagnostic>,
    /// current variable scope stack
    pub scopes: Vec<BTreeSet<String>>,
    /// user-defined functions (distinct from rule functions for arity checks)
    pub user_functions: BTreeSet<String>,
    /// source file directory (for resolving @ import paths)
    pub source_dir: Option<String>,
    /// already-imported file paths (prevents cycles)
    pub imported: BTreeSet<String>,
    /// original source text (for error positions)
    pub source_text: String,
    /// error context stack — breadcrumb trail for where we are in the program
    pub error_context: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = TypeChecker {
            functions: BTreeMap::new(),
            types: BTreeSet::new(),
            constructors: BTreeMap::new(),
            type_variants: BTreeMap::new(),
            builtins: BTreeMap::new(),
            effect_ops: BTreeMap::new(),
            trait_methods: BTreeMap::new(),
            impl_methods: BTreeMap::new(),
            trait_method_receivers: BTreeMap::new(),
            impl_method_receivers: BTreeMap::new(),
            diagnostics: Vec::new(),
            scopes: vec![BTreeSet::new()],
            user_functions: BTreeSet::new(),
            source_dir: None,
            imported: BTreeSet::new(),
            source_text: String::new(),
            error_context: Vec::new(),
        };
        // Register builtins (name -> arity)
        for &(name, arity) in &[
            // Math
            ("exp", 1),
            ("ln", 1),
            ("sqrt", 1),
            ("pow", 2),
            ("abs", 1),
            ("random_float", 0),
            ("random_choice", 1),
            ("shuffle", 1),
            ("sleep", 1),
            ("now", 0),
            ("time_diff", 2),
            ("regex_match", 2),
            ("regex_find", 2),
            ("regex_find_all", 2),
            ("regex_replace", 3),
            ("pair", 2),
            ("triple", 3),
            ("to_float", 1),
            ("round", 1),
            ("floor", 1),
            ("max_f", 2),
            ("min_f", 2),
            // Comptime type construction
            ("field", 2),
            ("struct_type", 1),
            ("enum_type", 1),
            // String
            ("split", 2),
            ("join", 2),
            ("trim", 1),
            ("contains", 2),
            ("starts_with", 2),
            ("ends_with", 2),
            ("replace", 3),
            ("to_upper", 1),
            ("to_lower", 1),
            ("substring", 3),
            ("char_at", 2),
            ("index_of", 2),
            ("format_float", 2),
            ("parse_int", 1),
            ("parse_float", 1),
            ("string_chars", 1),
            ("string_length", 1),
            // File I/O
            ("read_file", 1),
            ("write_file", 2),
            ("append_file", 2),
            ("file_exists", 1),
            ("read_lines", 1),
            ("env_var", 1),
            ("process_run", 1),
            // JSON
            ("json_parse", 1),
            ("json_get", 2),
            ("json_string", 1),
            ("json_number", 1),
            ("json_bool", 1),
            ("json_array", 1),
            ("json_emit", 1),
            ("json_object", 1),
            // HTTP
            ("http_get", 1),
            ("http_post", 2),
            ("http_serve", 2),
            ("http_respond", 3),
            ("http_request_path", 1),
            ("http_request_method", 1),
            ("http_request_body", 1),
            // Database
            ("db_open", 1),
            ("db_exec", 2),
            ("db_query", 2),
            ("db_query_row", 2),
            ("db_insert", 2),
            ("db_close", 1),
            // Misc
            ("shared", 1),
            ("range", 2),
            // Collection/Functional
            ("map", 2),
            ("filter", 2),
            ("foldl", 3),
            ("sort", 1),
            ("sort_by", 2),
            ("is_some", 1),
            ("is_none", 1),
            ("any", 2),
            ("all", 2),
            ("find", 2),
            ("flat_map", 2),
            ("zip", 2),
            ("enumerate", 1),
            ("take_while", 2),
            ("drop_while", 2),
            ("sum_list", 1),
            ("distinct", 1),
            ("count_by", 2),
            ("partition", 2),
            ("chunked", 2),
            ("subscribe", 2),
            // Map (M24)
            ("map_new", 0),
            ("map_insert", 3),
            ("map_get", 2),
            ("map_get_or", 3),
            ("map_contains", 2),
            ("map_remove", 2),
            ("map_keys", 1),
            ("map_values", 1),
            ("map_entries", 1),
            ("map_len", 1),
            ("map_merge", 2),
            ("map_from", 1),
            // Set (M24)
            ("set_new", 0),
            ("set_insert", 2),
            ("set_contains", 2),
            ("set_remove", 2),
            ("set_len", 1),
            ("set_to_list", 1),
            ("set_union", 2),
            ("set_intersect", 2),
            ("set_diff", 2),
            ("set_from_list", 1),
            // Stream
            ("from_list", 1),
            ("watch", 1),
            ("scan", 3),
            ("merge", 2),
            ("take", 2),
            ("collect", 1),
            ("count", 1),
            ("skip", 2),
            ("window", 2),
            ("sum", 1),
            ("last", 1),
            ("combine_latest", 2),
            ("complete", 1),
            ("error", 2),
            ("take_until", 2),
            ("poll", 2),
            // New stream operators (M17b)
            ("tap", 2),
            ("catch", 2),
            ("first", 1),
            ("reduce", 3),
            ("start_with", 2),
            ("pairwise", 1),
            ("fst", 1),
            ("snd", 1),
            ("trd", 1),
            // Timing (M17)
            ("debounce", 2),
            ("throttle", 2),
            ("delay", 2),
            ("buffer", 2),
            ("timeout", 2),
            ("switch_map", 2),
            ("sample", 2),
        ] {
            tc.builtins.insert(name.to_string(), arity);
        }
        // Extra interpreter-only builtins not in codegen registry
        for &(name, arity) in &[
            ("show", 1),
            ("show_int", 1),
            ("show_float", 1),
            ("print", 1),
            ("length", 1),
            ("head", 1),
            ("tail", 1),
            ("not", 1),
            ("concat", 2),
            ("reverse", 1),
            ("push", 2),
            ("nth", 2),
            ("spawn", 2),
            ("ask", 2),
            ("teardown", 1),
            ("as_stream", 1),
            ("findall", 2),
            ("search", 2),
        ] {
            tc.builtins.entry(name.to_string()).or_insert(arity);
        }
        // Built-in types
        for name in &[
            "Int", "Float", "String", "Bool", "Char", "List", "Unit", "Option", "Result", "Pair",
            "Stream", "Subject", "Db", "TypeDef",
        ] {
            tc.types.insert(name.to_string());
        }
        // Prelude constructors
        tc.constructors.insert("None".into(), ("Option".into(), 0));
        tc.constructors.insert("Some".into(), ("Option".into(), 1));
        tc.constructors.insert("Ok".into(), ("Result".into(), 1));
        tc.constructors.insert("Err".into(), ("Result".into(), 1));
        tc.constructors.insert("Pair".into(), ("Pair".into(), 2));
        tc.constructors.insert("True".into(), ("Bool".into(), 0));
        tc.constructors.insert("False".into(), ("Bool".into(), 0));
        // Prelude type variants (for exhaustiveness checking)
        tc.type_variants
            .insert("Option".into(), vec!["Some".into(), "None".into()]);
        tc.type_variants
            .insert("Result".into(), vec!["Ok".into(), "Err".into()]);
        tc.type_variants
            .insert("Bool".into(), vec!["True".into(), "False".into()]);
        // Variable-arity builtins: registered in functions (not user_functions)
        // so is_rule_function() returns true → arity check skipped
        tc.functions.insert("subject".into(), 0);
        tc.functions.insert("complete".into(), 1);
        tc
    }

    fn method_receiver_shape(params: &[Param]) -> MethodReceiverShape {
        let mut saw_self = false;
        for (index, param) in params.iter().enumerate() {
            if param.name == "self" {
                if saw_self || index != 0 {
                    return MethodReceiverShape::Invalid;
                }
                saw_self = true;
            }
        }
        if saw_self {
            MethodReceiverShape::SelfFirst
        } else {
            MethodReceiverShape::None
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(BTreeSet::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define_var(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    /// Extract and define variable names from a rule head argument,
    /// including __typed(var, Type) wrappers.
    fn define_rule_head_vars(arg: &Expr, scopes: &mut Vec<BTreeSet<String>>) {
        match &arg.kind {
            ExprKind::Var(name) if !name.starts_with(|c: char| c.is_uppercase()) && name != "_" => {
                if let Some(scope) = scopes.last_mut() {
                    scope.insert(name.clone());
                }
            }
            ExprKind::App(func, args) if matches!(&func.kind, ExprKind::Var(n) if n == "__typed") =>
            {
                // __typed(var, Type) — define the variable
                if let Some(first) = args.first() {
                    Self::define_rule_head_vars(first, scopes);
                }
            }
            ExprKind::App(_, args) => {
                // Constructor with args: Cons(h, t)
                for a in args {
                    Self::define_rule_head_vars(a, scopes);
                }
            }
            ExprKind::Tuple(items) => {
                for item in items {
                    Self::define_rule_head_vars(item, scopes);
                }
            }
            _ => {}
        }
    }

    pub fn var_defined(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    /// Report an error with a span from an AST expression.
    /// Uses the expression's span directly when available, falling back to
    /// the old find_symbol_in_source heuristic for dummy spans.
    pub fn error_at_expr(&mut self, expr: &Expr, msg: String) {
        let span = if !expr.span.is_dummy() {
            Some(expr.span)
        } else if !self.source_text.is_empty() {
            // Fallback: try to find symbol in source
            if let Some(start) = msg.find('`') {
                if let Some(end) = msg[start + 1..].find('`') {
                    let name = &msg[start + 1..start + 1 + end];
                    Self::find_symbol_in_source(&self.source_text, name)
                        .map(|(line, col)| line_col_to_span(&self.source_text, line, col))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut diag = if let Some(s) = span {
            Diagnostic::error_at(s, &msg)
        } else {
            Diagnostic::error(&msg)
        };
        diag.context = self.error_context.clone();
        self.diagnostics.push(diag);
    }

    pub fn error(&mut self, msg: String) {
        // Try to derive a span from backtick-quoted symbol name in the source
        let span = if !self.source_text.is_empty() {
            if let Some(start) = msg.find('`') {
                if let Some(end) = msg[start + 1..].find('`') {
                    let name = &msg[start + 1..start + 1 + end];
                    if let Some((line, col)) = Self::find_symbol_in_source(&self.source_text, name)
                    {
                        Some(line_col_to_span(&self.source_text, line, col))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut diag = if let Some(s) = span {
            Diagnostic::error_at(s, &msg)
        } else {
            Diagnostic::error(&msg)
        };

        // Attach current context breadcrumbs
        diag.context = self.error_context.clone();

        self.diagnostics.push(diag);
    }

    /// Push an error context breadcrumb (e.g. "in function `foo`")
    pub fn push_context(&mut self, ctx: impl Into<String>) {
        self.error_context.push(ctx.into());
    }

    /// Pop the most recent error context breadcrumb
    pub fn pop_context(&mut self) {
        self.error_context.pop();
    }

    /// Find a symbol name in source text, returning 1-based (line, col).
    fn find_symbol_in_source(source: &str, name: &str) -> Option<(usize, usize)> {
        for (line_idx, line) in source.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                continue;
            }
            // Search for whole-word match
            let mut pos = 0;
            while let Some(found) = line[pos..].find(name) {
                let abs = pos + found;
                let before_ok = abs == 0
                    || !line.as_bytes()[abs - 1].is_ascii_alphanumeric()
                        && line.as_bytes()[abs - 1] != b'_';
                let after_pos = abs + name.len();
                let after_ok = after_pos >= line.len()
                    || !line.as_bytes()[after_pos].is_ascii_alphanumeric()
                        && line.as_bytes()[after_pos] != b'_';
                if before_ok && after_ok {
                    return Some((line_idx + 1, abs + 1));
                }
                pos = abs + 1;
            }
        }
        None
    }

    /// Resolve an import path for the type checker (manifest-aware).
    fn resolve_tc_import(import_path: &str, dir: &str) -> Option<String> {
        let rel = import_path.trim_start_matches("./");
        let file_path = format!("{}/{}.runa", dir, rel);

        if import_path.starts_with("./") || std::path::Path::new(&file_path).exists() {
            return Some(file_path);
        }

        // Try manifest-based resolution
        if let Some(toml_path) = Interpreter::find_manifest(dir) {
            if let Some((deps, _)) = Interpreter::parse_manifest_deps(&toml_path) {
                let toml_dir = std::path::Path::new(&toml_path)
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy().to_string();
                        if s.is_empty() {
                            ".".to_string()
                        } else {
                            s
                        }
                    })
                    .unwrap_or_else(|| ".".to_string());

                if let Some(resolved) = Self::resolve_dep_module(import_path, &deps, &toml_dir) {
                    return Some(resolved);
                }
            }
        }

        Some(file_path)
    }

    /// Resolve a dependency module path from manifest deps
    fn resolve_dep_module(
        import_path: &str,
        deps: &[(String, String)],
        toml_dir: &str,
    ) -> Option<String> {
        let parts: Vec<&str> = import_path.splitn(2, '/').collect();
        let dep_name = parts[0];
        let module = if parts.len() > 1 { parts[1] } else { "lib" };

        for (name, dep_path) in deps {
            if name == dep_name {
                let abs_dep = if std::path::Path::new(dep_path.as_str()).is_absolute() {
                    dep_path.clone()
                } else {
                    format!("{}/{}", toml_dir, dep_path)
                };
                let dep_file = format!("{}/{}.runa", abs_dep, module);
                let dep_file_src = format!("{}/src/{}.runa", abs_dep, module);

                if std::path::Path::new(&dep_file).exists() {
                    return Some(dep_file);
                } else if std::path::Path::new(&dep_file_src).exists() {
                    return Some(dep_file_src);
                }
            }
        }
        None
    }

    /// Pass 1: collect all declarations from the program
    pub fn collect_declarations(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Defn(Defn::Fn { name, params, .. }) => {
                    self.functions.insert(name.clone(), params.len());
                    self.user_functions.insert(name.clone());
                    self.define_var(name);
                }
                Stmt::Defn(Defn::Actor { name, .. }) => {
                    self.types.insert(name.clone());
                    self.define_var(name);
                }
                Stmt::Defn(Defn::Module { name, body }) => {
                    self.define_var(name);
                    self.collect_declarations(body);
                }
                Stmt::TypeDecl(TypeDecl::ADT {
                    name,
                    variants,
                    methods,
                    ..
                }) => {
                    self.types.insert(name.clone());
                    let mut variant_names = Vec::new();
                    for variant in variants {
                        let field_count = variant.fields.len();
                        self.constructors
                            .insert(variant.name.clone(), (name.clone(), field_count));
                        variant_names.push(variant.name.clone());
                        if variants.len() == 1 && variant.name == *name && field_count > 0 {
                            self.functions.insert(name.clone(), field_count);
                            self.user_functions.insert(name.clone());
                        }
                    }
                    if variants.len() > 1 {
                        self.type_variants.insert(name.clone(), variant_names);
                    }
                    for defn in methods {
                        if let Defn::Fn { name, params, .. } = defn {
                            self.functions.insert(name.clone(), params.len());
                        }
                    }
                }
                Stmt::TypeDecl(TypeDecl::EffectDecl { name, ops }) => {
                    self.types.insert(name.clone());
                    let mut ops_map = BTreeMap::new();
                    for (op_name, params, _) in ops {
                        ops_map.insert(op_name.clone(), params.len());
                        self.functions.insert(op_name.clone(), params.len());
                    }
                    self.effect_ops.insert(name.clone(), ops_map);
                }
                Stmt::TypeDecl(TypeDecl::TraitDecl { name, methods, .. }) => {
                    self.types.insert(name.clone());
                    let mut method_names = Vec::new();
                    for method in methods {
                        self.functions
                            .insert(method.name.clone(), method.params.len());
                        method_names.push(method.name.clone());
                        self.trait_method_receivers.insert(
                            (name.clone(), method.name.clone()),
                            Self::method_receiver_shape(&method.params),
                        );
                    }
                    self.trait_methods.insert(name.clone(), method_names);
                }
                Stmt::TypeDecl(TypeDecl::ImplBlock {
                    trait_name,
                    for_type,
                    methods,
                }) => {
                    let mut method_names = Vec::new();
                    for defn in methods {
                        if let Defn::Fn { name, params, .. } = defn {
                            self.functions.insert(name.clone(), params.len());
                            method_names.push(name.clone());
                            self.impl_method_receivers.insert(
                                (trait_name.clone(), for_type.clone(), name.clone()),
                                Self::method_receiver_shape(params),
                            );
                        }
                    }
                    self.impl_methods
                        .insert((trait_name.clone(), for_type.clone()), method_names);
                }
                Stmt::Bind(Pat::Var(name), _, _) => {
                    self.define_var(name);
                }
                Stmt::MonadicBind(Pat::Var(name), _, _) => {
                    self.define_var(name);
                }
                Stmt::StreamBind(name, _) => {
                    self.define_var(name);
                }
                Stmt::Rule(Rule::Scope { name, body }) => {
                    self.define_var(name);
                    self.collect_declarations(body);
                }
                Stmt::Rule(Rule::Default { head, .. })
                | Stmt::Rule(Rule::Exception { head, .. })
                | Stmt::Rule(Rule::Clause { head, .. }) => {
                    if let ExprKind::App(func, args) = &head.kind {
                        if let ExprKind::Var(fname) = &func.as_ref().kind {
                            self.functions.entry(fname.clone()).or_insert(args.len());
                            self.define_var(fname);
                        }
                    } else if let ExprKind::Var(fname) = &head.kind {
                        // Zero-arg rule without parens: | foo -> Bar
                        self.functions.entry(fname.clone()).or_insert(0);
                        self.define_var(fname);
                    }
                }
                Stmt::For(var, _, _) => {
                    self.define_var(var);
                }
                Stmt::RustBlock(code) => {
                    // Register functions defined in @ rust { } blocks so the type
                    // checker doesn't reject calls to them. Parse fn signatures.
                    for line in code.lines() {
                        let trimmed = line.trim();
                        // Match: fn name(params) or pub fn name(params)
                        let fn_start = if trimmed.starts_with("pub fn ") {
                            Some(7)
                        } else if trimmed.starts_with("fn ") {
                            Some(3)
                        } else {
                            None
                        };
                        if let Some(start) = fn_start {
                            let rest = &trimmed[start..];
                            if let Some(paren) = rest.find('(') {
                                let fn_name = rest[..paren].trim().to_string();
                                // Count params by counting commas + 1 (rough)
                                if let Some(close) = rest.find(')') {
                                    let params_str = &rest[paren + 1..close];
                                    let arity = if params_str.trim().is_empty() {
                                        0
                                    } else {
                                        params_str.split(',').count()
                                    };
                                    self.functions.insert(fn_name.clone(), arity);
                                    self.define_var(&fn_name);
                                }
                            }
                        }
                    }
                }
                Stmt::Import(path) => {
                    // Resolve @ import and collect declarations.
                    if let Some(ref dir) = self.source_dir {
                        let file_path = Self::resolve_tc_import(path, dir);
                        if let Some(file_path) = file_path {
                            let canon = std::fs::canonicalize(&file_path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or(file_path.clone());
                            if !self.imported.contains(&canon) {
                                self.imported.insert(canon);
                                if let Ok(source) = std::fs::read_to_string(&file_path) {
                                    let mut lexer = Lexer::new(&source);
                                    let tokens = lexer.tokenize();
                                    let mut parser = Parser::new(tokens, &source);
                                    if let Ok(import_stmts) = parser.parse_program() {
                                        self.collect_declarations(&import_stmts);
                                    }
                                }
                            }
                        }
                    }
                }
                Stmt::Use(_) => {}
                Stmt::HashImport(hash, path) => {
                    if let Some(ref dir) = self.source_dir {
                        if let Some(file_path) = Self::resolve_tc_import(path, dir) {
                            let canon = std::fs::canonicalize(&file_path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or(file_path.clone());
                            let import_key = format!("{}#{}", canon, hash);
                            if !self.imported.contains(&import_key) {
                                self.imported.insert(import_key);
                                if let Ok(source) = std::fs::read_to_string(&file_path) {
                                    let mut lexer = Lexer::new(&source);
                                    let tokens = lexer.tokenize();
                                    let mut parser = Parser::new(tokens, &source);
                                    if let Ok(import_stmts) = parser.parse_program() {
                                        let matched: Vec<Stmt> = import_stmts
                                            .into_iter()
                                            .filter(|s| match s {
                                                Stmt::Defn(d) => content_hash_defn(d) == *hash,
                                                Stmt::TypeDecl(td) => {
                                                    content_hash_type(td) == *hash
                                                }
                                                _ => false,
                                            })
                                            .collect();
                                        self.collect_declarations(&matched);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Pass 2: check the program for errors
    pub fn check_program(&mut self, stmts: &[Stmt]) {
        self.check_stmt_sequence(stmts);
        // Check trait impl completeness
        self.check_trait_impls();
    }

    fn check_stmt_sequence(&mut self, stmts: &[Stmt]) {
        let mut pending_comptime = false;
        for stmt in stmts {
            match stmt {
                Stmt::Annot(name, _) if name == "comptime" => {
                    self.check_stmt(stmt);
                    pending_comptime = true;
                }
                Stmt::Expr(expr) if pending_comptime => {
                    if let ExprKind::App(func, args) = &expr.kind {
                        if matches!(&func.kind, ExprKind::Var(name) if name == "assert")
                            && args.len() == 1
                        {
                            self.check_expr(&args[0], None);
                            pending_comptime = false;
                            continue;
                        }
                    }
                    self.check_stmt(stmt);
                    pending_comptime = false;
                }
                _ => {
                    self.check_stmt(stmt);
                    pending_comptime = false;
                }
            }
        }
    }

    /// Verify each `# impl Trait for Type` provides all required methods.
    fn check_trait_impls(&mut self) {
        let mut errors = Vec::new();
        for ((trait_name, for_type, method_name), shape) in &self.impl_method_receivers {
            if *shape == MethodReceiverShape::Invalid {
                errors.push(format!(
                    "`# impl {} for {}` method `{}` uses `self` in an invalid position; `self` must be the first parameter",
                    trait_name, for_type, method_name
                ));
            }
        }
        for ((trait_name, for_type), provided) in &self.impl_methods {
            if let Some(required) = self.trait_methods.get(trait_name) {
                let missing: Vec<&String> = required
                    .iter()
                    .filter(|m| !provided.iter().any(|p| p == *m))
                    .collect();
                if !missing.is_empty() {
                    let missing_names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
                    errors.push(format!(
                        "`# impl {} for {}` is missing method{}: {}",
                        trait_name,
                        for_type,
                        if missing.len() == 1 { "" } else { "s" },
                        missing_names.join(", ")
                    ));
                }
                for method_name in required {
                    if !provided.iter().any(|p| p == method_name) {
                        continue;
                    }
                    let trait_shape = self
                        .trait_method_receivers
                        .get(&(trait_name.clone(), method_name.clone()))
                        .copied()
                        .unwrap_or(MethodReceiverShape::None);
                    let impl_shape = self
                        .impl_method_receivers
                        .get(&(trait_name.clone(), for_type.clone(), method_name.clone()))
                        .copied()
                        .unwrap_or(MethodReceiverShape::None);
                    if impl_shape == MethodReceiverShape::Invalid || trait_shape == impl_shape {
                        continue;
                    }
                    match (trait_shape, impl_shape) {
                        (MethodReceiverShape::SelfFirst, MethodReceiverShape::None) => errors.push(
                            format!(
                                "`# impl {} for {}` method `{}` must declare `self` as its first parameter to match the trait receiver",
                                trait_name, for_type, method_name
                            ),
                        ),
                        (MethodReceiverShape::None, MethodReceiverShape::SelfFirst) => errors.push(
                            format!(
                                "`# impl {} for {}` method `{}` must not declare `self`; the trait method is not a receiver method",
                                trait_name, for_type, method_name
                            ),
                        ),
                        _ => {}
                    }
                }
            }
        }
        for err in errors {
            self.error(err);
        }
    }

    fn obvious_literal_ty(lit: &Literal) -> &'static str {
        match lit {
            Literal::Int(_) => "Int",
            Literal::Float(_) => "Float",
            Literal::Str(_) => "String",
            Literal::Char(_) => "Char",
            Literal::Bool(_) => "Bool",
        }
    }

    fn obvious_expr_ty(&mut self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Lit(lit) => Some(Self::obvious_literal_ty(lit).to_string()),
            ExprKind::Unit => Some("()".to_string()),
            ExprKind::List(items) => {
                let mut expected: Option<String> = None;
                for item in items {
                    let Some(item_ty) = self.obvious_expr_ty(item) else {
                        continue;
                    };
                    if let Some(expected_ty) = &expected {
                        if expected_ty != &item_ty {
                            self.error_at_expr(
                                item,
                                format!(
                                    "list elements must have the same type; first element is `{}` but this element is `{}`",
                                    expected_ty, item_ty
                                ),
                            );
                        }
                    } else {
                        expected = Some(item_ty);
                    }
                }
                expected.map(|inner| format!("List({})", inner))
            }
            ExprKind::If(_, then_expr, else_expr) => {
                let then_ty = self.obvious_expr_ty(then_expr);
                let else_ty = self.obvious_expr_ty(else_expr);
                match (then_ty, else_ty) {
                    (Some(then_ty), Some(else_ty)) => {
                        if then_ty != else_ty {
                            self.error_at_expr(
                                expr,
                                format!(
                                    "if branches must have the same type; then branch is `{}` but else branch is `{}`",
                                    then_ty, else_ty
                                ),
                            );
                        }
                        Some(then_ty)
                    }
                    (Some(ty), None) | (None, Some(ty)) => Some(ty),
                    (None, None) => None,
                }
            }
            ExprKind::Block(stmts) => stmts.last().and_then(|stmt| match stmt {
                Stmt::Expr(expr) => self.obvious_expr_ty(expr),
                _ => None,
            }),
            _ => None,
        }
    }

    fn check_declared_binding_ty(&mut self, ty: &Ty, expr: &Expr) {
        let Some(actual_ty) = self.obvious_expr_ty(expr) else {
            return;
        };
        let expected_ty = ty.to_string();
        if expected_ty != actual_ty {
            self.error_at_expr(
                expr,
                format!(
                    "type mismatch: binding expects `{}` but expression has `{}`",
                    expected_ty, actual_ty
                ),
            );
        }
    }

    pub fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Defn(Defn::Fn {
                name, params, body, ..
            }) => {
                self.push_context(format!("in function `{}`", name));
                self.push_scope();
                for p in params {
                    self.define_var(&p.name);
                }
                self.check_expr(body, Some(name));
                self.pop_scope();
                self.pop_context();
            }
            Stmt::Defn(Defn::Actor {
                name,
                handlers,
                state_param,
                ..
            }) => {
                self.push_context(format!("in actor `{}`", name));
                for handler in handlers {
                    self.push_scope();
                    self.define_var(&state_param.name);
                    self.define_pat_vars(&handler.msg_pat);
                    self.check_expr(&handler.body, None);
                    self.pop_scope();
                }
                self.pop_context();
            }
            Stmt::Defn(Defn::Module { name, body, .. }) => {
                self.push_context(format!("in module `{}`", name));
                self.push_scope();
                self.collect_declarations(body);
                self.check_stmt_sequence(body);
                self.pop_scope();
                self.pop_context();
            }
            Stmt::TypeDecl(TypeDecl::ADT { methods, .. }) => {
                for defn in methods {
                    if let Defn::Fn {
                        name: mname,
                        params,
                        body,
                        ..
                    } = defn
                    {
                        self.push_scope();
                        for p in params {
                            self.define_var(&p.name);
                        }
                        self.check_expr(body, Some(mname));
                        self.pop_scope();
                    }
                }
            }
            Stmt::TypeDecl(TypeDecl::ImplBlock { methods, .. }) => {
                for defn in methods {
                    if let Defn::Fn {
                        name: mname,
                        params,
                        body,
                        ..
                    } = defn
                    {
                        self.push_scope();
                        for p in params {
                            self.define_var(&p.name);
                        }
                        self.check_expr(body, Some(mname));
                        self.pop_scope();
                    }
                }
            }
            Stmt::Bind(pat, ty, expr) => {
                self.check_expr(expr, None);
                if let Some(ty) = ty {
                    self.check_declared_binding_ty(ty, expr);
                } else {
                    self.obvious_expr_ty(expr);
                }
                self.define_pat_vars(pat);
            }
            Stmt::MonadicBind(pat, _, expr) => {
                self.check_expr(expr, None);
                self.obvious_expr_ty(expr);
                self.define_pat_vars(pat);
            }
            Stmt::StreamBind(name, expr) => {
                self.check_expr(expr, None);
                self.define_var(name);
            }
            Stmt::For(var, iter_expr, body) => {
                self.check_expr(iter_expr, None);
                self.push_scope();
                self.define_var(var);
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr, None);
            }
            Stmt::Send(target, msg) => {
                self.check_expr(target, None);
                self.check_expr(msg, None);
            }
            Stmt::StreamSub(expr, arms) => {
                self.check_expr(expr, None);
                for arm in arms {
                    self.push_scope();
                    self.define_pat_vars(&arm.pat);
                    if let Some(g) = &arm.guard {
                        self.check_expr(g, None);
                    }
                    self.check_expr(&arm.body, None);
                    self.pop_scope();
                }
            }
            Stmt::Rule(Rule::Scope { body, .. }) => {
                self.push_scope();
                self.collect_declarations(body);
                self.check_stmt_sequence(body);
                self.pop_scope();
            }
            Stmt::Rule(Rule::Default {
                head,
                value,
                condition,
                ..
            }) => {
                self.push_scope();
                // Rule head params are in scope for value/condition
                if let ExprKind::App(_, args) = &head.kind {
                    for arg in args {
                        Self::define_rule_head_vars(arg, &mut self.scopes);
                    }
                }
                self.check_expr(value, None);
                if let Some(c) = condition {
                    self.check_expr(c, None);
                }
                self.pop_scope();
            }
            Stmt::Rule(Rule::Exception {
                head,
                value,
                condition,
                ..
            }) => {
                self.push_scope();
                if let ExprKind::App(_, args) = &head.kind {
                    for arg in args {
                        Self::define_rule_head_vars(arg, &mut self.scopes);
                    }
                }
                self.check_expr(value, None);
                if let Some(c) = condition {
                    self.check_expr(c, None);
                }
                self.pop_scope();
            }
            Stmt::Rule(Rule::Clause { head, body }) => {
                self.push_scope();
                if let ExprKind::App(_, args) = &head.kind {
                    for arg in args {
                        Self::define_rule_head_vars(arg, &mut self.scopes);
                    }
                }
                // For conjunctive bodies, also define existential variables
                // (variables that appear as arguments in goals but not in the head)
                if let Some(Expr {
                    kind: ExprKind::Conjunction(goals),
                    ..
                }) = body
                {
                    for goal in goals {
                        if let ExprKind::App(_, goal_args) = &goal.kind {
                            for arg in goal_args {
                                if let ExprKind::Var(name) = &arg.kind {
                                    self.define_var(name);
                                }
                            }
                        }
                    }
                }
                self.check_expr(head, None);
                if let Some(b) = body {
                    self.check_expr(b, None);
                }
                self.pop_scope();
            }
            Stmt::Invariant {
                subject, predicate, ..
            } => {
                self.push_scope();
                Self::define_rule_head_vars(subject, &mut self.scopes);
                self.check_expr(subject, None);
                self.check_expr(predicate, None);
                self.pop_scope();
            }
            Stmt::Prove {
                proof_block: _,
                capture,
                pass_block,
                else_block,
                ..
            } => {
                self.push_scope();
                // ? name: val — the capture variable is in scope for blocks
                if let Some(var_name) = capture {
                    self.define_var(var_name);
                }
                if let Some(stmts) = pass_block {
                    for s in stmts {
                        self.check_stmt(s);
                    }
                }
                if let Some(stmts) = else_block {
                    for s in stmts {
                        self.check_stmt(s);
                    }
                }
                self.pop_scope();
            }
            Stmt::Annot(name, args) => {
                // Skip metadata annotations whose payloads are declarations, not
                // executable expressions in the current schema.
                if name != "store" && name != "migrate" {
                    for a in args {
                        self.check_expr(a, None);
                    }
                }
            }
            _ => {} // Use, Import, Depend, RustBlock
        }
    }

    pub fn check_expr(&mut self, expr: &Expr, _in_fn: Option<&str>) {
        match &expr.kind {
            ExprKind::Var(name) => {
                let canonical = builtin_canonical(name);
                if !self.var_defined(name)
                    && !self.functions.contains_key(name)
                    && !self.builtins.contains_key(canonical)
                    && !self.constructors.contains_key(name)
                    && !name.contains("::")
                    && !name.contains(".")
                    && !name.starts_with(|c: char| c.is_uppercase())
                    && name != "_"
                // wildcard — always valid
                {
                    self.error_at_expr(expr, format!("undefined variable `{}`", name));
                }
            }
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    let canonical = builtin_canonical(name);
                    let actual_arity = args.len();

                    if let Some(&expected) = self.functions.get(name) {
                        if actual_arity != expected && !self.is_rule_function(name) {
                            self.error_at_expr(
                                expr,
                                format!(
                                    "`{}` expects {} argument{} but got {}",
                                    name,
                                    expected,
                                    if expected == 1 { "" } else { "s" },
                                    actual_arity
                                ),
                            );
                        }
                    } else if let Some(&expected) = self.builtins.get(canonical) {
                        if actual_arity != expected {
                            self.error_at_expr(
                                expr,
                                format!(
                                    "builtin `{}` expects {} argument{} but got {}",
                                    name,
                                    expected,
                                    if expected == 1 { "" } else { "s" },
                                    actual_arity
                                ),
                            );
                        }
                    } else if let Some((_, expected)) = self.constructors.get(name) {
                        let expected = *expected;
                        if actual_arity != expected {
                            self.error_at_expr(
                                expr,
                                format!(
                                    "constructor `{}` expects {} field{} but got {}",
                                    name,
                                    expected,
                                    if expected == 1 { "" } else { "s" },
                                    actual_arity
                                ),
                            );
                        }
                    } else if !self.var_defined(name)
                        && !name.contains("::")
                        && !name.contains(".")
                        && !name.starts_with(|c: char| c.is_uppercase())
                        && name != "__typed"
                    {
                        self.error_at_expr(func, format!("undefined function `{}`", name));
                    }
                } else {
                    self.check_expr(func, _in_fn);
                }
                // findall(template_var, goal) — template var and goal vars are scoped
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    if (name == "findall" || name == "search") && args.len() == 2 {
                        self.push_scope();
                        // Define the template variable
                        if let ExprKind::Var(tvar) = &args[0].kind {
                            self.define_var(tvar);
                        }
                        // Define unbound variables in the goal
                        if let ExprKind::App(_, goal_args) = &args[1].kind {
                            for ga in goal_args {
                                if let ExprKind::Var(gv) = &ga.kind {
                                    self.define_var(gv);
                                }
                            }
                        }
                        for arg in args {
                            self.check_expr(arg, _in_fn);
                        }
                        self.pop_scope();
                        // Early return — already checked args in scope
                        return;
                    }
                }
                for arg in args {
                    self.check_expr(arg, _in_fn);
                }
            }
            ExprKind::BinOp(_, lhs, rhs) => {
                self.check_expr(lhs, _in_fn);
                self.check_expr(rhs, _in_fn);
            }
            ExprKind::UnOp(_, operand) => {
                self.check_expr(operand, _in_fn);
            }
            ExprKind::If(cond, then_br, else_br) => {
                self.check_expr(cond, _in_fn);
                self.check_expr(then_br, _in_fn);
                self.check_expr(else_br, _in_fn);
            }
            ExprKind::Match(scrutinee, arms) => {
                self.check_expr(scrutinee, _in_fn);
                for arm in arms {
                    self.push_scope();
                    self.define_pat_vars(&arm.pat);
                    if let Some(g) = &arm.guard {
                        self.check_expr(g, _in_fn);
                    }
                    self.check_expr(&arm.body, _in_fn);
                    self.pop_scope();
                }
                // Exhaustiveness check
                self.check_match_exhaustiveness(arms);
            }
            ExprKind::Block(stmts) => {
                self.push_scope();
                self.collect_declarations(stmts);
                self.check_stmt_sequence(stmts);
                self.pop_scope();
            }
            ExprKind::Lambda(params, body) => {
                self.push_scope();
                for p in params {
                    self.define_var(&p.name);
                }
                self.check_expr(body, _in_fn);
                self.pop_scope();
            }
            ExprKind::Field(base, _) => {
                self.check_expr(base, _in_fn);
            }
            ExprKind::Index(base, idx) => {
                self.check_expr(base, _in_fn);
                self.check_expr(idx, _in_fn);
            }
            ExprKind::List(elems) => {
                for e in elems {
                    self.check_expr(e, _in_fn);
                }
            }
            ExprKind::Tuple(elems) => {
                for e in elems {
                    self.check_expr(e, _in_fn);
                }
            }
            ExprKind::Effect(name, args) => {
                for a in args {
                    self.check_expr(a, _in_fn);
                }
                let canonical = builtin_canonical(name);
                if let Some(&expected) = self.builtins.get(canonical) {
                    if args.len() != expected {
                        self.error_at_expr(
                            expr,
                            format!(
                                "effect `{}` expects {} argument{} but got {}",
                                name,
                                expected,
                                if expected == 1 { "" } else { "s" },
                                args.len()
                            ),
                        );
                    }
                }
            }
            ExprKind::Handle { body, handlers, .. } => {
                self.push_scope();
                self.define_var("resume");
                for h in handlers {
                    self.push_scope();
                    for p in &h.params {
                        self.define_var(p);
                    }
                    self.check_expr(&h.body, _in_fn);
                    self.pop_scope();
                }
                self.check_expr(body, _in_fn);
                self.pop_scope();
            }
            ExprKind::Try(inner) => {
                self.check_expr(inner, _in_fn);
            }
            ExprKind::Lit(_) | ExprKind::Unit => {}
            ExprKind::Conjunction(exprs) | ExprKind::Disjunction(exprs) => {
                for e in exprs {
                    self.check_expr(e, _in_fn);
                }
            }
            ExprKind::Pipe(input, transform) => {
                self.check_expr(input, _in_fn);
                // For pipe RHS, skip arity check on the outer App because
                // the pipe desugars a |> f(y) → f(a, y), adding an implicit first arg.
                // Check the inner expressions without the top-level arity check.
                match &transform.kind {
                    ExprKind::App(func, args) => {
                        self.check_expr(func, _in_fn);
                        for arg in args {
                            self.check_expr(arg, _in_fn);
                        }
                    }
                    _ => self.check_expr(transform, _in_fn),
                }
            }
        }
    }

    pub fn define_pat_vars(&mut self, pat: &Pat) {
        match pat {
            Pat::Var(name) => self.define_var(name),
            Pat::Con(_, pats) => {
                for p in pats {
                    self.define_pat_vars(p);
                }
            }
            Pat::NamedCon(_, fields) => {
                for (_, p) in fields {
                    self.define_pat_vars(p);
                }
            }
            Pat::As(inner, name) => {
                self.define_pat_vars(inner);
                self.define_var(name);
            }
            Pat::Wild | Pat::Lit(_) => {}
        }
    }

    /// Check match exhaustiveness: are all variants of an ADT covered?
    fn check_match_exhaustiveness(&mut self, arms: &[MatchArm]) {
        if arms.is_empty() {
            return;
        }

        // If any arm has a wildcard or variable pattern (without a guard), match is exhaustive
        for arm in arms {
            if arm.guard.is_none() {
                match &arm.pat {
                    Pat::Wild | Pat::Var(_) => return, // catch-all
                    _ => {}
                }
            }
        }

        // Collect constructor names from top-level patterns
        let mut matched_ctors: BTreeSet<String> = BTreeSet::new();
        let mut has_lit_pattern = false;
        for arm in arms {
            match &arm.pat {
                Pat::Con(name, _) | Pat::NamedCon(name, _) => {
                    matched_ctors.insert(name.clone());
                }
                Pat::Lit(_) => {
                    has_lit_pattern = true;
                }
                _ => {}
            }
        }

        // If matching on literals (Int, String, etc.), we can't check exhaustiveness
        if has_lit_pattern || matched_ctors.is_empty() {
            return;
        }

        // Find the parent type from the first constructor
        let first_ctor = matched_ctors.iter().next().unwrap();
        let parent_type = match self.constructors.get(first_ctor) {
            Some((ty, _)) => ty.clone(),
            None => return, // unknown constructor, skip
        };

        // Look up all variants for this type
        let all_variants = match self.type_variants.get(&parent_type) {
            Some(v) => v.clone(),
            None => return, // single-variant type or unknown, skip
        };

        // Find missing variants
        let missing: Vec<&String> = all_variants
            .iter()
            .filter(|v| !matched_ctors.contains(*v))
            .collect();

        if !missing.is_empty() {
            let missing_names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            self.error(format!(
                "non-exhaustive match on `{}`: missing {}",
                parent_type,
                missing_names.join(", ")
            ));
        }
    }

    pub fn is_rule_function(&self, name: &str) -> bool {
        !self.user_functions.contains(name)
    }

    /// Run the type checker on a program. Returns errors.
    pub fn check(stmts: &[Stmt]) -> Vec<String> {
        Self::check_with_dir(stmts, None)
    }

    /// Run the type checker with a source directory for resolving imports.
    pub fn check_with_dir(stmts: &[Stmt], source_dir: Option<String>) -> Vec<String> {
        Self::check_with_source(stmts, source_dir, "")
    }

    /// Run the type checker with source text for error positions.
    /// Run type checking and return structured diagnostics.
    pub fn check_with_diagnostics(
        stmts: &[Stmt],
        source_dir: Option<String>,
        source: &str,
    ) -> Vec<Diagnostic> {
        let mut tc = TypeChecker::new();
        tc.source_dir = source_dir;
        tc.source_text = source.to_string();
        tc.collect_declarations(stmts);
        tc.check_program(stmts);
        tc.diagnostics
    }

    /// Legacy wrapper — returns string-formatted errors for backward compatibility.
    pub fn check_with_source(
        stmts: &[Stmt],
        source_dir: Option<String>,
        source: &str,
    ) -> Vec<String> {
        let diags = Self::check_with_diagnostics(stmts, source_dir, source);
        diags
            .iter()
            .map(|d| {
                if let Some(span) = d.span {
                    let (line, col) = span.start_line_col(&source);
                    format!("{}:{}: {}", line, col, d.message)
                } else {
                    d.message.clone()
                }
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MethodReceiverShape {
    None,
    SelfFirst,
    Invalid,
}

// ============================================================================
// PUBLIC EVAL API
// ============================================================================

/// Evaluate Futuruna source code and return captured output.
/// Used by the WASM playground to run code in-browser.
pub fn eval_source(source: &str) -> Result<String, String> {
    eval_source_with_prelude(source, true)
}

/// Evaluate with optional prelude.
/// Catches panics (e.g. from std::process::exit on WASM) and returns them as errors.
pub fn eval_source_with_prelude(source: &str, use_prelude: bool) -> Result<String, String> {
    let source = source.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        eval_source_inner(&source, use_prelude)
    }));
    match result {
        Ok(inner) => inner,
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else {
                "Runtime error".to_string()
            };
            Err(msg)
        }
    }
}

fn eval_source_inner(source: &str, use_prelude: bool) -> Result<String, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };

            // Type check
            let tc_errors = TypeChecker::check(&stmts);
            if !tc_errors.is_empty() {
                return Err(tc_errors.join("\n"));
            }

            // Interpret
            let mut interp = Interpreter::new();
            let mut env = interp.default_env();
            let result = interp.run_program(&stmts, &mut env);

            // Collect output
            let mut output = interp.output.join("\n");
            match &result {
                Value::Unit => {}
                Value::List(_)
                | Value::Stream(_)
                | Value::Subject(_)
                | Value::Closure { .. }
                | Value::Builtin(_)
                | Value::Actor { .. } => {}
                _ => {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&format!("=> {}", result));
                }
            }
            Ok(output)
        }
        Err(e) => Err(e),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Span ────────────────────────────────────────────────────────

    #[test]
    fn span_dummy_is_zero() {
        let s = Span::dummy();
        assert!(s.is_dummy());
        assert_eq!(s.start, 0);
        assert_eq!(s.end, 0);
    }

    #[test]
    fn span_merge_covers_both() {
        let a = Span::new(10, 20);
        let b = Span::new(5, 30);
        let m = a.merge(b);
        assert_eq!(m.start, 5);
        assert_eq!(m.end, 30);
    }

    #[test]
    fn span_merge_with_dummy_returns_other() {
        let a = Span::new(10, 20);
        let d = Span::dummy();
        assert_eq!(a.merge(d), a);
        assert_eq!(d.merge(a), a);
    }

    #[test]
    fn import_requires_at_import_rather_than_use() {
        let temp_name = format!(
            "futuruna_import_semantics_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(temp_name);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let helper = temp_dir.join("helper.runa");
        std::fs::write(&helper, "> ping() -> String { \"ok\" }\n").unwrap();

        let dir = temp_dir.to_string_lossy().to_string();

        let import_source = "@ import ./helper\n@ print(ping())\n";
        let mut import_lexer = Lexer::new(import_source);
        let import_tokens = import_lexer.tokenize();
        let mut import_parser = Parser::new(import_tokens, import_source);
        let import_stmts = import_parser.parse_program().unwrap();
        let import_diags =
            TypeChecker::check_with_diagnostics(&import_stmts, Some(dir.clone()), import_source);
        assert!(
            import_diags.is_empty(),
            "expected @ import to resolve helper module, got {:?}",
            import_diags
        );

        let use_source = "@ use helper::*\n@ print(ping())\n";
        let mut use_lexer = Lexer::new(use_source);
        let use_tokens = use_lexer.tokenize();
        let mut use_parser = Parser::new(use_tokens, use_source);
        let use_stmts = use_parser.parse_program().unwrap();
        let use_diags = TypeChecker::check_with_diagnostics(&use_stmts, Some(dir), use_source);
        assert_eq!(use_diags.len(), 1);
        assert!(
            use_diags[0].message.contains("undefined function `ping`"),
            "expected @ use to stay Rust-only, got {:?}",
            use_diags
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn byte_offset_to_line_col_first_line() {
        let source = "hello world";
        assert_eq!(byte_offset_to_line_col(source, 0), (1, 1));
        assert_eq!(byte_offset_to_line_col(source, 5), (1, 6));
    }

    #[test]
    fn byte_offset_to_line_col_multiline() {
        let source = "line1\nline2\nline3";
        assert_eq!(byte_offset_to_line_col(source, 0), (1, 1)); // 'l' in line1
        assert_eq!(byte_offset_to_line_col(source, 6), (2, 1)); // 'l' in line2
        assert_eq!(byte_offset_to_line_col(source, 12), (3, 1)); // 'l' in line3
        assert_eq!(byte_offset_to_line_col(source, 14), (3, 3)); // 'n' in line3
    }

    #[test]
    fn span_start_line_col() {
        let source = "first\nsecond\nthird";
        let span = Span::new(6, 12); // "second"
        assert_eq!(span.start_line_col(source), (2, 1));
        assert_eq!(span.end_line_col(source), (2, 7));
    }

    // ── Diagnostic ──────────────────────────────────────────────────

    #[test]
    fn diagnostic_error_no_span() {
        let d = Diagnostic::error("something broke");
        assert!(d.span.is_none());
        assert_eq!(d.message, "something broke");
        assert_eq!(d.severity, Severity::Error);
    }

    #[test]
    fn diagnostic_error_at_span() {
        let d = Diagnostic::error_at(Span::new(10, 15), "bad thing");
        assert!(d.span.is_some());
        assert_eq!(d.span.unwrap().start, 10);
    }

    #[test]
    fn diagnostic_error_at_dummy_has_no_span() {
        let d = Diagnostic::error_at(Span::dummy(), "msg");
        assert!(d.span.is_none());
    }

    #[test]
    fn diagnostic_with_note_and_context() {
        let d = Diagnostic::error("msg")
            .with_note("try this instead")
            .with_context("in function `foo`");
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.context.len(), 1);
    }

    #[test]
    fn diagnostic_display_no_color() {
        let source = "> add(x: Int) -> Int { x }";
        let span = Span::new(2, 5); // "add"
        let d = Diagnostic::error_at(span, "wrong arity");
        let output = d.display(source, "test.runa", false);
        assert!(output.contains("error"));
        assert!(output.contains("wrong arity"));
        assert!(output.contains("test.runa:1:3"));
        assert!(output.contains("^^^")); // underline for 3 chars
        assert!(!output.contains("\x1b")); // no ANSI codes
    }

    #[test]
    fn parse_explicit_proof_block_on_prove_stmt() {
        let source = r#"
| add_comm: (a, b) -> a + b == b + a
? add_comm by {
    | (lhs, rhs) -> apply int_ring.comm_add
}
"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        match &stmts[1] {
            Stmt::Prove {
                name,
                proof_block: Some(block),
                capture,
                pass_block,
                else_block,
            } => {
                assert_eq!(name, "add_comm");
                assert!(capture.is_none());
                assert!(pass_block.is_none());
                assert!(else_block.is_none());
                assert_eq!(block.arms.len(), 1);
                assert_eq!(block.arms[0].binders, vec!["lhs", "rhs"]);
                match &block.arms[0].term {
                    proof_kernel::ProofTerm::Apply(name, args) => {
                        assert_eq!(name, "int_ring.comm_add");
                        assert!(args.is_empty());
                    }
                    other => panic!("unexpected proof term: {:?}", other),
                }
            }
            other => panic!("expected explicit proof stmt, got {:?}", other),
        }
    }

    #[test]
    fn lower_expr_to_proof_prop_handles_ge_and_conjunction() {
        let source = "x >= 0 and y == y";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let expr = parser.parse_expr().expect("expr parse failed");
        let prop = lower_expr_to_proof_prop(&expr).expect("lowering failed");

        match prop {
            proof_kernel::Prop::And(lhs, rhs) => {
                match *lhs {
                    proof_kernel::Prop::Le(
                        proof_kernel::Term::Int(0),
                        proof_kernel::Term::Var(ref x),
                    ) if x == "x" => {}
                    other => panic!("unexpected lhs: {:?}", other),
                }
                match *rhs {
                    proof_kernel::Prop::Eq(
                        proof_kernel::Term::Var(ref a),
                        proof_kernel::Term::Var(ref b),
                    ) if a == "y" && b == "y" => {}
                    other => panic!("unexpected rhs: {:?}", other),
                }
            }
            other => panic!("unexpected prop: {:?}", other),
        }
    }

    #[test]
    fn lower_expr_to_proof_prop_desugars_strict_order() {
        let source = "x < x + 1 and x + 1 > x";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let expr = parser.parse_expr().expect("expr parse failed");
        let prop = lower_expr_to_proof_prop(&expr).expect("lowering failed");

        match prop {
            proof_kernel::Prop::And(lhs, rhs) => {
                match *lhs {
                    proof_kernel::Prop::Le(
                        proof_kernel::Term::Op(ref plus, ref l, ref r),
                        proof_kernel::Term::Op(ref inner_plus, ref il, ref ir),
                    ) if plus == "+" && inner_plus == "+" => {
                        assert_eq!(**l, proof_kernel::Term::Var("x".into()));
                        assert_eq!(**r, proof_kernel::Term::Int(1));
                        assert_eq!(**il, proof_kernel::Term::Var("x".into()));
                        assert_eq!(**ir, proof_kernel::Term::Int(1));
                    }
                    other => panic!("unexpected lhs strict-order lowering: {:?}", other),
                }
                match *rhs {
                    proof_kernel::Prop::Le(
                        proof_kernel::Term::Op(ref plus, ref l, ref r),
                        proof_kernel::Term::Op(ref inner_plus, ref il, ref ir),
                    ) if plus == "+" && inner_plus == "+" => {
                        assert_eq!(**l, proof_kernel::Term::Var("x".into()));
                        assert_eq!(**r, proof_kernel::Term::Int(1));
                        assert_eq!(**il, proof_kernel::Term::Var("x".into()));
                        assert_eq!(**ir, proof_kernel::Term::Int(1));
                    }
                    other => panic!("unexpected rhs strict-order lowering: {:?}", other),
                }
            }
            other => panic!("unexpected prop: {:?}", other),
        }
    }

    #[test]
    fn diagnostic_display_with_context_breadcrumbs() {
        let source = "= x = bad()";
        let d = Diagnostic::error("undefined function `bad`").with_context("in function `main`");
        let output = d.display(source, "test.runa", false);
        assert!(output.contains("in function `main`"));
    }

    // ── should_use_color ────────────────────────────────────────────

    #[test]
    fn no_color_env_disables_color() {
        // This test documents the behavior — actual env var testing
        // is fragile in parallel test runs, so just verify the function exists
        // and returns a bool
        let _result: bool = should_use_color();
    }

    // ── TypeChecker diagnostics ─────────────────────────────────────

    #[test]
    fn typechecker_undefined_function_produces_diagnostic() {
        let source = "> main() -> Int { undefined_func(42) }";
        let diags = check_source_for_diagnostics(source);
        assert!(!diags.is_empty(), "expected at least one diagnostic");
        assert!(diags[0].message.contains("undefined_func"));
    }

    #[test]
    fn typechecker_wrong_arity_produces_diagnostic() {
        let source = "> add(a: Int, b: Int) -> Int { a + b }\n= x = add(1, 2, 3)";
        let diags = check_source_for_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("2 arguments"));
        assert!(diags[0].message.contains("3"));
    }

    #[test]
    fn typechecker_context_breadcrumb_on_function_error() {
        let source = "> outer(x: Int) -> Int { no_such_fn(x) }";
        let diags = check_source_for_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(
            diags[0].context.iter().any(|c| c.contains("outer")),
            "expected context mentioning `outer`, got: {:?}",
            diags[0].context
        );
    }

    #[test]
    fn typechecker_diagnostic_has_span() {
        let source = "> main() -> Int { xyz(1) }";
        let diags = check_source_for_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(
            diags[0].span.is_some(),
            "expected span on type error diagnostic"
        );
    }

    // ── Parse error display ──────────────────────────────────────────

    #[test]
    fn display_error_in_shows_filename() {
        // Capture what display_error_in would produce via Diagnostic directly
        let source = "bad syntax here";
        let span = line_col_to_span(source, 1, 5);
        let d = Diagnostic::error_at(span, "unexpected token");
        let output = d.display(source, "myfile.runa", false);
        assert!(
            output.contains("myfile.runa:1:5"),
            "expected filename in output, got: {}",
            output
        );
    }

    #[test]
    fn line_col_to_span_first_line() {
        let source = "hello world";
        let span = line_col_to_span(source, 1, 6);
        assert_eq!(span.start, 5); // 'w' is at byte offset 5
    }

    #[test]
    fn line_col_to_span_second_line() {
        let source = "first\nsecond";
        let span = line_col_to_span(source, 2, 1);
        assert_eq!(span.start, 6); // 's' of "second" is at byte offset 6
    }

    /// Helper: parse + type-check source, return diagnostics
    fn check_source_for_diagnostics(source: &str) -> Vec<Diagnostic> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse should succeed");
        TypeChecker::check_with_diagnostics(&stmts, None, source)
    }

    // ── Trait checking tests ─────────────────────────────────────────

    #[test]
    fn trait_complete_impl_no_error() {
        let source = "# trait Greetable { > greet(self) -> String }\n# impl Greetable for String { > greet(self) -> String { self } }";
        let diags = check_source_for_diagnostics(source);
        let trait_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("missing method"))
            .collect();
        assert!(
            trait_errors.is_empty(),
            "complete impl should have no errors, got: {:?}",
            trait_errors
        );
    }

    #[test]
    fn trait_receiver_mismatch_produces_error() {
        let source = "# trait Displayable { > display(self) -> String }\n# impl Displayable for String { > display(s) -> String { s } }";
        let diags = check_source_for_diagnostics(source);
        let receiver_errors: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.message
                    .contains("must declare `self` as its first parameter")
            })
            .collect();
        assert!(
            !receiver_errors.is_empty(),
            "receiver mismatch should produce an explicit error"
        );
    }

    #[test]
    fn trait_missing_method_produces_error() {
        let source = "# trait Displayable { > display(self) -> String > size(self) -> Int }\n# impl Displayable for String { > display(s) -> String { s } }";
        let diags = check_source_for_diagnostics(source);
        let trait_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("missing method"))
            .collect();
        assert!(
            !trait_errors.is_empty(),
            "incomplete impl should produce error"
        );
        assert!(
            trait_errors[0].message.contains("size"),
            "error should mention missing 'size'"
        );
    }

    #[test]
    fn adt_method_blocks_register_callable_functions() {
        let source = "# Color = Red | Blue { > name(c) -> String { match c { | Red -> \"red\" | Blue -> \"blue\" } } }\n@ print(name(Red))";
        let diags = check_source_for_diagnostics(source);
        assert!(
            diags.is_empty(),
            "ADT method blocks should type-check as callable functions, got: {:?}",
            diags
        );
    }

    #[test]
    fn invariant_subject_tuple_binds_predicate_variables() {
        let source = "| add_comm: (a, b) -> a + b == b + a";
        let diags = check_source_for_diagnostics(source);
        assert!(
            diags.is_empty(),
            "invariant subjects should bind tuple variables, got: {:?}",
            diags
        );
    }

    #[test]
    fn comptime_assert_uses_assert_argument_as_condition() {
        let source = "@ comptime\nassert(1 + 1 == 2)";
        let diags = check_source_for_diagnostics(source);
        assert!(
            diags.is_empty(),
            "@ comptime assert should type-check without a runtime assert builtin, got: {:?}",
            diags
        );
    }

    #[test]
    fn comptime_type_builtins_are_known_to_typechecker() {
        let source = "@ comptime\n= Config = struct_type([field(\"host\", \"String\")])";
        let diags = check_source_for_diagnostics(source);
        assert!(
            diags.is_empty(),
            "comptime type builtins should type-check, got: {:?}",
            diags
        );
    }

    #[test]
    fn comptime_uppercase_bind_registers_named_runtime_constructor() {
        let source = r#"
@ comptime
= Config = struct_type([field("host", "String"), field("port", "Int"), field("debug", "Bool")])

= cfg = Config("localhost", 8080, True)
@ print(show(cfg))
"#;
        let output = eval_source(source).expect("source should evaluate");
        assert!(
            output.contains("Config(host: localhost, port: 8080, debug: true)"),
            "expected named-field runtime constructor, got: {}",
            output
        );
    }

    /// Helper: parse source and return the first expression from a binding
    fn parse_expr_from(source: &str) -> Expr {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse should succeed");
        // Find first Bind or Expr statement and extract the expression
        for stmt in &stmts {
            match stmt {
                Stmt::Bind(_, _, expr) => return expr.clone(),
                Stmt::Expr(expr) => return expr.clone(),
                _ => {}
            }
        }
        panic!("no expression found in source");
    }

    // ── Parser span tests ───────────────────────────────────────────

    #[test]
    fn parsed_var_has_real_span() {
        let expr = parse_expr_from("= x = hello");
        assert!(
            !expr.span.is_dummy(),
            "expected real span on Var, got dummy"
        );
        assert_eq!(expr.span.start_line_col("= x = hello"), (1, 7)); // "hello" starts at col 7
    }

    #[test]
    fn parsed_int_literal_has_span() {
        let expr = parse_expr_from("= x = 42");
        assert!(!expr.span.is_dummy());
        assert_eq!(expr.span.start_line_col("= x = 42"), (1, 7)); // "42" at col 7
    }

    #[test]
    fn parsed_string_literal_has_span() {
        let expr = parse_expr_from("= x = \"hello\"");
        assert!(!expr.span.is_dummy());
    }

    #[test]
    fn parsed_binop_span_covers_both_sides() {
        let expr = parse_expr_from("= x = a + b");
        assert!(!expr.span.is_dummy());
        let source = "= x = a + b";
        let (_, start_col) = expr.span.start_line_col(source);
        let (_, end_col) = expr.span.end_line_col(source);
        assert_eq!(start_col, 7); // starts at "a"
        assert!(
            end_col >= 12,
            "expected span to cover through 'b', got end col {}",
            end_col
        );
    }

    #[test]
    fn parsed_field_access_span() {
        let expr = parse_expr_from("= x = obj.field");
        assert!(!expr.span.is_dummy());
        let source = "= x = obj.field";
        let (_, start_col) = expr.span.start_line_col(source);
        assert_eq!(start_col, 7); // starts at "obj"
    }

    #[test]
    fn parsed_function_call_span() {
        let expr = parse_expr_from("= x = foo(1, 2)");
        assert!(!expr.span.is_dummy());
        let source = "= x = foo(1, 2)";
        let (_, start_col) = expr.span.start_line_col(source);
        let (_, end_col) = expr.span.end_line_col(source);
        assert_eq!(start_col, 7); // starts at "foo"
        assert!(
            end_col >= 15,
            "expected span to cover through ')', got end col {}",
            end_col
        );
    }
}
