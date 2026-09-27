use futuruna::{Lexer, Parser, Stmt, TypeChecker};

fn parse(source: &str) -> Result<Vec<Stmt>, String> {
    Parser::new(Lexer::new(source).tokenize(), source).parse_program()
}

#[test]
fn parser_rejects_removed_persistence_annotations_at_their_source() {
    let mut failures = Vec::new();
    for declaration in [
        "@ store Item",
        "@ store Item delete_on_change in \"application\"",
        "@ persist Item",
        "@ persist Item in \"application\"",
        "@ migrate Item(id) -> Item(id, \"default\")",
        "@ migrate Item(id, name) -> Item(id) unsafe",
        "> application() { @ persist Item }",
        "| scope application { @ store Item }",
        "= result = @ store(Item)",
        "assert Item(1)",
        "retract Item(1)",
        "abort",
        "> application() { assert Item(1) }",
        "| scope application { retract Item(1) }",
        "> application() { abort }",
    ] {
        let source = format!("# Item(id: Int)\n{declaration}\n");
        match parse(&source) {
            Err(error)
                if error.contains("database persistence was removed") && error.contains("2:") => {}
            result => failures.push(format!("{declaration}: {result:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn unchecked_hosts_receive_an_error_for_retired_annotations() {
    for name in ["store", "persist", "migrate"] {
        let mut statements = vec![Stmt::Annot(name.into(), vec![])];
        statements.extend(parse("@ print(\"must not run\")\n").unwrap());
        let mut interpreter = futuruna::Interpreter::new();
        interpreter.suppress_output = true;
        let mut env = interpreter.default_env();
        let error = interpreter
            .run_program_with_diagnostics(
                &statements,
                &mut env,
                std::path::Path::new("removed.runa"),
                "",
            )
            .expect_err("unchecked embedding must not silently ignore retired annotations");
        assert!(
            error.message.contains("database persistence was removed"),
            "{error:?}"
        );
        assert!(interpreter.output.is_empty());
    }
}

#[test]
fn database_and_change_feed_names_are_no_longer_builtins() {
    for name in [
        "db_open",
        "db_exec",
        "db_query",
        "db_query_row",
        "db_insert",
        "db_close",
        "watch",
    ] {
        for expression in [format!("{name}(1)"), format!("@ {name}(1)")] {
            let source = format!("= result = {expression}\n");
            let statements = parse(&source).unwrap();
            let errors = TypeChecker::check_with_source(&statements, None, &source);
            assert!(
                errors.iter().any(|error| error.contains(name)),
                "removed builtin {expression} must fail checking: {errors:?}"
            );
            let mut interpreter = futuruna::Interpreter::new();
            interpreter.suppress_output = true;
            let mut env = interpreter.default_env();
            assert!(
                interpreter
                    .run_program_with_diagnostics(
                        &statements,
                        &mut env,
                        std::path::Path::new("removed.runa"),
                        &source
                    )
                    .is_err(),
                "{expression} must not return a fabricated database result"
            );
            env.set(name.into(), futuruna::Value::Builtin(name.into()));
            assert!(
                interpreter
                    .run_program_with_diagnostics(
                        &statements,
                        &mut env,
                        std::path::Path::new("removed.runa"),
                        &source
                    )
                    .is_err(),
                "an embedding's stale builtin registration for {name} must fail explicitly"
            );
        }
    }
}

#[test]
fn db_is_no_longer_an_intrinsic_type() {
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
        "authored types may use the retired name: {errors:?}"
    );
}

#[test]
fn ordinary_rules_scopes_and_host_function_names_survive_without_sql() {
    let source = include_str!("differential/corpus/host_storage_boundary.runa");
    let interpreted = futuruna::eval_source_with_prelude(source, false).unwrap();
    assert_eq!(interpreted.trim(), "[Ada, Bo]\ntrue\n7");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/corpus/host_storage_boundary.runa");
    for mode in ["emit", "run"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_runa"))
            .args([mode])
            .arg(&path)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        if mode == "run" {
            assert_eq!(text.trim_end(), interpreted.trim_end());
        } else {
            assert!(
                !text.contains("rusqlite") && !text.contains("__FutPersist"),
                "{text}"
            );
        }
    }
}

#[test]
fn manually_constructed_removed_annotations_are_rejected_by_the_checker() {
    for name in ["store", "persist", "migrate"] {
        let mut statements = parse("# Item(id: Int)\n").unwrap();
        statements.push(Stmt::Annot(name.to_string(), vec![]));
        let diagnostics = TypeChecker::check_with_source(&statements, None, "");
        assert!(
            diagnostics
                .iter()
                .any(|error| error.contains("database persistence was removed")),
            "{name}: {diagnostics:?}"
        );
    }
}

#[test]
fn removed_database_forms_stop_cli_commands_before_effects_or_formatting() {
    let root = std::env::temp_dir().join(format!(
        "futuruna-removed-persistence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("model.runa");
    let source = "@ print(\"must not run\")\n# Item(id: Int)\n@ persist Item\n";
    std::fs::write(&path, source).unwrap();
    let mut failures = Vec::new();
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
            || !String::from_utf8_lossy(&output.stderr).contains("database persistence was removed")
        {
            failures.push(format!("{args:?}: {output:?}"));
        }
    }
    let after = std::fs::read_to_string(&path).unwrap();
    for name in [
        "db_open",
        "db_exec",
        "db_query",
        "db_query_row",
        "db_insert",
        "db_close",
        "watch",
    ] {
        for expression in [format!("{name}(1)"), format!("@ {name}(1)")] {
            std::fs::write(
                &path,
                format!("@ print(\"must not run\")\n= result = {expression}\n"),
            )
            .unwrap();
            for args in [
                &[][..],
                &["check", "--frontend"],
                &["check"],
                &["emit"],
                &["run"],
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
                    || !String::from_utf8_lossy(&output.stderr).contains(name)
                {
                    failures.push(format!("{expression}, {args:?}: {output:?}"));
                }
            }
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        after, source,
        "formatting must leave removed syntax untouched"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
