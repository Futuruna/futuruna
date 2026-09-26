use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "futuruna-source-arguments-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let output = run(&root, &["init", "sample"]);
        assert!(output.status.success(), "{output:?}");
        Self(root)
    }

    fn directory(&self) -> PathBuf {
        self.0.join("sample")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

#[test]
fn source_commands_without_a_file_fail_instead_of_entering_the_repl() {
    let project = Project::new();
    for directory in [&project.0, &project.directory()] {
        for mode in [
            "check", "run", "build", "emit", "lib", "hashes", "registry", "verify", "audit", "wasm",
        ] {
            let output = run(directory, &[mode]);
            assert_eq!(output.status.code(), Some(1), "{mode}: {output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("requires a source filename"), "{stderr}");
            assert!(stderr.contains("src/main.runa"), "{stderr}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    }
}

#[test]
fn source_commands_explain_directory_arguments() {
    let project = Project::new();
    for mode in ["check", "run", "build"] {
        let output = run(&project.directory(), &[mode, "."]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("source file, not a directory"), "{stderr}");
        assert!(stderr.contains("src/main.runa"), "{stderr}");
    }
    for args in [&["check", "--json"][..], &["check", "--json", "."][..]] {
        let output = run(&project.directory(), args);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["phase"], "usage", "{report}");
        assert_eq!(report["backend_validated"], false, "{report}");
    }
}

#[test]
fn explicit_source_and_intentional_repl_remain_available() {
    let project = Project::new();
    let directory = project.directory();
    for args in [
        &["check", "--frontend", "src/main.runa"][..],
        &["src/main.runa"][..],
    ] {
        let output = run(&directory, args);
        assert!(output.status.success(), "{output:?}");
    }
    let repl = run(&directory, &[]);
    assert!(repl.status.success(), "{repl:?}");
    let formatting = run(&directory, &["fmt", "--check", "."]);
    assert!(formatting.status.success(), "{formatting:?}");
}
