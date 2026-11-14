//! `runa fmt --migrate`: rewrite the retired comment directives
//! (`--@label`, `--@begin`/`--@end`, `--@cite`, `--@source`, `[sources/...]`
//! lines in quotes) as annotations (`@ section`, `@ quote`, `@ cite`,
//! `@ meta`).
//!
//! The rules are those of docs/rfcs/one-annotation-system.md, "Migration":
//!
//! 1. An anchor `--@label:X::meta:B--` with a `----` quote and a
//!    `--@begin:X--` ... `--@end:X--` span becomes `@ meta(B)` and
//!    `@ section X { ... }` around the span, the quote moved to the section's
//!    start, one `@ quote(<binding>)` per `[sources/<file>]` part, where
//!    `<binding>` is the program's only binding whose `file` is `<file>`.
//! 2. When `B` is used by this section only and holds role values
//!    (`Source(value = d)`) whose `d` is a quoted document, those values move
//!    onto the quotes and leave `B`; a `B` left with one value is written
//!    inline as `@ meta(<value>)`, an empty `B` is removed.
//! 3. An anchor before the declaration `X` (with or without a span around
//!    that declaration only) becomes `@ meta(B)` on `X`.
//! 4. `--@cite:...--` becomes `@ cite(...)`; `in sources/<file>` becomes
//!    `in <binding>`; a citation naming a paragraph and a part of it keeps
//!    the paragraph.
//! 5. `--@source::X::meta:B--` is read as `--@label`.
//!
//! Quoted text with no `[sources/...]` document is not a quote of a saved
//! file: it stays a `----` block comment, above the section.

use crate::annotations::quote_argument;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The bindings and types of the program a migrated file belongs to.
#[derive(Debug, Default)]
pub struct MigrationProgram {
    /// Bindings with a `file = "..."` field, by file: `(binding, constructor)`.
    documents: BTreeMap<String, Vec<(String, String)>>,
    /// Every identifier's number of occurrences in the program's sources.
    words: BTreeMap<String, usize>,
    /// Types with `# impl Document for <type> {}`.
    document_types: BTreeSet<String>,
    /// The file and line (0-based) declaring each record type.
    types: BTreeMap<String, (PathBuf, usize)>,
}

fn identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c == '_' || c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
}

/// The text of a binding or call starting at `lines[start]`, through its
/// balanced parentheses, and the index of its last line.
fn balanced_text(lines: &[&str], start: usize) -> (String, usize) {
    let mut depth = 0i64;
    let mut text = String::new();
    let mut opened = false;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let code = line.split(" --").next().unwrap_or(line);
        let mut in_string = false;
        for character in code.chars() {
            match character {
                '"' => in_string = !in_string,
                '(' | '[' if !in_string => {
                    depth += 1;
                    opened = true;
                }
                ')' | ']' if !in_string => depth -= 1,
                _ => {}
            }
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(line);
        if !opened || depth <= 0 {
            return (text, index);
        }
    }
    (text, lines.len().saturating_sub(1))
}

/// `= name = Ctor(`: the binding's name and constructor.
fn binding_head(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('=')?.trim_start();
    let (name, rest) = rest.split_once('=')?;
    let name = name.trim();
    let rest = rest.trim_start();
    let constructor: String = rest
        .chars()
        .take_while(|c| *c == '_' || c.is_alphanumeric())
        .collect();
    if name.is_empty() || name.contains(char::is_whitespace) || constructor.is_empty() {
        return None;
    }
    Some((name.to_string(), constructor))
}

/// The value of `field = "..."` in a binding's text.
fn string_field(text: &str, field: &str) -> Option<String> {
    let mut rest = text;
    while let Some(position) = rest.find(field) {
        let before = rest[..position].chars().next_back();
        let after = rest[position + field.len()..].trim_start();
        rest = &rest[position + field.len()..];
        if before.is_some_and(|c| c == '_' || c.is_alphanumeric()) {
            continue;
        }
        let Some(value) = after.strip_prefix('=') else {
            continue;
        };
        let value = value.trim_start().strip_prefix('"')?;
        return value.split_once('"').map(|(value, _)| value.to_string());
    }
    None
}

impl MigrationProgram {
    /// Read every `.runa` file under `root`.
    pub fn scan(root: &Path) -> Self {
        let mut files = Vec::new();
        collect_runa_files(root, &mut files);
        files.sort();
        let mut program = MigrationProgram::default();
        for file in files {
            if let Ok(source) = std::fs::read_to_string(&file) {
                program.add_source(&file, &source);
            }
        }
        program
    }

    pub fn add_source(&mut self, path: &Path, source: &str) {
        let lines: Vec<&str> = source.lines().collect();
        for word in identifiers(source) {
            *self.words.entry(word.to_string()).or_default() += 1;
        }
        for (index, line) in lines.iter().enumerate() {
            if let Some((name, constructor)) = binding_head(line) {
                let (text, _) = balanced_text(&lines, index);
                if let Some(file) = string_field(&text, "file") {
                    self.documents
                        .entry(file)
                        .or_default()
                        .push((name, constructor));
                }
            }
            if let Some(rest) = line.strip_prefix("# impl Document for ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| *c == '_' || c.is_alphanumeric())
                    .collect();
                self.document_types.insert(name);
            } else if let Some(rest) = line.strip_prefix("# ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| *c == '_' || c.is_alphanumeric())
                    .collect();
                if name.starts_with(char::is_uppercase)
                    && rest[name.len()..].starts_with('(')
                    && name != "impl"
                {
                    self.types
                        .entry(name)
                        .or_insert_with(|| (path.to_path_buf(), index));
                }
            }
        }
    }

    /// The binding whose `file` is `file`.
    fn document(&self, file: &str) -> Result<&(String, String), String> {
        let candidates: Vec<&(String, String)> = self
            .documents
            .iter()
            .filter(|(candidate, _)| {
                candidate.as_str() == file
                    || file.ends_with(&format!("/{candidate}"))
                    || candidate.ends_with(&format!("/{file}"))
            })
            .flat_map(|(_, bindings)| bindings)
            .collect();
        match candidates.as_slice() {
            [one] => Ok(one),
            [] => Err(format!(
                "`[{file}]` names a document no binding declares; declare it, as in `= law = SourceInfo(file = \"{file}\", ...)` with `# impl Document for SourceInfo {{}}`, and migrate again"
            )),
            _ => Err(format!(
                "`[{file}]` is the file of more than one binding ({}); keep one",
                candidates
                    .iter()
                    .map(|(name, _)| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// The types that quoted documents have but that do not implement
    /// `Document`, with the file and line declaring each.
    pub fn missing_document_impls(&self, constructors: &BTreeSet<String>) -> Vec<(String, PathBuf)> {
        constructors
            .iter()
            .filter(|name| !self.document_types.contains(*name))
            .filter_map(|name| {
                self.types
                    .get(name)
                    .map(|(path, _)| (name.clone(), path.clone()))
            })
            .collect()
    }
}

/// `source` with `# impl Document for <type> {}` after the declaration of
/// the record type `type`; `None` when the source does not declare it.
pub fn add_document_impl(source: &str, type_name: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    if lines
        .iter()
        .any(|text| text.starts_with(&format!("# impl Document for {type_name} ")))
    {
        return Some(source.to_string());
    }
    let line = lines
        .iter()
        .position(|text| text.starts_with(&format!("# {type_name}(")))?;
    let (_, end) = balanced_text(&lines, line);
    let mut output: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
    output.insert(end + 1, format!("# impl Document for {type_name} {{}}"));
    let mut text = output.join("\n");
    if source.ends_with('\n') {
        text.push('\n');
    }
    Some(text)
}

fn collect_runa_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !matches!(name.as_str(), "target" | ".git" | "node_modules") && !name.starts_with('.') {
                collect_runa_files(&path, files);
            }
        } else if name.ends_with(".runa") {
            files.push(path);
        }
    }
}

/// A rewritten file and what the migration changed that a reader should
/// know.
#[derive(Debug)]
pub struct MigratedFile {
    pub source: String,
    pub notes: Vec<String>,
    /// The constructors of the documents its quotes name.
    pub document_constructors: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct Directive {
    kind: String,
    label: String,
    attrs: Vec<(String, String)>,
}

fn parse_directive(line: &str) -> Option<Directive> {
    let body = line.trim().strip_prefix("--@")?.strip_suffix("--")?;
    let parts: Vec<&str> = body.split("::").map(str::trim).collect();
    let (kind, label, attrs_start) =
        if let Some(label) = parts.first().and_then(|part| part.strip_prefix("label:")) {
            ("meta", label.trim(), 1)
        } else if let Some((kind, label)) = parts.first().and_then(|part| part.split_once(':')) {
            (kind.trim(), label.trim(), 1)
        } else if parts.len() >= 2 {
            (parts[0], parts[1], 2)
        } else {
            return None;
        };
    let kind = if kind == "source" { "meta" } else { kind };
    let attrs = parts
        .iter()
        .skip(attrs_start)
        .filter(|part| !part.is_empty())
        .map(|part| match part.split_once(':') {
            Some((role, binding)) => (role.trim().to_string(), binding.trim().to_string()),
            None => (part.to_string(), String::new()),
        })
        .collect();
    Some(Directive {
        kind: kind.to_string(),
        label: label.to_string(),
        attrs,
    })
}

#[derive(Debug)]
struct Anchor {
    line: usize,
    attrs: Vec<(String, String)>,
    /// The `----` lines of the quote under the anchor.
    quote: Option<(usize, usize)>,
}

/// One quoted document: its `[sources/...]` file and its text.
#[derive(Debug)]
struct QuotedPart {
    file: String,
    text: Vec<String>,
}

fn trim_blank(lines: &[String]) -> Vec<String> {
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(start, |end| end + 1);
    lines[start..end].to_vec()
}

/// A quote's text before its first `[sources/...]` line, and its parts.
fn split_quote(lines: &[String]) -> (Vec<String>, Vec<QuotedPart>) {
    let mut preamble = Vec::new();
    let mut parts: Vec<QuotedPart> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let alone = index.checked_sub(1).is_none_or(|i| lines[i].trim().is_empty())
            && lines.get(index + 1).is_none_or(|next| next.trim().is_empty());
        if trimmed.len() > 2 && trimmed.starts_with('[') && trimmed.ends_with(']') && alone {
            parts.push(QuotedPart {
                file: trimmed[1..trimmed.len() - 1].trim().to_string(),
                text: Vec::new(),
            });
            continue;
        }
        match parts.last_mut() {
            Some(part) => part.text.push(line.clone()),
            None => preamble.push(line.clone()),
        }
    }
    for part in &mut parts {
        part.text = trim_blank(&part.text);
    }
    (trim_blank(&preamble), parts)
}

/// The elements of `B = Ctor(attachments = (a, b, ...))`, when `B` has
/// exactly that shape.
fn attachment_elements(text: &str) -> Option<(String, char, Vec<String>)> {
    let (_, value) = text.split_once('=')?.1.split_once('=').map(|(a, b)| (a, b))?;
    let value = value.trim();
    let (constructor, rest) = value.split_once('(')?;
    let rest = rest.trim().strip_suffix(')')?.trim();
    let rest = rest.strip_prefix("attachments")?.trim_start().strip_prefix('=')?.trim();
    let open = rest.chars().next()?;
    let close = match open {
        '(' => ')',
        '[' => ']',
        _ => return None,
    };
    let inner = rest[1..].trim_end().strip_suffix(close)?;
    let mut elements = Vec::new();
    let mut depth = 0i64;
    let mut current = String::new();
    let mut in_string = false;
    for character in inner.chars() {
        match character {
            '"' => in_string = !in_string,
            '(' | '[' if !in_string => depth += 1,
            ')' | ']' if !in_string => depth -= 1,
            ',' if depth == 0 && !in_string => {
                let element = current.split_whitespace().collect::<Vec<_>>().join(" ");
                if !element.is_empty() {
                    elements.push(element);
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    if depth != 0 {
        return None;
    }
    let element = current.split_whitespace().collect::<Vec<_>>().join(" ");
    if !element.is_empty() {
        elements.push(element);
    }
    Some((constructor.trim().to_string(), open, elements))
}

/// A migrated citation: `in sources/<file>` named by binding, and parts a
/// paragraph of the same citation contains dropped.
fn migrate_citation(
    text: &str,
    program: &MigrationProgram,
) -> Result<(String, Vec<String>), String> {
    let mut parts = Vec::new();
    for part in text.split(';') {
        let part = part.trim();
        match part.split_once(" in ") {
            Some((reference, file)) if file.contains('/') || file.contains('.') => {
                let (binding, _) = program.document(file.trim())?;
                parts.push(format!("{} in {binding}", reference.trim()));
            }
            _ => parts.push(part.to_string()),
        }
    }
    let joined = parts.join("; ");
    let Ok(references) = crate::citations::parse_citation(&joined) else {
        return Ok((joined, Vec::new()));
    };
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (index, reference) in references.iter().enumerate() {
        // Of two parts that name the same passage (`§ 5, nr. 2` and
        // `§ 5, stk. 1, nr. 2`) the first is kept.
        let contained = references.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && crate::citations::contains_passage(other, reference)
                && !(other_index > index && crate::citations::contains_passage(reference, other))
        });
        if contained {
            dropped.push(parts[index].clone());
        } else {
            kept.push(parts[index].clone());
        }
    }
    Ok((kept.join("; "), dropped))
}

/// The top-level items between two lines: `(line, name)`.
fn items_between(lines: &[&str], start: usize, end: usize) -> Vec<(usize, Option<String>)> {
    let mut items = Vec::new();
    let mut in_block = false;
    for index in start..end.min(lines.len()) {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.starts_with("----") {
            if in_block || crate::block_comment_delimiter_count(trimmed) % 2 == 1 {
                in_block = !in_block;
            }
            continue;
        }
        if in_block || trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with('@') {
            continue;
        }
        if line.starts_with([' ', '\t', '}', ')', ']']) {
            continue;
        }
        let name = crate::meta_type_symbol_name(trimmed)
            .or_else(|| crate::meta_rule_symbol_name(trimmed))
            .or_else(|| crate::meta_fn_symbol_name(trimmed))
            .or_else(|| crate::meta_binding_symbol_name(trimmed));
        items.push((index, name));
    }
    items
}

/// The first line of the declaration annotations (`@ calculate(...)`,
/// `@ field(...)`) directly above `line`.
fn declaration_start(lines: &[&str], line: usize) -> usize {
    let mut start = line;
    while start > 0 {
        let previous = lines[start - 1].trim();
        if previous.starts_with('@') {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

/// Rewrite the comment directives of `source`. On failure, every reason the
/// file cannot be rewritten, by line.
pub fn migrate_source(source: &str, program: &MigrationProgram) -> Result<MigratedFile, Vec<String>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut errors = Vec::new();
    let mut notes = Vec::new();
    let mut document_constructors = BTreeSet::new();

    // Directives outside block comments, and the quote under each anchor.
    let mut anchors: BTreeMap<String, Anchor> = BTreeMap::new();
    let mut anchor_order: Vec<String> = Vec::new();
    let mut spans: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut open_spans: Vec<(String, usize)> = Vec::new();
    let mut cites: Vec<(usize, String)> = Vec::new();
    let mut quote_lines: BTreeSet<usize> = BTreeSet::new();
    let mut in_block = false;
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.starts_with("----") {
            if in_block || crate::block_comment_delimiter_count(trimmed) % 2 == 1 {
                in_block = !in_block;
            }
            index += 1;
            continue;
        }
        if in_block {
            index += 1;
            continue;
        }
        let Some(directive) = parse_directive(trimmed) else {
            if trimmed.starts_with("--@") {
                errors.push(format!(
                    "{}: `{trimmed}` is not a directive the migration reads",
                    index + 1
                ));
            }
            index += 1;
            continue;
        };
        match directive.kind.as_str() {
            "meta" => {
                let mut next = index + 1;
                while next < lines.len() && lines[next].trim().is_empty() {
                    next += 1;
                }
                let quote = (next < lines.len() && lines[next].trim().starts_with("----"))
                    .then(|| {
                        let close = (next + 1..lines.len())
                            .find(|line| lines[*line].trim().starts_with("----"))?;
                        Some((next, close))
                    })
                    .flatten();
                if let Some((open, close)) = quote {
                    quote_lines.extend(open..=close);
                }
                match anchors.get_mut(&directive.label) {
                    Some(anchor) => {
                        anchor.attrs.extend(directive.attrs);
                        if anchor.quote.is_some() && quote.is_some() {
                            errors.push(format!(
                                "{}: anchor `{}` has a second quote; merge its quotes into one",
                                index + 1,
                                directive.label
                            ));
                        }
                        anchor.quote = anchor.quote.or(quote);
                    }
                    None => {
                        anchor_order.push(directive.label.clone());
                        anchors.insert(
                            directive.label.clone(),
                            Anchor {
                                line: index,
                                attrs: directive.attrs,
                                quote,
                            },
                        );
                    }
                }
                if let Some((_, close)) = quote {
                    index = close + 1;
                    continue;
                }
            }
            "begin" => {
                if lines[index].starts_with([' ', '\t']) {
                    errors.push(format!(
                        "{}: `--@begin:{}--` is indented; a section stands at top level",
                        index + 1,
                        directive.label
                    ));
                }
                open_spans.push((directive.label, index));
            }
            "end" => match open_spans.iter().rposition(|(label, _)| *label == directive.label) {
                Some(position) if position + 1 == open_spans.len() => {
                    let (label, begin) = open_spans.remove(position);
                    if spans.insert(label.clone(), (begin, index)).is_some() {
                        errors.push(format!(
                            "{}: section `{label}` has two spans; a section is one block",
                            index + 1
                        ));
                    }
                }
                Some(_) => errors.push(format!(
                    "{}: `--@end:{}--` closes a span that encloses another open span; sections do not nest",
                    index + 1,
                    directive.label
                )),
                None => errors.push(format!(
                    "{}: `--@end:{}--` has no matching `--@begin`",
                    index + 1,
                    directive.label
                )),
            },
            "cite" => cites.push((index, directive.label)),
            other => errors.push(format!(
                "{}: `--@{other}` is not a directive the migration reads",
                index + 1
            )),
        }
        index += 1;
    }
    for (label, begin) in &open_spans {
        errors.push(format!(
            "{}: `--@begin:{label}--` has no matching `--@end`",
            begin + 1
        ));
    }

    let mut removed: BTreeSet<usize> = BTreeSet::new();
    let mut replaced: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut inserted: BTreeMap<usize, Vec<String>> = BTreeMap::new();

    // Metadata lines of one anchor, after rule 2 moved roles onto quotes.
    let meta_lines = |attrs: &[(String, String)], notes: &mut Vec<String>, label: &str| {
        let mut values = Vec::new();
        for (role, binding) in attrs {
            if binding.is_empty() {
                continue;
            }
            if role != "meta" {
                notes.push(format!(
                    "`{label}`: the `{role}` role of `{binding}` is not written in `@ meta`; a role is a `MetaRole` value inside the metadata value"
                ));
            }
            values.push(format!("@ meta({binding})"));
        }
        values
    };

    let mut labels: Vec<String> = anchor_order.clone();
    for label in spans.keys() {
        if !labels.contains(label) {
            labels.push(label.clone());
        }
    }
    for label in &labels {
        let anchor = anchors.get(label);
        let span = spans.get(label).copied();
        let quote_text: Option<Vec<String>> = anchor.and_then(|anchor| anchor.quote).map(|(open, close)| {
            lines[open + 1..close].iter().map(|line| line.to_string()).collect()
        });
        let (preamble, parts) = quote_text
            .as_deref()
            .map(split_quote)
            .unwrap_or_default();
        let preamble_block = (!preamble.is_empty()).then(|| {
            let mut block = vec!["----".to_string()];
            block.extend(preamble.iter().cloned());
            block.push("----".to_string());
            block.push(String::new());
            block
        });
        if let Some(anchor) = anchor {
            removed.insert(anchor.line);
            if let Some((open, close)) = anchor.quote {
                removed.extend(open..=close);
            }
        }

        let span_items = span
            .map(|(begin, end)| items_between(&lines, begin + 1, end))
            .unwrap_or_default();
        let declaration_only = parts.is_empty()
            && !span_items.is_empty()
            && span_items.iter().all(|(_, name)| name.as_deref() == Some(label.as_str()));

        let attrs: Vec<(String, String)> = anchor.map(|anchor| anchor.attrs.clone()).unwrap_or_default();

        if let Some((begin, end)) = span.filter(|_| !declaration_only) {
            // Rule 1: a section.
            let mut quotes: Vec<(String, Vec<String>)> = Vec::new();
            let mut documents = Vec::new();
            for part in &parts {
                match program.document(&part.file) {
                    Ok((binding, constructor)) => {
                        document_constructors.insert(constructor.clone());
                        documents.push(binding.clone());
                        quotes.push((binding.clone(), part.text.clone()));
                    }
                    Err(message) => errors.push(format!(
                        "{}: {message}",
                        anchor.map_or(begin, |anchor| anchor.line) + 1
                    )),
                }
            }
            // Rule 2: roles of quoted documents move onto the quotes.
            let mut quote_arguments: Vec<String> = documents.clone();
            let mut metas: Vec<String> = Vec::new();
            for (role, binding) in &attrs {
                let definition = lines
                    .iter()
                    .position(|line| binding_head(line).is_some_and(|(name, _)| name == *binding));
                let shared = program.words.get(binding).copied().unwrap_or(0) > 2;
                let moved = (role == "meta" && !documents.is_empty() && !shared)
                    .then(|| definition)
                    .flatten()
                    .and_then(|definition| {
                        let (text, last) = balanced_text(&lines, definition);
                        attachment_elements(&text).map(|parsed| (definition, last, parsed))
                    });
                let Some((definition, last, (constructor, open, elements))) = moved else {
                    if role == "meta" && !documents.is_empty() && shared {
                        notes.push(format!(
                            "`{label}`: `{binding}` is used elsewhere too, so the roles of its quoted documents stay in it"
                        ));
                    }
                    metas.extend(meta_lines(&[(role.clone(), binding.clone())], &mut notes, label));
                    continue;
                };
                let mut remaining = Vec::new();
                let mut assigned = vec![false; documents.len()];
                for element in elements {
                    let target = quote_argument(&element).ok().and_then(|(document, role)| {
                        role?;
                        documents
                            .iter()
                            .enumerate()
                            .position(|(index, candidate)| *candidate == document && !assigned[index])
                    });
                    match target {
                        Some(index) => {
                            assigned[index] = true;
                            for (other, candidate) in documents.iter().enumerate() {
                                if *candidate == documents[index] {
                                    assigned[other] = true;
                                    quote_arguments[other] = element.clone();
                                }
                            }
                        }
                        None => remaining.push(element),
                    }
                }
                removed.extend(definition..=last);
                if lines.get(last + 1).is_some_and(|line| line.trim().is_empty()) {
                    removed.insert(last + 1);
                }
                match remaining.len() {
                    0 => {}
                    1 => metas.push(format!("@ meta({})", remaining[0])),
                    _ => {
                        let close = if open == '(' { ')' } else { ']' };
                        let mut rewritten = vec![
                            format!("= {binding} = {constructor}("),
                            format!("    attachments = {open}"),
                        ];
                        for element in &remaining {
                            rewritten.push(format!("        {element},"));
                        }
                        rewritten.push(format!("    {close}"));
                        rewritten.push(")".to_string());
                        rewritten.push(String::new());
                        removed.remove(&definition);
                        replaced.insert(definition, rewritten);
                        metas.push(format!("@ meta({binding})"));
                    }
                }
            }
            let mut header = preamble_block.clone().unwrap_or_default();
            header.extend(metas);
            header.push(format!("@ section {label} {{"));
            header.push(String::new());
            for ((_, text), argument) in quotes.iter().zip(&quote_arguments) {
                header.push(format!("@ quote({argument})"));
                header.push("----".to_string());
                header.extend(text.iter().cloned());
                header.push("----".to_string());
                header.push(String::new());
            }
            replaced.insert(begin, header);
            replaced.insert(end, vec![String::new(), "}".to_string()]);
            continue;
        }

        // Rule 3: metadata on the declaration `label`.
        if !parts.is_empty() {
            errors.push(format!(
                "{}: the quote of `{label}` names documents but no `--@begin:{label}--` span marks the code that implements it",
                anchor.map_or(0, |anchor| anchor.line) + 1
            ));
            continue;
        }
        if let Some((begin, end)) = span {
            removed.insert(begin);
            removed.insert(end);
        }
        if let (Some(anchor), Some(block)) = (anchor, preamble_block) {
            // Quoted text with no saved document stays a block comment.
            inserted.entry(anchor.line).or_default().extend(block);
        }
        let metas = meta_lines(&attrs, &mut notes, label);
        if metas.is_empty() {
            continue;
        }
        let after = anchor.map_or(0, |anchor| anchor.line);
        let named = |start: usize, end: usize| {
            items_between(&lines, start, end)
                .into_iter()
                .find(|(_, name)| name.as_deref() == Some(label.as_str()))
                .map(|(line, _)| line)
        };
        let declaration = span_items
            .first()
            .map(|(line, _)| *line)
            .or_else(|| named(after, lines.len()))
            .or_else(|| named(0, after));
        let Some(declaration) = declaration else {
            errors.push(format!(
                "{}: anchor `{label}` names no declaration in this file; write `@ meta(...)` above the item it describes",
                after + 1
            ));
            continue;
        };
        let start = declaration_start(&lines, declaration);
        inserted.entry(start).or_default().extend(metas);
    }

    for (line, text) in &cites {
        let indent: String = lines[*line]
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        match migrate_citation(text, program) {
            Ok((migrated, dropped)) => {
                for part in dropped {
                    notes.push(format!(
                        "line {}: citation `{text}` names a paragraph and a part of it; `{part}` is dropped, so the explanation quotes the whole paragraph once",
                        line + 1
                    ));
                }
                replaced.insert(*line, vec![format!("{indent}@ cite({migrated})")]);
            }
            Err(message) => errors.push(format!("{}: {message}", line + 1)),
        }
    }

    if !errors.is_empty() {
        errors.sort();
        return Err(errors);
    }
    let mut output: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(before) = inserted.get(&index) {
            output.extend(before.iter().cloned());
        }
        if let Some(replacement) = replaced.get(&index) {
            output.extend(replacement.iter().cloned());
        } else if !removed.contains(&index) {
            output.push(line.to_string());
        }
    }
    let mut text = output.join("\n");
    text.push('\n');
    Ok(MigratedFile {
        source: text,
        notes,
        document_constructors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCES: &str = "# SourceInfo(file: String, identifier: String)
# Meta(a) = Source(value: a) | Guidance(value: a) | Assumption(value: a)
# Metadata(a) = Metadata(attachments: a)
= law = SourceInfo(file = \"sources/law.txt\", identifier = \"Lov\")
= guide = SourceInfo(file = \"sources/guide.md\", identifier = \"Vejledning\")
";

    const OLD: &str = "= assumption_x = \"x\"

= ceiling_meta = Metadata(
    attachments = (
        Source(value = law),
        Guidance(value = guide),
        Assumption(value = assumption_x),
    )
)

--@label:ceiling::meta:ceiling_meta--
----
[sources/law.txt]

§ 9. Text.

Stk. 2. More.

[sources/guide.md]

Guide text.
----

--@begin:ceiling--
--@cite:§ 9; § 9, stk. 2--
| f(x: Int) -> x
--@cite:§ 9, stk. 2 in sources/law.txt--
| g(x: Int) -> x
--@end:ceiling--

--@label:entry::meta:guide_meta--
--@begin:entry--
@ calculate
> entry(x: Int) -> Int {
    f(x)
}
--@end:entry--
";

    fn program() -> MigrationProgram {
        let mut program = MigrationProgram::default();
        program.add_source(Path::new("sources.runa"), SOURCES);
        program.add_source(Path::new("model.runa"), OLD);
        program
    }

    #[test]
    fn a_labelled_quote_and_span_become_a_section() {
        let migrated = migrate_source(OLD, &program()).unwrap();
        let expected = "= assumption_x = \"x\"

@ meta(Assumption(value = assumption_x))
@ section ceiling {

@ quote(Source(value = law))
----
§ 9. Text.

Stk. 2. More.
----

@ quote(Guidance(value = guide))
----
Guide text.
----

@ cite(§ 9)
| f(x: Int) -> x
@ cite(§ 9, stk. 2 in law)
| g(x: Int) -> x

}

@ meta(guide_meta)
@ calculate
> entry(x: Int) -> Int {
    f(x)
}
";
        let squeezed = |text: &str| {
            let mut out = Vec::new();
            for line in text.lines() {
                if !(line.is_empty() && out.last().is_some_and(|last: &&str| last.is_empty())) {
                    out.push(line);
                }
            }
            out.join("\n")
        };
        assert_eq!(squeezed(&migrated.source), squeezed(expected).trim_end());
        assert!(migrated.notes[0].contains("`§ 9, stk. 2` is dropped"));
        assert_eq!(
            migrated.document_constructors,
            BTreeSet::from(["SourceInfo".to_string()])
        );
    }

    #[test]
    fn quoted_text_without_a_document_stays_a_block_comment() {
        let source = "--@label:checks--\n----\nInput checks.\n----\n--@begin:checks--\n| a(x: Int) -> x\n| b(x: Int) -> x\n--@end:checks--\n";
        let migrated = migrate_source(source, &MigrationProgram::default()).unwrap();
        assert_eq!(
            migrated.source,
            "----\nInput checks.\n----\n\n@ section checks {\n\n| a(x: Int) -> x\n| b(x: Int) -> x\n\n}\n"
        );
    }

    #[test]
    fn what_cannot_be_rewritten_stops_the_file() {
        let unmatched = "--@begin:a--\n| f(x: Int) -> x\n";
        assert!(migrate_source(unmatched, &MigrationProgram::default()).unwrap_err()[0]
            .contains("no matching `--@end`"));
        let unknown = "--@label:a::meta:m--\n----\n[sources/nowhere.txt]\n\ntext\n----\n--@begin:a--\n| f(x: Int) -> x\n--@end:a--\n";
        assert!(migrate_source(unknown, &MigrationProgram::default()).unwrap_err()[0]
            .contains("names a document no binding declares"));
    }

    #[test]
    fn document_impls_follow_their_type() {
        assert_eq!(
            add_document_impl(SOURCES, "SourceInfo").unwrap(),
            SOURCES.replacen(
                "identifier: String)\n",
                "identifier: String)\n# impl Document for SourceInfo {}\n",
                1
            )
        );
    }
}
