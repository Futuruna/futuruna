use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "futuruna-import-paths-{}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("model")).unwrap();
        std::fs::write(
            root.join("shared.runa"),
            "# Input(value: Int)\n# CalculationOutput(total: Int)\n> base() -> Int { 40 }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("model/dependency.runa"),
            "@ import ../shared\n> adjusted(input: Input) -> CalculationOutput { CalculationOutput(total = base() + input.value) }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("model/main.runa"),
            "@ import ./dependency\n@ calculate(\"Import path fixture\")\n> calculate_example(input: Input) -> CalculationOutput { adjusted(input) }\n@ print(show(calculate_example(Input(value = 2)).total))\n",
        )
        .unwrap();
        Self(root)
    }

    fn run(&self, command: &[&str], source: &Path, options: &[&str]) -> Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(runa)
            .current_dir(self.0.join("model"))
            .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
            .args(command)
            .arg(source)
            .args(options)
            .output()
            .unwrap()
    }

    fn source_paths(&self) -> [PathBuf; 4] {
        [
            "main.runa".into(),
            "./main.runa".into(),
            "../model/main.runa".into(),
            self.0.join("model/main.runa"),
        ]
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["model/main.runa", "model/dependency.runa", "shared.runa"] {
            std::fs::remove_file(self.0.join(name)).unwrap();
        }
        std::fs::remove_dir(self.0.join("model")).unwrap();
        std::fs::remove_dir(&self.0).unwrap();
    }
}

fn assert_success(output: &Output, command: &[&str], source: &Path) {
    assert!(
        output.status.success(),
        "{command:?} {source:?}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn interpreted_and_native_imports_follow_the_source_directory_for_all_filename_forms() {
    let fixture = Fixture::new();
    for command in [&[][..], &["--no-prelude"][..], &["run"][..]] {
        for source in fixture.source_paths() {
            let output = fixture.run(command, &source, &[]);
            assert_success(&output, command, &source);
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
        }
    }
}

#[test]
fn check_and_emit_resolve_bare_filename_imports() {
    let fixture = Fixture::new();
    for command in [&["check"][..], &["emit"][..]] {
        for source in fixture.source_paths() {
            let output = fixture.run(command, &source, &[]);
            assert_success(&output, command, &source);
            if command == ["emit"] {
                let code = String::from_utf8_lossy(&output.stdout);
                for declaration in [
                    "fn calculate_example",
                    "fn adjusted",
                    "fn base",
                    "struct Input",
                ] {
                    assert!(code.contains(declaration), "missing {declaration}: {code}");
                }
            }
        }
    }
}

#[test]
fn schema_and_audit_load_the_same_nested_imports_for_bare_filenames() {
    let fixture = Fixture::new();
    for source in fixture.source_paths() {
        let schema = fixture.run(&["schema"], &source, &[]);
        assert_success(&schema, &["schema"], &source);
        let schema: serde_json::Value = serde_json::from_slice(&schema.stdout).unwrap();
        assert_eq!(schema["input"]["name"], "Input");
        assert_eq!(schema["output"]["name"], "CalculationOutput");
        let audit = fixture.run(
            &["audit"],
            &source,
            &["--entry", "calculate_example", "--json"],
        );
        assert_success(&audit, &["audit"], &source);
        let audit: serde_json::Value = serde_json::from_slice(&audit.stdout).unwrap();
        assert!(audit["callables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| { item["qualified_name"] == "adjusted" && item["status"] == "reachable" }));
    }
}

#[test]
fn library_import_resolution_treats_an_empty_source_directory_as_current_directory() {
    for import in ["./dependency", "../dependency"] {
        let resolved = futuruna::Interpreter::resolve_import_path_for_source(import, "").unwrap();
        assert!(!Path::new(&resolved).is_absolute(), "{resolved}");
        assert_eq!(
            Some(resolved),
            futuruna::Interpreter::resolve_import_path_for_source(import, ".")
        );
    }
}
