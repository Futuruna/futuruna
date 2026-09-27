use futuruna::eval_source_with_prelude;
use std::process::Command;

const HELPERS: &str = "\
> add(a: Int, b: Int) -> Int { a + b }
> subtract(a: Int, b: Int) -> Int { a - b }
> multiply(a: Int, b: Int) -> Int { a * b }
> divide(a: Int, b: Int) -> Int { a / b }
> remainder(a: Int, b: Int) -> Int { a % b }
> negate(a: Int) -> Int { -a }
> magnitude(a: Int) -> Int { abs(a) }
> add_n(n: Int) -> (Int -> Int) { |x| x + n }
= increment = add_n(1)
= maximum = 9223372036854775807
= minimum = -maximum - 1
";

const INVALID: &[(&str, &str)] = &[
    ("add(maximum, 1)", "integer addition overflow"),
    ("increment(maximum)", "integer addition overflow"),
    ("subtract(minimum, 1)", "integer subtraction overflow"),
    (
        "multiply(4611686018427387904, 2)",
        "integer multiplication overflow",
    ),
    (
        "divide(minimum, -1)",
        "integer division by zero or overflow",
    ),
    (
        "remainder(minimum, -1)",
        "integer remainder by zero or overflow",
    ),
    ("divide(1000000, 0)", "integer division by zero or overflow"),
    (
        "remainder(1000000, 0)",
        "integer remainder by zero or overflow",
    ),
    ("negate(minimum)", "integer negation overflow"),
    ("magnitude(minimum)", "integer absolute-value overflow"),
    ("sum_list([maximum, 1])", "integer sum overflow"),
    ("sum(from_list([maximum, 1]))", "integer sum overflow"),
    (
        "sum_list(map([minimum], abs))",
        "integer absolute-value overflow",
    ),
];

#[test]
fn interpreter_rejects_undefined_integer_results_with_operation_diagnostics() {
    for (expression, diagnostic) in INVALID {
        let source = format!("{HELPERS}@ print(show({expression}))\n");
        let error =
            eval_source_with_prelude(&source, false).expect_err("undefined arithmetic must fail");
        assert!(error.contains(diagnostic), "{expression}: {error}");
    }
}

#[test]
fn cli_interpreter_rejects_undefined_integer_results_in_the_mint_release_build() {
    // Mint supplies the release compiler built from the current checkout.
    // Focused Cargo runs use their own compiler instead of a stale release.
    let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
    let directory = arithmetic_workspace();
    let input = directory.join("arithmetic.runa");
    for (expression, diagnostic) in INVALID {
        std::fs::write(&input, format!("{HELPERS}@ print(show({expression}))\n")).unwrap();
        let output = Command::new(&runa)
            .arg(&input)
            .output()
            .expect("run CLI interpreter");
        assert!(
            !output.status.success(),
            "{expression}: unexpectedly succeeded"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(diagnostic), "{expression}: {error}");
        assert!(
            output.stdout.is_empty(),
            "{expression}: emitted a fabricated value"
        );
    }
    std::fs::remove_file(input).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

fn arithmetic_workspace() -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "futuruna-integer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("temporary native arithmetic workspace");
    directory
}

// Compile emitted Rust with overflow checks explicitly disabled. This guards
// arithmetic semantics independently of Cargo's debug/release defaults, and
// separately requires successful code generation and native compilation.
fn optimized_native(source: &str) -> std::process::Output {
    let directory = arithmetic_workspace();
    let input = directory.join("arithmetic.runa");
    let rust = directory.join("arithmetic.rs");
    let binary = directory.join("arithmetic");
    std::fs::write(&input, source).expect("write arithmetic source");
    let emitted = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("emit")
        .arg(&input)
        .output()
        .expect("emit native arithmetic");
    assert!(
        emitted.status.success(),
        "{}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    std::fs::write(&rust, emitted.stdout).expect("write emitted Rust");
    let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg(&rust)
        .args([
            "--edition=2021",
            "-C",
            "opt-level=2",
            "-C",
            "overflow-checks=off",
            "-o",
        ])
        .arg(&binary)
        .output()
        .expect("compile native arithmetic");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&binary)
        .output()
        .expect("execute native arithmetic");
    for path in [input, rust, binary] {
        std::fs::remove_file(path).expect("remove generated arithmetic file");
    }
    std::fs::remove_dir(directory).expect("remove empty arithmetic workspace");
    output
}

#[test]
fn optimized_native_rejects_undefined_integer_results_with_operation_diagnostics() {
    for (expression, diagnostic) in INVALID {
        let source = format!("{HELPERS}@ print(show({expression}))\n");
        let output = optimized_native(&source);
        assert!(
            !output.status.success(),
            "{expression}: unexpectedly succeeded"
        );
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(diagnostic), "{expression}: {error}");
        assert!(
            output.stdout.is_empty(),
            "{expression}: emitted a fabricated value"
        );
    }
}

#[test]
fn integer_boundaries_and_signed_division_agree_in_both_backends() {
    let source = format!(
        "{HELPERS}\
@ print(show(add(maximum, 0)))
@ print(show(subtract(minimum, 0)))
@ print(show(multiply(minimum, 1)))
@ print(show(divide(-7, 3)))
@ print(show(remainder(-7, 3)))
@ print(show(divide(0, 2)))
@ print(show(remainder(0, 2)))
@ print(show(negate(-maximum)))
@ print(show(magnitude(-maximum)))
@ print(show(sum_list([maximum, -maximum])))
@ print(show(sum(from_list([maximum, -maximum]))))
@ print(show(sum_list([])))
@ print(show(sum(from_list([1.5, 2.5]))))
@ print(show(abs(7)))
@ print(show(abs(-7.5)))
@ print(show(sum_list(map([-3, 4], abs))))
@ print(show(increment(41)))
"
    );
    let expected = "9223372036854775807\n-9223372036854775808\n-9223372036854775808\n-2\n-1\n0\n0\n9223372036854775807\n9223372036854775807\n0\n0\n0\n4\n7\n7.5\n7\n42";
    let interpreted = eval_source_with_prelude(&source, false).expect("valid integer boundaries");
    assert_eq!(interpreted.trim(), expected);
    let native = optimized_native(&source);
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&native.stdout).trim(), expected);
}
