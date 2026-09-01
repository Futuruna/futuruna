//! Pure public projection of the authenticated classified-support prefix.
//!
//! This module deliberately does not discover arbitrary support-catalog cells.
//! The verified bounded partition fixes chunk order, each accepted classified
//! artifact fixes run order, and each selected-run materialization fixes case
//! order.  A publisher can therefore expose one append-only scalar ordinal
//! without making operational proof-discovery order public.
//!
//! Only complete chunk packages enter the public prefix.  If a selected run is
//! not materialized yet, that chunk and every later chunk remain unavailable.
//! The projection contains structural identities, classifications and counts;
//! source/context/successor values, coordinate intervals, materializer IDs and
//! proof payloads stay behind the journal boundary.

use std::error::Error;
use std::fmt;

use super::relation::{AdmissionId, QuestionId, RelationId, RelationalCaseId, ViewId};
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
use super::relational_selected_run_materialization::{
    RelationalSelectedRunMaterializationArtifact, RelationalSelectedRunMaterializationArtifactId,
};
use super::relational_support_planner::RelationalSupportPlanRoot;
use super::support_cell::SupportCellId;
use super::support_evidence::SupportEvidenceRoot;

pub(crate) const RELATIONAL_CASE_SUPPORT_PROJECTION_VERSION: u32 = 1;
pub(crate) const RELATIONAL_CASE_SUPPORT_PROJECTION_SCHEMA: &str =
    "futuruna.relational-case-support-graph.v1";

/// Checked public surface that authorizes equality-linkable `CaseId` records.
///
/// The projection never infers this capability from a field alias.  The
/// publisher must construct it only after resolving a checked result field
/// whose expression explicitly publishes case identity.
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

/// Narrow terminal authority supplied by the authenticated journal prefix.
///
/// Construction requires the caller to state that the support catalog is
/// sealed.  The remaining terminal conditions (complete classified cover,
/// complete selected materialization, scope and cardinality conservation) are
/// independently checked while deriving the projection. This is a narrow
/// crate-internal trust seam for the eventual publisher, not a replacement for
/// a journal-minted closure certificate.
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

/// Closure-sensitive evidence for a count in the current projection prefix.
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
    AwaitingClassifiedChunk {
        next_chunk_ordinal: u128,
    },
    AwaitingSelectedMaterialization {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    AwaitingClosureAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSupportClosureMetadata {
    pub(crate) partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
    pub(crate) support_evidence_root: SupportEvidenceRoot,
    pub(crate) selected_question_seal_id: RelationalSelectedQuestionSealId,
    pub(crate) exact_logical_case_count: u128,
    pub(crate) exact_selected_case_count: u128,
    pub(crate) classified_chunk_count: u128,
    pub(crate) region_count: u128,
    pub(crate) selected_materialization_count: u128,
    pub(crate) authorized_case_record_count: u128,
    /// Number of semantic records preceding the closure record.  The outer
    /// publisher may bind its byte-prefix digest at this exact boundary.
    pub(crate) data_record_count: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionFrontier {
    Open(RelationalCaseSupportOpenReason),
    Exact(RelationalCaseSupportClosureMetadata),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSupportProjectionMetadata {
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
    pub(crate) available_source_record_count: u128,
}

/// One domain-neutral public graph record before JSON encoding.
///
/// Parentage is expressed entirely with stable opaque IDs.  No record carries
/// case state, context, coordinate intervals, materializer identity or proof
/// bytes.  Mechanism signatures and their DAG remain a separate artifact and
/// join this projection only through an authorized `CaseId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionRecord {
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
        chunk_artifact_id: RelationalClassifiedChunkArtifactId,
        chunk_ordinal: u128,
        exact_case_count: u128,
        rejected_case_count: u128,
        admitted_not_selected_case_count: u128,
        admitted_selected_case_count: u128,
        region_count: u128,
    },
    Region {
        chunk_artifact_id: RelationalClassifiedChunkArtifactId,
        run_id: RelationalClassifiedRunId,
        run_ordinal: u16,
        exact_case_count: u128,
        outcome: RelationalClassifiedCaseOutcome,
    },
    SelectedMaterialization {
        run_id: RelationalClassifiedRunId,
        artifact_id: RelationalSelectedRunMaterializationArtifactId,
        exact_case_count: u128,
        materialized_cases_root: [u8; 32],
    },
    AuthorizedCase {
        materialization_artifact_id: RelationalSelectedRunMaterializationArtifactId,
        case_id: RelationalCaseId,
        authority: RelationalCaseIdPublicationAuthority,
    },
    Closure(RelationalCaseSupportClosureMetadata),
}

/// Invocation-local O(B) address index over a borrowed authenticated prefix.
///
/// Cumulative chunk ends make arbitrary ordinal lookup O(log B); locating the
/// exact record then scans only one V1 chunk, whose run and selected-case cover
/// is bounded by the partition contract.
pub(crate) struct RelationalCaseSupportProjection<'a> {
    partition: &'a RelationalCaseChunkPartitionArtifact,
    classified_chunks: &'a [RelationalClassifiedChunkArtifact],
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    chunk_indexes: Box<[RelationalCaseSupportChunkOrdinalIndex]>,
    materializations: Box<[&'a RelationalSelectedRunMaterializationArtifact]>,
    closure: Option<RelationalCaseSupportClosureMetadata>,
    metadata: RelationalCaseSupportProjectionMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationalCaseSupportChunkOrdinalIndex {
    record_end: u128,
    materialization_end: usize,
}

impl<'a> RelationalCaseSupportProjection<'a> {
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
            return Ok(Some(RelationalCaseSupportProjectionRecord::Root {
                relation_id: self.partition.relation_id(),
                admission_id: self.partition.admission_id(),
                question_id: self.partition.question_id(),
                support_plan_root: self.partition.plan_root(),
                partition_artifact_id: self.partition.id(),
                exact_logical_case_count: self.metadata.exact_logical_case_count,
                planned_chunk_count: self.metadata.planned_chunk_count,
                case_id_authority: self
                    .authorization
                    .map(|authorization| authorization.authority()),
            }));
        }

        let data_record_count = self
            .chunk_indexes
            .last()
            .map_or(1, |index| index.record_end);
        if source_ordinal == data_record_count {
            return Ok(self
                .closure
                .map(RelationalCaseSupportProjectionRecord::Closure));
        }

        let chunk_index = self
            .chunk_indexes
            .partition_point(|index| index.record_end <= source_ordinal);
        let chunk = self
            .classified_chunks
            .get(chunk_index)
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
        let chunk_start = chunk_index
            .checked_sub(1)
            .map_or(1, |prior| self.chunk_indexes[prior].record_end);
        let materialization_start = chunk_index
            .checked_sub(1)
            .map_or(0, |prior| self.chunk_indexes[prior].materialization_end);
        let materialization_end = self
            .chunk_indexes
            .get(chunk_index)
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?
            .materialization_end;
        let materializations = self
            .materializations
            .get(materialization_start..materialization_end)
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
        let mut materialization_index = 0usize;
        let mut relative = source_ordinal
            .checked_sub(chunk_start)
            .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
        if relative == 0 {
            return Ok(Some(chunk_record(self.partition, chunk)?));
        }
        relative -= 1;

        for run in chunk.runs() {
            if relative == 0 {
                return Ok(Some(RelationalCaseSupportProjectionRecord::Region {
                    chunk_artifact_id: chunk.id(),
                    run_id: run.id(),
                    run_ordinal: run.ordinal(),
                    exact_case_count: run.cardinality(),
                    outcome: run.outcome(),
                }));
            }
            relative -= 1;

            if run.outcome() != RelationalClassifiedCaseOutcome::AdmittedSelected {
                continue;
            }
            let materialization = *materializations
                .get(materialization_index)
                .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
            materialization_index = materialization_index
                .checked_add(1)
                .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
            validate_materialization(self.partition, chunk, run, materialization)?;
            if relative == 0 {
                return Ok(Some(
                    RelationalCaseSupportProjectionRecord::SelectedMaterialization {
                        run_id: run.id(),
                        artifact_id: materialization.id(),
                        exact_case_count: materialization.materialized_case_count(),
                        materialized_cases_root: materialization.materialized_cases_root(),
                    },
                ));
            }
            relative -= 1;

            let Some(authorization) = self.authorization else {
                continue;
            };
            let case_count = usize_to_u128(materialization.cases().len())?;
            if relative < case_count {
                let member_index = usize::try_from(relative)
                    .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
                let case = materialization
                    .cases()
                    .get(member_index)
                    .ok_or(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)?;
                return Ok(Some(
                    RelationalCaseSupportProjectionRecord::AuthorizedCase {
                        materialization_artifact_id: materialization.id(),
                        case_id: case.case_id(),
                        authority: authorization.authority(),
                    },
                ));
            }
            relative -= case_count;
        }

        Err(RelationalCaseSupportProjectionError::OrdinalIndexMismatch)
    }
}

/// Derive a deterministic public prefix from replay-verified partition and
/// classified artifacts.  `selected_materialization` must read the same
/// immutable authenticated journal prefix as the supplied artifacts.
pub(crate) fn derive_relational_case_support_projection<'a, F>(
    verified_partition: &'a VerifiedRelationalCaseChunkPartition,
    classified_chunks: &'a [RelationalClassifiedChunkArtifact],
    mut selected_materialization: F,
    authorization: Option<RelationalCaseIdPublicationAuthorization>,
    closure_authority: Option<RelationalCaseSupportClosureAuthority>,
) -> Result<RelationalCaseSupportProjection<'a>, RelationalCaseSupportProjectionError>
where
    F: FnMut(SupportCellId) -> Option<&'a RelationalSelectedRunMaterializationArtifact>,
{
    let partition = verified_partition.artifact();
    let planned_chunk_count = usize_to_u128(partition.chunks().len())?;
    let classified_chunk_count = usize_to_u128(classified_chunks.len())?;
    if classified_chunk_count > planned_chunk_count {
        return Err(RelationalCaseSupportProjectionError::ClassifiedPrefixExceedsPartition);
    }
    let exact_logical_case_count = partition
        .interval_end_exclusive()
        .checked_sub(partition.interval_start())
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;

    let mut classified_case_count = 0_u128;
    let mut selected_case_count = 0_u128;
    let mut classified_region_count = 0_u128;
    let mut classified_region_capacity = 0usize;
    for (index, chunk) in classified_chunks.iter().enumerate() {
        validate_classified_chunk(partition, index, chunk)?;
        classified_case_count = checked_add(classified_case_count, chunk.evaluated_case_count())?;
        selected_case_count = checked_add(selected_case_count, chunk.admitted_selected_count())?;
        classified_region_count =
            checked_add(classified_region_count, usize_to_u128(chunk.runs().len())?)?;
        classified_region_capacity = classified_region_capacity
            .checked_add(chunk.runs().len())
            .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
    }
    if classified_case_count > exact_logical_case_count {
        return Err(RelationalCaseSupportProjectionError::ClassificationExceedsRoot);
    }

    let mut chunk_indexes = Vec::new();
    chunk_indexes
        .try_reserve_exact(classified_chunks.len())
        .map_err(|_| {
            RelationalCaseSupportProjectionError::AllocationFailed("case-support chunk index")
        })?;
    let mut materializations = Vec::new();
    materializations
        .try_reserve_exact(classified_region_capacity)
        .map_err(|_| {
            RelationalCaseSupportProjectionError::AllocationFailed(
                "case-support materialization index",
            )
        })?;
    let mut record_end = 1_u128;
    let mut published_region_count = 0_u128;
    let mut published_materialization_count = 0_u128;
    let mut materialized_selected_case_count = 0_u128;
    let mut authorized_case_record_count = 0_u128;
    let mut first_missing = None;

    'chunks: for chunk in classified_chunks {
        let package_materialization_start = materializations.len();
        let mut package_record_count = 1_u128;
        let mut package_region_count = 0_u128;
        let mut package_materialization_count = 0_u128;
        let mut package_materialized_case_count = 0_u128;
        let mut package_authorized_case_count = 0_u128;

        for run in chunk.runs() {
            package_record_count = checked_add(package_record_count, 1)?;
            package_region_count = checked_add(package_region_count, 1)?;
            if run.outcome() != RelationalClassifiedCaseOutcome::AdmittedSelected {
                continue;
            }
            let Some(materialization) = selected_materialization(run.cell_id()) else {
                materializations.truncate(package_materialization_start);
                first_missing = Some((chunk.chunk_ordinal(), run.ordinal()));
                break 'chunks;
            };
            validate_materialization(partition, chunk, run, materialization)?;
            materializations.push(materialization);
            package_record_count = checked_add(package_record_count, 1)?;
            package_materialization_count = checked_add(package_materialization_count, 1)?;
            package_materialized_case_count = checked_add(
                package_materialized_case_count,
                materialization.materialized_case_count(),
            )?;
            if authorization.is_some() {
                let case_count = usize_to_u128(materialization.cases().len())?;
                package_record_count = checked_add(package_record_count, case_count)?;
                package_authorized_case_count =
                    checked_add(package_authorized_case_count, case_count)?;
            }
        }

        record_end = checked_add(record_end, package_record_count)?;
        chunk_indexes.push(RelationalCaseSupportChunkOrdinalIndex {
            record_end,
            materialization_end: materializations.len(),
        });
        published_region_count = checked_add(published_region_count, package_region_count)?;
        published_materialization_count = checked_add(
            published_materialization_count,
            package_materialization_count,
        )?;
        materialized_selected_case_count = checked_add(
            materialized_selected_case_count,
            package_materialized_case_count,
        )?;
        authorized_case_record_count =
            checked_add(authorized_case_record_count, package_authorized_case_count)?;
    }

    let published_chunk_count = usize_to_u128(chunk_indexes.len())?;
    let classification_complete = classified_chunk_count == planned_chunk_count;
    let materialization_complete =
        first_missing.is_none() && published_chunk_count == planned_chunk_count;
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
            if authority.question_id != partition.question_id() {
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
            Some(RelationalCaseSupportClosureMetadata {
                partition_artifact_id: partition.id(),
                support_evidence_root: authority.support_evidence_root,
                selected_question_seal_id: authority.selected_question_seal_id,
                exact_logical_case_count,
                exact_selected_case_count: selected_case_count,
                classified_chunk_count,
                region_count: classified_region_count,
                selected_materialization_count: published_materialization_count,
                authorized_case_record_count,
                data_record_count: record_end,
            })
        }
        None => None,
    };

    let frontier = if let Some(closure) = closure {
        RelationalCaseSupportProjectionFrontier::Exact(closure)
    } else if let Some((chunk_ordinal, run_ordinal)) = first_missing {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingSelectedMaterialization {
                chunk_ordinal,
                run_ordinal,
            },
        )
    } else if !classification_complete {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingClassifiedChunk {
                next_chunk_ordinal: classified_chunk_count,
            },
        )
    } else {
        RelationalCaseSupportProjectionFrontier::Open(
            RelationalCaseSupportOpenReason::AwaitingClosureAuthority,
        )
    };
    let closure_record_count = if closure.is_some() { 1 } else { 0 };
    let available_source_record_count = checked_add(record_end, closure_record_count)?;
    let metadata = RelationalCaseSupportProjectionMetadata {
        frontier,
        exact_logical_case_count,
        classified_case_count: classified_count,
        selected_case_count: selected_count,
        materialized_selected_case_count: materialized_count,
        planned_chunk_count,
        classified_chunk_count,
        published_chunk_count,
        published_region_count,
        published_selected_materialization_count: published_materialization_count,
        authorized_case_record_count,
        available_source_record_count,
    };

    Ok(RelationalCaseSupportProjection {
        partition,
        classified_chunks,
        authorization,
        chunk_indexes: chunk_indexes.into_boxed_slice(),
        materializations: materializations.into_boxed_slice(),
        closure,
        metadata,
    })
}

fn chunk_record(
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk: &RelationalClassifiedChunkArtifact,
) -> Result<RelationalCaseSupportProjectionRecord, RelationalCaseSupportProjectionError> {
    Ok(RelationalCaseSupportProjectionRecord::Chunk {
        partition_artifact_id: partition.id(),
        chunk_artifact_id: chunk.id(),
        chunk_ordinal: chunk.chunk_ordinal(),
        exact_case_count: chunk.evaluated_case_count(),
        rejected_case_count: chunk.rejected_count(),
        admitted_not_selected_case_count: chunk.admitted_not_selected_count(),
        admitted_selected_case_count: chunk.admitted_selected_count(),
        region_count: usize_to_u128(chunk.runs().len())?,
    })
}

fn validate_classified_chunk(
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk_index: usize,
    chunk: &RelationalClassifiedChunkArtifact,
) -> Result<(), RelationalCaseSupportProjectionError> {
    let expected_ordinal = usize_to_u128(chunk_index)?;
    let descriptor = partition
        .chunks()
        .get(chunk_index)
        .ok_or(RelationalCaseSupportProjectionError::ClassifiedPrefixExceedsPartition)?;
    if chunk.chunk_ordinal() != expected_ordinal {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkOrderMismatch {
                expected: expected_ordinal,
                actual: chunk.chunk_ordinal(),
            },
        );
    }
    if chunk.plan_root() != partition.plan_root()
        || chunk.relation_id() != partition.relation_id()
        || chunk.admission_id() != partition.admission_id()
        || chunk.question_id() != partition.question_id()
        || chunk.chunk_partition_id() != partition.id()
        || chunk.chunk_id() != descriptor.id()
        || chunk.chunk_cell_id() != descriptor.cell_id()
        || chunk.interval_start() != descriptor.interval_start()
        || chunk.interval_end_exclusive() != descriptor.interval_end_exclusive()
        || chunk.evaluated_case_count() != descriptor.cardinality()
    {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkScopeMismatch {
                chunk_ordinal: expected_ordinal,
            },
        );
    }
    let classified = chunk
        .rejected_count()
        .checked_add(chunk.admitted_not_selected_count())
        .and_then(|count| count.checked_add(chunk.admitted_selected_count()))
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
    if classified != chunk.evaluated_case_count() {
        return Err(
            RelationalCaseSupportProjectionError::ClassifiedChunkCountMismatch {
                chunk_ordinal: expected_ordinal,
            },
        );
    }
    for (run_index, run) in chunk.runs().iter().enumerate() {
        let expected_run_ordinal = u16::try_from(run_index)
            .map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)?;
        if run.ordinal() != expected_run_ordinal {
            return Err(
                RelationalCaseSupportProjectionError::ClassifiedRunOrderMismatch {
                    chunk_ordinal: expected_ordinal,
                    expected: expected_run_ordinal,
                    actual: run.ordinal(),
                },
            );
        }
    }
    Ok(())
}

fn validate_materialization(
    partition: &RelationalCaseChunkPartitionArtifact,
    chunk: &RelationalClassifiedChunkArtifact,
    run: &RelationalClassifiedRunDescriptor,
    materialization: &RelationalSelectedRunMaterializationArtifact,
) -> Result<(), RelationalCaseSupportProjectionError> {
    if materialization.plan_root() != partition.plan_root()
        || materialization.relation_id() != partition.relation_id()
        || materialization.admission_id() != partition.admission_id()
        || materialization.question_id() != partition.question_id()
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

fn checked_add(left: u128, right: u128) -> Result<u128, RelationalCaseSupportProjectionError> {
    left.checked_add(right)
        .ok_or(RelationalCaseSupportProjectionError::ArithmeticOverflow)
}

fn usize_to_u128(value: usize) -> Result<u128, RelationalCaseSupportProjectionError> {
    u128::try_from(value).map_err(|_| RelationalCaseSupportProjectionError::ArithmeticOverflow)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseSupportProjectionError {
    AllocationFailed(&'static str),
    ArithmeticOverflow,
    SupportCatalogOpen,
    InvalidSelectedQuestionSeal,
    SelectedQuestionIsNotSupportCertified,
    SelectedQuestionCoverageMismatch,
    ClassifiedPrefixExceedsPartition,
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
    SelectedMaterializationScopeMismatch {
        chunk_ordinal: u128,
        run_ordinal: u16,
    },
    SelectedMaterializationCountMismatch {
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
    OrdinalIndexMismatch,
}

impl fmt::Display for RelationalCaseSupportProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed(subject) => {
                write!(f, "failed to allocate {subject}")
            }
            Self::ArithmeticOverflow => write!(f, "case-support projection arithmetic overflow"),
            Self::SupportCatalogOpen => {
                write!(f, "an open support catalog cannot authorize exact closure")
            }
            Self::InvalidSelectedQuestionSeal => {
                write!(f, "selected-question closure seal is invalid")
            }
            Self::SelectedQuestionIsNotSupportCertified => write!(
                f,
                "classified case-support closure requires a support-certified selected question"
            ),
            Self::SelectedQuestionCoverageMismatch => write!(
                f,
                "selected-question seal coverage disagrees with its certified population"
            ),
            Self::ClassifiedPrefixExceedsPartition => {
                write!(f, "classified chunk prefix exceeds the verified partition")
            }
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
            Self::ClassificationExceedsRoot => {
                write!(f, "classified prefix exceeds the exact root population")
            }
            Self::PrematureClosure => write!(
                f,
                "case-support closure precedes complete classification or selected materialization"
            ),
            Self::ClosureQuestionMismatch => {
                write!(f, "case-support closure names a different question")
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
            Self::OrdinalIndexMismatch => {
                write!(
                    f,
                    "case-support ordinal index disagrees with its source prefix"
                )
            }
        }
    }
}

impl Error for RelationalCaseSupportProjectionError {}
