//! Resumable scheduling for the canonical bounded classified sweep.
//!
//! This driver is deliberately standalone. It schedules one caller-bounded
//! slice of the next proper canonical partition child from an authenticated
//! journal prefix. A complete slice fold additionally emits the canonical
//! whole-chunk event. It never seals support, starts the ordinary source
//! traversal, or claims source-image/result closure. Those transitions require
//! additional proof vocabulary and remain outside this layer.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use crate::CheckedExploreQueryView;

use super::relational_bounded_chunk_partition::{
    plan_relational_bounded_case_chunks, RelationalCaseChunkId, RelationalCaseChunkPartition,
    RelationalCaseChunkPartitionArtifact, RelationalCaseChunkPartitionArtifactId,
    RelationalCaseChunkPartitionError, RelationalCaseChunkPlanningOutcome,
    RelationalCaseChunkUnsupported,
};
use super::relational_classified_sweep::{
    classify_relational_case_chunk_slice, classify_relational_case_chunk_slice_with_backend,
    finalize_relational_classified_case_chunk, RelationalClassifiedChunkArtifactId,
    RelationalClassifiedChunkSliceId, RelationalClassifiedSweepError,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::relational_journal::{
    RelationalJournal, RelationalJournalError, RelationalJournalEvent, RelationalJournalHead,
    RelationalSchedulerView,
};
use super::relational_native_classifier::{
    RelationalNativeClassifierFallbackBackendV2, RelationalNativeClassifierV2,
};
use super::relational_support_planner::{
    prove_relational_case_image_injectivity, RelationalCaseImageInjectivityProofError,
    RelationalObligationActivation, RelationalRootObligationPlan,
    RelationalStagedObligationDescriptor, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::support_cell::{
    relational_case_chunk_partition_gateway, AdmissionClassificationClaim, InjectiveMappingClaim,
    SupportCellError, SupportCellEvidence, SupportCellId, SupportCellObligation,
    SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceError, SupportEvidenceRecord, SupportObligationRecord,
    SupportObligationRefinement,
};

/// One checked-executor slice/finalization bound to its canonical subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedSweepStepQuantum {
    partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    interval_start: u128,
    interval_end_exclusive: u128,
    slice_artifact_id: Option<RelationalClassifiedChunkSliceId>,
    evaluated_member_count: u16,
    classified_artifact_id: Option<RelationalClassifiedChunkArtifactId>,
}

impl RelationalClassifiedSweepStepQuantum {
    pub(crate) const fn partition_artifact_id(self) -> RelationalCaseChunkPartitionArtifactId {
        self.partition_artifact_id
    }

    pub(crate) const fn chunk_id(self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn chunk_ordinal(self) -> u128 {
        self.chunk_ordinal
    }

    pub(crate) const fn interval_start(self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn slice_artifact_id(self) -> Option<RelationalClassifiedChunkSliceId> {
        self.slice_artifact_id
    }

    pub(crate) const fn evaluated_member_count(self) -> Option<NonZeroU16> {
        NonZeroU16::new(self.evaluated_member_count)
    }

    pub(crate) const fn classified_artifact_id(
        self,
    ) -> Option<RelationalClassifiedChunkArtifactId> {
        self.classified_artifact_id
    }
}

/// One head-bound, unapplied classified-sweep event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedSweepStepBatch {
    expected_sequence: u64,
    expected_head: RelationalJournalHead,
    quantum: RelationalClassifiedSweepStepQuantum,
    events: Box<[RelationalJournalEvent]>,
}

impl RelationalClassifiedSweepStepBatch {
    pub(crate) const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub(crate) const fn expected_head(&self) -> RelationalJournalHead {
        self.expected_head
    }

    pub(crate) const fn quantum(&self) -> RelationalClassifiedSweepStepQuantum {
        self.quantum
    }

    pub(crate) fn events(&self) -> &[RelationalJournalEvent] {
        &self.events
    }

    pub(crate) fn into_events(self) -> Box<[RelationalJournalEvent]> {
        self.events
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassifiedSweepStepOutcome {
    Emitted(RelationalClassifiedSweepStepBatch),
    /// Every canonical child has a durable classified-chunk cursor endpoint.
    /// This is only scheduler exhaustion: a later layer must still establish
    /// source-image and selected-case obligations before closing support.
    ExhaustedAwaitingClosure {
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_count: usize,
        coordinate_count: u128,
    },
}

/// Checked-query-bound scheduler for one proper canonical chunk partition.
pub(crate) struct RelationalClassifiedSweepStepDriver<'query> {
    // This is a shallow copy of the checked view's producer-owned references,
    // not a clone of its IR. The interpreter runtime keys expressions by the
    // addresses in that original heap-stable query, so every classified step
    // must evaluate those exact Expr nodes.
    checked: CheckedExploreQueryView<'query>,
    support_plan: &'query RelationalSupportPlan,
    expected_root_injectivity: SupportCellEvidence<InjectiveMappingClaim>,
    partition: RelationalCaseChunkPartition,
    root_admission_obligation_id: SupportProofObligationId,
    root_admission_refinement: SupportObligationRefinement,
    /// Optional process-local accelerator. It is absent from every support,
    /// transcript and journal identity; the host falls back atomically to the
    /// checked interpreter whenever the sidecar is unavailable.
    native_classifier: Option<RelationalNativeClassifierV2>,
}

impl<'query> RelationalClassifiedSweepStepDriver<'query> {
    pub(crate) fn from_checked(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
    ) -> Result<Self, RelationalClassifiedSweepStepDriverError> {
        Self::from_checked_with_native_classifier(checked, support_plan, None)
    }

    pub(crate) fn from_checked_with_native_classifier(
        checked: &'query CheckedExploreQueryView<'_>,
        support_plan: &'query RelationalSupportPlan,
        native_classifier: Option<RelationalNativeClassifierV2>,
    ) -> Result<Self, RelationalClassifiedSweepStepDriverError> {
        checked
            .closed_query
            .validate()
            .map_err(RelationalClassifiedSweepStepDriverError::InvalidQuery)?;
        if !support_plan.validate_root()
            || support_plan.relation_id() != checked.relation_id()
            || support_plan.admission_id() != checked.admission_id()
            || support_plan.question_id() != checked.question_id()
        {
            return Err(RelationalClassifiedSweepStepDriverError::SupportPlanScopeMismatch);
        }

        let case_image_proof = prove_relational_case_image_injectivity(support_plan)?;
        let partition = match plan_relational_bounded_case_chunks(support_plan, &case_image_proof)?
        {
            RelationalCaseChunkPlanningOutcome::Partitioned(partition) => partition,
            RelationalCaseChunkPlanningOutcome::AlreadyBounded {
                root_cell_id,
                cardinality,
            } => {
                return Err(RelationalClassifiedSweepStepDriverError::AlreadyBounded {
                    root_cell_id,
                    cardinality,
                });
            }
            RelationalCaseChunkPlanningOutcome::Unsupported(reason) => {
                return Err(RelationalClassifiedSweepStepDriverError::UnsupportedPartition(reason));
            }
        };

        let root_admission = root_admission_obligation(support_plan)?;
        let root_admission_obligation_id = root_admission.id();
        let root_record = SupportObligationRecord::Admission(root_admission);
        let child_admissions = partition
            .chunks()
            .iter()
            .map(|chunk| {
                SupportCellObligation::new(
                    chunk.cell(),
                    AdmissionClassificationClaim::new(support_plan.admission_id()),
                )
                .map(SupportObligationRecord::Admission)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let root_admission_refinement = SupportObligationRefinement::new(
            &root_record,
            partition.certificate(),
            child_admissions.iter(),
        )?;

        Ok(Self {
            checked: *checked,
            support_plan,
            expected_root_injectivity: case_image_proof.injectivity().clone(),
            partition,
            root_admission_obligation_id,
            root_admission_refinement,
            native_classifier,
        })
    }

    pub(crate) const fn partition_artifact(&self) -> &RelationalCaseChunkPartitionArtifact {
        self.partition.artifact()
    }

    /// Evaluate and propose at most `max_members` from the next canonical
    /// chunk. The slice checkpoint is operational and replayable. If it closes
    /// the chunk, the batch appends the canonical whole-chunk evidence second;
    /// either proper crash prefix remains resumable.
    pub(crate) fn step<R: RelationalExpressionRuntime>(
        &self,
        journal: &RelationalJournal,
        max_members: NonZeroU16,
        runtime: &mut R,
    ) -> Result<RelationalClassifiedSweepStepOutcome, RelationalClassifiedSweepStepDriverError>
    {
        let checked = &self.checked;
        let view = journal.scheduler_view()?;
        self.validate_scope(&checked, view)?;

        let durable_root_injectivity =
            match view.support_evidence_record(self.expected_root_injectivity.id()) {
                Some(SupportEvidenceRecord::Injectivity(evidence))
                    if evidence == &self.expected_root_injectivity =>
                {
                    evidence
                }
                Some(_) => {
                    return Err(
                        RelationalClassifiedSweepStepDriverError::RootInjectivityEvidenceMismatch,
                    );
                }
                None => {
                    return Err(
                        RelationalClassifiedSweepStepDriverError::RootInjectivityEvidenceMissing,
                    );
                }
            };
        let verified_partition = view
            .verified_case_chunk_partition()
            .ok_or(RelationalClassifiedSweepStepDriverError::CanonicalPartitionMismatch)?;
        if verified_partition.artifact().id() != self.partition.artifact().id()
            || verified_partition.artifact().plan_root() != self.support_plan.root()
            || verified_partition.artifact().relation_id() != checked.relation_id()
            || verified_partition.artifact().admission_id() != checked.admission_id()
            || verified_partition.artifact().question_id() != checked.question_id()
            || verified_partition.durable_root_injectivity_evidence_id()
                != durable_root_injectivity.id()
            || verified_partition.durable_root_injectivity_receipt_id()
                != durable_root_injectivity.receipt().id()
        {
            return Err(RelationalClassifiedSweepStepDriverError::CanonicalPartitionMismatch);
        }

        match view.support_refinement_for_parent(self.root_admission_obligation_id) {
            Some(durable) if durable.id() == self.root_admission_refinement.id() => {}
            Some(_) => {
                return Err(
                    RelationalClassifiedSweepStepDriverError::RootAdmissionRefinementMismatch,
                );
            }
            None => {
                return Err(
                    RelationalClassifiedSweepStepDriverError::RootAdmissionRefinementMissing,
                );
            }
        }

        let artifact = verified_partition.artifact();
        let root_cell = self
            .support_plan
            .cases()
            .cell()
            .ok_or(RelationalClassifiedSweepStepDriverError::RootCellMissing)?;
        if root_cell.id() != artifact.root_cell_id() {
            return Err(RelationalClassifiedSweepStepDriverError::CanonicalPartitionMismatch);
        }
        let progress = view
            .classified_sweep_progress()
            .ok_or(RelationalClassifiedSweepStepDriverError::ClassifiedProgressMissing)?;
        if progress.partition_artifact_id() != artifact.id()
            || progress.root_cell_id() != root_cell.id()
            || progress.root_materializer_id() != root_cell.materializer_id()
            || progress.interval_start() != artifact.interval_start()
            || progress.interval_end_exclusive() != artifact.interval_end_exclusive()
            || u128::try_from(progress.accepted_chunk_count()).ok()
                != Some(progress.next_chunk_ordinal())
        {
            return Err(RelationalClassifiedSweepStepDriverError::ClassifiedProgressMismatch);
        }
        let coordinate_count = artifact
            .interval_end_exclusive()
            .checked_sub(artifact.interval_start())
            .ok_or(RelationalClassifiedSweepStepDriverError::CursorBoundsMismatch)?;
        let next_coordinate_ordinal = progress.next_coordinate_ordinal();
        if next_coordinate_ordinal > coordinate_count {
            return Err(RelationalClassifiedSweepStepDriverError::CursorBoundsMismatch);
        }
        match (
            progress.last_artifact_id(),
            view.support_latest_materialization_cursor(root_cell.id()),
        ) {
            (None, None) if next_coordinate_ordinal == 0 => {}
            (Some(last_artifact_id), Some(cursor)) => {
                cursor.validate_for(root_cell)?;
                let expected_checkpoint = last_artifact_id.bytes();
                if cursor.next_coordinate_ordinal() != next_coordinate_ordinal
                    || cursor.checkpoint() != expected_checkpoint.as_slice()
                {
                    return Err(
                        RelationalClassifiedSweepStepDriverError::ClassifiedCursorMirrorMismatch,
                    );
                }
            }
            (Some(_), None) => {
                return Err(
                    RelationalClassifiedSweepStepDriverError::ClassifiedCursorMirrorMissing,
                );
            }
            (None, Some(_)) | (None, None) => {
                return Err(
                    RelationalClassifiedSweepStepDriverError::ClassifiedCursorMirrorMismatch,
                );
            }
        }

        let chunks = verified_partition.partition().chunks();
        let next_chunk_ordinal = usize::try_from(progress.next_chunk_ordinal())
            .map_err(|_| RelationalClassifiedSweepStepDriverError::ClassifiedProgressMismatch)?;
        if next_coordinate_ordinal == coordinate_count && next_chunk_ordinal == chunks.len() {
            return Ok(
                RelationalClassifiedSweepStepOutcome::ExhaustedAwaitingClosure {
                    partition_artifact_id: artifact.id(),
                    chunk_count: chunks.len(),
                    coordinate_count,
                },
            );
        }
        let chunk = chunks
            .get(next_chunk_ordinal)
            .ok_or(RelationalClassifiedSweepStepDriverError::ClassifiedProgressMismatch)?;
        let relative_chunk_start = chunk
            .descriptor()
            .interval_start()
            .checked_sub(artifact.interval_start())
            .ok_or(RelationalClassifiedSweepStepDriverError::ClassifiedProgressMismatch)?;
        if chunk.descriptor().ordinal() != progress.next_chunk_ordinal()
            || relative_chunk_start != next_coordinate_ordinal
        {
            return Err(RelationalClassifiedSweepStepDriverError::ClassifiedProgressMismatch);
        }
        let expected_chunk_injectivity = relational_case_chunk_partition_gateway::injectivity(
            verified_partition,
            next_chunk_ordinal,
        )?;
        let durable_chunk_injectivity =
            match view.support_evidence_record(expected_chunk_injectivity.id()) {
                Some(SupportEvidenceRecord::Injectivity(evidence))
                    if evidence == &expected_chunk_injectivity =>
                {
                    evidence
                }
                Some(_) => {
                    return Err(
                        RelationalClassifiedSweepStepDriverError::ChunkInjectivityEvidenceMismatch {
                            chunk_id: chunk.descriptor().id(),
                        },
                    );
                }
                None => {
                    return Err(
                        RelationalClassifiedSweepStepDriverError::ChunkInjectivityEvidenceMissing {
                            chunk_id: chunk.descriptor().id(),
                        },
                    );
                }
            };

        let prior = view.classified_chunk_accumulator();
        if prior.is_some_and(|accumulator| accumulator.is_complete()) {
            let classified = finalize_relational_classified_case_chunk(
                self.support_plan,
                verified_partition,
                next_chunk_ordinal,
                durable_chunk_injectivity,
                prior.expect("the classified accumulator was present"),
            )?;
            let classified_artifact = classified.artifact().clone();
            let quantum = RelationalClassifiedSweepStepQuantum {
                partition_artifact_id: artifact.id(),
                chunk_id: chunk.descriptor().id(),
                chunk_ordinal: chunk.descriptor().ordinal(),
                interval_start: chunk.descriptor().interval_start(),
                interval_end_exclusive: chunk.descriptor().interval_end_exclusive(),
                slice_artifact_id: None,
                evaluated_member_count: 0,
                classified_artifact_id: Some(classified_artifact.id()),
            };
            return Ok(RelationalClassifiedSweepStepOutcome::Emitted(
                RelationalClassifiedSweepStepBatch {
                    expected_sequence: view.sequence(),
                    expected_head: view.head(),
                    quantum,
                    events: vec![
                        RelationalJournalEvent::relational_classified_chunk_accepted(
                            classified_artifact,
                        ),
                    ]
                    .into_boxed_slice(),
                },
            ));
        }

        let slice = match self.native_classifier.as_ref() {
            Some(native_classifier) => {
                let mut backend =
                    RelationalNativeClassifierFallbackBackendV2::new(native_classifier.clone());
                classify_relational_case_chunk_slice_with_backend(
                    &checked,
                    self.support_plan,
                    verified_partition,
                    next_chunk_ordinal,
                    durable_chunk_injectivity,
                    prior,
                    max_members,
                    runtime,
                    &mut backend,
                )?
            }
            None => classify_relational_case_chunk_slice(
                &checked,
                self.support_plan,
                verified_partition,
                next_chunk_ordinal,
                durable_chunk_injectivity,
                prior,
                max_members,
                runtime,
            )?,
        };
        let evaluated_member_count = slice.evaluated_member_count();
        let (slice_artifact, accumulator) = slice.into_parts();
        let slice_artifact_id = slice_artifact.id();
        let mut events = vec![
            RelationalJournalEvent::relational_classified_chunk_slice_checkpointed(slice_artifact),
        ];
        let classified_artifact_id = if accumulator.is_complete() {
            let classified = finalize_relational_classified_case_chunk(
                self.support_plan,
                verified_partition,
                next_chunk_ordinal,
                durable_chunk_injectivity,
                &accumulator,
            )?;
            let classified_artifact = classified.artifact().clone();
            let id = classified_artifact.id();
            events.push(
                RelationalJournalEvent::relational_classified_chunk_accepted(classified_artifact),
            );
            Some(id)
        } else {
            None
        };
        let quantum = RelationalClassifiedSweepStepQuantum {
            partition_artifact_id: artifact.id(),
            chunk_id: chunk.descriptor().id(),
            chunk_ordinal: chunk.descriptor().ordinal(),
            interval_start: chunk.descriptor().interval_start(),
            interval_end_exclusive: chunk.descriptor().interval_end_exclusive(),
            slice_artifact_id: Some(slice_artifact_id),
            evaluated_member_count: evaluated_member_count.get(),
            classified_artifact_id,
        };
        Ok(RelationalClassifiedSweepStepOutcome::Emitted(
            RelationalClassifiedSweepStepBatch {
                expected_sequence: view.sequence(),
                expected_head: view.head(),
                quantum,
                events: events.into_boxed_slice(),
            },
        ))
    }

    fn validate_scope(
        &self,
        checked: &CheckedExploreQueryView<'_>,
        view: RelationalSchedulerView<'_>,
    ) -> Result<(), RelationalClassifiedSweepStepDriverError> {
        let contract = view.contract();
        if contract.relation_id() != checked.relation_id()
            || contract.admission_id() != checked.admission_id()
            || contract.question_id() != checked.question_id()
        {
            return Err(RelationalClassifiedSweepStepDriverError::JournalScopeMismatch);
        }
        match view.support_plan_root() {
            Some(actual) if actual == self.support_plan.root() => {}
            Some(actual) => {
                return Err(
                    RelationalClassifiedSweepStepDriverError::SupportPlanRootMismatch {
                        expected: self.support_plan.root(),
                        actual,
                    },
                );
            }
            None => {
                return Err(RelationalClassifiedSweepStepDriverError::SupportPlanMissing);
            }
        }
        if view.source_traversal_is_started() {
            return Err(RelationalClassifiedSweepStepDriverError::SourceTraversalAlreadyStarted);
        }
        Ok(())
    }
}

fn root_admission_obligation(
    plan: &RelationalSupportPlan,
) -> Result<
    SupportCellObligation<AdmissionClassificationClaim>,
    RelationalClassifiedSweepStepDriverError,
> {
    let RelationalRootObligationPlan::CellBacked {
        root_cell_id,
        descriptors,
    } = plan.root_obligations()
    else {
        return Err(RelationalClassifiedSweepStepDriverError::RootAdmissionObligationMissing);
    };
    let mut root_admission = None;
    for descriptor in descriptors {
        let RelationalStagedObligationDescriptor::Root {
            activation: RelationalObligationActivation::RootCasePopulation,
            obligation: SupportObligationRecord::Admission(obligation),
        } = descriptor
        else {
            continue;
        };
        if obligation.cell_id() != *root_cell_id
            || obligation.claim().admission_id() != plan.admission_id()
        {
            return Err(RelationalClassifiedSweepStepDriverError::RootAdmissionScopeMismatch);
        }
        if root_admission.replace(obligation.clone()).is_some() {
            return Err(RelationalClassifiedSweepStepDriverError::MultipleRootAdmissions);
        }
    }
    root_admission.ok_or(RelationalClassifiedSweepStepDriverError::RootAdmissionObligationMissing)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassifiedSweepStepDriverError {
    InvalidQuery(String),
    SupportPlanScopeMismatch,
    AlreadyBounded {
        root_cell_id: SupportCellId,
        cardinality: u128,
    },
    UnsupportedPartition(RelationalCaseChunkUnsupported),
    RootCellMissing,
    RootAdmissionObligationMissing,
    RootAdmissionScopeMismatch,
    MultipleRootAdmissions,
    JournalScopeMismatch,
    SupportPlanMissing,
    SupportPlanRootMismatch {
        expected: RelationalSupportPlanRoot,
        actual: RelationalSupportPlanRoot,
    },
    SourceTraversalAlreadyStarted,
    RootInjectivityEvidenceMissing,
    RootInjectivityEvidenceMismatch,
    CanonicalPartitionMismatch,
    RootAdmissionRefinementMissing,
    RootAdmissionRefinementMismatch,
    ClassifiedProgressMissing,
    ClassifiedProgressMismatch,
    ClassifiedCursorMirrorMissing,
    ClassifiedCursorMirrorMismatch,
    CursorBoundsMismatch,
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
    SupportCell(SupportCellError),
    SupportEvidence(SupportEvidenceError),
}

impl fmt::Display for RelationalClassifiedSweepStepDriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(formatter, "invalid checked query: {message}"),
            Self::SupportPlanScopeMismatch => formatter
                .write_str("classified-sweep driver support plan does not match the checked query"),
            Self::AlreadyBounded {
                root_cell_id,
                cardinality,
            } => write!(
                formatter,
                "classified-sweep driver requires a proper partition; root {root_cell_id:?} is already bounded at {cardinality} coordinates"
            ),
            Self::UnsupportedPartition(reason) => write!(
                formatter,
                "classified-sweep driver does not support the canonical root shape: {reason:?}"
            ),
            Self::RootCellMissing => {
                formatter.write_str("classified-sweep support plan has no case root cell")
            }
            Self::RootAdmissionObligationMissing => formatter.write_str(
                "classified-sweep support plan has no root admission obligation",
            ),
            Self::RootAdmissionScopeMismatch => formatter.write_str(
                "classified-sweep root admission obligation has the wrong cell or admission scope",
            ),
            Self::MultipleRootAdmissions => formatter
                .write_str("classified-sweep support plan has multiple root admission obligations"),
            Self::JournalScopeMismatch => {
                formatter.write_str("classified-sweep journal contract does not match the query")
            }
            Self::SupportPlanMissing => {
                formatter.write_str("classified-sweep journal has no registered support plan")
            }
            Self::SupportPlanRootMismatch { expected, actual } => write!(
                formatter,
                "classified-sweep support-plan root mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::SourceTraversalAlreadyStarted => formatter.write_str(
                "classified sweep cannot interleave with the ordinary source traversal",
            ),
            Self::RootInjectivityEvidenceMissing => formatter.write_str(
                "classified sweep requires the exact durable root injectivity evidence",
            ),
            Self::RootInjectivityEvidenceMismatch => formatter.write_str(
                "durable root injectivity evidence differs from the checked producer proof",
            ),
            Self::CanonicalPartitionMismatch => formatter.write_str(
                "reconstructed classified-sweep partition differs from the retained canonical proposal",
            ),
            Self::RootAdmissionRefinementMissing => formatter.write_str(
                "classified sweep requires the accepted root-to-chunk admission refinement",
            ),
            Self::RootAdmissionRefinementMismatch => formatter.write_str(
                "durable root admission refinement differs from the canonical chunk cover",
            ),
            Self::ClassifiedProgressMissing => formatter.write_str(
                "classified-sweep partition has no typed durable progress authority",
            ),
            Self::ClassifiedProgressMismatch => formatter.write_str(
                "typed classified progress is not the contiguous prefix of the canonical partition",
            ),
            Self::ClassifiedCursorMirrorMissing => formatter.write_str(
                "typed classified progress has no matching operational root cursor",
            ),
            Self::ClassifiedCursorMirrorMismatch => formatter.write_str(
                "operational root cursor does not mirror the typed classified progress endpoint",
            ),
            Self::CursorBoundsMismatch => formatter.write_str(
                "classified-sweep root cursor falls outside the canonical coordinate interval",
            ),
            Self::ChunkInjectivityEvidenceMissing { chunk_id } => write!(
                formatter,
                "classified chunk {chunk_id:?} has no exact durable restricted-injectivity evidence"
            ),
            Self::ChunkInjectivityEvidenceMismatch { chunk_id } => write!(
                formatter,
                "classified chunk {chunk_id:?} durable injectivity differs from the canonical restriction"
            ),
            Self::Journal(error) => fmt::Display::fmt(error, formatter),
            Self::CaseImageProof(error) => fmt::Display::fmt(error, formatter),
            Self::ChunkPartition(error) => fmt::Display::fmt(error, formatter),
            Self::ClassifiedSweep(error) => fmt::Display::fmt(error, formatter),
            Self::SupportCell(error) => fmt::Display::fmt(error, formatter),
            Self::SupportEvidence(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for RelationalClassifiedSweepStepDriverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Journal(error) => Some(error),
            Self::CaseImageProof(error) => Some(error),
            Self::ChunkPartition(error) => Some(error),
            Self::ClassifiedSweep(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            Self::SupportEvidence(error) => Some(error),
            _ => None,
        }
    }
}

macro_rules! driver_error_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for RelationalClassifiedSweepStepDriverError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

driver_error_from!(RelationalJournalError, Journal);
driver_error_from!(RelationalCaseImageInjectivityProofError, CaseImageProof);
driver_error_from!(RelationalCaseChunkPartitionError, ChunkPartition);
driver_error_from!(RelationalClassifiedSweepError, ClassifiedSweep);
driver_error_from!(SupportCellError, SupportCell);
driver_error_from!(SupportEvidenceError, SupportEvidence);
