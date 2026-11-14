//! `runa test` and queries (docs/rfcs/needs-in-the-language.md): every query a
//! test asks runs the strict-mode oracle and the completion test, and a broken
//! guarantee fails the test at the query's line; `runa test --run` runs such a
//! file in the interpreter and says so.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

/// A calculation whose result depends on the income, and a test asking it
/// about a case where the income is not known yet. The query is on line 7.
const ASKS_INCOME: &str = "\
# Case(income: Int, married: Bool)
# Result(tax: Int)

@ calculate
| tax_of(c: Case) -> Result(tax = c.income / 10)

= asked = query tax_of(Case(income = unknown, married = False))
| asks_income: asked -> match asked {
    | Needs -> map(asked.needs, |n| n.path) == [pathof(Case::income)]
    | Complete -> False
}
? asks_income
";

fn fixture(source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "futuruna-needs-query-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("asks_income.runa"), source).unwrap();
    dir
}

fn runa_test(dir: &Path, arguments: &[&str], fault: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
    command
        .arg("test")
        .args(arguments)
        .arg(".")
        .current_dir(dir)
        .env("FUTURUNA_DISABLE_CALCULATION_CACHE", "1")
        .env_remove("FUTURUNA_TEST_FAULT");
    if let Some(fault) = fault {
        command.env("FUTURUNA_TEST_FAULT", fault);
    }
    let output: Output = command.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.success(),
        fault.is_none(),
        "runa test {arguments:?} with fault {fault:?}:\n{text}"
    );
    text
}

#[test]
fn a_query_passes_runa_test_and_runs_interpreted_under_run() {
    let dir = fixture(ASKS_INCOME);
    let tested = runa_test(&dir, &[], None);
    assert!(tested.contains("1 passed"), "{tested}");
    let run = runa_test(&dir, &["--run"], None);
    assert!(run.contains("interpreted (query)"), "{run}");
    let _ = std::fs::remove_dir_all(dir);
}

/// The fault hook exists only in debug builds: propagation claims `Complete`
/// for a case that still needs the income, and the completion test refutes
/// it at the query's line, naming the completion.
#[cfg(debug_assertions)]
#[test]
fn a_broken_guarantee_fails_runa_test_at_the_query_line() {
    let dir = fixture(ASKS_INCOME);
    let failed = runa_test(&dir, &[], Some("claim-complete"));
    let expected = "asks_income.runa:7: query tax_of, round 1: the propagated result is Complete, but the completion income = ";
    assert!(failed.contains(expected), "{failed}");
    assert!(
        failed.contains("returns a different result: {\"tax\":"),
        "{failed}"
    );
    let _ = std::fs::remove_dir_all(dir);
}
