use std::process::Command;

#[test]
fn version_matches_package_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("--version")
        .output()
        .expect("runa --version should execute");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output should be UTF-8"),
        format!("runa {}\n", env!("CARGO_PKG_VERSION"))
    );
}
