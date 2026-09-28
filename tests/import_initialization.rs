use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "futuruna-static-import-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        for (name, source) in files {
            std::fs::write(directory.join(name), source).unwrap();
        }
        Self(directory)
    }
    fn run(&self, args: &[&str]) -> Output {
        let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        Command::new(binary)
            .args(args)
            .arg(self.0.join("main.runa"))
            .output()
            .unwrap()
    }
    fn agrees(&self, expected: &str) {
        for args in [
            &[][..],
            &["run"][..],
            &["check", "--frontend"][..],
            &["check"][..],
        ] {
            let output = self.run(args);
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(&self.0).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn maintained_import_corpus_agrees_in_both_execution_modes() {
    let fixture = Fixture::new(&[
        (
            "main.runa",
            include_str!("differential/corpus/imports/static_import_initialization.runa"),
        ),
        (
            "static_initializer_base.runa",
            include_str!("differential/corpus/imports/static_initializer_base.runa"),
        ),
        (
            "static_initializer_bridge.runa",
            include_str!("differential/corpus/imports/static_initializer_bridge.runa"),
        ),
    ]);
    fixture.agrees("initialized\n6\n6\n6\nfalse\n[1]\n2");
}

#[test]
fn imported_initializers_and_later_calls_use_the_same_local_override() {
    for prefix in ["", "> anchor() -> Int { 0 }\n"] {
        let source = format!("{prefix}@ import ./dependency\n> fee(x: Int) -> Int {{ x + 1 }}\n@ print(show(seed))\n@ print(show(fee(5)))\n@ print(show(imported_total(5)))\n");
        let fixture = Fixture::new(&[
            ("dependency.runa", "> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n> imported_total(x: Int) -> Int { fee(x) }\n"),
            ("main.runa", &source),
        ]);
        fixture.agrees("6\n6\n6");
    }
}

#[test]
fn merged_initializer_dependencies_can_cross_the_import_boundary() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n"),
        ("main.runa", "@ import ./dependency\n> fee(x: Int) -> Int { x + adjustment }\n= adjustment = 1\n@ print(show(seed))\n"),
    ]);
    fixture.agrees("6");
}

#[test]
fn diamond_import_initializers_run_once_in_the_merged_context() {
    let fixture = Fixture::new(&[
        ("shared.runa", "> fee(x: Int) -> Int { x * 100 }\n> initialize() -> Int { @ print(\"initialized\"); fee(5) }\n= seed = initialize()\n@ print(\"library script\")\n"),
        ("left.runa", "@ import ./shared\n> left() -> Int { seed }\n"),
        ("right.runa", "@ import ./shared\n> right() -> Int { seed }\n"),
        ("main.runa", "@ import ./left\n@ import ./right\n> fee(x: Int) -> Int { x + 1 }\n@ print(show(left() + right()))\n"),
    ]);
    fixture.agrees("initialized\n12");
}

#[test]
fn imported_initializer_proofs_agree_with_execution() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n"),
        ("main.runa", "@ import ./dependency\n> fee(x: Int) -> Int { x + 1 }\n| correct: 0 -> seed == 6\n| wrong: 0 -> seed == 500\n? correct else { @ print(\"incorrect value\") }\n? wrong else { @ print(\"counterexample\") }\n"),
    ]);
    fixture.agrees("counterexample");
    if Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
    {
        let output = fixture.run(&["verify"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("PROVED: |correct|"), "{stdout}");
        assert!(!stdout.contains("PROVED: |wrong|"), "{stdout}");
        assert!(stdout.contains("COUNTEREXAMPLE"), "{stdout}");
    }
}

#[test]
fn imported_functions_override_injected_prelude_defaults() {
    let fixture = Fixture::new(&[
        (
            "dependency.runa",
            "> identity(x: Int) -> Int { x + 1 }\n= seed = identity(5)\n",
        ),
        (
            "main.runa",
            "@ import ./dependency\n@ print(show(seed))\n@ print(show(identity(5)))\n| imported_identity: 0 -> seed == 6\n",
        ),
    ]);
    fixture.agrees("6\n6");
    if Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        let output = fixture.run(&["verify"]);
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("PROVED: |imported_identity|"));
    }
}

#[test]
fn local_functions_before_imports_keep_their_result_types() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "> fee(x: Int) -> Int { x * 100 }\n"),
        ("main.runa", "> fee(x: Int) -> String { \"local\" }\n@ import ./dependency\n@ print(show(fee(5) == \"local\"))\n"),
    ]);
    fixture.agrees("true");
}

#[test]
fn plain_import_planning_keeps_qualified_initialization_isolated() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "@ export fee\n@ export seed\n> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n"),
        ("main.runa", "@ import ./dependency\n@ import Lib from ./dependency\n> fee(x: Int) -> Int { x + 1 }\n@ print(show(seed))\n@ print(show(Lib.seed))\n@ print(show(Lib.fee(5)))\n"),
    ]);
    fixture.agrees("6\n500\n500");
}

#[test]
fn qualified_initializers_do_not_reuse_caller_comptime_types() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "@ export seed\n> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n"),
        ("main.runa", "@ import Lib from ./dependency\n= seed = identity(\"root\")\n@ print(seed)\n@ print(show(Lib.seed + 1))\n"),
    ]);
    fixture.agrees("root\n501");
}

#[test]
fn qualified_functions_do_not_collide_with_caller_comptime_getters() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "@ export fee\n> fee(x: Int) -> Int { x * 100 }\n"),
        ("main.runa", "@ import Lib from ./dependency\n= fee = identity(7)\n@ print(show(fee))\n@ print(show(Lib.fee(5)))\n"),
    ]);
    fixture.agrees("7\n500");
}

#[test]
fn nested_qualified_initializers_keep_their_own_values_across_aliases() {
    let fixture = Fixture::new(&[
        ("leaf.runa", "@ export seed\n> initial() -> Int { 500 }\n= seed = initial()\n"),
        ("dependency.runa", "@ import Leaf from ./leaf\n@ export seed\n= seed = Leaf.seed + 1\n"),
        ("main.runa", "@ import First from ./dependency\n@ import Second from ./dependency\n@ import First from ./dependency\n= seed = identity(7)\n@ print(show(seed))\n@ print(show(First.seed))\n@ print(show(Second.seed))\n"),
    ]);
    fixture.agrees("7\n501\n501");
}

#[test]
fn qualified_initializer_proofs_agree_with_native_values() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "@ export fee\n@ export seed\n@ export snapshot\n> fee(x: Int) -> Int { x * 100 }\n= seed = fee(5)\n> snapshot() -> Int { seed }\n"),
        ("main.runa", "@ import ./dependency\n@ import Lib from ./dependency\n> fee(x: Int) -> Int { x + 1 }\n| correct: 0 -> Lib.snapshot() == 500\n| wrong: 0 -> Lib.snapshot() == 6\n? correct else { @ print(\"incorrect value\") }\n? wrong else { @ print(\"counterexample\") }\n"),
    ]);
    fixture.agrees("counterexample");
    if Command::new("z3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        let output = fixture.run(&["verify"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("PROVED: |correct|"), "{output:?}");
        assert!(!stdout.contains("PROVED: |wrong|"), "{stdout}");
        assert!(stdout.contains("COUNTEREXAMPLE"), "{stdout}");
    }
}

#[test]
fn maintained_qualified_initializer_corpus_agrees_in_both_execution_modes() {
    let fixture = Fixture::new(&[
        (
            "main.runa",
            include_str!("differential/corpus/imports/qualified_initializer_context.runa"),
        ),
        (
            "qualified_initializer_context_dep.runa",
            include_str!("differential/corpus/imports/qualified_initializer_context_dep.runa"),
        ),
    ]);
    fixture.agrees("6\n500\n500\n500\n500");
}

#[test]
fn late_plain_imports_keep_rule_misses_and_guard_priority() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "| known(1)\n| rate(x: Int) -> 1\n| rate(x: Int) -> 2 under x > 10\n"),
        ("main.runa", "| rate(x: Int) -> 3 under x > 5\n@ import ./dependency\n@ print(show(known(2)))\n@ print(show(findall(x, known(x))))\n@ print(show(rate(20)))\n"),
    ]);
    fixture.agrees("false\n[1]\n2");
}

#[test]
fn imported_initializer_errors_keep_their_source_location() {
    let fixture = Fixture::new(&[
        ("dependency.runa", "= broken = 1 / 0\n"),
        (
            "main.runa",
            "@ print(\"must not run\")\n@ import ./dependency\n@ print(show(broken))\n",
        ),
    ]);
    let output = fixture.run(&[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("dependency.runa:1:"), "{stderr}");
    assert!(stderr.contains("division by zero"), "{stderr}");
}
