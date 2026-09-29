use std::path::Path;
use std::process::Command;

fn assert_modes(fixture: &str, expected: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus")
        .join(format!("{fixture}.runa"));
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command.arg(&path).output().unwrap();
        assert!(
            output.status.success(),
            "native={native}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "native={native}"
        );
    }
}

#[test]
fn exceptions_exclude_candidates_in_direct_enumerated_and_existential_queries() {
    assert_modes(
        "logic_exception_priority",
        "logic exception priority passed",
    );
}

#[test]
fn query_priority_preserves_order_correlations_and_effect_counts() {
    assert_modes("logic_priority_candidates", "guard ann\nguard bob\nbody bob\n[bob]\nguard ann\nvalue ann\n[ann]\nquery priority candidates passed");
}

#[test]
fn unbounded_positive_overrides_fail_without_partial_answers() {
    let path = std::env::temp_dir().join(format!(
        "futuruna-query-unbound-priority-{}.runa",
        std::process::id()
    ));
    for rule in [
        "| selected(0)\n| exception everyone selected(person: Int) -> True",
        "| selected(0)\n| exception positive selected(person: Int) -> True under person > 0",
        "> decision(person: Int) -> Bool { False }\n| selected(1)\n| exception unresolved selected(person: Int) -> decision(person) under person > 0",
    ] {
        std::fs::write(
            &path,
            format!("{rule}\n@ print(show(findall(person, selected(person))))\n"),
        )
        .unwrap();
        for native in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
            if native {
                command.arg("run");
            }
            let output = command.arg(&path).output().unwrap();
            assert!(!output.status.success(), "{rule}, native={native}");
            assert!(output.stdout.is_empty(), "{rule}, native={native}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("evaluation is incomplete"),
                "{rule}, native={native}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    std::fs::remove_file(path).unwrap();
}
