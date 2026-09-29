use crate::{Parser, Rule, Stmt, Token, TokenKind};

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
        if let Some(previous) = previous.filter(|p| p.line == token.line) {
            if previous.kind == TokenKind::Ident
                && previous.text == "f"
                && token.kind == TokenKind::String_
                && previous.col + 1 == token.col
            {
                return "\n  Hint: interpolation uses a triple-quoted template, as in `\"\"\"Hello {{name}}\"\"\"`.";
            }
            if previous.kind == TokenKind::Op && previous.text == "?" {
                return "\n  Hint: conditional expressions use `if condition { value } else { value }`.";
            }
        }
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
                "let" | "var" | "val" | "const" => return "\n  Hint: ordinary bindings use `= name = value`.",
                "fn" | "func" | "fun" | "def" | "function" => return "\n  Hint: functions are declared with `> name(parameter: Type) -> Type { body }`.",
                "switch" => return "\n  Hint: branch on a value with `match value { pattern -> result }`.",
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

    /// A line that starts with `-` or `||` begins a new statement (negation or a
    /// closure) rather than continuing the previous line. Indenting such a line
    /// under the previous statement makes it look like a continuation, so it is
    /// rejected. Other binary operators continue the line during layout.
    pub(crate) fn reject_line_leading_operator(
        &self,
        previous_col: Option<usize>,
    ) -> Result<(), String> {
        let token = self.peek();
        if token.kind != TokenKind::Op || !matches!(token.text.as_str(), "-" | "||") {
            return Ok(());
        }
        let line_start = at_line_start(self.pos.checked_sub(1).and_then(|at| self.tokens.get(at)));
        let indented = previous_col.is_some_and(|col| token.col > col);
        if !line_start || !indented {
            return Ok(());
        }
        let operator = &token.text;
        Err(format!(
            "{}:{}: an indented line starting with `{operator}` does not continue the previous \
             line; `{operator}` at the start of a line begins a new statement. Put `{operator}` \
             at the end of the previous line, or wrap the whole expression in parentheses",
            token.line, token.col
        ))
    }
}

fn at_line_start(previous: Option<&Token>) -> bool {
    previous.is_none_or(|token| token.kind == TokenKind::Semi && token.text == "\n")
}
