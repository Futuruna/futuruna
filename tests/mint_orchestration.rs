#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct GateFixture(PathBuf);

impl GateFixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "futuruna-mint-orchestration-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Self(fs::canonicalize(path).unwrap());
        for directory in ["bin", "scripts", "target/release"] {
            fs::create_dir_all(fixture.0.join(directory)).unwrap();
        }
        for (script, source) in [
            ("scripts/mint.sh", include_str!("../scripts/mint.sh")),
            (
                "scripts/step-runner.sh",
                include_str!("../scripts/step-runner.sh"),
            ),
        ] {
            fs::write(fixture.0.join(script), source).unwrap();
        }
        fixture.executable(
            "bin/cargo",
            r#"#!/bin/bash
printf 'tool:cargo %s model=%s\n' "$*" "${FUTURUNA_MODEL_TEST_RUNA:-}"
if [[ "$*" == 'build --release' && "${FAIL_RELEASE_BUILD:-}" == 1 ]]; then
    exit 42
fi
"#,
        );
        fixture.executable("target/release/runa", "#!/bin/bash\nexit 0\n");
        for script in [
            "first-run-canary.sh",
            "rust-interop-canary.sh",
            "from-rust-downstream-canary.sh",
            "from-rust-differential.sh",
            "compiler-cross-product-canary.sh",
            "docs-oracle.sh",
            "wasm-canary.sh",
        ] {
            fixture.executable(
                &format!("scripts/{script}"),
                r#"#!/bin/bash
printf 'tool:%s\n' "${0##*/}"
if [[ " ${FAIL_SCRIPTS:-} " == *" ${0##*/} "* ]]; then
    exit 3
fi
"#,
            );
        }
        fixture
    }

    fn executable(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        fs::write(&path, source).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn run(&self, fail_build: bool) -> Output {
        self.run_with(&[], fail_build, "")
    }

    fn run_with(&self, args: &[&str], fail_build: bool, fail_scripts: &str) -> Output {
        let mut paths = vec![self.0.join("bin")];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        Command::new("/bin/bash")
            .arg(self.0.join("scripts/mint.sh"))
            .args(args)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("FUTURUNA_MODEL_TEST_RUNA", "/stale/compiler")
            .env("FAIL_RELEASE_BUILD", if fail_build { "1" } else { "0" })
            .env("FAIL_SCRIPTS", fail_scripts)
            .env_remove("GITHUB_STEP_SUMMARY")
            .output()
            .unwrap()
    }
}

impl Drop for GateFixture {
    fn drop(&mut self) {
        // This uniquely named directory and all contents were created above.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn mint_builds_before_tests_and_overrides_stale_model_compiler() {
    let fixture = GateFixture::new();
    let output = fixture.run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let build = stdout.find("tool:cargo build --release").unwrap();
    let pinned = fixture.0.join(Path::new("target/release/runa"));
    let tests = stdout
        .find(&format!(
            "tool:cargo test --quiet model={}",
            pinned.display()
        ))
        .unwrap();
    assert!(build < tests, "{stdout}");
    assert_eq!(stdout.matches("tool:cargo build --release").count(), 1);
    assert!(stdout.contains("[mint] Futuruna is mint."));
}

#[test]
fn mint_never_runs_tests_after_failed_release_build() {
    let fixture = GateFixture::new();
    let output = fixture.run(true);
    assert_eq!(output.status.code(), Some(42));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("tool:cargo build --release"));
    assert!(!stdout.contains("cargo test"));
    assert!(!stdout.contains("[mint] Futuruna is mint."));
}

#[test]
fn mint_stops_at_the_first_failed_step_by_default() {
    let fixture = GateFixture::new();
    let output = fixture.run_with(&[], false, "first-run-canary.sh");
    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("tool:first-run-canary.sh"), "{stdout}");
    assert!(!stdout.contains("tool:rust-interop-canary.sh"), "{stdout}");
    assert!(!stdout.contains("[mint] Futuruna is mint."));
}

#[test]
fn mint_keep_going_runs_every_selected_step_and_lists_each_failure() {
    let fixture = GateFixture::new();
    let output = fixture.run_with(
        &["--keep-going", "--group", "checks"],
        false,
        "first-run-canary.sh wasm-canary.sh",
    );
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("tool:cargo build --release").count(), 1);
    for script in [
        "first-run-canary.sh",
        "rust-interop-canary.sh",
        "from-rust-downstream-canary.sh",
        "from-rust-differential.sh",
        "compiler-cross-product-canary.sh",
        "docs-oracle.sh",
        "wasm-canary.sh",
    ] {
        assert!(stdout.contains(&format!("tool:{script}")), "{stdout}");
    }
    assert!(!stdout.contains("tool:cargo test"), "{stdout}");
    assert!(!stdout.contains("test --run"), "{stdout}");
    assert!(stdout.contains("[mint] 2 of 17 steps failed:"), "{stdout}");
    assert!(
        stdout.contains("[mint]   exit 3 [checks] ./scripts/first-run-canary.sh"),
        "{stdout}"
    );
    assert!(
        stdout.contains("[mint]   exit 3 [checks] ./scripts/wasm-canary.sh"),
        "{stdout}"
    );
    assert!(!stdout.contains("[mint] Futuruna is mint."));
}

#[test]
fn mint_keep_going_never_runs_steps_after_failed_release_build() {
    let fixture = GateFixture::new();
    let output = fixture.run_with(&["--keep-going"], true, "");
    assert_eq!(output.status.code(), Some(42));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("cargo test"));
    assert!(!stdout.contains("tool:first-run-canary.sh"));
}

#[test]
fn mint_groups_partition_the_full_gate_and_list_without_running() {
    let fixture = GateFixture::new();
    let output = fixture.run_with(&["--list"], false, "");
    assert!(output.status.success());
    let all = String::from_utf8(output.stdout).unwrap();
    assert!(!all.contains("tool:"), "{all}");
    assert!(all.lines().next().unwrap().starts_with("(always)"), "{all}");
    let steps: Vec<&str> = all.lines().skip(1).collect();
    assert_eq!(steps.len(), 21, "{all}");

    let mut covered = 0;
    for group in ["rust", "checks", "interpreter", "codegen", "roundtrip"] {
        let output = fixture.run_with(&["--list", "--group", group], false, "");
        assert!(output.status.success());
        let listed = String::from_utf8(output.stdout).unwrap();
        for line in listed.lines().skip(1) {
            assert!(line.starts_with(group), "{line}");
            assert!(steps.contains(&line), "{line}");
            covered += 1;
        }
    }
    assert_eq!(covered, steps.len());

    let unknown = fixture.run_with(&["--group", "nope"], false, "");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(!String::from_utf8(unknown.stdout).unwrap().contains("tool:"));
}
