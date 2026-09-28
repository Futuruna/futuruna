#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str, solver: Option<&str>) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "futuruna-verify-status-{}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("claim.runa"), source).unwrap();
        if let Some(solver) = solver {
            let path = directory.join("z3");
            std::fs::write(&path, solver).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self(directory)
    }

    fn verify(&self) -> Output {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(runa)
            .arg("verify")
            .arg(self.0.join("claim.runa"))
            // Isolate only this child process's solver lookup. No global PATH
            // mutation and no dependency on an installed solver for these tests.
            .env("PATH", &self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(self.0.join("claim.runa")).unwrap();
        if self.0.join("z3").exists() {
            std::fs::remove_file(self.0.join("z3")).unwrap();
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

// The fixtures emulate the solver protocol, not the truth of source claims.
// Semantic agreement with real Z3 is covered in verification_arithmetic.rs.
const UNSAT: &str = "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unsat\\n'\n";
const SAT: &str = "#!/bin/sh\nmodel=0\nwhile IFS= read -r line; do\n  if [ \"$line\" = '(get-model)' ]; then model=1; fi\ndone\nprintf 'sat\\n'\nif [ \"$model\" = 1 ]; then printf '(model)\\n'; fi\n";
const TRUE_CLAIM: &str = "| claim: x -> x == x\n";

fn assert_failed(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains(expected) || stderr.contains(expected),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn counterexamples_and_partially_verified_files_fail() {
    for source in [
        "= x = 5\n| small: x -> x < 3\n",
        "| valid: x -> x == x\n= x = 5\n| small: x -> x < 3\n",
        "= x = 5\n| small: x -> x < 3\n| valid: x -> x == x\n",
    ] {
        let output = Fixture::new(source, Some(SAT)).verify();
        assert_failed(&output, "COUNTEREXAMPLE found for |small|");
    }
}

#[test]
fn solver_errors_unknown_results_and_missing_solver_fail() {
    for solver in [
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unknown\\n'\n",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'solver broke\\n' >&2\nexit 7\n",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unsat\\n'\nexit 7\n",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unsat\\n(error \"invalid command\")\\n'\n",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unsat\\n'\nprintf 'solver error\\n' >&2\n",
        "#!/bin/sh\n/bin/cat >/dev/null\n",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'unexpected response\\n'\n",
    ] {
        let output = Fixture::new(TRUE_CLAIM, Some(solver)).verify();
        assert_failed(&output, "Z3");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("PROVED:"));
    }
    let output = Fixture::new(TRUE_CLAIM, None).verify();
    assert_failed(&output, "Z3 not found");
}

#[test]
fn unsupported_verification_and_empty_claim_sets_fail() {
    let output = Fixture::new("| floating: 0.1 -> 0.1 == 0.1\n", Some(UNSAT)).verify();
    assert_failed(&output, "unsupported");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PROVED"));
    let output = Fixture::new("= value = 1\n", Some(UNSAT)).verify();
    assert_failed(&output, "no invariants found");
}

#[test]
fn valid_solver_proofs_succeed() {
    // Requesting a model after unsat is a solver error, not a valid success
    // protocol. The verifier should request a model only for a counterexample.
    let solver = "#!/bin/sh\nmodel=0\nwhile IFS= read -r line; do\n  if [ \"$line\" = '(get-model)' ]; then model=1; fi\ndone\nif [ \"$model\" = 1 ]; then\n  printf '(error \"model is unavailable\")\\n'\n  exit 1\nfi\nprintf 'unsat\\n'\n";
    let output = Fixture::new(TRUE_CLAIM, Some(solver)).verify();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("PROVED: |claim|"));
}

#[test]
fn proof_term_blocks_are_rejected_before_solving() {
    let output = Fixture::new(
        "| identity: x -> x == x\n? identity by { | x -> refl }\n",
        Some(UNSAT),
    )
    .verify();
    assert_failed(&output, "2:12");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PROVED"));
}
