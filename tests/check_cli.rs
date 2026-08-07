use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_workspace() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("futuruna-check-cli-{}-{nonce}", std::process::id()))
}

#[test]
fn concurrent_checks_do_not_share_generated_rust_files() {
    let workspace = test_workspace();
    fs::create_dir_all(&workspace).expect("create check CLI workspace");

    let valid = workspace.join("valid.runa");
    let invalid = workspace.join("invalid.runa");
    fs::write(
        &valid,
        "# Answer(value: Int)\n| answer(value: Int) -> Answer(value = value)\n= result = answer(42)\n",
    )
    .expect("write valid fixture");
    fs::write(
        &invalid,
        "@ rust { compile_error!(\"intentional check isolation fixture\"); }\n",
    )
    .expect("write invalid fixture");

    let runa = env!("CARGO_BIN_EXE_runa");
    let mut children: Vec<(bool, Child)> = Vec::new();
    for _ in 0..8 {
        children.push((
            true,
            Command::new(runa)
                .arg("check")
                .arg(&valid)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn valid check"),
        ));
        children.push((
            false,
            Command::new(runa)
                .arg("check")
                .arg(&invalid)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn invalid check"),
        ));
    }

    for (should_succeed, mut child) in children {
        let status = child.wait().expect("wait for check process");
        assert_eq!(
            status.success(),
            should_succeed,
            "concurrent check observed another process's generated source"
        );
    }

    fs::remove_dir_all(&workspace).expect("remove check CLI workspace");
}

#[test]
fn frontend_check_succeeds_without_rust_toolchain() {
    let workspace = test_workspace();
    fs::create_dir_all(&workspace).expect("create frontend check workspace");
    let empty_path = workspace.join("empty-path");
    let empty_home = workspace.join("empty-home");
    fs::create_dir_all(&empty_path).expect("create empty PATH directory");
    fs::create_dir_all(&empty_home).expect("create empty HOME directory");
    let source = workspace.join("frontend.runa");
    fs::write(
        &source,
        "# Answer(value: Int)\n| answer(value: Int) -> Answer(value = value)\n= result = answer(42)\n",
    )
    .expect("write frontend check fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["check", "--frontend"])
        .arg(&source)
        .env("PATH", &empty_path)
        .env("HOME", &empty_home)
        .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
        .output()
        .expect("run frontend check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("frontend check ok"));
    assert!(stderr.contains("Rust backend not validated"));
    assert!(!stderr.contains("lines of Rust"));

    fs::remove_dir_all(&workspace).expect("remove frontend check workspace");
}

#[test]
fn frontend_check_includes_compiler_validation() {
    let workspace = test_workspace();
    fs::create_dir_all(&workspace).expect("create frontend validation workspace");
    let source = workspace.join("invalid-named-argument.runa");
    fs::write(
        &source,
        "> taxable(base: Int, active: Bool) -> Int {\n    if active { base } else { 0 }\n}\n\n= bad = taxable(missing = 1, active = True)\n",
    )
    .expect("write invalid frontend fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .args(["check", "--frontend"])
        .arg(&source)
        .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
        .output()
        .expect("run invalid frontend check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("has no parameter `missing`"), "{stderr}");
    assert!(!stderr.contains("frontend check ok"));

    fs::remove_dir_all(&workspace).expect("remove frontend validation workspace");
}
