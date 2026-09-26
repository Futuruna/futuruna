use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "futuruna-import-streams-{}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("shared.runa"),
            "> initial() -> String { \"boot\" }\n@ export\n~ pages = subject(initial())\n@ export\n> latest() -> String { pages.latest }\n",
        )
        .unwrap();
        std::fs::write(path.join("main.runa"), source).unwrap();
        Self(path)
    }

    fn assert_output(&self, arguments: &[&str], expected: &str) {
        let runa = std::env::var_os("FUTURUNA_MODEL_TEST_RUNA")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_runa").into());
        let output = Command::new(runa)
            .args(arguments)
            .arg(self.0.join("main.runa"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "{arguments:?}"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["shared.runa", "main.runa"] {
            std::fs::remove_file(self.0.join(name)).unwrap();
        }
        std::fs::remove_dir(&self.0).unwrap();
    }
}

const PLAIN: &str = "@ import ./shared\npages <- \"a\"\n@ import ./shared\n@ print(show(pages.count))\n@ print(latest())\n";
const QUALIFIED: &str = "@ import First from ./shared\n@ import Second from ./shared\n@ import First from ./shared\n@ print(show(First.pages.count))\n@ print(First.latest())\n@ print(show(Second.pages.count))\n@ print(Second.latest())\n";

#[test]
fn interpreted_plain_stream_bindings_initialize_once_and_share_helper_state() {
    let fixture = Fixture::new(PLAIN);
    for arguments in [&[][..], &["--no-prelude"][..]] {
        fixture.assert_output(arguments, "2\na");
    }
}

#[test]
fn interpreted_qualified_stream_bindings_initialize_before_exported_helpers() {
    let fixture = Fixture::new(QUALIFIED);
    for arguments in [&[][..], &["--no-prelude"][..]] {
        fixture.assert_output(arguments, "1\nboot\n1\nboot");
    }
}

#[test]
fn native_plain_imported_streams_agree_on_initialization_and_shared_state() {
    Fixture::new(PLAIN).assert_output(&["run"], "2\na");
}
