//! Test the checking tools against the runtime: whatever `fmt` and `verify`
//! claim must hold when the program actually runs.

use futuruna::{Defn, Lexer, Parser, Pat, Stmt, TokenKind, TypeDecl};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn runa() -> std::ffi::OsString {
    std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into())
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Run `runa <arguments>` from the repository root; `None` on timeout.
fn run(arguments: &[&str], timeout: Duration) -> Option<Output> {
    let mut child = Command::new(runa())
        .args(arguments)
        .current_dir(root())
        .env("FUTURUNA_SUPPRESS_COMPTIME_DIAGNOSTICS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        stdout.read_to_end(&mut buffer).unwrap();
        buffer
    });
    let err = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        stderr.read_to_end(&mut buffer).unwrap();
        buffer
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Some(Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    })
}

/// A sibling copy keeps relative imports working; its extension keeps it out
/// of every `.runa` corpus scan while it exists.
struct SiblingCopy(PathBuf);

impl SiblingCopy {
    fn new(original: &Path, source: &str) -> Self {
        let path = original.with_file_name(format!(
            ".{}.{}.oracle-tmp",
            original.file_stem().unwrap().to_string_lossy(),
            std::process::id()
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn arg(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for SiblingCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn parses(source: &str) -> Option<Vec<Stmt>> {
    Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .ok()
}

fn collect_runa(directory: &Path, recursive: bool, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if recursive {
                collect_runa(&path, recursive, files);
            }
        } else if path
            .extension()
            .is_some_and(|extension| extension == "runa")
        {
            files.push(path);
        }
    }
}

/// `meta --json` without positions and file names, which formatting may move.
fn normalized_meta(file: &str) -> Value {
    fn normalize(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.retain(|key, _| !key.ends_with("line") && !key.ends_with("file"));
                for (key, item) in object.iter_mut() {
                    if key == "text" {
                        if let Value::String(text) = item {
                            text.retain(|character| !character.is_whitespace());
                        }
                    }
                    normalize(item);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(normalize),
            _ => {}
        }
    }
    let output = run(&["meta", "--json", file], Duration::from_secs(60)).expect("meta finishes");
    let mut value: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    normalize(&mut value);
    value
}

/// Programs with layout that `fmt` rewrites. The repository corpus is kept
/// formatted, so these guarantee the oracle always compares real rewrites.
const UNFORMATTED_PROGRAMS: [(&str, &str); 2] = [
    (
        "rules_and_exceptions",
        "# Applicant(student: Bool, waiver: Bool)\n\
         |   fee(a: Applicant)   ->   100\n\
         | fee(a: Applicant) -> 40   under a.student\n\
         |   exception waived fee(a: Applicant) -> 0 under a.waiver\n\
         >   invoice(a: Applicant) -> String {\n\
         \x20       \"Fee: \" +   show(fee(a))\n\
         }\n\
         =   applicant = Applicant(true,   false)\n\
         @   print(invoice(applicant))\n",
    ),
    (
        "lambdas_and_templates",
        "> total(xs: List(Int)) -> Int {\n\
         \x20 foldl(xs, 0, |acc, x|   acc + x)\n\
         }\n\
         = items = [1,2,  3]\n\
         \x20 @ print(show(total(items)))\n\
         = labels = map(items, |x|   \"\"\"item {{x}}\"\"\")\n\
         @ print(join(labels,   \", \"))\n",
    ),
];

#[test]
fn formatting_preserves_program_output_and_metadata() {
    let mut files = Vec::new();
    collect_runa(&root().join("tests"), false, &mut files);
    collect_runa(&root().join("examples"), true, &mut files);
    let generated = root()
        .join("target")
        .join(format!("fmt-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&generated).unwrap();
    for (name, source) in UNFORMATTED_PROGRAMS {
        let path = generated.join(format!("{name}.runa"));
        std::fs::write(&path, source).unwrap();
        files.push(path);
    }
    files.sort();
    let mut compared = Vec::new();
    let mut failures = Vec::new();
    for path in &files {
        let relative = path.strip_prefix(root()).unwrap().to_str().unwrap();
        let source = std::fs::read_to_string(path).unwrap();
        if parses(&source).is_none() {
            continue;
        }
        let check = run(&["fmt", "--check", relative], Duration::from_secs(60)).unwrap();
        if check.status.success() {
            continue;
        }
        let copy = SiblingCopy::new(path, &source);
        let formatted = run(&["fmt", copy.arg()], Duration::from_secs(60)).unwrap();
        assert!(
            formatted.status.success(),
            "{relative}: {}",
            String::from_utf8_lossy(&formatted.stderr)
        );
        assert_ne!(
            std::fs::read_to_string(&copy.0).unwrap(),
            source,
            "{relative}"
        );
        compared.push(relative.to_string());
        if normalized_meta(relative) != normalized_meta(copy.arg()) {
            failures.push(format!("{relative}: meta --json changed after fmt"));
        }
        // Programs that do not finish promptly (servers, long searches) are
        // compared by metadata only.
        let Some(before) = run(&[relative], Duration::from_secs(20)) else {
            continue;
        };
        let after = run(&[copy.arg()], Duration::from_secs(60)).expect("formatted run finishes");
        let rename = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace(copy.arg(), relative);
        if before.status.code() != after.status.code()
            || rename(&before.stdout) != rename(&after.stdout)
        {
            failures.push(format!(
                "{relative}: output changed after fmt\nbefore: {}\nafter: {}",
                String::from_utf8_lossy(&before.stdout),
                String::from_utf8_lossy(&after.stdout)
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&generated);
    for (name, _) in UNFORMATTED_PROGRAMS {
        assert!(
            compared
                .iter()
                .any(|file| file.ends_with(&format!("{name}.runa"))),
            "{name} was not rewritten by fmt"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const BOUNDARY_INPUTS: [&str; 4] = [
    "0",
    "-1",
    "9223372036854775807",
    "(-9223372036854775807 - 1)",
];

fn top_level_names(statements: &[Stmt]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Stmt::Bind(Pat::Var(name), _, _) | Stmt::StreamBind(name, _) => {
                names.insert(name.clone());
            }
            Stmt::Defn(Defn::Fn { name, .. }) => {
                names.insert(name.clone());
            }
            Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                names.insert(name.clone());
            }
            Stmt::Rule(rule) => {
                if let Some((name, _)) = rule.callable_name_arity() {
                    names.insert(name);
                }
            }
            Stmt::QualifiedImport(alias, _) => {
                names.insert(alias.clone());
            }
            _ => {}
        }
    }
    names
}

/// Identifiers in the predicate that no declaration binds: `verify`
/// quantifies them over every Int.
fn free_variables(predicate: &str, declared: &BTreeSet<String>) -> Vec<String> {
    let tokens = Lexer::new(predicate).tokenize();
    let mut free = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let called = tokens
            .get(index + 1)
            .is_some_and(|next| next.kind == TokenKind::LParen);
        let member = index > 0 && tokens[index - 1].kind == TokenKind::Dot;
        if token.kind == TokenKind::Ident
            && !called
            && !member
            && !declared.contains(&token.text)
            && !free.contains(&token.text)
        {
            free.push(token.text.clone());
        }
    }
    free
}

fn assignments(count: usize) -> Vec<Vec<&'static str>> {
    let mut all = vec![Vec::new()];
    for _ in 0..count {
        all = all
            .into_iter()
            .flat_map(|prefix| {
                BOUNDARY_INPUTS.iter().map(move |value| {
                    let mut next = prefix.clone();
                    next.push(*value);
                    next
                })
            })
            .collect();
    }
    all
}

#[test]
fn invariants_proved_by_verify_hold_at_integer_boundaries() {
    if !std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        eprintln!("skipping verify oracle: Z3 not found, so verify proves no invariants");
        return;
    }
    let mut files = vec![
        root().join("tests/verify_test.runa"),
        root().join("tests/verify_blocks_test.runa"),
        root().join("tests/list_equality_verify_test.runa"),
        root().join("tests/issue-repros/tutorial-project-math/main.runa"),
        root().join("tests/issue-repros/tutorial-project-math/math.runa"),
    ];
    collect_runa(&root().join("tests/fixtures/verify"), false, &mut files);
    let marker = "__verify_oracle__";
    let mut evaluated = 0;
    let mut failures = Vec::new();
    for path in &files {
        let relative = path.strip_prefix(root()).unwrap().to_str().unwrap();
        let verify = run(&["verify", relative], Duration::from_secs(120)).unwrap();
        let report = String::from_utf8_lossy(&verify.stdout);
        let proved: Vec<&str> = report
            .lines()
            .filter_map(|line| line.trim().strip_prefix("✓ PROVED: |"))
            .filter_map(|rest| rest.split('|').next())
            .collect();
        if proved.is_empty() {
            continue;
        }
        let source = std::fs::read_to_string(path).unwrap();
        let statements = parses(&source).expect("verified file parses");
        let declared = top_level_names(&statements);
        // Keep declarations and program flow; `?` statements (with their
        // handler blocks) would halt on the program's own terms, which is not
        // what this oracle measures.
        let mut program = String::new();
        let mut open_braces = 0i32;
        for line in source.lines() {
            if open_braces > 0 || line.starts_with('?') {
                open_braces += line.matches('{').count() as i32;
                open_braces -= line.matches('}').count() as i32;
                continue;
            }
            program.push_str(line);
            program.push('\n');
        }
        let mut checks = 0;
        for (index, name) in proved.iter().enumerate() {
            let Some(predicate) = source.lines().find_map(|line| {
                line.strip_prefix(&format!("| {name}:"))
                    .and_then(|rest| rest.split_once(" -> "))
                    .map(|(_, predicate)| predicate.trim().to_string())
            }) else {
                continue;
            };
            let free = free_variables(&predicate, &declared);
            let parameters = free
                .iter()
                .map(|variable| format!("{variable}: Int"))
                .collect::<Vec<_>>()
                .join(", ");
            program.push_str(&format!(
                "> {marker}{index}({parameters}) -> Bool {{ {predicate} }}\n"
            ));
            for values in assignments(free.len()) {
                program.push_str(&format!(
                    "@ print(\"{marker} {name} {}: \" + show({marker}{index}({})))\n",
                    values.join(","),
                    values.join(", ")
                ));
                checks += 1;
            }
        }
        let copy = SiblingCopy::new(path, &program);
        let output = run(&[copy.arg()], Duration::from_secs(120)).expect("oracle program finishes");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let results: Vec<&str> = stdout
            .lines()
            .filter(|line| line.starts_with(marker))
            .collect();
        evaluated += results.len();
        if !output.status.success() || results.len() != checks {
            failures.push(format!(
                "{relative}: evaluating proved invariants failed\n{}\nprogram:\n{program}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        failures.extend(
            results
                .iter()
                .filter(|line| !line.ends_with(": true"))
                .map(|line| format!("{relative}: {line}")),
        );
    }
    assert!(evaluated > 0, "verify proved nothing in its test corpus");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
