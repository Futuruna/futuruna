#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(dependency: bool) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "futuruna-missing-toolchain-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let bin = directory.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let which = bin.join("which");
        // Override discovery only in the child process. The reported tool path
        // does not exist, so no Rust installation, dependency or global cache
        // is changed and the OS produces a real launch failure.
        std::fs::write(
            &which,
            "#!/bin/sh\nprintf '%s/missing/%s\\n' \"$PWD\" \"$1\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&which, std::fs::Permissions::from_mode(0o755)).unwrap();
        let source = if dependency {
            "@ depend \"sha2\" \"0.10\"\n@ print(7)\n"
        } else {
            "@ print(7)\n"
        };
        std::fs::write(directory.join("main.runa"), source).unwrap();
        Self(directory)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(args)
            .arg("main.runa")
            .current_dir(&self.0)
            .env("PATH", self.0.join("bin"))
            .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn assert_guidance(message: &str, tool: &str, operation: &str) {
    assert!(
        message.contains(&format!("Cannot launch `{tool}` for `runa {operation}`")),
        "{message}"
    );
    for expected in [
        "https://rustup.rs",
        "runa check --frontend <file.runa>",
        "runa <file.runa>",
        "does not validate generated Rust",
    ] {
        assert!(message.contains(expected), "{message}");
    }
    assert!(!message.contains("compiler bug"), "{message}");
}

#[test]
fn missing_native_tools_report_command_specific_alternatives() {
    for (dependency, tool) in [(false, "rustc"), (true, "cargo")] {
        let fixture = Fixture::new(dependency);
        for operation in ["check", "run", "build"] {
            let output = if dependency && operation == "check" {
                fixture.run(&[operation, "--build-deps"])
            } else {
                fixture.run(&[operation])
            };
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert_guidance(&String::from_utf8_lossy(&output.stderr), tool, operation);
        }
    }
}

#[test]
fn missing_toolchain_check_json_keeps_the_backend_failure_contract() {
    for (dependency, tool) in [(false, "rustc"), (true, "cargo")] {
        let fixture = Fixture::new(dependency);
        let output = fixture.run(&["check", "--json", "--build-deps"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["phase"], "backend", "{report}");
        assert_eq!(report["backend_validated"], false, "{report}");
        assert_guidance(
            report["diagnostics"][0]["message"].as_str().unwrap(),
            tool,
            "check",
        );
    }
}

#[test]
fn suggested_frontend_and_interpreter_paths_work_without_native_tools() {
    let fixture = Fixture::new(false);
    let frontend = fixture.run(&["check", "--frontend"]);
    assert!(frontend.status.success(), "{frontend:?}");
    assert!(String::from_utf8_lossy(&frontend.stderr).contains("Rust backend not validated"));
    let interpreted = fixture.run(&[]);
    assert!(interpreted.status.success(), "{interpreted:?}");
    assert_eq!(interpreted.stdout, b"7\n");
}
