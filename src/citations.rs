//! Paragraph citations on rule clauses.
//!
//! A law section is a `--@label:<section>::meta:<binding>--` anchor with a
//! `----` quote block, and the rules between `--@begin:<section>--` and
//! `--@end:<section>--` implement it. A clause names the paragraphs it
//! implements with a citation comment on the lines above it:
//!
//! ```text
//! --@cite:§ 9, stk. 3--
//! | excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> ...
//! ```
//!
//! Several paragraphs are separated by `;`, and a paragraph of a quote that
//! holds the same number from two documents names its document with
//! `in <source>` (`§ 1 in sources/bek-19-2024.txt`). The checker accepts a
//! citation only when every paragraph it names appears in the quote of the
//! section around the clause, read the way the quote marks paragraphs: a line
//! starting `§ N.` opens a paragraph, `Stk. N.` a subsection of it and `N)` a
//! numbered item, and a `[sources/...]` line starts the next quoted document.
//! A citation on the first clause of a rule covers the rule's other clauses
//! unless they carry their own; explanations (`runa call --explain`) show the
//! cited paragraphs' text.

use super::*;

/// One paragraph reference of a citation: `§ 9, stk. 3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitationRef {
    section: String,
    subsection: Option<u32>,
    item: Option<u32>,
    source: Option<String>,
}

impl CitationRef {
    pub(crate) fn label(&self) -> String {
        let mut label = format!("§ {}", self.section);
        if let Some(subsection) = self.subsection {
            label.push_str(&format!(", stk. {subsection}"));
        }
        if let Some(item) = self.item {
            label.push_str(&format!(", nr. {item}"));
        }
        label
    }
}

/// A cited paragraph and its quoted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitedParagraph {
    /// `§ 9, stk. 3`.
    pub(crate) paragraph: String,
    /// The `[sources/...]` document of the quote it was found in.
    pub(crate) source: Option<String>,
    /// The quoted lines of the paragraph, verbatim.
    pub(crate) text: String,
}

/// Parse the text of a `--@cite:...--` comment.
pub(crate) fn parse_citation(text: &str) -> Result<Vec<CitationRef>, String> {
    let mut references = Vec::new();
    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!(
                "citation `{}` has an empty paragraph; separate paragraphs with `;`, as in `§ 9, stk. 3; § 20`",
                text.trim()
            ));
        }
        references.push(parse_citation_ref(part)?);
    }
    Ok(references)
}

fn parse_citation_ref(text: &str) -> Result<CitationRef, String> {
    let malformed = || {
        format!(
            "citation `{text}` is not a paragraph reference; write `§ N`, `§ N, stk. M` or `§ N, stk. M, nr. K`, optionally followed by `in <source>`"
        )
    };
    let (reference, source) = match text.split_once(" in ") {
        Some((reference, source)) if !source.trim().is_empty() => {
            (reference.trim(), Some(source.trim().to_string()))
        }
        Some(_) => return Err(malformed()),
        None => (text, None),
    };
    let rest = reference.strip_prefix('§').ok_or_else(malformed)?;
    let mut parts = rest.split(',').map(str::trim);
    let section =
        normalize_section_number(parts.next().ok_or_else(malformed)?).ok_or_else(malformed)?;
    let mut subsection = None;
    let mut item = None;
    for part in parts {
        let lower = part.to_lowercase();
        if let Some(number) = lower.strip_prefix("stk.") {
            if subsection.is_some() || item.is_some() {
                return Err(malformed());
            }
            subsection = Some(number.trim().parse::<u32>().map_err(|_| malformed())?);
        } else if let Some(number) = lower.strip_prefix("nr.") {
            if item.is_some() {
                return Err(malformed());
            }
            item = Some(number.trim().parse::<u32>().map_err(|_| malformed())?);
        } else {
            return Err(malformed());
        }
    }
    Ok(CitationRef {
        section,
        subsection,
        item,
        source,
    })
}

/// `9`, `16 A` or `8a` as written after `§`, normalized to `9`, `16 A`, `8 A`.
fn normalize_section_number(text: &str) -> Option<String> {
    let text = text.trim();
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let suffix = text[digits.len()..].trim();
    match suffix.chars().count() {
        0 => Some(digits),
        1 if suffix.chars().all(|c| c.is_ascii_alphabetic()) => {
            Some(format!("{digits} {}", suffix.to_uppercase()))
        }
        _ => None,
    }
}

/// The paragraph markers of a quote: documents, paragraphs, subsections and
/// numbered items, each with the lines it spans.
#[derive(Debug, Default)]
pub(crate) struct Quote {
    lines: Vec<String>,
    documents: Vec<QuotedDocument>,
}

#[derive(Debug)]
struct QuotedDocument {
    source: Option<String>,
    paragraphs: Vec<QuotedParagraph>,
}

#[derive(Debug)]
struct QuotedParagraph {
    number: String,
    lines: std::ops::Range<usize>,
    subsections: Vec<QuotedPart>,
}

#[derive(Debug)]
struct QuotedPart {
    number: u32,
    lines: std::ops::Range<usize>,
    items: Vec<QuotedItem>,
}

#[derive(Debug)]
struct QuotedItem {
    number: u32,
    lines: std::ops::Range<usize>,
}

fn source_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    (trimmed.len() > 2 && trimmed.starts_with('[') && trimmed.ends_with(']'))
        .then(|| trimmed[1..trimmed.len() - 1].trim().to_string())
}

/// `§ 9.`, `§ 16 A.` at the start of a line.
fn paragraph_marker(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('§')?.trim_start();
    let end = rest.find('.')?;
    let after = &rest[end + 1..];
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    normalize_section_number(&rest[..end])
}

/// `Stk. 2.` at the start of a line.
fn subsection_marker(line: &str) -> Option<u32> {
    let rest = line.trim_start().strip_prefix("Stk.")?.trim_start();
    let end = rest.find('.')?;
    let after = &rest[end + 1..];
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    rest[..end].parse().ok()
}

/// `3)` at the start of a line.
fn item_marker(line: &str) -> Option<u32> {
    let rest = line.trim_start();
    let end = rest.find(')')?;
    let after = &rest[end + 1..];
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    let digits = &rest[..end];
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

impl Quote {
    pub(crate) fn parse(text: &str) -> Self {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let mut documents: Vec<QuotedDocument> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if let Some(source) = source_header(line) {
                documents.push(QuotedDocument {
                    source: Some(source),
                    paragraphs: Vec::new(),
                });
                continue;
            }
            if documents.is_empty() {
                documents.push(QuotedDocument {
                    source: None,
                    paragraphs: Vec::new(),
                });
            }
            let document = documents.last_mut().expect("a document is open");
            if let Some(number) = paragraph_marker(line) {
                document.paragraphs.push(QuotedParagraph {
                    number,
                    lines: index..index + 1,
                    subsections: vec![QuotedPart {
                        number: 1,
                        lines: index..index + 1,
                        items: Vec::new(),
                    }],
                });
                continue;
            }
            let Some(paragraph) = document.paragraphs.last_mut() else {
                continue;
            };
            paragraph.lines.end = index + 1;
            if let Some(number) = subsection_marker(line) {
                paragraph.subsections.push(QuotedPart {
                    number,
                    lines: index..index + 1,
                    items: Vec::new(),
                });
                continue;
            }
            let part = paragraph
                .subsections
                .last_mut()
                .expect("a paragraph has its first subsection");
            part.lines.end = index + 1;
            if let Some(number) = item_marker(line) {
                part.items.push(QuotedItem {
                    number,
                    lines: index..index + 1,
                });
            } else if let Some(item) = part.items.last_mut() {
                item.lines.end = index + 1;
            }
        }
        Quote { lines, documents }
    }

    fn text(&self, lines: &std::ops::Range<usize>) -> String {
        let mut text: Vec<&str> = self.lines[lines.clone()]
            .iter()
            .map(String::as_str)
            .collect();
        while text.last().is_some_and(|line| line.trim().is_empty()) {
            text.pop();
        }
        text.join("\n")
    }

    /// The paragraph numbers the quote holds, for a diagnostic.
    fn contents(&self) -> String {
        let mut labels = Vec::new();
        for document in &self.documents {
            for paragraph in &document.paragraphs {
                let subsections = paragraph
                    .subsections
                    .iter()
                    .map(|part| part.number.to_string())
                    .collect::<Vec<_>>();
                if subsections.len() > 1 {
                    labels.push(format!(
                        "§ {}, stk. {}",
                        paragraph.number,
                        subsections.join(", ")
                    ));
                } else {
                    labels.push(format!("§ {}", paragraph.number));
                }
            }
        }
        if labels.is_empty() {
            "no `§ N.` paragraph".to_string()
        } else {
            labels.join("; ")
        }
    }

    /// The quoted text of `reference`.
    pub(crate) fn resolve(&self, reference: &CitationRef) -> Result<CitedParagraph, String> {
        let mut found = Vec::new();
        for document in &self.documents {
            if let Some(source) = &reference.source {
                let matches = document
                    .source
                    .as_deref()
                    .is_some_and(|header| header == source || header.ends_with(source.as_str()));
                if !matches {
                    continue;
                }
            }
            for paragraph in &document.paragraphs {
                if paragraph.number != reference.section {
                    continue;
                }
                let lines = match (reference.subsection, reference.item) {
                    (None, None) => Some(paragraph.lines.clone()),
                    (subsection, item) => {
                        let part = paragraph
                            .subsections
                            .iter()
                            .find(|part| part.number == subsection.unwrap_or(1));
                        match (part, item) {
                            (Some(part), None) => Some(part.lines.clone()),
                            (Some(part), Some(item)) => part
                                .items
                                .iter()
                                .find(|candidate| candidate.number == item)
                                .map(|candidate| candidate.lines.clone()),
                            (None, _) => None,
                        }
                    }
                };
                if let Some(lines) = lines {
                    found.push((document.source.clone(), lines));
                }
            }
        }
        match found.len() {
            1 => {
                let (source, lines) = found.remove(0);
                Ok(CitedParagraph {
                    paragraph: reference.label(),
                    source,
                    text: self.text(&lines),
                })
            }
            0 => Err(format!(
                "{} does not appear in the quote of this section{}; the quote holds {}",
                reference.label(),
                reference
                    .source
                    .as_ref()
                    .map(|source| format!(" from `{source}`"))
                    .unwrap_or_default(),
                self.contents()
            )),
            _ => Err(format!(
                "{} appears in more than one quoted document of this section; name the document with `in <source>`, as in `{} in {}`",
                reference.label(),
                reference.label(),
                found
                    .iter()
                    .find_map(|(source, _)| source.clone())
                    .unwrap_or_else(|| "sources/...".to_string())
            )),
        }
    }
}

/// One `--@cite:...--` comment and the clause it cites for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClauseCitation {
    /// The line of the citation comment.
    pub(crate) comment_line: usize,
    /// The line of the rule clause it precedes.
    pub(crate) clause_line: usize,
    /// The section around the clause.
    pub(crate) section: String,
    pub(crate) paragraphs: Vec<CitedParagraph>,
}

/// The citations of one source file and the errors among them, by line.
#[derive(Debug, Default)]
pub(crate) struct FileCitations {
    pub(crate) by_clause_line: BTreeMap<usize, ClauseCitation>,
    pub(crate) errors: Vec<(usize, String)>,
}

/// Read and check every citation comment of `source`.
pub(crate) fn file_citations(source: &str) -> FileCitations {
    let mut result = FileCitations::default();
    if !source.contains("--@cite") {
        return result;
    }
    let index = scan_meta_comment_structure_with_rule_symbols(source, None);
    let lines: Vec<&str> = source.lines().collect();
    for comment in index
        .comments
        .iter()
        .filter(|comment| comment.kind == "cite")
    {
        let line = comment.line;
        let mut fail = |message: String| result.errors.push((line, message));
        if !comment.attrs.is_empty() {
            fail(format!(
                "citation `{}` has `::` attributes; separate cited paragraphs with `;`",
                comment.raw
            ));
            continue;
        }
        let references = match parse_citation(&comment.label) {
            Ok(references) => references,
            Err(message) => {
                fail(message);
                continue;
            }
        };
        let Some(clause_line) = cited_clause_line(&lines, line) else {
            fail(format!(
                "citation `{}` must stand directly above a rule clause (`| ...`) or function (`> ...`), with only comments between",
                comment.label
            ));
            continue;
        };
        let Some(span) = index
            .spans
            .iter()
            .filter(|span| span.code_start_line <= line && line <= span.code_end_line)
            .max_by_key(|span| span.code_start_line)
        else {
            fail(format!(
                "citation `{}` is outside a law section; cite inside `--@begin:<section>--` and `--@end:<section>--`",
                comment.label
            ));
            continue;
        };
        let Some(anchor) = index
            .anchors
            .iter()
            .find(|anchor| anchor.label == span.label && anchor.text_start_line.is_some())
        else {
            fail(format!(
                "citation `{}` is in section `{}`, which has no `----` quote under its `--@label:{}` anchor",
                comment.label, span.label, span.label
            ));
            continue;
        };
        let quote = Quote::parse(&anchor.text);
        let mut paragraphs = Vec::new();
        let mut failed = false;
        for reference in &references {
            match quote.resolve(reference) {
                Ok(paragraph) => paragraphs.push(paragraph),
                Err(message) => {
                    failed = true;
                    fail(format!(
                        "citation `{}`: {} (section `{}`)",
                        comment.label, message, span.label
                    ));
                }
            }
        }
        if failed {
            continue;
        }
        if result.by_clause_line.contains_key(&clause_line) {
            fail(format!(
                "the clause on line {clause_line} has more than one citation; cite several paragraphs in one comment, separated by `;`"
            ));
            continue;
        }
        result.by_clause_line.insert(
            clause_line,
            ClauseCitation {
                comment_line: line,
                clause_line,
                section: span.label.clone(),
                paragraphs,
            },
        );
    }
    result
}

/// The line of the rule clause or function below a citation comment on `comment_line`,
/// across blank lines and comments.
fn cited_clause_line(lines: &[&str], comment_line: usize) -> Option<usize> {
    for (index, line) in lines.iter().enumerate().skip(comment_line) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        return (trimmed.starts_with('|') || trimmed.starts_with('>')).then_some(index + 1);
    }
    None
}

/// The checker's diagnostics for the citations of `source`.
pub(crate) fn citation_diagnostics(source: &str) -> Vec<Diagnostic> {
    if !source.contains("--@cite") {
        return Vec::new();
    }
    let line_starts = source_line_char_offsets(source);
    file_citations(source)
        .errors
        .into_iter()
        .map(|(line, message)| {
            let start = line_starts.get(line - 1).copied().unwrap_or(0);
            let end = line_starts
                .get(line)
                .map_or(start, |next| next.saturating_sub(1));
            Diagnostic::error_at(Span::new(start, end.max(start + 1)), message)
        })
        .collect()
}

/// Character offset of the start of every line.
fn source_line_char_offsets(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, character) in source.chars().enumerate() {
        if character == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    const CEILING: &str = "--@label:ask_ceiling::meta:ask_ceiling_meta--
----
[sources/01-aktiesparekontoloven.txt]

§ 9. Personer, der er fuldt skattepligtige, kan foretage indskud,
jf. dog stk. 2.

Værdien opgøres pr. 31. december.

Stk. 2. Der kan uanset bestemmelsen i stk. 1 foretages indskud.

Stk. 3. Foretages indskud i strid med stk. 1, skal det overskydende beløb
udloddes. 1. og 2. pkt. gælder dog ikke for indskud som nævnt i § 13,
stk. 2, 3. pkt.

[sources/08-DJV.md]

Endvidere skal der betales en afgift på 3 pct. p.a.
----

--@begin:ask_ceiling--
-- The levy.
--@cite:§ 9, stk. 3--
-- A comment between.
| excess_levy_ore(days: Int) -> days * 3
--@cite:§ 9, stk. 4--
| other(days: Int) -> days
--@end:ask_ceiling--
";

    #[test]
    fn a_citation_resolves_to_the_quoted_paragraph() {
        let citations = file_citations(CEILING);
        let cited = &citations.by_clause_line[&25];
        assert_eq!(cited.section, "ask_ceiling");
        assert_eq!(cited.paragraphs[0].paragraph, "§ 9, stk. 3");
        assert_eq!(
            cited.paragraphs[0].source.as_deref(),
            Some("sources/01-aktiesparekontoloven.txt")
        );
        assert_eq!(
            cited.paragraphs[0].text,
            "Stk. 3. Foretages indskud i strid med stk. 1, skal det overskydende beløb\nudloddes. 1. og 2. pkt. gælder dog ikke for indskud som nævnt i § 13,\nstk. 2, 3. pkt."
        );
    }

    #[test]
    fn a_paragraph_missing_from_the_quote_is_rejected() {
        let citations = file_citations(CEILING);
        assert_eq!(citations.errors.len(), 1, "{:?}", citations.errors);
        let (line, message) = &citations.errors[0];
        assert_eq!(*line, 26);
        assert!(
            message.contains("§ 9, stk. 4 does not appear in the quote")
                && message.contains("§ 9, stk. 1, 2, 3"),
            "{message}"
        );
        let diagnostics = citation_diagnostics(CEILING);
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn whole_paragraphs_items_and_documents() {
        let quote = Quote::parse(
            "[sources/a.txt]\n\n§ 11. Første.\n\nStk. 2. Herunder:\n1) Værdien.\n2) Negativ\nskat.\n3) Skat.\n\n[sources/b.txt]\n\n§ 1. Begæres.\n\n§ 16 A. Udbytte.",
        );
        let item = parse_citation("§ 11, stk. 2, nr. 2").unwrap();
        assert_eq!(quote.resolve(&item[0]).unwrap().text, "2) Negativ\nskat.");
        let whole = parse_citation("§ 11").unwrap();
        assert_eq!(
            quote.resolve(&whole[0]).unwrap().text,
            "§ 11. Første.\n\nStk. 2. Herunder:\n1) Værdien.\n2) Negativ\nskat.\n3) Skat."
        );
        let other = parse_citation("§ 1 in sources/b.txt; §16a").unwrap();
        assert_eq!(quote.resolve(&other[0]).unwrap().text, "§ 1. Begæres.");
        assert_eq!(other[1].label(), "§ 16 A");
        assert_eq!(quote.resolve(&other[1]).unwrap().text, "§ 16 A. Udbytte.");
        assert!(quote
            .resolve(&parse_citation("§ 1 in sources/a.txt").unwrap()[0])
            .is_err());
    }

    #[test]
    fn a_function_in_a_section_can_be_cited() {
        let source = CEILING.replace(
            "--@cite:§ 9, stk. 4--\n| other(days: Int) -> days",
            "--@cite:§ 9, stk. 2--\n> other(days: Int) -> Int {\n    days\n}",
        );
        let citations = file_citations(&source);
        assert!(citations.errors.is_empty(), "{:?}", citations.errors);
        assert_eq!(
            citations.by_clause_line[&27].paragraphs[0].paragraph,
            "§ 9, stk. 2"
        );
    }

    #[test]
    fn malformed_and_misplaced_citations_are_rejected() {
        assert!(parse_citation("stk. 3").is_err());
        assert!(parse_citation("§ 9;").is_err());
        assert!(parse_citation("§ 9, pkt. 2").is_err());
        let outside = "--@cite:§ 9--\n| f(x: Int) -> x\n";
        assert!(file_citations(outside).errors[0]
            .1
            .contains("outside a law section"));
        let no_clause = CEILING.replace(
            "-- A comment between.\n| excess",
            "-- A comment between.\n= x = 1\n| excess",
        );
        assert!(file_citations(&no_clause)
            .errors
            .iter()
            .any(|(_, message)| message.contains("must stand directly above a rule clause")));
    }
}
