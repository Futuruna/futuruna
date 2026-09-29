use std::process::{Command, Output};

fn run(name: &str, source: &str, compiled: bool) -> Output {
    let dir = std::env::temp_dir().join(format!(
        "futuruna-fail-closed-{}-{name}-{}",
        std::process::id(),
        if compiled { "run" } else { "interp" }
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.runa"));
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
    if compiled {
        command.arg("run");
    }
    command
        .arg(&path)
        .env("NO_COLOR", "1")
        .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.starts_with("runa: "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_fails_with(name: &str, source: &str, message: &str) {
    let output = run(name, source, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
    assert!(stderr.contains(message), "{name}: {stderr}");
}

#[test]
fn parse_file_and_json_builtins_return_results_in_both_modes() {
    let source = r#"= missing = read_file("/path/that/does-not-exist.txt")
@ print(match missing { | Ok(text) -> "read " + text | Err(_) -> "unreadable" })
@ print(show(parse_int(" 42 ")))
@ print(show(parse_int("abc")))
@ print(show(parse_float("1,5")))
@ print(show(json_parse("{nope")))
"#;
    for compiled in [false, true] {
        let output = run("results", source, compiled);
        assert!(output.status.success(), "compiled={compiled}: {output:?}");
        let text = stdout(&output);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "unreadable", "compiled={compiled}: {text}");
        assert_eq!(lines[1], "Ok(42)", "compiled={compiled}: {text}");
        assert_eq!(
            lines[2], "Err(not an integer: `abc`)",
            "compiled={compiled}: {text}"
        );
        assert_eq!(
            lines[3], "Err(not a number: `1,5`)",
            "compiled={compiled}: {text}"
        );
        assert!(
            lines[4].starts_with("Err(invalid JSON"),
            "compiled={compiled}: {text}"
        );
    }
}

#[test]
fn failed_file_writes_stop_the_program() {
    for effect in ["write_file", "append_file"] {
        assert_fails_with(
            effect,
            &format!("@ {effect}(\"/path/no/such/dir/x.txt\", \"hi\")\n@ print(\"after write\")\n"),
            "cannot write /path/no/such/dir/x.txt",
        );
    }
}

#[test]
fn unhandled_effect_operation_names_the_effect() {
    assert_fails_with(
        "unhandled",
        "# effect Ask {\n    > get_num() -> Int\n}\n> twice() -> Int with Ask {\n    get_num() + get_num()\n}\n@ print(\"unhandled \" + show(twice()))\n",
        "effect `Ask` is not handled",
    );
}

#[test]
fn handler_without_resume_ends_the_handled_body() {
    let source = "# effect Ask {\n    > stop() -> Int\n}\n> body() -> Int with Ask {\n    = x = stop()\n    @ print(\"body continued\")\n    x\n}\n= r = | handle Ask {\n    | stop() -> 7\n} in body()\n@ print(show(r))\n";
    let output = run("abort", source, false);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stdout(&output), "7");
}

#[test]
fn value_rule_misses_and_non_boolean_guards_fail_closed() {
    assert_fails_with(
        "rule_miss",
        "| eligible(age) -> \"yes\" under age > 18\n@ print(\"[\" + eligible(3) + \"]\")\n",
        "no value rule matched `eligible/1`",
    );
    assert_fails_with(
        "guard",
        "> f(n: Int) -> Int { match n { | x if x -> 1 | _ -> 2 } }\n@ print(show(f(1)))\n",
        "match guard must return Bool",
    );
}

#[test]
fn rule_scope_call_syntax_reaches_the_global_function_in_both_modes() {
    let source = "> income() -> Int { 7 }\n# Case(income: Int) {\n    | tax() -> income() * 2\n}\n@ print(show(Case(100).tax()))\n";
    for compiled in [false, true] {
        let output = run("scope_call", source, compiled);
        assert!(output.status.success(), "compiled={compiled}: {output:?}");
        assert_eq!(stdout(&output), "14", "compiled={compiled}");
    }
}

#[test]
fn take_and_skip_keep_lists_as_lists_in_both_modes() {
    let source = "= a = [1,2,3,4] |> take(2)\n= c = take([1,2,3], 2)\n= d = skip([1,2,3], 1)\n@ print(show(a))\n@ print(show(c))\n@ print(show(d))\n@ print(show(length(a)))\n";
    for compiled in [false, true] {
        let output = run("take", source, compiled);
        assert!(output.status.success(), "compiled={compiled}: {output:?}");
        assert_eq!(
            stdout(&output),
            "[1, 2]\n[1, 2]\n[2, 3]\n2",
            "compiled={compiled}"
        );
    }
}
