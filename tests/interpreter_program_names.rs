//! Two checked programs run through the public `Interpreter` API on one
//! thread, alternately. Each shares constructor names between two of its
//! types, so each runs under its own internal names and reads its imports as
//! its elaboration rewrote them. An interpreter is made for a checked program
//! from its `TypeCheckArtifacts` and holds that program's names itself; no
//! caller makes them current by hand.
//!
//! Before, an interpreter read the names of whatever program the thread
//! checked last: the account program, run after the door program was
//! checked, read its imported `AccountState` as written while its root names
//! the internal `Closed`, and `account_label` found no arm for a closed
//! account; an account interpreter made earlier showed `AccountState__Closed`.

use futuruna::{Expr, Interpreter, Lexer, Parser, Stmt, TypeCheckArtifacts, TypeChecker, Value};
use std::path::PathBuf;

const EVENTS: &str = "# Event = Opened | Closed
# AccountState = Active | Closed
> account_label(state: AccountState) -> String {
    match state {
        | Active -> \"account open\"
        | Closed -> \"account closed\"
    }
}
";

const ACCOUNT: &str = "@ import ./events
> status(state: AccountState) -> String {
    match state {
        | Closed -> \"closed\"
        | _ -> \"open\"
    }
}
> closed_account() -> AccountState { Closed }
> report() -> String {
    account_label(closed_account()) + \" / \" + status(closed_account()) + \" / \" + show(closed_account())
}
@ print(report())
";

const DOOR: &str = "# Door = Open | Shut
# Valve = Shut | Leak
> valve_label(valve: Valve) -> String {
    match valve {
        | Shut -> \"valve shut\"
        | _ -> \"valve leaking\"
    }
}
> shut_valve() -> Valve { Shut }
@ print(valve_label(shut_valve()))
";

const ACCOUNT_REPORT: &str = "account closed / closed / Closed";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "futuruna-interpreter-program-names-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("events.runa"), EVENTS).unwrap();
        Self(directory)
    }

    fn source_dir(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn parse(source: &str) -> Vec<Stmt> {
    Parser::new(Lexer::new(source).tokenize(), source)
        .parse_program()
        .expect("parse")
}

fn expression(source: &str) -> Expr {
    let Stmt::Expr(expression) = parse(source).remove(0) else {
        panic!("expression fixture");
    };
    expression
}

fn check(fixture: &Fixture, source: &str) -> TypeCheckArtifacts {
    let artifacts =
        TypeChecker::check_with_artifacts(&parse(source), Some(fixture.source_dir()), source);
    assert!(
        artifacts.diagnostics.is_empty(),
        "{:?}",
        artifacts.diagnostics
    );
    assert!(
        artifacts.constructor_elaboration.is_some(),
        "the fixture shares a constructor name"
    );
    artifacts
}

/// An interpreter for the checked program that has run it: its output and
/// the environment it left.
fn run(fixture: &Fixture, artifacts: &TypeCheckArtifacts) -> (Interpreter, futuruna::Env) {
    let mut interpreter = Interpreter::for_checked(artifacts);
    interpreter.suppress_output = true;
    interpreter.source_dir = Some(fixture.source_dir());
    let mut env = interpreter.default_env();
    interpreter.run_program(artifacts.program(), &mut env);
    (interpreter, env)
}

fn string(value: Value) -> String {
    match value {
        Value::Str(text) => text,
        other => panic!("expected a string, got {other:?}"),
    }
}

#[test]
fn checked_programs_run_alternately_with_their_own_constructor_names() {
    let fixture = Fixture::new();
    let account = check(&fixture, ACCOUNT);
    let (mut account_run, account_env) = run(&fixture, &account);
    assert_eq!(account_run.output, [ACCOUNT_REPORT]);

    // Checking the door program makes its names current on this thread.
    let door = check(&fixture, DOOR);
    let (mut door_run, door_env) = run(&fixture, &door);
    assert_eq!(door_run.output, ["valve shut"]);

    // An account interpreter made after the door check reads its own
    // program's names and imports, and so does the one made before it.
    let (account_again, _) = run(&fixture, &account);
    assert_eq!(account_again.output, [ACCOUNT_REPORT]);
    let report = expression("report()");
    assert_eq!(
        string(account_run.eval(&report, &account_env)),
        ACCOUNT_REPORT
    );

    // Alternate, and render values outside any run: each under its own names.
    let valve = expression("valve_label(shut_valve())");
    let closed = expression("closed_account()");
    for _ in 0..2 {
        assert_eq!(string(door_run.eval(&valve, &door_env)), "valve shut");
        let closed_account = account_run.eval(&closed, &account_env);
        assert_eq!(
            string(account_run.eval(&report, &account_env)),
            ACCOUNT_REPORT
        );
        assert_eq!(
            door_run.display_value(&closed_account),
            "AccountState__Closed"
        );
        assert_eq!(account_run.display_value(&closed_account), "Closed");
    }
}
