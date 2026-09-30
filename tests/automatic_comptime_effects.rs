use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str, dependency: Option<&str>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "futuruna-constant-effects-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let fixture = Self(directory);
        fixture.write("main.runa", source);
        if let Some(dependency) = dependency {
            fixture.write("dependency.runa", dependency);
        }
        fixture
    }

    fn write(&self, filename: &str, source: &str) {
        let marker = self.0.join("effects.txt");
        let source = source.replace("MARKER", marker.to_str().unwrap());
        std::fs::write(self.0.join(filename), source).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .args(args)
            .arg(self.0.join("main.runa"))
            .current_dir(&self.0)
            .env("FUTURUNA_COMPILER_CACHE_DIR", self.0.join("cache"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        output
    }

    fn assert_compilation_has_no_effects(&self) {
        for args in [
            &["check", "--frontend"][..],
            &["check"],
            &["emit"],
            &["build"],
        ] {
            self.run(args);
            assert!(
                !self.0.join("effects.txt").exists(),
                "{args:?} executed an initializer while compiling"
            );
        }
    }

    fn assert_runtime_effect(&self) {
        let output = Command::new(self.0.join("main")).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "500");
        assert_eq!(
            std::fs::read_to_string(self.0.join("effects.txt")).unwrap(),
            "x"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn automatic_binding_preparation_never_writes_files() {
    for source in [
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n@ print(show(seed))\n",
        "> initialize() -> Int { append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n@ print(show(seed))\n",
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n> middle() -> Int { initialize() }\n> outer() -> Int { middle() }\n= seed = outer()\n@ print(show(seed))\n",
        "> initialize(x: Int) -> Int { @ append_file(\"MARKER\", \"x\"); x }\n> invoke(f: Int -> Int) -> Int { f(500) }\n= seed = invoke(initialize)\n@ print(show(seed))\n",
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n| answer() -> initialize()\n= seed = answer()\n@ print(show(seed))\n",
        "> identity(x: Int) -> Int { @ append_file(\"MARKER\", \"x\"); x }\n> initialize() -> Int { identity(500) }\n= seed = initialize()\n@ print(show(seed))\n",
    ] {
        let fixture = Fixture::new(source, None);
        fixture.assert_compilation_has_no_effects();
        fixture.assert_runtime_effect();
    }
}

#[test]
fn automatic_module_preparation_never_writes_files() {
    for (source, dependency) in [
        (
            "> module Lib { > initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize() }\n@ print(show(Lib.seed))\n",
            None,
        ),
        (
            "@ import Lib from ./dependency\n@ print(show(Lib.seed))\n",
            Some("@ export seed\n> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n"),
        ),
    ] {
        let fixture = Fixture::new(source, dependency);
        fixture.assert_compilation_has_no_effects();
        fixture.assert_runtime_effect();
    }
}

#[test]
fn rejected_initializer_does_not_become_a_symbolic_constant() {
    let fixture = Fixture::new(
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n> snapshot() -> String { show(seed) }\n= text = snapshot()\n@ print(text)\n",
        None,
    );
    fixture.assert_compilation_has_no_effects();
    fixture.assert_runtime_effect();
}

#[test]
fn runtime_file_reads_are_not_frozen_by_automatic_folding() {
    let fixture = Fixture::new(
        "> load() -> Result(String, String) { @ read_file(\"MARKER\") }\n> snapshot() -> String {\n    match load() {\n        | Ok(text) -> text\n        | Err(message) -> message\n    }\n}\n= text = snapshot()\n@ print(text)\n",
        None,
    );
    fixture.write("effects.txt", "compile-time contents");
    fixture.run(&["build"]);
    fixture.write("effects.txt", "runtime contents");
    let output = Command::new(fixture.0.join("main")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "runtime contents"
    );
}

#[test]
fn safe_comptime_values_and_types_work_beside_runtime_initializers() {
    let fixture = Fixture::new(
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n> square(x: Int) -> Int { x * x }\n@ comptime\n= size = square(5)\n@ comptime\n= Config = struct_type([field(\"size\", \"Int\")])\n= config = Config(size)\n@ comptime assert(size == 25)\n@ print(show(seed + config.size - 25))\n",
        None,
    );
    fixture.assert_compilation_has_no_effects();
    fixture.assert_runtime_effect();
}

/// `@ comptime` is pure in every command: a host effect reached from it,
/// directly or through local helpers, is a located compile-time error and is
/// never performed.
#[test]
fn explicit_comptime_rejects_host_effects_without_performing_them() {
    for (source, effect) in [
        (
            "@ comptime\n= r = process_run([\"/usr/bin/touch\", \"MARKER\"])\n@ print(\"hi\")\n",
            "process_run",
        ),
        (
            "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n@ comptime\n= seed = initialize()\n@ print(show(seed))\n",
            "append_file",
        ),
        (
            "> initialize() -> Int { append_file(\"MARKER\", \"x\"); 500 }\n> outer() -> Int { initialize() }\n@ comptime\n= seed = outer()\n= derived = seed + 1\n@ print(show(derived))\n",
            "append_file",
        ),
    ] {
        let fixture = Fixture::new(source, None);
        for mode in [&["check", "--frontend"][..], &["check"], &["emit"], &["build"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_runa"))
                .args(mode)
                .arg(fixture.0.join("main.runa"))
                .current_dir(&fixture.0)
                .env("FUTURUNA_COMPILER_CACHE_DIR", fixture.0.join("cache"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{mode:?}: {output:?}");
            assert!(
                stderr.contains(&format!(
                    "compile-time evaluation cannot perform the host effect `{effect}`"
                )) && stderr.contains("main.runa:"),
                "{mode:?}: {stderr}"
            );
            assert!(
                !fixture.0.join("effects.txt").exists(),
                "{mode:?} performed a compile-time effect"
            );
        }
    }
}

#[test]
fn explicit_comptime_rejects_unavailable_runtime_dependencies() {
    let fixture = Fixture::new(
        "> initialize() -> Int { @ append_file(\"MARKER\", \"x\"); 500 }\n= seed = initialize()\n> snapshot() -> String { show(seed) }\n@ comptime\n= text = snapshot()\n@ print(text)\n",
        None,
    );
    for mode in ["check", "emit", "build"] {
        let output = Command::new(env!("CARGO_BIN_EXE_runa"))
            .arg(mode)
            .arg(fixture.0.join("main.runa"))
            .current_dir(&fixture.0)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{mode}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("comptime evaluation failed")
                && stderr.contains("initialized value for `seed`"),
            "{mode}: {stderr}"
        );
        assert!(!fixture.0.join("effects.txt").exists());
    }
}

#[test]
fn runtime_bindings_hide_same_named_compile_time_builtins_and_functions() {
    for name in ["length", "identity"] {
        let source = format!("> initialize() -> Int {{ @ append_file(\"MARKER\", \"x\"); 500 }}\n= {name} = initialize()\n> snapshot() -> String {{ show({name}) }}\n= text = snapshot()\n@ print(text)\n");
        let fixture = Fixture::new(&source, None);
        fixture.assert_compilation_has_no_effects();
        fixture.assert_runtime_effect();
    }
}

#[test]
fn constant_candidates_restore_guards_and_do_not_publish_failed_modules() {
    use futuruna::{Expr, Interpreter, Lexer, Parser, Stmt, Value};

    fn parse(source: &str) -> Vec<Stmt> {
        Parser::new(Lexer::new(source).tokenize(), source)
            .parse_program()
            .unwrap()
    }
    fn expression(source: &str) -> Expr {
        let Stmt::Expr(expr) = parse(source).remove(0) else {
            panic!("expression fixture")
        };
        expr
    }

    let fixture = Fixture::new("", None);
    let mut interpreter = Interpreter::new();
    let mut env = interpreter.default_env();
    env.set("writer".into(), Value::Builtin("append_file".into()));
    interpreter.step_limit = 17;
    interpreter.step_count = 3;
    interpreter.budget_exceeded = true;
    interpreter.output.push("existing output".into());
    let write = format!(
        "writer({:?}, \"x\")",
        fixture.0.join("effects.txt").to_str().unwrap()
    );
    for source in [write.as_str(), "show(missing)", "range(0, 100)", "1 / 0"] {
        assert!(
            interpreter
                .try_eval_constant(&expression(source), &env, 100, 8)
                .is_none(),
            "{source}"
        );
        assert_eq!(interpreter.step_limit, 17);
        assert_eq!(interpreter.step_count, 3);
        assert!(interpreter.budget_exceeded);
        assert!(!interpreter.suppress_output);
        assert_eq!(interpreter.output, ["existing output"]);
    }
    assert!(!fixture.0.join("effects.txt").exists());
    assert!(interpreter
        .try_eval_constant(&expression("(1 + 2) * (3 + 4)"), &env, 1, 8)
        .is_none());
    assert_eq!(
        (
            interpreter.step_count,
            interpreter.step_limit,
            interpreter.budget_exceeded
        ),
        (3, 17, true)
    );

    let rejected_module = format!("> module Rejected {{ = seed = {write} }}");
    for (source, expected) in [
        (rejected_module.as_str(), false),
        ("> module Accepted { = seed = 40 + 2 }", true),
    ] {
        let Stmt::Defn(definition) = parse(source).remove(0) else {
            panic!("module fixture")
        };
        assert_eq!(
            interpreter
                .try_prepare_constant_definition(&definition, &mut env, 100, 8)
                .is_some(),
            expected
        );
    }
    assert!(env.get("Rejected").is_none());
    assert!(matches!(
        interpreter.try_eval_constant(&expression("Accepted.seed"), &env, 100, 8),
        Some(Value::Int(42))
    ));
    let Stmt::Defn(definition) = parse("> identity(x: Int) -> Int { x }").remove(0) else {
        panic!("function fixture")
    };
    interpreter.eval_defn(&definition, &mut env);
    env.defer_constant_binding("identity".into());
    for source in ["identity(42)", "identity(x = 42)"] {
        assert!(interpreter
            .try_eval_constant(&expression(source), &env, 100, 8)
            .is_none());
    }
}
