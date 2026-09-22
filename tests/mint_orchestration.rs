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
        fs::write(
            fixture.0.join("scripts/mint.sh"),
            include_str!("../scripts/mint.sh"),
        )
        .unwrap();
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
            "storage-canary.sh",
            "wasm-canary.sh",
        ] {
            fixture.executable(&format!("scripts/{script}"), "#!/bin/bash\nexit 0\n");
        }
        fixture
    }

    fn executable(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        fs::write(&path, source).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn run(&self, fail_build: bool) -> Output {
        let mut paths = vec![self.0.join("bin")];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        Command::new("/bin/bash")
            .arg(self.0.join("scripts/mint.sh"))
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("FUTURUNA_MODEL_TEST_RUNA", "/stale/compiler")
            .env("FAIL_RELEASE_BUILD", if fail_build { "1" } else { "0" })
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
