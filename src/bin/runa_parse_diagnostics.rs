//! Adapt the parser's legacy text errors into source diagnostics for CLI and
//! editor clients, without changing the public parser result type.

use futuruna::{Diagnostic, Span};

fn source_span(source: &str, line: usize, column: usize) -> Option<Span> {
    let mut current = (1, 1);
    let mut end = 0;
    for (offset, character) in source.chars().enumerate() {
        if current == (line, column) {
            return Some(Span::new(offset, offset + 1));
        }
        if character == '\n' {
            current = (current.0 + 1, 1);
        } else {
            current.1 += 1;
        }
        end = offset + 1;
    }
    (current == (line, column)).then(|| Span::new(end, end))
}

pub(super) fn parse_diagnostics(source: &str, error: &str) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for (index, text) in error.lines().enumerate() {
        // The aggregate parser header carries no diagnostic of its own.
        if index == 0
            && text
                .strip_suffix(" parse error:")
                .or_else(|| text.strip_suffix(" parse errors:"))
                .is_some_and(|count| count.parse::<usize>().is_ok())
        {
            continue;
        }
        let mut parts = text.splitn(3, ':');
        let position = parts
            .next()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .zip(
                parts
                    .next()
                    .and_then(|value| value.trim().parse::<usize>().ok()),
            )
            .zip(parts.next());
        if let Some(((line, column), message)) = position {
            let mut diagnostic = Diagnostic::error(message.trim());
            diagnostic.span = source_span(source, line, column);
            diagnostics.push(diagnostic);
        } else if let Some(diagnostic) = diagnostics.last_mut() {
            // Hints and the parser's recovery-limit notice belong to the
            // preceding error, not to a made-up location at the file start.
            diagnostic.message.push('\n');
            diagnostic.message.push_str(text);
        } else if !text.is_empty() {
            diagnostics.push(Diagnostic::error(text));
        }
    }
    if diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(error));
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpositioned_parser_errors_are_not_lost_before_positioned_errors() {
        let diagnostics = parse_diagnostics(
            "bad",
            "2 parse errors:\nunlocated failure\n1:2: positioned failure\n  Hint: retain context",
        );
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].message, "unlocated failure");
        assert_eq!(diagnostics[0].span, None);
        assert_eq!(diagnostics[1].span, Some(Span::new(1, 2)));
        assert!(diagnostics[1].message.ends_with("Hint: retain context"));
    }

    #[test]
    fn invalid_parser_positions_do_not_invent_source_spans() {
        for location in ["0:1", "1:0", "2:1", "1:5"] {
            let diagnostics = parse_diagnostics("bad", &format!("{location}: invalid position"));
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].span, None);
        }
        assert_eq!(
            parse_diagnostics("", "1:1: unexpected EOF")[0].span,
            Some(Span::new(0, 0))
        );
    }
}
