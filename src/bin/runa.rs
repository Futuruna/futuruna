//! runa — The Futuruna Compiler CLI
//!
//! This binary provides the CLI interface for the Futuruna compiler.
//! Core language implementation is in the library crate (src/lib.rs).

use futuruna::*;
use serde_json;
use sha2::{Digest as ShaDigest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fmt;
use std::io::{self, BufRead, Write as IoWrite};

fn main() {
    // Use a large stack (64 MB) to handle deep recursion in comptime evaluation
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handler = match builder.spawn(main_inner) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Fatal: failed to spawn main thread: {}", e);
            std::process::exit(1);
        }
    };
    if let Err(e) = handler.join() {
        std::panic::resume_unwind(e);
    }
}

fn main_inner() {
    let args: Vec<String> = env::args().collect();

    let mut mode = "interpret"; // "interpret", "emit", "build", "run"
    let mut filename = None;
    let mut use_prelude = true;
    let mut test_compile = false; // --run flag for `runa test --run`
    let mut fmt_check = false; // --check flag for `runa fmt --check`
    let mut use_fir = false; // --fir flag for `runa emit --fir`

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--emit" if i + 1 < args.len() && args[i + 1] == "rust" => {
                mode = "emit";
                i += 2;
            }
            "--emit=rust" => {
                mode = "emit";
                i += 1;
            }
            "--build" => {
                mode = "build";
                i += 1;
            }
            "--run" if mode == "test" => {
                test_compile = true;
                i += 1;
            }
            "--check" if mode == "fmt" => {
                fmt_check = true;
                i += 1;
            }
            "--fir" if mode == "emit" => {
                use_fir = true;
                i += 1;
            }
            "--run" => {
                mode = "run";
                i += 1;
            }
            "--hashes" => {
                mode = "hashes";
                i += 1;
            }
            "--verify" => {
                mode = "verify";
                i += 1;
            }
            "--lib" => {
                mode = "lib";
                i += 1;
            }
            "--no-prelude" => {
                use_prelude = false;
                i += 1;
            }
            "--version" | "-V" => {
                println!("runa 0.1.0");
                std::process::exit(0);
            }
            "--help" | "-h" | "help" => {
                eprintln!("runa — the Futuruna compiler");
                eprintln!("Usage: runa [COMMAND] [OPTIONS] <file.runa>");
                eprintln!();
                eprintln!("Commands:");
                eprintln!("  (none)        Interpret directly (default)");
                eprintln!("  init [name]   Create a new project with runa.toml");
                eprintln!("  add <path>    Add a local dependency to runa.toml");
                eprintln!("  emit          Print generated Rust to stdout");
                eprintln!("  build         Compile to native binary");
                eprintln!("  run           Compile and execute");
                eprintln!("  lib           Compile to Rust library (no main)");
                eprintln!("  hashes        Show content hashes for all definitions");
                eprintln!("  wasm          Compile to WebAssembly (via wasm-pack)");
                eprintln!("  check         Parse and type-check without running");
                eprintln!("  verify        Generate SMT-LIB2 and verify with Z3");
                eprintln!("  audit         Discover invariant gaps and rule asymmetries");
                eprintln!("  fmt           Format source file(s)");
                eprintln!("  fmt --check   Check formatting without modifying");
                eprintln!("  lsp           Start language server (stdio)");
                eprintln!("  test          Run all tests/*.runa (interpreted)");
                eprintln!("  test --run    Run all tests/*.runa (compiled + executed)");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --version     Show version");
                eprintln!("  --no-prelude  Don't auto-import standard prelude");
                eprintln!();
                eprintln!("Examples:");
                eprintln!("  runa program.runa           Interpret");
                eprintln!("  runa run program.runa       Compile + execute");
                eprintln!("  runa check program.runa     Type-check without running");
                eprintln!("  runa emit program.runa      Show Rust output");
                eprintln!("  runa build program.runa     Compile to ./program");
                eprintln!("  runa fmt program.runa       Format source file");
                eprintln!("  runa fmt .                  Format all .runa files");
                eprintln!("  runa test                   Run all tests");
                eprintln!("  runa test --run             Run all tests (compiled)");
                eprintln!();
                eprintln!("  runa audit program.runa     Discover invariant gaps automatically");
                std::process::exit(0);
            }
            "init" => {
                mode = "init";
                i += 1;
            }
            "add" => {
                mode = "add";
                i += 1;
            }
            "emit" => {
                mode = "emit";
                i += 1;
            }
            "build" => {
                mode = "build";
                i += 1;
            }
            "run" => {
                mode = "run";
                i += 1;
            }
            "lib" => {
                mode = "lib";
                i += 1;
            }
            "hashes" => {
                mode = "hashes";
                i += 1;
            }
            "registry" => {
                mode = "registry";
                i += 1;
            }
            "wasm" => {
                mode = "wasm";
                i += 1;
            }
            "check" => {
                mode = "check";
                i += 1;
            }
            "verify" => {
                mode = "verify";
                i += 1;
            }
            "test" => {
                mode = "test";
                i += 1;
            }
            "fmt" => {
                mode = "fmt";
                i += 1;
            }
            "lsp" => {
                mode = "lsp";
                i += 1;
            }
            "audit" => {
                mode = "audit";
                i += 1;
            }
            other => {
                if other.starts_with('-') {
                    eprintln!("error: unknown option '{}'", other);
                    eprintln!("Run 'runa --help' for usage.");
                    std::process::exit(1);
                } else if filename.is_none() {
                    filename = Some(args[i].clone());
                } else {
                    eprintln!("error: unexpected argument '{}'", other);
                    eprintln!("Run 'runa --help' for usage.");
                    std::process::exit(1);
                }
                i += 1;
            }
        }
    }

    // ── runa init [name] — create new project ──
    if mode == "init" {
        let name = filename.as_deref().unwrap_or("my-project");
        runa_init(name);
        return;
    }

    // ── runa add <path> — add dependency ──
    if mode == "add" {
        if let Some(ref dep_path) = filename {
            runa_add(dep_path);
        } else {
            eprintln!("Usage: runa add <path>");
            eprintln!("  Adds a local path dependency to runa.toml");
            std::process::exit(1);
        }
        return;
    }

    // ── runa test [--run] [dir] — test runner ──
    if mode == "test" {
        let test_dir = filename.as_deref().unwrap_or("tests");
        run_tests(test_dir, use_prelude, test_compile);
        // Also run error tests if they exist and no specific dir was given
        if filename.is_none() {
            let error_dir = std::path::Path::new(test_dir).join("errors");
            if error_dir.is_dir() {
                eprintln!();
                run_tests(&error_dir.to_string_lossy(), use_prelude, false);
            }
        }
        return;
    }

    // ── runa fmt [--check] [file|dir] — formatter ──
    if mode == "fmt" {
        let target = filename.as_deref().unwrap_or(".");
        format_target(target, fmt_check);
        return;
    }

    // ── runa lsp — language server ──
    if mode == "lsp" {
        run_lsp_server();
        return;
    }

    if let Some(ref path) = filename {
        match std::fs::read_to_string(path) {
            Ok(source) => match mode {
                "emit" if use_fir => emit_rust_source_fir(&source, path, use_prelude),
                "emit" => emit_rust_source(&source, path, use_prelude),
                "build" => build_native(&source, path, false, use_prelude),
                "run" => build_native(&source, path, true, use_prelude),
                "lib" => emit_rust_lib(&source, path, use_prelude),
                "hashes" => show_hashes(&source, path),
                "registry" => update_registry(&source, path),
                "wasm" => build_wasm(&source, path, use_prelude),
                "check" => check_source(&source, path, use_prelude),
                "audit" => audit_source(&source, path, use_prelude),
                "verify" => verify_with_z3(&source, path),
                _ => run_source(&source, path, use_prelude),
            },
            Err(e) => {
                eprintln!("Error reading {}: {}", path, e);
                std::process::exit(1);
            }
        }
    } else {
        // REPL mode
        run_repl();
    }
}

// ══════════════════════════════════════════════════════════════════════════
// ── Package Manager: runa init / runa add / runa.toml ──────────────────
// ══════════════════════════════════════════════════════════════════════════

/// Dependency specification: local path or git remote
#[derive(Clone, Debug)]
enum DepSpec {
    Path(String),
    Git { url: String, rev: Option<String> },
}

/// Parsed runa.toml manifest
struct RunaManifest {
    name: String,
    version: String,
    entry: String,
    dependencies: Vec<(String, DepSpec)>,
}

/// Extract a quoted value for a given key from an inline TOML table
fn extract_toml_table_value(raw: &str, key: &str) -> Option<String> {
    if let Some(k_start) = raw.find(key) {
        let after = &raw[k_start + key.len()..];
        if let Some(eq) = after.find('=') {
            let val_part = after[eq + 1..]
                .trim()
                .trim_end_matches('}')
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches('"');
            if !val_part.is_empty() {
                return Some(val_part.to_string());
            }
        }
    }
    None
}

/// Parse a minimal runa.toml — supports [package] and [dependencies] sections
fn parse_runa_toml(toml_path: &str) -> Option<RunaManifest> {
    let content = std::fs::read_to_string(toml_path).ok()?;
    let mut name = String::new();
    let mut version = String::from("0.1.0");
    let mut entry = String::from("src/main.runa");
    let mut deps: Vec<(String, DepSpec)> = Vec::new();
    let mut section = "";

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[package]" {
            section = "package";
            continue;
        }
        if trimmed == "[dependencies]" {
            section = "deps";
            continue;
        }
        if trimmed.starts_with('[') {
            section = "";
            continue;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            let val_raw = trimmed[eq_pos + 1..].trim();
            let val = val_raw.trim_matches('"');
            match section {
                "package" => match key {
                    "name" => name = val.to_string(),
                    "version" => version = val.to_string(),
                    "entry" => entry = val.to_string(),
                    _ => {}
                },
                "deps" => {
                    if val_raw.contains("git") {
                        // Git dependency: { git = "https://...", rev = "abc" }
                        if let Some(url) = extract_toml_table_value(val_raw, "git") {
                            let rev = extract_toml_table_value(val_raw, "rev");
                            deps.push((key.to_string(), DepSpec::Git { url, rev }));
                        }
                    } else if val_raw.contains("path") {
                        if let Some(path) = extract_toml_table_value(val_raw, "path") {
                            deps.push((key.to_string(), DepSpec::Path(path)));
                        }
                    } else {
                        // Simple string path
                        deps.push((key.to_string(), DepSpec::Path(val.to_string())));
                    }
                }
                _ => {}
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(RunaManifest {
        name,
        version,
        entry,
        dependencies: deps,
    })
}

/// Find runa.toml by walking up from a directory
fn find_runa_toml(start_dir: &str) -> Option<String> {
    let mut dir = std::path::PathBuf::from(start_dir);
    loop {
        let candidate = dir.join("runa.toml");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Generate a runa.lock file from resolved dependencies.
/// Records exact resolved paths for reproducible builds.
fn write_lock_file(toml_path: &str, manifest: &RunaManifest) {
    let lock_path = toml_path.replace("runa.toml", "runa.lock");
    let mut content = String::new();
    content.push_str("# This file is auto-generated by runa. Do not edit.\n");
    content.push_str(&format!("# Generated from: {}\n\n", toml_path));
    content.push_str(&format!("[package]\nname = \"{}\"\nversion = \"{}\"\n\n", manifest.name, manifest.version));

    if !manifest.dependencies.is_empty() {
        content.push_str("[dependencies]\n");
        for (name, spec) in &manifest.dependencies {
            match spec {
                DepSpec::Path(p) => {
                    // Resolve to absolute path for lock file
                    let toml_dir = std::path::Path::new(toml_path)
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    let abs = if std::path::Path::new(p).is_absolute() {
                        p.clone()
                    } else {
                        toml_dir.join(p).to_string_lossy().to_string()
                    };
                    content.push_str(&format!("{} = {{ path = \"{}\" }}\n", name, abs));
                }
                DepSpec::Git { url, rev } => {
                    if let Some(r) = rev {
                        content.push_str(&format!("{} = {{ git = \"{}\", rev = \"{}\" }}\n", name, url, r));
                    } else {
                        content.push_str(&format!("{} = {{ git = \"{}\" }}\n", name, url));
                    }
                }
            }
        }
    }

    match std::fs::write(&lock_path, &content) {
        Ok(_) => eprintln!("  wrote {}", lock_path),
        Err(e) => eprintln!("  warning: could not write lock file: {}", e),
    }
}

/// Check if lock file exists and is newer than runa.toml.
fn lock_file_is_current(toml_path: &str) -> Option<String> {
    let lock_path = toml_path.replace("runa.toml", "runa.lock");
    let lock_meta = std::fs::metadata(&lock_path).ok()?;
    let toml_meta = std::fs::metadata(toml_path).ok()?;
    let lock_time = lock_meta.modified().ok()?;
    let toml_time = toml_meta.modified().ok()?;
    if lock_time >= toml_time {
        Some(lock_path)
    } else {
        None
    }
}

/// `runa init [name]` — scaffold a new project
fn runa_init(name: &str) {
    let project_dir = std::path::Path::new(name);
    if project_dir.exists() {
        eprintln!(
            "\x1b[1;31merror\x1b[0m: directory '{}' already exists",
            name
        );
        std::process::exit(1);
    }

    // Create directory structure
    let src_dir = project_dir.join("src");
    if let Err(e) = std::fs::create_dir_all(&src_dir) {
        eprintln!("\x1b[1;31merror\x1b[0m: cannot create directory: {}", e);
        std::process::exit(1);
    }

    // Write runa.toml
    let toml_content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
entry = "src/main.runa"

[dependencies]
"#,
        name
    );

    let toml_path = project_dir.join("runa.toml");
    if let Err(e) = std::fs::write(&toml_path, toml_content) {
        eprintln!("\x1b[1;31merror\x1b[0m: cannot write runa.toml: {}", e);
        std::process::exit(1);
    }

    // Write src/main.runa
    let main_content = format!(
        r#"-- {}: a Futuruna project

@ print("Hello from {}!")
"#,
        name, name
    );

    let main_path = src_dir.join("main.runa");
    if let Err(e) = std::fs::write(&main_path, main_content) {
        eprintln!("\x1b[1;31merror\x1b[0m: cannot write src/main.runa: {}", e);
        std::process::exit(1);
    }

    eprintln!("\x1b[1;32mCreated\x1b[0m project '{}' with:", name);
    eprintln!("  {}/runa.toml", name);
    eprintln!("  {}/src/main.runa", name);
    eprintln!();
    eprintln!("  cd {} && runa run src/main.runa", name);
}

/// Get the dependency cache directory (~/.cache/futuruna/deps/)
fn dep_cache_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join(".cache")
        .join("futuruna")
        .join("deps")
}

/// Check if a string looks like a git URL
fn is_git_url(s: &str) -> bool {
    s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("git@")
        || s.ends_with(".git")
}

/// Extract repo name from a git URL (e.g., "https://github.com/user/mylib" → "mylib")
fn repo_name_from_url(url: &str) -> String {
    let clean = url.trim_end_matches(".git").trim_end_matches('/');
    clean.rsplit('/').next().unwrap_or("dep").to_string()
}

/// Hash a git URL to create a unique cache directory name
fn git_cache_key(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Ensure a git dependency is cloned/updated in the cache. Returns the local path.
fn ensure_git_dep(url: &str, rev: Option<&str>) -> Result<String, String> {
    let cache = dep_cache_dir();
    let repo_dir = cache.join(git_cache_key(url));

    if repo_dir.exists() {
        // Already cloned — fetch latest if no specific rev pinned
        if rev.is_none() {
            let status = std::process::Command::new("git")
                .args(["fetch", "--quiet"])
                .current_dir(&repo_dir)
                .status();
            if let Ok(s) = status {
                if s.success() {
                    let _ = std::process::Command::new("git")
                        .args(["reset", "--hard", "origin/HEAD"])
                        .current_dir(&repo_dir)
                        .status();
                }
            }
        }
    } else {
        // Clone
        std::fs::create_dir_all(&cache).map_err(|e| format!("cannot create cache dir: {}", e))?;
        eprintln!("\x1b[1;36mCloning\x1b[0m {} ...", url);
        let status = std::process::Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--depth",
                "1",
                url,
                &repo_dir.to_string_lossy(),
            ])
            .status()
            .map_err(|e| format!("git clone failed: {}", e))?;
        if !status.success() {
            return Err(format!("git clone failed for {}", url));
        }
    }

    // Checkout specific rev if specified
    if let Some(r) = rev {
        let status = std::process::Command::new("git")
            .args(["checkout", "--quiet", r])
            .current_dir(&repo_dir)
            .status()
            .map_err(|e| format!("git checkout failed: {}", e))?;
        if !status.success() {
            return Err(format!("git checkout '{}' failed", r));
        }
    }

    // Get the current commit hash for pinning
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .map_err(|e| format!("git rev-parse failed: {}", e))?;
    let _commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok(repo_dir.to_string_lossy().to_string())
}

/// Resolve a DepSpec to a local filesystem path (cloning git deps if needed)
fn resolve_dep_to_path(spec: &DepSpec, toml_dir: &str) -> Option<String> {
    match spec {
        DepSpec::Path(p) => {
            if std::path::Path::new(p).is_absolute() {
                Some(p.clone())
            } else {
                Some(format!("{}/{}", toml_dir, p))
            }
        }
        DepSpec::Git { url, rev } => match ensure_git_dep(url, rev.as_deref()) {
            Ok(path) => Some(path),
            Err(e) => {
                eprintln!("\x1b[1;31merror\x1b[0m: {}", e);
                None
            }
        },
    }
}

/// `runa add <path-or-url>` — add a dependency to runa.toml
fn runa_add(dep_arg: &str) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let toml_path = match find_runa_toml(&cwd.to_string_lossy()) {
        Some(p) => p,
        None => {
            eprintln!(
                "\x1b[1;31merror\x1b[0m: no runa.toml found in current or parent directories"
            );
            eprintln!("  Run 'runa init <name>' to create a project first");
            std::process::exit(1);
        }
    };

    let (dep_name, dep_line) = if is_git_url(dep_arg) {
        // Git dependency
        let name = repo_name_from_url(dep_arg);

        // Verify we can clone it
        match ensure_git_dep(dep_arg, None) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("\x1b[1;31merror\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }

        let line = format!("{} = {{ git = \"{}\" }}\n", name, dep_arg);
        (name, line)
    } else {
        // Local path dependency
        let abs_dep = if std::path::Path::new(dep_arg).is_absolute() {
            std::path::PathBuf::from(dep_arg)
        } else {
            cwd.join(dep_arg)
        };

        if !abs_dep.exists() {
            eprintln!("\x1b[1;31merror\x1b[0m: path '{}' does not exist", dep_arg);
            std::process::exit(1);
        }

        let name = abs_dep
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "dep".to_string());

        let toml_dir = std::path::Path::new(&toml_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let rel_path = pathdiff_relative(&abs_dep, toml_dir);
        let line = format!("{} = {{ path = \"{}\" }}\n", name, rel_path);
        (name, line)
    };

    // Read existing content
    let content = match std::fs::read_to_string(&toml_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[1;31merror\x1b[0m: cannot read {}: {}", toml_path, e);
            std::process::exit(1);
        }
    };

    // Check if dependency already exists
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&dep_name) && trimmed.contains('=') {
            eprintln!(
                "\x1b[1;33mwarning\x1b[0m: dependency '{}' already exists in runa.toml",
                dep_name
            );
            return;
        }
    }

    // Append to [dependencies] section
    let new_content = append_dep_to_toml(&content, &dep_line);

    if let Err(e) = std::fs::write(&toml_path, new_content) {
        eprintln!("\x1b[1;31merror\x1b[0m: cannot write {}: {}", toml_path, e);
        std::process::exit(1);
    }

    let display = if is_git_url(dep_arg) {
        dep_arg.to_string()
    } else {
        let abs_dep = if std::path::Path::new(dep_arg).is_absolute() {
            std::path::PathBuf::from(dep_arg)
        } else {
            cwd.join(dep_arg)
        };
        let toml_dir = std::path::Path::new(&toml_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        pathdiff_relative(&abs_dep, toml_dir)
    };
    eprintln!(
        "\x1b[1;32mAdded\x1b[0m dependency '{}' → {}",
        dep_name, display
    );

    // Regenerate lock file
    if let Some(manifest) = parse_runa_toml(&toml_path) {
        write_lock_file(&toml_path, &manifest);
    }
}

/// Append a dependency line to TOML content's [dependencies] section
fn append_dep_to_toml(content: &str, dep_line: &str) -> String {
    if content.contains("[dependencies]") {
        let mut result = String::new();
        let mut in_deps = false;
        let mut added = false;
        for line in content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim() == "[dependencies]" {
                in_deps = true;
            } else if in_deps && !added {
                if line.trim().starts_with('[') || line.trim().is_empty() {
                    if line.trim().starts_with('[') {
                        let len = result.len();
                        result.truncate(len - line.len() - 1);
                        result.push_str(dep_line);
                        result.push_str(line);
                        result.push('\n');
                    } else {
                        result.push_str(dep_line);
                    }
                    added = true;
                    in_deps = false;
                }
            }
        }
        if !added {
            result.push_str(dep_line);
        }
        result
    } else {
        let mut result = content.to_string();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("\n[dependencies]\n");
        result.push_str(dep_line);
        result
    }
}

/// Compute a relative path from `base` to `target` (simple implementation)
fn pathdiff_relative(target: &std::path::Path, base: &std::path::Path) -> String {
    // Canonicalize both if possible
    let target_c = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let base_c = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());

    let target_parts: Vec<_> = target_c.components().collect();
    let base_parts: Vec<_> = base_c.components().collect();

    // Find common prefix length
    let common = target_parts
        .iter()
        .zip(base_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Build relative path: go up from base, then down to target
    let mut rel = String::new();
    for _ in common..base_parts.len() {
        if !rel.is_empty() {
            rel.push('/');
        }
        rel.push_str("..");
    }
    for part in &target_parts[common..] {
        if !rel.is_empty() {
            rel.push('/');
        }
        rel.push_str(&part.as_os_str().to_string_lossy());
    }
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel
    }
}

/// Transpile Futuruna → Rust → native binary. If `execute` is true, run it after.
fn find_rust_tool(name: &str) -> String {
    // Check PATH first
    if let Ok(output) = std::process::Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    // Check ~/.cargo/bin/
    if let Some(home) = std::env::var_os("HOME") {
        let cargo_path = format!("{}/.cargo/bin/{}", home.to_string_lossy(), name);
        if std::path::Path::new(&cargo_path).exists() {
            return cargo_path;
        }
    }
    // Fallback: try rustup toolchain path
    if let Some(home) = std::env::var_os("HOME") {
        let rustup_path = format!(
            "{}/.rustup/toolchains/stable-aarch64-apple-darwin/bin/{}",
            home.to_string_lossy(),
            name
        );
        if std::path::Path::new(&rustup_path).exists() {
            return rustup_path;
        }
    }
    // Last resort: bare name, let OS resolve
    name.to_string()
}

fn source_dir_for(filename: &str) -> Option<String> {
    std::path::Path::new(filename)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
}

/// Run type checking and display any errors as structured diagnostics.
/// Returns true if there were errors (and already printed them).
fn run_type_check(stmts: &[Stmt], source: &str, filename: &str) -> bool {
    let diags = TypeChecker::check_with_diagnostics(stmts, source_dir_for(filename), source);
    if diags.is_empty() {
        return false;
    }
    let use_color = should_use_color();
    let (red, reset) = if use_color {
        ("\x1b[1;31m", "\x1b[0m")
    } else {
        ("", "")
    };
    let count = diags.len();
    eprintln!(
        "{}error{}: {} type error{} in {}:",
        red,
        reset,
        count,
        if count == 1 { "" } else { "s" },
        filename
    );
    for diag in &diags {
        eprint!("{}", diag.display(source, filename, use_color));
    }
    true
}

fn build_native(source: &str, filename: &str, execute: bool, use_prelude: bool) {
    use std::process::Command;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };
            // Pre-codegen type checking (M16)
            if run_type_check(&stmts, source, filename) {
                std::process::exit(1);
            }

            let mut cg = RustCodegen::new();
            // Set source directory for @ import resolution
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let code = cg.emit_program(&stmts);

            let stem = std::path::Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("tau_out");

            // Incremental compilation: hash the generated code, skip if unchanged
            let code_hash = hash_string(&code);

            if cg.cargo_deps.is_empty() {
                // No dependencies — use raw rustc (fast path)
                let cache_dir = std::env::temp_dir().join("runa-cache");
                std::fs::create_dir_all(&cache_dir).ok();
                let hash_path = cache_dir
                    .join(format!("{}.hash", stem))
                    .to_string_lossy()
                    .to_string();
                let cached_bin = cache_dir.join(stem).to_string_lossy().to_string();

                // Check if binary exists and hash matches (incremental: skip recompilation)
                let cached_hash = std::fs::read_to_string(&hash_path).unwrap_or_default();
                let bin_path = if execute {
                    cached_bin.clone()
                } else {
                    stem.to_string()
                };

                if cached_hash.trim() == code_hash && std::path::Path::new(&cached_bin).exists() {
                    eprintln!(
                        "runa: {} unchanged (hash #{}), using cached binary",
                        filename,
                        &code_hash[..8]
                    );
                    if execute {
                        let status = Command::new(&cached_bin).status().unwrap_or_else(|e| {
                            eprintln!("Error running {}: {}", cached_bin, e);
                            std::process::exit(1);
                        });
                        std::process::exit(status.code().unwrap_or(1));
                    } else if !execute {
                        // Copy cached binary to output location
                        std::fs::copy(&cached_bin, &bin_path).ok();
                        eprintln!("runa: {} -> {} (cached)", filename, bin_path);
                    }
                    return;
                }

                let rs_path = cache_dir
                    .join(format!("{}.rs", stem))
                    .to_string_lossy()
                    .to_string();

                std::fs::write(&rs_path, &code).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", rs_path, e);
                    std::process::exit(1);
                });

                let rustc_bin = find_rust_tool("rustc");
                let rustc = Command::new(&rustc_bin)
                    .args(&[&rs_path, "-o", &cached_bin, "--edition", "2021"])
                    .output();

                match rustc {
                    Ok(output) => {
                        if !output.status.success() {
                            eprintln!("\x1b[1;31merror\x1b[0m: generated Rust did not compile (this is a Futuruna compiler bug)");
                            eprintln!("  Source: {}", filename);
                            eprintln!("  Generated: {}", rs_path);
                            eprintln!();
                            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                            std::process::exit(1);
                        }
                        // Save hash for incremental compilation
                        std::fs::write(&hash_path, &code_hash).ok();
                        if execute {
                            let status = Command::new(&cached_bin).status().unwrap_or_else(|e| {
                                eprintln!("Error running {}: {}", cached_bin, e);
                                std::process::exit(1);
                            });
                            std::process::exit(status.code().unwrap_or(1));
                        } else {
                            // Copy from cache to output location
                            if bin_path != cached_bin {
                                std::fs::copy(&cached_bin, &bin_path).ok();
                            }
                            eprintln!(
                                "runa: {} -> {} ({} lines of Rust)",
                                filename,
                                bin_path,
                                code.lines().count()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error running rustc: {}. Is Rust installed?", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Has dependencies — generate Cargo project in .runa-build/
                let build_dir = format!(".runa-build/{}", stem);
                let src_dir = format!("{}/src", build_dir);
                std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
                    eprintln!("Error creating {}: {}", src_dir, e);
                    std::process::exit(1);
                });

                // Write main.rs
                let main_rs = format!("{}/main.rs", src_dir);
                std::fs::write(&main_rs, &code).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", main_rs, e);
                    std::process::exit(1);
                });

                // Generate Cargo.toml (sanitize inputs to prevent TOML injection)
                let safe_stem: String = stem
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let mut cargo_toml = format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    safe_stem
                );
                for (crate_name, version) in &cg.cargo_deps {
                    // Validate crate name: only alphanumeric, hyphens, underscores
                    let safe_name: String = crate_name
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    // M13c: inline table syntax for deps with features (e.g. tokio)
                    // Pass through as-is — already validated by the compiler
                    if version.starts_with('{') {
                        cargo_toml.push_str(&format!("{} = {}\n", safe_name, version));
                    } else {
                        // Validate version: only digits, dots, hyphens, plus
                        let safe_version: String = version
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '+')
                            .collect();
                        cargo_toml.push_str(&format!("{} = \"{}\"\n", safe_name, safe_version));
                    }
                }
                let cargo_path = format!("{}/Cargo.toml", build_dir);
                std::fs::write(&cargo_path, &cargo_toml).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", cargo_path, e);
                    std::process::exit(1);
                });

                // Build with cargo
                let cargo_bin = find_rust_tool("cargo");
                let cargo = Command::new(&cargo_bin)
                    .args(&["build", "--release"])
                    .current_dir(&build_dir)
                    .output();

                match cargo {
                    Ok(output) => {
                        if !output.status.success() {
                            eprintln!("\x1b[1;31merror\x1b[0m: generated Rust did not compile (this is a Futuruna compiler bug)");
                            eprintln!("  Source: {}", filename);
                            eprintln!("  Generated: {}/src/main.rs", build_dir);
                            eprintln!();
                            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                            std::process::exit(1);
                        }
                        let cargo_bin = format!("{}/target/release/{}", build_dir, stem);
                        if execute {
                            let status = Command::new(&cargo_bin).status().unwrap_or_else(|e| {
                                eprintln!("Error running {}: {}", cargo_bin, e);
                                std::process::exit(1);
                            });
                            std::process::exit(status.code().unwrap_or(1));
                        } else {
                            // Copy binary to current directory
                            std::fs::copy(&cargo_bin, stem).unwrap_or_else(|e| {
                                eprintln!("Error copying binary: {}", e);
                                std::process::exit(1);
                            });
                            let dep_count = cg.cargo_deps.len();
                            eprintln!(
                                "runa: {} -> {} ({} lines of Rust, {} dep{})",
                                filename,
                                stem,
                                code.lines().count(),
                                dep_count,
                                if dep_count == 1 { "" } else { "s" }
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error running cargo: {}. Is Rust installed?", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Build Futuruna → WASM via wasm-pack. Generates Cargo project with wasm-bindgen.
fn build_wasm(source: &str, filename: &str, use_prelude: bool) {
    use std::process::Command;
    use std::time::Instant;

    let start = Instant::now();

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };

            let mut cg = RustCodegen::new();
            cg.lib_mode = true;
            cg.wasm_mode = true;
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let code = cg.emit_program(&stmts);

            let stem = std::path::Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("tau_wasm");
            let safe_stem: String = stem
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();

            // Generate Cargo project in .runa-build/<name>-wasm/
            let build_dir = format!(".runa-build/{}-wasm", safe_stem);
            let src_dir = format!("{}/src", build_dir);
            std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
                eprintln!("Error creating {}: {}", src_dir, e);
                std::process::exit(1);
            });

            // Write lib.rs (not main.rs — WASM is a library)
            let lib_rs = format!("{}/lib.rs", src_dir);
            std::fs::write(&lib_rs, &code).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", lib_rs, e);
                std::process::exit(1);
            });

            // Generate Cargo.toml with wasm-bindgen + user deps
            let mut cargo_toml = format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                 [lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n\
                 [package.metadata.wasm-pack.profile.release]\nwasm-opt = false\n\n\
                 [dependencies]\n\
                 wasm-bindgen = \"0.2\"\n",
                safe_stem
            );
            for (crate_name, version) in &cg.cargo_deps {
                let safe_name: String = crate_name
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if version.starts_with('{') {
                    cargo_toml.push_str(&format!("{} = {}\n", safe_name, version));
                } else {
                    let safe_version: String = version
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '+')
                        .collect();
                    cargo_toml.push_str(&format!("{} = \"{}\"\n", safe_name, safe_version));
                }
            }
            let cargo_path = format!("{}/Cargo.toml", build_dir);
            std::fs::write(&cargo_path, &cargo_toml).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", cargo_path, e);
                std::process::exit(1);
            });

            eprintln!(
                "runa wasm: {} → {}/src/lib.rs ({} lines of Rust)",
                filename,
                build_dir,
                code.lines().count()
            );

            // Build with wasm-pack (ensure cargo is in PATH)
            let wasm_pack = find_tool("wasm-pack");
            let path_with_cargo = {
                let home = std::env::var("HOME").unwrap_or_default();
                let cargo_bin = format!("{}/.cargo/bin", home);
                match std::env::var("PATH") {
                    Ok(p) => format!("{}:{}", cargo_bin, p),
                    Err(_) => cargo_bin,
                }
            };
            let output = Command::new(&wasm_pack)
                .args(&["build", "--target", "web", "--release"])
                .env("PATH", &path_with_cargo)
                .current_dir(&build_dir)
                .output();

            let elapsed = start.elapsed();
            match output {
                Ok(o) if o.status.success() => {
                    let pkg_dir = format!("{}/pkg", build_dir);
                    eprintln!("\x1b[1;32mwasm ok\x1b[0m: {} → {}/", filename, pkg_dir);
                    // List generated files
                    if let Ok(entries) = std::fs::read_dir(&pkg_dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name();
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            if size > 0 {
                                let size_str = if size > 1024 * 1024 {
                                    format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
                                } else if size > 1024 {
                                    format!("{:.1} KB", size as f64 / 1024.0)
                                } else {
                                    format!("{} B", size)
                                };
                                eprintln!("  {} ({})", name.to_string_lossy(), size_str);
                            }
                        }
                    }
                    eprintln!("\x1b[2m[{:.1}s]\x1b[0m", elapsed.as_secs_f64());
                }
                Ok(o) => {
                    eprintln!("\x1b[1;31mwasm build failed\x1b[0m:");
                    eprintln!("{}", String::from_utf8_lossy(&o.stderr));
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("\x1b[1;31merror\x1b[0m: cannot run wasm-pack: {}", e);
                    eprintln!("  Install with: cargo install wasm-pack");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Find a tool in PATH or common locations
fn find_tool(name: &str) -> String {
    // Check PATH first
    if let Ok(output) = std::process::Command::new("which").arg(name).output() {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    // Check ~/.cargo/bin/
    let cargo_path = format!(
        "{}/.cargo/bin/{}",
        std::env::var("HOME").unwrap_or_default(),
        name
    );
    if std::path::Path::new(&cargo_path).exists() {
        return cargo_path;
    }
    // Fallback: hope it's in PATH
    name.to_string()
}

fn run_source(source: &str, filename: &str, use_prelude: bool) {
    // Tokenize
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();

    // Parse
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            // Prepend standard prelude (types + utility functions)
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };

            let stmt_count = stmts.len();
            let fn_count = stmts
                .iter()
                .filter(|s| matches!(s, Stmt::Defn(Defn::Fn { .. })))
                .count();
            let type_count = stmts
                .iter()
                .filter(|s| matches!(s, Stmt::TypeDecl(_)))
                .count();
            let rule_count = stmts.iter().filter(|s| matches!(s, Stmt::Rule(_))).count();

            eprintln!(
                "runa: parsed {} ({} statements: {} functions, {} types, {} rules)",
                filename, stmt_count, fn_count, type_count, rule_count
            );

            // Pre-codegen type checking (M16)
            if run_type_check(&stmts, source, filename) {
                std::process::exit(1);
            }

            // Evaluate
            let mut interp = Interpreter::new();
            // Set source directory for @ use resolution
            if let Some(parent) = std::path::Path::new(filename).parent() {
                interp.source_dir = Some(parent.to_string_lossy().to_string());
            }
            let mut env = interp.default_env();
            let result = interp.run_program(&stmts, &mut env);

            match result {
                Value::Unit => {}
                _ => println!("=> {}", result),
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Run all .runa files in a directory, report pass/fail summary.
fn run_tests(dir: &str, use_prelude: bool, compile_mode: bool) {
    use std::process::Command;
    use std::time::Instant;

    let path = std::path::Path::new(dir);
    if !path.is_dir() {
        eprintln!("\x1b[1;31merror\x1b[0m: '{}' is not a directory", dir);
        std::process::exit(1);
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap_or_else(|e| {
            eprintln!("Cannot read {}: {}", dir, e);
            std::process::exit(1);
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "runa").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        eprintln!("No .runa files found in {}", dir);
        std::process::exit(1);
    }

    let total = entries.len();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mode_label = if compile_mode {
        "runa test --run"
    } else {
        "runa test"
    };
    eprintln!(
        "\x1b[1m{}\x1b[0m: running {} tests from {}/\n",
        mode_label, total, dir
    );

    let suite_start = Instant::now();

    // Find our own binary for subprocess mode
    let self_bin = if compile_mode {
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("runa"))
    } else {
        std::path::PathBuf::new() // unused
    };

    for entry in &entries {
        let file_path = entry.path();
        let name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let test_start = Instant::now();

        if compile_mode {
            // Compile+execute mode: run as subprocess `runa run <file>`
            let file_str = file_path.to_string_lossy().to_string();
            let mut cmd = Command::new(&self_bin);
            cmd.args(&["run", &file_str]);
            if !use_prelude {
                cmd.arg("--no-prelude");
            }
            // Suppress stdout (test output) but capture stderr for errors
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.output() {
                Ok(output) => {
                    let elapsed = test_start.elapsed();
                    let ms = elapsed.as_millis();
                    let time_str = if ms >= 1000 {
                        format!("{:.1}s", elapsed.as_secs_f64())
                    } else {
                        format!("{}ms", ms)
                    };
                    if output.status.success() {
                        eprintln!(
                            "  \x1b[1;32mPASS\x1b[0m  {} \x1b[2m({})\x1b[0m",
                            name, time_str
                        );
                        passed += 1;
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let err_line = stderr
                            .lines()
                            .find(|l| {
                                l.contains("error")
                                    || l.contains("failed")
                                    || l.contains("panicked")
                            })
                            .unwrap_or("(compilation or runtime error)");
                        eprintln!(
                            "  \x1b[1;31mFAIL\x1b[0m  {} — {} \x1b[2m({})\x1b[0m",
                            name,
                            err_line.trim(),
                            time_str
                        );
                        failed += 1;
                        failures.push(name);
                    }
                }
                Err(e) => {
                    eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — cannot execute: {}", name, e);
                    failed += 1;
                    failures.push(name);
                }
            }
        } else {
            // Interpret mode: run in-process
            match std::fs::read_to_string(&file_path) {
                Ok(source) => {
                    // Check for negative test markers: -- expect-error: <substring>
                    let expected_errors: Vec<String> = source
                        .lines()
                        .filter_map(|line| {
                            let trimmed = line.trim();
                            if trimmed.starts_with("-- expect-error:") {
                                Some(trimmed["-- expect-error:".len()..].trim().to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let is_negative_test = !expected_errors.is_empty();

                    if is_negative_test {
                        // Negative test: run via subprocess so we can capture stderr
                        let self_bin = std::env::current_exe()
                            .unwrap_or_else(|_| std::path::PathBuf::from("runa"));
                        let file_str = file_path.to_string_lossy().to_string();
                        let output = std::process::Command::new(&self_bin)
                            .args(&["check", &file_str])
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::piped())
                            .output();

                        let elapsed = test_start.elapsed();
                        let ms = elapsed.as_millis();
                        let time_str = if ms >= 1000 {
                            format!("{:.1}s", elapsed.as_secs_f64())
                        } else {
                            format!("{}ms", ms)
                        };

                        match output {
                            Ok(out) => {
                                let stderr = String::from_utf8_lossy(&out.stderr);
                                let did_error = !out.status.success();
                                let all_found = expected_errors
                                    .iter()
                                    .all(|expected| stderr.contains(expected.as_str()));

                                if did_error && all_found {
                                    eprintln!("  \x1b[1;32mPASS\x1b[0m  {} \x1b[2m(expect-error, {})\x1b[0m", name, time_str);
                                    passed += 1;
                                } else if !did_error {
                                    eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — expected error but program succeeded \x1b[2m({})\x1b[0m", name, time_str);
                                    failed += 1;
                                    failures.push(name);
                                } else {
                                    let missing: Vec<&String> = expected_errors
                                        .iter()
                                        .filter(|e| !stderr.contains(e.as_str()))
                                        .collect();
                                    eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — error occurred but missing expected text: {:?} \x1b[2m({})\x1b[0m",
                                        name, missing, time_str);
                                    failed += 1;
                                    failures.push(name);
                                }
                            }
                            Err(e) => {
                                eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — cannot execute: {} \x1b[2m({})\x1b[0m", name, e, time_str);
                                failed += 1;
                                failures.push(name);
                            }
                        }
                    } else {
                        // Positive test: run in-process
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut lexer = Lexer::new(&source);
                            let tokens = lexer.tokenize();
                            let mut parser = Parser::new(tokens, &source);
                            match parser.parse_program() {
                                Ok(user_stmts) => {
                                    let stmts = if use_prelude {
                                        prepend_prelude(parse_prelude(), &user_stmts)
                                    } else {
                                        user_stmts
                                    };
                                    let mut interp = Interpreter::new();
                                    if let Some(parent) = file_path.parent() {
                                        interp.source_dir =
                                            Some(parent.to_string_lossy().to_string());
                                    }
                                    let mut env = interp.default_env();
                                    interp.run_program(&stmts, &mut env);
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }));

                        let elapsed = test_start.elapsed();
                        let ms = elapsed.as_millis();
                        let time_str = if ms >= 1000 {
                            format!("{:.1}s", elapsed.as_secs_f64())
                        } else {
                            format!("{}ms", ms)
                        };
                        match result {
                            Ok(Ok(())) => {
                                eprintln!(
                                    "  \x1b[1;32mPASS\x1b[0m  {} \x1b[2m({})\x1b[0m",
                                    name, time_str
                                );
                                passed += 1;
                            }
                            Ok(Err(e)) => {
                                eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — parse error: {} \x1b[2m({})\x1b[0m", name, e, time_str);
                                failed += 1;
                                failures.push(name);
                            }
                            Err(_) => {
                                eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — runtime panic \x1b[2m({})\x1b[0m", name, time_str);
                                failed += 1;
                                failures.push(name);
                            }
                        }
                    } // end positive test
                }
                Err(e) => {
                    eprintln!("  \x1b[1;31mFAIL\x1b[0m  {} — cannot read: {}", name, e);
                    failed += 1;
                    failures.push(name);
                }
            }
        }
    }

    let suite_elapsed = suite_start.elapsed();
    let suite_secs = suite_elapsed.as_secs_f64();
    let suite_time = if suite_secs >= 1.0 {
        format!("{:.1}s", suite_secs)
    } else {
        format!("{}ms", suite_elapsed.as_millis())
    };

    eprintln!();
    if failed == 0 {
        eprintln!(
            "\x1b[1;32mAll {} tests passed\x1b[0m in {}.",
            total, suite_time
        );
    } else {
        eprintln!(
            "\x1b[1;31m{} of {} tests failed\x1b[0m in {}:",
            failed, total, suite_time
        );
        for f in &failures {
            eprintln!("  - {}", f);
        }
        std::process::exit(1);
    }
}

fn run_repl() {
    println!("Futuruna v0.1 — the language designed by measuring consciousness");
    println!("Runes: > define  | rule  # type  @ annotate  = bind");
    println!("Type :quit to exit, :env to show bindings\n");

    let mut interp = Interpreter::new();
    let mut env = interp.default_env();
    // Load standard prelude into REPL environment
    let prelude = parse_prelude();
    if !prelude.is_empty() {
        interp.run_program(&prelude, &mut env);
    }
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("tau> ");
        let _ = stdout.flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == ":quit" || trimmed == ":q" {
                    println!("Goodbye.");
                    break;
                }
                if trimmed == ":env" {
                    for (k, v) in &env.bindings {
                        if !matches!(v, Value::Builtin(_)) {
                            println!("  {} = {}", k, v);
                        }
                    }
                    continue;
                }
                if trimmed == ":rules" {
                    for (name, _rule) in &interp.rules {
                        println!("  | {}(...)", name);
                    }
                    continue;
                }

                // Support multi-line input with { }
                let mut full_input = line.clone();
                let mut brace_depth: i32 = full_input.chars().filter(|&c| c == '{').count() as i32
                    - full_input.chars().filter(|&c| c == '}').count() as i32;
                while brace_depth > 0 {
                    print!("...  ");
                    let _ = stdout.flush();
                    let mut cont = String::new();
                    match stdin.lock().read_line(&mut cont) {
                        Ok(0) => break,
                        Ok(_) => {
                            brace_depth += cont.chars().filter(|&c| c == '{').count() as i32;
                            brace_depth -= cont.chars().filter(|&c| c == '}').count() as i32;
                            full_input.push_str(&cont);
                        }
                        Err(_) => break,
                    }
                }

                // Tokenize + Parse + Eval
                let mut lexer = Lexer::new(&full_input);
                let tokens = lexer.tokenize();
                let mut parser = Parser::new(tokens, &full_input);
                match parser.parse_program() {
                    Ok(stmts) => {
                        let result = interp.run_program(&stmts, &mut env);
                        match result {
                            Value::Unit => {}
                            _ => println!("=> {}", result),
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}

// ============================================================================
// PART 8: RUST CODEGEN (Futuruna → Rust transpiler)
// ============================================================================

// ============================================================================
// PART 7: Z3 SMT-LIB2 VERIFICATION
// ============================================================================
//
// The ? rune invokes formal verification. This generates SMT-LIB2 from
// Futuruna invariants and shells out to Z3 to prove or find counterexamples.
//
// Mapping:
//   # type declarations  → Z3 datatypes (declare-datatype)
//   > functions          → Z3 functions (define-fun)
//   | invariants         → Z3 assertions (assert)
//   = bindings           → Z3 constants (define-const)
//   ? prove              → Z3 (check-sat) invocation

/// Generate SMT-LIB2 from a Futuruna expression
fn expr_to_smt(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Lit(Literal::Int(n)) => format!("{}", n),
        ExprKind::Lit(Literal::Float(f)) => format!("{}", f),
        ExprKind::Lit(Literal::Bool(b)) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        ExprKind::Lit(Literal::Str(s)) => format!("\"{}\"", s),
        ExprKind::Var(name) => name.clone(),
        ExprKind::BinOp(op, lhs, rhs) => {
            let l = expr_to_smt(lhs);
            let r = expr_to_smt(rhs);
            match op.as_str() {
                "+" => format!("(+ {} {})", l, r),
                "-" => format!("(- {} {})", l, r),
                "*" => format!("(* {} {})", l, r),
                "/" => format!("(div {} {})", l, r),
                "%" => format!("(mod {} {})", l, r),
                "<" => format!("(< {} {})", l, r),
                ">" => format!("(> {} {})", l, r),
                "<=" => format!("(<= {} {})", l, r),
                ">=" => format!("(>= {} {})", l, r),
                "==" => format!("(= {} {})", l, r),
                "!=" => format!("(not (= {} {}))", l, r),
                "&&" => format!("(and {} {})", l, r),
                "||" => format!("(or {} {})", l, r),
                _ => format!("; unsupported op: {}", op),
            }
        }
        ExprKind::UnOp(op, inner) => {
            let i = expr_to_smt(inner);
            match op.as_str() {
                "!" => format!("(not {})", i),
                "-" => format!("(- {})", i),
                _ => format!("; unsupported unop: {}", op),
            }
        }
        ExprKind::App(func, args) => {
            let fname = expr_to_smt(func);
            let smt_args: Vec<String> = args.iter().map(|a| expr_to_smt(a)).collect();
            if smt_args.is_empty() {
                fname
            } else {
                format!("({} {})", fname, smt_args.join(" "))
            }
        }
        ExprKind::If(cond, then_, else_) => {
            format!(
                "(ite {} {} {})",
                expr_to_smt(cond),
                expr_to_smt(then_),
                expr_to_smt(else_)
            )
        }
        // Field access: obj.field → (field obj) — Z3 selector function
        ExprKind::Field(obj, field) => {
            format!("({} {})", field, expr_to_smt(obj))
        }
        // Match expression → nested ite with Z3 (_ is Ctor) testers
        ExprKind::Match(scrutinee, arms) => {
            let scrut = expr_to_smt(scrutinee);
            smt_match_arms(&scrut, arms, 0)
        }
        ExprKind::Block(stmts) => {
            if stmts.len() == 1 {
                if let Stmt::Expr(inner) = &stmts[0] {
                    return expr_to_smt(inner);
                }
            }
            // Multi-statement block: inline let-bindings for Z3
            if stmts.len() >= 2 {
                let mut local_lets: Vec<(String, String)> = Vec::new();
                let mut last_expr = None;
                for s in stmts {
                    match s {
                        Stmt::Bind(Pat::Var(name), _, e) => {
                            local_lets.push((name.clone(), expr_to_smt(e)));
                        }
                        Stmt::Expr(e) => {
                            last_expr = Some(expr_to_smt(e));
                        }
                        _ => {}
                    }
                }
                if let Some(body) = last_expr {
                    if local_lets.is_empty() {
                        return body;
                    }
                    let binds: Vec<String> = local_lets
                        .iter()
                        .map(|(n, v)| format!("({} {})", n, v))
                        .collect();
                    return format!("(let ({}) {})", binds.join(" "), body);
                }
            }
            format!("; block expr (not yet translatable)")
        }
        _ => format!("; complex expr (not yet translatable)"),
    }
}

/// Convert match arms to nested Z3 ite expressions with (_ is Ctor) testers
fn smt_match_arms(scrut: &str, arms: &[MatchArm], idx: usize) -> String {
    if idx >= arms.len() {
        return format!("; match: no arm matched");
    }
    let arm = &arms[idx];
    let body = expr_to_smt(&arm.body);
    let guarded_body = if let Some(ref guard) = arm.guard {
        let g = expr_to_smt(guard);
        if idx + 1 < arms.len() {
            format!(
                "(ite {} {} {})",
                g,
                body,
                smt_match_arms(scrut, arms, idx + 1)
            )
        } else {
            body.clone()
        }
    } else {
        body.clone()
    };

    match &arm.pat {
        Pat::Wild => guarded_body,
        Pat::Var(name) => format!("(let (({} {})) {})", name, scrut, guarded_body),
        Pat::Con(ctor, pats) if pats.is_empty() => {
            if idx + 1 < arms.len() {
                let rest = smt_match_arms(scrut, arms, idx + 1);
                format!(
                    "(ite ((_ is {}) {}) {} {})",
                    ctor, scrut, guarded_body, rest
                )
            } else {
                guarded_body
            }
        }
        Pat::Con(ctor, pats) => {
            let mut binds = Vec::new();
            for (i, p) in pats.iter().enumerate() {
                if let Pat::Var(v) = p {
                    binds.push(format!("({} ({}_f{} {}))", v, ctor, i, scrut));
                }
            }
            let inner = if binds.is_empty() {
                guarded_body.clone()
            } else {
                format!("(let ({}) {})", binds.join(" "), guarded_body)
            };
            if idx + 1 < arms.len() {
                format!(
                    "(ite ((_ is {}) {}) {} {})",
                    ctor,
                    scrut,
                    inner,
                    smt_match_arms(scrut, arms, idx + 1)
                )
            } else {
                inner
            }
        }
        Pat::NamedCon(ctor, named_pats) => {
            let mut binds = Vec::new();
            for (field, p) in named_pats {
                if let Pat::Var(v) = p {
                    binds.push(format!("({} ({} {}))", v, field, scrut));
                }
            }
            let inner = if binds.is_empty() {
                guarded_body.clone()
            } else {
                format!("(let ({}) {})", binds.join(" "), guarded_body)
            };
            if idx + 1 < arms.len() {
                format!(
                    "(ite ((_ is {}) {}) {} {})",
                    ctor,
                    scrut,
                    inner,
                    smt_match_arms(scrut, arms, idx + 1)
                )
            } else {
                inner
            }
        }
        Pat::Lit(lit) => {
            let lit_smt = match lit {
                Literal::Int(n) => format!("{}", n),
                Literal::Bool(b) => {
                    if *b {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                _ => format!("; unsupported literal pattern"),
            };
            if idx + 1 < arms.len() {
                format!(
                    "(ite (= {} {}) {} {})",
                    scrut,
                    lit_smt,
                    guarded_body,
                    smt_match_arms(scrut, arms, idx + 1)
                )
            } else {
                guarded_body
            }
        }
        _ => format!("; unsupported match pattern"),
    }
}

/// Infer a Z3 sort for a Futuruna expression (basic, no ADT awareness)
fn infer_smt_sort(expr: &Expr) -> &'static str {
    match &expr.kind {
        ExprKind::Lit(Literal::Int(_)) => "Int",
        ExprKind::Lit(Literal::Float(_)) => "Real",
        ExprKind::Lit(Literal::Bool(_)) => "Bool",
        ExprKind::BinOp(op, _, _) => match op.as_str() {
            "<" | ">" | "<=" | ">=" | "==" | "!=" | "&&" | "||" => "Bool",
            _ => "Int",
        },
        ExprKind::UnOp(op, _) => {
            if op == "!" {
                "Bool"
            } else {
                "Int"
            }
        }
        _ => "Int",
    }
}

/// Infer Z3 sort with ADT awareness — returns owned String (ADT type name or "Int"/"Bool")
fn infer_smt_sort_adts(expr: &Expr, ctor_to_type: &BTreeMap<String, String>) -> String {
    match &expr.kind {
        ExprKind::Lit(Literal::Int(_)) => "Int".into(),
        ExprKind::Lit(Literal::Float(_)) => "Real".into(),
        ExprKind::Lit(Literal::Bool(_)) => "Bool".into(),
        ExprKind::BinOp(op, _, _) => match op.as_str() {
            "<" | ">" | "<=" | ">=" | "==" | "!=" | "&&" | "||" => "Bool".into(),
            _ => "Int".into(),
        },
        ExprKind::UnOp(op, _) => {
            if op == "!" {
                "Bool".into()
            } else {
                "Int".into()
            }
        }
        ExprKind::Var(name) => ctor_to_type
            .get(name)
            .cloned()
            .unwrap_or_else(|| "Int".into()),
        ExprKind::App(func, _) => {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                if let Some(ty) = ctor_to_type.get(name) {
                    return ty.clone();
                }
            }
            "Int".into()
        }
        _ => "Int".into(),
    }
}

/// Emit a Z3 declare-datatype for a Futuruna ADT.
/// Handles nullary constructors, positional fields, and named fields.
fn emit_z3_datatype(name: &str, variants: &[Variant]) -> String {
    let mut ctors = Vec::new();
    for v in variants {
        if v.fields.is_empty() {
            ctors.push(v.name.clone());
        } else {
            let fields: Vec<String> = v
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let fname = if v.positional {
                        format!("{}_f{}", v.name, i)
                    } else {
                        f.name.clone()
                    };
                    let sort = ty_to_smt_sort(&f.ty);
                    format!("({} {})", fname, sort)
                })
                .collect();
            ctors.push(format!("({} {})", v.name, fields.join(" ")));
        }
    }
    format!("(declare-datatype {} ({}))", name, ctors.join(" "))
}

/// Map a Futuruna type annotation to a Z3 sort name
fn ty_to_smt_sort(ty: &Ty) -> String {
    match ty {
        Ty::Name(n) => match n.as_str() {
            "Int" => "Int".into(),
            "Float" => "Real".into(),
            "Bool" => "Bool".into(),
            "String" => "String".into(),
            other => other.to_string(), // ADT name
        },
        _ => "Int".into(),
    }
}

/// Collect free variables from an expression
/// Collect truly free variables: referenced vars minus locally bound ones.
/// `bound` tracks names defined in enclosing scopes (lambda params, let bindings).
fn collect_true_free_vars(expr: &Expr, free: &mut BTreeSet<String>, bound: &BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        ExprKind::Lit(_) => {}
        ExprKind::BinOp(_, lhs, rhs) => {
            collect_true_free_vars(lhs, free, bound);
            collect_true_free_vars(rhs, free, bound);
        }
        ExprKind::UnOp(_, inner) => collect_true_free_vars(inner, free, bound),
        ExprKind::App(func, args) => {
            collect_true_free_vars(func, free, bound);
            for a in args {
                collect_true_free_vars(a, free, bound);
            }
        }
        ExprKind::If(c, t, e) => {
            collect_true_free_vars(c, free, bound);
            collect_true_free_vars(t, free, bound);
            collect_true_free_vars(e, free, bound);
        }
        ExprKind::Field(obj, _) => collect_true_free_vars(obj, free, bound),
        ExprKind::Index(arr, idx) => {
            collect_true_free_vars(arr, free, bound);
            collect_true_free_vars(idx, free, bound);
        }
        ExprKind::Match(scrut, arms) => {
            collect_true_free_vars(scrut, free, bound);
            for arm in arms {
                let mut arm_bound = bound.clone();
                collect_pattern_names(&arm.pat, &mut arm_bound);
                collect_true_free_vars(&arm.body, free, &arm_bound);
                if let Some(ref g) = arm.guard {
                    collect_true_free_vars(g, free, &arm_bound);
                }
            }
        }
        ExprKind::Lambda(params, body) => {
            let mut inner_bound = bound.clone();
            for p in params {
                inner_bound.insert(p.name.clone());
            }
            collect_true_free_vars(body, free, &inner_bound);
        }
        ExprKind::Block(stmts) => {
            let mut block_bound = bound.clone();
            for s in stmts {
                match s {
                    Stmt::Bind(pat, _, e) | Stmt::MonadicBind(pat, _, e) => {
                        collect_true_free_vars(e, free, &block_bound);
                        collect_pattern_names(pat, &mut block_bound);
                    }
                    Stmt::Expr(e) => collect_true_free_vars(e, free, &block_bound),
                    Stmt::For(var, iter, body_stmts) => {
                        collect_true_free_vars(iter, free, &block_bound);
                        let mut for_bound = block_bound.clone();
                        for_bound.insert(var.clone());
                        for s in body_stmts {
                            match s {
                                Stmt::Bind(p, _, e) | Stmt::MonadicBind(p, _, e) => {
                                    collect_true_free_vars(e, free, &for_bound);
                                    collect_pattern_names(p, &mut for_bound);
                                }
                                Stmt::Expr(e) => collect_true_free_vars(e, free, &for_bound),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        ExprKind::List(elems) => {
            for e in elems {
                collect_true_free_vars(e, free, bound);
            }
        }
        _ => {}
    }
}

fn collect_pattern_names(pat: &Pat, names: &mut BTreeSet<String>) {
    match pat {
        Pat::Var(n) => {
            names.insert(n.clone());
        }
        Pat::Con(_, pats) => {
            for p in pats {
                collect_pattern_names(p, names);
            }
        }
        Pat::NamedCon(_, fields) => {
            for (_, p) in fields {
                collect_pattern_names(p, names);
            }
        }
        Pat::As(inner, alias) => {
            collect_pattern_names(inner, names);
            names.insert(alias.clone());
        }
        _ => {}
    }
}

fn collect_free_vars(expr: &Expr, vars: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            vars.insert(name.clone());
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            collect_free_vars(lhs, vars);
            collect_free_vars(rhs, vars);
        }
        ExprKind::UnOp(_, inner) => collect_free_vars(inner, vars),
        ExprKind::App(func, args) => {
            collect_free_vars(func, vars);
            for a in args {
                collect_free_vars(a, vars);
            }
        }
        ExprKind::If(c, t, e) => {
            collect_free_vars(c, vars);
            collect_free_vars(t, vars);
            collect_free_vars(e, vars);
        }
        ExprKind::Field(obj, _) => collect_free_vars(obj, vars),
        ExprKind::Match(scrut, arms) => {
            collect_free_vars(scrut, vars);
            for arm in arms {
                collect_free_vars(&arm.body, vars);
                if let Some(ref g) = arm.guard {
                    collect_free_vars(g, vars);
                }
            }
        }
        ExprKind::Lambda(_, body) => collect_free_vars(body, vars),
        ExprKind::Block(stmts) => {
            for s in stmts {
                match s {
                    Stmt::Bind(_, _, e) | Stmt::Expr(e) | Stmt::MonadicBind(_, _, e) => {
                        collect_free_vars(e, vars);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Audit: discover invariant gaps, rule asymmetries, and paradoxes automatically.
///
/// Works from the **rule graph** — types, values, and resolution chains — not string
/// heuristics. Rules returning the same ADT type are in the same semantic domain.
/// Rules resolved via exception-override-default chains reveal explicit tensions.
/// Invariant coverage analysis shows which rules are untested.
fn audit_source(source: &str, filename: &str, use_prelude: bool) {
    // Phase 1: Parse, resolve imports, register all rules via the interpreter
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    let stmts = match parser.parse_program() {
        Ok(user_stmts) => {
            if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    };

    let mut interp = Interpreter::new();
    if let Some(parent) = std::path::Path::new(filename).parent() {
        interp.source_dir = Some(parent.to_string_lossy().to_string());
    }
    let mut env = interp.default_env();

    // Filter out Prove statements — we don't want verification output during audit
    let audit_stmts: Vec<Stmt> = stmts
        .into_iter()
        .filter(|s| !matches!(s, Stmt::Prove { .. }))
        .collect();
    let _ = interp.run_program(&audit_stmts, &mut env);

    // Phase 2: Evaluate all zero-arg rules, classify by return VALUE TYPE
    struct RuleResult {
        name: String,
        value: Value,
        value_str: String,
        /// The type domain: "Bool", "Int", "String", or ADT parent type name
        type_domain: String,
        /// For Constructor values: the specific variant name
        constructor: Option<String>,
        is_bool: bool,
        is_true: bool,
        param_count: usize,
        /// How the rule resolved: "clause", "default", "conditional_default", "exception"
        resolution: String,
    }

    // Build constructor → parent type map from interpreter's registered types
    let ctor_to_parent: BTreeMap<String, String> = {
        let mut map = BTreeMap::new();
        for (ctor_name, parent_type) in &interp.ctor_to_type {
            map.insert(ctor_name.clone(), parent_type.clone());
        }
        map
    };

    // Classify a Value into its type domain
    let classify_value = |val: &Value| -> (String, Option<String>) {
        match val {
            Value::Bool(_) => ("Bool".into(), None),
            Value::Int(_) => ("Int".into(), None),
            Value::Float(_) => ("Float".into(), None),
            Value::Str(_) => ("String".into(), None),
            Value::Constructor(name, _) | Value::NamedConstructor(name, _) => {
                let parent = ctor_to_parent
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "?".into());
                (parent, Some(name.clone()))
            }
            Value::List(_) => ("List".into(), None),
            _ => ("Other".into(), None),
        }
    };

    // Snapshot rule metadata before evaluation (avoids borrow conflicts with try_rule_call)
    let rules_snapshot: Vec<(String, Rule)> = interp.rules.clone();

    // Determine resolution path for a rule (which layer resolved it)
    let rule_resolution = |rule_name: &str, snapshot: &[(String, Rule)]| -> String {
        let matching: Vec<&Rule> = snapshot
            .iter()
            .filter(|(n, _)| n == rule_name)
            .map(|(_, r)| r)
            .collect();
        let has_exception = matching.iter().any(|r| matches!(r, Rule::Exception { .. }));
        let has_cond_default = matching.iter().any(|r| {
            matches!(
                r,
                Rule::Default {
                    condition: Some(_),
                    ..
                }
            )
        });
        let has_default = matching.iter().any(|r| {
            matches!(
                r,
                Rule::Default {
                    condition: None,
                    ..
                }
            )
        });
        let has_clause = matching.iter().any(|r| matches!(r, Rule::Clause { .. }));
        let layers = matching.len();
        if has_exception {
            format!(
                "exception (overrides {} layer{})",
                layers,
                if layers > 1 { "s" } else { "" }
            )
        } else if has_cond_default && has_default {
            format!("conditional default ({} layers)", layers)
        } else if has_default {
            "default".into()
        } else if has_clause {
            "clause".into()
        } else {
            "unknown".into()
        }
    };

    let mut results: Vec<RuleResult> = Vec::new();
    let mut seen_names: BTreeSet<String> = BTreeSet::new();
    let mut rule_names: Vec<String> = Vec::new();
    for (name, _) in &rules_snapshot {
        if seen_names.insert(name.clone()) {
            rule_names.push(name.clone());
        }
    }

    for rule_name in &rule_names {
        let param_count = rules_snapshot
            .iter()
            .find(|(n, _)| n == rule_name)
            .and_then(|(_, rule)| {
                let head = match rule {
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. } => head,
                    Rule::Scope { .. } => return None,
                };
                match &head.kind {
                    ExprKind::App(_, args) => Some(
                        args.iter()
                            .filter(|a| matches!(a.kind, ExprKind::Var(_)))
                            .count(),
                    ),
                    ExprKind::Var(_) => Some(0),
                    _ => None,
                }
            })
            .unwrap_or(0);

        if param_count == 0 {
            if let Some(val) = interp.try_rule_call(rule_name, &[], &env) {
                let val_str = format!("{}", val);
                let (type_domain, constructor) = classify_value(&val);
                let is_bool = matches!(&val, Value::Bool(_));
                let is_true = matches!(&val, Value::Bool(true));
                let resolution = rule_resolution(rule_name, &rules_snapshot);
                results.push(RuleResult {
                    name: rule_name.clone(),
                    value: val.clone(),
                    value_str: val_str,
                    type_domain,
                    constructor,
                    is_bool,
                    is_true,
                    param_count,
                    resolution,
                });
            }
        } else {
            results.push(RuleResult {
                name: rule_name.clone(),
                value: Value::Unit,
                value_str: format!("({} params)", param_count),
                type_domain: "Parameterized".into(),
                constructor: None,
                is_bool: false,
                is_true: false,
                param_count,
                resolution: rule_resolution(rule_name, &rules_snapshot),
            });
        }
    }

    // =========================================================================
    // Phase 3: TYPE-GRAPH ANALYSIS — group by value type, not by name
    // =========================================================================

    #[derive(Debug)]
    enum FindingKind {
        Paradox,    // Direct contradiction in rules
        Tension,    // Same type domain, conflicting values, overlapping scope
        Asymmetry,  // Same suffix across entities, different truth values
        Gap,        // Uncovered rules, missing counterparts
        Consistent, // Symmetric agreement
    }

    struct Finding {
        kind: FindingKind,
        severity: u8,
        title: String,
        rules_involved: Vec<String>,
        explanation: String,
    }

    let mut findings: Vec<Finding> = Vec::new();

    // ── 3a: TYPE DOMAIN ANALYSIS ──
    // Group rules by their return type. Rules in the same ADT domain are
    // semantically related — the TYPE SYSTEM tells us this, not name heuristics.
    let mut type_domains: BTreeMap<String, Vec<&RuleResult>> = BTreeMap::new();
    for r in &results {
        if r.param_count == 0 {
            type_domains
                .entry(r.type_domain.clone())
                .or_default()
                .push(r);
        }
    }

    // Within each non-Bool ADT domain, find value asymmetries
    // e.g., in PowerHolder domain: treaty_power -> SharedPower, executive_power -> ExclusiveTo(Executive)
    for (domain, rules) in &type_domains {
        if domain == "Bool"
            || domain == "Int"
            || domain == "Float"
            || domain == "String"
            || domain == "List"
            || domain == "Other"
            || domain == "Parameterized"
        {
            continue;
        }

        // Sub-group by constructor variant
        let mut by_variant: BTreeMap<String, Vec<&RuleResult>> = BTreeMap::new();
        for r in rules {
            let key = r.constructor.as_deref().unwrap_or("?").to_string();
            by_variant.entry(key).or_default().push(r);
        }

        // If there are multiple variants in use, report the domain structure
        if by_variant.len() >= 2 && rules.len() >= 3 {
            let variant_summary: Vec<String> = by_variant
                .iter()
                .map(|(v, rs)| format!("{} ({})", v, rs.len()))
                .collect();

            // Find the minority variant — that's the interesting one
            let mut variants_sorted: Vec<(&String, &Vec<&RuleResult>)> =
                by_variant.iter().collect();
            variants_sorted.sort_by_key(|(_, rs)| rs.len());

            for (variant, variant_rules) in
                &variants_sorted[..std::cmp::min(2, variants_sorted.len())]
            {
                if variant_rules.len()
                    <= variants_sorted.last().map(|(_, rs)| rs.len()).unwrap_or(0) / 2
                {
                    // This variant is a minority — it's the exception in this domain
                    let rule_names: Vec<String> =
                        variant_rules.iter().map(|r| r.name.clone()).collect();
                    let severity = std::cmp::min(85, 55 + (rules.len() as u8 * 3));
                    findings.push(Finding {
                        kind: FindingKind::Asymmetry,
                        severity,
                        title: format!("Type domain {}: {} is the exception", domain, variant),
                        rules_involved: rule_names.clone(),
                        explanation: format!(
                            "In the {} domain ({} rules), most rules resolve to other variants [{}], \
                             but {} resolve to {}:\n{}",
                            domain, rules.len(),
                            variant_summary.join(", "),
                            variant_rules.len(), variant,
                            variant_rules.iter().map(|r| format!("       | {}() -> {}", r.name, r.value_str)).collect::<Vec<_>>().join("\n"),
                        ),
                    });
                }
            }
        }
    }

    // ── 3b: BOOL DOMAIN — symmetric pairs (name-based, works well) ──
    // Extract entity prefix from a rule name
    let entity_prefixes = [
        "congress",
        "house",
        "senate",
        "president",
        "vp",
        "states",
        "judiciary",
        "federal",
        "executive",
        "legislative",
        "judicial",
        "supreme_court",
        "treaty",
        "amendment",
        "impeach",
    ];
    let extract_prefix = |name: &str| -> Option<(String, String)> {
        for prefix in &entity_prefixes {
            if name.starts_with(prefix) {
                let suffix = name
                    .strip_prefix(prefix)
                    .unwrap_or("")
                    .trim_start_matches('_');
                if !suffix.is_empty() {
                    return Some((prefix.to_string(), suffix.to_string()));
                }
            }
        }
        if let Some(pos) = name.find('_') {
            let (prefix, rest) = name.split_at(pos);
            let suffix = rest.trim_start_matches('_');
            if !suffix.is_empty() && prefix.len() >= 2 {
                return Some((prefix.to_string(), suffix.to_string()));
            }
        }
        None
    };

    let mut entity_groups: BTreeMap<String, Vec<&RuleResult>> = BTreeMap::new();
    let mut suffix_groups: BTreeMap<String, Vec<(String, &RuleResult)>> = BTreeMap::new();
    for r in &results {
        if let Some((prefix, suffix)) = extract_prefix(&r.name) {
            entity_groups.entry(prefix.clone()).or_default().push(r);
            suffix_groups
                .entry(suffix.clone())
                .or_default()
                .push((prefix, r));
        }
    }

    for (suffix, group) in &suffix_groups {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let (ref ent_a, rule_a) = group[i];
                let (ref ent_b, rule_b) = group[j];
                if !rule_a.is_bool || !rule_b.is_bool {
                    continue;
                }
                if rule_a.is_true != rule_b.is_true {
                    let severity = if suffix.contains("pardon")
                        || suffix.contains("impeach")
                        || suffix.contains("veto")
                        || suffix.contains("war")
                    {
                        85
                    } else {
                        60
                    };
                    findings.push(Finding {
                        kind: FindingKind::Asymmetry,
                        severity,
                        title: format!("{} vs {} on '{}'", ent_a, ent_b, suffix),
                        rules_involved: vec![rule_a.name.clone(), rule_b.name.clone()],
                        explanation: format!(
                            "{}() = {} but {}() = {}.\n     {} has this while {} does not.",
                            rule_a.name,
                            rule_a.value_str,
                            rule_b.name,
                            rule_b.value_str,
                            if rule_a.is_true { ent_a } else { ent_b },
                            if rule_a.is_true { ent_b } else { ent_a }
                        ),
                    });
                } else {
                    findings.push(Finding {
                        kind: FindingKind::Consistent,
                        severity: 10,
                        title: format!("{} and {} agree on '{}'", ent_a, ent_b, suffix),
                        rules_involved: vec![rule_a.name.clone(), rule_b.name.clone()],
                        explanation: format!(
                            "{}() = {} and {}() = {}",
                            rule_a.name, rule_a.value_str, rule_b.name, rule_b.value_str
                        ),
                    });
                }
            }
        }
    }

    // ── 3c: PARADOX DETECTION — can/cannot contradictions ──
    let mut bool_rules: BTreeMap<String, bool> = BTreeMap::new();
    for r in &results {
        if r.is_bool && r.param_count == 0 {
            bool_rules.insert(r.name.clone(), r.is_true);
        }
    }
    for (name, val) in &bool_rules {
        if name.contains("_can_") {
            let negated = name.replace("_can_", "_cannot_");
            if let Some(neg_val) = bool_rules.get(&negated) {
                if val == neg_val {
                    findings.push(Finding {
                        kind: FindingKind::Paradox,
                        severity: 95,
                        title: format!(
                            "{} and {} both {}",
                            name,
                            negated,
                            if *val { "True" } else { "False" }
                        ),
                        rules_involved: vec![name.clone(), negated.clone()],
                        explanation: format!(
                            "{}() = {} and {}() = {} -- a direct logical contradiction.",
                            name, val, negated, neg_val
                        ),
                    });
                }
            }
        }
    }

    // ── 3d: RULE RESOLUTION TENSION — rules with exception overrides ──
    // If a rule has multiple layers (exception overrides default), the override
    // IS the tension — made explicit. Show what the default would have been.
    for r in &results {
        if r.param_count > 0 {
            continue;
        }
        if !r.resolution.starts_with("exception") {
            continue;
        }

        // Find what the default/clause layer says (evaluate without exceptions)
        let matching: Vec<Rule> = rules_snapshot
            .iter()
            .filter(|(n, _)| n == &r.name)
            .map(|(_, rule)| rule.clone())
            .collect();

        // Get the default/clause value by skipping exceptions
        let default_val = {
            let mut dv: Option<Value> = None;
            for rule in &matching {
                match rule {
                    Rule::Default {
                        value,
                        condition: None,
                        ..
                    } => {
                        dv = Some(interp.eval(value, &env));
                        break;
                    }
                    Rule::Clause {
                        body: Some(body), ..
                    } => {
                        dv = Some(interp.eval(body, &env));
                        break;
                    }
                    Rule::Clause { body: None, .. } => {
                        dv = Some(Value::Bool(true));
                        break;
                    }
                    _ => {}
                }
            }
            dv
        };

        if let Some(ref dval) = default_val {
            let dval_str = format!("{}", dval);
            if dval_str != r.value_str {
                let severity = 78;
                findings.push(Finding {
                    kind: FindingKind::Tension,
                    severity,
                    title: format!("Override: {} (exception changes outcome)", r.name),
                    rules_involved: vec![r.name.clone()],
                    explanation: format!(
                        "The default rule says {}() -> {} but an exception overrides it to {}.\n\
                         The exception exists because the general rule fails in a specific case.\n\
                         Resolution: {} ({})",
                        r.name, dval_str, r.value_str, r.value_str, r.resolution
                    ),
                });
            }
        }
    }

    // ── 3e: INVARIANT COVERAGE — which rules are tested, which aren't ──
    let invariant_count = interp.invariants.len();
    let mut covered_rules: BTreeSet<String> = BTreeSet::new();

    // Scan invariant subjects and predicates for rule references
    for (_inv_name, (subject, predicate)) in &interp.invariants {
        collect_rule_refs(subject, &mut covered_rules);
        collect_rule_refs(predicate, &mut covered_rules);
    }

    // Build binding → referenced rules map from `= name = expr` statements,
    // then transitively expand covered_rules through bindings.
    // Without this, `= pardon = president_can_pardon()` + `? pardon_exists`
    // would only mark "pardon" as covered, not "president_can_pardon".
    let mut binding_refs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for stmt in &audit_stmts {
        if let Stmt::Bind(Pat::Var(name), _, expr) = stmt {
            let mut refs = BTreeSet::new();
            collect_rule_refs(expr, &mut refs);
            if !refs.is_empty() {
                binding_refs.insert(name.clone(), refs);
            }
        }
    }
    // Transitive closure: if a covered name is a binding, also cover the rules it references
    let mut changed = true;
    while changed {
        changed = false;
        let snapshot: Vec<String> = covered_rules.iter().cloned().collect();
        for name in &snapshot {
            if let Some(refs) = binding_refs.get(name) {
                for r in refs {
                    if covered_rules.insert(r.clone()) {
                        changed = true;
                    }
                }
            }
        }
    }

    // Find uncovered zero-arg Bool rules that could be tested
    let uncovered: Vec<&RuleResult> = results
        .iter()
        .filter(|r| r.param_count == 0 && r.is_bool && !covered_rules.contains(&r.name))
        .collect();

    if !uncovered.is_empty() && invariant_count > 0 {
        // Only report uncovered rules if the file HAS invariants (otherwise everything is uncovered)
        let total_bool = results
            .iter()
            .filter(|r| r.param_count == 0 && r.is_bool)
            .count();
        let global_coverage_pct = if total_bool > 0 {
            ((total_bool - uncovered.len()) * 100) / total_bool
        } else {
            100
        };

        // Group uncovered rules by entity for cleaner output
        let mut uncovered_by_entity: BTreeMap<String, Vec<&RuleResult>> = BTreeMap::new();
        for r in &uncovered {
            let entity = extract_prefix(&r.name)
                .map(|(p, _)| p)
                .unwrap_or_else(|| "other".into());
            uncovered_by_entity.entry(entity).or_default().push(r);
        }

        for (entity, rules) in &uncovered_by_entity {
            if rules.len() < 2 {
                continue;
            } // single uncovered rules aren't interesting
            let rule_list: Vec<String> = rules
                .iter()
                .map(|r| format!("{}() -> {}", r.name, r.value_str))
                .collect();
            findings.push(Finding {
                kind: FindingKind::Gap,
                severity: 45,
                title: format!(
                    "Uncovered: {} has {} untested rules ({}% global coverage)",
                    entity,
                    rules.len(),
                    global_coverage_pct
                ),
                rules_involved: rules.iter().map(|r| r.name.clone()).collect(),
                explanation: format!(
                    "{} invariants exist but {} '{}' rules have no ? proof:\n{}",
                    invariant_count,
                    rules.len(),
                    entity,
                    rule_list
                        .iter()
                        .map(|s| format!("       | {}", s))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            });
        }
    }

    // ── 3f: BOOL POWER/ENFORCEMENT — structural ──
    // True `_can_` rules without any corresponding False restriction in same entity
    for r in &results {
        if !r.is_bool || !r.is_true || r.param_count > 0 {
            continue;
        }
        if !r.name.contains("_can_") {
            continue;
        }

        let r_prefix = extract_prefix(&r.name).map(|(p, _)| p);
        // Does the same entity have ANY False rule with _can_?
        let has_restriction = results.iter().any(|other| {
            if other.name == r.name || !other.is_bool || other.is_true || other.param_count > 0 {
                return false;
            }
            let o_prefix = extract_prefix(&other.name).map(|(p, _)| p);
            r_prefix == o_prefix && other.name.contains("_can_")
        });

        if !has_restriction {
            // This entity has powers but no restrictions at all
            let entity = r_prefix.clone().unwrap_or_default();
            // Check how many True _can_ rules this entity has
            let power_count = results
                .iter()
                .filter(|o| {
                    o.is_bool
                        && o.is_true
                        && o.param_count == 0
                        && o.name.contains("_can_")
                        && extract_prefix(&o.name).map(|(p, _)| p) == r_prefix
                })
                .count();

            if power_count >= 3 {
                // Only report if entity has several unrestricted powers (pattern, not noise)
                let severity = std::cmp::min(80, 50 + power_count as u8 * 4);
                findings.push(Finding {
                    kind: FindingKind::Gap,
                    severity,
                    title: format!("{}: {} powers granted, 0 restrictions", entity, power_count),
                    rules_involved: vec![r.name.clone()],
                    explanation: format!(
                        "The {} entity has {} rules granting powers (can_X = True) but no \
                         rules restricting powers (can_Y = False). Every grant without a \
                         corresponding limit is a potential gap.",
                        entity, power_count
                    ),
                });
                // Only report once per entity — skip subsequent rules for same entity
                continue;
            }
        }
    }

    // =========================================================================
    // Phase 4: Sort, deduplicate, output
    // =========================================================================
    findings.sort_by(|a, b| b.severity.cmp(&a.severity));

    // Deduplicate
    let mut seen_pairs: BTreeSet<String> = BTreeSet::new();
    findings.retain(|f| {
        let mut key_parts = f.rules_involved.clone();
        key_parts.sort();
        let key = format!("{:?}:{:?}", std::mem::discriminant(&f.kind), key_parts);
        seen_pairs.insert(key)
    });

    let paradox_count = findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::Paradox))
        .count();
    let asymmetry_count = findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::Asymmetry))
        .count();
    let gap_count = findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::Gap))
        .count();
    let tension_count = findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::Tension))
        .count();
    let consistent_count = findings
        .iter()
        .filter(|f| matches!(f.kind, FindingKind::Consistent))
        .count();

    // Type domain summary
    let adt_domains: Vec<(&String, &Vec<&RuleResult>)> = type_domains
        .iter()
        .filter(|(d, _)| {
            ![
                "Bool",
                "Int",
                "Float",
                "String",
                "List",
                "Other",
                "Parameterized",
            ]
            .contains(&d.as_str())
        })
        .collect();

    // Header
    println!();
    println!("\x1b[1;36m======================================================\x1b[0m");
    println!("\x1b[1m  runa audit\x1b[0m -- automated gap discovery");
    println!("\x1b[1;36m======================================================\x1b[0m");
    println!();
    println!("  Source: {}", filename);
    println!(
        "  Rules: {} total ({} zero-arg, {} parameterized)",
        results.len(),
        results.iter().filter(|r| r.param_count == 0).count(),
        results.iter().filter(|r| r.param_count > 0).count()
    );
    if !adt_domains.is_empty() {
        println!(
            "  Type domains: Bool ({}), {}",
            type_domains.get("Bool").map(|v| v.len()).unwrap_or(0),
            adt_domains
                .iter()
                .map(|(d, rs)| format!("{} ({})", d, rs.len()))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let covered_bool_count = results
        .iter()
        .filter(|r| r.param_count == 0 && r.is_bool && covered_rules.contains(&r.name))
        .count();
    let total_bool_count = results
        .iter()
        .filter(|r| r.param_count == 0 && r.is_bool)
        .count();
    let header_pct = if total_bool_count > 0 {
        (covered_bool_count * 100) / total_bool_count
    } else {
        100
    };
    println!(
        "  Invariants: {}, covering {} Bool rules ({}%)",
        invariant_count, covered_bool_count, header_pct
    );
    println!(
        "  Findings: {} paradox, {} tension, {} asymmetry, {} gap, {} consistent",
        paradox_count, tension_count, asymmetry_count, gap_count, consistent_count
    );
    println!();

    // Output
    let kind_order: Vec<(&str, fn(&FindingKind) -> bool, &str)> = vec![
        (
            "PARADOXES",
            (|k: &FindingKind| matches!(k, FindingKind::Paradox)) as fn(&FindingKind) -> bool,
            "\x1b[1;31m",
        ),
        (
            "TENSIONS",
            (|k: &FindingKind| matches!(k, FindingKind::Tension)) as fn(&FindingKind) -> bool,
            "\x1b[1;33m",
        ),
        (
            "ASYMMETRIES",
            (|k: &FindingKind| matches!(k, FindingKind::Asymmetry)) as fn(&FindingKind) -> bool,
            "\x1b[1;35m",
        ),
        (
            "GAPS",
            (|k: &FindingKind| matches!(k, FindingKind::Gap)) as fn(&FindingKind) -> bool,
            "\x1b[1;34m",
        ),
        (
            "CONSISTENT",
            (|k: &FindingKind| matches!(k, FindingKind::Consistent)) as fn(&FindingKind) -> bool,
            "\x1b[2m",
        ),
    ];

    for (label, filter, color) in &kind_order {
        let group: Vec<&Finding> = findings.iter().filter(|f| filter(&f.kind)).collect();
        if group.is_empty() {
            continue;
        }
        println!("{}-- {} ({}) --\x1b[0m", color, label, group.len());
        println!();
        for (i, f) in group.iter().enumerate() {
            let icon = match f.kind {
                FindingKind::Paradox => "!!",
                FindingKind::Tension => "~!",
                FindingKind::Asymmetry => "<>",
                FindingKind::Gap => "??",
                FindingKind::Consistent => "==",
            };
            println!("  {}{} [{}] {}\x1b[0m", color, icon, f.severity, f.title);
            for rule_name in &f.rules_involved {
                if let Some(r) = results.iter().find(|r| &r.name == rule_name) {
                    if r.param_count == 0 {
                        let val_color = if r.is_bool {
                            if r.is_true {
                                "\x1b[1;32m"
                            } else {
                                "\x1b[1;31m"
                            }
                        } else {
                            "\x1b[1;33m"
                        };
                        println!("     | {}() -> {}{}\x1b[0m", r.name, val_color, r.value_str);
                    }
                }
            }
            println!("     {}", f.explanation);
            if i < group.len() - 1 {
                println!();
            }
        }
        println!();
    }

    let interesting = paradox_count + asymmetry_count + gap_count + tension_count;
    if interesting > 0 {
        println!(
            "\x1b[1m{} findings\x1b[0m from {} rules across {} type domains.",
            interesting,
            results.iter().filter(|r| r.param_count == 0).count(),
            type_domains.len()
        );
        println!(
            "Analysis: type-graph (value domains + resolution chains), not string heuristics."
        );
    } else {
        println!("No gaps or tensions discovered.");
    }
    println!();
}

/// Collect rule name references from an expression (for invariant coverage analysis)
fn collect_rule_refs(expr: &Expr, refs: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            refs.insert(name.clone());
        }
        ExprKind::App(f, args) => {
            collect_rule_refs(f, refs);
            for a in args {
                collect_rule_refs(a, refs);
            }
        }
        ExprKind::BinOp(_, l, r) => {
            collect_rule_refs(l, refs);
            collect_rule_refs(r, refs);
        }
        ExprKind::UnOp(_, e) => {
            collect_rule_refs(e, refs);
        }
        ExprKind::If(c, t, e) => {
            collect_rule_refs(c, refs);
            collect_rule_refs(t, refs);
            collect_rule_refs(e, refs);
        }
        ExprKind::Field(e, _) => {
            collect_rule_refs(e, refs);
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                if let Stmt::Expr(e) = s {
                    collect_rule_refs(e, refs);
                }
            }
        }
        _ => {}
    }
}

/// Generate SMT-LIB2 for all invariants and verify with Z3
fn verify_with_z3(source: &str, filename: &str) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    let stmts = match parser.parse_program() {
        Ok(s) => s,
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    };

    // Also resolve @ use imports for cross-file verification
    let source_dir = std::path::Path::new(filename)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    let mut all_stmts: Vec<Stmt> = Vec::new();
    let mut imported: BTreeSet<String> = BTreeSet::new();
    for stmt in &stmts {
        if let Stmt::Use(path) = stmt {
            let module = path.trim_end_matches("::*").replace("::", "/");
            if let Some(ref dir) = source_dir {
                let file_path = format!("{}/{}.runa", dir, module);
                let canon = std::fs::canonicalize(&file_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(file_path.clone());
                if !imported.contains(&canon) {
                    imported.insert(canon);
                    if let Ok(src) = std::fs::read_to_string(&file_path) {
                        let mut lx = Lexer::new(&src);
                        let toks = lx.tokenize();
                        let mut px = Parser::new(toks, &src);
                        if let Ok(import_stmts) = px.parse_program() {
                            // Only pull in types, functions, and bindings
                            for s in import_stmts {
                                if matches!(s, Stmt::Defn(_) | Stmt::TypeDecl(_) | Stmt::Bind(..)) {
                                    all_stmts.push(s);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Append the main file's statements
    all_stmts.extend(stmts.iter().cloned());

    // Collect ADTs, invariants, bindings, and function definitions
    let mut adts: Vec<(String, Vec<Variant>)> = Vec::new();
    let mut ctor_to_type: BTreeMap<String, String> = BTreeMap::new();
    let mut invariants: Vec<(String, Expr, Expr)> = Vec::new();
    let mut bindings: BTreeMap<String, Expr> = BTreeMap::new();
    let mut binding_types: BTreeMap<String, Option<Ty>> = BTreeMap::new();
    let mut functions: Vec<(String, Vec<Param>, Option<Ty>, Expr)> = Vec::new();

    for stmt in &all_stmts {
        match stmt {
            Stmt::TypeDecl(TypeDecl::ADT { name, variants, .. }) => {
                adts.push((name.clone(), variants.clone()));
                for v in variants {
                    ctor_to_type.insert(v.name.clone(), name.clone());
                }
            }
            Stmt::Invariant {
                name,
                subject,
                predicate,
            } => {
                invariants.push((name.clone(), subject.clone(), predicate.clone()));
            }
            Stmt::Bind(Pat::Var(name), ty, expr) => {
                bindings.insert(name.clone(), expr.clone());
                binding_types.insert(name.clone(), ty.clone());
            }
            Stmt::Defn(Defn::Fn {
                name,
                params,
                ret_ty,
                body,
                ..
            }) => {
                functions.push((name.clone(), params.clone(), ret_ty.clone(), body.clone()));
            }
            _ => {}
        }
    }

    if invariants.is_empty() {
        println!("runa --verify: no invariants found in {}", filename);
        return;
    }

    println!(
        "runa --verify: {} invariant(s), {} ADT(s) from {}",
        invariants.len(),
        adts.len(),
        filename
    );
    println!();

    // For each invariant, generate SMT-LIB2 and run Z3
    for (inv_name, subject_expr, pred_expr) in &invariants {
        println!("--- | {} ---", inv_name);

        let mut smt = String::new();
        smt.push_str("; Auto-generated by runa --verify\n");
        smt.push_str(&format!("; Invariant: | {}\n", inv_name));
        smt.push_str("(set-logic ALL)\n\n");

        // Emit ADT datatype declarations
        for (adt_name, variants) in &adts {
            smt.push_str(&emit_z3_datatype(adt_name, variants));
            smt.push_str("\n");
        }
        if !adts.is_empty() {
            smt.push('\n');
        }

        // Declare free variables from the predicate
        let mut free_vars = BTreeSet::new();
        collect_free_vars(pred_expr, &mut free_vars);
        collect_free_vars(subject_expr, &mut free_vars);

        // Remove function names and constructor names from free vars
        let fn_names: BTreeSet<String> = functions.iter().map(|(n, _, _, _)| n.clone()).collect();
        let ctor_names: BTreeSet<String> = ctor_to_type.keys().cloned().collect();
        free_vars = free_vars.difference(&fn_names).cloned().collect();
        free_vars = free_vars.difference(&ctor_names).cloned().collect();

        // Transitively resolve: if a binding references other bindings, include those too
        let mut resolved = BTreeSet::new();
        let mut worklist: Vec<String> = free_vars.iter().cloned().collect();
        while let Some(var) = worklist.pop() {
            if resolved.contains(&var) {
                continue;
            }
            resolved.insert(var.clone());
            if let Some(bound_expr) = bindings.get(&var) {
                let mut sub_vars = BTreeSet::new();
                collect_free_vars(bound_expr, &mut sub_vars);
                let sub_vars: BTreeSet<String> = sub_vars
                    .difference(&fn_names)
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .difference(&ctor_names)
                    .cloned()
                    .collect();
                for sv in sub_vars {
                    if !resolved.contains(&sv) {
                        worklist.push(sv);
                    }
                }
            }
        }
        free_vars = resolved;

        // Emit known bindings as define-const, declare rest as free variables
        // Emit in dependency order: bindings that reference other bindings come later
        let ordered_vars = {
            let mut ordered = Vec::new();
            let mut emitted = BTreeSet::new();
            let mut remaining: Vec<String> = free_vars.iter().cloned().collect();
            let max_iters = remaining.len() * remaining.len() + 1;
            let mut iter_count = 0;
            while !remaining.is_empty() && iter_count < max_iters {
                iter_count += 1;
                let mut next_remaining = Vec::new();
                for var in &remaining {
                    if let Some(bound_expr) = bindings.get(var) {
                        let mut deps = BTreeSet::new();
                        collect_free_vars(bound_expr, &mut deps);
                        let deps: BTreeSet<String> = deps
                            .difference(&fn_names)
                            .cloned()
                            .collect::<BTreeSet<_>>()
                            .difference(&ctor_names)
                            .cloned()
                            .collect();
                        if deps
                            .iter()
                            .all(|d| emitted.contains(d) || !bindings.contains_key(d))
                        {
                            ordered.push(var.clone());
                            emitted.insert(var.clone());
                        } else {
                            next_remaining.push(var.clone());
                        }
                    } else {
                        ordered.push(var.clone());
                        emitted.insert(var.clone());
                    }
                }
                remaining = next_remaining;
            }
            for var in remaining {
                ordered.push(var);
            }
            ordered
        };

        // Emit functions with type-aware param/return sorts
        for (fname, params, ret_ty, body) in &functions {
            let used = smt_expr_uses_fn(pred_expr, fname)
                || smt_expr_uses_fn(subject_expr, fname)
                || ordered_vars.iter().any(|v| {
                    bindings
                        .get(v)
                        .map_or(false, |e| smt_expr_uses_fn(e, fname))
                });
            if used {
                let param_decls: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let sort = match &p.ty {
                            Some(ty) => ty_to_smt_sort(ty),
                            None => "Int".into(),
                        };
                        format!("({} {})", p.name, sort)
                    })
                    .collect();
                let ret_sort = match ret_ty {
                    Some(ty) => ty_to_smt_sort(ty),
                    None => "Bool".into(), // default for predicates
                };
                let body_smt = expr_to_smt(body);
                smt.push_str(&format!(
                    "(define-fun {} ({}) {} {})\n",
                    fname,
                    param_decls.join(" "),
                    ret_sort,
                    body_smt
                ));
            }
        }

        // Emit constants in dependency order with ADT-aware sorts
        for var in &ordered_vars {
            if let Some(bound_expr) = bindings.get(var) {
                // Use explicit type annotation if available, else infer
                let sort = if let Some(Some(ty)) = binding_types.get(var) {
                    ty_to_smt_sort(ty)
                } else {
                    infer_smt_sort_adts(bound_expr, &ctor_to_type)
                };
                let val = expr_to_smt(bound_expr);
                smt.push_str(&format!("(define-const {} {} {})\n", var, sort, val));
            } else {
                // Free variable — declare with inferred sort
                smt.push_str(&format!("(declare-const {} Int)\n", var));
            }
        }

        // The proof strategy: try to find a counterexample.
        // Assert NOT(predicate) — if UNSAT, the invariant holds for all values.
        let pred_smt = expr_to_smt(pred_expr);
        smt.push_str(&format!(
            "\n; Try to find counterexample to: {}\n",
            inv_name
        ));
        smt.push_str(&format!("(assert (not {}))\n", pred_smt));
        smt.push_str("(check-sat)\n");
        smt.push_str("(get-model)\n"); // only meaningful if sat

        println!("  SMT-LIB2:");
        for line in smt.lines() {
            println!("    {}", line);
        }
        println!();

        // Try to invoke Z3
        match std::process::Command::new("z3")
            .arg("-in")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                if let Some(ref mut stdin) = child.stdin {
                    use std::io::Write;
                    let _ = stdin.write_all(smt.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let first_line = stdout.lines().next().unwrap_or("");
                        match first_line.trim() {
                            "unsat" => {
                                println!("  ✓ PROVED: |{}| holds for all values", inv_name);
                            }
                            "sat" => {
                                println!("  ✗ COUNTEREXAMPLE found for |{}|:", inv_name);
                                for line in stdout.lines().skip(1) {
                                    println!("    {}", line);
                                }
                            }
                            other => {
                                println!("  ? Z3 returned: {}", other);
                                for line in stdout.lines().skip(1) {
                                    println!("    {}", line);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("  (Z3 execution error: {})", e);
                        println!("  SMT-LIB2 output above can be piped to z3 manually:");
                        println!("    echo '<smt>' | z3 -in");
                    }
                }
            }
            Err(_) => {
                println!("  (Z3 not found — install with: apt install z3 / brew install z3)");
                println!("  SMT-LIB2 output above can be piped to z3 manually.");
            }
        }
        println!();
    }
}

/// Check if an expression references a function name (for SMT emission)
fn smt_expr_uses_fn(expr: &Expr, fname: &str) -> bool {
    match &expr.kind {
        ExprKind::Var(name) => name == fname,
        ExprKind::App(func, args) => {
            smt_expr_uses_fn(func, fname) || args.iter().any(|a| smt_expr_uses_fn(a, fname))
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            smt_expr_uses_fn(lhs, fname) || smt_expr_uses_fn(rhs, fname)
        }
        ExprKind::UnOp(_, inner) => smt_expr_uses_fn(inner, fname),
        ExprKind::If(c, t, e) => {
            smt_expr_uses_fn(c, fname) || smt_expr_uses_fn(t, fname) || smt_expr_uses_fn(e, fname)
        }
        ExprKind::Field(obj, _) => smt_expr_uses_fn(obj, fname),
        ExprKind::Match(scrut, arms) => {
            smt_expr_uses_fn(scrut, fname)
                || arms.iter().any(|a| {
                    smt_expr_uses_fn(&a.body, fname)
                        || a.guard
                            .as_ref()
                            .map_or(false, |g| smt_expr_uses_fn(g, fname))
                })
        }
        ExprKind::Block(stmts) => stmts.iter().any(|s| match s {
            Stmt::Bind(_, _, e) | Stmt::Expr(e) => smt_expr_uses_fn(e, fname),
            _ => false,
        }),
        _ => false,
    }
}

fn show_hashes(source: &str, filename: &str) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(stmts) => {
            println!("runa --hashes: {}", filename);
            print_hashes(&stmts);
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Collect name→hash mappings from a program's definitions.
fn collect_registry(stmts: &[Stmt]) -> BTreeMap<String, String> {
    let mut registry = BTreeMap::new();
    for stmt in stmts {
        match stmt {
            Stmt::Defn(defn) => {
                let hash = content_hash_defn(defn);
                registry.insert(defn_name(defn).to_string(), hash);
            }
            Stmt::TypeDecl(td) => {
                let hash = content_hash_type(td);
                registry.insert(type_decl_name(td).to_string(), hash);
            }
            _ => {}
        }
    }
    registry
}

/// Save/update the name registry (.runa-registry.json) for a file.
fn update_registry(source: &str, filename: &str) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(stmts) => {
            let registry = collect_registry(&stmts);
            let registry_path = format!(
                "{}.registry.json",
                std::path::Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            );
            // Read existing registry if present, merge
            let mut existing: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
            if let Ok(data) = std::fs::read_to_string(&registry_path) {
                // Simple JSON parse (BTreeMap<module, BTreeMap<name, hash>>)
                if let Some(parsed) = parse_simple_json_registry(&data) {
                    existing = parsed;
                }
            }
            existing.insert(filename.to_string(), registry.clone());
            // Write as simple JSON
            let mut json = String::from("{\n");
            for (idx, (module, entries)) in existing.iter().enumerate() {
                json.push_str(&format!("  \"{}\": {{\n", module));
                for (eidx, (name, hash)) in entries.iter().enumerate() {
                    json.push_str(&format!("    \"{}\": \"#{}\"", name, hash));
                    if eidx < entries.len() - 1 {
                        json.push(',');
                    }
                    json.push('\n');
                }
                json.push_str("  }");
                if idx < existing.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("}\n");
            std::fs::write(&registry_path, &json).unwrap_or_else(|e| {
                eprintln!("Error writing registry: {}", e);
            });
            println!(
                "runa --registry: {} ({} definitions)",
                registry_path,
                registry.len()
            );
            for (name, hash) in &registry {
                println!("  {} → #{}", name, hash);
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Simple JSON parser for registry files (just nested string maps).
fn parse_simple_json_registry(data: &str) -> Option<BTreeMap<String, BTreeMap<String, String>>> {
    // Minimal JSON parse: { "key": { "name": "hash", ... }, ... }
    let trimmed = data.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let mut result = BTreeMap::new();
    let inner = &trimmed[1..trimmed.len() - 1];
    // Split by top-level entries (simplistic, works for our controlled output)
    let mut depth = 0;
    let mut current_key = String::new();
    let mut current_val_start = 0;
    let mut in_string = false;
    let mut escape = false;
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if c == '\\' {
            escape = true;
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if in_string {
            i += 1;
            continue;
        }
        if c == '{' {
            depth += 1;
            if depth == 1 {
                current_val_start = i;
            }
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                // Parse inner object
                let inner_str = &inner[current_val_start + 1..i];
                let mut entries = BTreeMap::new();
                // Parse "key": "value" pairs
                for part in inner_str.split(',') {
                    let part = part.trim();
                    if let Some(colon) = part.find(':') {
                        let k = part[..colon].trim().trim_matches('"').to_string();
                        let v = part[colon + 1..].trim().trim_matches('"').to_string();
                        if !k.is_empty() {
                            entries.insert(k, v);
                        }
                    }
                }
                result.insert(current_key.clone(), entries);
            }
        } else if c == ':' && depth == 0 {
            // Extract key (find the quoted string before this colon)
            let before: String = inner[..i].chars().collect();
            if let Some(start) = before.rfind('"') {
                if let Some(end) = before[..start].rfind('"') {
                    current_key = before[end + 1..start].to_string();
                }
            }
        }
        i += 1;
    }
    Some(result)
}

/// Parse + codegen + type-check without running. Fast feedback loop.
fn check_source(source: &str, filename: &str, use_prelude: bool) {
    use std::process::Command;
    use std::time::Instant;

    let start = Instant::now();
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };
            let stmt_count = stmts.len();
            let fn_count = stmts
                .iter()
                .filter(|s| matches!(s, Stmt::Defn(Defn::Fn { .. })))
                .count();
            let type_count = stmts
                .iter()
                .filter(|s| matches!(s, Stmt::TypeDecl(_)))
                .count();

            // Pre-codegen type checking (M16): catch errors before Rust codegen
            if run_type_check(&stmts, source, filename) {
                std::process::exit(1);
            }

            let mut cg = RustCodegen::new();
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let code = cg.emit_program(&stmts);

            if cg.cargo_deps.is_empty() {
                // No deps — type-check with rustc directly
                let cache_dir = std::env::temp_dir().join("runa-cache");
                std::fs::create_dir_all(&cache_dir).ok();
                let rs_path = cache_dir.join("__check.rs");
                std::fs::write(&rs_path, &code).ok();
                let rustc_bin = find_rust_tool("rustc");
                let meta_out = cache_dir.join("__check_out");
                let output = Command::new(&rustc_bin)
                    .args(&[
                        &*rs_path.to_string_lossy(),
                        "--edition",
                        "2021",
                        "--crate-type",
                        "bin",
                        "--emit=metadata",
                        "-o",
                        &*meta_out.to_string_lossy(),
                    ])
                    .output();
                let elapsed = start.elapsed();
                match output {
                    Ok(o) if o.status.success() => {
                        eprintln!("\x1b[1;32mcheck ok\x1b[0m: {} ({} stmts, {} fns, {} types, {} lines of Rust) \x1b[2m[{:.1}s]\x1b[0m",
                            filename, stmt_count, fn_count, type_count, code.lines().count(), elapsed.as_secs_f64());
                    }
                    Ok(o) => {
                        eprintln!("\x1b[1;31mcheck failed\x1b[0m: {}", filename);
                        eprintln!("{}", String::from_utf8_lossy(&o.stderr));
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error running rustc: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Has deps — use cargo check in .runa-build/
                let stem = std::path::Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("tau_out");
                let build_dir = format!(".runa-build/{}", stem);
                let src_dir = format!("{}/src", build_dir);
                std::fs::create_dir_all(&src_dir).ok();
                let main_rs = format!("{}/main.rs", src_dir);
                std::fs::write(&main_rs, &code).ok();
                // Generate Cargo.toml
                let safe_stem: String = stem
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let mut cargo_toml = format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
                    safe_stem
                );
                for (crate_name, version) in &cg.cargo_deps {
                    let safe_name: String = crate_name
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    if version.starts_with('{') {
                        cargo_toml.push_str(&format!("{} = {}\n", safe_name, version));
                    } else {
                        let safe_version: String = version
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '+')
                            .collect();
                        cargo_toml.push_str(&format!("{} = \"{}\"\n", safe_name, safe_version));
                    }
                }
                std::fs::write(format!("{}/Cargo.toml", build_dir), &cargo_toml).ok();
                let cargo_bin = find_rust_tool("cargo");
                let output = Command::new(&cargo_bin)
                    .args(&["check"])
                    .current_dir(&build_dir)
                    .output();
                let elapsed = start.elapsed();
                match output {
                    Ok(o) if o.status.success() => {
                        eprintln!("\x1b[1;32mcheck ok\x1b[0m: {} ({} stmts, {} fns, {} types, {} deps, {} lines of Rust) \x1b[2m[{:.1}s]\x1b[0m",
                            filename, stmt_count, fn_count, type_count, cg.cargo_deps.len(), code.lines().count(), elapsed.as_secs_f64());
                    }
                    Ok(o) => {
                        eprintln!("\x1b[1;31mcheck failed\x1b[0m: {}", filename);
                        eprintln!("{}", String::from_utf8_lossy(&o.stderr));
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error running cargo: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

/// Emit Rust via the FIR pipeline (M29).
/// Currently handles core expressions; falls back to old path for complex features.
fn emit_via_fir(
    stmts: &[Stmt],
    types: &TypeRegistry,
    borrow_params: &BTreeMap<String, Vec<bool>>,
    copy_vars: &BTreeSet<String>,
    ref_match: &BTreeSet<String>,
) -> String {
    let mut out = String::new();
    for stmt in stmts {
        match stmt {
            Stmt::Defn(Defn::Fn {
                name,
                params,
                ret_ty,
                body,
                ..
            }) => {
                // Build type environment from function parameters
                let mut type_env = BTreeMap::new();
                for p in params {
                    if let Some(ty) = &p.ty {
                        type_env.insert(p.name.clone(), LoweringCtx::ty_to_fir(ty));
                    }
                }
                // Compute per-function ownership
                let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
                let ownership = OwnershipAnalysis::analyze(
                    body,
                    borrow_params,
                    Some(name.as_str()),
                    &param_names,
                );
                let mut ctx = LoweringCtx {
                    type_env,
                    inference: None,
                    fn_schemes: BTreeMap::new(),
                    types,
                    ownership: &ownership,
                    copy_vars,
                    ref_match_bindings: ref_match,
                };
                let fir_body = ctx.lower_expr(body);

                // Emit function signature
                let param_strs: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let ty_str = match &p.ty {
                            Some(ty) => format!(": {}", emit_fir_ty_as_rust(ty)),
                            None => String::new(),
                        };
                        format!("{}{}", sanitize_name(&p.name), ty_str)
                    })
                    .collect();
                let ret_str = match ret_ty {
                    Some(ty) => format!(" -> {}", emit_fir_ty_as_rust(ty)),
                    None => String::new(),
                };
                out.push_str(&format!(
                    "fn {}({}){} {{\n",
                    sanitize_name(name),
                    param_strs.join(", "),
                    ret_str
                ));
                out.push_str(&format!("    {}\n", emit_fir_expr(&fir_body, types)));
                out.push_str("}\n\n");
            }
            Stmt::Bind(Pat::Var(name), _, expr) => {
                let ownership = OwnershipAnalysis::analyze_simple(expr);
                let mut ctx = LoweringCtx {
                    type_env: BTreeMap::new(),
                    inference: None,
                    fn_schemes: BTreeMap::new(),
                    types,
                    ownership: &ownership,
                    copy_vars,
                    ref_match_bindings: ref_match,
                };
                let fir_expr = ctx.lower_expr(expr);
                out.push_str(&format!(
                    "let {} = {};\n",
                    sanitize_name(name),
                    emit_fir_expr(&fir_expr, types)
                ));
            }
            Stmt::Expr(expr) => {
                let ownership = OwnershipAnalysis::analyze_simple(expr);
                let mut ctx = LoweringCtx {
                    type_env: BTreeMap::new(),
                    inference: None,
                    fn_schemes: BTreeMap::new(),
                    types,
                    ownership: &ownership,
                    copy_vars,
                    ref_match_bindings: ref_match,
                };
                let fir_expr = ctx.lower_expr(expr);
                out.push_str(&format!("{};\n", emit_fir_expr(&fir_expr, types)));
            }
            Stmt::For(var, iter_expr, body) => {
                let ownership = OwnershipAnalysis::analyze_simple(iter_expr);
                let mut ctx = LoweringCtx {
                    type_env: BTreeMap::new(),
                    inference: None,
                    fn_schemes: BTreeMap::new(),
                    types,
                    ownership: &ownership,
                    copy_vars,
                    ref_match_bindings: ref_match,
                };
                let fir_iter = ctx.lower_expr(iter_expr);
                let fir_body: Vec<FirStmt> = body.iter().map(|s| ctx.lower_stmt(s)).collect();
                let body_strs: Vec<String> =
                    fir_body.iter().map(|s| emit_fir_stmt(s, types)).collect();
                out.push_str(&format!(
                    "for {} in {} {{ {} }}\n",
                    sanitize_name(var),
                    emit_fir_expr(&fir_iter, types),
                    body_strs.join(" ")
                ));
            }
            Stmt::TypeDecl(_)
            | Stmt::Rule(_)
            | Stmt::Import(_)
            | Stmt::QualifiedImport(..)
            | Stmt::HashImport(..)
            | Stmt::Depend(..)
            | Stmt::RustBlock(_)
            | Stmt::Annot(..)
            | Stmt::Use(_) => {
                // These are declarations/metadata — handled by TypeRegistry scan, not emitted here
            }
            _ => {
                out.push_str("// [FIR: unhandled stmt]\n");
            }
        }
    }
    out
}

/// Convert a Futuruna Ty to a Rust type string (for FIR emission).
fn emit_fir_ty_as_rust(ty: &Ty) -> String {
    match ty {
        Ty::Name(n) => match n.as_str() {
            "Int" => "i64".to_string(),
            "Float" => "f64".to_string(),
            "Bool" => "bool".to_string(),
            "Char" => "char".to_string(),
            "String" => "String".to_string(),
            "Unit" | "()" => "()".to_string(),
            other => other.to_string(),
        },
        Ty::App(base, args) => {
            let base_str = emit_fir_ty_as_rust(base);
            let arg_strs: Vec<String> = args.iter().map(|a| emit_fir_ty_as_rust(a)).collect();
            match base_str.as_str() {
                "List" => format!("Vec<{}>", arg_strs.join(", ")),
                "Option" => format!("Option<{}>", arg_strs.join(", ")),
                "Result" => format!("Result<{}>", arg_strs.join(", ")),
                "Map" => format!("HashMap<{}>", arg_strs.join(", ")),
                "Set" => format!("HashSet<{}>", arg_strs.join(", ")),
                _ => format!("{}<{}>", base_str, arg_strs.join(", ")),
            }
        }
        Ty::Arrow(a, b) => format!(
            "impl Fn({}) -> {}",
            emit_fir_ty_as_rust(a),
            emit_fir_ty_as_rust(b)
        ),
        Ty::Unit => "()".to_string(),
        Ty::Optional(inner) => format!("Option<{}>", emit_fir_ty_as_rust(inner)),
        _ => "/* unknown type */".to_string(),
    }
}

/// Emit Rust source via the FIR pipeline (runa emit --fir).
/// Runs old codegen to get the full Rust output, then also runs FIR on each
/// function body and prints the FIR-emitted version for comparison.
fn emit_rust_source_fir(source: &str, filename: &str, use_prelude: bool) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };

            // Run type check
            if run_type_check(&stmts, source, filename) {
                std::process::exit(1);
            }

            // Scan declarations to populate TypeRegistry (without emitting Rust)
            let mut cg = RustCodegen::new();
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let resolved_stmts = cg.scan_declarations(&stmts);

            // Emit FIR version
            let code = emit_via_fir(
                &resolved_stmts,
                &cg.types,
                &cg.borrow_only_params,
                &cg.copy_vars,
                &cg.ref_match_bindings,
            );

            // Also run old path for comparison
            let mut cg2 = RustCodegen::new();
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg2.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg2.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let old_code = cg2.emit_program(&stmts);
            println!("// === FIR pipeline output ===");
            println!("{}", code);
            println!(
                "// === Old pipeline output ({} lines) ===",
                old_code.lines().count()
            );
            println!("{}", old_code);
            eprintln!(
                "// runa emit --fir: {} — FIR and old output shown side by side",
                filename
            );
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

fn emit_rust_source(source: &str, filename: &str, use_prelude: bool) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };
            let mut cg = RustCodegen::new();
            // Set source directory for @ import resolution
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let code = cg.emit_program(&stmts);
            println!("{}", code);
            eprintln!(
                "// runa --emit rust: {} → {} lines of Rust",
                filename,
                code.lines().count()
            );
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

fn emit_rust_lib(source: &str, filename: &str, use_prelude: bool) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    match parser.parse_program() {
        Ok(user_stmts) => {
            let stmts = if use_prelude {
                prepend_prelude(parse_prelude(), &user_stmts)
            } else {
                user_stmts
            };
            let mut cg = RustCodegen::new();
            cg.lib_mode = true;
            if let Some(parent) = std::path::Path::new(filename).parent() {
                cg.source_dir = Some(parent.to_string_lossy().to_string());
            }
            cg.source_name = std::path::Path::new(filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string());
            let code = cg.emit_program(&stmts);
            println!("{}", code);
            eprintln!(
                "// runa --lib: {} → {} lines of Rust library",
                filename,
                code.lines().count()
            );
        }
        Err(e) => {
            display_error_in(source, &e, filename);
            std::process::exit(1);
        }
    }
}

// ============================================================================
// PART 8b: SOURCE CODE FORMATTER (M18)
// ============================================================================
//
// Line-based formatter that preserves comments and normalizes indentation.
// Works without the AST — tracks brace depth to determine nesting.
//
// Rules:
//   - 4 spaces per indentation level
//   - Trailing whitespace removed
//   - Max 1 consecutive blank line
//   - Single newline at end of file
//   - `@ rust { }` blocks: content reindented preserving relative structure
//   - `----` and `{-` block comments: preserved, indentation normalized
//   - Triple-quoted strings: preserved verbatim

fn format_target(path: &str, check: bool) {
    let meta = std::fs::metadata(path);
    if let Ok(m) = &meta {
        if m.is_dir() {
            format_directory(path, check);
            return;
        }
    }
    match std::fs::read_to_string(path) {
        Ok(source) => format_one_file(&source, path, check),
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        }
    }
}

fn format_directory(dir: &str, check: bool) {
    let mut files = Vec::new();
    collect_runa_files(dir, &mut files);
    files.sort();

    if files.is_empty() {
        eprintln!("No .runa files found in {}", dir);
        return;
    }

    let mut changed = 0usize;
    let mut total = 0usize;
    for path in &files {
        total += 1;
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  \x1b[1;33mskip\x1b[0m {}: {}", path, e);
                continue;
            }
        };
        let formatted = format_runa_source(&source);
        if formatted != source {
            changed += 1;
            if check {
                eprintln!("  \x1b[1;31mneeds fmt\x1b[0m  {}", path);
            } else {
                match std::fs::write(path, &formatted) {
                    Ok(_) => eprintln!("  \x1b[1;32mformatted\x1b[0m  {}", path),
                    Err(e) => eprintln!("  \x1b[1;31merror\x1b[0m  {}: {}", path, e),
                }
            }
        }
    }

    if check {
        if changed > 0 {
            eprintln!(
                "\n{} of {} file{} need formatting.",
                changed,
                total,
                if total == 1 { "" } else { "s" }
            );
            std::process::exit(1);
        } else {
            eprintln!(
                "All {} file{} correctly formatted.",
                total,
                if total == 1 { " is" } else { "s are" }
            );
        }
    } else {
        if changed > 0 {
            eprintln!(
                "\nFormatted {} of {} file{}.",
                changed,
                total,
                if total == 1 { "" } else { "s" }
            );
        } else {
            eprintln!(
                "All {} file{} already formatted.",
                total,
                if total == 1 { "" } else { "s" }
            );
        }
    }
}

fn collect_runa_files(dir: &str, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // Skip hidden dirs, build artifacts, and caches
        if name.starts_with('.') || name == "target" || name == "runa-cache" {
            continue;
        }
        if path.is_dir() {
            collect_runa_files(&path.to_string_lossy(), out);
        } else if path.extension().map_or(false, |e| e == "runa") {
            out.push(path.to_string_lossy().to_string());
        }
    }
}

fn format_one_file(source: &str, filename: &str, check: bool) {
    let formatted = format_runa_source(source);
    if check {
        if formatted != source {
            eprintln!("{} needs formatting", filename);
            std::process::exit(1);
        } else {
            eprintln!("{} is correctly formatted", filename);
        }
    } else {
        if formatted != source {
            match std::fs::write(filename, &formatted) {
                Ok(_) => eprintln!("formatted {}", filename),
                Err(e) => eprintln!("error writing {}: {}", filename, e),
            }
        } else {
            eprintln!("{} unchanged", filename);
        }
    }
}

fn format_runa_source(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut depth: i32 = 0;
    let mut prev_blank = false;
    let mut in_block_comment_dash = false;
    let mut in_block_comment_brace = false;
    let mut in_rust_block = false;
    let mut rust_brace_depth: i32 = 0;
    let mut in_triple_string = false;
    let mut rust_block_lines: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // ── Triple-quoted strings: preserve verbatim ──
        if in_triple_string {
            // Keep the line exactly as-is (content of triple-quoted string)
            result.push(lines[i].trim_end().to_string());
            if trimmed.contains("\"\"\"") {
                in_triple_string = false;
            }
            i += 1;
            continue;
        }

        // ── @ rust { } blocks: collect and reindent preserving relative structure ──
        if in_rust_block {
            let net = fmt_count_raw_braces(trimmed);
            rust_brace_depth += net;
            if rust_brace_depth <= 0 {
                // Closing } of @ rust block — first reindent collected lines
                let reindented = fmt_reindent_block(&rust_block_lines, depth + 1);
                result.extend(reindented);
                rust_block_lines.clear();
                result.push(format!("{}}}", fmt_indent(depth)));
                in_rust_block = false;
            } else {
                rust_block_lines.push(lines[i].to_string());
            }
            prev_blank = false;
            i += 1;
            continue;
        }

        // ── ---- block comments ──
        if in_block_comment_dash {
            if trimmed.is_empty() {
                result.push(String::new());
            } else {
                result.push(format!("{}{}", fmt_indent(depth), trimmed));
            }
            if trimmed.contains("----") {
                in_block_comment_dash = false;
            }
            prev_blank = false;
            i += 1;
            continue;
        }

        // ── {- -} block comments ──
        if in_block_comment_brace {
            if trimmed.is_empty() {
                result.push(String::new());
            } else {
                result.push(format!("{}{}", fmt_indent(depth), trimmed));
            }
            if trimmed.contains("-}") {
                in_block_comment_brace = false;
            }
            prev_blank = false;
            i += 1;
            continue;
        }

        // ── Blank lines: max 1 consecutive ──
        if trimmed.is_empty() {
            if !prev_blank && !result.is_empty() {
                result.push(String::new());
            }
            prev_blank = true;
            i += 1;
            continue;
        }
        prev_blank = false;

        // ── Check for ---- block comment opening ──
        if trimmed.starts_with("----") {
            result.push(format!("{}{}", fmt_indent(depth), trimmed));
            // Count ---- occurrences: odd means unclosed
            let dash_count = trimmed.matches("----").count();
            if dash_count % 2 == 1 {
                in_block_comment_dash = true;
            }
            i += 1;
            continue;
        }

        // ── Check for {- block comment opening ──
        if trimmed.starts_with("{-") {
            result.push(format!("{}{}", fmt_indent(depth), trimmed));
            if !trimmed.contains("-}") {
                in_block_comment_brace = true;
            }
            i += 1;
            continue;
        }

        // ── Check for @ rust { block ──
        if trimmed.starts_with("@ rust") && trimmed.contains('{') {
            result.push(format!("{}{}", fmt_indent(depth), trimmed));
            rust_brace_depth = fmt_count_raw_braces(trimmed);
            if rust_brace_depth > 0 {
                in_rust_block = true;
            }
            i += 1;
            continue;
        }

        // ── Check for triple-quoted string opening ──
        if !trimmed.starts_with("--") {
            let tq_count = count_triple_quotes(trimmed);
            if tq_count % 2 == 1 {
                // Odd number of """ means one is unclosed → enter triple-string mode
                // Output this line normally first
                if trimmed.starts_with('}') {
                    depth = (depth - 1).max(0);
                }
                result.push(format!("{}{}", fmt_indent(depth), trimmed));
                if let Some('{') = fmt_effective_last_char(trimmed) {
                    depth += 1;
                }
                in_triple_string = true;
                i += 1;
                continue;
            }
        }

        // ── Normal line formatting ──

        // Apply rune spacing and operator normalization
        let trimmed = fmt_normalize_line(trimmed);
        let trimmed = trimmed.as_str();

        // Decrease depth if line starts with }
        if trimmed.starts_with('}') {
            depth = (depth - 1).max(0);
        }

        // Output with indentation
        result.push(format!("{}{}", fmt_indent(depth), trimmed));

        // Increase depth if line ends with { (outside strings/comments)
        if let Some('{') = fmt_effective_last_char(trimmed) {
            depth += 1;
        }

        i += 1;
    }

    // Remove trailing blank lines
    while result.last().map_or(false, |l| l.is_empty()) {
        result.pop();
    }

    let mut output = result.join("\n");
    output.push('\n');
    output
}

/// Normalize rune spacing and operator spacing on a single code line.
/// Handles: rune prefix spacing (# , > , | , = , ~ , @ , ?) and
/// binary operators (+, -, *, /, %, ==, !=, <=, >=, <, >, &&, ||, ->).
fn fmt_normalize_line(line: &str) -> String {
    // Don't touch comment-only lines
    if line.starts_with("--") {
        return line.to_string();
    }

    // Split into code part and trailing comment
    let (code, comment) = fmt_split_comment(line);
    if code.is_empty() {
        return line.to_string();
    }

    // Step 1: Rune spacing — ensure exactly one space after leading rune
    let normalized = fmt_rune_spacing(code.trim_end());

    // Step 2: Operator spacing — normalize binary operators outside strings
    let normalized = fmt_operator_spacing(&normalized);

    // Reassemble with comment
    if comment.is_empty() {
        normalized
    } else {
        format!("{} {}", normalized.trim_end(), comment.trim_start())
    }
}

/// Split a line into (code, comment) parts, respecting strings.
fn fmt_split_comment(line: &str) -> (String, String) {
    let chars: Vec<char> = line.chars().collect();
    let mut in_string = false;
    let mut escape = false;
    for i in 0..chars.len() {
        let c = chars[i];
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            continue;
        }
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            return (chars[..i].iter().collect(), chars[i..].iter().collect());
        }
    }
    (line.to_string(), String::new())
}

/// Ensure exactly one space after leading rune sigil.
/// `#Point(...)` → `# Point(...)`, `>foo(...)` → `> foo(...)`
/// But NOT: `>=`, `|>`, `||`, `= x =`, lines starting with `}`, etc.
fn fmt_rune_spacing(line: &str) -> String {
    if line.is_empty() {
        return line.to_string();
    }
    let first = line.chars().next().unwrap();
    match first {
        '#' | '>' | '~' | '?' => {
            if line.len() == 1 {
                return line.to_string();
            }
            let rest = &line[1..];
            // Already has a space
            if rest.starts_with(' ') {
                // Collapse multiple spaces to one
                let trimmed = rest.trim_start();
                return format!("{} {}", first, trimmed);
            }
            // Don't touch >= or compound operators
            if first == '>' && rest.starts_with('=') {
                return line.to_string();
            }
            // Rune followed by alphanumeric/( — insert space
            let next = rest.chars().next().unwrap();
            if next.is_alphanumeric() || next == '_' || next == '(' {
                return format!("{} {}", first, rest);
            }
            line.to_string()
        }
        '=' => {
            if line.len() == 1 {
                return line.to_string();
            }
            let rest = &line[1..];
            // Don't touch == operator
            if rest.starts_with('=') {
                return line.to_string();
            }
            if rest.starts_with(' ') {
                let trimmed = rest.trim_start();
                return format!("= {}", trimmed);
            }
            let next = rest.chars().next().unwrap();
            if next.is_alphanumeric() || next == '_' {
                return format!("= {}", rest);
            }
            line.to_string()
        }
        '@' => {
            if line.len() == 1 {
                return line.to_string();
            }
            let rest = &line[1..];
            if rest.starts_with(' ') {
                let trimmed = rest.trim_start();
                return format!("@ {}", trimmed);
            }
            let next = rest.chars().next().unwrap();
            if next.is_alphanumeric() || next == '_' {
                return format!("@ {}", rest);
            }
            line.to_string()
        }
        '|' => {
            if line.len() == 1 {
                return line.to_string();
            }
            let rest = &line[1..];
            // Don't touch |> pipe or || boolean-or
            if rest.starts_with('>') || rest.starts_with('|') {
                return line.to_string();
            }
            // Don't touch lambda syntax: |x| or |x, y|
            // Detect: | followed by identifier(s) then another |
            let rest_trimmed = rest.trim_start();
            if let Some(close_pipe) = rest_trimmed.find('|') {
                // Everything between the two pipes should be params (letters, commas, spaces)
                let between = &rest_trimmed[..close_pipe];
                if !between.is_empty()
                    && between.chars().all(|c| {
                        c.is_alphanumeric() || c == '_' || c == ',' || c == ' ' || c == ':'
                    })
                {
                    return line.to_string(); // Lambda — don't touch
                }
            }
            if rest.starts_with(' ') {
                let trimmed = rest.trim_start();
                return format!("| {}", trimmed);
            }
            let next = rest.chars().next().unwrap();
            if next.is_alphanumeric() || next == '_' {
                return format!("| {}", rest);
            }
            line.to_string()
        }
        _ => line.to_string(),
    }
}

/// Normalize binary operator spacing outside strings.
/// `a+b` → `a + b`, `x==y` → `x == y`, etc.
/// Careful with: negative literals, -> arrows, -- comments, unary minus, generic types.
fn fmt_operator_spacing(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut result = String::with_capacity(line.len() + 16);
    let mut in_string = false;
    let mut escape = false;
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        if escape {
            result.push(c);
            escape = false;
            i += 1;
            continue;
        }

        if in_string {
            result.push(c);
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            result.push(c);
            in_string = true;
            i += 1;
            continue;
        }

        // Two-char operators: ==, !=, <=, >=, ->, &&, ||, |>
        if i + 1 < len {
            let next = chars[i + 1];
            let pair = format!("{}{}", c, next);
            let after_op = chars.get(i + 2).copied();
            match pair.as_str() {
                "==" | "!=" | "<=" | ">=" | "&&" | "||" => {
                    fmt_ensure_space_around(&mut result, &pair, after_op);
                    i += 2;
                    continue;
                }
                "->" => {
                    fmt_ensure_space_around(&mut result, "->", after_op);
                    i += 2;
                    continue;
                }
                "|>" => {
                    fmt_ensure_space_around(&mut result, "|>", after_op);
                    i += 2;
                    continue;
                }
                "--" => {
                    // Rest is a comment, append verbatim
                    result.extend(&chars[i..]);
                    return result;
                }
                _ => {}
            }
        }

        // Single-char operators: + - * / %
        // Also single < and > when used as comparison (not type params)
        if "+-*/%<>".contains(c) {
            let is_minus = c == '-';
            // Skip unary minus
            if is_minus && fmt_is_unary_context(&result) {
                result.push(c);
                i += 1;
                continue;
            }
            // For < and >, only treat as operators if between operands
            // (avoids touching type params like List(Int))
            let prev_is_operand = result.ends_with(|ch: char| {
                ch.is_alphanumeric() || ch == '_' || ch == ')' || ch == ']' || ch == '"'
            });
            let next_is_operand = i + 1 < len
                && (chars[i + 1].is_alphanumeric()
                    || chars[i + 1] == '_'
                    || chars[i + 1] == '('
                    || chars[i + 1] == '-'
                    || chars[i + 1] == '"');

            if prev_is_operand && next_is_operand {
                let after_op = chars.get(i + 1).copied();
                fmt_ensure_space_around(&mut result, &c.to_string(), after_op);
                i += 1;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

/// Ensure spaces around an operator in the result string.
/// Skips adding space after if the next char in the input is already a space.
fn fmt_ensure_space_around(result: &mut String, op: &str, next_char: Option<char>) {
    // Ensure space before
    if !result.is_empty()
        && !result.ends_with(' ')
        && !result.ends_with('(')
        && !result.ends_with(',')
    {
        result.push(' ');
    }
    result.push_str(op);
    // Only add space after if next char isn't already a space
    if next_char != Some(' ') {
        result.push(' ');
    }
}

/// Check if the current position is a unary minus context.
/// Returns true if minus should be treated as unary (not binary).
fn fmt_is_unary_context(before: &str) -> bool {
    let trimmed = before.trim_end();
    if trimmed.is_empty() {
        return true;
    }
    let last = trimmed.chars().last().unwrap();
    // After operator, open paren, comma, = → unary
    matches!(
        last,
        '(' | '['
            | ','
            | '='
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '<'
            | '>'
            | '!'
            | '|'
            | '&'
            | '{'
            | ':'
    )
}

fn fmt_indent(depth: i32) -> String {
    "    ".repeat(depth.max(0) as usize)
}

/// Find the last significant character on a line, ignoring trailing comments and whitespace.
/// String-aware: braces inside strings are not considered.
fn fmt_effective_last_char(line: &str) -> Option<char> {
    let mut last_significant = None;
    let mut in_string = false;
    let mut escape = false;
    let chars: Vec<char> = line.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if escape {
            escape = false;
            continue;
        }

        if in_string {
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        // Line comment -- : rest is comment
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            break;
        }

        if c == '"' {
            in_string = true;
            continue;
        }

        if !c.is_whitespace() {
            last_significant = Some(c);
        }
    }

    last_significant
}

/// Count net braces in a line of raw Rust code (for @ rust blocks).
/// Aware of Rust strings, char literals, and // comments.
fn fmt_count_raw_braces(line: &str) -> i32 {
    let mut net: i32 = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;
    let chars: Vec<char> = line.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        if escape {
            escape = false;
            i += 1;
            continue;
        }

        if in_char {
            if c == '\\' {
                escape = true;
            } else if c == '\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }

        if in_string {
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Rust // comment
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            break;
        }
        // Runa -- comment (also skip in @ rust line detection)
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            break;
        }

        if c == '"' {
            in_string = true;
        } else if c == '\'' {
            in_char = true;
        } else if c == '{' {
            net += 1;
        } else if c == '}' {
            net -= 1;
        }

        i += 1;
    }

    net
}

/// Count occurrences of `"""` in a line (char-safe).
fn count_triple_quotes(line: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i + 2 < chars.len() {
        if chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
            count += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    count
}

/// Reindent a block of lines (e.g. @ rust content) preserving relative indentation.
/// Finds the minimum indent among non-blank lines, strips it, and applies target_depth.
fn fmt_reindent_block(lines: &[String], target_depth: i32) -> Vec<String> {
    let base = fmt_indent(target_depth);
    // Find minimum indentation among non-empty lines
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                let current_indent = line.len() - line.trim_start().len();
                let relative = current_indent.saturating_sub(min_indent);
                format!("{}{}{}", base, " ".repeat(relative), trimmed)
            }
        })
        .collect()
}

// ============================================================================
// PART 8c: LANGUAGE SERVER PROTOCOL (M19)
// ============================================================================
//
// Zero-dependency LSP server (uses only serde_json, already a dep).
// Communicates via JSON-RPC over stdin/stdout.
//
// Features:
//   - Diagnostics: parse errors + type checker errors (M16) as editor squiggles
//   - Go-to-definition: jump to function/type declarations
//   - Hover: show function signatures and type definitions
//   - Completion: rune-aware snippets + builtins + user-defined symbols

fn run_lsp_server() {
    use std::collections::HashMap;
    use std::io::{BufRead, Read, Write};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    // Cache prelude once at startup
    let prelude = parse_prelude();
    let mut documents: HashMap<String, String> = HashMap::new();

    loop {
        // Read JSON-RPC message
        let msg = match lsp_read_message(&mut reader) {
            Some(m) => m,
            None => break,
        };

        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        match method {
            "initialize" => {
                if let Some(id) = id.clone() {
                    lsp_send_response(&mut writer, id, lsp_server_capabilities());
                }
            }
            "initialized" => {} // client ready, nothing to do
            "shutdown" => {
                if let Some(id) = id.clone() {
                    lsp_send_response(&mut writer, id, serde_json::Value::Null);
                }
            }
            "exit" => break,

            "textDocument/didOpen" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let text = params["textDocument"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                documents.insert(uri.clone(), text.clone());
                lsp_analyze(&mut writer, &uri, &text, &prelude);
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if let Some(changes) = params["contentChanges"].as_array() {
                    if let Some(change) = changes.first() {
                        let text = change["text"].as_str().unwrap_or("").to_string();
                        documents.insert(uri.clone(), text.clone());
                        lsp_analyze(&mut writer, &uri, &text, &prelude);
                    }
                }
            }
            "textDocument/didClose" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                documents.remove(&uri);
                lsp_send_notification(
                    &mut writer,
                    "textDocument/publishDiagnostics",
                    serde_json::json!({"uri": uri, "diagnostics": []}),
                );
            }
            "textDocument/didSave" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if let Some(text) = documents.get(&uri).cloned() {
                    lsp_analyze(&mut writer, &uri, &text, &prelude);
                }
            }

            "textDocument/completion" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let text = documents.get(uri).cloned().unwrap_or_default();
                let items = lsp_completions(&text, line, col);
                if let Some(id) = id.clone() {
                    lsp_send_response(&mut writer, id, serde_json::json!(items));
                }
            }
            "textDocument/hover" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let text = documents.get(uri).cloned().unwrap_or_default();
                let hover = lsp_hover(&text, line, col);
                if let Some(id) = id.clone() {
                    lsp_send_response(&mut writer, id, hover);
                }
            }
            "textDocument/definition" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as u32;
                let text = documents.get(&uri).cloned().unwrap_or_default();
                let loc = lsp_definition(&uri, &text, line, col);
                if let Some(id) = id.clone() {
                    lsp_send_response(&mut writer, id, loc);
                }
            }

            _ => {
                // Unknown request → null response
                if let Some(id) = id {
                    lsp_send_response(&mut writer, id, serde_json::Value::Null);
                }
            }
        }
    }
}

// ── JSON-RPC transport ──────────────────────────────────────────────

fn lsp_read_message(reader: &mut impl std::io::BufRead) -> Option<serde_json::Value> {
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(len) = header.strip_prefix("Content-Length: ") {
            content_length = len.parse().ok()?;
        }
    }
    if content_length == 0 {
        return None;
    }
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn lsp_send_response(
    writer: &mut impl std::io::Write,
    id: serde_json::Value,
    result: serde_json::Value,
) {
    let msg = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
    let body = serde_json::to_string(&msg).unwrap();
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = writer.flush();
}

fn lsp_send_notification(
    writer: &mut impl std::io::Write,
    method: &str,
    params: serde_json::Value,
) {
    let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
    let body = serde_json::to_string(&msg).unwrap();
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = writer.flush();
}

fn lsp_server_capabilities() -> serde_json::Value {
    serde_json::json!({
        "capabilities": {
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": {"includeText": false}
            },
            "completionProvider": {
                "triggerCharacters": ["#", ">", "|", "=", "~", "@", "?", "."]
            },
            "hoverProvider": true,
            "definitionProvider": true
        },
        "serverInfo": {"name": "runa-lsp", "version": "0.1.0"}
    })
}

// ── Diagnostics ─────────────────────────────────────────────────────

fn lsp_analyze(writer: &mut impl std::io::Write, uri: &str, source: &str, prelude: &[Stmt]) {
    let mut diagnostics = Vec::new();

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);

    match parser.parse_program() {
        Err(error) => {
            diagnostics.push(lsp_parse_error_to_diag(&error));
        }
        Ok(user_stmts) => {
            let stmts = prepend_prelude(prelude.to_vec(), &user_stmts);
            let src_dir = lsp_source_dir(uri);
            let tc_diags = TypeChecker::check_with_diagnostics(&stmts, src_dir, source);
            for diag in &tc_diags {
                diagnostics.push(diagnostic_to_lsp(diag, source));
            }
        }
    }

    lsp_send_notification(
        writer,
        "textDocument/publishDiagnostics",
        serde_json::json!({"uri": uri, "diagnostics": diagnostics}),
    );
}

fn lsp_parse_error_to_diag(error: &str) -> serde_json::Value {
    let parts: Vec<&str> = error.splitn(3, ':').collect();
    let (line, col, message) = if parts.len() >= 3 {
        if let (Ok(l), Ok(c)) = (
            parts[0].trim().parse::<u32>(),
            parts[1].trim().parse::<u32>(),
        ) {
            (
                l.saturating_sub(1),
                c.saturating_sub(1),
                parts[2..].join(":").trim().to_string(),
            )
        } else {
            (0, 0, error.to_string())
        }
    } else {
        (0, 0, error.to_string())
    };

    serde_json::json!({
        "range": {
            "start": {"line": line, "character": col},
            "end": {"line": line, "character": col + 1}
        },
        "severity": 1,
        "source": "runa",
        "message": message
    })
}

fn lsp_type_error_to_diag(error: &str) -> serde_json::Value {
    // Type checker errors now carry LINE:COL: prefix (same format as parse errors)
    let parts: Vec<&str> = error.splitn(3, ':').collect();
    let (line, col, message) = if parts.len() >= 3 {
        if let (Ok(l), Ok(c)) = (
            parts[0].trim().parse::<u32>(),
            parts[1].trim().parse::<u32>(),
        ) {
            (
                l.saturating_sub(1),
                c.saturating_sub(1),
                parts[2..].join(":").trim().to_string(),
            )
        } else {
            (0, 0, error.to_string())
        }
    } else {
        (0, 0, error.to_string())
    };

    serde_json::json!({
        "range": {
            "start": {"line": line, "character": col},
            "end": {"line": line, "character": col + 1}
        },
        "severity": 1,
        "source": "runa",
        "message": message
    })
}

/// Convert a structured Diagnostic to LSP JSON format.
fn diagnostic_to_lsp(diag: &Diagnostic, source: &str) -> serde_json::Value {
    let (line, col, end_line, end_col) = if let Some(span) = diag.span {
        let (l, c) = span.start_line_col(source);
        let (el, ec) = span.end_line_col(source);
        (
            l.saturating_sub(1) as u32,
            c.saturating_sub(1) as u32,
            el.saturating_sub(1) as u32,
            ec.saturating_sub(1) as u32,
        )
    } else {
        (0, 0, 0, 1)
    };

    let severity = match diag.severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Help => 3,
    };

    let mut message = diag.message.clone();
    if !diag.context.is_empty() {
        message.push_str(&format!(" ({})", diag.context.join(", ")));
    }

    serde_json::json!({
        "range": {
            "start": {"line": line, "character": col},
            "end": {"line": end_line, "character": end_col}
        },
        "severity": severity,
        "source": "runa",
        "message": message
    })
}

fn lsp_find_symbol_pos(source: &str, name: &str) -> Option<(u32, u32)> {
    for (idx, line) in source.lines().enumerate() {
        if let Some(col) = line.find(name) {
            let before = if col > 0 {
                line.as_bytes().get(col - 1).copied().unwrap_or(b' ')
            } else {
                b' '
            };
            let after = line
                .as_bytes()
                .get(col + name.len())
                .copied()
                .unwrap_or(b' ');
            if !before.is_ascii_alphanumeric()
                && before != b'_'
                && !after.is_ascii_alphanumeric()
                && after != b'_'
            {
                return Some((idx as u32, col as u32));
            }
        }
    }
    None
}

fn lsp_source_dir(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    // Handle percent-encoded spaces on macOS
    let decoded = path.replace("%20", " ");
    std::path::Path::new(&decoded)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
}

// ── Go-to-definition ────────────────────────────────────────────────

fn lsp_definition(uri: &str, source: &str, line: u32, col: u32) -> serde_json::Value {
    let word = match lsp_word_at(source, line, col) {
        Some(w) => w,
        None => return serde_json::Value::Null,
    };

    if let Some((dl, dc)) = lsp_find_def_pos(source, &word) {
        return serde_json::json!({
            "uri": uri,
            "range": {
                "start": {"line": dl, "character": dc},
                "end": {"line": dl, "character": dc + word.len() as u32}
            }
        });
    }
    serde_json::Value::Null
}

fn lsp_find_def_pos(source: &str, name: &str) -> Option<(u32, u32)> {
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim();
        // Function: > name(
        if t.starts_with('>') {
            let rest = t[1..].trim();
            if rest.starts_with(name) {
                let after = &rest[name.len()..];
                if after.starts_with('(') || after.starts_with(' ') {
                    let col = line.find(name).unwrap_or(0);
                    return Some((idx as u32, col as u32));
                }
            }
        }
        // Type: # Name
        if t.starts_with('#') {
            let rest = t[1..].trim();
            if rest.starts_with(name) {
                let after = &rest[name.len()..];
                if after.is_empty()
                    || after.starts_with(' ')
                    || after.starts_with('(')
                    || after.starts_with('=')
                {
                    let col = line.find(name).unwrap_or(0);
                    return Some((idx as u32, col as u32));
                }
            }
        }
        // Binding: = name =
        if t.starts_with('=') {
            let rest = t[1..].trim();
            if rest.starts_with(name) {
                let after = rest[name.len()..].trim();
                if after.starts_with('=') {
                    let col = line.find(name).unwrap_or(0);
                    return Some((idx as u32, col as u32));
                }
            }
        }
        // Stream: ~ name =
        if t.starts_with('~') {
            let rest = t[1..].trim();
            if rest.starts_with(name) {
                let after = rest[name.len()..].trim();
                if after.starts_with('=') {
                    let col = line.find(name).unwrap_or(0);
                    return Some((idx as u32, col as u32));
                }
            }
        }
    }
    None
}

// ── Hover ───────────────────────────────────────────────────────────

fn lsp_hover(source: &str, line: u32, col: u32) -> serde_json::Value {
    let word = match lsp_word_at(source, line, col) {
        Some(w) => w,
        None => return serde_json::Value::Null,
    };

    // Search parsed AST for user-defined symbols
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    if let Ok(stmts) = parser.parse_program() {
        if let Some(info) = lsp_hover_from_ast(&word, &stmts) {
            return serde_json::json!({
                "contents": {"kind": "markdown", "value": format!("```runa\n{}\n```", info)}
            });
        }
    }

    // Builtins
    if let Some((sig, doc)) = lsp_builtin_doc(&word) {
        return serde_json::json!({
            "contents": {"kind": "markdown",
                "value": format!("```runa\n{}\n```\n\n{}", sig, doc)}
        });
    }

    serde_json::Value::Null
}

fn lsp_hover_from_ast(name: &str, stmts: &[Stmt]) -> Option<String> {
    for stmt in stmts {
        // Function
        if let Stmt::Defn(Defn::Fn {
            name: fn_name,
            params,
            ret_ty,
            ..
        }) = stmt
        {
            if fn_name == name {
                let ps = params
                    .iter()
                    .map(|p| {
                        if let Some(ty) = &p.ty {
                            format!("{}{}: {}", if p.inout { "inout " } else { "" }, p.name, ty)
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = ret_ty
                    .as_ref()
                    .map(|t| format!(" -> {}", t))
                    .unwrap_or_default();
                return Some(format!("> {}({}){}", fn_name, ps, ret));
            }
        }
        // Type
        if let Stmt::TypeDecl(TypeDecl::ADT {
            name: tn, variants, ..
        }) = stmt
        {
            if tn == name {
                let vs = variants
                    .iter()
                    .map(|v| {
                        if v.fields.is_empty() {
                            v.name.clone()
                        } else {
                            let fs = v
                                .fields
                                .iter()
                                .map(|f| {
                                    if v.positional {
                                        format!("{}", f.ty)
                                    } else {
                                        format!("{}: {}", f.name, f.ty)
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{}({})", v.name, fs)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                return Some(format!("# {} = {}", tn, vs));
            }
            // Constructor/variant
            for v in variants {
                if v.name == name {
                    if v.fields.is_empty() {
                        return Some(format!("{} (variant of {})", name, tn));
                    }
                    let fs = v
                        .fields
                        .iter()
                        .map(|f| {
                            if v.positional {
                                format!("{}", f.ty)
                            } else {
                                format!("{}: {}", f.name, f.ty)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Some(format!("{}({}) — variant of {}", name, fs, tn));
                }
            }
        }
        // Actor
        if let Stmt::Defn(Defn::Actor {
            name: an,
            state_param,
            handlers,
        }) = stmt
        {
            if an == name {
                let msgs = handlers
                    .iter()
                    .filter_map(|h| {
                        if let Pat::Con(n, _) = &h.msg_pat {
                            Some(n.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return Some(format!(
                    "> actor {}(state: {}) handles [{}]",
                    an,
                    state_param
                        .ty
                        .as_ref()
                        .map(|t| format!("{}", t))
                        .unwrap_or_default(),
                    msgs
                ));
            }
        }
    }
    None
}

// ── Completion ──────────────────────────────────────────────────────

fn lsp_completions(source: &str, line: u32, col: u32) -> Vec<serde_json::Value> {
    let lines: Vec<&str> = source.lines().collect();
    let line_text = lines.get(line as usize).copied().unwrap_or("");
    let prefix = if (col as usize) <= line_text.len() {
        &line_text[..col as usize]
    } else {
        ""
    };
    let trimmed = prefix.trim();

    let mut items = Vec::new();

    // At start of line → rune snippets
    if trimmed.is_empty() {
        items.extend(lsp_rune_snippets());
        return items;
    }

    // Word being typed
    let word = lsp_word_before(line_text, col as usize);

    // Parse source for user-defined symbols
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, source);
    if let Ok(stmts) = parser.parse_program() {
        for stmt in &stmts {
            match stmt {
                Stmt::Defn(Defn::Fn {
                    name,
                    params,
                    ret_ty,
                    ..
                }) => {
                    if word.is_empty() || name.starts_with(&word) {
                        let ps = params
                            .iter()
                            .map(|p| {
                                if let Some(ty) = &p.ty {
                                    format!("{}: {}", p.name, ty)
                                } else {
                                    p.name.clone()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let ret = ret_ty
                            .as_ref()
                            .map(|t| format!(" -> {}", t))
                            .unwrap_or_default();
                        items.push(serde_json::json!({
                            "label": name,
                            "kind": 3,
                            "detail": format!("> {}({}){}", name, ps, ret)
                        }));
                    }
                }
                Stmt::TypeDecl(TypeDecl::ADT {
                    name: tn, variants, ..
                }) => {
                    if word.is_empty() || tn.starts_with(&word) {
                        items.push(serde_json::json!({
                            "label": tn, "kind": 22, "detail": "type"
                        }));
                    }
                    for v in variants {
                        if word.is_empty() || v.name.starts_with(&word) {
                            items.push(serde_json::json!({
                                "label": &v.name, "kind": 21,
                                "detail": format!("variant of {}", tn)
                            }));
                        }
                    }
                }
                Stmt::Bind(Pat::Var(name), _, _) | Stmt::Bind(Pat::Con(name, _), _, _) => {
                    if word.is_empty() || name.starts_with(&word) {
                        items.push(serde_json::json!({"label": name, "kind": 6}));
                    }
                }
                Stmt::StreamBind(name, _) => {
                    if word.is_empty() || name.starts_with(&word) {
                        items.push(serde_json::json!({
                            "label": name, "kind": 6, "detail": "stream"
                        }));
                    }
                }
                _ => {}
            }
        }
    }

    // Builtins
    for (bname, _) in LSP_BUILTINS {
        if word.is_empty() || bname.starts_with(&word) {
            items.push(serde_json::json!({
                "label": bname, "kind": 3, "detail": "builtin"
            }));
        }
    }

    // Keywords
    for kw in &["match", "for", "if", "true", "false", "in", "inout"] {
        if word.is_empty() || kw.starts_with(&word) {
            items.push(serde_json::json!({"label": kw, "kind": 14}));
        }
    }

    items
}

fn lsp_rune_snippets() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "label": "# Type", "kind": 15,
            "insertText": "# ${1:Name} = ${2:Variant1} | ${3:Variant2}",
            "insertTextFormat": 2,
            "detail": "Type declaration",
            "sortText": "0a"
        }),
        serde_json::json!({
            "label": "> Function", "kind": 15,
            "insertText": "> ${1:name}(${2:params}) -> ${3:RetType} {\n    ${0}\n}",
            "insertTextFormat": 2,
            "detail": "Function definition",
            "sortText": "0b"
        }),
        serde_json::json!({
            "label": "| Rule", "kind": 15,
            "insertText": "| ${1:name}(${2:x}) -> ${0:value}",
            "insertTextFormat": 2,
            "detail": "Rule / invariant",
            "sortText": "0c"
        }),
        serde_json::json!({
            "label": "= Binding", "kind": 15,
            "insertText": "= ${1:name} = ${0:value}",
            "insertTextFormat": 2,
            "detail": "Value binding",
            "sortText": "0d"
        }),
        serde_json::json!({
            "label": "~ Stream", "kind": 15,
            "insertText": "~ ${1:name} = ${0:stream_expr}",
            "insertTextFormat": 2,
            "detail": "Stream binding",
            "sortText": "0e"
        }),
        serde_json::json!({
            "label": "@ Effect", "kind": 15,
            "insertText": "@ ${0:print(\"message\")}",
            "insertTextFormat": 2,
            "detail": "Effect / meta",
            "sortText": "0f"
        }),
        serde_json::json!({
            "label": "? Verify", "kind": 15,
            "insertText": "? ${0:invariant_name}",
            "insertTextFormat": 2,
            "detail": "Verification",
            "sortText": "0g"
        }),
        serde_json::json!({
            "label": "for", "kind": 15,
            "insertText": "for ${1:x} in ${2:xs} {\n    ${0}\n}",
            "insertTextFormat": 2,
            "detail": "For loop",
            "sortText": "0h"
        }),
        serde_json::json!({
            "label": "match", "kind": 15,
            "insertText": "match ${1:expr} {\n    | ${2:Pat} -> ${0}\n}",
            "insertTextFormat": 2,
            "detail": "Pattern match",
            "sortText": "0i"
        }),
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────

fn lsp_word_at(source: &str, line: u32, col: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let text = lines.get(line as usize)?;
    let chars: Vec<char> = text.chars().collect();
    let c = (col as usize).min(chars.len());
    let is_wc = |ch: char| ch.is_alphanumeric() || ch == '_';

    let mut start = c;
    if start < chars.len() && !is_wc(chars[start]) {
        if start > 0 && is_wc(chars[start - 1]) {
            start -= 1;
        } else {
            return None;
        }
    }
    while start > 0 && is_wc(chars[start - 1]) {
        start -= 1;
    }
    let mut end = start;
    while end < chars.len() && is_wc(chars[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(chars[start..end].iter().collect())
}

fn lsp_word_before(line: &str, col: usize) -> String {
    let chars: Vec<char> = line.chars().collect();
    let c = col.min(chars.len());
    let mut s = c;
    while s > 0 && (chars[s - 1].is_alphanumeric() || chars[s - 1] == '_') {
        s -= 1;
    }
    chars[s..c].iter().collect()
}

// ── Builtin documentation ───────────────────────────────────────────

static LSP_BUILTINS: &[(&str, usize)] = &[
    ("print", 1),
    ("show", 1),
    ("length", 1),
    ("append", 2),
    ("head", 1),
    ("tail", 1),
    ("nth", 2),
    ("reverse", 1),
    ("range", 2),
    ("map", 2),
    ("filter", 2),
    ("foldl", 3),
    ("scan", 3),
    ("merge", 2),
    ("take", 2),
    ("skip", 2),
    ("collect", 1),
    ("count", 1),
    ("sum", 1),
    ("distinct", 1),
    ("window", 2),
    ("from_list", 1),
    ("sort", 1),
    ("sort_by", 2),
    ("any", 2),
    ("all", 2),
    ("find", 2),
    ("flat_map", 2),
    ("zip", 2),
    ("enumerate", 1),
    ("take_while", 2),
    ("drop_while", 2),
    ("partition", 2),
    ("chunked", 2),
    ("join", 2),
    ("split", 2),
    ("trim", 1),
    ("contains", 2),
    ("starts_with", 2),
    ("ends_with", 2),
    ("replace", 3),
    ("to_upper", 1),
    ("to_lower", 1),
    ("substring", 3),
    ("char_at", 2),
    ("index_of", 2),
    ("parse_int", 1),
    ("parse_float", 1),
    ("string_chars", 1),
    ("format_float", 2),
    ("read_file", 1),
    ("write_file", 2),
    ("append_file", 2),
    ("file_exists", 1),
    ("read_lines", 1),
    ("env_var", 1),
    ("json_parse", 1),
    ("json_get", 2),
    ("json_emit", 1),
    ("json_string", 1),
    ("json_number", 1),
    ("json_bool", 1),
    ("json_array", 1),
    ("json_object", 1),
    ("http_get", 1),
    ("http_post", 2),
    ("db_open", 1),
    ("db_exec", 2),
    ("db_query", 2),
    ("db_close", 1),
    ("tap", 2),
    ("first", 1),
    ("last", 1),
    ("reduce", 3),
    ("start_with", 2),
    ("concat", 2),
    ("pairwise", 1),
    ("fst", 1),
    ("snd", 1),
    ("combine_latest", 2),
    ("subject", 1),
    ("debounce", 2),
    ("throttle", 2),
    ("delay", 2),
    ("buffer", 2),
    ("timeout", 2),
    ("switch_map", 2),
    ("sample", 2),
];

fn lsp_builtin_doc(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "print" => Some(("@ print(msg: String)", "Print a message to stdout")),
        "show" => Some((
            "> show(value) -> String",
            "Convert any value to string representation",
        )),
        "map" => Some((
            "> map(list, f: (a) -> b) -> [b]",
            "Apply function to each element",
        )),
        "filter" => Some((
            "> filter(list, f: (a) -> Bool) -> [a]",
            "Keep elements where predicate is true",
        )),
        "foldl" => Some((
            "> foldl(list, init, f: (acc, x) -> acc) -> acc",
            "Left fold over a list",
        )),
        "scan" => Some((
            "~ scan(stream, init, f) -> Stream",
            "Running accumulator over stream values",
        )),
        "merge" => Some(("~ merge(s1, s2) -> Stream", "Merge two streams")),
        "take" => Some(("> take(list, n: Int) -> [a]", "Take first n elements")),
        "skip" => Some(("> skip(list, n: Int) -> [a]", "Skip first n elements")),
        "collect" => Some((
            "~ collect(stream) -> [a]",
            "Collect stream values into a list",
        )),
        "count" => Some(("~ count(stream) -> Int", "Count elements")),
        "sum" => Some(("~ sum(stream) -> Int", "Sum all elements")),
        "distinct" => Some(("~ distinct(stream) -> Stream", "Remove duplicates")),
        "from_list" => Some(("~ from_list(list) -> Stream", "Create stream from list")),
        "sort" => Some(("> sort(list) -> [a]", "Sort in ascending order")),
        "join" => Some((
            "> join(list: [String], sep: String) -> String",
            "Join strings with separator",
        )),
        "split" => Some((
            "> split(s: String, sep: String) -> [String]",
            "Split by separator",
        )),
        "trim" => Some((
            "> trim(s: String) -> String",
            "Remove leading/trailing whitespace",
        )),
        "contains" => Some((
            "> contains(s: String, sub: String) -> Bool",
            "Check if string contains substring",
        )),
        "length" => Some(("> length(list) -> Int", "Number of elements")),
        "reverse" => Some(("> reverse(list) -> [a]", "Reverse a list")),
        "range" => Some((
            "> range(start: Int, end: Int) -> [Int]",
            "Integer range [start, end)",
        )),
        "head" => Some(("> head(list) -> a", "First element")),
        "tail" => Some(("> tail(list) -> [a]", "All elements except first")),
        "nth" => Some(("> nth(list, index: Int) -> a", "Element at index (0-based)")),
        "first" => Some(("~ first(stream) -> a", "First stream element")),
        "last" => Some(("~ last(stream) -> a", "Last stream element")),
        "reduce" => Some(("~ reduce(stream, init, f) -> a", "Terminal fold")),
        "tap" => Some(("~ tap(stream, f) -> Stream", "Side-effect observation")),
        "pairwise" => Some(("~ pairwise(stream) -> Stream((a, a))", "Consecutive pairs")),
        "fst" => Some(("> fst(tuple) -> a", "First element of tuple")),
        "snd" => Some(("> snd(tuple) -> b", "Second element of tuple")),
        "subject" => Some(("~ subject(init) -> Subject", "Create pushable subject")),
        "start_with" => Some(("~ start_with(stream, val) -> Stream", "Prepend value")),
        "concat" => Some(("~ concat(s1, s2) -> Stream", "Concatenate streams")),
        "read_file" => Some(("@ read_file(path: String) -> String", "Read file contents")),
        "write_file" => Some((
            "@ write_file(path: String, content: String)",
            "Write to file",
        )),
        "json_parse" => Some(("> json_parse(s: String) -> Json", "Parse JSON string")),
        "json_get" => Some((
            "> json_get(json: Json, key: String) -> Json",
            "Get field from JSON",
        )),
        _ => None,
    }
}

// ============================================================================
// PART 9: PRE-CODEGEN TYPE CHECKER (M16)
// ============================================================================
//

// ============================================================================
// BUILTIN REGISTRY — Data-driven builtin codegen
// ============================================================================
//
// Each BuiltinDef describes one built-in function for the Rust backend.
// The registry replaces the if-chain in emit_expr, centralizing:
//   - arity checking, user-function shadowing, dependency injection
//   - purity analysis (impure builtins prevent auto-comptime folding)
//   - Rust code templates (future: add wasm_tpl for WASM backend)

struct BuiltinDef {
    arity: usize,
    shadowable: bool,
    impure: bool,
    deps: &'static [(&'static str, &'static str)],
    rust_tpl: &'static str,
}

fn apply_builtin_template(tpl: &str, args: &[String]) -> String {
    let mut s = tpl.to_string();
    for (i, arg) in args.iter().enumerate() {
        s = s.replace(&format!("{{{}}}", i), arg);
    }
    s
}

fn rust_builtin_registry() -> BTreeMap<String, BuiltinDef> {
    const D: &[(&str, &str)] = &[];
    const SERDE: &[(&str, &str)] = &[("serde_json", "1")];
    const UREQ: &[(&str, &str)] = &[("ureq", "2")];
    const TINY: &[(&str, &str)] = &[("tiny_http", "0.12")];
    const AXUM: &[(&str, &str)] = &[
        ("axum", "0.8"),
        ("tokio", "{ version = \"1\", features = [\"full\"] }"),
    ];
    const RSQL: &[(&str, &str)] = &[(
        "rusqlite",
        "{ version = \"0.32\", features = [\"bundled\"] }",
    )];

    let entries: Vec<(&str, BuiltinDef)> = vec![
        // ---- Math (not shadowable, pure) ----
        ("exp",      BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).exp()" }),
        ("ln",       BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).ln()" }),
        ("sqrt",     BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).sqrt()" }),
        ("pow",      BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).powf({1} as f64)" }),
        ("abs",      BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0}).abs()" }),
        ("to_float", BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64)" }),
        ("round",    BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "(({0} as f64).round() as i64)" }),
        ("floor",    BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "(({0} as f64).floor() as i64)" }),
        ("max_f",    BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).max({1} as f64)" }),
        ("min_f",    BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "({0} as f64).min({1} as f64)" }),

        // ---- String (shadowable, pure) ----
        ("split",        BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.split(&*{1}).map(|s| s.to_string()).collect::<Vec<String>>()" }),
        ("join",         BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.join(&*{1})" }),
        ("trim",         BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.trim().to_string()" }),
        ("contains",     BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.contains(&*{1})" }),
        ("starts_with",  BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.starts_with(&*{1})" }),
        ("ends_with",    BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.ends_with(&*{1})" }),
        ("replace",      BuiltinDef { arity: 3, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.replace(&*{1}, &*{2})" }),
        ("to_upper",     BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.to_uppercase()" }),
        ("to_lower",     BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.to_lowercase()" }),
        ("substring",    BuiltinDef { arity: 3, shadowable: true, impure: false, deps: D, rust_tpl: "{ let __s: Vec<char> = {0}.chars().collect(); let __start = ({1} as usize).min(__s.len()); let __end = ({2} as usize).min(__s.len()); __s[__start..__end].iter().collect::<String>() }" }),
        ("char_at",      BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{ let __s: Vec<char> = {0}.chars().collect(); let __i = {1} as usize; if __i < __s.len() { __s[__i].to_string() } else { String::new() } }" }),
        ("index_of",     BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.find(&*{1}).map(|p| p as i64).unwrap_or(-1i64)" }),
        ("format_float", BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "format!(\"{:.prec$}\", {0} as f64, prec = {1} as usize)" }),
        ("parse_int",    BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.trim().parse::<i64>().unwrap_or(0)" }),
        ("parse_float",  BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.trim().parse::<f64>().unwrap_or(0.0)" }),
        ("string_chars", BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.chars().map(|c| c.to_string()).collect::<Vec<String>>()" }),
        ("string_length",BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0}.len() as i64)" }),
        ("length",       BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "({0}.len() as i64)" }),
        ("head",         BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}[0].clone()" }),
        ("tail",         BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}[1..].to_vec()" }),
        ("nth",          BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}[{1} as usize].clone()" }),

        // ---- File I/O (not shadowable, impure) ----
        ("read_file",    BuiltinDef { arity: 1, shadowable: false, impure: true, deps: D, rust_tpl: "std::fs::read_to_string(&*{0}).unwrap_or_default()" }),
        ("write_file",   BuiltinDef { arity: 2, shadowable: false, impure: true, deps: D, rust_tpl: "{ let _ = std::fs::write(&*{0}, &*{1}); }" }),
        ("append_file",  BuiltinDef { arity: 2, shadowable: false, impure: true, deps: D, rust_tpl: "{ use std::io::Write; if let Ok(mut __f) = std::fs::OpenOptions::new().append(true).create(true).open(&*{0}) { let _ = __f.write_all({1}.as_bytes()); } }" }),
        ("file_exists",  BuiltinDef { arity: 1, shadowable: false, impure: true, deps: D, rust_tpl: "std::path::Path::new(&*{0}).exists()" }),
        ("read_lines",   BuiltinDef { arity: 1, shadowable: false, impure: true, deps: D, rust_tpl: "std::fs::read_to_string(&*{0}).unwrap_or_default().lines().map(|l| l.to_string()).collect::<Vec<String>>()" }),
        ("env_var",      BuiltinDef { arity: 1, shadowable: false, impure: true, deps: D, rust_tpl: "std::env::var(&*{0}).unwrap_or_default()" }),

        // ---- JSON (shadowable, pure, deps: serde_json) ----
        ("json_parse",   BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __s = {0}; match serde_json::from_str::<serde_json::Value>(&__s) { Ok(_) => __s, Err(_) => \"null\".to_string() } }" }),
        ("json_get",     BuiltinDef { arity: 2, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __j: serde_json::Value = serde_json::from_str(&{0}).unwrap_or(serde_json::Value::Null); match __j.get(&*{1}) { Some(v) => v.to_string(), None => \"null\".to_string() } }" }),
        ("json_string",  BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __j: serde_json::Value = serde_json::from_str(&{0}).unwrap_or(serde_json::Value::Null); match __j { serde_json::Value::String(s) => s, _ => {0}.trim_matches('\"').to_string() } }" }),
        ("json_number",  BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __j: serde_json::Value = serde_json::from_str(&{0}).unwrap_or(serde_json::Value::Null); __j.as_f64().unwrap_or(0.0) }" }),
        ("json_bool",    BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __j: serde_json::Value = serde_json::from_str(&{0}).unwrap_or(serde_json::Value::Null); __j.as_bool().unwrap_or(false) }" }),
        ("json_array",   BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __j: serde_json::Value = serde_json::from_str(&{0}).unwrap_or(serde_json::Value::Null); match __j { serde_json::Value::Array(a) => a.iter().map(|v| v.to_string()).collect::<Vec<String>>(), _ => vec![] } }" }),
        ("json_emit",    BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{0}.clone()" }),
        ("json_object",  BuiltinDef { arity: 1, shadowable: true, impure: false, deps: SERDE, rust_tpl: "{ let __pairs = &{0}; let mut __obj = serde_json::Map::new(); for __p in __pairs.iter() { if __p.len() >= 2 { let __k = __p[0].clone(); let __v: serde_json::Value = serde_json::from_str(&__p[1]).unwrap_or(serde_json::Value::String(__p[1].clone())); __obj.insert(__k, __v); } } serde_json::Value::Object(__obj).to_string() }" }),

        // ---- HTTP (shadowable, impure for I/O ones) ----
        ("http_get",     BuiltinDef { arity: 1, shadowable: true, impure: true, deps: UREQ, rust_tpl: "ureq::get(&*{0}).call().map(|r| r.into_string().unwrap_or_default()).unwrap_or_default()" }),
        ("http_post",    BuiltinDef { arity: 2, shadowable: true, impure: true, deps: UREQ, rust_tpl: "ureq::post(&*{0}).send_string(&*{1}).map(|r| r.into_string().unwrap_or_default()).unwrap_or_default()" }),
        ("http_serve",   BuiltinDef { arity: 2, shadowable: true, impure: true, deps: AXUM, rust_tpl: "{ let __handler = {1}; let __port = {0}; let __app = axum::Router::new().fallback(move |__req: axum::extract::Request| {{ let __h = __handler.clone(); async move {{ let __path = __req.uri().path().to_string(); let __method = __req.method().to_string(); let __body_bytes = axum::body::to_bytes(__req.into_body(), 1048576).await.unwrap_or_default(); let __body = String::from_utf8_lossy(&__body_bytes).to_string(); let __result: (i64, String, String) = __h(__path, __method, __body); axum::http::Response::builder().status(__result.0 as u16).header(\"Content-Type\", __result.1).body(axum::body::Body::from(__result.2)).unwrap() }} }}); let __listener = tokio::net::TcpListener::bind(format!(\"0.0.0.0:{}\", __port)).await.expect(\"Failed to bind\"); println!(\"Listening on port {}\", __port); axum::serve(__listener, __app).await.unwrap(); }" }),
        ("http_respond", BuiltinDef { arity: 3, shadowable: true, impure: false, deps: D, rust_tpl: "({0} as i64, {1}.to_string(), {2}.to_string())" }),
        ("http_request_path",   BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.0.clone()" }),
        ("http_request_method", BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.1.clone()" }),
        ("http_request_body",   BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.2.clone()" }),

        // ---- Database (shadowable, impure, deps: rusqlite) ----
        ("db_open",      BuiltinDef { arity: 1, shadowable: true, impure: true, deps: RSQL, rust_tpl: "std::sync::Arc::new(std::sync::Mutex::new(rusqlite::Connection::open(&*{0}).expect(\"Failed to open database\")))" }),
        ("db_exec",      BuiltinDef { arity: 2, shadowable: true, impure: true, deps: RSQL, rust_tpl: "{0}.lock().unwrap().execute_batch(&*{1}).expect(\"db_exec failed\")" }),
        ("db_query",     BuiltinDef { arity: 2, shadowable: true, impure: true, deps: RSQL, rust_tpl: "{ let __rc = {0}; let __db = __rc.lock().unwrap(); let mut __stmt = __db.prepare(&*{1}).expect(\"SQL prepare failed\"); let __result: Vec<Vec<String>> = __stmt.query_map(rusqlite::params![], |row: &rusqlite::Row| { let mut __cols = Vec::new(); let mut __i = 0usize; loop { match row.get_ref(__i) { Ok(v) => { __cols.push(match v { rusqlite::types::ValueRef::Null => \"null\".to_string(), rusqlite::types::ValueRef::Integer(n) => n.to_string(), rusqlite::types::ValueRef::Real(f) => f.to_string(), rusqlite::types::ValueRef::Text(s) => String::from_utf8_lossy(s).to_string(), rusqlite::types::ValueRef::Blob(b) => format!(\"<blob:{}>\", b.len()), }); __i += 1; }, Err(_) => break, } } Ok(__cols) }).expect(\"query failed\").filter_map(|r| r.ok()).collect(); __result }" }),
        ("db_query_row", BuiltinDef { arity: 2, shadowable: true, impure: true, deps: RSQL, rust_tpl: "{ let __rc = {0}; let __db = __rc.lock().unwrap(); let mut __stmt = __db.prepare(&*{1}).expect(\"SQL prepare failed\"); let __result: String = __stmt.query_map(rusqlite::params![], |row: &rusqlite::Row| { let mut __cols = Vec::new(); let mut __i = 0usize; loop { match row.get_ref(__i) { Ok(v) => { __cols.push(match v { rusqlite::types::ValueRef::Null => \"null\".to_string(), rusqlite::types::ValueRef::Integer(n) => n.to_string(), rusqlite::types::ValueRef::Real(f) => f.to_string(), rusqlite::types::ValueRef::Text(s) => String::from_utf8_lossy(s).to_string(), rusqlite::types::ValueRef::Blob(b) => format!(\"<blob:{}>\", b.len()), }); __i += 1; }, Err(_) => break, } } Ok(__cols) }).expect(\"query failed\").filter_map(|r| r.ok()).next().map(|cols| if cols.len() == 1 { cols.into_iter().next().unwrap() } else { cols.join(\", \") }).unwrap_or_default(); __result }" }),
        ("db_insert",    BuiltinDef { arity: 2, shadowable: true, impure: true, deps: RSQL, rust_tpl: "{ let __rc = {0}; let __db = __rc.lock().unwrap(); __db.execute_batch(&*{1}).expect(\"insert failed\"); __db.last_insert_rowid() }" }),
        ("db_close",     BuiltinDef { arity: 1, shadowable: true, impure: true, deps: D, rust_tpl: "drop({0})" }),

        // ---- Misc (not shadowable, pure) ----
        ("shared",       BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "std::sync::Arc::new({0})" }),
        ("range",        BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "({0}..{1})" }),

        // ---- Functional / Collection (shadowable, pure) ----
        // Unified: list and stream ops share names. Templates use .clone() for pipe safety.
        ("map",          BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().map({1}).collect::<Vec<_>>()" }),
        ("filter",       BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().filter(|x| ({1})( x.clone())).collect::<Vec<_>>()" }),
        ("foldl",        BuiltinDef { arity: 3, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().fold({1}, {2})" }),
        ("sort",         BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{ let mut __v = {0}.clone(); __v.sort_by(|a, b| format!(\"{}\", a).cmp(&format!(\"{}\", b))); __v }" }),
        ("sort_by",      BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{ let mut __v = {0}.clone(); __v.sort_by(|a, b| format!(\"{}\", ({1})(a.clone())).cmp(&format!(\"{}\", ({1})(b.clone())))); __v }" }),
        ("reverse",      BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{ let mut __v = {0}.clone(); __v.reverse(); __v }" }),
        ("any",          BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().any(|x| ({1})( x.clone()))" }),
        ("all",          BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().all(|x| ({1})( x.clone()))" }),
        ("find",         BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.iter().find(|x| ({1})((*x).clone())).cloned()" }),
        ("flat_map",     BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().flat_map({1}).collect::<Vec<_>>()" }),
        ("zip",          BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().zip({1}.clone().into_iter()).collect::<Vec<_>>()" }),
        ("enumerate",    BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().enumerate().map(|(i, v)| (i as i64, v)).collect::<Vec<_>>()" }),
        ("take_while",   BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().take_while(|x| ({1})(x.clone())).collect::<Vec<_>>()" }),
        ("drop_while",   BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().skip_while(|x| ({1})(x.clone())).collect::<Vec<_>>()" }),
        ("sum_list",     BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.iter().map(|x| *x as i64).sum::<i64>()" }),
        ("distinct",     BuiltinDef { arity: 1, shadowable: true, impure: false, deps: D, rust_tpl: "{ let mut __seen = std::collections::HashSet::new(); {0}.clone().into_iter().filter(|x| __seen.insert(format!(\"{}\", x))).collect::<Vec<_>>() }" }),
        ("count_by",     BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{0}.iter().filter(|x| ({1})((*x).clone())).count() as i64" }),
        ("partition",    BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{ let (__yes, __no): (Vec<_>, Vec<_>) = {0}.clone().into_iter().partition(|x| ({1})(x.clone())); (__yes, __no) }" }),
        ("chunked",      BuiltinDef { arity: 2, shadowable: true, impure: false, deps: D, rust_tpl: "{ let __v = {0}.clone(); let __n = ({1} as usize).max(1); __v.chunks(__n).map(|c| c.to_vec()).collect::<Vec<Vec<_>>>() }" }),
        ("subscribe",    BuiltinDef { arity: 2, shadowable: true, impure: true, deps: D, rust_tpl: "{ for __item in {0}.iter() { ({1})(__item.clone()); } }" }),

        // ---- Map builtins (M24) ----
        ("map_new",      BuiltinDef { arity: 0, shadowable: false, impure: false, deps: D, rust_tpl: "HashMap::new()" }),
        ("map_insert",   BuiltinDef { arity: 3, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __m = {0}.clone(); __m.insert({1}.clone(), {2}.clone()); __m }" }),
        ("map_get",      BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.get(&{1}).cloned()" }),
        ("map_get_or",   BuiltinDef { arity: 3, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.get(&{1}).cloned().unwrap_or_else(|| {2}.clone())" }),
        ("map_contains", BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.contains_key(&{1})" }),
        ("map_remove",   BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __m = {0}.clone(); __m.remove(&{1}); __m }" }),
        ("map_keys",     BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.keys().cloned().collect::<Vec<_>>()" }),
        ("map_values",   BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.values().cloned().collect::<Vec<_>>()" }),
        ("map_entries",  BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()" }),
        ("map_len",      BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0}.len() as i64)" }),
        ("map_merge",    BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __m = {0}.clone(); __m.extend({1}.clone()); __m }" }),
        ("map_from",     BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.into_iter().collect::<HashMap<_, _>>()" }),

        // ---- Set builtins (M24) ----
        ("set_new",       BuiltinDef { arity: 0, shadowable: false, impure: false, deps: D, rust_tpl: "HashSet::new()" }),
        ("set_insert",    BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __s = {0}.clone(); __s.insert({1}.clone()); __s }" }),
        ("set_contains",  BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.contains(&{1})" }),
        ("set_remove",    BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __s = {0}.clone(); __s.remove(&{1}); __s }" }),
        ("set_len",       BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0}.len() as i64)" }),
        ("set_to_list",   BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.iter().cloned().collect::<Vec<_>>()" }),
        ("set_union",     BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.union(&{1}).cloned().collect::<HashSet<_>>()" }),
        ("set_intersect", BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.intersection(&{1}).cloned().collect::<HashSet<_>>()" }),
        ("set_diff",      BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.difference(&{1}).cloned().collect::<HashSet<_>>()" }),
        ("set_from_list", BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.into_iter().collect::<HashSet<_>>()" }),

        // ---- Stream builtins (M12, sync Vec-based — clean names, no s_ prefix) ----
        ("from_list",    BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone()" }),
        ("scan",         BuiltinDef { arity: 3, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut acc = {1}; {0}.clone().into_iter().map(|x| { acc = ({2})(acc.clone(), x); acc.clone() }).collect::<Vec<_>>() }" }),
        ("merge",        BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut m = Vec::new(); let (mut a, mut b) = ({0}.clone().into_iter(), {1}.clone().into_iter()); loop { match (a.next(), b.next()) { (Some(x), Some(y)) => { m.push(x); m.push(y); }, (Some(x), None) => { m.push(x); m.extend(a); break; }, (None, Some(y)) => { m.push(y); m.extend(b); break; }, _ => break } }; m }" }),
        ("take",         BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().take(({1}).max(0) as usize).collect::<Vec<_>>()" }),
        ("collect",      BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone()" }),
        ("count",        BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "({0}.len() as i64)" }),
        ("skip",         BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().skip(({1}).max(0) as usize).collect::<Vec<_>>()" }),
        ("window",       BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let src: Vec<_> = {0}.clone().into_iter().collect(); let __n = ({1} as usize).max(1); src.windows(__n).map(|w| w.to_vec()).collect::<Vec<Vec<_>>>() }" }),
        ("sum",          BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().reduce(|a, b| a + b).unwrap_or_default()" }),
        ("last",         BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().last().unwrap_or_default()" }),
        ("combine_latest", BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let a: Vec<_> = {0}.clone().into_iter().collect(); let b: Vec<_> = {1}.clone().into_iter().collect(); if a.is_empty() || b.is_empty() {{ vec![] }} else {{ let n = a.len().max(b.len()); (0..n).map(|i| (a.get(i).or(a.last()).cloned().unwrap(), b.get(i).or(b.last()).cloned().unwrap())).collect::<Vec<_>>() }} }" }),

        // ---- Stream lifecycle (sync mode) ----
        ("complete",     BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}" }),
        ("error",        BuiltinDef { arity: 2, shadowable: false, impure: true, deps: D, rust_tpl: "{ eprintln!(\"stream error: {}\", {1}); {0}.clone() }" }),
        ("take_until",   BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}" }),
        ("poll",         BuiltinDef { arity: 2, shadowable: false, impure: true, deps: D, rust_tpl: "({0})()" }),

        // ---- New stream operators (M17b) ----
        ("tap",          BuiltinDef { arity: 2, shadowable: false, impure: true, deps: D, rust_tpl: "{ let __v = {0}.clone(); for __x in __v.iter() {{ let __f = {1}; __f(__x.clone()); }} __v }" }),
        ("catch",        BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone()" }),
        ("first",        BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().into_iter().next().unwrap_or_default()" }),
        ("reduce",       BuiltinDef { arity: 3, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __acc = {1}; for __x in {0}.clone().into_iter() {{ __acc = ({2})(__acc.clone(), __x); }} __acc }" }),
        ("start_with",   BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __v = vec![{1}]; __v.extend({0}.clone()); __v }" }),
        ("concat",       BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let mut __v = {0}.clone(); __v.extend({1}.clone()); __v }" }),
        ("pairwise",     BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone().windows(2).map(|w| (w[0].clone(), w[1].clone())).collect::<Vec<_>>()" }),
        ("fst",          BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.0" }),
        ("snd",          BuiltinDef { arity: 1, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.1" }),

        // ---- Timing operators (M17, sync mode) ----
        ("debounce",     BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let __v: Vec<_> = {0}.clone(); if let Some(__last) = __v.last() {{ vec![__last.clone()] }} else {{ vec![] }} }" }),
        ("throttle",     BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let __v: Vec<_> = {0}.clone(); let __step = if {1} > 0 {{ (__v.len() / 10).max(1) }} else {{ 1 }}; __v.iter().step_by(__step).cloned().collect::<Vec<_>>() }" }),
        ("delay",        BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone()" }),
        ("buffer",       BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "vec![{0}.clone()]" }),
        ("timeout",      BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{0}.clone()" }),
        ("switch_map",   BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let __v: Vec<_> = {0}.clone(); if let Some(__last) = __v.last() {{ ({1})(__last.clone()) }} else {{ vec![] }} }" }),
        ("sample",       BuiltinDef { arity: 2, shadowable: false, impure: false, deps: D, rust_tpl: "{ let __src: Vec<_> = {0}.clone(); let __trg: Vec<_> = {1}.clone(); let __tlen = __trg.len().max(1); __trg.iter().enumerate().filter_map(|(i, _)| { let __idx = ((i + 1) * __src.len()) / __tlen; __src.get(__idx.min(__src.len().saturating_sub(1))).cloned() }).collect::<Vec<_>>() }" }),
    ];
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

/// Shared type metadata: types, variants, constructors, field info.
/// Populated once during declaration scanning, consumed by analysis passes and emission.
#[derive(Debug, Clone)]
struct TypeRegistry {
    /// Type declarations: name -> list of type params + list of variants
    type_decls: BTreeMap<String, (Vec<String>, Vec<String>)>,
    /// Maps variant name -> parent ADT name (e.g. "Some" -> "FuturunaOption")
    variant_parent: BTreeMap<String, String>,
    /// Maps original ADT name -> Rust-safe name (e.g. "Option" -> "FuturunaOption")
    type_rename: BTreeMap<String, String>,
    /// Maps variant name -> which argument indices need Box::new() wrapping (recursive fields)
    variant_boxed_args: BTreeMap<String, Vec<usize>>,
    /// Maps variant name -> whether it uses positional (tuple) fields
    variant_positional: BTreeMap<String, bool>,
    /// Maps variant name -> field names (for named/struct variants)
    variant_fields: BTreeMap<String, Vec<String>>,
    /// Maps variant name -> (field name -> field type) for non-Copy field detection
    variant_field_types: BTreeMap<String, BTreeMap<String, Ty>>,
    /// Types with explicit user-provided Display impl (skip auto-generation)
    explicit_display_impls: BTreeSet<String>,
    /// Types that are structs (single-variant ADTs where variant name == type name)
    struct_types: BTreeSet<String>,
    /// Immutable recursive ADT names that use Rc/Arc instead of Box
    rc_types: BTreeSet<String>,
    /// Effect declarations: effect_name -> set of operation names
    effect_ops: BTreeMap<String, BTreeSet<String>>,
    /// Effect declarations with full signatures: effect_name -> [(op_name, params, ret_ty)]
    effect_ops_detail: BTreeMap<String, Vec<(String, Vec<Param>, Option<Ty>)>>,
    /// Functions that use effects: fn_name -> list of effect names (from `with`)
    fn_effects: BTreeMap<String, Vec<String>>,
    /// Rule function parameters that need .clone() at each use site (non-Copy types)
    rule_clone_params: BTreeSet<String>,
    /// Prolog-style rule functions: fn_name -> param types (e.g., ["&str", "&str"])
    prolog_rule_fns: BTreeMap<String, Vec<String>>,
    /// Value-returning Prolog rule functions: fn_name -> return type (e.g., "String")
    prolog_value_fns: BTreeMap<String, String>,
    /// Simple literal = bindings at top level: name -> (rust_literal, rust_type)
    literal_bindings: BTreeMap<String, (String, String)>,
    /// M26: Types with `@ store` annotation
    stored_types: BTreeSet<String>,
    /// M26: For stored types, the name of the first field (primary key)
    stored_type_key_field: BTreeMap<String, String>,
    /// M26: Store scope — determines DB filename
    store_scope: Option<String>,
    /// M26: Types with `delete_on_change` strategy
    store_delete_on_change: BTreeSet<String>,
    /// M26: Schema hash per stored type
    stored_type_schema_hash: BTreeMap<String, String>,
    /// User-defined function names (avoid overriding with builtins)
    user_functions: BTreeSet<String>,
    /// Names marked with `@ export` — emitted as `pub` in Rust
    exported_names: BTreeSet<String>,
    /// Module names from imports and inline modules
    known_modules: BTreeSet<String>,
    /// Comptime-evaluated values: variable name -> Rust literal string
    comptime_values: BTreeMap<String, String>,
    /// Comptime Rust type strings: variable name -> Rust type
    comptime_types: BTreeMap<String, String>,
    /// Functions with inout parameters: fn_name -> vec of is_inout per param
    inout_params: BTreeMap<String, Vec<bool>>,
    /// Functions with inout+shared (copy-on-write) parameters
    cow_params: BTreeMap<String, Vec<bool>>,
}

impl TypeRegistry {
    fn new() -> Self {
        TypeRegistry {
            type_decls: BTreeMap::new(),
            variant_parent: BTreeMap::new(),
            type_rename: BTreeMap::new(),
            variant_boxed_args: BTreeMap::new(),
            variant_positional: BTreeMap::new(),
            variant_fields: BTreeMap::new(),
            variant_field_types: BTreeMap::new(),
            explicit_display_impls: BTreeSet::new(),
            struct_types: BTreeSet::new(),
            rc_types: BTreeSet::new(),
            effect_ops: BTreeMap::new(),
            effect_ops_detail: BTreeMap::new(),
            fn_effects: BTreeMap::new(),
            rule_clone_params: BTreeSet::new(),
            prolog_rule_fns: BTreeMap::new(),
            prolog_value_fns: BTreeMap::new(),
            literal_bindings: BTreeMap::new(),
            stored_types: BTreeSet::new(),
            stored_type_key_field: BTreeMap::new(),
            store_scope: None,
            store_delete_on_change: BTreeSet::new(),
            stored_type_schema_hash: BTreeMap::new(),
            user_functions: BTreeSet::new(),
            exported_names: BTreeSet::new(),
            known_modules: BTreeSet::new(),
            comptime_values: BTreeMap::new(),
            comptime_types: BTreeMap::new(),
            inout_params: BTreeMap::new(),
            cow_params: BTreeMap::new(),
        }
    }
}

struct RustCodegen {
    indent: usize,
    /// Shared type metadata
    types: TypeRegistry,
    /// Escape analysis: for each variable in current function, total use count
    /// Key = variable name, Value = total use count in function body
    var_use_counts: BTreeMap<String, usize>,
    /// Escape analysis: consuming uses only (function args, constructor args — not show/builtins)
    var_consuming_counts: BTreeMap<String, usize>,
    /// Variables known to be Copy types in current scope (i64, f64, bool, char, u64)
    copy_vars: BTreeSet<String>,
    /// Variables that need `let mut` (rebound inside for loops)
    mutable_vars: BTreeSet<String>,
    /// Library mode: emit no fn main(), exported names get pub
    lib_mode: bool,
    /// WASM mode: emit wasm-bindgen annotations on exported functions
    wasm_mode: bool,
    /// Cargo dependencies: crate_name -> version
    cargo_deps: BTreeMap<String, String>,
    /// Source file directory (for resolving @ import paths)
    source_dir: Option<String>,
    /// Already-imported files (prevent cycles)
    imported: BTreeSet<String>,
    /// Auto-borrow: functions whose params are borrow-only (never consumed in body)
    /// fn_name -> vec of bools (true = param is borrow-only, emit &T)
    borrow_only_params: BTreeMap<String, Vec<bool>>,
    /// Aliased variables: var_name -> true if assigned from another variable
    /// (e.g. `= y = x` means y aliases x — both need consideration for ownership)
    aliased_vars: BTreeSet<String>,
    /// Pattern bindings from ref-matched params (match on &T) — need * deref in expressions
    ref_match_bindings: BTreeSet<String>,
    /// Borrowed params of current function being emitted — already &T, don't double-borrow
    current_borrow_params: BTreeSet<String>,
    /// Variables known to be String-typed in current scope (for string concat detection)
    string_typed_vars: BTreeSet<String>,
    /// Variables known to be Float-typed in current scope (for float division/fold detection)
    float_typed_vars: BTreeSet<String>,
    /// Functions known to return String (for string concat format! emission)
    string_returning_fns: BTreeSet<String>,
    /// Temporary flag: emit FnOnce instead of FnMut for arrow types
    fn_once_mode: bool,
    /// True when emitting a method body where self is &self — skip boxed unboxing
    in_self_method: bool,
    /// Effects of the function currently being emitted (for routing op calls to handler params)
    current_effects: Vec<String>,
    /// Effects provided by `| handle` blocks (concrete struct types, need `&mut`)
    /// vs effects from function params (already `&mut impl E`, just reborrow)
    handle_scope_effects: BTreeSet<String>,
    /// Inferred types for bindings (var_name -> Rust type string)
    /// Used by handler capture to know field types.
    var_types: BTreeMap<String, String>,
    /// M13c: program needs async tokio runtime (subjects or actors detected)
    has_async: bool,
    /// M13c: variables that are subjects (broadcast::Sender) — for codegen routing
    subject_vars: BTreeSet<String>,
    /// M13c: scope name -> list of subscription JoinHandle variable names
    scope_handles: BTreeMap<String, Vec<String>>,
    /// M13c: current scope being emitted (for registering subscription handles)
    current_scope: Option<String>,
    /// M13c: counter for generating unique subscription handle names
    sub_counter: usize,
    /// M13c: scope name -> list of binding names (for qualified access ScopeName.field → field)
    scope_bindings: BTreeMap<String, Vec<String>>,
    /// Stored invariants for ? verification: name -> (subject_expr, predicate_expr)
    codegen_invariants: BTreeMap<String, (Expr, Expr)>,
    /// Actor handle variables: var_name -> actor_name (for qualifying __Ask in ask())
    actor_handle_vars: BTreeMap<String, String>,
    /// Sync subject variables: subject vars in non-async mode (Vec-based)
    sync_subject_vars: BTreeSet<String>,
    /// Inferred element types for broadcast subjects: name -> Rust type string
    subject_elem_type: BTreeMap<String, String>,
    /// Builtin registry: name -> definition (template, arity, deps, purity)
    builtin_registry: BTreeMap<String, BuiltinDef>,
    /// Counter for generating unique async stream operator variable names
    async_stream_counter: usize,
    /// Source file stem (e.g. "weather" from "weather.runa") — used for store DB naming
    source_name: Option<String>,
}

/// Per-function ownership analysis results.
/// Computed once per function body, consumed during Rust emission to decide
/// clone/move/borrow for each variable.
struct OwnershipAnalysis {
    /// Total use count per variable (for single-use → move optimization)
    var_uses: BTreeMap<String, usize>,
    /// Consuming use count per variable (args to non-borrow functions)
    consuming_uses: BTreeMap<String, usize>,
}

impl OwnershipAnalysis {
    /// Analyze a function body for ownership decisions.
    /// `borrow_fns` maps function names to which params are borrow-only.
    /// `self_fn_name` + `self_param_names` enable self-recursive passthrough detection.
    fn analyze(
        body: &Expr,
        borrow_fns: &BTreeMap<String, Vec<bool>>,
        self_fn_name: Option<&str>,
        self_param_names: &[&str],
    ) -> Self {
        let mut var_uses = BTreeMap::new();
        let mut consuming_uses = BTreeMap::new();
        count_var_uses(body, &mut var_uses);
        count_consuming_uses_borrow_aware(
            body,
            &mut consuming_uses,
            borrow_fns,
            self_fn_name,
            self_param_names,
        );
        OwnershipAnalysis {
            var_uses,
            consuming_uses,
        }
    }

    /// Simple analysis without borrow-awareness (for rule bodies, etc.)
    fn analyze_simple(body: &Expr) -> Self {
        let mut var_uses = BTreeMap::new();
        let mut consuming_uses = BTreeMap::new();
        count_var_uses(body, &mut var_uses);
        count_consuming_uses(body, &mut consuming_uses);
        OwnershipAnalysis {
            var_uses,
            consuming_uses,
        }
    }

    /// Analyze from statement references (for top-level code).
    fn analyze_stmt_refs(stmts: &[&Stmt], borrow_fns: &BTreeMap<String, Vec<bool>>) -> Self {
        let mut var_uses = BTreeMap::new();
        let mut consuming_uses = BTreeMap::new();
        for stmt in stmts {
            count_var_uses_stmt(stmt, &mut var_uses);
            count_consuming_uses_borrow_aware_stmt(
                stmt,
                &mut consuming_uses,
                borrow_fns,
                None,
                &[],
            );
        }
        OwnershipAnalysis {
            var_uses,
            consuming_uses,
        }
    }
}

// ============================================================================
// FIR: Futuruna Intermediate Representation
// ============================================================================
//
// FIR sits between the AST and Rust emission. Each node carries:
// - Resolved type (FirTy) — what Rust type to emit
// - Ownership mode (on Var nodes) — move, clone, borrow, or copy
// - Span — source location from the AST
//
// FIR is produced by lowering the AST using TypeRegistry + OwnershipAnalysis.
// Rust emission walks FIR without needing mutable state for type/ownership decisions.

/// How a variable reference should be handled in Rust emission.
#[derive(Debug, Clone, Copy, PartialEq)]
enum VarMode {
    /// Single use of a non-Copy value — transfer ownership (bare name)
    Move,
    /// Multiple uses of a non-Copy value — emit `.clone()`
    Clone,
    /// Read-only access — emit `&`
    Borrow,
    /// Copy type (i64, f64, bool, char) — free to duplicate
    Copy,
    /// Ref-match binding — emit `(*name)` for deref from &T pattern match
    Deref,
    /// Rule clone param — always `.clone()` regardless of use count
    RuleClone,
}

/// Resolved type for FIR — what the Rust emitter needs to know.
/// Not a full type system — just enough for emission decisions.
#[derive(Debug, Clone, PartialEq)]
enum FirTy {
    Int,
    Float,
    Bool,
    Char,
    String,
    Unit,
    List(Box<FirTy>),
    Option(Box<FirTy>),
    Result(Box<FirTy>, Box<FirTy>),
    Tuple(Vec<FirTy>),
    Map(Box<FirTy>, Box<FirTy>),
    Set(Box<FirTy>),
    /// User-defined type (ADT name, Rust-safe)
    Named(String),
    /// Function type
    Arrow(Box<FirTy>, Box<FirTy>),
    /// Type variable (for inference) — resolved by unification
    Var(usize),
    /// Not yet resolved (fallback)
    Unknown,
}

/// Type inference engine — union-find unification with occurs check.
struct TypeInference {
    /// Next fresh type variable ID
    next_var: usize,
    /// Union-find: var_id → resolved type (or Var pointing to parent)
    bindings: BTreeMap<usize, FirTy>,
}

impl TypeInference {
    fn new() -> Self {
        TypeInference {
            next_var: 0,
            bindings: BTreeMap::new(),
        }
    }

    /// Create a fresh type variable.
    fn fresh(&mut self) -> FirTy {
        let id = self.next_var;
        self.next_var += 1;
        FirTy::Var(id)
    }

    /// Find the root type for a type variable (path compression).
    fn find(&self, ty: &FirTy) -> FirTy {
        match ty {
            FirTy::Var(id) => {
                if let Some(bound) = self.bindings.get(id) {
                    self.find(bound)
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Resolve a type fully — substitute all type variables with their bindings.
    fn resolve(&self, ty: &FirTy) -> FirTy {
        match self.find(ty) {
            FirTy::Var(_) => FirTy::Unknown, // unresolved var → Unknown
            FirTy::List(inner) => FirTy::List(Box::new(self.resolve(&inner))),
            FirTy::Option(inner) => FirTy::Option(Box::new(self.resolve(&inner))),
            FirTy::Result(ok, err) => {
                FirTy::Result(Box::new(self.resolve(&ok)), Box::new(self.resolve(&err)))
            }
            FirTy::Tuple(elems) => FirTy::Tuple(elems.iter().map(|e| self.resolve(e)).collect()),
            FirTy::Map(k, v) => FirTy::Map(Box::new(self.resolve(&k)), Box::new(self.resolve(&v))),
            FirTy::Set(inner) => FirTy::Set(Box::new(self.resolve(&inner))),
            FirTy::Arrow(a, b) => {
                FirTy::Arrow(Box::new(self.resolve(&a)), Box::new(self.resolve(&b)))
            }
            other => other,
        }
    }

    /// Unify two types. Returns Ok(()) on success, Err(msg) on failure.
    fn unify(&mut self, a: &FirTy, b: &FirTy) -> Result<(), String> {
        let a = self.find(a);
        let b = self.find(b);

        if a == b {
            return Ok(());
        }

        match (&a, &b) {
            // Var binds to anything
            (FirTy::Var(id), other) | (other, FirTy::Var(id)) => {
                // Occurs check: don't bind a var to a type containing itself
                if self.occurs(*id, other) {
                    return Err(format!("infinite type: _t{} occurs in {:?}", id, other));
                }
                self.bindings.insert(*id, other.clone());
                Ok(())
            }
            // Unknown unifies with anything (acts like a wildcard)
            (FirTy::Unknown, _) | (_, FirTy::Unknown) => Ok(()),
            // Structural unification
            (FirTy::List(a), FirTy::List(b)) => self.unify(a, b),
            (FirTy::Option(a), FirTy::Option(b)) => self.unify(a, b),
            (FirTy::Set(a), FirTy::Set(b)) => self.unify(a, b),
            (FirTy::Result(a1, a2), FirTy::Result(b1, b2)) => {
                self.unify(a1, b1)?;
                self.unify(a2, b2)
            }
            (FirTy::Map(k1, v1), FirTy::Map(k2, v2)) => {
                self.unify(k1, k2)?;
                self.unify(v1, v2)
            }
            (FirTy::Arrow(a1, a2), FirTy::Arrow(b1, b2)) => {
                self.unify(a1, b1)?;
                self.unify(a2, b2)
            }
            (FirTy::Tuple(as_), FirTy::Tuple(bs)) if as_.len() == bs.len() => {
                for (a, b) in as_.iter().zip(bs.iter()) {
                    self.unify(a, b)?;
                }
                Ok(())
            }
            _ => Err(format!("cannot unify {:?} with {:?}", a, b)),
        }
    }

    /// Occurs check: does var_id appear anywhere in ty?
    fn occurs(&self, var_id: usize, ty: &FirTy) -> bool {
        match self.find(ty) {
            FirTy::Var(id) => id == var_id,
            FirTy::List(inner) | FirTy::Option(inner) | FirTy::Set(inner) => {
                self.occurs(var_id, &inner)
            }
            FirTy::Result(a, b) | FirTy::Map(a, b) | FirTy::Arrow(a, b) => {
                self.occurs(var_id, &a) || self.occurs(var_id, &b)
            }
            FirTy::Tuple(elems) => elems.iter().any(|e| self.occurs(var_id, e)),
            _ => false,
        }
    }

    /// Resolve all type variables in a FIR expression tree.
    fn substitute_expr(&self, expr: &mut FirExpr) {
        expr.ty = self.resolve(&expr.ty);
        match &mut expr.kind {
            FirExprKind::App(func, args) => {
                self.substitute_expr(func);
                for a in args {
                    self.substitute_expr(a);
                }
            }
            FirExprKind::BinOp(_, lhs, rhs) => {
                self.substitute_expr(lhs);
                self.substitute_expr(rhs);
            }
            FirExprKind::UnOp(_, inner) | FirExprKind::Try(inner) => {
                self.substitute_expr(inner);
            }
            FirExprKind::If(c, t, e) => {
                self.substitute_expr(c);
                self.substitute_expr(t);
                self.substitute_expr(e);
            }
            FirExprKind::Lambda(_, body) => self.substitute_expr(body),
            FirExprKind::Field(obj, _) => self.substitute_expr(obj),
            FirExprKind::Index(base, idx) => {
                self.substitute_expr(base);
                self.substitute_expr(idx);
            }
            FirExprKind::List(elems)
            | FirExprKind::Tuple(elems)
            | FirExprKind::Conjunction(elems) => {
                for e in elems {
                    self.substitute_expr(e);
                }
            }
            FirExprKind::Match(scrutinee, arms) => {
                self.substitute_expr(scrutinee);
                for arm in arms {
                    self.substitute_expr(&mut arm.body);
                    if let Some(g) = &mut arm.guard {
                        self.substitute_expr(g);
                    }
                }
            }
            FirExprKind::Pipe(lhs, rhs) => {
                self.substitute_expr(lhs);
                self.substitute_expr(rhs);
            }
            FirExprKind::Block(stmts) => {
                for s in stmts {
                    match s {
                        FirStmt::Expr(e)
                        | FirStmt::Bind(_, _, e)
                        | FirStmt::MonadicBind(_, _, e)
                        | FirStmt::StreamBind(_, e) => self.substitute_expr(e),
                        FirStmt::For(_, iter, body) => {
                            self.substitute_expr(iter);
                            // body stmts would need recursive handling
                        }
                        _ => {}
                    }
                }
            }
            FirExprKind::Effect(_, args) => {
                for a in args {
                    self.substitute_expr(a);
                }
            }
            FirExprKind::Handle { body, handlers, .. } => {
                self.substitute_expr(body);
                for h in handlers {
                    self.substitute_expr(&mut h.body);
                }
            }
            _ => {} // Var, Lit, Unit — no children
        }
    }

    /// Collect all unresolved type variables in a type.
    fn free_vars(&self, ty: &FirTy) -> BTreeSet<usize> {
        let mut vars = BTreeSet::new();
        self.collect_free_vars(ty, &mut vars);
        vars
    }

    fn collect_free_vars(&self, ty: &FirTy, vars: &mut BTreeSet<usize>) {
        match self.find(ty) {
            FirTy::Var(id) => {
                vars.insert(id);
            }
            FirTy::List(inner) | FirTy::Option(inner) | FirTy::Set(inner) => {
                self.collect_free_vars(&inner, vars)
            }
            FirTy::Result(a, b) | FirTy::Map(a, b) | FirTy::Arrow(a, b) => {
                self.collect_free_vars(&a, vars);
                self.collect_free_vars(&b, vars);
            }
            FirTy::Tuple(elems) => {
                for e in &elems {
                    self.collect_free_vars(e, vars);
                }
            }
            _ => {}
        }
    }

    /// Instantiate a type by replacing the given generic var IDs with fresh variables.
    fn instantiate(&mut self, ty: &FirTy, generics: &BTreeSet<usize>) -> FirTy {
        if generics.is_empty() {
            return ty.clone();
        }
        let mut mapping: BTreeMap<usize, FirTy> = BTreeMap::new();
        for &id in generics {
            mapping.insert(id, self.fresh());
        }
        self.apply_mapping(ty, &mapping)
    }

    fn apply_mapping(&self, ty: &FirTy, mapping: &BTreeMap<usize, FirTy>) -> FirTy {
        match self.find(ty) {
            FirTy::Var(id) => mapping.get(&id).cloned().unwrap_or(FirTy::Var(id)),
            FirTy::List(inner) => FirTy::List(Box::new(self.apply_mapping(&inner, mapping))),
            FirTy::Option(inner) => FirTy::Option(Box::new(self.apply_mapping(&inner, mapping))),
            FirTy::Set(inner) => FirTy::Set(Box::new(self.apply_mapping(&inner, mapping))),
            FirTy::Result(a, b) => FirTy::Result(
                Box::new(self.apply_mapping(&a, mapping)),
                Box::new(self.apply_mapping(&b, mapping)),
            ),
            FirTy::Map(k, v) => FirTy::Map(
                Box::new(self.apply_mapping(&k, mapping)),
                Box::new(self.apply_mapping(&v, mapping)),
            ),
            FirTy::Arrow(a, b) => FirTy::Arrow(
                Box::new(self.apply_mapping(&a, mapping)),
                Box::new(self.apply_mapping(&b, mapping)),
            ),
            FirTy::Tuple(elems) => FirTy::Tuple(
                elems
                    .iter()
                    .map(|e| self.apply_mapping(e, mapping))
                    .collect(),
            ),
            other => other,
        }
    }
}

/// A type scheme: a type with generic (universally quantified) variables.
/// Used for let-generalization of polymorphic functions.
#[derive(Debug, Clone)]
struct TypeScheme {
    /// The generic type variable IDs (universally quantified)
    generics: BTreeSet<usize>,
    /// The type (containing Var references to the generic IDs)
    ty: FirTy,
}

/// FIR expression — AST expression with ownership and type annotations.
#[derive(Debug, Clone)]
struct FirExpr {
    kind: FirExprKind,
    span: Span,
    ty: FirTy,
}

/// FIR expression kinds — mirrors ExprKind with ownership on Var nodes.
#[derive(Debug, Clone)]
enum FirExprKind {
    /// Variable reference with resolved ownership mode
    Var(String, VarMode),
    /// Literal value
    Lit(Literal),
    /// Function application
    App(Box<FirExpr>, Vec<FirExpr>),
    /// Lambda (params may have resolved types)
    Lambda(Vec<Param>, Box<FirExpr>),
    /// Binary operator
    BinOp(String, Box<FirExpr>, Box<FirExpr>),
    /// Unary operator
    UnOp(String, Box<FirExpr>),
    /// If-then-else
    If(Box<FirExpr>, Box<FirExpr>, Box<FirExpr>),
    /// Pattern match
    Match(Box<FirExpr>, Vec<FirMatchArm>),
    /// Statement block
    Block(Vec<FirStmt>),
    /// Field access
    Field(Box<FirExpr>, String),
    /// Index
    Index(Box<FirExpr>, Box<FirExpr>),
    /// List literal
    List(Vec<FirExpr>),
    /// Tuple literal
    Tuple(Vec<FirExpr>),
    /// Effect operation call
    Effect(String, Vec<FirExpr>),
    /// Effect handler
    Handle {
        effect: String,
        handlers: Vec<FirEffHandler>,
        body: Box<FirExpr>,
    },
    /// Try (? operator)
    Try(Box<FirExpr>),
    /// Conjunction (Prolog-style)
    Conjunction(Vec<FirExpr>),
    /// Pipe forward (preserved for stream identity)
    Pipe(Box<FirExpr>, Box<FirExpr>),
    /// Unit value
    Unit,
}

/// FIR match arm
#[derive(Debug, Clone)]
struct FirMatchArm {
    pat: Pat,
    guard: Option<FirExpr>,
    body: FirExpr,
}

/// FIR effect handler
#[derive(Debug, Clone)]
struct FirEffHandler {
    op_name: String,
    params: Vec<String>,
    body: FirExpr,
}

/// FIR statement — mirrors Stmt with FIR expressions.
#[derive(Debug, Clone)]
enum FirStmt {
    Defn(FirDefn),
    TypeDecl(TypeDecl),
    Rule(Rule),
    Use(String),
    Import(String),
    QualifiedImport(String, String),
    HashImport(String, String),
    Depend(String, String),
    RustBlock(String),
    Annot(String, Vec<FirExpr>),
    Bind(Pat, Option<Ty>, FirExpr),
    MonadicBind(Pat, Option<Ty>, FirExpr),
    For(String, FirExpr, Vec<FirStmt>),
    Send(FirExpr, FirExpr),
    StreamBind(String, FirExpr),
    StreamSub(FirExpr, Vec<FirMatchArm>),
    Invariant {
        name: String,
        subject: FirExpr,
        predicate: FirExpr,
    },
    Prove {
        name: String,
        capture: Option<String>,
        pass_block: Option<Vec<FirStmt>>,
        else_block: Option<Vec<FirStmt>>,
    },
    Assert(String, Vec<FirExpr>),
    Retract(String, Vec<FirExpr>),
    Abort,
    Expr(FirExpr),
}

/// FIR function definition
#[derive(Debug, Clone)]
enum FirDefn {
    Fn {
        name: String,
        params: Vec<Param>,
        ret_ty: Option<Ty>,
        effects: Vec<String>,
        body: FirExpr,
    },
    Actor {
        name: String,
        state_param: Param,
        handlers: Vec<FirHandler>,
    },
    Module {
        name: String,
        body: Vec<FirStmt>,
    },
}

/// FIR actor handler
#[derive(Debug, Clone)]
struct FirHandler {
    msg_pat: Pat,
    body: FirExpr,
}

/// The complete FIR program — ready for Rust emission.
#[derive(Debug, Clone)]
struct FirProgram {
    stmts: Vec<FirStmt>,
    types: TypeRegistry,
}

// ============================================================================
// AST → FIR LOWERING
// ============================================================================

/// Context for lowering AST to FIR.
/// Carries the analysis results needed to annotate FIR nodes.
struct LoweringCtx<'a> {
    types: &'a TypeRegistry,
    ownership: &'a OwnershipAnalysis,
    copy_vars: &'a BTreeSet<String>,
    ref_match_bindings: &'a BTreeSet<String>,
    /// Type environment: variable name → resolved FirTy
    type_env: BTreeMap<String, FirTy>,
    /// Type inference engine (optional — used when constraint solving is active)
    inference: Option<TypeInference>,
    /// Polymorphic function type schemes (from let-generalization)
    fn_schemes: BTreeMap<String, TypeScheme>,
}

impl<'a> LoweringCtx<'a> {
    /// Convert a Futuruna Ty to a FirTy.
    fn ty_to_fir(ty: &Ty) -> FirTy {
        match ty {
            Ty::Name(n) => match n.as_str() {
                "Int" => FirTy::Int,
                "Float" => FirTy::Float,
                "Bool" => FirTy::Bool,
                "Char" => FirTy::Char,
                "String" => FirTy::String,
                "Unit" | "()" => FirTy::Unit,
                other => FirTy::Named(other.to_string()),
            },
            Ty::App(base, args) => {
                if let Ty::Name(n) = base.as_ref() {
                    match n.as_str() {
                        "List" if args.len() == 1 => {
                            FirTy::List(Box::new(Self::ty_to_fir(&args[0])))
                        }
                        "Option" if args.len() == 1 => {
                            FirTy::Option(Box::new(Self::ty_to_fir(&args[0])))
                        }
                        "Result" if args.len() == 2 => FirTy::Result(
                            Box::new(Self::ty_to_fir(&args[0])),
                            Box::new(Self::ty_to_fir(&args[1])),
                        ),
                        "Map" if args.len() == 2 => FirTy::Map(
                            Box::new(Self::ty_to_fir(&args[0])),
                            Box::new(Self::ty_to_fir(&args[1])),
                        ),
                        "Set" if args.len() == 1 => FirTy::Set(Box::new(Self::ty_to_fir(&args[0]))),
                        _ => FirTy::Named(n.clone()),
                    }
                } else {
                    FirTy::Unknown
                }
            }
            Ty::Arrow(a, b) => {
                FirTy::Arrow(Box::new(Self::ty_to_fir(a)), Box::new(Self::ty_to_fir(b)))
            }
            Ty::Optional(inner) => FirTy::Option(Box::new(Self::ty_to_fir(inner))),
            Ty::Unit => FirTy::Unit,
            _ => FirTy::Unknown,
        }
    }

    /// Infer the type of a literal.
    fn literal_ty(lit: &Literal) -> FirTy {
        match lit {
            Literal::Int(_) => FirTy::Int,
            Literal::Float(_) => FirTy::Float,
            Literal::Str(_) => FirTy::String,
            Literal::Char(_) => FirTy::Char,
            Literal::Bool(_) => FirTy::Bool,
        }
    }

    /// Infer type of a binary operation from its operands.
    fn binop_ty(op: &str, lhs_ty: &FirTy, rhs_ty: &FirTy) -> FirTy {
        match op {
            // Comparison operators always return Bool
            "==" | "!=" | "<" | ">" | "<=" | ">=" => FirTy::Bool,
            // Logical operators return Bool
            "&&" | "||" => FirTy::Bool,
            // String concatenation
            "+" if matches!(lhs_ty, FirTy::String) || matches!(rhs_ty, FirTy::String) => {
                FirTy::String
            }
            // Arithmetic: prefer Float if either operand is Float
            "+" | "-" | "*" | "/" | "%" => {
                if matches!(lhs_ty, FirTy::Float) || matches!(rhs_ty, FirTy::Float) {
                    FirTy::Float
                } else {
                    FirTy::Int
                }
            }
            _ => FirTy::Unknown,
        }
    }

    /// Look up a variable's type from the environment.
    fn var_ty(&self, name: &str) -> FirTy {
        self.type_env.get(name).cloned().unwrap_or(FirTy::Unknown)
    }

    /// Infer types for a function with possibly unannotated parameters.
    /// Creates type variables for missing annotations, lowers the body,
    /// generates constraints from usage, solves, and substitutes.
    fn infer_function(
        &mut self,
        params: &[Param],
        body: &Expr,
        ret_ty: Option<&Ty>,
        fn_name: Option<&str>,
    ) -> FirExpr {
        let mut inf = TypeInference::new();

        // Create type vars for unannotated params, concrete types for annotated ones
        for p in params {
            let ty = match &p.ty {
                Some(ty) => Self::ty_to_fir(ty),
                None => inf.fresh(),
            };
            self.type_env.insert(p.name.clone(), ty);
        }

        // Store inference engine and lower the body
        self.inference = Some(inf);
        let mut fir_body = self.lower_expr(body);

        // If return type is declared, unify body type with it
        if let Some(rt) = ret_ty {
            let declared_ret = Self::ty_to_fir(rt);
            if let Some(ref mut inf) = self.inference {
                let _ = inf.unify(&fir_body.ty, &declared_ret);
            }
        }

        // Before substitution: build the function type and check for generics
        if let Some(ref inf) = self.inference {
            // Build Arrow type from current (possibly unresolved) param types
            let param_tys: Vec<FirTy> = params
                .iter()
                .map(|p| {
                    self.type_env
                        .get(&p.name)
                        .cloned()
                        .unwrap_or(FirTy::Unknown)
                })
                .collect();
            let mut fn_ty = fir_body.ty.clone();
            for pt in param_tys.into_iter().rev() {
                fn_ty = FirTy::Arrow(Box::new(pt), Box::new(fn_ty));
            }

            // Collect free (unresolved) type vars — these become generics
            let free = inf.free_vars(&fn_ty);
            if !free.is_empty() {
                self.fn_schemes.insert(
                    fn_name.unwrap_or("_").to_string(),
                    TypeScheme {
                        generics: free,
                        ty: fn_ty.clone(),
                    },
                );
            } else {
                self.type_env
                    .insert(fn_name.unwrap_or("_").to_string(), fn_ty);
            }

            // Now substitute resolved vars
            inf.substitute_expr(&mut fir_body);
            let resolved_env: BTreeMap<String, FirTy> = self
                .type_env
                .iter()
                .map(|(k, v)| (k.clone(), inf.resolve(v)))
                .collect();
            self.type_env = resolved_env;
        }

        self.inference = None;
        fir_body
    }

    /// Determine the VarMode for a variable reference.
    fn var_mode(&self, name: &str) -> VarMode {
        // Ref-match binding: dereference
        if self.ref_match_bindings.contains(name) {
            return VarMode::Deref;
        }
        // Rule clone param: always clone
        if self.types.rule_clone_params.contains(name) {
            return VarMode::RuleClone;
        }
        // Copy type: free to duplicate
        if self.copy_vars.contains(name) {
            return VarMode::Copy;
        }
        // Multi-use non-Copy: clone
        if self
            .ownership
            .consuming_uses
            .get(name)
            .copied()
            .unwrap_or(0)
            > 1
        {
            return VarMode::Clone;
        }
        // Single use: move
        VarMode::Move
    }

    /// Lower an AST expression to FIR with type resolution.
    fn lower_expr(&mut self, expr: &Expr) -> FirExpr {
        match &expr.kind {
            ExprKind::Var(name) => {
                let ty = self.var_ty(name);
                if self.types.variant_parent.contains_key(name.as_str()) {
                    FirExpr {
                        kind: FirExprKind::Var(name.clone(), VarMode::Move),
                        span: expr.span,
                        ty,
                    }
                } else {
                    FirExpr {
                        kind: FirExprKind::Var(name.clone(), self.var_mode(name)),
                        span: expr.span,
                        ty,
                    }
                }
            }
            ExprKind::Lit(lit) => {
                let ty = Self::literal_ty(lit);
                FirExpr {
                    kind: FirExprKind::Lit(lit.clone()),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::App(func, args) => {
                let fir_func = self.lower_expr(func);
                let fir_args: Vec<FirExpr> = args.iter().map(|a| self.lower_expr(a)).collect();

                // Check if the function has a polymorphic type scheme → instantiate
                let func_ty = if let ExprKind::Var(ref fn_name) = func.kind {
                    if let Some(scheme) = self.fn_schemes.get(fn_name).cloned() {
                        if let Some(ref mut inf) = self.inference {
                            // Instantiate with fresh type vars
                            let inst = inf.instantiate(&scheme.ty, &scheme.generics);
                            // Unify arg types with param types
                            let mut current = &inst;
                            for arg in &fir_args {
                                if let FirTy::Arrow(param_ty, ret) = current {
                                    let _ = inf.unify(&arg.ty, param_ty);
                                    current = ret;
                                }
                            }
                            Some(inst)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Infer return type from function's type (if Arrow)
                let effective_ty = func_ty.as_ref().unwrap_or(&fir_func.ty);
                let ty = match effective_ty {
                    FirTy::Arrow(_, ret) => {
                        // Walk through arrows to get the return type after all args applied
                        let mut current = effective_ty;
                        for _ in &fir_args {
                            if let FirTy::Arrow(_, ret) = current {
                                current = ret;
                            } else {
                                break;
                            }
                        }
                        current.clone()
                    }
                    _ => FirTy::Unknown,
                };
                FirExpr {
                    kind: FirExprKind::App(Box::new(fir_func), fir_args),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Lambda(params, body) => {
                let fir_body = self.lower_expr(body);
                let body_ty = fir_body.ty.clone();
                FirExpr {
                    kind: FirExprKind::Lambda(params.clone(), Box::new(fir_body)),
                    span: expr.span,
                    ty: body_ty,
                }
            }
            ExprKind::BinOp(op, lhs, rhs) => {
                let fir_lhs = self.lower_expr(lhs);
                let fir_rhs = self.lower_expr(rhs);
                let ty = Self::binop_ty(op, &fir_lhs.ty, &fir_rhs.ty);
                // Generate constraints: for arithmetic ops, operands should match result type
                if let Some(ref mut inf) = self.inference {
                    match op.as_str() {
                        "+" | "-" | "*" | "/" | "%" => {
                            let _ = inf.unify(&fir_lhs.ty, &ty);
                            let _ = inf.unify(&fir_rhs.ty, &ty);
                        }
                        "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                            // Operands should have the same type
                            let _ = inf.unify(&fir_lhs.ty, &fir_rhs.ty);
                        }
                        "&&" | "||" => {
                            let _ = inf.unify(&fir_lhs.ty, &FirTy::Bool);
                            let _ = inf.unify(&fir_rhs.ty, &FirTy::Bool);
                        }
                        _ => {}
                    }
                }
                FirExpr {
                    kind: FirExprKind::BinOp(op.clone(), Box::new(fir_lhs), Box::new(fir_rhs)),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::UnOp(op, inner) => {
                let fir_inner = self.lower_expr(inner);
                let ty = if op == "!" {
                    FirTy::Bool
                } else {
                    fir_inner.ty.clone()
                };
                FirExpr {
                    kind: FirExprKind::UnOp(op.clone(), Box::new(fir_inner)),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::If(cond, then_, else_) => {
                let fir_cond = self.lower_expr(cond);
                let fir_then = self.lower_expr(then_);
                let fir_else = self.lower_expr(else_);
                let ty = fir_then.ty.clone(); // if/else branches should have same type
                FirExpr {
                    kind: FirExprKind::If(
                        Box::new(fir_cond),
                        Box::new(fir_then),
                        Box::new(fir_else),
                    ),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Match(scrutinee, arms) => {
                let fir_scrutinee = self.lower_expr(scrutinee);
                let fir_arms: Vec<FirMatchArm> = arms
                    .iter()
                    .map(|a| FirMatchArm {
                        pat: a.pat.clone(),
                        guard: a.guard.as_ref().map(|g| self.lower_expr(g)),
                        body: self.lower_expr(&a.body),
                    })
                    .collect();
                let ty = fir_arms
                    .first()
                    .map(|a| a.body.ty.clone())
                    .unwrap_or(FirTy::Unknown);
                FirExpr {
                    kind: FirExprKind::Match(Box::new(fir_scrutinee), fir_arms),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Block(stmts) => {
                let fir_stmts: Vec<FirStmt> = stmts.iter().map(|s| self.lower_stmt(s)).collect();
                // Block type = type of last expression statement
                let ty = fir_stmts
                    .last()
                    .and_then(|s| match s {
                        FirStmt::Expr(e) => Some(e.ty.clone()),
                        _ => None,
                    })
                    .unwrap_or(FirTy::Unit);
                FirExpr {
                    kind: FirExprKind::Block(fir_stmts),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Field(obj, field) => {
                let fir_obj = self.lower_expr(obj);
                FirExpr {
                    kind: FirExprKind::Field(Box::new(fir_obj), field.clone()),
                    span: expr.span,
                    ty: FirTy::Unknown,
                }
            }
            ExprKind::Index(base, idx) => {
                let fir_base = self.lower_expr(base);
                let ty = match &fir_base.ty {
                    FirTy::List(elem) => *elem.clone(),
                    _ => FirTy::Unknown,
                };
                FirExpr {
                    kind: FirExprKind::Index(Box::new(fir_base), Box::new(self.lower_expr(idx))),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::List(elems) => {
                let fir_elems: Vec<FirExpr> = elems.iter().map(|e| self.lower_expr(e)).collect();
                let elem_ty = fir_elems
                    .first()
                    .map(|e| e.ty.clone())
                    .unwrap_or(FirTy::Unknown);
                FirExpr {
                    kind: FirExprKind::List(fir_elems),
                    span: expr.span,
                    ty: FirTy::List(Box::new(elem_ty)),
                }
            }
            ExprKind::Tuple(elems) => {
                let fir_elems: Vec<FirExpr> = elems.iter().map(|e| self.lower_expr(e)).collect();
                let tys: Vec<FirTy> = fir_elems.iter().map(|e| e.ty.clone()).collect();
                FirExpr {
                    kind: FirExprKind::Tuple(fir_elems),
                    span: expr.span,
                    ty: FirTy::Tuple(tys),
                }
            }
            ExprKind::Effect(name, args) => {
                let fir_args: Vec<FirExpr> = args.iter().map(|a| self.lower_expr(a)).collect();
                FirExpr {
                    kind: FirExprKind::Effect(name.clone(), fir_args),
                    span: expr.span,
                    ty: FirTy::Unit,
                }
            }
            ExprKind::Handle {
                effect,
                handlers,
                body,
            } => {
                let fir_body = self.lower_expr(body);
                let ty = fir_body.ty.clone();
                FirExpr {
                    kind: FirExprKind::Handle {
                        effect: effect.clone(),
                        handlers: handlers
                            .iter()
                            .map(|h| FirEffHandler {
                                op_name: h.op_name.clone(),
                                params: h.params.clone(),
                                body: self.lower_expr(&h.body),
                            })
                            .collect(),
                        body: Box::new(fir_body),
                    },
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Try(inner) => {
                let fir_inner = self.lower_expr(inner);
                // Try unwraps Result/Option → inner type
                let ty = match &fir_inner.ty {
                    FirTy::Option(inner) => *inner.clone(),
                    FirTy::Result(ok, _) => *ok.clone(),
                    other => other.clone(),
                };
                FirExpr {
                    kind: FirExprKind::Try(Box::new(fir_inner)),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Conjunction(goals) => {
                let fir_goals: Vec<FirExpr> = goals.iter().map(|g| self.lower_expr(g)).collect();
                FirExpr {
                    kind: FirExprKind::Conjunction(fir_goals),
                    span: expr.span,
                    ty: FirTy::Bool,
                }
            }
            ExprKind::Pipe(lhs, rhs) => {
                let fir_lhs = self.lower_expr(lhs);
                let fir_rhs = self.lower_expr(rhs);
                let ty = fir_rhs.ty.clone(); // pipe result type = transform result type
                FirExpr {
                    kind: FirExprKind::Pipe(Box::new(fir_lhs), Box::new(fir_rhs)),
                    span: expr.span,
                    ty,
                }
            }
            ExprKind::Unit => FirExpr {
                kind: FirExprKind::Unit,
                span: expr.span,
                ty: FirTy::Unit,
            },
        }
    }

    /// Lower an AST statement to FIR.
    fn lower_stmt(&mut self, stmt: &Stmt) -> FirStmt {
        match stmt {
            Stmt::Defn(Defn::Fn {
                name,
                params,
                ret_ty,
                effects,
                body,
            }) => FirStmt::Defn(FirDefn::Fn {
                name: name.clone(),
                params: params.clone(),
                ret_ty: ret_ty.clone(),
                effects: effects.clone(),
                body: self.lower_expr(body),
            }),
            Stmt::Defn(Defn::Actor {
                name,
                state_param,
                handlers,
            }) => FirStmt::Defn(FirDefn::Actor {
                name: name.clone(),
                state_param: state_param.clone(),
                handlers: handlers
                    .iter()
                    .map(|h| FirHandler {
                        msg_pat: h.msg_pat.clone(),
                        body: self.lower_expr(&h.body),
                    })
                    .collect(),
            }),
            Stmt::Defn(Defn::Module { name, body }) => FirStmt::Defn(FirDefn::Module {
                name: name.clone(),
                body: body.iter().map(|s| self.lower_stmt(s)).collect(),
            }),
            Stmt::TypeDecl(td) => FirStmt::TypeDecl(td.clone()),
            Stmt::Rule(r) => FirStmt::Rule(r.clone()),
            Stmt::Use(s) => FirStmt::Use(s.clone()),
            Stmt::Import(s) => FirStmt::Import(s.clone()),
            Stmt::QualifiedImport(a, b) => FirStmt::QualifiedImport(a.clone(), b.clone()),
            Stmt::HashImport(a, b) => FirStmt::HashImport(a.clone(), b.clone()),
            Stmt::Depend(a, b) => FirStmt::Depend(a.clone(), b.clone()),
            Stmt::RustBlock(s) => FirStmt::RustBlock(s.clone()),
            Stmt::Annot(name, args) => FirStmt::Annot(
                name.clone(),
                args.iter().map(|a| self.lower_expr(a)).collect(),
            ),
            Stmt::Bind(pat, ty, expr) => {
                FirStmt::Bind(pat.clone(), ty.clone(), self.lower_expr(expr))
            }
            Stmt::MonadicBind(pat, ty, expr) => {
                FirStmt::MonadicBind(pat.clone(), ty.clone(), self.lower_expr(expr))
            }
            Stmt::For(var, iter_expr, body) => FirStmt::For(
                var.clone(),
                self.lower_expr(iter_expr),
                body.iter().map(|s| self.lower_stmt(s)).collect(),
            ),
            Stmt::Send(target, msg) => FirStmt::Send(self.lower_expr(target), self.lower_expr(msg)),
            Stmt::StreamBind(name, expr) => {
                FirStmt::StreamBind(name.clone(), self.lower_expr(expr))
            }
            Stmt::StreamSub(expr, arms) => FirStmt::StreamSub(
                self.lower_expr(expr),
                arms.iter()
                    .map(|a| FirMatchArm {
                        pat: a.pat.clone(),
                        guard: a.guard.as_ref().map(|g| self.lower_expr(g)),
                        body: self.lower_expr(&a.body),
                    })
                    .collect(),
            ),
            Stmt::Invariant {
                name,
                subject,
                predicate,
            } => FirStmt::Invariant {
                name: name.clone(),
                subject: self.lower_expr(subject),
                predicate: self.lower_expr(predicate),
            },
            Stmt::Prove {
                name,
                capture,
                pass_block,
                else_block,
            } => FirStmt::Prove {
                name: name.clone(),
                capture: capture.clone(),
                pass_block: pass_block
                    .as_ref()
                    .map(|b| b.iter().map(|s| self.lower_stmt(s)).collect()),
                else_block: else_block
                    .as_ref()
                    .map(|b| b.iter().map(|s| self.lower_stmt(s)).collect()),
            },
            Stmt::Assert(name, args) => FirStmt::Assert(
                name.clone(),
                args.iter().map(|a| self.lower_expr(a)).collect(),
            ),
            Stmt::Retract(name, args) => FirStmt::Retract(
                name.clone(),
                args.iter().map(|a| self.lower_expr(a)).collect(),
            ),
            Stmt::Abort => FirStmt::Abort,
            Stmt::Expr(expr) => FirStmt::Expr(self.lower_expr(expr)),
        }
    }
}

// ============================================================================
// FIR → RUST EMISSION
// ============================================================================
//
// Stateless walk of FIR nodes → Rust source string.
// All ownership/type decisions are pre-computed in FIR — no mutable state needed
// beyond indentation. This replaces the decision-making parts of emit_expr.

/// Emit a FIR expression as Rust source code.
/// This is the core of the new emission pipeline. It reads VarMode from FIR
/// nodes instead of computing it from analysis state.
fn emit_fir_expr(expr: &FirExpr, types: &TypeRegistry) -> String {
    match &expr.kind {
        FirExprKind::Var(name, mode) => {
            // Check if it's a nullary constructor
            if let Some(parent) = types.variant_parent.get(name.as_str()) {
                if types.struct_types.contains(parent) {
                    return name.clone();
                }
                return format!("{}::{}", parent, name);
            }
            let sname = sanitize_name(name);
            match mode {
                VarMode::Deref => format!("(*{})", sname),
                VarMode::RuleClone | VarMode::Clone => format!("{}.clone()", sname),
                VarMode::Borrow => format!("&{}", sname),
                VarMode::Copy | VarMode::Move => sname,
            }
        }
        FirExprKind::Lit(Literal::Str(s)) => format!("{:?}.to_string()", s),
        FirExprKind::Lit(Literal::Int(n)) => format!("{}i64", n),
        FirExprKind::Lit(Literal::Float(f)) => {
            let s = format!("{}", f);
            if s.contains('.') {
                s
            } else {
                format!("{}.0", s)
            }
        }
        FirExprKind::Lit(Literal::Char(c)) => format!("'{}'", c),
        FirExprKind::Lit(Literal::Bool(b)) => format!("{}", b),
        FirExprKind::BinOp(op, lhs, rhs) => {
            let l = emit_fir_expr(lhs, types);
            let r = emit_fir_expr(rhs, types);
            // String concatenation
            if op == "+" {
                match (&lhs.ty, &rhs.ty) {
                    (FirTy::String, _) | (_, FirTy::String) => {
                        return format!("format!(\"{{}}{{}}\", {}, {})", l, r);
                    }
                    _ => {}
                }
            }
            let rust_op = if op == "==" {
                "=="
            } else if op == "!=" {
                "!="
            } else if op == "&&" {
                "&&"
            } else if op == "||" {
                "||"
            } else {
                op.as_str()
            };
            format!("({} {} {})", l, rust_op, r)
        }
        FirExprKind::UnOp(op, inner) => {
            format!("({}{})", op, emit_fir_expr(inner, types))
        }
        FirExprKind::If(cond, then_, else_) => {
            format!(
                "if {} {{ {} }} else {{ {} }}",
                emit_fir_expr(cond, types),
                emit_fir_expr(then_, types),
                emit_fir_expr(else_, types)
            )
        }
        FirExprKind::App(func, args) => {
            let f = emit_fir_expr(func, types);
            let arg_strs: Vec<String> = args.iter().map(|a| emit_fir_expr(a, types)).collect();
            format!("{}({})", f, arg_strs.join(", "))
        }
        FirExprKind::Field(obj, field) => {
            format!("{}.{}", emit_fir_expr(obj, types), field)
        }
        FirExprKind::Index(base, idx) => {
            format!(
                "{}[{} as usize]",
                emit_fir_expr(base, types),
                emit_fir_expr(idx, types)
            )
        }
        FirExprKind::List(elems) => {
            let items: Vec<String> = elems.iter().map(|e| emit_fir_expr(e, types)).collect();
            format!("vec![{}]", items.join(", "))
        }
        FirExprKind::Tuple(elems) => {
            let items: Vec<String> = elems.iter().map(|e| emit_fir_expr(e, types)).collect();
            format!("({})", items.join(", "))
        }
        FirExprKind::Lambda(params, body) => {
            let param_strs: Vec<String> = params.iter().map(|p| sanitize_name(&p.name)).collect();
            format!(
                "|{}| {{ {} }}",
                param_strs.join(", "),
                emit_fir_expr(body, types)
            )
        }
        FirExprKind::Try(inner) => {
            format!("{}?", emit_fir_expr(inner, types))
        }
        FirExprKind::Unit => "()".to_string(),
        FirExprKind::Match(scrutinee, arms) => {
            let mut out = format!("match {} {{\n", emit_fir_expr(scrutinee, types));
            for arm in arms {
                let pat_str = format_pat(&arm.pat);
                if let Some(ref guard) = arm.guard {
                    out.push_str(&format!(
                        "    {} if {} => {},\n",
                        pat_str,
                        emit_fir_expr(guard, types),
                        emit_fir_expr(&arm.body, types)
                    ));
                } else {
                    out.push_str(&format!(
                        "    {} => {},\n",
                        pat_str,
                        emit_fir_expr(&arm.body, types)
                    ));
                }
            }
            out.push_str("}");
            out
        }
        FirExprKind::Pipe(lhs, rhs) => {
            // Pipe desugars: a |> f → f(a), a |> f(y) → f(a, y)
            match &rhs.kind {
                FirExprKind::App(func, existing_args) => {
                    let mut new_args = vec![emit_fir_expr(lhs, types)];
                    new_args.extend(existing_args.iter().map(|a| emit_fir_expr(a, types)));
                    format!("{}({})", emit_fir_expr(func, types), new_args.join(", "))
                }
                _ => {
                    format!(
                        "{}({})",
                        emit_fir_expr(rhs, types),
                        emit_fir_expr(lhs, types)
                    )
                }
            }
        }
        // These require more context than the simple emitter provides — placeholder
        FirExprKind::Effect(name, args) => {
            let arg_strs: Vec<String> = args.iter().map(|a| emit_fir_expr(a, types)).collect();
            format!("/* effect {} */ {}({})", name, name, arg_strs.join(", "))
        }
        FirExprKind::Handle { effect, body, .. } => {
            format!("/* handle {} */ {}", effect, emit_fir_expr(body, types))
        }
        FirExprKind::Block(stmts) => {
            let parts: Vec<String> = stmts.iter().map(|s| emit_fir_stmt(s, types)).collect();
            format!("{{ {} }}", parts.join(" "))
        }
        FirExprKind::Conjunction(goals) => {
            let parts: Vec<String> = goals.iter().map(|g| emit_fir_expr(g, types)).collect();
            parts.join(" && ")
        }
    }
}

/// Emit a FIR statement as Rust source code.
fn emit_fir_stmt(stmt: &FirStmt, types: &TypeRegistry) -> String {
    match stmt {
        FirStmt::Bind(Pat::Var(name), _, expr) => {
            format!(
                "let {} = {};",
                sanitize_name(name),
                emit_fir_expr(expr, types)
            )
        }
        FirStmt::Bind(pat, _, expr) => {
            format!("let {} = {};", format_pat(pat), emit_fir_expr(expr, types))
        }
        FirStmt::Expr(expr) => {
            format!("{};", emit_fir_expr(expr, types))
        }
        FirStmt::For(var, iter_expr, body) => {
            let body_strs: Vec<String> = body.iter().map(|s| emit_fir_stmt(s, types)).collect();
            format!(
                "for {} in {} {{ {} }}",
                sanitize_name(var),
                emit_fir_expr(iter_expr, types),
                body_strs.join(" ")
            )
        }
        _ => "/* unhandled FIR stmt */".to_string(),
    }
}

/// Format a pattern for Rust output (simple version for FIR emission).
fn format_pat(pat: &Pat) -> String {
    match pat {
        Pat::Wild => "_".to_string(),
        Pat::Var(name) => sanitize_name(name),
        Pat::Lit(lit) => match lit {
            Literal::Int(n) => format!("{}", n),
            Literal::Float(f) => format!("{}", f),
            Literal::Str(s) => format!("{:?}", s),
            Literal::Char(c) => format!("'{}'", c),
            Literal::Bool(b) => format!("{}", b),
        },
        Pat::Con(name, args) if args.is_empty() => name.clone(),
        Pat::Con(name, args) => {
            let arg_strs: Vec<String> = args.iter().map(format_pat).collect();
            format!("{}({})", name, arg_strs.join(", "))
        }
        Pat::NamedCon(name, fields) => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_pat(v)))
                .collect();
            format!("{} {{ {} }}", name, field_strs.join(", "))
        }
        Pat::As(pat, alias) => format!("{} @ {}", format_pat(pat), alias),
    }
}

// ============================================================================
// OWNERSHIP COUNTING FUNCTIONS
// ============================================================================

/// Count how many times each variable name appears as ExprKind::Var in an expression tree.
/// This is the core of escape analysis: single-use variables can be moved, not cloned.
fn count_var_uses(expr: &Expr, counts: &mut BTreeMap<String, usize>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
        ExprKind::App(func, args) => {
            count_var_uses(func, counts);
            for a in args {
                count_var_uses(a, counts);
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            count_var_uses(lhs, counts);
            count_var_uses(rhs, counts);
        }
        ExprKind::UnOp(_, inner) => count_var_uses(inner, counts),
        ExprKind::If(cond, then_, else_) => {
            count_var_uses(cond, counts);
            // Both branches count — a variable used in either branch still needs
            // to be available, so we count the max across branches for safety.
            // But for simplicity in this first pass, count all uses (overapproximation
            // means more clones, but never incorrect).
            count_var_uses(then_, counts);
            count_var_uses(else_, counts);
        }
        ExprKind::Match(scrutinee, arms) => {
            count_var_uses(scrutinee, counts);
            for arm in arms {
                count_var_uses(&arm.body, counts);
                if let Some(guard) = &arm.guard {
                    count_var_uses(guard, counts);
                }
            }
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                count_var_uses_stmt(stmt, counts);
            }
        }
        ExprKind::Lambda(_, body) => {
            // Lambda captures count as uses of outer variables
            count_var_uses(body, counts);
        }
        ExprKind::Field(base, _) => count_var_uses(base, counts),
        ExprKind::Index(base, idx) => {
            count_var_uses(base, counts);
            count_var_uses(idx, counts);
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            for e in elems {
                count_var_uses(e, counts);
            }
        }
        ExprKind::Effect(_, args) => {
            for a in args {
                count_var_uses(a, counts);
            }
        }
        ExprKind::Lit(_) | ExprKind::Unit => {}
        ExprKind::Try(inner) => count_var_uses(inner, counts),
        ExprKind::Conjunction(goals) => {
            for g in goals {
                count_var_uses(g, counts);
            }
        }
        ExprKind::Pipe(input, transform) => {
            count_var_uses(input, counts);
            count_var_uses(transform, counts);
        }
        ExprKind::Handle { handlers, body, .. } => {
            count_var_uses(body, counts);
            for h in handlers {
                count_var_uses(&h.body, counts);
            }
        }
    }
}

fn count_var_uses_stmt(stmt: &Stmt, counts: &mut BTreeMap<String, usize>) {
    match stmt {
        Stmt::Bind(_, _, expr) | Stmt::Expr(expr) | Stmt::MonadicBind(_, _, expr) => {
            count_var_uses(expr, counts)
        }
        Stmt::Defn(defn) => match defn {
            Defn::Fn { body, .. } => count_var_uses(body, counts),
            _ => {}
        },
        Stmt::Annot(_, args) => {
            for a in args {
                count_var_uses(a, counts);
            }
        }
        Stmt::For(_, iter_expr, body) => {
            count_var_uses(iter_expr, counts);
            for s in body {
                count_var_uses_stmt(s, counts);
            }
        }
        Stmt::Send(target, msg) => {
            count_var_uses(target, counts);
            count_var_uses(msg, counts);
        }
        Stmt::StreamBind(_, expr) => count_var_uses(expr, counts),
        Stmt::StreamSub(expr, arms) => {
            count_var_uses(expr, counts);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_var_uses(g, counts);
                }
                count_var_uses(&arm.body, counts);
            }
        }
        Stmt::Rule(Rule::Scope { body, .. }) => {
            for s in body {
                count_var_uses_stmt(s, counts);
            }
        }
        _ => {}
    }
}

/// Count only CONSUMING uses of variables — places where ownership is required.
/// A consuming use is: argument to a non-borrow function call, or constructor argument.
/// Non-consuming: show() args (emitted as .to_string(), which borrows), BinOp operands,
/// if conditions, field access, match scrutinees.
fn count_consuming_uses(expr: &Expr, counts: &mut BTreeMap<String, usize>) {
    match &expr.kind {
        ExprKind::App(func, args) => {
            count_consuming_uses(func, counts);
            let is_borrow_builtin = matches!(func.as_ref().kind, ExprKind::Var(ref n) if matches!(builtin_canonical(n), "show" | "length" | "head" | "tail" | "nth" | "contains" | "string_length" | "char_at" | "substring" | "any" | "all" | "find" | "count_by" | "map_get" | "map_get_or" | "map_len" | "map_keys" | "map_values" | "map_contains_key" | "set_contains" | "set_len" | "set_to_list"));
            for a in args {
                if !is_borrow_builtin {
                    // Non-borrow function call: Var args are consuming
                    if let ExprKind::Var(name) = &a.kind {
                        *counts.entry(name.clone()).or_insert(0) += 1;
                    }
                }
                // Always recurse into complex arg expressions
                count_consuming_uses(a, counts);
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            // BinOp operands are non-consuming (arithmetic/comparison borrows or uses Copy)
            count_consuming_uses(lhs, counts);
            count_consuming_uses(rhs, counts);
        }
        ExprKind::UnOp(_, inner) => count_consuming_uses(inner, counts),
        ExprKind::If(cond, then_, else_) => {
            count_consuming_uses(cond, counts);
            count_consuming_uses(then_, counts);
            count_consuming_uses(else_, counts);
        }
        ExprKind::Match(scrutinee, arms) => {
            count_consuming_uses(scrutinee, counts);
            for arm in arms {
                count_consuming_uses(&arm.body, counts);
                if let Some(guard) = &arm.guard {
                    count_consuming_uses(guard, counts);
                }
            }
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                count_consuming_uses_stmt(stmt, counts);
            }
        }
        ExprKind::Lambda(_, body) => count_consuming_uses(body, counts),
        ExprKind::Field(base, _) => count_consuming_uses(base, counts),
        ExprKind::Index(base, idx) => {
            count_consuming_uses(base, counts);
            count_consuming_uses(idx, counts);
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            for e in elems {
                // Variables placed in a list/tuple are moved (consumed)
                if let ExprKind::Var(name) = &e.kind {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
                count_consuming_uses(e, counts);
            }
        }
        ExprKind::Effect(_, args) => {
            for a in args {
                count_consuming_uses(a, counts);
            }
        }
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
        ExprKind::Try(inner) => count_consuming_uses(inner, counts),
        ExprKind::Conjunction(goals) => {
            for g in goals {
                count_consuming_uses(g, counts);
            }
        }
        ExprKind::Pipe(input, transform) => {
            // Pipe input is consumed (passed as arg)
            if let ExprKind::Var(name) = &input.as_ref().kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses(input, counts);
            count_consuming_uses(transform, counts);
        }
        ExprKind::Handle { handlers, body, .. } => {
            count_consuming_uses(body, counts);
            for h in handlers {
                count_consuming_uses(&h.body, counts);
            }
        }
    }
}

fn count_consuming_uses_stmt(stmt: &Stmt, counts: &mut BTreeMap<String, usize>) {
    match stmt {
        Stmt::Bind(_, _, expr) => {
            // Binding a variable to another variable is a consuming use (move)
            if let ExprKind::Var(name) = &expr.kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses(expr, counts);
        }
        Stmt::Expr(expr) | Stmt::MonadicBind(_, _, expr) => count_consuming_uses(expr, counts),
        Stmt::Defn(defn) => {
            if let Defn::Fn { body, .. } = defn {
                count_consuming_uses(body, counts);
            }
        }
        Stmt::Annot(_, args) => {
            for a in args {
                count_consuming_uses(a, counts);
            }
        }
        Stmt::For(_, iter_expr, body) => {
            count_consuming_uses(iter_expr, counts);
            for s in body {
                count_consuming_uses_stmt(s, counts);
            }
        }
        Stmt::Send(target, msg) => {
            count_consuming_uses(target, counts);
            count_consuming_uses(msg, counts);
        }
        Stmt::StreamBind(_, expr) => count_consuming_uses(expr, counts),
        Stmt::StreamSub(expr, arms) => {
            count_consuming_uses(expr, counts);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_consuming_uses(g, counts);
                }
                count_consuming_uses(&arm.body, counts);
            }
        }
        _ => {}
    }
}

/// Branch-aware consuming use counting: for if/match, takes the MAX across branches
/// rather than summing them. A variable used once in each branch of an if/else
/// only needs one consuming use, not two — it can be moved on whichever branch executes.
fn count_consuming_uses_branch_aware(expr: &Expr, counts: &mut BTreeMap<String, usize>) {
    match &expr.kind {
        ExprKind::App(func, args) => {
            count_consuming_uses_branch_aware(func, counts);
            let is_borrow_builtin = matches!(func.as_ref().kind, ExprKind::Var(ref n) if matches!(builtin_canonical(n), "show" | "length" | "head" | "tail" | "nth" | "contains" | "string_length" | "char_at" | "substring" | "any" | "all" | "find" | "count_by" | "map_get" | "map_get_or" | "map_len" | "map_keys" | "map_values" | "map_contains_key" | "set_contains" | "set_len" | "set_to_list"));
            for a in args {
                if !is_borrow_builtin {
                    if let ExprKind::Var(name) = &a.kind {
                        *counts.entry(name.clone()).or_insert(0) += 1;
                    }
                }
                count_consuming_uses_branch_aware(a, counts);
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            count_consuming_uses_branch_aware(lhs, counts);
            count_consuming_uses_branch_aware(rhs, counts);
        }
        ExprKind::UnOp(_, inner) => count_consuming_uses_branch_aware(inner, counts),
        ExprKind::If(cond, then_, else_) => {
            count_consuming_uses_branch_aware(cond, counts);
            // Branch-aware: take MAX of then/else, not sum
            let mut then_counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut else_counts: BTreeMap<String, usize> = BTreeMap::new();
            count_consuming_uses_branch_aware(then_, &mut then_counts);
            count_consuming_uses_branch_aware(else_, &mut else_counts);
            // Merge: for each variable, add max(then, else) to counts
            let all_vars: BTreeSet<String> = then_counts
                .keys()
                .chain(else_counts.keys())
                .cloned()
                .collect();
            for var in all_vars {
                let t = then_counts.get(&var).copied().unwrap_or(0);
                let e = else_counts.get(&var).copied().unwrap_or(0);
                *counts.entry(var).or_insert(0) += std::cmp::max(t, e);
            }
        }
        ExprKind::Match(scrutinee, arms) => {
            count_consuming_uses_branch_aware(scrutinee, counts);
            // Branch-aware: take MAX across all arms
            let mut arm_counts: Vec<BTreeMap<String, usize>> = Vec::new();
            for arm in arms {
                let mut ac: BTreeMap<String, usize> = BTreeMap::new();
                count_consuming_uses_branch_aware(&arm.body, &mut ac);
                if let Some(guard) = &arm.guard {
                    count_consuming_uses_branch_aware(guard, &mut ac);
                }
                arm_counts.push(ac);
            }
            // Merge: for each variable, add max across all arms
            let all_vars: BTreeSet<String> = arm_counts
                .iter()
                .flat_map(|ac| ac.keys().cloned())
                .collect();
            for var in all_vars {
                let max_count = arm_counts
                    .iter()
                    .map(|ac| ac.get(&var).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                *counts.entry(var).or_insert(0) += max_count;
            }
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                count_consuming_uses_branch_aware_stmt(stmt, counts);
            }
        }
        ExprKind::Lambda(_, body) => count_consuming_uses_branch_aware(body, counts),
        ExprKind::Field(base, _) => count_consuming_uses_branch_aware(base, counts),
        ExprKind::Index(base, idx) => {
            count_consuming_uses_branch_aware(base, counts);
            count_consuming_uses_branch_aware(idx, counts);
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            for e in elems {
                if let ExprKind::Var(name) = &e.kind {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
                count_consuming_uses_branch_aware(e, counts);
            }
        }
        ExprKind::Effect(_, args) => {
            for a in args {
                count_consuming_uses_branch_aware(a, counts);
            }
        }
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
        ExprKind::Try(inner) => count_consuming_uses_branch_aware(inner, counts),
        ExprKind::Conjunction(goals) => {
            for g in goals {
                count_consuming_uses_branch_aware(g, counts);
            }
        }
        ExprKind::Pipe(input, transform) => {
            if let ExprKind::Var(name) = &input.as_ref().kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses_branch_aware(input, counts);
            count_consuming_uses_branch_aware(transform, counts);
        }
        ExprKind::Handle { handlers, body, .. } => {
            count_consuming_uses_branch_aware(body, counts);
            for h in handlers {
                count_consuming_uses_branch_aware(&h.body, counts);
            }
        }
    }
}

fn count_consuming_uses_branch_aware_stmt(stmt: &Stmt, counts: &mut BTreeMap<String, usize>) {
    match stmt {
        Stmt::Bind(_, _, expr) => {
            // Binding a variable to another variable is a consuming use (move)
            if let ExprKind::Var(name) = &expr.kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses_branch_aware(expr, counts);
        }
        Stmt::Expr(expr) | Stmt::MonadicBind(_, _, expr) => {
            count_consuming_uses_branch_aware(expr, counts)
        }
        Stmt::Defn(defn) => {
            if let Defn::Fn { body, .. } = defn {
                count_consuming_uses_branch_aware(body, counts);
            }
        }
        Stmt::Annot(_, args) => {
            for a in args {
                count_consuming_uses_branch_aware(a, counts);
            }
        }
        Stmt::For(_, iter_expr, body) => {
            count_consuming_uses_branch_aware(iter_expr, counts);
            for s in body {
                count_consuming_uses_branch_aware_stmt(s, counts);
            }
        }
        Stmt::Send(target, msg) => {
            count_consuming_uses_branch_aware(target, counts);
            count_consuming_uses_branch_aware(msg, counts);
        }
        Stmt::StreamBind(_, expr) => count_consuming_uses_branch_aware(expr, counts),
        Stmt::StreamSub(expr, arms) => {
            count_consuming_uses_branch_aware(expr, counts);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_consuming_uses_branch_aware(g, counts);
                }
                count_consuming_uses_branch_aware(&arm.body, counts);
            }
        }
        _ => {}
    }
}

/// Count consuming uses with borrow-awareness: args to known-borrow-param functions
/// are NOT counted as consuming (they're passed by reference, not moved).
fn count_consuming_uses_borrow_aware(
    expr: &Expr,
    counts: &mut BTreeMap<String, usize>,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
    self_fn_name: Option<&str>,
    self_param_names: &[&str],
) {
    match &expr.kind {
        ExprKind::App(func, args) => {
            // Count closure variable in callee position as consuming use.
            // If a variable is used as a function (closure call), it must be alive.
            // Only count if not a known top-level function (borrow_fns tracks all defined fns).
            if let ExprKind::Var(name) = &func.as_ref().kind {
                if !known_borrow_fns.contains_key(name.as_str())
                    && self_fn_name != Some(name.as_str())
                {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
            }
            count_consuming_uses_borrow_aware(
                func,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            let is_borrow_builtin = matches!(func.as_ref().kind, ExprKind::Var(ref n) if matches!(builtin_canonical(n), "show" | "length" | "head" | "tail" | "nth" | "contains" | "string_length" | "char_at" | "substring" | "any" | "all" | "find" | "count_by" | "map_get" | "map_get_or" | "map_len" | "map_keys" | "map_values" | "map_contains_key" | "set_contains" | "set_len" | "set_to_list"));
            // Phase 3d: Check if this is a self-recursive call
            let is_self_recursive = if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                self_fn_name == Some(fn_name.as_str())
            } else {
                false
            };
            // Look up borrow flags for the called function
            let borrow_flags = if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                known_borrow_fns.get(fn_name.as_str())
            } else {
                None
            };
            for (idx, a) in args.iter().enumerate() {
                let is_borrow_param = borrow_flags
                    .and_then(|f| f.get(idx).copied())
                    .unwrap_or(false);
                // Phase 3d: self-recursive pass-through — if the arg is a param
                // passed at the same position to itself, it's not consuming
                let is_self_passthrough = is_self_recursive
                    && if let ExprKind::Var(name) = &a.kind {
                        self_param_names
                            .get(idx)
                            .map(|pn| *pn == name.as_str())
                            .unwrap_or(false)
                    } else {
                        false
                    };
                if !is_borrow_builtin && !is_borrow_param && !is_self_passthrough {
                    if let ExprKind::Var(name) = &a.kind {
                        *counts.entry(name.clone()).or_insert(0) += 1;
                    }
                }
                count_consuming_uses_borrow_aware(
                    a,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            count_consuming_uses_borrow_aware(
                lhs,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            count_consuming_uses_borrow_aware(
                rhs,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        ExprKind::UnOp(_, inner) => count_consuming_uses_borrow_aware(
            inner,
            counts,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::If(cond, then_, else_) => {
            count_consuming_uses_borrow_aware(
                cond,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            let mut then_counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut else_counts: BTreeMap<String, usize> = BTreeMap::new();
            count_consuming_uses_borrow_aware(
                then_,
                &mut then_counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            count_consuming_uses_borrow_aware(
                else_,
                &mut else_counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            let all_vars: BTreeSet<String> = then_counts
                .keys()
                .chain(else_counts.keys())
                .cloned()
                .collect();
            for var in all_vars {
                let t = then_counts.get(&var).copied().unwrap_or(0);
                let e = else_counts.get(&var).copied().unwrap_or(0);
                *counts.entry(var).or_insert(0) += std::cmp::max(t, e);
            }
        }
        ExprKind::Match(scrutinee, arms) => {
            count_consuming_uses_borrow_aware(
                scrutinee,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            let mut arm_counts: Vec<BTreeMap<String, usize>> = Vec::new();
            for arm in arms {
                let mut ac: BTreeMap<String, usize> = BTreeMap::new();
                count_consuming_uses_borrow_aware(
                    &arm.body,
                    &mut ac,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
                if let Some(guard) = &arm.guard {
                    count_consuming_uses_borrow_aware(
                        guard,
                        &mut ac,
                        known_borrow_fns,
                        self_fn_name,
                        self_param_names,
                    );
                }
                arm_counts.push(ac);
            }
            let all_vars: BTreeSet<String> = arm_counts
                .iter()
                .flat_map(|ac| ac.keys().cloned())
                .collect();
            for var in all_vars {
                let max_count = arm_counts
                    .iter()
                    .map(|ac| ac.get(&var).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                *counts.entry(var).or_insert(0) += max_count;
            }
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                count_consuming_uses_borrow_aware_stmt(
                    stmt,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        ExprKind::Lambda(_, body) => count_consuming_uses_borrow_aware(
            body,
            counts,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::Field(base, _) => {
            // Field access borrows the base, but if the base was already moved, it fails.
            // Count field access as a consuming use so variables are cloned when needed.
            if let ExprKind::Var(name) = &base.as_ref().kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses_borrow_aware(
                base,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        ExprKind::Index(base, idx) => {
            count_consuming_uses_borrow_aware(
                base,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            count_consuming_uses_borrow_aware(
                idx,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            for e in elems {
                if let ExprKind::Var(name) = &e.kind {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
                count_consuming_uses_borrow_aware(
                    e,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        ExprKind::Effect(_, args) => {
            for a in args {
                count_consuming_uses_borrow_aware(
                    a,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
        ExprKind::Try(inner) => count_consuming_uses_borrow_aware(
            inner,
            counts,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::Conjunction(goals) => {
            for g in goals {
                count_consuming_uses_borrow_aware(
                    g,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        ExprKind::Pipe(input, transform) => {
            if let ExprKind::Var(name) = &input.as_ref().kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses_borrow_aware(
                input,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            count_consuming_uses_borrow_aware(
                transform,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        ExprKind::Handle { handlers, body, .. } => {
            count_consuming_uses_borrow_aware(
                body,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            for h in handlers {
                count_consuming_uses_borrow_aware(
                    &h.body,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
    }
}

fn count_consuming_uses_borrow_aware_stmt(
    stmt: &Stmt,
    counts: &mut BTreeMap<String, usize>,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
    self_fn_name: Option<&str>,
    self_param_names: &[&str],
) {
    match stmt {
        Stmt::Bind(_, _, expr) => {
            if let ExprKind::Var(name) = &expr.kind {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
            count_consuming_uses_borrow_aware(
                expr,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        Stmt::Expr(expr) | Stmt::MonadicBind(_, _, expr) => {
            count_consuming_uses_borrow_aware(
                expr,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        Stmt::Defn(defn) => {
            if let Defn::Fn { body, .. } = defn {
                count_consuming_uses_borrow_aware(
                    body,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        Stmt::Annot(_, args) => {
            for a in args {
                count_consuming_uses_borrow_aware(
                    a,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        Stmt::For(_, iter_expr, body) => {
            count_consuming_uses_borrow_aware(
                iter_expr,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            for s in body {
                count_consuming_uses_borrow_aware_stmt(
                    s,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        Stmt::Send(target, msg) => {
            count_consuming_uses_borrow_aware(
                target,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            count_consuming_uses_borrow_aware(
                msg,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
        }
        Stmt::StreamBind(_, expr) => count_consuming_uses_borrow_aware(
            expr,
            counts,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        Stmt::StreamSub(expr, arms) => {
            count_consuming_uses_borrow_aware(
                expr,
                counts,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            );
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_consuming_uses_borrow_aware(
                        g,
                        counts,
                        known_borrow_fns,
                        self_fn_name,
                        self_param_names,
                    );
                }
                count_consuming_uses_borrow_aware(
                    &arm.body,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        Stmt::Rule(Rule::Scope { body, .. }) => {
            for s in body {
                count_consuming_uses_borrow_aware_stmt(
                    s,
                    counts,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                );
            }
        }
        _ => {}
    }
}

/// Check whether a parameter has any consuming use that is NOT a field access.
/// Field access (w.temp, w.condition) borrows the base — it works fine on &T.
/// Returns true if the param is consumed in a non-field context (function arg, list element, bind RHS).
fn has_consuming_non_field_use(
    expr: &Expr,
    param: &str,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
    self_fn_name: Option<&str>,
    self_param_names: &[&str],
) -> bool {
    match &expr.kind {
        ExprKind::App(func, args) => {
            if has_consuming_non_field_use(
                func,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) {
                return true;
            }
            let is_borrow_builtin = matches!(func.as_ref().kind, ExprKind::Var(ref n) if matches!(builtin_canonical(n), "show" | "length" | "head" | "tail" | "nth" | "contains" | "string_length" | "char_at" | "substring" | "any" | "all" | "find" | "count_by" | "map_get" | "map_get_or" | "map_len" | "map_keys" | "map_values" | "map_contains_key" | "set_contains" | "set_len" | "set_to_list"));
            // Phase 3d: self-recursive call detection
            let is_self_recursive = if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                self_fn_name == Some(fn_name.as_str())
            } else {
                false
            };
            let borrow_flags = if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                known_borrow_fns.get(fn_name.as_str())
            } else {
                None
            };
            for (idx, a) in args.iter().enumerate() {
                let is_borrow_param = borrow_flags
                    .and_then(|f| f.get(idx).copied())
                    .unwrap_or(false);
                // Phase 3d: self-recursive pass-through is not consuming
                let is_self_passthrough = is_self_recursive
                    && if let ExprKind::Var(name) = &a.kind {
                        self_param_names
                            .get(idx)
                            .map(|pn| *pn == name.as_str())
                            .unwrap_or(false)
                    } else {
                        false
                    };
                if !is_borrow_builtin && !is_borrow_param && !is_self_passthrough {
                    if let ExprKind::Var(name) = &a.kind {
                        if name == param {
                            return true;
                        }
                    }
                }
                if has_consuming_non_field_use(
                    a,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                ) {
                    return true;
                }
            }
            false
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            has_consuming_non_field_use(
                lhs,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                rhs,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        ExprKind::UnOp(_, inner) => has_consuming_non_field_use(
            inner,
            param,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::If(cond, then_, else_) => {
            has_consuming_non_field_use(
                cond,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                then_,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                else_,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        ExprKind::Match(scrutinee, arms) => {
            if has_consuming_non_field_use(
                scrutinee,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) {
                return true;
            }
            arms.iter().any(|arm| {
                has_consuming_non_field_use(
                    &arm.body,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                ) || arm.guard.as_ref().map_or(false, |g| {
                    has_consuming_non_field_use(
                        g,
                        param,
                        known_borrow_fns,
                        self_fn_name,
                        self_param_names,
                    )
                })
            })
        }
        ExprKind::Block(stmts) => stmts.iter().any(|s| {
            has_consuming_non_field_use_stmt(
                s,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }),
        ExprKind::Lambda(_, body) => has_consuming_non_field_use(
            body,
            param,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::Field(_, _) => false, // Field access is a borrow — never consuming
        ExprKind::Index(base, idx) => {
            has_consuming_non_field_use(
                base,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                idx,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => elems.iter().any(|e| {
            if let ExprKind::Var(name) = &e.kind {
                if name == param {
                    return true;
                }
            }
            has_consuming_non_field_use(e, param, known_borrow_fns, self_fn_name, self_param_names)
        }),
        ExprKind::Effect(_, args) => args.iter().any(|a| {
            has_consuming_non_field_use(a, param, known_borrow_fns, self_fn_name, self_param_names)
        }),
        ExprKind::Try(inner) => has_consuming_non_field_use(
            inner,
            param,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        ExprKind::Pipe(input, transform) => {
            // Pipe input is consumed
            if let ExprKind::Var(name) = &input.as_ref().kind {
                if name == param {
                    return true;
                }
            }
            has_consuming_non_field_use(
                input,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                transform,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        ExprKind::Handle { handlers, body, .. } => {
            has_consuming_non_field_use(
                body,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || handlers.iter().any(|h| {
                has_consuming_non_field_use(
                    &h.body,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                )
            })
        }
        _ => false,
    }
}

fn has_consuming_non_field_use_stmt(
    stmt: &Stmt,
    param: &str,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
    self_fn_name: Option<&str>,
    self_param_names: &[&str],
) -> bool {
    match stmt {
        Stmt::Bind(_, _, expr) => {
            if let ExprKind::Var(name) = &expr.kind {
                if name == param {
                    return true;
                }
            }
            has_consuming_non_field_use(
                expr,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        Stmt::Expr(expr) | Stmt::MonadicBind(_, _, expr) => has_consuming_non_field_use(
            expr,
            param,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        Stmt::Defn(defn) => {
            if let Defn::Fn { body, .. } = defn {
                has_consuming_non_field_use(
                    body,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                )
            } else {
                false
            }
        }
        Stmt::Annot(_, args) => args.iter().any(|a| {
            has_consuming_non_field_use(a, param, known_borrow_fns, self_fn_name, self_param_names)
        }),
        Stmt::For(_, iter_expr, body) => {
            has_consuming_non_field_use(
                iter_expr,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || body.iter().any(|s| {
                has_consuming_non_field_use_stmt(
                    s,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                )
            })
        }
        Stmt::Send(target, msg) => {
            has_consuming_non_field_use(
                target,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) || has_consuming_non_field_use(
                msg,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            )
        }
        Stmt::StreamBind(_, expr) => has_consuming_non_field_use(
            expr,
            param,
            known_borrow_fns,
            self_fn_name,
            self_param_names,
        ),
        Stmt::StreamSub(expr, arms) => {
            if has_consuming_non_field_use(
                expr,
                param,
                known_borrow_fns,
                self_fn_name,
                self_param_names,
            ) {
                return true;
            }
            for arm in arms {
                if let Some(g) = &arm.guard {
                    if has_consuming_non_field_use(
                        g,
                        param,
                        known_borrow_fns,
                        self_fn_name,
                        self_param_names,
                    ) {
                        return true;
                    }
                }
                if has_consuming_non_field_use(
                    &arm.body,
                    param,
                    known_borrow_fns,
                    self_fn_name,
                    self_param_names,
                ) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Check if TCE would need to update a borrowed parameter (which is impossible).
/// Returns true if any tail call passes a different expression for a borrowed param.
fn tce_has_borrowed_param_update(
    fn_name: &str,
    params: &[Param],
    borrow_flags: &[bool],
    body: &Expr,
) -> bool {
    let mut found = false;
    tce_check_borrowed_update(fn_name, params, borrow_flags, body, &mut found);
    found
}

fn tce_check_borrowed_update(
    fn_name: &str,
    params: &[Param],
    borrow_flags: &[bool],
    expr: &Expr,
    found: &mut bool,
) {
    match &expr.kind {
        ExprKind::App(func, args) => {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                if name == fn_name {
                    // Check each arg: if param is borrowed and arg differs from param name, flag it
                    for (idx, (p, a)) in params.iter().zip(args.iter()).enumerate() {
                        let is_borrowed = borrow_flags.get(idx).copied().unwrap_or(false);
                        if is_borrowed {
                            // If the arg is just the same variable, it's a pass-through (OK)
                            let is_passthrough =
                                matches!(a.kind, ExprKind::Var(ref n) if n == &p.name);
                            if !is_passthrough {
                                *found = true;
                            }
                        }
                    }
                }
            }
        }
        ExprKind::If(_, then_, else_) => {
            tce_check_borrowed_update(fn_name, params, borrow_flags, then_, found);
            tce_check_borrowed_update(fn_name, params, borrow_flags, else_, found);
        }
        ExprKind::Block(stmts) => {
            if let Some(Stmt::Expr(last)) = stmts.last() {
                tce_check_borrowed_update(fn_name, params, borrow_flags, last, found);
            }
        }
        _ => {}
    }
}

/// ── Tail-Call Elimination ────────────────────────────────────────────
/// Detect if a function body is tail-recursive: every exit path is either
/// a base case (no self-call) or a self-call in tail position.
/// A self-call is in tail position if it's the final expression evaluated.
fn is_tail_recursive(fn_name: &str, body: &Expr) -> bool {
    let mut has_tail_call = false;
    is_tail_recursive_expr(fn_name, body, &mut has_tail_call);
    has_tail_call
}

fn is_tail_recursive_expr(fn_name: &str, expr: &Expr, found: &mut bool) -> bool {
    match &expr.kind {
        ExprKind::App(func, _args) => {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                if name == fn_name {
                    *found = true;
                    return true; // tail call
                }
            }
            true // non-self call in tail position is fine (base case)
        }
        ExprKind::If(_, then_, else_) => {
            is_tail_recursive_expr(fn_name, then_, found)
                && is_tail_recursive_expr(fn_name, else_, found)
        }
        ExprKind::Block(stmts) => {
            // Check if any non-last stmt contains a self-call (that would be non-tail)
            for (i, stmt) in stmts.iter().enumerate() {
                let is_last = i == stmts.len() - 1;
                match stmt {
                    Stmt::Expr(e) if is_last => {
                        return is_tail_recursive_expr(fn_name, e, found);
                    }
                    Stmt::Bind(_, _, val) | Stmt::MonadicBind(_, _, val) => {
                        // Bindings must not contain self-calls
                        if expr_contains_call(fn_name, val) {
                            return false;
                        }
                    }
                    Stmt::Expr(e) => {
                        if expr_contains_call(fn_name, e) {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
            true
        }
        ExprKind::Match(scrutinee, arms) => {
            if expr_contains_call(fn_name, scrutinee) {
                return false;
            }
            arms.iter()
                .all(|arm| is_tail_recursive_expr(fn_name, &arm.body, found))
        }
        // Any other expression in tail position is a base case — fine
        _ => true,
    }
}

/// Check if an expression contains a call to fn_name anywhere (non-tail position check)
fn expr_contains_call(fn_name: &str, expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::App(func, args) => {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                if name == fn_name {
                    return true;
                }
            }
            args.iter().any(|a| expr_contains_call(fn_name, a))
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            expr_contains_call(fn_name, lhs) || expr_contains_call(fn_name, rhs)
        }
        ExprKind::UnOp(_, inner) => expr_contains_call(fn_name, inner),
        ExprKind::If(c, t, e) => {
            expr_contains_call(fn_name, c)
                || expr_contains_call(fn_name, t)
                || expr_contains_call(fn_name, e)
        }
        ExprKind::Block(stmts) => stmts.iter().any(|s| match s {
            Stmt::Bind(_, _, e) | Stmt::MonadicBind(_, _, e) | Stmt::Expr(e) => {
                expr_contains_call(fn_name, e)
            }
            _ => false,
        }),
        ExprKind::Match(scrutinee, arms) => {
            expr_contains_call(fn_name, scrutinee)
                || arms.iter().any(|a| expr_contains_call(fn_name, &a.body))
        }
        ExprKind::Lambda(_, body) => expr_contains_call(fn_name, body),
        ExprKind::Field(base, _) => expr_contains_call(fn_name, base),
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            elems.iter().any(|e| expr_contains_call(fn_name, e))
        }
        _ => false,
    }
}

/// Analyze a function body to determine which parameters are only borrowed (never consumed).
/// A param is borrow-only if it never appears as a consuming use (function arg, constructor arg,
/// list/tuple element, returned value) AND is not destructured by match (unless ref-match safe).
/// Borrow-only params can be emitted as &T instead of T.
///
/// Phase 3b: ref-match relaxation — matched params CAN be borrowed if:
/// - The param type has all-Copy type arguments (so pattern bindings are references to Copy types)
/// - The return type is Copy (so returning a pattern binding just copies through the deref)
/// - The param is not consumed in other ways (not passed to non-borrow functions)
fn analyze_borrow_only_params(
    params: &[Param],
    body: &Expr,
    ret_ty: Option<&Ty>,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
) -> Vec<bool> {
    analyze_borrow_only_params_named(params, body, ret_ty, known_borrow_fns, None)
}

fn analyze_borrow_only_params_named(
    params: &[Param],
    body: &Expr,
    ret_ty: Option<&Ty>,
    known_borrow_fns: &BTreeMap<String, Vec<bool>>,
    self_fn_name: Option<&str>,
) -> Vec<bool> {
    // Phase 3d: Self-recursive borrow relaxation
    // When a param is passed to a self-recursive call at the same position,
    // that's NOT a consuming use — if we decide to borrow, the recursive call
    // will also take &T, so it's a pass-through.
    let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();

    let mut consuming: BTreeMap<String, usize> = BTreeMap::new();
    count_consuming_uses_borrow_aware(
        body,
        &mut consuming,
        known_borrow_fns,
        self_fn_name,
        &param_names,
    );

    // Check if the param is returned (tail position) — that's a consuming use
    let mut returned_vars: BTreeSet<String> = BTreeSet::new();
    collect_returned_vars(body, &mut returned_vars);

    // Check if param is used as a match scrutinee — pattern matching destructures
    let mut matched_vars: BTreeSet<String> = BTreeSet::new();
    collect_matched_vars(body, &mut matched_vars);

    // Can the return type be produced from a Copy deref?
    let ret_is_copy = ret_ty.map(|t| is_copy_type(t)).unwrap_or(false);

    params
        .iter()
        .map(|p| {
            if p.inout {
                return false;
            } // inout params already have &mut, skip
            if let Some(ty) = &p.ty {
                if is_copy_type(ty) {
                    return false;
                } // Copy types don't benefit from borrowing
                  // Check for function types — can't borrow closures easily
                if matches!(ty, Ty::Arrow(..)) {
                    return false;
                }
            }
            let consumed = consuming.get(&p.name).copied().unwrap_or(0);
            let returned = returned_vars.contains(&p.name);
            let matched = matched_vars.contains(&p.name);

            if consumed == 0 && !returned && !matched {
                // Classic case: param is only read (show, field access, etc.) → borrow
                return true;
            }

            // Phase 3c: field-only relaxation
            // If a param's "consuming" uses are ALL from field access (w.temp, w.condition),
            // it can still be borrowed — field access works fine on &T.
            if consumed > 0 && !returned && !matched {
                if !has_consuming_non_field_use(
                    body,
                    &p.name,
                    known_borrow_fns,
                    self_fn_name,
                    &param_names,
                ) {
                    return true; // All consuming uses are field access → safe to borrow
                }
            }

            // Phase 3b: ref-match relaxation
            // If param is ONLY matched (not consumed in other ways) and all type args are Copy,
            // and return type is Copy, we can match on &T — pattern bindings auto-deref for Copy
            if matched && consumed == 0 && !returned {
                if let Some(ty) = &p.ty {
                    if type_has_all_copy_args(ty) && ret_is_copy {
                        return true; // ref-match: borrow despite matching
                    }
                }
            }

            false
        })
        .collect()
}

/// Collect variables used as match scrutinees (pattern matching destructures them)
fn collect_matched_vars(expr: &Expr, vars: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Match(scrutinee, arms) => {
            if let ExprKind::Var(name) = &scrutinee.as_ref().kind {
                vars.insert(name.clone());
            }
            collect_matched_vars(scrutinee, vars);
            for arm in arms {
                collect_matched_vars(&arm.body, vars);
                if let Some(guard) = &arm.guard {
                    collect_matched_vars(guard, vars);
                }
            }
        }
        ExprKind::App(func, args) => {
            collect_matched_vars(func, vars);
            for a in args {
                collect_matched_vars(a, vars);
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            collect_matched_vars(lhs, vars);
            collect_matched_vars(rhs, vars);
        }
        ExprKind::UnOp(_, inner) => collect_matched_vars(inner, vars),
        ExprKind::If(cond, then_, else_) => {
            collect_matched_vars(cond, vars);
            collect_matched_vars(then_, vars);
            collect_matched_vars(else_, vars);
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                match stmt {
                    Stmt::Bind(_, _, e) | Stmt::Expr(e) | Stmt::MonadicBind(_, _, e) => {
                        collect_matched_vars(e, vars)
                    }
                    Stmt::For(_, iter_expr, body) => {
                        collect_matched_vars(iter_expr, vars);
                        for s in body {
                            if let Stmt::Expr(e) | Stmt::Bind(_, _, e) = s {
                                collect_matched_vars(e, vars);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        ExprKind::Lambda(_, body) => collect_matched_vars(body, vars),
        ExprKind::Field(base, _) => collect_matched_vars(base, vars),
        ExprKind::Index(base, idx) => {
            collect_matched_vars(base, vars);
            collect_matched_vars(idx, vars);
        }
        ExprKind::List(elems) | ExprKind::Tuple(elems) => {
            for e in elems {
                collect_matched_vars(e, vars);
            }
        }
        ExprKind::Try(inner) => collect_matched_vars(inner, vars),
        ExprKind::Handle { handlers, body, .. } => {
            collect_matched_vars(body, vars);
            for h in handlers {
                collect_matched_vars(&h.body, vars);
            }
        }
        _ => {}
    }
}

/// Collect pattern bindings from match expressions whose scrutinee is `param_name`.
/// These bindings are references (&T) because the param is borrowed — they need * deref.
fn collect_ref_match_bindings_from_body(
    body: &Expr,
    param_name: &str,
    bindings: &mut BTreeSet<String>,
) {
    match &body.kind {
        ExprKind::Match(scrutinee, arms) => {
            if let ExprKind::Var(name) = &scrutinee.as_ref().kind {
                if name == param_name {
                    // This match is on the ref-matched param — collect all pattern bindings
                    for arm in arms {
                        collect_pattern_binding_names(&arm.pat, bindings);
                    }
                }
            }
            // Also recurse into arm bodies and scrutinee
            collect_ref_match_bindings_from_body(scrutinee, param_name, bindings);
            for arm in arms {
                collect_ref_match_bindings_from_body(&arm.body, param_name, bindings);
                if let Some(guard) = &arm.guard {
                    collect_ref_match_bindings_from_body(guard, param_name, bindings);
                }
            }
        }
        ExprKind::App(func, args) => {
            collect_ref_match_bindings_from_body(func, param_name, bindings);
            for a in args {
                collect_ref_match_bindings_from_body(a, param_name, bindings);
            }
        }
        ExprKind::BinOp(_, lhs, rhs) => {
            collect_ref_match_bindings_from_body(lhs, param_name, bindings);
            collect_ref_match_bindings_from_body(rhs, param_name, bindings);
        }
        ExprKind::UnOp(_, inner) => {
            collect_ref_match_bindings_from_body(inner, param_name, bindings)
        }
        ExprKind::If(cond, then_, else_) => {
            collect_ref_match_bindings_from_body(cond, param_name, bindings);
            collect_ref_match_bindings_from_body(then_, param_name, bindings);
            collect_ref_match_bindings_from_body(else_, param_name, bindings);
        }
        ExprKind::Block(stmts) => {
            for stmt in stmts {
                match stmt {
                    Stmt::Bind(_, _, e) | Stmt::Expr(e) | Stmt::MonadicBind(_, _, e) => {
                        collect_ref_match_bindings_from_body(e, param_name, bindings);
                    }
                    Stmt::For(_, iter_expr, body_stmts) => {
                        collect_ref_match_bindings_from_body(iter_expr, param_name, bindings);
                        for s in body_stmts {
                            if let Stmt::Expr(e) | Stmt::Bind(_, _, e) = s {
                                collect_ref_match_bindings_from_body(e, param_name, bindings);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        ExprKind::Lambda(_, inner) => {
            collect_ref_match_bindings_from_body(inner, param_name, bindings)
        }
        ExprKind::Field(base, _) => {
            collect_ref_match_bindings_from_body(base, param_name, bindings)
        }
        ExprKind::Try(inner) => collect_ref_match_bindings_from_body(inner, param_name, bindings),
        _ => {}
    }
}

/// Collect all variable names bound in a pattern (excluding wildcards and literals)
fn collect_pattern_binding_names(pat: &Pat, names: &mut BTreeSet<String>) {
    match pat {
        Pat::Var(name) if name != "_" => {
            names.insert(name.clone());
        }
        Pat::Con(_, args) => {
            for a in args {
                collect_pattern_binding_names(a, names);
            }
        }
        Pat::NamedCon(_, named_args) => {
            for (_, p) in named_args {
                collect_pattern_binding_names(p, names);
            }
        }
        Pat::As(inner, name) => {
            collect_pattern_binding_names(inner, names);
            names.insert(name.clone());
        }
        _ => {}
    }
}

/// Collect variables that appear in return/tail position of an expression
fn collect_returned_vars(expr: &Expr, vars: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Var(name) => {
            vars.insert(name.clone());
        }
        ExprKind::If(_, then_, else_) => {
            collect_returned_vars(then_, vars);
            collect_returned_vars(else_, vars);
        }
        ExprKind::Match(_, arms) => {
            for arm in arms {
                collect_returned_vars(&arm.body, vars);
            }
        }
        ExprKind::Block(stmts) => {
            if let Some(last) = stmts.last() {
                match last {
                    Stmt::Expr(e) => collect_returned_vars(e, vars),
                    Stmt::Bind(_, _, e) => collect_returned_vars(e, vars),
                    _ => {}
                }
            }
        }
        // Other expressions: not a simple variable return
        _ => {}
    }
}

/// Detect aliased variables: `= y = x` where x is a non-Copy variable
fn collect_aliased_vars(stmts: &[&Stmt], copy_vars: &BTreeSet<String>) -> BTreeSet<String> {
    let mut aliased = BTreeSet::new();
    for stmt in stmts {
        if let Stmt::Bind(
            Pat::Var(_name),
            _,
            Expr {
                kind: ExprKind::Var(source),
                ..
            },
        ) = stmt
        {
            if !copy_vars.contains(source.as_str()) {
                aliased.insert(source.clone());
            }
        }
    }
    aliased
}

/// Check if an expression contains the ? (Try) operator
fn expr_contains_try(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Try(_) => true,
        ExprKind::App(f, args) => expr_contains_try(f) || args.iter().any(expr_contains_try),
        ExprKind::BinOp(_, l, r) => expr_contains_try(l) || expr_contains_try(r),
        ExprKind::UnOp(_, e) => expr_contains_try(e),
        ExprKind::If(c, t, e) => {
            expr_contains_try(c) || expr_contains_try(t) || expr_contains_try(e)
        }
        ExprKind::Match(s, arms) => {
            expr_contains_try(s) || arms.iter().any(|a| expr_contains_try(&a.body))
        }
        ExprKind::Block(stmts) => stmts.iter().any(stmt_contains_try),
        ExprKind::Field(e, _) | ExprKind::Index(e, _) => expr_contains_try(e),
        ExprKind::Lambda(_, body) => expr_contains_try(body),
        ExprKind::List(es) | ExprKind::Tuple(es) | ExprKind::Effect(_, es) => {
            es.iter().any(expr_contains_try)
        }
        ExprKind::Handle { body, handlers, .. } => {
            expr_contains_try(body) || handlers.iter().any(|h| expr_contains_try(&h.body))
        }
        ExprKind::Pipe(input, transform) => {
            expr_contains_try(input) || expr_contains_try(transform)
        }
        _ => false,
    }
}

fn stmt_contains_try(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Bind(_, _, e) | Stmt::Expr(e) => expr_contains_try(e),
        Stmt::For(_, iter, body) => expr_contains_try(iter) || body.iter().any(stmt_contains_try),
        Stmt::MonadicBind(_, _, _) => true, // MonadicBind IS a ? operation
        Stmt::Send(target, msg) => expr_contains_try(target) || expr_contains_try(msg),
        Stmt::Assert(_, args) | Stmt::Retract(_, args) => args.iter().any(expr_contains_try),
        Stmt::Abort => false,
        Stmt::Defn(_)
        | Stmt::TypeDecl(_)
        | Stmt::Use(_)
        | Stmt::Import(_)
        | Stmt::QualifiedImport(_, _)
        | Stmt::HashImport(_, _)
        | Stmt::Depend(_, _)
        | Stmt::RustBlock(_)
        | Stmt::Annot(_, _)
        | Stmt::Rule(_)
        | Stmt::StreamBind(_, _)
        | Stmt::StreamSub(_, _)
        | Stmt::Invariant { .. }
        | Stmt::Prove { .. } => false,
    }
}

/// Find variables that are bound outside a for loop and rebound inside it (need `let mut`)
fn collect_mutable_vars(stmts: &[&Stmt], mutable: &mut BTreeSet<String>) {
    let mut bound = BTreeSet::new();
    for stmt in stmts {
        if let Stmt::Bind(Pat::Var(name), _, _) = stmt {
            bound.insert(name.clone());
        }
        if let Stmt::For(_, _, body) = stmt {
            collect_rebound_in_body(body, &bound, mutable);
        }
    }
    // Variables that are bound to closures/lambdas and called as functions need `let mut`
    // because they may be FnMut. Detect: variable in `bound` used in App(Var(name), _)
    let mut called: BTreeSet<String> = BTreeSet::new();
    for stmt in stmts {
        collect_called_vars(stmt, &mut called);
    }
    // Variables that are called AND are local bindings (not function defs) need mut
    for name in called {
        if bound.contains(&name) {
            mutable.insert(name);
        }
    }
}

fn collect_called_vars(stmt: &Stmt, called: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Expr(expr) | Stmt::Bind(_, _, expr) => collect_called_vars_expr(expr, called),
        Stmt::For(_, iter, body) => {
            collect_called_vars_expr(iter, called);
            for s in body {
                collect_called_vars(s, called);
            }
        }
        _ => {}
    }
}

fn collect_called_vars_expr(expr: &Expr, called: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::App(func, args) => {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                called.insert(name.clone());
            }
            collect_called_vars_expr(func, called);
            for a in args {
                collect_called_vars_expr(a, called);
            }
        }
        ExprKind::BinOp(_, l, r) => {
            collect_called_vars_expr(l, called);
            collect_called_vars_expr(r, called);
        }
        ExprKind::If(c, t, e) => {
            collect_called_vars_expr(c, called);
            collect_called_vars_expr(t, called);
            collect_called_vars_expr(e, called);
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                collect_called_vars(s, called);
            }
        }
        ExprKind::Match(scrutinee, arms) => {
            collect_called_vars_expr(scrutinee, called);
            for arm in arms {
                collect_called_vars_expr(&arm.body, called);
            }
        }
        ExprKind::Lambda(_, body) => collect_called_vars_expr(body, called),
        ExprKind::Effect(_, args) => {
            for a in args {
                collect_called_vars_expr(a, called);
            }
        }
        ExprKind::Field(obj, _) => collect_called_vars_expr(obj, called),
        ExprKind::Index(arr, idx) => {
            collect_called_vars_expr(arr, called);
            collect_called_vars_expr(idx, called);
        }
        _ => {}
    }
}

/// Determine if a closure-typed param can safely be FnOnce.
/// Returns true only when the param is used exactly once as a direct call
/// at the top level of the function body (not inside a nested lambda,
/// not passed as an argument to another function).
fn can_be_fn_once(body: &Expr, name: &str) -> bool {
    // Count total uses of this variable (all references)
    let mut total = BTreeMap::new();
    count_var_uses(body, &mut total);
    let total_uses = total.get(name).copied().unwrap_or(0);
    if total_uses != 1 {
        return false;
    }

    // The single use must be a direct call at top level (not inside a lambda)
    is_direct_call_at_top_level(body, name)
}

/// Check if a variable's sole use is a direct function call NOT inside a lambda body
fn is_direct_call_at_top_level(expr: &Expr, name: &str) -> bool {
    match &expr.kind {
        ExprKind::App(func, args) => {
            // Direct call: f(args) where f is our variable
            if matches!(func.as_ref().kind, ExprKind::Var(ref n) if n == name) {
                return true;
            }
            // Check in args (but NOT in lambda bodies within args)
            is_direct_call_at_top_level(func, name)
                || args.iter().any(|a| is_direct_call_at_top_level(a, name))
        }
        ExprKind::BinOp(_, l, r) => {
            is_direct_call_at_top_level(l, name) || is_direct_call_at_top_level(r, name)
        }
        ExprKind::UnOp(_, inner) => is_direct_call_at_top_level(inner, name),
        ExprKind::If(c, t, e) => {
            is_direct_call_at_top_level(c, name)
                || is_direct_call_at_top_level(t, name)
                || is_direct_call_at_top_level(e, name)
        }
        ExprKind::Match(scrutinee, arms) => {
            is_direct_call_at_top_level(scrutinee, name)
                || arms.iter().any(|arm| {
                    is_direct_call_at_top_level(&arm.body, name)
                        || arm
                            .guard
                            .as_ref()
                            .map_or(false, |g| is_direct_call_at_top_level(g, name))
                })
        }
        ExprKind::Block(stmts) => stmts.iter().any(|s| match s {
            Stmt::Expr(e) | Stmt::Bind(_, _, e) | Stmt::MonadicBind(_, _, e) => {
                is_direct_call_at_top_level(e, name)
            }
            Stmt::For(_, iter, body) => {
                is_direct_call_at_top_level(iter, name)
                    || body.iter().any(|s2| match s2 {
                        Stmt::Expr(e) | Stmt::Bind(_, _, e) => is_direct_call_at_top_level(e, name),
                        _ => false,
                    })
            }
            _ => false,
        }),
        // STOP recursion at lambdas — a call inside a lambda is NOT top-level
        ExprKind::Lambda(_, _) => false,
        ExprKind::Field(obj, _) => is_direct_call_at_top_level(obj, name),
        _ => false,
    }
}

/// Recursively search for variable rebindings inside for-loop bodies,
/// including nested if/else blocks and match arms.
fn collect_rebound_in_body(
    stmts: &[Stmt],
    bound: &BTreeSet<String>,
    mutable: &mut BTreeSet<String>,
) {
    for s in stmts {
        match s {
            Stmt::Bind(Pat::Var(name), _, _) => {
                if bound.contains(name) {
                    mutable.insert(name.clone());
                }
            }
            Stmt::Expr(expr) => collect_rebound_in_expr(expr, bound, mutable),
            Stmt::For(_, _, body) => collect_rebound_in_body(body, bound, mutable),
            _ => {}
        }
    }
}

fn collect_rebound_in_expr(expr: &Expr, bound: &BTreeSet<String>, mutable: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::If(_, then_, else_) => {
            collect_rebound_in_expr(then_, bound, mutable);
            collect_rebound_in_expr(else_, bound, mutable);
        }
        ExprKind::Block(stmts) => collect_rebound_in_body(stmts, bound, mutable),
        ExprKind::Match(_, arms) => {
            for arm in arms {
                collect_rebound_in_expr(&arm.body, bound, mutable);
            }
        }
        _ => {}
    }
}

/// Check if a Futuruna type is Copy in Rust (primitive numeric/char types)
fn is_copy_type(ty: &Ty) -> bool {
    matches!(ty, Ty::Name(n) if matches!(n.as_str(), "Int" | "Float" | "Char" | "Nat" | "Bool"))
}

/// Check if a type's ALL type arguments are Copy types.
/// For Pair(Int, Int) → true. For Pair(Int, String) → false. For Name("Int") → true.
fn type_has_all_copy_args(ty: &Ty) -> bool {
    match ty {
        Ty::App(_, args) => args.iter().all(|a| is_copy_type(a)),
        Ty::Name(n) => {
            is_copy_type(ty) || matches!(n.as_str(), "Int" | "Float" | "Char" | "Nat" | "Bool")
        }
        _ => false,
    }
}

impl RustCodegen {
    fn new() -> Self {
        RustCodegen {
            indent: 0,
            types: TypeRegistry::new(),
            var_use_counts: BTreeMap::new(),
            var_consuming_counts: BTreeMap::new(),
            copy_vars: BTreeSet::new(),
            mutable_vars: BTreeSet::new(),
            lib_mode: false,
            wasm_mode: false,
            cargo_deps: BTreeMap::new(),
            source_dir: None,
            imported: BTreeSet::new(),
            borrow_only_params: BTreeMap::new(),
            aliased_vars: BTreeSet::new(),
            ref_match_bindings: BTreeSet::new(),
            current_borrow_params: BTreeSet::new(),
            string_typed_vars: BTreeSet::new(),
            float_typed_vars: BTreeSet::new(),
            string_returning_fns: BTreeSet::new(),
            fn_once_mode: false,
            in_self_method: false,
            current_effects: Vec::new(),
            handle_scope_effects: BTreeSet::new(),
            var_types: BTreeMap::new(),
            has_async: false,
            subject_vars: BTreeSet::new(),
            scope_handles: BTreeMap::new(),
            current_scope: None,
            sub_counter: 0,
            scope_bindings: BTreeMap::new(),
            codegen_invariants: BTreeMap::new(),
            actor_handle_vars: BTreeMap::new(),
            sync_subject_vars: BTreeSet::new(),
            subject_elem_type: BTreeMap::new(),
            builtin_registry: rust_builtin_registry(),
            async_stream_counter: 0,
            source_name: None,
        }
    }

    /// Resolve and parse an imported .runa file, returning its statements.
    /// Supports relative paths (`./module`) and manifest dependencies (`dep_name/module`).
    fn resolve_import(&mut self, import_path: &str) -> Vec<Stmt> {
        let dir = match &self.source_dir {
            Some(d) => d.clone(),
            None => return Vec::new(),
        };

        // Try relative path first (existing behavior)
        let rel = import_path.trim_start_matches("./");
        let file_path = format!("{}/{}.runa", dir, rel);

        // If file exists OR path starts with ./ or ../, use it directly
        if import_path.starts_with("./")
            || import_path.starts_with("../")
            || std::path::Path::new(&file_path).exists()
        {
            let canon = std::fs::canonicalize(&file_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(file_path.clone());
            if self.imported.contains(&canon) {
                return Vec::new();
            }
            self.imported.insert(canon);
            return Self::parse_tau_file(&file_path);
        }

        // Try manifest-based resolution: `dep_name/module` → dependency path
        if let Some(toml_path) = find_runa_toml(&dir) {
            if let Some(manifest) = parse_runa_toml(&toml_path) {
                let toml_dir = std::path::Path::new(&toml_path)
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy().to_string();
                        if s.is_empty() {
                            ".".to_string()
                        } else {
                            s
                        }
                    })
                    .unwrap_or_else(|| ".".to_string());

                // Split import path: first component is dep name, rest is module path
                let parts: Vec<&str> = import_path.splitn(2, '/').collect();
                let dep_name = parts[0];
                let module = if parts.len() > 1 { parts[1] } else { "lib" };

                for (name, dep_spec) in &manifest.dependencies {
                    if name == dep_name {
                        let abs_dep = match resolve_dep_to_path(dep_spec, &toml_dir) {
                            Some(p) => p,
                            None => return Vec::new(),
                        };
                        let dep_file = format!("{}/{}.runa", abs_dep, module);
                        let dep_file_src = format!("{}/src/{}.runa", abs_dep, module);

                        let resolved = if std::path::Path::new(&dep_file).exists() {
                            dep_file
                        } else if std::path::Path::new(&dep_file_src).exists() {
                            dep_file_src
                        } else {
                            eprintln!("\x1b[1;31merror\x1b[0m: cannot find module '{}' in dependency '{}'", module, dep_name);
                            eprintln!("  Searched: {}", dep_file);
                            eprintln!("  Searched: {}", dep_file_src);
                            return Vec::new();
                        };

                        let canon = std::fs::canonicalize(&resolved)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or(resolved.clone());
                        if self.imported.contains(&canon) {
                            return Vec::new();
                        }
                        self.imported.insert(canon);
                        return Self::parse_tau_file(&resolved);
                    }
                }
            }
        }

        // Fallback: try the original relative path anyway (gives a proper error)
        let canon = std::fs::canonicalize(&file_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(file_path.clone());
        if self.imported.contains(&canon) {
            return Vec::new();
        }
        self.imported.insert(canon);
        Self::parse_tau_file(&file_path)
    }

    /// Parse a .runa file without import-cycle tracking (for hash imports).
    fn parse_tau_file(file_path: &str) -> Vec<Stmt> {
        match std::fs::read_to_string(file_path) {
            Ok(source) => {
                let mut lexer = Lexer::new(&source);
                let tokens = lexer.tokenize();
                let mut parser = Parser::new(tokens, &source);
                match parser.parse_program() {
                    Ok(stmts) => stmts,
                    Err(e) => {
                        eprintln!(
                            "\x1b[1;31merror\x1b[0m: parse error in {}: {}",
                            file_path, e
                        );
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("Cannot read {}: {}", file_path, e);
                Vec::new()
            }
        }
    }

    /// Resolve a hash import: parse file (no caching) and find matching definition.
    fn resolve_hash_import(&self, hash: &str, import_path: &str) -> Vec<Stmt> {
        let dir = match &self.source_dir {
            Some(d) => d.clone(),
            None => return Vec::new(),
        };
        let rel = import_path.trim_start_matches("./");
        let file_path = format!("{}/{}.runa", dir, rel);
        let stmts = Self::parse_tau_file(&file_path);
        for s in stmts {
            let matches = match &s {
                Stmt::Defn(d) => content_hash_defn(d) == hash,
                Stmt::TypeDecl(td) => content_hash_type(td) == hash,
                _ => false,
            };
            if matches {
                return vec![s];
            }
        }
        eprintln!("Hash #{} not found in {}", hash, file_path);
        Vec::new()
    }

    /// Convert a comptime TypeDef value into a TypeDecl AST node for emission
    fn typedef_to_type_decl(name: &str, kind: &str, fields: &[(String, String)]) -> TypeDecl {
        fn parse_ty(s: &str) -> Ty {
            match s {
                "Int" => Ty::Name("Int".into()),
                "Float" => Ty::Name("Float".into()),
                "String" => Ty::Name("String".into()),
                "Bool" => Ty::Name("Bool".into()),
                "Char" => Ty::Name("Char".into()),
                other => Ty::Name(other.into()),
            }
        }

        match kind {
            "struct" => {
                // Single variant with named fields
                let variant_fields: Vec<Field> = fields
                    .iter()
                    .map(|(fname, fty)| Field {
                        name: fname.clone(),
                        ty: parse_ty(fty),
                    })
                    .collect();
                let variant = Variant {
                    name: name.to_string(),
                    fields: variant_fields,
                    positional: false,
                };
                TypeDecl::ADT {
                    name: name.to_string(),
                    params: vec![],
                    variants: vec![variant],
                    methods: vec![],
                }
            }
            "enum" => {
                // Multiple variants; field string encodes sub-fields
                let variants: Vec<Variant> = fields
                    .iter()
                    .map(|(vname, field_str)| {
                        let variant_fields = if field_str.is_empty() {
                            vec![]
                        } else {
                            field_str
                                .split(',')
                                .filter_map(|part| {
                                    let parts: Vec<&str> = part.trim().splitn(2, ':').collect();
                                    if parts.len() == 2 {
                                        Some(Field {
                                            name: parts[0].trim().to_string(),
                                            ty: parse_ty(parts[1].trim()),
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        };
                        Variant {
                            name: vname.clone(),
                            fields: variant_fields,
                            positional: false,
                        }
                    })
                    .collect();
                TypeDecl::ADT {
                    name: name.to_string(),
                    params: vec![],
                    variants,
                    methods: vec![],
                }
            }
            _ => {
                // Unknown kind — fallback to empty struct
                TypeDecl::ADT {
                    name: name.to_string(),
                    params: vec![],
                    variants: vec![Variant {
                        name: name.to_string(),
                        fields: vec![],
                        positional: false,
                    }],
                    methods: vec![],
                }
            }
        }
    }

    /// Convert an interpreter Value to a Rust literal string for comptime embedding
    fn value_to_rust_literal(
        val: &Value,
        variant_parent: &BTreeMap<String, String>,
    ) -> (String, String) {
        match val {
            Value::Int(n) => (format!("{}", n), "i64".to_string()),
            Value::Float(v) => {
                let s = format!("{}", v);
                // Ensure it has a decimal point for Rust
                if s.contains('.') {
                    (format!("{}f64", s), "f64".to_string())
                } else {
                    (format!("{}.0f64", s), "f64".to_string())
                }
            }
            Value::Str(s) => (format!("{:?}.to_string()", s), "String".to_string()),
            Value::Char(c) => (format!("{:?}", c), "char".to_string()),
            Value::Bool(b) => (format!("{}", b), "bool".to_string()),
            Value::Unit => ("()".to_string(), "()".to_string()),
            Value::List(elems) => {
                let items: Vec<String> = elems
                    .iter()
                    .map(|e| Self::value_to_rust_literal(e, variant_parent).0)
                    .collect();
                if elems.is_empty() {
                    ("vec![]".to_string(), "Vec<i64>".to_string())
                } else {
                    let (_, elem_ty) = Self::value_to_rust_literal(&elems[0], variant_parent);
                    (
                        format!("vec![{}]", items.join(", ")),
                        format!("Vec<{}>", elem_ty),
                    )
                }
            }
            Value::Tuple(elems) => {
                let items: Vec<String> = elems
                    .iter()
                    .map(|e| Self::value_to_rust_literal(e, variant_parent).0)
                    .collect();
                let types: Vec<String> = elems
                    .iter()
                    .map(|e| Self::value_to_rust_literal(e, variant_parent).1)
                    .collect();
                (
                    format!("({})", items.join(", ")),
                    format!("({})", types.join(", ")),
                )
            }
            Value::Constructor(name, args) => {
                // Flatten Cons/Nil linked lists into vec![]
                if name == "Nil" || name == "Cons" {
                    let mut elems = Vec::new();
                    let mut cur = val;
                    while let Value::Constructor(n, a) = cur {
                        if n == "Cons" && a.len() == 2 {
                            elems.push(&a[0]);
                            cur = &a[1];
                        } else if n == "Nil" {
                            break;
                        } else {
                            // Not a list — fall through to generic constructor
                            let items: Vec<String> = args
                                .iter()
                                .map(|e| Self::value_to_rust_literal(e, variant_parent).0)
                                .collect();
                            return (
                                format!("/* {} */ ({})", name, items.join(", ")),
                                "()".to_string(),
                            );
                        }
                    }
                    if elems.is_empty() {
                        return ("vec![]".to_string(), "Vec<i64>".to_string());
                    }
                    let items: Vec<String> = elems
                        .iter()
                        .map(|e| Self::value_to_rust_literal(e, variant_parent).0)
                        .collect();
                    let (_, elem_ty) = Self::value_to_rust_literal(elems[0], variant_parent);
                    return (
                        format!("vec![{}]", items.join(", ")),
                        format!("Vec<{}>", elem_ty),
                    );
                }
                // Ok(val) / Err(val) / Some(val) / None
                if name == "Ok" && args.len() == 1 {
                    let (inner, inner_ty) = Self::value_to_rust_literal(&args[0], variant_parent);
                    return (
                        format!("Ok({})", inner),
                        format!("Result<{}, String>", inner_ty),
                    );
                }
                if name == "Err" && args.len() == 1 {
                    let (inner, _) = Self::value_to_rust_literal(&args[0], variant_parent);
                    return (format!("Err({})", inner), "Result<(), String>".to_string());
                }
                if name == "Some" && args.len() == 1 {
                    let (inner, inner_ty) = Self::value_to_rust_literal(&args[0], variant_parent);
                    return (format!("Some({})", inner), format!("Option<{}>", inner_ty));
                }
                if name == "None" && args.is_empty() {
                    return ("None".to_string(), "Option<()>".to_string());
                }
                // Generic constructor — use variant_parent to find the parent ADT type
                let parent_ty = variant_parent
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_default();
                let items: Vec<String> = args
                    .iter()
                    .map(|e| Self::value_to_rust_literal(e, variant_parent).0)
                    .collect();
                if parent_ty.is_empty() {
                    // Unknown constructor — emit as-is (no parent type known)
                    if args.is_empty() {
                        (name.clone(), "()".to_string())
                    } else {
                        (format!("{}({})", name, items.join(", ")), "()".to_string())
                    }
                } else if args.is_empty() {
                    (format!("{}::{}", parent_ty, name), parent_ty.clone())
                } else {
                    (
                        format!("{}::{}({})", parent_ty, name, items.join(", ")),
                        parent_ty.clone(),
                    )
                }
            }
            Value::NamedConstructor(name, fields) => {
                // Look up the parent type for enum variants (e.g., ResultKind::Leaf)
                let parent_ty = variant_parent
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_default();
                let qualified = if parent_ty.is_empty() || parent_ty == *name {
                    name.clone() // Struct type (parent == name) or unknown
                } else {
                    format!("{}::{}", parent_ty, name) // Enum variant
                };
                if fields.is_empty() {
                    (
                        qualified.clone(),
                        if parent_ty.is_empty() {
                            name.clone()
                        } else {
                            parent_ty
                        },
                    )
                } else {
                    let items: Vec<String> = fields
                        .iter()
                        .map(|(fname, fval)| {
                            let (v, _) = Self::value_to_rust_literal(fval, variant_parent);
                            format!("{}: {}", fname, v)
                        })
                        .collect();
                    (
                        format!("{} {{ {} }}", qualified, items.join(", ")),
                        if parent_ty.is_empty() {
                            name.clone()
                        } else {
                            parent_ty
                        },
                    )
                }
            }
            _ => (
                format!("todo!(\"comptime: unsupported value\")",),
                "()".to_string(),
            ),
        }
    }

    /// Static check: does a type reference a given ADT name? (no &self needed logic)
    fn type_references_adt_static(ty: &Ty, adt_name: &str) -> bool {
        match ty {
            Ty::Name(n) => n == adt_name,
            Ty::App(con, args) => {
                Self::type_references_adt_static(con, adt_name)
                    || args
                        .iter()
                        .any(|a| Self::type_references_adt_static(a, adt_name))
            }
            Ty::Arrow(from, to) => {
                Self::type_references_adt_static(from, adt_name)
                    || Self::type_references_adt_static(to, adt_name)
            }
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
                Self::type_references_adt_static(inner, adt_name)
            }
            _ => false,
        }
    }

    /// Rename types that conflict with Rust std (Option, Result, Bool)
    fn rust_type_name(&self, tau_name: &str) -> String {
        self.types
            .type_rename
            .get(tau_name)
            .cloned()
            .unwrap_or_else(|| tau_name.to_string())
    }

    /// Returns "Rc" or "Arc" depending on whether the program uses async
    fn rc_name(&self) -> &str {
        if self.has_async {
            "Arc"
        } else {
            "Rc"
        }
    }

    /// Check if a pattern's constructor belongs to an Rc-backed type
    fn pattern_is_rc_type(&self, pat: &Pat) -> bool {
        match pat {
            Pat::Con(name, _) | Pat::NamedCon(name, _) => self
                .types
                .variant_parent
                .get(name.as_str())
                .map_or(false, |parent| self.types.rc_types.contains(parent)),
            _ => false,
        }
    }

    fn ind(&self) -> String {
        "    ".repeat(self.indent)
    }

    /// Pass 1: Scan declarations — resolve imports, register types, detect async.
    /// Returns the resolved statement list (imports merged, deduped).
    /// Populates TypeRegistry, exported_names, effect tracking, async flags.
    fn scan_declarations(&mut self, stmts: &[Stmt]) -> Vec<Stmt> {
        // Resolve @ import statements: parse imported .runa files and merge their definitions
        let mut all_stmts: Vec<Stmt> = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::Import(path) => {
                    let imported = self.resolve_import(path);
                    // M3b: import all definitions (transitive deps needed).
                    // @ export controls `pub` in Rust output via exported_names pre-scan.
                    // Also propagate @ export annotations from imported files so their
                    // exported names get `pub` in the Rust output.
                    {
                        let mut is_exp = false;
                        for s in &imported {
                            if let Stmt::Annot(n, args) = s {
                                if n == "export" {
                                    for a in args {
                                        if let ExprKind::Var(v) = &a.kind {
                                            self.types.exported_names.insert(v.clone());
                                        }
                                    }
                                    if args.is_empty() {
                                        is_exp = true;
                                    }
                                    continue;
                                }
                            }
                            if is_exp {
                                match s {
                                    Stmt::Defn(Defn::Fn { name, .. })
                                    | Stmt::Defn(Defn::Actor { name, .. }) => {
                                        self.types.exported_names.insert(name.clone());
                                    }
                                    Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                                        self.types.exported_names.insert(name.clone());
                                    }
                                    _ => {}
                                }
                                is_exp = false;
                            }
                        }
                    }
                    // Merge definitions (functions, types, rules, bindings, rust blocks, deps)
                    // Skip only side-effects (@ print, for loops, etc.)
                    for s in imported {
                        match &s {
                            Stmt::Defn(_)
                            | Stmt::TypeDecl(_)
                            | Stmt::RustBlock(_)
                            | Stmt::Rule(_)
                            | Stmt::Bind(..)
                            | Stmt::Use(_)
                            | Stmt::Invariant { .. } => {
                                all_stmts.push(s);
                            }
                            Stmt::Depend(cn, cv) => {
                                self.cargo_deps.insert(cn.clone(), cv.clone());
                            }
                            _ => {} // skip side-effects from imported files
                        }
                    }
                }
                Stmt::Use(path) => {
                    // @ use grundlov::* → load grundlov.runa from same directory
                    // Same resolution as interpreter: strip ::*, replace :: with /
                    let module = path.trim_end_matches("::*").replace("::", "/");
                    let imported = self.resolve_import(&format!("./{}", module));
                    // Propagate @ export annotations
                    {
                        let mut is_exp = false;
                        for s in &imported {
                            if let Stmt::Annot(n, args) = s {
                                if n == "export" {
                                    for a in args {
                                        if let ExprKind::Var(v) = &a.kind {
                                            self.types.exported_names.insert(v.clone());
                                        }
                                    }
                                    if args.is_empty() {
                                        is_exp = true;
                                    }
                                    continue;
                                }
                            }
                            if is_exp {
                                match s {
                                    Stmt::Defn(Defn::Fn { name, .. })
                                    | Stmt::Defn(Defn::Actor { name, .. }) => {
                                        self.types.exported_names.insert(name.clone());
                                    }
                                    Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                                        self.types.exported_names.insert(name.clone());
                                    }
                                    _ => {}
                                }
                                is_exp = false;
                            }
                        }
                    }
                    // Merge definitions (functions, types, bindings, rules, rust blocks, deps)
                    // Skip side effects (@ print, @ skriv, for loops, etc.)
                    for s in imported {
                        match &s {
                            Stmt::Defn(_)
                            | Stmt::TypeDecl(_)
                            | Stmt::RustBlock(_)
                            | Stmt::Rule(_)
                            | Stmt::Bind(..)
                            | Stmt::Use(_)
                            | Stmt::Invariant { .. } => {
                                all_stmts.push(s);
                            }
                            Stmt::Depend(cn, cv) => {
                                self.cargo_deps.insert(cn.clone(), cv.clone());
                            }
                            _ => {} // skip side effects from imported files
                        }
                    }
                }
                Stmt::QualifiedImport(mod_name, path) => {
                    // @ import Name from ./module — qualified import (M3b)
                    // Parse imported file, scan for exports, wrap in Rust `mod Name { }`
                    let imported = self.resolve_import(path);
                    // Collect exported names from the imported file
                    let mut mod_exported: BTreeSet<String> = BTreeSet::new();
                    {
                        let mut is_exp = false;
                        for s in &imported {
                            if let Stmt::Annot(n, args) = s {
                                if n == "export" {
                                    for a in args {
                                        if let ExprKind::Var(v) = &a.kind {
                                            mod_exported.insert(v.clone());
                                        }
                                    }
                                    if args.is_empty() {
                                        is_exp = true;
                                    }
                                    continue;
                                }
                            }
                            if is_exp {
                                match s {
                                    Stmt::Defn(Defn::Fn { name, .. })
                                    | Stmt::Defn(Defn::Actor { name, .. }) => {
                                        mod_exported.insert(name.clone());
                                    }
                                    Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                                        mod_exported.insert(name.clone());
                                    }
                                    Stmt::Bind(Pat::Var(name), _, _) => {
                                        mod_exported.insert(name.clone());
                                    }
                                    Stmt::StreamBind(name, _) => {
                                        mod_exported.insert(name.clone());
                                    }
                                    _ => {}
                                }
                                is_exp = false;
                            }
                        }
                    }
                    // Mark exported names so they get `pub` in the module
                    for n in &mod_exported {
                        self.types.exported_names.insert(n.clone());
                    }
                    // Collect definitions (functions, types, rust blocks), skip executable code
                    let mut mod_body: Vec<Stmt> = Vec::new();
                    for s in imported {
                        match &s {
                            Stmt::Defn(_)
                            | Stmt::TypeDecl(_)
                            | Stmt::RustBlock(_)
                            | Stmt::Use(_)
                            | Stmt::Annot(_, _) => {
                                mod_body.push(s);
                            }
                            Stmt::Depend(cn, cv) => {
                                self.cargo_deps.insert(cn.clone(), cv.clone());
                            }
                            _ => {} // skip top-level expressions/binds from imported files
                        }
                    }
                    // Wrap in a Defn::Module so it emits as `mod Name { ... }`
                    self.types.known_modules.insert(mod_name.clone());
                    all_stmts.push(Stmt::Defn(Defn::Module {
                        name: mod_name.clone(),
                        body: mod_body,
                    }));
                }
                Stmt::HashImport(hash, path) => {
                    let matched = self.resolve_hash_import(hash, path);
                    all_stmts.extend(matched);
                }
                Stmt::Depend(crate_name, version) => {
                    self.cargo_deps.insert(crate_name.clone(), version.clone());
                }
                _ => all_stmts.push(stmt.clone()),
            }
        }

        // Deduplicate type declarations from imports: if the same type name is
        // defined multiple times (e.g. each file redefines # Branch for standalone use),
        // keep only the first definition. This prevents Rust duplicate type errors.
        {
            let mut seen_types: BTreeSet<String> = BTreeSet::new();
            all_stmts.retain(|s| {
                if let Stmt::TypeDecl(TypeDecl::ADT { name, .. })
                | Stmt::TypeDecl(TypeDecl::EffectDecl { name, .. })
                | Stmt::TypeDecl(TypeDecl::TraitDecl { name, .. }) = s
                {
                    seen_types.insert(name.clone())
                } else {
                    true
                }
            });
        }

        let stmts = &all_stmts;

        // Pre-scan: collect @ export annotations (M3b)
        // Two forms: `@ export` (prefix, next stmt is exported) or `@ export name` (post-hoc)
        {
            let mut is_export = false;
            for stmt in stmts {
                if let Stmt::Annot(name, args) = stmt {
                    if name == "export" {
                        // Post-hoc form: `@ export add` → args contains Var("add")
                        if !args.is_empty() {
                            for arg in args {
                                if let ExprKind::Var(n) = &arg.kind {
                                    self.types.exported_names.insert(n.clone());
                                }
                            }
                            continue;
                        }
                        // Prefix form: next statement is exported
                        is_export = true;
                        continue;
                    }
                }
                if is_export {
                    match stmt {
                        Stmt::Defn(Defn::Fn { name, .. }) => {
                            self.types.exported_names.insert(name.clone());
                        }
                        Stmt::Defn(Defn::Actor { name, .. }) => {
                            self.types.exported_names.insert(name.clone());
                        }
                        Stmt::TypeDecl(TypeDecl::ADT { name, .. }) => {
                            self.types.exported_names.insert(name.clone());
                        }
                        Stmt::Bind(Pat::Var(name), _, _) => {
                            self.types.exported_names.insert(name.clone());
                        }
                        Stmt::StreamBind(name, _) => {
                            self.types.exported_names.insert(name.clone());
                        }
                        _ => {}
                    }
                    is_export = false;
                }
            }
        }

        // M26: Pre-scan for @ store annotations — collect stored types and their key fields
        {
            for stmt in stmts {
                if let Stmt::Annot(name, args) = stmt {
                    if name == "store" {
                        if let Some(Expr {
                            kind: ExprKind::Var(type_name),
                            ..
                        }) = args.first()
                        {
                            self.types.stored_types.insert(type_name.clone());
                            // Scan remaining args for flags and scope
                            for arg in args.iter().skip(1) {
                                match &arg.kind {
                                    ExprKind::Var(flag) if flag == "delete_on_change" => {
                                        self.types.store_delete_on_change.insert(type_name.clone());
                                    }
                                    ExprKind::Lit(Literal::Str(scope)) => {
                                        self.types.store_scope = Some(scope.clone());
                                    }
                                    _ => {}
                                }
                            }
                            // Auto-add dependencies
                            self.cargo_deps.insert(
                                "rusqlite".into(),
                                "{ version = \"0.32\", features = [\"bundled\"] }".into(),
                            );
                            self.cargo_deps.insert(
                                "serde".into(),
                                "{ version = \"1\", features = [\"derive\"] }".into(),
                            );
                            self.cargo_deps.insert("serde_json".into(), "1".into());
                        }
                    }
                }
            }
            // Find key fields and compute schema hashes for stored types
            for stmt in stmts {
                if let Stmt::TypeDecl(TypeDecl::ADT { name, variants, .. }) = stmt {
                    if self.types.stored_types.contains(name.as_str()) {
                        if let Some(v) = variants.first() {
                            if let Some(f) = v.fields.first() {
                                self.types
                                    .stored_type_key_field
                                    .insert(name.clone(), f.name.clone());
                            }
                            // Compute schema hash from field names + types
                            let mut schema_str = String::new();
                            for f in &v.fields {
                                schema_str.push_str(&f.name);
                                schema_str.push(':');
                                schema_str.push_str(&format!("{:?}", f.ty));
                                schema_str.push(';');
                            }
                            use std::hash::{Hash, Hasher};
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            schema_str.hash(&mut hasher);
                            let hash = format!("{:016x}", hasher.finish());
                            self.types
                                .stored_type_schema_hash
                                .insert(name.clone(), hash);
                        }
                    }
                }
            }
        }

        // M13c: Pre-scan for async mode — only async when subjects have for-loop subscribers
        {
            // Pass 1: collect subject variable names
            fn collect_subject_names(stmts: &[Stmt], names: &mut BTreeSet<String>) {
                for s in stmts {
                    match s {
                        Stmt::StreamBind(name, expr) => {
                            if matches!(expr.kind, ExprKind::App(ref f, _) if matches!(f.as_ref().kind, ExprKind::Var(ref n) if n == "subject"))
                            {
                                names.insert(name.clone());
                            }
                        }
                        Stmt::Rule(Rule::Scope { body, .. }) => collect_subject_names(body, names),
                        _ => {}
                    }
                }
            }
            // Pass 2: check if any for-loop iterates a subject (= async subscription)
            fn has_for_on_subject(stmts: &[Stmt], subjects: &BTreeSet<String>) -> bool {
                for s in stmts {
                    match s {
                        Stmt::For(
                            _,
                            Expr {
                                kind: ExprKind::Var(name),
                                ..
                            },
                            _,
                        ) if subjects.contains(name) => return true,
                        Stmt::StreamSub(
                            Expr {
                                kind: ExprKind::Var(name),
                                ..
                            },
                            _,
                        ) if subjects.contains(name) => return true,
                        Stmt::Rule(Rule::Scope { body, .. }) => {
                            if has_for_on_subject(body, subjects) {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
                false
            }
            let mut subject_names = BTreeSet::new();
            collect_subject_names(stmts, &mut subject_names);
            if !subject_names.is_empty() && has_for_on_subject(stmts, &subject_names) {
                self.has_async = true;
            }
            // Also enable async if subjects exist and ANY StreamSub is present
            // (the subscription might be on a derived stream like map(subject, f))
            if !subject_names.is_empty() && !self.has_async {
                fn has_any_stream_sub(stmts: &[Stmt]) -> bool {
                    for s in stmts {
                        match s {
                            Stmt::StreamSub(_, _) => return true,
                            Stmt::Rule(Rule::Scope { body, .. }) => {
                                if has_any_stream_sub(body) {
                                    return true;
                                }
                            }
                            _ => {}
                        }
                    }
                    false
                }
                if has_any_stream_sub(stmts) {
                    self.has_async = true;
                }
            }
            for stmt in stmts {
                if matches!(stmt, Stmt::Defn(Defn::Actor { .. })) {
                    self.has_async = true;
                }
                // http_serve with axum needs async runtime
                if let Stmt::Expr(Expr {
                    kind: ExprKind::Effect(name, _),
                    ..
                }) = stmt
                {
                    if name == "http_serve" {
                        self.has_async = true;
                    }
                }
                // Also detect http_serve in = bindings (e.g. = _ = http_serve(...))
                if let Stmt::Bind(
                    _,
                    _,
                    Expr {
                        kind: ExprKind::App(f, _),
                        ..
                    },
                ) = stmt
                {
                    if let ExprKind::Var(name) = &f.as_ref().kind {
                        if name == "http_serve" {
                            self.has_async = true;
                        }
                    }
                }
            }
            if self.has_async {
                self.cargo_deps.insert(
                    "tokio".to_string(),
                    "{ version = \"1\", features = [\"full\"] }".to_string(),
                );
            }
        }

        // Pre-scan: infer subject element types from first Send or initial value
        {
            fn infer_subject_types(stmts: &[Stmt], types: &mut BTreeMap<String, String>) {
                // Collect subject names and their initial values
                let mut subject_names: BTreeSet<String> = BTreeSet::new();
                for s in stmts {
                    if let Stmt::StreamBind(name, expr) = s {
                        if matches!(expr.kind, ExprKind::App(ref f, _) if matches!(f.as_ref().kind, ExprKind::Var(ref n) if n == "subject"))
                        {
                            subject_names.insert(name.clone());
                            // Check if initial value provided: subject(val)
                            if let ExprKind::App(_, args) = &expr.kind {
                                if let Some(init) = args.first() {
                                    let ty = expr_to_rust_type(init);
                                    types.insert(name.clone(), ty);
                                }
                            }
                        }
                    }
                    if let Stmt::Rule(Rule::Scope { body, .. }) = s {
                        infer_subject_types(body, types);
                    }
                }
                // Infer from first Send if not already known
                for s in stmts {
                    match s {
                        Stmt::Send(
                            Expr {
                                kind: ExprKind::Var(target),
                                ..
                            },
                            msg,
                        ) if subject_names.contains(target) && !types.contains_key(target) => {
                            let ty = expr_to_rust_type(msg);
                            types.insert(target.clone(), ty);
                        }
                        Stmt::For(_, _, body) => {
                            for bs in body {
                                if let Stmt::Send(
                                    Expr {
                                        kind: ExprKind::Var(target),
                                        ..
                                    },
                                    msg,
                                ) = bs
                                {
                                    if subject_names.contains(target) && !types.contains_key(target)
                                    {
                                        let ty = expr_to_rust_type(msg);
                                        types.insert(target.clone(), ty);
                                    }
                                }
                            }
                        }
                        Stmt::Rule(Rule::Scope { body, .. }) => infer_subject_types(body, types),
                        _ => {}
                    }
                }
            }

            fn expr_to_rust_type(expr: &Expr) -> String {
                match &expr.kind {
                    ExprKind::Lit(Literal::Str(_)) => "String".to_string(),
                    ExprKind::Lit(Literal::Int(_)) => "i64".to_string(),
                    ExprKind::Lit(Literal::Float(_)) => "f64".to_string(),
                    ExprKind::Lit(Literal::Bool(_)) => "bool".to_string(),
                    ExprKind::Lit(Literal::Char(_)) => "char".to_string(),
                    // String concatenation: expr + expr where either is a string
                    ExprKind::BinOp(op, lhs, _) if op == "+" => {
                        if matches!(lhs.as_ref().kind, ExprKind::Lit(Literal::Str(_))) {
                            "String".to_string()
                        } else {
                            expr_to_rust_type(lhs)
                        }
                    }
                    // Constructor: infer from variant parent (fallback to String)
                    _ => "String".to_string(),
                }
            }

            infer_subject_types(stmts, &mut self.subject_elem_type);
        }

        // Build type rename map + variant→parent lookup for all ADTs
        let conflicting = ["Bool", "Box", "Vec", "String"];
        for stmt in stmts {
            if let Stmt::TypeDecl(TypeDecl::ADT {
                name,
                params,
                variants,
                ..
            }) = stmt
            {
                let rust_name = if conflicting.contains(&name.as_str()) {
                    format!("Futuruna{}", name)
                } else {
                    name.clone()
                };
                if rust_name != *name {
                    self.types
                        .type_rename
                        .insert(name.clone(), rust_name.clone());
                }
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
                for v in variants {
                    self.types
                        .variant_parent
                        .insert(v.name.clone(), rust_name.clone());
                    self.types
                        .variant_positional
                        .insert(v.name.clone(), v.positional);
                    if !v.positional {
                        let names: Vec<String> = v.fields.iter().map(|f| f.name.clone()).collect();
                        self.types.variant_fields.insert(v.name.clone(), names);
                    }
                    let ft_map: BTreeMap<String, Ty> = v
                        .fields
                        .iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    self.types
                        .variant_field_types
                        .insert(v.name.clone(), ft_map);
                    let boxed: Vec<usize> = v
                        .fields
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| RustCodegen::type_references_adt_static(&f.ty, name))
                        .map(|(i, _)| i)
                        .collect();
                    if !boxed.is_empty() {
                        self.types.variant_boxed_args.insert(v.name.clone(), boxed);
                    }
                }
                if variants.len() == 1
                    && variants[0].name == *name
                    && !variants[0].fields.is_empty()
                {
                    self.types.struct_types.insert(rust_name.clone());
                }
                self.types
                    .type_decls
                    .insert(rust_name, (param_names, variant_names));
            }
            // Scan types inside modules too
            if let Stmt::Defn(Defn::Module { body, .. }) = stmt {
                for inner_stmt in body {
                    if let Stmt::TypeDecl(TypeDecl::ADT {
                        name,
                        params,
                        variants,
                        ..
                    }) = inner_stmt
                    {
                        let rust_name = if conflicting.contains(&name.as_str()) {
                            format!("Futuruna{}", name)
                        } else {
                            name.clone()
                        };
                        if rust_name != *name {
                            self.types
                                .type_rename
                                .insert(name.clone(), rust_name.clone());
                        }
                        let param_names: Vec<String> =
                            params.iter().map(|p| p.name.clone()).collect();
                        let variant_names: Vec<String> =
                            variants.iter().map(|v| v.name.clone()).collect();
                        for v in variants {
                            self.types
                                .variant_parent
                                .insert(v.name.clone(), rust_name.clone());
                            self.types
                                .variant_positional
                                .insert(v.name.clone(), v.positional);
                            if !v.positional {
                                let names: Vec<String> =
                                    v.fields.iter().map(|f| f.name.clone()).collect();
                                self.types.variant_fields.insert(v.name.clone(), names);
                            }
                            let ft_map: BTreeMap<String, Ty> = v
                                .fields
                                .iter()
                                .map(|f| (f.name.clone(), f.ty.clone()))
                                .collect();
                            self.types
                                .variant_field_types
                                .insert(v.name.clone(), ft_map);
                            let boxed: Vec<usize> = v
                                .fields
                                .iter()
                                .enumerate()
                                .filter(|(_, f)| {
                                    RustCodegen::type_references_adt_static(&f.ty, name)
                                })
                                .map(|(i, _)| i)
                                .collect();
                            if !boxed.is_empty() {
                                self.types.variant_boxed_args.insert(v.name.clone(), boxed);
                            }
                        }
                        if variants.len() == 1
                            && variants[0].name == *name
                            && !variants[0].fields.is_empty()
                        {
                            self.types.struct_types.insert(rust_name.clone());
                        }
                        self.types
                            .type_decls
                            .insert(rust_name, (param_names, variant_names));
                    }
                }
            }
            // Register user-defined function names
            if let Stmt::Defn(Defn::Fn { name, ret_ty, .. }) = stmt {
                self.types.user_functions.insert(name.clone());
                if matches!(ret_ty.as_ref(), Some(Ty::Name(n)) if n == "String") {
                    self.string_returning_fns.insert(name.clone());
                }
            }
        }

        // Compute Rc types for transparent structural sharing (M25)
        {
            let mut recursive_types = BTreeSet::new();
            for (variant_name, indices) in &self.types.variant_boxed_args {
                if !indices.is_empty() {
                    if let Some(parent) = self.types.variant_parent.get(variant_name.as_str()) {
                        recursive_types.insert(parent.clone());
                    }
                }
            }
            self.types.rc_types = recursive_types;
        }

        all_stmts
    }

    /// Pass 2: Compute borrow-only parameter flags for all functions.
    /// Iterates to fixed point so transitive borrow info propagates.
    fn compute_borrow_flags(&mut self, fn_stmts: &[&Stmt]) {
        for _round in 0..8 {
            let prev_count = self.borrow_only_params.len();
            for stmt in fn_stmts {
                if let Stmt::Defn(Defn::Fn {
                    name,
                    params,
                    ret_ty,
                    body,
                    ..
                }) = stmt
                {
                    let mut borrow_flags = analyze_borrow_only_params_named(
                        params,
                        body,
                        ret_ty.as_ref(),
                        &self.borrow_only_params,
                        Some(name.as_str()),
                    );
                    // Disable ref-match for types with boxed (recursive) fields
                    {
                        let mut matched_vars: BTreeSet<String> = BTreeSet::new();
                        collect_matched_vars(body, &mut matched_vars);
                        for (idx, p) in params.iter().enumerate() {
                            if borrow_flags[idx] && matched_vars.contains(&p.name) {
                                if let Some(ty) = &p.ty {
                                    let type_name = match ty {
                                        Ty::App(base, _) => {
                                            if let Ty::Name(n) = base.as_ref() {
                                                Some(n.as_str())
                                            } else {
                                                None
                                            }
                                        }
                                        Ty::Name(n) => Some(n.as_str()),
                                        _ => None,
                                    };
                                    if let Some(tn) = type_name {
                                        let has_boxed = self.types.variant_boxed_args.iter().any(
                                            |(vname, indices)| {
                                                !indices.is_empty()
                                                    && self
                                                        .types
                                                        .variant_parent
                                                        .get(vname.as_str())
                                                        .map(|p| {
                                                            p == tn
                                                                || self
                                                                    .types
                                                                    .type_rename
                                                                    .get(tn)
                                                                    .map(|r| r == p)
                                                                    .unwrap_or(false)
                                                        })
                                                        .unwrap_or(false)
                                            },
                                        );
                                        if has_boxed {
                                            borrow_flags[idx] = false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if borrow_flags.iter().any(|f| *f) {
                        self.borrow_only_params.insert(name.clone(), borrow_flags);
                    }
                    // Also pre-register inout params
                    let inout_flags: Vec<bool> = params.iter().map(|p| p.inout).collect();
                    if inout_flags.iter().any(|f| *f) {
                        self.types.inout_params.insert(name.clone(), inout_flags);
                    }
                }
            }
            if self.borrow_only_params.len() == prev_count {
                break;
            }
        }
    }

    fn emit_program(&mut self, input_stmts: &[Stmt]) -> String {
        let all_stmts = self.scan_declarations(input_stmts);
        let stmts = &all_stmts;
        let mut out = String::new();

        // Header
        out.push_str("// Generated by runa --emit rust\n");
        out.push_str("// Futuruna: the language designed by measuring consciousness\n\n");
        out.push_str("#![allow(unused, non_snake_case, non_camel_case_types)]\n");
        if self.wasm_mode {
            out.push_str("use wasm_bindgen::prelude::*;\n");
        }
        out.push_str("use std::fmt;\n");
        out.push_str("use std::collections::{HashMap, HashSet};\n");

        // Emit use declarations
        for stmt in stmts.iter() {
            if let Stmt::Use(path) = stmt {
                out.push_str(&format!("use {};\n", path));
            }
        }
        // M13c: emit ScopeGuard struct when async mode is active
        if self.has_async {
            out.push_str("use tokio::sync::broadcast;\n");
            out.push_str(
                "\n/// Scope lifecycle guard: Drop aborts all subscription tasks (M13c)\n",
            );
            out.push_str("struct _ScopeGuard {\n");
            out.push_str("    handles: Vec<tokio::task::JoinHandle<()>>,\n");
            out.push_str("}\n");
            out.push_str("impl Drop for _ScopeGuard {\n");
            out.push_str("    fn drop(&mut self) {\n");
            out.push_str("        for h in &self.handles {\n");
            out.push_str("            h.abort();\n");
            out.push_str("        }\n");
            out.push_str("    }\n");
            out.push_str("}\n");
        }
        out.push('\n');

        // Type registration + Rc computation done in scan_declarations (Pass 1).
        // Emit Rc/Arc import for transparent structural sharing
        if !self.types.rc_types.is_empty() {
            if self.has_async {
                out.push_str("use std::sync::Arc;\n");
            } else {
                out.push_str("use std::rc::Rc;\n");
            }
        }

        // Pre-scan: find explicit Display impls to avoid auto-generating duplicates
        for stmt in stmts {
            if let Stmt::TypeDecl(TypeDecl::ImplBlock {
                trait_name,
                for_type,
                ..
            }) = stmt
            {
                if trait_name == "Display"
                    || trait_name == "fmt::Display"
                    || trait_name == "std::fmt::Display"
                {
                    self.types.explicit_display_impls.insert(for_type.clone());
                }
            }
        }

        // Pre-scan: build effect_ops map (effect_name -> set of operation names)
        for stmt in stmts {
            if let Stmt::TypeDecl(TypeDecl::EffectDecl { name, ops }) = stmt {
                let op_names: BTreeSet<String> = ops.iter().map(|(n, _, _)| n.clone()).collect();
                self.types.effect_ops.insert(name.clone(), op_names);
                self.types
                    .effect_ops_detail
                    .insert(name.clone(), ops.clone());
            }
        }

        // Pre-scan: build fn_effects map (fn_name -> effect names from `with`)
        for stmt in stmts {
            if let Stmt::Defn(Defn::Fn { name, effects, .. }) = stmt {
                if !effects.is_empty() {
                    self.types.fn_effects.insert(name.clone(), effects.clone());
                }
            }
        }

        // Effect type inference: walk function bodies to discover effects not
        // explicitly declared with `with`. Iterates to fixed point for transitive effects.
        {
            // Reverse map: op_name -> effect_name
            let mut op_to_effect: BTreeMap<String, String> = BTreeMap::new();
            for (eff_name, ops) in &self.types.effect_ops {
                for op in ops {
                    op_to_effect.insert(op.clone(), eff_name.clone());
                }
            }
            // Collect function bodies (only those without explicit `with`)
            let mut fn_bodies: Vec<(String, Expr)> = Vec::new();
            for stmt in stmts.iter() {
                if let Stmt::Defn(Defn::Fn {
                    name,
                    effects,
                    body,
                    ..
                }) = stmt
                {
                    if effects.is_empty() {
                        fn_bodies.push((name.clone(), body.clone()));
                    }
                }
            }
            if !op_to_effect.is_empty() && !fn_bodies.is_empty() {
                // Iterate until fixed point
                loop {
                    let mut changed = false;
                    for (fn_name, body) in &fn_bodies {
                        let handled = BTreeSet::new();
                        let inferred = Self::collect_expr_effects(
                            body,
                            &handled,
                            &op_to_effect,
                            &self.types.fn_effects,
                        );
                        if !inferred.is_empty() {
                            let existing =
                                self.types.fn_effects.entry(fn_name.clone()).or_default();
                            for eff in inferred {
                                if !existing.contains(&eff) {
                                    existing.push(eff);
                                    changed = true;
                                }
                            }
                        }
                    }
                    if !changed {
                        break;
                    }
                }
            }
        }

        // First pass: emit type declarations
        for stmt in stmts {
            if let Stmt::TypeDecl(decl) = stmt {
                out.push_str(&self.emit_type_decl(decl));
                out.push('\n');
            }
        }

        // Collect all top-level bindings and effect calls into main()
        let mut main_stmts = Vec::new();
        let mut fn_stmts = Vec::new();

        for stmt in stmts {
            match stmt {
                Stmt::Defn(defn) => {
                    // Track user-defined function names and string-returning functions
                    if let Defn::Fn { name, ret_ty, .. } = defn {
                        self.types.user_functions.insert(name.clone());
                        if matches!(ret_ty.as_ref(), Some(Ty::Name(n)) if n == "String") {
                            self.string_returning_fns.insert(name.clone());
                        }
                    }
                    fn_stmts.push(stmt);
                }
                Stmt::TypeDecl(_) => {}                    // already emitted
                Stmt::Use(_) => {}                         // already emitted in header
                Stmt::RustBlock(_) => fn_stmts.push(stmt), // emit at top level
                Stmt::Annot(name, _) if name == "comptime" => main_stmts.push(stmt), // keep comptime annotations
                Stmt::Annot(_, _) => {}                                              // skip others
                Stmt::Rule(rule) => {
                    // M13c: scopes with async content go to main (need async context)
                    if matches!(rule, Rule::Scope { .. }) {
                        if self.has_async {
                            main_stmts.push(stmt);
                        } else {
                            main_stmts.push(stmt); // sync scopes also in main (emit inline)
                        }
                    }
                    // Other rules collected below for grouped emission
                }
                _ => main_stmts.push(stmt),
            }
        }

        // Pass 2: Borrow analysis (extracted from emit_program)
        self.compute_borrow_flags(&fn_stmts);

        // Emit function definitions and @ rust { } blocks
        for stmt in &fn_stmts {
            match stmt {
                Stmt::Defn(defn) => {
                    out.push_str(&self.emit_defn(defn));
                    out.push('\n');
                }
                Stmt::RustBlock(code) => {
                    out.push_str(code);
                    out.push_str("\n\n");
                }
                _ => {}
            }
        }

        // Collect simple literal = bindings for inlining in rule bodies
        for stmt in stmts {
            if let Stmt::Bind(
                Pat::Var(name),
                _,
                Expr {
                    kind: ExprKind::Lit(lit),
                    ..
                },
            ) = stmt
            {
                let val = Self::emit_literal_value(lit);
                let ty = Self::literal_rust_type(lit).to_string();
                self.types.literal_bindings.insert(name.clone(), (val, ty));
            }
        }

        // Emit rules as Rust functions (Catala-style: exception > conditional default > unconditional default > clause)
        {
            // Group rules by name
            let mut rule_groups: BTreeMap<String, Vec<&Rule>> = BTreeMap::new();
            for stmt in stmts {
                if let Stmt::Rule(rule) = stmt {
                    match rule {
                        Rule::Scope { .. } => {} // scopes handled separately
                        _ => {
                            let name = match rule {
                                Rule::Clause { head, .. }
                                | Rule::Default { head, .. }
                                | Rule::Exception { head, .. } => match &head.kind {
                                    ExprKind::App(f, _) => {
                                        if let ExprKind::Var(n) = &f.as_ref().kind {
                                            n.clone()
                                        } else {
                                            continue;
                                        }
                                    }
                                    ExprKind::Var(n) => n.clone(),
                                    _ => continue,
                                },
                                _ => continue,
                            };
                            rule_groups.entry(name).or_default().push(rule);
                        }
                    }
                }
            }
            // Pre-register Prolog-style rule functions so type propagation works across groups
            for (fn_name, rules) in &rule_groups {
                let arity = Self::rule_arity(rules);
                if arity > 0 && Self::rules_have_prolog_features(rules) {
                    let mut param_types: Vec<&str> = vec!["String"; arity];
                    // Infer from ground terms in fact heads
                    for r in rules.iter() {
                        if let Rule::Clause { head, body: None } = r {
                            if let ExprKind::App(_, args) = &head.kind {
                                for (i, arg) in args.iter().enumerate() {
                                    if let ExprKind::Lit(lit) = &arg.kind {
                                        param_types[i] = Self::literal_rust_type(lit);
                                    }
                                }
                            }
                        }
                    }
                    // Infer from literals in body calls
                    for r in rules.iter() {
                        if let Rule::Clause {
                            head,
                            body: Some(body),
                        } = r
                        {
                            if let ExprKind::App(_, head_args) = &head.kind {
                                let head_vars: Vec<(String, usize)> = head_args
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(i, a)| {
                                        if let ExprKind::Var(name) = &a.kind {
                                            Some((name.clone(), i))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                // Check body for literal args that tell us param types
                                let body_calls: Vec<&Expr> = match &body.kind {
                                    ExprKind::Conjunction(goals) => goals.iter().collect(),
                                    _ => vec![body],
                                };
                                for call in body_calls {
                                    if let ExprKind::App(_, call_args) = &call.kind {
                                        for (ci, ca) in call_args.iter().enumerate() {
                                            if let ExprKind::Lit(lit) = &ca.kind {
                                                // A literal in the body tells us the type for that call position
                                                // If another arg in same call is a head var, propagate type
                                                let _ = lit; // type info used below
                                            }
                                            if let ExprKind::Var(vname) = &ca.kind {
                                                if let Some((_, hi)) =
                                                    head_vars.iter().find(|(n, _)| n == vname)
                                                {
                                                    // Check if any other arg in this call is a literal
                                                    for (oi, oa) in call_args.iter().enumerate() {
                                                        if let ExprKind::Lit(lit) = &oa.kind {
                                                            if param_types[*hi] == "String" {
                                                                // Same call, same function → likely same type domain
                                                                param_types[*hi] =
                                                                    Self::literal_rust_type(lit);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let param_type_strs: Vec<String> = param_types
                        .iter()
                        .map(|t| {
                            if *t == "String" {
                                "&str".to_string()
                            } else {
                                t.to_string()
                            }
                        })
                        .collect();
                    self.types
                        .prolog_rule_fns
                        .insert(fn_name.clone(), param_type_strs);
                }
            }
            // Second pass: propagate types from known Prolog functions to dependent ones
            let known: BTreeMap<String, Vec<String>> = self.types.prolog_rule_fns.clone();
            for (fn_name, rules) in &rule_groups {
                let arity = Self::rule_arity(rules);
                if arity > 0 && Self::rules_have_prolog_features(rules) {
                    if let Some(cur_types) = self.types.prolog_rule_fns.get(fn_name).cloned() {
                        let mut updated = cur_types.clone();
                        for r in rules.iter() {
                            if let Rule::Clause {
                                head,
                                body: Some(body),
                            } = r
                            {
                                if let ExprKind::App(_, head_args) = &head.kind {
                                    let head_vars: Vec<(String, usize)> = head_args
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(i, a)| {
                                            if let ExprKind::Var(name) = &a.kind {
                                                Some((name.clone(), i))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();
                                    let body_calls: Vec<&Expr> = match &body.kind {
                                        ExprKind::Conjunction(goals) => goals.iter().collect(),
                                        _ => vec![body],
                                    };
                                    for call in body_calls {
                                        if let ExprKind::App(func, call_args) = &call.kind {
                                            let called_fn = Self::expr_fn_name(func);
                                            if let Some(called_types) = known.get(&called_fn) {
                                                for (ci, ca) in call_args.iter().enumerate() {
                                                    if let ExprKind::Var(vname) = &ca.kind {
                                                        if let Some((_, hi)) = head_vars
                                                            .iter()
                                                            .find(|(n, _)| n == vname)
                                                        {
                                                            if updated[*hi] == "&str"
                                                                || ci >= called_types.len()
                                                            {
                                                                continue;
                                                            }
                                                            updated[*hi] = called_types[ci].clone();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        self.types.prolog_rule_fns.insert(fn_name.clone(), updated);
                    }
                }
            }

            for (fn_name, rules) in &rule_groups {
                // Count params and track which are borrowed (struct/enum types)
                let param_count = rules
                    .iter()
                    .find_map(|r| {
                        let head = match r {
                            Rule::Clause { head, .. }
                            | Rule::Default { head, .. }
                            | Rule::Exception { head, .. } => head,
                            _ => return None,
                        };
                        if let ExprKind::App(_, args) = &head.kind {
                            Some(
                                args.iter()
                                    .filter(|a| matches!(a.kind, ExprKind::Var(_)))
                                    .count(),
                            )
                        } else {
                            Some(0)
                        }
                    })
                    .unwrap_or(0);
                // Register borrow flags: true for params whose type was inferred from fields
                let borrow_flags: Vec<bool> = {
                    let params: Vec<String> = rules
                        .iter()
                        .find_map(|r| {
                            let head = match r {
                                Rule::Clause { head, .. }
                                | Rule::Default { head, .. }
                                | Rule::Exception { head, .. } => head,
                                _ => return None,
                            };
                            if let ExprKind::App(_, args) = &head.kind {
                                Some(
                                    args.iter()
                                        .filter_map(|a| {
                                            if let ExprKind::Var(n) = &a.kind {
                                                Some(n.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect(),
                                )
                            } else {
                                Some(vec![])
                            }
                        })
                        .unwrap_or_default();
                    params
                        .iter()
                        .map(|p| self.infer_param_type_from_fields(p, rules).is_some())
                        .collect()
                };
                if borrow_flags.iter().any(|b| *b) {
                    self.borrow_only_params
                        .insert(fn_name.clone(), borrow_flags);
                }
                out.push_str(&self.emit_rule_function(fn_name, rules));
                self.types.rule_clone_params.clear();
                out.push('\n');
            }
        }

        // Comptime pass: evaluate @ comptime bindings using the interpreter
        {
            let mut comptime_interp = Interpreter::new();
            let mut comptime_env = comptime_interp.default_env();
            // Register all types, functions, and rules so comptime expressions can call them
            for stmt in stmts {
                match stmt {
                    Stmt::TypeDecl(decl) => {
                        comptime_interp.register_type(decl);
                        comptime_interp.register_constructors(decl, &mut comptime_env);
                    }
                    Stmt::Defn(defn) => {
                        comptime_interp.eval_defn(defn, &mut comptime_env);
                    }
                    Stmt::Rule(rule) => {
                        let name = comptime_interp.rule_name(rule);
                        comptime_interp.rules.push((name, rule.clone()));
                    }
                    Stmt::Bind(pat, _ty, expr) => {
                        // Use step budget to avoid hanging on expensive bindings
                        comptime_interp.step_count = 0;
                        comptime_interp.step_limit = 50_000;
                        comptime_interp.budget_exceeded = false;
                        let val = comptime_interp.eval(expr, &comptime_env);
                        comptime_interp.step_limit = 0;
                        if !comptime_interp.budget_exceeded {
                            comptime_interp.bind_pattern(pat, &val, &mut comptime_env);
                        }
                    }
                    _ => {}
                }
            }
            // Now scan main_stmts for @ comptime + Bind pairs
            let mut comptime_type_decls: Vec<String> = Vec::new();
            let mut is_comptime = false;
            for stmt in &main_stmts {
                if let Stmt::Annot(name, _) = stmt {
                    if name == "comptime" {
                        is_comptime = true;
                        continue;
                    }
                }
                if is_comptime {
                    // Match both Pat::Var (lowercase) and Pat::Con with no args (uppercase type names)
                    let bind_name_expr = match stmt {
                        Stmt::Bind(Pat::Var(name), _, expr) => Some((name.clone(), expr)),
                        Stmt::Bind(Pat::Con(name, args), _, expr) if args.is_empty() => {
                            Some((name.clone(), expr))
                        }
                        _ => None,
                    };
                    if let Some((name, expr)) = bind_name_expr {
                        let val = comptime_interp.eval(expr, &mut comptime_env);
                        // Comptime type: if the value is a TypeDef, generate a type declaration
                        if let Value::TypeDef { kind, fields } = &val {
                            let type_decl = Self::typedef_to_type_decl(&name, kind, fields);
                            eprintln!(
                                "// comptime type: {} ({}, {} fields)",
                                name,
                                kind,
                                fields.len()
                            );
                            // Register type metadata (variant_parent, struct_types, etc.)
                            // before emit_type_decl so struct detection works
                            if let TypeDecl::ADT {
                                name: tname,
                                variants,
                                ..
                            } = &type_decl
                            {
                                for v in variants {
                                    self.types
                                        .variant_parent
                                        .insert(v.name.clone(), tname.clone());
                                    self.types
                                        .variant_positional
                                        .insert(v.name.clone(), v.positional);
                                    if !v.positional {
                                        let names: Vec<String> =
                                            v.fields.iter().map(|f| f.name.clone()).collect();
                                        self.types.variant_fields.insert(v.name.clone(), names);
                                    }
                                    let ft_map: BTreeMap<String, Ty> = v
                                        .fields
                                        .iter()
                                        .map(|f| (f.name.clone(), f.ty.clone()))
                                        .collect();
                                    self.types
                                        .variant_field_types
                                        .insert(v.name.clone(), ft_map);
                                }
                                if variants.len() == 1
                                    && variants[0].name == *tname
                                    && !variants[0].fields.is_empty()
                                {
                                    self.types.struct_types.insert(tname.clone());
                                }
                                let param_names: Vec<String> = vec![];
                                let variant_names: Vec<String> =
                                    variants.iter().map(|v| v.name.clone()).collect();
                                self.types
                                    .type_decls
                                    .insert(tname.clone(), (param_names, variant_names));
                            }
                            let decl_str = self.emit_type_decl(&type_decl);
                            // Insert before main function
                            comptime_type_decls.push(decl_str);
                            // Register constructors in comptime env for later comptime expressions
                            comptime_interp.register_type(&type_decl);
                            comptime_interp.register_constructors(&type_decl, &mut comptime_env);
                            // Mark as comptime with empty value so it doesn't re-emit as a binding
                            self.types
                                .comptime_values
                                .insert(name.clone(), String::new());
                            self.types
                                .comptime_types
                                .insert(name.clone(), String::new());
                        } else {
                            let (rust_lit, rust_ty) =
                                Self::value_to_rust_literal(&val, &self.types.variant_parent);
                            eprintln!("// comptime: {} = {} ({})", name, rust_lit, rust_ty);
                            self.types.comptime_values.insert(name.clone(), rust_lit);
                            self.types.comptime_types.insert(name.clone(), rust_ty);
                        }
                        // Also bind in comptime env so later comptime expressions can use it
                        comptime_interp.bind_pattern(
                            &Pat::Var(name.clone()),
                            &val,
                            &mut comptime_env,
                        );
                    }
                    // @ comptime assert(expr) — compile-time assertion
                    if let Stmt::Expr(expr) = stmt {
                        // Check if it's assert(expr) or just a bare expression
                        let inner_expr = match &expr.kind {
                            ExprKind::App(f, args)
                                if matches!(f.kind, ExprKind::Var(ref n) if n == "assert")
                                    && args.len() == 1 =>
                            {
                                Some(&args[0])
                            }
                            _ => None,
                        };
                        if let Some(assert_expr) = inner_expr {
                            let val = comptime_interp.eval(assert_expr, &mut comptime_env);
                            let is_truthy = match &val {
                                Value::Bool(b) => Some(*b),
                                Value::Constructor(name, args) if args.is_empty() => {
                                    match name.as_str() {
                                        "true" | "True" => Some(true),
                                        "false" | "False" => Some(false),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            };
                            match is_truthy {
                                Some(true) => {
                                    eprintln!("// comptime assert: PASS");
                                }
                                Some(false) => {
                                    eprintln!("comptime assert FAILED: {:?}", assert_expr);
                                    std::process::exit(1);
                                }
                                None => {
                                    eprintln!("comptime assert: expected Bool, got {:?}", val);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            // Bare comptime expression — just evaluate it
                            comptime_interp.eval(expr, &mut comptime_env);
                        }
                    }
                    is_comptime = false;
                }
            }

            // Auto-comptime: pure functions with all-literal/comptime args get evaluated
            // at compile time without requiring explicit @ comptime annotation.
            let pure_fns =
                Self::find_pure_functions(stmts, &self.types.effect_ops, &self.types.fn_effects);
            for stmt in &main_stmts {
                if let Stmt::Bind(Pat::Var(name), _, expr) = stmt {
                    // Skip already-comptime bindings
                    if self.types.comptime_values.contains_key(name) {
                        continue;
                    }
                    if let Some(val) = Self::try_auto_comptime(
                        expr,
                        &pure_fns,
                        &self.types.comptime_values,
                        &mut comptime_interp,
                        &comptime_env,
                    ) {
                        // Skip auto-comptime for None/Err values — can't determine full type
                        let skip_comptime = matches!(&val,
                            Value::Constructor(n, args) if (n == "None" && args.is_empty()) || n == "Err"
                        );
                        if !skip_comptime {
                            let (rust_lit, rust_ty) =
                                Self::value_to_rust_literal(&val, &self.types.variant_parent);
                            // Skip values that can't be represented as Rust literals (closures, actors, etc.)
                            if rust_lit.contains("todo!(\"comptime: unsupported value\")") {
                                eprintln!("// auto-comptime: {} = todo!(\"comptime: unsupported value\") ({})", name, rust_ty);
                            } else {
                                eprintln!(
                                    "// auto-comptime: {} = {} ({})",
                                    name, rust_lit, rust_ty
                                );
                                self.types.comptime_values.insert(name.clone(), rust_lit);
                                self.types.comptime_types.insert(name.clone(), rust_ty);
                            }
                        }
                        comptime_interp.bind_pattern(
                            &Pat::Var(name.clone()),
                            &val,
                            &mut comptime_env,
                        );
                    }
                }
            }
            // Emit comptime-generated type declarations
            for decl_str in &comptime_type_decls {
                out.push_str(decl_str);
                out.push('\n');
            }
        }

        // Emit main function — with escape analysis
        // Count variable uses across all main statements
        self.copy_vars.clear();
        let main_ownership =
            OwnershipAnalysis::analyze_stmt_refs(&main_stmts, &self.borrow_only_params);
        self.var_use_counts = main_ownership.var_uses;
        self.var_consuming_counts = main_ownership.consuming_uses;
        // Detect Copy-type bindings in main (from literal type inference)
        for stmt in &main_stmts {
            if let Stmt::Bind(Pat::Var(name), Some(ty), _) = stmt {
                if is_copy_type(ty) {
                    self.copy_vars.insert(name.clone());
                }
            }
            // Also infer Copy from integer/float/bool literals
            if let Stmt::Bind(Pat::Var(name), None, expr) = stmt {
                if matches!(
                    expr.kind,
                    ExprKind::Lit(Literal::Int(_))
                        | ExprKind::Lit(Literal::Float(_))
                        | ExprKind::Lit(Literal::Bool(_))
                        | ExprKind::Lit(Literal::Char(_))
                ) {
                    self.copy_vars.insert(name.clone());
                }
            }
        }
        // Mark actor message variant names as copy-like (enum constructors, not variables)
        for stmt in stmts {
            if let Stmt::Defn(Defn::Actor { handlers, .. }) = stmt {
                for h in handlers {
                    match &h.msg_pat {
                        Pat::Con(n, _) | Pat::Var(n) => {
                            self.copy_vars.insert(n.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
        // Detect variables rebound inside for loops (need `let mut`)
        self.mutable_vars.clear();
        collect_mutable_vars(&main_stmts, &mut self.mutable_vars);
        // Detect variables passed to inout parameters (need `let mut`)
        self.collect_inout_mutables_stmts(&main_stmts);
        // Independence analysis: track aliased variables (= y = x where x is non-Copy)
        self.aliased_vars = collect_aliased_vars(&main_stmts, &self.copy_vars);
        // Library mode: skip fn main(), just emit exported types and functions
        if !self.lib_mode {
            // Detect if any main statement uses ? operator
            let uses_try = main_stmts.iter().any(|s| stmt_contains_try(s));
            // M13c: async main when subjects or actors are used
            if self.has_async {
                // M15: multi-threaded Tokio — trust the topology.
                // Independent streams (zero Phi) auto-parallelize across CPU cores.
                // Synchronization happens at fan-in nodes (zip, merge).
                if uses_try {
                    out.push_str("#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
                } else {
                    out.push_str("#[tokio::main]\nasync fn main() {\n");
                }
            } else if uses_try {
                out.push_str("fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
            } else {
                out.push_str("fn main() {\n");
            }
            self.indent = 1;

            // M26: Object store — open DB, version check, create tables for stored types
            // DB named per scope: explicit `@ store T in "scope"` or derived from source file stem
            if !self.types.stored_types.is_empty() {
                let db_name = if let Some(ref scope) = self.types.store_scope {
                    format!(".{}.store.db", scope)
                } else if let Some(ref stem) = self.source_name {
                    format!(".{}.store.db", stem)
                } else {
                    ".store.db".to_string()
                };
                let i = self.ind();
                out.push_str(&format!("{i}// M26: Object store initialization\n"));
                out.push_str(&format!(
                    "{i}let __db = std::sync::Arc::new(std::sync::Mutex::new(\n"
                ));
                out.push_str(&format!("{i}    rusqlite::Connection::open(\"{db_name}\").expect(\"Failed to open store database\")\n"));
                out.push_str(&format!("{i}));\n"));
                out.push_str(&format!(
                    "{i}__db.lock().unwrap().execute_batch(\"PRAGMA journal_mode=WAL;\").ok();\n"
                ));
                // Create meta table for schema versioning
                out.push_str(&format!("{i}__db.lock().unwrap().execute(\n"));
                out.push_str(&format!("{i}    \"CREATE TABLE IF NOT EXISTS __store_meta (type_name TEXT PRIMARY KEY, schema_hash TEXT NOT NULL)\",\n"));
                out.push_str(&format!("{i}    rusqlite::params![]\n"));
                out.push_str(&format!("{i}).ok();\n"));
                for type_name in &self.types.stored_types.clone() {
                    let table_name = sanitize_name(type_name).to_lowercase();
                    let hash = self
                        .types
                        .stored_type_schema_hash
                        .get(type_name)
                        .cloned()
                        .unwrap_or_default();
                    let is_dump = self.types.store_delete_on_change.contains(type_name);
                    // Check stored schema hash vs current
                    out.push_str(&format!("{i}{{\n"));
                    out.push_str(&format!("{i}    let __db_lock = __db.lock().unwrap();\n"));
                    out.push_str(&format!(
                        "{i}    let __old_hash: Option<String> = __db_lock.query_row(\n"
                    ));
                    out.push_str(&format!("{i}        \"SELECT schema_hash FROM __store_meta WHERE type_name = ?1\",\n"));
                    out.push_str(&format!("{i}        rusqlite::params![\"{type_name}\"],\n"));
                    out.push_str(&format!("{i}        |row| row.get(0)\n"));
                    out.push_str(&format!("{i}    ).ok();\n"));
                    out.push_str(&format!("{i}    let __new_hash = \"{hash}\";\n"));
                    out.push_str(&format!("{i}    match __old_hash.as_deref() {{\n"));
                    out.push_str(&format!("{i}        Some(h) if h == __new_hash => {{}}\n"));
                    out.push_str(&format!("{i}        Some(_) => {{\n"));
                    out.push_str(&format!("{i}            // Schema changed\n"));
                    if is_dump {
                        // delete_on_change: export data to dump file, then drop table
                        let dump_file = format!(
                            ".{}.dump.runa",
                            self.types
                                .store_scope
                                .as_ref()
                                .or(self.source_name.as_ref())
                                .map(|s| s.as_str())
                                .unwrap_or("store")
                        );
                        out.push_str(&format!("{i}            eprintln!(\"store: {type_name} schema changed — dumping old data to {dump_file}\");\n"));
                        out.push_str(&format!("{i}            let mut __dump = String::new();\n"));
                        out.push_str(&format!("{i}            let mut __stmt = __db_lock.prepare(\"SELECT data FROM {table_name}\").unwrap();\n"));
                        out.push_str(&format!("{i}            let __rows = __stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();\n"));
                        out.push_str(&format!("{i}            for __row in __rows {{ if let Ok(json) = __row {{ __dump.push_str(&format!(\"| {type_name}({{}})\n\", json)); }} }}\n"));
                        out.push_str(&format!("{i}            if !__dump.is_empty() {{ std::fs::write(\"{dump_file}\", &__dump).ok(); }}\n"));
                        out.push_str(&format!("{i}            __db_lock.execute(\"DROP TABLE IF EXISTS {table_name}\", []).ok();\n"));
                    } else {
                        // default: keep data, log that schema changed (serde defaults handle missing fields)
                        out.push_str(&format!("{i}            eprintln!(\"store: {type_name} schema changed — keeping data (new fields get defaults)\");\n"));
                    }
                    out.push_str(&format!("{i}            __db_lock.execute(\n"));
                    out.push_str(&format!("{i}                \"INSERT OR REPLACE INTO __store_meta (type_name, schema_hash) VALUES (?1, ?2)\",\n"));
                    out.push_str(&format!(
                        "{i}                rusqlite::params![\"{type_name}\", __new_hash],\n"
                    ));
                    out.push_str(&format!("{i}            ).ok();\n"));
                    out.push_str(&format!("{i}        }}\n"));
                    out.push_str(&format!("{i}        None => {{\n"));
                    out.push_str(&format!(
                        "{i}            // First run — record schema hash\n"
                    ));
                    out.push_str(&format!("{i}            __db_lock.execute(\n"));
                    out.push_str(&format!("{i}                \"INSERT INTO __store_meta (type_name, schema_hash) VALUES (?1, ?2)\",\n"));
                    out.push_str(&format!(
                        "{i}                rusqlite::params![\"{type_name}\", __new_hash],\n"
                    ));
                    out.push_str(&format!("{i}            ).ok();\n"));
                    out.push_str(&format!("{i}        }}\n"));
                    out.push_str(&format!("{i}    }}\n"));
                    out.push_str(&format!("{i}}}\n"));
                    // Create the data table (after potential DROP)
                    out.push_str(&format!("{i}__db.lock().unwrap().execute(\n"));
                    out.push_str(&format!("{i}    \"CREATE TABLE IF NOT EXISTS {table_name} (id TEXT PRIMARY KEY, data TEXT NOT NULL)\",\n"));
                    out.push_str(&format!("{i}    rusqlite::params![]\n"));
                    out.push_str(&format!(
                        "{i}).expect(\"Failed to create table {table_name}\");\n"
                    ));
                }
                out.push_str("\n");
            }

            let mut skip_next_comptime = false;
            for stmt in &main_stmts {
                if let Stmt::Annot(name, _) = stmt {
                    if name == "comptime" {
                        skip_next_comptime = true;
                        continue; // @ comptime itself emits nothing
                    }
                }
                if skip_next_comptime {
                    skip_next_comptime = false;
                    // Comptime binds are handled via comptime_values in emit_stmt.
                    // Comptime asserts were already evaluated — skip the assert expression.
                    if matches!(stmt, Stmt::Expr(_)) {
                        continue;
                    }
                }
                out.push_str(&self.emit_stmt(stmt));
            }
            if uses_try {
                out.push_str("    Ok(())\n");
            }
            self.indent = 0;
            out.push_str("}\n");
        } else {
            // lib_mode: emit top-level bindings as pub fn getters
            // so they're accessible from other modules.
            // = exported_value = compute_value() → pub fn exported_value() -> T { compute_value() }
            for stmt in &main_stmts {
                if let Stmt::Bind(Pat::Var(name), _ty, expr) = stmt {
                    // Skip @ print and other side-effectful statements
                    if name.starts_with("__") { continue; }
                    let body = self.emit_expr(expr);
                    out.push_str(&format!("pub fn {name}() -> impl Clone {{ {body} }}\n"));
                }
            }
        }

        out
    }

    /// Check if a type recursively references the given ADT name
    fn type_references_adt(&self, ty: &Ty, adt_name: &str) -> bool {
        Self::type_references_adt_static(ty, adt_name)
    }

    fn emit_type_decl(&mut self, decl: &TypeDecl) -> String {
        match decl {
            TypeDecl::ADT {
                name,
                params,
                variants,
                methods,
            } => {
                if variants.is_empty() {
                    return format!("// type {} (opaque)\n", name);
                }
                // Result and Option use Rust's native types — don't emit custom enums
                if name == "Result" || name == "Option" {
                    return String::new();
                }

                let rust_name = self.rust_type_name(name);

                // Use actual param names from Futuruna source, uppercased
                let param_rust_names: Vec<String> = params
                    .iter()
                    .map(|p| {
                        let mut s = p.name.clone();
                        s.make_ascii_uppercase();
                        s
                    })
                    .collect();

                let type_params = if param_rust_names.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", param_rust_names.join(", "))
                };

                // Display trait bound needed for generic params
                let display_bounds = if param_rust_names.is_empty() {
                    String::new()
                } else {
                    let bounds: Vec<String> = param_rust_names
                        .iter()
                        .map(|p| format!("{}: fmt::Display", p))
                        .collect();
                    format!("<{}>", bounds.join(", "))
                };

                let mut out = String::new();
                let is_struct = self.types.struct_types.contains(&rust_name);
                let pub_prefix = if self.types.exported_names.contains(name) {
                    "pub "
                } else {
                    ""
                };
                // Rc/Arc for immutable recursive ADTs (O(1) structural sharing)
                let wrap_name = if self.types.rc_types.contains(&rust_name) {
                    self.rc_name()
                } else {
                    "Box"
                };

                if is_struct {
                    // Single-variant with same name → emit Rust struct
                    let v = &variants[0];
                    // Derive Default only if all fields have types that implement Default.
                    // Enums whose first variant has fields don't get Default — check field types.
                    let all_fields_defaultable = v.fields.iter().all(|f| {
                        let ty_str = self.emit_type_with_params(&f.ty, params);
                        // Primitive types, Vec, Option, HashMap, HashSet all have Default
                        // Enum types without Default don't — check if it's a non-struct user type
                        let base_name = ty_str.split('<').next().unwrap_or(&ty_str).trim();
                        matches!(base_name, "i64" | "f64" | "String" | "bool" | "char" | "()"
                            | "Vec" | "Option" | "HashMap" | "HashSet" | "Rc" | "Arc")
                            || self.types.struct_types.contains(base_name)
                            // Enums with fieldless first variant have Default
                            || self.types.type_decls.get(base_name).map_or(false, |(_, vnames)| {
                                vnames.first().map_or(false, |vn| {
                                    self.types.variant_field_types.get(vn).map_or(true, |ft| ft.is_empty())
                                })
                            })
                    });
                    let serde_derives = if self.types.stored_types.contains(name) {
                        ", serde::Serialize, serde::Deserialize"
                    } else {
                        ""
                    };
                    if all_fields_defaultable {
                        out.push_str(&format!(
                            "#[derive(Debug, Clone, PartialEq, Default{})]\n",
                            serde_derives
                        ));
                    } else {
                        out.push_str(&format!(
                            "#[derive(Debug, Clone, PartialEq{})]\n",
                            serde_derives
                        ));
                    }
                    // For stored types, allow missing fields during deserialization (schema flex)
                    if self.types.stored_types.contains(name) {
                        out.push_str("#[serde(default)]\n");
                    }
                    if v.positional {
                        // Tuple struct: struct Point(f64, f64);
                        let fields_str: Vec<String> = v
                            .fields
                            .iter()
                            .map(|f| {
                                let base = self.emit_type_with_params(&f.ty, params);
                                if self.type_references_adt(&f.ty, name) {
                                    format!("pub {}<{}>", wrap_name, base)
                                } else {
                                    format!("pub {}", base)
                                }
                            })
                            .collect();
                        out.push_str(&format!(
                            "{}struct {}{}({});\n",
                            pub_prefix,
                            rust_name,
                            type_params,
                            fields_str.join(", ")
                        ));
                    } else {
                        // Named struct: struct Point { pub x: f64, pub y: f64 }
                        out.push_str(&format!(
                            "{}struct {}{} {{\n",
                            pub_prefix, rust_name, type_params
                        ));
                        for f in &v.fields {
                            let base = self.emit_type_with_params(&f.ty, params);
                            let ty_str = if self.type_references_adt(&f.ty, name) {
                                format!("{}<{}>", wrap_name, base)
                            } else {
                                base
                            };
                            out.push_str(&format!("    pub {}: {},\n", f.name, ty_str));
                        }
                        out.push_str("}\n");
                    }
                } else {
                    // Multi-variant → emit Rust enum
                    // Derive Default only if first variant is fieldless (can use #[default])
                    let first_variant_fieldless =
                        variants.first().map_or(false, |v| v.fields.is_empty());
                    if first_variant_fieldless {
                        out.push_str("#[derive(Debug, Clone, PartialEq, Default)]\n");
                    } else {
                        out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
                    }
                    out.push_str(&format!(
                        "{}enum {}{} {{\n",
                        pub_prefix, rust_name, type_params
                    ));
                    for (vi, v) in variants.iter().enumerate() {
                        let default_attr = if vi == 0 && first_variant_fieldless {
                            "    #[default]\n"
                        } else {
                            ""
                        };
                        if v.fields.is_empty() {
                            out.push_str(&format!("{}    {},\n", default_attr, v.name));
                        } else if v.positional {
                            // Positional → Rust tuple variant
                            let fields_str: Vec<String> = v
                                .fields
                                .iter()
                                .map(|f| {
                                    let base = self.emit_type_with_params(&f.ty, params);
                                    if self.type_references_adt(&f.ty, name) {
                                        format!("{}<{}>", wrap_name, base)
                                    } else {
                                        base
                                    }
                                })
                                .collect();
                            out.push_str(&format!("    {}({}),\n", v.name, fields_str.join(", ")));
                        } else {
                            // Named → Rust struct variant
                            out.push_str(&format!("    {} {{\n", v.name));
                            for f in &v.fields {
                                let base = self.emit_type_with_params(&f.ty, params);
                                let ty_str = if self.type_references_adt(&f.ty, name) {
                                    format!("{}<{}>", wrap_name, base)
                                } else {
                                    base
                                };
                                out.push_str(&format!("        {}: {},\n", f.name, ty_str));
                            }
                            out.push_str("    },\n");
                        }
                    }
                    out.push_str("}\n");
                }

                // Helper: check if a field type needs {:?} format (Vec, Option, HashMap, HashSet)
                let field_needs_debug_fmt = |f: &Field| -> bool {
                    let ty_str = self.emit_type_with_params(&f.ty, params);
                    let base = ty_str.split('<').next().unwrap_or(&ty_str).trim();
                    matches!(
                        base,
                        "Vec" | "Option" | "HashMap" | "HashSet" | "Rc" | "Arc"
                    )
                };

                // Impl Display (skip if user provided explicit # impl fmt::Display)
                if self.types.explicit_display_impls.contains(name) {
                    // User provides their own Display impl
                } else if is_struct {
                    let v = &variants[0];
                    out.push_str(&format!(
                        "\nimpl{} fmt::Display for {}{} {{\n",
                        display_bounds, rust_name, type_params
                    ));
                    out.push_str(
                        "    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n",
                    );
                    out.push_str(&format!("        write!(f, \"{}(\")?;\n", name));
                    for (i, field) in v.fields.iter().enumerate() {
                        if i > 0 {
                            out.push_str("        write!(f, \", \")?;\n");
                        }
                        let fmt_spec = if field_needs_debug_fmt(field) {
                            "{:?}"
                        } else {
                            "{}"
                        };
                        if v.positional {
                            out.push_str(&format!(
                                "        write!(f, \"{}\", self.{})?;\n",
                                fmt_spec, i
                            ));
                        } else {
                            out.push_str(&format!(
                                "        write!(f, \"{}: {}\", self.{})?;\n",
                                field.name, fmt_spec, field.name
                            ));
                        }
                    }
                    out.push_str("        write!(f, \")\")\n");
                    out.push_str("    }\n");
                    out.push_str("}\n");
                } else {
                    out.push_str(&format!(
                        "\nimpl{} fmt::Display for {}{} {{\n",
                        display_bounds, rust_name, type_params
                    ));
                    out.push_str(
                        "    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n",
                    );
                    out.push_str("        match self {\n");
                    for v in variants {
                        if v.fields.is_empty() {
                            out.push_str(&format!(
                                "            {}::{} => write!(f, \"{}\"),\n",
                                rust_name, v.name, v.name
                            ));
                        } else if v.positional {
                            // Tuple variant Display
                            let binds: Vec<String> =
                                (0..v.fields.len()).map(|i| format!("f{}", i)).collect();
                            out.push_str(&format!(
                                "            {}::{}({}) => {{\n",
                                rust_name,
                                v.name,
                                binds.join(", ")
                            ));
                            out.push_str(&format!(
                                "                write!(f, \"{}(\")?;\n",
                                v.name
                            ));
                            for (i, b) in binds.iter().enumerate() {
                                if i > 0 {
                                    out.push_str("                write!(f, \", \")?;\n");
                                }
                                if field_needs_debug_fmt(&v.fields[i]) {
                                    out.push_str(&format!(
                                        "                write!(f, \"{{:?}}\", {})?;\n",
                                        b
                                    ));
                                } else {
                                    out.push_str(&format!(
                                        "                write!(f, \"{{}}\", {})?;\n",
                                        b
                                    ));
                                }
                            }
                            out.push_str("                write!(f, \")\")\n");
                            out.push_str("            }\n");
                        } else {
                            // Struct variant Display
                            let binds: Vec<String> =
                                v.fields.iter().map(|f| f.name.clone()).collect();
                            out.push_str(&format!(
                                "            {}::{} {{ {} }} => {{\n",
                                rust_name,
                                v.name,
                                binds.join(", ")
                            ));
                            out.push_str(&format!(
                                "                write!(f, \"{}(\")?;\n",
                                v.name
                            ));
                            for (i, b) in binds.iter().enumerate() {
                                if i > 0 {
                                    out.push_str("                write!(f, \", \")?;\n");
                                }
                                if field_needs_debug_fmt(&v.fields[i]) {
                                    out.push_str(&format!(
                                        "                write!(f, \"{}: {{:?}}\", {})?;\n",
                                        b, b
                                    ));
                                } else {
                                    out.push_str(&format!(
                                        "                write!(f, \"{}: {{}}\", {})?;\n",
                                        b, b
                                    ));
                                }
                            }
                            out.push_str("                write!(f, \")\")\n");
                            out.push_str("            }\n");
                        }
                    }
                    out.push_str("        }\n");
                    out.push_str("    }\n");
                    out.push_str("}\n");
                }

                // Emit methods as standalone functions (not in impl block)
                // In Futuruna, methods defined in ADT blocks are callable by name: name(Red)
                // First param without type annotation gets the parent ADT type
                if !methods.is_empty() {
                    for method in methods {
                        if let Defn::Fn {
                            name: mname,
                            params: mparams,
                            ret_ty,
                            body,
                            ..
                        } = method
                        {
                            let self_type = format!("{}{}", rust_name, type_params);
                            // Fill in missing type for first param (the ADT type) so borrow analysis works
                            let augmented_params: Vec<Param> = mparams
                                .iter()
                                .enumerate()
                                .map(|(i, p)| {
                                    if i == 0 && p.ty.is_none() {
                                        Param {
                                            name: p.name.clone(),
                                            ty: Some(Ty::Name(name.clone())),
                                            inout: p.inout,
                                        }
                                    } else {
                                        p.clone()
                                    }
                                })
                                .collect();
                            // Borrow inference: analyze if params are read-only
                            let borrow_flags = analyze_borrow_only_params(
                                &augmented_params,
                                body,
                                ret_ty.as_ref(),
                                &self.borrow_only_params,
                            );
                            if borrow_flags.iter().any(|f| *f) {
                                self.borrow_only_params
                                    .insert(mname.clone(), borrow_flags.clone());
                            }
                            let rust_params: Vec<String> = mparams
                                .iter()
                                .enumerate()
                                .map(|(i, p)| {
                                    if p.name == "self" {
                                        format!("self_: &{}", self_type)
                                    } else if i == 0 && p.ty.is_none() {
                                        // First param without type = the ADT itself
                                        let borrow = borrow_flags.get(i).copied().unwrap_or(false);
                                        if borrow {
                                            format!("{}: &{}", sanitize_name(&p.name), self_type)
                                        } else {
                                            format!("{}: {}", sanitize_name(&p.name), self_type)
                                        }
                                    } else {
                                        let ty =
                                            p.ty.as_ref()
                                                .map(|t| self.emit_type(t))
                                                .unwrap_or_else(|| "String".into());
                                        let borrow = borrow_flags.get(i).copied().unwrap_or(false);
                                        if borrow {
                                            format!("{}: &{}", sanitize_name(&p.name), ty)
                                        } else {
                                            format!("{}: {}", sanitize_name(&p.name), ty)
                                        }
                                    }
                                })
                                .collect();
                            let ret = ret_ty
                                .as_ref()
                                .map(|t| format!(" -> {}", self.emit_type(t)))
                                .unwrap_or_default();
                            out.push_str(&format!(
                                "fn {}({}){} {{\n",
                                sanitize_name(mname),
                                rust_params.join(", "),
                                ret
                            ));
                            // Emit actual method body (escape analysis for methods)
                            let prev_counts = std::mem::take(&mut self.var_use_counts);
                            let prev_consuming = std::mem::take(&mut self.var_consuming_counts);
                            let prev_copy = std::mem::take(&mut self.copy_vars);
                            let ownership = OwnershipAnalysis::analyze_simple(body);
                            self.var_use_counts = ownership.var_uses;
                            self.var_consuming_counts = ownership.consuming_uses;
                            let saved_indent = self.indent;
                            self.indent = 1;
                            out.push_str(&self.emit_expr_as_return(body));
                            self.indent = saved_indent;
                            self.var_use_counts = prev_counts;
                            self.var_consuming_counts = prev_consuming;
                            self.copy_vars = prev_copy;
                            out.push_str("}\n\n");
                        }
                    }
                }

                out
            }
            TypeDecl::EffectDecl { name, ops } => {
                // Emit effect as a trait — each operation becomes a method
                let mut out = format!("trait {} {{\n", name);
                for (op_name, params, ret_ty) in ops {
                    let rust_params: Vec<String> = params
                        .iter()
                        .map(|p| {
                            let ty =
                                p.ty.as_ref()
                                    .map(|t| self.emit_type(t))
                                    .unwrap_or_else(|| "String".into());
                            format!("{}: {}", sanitize_name(&p.name), ty)
                        })
                        .collect();
                    let ret = ret_ty
                        .as_ref()
                        .map(|t| format!(" -> {}", self.emit_type(t)))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "    fn {}(&mut self, {}){};",
                        op_name,
                        rust_params.join(", "),
                        ret
                    ));
                    out.push('\n');
                }
                out.push_str("}\n");
                out
            }
            TypeDecl::TraitDecl {
                name,
                params,
                methods,
            } => {
                let type_params = if params.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = params
                        .iter()
                        .map(|p| {
                            let mut s = p.name.clone();
                            s.make_ascii_uppercase();
                            s
                        })
                        .collect();
                    format!("<{}>", names.join(", "))
                };
                let mut out = format!("trait {}{} {{\n", name, type_params);
                for m in methods {
                    let rust_params: Vec<String> = m
                        .params
                        .iter()
                        .map(|p| {
                            if p.name == "self" {
                                "&self".to_string()
                            } else {
                                let ty =
                                    p.ty.as_ref()
                                        .map(|t| self.emit_type(t))
                                        .unwrap_or_else(|| "String".into());
                                format!("{}: {}", p.name, ty)
                            }
                        })
                        .collect();
                    let ret = m
                        .ret_ty
                        .as_ref()
                        .map(|t| format!(" -> {}", self.emit_type(t)))
                        .unwrap_or_default();
                    if let Some(body) = &m.default_body {
                        out.push_str(&format!(
                            "    fn {}({}){} {{\n",
                            m.name,
                            rust_params.join(", "),
                            ret
                        ));
                        let saved_indent = self.indent;
                        self.indent = 2;
                        // Rewrite fn_name(self) → self.fn_name() in trait default bodies
                        let rewritten = Self::rewrite_self_calls(body);
                        let prev_in_self = self.in_self_method;
                        self.in_self_method = true;
                        out.push_str(&self.emit_expr_as_return(&rewritten));
                        self.in_self_method = prev_in_self;
                        self.indent = saved_indent;
                        out.push_str("    }\n\n");
                    } else {
                        out.push_str(&format!(
                            "    fn {}({}){};\n",
                            m.name,
                            rust_params.join(", "),
                            ret
                        ));
                    }
                }
                out.push_str("}\n");
                out
            }
            TypeDecl::ImplBlock {
                trait_name,
                for_type,
                methods,
            } => {
                let rust_type = self.rust_type_name(for_type);
                let mut out = String::new();
                // Separate methods: self-methods go into trait impl, others become standalone functions
                let mut trait_methods = Vec::new();
                let mut standalone_methods = Vec::new();
                for method in methods {
                    if let Defn::Fn { params, .. } = method {
                        if params.iter().any(|p| p.name == "self") {
                            trait_methods.push(method);
                        } else {
                            standalone_methods.push(method);
                        }
                    }
                }
                // Emit proper Rust trait impl for self-methods
                if !trait_methods.is_empty() {
                    out.push_str(&format!("impl {} for {} {{\n", trait_name, rust_type));
                    for method in trait_methods {
                        if let Defn::Fn {
                            name: mname,
                            params: mparams,
                            ret_ty,
                            body,
                            ..
                        } = method
                        {
                            let rust_params: Vec<String> = mparams
                                .iter()
                                .map(|p| {
                                    if p.name == "self" {
                                        "&self".to_string()
                                    } else {
                                        let ty =
                                            p.ty.as_ref()
                                                .map(|t| self.emit_type(t))
                                                .unwrap_or_else(|| "String".into());
                                        format!("{}: {}", p.name, ty)
                                    }
                                })
                                .collect();
                            let ret = ret_ty
                                .as_ref()
                                .map(|t| format!(" -> {}", self.emit_type(t)))
                                .unwrap_or_default();
                            out.push_str(&format!(
                                "    fn {}({}){} {{\n",
                                mname,
                                rust_params.join(", "),
                                ret
                            ));
                            let prev_counts = std::mem::take(&mut self.var_use_counts);
                            let prev_consuming = std::mem::take(&mut self.var_consuming_counts);
                            let prev_copy = std::mem::take(&mut self.copy_vars);
                            let ownership = OwnershipAnalysis::analyze_simple(body);
                            self.var_use_counts = ownership.var_uses;
                            self.var_consuming_counts = ownership.consuming_uses;
                            let saved_indent = self.indent;
                            self.indent = 2;
                            let prev_in_self = self.in_self_method;
                            self.in_self_method = true;
                            out.push_str(&self.emit_expr_as_return(body));
                            self.in_self_method = prev_in_self;
                            self.indent = saved_indent;
                            self.var_use_counts = prev_counts;
                            self.var_consuming_counts = prev_consuming;
                            self.copy_vars = prev_copy;
                            out.push_str("    }\n\n");
                        }
                    }
                    out.push_str("}\n");
                }
                // Emit standalone functions for non-self methods
                for method in standalone_methods {
                    if let Defn::Fn {
                        name: mname,
                        params: mparams,
                        ret_ty,
                        body,
                        ..
                    } = method
                    {
                        // Fill in missing type for first param so borrow analysis works
                        let augmented_params: Vec<Param> = mparams
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                if i == 0 && p.ty.is_none() {
                                    Param {
                                        name: p.name.clone(),
                                        ty: Some(Ty::Name(for_type.clone())),
                                        inout: p.inout,
                                    }
                                } else {
                                    p.clone()
                                }
                            })
                            .collect();
                        // Borrow inference
                        let borrow_flags = analyze_borrow_only_params(
                            &augmented_params,
                            body,
                            ret_ty.as_ref(),
                            &self.borrow_only_params,
                        );
                        if borrow_flags.iter().any(|f| *f) {
                            self.borrow_only_params
                                .insert(mname.clone(), borrow_flags.clone());
                        }
                        let rust_params: Vec<String> = mparams
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                let borrow = borrow_flags.get(i).copied().unwrap_or(false);
                                if i == 0 && p.ty.is_none() {
                                    if borrow {
                                        format!("{}: &{}", sanitize_name(&p.name), rust_type)
                                    } else {
                                        format!("{}: {}", sanitize_name(&p.name), rust_type)
                                    }
                                } else {
                                    let ty =
                                        p.ty.as_ref()
                                            .map(|t| self.emit_type(t))
                                            .unwrap_or_else(|| "String".into());
                                    if borrow {
                                        format!("{}: &{}", sanitize_name(&p.name), ty)
                                    } else {
                                        format!("{}: {}", sanitize_name(&p.name), ty)
                                    }
                                }
                            })
                            .collect();
                        let ret = ret_ty
                            .as_ref()
                            .map(|t| format!(" -> {}", self.emit_type(t)))
                            .unwrap_or_default();
                        out.push_str(&format!(
                            "fn {}({}){} {{\n",
                            sanitize_name(mname),
                            rust_params.join(", "),
                            ret
                        ));
                        let prev_counts = std::mem::take(&mut self.var_use_counts);
                        let prev_consuming = std::mem::take(&mut self.var_consuming_counts);
                        let prev_copy = std::mem::take(&mut self.copy_vars);
                        let ownership = OwnershipAnalysis::analyze_simple(body);
                        self.var_use_counts = ownership.var_uses;
                        self.var_consuming_counts = ownership.consuming_uses;
                        let saved_indent = self.indent;
                        self.indent = 1;
                        out.push_str(&self.emit_expr_as_return(body));
                        self.indent = saved_indent;
                        self.var_use_counts = prev_counts;
                        self.var_consuming_counts = prev_consuming;
                        self.copy_vars = prev_copy;
                        out.push_str("}\n\n");
                    }
                }
                out
            }
        }
    }

    /// Emit a type, mapping Futuruna type params to their uppercased Rust equivalents
    fn emit_type_with_params(&self, ty: &Ty, params: &[Param]) -> String {
        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        match ty {
            Ty::Name(n) if param_names.contains(n) => n.to_uppercase(),
            Ty::Var(n) if param_names.contains(n) => n.to_uppercase(),
            _ => self.emit_type(ty),
        }
    }

    fn emit_type(&self, ty: &Ty) -> String {
        match ty {
            Ty::Name(n) => match n.as_str() {
                "Int" => "i64".to_string(),
                "Float" => "f64".to_string(),
                "String" => "String".to_string(),
                "Char" => "char".to_string(),
                "Bool" => "bool".to_string(),
                "Nat" => "u64".to_string(),
                _ => {
                    // Qualified paths (fmt::Display) pass through unchanged
                    if n.contains("::") {
                        n.clone()
                    // Single lowercase letter → type variable (uppercase it)
                    } else if n.len() == 1 && n.chars().next().unwrap().is_lowercase() {
                        n.to_uppercase()
                    } else {
                        self.rust_type_name(n)
                    }
                }
            },
            Ty::App(con, args) => {
                let con_str = self.emit_type(con);
                let args_str: Vec<String> = args.iter().map(|a| self.emit_type(a)).collect();
                // Only map List→Vec if List is NOT a user-defined ADT
                if con_str == "List" && !self.types.type_decls.contains_key("List") {
                    format!("Vec<{}>", args_str.first().unwrap_or(&"()".to_string()))
                } else if con_str == "Map" && !self.types.type_decls.contains_key("Map") {
                    format!("HashMap<{}>", args_str.join(", "))
                } else if con_str == "Set" && !self.types.type_decls.contains_key("Set") {
                    format!("HashSet<{}>", args_str.first().unwrap_or(&"()".to_string()))
                } else {
                    format!("{}<{}>", con_str, args_str.join(", "))
                }
            }
            Ty::Arrow(from, to) => {
                // Uncurry: a -> b -> c emits as impl FnMut(A, B) -> C, not nested impls
                // Skip Unit on left side: () -> T means zero-arg function
                let mut params: Vec<String> = if matches!(from.as_ref(), Ty::Unit) {
                    Vec::new()
                } else {
                    vec![self.emit_type(from)]
                };
                let mut current = to.as_ref();
                while let Ty::Arrow(inner_from, inner_to) = current {
                    if !matches!(inner_from.as_ref(), Ty::Unit) {
                        params.push(self.emit_type(inner_from));
                    }
                    current = inner_to.as_ref();
                }
                let ret = self.emit_type(current);
                let trait_name = if self.fn_once_mode { "FnOnce" } else { "FnMut" };
                // Add + Clone so closure values support .clone() (Futuruna: all values are cloneable)
                format!(
                    "impl {}({}) -> {} + Clone",
                    trait_name,
                    params.join(", "),
                    ret
                )
            }
            Ty::Ref(inner) => format!("&{}", self.emit_type(inner)),
            Ty::MutRef(inner) => format!("&mut {}", self.emit_type(inner)),
            Ty::Shared(inner) => format!("std::sync::Arc<{}>", self.emit_type(inner)),
            Ty::Optional(inner) => format!(
                "{}<{}>",
                self.rust_type_name("Option"),
                self.emit_type(inner)
            ),
            Ty::Var(n) => n.to_uppercase(),
            Ty::Unit => "()".to_string(),
            Ty::Hole => "_".to_string(),
        }
    }

    /// Check if a Futuruna type is directly compatible with wasm-bindgen.
    /// Supported: Int, Float, String, Bool, (), Option(primitive), Vec(primitive)
    fn is_wasm_compatible_type(ty: &Ty) -> bool {
        match ty {
            Ty::Name(n) => matches!(
                n.as_str(),
                "Int" | "Float" | "String" | "Bool" | "Char" | "Nat"
            ),
            Ty::Unit => true,
            Ty::App(con, args) if args.len() == 1 => {
                if let Ty::Name(n) = con.as_ref() {
                    match n.as_str() {
                        // wasm-bindgen supports Option<primitive> and Vec<numeric>
                        "Option" => Self::is_wasm_compatible_type(&args[0]),
                        "List" => {
                            matches!(&args[0], Ty::Name(inner) if matches!(inner.as_str(), "Int" | "Float" | "Nat"))
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Collect free type variables from a type expression
    fn collect_type_vars(&self, ty: &Ty, vars: &mut Vec<String>) {
        match ty {
            Ty::Var(n) => {
                let upper = n.to_uppercase();
                if !vars.contains(&upper) {
                    vars.push(upper);
                }
            }
            Ty::Name(n) => {
                // Single lowercase letter names are type vars in Futuruna
                if n.len() == 1 && n.chars().next().unwrap().is_lowercase() {
                    let upper = n.to_uppercase();
                    if !vars.contains(&upper) {
                        vars.push(upper);
                    }
                }
            }
            Ty::App(con, args) => {
                self.collect_type_vars(con, vars);
                for a in args {
                    self.collect_type_vars(a, vars);
                }
            }
            Ty::Arrow(from, to) => {
                self.collect_type_vars(from, vars);
                self.collect_type_vars(to, vars);
            }
            Ty::Ref(inner) | Ty::MutRef(inner) | Ty::Shared(inner) | Ty::Optional(inner) => {
                self.collect_type_vars(inner, vars)
            }
            _ => {}
        }
    }

    /// Check if any clause in a rule group has Prolog-style features:
    /// ground terms in heads, or conjunction bodies
    fn rules_have_prolog_features(rules: &[&Rule]) -> bool {
        rules.iter().any(|r| {
            if let Rule::Clause { head, body } = r {
                let has_ground = if let ExprKind::App(_, args) = &head.kind {
                    args.iter().any(|a| matches!(a.kind, ExprKind::Lit(_)))
                } else {
                    false
                };
                let has_conjunction = body
                    .as_ref()
                    .map_or(false, |b| matches!(b.kind, ExprKind::Conjunction(_)));
                // Body calls with literal args (e.g., has_children(p) -> parent(p, "bob"))
                let has_body_literals = match body {
                    Some(Expr {
                        kind: ExprKind::App(_, args),
                        ..
                    }) => args.iter().any(|a| matches!(a.kind, ExprKind::Lit(_))),
                    Some(Expr {
                        kind: ExprKind::Conjunction(goals),
                        ..
                    }) => goals.iter().any(|g| {
                        if let ExprKind::App(_, args) = &g.kind {
                            args.iter().any(|a| matches!(a.kind, ExprKind::Lit(_)))
                        } else {
                            false
                        }
                    }),
                    _ => false,
                };
                has_ground || has_conjunction || has_body_literals
            } else {
                false
            }
        })
    }

    /// Determine if a Prolog rule group returns values (not just bool).
    /// Returns Some(rust_type) if bodies are non-boolean literals/expressions,
    /// None if the group is bool-returning (facts, conjunctions, comparisons).
    fn prolog_rules_value_type(rules: &[&Rule]) -> Option<String> {
        for r in rules {
            if let Rule::Clause {
                body: Some(body), ..
            } = r
            {
                match &body.kind {
                    ExprKind::Lit(lit) => match lit {
                        Literal::Str(_) => return Some("String".to_string()),
                        Literal::Int(_) => return Some("i64".to_string()),
                        Literal::Float(_) => return Some("f64".to_string()),
                        Literal::Char(_) => return Some("char".to_string()),
                        Literal::Bool(_) => {} // bool body → bool-returning
                    },
                    ExprKind::App(func, _) => {
                        // Constructor call → return the type name
                        if let ExprKind::Var(name) = &func.as_ref().kind {
                            if name
                                .chars()
                                .next()
                                .map(|c| c.is_uppercase())
                                .unwrap_or(false)
                            {
                                return Some(name.clone());
                            }
                        }
                    }
                    // Conjunction, comparison, etc. → bool
                    _ => {}
                }
            }
        }
        None
    }

    /// Get the arity of a rule group (max arg count across all heads)
    fn rule_arity(rules: &[&Rule]) -> usize {
        rules
            .iter()
            .filter_map(|r| {
                let head = match r {
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. } => head,
                    _ => return None,
                };
                if let ExprKind::App(_, args) = &head.kind {
                    Some(args.len())
                } else {
                    Some(0)
                }
            })
            .max()
            .unwrap_or(0)
    }

    /// Emit a Rust literal from a Futuruna Literal
    fn emit_literal_value(lit: &Literal) -> String {
        match lit {
            Literal::Str(s) => format!("\"{}\"", s),
            Literal::Int(n) => format!("{}", n),
            Literal::Float(f) => format!("{:.1}", f),
            Literal::Bool(b) => format!("{}", b),
            Literal::Char(c) => format!("'{}'", c),
        }
    }

    /// Infer the Rust type from a ground term literal
    fn literal_rust_type(lit: &Literal) -> &'static str {
        match lit {
            Literal::Int(_) => "i64",
            Literal::Float(_) => "f64",
            Literal::Str(_) => "String",
            Literal::Bool(_) => "bool",
            Literal::Char(_) => "char",
        }
    }

    /// Get function name from an Expr (non-recursive through App)
    fn expr_fn_name(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::App(f, _) => Self::expr_fn_name(f),
            ExprKind::Var(name) => name.clone(),
            _ => "?".into(),
        }
    }

    /// Emit an expression, but string literals stay as &str (no .to_string()) when target is a Prolog fn
    fn emit_prolog_arg(&mut self, a: &Expr) -> String {
        if let ExprKind::Lit(Literal::Str(s)) = &a.kind {
            format!("{:?}", s) // &str literal, no .to_string()
        } else {
            self.emit_expr(a)
        }
    }

    /// Emit Prolog-style rule function with fact table + backtracking search
    fn emit_prolog_rule_function(
        &mut self,
        fn_name: &str,
        rules: &[&Rule],
        arity: usize,
    ) -> String {
        let mut out = String::new();
        let sanitized = sanitize_name(fn_name);

        // Determine param types from ground terms in any clause head
        let mut param_types: Vec<&str> = vec!["String"; arity];
        for r in rules {
            if let Rule::Clause { head, .. } = r {
                if let ExprKind::App(_, args) = &head.kind {
                    for (i, arg) in args.iter().enumerate() {
                        if let ExprKind::Lit(lit) = &arg.kind {
                            param_types[i] = Self::literal_rust_type(lit);
                        }
                    }
                }
            }
        }

        // Register this function as Prolog-style so call sites can emit correct types
        let param_type_strs: Vec<String> = param_types
            .iter()
            .map(|t| {
                if *t == "String" {
                    "&str".to_string()
                } else {
                    t.to_string()
                }
            })
            .collect();
        self.types
            .prolog_rule_fns
            .insert(fn_name.to_string(), param_type_strs);

        // Value-returning Prolog rules: emit Option<T> instead of bool
        if let Some(value_type_str) = Self::prolog_rules_value_type(rules) {
            self.types
                .prolog_value_fns
                .insert(fn_name.to_string(), value_type_str.clone());
            return self.emit_prolog_value_function(
                &sanitized,
                fn_name,
                rules,
                arity,
                &param_types,
                &value_type_str,
            );
        }

        // Collect bare facts (clauses with no body, all-literal heads)
        let facts: Vec<Vec<String>> = rules
            .iter()
            .filter_map(|r| {
                if let Rule::Clause { head, body: None } = r {
                    if let ExprKind::App(_, args) = &head.kind {
                        if args.iter().all(|a| matches!(a.kind, ExprKind::Lit(_))) {
                            let vals: Vec<String> = args
                                .iter()
                                .map(|a| {
                                    if let ExprKind::Lit(lit) = &a.kind {
                                        Self::emit_literal_value(lit)
                                    } else {
                                        "?".into()
                                    }
                                })
                                .collect();
                            return Some(vals);
                        }
                    }
                }
                None
            })
            .collect();

        // Emit fact table
        let table_name = format!("{}_FACTS", sanitized.to_uppercase());
        if !facts.is_empty() {
            if arity == 1 {
                let ty = if param_types[0] == "String" {
                    "&str"
                } else {
                    param_types[0]
                };
                out.push_str(&format!("const {}: &[{}] = &[", table_name, ty));
                let vals: Vec<String> = facts.iter().map(|f| f[0].clone()).collect();
                out.push_str(&vals.join(", "));
                out.push_str("];\n\n");
            } else {
                let types: Vec<&str> = param_types
                    .iter()
                    .map(|t| if *t == "String" { "&str" } else { t })
                    .collect();
                out.push_str(&format!(
                    "const {}: &[({},)] = &[\n",
                    table_name,
                    types.join(", ")
                ));
                for fact in &facts {
                    out.push_str(&format!("    ({}),\n", fact.join(", ")));
                }
                out.push_str("];\n\n");
            }
        }

        // Function signature
        let param_names: Vec<String> = (0..arity).map(|i| format!("_p{}", i)).collect();
        let param_str: String = param_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if param_types[i] == "String" {
                    format!("{}: &str", name)
                } else {
                    format!("{}: {}", name, param_types[i])
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("fn {}({}) -> bool {{\n", sanitized, param_str));

        // Emit rules with bodies (conjunction / backtracking)
        for r in rules {
            if let Rule::Clause {
                head,
                body: Some(body),
            } = r
            {
                if let ExprKind::App(_, head_args) = &head.kind {
                    // Map head variable names to parameter positions
                    let head_vars: Vec<(String, usize)> = head_args
                        .iter()
                        .enumerate()
                        .filter_map(|(i, a)| {
                            if let ExprKind::Var(name) = &a.kind {
                                Some((name.clone(), i))
                            } else {
                                None
                            }
                        })
                        .collect();

                    if let ExprKind::Conjunction(goals) = &body.kind {
                        // Find existential variables (in goals but not in head)
                        let head_var_names: std::collections::BTreeSet<String> =
                            head_vars.iter().map(|(n, _)| n.clone()).collect();

                        let has_existential = goals.iter().any(|g| {
                            if let ExprKind::App(_, gargs) = &g.kind {
                                gargs.iter().any(|a| {
                                    if let ExprKind::Var(name) = &a.kind {
                                        !head_var_names.contains(name)
                                    } else {
                                        false
                                    }
                                })
                            } else {
                                false
                            }
                        });

                        if has_existential {
                            // Existential search: iterate over fact table of the first goal
                            let first_goal = &goals[0];
                            if let ExprKind::App(func, goal_args) = &first_goal.kind {
                                let goal_fn = Self::expr_fn_name(func);
                                let source_table =
                                    format!("{}_FACTS", sanitize_name(&goal_fn).to_uppercase());

                                out.push_str(&format!(
                                    "    for fact in {}.iter() {{\n",
                                    source_table
                                ));
                                for (gi, ga) in goal_args.iter().enumerate() {
                                    if let ExprKind::Var(name) = &ga.kind {
                                        if let Some((_, idx)) =
                                            head_vars.iter().find(|(n, _)| n == name)
                                        {
                                            // Bound from head — check match
                                            out.push_str(&format!(
                                                "        if fact.{} != {} {{ continue; }}\n",
                                                gi, param_names[*idx]
                                            ));
                                        } else {
                                            // Existential — bind
                                            out.push_str(&format!(
                                                "        let {} = fact.{};\n",
                                                sanitize_name(name),
                                                gi
                                            ));
                                        }
                                    }
                                }
                                // Check remaining goals
                                let remaining: Vec<String> = goals[1..]
                                    .iter()
                                    .map(|goal| {
                                        if let ExprKind::App(func, goal_args) = &goal.kind {
                                            let gfn = Self::expr_fn_name(func);
                                            let gargs: Vec<String> = goal_args
                                                .iter()
                                                .map(|a| {
                                                    if let ExprKind::Var(name) = &a.kind {
                                                        if let Some((_, idx)) = head_vars
                                                            .iter()
                                                            .find(|(n, _)| n == name)
                                                        {
                                                            param_names[*idx].clone()
                                                        } else {
                                                            sanitize_name(name)
                                                        }
                                                    } else {
                                                        self.emit_prolog_arg(a)
                                                    }
                                                })
                                                .collect();
                                            format!("{}({})", sanitize_name(&gfn), gargs.join(", "))
                                        } else {
                                            self.emit_prolog_arg(goal)
                                        }
                                    })
                                    .collect();

                                if remaining.is_empty() {
                                    out.push_str("        return true;\n");
                                } else {
                                    out.push_str(&format!(
                                        "        if {} {{ return true; }}\n",
                                        remaining.join(" && ")
                                    ));
                                }
                                out.push_str("    }\n");
                            }
                        } else {
                            // Simple conjunction: all vars bound
                            let cond_parts: Vec<String> = goals
                                .iter()
                                .map(|goal| {
                                    if let ExprKind::App(func, goal_args) = &goal.kind {
                                        let gfn = Self::expr_fn_name(func);
                                        let gargs: Vec<String> = goal_args
                                            .iter()
                                            .map(|a| {
                                                if let ExprKind::Var(name) = &a.kind {
                                                    if let Some((_, idx)) =
                                                        head_vars.iter().find(|(n, _)| n == name)
                                                    {
                                                        param_names[*idx].clone()
                                                    } else {
                                                        sanitize_name(name)
                                                    }
                                                } else {
                                                    self.emit_prolog_arg(a)
                                                }
                                            })
                                            .collect();
                                        format!("{}({})", sanitize_name(&gfn), gargs.join(", "))
                                    } else {
                                        self.emit_prolog_arg(goal)
                                    }
                                })
                                .collect();
                            out.push_str(&format!(
                                "    if {} {{ return true; }}\n",
                                cond_parts.join(" && ")
                            ));
                        }
                    } else {
                        // Simple non-conjunction body — emit as a call with proper arg substitution
                        if let ExprKind::App(func, call_args) = &body.kind {
                            let called_fn = Self::expr_fn_name(func);
                            let gargs: Vec<String> = call_args
                                .iter()
                                .map(|a| {
                                    if let ExprKind::Var(name) = &a.kind {
                                        if let Some((_, idx)) =
                                            head_vars.iter().find(|(n, _)| n == name)
                                        {
                                            param_names[*idx].clone()
                                        } else {
                                            sanitize_name(name)
                                        }
                                    } else {
                                        self.emit_prolog_arg(a)
                                    }
                                })
                                .collect();
                            out.push_str(&format!(
                                "    if {}({}) {{ return true; }}\n",
                                sanitize_name(&called_fn),
                                gargs.join(", ")
                            ));
                        } else {
                            // Fallback: use word-boundary replacement
                            let mut body_str = self.emit_expr(body);
                            for (var_name, idx) in &head_vars {
                                let san = sanitize_name(var_name);
                                let replacement = &param_names[*idx];
                                let mut result = String::new();
                                let chars: Vec<char> = body_str.chars().collect();
                                let san_chars: Vec<char> = san.chars().collect();
                                let mut i = 0;
                                while i < chars.len() {
                                    if i + san_chars.len() <= chars.len()
                                        && chars[i..i + san_chars.len()] == san_chars[..]
                                    {
                                        let before_ok = i == 0
                                            || !(chars[i - 1].is_alphanumeric()
                                                || chars[i - 1] == '_');
                                        let after_ok = i + san_chars.len() >= chars.len()
                                            || !(chars[i + san_chars.len()].is_alphanumeric()
                                                || chars[i + san_chars.len()] == '_');
                                        if before_ok && after_ok {
                                            result.push_str(replacement);
                                            i += san_chars.len();
                                            continue;
                                        }
                                    }
                                    result.push(chars[i]);
                                    i += 1;
                                }
                                body_str = result;
                            }
                            out.push_str(&format!("    if {} {{ return true; }}\n", body_str));
                        }
                    }
                }
            }
        }

        // Check fact table
        if !facts.is_empty() {
            if arity == 1 {
                out.push_str(&format!(
                    "    if {}.contains(&{}) {{ return true; }}\n",
                    table_name, param_names[0]
                ));
            } else {
                let checks: Vec<String> = (0..arity)
                    .map(|i| format!("f.{} == {}", i, param_names[i]))
                    .collect();
                out.push_str(&format!(
                    "    if {}.iter().any(|f| {}) {{ return true; }}\n",
                    table_name,
                    checks.join(" && ")
                ));
            }
        }

        out.push_str("    false\n}\n");
        out
    }

    /// Emit findall(template_var, goal) as a Rust expression.
    /// findall(c, parent("bob", c)) → iterate PARENT_FACTS, collect matching values.
    fn emit_findall(&mut self, template: &Expr, goal: &Expr) -> String {
        let template_name = match &template.kind {
            ExprKind::Var(name) => name.clone(),
            _ => return "vec![]".to_string(),
        };

        if let ExprKind::App(func, goal_args) = &goal.kind {
            let fn_name = Self::expr_fn_name(func);
            let table = format!("{}_FACTS", sanitize_name(&fn_name).to_uppercase());

            // Find which position is the template variable
            let template_pos = goal_args
                .iter()
                .position(|a| matches!(a.kind, ExprKind::Var(ref n) if n == &template_name));

            if let Some(t_pos) = template_pos {
                let arity = goal_args.len();
                let is_unary = arity == 1;

                // Determine if the value type needs .to_string()
                let is_str = self
                    .types
                    .prolog_rule_fns
                    .get(&fn_name)
                    .and_then(|types| types.get(t_pos))
                    .map(|t| t == "&str")
                    .unwrap_or(false);

                // For rules that have no fact table (only variable-head rules),
                // fall back to calling the function in a loop
                if !self.types.prolog_rule_fns.contains_key(&fn_name) {
                    return "vec![]".to_string(); // can't iterate non-fact rules
                }

                // Build filter conditions for non-template positions
                let mut filters = Vec::new();
                for (i, a) in goal_args.iter().enumerate() {
                    if i == t_pos {
                        continue;
                    }
                    if matches!(a.kind, ExprKind::Var(ref n) if n == "_") {
                        continue;
                    }
                    let val = self.emit_prolog_arg(a);
                    if is_unary {
                        filters.push(format!("f == &{}", val));
                    } else {
                        filters.push(format!("f.{} == {}", i, val));
                    }
                }

                let value_expr = if is_unary {
                    if is_str {
                        "f.to_string()".to_string()
                    } else {
                        "(*f)".to_string()
                    }
                } else if is_str {
                    format!("f.{}.to_string()", t_pos)
                } else {
                    format!("f.{}", t_pos)
                };

                if filters.is_empty() {
                    format!(
                        "{}.iter().map(|f| {}).collect::<Vec<_>>()",
                        table, value_expr
                    )
                } else {
                    format!(
                        "{}.iter().filter(|f| {}).map(|f| {}).collect::<Vec<_>>()",
                        table,
                        filters.join(" && "),
                        value_expr
                    )
                }
            } else {
                // Template var not directly in goal args — can't optimize, return empty
                "vec![]".to_string()
            }
        } else {
            "vec![]".to_string()
        }
    }

    /// Collect all variable names referenced in an expression (for detecting free variable usage)
    fn collect_free_var_refs(expr: &Expr, refs: &mut BTreeSet<String>) {
        match &expr.kind {
            ExprKind::Var(name) => {
                refs.insert(name.clone());
            }
            ExprKind::App(f, args) => {
                Self::collect_free_var_refs(f, refs);
                for a in args {
                    Self::collect_free_var_refs(a, refs);
                }
            }
            ExprKind::BinOp(_, l, r) => {
                Self::collect_free_var_refs(l, refs);
                Self::collect_free_var_refs(r, refs);
            }
            ExprKind::UnOp(_, e) => {
                Self::collect_free_var_refs(e, refs);
            }
            ExprKind::If(c, t, e) => {
                Self::collect_free_var_refs(c, refs);
                Self::collect_free_var_refs(t, refs);
                Self::collect_free_var_refs(e, refs);
            }
            ExprKind::Field(e, _) => {
                Self::collect_free_var_refs(e, refs);
            }
            ExprKind::Conjunction(goals) => {
                for g in goals {
                    Self::collect_free_var_refs(g, refs);
                }
            }
            _ => {}
        }
    }

    /// Word-boundary replacement: replace `old` with `new` only at word boundaries
    fn word_replace(&self, text: &str, old: &str, new: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = text.chars().collect();
        let old_chars: Vec<char> = old.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + old_chars.len() <= chars.len() && chars[i..i + old_chars.len()] == old_chars[..]
            {
                let before_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
                let after_ok = i + old_chars.len() >= chars.len()
                    || !(chars[i + old_chars.len()].is_alphanumeric()
                        || chars[i + old_chars.len()] == '_');
                if before_ok && after_ok {
                    result.push_str(new);
                    i += old_chars.len();
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    }

    /// Emit value-returning Prolog rule function: returns Option<T> instead of bool.
    /// Used for rules like `| capital("Denmark") -> "Copenhagen"` (lookup tables).
    fn emit_prolog_value_function(
        &mut self,
        sanitized: &str,
        fn_name: &str,
        rules: &[&Rule],
        arity: usize,
        param_types: &[&str],
        value_type_str: &str,
    ) -> String {
        let mut out = String::new();
        let value_rust_type = if value_type_str == "String" {
            "&str"
        } else {
            value_type_str
        };

        // Collect value facts: clauses with all-literal heads and a literal body
        let value_facts: Vec<(Vec<String>, String)> = rules
            .iter()
            .filter_map(|r| {
                if let Rule::Clause {
                    head,
                    body: Some(body),
                } = r
                {
                    if let ExprKind::App(_, args) = &head.kind {
                        if args.iter().all(|a| matches!(a.kind, ExprKind::Lit(_))) {
                            if let ExprKind::Lit(lit) = &body.kind {
                                let keys: Vec<String> = args
                                    .iter()
                                    .map(|a| {
                                        if let ExprKind::Lit(l) = &a.kind {
                                            Self::emit_literal_value(l)
                                        } else {
                                            "?".into()
                                        }
                                    })
                                    .collect();
                                return Some((keys, Self::emit_literal_value(lit)));
                            }
                        }
                    }
                }
                None
            })
            .collect();

        // Emit fact table (key columns + value column)
        let table_name = format!("{}_FACTS", sanitized.to_uppercase());
        if !value_facts.is_empty() {
            let key_types: Vec<&str> = param_types
                .iter()
                .map(|t| if *t == "String" { "&str" } else { t })
                .collect();
            out.push_str(&format!(
                "const {}: &[({}, {},)] = &[\n",
                table_name,
                key_types.join(", "),
                value_rust_type
            ));
            for (keys, val) in &value_facts {
                out.push_str(&format!("    ({}, {}),\n", keys.join(", "), val));
            }
            out.push_str("];\n\n");
        }

        // Function signature: returns Option<T>
        let param_names: Vec<String> = (0..arity).map(|i| format!("_p{}", i)).collect();
        let param_str: String = param_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if param_types[i] == "String" {
                    format!("{}: &str", name)
                } else {
                    format!("{}: {}", name, param_types[i])
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let ret_type = if value_type_str == "String" {
            "Option<String>".to_string()
        } else {
            format!("Option<{}>", value_type_str)
        };
        out.push_str(&format!(
            "fn {}({}) -> {} {{\n",
            sanitized, param_str, ret_type
        ));

        // Lookup in fact table
        if !value_facts.is_empty() {
            let key_checks: Vec<String> = (0..arity)
                .map(|i| format!("f.{} == {}", i, param_names[i]))
                .collect();
            let value_idx = arity;
            let value_expr = if value_type_str == "String" {
                format!("f.{}.to_string()", value_idx)
            } else {
                format!("f.{}", value_idx)
            };
            out.push_str(&format!("    for f in {}.iter() {{\n", table_name));
            out.push_str(&format!(
                "        if {} {{ return Some({}); }}\n",
                key_checks.join(" && "),
                value_expr
            ));
            out.push_str("    }\n");
        }

        // Emit rules with variable heads (non-fact clauses that return values)
        for r in rules {
            if let Rule::Clause {
                head,
                body: Some(body),
            } = r
            {
                if let ExprKind::App(_, head_args) = &head.kind {
                    // Skip all-literal heads (already in fact table)
                    if head_args.iter().all(|a| matches!(a.kind, ExprKind::Lit(_))) {
                        continue;
                    }
                    let head_vars: Vec<(String, usize)> = head_args
                        .iter()
                        .enumerate()
                        .filter_map(|(i, a)| {
                            if let ExprKind::Var(name) = &a.kind {
                                Some((name.clone(), i))
                            } else {
                                None
                            }
                        })
                        .collect();

                    let mut body_str = self.emit_expr(body);
                    for (var_name, idx) in &head_vars {
                        body_str = self.word_replace(
                            &body_str,
                            &sanitize_name(var_name),
                            &param_names[*idx],
                        );
                    }
                    // Inline literal bindings from outer scope
                    let bindings: Vec<(String, String)> = self
                        .types
                        .literal_bindings
                        .iter()
                        .map(|(k, (v, _))| (k.clone(), v.clone()))
                        .collect();
                    for (bind_name, bind_val) in &bindings {
                        body_str =
                            self.word_replace(&body_str, &sanitize_name(bind_name), bind_val);
                    }

                    let wrapped = if value_type_str == "String" {
                        format!("Some({}.to_string())", body_str)
                    } else {
                        format!("Some({})", body_str)
                    };
                    out.push_str(&format!("    return {};\n", wrapped));
                }
            }
        }

        out.push_str("    None\n}\n");
        out
    }

    /// Emit a group of rules with the same name as a single Rust function.
    /// Handles both Catala-style (exception/default/under) and Prolog-style (facts/conjunction).
    fn emit_rule_function(&mut self, fn_name: &str, rules: &[&Rule]) -> String {
        // Check if this rule group has Prolog-style features (ground terms or conjunction)
        let arity = Self::rule_arity(rules);
        if Self::rules_have_prolog_features(rules) && arity > 0 {
            return self.emit_prolog_rule_function(fn_name, rules, arity);
        }

        // --- Original Catala-style codegen ---

        // Extract params from the first rule's head
        let params: Vec<String> = rules
            .iter()
            .find_map(|r| {
                let head = match r {
                    Rule::Clause { head, .. }
                    | Rule::Default { head, .. }
                    | Rule::Exception { head, .. } => head,
                    _ => return None,
                };
                if let ExprKind::App(_, args) = &head.kind {
                    Some(
                        args.iter()
                            .filter_map(|a| {
                                if let ExprKind::Var(n) = &a.kind {
                                    Some(n.clone())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    Some(vec![])
                }
            })
            .unwrap_or_default();

        // Infer parameter types from field access patterns and body expression analysis
        let inferred_types: Vec<String> = params
            .iter()
            .map(|p| {
                if let Some(ty) = self.infer_param_type_from_fields(p, rules) {
                    format!("&{}", ty)
                } else if let Some(ty) = self.infer_param_type_from_body(p, rules) {
                    ty
                } else {
                    "bool".to_string()
                }
            })
            .collect();
        let param_str = params
            .iter()
            .zip(inferred_types.iter())
            .map(|(p, ty)| format!("{}: {}", sanitize_name(p), ty))
            .collect::<Vec<_>>()
            .join(", ");

        // Register call-site type info if any param takes &str (so callers pass &str not String)
        if inferred_types.iter().any(|t| t == "&str") {
            self.types
                .prolog_rule_fns
                .insert(fn_name.to_string(), inferred_types.clone());
        }

        // Infer return type from constructor calls in rule values
        let ret_type = self
            .infer_rule_return_type(rules)
            .unwrap_or_else(|| "bool".to_string());

        let mut out = format!(
            "fn {}({}) -> {} {{\n",
            sanitize_name(fn_name),
            param_str,
            ret_type
        );

        // Mark non-Copy rule params for .clone() at each use site
        for p in &params {
            let ty = self
                .infer_param_type_from_fields(p, rules)
                .unwrap_or_default();
            if !matches!(ty.as_str(), "bool" | "i64" | "f64" | "char" | "u64" | "") {
                self.types.rule_clone_params.insert(p.clone());
            }
        }

        // Emit literal bindings referenced by rule bodies (outer = bindings promoted into scope)
        {
            let mut needed: BTreeSet<String> = BTreeSet::new();
            for rule in rules {
                let exprs: Vec<&Expr> = match rule {
                    Rule::Clause { body: Some(b), .. } => vec![b],
                    Rule::Default {
                        value, condition, ..
                    } => {
                        let mut v = vec![value];
                        if let Some(c) = condition {
                            v.push(c);
                        }
                        v
                    }
                    Rule::Exception {
                        value, condition, ..
                    } => {
                        let mut v = vec![value];
                        if let Some(c) = condition {
                            v.push(c);
                        }
                        v
                    }
                    _ => vec![],
                };
                for e in exprs {
                    Self::collect_free_var_refs(e, &mut needed);
                }
            }
            let param_set: BTreeSet<String> = params.iter().cloned().collect();
            for name in &needed {
                if param_set.contains(name) {
                    continue;
                }
                if let Some((val, ty)) = self.types.literal_bindings.get(name) {
                    let rust_val = if ty == "i64" {
                        format!("{}i64", val)
                    } else if ty == "f64" {
                        format!("{}f64", val)
                    } else {
                        val.clone()
                    };
                    out.push_str(&format!(
                        "    let {} = {};\n",
                        sanitize_name(name),
                        rust_val
                    ));
                }
            }
        }

        // Pass 1: exceptions (highest priority)
        for rule in rules {
            if let Rule::Exception {
                value, condition, ..
            } = rule
            {
                if let Some(cond) = condition {
                    out.push_str(&format!(
                        "    if {} {{ return {}; }}\n",
                        self.emit_expr(cond),
                        self.emit_expr(value)
                    ));
                } else {
                    out.push_str(&format!("    return {};\n", self.emit_expr(value)));
                    out.push_str("}\n");
                    return out;
                }
            }
        }

        // Pass 2: conditional defaults
        for rule in rules {
            if let Rule::Default {
                value,
                condition: Some(cond),
                ..
            } = rule
            {
                out.push_str(&format!(
                    "    if {} {{ return {}; }}\n",
                    self.emit_expr(cond),
                    self.emit_expr(value)
                ));
            }
        }

        // Pass 3: unconditional defaults and clauses with backtracking
        for rule in rules {
            match rule {
                Rule::Default {
                    value,
                    condition: None,
                    ..
                } => {
                    out.push_str(&format!("    {}\n", self.emit_expr(value)));
                    out.push_str("}\n");
                    return out;
                }
                Rule::Clause {
                    body: Some(body), ..
                } => {
                    if ret_type == "bool" {
                        out.push_str(&format!(
                            "    if {} {{ return true; }}\n",
                            self.emit_expr(body)
                        ));
                    } else {
                        out.push_str(&format!("    {}\n", self.emit_expr(body)));
                        out.push_str("}\n");
                        return out;
                    }
                }
                Rule::Clause { body: None, .. } => {
                    out.push_str("    true\n");
                    out.push_str("}\n");
                    return out;
                }
                _ => {}
            }
        }

        if ret_type == "bool" {
            out.push_str("    false\n");
        } else {
            out.push_str(&format!(
                "    panic!(\"no | rule matched for '{}'\")\n",
                fn_name
            ));
        }
        out.push_str("}\n");
        out
    }

    fn emit_defn(&mut self, defn: &Defn) -> String {
        match defn {
            Defn::Fn {
                name,
                params,
                ret_ty,
                effects,
                body,
            } => {
                // Skip comptime-only functions (return TypeDef — not a real Rust type)
                if let Some(Ty::Name(ty_name)) = ret_ty.as_ref() {
                    if ty_name == "TypeDef" {
                        return format!("// comptime-only fn {} (returns TypeDef)\n", name);
                    }
                }
                // Register inout params for call-site emission
                let inout_flags: Vec<bool> = params.iter().map(|p| p.inout).collect();
                if inout_flags.iter().any(|f| *f) {
                    self.types
                        .inout_params
                        .insert(name.clone(), inout_flags.clone());
                }
                // Copy-on-write: detect inout + shared T params
                let cow_flags: Vec<bool> = params
                    .iter()
                    .map(|p| p.inout && matches!(p.ty.as_ref(), Some(Ty::Shared(_))))
                    .collect();
                if cow_flags.iter().any(|f| *f) {
                    self.types.cow_params.insert(name.clone(), cow_flags);
                }

                // Phase 3b/3d: Auto-borrow analysis with ref-match + self-recursive relaxation
                // Use pre-pass results if available (fixed-point borrow analysis ran earlier),
                // otherwise compute fresh.
                let borrow_flags = if let Some(pre) = self.borrow_only_params.get(name) {
                    pre.clone()
                } else {
                    let mut flags = analyze_borrow_only_params_named(
                        params,
                        body,
                        ret_ty.as_ref(),
                        &self.borrow_only_params,
                        Some(name.as_str()),
                    );
                    // Phase 3b safety: disable ref-match for types with boxed (recursive) fields.
                    // Matching on &T can't dereference Box<T> fields — they'd be &Box<T>.
                    {
                        let mut matched_vars: BTreeSet<String> = BTreeSet::new();
                        collect_matched_vars(body, &mut matched_vars);
                        for (idx, p) in params.iter().enumerate() {
                            if flags[idx] && matched_vars.contains(&p.name) {
                                if let Some(ty) = &p.ty {
                                    let type_name = match ty {
                                        Ty::App(base, _) => {
                                            if let Ty::Name(n) = base.as_ref() {
                                                Some(n.as_str())
                                            } else {
                                                None
                                            }
                                        }
                                        Ty::Name(n) => Some(n.as_str()),
                                        _ => None,
                                    };
                                    if let Some(tn) = type_name {
                                        let has_boxed = self.types.variant_boxed_args.iter().any(
                                            |(vname, indices)| {
                                                !indices.is_empty()
                                                    && self
                                                        .types
                                                        .variant_parent
                                                        .get(vname.as_str())
                                                        .map(|p| {
                                                            p == tn
                                                                || self
                                                                    .types
                                                                    .type_rename
                                                                    .get(tn)
                                                                    .map(|r| r == p)
                                                                    .unwrap_or(false)
                                                        })
                                                        .unwrap_or(false)
                                            },
                                        );
                                        if has_boxed {
                                            flags[idx] = false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if flags.iter().any(|f| *f) {
                        self.borrow_only_params.insert(name.clone(), flags.clone());
                    }
                    flags
                };

                let params_str: Vec<String> = params
                    .iter()
                    .enumerate()
                    .map(|(idx, p)| {
                        // For arrow-typed params, determine FnOnce vs FnMut:
                        // FnOnce only when: exactly 1 total use, that use is a direct call,
                        // and the call is not inside a nested lambda.
                        let is_fn_once = if matches!(p.ty.as_ref(), Some(Ty::Arrow(..))) {
                            can_be_fn_once(body, &p.name)
                        } else {
                            false
                        };
                        self.fn_once_mode = is_fn_once;
                        let ty =
                            p.ty.as_ref()
                                .map(|t| self.emit_type(t))
                                .unwrap_or_else(|| "i64".to_string());
                        self.fn_once_mode = false;
                        if p.inout {
                            // Copy-on-write: inout on shared T → &mut T (Arc::make_mut at call site)
                            let inner_ty = match p.ty.as_ref() {
                                Some(Ty::Shared(inner)) => self.emit_type(inner),
                                _ => ty.clone(),
                            };
                            format!("{}: &mut {}", sanitize_name(&p.name), inner_ty)
                        } else if borrow_flags.get(idx).copied().unwrap_or(false) {
                            // Auto-borrow: param is only read, emit &T
                            format!("{}: &{}", sanitize_name(&p.name), ty)
                        } else if matches!(p.ty.as_ref(), Some(Ty::Arrow(..))) {
                            if !is_fn_once {
                                // FnMut params called multiple times need `mut`
                                format!("mut {}: {}", sanitize_name(&p.name), ty)
                            } else {
                                // FnOnce params called at most once — no `mut` needed
                                format!("{}: {}", sanitize_name(&p.name), ty)
                            }
                        } else {
                            format!("{}: {}", sanitize_name(&p.name), ty)
                        }
                    })
                    .collect();

                let ret = ret_ty
                    .as_ref()
                    .map(|t| format!(" -> {}", self.emit_type(t)))
                    .unwrap_or_default();

                // Collect generic type variables from all param types + return type
                let mut type_vars = Vec::new();
                for p in params {
                    if let Some(ty) = &p.ty {
                        self.collect_type_vars(ty, &mut type_vars);
                    }
                }
                if let Some(ty) = ret_ty {
                    self.collect_type_vars(ty, &mut type_vars);
                }
                let generics = if type_vars.is_empty() {
                    String::new()
                } else {
                    // Add Display bound so .to_string() works
                    let bounds: Vec<String> = type_vars
                        .iter()
                        .map(|v| format!("{}: fmt::Display + Clone", v))
                        .collect();
                    format!("<{}>", bounds.join(", "))
                };

                // Escape analysis: count variable uses in function body
                // Phase 3b: Use borrow-aware counting for consuming uses
                let prev_counts = std::mem::take(&mut self.var_use_counts);
                let prev_consuming = std::mem::take(&mut self.var_consuming_counts);
                let prev_copy = std::mem::take(&mut self.copy_vars);
                let prev_ref_match = std::mem::take(&mut self.ref_match_bindings);
                let prev_borrow_params = std::mem::take(&mut self.current_borrow_params);
                let prev_mutable = std::mem::take(&mut self.mutable_vars);
                let prev_aliased = std::mem::take(&mut self.aliased_vars);
                let prev_string_vars = std::mem::take(&mut self.string_typed_vars);
                let prev_float_vars = std::mem::take(&mut self.float_typed_vars);
                let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
                let ownership = OwnershipAnalysis::analyze(
                    body,
                    &self.borrow_only_params,
                    Some(name.as_str()),
                    &param_names,
                );
                self.var_use_counts = ownership.var_uses;
                self.var_consuming_counts = ownership.consuming_uses;
                // Detect accumulators (variables rebound inside for loops) in function body
                if let ExprKind::Block(body_stmts) = &body.kind {
                    let refs: Vec<&Stmt> = body_stmts.iter().collect();
                    collect_mutable_vars(&refs, &mut self.mutable_vars);
                    self.collect_inout_mutables_stmts(&refs);
                    self.aliased_vars = collect_aliased_vars(&refs, &self.copy_vars);
                }
                // Track inout params as mutable (they're &mut, so push() can use them directly)
                for p in params {
                    if p.inout {
                        self.mutable_vars.insert(p.name.clone());
                    }
                }
                // Track Copy-type parameters
                for p in params {
                    if let Some(ty) = &p.ty {
                        if is_copy_type(ty) {
                            self.copy_vars.insert(p.name.clone());
                        }
                    }
                }
                // Track String-typed parameters (for string concat detection)
                for p in params {
                    if matches!(p.ty.as_ref(), Some(Ty::Name(n)) if n == "String") {
                        self.string_typed_vars.insert(p.name.clone());
                    }
                }
                // Track Float-typed parameters (for float division/fold detection)
                for p in params {
                    if matches!(p.ty.as_ref(), Some(Ty::Name(n)) if n == "Float") {
                        self.float_typed_vars.insert(p.name.clone());
                    }
                }
                // Auto-borrow params are effectively Copy (accessed via &, no ownership transfer)
                for (idx, p) in params.iter().enumerate() {
                    if borrow_flags.get(idx).copied().unwrap_or(false) {
                        self.copy_vars.insert(p.name.clone());
                    }
                }
                // Phase 3b: Track current function's borrowed params (to avoid double-borrow at call sites)
                self.current_borrow_params.clear();
                for (idx, p) in params.iter().enumerate() {
                    if borrow_flags.get(idx).copied().unwrap_or(false) {
                        self.current_borrow_params.insert(p.name.clone());
                    }
                }
                // Phase 3b: Collect ref-match bindings — pattern vars from matches on borrowed params
                // These need * deref because they're &T from matching on a reference
                self.ref_match_bindings.clear();
                {
                    let mut matched_vars: BTreeSet<String> = BTreeSet::new();
                    collect_matched_vars(body, &mut matched_vars);
                    for (idx, p) in params.iter().enumerate() {
                        if borrow_flags.get(idx).copied().unwrap_or(false)
                            && matched_vars.contains(&p.name)
                        {
                            // This param is borrowed AND matched → ref-match
                            // Collect pattern bindings from matches on this param
                            collect_ref_match_bindings_from_body(
                                body,
                                &p.name,
                                &mut self.ref_match_bindings,
                            );
                        }
                    }
                }

                // Effect handler params: `with Console` adds `__eff_Console: &mut impl Console`
                // Merge explicit effects (from AST `with` clause) with inferred effects
                let mut merged_effects = effects.clone();
                if let Some(inferred) = self.types.fn_effects.get(name) {
                    for eff in inferred {
                        if !merged_effects.contains(eff) {
                            merged_effects.push(eff.clone());
                        }
                    }
                }
                let mut all_params = params_str.clone();
                for eff in &merged_effects {
                    all_params.push(format!("__eff_{}: &mut impl {}", eff, eff));
                }

                let prev_effects = std::mem::take(&mut self.current_effects);
                self.current_effects = merged_effects;

                let is_exported = self.types.exported_names.contains(name);
                let pub_prefix = if is_exported { "pub " } else { "" };
                // M4: wasm-bindgen annotation for exported functions with compatible types
                let wasm_attr = if self.wasm_mode && is_exported {
                    let params_ok = params.iter().all(|p| {
                        p.ty.as_ref()
                            .map(|t| Self::is_wasm_compatible_type(t))
                            .unwrap_or(false)
                    });
                    let ret_ok = ret_ty
                        .as_ref()
                        .map(|t| Self::is_wasm_compatible_type(t))
                        .unwrap_or(true);
                    if params_ok && ret_ok && generics.is_empty() && self.current_effects.is_empty()
                    {
                        // wasm-bindgen needs &str not &String for params
                        for p in all_params.iter_mut() {
                            if p.ends_with(": &String") {
                                let prefix = &p[..p.len() - ": &String".len()];
                                *p = format!("{}: &str", prefix);
                            }
                        }
                        "#[wasm_bindgen]\n"
                    } else {
                        "// wasm: skipped (complex types or effects)\n"
                    }
                } else {
                    ""
                };
                let mut out = format!(
                    "{}{}fn {}{}({}){} {{\n",
                    wasm_attr,
                    pub_prefix,
                    sanitize_name(name),
                    generics,
                    all_params.join(", "),
                    ret
                );
                self.indent = 1;
                // Tail-call elimination: if the function is tail-recursive,
                // emit as a loop with parameter reassignment instead of recursive calls.
                // Disable TCE when a borrowed param gets a new value in a tail call
                // (can't reassign &T loop variables).
                let tce_safe = is_tail_recursive(name, body)
                    && !tce_has_borrowed_param_update(name, params, &borrow_flags, body);
                if tce_safe {
                    out.push_str(&self.emit_tce_body(name, params, &borrow_flags, body));
                } else {
                    out.push_str(&self.emit_expr_as_return(body));
                }
                self.indent = 0;
                out.push_str("}\n");
                // Restore previous state (for nested functions)
                self.var_use_counts = prev_counts;
                self.var_consuming_counts = prev_consuming;
                self.copy_vars = prev_copy;
                self.ref_match_bindings = prev_ref_match;
                self.current_borrow_params = prev_borrow_params;
                self.mutable_vars = prev_mutable;
                self.aliased_vars = prev_aliased;
                self.string_typed_vars = prev_string_vars;
                self.float_typed_vars = prev_float_vars;
                self.current_effects = prev_effects;
                out
            }
            Defn::Actor {
                name,
                state_param,
                handlers,
            } => {
                let sname = sanitize_name(name);
                let state_type = state_param
                    .ty
                    .as_ref()
                    .map(|t| self.emit_type(t))
                    .unwrap_or_else(|| "i64".to_string());
                // Mark actor message variant names as copy-like (they're enum constructors, not variables)
                for h in handlers {
                    let vname = match &h.msg_pat {
                        Pat::Con(n, _) => Some(n.clone()),
                        Pat::Var(n) => Some(n.clone()),
                        _ => None,
                    };
                    if let Some(vn) = vname {
                        self.copy_vars.insert(vn);
                    }
                }
                let mut out = String::new();

                // Message enum
                out.push_str(&format!("#[derive(Debug)]\n"));
                out.push_str(&format!("enum {}Msg {{\n", sname));
                for h in handlers {
                    let variant = self.emit_pattern_as_enum_variant(&h.msg_pat);
                    out.push_str(&format!("    {},\n", variant));
                }
                out.push_str(&format!(
                    "    __Ask(Box<{}Msg>, tokio::sync::oneshot::Sender<{}>),\n",
                    sname, state_type
                ));
                out.push_str("}\n\n");

                // Actor loop
                out.push_str(&format!(
                    "async fn {}_run(mut rx: tokio::sync::mpsc::UnboundedReceiver<{}Msg>, mut {}: {}) {{\n",
                    sname, sname, sanitize_name(&state_param.name), state_type
                ));
                out.push_str("    while let Some(msg) = rx.recv().await {\n");
                out.push_str("        match msg {\n");
                for h in handlers {
                    let pat = self.emit_pattern_as_match_arm(&h.msg_pat, &sname);
                    let body = self.emit_expr(&h.body);
                    out.push_str(&format!(
                        "            {} => {{ {} = {}; }}\n",
                        pat,
                        sanitize_name(&state_param.name),
                        body
                    ));
                }
                // __Ask: process the inner message first, then reply with updated state
                let state_name = sanitize_name(&state_param.name);
                out.push_str(&format!(
                    "            {}Msg::__Ask(inner, reply) => {{\n",
                    sname
                ));
                out.push_str("                match *inner {\n");
                for h in handlers {
                    let pat = self.emit_pattern_as_match_arm(&h.msg_pat, &sname);
                    let body = self.emit_expr(&h.body);
                    out.push_str(&format!(
                        "                    {} => {{ {} = {}; }}\n",
                        pat, state_name, body
                    ));
                }
                out.push_str(&format!(
                    "                    {}Msg::__Ask(_, _) => {{}}\n",
                    sname
                ));
                out.push_str("                }\n");
                out.push_str(&format!(
                    "                let _ = reply.send({});\n",
                    state_name
                ));
                out.push_str("            }\n");
                out.push_str("        }\n    }\n}\n\n");

                // Spawn helper
                out.push_str(&format!(
                    "fn {}_spawn(initial: {}) -> tokio::sync::mpsc::UnboundedSender<{}Msg> {{\n",
                    sname, state_type, sname
                ));
                out.push_str("    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();\n");
                out.push_str(&format!("    tokio::spawn({}_run(rx, initial));\n", sname));
                out.push_str("    tx\n}\n");
                // Bring message variants into scope so bare names work: c <- Increment
                out.push_str(&format!("#[allow(unused_imports)]\nuse {}Msg::*;\n", sname));
                out
            }
            Defn::Module { name, body } => {
                self.types.known_modules.insert(name.clone());
                let pub_prefix = if self.types.exported_names.contains(name) {
                    "pub "
                } else {
                    ""
                };
                let mut out = format!("{}mod {} {{\n", pub_prefix, sanitize_name(name));
                out.push_str("    use super::*;\n");
                // Mark all items inside inline modules as exported (pub)
                // so they're accessible via Module::item()
                let saved_exported = self.types.exported_names.clone();
                for stmt in body {
                    match stmt {
                        Stmt::Defn(Defn::Fn { name: fn_name, .. }) => {
                            self.types.exported_names.insert(fn_name.clone());
                        }
                        Stmt::Defn(Defn::Module { name: mod_name, .. }) => {
                            self.types.exported_names.insert(mod_name.clone());
                        }
                        Stmt::TypeDecl(TypeDecl::ADT { name: ty_name, .. }) => {
                            self.types.exported_names.insert(ty_name.clone());
                        }
                        _ => {}
                    }
                }
                self.indent = 1;
                for stmt in body {
                    out.push_str(&self.emit_stmt(stmt));
                }
                self.indent = 0;
                out.push_str("}\n");
                // Re-export enum variants so Module::Variant works
                for stmt in body {
                    if let Stmt::TypeDecl(TypeDecl::ADT {
                        name: ty_name,
                        variants,
                        ..
                    }) = stmt
                    {
                        if variants.len() > 1
                            || (variants.len() == 1 && variants[0].name != *ty_name)
                        {
                            let sname = sanitize_name(name);
                            let rty = self.rust_type_name(ty_name);
                            let vnames: Vec<String> =
                                variants.iter().map(|v| v.name.clone()).collect();
                            out.push_str(&format!(
                                "#[allow(unused_imports)]\nuse {}::{}::{{{}}};\n",
                                sname,
                                rty,
                                vnames.join(", ")
                            ));
                        }
                    }
                }
                self.types.exported_names = saved_exported;
                out
            }
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Defn(defn) => self.emit_defn(defn),
            Stmt::TypeDecl(decl) => self.emit_type_decl(decl),
            Stmt::Bind(pat, _ty, value) => {
                // Check if this binding was comptime-evaluated
                let comptime_name = match pat {
                    Pat::Var(name) => Some(name.as_str()),
                    Pat::Con(name, args) if args.is_empty() => Some(name.as_str()),
                    _ => None,
                };
                if let Some(name) = comptime_name {
                    if let Some(rust_lit) = self.types.comptime_values.get(name).cloned() {
                        // Comptime types are emitted as type declarations, not bindings
                        if rust_lit.is_empty() {
                            return format!(
                                "{}// @ comptime type {} (emitted above)\n",
                                self.ind(),
                                name
                            );
                        }
                        let rust_ty = self
                            .types
                            .comptime_types
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        // Use const for Copy types, let for heap types
                        if matches!(
                            rust_ty.as_str(),
                            "i64" | "f64" | "bool" | "char" | "u64" | "()"
                        ) {
                            return format!(
                                "{}const {}: {} = {}; // @ comptime\n",
                                self.ind(),
                                sanitize_name(name),
                                rust_ty,
                                rust_lit
                            );
                        } else {
                            // Include type annotation for Result/Option (Rust can't infer error type from Ok/Some)
                            let ty_annot = if !rust_ty.is_empty()
                                && (rust_ty.starts_with("Result") || rust_ty.starts_with("Option"))
                            {
                                format!(": {}", rust_ty)
                            } else {
                                String::new()
                            };
                            return format!(
                                "{}let {}{} = {}; // @ comptime\n",
                                self.ind(),
                                sanitize_name(name),
                                ty_annot,
                                rust_lit
                            );
                        }
                    }
                }
                let pat_str = self.emit_pattern_binding(pat);
                let mut val_str = self.emit_expr(value);
                // Clone when binding from a variable that has other consuming uses
                // (= alias = original where original is used elsewhere → clone)
                if let ExprKind::Var(src_name) = &value.kind {
                    if !self.copy_vars.contains(src_name.as_str())
                        && !self.types.variant_parent.contains_key(src_name.as_str())
                    {
                        let consuming = self
                            .var_consuming_counts
                            .get(src_name.as_str())
                            .copied()
                            .unwrap_or(0);
                        let total = self
                            .var_use_counts
                            .get(src_name.as_str())
                            .copied()
                            .unwrap_or(0);
                        if consuming >= 1 && total > 1 {
                            val_str = format!("{}.clone()", val_str);
                        }
                    }
                }
                // Use `let mut` for variables rebound inside for loops
                let mutability = if let Pat::Var(name) = pat {
                    if self.mutable_vars.contains(name.as_str()) {
                        "mut "
                    } else {
                        ""
                    }
                } else {
                    ""
                };
                // Record type for handler capture
                if let Pat::Var(name) = pat {
                    if let Some(ty) = Self::infer_expr_type(value) {
                        self.var_types.insert(name.clone(), ty);
                    }
                }
                // Track actor handle variables: = c = spawn(counter, 0) → c maps to "counter"
                if let Pat::Var(var_name) = pat {
                    if let ExprKind::App(f, spawn_args) = &value.kind {
                        if let ExprKind::Var(fn_name) = &f.as_ref().kind {
                            if fn_name == "spawn" && spawn_args.len() == 2 {
                                if let ExprKind::Var(actor_name) = &&spawn_args[0].kind {
                                    self.actor_handle_vars
                                        .insert(var_name.clone(), actor_name.clone());
                                }
                            }
                        }
                    }
                }
                // Track string-typed bindings (for show() Display detection)
                if let Pat::Var(var_name) = pat {
                    if self.expr_is_string(value) {
                        self.string_typed_vars.insert(var_name.clone());
                    }
                    if self.expr_is_float(value) {
                        self.float_typed_vars.insert(var_name.clone());
                    }
                }
                format!(
                    "{}let {}{} = {};\n",
                    self.ind(),
                    mutability,
                    pat_str,
                    val_str
                )
            }
            Stmt::Expr(Expr {
                kind: ExprKind::Effect(name, args),
                ..
            }) if builtin_canonical(name) == "print" => self.emit_print(args, &self.ind()),
            // M13c: @ teardown("ScopeName") → drop scope guard + yield for cleanup
            Stmt::Expr(Expr {
                kind: ExprKind::Effect(name, args),
                ..
            }) if name == "teardown" && self.has_async => {
                if let Some(Expr {
                    kind: ExprKind::Lit(Literal::Str(scope_name)),
                    ..
                }) = args.first()
                {
                    let mut out = String::new();
                    out.push_str(&format!("{}// teardown scope {}\n", self.ind(), scope_name));
                    out.push_str(&format!("{}drop(_scope_{});\n", self.ind(), scope_name));
                    out.push_str(&format!("{}tokio::task::yield_now().await;\n", self.ind()));
                    out
                } else {
                    format!("{}// teardown (unknown scope)\n", self.ind())
                }
            }
            // Sync teardown: no guard to drop, just emit a comment
            Stmt::Expr(Expr {
                kind: ExprKind::Effect(name, args),
                ..
            }) if name == "teardown" && !self.has_async => {
                if let Some(Expr {
                    kind: ExprKind::Lit(Literal::Str(scope_name)),
                    ..
                }) = args.first()
                {
                    format!(
                        "{}// teardown scope {} (sync — no guard)\n",
                        self.ind(),
                        scope_name
                    )
                } else {
                    format!("{}// teardown (unknown scope)\n", self.ind())
                }
            }
            // M13c: @ complete(subject) → drop sender to close channel
            Stmt::Expr(Expr {
                kind: ExprKind::Effect(name, args),
                ..
            }) if name == "complete" && self.has_async => {
                if let Some(Expr {
                    kind: ExprKind::Var(subj),
                    ..
                }) = args.first()
                {
                    if self.subject_vars.contains(subj.as_str()) {
                        return format!("{}drop({});\n", self.ind(), subj);
                    }
                }
                format!(
                    "{}{};\n",
                    self.ind(),
                    self.emit_expr(&ExprKind::Effect(name.clone(), args.clone()).into())
                )
            }
            Stmt::Expr(expr) => {
                format!("{}{};\n", self.ind(), self.emit_expr(expr))
            }
            Stmt::Annot(name, _) if name == "comptime" => {
                String::new() // suppress — already processed in comptime pass
            }
            Stmt::Annot(name, _) => {
                format!("{}// @{}\n", self.ind(), name)
            }
            Stmt::Rule(rule) => {
                // M13c: scope codegen — emit scope body with lifecycle guard
                if let Rule::Scope { name, body } = rule {
                    let mut out = String::new();
                    out.push_str(&format!("{}// | scope {}\n", self.ind(), name));

                    // Collect scope bindings for struct generation
                    let mut scope_binds: Vec<(String, String)> = Vec::new(); // (name, value_expr)
                    let mut other_stmts: Vec<&Stmt> = Vec::new();
                    for s in body {
                        match s {
                            Stmt::Bind(Pat::Var(vname), _, expr) => {
                                scope_binds.push((vname.clone(), self.emit_expr(expr)));
                            }
                            Stmt::StreamBind(vname, expr) => {
                                scope_binds.push((vname.clone(), self.emit_expr(expr)));
                            }
                            _ => other_stmts.push(s),
                        }
                    }

                    if self.has_async {
                        // Set current scope for handle registration
                        let prev_scope = self.current_scope.clone();
                        self.current_scope = Some(name.clone());
                        self.scope_handles.insert(name.clone(), Vec::new());
                        // Emit all body statements (including subject/subscription handling)
                        for s in body {
                            out.push_str(&self.emit_stmt(s));
                        }
                        // Always emit scope guard so @ teardown("Name") can drop it
                        let handles = self.scope_handles.get(name).cloned().unwrap_or_default();
                        out.push_str(&format!(
                            "{}let _scope_{} = _ScopeGuard {{ handles: vec![{}] }};\n",
                            self.ind(),
                            name,
                            handles.join(", ")
                        ));
                        self.current_scope = prev_scope;
                    } else {
                        // Sync mode: emit scope body inline
                        for s in body {
                            out.push_str(&self.emit_stmt(s));
                        }
                    }

                    // Track scope bindings for qualified access (ScopeName.field → field)
                    for (vname, _) in &scope_binds {
                        self.scope_bindings
                            .entry(name.clone())
                            .or_insert_with(Vec::new)
                            .push(vname.clone());
                    }

                    return out;
                }
                String::new() // other rules emitted as top-level functions
            }
            Stmt::Use(path) => {
                format!(
                    "{}// use {} (already emitted in header)\n",
                    self.ind(),
                    path
                )
            }
            Stmt::Import(path) => {
                format!("{}// import {} (module system)\n", self.ind(), path)
            }
            Stmt::QualifiedImport(mod_name, path) => {
                format!(
                    "{}// import {} from {} (qualified, M3b)\n",
                    self.ind(),
                    mod_name,
                    path
                )
            }
            Stmt::HashImport(hash, path) => {
                format!(
                    "{}// import #{} from {} (content-addressed)\n",
                    self.ind(),
                    hash,
                    path
                )
            }
            Stmt::Depend(crate_name, version) => {
                format!(
                    "{}// depend {} {} (external crate)\n",
                    self.ind(),
                    crate_name,
                    version
                )
            }
            Stmt::RustBlock(code) => {
                // Inline @ rust { } — emit raw Rust code
                format!("{}{}\n", self.ind(), code)
            }
            Stmt::MonadicBind(pat, _ty, value) => {
                let pat_str = self.emit_pattern_binding(pat);
                let val_str = self.emit_expr(value);
                let suffix = if self.is_effect_op_call(value) {
                    ""
                } else {
                    "?"
                };
                format!("{}let {} = {}{};\n", self.ind(), pat_str, val_str, suffix)
            }

            Stmt::StreamSub(expr, arms) => {
                let mut iter_name = self.emit_expr(expr);
                if let ExprKind::Var(name) = &expr.kind {
                    let uses = self.var_use_counts.get(name.as_str()).copied().unwrap_or(0);
                    if uses > 1 && !self.copy_vars.contains(name.as_str()) {
                        iter_name = format!("{}.clone()", iter_name);
                    }
                }

                // Classify arms
                let mut value_arms = Vec::new();
                let mut error_arm = None;
                let mut complete_arm = None;
                for arm in arms {
                    let is_complete = matches!(&arm.pat, Pat::Var(n) if n == "Complete")
                        || matches!(&arm.pat, Pat::Con(n, _) if n == "Complete");
                    let is_error = matches!(&arm.pat, Pat::Con(n, _) if n == "Err");
                    if is_complete {
                        complete_arm = Some(arm);
                    } else if is_error {
                        error_arm = Some(arm);
                    } else {
                        value_arms.push(arm);
                    }
                }

                if self.has_async && self.is_async_stream_expr(expr) {
                    self.sub_counter += 1;
                    let handle_name = format!("_sub_{}", self.sub_counter);
                    let mut out = String::new();
                    // We assume expr evaluates to an rx channel or a stream wrapper with `.subscribe()`
                    // For now, assume subject variable name directly like `name.subscribe()`
                    // If it's a stream object, it should have a `.subscribe()` method or we do it inline.
                    out.push_str(&format!(
                        "{}let mut _rx_{} = {}.subscribe();
",
                        self.ind(),
                        self.sub_counter,
                        iter_name
                    ));
                    out.push_str(&format!(
                        "{}let {} = tokio::spawn(async move {{
",
                        self.ind(),
                        handle_name
                    ));
                    self.indent += 1;
                    out.push_str(&format!(
                        "{}loop {{
",
                        self.ind()
                    ));
                    self.indent += 1;
                    out.push_str(&format!(
                        "{}match _rx_{}.recv().await {{
",
                        self.ind(),
                        self.sub_counter
                    ));
                    self.indent += 1;

                    // Values
                    for arm in &value_arms {
                        let pat_str = self.emit_pattern_match(&arm.pat);
                        out.push_str(&format!(
                            "{}Ok({}) => {{
",
                            self.ind(),
                            pat_str
                        ));
                        self.indent += 1;
                        if let Some(guard) = &arm.guard {
                            out.push_str(&format!(
                                "{}if {} {{
",
                                self.ind(),
                                self.emit_expr(guard)
                            ));
                            self.indent += 1;
                        }
                        out.push_str(&format!(
                            "{};
",
                            self.emit_expr(&arm.body)
                        ));
                        if arm.guard.is_some() {
                            self.indent -= 1;
                            out.push_str(&format!(
                                "{}}}
",
                                self.ind()
                            ));
                        }
                        self.indent -= 1;
                        out.push_str(&format!(
                            "{}}}
",
                            self.ind()
                        ));
                    }
                    if !value_arms.is_empty() {
                        // Fallback for Ok(_) if patterns don't cover everything
                        out.push_str(&format!(
                            "{}Ok(_) => {{}}
",
                            self.ind()
                        ));
                    }

                    // Error
                    out.push_str(&format!(
                        "{}Err(tokio::sync::broadcast::error::RecvError::Lagged(_n)) => {{
",
                        self.ind()
                    ));
                    self.indent += 1;
                    if let Some(arm) = error_arm {
                        if let Pat::Con(_, args) = &arm.pat {
                            if let Some(inner) = args.first() {
                                out.push_str(&format!(
                                    "{}let {} = _n.to_string();
",
                                    self.ind(),
                                    self.emit_pattern_match(inner)
                                ));
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            out.push_str(&format!(
                                "{}if {} {{
",
                                self.ind(),
                                self.emit_expr(guard)
                            ));
                            self.indent += 1;
                        }
                        out.push_str(&format!(
                            "{};
",
                            self.emit_expr(&arm.body)
                        ));
                        if arm.guard.is_some() {
                            self.indent -= 1;
                            out.push_str(&format!(
                                "{}}}
",
                                self.ind()
                            ));
                        }
                    }
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    ));

                    // Complete
                    out.push_str(&format!(
                        "{}Err(tokio::sync::broadcast::error::RecvError::Closed) => {{
",
                        self.ind()
                    ));
                    self.indent += 1;
                    if let Some(arm) = complete_arm {
                        if let Some(guard) = &arm.guard {
                            out.push_str(&format!(
                                "{}if {} {{
",
                                self.ind(),
                                self.emit_expr(guard)
                            ));
                            self.indent += 1;
                        }
                        out.push_str(&format!(
                            "{};
",
                            self.emit_expr(&arm.body)
                        ));
                        if arm.guard.is_some() {
                            self.indent -= 1;
                            out.push_str(&format!(
                                "{}}}
",
                                self.ind()
                            ));
                        }
                    }
                    out.push_str(&format!(
                        "{}break;
",
                        self.ind()
                    ));
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    ));

                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    )); // end match
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    )); // end loop
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}});
",
                        self.ind()
                    )); // end spawn

                    if let Some(scope) = &self.current_scope.clone() {
                        self.scope_handles
                            .entry(scope.clone())
                            .or_default()
                            .push(handle_name);
                    }
                    return out;
                } else {
                    // Sync mode execution for StreamSub
                    let mut out = String::new();
                    out.push_str(&format!(
                        "{}for _item in {}.into_iter() {{
",
                        self.ind(),
                        iter_name
                    ));
                    self.indent += 1;
                    out.push_str(&format!(
                        "{}match _item {{
",
                        self.ind()
                    ));
                    self.indent += 1;
                    for arm in &value_arms {
                        let pat_str = self.emit_pattern_match(&arm.pat);
                        if let Some(guard) = &arm.guard {
                            out.push_str(&format!(
                                "{}{} if {} => {{
",
                                self.ind(),
                                pat_str,
                                self.emit_expr(guard)
                            ));
                        } else {
                            out.push_str(&format!(
                                "{}{} => {{
",
                                self.ind(),
                                pat_str
                            ));
                        }
                        self.indent += 1;
                        out.push_str(&format!(
                            "{};
",
                            self.emit_expr(&arm.body)
                        ));
                        self.indent -= 1;
                        out.push_str(&format!(
                            "{}}}
",
                            self.ind()
                        ));
                    }
                    out.push_str(&format!(
                        "{}_ => {{}}
",
                        self.ind()
                    ));
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    ));
                    self.indent -= 1;
                    out.push_str(&format!(
                        "{}}}
",
                        self.ind()
                    ));

                    if let Some(arm) = complete_arm {
                        out.push_str(&format!(
                            "{{
"
                        ));
                        self.indent += 1;
                        out.push_str(&format!(
                            "{};
",
                            self.emit_expr(&arm.body)
                        ));
                        self.indent -= 1;
                        out.push_str(&format!(
                            "{}}}
",
                            self.ind()
                        ));
                    }
                    return out;
                }
            }

            Stmt::For(var, iter_expr, body) => {
                // M13c: async subscription — for x in subject spawns a subscriber task
                let is_subject_iter = if let ExprKind::Var(name) = &iter_expr.kind {
                    self.has_async && self.subject_vars.contains(name.as_str())
                } else {
                    false
                };

                if is_subject_iter {
                    let iter_name = if let ExprKind::Var(n) = &iter_expr.kind {
                        n.clone()
                    } else {
                        unreachable!()
                    };
                    self.sub_counter += 1;
                    let handle_name = format!("_sub_{}", self.sub_counter);
                    let mut out = String::new();
                    out.push_str(&format!(
                        "{}let mut _rx_{} = {}.subscribe();\n",
                        self.ind(),
                        self.sub_counter,
                        iter_name
                    ));
                    out.push_str(&format!(
                        "{}let {} = tokio::spawn(async move {{\n",
                        self.ind(),
                        handle_name
                    ));
                    self.indent += 1;
                    out.push_str(&format!(
                        "{}while let Ok({}) = _rx_{}.recv().await {{\n",
                        self.ind(),
                        var,
                        self.sub_counter
                    ));
                    self.indent += 1;
                    for s in body {
                        out.push_str(&self.emit_stmt(s));
                    }
                    self.indent -= 1;
                    out.push_str(&format!("{}}}\n", self.ind()));
                    self.indent -= 1;
                    out.push_str(&format!("{}}});\n", self.ind()));
                    // Register handle with current scope if inside one
                    if let Some(scope) = &self.current_scope.clone() {
                        self.scope_handles
                            .entry(scope.clone())
                            .or_default()
                            .push(handle_name);
                    }
                    return out;
                }

                let mut iter_str = self.emit_expr(iter_expr);
                // For-loop consumes the iterable via into_iter(). If the variable is used
                // elsewhere (multi-use), we need to clone to avoid move errors.
                if let ExprKind::Var(name) = &iter_expr.kind {
                    let uses = self.var_use_counts.get(name.as_str()).copied().unwrap_or(0);
                    if uses > 1 && !self.copy_vars.contains(name.as_str()) {
                        iter_str = format!("{}.clone()", iter_str);
                    }
                }
                // Use mutable_vars (correctly computed by collect_mutable_vars)
                // to identify accumulators that need assignment instead of let binding
                let rebound_vars: Vec<String> = body
                    .iter()
                    .filter_map(|s| {
                        if let Stmt::Bind(Pat::Var(name), _, _) = s {
                            if self.mutable_vars.contains(name.as_str()) {
                                return Some(name.clone());
                            }
                        }
                        None
                    })
                    .collect();
                let mut out = String::new();
                // When iterating over a borrowed param (&Vec), items are &T.
                // Use `for &var` to auto-deref so the loop variable is T, not &T.
                let is_borrowed_iter = if let ExprKind::Var(name) = &iter_expr.kind {
                    self.current_borrow_params.contains(name.as_str())
                } else {
                    false
                };
                let loop_var = if is_borrowed_iter {
                    format!("&{}", var)
                } else {
                    var.clone()
                };
                out.push_str(&format!(
                    "{}for {} in {} {{\n",
                    self.ind(),
                    loop_var,
                    iter_str
                ));
                self.indent += 1;
                for s in body {
                    // If this binding rebinds an accumulator, emit as assignment
                    if let Stmt::Bind(Pat::Var(name), _, value) = s {
                        if rebound_vars.contains(name) {
                            let val_str = self.emit_expr(value);
                            out.push_str(&format!("{}{} = {};\n", self.ind(), name, val_str));
                            continue;
                        }
                    }
                    out.push_str(&self.emit_stmt(s));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}\n", self.ind()));
                out
            }
            Stmt::Send(target, msg) => {
                let t = self.emit_expr(target);
                let m = self.emit_expr(msg);
                // M13c: subject send emits broadcast send + yield for deterministic ordering
                if self.has_async {
                    let var_name = if let ExprKind::Var(n) = &target.kind {
                        n.clone()
                    } else {
                        t.clone()
                    };
                    if self.subject_vars.contains(&var_name) {
                        let mut out = format!("{}let _ = {}.send({});\n", self.ind(), t, m);
                        out.push_str(&format!("{}tokio::task::yield_now().await;\n", self.ind()));
                        return out;
                    }
                    // Actor send: yield after send for deterministic message processing
                    if self.actor_handle_vars.contains_key(&var_name) {
                        let mut out = format!("{}{}.send({}).unwrap();\n", self.ind(), t, m);
                        out.push_str(&format!("{}tokio::task::yield_now().await;\n", self.ind()));
                        return out;
                    }
                }
                // Sync subject: push to Vec
                let var_name = if let ExprKind::Var(n) = &target.kind {
                    n.clone()
                } else {
                    t.clone()
                };
                if self.sync_subject_vars.contains(&var_name) {
                    return format!("{}{}.push({});\n", self.ind(), t, m);
                }
                format!("{}{}.send({}).unwrap();\n", self.ind(), t, m)
            }
            Stmt::StreamBind(name, expr) => {
                // M13c: detect subject() calls → emit broadcast channel
                if self.has_async {
                    let is_subject = matches!(expr.kind, ExprKind::App(ref f, _) if matches!(f.as_ref().kind, ExprKind::Var(ref n) if n == "subject"));
                    if is_subject {
                        self.subject_vars.insert(name.clone());
                        let mut out = String::new();
                        // Extract initial value if provided: subject(val) or subject()
                        // subject() → no initial, subject(val) → initial, subject(val, n) → initial + replay
                        let initial_val = if let ExprKind::App(_, args) = &expr.kind {
                            if !args.is_empty() {
                                Some(self.emit_expr(&args[0]))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let elem_type = self
                            .subject_elem_type
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| "i64".to_string());
                        out.push_str(&format!(
                            "{}let ({}, _) = broadcast::channel::<{}>(256);\n",
                            self.ind(),
                            name,
                            elem_type
                        ));
                        if let Some(init) = initial_val {
                            out.push_str(&format!(
                                "{}let _ = {}.send({});\n",
                                self.ind(),
                                name,
                                init
                            ));
                        }
                        return out;
                    }
                }
                // Sync mode: Vec-based stream binding
                // Track sync subjects for Send (push) and field access (.count, .latest)
                let is_subject = matches!(expr.kind, ExprKind::App(ref f, _) if matches!(f.as_ref().kind, ExprKind::Var(ref n) if n == "subject"));
                if is_subject {
                    self.sync_subject_vars.insert(name.clone());
                }
                let val = self.emit_expr(expr);
                let mutability = if is_subject { "mut " } else { "" };
                format!("{}let {}{} = {};\n", self.ind(), mutability, name, val)
            }
            Stmt::Invariant {
                name,
                subject,
                predicate,
            } => {
                // Store invariant for later ? verification — no assertion here,
                // the ? rune is where verification actually happens
                self.codegen_invariants
                    .insert(name.clone(), (subject.clone(), predicate.clone()));
                format!(
                    "{}// | {}: invariant (checked by ? rune)\n",
                    self.ind(),
                    name
                )
            }
            Stmt::Prove {
                name,
                capture,
                pass_block,
                else_block,
            } => {
                // ? name → runtime verification with inlined predicate
                let mut out = String::new();

                // Collect target invariants: single name or "all"
                let targets: Vec<(String, Expr, Expr)> = if name == "all" {
                    self.codegen_invariants
                        .iter()
                        .map(|(n, (s, p))| (n.clone(), s.clone(), p.clone()))
                        .collect()
                } else if let Some((s, p)) = self.codegen_invariants.get(name) {
                    vec![(name.clone(), s.clone(), p.clone())]
                } else {
                    // Fallback: emit a compile-time warning comment
                    out.push_str(&format!(
                        "{}// ? {}: invariant not found\n",
                        self.ind(),
                        name
                    ));
                    return out;
                };

                // For "? all" with blocks: check all invariants, run block once
                let has_blocks = pass_block.is_some() || else_block.is_some();
                if name == "all" && has_blocks && targets.len() > 1 {
                    // Combine all predicates into one boolean
                    let combined: Vec<String> = targets
                        .iter()
                        .map(|(_, _, p)| format!("({})", self.emit_expr(p)))
                        .collect();
                    let all_pred = combined.join(" && ");
                    out.push_str(&format!("{}// ? all\n", self.ind()));

                    match (pass_block, else_block) {
                        (Some(pass), None) => {
                            out.push_str(&format!("{}if {} {{\n", self.ind(), all_pred));
                            self.indent += 1;
                            for s in pass {
                                out.push_str(&self.emit_stmt(s));
                            }
                            self.indent -= 1;
                            out.push_str(&format!("{}}} else {{\n", self.ind()));
                            self.indent += 1;
                            out.push_str(&format!("{}panic!(\"? all FAILED\");\n", self.ind()));
                            self.indent -= 1;
                            out.push_str(&format!("{}}}\n", self.ind()));
                        }
                        (None, Some(fail)) => {
                            out.push_str(&format!("{}if !({}) {{\n", self.ind(), all_pred));
                            self.indent += 1;
                            for s in fail {
                                out.push_str(&self.emit_stmt(s));
                            }
                            self.indent -= 1;
                            out.push_str(&format!("{}}}\n", self.ind()));
                        }
                        (Some(pass), Some(fail)) => {
                            out.push_str(&format!("{}if {} {{\n", self.ind(), all_pred));
                            self.indent += 1;
                            for s in pass {
                                out.push_str(&self.emit_stmt(s));
                            }
                            self.indent -= 1;
                            out.push_str(&format!("{}}} else {{\n", self.ind()));
                            self.indent += 1;
                            for s in fail {
                                out.push_str(&self.emit_stmt(s));
                            }
                            self.indent -= 1;
                            out.push_str(&format!("{}}}\n", self.ind()));
                        }
                        _ => {}
                    }
                } else {
                    for (inv_name, subject, predicate) in &targets {
                        let pred_str = self.emit_expr(predicate);
                        let subj_str = self.emit_expr(subject);

                        // Bind capture variable if requested
                        if let Some(cap) = capture {
                            out.push_str(&format!(
                                "{}let {} = {}.clone();\n",
                                self.ind(),
                                cap,
                                subj_str
                            ));
                        }

                        match (pass_block, else_block) {
                            (None, None) => {
                                // Bare ? name — assert and panic on failure
                                out.push_str(&format!(
                                    "{}// ? {}\n{}assert!({}, \"? {} FAILED\");\n",
                                    self.ind(),
                                    inv_name,
                                    self.ind(),
                                    pred_str,
                                    inv_name
                                ));
                            }
                            (Some(pass), None) => {
                                // ? name -> { pass } — run pass block, panic on failure
                                out.push_str(&format!(
                                    "{}// ? {}\n{}if {} {{\n",
                                    self.ind(),
                                    inv_name,
                                    self.ind(),
                                    pred_str
                                ));
                                self.indent += 1;
                                for s in pass {
                                    out.push_str(&self.emit_stmt(s));
                                }
                                self.indent -= 1;
                                out.push_str(&format!("{}}} else {{\n", self.ind()));
                                self.indent += 1;
                                out.push_str(&format!(
                                    "{}panic!(\"? {} FAILED\");\n",
                                    self.ind(),
                                    inv_name
                                ));
                                self.indent -= 1;
                                out.push_str(&format!("{}}}\n", self.ind()));
                            }
                            (None, Some(fail)) => {
                                // ? name else { fail } — custom fail handler, no halt
                                out.push_str(&format!(
                                    "{}// ? {}\n{}if !({}) {{\n",
                                    self.ind(),
                                    inv_name,
                                    self.ind(),
                                    pred_str
                                ));
                                self.indent += 1;
                                for s in fail {
                                    out.push_str(&self.emit_stmt(s));
                                }
                                self.indent -= 1;
                                out.push_str(&format!("{}}}\n", self.ind()));
                            }
                            (Some(pass), Some(fail)) => {
                                // ? name -> { pass } else { fail } — both branches, no halt
                                out.push_str(&format!(
                                    "{}// ? {}\n{}if {} {{\n",
                                    self.ind(),
                                    inv_name,
                                    self.ind(),
                                    pred_str
                                ));
                                self.indent += 1;
                                for s in pass {
                                    out.push_str(&self.emit_stmt(s));
                                }
                                self.indent -= 1;
                                out.push_str(&format!("{}}} else {{\n", self.ind()));
                                self.indent += 1;
                                for s in fail {
                                    out.push_str(&self.emit_stmt(s));
                                }
                                self.indent -= 1;
                                out.push_str(&format!("{}}}\n", self.ind()));
                            }
                        }
                    }
                }
                out
            }

            // ---- Persist: assert / retract / abort ----
            Stmt::Assert(type_name, args) => {
                let mut out = String::new();
                let sname = sanitize_name(type_name);
                if self.types.stored_types.contains(type_name.as_str()) {
                    // Object store: serialize struct to JSON, INSERT OR REPLACE
                    let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                    // Build struct literal with named fields (named struct) or positional (tuple struct)
                    let is_positional = self
                        .types
                        .variant_positional
                        .get(type_name.as_str())
                        .copied()
                        .unwrap_or(false);
                    let construct = if is_positional {
                        format!("{}({})", sname, arg_strs.join(", "))
                    } else {
                        // Named fields
                        let fields = self
                            .types
                            .variant_fields
                            .get(type_name.as_str())
                            .cloned()
                            .unwrap_or_default();
                        let pairs: Vec<String> = fields
                            .iter()
                            .zip(arg_strs.iter())
                            .map(|(f, v)| format!("{}: {}", sanitize_name(f), v))
                            .collect();
                        format!("{} {{ {} }}", sname, pairs.join(", "))
                    };
                    out.push_str(&format!(
                        "{}{{ // assert {} (object store)\n",
                        self.ind(),
                        type_name
                    ));
                    self.indent += 1;
                    out.push_str(&format!("{}let __val = {};\n", self.ind(), construct));
                    out.push_str(&format!("{}let __json = serde_json::to_string(&__val).expect(\"serialize failed\");\n", self.ind()));
                    // First field is the key — use Display for the key value
                    out.push_str(&format!(
                        "{}let __key = format!(\"{{}}\", __val.{});\n",
                        self.ind(),
                        self.types
                            .stored_type_key_field
                            .get(type_name.as_str())
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    ));
                    out.push_str(&format!("{}__db.lock().unwrap().execute(\"INSERT OR REPLACE INTO {} (id, data) VALUES (?1, ?2)\", rusqlite::params![__key, __json]).expect(\"assert failed\");\n",
                        self.ind(), sname.to_lowercase()));
                    self.indent -= 1;
                    out.push_str(&format!("{}}}\n", self.ind()));
                } else {
                    // Not a stored type — emit as constructor expression (future: in-memory facts)
                    let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                    out.push_str(&format!(
                        "{}// assert {}({}) — not a stored type, no-op\n",
                        self.ind(),
                        type_name,
                        arg_strs.join(", ")
                    ));
                }
                out
            }
            Stmt::Retract(type_name, args) => {
                let mut out = String::new();
                let sname = sanitize_name(type_name);
                if self.types.stored_types.contains(type_name.as_str()) {
                    // Object store: DELETE by key (first arg)
                    // For now, use first arg as key
                    if let Some(first) = args.first() {
                        let key_str = self.emit_expr(first);
                        out.push_str(&format!("{}__db.lock().unwrap().execute(\"DELETE FROM {} WHERE id = ?1\", rusqlite::params![format!(\"{{}}\", {})] ).expect(\"retract failed\");\n",
                            self.ind(), sname.to_lowercase(), key_str));
                    } else {
                        out.push_str(&format!(
                            "{}// retract {} — no arguments\n",
                            self.ind(),
                            type_name
                        ));
                    }
                } else {
                    let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                    out.push_str(&format!(
                        "{}// retract {}({}) — not a stored type, no-op\n",
                        self.ind(),
                        type_name,
                        arg_strs.join(", ")
                    ));
                }
                out
            }
            Stmt::Abort => {
                // For now, emit a comment. Transactional abort (break 'scope) comes in M26e.
                format!("{}// abort — transactional abort (M26e)\n{}panic!(\"abort outside transactional scope\");\n", self.ind(), self.ind())
            }
        }
    }

    /// Find which variant contains a named field, returns (variant_name, parent_enum_name, is_boxed)
    fn find_variant_field(&self, field: &str) -> Option<(String, String)> {
        for (variant_name, fields) in &self.types.variant_fields {
            if fields.contains(&field.to_string()) {
                if let Some(parent) = self.types.variant_parent.get(variant_name) {
                    // Only for enum types (not struct types)
                    if !self.types.struct_types.contains(parent) {
                        return Some((variant_name.clone(), parent.clone()));
                    }
                }
            }
        }
        None
    }

    /// Find ALL variants that have a given field name (for multi-variant field access)
    fn find_all_variant_fields(&self, field: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        for (variant_name, fields) in &self.types.variant_fields {
            if fields.contains(&field.to_string()) {
                if let Some(parent) = self.types.variant_parent.get(variant_name) {
                    if !self.types.struct_types.contains(parent) {
                        results.push((variant_name.clone(), parent.clone()));
                    }
                }
            }
        }
        results
    }

    /// Check if a named field in a variant is boxed (recursive)
    fn is_field_boxed(&self, variant_name: &str, field: &str) -> bool {
        if let Some(fields) = self.types.variant_fields.get(variant_name) {
            if let Some(idx) = fields.iter().position(|f| f == field) {
                if let Some(boxed) = self.types.variant_boxed_args.get(variant_name) {
                    return boxed.contains(&idx);
                }
            }
        }
        false
    }

    /// Stub for field index lookup — currently covered by find_variant_field
    fn find_field_index_in_any_variant(&self, _field: &str) -> Option<(String, String)> {
        None
    }

    /// Rewrite fn_name(self) calls to self.fn_name() for trait default bodies
    fn rewrite_self_calls(expr: &Expr) -> Expr {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                    // Check if first arg is Var("self")
                    if args.len() >= 1 {
                        if let ExprKind::Var(arg_name) = &&args[0].kind {
                            if arg_name == "self" {
                                // Rewrite: fn_name(self, ...) → self.fn_name(...)
                                let remaining_args: Vec<Expr> = args[1..]
                                    .iter()
                                    .map(|a| Self::rewrite_self_calls(a))
                                    .collect();
                                return ExprKind::App(
                                    Box::new(
                                        ExprKind::Field(
                                            Box::new(ExprKind::Var("self".into()).into()),
                                            fn_name.clone(),
                                        )
                                        .into(),
                                    ),
                                    remaining_args,
                                )
                                .into();
                            }
                        }
                    }
                }
                ExprKind::App(
                    Box::new(Self::rewrite_self_calls(func)),
                    args.iter().map(|a| Self::rewrite_self_calls(a)).collect(),
                )
                .into()
            }
            ExprKind::BinOp(op, l, r) => ExprKind::BinOp(
                op.clone(),
                Box::new(Self::rewrite_self_calls(l)),
                Box::new(Self::rewrite_self_calls(r)),
            )
            .into(),
            ExprKind::If(c, t, e) => ExprKind::If(
                Box::new(Self::rewrite_self_calls(c)),
                Box::new(Self::rewrite_self_calls(t)),
                Box::new(Self::rewrite_self_calls(e)),
            )
            .into(),
            ExprKind::Block(stmts) => ExprKind::Block(
                stmts
                    .iter()
                    .map(|s| match s {
                        Stmt::Expr(e) => Stmt::Expr(Self::rewrite_self_calls(e)),
                        Stmt::Bind(p, t, e) => {
                            Stmt::Bind(p.clone(), t.clone(), Self::rewrite_self_calls(e))
                        }
                        other => other.clone(),
                    })
                    .collect(),
            )
            .into(),
            _ => expr.clone(),
        }
    }

    /// M3b: Check if an expression is a module path (for :: emission)
    /// Returns true for Var("ModuleName") where ModuleName is a known module,
    /// or for Field(module_path, "SubModule") chains.
    fn is_module_path(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Var(name) => self.types.known_modules.contains(name),
            ExprKind::Field(obj, _field) => self.is_module_path(obj),
            _ => false,
        }
    }

    /// M3b: Emit a module path with :: separators (App::Utils)
    fn emit_module_path(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => sanitize_name(name),
            ExprKind::Field(obj, field) => {
                format!("{}::{}", self.emit_module_path(obj), sanitize_name(field))
            }
            _ => self.emit_expr(expr),
        }
    }

    /// Check if an expression is likely a Copy type (no clone needed)
    /// Conservative: only skip clone for literals
    fn is_copy_type_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr.kind,
            ExprKind::Lit(Literal::Int(_))
                | ExprKind::Lit(Literal::Float(_))
                | ExprKind::Lit(Literal::Bool(_))
                | ExprKind::Lit(Literal::Char(_))
        )
    }

    /// Scan statements for variables passed to inout parameters and mark them as mutable
    fn collect_inout_mutables_stmts(&mut self, stmts: &[&Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr)
                | Stmt::Bind(_, _, expr)
                | Stmt::MonadicBind(_, _, expr)
                | Stmt::StreamBind(_, expr) => {
                    self.collect_inout_mutables_expr(expr);
                }
                Stmt::StreamSub(expr, arms) => {
                    self.collect_inout_mutables_expr(expr);
                    for arm in arms {
                        if let Some(g) = &arm.guard {
                            self.collect_inout_mutables_expr(g);
                        }
                        self.collect_inout_mutables_expr(&arm.body);
                    }
                }
                Stmt::For(_, iter, body) => {
                    self.collect_inout_mutables_expr(iter);
                    let body_refs: Vec<&Stmt> = body.iter().collect();
                    self.collect_inout_mutables_stmts(&body_refs);
                }
                _ => {}
            }
        }
    }

    fn collect_inout_mutables_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                    if let Some(flags) = self.types.inout_params.get(fn_name.as_str()).cloned() {
                        for (idx, arg) in args.iter().enumerate() {
                            if flags.get(idx).copied().unwrap_or(false) {
                                if let ExprKind::Var(var_name) = &arg.kind {
                                    self.mutable_vars.insert(var_name.clone());
                                }
                            }
                        }
                    }
                }
                self.collect_inout_mutables_expr(func);
                for a in args {
                    self.collect_inout_mutables_expr(a);
                }
            }
            ExprKind::BinOp(_, l, r) => {
                self.collect_inout_mutables_expr(l);
                self.collect_inout_mutables_expr(r);
            }
            ExprKind::If(c, t, e) => {
                self.collect_inout_mutables_expr(c);
                self.collect_inout_mutables_expr(t);
                self.collect_inout_mutables_expr(e);
            }
            ExprKind::Block(stmts) => {
                let refs: Vec<&Stmt> = stmts.iter().collect();
                self.collect_inout_mutables_stmts(&refs);
            }
            _ => {}
        }
    }

    /// Check if an expression involves float values (for safe div/mod — floats don't panic on /0)
    fn expr_is_float(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Lit(Literal::Float(_)) => true,
            ExprKind::Var(name) => self.float_typed_vars.contains(name.as_str()),
            ExprKind::BinOp(_, lhs, rhs) => self.expr_is_float(lhs) || self.expr_is_float(rhs),
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    if matches!(
                        name.as_str(),
                        "to_float"
                            | "sqrt"
                            | "exp"
                            | "ln"
                            | "pow"
                            | "abs"
                            | "round"
                            | "floor"
                            | "min_f"
                            | "max_f"
                            | "parse_float"
                            | "phi"
                            | "mint"
                            ) {
                        return true;
                    }
                    // foldl with float initial value → result is float
                    if name == "foldl" && args.len() >= 2 && self.expr_is_float(&args[1]) {
                        return true;
                    }
                }
                false
            }
            ExprKind::If(_, then_, else_) => self.expr_is_float(then_) || self.expr_is_float(else_),
            _ => false,
        }
    }

    /// Check if a variable is used as a tuple argument to fst/snd in an expression.
    fn expr_uses_as_tuple(expr: &Expr, var_name: &str) -> bool {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if let ExprKind::Var(f) = &func.as_ref().kind {
                    if (f == "fst" || f == "snd") && args.len() == 1 {
                        if let ExprKind::Var(a) = &&args[0].kind {
                            if a == var_name {
                                return true;
                            }
                        }
                    }
                }
                args.iter().any(|a| Self::expr_uses_as_tuple(a, var_name))
                    || Self::expr_uses_as_tuple(func, var_name)
            }
            ExprKind::BinOp(_, lhs, rhs) => {
                Self::expr_uses_as_tuple(lhs, var_name) || Self::expr_uses_as_tuple(rhs, var_name)
            }
            ExprKind::If(c, t, e) => {
                Self::expr_uses_as_tuple(c, var_name)
                    || Self::expr_uses_as_tuple(t, var_name)
                    || Self::expr_uses_as_tuple(e, var_name)
            }
            _ => false,
        }
    }

    /// Collect all field names accessed on a given variable in an expression tree.
    /// e.g. for `w.temp > 35.0 && w.city == "X"`, calling with var_name="w" returns {"temp","city"}
    fn collect_field_accesses(&self, expr: &Expr, var_name: &str, fields: &mut BTreeSet<String>) {
        match &expr.kind {
            ExprKind::Field(obj, field) => {
                if let ExprKind::Var(v) = &obj.as_ref().kind {
                    if v == var_name {
                        fields.insert(field.clone());
                    }
                }
                self.collect_field_accesses(obj, var_name, fields);
            }
            ExprKind::BinOp(_, lhs, rhs) => {
                self.collect_field_accesses(lhs, var_name, fields);
                self.collect_field_accesses(rhs, var_name, fields);
            }
            ExprKind::App(func, args) => {
                self.collect_field_accesses(func, var_name, fields);
                for a in args {
                    self.collect_field_accesses(a, var_name, fields);
                }
            }
            ExprKind::If(c, t, e) => {
                self.collect_field_accesses(c, var_name, fields);
                self.collect_field_accesses(t, var_name, fields);
                self.collect_field_accesses(e, var_name, fields);
            }
            ExprKind::UnOp(_, inner) => {
                self.collect_field_accesses(inner, var_name, fields);
            }
            _ => {}
        }
    }

    /// Infer the struct/enum type of a rule parameter by matching field access patterns
    /// against known struct definitions in variant_fields.
    fn infer_param_type_from_fields(&self, param: &str, rules: &[&Rule]) -> Option<String> {
        let mut accessed_fields = BTreeSet::new();
        for rule in rules {
            let (value, condition) = match rule {
                Rule::Default {
                    value, condition, ..
                } => (Some(value), condition.as_ref()),
                Rule::Exception {
                    value, condition, ..
                } => (Some(value), condition.as_ref()),
                Rule::Clause { body, .. } => (body.as_ref(), None),
                _ => (None, None),
            };
            if let Some(v) = value {
                self.collect_field_accesses(v, param, &mut accessed_fields);
            }
            if let Some(c) = condition {
                self.collect_field_accesses(c, param, &mut accessed_fields);
            }
        }
        if !accessed_fields.is_empty() {
            // Find the struct/variant whose fields are a superset of accessed_fields
            for (type_name, fields) in &self.types.variant_fields {
                let field_set: BTreeSet<String> = fields.iter().cloned().collect();
                if accessed_fields.is_subset(&field_set) {
                    return Some(type_name.clone());
                }
            }
        }
        // Fallback: infer type from match patterns on this parameter
        // e.g. match v { HeartRate(bpm: b) -> ... } → look up HeartRate in variant_parent
        let rule_exprs: Vec<Vec<&Expr>> = rules
            .iter()
            .map(|rule| match rule {
                Rule::Default {
                    value, condition, ..
                } => {
                    let mut v = vec![value];
                    if let Some(c) = condition {
                        v.push(c);
                    }
                    v
                }
                Rule::Exception {
                    value, condition, ..
                } => {
                    let mut v = vec![value];
                    if let Some(c) = condition {
                        v.push(c);
                    }
                    v
                }
                Rule::Clause { body, .. } => body.iter().collect(),
                _ => vec![],
            })
            .collect();
        for exprs in &rule_exprs {
            for expr in exprs {
                if let Some(ty) = self.infer_param_type_from_match(param, expr) {
                    return Some(ty);
                }
            }
        }
        // Fallback 3: infer type from `param == Constructor` comparisons in conditions
        for rule in rules {
            let condition = match rule {
                Rule::Default {
                    condition: Some(c), ..
                }
                | Rule::Exception {
                    condition: Some(c), ..
                } => c,
                _ => continue,
            };
            if let Some(ty) = self.infer_param_type_from_comparison(param, condition) {
                return Some(ty);
            }
        }
        None
    }

    /// Infer a parameter's type from how it's used in rule body expressions.
    /// Detects comparisons (>=, <=, >, <) and arithmetic (+, -, *, /) → i64 or f64.
    fn infer_param_type_from_body(&self, param: &str, rules: &[&Rule]) -> Option<String> {
        for rule in rules {
            let body = match rule {
                Rule::Clause { body: Some(b), .. } => b,
                Rule::Default { value, .. } | Rule::Exception { value, .. } => value,
                _ => continue,
            };
            if let Some(ty) = self.infer_param_type_from_expr_usage(param, body) {
                return Some(ty);
            }
        }
        None
    }

    /// Recursively check if `param` is used in a context that reveals its type.
    fn infer_param_type_from_expr_usage(&self, param: &str, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::BinOp(op, lhs, rhs) => {
                let is_numeric_op = matches!(
                    op.as_str(),
                    ">=" | "<=" | ">" | "<" | "+" | "-" | "*" | "/" | "%"
                );
                if is_numeric_op {
                    let lhs_is_param =
                        matches!(lhs.as_ref().kind, ExprKind::Var(ref n) if n == param);
                    let rhs_is_param =
                        matches!(rhs.as_ref().kind, ExprKind::Var(ref n) if n == param);
                    if lhs_is_param || rhs_is_param {
                        // Check the other operand for type hints
                        let other = if lhs_is_param { rhs } else { lhs };
                        if let ExprKind::Lit(Literal::Float(_)) = &other.as_ref().kind {
                            return Some("f64".to_string());
                        }
                        if let ExprKind::Lit(Literal::Int(_)) = &other.as_ref().kind {
                            return Some("i64".to_string());
                        }
                        // Check if the other side is a known literal binding
                        if let ExprKind::Var(other_name) = &other.as_ref().kind {
                            if let Some((_, ty)) = self.types.literal_bindings.get(other_name) {
                                return Some(ty.clone());
                            }
                        }
                        // Default to i64 for numeric ops
                        return Some("i64".to_string());
                    }
                }
                // Recurse into both sides
                self.infer_param_type_from_expr_usage(param, lhs)
                    .or_else(|| self.infer_param_type_from_expr_usage(param, rhs))
            }
            ExprKind::App(func, args) => {
                // Check if param is passed to a known Prolog function → inherit that type
                if let ExprKind::Var(fn_name) = &func.as_ref().kind {
                    if let Some(fn_param_types) = self.types.prolog_rule_fns.get(fn_name.as_str()) {
                        for (i, a) in args.iter().enumerate() {
                            if let ExprKind::Var(n) = &a.kind {
                                if n == param && i < fn_param_types.len() {
                                    return Some(fn_param_types[i].clone());
                                }
                            }
                        }
                    }
                }
                // Recurse into subexpressions
                for a in args {
                    if let Some(ty) = self.infer_param_type_from_expr_usage(param, a) {
                        return Some(ty);
                    }
                }
                None
            }
            ExprKind::If(cond, then_br, else_br) => self
                .infer_param_type_from_expr_usage(param, cond)
                .or_else(|| self.infer_param_type_from_expr_usage(param, then_br))
                .or_else(|| self.infer_param_type_from_expr_usage(param, else_br)),
            ExprKind::UnOp(_, inner) => self.infer_param_type_from_expr_usage(param, inner),
            _ => None,
        }
    }

    /// Walk an expression tree looking for `param == Constructor` or `Constructor == param`.
    /// If the constructor's parent type is known, return it.
    fn infer_param_type_from_comparison(&self, param: &str, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::BinOp(op, lhs, rhs) if op == "==" || op == "!=" => {
                // param == Constructor
                if let ExprKind::Var(v) = &lhs.as_ref().kind {
                    if v == param {
                        if let ExprKind::Var(con) = &rhs.as_ref().kind {
                            if let Some(parent) = self.types.variant_parent.get(con.as_str()) {
                                return Some(parent.clone());
                            }
                        }
                    }
                }
                // Constructor == param
                if let ExprKind::Var(v) = &rhs.as_ref().kind {
                    if v == param {
                        if let ExprKind::Var(con) = &lhs.as_ref().kind {
                            if let Some(parent) = self.types.variant_parent.get(con.as_str()) {
                                return Some(parent.clone());
                            }
                        }
                    }
                }
                None
            }
            ExprKind::BinOp(_, lhs, rhs) => self
                .infer_param_type_from_comparison(param, lhs)
                .or_else(|| self.infer_param_type_from_comparison(param, rhs)),
            _ => None,
        }
    }

    /// Walk an expression tree looking for `match param { Constructor(...) -> ... }` patterns.
    fn infer_param_type_from_match(&self, param: &str, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Match(scrutinee, arms) => {
                if let ExprKind::Var(v) = &scrutinee.as_ref().kind {
                    if v == param {
                        for arm in arms {
                            let con_name = match &arm.pat {
                                Pat::Con(name, _) => Some(name.as_str()),
                                Pat::NamedCon(name, _) => Some(name.as_str()),
                                _ => None,
                            };
                            if let Some(name) = con_name {
                                if let Some(parent) = self.types.variant_parent.get(name) {
                                    return Some(parent.clone());
                                }
                            }
                        }
                    }
                }
                for arm in arms {
                    if let Some(ty) = self.infer_param_type_from_match(param, &arm.body) {
                        return Some(ty);
                    }
                }
                None
            }
            ExprKind::App(func, args) => {
                if let Some(ty) = self.infer_param_type_from_match(param, func) {
                    return Some(ty);
                }
                for a in args {
                    if let Some(ty) = self.infer_param_type_from_match(param, a) {
                        return Some(ty);
                    }
                }
                None
            }
            ExprKind::BinOp(_, lhs, rhs) => self
                .infer_param_type_from_match(param, lhs)
                .or_else(|| self.infer_param_type_from_match(param, rhs)),
            ExprKind::If(c, t, e) => self
                .infer_param_type_from_match(param, c)
                .or_else(|| self.infer_param_type_from_match(param, t))
                .or_else(|| self.infer_param_type_from_match(param, e)),
            _ => None,
        }
    }

    /// Infer the return type of a rule function from constructor calls in value expressions.
    fn infer_rule_return_type(&self, rules: &[&Rule]) -> Option<String> {
        for rule in rules {
            let value = match rule {
                Rule::Default { value, .. } | Rule::Exception { value, .. } => value,
                Rule::Clause {
                    body: Some(body), ..
                } => body,
                _ => continue,
            };
            if let Some(ty) = self.infer_type_from_expr(value) {
                return Some(ty);
            }
        }
        None
    }

    /// Infer a type from an expression (constructors, literals, variants)
    fn infer_type_from_expr(&self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Lit(lit) => match lit {
                Literal::Str(_) => Some("String".to_string()),
                Literal::Int(_) => Some("i64".to_string()),
                Literal::Float(_) => Some("f64".to_string()),
                Literal::Bool(_) => Some("bool".to_string()),
                _ => None,
            },
            ExprKind::Var(name) => {
                // Bare enum variant (no args), e.g. Safe, Danger
                if let Some(parent) = self.types.variant_parent.get(name.as_str()) {
                    return Some(parent.clone());
                }
                None
            }
            ExprKind::App(func, _) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Check if it's a known struct
                    if self.types.struct_types.contains(name.as_str()) {
                        return Some(name.clone());
                    }
                    // Check if it's a variant — return the parent enum name
                    if let Some(parent) = self.types.variant_parent.get(name.as_str()) {
                        return Some(parent.clone());
                    }
                }
                None
            }
            ExprKind::If(_, then_br, else_br) => self
                .infer_type_from_expr(then_br)
                .or_else(|| self.infer_type_from_expr(else_br)),
            _ => None,
        }
    }

    /// Check if an expression is purely arithmetic (uses *, +, -, /, % operators)
    fn expr_is_arithmetic(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::BinOp(op, _, _) => matches!(op.as_str(), "+" | "-" | "*" | "/" | "%"),
            ExprKind::Lit(Literal::Int(_)) | ExprKind::Lit(Literal::Float(_)) => true,
            ExprKind::Var(_) => true,
            _ => false,
        }
    }

    /// Check if an expression involves string values (string literal or concat chain)
    fn expr_is_string(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Lit(Literal::Str(_)) => true,
            ExprKind::Var(name) => {
                // Known String-typed variable in current scope
                self.string_typed_vars.contains(name.as_str())
            }
            ExprKind::BinOp(op, lhs, rhs) if op == "+" => {
                self.expr_is_string(lhs) || self.expr_is_string(rhs)
            }
            ExprKind::App(func, _) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Built-in string-returning functions
                    matches!(builtin_canonical(name.as_str()), "show" | "show_int" | "show_float" | "describe"
                        | "fizzbuzz" | "list_to_string" | "list_items" | "db_query_row")
                    // User-defined functions that return String
                    || self.string_returning_fns.contains(name.as_str())
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Collect all parts of a string concatenation chain as format! arguments
    fn collect_concat_parts(&mut self, expr: &Expr) -> Vec<String> {
        match &expr.kind {
            ExprKind::BinOp(op, lhs, rhs) if op == "+" && self.expr_is_string(expr) => {
                let mut parts = self.collect_concat_parts(lhs);
                parts.extend(self.collect_concat_parts(rhs));
                parts
            }
            ExprKind::Lit(Literal::Str(s)) => vec![format!("literal:{}", s)],
            ExprKind::Var(name) => {
                // If this variable has consuming uses elsewhere, clone for format!
                // so the original stays available for consuming function calls in the chain
                let consuming = self
                    .var_consuming_counts
                    .get(name.as_str())
                    .copied()
                    .unwrap_or(0);
                let is_copy = self.copy_vars.contains(name.as_str());
                if consuming > 0 && !is_copy {
                    vec![format!("expr:{}.clone()", sanitize_name(name))]
                } else {
                    vec![format!("expr:{}", self.emit_expr(expr))]
                }
            }
            _ => vec![format!("expr:{}", self.emit_expr(expr))],
        }
    }

    /// Emit a string concat chain as format!()
    fn emit_string_concat(&mut self, expr: &Expr) -> String {
        let parts = self.collect_concat_parts(expr);
        let mut fmt_str = String::new();
        let mut args = Vec::new();
        for part in &parts {
            if let Some(lit) = part.strip_prefix("literal:") {
                // Escape any {} in the literal for format!
                fmt_str.push_str(&lit.replace('{', "{{").replace('}', "}}"));
            } else if let Some(expr_str) = part.strip_prefix("expr:") {
                fmt_str.push_str("{}");
                args.push(expr_str.to_string());
            }
        }
        if args.is_empty() {
            format!("{:?}.to_string()", fmt_str)
        } else {
            format!("format!({:?}, {})", fmt_str, args.join(", "))
        }
    }

    /// Check if an expression evaluates to an async broadcast stream.
    /// Returns true for: subject variables, and stream ops applied to async streams.
    fn is_async_stream_expr(&self, expr: &Expr) -> bool {
        if !self.has_async {
            return false;
        }
        match &expr.kind {
            ExprKind::Var(name) => self.subject_vars.contains(name.as_str()),
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    let stream_ops = [
                        "map",
                        "filter",
                        "scan",
                        "take",
                        "skip",
                        "tap",
                        "merge",
                        "start_with",
                        "concat",
                    ];
                    if stream_ops.contains(&name.as_str()) && !args.is_empty() {
                        return self.is_async_stream_expr(&args[0]);
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Emit an async stream operator as a Rust block expression.
    /// Creates a new broadcast channel, spawns a forwarding task, returns the Sender.
    fn emit_async_stream_op(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        if !self.has_async || args.is_empty() || !self.is_async_stream_expr(&args[0]) {
            return None;
        }
        self.async_stream_counter += 1;
        let n = self.async_stream_counter;
        let source = self.emit_expr(&args[0]);

        match name {
            "map" if args.len() == 2 => {
                let f = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            let _ = __sfwd_{n}.send(({f})(__v)); \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "filter" if args.len() == 2 => {
                let f = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            if ({f})(__v.clone()) {{ let _ = __sfwd_{n}.send(__v); }} \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "scan" if args.len() == 3 => {
                let init = self.emit_expr(&args[1]);
                let f = self.emit_expr(&args[2]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        let mut __acc = {init}; \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            __acc = ({f})(__acc.clone(), __v); \
                            let _ = __sfwd_{n}.send(__acc.clone()); \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "take" if args.len() == 2 => {
                let count = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        let mut __c = 0i64; \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            if __c >= {count} {{ break; }} \
                            let _ = __sfwd_{n}.send(__v); \
                            __c += 1; \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "skip" if args.len() == 2 => {
                let count = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        let mut __c = 0i64; \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            if __c >= {count} {{ let _ = __sfwd_{n}.send(__v); }} \
                            else {{ __c += 1; }} \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "tap" if args.len() == 2 => {
                let f = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            ({f})(__v.clone()); \
                            let _ = __sfwd_{n}.send(__v); \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            "merge" if args.len() == 2 => {
                self.async_stream_counter += 1; // need two rx counters
                let n2 = self.async_stream_counter;
                let source2 = self.emit_expr(&args[1]);
                Some(format!(
                    "{{ let (__stx_{n}, _) = broadcast::channel::<i64>(256); \
                    let mut __srx_{n} = {source}.subscribe(); \
                    let mut __srx_{n2} = {source2}.subscribe(); \
                    let __sfwd_{n} = __stx_{n}.clone(); \
                    let __sfwd_{n2} = __stx_{n}.clone(); \
                    tokio::spawn(async move {{ \
                        while let Ok(__v) = __srx_{n}.recv().await {{ \
                            let _ = __sfwd_{n}.send(__v); \
                        }} \
                    }}); \
                    tokio::spawn(async move {{ \
                        while let Ok(__v) = __srx_{n2}.recv().await {{ \
                            let _ = __sfwd_{n2}.send(__v); \
                        }} \
                    }}); \
                    __stx_{n} }}"
                ))
            }
            _ => None,
        }
    }

    /// Check if an expression is a fusible Vec iterator operation (map, filter, take, skip, etc.)
    fn is_fusible_vec_op(expr: &Expr) -> bool {
        if let ExprKind::App(func, args) = &expr.kind {
            if let ExprKind::Var(name) = &func.as_ref().kind {
                let fusible = [
                    "map",
                    "filter",
                    "take",
                    "skip",
                    "flat_map",
                    "take_while",
                    "drop_while",
                ];
                return fusible.contains(&name.as_str()) && !args.is_empty();
            }
        }
        false
    }

    /// Try to fuse a chain of Vec iterator ops into a single .into_iter()...collect().
    /// Returns None if this isn't a fusible chain (i.e., source arg isn't another fusible op).
    fn try_emit_fused_chain(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let fusible = [
            "map",
            "filter",
            "take",
            "skip",
            "flat_map",
            "take_while",
            "drop_while",
        ];
        if !fusible.contains(&name) || args.is_empty() {
            return None;
        }
        // Only fuse if the first arg is ALSO a fusible op (chain of 2+)
        if !Self::is_fusible_vec_op(&args[0]) {
            return None;
        }
        // Don't fuse async stream expressions
        if self.is_async_stream_expr(&args[0]) {
            return None;
        }
        // User-defined functions shadow builtins — don't fuse those
        if self.types.user_functions.contains(name) {
            return None;
        }

        // Collect the chain: walk nested fusible ops, building (op_name, extra_args) list
        let mut chain: Vec<(&str, &[Expr])> = Vec::new();
        chain.push((name, &args[1..]));

        let mut source = &args[0];
        while let ExprKind::App(func, inner_args) = &source.kind {
            if let ExprKind::Var(inner_name) = &func.as_ref().kind {
                if fusible.contains(&inner_name.as_str())
                    && !inner_args.is_empty()
                    && !self.types.user_functions.contains(inner_name.as_str())
                {
                    chain.push((inner_name.as_str(), &inner_args[1..]));
                    source = &inner_args[0];
                    continue;
                }
            }
            break;
        }
        chain.reverse(); // innermost first

        // Emit the source (the non-fusible base expression)
        let source_str = self.emit_expr(source);
        let mut out = format!("{}.clone().into_iter()", source_str);

        // Append each operation
        for (op, extra_args) in &chain {
            match *op {
                "map" if extra_args.len() == 1 => {
                    let f = self.emit_expr(&extra_args[0]);
                    out = format!("{}.map({})", out, f);
                }
                "filter" if extra_args.len() == 1 => {
                    let f = self.emit_expr(&extra_args[0]);
                    out = format!("{}.filter(|x| ({})( x.clone()))", out, f);
                }
                "take" if extra_args.len() == 1 => {
                    let n = self.emit_expr(&extra_args[0]);
                    out = format!("{}.take(({}).max(0) as usize)", out, n);
                }
                "skip" if extra_args.len() == 1 => {
                    let n = self.emit_expr(&extra_args[0]);
                    out = format!("{}.skip(({}).max(0) as usize)", out, n);
                }
                "flat_map" if extra_args.len() == 1 => {
                    let f = self.emit_expr(&extra_args[0]);
                    out = format!("{}.flat_map({})", out, f);
                }
                "take_while" if extra_args.len() == 1 => {
                    let f = self.emit_expr(&extra_args[0]);
                    out = format!("{}.take_while(|x| ({})(x.clone()))", out, f);
                }
                "drop_while" if extra_args.len() == 1 => {
                    let f = self.emit_expr(&extra_args[0]);
                    out = format!("{}.skip_while(|x| ({})(x.clone()))", out, f);
                }
                _ => {
                    // Shouldn't happen, but fall through
                    return None;
                }
            }
        }

        out.push_str(".collect::<Vec<_>>()");
        Some(out)
    }

    fn emit_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => {
                // Nullary constructor
                if let Some(parent) = self.types.variant_parent.get(name.as_str()) {
                    if self.types.struct_types.contains(parent) {
                        return name.clone(); // struct type — no prefix
                    }
                    return format!("{}::{}", parent, name);
                }
                let sname = sanitize_name(name);
                // Phase 3b: ref-match binding — dereference because it's &T from matching on a reference
                if self.ref_match_bindings.contains(name.as_str()) {
                    return format!("(*{})", sname);
                }
                // Rule function params: clone non-Copy types to avoid ownership errors
                if self.types.rule_clone_params.contains(name.as_str()) {
                    return format!("{}.clone()", sname);
                }
                // Multi-use non-Copy variables: clone to avoid move errors
                if !self.copy_vars.contains(name.as_str())
                    && self.var_consuming_counts.get(name).copied().unwrap_or(0) > 1
                {
                    return format!("{}.clone()", sname);
                }
                sname
            }
            ExprKind::Lit(Literal::Str(s)) => format!("{:?}.to_string()", s),
            ExprKind::Lit(lit) => self.emit_literal(lit),
            ExprKind::App(func, args) => {
                // resume(val) in effect handler body → just val (the return value)
                if matches!(func.as_ref().kind, ExprKind::Var(ref n) if n == "resume") {
                    return if let Some(arg) = args.first() {
                        self.emit_expr(arg)
                    } else {
                        "()".to_string()
                    };
                }
                // Phase 1b: Check if this is a borrow-builtin BEFORE processing args
                let is_borrow_call = matches!(func.as_ref().kind, ExprKind::Var(ref n) if builtin_canonical(n) == "show");
                // Method calls: string literal args stay as &str (no .to_string())
                let is_method_call = matches!(func.as_ref().kind, ExprKind::Field(..));

                // Extract the function name (for Var or module-qualified Field access)
                let resolved_fn_name: Option<&str> = match &func.as_ref().kind {
                    ExprKind::Var(fn_name) => Some(fn_name.as_str()),
                    // M3b: Module.func() — look up func's borrow/inout params
                    ExprKind::Field(_obj, fn_name) => Some(fn_name.as_str()),
                    _ => None,
                };

                // Prolog rule functions: take &str, not String
                let is_prolog_call = resolved_fn_name
                    .map(|n| self.types.prolog_rule_fns.contains_key(n))
                    .unwrap_or(false);

                // Check if the called function has inout params
                let inout_flags =
                    resolved_fn_name.and_then(|n| self.types.inout_params.get(n).cloned());

                // Phase 2: Check if the called function has auto-borrow params
                let borrow_flags =
                    resolved_fn_name.and_then(|n| self.borrow_only_params.get(n).cloned());

                let args_str: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(idx, a)| {
                        // inout parameter: emit &mut var (no clone — passed by mutable reference)
                        let is_inout = inout_flags
                            .as_ref()
                            .map(|f| f.get(idx).copied().unwrap_or(false))
                            .unwrap_or(false);
                        if is_inout {
                            if let ExprKind::Var(n) = &a.kind {
                                // Copy-on-write: shared + inout → Arc::make_mut
                                let is_cow = resolved_fn_name
                                    .and_then(|fn_n| self.types.cow_params.get(fn_n))
                                    .map(|f| f.get(idx).copied().unwrap_or(false))
                                    .unwrap_or(false);
                                if is_cow {
                                    return format!(
                                        "std::sync::Arc::make_mut(&mut {})",
                                        sanitize_name(n)
                                    );
                                }
                                return format!("&mut {}", sanitize_name(n));
                            }
                        }

                        // Phase 2/3b: Auto-borrow parameter — emit &var (no ownership transfer)
                        // Phase 3b: If the arg variable is already a borrowed param (&T), don't double-borrow
                        let is_borrow_param = borrow_flags
                            .as_ref()
                            .map(|f| f.get(idx).copied().unwrap_or(false))
                            .unwrap_or(false);
                        if is_borrow_param {
                            if let ExprKind::Var(n) = &a.kind {
                                if self.current_borrow_params.contains(n.as_str()) {
                                    return self.emit_expr(a); // already &T, don't emit &&T
                                }
                            }
                            let s = self.emit_expr(a);
                            return format!("&{}", s);
                        }

                        let s = if is_method_call || is_prolog_call {
                            if let ExprKind::Lit(Literal::Str(ref str_val)) = &a.kind {
                                format!("{:?}", str_val) // &str, no .to_string()
                            } else if is_prolog_call {
                                // Prolog functions take &str; variables may be String — coerce with &*
                                let base = self.emit_expr(a);
                                let param_is_str = resolved_fn_name
                                    .and_then(|n| self.types.prolog_rule_fns.get(n))
                                    .and_then(|types| types.get(idx))
                                    .map(|t| t == "&str")
                                    .unwrap_or(false);
                                if param_is_str
                                    && matches!(a.kind, ExprKind::Var(ref n) if n != "_")
                                {
                                    format!("&*{}", base)
                                } else {
                                    base
                                }
                            } else {
                                self.emit_expr(a)
                            }
                        } else {
                            self.emit_expr(a)
                        };
                        // Escape analysis with borrow awareness:
                        // 1. Constructors: never clone (enum variants)
                        // 2. Copy types: never clone (i64, f64, char — free to duplicate)
                        // 3. Borrow builtins (show → .to_string()): never clone (borrows via &self)
                        // 4. Single consuming use: move (no clone)
                        // 5. Multiple consuming uses: clone
                        if let ExprKind::Var(n) = &a.kind {
                            if self.types.variant_parent.contains_key(n.as_str()) {
                                s // Constructor — never clone
                            } else if self.copy_vars.contains(n.as_str()) {
                                s // Copy type — no clone needed (free to duplicate)
                            } else if is_borrow_call {
                                s // Borrow builtin (show) — borrows via &self, no clone
                            } else if self
                                .var_consuming_counts
                                .get(n.as_str())
                                .copied()
                                .unwrap_or(0)
                                <= 1
                            {
                                s // 0-1 consuming uses — this is the only ownership transfer
                            } else {
                                format!("{}.clone()", s)
                            }
                        } else {
                            s
                        }
                    })
                    .collect();
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Builtin: show(x) — Display for strings, Debug for everything else
                    // Strings: no quotes. Vec/Option/Result: Debug works universally.
                    if builtin_canonical(name) == "show" && args_str.len() == 1 {
                        if self.expr_is_string(&args[0]) {
                            // String expression: use Display (no quotes)
                            return format!("format!(\"{{}}\", {})", args_str[0]);
                        } else {
                            // Non-string: use Debug (works for Vec, Option, Result, ADTs, etc.)
                            return format!("format!(\"{{:?}}\", {})", args_str[0]);
                        }
                    }
                    // Builtin: not(x) → !x (boolean negation / negation as failure)
                    if name == "not" && args_str.len() == 1 {
                        return format!("!({})", args_str[0]);
                    }
                    // findall(template_var, goal) → iterate fact table, collect matches
                    if name == "findall" && args.len() == 2 {
                        return self.emit_findall(&args[0], &args[1]);
                    }
                    // Prolog wildcard calls: fn(x, _) → inline fact table scan
                    if self.types.prolog_rule_fns.contains_key(name.as_str()) {
                        let has_wildcard = args
                            .iter()
                            .any(|a| matches!(a.kind, ExprKind::Var(ref n) if n == "_"));
                        if has_wildcard {
                            let table = format!("{}_FACTS", sanitize_name(name).to_uppercase());
                            let checks: Vec<String> = args
                                .iter()
                                .enumerate()
                                .filter_map(|(i, a)| {
                                    if matches!(a.kind, ExprKind::Var(ref n) if n == "_") {
                                        None
                                    } else {
                                        Some(format!("f.{} == {}", i, args_str[i]))
                                    }
                                })
                                .collect();
                            if checks.is_empty() {
                                return format!("!{}.is_empty()", table);
                            } else {
                                return format!(
                                    "{}.iter().any(|f| {})",
                                    table,
                                    checks.join(" && ")
                                );
                            }
                        }
                    }
                    // Async stream operators: intercept before sync builtin registry
                    if let Some(async_code) = self.emit_async_stream_op(name, args) {
                        return async_code;
                    }
                    // Stream fusion: fuse chains of map/filter/take/skip into single iterator
                    if let Some(fused) = self.try_emit_fused_chain(name, args) {
                        return fused;
                    }
                    // Inline lambda into filter/map to avoid double-lambda type inference failure
                    // Inline lambda into filter/map to avoid double-lambda type inference failure
                    if (name == "filter" || name == "map")
                        && args.len() == 2
                        && !self.types.user_functions.contains(name.as_str())
                    {
                        if let ExprKind::Lambda(params, body) = &&args[1].kind {
                            let coll = self.emit_expr(&args[0]);
                            let param = if params.is_empty() {
                                "_x".to_string()
                            } else {
                                sanitize_name(&params[0].name)
                            };
                            let body_code = self.emit_expr(body);
                            // Detect captured variables in the lambda body
                            let mut param_bound = BTreeSet::new();
                            for p in params {
                                param_bound.insert(p.name.clone());
                            }
                            let mut free_in_body = BTreeSet::new();
                            collect_true_free_vars(body, &mut free_in_body, &param_bound);
                            let lsp_names: BTreeSet<&str> =
                                LSP_BUILTINS.iter().map(|(n, _)| *n).collect();
                            let captured: Vec<String> = free_in_body
                                .into_iter()
                                .filter(|v| {
                                    !self.types.user_functions.contains(v.as_str())
                                        && !self.builtin_registry.contains_key(v.as_str())
                                        && !self.types.variant_parent.contains_key(v.as_str())
                                        && !self.copy_vars.contains(v.as_str())
                                        && !lsp_names.contains(v.as_str())
                                        && !matches!(
                                            v.as_str(),
                                            "true"
                                                | "false"
                                                | "True"
                                                | "False"
                                                | "None"
                                                | "Some"
                                                | "Ok"
                                                | "Err"
                                                | "Nil"
                                                | "Cons"
                                        )
                                })
                                .collect();
                            let clone_prefix = if captured.is_empty() {
                                String::new()
                            } else {
                                captured
                                    .iter()
                                    .map(|v| {
                                        format!(
                                            "let {} = {}.clone();",
                                            sanitize_name(v),
                                            sanitize_name(v)
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                    + " "
                            };
                            if name == "filter" {
                                return format!("{}.clone().into_iter().filter(|{}| {{ {} let {} = {}.clone(); {} }}).collect::<Vec<_>>()",
                                    coll, param, clone_prefix, param, param, body_code);
                            } else {
                                if clone_prefix.is_empty() {
                                    return format!(
                                        "{}.clone().into_iter().map(|{}| {}).collect::<Vec<_>>()",
                                        coll, param, body_code
                                    );
                                } else {
                                    return format!("{}.clone().into_iter().map(|{}| {{ {} {} }}).collect::<Vec<_>>()", coll, param, clone_prefix, body_code);
                                }
                            }
                        }
                        // map with function name (not lambda): wrap to handle borrow
                        if name == "map" {
                            if let ExprKind::Var(fn_name) = &&args[1].kind {
                                let coll = self.emit_expr(&args[0]);
                                let f = sanitize_name(fn_name);
                                let borrows = self
                                    .borrow_only_params
                                    .get(fn_name.as_str())
                                    .map_or(false, |flags| flags.first().copied().unwrap_or(false));
                                if borrows {
                                    return format!("{}.clone().into_iter().map(|__x| {}(&__x)).collect::<Vec<_>>()", coll, f);
                                } else {
                                    return format!("{}.clone().into_iter().map(|__x| {}(__x)).collect::<Vec<_>>()", coll, f);
                                }
                            }
                        }
                    }
                    // sort_by with lambda: inline key function to avoid type inference issues
                    if name == "sort_by"
                        && args.len() == 2
                        && !self.types.user_functions.contains(name.as_str())
                    {
                        if let ExprKind::Lambda(sort_params, sort_body) = &&args[1].kind {
                            let coll = self.emit_expr(&args[0]);
                            let sp = if sort_params.is_empty() {
                                "__e".to_string()
                            } else {
                                sanitize_name(&sort_params[0].name)
                            };
                            let saved_copy = self.copy_vars.clone();
                            self.copy_vars.insert(sort_params[0].name.clone());
                            let body_a = self.emit_expr(sort_body);
                            let body_b = self.emit_expr(sort_body);
                            self.copy_vars = saved_copy;
                            return format!("{{ let mut __v = {}.clone(); __v.sort_by(|__a, __b| {{ let {} = __a.clone(); format!(\"{{}}\", {}) }}.cmp(&{{ let {} = __b.clone(); format!(\"{{}}\", {}) }})); __v }}",
                                coll, sp, body_a, sp, body_b);
                        }
                    }
                    // Inline lambda into foldl: propagate initial value type to closure params
                    if name == "foldl"
                        && args.len() == 3
                        && !self.types.user_functions.contains(name.as_str())
                    {
                        if let ExprKind::Lambda(params, body) = &&args[2].kind {
                            let init_is_float = self.expr_is_float(&args[1]);
                            if init_is_float
                                && params.len() == 2
                                && params[0].ty.is_none()
                                && params[1].ty.is_none()
                            {
                                let coll = self.emit_expr(&args[0]);
                                let init = self.emit_expr(&args[1]);
                                let acc = sanitize_name(&params[0].name);
                                let elem = sanitize_name(&params[1].name);
                                // Mark params as float for body emission
                                self.float_typed_vars.insert(params[0].name.clone());
                                self.float_typed_vars.insert(params[1].name.clone());
                                let body_code = self.emit_expr(body);
                                return format!(
                                    "{}.clone().into_iter().fold({}, |{}: f64, {}: f64| {})",
                                    coll, init, acc, elem, body_code
                                );
                            }
                        }
                    }
                    // Builtin registry lookup — replaces 300+ lines of if-chain
                    if let Some(def) = self.builtin_registry.get(name.as_str()) {
                        if args_str.len() == def.arity
                            && (!def.shadowable
                                || !self.types.user_functions.contains(name.as_str()))
                        {
                            for &(dep_name, dep_ver) in def.deps {
                                self.cargo_deps
                                    .entry(dep_name.to_string())
                                    .or_insert(dep_ver.to_string());
                            }
                            return apply_builtin_template(def.rust_tpl, &args_str);
                        }
                    }
                    // Custom builtins that need runtime state beyond templates
                    if name == "push" && args_str.len() == 2 {
                        let is_mutable_target = if let ExprKind::Var(vn) = &args[0].kind {
                            self.mutable_vars.contains(vn.as_str())
                        } else {
                            false
                        };
                        if is_mutable_target {
                            return format!("{}.push({})", args_str[0], args_str[1]);
                        }
                        return format!(
                            "{{ let mut v = {}; v.push({}); v }}",
                            args_str[0], args_str[1]
                        );
                    }
                    if name == "subject" {
                        if args_str.is_empty() {
                            return "vec![]".to_string();
                        } else {
                            return format!("vec![{}]", args_str[0]);
                        }
                    }
                    if name == "spawn" && args.len() == 2 {
                        let actor_name = if let ExprKind::Var(n) = &args[0].kind {
                            sanitize_name(n)
                        } else {
                            args_str[0].clone()
                        };
                        let init_val = &args_str[1];
                        return format!("{}_spawn({})", actor_name, init_val);
                    }
                    if name == "ask" && args.len() == 2 {
                        let handle_name = if let ExprKind::Var(n) = &args[0].kind {
                            n.clone()
                        } else {
                            args_str[0].clone()
                        };
                        let actor_name = self
                            .actor_handle_vars
                            .get(&handle_name)
                            .map(|n| sanitize_name(n))
                            .unwrap_or_else(|| handle_name.clone());
                        return format!("{{ let (__tx, __rx) = tokio::sync::oneshot::channel(); {}.send({}Msg::__Ask(Box::new({}), __tx)).unwrap(); __rx.await.unwrap() }}",
                            args_str[0], actor_name, args_str[1]);
                    }
                    if name == "as_stream" && args_str.len() == 1 {
                        if self.has_async {
                            let arg_name = if let ExprKind::Var(n) = &args[0].kind {
                                n.as_str()
                            } else {
                                ""
                            };
                            if self.subject_vars.contains(arg_name) {
                                return format!("{}.subscribe()", args_str[0]);
                            }
                        }
                        return format!("{}.clone()", args_str[0]);
                    }
                    // Constructor application — wrap recursive args in Rc::new/Arc::new/Box::new
                    if let Some(parent) = self.types.variant_parent.get(name.as_str()) {
                        let is_pos = self
                            .types
                            .variant_positional
                            .get(name.as_str())
                            .copied()
                            .unwrap_or(true);
                        let boxed_indices = self.types.variant_boxed_args.get(name.as_str());
                        let use_rc = self.types.rc_types.contains(parent);
                        let wrap_fn = if use_rc {
                            format!("{}::new", self.rc_name())
                        } else {
                            "Box::new".to_string()
                        };
                        let wrapped: Vec<String> = args_str
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                if boxed_indices.map_or(false, |bi| bi.contains(&i)) {
                                    format!("{}({})", wrap_fn, a)
                                } else {
                                    a.clone()
                                }
                            })
                            .collect();
                        let is_struct_type = self.types.struct_types.contains(parent);
                        if is_pos {
                            if is_struct_type {
                                return format!("{}({})", parent, wrapped.join(", "));
                            } else {
                                return format!("{}::{}({})", parent, name, wrapped.join(", "));
                            }
                        } else {
                            // Named/struct variant
                            let fields = self.types.variant_fields.get(name.as_str());
                            let pairs: Vec<String> = wrapped
                                .iter()
                                .enumerate()
                                .map(|(i, a)| {
                                    let fname = fields
                                        .and_then(|f| f.get(i))
                                        .map(|s| s.as_str())
                                        .unwrap_or("_");
                                    format!("{}: {}", fname, a)
                                })
                                .collect();
                            if is_struct_type {
                                return format!("{} {{ {} }}", parent, pairs.join(", "));
                            } else {
                                return format!("{}::{} {{ {} }}", parent, name, pairs.join(", "));
                            }
                        }
                    }
                }

                // Effect operation routing: if calling an effect op, dispatch through handler
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Check if this is a direct effect operation (say, ask, etc.)
                    for eff in &self.current_effects {
                        if let Some(ops) = self.types.effect_ops.get(eff.as_str()) {
                            if ops.contains(name.as_str()) {
                                return format!("__eff_{}.{}({})", eff, name, args_str.join(", "));
                            }
                        }
                    }
                    // Effect forwarding: if calling a function that requires effects, pass handlers
                    if let Some(callee_effects) = self.types.fn_effects.get(name.as_str()).cloned()
                    {
                        let mut extra_args = Vec::new();
                        for ce in &callee_effects {
                            if self.current_effects.contains(ce) {
                                if self.handle_scope_effects.contains(ce) {
                                    // Concrete handler struct from | handle block — needs &mut
                                    extra_args.push(format!("&mut __eff_{}", ce));
                                } else {
                                    // Already a &mut impl E param — reborrow automatically
                                    extra_args.push(format!("__eff_{}", ce));
                                }
                            }
                        }
                        if !extra_args.is_empty() {
                            let mut all_args = args_str;
                            all_args.extend(extra_args);
                            return format!("{}({})", sanitize_name(name), all_args.join(", "));
                        }
                    }
                }

                // FnMut nested call fix: f(f(x)) needs temporaries to avoid
                // double mutable borrow. Pre-bind inner calls to temps.
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    let needs_temp = args.iter().any(|a| Self::expr_calls_var(a, name));
                    if needs_temp {
                        let mut parts = Vec::new();
                        let mut temp_count = 0;
                        let new_args: Vec<String> = args
                            .iter()
                            .zip(args_str.iter())
                            .map(|(a, s)| {
                                if Self::expr_calls_var(a, name) {
                                    temp_count += 1;
                                    let tmp = format!("__fnmut_tmp{}", temp_count);
                                    parts.push(format!("let {} = {};", tmp, s));
                                    tmp
                                } else {
                                    s.clone()
                                }
                            })
                            .collect();
                        let call = format!("{}({})", sanitize_name(name), new_args.join(", "));
                        parts.push(call);
                        return format!("{{ {} }}", parts.join(" "));
                    }
                }

                let f = self.emit_expr(func);
                let call = format!("{}({})", f, args_str.join(", "));
                // Value-returning Prolog functions return Option<T> — unwrap at call site
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    if self.types.prolog_value_fns.contains_key(name.as_str()) {
                        return format!("{}.unwrap()", call);
                    }
                }
                call
            }
            ExprKind::Lambda(params, body) => {
                let ps: Vec<String> = params
                    .iter()
                    .map(|p| {
                        if let Some(ty) = &p.ty {
                            format!("{}: {}", sanitize_name(&p.name), self.emit_type(ty))
                        } else if {
                            // Check if body accesses fields on this param → infer struct type
                            let mut fields = BTreeSet::new();
                            self.collect_field_accesses(body, &p.name, &mut fields);
                            !fields.is_empty()
                        } {
                            let mut fields = BTreeSet::new();
                            self.collect_field_accesses(body, &p.name, &mut fields);
                            // If only fst/snd accessed, these are tuple accesses (codegen maps to .0/.1)
                            // Don't infer Pair struct — leave untyped for Rust to infer
                            let only_tuple_fields = fields.iter().all(|f| f == "fst" || f == "snd");
                            if only_tuple_fields {
                                sanitize_name(&p.name)
                            } else {
                                // Find the struct/variant whose fields match
                                let mut inferred = None;
                                for (type_name, type_fields) in &self.types.variant_fields {
                                    let field_set: BTreeSet<String> =
                                        type_fields.iter().cloned().collect();
                                    if fields.is_subset(&field_set) {
                                        inferred = Some(type_name.clone());
                                        break;
                                    }
                                }
                                if let Some(ty) = inferred {
                                    format!("{}: {}", sanitize_name(&p.name), ty)
                                } else {
                                    sanitize_name(&p.name)
                                }
                            }
                        } else if Self::expr_uses_as_tuple(body, &p.name) {
                            // Infer tuple type for params used with fst/snd
                            if self.expr_is_float(body) {
                                format!("{}: (f64, f64)", sanitize_name(&p.name))
                            } else {
                                format!("{}: (i64, i64)", sanitize_name(&p.name))
                            }
                        } else if self.expr_is_string(body) {
                            // Infer String for untyped lambda params in string concat context
                            format!("{}: String", sanitize_name(&p.name))
                        } else if self.expr_is_float(body) {
                            // Infer f64 for untyped lambda params when float literals are present
                            format!("{}: f64", sanitize_name(&p.name))
                        } else if self.expr_is_arithmetic(body) {
                            // Infer i64 for untyped lambda params in arithmetic context
                            format!("{}: i64", sanitize_name(&p.name))
                        } else {
                            sanitize_name(&p.name)
                        }
                    })
                    .collect();
                // Lambda params are locally scoped — prevent escape analysis from cloning them.
                // Save outer counts, mark lambda params as single-use, emit body, restore.
                let lambda_param_names: Vec<String> =
                    params.iter().map(|p| p.name.clone()).collect();
                let saved: Vec<(String, Option<usize>, Option<usize>, bool)> = lambda_param_names
                    .iter()
                    .map(|n| {
                        (
                            n.clone(),
                            self.var_use_counts.get(n).copied(),
                            self.var_consuming_counts.get(n).copied(),
                            self.copy_vars.contains(n.as_str()),
                        )
                    })
                    .collect();
                for name in &lambda_param_names {
                    self.var_use_counts.insert(name.clone(), 1);
                    self.var_consuming_counts.insert(name.clone(), 0);
                    self.copy_vars.insert(name.clone());
                }
                let body_str = self.emit_expr(body);
                // Restore outer escape analysis state
                for (name, uses, consuming, was_copy) in saved {
                    if let Some(u) = uses {
                        self.var_use_counts.insert(name.clone(), u);
                    } else {
                        self.var_use_counts.remove(&name);
                    }
                    if let Some(c) = consuming {
                        self.var_consuming_counts.insert(name.clone(), c);
                    } else {
                        self.var_consuming_counts.remove(&name);
                    }
                    if !was_copy {
                        self.copy_vars.remove(&name);
                    }
                }
                // Identify truly captured variables: free in body, not params, not functions/builtins
                let param_bound: BTreeSet<String> = lambda_param_names.iter().cloned().collect();
                let mut free_in_body = BTreeSet::new();
                collect_true_free_vars(body, &mut free_in_body, &param_bound);
                // Check LSP_BUILTINS arity table for special functions (show, print, etc.)
                let lsp_builtin_names: BTreeSet<&str> =
                    LSP_BUILTINS.iter().map(|(n, _)| *n).collect();
                let captured: Vec<String> = free_in_body
                    .into_iter()
                    .filter(|v| {
                        !self.types.user_functions.contains(v.as_str())
                            && !self.builtin_registry.contains_key(v.as_str())
                            && !self.types.variant_parent.contains_key(v.as_str())
                            && !self.copy_vars.contains(v.as_str())
                            && !lsp_builtin_names.contains(v.as_str())
                            && !matches!(
                                v.as_str(),
                                "true"
                                    | "false"
                                    | "True"
                                    | "False"
                                    | "None"
                                    | "Some"
                                    | "Ok"
                                    | "Err"
                                    | "Nil"
                                    | "Cons"
                            )
                    })
                    .collect();
                if captured.is_empty() {
                    format!("|{}| {}", ps.join(", "), body_str)
                } else {
                    // Clone non-Copy captured vars, then use `move` to own the clones
                    let clones: Vec<String> = captured
                        .iter()
                        .map(|v| {
                            format!("let {} = {}.clone();", sanitize_name(v), sanitize_name(v))
                        })
                        .collect();
                    format!(
                        "{{ {} move |{}| {} }}",
                        clones.join(" "),
                        ps.join(", "),
                        body_str
                    )
                }
            }
            ExprKind::BinOp(op, lhs, rhs) => {
                // String concatenation → format!()
                if op == "+" && self.expr_is_string(expr) {
                    return self.emit_string_concat(expr);
                }
                let is_comparison = op == "==" || op == "!=" || op == "=";
                // For comparisons with string literals, emit &str (no .to_string())
                // so that &String == &str works via Deref coercion
                let mut l = if is_comparison
                    && matches!(lhs.as_ref().kind, ExprKind::Lit(Literal::Str(_)))
                {
                    if let ExprKind::Lit(Literal::Str(s)) = &lhs.as_ref().kind {
                        format!("{:?}", s) // &str, no .to_string()
                    } else {
                        self.emit_expr(lhs)
                    }
                } else {
                    self.emit_expr(lhs)
                };
                let mut r = if is_comparison
                    && matches!(rhs.as_ref().kind, ExprKind::Lit(Literal::Str(_)))
                {
                    if let ExprKind::Lit(Literal::Str(s)) = &rhs.as_ref().kind {
                        format!("{:?}", s) // &str, no .to_string()
                    } else {
                        self.emit_expr(rhs)
                    }
                } else {
                    self.emit_expr(rhs)
                };
                // Deref borrowed params in comparisons: &T == T → *param == T
                if is_comparison {
                    if let ExprKind::Var(n) = &lhs.as_ref().kind {
                        if self.current_borrow_params.contains(n.as_str()) {
                            l = format!("*{}", l);
                        }
                    }
                    if let ExprKind::Var(n) = &rhs.as_ref().kind {
                        if self.current_borrow_params.contains(n.as_str()) {
                            r = format!("*{}", r);
                        }
                    }
                }
                // Futuruna uses = for equality; Rust uses ==
                let rust_op = if op == "=" { "==" } else { op.as_str() };
                // Safe division/modulo: return 0 on division by zero (matches interpreter)
                if rust_op == "/" || rust_op == "%" {
                    let is_float = self.expr_is_float(lhs) || self.expr_is_float(rhs);
                    if is_float {
                        return format!(
                            "{{ let __d = {}; if __d == 0.0 {{ 0.0 }} else {{ {} {} __d }} }}",
                            r, l, rust_op
                        );
                    } else {
                        return format!(
                            "{{ let __d = {}; if __d == 0 {{ 0 }} else {{ {} {} __d }} }}",
                            r, l, rust_op
                        );
                    }
                }
                format!("({} {} {})", l, rust_op, r)
            }
            ExprKind::UnOp(op, operand) => {
                format!("{}{}", op, self.emit_expr(operand))
            }
            ExprKind::If(cond, then_, else_) => {
                let c = self.emit_expr(cond);
                let t = self.emit_if_branch(then_);
                let e = self.emit_if_branch(else_);
                format!("if {} {{ {} }} else {{ {} }}", c, t, e)
            }
            ExprKind::Match(scrut, arms) => {
                let s = self.emit_expr(scrut);
                let mut out = format!("match {} {{\n", s);
                for arm in arms {
                    let pat = self.emit_pattern_match(&arm.pat);
                    // Build guard: combine user guard + boxed pattern guards
                    let user_guard = arm.guard.as_ref().map(|g| self.emit_expr(g));
                    let box_guard = self.emit_boxed_pattern_guard(&arm.pat);
                    let full_guard = match (user_guard, box_guard) {
                        (Some(u), Some(b)) => format!(" if {} && {}", u, b),
                        (Some(u), None) => format!(" if {}", u),
                        (None, Some(b)) => format!(" if {}", b),
                        (None, None) => String::new(),
                    };
                    // Collect boxed bindings that need deref
                    // In &self methods, skip unboxing — can't move out of reference.
                    // Rust auto-derefs &Box<T> → &T for method calls.
                    let boxed_binds = if self.in_self_method {
                        vec![]
                    } else {
                        self.collect_boxed_bindings(&arm.pat)
                    };
                    if boxed_binds.is_empty() && !self.has_boxed_constructor_patterns(&arm.pat) {
                        let body = self.emit_expr(&arm.body);
                        out.push_str(&format!(
                            "{}    {}{} => {},\n",
                            self.ind(),
                            pat,
                            full_guard,
                            body
                        ));
                    } else {
                        // Emit a block with deref lets
                        out.push_str(&format!("{}    {}{} => {{\n", self.ind(), pat, full_guard));
                        // Rc types: (*var).clone() instead of *var (can't move out of Rc)
                        let is_rc = self.pattern_is_rc_type(&arm.pat);
                        for var in &boxed_binds {
                            if is_rc {
                                out.push_str(&format!(
                                    "{}        let {} = (*{}).clone();\n",
                                    self.ind(),
                                    var,
                                    var
                                ));
                            } else {
                                out.push_str(&format!(
                                    "{}        let {} = *{};\n",
                                    self.ind(),
                                    var,
                                    var
                                ));
                            }
                        }
                        let body = self.emit_expr(&arm.body);
                        out.push_str(&format!("{}        {}\n", self.ind(), body));
                        out.push_str(&format!("{}    }},\n", self.ind()));
                    }
                }
                out.push_str(&format!("{}}}", self.ind()));
                out
            }
            ExprKind::Block(stmts) => {
                let mut out = "{\n".to_string();
                let saved_indent = self.indent;
                // Can't use &mut self here since emit_expr takes &self
                // So we build the block manually
                for (i, stmt) in stmts.iter().enumerate() {
                    let is_last = i == stmts.len() - 1;
                    let prefix = "    ".repeat(saved_indent + 1);
                    match stmt {
                        Stmt::Bind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            // Accumulator rebinding inside for-loop → assignment, not let
                            if let Pat::Var(name) = pat {
                                if self.mutable_vars.contains(name.as_str()) {
                                    out.push_str(&format!(
                                        "{}{} = {};\n",
                                        prefix,
                                        sanitize_name(name),
                                        val_str
                                    ));
                                } else {
                                    out.push_str(&format!(
                                        "{}let {} = {};\n",
                                        prefix, pat_str, val_str
                                    ));
                                }
                            } else {
                                out.push_str(&format!(
                                    "{}let {} = {};\n",
                                    prefix, pat_str, val_str
                                ));
                            }
                        }
                        Stmt::MonadicBind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            let suffix = if self.is_effect_op_call(value) {
                                ""
                            } else {
                                "?"
                            };
                            out.push_str(&format!(
                                "{}let {} = {}{};\n",
                                prefix, pat_str, val_str, suffix
                            ));
                        }
                        Stmt::Expr(expr) if is_last => {
                            out.push_str(&format!("{}{}\n", prefix, self.emit_expr(expr)));
                        }
                        Stmt::Expr(Expr {
                            kind: ExprKind::Effect(name, args),
                            ..
                        }) if name == "print" => {
                            out.push_str(&self.emit_print(args, &prefix));
                        }
                        Stmt::Expr(expr) => {
                            out.push_str(&format!("{}{};\n", prefix, self.emit_expr(expr)));
                        }
                        Stmt::Defn(Defn::Fn {
                            name, params, body, ..
                        }) => {
                            let ps: Vec<String> = params
                                .iter()
                                .map(|p| {
                                    let ty =
                                        p.ty.as_ref()
                                            .map(|t| self.emit_type(t))
                                            .unwrap_or("i64".into());
                                    format!("{}: {}", sanitize_name(&p.name), ty)
                                })
                                .collect();
                            out.push_str(&format!(
                                "{}fn {}({}) {{\n",
                                prefix,
                                sanitize_name(name),
                                ps.join(", ")
                            ));
                            out.push_str(&format!("{}    {}\n", prefix, self.emit_expr(body)));
                            out.push_str(&format!("{}}}\n", prefix));
                        }
                        _ => {
                            out.push_str(&format!("{}// stmt\n", prefix));
                        }
                    }
                }
                out.push_str(&format!("{}}}", "    ".repeat(saved_indent)));
                out
            }
            ExprKind::Field(obj, field) => {
                // Sync subject field access: subject.count → subject.len(), subject.latest → subject.last().cloned().unwrap()
                if let ExprKind::Var(var_name) = &obj.as_ref().kind {
                    if self.sync_subject_vars.contains(var_name.as_str()) {
                        let obj_str = self.emit_expr(obj);
                        match field.as_str() {
                            "count" => return format!("({}.len() as i64)", obj_str),
                            "latest" => return format!("{}.last().cloned().unwrap()", obj_str),
                            _ => {}
                        }
                    }
                }
                // Scope-qualified access: ScopeName.field → field (local variable)
                if let ExprKind::Var(scope_name) = &obj.as_ref().kind {
                    if self.scope_bindings.contains_key(scope_name.as_str()) {
                        return sanitize_name(field);
                    }
                }
                // Nested scope access: Outer.Inner.field → field
                if let ExprKind::Field(outer, inner_scope) = &obj.as_ref().kind {
                    if let ExprKind::Var(scope_name) = &outer.as_ref().kind {
                        if self.scope_bindings.contains_key(scope_name.as_str()) {
                            return sanitize_name(field);
                        }
                    }
                }
                // M3b: module qualified access uses :: in Rust (Name::func)
                // Handles nested modules: App.Utils.func → App::Utils::func
                if self.is_module_path(obj) {
                    let path = self.emit_module_path(obj);
                    // If field is a variant constructor, insert the parent type
                    // e.g. Lib.Red → Lib::Color::Red (not Lib::Red)
                    if let Some(parent) = self.types.variant_parent.get(field) {
                        return format!(
                            "{}::{}::{}",
                            path,
                            self.rust_type_name(parent),
                            sanitize_name(field)
                        );
                    }
                    return format!("{}::{}", path, sanitize_name(field));
                }
                // Enum variant field access: emit a match expression
                // Find ALL variants that have this field (handles shared field names like Dog.name and Cat.name)
                {
                    let matches = self.find_all_variant_fields(field);
                    if !matches.is_empty() {
                        let mut obj_str = self.emit_expr(obj);
                        // If the object is itself a boxed field access, we need to deref it
                        if let ExprKind::Field(_, inner_field) = &obj.as_ref().kind {
                            if let Some((iv, _ip)) = self.find_variant_field(inner_field) {
                                if self.is_field_boxed(&iv, inner_field) {
                                    obj_str = format!("*{}", obj_str);
                                }
                            }
                        }
                        // Build match arms for all variants that have this field
                        let mut arms = Vec::new();
                        for (variant_name, parent_name) in &matches {
                            if self.types.variant_positional.get(variant_name.as_str())
                                == Some(&false)
                            {
                                let is_boxed = self.is_field_boxed(variant_name, field);
                                let clone_expr = if is_boxed {
                                    "(*__f).clone()"
                                } else {
                                    "__f.clone()"
                                };
                                arms.push(format!(
                                    "{}::{} {{ {}: ref __f, .. }} => {}",
                                    parent_name, variant_name, field, clone_expr
                                ));
                            }
                        }
                        if !arms.is_empty() {
                            if arms.len() == 1 {
                                return format!("{{ if let {} = {} {{ {} }} else {{ panic!(\"field access on wrong variant\") }} }}",
                                    arms[0].split(" => ").next().unwrap(), obj_str,
                                    arms[0].split(" => ").nth(1).unwrap());
                            }
                            arms.push("_ => panic!(\"field access on wrong variant\")".to_string());
                            return format!("{{ match {} {{ {} }} }}", obj_str, arms.join(", "));
                        }
                    }
                }
                // Non-Copy field access on borrowed param: p.name on &Person needs .clone()
                // because moving a String out of &T is not allowed
                let needs_clone = if let ExprKind::Var(var_name) = &obj.as_ref().kind {
                    if self.current_borrow_params.contains(var_name.as_str()) {
                        // Check if this field's type is non-Copy in any variant_field_types
                        let field_is_copy = self.types.variant_field_types.iter().any(|(_, ft)| {
                            ft.get(field).map(|ty| is_copy_type(ty)).unwrap_or(false)
                        });
                        !field_is_copy
                    } else {
                        false
                    }
                } else {
                    false
                };
                // Tuple field access: .fst → .0, .snd → .1
                // (enumerate, zip, and other builtins produce Rust tuples, not named structs)
                let rust_field = match field.as_str() {
                    "fst" => "0",
                    "snd" => "1",
                    _ => field.as_str(),
                };
                if needs_clone {
                    format!("{}.{}.clone()", self.emit_expr(obj), rust_field)
                } else {
                    format!("{}.{}", self.emit_expr(obj), rust_field)
                }
            }
            ExprKind::Index(arr, idx) => {
                // Safe index: bounds-check instead of panic on negative/out-of-range
                let arr_str = self.emit_expr(arr);
                let idx_str = self.emit_expr(idx);
                format!("{{ let __arr = &{}; let __i = {}; if __i < 0 || __i as usize >= __arr.len() {{ panic!(\"index out of bounds: {{}} (len {{}})\", __i, __arr.len()) }} else {{ __arr[__i as usize].clone() }} }}", arr_str, idx_str)
            }
            ExprKind::List(elems) => {
                let items: Vec<String> = elems
                    .iter()
                    .map(|e| {
                        let s = self.emit_expr(e);
                        // Auto-clone variables with multiple consuming uses (same logic as fn args)
                        if let ExprKind::Var(n) = &e.kind {
                            if !self.types.variant_parent.contains_key(n.as_str())
                                && !self.copy_vars.contains(n.as_str())
                                && self
                                    .var_consuming_counts
                                    .get(n.as_str())
                                    .copied()
                                    .unwrap_or(0)
                                    > 1
                            {
                                return format!("{}.clone()", s);
                            }
                        }
                        s
                    })
                    .collect();
                format!("vec![{}]", items.join(", "))
            }
            ExprKind::Tuple(elems) => {
                let items: Vec<String> = elems.iter().map(|e| self.emit_expr(e)).collect();
                format!("({})", items.join(", "))
            }
            ExprKind::Effect(name, args) => {
                match name.as_str() {
                    "print" => {
                        // Inline effect expression — emit as println! statement
                        let s = self.emit_print(args, "");
                        s.trim_end().to_string()
                    }
                    "time" => {
                        "std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()".to_string()
                    }
                    "random" => {
                        // Deterministic-seed xorshift (no external dependency)
                        "{ let mut __x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64; __x ^= __x << 13; __x ^= __x >> 7; __x ^= __x << 17; (__x as f64) / (u64::MAX as f64) }".to_string()
                    }
                    "input" => {
                        "{ let mut __s = String::new(); std::io::stdin().read_line(&mut __s).unwrap(); __s.trim().to_string() }".to_string()
                    }
                    _ if self.builtin_registry.contains_key(name.as_str()) => {
                        let app = ExprKind::App(Box::new(ExprKind::Var(name.clone()).into()), args.clone()).into();
                        self.emit_expr(&app)
                    }
                    _ => {
                        let args_str: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                        format!("{}({})", name, args_str.join(", "))
                    }
                }
            }
            ExprKind::Try(inner) => {
                format!("{}?", self.emit_expr(inner))
            }
            ExprKind::Pipe(input, transform) => {
                // Desugar Pipe to App in codegen: a |> f → f(a), a |> f(y) → f(a, y)
                let desugared: Expr = match &transform.as_ref().kind {
                    ExprKind::App(func, existing_args) => {
                        let mut new_args = vec![input.as_ref().clone()];
                        new_args.extend(existing_args.iter().cloned());
                        ExprKind::App(func.clone(), new_args).into()
                    }
                    _ => ExprKind::App(
                        Box::new(transform.as_ref().clone()),
                        vec![input.as_ref().clone()],
                    )
                    .into(),
                };
                self.emit_expr(&desugared)
            }
            ExprKind::Unit => "()".to_string(),
            ExprKind::Conjunction(goals) => {
                // Emit as && chain for simple conjunction
                let parts: Vec<String> = goals.iter().map(|g| self.emit_expr(g)).collect();
                parts.join(" && ")
            }
            ExprKind::Handle {
                effect,
                handlers,
                body,
            } => {
                // Collect free variables in handler bodies for capture
                let mut all_handler_params: BTreeSet<String> = BTreeSet::new();
                for h in handlers {
                    for p in &h.params {
                        all_handler_params.insert(p.clone());
                    }
                }
                let mut captures: BTreeSet<String> = BTreeSet::new();
                for h in handlers {
                    let free = Self::collect_handler_free_vars(
                        &h.body,
                        &all_handler_params,
                        &self.types.effect_ops,
                    );
                    captures.extend(free);
                }
                // Only keep captures that we know the type of
                let typed_captures: Vec<(String, String)> = captures
                    .iter()
                    .filter_map(|name| {
                        self.var_types
                            .get(name)
                            .map(|ty| (name.clone(), ty.clone()))
                    })
                    .collect();

                let mut out = String::from("{\n");
                let handler_name = format!("__Eff{}Handler", effect);
                if typed_captures.is_empty() {
                    out.push_str(&format!("{}struct {};\n", self.ind(), handler_name));
                } else {
                    out.push_str(&format!("{}struct {} {{\n", self.ind(), handler_name));
                    for (name, ty) in &typed_captures {
                        out.push_str(&format!(
                            "{}    {}: {},\n",
                            self.ind(),
                            sanitize_name(name),
                            ty
                        ));
                    }
                    out.push_str(&format!("{}}}\n", self.ind()));
                }
                out.push_str(&format!(
                    "{}impl {} for {} {{\n",
                    self.ind(),
                    effect,
                    handler_name
                ));
                for h in handlers {
                    // Look up param types and return type from effect declaration
                    let op_sig = self
                        .types
                        .effect_ops_detail
                        .get(effect)
                        .and_then(|ops| ops.iter().find(|(n, _, _)| n == &h.op_name));
                    let params_str: Vec<String> = h
                        .params
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let ty = op_sig
                                .and_then(|(_, params, _)| params.get(i))
                                .and_then(|p| p.ty.as_ref())
                                .map(|t| self.emit_type(t))
                                .unwrap_or_else(|| "String".into());
                            format!("{}: {}", sanitize_name(p), ty)
                        })
                        .collect();
                    let ret = op_sig
                        .and_then(|(_, _, ret)| ret.as_ref())
                        .map(|t| format!(" -> {}", self.emit_type(t)))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "{}    fn {}(&mut self, {}){} {{\n",
                        self.ind(),
                        h.op_name,
                        params_str.join(", "),
                        ret
                    ));
                    // Emit handler body, replacing captured vars with self.var_name
                    let capture_names: BTreeSet<String> =
                        typed_captures.iter().map(|(n, _)| n.clone()).collect();
                    out.push_str(&format!(
                        "{}        {}\n",
                        self.ind(),
                        self.emit_handle_body_with_captures(&h.body, &capture_names)
                    ));
                    out.push_str(&format!("{}    }}\n", self.ind()));
                }
                out.push_str(&format!("{}}}\n", self.ind()));
                let eff_var = format!("__eff_{}", effect);
                if typed_captures.is_empty() {
                    out.push_str(&format!(
                        "{}let mut {} = {};\n",
                        self.ind(),
                        eff_var,
                        handler_name
                    ));
                } else {
                    let init_fields: Vec<String> = typed_captures
                        .iter()
                        .map(|(name, _)| {
                            let sname = sanitize_name(name);
                            if self.copy_vars.contains(name.as_str()) {
                                format!("{}: {}", sname, sname)
                            } else {
                                format!("{}: {}.clone()", sname, sname)
                            }
                        })
                        .collect();
                    out.push_str(&format!(
                        "{}let mut {} = {} {{ {} }};\n",
                        self.ind(),
                        eff_var,
                        handler_name,
                        init_fields.join(", ")
                    ));
                }
                // Emit body with the handler in scope (concrete struct, needs &mut)
                self.current_effects.push(effect.clone());
                self.handle_scope_effects.insert(effect.clone());
                out.push_str(&format!("{}{}\n", self.ind(), self.emit_expr(body)));
                self.current_effects.pop();
                self.handle_scope_effects.remove(effect);
                out.push_str(&format!("{}}}", self.ind()));
                out
            }
        }
    }

    /// Collect variable names bound to boxed fields in a pattern
    fn collect_boxed_bindings(&self, pat: &Pat) -> Vec<String> {
        let mut result = Vec::new();
        match pat {
            Pat::Con(name, args) => {
                if let Some(boxed_indices) = self.types.variant_boxed_args.get(name.as_str()) {
                    for (i, sub_pat) in args.iter().enumerate() {
                        if boxed_indices.contains(&i) {
                            // Collect var names from this sub-pattern
                            self.collect_pat_vars(sub_pat, &mut result);
                        }
                        // Also recurse into sub-patterns for nested constructors
                        result.extend(self.collect_boxed_bindings(sub_pat));
                    }
                } else {
                    for sub_pat in args {
                        result.extend(self.collect_boxed_bindings(sub_pat));
                    }
                }
            }
            _ => {}
        }
        result
    }

    /// Collect all variable names from a pattern
    fn collect_pat_vars(&self, pat: &Pat, vars: &mut Vec<String>) {
        match pat {
            Pat::Var(name) if name != "_" => vars.push(sanitize_name(name)),
            Pat::Con(_, args) => {
                for a in args {
                    self.collect_pat_vars(a, vars);
                }
            }
            Pat::NamedCon(_, named_args) => {
                for (_, p) in named_args {
                    self.collect_pat_vars(p, vars);
                }
            }
            Pat::As(inner, name) => {
                self.collect_pat_vars(inner, vars);
                vars.push(sanitize_name(name));
            }
            _ => {}
        }
    }

    /// Emit a print() call as println!() with proper format string
    fn emit_print(&mut self, args: &[Expr], prefix: &str) -> String {
        if args.is_empty() {
            return format!("{}println!();\n", prefix);
        }
        let arg = &args[0];
        // If the argument is a string concat, decompose into println! format args directly
        if self.expr_is_string(arg) {
            if let ExprKind::BinOp(op, _, _) = &arg.kind {
                if op == "+" {
                    let parts = self.collect_concat_parts(arg);
                    let mut fmt_str = String::new();
                    let mut fmt_args = Vec::new();
                    for part in &parts {
                        if let Some(lit) = part.strip_prefix("literal:") {
                            fmt_str.push_str(&lit.replace('{', "{{").replace('}', "}}"));
                        } else if let Some(expr_str) = part.strip_prefix("expr:") {
                            fmt_str.push_str("{}");
                            fmt_args.push(expr_str.to_string());
                        }
                    }
                    if fmt_args.is_empty() {
                        return format!("{}println!({:?});\n", prefix, fmt_str);
                    } else {
                        return format!(
                            "{}println!({:?}, {});\n",
                            prefix,
                            fmt_str,
                            fmt_args.join(", ")
                        );
                    }
                }
            }
        }
        let val = self.emit_expr(arg);
        format!("{}println!(\"{{}}\", {});\n", prefix, val)
    }

    /// Emit the body of an effect handler — replace resume(val) with just val
    /// Check if an expression contains a call to the named variable
    fn expr_calls_var(expr: &Expr, var_name: &str) -> bool {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if matches!(func.as_ref().kind, ExprKind::Var(ref n) if n == var_name) {
                    return true;
                }
                Self::expr_calls_var(func, var_name)
                    || args.iter().any(|a| Self::expr_calls_var(a, var_name))
            }
            ExprKind::BinOp(_, lhs, rhs) => {
                Self::expr_calls_var(lhs, var_name) || Self::expr_calls_var(rhs, var_name)
            }
            ExprKind::UnOp(_, inner) => Self::expr_calls_var(inner, var_name),
            ExprKind::If(c, t, e) => {
                Self::expr_calls_var(c, var_name)
                    || Self::expr_calls_var(t, var_name)
                    || Self::expr_calls_var(e, var_name)
            }
            ExprKind::Block(stmts) => stmts.iter().any(|s| match s {
                Stmt::Expr(e) => Self::expr_calls_var(e, var_name),
                Stmt::Bind(_, _, e) => Self::expr_calls_var(e, var_name),
                _ => false,
            }),
            _ => false,
        }
    }

    fn emit_handle_body(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::App(func, args) if matches!(func.kind, ExprKind::Var(ref n) if n == "resume") =>
            {
                // resume(val) → just emit val (it's the return value)
                if let Some(arg) = args.first() {
                    self.emit_expr(arg)
                } else {
                    "()".to_string()
                }
            }
            _ => self.emit_expr(expr),
        }
    }

    /// Emit handler body, replacing captured variables with self.field references.
    fn emit_handle_body_with_captures(
        &mut self,
        expr: &Expr,
        captures: &BTreeSet<String>,
    ) -> String {
        if captures.is_empty() {
            return self.emit_handle_body(expr);
        }
        // Emit the body, then replace captured var names with self.var_name
        let body_str = self.emit_handle_body(expr);
        let mut result = body_str;
        for name in captures {
            let sname = sanitize_name(name);
            // Replace standalone variable references (word boundaries)
            // This is a simple text replacement — works for common cases
            result = Self::replace_var_with_self(&result, &sname);
        }
        result
    }

    /// Replace standalone variable references with self.var_name.
    /// Uses word boundary detection to avoid replacing substrings.
    fn replace_var_with_self(code: &str, var_name: &str) -> String {
        let mut result = String::with_capacity(code.len());
        let var_bytes = var_name.as_bytes();
        let code_bytes = code.as_bytes();
        let mut i = 0;
        while i < code_bytes.len() {
            if i + var_bytes.len() <= code_bytes.len()
                && &code_bytes[i..i + var_bytes.len()] == var_bytes
            {
                // Check word boundaries
                let before_ok = i == 0
                    || !code_bytes[i - 1].is_ascii_alphanumeric() && code_bytes[i - 1] != b'_';
                let after_idx = i + var_bytes.len();
                let after_ok = after_idx >= code_bytes.len()
                    || !code_bytes[after_idx].is_ascii_alphanumeric()
                        && code_bytes[after_idx] != b'_';
                // Don't replace if already `self.var`
                let already_self = i >= 5 && &code_bytes[i - 5..i] == b"self.";
                if before_ok && after_ok && !already_self {
                    result.push_str("self.");
                    result.push_str(var_name);
                    i += var_bytes.len();
                    continue;
                }
            }
            result.push(code_bytes[i] as char);
            i += 1;
        }
        result
    }

    /// Infer the Rust type of an expression (best-effort, for handler capture).
    fn infer_expr_type(expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Lit(Literal::Int(_)) => Some("i64".to_string()),
            ExprKind::Lit(Literal::Float(_)) => Some("f64".to_string()),
            ExprKind::Lit(Literal::Str(_)) => Some("String".to_string()),
            ExprKind::Lit(Literal::Bool(_)) => Some("bool".to_string()),
            ExprKind::Lit(Literal::Char(_)) => Some("char".to_string()),
            _ => None,
        }
    }

    /// Collect free variables in a handler body (variables not bound as handler params).
    fn collect_handler_free_vars(
        expr: &Expr,
        handler_params: &BTreeSet<String>,
        effect_ops: &BTreeMap<String, BTreeSet<String>>,
    ) -> BTreeSet<String> {
        let mut free = BTreeSet::new();
        Self::walk_free_vars(expr, handler_params, effect_ops, &mut free);
        free
    }

    fn walk_free_vars(
        expr: &Expr,
        bound: &BTreeSet<String>,
        effect_ops: &BTreeMap<String, BTreeSet<String>>,
        free: &mut BTreeSet<String>,
    ) {
        // Known builtins that should NOT be treated as free variables
        let builtins: BTreeSet<&str> = [
            "show",
            "print",
            "length",
            "push",
            "map",
            "filter",
            "foldl",
            "range",
            "resume",
            "assert",
            "parse_int",
            "to_float",
        ]
        .iter()
        .copied()
        .collect();
        // Flatten all effect op names
        let all_ops: BTreeSet<&str> = effect_ops
            .values()
            .flat_map(|ops| ops.iter().map(|s| s.as_str()))
            .collect();

        match &expr.kind {
            ExprKind::Var(name) => {
                if !bound.contains(name)
                    && !builtins.contains(name.as_str())
                    && !all_ops.contains(name.as_str())
                    && name.chars().next().map_or(false, |c| c.is_lowercase())
                {
                    free.insert(name.clone());
                }
            }
            ExprKind::App(func, args) => {
                Self::walk_free_vars(func, bound, effect_ops, free);
                for arg in args {
                    Self::walk_free_vars(arg, bound, effect_ops, free);
                }
            }
            ExprKind::BinOp(_, l, r) => {
                Self::walk_free_vars(l, bound, effect_ops, free);
                Self::walk_free_vars(r, bound, effect_ops, free);
            }
            ExprKind::UnOp(_, e) | ExprKind::Try(e) | ExprKind::Field(e, _) => {
                Self::walk_free_vars(e, bound, effect_ops, free);
            }
            ExprKind::If(c, t, e) => {
                Self::walk_free_vars(c, bound, effect_ops, free);
                Self::walk_free_vars(t, bound, effect_ops, free);
                Self::walk_free_vars(e, bound, effect_ops, free);
            }
            ExprKind::Block(stmts) => {
                for stmt in stmts {
                    match stmt {
                        Stmt::Expr(e) | Stmt::Bind(_, _, e) | Stmt::MonadicBind(_, _, e) => {
                            Self::walk_free_vars(e, bound, effect_ops, free);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::Lambda(params, body) => {
                let mut new_bound = bound.clone();
                for p in params {
                    new_bound.insert(p.name.clone());
                }
                Self::walk_free_vars(body, &new_bound, effect_ops, free);
            }
            ExprKind::Match(scrut, arms) => {
                Self::walk_free_vars(scrut, bound, effect_ops, free);
                for arm in arms {
                    Self::walk_free_vars(&arm.body, bound, effect_ops, free);
                }
            }
            ExprKind::List(items) | ExprKind::Tuple(items) | ExprKind::Effect(_, items) => {
                for item in items {
                    Self::walk_free_vars(item, bound, effect_ops, free);
                }
            }
            ExprKind::Index(a, b) => {
                Self::walk_free_vars(a, bound, effect_ops, free);
                Self::walk_free_vars(b, bound, effect_ops, free);
            }
            ExprKind::Pipe(input, transform) => {
                Self::walk_free_vars(input, bound, effect_ops, free);
                Self::walk_free_vars(transform, bound, effect_ops, free);
            }
            ExprKind::Handle { body, handlers, .. } => {
                Self::walk_free_vars(body, bound, effect_ops, free);
                for h in handlers {
                    Self::walk_free_vars(&h.body, bound, effect_ops, free);
                }
            }
            ExprKind::Conjunction(goals) => {
                for g in goals {
                    Self::walk_free_vars(g, bound, effect_ops, free);
                }
            }
            ExprKind::Lit(_) | ExprKind::Unit => {}
        }
    }

    /// Determine which functions are pure (no side effects).
    /// A function is pure if: no effect ops, no @ print calls in the body,
    /// and all called functions are also pure. Iterates to fixed point.
    fn find_pure_functions(
        stmts: &[Stmt],
        effect_ops: &BTreeMap<String, BTreeSet<String>>,
        fn_effects: &BTreeMap<String, Vec<String>>,
    ) -> BTreeSet<String> {
        // Start: all functions without explicit effects are candidates
        let mut pure: BTreeSet<String> = BTreeSet::new();
        let mut fn_bodies: BTreeMap<String, &Expr> = BTreeMap::new();
        let mut fn_calls: BTreeMap<String, Vec<String>> = BTreeMap::new();

        // All effect op names (impure by definition) + impure builtins from registry
        let registry = rust_builtin_registry();
        let mut all_effect_ops: BTreeSet<String> = effect_ops
            .values()
            .flat_map(|ops| ops.iter().cloned())
            .collect();
        all_effect_ops.insert("print".to_string());
        for (name, def) in &registry {
            if def.impure {
                all_effect_ops.insert(name.clone());
            }
        }

        for stmt in stmts {
            if let Stmt::Defn(Defn::Fn {
                name,
                effects,
                body,
                ..
            }) = stmt
            {
                // Functions with explicit or inferred effects are impure
                if !effects.is_empty() || fn_effects.get(name).map_or(false, |e| !e.is_empty()) {
                    continue;
                }
                fn_bodies.insert(name.clone(), body);
                pure.insert(name.clone());
            }
        }

        // Collect function calls from each body and check for impure expressions
        for (fn_name, body) in &fn_bodies {
            let mut calls = Vec::new();
            let mut is_impure = false;
            Self::check_purity(body, &all_effect_ops, &mut calls, &mut is_impure);
            if is_impure {
                pure.remove(fn_name);
            }
            fn_calls.insert(fn_name.clone(), calls);
        }

        // Fixed point: if a pure function calls an impure one, it becomes impure
        loop {
            let mut changed = false;
            for (fn_name, calls) in &fn_calls {
                if !pure.contains(fn_name) {
                    continue;
                }
                for callee in calls {
                    // Calling a non-pure function makes us impure
                    // (unless the callee is a builtin like +, -, etc. handled elsewhere)
                    if fn_bodies.contains_key(callee) && !pure.contains(callee) {
                        pure.remove(fn_name);
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        pure
    }

    /// Check if an expression contains impure operations.
    /// Collects function call names and sets impure flag for side-effectful ops.
    fn check_purity(
        expr: &Expr,
        effect_ops: &BTreeSet<String>,
        calls: &mut Vec<String>,
        impure: &mut bool,
    ) {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    if effect_ops.contains(name) {
                        *impure = true;
                    }
                    calls.push(name.clone());
                }
                for arg in args {
                    Self::check_purity(arg, effect_ops, calls, impure);
                }
            }
            ExprKind::Block(stmts) => {
                for stmt in stmts {
                    match stmt {
                        Stmt::Expr(e) | Stmt::Bind(_, _, e) | Stmt::MonadicBind(_, _, e) => {
                            Self::check_purity(e, effect_ops, calls, impure);
                        }
                        Stmt::Annot(name, _) if name == "print" => {
                            *impure = true;
                        }
                        Stmt::For(_, iter_e, body) => {
                            Self::check_purity(iter_e, effect_ops, calls, impure);
                            for s in body {
                                if let Stmt::Expr(e) | Stmt::Bind(_, _, e) = s {
                                    Self::check_purity(e, effect_ops, calls, impure);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::If(c, t, e) => {
                Self::check_purity(c, effect_ops, calls, impure);
                Self::check_purity(t, effect_ops, calls, impure);
                Self::check_purity(e, effect_ops, calls, impure);
            }
            ExprKind::Match(scrut, arms) => {
                Self::check_purity(scrut, effect_ops, calls, impure);
                for arm in arms {
                    Self::check_purity(&arm.body, effect_ops, calls, impure);
                }
            }
            ExprKind::BinOp(_, l, r) => {
                Self::check_purity(l, effect_ops, calls, impure);
                Self::check_purity(r, effect_ops, calls, impure);
            }
            ExprKind::UnOp(_, e) | ExprKind::Try(e) | ExprKind::Field(e, _) => {
                Self::check_purity(e, effect_ops, calls, impure);
            }
            ExprKind::Lambda(_, body) => {
                Self::check_purity(body, effect_ops, calls, impure);
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for item in items {
                    Self::check_purity(item, effect_ops, calls, impure);
                }
            }
            ExprKind::Index(a, b) => {
                Self::check_purity(a, effect_ops, calls, impure);
                Self::check_purity(b, effect_ops, calls, impure);
            }
            ExprKind::Pipe(input, transform) => {
                Self::check_purity(input, effect_ops, calls, impure);
                Self::check_purity(transform, effect_ops, calls, impure);
            }
            ExprKind::Handle { .. } => {
                *impure = true;
            }
            ExprKind::Effect(_, _) => {
                *impure = true;
            }
            ExprKind::Conjunction(goals) => {
                for g in goals {
                    Self::check_purity(g, effect_ops, calls, impure);
                }
            }
            ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Unit => {}
        }
    }

    /// Try to auto-evaluate an expression at compile time.
    /// Returns Some(val) if the expression is a pure function call with all literal/comptime args.
    fn try_auto_comptime(
        expr: &Expr,
        pure_fns: &BTreeSet<String>,
        comptime_values: &BTreeMap<String, String>,
        interp: &mut Interpreter,
        env: &Env,
    ) -> Option<Value> {
        match &expr.kind {
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Function must be pure
                    if !pure_fns.contains(name) {
                        return None;
                    }
                    // All args must be literals or known comptime values
                    for arg in args {
                        if !Self::is_comptime_arg(arg, comptime_values) {
                            return None;
                        }
                    }
                    // Evaluate at compile time with step budget
                    let val = Self::eval_with_budget(interp, expr, env)?;
                    Some(val)
                } else {
                    None
                }
            }
            // Also handle simple arithmetic on literals
            ExprKind::BinOp(_, l, r) => {
                if Self::is_comptime_arg(l, comptime_values)
                    && Self::is_comptime_arg(r, comptime_values)
                {
                    let val = Self::eval_with_budget(interp, expr, env)?;
                    Some(val)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Evaluate an expression with a step budget for auto-comptime.
    /// Returns None if the budget is exceeded (too many recursive steps).
    fn eval_with_budget(interp: &mut Interpreter, expr: &Expr, env: &Env) -> Option<Value> {
        interp.step_count = 0;
        interp.step_limit = 50_000;
        interp.budget_exceeded = false;
        let val = interp.eval(expr, env);
        interp.step_limit = 0;
        if interp.budget_exceeded {
            interp.budget_exceeded = false;
            None
        } else {
            Some(val)
        }
    }

    /// Check if an expression is a compile-time known value (literal or comptime binding).
    fn is_comptime_arg(expr: &Expr, comptime_values: &BTreeMap<String, String>) -> bool {
        match &expr.kind {
            ExprKind::Lit(_) => true,
            ExprKind::Var(name) => comptime_values.contains_key(name),
            ExprKind::BinOp(_, l, r) => {
                Self::is_comptime_arg(l, comptime_values)
                    && Self::is_comptime_arg(r, comptime_values)
            }
            ExprKind::UnOp(_, e) => Self::is_comptime_arg(e, comptime_values),
            ExprKind::List(items) => items
                .iter()
                .all(|e| Self::is_comptime_arg(e, comptime_values)),
            _ => false,
        }
    }

    /// Walk an expression tree collecting unhandled effect references.
    /// Used by effect type inference to discover which effects a function needs.
    fn collect_expr_effects(
        expr: &Expr,
        handled: &BTreeSet<String>,
        op_to_effect: &BTreeMap<String, String>,
        fn_effects: &BTreeMap<String, Vec<String>>,
    ) -> BTreeSet<String> {
        let mut effects = BTreeSet::new();
        match &expr.kind {
            ExprKind::Var(name) => {
                if let Some(eff) = op_to_effect.get(name.as_str()) {
                    if !handled.contains(eff) {
                        effects.insert(eff.clone());
                    }
                }
            }
            ExprKind::App(func, args) => {
                if let ExprKind::Var(name) = &func.as_ref().kind {
                    // Direct effect op call
                    if let Some(eff) = op_to_effect.get(name.as_str()) {
                        if !handled.contains(eff) {
                            effects.insert(eff.clone());
                        }
                    }
                    // Transitive: calling a function that has effects
                    if let Some(callee_effs) = fn_effects.get(name.as_str()) {
                        for e in callee_effs {
                            if !handled.contains(e) {
                                effects.insert(e.clone());
                            }
                        }
                    }
                } else {
                    effects.extend(Self::collect_expr_effects(
                        func,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
                for arg in args {
                    effects.extend(Self::collect_expr_effects(
                        arg,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::Handle {
                effect,
                handlers,
                body,
            } => {
                // Body has this effect handled — don't propagate it
                let mut inner_handled = handled.clone();
                inner_handled.insert(effect.clone());
                effects.extend(Self::collect_expr_effects(
                    body,
                    &inner_handled,
                    op_to_effect,
                    fn_effects,
                ));
                // Handler bodies can introduce other effects
                for h in handlers {
                    effects.extend(Self::collect_expr_effects(
                        &h.body,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::Block(stmts) => {
                for stmt in stmts {
                    match stmt {
                        Stmt::Expr(e) | Stmt::Bind(_, _, e) | Stmt::MonadicBind(_, _, e) => {
                            effects.extend(Self::collect_expr_effects(
                                e,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                        }
                        Stmt::StreamBind(_, e) => {
                            effects.extend(Self::collect_expr_effects(
                                e,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                        }
                        Stmt::StreamSub(e, arms) => {
                            effects.extend(Self::collect_expr_effects(
                                e,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                            for arm in arms {
                                if let Some(g) = &arm.guard {
                                    effects.extend(Self::collect_expr_effects(
                                        g,
                                        handled,
                                        op_to_effect,
                                        fn_effects,
                                    ));
                                }
                                effects.extend(Self::collect_expr_effects(
                                    &arm.body,
                                    handled,
                                    op_to_effect,
                                    fn_effects,
                                ));
                            }
                        }
                        Stmt::For(_, iter_e, body_stmts) => {
                            effects.extend(Self::collect_expr_effects(
                                iter_e,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                            for s in body_stmts {
                                if let Stmt::Expr(e) | Stmt::Bind(_, _, e) = s {
                                    effects.extend(Self::collect_expr_effects(
                                        e,
                                        handled,
                                        op_to_effect,
                                        fn_effects,
                                    ));
                                }
                            }
                        }
                        Stmt::Send(target, msg) => {
                            effects.extend(Self::collect_expr_effects(
                                target,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                            effects.extend(Self::collect_expr_effects(
                                msg,
                                handled,
                                op_to_effect,
                                fn_effects,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::If(cond, then_, else_) => {
                effects.extend(Self::collect_expr_effects(
                    cond,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                effects.extend(Self::collect_expr_effects(
                    then_,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                effects.extend(Self::collect_expr_effects(
                    else_,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::Match(scrutinee, arms) => {
                effects.extend(Self::collect_expr_effects(
                    scrutinee,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        effects.extend(Self::collect_expr_effects(
                            g,
                            handled,
                            op_to_effect,
                            fn_effects,
                        ));
                    }
                    effects.extend(Self::collect_expr_effects(
                        &arm.body,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::BinOp(_, l, r) => {
                effects.extend(Self::collect_expr_effects(
                    l,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                effects.extend(Self::collect_expr_effects(
                    r,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::UnOp(_, e) | ExprKind::Try(e) | ExprKind::Field(e, _) => {
                effects.extend(Self::collect_expr_effects(
                    e,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::Lambda(_, body) => {
                effects.extend(Self::collect_expr_effects(
                    body,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::Index(a, b) => {
                effects.extend(Self::collect_expr_effects(
                    a,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                effects.extend(Self::collect_expr_effects(
                    b,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::Pipe(input, transform) => {
                effects.extend(Self::collect_expr_effects(
                    input,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
                effects.extend(Self::collect_expr_effects(
                    transform,
                    handled,
                    op_to_effect,
                    fn_effects,
                ));
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for item in items {
                    effects.extend(Self::collect_expr_effects(
                        item,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::Effect(_, args) => {
                for arg in args {
                    effects.extend(Self::collect_expr_effects(
                        arg,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::Conjunction(goals) => {
                for g in goals {
                    effects.extend(Self::collect_expr_effects(
                        g,
                        handled,
                        op_to_effect,
                        fn_effects,
                    ));
                }
            }
            ExprKind::Lit(_) | ExprKind::Unit => {}
        }
        effects
    }

    /// Emit an if/else branch: unwrap single-expression blocks to avoid double braces
    fn emit_if_branch(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Block(stmts) if stmts.len() == 1 => {
                if let Stmt::Expr(inner) = &stmts[0] {
                    return self.emit_expr(inner);
                }
                self.emit_expr(expr)
            }
            _ => self.emit_expr(expr),
        }
    }

    fn emit_expr_as_return(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Block(stmts) => {
                let mut out = String::new();
                for (i, stmt) in stmts.iter().enumerate() {
                    let is_last = i == stmts.len() - 1;
                    match stmt {
                        Stmt::Bind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            let mutability = if let Pat::Var(name) = pat {
                                if self.mutable_vars.contains(name.as_str()) {
                                    "mut "
                                } else {
                                    ""
                                }
                            } else {
                                ""
                            };
                            // Track typed bindings (for float division/string concat detection)
                            if let Pat::Var(var_name) = pat {
                                if self.expr_is_string(value) {
                                    self.string_typed_vars.insert(var_name.clone());
                                }
                                if self.expr_is_float(value) {
                                    self.float_typed_vars.insert(var_name.clone());
                                }
                            }
                            out.push_str(&format!(
                                "{}let {}{} = {};\n",
                                self.ind(),
                                mutability,
                                pat_str,
                                val_str
                            ));
                        }
                        Stmt::MonadicBind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            let suffix = if self.is_effect_op_call(value) {
                                ""
                            } else {
                                "?"
                            };
                            out.push_str(&format!(
                                "{}let {} = {}{};\n",
                                self.ind(),
                                pat_str,
                                val_str,
                                suffix
                            ));
                        }
                        Stmt::Expr(expr) if is_last => {
                            out.push_str(&format!("{}{}\n", self.ind(), self.emit_expr(expr)));
                        }
                        Stmt::Expr(Expr {
                            kind: ExprKind::Effect(name, args),
                            ..
                        }) if name == "print" => {
                            out.push_str(&self.emit_print(args, &self.ind()));
                        }
                        Stmt::Expr(expr) => {
                            out.push_str(&format!("{}{};\n", self.ind(), self.emit_expr(expr)));
                        }
                        Stmt::Defn(defn) => {
                            // Nested function
                            let _saved = self.indent;
                            out.push_str(&self.emit_defn(defn));
                        }
                        Stmt::RustBlock(code) => {
                            for line in code.lines() {
                                out.push_str(&format!("{}{}\n", self.ind(), line));
                            }
                        }
                        Stmt::For(..) => {
                            out.push_str(&self.emit_stmt(stmt));
                        }
                        _ => {}
                    }
                }
                out
            }
            _ => {
                format!("{}{}\n", self.ind(), self.emit_expr(expr))
            }
        }
    }

    /// Emit a tail-recursive function body as a loop.
    /// Transforms: self-calls become parameter reassignment + continue;
    /// base cases become return.
    fn emit_tce_body(
        &mut self,
        fn_name: &str,
        params: &[Param],
        borrow_flags: &[bool],
        body: &Expr,
    ) -> String {
        let ind = self.ind();
        // Make non-borrowed, non-Copy params mutable for reassignment
        let mut mutables: BTreeSet<String> = BTreeSet::new();
        for (idx, p) in params.iter().enumerate() {
            let is_borrowed = borrow_flags.get(idx).copied().unwrap_or(false);
            let is_copy = p.ty.as_ref().map(|t| is_copy_type(t)).unwrap_or(false);
            if !is_borrowed && !is_copy && !p.inout {
                mutables.insert(p.name.clone());
            }
        }
        // Emit mut rebindings for non-Copy owned params
        let mut out = String::new();
        for p in params {
            if mutables.contains(&p.name) {
                out.push_str(&format!(
                    "{}let mut {} = {};\n",
                    ind,
                    sanitize_name(&p.name),
                    sanitize_name(&p.name)
                ));
            }
        }
        // Also make Copy params mutable (they'll be reassigned in the loop)
        for (idx, p) in params.iter().enumerate() {
            let is_borrowed = borrow_flags.get(idx).copied().unwrap_or(false);
            let is_copy = p.ty.as_ref().map(|t| is_copy_type(t)).unwrap_or(false);
            if !is_borrowed && is_copy && !p.inout {
                out.push_str(&format!(
                    "{}let mut {} = {};\n",
                    ind,
                    sanitize_name(&p.name),
                    sanitize_name(&p.name)
                ));
            }
        }
        out.push_str(&format!("{}loop {{\n", ind));
        self.indent += 1;
        out.push_str(&self.emit_tce_expr(fn_name, params, borrow_flags, body));
        self.indent -= 1;
        out.push_str(&format!("{}}}\n", ind));
        out
    }

    fn emit_tce_expr(
        &mut self,
        fn_name: &str,
        params: &[Param],
        borrow_flags: &[bool],
        expr: &Expr,
    ) -> String {
        let ind = self.ind();
        // Collect param names that TCE reassigns (non-borrowed) — inner shadows need `let mut`
        let tce_mut_params: std::collections::HashSet<&str> = params
            .iter()
            .enumerate()
            .filter(|(idx, p)| !borrow_flags.get(*idx).copied().unwrap_or(false) && !p.inout)
            .map(|(_, p)| p.name.as_str())
            .collect();
        match &expr.kind {
            ExprKind::App(func, args) if matches!(func.as_ref().kind, ExprKind::Var(ref n) if n == fn_name) =>
            {
                // Self-call: emit temp assignments then reassign params + continue
                let mut out = String::new();
                // Compute all new values into temps first (simultaneous assignment)
                let mut temps = Vec::new();
                for (idx, (p, a)) in params.iter().zip(args.iter()).enumerate() {
                    let is_borrowed = borrow_flags.get(idx).copied().unwrap_or(false);
                    if is_borrowed {
                        // Borrowed param: not reassigned, skip
                        continue;
                    }
                    let val = self.emit_expr(a);
                    let tmp = format!("__tce_{}", sanitize_name(&p.name));
                    out.push_str(&format!("{}let {} = {};\n", ind, tmp, val));
                    temps.push((sanitize_name(&p.name), tmp));
                }
                // Reassign all params from temps
                for (param_name, tmp_name) in &temps {
                    out.push_str(&format!("{}{} = {};\n", ind, param_name, tmp_name));
                }
                out.push_str(&format!("{}continue;\n", ind));
                out
            }
            ExprKind::If(cond, then_, else_) => {
                let cond_str = self.emit_expr(cond);
                let mut out = format!("{}if {} {{\n", ind, cond_str);
                self.indent += 1;
                out.push_str(&self.emit_tce_expr(fn_name, params, borrow_flags, then_));
                self.indent -= 1;
                out.push_str(&format!("{}}} else {{\n", ind));
                self.indent += 1;
                out.push_str(&self.emit_tce_expr(fn_name, params, borrow_flags, else_));
                self.indent -= 1;
                out.push_str(&format!("{}}}\n", ind));
                out
            }
            ExprKind::Block(stmts) => {
                let mut out = String::new();
                for (i, stmt) in stmts.iter().enumerate() {
                    let is_last = i == stmts.len() - 1;
                    match stmt {
                        Stmt::Bind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            // Track typed bindings (same as emit_stmt Bind path)
                            if let Pat::Var(var_name) = pat {
                                if self.expr_is_string(value) {
                                    self.string_typed_vars.insert(var_name.clone());
                                }
                                if self.expr_is_float(value) {
                                    self.float_typed_vars.insert(var_name.clone());
                                }
                            }
                            // If binding shadows a TCE loop variable, reassign instead of new let
                            if let Pat::Var(name) = pat {
                                if tce_mut_params.contains(name.as_str()) {
                                    out.push_str(&format!("{}{} = {};\n", ind, pat_str, val_str));
                                } else {
                                    let mutability = if self.mutable_vars.contains(name.as_str()) {
                                        "mut "
                                    } else {
                                        ""
                                    };
                                    out.push_str(&format!(
                                        "{}let {}{} = {};\n",
                                        ind, mutability, pat_str, val_str
                                    ));
                                }
                            } else {
                                out.push_str(&format!("{}let {} = {};\n", ind, pat_str, val_str));
                            }
                        }
                        Stmt::MonadicBind(pat, _, value) => {
                            let pat_str = self.emit_pattern_binding(pat);
                            let val_str = self.emit_expr(value);
                            let suffix = if self.is_effect_op_call(value) {
                                ""
                            } else {
                                "?"
                            };
                            out.push_str(&format!(
                                "{}let {} = {}{};\n",
                                ind, pat_str, val_str, suffix
                            ));
                        }
                        Stmt::Expr(expr) if is_last => {
                            // Last expression: recurse into TCE
                            out.push_str(&self.emit_tce_expr(fn_name, params, borrow_flags, expr));
                        }
                        Stmt::Expr(Expr {
                            kind: ExprKind::Effect(name, args),
                            ..
                        }) if name == "print" => {
                            out.push_str(&self.emit_print(args, &ind));
                        }
                        Stmt::Expr(expr) => {
                            out.push_str(&format!("{}{};\n", ind, self.emit_expr(expr)));
                        }
                        Stmt::Annot(name, args) if name == "print" => {
                            out.push_str(&self.emit_print(args, &ind));
                        }
                        Stmt::For(..) => {
                            out.push_str(&self.emit_stmt(stmt));
                        }
                        _ => {}
                    }
                }
                out
            }
            ExprKind::Match(scrutinee, arms) => {
                let scrut_str = self.emit_expr(scrutinee);
                let mut out = format!("{}match {} {{\n", ind, scrut_str);
                self.indent += 1;
                for arm in arms {
                    let pat_str = self.emit_pattern_match(&arm.pat);
                    let guard_str = arm
                        .guard
                        .as_ref()
                        .map(|g| format!(" if {}", self.emit_expr(g)))
                        .unwrap_or_default();
                    out.push_str(&format!("{}{}{} => {{\n", self.ind(), pat_str, guard_str));
                    self.indent += 1;
                    out.push_str(&self.emit_tce_expr(fn_name, params, borrow_flags, &arm.body));
                    self.indent -= 1;
                    out.push_str(&format!("{}}}\n", self.ind()));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}\n", ind));
                out
            }
            // Base case: not a self-call, emit as return
            _ => {
                format!("{}return {};\n", ind, self.emit_expr(expr))
            }
        }
    }

    fn emit_literal(&self, lit: &Literal) -> String {
        match lit {
            Literal::Int(n) => format!("{}i64", n),
            Literal::Float(f) => {
                let s = format!("{}", f);
                if s.contains('.') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            Literal::Str(s) => format!("{:?}.to_string()", s),
            Literal::Char(c) => format!("'{}'", c),
            Literal::Bool(b) => format!("{}", b),
        }
    }

    /// Convert a handler pattern to an enum variant declaration: Increment or Add(i64)
    fn emit_pattern_as_enum_variant(&self, pat: &Pat) -> String {
        match pat {
            Pat::Var(name) => name.clone(),
            Pat::Wild => "_".to_string(),
            Pat::Con(name, args) if args.is_empty() => name.clone(),
            Pat::Con(name, args) => {
                let types: Vec<&str> = args.iter().map(|_| "i64").collect();
                format!("{}({})", name, types.join(", "))
            }
            _ => "Unknown".to_string(),
        }
    }

    /// Convert a handler pattern to a match arm: counterMsg::Increment or counterMsg::Add(n)
    fn emit_pattern_as_match_arm(&self, pat: &Pat, actor_name: &str) -> String {
        match pat {
            Pat::Con(name, args) if args.is_empty() => {
                format!("{}Msg::{}", actor_name, name)
            }
            Pat::Con(name, args) => {
                let ps: Vec<String> = args.iter().map(|p| self.emit_pattern_binding(p)).collect();
                format!("{}Msg::{}({})", actor_name, name, ps.join(", "))
            }
            Pat::Var(name) => format!("{}Msg::{}", actor_name, sanitize_name(name)),
            _ => format!("_ /* unhandled pattern */"),
        }
    }

    /// Check if a monadic bind value is an effect operation call (no ? needed)
    fn is_effect_op_call(&self, value: &Expr) -> bool {
        if let ExprKind::App(func, _) = &value.kind {
            if let ExprKind::Var(op_name) = &func.as_ref().kind {
                return self.current_effects.iter().any(|eff| {
                    self.types
                        .effect_ops
                        .get(eff.as_str())
                        .map(|ops| ops.contains(op_name.as_str()))
                        .unwrap_or(false)
                });
            }
        }
        false
    }

    fn emit_pattern_binding(&self, pat: &Pat) -> String {
        match pat {
            Pat::Var(name) => sanitize_name(name),
            Pat::Wild => "_".to_string(),
            Pat::Con(name, args) if args.is_empty() => {
                let parent = self.find_parent_type(name);
                format!("{}::{}", parent, name)
            }
            Pat::Con(name, args) => {
                let parent = self.find_parent_type(name);
                let is_pos = self
                    .types
                    .variant_positional
                    .get(name.as_str())
                    .copied()
                    .unwrap_or(true);
                let ps: Vec<String> = args.iter().map(|p| self.emit_pattern_binding(p)).collect();
                if is_pos {
                    format!("{}::{}({})", parent, name, ps.join(", "))
                } else {
                    // Named variant: destructure with field names
                    let fields = self.types.variant_fields.get(name.as_str());
                    let named_ps: Vec<String> = ps
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let fname = fields
                                .and_then(|f| f.get(i))
                                .map(|s| s.as_str())
                                .unwrap_or("_");
                            format!("{}: {}", fname, p)
                        })
                        .collect();
                    format!("{}::{} {{ {} }}", parent, name, named_ps.join(", "))
                }
            }
            Pat::NamedCon(name, named_args) => {
                let parent = self.find_parent_type(name);
                let ps: Vec<String> = named_args
                    .iter()
                    .map(|(fname, p)| format!("{}: {}", fname, self.emit_pattern_binding(p)))
                    .collect();
                format!("{}::{} {{ {} }}", parent, name, ps.join(", "))
            }
            Pat::Lit(lit) => self.emit_literal(lit),
            Pat::As(inner, name) => {
                format!(
                    "{} @ {}",
                    self.emit_pattern_binding(inner),
                    sanitize_name(name)
                )
            }
        }
    }

    fn emit_pattern_match(&self, pat: &Pat) -> String {
        self.emit_pattern_with_boxing(pat, false)
    }

    /// Emit a pattern, handling boxed fields properly
    /// When `inside_box` is true, this pattern is matching against a Box<T>
    fn emit_pattern_with_boxing(&self, pat: &Pat, inside_box: bool) -> String {
        match pat {
            Pat::Var(name) => sanitize_name(name),
            Pat::Wild => "_".to_string(),
            Pat::Con(name, args) if args.is_empty() => {
                // Boolean literals: True/False → Rust's true/false
                if name == "True" {
                    return "true".to_string();
                }
                if name == "False" {
                    return "false".to_string();
                }
                let parent = self.find_parent_type(name);
                if self.types.struct_types.contains(&parent) {
                    name.clone()
                } else {
                    format!("{}::{}", parent, name)
                }
            }
            Pat::Con(name, args) => {
                let parent = self.find_parent_type(name);
                let is_struct_type = self.types.struct_types.contains(&parent);
                let is_pos = self
                    .types
                    .variant_positional
                    .get(name.as_str())
                    .copied()
                    .unwrap_or(true);
                let boxed_indices = self.types.variant_boxed_args.get(name.as_str());
                let ps: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let is_boxed = boxed_indices.map_or(false, |bi| bi.contains(&i));
                        if is_boxed {
                            match p {
                                Pat::Var(_) | Pat::Wild => self.emit_pattern_with_boxing(p, true),
                                _ => format!("__boxed_{}", i),
                            }
                        } else {
                            self.emit_pattern_with_boxing(p, false)
                        }
                    })
                    .collect();
                if is_pos {
                    if is_struct_type {
                        format!("{}({})", parent, ps.join(", "))
                    } else {
                        format!("{}::{}({})", parent, name, ps.join(", "))
                    }
                } else {
                    let fields = self.types.variant_fields.get(name.as_str());
                    let named_ps: Vec<String> = ps
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let fname = fields
                                .and_then(|f| f.get(i))
                                .map(|s| s.as_str())
                                .unwrap_or("_");
                            format!("{}: {}", fname, p)
                        })
                        .collect();
                    if is_struct_type {
                        format!("{} {{ {} }}", parent, named_ps.join(", "))
                    } else {
                        format!("{}::{} {{ {} }}", parent, name, named_ps.join(", "))
                    }
                }
            }
            Pat::NamedCon(name, named_args) => {
                let parent = self.find_parent_type(name);
                let is_struct_type = self.types.struct_types.contains(&parent);
                let ps: Vec<String> = named_args
                    .iter()
                    .map(|(fname, p)| {
                        format!("{}: {}", fname, self.emit_pattern_with_boxing(p, false))
                    })
                    .collect();
                if is_struct_type {
                    format!("{} {{ {} }}", parent, ps.join(", "))
                } else {
                    format!("{}::{} {{ {} }}", parent, name, ps.join(", "))
                }
            }
            Pat::Lit(lit) => self.emit_literal(lit),
            Pat::As(inner, name) => {
                format!(
                    "{} @ {}",
                    self.emit_pattern_with_boxing(inner, inside_box),
                    sanitize_name(name)
                )
            }
        }
    }

    /// Check if a pattern has nested constructor patterns in boxed positions
    fn has_boxed_constructor_patterns(&self, pat: &Pat) -> bool {
        if let Pat::Con(name, args) = pat {
            if let Some(boxed_indices) = self.types.variant_boxed_args.get(name.as_str()) {
                for (i, sub_pat) in args.iter().enumerate() {
                    if boxed_indices.contains(&i) {
                        if matches!(sub_pat, Pat::Con(_, _) | Pat::Lit(_)) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Generate a guard expression for boxed constructor patterns
    fn emit_boxed_pattern_guard(&self, pat: &Pat) -> Option<String> {
        if let Pat::Con(name, args) = pat {
            if let Some(boxed_indices) = self.types.variant_boxed_args.get(name.as_str()) {
                let is_rc = self.pattern_is_rc_type(pat);
                let mut guards = Vec::new();
                for (i, sub_pat) in args.iter().enumerate() {
                    if boxed_indices.contains(&i) {
                        // Rc: use .as_ref() (can't deref-move); Box: use *
                        let deref_expr = if is_rc {
                            format!("__boxed_{}.as_ref()", i)
                        } else {
                            format!("*__boxed_{}", i)
                        };
                        match sub_pat {
                            Pat::Con(sub_name, sub_args) if sub_args.is_empty() => {
                                let parent = self.find_parent_type(sub_name);
                                guards.push(format!(
                                    "matches!({}, {}::{})",
                                    deref_expr, parent, sub_name
                                ));
                            }
                            Pat::Con(sub_name, _) => {
                                let parent = self.find_parent_type(sub_name);
                                guards.push(format!(
                                    "matches!({}, {}::{}(..))",
                                    deref_expr, parent, sub_name
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                if !guards.is_empty() {
                    return Some(guards.join(" && "));
                }
            }
        }
        None
    }

    fn find_parent_type(&self, variant_name: &str) -> String {
        self.types
            .variant_parent
            .get(variant_name)
            .cloned()
            .unwrap_or_else(|| {
                // Fallback heuristic for built-in types not declared in source
                match variant_name {
                    "None" | "Some" => "Option".to_string(),
                    "Nil" | "Cons" => "List".to_string(),
                    "True" | "False" => "Bool".to_string(),
                    "Ok" | "Err" => "Result".to_string(),
                    _ => variant_name.to_string(),
                }
            })
    }
}

fn sanitize_name(name: &str) -> String {
    match name {
        "type" | "match" | "fn" | "let" | "mut" | "ref" | "super" | "mod" | "use" | "pub"
        | "impl" | "trait" | "where" | "for" | "loop" | "while" | "break" | "continue"
        | "return" | "async" | "await" | "move" | "static" | "const" | "struct" | "enum" => {
            format!("r#{}", name)
        }
        _ => name.to_string(),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fir_var_with_move() {
        let expr = FirExpr {
            kind: FirExprKind::Var("x".into(), VarMode::Move),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert!(matches!(expr.kind, FirExprKind::Var(_, VarMode::Move)));
    }

    #[test]
    fn fir_var_with_clone() {
        let expr = FirExpr {
            kind: FirExprKind::Var("name".into(), VarMode::Clone),
            span: Span::dummy(),
            ty: FirTy::String,
        };
        assert!(matches!(expr.kind, FirExprKind::Var(_, VarMode::Clone)));
        assert_eq!(expr.ty, FirTy::String);
    }

    #[test]
    fn fir_binop_carries_type() {
        let lhs = FirExpr {
            kind: FirExprKind::Lit(Literal::Int(1)),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        let rhs = FirExpr {
            kind: FirExprKind::Lit(Literal::Int(2)),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        let add = FirExpr {
            kind: FirExprKind::BinOp("+".into(), Box::new(lhs), Box::new(rhs)),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert_eq!(add.ty, FirTy::Int);
    }

    #[test]
    fn fir_program_holds_stmts_and_types() {
        let prog = FirProgram {
            stmts: vec![],
            types: TypeRegistry::new(),
        };
        assert!(prog.stmts.is_empty());
    }

    #[test]
    fn fir_match_arm_with_guard() {
        let arm = FirMatchArm {
            pat: Pat::Var("x".into()),
            guard: Some(FirExpr {
                kind: FirExprKind::BinOp(
                    ">".into(),
                    Box::new(FirExpr {
                        kind: FirExprKind::Var("x".into(), VarMode::Copy),
                        span: Span::dummy(),
                        ty: FirTy::Int,
                    }),
                    Box::new(FirExpr {
                        kind: FirExprKind::Lit(Literal::Int(0)),
                        span: Span::dummy(),
                        ty: FirTy::Int,
                    }),
                ),
                span: Span::dummy(),
                ty: FirTy::Bool,
            }),
            body: FirExpr {
                kind: FirExprKind::Var("x".into(), VarMode::Copy),
                span: Span::dummy(),
                ty: FirTy::Int,
            },
        };
        assert!(arm.guard.is_some());
    }

    #[test]
    fn ownership_analysis_simple() {
        // Parse "= x = a + a" — 'a' used twice → should have count 2
        let source = "= x = a + a";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");
        if let Stmt::Bind(_, _, ref expr) = stmts[0] {
            let analysis = OwnershipAnalysis::analyze_simple(expr);
            assert_eq!(
                analysis.var_uses.get("a").copied().unwrap_or(0),
                2,
                "expected 'a' used twice"
            );
        } else {
            panic!("expected Bind statement");
        }
    }

    #[test]
    fn var_mode_clone_for_multi_use() {
        // If a non-Copy var has consuming_uses > 1, it should be Clone
        let analysis = OwnershipAnalysis {
            var_uses: [("s".into(), 2)].into(),
            consuming_uses: [("s".into(), 2)].into(),
        };
        // Simulate what emit_expr does: multi-use non-Copy → Clone
        let mode = if analysis.consuming_uses.get("s").copied().unwrap_or(0) > 1 {
            VarMode::Clone
        } else {
            VarMode::Move
        };
        assert_eq!(mode, VarMode::Clone);
    }

    // ── Lowering tests ──────────────────────────────────────────────

    /// Helper: parse source, compute ownership, lower to FIR
    fn lower_source(source: &str) -> Vec<FirStmt> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        // Find the expression from a Bind statement
        let types = TypeRegistry::new();
        let copy_vars = BTreeSet::new();
        let ref_match = BTreeSet::new();

        // Compute ownership for the whole program
        let ownership = OwnershipAnalysis {
            var_uses: BTreeMap::new(),
            consuming_uses: BTreeMap::new(),
        };

        let mut ctx = LoweringCtx {
            type_env: BTreeMap::new(),
            inference: None,
            fn_schemes: BTreeMap::new(),
            types: &types,
            ownership: &ownership,
            copy_vars: &copy_vars,
            ref_match_bindings: &ref_match,
        };

        stmts.iter().map(|s| ctx.lower_stmt(s)).collect()
    }

    #[test]
    fn lowering_var_produces_fir_var() {
        let fir = lower_source("= x = hello");
        if let FirStmt::Bind(_, _, ref expr) = fir[0] {
            assert!(matches!(expr.kind, FirExprKind::Var(ref n, _) if n == "hello"));
        } else {
            panic!("expected FirStmt::Bind");
        }
    }

    #[test]
    fn lowering_binop_produces_fir_binop() {
        let fir = lower_source("= x = 1 + 2");
        if let FirStmt::Bind(_, _, ref expr) = fir[0] {
            assert!(matches!(expr.kind, FirExprKind::BinOp(ref op, _, _) if op == "+"));
        } else {
            panic!("expected FirStmt::Bind");
        }
    }

    #[test]
    fn lowering_preserves_span() {
        let source = "= x = hello";
        let fir = lower_source(source);
        if let FirStmt::Bind(_, _, ref expr) = fir[0] {
            assert!(!expr.span.is_dummy(), "FIR expr should preserve AST span");
        } else {
            panic!("expected FirStmt::Bind");
        }
    }

    #[test]
    fn lowering_function_def() {
        let fir = lower_source("> add(a: Int, b: Int) -> Int { a + b }");
        assert!(matches!(fir[0], FirStmt::Defn(FirDefn::Fn { ref name, .. }) if name == "add"));
    }

    #[test]
    fn lowering_multi_use_var_in_call_gets_clone() {
        // 'a' passed twice to a function call — consuming positions → Clone
        let source = "> use_twice(x: String, y: String) -> String { x + y }\n= x = use_twice(a, a)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let copy_vars = BTreeSet::new();
        let ref_match = BTreeSet::new();

        // Find the bind statement (second stmt after the fn def)
        if let Stmt::Bind(_, _, ref expr) = stmts[1] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);

            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &copy_vars,
                ref_match_bindings: &ref_match,
            };

            let fir_expr = ctx.lower_expr(expr);
            // use_twice(a, a) → App with two Var("a", Clone) args
            if let FirExprKind::App(_, ref args) = fir_expr.kind {
                assert!(args.len() == 2);
                assert!(
                    matches!(args[0].kind, FirExprKind::Var(ref n, VarMode::Clone) if n == "a"),
                    "expected Clone for multi-use 'a' in call, got: {:?}",
                    args[0].kind
                );
                assert!(
                    matches!(args[1].kind, FirExprKind::Var(ref n, VarMode::Clone) if n == "a")
                );
            } else {
                panic!("expected App, got: {:?}", fir_expr.kind);
            }
        } else {
            panic!("expected Bind at stmts[1]");
        }
    }

    #[test]
    fn lowering_binop_operands_are_not_consuming() {
        // 'a + a' — BinOp operands are not consuming uses, so 'a' should be Move
        let source = "= x = a + a";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let copy_vars = BTreeSet::new();
        let ref_match = BTreeSet::new();

        if let Stmt::Bind(_, _, ref expr) = stmts[0] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);
            // BinOp operands are NOT consuming — consuming_uses for 'a' should be 0
            assert_eq!(
                ownership.consuming_uses.get("a").copied().unwrap_or(0),
                0,
                "BinOp operands should not count as consuming uses"
            );

            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &copy_vars,
                ref_match_bindings: &ref_match,
            };
            let fir_expr = ctx.lower_expr(expr);
            if let FirExprKind::BinOp(_, ref lhs, _) = fir_expr.kind {
                // 'a' has 0 consuming uses → Move (not Clone)
                assert!(
                    matches!(lhs.kind, FirExprKind::Var(_, VarMode::Move)),
                    "expected Move for non-consuming BinOp use, got: {:?}",
                    lhs.kind
                );
            }
        }
    }

    #[test]
    fn lowering_copy_var_gets_copy_mode() {
        let source = "= x = n + n";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let mut copy_vars = BTreeSet::new();
        copy_vars.insert("n".to_string()); // mark 'n' as Copy
        let ref_match = BTreeSet::new();

        if let Stmt::Bind(_, _, ref expr) = stmts[0] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &copy_vars,
                ref_match_bindings: &ref_match,
            };
            let fir_expr = ctx.lower_expr(expr);
            if let FirExprKind::BinOp(_, ref lhs, _) = fir_expr.kind {
                assert!(
                    matches!(lhs.kind, FirExprKind::Var(ref n, VarMode::Copy) if n == "n"),
                    "expected Copy for Copy-typed 'n', got: {:?}",
                    lhs.kind
                );
            }
        }
    }

    // ── FIR end-to-end pipeline test ────────────────────────────────

    #[test]
    fn fir_pipeline_add_function() {
        // Full pipeline: source → parse → ownership → lower → emit
        let source = "> add(a: Int, b: Int) -> Int { a + b }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let borrow_params = BTreeMap::new();
        let copy_vars = BTreeSet::new();
        let ref_match = BTreeSet::new();

        let code = emit_via_fir(&stmts, &types, &borrow_params, &copy_vars, &ref_match);
        assert!(
            code.contains("fn add(a: i64, b: i64) -> i64"),
            "expected function signature, got:\n{}",
            code
        );
        assert!(
            code.contains("(a + b)"),
            "expected body expression, got:\n{}",
            code
        );
    }

    #[test]
    fn fir_pipeline_binding() {
        let source = "= x = 42";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let code = emit_via_fir(
            &stmts,
            &types,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert!(
            code.contains("let x = 42i64;"),
            "expected let binding, got:\n{}",
            code
        );
    }

    // ── FIR emission tests ──────────────────────────────────────────

    fn fir_var(name: &str, mode: VarMode) -> FirExpr {
        FirExpr {
            kind: FirExprKind::Var(name.into(), mode),
            span: Span::dummy(),
            ty: FirTy::Unknown,
        }
    }
    fn fir_int(n: i64) -> FirExpr {
        FirExpr {
            kind: FirExprKind::Lit(Literal::Int(n)),
            span: Span::dummy(),
            ty: FirTy::Int,
        }
    }
    fn fir_str(s: &str) -> FirExpr {
        FirExpr {
            kind: FirExprKind::Lit(Literal::Str(s.into())),
            span: Span::dummy(),
            ty: FirTy::String,
        }
    }

    #[test]
    fn emit_fir_var_move() {
        let types = TypeRegistry::new();
        assert_eq!(emit_fir_expr(&fir_var("x", VarMode::Move), &types), "x");
    }

    #[test]
    fn emit_fir_var_clone() {
        let types = TypeRegistry::new();
        assert_eq!(
            emit_fir_expr(&fir_var("name", VarMode::Clone), &types),
            "name.clone()"
        );
    }

    #[test]
    fn emit_fir_var_deref() {
        let types = TypeRegistry::new();
        assert_eq!(
            emit_fir_expr(&fir_var("val", VarMode::Deref), &types),
            "(*val)"
        );
    }

    #[test]
    fn emit_fir_var_borrow() {
        let types = TypeRegistry::new();
        assert_eq!(
            emit_fir_expr(&fir_var("data", VarMode::Borrow), &types),
            "&data"
        );
    }

    #[test]
    fn emit_fir_int_literal() {
        let types = TypeRegistry::new();
        assert_eq!(emit_fir_expr(&fir_int(42), &types), "42i64");
    }

    #[test]
    fn emit_fir_string_literal() {
        let types = TypeRegistry::new();
        assert_eq!(
            emit_fir_expr(&fir_str("hello"), &types),
            "\"hello\".to_string()"
        );
    }

    #[test]
    fn emit_fir_binop() {
        let types = TypeRegistry::new();
        let expr = FirExpr {
            kind: FirExprKind::BinOp("+".into(), Box::new(fir_int(1)), Box::new(fir_int(2))),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert_eq!(emit_fir_expr(&expr, &types), "(1i64 + 2i64)");
    }

    #[test]
    fn emit_fir_function_call() {
        let types = TypeRegistry::new();
        let expr = FirExpr {
            kind: FirExprKind::App(
                Box::new(fir_var("add", VarMode::Move)),
                vec![fir_int(1), fir_int(2)],
            ),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert_eq!(emit_fir_expr(&expr, &types), "add(1i64, 2i64)");
    }

    #[test]
    fn emit_fir_if_else() {
        let types = TypeRegistry::new();
        let expr = FirExpr {
            kind: FirExprKind::If(
                Box::new(FirExpr {
                    kind: FirExprKind::Lit(Literal::Bool(true)),
                    span: Span::dummy(),
                    ty: FirTy::Bool,
                }),
                Box::new(fir_int(1)),
                Box::new(fir_int(0)),
            ),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert_eq!(
            emit_fir_expr(&expr, &types),
            "if true { 1i64 } else { 0i64 }"
        );
    }

    #[test]
    fn emit_fir_pipe_simple() {
        // x |> f → f(x)
        let types = TypeRegistry::new();
        let expr = FirExpr {
            kind: FirExprKind::Pipe(
                Box::new(fir_int(42)),
                Box::new(fir_var("double", VarMode::Move)),
            ),
            span: Span::dummy(),
            ty: FirTy::Int,
        };
        assert_eq!(emit_fir_expr(&expr, &types), "double(42i64)");
    }

    #[test]
    fn emit_fir_list_literal() {
        let types = TypeRegistry::new();
        let expr = FirExpr {
            kind: FirExprKind::List(vec![fir_int(1), fir_int(2), fir_int(3)]),
            span: Span::dummy(),
            ty: FirTy::List(Box::new(FirTy::Int)),
        };
        assert_eq!(emit_fir_expr(&expr, &types), "vec![1i64, 2i64, 3i64]");
    }

    #[test]
    fn emit_fir_end_to_end_lower_and_emit() {
        // Full pipeline: source → AST → FIR → Rust
        let source = "= x = 1 + 2";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        let copy_vars = BTreeSet::new();
        let ref_match = BTreeSet::new();

        if let Stmt::Bind(_, _, ref expr) = stmts[0] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &copy_vars,
                ref_match_bindings: &ref_match,
            };
            let fir = ctx.lower_expr(expr);
            let rust = emit_fir_expr(&fir, &types);
            assert_eq!(rust, "(1i64 + 2i64)");
        }
    }

    // ── scan_declarations tests ───────────────────────────────────

    #[test]
    fn scan_populates_type_registry() {
        let source = "# Color = Red | Green | Blue\n> name(c: Color) -> String { match c { | Red -> \"r\" | Green -> \"g\" | Blue -> \"b\" } }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let mut cg = RustCodegen::new();
        let resolved = cg.scan_declarations(&stmts);

        // TypeRegistry should know about the Color type and its variants
        assert!(
            cg.types.variant_parent.contains_key("Red"),
            "Red should be registered"
        );
        assert!(
            cg.types.variant_parent.contains_key("Green"),
            "Green should be registered"
        );
        assert!(
            cg.types.variant_parent.contains_key("Blue"),
            "Blue should be registered"
        );
        assert!(!resolved.is_empty(), "resolved stmts should not be empty");
    }

    #[test]
    fn scan_detects_struct_types() {
        let source = "# Point(x: Float, y: Float)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let mut cg = RustCodegen::new();
        cg.scan_declarations(&stmts);

        assert!(
            cg.types.struct_types.contains("Point"),
            "Point should be detected as struct type"
        );
    }

    #[test]
    fn scan_registers_user_functions() {
        let source =
            "> add(a: Int, b: Int) -> Int { a + b }\n> mul(a: Int, b: Int) -> Int { a * b }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let mut cg = RustCodegen::new();
        cg.scan_declarations(&stmts);

        assert!(
            cg.types.user_functions.contains("add"),
            "add should be registered"
        );
        assert!(
            cg.types.user_functions.contains("mul"),
            "mul should be registered"
        );
    }

    #[test]
    fn scan_and_emit_program_produce_same_output() {
        // scan_declarations + emit_program should give same result as emit_program alone
        let source = "> add(a: Int, b: Int) -> Int { a + b }\n= x = add(1, 2)\n@ print(show(x))";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let mut cg = RustCodegen::new();
        let output = cg.emit_program(&stmts);

        // emit_program internally calls scan_declarations, so the output should be valid Rust
        assert!(
            output.contains("fn add("),
            "output should contain add function"
        );
        assert!(output.contains("fn main()"), "output should contain main");
    }

    // ── FIR pipeline coverage tests ─────────────────────────────────

    /// Helper: emit a program through the FIR pipeline
    fn fir_emit(source: &str) -> String {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");
        let types = TypeRegistry::new();
        emit_via_fir(
            &stmts,
            &types,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
    }

    #[test]
    fn fir_emit_if_expression() {
        let code = fir_emit("> max(a: Int, b: Int) -> Int { if a > b { a } else { b } }");
        assert!(
            code.contains("fn max(a: i64, b: i64) -> i64"),
            "missing sig:\n{}",
            code
        );
        assert!(code.contains("if (a > b)"), "missing if:\n{}", code);
    }

    #[test]
    fn fir_emit_match_expression() {
        let code = fir_emit("# Color = Red | Green | Blue\n> name(c: Color) -> String { match c { | Red -> \"red\" | Green -> \"green\" | Blue -> \"blue\" } }");
        assert!(code.contains("fn name("), "missing fn:\n{}", code);
        assert!(code.contains("match c"), "missing match:\n{}", code);
        assert!(code.contains("Red =>"), "missing arm:\n{}", code);
    }

    #[test]
    fn fir_emit_lambda() {
        let code = fir_emit("= f = |x| x + 1");
        assert!(code.contains("|x|"), "missing lambda:\n{}", code);
        assert!(code.contains("(x + 1"), "missing body:\n{}", code);
    }

    #[test]
    fn fir_emit_list_literal() {
        let code = fir_emit("= xs = [1, 2, 3]");
        assert!(
            code.contains("vec![1i64, 2i64, 3i64]"),
            "missing list:\n{}",
            code
        );
    }

    #[test]
    fn fir_emit_pipe() {
        let code = fir_emit("> double(x: Int) -> Int { x * 2 }\n= y = 21 |> double");
        assert!(
            code.contains("double(21i64)"),
            "pipe should desugar to call:\n{}",
            code
        );
    }

    #[test]
    fn fir_emit_field_access() {
        let code = fir_emit("= x = obj.name");
        assert!(code.contains("obj.name"), "missing field access:\n{}", code);
    }

    #[test]
    fn fir_emit_for_loop() {
        let code = fir_emit("= xs = [1, 2, 3]\nfor x in xs { @ print(show(x)) }");
        assert!(code.contains("for x in"), "missing for loop:\n{}", code);
    }

    #[test]
    fn fir_emit_clone_multi_use() {
        // When a non-Copy var is used twice in function args, it should clone
        let source =
            "> use_both(a: String, b: String) -> String { a + b }\n= result = use_both(name, name)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        let types = TypeRegistry::new();
        // The bind expression is stmts[1]: = result = use_both(name, name)
        if let Stmt::Bind(_, _, ref expr) = stmts[1] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &BTreeSet::new(),
                ref_match_bindings: &BTreeSet::new(),
            };
            let fir = ctx.lower_expr(expr);
            let rust = emit_fir_expr(&fir, &types);
            assert!(
                rust.contains(".clone()"),
                "multi-use non-Copy should clone:\n{}",
                rust
            );
        }
    }

    // ── Type resolution tests ───────────────────────────────────────

    /// Helper: lower an expression with a type environment and return FirExpr
    fn lower_with_types(source: &str, type_env: BTreeMap<String, FirTy>) -> FirExpr {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");
        let types = TypeRegistry::new();
        if let Stmt::Bind(_, _, ref expr) = stmts[0] {
            let ownership = OwnershipAnalysis::analyze_simple(expr);
            let mut ctx = LoweringCtx {
                type_env,
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &BTreeSet::new(),
                ref_match_bindings: &BTreeSet::new(),
            };
            ctx.lower_expr(expr)
        } else {
            panic!("expected Bind");
        }
    }

    #[test]
    fn type_resolution_int_literal() {
        let fir = lower_with_types("= x = 42", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Int);
    }

    #[test]
    fn type_resolution_float_literal() {
        let fir = lower_with_types("= x = 3.14", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Float);
    }

    #[test]
    fn type_resolution_string_literal() {
        let fir = lower_with_types("= x = \"hello\"", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::String);
    }

    #[test]
    fn type_resolution_bool_literal() {
        let fir = lower_with_types("= x = True", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Bool);
    }

    #[test]
    fn type_resolution_int_binop() {
        let fir = lower_with_types("= x = 1 + 2", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Int, "Int + Int should be Int");
    }

    #[test]
    fn type_resolution_comparison() {
        let fir = lower_with_types("= x = 1 > 2", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Bool, "comparison should be Bool");
    }

    #[test]
    fn type_resolution_float_arithmetic() {
        let mut env = BTreeMap::new();
        env.insert("a".into(), FirTy::Float);
        env.insert("b".into(), FirTy::Float);
        let fir = lower_with_types("= x = a + b", env);
        assert_eq!(fir.ty, FirTy::Float, "Float + Float should be Float");
    }

    #[test]
    fn type_resolution_string_concat() {
        let mut env = BTreeMap::new();
        env.insert("a".into(), FirTy::String);
        env.insert("b".into(), FirTy::String);
        let fir = lower_with_types("= x = a + b", env);
        assert_eq!(fir.ty, FirTy::String, "String + String should be String");
    }

    #[test]
    fn type_resolution_var_from_env() {
        let mut env = BTreeMap::new();
        env.insert("myvar".into(), FirTy::Int);
        let fir = lower_with_types("= x = myvar", env);
        assert_eq!(fir.ty, FirTy::Int, "var should resolve from type env");
    }

    #[test]
    fn type_resolution_list_literal() {
        let fir = lower_with_types("= x = [1, 2, 3]", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::List(Box::new(FirTy::Int)));
    }

    #[test]
    fn type_resolution_if_takes_branch_type() {
        let fir = lower_with_types("= x = if True { 42 } else { 0 }", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Int, "if/else type should be branch type");
    }

    #[test]
    fn type_resolution_unit() {
        let fir = lower_with_types("= x = ()", BTreeMap::new());
        assert_eq!(fir.ty, FirTy::Unit);
    }

    // ── Unification engine tests ──────────────────────────────────

    #[test]
    fn unify_var_with_int() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        assert!(inf.unify(&t0, &FirTy::Int).is_ok());
        assert_eq!(inf.resolve(&t0), FirTy::Int);
    }

    #[test]
    fn unify_two_vars() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        let t1 = inf.fresh();
        assert!(inf.unify(&t0, &t1).is_ok());
        assert!(inf.unify(&t1, &FirTy::String).is_ok());
        assert_eq!(inf.resolve(&t0), FirTy::String);
        assert_eq!(inf.resolve(&t1), FirTy::String);
    }

    #[test]
    fn unify_same_concrete() {
        let mut inf = TypeInference::new();
        assert!(inf.unify(&FirTy::Int, &FirTy::Int).is_ok());
    }

    #[test]
    fn unify_different_concrete_fails() {
        let mut inf = TypeInference::new();
        assert!(inf.unify(&FirTy::Int, &FirTy::String).is_err());
    }

    #[test]
    fn unify_list_types() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        let list_t0 = FirTy::List(Box::new(t0.clone()));
        let list_int = FirTy::List(Box::new(FirTy::Int));
        assert!(inf.unify(&list_t0, &list_int).is_ok());
        assert_eq!(inf.resolve(&t0), FirTy::Int);
    }

    #[test]
    fn unify_arrow_types() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        let t1 = inf.fresh();
        let arrow1 = FirTy::Arrow(Box::new(t0.clone()), Box::new(t1.clone()));
        let arrow2 = FirTy::Arrow(Box::new(FirTy::Int), Box::new(FirTy::Bool));
        assert!(inf.unify(&arrow1, &arrow2).is_ok());
        assert_eq!(inf.resolve(&t0), FirTy::Int);
        assert_eq!(inf.resolve(&t1), FirTy::Bool);
    }

    #[test]
    fn unify_occurs_check() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        let list_t0 = FirTy::List(Box::new(t0.clone()));
        // t0 = List(t0) would be infinite — should fail
        assert!(inf.unify(&t0, &list_t0).is_err());
    }

    #[test]
    fn unify_unknown_wildcard() {
        let mut inf = TypeInference::new();
        // Unknown unifies with anything
        assert!(inf.unify(&FirTy::Unknown, &FirTy::Int).is_ok());
        assert!(inf.unify(&FirTy::String, &FirTy::Unknown).is_ok());
    }

    #[test]
    fn substitute_resolves_vars_in_expr() {
        let mut inf = TypeInference::new();
        let t0 = inf.fresh();
        inf.unify(&t0, &FirTy::Int).unwrap();

        let mut expr = FirExpr {
            kind: FirExprKind::Var("x".into(), VarMode::Move),
            span: Span::dummy(),
            ty: t0,
        };
        inf.substitute_expr(&mut expr);
        assert_eq!(expr.ty, FirTy::Int);
    }

    // ── Type inference tests ──────────────────────────────────────

    /// Helper: infer types for a function via LoweringCtx
    fn infer_fn(source: &str) -> (FirExpr, BTreeMap<String, FirTy>) {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        if let Stmt::Defn(Defn::Fn {
            name,
            params,
            body,
            ret_ty,
            ..
        }) = &stmts[0]
        {
            let types = TypeRegistry::new();
            let ownership = OwnershipAnalysis::analyze_simple(body);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &BTreeSet::new(),
                ref_match_bindings: &BTreeSet::new(),
            };
            let fir = ctx.infer_function(params, body, ret_ty.as_ref(), Some(name.as_str()));
            let env = ctx.type_env.clone();
            (fir, env)
        } else {
            panic!("expected function definition");
        }
    }

    #[test]
    fn infer_annotated_params_keep_types() {
        let (fir, env) = infer_fn("> add(a: Int, b: Int) -> Int { a + b }");
        assert_eq!(env.get("a"), Some(&FirTy::Int));
        assert_eq!(env.get("b"), Some(&FirTy::Int));
        assert_eq!(fir.ty, FirTy::Int);
    }

    #[test]
    fn infer_unannotated_param_from_return_type() {
        // x has no annotation but body is returned as Int → x should be inferred
        let (fir, env) = infer_fn("> double(x) -> Int { x + x }");
        // x is used in x + x where + defaults to Int, and return type is Int
        assert_eq!(fir.ty, FirTy::Int, "body should be Int");
        // x should be inferred as Int from the arithmetic
        assert_eq!(
            env.get("x"),
            Some(&FirTy::Int),
            "x should be inferred as Int"
        );
    }

    #[test]
    fn infer_body_type_from_return_annotation() {
        let (fir, _) = infer_fn("> greet(name: String) -> String { name }");
        assert_eq!(fir.ty, FirTy::String);
    }

    #[test]
    fn generalize_polymorphic_function() {
        // > id(x) { x } → should be generic (x is unresolved type var)
        let source = "> id(x) { x }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        if let Stmt::Defn(Defn::Fn {
            name,
            params,
            body,
            ret_ty,
            ..
        }) = &stmts[0]
        {
            let types = TypeRegistry::new();
            let ownership = OwnershipAnalysis::analyze_simple(body);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &BTreeSet::new(),
                ref_match_bindings: &BTreeSet::new(),
            };
            let fir = ctx.infer_function(params, body, ret_ty.as_ref(), Some(name.as_str()));

            // id should have a type scheme with generics
            assert!(
                ctx.fn_schemes.contains_key("id"),
                "id should be polymorphic, got env: {:?}, schemes: {:?}",
                ctx.type_env,
                ctx.fn_schemes
            );
            let scheme = &ctx.fn_schemes["id"];
            assert!(
                !scheme.generics.is_empty(),
                "id should have generic type vars"
            );
        }
    }

    #[test]
    fn instantiate_polymorphic_at_call_site() {
        // id(42) should instantiate id's scheme and infer return type Int
        let mut inf = TypeInference::new();
        let a = inf.fresh(); // a = _t0
        let scheme = TypeScheme {
            generics: inf.free_vars(&a),
            ty: FirTy::Arrow(Box::new(a.clone()), Box::new(a.clone())),
        };
        // Instantiate: creates fresh _t1
        let inst = inf.instantiate(&scheme.ty, &scheme.generics);
        // Unify arg type (Int) with param type
        if let FirTy::Arrow(param, _) = &inst {
            inf.unify(param, &FirTy::Int).unwrap();
        }
        // Resolve return type
        if let FirTy::Arrow(_, ret) = &inst {
            assert_eq!(inf.resolve(ret), FirTy::Int, "id(42) should return Int");
        }
    }

    #[test]
    fn monomorphic_function_not_generalized() {
        let source = "> add(a: Int, b: Int) -> Int { a + b }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens, source);
        let stmts = parser.parse_program().expect("parse failed");

        if let Stmt::Defn(Defn::Fn {
            name,
            params,
            body,
            ret_ty,
            ..
        }) = &stmts[0]
        {
            let types = TypeRegistry::new();
            let ownership = OwnershipAnalysis::analyze_simple(body);
            let mut ctx = LoweringCtx {
                type_env: BTreeMap::new(),
                inference: None,
                fn_schemes: BTreeMap::new(),
                types: &types,
                ownership: &ownership,
                copy_vars: &BTreeSet::new(),
                ref_match_bindings: &BTreeSet::new(),
            };
            let fir = ctx.infer_function(params, body, ret_ty.as_ref(), Some(name.as_str()));

            // add is monomorphic — should NOT be in fn_schemes
            assert!(
                !ctx.fn_schemes.contains_key("add"),
                "add should be monomorphic, not in schemes"
            );
            // Should be in type_env instead
            assert!(
                ctx.type_env.contains_key("add"),
                "add should be in type_env as Arrow(Int, Arrow(Int, Int))"
            );
        }
    }

    #[test]
    fn ty_to_fir_conversion() {
        assert_eq!(LoweringCtx::ty_to_fir(&Ty::Name("Int".into())), FirTy::Int);
        assert_eq!(
            LoweringCtx::ty_to_fir(&Ty::Name("String".into())),
            FirTy::String
        );
        assert_eq!(LoweringCtx::ty_to_fir(&Ty::Unit), FirTy::Unit);
        assert_eq!(
            LoweringCtx::ty_to_fir(&Ty::App(
                Box::new(Ty::Name("List".into())),
                vec![Ty::Name("Int".into())]
            )),
            FirTy::List(Box::new(FirTy::Int))
        );
    }
}
