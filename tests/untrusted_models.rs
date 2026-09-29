//! Analysis commands never execute a model's effects, fetch code or build
//! foreign crates; dependencies are fetched only by `runa add` / `runa fetch`
//! and resolved from `runa.lock`.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "futuruna-untrusted-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(root.join("project")).unwrap();
        // A VCS root stops the manifest search inside the sandbox.
        std::fs::create_dir_all(root.join("project/.git")).unwrap();
        Self(std::fs::canonicalize(root).unwrap())
    }

    fn project(&self) -> PathBuf {
        self.0.join("project")
    }

    fn marker(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Write a project file; `MARK` expands to the sandbox directory.
    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.project().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content.replace("MARK", self.0.to_str().unwrap())).unwrap();
        path
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        command
            .current_dir(self.project())
            .env("HOME", self.0.join("home"))
            .env_remove("XDG_CACHE_HOME")
            .env("FUTURUNA_COMPILER_CACHE_DIR", self.0.join("cache"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("NO_COLOR", "1");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn dependency_cache(&self) -> PathBuf {
        let home = self.0.join("home");
        if cfg!(target_os = "macos") {
            home.join("Library/Caches/futuruna/deps")
        } else {
            home.join(".cache/futuruna/deps")
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn git(directory: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(directory)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// A bare repository `<sandbox>/<name>.git` whose lib.runa exports `value`.
fn local_repository(sandbox: &Sandbox, name: &str, value: i64) -> PathBuf {
    let work = sandbox.0.join(format!("{name}-work"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(
        work.join("lib.runa"),
        format!("@ export\n> value() -> Int {{ {value} }}\n"),
    )
    .unwrap();
    git(&work, &["init", "--quiet", "-b", "main"]);
    git(&work, &["add", "lib.runa"]);
    git(&work, &["commit", "--quiet", "-m", "lib"]);
    let bare = sandbox.0.join(format!("{name}.git"));
    git(
        &sandbox.0,
        &[
            "clone",
            "--quiet",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    bare
}

const MANIFEST: &str = "[package]\nname = \"p\"\nversion = \"0.1.0\"\n\n[dependencies]\n";

#[test]
fn depend_accepts_only_registry_versions_and_features() {
    let sandbox = Sandbox::new();
    for (source, expected) in [
        (
            "@ depend \"markercrate\" \"{ path = \\\"MARK/markercrate\\\" }\"\n@ print(\"hi\")\n",
            "path, git and inline-table sources are not supported",
        ),
        (
            "@ depend \"x\" \"{ git = \\\"https://example.com/x\\\" }\"\n@ print(\"hi\")\n",
            "path, git and inline-table sources are not supported",
        ),
        (
            "@ depend \"../x\" \"1\"\n@ print(\"hi\")\n",
            "invalid crate name",
        ),
    ] {
        let path = sandbox.write("m.runa", source);
        for mode in [&["check"][..], &["check", "--build-deps"], &["build"]] {
            let output = sandbox.run(&[mode, &[path.to_str().unwrap()]].concat());
            assert_eq!(output.status.code(), Some(1), "{mode:?}: {output:?}");
            let message = stderr(&output);
            assert!(message.contains(expected), "{mode:?}: {message}");
            assert!(message.contains("1:"), "unlocated: {message}");
        }
    }
    assert!(!sandbox.project().join(".runa-build").exists());
}

#[test]
fn check_builds_cargo_dependencies_only_when_asked() {
    let sandbox = Sandbox::new();
    let path = sandbox.write(
        "m.runa",
        "@ depend \"sha2\" \"0.10\" [\"std\"]\n@ print(\"hi\")\n",
    );
    let output = sandbox
        .command()
        .args(["check", path.to_str().unwrap()])
        .env("PATH", "/nonexistent")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = stderr(&output);
    assert!(
        message.contains("runa check --build-deps") && message.contains("`sha2`"),
        "{message}"
    );
    assert!(!sandbox.project().join(".runa-build").exists());
}

#[test]
fn audit_never_runs_top_level_statements_or_host_effects() {
    let sandbox = Sandbox::new();
    let path = sandbox.write(
        "m_audit.runa",
        "@ write_file(\"MARK/AUDIT_TOPLEVEL.marker\", \"x\")\n| r() -> True\n",
    );
    let output = sandbox.run(&["audit", path.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert!(!sandbox.marker("AUDIT_TOPLEVEL.marker").exists());

    let path = sandbox.write(
        "rule_audit.runa",
        "> touch() -> Bool { write_file(\"MARK/AUDIT_RULE.marker\", \"x\"); True }\n| r() -> touch()\n",
    );
    let output = sandbox.run(&["audit", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = stderr(&output);
    assert!(
        message.contains("`runa audit` cannot perform the host effect `write_file`")
            && message.contains("rule_audit.runa:1:"),
        "{message}"
    );
    assert!(!sandbox.marker("AUDIT_RULE.marker").exists());
}

const CALCULATION: &str = "# In(n: Int)\n# Out(n: Int)\n";

#[test]
fn calculations_reject_effects_statically_and_at_run_time() {
    let sandbox = Sandbox::new();
    // The issue's reproduction: a plain binding inside the calculation.
    let direct = sandbox.write(
        "calcfx.runa",
        &format!("{CALCULATION}@ calculate\n> calc(i: In) -> Out {{\n    = w = write_file(\"MARK/CALL.marker\", \"x\")\n    Out(i.n)\n}}\n"),
    );
    // Reached through a helper.
    let helper = sandbox.write(
        "helper.runa",
        &format!("{CALCULATION}> log(n: Int) -> Int {{ @ print(show(n)); n }}\n@ calculate\n> calc(i: In) -> Out {{ Out(log(i.n)) }}\n"),
    );
    for (path, effect, line) in [(&direct, "write_file", ":5:"), (&helper, "print", ":3:")] {
        for args in [
            &["check", "--frontend"][..],
            &["schema"],
            &["template", "--format", "json"],
        ] {
            let output = sandbox.run(&[args, &[path.to_str().unwrap()]].concat());
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            let message = stderr(&output);
            assert!(
                message.contains(&format!("performs the effect `{effect}`"))
                    && message.contains(line),
                "{args:?}: {message}"
            );
        }
    }

    // An imported helper is invisible to the local walk; `call` refuses it.
    sandbox.write(
        "effects.runa",
        "@ export\n> touch(n: Int) -> Int { write_file(\"MARK/CALL_IMPORTED.marker\", \"x\"); n }\n",
    );
    let imported = sandbox.write(
        "imported.runa",
        &format!("@ import Effects from ./effects\n{CALCULATION}@ calculate\n> calc(i: In) -> Out {{ Out(Effects.touch(i.n)) }}\n"),
    );
    let input = sandbox.project().join("input.json");
    let template = sandbox.run(&[
        "template",
        imported.to_str().unwrap(),
        "--format",
        "json",
        "--output",
        input.to_str().unwrap(),
    ]);
    assert!(template.status.success(), "{template:?}");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    envelope["cases"][0]["input_status"] = "ready".into();
    envelope["cases"][0]["input"] = serde_json::json!({"n": 3});
    std::fs::write(&input, serde_json::to_vec(&envelope).unwrap()).unwrap();
    let call = sandbox.run(&[
        "call",
        imported.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ]);
    assert!(!call.status.success(), "{call:?}");
    let report = format!("{}{}", String::from_utf8_lossy(&call.stdout), stderr(&call));
    assert!(
        report.contains("a calculation cannot perform the host effect `write_file`"),
        "{report}"
    );
    assert!(!sandbox.marker("CALL.marker").exists());
    assert!(!sandbox.marker("CALL_IMPORTED.marker").exists());
}

#[test]
fn compiler_caches_live_in_a_private_directory_owned_by_the_user() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let path = sandbox.write("main.runa", "@ print(\"hi\")\n");
    let cache = sandbox.0.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777)).unwrap();
    let output = sandbox.run(&["run", path.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hi");
    let mode = std::fs::metadata(&cache).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert!(!sandbox.project().join(".runa-build").exists());

    // A symbolic link could point anywhere another account controls.
    let elsewhere = sandbox.0.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let linked = sandbox.0.join("linked-cache");
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    let output = sandbox
        .command()
        .args(["run", path.to_str().unwrap()])
        .env("FUTURUNA_COMPILER_CACHE_DIR", &linked)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("is not a directory; refusing to cache or build there"),
        "{output:?}"
    );
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[test]
fn git_sources_that_could_become_options_are_rejected_before_git_runs() {
    let sandbox = Sandbox::new();
    sandbox.write(
        "runa.toml",
        &format!("{MANIFEST}dep = {{ git = \"--upload-pack=touch MARK/UPLOADPACK.marker\" }}\n"),
    );
    let output = sandbox.run(&["fetch"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("unsupported git source"),
        "{output:?}"
    );
    assert!(stderr(&output).contains("runa.toml:6:"), "{output:?}");
    assert!(!sandbox.marker("UPLOADPACK.marker").exists());

    sandbox.write("runa.toml", MANIFEST);
    let bare = local_repository(&sandbox, "lib", 1);
    let output = sandbox.run(&["add", bare.to_str().unwrap(), "--rev", "--output=x"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let output = sandbox.run(&["add", bare.to_str().unwrap(), "--rev", "main..x"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("invalid git rev"), "{output:?}");
}

#[test]
fn dependencies_are_added_locked_and_imported_without_network_access() {
    let sandbox = Sandbox::new();
    sandbox.write("runa.toml", MANIFEST);
    std::fs::create_dir_all(sandbox.0.join("shared")).unwrap();
    std::fs::write(
        sandbox.0.join("shared/rates.runa"),
        "@ export\n> rate() -> Int { 25 }\n",
    )
    .unwrap();
    let bare = local_repository(&sandbox, "taxlib", 40);

    let added = sandbox.run(&["add", "../shared"]);
    assert!(added.status.success(), "{added:?}");
    let added = sandbox.run(&["add", bare.to_str().unwrap()]);
    assert!(added.status.success(), "{added:?}");
    assert!(stderr(&added).contains("@ import taxlib"), "{added:?}");

    let manifest = std::fs::read_to_string(sandbox.project().join("runa.toml")).unwrap();
    assert!(
        manifest.contains("shared = { path = \"../shared\" }"),
        "{manifest}"
    );
    assert!(
        manifest.contains("taxlib = { git = \"../taxlib.git\" }"),
        "{manifest}"
    );
    let lock = std::fs::read_to_string(sandbox.project().join("runa.lock")).unwrap();
    assert!(lock.contains("shared = { path = \"../shared\" }"), "{lock}");
    let commit = lock
        .lines()
        .find_map(|line| line.split("commit = \"").nth(1))
        .map(|rest| rest.trim_end_matches(&['"', ' ', '}'][..]).to_string())
        .unwrap_or_else(|| panic!("no commit in {lock}"));
    assert_eq!(commit.len(), 40, "{lock}");
    assert!(
        !lock.contains(sandbox.0.to_str().unwrap()),
        "absolute path: {lock}"
    );

    let main = sandbox.write(
        "src/main.runa",
        "@ import shared/rates\n@ import taxlib\n@ print(show(rate() + value()))\n",
    );
    let output = sandbox.run(&[main.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "65");

    // A new upstream commit does not change what the project resolves.
    let work = sandbox.0.join("taxlib-work");
    std::fs::write(
        work.join("lib.runa"),
        "@ export\n> value() -> Int { 1000 }\n",
    )
    .unwrap();
    git(&work, &["commit", "--quiet", "-am", "update"]);
    git(&work, &["push", "--quiet", bare.to_str().unwrap(), "main"]);
    let output = sandbox.run(&[main.to_str().unwrap()]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "65",
        "{output:?}"
    );

    // Without the locked checkout, imports report how to fetch it.
    std::fs::remove_dir_all(sandbox.dependency_cache()).unwrap();
    let output = sandbox.run(&["check", "--frontend", main.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("run `runa fetch`"), "{output:?}");

    let fetched = sandbox.run(&["fetch"]);
    assert!(fetched.status.success(), "{fetched:?}");
    let output = sandbox.run(&[main.to_str().unwrap()]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "65",
        "{output:?}"
    );

    let updated = sandbox.run(&["fetch", "--update"]);
    assert!(updated.status.success(), "{updated:?}");
    let output = sandbox.run(&[main.to_str().unwrap()]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "1025",
        "{output:?}"
    );
}

/// Minimal LSP session: open `document` and return the published diagnostics.
fn lsp_diagnostics(sandbox: &Sandbox, document: &Path) -> serde_json::Value {
    let mut child = sandbox
        .command()
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut send = |value: serde_json::Value| {
        let body = serde_json::to_vec(&value).unwrap();
        input
            .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
            .unwrap();
        input.write_all(&body).unwrap();
        input.flush().unwrap();
    };
    let uri = format!("file://{}", document.display());
    send(
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}),
    );
    send(serde_json::json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    send(
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"runa","version":1,"text":std::fs::read_to_string(document).unwrap()}}}),
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || loop {
        let mut header = String::new();
        if output.read_line(&mut header).unwrap_or(0) == 0 {
            return;
        }
        let length: usize = header["Content-Length: ".len()..].trim().parse().unwrap();
        let mut blank = String::new();
        output.read_line(&mut blank).unwrap();
        let mut body = vec![0; length];
        output.read_exact(&mut body).unwrap();
        let message: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if message["method"] == "textDocument/publishDiagnostics" {
            let _ = sender.send(message["params"]["diagnostics"].clone());
            return;
        }
    });
    let diagnostics = receiver.recv_timeout(Duration::from_secs(30)).unwrap();
    let _ = child.kill();
    let _ = child.wait();
    diagnostics
}

#[test]
fn language_server_never_clones_dependencies() {
    let sandbox = Sandbox::new();
    let bare = local_repository(&sandbox, "src", 1);
    sandbox.write(
        "runa.toml",
        &format!("{MANIFEST}dep = {{ git = \"{}\" }}\n", bare.display()),
    );
    let main = sandbox.write("main.runa", "@ import dep\n@ print(show(value()))\n");
    let diagnostics = lsp_diagnostics(&sandbox, &main);
    assert!(
        !sandbox.dependency_cache().exists(),
        "the editor fetched a dependency"
    );
    assert!(
        diagnostics.to_string().contains("run `runa fetch`"),
        "{diagnostics}"
    );

    sandbox.write(
        "runa.toml",
        &format!("{MANIFEST}dep = {{ git = \"--upload-pack=touch MARK/UPLOADPACK.marker\" }}\n"),
    );
    lsp_diagnostics(&sandbox, &main);
    assert!(!sandbox.marker("UPLOADPACK.marker").exists());
    assert!(!sandbox.dependency_cache().exists());
}

#[test]
fn oversized_workbooks_are_rejected_before_sheets_are_read() {
    use zip::write::SimpleFileOptions;

    let sandbox = Sandbox::new();
    let model = sandbox.write(
        "calc.runa",
        &format!("{CALCULATION}@ calculate\n> calc(i: In) -> Out {{ Out(i.n) }}\n"),
    );
    let workbook = sandbox.project().join("input.xlsx");
    let template = sandbox.run(&[
        "template",
        model.to_str().unwrap(),
        "--format",
        "xlsx",
        "--output",
        workbook.to_str().unwrap(),
    ]);
    assert!(template.status.success(), "{template:?}");

    // Re-pack the generated workbook with one extra sheet cell far away and,
    // separately, with a highly compressible oversized entry.
    let repack = |target: &Path, extra: &dyn Fn(&mut zip::ZipWriter<std::fs::File>)| {
        let mut source = zip::ZipArchive::new(std::fs::File::open(&workbook).unwrap()).unwrap();
        let mut writer = zip::ZipWriter::new(std::fs::File::create(target).unwrap());
        for index in 0..source.len() {
            let mut entry = source.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut content = Vec::new();
            entry.read_to_end(&mut content).unwrap();
            if name.ends_with("sheet1.xml") {
                let text = String::from_utf8(content).unwrap();
                content = text
                    .replace(
                        "</sheetData>",
                        "<row r=\"1048576\"><c r=\"XFD1048576\" t=\"n\"><v>1</v></c></row></sheetData>",
                    )
                    .into_bytes();
            }
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&content).unwrap();
        }
        extra(&mut writer);
        writer.finish().unwrap();
    };
    let wide = sandbox.project().join("wide.xlsx");
    repack(&wide, &|_| {});
    let output = sandbox.run(&[
        "call",
        model.to_str().unwrap(),
        "--input",
        wide.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("cells per sheet"), "{output:?}");

    let bomb = sandbox.project().join("bomb.xlsx");
    repack(&bomb, &|writer| {
        writer
            .start_file("xl/padding.bin", SimpleFileOptions::default())
            .unwrap();
        let block = vec![0u8; 1 << 20];
        for _ in 0..300 {
            writer.write_all(&block).unwrap();
        }
    });
    let output = sandbox.run(&[
        "call",
        model.to_str().unwrap(),
        "--input",
        bomb.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("workbook unpacks to more than 256 MiB"),
        "{output:?}"
    );
}
