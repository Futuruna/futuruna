//! `runa add` and `runa fetch`: the only commands that download dependencies.
//! Every other command resolves imports from the checkouts recorded in
//! `runa.lock` (see `futuruna::manifest`).

use futuruna::manifest::{
    self, Dependency, DependencySource, LockedDependency, Lockfile, Manifest,
};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static FETCH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn fail(message: impl std::fmt::Display) -> ! {
    status_eprintln!("\x1b[1;31merror\x1b[0m: {}", message);
    std::process::exit(1);
}

fn project_manifest() -> Manifest {
    let cwd = std::env::current_dir()
        .unwrap_or_else(|error| fail(format!("cannot read the current directory: {error}")));
    let Some(path) = manifest::find_manifest(&cwd) else {
        status_eprintln!("\x1b[1;31merror\x1b[0m: no runa.toml found in this project");
        eprintln!("  Run 'runa init <name>' to create a project first");
        std::process::exit(1);
    };
    manifest::parse_manifest(&path).unwrap_or_else(|error| fail(error))
}

/// Git sources are URLs (https://, ssh://, git@host:path) or local
/// repository paths ending in `.git`; anything else is a directory.
fn is_git_source(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
        || source.trim_end_matches('/').ends_with(".git")
}

fn repository_name(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next().unwrap_or(trimmed);
    last.trim_end_matches(".git").to_string()
}

/// `runa add <path-or-git-url> [--rev REV]`
pub(crate) fn runa_add(source: &str, rev: Option<&str>) {
    let manifest = project_manifest();
    let cwd = std::env::current_dir()
        .unwrap_or_else(|error| fail(format!("cannot read the current directory: {error}")));
    let dependency = if is_git_source(source) {
        let remote = source.starts_with("https://")
            || source.starts_with("ssh://")
            || source.starts_with("git@");
        let url = if remote {
            source.to_string()
        } else {
            let relative = super::pathdiff_relative(&cwd.join(source), manifest.directory());
            if relative.starts_with("../") || relative.starts_with('/') {
                relative
            } else {
                format!("./{relative}")
            }
        };
        manifest::validate_git_url(&url).unwrap_or_else(|error| fail(error));
        if let Some(rev) = rev {
            manifest::validate_git_rev(rev).unwrap_or_else(|error| fail(error));
        }
        Dependency {
            name: repository_name(&url),
            source: DependencySource::Git {
                url,
                rev: rev.map(str::to_string),
            },
        }
    } else {
        if rev.is_some() {
            fail("--rev applies only to git dependencies");
        }
        let directory = cwd.join(source);
        if !directory.is_dir() {
            fail(format!("path '{source}' is not a directory"));
        }
        let name = std::fs::canonicalize(&directory)
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| fail(format!("cannot name dependency at '{source}'")));
        Dependency {
            name,
            source: DependencySource::Path(super::pathdiff_relative(
                &directory,
                manifest.directory(),
            )),
        }
    };
    manifest::validate_dependency_name(&dependency.name).unwrap_or_else(|error| fail(error));
    if manifest.dependency(&dependency.name).is_some() {
        fail(format!(
            "dependency '{}' already exists in runa.toml",
            dependency.name
        ));
    }

    let lockfile = manifest::read_lockfile(&manifest).unwrap_or_else(|error| fail(error));
    let added = resolve(&manifest, &dependency, None);

    let content = std::fs::read_to_string(&manifest.path).unwrap_or_else(|error| {
        fail(format!(
            "cannot read {}: {}",
            manifest.path.display(),
            error
        ))
    });
    let updated = append_dependency(&content, &manifest::dependency_line(&dependency));
    std::fs::write(&manifest.path, updated).unwrap_or_else(|error| {
        fail(format!(
            "cannot write {}: {}",
            manifest.path.display(),
            error
        ))
    });

    let mut entries: Vec<LockedDependency> = manifest
        .dependencies
        .iter()
        .filter_map(|existing| {
            lockfile
                .entry(&existing.name)
                .filter(|entry| entry.source == existing.source)
                .cloned()
        })
        .collect();
    let description = match &added.commit {
        Some(commit) => format!("{} (commit {})", source, &commit[..12]),
        None => source.to_string(),
    };
    entries.push(added);
    write_lockfile(&manifest, entries);

    status_eprintln!(
        "\x1b[1;32mAdded\x1b[0m dependency '{}' → {}",
        dependency.name,
        description
    );
    eprintln!(
        "  Import its modules with `@ import {}/<module>` (or `@ import {}` for lib.runa)",
        dependency.name, dependency.name
    );
}

/// `runa fetch [--update]`: make every dependency available locally. Locked
/// Git commits are reused; `--update` re-resolves each Git `rev` (or the
/// default branch) and rewrites runa.lock.
pub(crate) fn runa_fetch(update: bool) {
    let manifest = project_manifest();
    let lockfile = manifest::read_lockfile(&manifest).unwrap_or_else(|error| fail(error));
    let entries: Vec<LockedDependency> = manifest
        .dependencies
        .iter()
        .map(|dependency| {
            let locked = (!update)
                .then(|| lockfile.locked_commit(dependency))
                .flatten();
            resolve(&manifest, dependency, locked)
        })
        .collect();
    let count = entries.len();
    write_lockfile(&manifest, entries);
    status_eprintln!(
        "\x1b[1;32mFetched\x1b[0m {} dependenc{}",
        count,
        if count == 1 { "y" } else { "ies" }
    );
}

fn write_lockfile(manifest: &Manifest, dependencies: Vec<LockedDependency>) {
    let path = manifest.lock_path();
    std::fs::write(&path, Lockfile { dependencies }.render())
        .unwrap_or_else(|error| fail(format!("cannot write {}: {}", path.display(), error)));
    eprintln!("  wrote {}", path.display());
}

fn resolve(manifest: &Manifest, dependency: &Dependency, locked: Option<&str>) -> LockedDependency {
    let commit = match &dependency.source {
        DependencySource::Path(_) => {
            manifest::dependency_root(manifest, dependency).unwrap_or_else(|error| fail(error));
            None
        }
        DependencySource::Git { url, rev } => {
            let location = manifest::git_fetch_location(url, manifest.directory());
            let store = manifest::git_source_store(&location)
                .unwrap_or_else(|| fail("cannot locate the dependency cache: HOME is not set"));
            let commit = match locked {
                Some(commit) if store.join(commit).is_dir() => commit.to_string(),
                _ => {
                    let target = locked.or(rev.as_deref()).unwrap_or("HEAD");
                    fetch_checkout(&location, target, &store, locked).unwrap_or_else(|error| {
                        fail(format!("dependency '{}': {}", dependency.name, error))
                    })
                }
            };
            Some(commit)
        }
    };
    LockedDependency {
        name: dependency.name.clone(),
        source: dependency.source.clone(),
        commit,
    }
}

fn git(directory: Option<&Path>) -> Command {
    let mut command = Command::new("git");
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    command.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "protocol.ext.allow=never",
    ]);
    command
}

fn run_git(command: &mut Command, action: &str) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run git to {action}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "git could not {action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Shallow-fetch `target` (a branch, tag, commit or HEAD) from `location`
/// into `store/<commit>` and return the commit. Arguments that come from
/// runa.toml follow `--`, and were validated by the manifest parser.
fn fetch_checkout(
    location: &str,
    target: &str,
    store: &Path,
    expected: Option<&str>,
) -> Result<String, String> {
    std::fs::create_dir_all(store)
        .map_err(|error| format!("cannot create {}: {}", store.display(), error))?;
    let staging = store.join(format!(
        ".fetch-{}-{}",
        std::process::id(),
        FETCH_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&staging);
    status_eprintln!("\x1b[1;36mFetching\x1b[0m {} ({})", location, target);
    let result = (|| {
        run_git(
            git(None).args(["init", "--quiet"]).arg(&staging),
            "create a checkout",
        )?;
        run_git(
            git(Some(&staging))
                .args(["fetch", "--quiet", "--depth", "1", "--no-tags", "--"])
                .arg(location)
                .arg(target),
            &format!("fetch `{target}` from {location}"),
        )?;
        run_git(
            git(Some(&staging)).args(["checkout", "--quiet", "--detach", "FETCH_HEAD"]),
            "check out the fetched commit",
        )?;
        let commit = run_git(
            git(Some(&staging)).args(["rev-parse", "HEAD"]),
            "read the fetched commit",
        )?;
        if !manifest::is_commit_hash(&commit) {
            return Err(format!("unexpected commit id `{commit}`"));
        }
        if let Some(expected) = expected {
            if commit != expected {
                return Err(format!(
                    "fetched commit {commit} differs from the locked commit {expected}"
                ));
            }
        }
        let checkout = store.join(&commit);
        if !checkout.is_dir() {
            std::fs::rename(&staging, &checkout).map_err(|error| {
                format!("cannot store checkout at {}: {}", checkout.display(), error)
            })?;
        }
        Ok(commit)
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Append a line to the [dependencies] section of runa.toml.
fn append_dependency(content: &str, line: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let Some(section) = lines
        .iter()
        .position(|line| line.trim() == "[dependencies]")
    else {
        let mut result = content.trim_end().to_string();
        result.push_str("\n\n[dependencies]\n");
        result.push_str(line);
        result.push('\n');
        return result;
    };
    // Insert after the last non-blank line of the section.
    let section_end = lines[section + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .map_or(lines.len(), |offset| section + 1 + offset);
    let insert_at = lines[section + 1..section_end]
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(section + 1, |offset| section + 2 + offset);
    let mut result = String::with_capacity(content.len() + line.len() + 1);
    for (index, existing) in lines.iter().enumerate() {
        if index == insert_at {
            result.push_str(line);
            result.push('\n');
        }
        result.push_str(existing);
        result.push('\n');
    }
    if insert_at == lines.len() {
        result.push_str(line);
        result.push('\n');
    }
    result
}
