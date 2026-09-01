//! Canonical sparse realization of admitted+selected classified runs.
//!
//! A process-local cursor scans the retained classified prefix in chunk/run
//! order and evaluates at most one missing selected run. The cursor is only a
//! search hint: it remains on an emitted run until the journal proves that run
//! was accepted, and cold replay safely starts from zero. The resulting bounded
//! artifact is proposed as one unapplied journal event. This lets concrete
//! interesting cases emerge immediately after their classified chunk while
//! preserving the exact symbolic population as the closure authority.

use std::cell::RefCell;
use std::error::Error;
use std::fmt;

use crate::CheckedExploreQueryView;

use super::relational_bounded_chunk_partition::{
    RelationalCaseChunkId, RelationalCaseChunkPartitionError, RelationalCaseChunkUnsupported,
};
use super::relational_classified_sweep::{
    reverify_relational_classified_chunk_artifact, RelationalClassifiedCaseOutcome,
    RelationalClassifiedChunkArtifactId, RelationalClassifiedRunId, RelationalClassifiedSweepError,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalSchedulerView,
};
use super::relational_selected_run_materialization::{
    materialize_relational_selected_run, RelationalSelectedRunMaterializationArtifactId,
    RelationalSelectedRunMaterializationError,
};
use super::relational_support_planner::{
    RelationalCaseImageInjectivityProofError, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::support_cell::{relational_case_chunk_partition_gateway, SupportCellError};
use super::support_evidence::SupportEvidenceRecord;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedRunStepQuantum {
    classified_chunk_artifact_id: RelationalClassifiedChunkArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    run_id: RelationalClassifiedRunId,
    run_ordinal: u16,
    interval_start: u128,
    interval_end_exclusive: u128,
    materialized_case_count: u128,
    materialization_artifact_id: RelationalSelectedRunMaterializationArtifactId,
}

impl RelationalSelectedRunStepQuantum {
    pub(crate) const fn classified_chunk_artifact_id(self) -> RelationalClassifiedChunkArtifactId {
        self.classified_chunk_artifact_id
    }

    pub(crate) const fn chunk_id(self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn chunk_ordinal(self) -> u128 {
        self.chunk_ordinal
    }

    pub(crate) const fn run_id(self) -> RelationalClassifiedRunId {
        self.run_id
    }

    pub(crate) const fn run_ordinal(self) -> u16 {
        self.run_ordinal
    }

    pub(crate) const fn interval_start(self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn materialized_case_count(self) -> u128 {
        self.materialized_case_count
    }

    pub(crate) const fn materialization_artifact_id(
        self,
    ) -> RelationalSelectedRunMaterializationArtifactId {
        self.materialization_artifact_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedRunStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalSelectedRunStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalSelectedRunStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalSelectedRunStepQuantum {
        self.quantum
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSelectedRunStepOutcome {
    Emitted(RelationalSelectedRunStepBatch),
    CaughtUp,
}

pub(crate) struct RelationalSelectedRunStepDriver<'query> {
    // Preserve the producer-owned expression addresses used by the checked
    // interpreter runtime; this is a shallow view copy, never a cloned IR.
    checked: CheckedExploreQueryView<'query>,
    support_plan: &'query RelationalSupportPlan,
    discovery_cursor: RefCell<(usize, usize)>,
}

impl<'query> RelationalSelectedRunStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
    ) -> Result<Self, RelationalSelectedRunStepDriverError> {
        checked
            .closed_query
            .validate()
            .map_err(RelationalSelectedRunStepDriverError::InvalidQuery)?;
        if !support_plan.validate_root()
            || support_plan.relation_id() != checked.relation_id()
            || support_plan.admission_id() != checked.admission_id()
            || support_plan.question_id() != checked.question_id()
        {
            return Err(RelationalSelectedRunStepDriverError::SupportPlanScopeMismatch);
        }
        Ok(Self {
            checked: *checked,
            support_plan,
            discovery_cursor: RefCell::new((0, 0)),
        })
    }

    pub(crate) fn step<R: RelationalExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        runtime: &mut R,
    ) -> Result<RelationalSelectedRunStepOutcome, RelationalSelectedRunStepDriverError> {
        let checked = &self.checked;
        let view = journal.scheduler_view()?;
        self.validate_scope(checked, view)?;

        let retained = view.classified_chunk_artifacts();
        if retained.is_empty() {
            return Ok(RelationalSelectedRunStepOutcome::CaughtUp);
        }
        let verified_partition = view
            .verified_case_chunk_partition()
            .ok_or(RelationalSelectedRunStepDriverError::CanonicalPartitionMismatch)?;
        let durable_root_injectivity = match view
            .support_evidence_record(verified_partition.durable_root_injectivity_evidence_id())
        {
            Some(SupportEvidenceRecord::Injectivity(evidence))
                if evidence.id() == verified_partition.durable_root_injectivity_evidence_id()
                    && evidence.receipt().id()
                        == verified_partition.durable_root_injectivity_receipt_id() =>
            {
                evidence
            }
            Some(_) => {
                return Err(RelationalSelectedRunStepDriverError::RootInjectivityEvidenceMismatch);
            }
            None => {
                return Err(RelationalSelectedRunStepDriverError::RootInjectivityEvidenceMissing);
            }
        };
        if verified_partition.artifact().plan_root() != self.support_plan.root()
            || verified_partition.artifact().relation_id() != checked.relation_id()
            || verified_partition.artifact().admission_id() != checked.admission_id()
            || verified_partition.artifact().question_id() != checked.question_id()
            || verified_partition.artifact().injectivity_evidence_id()
                != durable_root_injectivity.id()
        {
            return Err(RelationalSelectedRunStepDriverError::CanonicalPartitionMismatch);
        }
        let progress = view
            .classified_sweep_progress()
            .ok_or(RelationalSelectedRunStepDriverError::ClassifiedProgressMissing)?;
        if progress.accepted_chunk_count() != retained.len()
            || progress.partition_artifact_id() != verified_partition.artifact().id()
        {
            return Err(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch);
        }

        // Retained artifacts were structurally reverified at journal
        // admission. Locate the first missing selected descriptor from the
        // process-local hint, then remint authority for only that one chunk.
        // The cursor never outruns a proposed-but-unapplied event: it remains
        // on the target until the durable view reports that target present.
        let mut cursor = *self.discovery_cursor.borrow();
        if cursor.0 > retained.len() {
            cursor = (0, 0);
        }
        let mut target = None;
        for chunk_index in cursor.0..retained.len() {
            let artifact = &retained[chunk_index];
            let chunk = verified_partition
                .partition()
                .chunks()
                .get(chunk_index)
                .ok_or(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch)?;
            let canonical_chunk_ordinal = u128::try_from(chunk_index)
                .map_err(|_| RelationalSelectedRunStepDriverError::RetainedPrefixMismatch)?;
            if artifact.chunk_ordinal() != canonical_chunk_ordinal
                || artifact.chunk_id() != chunk.descriptor().id()
            {
                return Err(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch);
            }
            let first_run = if chunk_index == cursor.0 {
                if cursor.1 > artifact.runs().len() {
                    return Err(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch);
                }
                cursor.1
            } else {
                0
            };
            for (run_index, run) in artifact.runs().iter().enumerate().skip(first_run) {
                if run.outcome() == RelationalClassifiedCaseOutcome::AdmittedSelected
                    && view.selected_run_materialization(run.cell_id()).is_none()
                {
                    cursor = (chunk_index, run_index);
                    target = Some((chunk_index, run_index));
                    break;
                }
            }
            if target.is_some() {
                break;
            }
            cursor = (chunk_index + 1, 0);
        }
        *self.discovery_cursor.borrow_mut() = cursor;
        let Some((chunk_index, run_index)) = target else {
            return Ok(RelationalSelectedRunStepOutcome::CaughtUp);
        };

        let artifact = &retained[chunk_index];
        let chunk = &verified_partition.partition().chunks()[chunk_index];
        let expected_chunk_injectivity =
            relational_case_chunk_partition_gateway::injectivity(verified_partition, chunk_index)?;
        let durable_chunk_injectivity =
            match view.support_evidence_record(expected_chunk_injectivity.id()) {
                Some(SupportEvidenceRecord::Injectivity(evidence))
                    if evidence == &expected_chunk_injectivity =>
                {
                    evidence
                }
                Some(_) => {
                    return Err(
                        RelationalSelectedRunStepDriverError::ChunkInjectivityEvidenceMismatch {
                            chunk_id: chunk.descriptor().id(),
                        },
                    );
                }
                None => {
                    return Err(
                        RelationalSelectedRunStepDriverError::ChunkInjectivityEvidenceMissing {
                            chunk_id: chunk.descriptor().id(),
                        },
                    );
                }
            };
        let verified_classified = reverify_relational_classified_chunk_artifact(
            artifact,
            self.support_plan,
            verified_partition,
            durable_chunk_injectivity,
        )?;
        let run = verified_classified
            .runs()
            .get(run_index)
            .ok_or(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch)?;
        if run.descriptor().outcome() != RelationalClassifiedCaseOutcome::AdmittedSelected
            || view.selected_run_materialization(run.cell().id()).is_some()
        {
            return Err(RelationalSelectedRunStepDriverError::RetainedPrefixMismatch);
        }
        let run_ordinal = u16::try_from(run_index)
            .map_err(|_| RelationalSelectedRunStepDriverError::RetainedPrefixMismatch)?;
        let materialized = materialize_relational_selected_run(
            checked,
            self.support_plan,
            verified_partition,
            &verified_classified,
            run_ordinal,
            runtime,
        )?;
        let materialized_artifact = materialized.artifact().clone();
        let quantum = RelationalSelectedRunStepQuantum {
            classified_chunk_artifact_id: artifact.id(),
            chunk_id: artifact.chunk_id(),
            chunk_ordinal: artifact.chunk_ordinal(),
            run_id: run.descriptor().id(),
            run_ordinal,
            interval_start: run.descriptor().interval_start(),
            interval_end_exclusive: run.descriptor().interval_end_exclusive(),
            materialized_case_count: materialized_artifact.materialized_case_count(),
            materialization_artifact_id: materialized_artifact.id(),
        };
        Ok(RelationalSelectedRunStepOutcome::Emitted(
            RelationalSelectedRunStepBatch {
                expected_sequence: view.sequence(),
                expected_head: view.head(),
                quantum,
                events: vec![
                    RelationalJournalEvent::relational_selected_run_materialization_accepted(
                        materialized_artifact,
                    ),
                ]
                .into_boxed_slice(),
            },
        ))
    }

    fn validate_scope(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalSelectedRunStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != checked.relation_id()
            || contract.admission_id() != checked.admission_id()
            || contract.question_id() != checked.question_id()
            || contract.state_schema_id() != checked.transition_schemas().state_schema_id()
            || contract.context_schema_id() != checked.transition_schemas().context_schema_id()
            || contract.transition_type_id() != checked.transition_schemas().transition_type_id()
        {
            return Err(RelationalSelectedRunStepDriverError::JournalScopeMismatch);
        }
        match view.support_plan_root() {
            Some(actual) if actual == self.support_plan.root() => {}
            Some(actual) => {
                return Err(
                    RelationalSelectedRunStepDriverError::SupportPlanRootMismatch {
                        expected: self.support_plan.root(),
                        actual,
                    },
                );
            }
            None => return Err(RelationalSelectedRunStepDriverError::SupportPlanMissing),
        }
        if view.source_traversal_is_started() {
            return Err(RelationalSelectedRunStepDriverError::SourceTraversalAlreadyStarted);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSelectedRunStepDriverError {
    InvalidQuery(String),
    SupportPlanScopeMismatch,
    AlreadyBounded,
    UnsupportedPartition(RelationalCaseChunkUnsupported),
    JournalScopeMismatch,
    SupportPlanMissing,
    SupportPlanRootMismatch {
        expected: RelationalSupportPlanRoot,
        actual: RelationalSupportPlanRoot,
    },
    SourceTraversalAlreadyStarted,
    ClassifiedProgressMissing,
    RetainedPrefixMismatch,
    RootInjectivityEvidenceMissing,
    RootInjectivityEvidenceMismatch,
    CanonicalPartitionMismatch,
    ChunkInjectivityEvidenceMissing {
        chunk_id: RelationalCaseChunkId,
    },
    ChunkInjectivityEvidenceMismatch {
        chunk_id: RelationalCaseChunkId,
    },
    Journal(RelationalJournalError),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    ChunkPartition(RelationalCaseChunkPartitionError),
    ClassifiedSweep(RelationalClassifiedSweepError),
    SelectedRunMaterialization(RelationalSelectedRunMaterializationError),
    SupportCell(SupportCellError),
}

impl fmt::Display for RelationalSelectedRunStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(formatter, "invalid checked query: {message}"),
            Self::SupportPlanScopeMismatch => formatter
                .write_str("selected-run driver support plan does not match the checked query"),
            Self::AlreadyBounded => formatter
                .write_str("selected-run driver requires the proper classified chunk partition"),
            Self::UnsupportedPartition(reason) => write!(
                formatter,
                "selected-run driver does not support partition shape {reason:?}"
            ),
            Self::JournalScopeMismatch => {
                formatter.write_str("selected-run journal contract does not match the query")
            }
            Self::SupportPlanMissing => {
                formatter.write_str("selected-run journal has no registered support plan")
            }
            Self::SupportPlanRootMismatch { expected, actual } => write!(
                formatter,
                "selected-run support-plan root mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::SourceTraversalAlreadyStarted => formatter.write_str(
                "selected-run materialization cannot interleave with ordinary source traversal",
            ),
            Self::ClassifiedProgressMissing => {
                formatter.write_str("selected-run materialization has no typed classified progress")
            }
            Self::RetainedPrefixMismatch => formatter.write_str(
                "retained classified artifacts are not the canonical accepted chunk prefix",
            ),
            Self::RootInjectivityEvidenceMissing => formatter.write_str(
                "selected-run materialization requires durable root injectivity evidence",
            ),
            Self::RootInjectivityEvidenceMismatch => formatter
                .write_str("selected-run root injectivity differs from the checked producer proof"),
            Self::CanonicalPartitionMismatch => formatter.write_str(
                "selected-run canonical partition differs from the retained checked plan",
            ),
            Self::ChunkInjectivityEvidenceMissing { chunk_id } => write!(
                formatter,
                "selected-run chunk {chunk_id:?} has no durable injectivity evidence"
            ),
            Self::ChunkInjectivityEvidenceMismatch { chunk_id } => write!(
                formatter,
                "selected-run chunk {chunk_id:?} durable injectivity evidence differs"
            ),
            Self::Journal(error) => fmt::Display::fmt(error, formatter),
            Self::CaseImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::ChunkPartition(error) => fmt::Display::fmt(error, formatter),
            Self::ClassifiedSweep(error) => fmt::Display::fmt(error, formatter),
            Self::SelectedRunMaterialization(error) => fmt::Display::fmt(error, formatter),
            Self::SupportCell(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for RelationalSelectedRunStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Journal(error) => Some(error),
            Self::CaseImageProof(error) => Some(error),
            Self::ChunkPartition(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::SelectedRunMaterialization(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

macro_rules! selected_driver_error_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for RelationalSelectedRunStepDriverError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

selected_driver_error_from!(RelationalJournalError, Journal);
selected_driver_error_from!(RelationalCaseImageInjectivityProofError, CaseImageProof);
selected_driver_error_from!(RelationalCaseChunkPartitionError, ChunkPartition);
selected_driver_error_from!(RelationalClassifiedSweepError, ClassifiedSweep);
selected_driver_error_from!(
    RelationalSelectedRunMaterializationError,
    SelectedRunMaterialization
);
selected_driver_error_from!(SupportCellError, SupportCell);
