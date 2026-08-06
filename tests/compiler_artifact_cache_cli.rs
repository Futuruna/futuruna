use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_PATH_ID: AtomicU64 = AtomicU64::new(0);

fn runa() -> &'static str {
    env!("CARGO_BIN_EXE_runa")
}

fn temp_root(prefix: &str) -> PathBuf {
    let unique_id = NEXT_TEMP_PATH_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "futuruna-{prefix}-{}-{}-{unique_id}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn fixture_paths(prefix: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = temp_root(prefix);
    std::fs::create_dir_all(&root).expect("create compiler cache fixture directory");
    let unique_stem = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temporary directory name");
    let source = root.join(format!("{unique_stem}.runa"));
    let dependency = root.join("domain.runa");
    let cache = root.join("cache");
    (root, source, dependency, cache)
}

fn run_with_cache(args: &[&str], cache: &Path) -> Output {
    Command::new(runa())
        .args(args)
        .env("FUTURUNA_COMPILER_CACHE_DIR", cache)
        .env("FUTURUNA_COMPILER_CACHE_TRACE", "1")
        .output()
        .expect("run runa with isolated compiler cache")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_cache_hits_and_invalidates_on_transitive_import_change() {
    let (root, source, dependency, cache) = fixture_paths("check-artifact-cache");
    std::fs::write(&source, "@ import ./domain\n@ print(answer())\n")
        .expect("write check cache source");
    std::fs::write(&dependency, "| answer() -> 42\n").expect("write check cache dependency");
    let source = source.to_str().expect("source path");

    let first = run_with_cache(&["check", source], &cache);
    assert_success(&first);
    assert!(String::from_utf8_lossy(&first.stderr).contains("check miss"));

    let second = run_with_cache(&["check", source], &cache);
    assert_success(&second);
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    assert!(second_stderr.contains("check hit"));
    assert!(second_stderr.contains("cached"));

    std::fs::write(
        root.join("runa.toml"),
        "[package]\nname = \"cache-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("add nearer manifest");
    let manifest_invalidated = run_with_cache(&["check", source], &cache);
    assert_success(&manifest_invalidated);
    assert!(String::from_utf8_lossy(&manifest_invalidated.stderr).contains("check miss"));

    let manifest_cached = run_with_cache(&["check", source], &cache);
    assert_success(&manifest_cached);
    assert!(String::from_utf8_lossy(&manifest_cached.stderr).contains("check hit"));

    std::fs::write(&dependency, "| answer() -> 43\n").expect("change check cache dependency");
    let invalidated = run_with_cache(&["check", source], &cache);
    assert_success(&invalidated);
    let invalidated_stderr = String::from_utf8_lossy(&invalidated.stderr);
    assert!(invalidated_stderr.contains("check miss"));
    assert!(!invalidated_stderr.contains("lines of Rust, cached"));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn native_cache_hits_and_executes_new_binary_after_import_change() {
    let (root, source, dependency, cache) = fixture_paths("native-artifact-cache");
    std::fs::write(&source, "@ import ./domain\n@ print(answer())\n")
        .expect("write native cache source");
    std::fs::write(&dependency, "| answer() -> 42\n").expect("write native cache dependency");
    let source = source.to_str().expect("source path");

    let first = run_with_cache(&["run", source], &cache);
    assert_success(&first);
    assert_eq!(String::from_utf8_lossy(&first.stdout).trim(), "42");
    assert!(String::from_utf8_lossy(&first.stderr).contains("native miss"));

    let second = run_with_cache(&["run", source], &cache);
    assert_success(&second);
    assert_eq!(String::from_utf8_lossy(&second.stdout).trim(), "42");
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    assert!(second_stderr.contains("native hit"));
    assert!(second_stderr.contains("source graph unchanged"));

    std::fs::write(&dependency, "| answer() -> 43\n").expect("change native cache dependency");
    let invalidated = run_with_cache(&["run", source], &cache);
    assert_success(&invalidated);
    assert_eq!(String::from_utf8_lossy(&invalidated.stdout).trim(), "43");
    assert!(String::from_utf8_lossy(&invalidated.stderr).contains("native miss"));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn check_cache_invalidates_when_local_module_shadows_manifest_dependency() {
    let (root, source, _dependency, cache) = fixture_paths("import-resolution-cache");
    let dependency_root = root.join("dependency");
    std::fs::create_dir_all(dependency_root.join("src")).expect("create dependency source tree");
    std::fs::write(
        root.join("runa.toml"),
        format!(
            "[package]\nname = \"cache-fixture\"\nversion = \"0.1.0\"\n\n[dependencies]\ndomain = {{ path = \"{}\" }}\n",
            dependency_root.display()
        ),
    )
    .expect("write manifest dependency");
    std::fs::write(dependency_root.join("src/value.runa"), "| answer() -> 42\n")
        .expect("write manifest module");
    std::fs::write(&source, "@ import domain/value\n@ print(answer())\n")
        .expect("write cache source");
    let source_text = source.to_str().expect("source path");

    let first = run_with_cache(&["check", source_text], &cache);
    assert_success(&first);
    assert!(String::from_utf8_lossy(&first.stderr).contains("check miss"));
    let second = run_with_cache(&["check", source_text], &cache);
    assert_success(&second);
    assert!(String::from_utf8_lossy(&second.stderr).contains("check hit"));

    std::fs::create_dir_all(root.join("domain")).expect("create local shadow directory");
    std::fs::write(root.join("domain/value.runa"), "| answer() -> 43\n")
        .expect("write local shadow module");
    let shadowed = run_with_cache(&["check", source_text], &cache);
    assert_success(&shadowed);
    assert!(String::from_utf8_lossy(&shadowed.stderr).contains("check miss"));

    std::fs::remove_dir_all(root).ok();
}
