//! Paragraph citations on rule clauses, members and functions.
//!
//! A law section is `@ section <name> { ... }`; its `@ quote(<document>)`
//! blocks hold the text it implements. A clause names the paragraphs it
//! implements with `@ cite(...)` on the line above it:
//!
//! ```text
//! @ cite(§ 9, stk. 3)
//! | excess_levy_ore(excess_days_ore: Int, levy_bp: Int) -> ...
//! ```
//!
//! Several paragraphs are separated by `;`. Each is `§ N` with optional
//! `, stk. N`, `, nr. N` and `, N. pkt.` (a sentence); a paragraph the
//! section quotes from two documents names its document with
//! `in <binding>` (`§ 1 in bek_19_2024`). The checker accepts a citation
//! only when every paragraph it names appears in the section's quotes, read
//! the way the quote marks them: a line starting `§ N.` opens a paragraph,
//! `Stk. N.` a subsection of it and `N)` a numbered item. A citation may not
//! name a paragraph and a part of it.
//!
//! A passage reworded by an amending act the section quotes is cited
//! `as amended`: `§ 8 a, stk. 1, 1. pkt. as amended`. In a quote of an
//! amending act, a line `§ N I <act>, jf. lovbekendtgørelse nr. ..., ...
//! foretages følgende ændring(er):` opens amending paragraph `§ N`, and a
//! line `N. <passage>, affattes således:` (or `ændres »x« til: »y«`, or
//! `ophæves`) inside it is instruction `nr. N`. A plain citation of a
//! passage an instruction quoted in the section amends is an error.
//!
//! A citation on the first clause of a rule covers the rule's other clauses
//! unless they carry their own; explanations (`runa call --explain`) show
//! the cited paragraphs' text.

use super::*;
use crate::annotations::{Annotations, Documents};

/// The amending instruction a citation names: `as amended by
/// <document> § N, nr. M`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AmendedBy {
    document: String,
    paragraph: String,
    instruction: Option<u32>,
}

/// One paragraph reference of a citation: `§ 9, stk. 3`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitationRef {
    section: String,
    subsection: Option<u32>,
    item: Option<u32>,
    sentence: Option<u32>,
    /// The document binding after `in`.
    source: Option<String>,
    /// `as amended`, with the instruction named after `by`.
    amended: Option<Option<AmendedBy>>,
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
        if let Some(sentence) = self.sentence {
            label.push_str(&format!(", {sentence}. pkt."));
        }
        label
    }

    /// The passage, without document or amendment; a paragraph's letter
    /// compares without case (`§ 8 a` is `§ 8 A`).
    fn passage(&self) -> (String, Option<u32>, Option<u32>, Option<u32>) {
        (
            self.section.to_lowercase(),
            self.subsection,
            self.item,
            self.sentence,
        )
    }

    /// Whether this passage contains `other` and is not the same passage.
    fn contains(&self, other: &CitationRef) -> bool {
        if !same_number(&self.section, &other.section) || self.passage() == other.passage() {
            return false;
        }
        let path = |reference: &CitationRef| {
            [
                reference.subsection,
                reference.item,
                reference.sentence,
            ]
        };
        let (mine, theirs) = (path(self), path(other));
        // A paragraph's first subsection is its unnumbered opening.
        let mine_subsection = mine[0];
        let theirs_subsection = theirs[0];
        match mine_subsection {
            None => true,
            Some(subsection) => {
                theirs_subsection.unwrap_or(1) == subsection
                    && mine[1..]
                        .iter()
                        .zip(&theirs[1..])
                        .all(|(mine, theirs)| mine.is_none() || mine == theirs)
            }
        }
    }
}

/// The instruction that amended a cited passage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitedAmendment {
    /// The amending document's binding.
    pub(crate) document: String,
    /// Its saved file.
    pub(crate) file: Option<String>,
    /// Its `identifier`, when the document has one.
    pub(crate) identifier: Option<String>,
    /// `§ 4, nr. 1`.
    pub(crate) paragraph: String,
    /// The instruction line: `1. § 8 a, stk. 1, 1. pkt., affattes således:`.
    pub(crate) instruction: String,
}

/// A cited paragraph and its quoted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitedParagraph {
    /// `§ 9, stk. 3`.
    pub(crate) paragraph: String,
    /// The file of the quoted document it was found in.
    pub(crate) source: Option<String>,
    /// The quoted lines of the paragraph, verbatim; for a passage cited as
    /// amended, the wording in force.
    pub(crate) text: String,
    /// The instruction that gives the wording, for `as amended`.
    pub(crate) as_amended_by: Option<CitedAmendment>,
}

/// Whether `whole` names a paragraph that contains `part` (and is not the
/// same passage), in the same document.
pub(crate) fn contains_passage(whole: &CitationRef, part: &CitationRef) -> bool {
    whole.source == part.source && whole.contains(part)
}

/// Parse the text of a `@ cite(...)` annotation.
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
            "citation `{text}` is not a paragraph reference; write `§ N` with optional `, stk. M`, `, nr. K` and `, P. pkt.`, followed by `in <document>` or `as amended`"
        )
    };
    let (reference, amended) = match text.split_once(" as amended") {
        Some((reference, rest)) => {
            let rest = rest.trim();
            let by = if rest.is_empty() {
                None
            } else {
                let rest = rest.strip_prefix("by ").ok_or_else(malformed)?.trim();
                let (document, paragraph) = rest.split_once(' ').ok_or_else(malformed)?;
                let paragraph = parse_passage(paragraph.trim()).ok_or_else(malformed)?;
                if paragraph.subsection.is_some() || paragraph.sentence.is_some() {
                    return Err(malformed());
                }
                Some(AmendedBy {
                    document: document.to_string(),
                    paragraph: paragraph.section,
                    instruction: paragraph.item,
                })
            };
            (reference.trim(), Some(by))
        }
        None => (text, None),
    };
    let (reference, source) = match reference.split_once(" in ") {
        Some(_) if amended.is_some() => return Err(malformed()),
        Some((reference, source)) => {
            let source = source.trim();
            if source.contains('/') || source.contains('.') {
                return Err(format!(
                    "citation `{text}` names its document by a path; name the document's binding, as in `§ 1 in bek_19_2024`"
                ));
            }
            if source.is_empty() || source.contains(char::is_whitespace) {
                return Err(malformed());
            }
            (reference.trim(), Some(source.to_string()))
        }
        None => (reference, None),
    };
    let mut parsed = parse_passage(reference).ok_or_else(malformed)?;
    parsed.source = source;
    parsed.amended = amended;
    Ok(parsed)
}

/// `§ 8 a, stk. 1, nr. 2, 1. pkt.`
fn parse_passage(text: &str) -> Option<CitationRef> {
    let rest = text.trim().strip_prefix('§')?;
    let mut parts = rest.split(',').map(str::trim);
    let section = normalize_section_number(parts.next()?)?;
    let mut subsection = None;
    let mut item = None;
    let mut sentence = None;
    for part in parts {
        let lower = part.to_lowercase();
        if sentence.is_some() {
            return None;
        }
        if let Some(number) = lower.strip_prefix("stk.") {
            if subsection.is_some() || item.is_some() {
                return None;
            }
            subsection = Some(number.trim().parse::<u32>().ok()?);
        } else if let Some(number) = lower.strip_prefix("nr.") {
            if item.is_some() {
                return None;
            }
            item = Some(number.trim().parse::<u32>().ok()?);
        } else if let Some(number) = lower.strip_suffix("pkt.") {
            let number = number.trim().strip_suffix('.')?;
            sentence = Some(number.trim().parse::<u32>().ok()?);
        } else {
            return None;
        }
    }
    Some(CitationRef {
        section,
        subsection,
        item,
        sentence,
        source: None,
        amended: None,
    })
}

/// `9`, `16 A` or `8a` as written after `§`, normalized to `9`, `16 A`, `8 a`.
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
            Some(format!("{digits} {suffix}"))
        }
        _ => None,
    }
}

/// Whether two normalized paragraph numbers are the same: `8 a` is `8 A`.
fn same_number(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// One quoted document's text in a section: its binding, file and
/// identifier, and the quoted lines.
pub(crate) struct QuotePart<'a> {
    pub(crate) document: Option<&'a str>,
    pub(crate) file: Option<String>,
    pub(crate) identifier: Option<String>,
    pub(crate) text: &'a str,
}

/// The paragraph markers of a section's quotes: documents, paragraphs,
/// subsections, numbered items and amending instructions, each with the
/// lines it spans.
#[derive(Debug, Default)]
pub(crate) struct Quote {
    lines: Vec<String>,
    documents: Vec<QuotedDocument>,
}

#[derive(Debug)]
struct QuotedDocument {
    binding: Option<String>,
    file: Option<String>,
    identifier: Option<String>,
    paragraphs: Vec<QuotedParagraph>,
    amendments: Vec<AmendingParagraph>,
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

/// `§ 4 I personskatteloven, jf. lovbekendtgørelse nr. 1284 af ...,
/// foretages følgende ændringer:`.
#[derive(Debug)]
struct AmendingParagraph {
    number: String,
    /// The number and year of the consolidated act it amends.
    base: Option<(u32, u32)>,
    instructions: Vec<Instruction>,
}

#[derive(Debug)]
struct Instruction {
    number: u32,
    line: String,
    target: CitationRef,
    change: Change,
}

#[derive(Debug)]
enum Change {
    /// `affattes således:` and the new text's lines.
    Reworded(std::ops::Range<usize>),
    /// `ændres »from« til: »to«`.
    Replaced { from: String, to: String },
    /// `ophæves`.
    Repealed,
}

impl Change {
    fn verb(&self) -> &'static str {
        match self {
            Change::Reworded(_) => "rewords",
            Change::Replaced { .. } => "changes",
            Change::Repealed => "repeals",
        }
    }
}

/// `§ 9.`, `§ 16 A.` at the start of a line, and the subsection a
/// `§ 9 Stk. 1.` line (an amending act's rendering) opens.
fn paragraph_marker(line: &str) -> Option<(String, Option<u32>)> {
    let rest = line.trim_start().strip_prefix('§')?.trim_start();
    if let Some((number, after)) = rest.split_once(" Stk.") {
        if let (Some(number), Some(subsection)) =
            (normalize_section_number(number), subsection_marker(&format!("Stk.{after}")))
        {
            return Some((number, Some(subsection)));
        }
    }
    let end = rest.find('.')?;
    let after = &rest[end + 1..];
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    normalize_section_number(&rest[..end]).map(|number| (number, None))
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

/// The number and year in `nr. 1284 af 14. juni 2021` or
/// `nr. 1284 af 14/06/2021`, after `marker`.
fn act_number_and_year(text: &str, marker: &str) -> Option<(u32, u32)> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let number: String = rest.trim_start().chars().take_while(char::is_ascii_digit).collect();
    let number = number.parse().ok()?;
    let date = rest.split_once(" af ")?.1;
    let date: String = date.chars().take_while(|c| *c != ',').collect();
    let year = date
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| part.len() == 4)
        .last()?
        .parse()
        .ok()?;
    Some((number, year))
}

/// A line opening an amending paragraph.
fn amending_marker(line: &str) -> Option<AmendingParagraph> {
    let trimmed = line.trim();
    if !trimmed.contains("foretages følgende ændring") {
        return None;
    }
    let rest = trimmed.strip_prefix('§')?.trim_start();
    let (number, _) = rest.split_once(" I ")?;
    Some(AmendingParagraph {
        number: normalize_section_number(number)?,
        base: act_number_and_year(trimmed, "jf. lovbekendtgørelse nr."),
        instructions: Vec::new(),
    })
}

/// A line `N. <passage>, <verb>` inside an amending paragraph.
fn instruction_marker(line: &str) -> Option<(u32, CitationRef, Option<Change>)> {
    let trimmed = line.trim();
    let (number, rest) = trimmed.split_once(". ")?;
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number = number.parse().ok()?;
    let rest = rest.strip_prefix("I ").unwrap_or(rest);
    if !rest.starts_with('§') {
        return None;
    }
    let passage_of = |text: &str| parse_passage(text.trim().trim_end_matches(',').trim());
    if let Some(passage) = rest.strip_suffix("affattes således:") {
        return Some((number, passage_of(passage)?, None));
    }
    if let Some(passage) = rest
        .strip_suffix("ophæves.")
        .or_else(|| rest.strip_suffix("ophæves"))
    {
        return Some((number, passage_of(passage)?, Some(Change::Repealed)));
    }
    let (passage, change) = rest.split_once(" ændres »")?;
    let (from, after) = change.split_once('«')?;
    let to = after.trim().strip_prefix("til:")?.trim();
    let to = to.strip_prefix('»')?;
    let to = to.trim_end_matches('.').strip_suffix('«')?;
    Some((
        number,
        passage_of(passage)?,
        Some(Change::Replaced {
            from: from.to_string(),
            to: to.to_string(),
        }),
    ))
}

const ABBREVIATIONS: &[&str] = &["kr", "pct", "stk", "nr", "pkt", "jf", "m.v", "f.eks", "bl.a"];

/// The sentences of a quoted subsection or item: its lines joined, the
/// `§ N.`, `Stk. N.` or `N)` marker removed, split after a full stop
/// followed by a space and a capital letter that does not end an
/// abbreviation.
fn sentences(text: &str) -> Vec<String> {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut body = joined.as_str();
    if let Some(rest) = body.strip_prefix('§') {
        if let Some((_, after)) = rest.split_once(". ") {
            body = after;
        }
    } else if subsection_marker(body).is_some() {
        if let Some((_, after)) = body["Stk.".len()..].trim_start().split_once(". ") {
            body = after;
        }
    } else if item_marker(body).is_some() {
        if let Some((_, after)) = body.split_once(") ") {
            body = after;
        }
    }
    let characters: Vec<(usize, char)> = body.char_indices().collect();
    let mut found = Vec::new();
    let mut start = 0;
    for (index, (offset, character)) in characters.iter().enumerate() {
        if *character != '.' {
            continue;
        }
        let (Some((_, ' ')), Some((_, next))) = (characters.get(index + 1), characters.get(index + 2))
        else {
            continue;
        };
        let word = body[start..*offset]
            .rsplit(' ')
            .next()
            .unwrap_or_default()
            .to_lowercase();
        // A sentence may open with an ordinal: `1. og 2. pkt. gælder ...`.
        let ordinal = next.is_ascii_digit() && !word.chars().all(|c| c.is_ascii_digit()) && {
            let rest: String = characters[index + 2..].iter().map(|(_, c)| *c).collect();
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            rest[digits..].starts_with(". ")
        };
        if !next.is_uppercase() && !ordinal {
            continue;
        }
        if ABBREVIATIONS.contains(&word.as_str()) {
            continue;
        }
        found.push(body[start..=*offset].trim().to_string());
        start = offset + 2;
    }
    if start < body.len() {
        found.push(body[start..].trim().to_string());
    }
    found
}

impl Quote {
    /// The quote of a section: its documents' texts in order. Consecutive
    /// parts of the same document continue it.
    pub(crate) fn from_parts(parts: &[QuotePart<'_>]) -> Self {
        let mut quote = Quote::default();
        let mut previous: Option<Option<&str>> = None;
        for part in parts {
            if previous != Some(part.document) {
                quote.documents.push(QuotedDocument {
                    binding: part.document.map(str::to_string),
                    file: part.file.clone(),
                    identifier: part.identifier.clone(),
                    paragraphs: Vec::new(),
                    amendments: Vec::new(),
                });
                previous = Some(part.document);
            }
            if !quote.lines.is_empty() {
                // A blank line between quotes ends a paragraph's text.
                quote.lines.push(String::new());
            }
            let start = quote.lines.len();
            quote.lines.extend(part.text.lines().map(str::to_string));
            quote.read_lines(start);
        }
        quote
    }

    fn read_lines(&mut self, start: usize) {
        let document = self.documents.last_mut().expect("a document is open");
        // The rewording after an `affattes således:` instruction.
        let mut rewording: Option<(usize, usize)> = None;
        let mut in_amendment = !document.amendments.is_empty()
            && document
                .paragraphs
                .last()
                .is_none_or(|paragraph| paragraph.lines.end <= start);
        let close_rewording =
            |document: &mut QuotedDocument, rewording: &mut Option<(usize, usize)>, end: usize| {
                if let Some((amendment, instruction)) = rewording.take() {
                    if let Change::Reworded(lines) =
                        &mut document.amendments[amendment].instructions[instruction].change
                    {
                        lines.end = end;
                    }
                }
            };
        for index in start..self.lines.len() {
            let line = self.lines[index].clone();
            if let Some(amendment) = amending_marker(&line) {
                close_rewording(document, &mut rewording, index);
                document.amendments.push(amendment);
                in_amendment = true;
                continue;
            }
            if let Some((number, subsection)) = paragraph_marker(&line) {
                close_rewording(document, &mut rewording, index);
                in_amendment = false;
                document.paragraphs.push(QuotedParagraph {
                    number,
                    lines: index..index + 1,
                    subsections: vec![QuotedPart {
                        number: subsection.unwrap_or(1),
                        lines: index..index + 1,
                        items: Vec::new(),
                    }],
                });
                continue;
            }
            if in_amendment {
                if let Some((number, target, change)) = instruction_marker(&line) {
                    close_rewording(document, &mut rewording, index);
                    let amendment = document.amendments.len() - 1;
                    let change = change.unwrap_or_else(|| {
                        rewording = Some((
                            amendment,
                            document.amendments[amendment].instructions.len(),
                        ));
                        Change::Reworded(index + 1..index + 1)
                    });
                    document.amendments[amendment].instructions.push(Instruction {
                        number,
                        line: line.trim().to_string(),
                        target,
                        change,
                    });
                }
                continue;
            }
            let Some(paragraph) = document.paragraphs.last_mut() else {
                continue;
            };
            paragraph.lines.end = index + 1;
            if let Some(number) = subsection_marker(&line) {
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
            if let Some(number) = item_marker(&line) {
                part.items.push(QuotedItem {
                    number,
                    lines: index..index + 1,
                });
            } else if let Some(item) = part.items.last_mut() {
                item.lines.end = index + 1;
            }
        }
        close_rewording(document, &mut rewording, self.lines.len());
    }

    fn text(&self, lines: &std::ops::Range<usize>) -> String {
        crate::annotations::trim_blank_lines(&self.lines[lines.clone()].join("\n"))
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

    /// The text of `reference`'s passage in `document`, without its
    /// sentence; `None` when the document does not quote it.
    fn passage_lines(
        document: &QuotedDocument,
        reference: &CitationRef,
    ) -> Option<std::ops::Range<usize>> {
        let mut found = None;
        for paragraph in &document.paragraphs {
            if !same_number(&paragraph.number, &reference.section) {
                continue;
            }
            let whole = reference.subsection.is_none()
                && reference.item.is_none()
                && reference.sentence.is_none();
            let lines = if whole {
                Some(paragraph.lines.clone())
            } else {
                let part = paragraph
                    .subsections
                    .iter()
                    .find(|part| part.number == reference.subsection.unwrap_or(1));
                match (part, reference.item) {
                    (Some(part), None) => Some(part.lines.clone()),
                    (Some(part), Some(item)) => part
                        .items
                        .iter()
                        .find(|candidate| candidate.number == item)
                        .map(|candidate| candidate.lines.clone()),
                    (None, _) => None,
                }
            };
            if lines.is_some() {
                found = lines;
            }
        }
        found
    }

    /// The passage text of `reference` in `document`, sentence included.
    fn passage_text(
        &self,
        document: &QuotedDocument,
        reference: &CitationRef,
    ) -> Option<Result<String, String>> {
        let lines = Self::passage_lines(document, reference)?;
        let text = self.text(&lines);
        let Some(sentence) = reference.sentence else {
            return Some(Ok(text));
        };
        let found = sentences(&text);
        Some(
            found
                .get(sentence as usize - 1)
                .cloned()
                .filter(|_| sentence > 0)
                .ok_or_else(|| {
                    format!(
                        "{} is not quoted: the passage has {} sentence(s): {}",
                        reference.label(),
                        found.len(),
                        found
                            .iter()
                            .enumerate()
                            .map(|(index, text)| format!("{}. pkt. »{text}«", index + 1))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                }),
        )
    }

    fn documents_named<'a>(
        &'a self,
        reference: &'a CitationRef,
    ) -> Result<Vec<&'a QuotedDocument>, String> {
        let documents: Vec<&QuotedDocument> = self
            .documents
            .iter()
            .filter(|document| {
                reference
                    .source
                    .as_ref()
                    .is_none_or(|source| document.binding.as_deref() == Some(source))
            })
            .collect();
        if documents.is_empty() {
            if let Some(source) = &reference.source {
                return Err(format!(
                    "{} names document `{source}`, which this section does not quote",
                    reference.label()
                ));
            }
        }
        Ok(documents)
    }

    /// The quoted text of `reference`.
    pub(crate) fn resolve(&self, reference: &CitationRef) -> Result<CitedParagraph, String> {
        if let Some(by) = &reference.amended {
            return self.resolve_amended(reference, by.as_ref());
        }
        let mut found = Vec::new();
        for document in self.documents_named(reference)? {
            if let Some(text) = self.passage_text(document, reference) {
                found.push((document, text?));
            }
        }
        match found.len() {
            1 => {
                let (document, text) = found.remove(0);
                if let Some((amendment, instruction, contained)) =
                    self.amendment_within(document, reference)
                {
                    let amending = self.amending_document(amendment);
                    let by = format!(
                        "{} § {}, nr. {}",
                        amending.identifier.as_deref().or(amending.binding.as_deref()).unwrap_or("the amending act"),
                        amendment.number,
                        instruction.number
                    );
                    return Err(if contained {
                        format!(
                            "contains {}, which {by} {} (quoted in this section); cite its parts",
                            instruction.target.label(),
                            instruction.change.verb()
                        )
                    } else {
                        format!(
                            "{by} (quoted in this section) {} this passage; cite `{} as amended`",
                            instruction.change.verb(),
                            reference.label()
                        )
                    });
                }
                Ok(CitedParagraph {
                    paragraph: reference.label(),
                    source: document.file.clone(),
                    text,
                    as_amended_by: None,
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
                "{} appears in more than one quoted document of this section; name the document with `in <document>`, as in `{} in {}`",
                reference.label(),
                reference.label(),
                found
                    .iter()
                    .find_map(|(document, _)| document.binding.clone())
                    .unwrap_or_else(|| "<document>".to_string())
            )),
        }
    }

    fn amending_document(&self, amendment: &AmendingParagraph) -> &QuotedDocument {
        self.documents
            .iter()
            .find(|document| {
                document
                    .amendments
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, amendment))
            })
            .expect("an amendment belongs to a document")
    }

    /// Whether an instruction quoted in the section amends `reference`'s
    /// passage of `document` (`false`) or a part of it (`true`).
    fn amendment_within(
        &self,
        document: &QuotedDocument,
        reference: &CitationRef,
    ) -> Option<(&AmendingParagraph, &Instruction, bool)> {
        let identifier = document
            .identifier
            .as_deref()
            .and_then(|identifier| act_number_and_year(identifier, "nr."));
        for amending in &self.documents {
            if std::ptr::eq(amending, document) {
                continue;
            }
            for amendment in &amending.amendments {
                if let (Some(base), Some(identifier)) = (amendment.base, identifier) {
                    if base != identifier {
                        continue;
                    }
                }
                for instruction in &amendment.instructions {
                    if instruction.target.passage() == reference.passage() {
                        return Some((amendment, instruction, false));
                    }
                    if reference.contains(&instruction.target) {
                        return Some((amendment, instruction, true));
                    }
                }
            }
        }
        None
    }

    fn resolve_amended(
        &self,
        reference: &CitationRef,
        by: Option<&AmendedBy>,
    ) -> Result<CitedParagraph, String> {
        let mut candidates = Vec::new();
        for document in &self.documents {
            if let Some(by) = by {
                if document.binding.as_deref() != Some(by.document.as_str()) {
                    continue;
                }
            }
            for amendment in &document.amendments {
                if by.is_some_and(|by| !same_number(&by.paragraph, &amendment.number)) {
                    continue;
                }
                for instruction in &amendment.instructions {
                    if by.is_some_and(|by| {
                        by.instruction.is_some_and(|number| number != instruction.number)
                    }) {
                        continue;
                    }
                    if instruction.target.passage() == reference.passage() {
                        candidates.push((document, amendment, instruction));
                    }
                }
            }
        }
        let label = reference.label();
        let (amending, amendment, instruction) = match candidates.len() {
            1 => candidates[0],
            0 => {
                return Err(format!(
                    "{label} as amended: no instruction quoted in this section amends {label}; quote the amending act's instruction (`N. {label}, affattes således:`) under its amending paragraph line (`§ N I <act>, jf. lovbekendtgørelse nr. ..., foretages følgende ændringer:`)"
                ))
            }
            _ => {
                return Err(format!(
                    "{label} is amended by more than one quoted instruction ({}); name the one you mean, as in `{label} as amended by {} § {}, nr. {}`",
                    candidates
                        .iter()
                        .map(|(document, amendment, instruction)| format!(
                            "{} § {}, nr. {}",
                            document.binding.as_deref().unwrap_or("?"),
                            amendment.number,
                            instruction.number
                        ))
                        .collect::<Vec<_>>()
                        .join(", "),
                    candidates[0].0.binding.as_deref().unwrap_or("<document>"),
                    candidates[0].1.number,
                    candidates[0].2.number
                ))
            }
        };
        let amending_name = amending
            .identifier
            .clone()
            .or_else(|| amending.binding.clone())
            .unwrap_or_else(|| "the amending act".to_string());
        // The base: the other quoted document holding the passage's paragraph.
        let base = self.documents.iter().find(|document| {
            !std::ptr::eq(*document, amending)
                && document
                    .paragraphs
                    .iter()
                    .any(|paragraph| same_number(&paragraph.number, &reference.section))
        });
        if let (Some(base), Some(amended_base)) = (base, amendment.base) {
            if let Some(identifier) = base
                .identifier
                .as_deref()
                .and_then(|identifier| act_number_and_year(identifier, "nr."))
            {
                if identifier != amended_base {
                    return Err(format!(
                        "{label} as amended: {amending_name} § {} amends lovbekendtgørelse nr. {} of {}, but § {} is quoted from `{}` ({})",
                        amendment.number,
                        amended_base.0,
                        amended_base.1,
                        reference.section,
                        base.binding.as_deref().unwrap_or("?"),
                        base.identifier.as_deref().unwrap_or_default()
                    ));
                }
            }
        }
        let text = match &instruction.change {
            Change::Repealed => {
                return Err(format!(
                    "{label} as amended: {amending_name} § {}, nr. {} repeals it; a repealed passage cannot be cited",
                    amendment.number, instruction.number
                ))
            }
            Change::Reworded(lines) => self.text(lines),
            Change::Replaced { from, to } => {
                let Some(base_text) = base.and_then(|base| {
                    self.passage_text(
                        base,
                        &CitationRef {
                            amended: None,
                            source: None,
                            ..reference.clone()
                        },
                    )
                }) else {
                    return Err(format!(
                        "{label} as amended: the instruction changes a passage of the base act; quote {label} from the act it amends in this section"
                    ));
                };
                let base_text = crate::annotations::normalise_quoted_text(&base_text?);
                let from = crate::annotations::normalise_quoted_text(from);
                if base_text.matches(&from).count() != 1 {
                    return Err(format!(
                        "{label} as amended: »{from}« must occur exactly once in the quoted passage, which reads »{base_text}«"
                    ));
                }
                base_text.replacen(&from, to, 1)
            }
        };
        Ok(CitedParagraph {
            paragraph: label,
            source: base.and_then(|base| base.file.clone()),
            text,
            as_amended_by: Some(CitedAmendment {
                document: amending.binding.clone().unwrap_or_default(),
                file: amending.file.clone(),
                identifier: amending.identifier.clone(),
                paragraph: format!("§ {}, nr. {}", amendment.number, instruction.number),
                instruction: instruction.line.clone(),
            }),
        })
    }

    /// The base passages an instruction quoted in the section amends, with
    /// the instruction: `(paragraph, amendment)`.
    pub(crate) fn superseded(&self) -> Vec<(String, CitedAmendment)> {
        let mut superseded = Vec::new();
        for amending in &self.documents {
            for amendment in &amending.amendments {
                for instruction in &amendment.instructions {
                    let quoted = self.documents.iter().any(|document| {
                        !std::ptr::eq(document, amending)
                            && Self::passage_lines(
                                document,
                                &CitationRef {
                                    sentence: None,
                                    ..instruction.target.clone()
                                },
                            )
                            .is_some()
                    });
                    if !quoted {
                        continue;
                    }
                    superseded.push((
                        instruction.target.label(),
                        CitedAmendment {
                            document: amending.binding.clone().unwrap_or_default(),
                            file: amending.file.clone(),
                            identifier: amending.identifier.clone(),
                            paragraph: format!(
                                "§ {}, nr. {}",
                                amendment.number, instruction.number
                            ),
                            instruction: instruction.line.clone(),
                        },
                    ));
                }
            }
        }
        superseded
    }
}

/// The quote of a section, its documents resolved by `documents`.
pub(crate) fn section_quote(
    section: &crate::annotations::Section,
    documents: &Documents,
) -> Quote {
    let parts: Vec<QuotePart<'_>> = section
        .quotes
        .iter()
        .map(|quote| QuotePart {
            document: Some(&quote.document),
            file: documents.file(&quote.document).ok(),
            identifier: documents.field(&quote.document, "identifier"),
            text: &quote.text,
        })
        .collect();
    Quote::from_parts(&parts)
}

/// One `@ cite(...)` and the clause it cites for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClauseCitation {
    /// The line of the citation.
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

/// Read and check every citation of a file's annotations.
pub(crate) fn file_citations_from(annotations: &Annotations, documents: &Documents) -> FileCitations {
    let mut result = FileCitations::default();
    let mut quotes: BTreeMap<&str, Quote> = BTreeMap::new();
    for citation in &annotations.citations {
        let line = citation.line;
        let mut fail = |message: String| result.errors.push((line, message));
        let references = match parse_citation(&citation.text) {
            Ok(references) => references,
            Err(message) => {
                fail(message);
                continue;
            }
        };
        let mut overlapping = false;
        for (index, reference) in references.iter().enumerate() {
            for other in &references[index + 1..] {
                if reference.source != other.source {
                    continue;
                }
                let (whole, part) = if reference.contains(other) {
                    (reference, other)
                } else if other.contains(reference) {
                    (other, reference)
                } else if reference.passage() == other.passage() {
                    fail(format!(
                        "citation `{}` names {} twice",
                        citation.text,
                        reference.label()
                    ));
                    overlapping = true;
                    continue;
                } else {
                    continue;
                };
                fail(format!(
                    "citation `{}`: `{}` already contains `{}`; cite `{}` or its parts",
                    citation.text,
                    whole.label(),
                    part.label(),
                    whole.label()
                ));
                overlapping = true;
            }
        }
        if overlapping {
            continue;
        }
        let Some(section_name) = &citation.section else {
            fail(format!(
                "citation `{}` is outside a law section; cite inside `@ section <name> {{ ... }}`",
                citation.text
            ));
            continue;
        };
        let Some(section) = annotations.section(section_name) else {
            continue;
        };
        if section.quotes.is_empty() {
            fail(format!(
                "citation `{}` is in section `{section_name}`, which quotes no document; add `@ quote(<document>)` with the passage",
                citation.text
            ));
            continue;
        }
        let quote = quotes
            .entry(section_name.as_str())
            .or_insert_with(|| section_quote(section, documents));
        let mut paragraphs = Vec::new();
        let mut failed = false;
        for reference in &references {
            match quote.resolve(reference) {
                Ok(paragraph) => paragraphs.push(paragraph),
                Err(message) => {
                    failed = true;
                    fail(format!(
                        "citation `{}`: {} (section `{section_name}`)",
                        reference.label(),
                        message
                    ));
                }
            }
        }
        if failed {
            continue;
        }
        result.by_clause_line.insert(
            citation.clause_line,
            ClauseCitation {
                comment_line: line,
                clause_line: citation.clause_line,
                section: section_name.clone(),
                paragraphs,
            },
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const CEILING: &str = "# SourceInfo(file: String, identifier: String)
# impl Document for SourceInfo {}
# TaxMeta(a) = Source(value: a) | Guidance(value: a)
= law = SourceInfo(file = \"sources/01-aktiesparekontoloven.txt\", identifier = \"Aktiesparekontoloven, LBK nr. 281 af 13/03/2025\")
= guide = SourceInfo(file = \"sources/08-DJV.md\", identifier = \"DJV\")

@ section ask_ceiling {

@ quote(Source(value = law))
----
§ 9. Personer, der er fuldt skattepligtige, kan foretage indskud,
jf. dog stk. 2.

Værdien opgøres pr. 31. december.

Stk. 2. Der kan uanset bestemmelsen i stk. 1 foretages indskud.

Stk. 3. Foretages indskud i strid med stk. 1, skal det overskydende beløb
udloddes. Endvidere skal der betales en afgift på 3 pct. p.a. af det
overskydende beløb. 1. og 2. pkt. gælder dog ikke for indskud som nævnt i § 13,
stk. 2, 3. pkt.
----

@ quote(Guidance(value = guide))
----
Endvidere skal der betales en afgift på 3 pct. p.a.
----

-- The levy.
@ cite(§ 9, stk. 3)
-- A comment between.
| excess_levy_ore(days: Int) -> days * 3
@ cite(§ 9, stk. 4)
| other(days: Int) -> days

}
";

    fn citations_of(source: &str) -> FileCitations {
        let stmts = Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .expect("the test model parses");
        let documents = Documents::collect(source, &stmts, None);
        file_citations_from(&crate::annotations::read(source), &documents)
    }

    #[test]
    fn a_citation_resolves_to_the_quoted_paragraph() {
        let citations = citations_of(CEILING);
        let cited = &citations.by_clause_line[&32];
        assert_eq!(cited.section, "ask_ceiling");
        assert_eq!(cited.paragraphs[0].paragraph, "§ 9, stk. 3");
        assert_eq!(
            cited.paragraphs[0].source.as_deref(),
            Some("sources/01-aktiesparekontoloven.txt")
        );
        assert!(cited.paragraphs[0].text.starts_with("Stk. 3. Foretages indskud"));
        assert!(cited.paragraphs[0].text.ends_with("stk. 2, 3. pkt."));
    }

    #[test]
    fn a_paragraph_missing_from_the_quote_is_rejected() {
        let citations = citations_of(CEILING);
        assert_eq!(citations.errors.len(), 1, "{:?}", citations.errors);
        let (line, message) = &citations.errors[0];
        assert_eq!(*line, 33);
        assert!(
            message.contains("§ 9, stk. 4 does not appear in the quote")
                && message.contains("§ 9, stk. 1, 2, 3"),
            "{message}"
        );
    }

    #[test]
    fn sentences_are_split_after_a_full_stop_and_a_capital() {
        let source = CEILING.replace("@ cite(§ 9, stk. 4)", "@ cite(§ 9, stk. 3, 2. pkt.)");
        let citations = citations_of(&source);
        assert!(citations.errors.is_empty(), "{:?}", citations.errors);
        assert_eq!(
            citations.by_clause_line[&34].paragraphs[0].text,
            "Endvidere skal der betales en afgift på 3 pct. p.a. af det overskydende beløb."
        );
        let missing = CEILING.replace("@ cite(§ 9, stk. 4)", "@ cite(§ 9, stk. 3, 4. pkt.)");
        let errors = citations_of(&missing).errors;
        assert!(errors[0].1.contains("has 3 sentence(s)"), "{errors:?}");
        assert!(errors[0].1.contains("1. pkt. »Foretages indskud"), "{errors:?}");
    }

    #[test]
    fn a_paragraph_and_a_part_of_it_are_not_cited_together() {
        let source = CEILING.replace("@ cite(§ 9, stk. 4)", "@ cite(§ 9; § 9, stk. 2)");
        let errors = citations_of(&source).errors;
        assert!(
            errors[0]
                .1
                .contains("`§ 9` already contains `§ 9, stk. 2`; cite `§ 9` or its parts"),
            "{errors:?}"
        );
        let siblings = CEILING.replace("@ cite(§ 9, stk. 4)", "@ cite(§ 9, stk. 1; § 9, stk. 2)");
        assert!(citations_of(&siblings).errors.is_empty());
    }

    #[test]
    fn whole_paragraphs_items_and_documents() {
        let a = "§ 11. Første.\n\nStk. 2. Herunder:\n1) Værdien.\n2) Negativ\nskat.\n3) Skat.";
        let b = "§ 1. Begæres.\n\n§ 16 A. Udbytte.";
        let quote = Quote::from_parts(&[
            QuotePart {
                document: Some("a"),
                file: Some("sources/a.txt".to_string()),
                identifier: None,
                text: a,
            },
            QuotePart {
                document: Some("b"),
                file: Some("sources/b.txt".to_string()),
                identifier: None,
                text: b,
            },
        ]);
        let item = parse_citation("§ 11, stk. 2, nr. 2").unwrap();
        assert_eq!(quote.resolve(&item[0]).unwrap().text, "2) Negativ\nskat.");
        let whole = parse_citation("§ 11").unwrap();
        assert_eq!(quote.resolve(&whole[0]).unwrap().text, a);
        let other = parse_citation("§ 1 in b; §16A").unwrap();
        assert_eq!(quote.resolve(&other[0]).unwrap().text, "§ 1. Begæres.");
        assert_eq!(other[1].label(), "§ 16 A");
        assert_eq!(quote.resolve(&other[1]).unwrap().text, "§ 16 A. Udbytte.");
        assert!(quote.resolve(&parse_citation("§ 1 in a").unwrap()[0]).is_err());
        assert!(parse_citation("§ 1 in sources/b.txt")
            .unwrap_err()
            .contains("name the document's binding"));
    }

    const AMENDED: &str = "§ 4 I personskatteloven, jf. lovbekendtgørelse nr. 1284 af 14. juni 2021, som ændret senest ved § 1 i lov nr. 482 af 22. maj 2024, foretages følgende ændringer:

1. § 8 a, stk. 1, 1. pkt., affattes således:

Skat af aktieindkomst, der ikke overstiger et grundbeløb på 51.500 kr.
(2010-niveau), beregnes som en endelig skat på 27 pct.

2. I § 8 a, stk. 2, 1. pkt., ændres »42 pct.« til: »43 pct.«

3. § 8 a, stk. 3, ophæves.";

    const BASE: &str = "§ 8 a. Skat af aktieindkomst, der ikke overstiger et grundbeløb på 48.300 kr.
(2010-niveau), beregnes som en endelig skat på 27 pct., jf. dog stk. 2. Andet punktum.

Stk. 2. Skat af aktieindkomst, der overstiger grundbeløbet, beregnes med 42 pct.

Stk. 3. Ophævet tekst.";

    fn amended_quote(base_identifier: &str) -> Quote {
        Quote::from_parts(&[
            QuotePart {
                document: Some("aendring"),
                file: Some("sources/04b.txt".to_string()),
                identifier: Some("LOV nr. 1691 af 30/12/2024".to_string()),
                text: AMENDED,
            },
            QuotePart {
                document: Some("psl"),
                file: Some("sources/04.txt".to_string()),
                identifier: Some(base_identifier.to_string()),
                text: BASE,
            },
        ])
    }

    #[test]
    fn a_passage_cited_as_amended_reads_the_instruction() {
        let quote = amended_quote("Personskatteloven, LBK nr. 1284 af 14/06/2021");
        let reworded = quote
            .resolve(&parse_citation("§ 8 a, stk. 1, 1. pkt. as amended").unwrap()[0])
            .unwrap();
        assert!(reworded.text.contains("51.500 kr."), "{}", reworded.text);
        assert_eq!(reworded.source.as_deref(), Some("sources/04.txt"));
        let by = reworded.as_amended_by.unwrap();
        assert_eq!(by.paragraph, "§ 4, nr. 1");
        assert_eq!(by.instruction, "1. § 8 a, stk. 1, 1. pkt., affattes således:");
        let replaced = quote
            .resolve(&parse_citation("§ 8 a, stk. 2, 1. pkt. as amended by aendring § 4, nr. 2").unwrap()[0])
            .unwrap();
        assert!(replaced.text.ends_with("beregnes med 43 pct."), "{}", replaced.text);
        let repealed = quote
            .resolve(&parse_citation("§ 8 a, stk. 3 as amended").unwrap()[0])
            .unwrap_err();
        assert!(repealed.contains("repeals it"), "{repealed}");
    }

    #[test]
    fn an_amended_passage_is_not_cited_plainly() {
        let quote = amended_quote("Personskatteloven, LBK nr. 1284 af 14/06/2021");
        let plain = quote
            .resolve(&parse_citation("§ 8 a, stk. 1, 1. pkt.").unwrap()[0])
            .unwrap_err();
        assert!(
            plain.contains("LOV nr. 1691 af 30/12/2024 § 4, nr. 1 (quoted in this section) rewords this passage; cite `§ 8 a, stk. 1, 1. pkt. as amended`"),
            "{plain}"
        );
        let container = quote
            .resolve(&parse_citation("§ 8 a, stk. 1").unwrap()[0])
            .unwrap_err();
        assert!(container.contains("contains § 8 a, stk. 1, 1. pkt."), "{container}");
        assert!(quote
            .resolve(&parse_citation("§ 8 a, stk. 1, 2. pkt.").unwrap()[0])
            .is_ok());
    }

    #[test]
    fn an_amended_citation_needs_its_instruction_and_base() {
        let quote = amended_quote("Personskatteloven, LBK nr. 1284 af 14/06/2021");
        let missing = quote
            .resolve(&parse_citation("§ 8 a, stk. 4, 1. pkt. as amended").unwrap()[0])
            .unwrap_err();
        assert!(missing.contains("no instruction quoted in this section amends"), "{missing}");
        let other_act = amended_quote("Kildeskatteloven, LBK nr. 460 af 01/04/2024");
        let mismatch = other_act
            .resolve(&parse_citation("§ 8 a, stk. 1, 1. pkt. as amended").unwrap()[0])
            .unwrap_err();
        assert!(mismatch.contains("amends lovbekendtgørelse nr. 1284 of 2021"), "{mismatch}");
        assert_eq!(quote.superseded().len(), 3);
    }

    #[test]
    fn malformed_and_misplaced_citations_are_rejected() {
        assert!(parse_citation("stk. 3").is_err());
        assert!(parse_citation("§ 9;").is_err());
        assert!(parse_citation("§ 9, pkt. 2").is_err());
        let outside = "@ cite(§ 9)\n| f(x: Int) -> x\n";
        assert!(citations_of(outside).errors[0]
            .1
            .contains("outside a law section"));
    }
}
