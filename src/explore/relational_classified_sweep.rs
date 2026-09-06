//! Exhaustive checked classification of one bounded mapped-case chunk.
//!
//! This is the exact concrete fallback beneath later symbolic classifiers. It
//! visits every coordinate in a canonical chunk through the ordinary checked
//! FROM/TO/WHERE/FIND runtime, commits the complete per-coordinate transcript,
//! and coalesces only adjacent equal outcomes. Nonselected and rejected cases
//! therefore become small support intervals without ever being represented by
//! invented `CaseId`s. A selected interval remains an exact materialization
//! obligation for the later sparse-case driver.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU16;

use sha2::{Digest, Sha256};

use crate::CheckedExploreQueryView;

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
use super::relational_bounded_chunk_partition::{
    decode_relational_case_chunk_finite_ordinals, derive_relational_case_chunk_subinterval_cell,
    RelationalCaseChunk, RelationalCaseChunkId, RelationalCaseChunkPartition,
    RelationalCaseChunkPartitionArtifactId, RelationalCaseChunkPartitionError,
    RelationalCaseChunkShape, VerifiedRelationalCaseChunkPartition,
    RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1, RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES,
};
use super::relational_case_executor::{
    RelationalCaseExecutor, RelationalCaseExecutorError, RelationalConcreteCase,
    RelationalQuestionEvaluationPlan,
};
use super::relational_classification_capsule::FrozenClassificationQuestionSet;
use super::relational_executor::{
    RelationalCompletedSource, RelationalExpressionRuntime, RelationalSourceEnumerator,
    RelationalSourceExecutorError,
};
use super::relational_support_planner::{
    RelationalBindingStage, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::support_cell::{
    relational_case_chunk_partition_gateway, AdmissionClassificationClaim, ExactCardinalityClaim,
    InjectiveMappingClaim, SelectionClassificationClaim, SupportCell, SupportCellClaim,
    SupportCellError, SupportCellEvidence, SupportCellId, SupportCellObligation,
    SupportMaterializerId, SupportPartitionCertificate, SupportPartitionId,
    SupportProofObligationId,
};

pub(crate) const RELATIONAL_CLASSIFIED_CHUNK_VERSION: u32 = 4;
pub(crate) const RELATIONAL_CLASSIFIED_CHUNK_SLICE_VERSION: u32 = 3;

const CLASSIFIED_RUN_ID_V2: &[u8] = b"futuruna.explore.relational-classified-run.id.v2";
const CLASSIFIED_CHUNK_ARTIFACT_ID_V2: &[u8] =
    b"futuruna.explore.relational-classified-chunk.artifact.v2";
const CLASSIFIED_CHUNK_TRANSCRIPT_GENESIS_V3: &[u8] =
    b"futuruna.explore.relational-classified-chunk.transcript-genesis.v3";
const CLASSIFIED_CHUNK_TRANSCRIPT_MEMBER_V3: &[u8] =
    b"futuruna.explore.relational-classified-chunk.transcript-member.v3";
const CLASSIFIED_CHUNK_SLICE_ID_V2: &[u8] =
    b"futuruna.explore.relational-classified-chunk.slice-id.v2";
const CLASSIFIED_CHUNK_EVIDENCE_V2: &[u8] =
    b"futuruna.explore.relational-classified-chunk.evidence.v2";

/// Packed decisions for one admitted case in the exact canonical QuestionId
/// order bound by its enclosing frozen question set.
///
/// Bit `i` is one exactly evaluated FIND decision: one means selected and
/// zero means not selected. The byte length is exactly `ceil(Q / 8)` and all
/// unused high bits in the final byte are zero, so one logical vector has one
/// wire representation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalQuestionDecisionMask {
    bytes: Box<[u8]>,
    question_count: usize,
}

impl RelationalQuestionDecisionMask {
    pub(crate) fn from_ordered_decisions(
        question_set: &FrozenClassificationQuestionSet,
        decisions: impl IntoIterator<Item = (QuestionId, SelectionDecision)>,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        if !question_set.validate_identity() {
            return Err(RelationalClassifiedSweepError::InvalidQuestionSet);
        }
        let mut bytes = vec![0; decision_mask_byte_len(question_set.question_ids().len())];
        let mut decisions = decisions.into_iter();
        for (index, expected_question_id) in question_set.question_ids().iter().copied().enumerate()
        {
            let Some((actual_question_id, decision)) = decisions.next() else {
                return Err(RelationalClassifiedSweepError::QuestionDecisionVectorMismatch);
            };
            if actual_question_id != expected_question_id {
                return Err(RelationalClassifiedSweepError::QuestionDecisionVectorMismatch);
            }
            if decision == SelectionDecision::Selected {
                bytes[index / 8] |= 1 << (index % 8);
            }
        }
        if decisions.next().is_some() {
            return Err(RelationalClassifiedSweepError::QuestionDecisionVectorMismatch);
        }
        Ok(Self {
            bytes: bytes.into_boxed_slice(),
            question_count: question_set.question_ids().len(),
        })
    }

    pub(super) fn restore_from_journal_codec(
        bytes: Box<[u8]>,
        question_count: usize,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        let mask = Self {
            bytes,
            question_count,
        };
        mask.validate(question_count)?;
        Ok(mask)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn selection(&self, question_index: usize) -> Option<SelectionDecision> {
        if question_index >= self.question_count {
            return None;
        }
        let byte = *self.bytes.get(question_index / 8)?;
        Some(if byte & (1 << (question_index % 8)) == 0 {
            SelectionDecision::NotSelected
        } else {
            SelectionDecision::Selected
        })
    }

    pub(crate) fn any_selected(&self) -> bool {
        self.bytes.iter().any(|byte| *byte != 0)
    }

    fn validate(&self, question_count: usize) -> Result<(), RelationalClassifiedSweepError> {
        if self.question_count != question_count
            || self.bytes.len() != decision_mask_byte_len(question_count)
        {
            return Err(RelationalClassifiedSweepError::InvalidDecisionMask);
        }
        let used_final_bits = question_count % 8;
        if used_final_bits != 0
            && self
                .bytes
                .last()
                .is_some_and(|byte| byte & !((1u8 << used_final_bits) - 1) != 0)
        {
            return Err(RelationalClassifiedSweepError::InvalidDecisionMask);
        }
        Ok(())
    }
}

fn decision_mask_byte_len(question_count: usize) -> usize {
    question_count / 8 + usize::from(question_count % 8 != 0)
}

/// The complete WHERE/FIND outcome for one admitted or rejected case.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalClassifiedCaseOutcome {
    Rejected,
    Admitted(RelationalQuestionDecisionMask),
}

/// One host-materialized case presented read-only to an ordered classifier.
///
/// The host retains the source row, successor row, coordinate and every
/// semantic identity. A backend can inspect only the extensional transition
/// values and can return only one classification outcome at this position in
/// the batch. In particular, it cannot mint a `SourceKey`, `CaseId`, transcript
/// root, run, or journal artifact.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalOrderedClassificationSubject<'a> {
    source: &'a RelationalCompletedSource,
    case: &'a RelationalConcreteCase,
}

impl<'a> RelationalOrderedClassificationSubject<'a> {
    pub(crate) const fn new(
        source: &'a RelationalCompletedSource,
        case: &'a RelationalConcreteCase,
    ) -> Self {
        Self { source, case }
    }

    pub(crate) fn context(self) -> &'a super::ExploreValue {
        self.source.row().context()
    }

    pub(crate) fn before(self) -> &'a super::ExploreValue {
        self.source.row().before()
    }

    pub(crate) fn after(self) -> &'a super::ExploreValue {
        self.case.successor().after()
    }

    /// Return one checked source-prefix binding by authored binding index.
    /// Native classifier inputs are copied from this producer-owned prefix;
    /// they are operational values and never replace source/case identity.
    pub(crate) fn source_binding(self, binding_index: usize) -> Option<&'a super::ExploreValue> {
        self.source.prefix().values.get(binding_index)
    }

    /// Return the canonical producer-issued ordinal for one source binding.
    /// This is authenticated as part of the completed prefix and lets an
    /// accelerator consume a finite typed coordinate without re-ranking its
    /// semantic value through a second implementation.
    pub(crate) fn source_binding_canonical_ordinal(self, binding_index: usize) -> Option<u128> {
        let selection = self.source.prefix().selections.get(binding_index)?;
        (usize::try_from(selection.binding_index).ok() == Some(binding_index))
            .then_some(selection.canonical_ordinal)
    }
}

/// Ordered classification boundary beneath the canonical sweep host.
///
/// Outcomes are positionally paired with `subjects`; returning a different
/// count is rejected before any transcript or artifact is derived. The first
/// implementation remains the checked interpreter. A future native sidecar
/// can implement this boundary without receiving evidence-producing
/// authority.
pub(crate) trait RelationalOrderedClassificationBackend {
    fn classify_ordered_batch<R: RelationalExpressionRuntime>(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError>;
}

/// Host-owned access to the existing checked classifier.
///
/// A backend may use this fallback to obtain ordered outcomes, but it cannot
/// observe the canonical identities used by the transcript or artifacts.
pub(crate) struct RelationalCheckedClassificationContext<'executor, 'query, 'runtime, R> {
    executor: &'executor RelationalCaseExecutor<'query>,
    questions: &'executor RelationalQuestionEvaluationPlan,
    question_set: FrozenClassificationQuestionSet,
    runtime: &'runtime mut R,
}

impl<R: RelationalExpressionRuntime> RelationalCheckedClassificationContext<'_, '_, '_, R> {
    pub(crate) fn new<'executor, 'query, 'runtime>(
        executor: &'executor RelationalCaseExecutor<'query>,
        questions: &'executor RelationalQuestionEvaluationPlan,
        runtime: &'runtime mut R,
    ) -> Result<
        RelationalCheckedClassificationContext<'executor, 'query, 'runtime, R>,
        RelationalClassifiedSweepError,
    > {
        let question_set = FrozenClassificationQuestionSet::freeze(questions.question_ids())
            .map_err(|_| RelationalClassifiedSweepError::InvalidQuestionSet)?;
        Ok(RelationalCheckedClassificationContext {
            executor,
            questions,
            question_set,
            runtime,
        })
    }

    pub(crate) fn classify(
        &mut self,
        subject: RelationalOrderedClassificationSubject<'_>,
    ) -> Result<RelationalClassifiedCaseOutcome, RelationalClassifiedSweepError> {
        let classification = self.executor.classify(
            subject.source.row(),
            subject.case,
            self.questions,
            &mut *self.runtime,
        )?;
        match (
            classification.admission(),
            classification.question_evidence(),
        ) {
            (AdmissionDecision::Rejected, []) => Ok(RelationalClassifiedCaseOutcome::Rejected),
            (AdmissionDecision::Admitted, questions) => {
                RelationalQuestionDecisionMask::from_ordered_decisions(
                    &self.question_set,
                    questions
                        .iter()
                        .map(|question| (question.question_id(), question.decision())),
                )
                .map(RelationalClassifiedCaseOutcome::Admitted)
            }
            _ => Err(
                RelationalClassifiedSweepError::InvalidCheckedClassification(
                    classification.case_id(),
                ),
            ),
        }
    }
}

struct RelationalInterpreterOrderedClassificationBackend;

impl RelationalOrderedClassificationBackend for RelationalInterpreterOrderedClassificationBackend {
    fn classify_ordered_batch<R: RelationalExpressionRuntime>(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError> {
        subjects
            .iter()
            .copied()
            .map(|subject| checked.classify(subject))
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }
}

struct RelationalMaterializedClassificationSubject {
    coordinate: u128,
    source: RelationalCompletedSource,
    case: RelationalConcreteCase,
}

impl RelationalClassifiedCaseOutcome {
    pub(crate) const fn admission(&self) -> AdmissionDecision {
        match self {
            Self::Rejected => AdmissionDecision::Rejected,
            Self::Admitted(_) => AdmissionDecision::Admitted,
        }
    }

    pub(crate) fn selection(&self, question_index: usize) -> Option<SelectionDecision> {
        match self {
            Self::Rejected => None,
            Self::Admitted(mask) => mask.selection(question_index),
        }
    }

    pub(crate) fn any_selected(&self) -> bool {
        match self {
            Self::Rejected => false,
            Self::Admitted(mask) => mask.any_selected(),
        }
    }

    /// Adapt the retained single-question native V2 response tags into the
    /// plural host outcome. This is deliberately not the classified-artifact
    /// codec: journal V3 admits a full packed decision vector.
    pub(super) fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(Self::Rejected),
            0x02 => Some(Self::Admitted(RelationalQuestionDecisionMask {
                bytes: vec![0u8].into_boxed_slice(),
                question_count: 1,
            })),
            0x03 => Some(Self::Admitted(RelationalQuestionDecisionMask {
                bytes: vec![1u8].into_boxed_slice(),
                question_count: 1,
            })),
            _ => None,
        }
    }

    pub(crate) const fn canonical_tag(&self) -> u8 {
        match self {
            Self::Rejected => 0x01,
            Self::Admitted(_) => 0x02,
        }
    }

    pub(crate) fn decision_mask(&self) -> Option<&RelationalQuestionDecisionMask> {
        match self {
            Self::Rejected => None,
            Self::Admitted(mask) => Some(mask),
        }
    }

    fn validate(
        &self,
        question_set: &FrozenClassificationQuestionSet,
    ) -> Result<(), RelationalClassifiedSweepError> {
        if !question_set.validate_identity() {
            return Err(RelationalClassifiedSweepError::InvalidQuestionSet);
        }
        match self {
            Self::Rejected => Ok(()),
            Self::Admitted(mask) => mask.validate(question_set.question_ids().len()),
        }
    }
}

/// Boundary-independent prefix commitment for one canonical classified chunk.
///
/// The checked producer starts from a chunk-scoped genesis value and folds one
/// coordinate at a time. Operational slice boundaries are deliberately absent
/// from that fold, so every complete cover of the same checked transcript has
/// the same terminal root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalClassifiedChunkTranscriptRoot([u8; 32]);

impl RelationalClassifiedChunkTranscriptRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable content identity of one caller-bounded checked slice.
///
/// This identity is operational and may differ across schedules. It never
/// enters final classified-run or support-cell identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalClassifiedChunkSliceId([u8; 32]);

impl RelationalClassifiedChunkSliceId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One maximal homogeneous interval within a single operational slice.
/// Adjacent equal runs from separate slices are merged only in the accumulator,
/// before final semantic run IDs are derived.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunkSliceRun {
    interval_start: u128,
    interval_end_exclusive: u128,
    outcome: RelationalClassifiedCaseOutcome,
}

impl RelationalClassifiedChunkSliceRun {
    pub(super) fn restore_from_journal_codec(
        interval_start: u128,
        interval_end_exclusive: u128,
        outcome: RelationalClassifiedCaseOutcome,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        if interval_start >= interval_end_exclusive {
            return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
                "classified slice run is empty or reversed",
            ));
        }
        Ok(Self {
            interval_start,
            interval_end_exclusive,
            outcome,
        })
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn cardinality(&self) -> u128 {
        self.interval_end_exclusive - self.interval_start
    }

    pub(crate) const fn outcome(&self) -> &RelationalClassifiedCaseOutcome {
        &self.outcome
    }
}

/// Replayable checked-executor output for one nonempty contiguous subrange of
/// a canonical chunk. The artifact is sufficient to resume and merge checked
/// work, but is not itself final support-classification evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunkSliceArtifact {
    schema_version: u32,
    id: RelationalClassifiedChunkSliceId,
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_set: FrozenClassificationQuestionSet,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    chunk_cell_id: SupportCellId,
    chunk_materializer_id: SupportMaterializerId,
    chunk_interval_start: u128,
    chunk_interval_end_exclusive: u128,
    slice_interval_start: u128,
    slice_interval_end_exclusive: u128,
    predecessor_slice_id: Option<RelationalClassifiedChunkSliceId>,
    transcript_root_before: RelationalClassifiedChunkTranscriptRoot,
    transcript_root_after: RelationalClassifiedChunkTranscriptRoot,
    rejected_count: u128,
    admitted_count: u128,
    admitted_selected_counts: Box<[u128]>,
    runs: Box<[RelationalClassifiedChunkSliceRun]>,
}

impl RelationalClassifiedChunkSliceArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        schema_version: u32,
        id: RelationalClassifiedChunkSliceId,
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_set: FrozenClassificationQuestionSet,
        chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        chunk_cell_id: SupportCellId,
        chunk_materializer_id: SupportMaterializerId,
        chunk_interval_start: u128,
        chunk_interval_end_exclusive: u128,
        slice_interval_start: u128,
        slice_interval_end_exclusive: u128,
        predecessor_slice_id: Option<RelationalClassifiedChunkSliceId>,
        transcript_root_before: RelationalClassifiedChunkTranscriptRoot,
        transcript_root_after: RelationalClassifiedChunkTranscriptRoot,
        rejected_count: u128,
        admitted_count: u128,
        admitted_selected_counts: Box<[u128]>,
        runs: Box<[RelationalClassifiedChunkSliceRun]>,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        let artifact = Self {
            schema_version,
            id,
            plan_root,
            relation_id,
            admission_id,
            question_set,
            chunk_partition_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
            chunk_interval_start,
            chunk_interval_end_exclusive,
            slice_interval_start,
            slice_interval_end_exclusive,
            predecessor_slice_id,
            transcript_root_before,
            transcript_root_after,
            rejected_count,
            admitted_count,
            admitted_selected_counts,
            runs,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn id(&self) -> RelationalClassifiedChunkSliceId {
        self.id
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) fn question_set(&self) -> &FrozenClassificationQuestionSet {
        &self.question_set
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        self.question_set.question_ids()
    }

    pub(crate) fn question_index(&self, question_id: QuestionId) -> Option<usize> {
        self.question_ids().binary_search(&question_id).ok()
    }

    pub(crate) const fn chunk_partition_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.chunk_partition_id
    }

    pub(crate) const fn chunk_id(&self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn chunk_ordinal(&self) -> u128 {
        self.chunk_ordinal
    }

    pub(crate) const fn chunk_cell_id(&self) -> SupportCellId {
        self.chunk_cell_id
    }

    pub(crate) const fn chunk_materializer_id(&self) -> SupportMaterializerId {
        self.chunk_materializer_id
    }

    pub(crate) const fn chunk_interval_start(&self) -> u128 {
        self.chunk_interval_start
    }

    pub(crate) const fn chunk_interval_end_exclusive(&self) -> u128 {
        self.chunk_interval_end_exclusive
    }

    pub(crate) const fn slice_interval_start(&self) -> u128 {
        self.slice_interval_start
    }

    pub(crate) const fn slice_interval_end_exclusive(&self) -> u128 {
        self.slice_interval_end_exclusive
    }

    pub(crate) const fn evaluated_case_count(&self) -> u128 {
        self.slice_interval_end_exclusive - self.slice_interval_start
    }

    pub(crate) const fn predecessor_slice_id(&self) -> Option<RelationalClassifiedChunkSliceId> {
        self.predecessor_slice_id
    }

    pub(crate) const fn transcript_root_before(&self) -> RelationalClassifiedChunkTranscriptRoot {
        self.transcript_root_before
    }

    pub(crate) const fn transcript_root_after(&self) -> RelationalClassifiedChunkTranscriptRoot {
        self.transcript_root_after
    }

    pub(crate) const fn rejected_count(&self) -> u128 {
        self.rejected_count
    }

    pub(crate) const fn admitted_count(&self) -> u128 {
        self.admitted_count
    }

    pub(crate) fn admitted_selected_counts(&self) -> &[u128] {
        &self.admitted_selected_counts
    }

    pub(crate) fn admitted_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.question_index(question_id)
            .and_then(|index| self.admitted_selected_counts.get(index).copied())
    }

    pub(crate) fn admitted_not_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.admitted_selected_count(question_id)
            .and_then(|selected| self.admitted_count.checked_sub(selected))
    }

    pub(crate) fn runs(&self) -> &[RelationalClassifiedChunkSliceRun] {
        &self.runs
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RelationalClassifiedSweepError> {
        if self.schema_version != RELATIONAL_CLASSIFIED_CHUNK_SLICE_VERSION {
            return Err(
                RelationalClassifiedSweepError::UnsupportedSliceArtifactVersion(
                    self.schema_version,
                ),
            );
        }
        if !self.question_set.validate_identity()
            || self.admitted_selected_counts.len() != self.question_ids().len()
        {
            return Err(RelationalClassifiedSweepError::InvalidQuestionSet);
        }
        if self.chunk_interval_start >= self.chunk_interval_end_exclusive
            || self.slice_interval_start < self.chunk_interval_start
            || self.slice_interval_end_exclusive > self.chunk_interval_end_exclusive
            || self.slice_interval_start >= self.slice_interval_end_exclusive
            || self.runs.is_empty()
        {
            return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
                "classified slice or canonical chunk bounds are invalid",
            ));
        }
        let chunk_cardinality = self.chunk_interval_end_exclusive - self.chunk_interval_start;
        let slice_cardinality = self.evaluated_case_count();
        if chunk_cardinality > RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES
            || slice_cardinality > RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1
            || u128::try_from(self.runs.len())
                .ok()
                .is_none_or(|count| count > slice_cardinality)
        {
            return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
                "classified slice exceeds the canonical chunk or run bound",
            ));
        }

        let mut next_start = self.slice_interval_start;
        let mut rejected = 0u128;
        let mut admitted = 0u128;
        let mut admitted_selected = vec![0u128; self.question_ids().len()];
        for (index, run) in self.runs.iter().enumerate() {
            run.outcome.validate(&self.question_set)?;
            if run.interval_start != next_start
                || run.interval_end_exclusive > self.slice_interval_end_exclusive
                || !selected_run_within_quantum(
                    &run.outcome,
                    run.interval_start,
                    run.interval_end_exclusive,
                    self.chunk_interval_start,
                )
                || index
                    .checked_sub(1)
                    .and_then(|previous| self.runs.get(previous))
                    .is_some_and(|previous| {
                        can_merge_run_outcomes(
                            &previous.outcome,
                            &run.outcome,
                            run.interval_start,
                            self.chunk_interval_start,
                        )
                    })
            {
                return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
                    "classified slice runs are not a maximal contiguous cover",
                ));
            }
            accumulate_outcome_counts(
                &run.outcome,
                run.cardinality(),
                &mut rejected,
                &mut admitted,
                &mut admitted_selected,
            )?;
            next_start = run.interval_end_exclusive;
        }
        if next_start != self.slice_interval_end_exclusive
            || rejected != self.rejected_count
            || admitted != self.admitted_count
            || admitted_selected.as_slice() != self.admitted_selected_counts.as_ref()
            || rejected.checked_add(admitted) != Some(slice_cardinality)
        {
            return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
                "classified slice outcome counts do not conserve its interval",
            ));
        }
        if derive_slice_artifact_id(self) != self.id {
            return Err(RelationalClassifiedSweepError::SliceArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// Pure folded state for a contiguous prefix of one canonical chunk.
///
/// A durable layer should rebuild this value by replaying accepted slice
/// artifacts in order. Only a complete accumulator may mint final run cells and
/// semantic classification evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunkAccumulator {
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_set: FrozenClassificationQuestionSet,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    chunk_cell_id: SupportCellId,
    chunk_materializer_id: SupportMaterializerId,
    interval_start: u128,
    interval_end_exclusive: u128,
    next_coordinate: u128,
    transcript_root: RelationalClassifiedChunkTranscriptRoot,
    last_slice_id: Option<RelationalClassifiedChunkSliceId>,
    rejected_count: u128,
    admitted_count: u128,
    admitted_selected_counts: Vec<u128>,
    runs: Vec<RelationalClassifiedChunkSliceRun>,
}

impl RelationalClassifiedChunkAccumulator {
    pub(crate) const fn chunk_partition_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.chunk_partition_id
    }

    pub(crate) const fn chunk_id(&self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn chunk_ordinal(&self) -> u128 {
        self.chunk_ordinal
    }

    pub(crate) const fn chunk_cell_id(&self) -> SupportCellId {
        self.chunk_cell_id
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn next_coordinate(&self) -> u128 {
        self.next_coordinate
    }

    pub(crate) const fn evaluated_case_count(&self) -> u128 {
        self.next_coordinate - self.interval_start
    }

    pub(crate) const fn transcript_root(&self) -> RelationalClassifiedChunkTranscriptRoot {
        self.transcript_root
    }

    pub(crate) const fn last_slice_id(&self) -> Option<RelationalClassifiedChunkSliceId> {
        self.last_slice_id
    }

    pub(crate) const fn rejected_count(&self) -> u128 {
        self.rejected_count
    }

    pub(crate) const fn admitted_count(&self) -> u128 {
        self.admitted_count
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        self.question_set.question_ids()
    }

    pub(crate) fn admitted_selected_counts(&self) -> &[u128] {
        &self.admitted_selected_counts
    }

    pub(crate) fn admitted_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.question_ids()
            .binary_search(&question_id)
            .ok()
            .and_then(|index| self.admitted_selected_counts.get(index).copied())
    }

    pub(crate) fn admitted_not_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.admitted_selected_count(question_id)
            .and_then(|selected| self.admitted_count.checked_sub(selected))
    }

    pub(crate) fn runs(&self) -> &[RelationalClassifiedChunkSliceRun] {
        &self.runs
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.next_coordinate == self.interval_end_exclusive
    }
}

/// Fresh checked slice plus the exact folded prefix it produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunkSlice {
    artifact: RelationalClassifiedChunkSliceArtifact,
    accumulator: RelationalClassifiedChunkAccumulator,
}

impl RelationalClassifiedChunkSlice {
    pub(crate) const fn artifact(&self) -> &RelationalClassifiedChunkSliceArtifact {
        &self.artifact
    }

    pub(crate) const fn accumulator(&self) -> &RelationalClassifiedChunkAccumulator {
        &self.accumulator
    }

    pub(crate) fn evaluated_member_count(&self) -> NonZeroU16 {
        let count = u16::try_from(self.artifact.evaluated_case_count())
            .expect("a validated classified slice has at most 256 members");
        NonZeroU16::new(count).expect("a validated classified slice is nonempty")
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.accumulator.is_complete()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        RelationalClassifiedChunkSliceArtifact,
        RelationalClassifiedChunkAccumulator,
    ) {
        (self.artifact, self.accumulator)
    }
}

/// Canonical identity of one maximal homogeneous interval in a chunk.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalClassifiedRunId([u8; 32]);

impl RelationalClassifiedRunId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Durable description of one maximal homogeneous support interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedRunDescriptor {
    id: RelationalClassifiedRunId,
    ordinal: u16,
    cell_id: SupportCellId,
    interval_start: u128,
    interval_end_exclusive: u128,
    outcome: RelationalClassifiedCaseOutcome,
}

impl RelationalClassifiedRunDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        id: RelationalClassifiedRunId,
        ordinal: u16,
        cell_id: SupportCellId,
        interval_start: u128,
        interval_end_exclusive: u128,
        outcome: RelationalClassifiedCaseOutcome,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        if interval_start >= interval_end_exclusive {
            return Err(RelationalClassifiedSweepError::InvalidArtifactShape(
                "classified run is empty or reversed",
            ));
        }
        Ok(Self {
            id,
            ordinal,
            cell_id,
            interval_start,
            interval_end_exclusive,
            outcome,
        })
    }

    pub(crate) const fn id(&self) -> RelationalClassifiedRunId {
        self.id
    }

    pub(crate) const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn cardinality(&self) -> u128 {
        self.interval_end_exclusive - self.interval_start
    }

    pub(crate) const fn outcome(&self) -> &RelationalClassifiedCaseOutcome {
        &self.outcome
    }
}

/// Stable identity of one exhaustive classified-chunk producer artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalClassifiedChunkArtifactId([u8; 32]);

impl RelationalClassifiedChunkArtifactId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Replayable output of one exhaustive bounded evaluation.
///
/// Like concrete `AdmissionClassified` events, the outcome transcript is a
/// trusted checked-executor output. Replay verifies its complete structural
/// scope, interval conservation, canonical IDs, and proof bindings; it does
/// not rerun user code merely to read an authenticated journal prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunkArtifact {
    schema_version: u32,
    id: RelationalClassifiedChunkArtifactId,
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_set: FrozenClassificationQuestionSet,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    chunk_cell_id: SupportCellId,
    chunk_materializer_id: SupportMaterializerId,
    interval_start: u128,
    interval_end_exclusive: u128,
    evaluated_case_count: u128,
    evaluated_cases_root: [u8; 32],
    rejected_count: u128,
    admitted_count: u128,
    admitted_selected_counts: Box<[u128]>,
    runs: Box<[RelationalClassifiedRunDescriptor]>,
    partition_id: Option<SupportPartitionId>,
}

impl RelationalClassifiedChunkArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        schema_version: u32,
        id: RelationalClassifiedChunkArtifactId,
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_set: FrozenClassificationQuestionSet,
        chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        chunk_cell_id: SupportCellId,
        chunk_materializer_id: SupportMaterializerId,
        interval_start: u128,
        interval_end_exclusive: u128,
        evaluated_case_count: u128,
        evaluated_cases_root: [u8; 32],
        rejected_count: u128,
        admitted_count: u128,
        admitted_selected_counts: Box<[u128]>,
        runs: Box<[RelationalClassifiedRunDescriptor]>,
        partition_id: Option<SupportPartitionId>,
    ) -> Result<Self, RelationalClassifiedSweepError> {
        let artifact = Self {
            schema_version,
            id,
            plan_root,
            relation_id,
            admission_id,
            question_set,
            chunk_partition_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
            interval_start,
            interval_end_exclusive,
            evaluated_case_count,
            evaluated_cases_root,
            rejected_count,
            admitted_count,
            admitted_selected_counts,
            runs,
            partition_id,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn id(&self) -> RelationalClassifiedChunkArtifactId {
        self.id
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) fn question_set(&self) -> &FrozenClassificationQuestionSet {
        &self.question_set
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        self.question_set.question_ids()
    }

    pub(crate) fn question_index(&self, question_id: QuestionId) -> Option<usize> {
        self.question_ids().binary_search(&question_id).ok()
    }

    pub(crate) const fn chunk_partition_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.chunk_partition_id
    }

    pub(crate) const fn chunk_id(&self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn chunk_ordinal(&self) -> u128 {
        self.chunk_ordinal
    }

    pub(crate) const fn chunk_cell_id(&self) -> SupportCellId {
        self.chunk_cell_id
    }

    pub(crate) const fn chunk_materializer_id(&self) -> SupportMaterializerId {
        self.chunk_materializer_id
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn evaluated_case_count(&self) -> u128 {
        self.evaluated_case_count
    }

    pub(crate) const fn evaluated_cases_root(&self) -> [u8; 32] {
        self.evaluated_cases_root
    }

    pub(crate) const fn rejected_count(&self) -> u128 {
        self.rejected_count
    }

    pub(crate) const fn admitted_count(&self) -> u128 {
        self.admitted_count
    }

    pub(crate) fn admitted_selected_counts(&self) -> &[u128] {
        &self.admitted_selected_counts
    }

    pub(crate) fn admitted_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.question_index(question_id)
            .and_then(|index| self.admitted_selected_counts.get(index).copied())
    }

    pub(crate) fn admitted_not_selected_count(&self, question_id: QuestionId) -> Option<u128> {
        self.admitted_selected_count(question_id)
            .and_then(|selected| self.admitted_count.checked_sub(selected))
    }

    pub(crate) fn runs(&self) -> &[RelationalClassifiedRunDescriptor] {
        &self.runs
    }

    pub(crate) const fn partition_id(&self) -> Option<SupportPartitionId> {
        self.partition_id
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RelationalClassifiedSweepError> {
        if self.schema_version != RELATIONAL_CLASSIFIED_CHUNK_VERSION {
            return Err(RelationalClassifiedSweepError::UnsupportedArtifactVersion(
                self.schema_version,
            ));
        }
        if !self.question_set.validate_identity()
            || self.admitted_selected_counts.len() != self.question_ids().len()
        {
            return Err(RelationalClassifiedSweepError::InvalidQuestionSet);
        }
        if self.interval_start >= self.interval_end_exclusive || self.runs.is_empty() {
            return Err(RelationalClassifiedSweepError::InvalidArtifactShape(
                "classified chunk or its run cover is empty",
            ));
        }
        let interval_cardinality = self.interval_end_exclusive - self.interval_start;
        if self.evaluated_case_count != interval_cardinality
            || interval_cardinality > RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES
            || self.runs.len() as u128 > interval_cardinality
        {
            return Err(RelationalClassifiedSweepError::InvalidArtifactShape(
                "classified chunk count or run bound is not canonical",
            ));
        }

        let mut next_start = self.interval_start;
        let mut rejected = 0u128;
        let mut admitted = 0u128;
        let mut admitted_selected = vec![0u128; self.question_ids().len()];
        for (index, run) in self.runs.iter().enumerate() {
            run.outcome.validate(&self.question_set)?;
            let expected_ordinal = u16::try_from(index).map_err(|_| {
                RelationalClassifiedSweepError::InvalidArtifactShape(
                    "classified run ordinal exceeds u16",
                )
            })?;
            if run.ordinal != expected_ordinal
                || run.interval_start != next_start
                || run.interval_end_exclusive > self.interval_end_exclusive
                || !selected_run_within_quantum(
                    &run.outcome,
                    run.interval_start,
                    run.interval_end_exclusive,
                    self.interval_start,
                )
                || index
                    .checked_sub(1)
                    .and_then(|previous| self.runs.get(previous))
                    .is_some_and(|previous| {
                        can_merge_run_outcomes(
                            &previous.outcome,
                            &run.outcome,
                            run.interval_start,
                            self.interval_start,
                        )
                    })
            {
                return Err(RelationalClassifiedSweepError::InvalidArtifactShape(
                    "classified runs are not a maximal contiguous cover",
                ));
            }
            let expected_run_id = derive_run_id(
                self.plan_root,
                self.chunk_partition_id,
                self.chunk_id,
                self.chunk_cell_id,
                self.chunk_materializer_id,
                run.ordinal,
                run.cell_id,
                run.interval_start,
                run.interval_end_exclusive,
                &run.outcome,
            );
            if run.id != expected_run_id {
                return Err(RelationalClassifiedSweepError::RunIdentityMismatch {
                    ordinal: run.ordinal,
                });
            }
            accumulate_outcome_counts(
                &run.outcome,
                run.cardinality(),
                &mut rejected,
                &mut admitted,
                &mut admitted_selected,
            )?;
            next_start = run.interval_end_exclusive;
        }
        if next_start != self.interval_end_exclusive
            || rejected != self.rejected_count
            || admitted != self.admitted_count
            || admitted_selected.as_slice() != self.admitted_selected_counts.as_ref()
            || rejected.checked_add(admitted) != Some(interval_cardinality)
            || (self.runs.len() == 1) != self.partition_id.is_none()
        {
            return Err(RelationalClassifiedSweepError::InvalidArtifactShape(
                "classified run counts, coverage, or partition shape disagree",
            ));
        }
        if derive_artifact_id(self) != self.id {
            return Err(RelationalClassifiedSweepError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// A worst-case structural codec fixture, deliberately not proof authority.
/// Its scope/cell IDs are synthetic; journal acceptance must still reverify
/// against an installed plan and durable injectivity evidence.
#[cfg(test)]
pub(super) fn alternating_maximum_page_codec_fixture() -> RelationalClassifiedChunkArtifact {
    use super::relation::FindPolarity;
    let relation_id = RelationId::from_canonical_semantic_preimage(b"maximum-page-codec");
    let admission_id = AdmissionId::from_canonical_admission_preimage(relation_id, b"admit");
    let question_id =
        QuestionId::from_canonical_find_preimage(admission_id, b"alternate", FindPolarity::All);
    let question_set = FrozenClassificationQuestionSet::freeze([question_id]).unwrap();
    let outcomes = [SelectionDecision::NotSelected, SelectionDecision::Selected].map(|decision| {
        RelationalClassifiedCaseOutcome::Admitted(
            RelationalQuestionDecisionMask::from_ordered_decisions(
                &question_set,
                [(question_id, decision)],
            )
            .unwrap(),
        )
    });
    let mut artifact = RelationalClassifiedChunkArtifact {
        schema_version: RELATIONAL_CLASSIFIED_CHUNK_VERSION,
        id: RelationalClassifiedChunkArtifactId([0; 32]),
        plan_root: RelationalSupportPlanRoot::from_journal_codec_bytes([1; 32]),
        relation_id,
        admission_id,
        question_set,
        chunk_partition_id: RelationalCaseChunkPartitionArtifactId::from_canonical_bytes([2; 32]),
        chunk_id: RelationalCaseChunkId::from_canonical_bytes([3; 32]),
        chunk_ordinal: 0,
        chunk_cell_id: SupportCellId::from_journal_codec_bytes([4; 32]),
        chunk_materializer_id: SupportMaterializerId::from_journal_codec_bytes([5; 32]),
        interval_start: 0,
        interval_end_exclusive: RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES,
        evaluated_case_count: RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES,
        evaluated_cases_root: [6; 32],
        rejected_count: 0,
        admitted_count: RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES,
        admitted_selected_counts: vec![RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES / 2]
            .into_boxed_slice(),
        runs: Box::new([]),
        partition_id: Some(SupportPartitionId::from_journal_codec_bytes([7; 32])),
    };
    artifact.runs = (0..RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES)
        .map(|index| {
            let mut cell_bytes = [8; 32];
            cell_bytes[..16].copy_from_slice(&index.to_be_bytes());
            let cell_id = SupportCellId::from_journal_codec_bytes(cell_bytes);
            let ordinal = u16::try_from(index).unwrap();
            let outcome = outcomes[index as usize % 2].clone();
            let id = derive_run_id(
                artifact.plan_root,
                artifact.chunk_partition_id,
                artifact.chunk_id,
                artifact.chunk_cell_id,
                artifact.chunk_materializer_id,
                ordinal,
                cell_id,
                index,
                index + 1,
                &outcome,
            );
            RelationalClassifiedRunDescriptor {
                id,
                ordinal,
                cell_id,
                interval_start: index,
                interval_end_exclusive: index + 1,
                outcome,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    artifact.id = derive_artifact_id(&artifact);
    artifact.validate_identity().unwrap();
    artifact
}

/// One run paired with its reconstructed support cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedRun {
    descriptor: RelationalClassifiedRunDescriptor,
    cell: SupportCell,
}

impl RelationalClassifiedRun {
    pub(crate) const fn descriptor(&self) -> &RelationalClassifiedRunDescriptor {
        &self.descriptor
    }

    pub(crate) const fn cell(&self) -> &SupportCell {
        &self.cell
    }
}

/// Typed receipt preimage consumed by the private support-cell gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedEvidenceBinding {
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl RelationalClassifiedEvidenceBinding {
    pub(crate) const fn obligation_id(self) -> SupportProofObligationId {
        self.obligation_id
    }

    pub(crate) const fn conclusion_digest(self) -> [u8; 32] {
        self.conclusion_digest
    }

    pub(crate) const fn proof_digest(self) -> [u8; 32] {
        self.proof_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedRunEvidenceBindings {
    injectivity: Option<RelationalClassifiedEvidenceBinding>,
    cardinality: RelationalClassifiedEvidenceBinding,
    admission: RelationalClassifiedEvidenceBinding,
    selections: Box<[RelationalClassifiedEvidenceBinding]>,
}

impl RelationalClassifiedRunEvidenceBindings {
    pub(crate) const fn injectivity(&self) -> Option<RelationalClassifiedEvidenceBinding> {
        self.injectivity
    }

    pub(crate) const fn cardinality(&self) -> RelationalClassifiedEvidenceBinding {
        self.cardinality
    }

    pub(crate) const fn admission(&self) -> RelationalClassifiedEvidenceBinding {
        self.admission
    }

    pub(crate) fn selections(&self) -> &[RelationalClassifiedEvidenceBinding] {
        &self.selections
    }

    pub(crate) fn selection(
        &self,
        question_index: usize,
    ) -> Option<RelationalClassifiedEvidenceBinding> {
        self.selections.get(question_index).copied()
    }
}

/// Opaque structural authority recovered only through the checked sweep
/// module. It binds the retained producer artifact to exact cells, a possible
/// run partition, and every typed evidence conclusion. Journal durability is
/// established separately by exact catalog lookups at acceptance time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalClassifiedChunk {
    artifact: RelationalClassifiedChunkArtifact,
    runs: Box<[RelationalClassifiedRun]>,
    partition: Option<SupportPartitionCertificate>,
    bindings: Box<[RelationalClassifiedRunEvidenceBindings]>,
}

impl VerifiedRelationalClassifiedChunk {
    pub(crate) const fn artifact(&self) -> &RelationalClassifiedChunkArtifact {
        &self.artifact
    }

    pub(crate) fn runs(&self) -> &[RelationalClassifiedRun] {
        &self.runs
    }

    pub(crate) const fn partition(&self) -> Option<&SupportPartitionCertificate> {
        self.partition.as_ref()
    }

    pub(crate) fn bindings(&self) -> &[RelationalClassifiedRunEvidenceBindings] {
        &self.bindings
    }

    pub(crate) fn run_and_bindings(
        &self,
        run_ordinal: usize,
    ) -> Option<(
        &RelationalClassifiedRun,
        &RelationalClassifiedRunEvidenceBindings,
    )> {
        Some((self.runs.get(run_ordinal)?, self.bindings.get(run_ordinal)?))
    }
}

/// Fresh checked-executor output before it is reduced to a retained artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassifiedChunk {
    verified: VerifiedRelationalClassifiedChunk,
}

impl RelationalClassifiedChunk {
    pub(crate) const fn verified(&self) -> &VerifiedRelationalClassifiedChunk {
        &self.verified
    }

    pub(crate) const fn artifact(&self) -> &RelationalClassifiedChunkArtifact {
        self.verified.artifact()
    }
}

/// Exhaustively evaluate one canonical chunk through the checked query.
///
/// This compatibility entry point streams bounded slices even for a larger
/// page. It produces the same final artifact as any contiguous slice schedule.
pub(crate) fn classify_relational_case_chunk<R: RelationalExpressionRuntime>(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_ordinal: usize,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    runtime: &mut R,
) -> Result<RelationalClassifiedChunk, RelationalClassifiedSweepError> {
    let member_limit = NonZeroU16::new(RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 as u16)
        .expect("the fixed execution quantum is nonzero and fits u16");
    let mut prior = None;
    loop {
        let slice = classify_relational_case_chunk_slice(
            checked,
            plan,
            verified_chunk_partition,
            chunk_ordinal,
            chunk_injectivity,
            prior.as_ref(),
            member_limit,
            runtime,
        )?;
        if slice.accumulator().is_complete() {
            return finalize_relational_classified_case_chunk(
                plan,
                verified_chunk_partition,
                chunk_ordinal,
                chunk_injectivity,
                slice.accumulator(),
            );
        }
        prior = Some(slice.accumulator().clone());
    }
}

/// Evaluate a caller-bounded nonempty contiguous slice of one canonical chunk.
///
/// `prior` is a folded prefix rebuilt from previously accepted slice artifacts.
/// The caller-selected bound is operational: it affects only the returned slice
/// artifact, never the complete chunk's run IDs, support cells, or evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn classify_relational_case_chunk_slice<R: RelationalExpressionRuntime>(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_ordinal: usize,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    prior: Option<&RelationalClassifiedChunkAccumulator>,
    max_members: NonZeroU16,
    runtime: &mut R,
) -> Result<RelationalClassifiedChunkSlice, RelationalClassifiedSweepError> {
    let mut backend = RelationalInterpreterOrderedClassificationBackend;
    classify_relational_case_chunk_slice_with_backend(
        checked,
        plan,
        verified_chunk_partition,
        chunk_ordinal,
        chunk_injectivity,
        prior,
        max_members,
        runtime,
        &mut backend,
    )
}

/// Internal classifier injection seam; all materialization and evidence stay
/// in this host function regardless of which ordered classifier is supplied.
#[allow(clippy::too_many_arguments)]
pub(crate) fn classify_relational_case_chunk_slice_with_backend<
    R: RelationalExpressionRuntime,
    B: RelationalOrderedClassificationBackend,
>(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_ordinal: usize,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    prior: Option<&RelationalClassifiedChunkAccumulator>,
    max_members: NonZeroU16,
    runtime: &mut R,
    backend: &mut B,
) -> Result<RelationalClassifiedChunkSlice, RelationalClassifiedSweepError> {
    let chunk_partition = verified_chunk_partition.partition();
    let chunk = chunk_partition
        .chunks()
        .get(chunk_ordinal)
        .ok_or(RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let expected_injectivity = relational_case_chunk_partition_gateway::injectivity(
        verified_chunk_partition,
        chunk_ordinal,
    )?;
    if &expected_injectivity != chunk_injectivity {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    let question_set = validate_scope(checked, plan, chunk_partition, chunk, chunk_injectivity)?;

    let accumulator = match prior {
        Some(prior) => {
            validate_accumulator_scope(prior, plan, chunk_partition, chunk)?;
            prior.clone()
        }
        None => initial_chunk_accumulator(plan, chunk_partition, chunk)?,
    };
    if accumulator.is_complete() {
        return Err(RelationalClassifiedSweepError::ClassifiedChunkAlreadyComplete);
    }

    let source =
        RelationalSourceEnumerator::new(checked.relation_id(), &checked.closed_query.source)?;
    let cases = RelationalCaseExecutor::new(checked.relation_id(), checked.closed_query)?;
    let questions = cases.checked_question_evaluation_plan(checked)?;
    if !questions
        .question_ids()
        .eq(question_set.question_ids().iter().copied())
    {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    let finite_factor_count = plan
        .stages()
        .iter()
        .filter(|stage| matches!(stage, RelationalBindingStage::Finite(_)))
        .count();

    let descriptor = chunk.descriptor();
    let slice_start = accumulator.next_coordinate;
    let member_limit = u128::from(max_members.get()).min(RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1);
    let slice_end_exclusive = slice_start
        .checked_add(member_limit)
        .unwrap_or(u128::MAX)
        .min(descriptor.interval_end_exclusive());
    if slice_start >= slice_end_exclusive {
        return Err(RelationalClassifiedSweepError::InvalidSliceArtifactShape(
            "caller-bounded classified slice is empty",
        ));
    }

    let transcript_root_before = accumulator.transcript_root;
    let mut materialized = Vec::with_capacity(member_limit as usize);
    for coordinate in slice_start..slice_end_exclusive {
        let finite_ordinals = decode_relational_case_chunk_finite_ordinals(
            chunk_partition,
            chunk.descriptor().ordinal(),
            coordinate,
            finite_factor_count,
        )?;
        let completed =
            source.completed_source_at_independent_finite_ordinals(&finite_ordinals, runtime)?;
        let transition = cases
            .statically_singleton_transition(completed.source_key(), completed.row(), runtime)?
            .ok_or(
                RelationalClassifiedSweepError::UnsupportedMaterializerShape(
                    "classified sweep requires a statically singleton TO relation",
                ),
            )?;
        let (case, _) = transition.into_parts();
        materialized.push(RelationalMaterializedClassificationSubject {
            coordinate,
            source: completed,
            case,
        });
    }

    let subjects = materialized
        .iter()
        .map(|materialized| {
            RelationalOrderedClassificationSubject::new(&materialized.source, &materialized.case)
        })
        .collect::<Vec<_>>();
    let mut checked_classifier =
        RelationalCheckedClassificationContext::new(&cases, &questions, runtime)?;
    let outcomes = backend.classify_ordered_batch(&subjects, &mut checked_classifier)?;
    if outcomes.len() != materialized.len() {
        return Err(
            RelationalClassifiedSweepError::OrderedClassifierOutcomeCountMismatch {
                expected: materialized.len(),
                actual: outcomes.len(),
            },
        );
    }

    let mut transcript_root_after = transcript_root_before;
    let mut raw_runs = Vec::<RelationalClassifiedChunkSliceRun>::new();
    for (materialized, outcome) in materialized.iter().zip(outcomes.iter()) {
        let coordinate = materialized.coordinate;
        outcome.validate(&question_set)?;

        transcript_root_after = advance_classified_transcript(
            transcript_root_after,
            coordinate,
            materialized.source.source_key().bytes(),
            materialized.case.case_id().bytes(),
            outcome,
        );

        match raw_runs.last_mut() {
            Some(previous)
                if can_merge_run_outcomes(
                    &previous.outcome,
                    outcome,
                    coordinate,
                    descriptor.interval_start(),
                ) =>
            {
                previous.interval_end_exclusive = coordinate
                    .checked_add(1)
                    .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
            }
            _ => raw_runs.push(RelationalClassifiedChunkSliceRun {
                interval_start: coordinate,
                interval_end_exclusive: coordinate
                    .checked_add(1)
                    .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?,
                outcome: outcome.clone(),
            }),
        }
    }

    let (rejected_count, admitted_count, admitted_selected_counts) =
        outcome_counts_from_slice_runs(&raw_runs, question_set.question_ids().len())?;
    let mut artifact = RelationalClassifiedChunkSliceArtifact {
        schema_version: RELATIONAL_CLASSIFIED_CHUNK_SLICE_VERSION,
        id: RelationalClassifiedChunkSliceId([0; 32]),
        plan_root: plan.root(),
        relation_id: checked.relation_id(),
        admission_id: checked.admission_id(),
        question_set,
        chunk_partition_id: chunk_partition.artifact().id(),
        chunk_id: descriptor.id(),
        chunk_ordinal: descriptor.ordinal(),
        chunk_cell_id: chunk.cell().id(),
        chunk_materializer_id: chunk.cell().materializer_id(),
        chunk_interval_start: descriptor.interval_start(),
        chunk_interval_end_exclusive: descriptor.interval_end_exclusive(),
        slice_interval_start: slice_start,
        slice_interval_end_exclusive: slice_end_exclusive,
        predecessor_slice_id: accumulator.last_slice_id,
        transcript_root_before,
        transcript_root_after,
        rejected_count,
        admitted_count,
        admitted_selected_counts: admitted_selected_counts.into_boxed_slice(),
        runs: raw_runs.into_boxed_slice(),
    };
    artifact.id = derive_slice_artifact_id(&artifact);
    artifact.validate_identity()?;
    let accumulator = reverify_relational_classified_chunk_slice_artifact(
        &artifact,
        plan,
        verified_chunk_partition,
        chunk_injectivity,
        prior,
    )?;
    Ok(RelationalClassifiedChunkSlice {
        artifact,
        accumulator,
    })
}

/// Replay-verify and merge one exact next slice without executing user code.
/// The returned accumulator is a pure deterministic fold of its predecessor and
/// this artifact.
pub(crate) fn reverify_relational_classified_chunk_slice_artifact(
    artifact: &RelationalClassifiedChunkSliceArtifact,
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    prior: Option<&RelationalClassifiedChunkAccumulator>,
) -> Result<RelationalClassifiedChunkAccumulator, RelationalClassifiedSweepError> {
    artifact.validate_identity()?;
    let question_set = freeze_question_set(plan.question_ids())?;
    let chunk_ordinal = usize::try_from(artifact.chunk_ordinal)
        .map_err(|_| RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let chunk_partition = verified_chunk_partition.partition();
    let chunk = chunk_partition
        .chunks()
        .get(chunk_ordinal)
        .ok_or(RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let expected_injectivity = relational_case_chunk_partition_gateway::injectivity(
        verified_chunk_partition,
        chunk_ordinal,
    )?;
    if &expected_injectivity != chunk_injectivity {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    validate_chunk_injectivity(chunk, chunk_injectivity)?;
    if !plan.validate_root()
        || artifact.plan_root != plan.root()
        || artifact.relation_id != plan.relation_id()
        || artifact.admission_id != plan.admission_id()
        || artifact.question_set != question_set
        || artifact.chunk_partition_id != chunk_partition.artifact().id()
        || artifact.chunk_id != chunk.descriptor().id()
        || artifact.chunk_ordinal != chunk.descriptor().ordinal()
        || artifact.chunk_cell_id != chunk.cell().id()
        || artifact.chunk_materializer_id != chunk.cell().materializer_id()
        || artifact.chunk_interval_start != chunk.descriptor().interval_start()
        || artifact.chunk_interval_end_exclusive != chunk.descriptor().interval_end_exclusive()
    {
        return Err(RelationalClassifiedSweepError::SliceArtifactScopeMismatch);
    }

    let mut accumulator = match prior {
        Some(prior) => {
            validate_accumulator_scope(prior, plan, chunk_partition, chunk)?;
            prior.clone()
        }
        None => initial_chunk_accumulator(plan, chunk_partition, chunk)?,
    };
    if accumulator.is_complete() {
        return Err(RelationalClassifiedSweepError::ClassifiedChunkAlreadyComplete);
    }
    if artifact.slice_interval_start != accumulator.next_coordinate
        || artifact.predecessor_slice_id != accumulator.last_slice_id
        || artifact.transcript_root_before != accumulator.transcript_root
    {
        return Err(RelationalClassifiedSweepError::SliceArtifactPredecessorMismatch);
    }

    for run in artifact.runs.iter().cloned() {
        match accumulator.runs.last_mut() {
            Some(previous)
                if previous.interval_end_exclusive == run.interval_start
                    && can_merge_run_outcomes(
                        &previous.outcome,
                        &run.outcome,
                        run.interval_start,
                        accumulator.interval_start,
                    ) =>
            {
                previous.interval_end_exclusive = run.interval_end_exclusive;
            }
            _ => accumulator.runs.push(run),
        }
    }
    accumulator.rejected_count = accumulator
        .rejected_count
        .checked_add(artifact.rejected_count)
        .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
    accumulator.admitted_count = accumulator
        .admitted_count
        .checked_add(artifact.admitted_count)
        .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
    if accumulator.admitted_selected_counts.len() != artifact.admitted_selected_counts.len() {
        return Err(RelationalClassifiedSweepError::QuestionDecisionVectorMismatch);
    }
    for (total, delta) in accumulator
        .admitted_selected_counts
        .iter_mut()
        .zip(artifact.admitted_selected_counts.iter().copied())
    {
        *total = total
            .checked_add(delta)
            .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
    }
    accumulator.next_coordinate = artifact.slice_interval_end_exclusive;
    accumulator.transcript_root = artifact.transcript_root_after;
    accumulator.last_slice_id = Some(artifact.id);
    validate_accumulator_scope(&accumulator, plan, chunk_partition, chunk)?;
    Ok(accumulator)
}

/// Deterministically lower one complete folded transcript into the existing
/// canonical whole-chunk artifact and typed support consequences.
pub(crate) fn finalize_relational_classified_case_chunk(
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_ordinal: usize,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    accumulator: &RelationalClassifiedChunkAccumulator,
) -> Result<RelationalClassifiedChunk, RelationalClassifiedSweepError> {
    let chunk_partition = verified_chunk_partition.partition();
    let chunk = chunk_partition
        .chunks()
        .get(chunk_ordinal)
        .ok_or(RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let expected_injectivity = relational_case_chunk_partition_gateway::injectivity(
        verified_chunk_partition,
        chunk_ordinal,
    )?;
    if &expected_injectivity != chunk_injectivity {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    validate_chunk_injectivity(chunk, chunk_injectivity)?;
    validate_accumulator_scope(accumulator, plan, chunk_partition, chunk)?;
    if !accumulator.is_complete() {
        return Err(RelationalClassifiedSweepError::ClassifiedChunkIncomplete {
            next_coordinate: accumulator.next_coordinate,
            end_exclusive: accumulator.interval_end_exclusive,
        });
    }
    let raw_runs = accumulator
        .runs
        .iter()
        .map(|run| {
            (
                run.interval_start,
                run.interval_end_exclusive,
                run.outcome.clone(),
            )
        })
        .collect::<Vec<_>>();
    issue_verified_chunk(
        plan,
        chunk_partition,
        chunk,
        chunk_injectivity,
        accumulator.transcript_root.bytes(),
        raw_runs,
    )
    .map(|verified| RelationalClassifiedChunk { verified })
}

fn initial_chunk_accumulator(
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
) -> Result<RelationalClassifiedChunkAccumulator, RelationalClassifiedSweepError> {
    let descriptor = chunk.descriptor();
    let question_set = freeze_question_set(plan.question_ids())?;
    Ok(RelationalClassifiedChunkAccumulator {
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        admission_id: plan.admission_id(),
        question_set: question_set.clone(),
        chunk_partition_id: chunk_partition.artifact().id(),
        chunk_id: descriptor.id(),
        chunk_ordinal: descriptor.ordinal(),
        chunk_cell_id: chunk.cell().id(),
        chunk_materializer_id: chunk.cell().materializer_id(),
        interval_start: descriptor.interval_start(),
        interval_end_exclusive: descriptor.interval_end_exclusive(),
        next_coordinate: descriptor.interval_start(),
        transcript_root: classified_transcript_genesis(plan, chunk_partition, chunk)?,
        last_slice_id: None,
        rejected_count: 0,
        admitted_count: 0,
        admitted_selected_counts: vec![0; question_set.question_ids().len()],
        runs: Vec::new(),
    })
}

fn validate_accumulator_scope(
    accumulator: &RelationalClassifiedChunkAccumulator,
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
) -> Result<(), RelationalClassifiedSweepError> {
    let descriptor = chunk.descriptor();
    let question_set = freeze_question_set(plan.question_ids())?;
    if !plan.validate_root()
        || accumulator.plan_root != plan.root()
        || accumulator.relation_id != plan.relation_id()
        || accumulator.admission_id != plan.admission_id()
        || accumulator.question_set != question_set
        || accumulator.admitted_selected_counts.len() != question_set.question_ids().len()
        || accumulator.chunk_partition_id != chunk_partition.artifact().id()
        || accumulator.chunk_id != descriptor.id()
        || accumulator.chunk_ordinal != descriptor.ordinal()
        || accumulator.chunk_cell_id != chunk.cell().id()
        || accumulator.chunk_materializer_id != chunk.cell().materializer_id()
        || accumulator.interval_start != descriptor.interval_start()
        || accumulator.interval_end_exclusive != descriptor.interval_end_exclusive()
        || accumulator.next_coordinate < accumulator.interval_start
        || accumulator.next_coordinate > accumulator.interval_end_exclusive
        || accumulator.runs.len()
            > usize::try_from(RELATIONAL_CASE_CHUNK_MAX_PAGE_COORDINATES)
                .expect("the bounded page run limit fits usize")
    {
        return Err(RelationalClassifiedSweepError::ClassifiedAccumulatorScopeMismatch);
    }

    let expected_genesis = classified_transcript_genesis(plan, chunk_partition, chunk)?;
    if accumulator.next_coordinate == accumulator.interval_start {
        if accumulator.last_slice_id.is_some()
            || accumulator.transcript_root != expected_genesis
            || accumulator.rejected_count != 0
            || accumulator.admitted_count != 0
            || accumulator
                .admitted_selected_counts
                .iter()
                .any(|count| *count != 0)
            || !accumulator.runs.is_empty()
        {
            return Err(RelationalClassifiedSweepError::InvalidAccumulatorShape(
                "empty classified accumulator has non-genesis state",
            ));
        }
        return Ok(());
    }
    if accumulator.last_slice_id.is_none() || accumulator.runs.is_empty() {
        return Err(RelationalClassifiedSweepError::InvalidAccumulatorShape(
            "positive classified accumulator has no slice or runs",
        ));
    }

    let mut next_start = accumulator.interval_start;
    for (index, run) in accumulator.runs.iter().enumerate() {
        run.outcome.validate(&question_set)?;
        if run.interval_start != next_start
            || run.interval_end_exclusive > accumulator.next_coordinate
            || !selected_run_within_quantum(
                &run.outcome,
                run.interval_start,
                run.interval_end_exclusive,
                accumulator.interval_start,
            )
            || index
                .checked_sub(1)
                .and_then(|previous| accumulator.runs.get(previous))
                .is_some_and(|previous| {
                    can_merge_run_outcomes(
                        &previous.outcome,
                        &run.outcome,
                        run.interval_start,
                        accumulator.interval_start,
                    )
                })
        {
            return Err(RelationalClassifiedSweepError::InvalidAccumulatorShape(
                "classified accumulator runs are not a maximal contiguous prefix",
            ));
        }
        next_start = run.interval_end_exclusive;
    }
    let (rejected, admitted, admitted_selected) =
        outcome_counts_from_slice_runs(&accumulator.runs, question_set.question_ids().len())?;
    let evaluated = accumulator
        .next_coordinate
        .checked_sub(accumulator.interval_start)
        .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
    if next_start != accumulator.next_coordinate
        || rejected != accumulator.rejected_count
        || admitted != accumulator.admitted_count
        || admitted_selected != accumulator.admitted_selected_counts
        || rejected.checked_add(admitted) != Some(evaluated)
    {
        return Err(RelationalClassifiedSweepError::InvalidAccumulatorShape(
            "classified accumulator counts do not conserve its prefix",
        ));
    }
    Ok(())
}

fn classified_transcript_genesis(
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
) -> Result<RelationalClassifiedChunkTranscriptRoot, RelationalClassifiedSweepError> {
    let descriptor = chunk.descriptor();
    let question_set = freeze_question_set(plan.question_ids())?;
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_CHUNK_TRANSCRIPT_GENESIS_V3);
    hasher.u32(RELATIONAL_CLASSIFIED_CHUNK_VERSION);
    hasher.digest(plan.root().bytes());
    hasher.digest(plan.relation_id().bytes());
    hasher.digest(plan.admission_id().bytes());
    hasher.digest(question_set.root().bytes());
    hasher.digest(chunk_partition.artifact().id().bytes());
    hasher.digest(descriptor.id().bytes());
    hasher.u128(descriptor.ordinal());
    hasher.digest(chunk.cell().id().bytes());
    hasher.digest(chunk.cell().materializer_id().bytes());
    hasher.u128(descriptor.interval_start());
    hasher.u128(descriptor.interval_end_exclusive());
    Ok(RelationalClassifiedChunkTranscriptRoot(hasher.finish()))
}

fn advance_classified_transcript(
    previous: RelationalClassifiedChunkTranscriptRoot,
    coordinate: u128,
    source_key: [u8; 32],
    case_id: [u8; 32],
    outcome: &RelationalClassifiedCaseOutcome,
) -> RelationalClassifiedChunkTranscriptRoot {
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_CHUNK_TRANSCRIPT_MEMBER_V3);
    hasher.u32(RELATIONAL_CLASSIFIED_CHUNK_VERSION);
    hasher.digest(previous.bytes());
    hasher.u128(coordinate);
    hasher.digest(source_key);
    hasher.digest(case_id);
    hasher.u8(outcome.canonical_tag());
    if let Some(mask) = outcome.decision_mask() {
        hasher.bytes(mask.bytes());
    }
    RelationalClassifiedChunkTranscriptRoot(hasher.finish())
}

/// Reconstruct cells, partition, and evidence bindings from a retained
/// checked-executor artifact. Semantic outcomes are journal evidence just like
/// concrete classification events; all derivable structure is rechecked.
pub(crate) fn reverify_relational_classified_chunk_artifact(
    artifact: &RelationalClassifiedChunkArtifact,
    plan: &RelationalSupportPlan,
    verified_chunk_partition: &VerifiedRelationalCaseChunkPartition,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<VerifiedRelationalClassifiedChunk, RelationalClassifiedSweepError> {
    artifact.validate_identity()?;
    let chunk_partition = verified_chunk_partition.partition();
    if artifact.chunk_partition_id() != verified_chunk_partition.artifact().id() {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    let chunk_ordinal = usize::try_from(artifact.chunk_ordinal)
        .map_err(|_| RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let chunk = chunk_partition
        .chunks()
        .get(chunk_ordinal)
        .ok_or(RelationalClassifiedSweepError::ChunkOrdinalOutOfBounds)?;
    let expected_injectivity = relational_case_chunk_partition_gateway::injectivity(
        verified_chunk_partition,
        chunk_ordinal,
    )?;
    if &expected_injectivity != chunk_injectivity {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    validate_retained_scope(artifact, plan, chunk_partition, chunk, chunk_injectivity)?;
    finish_verified_chunk(artifact.clone(), chunk_partition, chunk, chunk_injectivity)
}

fn issue_verified_chunk(
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    evaluated_cases_root: [u8; 32],
    raw_runs: Vec<(u128, u128, RelationalClassifiedCaseOutcome)>,
) -> Result<VerifiedRelationalClassifiedChunk, RelationalClassifiedSweepError> {
    let question_set = freeze_question_set(plan.question_ids())?;
    let run_cells = build_run_cells(chunk_partition, chunk, chunk_injectivity, &raw_runs)?;
    let mut descriptors = Vec::with_capacity(run_cells.len());
    for (index, ((start, end_exclusive, outcome), cell)) in
        raw_runs.iter().zip(&run_cells).enumerate()
    {
        let ordinal = u16::try_from(index).map_err(|_| {
            RelationalClassifiedSweepError::InvalidArtifactShape(
                "classified run ordinal exceeds u16",
            )
        })?;
        let id = derive_run_id(
            plan.root(),
            chunk_partition.artifact().id(),
            chunk.descriptor().id(),
            chunk.cell().id(),
            chunk.cell().materializer_id(),
            ordinal,
            cell.id(),
            *start,
            *end_exclusive,
            outcome,
        );
        descriptors.push(RelationalClassifiedRunDescriptor {
            id,
            ordinal,
            cell_id: cell.id(),
            interval_start: *start,
            interval_end_exclusive: *end_exclusive,
            outcome: outcome.clone(),
        });
    }
    let partition = build_run_partition(chunk_partition, chunk, chunk_injectivity, &run_cells)?;
    let (rejected_count, admitted_count, admitted_selected_counts) =
        outcome_counts_from_descriptors(&descriptors, question_set.question_ids().len())?;
    let descriptor = chunk.descriptor();
    let mut artifact = RelationalClassifiedChunkArtifact {
        schema_version: RELATIONAL_CLASSIFIED_CHUNK_VERSION,
        id: RelationalClassifiedChunkArtifactId([0; 32]),
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        admission_id: plan.admission_id(),
        question_set,
        chunk_partition_id: chunk_partition.artifact().id(),
        chunk_id: descriptor.id(),
        chunk_ordinal: descriptor.ordinal(),
        chunk_cell_id: chunk.cell().id(),
        chunk_materializer_id: chunk.cell().materializer_id(),
        interval_start: descriptor.interval_start(),
        interval_end_exclusive: descriptor.interval_end_exclusive(),
        evaluated_case_count: descriptor.cardinality(),
        evaluated_cases_root,
        rejected_count,
        admitted_count,
        admitted_selected_counts: admitted_selected_counts.into_boxed_slice(),
        runs: descriptors.into_boxed_slice(),
        partition_id: partition.as_ref().map(SupportPartitionCertificate::id),
    };
    artifact.id = derive_artifact_id(&artifact);
    artifact.validate_identity()?;
    finish_verified_chunk_with_parts(artifact, run_cells, partition)
}

fn finish_verified_chunk(
    artifact: RelationalClassifiedChunkArtifact,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<VerifiedRelationalClassifiedChunk, RelationalClassifiedSweepError> {
    let raw_runs = artifact
        .runs
        .iter()
        .map(|run| {
            (
                run.interval_start,
                run.interval_end_exclusive,
                run.outcome.clone(),
            )
        })
        .collect::<Vec<_>>();
    let run_cells = build_run_cells(chunk_partition, chunk, chunk_injectivity, &raw_runs)?;
    for (descriptor, cell) in artifact.runs.iter().zip(&run_cells) {
        if descriptor.cell_id != cell.id() {
            return Err(RelationalClassifiedSweepError::RunCellMismatch {
                ordinal: descriptor.ordinal,
            });
        }
    }
    let partition = build_run_partition(chunk_partition, chunk, chunk_injectivity, &run_cells)?;
    if artifact.partition_id != partition.as_ref().map(SupportPartitionCertificate::id) {
        return Err(RelationalClassifiedSweepError::RunPartitionMismatch);
    }
    finish_verified_chunk_with_parts(artifact, run_cells, partition)
}

fn finish_verified_chunk_with_parts(
    artifact: RelationalClassifiedChunkArtifact,
    run_cells: Vec<SupportCell>,
    partition: Option<SupportPartitionCertificate>,
) -> Result<VerifiedRelationalClassifiedChunk, RelationalClassifiedSweepError> {
    let mut runs = Vec::with_capacity(run_cells.len());
    let mut bindings = Vec::with_capacity(run_cells.len());
    for (descriptor, cell) in artifact.runs.iter().cloned().zip(run_cells) {
        let injectivity_obligation = (cell.id() != artifact.chunk_cell_id)
            .then(|| {
                SupportCellObligation::new(
                    &cell,
                    InjectiveMappingClaim::new(cell.materializer_id()),
                )
            })
            .transpose()?;
        let cardinality_obligation = SupportCellObligation::new(&cell, ExactCardinalityClaim)?;
        let admission_obligation = SupportCellObligation::new(
            &cell,
            AdmissionClassificationClaim::new(artifact.admission_id),
        )?;
        let cardinality = descriptor.cardinality();
        let admission = descriptor.outcome.admission();
        let mut selections = Vec::new();
        if admission == AdmissionDecision::Admitted {
            selections
                .try_reserve_exact(artifact.question_ids().len())
                .map_err(|_| {
                    RelationalClassifiedSweepError::InvalidArtifactShape(
                        "classified selection binding allocation failed",
                    )
                })?;
            for (question_index, question_id) in artifact.question_ids().iter().copied().enumerate()
            {
                let conclusion = descriptor
                    .outcome
                    .selection(question_index)
                    .ok_or(RelationalClassifiedSweepError::QuestionDecisionVectorMismatch)?;
                let obligation = SupportCellObligation::new(
                    &cell,
                    SelectionClassificationClaim::new(question_id),
                )?;
                selections.push(classified_evidence_binding(
                    artifact.id,
                    descriptor.id,
                    0x04,
                    obligation.id(),
                    obligation.claim().conclusion_digest(&conclusion),
                ));
            }
        }
        bindings.push(RelationalClassifiedRunEvidenceBindings {
            injectivity: injectivity_obligation.map(|obligation| {
                let conclusion = super::support_cell::CertifiedInjective;
                classified_evidence_binding(
                    artifact.id,
                    descriptor.id,
                    0x01,
                    obligation.id(),
                    obligation.claim().conclusion_digest(&conclusion),
                )
            }),
            cardinality: classified_evidence_binding(
                artifact.id,
                descriptor.id,
                0x02,
                cardinality_obligation.id(),
                cardinality_obligation
                    .claim()
                    .conclusion_digest(&cardinality),
            ),
            admission: classified_evidence_binding(
                artifact.id,
                descriptor.id,
                0x03,
                admission_obligation.id(),
                admission_obligation.claim().conclusion_digest(&admission),
            ),
            selections: selections.into_boxed_slice(),
        });
        runs.push(RelationalClassifiedRun { descriptor, cell });
    }
    Ok(VerifiedRelationalClassifiedChunk {
        artifact,
        runs: runs.into_boxed_slice(),
        partition,
        bindings: bindings.into_boxed_slice(),
    })
}

fn validate_scope(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<FrozenClassificationQuestionSet, RelationalClassifiedSweepError> {
    checked
        .closed_query
        .validate()
        .map_err(RelationalClassifiedSweepError::InvalidQuery)?;
    let checked_question_set = freeze_question_set(checked.question_ids())?;
    let plan_question_set = freeze_question_set(plan.question_ids())?;
    if !plan.validate_root()
        || plan.relation_id() != checked.relation_id()
        || plan.admission_id() != checked.admission_id()
        || plan_question_set != checked_question_set
        || chunk_partition.artifact().plan_root() != plan.root()
    {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    validate_chunk_injectivity(chunk, chunk_injectivity)?;
    Ok(checked_question_set)
}

fn validate_retained_scope(
    artifact: &RelationalClassifiedChunkArtifact,
    plan: &RelationalSupportPlan,
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<(), RelationalClassifiedSweepError> {
    let descriptor = chunk.descriptor();
    let question_set = freeze_question_set(plan.question_ids())?;
    if !plan.validate_root()
        || artifact.plan_root != plan.root()
        || artifact.relation_id != plan.relation_id()
        || artifact.admission_id != plan.admission_id()
        || artifact.question_set != question_set
        || artifact.chunk_partition_id != chunk_partition.artifact().id()
        || artifact.chunk_id != descriptor.id()
        || artifact.chunk_ordinal != descriptor.ordinal()
        || artifact.chunk_cell_id != chunk.cell().id()
        || artifact.chunk_materializer_id != chunk.cell().materializer_id()
        || artifact.interval_start != descriptor.interval_start()
        || artifact.interval_end_exclusive != descriptor.interval_end_exclusive()
    {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    validate_chunk_injectivity(chunk, chunk_injectivity)
}

fn freeze_question_set(
    question_ids: &[QuestionId],
) -> Result<FrozenClassificationQuestionSet, RelationalClassifiedSweepError> {
    FrozenClassificationQuestionSet::freeze(question_ids.iter().copied())
        .map_err(|_| RelationalClassifiedSweepError::InvalidQuestionSet)
}

fn validate_chunk_injectivity(
    chunk: &RelationalCaseChunk,
    evidence: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<(), RelationalClassifiedSweepError> {
    chunk.cell().validate_evidence(evidence)?;
    if evidence.obligation().claim().materializer_id() != chunk.cell().materializer_id() {
        return Err(RelationalClassifiedSweepError::ScopeMismatch);
    }
    Ok(())
}

fn build_run_cells(
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    raw_runs: &[(u128, u128, RelationalClassifiedCaseOutcome)],
) -> Result<Vec<SupportCell>, RelationalClassifiedSweepError> {
    if raw_runs.len() == 1
        && raw_runs[0].0 == chunk.descriptor().interval_start()
        && raw_runs[0].1 == chunk.descriptor().interval_end_exclusive()
    {
        return Ok(vec![chunk.cell().clone()]);
    }
    validate_chunk_injectivity(chunk, chunk_injectivity)?;
    raw_runs
        .iter()
        .map(|(start, end_exclusive, _)| {
            derive_relational_case_chunk_subinterval_cell(
                chunk_partition,
                chunk.descriptor().ordinal(),
                *start,
                *end_exclusive,
            )
            .map_err(Into::into)
        })
        .collect()
}

fn build_run_partition(
    chunk_partition: &RelationalCaseChunkPartition,
    chunk: &RelationalCaseChunk,
    chunk_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    run_cells: &[SupportCell],
) -> Result<Option<SupportPartitionCertificate>, RelationalClassifiedSweepError> {
    if run_cells.len() == 1 {
        if run_cells[0] != *chunk.cell() {
            return Err(RelationalClassifiedSweepError::RunPartitionMismatch);
        }
        return Ok(None);
    }
    let certificate = match chunk_partition.artifact().shape() {
        RelationalCaseChunkShape::BareOrdinalInterval => {
            SupportPartitionCertificate::mapped_injective_ordinal_cover(
                chunk.cell(),
                run_cells.to_vec(),
                chunk_injectivity,
            )?
        }
        RelationalCaseChunkShape::ProductFactor => {
            let factor_index = chunk_partition
                .artifact()
                .factor_index()
                .and_then(|index| usize::try_from(index).ok())
                .ok_or(RelationalClassifiedSweepError::RunPartitionMismatch)?;
            SupportPartitionCertificate::mapped_injective_product_factor_cover(
                chunk.cell(),
                run_cells.to_vec(),
                factor_index,
                chunk_injectivity,
            )?
        }
        RelationalCaseChunkShape::ProductRankInterval => {
            SupportPartitionCertificate::mapped_injective_product_rank_interval_cover(
                chunk.cell(),
                run_cells.to_vec(),
                chunk_injectivity,
            )?
        }
    };
    Ok(Some(certificate))
}

fn outcome_counts_from_descriptors(
    runs: &[RelationalClassifiedRunDescriptor],
    question_count: usize,
) -> Result<(u128, u128, Vec<u128>), RelationalClassifiedSweepError> {
    let mut rejected = 0u128;
    let mut admitted = 0u128;
    let mut admitted_selected = vec![0u128; question_count];
    for run in runs {
        accumulate_outcome_counts(
            &run.outcome,
            run.cardinality(),
            &mut rejected,
            &mut admitted,
            &mut admitted_selected,
        )?;
    }
    Ok((rejected, admitted, admitted_selected))
}

// Selected runs must remain bounded inputs to the existing concrete
// materializer. Cut them at page-relative 256-coordinate boundaries, not at
// operational slice boundaries, so changing a time quantum cannot change IDs.
// Harmless/rejected runs may coalesce across the whole page.
fn can_merge_run_outcomes(
    left: &RelationalClassifiedCaseOutcome,
    right: &RelationalClassifiedCaseOutcome,
    boundary: u128,
    page_start: u128,
) -> bool {
    left == right
        && (!left.any_selected()
            || boundary
                .checked_sub(page_start)
                .is_some_and(|offset| offset % RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 != 0))
}

fn selected_run_within_quantum(
    outcome: &RelationalClassifiedCaseOutcome,
    start: u128,
    end: u128,
    page_start: u128,
) -> bool {
    if !outcome.any_selected() {
        return true;
    }
    match (
        start.checked_sub(page_start),
        end.checked_sub(1)
            .and_then(|last| last.checked_sub(page_start)),
    ) {
        (Some(first), Some(last)) => {
            first <= last
                && first / RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1
                    == last / RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1
        }
        _ => false,
    }
}

fn outcome_counts_from_slice_runs(
    runs: &[RelationalClassifiedChunkSliceRun],
    question_count: usize,
) -> Result<(u128, u128, Vec<u128>), RelationalClassifiedSweepError> {
    let mut rejected = 0u128;
    let mut admitted = 0u128;
    let mut admitted_selected = vec![0u128; question_count];
    for run in runs {
        accumulate_outcome_counts(
            &run.outcome,
            run.cardinality(),
            &mut rejected,
            &mut admitted,
            &mut admitted_selected,
        )?;
    }
    Ok((rejected, admitted, admitted_selected))
}

fn accumulate_outcome_counts(
    outcome: &RelationalClassifiedCaseOutcome,
    cardinality: u128,
    rejected: &mut u128,
    admitted: &mut u128,
    admitted_selected: &mut [u128],
) -> Result<(), RelationalClassifiedSweepError> {
    match outcome {
        RelationalClassifiedCaseOutcome::Rejected => {
            *rejected = rejected
                .checked_add(cardinality)
                .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
        }
        RelationalClassifiedCaseOutcome::Admitted(mask) => {
            mask.validate(admitted_selected.len())?;
            *admitted = admitted
                .checked_add(cardinality)
                .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
            for (question_index, selected) in admitted_selected.iter_mut().enumerate() {
                if mask.selection(question_index) == Some(SelectionDecision::Selected) {
                    *selected = selected
                        .checked_add(cardinality)
                        .ok_or(RelationalClassifiedSweepError::CardinalityOverflow)?;
                }
            }
        }
    }
    Ok(())
}

fn derive_slice_artifact_id(
    artifact: &RelationalClassifiedChunkSliceArtifact,
) -> RelationalClassifiedChunkSliceId {
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_CHUNK_SLICE_ID_V2);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.question_set.root().bytes());
    hasher.digest(artifact.chunk_partition_id.bytes());
    hasher.digest(artifact.chunk_id.bytes());
    hasher.u128(artifact.chunk_ordinal);
    hasher.digest(artifact.chunk_cell_id.bytes());
    hasher.digest(artifact.chunk_materializer_id.bytes());
    hasher.u128(artifact.chunk_interval_start);
    hasher.u128(artifact.chunk_interval_end_exclusive);
    hasher.u128(artifact.slice_interval_start);
    hasher.u128(artifact.slice_interval_end_exclusive);
    match artifact.predecessor_slice_id {
        None => hasher.u8(0x00),
        Some(predecessor) => {
            hasher.u8(0x01);
            hasher.digest(predecessor.bytes());
        }
    }
    hasher.digest(artifact.transcript_root_before.bytes());
    hasher.digest(artifact.transcript_root_after.bytes());
    hasher.u128(artifact.rejected_count);
    hasher.u128(artifact.admitted_count);
    hasher.u128(artifact.admitted_selected_counts.len() as u128);
    for count in artifact.admitted_selected_counts.iter().copied() {
        hasher.u128(count);
    }
    hasher.u128(artifact.runs.len() as u128);
    for run in artifact.runs.iter() {
        hasher.u128(run.interval_start);
        hasher.u128(run.interval_end_exclusive);
        hash_outcome(&mut hasher, &run.outcome);
    }
    RelationalClassifiedChunkSliceId(hasher.finish())
}

#[allow(clippy::too_many_arguments)]
fn derive_run_id(
    plan_root: RelationalSupportPlanRoot,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_cell_id: SupportCellId,
    materializer_id: SupportMaterializerId,
    ordinal: u16,
    cell_id: SupportCellId,
    interval_start: u128,
    interval_end_exclusive: u128,
    outcome: &RelationalClassifiedCaseOutcome,
) -> RelationalClassifiedRunId {
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_RUN_ID_V2);
    hasher.u32(RELATIONAL_CLASSIFIED_CHUNK_VERSION);
    hasher.digest(plan_root.bytes());
    hasher.digest(chunk_partition_id.bytes());
    hasher.digest(chunk_id.bytes());
    hasher.digest(chunk_cell_id.bytes());
    hasher.digest(materializer_id.bytes());
    hasher.u16(ordinal);
    hasher.digest(cell_id.bytes());
    hasher.u128(interval_start);
    hasher.u128(interval_end_exclusive);
    hash_outcome(&mut hasher, outcome);
    RelationalClassifiedRunId(hasher.finish())
}

fn derive_artifact_id(
    artifact: &RelationalClassifiedChunkArtifact,
) -> RelationalClassifiedChunkArtifactId {
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_CHUNK_ARTIFACT_ID_V2);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.question_set.root().bytes());
    hasher.digest(artifact.chunk_partition_id.bytes());
    hasher.digest(artifact.chunk_id.bytes());
    hasher.u128(artifact.chunk_ordinal);
    hasher.digest(artifact.chunk_cell_id.bytes());
    hasher.digest(artifact.chunk_materializer_id.bytes());
    hasher.u128(artifact.interval_start);
    hasher.u128(artifact.interval_end_exclusive);
    hasher.u128(artifact.evaluated_case_count);
    hasher.digest(artifact.evaluated_cases_root);
    hasher.u128(artifact.rejected_count);
    hasher.u128(artifact.admitted_count);
    hasher.u128(artifact.admitted_selected_counts.len() as u128);
    for count in artifact.admitted_selected_counts.iter().copied() {
        hasher.u128(count);
    }
    hasher.u128(artifact.runs.len() as u128);
    for run in artifact.runs.iter() {
        hasher.digest(run.id.bytes());
        hasher.u16(run.ordinal);
        hasher.digest(run.cell_id.bytes());
        hasher.u128(run.interval_start);
        hasher.u128(run.interval_end_exclusive);
        hash_outcome(&mut hasher, &run.outcome);
    }
    match artifact.partition_id {
        None => hasher.u8(0x00),
        Some(partition_id) => {
            hasher.u8(0x01);
            hasher.digest(partition_id.bytes());
        }
    }
    RelationalClassifiedChunkArtifactId(hasher.finish())
}

fn classified_evidence_binding(
    artifact_id: RelationalClassifiedChunkArtifactId,
    run_id: RelationalClassifiedRunId,
    role: u8,
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
) -> RelationalClassifiedEvidenceBinding {
    let mut hasher = ClassifiedHasher::new(CLASSIFIED_CHUNK_EVIDENCE_V2);
    hasher.digest(artifact_id.bytes());
    hasher.digest(run_id.bytes());
    hasher.u8(role);
    hasher.digest(obligation_id.bytes());
    hasher.digest(conclusion_digest);
    RelationalClassifiedEvidenceBinding {
        obligation_id,
        conclusion_digest,
        proof_digest: hasher.finish(),
    }
}

fn hash_outcome(hasher: &mut ClassifiedHasher, outcome: &RelationalClassifiedCaseOutcome) {
    hasher.u8(outcome.canonical_tag());
    if let Some(mask) = outcome.decision_mask() {
        hasher.bytes(mask.bytes());
    }
}

struct ClassifiedHasher(Sha256);

impl ClassifiedHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u16(&mut self, value: u16) {
        self.0.update(value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u128(value.len() as u128);
        self.0.update(value);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassifiedSweepError {
    UnsupportedArtifactVersion(u32),
    UnsupportedSliceArtifactVersion(u32),
    InvalidArtifactShape(&'static str),
    InvalidSliceArtifactShape(&'static str),
    InvalidAccumulatorShape(&'static str),
    ArtifactIdentityMismatch,
    SliceArtifactIdentityMismatch,
    RunIdentityMismatch {
        ordinal: u16,
    },
    RunCellMismatch {
        ordinal: u16,
    },
    RunPartitionMismatch,
    ChunkOrdinalOutOfBounds,
    ClassifiedChunkAlreadyComplete,
    ClassifiedChunkIncomplete {
        next_coordinate: u128,
        end_exclusive: u128,
    },
    ScopeMismatch,
    SliceArtifactScopeMismatch,
    SliceArtifactPredecessorMismatch,
    ClassifiedAccumulatorScopeMismatch,
    InvalidQuestionSet,
    InvalidDecisionMask,
    QuestionDecisionVectorMismatch,
    UnsupportedMaterializerShape(&'static str),
    ProductFactorOutOfBounds {
        factor_index: usize,
        factor_count: usize,
    },
    InvalidCheckedClassification(super::relation::RelationalCaseId),
    OrderedClassifierOutcomeCountMismatch {
        expected: usize,
        actual: usize,
    },
    CardinalityOverflow,
    InvalidQuery(String),
    ChunkPartition(RelationalCaseChunkPartitionError),
    Source(RelationalSourceExecutorError),
    Case(RelationalCaseExecutorError),
    SupportCell(SupportCellError),
}

impl From<RelationalCaseChunkPartitionError> for RelationalClassifiedSweepError {
    fn from(error: RelationalCaseChunkPartitionError) -> Self {
        Self::ChunkPartition(error)
    }
}

impl From<RelationalSourceExecutorError> for RelationalClassifiedSweepError {
    fn from(error: RelationalSourceExecutorError) -> Self {
        Self::Source(error)
    }
}

impl From<RelationalCaseExecutorError> for RelationalClassifiedSweepError {
    fn from(error: RelationalCaseExecutorError) -> Self {
        Self::Case(error)
    }
}

impl From<SupportCellError> for RelationalClassifiedSweepError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl fmt::Display for RelationalClassifiedSweepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion(version) => {
                write!(formatter, "unsupported classified-chunk version {version}")
            }
            Self::UnsupportedSliceArtifactVersion(version) => write!(
                formatter,
                "unsupported classified-chunk slice version {version}"
            ),
            Self::InvalidArtifactShape(message) => {
                write!(formatter, "invalid classified-chunk artifact: {message}")
            }
            Self::InvalidSliceArtifactShape(message) => {
                write!(
                    formatter,
                    "invalid classified-chunk slice artifact: {message}"
                )
            }
            Self::InvalidAccumulatorShape(message) => {
                write!(formatter, "invalid classified-chunk accumulator: {message}")
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("classified-chunk artifact identity does not match its content")
            }
            Self::SliceArtifactIdentityMismatch => {
                formatter.write_str("classified-chunk slice identity does not match its content")
            }
            Self::RunIdentityMismatch { ordinal } => write!(
                formatter,
                "classified run identity is not canonical at ordinal {ordinal}"
            ),
            Self::RunCellMismatch { ordinal } => write!(
                formatter,
                "classified run cell is not canonical at ordinal {ordinal}"
            ),
            Self::RunPartitionMismatch => formatter
                .write_str("classified runs do not form the canonical injective chunk cover"),
            Self::ChunkOrdinalOutOfBounds => {
                formatter.write_str("classified artifact names an absent case chunk")
            }
            Self::ClassifiedChunkAlreadyComplete => {
                formatter.write_str("classified-chunk accumulator is already complete")
            }
            Self::ClassifiedChunkIncomplete {
                next_coordinate,
                end_exclusive,
            } => write!(
                formatter,
                "classified-chunk accumulator stops at coordinate {next_coordinate}, before canonical end {end_exclusive}"
            ),
            Self::ScopeMismatch => formatter.write_str(
                "classified artifact, checked query, support plan, chunk, or proof scope disagrees",
            ),
            Self::SliceArtifactScopeMismatch => {
                formatter.write_str("classified-chunk slice and canonical chunk scopes disagree")
            }
            Self::SliceArtifactPredecessorMismatch => formatter.write_str(
                "classified-chunk slice is not the next authenticated contiguous prefix",
            ),
            Self::ClassifiedAccumulatorScopeMismatch => formatter
                .write_str("classified-chunk accumulator and canonical chunk scopes disagree"),
            Self::InvalidQuestionSet => formatter
                .write_str("classified-sweep question set is not canonical or does not match"),
            Self::InvalidDecisionMask => formatter.write_str(
                "classified-sweep decision mask is not the canonical packed question vector",
            ),
            Self::QuestionDecisionVectorMismatch => formatter.write_str(
                "classified-sweep decisions do not cover the canonical question set exactly",
            ),
            Self::UnsupportedMaterializerShape(message) => {
                write!(
                    formatter,
                    "unsupported classified-sweep materializer: {message}"
                )
            }
            Self::ProductFactorOutOfBounds {
                factor_index,
                factor_count,
            } => write!(
                formatter,
                "classified-sweep factor {factor_index} is outside {factor_count} product factors"
            ),
            Self::InvalidCheckedClassification(case_id) => write!(
                formatter,
                "checked classifier returned an inconsistent admission/FIND pair for case {}",
                hex(case_id.bytes())
            ),
            Self::OrderedClassifierOutcomeCountMismatch { expected, actual } => write!(
                formatter,
                "ordered classifier returned {actual} outcomes for {expected} canonical cases"
            ),
            Self::CardinalityOverflow => {
                formatter.write_str("classified-sweep cardinality exceeds u128")
            }
            Self::InvalidQuery(message) => write!(formatter, "invalid Explore query: {message}"),
            Self::ChunkPartition(error) => write!(formatter, "invalid case chunk: {error}"),
            Self::Source(error) => write!(formatter, "source materialization failed: {error}"),
            Self::Case(error) => write!(formatter, "case classification failed: {error}"),
            Self::SupportCell(error) => write!(formatter, "invalid classified support: {error}"),
        }
    }
}

impl Error for RelationalClassifiedSweepError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChunkPartition(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Case(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

fn hex(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
