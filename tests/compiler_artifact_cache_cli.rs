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

fn rustc_check_workspaces(cache: &Path) -> Vec<PathBuf> {
    let workspace_root = cache.join("compiler-artifacts-v1").join("rustc-check-v2");
    let mut workspaces = std::fs::read_dir(workspace_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    workspaces.sort();
    workspaces
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
    let first_workspaces = rustc_check_workspaces(&cache);
    assert_eq!(first_workspaces.len(), 1);
    assert!(
        std::fs::read_to_string(first_workspaces[0].join("check.rs"))
            .expect("read first generated Rust source")
            .contains("42")
    );
    let generated_rust_path = first_workspaces[0].join("check.rs");
    let first_generated_rust_modified = std::fs::metadata(&generated_rust_path)
        .expect("stat first generated Rust source")
        .modified()
        .expect("generated Rust modification time");

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
    let manifest_invalidated_stderr = String::from_utf8_lossy(&manifest_invalidated.stderr);
    assert!(manifest_invalidated_stderr.contains("check miss"));
    assert!(manifest_invalidated_stderr.contains("rustc validation hit"));
    assert_eq!(
        std::fs::metadata(&generated_rust_path)
            .expect("stat unchanged generated Rust source")
            .modified()
            .expect("unchanged generated Rust modification time"),
        first_generated_rust_modified
    );

    let manifest_cached = run_with_cache(&["check", source], &cache);
    assert_success(&manifest_cached);
    assert!(String::from_utf8_lossy(&manifest_cached.stderr).contains("check hit"));

    std::fs::write(&dependency, "| answer() -> 43\n").expect("change check cache dependency");
    let invalidated = run_with_cache(&["check", source], &cache);
    assert_success(&invalidated);
    let invalidated_stderr = String::from_utf8_lossy(&invalidated.stderr);
    assert!(invalidated_stderr.contains("check miss"));
    assert!(invalidated_stderr.contains("rustc validation miss"));
    assert!(!invalidated_stderr.contains("lines of Rust, cached"));
    let invalidated_workspaces = rustc_check_workspaces(&cache);
    assert_eq!(invalidated_workspaces, first_workspaces);
    assert!(
        std::fs::read_to_string(invalidated_workspaces[0].join("check.rs"))
            .expect("read rewritten generated Rust source")
            .contains("43")
    );

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

#[test]
fn rustc_validation_cache_does_not_cover_raw_rust_blocks() {
    let (root, source, _dependency, cache) = fixture_paths("raw-rust-validation-cache");
    std::fs::write(
        &source,
        "@ rust {\n    fn rust_multiply(a: i64, b: i64) -> i64 { a * b }\n}\n@ print(rust_multiply(6, 7))\n",
    )
    .expect("write raw Rust cache fixture");
    let source = source.to_str().expect("source path");

    let first = run_with_cache(&["check", source], &cache);
    assert_success(&first);
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    assert!(first_stderr.contains("rustc validation bypassed: raw Rust"));

    std::fs::write(
        root.join("runa.toml"),
        "[package]\nname = \"raw-rust-cache-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("invalidate source graph without changing generated Rust");
    let invalidated = run_with_cache(&["check", source], &cache);
    assert_success(&invalidated);
    let invalidated_stderr = String::from_utf8_lossy(&invalidated.stderr);
    assert!(invalidated_stderr.contains("check miss"));
    assert!(invalidated_stderr.contains("rustc validation bypassed: raw Rust"));
    assert!(!invalidated_stderr.contains("rustc validation hit"));

    std::fs::remove_dir_all(root).ok();
}
