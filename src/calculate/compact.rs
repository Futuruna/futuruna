//! Explicit, lossless compact wire representation of calculation schemas.
use super::{
    CalculationContract, CalculationFieldMetadata, CalculationMetadataReference,
    CalculationTypeDefinition, CalculationTypeRef, CONTRACT_SCHEMA,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use super::parse_calculation_json;

/// Read the explicit compact wire format without dropping unknown fields or
/// overwriting duplicate keys. Object key order and whitespace are immaterial;
/// omitted/empty optionals must follow this version's canonical writer shape.
pub fn expand_compact_json(
    source: &str,
    max_expanded_source_bytes: usize,
) -> Result<CalculationContract, String> {
    let raw = parse_calculation_json(source).map_err(|error| error.to_string())?;
    let compact: CompactCalculationContract =
        serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    if serde_json::to_value(&compact).expect("compact schema serializes") != raw {
        return Err("compact schema has unknown fields or noncanonical optional fields".into());
    }
    compact.expand(max_expanded_source_bytes)
}

pub const COMPACT_SCHEMA: &str = "futuruna.calculate.compact.v1";
const SOURCE_DOMAIN: &[u8] = b"futuruna.calculate.compact.v1/source\0";
const GROUP_DOMAIN: &[u8] = b"futuruna.calculate.compact.v1/source-group\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactFieldMetadata {
    pub path: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub anchor: String,
    pub binding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactCalculationContract {
    pub schema: String,
    pub schema_version: u32,
    pub contract_schema: String,
    pub contract_schema_version: u32,
    pub entry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub parameter: String,
    pub input: CalculationTypeRef,
    pub output: CalculationTypeRef,
    pub definitions: Vec<CalculationTypeDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<CalculationMetadataReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_metadata: Vec<CompactFieldMetadata>,
    pub source_objects: BTreeMap<String, CalculationMetadataReference>,
    pub source_groups: BTreeMap<String, Vec<String>>,
    pub schema_hash: String,
}

fn content_id<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(value).expect("schema metadata serializes"));
    format!("{:x}", digest.finalize())
}

impl CompactCalculationContract {
    pub fn from_contract(contract: &CalculationContract) -> Self {
        let mut source_objects = BTreeMap::new();
        let mut source_groups = BTreeMap::new();
        let field_metadata = contract
            .field_metadata
            .iter()
            .map(|field| {
                let source_group = if field.sources.is_empty() {
                    None
                } else {
                    let ids: Vec<_> = field
                        .sources
                        .iter()
                        .map(|source| {
                            let id = content_id(SOURCE_DOMAIN, source);
                            let stored = source_objects
                                .entry(id.clone())
                                .or_insert_with(|| source.clone());
                            assert_eq!(stored, source, "schema source content-ID collision");
                            id
                        })
                        .collect();
                    let id = content_id(GROUP_DOMAIN, &ids);
                    let stored = source_groups
                        .entry(id.clone())
                        .or_insert_with(|| ids.clone());
                    assert_eq!(stored, &ids, "schema source-group content-ID collision");
                    Some(id)
                };
                CompactFieldMetadata {
                    path: field.path.clone(),
                    label: field.label.clone(),
                    question: field.question.clone(),
                    help: field.help.clone(),
                    unit: field.unit.clone(),
                    anchor: field.anchor.clone(),
                    binding: field.binding.clone(),
                    source_group,
                }
            })
            .collect();
        Self {
            schema: COMPACT_SCHEMA.into(),
            schema_version: 1,
            contract_schema: contract.schema.clone(),
            contract_schema_version: contract.schema_version,
            entry: contract.entry.clone(),
            label: contract.label.clone(),
            parameter: contract.parameter.clone(),
            input: contract.input.clone(),
            output: contract.output.clone(),
            definitions: contract.definitions.clone(),
            metadata: contract.metadata.clone(),
            field_metadata,
            source_objects,
            source_groups,
            schema_hash: contract.schema_hash.clone(),
        }
    }

    /// Expand only after checking the complete reference graph and its budget.
    /// The budget counts serialized source-object bytes repeated at field paths;
    /// it does not bound parsing, existing metadata, or total process memory.
    pub fn expand(self, max_expanded_source_bytes: usize) -> Result<CalculationContract, String> {
        if self.schema != COMPACT_SCHEMA || self.schema_version != 1 {
            return Err("unsupported compact calculation schema".into());
        }
        if self.contract_schema != CONTRACT_SCHEMA || self.contract_schema_version != 1 {
            return Err("unsupported logical calculation schema".into());
        }
        let mut source_bytes = BTreeMap::new();
        for (id, source) in &self.source_objects {
            if *id != content_id(SOURCE_DOMAIN, source) {
                return Err(format!("source object content-ID mismatch: {id}"));
            }
            source_bytes.insert(id, serde_json::to_vec(source).unwrap().len());
        }
        let mut group_bytes = BTreeMap::new();
        let mut used_sources = BTreeSet::new();
        for (id, sources) in &self.source_groups {
            if sources.is_empty() {
                return Err(format!("noncanonical empty source group: {id}"));
            }
            if *id != content_id(GROUP_DOMAIN, sources) {
                return Err(format!("source group content-ID mismatch: {id}"));
            }
            let mut bytes = 0usize;
            for source in sources {
                let size = source_bytes
                    .get(source)
                    .ok_or_else(|| format!("dangling source object reference: {source}"))?;
                bytes = bytes.checked_add(*size).ok_or("source size overflow")?;
                used_sources.insert(source);
            }
            group_bytes.insert(id, bytes);
        }
        if used_sources.len() != self.source_objects.len() {
            return Err("unreferenced source object in compact calculation schema".into());
        }
        let mut used_groups = BTreeSet::new();
        let mut expanded_bytes = 0usize;
        for field in &self.field_metadata {
            if let Some(id) = &field.source_group {
                let bytes = group_bytes
                    .get(id)
                    .ok_or_else(|| format!("dangling source group reference at {}", field.path))?;
                expanded_bytes = expanded_bytes
                    .checked_add(*bytes)
                    .ok_or("expanded source size overflow")?;
                if expanded_bytes > max_expanded_source_bytes {
                    return Err("expanded field sources exceed the caller's byte budget".into());
                }
                used_groups.insert(id);
            }
        }
        if used_groups.len() != self.source_groups.len() {
            return Err("unreferenced source group in compact calculation schema".into());
        }

        let fields = self
            .field_metadata
            .into_iter()
            .map(|field| {
                let sources = field.source_group.map_or_else(Vec::new, |id| {
                    self.source_groups[&id]
                        .iter()
                        .map(|id| self.source_objects[id].clone())
                        .collect()
                });
                CalculationFieldMetadata {
                    path: field.path,
                    label: field.label,
                    question: field.question,
                    help: field.help,
                    unit: field.unit,
                    anchor: field.anchor,
                    binding: field.binding,
                    sources,
                }
            })
            .collect();
        let expected_hash = self.schema_hash;
        let contract = CalculationContract {
            schema: self.contract_schema,
            schema_version: self.contract_schema_version,
            entry: self.entry,
            label: self.label,
            parameter: self.parameter,
            input: self.input,
            output: self.output,
            definitions: self.definitions,
            metadata: self.metadata,
            field_metadata: fields,
            schema_hash: String::new(),
        }
        .finish_hash();
        if contract.schema_hash != expected_hash {
            return Err("logical calculation schema fingerprint mismatch".into());
        }
        Ok(contract)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> CalculationContract {
        let source = CalculationMetadataReference {
            label: "law".into(),
            role: "source".into(),
            binding: "source".into(),
            attachment_path: Some("root.attachments[0]".into()),
            qualified_type: Some("SourceInfo".into()),
            value: Some("complete value".into()),
            data: Some(json!({"url":"https://example.invalid", "section": 3})),
            text: "Original source text æøå".into(),
            symbols: vec!["input".into()],
        };
        let mut alternate = source.clone();
        alternate.attachment_path = Some("other.attachments[0]".into());
        let field = |path: &str, sources| CalculationFieldMetadata {
            path: path.into(),
            label: path.into(),
            question: Some("Question?".into()),
            help: Some("Help".into()),
            unit: Some("DKK".into()),
            anchor: "input".into(),
            binding: "field".into(),
            sources,
        };
        CalculationContract {
            schema: CONTRACT_SCHEMA.into(),
            schema_version: 1,
            entry: "calculate".into(),
            label: Some("Synthetic calculation".into()),
            parameter: "input".into(),
            input: CalculationTypeRef::Primitive { name: "Int".into() },
            output: CalculationTypeRef::Primitive { name: "Int".into() },
            definitions: vec![],
            metadata: vec![source.clone()],
            field_metadata: vec![
                field("a", vec![source.clone(), alternate.clone(), source.clone()]),
                field("b", vec![source.clone(), alternate.clone(), source.clone()]),
                field("c", vec![]),
                field("d", vec![alternate, source]),
            ],
            schema_hash: String::new(),
        }
        .finish_hash()
    }

    #[test]
    fn lossless_deterministic_and_ordered() {
        let original = fixture();
        let compact = CompactCalculationContract::from_contract(&original);
        assert_eq!(compact.source_objects.len(), 2);
        assert_eq!(compact.source_groups.len(), 2);
        assert_eq!(
            compact.field_metadata[0].source_group,
            compact.field_metadata[1].source_group
        );
        assert_ne!(
            compact.field_metadata[0].source_group,
            compact.field_metadata[3].source_group
        );
        assert!(compact.field_metadata[2].source_group.is_none());
        let encoded = serde_json::to_vec(&compact).unwrap();
        assert_eq!(
            encoded,
            serde_json::to_vec(&CompactCalculationContract::from_contract(&original)).unwrap()
        );
        let decoded: CompactCalculationContract = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.expand(100_000).unwrap(), original);
    }

    #[test]
    fn rejects_corrupt_source_group_and_logical_hash() {
        let compact = CompactCalculationContract::from_contract(&fixture());
        let mut changed = compact.clone();
        changed
            .source_objects
            .values_mut()
            .next()
            .unwrap()
            .text
            .push('!');
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("object content-ID"));
        let mut changed = compact.clone();
        changed.source_groups.values_mut().next().unwrap().reverse();
        // One group is palindromic, so explicitly mutate its length as well.
        changed.source_groups.values_mut().next().unwrap().pop();
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("group content-ID"));
        let mut changed = compact.clone();
        changed.field_metadata[0].label.push('!');
        assert!(changed.expand(100_000).unwrap_err().contains("fingerprint"));
        let mut changed = compact;
        changed.metadata[0].text.push('!');
        assert!(changed.expand(100_000).unwrap_err().contains("fingerprint"));
    }

    #[test]
    fn rejects_dangling_or_unreferenced_sources_and_budget_overrun() {
        let compact = CompactCalculationContract::from_contract(&fixture());
        let mut changed = compact.clone();
        changed.source_objects.pop_first();
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("dangling source object"));
        let mut changed = compact.clone();
        changed.field_metadata[0].source_group = Some("missing".into());
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("dangling source group"));
        let mut changed = compact.clone();
        changed.field_metadata.clear();
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("unreferenced source group"));
        let mut changed = compact.clone();
        changed.source_groups.clear();
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("unreferenced source object"));
        assert!(compact.expand(0).unwrap_err().contains("byte budget"));
    }

    #[test]
    fn rejects_versions_and_preserves_empty_contract_metadata() {
        let original = fixture();
        let compact = CompactCalculationContract::from_contract(&original);
        let mut changed = compact.clone();
        changed.schema_version += 1;
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("unsupported compact"));
        let mut changed = compact;
        changed.contract_schema_version += 1;
        assert!(changed
            .expand(100_000)
            .unwrap_err()
            .contains("unsupported logical"));
        let mut empty = original;
        empty.field_metadata.clear();
        empty.metadata.clear();
        let empty = empty.finish_hash();
        let compact = CompactCalculationContract::from_contract(&empty);
        assert_eq!(compact.expand(0).unwrap(), empty);
    }

    #[test]
    fn wire_reader_rejects_duplicate_keys_in_pools_and_nested_provenance() {
        let original = fixture();
        let compact = CompactCalculationContract::from_contract(&original);
        let document = serde_json::to_string_pretty(&compact).unwrap();
        assert_eq!(expand_compact_json(&document, 100_000).unwrap(), original);
        for document in [
            document.replacen("\"section\": 3", "\"section\": 3, \"section\": 4", 1),
            document.replacen("\"section\": 3", "\"section\": 3, \"\\u0073ection\": 4", 1),
        ] {
            assert!(expand_compact_json(&document, 100_000)
                .unwrap_err()
                .contains("duplicate JSON object member"));
        }
        let id = compact.source_objects.keys().next().unwrap();
        let entry = serde_json::to_string(&compact.source_objects[id]).unwrap();
        let duplicate = format!("\"source_objects\":{{\"{id}\":{entry},\"{id}\":{entry}}}");
        let mut raw = serde_json::to_value(&compact).unwrap();
        raw["source_objects"] = serde_json::json!({});
        let document = serde_json::to_string(&raw)
            .unwrap()
            .replace("\"source_objects\":{}", &duplicate);
        assert!(expand_compact_json(&document, 100_000)
            .unwrap_err()
            .contains("duplicate JSON object member"));
    }

    #[test]
    fn wire_reader_rejects_silently_ignored_fields_and_moved_source_groups() {
        let compact = CompactCalculationContract::from_contract(&fixture());
        let mut raw = serde_json::to_value(&compact).unwrap();
        raw["metadata"][0]["unrecognized_evidence"] = serde_json::json!("do not silently discard");
        assert!(expand_compact_json(&raw.to_string(), 100_000)
            .unwrap_err()
            .contains("unknown fields"));
        let mut changed = compact;
        changed.field_metadata[0].source_group = changed.field_metadata[3].source_group.clone();
        assert!(
            expand_compact_json(&serde_json::to_string(&changed).unwrap(), 100_000)
                .unwrap_err()
                .contains("fingerprint")
        );
    }
}
