use futuruna::eval_source_with_prelude;
use serde_json::{json, Value};

const SOURCE: &str = include_str!("differential/corpus/json_object_values.runa");

fn assert_json_values(output: &str) {
    let actual = output
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap_or_else(|error| {
                panic!("json_object emitted invalid JSON: {line:?}: {error}")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            json!({"k\"ey": 1}),
            json!({"note": "line1\nline2"}),
            json!({"title": "[draft] Lov om skat"}),
            json!({"quote": "\"Loven\" gælder"}),
            json!({"amount": "NaN"}),
            json!({"amount": "inf"}),
            json!({"amount": "+5"}),
            json!({"amount": ".5"}),
            json!({"tab": "a\tb"}),
            json!({"slash\\key": "a\\b\nnext"}),
            json!({"city": "Copenhagen", "temp": 22}),
            json!({"nested": {"active": true}, "items": [1,2], "empty": null}),
            json!({"literal": "true", "flag": true, "identifier": "12345"}),
            json!({"number": 150.0, "unquoted": "ordinary text"}),
            json!({"key": 2}),
        ]
    );
}

#[test]
fn interpreted_json_objects_preserve_keys_text_and_explicit_json_values() {
    let output = eval_source_with_prelude(SOURCE, false).expect("interpret JSON fixture");
    assert_json_values(&output);
}

#[test]
fn native_json_objects_compile_and_preserve_the_same_values() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/json_object_values.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(fixture)
        .output()
        .expect("run native JSON fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_json_values(&String::from_utf8_lossy(&output.stdout));
}
