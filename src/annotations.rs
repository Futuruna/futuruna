//! Annotations that describe code: `@ section`, `@ quote`, `@ cite` and
//! `@ meta`.
//!
//! The lexer reads each such annotation line whole ([`LexedAnnotation`]) and
//! emits no tokens for it, so the program's statements are the same with or
//! without them. This module attaches every annotation to the item on the
//! line below it (comments may stand between, a blank line may not) and
//! reports an annotation that describes nothing it may describe:
//!
//! ```text
//! @ meta(Assumption(value = assumption_ask_tax_paid_and_room))
//! @ section ask_ceiling {
//!
//! @ quote(Source(value = aktiesparekontoloven))
//! ----
//! § 9. Personer, der er fuldt skattepligtige, ...
//! ----
//!
//! @ cite(§ 9, stk. 3)
//! | excess_levy_ore(excess_days_ore: Ore, levy_bp: Int) -> ...
//!
//! }
//! ```
//!
//! A section opens no scope: its items are top-level items of the file. A
//! quote's document is a binding whose type implements the marker trait
//! `Document` (a field `file: String`, relative to the project root).

use super::*;

/// One `@ meta(<value>)` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetaAnnotation {
    pub(crate) line: usize,
    /// The value as written.
    pub(crate) value: String,
    /// `when <condition>`: on a rule clause or member, an `Assumption` that
    /// holds only when the condition does, as written.
    pub(crate) condition: Option<String>,
}

/// One `@ quote(<document>)` and its `----` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuoteAnnotation {
    pub(crate) line: usize,
    /// The argument as written: `aktiesparekontoloven` or
    /// `Source(value = aktiesparekontoloven)`.
    pub(crate) argument: String,
    /// The document's binding.
    pub(crate) document: String,
    /// The role constructor, when the argument is a role value.
    pub(crate) role: Option<String>,
    /// The line of the opening `----`.
    pub(crate) open_line: usize,
    /// The line of the closing `----`.
    pub(crate) close_line: usize,
    /// The quoted lines, verbatim.
    pub(crate) text: String,
}

impl QuoteAnnotation {
    /// The source line of the first quoted line.
    pub(crate) fn text_start_line(&self) -> usize {
        self.open_line + 1
    }
}

/// `@ section <name> { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) name: String,
    /// The line of `@ section`.
    pub(crate) line: usize,
    /// The line of the closing `}`.
    pub(crate) end_line: usize,
    /// The `@ meta` lines above `@ section`.
    pub(crate) metas: Vec<MetaAnnotation>,
    pub(crate) quotes: Vec<QuoteAnnotation>,
}

/// The declaration, clause or member below an annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    /// `type`, `binding`, `function`, `rule` (a top-level rule clause) or
    /// `member` (a clause inside a RuleScope body).
    pub(crate) kind: &'static str,
    pub(crate) name: String,
    pub(crate) line: usize,
    /// The item's last line.
    pub(crate) end_line: usize,
}

/// `@ meta` lines on a declaration, clause or member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemMeta {
    pub(crate) metas: Vec<MetaAnnotation>,
    pub(crate) item: Item,
    /// The section the item stands in.
    pub(crate) section: Option<String>,
}

/// One `@ cite(...)` and the clause, member or function it cites for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CiteAnnotation {
    pub(crate) line: usize,
    pub(crate) text: String,
    /// The line of the clause, member or function below.
    pub(crate) clause_line: usize,
    /// The section around the clause.
    pub(crate) section: Option<String>,
}

/// `@ meta(<value>) for <name>`: metadata on a declaration written apart
/// from it, standing alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetaFor {
    pub(crate) line: usize,
    /// The value as written.
    pub(crate) value: String,
    /// The declaration it describes, as named after `for`.
    pub(crate) target: String,
}

/// The annotations of one source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Annotations {
    pub(crate) sections: Vec<Section>,
    pub(crate) item_metas: Vec<ItemMeta>,
    pub(crate) citations: Vec<CiteAnnotation>,
    /// `@ meta(...) for <name>` lines.
    pub(crate) metas_for: Vec<MetaFor>,
    /// Misplaced annotations, by line.
    pub(crate) errors: Vec<(usize, String)>,
}

impl Annotations {
    pub(crate) fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.name == name)
    }

    /// The section around `line`.
    pub(crate) fn section_at(&self, line: usize) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.line < line && line < section.end_line)
    }
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("--") && !trimmed.starts_with("----")
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The first line after `after` that is not a comment and not a
/// declaration annotation (`@ calculate(...)`, `@ export`, `@ field(...)`),
/// and whether a blank line stands before it.
fn next_item_line(
    lines: &[&str],
    after: usize,
    annotation_lines: &BTreeSet<usize>,
) -> (Option<usize>, bool) {
    let mut blank = false;
    let mut line = after + 1;
    while line <= lines.len() {
        let trimmed = lines[line - 1].trim();
        if trimmed.is_empty() {
            blank = true;
            line += 1;
            continue;
        }
        if is_comment_line(trimmed) {
            line += 1;
            continue;
        }
        if trimmed.starts_with('@') && !annotation_lines.contains(&line) {
            // A declaration annotation is part of the declaration; skip it,
            // with any continuation lines of its arguments.
            let mut depth = 0i64;
            loop {
                for character in lines[line - 1].chars() {
                    match character {
                        '(' | '[' => depth += 1,
                        ')' | ']' => depth -= 1,
                        _ => {}
                    }
                }
                line += 1;
                if depth <= 0 || line > lines.len() {
                    break;
                }
            }
            continue;
        }
        return (Some(line), blank);
    }
    (None, blank)
}

/// The last line of the declaration or clause starting on `line`: the lines
/// after it that are indented deeper, or close a bracket at its indentation.
fn item_end_line(lines: &[&str], line: usize, stop_lines: &BTreeSet<usize>) -> usize {
    let base = indentation(lines[line - 1]);
    let mut last = line;
    let mut next = line + 1;
    while next <= lines.len() {
        if stop_lines.contains(&next) {
            break;
        }
        let text = lines[next - 1];
        let trimmed = text.trim();
        if trimmed.is_empty() || is_comment_line(trimmed) {
            next += 1;
            continue;
        }
        let indent = indentation(text);
        let closes = trimmed.starts_with(['}', ')', ']']);
        if indent > base || (indent == base && closes) {
            last = next;
            next += 1;
            continue;
        }
        break;
    }
    last
}

/// The item a line declares.
fn item_on_line(lines: &[&str], line: usize, stop_lines: &BTreeSet<usize>) -> Option<Item> {
    let text = lines[line - 1];
    let trimmed = text.trim_start();
    let (kind, name) = if trimmed.starts_with('#') {
        ("type", meta_type_symbol_name(trimmed)?)
    } else if trimmed.starts_with('|') {
        let kind = if indentation(text) > 0 {
            "member"
        } else {
            "rule"
        };
        (kind, meta_rule_symbol_name(trimmed)?)
    } else if trimmed.starts_with('>') {
        ("function", meta_fn_symbol_name(trimmed)?)
    } else if trimmed.starts_with('=') {
        ("binding", meta_binding_symbol_name(trimmed)?)
    } else {
        return None;
    };
    let end_line = if kind == "type" && !trimmed.trim_end().ends_with('{') {
        // A data type's metadata describes the type, not a code span; a
        // RuleScope (`# Name(...) {`) spans its body.
        line
    } else {
        item_end_line(lines, line, stop_lines)
    };
    Some(Item {
        kind,
        name,
        line,
        end_line,
    })
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

/// The document and role of a `@ quote` argument.
pub(crate) fn quote_argument(argument: &str) -> Result<(String, Option<String>), String> {
    let argument = argument.trim();
    let malformed = || {
        format!(
            "`@ quote({argument})`: the argument is a document binding, as in `@ quote(aktiesparekontoloven)`, or a role holding one, as in `@ quote(Source(value = aktiesparekontoloven))`"
        )
    };
    if is_identifier(argument) {
        if argument.starts_with(char::is_uppercase) {
            return Err(malformed());
        }
        return Ok((argument.to_string(), None));
    }
    let (role, rest) = argument.split_once('(').ok_or_else(malformed)?;
    let role = role.trim();
    let inner = rest.strip_suffix(')').ok_or_else(malformed)?.trim();
    let document = inner
        .strip_prefix("value")
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix('='))
        .map(str::trim)
        .ok_or_else(malformed)?;
    if !is_identifier(role)
        || !role.starts_with(char::is_uppercase)
        || !is_identifier(document)
        || document.starts_with(char::is_uppercase)
    {
        return Err(malformed());
    }
    Ok((document.to_string(), Some(role.to_string())))
}

const CITE_PLACEMENT: &str =
    "stands directly above a rule clause (`| ...`), an exception, a RuleScope member or a function (`> ...`)";
const META_WHEN_PLACEMENT: &str =
    "`@ meta(...) when <condition>` stands on a rule clause or RuleScope member: the assumption is reported when the clause fires and the condition holds";
const META_PLACEMENT: &str =
    "stands directly above a section, a data type, a binding, a rule clause, a RuleScope member or a function";

/// Read and place the annotations of `source`.
pub(crate) fn read(source: &str) -> Annotations {
    let mut lexer = Lexer::new(source);
    lexer.tokenize();
    read_lexed(source, std::mem::take(&mut lexer.annotations))
}

fn read_lexed(source: &str, lexed: Vec<LexedAnnotation>) -> Annotations {
    let lines: Vec<&str> = source.lines().collect();
    let mut result = Annotations::default();
    let annotation_lines: BTreeSet<usize> = lexed
        .iter()
        .flat_map(|annotation| annotation.line..=annotation.end_line)
        .collect();
    let section_end_lines: BTreeSet<usize> = lexed
        .iter()
        .filter(|annotation| annotation.kind == "section_end")
        .map(|annotation| annotation.line)
        .collect();
    let stop_lines: BTreeSet<usize> = annotation_lines
        .iter()
        .chain(section_end_lines.iter())
        .copied()
        .collect();

    let mut open: Option<usize> = None;
    let mut quote_blocks = BTreeSet::new();
    let mut index = 0;
    while index < lexed.len() {
        let first = &lexed[index];
        if first.kind == "section_end" {
            if let Some(open) = open.take() {
                result.sections[open].end_line = first.line;
            }
            index += 1;
            continue;
        }
        if first.kind == "meta_for" {
            result.metas_for.push(MetaFor {
                line: first.line,
                value: first.argument.clone(),
                target: first.target.clone().unwrap_or_default(),
            });
            index += 1;
            continue;
        }
        // A group: annotations on consecutive lines, with only comments
        // between; a section ends its group.
        let mut group = vec![index];
        let mut end_line = first.end_line;
        while lexed[*group.last().unwrap()].kind != "section" {
            let next = *group.last().unwrap() + 1;
            let Some(candidate) = lexed.get(next) else {
                break;
            };
            if candidate.kind == "section_end"
                || candidate.kind == "meta_for"
                || ((end_line + 1)..candidate.line).any(|line| {
                    let trimmed = lines[line - 1].trim();
                    !is_comment_line(trimmed)
                })
            {
                break;
            }
            end_line = candidate.end_line;
            group.push(next);
        }
        index = *group.last().unwrap() + 1;
        let annotations: Vec<&LexedAnnotation> = group.iter().map(|i| &lexed[*i]).collect();
        let current_section = open.map(|open| result.sections[open].name.clone());

        let last = annotations.last().unwrap();
        if last.kind == "section" {
            let mut metas = Vec::new();
            for annotation in &annotations[..annotations.len() - 1] {
                match annotation.kind {
                    "meta" => {
                        if annotation.condition.is_some() {
                            result.errors.push((annotation.line, META_WHEN_PLACEMENT.to_string()));
                        }
                        metas.push(MetaAnnotation {
                            line: annotation.line,
                            value: annotation.argument.clone(),
                            condition: None,
                        })
                    }
                    "quote" => result.errors.push((
                        annotation.line,
                        "`@ quote(...)` stands directly above the `----` block it quotes, inside a section".to_string(),
                    )),
                    _ => result.errors.push((
                        annotation.line,
                        format!("`@ cite(...)` {CITE_PLACEMENT}, not a section"),
                    )),
                }
            }
            if result.section(&last.argument).is_some() {
                result.errors.push((
                    last.line,
                    format!(
                        "section `{}` is declared twice; a section's name names one passage",
                        last.argument
                    ),
                ));
            }
            result.sections.push(Section {
                name: last.argument.clone(),
                line: last.line,
                end_line: lines.len() + 1,
                metas,
                quotes: Vec::new(),
            });
            open = Some(result.sections.len() - 1);
            continue;
        }

        let (target, blank) = next_item_line(&lines, end_line, &annotation_lines);
        let describe = |annotation: &LexedAnnotation| match annotation.kind {
            "meta" => format!("`@ meta({})`", annotation.argument),
            "cite" => format!("`@ cite({})`", annotation.argument),
            _ => format!("`@ quote({})`", annotation.argument),
        };
        let placement = |annotation: &LexedAnnotation| match annotation.kind {
            "meta" => META_PLACEMENT.to_string(),
            "cite" => CITE_PLACEMENT.to_string(),
            _ => "stands directly above the `----` block it quotes".to_string(),
        };
        let Some(target) = target.filter(|_| !blank) else {
            for annotation in &annotations {
                let reason = if blank && target.is_some() {
                    "; remove the blank line below it"
                } else {
                    ", but nothing follows it"
                };
                result.errors.push((
                    annotation.line,
                    format!("{} {}{reason}", describe(annotation), placement(annotation)),
                ));
            }
            continue;
        };
        let target_text = lines[target - 1].trim_start();
        if annotation_lines.contains(&target) || section_end_lines.contains(&target) {
            for annotation in &annotations {
                result.errors.push((
                    annotation.line,
                    format!("{} {}", describe(annotation), placement(annotation)),
                ));
            }
            continue;
        }

        if target_text.starts_with("----") {
            let quotes: Vec<&&LexedAnnotation> = annotations
                .iter()
                .filter(|annotation| annotation.kind == "quote")
                .collect();
            for annotation in annotations.iter().filter(|a| a.kind != "quote") {
                let message = if annotation.kind == "meta" {
                    format!(
                        "{} stands above a `----` block; a quoted document's role is written on the quote, `@ quote(Source(value = <document>))`",
                        describe(annotation)
                    )
                } else {
                    format!("{} {CITE_PLACEMENT}", describe(annotation))
                };
                result.errors.push((annotation.line, message));
            }
            let Some(quote) = quotes.first() else {
                continue;
            };
            for extra in quotes.iter().skip(1) {
                result.errors.push((
                    extra.line,
                    "a `----` block quotes one document; write one `@ quote(...)` above it"
                        .to_string(),
                ));
            }
            let Some(open_index) = open else {
                result.errors.push((
                    quote.line,
                    format!(
                        "{} stands only inside a section; outside one, a `----` block is a block comment",
                        describe(quote)
                    ),
                ));
                continue;
            };
            if block_comment_delimiter_count(target_text.trim()) != 1 {
                result.errors.push((
                    target,
                    "a quote's opening `----` stands on a line of its own".to_string(),
                ));
                continue;
            }
            let Some(close_line) = (target + 1..=lines.len())
                .find(|line| lines[line - 1].trim_start().starts_with("----"))
            else {
                continue;
            };
            quote_blocks.insert(target);
            let (document, role) = match quote_argument(&quote.argument) {
                Ok(parsed) => parsed,
                Err(message) => {
                    result.errors.push((quote.line, message));
                    continue;
                }
            };
            result.sections[open_index].quotes.push(QuoteAnnotation {
                line: quote.line,
                argument: quote.argument.clone(),
                document,
                role,
                open_line: target,
                close_line,
                text: lines[target..close_line - 1].join("\n"),
            });
            continue;
        }

        let item = item_on_line(&lines, target, &stop_lines);
        let mut cited = false;
        let mut metas = Vec::new();
        for annotation in &annotations {
            match annotation.kind {
                "quote" => result.errors.push((
                    annotation.line,
                    format!("{} {}", describe(annotation), placement(annotation)),
                )),
                "cite" => {
                    let clause = target_text.starts_with('|') || target_text.starts_with('>');
                    if !clause {
                        result.errors.push((
                            annotation.line,
                            format!("{} {CITE_PLACEMENT}", describe(annotation)),
                        ));
                    } else if cited {
                        result.errors.push((
                            annotation.line,
                            "a clause takes one `@ cite(...)`; cite several paragraphs in it, separated by `;`".to_string(),
                        ));
                    } else {
                        cited = true;
                        result.citations.push(CiteAnnotation {
                            line: annotation.line,
                            text: annotation.argument.clone(),
                            clause_line: target,
                            section: current_section.clone(),
                        });
                    }
                }
                _ => {
                    if let Some(item) = &item {
                        let condition = annotation.condition.clone();
                        if condition.is_some() {
                            if !matches!(item.kind, "rule" | "member") {
                                result.errors.push((annotation.line, META_WHEN_PLACEMENT.to_string()));
                            } else if !annotation.argument.trim_start().starts_with("Assumption(") {
                                result.errors.push((
                                    annotation.line,
                                    "`@ meta(...) when` qualifies an assumption only: write `@ meta(Assumption(value = ...)) when <condition>`".to_string(),
                                ));
                            }
                        }
                        metas.push(MetaAnnotation {
                            line: annotation.line,
                            value: annotation.argument.clone(),
                            condition,
                        });
                    } else {
                        result.errors.push((
                            annotation.line,
                            format!("{} {META_PLACEMENT}", describe(annotation)),
                        ));
                    }
                }
            }
        }
        if let (Some(item), false) = (item, metas.is_empty()) {
            result.item_metas.push(ItemMeta {
                metas,
                item,
                section: current_section,
            });
        }
    }

    // Inside a section every `----` block is a quote.
    for section in &result.sections {
        let mut in_block = false;
        for line in section.line + 1..section.end_line.min(lines.len() + 1) {
            let trimmed = lines[line - 1].trim_start();
            if !trimmed.starts_with("----") {
                continue;
            }
            if in_block {
                in_block = false;
                continue;
            }
            if block_comment_delimiter_count(trimmed) % 2 == 1 {
                in_block = true;
            }
            if !quote_blocks.contains(&line) {
                result.errors.push((
                    line,
                    format!(
                        "a `----` block inside section `{}` is a quote; write `@ quote(<document>)` on the line above it",
                        section.name
                    ),
                ));
            }
        }
    }
    result.errors.sort();
    result
}

/// The documents a file can quote: its bindings and its imports', the
/// types that implement `Document`, and which type each constructor makes.
#[derive(Default)]
pub(crate) struct Documents {
    bindings: BTreeMap<String, Expr>,
    constructor_types: BTreeMap<String, String>,
    document_types: BTreeSet<String>,
}

impl Documents {
    pub(crate) fn collect(source: &str, stmts: &[Stmt], source_dir: Option<&str>) -> Self {
        let mut documents = Documents::default();
        let mut bindings = BTreeMap::new();
        collect_local_meta_bindings(source, stmts, None, &mut bindings);
        collect_imported_meta_bindings(stmts, source_dir, &mut bindings, &mut BTreeSet::new());
        documents.bindings = bindings
            .into_iter()
            .map(|(name, definition)| (name, definition.expr))
            .collect();
        documents.collect_types(stmts);
        documents.collect_imported_types(stmts, source_dir, &mut BTreeSet::new());
        documents
    }

    /// Documents read from the file at `path`.
    pub(crate) fn for_file(source: &str, stmts: &[Stmt], path: &str) -> Self {
        let directory = Path::new(path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned());
        Self::collect(source, stmts, directory.as_deref())
    }

    fn collect_types(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::TypeDecl(TypeDecl::ADT { name, variants, .. }) => {
                    for variant in variants {
                        self.constructor_types
                            .entry(variant.name.clone())
                            .or_insert_with(|| name.clone());
                    }
                }
                Stmt::TypeDecl(TypeDecl::ImplBlock {
                    trait_name,
                    for_type,
                    ..
                }) if trait_name == "Document" => {
                    self.document_types.insert(for_type.clone());
                }
                _ => {}
            }
        }
    }

    fn collect_imported_types(
        &mut self,
        stmts: &[Stmt],
        source_dir: Option<&str>,
        visited: &mut BTreeSet<String>,
    ) {
        let Some(source_dir) = source_dir else {
            return;
        };
        for stmt in stmts {
            let Stmt::Import(import_path) = stmt else {
                continue;
            };
            let Some(file_path) =
                Interpreter::resolve_import_path_for_source(import_path, source_dir)
            else {
                continue;
            };
            let canonical = std::fs::canonicalize(&file_path)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.clone());
            if !visited.insert(canonical) {
                continue;
            }
            let Ok(module) = parse_source_module_file_cached(Path::new(&file_path)) else {
                continue;
            };
            self.collect_types(module.statements());
            let nested = Path::new(&file_path)
                .parent()
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            self.collect_imported_types(module.statements(), Some(&nested), visited);
        }
    }

    fn constructor_call<'a>(&'a self, name: &str) -> Option<(&'a str, &'a [Expr])> {
        let expr = self.bindings.get(name)?;
        let ExprKind::App(function, arguments) = &expr.kind else {
            return None;
        };
        let ExprKind::Var(constructor) = &function.kind else {
            return None;
        };
        Some((constructor.as_str(), arguments.as_slice()))
    }

    /// A string field of the document bound to `name`.
    pub(crate) fn field(&self, name: &str, field: &str) -> Option<String> {
        let (_, arguments) = self.constructor_call(name)?;
        arguments.iter().find_map(|argument| {
            let (key, value) = named_arg_parts(argument)?;
            match (&value.kind, key == field) {
                (ExprKind::Lit(Literal::Str(text)), true) => Some(text.clone()),
                _ => None,
            }
        })
    }

    /// The file of the document bound to `name`, or why it is not a
    /// document.
    pub(crate) fn file(&self, name: &str) -> Result<String, String> {
        if !self.bindings.contains_key(name) {
            return Err(format!(
                "`{name}` is not a binding in scope; `@ quote` names a document binding, a value whose type implements `Document`"
            ));
        }
        let Some((constructor, _)) = self.constructor_call(name) else {
            return Err(format!(
                "`{name}` is not a document: a document binding is a record value, as in `= {name} = SourceInfo(file = \"sources/...\", ...)`"
            ));
        };
        let type_name = self
            .constructor_types
            .get(constructor)
            .cloned()
            .unwrap_or_else(|| constructor.to_string());
        if !self.document_types.contains(&type_name) {
            return Err(format!(
                "`{name}` has type `{type_name}`, which does not implement `Document`; add `# impl Document for {type_name} {{}}`"
            ));
        }
        self.field(name, "file").ok_or_else(|| {
            format!("document `{name}` has no `file = \"...\"`; a `Document` names its saved file")
        })
    }
}

/// The project root of a model file: the nearest directory at or above it
/// that holds `runa.toml`, else the file's own directory.
pub(crate) fn project_root(file: &Path) -> PathBuf {
    let file = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let directory = file.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut current = Some(directory.as_path());
    while let Some(candidate) = current {
        if candidate.join("runa.toml").is_file() {
            return candidate.to_path_buf();
        }
        current = candidate.parent();
    }
    directory
}

/// Text as `verify_quotes.py` compares it: no-break spaces as spaces, soft
/// hyphens and zero-width joiners removed, whitespace collapsed.
pub(crate) fn normalise_quoted_text(text: &str) -> String {
    text.replace('\u{a0}', " ")
        .replace(['\u{ad}', '\u{200d}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every paragraph (lines separated by a blank line) of each quote must
/// occur in the saved file of its document. Errors by line.
pub(crate) fn quote_file_errors(
    annotations: &Annotations,
    documents: &Documents,
    root: &Path,
) -> Vec<(usize, String)> {
    let mut errors = Vec::new();
    let mut files: BTreeMap<String, Option<String>> = BTreeMap::new();
    for section in &annotations.sections {
        for quote in &section.quotes {
            let file = match documents.file(&quote.document) {
                Ok(file) => file,
                Err(message) => {
                    errors.push((quote.line, message));
                    continue;
                }
            };
            let saved = files
                .entry(file.clone())
                .or_insert_with(|| {
                    crate::read_source_file(&root.join(&file))
                        .ok()
                        .map(|text| normalise_quoted_text(&text))
                })
                .clone();
            let Some(saved) = saved else {
                errors.push((
                    quote.line,
                    format!(
                        "the file of `{}`, `{file}`, does not exist under the project root `{}`",
                        quote.document,
                        root.display()
                    ),
                ));
                continue;
            };
            let mut paragraph: Vec<(usize, &str)> = Vec::new();
            let quoted: Vec<&str> = quote.text.lines().collect();
            for (offset, text) in quoted.iter().chain(std::iter::once(&"")).enumerate() {
                if !text.trim().is_empty() {
                    paragraph.push((quote.text_start_line() + offset, text.trim()));
                    continue;
                }
                if let Some((line, _)) = paragraph.first().copied() {
                    let joined = paragraph
                        .iter()
                        .map(|(_, text)| *text)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let normalised = normalise_quoted_text(&joined);
                    if !saved.contains(&normalised) {
                        let shown: String = normalised.chars().take(90).collect();
                        errors.push((
                            line,
                            format!(
                                "this quoted paragraph is not in `{file}` (document `{}`) verbatim: {shown}",
                                quote.document
                            ),
                        ));
                    }
                }
                paragraph.clear();
            }
        }
    }
    errors
}

/// The text a section quotes, as explanations list it: each document's text
/// after a `[<file>]` line where the quoted document changes.
pub(crate) fn section_quote_text(section: &Section, documents: &Documents) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut previous: Option<&str> = None;
    for quote in &section.quotes {
        let text = trim_blank_lines(&quote.text);
        if previous != Some(quote.document.as_str()) {
            let header = documents
                .file(&quote.document)
                .unwrap_or_else(|_| quote.document.clone());
            parts.push(format!("[{header}]"));
        }
        parts.push(text);
        previous = Some(&quote.document);
    }
    parts.join("\n\n")
}

pub(crate) fn trim_blank_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(start, |end| end + 1);
    lines[start..end].join("\n")
}

/// The checker's diagnostics for the annotations of `source`: misplaced
/// annotations and citations the section's quotes do not hold.
pub(crate) fn annotation_diagnostics(
    source: &str,
    stmts: &[Stmt],
    source_dir: Option<&str>,
) -> Vec<Diagnostic> {
    if !source.contains('@') {
        return Vec::new();
    }
    let annotations = read(source);
    if annotations.sections.is_empty()
        && annotations.citations.is_empty()
        && annotations.errors.is_empty()
    {
        return Vec::new();
    }
    let documents = Documents::collect(source, stmts, source_dir);
    let mut errors = annotations.errors.clone();
    for section in &annotations.sections {
        for quote in &section.quotes {
            if let Err(message) = documents.file(&quote.document) {
                errors.push((quote.line, message));
            }
        }
    }
    errors.extend(crate::citations::file_citations_from(&annotations, &documents).errors);
    line_diagnostics(source, errors)
}

/// `runa check`'s comparison of every quote with its saved document, for
/// the model `filename` and every file it imports. Documents are found
/// under the project root: the directory of `runa.toml`, else of the model.
pub fn check_quoted_documents(filename: &str, source: &str) -> Vec<Diagnostic> {
    let root = project_root(Path::new(filename));
    let mut diagnostics = Vec::new();
    let mut visited = BTreeSet::new();
    let Ok(stmts) = Parser::new(Lexer::new(source).tokenize(), source).parse_program() else {
        return diagnostics;
    };
    let directory = Path::new(filename)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .filter(|parent| !parent.is_empty())
        .unwrap_or_else(|| ".".to_string());
    let root_errors = quote_errors_of(source, &stmts, &directory, &root);
    diagnostics.extend(line_diagnostics(source, root_errors));
    check_imported_quotes(&stmts, &directory, &root, &mut visited, &mut diagnostics);
    diagnostics.extend(meta_for_diagnostics(filename, source, &stmts, &directory));
    diagnostics
}

/// A declaration `@ meta(...) for <name>` may name.
#[derive(Debug, Clone)]
struct Declaration {
    kind: &'static str,
    line: usize,
}

/// One file of a program, as the metadata written apart from its
/// declarations needs it.
struct ProgramFile {
    /// The path as the program reads it.
    display: String,
    source: Arc<str>,
    annotations: Annotations,
    declarations: Vec<(String, Declaration)>,
    /// The files it imports, by key.
    imports: Vec<String>,
}

/// The types, rules, functions and sections a file declares.
fn file_declarations(source: &str, annotations: &Annotations) -> Vec<(String, Declaration)> {
    let mut declarations = Vec::new();
    let mut in_block = false;
    for (index, line) in source.lines().enumerate() {
        if line.starts_with("----") {
            if in_block || block_comment_delimiter_count(line.trim()) % 2 == 1 {
                in_block = !in_block;
            }
            continue;
        }
        if in_block || line.starts_with(char::is_whitespace) {
            continue;
        }
        let declared = if line.starts_with('#') {
            meta_type_symbol_name(line)
                .filter(|name| !matches!(name.as_str(), "impl" | "trait" | "unit" | "effect"))
                .map(|name| ("type", name))
        } else if line.starts_with('|') {
            meta_rule_symbol_name(line).map(|name| ("rule", name))
        } else if line.starts_with('>') {
            meta_fn_symbol_name(line).map(|name| ("function", name))
        } else {
            None
        };
        if let Some((kind, name)) = declared {
            declarations.push((
                name,
                Declaration {
                    kind,
                    line: index + 1,
                },
            ));
        }
    }
    for section in &annotations.sections {
        declarations.push((
            section.name.clone(),
            Declaration {
                kind: "section",
                line: section.line,
            },
        ));
    }
    declarations
}

/// The file `key` and every file it imports, transitively.
fn collect_program_files(
    key: String,
    display: String,
    source: Arc<str>,
    stmts: &[Stmt],
    directory: &str,
    files: &mut BTreeMap<String, ProgramFile>,
) {
    let annotations = read(&source);
    let declarations = file_declarations(&source, &annotations);
    let mut imports = Vec::new();
    let mut pending = Vec::new();
    for stmt in stmts {
        let Stmt::Import(import_path) = stmt else {
            continue;
        };
        let Some(file_path) = Interpreter::resolve_import_path_for_source(import_path, directory)
        else {
            continue;
        };
        let canonical = std::fs::canonicalize(&file_path)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| file_path.clone());
        imports.push(canonical.clone());
        pending.push((canonical, file_path));
    }
    files.insert(
        key,
        ProgramFile {
            display,
            source,
            annotations,
            declarations,
            imports,
        },
    );
    for (canonical, file_path) in pending {
        if files.contains_key(&canonical) {
            continue;
        }
        let Ok(module) = parse_source_module_file_cached(Path::new(&file_path)) else {
            continue;
        };
        let nested = Path::new(&file_path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        collect_program_files(
            canonical,
            file_path.clone(),
            Arc::from(module.source()),
            module.statements(),
            &nested,
            files,
        );
    }
}

/// The files in scope of `key`: itself and what it imports, transitively,
/// in import order.
fn files_in_scope(files: &BTreeMap<String, ProgramFile>, key: &str) -> Vec<String> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![key.to_string()];
    while let Some(next) = stack.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        if let Some(file) = files.get(&next) {
            stack.extend(file.imports.iter().rev().cloned());
        }
        order.push(next);
    }
    order
}

/// The declaration `@ meta(...) for <target>` in file `key` describes: its
/// file's key, kind and line.
fn resolve_meta_for(
    files: &BTreeMap<String, ProgramFile>,
    key: &str,
    target: &str,
) -> Result<(String, &'static str, usize), String> {
    let mut found: Vec<(String, &'static str, usize)> = Vec::new();
    for file_key in files_in_scope(files, key) {
        let Some(file) = files.get(&file_key) else {
            continue;
        };
        for (name, declaration) in &file.declarations {
            // The clauses of one rule are one declaration.
            let same = found.iter().any(|(other, kind, _)| {
                *kind == declaration.kind && (*kind == "rule" || *other == file_key)
            });
            if name == target && !same {
                found.push((file_key.clone(), declaration.kind, declaration.line));
            }
        }
    }
    match found.len() {
        0 => Err(format!(
            "`@ meta(...) for {target}`: no type, rule, function or section named `{target}` is declared in this file or a file it imports"
        )),
        1 => Ok(found.remove(0)),
        _ => Err(format!(
            "`@ meta(...) for {target}`: `{target}` names {}; write `@ meta(...)` on the line above the one it describes",
            found
                .iter()
                .map(|(file, kind, line)| format!(
                    "a {kind} at {}:{line}",
                    files.get(file).map_or(file.as_str(), |file| file.display.as_str())
                ))
                .collect::<Vec<_>>()
                .join(" and ")
        )),
    }
}

/// The kind and declaring file of every `@ meta(...) for <name>` in
/// `source`, for the metadata index; the file is empty when it is `source`
/// itself. Names that do not resolve are left out (`runa check` reports
/// them).
pub(crate) fn meta_for_targets(
    source: &str,
    stmts: &[Stmt],
    source_dir: Option<&str>,
) -> BTreeMap<String, (&'static str, String)> {
    if !source.contains(") for ") {
        return BTreeMap::new();
    }
    let annotations = read(source);
    if annotations.metas_for.is_empty() {
        return BTreeMap::new();
    }
    let mut files = BTreeMap::new();
    collect_program_files(
        String::new(),
        String::new(),
        Arc::from(source),
        stmts,
        source_dir.unwrap_or("."),
        &mut files,
    );
    annotations
        .metas_for
        .iter()
        .filter_map(|meta_for| {
            let (file, kind, _) = resolve_meta_for(&files, "", &meta_for.target).ok()?;
            let display = files.get(&file).map(|file| file.display.clone())?;
            Some((meta_for.target.clone(), (kind, display)))
        })
        .collect()
}

/// `@ meta(...) for <name>` across the program of `filename`: the name
/// resolves to one declaration in scope, and a declaration takes its
/// metadata in one place, on itself or in one `for`.
fn meta_for_diagnostics(
    filename: &str,
    source: &str,
    stmts: &[Stmt],
    directory: &str,
) -> Vec<Diagnostic> {
    let mut files = BTreeMap::new();
    let root = std::fs::canonicalize(filename)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| filename.to_string());
    collect_program_files(
        root.clone(),
        filename.to_string(),
        Arc::from(source),
        stmts,
        directory,
        &mut files,
    );
    if files
        .values()
        .all(|file| file.annotations.metas_for.is_empty())
    {
        return Vec::new();
    }
    let mut errors: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    // Every place a declaration's metadata is written: (file, line).
    let mut places: BTreeMap<(&'static str, String, String), Vec<(String, usize, bool)>> =
        BTreeMap::new();
    let declaration_key = |kind: &'static str, file: &str, name: &str| {
        // A rule's clauses may stand in several files.
        let file = if kind == "rule" {
            String::new()
        } else {
            file.to_string()
        };
        (kind, file, name.to_string())
    };
    for (key, file) in &files {
        let mut previous: Option<(&str, usize)> = None;
        for meta_for in &file.annotations.metas_for {
            match resolve_meta_for(&files, key, &meta_for.target) {
                Ok((declared, kind, _)) => {
                    // Consecutive lines for one name are one place.
                    let continues = previous.is_some_and(|(target, line)| {
                        target == meta_for.target
                            && file
                                .source
                                .lines()
                                .skip(line)
                                .take(meta_for.line - line - 1)
                                .all(|text| {
                                    let trimmed = text.trim();
                                    trimmed.starts_with("--") && !trimmed.starts_with("----")
                                })
                    });
                    previous = Some((&meta_for.target, meta_for.line));
                    if !continues {
                        places
                            .entry(declaration_key(kind, &declared, &meta_for.target))
                            .or_default()
                            .push((key.clone(), meta_for.line, true));
                    }
                }
                Err(message) => {
                    previous = None;
                    errors
                        .entry(key.clone())
                        .or_default()
                        .push((meta_for.line, message));
                }
            }
        }
    }
    for (key, file) in &files {
        for item_meta in &file.annotations.item_metas {
            let item = &item_meta.item;
            if !matches!(item.kind, "type" | "rule" | "function") {
                continue;
            }
            let declaration = declaration_key(item.kind, key, &item.name);
            if let Some(found) = places.get_mut(&declaration) {
                let line = item_meta.metas.first().map_or(item.line, |meta| meta.line);
                found.push((key.clone(), line, false));
            }
        }
        for section in &file.annotations.sections {
            let declaration = declaration_key("section", key, &section.name);
            if let (Some(found), Some(meta)) = (places.get_mut(&declaration), section.metas.first())
            {
                found.push((key.clone(), meta.line, false));
            }
        }
    }
    for ((kind, _, name), found) in &places {
        if found.len() < 2 {
            continue;
        }
        let shown = |(file, line, apart): &(String, usize, bool)| {
            let display = files
                .get(file)
                .map_or(file.as_str(), |file| file.display.as_str());
            if *apart {
                format!("`@ meta(...) for {name}` at {display}:{line}")
            } else {
                format!("`@ meta(...)` on the {kind} at {display}:{line}")
            }
        };
        let all = found.iter().map(shown).collect::<Vec<_>>().join(" and ");
        for (file, line, apart) in found {
            if *apart {
                errors.entry(file.clone()).or_default().push((
                    *line,
                    format!(
                        "the metadata of {kind} `{name}` is written in more than one place: {all}; write it in one, so a reader finds all of it there"
                    ),
                ));
            }
        }
    }
    let mut diagnostics = Vec::new();
    for (key, file_errors) in errors {
        let Some(file) = files.get(&key) else {
            continue;
        };
        for mut diagnostic in line_diagnostics(&file.source, file_errors) {
            if key != root {
                diagnostic.origin = Some(DiagnosticOrigin {
                    path: PathBuf::from(&key),
                    source: file.source.clone(),
                    span: diagnostic.span,
                });
                diagnostic.span = None;
            }
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn quote_errors_of(
    source: &str,
    stmts: &[Stmt],
    directory: &str,
    root: &Path,
) -> Vec<(usize, String)> {
    if !source.contains("@ quote") {
        return Vec::new();
    }
    let annotations = read(source);
    let documents = Documents::collect(source, stmts, Some(directory));
    quote_file_errors(&annotations, &documents, root)
}

fn check_imported_quotes(
    stmts: &[Stmt],
    directory: &str,
    root: &Path,
    visited: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        let Stmt::Import(import_path) = stmt else {
            continue;
        };
        let Some(file_path) = Interpreter::resolve_import_path_for_source(import_path, directory)
        else {
            continue;
        };
        let canonical =
            std::fs::canonicalize(&file_path).unwrap_or_else(|_| PathBuf::from(&file_path));
        if !visited.insert(canonical.to_string_lossy().into_owned()) {
            continue;
        }
        let Ok(module) = parse_source_module_file_cached(Path::new(&file_path)) else {
            continue;
        };
        let nested = Path::new(&file_path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        let source: Arc<str> = Arc::from(module.source());
        let errors = quote_errors_of(&source, module.statements(), &nested, root);
        for mut diagnostic in line_diagnostics(&source, errors) {
            diagnostic.origin = Some(DiagnosticOrigin {
                path: canonical.clone(),
                source: source.clone(),
                span: diagnostic.span,
            });
            diagnostic.span = None;
            diagnostics.push(diagnostic);
        }
        check_imported_quotes(module.statements(), &nested, root, visited, diagnostics);
    }
}

/// Diagnostics at whole lines.
pub(crate) fn line_diagnostics(source: &str, mut errors: Vec<(usize, String)>) -> Vec<Diagnostic> {
    errors.sort();
    errors.dedup();
    let mut starts = vec![0];
    for (index, character) in source.chars().enumerate() {
        if character == '\n' {
            starts.push(index + 1);
        }
    }
    errors
        .into_iter()
        .map(|(line, message)| {
            let start = starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
            let end = starts
                .get(line)
                .map_or(start, |next| next.saturating_sub(1));
            Diagnostic::error_at(Span::new(start, end.max(start + 1)), message)
        })
        .collect()
}

/// Parse the condition of `@ meta(...) when <condition>` written on `line`
/// of `source`; its spans are its place in the source, so a type error
/// points at it.
pub(crate) fn parse_meta_condition(source: &str, line: usize, text: &str) -> Result<Expr, String> {
    let mut offset = 0;
    for (index, source_line) in source.split('\n').enumerate() {
        if index + 1 == line {
            offset += source_line
                .rfind(text)
                .map_or(0, |byte| source_line[..byte].chars().count());
            break;
        }
        offset += source_line.chars().count() + 1;
    }
    let mut placed: String = source
        .chars()
        .take(offset)
        .map(|c| if c == '\n' { '\n' } else { ' ' })
        .collect();
    placed.push_str(text);
    Parser::new(Lexer::new(&placed).tokenize(), &placed)
        .parse_expr()
        .or_else(|_| Parser::new(Lexer::new(text).tokenize(), text).parse_expr())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "# SourceInfo(file: String)
# impl Document for SourceInfo {}
# Role(a) = Source(value: a)
= law = SourceInfo(file = \"law.txt\")

@ meta(note)
@ section ceiling {

@ quote(Source(value = law))
----
§ 9. Text.
----

-- comment
@ cite(§ 9)
| f(x: Int) -> x

@ meta(guide)
> g(x: Int) -> Int {
    x
}

}
";

    #[test]
    fn a_section_holds_its_quotes_citations_and_metadata() {
        let annotations = read(MODEL);
        assert_eq!(annotations.errors, Vec::<(usize, String)>::new());
        let section = &annotations.sections[0];
        assert_eq!(section.name, "ceiling");
        assert_eq!((section.line, section.end_line), (7, 23));
        assert_eq!(section.metas[0].value, "note");
        assert_eq!(section.quotes[0].document, "law");
        assert_eq!(section.quotes[0].role.as_deref(), Some("Source"));
        assert_eq!(section.quotes[0].text, "§ 9. Text.");
        assert_eq!(annotations.citations[0].clause_line, 16);
        assert_eq!(annotations.citations[0].section.as_deref(), Some("ceiling"));
        let meta = &annotations.item_metas[0];
        assert_eq!(
            (
                meta.item.kind,
                meta.item.name.as_str(),
                meta.item.line,
                meta.item.end_line
            ),
            ("function", "g", 19, 21)
        );
    }

    #[test]
    fn the_program_is_the_same_without_its_annotations() {
        let plain = "# SourceInfo(file: String)\n# impl Document for SourceInfo {}\n# Role(a) = Source(value: a)\n= law = SourceInfo(file = \"law.txt\")\n| f(x: Int) -> x\n> g(x: Int) -> Int {\n    x\n}\n";
        let significant = |source: &str| {
            Lexer::new(source)
                .tokenize()
                .into_iter()
                .filter(|token| token.kind != TokenKind::Semi)
                .map(|token| token.text)
                .collect::<Vec<_>>()
        };
        assert_eq!(significant(MODEL), significant(plain));
    }

    #[test]
    fn misplaced_annotations_are_errors() {
        let blank = "@ cite(§ 9)\n\n| f(x: Int) -> x\n";
        assert!(read(blank).errors[0].1.contains("remove the blank line"));
        let nothing = "| f(x: Int) -> x\n@ meta(note)\n";
        assert!(read(nothing).errors[0].1.contains("nothing follows it"));
        let binding = "@ cite(§ 9)\n= x = 1\n";
        assert!(read(binding).errors[0].1.contains("rule clause"));
        let outside = "@ quote(law)\n----\ntext\n----\n";
        assert!(read(outside).errors[0].1.contains("only inside a section"));
        let unquoted = "@ section s {\n----\ntext\n----\n}\n";
        assert!(read(unquoted).errors[0].1.contains("is a quote"));
    }

    #[test]
    fn metadata_written_for_a_declaration_stands_alone() {
        let source = "@ meta(guide) for Input\n@ meta(note)\n| f(x: Int) -> x\n";
        let annotations = read(source);
        assert_eq!(annotations.errors, Vec::<(usize, String)>::new());
        assert_eq!(
            annotations.metas_for,
            vec![MetaFor {
                line: 1,
                value: "guide".to_string(),
                target: "Input".to_string(),
            }]
        );
        assert_eq!(annotations.item_metas[0].metas.len(), 1);
        assert_eq!(annotations.item_metas[0].item.name, "f");
    }

    #[test]
    fn retired_directives_and_broken_sections_are_lexical_errors() {
        for (source, expected) in [
            ("--@label:x::meta:y--\n", "runa fmt --migrate"),
            ("--@cite:§ 9--\n| f(x: Int) -> x\n", "`@ cite(...)`"),
            ("@ section a {\n@ section b {\n}\n}\n", "do not nest"),
            ("@ section a {\n| f(x: Int) -> x\n", "no closing `}`"),
            ("> g() -> Int {\n@ section a {\n}\n1\n}\n", "top level"),
            (
                "> g() -> Int {\n    @ meta(m) for X\n    1\n}\n",
                "stands alone at top level",
            ),
            ("@ meta(m) for\n", "names the declaration"),
        ] {
            let tokens = Lexer::new(source).tokenize();
            assert!(
                tokens
                    .iter()
                    .any(|token| token.kind == TokenKind::Invalid && token.text.contains(expected)),
                "{source}: {tokens:?}"
            );
        }
    }

    #[test]
    fn quote_arguments() {
        assert_eq!(quote_argument("law"), Ok(("law".to_string(), None)));
        assert_eq!(
            quote_argument("Guidance(value = guide)"),
            Ok(("guide".to_string(), Some("Guidance".to_string())))
        );
        assert!(quote_argument("\"sources/law.txt\"").is_err());
    }

    #[test]
    fn quoted_paragraphs_are_compared_with_the_saved_file() {
        let directory =
            std::env::temp_dir().join(format!("futuruna-quote-check-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("law.txt"),
            "§ 9. Text\u{ad} that\n  is   saved.\n",
        )
        .unwrap();
        let source = MODEL.replace("§ 9. Text.", "§ 9. Text that is saved.\n\nAltered text.");
        let stmts = Parser::new(Lexer::new(&source).tokenize(), &source)
            .parse_program()
            .unwrap();
        let documents = Documents::collect(&source, &stmts, None);
        let errors = quote_file_errors(&read(&source), &documents, &directory);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].0, 13);
        assert!(errors[0].1.contains("Altered text."));
        std::fs::remove_dir_all(&directory).ok();
    }
}
