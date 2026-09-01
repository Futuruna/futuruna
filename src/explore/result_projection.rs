//! Bounded durable publication of one exact relational result projection.
//!
//! Row evidence already reaches the analysis journal one input row at a time.
//! A [`ClosedResultView`], however, owns the whole contribution set and the
//! whole projected output. Putting that value in one terminal event makes the
//! largest result determine the largest journal frame. This module flattens
//! only the public projection into independently authenticated records and
//! authenticates its exact root over borrowed row evidence at closure.
//!
//! Grouped output needs two record kinds: one bounded group header and one
//! record per chosen row. A group with 200,000 ties therefore cannot smuggle a
//! 200,000-row array into a nominally bounded frame. Records are accepted in
//! the canonical order produced by [`ClosedResultView`]; the ordinal is part
//! of each record identity and the prefix root. Prefix hashing and resume are
//! constant-work per record, while the defensive collision index adds a
//! logarithmic lookup and retains deterministic replay.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::ViewId;
use super::result_evidence::RelationalResultEvidenceCatalogBuilder;
use super::result_view::{
    compact_exact_result_view_from_borrowed, compact_exact_result_view_from_certified_groups,
    compact_exact_result_view_from_certified_input, CertifiedResultGroupSummary,
    CertifiedResultInputRoot, ClosedResultView, CompactClosedResultView,
    ResultCountDistinctSnapshot, ResultGroupDisposition, ResultGroupKey, ResultGroupSnapshot,
    ResultOutputRow, ResultValue, ResultViewCount, ResultViewCounts, ResultViewGrain,
    ResultViewHaving, ResultViewInputRowId, ResultViewOutput, ResultViewRoot, ResultViewSpec,
    ResultViewSpecRoot,
};

const RESULT_PROJECTION_RECORD_ID_V1: &[u8] =
    b"futuruna.explore.relational-result-projection-record-id.v1";
const RESULT_PROJECTION_GENESIS_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-result-projection-genesis-root.v1";
const RESULT_PROJECTION_PREFIX_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-result-projection-prefix-root.v1";

pub(crate) const RESULT_PROJECTION_SNAPSHOT_VERSION: u32 = 1;

/// Content identity of one bounded output record at one canonical ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultProjectionRecordId([u8; 32]);

impl ResultProjectionRecordId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Incremental commitment to one canonical projection-record prefix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResultProjectionRoot([u8; 32]);

impl ResultProjectionRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Group metadata without the potentially unbounded chosen-row array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultProjectionGroup {
    key: ResultGroupKey,
    member_count: ResultViewCount,
    observed_having_varies: Option<bool>,
    disposition: ResultGroupDisposition,
    aggregates: Box<[ResultCountDistinctSnapshot]>,
    projected_values: Option<Box<[ResultValue]>>,
    chosen_row_count: u128,
}

impl ResultProjectionGroup {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        key: ResultGroupKey,
        member_count: ResultViewCount,
        observed_having_varies: Option<bool>,
        disposition: ResultGroupDisposition,
        aggregates: Box<[ResultCountDistinctSnapshot]>,
        projected_values: Option<Box<[ResultValue]>>,
        chosen_row_count: u128,
    ) -> Self {
        Self {
            key,
            member_count,
            observed_having_varies,
            disposition,
            aggregates,
            projected_values,
            chosen_row_count,
        }
    }

    fn from_closed(group: &ResultGroupSnapshot) -> Self {
        Self {
            key: group.key().clone(),
            member_count: group.member_count(),
            observed_having_varies: group.observed_having_varies(),
            disposition: group.disposition(),
            aggregates: group.aggregates().to_vec().into_boxed_slice(),
            projected_values: group
                .projected_values()
                .map(|values| values.to_vec().into_boxed_slice()),
            chosen_row_count: group.chosen_rows().len() as u128,
        }
    }

    pub(crate) const fn key(&self) -> &ResultGroupKey {
        &self.key
    }

    pub(crate) const fn member_count(&self) -> ResultViewCount {
        self.member_count
    }

    pub(crate) const fn observed_having_varies(&self) -> Option<bool> {
        self.observed_having_varies
    }

    pub(crate) const fn disposition(&self) -> ResultGroupDisposition {
        self.disposition
    }

    pub(crate) fn aggregates(&self) -> &[ResultCountDistinctSnapshot] {
        &self.aggregates
    }

    pub(crate) fn projected_values(&self) -> Option<&[ResultValue]> {
        self.projected_values.as_deref()
    }

    pub(crate) const fn chosen_row_count(&self) -> u128 {
        self.chosen_row_count
    }

    fn canonicalize_value_storage(&mut self, visitor: &mut impl FnMut(&mut super::ExploreValue)) {
        self.key.canonicalize_value_storage(visitor);
        if let Some(values) = &mut self.projected_values {
            for value in values.iter_mut() {
                value.canonicalize_value_storage(visitor);
            }
        }
    }
}

/// One bounded unit of the public projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultProjectionRecord {
    /// One ungrouped output row.
    Row(ResultOutputRow),
    /// One grouped-output header, excluding chosen rows.
    Group(ResultProjectionGroup),
    /// One chosen row belonging to the immediately preceding group header.
    ChosenRow {
        group_key: ResultGroupKey,
        row: ResultOutputRow,
    },
}

impl ResultProjectionRecord {
    pub(crate) fn row(&self) -> Option<&ResultOutputRow> {
        match self {
            Self::Row(row) => Some(row),
            Self::Group(_) | Self::ChosenRow { .. } => None,
        }
    }

    pub(crate) fn group(&self) -> Option<&ResultProjectionGroup> {
        match self {
            Self::Group(group) => Some(group),
            Self::Row(_) | Self::ChosenRow { .. } => None,
        }
    }

    pub(crate) fn chosen_row(&self) -> Option<(&ResultGroupKey, &ResultOutputRow)> {
        match self {
            Self::ChosenRow { group_key, row } => Some((group_key, row)),
            Self::Row(_) | Self::Group(_) => None,
        }
    }

    fn canonicalize_value_storage(&mut self, visitor: &mut impl FnMut(&mut super::ExploreValue)) {
        match self {
            Self::Row(row) => row.canonicalize_value_storage(visitor),
            Self::Group(group) => group.canonicalize_value_storage(visitor),
            Self::ChosenRow { group_key, row } => {
                group_key.canonicalize_value_storage(visitor);
                row.canonicalize_value_storage(visitor);
            }
        }
    }
}

/// Canonically ordered record with its durable ordinal and identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexedResultProjectionRecord {
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    ordinal: u128,
    id: ResultProjectionRecordId,
    record: ResultProjectionRecord,
}

impl IndexedResultProjectionRecord {
    pub(super) fn restore_from_journal_codec(
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        ordinal: u128,
        claimed_id: ResultProjectionRecordId,
        record: ResultProjectionRecord,
    ) -> Result<Self, ResultProjectionError> {
        let restored = Self::derive(view_id, spec_root, ordinal, record);
        if restored.id != claimed_id {
            return Err(ResultProjectionError::RecordIdMismatch {
                claimed: claimed_id,
                derived: restored.id,
            });
        }
        Ok(restored)
    }

    pub(crate) fn derive(
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        ordinal: u128,
        record: ResultProjectionRecord,
    ) -> Self {
        let id = derive_record_id(view_id, spec_root, ordinal, &record);
        Self {
            view_id,
            spec_root,
            ordinal,
            id,
            record,
        }
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    pub(crate) const fn id(&self) -> ResultProjectionRecordId {
        self.id
    }

    pub(crate) const fn record(&self) -> &ResultProjectionRecord {
        &self.record
    }

    /// Rewrite only process-local value backing after a journal frame's
    /// content identity has been verified. Record identity remains entirely
    /// value-derived and does not observe shared allocation identity.
    pub(crate) fn canonicalize_value_storage(
        &mut self,
        visitor: &mut impl FnMut(&mut super::ExploreValue),
    ) {
        let expected_id = self.id;
        self.record.canonicalize_value_storage(visitor);
        debug_assert_eq!(
            derive_record_id(self.view_id, self.spec_root, self.ordinal, &self.record),
            expected_id
        );
    }

    fn validate_for(
        &self,
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
    ) -> Result<(), ResultProjectionError> {
        if self.view_id != view_id || self.spec_root != spec_root {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        let derived = derive_record_id(view_id, spec_root, self.ordinal, &self.record);
        if derived == self.id {
            Ok(())
        } else {
            Err(ResultProjectionError::RecordIdMismatch {
                claimed: self.id,
                derived,
            })
        }
    }
}

/// Compact terminal claim. It is authority only after the journal replays
/// every bounded record, hashes the view from exact borrowed row evidence,
/// and compares all closure fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResultProjectionClosure {
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    projection_root: ResultProjectionRoot,
    record_count: u128,
    counts: ResultViewCounts,
    result_root: ResultViewRoot,
}

impl ResultProjectionClosure {
    pub(super) const fn restore_from_journal_codec(
        view_id: ViewId,
        spec_root: ResultViewSpecRoot,
        projection_root: ResultProjectionRoot,
        record_count: u128,
        counts: ResultViewCounts,
        result_root: ResultViewRoot,
    ) -> Self {
        Self {
            view_id,
            spec_root,
            projection_root,
            record_count,
            counts,
            result_root,
        }
    }

    pub(crate) const fn view_id(self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn projection_root(self) -> ResultProjectionRoot {
        self.projection_root
    }

    pub(crate) const fn record_count(self) -> u128 {
        self.record_count
    }

    pub(crate) const fn counts(self) -> ResultViewCounts {
        self.counts
    }

    pub(crate) const fn result_root(self) -> ResultViewRoot {
        self.result_root
    }
}

/// Open projection prefix retained while bounded output events arrive.
#[derive(Clone, Debug)]
pub(crate) struct ResultProjectionCatalogBuilder {
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    root: ResultProjectionRoot,
    records: Vec<IndexedResultProjectionRecord>,
    ordinals_by_id: BTreeMap<ResultProjectionRecordId, u128>,
}

/// Process-local proof that a durable projection prefix matched one
/// deterministic publication plan. The root makes an unchanged prefix an
/// O(1) check and lets a growing catalog authenticate only its new suffix.
/// It is deliberately absent from durable snapshots and semantic identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedResultProjectionPrefix {
    record_count: usize,
    root: ResultProjectionRoot,
}

/// Canonically ordered checkpoint for a projection prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultProjectionSnapshot {
    version: u32,
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    root: ResultProjectionRoot,
    records: Box<[IndexedResultProjectionRecord]>,
}

impl ResultProjectionSnapshot {
    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn root(&self) -> ResultProjectionRoot {
        self.root
    }

    pub(crate) fn records(&self) -> &[IndexedResultProjectionRecord] {
        &self.records
    }
}

impl ResultProjectionCatalogBuilder {
    pub(crate) fn new(spec: &ResultViewSpec) -> Result<Self, ResultProjectionError> {
        spec.validate_spec_root()
            .map_err(|_| ResultProjectionError::SpecRootMismatch)?;
        let view_id = spec.view_id();
        let spec_root = spec.spec_root();
        Ok(Self {
            view_id,
            spec_root,
            root: projection_genesis_root(view_id, spec_root),
            records: Vec::new(),
            ordinals_by_id: BTreeMap::new(),
        })
    }

    pub(crate) const fn view_id(&self) -> ViewId {
        self.view_id
    }

    pub(crate) const fn spec_root(&self) -> ResultViewSpecRoot {
        self.spec_root
    }

    pub(crate) const fn root(&self) -> ResultProjectionRoot {
        self.root
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn record(&self, ordinal: u128) -> Option<&IndexedResultProjectionRecord> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| self.records.get(ordinal))
    }

    pub(crate) fn records(&self) -> impl Iterator<Item = &IndexedResultProjectionRecord> + '_ {
        self.records.iter()
    }

    /// Validate the durable records against a deterministic publication plan
    /// without rescanning an already authenticated prefix. A missing or stale
    /// process-local cursor falls back to one full prefix validation. Cursor
    /// advancement stops at the durable catalog length, so records merely
    /// planned for the next append remain retry-safe.
    pub(crate) fn validate_expected_prefix(
        &self,
        expected: &[IndexedResultProjectionRecord],
        validated: &mut Option<ValidatedResultProjectionPrefix>,
    ) -> Result<(), ResultProjectionError> {
        if self.records.len() > expected.len() {
            return Err(ResultProjectionError::ExpectedPrefixTooShort {
                durable_records: self.records.len() as u128,
                expected_records: expected.len() as u128,
            });
        }

        if let Some(previous) = *validated {
            if previous.record_count <= self.records.len() {
                let mut root = previous.root;
                for ordinal in previous.record_count..self.records.len() {
                    let durable = &self.records[ordinal];
                    if expected.get(ordinal) != Some(durable) {
                        return Err(ResultProjectionError::ExpectedRecordMismatch {
                            ordinal: ordinal as u128,
                        });
                    }
                    root = extend_projection_root(root, durable);
                }
                if root == self.root {
                    *validated = Some(ValidatedResultProjectionPrefix {
                        record_count: self.records.len(),
                        root,
                    });
                    return Ok(());
                }
            }
        }

        // The catalog shrank, an authenticated prefix was replaced, or this
        // process-local cursor belongs to an older catalog instance. Rebuild
        // the small cursor from durable authority exactly once.
        let mut root = projection_genesis_root(self.view_id, self.spec_root);
        for (ordinal, durable) in self.records.iter().enumerate() {
            if expected.get(ordinal) != Some(durable) {
                return Err(ResultProjectionError::ExpectedRecordMismatch {
                    ordinal: ordinal as u128,
                });
            }
            root = extend_projection_root(root, durable);
        }
        if root != self.root {
            return Err(ResultProjectionError::ValidatedPrefixRootMismatch {
                expected: root,
                actual: self.root,
            });
        }
        *validated = Some(ValidatedResultProjectionPrefix {
            record_count: self.records.len(),
            root,
        });
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> ResultProjectionSnapshot {
        ResultProjectionSnapshot {
            version: RESULT_PROJECTION_SNAPSHOT_VERSION,
            view_id: self.view_id,
            spec_root: self.spec_root,
            root: self.root,
            records: self.records.clone().into_boxed_slice(),
        }
    }

    /// Consume this projection prefix into its canonical checkpoint. The
    /// identity-to-ordinal map is a replay accelerator rather than snapshot
    /// evidence, so terminal assembly moves the record vector directly.
    pub(crate) fn into_snapshot(self) -> ResultProjectionSnapshot {
        let Self {
            view_id,
            spec_root,
            root,
            records,
            ordinals_by_id,
        } = self;
        drop(ordinals_by_id);
        ResultProjectionSnapshot {
            version: RESULT_PROJECTION_SNAPSHOT_VERSION,
            view_id,
            spec_root,
            root,
            records: records.into_boxed_slice(),
        }
    }

    pub(crate) fn from_snapshot(
        snapshot: ResultProjectionSnapshot,
        spec: &ResultViewSpec,
    ) -> Result<Self, ResultProjectionError> {
        if snapshot.version != RESULT_PROJECTION_SNAPSHOT_VERSION {
            return Err(ResultProjectionError::UnsupportedSnapshotVersion {
                actual: snapshot.version,
                expected: RESULT_PROJECTION_SNAPSHOT_VERSION,
            });
        }
        if snapshot.view_id != spec.view_id() || snapshot.spec_root != spec.spec_root() {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        let expected_root = snapshot.root;
        let mut restored = Self::new(spec)?;
        for record in snapshot.records.into_vec() {
            restored.insert(record)?;
        }
        if restored.root != expected_root {
            return Err(ResultProjectionError::SnapshotRootMismatch);
        }
        Ok(restored)
    }

    pub(crate) fn insert(
        &mut self,
        indexed: IndexedResultProjectionRecord,
    ) -> Result<bool, ResultProjectionError> {
        indexed.validate_for(self.view_id, self.spec_root)?;
        let expected = self.records.len() as u128;
        if indexed.ordinal < expected {
            let existing = self
                .record(indexed.ordinal)
                .ok_or(ResultProjectionError::CatalogIndexDiverged)?;
            return if existing == &indexed {
                Ok(false)
            } else {
                Err(ResultProjectionError::RecordConflict {
                    ordinal: indexed.ordinal,
                })
            };
        }
        if indexed.ordinal != expected {
            return Err(ResultProjectionError::NonContiguousOrdinal {
                expected,
                actual: indexed.ordinal,
            });
        }
        if let Some(first_ordinal) = self.ordinals_by_id.get(&indexed.id) {
            return Err(ResultProjectionError::RecordIdentityCollision {
                id: indexed.id,
                first_ordinal: *first_ordinal,
                second_ordinal: indexed.ordinal,
            });
        }

        let next_root = extend_projection_root(self.root, &indexed);
        self.ordinals_by_id.insert(indexed.id, indexed.ordinal);
        self.records.push(indexed);
        self.root = next_root;
        Ok(true)
    }

    /// Flatten a freshly evaluated exact view into the same canonical records
    /// the durable journal accepts. This is operational preparation only; the
    /// returned values gain authority one bounded event at a time.
    pub(crate) fn records_from_closed(
        view: &ClosedResultView,
    ) -> Result<Box<[IndexedResultProjectionRecord]>, ResultProjectionError> {
        if !view.validate_identity() {
            return Err(ResultProjectionError::InvalidClosedView);
        }
        let snapshot = view.snapshot();
        Ok(records_from_output(
            view.view_id(),
            snapshot.spec().spec_root(),
            snapshot.output(),
        ))
    }

    /// Flatten compact grouped publication preparation without first owning a
    /// second copy of every exact input contribution.
    pub(crate) fn records_from_compact(
        view: &CompactClosedResultView,
    ) -> Box<[IndexedResultProjectionRecord]> {
        records_from_output(view.view_id(), view.spec_root(), view.output())
    }

    /// Rebuild and validate the exact projected result without evaluating any
    /// checked expression. All expression results came from bounded durable
    /// records; all reducer contributions come from the sealed evidence
    /// catalog.
    pub(crate) fn materialize_closed(
        &self,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<ClosedResultView, ResultProjectionError> {
        self.validate_scope(spec, evidence)?;
        let output = self.materialize_output(spec)?;
        validate_output_membership(spec, evidence, &output)?;
        let contributions = evidence
            .records()
            .map(|record| record.contribution().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        ClosedResultView::restore_from_journal_codec(spec.clone(), contributions, output)
            .map_err(ResultProjectionError::ClosedView)
    }

    pub(crate) fn closure_for(
        &self,
        view: &ClosedResultView,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<ResultProjectionClosure, ResultProjectionError> {
        if view.view_id() != self.view_id || view.snapshot().spec().spec_root() != self.spec_root {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        if !view.validate_identity() {
            return Err(ResultProjectionError::InvalidClosedView);
        }
        let compact = self.compact_from_durable(view.snapshot().spec(), evidence)?;
        if compact.root() != view.root()
            || compact.counts() != view.counts()
            || compact.output() != view.snapshot().output()
        {
            return Err(ResultProjectionError::ProjectionMismatch);
        }
        Ok(ResultProjectionClosure {
            view_id: self.view_id,
            spec_root: self.spec_root,
            projection_root: self.root,
            record_count: self.records.len() as u128,
            counts: view.counts(),
            result_root: view.root(),
        })
    }

    /// Derive the compact closure from this complete durable prefix itself.
    /// This is used after all bounded records are installed, so an executor
    /// does not need to retain a second full `ClosedResultView` in memory.
    pub(crate) fn closure_from_durable(
        &self,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<ResultProjectionClosure, ResultProjectionError> {
        let view = self.compact_from_durable(spec, evidence)?;
        Ok(ResultProjectionClosure {
            view_id: self.view_id,
            spec_root: self.spec_root,
            projection_root: self.root,
            record_count: self.records.len() as u128,
            counts: view.counts(),
            result_root: view.root(),
        })
    }

    /// Derive the compact closure for a proof-specialized source population.
    /// The durable projection owns the one public group; the certified input
    /// root and exact N replace a population-sized contribution catalog.
    pub(crate) fn closure_from_certified_source(
        &self,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        group_values: &[ResultValue],
    ) -> Result<ResultProjectionClosure, ResultProjectionError> {
        let view = self.compact_from_certified_source(
            spec,
            certified_input_root,
            exact_input_count,
            group_values,
        )?;
        Ok(ResultProjectionClosure {
            view_id: self.view_id,
            spec_root: self.spec_root,
            projection_root: self.root,
            record_count: self.records.len() as u128,
            counts: view.counts(),
            result_root: view.root(),
        })
    }

    pub(crate) fn closure_from_certified_source_groups(
        &self,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        groups: &[CertifiedResultGroupSummary],
    ) -> Result<ResultProjectionClosure, ResultProjectionError> {
        let view = self.compact_from_certified_source_groups(
            spec,
            certified_input_root,
            exact_input_count,
            groups,
        )?;
        Ok(ResultProjectionClosure {
            view_id: self.view_id,
            spec_root: self.spec_root,
            projection_root: self.root,
            record_count: self.records.len() as u128,
            counts: view.counts(),
            result_root: view.root(),
        })
    }

    pub(crate) fn validate_closure(
        &self,
        closure: ResultProjectionClosure,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        if closure.view_id != self.view_id
            || closure.spec_root != self.spec_root
            || closure.projection_root != self.root
            || closure.record_count != self.records.len() as u128
        {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        let closed = self.compact_from_durable(spec, evidence)?;
        if closed.root() != closure.result_root || closed.counts() != closure.counts {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        Ok(closed)
    }

    pub(crate) fn validate_certified_source_closure(
        &self,
        closure: ResultProjectionClosure,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        group_values: &[ResultValue],
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        if closure.view_id != self.view_id
            || closure.spec_root != self.spec_root
            || closure.projection_root != self.root
            || closure.record_count != self.records.len() as u128
        {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        let closed = self.compact_from_certified_source(
            spec,
            certified_input_root,
            exact_input_count,
            group_values,
        )?;
        if closed.root() != closure.result_root || closed.counts() != closure.counts {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        Ok(closed)
    }

    pub(crate) fn validate_certified_source_groups_closure(
        &self,
        closure: ResultProjectionClosure,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        groups: &[CertifiedResultGroupSummary],
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        if closure.view_id != self.view_id
            || closure.spec_root != self.spec_root
            || closure.projection_root != self.root
            || closure.record_count != self.records.len() as u128
        {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        let closed = self.compact_from_certified_source_groups(
            spec,
            certified_input_root,
            exact_input_count,
            groups,
        )?;
        if closed.root() != closure.result_root || closed.counts() != closure.counts {
            return Err(ResultProjectionError::ClosureMismatch);
        }
        Ok(closed)
    }

    /// Validate and hash the complete durable projection against borrowed
    /// sealed row evidence. Only the materialized public output is owned; the
    /// canonical contribution set remains in the evidence catalog.
    pub(crate) fn compact_from_durable(
        &self,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        self.validate_scope(spec, evidence)?;
        let output = self.materialize_output(spec)?;
        validate_output_membership(spec, evidence, &output)?;
        let contributions = evidence
            .records()
            .map(|record| record.contribution())
            .collect::<Vec<_>>();
        compact_exact_result_view_from_borrowed(spec, &contributions, output)
            .map_err(ResultProjectionError::ClosedView)
    }

    pub(crate) fn compact_from_certified_source(
        &self,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        group_values: &[ResultValue],
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        spec.validate_spec_root()
            .map_err(|_| ResultProjectionError::SpecRootMismatch)?;
        if spec.view_id() != self.view_id || spec.spec_root() != self.spec_root {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        let output = self.materialize_output(spec)?;
        compact_exact_result_view_from_certified_input(
            spec,
            certified_input_root,
            exact_input_count,
            group_values,
            output,
        )
        .map_err(ResultProjectionError::ClosedView)
    }

    pub(crate) fn compact_from_certified_source_groups(
        &self,
        spec: &ResultViewSpec,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        groups: &[CertifiedResultGroupSummary],
    ) -> Result<CompactClosedResultView, ResultProjectionError> {
        spec.validate_spec_root()
            .map_err(|_| ResultProjectionError::SpecRootMismatch)?;
        if spec.view_id() != self.view_id || spec.spec_root() != self.spec_root {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        let output = self.materialize_output(spec)?;
        compact_exact_result_view_from_certified_groups(
            spec,
            certified_input_root,
            exact_input_count,
            groups,
            output,
        )
        .map_err(ResultProjectionError::ClosedView)
    }

    fn validate_scope(
        &self,
        spec: &ResultViewSpec,
        evidence: &RelationalResultEvidenceCatalogBuilder,
    ) -> Result<(), ResultProjectionError> {
        spec.validate_spec_root()
            .map_err(|_| ResultProjectionError::SpecRootMismatch)?;
        if spec.view_id() != self.view_id
            || spec.spec_root() != self.spec_root
            || evidence.view_id() != self.view_id
            || evidence.spec_root() != self.spec_root
        {
            return Err(ResultProjectionError::ScopeMismatch);
        }
        if !evidence.input_is_sealed() {
            return Err(ResultProjectionError::InputFrontierOpen);
        }
        Ok(())
    }

    fn materialize_output(
        &self,
        spec: &ResultViewSpec,
    ) -> Result<ResultViewOutput, ResultProjectionError> {
        if spec.grain().is_grouped() {
            self.materialize_groups(spec)
        } else {
            self.materialize_rows(spec)
        }
    }

    fn materialize_rows(
        &self,
        spec: &ResultViewSpec,
    ) -> Result<ResultViewOutput, ResultProjectionError> {
        let mut rows = Vec::with_capacity(self.records.len());
        let mut previous = None;
        for indexed in &self.records {
            let ResultProjectionRecord::Row(row) = &indexed.record else {
                return Err(ResultProjectionError::RecordKindMismatch);
            };
            validate_output_row(spec, row)?;
            if previous.is_some_and(|previous| row.row_id() <= previous) {
                return Err(ResultProjectionError::NonCanonicalOutputOrder);
            }
            previous = Some(row.row_id());
            rows.push(row.clone());
        }
        Ok(ResultViewOutput::Rows(rows.into_boxed_slice()))
    }

    fn materialize_groups(
        &self,
        spec: &ResultViewSpec,
    ) -> Result<ResultViewOutput, ResultProjectionError> {
        let mut groups = Vec::new();
        let mut cursor = 0_usize;
        let mut previous_key: Option<&ResultGroupKey> = None;
        let mut chosen_rows_seen = BTreeMap::<ResultViewInputRowId, ()>::new();

        while cursor < self.records.len() {
            let ResultProjectionRecord::Group(group) = &self.records[cursor].record else {
                return Err(ResultProjectionError::RecordKindMismatch);
            };
            validate_group(spec, group)?;
            if previous_key.is_some_and(|previous| group.key() <= previous) {
                return Err(ResultProjectionError::NonCanonicalOutputOrder);
            }
            previous_key = Some(&group.key);
            cursor += 1;

            let chosen_count = usize::try_from(group.chosen_row_count)
                .map_err(|_| ResultProjectionError::CountOverflow)?;
            let end = cursor
                .checked_add(chosen_count)
                .ok_or(ResultProjectionError::CountOverflow)?;
            if end > self.records.len() {
                return Err(ResultProjectionError::ChosenRowCountMismatch);
            }
            let mut chosen_rows = Vec::with_capacity(chosen_count);
            let mut previous_row = None;
            for indexed in &self.records[cursor..end] {
                let ResultProjectionRecord::ChosenRow { group_key, row } = &indexed.record else {
                    return Err(ResultProjectionError::RecordKindMismatch);
                };
                if group_key != &group.key {
                    return Err(ResultProjectionError::ChosenRowGroupMismatch);
                }
                validate_output_row(spec, row)?;
                if previous_row.is_some_and(|previous| row.row_id() <= previous)
                    || chosen_rows_seen.insert(row.row_id(), ()).is_some()
                {
                    return Err(ResultProjectionError::NonCanonicalOutputOrder);
                }
                previous_row = Some(row.row_id());
                chosen_rows.push(row.clone());
            }
            cursor = end;

            groups.push(ResultGroupSnapshot::from_journal_codec_parts(
                group.key.clone(),
                group.member_count,
                group.observed_having_varies,
                group.disposition,
                group.aggregates.clone(),
                group.projected_values.clone(),
                chosen_rows.into_boxed_slice(),
            ));
        }
        Ok(ResultViewOutput::Groups(groups.into_boxed_slice()))
    }
}

fn records_from_output(
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    output: &ResultViewOutput,
) -> Box<[IndexedResultProjectionRecord]> {
    let mut records = Vec::new();
    match output {
        ResultViewOutput::Rows(rows) => {
            records.extend(rows.iter().cloned().map(ResultProjectionRecord::Row));
        }
        ResultViewOutput::Groups(groups) => {
            for group in groups {
                let key = group.key().clone();
                records.push(ResultProjectionRecord::Group(
                    ResultProjectionGroup::from_closed(group),
                ));
                records.extend(group.chosen_rows().iter().cloned().map(|row| {
                    ResultProjectionRecord::ChosenRow {
                        group_key: key.clone(),
                        row,
                    }
                }));
            }
        }
    }
    records
        .into_iter()
        .enumerate()
        .map(|(ordinal, record)| {
            IndexedResultProjectionRecord::derive(view_id, spec_root, ordinal as u128, record)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResultProjectionError {
    UnsupportedSnapshotVersion {
        actual: u32,
        expected: u32,
    },
    SnapshotRootMismatch,
    SpecRootMismatch,
    ScopeMismatch,
    InputFrontierOpen,
    InvalidClosedView,
    ClosedView(super::result_view::ResultViewError),
    RecordIdMismatch {
        claimed: ResultProjectionRecordId,
        derived: ResultProjectionRecordId,
    },
    RecordIdentityCollision {
        id: ResultProjectionRecordId,
        first_ordinal: u128,
        second_ordinal: u128,
    },
    RecordConflict {
        ordinal: u128,
    },
    NonContiguousOrdinal {
        expected: u128,
        actual: u128,
    },
    CatalogIndexDiverged,
    RecordKindMismatch,
    NonCanonicalOutputOrder,
    OutputRowKindMismatch,
    OutputShapeMismatch,
    OutputRowOutsideEvidence,
    UnchosenProjectionCoverageMismatch,
    GroupShapeMismatch,
    GroupStateMismatch,
    GroupEvidenceMismatch,
    ChosenRowGroupMismatch,
    ChosenRowCountMismatch,
    CountOverflow,
    ProjectionMismatch,
    ClosureMismatch,
    ExpectedPrefixTooShort {
        durable_records: u128,
        expected_records: u128,
    },
    ExpectedRecordMismatch {
        ordinal: u128,
    },
    ValidatedPrefixRootMismatch {
        expected: ResultProjectionRoot,
        actual: ResultProjectionRoot,
    },
}

impl fmt::Display for ResultProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSnapshotVersion { actual, expected } => write!(
                formatter,
                "unsupported result projection snapshot version {actual}; expected {expected}"
            ),
            Self::SnapshotRootMismatch => {
                formatter.write_str("result projection snapshot root does not match its records")
            }
            Self::SpecRootMismatch => formatter
                .write_str("result projection spec root does not match its checked contract"),
            Self::ScopeMismatch => {
                formatter.write_str("result projection belongs to another result layer")
            }
            Self::InputFrontierOpen => formatter
                .write_str("result projection cannot close before exact row evidence closes"),
            Self::InvalidClosedView => {
                formatter.write_str("result projection source view is not an exact closed view")
            }
            Self::ClosedView(error) => error.fmt(formatter),
            Self::RecordIdMismatch { .. } => {
                formatter.write_str("result projection record identity does not match its content")
            }
            Self::RecordIdentityCollision { .. } => formatter
                .write_str("different result projection ordinals share one content identity"),
            Self::RecordConflict { .. } => {
                formatter.write_str("result projection ordinal was replayed with different content")
            }
            Self::NonContiguousOrdinal { .. } => {
                formatter.write_str("result projection records must extend the canonical prefix")
            }
            Self::CatalogIndexDiverged => formatter.write_str("result projection indexes disagree"),
            Self::RecordKindMismatch => {
                formatter.write_str("result projection record kind disagrees with the result grain")
            }
            Self::NonCanonicalOutputOrder => {
                formatter.write_str("result projection records are not in canonical output order")
            }
            Self::OutputRowKindMismatch => {
                formatter.write_str("result projection row has the wrong input identity kind")
            }
            Self::OutputShapeMismatch => {
                formatter.write_str("result projection row has the wrong SELECT value shape")
            }
            Self::OutputRowOutsideEvidence => {
                formatter.write_str("result projection row is absent from the exact input evidence")
            }
            Self::UnchosenProjectionCoverageMismatch => formatter
                .write_str("result projection without a choice must publish every exact input row"),
            Self::GroupShapeMismatch => {
                formatter.write_str("result projection group has the wrong key or aggregate shape")
            }
            Self::GroupStateMismatch => formatter
                .write_str("result projection group is not an exact checked terminal group"),
            Self::GroupEvidenceMismatch => formatter
                .write_str("result projection group metadata differs from exact reducer evidence"),
            Self::ChosenRowGroupMismatch => {
                formatter.write_str("result projection chosen row belongs to another group")
            }
            Self::ChosenRowCountMismatch => formatter
                .write_str("result projection group does not contain its claimed chosen rows"),
            Self::CountOverflow => formatter.write_str("result projection count overflowed usize"),
            Self::ProjectionMismatch => formatter
                .write_str("durable result projection differs from the evaluated closed view"),
            Self::ClosureMismatch => {
                formatter.write_str("result projection closure disagrees with its durable prefix")
            }
            Self::ExpectedPrefixTooShort { .. } => formatter
                .write_str("durable result projection exceeds its deterministic publication plan"),
            Self::ExpectedRecordMismatch { .. } => formatter.write_str(
                "durable result projection prefix differs from its deterministic publication plan",
            ),
            Self::ValidatedPrefixRootMismatch { .. } => formatter.write_str(
                "durable result projection prefix root disagrees with its retained records",
            ),
        }
    }
}

impl Error for ResultProjectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ClosedView(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_output_row(
    spec: &ResultViewSpec,
    row: &ResultOutputRow,
) -> Result<(), ResultProjectionError> {
    if row.row_id().kind() != spec.input_kind() {
        return Err(ResultProjectionError::OutputRowKindMismatch);
    }
    if row.values().len() != spec.projection_names().len() {
        return Err(ResultProjectionError::OutputShapeMismatch);
    }
    Ok(())
}

fn validate_output_membership(
    spec: &ResultViewSpec,
    evidence: &RelationalResultEvidenceCatalogBuilder,
    output: &ResultViewOutput,
) -> Result<(), ResultProjectionError> {
    let validate = |row: &ResultOutputRow| {
        if evidence.record(row.row_id()).is_some() {
            Ok(())
        } else {
            Err(ResultProjectionError::OutputRowOutsideEvidence)
        }
    };
    match output {
        ResultViewOutput::Rows(rows) => {
            for row in rows {
                validate(row)?;
            }
            if spec.choice().is_none()
                && (rows.len() != evidence.len()
                    || rows
                        .iter()
                        .zip(evidence.records())
                        .any(|(row, record)| row.row_id() != record.row_id()))
            {
                return Err(ResultProjectionError::UnchosenProjectionCoverageMismatch);
            }
        }
        ResultViewOutput::Groups(groups) => {
            validate_group_evidence(spec, evidence, groups)?;
            for group in groups {
                for row in group.chosen_rows() {
                    validate(row)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_group_evidence(
    spec: &ResultViewSpec,
    evidence: &RelationalResultEvidenceCatalogBuilder,
    groups: &[ResultGroupSnapshot],
) -> Result<(), ResultProjectionError> {
    let mut expected = BTreeMap::<Box<[ResultValue]>, Vec<_>>::new();
    if matches!(spec.grain(), ResultViewGrain::GroupAll) {
        expected.insert(Vec::new().into_boxed_slice(), Vec::new());
    }
    for record in evidence.records() {
        expected
            .entry(record.grain_values().to_vec().into_boxed_slice())
            .or_default()
            .push(record);
    }
    if expected.len() != groups.len() {
        return Err(ResultProjectionError::GroupEvidenceMismatch);
    }

    for ((expected_key, members), group) in expected.iter().zip(groups) {
        if expected_key.as_ref() != group.key().values()
            || group.member_count().current() != members.len() as u128
        {
            return Err(ResultProjectionError::GroupEvidenceMismatch);
        }
        let expected_varies = match spec.having() {
            None => None,
            Some(ResultViewHaving::Varies { measure_index }) => {
                let mut varies = false;
                if let Some(first) = members.first() {
                    let first = first
                        .measures()
                        .get(measure_index)
                        .ok_or(ResultProjectionError::GroupEvidenceMismatch)?;
                    for member in members.iter().skip(1) {
                        let value = member
                            .measures()
                            .get(measure_index)
                            .ok_or(ResultProjectionError::GroupEvidenceMismatch)?;
                        varies |= value != first;
                    }
                }
                Some(varies)
            }
        };
        if group.observed_having_varies() != expected_varies {
            return Err(ResultProjectionError::GroupEvidenceMismatch);
        }
        let included = expected_varies.unwrap_or(true);
        let expected_disposition = if included {
            ResultGroupDisposition::ExactIncluded
        } else {
            ResultGroupDisposition::ExactExcluded
        };
        if group.disposition() != expected_disposition {
            return Err(ResultProjectionError::GroupEvidenceMismatch);
        }
        for (index, aggregate) in group.aggregates().iter().enumerate() {
            let mut values = BTreeSet::new();
            for member in members {
                values.insert(
                    member
                        .distinct_arguments()
                        .get(index)
                        .ok_or(ResultProjectionError::GroupEvidenceMismatch)?,
                );
            }
            let distinct = values.len() as u128;
            if aggregate.count().current() != distinct {
                return Err(ResultProjectionError::GroupEvidenceMismatch);
            }
        }
    }
    Ok(())
}

fn validate_group(
    spec: &ResultViewSpec,
    group: &ResultProjectionGroup,
) -> Result<(), ResultProjectionError> {
    let (ResultViewGrain::GroupAll | ResultViewGrain::GroupBy { .. }) = spec.grain() else {
        return Err(ResultProjectionError::RecordKindMismatch);
    };
    if group.key.values().len() != spec.grain().group_field_names().len()
        || group.aggregates.len() != spec.aggregate_names().len()
        || group
            .aggregates
            .iter()
            .zip(spec.aggregate_names())
            .any(|(aggregate, expected)| aggregate.name() != expected.as_ref())
    {
        return Err(ResultProjectionError::GroupShapeMismatch);
    }
    if !group.member_count.is_exact()
        || !group.disposition.is_exact()
        || spec.having().is_some() != group.observed_having_varies.is_some()
        || group
            .aggregates
            .iter()
            .any(|aggregate| !aggregate.count().is_exact())
    {
        return Err(ResultProjectionError::GroupStateMismatch);
    }

    let included = group.disposition == ResultGroupDisposition::ExactIncluded;
    match spec.choice() {
        Some(_) => {
            if group.projected_values.is_some()
                || group.chosen_row_count > group.member_count.current()
                || (included && group.member_count.current() != 0 && group.chosen_row_count == 0)
                || (!included && group.chosen_row_count != 0)
            {
                return Err(ResultProjectionError::GroupStateMismatch);
            }
        }
        None => {
            let projection_matches = match (included, group.projected_values.as_ref()) {
                (true, Some(values)) => values.len() == spec.projection_names().len(),
                (false, None) => true,
                _ => false,
            };
            if group.chosen_row_count != 0 || !projection_matches {
                return Err(ResultProjectionError::GroupStateMismatch);
            }
        }
    }
    Ok(())
}

fn derive_record_id(
    view_id: ViewId,
    spec_root: ResultViewSpecRoot,
    ordinal: u128,
    record: &ResultProjectionRecord,
) -> ResultProjectionRecordId {
    let mut hasher = ProjectionHasher::new(RESULT_PROJECTION_RECORD_ID_V1);
    hasher.digest(view_id.bytes());
    hasher.digest(spec_root.bytes());
    hasher.u128(ordinal);
    hash_record(&mut hasher, record);
    ResultProjectionRecordId(hasher.finish())
}

fn projection_genesis_root(view_id: ViewId, spec_root: ResultViewSpecRoot) -> ResultProjectionRoot {
    let mut hasher = ProjectionHasher::new(RESULT_PROJECTION_GENESIS_ROOT_V1);
    hasher.digest(view_id.bytes());
    hasher.digest(spec_root.bytes());
    ResultProjectionRoot(hasher.finish())
}

fn extend_projection_root(
    previous: ResultProjectionRoot,
    record: &IndexedResultProjectionRecord,
) -> ResultProjectionRoot {
    let mut hasher = ProjectionHasher::new(RESULT_PROJECTION_PREFIX_ROOT_V1);
    hasher.digest(previous.bytes());
    hasher.u128(record.ordinal);
    hasher.digest(record.id.bytes());
    ResultProjectionRoot(hasher.finish())
}

fn hash_record(hasher: &mut ProjectionHasher, record: &ResultProjectionRecord) {
    match record {
        ResultProjectionRecord::Row(row) => {
            hasher.tag(0x01);
            hash_output_row(hasher, row);
        }
        ResultProjectionRecord::Group(group) => {
            hasher.tag(0x02);
            hash_values(hasher, group.key.values());
            hash_count(hasher, group.member_count);
            match group.observed_having_varies {
                None => hasher.tag(0x00),
                Some(value) => {
                    hasher.tag(0x01);
                    hasher.tag(u8::from(value));
                }
            }
            hasher.tag(match group.disposition {
                ResultGroupDisposition::Provisional { .. } => 0x01,
                ResultGroupDisposition::ExactIncluded => 0x02,
                ResultGroupDisposition::ExactExcluded => 0x03,
            });
            if let ResultGroupDisposition::Provisional {
                currently_passes_having,
            } = group.disposition
            {
                hasher.tag(u8::from(currently_passes_having));
            }
            hasher.u128(group.aggregates.len() as u128);
            for aggregate in &group.aggregates {
                hasher.bytes(aggregate.name().as_bytes());
                hash_count(hasher, aggregate.count());
            }
            match &group.projected_values {
                None => hasher.tag(0x00),
                Some(values) => {
                    hasher.tag(0x01);
                    hash_values(hasher, values);
                }
            }
            hasher.u128(group.chosen_row_count);
        }
        ResultProjectionRecord::ChosenRow { group_key, row } => {
            hasher.tag(0x03);
            hash_values(hasher, group_key.values());
            hash_output_row(hasher, row);
        }
    }
}

fn hash_output_row(hasher: &mut ProjectionHasher, row: &ResultOutputRow) {
    hash_row_id(hasher, row.row_id());
    hash_values(hasher, row.values());
}

fn hash_row_id(hasher: &mut ProjectionHasher, row_id: ResultViewInputRowId) {
    match row_id {
        ResultViewInputRowId::Source(source_key) => {
            hasher.tag(0x03);
            hasher.digest(source_key.bytes());
        }
        ResultViewInputRowId::Case(case_id) => {
            hasher.tag(0x01);
            hasher.digest(case_id.bytes());
        }
        ResultViewInputRowId::Incidence(incidence) => {
            hasher.tag(0x02);
            hasher.digest(incidence.case_id().bytes());
            hasher.digest(incidence.transition_id().bytes());
            hasher.digest(incidence.signature_id().request_id().bytes());
            hasher.digest(incidence.signature_id().bytes());
        }
    }
}

fn hash_values(hasher: &mut ProjectionHasher, values: &[ResultValue]) {
    use super::transition::canonical_explore_value_digest;

    hasher.u128(values.len() as u128);
    for value in values {
        match value {
            ResultValue::Value(value) => {
                hasher.tag(0x01);
                hasher.digest(canonical_explore_value_digest(value));
            }
            ResultValue::CaseId(case_id) => {
                hasher.tag(0x02);
                hasher.digest(case_id.bytes());
            }
            ResultValue::TransitionId(transition_id) => {
                hasher.tag(0x03);
                hasher.digest(transition_id.bytes());
            }
            ResultValue::SignatureId(signature_id) => {
                hasher.tag(0x04);
                hasher.digest(signature_id.request_id().bytes());
                hasher.digest(signature_id.bytes());
            }
            ResultValue::StructuralMechanismId(mechanism_id) => {
                hasher.tag(0x05);
                hasher.digest(mechanism_id.bytes());
            }
            ResultValue::ExecutionProfileId(profile_id) => {
                hasher.tag(0x06);
                hasher.digest(profile_id.bytes());
            }
        }
    }
}

fn hash_count(hasher: &mut ProjectionHasher, count: ResultViewCount) {
    match count {
        ResultViewCount::LowerBound(value) => {
            hasher.tag(0x01);
            hasher.u128(value);
        }
        ResultViewCount::Provisional(value) => {
            hasher.tag(0x02);
            hasher.u128(value);
        }
        ResultViewCount::Exact(value) => {
            hasher.tag(0x03);
            hasher.u128(value);
        }
    }
}

struct ProjectionHasher(Sha256);

impl ProjectionHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
