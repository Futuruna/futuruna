use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "futuruna-module-initialization-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        for (name, source) in files {
            std::fs::write(directory.join(name), source).unwrap();
        }
        Self(directory)
    }

    fn agrees(&self, expected: &str) {
        let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        for args in [&[][..], &["check", "--frontend"], &["check"], &["run"]] {
            let output = Command::new(&binary)
                .args(args)
                .arg(self.0.join("main.runa"))
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

    fn fails(&self, expected_stdout: &str, diagnostic: &str) {
        let binary = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        for args in [&[][..], &["run"]] {
            let output = Command::new(&binary)
                .args(args)
                .arg(self.0.join("main.runa"))
                .output()
                .unwrap();
            assert!(!output.status.success(), "{args:?}: {output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                expected_stdout,
                "{args:?}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "{args:?}: {output:?}"
            );
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

const DEPENDENCY: &str = "@ export seed\n@ export read\n> initialize() -> Int { @ print(\"boot\"); 500 }\n= seed = initialize()\n> read() -> Int { seed }\n";

#[test]
fn qualified_module_reads_share_one_initialized_value() {
    Fixture::new(&[
        ("dependency.runa", DEPENDENCY),
        ("main.runa", "@ import Lib from ./dependency\n@ print(show(Lib.seed))\n@ print(show(Lib.seed))\n@ print(show(Lib.read()))\n"),
    ]).agrees("boot\n500\n500\n500");
}

#[test]
fn unused_exported_and_private_module_bindings_initialize_eagerly() {
    Fixture::new(&[
        ("dependency.runa", "@ export seed\n> initialize(label: String) -> Int { @ print(label); 500 }\n= private_value = initialize(\"private\")\n= seed = initialize(\"exported\")\n"),
        ("main.runa", "@ import Lib from ./dependency\n@ print(\"main\")\n"),
    ]).agrees("private\nexported\nmain");
}

#[test]
fn qualified_aliases_own_separate_values_and_repeated_aliases_initialize_once() {
    Fixture::new(&[
        ("dependency.runa", DEPENDENCY),
        ("main.runa", "@ import A from ./dependency\n@ import A from ./dependency\n@ import B from ./dependency\n@ print(show(A.seed + A.read() + B.seed + B.read()))\n"),
    ]).agrees("boot\nboot\n2000");
}

#[test]
fn module_initialization_respects_forward_value_dependencies() {
    Fixture::new(&[
        ("dependency.runa", "@ export result\n@ export read\n> initialize(x: Int) -> Int { @ print(show(x)); x }\n> compute() -> Int { base + 1 }\n= result = initialize(compute())\n= base = initialize(4)\n> read() -> Int { result + base }\n"),
        ("main.runa", "@ import Lib from ./dependency\n@ print(show(Lib.result))\n@ print(show(Lib.read()))\n"),
    ]).agrees("4\n5\n5\n9");
}

#[test]
fn nested_aliases_initialize_once_per_parent_namespace() {
    Fixture::new(&[
        ("dependency.runa", DEPENDENCY),
        ("wrapper.runa", "@ export Inner\n@ import Inner from ./dependency\n@ import Inner from ./dependency\n"),
        ("main.runa", "@ import A from ./wrapper\n@ import B from ./wrapper\n@ print(show(A.Inner.seed + A.Inner.read()))\n@ print(show(B.Inner.seed + B.Inner.read()))\n"),
    ]).agrees("boot\nboot\n1000\n1000");
}

#[test]
fn inline_module_values_belong_to_each_enclosing_call() {
    Fixture::new(&[("main.runa", "> initialize(x: Int) -> Int { @ print(show(x)); x }\n> make(x: Int) -> Int { > module Local { = seed = initialize(x)\n> read() -> Int { seed } }\nLocal.seed + Local.read() }\n@ print(show(make(3)))\n@ print(show(make(7)))\n")])
        .agrees("3\n6\n7\n14");
}

#[test]
fn module_body_effects_execute_once() {
    Fixture::new(&[("main.runa", "> module Local { @ print(\"boot\")\n= seed = 5 }\n@ print(show(Local.seed))\n@ print(show(Local.seed))\n")]).agrees("boot\n5\n5");
}

#[test]
fn module_initialization_executes_statement_bodies() {
    Fixture::new(&[("main.runa", "> module Local { for x in [1, 2] { @ print(show(x)) }\n= seed = 3 }\n@ print(show(Local.seed))\n")]).agrees("1\n2\n3");
}

#[test]
fn module_callback_owns_its_initialized_values() {
    Fixture::new(&[("main.runa", "> initialize() -> Int { @ print(\"boot\"); 4 }\n> module Local { = seed = initialize()\n> add(x: Int) -> Int { seed + x } }\n@ print(show(map([1,2], Local.add)))\n")]).agrees("boot\n[5, 6]");
}

#[test]
fn inline_module_captures_do_not_consume_the_callers_strings() {
    Fixture::new(&[("main.runa", "> make(x: String) -> String { > module Local { = seed = x\n> read() -> String { seed } }\nLocal.read() + x }\n@ print(make(\"yes\"))\n")]).agrees("yesyes");
}

#[test]
fn nested_inline_module_owns_parent_values() {
    Fixture::new(&[("main.runa", "> initialize() -> Int { @ print(\"boot\"); 4 }\n> module Parent { = seed = initialize()\n> module Child { > read() -> Int { seed } } }\n@ print(show(Parent.Child.read()))\n@ print(show(Parent.Child.read()))\n")]).agrees("boot\n4\n4");
}

#[test]
fn nested_module_calls_helpers_with_their_parent_state() {
    Fixture::new(&[("main.runa", "> init() -> Int { @ print(\"boot\"); 4 }\n> module Parent { = seed = init()\n> read() -> Int { seed }\n> module Child { > value() -> Int { read() } } }\n@ print(show(Parent.Child.value()))\n@ print(show(Parent.Child.value()))\n")]).agrees("boot\n4\n4");
}

#[test]
fn module_parent_callbacks_read_the_current_parent_bindings() {
    Fixture::new(&[("main.runa", "> init() -> Int { @ print(\"boot\"); 4 }\n> module Parent { = seed = init()\n> read(x: Int) -> Int { seed + x }\n> module Child { > value(x: Int) -> Int { read(x) } }\n= seed = 7 }\n@ print(show(map([1,2], Parent.Child.value)))\n")]).agrees("boot\n[8, 9]");
}

#[test]
fn function_local_module_helpers_use_the_root_instance() {
    Fixture::new(&[("main.runa", "> init() -> Int { @ print(\"boot\"); 4 }\n= seed = init()\n> read() -> Int { seed }\n> make(x: Int) -> Int { > module Local { > value() -> Int { read() + x } }\nLocal.value() }\n@ print(show(make(2)))\n@ print(show(make(3)))\n")]).agrees("boot\n6\n7");
}

#[test]
fn module_initialization_failure_stops_the_importer() {
    Fixture::new(&[("dependency.runa", "> initialize() -> Int { @ print(\"before\"); assert(False); 5 }\n= private_value = initialize()\n"), ("main.runa", "@ import Lib from ./dependency\n@ print(\"after\")\n")]).fails("before", "Assertion failed!");
}

#[test]
fn cyclic_module_bindings_fail_without_running_the_importer() {
    Fixture::new(&[
        ("dependency.runa", "= left = right\n= right = left\n"),
        (
            "main.runa",
            "@ import Lib from ./dependency\n@ print(\"after\")\n",
        ),
    ])
    .fails(
        "",
        "cyclic top-level value initialization: left -> right -> left",
    );
}

#[test]
fn escaping_module_callback_keeps_its_instance_alive() {
    Fixture::new(&[("main.runa", "> make(x: Int) -> Int -> Int { > module Local { = seed = x\n> add(y: Int) -> Int { seed + y } }\nLocal.add }\n= earlier_reader = make(3)\n= later_reader = make(7)\n@ print(show(earlier_reader(1)))\n@ print(show(later_reader(1)))\n@ print(show(earlier_reader(2)))\n")]).agrees("4\n8\n5");
}

#[test]
fn generic_local_module_owns_each_calls_value() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> a {
    > module Local {
        = seed = x
        > read() -> a { seed }
    }
    Local.read()
}
@ print(show(make(7)))
@ print(make("yes"))
@ print(show(make(11)))
"#,
    )])
    .agrees("7\nyes\n11");
}

#[test]
fn generic_local_module_captures_local_alias_and_compound_values() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: List(a)) -> List(a) {
    = saved = x
    > module Local {
        = seed = saved
        > read() -> List(a) { seed }
    }
    Local.read()
}
> optional(x: Option(a)) -> Option(a) {
    > module Local { = seed = x; > read() -> Option(a) { seed } }
    Local.read()
}
@ print(show(make([7, 8])))
@ print(show(make(["yes", "again"])))
@ print(show(optional(Some(9))))
@ print(show(optional(Some("value"))))
"#,
    )])
    .agrees("[7, 8]\n[yes, again]\nSome(9)\nSome(value)");
}

#[test]
fn generic_module_callback_owns_its_value_after_enclosing_call_returns() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> Int -> a {
    > module Local {
        = seed = x
        > read(ignored: Int) -> a { seed }
    }
    Local.read
}
= earlier_reader = make("earlier")
= later_reader = make("later")
= number_reader = make(19)
@ print(earlier_reader(0))
@ print(later_reader(0))
@ print(show(number_reader(0)))
@ print(earlier_reader(0))
"#,
    )])
    .agrees("earlier\nlater\n19\nearlier");
}

#[test]
fn nested_generic_module_owns_parent_values() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> a {
    > module Parent {
        = seed = x
        > module Child { > read() -> a { seed } }
    }
    Parent.Child.read()
}
@ print(make("value"))
@ print(show(make(23)))
"#,
    )])
    .agrees("value\n23");
}

#[test]
fn nested_generic_module_helpers_receive_parent_context_type() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> a {
    > module Parent {
        = seed = x
        > read() -> a { seed }
        > module Child { > value() -> a { read() } }
    }
    Parent.Child.value()
}
@ print(make("helper"))
@ print(show(make(29)))
"#,
    )])
    .agrees("helper\n29");
}

#[test]
fn generic_module_state_does_not_require_default_or_unused_type_parameters() {
    Fixture::new(&[(
        "main.runa",
        r#"
# Token = Token(value: Int)
> make(x: a, unrelated: b) -> a {
    > module Local {
        = seed = x
        > read() -> a { seed }
        > rendered() -> String { show(seed) }
    }
    @ print(Local.rendered())
    Local.read()
}
@ print(show(make(Token(31), "unused")))
@ print(show(make(Token(37), True)))
"#,
    )])
    .agrees("Token(value: 31)\nToken(value: 31)\nToken(value: 37)\nToken(value: 37)");
}

#[test]
fn rust_library_consumer_initializes_and_reuses_module_values() {
    let fixture = Fixture::new(&[("dependency.runa", DEPENDENCY), ("main.runa", "@ import Lib from ./dependency\n@ export answer\n> answer() -> Int { Lib.seed + Lib.read() }\n")]);
    let output = Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("lib")
        .arg(fixture.0.join("main.runa"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    std::fs::write(fixture.0.join("generated.rs"), &output.stdout).unwrap();
    std::fs::write(fixture.0.join("consumer.rs"), "mod generated;\nfn main() { let model = generated::__fut_init(); println!(\"{}\", generated::answer(&model)); println!(\"{}\", generated::answer(&model)); }\n").unwrap();
    let compilation = Command::new("rustc")
        .args(["--edition=2021", "--crate-name", "consumer"])
        .arg(fixture.0.join("consumer.rs"))
        .arg("-o")
        .arg(fixture.0.join("consumer"))
        .output()
        .unwrap();
    assert!(compilation.status.success(), "{compilation:?}");
    let execution = Command::new(fixture.0.join("consumer")).output().unwrap();
    assert!(execution.status.success(), "{execution:?}");
    assert_eq!(
        String::from_utf8_lossy(&execution.stdout),
        "boot\n1000\n1000\n"
    );
}

#[test]
fn generic_module_rule_returns_owned_capture() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> a { > module Local { = seed = x
| read() -> seed }
Local.read() }
@ print(show(make(7)))
@ print(make("yes"))
"#,
    )])
    .agrees("7\nyes");
}

#[test]
fn generic_capture_metadata_does_not_leak_to_a_same_named_outer_module() {
    Fixture::new(&[(
        "main.runa",
        r#"
> module Local { = value = 4
> read() -> Int { value } }
> make(x: a) -> a { > module Local { = seed = x
> read() -> a { seed } }
Local.read() }
@ print(make("yes"))
@ print(show(Local.read()))
"#,
    )])
    .agrees("yes\n4");
}

#[test]
fn generic_typed_empty_module_binding_retains_declared_parameter() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> Option(a) { > module Local { = seed: Option(a) = None
> read() -> Option(a) { seed } }
Local.read() }
@ print(show(make(7)))
"#,
    )])
    .agrees("None");
}

#[test]
fn nested_generic_empty_binding_parameter_reaches_parent_state() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(x: a) -> Option(a) { > module Parent { > module Child { = seed: Option(a) = None
> read() -> Option(a) { seed } }
> read() -> Option(a) { Child.read() } }
Parent.read() }
@ print(show(make(7)))
"#,
    )])
    .agrees("None");
}

#[test]
fn generic_module_capture_preserves_nested_result_and_pair_types() {
    Fixture::new(&[("main.runa", r#"
> make(x: Result(Pair(a, b), String)) -> Result(Pair(a, b), String) {
    > module Local {
        = seed = x
        > read() -> Result(Pair(a, b), String) { seed }
    }
    Local.read()
}
= number_text: Result(Pair(Int, String), String) = Ok(Pair(7, "yes"))
= text_number: Result(Pair(String, Int), String) = Ok(Pair("again", 11))
= failed: Result(Pair(Int, String), String) = Err("failure")
@ print(match make(number_text) { | Ok(value) -> show(value.fst) + value.snd; | Err(message) -> message })
@ print(match make(text_number) { | Ok(value) -> value.fst + show(value.snd); | Err(message) -> message })
@ print(match make(failed) { | Ok(value) -> show(value.fst) + value.snd; | Err(message) -> message })
"#)]).agrees("7yes\nagain11\nfailure");
}

#[test]
fn generic_child_helper_uses_the_complete_ancestor_state_type() {
    Fixture::new(&[(
        "main.runa",
        r#"
> make(y: b, x: a) -> Int {
    > module Parent {
        > size_of_child() -> Int { length(Child.items) }
        > module Child {
            = items: List(Pair(a, b)) = []
            > count() -> Int { size_of_child() }
        }
        > read() -> Int { Child.count() }
    }
    Parent.read()
}
@ print(show(make(5, "unused")))
"#,
    )])
    .agrees("0");
}
