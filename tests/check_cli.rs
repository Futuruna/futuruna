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
