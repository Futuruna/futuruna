use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "futuruna-native-isolation-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(path.join("scratch")).unwrap();
        Self(path)
    }

    fn project(&self, index: usize) -> PathBuf {
        let project = self.0.join(format!("project-{index}"));
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(
            project.join("src/main.runa"),
            format!("@ print(\"project {index}\")\n"),
        )
        .unwrap();
        project
    }

    fn command(&self, project: &Path, mode: &str, cache: &str) -> Command {
        self.command_source(project, mode, cache, Path::new("src/main.runa"))
    }

    fn command_source(&self, project: &Path, mode: &str, cache: &str, source: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_runa"));
        command
            .current_dir(project)
            .arg(mode)
            .arg(source)
            .env("FUTURUNA_COMPILER_CACHE_DIR", self.0.join(cache))
            .env("FUTURUNA_COMPILER_CACHE_TRACE", "1")
            .env_remove("FUTURUNA_DISABLE_COMPILER_CACHE")
            .env("TMPDIR", self.0.join("scratch"))
            .env("TMP", self.0.join("scratch"))
            .env("TEMP", self.0.join("scratch"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn finish(child: Child) -> Output {
    finish_all(vec![child]).pop().unwrap()
}

fn finish_all(mut children: Vec<Child>) -> Vec<Output> {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if children
            .iter_mut()
            .all(|child| child.try_wait().unwrap().is_some())
        {
            return children
                .into_iter()
                .map(|child| child.wait_with_output().unwrap())
                .collect();
        }
        if Instant::now() >= deadline {
            for child in &mut children {
                child.kill().ok();
            }
            let outputs = children
                .into_iter()
                .map(|child| child.wait_with_output().unwrap())
                .collect::<Vec<_>>();
            panic!("native builds exceeded 90 seconds: {outputs:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn success(output: &Output) {
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn concurrent_same_named_entrypoints_keep_their_own_executable_and_cache() {
    for mode in ["run", "build"] {
        let fixture = Fixture::new();
        let projects = (0..4).map(|i| fixture.project(i)).collect::<Vec<_>>();
        let children = projects
            .iter()
            .map(|project| fixture.command(project, mode, "cache").spawn().unwrap())
            .collect::<Vec<_>>();
        // Reap every child before asserting, so a failure cannot leave builds
        // writing into a fixture being cleaned up.
        let outputs = finish_all(children);
        for (index, (project, output)) in projects.iter().zip(outputs).enumerate() {
            success(&output);
            let output = if mode == "build" {
                Command::new(project.join(format!("main{}", std::env::consts::EXE_SUFFIX)))
                    .output()
                    .unwrap()
            } else {
                output
            };
            success(&output);
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                format!("project {index}")
            );
            let cached = finish(fixture.command(project, "run", "cache").spawn().unwrap());
            success(&cached);
            assert!(
                String::from_utf8_lossy(&cached.stderr).contains("native hit"),
                "{cached:?}"
            );
            assert_eq!(
                String::from_utf8_lossy(&cached.stdout).trim(),
                format!("project {index}")
            );
        }
    }
}

#[test]
fn cargo_builds_keep_separate_sources_and_targets_in_the_same_working_directory() {
    let fixture = Fixture::new();
    let projects = (0..2)
        .map(|index| {
            let project = fixture.project(index);
            // sha2 is already a compiler dependency. Offline mode keeps this guard
            // independent of network availability or installing new dependencies.
            std::fs::write(
                project.join("src/main.runa"),
                format!("@ depend \"sha2\" \"0.10\"\n@ print(\"project {index}\")\n"),
            )
            .unwrap();
            project
        })
        .collect::<Vec<_>>();
    let children = projects
        .iter()
        .map(|project| {
            fixture
                .command_source(&fixture.0, "run", "cache", &project.join("src/main.runa"))
                .env("CARGO_NET_OFFLINE", "true")
                .env("CARGO_TARGET_DIR", fixture.0.join("shared-cargo-target"))
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    for (index, output) in finish_all(children).into_iter().enumerate() {
        success(&output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            format!("project {index}")
        );
        let cached = finish(
            fixture
                .command(&projects[index], "run", "cache")
                .spawn()
                .unwrap(),
        );
        success(&cached);
        assert_eq!(
            String::from_utf8_lossy(&cached.stdout).trim(),
            format!("project {index}")
        );
        assert!(
            String::from_utf8_lossy(&cached.stderr).contains("native hit"),
            "{cached:?}"
        );
    }
}

#[test]
fn cargo_check_and_run_accept_date_and_punctuation_source_names() {
    let fixture = Fixture::new();
    let project = fixture.project(0);
    let source = "@ depend \"sha2\" \"0.10\"\n@ print(\"source name preserved\")\n";
    for name in ["2026-actor-zq.runa", "!!!.runa", "2026-合同.runa"] {
        let path = project.join(name);
        std::fs::write(&path, source).unwrap();
        for mode in ["check", "run"] {
            let mut command = fixture.command_source(&project, mode, "cache", &path);
            if mode == "check" {
                command.arg("--build-deps");
            }
            let output = finish(command.env("CARGO_NET_OFFLINE", "true").spawn().unwrap());
            assert!(output.status.success(), "{mode} {name}: {output:?}");
            if mode == "run" {
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout).trim(),
                    "source name preserved"
                );
            }
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    }
}

#[test]
fn build_cannot_report_success_when_the_output_is_a_directory() {
    let fixture = Fixture::new();
    let project = fixture.project(0);
    std::fs::create_dir(project.join("main")).unwrap();
    let output = finish(fixture.command(&project, "build", "cache").spawn().unwrap());
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Error copying compiled binary"),
        "{output:?}"
    );
    assert!(project.join("main").is_dir());
}

#[test]
fn disabled_or_relocated_cache_cannot_reuse_a_legacy_scratch_binary() {
    let fixture = Fixture::new();
    let project = fixture.project(0);
    let first = finish(fixture.command(&project, "run", "cache").spawn().unwrap());
    success(&first);
    let relocated = finish(
        fixture
            .command(&project, "run", "fresh-cache")
            .spawn()
            .unwrap(),
    );
    let disabled = finish(
        fixture
            .command(&project, "run", "disabled-cache")
            .env("FUTURUNA_DISABLE_COMPILER_CACHE", "1")
            .spawn()
            .unwrap(),
    );
    for output in [relocated, disabled] {
        success(&output);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "project 0");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("using cached binary"),
            "{output:?}"
        );
    }
    assert!(!fixture
        .0
        .join("disabled-cache/compiler-artifacts-v1")
        .exists());
}

#[test]
fn a_foreign_compiler_cache_entry_cannot_fall_back_to_an_unkeyed_binary() {
    let fixture = Fixture::new();
    let project = fixture.project(0);
    success(&finish(
        fixture.command(&project, "run", "cache").spawn().unwrap(),
    ));
    let index = fixture
        .0
        .join("cache/compiler-artifacts-v1/native-index-v1");
    let entries = std::fs::read_dir(index)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1);
    let path = entries[0].path();
    let mut entry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    entry["source_graph"]["compiler_hash"] = "different compiler identity".into();
    std::fs::write(path, serde_json::to_vec(&entry).unwrap()).unwrap();
    let output = finish(fixture.command(&project, "run", "cache").spawn().unwrap());
    success(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("native miss"), "{output:?}");
    assert!(!stderr.contains("using cached binary"), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "project 0");
}
