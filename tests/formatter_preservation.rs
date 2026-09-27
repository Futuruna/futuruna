use futuruna::{eval_source_with_prelude, scan_meta_comments, Lexer, Parser};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn fixture_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "futuruna-format-preservation-{}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn format_path(path: &Path, check: bool) -> Output {
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let mut command = Command::new(runa);
    command.arg("fmt");
    if check {
        command.arg("--check");
    }
    command.arg(path).output().unwrap()
}

struct SourceFile(PathBuf);

impl SourceFile {
    fn new(source: &str) -> Self {
        let path = fixture_path().with_extension("runa");
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn format(&self, check: bool) -> Output {
        format_path(&self.0, check)
    }

    fn text(&self) -> String {
        std::fs::read_to_string(&self.0).unwrap()
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn format_successfully(source: &str) -> String {
    let file = SourceFile::new(source);
    let output = file.format(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let formatted = file.text();
    let check = file.format(true);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(file.text(), formatted, "--check must not write");
    formatted
}

#[test]
fn formatting_preserves_the_send_operator_and_its_effect() {
    let source = "~ s = subject(\"boot\")\ns<-\"a\"\n@ print(show(s.count))\n";
    assert_eq!(eval_source_with_prelude(source, true).unwrap().trim(), "2");
    let formatted = format_successfully(source);
    assert!(formatted.contains("s <- \"a\""), "{formatted}");
    assert_eq!(
        eval_source_with_prelude(&formatted, true).unwrap().trim(),
        "2"
    );
}

#[test]
fn quote_characters_do_not_expose_string_contents_to_operator_spacing() {
    let source =
        "@ print(show('\"') + \"a+b\")\n@ print(\"c-d\")\n@ print(show('\\'') + \"x*y\")\n";
    let expected = eval_source_with_prelude(source, true).unwrap();
    let formatted = format_successfully(source);
    assert_eq!(
        eval_source_with_prelude(&formatted, true).unwrap(),
        expected
    );
    assert!(formatted.contains("\"a+b\""), "{formatted}");
    assert!(formatted.contains("\"c-d\""), "{formatted}");
}

#[test]
fn multiline_and_template_strings_keep_their_exact_value() {
    for source in [
        "> f() -> String {\n    \"line1\n      line2\"\n}\n@ print(f())\n",
        "> f() -> String {\n    \"line1  \n\n\n\t line2 \"\n}\n@ print(f())\n",
        "= number = 3\n@ print(\"\"\"\n  first  \n\n\n    second {{number+1}}  \n\"\"\")\n",
    ] {
        let expected = eval_source_with_prelude(source, true).unwrap();
        let formatted = format_successfully(source);
        assert_eq!(
            eval_source_with_prelude(&formatted, true).unwrap(),
            expected,
            "{formatted}"
        );
    }
}

#[test]
fn quoted_legal_source_preserves_indentation_spaces_and_blank_lines() {
    let quoted =
        "----\nSection 3. Text.  \n      Item 1) indented item\n\n\n   Item 2) other\n----";
    let source = format!("# SourceInfo(identifier: String)\n# impl Meta for SourceInfo {{}}\n= section_meta = SourceInfo(identifier = \"section 3\")\n\n--@label:section3::meta:section_meta--\n{quoted}\n--@begin:section3--\n| permitted()->True\n--@end:section3--\n");
    let formatted = format_successfully(&source);
    assert!(formatted.contains(quoted), "{formatted}");
    // Raw source text is provenance independent of the executable AST.
    let before = scan_meta_comments(&source);
    let after = scan_meta_comments(&formatted);
    assert_eq!(before.anchors[0].text, after.anchors[0].text);
}

#[test]
fn malformed_programs_are_rejected_without_rewriting_them() {
    for source in ["= x = (\n", "> broken(x: Int) -> Int {\n"] {
        let file = SourceFile::new(source);
        for check in [false, true] {
            let output = file.format(check);
            assert!(
                !output.status.success(),
                "malformed source reported formatted: {source}"
            );
            assert_eq!(file.text(), source);
        }
    }
}

#[test]
fn formatting_preserves_unicode_literals_comments_and_embedded_rust() {
    let source = r####"-- Quotes in comments: " ' ---- do not open literals.
= __runa_fmt_verbatim_0__ = "§ 3 — æøå + ⊢"
> text() -> String {
"""
  {{__runa_fmt_verbatim_0__}}
    "quoted" -- still string contents
"""
}
@ print(text())
"####;
    let expected = eval_source_with_prelude(source, true).unwrap();
    let formatted = format_successfully(source);
    assert_eq!(
        eval_source_with_prelude(&formatted, true).unwrap(),
        expected
    );
    let rust = "@ rust {\nfn text() -> &'static str {\n    \"first\n      second  \"\n}\n}\n";
    let formatted = format_successfully(rust);
    assert!(
        formatted.contains("\"first\n      second  \""),
        "{formatted}"
    );
}

#[test]
fn directory_formatting_reports_invalid_files_and_preserves_them() {
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            for name in ["valid.runa", "invalid.runa"] {
                std::fs::remove_file(self.0.join(name)).unwrap();
            }
            std::fs::remove_dir(&self.0).unwrap();
        }
    }
    let directory = Directory(fixture_path());
    std::fs::create_dir(&directory.0).unwrap();
    let invalid = "= x = (\n";
    std::fs::write(directory.0.join("invalid.runa"), invalid).unwrap();
    for check in [true, false] {
        let valid = "=answer = 1+2\n";
        let valid_path = directory.0.join("valid.runa");
        std::fs::write(&valid_path, valid).unwrap();
        let output = format_path(&directory.0, check);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("invalid.runa"), "{stderr}");
        assert!(stderr.contains("cannot format invalid syntax"), "{stderr}");
        assert_eq!(
            std::fs::read_to_string(directory.0.join("invalid.runa")).unwrap(),
            invalid
        );
        assert_eq!(
            std::fs::read_to_string(valid_path).unwrap(),
            if check { valid } else { "= answer = 1 + 2\n" }
        );
    }
}

#[test]
fn repository_format_check_covers_valid_and_intentionally_invalid_sources() {
    // These parser-diagnostic fixtures intentionally have no valid AST. They
    // must be rejected, not silently skipped or reported correctly formatted.
    const INVALID_SYNTAX: &[&str] = &[
        "tests/errors/multi_parse_error.runa",
        "tests/errors/parse_bad_arrow.runa",
        "tests/errors/parse_missing_body.runa",
        "tests/errors/parse_unclosed_brace.runa",
        "tests/expect/check/late_language_directive.runa",
        "tests/expect/check/missing_exception_label.runa",
        "tests/expect/check/single_equals_expression.runa",
        "tests/expect/diagnostics/depend_missing_quotes.runa",
        "tests/expect/diagnostics/depend_missing_version.runa",
        "tests/expect/diagnostics/parse_bad_arrow.runa",
    ];
    fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "runa") {
                files.push(path);
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect(&root.join("tests"), &mut files);
    files.sort();
    assert!(!files.is_empty());
    let mut rejected = Vec::new();
    for path in &files {
        let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
        let source = std::fs::read_to_string(path).unwrap();
        let output = format_path(path, true);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(std::fs::read_to_string(path).unwrap(), source, "{relative}");
        if INVALID_SYNTAX.contains(&relative) {
            let tokens = Lexer::new(&source).tokenize();
            assert!(
                Parser::new(tokens, &source).parse_program().is_err(),
                "{relative}"
            );
            assert!(!output.status.success(), "{relative}: {stderr}");
            assert!(
                stderr.contains("cannot format invalid syntax"),
                "{relative}: {stderr}"
            );
            rejected.push(relative.to_string());
        } else {
            assert!(output.status.success(), "{relative}: {stderr}");
        }
    }
    assert_eq!(rejected, INVALID_SYNTAX);
    eprintln!(
        "Checked {} source files, including {} expected syntax rejections",
        files.len(),
        rejected.len()
    );
}
