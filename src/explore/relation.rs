//! Canonical content identities and set semantics for relational Explore cases.
//!
//! This module is deliberately independent of the current Cartesian executor,
//! output projection, probe plan, and durable rank encoding. It is the common
//! identity boundary for a finite source relation and each source row's finite
//! successor relation.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{transition::canonical_explore_value_digest, ExploreValue};

const RELATION_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-id.v1";
const SOURCE_KEY_HASH_V1: &[u8] = b"futuruna.explore.source-key.v1";
const SUCCESSOR_KEY_HASH_V1: &[u8] = b"futuruna.explore.successor-key.v1";
const RELATIONAL_CASE_ID_HASH_V1: &[u8] = b"futuruna.explore.relational-case-id.v1";
const RELATION_LINEAGE_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-lineage-id.v1";
const RELATION_SUPPORT_ID_HASH_V1: &[u8] = b"futuruna.explore.relation-support-id.v1";

const RELATION_SEMANTIC_DIGEST_ROLE: u8 = 0x01;
const RELATION_ROLE: u8 = 0x01;
const SOURCE_CONTEXT_ROLE: u8 = 0x02;
const SOURCE_BEFORE_ROLE: u8 = 0x03;
const SOURCE_ROLE: u8 = 0x02;
const SUCCESSOR_AFTER_ROLE: u8 = 0x03;
const SUCCESSOR_ROLE: u8 = 0x03;

/// Identity of one normalized source/successor relation contract.
///
/// Its canonical semantic digest is supplied by the checked relational IR. In
/// particular, presentation views, probe/scheduling choices, and run-local
/// limits are not inputs to this identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationId([u8; 32]);

impl RelationId {
    /// Derive an identity directly from the canonical semantic relation
    /// preimage. This is equivalent to first taking its SHA-256 digest and
    /// calling [`Self::from_canonical_semantic_digest`].
    pub(crate) fn from_canonical_semantic_preimage(preimage: &[u8]) -> Self {
        Self::from_canonical_semantic_digest(Sha256::digest(preimage).into())
    }

    /// Derive an identity from an already computed canonical semantic digest.
    pub(crate) fn from_canonical_semantic_digest(semantic_digest: [u8; 32]) -> Self {
        let mut hasher = IdentityHasher::new(RELATION_ID_HASH_V1);
        hasher.tag(RELATION_SEMANTIC_DIGEST_ROLE);
        hasher.digest(semantic_digest);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content-stable identity of one canonical `(Context, Before)` source row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceKey([u8; 32]);

impl SourceKey {
    fn derive(relation_id: RelationId, row: &SourceRow) -> Self {
        let mut hasher = IdentityHasher::new(SOURCE_KEY_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_CONTEXT_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.context));
        hasher.tag(SOURCE_BEFORE_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.before));
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content-stable identity of one After row in a particular source's
/// dependent successor relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SuccessorKey([u8; 32]);

impl SuccessorKey {
    fn derive(relation_id: RelationId, source_key: SourceKey, row: &SuccessorRow) -> Self {
        let mut hasher = IdentityHasher::new(SUCCESSOR_KEY_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_ROLE);
        hasher.digest(source_key.0);
        hasher.tag(SUCCESSOR_AFTER_ROLE);
        hasher.digest(canonical_explore_value_digest(&row.after));
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable case identity for one source/successor coordinate.
///
/// No ordinal, discovery sequence, worker, or scheduler decision contributes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseId([u8; 32]);

impl RelationalCaseId {
    fn derive(relation_id: RelationId, source_key: SourceKey, successor_key: SuccessorKey) -> Self {
        let mut hasher = IdentityHasher::new(RELATIONAL_CASE_ID_HASH_V1);
        hasher.tag(RELATION_ROLE);
        hasher.digest(relation_id.0);
        hasher.tag(SOURCE_ROLE);
        hasher.digest(source_key.0);
        hasher.tag(SUCCESSOR_ROLE);
        hasher.digest(successor_key.0);
        Self(hasher.finish())
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of one producer lineage claim retained behind a row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationLineageId([u8; 32]);

impl RelationLineageId {
    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_content_identity(
            RELATION_LINEAGE_ID_HASH_V1,
            preimage,
        ))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of one producer support coordinate retained behind a
/// deduplicated row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationSupportId([u8; 32]);

impl RelationSupportId {
    pub(crate) fn from_canonical_preimage(preimage: &[u8]) -> Self {
        Self(derive_content_identity(
            RELATION_SUPPORT_ID_HASH_V1,
            preimage,
        ))
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Exact provenance retained behind one canonical set member.
///
/// Provenance does not rename a row. Rediscovering equal content unions both
/// sets so duplicate generation cannot inflate source or case counts and does
/// not discard support.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RelationProvenance {
    lineage: BTreeSet<RelationLineageId>,
    support: BTreeSet<RelationSupportId>,
}

impl RelationProvenance {
    pub(crate) fn new(
        lineage: impl IntoIterator<Item = RelationLineageId>,
        support: impl IntoIterator<Item = RelationSupportId>,
    ) -> Self {
        Self {
            lineage: lineage.into_iter().collect(),
            support: support.into_iter().collect(),
        }
    }

    pub(crate) fn lineage(&self) -> &BTreeSet<RelationLineageId> {
        &self.lineage
    }

    pub(crate) fn support(&self) -> &BTreeSet<RelationSupportId> {
        &self.support
    }

    fn union(&mut self, mut other: Self) {
        self.lineage.append(&mut other.lineage);
        self.support.append(&mut other.support);
    }
}

/// Canonical source-relation row. Context deliberately includes the action or
/// intervention identity whenever two otherwise equal endpoint pairs are
/// materially different cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRow {
    context: ExploreValue,
    before: ExploreValue,
    provenance: RelationProvenance,
}

impl SourceRow {
    pub(crate) fn new(
        context: ExploreValue,
        before: ExploreValue,
        provenance: RelationProvenance,
    ) -> Self {
        Self {
            context,
            before,
            provenance,
        }
    }

    pub(crate) fn context(&self) -> &ExploreValue {
        &self.context
    }

    pub(crate) fn before(&self) -> &ExploreValue {
        &self.before
    }

    pub(crate) fn provenance(&self) -> &RelationProvenance {
        &self.provenance
    }

    fn has_same_content(&self, other: &Self) -> bool {
        self.context == other.context && self.before == other.before
    }

    fn union_provenance(&mut self, other: Self) {
        self.provenance.union(other.provenance);
    }
}

/// Canonical member of one source row's dependent successor relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SuccessorRow {
    after: ExploreValue,
    provenance: RelationProvenance,
}

impl SuccessorRow {
    pub(crate) fn new(after: ExploreValue, provenance: RelationProvenance) -> Self {
        Self { after, provenance }
    }

    pub(crate) fn after(&self) -> &ExploreValue {
        &self.after
    }

    pub(crate) fn provenance(&self) -> &RelationProvenance {
        &self.provenance
    }

    fn has_same_content(&self, other: &Self) -> bool {
        self.after == other.after
    }

    fn union_provenance(&mut self, other: Self) {
        self.provenance.union(other.provenance);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDraft {
    row: SourceRow,
    successors: BTreeMap<SuccessorKey, SuccessorDraft>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SuccessorDraft {
    row: SuccessorRow,
    case_id: RelationalCaseId,
}

/// Incremental, collision-checking set builder for one relational exploration
/// universe.
#[derive(Clone, Debug)]
pub(crate) struct RelationCatalogBuilder {
    relation_id: RelationId,
    sources: BTreeMap<SourceKey, SourceDraft>,
    successor_claims: BTreeMap<SuccessorKey, (SourceKey, ExploreValue)>,
    case_claims: BTreeMap<RelationalCaseId, (SourceKey, SuccessorKey)>,
}

impl RelationCatalogBuilder {
    pub(crate) fn new(relation_id: RelationId) -> Self {
        Self {
            relation_id,
            sources: BTreeMap::new(),
            successor_claims: BTreeMap::new(),
            case_claims: BTreeMap::new(),
        }
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    /// Insert or merge one canonical source member.
    pub(crate) fn insert_source(
        &mut self,
        row: SourceRow,
    ) -> Result<SourceKey, RelationCatalogError> {
        let source_key = SourceKey::derive(self.relation_id, &row);
        match self.sources.get_mut(&source_key) {
            Some(existing) if existing.row.has_same_content(&row) => {
                existing.row.union_provenance(row);
            }
            Some(_) => {
                return Err(RelationCatalogError::SourceKeyCollision { source_key });
            }
            None => {
                self.sources.insert(
                    source_key,
                    SourceDraft {
                        row,
                        successors: BTreeMap::new(),
                    },
                );
            }
        }
        Ok(source_key)
    }

    /// Insert or merge one member of a source row's dependent successor set.
    ///
    /// All collision checks happen before mutation, so a rejected insertion is
    /// atomic with respect to the catalog under construction.
    pub(crate) fn insert_successor(
        &mut self,
        source_key: SourceKey,
        row: SuccessorRow,
    ) -> Result<(SuccessorKey, RelationalCaseId), RelationCatalogError> {
        let source = self
            .sources
            .get(&source_key)
            .ok_or(RelationCatalogError::UnknownSource { source_key })?;
        let successor_key = SuccessorKey::derive(self.relation_id, source_key, &row);
        let case_id = RelationalCaseId::derive(self.relation_id, source_key, successor_key);

        if let Some((claimed_source, claimed_after)) = self.successor_claims.get(&successor_key) {
            if *claimed_source != source_key || claimed_after != &row.after {
                return Err(RelationCatalogError::SuccessorKeyCollision { successor_key });
            }
        }
        if let Some((claimed_source, claimed_successor)) = self.case_claims.get(&case_id) {
            if *claimed_source != source_key || *claimed_successor != successor_key {
                return Err(RelationCatalogError::CaseIdCollision { case_id });
            }
        }
        if let Some(existing) = source.successors.get(&successor_key) {
            if !existing.row.has_same_content(&row) {
                return Err(RelationCatalogError::SuccessorKeyCollision { successor_key });
            }
        }

        self.successor_claims
            .entry(successor_key)
            .or_insert_with(|| (source_key, row.after.clone()));
        self.case_claims
            .entry(case_id)
            .or_insert((source_key, successor_key));
        let source = self
            .sources
            .get_mut(&source_key)
            .expect("source existence was checked before relational insertion");
        match source.successors.get_mut(&successor_key) {
            Some(existing) => existing.row.union_provenance(row),
            None => {
                source
                    .successors
                    .insert(successor_key, SuccessorDraft { row, case_id });
            }
        }

        Ok((successor_key, case_id))
    }

    /// Freeze the catalog into canonical value order, independently of
    /// discovery order.
    pub(crate) fn finish(self) -> RelationCatalog {
        let mut sources = self
            .sources
            .into_iter()
            .map(|(key, source)| {
                let mut successors = source
                    .successors
                    .into_iter()
                    .map(|(key, successor)| CatalogSuccessor {
                        key,
                        case_id: successor.case_id,
                        row: successor.row,
                    })
                    .collect::<Vec<_>>();
                successors.sort_by(|left, right| {
                    left.row
                        .after
                        .cmp(&right.row.after)
                        .then_with(|| left.key.cmp(&right.key))
                });
                CatalogSource {
                    key,
                    row: source.row,
                    successors: successors.into_boxed_slice(),
                }
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| {
            left.row
                .context
                .cmp(&right.row.context)
                .then_with(|| left.row.before.cmp(&right.row.before))
                .then_with(|| left.key.cmp(&right.key))
        });

        let mut source_index = BTreeMap::new();
        let mut case_index = BTreeMap::new();
        for (source_index_value, source) in sources.iter().enumerate() {
            source_index.insert(source.key, source_index_value);
            for (successor_index, successor) in source.successors.iter().enumerate() {
                case_index.insert(successor.case_id, (source_index_value, successor_index));
            }
        }

        RelationCatalog {
            relation_id: self.relation_id,
            sources: sources.into_boxed_slice(),
            source_index,
            case_index,
        }
    }
}

/// One canonical source row and its canonically ordered dependent successor
/// set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogSource {
    key: SourceKey,
    row: SourceRow,
    successors: Box<[CatalogSuccessor]>,
}

impl CatalogSource {
    pub(crate) const fn key(&self) -> SourceKey {
        self.key
    }

    pub(crate) fn row(&self) -> &SourceRow {
        &self.row
    }

    pub(crate) fn successors(&self) -> &[CatalogSuccessor] {
        &self.successors
    }
}

/// One canonical successor and the case identity formed with its owning
/// source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogSuccessor {
    key: SuccessorKey,
    case_id: RelationalCaseId,
    row: SuccessorRow,
}

impl CatalogSuccessor {
    pub(crate) const fn key(&self) -> SuccessorKey {
        self.key
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }

    pub(crate) fn row(&self) -> &SuccessorRow {
        &self.row
    }
}

/// Immutable, canonically ordered set of source rows and their successors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationCatalog {
    relation_id: RelationId,
    sources: Box<[CatalogSource]>,
    source_index: BTreeMap<SourceKey, usize>,
    case_index: BTreeMap<RelationalCaseId, (usize, usize)>,
}

impl RelationCatalog {
    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub(crate) fn case_count(&self) -> usize {
        self.case_index.len()
    }

    pub(crate) fn sources(&self) -> &[CatalogSource] {
        &self.sources
    }

    pub(crate) fn source(&self, source_key: SourceKey) -> Option<&CatalogSource> {
        self.source_index
            .get(&source_key)
            .and_then(|index| self.sources.get(*index))
    }

    pub(crate) fn case(&self, case_id: RelationalCaseId) -> Option<RelationalCaseRef<'_>> {
        let (source_index, successor_index) = *self.case_index.get(&case_id)?;
        let source = self.sources.get(source_index)?;
        let successor = source.successors.get(successor_index)?;
        Some(RelationalCaseRef {
            relation_id: self.relation_id,
            source,
            successor,
        })
    }

    /// Iterate in canonical `(Context, Before, After, identity)` value order.
    pub(crate) fn cases(&self) -> impl Iterator<Item = RelationalCaseRef<'_>> {
        self.sources.iter().flat_map(move |source| {
            source
                .successors
                .iter()
                .map(move |successor| RelationalCaseRef {
                    relation_id: self.relation_id,
                    source,
                    successor,
                })
        })
    }
}

/// Borrowed extensional transition frame for one stable relational CaseId.
///
/// The existing transition interner can independently consume `context`,
/// `before`, and `after`; neither its `TransitionId` nor its deduplication is
/// coupled to the relational CaseId.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalCaseRef<'a> {
    relation_id: RelationId,
    source: &'a CatalogSource,
    successor: &'a CatalogSuccessor,
}

impl<'a> RelationalCaseRef<'a> {
    pub(crate) const fn relation_id(self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source.key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor.key
    }

    pub(crate) const fn case_id(self) -> RelationalCaseId {
        self.successor.case_id
    }

    pub(crate) fn context(self) -> &'a ExploreValue {
        &self.source.row.context
    }

    pub(crate) fn before(self) -> &'a ExploreValue {
        &self.source.row.before
    }

    pub(crate) fn after(self) -> &'a ExploreValue {
        &self.successor.row.after
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationCatalogError {
    UnknownSource { source_key: SourceKey },
    SourceKeyCollision { source_key: SourceKey },
    SuccessorKeyCollision { successor_key: SuccessorKey },
    CaseIdCollision { case_id: RelationalCaseId },
}

impl fmt::Display for RelationCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSource { .. } => {
                formatter.write_str("relational successor names an unknown SourceKey")
            }
            Self::SourceKeyCollision { .. } => formatter
                .write_str("Explore SourceKey SHA-256 collision rejected by relation catalog"),
            Self::SuccessorKeyCollision { .. } => formatter
                .write_str("Explore SuccessorKey SHA-256 collision rejected by relation catalog"),
            Self::CaseIdCollision { .. } => formatter
                .write_str("Explore relational CaseId SHA-256 collision rejected by catalog"),
        }
    }
}

impl Error for RelationCatalogError {}

fn derive_content_identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut hasher = IdentityHasher::new(domain);
    hasher.bytes(preimage);
    hasher.finish()
}

struct IdentityHasher(Sha256);

impl IdentityHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, tag: u8) {
        self.0.update([tag]);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u128).to_be_bytes());
        self.0.update(bytes);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage(name: &str) -> RelationLineageId {
        RelationLineageId::from_canonical_preimage(name.as_bytes())
    }

    fn support(name: &str) -> RelationSupportId {
        RelationSupportId::from_canonical_preimage(name.as_bytes())
    }

    fn provenance(lineage_name: &str, support_name: &str) -> RelationProvenance {
        RelationProvenance::new([lineage(lineage_name)], [support(support_name)])
    }

    fn source(context: &str, before: i64, suffix: &str) -> SourceRow {
        SourceRow::new(
            ExploreValue::String(context.to_string()),
            ExploreValue::Int(before),
            provenance(&format!("lineage-{suffix}"), &format!("support-{suffix}")),
        )
    }

    fn successor(after: i64, suffix: &str) -> SuccessorRow {
        SuccessorRow::new(
            ExploreValue::Int(after),
            provenance(&format!("lineage-{suffix}"), &format!("support-{suffix}")),
        )
    }

    #[test]
    fn catalog_and_ids_are_arrival_order_independent() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"income-cliff-relation");

        let mut forward = RelationCatalogBuilder::new(relation_id);
        let salary = forward.insert_source(source("salary", 100, "s1")).unwrap();
        forward
            .insert_successor(salary, successor(101, "a1"))
            .unwrap();
        forward
            .insert_successor(salary, successor(102, "a2"))
            .unwrap();
        let pension = forward.insert_source(source("pension", 200, "s2")).unwrap();
        forward
            .insert_successor(pension, successor(201, "a3"))
            .unwrap();

        let mut reverse = RelationCatalogBuilder::new(relation_id);
        let pension = reverse.insert_source(source("pension", 200, "s2")).unwrap();
        reverse
            .insert_successor(pension, successor(201, "a3"))
            .unwrap();
        let salary = reverse.insert_source(source("salary", 100, "s1")).unwrap();
        reverse
            .insert_successor(salary, successor(102, "a2"))
            .unwrap();
        reverse
            .insert_successor(salary, successor(101, "a1"))
            .unwrap();

        assert_eq!(forward.finish(), reverse.finish());
    }

    #[test]
    fn duplicate_rows_collapse_and_union_lineage_and_support() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"set-semantics");
        let mut builder = RelationCatalogBuilder::new(relation_id);

        let first_source = builder
            .insert_source(source("salary", 100, "first"))
            .unwrap();
        let duplicate_source = builder
            .insert_source(source("salary", 100, "second"))
            .unwrap();
        assert_eq!(first_source, duplicate_source);

        let first_successor = builder
            .insert_successor(first_source, successor(101, "third"))
            .unwrap();
        let duplicate_successor = builder
            .insert_successor(first_source, successor(101, "fourth"))
            .unwrap();
        assert_eq!(first_successor, duplicate_successor);

        let catalog = builder.finish();
        assert_eq!(catalog.source_count(), 1);
        assert_eq!(catalog.case_count(), 1);
        let source = &catalog.sources()[0];
        assert_eq!(source.row().provenance().lineage().len(), 2);
        assert_eq!(source.row().provenance().support().len(), 2);
        let successor = &source.successors()[0];
        assert_eq!(successor.row().provenance().lineage().len(), 2);
        assert_eq!(successor.row().provenance().support().len(), 2);
    }

    #[test]
    fn materially_distinct_actions_in_context_remain_distinct_cases() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"action-identity");
        let mut builder = RelationCatalogBuilder::new(relation_id);

        let salary = builder
            .insert_source(source("increase-salary", 100, "salary"))
            .unwrap();
        let pension = builder
            .insert_source(source("increase-pension", 100, "pension"))
            .unwrap();
        let (_, salary_case) = builder
            .insert_successor(salary, successor(101, "after-salary"))
            .unwrap();
        let (_, pension_case) = builder
            .insert_successor(pension, successor(101, "after-pension"))
            .unwrap();

        assert_ne!(salary, pension);
        assert_ne!(salary_case, pension_case);
        let catalog = builder.finish();
        assert_eq!(catalog.case_count(), 2);
        assert_eq!(
            catalog.case(salary_case).unwrap().before(),
            &ExploreValue::Int(100)
        );
        assert_eq!(
            catalog.case(salary_case).unwrap().after(),
            &ExploreValue::Int(101)
        );
        assert_ne!(
            catalog.case(salary_case).unwrap().context(),
            catalog.case(pension_case).unwrap().context()
        );
    }

    #[test]
    fn semantic_preimage_and_digest_produce_stable_relation_and_case_ids() {
        let semantic_preimage = b"canonical-source-and-successor-semantics";
        let semantic_digest = Sha256::digest(semantic_preimage).into();
        let from_preimage = RelationId::from_canonical_semantic_preimage(semantic_preimage);
        let from_digest = RelationId::from_canonical_semantic_digest(semantic_digest);
        assert_eq!(from_preimage, from_digest);

        let mut first = RelationCatalogBuilder::new(from_preimage);
        let first_source = first.insert_source(source("salary", 100, "one")).unwrap();
        let (first_successor, first_case) = first
            .insert_successor(first_source, successor(101, "two"))
            .unwrap();

        let mut second = RelationCatalogBuilder::new(from_digest);
        let second_source = second
            .insert_source(source("salary", 100, "different-lineage"))
            .unwrap();
        let (second_successor, second_case) = second
            .insert_successor(second_source, successor(101, "different-successor-lineage"))
            .unwrap();

        assert_eq!(first_source, second_source);
        assert_eq!(first_successor, second_successor);
        assert_eq!(first_case, second_case);
        assert_eq!(
            first.finish().cases().next().unwrap().case_id(),
            second.finish().cases().next().unwrap().case_id()
        );
    }
}
