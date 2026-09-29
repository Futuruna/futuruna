//! Report rustc failures on generated Rust in terms of the Futuruna source.
//! Lines of an `@ rust` block map to their exact source line; lines of
//! compiler-generated code map to the declaration they were emitted for and
//! are reported as a compiler bug. Generated file paths are never shown; the
//! complete rustc output is available with `FUTURUNA_SHOW_RUSTC=1`.

use super::*;

pub(super) const SHOW_RUSTC_ENV: &str = "FUTURUNA_SHOW_RUSTC";

/// One rustc error with its primary location in the generated file. Errors
/// located elsewhere (a dependency, a build script) have no generated line.
#[derive(Debug, PartialEq, Eq)]
struct GeneratedRustError {
    message: String,
    line: Option<usize>,
}

fn rustc_errors(stderr: &str, generated_file_name: &str) -> Vec<GeneratedRustError> {
    let mut errors = Vec::new();
    let mut pending: Option<String> = None;
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("error") {
            if let Some(message) = pending.take() {
                errors.push(GeneratedRustError {
                    message,
                    line: None,
                });
            }
            pending = rest
                .split_once(": ")
                .map(|(_, message)| message.trim().to_string())
                .filter(|message| {
                    !message.starts_with("aborting due to")
                        && !message.starts_with("could not compile")
                });
            continue;
        }
        let Some(location) = line.trim_start().strip_prefix("--> ") else {
            continue;
        };
        if let Some(message) = pending.take() {
            // `path:line:column`; the path itself may contain `:`.
            let mut parts = location.rsplitn(3, ':');
            let _column = parts.next();
            let line = parts.next().and_then(|line| line.trim().parse().ok());
            let in_generated_file = parts.next().is_some_and(|path| {
                Path::new(path.trim())
                    .file_name()
                    .is_some_and(|name| name == generated_file_name)
            });
            errors.push(GeneratedRustError {
                message,
                line: line.filter(|_| in_generated_file),
            });
        }
    }
    if let Some(message) = pending {
        errors.push(GeneratedRustError {
            message,
            line: None,
        });
    }
    errors
}

#[derive(Debug, PartialEq, Eq)]
enum GeneratedRustOrigin {
    /// Zero-based source line inside an `@ rust` block.
    EmbeddedRust(usize),
    /// A declaration name and its zero-based line and byte column.
    Declaration(String, usize, usize),
    Unknown,
}

fn trimmed_window_start(haystack: &[&str], needle: &[&str]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|start| {
        needle
            .iter()
            .enumerate()
            .all(|(offset, line)| haystack[start + offset].trim() == line.trim())
    })
}

/// Name of a top-level generated Rust item starting on this line.
fn generated_item_name(line: &str) -> Option<&str> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let line = line.strip_prefix("pub ").unwrap_or(line);
    let identifier = |text: &str| -> Option<usize> {
        let end = text
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .unwrap_or(text.len());
        (end > 0).then_some(end)
    };
    for keyword in [
        "fn ", "struct ", "enum ", "const ", "static ", "type ", "trait ",
    ] {
        if let Some(rest) = line.strip_prefix(keyword) {
            return identifier(rest).map(|end| &rest[..end]);
        }
    }
    let rest = line.strip_prefix("impl")?;
    let header = rest.split('{').next()?.trim_end();
    let header = header.rsplit(" for ").next()?;
    let name = header.trim().rsplit("::").next()?;
    identifier(name).map(|end| &name[..end])
}

fn generated_rust_origin(
    source: &str,
    statements: &[Stmt],
    code: &str,
    line: usize,
) -> GeneratedRustOrigin {
    let code_lines: Vec<&str> = code.lines().collect();
    let source_lines: Vec<&str> = source.lines().collect();
    let Some(index) = line
        .checked_sub(1)
        .filter(|index| *index < code_lines.len())
    else {
        return GeneratedRustOrigin::Unknown;
    };
    for statement in statements {
        let Stmt::RustBlock(block) = statement else {
            continue;
        };
        let block_lines: Vec<&str> = block.lines().collect();
        let (Some(generated), Some(authored)) = (
            trimmed_window_start(&code_lines, &block_lines),
            trimmed_window_start(&source_lines, &block_lines),
        ) else {
            continue;
        };
        if (generated..generated + block_lines.len()).contains(&index) {
            return GeneratedRustOrigin::EmbeddedRust(authored + index - generated);
        }
    }
    // The nearest line starting in column 0 with a keyword opens the item
    // that contains the error; closing braces and attributes are skipped.
    let Some(name) = code_lines[..=index]
        .iter()
        .rev()
        .find(|line| line.starts_with(char::is_alphabetic))
        .and_then(|line| generated_item_name(line))
    else {
        return GeneratedRustOrigin::Unknown;
    };
    match lsp_find_def_pos(source, name) {
        Some((line, column)) => {
            GeneratedRustOrigin::Declaration(name.to_string(), line as usize, column)
        }
        None => GeneratedRustOrigin::Unknown,
    }
}

fn line_span(source: &str, line: usize, byte_column: usize, byte_length: usize) -> Span {
    let line_start: usize = source
        .split_inclusive('\n')
        .take(line)
        .map(|line| line.chars().count())
        .sum();
    let text = source.lines().nth(line).unwrap_or("");
    let byte_column = byte_column.min(text.len());
    let byte_end = (byte_column + byte_length).min(text.len());
    let start = line_start + text[..byte_column].chars().count();
    let end = line_start + text[..byte_end].chars().count();
    Span::new(start, end.max(start + 1))
}

/// Located diagnostics for a failed rustc/Cargo validation of generated Rust.
/// `generated_file_name` is the file name rustc reports for the generated code.
pub(super) fn generated_rust_diagnostics(
    source: &str,
    filename: &str,
    statements: &[Stmt],
    code: &str,
    rustc_stderr: &str,
    generated_file_name: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut unmapped = Vec::new();
    let mut unlocated = Vec::new();
    let mut seen = BTreeSet::new();
    for error in rustc_errors(rustc_stderr, generated_file_name) {
        let Some(line) = error.line else {
            unlocated.push(error.message);
            continue;
        };
        let diagnostic = match generated_rust_origin(source, statements, code, line) {
            GeneratedRustOrigin::EmbeddedRust(line) => {
                let text = source.lines().nth(line).unwrap_or("");
                let indent = text.len() - text.trim_start().len();
                Diagnostic::error_at(
                    line_span(source, line, indent, text.trim().len()),
                    format!("embedded Rust does not compile: {}", error.message),
                )
            }
            GeneratedRustOrigin::Declaration(name, line, column) => Diagnostic::error_at(
                line_span(source, line, column, name.len()),
                format!(
                    "the Rust generated for `{}` does not compile (Futuruna compiler bug): {}",
                    name, error.message
                ),
            )
            .with_note(
                "the frontend accepted this declaration; please report the program as a compiler bug",
            ),
            GeneratedRustOrigin::Unknown => {
                unmapped.push(error.message);
                continue;
            }
        };
        if seen.insert((
            diagnostic.span.map(|span| span.start),
            diagnostic.message.clone(),
        )) {
            diagnostics.push(diagnostic);
        }
    }
    if !unmapped.is_empty() {
        let mut diagnostic = Diagnostic::error(format!(
            "the Rust generated for {} does not compile (Futuruna compiler bug)",
            filename
        ));
        for message in unmapped {
            diagnostic = diagnostic.with_note(format!("rustc: {message}"));
        }
        diagnostics.push(diagnostic);
    }
    if !unlocated.is_empty() || diagnostics.is_empty() {
        let mut diagnostic = Diagnostic::error(format!(
            "building the generated Rust for {} failed",
            filename
        ));
        for message in unlocated {
            diagnostic = diagnostic.with_note(message);
        }
        diagnostics.push(diagnostic);
    }
    if let Some(last) = diagnostics.last_mut() {
        last.notes.push(format!(
            "set {SHOW_RUSTC_ENV}=1 to print the complete rustc output"
        ));
    }
    diagnostics
}

pub(super) fn show_rustc_output() -> bool {
    cache_env_enabled(SHOW_RUSTC_ENV)
}

/// `run`/`build` report: located diagnostics, and with `FUTURUNA_SHOW_RUSTC`
/// the generated file and the complete rustc output.
pub(super) fn print_generated_rust_failure(
    source: &str,
    filename: &str,
    statements: &[Stmt],
    code: &str,
    rustc_stderr: &str,
    generated_path: &str,
) {
    let use_color = should_use_color();
    let generated_file_name = Path::new(generated_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    for diagnostic in generated_rust_diagnostics(
        source,
        filename,
        statements,
        code,
        rustc_stderr,
        &generated_file_name,
    ) {
        eprint!("{}", diagnostic.display(source, filename, use_color));
    }
    if show_rustc_output() {
        eprintln!("\ngenerated Rust: {generated_path}\n{rustc_stderr}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<Stmt> {
        Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .unwrap()
    }

    #[test]
    fn rustc_errors_keep_the_primary_generated_line_of_each_error() {
        let stderr = "error[E0425]: cannot find type `Intt` in this scope\n   --> /cache/x:y/check.rs:167:10\n    |\n\nerror: linking failed\n\nerror[E0599]: no method\n --> /registry/dep-1.0/src/lib.rs:4:2\n\nerror: aborting due to 3 previous errors\n";
        assert_eq!(
            rustc_errors(stderr, "check.rs"),
            vec![
                GeneratedRustError {
                    message: "cannot find type `Intt` in this scope".into(),
                    line: Some(167),
                },
                GeneratedRustError {
                    message: "linking failed".into(),
                    line: None,
                },
                GeneratedRustError {
                    message: "no method".into(),
                    line: None,
                },
            ]
        );
    }

    #[test]
    fn generated_lines_map_to_embedded_rust_or_the_emitting_declaration() {
        let source = "> twice(x: Int) -> Int { x * 2 }\n@ rust {\n    fn helper() -> i64 {\n        \"x\"\n    }\n}\n";
        let statements = parse(source);
        let code = "use std::fmt;\nfn twice(x: i64) -> i64 {\n    x * 2\n}\nfn helper() -> i64 {\n    \"x\"\n}\nfn main() {\n}\n";
        assert_eq!(
            generated_rust_origin(source, &statements, code, 6),
            GeneratedRustOrigin::EmbeddedRust(3)
        );
        assert_eq!(
            generated_rust_origin(source, &statements, code, 3),
            GeneratedRustOrigin::Declaration("twice".into(), 0, 2)
        );
        assert_eq!(
            generated_rust_origin(source, &statements, code, 9),
            GeneratedRustOrigin::Unknown
        );
        let diagnostics = generated_rust_diagnostics(
            source,
            "main.runa",
            &statements,
            code,
            "error[E0308]: mismatched types\n --> /cache/check.rs:3:5\n",
            "check.rs",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].span.map(|span| span.start_line_col(source)),
            Some((1, 3))
        );
    }
}
