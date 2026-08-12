use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn chapter_paths(root: &Path) -> Vec<PathBuf> {
    (1..=11)
        .map(|chapter| root.join(format!("kapitel-{chapter:02}.runa")))
        .collect()
}

fn fnv1a_update(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[test]
fn constitution_source_blocks_have_canonical_typed_spans() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/danish-constitution");
    let expected_labels = (1..=89)
        .map(|paragraph| format!("grundlov_par{paragraph}"))
        .chain([
            "grundlov_indledning".to_string(),
            "grundlov_stadfaestelse".to_string(),
        ])
        .collect::<BTreeSet<_>>();
    let mut label_counts = BTreeMap::<String, usize>::new();
    let mut source_block_count = 0;

    for path in chapter_paths(&root) {
        let source = fs::read_to_string(&path).expect("read Constitution chapter");
        let lines = source.lines().collect::<Vec<_>>();
        let delimiter_count = lines.iter().filter(|line| **line == "----").count();
        assert_eq!(
            delimiter_count % 2,
            0,
            "{} contains an unmatched legal-source delimiter",
            path.display()
        );
        let expected_file_blocks = delimiter_count / 2;
        let mut labeled_file_blocks = 0;
        let mut index = 0;

        while index < lines.len() {
            let Some(anchor) = lines[index].strip_prefix("--@label:") else {
                index += 1;
                continue;
            };
            let (label, binding) = anchor
                .strip_suffix("--")
                .and_then(|body| body.split_once("::meta:"))
                .expect("canonical typed Constitution metadata anchor");
            assert_eq!(
                binding,
                "grundlov_kildemetadata",
                "{} uses an unexpected source binding for {label}",
                path.display()
            );
            assert_eq!(
                lines.get(index + 1),
                Some(&"----"),
                "{} must place the legal source block directly after {label}",
                path.display()
            );

            let source_end = lines[index + 2..]
                .iter()
                .position(|line| *line == "----")
                .map(|offset| index + 2 + offset)
                .expect("closed Constitution source block");
            let begin = format!("--@begin:{label}--");
            let end = format!("--@end:{label}--");
            assert_eq!(
                lines.get(source_end + 2),
                Some(&begin.as_str()),
                "{} must begin the matching implementation span after {label}",
                path.display()
            );
            assert_eq!(
                lines.iter().filter(|line| **line == end).count(),
                1,
                "{} must contain exactly one end marker for {label}",
                path.display()
            );

            source_block_count += 1;
            labeled_file_blocks += 1;
            *label_counts.entry(label.to_string()).or_default() += 1;
            index = source_end + 1;
        }

        assert_eq!(
            labeled_file_blocks,
            expected_file_blocks,
            "{} contains an unanchored legal-source block",
            path.display()
        );
    }

    assert_eq!(source_block_count, 91);
    assert_eq!(
        label_counts.keys().cloned().collect::<BTreeSet<_>>(),
        expected_labels
    );
    assert!(
        label_counts.values().all(|count| *count == 1),
        "every Constitution source label must be unique: {label_counts:?}"
    );
}

#[test]
fn restored_official_constitution_text_remains_present() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/danish-constitution");
    let chapter_03 =
        fs::read_to_string(root.join("kapitel-03.runa")).expect("read Constitution chapter III");
    let chapter_11 =
        fs::read_to_string(root.join("kapitel-11.runa")).expect("read Constitution chapter XI");

    assert!(chapter_03.contains(
        "Fungerende ministre kan i deres embede kun foretage sig, hvad der er\n\
fornødent til embedsforretningernes uforstyrrede førelse."
    ));
    assert!(chapter_03.contains("eller indgå nogen forpligtelse,\ntil hvis opfyldelse"));
    for signatory in [
        "Helga Pedersen.",
        "Knud Ree.",
        "Aage L. Rytter.",
        "Jørgen Jørgensen.",
    ] {
        assert!(
            chapter_11.contains(signatory),
            "missing signatory {signatory}"
        );
    }
}

#[test]
fn constitution_legal_text_matches_verified_source_fingerprint() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/danish-constitution");
    let mut hash = 0xcbf29ce484222325;

    for path in chapter_paths(&root) {
        let source = fs::read_to_string(&path).expect("read Constitution chapter");
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            let Some(anchor) = line.strip_prefix("--@label:") else {
                continue;
            };
            let label = anchor
                .strip_suffix("--")
                .and_then(|body| body.split_once("::meta:"))
                .map(|(label, _)| label)
                .expect("canonical Constitution metadata anchor");
            let source_end = lines[index + 2..]
                .iter()
                .position(|line| *line == "----")
                .map(|offset| index + 2 + offset)
                .expect("closed Constitution source block");
            let normalized = lines[index + 2..source_end]
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            fnv1a_update(&mut hash, label);
            fnv1a_update(&mut hash, "\0");
            fnv1a_update(&mut hash, &normalized);
            fnv1a_update(&mut hash, "\u{00ff}");
        }
    }

    assert_eq!(hash, 0x5c1bbaf1142e3696);
}

#[test]
fn constitution_uses_shared_typed_source_protocol() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/danish-constitution");
    let protocol = fs::read_to_string(root.join("grundlov-faelles.runa"))
        .expect("read Constitution source protocol");

    assert!(protocol.contains("# impl MetaRole for GrundlovMetaRolle {}"));
    assert!(protocol.contains("# impl Meta for GrundlovMetadata {}"));
    assert!(protocol.contains("https://www.retsinformation.dk/eli/lta/1953/169"));

    for path in chapter_paths(&root) {
        let source = fs::read_to_string(&path).expect("read Constitution chapter");
        assert!(
            source.contains("@ importer ./grundlov - faelles"),
            "{} must import the shared domain and source protocol",
            path.display()
        );
    }
}
