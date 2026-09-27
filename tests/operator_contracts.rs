use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "futuruna-operator-contract-{}-{}.runa",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn rejected_before_effects(source: &str, diagnostic: &str, types: &[&str]) {
    let fixture = Fixture::new(source);
    for args in [
        vec![],
        vec!["run"],
        vec!["check", "--frontend"],
        vec!["check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(&args)
            .arg(&fixture.0)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert!(stderr.contains(diagnostic), "{args:?}: {stderr}");
        assert!(
            stderr.contains(&format!("{}:2:", fixture.0.display())),
            "source position missing: {stderr}"
        );
        assert!(
            !stderr.contains("error[E"),
            "Rust diagnostic leaked: {stderr}"
        );
        for ty in types {
            assert!(stderr.contains(ty), "missing {ty}: {stderr}");
        }
    }
}

#[test]
fn invalid_primitive_operators_fail_in_all_modes_before_effects() {
    for (expression, operator, types) in [
        ("5 == 5.0", "==", vec!["Int", "Float", "to_float"]),
        ("5.0 != 5", "!=", vec!["Float", "Int", "to_float"]),
        ("(1 + 2.0) == 3", "==", vec!["Float", "Int", "to_float"]),
        ("1 == \"1\"", "==", vec!["Int", "String"]),
        ("\"17\" >= 18", ">=", vec!["String", "Int"]),
        ("\"ab\" * 3", "*", vec!["String", "Int"]),
        ("True + True", "+", vec!["Bool"]),
        ("'a' - 'b'", "-", vec!["Char"]),
        ("True < False", "<", vec!["Bool"]),
        ("True && 1", "&&", vec!["Bool", "Int"]),
        ("0 || False", "||", vec!["Int", "Bool"]),
        ("5 % 2.0", "%", vec!["Int", "Float", "to_float"]),
        ("[1] * 2", "*", vec!["List", "Int"]),
        ("!3", "!", vec!["Int"]),
        ("-True", "-", vec!["Bool"]),
    ] {
        rejected_before_effects(
            &format!("@ print(\"must not execute\")\n= result = {expression}\n"),
            &format!("unsupported operands for operator `{operator}`"),
            &types,
        );
    }
}

#[test]
fn typed_parameters_fields_and_locals_have_operator_diagnostics() {
    for (declaration, types) in [
        (
            "> bad(a: Int, b: String) -> Bool { a == b }",
            vec!["Int", "String"],
        ),
        ("> bad(a: Bool) -> Bool { = b = a; b + b }", vec!["Bool"]),
        (
            "# Input(value: String)\n> bad(input: Input) -> Bool { input.value >= 18 }",
            vec!["String", "Int"],
        ),
    ] {
        // Put the offending expression on line 2 even when a type is declared first.
        let source = if let Some((record, function)) = declaration.split_once('\n') {
            format!("{record}; @ print(\"must not execute\")\n{function}\n")
        } else {
            format!("@ print(\"must not execute\")\n{declaration}\n")
        };
        rejected_before_effects(&source, "unsupported operands for operator", &types);
    }
}

#[test]
fn known_non_boolean_conditions_fail_before_any_effect() {
    for (condition, ty) in [
        ("()", "()"),
        ("0", "Int"),
        ("\"yes\"", "String"),
        ("[]", "List"),
    ] {
        rejected_before_effects(
            &format!(
                "@ print(\"must not execute\")\n= result = if {condition} {{ 1 }} else {{ 2 }}\n"
            ),
            "if condition must return Bool",
            &[ty],
        );
    }
}

#[test]
fn supported_operators_agree_in_interpretation_and_native_execution() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/ordinary_operator_contracts.runa");
    let expected = "3.5\n-0.5\n3\n0.25\ntrue\ntrue\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\nvalue=3\n3 apples\ntrue";
    for args in [
        vec![],
        vec!["run"],
        vec!["check", "--frontend"],
        vec!["check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(&args)
            .arg(&fixture)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        if !args.contains(&"check") {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                expected,
                "{args:?}"
            );
        }
    }
}

#[test]
fn unannotated_parameters_shadow_outer_operand_types() {
    let fixture = Fixture::new(
        "= x = Some(42)\n\
         > apply(f: Int -> Int, value: Int) -> Int { f(value) }\n\
         = answer = apply(|x| x * 2, 21)\n\
         @ print(show(answer))\n\
         @ print(show(x == Some(42)))\n",
    );
    for args in [
        vec![],
        vec!["run"],
        vec!["check", "--frontend"],
        vec!["check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(&args)
            .arg(&fixture.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        if !args.contains(&"check") {
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42\ntrue");
        }
    }
}
