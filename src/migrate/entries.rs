//! The 0.6 rewrites of `runa fmt --migrate` (docs/reference/calculations.md,
//! "Answering an entry"): `@ calculate` becomes `@ entry`, and `interview`
//! becomes `answer`:
//!
//! - `interview(|c| query e(c), partial, complete)` becomes
//!   `answer(e, partial, from_case(complete))`;
//! - for a binding `x` of it, `x.rounds` becomes `map(x.rounds, |r| r.paths)`
//!   and `x.result == v` becomes `x.outcome == Answered(result = v)`.
//!
//! Any other use of `interview` or `Interview` is listed. The rewrites are
//! lexed, not parsed: a file that still says `@ calculate` does not parse in
//! 0.6, and the other migrations read the file after these.

use super::{is_word_char, source_line_starts};

/// A file after the 0.6 rewrites, with what was rewritten and what is left
/// for a person, each as `line: message`.
#[derive(Debug, Default)]
pub struct EntriesMigration {
    pub source: String,
    pub rewritten: Vec<String>,
    pub listed: Vec<String>,
}

/// The character offset of `token` in `chars`.
fn token_offset(chars: &[char], starts: &[usize], token: &crate::Token) -> Option<usize> {
    let bom = usize::from(chars.first() == Some(&'\u{feff}'));
    let start = token
        .line
        .checked_sub(1)
        .and_then(|line| starts.get(line))?;
    Some(start + token.col.saturating_sub(1) + if token.line == 1 { bom } else { 0 })
}

/// Whether `word` is spelled at `offset` of `chars` as a whole word.
fn spelled_at(chars: &[char], offset: usize, word: &str) -> bool {
    let length = word.chars().count();
    chars
        .get(offset..offset + length)
        .is_some_and(|found| found.iter().copied().eq(word.chars()))
        && (offset == 0 || !is_word_char(chars[offset - 1]))
        && !chars
            .get(offset + length)
            .is_some_and(|character| is_word_char(*character))
}

/// The offset just after the parenthesis that closes the one at `open`,
/// skipping strings.
fn closing_paren(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    let mut in_string = false;
    while let Some(character) = chars.get(index) {
        match character {
            '\\' if in_string => index += 1,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// The arguments between `start` and `end`, split at top-level commas.
fn arguments(chars: &[char], start: usize, end: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for character in &chars[start..end] {
        match character {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            ',' if !in_string && depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(*character);
    }
    parts.push(current.trim().to_string());
    parts
}

/// The entry `ask` queries when it is `|c| query e(c)`.
fn queried_entry(ask: &str) -> Option<&str> {
    let rest = ask.strip_prefix('|')?;
    let (parameter, rest) = rest.split_once('|')?;
    let parameter = parameter.trim();
    let rest = rest.trim().strip_prefix("query")?;
    let (entry, rest) = rest.trim_start().split_once('(')?;
    let argument = rest.trim().strip_suffix(')')?;
    let word = |text: &str| !text.is_empty() && text.chars().all(is_word_char);
    (word(parameter) && word(entry.trim()) && argument.trim() == parameter).then(|| entry.trim())
}

/// The end of the expression that starts at `start`: before a comma, closing
/// bracket, `&&`, `||` or comment at its own depth, or the end of the line.
fn expression_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut index = start;
    while let Some(character) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        match character {
            '\\' if in_string => index += 1,
            '"' => in_string = !in_string,
            _ if in_string => {}
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => break,
            ')' | ']' | '}' => depth -= 1,
            ',' | '\n' if depth == 0 => break,
            '&' if depth == 0 && next == Some('&') => break,
            '|' if depth == 0 && next == Some('|') => break,
            '-' if depth == 0 && next == Some('-') => break,
            _ => {}
        }
        index += 1;
    }
    while index > start && chars[index - 1].is_whitespace() {
        index -= 1;
    }
    index
}

/// `source` with the 0.6 rewrites.
pub fn migrate_entries(source: &str) -> EntriesMigration {
    if !source.contains("calculate") && !source.contains("nterview") {
        return EntriesMigration {
            source: source.to_string(),
            ..EntriesMigration::default()
        };
    }
    let tokens = crate::Lexer::new(source).tokenize();
    let chars: Vec<char> = source.chars().collect();
    let starts = source_line_starts(&chars);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut rewritten = Vec::new();
    let mut listed = Vec::new();
    for pair in tokens.windows(2) {
        let [at, name] = pair else { continue };
        if at.kind != crate::TokenKind::At || name.source_text != "calculate" {
            continue;
        }
        let Some(offset) = token_offset(&chars, &starts, name) else {
            continue;
        };
        if !spelled_at(&chars, offset, "calculate") {
            continue;
        }
        edits.push((offset, offset + "calculate".len(), "entry".to_string()));
        rewritten.push(format!("{}: `@ calculate` is now `@ entry`", name.line));
    }
    let declares_interview = source.contains("> interview(");
    let mut bound = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let Some(offset) = token_offset(&chars, &starts, token) else {
            continue;
        };
        if token.source_text == "Interview" && spelled_at(&chars, offset, "Interview") {
            listed.push(format!(
                "{}: left `Interview`: it is now `Answering(o, i, g)`, with `rounds` and `outcome`; rewrite this use by hand",
                token.line
            ));
            continue;
        }
        if token.source_text != "interview"
            || declares_interview
            || !spelled_at(&chars, offset, "interview")
        {
            continue;
        }
        let open = tokens
            .get(index + 1)
            .filter(|next| next.text == "(")
            .and_then(|next| token_offset(&chars, &starts, next));
        let call = open.and_then(|open| Some((open, closing_paren(&chars, open)?)));
        let parts = call.map(|(open, close)| arguments(&chars, open + 1, close - 1));
        let rewrite = parts.as_ref().and_then(|parts| match parts.as_slice() {
            [ask, partial, complete] => queried_entry(ask)
                .map(|entry| format!("answer({entry}, {partial}, from_case({complete}))")),
            _ => None,
        });
        let (Some((_, close)), Some(rewrite)) = (call, rewrite) else {
            listed.push(format!(
                "{}: left `interview`: it is now `answer(entry, case, resolve)`; rewrite this use by hand",
                token.line
            ));
            continue;
        };
        edits.push((offset, close, rewrite));
        rewritten.push(format!(
            "{}: `interview(|c| query e(c), partial, complete)` is now `answer(e, partial, from_case(complete))`",
            token.line
        ));
        let line_start = starts[token.line - 1];
        let before: String = chars[line_start..offset].iter().collect();
        let before = before.trim_end();
        if let Some(rest) = before.trim_start().strip_prefix('=') {
            if let Some(head) = rest.strip_suffix('=') {
                let name = head.split(':').next().unwrap_or_default().trim();
                if !name.is_empty() && name.chars().all(is_word_char) {
                    bound.push(name.to_string());
                }
            }
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        if !bound.contains(&token.source_text) {
            continue;
        }
        let (Some(dot), Some(member)) = (tokens.get(index + 1), tokens.get(index + 2)) else {
            continue;
        };
        if dot.text != "." {
            continue;
        }
        let (Some(start), Some(member_start)) = (
            token_offset(&chars, &starts, token),
            token_offset(&chars, &starts, member),
        ) else {
            continue;
        };
        let name = &token.source_text;
        let member_end = member_start + member.source_text.chars().count();
        match member.source_text.as_str() {
            "rounds" => {
                edits.push((start, member_end, format!("map({name}.rounds, |r| r.paths)")));
                rewritten.push(format!(
                    "{}: `{name}.rounds` is now `map({name}.rounds, |r| r.paths)`",
                    token.line
                ));
            }
            "result" if tokens.get(index + 3).is_some_and(|next| next.text == "==") => {
                let Some(equals) = token_offset(&chars, &starts, &tokens[index + 3]) else {
                    continue;
                };
                let value_start = equals + 2;
                let value_end = expression_end(&chars, value_start);
                let value: String = chars[value_start..value_end].iter().collect();
                edits.push((
                    start,
                    value_end,
                    format!("{name}.outcome == Answered(result = {})", value.trim()),
                ));
                rewritten.push(format!(
                    "{}: `{name}.result == v` is now `{name}.outcome == Answered(result = v)`",
                    token.line
                ));
            }
            "result" => listed.push(format!(
                "{}: left `{name}.result`: the result is now in `{name}.outcome`, `Answered(result)` or `Unanswered(open, case)`; rewrite this use by hand",
                token.line
            )),
            _ => {}
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
    EntriesMigration {
        source: output.into_iter().collect(),
        rewritten,
        listed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_becomes_entry_with_and_without_a_title() {
        let migrated = migrate_entries(
            "-- @ calculate in a comment stays\n@ calculate\n| f(x: Int) -> x\n@ calculate(\"Tax\")\n| g(x: Int) -> x\n= s = \"@ calculate\"\n",
        );
        assert_eq!(
            migrated.source,
            "-- @ calculate in a comment stays\n@ entry\n| f(x: Int) -> x\n@ entry(\"Tax\")\n| g(x: Int) -> x\n= s = \"@ calculate\"\n"
        );
        assert_eq!(
            migrated.rewritten,
            [
                "2: `@ calculate` is now `@ entry`",
                "4: `@ calculate` is now `@ entry`"
            ]
        );
    }

    #[test]
    fn an_interview_becomes_an_answer_with_its_rounds_and_result() {
        let migrated = migrate_entries(
            "= asked = interview(|c| query tax(c), H(a = unknown), done)\n| two: asked -> asked.rounds == [[pathof(H::a)]]\n| ends: asked -> asked.result == tax(done)\n| shown: asked -> show(asked.result)\n= other = interview(ask, h, done)\n",
        );
        assert_eq!(
            migrated.source,
            "= asked = answer(tax, H(a = unknown), from_case(done))\n| two: asked -> map(asked.rounds, |r| r.paths) == [[pathof(H::a)]]\n| ends: asked -> asked.outcome == Answered(result = tax(done))\n| shown: asked -> show(asked.result)\n= other = interview(ask, h, done)\n"
        );
        assert_eq!(migrated.rewritten.len(), 3);
        assert_eq!(migrated.listed.len(), 2);
        assert!(migrated.listed[0].starts_with("4: left `asked.result`"));
        assert!(migrated.listed[1].starts_with("5: left `interview`"));
    }
}
