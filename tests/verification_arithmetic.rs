use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

use futuruna::eval_source_with_prelude;
use std::process::Command;

fn verify_with_status(source: &str, expected_success: bool) -> String {
    let path = std::env::temp_dir().join(format!(
        "futuruna-proof-arithmetic-{}-{}-{}.runa",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, source).unwrap();
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let output = Command::new(runa)
        .arg("verify")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        output.status.code(),
        Some(if expected_success { 0 } else { 1 }),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn z3_available() -> bool {
    Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn verify_unproved(source: &str) -> String {
    verify_with_status(source, false)
}

fn verify_proved_by_solver(source: &str) -> String {
    verify_with_status(source, z3_available())
}

fn verify_proved_by_kernel(source: &str) -> String {
    verify_with_status(source, true)
}

fn assert_solver_result(output: &str, expected: &str) {
    if Command::new("z3").arg("--version").output().is_ok() {
        assert!(output.contains(expected), "{output}");
    } else {
        // Linux CI installs Z3. Other environments must explicitly report the
        // unavailable solver; absence never counts as a successful proof.
        assert!(output.contains("Z3 not found"), "{output}");
        assert!(!output.contains("PROVED"), "{output}");
    }
}

#[test]
fn verify_does_not_prove_undefined_integer_arithmetic() {
    for expression in [
        "maximum + 1 > maximum",
        "minimum - 1 < minimum",
        "maximum * 2 == maximum * 2",
        "-minimum == -minimum",
        "minimum / -1 == minimum / -1",
        "minimum % -1 == 0",
        "1 / 0 == 1 / 0",
        "1 % 0 == 0",
    ] {
        let bindings = "= maximum = 9223372036854775807\n= minimum = -maximum - 1\n";
        let runtime = format!("{bindings}@ print(show({expression}))\n");
        assert!(
            eval_source_with_prelude(&runtime, false).is_err(),
            "{expression}"
        );
        let output = verify_unproved(&format!("{bindings}| claim: maximum -> {expression}\n"));
        assert!(!output.contains("PROVED"), "{expression}: {output}");
        assert_solver_result(&output, "COUNTEREXAMPLE found for |claim|");
    }
}

#[test]
fn verify_division_and_remainder_agree_with_runtime_signs() {
    for (expression, expected) in [
        ("-7 / 2 == -3", true),
        ("7 / -2 == -3", true),
        ("-7 / -2 == 3", true),
        ("-7 % 2 == -1", true),
        ("7 % -2 == 1", true),
        ("-7 % -2 == -1", true),
        ("-7 % 2 >= 0", false),
        ("-7 / 2 == -4", false),
    ] {
        let runtime =
            eval_source_with_prelude(&format!("@ print(show({expression}))\n"), false).unwrap();
        assert_eq!(runtime.trim(), expected.to_string(), "{expression}");
        let output = verify_with_status(
            &format!("| claim: 0 -> {expression}\n"),
            expected && z3_available(),
        );
        assert_solver_result(
            &output,
            if expected {
                "PROVED: |claim|"
            } else {
                "COUNTEREXAMPLE found for |claim|"
            },
        );
    }
}

#[test]
fn verify_refuses_real_number_substitution_for_float_arithmetic() {
    for (binding, expression) in [
        ("= f = 0.1", "(f + 0.2) + 0.3 == f + (0.2 + 0.3)"),
        (
            "= f = 10000000000000000.0",
            "(f + (0.0 - f)) + 1.0 == f + ((0.0 - f) + 1.0)",
        ),
    ] {
        assert_eq!(
            eval_source_with_prelude(&format!("{binding}\n@ print(show({expression}))\n"), false)
                .unwrap()
                .trim(),
            "false"
        );
        let output = verify_unproved(&format!("{binding}\n| claim: f -> {expression}\n"));
        assert!(!output.contains("PROVED"), "{output}");
        assert!(
            output.contains("Float verification requires runtime floating-point semantics"),
            "{output}"
        );
    }
    for source in [
        "> same(x: Float) -> Bool { x == x }\n= f = 0.1\n| claim: f -> same(f)\n",
        "# FloatingBox = FloatingBox(Float)\n= b = FloatingBox(0.1)\n| claim: b -> b == b\n? claim by { | b -> refl }\n",
    ] {
        let output = verify_unproved(source);
        assert!(!output.contains("PROVED"), "{output}");
        assert!(output.contains("Float verification requires runtime floating-point semantics"), "{output}");
    }
}

#[test]
fn explicit_kernel_proof_cannot_bypass_runtime_arithmetic_checks() {
    let output = verify_unproved("= big = 9223372036854775807\n| claim: big -> big < big + 1\n? claim by { | x -> apply int_ord.le_refl }\n");
    assert!(!output.contains("PROVED"), "{output}");
    assert_solver_result(&output, "COUNTEREXAMPLE found for |claim|");
    let output =
        verify_unproved("| claim: x -> x < x + 1\n? claim by { | x -> apply int_ord.le_refl }\n");
    assert!(!output.contains("PROVED"), "{output}");
    assert_solver_result(&output, "COUNTEREXAMPLE found for |claim|");
    let output = verify_proved_by_kernel("= big = 9223372036854775807\n| claim: big -> big <= big\n? claim by { | x -> apply int_ord.le_refl }\n");
    assert!(output.contains("PROVED by kernel"), "{output}");
}

#[test]
fn definedness_follows_short_circuit_and_selected_branches() {
    for expression in [
        "True || 1 / 0 == 0",
        "if False { 1 / 0 == 0 } else { True }",
        "!(False && 1 / 0 == 0)",
        "if x < 9223372036854775807 { x + 1 > x } else { True }",
        "x == 0 || x / 2 <= x / 2",
        "x >= -9223372036854775807 - 1 && x <= 9223372036854775807",
    ] {
        let output = verify_proved_by_solver(&format!("| claim: x -> {expression}\n"));
        assert_solver_result(&output, "PROVED: |claim|");
    }
}

#[test]
fn helper_calls_and_unused_local_values_preserve_arithmetic_failures() {
    for source in [
        "> inc(x: Int) -> Int { x + 1 }\n| claim: x -> inc(x) == inc(x)\n",
        "> bad(x: Int) -> Int { = ignored = x + 1\n 0 }\n| claim: x -> bad(x) == 0\n",
        "> bad(x: Int) -> Int { x + 1\n 0 }\n| claim: x -> bad(x) == 0\n",
        "= bad = 9223372036854775807 + 1\n| claim: bad -> bad == bad\n",
        "> bad(x: Int) -> Int { x + 1 }\n> outer(x: Int) -> Int { bad(x) }\n| claim: x -> outer(x) == outer(x)\n? claim by { | x -> refl }\n",
    ] {
        let output = verify_unproved(source);
        assert!(!output.contains("PROVED"), "{output}");
        assert_solver_result(&output, "COUNTEREXAMPLE found for |claim|");
    }
    let output = verify_proved_by_solver("> safe(x: Int) -> Int { if x < 9223372036854775807 { x + 1 } else { x } }\n| claim: x -> safe(x) >= x\n");
    assert_solver_result(&output, "PROVED: |claim|");
}

#[test]
fn opaque_builtin_arithmetic_cannot_be_certified_by_reflexivity() {
    for expression in [
        "abs(-9223372036854775807 - 1)",
        "sum_list([9223372036854775807, 1])",
    ] {
        assert!(eval_source_with_prelude(&format!("= value = {expression}\n"), false).is_err());
        let output = verify_unproved(&format!("= value = {expression}\n| claim: value -> value == value\n? claim by {{ | x -> refl }}\n"));
        assert!(!output.contains("PROVED"), "{output}");
        assert!(output.contains("SMT fallback skipped"), "{output}");
    }
}

#[test]
fn match_patterns_guards_and_field_values_select_the_same_arithmetic_paths() {
    let functions = r#"
# Packet = Packet(Int, Bool)
# Wrapped = Wrapped(Packet)
> route(p: Wrapped) -> Int {
    match p {
        | Wrapped(Packet(0, True)) -> 0
        | Wrapped(Packet(n, True)) -> 10 / n
        | Wrapped(Packet(n, False)) -> n
    }
}
"#;
    for (value, result) in [
        ("Wrapped(Packet(0, True))", "0"),
        ("Wrapped(Packet(2, True))", "5"),
        ("Wrapped(Packet(0, False))", "0"),
    ] {
        let source = format!("{functions}\n= p = {value}\n");
        assert_eq!(
            eval_source_with_prelude(&format!("{source}@ print(show(route(p)))\n"), false)
                .unwrap()
                .trim(),
            result
        );
        let output =
            verify_proved_by_solver(&format!("{source}| claim: p -> route(p) == {result}\n"));
        assert_solver_result(&output, "PROVED: |claim|");
    }
    let functions = r#"
# Packet = Packet(Int)
= n = 1
> route(p: Packet) -> Int {
    match p {
        | Packet(n) if False -> 1 / n
        | _ -> 1 / n
    }
}
"#;
    let source = format!("{functions}\n= p = Packet(0)\n");
    assert_eq!(
        eval_source_with_prelude(&format!("{source}@ print(show(route(p)))\n"), false)
            .unwrap()
            .trim(),
        "1"
    );
    assert_solver_result(
        &verify_proved_by_solver(&format!("{source}| claim: p -> route(p) == 1\n")),
        "PROVED: |claim|",
    );

    let source = "# Packet = Packet(Int)\n> route(p: Packet) -> Int { match p { | Packet(n) if 1 / n > 0 -> 1 | _ -> 0 } }\n= p = Packet(0)\n";
    assert!(
        eval_source_with_prelude(&format!("{source}@ print(show(route(p)))\n"), false).is_err()
    );
    assert_solver_result(
        &verify_unproved(&format!("{source}| claim: p -> route(p) == route(p)\n")),
        "COUNTEREXAMPLE found for |claim|",
    );
}

#[test]
fn a_failed_arithmetic_proof_does_not_become_a_trusted_local_lemma() {
    let output = verify_unproved(
        r#"
| bad: x -> x < x + 1
? bad by { | x -> apply int_ord.le_refl }
| copied: x -> x < x + 1
? copied by { | x -> apply bad }
"#,
    );
    assert!(!output.contains("PROVED"), "{output}");
    assert_solver_result(&output, "COUNTEREXAMPLE found for |bad|");
    assert_solver_result(&output, "COUNTEREXAMPLE found for |copied|");
}

#[test]
fn a_mathematical_lemma_is_available_after_its_arithmetic_check_passes() {
    let output = verify_proved_by_solver(
        r#"
| zero: x -> 0 + x == x
? zero by { | x -> apply int_ring.zero_add }
| reversed: x -> x == 0 + x
? reversed by { | x -> apply eq.sym(apply zero) }
"#,
    );
    assert_solver_result(&output, "PROVED: |zero|");
    assert_solver_result(&output, "PROVED: |reversed|");
    if Command::new("z3").arg("--version").output().is_ok() {
        assert_eq!(
            output
                .matches("explicit mathematical proof checked")
                .count(),
            2,
            "{output}"
        );
    }
}
