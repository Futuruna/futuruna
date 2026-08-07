use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn runa() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_runa"))
}

fn temp_fixture_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "futuruna-semantic-interface-cli-{}",
        std::process::id()
    ))
}

fn run_interface(path: &PathBuf) -> std::process::Output {
    Command::new(runa())
        .args(["interface", "--no-prelude"])
        .arg(path)
        .output()
        .expect("run semantic interface command")
}

#[test]
fn semantic_interface_is_stable_across_processes_and_tracks_signatures() {
    let directory = temp_fixture_dir();
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create semantic interface fixture");
    let source_path = directory.join("model.runa");
    fs::write(
        &source_path,
        "# Input(amount: Int)\n| amount(input: Input) -> input.amount + 1\n",
    )
    .expect("write semantic interface fixture");

    let first = run_interface(&source_path);
    let second = run_interface(&source_path);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let first_json: Value = serde_json::from_slice(&first.stdout).expect("first graph JSON");

    fs::write(
        &source_path,
        "# Input(amount: Int)\n| amount(input: Input) -> input.amount + 2\n",
    )
    .expect("write body edit");
    let body_edit = run_interface(&source_path);
    assert!(body_edit.status.success());
    let body_json: Value = serde_json::from_slice(&body_edit.stdout).expect("body graph JSON");
    assert_eq!(
        first_json["root_dependency_hash"],
        body_json["root_dependency_hash"]
    );
    assert_ne!(
        first_json["modules"][0]["content_hash"],
        body_json["modules"][0]["content_hash"]
    );

    fs::write(
        &source_path,
        "# Input(amount: Int)\n| amount(facts: Input) -> facts.amount + 2\n",
    )
    .expect("write signature edit");
    let signature_edit = run_interface(&source_path);
    assert!(signature_edit.status.success());
    let signature_json: Value =
        serde_json::from_slice(&signature_edit.stdout).expect("signature graph JSON");
    assert_ne!(
        first_json["root_dependency_hash"],
        signature_json["root_dependency_hash"]
    );

    fs::remove_dir_all(&directory).expect("remove semantic interface fixture");
}
