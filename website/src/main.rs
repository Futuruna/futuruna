use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use pulldown_cmark::{html, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const CSS: Asset = asset!("../assets/main.css");
const LOGO: Asset = asset!("../assets/logo.svg");
const FAVICON: Asset = asset!("../assets/favicon.svg");

fn main() {
    dioxus::launch(App);
}

// ============================================================================
// Router
// ============================================================================

#[derive(Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Shell)]
        #[route("/")]
        Home {},
        #[route("/playground")]
        PlaygroundPage {},
        #[route("/docs")]
        DocsPage {},
        #[route("/why")]
        WhyPage {},
        #[route("/research")]
        ResearchIndex {},
        #[route("/research/philosophy")]
        ResearchPhilosophy {},
        #[route("/research/optimization")]
        ResearchOptimization {},
        #[route("/research/danish-constitution")]
        ResearchDanishConstitution {},
        #[route("/research/danish-constitution-audit")]
        ResearchDanishConstitutionAudit {},
        #[route("/research/personskatteloven")]
        ResearchPersonskatteloven {},
        #[route("/research/us-constitution")]
        ResearchUSConstitution {},
        #[route("/research/ownership")]
        ResearchOwnership {},
    #[end_layout]
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}

#[component]
fn App() -> Element {
    rsx! { Router::<Route> {} }
}

/// Shared shell: head elements, nav, outlet, footer.
#[component]
fn Shell() -> Element {
    // Inject the doc tooltip system once on mount
    use_effect(move || {
        let db_js = doc_db_js();
        let setup_js = format!(
            r#"
            {db_js}
            (function() {{
                var tip = document.getElementById('doc-tooltip');
                if (!tip) return;
                var nameEl  = document.getElementById('doc-tip-name');
                var runeEl  = document.getElementById('doc-tip-rune');
                var bodyEl  = document.getElementById('doc-tip-body');
                var timer   = null;
                var lastId  = '';

                document.addEventListener('mousemove', function(e) {{
                    if (timer) return;
                    timer = setTimeout(function() {{ timer = null; }}, 50);

                    var docId = null;
                    // Fast path: check target + parents (works for non-editor code)
                    var el = e.target;
                    for (var d = 0; d < 6 && el && el !== document.body; d++) {{
                        if (el.dataset && el.dataset.doc) {{ docId = el.dataset.doc; break; }}
                        el = el.parentElement;
                    }}
                    // Slow path: inside .editor-layer the textarea covers spans.
                    // Temporarily enable pointer-events on the highlight layer,
                    // use elementsFromPoint to see through the textarea, then restore.
                    if (!docId) {{
                        var ed = e.target.closest && e.target.closest('.editor-layer');
                        if (ed) {{
                            var hl = ed.querySelector('.editor-highlight');
                            if (hl) {{
                                hl.style.pointerEvents = 'auto';
                                var els = document.elementsFromPoint(e.clientX, e.clientY);
                                hl.style.pointerEvents = '';
                                for (var j = 0; j < els.length; j++) {{
                                    if (els[j].dataset && els[j].dataset.doc) {{
                                        docId = els[j].dataset.doc;
                                        break;
                                    }}
                                }}
                            }}
                        }}
                    }}

                    if (!docId) {{
                        if (lastId) {{ tip.classList.remove('visible'); lastId = ''; }}
                        return;
                    }}

                    var entry = window.__FDOCS[docId];
                    if (!entry) {{ tip.classList.remove('visible'); lastId = ''; return; }}

                    if (docId !== lastId) {{
                        lastId = docId;
                        runeEl.textContent = entry.r;
                        runeEl.className   = 'doc-tip-rune ' + entry.c;
                        // Hide name if it's just the rune symbol repeated
                        if (entry.n === entry.r) {{
                            nameEl.textContent = '';
                            nameEl.style.display = 'none';
                        }} else {{
                            nameEl.textContent = entry.n;
                            nameEl.style.display = '';
                        }}
                        bodyEl.textContent = entry.d;
                    }}

                    // Position near cursor, keep on screen
                    var tx = e.clientX + 14;
                    var ty = e.clientY + 18;
                    var tw = tip.offsetWidth  || 300;
                    var th = tip.offsetHeight || 48;
                    if (tx + tw > window.innerWidth  - 12) tx = e.clientX - tw - 8;
                    if (ty + th > window.innerHeight - 12) ty = e.clientY - th - 8;
                    if (tx < 4) tx = 4;
                    if (ty < 4) ty = 4;
                    tip.style.left = tx + 'px';
                    tip.style.top  = ty + 'px';
                    tip.classList.add('visible');
                }});
            }})();
        "#,
            db_js = db_js
        );
        dioxus::document::eval(&setup_js);
    });

    rsx! {
        document::Link { rel: "stylesheet", href: CSS }
        document::Link { rel: "icon", href: FAVICON }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600;700&family=Inter:wght@300;400;600;700&display=swap"
        }
        Nav {}
        Outlet::<Route> {}
        Footer {}
        // Global doc tooltip (positioned fixed, JS-driven)
        div { id: "doc-tooltip", class: "doc-tooltip",
            div { class: "doc-tip-header",
                span { id: "doc-tip-rune", class: "doc-tip-rune" }
                span { id: "doc-tip-name", class: "doc-tip-name" }
            }
            div { id: "doc-tip-body", class: "doc-tip-body" }
        }
    }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    rsx! {
        document::Title { "Page Not Found — Futuruna" }
        section { class: "hero",
            div { class: "hero-inner",
                h1 { class: "hero-title", "404" }
                p { class: "hero-subtitle", "Page not found." }
                div { class: "hero-actions",
                    a { class: "btn btn-primary", href: "/", "Back home" }
                }
            }
        }
    }
}

// ============================================================================
// Home — the landing page
// ============================================================================

#[component]
fn Home() -> Element {
    rsx! {
        document::Title { "Futuruna - Law Programming" }
        document::Meta { name: "description", content: "Futuruna is a programming language for expressing, running, testing, and auditing laws, contracts, policies, and ordinary programs in one execution space." }
        Hero {}
        Discovery {}
        AiGuide {}
        RunesShowcase {}
        Pitch {}
        CodeExample {}
        Playground {}
    }
}

// ============================================================================
// Navigation
// ============================================================================

#[component]
fn Nav() -> Element {
    let route = use_route::<Route>();
    let active = |path: &str| -> &str {
        let r = format!("{}", route);
        if r == path || (path != "/" && r.starts_with(path)) {
            "nav-link active"
        } else {
            "nav-link"
        }
    };

    rsx! {
        nav { class: "top-nav",
            a { class: "nav-logo", href: "/",
                img { src: LOGO, alt: "Futuruna", width: "28", height: "28" }
            }
            a { class: active("/why"), href: "/why", "Why" }
            a { class: "nav-link", href: "/#ai-guide", "AI Guide" }
            a { class: active("/research"), href: "/research", "Research" }
            a { class: active("/docs"), href: "/docs", "Docs" }
            a { class: active("/playground"), href: "/playground", "Playground" }
            a { class: "nav-link", href: "https://github.com/Futuruna/futuruna", "GitHub" }
        }
    }
}

// ============================================================================
// Hero Section
// ============================================================================

#[component]
fn Hero() -> Element {
    rsx! {
        section { class: "hero",
            div { class: "hero-inner",
                div { class: "hero-runes",
                    span { "data-doc": "rune_hash", class: "hl-rune-hash", "#" }
                    " "
                    span { "data-doc": "rune_gt", class: "hl-rune-gt", ">" }
                    " "
                    span { "data-doc": "rune_pipe", class: "hl-rune-pipe", "|" }
                    " "
                    span { "data-doc": "rune_eq", class: "hl-rune-eq", "=" }
                    " "
                    span { "data-doc": "rune_tilde", class: "hl-rune-tilde", "~" }
                    " "
                    span { "data-doc": "rune_at", class: "hl-rune-at", "@" }
                    " "
                    span { "data-doc": "rune_question", class: "hl-rune-question", "?" }
                }
                h1 { class: "hero-title",
                    span { class: "hero-f-wrap",
                        span { class: "hero-f-hidden", "F" }
                        img { src: LOGO, alt: "F", class: "fehu-glyph" }
                    }
                    "uturuna"
                }
                p { class: "hero-clarifier",
                    "A programming language for law."
                }
                p { class: "hero-optimal",
                    "Write laws, contracts, and policies you can run, test, and audit."
                }
                div { class: "hero-actions",
                    a { class: "btn btn-primary", href: "#ai-guide", "AI Setup" }
                    a { class: "btn btn-secondary", href: "/why", "Learn More" }
                    a { class: "btn btn-tertiary", href: "/playground", "Try it yourself" }
                }
            }
        }
    }
}

// ============================================================================
// Discovery
// ============================================================================

#[component]
fn Discovery() -> Element {
    rsx! {
        section { class: "discovery-section",
            div { class: "discovery-text",
                p {
                    "Futuruna brings "
                    strong { "formal rules and ordinary programming" }
                    " into one execution space. Model definitions, defaults, conditions, "
                    "exceptions, calculations, and effects, then run and audit them together."
                }
                a { class: "discovery-link", href: "/why", "Why Futuruna \u{2192}" }
            }
        }
    }
}

// ============================================================================
// AI Guide
// ============================================================================

#[component]
fn AiGuide() -> Element {
    rsx! {
        section { id: "ai-guide", class: "ai-guide-section",
            div { class: "ai-guide-shell",
                p { class: "ai-guide-kicker", "AI Guide" }
                h2 { class: "section-title", "How to get your AI working with Futuruna" }
                p { class: "section-desc",
                    "Ask your Claude Code, Codex, Claude Cowork, or AI system to:"
                }
                ol { class: "ai-guide-steps",
                    li { class: "ai-guide-step",
                        span { class: "ai-guide-step-index", "1" }
                        h3 { "Download and install Futuruna" }
                        p {
                            "Use the public bootstrap script to clone the project, build "
                            code { "runa" }
                            ", and run the weather demo smoke test."
                        }
                        code { class: "ai-guide-command",
                            "curl -fsSL https://futuruna.com/ai-bootstrap.sh | bash"
                        }
                        a {
                            class: "ai-guide-link",
                            href: "https://futuruna.com/ai-bootstrap.sh",
                            "https://futuruna.com/ai-bootstrap.sh"
                        }
                    }
                    li { class: "ai-guide-step",
                        span { class: "ai-guide-step-index", "2" }
                        h3 { "Integrate the Futuruna skill" }
                        p {
                            "Take the "
                            code { "Using Futuruna" }
                            " skill or instruction packet, add it to the AI's working context, and ask it to follow that workflow while translating."
                        }
                    }
                    li { class: "ai-guide-step",
                        span { class: "ai-guide-step-index", "3" }
                        h3 { "Translate and audit a real text" }
                        p {
                            "Start with a law, policy, or contract. Ask for a Futuruna translation and an audit pass that calls out paradoxes, tensions, loopholes, missing definitions, and enforcement gaps."
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// The Seven Runes
// ============================================================================

struct RuneInfo {
    symbol: &'static str,
    name: &'static str,
    meaning: &'static str,
    example: &'static str,
}

const RUNES: [RuneInfo; 7] = [
    RuneInfo {
        symbol: "#",
        name: "What exists",
        meaning: "Types, effects, traits, impls",
        example: "# Point(x: Float, y: Float)",
    },
    RuneInfo {
        symbol: ">",
        name: "What happens",
        meaning: "Functions, actors, modules",
        example: "> distance(a: Point, b: Point) -> Float",
    },
    RuneInfo {
        symbol: "|",
        name: "What should be true",
        meaning: "Rules, match arms, handlers",
        example: "| is_valid(p) -> p.x > 0 && p.y > 0",
    },
    RuneInfo {
        symbol: "=",
        name: "What is",
        meaning: "Bindings, ground truth",
        example: "= origin = Point(0.0, 0.0)",
    },
    RuneInfo {
        symbol: "~",
        name: "What flows",
        meaning: "Reactive streams, temporal behavior",
        example: "~ clicks = from_list([1, 2, 3]) |> map(|x| x * 2)",
    },
    RuneInfo {
        symbol: "@",
        name: "Where proofs stop",
        meaning: "Meta/effects: print, use, import",
        example: "@ print(\"Hello, Futuruna\")",
    },
    RuneInfo {
        symbol: "?",
        name: "Prove it",
        meaning: "Solver/verification invocation",
        example: "? valid_point -> { @ print(\"verified\") }",
    },
];

#[component]
fn RunesShowcase() -> Element {
    rsx! {
        section { id: "runes", class: "runes-section",
            h2 { class: "section-title", "The Seven Runes" }
            p { class: "section-desc",
                "Each line begins with a semantic fly-in: a compact signal for types, functions, "
                "rules, values, flows, effects, or verification."
            }
            div { class: "runes-grid",
                for rune in RUNES.iter() {
                    div { class: "rune-card",
                        div { class: "rune-symbol", "{rune.symbol}" }
                        div { class: "rune-name", "{rune.name}" }
                        div { class: "rune-meaning", "{rune.meaning}" }
                        code { class: "rune-example",
                            dangerous_inner_html: highlight_runa(rune.example)
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// Pitch
// ============================================================================

#[component]
fn Pitch() -> Element {
    rsx! {
        section { class: "pitch-section",
            h2 { class: "section-title", "Law You Can Run" }
            p { class: "pitch-intro",
                "Encode legal rules without giving up ordinary programming. Keep the model, "
                "the calculations, and the audit in one language, then compile it through Rust."
            }
            div { class: "pitch-grid",
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Rules and Programs Together" }
                    p { class: "pitch-text",
                        "Express defaults, conditions, and named exceptions beside types, functions, "
                        "values, streams, and effects. No separate legal rules engine is required."
                    }
                }
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Audit the Model" }
                    p { class: "pitch-text",
                        "Demand checks close to the rules they examine. Surface conflicts, gaps, "
                        "unexpected outcomes, and the assumptions that produced them."
                    }
                }
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Source Beside Structure" }
                    p { class: "pitch-text",
                        "Keep statutory text, citations, effective dates, and explanatory metadata "
                        "close to the executable definitions and rules they support."
                    }
                }
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Compiles Through Rust" }
                    p { class: "pitch-text",
                        "Generate native programs through Rust's compiler and safety checks, with "
                        "ownership inference for ordinary value-oriented Futuruna code."
                    }
                }
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Built for AI Collaboration" }
                    p { class: "pitch-text",
                        "Give AI systems explicit forms for rules, exceptions, effects, and audit demands "
                        "instead of asking them to simulate those concepts through conventions."
                    }
                }
                div { class: "pitch-card",
                    h3 { class: "pitch-title", "Seven Semantic Modes" }
                    p { class: "pitch-text",
                        "A rune at the start of each statement provides a quick entry point into its "
                        "role while preserving a compact syntax across programming domains."
                    }
                }
            }
        }
    }
}

// ============================================================================
// Syntax highlighting
// ============================================================================

fn push_esc(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

fn push_span(out: &mut String, cls: &str, text: &str) {
    out.push_str("<span class=\"");
    out.push_str(cls);
    out.push_str("\">");
    push_esc(out, text);
    out.push_str("</span>");
}

fn is_kw(w: &str) -> bool {
    matches!(
        w,
        "if" | "else"
            | "match"
            | "for"
            | "in"
            | "under"
            | "exception"
            | "with"
            | "resume"
            | "return"
            | "spawn"
            | "ask"
            | "inout"
            | "pub"
            | "actor"
            | "trait"
            | "sealed"
            | "effect"
            | "impl"
            | "handle"
    )
}

fn is_at_kw(w: &str) -> bool {
    matches!(
        w,
        "print" | "import" | "depend" | "use" | "comptime" | "export" | "rust"
    )
}

// ============================================================================
// Documentation tooltip — data model + database
// ============================================================================

struct DocEntry {
    id: &'static str,
    name: &'static str,
    oneliner: &'static str,
    rune: &'static str,
    rune_class: &'static str,
}

const DOC_DB: &[DocEntry] = &[
    // -- Runes --
    DocEntry {
        id: "rune_hash",
        name: "#",
        oneliner: "What exists \u{2014} define types, effects, traits, impls",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "rune_gt",
        name: ">",
        oneliner: "What happens \u{2014} define functions, actors, modules",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "rune_pipe",
        name: "|",
        oneliner: "What must be true \u{2014} rules, invariants, match arms",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "rune_eq",
        name: "=",
        oneliner: "What is \u{2014} bindings, ground truth, constants",
        rune: "=",
        rune_class: "hl-rune-eq",
    },
    DocEntry {
        id: "rune_tilde",
        name: "~",
        oneliner: "What flows \u{2014} reactive streams, temporal behavior",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "rune_at",
        name: "@",
        oneliner: "Where proofs stop \u{2014} IO, imports, effects, meta",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "rune_question",
        name: "?",
        oneliner: "Prove it \u{2014} verification demands and assertions",
        rune: "?",
        rune_class: "hl-rune-question",
    },
    // -- Operators --
    DocEntry {
        id: "op_pipe",
        name: "|>",
        oneliner: "Pipe \u{2014} pass left side as input to right side",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "op_arrow",
        name: "->",
        oneliner: "Arrow \u{2014} separates parameters from return type or body",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "op_send",
        name: "<-",
        oneliner: "Send \u{2014} deliver a message to an actor",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- Keywords --
    DocEntry {
        id: "match",
        name: "match",
        oneliner: "Branch on value \u{2014} pattern matching with | arms",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "for",
        name: "for",
        oneliner: "Loop over a list or range",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "if",
        name: "if",
        oneliner: "Conditional expression \u{2014} evaluates to a value",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "else",
        name: "else",
        oneliner: "Alternative branch of an if or ? expression",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "in",
        name: "in",
        oneliner: "Iterator binding \u{2014} used with for loops",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "while",
        name: "while",
        oneliner: "Loop while condition is true",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "under",
        name: "under",
        oneliner: "Rule condition \u{2014} when this rule layer applies",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "exception",
        name: "exception",
        oneliner: "Rule override \u{2014} takes priority over base rules",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "inout",
        name: "inout",
        oneliner: "Mutable parameter \u{2014} allows in-place mutation",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "return",
        name: "return",
        oneliner: "Early return from a function",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "actor",
        name: "actor",
        oneliner: "Define a stateful concurrent actor",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "spawn",
        name: "spawn",
        oneliner: "Launch an actor instance",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "ask",
        name: "ask",
        oneliner: "Send a request to an actor and await response",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "trait",
        name: "trait",
        oneliner: "Define a shared interface for types",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "effect",
        name: "effect",
        oneliner: "Declare algebraic effects",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "impl",
        name: "impl",
        oneliner: "Implement a trait for a type",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "handle",
        name: "handle",
        oneliner: "Intercept and handle algebraic effects",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "with",
        name: "with",
        oneliner: "Provide an effect handler scope",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "resume",
        name: "resume",
        oneliner: "Continue execution from an effect handler",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    // -- Booleans --
    DocEntry {
        id: "True",
        name: "True",
        oneliner: "Boolean true value",
        rune: "=",
        rune_class: "hl-rune-eq",
    },
    DocEntry {
        id: "False",
        name: "False",
        oneliner: "Boolean false value",
        rune: "=",
        rune_class: "hl-rune-eq",
    },
    // -- Primitive types --
    DocEntry {
        id: "Int",
        name: "Int",
        oneliner: "64-bit signed integer \u{2014} compiles to i64",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Float",
        name: "Float",
        oneliner: "64-bit floating point \u{2014} compiles to f64",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "String",
        name: "String",
        oneliner: "UTF-8 text \u{2014} compiles to String",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Bool",
        name: "Bool",
        oneliner: "Boolean type \u{2014} True or False",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Char",
        name: "Char",
        oneliner: "Single Unicode character \u{2014} compiles to char",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    // -- Composite types --
    DocEntry {
        id: "List",
        name: "List",
        oneliner: "Ordered collection \u{2014} compiles to Vec<T>",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Option",
        name: "Option",
        oneliner: "Optional value \u{2014} None or Some(value)",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Result",
        name: "Result",
        oneliner: "Success or error \u{2014} Ok(value) or Err(error)",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Pair",
        name: "Pair",
        oneliner: "Two-element tuple \u{2014} access with .fst and .snd",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Map",
        name: "Map",
        oneliner: "Key-value dictionary \u{2014} compiles to HashMap",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Set",
        name: "Set",
        oneliner: "Unique collection \u{2014} compiles to HashSet",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Some",
        name: "Some",
        oneliner: "Wraps a value in Option \u{2014} indicates presence",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "None",
        name: "None",
        oneliner: "Empty Option \u{2014} indicates absence",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Ok",
        name: "Ok",
        oneliner: "Success variant of Result",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    DocEntry {
        id: "Err",
        name: "Err",
        oneliner: "Error variant of Result",
        rune: "#",
        rune_class: "hl-rune-hash",
    },
    // -- @ keywords --
    DocEntry {
        id: "print",
        name: "print",
        oneliner: "Output text to stdout \u{2014} primary IO effect",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "import",
        name: "import",
        oneliner: "Import definitions from another module",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "depend",
        name: "depend",
        oneliner: "Declare a Rust crate dependency",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "export",
        name: "export",
        oneliner: "Make a definition visible to other modules",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "comptime",
        name: "comptime",
        oneliner: "Evaluate expression at compile time",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "rust",
        name: "@ rust",
        oneliner: "Embed raw Rust code \u{2014} escape hatch",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    // -- Display --
    DocEntry {
        id: "show",
        name: "show",
        oneliner: "Convert any value to its string representation",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- Math --
    DocEntry {
        id: "abs",
        name: "abs",
        oneliner: "Absolute value of an integer",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "sqrt",
        name: "sqrt",
        oneliner: "Square root of a float",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "pow",
        name: "pow",
        oneliner: "Raise a float to a power",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "exp",
        name: "exp",
        oneliner: "Natural exponential (e^x)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "ln",
        name: "ln",
        oneliner: "Natural logarithm",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "round",
        name: "round",
        oneliner: "Round a float to the nearest integer",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "floor",
        name: "floor",
        oneliner: "Round a float down to an integer",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "to_float",
        name: "to_float",
        oneliner: "Convert an integer to a float",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "max_int",
        name: "max_int",
        oneliner: "Maximum of two integers",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "min_int",
        name: "min_int",
        oneliner: "Minimum of two integers",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "clamp",
        name: "clamp",
        oneliner: "Clamp value to range [lo, hi]",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- String --
    DocEntry {
        id: "string_length",
        name: "string_length",
        oneliner: "Number of Unicode scalar values in a string",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "split",
        name: "split",
        oneliner: "Split a string by separator into a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "join",
        name: "join",
        oneliner: "Join a list of strings with a separator",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "trim",
        name: "trim",
        oneliner: "Remove leading and trailing whitespace",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "contains",
        name: "contains",
        oneliner: "Test if a string contains a substring",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "starts_with",
        name: "starts_with",
        oneliner: "Test if a string starts with a prefix",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "ends_with",
        name: "ends_with",
        oneliner: "Test if a string ends with a suffix",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "replace",
        name: "replace",
        oneliner: "Replace all occurrences of a substring",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "to_upper",
        name: "to_upper",
        oneliner: "Convert a string to uppercase",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "to_lower",
        name: "to_lower",
        oneliner: "Convert a string to lowercase",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "substring",
        name: "substring",
        oneliner: "Extract a string slice by Unicode scalar index and length",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "char_at",
        name: "char_at",
        oneliner: "Get one Unicode scalar value from a string by index",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "index_of",
        name: "index_of",
        oneliner: "Find scalar position of a substring (\u{2212}1 if absent)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "string_chars",
        name: "string_chars",
        oneliner: "Explode a string into Unicode scalar values",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "parse_int",
        name: "parse_int",
        oneliner: "Parse a string to integer (0 on failure)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "parse_float",
        name: "parse_float",
        oneliner: "Parse a string to float (0.0 on failure)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "format_float",
        name: "format_float",
        oneliner: "Format a float with N decimal places",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- List --
    DocEntry {
        id: "push",
        name: "push",
        oneliner: "Append an element to the end of a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "length",
        name: "length",
        oneliner: "Number of elements in a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "head",
        name: "head",
        oneliner: "First element of a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "tail",
        name: "tail",
        oneliner: "All elements except the first",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "nth",
        name: "nth",
        oneliner: "Get element at index (O(1) access)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "reverse",
        name: "reverse",
        oneliner: "Reverse the order of a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "count",
        name: "count",
        oneliner: "Number of elements in a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "sort",
        name: "sort",
        oneliner: "Sort a list in ascending order",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "sort_by",
        name: "sort_by",
        oneliner: "Sort a list with a custom comparison function",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "filter",
        name: "filter",
        oneliner: "Keep elements matching a predicate",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "map",
        name: "map",
        oneliner: "Transform each element with a function",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "flat_map",
        name: "flat_map",
        oneliner: "Map then flatten nested lists",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "zip",
        name: "zip",
        oneliner: "Combine two lists element-wise into pairs",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "enumerate",
        name: "enumerate",
        oneliner: "Pair each element with its index",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "any",
        name: "any",
        oneliner: "True if any element matches a predicate",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "all",
        name: "all",
        oneliner: "True if all elements match a predicate",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "find",
        name: "find",
        oneliner: "First element matching a predicate",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "sum_list",
        name: "sum_list",
        oneliner: "Sum all numbers in a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "distinct",
        name: "distinct",
        oneliner: "Remove duplicate elements",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "take_while",
        name: "take_while",
        oneliner: "Take elements while predicate holds",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "drop_while",
        name: "drop_while",
        oneliner: "Skip elements while predicate holds",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "count_by",
        name: "count_by",
        oneliner: "Count elements matching a predicate",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "partition",
        name: "partition",
        oneliner: "Split list by predicate into two lists",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "chunked",
        name: "chunked",
        oneliner: "Split list into chunks of size N",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "range",
        name: "range",
        oneliner: "Generate a list of integers from start to end",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- Streams --
    DocEntry {
        id: "from_list",
        name: "from_list",
        oneliner: "Create a stream from a list",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "subject",
        name: "subject",
        oneliner: "Create a push-based broadcast stream",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "complete",
        name: "complete",
        oneliner: "Signal that a subject has finished",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "subscribe",
        name: "subscribe",
        oneliner: "Listen to stream emissions",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "scan",
        name: "scan",
        oneliner: "Accumulate stream values with a function",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "take",
        name: "take",
        oneliner: "Take first N elements from a stream",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "skip",
        name: "skip",
        oneliner: "Skip first N elements of a stream",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "tap",
        name: "tap",
        oneliner: "Side-effect on each stream element",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "merge",
        name: "merge",
        oneliner: "Combine multiple streams into one",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "collect",
        name: "collect",
        oneliner: "Gather all stream elements into a list",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "debounce",
        name: "debounce",
        oneliner: "Wait for silence before emitting latest value",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "throttle",
        name: "throttle",
        oneliner: "Limit emission rate to one per interval",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "delay",
        name: "delay",
        oneliner: "Delay each emission by N milliseconds",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "buffer",
        name: "buffer",
        oneliner: "Collect emissions into time-windowed batches",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "timeout",
        name: "timeout",
        oneliner: "Fail if no emission within time limit",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "switch_map",
        name: "switch_map",
        oneliner: "Map to stream, cancel previous on new emission",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "sample",
        name: "sample",
        oneliner: "Emit latest value when another stream emits",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "first",
        name: "first",
        oneliner: "Take only the first emission",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "reduce",
        name: "reduce",
        oneliner: "Reduce stream to single value",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "start_with",
        name: "start_with",
        oneliner: "Prepend a value before stream emissions",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    DocEntry {
        id: "pairwise",
        name: "pairwise",
        oneliner: "Emit consecutive pairs of values",
        rune: "~",
        rune_class: "hl-rune-tilde",
    },
    // -- Logic --
    DocEntry {
        id: "findall",
        name: "findall",
        oneliner: "Collect all solutions to a goal into a list",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    DocEntry {
        id: "not",
        name: "not",
        oneliner: "Negation as failure \u{2014} true if goal fails",
        rune: "|",
        rune_class: "hl-rune-pipe",
    },
    // -- Map builtins --
    DocEntry {
        id: "map_new",
        name: "map_new",
        oneliner: "Create an empty Map",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_insert",
        name: "map_insert",
        oneliner: "Add or update a key-value pair in a Map",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_get",
        name: "map_get",
        oneliner: "Get value by key (returns Option)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_contains",
        name: "map_contains",
        oneliner: "Check if a key exists in a Map",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_remove",
        name: "map_remove",
        oneliner: "Remove a key from a Map",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_keys",
        name: "map_keys",
        oneliner: "Get all keys as a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_values",
        name: "map_values",
        oneliner: "Get all values as a list",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_len",
        name: "map_len",
        oneliner: "Number of entries in a Map",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "map_merge",
        name: "map_merge",
        oneliner: "Merge two Maps (right wins on conflict)",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_new",
        name: "set_new",
        oneliner: "Create an empty Set",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_insert",
        name: "set_insert",
        oneliner: "Add an element to a Set",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_contains",
        name: "set_contains",
        oneliner: "Check if element exists in a Set",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_union",
        name: "set_union",
        oneliner: "Union of two Sets",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_intersect",
        name: "set_intersect",
        oneliner: "Intersection of two Sets",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "set_diff",
        name: "set_diff",
        oneliner: "Elements in first Set but not second",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- File I/O --
    DocEntry {
        id: "read_file",
        name: "read_file",
        oneliner: "Read entire file contents as a string",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "write_file",
        name: "write_file",
        oneliner: "Write string to a file (creates or overwrites)",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "read_lines",
        name: "read_lines",
        oneliner: "Read file as list of lines",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "env_var",
        name: "env_var",
        oneliner: "Get environment variable value",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    // -- JSON --
    DocEntry {
        id: "json_parse",
        name: "json_parse",
        oneliner: "Parse a JSON string into a value",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "json_get",
        name: "json_get",
        oneliner: "Get a field from a JSON object",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "json_emit",
        name: "json_emit",
        oneliner: "Convert a value to a JSON string",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    // -- HTTP --
    DocEntry {
        id: "http_get",
        name: "http_get",
        oneliner: "Make an HTTP GET request",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "http_post",
        name: "http_post",
        oneliner: "Make an HTTP POST request",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    // -- DB --
    DocEntry {
        id: "db_open",
        name: "db_open",
        oneliner: "Open a SQLite database connection",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "db_exec",
        name: "db_exec",
        oneliner: "Execute a SQL statement",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    DocEntry {
        id: "db_query",
        name: "db_query",
        oneliner: "Query rows from the database",
        rune: "@",
        rune_class: "hl-rune-at",
    },
    // -- Pair accessors --
    DocEntry {
        id: "fst",
        name: "fst",
        oneliner: "First element of a Pair",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
    DocEntry {
        id: "snd",
        name: "snd",
        oneliner: "Second element of a Pair",
        rune: ">",
        rune_class: "hl-rune-gt",
    },
];

/// Check whether a word has a doc entry (for the highlighter).
fn word_doc_id<'a>(w: &'a str) -> Option<&'a str> {
    // Keywords, at-keywords, booleans, types, and builtins.
    // Rune characters and operators are handled separately in the highlighter.
    match w {
        // keywords
        "match" | "for" | "if" | "else" | "in" | "while" | "under" | "exception" | "inout"
        | "return" | "actor" | "spawn" | "ask" | "trait" | "effect" | "impl" | "handle"
        | "with" | "resume" => Some(w),
        // booleans (source is lowercase true/false, doc id is True/False)
        "true" => Some("True"),
        "false" => Some("False"),
        // types + constructors
        "Int" | "Float" | "String" | "Bool" | "Char" | "List" | "Option" | "Result" | "Pair"
        | "Map" | "Set" | "Some" | "None" | "Ok" | "Err" => Some(w),
        // @ keywords
        "print" | "import" | "depend" | "export" | "comptime" | "rust" => Some(w),
        // display
        "show" => Some(w),
        // math
        "abs" | "sqrt" | "pow" | "exp" | "ln" | "round" | "floor" | "to_float" | "max_int"
        | "min_int" | "clamp" => Some(w),
        // string
        "string_length" | "split" | "join" | "trim" | "contains" | "starts_with" | "ends_with"
        | "replace" | "to_upper" | "to_lower" | "substring" | "char_at" | "index_of"
        | "string_chars" | "parse_int" | "parse_float" | "format_float" => Some(w),
        // list
        "push" | "length" | "head" | "tail" | "nth" | "reverse" | "count" | "sort" | "sort_by"
        | "filter" | "map" | "flat_map" | "zip" | "enumerate" | "any" | "all" | "find"
        | "sum_list" | "distinct" | "take_while" | "drop_while" | "count_by" | "partition"
        | "chunked" | "range" => Some(w),
        // streams
        "from_list" | "subject" | "complete" | "subscribe" | "scan" | "take" | "skip" | "tap"
        | "merge" | "collect" | "debounce" | "throttle" | "delay" | "buffer" | "timeout"
        | "switch_map" | "sample" | "first" | "reduce" | "start_with" | "pairwise" => Some(w),
        // logic
        "findall" | "not" => Some(w),
        // map/set builtins
        "map_new" | "map_insert" | "map_get" | "map_contains" | "map_remove" | "map_keys"
        | "map_values" | "map_len" | "map_merge" | "set_new" | "set_insert" | "set_contains"
        | "set_union" | "set_intersect" | "set_diff" => Some(w),
        // file/json/http/db
        "read_file" | "write_file" | "read_lines" | "env_var" | "json_parse" | "json_get"
        | "json_emit" | "http_get" | "http_post" | "db_open" | "db_exec" | "db_query" => Some(w),
        // pair
        "fst" | "snd" => Some(w),
        _ => None,
    }
}

/// Emit a <span> with both a CSS class and a data-doc attribute.
fn push_span_doc(out: &mut String, cls: &str, text: &str, doc_id: &str) {
    out.push_str("<span");
    if !cls.is_empty() {
        out.push_str(" class=\"");
        out.push_str(cls);
        out.push('"');
    }
    out.push_str(" data-doc=\"");
    out.push_str(doc_id);
    out.push_str("\">");
    push_esc(out, text);
    out.push_str("</span>");
}

/// Generate the documentation database as a JavaScript object literal.
fn doc_db_js() -> String {
    let mut js = String::from("window.__FDOCS={");
    for e in DOC_DB {
        js.push_str(&format!(
            "\"{}\":{{n:\"{}\",d:\"{}\",r:\"{}\",c:\"{}\"}},",
            e.id,
            e.name.replace('"', "\\\""),
            e.oneliner.replace('"', "\\\""),
            e.rune.replace('"', "\\\""),
            e.rune_class,
        ));
    }
    js.push_str("};");
    js
}

fn highlight_runa(code: &str) -> String {
    let mut out = String::with_capacity(code.len() * 2);
    let mut in_block_comment = false;
    for (i, line) in code.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        in_block_comment = hl_line(line, &mut out, in_block_comment);
    }
    out
}

/// Returns true if still inside a block comment at end of line.
fn hl_line(line: &str, out: &mut String, in_block_comment: bool) -> bool {
    let trimmed = line.trim_start();
    out.push_str(&line[..line.len() - trimmed.len()]);

    // Continuing a block comment from a previous line
    if in_block_comment {
        if let Some(end) = trimmed.find("-}") {
            let comment_part: &str = &trimmed[..end + 2];
            push_span(out, "hl-comment", comment_part);
            // Process rest of line after the block comment ends
            let rest = trimmed[end + 2..].trim_start();
            if !rest.is_empty() {
                out.push_str(&trimmed[end + 2..trimmed.len() - rest.len() + end + 2 - (end + 2)]);
                return hl_line_inner(rest, out);
            }
            return false;
        } else {
            push_span(out, "hl-comment", trimmed);
            return true;
        }
    }

    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with("--") {
        push_span(out, "hl-comment", trimmed);
        return false;
    }

    hl_line_inner(trimmed, out)
}

/// Highlight a trimmed line (not inside a block comment). Returns true if ends inside block comment.
fn hl_line_inner(trimmed: &str, out: &mut String) -> bool {
    let ch: Vec<char> = trimmed.chars().collect();
    let n = ch.len();
    let mut i = 0;
    let mut after_at = false;

    // Leading rune: first non-ws char followed by space or EOL
    let rune = if n == 1 || ch[1] == ' ' {
        match ch[0] {
            '#' => Some("hl-rune-hash"),
            '>' => Some("hl-rune-gt"),
            '|' => Some("hl-rune-pipe"),
            '=' => Some("hl-rune-eq"),
            '~' => Some("hl-rune-tilde"),
            '@' => Some("hl-rune-at"),
            '?' => Some("hl-rune-question"),
            _ => None,
        }
    } else {
        None
    };

    if let Some(cls) = rune {
        let doc_id = match ch[0] {
            '#' => "rune_hash",
            '>' => "rune_gt",
            '|' => "rune_pipe",
            '=' => "rune_eq",
            '~' => "rune_tilde",
            '@' => "rune_at",
            '?' => "rune_question",
            _ => "",
        };
        push_span_doc(out, cls, &ch[0].to_string(), doc_id);
        i = 1;
        after_at = cls == "hl-rune-at";
    }

    while i < n {
        let c = ch[i];

        // Block comment {- ... -}
        if c == '{' && i + 1 < n && ch[i + 1] == '-' {
            // Find closing -} on this line
            let rest: String = ch[i..].iter().collect();
            if let Some(end) = rest[2..].find("-}") {
                let comment = &rest[..end + 4]; // {- ... -}
                push_span(out, "hl-comment", comment);
                i += end + 4;
                continue;
            } else {
                // Block comment continues to next line
                push_span(out, "hl-comment", &rest);
                return true;
            }
        }

        // Inline comment
        if c == '-' && i + 1 < n && ch[i + 1] == '-' {
            let rest: String = ch[i..].iter().collect();
            push_span(out, "hl-comment", &rest);
            return false;
        }

        // String literal
        if c == '"' {
            let start = i;
            i += 1;
            while i < n {
                if ch[i] == '\\' && i + 1 < n {
                    i += 2;
                } else if ch[i] == '"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            let s: String = ch[start..i].iter().collect();
            push_span(out, "hl-string", &s);
            continue;
        }

        // Pipe |>
        if c == '|' && i + 1 < n && ch[i + 1] == '>' {
            push_span_doc(out, "hl-pipe", "|>", "op_pipe");
            i += 2;
            continue;
        }

        // Arrow ->
        if c == '-' && i + 1 < n && ch[i + 1] == '>' {
            push_span_doc(out, "hl-arrow", "->", "op_arrow");
            i += 2;
            continue;
        }

        // Send <-
        if c == '<' && i + 1 < n && ch[i + 1] == '-' {
            push_span_doc(out, "hl-arrow", "<-", "op_send");
            i += 2;
            continue;
        }

        // Number
        if c.is_ascii_digit() {
            let start = i;
            while i < n && (ch[i].is_ascii_digit() || ch[i] == '.') {
                i += 1;
            }
            let num: String = ch[start..i].iter().collect();
            push_span(out, "hl-number", &num);
            continue;
        }

        // Word
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < n && (ch[i].is_alphanumeric() || ch[i] == '_') {
                i += 1;
            }
            let w: String = ch[start..i].iter().collect();
            if after_at && is_at_kw(&w) {
                if let Some(did) = word_doc_id(&w) {
                    push_span_doc(out, "hl-at-word", &w, did);
                } else {
                    push_span(out, "hl-at-word", &w);
                }
                after_at = false;
            } else if is_kw(&w) {
                if let Some(did) = word_doc_id(&w) {
                    push_span_doc(out, "hl-keyword", &w, did);
                } else {
                    push_span(out, "hl-keyword", &w);
                }
            } else if w == "true" || w == "false" {
                let did = if w == "true" { "True" } else { "False" };
                push_span_doc(out, "hl-bool", &w, did);
            } else if ch[start].is_uppercase() {
                if let Some(did) = word_doc_id(&w) {
                    push_span_doc(out, "hl-type", &w, did);
                } else {
                    push_span(out, "hl-type", &w);
                }
            } else if let Some(did) = word_doc_id(&w) {
                // Known builtin — no visual change, but add data-doc for tooltip
                push_span_doc(out, "", &w, did);
            } else {
                push_esc(out, &w);
            }
            continue;
        }

        push_esc(out, &c.to_string());
        i += 1;
    }
    false
}

// ============================================================================
// Code Example
// ============================================================================

#[component]
fn CodeExample() -> Element {
    let code = r#"-- define the universe
# Coffee = Espresso | Latte | Decaf

-- define what matters
> strength(c: Coffee) -> Int { match c { | Decaf -> 0 | _ -> 100 } }

-- assign reality
= your_order = Espresso

-- state the law
| real_coffee: your_order -> your_order != Decaf

-- watch it flow
~ real = from_list([Espresso, Latte, Decaf]) |> filter(|c| c != Decaf)

-- cross the boundary
@ print(show(count(real)) + " real coffees. Yours: " + show(strength(your_order)) + "mg")

-- demand proof
? real_coffee"#;

    let output = r#"2 real coffees. Yours: 100mg
  [ok] |real_coffee| holds (value: Espresso)"#;

    rsx! {
        section { class: "code-section",
            h2 { class: "section-title", "Seven Runes, Fourteen Lines" }
            div { class: "code-container",
                pre { class: "code-block",
                    code { dangerous_inner_html: highlight_runa(code) }
                }
                pre { class: "code-output",
                    code { "{output}" }
                }
            }
        }
    }
}

// ============================================================================
// Playground
// ============================================================================

const EXAMPLE_WEATHER: &str = r#"# Condition = Sunny | Cloudy | Stormy
# Weather(day: String, temp: Float, condition: Condition)

> describe(w: Weather) -> String {
    match w.condition {
        | Sunny -> show(w.temp) + " C, sunny"
        | Stormy -> show(w.temp) + " C, storm"
        | Cloudy -> show(w.temp) + " C, cloudy"
    }
}

| advisory(w) -> "all clear"
| advisory(w) -> "heat warning" under w.temp > 35.0
| exception storm advisory(w) -> "danger" under w.condition == Stormy

= today = Weather("today", 22.0, Sunny)
= alert = advisory(today)

@ print(today.day + ": " + describe(today) + " -- " + alert)

~ forecast = from_list([today, Weather("tomorrow", 40.0, Sunny), Weather("in 2 days", 18.0, Cloudy), Weather("in 3 days", 10.0, Stormy)]) |> filter(|w| advisory(w) != "all clear")

= warning_count = count(forecast)
| has_warnings: warning_count -> warning_count > 0

? has_warnings: n -> {
    @ print("Upcoming warnings (" + show(n) + "):")
    ~ forecast | w -> {
        @ print("  " + w.day + ": " + advisory(w) + " -> " + describe(w))
    }
} else {
    @ print("No warnings -- all clear ahead")
}"#;

const EXAMPLE_HELLO: &str = r#"# Greeting(name: String, times: Int)

> repeat_greeting(g: Greeting) -> String {
    = msg = "Hello, " + g.name + "!"
    = result = []
    for i in range(0, g.times) {
        = result = push(result, msg)
    }
    join(result, " ")
}

= g = Greeting("World", 3)
@ print(repeat_greeting(g))
@ print("Length: " + show(string_length(repeat_greeting(g))))"#;

const EXAMPLE_STREAMS: &str = r#"-- Reactive streams with pipe operators

~ numbers = from_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])

~ evens = numbers |> filter(|x| x % 2 == 0)
~ doubled = evens |> map(|x| x * 2)
= total = sum_list(doubled)

@ print("Numbers: " + show(numbers))
@ print("Evens: " + show(evens))
@ print("Doubled: " + show(doubled))
@ print("Sum: " + show(total))

-- Strings through pipes
~ words = from_list(["hello", "world", "from", "futuruna"])
~ upper = words |> map(|w| to_upper(w))
~ long = upper |> filter(|w| string_length(w) > 4)

@ print("Upper: " + show(upper))
@ print("Long words: " + show(long))"#;

const EXAMPLE_RULES: &str = r#"-- Layered rules with exceptions (Catala-style)

# Status = Student | Employee | Retired | Unemployed
# Person(name: String, age: Int, status: Status)

-- Base rule: standard tax rate
| tax_rate(p) -> 25.0

-- Students pay less
| tax_rate(p) -> 10.0 under p.status == Student

-- Retirees pay less
| tax_rate(p) -> 15.0 under p.status == Retired

-- Senior exception overrides everything
| exception senior tax_rate(p) -> 0.0 under p.age >= 70

= people = [Person("Alice", 25, Student), Person("Bob", 45, Employee), Person("Carol", 72, Retired), Person("Dan", 30, Unemployed)]

for p in people {
    @ print(p.name + " (age " + show(p.age) + ", " + show(p.status) + "): " + show(tax_rate(p)) + "% tax")
}"#;

const EXAMPLE_FIBONACCI: &str = r#"-- Classic algorithms in Futuruna

> fib(n: Int) -> Int {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

> fizzbuzz(n: Int) -> String {
    if n % 15 == 0 { "FizzBuzz" }
    else if n % 3 == 0 { "Fizz" }
    else if n % 5 == 0 { "Buzz" }
    else { show(n) }
}

@ print("Fibonacci sequence:")
for i in range(0, 12) {
    @ print("  fib(" + show(i) + ") = " + show(fib(i)))
}

@ print("")
@ print("FizzBuzz 1-20:")
= results = []
for i in range(1, 21) {
    = results = push(results, fizzbuzz(i))
}
@ print("  " + join(results, ", "))"#;

const EXAMPLE_BOOT: &str = r##"-- Futuruna Boot Sequence
-- Uses ~ streams with delay to print line by line

> pad(s: String, n: Int) -> String {
    if string_length(s) >= n { s }
    else { pad(s + " ", n) }
}

> bar(n: Int) -> String {
    = s = ""
    for i in range(0, n) {
        = s = s + "#"
    }
    s
}

-- Build the boot log as a stream of messages
= modules = ["consciousness", "entropy", "runes", "streams", "rules", "verification", "effects"]

= log = ["FUTURUNA v0.1.0", "================", ""]
= log = push(log, "[init] Booting language runtime...")
= log = push(log, "")
for m in modules {
    = log = push(log, "  [load] " + pad(m, 16) + "OK")
}
= log = push(log, "")
= log = push(log, "[calc] Shannon entropy H     = 2.807 bits")
= log = push(log, "[calc] Integrated info Phi   = 3.0")
= log = push(log, "[calc] Effective dim d_eff   = 3")
= log = push(log, "[calc] Causal entropy S_tau  = optimal")
= log = push(log, "")
= log = push(log, "[consciousness meter]")
for i in range(1, 11) {
    = log = push(log, "  [" + bar(i * 2) + pad("", 20 - i * 2) + "] " + show(i * 10) + "%")
}
= log = push(log, "")
= log = push(log, "Consciousness achieved.")
= log = push(log, "")
= log = push(log, "  # what exists")
= log = push(log, "  > what happens")
= log = push(log, "  | what must be true")
= log = push(log, "  = what is")
= log = push(log, "  ~ what flows")
= log = push(log, "  @ where proofs stop")
= log = push(log, "  ? prove it")
= log = push(log, "")
= log = push(log, "You are the programmer now.")

-- Stream it with 150ms delay between each line
~ boot = from_list(log) |> delay(150)

~ boot | line -> {
    @ print(line)
}"##;

struct Example {
    name: &'static str,
    code: &'static str,
}

const EXAMPLES: &[Example] = &[
    Example {
        name: "Weather",
        code: EXAMPLE_WEATHER,
    },
    Example {
        name: "Hello",
        code: EXAMPLE_HELLO,
    },
    Example {
        name: "Streams",
        code: EXAMPLE_STREAMS,
    },
    Example {
        name: "Rules",
        code: EXAMPLE_RULES,
    },
    Example {
        name: "Fibonacci",
        code: EXAMPLE_FIBONACCI,
    },
    Example {
        name: "Boot",
        code: EXAMPLE_BOOT,
    },
];

#[component]
fn Playground() -> Element {
    let mut code = use_signal(|| String::from(EXAMPLE_WEATHER));
    let mut output = use_signal(|| String::from("Click 'Run' to execute..."));
    let mut is_running = use_signal(|| false);
    let mut active_example = use_signal(|| 0usize);

    let run_code = move |_| {
        if *is_running.read() {
            return;
        }
        is_running.set(true);
        output.set(String::new());
        let source = code.read().clone();

        // Extract delay from |> delay(N) in source, if present
        let delay_ms = {
            let s = &source;
            if let Some(pos) = s.find("|> delay(") {
                let after = &s[pos + 9..];
                if let Some(end) = after.find(')') {
                    after[..end].trim().parse::<u32>().unwrap_or(0)
                } else {
                    0
                }
            } else {
                0
            }
        };

        spawn(async move {
            match futuruna::eval_source(&source) {
                Ok(result) => {
                    if delay_ms > 0 {
                        let lines: Vec<&str> = result.lines().collect();
                        let mut displayed = String::new();
                        for line in &lines {
                            displayed.push_str(line);
                            displayed.push('\n');
                            output.set(displayed.clone());
                            TimeoutFuture::new(delay_ms).await;
                        }
                    } else {
                        output.set(result);
                    }
                }
                Err(e) => output.set(format!("Error: {}", e)),
            }
            is_running.set(false);
        });
    };

    rsx! {
        section { id: "playground", class: "playground-section",
            h2 { class: "section-title", "Playground" }
            p { class: "section-desc",
                "Write Futuruna code and run it in your browser. "
                a { class: "playground-fullscreen-link", href: "/playground", "Open full playground \u{2192}" }
            }
            div { class: "example-buttons",
                for (i, ex) in EXAMPLES.iter().enumerate() {
                    button {
                        class: if *active_example.read() == i { "btn btn-example active" } else { "btn btn-example" },
                        onclick: move |_| {
                            code.set(EXAMPLES[i].code.to_string());
                            output.set("Click 'Run' to execute...".to_string());
                            active_example.set(i);
                        },
                        "{ex.name}"
                    }
                }
            }
            div { class: "playground-container",
                div { class: "playground-editor",
                    div { class: "editor-header",
                        span { class: "editor-title", "main.runa" }
                        button {
                            class: if *is_running.read() { "btn btn-run disabled" } else { "btn btn-run" },
                            onclick: run_code,
                            if *is_running.read() { "Running..." } else { "Run" }
                        }
                    }
                    div { class: "editor-layer",
                        pre { class: "editor-highlight",
                            code { dangerous_inner_html: format!("{}\n", highlight_runa(&code.read())) }
                        }
                        textarea {
                            spellcheck: false,
                            value: "{code}",
                            oninput: move |evt| code.set(evt.value())
                        }
                    }
                }
                div { class: "playground-output",
                    div { class: "output-header",
                        span { class: "output-title", "Output" }
                    }
                    pre { class: "output-content",
                        "{output}"
                    }
                }
            }
        }
    }
}

// ============================================================================
// Playground Page — full-viewport dedicated route
// ============================================================================

#[component]
fn PlaygroundPage() -> Element {
    let mut code = use_signal(|| String::from(EXAMPLE_WEATHER));
    let mut output = use_signal(|| String::from("Click 'Run' to execute..."));
    let mut is_running = use_signal(|| false);
    let mut active_example = use_signal(|| 0usize);
    let mut share_flash = use_signal(|| false);

    // Restore from URL hash (deflate+base64url) or localStorage on mount
    use_effect(move || {
        spawn(async move {
            let mut eval = dioxus::document::eval(
                r#"
                (async function() {
                    var hash = window.location.hash || '';
                    if (hash.startsWith('#code=')) {
                        var enc = hash.slice(6);
                        try {
                            var bin = atob(enc.replace(/-/g, '+').replace(/_/g, '/'));
                            var bytes = Uint8Array.from(bin, function(c) { return c.charCodeAt(0); });
                            var ds = new DecompressionStream('deflate-raw');
                            var stream = new Blob([bytes]).stream().pipeThrough(ds);
                            var text = await new Response(stream).text();
                            if (text) { dioxus.send(text); return; }
                        } catch(e) { console.warn('URL decode failed:', e); }
                    }
                    var saved = localStorage.getItem('futuruna-pg-code');
                    dioxus.send(saved || '');
                })();
            "#,
            );
            if let Ok(val) = eval.recv::<String>().await {
                if !val.is_empty() {
                    code.set(val);
                    active_example.set(usize::MAX); // mark as custom
                }
            }
        });
    });

    // Auto-save to localStorage on every edit (JS-side debounced)
    let on_input = move |evt: dioxus::events::FormEvent| {
        let val = evt.value();
        code.set(val.clone());
        // Fire-and-forget save
        dioxus::document::eval(&format!(
            r#"clearTimeout(window.__pgSaveTimer);
               window.__pgSaveTimer = setTimeout(function() {{
                   localStorage.setItem('futuruna-pg-code', {});
               }}, 400);"#,
            serde_json_str(&val)
        ));
    };

    // Share: compress code to URL hash and copy to clipboard
    let on_share = move |_| {
        let src = code.read().clone();
        share_flash.set(true);
        spawn(async move {
            dioxus::document::eval(&format!(
                r#"(async function() {{
                    var text = {};
                    var blob = new Blob([text]);
                    var cs = new CompressionStream('deflate-raw');
                    var stream = blob.stream().pipeThrough(cs);
                    var buf = await new Response(stream).arrayBuffer();
                    var b64 = btoa(String.fromCharCode.apply(null, new Uint8Array(buf)));
                    var safe = b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
                    var url = window.location.origin + '/playground#code=' + safe;
                    window.history.replaceState(null, '', '/playground#code=' + safe);
                    try {{ await navigator.clipboard.writeText(url); }} catch(e) {{}}
                }})();"#,
                serde_json_str(&src)
            ));
            TimeoutFuture::new(1500).await;
            share_flash.set(false);
        });
    };

    let run_code = move |_| {
        if *is_running.read() {
            return;
        }
        is_running.set(true);
        output.set(String::new());
        let source = code.read().clone();

        let delay_ms = {
            let s = &source;
            if let Some(pos) = s.find("|> delay(") {
                let after = &s[pos + 9..];
                if let Some(end) = after.find(')') {
                    after[..end].trim().parse::<u32>().unwrap_or(0)
                } else {
                    0
                }
            } else {
                0
            }
        };

        spawn(async move {
            match futuruna::eval_source(&source) {
                Ok(result) => {
                    if delay_ms > 0 {
                        let lines: Vec<&str> = result.lines().collect();
                        let mut displayed = String::new();
                        for line in &lines {
                            displayed.push_str(line);
                            displayed.push('\n');
                            output.set(displayed.clone());
                            TimeoutFuture::new(delay_ms).await;
                        }
                    } else {
                        output.set(result);
                    }
                }
                Err(e) => output.set(format!("Error: {}", e)),
            }
            is_running.set(false);
        });
    };

    rsx! {
        document::Title { "Playground — Futuruna Programming Language" }
        document::Meta { name: "description", content: "Try Futuruna in your browser — write code with the seven runes, run it instantly, and explore examples from weather demos to reactive streams." }
        div { class: "pg-page",
            aside { class: "pg-sidebar",
                div { class: "pg-get-started",
                    h3 { class: "pg-sidebar-title", "Playground" }
                    p { class: "pg-sidebar-hint", "Pick an example or write your own Futuruna code." }
                }
                div { class: "pg-examples",
                    h4 { class: "pg-examples-label", "Examples" }
                    for (i, ex) in EXAMPLES.iter().enumerate() {
                        button {
                            class: if *active_example.read() == i { "pg-example-btn active" } else { "pg-example-btn" },
                            onclick: move |_| {
                                code.set(EXAMPLES[i].code.to_string());
                                output.set("Click 'Run' to execute...".to_string());
                                active_example.set(i);
                                // Clear localStorage so example is default on reload
                                dioxus::document::eval("localStorage.removeItem('futuruna-pg-code'); history.replaceState(null,'','/playground');");
                            },
                            "{ex.name}"
                        }
                    }
                }
                div { class: "pg-sidebar-footer",
                    a { class: "pg-sidebar-link", href: "/docs", "Docs" }
                    a { class: "pg-sidebar-link", href: "/why", "Why Futuruna?" }
                }
            }
            div { class: "pg-main",
                div { class: "pg-editor",
                    div { class: "pg-panel-header",
                        span { class: "pg-panel-title", "main.runa" }
                        div { class: "pg-panel-actions",
                            button {
                                class: if *share_flash.read() { "btn btn-share flash" } else { "btn btn-share" },
                                onclick: on_share,
                                if *share_flash.read() { "Copied!" } else { "Share" }
                            }
                            button {
                                class: if *is_running.read() { "btn btn-run disabled" } else { "btn btn-run" },
                                onclick: run_code,
                                if *is_running.read() { "Running..." } else { "Run" }
                            }
                        }
                    }
                    div { class: "editor-layer",
                        pre { class: "editor-highlight",
                            code { dangerous_inner_html: format!("{}\n", highlight_runa(&code.read())) }
                        }
                        textarea {
                            spellcheck: false,
                            value: "{code}",
                            oninput: on_input
                        }
                    }
                }
                div { class: "pg-output",
                    div { class: "pg-panel-header",
                        span { class: "pg-panel-title", "Output" }
                    }
                    pre { class: "pg-output-content",
                        "{output}"
                    }
                }
            }
        }
    }
}

/// JSON-encode a string for safe embedding in JS. Handles quotes, newlines, backslashes.
fn serde_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

// ============================================================================
// Docs page — /docs
// ============================================================================

const DOC_BASICS: &str = include_str!("../../docs/reference/basics.md");
const DOC_RUNES: &str = include_str!("../../docs/reference/runes.md");
const DOC_STDLIB: &str = include_str!("../../docs/reference/stdlib.md");
const DOC_STREAMS: &str = include_str!("../../docs/reference/streams.md");
const DOC_RUST: &str = include_str!("../../docs/reference/rust-compatibility.md");

struct DocPage {
    label: &'static str,
    content: &'static str,
}

const DOC_PAGES: &[DocPage] = &[
    DocPage {
        label: "Runes",
        content: DOC_RUNES,
    },
    DocPage {
        label: "Basics",
        content: DOC_BASICS,
    },
    DocPage {
        label: "Stdlib",
        content: DOC_STDLIB,
    },
    DocPage {
        label: "Streams",
        content: DOC_STREAMS,
    },
    DocPage {
        label: "Rust",
        content: DOC_RUST,
    },
];

fn md_to_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let md = strip_markdown_frontmatter(md);
    let parser = Parser::new_ext(md, opts);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}

#[component]
fn DocsPage() -> Element {
    let mut active_doc = use_signal(|| 0usize);
    let html_content = md_to_html(DOC_PAGES[active_doc()].content);

    rsx! {
        document::Title { "Documentation — Futuruna Programming Language" }
        document::Meta { name: "description", content: "Futuruna language reference: basics, runes, standard library, reactive streams, and Rust compatibility." }
        div { class: "docs-page",
            div { class: "docs-sidebar",
                h3 { class: "docs-sidebar-title", "Reference" }
                span { class: "docs-version", "v0.1.0" }
                for (i, page) in DOC_PAGES.iter().enumerate() {
                    button {
                        class: if active_doc() == i { "docs-sidebar-link active" } else { "docs-sidebar-link" },
                        onclick: move |_| active_doc.set(i),
                        "{page.label}"
                    }
                }
            }
            div { class: "docs-main",
                div { class: "docs-rendered", dangerous_inner_html: html_content }
            }
        }
    }
}

// ============================================================================
// Why page — /why
// ============================================================================

const DOC_WHY: &str = include_str!("../../docs/why.md");
const DOC_PHILOSOPHY: &str = include_str!("../../docs/research.md");
const DOC_OWNERSHIP: &str = include_str!("../../docs/research-ownership.md");

// Danish Constitution .runa files (chapters 1-11 + audit)
const DK_KAP01: &str = include_str!("../../examples/danish-constitution/kapitel-01.runa");
const DK_KAP02: &str = include_str!("../../examples/danish-constitution/kapitel-02.runa");
const DK_KAP03: &str = include_str!("../../examples/danish-constitution/kapitel-03.runa");
const DK_KAP04: &str = include_str!("../../examples/danish-constitution/kapitel-04.runa");
const DK_KAP05: &str = include_str!("../../examples/danish-constitution/kapitel-05.runa");
const DK_KAP06: &str = include_str!("../../examples/danish-constitution/kapitel-06.runa");
const DK_KAP07: &str = include_str!("../../examples/danish-constitution/kapitel-07.runa");
const DK_KAP08: &str = include_str!("../../examples/danish-constitution/kapitel-08.runa");
const DK_KAP09: &str = include_str!("../../examples/danish-constitution/kapitel-09.runa");
const DK_KAP10: &str = include_str!("../../examples/danish-constitution/kapitel-10.runa");
const DK_KAP11: &str = include_str!("../../examples/danish-constitution/kapitel-11.runa");
const DK_AUDIT: &str = include_str!("../../examples/danish-constitution/grundlov.audit.runa");

// Danish personal income tax website overview
const TAX_WEBSITE_OVERVIEW_MD: &str =
    include_str!("../../examples/danish-income-tax/website-overblik.md");

// US Constitution .runa files
const US_CONSTITUTION: &str = include_str!("../../examples/us-constitution/constitution.runa");
const US_ART1: &str = include_str!("../../examples/us-constitution/article-1.runa");
const US_ART1_S3: &str = include_str!("../../examples/us-constitution/article-1-section-3.runa");
const US_ART1_S4: &str = include_str!("../../examples/us-constitution/article-1-section-4.runa");
const US_ART1_S5: &str = include_str!("../../examples/us-constitution/article-1-section-5.runa");
const US_ART1_S6: &str = include_str!("../../examples/us-constitution/article-1-section-6.runa");
const US_ART1_S7: &str = include_str!("../../examples/us-constitution/article-1-section-7.runa");
const US_ART1_S8: &str = include_str!("../../examples/us-constitution/article-1-section-8.runa");
const US_ART1_S9: &str = include_str!("../../examples/us-constitution/article-1-section-9.runa");
const US_ART1_S10: &str = include_str!("../../examples/us-constitution/article-1-section-10.runa");
const US_ART2_S1: &str = include_str!("../../examples/us-constitution/article-2-section-1.runa");
const US_ART2_S2: &str = include_str!("../../examples/us-constitution/article-2-section-2.runa");
const US_ART2_S3: &str = include_str!("../../examples/us-constitution/article-2-section-3.runa");
const US_ART2_S4: &str = include_str!("../../examples/us-constitution/article-2-section-4.runa");
const US_ART3: &str = include_str!("../../examples/us-constitution/article-3.runa");
const US_ART4: &str = include_str!("../../examples/us-constitution/article-4.runa");
const US_ART5: &str = include_str!("../../examples/us-constitution/article-5.runa");
const US_ART6: &str = include_str!("../../examples/us-constitution/article-6.runa");
const US_ART7: &str = include_str!("../../examples/us-constitution/article-7.runa");
const US_SUCCESSION: &str = include_str!("../../examples/us-constitution/succession-act-1947.runa");
const US_VERIFICATION: &str = include_str!("../../examples/us-constitution/verification.runa");

/// Extract h2 headings from markdown source for TOC generation.
fn extract_h2_headings(md: &str) -> Vec<(String, String)> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let md = strip_markdown_frontmatter(md);
    let parser = Parser::new_ext(md, opts);
    let mut headings = Vec::new();
    let mut in_h2 = false;
    let mut text = String::new();
    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => {
                in_h2 = true;
                text.clear();
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                in_h2 = false;
                let slug = text
                    .trim()
                    .to_lowercase()
                    .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                    .replace(' ', "-");
                headings.push((slug, text.trim().to_string()));
            }
            Event::Text(t) if in_h2 => text.push_str(&t),
            Event::Code(t) if in_h2 => text.push_str(&t),
            _ => {}
        }
    }
    headings
}

/// Render markdown to HTML, injecting id attributes on h2 headings.
fn md_to_html_with_ids(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let md = strip_markdown_frontmatter(md);
    let parser = Parser::new_ext(md, opts);

    // Collect events and inject ids on h2 tags
    let mut events: Vec<Event> = Vec::new();
    let mut h2_text = String::new();
    let mut in_h2 = false;
    let mut h2_start_idx: Option<usize> = None;

    for event in parser {
        match &event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => {
                in_h2 = true;
                h2_text.clear();
                h2_start_idx = Some(events.len());
                events.push(event);
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                in_h2 = false;
                let slug = h2_text
                    .trim()
                    .to_lowercase()
                    .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                    .replace(' ', "-");
                // Replace the start tag with one that has an id
                if let Some(idx) = h2_start_idx {
                    events[idx] = Event::Html(format!("<h2 id=\"{}\">", slug).into());
                }
                events.push(Event::Html("</h2>".into()));
            }
            Event::Text(t) if in_h2 => {
                h2_text.push_str(t);
                events.push(event);
            }
            Event::Code(t) if in_h2 => {
                h2_text.push_str(t);
                events.push(event);
            }
            _ => {
                events.push(event);
            }
        }
    }

    let mut html_out = String::new();
    html::push_html(&mut html_out, events.into_iter());
    html_out
}

fn strip_markdown_frontmatter(md: &str) -> &str {
    let Some(rest) = md.strip_prefix("---\n") else {
        return md;
    };
    let Some(end) = rest.find("\n---\n") else {
        return md;
    };
    &rest[end + "\n---\n".len()..]
}

#[component]
fn WhyPage() -> Element {
    let headings = extract_h2_headings(DOC_WHY);
    let html_content = md_to_html_with_ids(DOC_WHY);

    rsx! {
        document::Title { "Why Futuruna — A Programming Language for Law" }
        document::Meta { name: "description", content: "Why Futuruna brings legal rules, defaults, exceptions, verification, and ordinary programming into one execution space." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Contents" }
                for (slug, label) in headings.iter() {
                    a { class: "why-toc-link", href: "#{slug}", "{label}" }
                }
                button {
                    class: "btn btn-print",
                    onclick: move |_| {
                        dioxus::document::eval("window.print()");
                    },
                    "Print"
                }
            }
            article { class: "why-main",
                div { class: "docs-rendered", dangerous_inner_html: html_content }
            }
        }
    }
}

// ============================================================================
// Research hub — /research (index) + sub-articles
// ============================================================================

#[component]
fn ResearchIndex() -> Element {
    rsx! {
        document::Title { "Research — Futuruna Programming Language" }
        document::Meta { name: "description", content: "Futuruna research and working models: executable constitutions, Danish tax law, language design, audits, and Rust ownership inference." }
        div { class: "research-hub",
            div { class: "research-header",
                h1 { class: "research-title", "Research" }
                p { class: "research-subtitle",
                    "Law as executable code, with research into syntax, audits, and compilation."
                }
            }
            div { class: "research-grid",
                // Language Philosophy
                a { class: "research-card", href: "/research/philosophy",
                    div { class: "research-card-rune rune-hash", "#" }
                    h2 { class: "research-card-title", "Philosophy of Futuruna" }
                    p { class: "research-card-desc",
                        "Partitioned optionality: how seven front runes create independent grammatical \
                         namespaces and support many paradigms without syntactic clutter."
                    }
                    span { class: "research-card-meta", "Language Design · Shannon Entropy" }
                }
                // Danish Constitution
                a { class: "research-card", href: "/research/danish-constitution",
                    div { class: "research-card-rune rune-flag", "\u{1F1E9}\u{1F1F0}" }
                    h2 { class: "research-card-title", "Danmarks Riges Grundlov" }
                    p { class: "research-card-desc",
                        "Den komplette danske grundlov af 1953 kodet i Futuruna. \
                         11 kapitler, 89 paragraffer: original lovtekst, typer, \
                         typede | regler, betingelser og undtagelser."
                    }
                    span { class: "research-card-meta", "Forfatningsret \u{00B7} Dansk" }
                }
                // Danish Constitution Audit
                a { class: "research-card", href: "/research/danish-constitution-audit",
                    div { class: "research-card-rune rune-question", "?" }
                    h2 { class: "research-card-title", "Grundlovsrevision" }
                    p { class: "research-card-desc",
                        "Auditlaget for den danske grundlovsmodel: tærskelsymmetrier, \
                         grundlovsparadokser, indfødsret/vælgerkorps, fattighjælp/valgret \
                         og påtrængende love før vælgerkontrol."
                    }
                    span { class: "research-card-meta", "Formel verifikation \u{00B7} Dansk grundlov" }
                }
                a { class: "research-card", href: "/research/personskatteloven",
                    div { class: "research-card-rune rune-pipe", "|" }
                    h2 { class: "research-card-title", "Personskatteloven" }
                    p { class: "research-card-desc",
                        "Et dansk overblik over Personskatteloven som eksekverbar \
                         Futuruna: lovtekst, regelkaskader, beregningseksempel \
                         og audits der kan finde hårde skatteforhold."
                    }
                    span { class: "research-card-meta", "Skatteret \u{00B7} Dansk \u{00B7} Projektstatus" }
                }
                // US Constitution
                a { class: "research-card", href: "/research/us-constitution",
                    div { class: "research-card-rune rune-question", "?" }
                    h2 { class: "research-card-title", "The United States Constitution" }
                    p { class: "research-card-desc",
                        "All 7 Articles of the US Constitution encoded in Futuruna. \
                         Electoral college, enumerated powers, separation of powers, \
                         and 65 cross-file verification proofs. ~1,200 lines."
                    }
                    span { class: "research-card-meta", "Constitutional Law — American" }
                }
                // Ownership Inference
                a { class: "research-card", href: "/research/ownership",
                    div { class: "research-card-rune rune-eq", "=" }
                    h2 { class: "research-card-title", "Invisible Ownership" }
                    p { class: "research-card-desc",
                        "76 adversarial patterns — self-referential structs, arena allocators, \
                         intrusive linked lists, async state machines — all compiling to valid Rust \
                         with zero ownership annotations. The inference algorithm, honest limits, \
                         and three bugs discovered."
                    }
                    span { class: "research-card-meta", "Ownership & Memory Safety" }
                }
            }
        }
    }
}

// ============================================================================
// Research: Language Philosophy — /research/philosophy
// ============================================================================

#[component]
fn ResearchPhilosophy() -> Element {
    philosophy_article()
}

// Preserve the former public URL for existing links.
#[component]
fn ResearchOptimization() -> Element {
    philosophy_article()
}

fn philosophy_article() -> Element {
    let headings = extract_h2_headings(DOC_PHILOSOPHY);
    let html_content = md_to_html_with_ids(DOC_PHILOSOPHY);

    rsx! {
        document::Title { "Philosophy of Futuruna — Partitioned Optionality" }
        document::Meta { name: "description", content: "How Futuruna's front runes create partitioned optionality: independent grammatical namespaces, Shannon information, and high paradigm coverage without syntactic clutter." }
        document::Link { rel: "canonical", href: "https://futuruna.com/research/philosophy" }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Philosophy" }
                a { class: "why-toc-link research-back", href: "/research", "← All Research" }
                for (slug, label) in headings.iter() {
                    a { class: "why-toc-link", href: "#{slug}", "{label}" }
                }
                button {
                    class: "btn btn-print",
                    onclick: move |_| { dioxus::document::eval("window.print()"); },
                    "Print"
                }
            }
            article { class: "why-main",
                div { class: "docs-rendered", dangerous_inner_html: html_content }
            }
        }
    }
}

// ============================================================================
// Research: Ownership Inference — /research/ownership
// ============================================================================

#[component]
fn ResearchOwnership() -> Element {
    let headings = extract_h2_headings(DOC_OWNERSHIP);
    let html_content = md_to_html_with_ids(DOC_OWNERSHIP);

    rsx! {
        document::Title { "Invisible Ownership — Futuruna Research" }
        document::Meta { name: "description", content: "How Futuruna infers Rust-level ownership without explicit annotations — escape analysis, borrow elimination, and the Kotlin-to-Rust philosophy." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Ownership" }
                a { class: "why-toc-link research-back", href: "/research", "← All Research" }
                for (slug, label) in headings.iter() {
                    a { class: "why-toc-link", href: "#{slug}", "{label}" }
                }
                button {
                    class: "btn btn-print",
                    onclick: move |_| { dioxus::document::eval("window.print()"); },
                    "Print"
                }
            }
            article { class: "why-main",
                div { class: "docs-rendered", dangerous_inner_html: html_content }
            }
        }
    }
}

// ============================================================================
// Research: Danish Constitution — /research/danish-constitution
// ============================================================================

/// Extract first comment block from a .runa file as description text.
fn extract_runa_header(src: &str) -> String {
    let mut lines = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("--") {
            lines.push(t.trim_start_matches("--").trim());
        } else if !t.is_empty() {
            break;
        }
    }
    lines.join(" ")
}

/// Render a constitution page: scrollable list of syntax-highlighted .runa files with TOC.
fn constitution_file_section(title: &str, id: &str, src: &str) -> String {
    let header = extract_runa_header(src);
    let highlighted = highlight_runa(src);
    let mut html = String::new();
    html.push_str(&format!("<div class=\"const-section\" id=\"{}\">", id));
    html.push_str(&format!("<h2 class=\"const-file-title\">{}</h2>", title));
    if !header.is_empty() {
        html.push_str(&format!("<p class=\"const-file-desc\">{}</p>", header));
    }
    html.push_str("<pre class=\"code-block const-code\"><code>");
    html.push_str(&highlighted);
    html.push_str("</code></pre></div>");
    html
}

#[component]
fn ResearchDanishConstitution() -> Element {
    let sections: Vec<(&str, &str, &str)> = vec![
        ("Kapitel I — Statsformen (§§ 1-4)", "kap-1", DK_KAP01),
        ("Kapitel II — Kongen (§§ 5-11)", "kap-2", DK_KAP02),
        (
            "Kapitel III — Kongen og ministrene (§§ 12-27)",
            "kap-3",
            DK_KAP03,
        ),
        ("Kapitel IV — Folketinget (§§ 28-34)", "kap-4", DK_KAP04),
        (
            "Kapitel V — Folketingets virksomhed (§§ 35-58)",
            "kap-5",
            DK_KAP05,
        ),
        ("Kapitel VI — Domstolene (§§ 59-65)", "kap-6", DK_KAP06),
        ("Kapitel VII — Folkekirken (§§ 66-70)", "kap-7", DK_KAP07),
        (
            "Kapitel VIII — Grundrettigheder (§§ 71-85)",
            "kap-8",
            DK_KAP08,
        ),
        (
            "Kapitel IX — Forskellige bestemmelser (§§ 86-87)",
            "kap-9",
            DK_KAP09,
        ),
        ("Kapitel X — Grundlovsændring (§ 88)", "kap-10", DK_KAP10),
        (
            "Kapitel XI — Overgangsbestemmelser (§ 89)",
            "kap-11",
            DK_KAP11,
        ),
    ];

    let toc: Vec<(String, String)> = sections
        .iter()
        .map(|(title, id, _)| (id.to_string(), title.to_string()))
        .collect();

    let body_html: String = sections
        .iter()
        .map(|(title, id, src)| constitution_file_section(title, id, src))
        .collect();

    rsx! {
        document::Title { "Danmarks Riges Grundlov — Futuruna Research" }
        document::Meta { name: "description", content: "The complete Danish Constitution of 1953 encoded in Futuruna: original source text, 89 paragraphs, 11 chapters, and | rule formulations for legal structure." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Grundlov" }
                a { class: "why-toc-link research-back", href: "/research", "← All Research" }
                for (id, label) in toc.iter() {
                    a { class: "why-toc-link", href: "#{id}", "{label}" }
                }
            }
            article { class: "why-main const-article",
                div { class: "const-intro",
                    p { class: "lang-note", "This page is in Danish — the constitution is encoded in its original language." }
                    h1 { "Danmarks Riges Grundlov" }
                    p {
                        "Den danske grundlov af 5. juni 1953, kodet i Futuruna. \
                         89 paragraffer fordelt p\u{00E5} 11 kapitler, hvor den originale \
                         lovtekst st\u{00E5}r i multiline source blocks og Futuruna-oversættelsen \
                         f\u{00F8}lger direkte nedenunder som typer, konstanter og typede | regler. \
                         Betingelser modelleres med "
                        code { "under" }
                        ", og undtagelser modelleres med "
                        code { "exception" }
                        "."
                    }
                    p { class: "lang-note",
                        "Kilde: "
                        a { href: "https://www.retsinformation.dk/eli/lta/1953/169", "Retsinformation, LOV nr. 169 af 05/06/1953" }
                        " · "
                        a { href: "https://www.ft.dk/da/dokumenter/bestil-publikationer/publikationer/grundloven/danmarks-riges-grundlov", "Folketingets tekstvisning" }
                    }
                    p { class: "const-stats",
                        "12 filer \u{00B7} 11 kapitler + revision \u{00B7} 89 paragraffer \u{00B7} typede | lovregler \u{00B7} officiel kilde citeret"
                    }
                    div { class: "const-analysis-strip",
                        a { href: "/research/danish-constitution-audit#indfoedsret-vaelgerkorps",
                            span { class: "const-analysis-kicker", "Ny audit" }
                            strong { "Indfødsret og vælgerkorps" }
                            small { "§§ 29, 41, 42, 44" }
                        }
                        a { href: "/research/danish-constitution-audit#fattighjaelp-valgret",
                            span { class: "const-analysis-kicker", "Ny audit" }
                            strong { "Fattighjælp og valgret" }
                            small { "§§ 29, 75" }
                        }
                        a { href: "/research/danish-constitution-audit#paatraengende-love",
                            span { class: "const-analysis-kicker", "Ny audit" }
                            strong { "Påtrængende love" }
                            small { "§ 42 stk. 7" }
                        }
                    }
                }
                div { dangerous_inner_html: body_html }
            }
        }
    }
}

// ============================================================================
// Research: Danish Constitution Audit — /research/danish-constitution-audit
// ============================================================================

const DK_AUDIT_ARTICLE: &str = r#"## Hvad er en grundlovsrevision?

En grundlov er et system af regler, og som ethvert system kan den indeholde huller,
spændinger og paradokser, der først bliver synlige når man formaliserer reglerne
præcist nok til at teste dem.

**Grundlovsrevisionen** tager enhver regel, tærskel, delegering og garanti
i den danske grundlov og udtrykker dem først som kildefaste Futuruna-regler.
Auditlaget kan derefter bevise, sammenligne og finde huller i regelstrukturen.
Ikke ved juridisk argumentation, men ved eksekvering.

Kildeteksten er Danmarks Riges Grundlov, LOV nr. 169 af 05/06/1953, med
Retsinformation som officiel tekstgrundlag og Folketingets tekstvisning som
parlamentarisk spejl. I kapitel-filerne står den originale lovtekst i
`----` blokke; Futuruna-oversættelsen står direkte nedenunder.

## Sådan virker det

Hver paragraf i grundloven kodes med Futurunas syv runer:

- **`#` (typer)** definerer de forfatningsmæssige aktører: `Monark`, `Tronfølger`,
  `Rigsdel`, `Statsmagt`, `Samtykke`
- **`|` (regler)** koder de juridiske udsagn: pligter, forbud, beføjelser,
  delegationer og tærskler som `har_valgret()`, `personlig_frihed_er_ukrænkelig()`
  og `grundlovsændring_godkendt()`
- **`>` (funktioner)** reserveres til egentlig beregning uden selvstændig
  retsnorm; lovformuleringen skal som udgangspunkt være `|`
- **`=` (bindinger)** fastsætter forfatningskonstanter: `frist_fremstilling_timer = 24`,
  `godkendelsestærskel_pct = 40`
- **`?` (beviser)** verificerer at hver invariant holder

## Hvad revisionen opdager

### Tærskelsymmetrier
Flere paragraffer deler de samme brøktærskler uden at krydsreferere hinanden:
- §§ 39 og 41 bruger begge en **2/5-tærskel** (indkaldelse og udsættelse)
- §§ 42 og 73 bruger begge en **1/3-tærskel** (folkeafstemning og ekspropriationsudsættelse)

Revisionen beviser at disse er *strukturelt identiske*. Ikke tilfældigheder, men
forfatningsdesign.

### Grundlovsparadokser
- **§ 15 vs § 32 stk. 2 — Dødvande**: En ny regering, der ikke har præsenteret
  sig for Folketinget, kan hverken blive siddende (§ 15 forbyder det efter mistillidsvotum)
  eller udskrive valg (§ 32 kræver præsentation først). Begge udgange er blokeret.
- **§ 7 vs § 8 — Den umyndige monark med ed**: § 8 siger at en arving, der allerede
  har aflagt eden, tiltræder straks ved tronfølge. § 7 siger monarken skal være 18.
  Ingen alderstjek i § 8.
- **§ 6 vs § 70 — Den muslimske konge**: § 6 *kræver* at monarken tilhører
  den evangelisk-lutherske kirke. § 70 siger at *ingen* kan berøves rettigheder
  på grund af trosbekendelse. Grundloven forbyder hvad den selv kræver.
  Endnu vigtigere: § 6 gælder kongen, men *ikke* tronfølgeren. Troskravet
  nævnes kun for den siddende monark. Hvad sker der når en muslimsk
  tronfølger arver tronen via § 2? Grundloven er tavs. Revisionen tester dette
  med en `muslim_arving` og viser at `opfylder_troskrav()` returnerer `Falskt`.
  Ingen paragraf forhindrer arvefølgen.

### Beskatning vs. ekspropriation: det usynlige hul
§ 43 siger: ingen skat uden lov. § 73 siger: ejendomsretten er ukrænkelig,
og ekspropriation kræver fuldstændig erstatning.

Men grundloven definerer *ikke* grænsen mellem beskatning og ekspropriation.
En skat på 100% er forfatningsmæssigt en "skat" under § 43, som blot kræver
en lov. I praksis er det en total ekspropriation, som under § 73 ville
kræve fuldstændig erstatning. Revisionen beviser at begge paragraffer holder
isoleret (`par43_skat_kræver_lov` og `par73_ekspropriation_kræver_erstatning`
består begge). Men en konfiskatorisk skattesats opfylder § 43 og omgår § 73.
Ingen bestemmelse forbyder det. Ingen støttebestemmelse definerer hvornår
beskatning bliver ekspropriation.

<span id="indfoedsret-vaelgerkorps"></span>

### Indfødsret og vælgerkorps uden referendum

§ 44 siger at ingen udlænding kan få indfødsret uden ved lov. § 29 gør
indfødsret til adgangsbillet til valgret. Samtidig er indfødsretslove afskåret
fra folkeafstemning efter § 42 stk. 6, og de kan heller ikke bremses med
12-dages udsættelsen i § 41 stk. 3.

Revisionen koder dette som en særskilt lovtype:

```runa
# AfskærmetLovtype = Indfødsretslov | Ekspropriationslov | DirekteSkattelov | IndirekteSkattelov | Traktatgennemførelseslov

| folkeafstemning_afskåret(lovtype: AfskærmetLovtype) -> lovtype == Indfødsretslov || lovtype == Ekspropriationslov || lovtype == DirekteSkattelov || lovtype == IndirekteSkattelov || lovtype == Traktatgennemførelseslov
| udsættelse_af_tredje_behandling_afskåret(lovtype: AfskærmetLovtype) -> lovtype == Indfødsretslov || lovtype == Ekspropriationslov || lovtype == IndirekteSkattelov
| dobbelt_afskåret(lovtype: AfskærmetLovtype) -> folkeafstemning_afskåret(lovtype) && udsættelse_af_tredje_behandling_afskåret(lovtype)
```

Det interessante er ikke bare at indfødsret gives ved lov. Det er at
vælgerkorpsets medlemskreds kan udvides via en lovtype, som grundloven selv
afskærer fra de normale vælgerkontrolmekanismer. Valgretsalderen er låst af
folkeafstemning; indfødsretsadgangen er ikke.

<span id="fattighjaelp-valgret"></span>

### Fattighjælp som valgretsrisiko

§ 75 giver ret til offentlig hjælp ved manglende forsørgelse. § 29 lader
derimod loven bestemme i hvilket omfang understøttelse, der betragtes som
fattighjælp, medfører tab af valgret.

Revisionen modellerer det som en betinget undtagelse:

```runa
# ForsørgelsesStatus = Selvforsørgende | Fattighjælp

| hjælp_kan_have_valgretspris(status: ForsørgelsesStatus) -> Falskt
| exception fattighjælp hjælp_kan_have_valgretspris(status: ForsørgelsesStatus) -> Sandt under status == Fattighjælp
```

Dermed bliver spændingen synlig: social beskyttelse og politisk deltagelse er
ikke fuldt adskilte spor i teksten. Hjælp er en rettighed, men en bestemt
historisk kategori af hjælp kan stadig være en lovkanal til tab af valgret.

<span id="paatraengende-love"></span>

### Påtrængende love før vælgerkontrol

§ 42 stk. 7 siger at et særdeles påtrængende lovforslag kan stadfæstes straks,
hvis forslaget selv bestemmer det. Hvis en tredjedel derefter kræver
folkeafstemning og loven forkastes, bortfalder loven først fra
kundgørelsesdagen.

Revisionen koder tidsstillingen direkte:

```runa
# PåtrængendeLovStadium = FørFolkeafstemning | EfterForkastelseKundgjort

| påtrængende_lov_virker(stadium: PåtrængendeLovStadium) -> påtrængende_lovforslag_kan_stadfæstes_straks() under stadium == FørFolkeafstemning
| exception forkastet påtrængende_lov_virker(stadium: PåtrængendeLovStadium) -> Falskt under stadium == EfterForkastelseKundgjort
```

Vælgerkontrollen er derfor ikke nødvendigvis opsættende. I den påtrængende
lovkanal kan loven virke først og bortfalde bagefter.

### Delegeringssporing
Revisionen identificerer ethvert punkt hvor grundloven delegerer til almindelig lovgivning
(31 delegeringer på tværs af alle kapitler), og synliggør grænsen mellem
forfatningsgaranti og lovgivningsmæssigt skøn.

### De fire ukrænkeligheder
Fire ting erklæres "ukrænkelige": Folketinget (§ 34),
den personlige frihed (§ 71), boligen (§ 72) og ejendomsretten (§ 73). Hver har
forskellige undtagelsesmekanismer. Revisionen kortlægger og sammenligner dem.

## 100+ verificerede invarianter

Enhver invariant i revisionsfilen er maskintjekket. Den afsluttende linje:

```
? all -> { @ skriv("Alle invarianter holder.") }
```

Den kører ethvert `?`-bevis. Hvis en invariant fejler, rapporterer programmet det.
Alle består.
"#;

#[component]
fn ResearchDanishConstitutionAudit() -> Element {
    let article_html = md_to_html_with_ids(DK_AUDIT_ARTICLE);
    let audit_highlighted = constitution_file_section(
        "grundlov.audit.runa \u{2014} Formelle invarianter og krydskontrol",
        "audit-code",
        DK_AUDIT,
    );

    rsx! {
        document::Title { "Grundlov Audit — Futuruna Research" }
        document::Meta { name: "description", content: "Formal audit layer for the Danish Constitution model in Futuruna: threshold symmetries, constitutional tensions, delegation tracking, and cross-chapter checks." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Revision" }
                a { class: "why-toc-link research-back", href: "/research", "\u{2190} Al forskning" }
                a { class: "why-toc-link", href: "#article", "Artikel" }
                a { class: "why-toc-link", href: "#audit-code", "Kildekode" }
            }
            article { class: "why-main const-article",
                div { class: "const-intro",
                    p { class: "lang-note", "This page is in Danish — the audit analyses the constitution in its original language." }
                    h1 { "Grundlovsrevision" }
                    p {
                        "Auditlaget samler formelle invarianter p\u{00E5} tv\u{00E6}rs af alle 11 kapitler \
                         i den danske grundlov: tærskelsymmetrier, grundlovsparadokser, \
                         delegeringssporing, de fire ukrænkeligheder og nye krydsanalyser \
                         af indfødsret, fattighjælp og påtrængende love."
                    }
                    div { class: "const-audit-highlights",
                        a { href: "#indfoedsret-vaelgerkorps",
                            span { "|" }
                            strong { "Indfødsret ændrer vælgerkorps uden referendum" }
                        }
                        a { href: "#fattighjaelp-valgret",
                            span { "exception" }
                            strong { "Fattighjælp kan blive valgretspris" }
                        }
                        a { href: "#paatraengende-love",
                            span { "under" }
                            strong { "Påtrængende love virker før vælgerkontrol" }
                        }
                    }
                }
                div { id: "article", class: "docs-rendered", dangerous_inner_html: article_html }
                div { dangerous_inner_html: audit_highlighted }
            }
        }
    }
}

// ============================================================================
// Research: Personskatteloven — /research/personskatteloven
// ============================================================================

#[component]
fn ResearchPersonskatteloven() -> Element {
    let project_file_html = md_to_html_with_ids(TAX_WEBSITE_OVERVIEW_MD);

    rsx! {
        document::Title { "Personskatteloven — Futuruna-forskning" }
        document::Meta { name: "description", content: "Dansk personskat modelleret i Futuruna: et samlet sprog til lov, regelkaskader, beregning og audit af hårde skatteforhold." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "Personskat" }
                a { class: "why-toc-link research-back", href: "/research", "\u{2190} Al forskning" }
                a { class: "why-toc-link", href: "#intro", "Intro" }
                a { class: "why-toc-link", href: "#personskatteloven", "Indkomstskat" }
                a { class: "why-toc-link", href: "#example", "Eksempel" }
                a { class: "why-toc-link", href: "#audit-signals", "Hårde forhold" }
                a { class: "why-toc-link", href: "#project-file", "Projektfil" }
                a { class: "why-toc-link", href: "#source-posture", "Kilder" }
            }
            main { class: "why-main const-article tax-article",
                div { id: "intro", class: "const-intro",
                    p { class: "lang-note", "Denne side er på dansk, fordi Personskatteloven er dansk. Selve lovteksten bliver i Futuruna-projektfilerne, lige over de regler der oversætter den." }
                    h1 { "Et samlet sprog til lov og ret" }
                    p {
                        "Futuruna er et forsøg på at give lov og ret et samlet eksekverbart \
                         sprog uden at fjerne lovens juridiske form. En paragraf kan stå som \
                         original dansk tekst og derefter som typede regler, betingelser, \
                         undtagelser og audit-spørgsmål."
                    }
                    p {
                        "Det betyder indkapslede regler for juridiske situationer, \
                         regelkaskader for de beløb der følger af andre beløb, og \
                         audit-muligheder for de steder hvor et regelsystem bliver hårdt, \
                         overraskende eller uklart."
                    }
                    p {
                        "Derfor er "
                        code { "|" }
                        " central. Den formulerer, hvad der skal gælde. "
                        code { "under" }
                        " holder betingelser synlige, "
                        code { "exception" }
                        " holder undtagelser synlige, og "
                        code { "?" }
                        " gør reglen til noget, der kan spørges til."
                    }
                    p { class: "const-stats", "Dansk overblik \u{00B7} én webvist projektfil \u{00B7} lovkorpus i repoet \u{00B7} original dansk lovtekst bevares i Futuruna-filerne" }
                }

                section { id: "personskatteloven", class: "tax-section",
                    h2 { "Personskatteloven (indkomstskat)" }
                    p {
                        "Personskatteloven er den rigtige prøve, fordi dansk indkomstskat ikke \
                         er én isoleret formel. En virkelig beregning rammer også \
                         arbejdsmarkedsbidrag, kommunal skat, kirkeskat, kildeskat, \
                         forskudsregistrering, slutopgørelse, aktieindkomst, kapitalindkomst, \
                         ægtefælleregler, underskud og afhængige love."
                    }
                    p {
                        "Futuruna gør det muligt at tage den samlede danske \
                         indkomstskattelovgivning og formulere den i ét sprog, så den samme \
                         regelkæde både kan udregne samlet skat for almindelige borgere, \
                         forklare hvilke bestemmelser der bar beregningen, og danne grundlag \
                         for audits af retlige knæk."
                    }
                    div { class: "tax-status-grid",
                        div { class: "tax-status-item ready",
                            span { class: "tax-status-label", "Status" }
                            strong { "Beregningsegnet, men ikke færdig" }
                            p { "Den almindelige lønmodtagervej, flere kapitalindkomstgrene, aktieindkomst, pensionsfradrag efter Pensionsbeskatningsloven §§ 18 og 52, husdyr- og varelagerfradrag samt beløbskaskaden for § 3, stk. 2, nr. 10-11 er eksekverbare. Den ordinære personaktievej beregner nu anskaffelser, gennemsnitlig anskaffelsessum, afståelser, noterede og unoterede tab, ægtefælleoverførsel og den afledte § 4 a-aktieindkomst efter Aktieavancebeskatningsloven §§ 12-15 og de anvendte dele af §§ 23-26. Afskrivningslovens kilde- og regelkorpus dækker nu hele paragrafsekvensen §§ 1-69, inklusive ophævede bestemmelser og historiske overgangsregler. §§ 50-52 håndterer næringsaktiver, forsøgs- og forskningsudgifter samt ansøgningsfrister; §§ 54-62 og 68-69 gør overgangssaldi, ældre afskrivningsgrundlag, miljøinvesteringer, udlejningsregimet og territorial afgrænsning eksekverbare. LOV 615/2026's ejendomskategorier og 2027-virkning er indarbejdet i §§ 40 C og 42. Personskatteloven § 3 modtager afledte fortjenester, afskrivninger, tab og andre fradrag som særskilte typede poster, mens § 35 fortsat peger på de oprindelige indkomstår. Andre Personskattelov-bestemmelser, afhængige love, personfradrag, underskud, delår, skatteloft, indeholdelse og slutopgørelsens yderkanter er fortsat åbne. Fuld lovdækning er stadig målet, ikke noget vi påstår er afsluttet." }
                        }
                        div { class: "tax-status-item research",
                            span { class: "tax-status-label", "Form" }
                            strong { "Lovtekst først, regler bagefter" }
                            p { "I repoet gentages strukturen: original dansk lovtekst i flerlinjeblok, kun en note hvis nødvendigt, og derefter egentlige Futuruna-regler. Websitet gengiver ikke hele korpusset." }
                        }
                        div { class: "tax-status-item research",
                            span { class: "tax-status-label", "Input" }
                            strong { "Typet JSON, TOML og XLSX" }
                            p { "Lønmodtagerberegningen har nu én kildebundet kontrakt, som kan generere inputskabeloner, validere udfyldte sager og returnere det fulde skatteresultat. Relaterede regnearksfaner oprettes kun for faktiske samlinger i domænemodellen." }
                        }
                    }
                }

                section { id: "example", class: "tax-section",
                    h2 { "Eksempel" }
                    p {
                        "En fiktiv mand tjener 50.000 kr. om måneden før skat. Han bor til leje \
                         for 10.000 kr. om måneden, er gift, har tre børn på 2, 7 og 10 år, og \
                         ægtefællen tjener 20.000 kr. om måneden. Den nuværende scenario-fil \
                         modellerer 2026, København, ingen kirkeskat, ingen positiv nettokapitalindkomst \
                         og ingen ekstra ligningsmæssige fradrag."
                    }
                    div { class: "tax-source-grid",
                        div { class: "tax-source-row",
                            span { "Mand" }
                            strong { "Årlig skat inkl. AM efter personfradrag: 208.726 kr. — ca. 17.393 kr. pr. måned" }
                        }
                        div { class: "tax-source-row",
                            span { "Husholdning" }
                            strong { "Samlet årlig skat: 279.731 kr. — samlet netto: 46.689 kr. pr. måned" }
                        }
                        div { class: "tax-source-row",
                            span { "Efter husleje" }
                            strong { "36.689 kr. pr. måned efter 10.000 kr. husleje i den årlige skatteberegningsmodel" }
                        }
                        div { class: "tax-source-row",
                            span { "Afgrænsning" }
                            strong { "Børneydelser, boligstøtte og anden social ydelsesret ligger uden for denne Personskatteloven/Kildeskatteloven-slice" }
                        }
                    }
                    p { class: "lang-note",
                        "Scenario: "
                        code { "examples/danish-income-tax/husholdning-scenarier.scenario.runa" }
                        " og "
                        code { "examples/danish-income-tax/slutopgoerelse.scenario.runa" }
                    }
                }

                section { id: "audit-signals", class: "tax-section",
                    h2 { "Hårde skatteforhold" }
                    p {
                        "Futuruna kan også bruges den anden vej: ikke kun til at beregne én borger, \
                         men til at søge i et afgrænset rum af konfigurationer og spørge, hvornår \
                         regelsystemet giver mærkelige eller hårde resultater."
                    }
                    p {
                        "For eksempel kan den aktuelle konfiskatoriske audit søge 8.064 \
                         kombinationer og finde ud af, at der i det afgrænsede søgeområde ikke \
                         er almindelig årsskat over 100 pct. af positivt indkomstgrundlag. Den \
                         finder derimod 360 konfigurationer af skatteforhold, hvor den samlede \
                         betalingsbelastning overstiger 100 pct. af årets indkomstgrundlag."
                    }
                    p {
                        "Før man finder høtyvene frem, er forklaringen vigtig: de fund skyldes \
                         overført restskat m.v. Det er ikke en skjult almindelig skattesats over \
                         100 pct., men et betalingsproblem fra tidligere år, som Futuruna kan gøre \
                         præcist, synligt og auditérbart."
                    }
                    div { class: "const-audit-highlights tax-audit-highlights",
                        a { href: "#project-file",
                            span { "årsskat" }
                            strong { "0 fund over 100 pct. i det aktuelle bounded search-rum" }
                        }
                        a { href: "#project-file",
                            span { "betaling" }
                            strong { "360 betalingsbelastningsfund over 100 pct. med overført restskat" }
                        }
                        a { href: "#project-file",
                            span { "forklaring" }
                            strong { "Restskat adskilles fra ordinær current-year Personskatteloven-skat" }
                        }
                    }
                }

                section { id: "project-file", class: "tax-section",
                    h2 { "Projektfil" }
                    p { "Websitet gengiver kun denne ene danske overbliksfil, ikke et parallelt lovkorpus. De konkrete lovregler, audit-filer og scenarier ligger i Futuruna-projektet og er ikke foldet ud på siden." }
                    div { class: "docs-rendered tax-milestones", dangerous_inner_html: project_file_html }
                }

                section { id: "source-posture", class: "tax-section",
                    h2 { "Kilder" }
                    p { class: "lang-note",
                        "Kildegrundlag: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2021/1284", "Retsinformation, LBK nr. 1284 af 14/06/2021" }
                        " · ændringer: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2023/1564", "LOV nr. 1564/2023" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2024/482", "LOV nr. 482/2024" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2024/1691", "LOV nr. 1691/2024" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2026/615", "LOV nr. 615/2026" }
                        " · Ligningsloven: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1500", "LBK nr. 1500/2025" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/198", "LOV nr. 198/2025" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1333", "BEK nr. 1333/2025" }
                        " · Pensionsbeskatningsloven: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2024/1243", "LBK nr. 1243/2024" }
                        " / "
                        a { href: "https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/pensionsbeskatningsloven", "Skatteministeriets beløbsgrænser" }
                        " · husdyr og varelager: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1099", "Husdyrbeskatningsloven, LBK nr. 1099/2025" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/1981/543", "BEK nr. 543/1981" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1088", "Varelagerloven, LBK nr. 1088/2025" }
                        " · afskrivning og iværksætterkonto: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1222", "Afskrivningsloven, LBK nr. 1222/2025" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/749", "ændringslov nr. 749/2025" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2026/615", "ændringslov nr. 615/2026" }
                        " / "
                        a { href: "https://skm.dk/tal-og-metode/satser/satser-og-beloebsgraenser-i-lovgivningen/afskrivningsloven", "Skatteministeriets 2026-satser" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2060781", "Skattestyrelsens saldovejledning" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2060787", "Skattestyrelsens vejledning til § 6" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2060792", "Skattestyrelsens vejledning til §§ 11-13" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083984", "Skattestyrelsens bygningsafgrænsning til § 14" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083985", "Skattestyrelsens installationsvejledning til § 15" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083987", "Skattestyrelsens afskrivningsmetoder til §§ 16-20" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083993", "Skattestyrelsens vejledning til § 38" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083992", "Skattestyrelsens vejledning til § 39" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083994", "Skattestyrelsens vejledning til § 40" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083996", "Skattestyrelsens vejledning til § 42" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083997", "Skattestyrelsens vejledning til § 43" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2061440", "Skattestyrelsens vejledning til § 44" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2083999", "Skattestyrelsens vejledning til §§ 44 A-44 B" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=2060796", "Skattestyrelsens vejledning til § 44 C" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=1976528", "Skattestyrelsens kontantomregning efter § 45" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=1976529", "Skattestyrelsens aktivfordeling efter § 45" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=1976531", "Skattestyrelsens vejledning til §§ 47-49" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=1976532", "Skattestyrelsens vejledning til § 50" }
                        " / "
                        a { href: "https://info.skat.dk/data.aspx?oid=1976534", "Skattestyrelsens vejledning til § 51" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/1922/149", "Statsskatteloven, LOV nr. 149/1922" }
                        " / "
                        a { href: "https://www.retsinformation.dk/eli/lta/2025/1307", "Etableringskontoloven, LBK nr. 1307/2025" }
                        " · historisk linje: "
                        a { href: "https://www.retsinformation.dk/eli/lta/2019/799", "LBK nr. 799 af 07/08/2019" }
                    }
                    div { class: "tax-source-grid",
                        div { class: "tax-source-row",
                            span { "Arbejdskilde" }
                            strong { "2021/1284 — gældende konsolideret Personskattelov med sporede ændringslove" }
                        }
                        div { class: "tax-source-row",
                            span { "Beregningværn" }
                            strong { "Historiske kilder må ikke drive aktuel beregning uden eksplicit kildepostur" }
                        }
                        div { class: "tax-source-row",
                            span { "Afhængigheder" }
                            strong { "AM-bidrag, kommunal/kirkelig skat, Kildeskatteloven, Ligningsloven, Kursgevinstloven, Virksomhedsskatteloven, Pensionsbeskatningsloven, Afskrivningsloven, Statsskatteloven, Etableringskontoloven, Husdyrbeskatningsloven, Varelagerloven, Selskabsskatteloven og flere andre kilder modelleres ved behov" }
                        }
                        div { class: "tax-source-row",
                            span { "Websitegrænse" }
                            strong { "Denne side er kun overblikket; den fulde lovoversættelse, audits og scenarier bor i Futuruna-projektet" }
                        }
                    }
                }

            }
        }
    }
}

// ============================================================================
// Research: US Constitution — /research/us-constitution
// ============================================================================

#[component]
fn ResearchUSConstitution() -> Element {
    let sections: Vec<(&str, &str, &str)> = vec![
        (
            "constitution.runa — Preamble & Foundation",
            "preamble",
            US_CONSTITUTION,
        ),
        ("Article I — Legislative Branch (§§ 1-2)", "art-1", US_ART1),
        ("Article I, Section 3 — The Senate", "art-1-s3", US_ART1_S3),
        ("Article I, Section 4 — Elections", "art-1-s4", US_ART1_S4),
        (
            "Article I, Section 5 — Rules of Each House",
            "art-1-s5",
            US_ART1_S5,
        ),
        ("Article I, Section 6 — Privileges", "art-1-s6", US_ART1_S6),
        (
            "Article I, Section 7 — Bills & Veto",
            "art-1-s7",
            US_ART1_S7,
        ),
        (
            "Article I, Section 8 — Enumerated Powers",
            "art-1-s8",
            US_ART1_S8,
        ),
        (
            "Article I, Section 9 — Limits on Congress",
            "art-1-s9",
            US_ART1_S9,
        ),
        (
            "Article I, Section 10 — Limits on States",
            "art-1-s10",
            US_ART1_S10,
        ),
        (
            "Article II, Section 1 — Executive Power",
            "art-2-s1",
            US_ART2_S1,
        ),
        (
            "Article II, Section 2 — Presidential Powers",
            "art-2-s2",
            US_ART2_S2,
        ),
        (
            "Article II, Section 3 — Presidential Duties",
            "art-2-s3",
            US_ART2_S3,
        ),
        (
            "Article II, Section 4 — Impeachment",
            "art-2-s4",
            US_ART2_S4,
        ),
        ("Article III — Judicial Power", "art-3", US_ART3),
        ("Article IV — States Relations", "art-4", US_ART4),
        ("Article V — Amendment Process", "art-5", US_ART5),
        ("Article VI — Supremacy Clause", "art-6", US_ART6),
        ("Article VII — Ratification", "art-7", US_ART7),
        (
            "Presidential Succession Act (1947)",
            "succession",
            US_SUCCESSION,
        ),
        (
            "Verification — Cross-File Proofs",
            "verification",
            US_VERIFICATION,
        ),
    ];

    let toc: Vec<(String, String)> = sections
        .iter()
        .map(|(title, id, _)| (id.to_string(), title.to_string()))
        .collect();

    let body_html: String = sections
        .iter()
        .map(|(title, id, src)| constitution_file_section(title, id, src))
        .collect();

    rsx! {
        document::Title { "US Constitution in Futuruna — Research" }
        document::Meta { name: "description", content: "The United States Constitution encoded in Futuruna: Articles I-VII, presidential succession, and cross-file verification proofs with computable invariants." }
        div { class: "why-page",
            nav { class: "why-toc",
                h3 { class: "why-toc-title", "US Const." }
                a { class: "why-toc-link research-back", href: "/research", "← All Research" }
                for (id, label) in toc.iter() {
                    a { class: "why-toc-link", href: "#{id}", "{label}" }
                }
            }
            article { class: "why-main const-article",
                div { class: "const-intro",
                    h1 { "The United States Constitution" }
                    p {
                        "All 7 Articles of the US Constitution encoded in Futuruna. \
                         The electoral college, enumerated powers, separation of powers, \
                         impeachment procedures, and the amendment process — formalized as \
                         types, rules, invariants, and cross-file verification proofs."
                    }
                    p { class: "const-stats",
                        "21 files — ~1,200 lines — 65 cross-article proofs"
                    }
                }
                div { dangerous_inner_html: body_html }
            }
        }
    }
}

// ============================================================================
// Footer
// ============================================================================

#[component]
fn Footer() -> Element {
    rsx! {
        footer { class: "footer",
            div { class: "footer-inner",
                p { "Andreas Rudolph · Researcher · Copenhagen · X: ",
                    a { href: "https://x.com/OneManMobile", "@OneManMobile" }
                }
                p { "Futuruna — a programming language for law, built through Rust" }
                div { class: "footer-links",
                    a { href: "https://github.com/Futuruna/futuruna", "GitHub" }
                    span { class: "footer-sep", "|" }
                    a { href: "/docs", "Documentation" }
                    span { class: "footer-sep", "|" }
                    a { href: "/playground", "Playground" }
                }
            }
        }
    }
}
