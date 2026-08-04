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

#[test]
fn danish_tax_metadata_uses_typed_classes_instead_of_legacy_role_objects() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/danish-income-tax");
    let mut files = Vec::new();
    collect_runa_files(&root, &mut files);
    let mut violations = Vec::new();

    for path in files {
        let source = fs::read_to_string(&path).expect("read Futuruna source");
        if source.contains("MetaAttachment") || source.contains("DanskSkatMetaRolle") {
            violations.push(path.display().to_string());
        }
    }

    assert!(
        violations.is_empty(),
        "Danish tax metadata must use Meta/MetaRole classes, not legacy role objects:\n{}",
        violations.join("\n")
    );

    let protocol =
        fs::read_to_string(root.join("metadata.runa")).expect("read Danish tax metadata protocol");
    assert!(protocol.contains("# impl MetaRole for DanskSkatMeta {}"));
    assert!(protocol.contains("# impl Meta for DanskSkatMeta {}"));
    assert!(protocol.contains("# impl Meta for Metadata {}"));
    assert!(protocol.contains("# impl Meta for Beregningsmeta {}"));
}

#[test]
fn personskat_calculation_uses_a_partitioned_typed_metadata_root() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/danish-income-tax/personskat.calculate.runa");
    let source = fs::read_to_string(path).expect("read Personskat calculation");
    let anchors = source
        .lines()
        .filter(|line| line.starts_with("--@label:beregn_personskat::meta:"))
        .collect::<Vec<_>>();

    assert_eq!(
        anchors,
        vec!["--@label:beregn_personskat::meta:personskat_beregningsmetadata--"]
    );
    assert!(source.contains("# impl Meta for PersonskatBeregningsmetadata {}"));
    assert!(source.contains("= personskat_beregningsmetadata = PersonskatBeregningsmetadata("));
    for field in [
        "kilder =",
        "pension =",
        "erhvervsbefordring =",
        "grundforhold =",
        "kapitalindkomst =",
        "ejendomsavance =",
        "kursgevinst =",
        "aktieavance =",
        "årsopgørelse =",
    ] {
        assert!(
            source.contains(field),
            "missing metadata partition `{field}`"
        );
    }
    assert!(!source.contains("--@begin:personskat_lønmodtagerberegning--"));
    assert!(!source.contains("--@end:personskat_lønmodtagerberegning--"));
}
