use crate::{Parser, Rule, Stmt, TokenKind};

impl Parser {
    /// Add advice only after parsing has failed. These names remain available
    /// as ordinary identifiers in valid source.
    pub(crate) fn foreign_syntax_hint(&self, at: usize) -> &'static str {
        let Some(token) = self.tokens.get(at) else {
            return "";
        };
        if token.kind == TokenKind::FatArrow {
            return "\n  Hint: lambdas use `|x| expression`; rule and match arms use `->`.";
        }
        let previous = at.checked_sub(1).and_then(|i| self.tokens.get(i));
        for candidate in [Some(token), previous.filter(|p| p.line == token.line)]
            .into_iter()
            .flatten()
        {
            if !matches!(candidate.kind, TokenKind::Ident | TokenKind::KW) {
                continue;
            }
            match candidate.text.as_str() {
                "when" => return "\n  Hint: conditionals use `if condition { value } else { value }`; rule guards use `under condition`.",
                "where" => return "\n  Hint: rule guards use `under condition`.",
                "elif" => return "\n  Hint: continue a conditional with `else if condition { value }`.",
                "return" => return "\n  Hint: a function returns its last expression; omit `return`.",
                "let" => return "\n  Hint: ordinary bindings use `= name = value`.",
                _ => {}
            }
        }
        ""
    }

    pub(crate) fn statement_syntax_hint(&self, statement: &Stmt) -> &'static str {
        match statement {
            Stmt::TypeDecl(_) => "\n  Hint: `#` declares a type; line comments use `--`.",
            Stmt::Rule(Rule::Default { condition: Some(_), .. } | Rule::Exception { condition: Some(_), .. })
                if self.peek_kind() == TokenKind::Comma =>
                "\n  Hint: combine guard conditions with `and` or `&&`; commas separate goals in a rule body.",
            Stmt::Rule(_) if self.peek_kind() == TokenKind::Colon
                && self.tokens.get(self.pos + 1).is_some_and(|next| next.kind == TokenKind::Op && next.text == "-") =>
                "\n  Hint: a rule separates its head and body with `->`, not `:-`.",
            _ => self.foreign_syntax_hint(self.pos),
        }
    }
}
