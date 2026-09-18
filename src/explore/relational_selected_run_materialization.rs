//! Exact sparse materialization of one classified selected support run.
//!
//! A classified chunk proves the WHERE/FIND outcome of every coordinate while
//! retaining only homogeneous support intervals. This producer revisits one
//! such interval only when at least one question selects it and retains the
//! canonical concrete cases needed by later relation, result, and mechanism
//! layers. It follows the same checked independent-FROM and singleton-TO path
//! as the exhaustive classifier and rejects any coordinate whose complete
//! ordered question outcome no longer matches the classified run.
//!
//! The retained artifact is bounded to one V1 chunk and is producer evidence,
//! not proof authority by itself. Codec restoration validates only canonical
//! shape, content identities, coverage, and artifact identity. The opaque
//! verified token is recovered separately against the checked query, support
//! plan, and exact verified classified chunk; replay does not rerun user code.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::CheckedExploreQueryView;

use super::relation::{
    AdmissionId, QuestionId, RelationId, RelationProvenance, RelationalCaseId, SelectionDecision,
    SourceKey, SourceRow, SuccessorKey, SuccessorRow,
};
use super::relational_bounded_chunk_partition::{
    decode_relational_case_chunk_finite_ordinals, RelationalCaseChunkId,
    RelationalCaseChunkPartitionArtifactId, RelationalCaseChunkPartitionError,
    RelationalCaseChunkUnsupported, VerifiedRelationalCaseChunkPartition,
    RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1,
};
use super::relational_case_executor::{RelationalCaseExecutor, RelationalCaseExecutorError};
use super::relational_classification_capsule::FrozenClassificationQuestionSet;
use super::relational_classified_sweep::{
    RelationalClassifiedCaseOutcome, RelationalClassifiedChunkArtifactId,
    RelationalClassifiedRunId, VerifiedRelationalClassifiedChunk,
};
use super::relational_executor::{
    RelationalExpressionRuntime, RelationalSourceEnumerator, RelationalSourceExecutorError,
};
use super::relational_support_planner::{
    RelationalBindingStage, RelationalCaseImageInjectivityProofError, RelationalSupportPlan,
    RelationalSupportPlanRoot,
};
use super::support_cell::{SupportCellId, SupportMaterializerId};
use super::transition::canonical_explore_value_digest;

pub(crate) const RELATIONAL_SELECTED_RUN_MATERIALIZATION_VERSION: u32 = 2;

const SELECTED_RUN_CASES_ROOT_V2: &[u8] = b"futuruna.explore.relational-selected-run.cases-root.v2";
const SELECTED_RUN_ARTIFACT_ID_V2: &[u8] =
    b"futuruna.explore.relational-selected-run.artifact-id.v2";

/// Stable identity of one bounded selected-run materialization artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalSelectedRunMaterializationArtifactId([u8; 32]);

impl RelationalSelectedRunMaterializationArtifactId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One exact concrete case retained at its canonical support coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedCaseRecord {
    coordinate_ordinal: u128,
    source_key: SourceKey,
    source: SourceRow,
    successor_key: SuccessorKey,
    successor: SuccessorRow,
    case_id: RelationalCaseId,
}

impl RelationalSelectedCaseRecord {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        coordinate_ordinal: u128,
        source_key: SourceKey,
        source: SourceRow,
        successor_key: SuccessorKey,
        successor: SuccessorRow,
        case_id: RelationalCaseId,
    ) -> Self {
        Self {
            coordinate_ordinal,
            source_key,
            source,
            successor_key,
            successor,
            case_id,
        }
    }

    pub(crate) const fn coordinate_ordinal(&self) -> u128 {
        self.coordinate_ordinal
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn source(&self) -> &SourceRow {
        &self.source
    }

    pub(crate) const fn successor_key(&self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn successor(&self) -> &SuccessorRow {
        &self.successor
    }

    pub(crate) const fn case_id(&self) -> RelationalCaseId {
        self.case_id
    }
}

/// Replayable bounded transcript for one exact selected support interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedRunMaterializationArtifact {
    schema_version: u32,
    id: RelationalSelectedRunMaterializationArtifactId,
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    selected_question_ids: Box<[QuestionId]>,
    classified_chunk_artifact_id: RelationalClassifiedChunkArtifactId,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    chunk_cell_id: SupportCellId,
    chunk_materializer_id: SupportMaterializerId,
    run_id: RelationalClassifiedRunId,
    run_ordinal: u16,
    run_cell_id: SupportCellId,
    run_materializer_id: SupportMaterializerId,
    interval_start: u128,
    interval_end_exclusive: u128,
    materialized_case_count: u128,
    materialized_cases_root: [u8; 32],
    cases: Box<[RelationalSelectedCaseRecord]>,
}

impl RelationalSelectedRunMaterializationArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        schema_version: u32,
        id: RelationalSelectedRunMaterializationArtifactId,
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        admission_id: AdmissionId,
        selected_question_ids: Box<[QuestionId]>,
        classified_chunk_artifact_id: RelationalClassifiedChunkArtifactId,
        chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        chunk_cell_id: SupportCellId,
        chunk_materializer_id: SupportMaterializerId,
        run_id: RelationalClassifiedRunId,
        run_ordinal: u16,
        run_cell_id: SupportCellId,
        run_materializer_id: SupportMaterializerId,
        interval_start: u128,
        interval_end_exclusive: u128,
        materialized_case_count: u128,
        materialized_cases_root: [u8; 32],
        cases: Box<[RelationalSelectedCaseRecord]>,
    ) -> Result<Self, RelationalSelectedRunMaterializationError> {
        let artifact = Self {
            schema_version,
            id,
            plan_root,
            relation_id,
            admission_id,
            selected_question_ids,
            classified_chunk_artifact_id,
            chunk_partition_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
            run_id,
            run_ordinal,
            run_cell_id,
            run_materializer_id,
            interval_start,
            interval_end_exclusive,
            materialized_case_count,
            materialized_cases_root,
            cases,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn id(&self) -> RelationalSelectedRunMaterializationArtifactId {
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

    pub(crate) fn selected_question_ids(&self) -> &[QuestionId] {
        &self.selected_question_ids
    }

    pub(crate) fn contains_question(&self, question_id: QuestionId) -> bool {
        self.selected_question_ids
            .binary_search(&question_id)
            .is_ok()
    }

    pub(crate) const fn classified_chunk_artifact_id(&self) -> RelationalClassifiedChunkArtifactId {
        self.classified_chunk_artifact_id
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

    pub(crate) const fn run_id(&self) -> RelationalClassifiedRunId {
        self.run_id
    }

    pub(crate) const fn run_ordinal(&self) -> u16 {
        self.run_ordinal
    }

    pub(crate) const fn run_cell_id(&self) -> SupportCellId {
        self.run_cell_id
    }

    pub(crate) const fn run_materializer_id(&self) -> SupportMaterializerId {
        self.run_materializer_id
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn materialized_case_count(&self) -> u128 {
        self.materialized_case_count
    }

    pub(crate) const fn materialized_cases_root(&self) -> [u8; 32] {
        self.materialized_cases_root
    }

    pub(crate) fn cases(&self) -> &[RelationalSelectedCaseRecord] {
        &self.cases
    }

    fn validate_identity(&self) -> Result<(), RelationalSelectedRunMaterializationError> {
        if self.schema_version != RELATIONAL_SELECTED_RUN_MATERIALIZATION_VERSION {
            return Err(
                RelationalSelectedRunMaterializationError::UnsupportedArtifactVersion {
                    actual: self.schema_version,
                    expected: RELATIONAL_SELECTED_RUN_MATERIALIZATION_VERSION,
                },
            );
        }
        if self.interval_start >= self.interval_end_exclusive {
            return Err(
                RelationalSelectedRunMaterializationError::InvalidArtifactShape(
                    "selected-run interval is empty or reversed",
                ),
            );
        }
        if self.selected_question_ids.is_empty()
            || !self
                .selected_question_ids
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(
                RelationalSelectedRunMaterializationError::InvalidArtifactShape(
                    "selected question IDs must be nonempty, sorted, and duplicate-free",
                ),
            );
        }
        let interval_cardinality = self.interval_end_exclusive - self.interval_start;
        if interval_cardinality > RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1
            || self.materialized_case_count != interval_cardinality
            || u128::try_from(self.cases.len()).ok() != Some(interval_cardinality)
        {
            return Err(
                RelationalSelectedRunMaterializationError::InvalidArtifactShape(
                    "selected-run count or V1 bound is not canonical",
                ),
            );
        }

        let mut source_keys = BTreeSet::new();
        let mut successor_keys = BTreeSet::new();
        let mut case_ids = BTreeSet::new();
        for (offset, record) in self.cases.iter().enumerate() {
            let offset = u128::try_from(offset)
                .map_err(|_| RelationalSelectedRunMaterializationError::CardinalityOverflow)?;
            let expected_coordinate = self
                .interval_start
                .checked_add(offset)
                .ok_or(RelationalSelectedRunMaterializationError::CardinalityOverflow)?;
            if record.coordinate_ordinal != expected_coordinate {
                return Err(
                    RelationalSelectedRunMaterializationError::InvalidArtifactShape(
                        "selected cases do not form a contiguous coordinate cover",
                    ),
                );
            }
            validate_case_record(self.relation_id, record)?;
            if !source_keys.insert(record.source_key) {
                return Err(
                    RelationalSelectedRunMaterializationError::DuplicateSourceKey {
                        coordinate: record.coordinate_ordinal,
                    },
                );
            }
            if !successor_keys.insert(record.successor_key) {
                return Err(
                    RelationalSelectedRunMaterializationError::DuplicateSuccessorKey {
                        coordinate: record.coordinate_ordinal,
                    },
                );
            }
            if !case_ids.insert(record.case_id) {
                return Err(RelationalSelectedRunMaterializationError::DuplicateCaseId {
                    coordinate: record.coordinate_ordinal,
                });
            }
        }

        if derive_materialized_cases_root(self) != self.materialized_cases_root {
            return Err(RelationalSelectedRunMaterializationError::CasesRootMismatch);
        }
        if derive_artifact_id(self) != self.id {
            return Err(RelationalSelectedRunMaterializationError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// Opaque structural authority recovered against the exact selected run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalSelectedRunMaterialization {
    artifact: RelationalSelectedRunMaterializationArtifact,
}

impl VerifiedRelationalSelectedRunMaterialization {
    pub(crate) const fn artifact(&self) -> &RelationalSelectedRunMaterializationArtifact {
        &self.artifact
    }

    pub(crate) fn cases(&self) -> &[RelationalSelectedCaseRecord] {
        self.artifact.cases()
    }
}

/// Fresh checked producer output before a future journal accepts its artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedRunMaterialization {
    verified: VerifiedRelationalSelectedRunMaterialization,
}

impl RelationalSelectedRunMaterialization {
    pub(crate) const fn verified(&self) -> &VerifiedRelationalSelectedRunMaterialization {
        &self.verified
    }

    pub(crate) const fn artifact(&self) -> &RelationalSelectedRunMaterializationArtifact {
        self.verified.artifact()
    }
}

/// Re-evaluate and retain every exact case in one selected classified run.
pub(crate) fn materialize_relational_selected_run<R: RelationalExpressionRuntime>(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    verified_partition: &VerifiedRelationalCaseChunkPartition,
    classified_chunk: &VerifiedRelationalClassifiedChunk,
    selected_run_ordinal: u16,
    runtime: &mut R,
) -> Result<RelationalSelectedRunMaterialization, RelationalSelectedRunMaterializationError> {
    let scope = validate_selected_run_scope(
        checked,
        plan,
        verified_partition,
        classified_chunk,
        selected_run_ordinal,
    )?;
    let source =
        RelationalSourceEnumerator::new(checked.relation_id(), &checked.closed_query.source)?;
    let cases = RelationalCaseExecutor::new(checked.relation_id(), checked.closed_query)?;
    let questions = cases.checked_question_evaluation_plan(checked)?;
    let evaluated_question_ids = questions.question_ids().collect::<Vec<_>>();
    if evaluated_question_ids.as_slice() != plan.question_ids() {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }
    let capacity = usize::try_from(scope.cardinality())
        .map_err(|_| RelationalSelectedRunMaterializationError::CardinalityOverflow)?;
    let mut records = Vec::with_capacity(capacity);

    for coordinate in scope.interval_start..scope.interval_end_exclusive {
        let finite_ordinals = decode_relational_case_chunk_finite_ordinals(
            verified_partition.partition(),
            scope.chunk_ordinal,
            coordinate,
            scope.finite_factor_count,
        )?;
        let completed =
            source.completed_source_at_independent_finite_ordinals(&finite_ordinals, runtime)?;
        let transition = cases
            .statically_singleton_transition(completed.source_key(), completed.row(), runtime)?
            .ok_or(
                RelationalSelectedRunMaterializationError::UnsupportedMaterializerShape(
                    "selected-run materialization requires a statically singleton TO relation",
                ),
            )?;
        let (case, _) = transition.into_parts();
        let classification = cases.classify(completed.row(), &case, &questions, runtime)?;
        if classification.case_id() != case.case_id()
            || classification.admission() != scope.outcome.admission()
            || classification.question_evidence().len() != scope.questions.question_ids().len()
            || classification
                .question_evidence()
                .iter()
                .zip(scope.questions.question_ids().iter().copied())
                .enumerate()
                .any(|(question_index, (question, question_id))| {
                    question.question_id() != question_id
                        || scope.outcome.selection(question_index) != Some(question.decision())
                })
        {
            return Err(
                RelationalSelectedRunMaterializationError::SelectedClassificationMismatch {
                    coordinate,
                    case_id: case.case_id(),
                },
            );
        }

        let record = RelationalSelectedCaseRecord {
            coordinate_ordinal: coordinate,
            source_key: completed.source_key(),
            source: completed.row().clone(),
            successor_key: case.successor_key(),
            successor: case.successor().clone(),
            case_id: case.case_id(),
        };
        validate_case_record(checked.relation_id(), &record)?;
        records.push(record);
    }

    let materialized_case_count = scope.cardinality();
    let mut artifact = RelationalSelectedRunMaterializationArtifact {
        schema_version: RELATIONAL_SELECTED_RUN_MATERIALIZATION_VERSION,
        id: RelationalSelectedRunMaterializationArtifactId([0; 32]),
        plan_root: scope.plan_root,
        relation_id: scope.relation_id,
        admission_id: scope.admission_id,
        selected_question_ids: scope.selected_question_ids,
        classified_chunk_artifact_id: scope.classified_chunk_artifact_id,
        chunk_partition_id: scope.chunk_partition_id,
        chunk_id: scope.chunk_id,
        chunk_ordinal: scope.chunk_ordinal,
        chunk_cell_id: scope.chunk_cell_id,
        chunk_materializer_id: scope.chunk_materializer_id,
        run_id: scope.run_id,
        run_ordinal: scope.run_ordinal,
        run_cell_id: scope.run_cell_id,
        run_materializer_id: scope.run_materializer_id,
        interval_start: scope.interval_start,
        interval_end_exclusive: scope.interval_end_exclusive,
        materialized_case_count,
        materialized_cases_root: [0; 32],
        cases: records.into_boxed_slice(),
    };
    artifact.materialized_cases_root = derive_materialized_cases_root(&artifact);
    artifact.id = derive_artifact_id(&artifact);
    artifact.validate_identity()?;
    Ok(RelationalSelectedRunMaterialization {
        verified: VerifiedRelationalSelectedRunMaterialization { artifact },
    })
}

/// Restore opaque authority without rerunning user code.
///
/// The artifact's rows remain trusted checked-producer output, like retained
/// concrete classification events. This function independently reconstructs
/// the addressed chunk/run from the partition authority reconstructed by the
/// journal and matches all artifact scope before exposing its cases.
pub(crate) fn reverify_relational_selected_run_materialization_artifact(
    artifact: &RelationalSelectedRunMaterializationArtifact,
    plan: &RelationalSupportPlan,
    verified_partition: &VerifiedRelationalCaseChunkPartition,
    classified_chunk: &VerifiedRelationalClassifiedChunk,
    selected_run_ordinal: u16,
) -> Result<VerifiedRelationalSelectedRunMaterialization, RelationalSelectedRunMaterializationError>
{
    artifact.validate_identity()?;
    let scope = validate_selected_run_scope_from_plan(
        plan,
        verified_partition,
        classified_chunk,
        selected_run_ordinal,
    )?;
    if !scope.matches_artifact(artifact) {
        return Err(RelationalSelectedRunMaterializationError::ArtifactSemanticMismatch);
    }
    Ok(VerifiedRelationalSelectedRunMaterialization {
        artifact: artifact.clone(),
    })
}

#[derive(Clone)]
struct SelectedRunScope {
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    questions: FrozenClassificationQuestionSet,
    outcome: RelationalClassifiedCaseOutcome,
    selected_question_ids: Box<[QuestionId]>,
    classified_chunk_artifact_id: RelationalClassifiedChunkArtifactId,
    chunk_partition_id: RelationalCaseChunkPartitionArtifactId,
    chunk_id: RelationalCaseChunkId,
    chunk_ordinal: u128,
    chunk_cell_id: SupportCellId,
    chunk_materializer_id: SupportMaterializerId,
    run_id: RelationalClassifiedRunId,
    run_ordinal: u16,
    run_cell_id: SupportCellId,
    run_materializer_id: SupportMaterializerId,
    interval_start: u128,
    interval_end_exclusive: u128,
    finite_factor_count: usize,
}

impl SelectedRunScope {
    const fn cardinality(&self) -> u128 {
        self.interval_end_exclusive - self.interval_start
    }

    fn matches_artifact(&self, artifact: &RelationalSelectedRunMaterializationArtifact) -> bool {
        artifact.plan_root == self.plan_root
            && artifact.relation_id == self.relation_id
            && artifact.admission_id == self.admission_id
            && artifact.selected_question_ids == self.selected_question_ids
            && artifact.classified_chunk_artifact_id == self.classified_chunk_artifact_id
            && artifact.chunk_partition_id == self.chunk_partition_id
            && artifact.chunk_id == self.chunk_id
            && artifact.chunk_ordinal == self.chunk_ordinal
            && artifact.chunk_cell_id == self.chunk_cell_id
            && artifact.chunk_materializer_id == self.chunk_materializer_id
            && artifact.run_id == self.run_id
            && artifact.run_ordinal == self.run_ordinal
            && artifact.run_cell_id == self.run_cell_id
            && artifact.run_materializer_id == self.run_materializer_id
            && artifact.interval_start == self.interval_start
            && artifact.interval_end_exclusive == self.interval_end_exclusive
            && artifact.materialized_case_count == self.cardinality()
    }
}

fn validate_selected_run_scope(
    checked: &CheckedExploreQueryView<'_>,
    plan: &RelationalSupportPlan,
    verified_partition: &VerifiedRelationalCaseChunkPartition,
    classified_chunk: &VerifiedRelationalClassifiedChunk,
    selected_run_ordinal: u16,
) -> Result<SelectedRunScope, RelationalSelectedRunMaterializationError> {
    checked
        .closed_query
        .validate()
        .map_err(RelationalSelectedRunMaterializationError::InvalidQuery)?;
    if !plan.validate_root()
        || plan.relation_id() != checked.relation_id()
        || plan.admission_id() != checked.admission_id()
        || plan.question_ids() != checked.question_ids()
    {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }

    validate_selected_run_scope_from_plan(
        plan,
        verified_partition,
        classified_chunk,
        selected_run_ordinal,
    )
}

/// Recover the exact selected support interval during journal replay without
/// rerunning user expressions. The checked producer already committed the
/// concrete rows; replay indexes the already reconstructed partition authority,
/// rebuilds only the addressed bounded run, and validates every content-derived
/// row identity.
fn validate_selected_run_scope_from_plan(
    plan: &RelationalSupportPlan,
    verified_partition: &VerifiedRelationalCaseChunkPartition,
    classified_chunk: &VerifiedRelationalClassifiedChunk,
    selected_run_ordinal: u16,
) -> Result<SelectedRunScope, RelationalSelectedRunMaterializationError> {
    let questions = FrozenClassificationQuestionSet::freeze(plan.question_ids().iter().copied())
        .map_err(|_| RelationalSelectedRunMaterializationError::ScopeMismatch)?;
    let classified_artifact = classified_chunk.artifact();
    if !plan.validate_root()
        || classified_artifact.plan_root() != plan.root()
        || classified_artifact.relation_id() != plan.relation_id()
        || classified_artifact.admission_id() != plan.admission_id()
        || classified_artifact.question_set() != &questions
    {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }

    let partition = verified_partition.partition();
    if verified_partition.artifact().plan_root() != plan.root()
        || verified_partition.artifact().relation_id() != plan.relation_id()
        || verified_partition.artifact().admission_id() != plan.admission_id()
        || verified_partition.artifact().questions() != &questions
        || verified_partition.artifact().id() != classified_artifact.chunk_partition_id()
    {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }
    let chunk_index = usize::try_from(classified_artifact.chunk_ordinal())
        .map_err(|_| RelationalSelectedRunMaterializationError::ScopeMismatch)?;
    let chunk = partition
        .chunks()
        .get(chunk_index)
        .ok_or(RelationalSelectedRunMaterializationError::ScopeMismatch)?;
    if chunk.descriptor().id() != classified_artifact.chunk_id()
        || chunk.descriptor().ordinal() != classified_artifact.chunk_ordinal()
        || chunk.cell().id() != classified_artifact.chunk_cell_id()
        || chunk.cell().materializer_id() != classified_artifact.chunk_materializer_id()
        || chunk.descriptor().interval_start() != classified_artifact.interval_start()
        || chunk.descriptor().interval_end_exclusive()
            != classified_artifact.interval_end_exclusive()
    {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }

    let run_index = usize::from(selected_run_ordinal);
    let (run, bindings) = classified_chunk.run_and_bindings(run_index).ok_or(
        RelationalSelectedRunMaterializationError::RunOrdinalOutOfBounds {
            run_ordinal: selected_run_ordinal,
        },
    )?;
    if run.descriptor().ordinal() != selected_run_ordinal {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }
    let outcome = run.descriptor().outcome().clone();
    if !outcome.any_selected() {
        return Err(
            RelationalSelectedRunMaterializationError::RunIsNotSelected {
                run_ordinal: selected_run_ordinal,
            },
        );
    }
    let proper_run_restriction = run.cell().id() != chunk.cell().id();
    if bindings.injectivity().is_some() != proper_run_restriction {
        return Err(RelationalSelectedRunMaterializationError::ScopeMismatch);
    }
    let interval_start = run.descriptor().interval_start();
    let interval_end_exclusive = run.descriptor().interval_end_exclusive();
    let cardinality = interval_end_exclusive.checked_sub(interval_start).ok_or(
        RelationalSelectedRunMaterializationError::InvalidArtifactShape(
            "selected run is empty or reversed",
        ),
    )?;
    if cardinality == 0
        || cardinality > RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1
        || interval_start < chunk.descriptor().interval_start()
        || interval_end_exclusive > chunk.descriptor().interval_end_exclusive()
    {
        return Err(
            RelationalSelectedRunMaterializationError::InvalidArtifactShape(
                "selected run is outside its canonical bounded chunk",
            ),
        );
    }

    let finite_factor_count = plan
        .stages()
        .iter()
        .filter(|stage| matches!(stage, RelationalBindingStage::Finite(_)))
        .count();
    decode_relational_case_chunk_finite_ordinals(
        partition,
        chunk.descriptor().ordinal(),
        interval_start,
        finite_factor_count,
    )?;

    let selected_question_ids = questions
        .question_ids()
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(question_index, question_id)| {
            (outcome.selection(question_index) == Some(SelectionDecision::Selected))
                .then_some(question_id)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    if selected_question_ids.is_empty() {
        return Err(
            RelationalSelectedRunMaterializationError::RunIsNotSelected {
                run_ordinal: selected_run_ordinal,
            },
        );
    }

    Ok(SelectedRunScope {
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        admission_id: plan.admission_id(),
        questions,
        outcome,
        selected_question_ids,
        classified_chunk_artifact_id: classified_artifact.id(),
        chunk_partition_id: partition.artifact().id(),
        chunk_id: chunk.descriptor().id(),
        chunk_ordinal: chunk.descriptor().ordinal(),
        chunk_cell_id: chunk.cell().id(),
        chunk_materializer_id: chunk.cell().materializer_id(),
        run_id: run.descriptor().id(),
        run_ordinal: run.descriptor().ordinal(),
        run_cell_id: run.cell().id(),
        run_materializer_id: run.cell().materializer_id(),
        interval_start,
        interval_end_exclusive,
        finite_factor_count,
    })
}

fn validate_case_record(
    relation_id: RelationId,
    record: &RelationalSelectedCaseRecord,
) -> Result<(), RelationalSelectedRunMaterializationError> {
    let derived_source_key = SourceKey::derive(relation_id, &record.source);
    if derived_source_key != record.source_key {
        return Err(
            RelationalSelectedRunMaterializationError::SourceKeyMismatch {
                coordinate: record.coordinate_ordinal,
            },
        );
    }
    let derived_successor_key =
        SuccessorKey::derive(relation_id, record.source_key, &record.successor);
    if derived_successor_key != record.successor_key {
        return Err(
            RelationalSelectedRunMaterializationError::SuccessorKeyMismatch {
                coordinate: record.coordinate_ordinal,
            },
        );
    }
    let derived_case_id =
        RelationalCaseId::derive(relation_id, record.source_key, record.successor_key);
    if derived_case_id != record.case_id {
        return Err(RelationalSelectedRunMaterializationError::CaseIdMismatch {
            coordinate: record.coordinate_ordinal,
        });
    }
    Ok(())
}

fn derive_materialized_cases_root(
    artifact: &RelationalSelectedRunMaterializationArtifact,
) -> [u8; 32] {
    let mut hasher = SelectedRunHasher::new(SELECTED_RUN_CASES_ROOT_V2);
    hash_scope(&mut hasher, artifact);
    hasher.u128(artifact.materialized_case_count);
    hasher.u128(artifact.cases.len() as u128);
    for record in artifact.cases.iter() {
        hash_case_record(&mut hasher, record);
    }
    hasher.finish()
}

fn derive_artifact_id(
    artifact: &RelationalSelectedRunMaterializationArtifact,
) -> RelationalSelectedRunMaterializationArtifactId {
    let mut hasher = SelectedRunHasher::new(SELECTED_RUN_ARTIFACT_ID_V2);
    hash_scope(&mut hasher, artifact);
    hasher.u128(artifact.materialized_case_count);
    hasher.digest(artifact.materialized_cases_root);
    RelationalSelectedRunMaterializationArtifactId(hasher.finish())
}

fn hash_scope(
    hasher: &mut SelectedRunHasher,
    artifact: &RelationalSelectedRunMaterializationArtifact,
) {
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.u128(artifact.selected_question_ids.len() as u128);
    for question_id in artifact.selected_question_ids.iter().copied() {
        hasher.digest(question_id.bytes());
    }
    hasher.digest(artifact.classified_chunk_artifact_id.bytes());
    hasher.digest(artifact.chunk_partition_id.bytes());
    hasher.digest(artifact.chunk_id.bytes());
    hasher.u128(artifact.chunk_ordinal);
    hasher.digest(artifact.chunk_cell_id.bytes());
    hasher.digest(artifact.chunk_materializer_id.bytes());
    hasher.digest(artifact.run_id.bytes());
    hasher.u16(artifact.run_ordinal);
    hasher.digest(artifact.run_cell_id.bytes());
    hasher.digest(artifact.run_materializer_id.bytes());
    hasher.u128(artifact.interval_start);
    hasher.u128(artifact.interval_end_exclusive);
}

fn hash_case_record(hasher: &mut SelectedRunHasher, record: &RelationalSelectedCaseRecord) {
    hasher.u128(record.coordinate_ordinal);
    hasher.digest(record.source_key.bytes());
    hasher.digest(canonical_explore_value_digest(record.source.context()));
    hasher.digest(canonical_explore_value_digest(record.source.before()));
    hash_provenance(hasher, record.source.provenance());
    hasher.digest(record.successor_key.bytes());
    hasher.digest(canonical_explore_value_digest(record.successor.after()));
    hash_provenance(hasher, record.successor.provenance());
    hasher.digest(record.case_id.bytes());
}

fn hash_provenance(hasher: &mut SelectedRunHasher, provenance: &RelationProvenance) {
    hasher.u128(provenance.lineage().len() as u128);
    for lineage in provenance.lineage() {
        hasher.digest(lineage.bytes());
    }
    hasher.u128(provenance.support().len() as u128);
    for support in provenance.support() {
        hasher.digest(support.bytes());
    }
}

struct SelectedRunHasher(Sha256);

impl SelectedRunHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
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

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSelectedRunMaterializationError {
    UnsupportedArtifactVersion {
        actual: u32,
        expected: u32,
    },
    InvalidArtifactShape(&'static str),
    ArtifactIdentityMismatch,
    CasesRootMismatch,
    ArtifactSemanticMismatch,
    ScopeMismatch,
    AlreadyBounded,
    UnsupportedPartition(RelationalCaseChunkUnsupported),
    UnsupportedMaterializerShape(&'static str),
    RunOrdinalOutOfBounds {
        run_ordinal: u16,
    },
    RunIsNotSelected {
        run_ordinal: u16,
    },
    SelectedClassificationMismatch {
        coordinate: u128,
        case_id: RelationalCaseId,
    },
    SourceKeyMismatch {
        coordinate: u128,
    },
    SuccessorKeyMismatch {
        coordinate: u128,
    },
    CaseIdMismatch {
        coordinate: u128,
    },
    DuplicateSourceKey {
        coordinate: u128,
    },
    DuplicateSuccessorKey {
        coordinate: u128,
    },
    DuplicateCaseId {
        coordinate: u128,
    },
    CardinalityOverflow,
    InvalidQuery(String),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    ChunkPartition(RelationalCaseChunkPartitionError),
    Source(RelationalSourceExecutorError),
    Case(RelationalCaseExecutorError),
}

impl From<RelationalCaseImageInjectivityProofError> for RelationalSelectedRunMaterializationError {
    fn from(error: RelationalCaseImageInjectivityProofError) -> Self {
        Self::CaseImageProof(error)
    }
}

impl From<RelationalCaseChunkPartitionError> for RelationalSelectedRunMaterializationError {
    fn from(error: RelationalCaseChunkPartitionError) -> Self {
        Self::ChunkPartition(error)
    }
}

impl From<RelationalSourceExecutorError> for RelationalSelectedRunMaterializationError {
    fn from(error: RelationalSourceExecutorError) -> Self {
        Self::Source(error)
    }
}

impl From<RelationalCaseExecutorError> for RelationalSelectedRunMaterializationError {
    fn from(error: RelationalCaseExecutorError) -> Self {
        Self::Case(error)
    }
}

impl fmt::Display for RelationalSelectedRunMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion { actual, expected } => write!(
                formatter,
                "unsupported selected-run materialization version {actual}; expected {expected}"
            ),
            Self::InvalidArtifactShape(message) => {
                write!(formatter, "invalid selected-run artifact: {message}")
            }
            Self::ArtifactIdentityMismatch => formatter
                .write_str("selected-run artifact identity does not match its canonical content"),
            Self::CasesRootMismatch => formatter
                .write_str("selected-run cases root does not match its canonical case records"),
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "selected-run artifact does not match the checked plan and verified classified run",
            ),
            Self::ScopeMismatch => formatter.write_str(
                "selected-run query, support plan, partition, chunk, or classified scope disagrees",
            ),
            Self::AlreadyBounded => formatter.write_str(
                "selected-run materialization requires a classified partition child, but the case root is already bounded",
            ),
            Self::UnsupportedPartition(reason) => {
                write!(formatter, "selected-run materialization does not support partition shape {reason:?}")
            }
            Self::UnsupportedMaterializerShape(message) => {
                write!(formatter, "unsupported selected-run materializer: {message}")
            }
            Self::RunOrdinalOutOfBounds { run_ordinal } => {
                write!(formatter, "classified chunk has no run ordinal {run_ordinal}")
            }
            Self::RunIsNotSelected { run_ordinal } => write!(
                formatter,
                "classified run ordinal {run_ordinal} is not admitted+selected"
            ),
            Self::SelectedClassificationMismatch {
                coordinate,
                case_id,
            } => write!(
                formatter,
                "coordinate {coordinate} no longer evaluates admitted+selected for case {}",
                hex(case_id.bytes())
            ),
            Self::SourceKeyMismatch { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} has a SourceKey inconsistent with its source row"
            ),
            Self::SuccessorKeyMismatch { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} has a SuccessorKey inconsistent with its successor row"
            ),
            Self::CaseIdMismatch { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} has a CaseId inconsistent with its endpoint keys"
            ),
            Self::DuplicateSourceKey { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} duplicates a SourceKey in an injective run"
            ),
            Self::DuplicateSuccessorKey { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} duplicates a SuccessorKey in an injective run"
            ),
            Self::DuplicateCaseId { coordinate } => write!(
                formatter,
                "selected coordinate {coordinate} duplicates a CaseId in an injective run"
            ),
            Self::CardinalityOverflow => {
                formatter.write_str("selected-run cardinality exceeds a canonical integer bound")
            }
            Self::InvalidQuery(message) => write!(formatter, "invalid Explore query: {message}"),
            Self::CaseImageProof(error) => write!(formatter, "invalid case-image proof: {error}"),
            Self::ChunkPartition(error) => write!(formatter, "invalid bounded partition: {error}"),
            Self::Source(error) => write!(formatter, "selected source materialization failed: {error}"),
            Self::Case(error) => write!(formatter, "selected case materialization failed: {error}"),
        }
    }
}

impl Error for RelationalSelectedRunMaterializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CaseImageProof(error) => Some(error),
            Self::ChunkPartition(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Case(error) => Some(error),
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
