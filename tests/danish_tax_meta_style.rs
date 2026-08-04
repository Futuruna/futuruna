use std::fs;
use std::path::{Path, PathBuf};

fn collect_runa_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read example directory") {
        let path = entry.expect("read example entry").path();
        if path.is_dir() {
            collect_runa_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("runa") {
            files.push(path);
        }
    }
}

#[test]
fn example_metadata_uses_thin_typed_anchors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut files = Vec::new();
    collect_runa_files(&root, &mut files);
    let mut violations = Vec::new();

    for path in files {
        let source = fs::read_to_string(&path).expect("read Futuruna source");
        for (index, line) in source.lines().enumerate() {
            let is_metadata_anchor = line.starts_with("--@label:")
                || line.starts_with("--@meta:")
                || line.starts_with("--@source:");
            if !is_metadata_anchor {
                continue;
            }
            let canonical = line
                .strip_prefix("--@label:")
                .and_then(|body| body.strip_suffix("--"))
                .and_then(|body| body.split_once("::meta:"))
                .is_some_and(|(label, binding)| {
                    !label.is_empty()
                        && !binding.is_empty()
                        && !label.contains("::")
                        && !binding.contains("::")
                        && !binding.contains(':')
                });

            if !canonical || line.chars().count() > 160 {
                violations.push(format!("{}:{}: {line}", path.display(), index + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Example metadata comments must be one thin --@label:LABEL::meta:BINDING-- anchor:\n{}",
        violations.join("\n")
    );
}
