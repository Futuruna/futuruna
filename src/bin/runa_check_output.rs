//! Presentation of `check` results. Backend failures are explicitly labelled:
//! generated Rust locations are not claimed to be Futuruna source locations.

use super::*;

/// Only CLI status messages use this writer. Authored program output and
/// emitted source bypass it, so NO_COLOR cannot change a program's values.
pub(super) fn write_status(arguments: std::fmt::Arguments<'_>, stderr: bool, newline: bool) {
    use std::io::IsTerminal;
    let terminal = if stderr {
        std::io::stderr().is_terminal()
    } else {
        std::io::stdout().is_terminal()
    };
    let color = terminal
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").ok().as_deref() != Some("dumb");
    let mut text = arguments.to_string();
    if !color {
        static COLORS: OnceLock<regex::Regex> = OnceLock::new();
        let colors = COLORS.get_or_init(|| regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap());
        text = colors.replace_all(&text, "").into_owned();
    }
    if newline {
        text.push('\n');
    }
    if stderr {
        eprint!("{text}");
    } else {
        print!("{text}");
    }
}

pub(super) struct CheckOutput<'a> {
    pub(super) filename: &'a str,
    pub(super) source: &'a str,
    pub(super) frontend_only: bool,
    pub(super) json: bool,
}

impl CheckOutput<'_> {
    fn location(filename: &str, source: &str, span: Option<Span>) -> serde_json::Value {
        let range = span.map(|span| {
            let (start_line, start_column) = span.start_line_col(source);
            let (end_line, end_column) = Span::new(span.end, span.end).start_line_col(source);
            serde_json::json!({
                "start": { "line": start_line, "column": start_column },
                "end": { "line": end_line, "column": end_column },
            })
        });
        serde_json::json!({"file": filename, "range": range})
    }

    fn diagnostic(&self, diagnostic: &Diagnostic) -> serde_json::Value {
        let location = match &diagnostic.origin {
            Some(origin) => {
                Self::location(&origin.path.to_string_lossy(), &origin.source, origin.span)
            }
            None => Self::location(self.filename, self.source, diagnostic.span),
        };
        let import_site = diagnostic
            .origin
            .as_ref()
            .map(|_| Self::location(self.filename, self.source, diagnostic.span));
        serde_json::json!({
            "severity": match diagnostic.severity {
                Severity::Error => "error", Severity::Warning => "warning", Severity::Help => "help",
            },
            "message": diagnostic.message,
            "location": location,
            "import_site": import_site,
            "notes": diagnostic.notes,
            "context": diagnostic.context,
        })
    }

    fn report(
        &self,
        ok: bool,
        phase: &str,
        diagnostics: Vec<serde_json::Value>,
        summary: Option<&CheckArtifactSummary>,
    ) {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": 1,
                "ok": ok,
                "file": self.filename,
                "phase": phase,
                "backend_validated": ok && !self.frontend_only,
                "diagnostics": diagnostics,
                "summary": summary,
            })
        );
    }

    pub(super) fn success(
        &self,
        summary: &CheckArtifactSummary,
        cached: Option<&str>,
        elapsed: std::time::Duration,
    ) {
        if self.json {
            self.report(
                true,
                if self.frontend_only {
                    "frontend"
                } else {
                    "backend"
                },
                vec![],
                Some(summary),
            );
            return;
        }
        let (green, dim, reset) = if should_use_color() {
            ("\x1b[1;32m", "\x1b[2m", "\x1b[0m")
        } else {
            ("", "", "")
        };
        let label = if self.frontend_only {
            "frontend check ok"
        } else {
            "check ok"
        };
        let backend = if self.frontend_only {
            "; Rust backend not validated".to_string()
        } else {
            let deps = if summary.cargo_dependency_count == 0 {
                String::new()
            } else {
                format!(", {} deps", summary.cargo_dependency_count)
            };
            let cache = cached.map(|kind| format!(", {kind}")).unwrap_or_default();
            format!("{deps}, {} lines of Rust{cache}", summary.rust_line_count)
        };
        eprintln!(
            "{green}{label}{reset}: {} ({} stmts, {} fns, {} types{backend}) {dim}[{:.1}s]{reset}",
            self.filename,
            summary.statement_count,
            summary.function_count,
            summary.type_count,
            elapsed.as_secs_f64()
        );
    }

    pub(super) fn frontend_errors(&self, diagnostics: &[Diagnostic]) -> bool {
        if diagnostics.is_empty() {
            return false;
        }
        if self.json {
            self.report(
                false,
                "frontend",
                diagnostics
                    .iter()
                    .map(|diag| self.diagnostic(diag))
                    .collect(),
                None,
            );
        } else {
            print_type_check_diagnostics(diagnostics, self.source, self.filename);
        }
        true
    }

    pub(super) fn failure(&self, phase: &str, message: String) {
        if self.json {
            self.report(
                false,
                phase,
                vec![self.diagnostic(&Diagnostic::error(message))],
                None,
            );
        } else {
            let (red, reset) = if should_use_color() {
                ("\x1b[1;31m", "\x1b[0m")
            } else {
                ("", "")
            };
            eprintln!("{red}check failed{reset}: {}\n{message}", self.filename);
        }
    }

    pub(super) fn parse_failure(&self, error: &str) {
        if !self.json {
            display_error_in(self.source, error, self.filename);
            return;
        }
        let diagnostics = runa_parse_diagnostics::parse_diagnostics(self.source, error);
        self.report(
            false,
            "parse",
            diagnostics
                .iter()
                .map(|diag| self.diagnostic(diag))
                .collect(),
            None,
        );
    }
}
