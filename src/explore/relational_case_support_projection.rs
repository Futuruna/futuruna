//! Stable-key public projection of sparse classified case/support evidence.
//!
//! Discovery order controls only when an `add` becomes visible. Logical keys
//! and row hashes are independent of journal checkpoints and arrival order, so
//! a late lower chunk ordinal can append safely. Exact closure seals the
//! canonical active set sorted by logical key.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{
    AdmissionId, QuestionId, RelationId, RelationalCaseId, SelectionDecision, ViewId,
};
use super::relational_analysis_journal::{
    RelationalSelectedPopulationAuthority, RelationalSelectedQuestionSeal,
    RelationalSelectedQuestionSealId,
};
use super::relational_bounded_chunk_partition::{
    RelationalCaseChunkPartitionArtifact, RelationalCaseChunkPartitionArtifactId,
    VerifiedRelationalCaseChunkPartition,
};
use super::relational_classified_sweep::{
    RelationalClassifiedCaseOutcome, RelationalClassifiedChunkArtifact,
    RelationalClassifiedChunkArtifactId, RelationalClassifiedRunDescriptor,
    RelationalClassifiedRunId,
};
use super::relational_journal::{
    RelationalCaseSupportDiscoveryEvent, RelationalClassifiedSupportFragment,
    RelationalSchedulerView,
};
use super::relational_region_proof::{
    RelationalCertifiedRegionConclusion, RelationalRegionProofSubject, RelationalStarterRegionId,
};
use super::relational_selected_run_materialization::{
    RelationalSelectedRunMaterializationArtifact, RelationalSelectedRunMaterializationArtifactId,
};
use super::relational_support_planner::RelationalSupportPlanRoot;
use super::support_evidence::SupportEvidenceRoot;

pub(crate) const RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION: u32 = 4;
pub(crate) const RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA: &str =
    "futuruna.relational-case-support-graph.v4";
pub(crate) const RELATIONAL_CASE_SUPPORT_UPDATE_ALGEBRA: &str = "stable_key_add_seal.v1";

const PROJECTION_ID_HASH_V4: &[u8] = b"futuruna.explore.relational-case-support-projection-id.v4";
const ROW_HASH_V1: &[u8] = b"futuruna.explore.relational-case-support-row.v1";
const ACTIVE_SET_ROOT_HASH_V1: &[u8] = b"futuruna.explore.relational-case-support-active-set.v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseSupportProjectionId([u8; 32]);

impl RelationalCaseSupportProjectionId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseSupportRowHash([u8; 32]);

impl RelationalCaseSupportRowHash {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseSupportActiveSetRoot([u8; 32]);

impl RelationalCaseSupportActiveSetRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// The semantic basis distinguishes the bounded-partition projection from the
/// exact classification-summary fallback while retaining one v4 envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionBasis {
    Partition(RelationalCaseChunkPartitionArtifactId),
    ClassificationSummary,
}

/// Stable logical address. Artifact/content IDs belong in the row payload,
/// never in this key, so replacing a fact at one slot is detectable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RelationalCaseSupportRecordKey {
    Root,
    ClassificationRegion {
        region_ordinal: u16,
    },
    ClassificationAuthorizedCase {
        case_id: RelationalCaseId,
    },
    Chunk {
        chunk_ordinal: u128,
    },
    Region {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    SelectedMaterialization {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    AuthorizedCase {
        chunk_ordinal: u128,
        run_ordinal: u16,
        case_id: RelationalCaseId,
    },
}

impl Ord for RelationalCaseSupportRecordKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        use RelationalCaseSupportRecordKey as Key;
        match (self, other) {
            (Key::Root, Key::Root) => Ordering::Equal,
            (Key::Root, _) => Ordering::Less,
            (_, Key::Root) => Ordering::Greater,
            (
                Key::ClassificationRegion {
                    region_ordinal: left,
                },
                Key::ClassificationRegion {
                    region_ordinal: right,
                },
            ) => left.cmp(right),
            (Key::ClassificationRegion { .. }, Key::ClassificationAuthorizedCase { .. }) => {
                Ordering::Less
            }
            (Key::ClassificationAuthorizedCase { .. }, Key::ClassificationRegion { .. }) => {
                Ordering::Greater
            }
            (
                Key::ClassificationAuthorizedCase { case_id: left },
                Key::ClassificationAuthorizedCase { case_id: right },
            ) => left.cmp(right),
            (Key::ClassificationRegion { .. } | Key::ClassificationAuthorizedCase { .. }, _) => {
                Ordering::Less
            }
            (_, Key::ClassificationRegion { .. } | Key::ClassificationAuthorizedCase { .. }) => {
                Ordering::Greater
            }
            _ => {
                let left_chunk = self.chunk_ordinal().expect("non-root key has a chunk");
                let right_chunk = other.chunk_ordinal().expect("non-root key has a chunk");
                left_chunk
                    .cmp(&right_chunk)
                    .then_with(|| match (self, other) {
                        (Key::Chunk { .. }, Key::Chunk { .. }) => Ordering::Equal,
                        (Key::Chunk { .. }, _) => Ordering::Less,
                        (_, Key::Chunk { .. }) => Ordering::Greater,
                        (
                            Key::Region {
                                run_ordinal: left, ..
                            },
                            Key::Region {
                                run_ordinal: right, ..
                            },
                        ) => left.cmp(right),
                        (Key::Region { .. }, _) => Ordering::Less,
                        (_, Key::Region { .. }) => Ordering::Greater,
                        (
                            Key::SelectedMaterialization {
                                run_ordinal: left, ..
                            },
                            Key::SelectedMaterialization {
                                run_ordinal: right, ..
                            },
                        ) => left.cmp(right),
                        (Key::SelectedMaterialization { .. }, _) => Ordering::Less,
                        (_, Key::SelectedMaterialization { .. }) => Ordering::Greater,
                        (
                            Key::AuthorizedCase {
                                run_ordinal: left_run,
                                case_id: left_case,
                                ..
                            },
                            Key::AuthorizedCase {
                                run_ordinal: right_run,
                                case_id: right_case,
                                ..
                            },
                        ) => left_run
                            .cmp(right_run)
                            .then_with(|| left_case.cmp(right_case)),
                        _ => Ordering::Equal,
                    })
            }
        }
    }
}

impl PartialOrd for RelationalCaseSupportRecordKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl RelationalCaseSupportRecordKey {
    const fn chunk_ordinal(self) -> Option<u128> {
        match self {
            Self::Root
            | Self::ClassificationRegion { .. }
            | Self::ClassificationAuthorizedCase { .. } => None,
            Self::Chunk { chunk_ordinal }
            | Self::Region { chunk_ordinal, .. }
            | Self::SelectedMaterialization { chunk_ordinal, .. }
            | Self::AuthorizedCase { chunk_ordinal, .. } => Some(chunk_ordinal),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportClassificationAuthority {
    ConcreteSweep(RelationalClassifiedChunkArtifactId),
    CertifiedRegion([u8; 32]),
}

impl RelationalCaseSupportClassificationAuthority {
    pub(crate) const fn id(self) -> [u8; 32] {
        match self {
            Self::ConcreteSweep(id) => id.bytes(),
            Self::CertifiedRegion(id) => id,
        }
    }

    pub(crate) const fn kind(self) -> &'static str {
        match self {
            Self::ConcreteSweep(_) => "concrete_sweep",
            Self::CertifiedRegion(_) => "regional_certificate",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportRegionAuthority {
    ConcreteRun(RelationalClassifiedRunId),
    CertifiedRegion([u8; 32]),
}

impl RelationalCaseSupportRegionAuthority {
    pub(crate) const fn id(self) -> [u8; 32] {
        match self {
            Self::ConcreteRun(id) => id.bytes(),
            Self::CertifiedRegion(id) => id,
        }
    }

    pub(crate) const fn kind(self) -> &'static str {
        match self {
            Self::ConcreteRun(_) => "concrete_run",
            Self::CertifiedRegion(_) => "regional_certificate",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportOutcome {
    Rejected,
    AdmittedNotSelected,
    AdmittedSelected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseIdPublicationAuthorization {
    authority: RelationalCaseIdPublicationAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseIdPublicationAuthority {
    ResultView(ViewId),
}

impl RelationalCaseIdPublicationAuthorization {
    pub(crate) const fn from_checked_result_view(view_id: ViewId) -> Self {
        Self {
            authority: RelationalCaseIdPublicationAuthority::ResultView(view_id),
        }
    }

    pub(crate) const fn authority(self) -> RelationalCaseIdPublicationAuthority {
        self.authority
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSupportClosureAuthority {
    certified_root_case_count: u128,
    support_evidence_root: SupportEvidenceRoot,
    selected_question_seal_id: RelationalSelectedQuestionSealId,
    question_id: QuestionId,
    exact_selected_case_count: u128,
}

impl RelationalCaseSupportClosureAuthority {
    pub(crate) fn from_authenticated_certified_support(
        support_catalog_is_sealed: bool,
        certified_root_case_count: u128,
        support_evidence_root: SupportEvidenceRoot,
        selected_question: RelationalSelectedQuestionSeal,
    ) -> Result<Self, RelationalCaseSupportProjectionError> {
        if !support_catalog_is_sealed {
            return Err(RelationalCaseSupportProjectionError::SupportCatalogOpen);
        }
        selected_question
            .validate_identity()
            .map_err(|_| RelationalCaseSupportProjectionError::InvalidSelectedQuestionSeal)?;
        let exact_selected_case_count = match selected_question.authority() {
            RelationalSelectedPopulationAuthority::CertifiedSupport {
                exact_cardinality, ..
            } => exact_cardinality,
            RelationalSelectedPopulationAuthority::ExtensionalQuestion { .. } => {
                return Err(
                    RelationalCaseSupportProjectionError::SelectedQuestionIsNotSupportCertified,
                );
            }
        };
        if selected_question.result_input_seal().coverage().row_count() != exact_selected_case_count
        {
            return Err(RelationalCaseSupportProjectionError::SelectedQuestionCoverageMismatch);
        }
        Ok(Self {
            certified_root_case_count,
            support_evidence_root,
            selected_question_seal_id: selected_question.id(),
            question_id: selected_question.question_id(),
            exact_selected_case_count,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportCount {
    LowerBound(u128),
    Exact(u128),
}

impl RelationalCaseSupportCount {
    pub(crate) const fn value(self) -> u128 {
        match self {
            Self::LowerBound(value) | Self::Exact(value) => value,
        }
    }

    pub(crate) const fn is_exact(self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportOpenReason {
    AwaitingClassifiedFragments {
        missing_chunk_count: u128,
        first_missing_chunk_ordinal: u128,
    },
    AwaitingSelectedMaterializations {
        missing_materialization_count: u128,
        first_chunk_ordinal: u128,
        first_run_ordinal: u16,
    },
    AwaitingClosureAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSupportClosureMetadata {
    pub(crate) projection_id: RelationalCaseSupportProjectionId,
    pub(crate) active_set_root: RelationalCaseSupportActiveSetRoot,
    pub(crate) partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
    pub(crate) support_evidence_root: SupportEvidenceRoot,
    pub(crate) selected_question_seal_id: RelationalSelectedQuestionSealId,
    pub(crate) exact_logical_case_count: u128,
    pub(crate) exact_selected_case_count: u128,
    pub(crate) classified_chunk_count: u128,
    pub(crate) region_count: u128,
    pub(crate) selected_materialization_count: u128,
    pub(crate) authorized_case_record_count: u128,
    pub(crate) active_record_count: u128,
    pub(crate) data_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionFrontier {
    Open(RelationalCaseSupportOpenReason),
    Exact(RelationalCaseSupportClosureMetadata),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSupportProjectionMetadata {
    pub(crate) projection_id: RelationalCaseSupportProjectionId,
    pub(crate) frontier: RelationalCaseSupportProjectionFrontier,
    pub(crate) exact_logical_case_count: u128,
    pub(crate) classified_case_count: RelationalCaseSupportCount,
    pub(crate) selected_case_count: RelationalCaseSupportCount,
    pub(crate) materialized_selected_case_count: RelationalCaseSupportCount,
    pub(crate) planned_chunk_count: u128,
    pub(crate) classified_chunk_count: u128,
    pub(crate) published_chunk_count: u128,
    pub(crate) published_region_count: u128,
    pub(crate) published_selected_materialization_count: u128,
    pub(crate) authorized_case_record_count: u128,
    pub(crate) active_record_count: u128,
    pub(crate) available_source_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportRow {
    Root {
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        support_plan_root: RelationalSupportPlanRoot,
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        exact_logical_case_count: u128,
        planned_chunk_count: u128,
        case_id_authority: Option<RelationalCaseIdPublicationAuthority>,
    },
    Chunk {
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        classification_authority: RelationalCaseSupportClassificationAuthority,
        chunk_ordinal: u128,
        exact_case_count: u128,
        rejected_case_count: u128,
        admitted_not_selected_case_count: u128,
        admitted_selected_case_count: u128,
        region_count: u128,
    },
    Region {
        classification_authority: RelationalCaseSupportClassificationAuthority,
        region_authority: RelationalCaseSupportRegionAuthority,
        chunk_ordinal: u128,
        run_ordinal: u16,
        exact_case_count: u128,
        outcome: RelationalCaseSupportOutcome,
        correlated_starter_region_id: Option<RelationalStarterRegionId>,
    },
    SelectedMaterialization {
        chunk_ordinal: u128,
        run_ordinal: u16,
        run_id: RelationalClassifiedRunId,
        artifact_id: RelationalSelectedRunMaterializationArtifactId,
        exact_case_count: u128,
        materialized_cases_root: [u8; 32],
    },
    AuthorizedCase {
        chunk_ordinal: u128,
        run_ordinal: u16,
        materialization_artifact_id: RelationalSelectedRunMaterializationArtifactId,
        case_id: RelationalCaseId,
        authority: RelationalCaseIdPublicationAuthority,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionRecord {
    Add {
        key: RelationalCaseSupportRecordKey,
        row_hash: RelationalCaseSupportRowHash,
        row: RelationalCaseSupportRow,
    },
    Seal(RelationalCaseSupportClosureMetadata),
}

#[derive(Clone, Copy)]
enum DiscoveryPackage<'a> {
    Classified {
        fragment: &'a RelationalClassifiedSupportFragment,
        record_end: u128,
    },
    SelectedMaterialization {
        chunk: &'a RelationalClassifiedChunkArtifact,
        run: &'a RelationalClassifiedRunDescriptor,
        materialization: &'a RelationalSelectedRunMaterializationArtifact,
        record_end: u128,
    },
}

impl DiscoveryPackage<'_> {
    const fn record_end(self) -> u128 {
        match self {
            Self::Classified { record_end, .. }
            | Self::SelectedMaterialization { record_end, .. } => record_end,
        }
    }
}

pub(crate) struct RelationalCaseSupportProjection<'a> {
    projection_id: RelationalCaseSupportProjectionId,
    question_id: QuestionId,
    partition: &'a RelationalCaseChunkPartitionArtifact,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    packages: Box<[DiscoveryPackage<'a>]>,
    closure: Option<RelationalCaseSupportClosureMetadata>,
    metadata: RelationalCaseSupportProjectionMetadata,
}

impl<'a> RelationalCaseSupportProjection<'a> {
    pub(crate) const fn projection_id(&self) -> RelationalCaseSupportProjectionId {
        self.projection_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn partition_artifact_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.partition.id()
    }

    pub(crate) const fn metadata(&self) -> RelationalCaseSupportProjectionMetadata {
        self.metadata
    }

    pub(crate) const fn available_source_record_count(&self) -> u128 {
        self.metadata.available_source_record_count
    }

    pub(crate) fn record_at(
        &self,
        source_ordinal: u128,
    ) -> Result<Option<RelationalCaseSupportProjectionRecord>, RelationalCaseSupportProjectionError>
    {
        if source_ordinal >= self.metadata.available_source_record_count {
            return Ok(None);
        }
        if source_ordinal == 0 {
            return self
                .add_record(RelationalCaseSupportRecordKey::Root, self.root_row())
                .map(Some);
        }
        let data_record_count = self
            .packages
            .last()
            .map_or(1, |package| package.record_end());
        if source_ordinal == data_record_count {
            return Ok(self
                .closure
                .map(RelationalCaseSupportProjectionRecord::Seal));
        }
        let package_index = self
            .packages
            .partition_point(|package| package.record_end() <= source_ordinal);
        let package = self
            .packages
            .get(package_index)
            .copied()
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
        let package_start = package_index
            .checked_sub(1)
            .map_or(1, |prior| self.packages[prior].record_end());
        let relative = source_ordinal
            .checked_sub(package_start)
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
        let (key, row) = package_row_at(self.question_id, self.authorization, package, relative)?;
        self.add_record(key, row).map(Some)
    }

    fn root_row(&self) -> RelationalCaseSupportRow {
        RelationalCaseSupportRow::Root {
            relation_id: self.partition.relation_id(),
            admission_id: self.partition.admission_id(),
            question_id: self.question_id,
            support_plan_root: self.partition.plan_root(),
            partition_artifact_id: self.partition.id(),
            exact_logical_case_count: self.metadata.exact_logical_case_count,
            planned_chunk_count: self.metadata.planned_chunk_count,
            case_id_authority: self.authorization.map(|value| value.authority()),
        }
    }

    fn add_record(
        &self,
        key: RelationalCaseSupportRecordKey,
        row: RelationalCaseSupportRow,
    ) -> Result<RelationalCaseSupportProjectionRecord, RelationalCaseSupportProjectionError> {
        validate_key_row(key, row)?;
        Ok(RelationalCaseSupportProjectionRecord::Add {
            key,
            row_hash: relational_case_support_row_hash(self.projection_id, key, row)?,
            row,
        })
    }
}

/// Derive the sparse projection from one authenticated journal observation.
/// The scheduler view supplies both the immutable first-arrival sequence and
/// the canonical sparse catalogs against which that sequence is checked.
pub(crate) fn derive_relational_case_support_projection<'a>(
    question_id: QuestionId,
    verified_partition: &'a VerifiedRelationalCaseChunkPartition,
    scheduler: RelationalSchedulerView<'a>,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    closure_authority: Option<RelationalCaseSupportClosureAuthority>,
) -> Result<RelationalCaseSupportProjection<'a>, RelationalCaseSupportProjectionError> {
    let partition = verified_partition.artifact();
    let retained_partition = scheduler
        .verified_case_chunk_partition()
        .map_err(journal_state_error)?
        .ok_or(RelationalCaseSupportProjectionError::PartitionAuthorityMismatch)?;
    if retained_partition.artifact() != partition {
        return Err(RelationalCaseSupportProjectionError::PartitionAuthorityMismatch);
    }
    if partition
        .question_ids()
        .binary_search(&question_id)
        .is_err()
    {
        return Err(RelationalCaseSupportProjectionError::QuestionNotInPartition { question_id });
    }
    let planned_chunk_count = usize_to_u128(partition.chunks().len())?;
    let exact_logical_case_count = partition
        .interval_end_exclusive()
        .checked_sub(partition.interval_start())
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
    let projection_id = derive_relational_case_support_projection_id(
        partition.relation_id(),
        partition.admission_id(),
        question_id,
        partition.plan_root().bytes(),
        RelationalCaseSupportProjectionBasis::Partition(partition.id()),
        authorization,
    );

    let mut classified_case_count = 0_u128;
    let mut selected_case_count = 0_u128;
    let mut materialized_selected_case_count = 0_u128;
    let mut classified_chunk_count = 0_u128;
    let mut classified_region_count = 0_u128;
    let mut selected_materialization_count = 0_u128;
    let mut first_missing_chunk = None;
    let mut missing_chunk_count = 0_u128;
    let mut first_missing_materialization = None;
    let mut missing_materialization_count = 0_u128;
    let mut expected_materializations = BTreeSet::new();

    for (chunk_index, _) in partition.chunks().iter().enumerate() {
        let Some(fragment) = scheduler
            .classified_support_fragment_at(chunk_index)
            .map_err(journal_state_error)?
        else {
            missing_chunk_count = checked_add(missing_chunk_count, 1)?;
            first_missing_chunk.get_or_insert(usize_to_u128(chunk_index)?);
            continue;
        };
        validate_classified_fragment(question_id, partition, chunk_index, fragment)?;
        classified_chunk_count = checked_add(classified_chunk_count, 1)?;
        classified_case_count = checked_add(classified_case_count, fragment.exact_case_count())?;
        selected_case_count = checked_add(
            selected_case_count,
            fragment_admitted_selected_count(question_id, fragment)?,
        )?;
        classified_region_count = checked_add(
            classified_region_count,
            match fragment {
                RelationalClassifiedSupportFragment::Concrete(chunk) => {
                    usize_to_u128(chunk.runs().len())?
                }
                RelationalClassifiedSupportFragment::CertifiedZeroSelected(_) => 1,
            },
        )?;
        let RelationalClassifiedSupportFragment::Concrete(chunk) = fragment else {
            continue;
        };
        for run in chunk.runs() {
            if scalar_outcome(question_id, chunk, run)?
                != RelationalCaseSupportOutcome::AdmittedSelected
            {
                continue;
            }
            let coordinate = (chunk.chunk_ordinal(), run.ordinal());
            let Some(materialization) = scheduler
                .selected_run_materialization(run.cell_id())
                .map_err(journal_state_error)?
            else {
                missing_materialization_count = checked_add(missing_materialization_count, 1)?;
                first_missing_materialization.get_or_insert(coordinate);
                continue;
            };
            validate_materialization(question_id, partition, chunk, run, materialization)?;
            if !expected_materializations.insert(coordinate) {
                return Err(RelationalCaseSupportProjectionError::DuplicateLogicalSlot);
            }
            selected_materialization_count = checked_add(selected_materialization_count, 1)?;
            materialized_selected_case_count = checked_add(
                materialized_selected_case_count,
                materialization.materialized_case_count(),
            )?;
        }
    }
    if classified_case_count > exact_logical_case_count {
        return Err(RelationalCaseSupportProjectionError::ClassificationExceedsRoot);
    }

    let mut packages = Vec::new();
    packages
        .try_reserve_exact(scheduler.case_support_discovery_event_count())
        .map_err(|_| {
            RelationalCaseSupportProjectionError::AllocationFailed(
                "case-support discovery package index",
            )
        })?;
    let mut record_end = 1_u128;
    let mut published_chunks = BTreeSet::new();
    let mut published_materializations = BTreeSet::new();
    let mut published_region_count = 0_u128;
    let mut authorized_case_record_count = 0_u128;

    for event_ordinal in 0..scheduler.case_support_discovery_event_count() {
        let event = scheduler
            .case_support_discovery_event_at(event_ordinal)
            .map_err(journal_state_error)?
            .ok_or(
                RelationalCaseSupportProjectionError::DiscoveryIndexMismatch { event_ordinal },
            )?;
        match event {
            RelationalCaseSupportDiscoveryEvent::ClassifiedFragment {
                chunk_ordinal,
                fragment,
            } => {
                let chunk_index = usize::try_from(chunk_ordinal)
                    .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
                validate_classified_fragment(question_id, partition, chunk_index, fragment)?;
                if !published_chunks.insert(chunk_ordinal) {
                    continue;
                }
                let _ = chunk_row(question_id, partition, fragment)?;
                let region_count = fragment_region_count(fragment)?;
                for region_index in 0..region_count {
                    let _ = fragment_region_row(question_id, fragment, region_index)?;
                }
                let package_count = checked_add(1, usize_to_u128(region_count)?)?;
                record_end = checked_add(record_end, package_count)?;
                published_region_count =
                    checked_add(published_region_count, usize_to_u128(region_count)?)?;
                packages.push(DiscoveryPackage::Classified {
                    fragment,
                    record_end,
                });
            }
            RelationalCaseSupportDiscoveryEvent::SelectedRunMaterialization {
                chunk_ordinal,
                run_ordinal,
                materialization,
            } => {
                let chunk_index = usize::try_from(chunk_ordinal)
                    .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
                let fragment = scheduler
                    .classified_support_fragment_at(chunk_index)
                    .map_err(journal_state_error)?
                    .ok_or(
                        RelationalCaseSupportProjectionError::MaterializationBeforeChunk {
                            chunk_ordinal,
                            run_ordinal,
                        },
                    )?;
                let chunk = fragment.concrete().ok_or(
                    RelationalCaseSupportProjectionError::MaterializationBeforeChunk {
                        chunk_ordinal,
                        run_ordinal,
                    },
                )?;
                let run = chunk
                    .runs()
                    .get(usize::from(run_ordinal))
                    .filter(|run| run.ordinal() == run_ordinal)
                    .ok_or(RelationalCaseSupportProjectionError::SelectedMaterializationScopeMismatch {
                        chunk_ordinal,
                        run_ordinal,
                    })?;
                validate_materialization_scope(partition, chunk, run, materialization)?;
                if !materialization.contains_question(question_id) {
                    continue;
                }
                if scalar_outcome(question_id, chunk, run)?
                    != RelationalCaseSupportOutcome::AdmittedSelected
                {
                    return Err(
                        RelationalCaseSupportProjectionError::SelectedMaterializationOutcomeMismatch {
                            chunk_ordinal,
                            run_ordinal,
                        },
                    );
                }
                validate_materialization(question_id, partition, chunk, run, materialization)?;
                let coordinate = (chunk_ordinal, run_ordinal);
                if !published_materializations.insert(coordinate) {
                    continue;
                }
                let mut package_count = 1_u128;
                if authorization.is_some() {
                    let case_count = usize_to_u128(materialization.cases().len())?;
                    package_count = checked_add(package_count, case_count)?;
                    authorized_case_record_count =
                        checked_add(authorized_case_record_count, case_count)?;
                }
                record_end = checked_add(record_end, package_count)?;
                packages.push(DiscoveryPackage::SelectedMaterialization {
                    chunk,
                    run,
                    materialization,
                    record_end,
                });
            }
        }
    }

    if published_chunks.len() != usize::try_from(classified_chunk_count).unwrap_or(usize::MAX) {
        return Err(
            RelationalCaseSupportProjectionError::DiscoveryCatalogMismatch("classified fragments"),
        );
    }
    if published_materializations != expected_materializations {
        return Err(
            RelationalCaseSupportProjectionError::DiscoveryCatalogMismatch(
                "selected materializations",
            ),
        );
    }
    if published_region_count != classified_region_count {
        return Err(
            RelationalCaseSupportProjectionError::DiscoveryCatalogMismatch(
                "classification regions",
            ),
        );
    }
    let active_record_count = record_end;

    let classification_complete = missing_chunk_count == 0;
    let materialization_complete = classification_complete && missing_materialization_count == 0;
    let classified_count = if classification_complete {
        RelationalCaseSupportCount::Exact(classified_case_count)
    } else {
        RelationalCaseSupportCount::LowerBound(classified_case_count)
    };
    let selected_count = if classification_complete {
        RelationalCaseSupportCount::Exact(selected_case_count)
    } else {
        RelationalCaseSupportCount::LowerBound(selected_case_count)
    };
    let materialized_count = if materialization_complete {
        RelationalCaseSupportCount::Exact(materialized_selected_case_count)
    } else {
        RelationalCaseSupportCount::LowerBound(materialized_selected_case_count)
    };

    let closure = match closure_authority {
        Some(authority) => {
            if !classification_complete || !materialization_complete {
                return Err(RelationalCaseSupportProjectionError::PrematureClosure);
            }
            if authority.question_id != question_id {
                return Err(RelationalCaseSupportProjectionError::ClosureQuestionMismatch);
            }
            if authority.certified_root_case_count != exact_logical_case_count
                || classified_case_count != exact_logical_case_count
            {
                return Err(
                    RelationalCaseSupportProjectionError::ClosureRootCountMismatch {
                        expected: exact_logical_case_count,
                        certified: authority.certified_root_case_count,
                        classified: classified_case_count,
                    },
                );
            }
            if authority.exact_selected_case_count != selected_case_count
                || materialized_selected_case_count != selected_case_count
            {
                return Err(
                    RelationalCaseSupportProjectionError::ClosureSelectedCountMismatch {
                        classified: selected_case_count,
                        materialized: materialized_selected_case_count,
                        sealed: authority.exact_selected_case_count,
                    },
                );
            }
            if classified_chunk_count != planned_chunk_count
                || selected_materialization_count != usize_to_u128(expected_materializations.len())?
            {
                return Err(RelationalCaseSupportProjectionError::PrematureClosure);
            }
            Some(RelationalCaseSupportClosureMetadata {
                projection_id,
                active_set_root: derive_partition_active_set_root(
                    projection_id,
                    question_id,
                    partition,
                    scheduler,
                    authorization,
                    active_record_count,
                )?,
                partition_artifact_id: partition.id(),
                support_evidence_root: authority.support_evidence_root,
                selected_question_seal_id: authority.selected_question_seal_id,
                exact_logical_case_count,
                exact_selected_case_count: selected_case_count,
                classified_chunk_count,
                region_count: classified_region_count,
                selected_materialization_count,
                authorized_case_record_count,
                active_record_count,
                data_record_count: record_end,
            })
        }
        None => None,
    };

    let frontier = if let Some(closure) = closure {
        RelationalCaseSupportProjectionFrontier::Exact(closure)
    } else if let Some(first_missing_chunk_ordinal) = first_missing_chunk {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingClassifiedFragments {
                missing_chunk_count,
                first_missing_chunk_ordinal,
            },
        )
    } else if let Some((first_chunk_ordinal, first_run_ordinal)) = first_missing_materialization {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingSelectedMaterializations {
                missing_materialization_count,
                first_chunk_ordinal,
                first_run_ordinal,
            },
        )
    } else {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingClosureAuthority,
        )
    };
    let available_source_record_count =
        checked_add(record_end, if closure.is_some() { 1 } else { 0 })?;
    let metadata = RelationalCaseSupportProjectionMetadata {
        projection_id,
        frontier,
        exact_logical_case_count,
        classified_case_count: classified_count,
        selected_case_count: selected_count,
        materialized_selected_case_count: materialized_count,
        planned_chunk_count,
        classified_chunk_count,
        published_chunk_count: usize_to_u128(published_chunks.len())?,
        published_region_count,
        published_selected_materialization_count: usize_to_u128(published_materializations.len())?,
        authorized_case_record_count,
        active_record_count,
        available_source_record_count,
    };
    Ok(RelationalCaseSupportProjection {
        projection_id,
        question_id,
        partition,
        authorization,
        packages: packages.into_boxed_slice(),
        closure,
        metadata,
    })
}

fn package_row_at(
    question_id: QuestionId,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    package: DiscoveryPackage<'_>,
    relative: u128,
) -> Result<
    (RelationalCaseSupportRecordKey, RelationalCaseSupportRow),
    RelationalCaseSupportProjectionError,
> {
    match package {
        DiscoveryPackage::Classified { fragment, .. } => {
            if relative == 0 {
                return chunk_row_without_partition(question_id, fragment);
            }
            let region_index = usize::try_from(relative - 1)
                .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
            fragment_region_row(question_id, fragment, region_index)
        }
        DiscoveryPackage::SelectedMaterialization {
            chunk,
            run,
            materialization,
            ..
        } => {
            if relative == 0 {
                return Ok(materialization_row(chunk, run, materialization));
            }
            let authorization =
                authorization.ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
            let case_index = usize::try_from(relative - 1)
                .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
            let case = materialization
                .cases()
                .get(case_index)
                .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
            Ok((
                RelationalCaseSupportRecordKey::AuthorizedCase {
                    chunk_ordinal: chunk.chunk_ordinal(),
                    run_ordinal: run.ordinal(),
                    case_id: case.case_id(),
                },
                RelationalCaseSupportRow::AuthorizedCase {
                    chunk_ordinal: chunk.chunk_ordinal(),
                    run_ordinal: run.ordinal(),
                    materialization_artifact_id: materialization.id(),
                    case_id: case.case_id(),
                    authority: authorization.authority(),
                },
            ))
        }
    }
}

fn chunk_row(
    question_id: QuestionId,
    partition: &RelationalCaseChunkPartitionArtifact,
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<
    (RelationalCaseSupportRecordKey, RelationalCaseSupportRow),
    RelationalCaseSupportProjectionError,
> {
    let (key, row) = chunk_row_without_partition(question_id, fragment)?;
    let RelationalCaseSupportRow::Chunk {
        partition_artifact_id,
        ..
    } = row
    else {
        return Err(RelationalCaseSupportProjectionError::KeyRowMismatch);
    };
    if partition_artifact_id != partition.id() {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkScopeMismatch {
                chunk_ordinal: fragment.chunk_ordinal(),
            },
        );
    }
    Ok((key, row))
}

fn chunk_row_without_partition(
    question_id: QuestionId,
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<
    (RelationalCaseSupportRecordKey, RelationalCaseSupportRow),
    RelationalCaseSupportProjectionError,
> {
    let (partition_artifact_id, classification_authority) = match fragment {
        RelationalClassifiedSupportFragment::Concrete(chunk) => (
            chunk.chunk_partition_id(),
            RelationalCaseSupportClassificationAuthority::ConcreteSweep(chunk.id()),
        ),
        RelationalClassifiedSupportFragment::CertifiedZeroSelected(certificate) => {
            let RelationalRegionProofSubject::CanonicalChunk {
                partition_artifact_id,
                ..
            } = certificate.subject()
            else {
                return Err(
                    RelationalCaseSupportProjectionError::ClassifiedChunkScopeMismatch {
                        chunk_ordinal: fragment.chunk_ordinal(),
                    },
                );
            };
            (
                partition_artifact_id,
                RelationalCaseSupportClassificationAuthority::CertifiedRegion(
                    certificate.certificate_id(),
                ),
            )
        }
    };
    let chunk_ordinal = fragment.chunk_ordinal();
    Ok((
        RelationalCaseSupportRecordKey::Chunk { chunk_ordinal },
        RelationalCaseSupportRow::Chunk {
            partition_artifact_id,
            classification_authority,
            chunk_ordinal,
            exact_case_count: fragment.exact_case_count(),
            rejected_case_count: fragment.rejected_count(),
            admitted_not_selected_case_count: fragment_admitted_not_selected_count(
                question_id,
                fragment,
            )?,
            admitted_selected_case_count: fragment_admitted_selected_count(question_id, fragment)?,
            region_count: usize_to_u128(fragment_region_count(fragment)?)?,
        },
    ))
}

fn fragment_region_count(
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<usize, RelationalCaseSupportProjectionError> {
    Ok(match fragment {
        RelationalClassifiedSupportFragment::Concrete(chunk) => chunk.runs().len(),
        RelationalClassifiedSupportFragment::CertifiedZeroSelected(_) => 1,
    })
}

fn fragment_region_row(
    question_id: QuestionId,
    fragment: &RelationalClassifiedSupportFragment,
    region_index: usize,
) -> Result<
    (RelationalCaseSupportRecordKey, RelationalCaseSupportRow),
    RelationalCaseSupportProjectionError,
> {
    let chunk_ordinal = fragment.chunk_ordinal();
    match fragment {
        RelationalClassifiedSupportFragment::Concrete(chunk) => {
            let run = chunk
                .runs()
                .get(region_index)
                .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
            let run_ordinal = run.ordinal();
            Ok((
                RelationalCaseSupportRecordKey::Region {
                    chunk_ordinal,
                    run_ordinal,
                },
                RelationalCaseSupportRow::Region {
                    classification_authority:
                        RelationalCaseSupportClassificationAuthority::ConcreteSweep(chunk.id()),
                    region_authority: RelationalCaseSupportRegionAuthority::ConcreteRun(run.id()),
                    chunk_ordinal,
                    run_ordinal,
                    exact_case_count: run.cardinality(),
                    outcome: scalar_outcome(question_id, chunk, run)?,
                    correlated_starter_region_id: None,
                },
            ))
        }
        RelationalClassifiedSupportFragment::CertifiedZeroSelected(certificate) => {
            if region_index != 0 {
                return Err(RelationalCaseSupportProjectionError::OrdinalIndexMismatch);
            }
            let outcome = match certificate.conclusion() {
                RelationalCertifiedRegionConclusion::Rejected => {
                    RelationalCaseSupportOutcome::Rejected
                }
                RelationalCertifiedRegionConclusion::AdmittedNotSelected => {
                    RelationalCaseSupportOutcome::AdmittedNotSelected
                }
            };
            Ok((
                RelationalCaseSupportRecordKey::Region {
                    chunk_ordinal,
                    run_ordinal: 0,
                },
                RelationalCaseSupportRow::Region {
                    classification_authority:
                        RelationalCaseSupportClassificationAuthority::CertifiedRegion(
                            certificate.certificate_id(),
                        ),
                    region_authority: RelationalCaseSupportRegionAuthority::CertifiedRegion(
                        certificate.certificate_id(),
                    ),
                    chunk_ordinal,
                    run_ordinal: 0,
                    exact_case_count: certificate.case_cardinality(),
                    outcome,
                    correlated_starter_region_id: Some(certificate.starter_region_id()),
                },
            ))
        }
    }
}

fn materialization_row(
    chunk: &RelationalClassifiedChunkArtifact,
    run: &RelationalClassifiedRunDescriptor,
    materialization: &RelationalSelectedRunMaterializationArtifact,
) -> (RelationalCaseSupportRecordKey, RelationalCaseSupportRow) {
    let chunk_ordinal = chunk.chunk_ordinal();
    let run_ordinal = run.ordinal();
    (
        RelationalCaseSupportRecordKey::SelectedMaterialization {
            chunk_ordinal,
            run_ordinal,
        },
        RelationalCaseSupportRow::SelectedMaterialization {
            chunk_ordinal,
            run_ordinal,
            run_id: run.id(),
            artifact_id: materialization.id(),
            exact_case_count: materialization.materialized_case_count(),
            materialized_cases_root: materialization.materialized_cases_root(),
        },
    )
}

fn scalar_outcome(
    question_id: QuestionId,
    chunk: &RelationalClassifiedChunkArtifact,
    run: &RelationalClassifiedRunDescriptor,
) -> Result<RelationalCaseSupportOutcome, RelationalCaseSupportProjectionError> {
    let question_index = chunk.question_index(question_id).ok_or(
        RelationalCaseSupportProjectionError::QuestionNotInClassifiedChunk {
            question_id,
            chunk_ordinal: chunk.chunk_ordinal(),
        },
    )?;
    match run.outcome() {
        RelationalClassifiedCaseOutcome::Rejected => Ok(RelationalCaseSupportOutcome::Rejected),
        RelationalClassifiedCaseOutcome::Admitted(_) => {
            match run.outcome().selection(question_index) {
                Some(SelectionDecision::NotSelected) => {
                    Ok(RelationalCaseSupportOutcome::AdmittedNotSelected)
                }
                Some(SelectionDecision::Selected) => {
                    Ok(RelationalCaseSupportOutcome::AdmittedSelected)
                }
                None => Err(
                    RelationalCaseSupportProjectionError::ClassifiedRunOutcomeMismatch {
                        chunk_ordinal: chunk.chunk_ordinal(),
                        run_ordinal: run.ordinal(),
                    },
                ),
            }
        }
    }
}

fn fragment_admitted_selected_count(
    question_id: QuestionId,
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<u128, RelationalCaseSupportProjectionError> {
    match fragment {
        RelationalClassifiedSupportFragment::Concrete(chunk) => {
            chunk.admitted_selected_count(question_id).ok_or(
                RelationalCaseSupportProjectionError::QuestionNotInClassifiedChunk {
                    question_id,
                    chunk_ordinal: chunk.chunk_ordinal(),
                },
            )
        }
        RelationalClassifiedSupportFragment::CertifiedZeroSelected(_) => Ok(0),
    }
}

fn fragment_admitted_not_selected_count(
    question_id: QuestionId,
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<u128, RelationalCaseSupportProjectionError> {
    match fragment {
        RelationalClassifiedSupportFragment::Concrete(chunk) => {
            chunk.admitted_not_selected_count(question_id).ok_or(
                RelationalCaseSupportProjectionError::QuestionNotInClassifiedChunk {
                    question_id,
                    chunk_ordinal: chunk.chunk_ordinal(),
                },
            )
        }
        RelationalClassifiedSupportFragment::CertifiedZeroSelected(certificate) => {
            Ok(match certificate.conclusion() {
                RelationalCertifiedRegionConclusion::Rejected => 0,
                RelationalCertifiedRegionConclusion::AdmittedNotSelected => {
                    certificate.case_cardinality()
                }
            })
        }
    }
}

fn validate_classified_fragment(
    question_id: QuestionId,
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk_index: usize,
    fragment: &RelationalClassifiedSupportFragment,
) -> Result<(), RelationalCaseSupportProjectionError> {
    let expected_ordinal = usize_to_u128(chunk_index)?;
    let descriptor = partition.chunks().get(chunk_index).ok_or(
        RelationalCaseSupportProjectionError::UnexpectedClassifiedChunk {
            chunk_ordinal: fragment.chunk_ordinal(),
        },
    )?;
    if fragment.chunk_ordinal() != expected_ordinal {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkOrderMismatch {
                expected: expected_ordinal,
                actual: fragment.chunk_ordinal(),
            },
        );
    }
    let scope_matches = fragment.chunk_id() == Some(descriptor.id())
        && fragment.chunk_cell_id() == descriptor.cell_id()
        && fragment.interval_start() == descriptor.interval_start()
        && fragment.interval_end_exclusive() == descriptor.interval_end_exclusive()
        && fragment.exact_case_count() == descriptor.cardinality()
        && match fragment {
            RelationalClassifiedSupportFragment::Concrete(chunk) => {
                chunk.plan_root() == partition.plan_root()
                    && chunk.relation_id() == partition.relation_id()
                    && chunk.admission_id() == partition.admission_id()
                    && chunk.question_ids() == partition.question_ids()
                    && chunk.question_index(question_id).is_some()
                    && chunk.chunk_partition_id() == partition.id()
            }
            RelationalClassifiedSupportFragment::CertifiedZeroSelected(certificate) => {
                let [partition_question_id] = partition.question_ids() else {
                    return Err(
                        RelationalCaseSupportProjectionError::RegionalCertificateRequiresOneQuestion,
                    );
                };
                certificate.plan_root() == partition.plan_root()
                    && certificate.relation_id() == partition.relation_id()
                    && certificate.admission_id() == partition.admission_id()
                    && certificate.question_id() == *partition_question_id
                    && certificate.question_id() == question_id
                    && matches!(
                        certificate.subject(),
                        RelationalRegionProofSubject::CanonicalChunk {
                            partition_artifact_id,
                            ..
                        } if partition_artifact_id == partition.id()
                    )
            }
        };
    if !scope_matches {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkScopeMismatch {
                chunk_ordinal: expected_ordinal,
            },
        );
    }
    let admitted_not_selected = fragment_admitted_not_selected_count(question_id, fragment)?;
    let admitted_selected = fragment_admitted_selected_count(question_id, fragment)?;
    let classified = fragment
        .rejected_count()
        .checked_add(admitted_not_selected)
        .and_then(|count| count.checked_add(admitted_selected))
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
    if classified != fragment.exact_case_count() {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkCountMismatch {
                chunk_ordinal: expected_ordinal,
            },
        );
    }
    let RelationalClassifiedSupportFragment::Concrete(chunk) = fragment else {
        return Ok(());
    };
    for (run_index, run) in chunk.runs().iter().enumerate() {
        let expected = u16::try_from(run_index)
            .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
        if run.ordinal() != expected {
            return Err(
                RelationalCaseSupportProjectionError::ClassifiedRunOrderMismatch {
                    chunk_ordinal: expected_ordinal,
                    expected,
                    actual: run.ordinal(),
                },
            );
        }
    }
    Ok(())
}

fn validate_materialization_scope(
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk: &RelationalClassifiedChunkArtifact,
    run: &RelationalClassifiedRunDescriptor,
    materialization: &RelationalSelectedRunMaterializationArtifact,
) -> Result<(), RelationalCaseSupportProjectionError> {
    if materialization.plan_root() != partition.plan_root()
        || materialization.relation_id() != partition.relation_id()
        || materialization.admission_id() != partition.admission_id()
        || materialization.classified_chunk_artifact_id() != chunk.id()
        || materialization.chunk_partition_id() != partition.id()
        || materialization.chunk_id() != chunk.chunk_id()
        || materialization.chunk_ordinal() != chunk.chunk_ordinal()
        || materialization.chunk_cell_id() != chunk.chunk_cell_id()
        || materialization.chunk_materializer_id() != chunk.chunk_materializer_id()
        || materialization.run_id() != run.id()
        || materialization.run_ordinal() != run.ordinal()
        || materialization.run_cell_id() != run.cell_id()
        || materialization.run_materializer_id() != chunk.chunk_materializer_id()
        || materialization.interval_start() != run.interval_start()
        || materialization.interval_end_exclusive() != run.interval_end_exclusive()
    {
        return Err(
            RelationalCaseSupportProjectionError::SelectedMaterializationScopeMismatch {
                chunk_ordinal: chunk.chunk_ordinal(),
                run_ordinal: run.ordinal(),
            },
        );
    }
    if materialization.materialized_case_count() != run.cardinality()
        || usize_to_u128(materialization.cases().len())? != run.cardinality()
    {
        return Err(
            RelationalCaseSupportProjectionError::SelectedMaterializationCountMismatch {
                chunk_ordinal: chunk.chunk_ordinal(),
                run_ordinal: run.ordinal(),
            },
        );
    }
    Ok(())
}

fn validate_materialization(
    question_id: QuestionId,
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk: &RelationalClassifiedChunkArtifact,
    run: &RelationalClassifiedRunDescriptor,
    materialization: &RelationalSelectedRunMaterializationArtifact,
) -> Result<(), RelationalCaseSupportProjectionError> {
    validate_materialization_scope(partition, chunk, run, materialization)?;
    if !materialization.contains_question(question_id) {
        return Err(
            RelationalCaseSupportProjectionError::SelectedMaterializationScopeMismatch {
                chunk_ordinal: chunk.chunk_ordinal(),
                run_ordinal: run.ordinal(),
            },
        );
    }
    Ok(())
}

/// Closure-only canonical traversal. Memory is O(one bounded run) for sorting
/// authorized CaseIds by their stable key, rather than O(all selected cases).
fn derive_partition_active_set_root(
    projection_id: RelationalCaseSupportProjectionId,
    question_id: QuestionId,
    partition: &RelationalCaseChunkPartitionArtifact,
    scheduler: RelationalSchedulerView<'_>,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    active_record_count: u128,
) -> Result<RelationalCaseSupportActiveSetRoot, RelationalCaseSupportProjectionError> {
    let mut fold = RelationalCaseSupportActiveSetFold::new(projection_id, active_record_count);
    fold.add_row(
        projection_id,
        RelationalCaseSupportRecordKey::Root,
        RelationalCaseSupportRow::Root {
            relation_id: partition.relation_id(),
            admission_id: partition.admission_id(),
            question_id,
            support_plan_root: partition.plan_root(),
            partition_artifact_id: partition.id(),
            exact_logical_case_count: partition
                .interval_end_exclusive()
                .checked_sub(partition.interval_start())
                .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?,
            planned_chunk_count: usize_to_u128(partition.chunks().len())?,
            case_id_authority: authorization.map(|value| value.authority()),
        },
    )?;

    for (chunk_index, _) in partition.chunks().iter().enumerate() {
        let fragment = scheduler
            .classified_support_fragment_at(chunk_index)
            .map_err(journal_state_error)?
            .ok_or(RelationalCaseSupportProjectionError::PrematureClosure)?;
        validate_classified_fragment(question_id, partition, chunk_index, fragment)?;
        let (key, row) = chunk_row(question_id, partition, fragment)?;
        fold.add_row(projection_id, key, row)?;

        let region_count = fragment_region_count(fragment)?;
        for region_index in 0..region_count {
            let (key, row) = fragment_region_row(question_id, fragment, region_index)?;
            fold.add_row(projection_id, key, row)?;
        }

        let RelationalClassifiedSupportFragment::Concrete(chunk) = fragment else {
            continue;
        };
        for run in chunk.runs() {
            if scalar_outcome(question_id, chunk, run)?
                != RelationalCaseSupportOutcome::AdmittedSelected
            {
                continue;
            }
            let materialization = scheduler
                .selected_run_materialization(run.cell_id())
                .map_err(journal_state_error)?
                .ok_or(RelationalCaseSupportProjectionError::PrematureClosure)?;
            validate_materialization(question_id, partition, chunk, run, materialization)?;
            let (key, row) = materialization_row(chunk, run, materialization);
            fold.add_row(projection_id, key, row)?;
        }

        let Some(authorization) = authorization else {
            continue;
        };
        for run in chunk.runs() {
            if scalar_outcome(question_id, chunk, run)?
                != RelationalCaseSupportOutcome::AdmittedSelected
            {
                continue;
            }
            let materialization = scheduler
                .selected_run_materialization(run.cell_id())
                .map_err(journal_state_error)?
                .ok_or(RelationalCaseSupportProjectionError::PrematureClosure)?;
            validate_materialization(question_id, partition, chunk, run, materialization)?;
            let mut case_ids = Vec::new();
            case_ids
                .try_reserve_exact(materialization.cases().len())
                .map_err(|_| {
                    RelationalCaseSupportProjectionError::AllocationFailed(
                        "bounded canonical case-key order",
                    )
                })?;
            case_ids.extend(materialization.cases().iter().map(|case| case.case_id()));
            case_ids.sort_unstable();
            if !case_ids.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(RelationalCaseSupportProjectionError::DuplicateLogicalSlot);
            }
            for case_id in case_ids {
                let key = RelationalCaseSupportRecordKey::AuthorizedCase {
                    chunk_ordinal: chunk.chunk_ordinal(),
                    run_ordinal: run.ordinal(),
                    case_id,
                };
                fold.add_row(
                    projection_id,
                    key,
                    RelationalCaseSupportRow::AuthorizedCase {
                        chunk_ordinal: chunk.chunk_ordinal(),
                        run_ordinal: run.ordinal(),
                        materialization_artifact_id: materialization.id(),
                        case_id,
                        authority: authorization.authority(),
                    },
                )?;
            }
        }
    }
    fold.finish()
}

pub(crate) fn derive_relational_case_support_projection_id(
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    support_plan_root: [u8; 32],
    basis: RelationalCaseSupportProjectionBasis,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
) -> RelationalCaseSupportProjectionId {
    let mut hasher = Sha256::new();
    hasher.update(PROJECTION_ID_HASH_V4);
    hasher.update(RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA.as_bytes());
    hasher.update(RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION.to_be_bytes());
    hasher.update(RELATIONAL_CASE_SUPPORT_UPDATE_ALGEBRA.as_bytes());
    hasher.update(relation_id.bytes());
    hasher.update(admission_id.bytes());
    hasher.update(question_id.bytes());
    hasher.update(support_plan_root);
    match basis {
        RelationalCaseSupportProjectionBasis::Partition(partition_id) => {
            hasher.update([0_u8]);
            hasher.update(partition_id.bytes());
        }
        RelationalCaseSupportProjectionBasis::ClassificationSummary => {
            hasher.update([1_u8]);
        }
    }
    match authorization.map(|value| value.authority()) {
        None => hasher.update([0_u8]),
        Some(RelationalCaseIdPublicationAuthority::ResultView(view_id)) => {
            hasher.update([1_u8]);
            hasher.update(view_id.bytes());
        }
    }
    RelationalCaseSupportProjectionId(hasher.finalize().into())
}

/// Common envelope for the exact classification-summary fallback. Callers
/// supply a versioned canonical payload encoding; key encoding and projection
/// scope remain centralized here.
pub(crate) fn relational_case_support_row_hash_from_canonical_bytes(
    projection_id: RelationalCaseSupportProjectionId,
    key: RelationalCaseSupportRecordKey,
    canonical_payload: &[u8],
) -> Result<RelationalCaseSupportRowHash, RelationalCaseSupportProjectionError> {
    let mut hasher = Sha256::new();
    hasher.update(ROW_HASH_V1);
    hasher.update(projection_id.bytes());
    hash_record_key(&mut hasher, key);
    hasher.update(usize_to_u128(canonical_payload.len())?.to_be_bytes());
    hasher.update(canonical_payload);
    Ok(RelationalCaseSupportRowHash(hasher.finalize().into()))
}

pub(crate) fn relational_case_support_row_hash(
    projection_id: RelationalCaseSupportProjectionId,
    key: RelationalCaseSupportRecordKey,
    row: RelationalCaseSupportRow,
) -> Result<RelationalCaseSupportRowHash, RelationalCaseSupportProjectionError> {
    validate_key_row(key, row)?;
    let payload = canonical_row_payload(row)?;
    relational_case_support_row_hash_from_canonical_bytes(projection_id, key, &payload)
}

/// Canonical closure over an already key-sorted active set, independent of
/// discovery order. Strict order rejects duplicate logical keys; replay
/// idempotency is handled by the public add algebra before forming this set.
pub(crate) fn relational_case_support_active_set_root(
    projection_id: RelationalCaseSupportProjectionId,
    active_record_count: u128,
    entries: impl IntoIterator<Item = (RelationalCaseSupportRecordKey, RelationalCaseSupportRowHash)>,
) -> Result<RelationalCaseSupportActiveSetRoot, RelationalCaseSupportProjectionError> {
    let mut fold = RelationalCaseSupportActiveSetFold::new(projection_id, active_record_count);
    for (key, row_hash) in entries {
        fold.add_hash(key, row_hash)?;
    }
    fold.finish()
}

pub(crate) struct RelationalCaseSupportActiveSetFold {
    hasher: Sha256,
    prior: Option<RelationalCaseSupportRecordKey>,
    expected: u128,
    observed: u128,
}

impl RelationalCaseSupportActiveSetFold {
    pub(crate) fn new(projection_id: RelationalCaseSupportProjectionId, expected: u128) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(ACTIVE_SET_ROOT_HASH_V1);
        hasher.update(projection_id.bytes());
        hasher.update(expected.to_be_bytes());
        Self {
            hasher,
            prior: None,
            expected,
            observed: 0,
        }
    }

    fn add_row(
        &mut self,
        projection_id: RelationalCaseSupportProjectionId,
        key: RelationalCaseSupportRecordKey,
        row: RelationalCaseSupportRow,
    ) -> Result<(), RelationalCaseSupportProjectionError> {
        self.add_hash(
            key,
            relational_case_support_row_hash(projection_id, key, row)?,
        )
    }

    pub(crate) fn add_hash(
        &mut self,
        key: RelationalCaseSupportRecordKey,
        row_hash: RelationalCaseSupportRowHash,
    ) -> Result<(), RelationalCaseSupportProjectionError> {
        if self.prior.is_some_and(|prior| prior >= key) {
            return Err(
                RelationalCaseSupportProjectionError::NonCanonicalActiveSetOrder {
                    prior: self.prior,
                    current: key,
                },
            );
        }
        hash_record_key(&mut self.hasher, key);
        self.hasher.update(row_hash.bytes());
        self.prior = Some(key);
        self.observed = checked_add(self.observed, 1)?;
        Ok(())
    }

    pub(crate) fn finish(
        self,
    ) -> Result<RelationalCaseSupportActiveSetRoot, RelationalCaseSupportProjectionError> {
        if self.observed != self.expected {
            return Err(
                RelationalCaseSupportProjectionError::ActiveRecordCountMismatch {
                    expected: self.expected,
                    observed: self.observed,
                },
            );
        }
        Ok(RelationalCaseSupportActiveSetRoot(
            self.hasher.finalize().into(),
        ))
    }
}

fn validate_key_row(
    key: RelationalCaseSupportRecordKey,
    row: RelationalCaseSupportRow,
) -> Result<(), RelationalCaseSupportProjectionError> {
    let matches = match (key, row) {
        (RelationalCaseSupportRecordKey::Root, RelationalCaseSupportRow::Root { .. }) => true,
        (
            RelationalCaseSupportRecordKey::Chunk { chunk_ordinal: key },
            RelationalCaseSupportRow::Chunk { chunk_ordinal, .. },
        ) => key == chunk_ordinal,
        (
            RelationalCaseSupportRecordKey::Region {
                chunk_ordinal: key_chunk,
                run_ordinal: key_run,
            },
            RelationalCaseSupportRow::Region {
                chunk_ordinal,
                run_ordinal,
                ..
            },
        ) => key_chunk == chunk_ordinal && key_run == run_ordinal,
        (
            RelationalCaseSupportRecordKey::SelectedMaterialization {
                chunk_ordinal: key_chunk,
                run_ordinal: key_run,
            },
            RelationalCaseSupportRow::SelectedMaterialization {
                chunk_ordinal,
                run_ordinal,
                ..
            },
        ) => key_chunk == chunk_ordinal && key_run == run_ordinal,
        (
            RelationalCaseSupportRecordKey::AuthorizedCase {
                chunk_ordinal: key_chunk,
                run_ordinal: key_run,
                case_id: key_case,
            },
            RelationalCaseSupportRow::AuthorizedCase {
                chunk_ordinal,
                run_ordinal,
                case_id,
                ..
            },
        ) => key_chunk == chunk_ordinal && key_run == run_ordinal && key_case == case_id,
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(RelationalCaseSupportProjectionError::KeyRowMismatch)
    }
}

fn canonical_row_payload(
    row: RelationalCaseSupportRow,
) -> Result<Vec<u8>, RelationalCaseSupportProjectionError> {
    let mut payload = Vec::new();
    payload
        .try_reserve(320)
        .map_err(|_| RelationalCaseSupportProjectionError::AllocationFailed("row hash payload"))?;
    match row {
        RelationalCaseSupportRow::Root {
            relation_id,
            admission_id,
            question_id,
            support_plan_root,
            partition_artifact_id,
            exact_logical_case_count,
            planned_chunk_count,
            case_id_authority,
        } => {
            payload.push(0);
            payload.extend_from_slice(&relation_id.bytes());
            payload.extend_from_slice(&admission_id.bytes());
            payload.extend_from_slice(&question_id.bytes());
            payload.extend_from_slice(&support_plan_root.bytes());
            payload.extend_from_slice(&partition_artifact_id.bytes());
            payload.extend_from_slice(&exact_logical_case_count.to_be_bytes());
            payload.extend_from_slice(&planned_chunk_count.to_be_bytes());
            encode_case_id_authority(&mut payload, case_id_authority);
        }
        RelationalCaseSupportRow::Chunk {
            partition_artifact_id,
            classification_authority,
            chunk_ordinal,
            exact_case_count,
            rejected_case_count,
            admitted_not_selected_case_count,
            admitted_selected_case_count,
            region_count,
        } => {
            payload.push(1);
            payload.extend_from_slice(&partition_artifact_id.bytes());
            encode_classification_authority(&mut payload, classification_authority);
            payload.extend_from_slice(&chunk_ordinal.to_be_bytes());
            payload.extend_from_slice(&exact_case_count.to_be_bytes());
            payload.extend_from_slice(&rejected_case_count.to_be_bytes());
            payload.extend_from_slice(&admitted_not_selected_case_count.to_be_bytes());
            payload.extend_from_slice(&admitted_selected_case_count.to_be_bytes());
            payload.extend_from_slice(&region_count.to_be_bytes());
        }
        RelationalCaseSupportRow::Region {
            classification_authority,
            region_authority,
            chunk_ordinal,
            run_ordinal,
            exact_case_count,
            outcome,
            correlated_starter_region_id,
        } => {
            payload.push(2);
            encode_classification_authority(&mut payload, classification_authority);
            match region_authority {
                RelationalCaseSupportRegionAuthority::ConcreteRun(id) => {
                    payload.push(0);
                    payload.extend_from_slice(&id.bytes());
                }
                RelationalCaseSupportRegionAuthority::CertifiedRegion(id) => {
                    payload.push(1);
                    payload.extend_from_slice(&id);
                }
            }
            payload.extend_from_slice(&chunk_ordinal.to_be_bytes());
            payload.extend_from_slice(&run_ordinal.to_be_bytes());
            payload.extend_from_slice(&exact_case_count.to_be_bytes());
            payload.push(match outcome {
                RelationalCaseSupportOutcome::Rejected => 0,
                RelationalCaseSupportOutcome::AdmittedNotSelected => 1,
                RelationalCaseSupportOutcome::AdmittedSelected => 2,
            });
            match correlated_starter_region_id {
                None => payload.push(0),
                Some(id) => {
                    payload.push(1);
                    payload.extend_from_slice(&id.bytes());
                }
            }
        }
        RelationalCaseSupportRow::SelectedMaterialization {
            chunk_ordinal,
            run_ordinal,
            run_id,
            artifact_id,
            exact_case_count,
            materialized_cases_root,
        } => {
            payload.push(3);
            payload.extend_from_slice(&chunk_ordinal.to_be_bytes());
            payload.extend_from_slice(&run_ordinal.to_be_bytes());
            payload.extend_from_slice(&run_id.bytes());
            payload.extend_from_slice(&artifact_id.bytes());
            payload.extend_from_slice(&exact_case_count.to_be_bytes());
            payload.extend_from_slice(&materialized_cases_root);
        }
        RelationalCaseSupportRow::AuthorizedCase {
            chunk_ordinal,
            run_ordinal,
            materialization_artifact_id,
            case_id,
            authority,
        } => {
            payload.push(4);
            payload.extend_from_slice(&chunk_ordinal.to_be_bytes());
            payload.extend_from_slice(&run_ordinal.to_be_bytes());
            payload.extend_from_slice(&materialization_artifact_id.bytes());
            payload.extend_from_slice(&case_id.bytes());
            encode_case_id_authority(&mut payload, Some(authority));
        }
    }
    Ok(payload)
}

fn encode_classification_authority(
    payload: &mut Vec<u8>,
    authority: RelationalCaseSupportClassificationAuthority,
) {
    match authority {
        RelationalCaseSupportClassificationAuthority::ConcreteSweep(id) => {
            payload.push(0);
            payload.extend_from_slice(&id.bytes());
        }
        RelationalCaseSupportClassificationAuthority::CertifiedRegion(id) => {
            payload.push(1);
            payload.extend_from_slice(&id);
        }
    }
}

fn encode_case_id_authority(
    payload: &mut Vec<u8>,
    authority: Option<RelationalCaseIdPublicationAuthority>,
) {
    match authority {
        None => payload.push(0),
        Some(RelationalCaseIdPublicationAuthority::ResultView(view_id)) => {
            payload.push(1);
            payload.extend_from_slice(&view_id.bytes());
        }
    }
}

fn hash_record_key(hasher: &mut Sha256, key: RelationalCaseSupportRecordKey) {
    match key {
        RelationalCaseSupportRecordKey::Root => hasher.update([0_u8]),
        RelationalCaseSupportRecordKey::ClassificationRegion { region_ordinal } => {
            hasher.update([1_u8]);
            hasher.update(region_ordinal.to_be_bytes());
        }
        RelationalCaseSupportRecordKey::ClassificationAuthorizedCase { case_id } => {
            hasher.update([2_u8]);
            hasher.update(case_id.bytes());
        }
        RelationalCaseSupportRecordKey::Chunk { chunk_ordinal } => {
            hasher.update([3_u8]);
            hasher.update(chunk_ordinal.to_be_bytes());
            hasher.update([0_u8]);
        }
        RelationalCaseSupportRecordKey::Region {
            chunk_ordinal,
            run_ordinal,
        } => {
            hasher.update([3_u8]);
            hasher.update(chunk_ordinal.to_be_bytes());
            hasher.update([1_u8]);
            hasher.update(run_ordinal.to_be_bytes());
        }
        RelationalCaseSupportRecordKey::SelectedMaterialization {
            chunk_ordinal,
            run_ordinal,
        } => {
            hasher.update([3_u8]);
            hasher.update(chunk_ordinal.to_be_bytes());
            hasher.update([2_u8]);
            hasher.update(run_ordinal.to_be_bytes());
        }
        RelationalCaseSupportRecordKey::AuthorizedCase {
            chunk_ordinal,
            run_ordinal,
            case_id,
        } => {
            hasher.update([3_u8]);
            hasher.update(chunk_ordinal.to_be_bytes());
            hasher.update([3_u8]);
            hasher.update(run_ordinal.to_be_bytes());
            hasher.update(case_id.bytes());
        }
    }
}

fn checked_add(left: u128, right: u128) -> Result<u128, RelationalCaseSupportProjectionError> {
    left.checked_add(right)
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)
}

fn usize_to_u128(value: usize) -> Result<u128, RelationalCaseSupportProjectionError> {
    u128::try_from(value).map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)
}

fn journal_state_error(error: impl fmt::Display) -> RelationalCaseSupportProjectionError {
    RelationalCaseSupportProjectionError::JournalState(error.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionError {
    AllocationFailed(&'static str),
    ArithmeticOverflow,
    JournalState(String),
    PartitionAuthorityMismatch,
    SupportCatalogOpen,
    InvalidSelectedQuestionSeal,
    SelectedQuestionIsNotSupportCertified,
    SelectedQuestionCoverageMismatch,
    QuestionNotInPartition {
        question_id: QuestionId,
    },
    QuestionNotInClassifiedChunk {
        question_id: QuestionId,
        chunk_ordinal: u128,
    },
    RegionalCertificateRequiresOneQuestion,
    UnexpectedClassifiedChunk {
        chunk_ordinal: u128,
    },
    ClassifiedChunkOrderMismatch {
        expected: u128,
        actual: u128,
    },
    ClassifiedChunkScopeMismatch {
        chunk_ordinal: u128,
    },
    ClassifiedChunkCountMismatch {
        chunk_ordinal: u128,
    },
    ClassifiedRunOrderMismatch {
        chunk_ordinal: u128,
        expected: u16,
        actual: u16,
    },
    ClassifiedRunOutcomeMismatch {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    MaterializationBeforeChunk {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    SelectedMaterializationScopeMismatch {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    SelectedMaterializationCountMismatch {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    SelectedMaterializationOutcomeMismatch {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    ClassificationExceedsRoot,
    PrematureClosure,
    ClosureQuestionMismatch,
    ClosureRootCountMismatch {
        expected: u128,
        certified: u128,
        classified: u128,
    },
    ClosureSelectedCountMismatch {
        classified: u128,
        materialized: u128,
        sealed: u128,
    },
    DiscoveryIndexMismatch {
        event_ordinal: usize,
    },
    DiscoveryCatalogMismatch(&'static str),
    DuplicateLogicalSlot,
    StableKeyConflict {
        key: RelationalCaseSupportRecordKey,
    },
    NonCanonicalActiveSetOrder {
        prior: Option<RelationalCaseSupportRecordKey>,
        current: RelationalCaseSupportRecordKey,
    },
    ActiveRecordCountMismatch {
        expected: u128,
        observed: u128,
    },
    KeyRowMismatch,
    OrdinalIndexMismatch,
}

impl fmt::Display for RelationalCaseSupportProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed(subject) => write!(f, "failed to allocate {subject}"),
            Self::ArithmeticOverflow => f.write_str("case-support projection arithmetic overflow"),
            Self::JournalState(error) => write!(f, "invalid case-support journal state: {error}"),
            Self::PartitionAuthorityMismatch => f.write_str(
                "case-support partition does not match the authenticated scheduler authority",
            ),
            Self::SupportCatalogOpen => {
                f.write_str("an open support catalog cannot authorize exact closure")
            }
            Self::InvalidSelectedQuestionSeal => {
                f.write_str("selected-question closure seal is invalid")
            }
            Self::SelectedQuestionIsNotSupportCertified => f.write_str(
                "classified case-support closure requires a support-certified selected question",
            ),
            Self::SelectedQuestionCoverageMismatch => f.write_str(
                "selected-question seal coverage disagrees with its certified population",
            ),
            Self::QuestionNotInPartition { question_id } => write!(
                f,
                "case-support question {question_id:?} is not in the shared classified partition"
            ),
            Self::QuestionNotInClassifiedChunk {
                question_id,
                chunk_ordinal,
            } => write!(
                f,
                "case-support question {question_id:?} is not in classified chunk {chunk_ordinal}"
            ),
            Self::RegionalCertificateRequiresOneQuestion => f.write_str(
                "regional classification certificates require an exact-one-question partition",
            ),
            Self::UnexpectedClassifiedChunk { chunk_ordinal } => write!(
                f,
                "classified chunk {chunk_ordinal} is outside the verified partition"
            ),
            Self::ClassifiedChunkOrderMismatch { expected, actual } => write!(
                f,
                "classified chunk order mismatch: expected ordinal {expected}, found {actual}"
            ),
            Self::ClassifiedChunkScopeMismatch { chunk_ordinal } => write!(
                f,
                "classified chunk {chunk_ordinal} does not match the verified partition scope"
            ),
            Self::ClassifiedChunkCountMismatch { chunk_ordinal } => write!(
                f,
                "classified chunk {chunk_ordinal} does not conserve its exact population"
            ),
            Self::ClassifiedRunOrderMismatch {
                chunk_ordinal,
                expected,
                actual,
            } => write!(
                f,
                "classified chunk {chunk_ordinal} run order mismatch: expected {expected}, found {actual}"
            ),
            Self::ClassifiedRunOutcomeMismatch {
                chunk_ordinal,
                run_ordinal,
            } => write!(
                f,
                "classified chunk {chunk_ordinal} run {run_ordinal} has no outcome for the requested question"
            ),
            Self::MaterializationBeforeChunk {
                chunk_ordinal,
                run_ordinal,
            } => write!(
                f,
                "selected materialization for chunk {chunk_ordinal} run {run_ordinal} has no concrete classified parent"
            ),
            Self::SelectedMaterializationScopeMismatch {
                chunk_ordinal,
                run_ordinal,
            } => write!(
                f,
                "selected materialization does not match chunk {chunk_ordinal} run {run_ordinal}"
            ),
            Self::SelectedMaterializationCountMismatch {
                chunk_ordinal,
                run_ordinal,
            } => write!(
                f,
                "selected materialization count does not match chunk {chunk_ordinal} run {run_ordinal}"
            ),
            Self::SelectedMaterializationOutcomeMismatch {
                chunk_ordinal,
                run_ordinal,
            } => write!(
                f,
                "materialization names question at non-selected chunk {chunk_ordinal} run {run_ordinal}"
            ),
            Self::ClassificationExceedsRoot => {
                f.write_str("classified sparse slots exceed the exact root population")
            }
            Self::PrematureClosure => f.write_str(
                "case-support closure precedes complete sparse classification or materialization",
            ),
            Self::ClosureQuestionMismatch => {
                f.write_str("case-support closure names a different question")
            }
            Self::ClosureRootCountMismatch {
                expected,
                certified,
                classified,
            } => write!(
                f,
                "case-support root count mismatch: expected {expected}, certified {certified}, classified {classified}"
            ),
            Self::ClosureSelectedCountMismatch {
                classified,
                materialized,
                sealed,
            } => write!(
                f,
                "case-support selected count mismatch: classified {classified}, materialized {materialized}, sealed {sealed}"
            ),
            Self::DiscoveryIndexMismatch { event_ordinal } => write!(
                f,
                "case-support discovery event {event_ordinal} is missing from its declared prefix"
            ),
            Self::DiscoveryCatalogMismatch(subject) => write!(
                f,
                "case-support discovery stream disagrees with sparse {subject} catalog"
            ),
            Self::DuplicateLogicalSlot => {
                f.write_str("case-support catalog contains a duplicate logical slot")
            }
            Self::StableKeyConflict { key } => write!(
                f,
                "case-support stable key {key:?} was observed with different row hashes"
            ),
            Self::NonCanonicalActiveSetOrder { prior, current } => write!(
                f,
                "case-support active set is not in strict canonical key order: {prior:?} then {current:?}"
            ),
            Self::ActiveRecordCountMismatch { expected, observed } => write!(
                f,
                "case-support active-set count mismatch: expected {expected}, observed {observed}"
            ),
            Self::KeyRowMismatch => {
                f.write_str("case-support stable key does not address its row payload")
            }
            Self::OrdinalIndexMismatch => {
                f.write_str("case-support cumulative package index disagrees with discovery rows")
            }
        }
    }
}

impl Error for RelationalCaseSupportProjectionError {}
