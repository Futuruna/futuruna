use std::fs;
use std::path::PathBuf;

#[test]
fn danish_tax_metadata_uses_thin_typed_anchors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("danish-income-tax");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&root).expect("read Danish tax corpus") {
        let path = entry.expect("read corpus entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("runa") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read Futuruna source");
        for (index, line) in source.lines().enumerate() {
            let is_metadata_anchor = line.starts_with("--@label:")
                || line.starts_with("--@meta:")
                || line.starts_with("--@source:");
            if !is_metadata_anchor {
                continue;
            }

            let separator_count = line.matches("::").count();
            let direct_pair_count =
                if line.starts_with("--@meta::") || line.starts_with("--@source::") {
                    separator_count.saturating_sub(1)
                } else {
                    separator_count
                };

            if line.chars().count() > 160 || direct_pair_count > 1 {
                violations.push(format!("{}:{}: {line}", path.display(), index + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Danish-tax metadata comments must remain thin typed anchors:\n{}",
        violations.join("\n")
    );
}
