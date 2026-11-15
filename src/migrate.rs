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
    /// Types with `# impl MetaRole for <type> {}`.
    meta_role_types: BTreeSet<String>,
    /// The constructors each declared type has.
    constructors: BTreeMap<String, Vec<String>>,
    /// Files with an `@ calculate` entry: the calculations whose inputs may
    /// reach a field another file declares.
    calculation_files: Vec<PathBuf>,
    /// Some file uses `unknown` as a name, which 0.5 reserves.
    names_unknown: bool,
    /// Some file has the identifier `unknown_`, the name `unknown` would get.
    has_unknown_: bool,
    /// Every named scalar type (`# Isin: String "..."`), with what its
    /// `@ field` says apart from `key` (empty without one).
    scalar_field_meta: BTreeMap<String, Vec<(String, String)>>,
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
        if let Ok(module) = crate::parse_source_module_cached(path, source) {
            for statement in module.statements() {
                if let crate::Stmt::TypeDecl(crate::TypeDecl::Scalar {
                    name, field_meta, ..
                }) = statement
                {
                    self.scalar_field_meta.insert(
                        name.clone(),
                        field_meta.as_ref().map(presentation).unwrap_or_default(),
                    );
                }
            }
        }
        let lines: Vec<&str> = source.lines().collect();
        for word in identifiers(source) {
            *self.words.entry(word.to_string()).or_default() += 1;
        }
        if source
            .lines()
            .any(|line| line.trim_start().starts_with("@ calculate"))
        {
            self.calculation_files.push(path.to_path_buf());
        }
        self.names_unknown |= unknown_tokens(source).iter().any(|token| token.named);
        self.has_unknown_ |= lexes_identifier(source, "unknown_");
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
            } else if let Some(rest) = line.strip_prefix("# impl MetaRole for ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| *c == '_' || c.is_alphanumeric())
                    .collect();
                self.meta_role_types.insert(name);
            } else if let Some(rest) = line.strip_prefix("# ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| *c == '_' || c.is_alphanumeric())
                    .collect();
                if !name.starts_with(char::is_uppercase) || name == "impl" {
                    continue;
                }
                if rest[name.len()..].starts_with('(') {
                    self.types
                        .entry(name.clone())
                        .or_insert_with(|| (path.to_path_buf(), index));
                }
                let (text, _) = balanced_text(&lines, index);
                self.constructors
                    .entry(name.clone())
                    .or_insert_with(|| type_constructors(&name, &text));
            }
        }
    }

    /// The constructor of the program's `MetaRole` wrapper whose role is
    /// `role` (`source` is `Source`).
    fn role_wrapper(&self, role: &str) -> Result<String, String> {
        let wrappers: BTreeSet<&String> = self
            .meta_role_types
            .iter()
            .filter_map(|type_name| self.constructors.get(type_name))
            .flatten()
            .filter(|constructor| crate::meta_constructor_role_identifier(constructor) == role)
            .collect();
        let wrapper = role
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<String>();
        match wrappers.into_iter().collect::<Vec<_>>().as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(format!(
                "a role is a type, and no type implementing `MetaRole` has a constructor `{wrapper}`; declare `# {wrapper}(value: T)` with `# impl MetaRole for {wrapper} {{}}` and `# impl Meta for {wrapper} {{}}` (or add a `{wrapper}(value: a)` variant to an existing `MetaRole` type), then migrate again"
            )),
            several => Err(format!(
                "more than one `MetaRole` type has a constructor for it ({}); keep one",
                several
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

/// The constructors a type declaration `# Name...` declares: the record
/// itself, or each variant of a sum type.
fn type_constructors(name: &str, text: &str) -> Vec<String> {
    let rest = text.trim().strip_prefix("# ").unwrap_or(text)[name.len()..].to_string();
    let mut depth = 0i32;
    let mut equals = None;
    for (offset, character) in rest.char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '=' if depth == 0 => {
                equals = Some(offset);
                break;
            }
            _ => {}
        }
    }
    let Some(equals) = equals else {
        return vec![name.to_string()];
    };
    let mut constructors = Vec::new();
    let mut depth = 0i32;
    let mut start = equals + 1;
    let body = &rest;
    for (offset, character) in body
        .char_indices()
        .skip_while(|(offset, _)| *offset <= equals)
    {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '|' if depth == 0 => {
                constructors.push(body[start..offset].to_string());
                start = offset + 1;
            }
            _ => {}
        }
    }
    constructors.push(body[start..].to_string());
    constructors
        .iter()
        .map(|variant| {
            variant
                .trim()
                .chars()
                .take_while(|c| *c == '_' || c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|constructor| !constructor.is_empty())
        .collect()
}

impl MigrationProgram {
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

    /// The types that quoted documents of `file` (whose text is `source`)
    /// have but that do not implement `Document`, with the file declaring
    /// each: `file` itself when it declares the type (independent models
    /// may declare types of one name), else the program's declaration.
    pub fn missing_document_impls(
        &self,
        constructors: &BTreeSet<String>,
        file: &Path,
        source: &str,
    ) -> Vec<(String, PathBuf)> {
        constructors
            .iter()
            .filter_map(|name| {
                if source
                    .lines()
                    .any(|line| line.starts_with(&format!("# {name}(")))
                {
                    let implemented = source
                        .lines()
                        .any(|line| line.starts_with(&format!("# impl Document for {name} ")));
                    return (!implemented).then(|| (name.clone(), file.to_path_buf()));
                }
                if self.document_types.contains(name) {
                    return None;
                }
                self.types
                    .get(name)
                    .map(|(path, _)| (name.clone(), path.clone()))
            })
            .collect()
    }
}

/// What an `@ field` says apart from `key`, comparable across files.
fn presentation(meta: &crate::FieldMetaDecl) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = meta
        .entries
        .iter()
        .filter(|(key, _)| key != "key")
        .map(|(key, value)| (key.clone(), format!("{:?}", crate::strip_spans_expr(value))))
        .collect();
    entries.sort();
    entries
}

impl MigrationProgram {
    /// The named scalar a field of this type holds: the type itself, or the
    /// item of an `Option`, `List` or `Set`, or the value of a `Map`.
    fn held_scalar<'a>(&self, ty: &'a crate::Ty) -> Option<&'a str> {
        use crate::Ty;
        match ty {
            Ty::Name(name) => self
                .scalar_field_meta
                .contains_key(name)
                .then_some(name.as_str()),
            Ty::Optional(inner) | Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) => {
                self.held_scalar(inner)
            }
            Ty::App(head, arguments) => match (head.as_ref(), arguments.as_slice()) {
                (Ty::Name(head), [item]) if matches!(head.as_str(), "Option" | "List" | "Set") => {
                    self.held_scalar(item)
                }
                (Ty::Name(head), [_, value]) if head == "Map" => self.held_scalar(value),
                _ => None,
            },
            _ => None,
        }
    }

    /// Remove each field-level `@ field` that says what its named scalar's
    /// `@ field` says; one that also sets `key` becomes `@ field(key = True)`.
    /// The rewritten source and the number of entries removed, or `None`
    /// when nothing repeats.
    pub fn remove_repeated_scalar_field_meta(
        &self,
        path: &Path,
        source: &str,
    ) -> Option<(String, usize)> {
        if self.scalar_field_meta.values().all(Vec::is_empty) {
            return None;
        }
        let module = crate::parse_source_module_cached(path, source).ok()?;
        let mut edits: Vec<(usize, usize, &str)> = Vec::new();
        let chars: Vec<char> = source.chars().collect();
        for statement in module.statements() {
            let crate::Stmt::TypeDecl(crate::TypeDecl::ADT {
                variants,
                field_meta: Some(field_meta),
                ..
            }) = statement
            else {
                continue;
            };
            for member in &field_meta.members {
                let Some(scalar) = variants
                    .iter()
                    .find(|variant| variant.name == member.variant)
                    .and_then(|variant| {
                        variant
                            .fields
                            .iter()
                            .find(|field| field.name == member.field)
                    })
                    .and_then(|field| self.held_scalar(&field.ty))
                else {
                    continue;
                };
                let written = presentation(&member.meta);
                if written.is_empty() || written != self.scalar_field_meta[scalar] {
                    continue;
                }
                let (start, end) = (member.meta.span.start, member.meta.span.end);
                if member.meta.value("key").is_some() {
                    edits.push((start, end, "field(key = True)"));
                    continue;
                }
                // From the `@` before `field(...)`.
                let Some(at) = (0..start)
                    .rev()
                    .find(|index| !chars[*index].is_whitespace())
                    .filter(|index| chars[*index] == '@')
                else {
                    continue;
                };
                let line_start = chars[..at]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map_or(0, |i| i + 1);
                let line_end = chars[end..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map_or(chars.len(), |i| end + i);
                let alone = chars[line_start..at].iter().all(|c| c.is_whitespace())
                    && chars[end..line_end].iter().all(|c| c.is_whitespace());
                if alone {
                    edits.push((line_start, (line_end + 1).min(chars.len()), ""));
                } else {
                    let after = chars[end..line_end]
                        .iter()
                        .position(|c| !c.is_whitespace())
                        .map_or(line_end, |i| end + i);
                    edits.push((at, after, ""));
                }
            }
        }
        if edits.is_empty() {
            return None;
        }
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
        let mut chars = chars;
        let removed = edits.len();
        for (start, end, text) in edits {
            chars.splice(start..end, text.chars());
        }
        Some((chars.into_iter().collect(), removed))
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
            if !matches!(name.as_str(), "target" | ".git" | "node_modules")
                && !name.starts_with('.')
            {
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
    /// The anchor's lines: one per `--@label` naming it, and the anchors of
    /// spans merged into it.
    lines: Vec<usize>,
    attrs: Vec<(String, String)>,
    /// The `----` lines of each quote under the anchor.
    quotes: Vec<(usize, usize)>,
}

impl Anchor {
    fn line(&self) -> usize {
        self.lines[0]
    }
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
        let alone = index
            .checked_sub(1)
            .is_none_or(|i| lines[i].trim().is_empty())
            && lines
                .get(index + 1)
                .is_none_or(|next| next.trim().is_empty());
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
    let (_, value) = text
        .split_once('=')?
        .1
        .split_once('=')
        .map(|(a, b)| (a, b))?;
    let value = value.trim();
    let (constructor, rest) = value.split_once('(')?;
    let rest = rest.trim().strip_suffix(')')?.trim();
    let rest = rest
        .strip_prefix("attachments")?
        .trim_start()
        .strip_prefix('=')?
        .trim();
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
pub fn migrate_source(
    source: &str,
    program: &MigrationProgram,
) -> Result<MigratedFile, Vec<String>> {
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
                        // A second anchor for the label: its quote is one
                        // more quote of the same section.
                        anchor.attrs.extend(directive.attrs);
                        anchor.lines.push(index);
                        anchor.quotes.extend(quote);
                    }
                    None => {
                        anchor_order.push(directive.label.clone());
                        anchors.insert(
                            directive.label.clone(),
                            Anchor {
                                lines: vec![index],
                                attrs: directive.attrs,
                                quotes: quote.into_iter().collect(),
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
    // The spans that become sections: (begin, end, label).
    let mut section_spans: Vec<(usize, usize, String)> = Vec::new();

    // Spans that enclose the same code (only directives between their
    // `--@begin` lines and between their `--@end` lines) implement several
    // passages together: one section, holding all their quotes and
    // metadata. Sections do not nest. It is named by the innermost label,
    // the section the code's rules belonged to (the innermost span around a
    // line was its section), so no rule changes section.
    let only_directives = |range: std::ops::Range<usize>| {
        range.into_iter().all(|line| {
            let trimmed = lines[line].trim();
            trimmed.is_empty() || (trimmed.starts_with("--") && !trimmed.starts_with("----"))
        })
    };
    let mut by_begin: Vec<(String, (usize, usize))> = spans
        .iter()
        .map(|(label, span)| (label.clone(), *span))
        .collect();
    by_begin.sort_by_key(|(_, (begin, _))| *begin);
    // The span directly inside each span that marks the same code.
    let mut same_inner: BTreeMap<String, String> = BTreeMap::new();
    for (position, (inner, (inner_begin, inner_end))) in by_begin.iter().enumerate() {
        let Some((outer, (outer_begin, outer_end))) = by_begin[..position]
            .iter()
            .rev()
            .find(|(_, (begin, end))| begin < inner_begin && inner_end < end)
        else {
            continue;
        };
        if only_directives(outer_begin + 1..*inner_begin)
            && only_directives(inner_end + 1..*outer_end)
        {
            same_inner.insert(outer.clone(), inner.clone());
        }
    }
    let mut joins: Vec<(String, String)> = Vec::new();
    for outer in same_inner.keys() {
        let mut innermost = outer;
        while let Some(inner) = same_inner.get(innermost) {
            innermost = inner;
        }
        joins.push((outer.clone(), innermost.clone()));
    }
    // Innermost enclosing span first, so each join puts the next enclosing
    // label's values before those already joined.
    joins.sort_by_key(|(outer, _)| std::cmp::Reverse(spans.get(outer).map(|(begin, _)| *begin)));
    for (outer, innermost) in &joins {
        let Some((begin, end)) = spans.remove(outer) else {
            continue;
        };
        removed.insert(begin);
        removed.insert(end);
        anchor_order.retain(|label| label != outer);
        if let Some(merged) = anchors.remove(outer) {
            match anchors.get_mut(innermost) {
                Some(anchor) => {
                    anchor.lines.extend(merged.lines);
                    // The enclosing label's values stand first, as written.
                    let inner_attrs = std::mem::replace(&mut anchor.attrs, merged.attrs);
                    anchor.attrs.extend(inner_attrs);
                    anchor.quotes.extend(merged.quotes);
                }
                None => {
                    anchor_order.push(innermost.clone());
                    anchors.insert(innermost.clone(), merged);
                }
            }
        }
        notes.push(format!(
            "`{outer}` joins section `{innermost}`: they mark the same code, sections do not nest, and the code's rules belonged to `{innermost}`, the innermost"
        ));
    }
    for anchor in anchors.values_mut() {
        anchor.lines.sort();
        anchor.quotes.sort();
        let mut seen = BTreeSet::new();
        anchor.attrs.retain(|attr| seen.insert(attr.clone()));
    }

    // Metadata lines of one anchor. A role of a chain (`::source:a`) is a
    // type: the value is written in the program's `MetaRole` wrapper of that
    // role, `@ meta(Source(value = a))`, the file's own wrapper first (one
    // directory may hold independent models declaring types of one name).
    // A role with no wrapper stops the file, since dropping it would change
    // what the metadata says.
    let mut own = MigrationProgram::default();
    own.add_source(Path::new(""), source);
    let meta_lines =
        |attrs: &[(String, String)], errors: &mut Vec<String>, label: &str, line: usize| {
            let mut values = Vec::new();
            for (role, binding) in attrs {
                if binding.is_empty() {
                    continue;
                }
                if role == "meta" {
                    values.push(format!("@ meta({binding})"));
                    continue;
                }
                match own
                    .role_wrapper(role)
                    .or_else(|_| program.role_wrapper(role))
                {
                    Ok(wrapper) => values.push(format!("@ meta({wrapper}(value = {binding}))")),
                    Err(message) => errors.push(format!(
                        "{}: `{label}` gives `{binding}` the role `{role}`: {message}",
                        line + 1
                    )),
                }
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
        let quote_texts: Vec<Vec<String>> = anchor
            .map(|anchor| {
                anchor
                    .quotes
                    .iter()
                    .map(|(open, close)| {
                        lines[open + 1..*close]
                            .iter()
                            .map(|line| line.to_string())
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut preamble: Vec<String> = Vec::new();
        let mut parts: Vec<QuotedPart> = Vec::new();
        for text in &quote_texts {
            let (own_preamble, own_parts) = split_quote(text);
            if !own_preamble.is_empty() {
                if !preamble.is_empty() {
                    preamble.push(String::new());
                }
                preamble.extend(own_preamble);
            }
            parts.extend(own_parts);
        }
        let preamble_block = (!preamble.is_empty()).then(|| {
            let mut block = vec!["----".to_string()];
            block.extend(preamble.iter().cloned());
            block.push("----".to_string());
            block.push(String::new());
            block
        });
        if let Some(anchor) = anchor {
            removed.extend(anchor.lines.iter().copied());
            for (open, close) in &anchor.quotes {
                removed.extend(*open..=*close);
            }
        }

        let span_items = span
            .map(|(begin, end)| items_between(&lines, begin + 1, end))
            .unwrap_or_default();
        let declaration_only = parts.is_empty()
            && !span_items.is_empty()
            && span_items
                .iter()
                .all(|(_, name)| name.as_deref() == Some(label.as_str()));

        let attrs: Vec<(String, String)> = anchor
            .map(|anchor| anchor.attrs.clone())
            .unwrap_or_default();

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
                        anchor.map_or(begin, Anchor::line) + 1
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
                    metas.extend(meta_lines(
                        &[(role.clone(), binding.clone())],
                        &mut errors,
                        label,
                        anchor.map_or(begin, Anchor::line),
                    ));
                    continue;
                };
                let mut remaining = Vec::new();
                let mut assigned = vec![false; documents.len()];
                for element in elements {
                    let target = quote_argument(&element).ok().and_then(|(document, role)| {
                        role?;
                        documents.iter().enumerate().position(|(index, candidate)| {
                            *candidate == document && !assigned[index]
                        })
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
                if lines
                    .get(last + 1)
                    .is_some_and(|line| line.trim().is_empty())
                {
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
            section_spans.push((begin, end, label.clone()));
            replaced.insert(end, vec![String::new(), "}".to_string()]);
            continue;
        }

        // Rule 3: metadata on the declaration `label`.
        if !parts.is_empty() {
            errors.push(format!(
                "{}: the quote of `{label}` names documents but no `--@begin:{label}--` span marks the code that implements it",
                anchor.map_or(0, Anchor::line) + 1
            ));
            continue;
        }
        if let Some((begin, end)) = span {
            removed.insert(begin);
            removed.insert(end);
        }
        if let (Some(anchor), Some(block)) = (anchor, preamble_block) {
            // Quoted text with no saved document stays a block comment.
            inserted.entry(anchor.line()).or_default().extend(block);
        }
        let metas = meta_lines(&attrs, &mut errors, label, anchor.map_or(0, Anchor::line));
        if metas.is_empty() {
            continue;
        }
        let after = anchor.map_or(0, Anchor::line);
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
            // Declared in another file: the metadata is written apart, for
            // the declaration by name; `runa check` resolves the name.
            if anchor.is_none() {
                errors.push(format!(
                    "{}: span `{label}` names no declaration in this file",
                    after + 1
                ));
                continue;
            }
            inserted
                .entry(after)
                .or_default()
                .extend(metas.into_iter().map(|meta| format!("{meta} for {label}")));
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
    // Sections do not nest: a span inside another span, around code of its
    // own, moves to just after the enclosing span, so each passage's code
    // stays in its own section.
    section_spans.sort();
    let parent_of = |index: usize| {
        let (begin, end, _) = &section_spans[index];
        section_spans[..index]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (outer_begin, outer_end, _))| outer_begin < begin && end < outer_end)
            .map(|(position, _)| position)
    };
    let parents: Vec<Option<usize>> = (0..section_spans.len()).map(parent_of).collect();
    for (index, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent {
            notes.push(format!(
                "section `{}` stood inside section `{}`; it now follows it, since sections do not nest",
                section_spans[index].2, section_spans[*parent].2
            ));
        }
    }
    fn emit_span(
        index: usize,
        spans: &[(usize, usize, String)],
        parents: &[Option<usize>],
        order: &mut Vec<usize>,
    ) {
        let (begin, end, _) = &spans[index];
        let children: Vec<usize> = (0..spans.len())
            .filter(|child| parents[*child] == Some(index))
            .collect();
        let mut line = *begin;
        while line <= *end {
            if let Some(child) = children.iter().find(|child| spans[**child].0 == line) {
                line = spans[*child].1 + 1;
                continue;
            }
            order.push(line);
            line += 1;
        }
        for child in children {
            emit_span(child, spans, parents, order);
        }
    }
    let mut order: Vec<usize> = Vec::with_capacity(lines.len());
    let mut line = 0;
    while line < lines.len() {
        if let Some(top) = (0..section_spans.len())
            .find(|index| parents[*index].is_none() && section_spans[*index].0 == line)
        {
            emit_span(top, &section_spans, &parents, &mut order);
            line = section_spans[top].1 + 1;
            continue;
        }
        order.push(line);
        line += 1;
    }
    let mut output: Vec<String> = Vec::new();
    for index in order {
        let line = lines[index];
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

// ── 0.5: needs in the language ───────────────────────────────────────────

/// A file's rewrites for docs/rfcs/needs-in-the-language.md, "Migration":
/// the new source, what was rewritten and what was left for the writer,
/// each as `line: message`.
#[derive(Debug)]
pub struct NeedsMigration {
    pub source: String,
    pub rewritten: Vec<String>,
    pub listed: Vec<String>,
}

/// One `unknown` identifier: its character offset and line, and whether its
/// place shows it is a name (`= unknown =`, `unknown: Int`, `unknown(...)`,
/// `x.unknown`, `R(unknown = 1)`, `pathof(R::unknown)`).
struct UnknownToken {
    offset: usize,
    line: usize,
    named: bool,
}

fn source_line_starts(chars: &[char]) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        chars
            .iter()
            .enumerate()
            .filter(|(_, character)| **character == '\n')
            .map(|(index, _)| index + 1),
    );
    starts
}

fn is_word_char(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

/// The `unknown` identifier tokens of `source`. Lexed, so text in strings
/// and comments is not one, and the source need not parse.
fn unknown_tokens(source: &str) -> Vec<UnknownToken> {
    if !source.contains("unknown") {
        return Vec::new();
    }
    let tokens = crate::Lexer::new(source).tokenize();
    let chars: Vec<char> = source.chars().collect();
    let starts = source_line_starts(&chars);
    let bom = usize::from(chars.first() == Some(&'\u{feff}'));
    let text = |index: Option<usize>| {
        index
            .and_then(|index| tokens.get(index))
            .map_or("", |token| token.text.as_str())
    };
    let mut found = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.source_text != "unknown"
            || !matches!(token.kind, crate::TokenKind::Ident | crate::TokenKind::KW)
        {
            continue;
        }
        let Some(start) = token.line.checked_sub(1).and_then(|line| starts.get(line)) else {
            continue;
        };
        let offset = start + token.col.saturating_sub(1) + if token.line == 1 { bom } else { 0 };
        let spelled = chars
            .get(offset..offset + "unknown".len())
            .is_some_and(|word| word.iter().copied().eq("unknown".chars()));
        let bounded = (offset == 0 || !is_word_char(chars[offset - 1]))
            && !chars
                .get(offset + "unknown".len())
                .is_some_and(|character| is_word_char(*character));
        if !spelled || !bounded {
            continue;
        }
        let before = text(index.checked_sub(1));
        let after = text(Some(index + 1));
        found.push(UnknownToken {
            offset,
            line: token.line,
            named: matches!(before, "." | ":") || matches!(after, ":" | "=" | "("),
        });
    }
    found
}

/// A field provided by another entry, in the string form: `@ field(entry =
/// "e", subject = "a.b")` or a field record's `entry = Some("e"), subject =
/// Some("a.b")`.
struct ProvidedField {
    /// `Type::field` for `@ field`; the binding holding the field record.
    binding: String,
    /// The field record's constructor; `None` for `@ field`.
    record: Option<String>,
    entry: String,
    subject: String,
    /// The character ranges of the two string literals, quotes included.
    entry_range: (usize, usize),
    subject_range: (usize, usize),
}

/// The character range of the string literal `value` that the expression
/// at `span` writes, quotes included.
fn literal_range(chars: &[char], span: crate::Span, value: &str) -> Option<(usize, usize)> {
    let open =
        (span.start..span.end.max(span.start + 1)).find(|index| chars.get(*index) == Some(&'"'))?;
    let mut close = open + 1;
    while *chars.get(close)? != '"' {
        close += if chars[close] == '\\' { 2 } else { 1 };
    }
    chars[open + 1..close]
        .iter()
        .copied()
        .eq(value.chars())
        .then_some((open, close + 1))
}

/// The string literal `expr` is, or with `optional` the one in `Some("...")`.
fn provided_literal<'a>(expr: &'a crate::Expr, optional: bool) -> Option<(&'a str, crate::Span)> {
    let expr = if optional {
        let crate::ExprKind::App(function, arguments) = &expr.kind else {
            return None;
        };
        if !matches!(&function.kind, crate::ExprKind::Var(name) if name == "Some")
            || arguments.len() != 1
        {
            return None;
        }
        &arguments[0]
    } else {
        expr
    };
    match &expr.kind {
        crate::ExprKind::Lit(crate::Literal::Str(value)) => Some((value.as_str(), expr.span)),
        _ => None,
    }
}

fn push_provided_field(
    binding: String,
    record: Option<String>,
    entry: &crate::Expr,
    subject: &crate::Expr,
    chars: &[char],
    fields: &mut Vec<ProvidedField>,
) {
    let optional = record.is_some();
    let (Some((entry, entry_span)), Some((subject, subject_span))) = (
        provided_literal(entry, optional),
        provided_literal(subject, optional),
    ) else {
        return;
    };
    let (Some(entry_range), Some(subject_range)) = (
        literal_range(chars, entry_span, entry),
        literal_range(chars, subject_span, subject),
    ) else {
        return;
    };
    fields.push(ProvidedField {
        binding,
        record,
        entry: entry.to_string(),
        subject: subject.to_string(),
        entry_range,
        subject_range,
    });
}

/// The field records in a binding's value with string `entry` and `subject`.
fn record_provided_fields(
    binding: &str,
    expr: &crate::Expr,
    chars: &[char],
    fields: &mut Vec<ProvidedField>,
) {
    match &expr.kind {
        crate::ExprKind::App(function, arguments) => {
            if let crate::ExprKind::Var(constructor) = &function.kind {
                let named = |key: &str| {
                    arguments
                        .iter()
                        .filter_map(crate::named_arg_parts)
                        .find(|(field, _)| *field == key)
                        .map(|(_, value)| value)
                };
                if constructor.starts_with(char::is_uppercase) {
                    if let (Some(entry), Some(subject)) = (named("entry"), named("subject")) {
                        push_provided_field(
                            binding.to_string(),
                            Some(constructor.clone()),
                            entry,
                            subject,
                            chars,
                            fields,
                        );
                    }
                }
            }
            for argument in arguments {
                record_provided_fields(binding, argument, chars, fields);
            }
        }
        crate::ExprKind::List(items) | crate::ExprKind::Tuple(items) => {
            for item in items {
                record_provided_fields(binding, item, chars, fields);
            }
        }
        _ => {}
    }
}

fn provided_fields(stmts: &[crate::Stmt], chars: &[char]) -> Vec<ProvidedField> {
    let mut fields = Vec::new();
    for stmt in stmts {
        match stmt {
            crate::Stmt::TypeDecl(crate::TypeDecl::ADT {
                name,
                field_meta: Some(meta),
                ..
            }) => {
                let members = meta.members.iter().map(|member| {
                    let binding = if &member.variant == name {
                        format!("{name}::{}", member.field)
                    } else {
                        format!("{name}::{}::{}", member.variant, member.field)
                    };
                    (binding, &member.meta)
                });
                for (binding, declaration) in meta
                    .type_default
                    .iter()
                    .map(|declaration| (name.clone(), declaration))
                    .chain(members)
                {
                    if let (Some(entry), Some(subject)) =
                        (declaration.value("entry"), declaration.value("subject"))
                    {
                        push_provided_field(binding, None, entry, subject, chars, &mut fields);
                    }
                }
            }
            crate::Stmt::Bind(crate::Pat::Var(binding), _, expr) => {
                record_provided_fields(binding, expr, chars, &mut fields)
            }
            _ => {}
        }
    }
    fields
}

/// Whether `stmts` (in `directory`) import `target`, directly or through
/// another import.
fn imports_file(
    stmts: &[crate::Stmt],
    directory: &str,
    target: &Path,
    visited: &mut BTreeSet<PathBuf>,
) -> bool {
    for stmt in stmts {
        let crate::Stmt::Import(import) = stmt else {
            continue;
        };
        let Some(file) = crate::Interpreter::resolve_import_path_for_source(import, directory)
        else {
            continue;
        };
        let canonical = std::fs::canonicalize(&file).unwrap_or_else(|_| PathBuf::from(&file));
        if canonical == target {
            return true;
        }
        if !visited.insert(canonical.clone()) {
            continue;
        }
        let Ok(module) = crate::parse_source_module_file_cached(Path::new(&file)) else {
            continue;
        };
        let nested = canonical.parent().map_or_else(
            || ".".to_string(),
            |parent| parent.to_string_lossy().to_string(),
        );
        if imports_file(module.statements(), &nested, target, visited) {
            return true;
        }
    }
    false
}

fn directory_of(path: &Path) -> String {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().to_string(),
        _ => ".".to_string(),
    }
}

/// The calculations of the file and of the program's files importing it,
/// and why those that do not check give none.
fn reaching_contracts(
    stmts: &[crate::Stmt],
    source: &str,
    path: &Path,
    program: &MigrationProgram,
) -> (Vec<crate::calculate::CalculationContract>, Vec<String>) {
    let mut contracts = Vec::new();
    let mut failures = Vec::new();
    let mut extract = |stmts: &[crate::Stmt], source: &str, file: &Path| {
        match crate::calculate::extract_calculation_contracts(
            stmts,
            source,
            Some(directory_of(file)),
        ) {
            Ok(extracted) => contracts.extend(extracted),
            Err(diagnostics) => failures.push(format!(
                "the calculations of `{}` do not check ({})",
                file.display(),
                diagnostics
                    .first()
                    .map_or("no diagnostic", |diagnostic| diagnostic.message.as_str())
            )),
        }
    };
    extract(stmts, source, path);
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    for file in &program.calculation_files {
        if std::fs::canonicalize(file).unwrap_or_else(|_| file.clone()) == target {
            continue;
        }
        let Ok(other) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(other_stmts) =
            crate::Parser::new(crate::Lexer::new(&other).tokenize(), &other).parse_program()
        else {
            continue;
        };
        if imports_file(
            &other_stmts,
            &directory_of(file),
            &target,
            &mut BTreeSet::new(),
        ) {
            extract(&other_stmts, &other, file);
        }
    }
    (contracts, failures)
}

/// The character range of `Record`'s `entry` field type (`String?` or
/// `Option(String)`) in its declaration in `source`.
fn record_entry_type_range(source: &str, record: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    let line = lines
        .iter()
        .position(|line| line.starts_with(&format!("# {record}(")))?;
    let line_start: usize = source.split_inclusive('\n').take(line).map(str::len).sum();
    let (text, _) = balanced_text(&lines, line);
    for (position, _) in text.match_indices("entry") {
        if text[..position]
            .chars()
            .next_back()
            .is_some_and(is_word_char)
        {
            continue;
        }
        let after = &text[position + "entry".len()..];
        let Some(ty) = after.trim_start().strip_prefix(':') else {
            continue;
        };
        let ty = ty.trim_start();
        let Some(spelled) = ["String?", "Option(String)"]
            .into_iter()
            .find(|spelled| ty.starts_with(spelled))
        else {
            continue;
        };
        let start = line_start + text.len() - ty.len();
        let start = source[..start].chars().count();
        return Some((start, start + spelled.len()));
    }
    None
}

/// Rewrite the string `entry`/`subject` forms as `refof`/`pathof` where
/// exactly one calculation input reaches the field; list the others.
fn migrate_provided_fields(
    source: &str,
    path: &Path,
    program: &MigrationProgram,
    chars: &[char],
    line_of: &dyn Fn(usize) -> usize,
    edits: &mut Vec<(usize, usize, String)>,
    rewritten: &mut Vec<String>,
    listed: &mut Vec<String>,
) {
    let Ok(stmts) =
        crate::Parser::new(crate::Lexer::new(source).tokenize(), source).parse_program()
    else {
        return;
    };
    let fields = provided_fields(&stmts, chars);
    if fields.is_empty() {
        return;
    }
    let (contracts, failures) = reaching_contracts(&stmts, source, path, program);
    let provides = |field: &ProvidedField,
                    metadata: &crate::calculate::CalculationFieldMetadata| {
        metadata.provided_by.as_ref().is_some_and(|provider| {
            provider.entry == field.entry && provider.subject == field.subject
        }) && (metadata.binding == field.binding
            || field.record.is_some()
                && (metadata.binding.starts_with(&format!("{}.", field.binding))
                    || metadata.binding.starts_with(&format!("{}[", field.binding))))
    };
    let root = |field: &ProvidedField| -> Result<String, String> {
        if !field.entry.chars().all(is_word_char) || field.entry.is_empty() {
            return Err(format!(
                "`{}` is not the name of a calculation",
                field.entry
            ));
        }
        if field.subject.is_empty()
            || !field.subject.split('.').all(|segment| {
                !segment.is_empty() && segment.chars().all(|c| is_word_char(c) || c == '$')
            })
        {
            return Err(format!("`{}` is not a field path", field.subject));
        }
        let roots: BTreeSet<&str> = contracts
            .iter()
            .filter(|contract| {
                contract
                    .field_metadata
                    .iter()
                    .any(|metadata| provides(field, metadata))
            })
            .filter_map(|contract| match &contract.input {
                crate::calculate::CalculationTypeRef::Named { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        match roots.into_iter().collect::<Vec<_>>().as_slice() {
            [one] => Ok(one.to_string()),
            [] if !failures.is_empty() => Err(failures.join("; ")),
            [] => Err(format!(
                "no calculation's input reaches `{}`, so its subject has no root type",
                field.binding
            )),
            several => Err(format!(
                "the inputs of several calculations reach `{}` ({}); a subject is rooted at one of them",
                field.binding,
                several.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
            )),
        }
    };
    let mut resolved: Vec<(&ProvidedField, Result<String, String>)> =
        fields.iter().map(|field| (field, root(field))).collect();
    // A record's `entry` type changes with its values: every string value of
    // the record in this file is rewritten, and the record is declared here.
    let records: BTreeSet<&str> = fields
        .iter()
        .filter_map(|field| field.record.as_deref())
        .collect();
    let mut record_types = Vec::new();
    for record in records {
        let failed = resolved
            .iter()
            .filter(|(field, result)| field.record.as_deref() == Some(record) && result.is_err())
            .map(|(field, _)| line_of(field.entry_range.0))
            .collect::<Vec<_>>();
        let reason = if !failed.is_empty() {
            Some(format!(
                "another `{record}` value of this file cannot be rewritten (line {}), and `{record}`'s `entry` has one type",
                failed.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ))
        } else {
            match record_entry_type_range(source, record) {
                Some(range) => {
                    record_types.push((record, range));
                    None
                }
                None => Some(format!(
                    "`{record}` is not declared in this file with `entry: String?`; declare `entry: ProgramReference?` and migrate again"
                )),
            }
        };
        if let Some(reason) = reason {
            for (field, result) in resolved.iter_mut() {
                if field.record.as_deref() == Some(record) && result.is_ok() {
                    *result = Err(reason.clone());
                }
            }
        }
    }
    for (field, result) in resolved {
        let line = line_of(field.entry_range.0);
        let (open, close) = if field.record.is_some() {
            ("Some(", ")")
        } else {
            ("", "")
        };
        let old = format!(
            "`entry = {open}\"{}\"{close}`, `subject = {open}\"{}\"{close}`",
            field.entry, field.subject
        );
        match result {
            Ok(root) => {
                let entry = format!("refof({})", field.entry);
                let subject = format!("pathof({root}::{})", field.subject.replace('.', "::"));
                rewritten.push(format!(
                    "{line}: `{}`: {old} are now `entry = {open}{entry}{close}`, `subject = {open}{subject}{close}`",
                    field.binding
                ));
                edits.push((field.entry_range.0, field.entry_range.1, entry));
                edits.push((field.subject_range.0, field.subject_range.1, subject));
            }
            Err(reason) => listed.push(format!(
                "{line}: left {old} on `{}`: {reason}; write `entry = {open}refof(<calculation>){close}`, `subject = {open}pathof(<Input>::...){close}` by hand",
                field.binding
            )),
        }
    }
    for (record, (start, end)) in record_types {
        rewritten.push(format!(
            "{}: `{record}` field `entry` is now `ProgramReference?`, the type of `Some(refof(...))`",
            line_of(start)
        ));
        edits.push((start, end, "ProgramReference?".to_string()));
    }
}

/// Whether `name` is an identifier token of `source`, not text in a string
/// or a comment.
fn lexes_identifier(source: &str, name: &str) -> bool {
    source.contains(name)
        && crate::Lexer::new(source)
            .tokenize()
            .iter()
            .any(|token| token.source_text == name && token.kind == crate::TokenKind::Ident)
}

/// The 0.5 rewrites of the file at `path` (whose text is `source`): string
/// `entry`/`subject` field metadata becomes `refof`/`pathof`, and `unknown`,
/// a reserved word in 0.5, is renamed `unknown_` where the program uses it
/// as a name.
pub fn migrate_needs(source: &str, path: &Path, program: &MigrationProgram) -> NeedsMigration {
    let chars: Vec<char> = source.chars().collect();
    let starts = source_line_starts(&chars);
    let line_of = |offset: usize| starts.partition_point(|start| *start <= offset);
    let mut edits = Vec::new();
    let mut rewritten = Vec::new();
    let mut listed = Vec::new();
    if source.contains("entry") && source.contains("subject") {
        migrate_provided_fields(
            source,
            path,
            program,
            &chars,
            &line_of,
            &mut edits,
            &mut rewritten,
            &mut listed,
        );
    }
    let unknowns = unknown_tokens(source);
    if program.names_unknown || unknowns.iter().any(|token| token.named) {
        let taken = program.has_unknown_ || lexes_identifier(source, "unknown_");
        for token in &unknowns {
            if taken {
                listed.push(format!(
                    "{}: left `unknown`: it is a reserved word in 0.5, and `unknown_` is already a name; rename it by hand",
                    token.line
                ));
            } else {
                rewritten.push(format!(
                    "{}: renamed `unknown` to `unknown_`: `unknown` is a reserved word in 0.5",
                    token.line
                ));
                edits.push((
                    token.offset,
                    token.offset + "unknown".len(),
                    "unknown_".to_string(),
                ));
            }
        }
    }
    let mut output = chars;
    edits.sort_by(|left, right| right.0.cmp(&left.0));
    for (start, end, replacement) in edits {
        output.splice(start..end, replacement.chars());
    }
    let line_number = |message: &String| {
        message
            .split(':')
            .next()
            .and_then(|line| line.parse::<usize>().ok())
            .unwrap_or(0)
    };
    rewritten.sort_by_key(line_number);
    listed.sort_by_key(line_number);
    NeedsMigration {
        source: output.into_iter().collect(),
        rewritten,
        listed,
    }
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
        assert!(
            migrate_source(unmatched, &MigrationProgram::default()).unwrap_err()[0]
                .contains("no matching `--@end`")
        );
        let unknown = "--@label:a::meta:m--\n----\n[sources/nowhere.txt]\n\ntext\n----\n--@begin:a--\n| f(x: Int) -> x\n--@end:a--\n";
        assert!(
            migrate_source(unknown, &MigrationProgram::default()).unwrap_err()[0]
                .contains("names a document no binding declares")
        );
    }

    #[test]
    fn metadata_for_a_declaration_of_another_file_is_written_for_it() {
        let source = "@ import ./domain\n= guide = \"g\"\n--@label:Input::meta:guide--\n\n| f(x: Int) -> x\n";
        let migrated = migrate_source(source, &MigrationProgram::default()).unwrap();
        assert_eq!(
            migrated.source,
            "@ import ./domain\n= guide = \"g\"\n@ meta(guide) for Input\n\n| f(x: Int) -> x\n"
        );
    }

    #[test]
    fn spans_around_the_same_code_are_one_section() {
        let source = "--@label:base::meta:base_meta--\n----\nBase.\n----\n\n--@label:amended::meta:amended_meta--\n----\nAmended.\n----\n\n--@label:base::meta:base_meta--\n----\nMore base.\n----\n\n--@begin:base--\n--@begin:amended--\n| f(x: Int) -> x\n--@end:amended--\n--@end:base--\n";
        let migrated = migrate_source(source, &MigrationProgram::default()).unwrap();
        assert_eq!(
            migrated.source.trim_start(),
            "----\nBase.\n\nAmended.\n\nMore base.\n----\n\n@ meta(base_meta)\n@ meta(amended_meta)\n@ section amended {\n\n| f(x: Int) -> x\n\n}\n"
        );
        assert!(migrated
            .notes
            .iter()
            .any(|note| note.contains("`base` joins section `amended`")));
    }

    #[test]
    fn a_span_inside_another_follows_it() {
        let source = "--@begin:outer--\n| a(x: Int) -> x\n--@begin:inner--\n| b(x: Int) -> x\n--@end:inner--\n| c(x: Int) -> x\n--@end:outer--\n";
        let migrated = migrate_source(source, &MigrationProgram::default()).unwrap();
        assert_eq!(
            migrated.source,
            "@ section outer {\n\n| a(x: Int) -> x\n| c(x: Int) -> x\n\n}\n@ section inner {\n\n| b(x: Int) -> x\n\n}\n"
        );
        assert!(migrated.notes[0].contains("now follows it"));
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

    #[test]
    fn a_role_chain_is_written_in_its_typed_wrappers() {
        let source = "# Shape = Circle | Square\n# Role(a) = Source(value: a) | Warning(value: a)\n# impl MetaRole for Role {}\n# impl Meta for Role {}\n= a = Circle\n= b = Square\n--@label:f::source:a::warning:b::meta:guide--\n| f(x: Int) -> x\n";
        let mut program = MigrationProgram::default();
        program.add_source(Path::new("model.runa"), source);
        let migrated = migrate_source(source, &program).unwrap();
        assert!(
            migrated.source.contains(
                "@ meta(Source(value = a))\n@ meta(Warning(value = b))\n@ meta(guide)\n| f(x: Int) -> x\n"
            ),
            "{}",
            migrated.source
        );
    }

    #[test]
    fn a_role_without_a_wrapper_stops_the_file() {
        let source = "# Role(a) = Source(value: a)\n# impl MetaRole for Role {}\n= a = 1\n= b = 2\n--@label:f::source:a::warning:b--\n| f(x: Int) -> x\n";
        let mut program = MigrationProgram::default();
        program.add_source(Path::new("model.runa"), source);
        let errors = migrate_source(source, &program).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with("5: `f` gives `b` the role `warning`: a role is a type")
                && errors[0].contains("declare `# Warning(value: T)` with `# impl MetaRole for Warning {}` and `# impl Meta for Warning {}`"),
            "{}",
            errors[0]
        );
    }

    fn needs_program(source: &str) -> MigrationProgram {
        let mut program = MigrationProgram::default();
        program.add_source(Path::new("model.runa"), source);
        program
    }

    #[test]
    fn unknown_used_as_a_name_is_renamed_on_its_tokens() {
        // `@ field(` without a key does not parse; the rename does not need it.
        let source = "# Reading(unknown: Int)\n= unknown = 3 -- the unknown default\n| scaled(r: Reading) -> r.unknown + unknown\n= text = \"unknown\"\n@ field(\n";
        let migrated = migrate_needs(source, Path::new("model.runa"), &needs_program(source));
        assert_eq!(
            migrated.source,
            "# Reading(unknown_: Int)\n= unknown_ = 3 -- the unknown default\n| scaled(r: Reading) -> r.unknown_ + unknown_\n= text = \"unknown\"\n@ field(\n"
        );
        assert_eq!(migrated.rewritten.len(), 4, "{:?}", migrated.rewritten);
        assert_eq!(
            migrated.rewritten[0],
            "1: renamed `unknown` to `unknown_`: `unknown` is a reserved word in 0.5"
        );
        assert!(migrated.listed.is_empty(), "{:?}", migrated.listed);
    }

    #[test]
    fn unknown_as_a_value_is_kept() {
        let source = "| f(x: Int) -> x\n= case = f(unknown)\n";
        let migrated = migrate_needs(source, Path::new("model.runa"), &needs_program(source));
        assert_eq!(migrated.source, source);
        assert!(migrated.rewritten.is_empty() && migrated.listed.is_empty());
    }

    #[test]
    fn unknown_is_left_when_unknown_underscore_is_taken() {
        let source = "= unknown = 3\n= unknown_ = 4\n";
        let migrated = migrate_needs(source, Path::new("model.runa"), &needs_program(source));
        assert_eq!(migrated.source, source);
        assert_eq!(
            migrated.listed,
            vec!["1: left `unknown`: it is a reserved word in 0.5, and `unknown_` is already a name; rename it by hand"]
        );
    }

    const PROVIDED: &str = "# Spouse(income: Int)
# Household(
    spouse: Spouse,
    @ field(label = \"Spouse's income\", entry = \"spouse_income\", subject = \"spouse\")
    spouse_income: Int,
    spouse_tax: Int
)
# TaxResult(tax: Int)
# CalculationField(path: String, label: String, question: String?, help: String?, unit: String?, entry: String?, subject: String?)
# CalculationMeta = CalculationMeta(fields: List(CalculationField))
# impl Meta for CalculationMeta {}
= household_meta = CalculationMeta(fields = [
    CalculationField(path = pathof(Household::spouse_tax), label = \"Spouse's tax\", question = None, help = None, unit = None, entry = Some(\"spouse_tax\"), subject = Some(\"spouse\"))
])
@ calculate
| spouse_income(s: Spouse) -> s.income
@ calculate
| spouse_tax(s: Spouse) -> s.income / 4
@ meta(household_meta)
@ calculate
| household_tax(h: Household) -> TaxResult(tax = h.spouse_income - h.spouse_tax)
";

    #[test]
    fn string_entry_and_subject_become_refof_and_pathof() {
        let migrated = migrate_needs(PROVIDED, Path::new("model.runa"), &needs_program(PROVIDED));
        let expected = PROVIDED
            .replace(
                "entry = \"spouse_income\", subject = \"spouse\"",
                "entry = refof(spouse_income), subject = pathof(Household::spouse)",
            )
            .replace(
                "entry = Some(\"spouse_tax\"), subject = Some(\"spouse\")",
                "entry = Some(refof(spouse_tax)), subject = Some(pathof(Household::spouse))",
            )
            .replace(
                "entry: String?, subject",
                "entry: ProgramReference?, subject",
            );
        assert_eq!(migrated.source, expected);
        assert!(migrated.listed.is_empty(), "{:?}", migrated.listed);
        assert_eq!(
            migrated.rewritten,
            vec![
                "4: `Household::spouse_income`: `entry = \"spouse_income\"`, `subject = \"spouse\"` are now `entry = refof(spouse_income)`, `subject = pathof(Household::spouse)`",
                "9: `CalculationField` field `entry` is now `ProgramReference?`, the type of `Some(refof(...))`",
                "13: `household_meta`: `entry = Some(\"spouse_tax\")`, `subject = Some(\"spouse\")` are now `entry = Some(refof(spouse_tax))`, `subject = Some(pathof(Household::spouse))`",
            ]
        );
        // The rewritten metadata gives the contract the same field metadata
        // strings. (The hash changes with the `@ meta` value, which the
        // contract carries as written.)
        let contract = |source: &str| {
            let stmts = crate::Parser::new(crate::Lexer::new(source).tokenize(), source)
                .parse_program()
                .expect("parses");
            crate::calculate::extract_calculation_contracts(&stmts, source, None)
                .expect("checks")
                .into_iter()
                .find(|contract| contract.entry == "household_tax")
                .expect("household_tax")
        };
        let (before, after) = (contract(PROVIDED), contract(&migrated.source));
        assert_eq!(
            serde_json::to_value(&before.field_metadata).unwrap(),
            serde_json::to_value(&after.field_metadata).unwrap()
        );
    }

    #[test]
    fn a_field_record_with_a_value_that_cannot_be_rooted_is_listed() {
        // `other_meta` describes no calculation, so its subject has no root;
        // `CalculationField`'s `entry` keeps its type, and so does
        // `household_meta`'s value. The `@ field` pair is still rewritten.
        let source = format!(
            "{PROVIDED}= other_meta = CalculationMeta(fields = [\n    CalculationField(path = pathof(Spouse::income), label = \"Income\", question = None, help = None, unit = None, entry = Some(\"spouse_tax\"), subject = Some(\"spouse\"))\n])\n"
        );
        let migrated = migrate_needs(&source, Path::new("model.runa"), &needs_program(&source));
        assert_eq!(
            migrated.listed,
            vec![
                "13: left `entry = Some(\"spouse_tax\")`, `subject = Some(\"spouse\")` on `household_meta`: another `CalculationField` value of this file cannot be rewritten (line 23), and `CalculationField`'s `entry` has one type; write `entry = Some(refof(<calculation>))`, `subject = Some(pathof(<Input>::...))` by hand",
                "23: left `entry = Some(\"spouse_tax\")`, `subject = Some(\"spouse\")` on `other_meta`: no calculation's input reaches `other_meta`, so its subject has no root type; write `entry = Some(refof(<calculation>))`, `subject = Some(pathof(<Input>::...))` by hand",
            ]
        );
        assert_eq!(migrated.rewritten.len(), 1, "{:?}", migrated.rewritten);
        assert!(migrated.source.contains("entry: String?, subject"));
        assert!(migrated.source.contains("entry = refof(spouse_income)"));
    }

    #[test]
    fn a_field_meta_repeating_its_scalars_is_removed() {
        let types = "@ field(label = \"ISIN\", help = \"The code.\")\n# Isin: String \"an ISIN\"\n";
        let source = "# Holding(\n    @ field(label = \"ISIN\", help = \"The code.\")\n    isin: Isin,\n    @ field(key = True, label = \"ISIN\", help = \"The code.\") id: Isin,\n    @ field(label = \"Old ISIN\") old: Isin?,\n    @ field(help = \"The code.\", label = \"ISIN\")\n    others: List(Isin),\n    count: Int\n)\n";
        let mut program = MigrationProgram::default();
        program.add_source(Path::new("types.runa"), types);
        program.add_source(Path::new("model.runa"), source);
        let (migrated, removed) = program
            .remove_repeated_scalar_field_meta(Path::new("model.runa"), source)
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            migrated,
            "# Holding(\n    isin: Isin,\n    @ field(key = True) id: Isin,\n    @ field(label = \"Old ISIN\") old: Isin?,\n    others: List(Isin),\n    count: Int\n)\n"
        );
        assert!(program
            .remove_repeated_scalar_field_meta(Path::new("model.runa"), &migrated)
            .is_none());
    }
}
