use futuruna::eval_source_with_prelude;
use std::process::Command;

const SOURCE: &str = include_str!("differential/corpus/float_formatting.runa");
const EXPECTED: &str = "3.14\n3\n7.000\n65537\nvalue\nprecision\n3.14";
const DIAGNOSTIC: &str = "format_float precision must be between 0 and 65535";
const INVALID: &[&str] = &[
    "-1",
    "(-9223372036854775807 - 1)",
    "65536",
    "9223372036854775807",
];

#[test]
fn interpreted_formatting_rejects_out_of_range_precision() {
    for precision in INVALID {
        for value in ["3.14159", "7"] {
            let source = format!("@ print(format_float({value}, {precision}))\n");
            let error =
                eval_source_with_prelude(&source, false).expect_err("invalid precision must fail");
            assert!(error.contains(DIAGNOSTIC), "{source}: {error}");
        }
    }
}

#[test]
fn native_formatting_rejects_out_of_range_precision_after_successful_compilation() {
    let input = std::env::temp_dir().join(format!(
        "futuruna-float-precision-{}-{}.runa",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for precision in INVALID {
        let source = format!("@ print(format_float(3.14159, {precision}))\n");
        std::fs::write(&input, source).unwrap();
        let checked = Command::new(env!("CARGO_BIN_EXE_runa"))
            .arg("check")
            .arg(&input)
            .output()
            .expect("compile formatting regression");
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .arg("run")
            .arg(&input)
            .output()
            .expect("execute formatting regression");
        assert!(
            !output.status.success(),
            "{precision}: invalid precision succeeded"
        );
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(DIAGNOSTIC), "{precision}: {error}");
    }
    std::fs::remove_file(input).unwrap();
}

#[test]
fn valid_formatting_boundaries_and_argument_effects_agree_in_both_backends() {
    let interpreted = eval_source_with_prelude(SOURCE, false).expect("interpret valid formatting");
    assert_eq!(interpreted.trim(), EXPECTED);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/float_formatting.runa");
    let native = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .expect("compile and run valid formatting");
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout).trim(), EXPECTED);
}
