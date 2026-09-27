use std::path::Path;
use std::process::Command;

#[test]
fn empty_list_comparisons_use_the_other_operands_element_type() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/contextual_empty_list_comparisons.runa");
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        if native {
            command.arg("run");
        }
        let output = command.arg(&source).output().unwrap();
        assert!(
            output.status.success(),
            "native={native}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "left once\nright once\ncontextual empty list comparisons passed\n",
            "native={native}"
        );
    }
}
