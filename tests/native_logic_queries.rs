use std::path::Path;
use std::process::Command;

#[test]
fn anonymous_logic_queries_compile_and_execute_with_independent_wildcards() {
    assert_both_backends(
        "logic_wildcard_independence",
        "anonymous logic queries passed",
    );
}

#[test]
fn signed_and_list_rule_heads_compile_and_match_exact_values() {
    assert_both_backends(
        "logic_signed_list_heads",
        "signed and list rule heads passed",
    );
}

#[test]
fn ground_rule_alternatives_preserve_boolean_operators_and_short_circuiting() {
    assert_both_backends(
        "logic_ground_alternatives",
        "ground rule alternatives passed",
    );
}

fn assert_both_backends(name: &str, expected: &str) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/differential/corpus/{name}.runa"));
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command
            .arg(&fixture)
            .output()
            .expect("execute rule regression");
        assert!(
            output.status.success(),
            "{name} (native={native}) failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
}
