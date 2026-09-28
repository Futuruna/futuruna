use futuruna::{Lexer, Parser, Stmt, TypeChecker};

fn parse(source: &str) -> Result<Vec<Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "futuruna-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

#[test]
fn storage_annotations_are_unknown_annotations_at_their_source() {
    let mut failures = Vec::new();
    for declaration in [
        "@ store Item",
        "@ store Item delete_on_change in \"application\"",
        "@ persist Item",
        "@ persist Item in \"application\"",
        "@ migrate Item(id) -> Item(id, \"default\")",
        "> application() { @ persist Item }",
        "| scope application { @ store Item }",
    ] {
        let source = format!("# Item(id: Int)\n{declaration}\n");
        match parse(&source) {
            Err(error) if error.contains("unknown annotation `@ ") && error.contains("2:") => {}
            result => failures.push(format!("{declaration}: {result:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn fact_transaction_statements_are_located_errors() {
    for (statement, expected) in [("assert Item(1)", "2:8"), ("retract Item(1)", "2:9")] {
        let source = format!("# Item(id: Int)\n{statement}\n");
        let error = parse(&source).expect_err(statement);
        assert!(error.contains(expected), "{statement}: {error}");
    }
    let source = "# Item(id: Int)\n> application() { abort }\n";
    let statements = parse(source).unwrap();
    let errors = TypeChecker::check_with_source(&statements, None, source);
    assert!(
        errors.iter().any(|error| error.contains("`abort`")),
        "{errors:?}"
    );
}

#[test]
fn unchecked_hosts_receive_an_error_for_unknown_annotations() {
    for name in ["store", "persist", "migrate"] {
        let mut statements = vec![Stmt::Annot(name.into(), vec![])];
        statements.extend(parse("@ print(\"must not run\")\n").unwrap());
        let diagnostics = TypeChecker::check_with_source(&statements, None, "");
        assert!(
            diagnostics
                .iter()
                .any(|error| error.contains(&format!("unknown annotation `@ {name}`"))),
            "{name}: {diagnostics:?}"
        );
        let mut interpreter = futuruna::Interpreter::new();
        interpreter.suppress_output = true;
        let mut env = interpreter.default_env();
        let error = interpreter
            .run_program_with_diagnostics(
                &statements,
                &mut env,
                std::path::Path::new("unknown.runa"),
                "",
            )
            .expect_err("unchecked embedding must not silently ignore unknown annotations");
        assert!(error.message.contains("unknown annotation"), "{error:?}");
        assert!(interpreter.output.is_empty());
    }
}

#[test]
fn database_names_are_unknown_functions() {
    for name in [
        "db_open",
        "db_exec",
        "db_query",
        "db_query_row",
        "db_insert",
        "db_close",
        "watch",
    ] {
        let source = format!("= result = {name}(1)\n");
        let statements = parse(&source).unwrap();
        let errors = TypeChecker::check_with_source(&statements, None, &source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains(&format!("undefined function `{name}`"))),
            "{name}: {errors:?}"
        );
        let source = format!("= result = @ {name}(1)\n");
        let statements = parse(&source).unwrap();
        let errors = TypeChecker::check_with_source(&statements, None, &source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains(&format!("unknown effect `{name}`"))),
            "@ {name}: {errors:?}"
        );
    }
}

#[test]
fn db_is_an_ordinary_type_name() {
    let source = "> inspect(value: Db) -> Int { 1 }\n";
    let statements = parse(source).unwrap();
    let errors = TypeChecker::check_with_source(&statements, None, source);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("unknown type `Db`")),
        "{errors:?}"
    );
    let source = format!("# Db(id: Int)\n{source}");
    let statements = parse(&source).unwrap();
    let errors = TypeChecker::check_with_source(&statements, None, &source);
    assert!(
        errors.is_empty(),
        "authored types may use the name: {errors:?}"
    );
}

#[test]
fn ordinary_rules_scopes_and_host_function_names_work_without_storage() {
    let source = include_str!("differential/corpus/host_storage_boundary.runa");
    let interpreted = futuruna::eval_source_with_prelude(source, false).unwrap();
    assert_eq!(interpreted.trim(), "[Ada, Bo]\ntrue\n7");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/host_storage_boundary.runa");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
        .arg("run")
        .arg(&path)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim_end(),
        interpreted.trim_end()
    );
}

#[test]
fn proof_term_blocks_are_located_parse_errors() {
    for source in [
        "| add_comm: (a, b) -> a + b == b + a\n? add_comm by {\n    | (lhs, rhs) -> apply int_ring.comm_add\n}\n",
        "= big = 9223372036854775807\n| claim: big -> big < big + 1\n? claim by { | x -> apply int_ord.le_refl }\n",
    ] {
        let line = source.lines().position(|line| line.starts_with('?')).unwrap() + 1;
        let error = parse(source).expect_err(source);
        assert!(
            error.contains(&format!("{line}:")) && error.contains("`by`"),
            "{error}"
        );
    }
}

#[test]
fn cli_commands_stop_on_unknown_surface_forms_before_effects_or_formatting() {
    let root = temp_dir("unknown-surface");
    let path = root.join("model.runa");
    let mut failures = Vec::new();
    for (source, expected) in [
        (
            "@ print(\"must not run\")\n# Item(id: Int)\n@ persist Item\n",
            "unknown annotation `@ persist`",
        ),
        (
            "@ print(\"must not run\")\n| claim: x -> x == x\n? claim by { | x -> refl }\n",
            "`by`",
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        for args in [
            &[][..],
            &["check", "--frontend"],
            &["check"],
            &["emit"],
            &["run"],
            &["fmt", "--check"],
            &["fmt"],
        ] {
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
                .current_dir(&root)
                .args(args)
                .arg(&path)
                .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            if output.status.code() != Some(1)
                || !output.stdout.is_empty()
                || !String::from_utf8_lossy(&output.stderr).contains(expected)
            {
                failures.push(format!("{source:?} {args:?}: {output:?}"));
            }
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            source,
            "formatting must leave unparseable source untouched"
        );
    }
    for name in ["db_open", "db_query"] {
        std::fs::write(
            &path,
            format!("@ print(\"must not run\")\n= result = {name}(1)\n"),
        )
        .unwrap();
        for args in [&[][..], &["check"], &["emit"], &["run"]] {
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
                .current_dir(&root)
                .args(args)
                .arg(&path)
                .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.code() != Some(1)
                || !output.stdout.is_empty()
                || !stderr.contains(name)
                || !stderr.contains(":2:")
            {
                failures.push(format!("{name} {args:?}: {output:?}"));
            }
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
