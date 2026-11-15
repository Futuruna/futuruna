//! `runa fmt --migrate` changes how a model is annotated, never what it
//! computes: a migrated copy of tests/fixtures/annotation-migration gives the
//! results the old-syntax model gave (expected.json, recorded before the
//! comment directives were retired) and accepts the old contract hash. The
//! recording predates per-result assumptions (0.4.1); each case now also
//! carries the assumption the fixture's section states.

use std::path::{Path, PathBuf};
use std::process::Command;

fn runa() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_runa"))
}

fn copy_directory(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create copy");
    for entry in std::fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

#[test]
fn a_migrated_model_computes_what_it_computed() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/annotation-migration");
    let copy = std::env::temp_dir().join(format!(
        "futuruna-annotation-migration-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&copy).ok();
    copy_directory(&fixture, &copy);

    let migrate = Command::new(runa())
        .args(["fmt", "--migrate", "."])
        .current_dir(&copy)
        .output()
        .expect("run runa fmt --migrate");
    let migrate_log = String::from_utf8_lossy(&migrate.stderr).into_owned();
    assert!(migrate.status.success(), "{migrate_log}");
    let model = std::fs::read_to_string(copy.join("model.calculate.runa")).unwrap();
    assert!(!model.contains("--@"), "{model}");
    assert!(model.contains("@ section tax {"), "{model}");
    assert!(model.contains("@ quote(Source(value = law))"), "{model}");
    let domain_meta = std::fs::read_to_string(copy.join("domain_meta.runa")).unwrap();
    assert!(
        domain_meta.contains("@ meta(input_meta) for Input"),
        "{domain_meta}"
    );

    let check = Command::new(runa())
        .args(["check", "--frontend", "model.calculate.runa"])
        .current_dir(&copy)
        .output()
        .expect("run runa check");
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );

    let call = Command::new(runa())
        .args(["call", "model.calculate.runa", "--input", "cases.json"])
        .current_dir(&copy)
        .output()
        .expect("run runa call");
    let expected = std::fs::read_to_string(fixture.join("expected.json")).unwrap();
    std::fs::remove_dir_all(&copy).ok();
    assert!(
        call.status.success(),
        "{}",
        String::from_utf8_lossy(&call.stderr)
    );
    let mut actual: serde_json::Value =
        serde_json::from_slice(&call.stdout).expect("runa call writes JSON");
    let expected: serde_json::Value =
        serde_json::from_str(&expected).expect("expected.json is JSON");
    let results = actual["results"].as_array_mut().expect("a results list");
    for case in results.iter_mut() {
        let assumptions = case
            .as_object_mut()
            .expect("a case is an object")
            .remove("assumptions")
            .expect("each case carries the section's assumption");
        assert_eq!(
            assumptions[0]["value"]["id"], "WHOLE-KRONER",
            "{assumptions}"
        );
    }
    assert_eq!(actual, expected);
}
