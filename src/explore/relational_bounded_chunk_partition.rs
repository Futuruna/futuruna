//! Canonical bounded chunks for one exact injective mapped case root.
//!
//! This module is a pure planner. It neither evaluates admission/FIND outcomes
//! nor mutates a journal. V1 recognizes the producer shape needed by the
//! original one-dimensional ordered concrete fallback:
//!
//! - the verified case image is the exact composed image of an independent
//!   assignment product and a singleton successor;
//! - the case-root coordinate expression is either one `OrdinalInterval`, or
//!   a product containing exactly one varying `OrdinalInterval` while every
//!   other factor has exact cardinality one; and
//! - accepted case-image injectivity lifts the coordinate chunks into an exact
//!   mapped-image partition.
//!
//! The V1 maximum is 256 coordinates. This conservative power of two bounds
//! time to the first durable classified chunk for an expensive observer such
//! as Personskat, while a 200,000-coordinate root still needs only 782
//! children. Physical workers may use smaller quanta inside a chunk; the fixed
//! maximum is semantic because it determines child-cell and artifact identity.
//!
//! V2 added one shape without changing any V1 artifact or identity: an exact
//! independent product of two or more varying zero-based ordinal factors is
//! linearized by canonical mixed-radix rank (last factor fastest) and split
//! into `ProductRankInterval` children. This planner still requires the same
//! separately verified assignment-to-case injectivity proof; rankability is
//! never treated as evidence that the mapped image is injective.
//!
//! V3 binds every partition shape to the complete canonical classification
//! question set. The partition remains shared structural scheduling data: it
//! has no primary question, and an empty set is valid when the enclosing
//! support plan permits one.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionId, QuestionId, RelationId};
use super::relational_classification_capsule::{
    ClassificationQuestionSetRoot, FrozenClassificationQuestionSet,
};
use super::relational_support_planner::{
    prove_relational_case_image_injectivity, RelationalCaseImageAssignmentKind,
    RelationalCaseImageInjectivityProof, RelationalCaseImageInjectivityProofError,
    RelationalCaseImagePreimageKind, RelationalSourceAssignmentImageProof,
    RelationalSuccessorRecipeKind, RelationalSupportPlan, RelationalSupportPlanRoot,
};
use super::support_cell::{
    CertifiedInjective, InjectiveMappingClaim, SupportCardinality, SupportCell, SupportCellClaim,
    SupportCellError, SupportCellEvidence, SupportCellEvidenceId, SupportCellId,
    SupportCellObligation, SupportExpr, SupportExprKind, SupportMaterializerId,
    SupportPartitionCertificate, SupportPartitionId, SupportPartitionKind,
    SupportProofObligationId, SupportProofReceiptId,
};

pub(crate) const RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V1: u32 = 1;
pub(crate) const RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V2: u32 = 2;
pub(crate) const RELATIONAL_CASE_CHUNK_PARTITION_VERSION: u32 = 3;

/// Semantic V1 ceiling for one later exhaustive classification chunk.
pub(crate) const RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1: u128 = 256;

// Until canonical partition artifacts are paged, cap their eager expansion.
// At 112 encoded bytes per descriptor this keeps the descriptor array below
// 8 MiB, leaving room inside the default 16-MiB journal entry limit. Declining
// this accelerator preserves the ordinary bounded concrete scheduler.
const MAX_EAGER_CASE_CHUNKS: u128 = 65_536;

const CASE_CHUNK_ID_V1: &[u8] = b"futuruna.explore.relational-case-chunk.id.v1";
const CASE_CHUNK_PARTITION_ARTIFACT_ID_V1: &[u8] =
    b"futuruna.explore.relational-case-chunk.partition-artifact.v1";
const CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V1: &[u8] =
    b"futuruna.explore.relational-case-chunk.restricted-injectivity-proof.v1";
const CASE_CHUNK_ID_V2: &[u8] = b"futuruna.explore.relational-case-chunk.id.v2";
const CASE_CHUNK_PARTITION_ARTIFACT_ID_V2: &[u8] =
    b"futuruna.explore.relational-case-chunk.partition-artifact.v2";
const CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V2: &[u8] =
    b"futuruna.explore.relational-case-chunk.restricted-injectivity-proof.v2";
const CASE_CHUNK_ID_V3: &[u8] = b"futuruna.explore.relational-case-chunk.id.v3";
const CASE_CHUNK_PARTITION_ARTIFACT_ID_V3: &[u8] =
    b"futuruna.explore.relational-case-chunk.partition-artifact.v3";
const CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V3: &[u8] =
    b"futuruna.explore.relational-case-chunk.restricted-injectivity-proof.v3";

/// Canonical coordinate shape recognized by the versioned planner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalCaseChunkShape {
    BareOrdinalInterval,
    ProductFactor,
    ProductRankInterval,
}

impl RelationalCaseChunkShape {
    const fn tag(self) -> u8 {
        match self {
            Self::BareOrdinalInterval => 0x01,
            Self::ProductFactor => 0x02,
            Self::ProductRankInterval => 0x03,
        }
    }

    const fn partition_kind(self) -> SupportPartitionKind {
        match self {
            Self::BareOrdinalInterval => SupportPartitionKind::MappedInjectiveOrdinalCover,
            Self::ProductFactor => SupportPartitionKind::MappedInjectiveProductFactorCover,
            Self::ProductRankInterval => {
                SupportPartitionKind::MappedInjectiveProductRankIntervalCover
            }
        }
    }

    const fn factor_index_is_canonical(self, factor_index: Option<u32>) -> bool {
        matches!(
            (self, factor_index),
            (Self::BareOrdinalInterval, None)
                | (Self::ProductFactor, Some(_))
                | (Self::ProductRankInterval, None)
        )
    }

    const fn schema_version(self) -> u32 {
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION
    }
}

/// Content identity of one bounded child cell in a partition plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseChunkId([u8; 32]);

impl RelationalCaseChunkId {
    /// Future codec seam; the enclosing artifact must still validate the ID.
    pub(super) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical scheduling descriptor for one nonempty half-open child interval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseChunkDescriptor {
    id: RelationalCaseChunkId,
    ordinal: u128,
    cell_id: SupportCellId,
    interval_start: u128,
    interval_end_exclusive: u128,
}

impl RelationalCaseChunkDescriptor {
    /// Future codec seam. Basic interval shape is checked here; the enclosing
    /// artifact re-derives the ID against its complete semantic scope.
    pub(super) fn restore_from_canonical_parts(
        id: RelationalCaseChunkId,
        ordinal: u128,
        cell_id: SupportCellId,
        interval_start: u128,
        interval_end_exclusive: u128,
    ) -> Result<Self, RelationalCaseChunkPartitionError> {
        if interval_start >= interval_end_exclusive {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "chunk interval is empty or reversed",
            ));
        }
        Ok(Self {
            id,
            ordinal,
            cell_id,
            interval_start,
            interval_end_exclusive,
        })
    }

    pub(crate) const fn id(&self) -> RelationalCaseChunkId {
        self.id
    }

    pub(crate) const fn ordinal(&self) -> u128 {
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
}

/// Content identity of one complete canonical chunk-partition artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCaseChunkPartitionArtifactId([u8; 32]);

impl RelationalCaseChunkPartitionArtifactId {
    /// Future codec seam; the restored artifact re-derives the ID.
    pub(super) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Replayable canonical partition description. It is scheduling data, not
/// classification evidence and not proof authority by itself. Later replay
/// must call [`reverify_relational_case_chunk_partition_artifact`] against the
/// installed support plan before accepting its cells or partition certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseChunkPartitionArtifact {
    schema_version: u32,
    id: RelationalCaseChunkPartitionArtifactId,
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    questions: FrozenClassificationQuestionSet,
    case_image_certificate_id: [u8; 32],
    injectivity_evidence_id: SupportCellEvidenceId,
    root_cell_id: SupportCellId,
    root_materializer_id: SupportMaterializerId,
    shape: RelationalCaseChunkShape,
    factor_index: Option<u32>,
    interval_start: u128,
    interval_end_exclusive: u128,
    max_chunk_coordinates: u128,
    chunks: Box<[RelationalCaseChunkDescriptor]>,
    partition_id: SupportPartitionId,
}

impl RelationalCaseChunkPartitionArtifact {
    /// Future codec seam. This checks complete canonical structure and identity
    /// but does not recover proof authority from serialized bytes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_canonical_parts(
        schema_version: u32,
        id: RelationalCaseChunkPartitionArtifactId,
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        admission_id: AdmissionId,
        questions: FrozenClassificationQuestionSet,
        case_image_certificate_id: [u8; 32],
        injectivity_evidence_id: SupportCellEvidenceId,
        root_cell_id: SupportCellId,
        root_materializer_id: SupportMaterializerId,
        shape: RelationalCaseChunkShape,
        factor_index: Option<u32>,
        interval_start: u128,
        interval_end_exclusive: u128,
        max_chunk_coordinates: u128,
        chunks: Box<[RelationalCaseChunkDescriptor]>,
        partition_id: SupportPartitionId,
    ) -> Result<Self, RelationalCaseChunkPartitionError> {
        let artifact = Self {
            schema_version,
            id,
            plan_root,
            relation_id,
            admission_id,
            questions,
            case_image_certificate_id,
            injectivity_evidence_id,
            root_cell_id,
            root_materializer_id,
            shape,
            factor_index,
            interval_start,
            interval_end_exclusive,
            max_chunk_coordinates,
            chunks,
            partition_id,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn id(&self) -> RelationalCaseChunkPartitionArtifactId {
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

    pub(crate) const fn questions(&self) -> &FrozenClassificationQuestionSet {
        &self.questions
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        self.questions.question_ids()
    }

    pub(crate) const fn question_set_root(&self) -> ClassificationQuestionSetRoot {
        self.questions.root()
    }

    pub(crate) const fn case_image_certificate_id(&self) -> [u8; 32] {
        self.case_image_certificate_id
    }

    pub(crate) const fn injectivity_evidence_id(&self) -> SupportCellEvidenceId {
        self.injectivity_evidence_id
    }

    pub(crate) const fn root_cell_id(&self) -> SupportCellId {
        self.root_cell_id
    }

    pub(crate) const fn root_materializer_id(&self) -> SupportMaterializerId {
        self.root_materializer_id
    }

    pub(crate) const fn shape(&self) -> RelationalCaseChunkShape {
        self.shape
    }

    pub(crate) const fn factor_index(&self) -> Option<u32> {
        self.factor_index
    }

    pub(crate) const fn interval_start(&self) -> u128 {
        self.interval_start
    }

    pub(crate) const fn interval_end_exclusive(&self) -> u128 {
        self.interval_end_exclusive
    }

    pub(crate) const fn max_chunk_coordinates(&self) -> u128 {
        self.max_chunk_coordinates
    }

    pub(crate) fn chunks(&self) -> &[RelationalCaseChunkDescriptor] {
        &self.chunks
    }

    pub(crate) const fn partition_id(&self) -> SupportPartitionId {
        self.partition_id
    }

    pub(crate) fn validate_identity(&self) -> Result<(), RelationalCaseChunkPartitionError> {
        if self.schema_version != self.shape.schema_version() {
            return Err(
                RelationalCaseChunkPartitionError::UnsupportedArtifactVersion(self.schema_version),
            );
        }
        if !self.questions.validate_identity() {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "artifact question set is not canonical",
            ));
        }
        if self.max_chunk_coordinates != RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "artifact chunk maximum is not the fixed semantic maximum",
            ));
        }
        if !self.shape.factor_index_is_canonical(self.factor_index) {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "shape tag and factor index disagree",
            ));
        }
        if self.shape == RelationalCaseChunkShape::ProductRankInterval && self.interval_start != 0 {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "ranked product root must begin at rank zero",
            ));
        }
        if self.interval_start >= self.interval_end_exclusive {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "root interval is empty or reversed",
            ));
        }
        let cardinality = self.interval_end_exclusive - self.interval_start;
        if cardinality <= self.max_chunk_coordinates || self.chunks.len() < 2 {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "partition artifact must split a root larger than one bounded chunk",
            ));
        }
        let expected_chunk_count = canonical_chunk_count(cardinality);
        if u128::try_from(self.chunks.len()).ok() != Some(expected_chunk_count) {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "artifact chunk count is not canonical",
            ));
        }

        let mut expected_start = self.interval_start;
        let mut cell_ids = BTreeSet::new();
        let mut chunk_ids = BTreeSet::new();
        for (index, chunk) in self.chunks.iter().enumerate() {
            let ordinal = u128::try_from(index).map_err(|_| {
                RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
                    "chunk ordinal exceeds u128",
                )
            })?;
            if chunk.ordinal != ordinal
                || chunk.interval_start != expected_start
                || chunk.interval_start >= chunk.interval_end_exclusive
                || chunk.interval_end_exclusive > self.interval_end_exclusive
                || chunk.cardinality() > self.max_chunk_coordinates
                || (chunk.interval_end_exclusive != self.interval_end_exclusive
                    && chunk.cardinality() != self.max_chunk_coordinates)
            {
                return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                    "chunk ordinals or intervals are not the canonical bounded cover",
                ));
            }
            if chunk.cell_id == self.root_cell_id
                || !cell_ids.insert(chunk.cell_id)
                || !chunk_ids.insert(chunk.id)
            {
                return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                    "chunk cells and IDs must be distinct from the parent and one another",
                ));
            }
            let expected_id = derive_chunk_id(
                self.plan_root,
                self.case_image_certificate_id,
                self.injectivity_evidence_id,
                self.root_cell_id,
                self.root_materializer_id,
                self.shape,
                self.factor_index,
                chunk.ordinal,
                chunk.cell_id,
                chunk.interval_start,
                chunk.interval_end_exclusive,
            );
            if chunk.id != expected_id {
                return Err(RelationalCaseChunkPartitionError::ChunkIdentityMismatch {
                    ordinal: chunk.ordinal,
                });
            }
            expected_start = chunk.interval_end_exclusive;
        }
        if expected_start != self.interval_end_exclusive {
            return Err(RelationalCaseChunkPartitionError::InvalidArtifactShape(
                "chunk intervals do not close the root interval",
            ));
        }
        if derive_artifact_id(self) != self.id {
            return Err(RelationalCaseChunkPartitionError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// One planned child cell paired with its canonical scheduling descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseChunk {
    descriptor: RelationalCaseChunkDescriptor,
    cell: SupportCell,
}

impl RelationalCaseChunk {
    pub(crate) const fn descriptor(&self) -> &RelationalCaseChunkDescriptor {
        &self.descriptor
    }

    pub(crate) const fn cell(&self) -> &SupportCell {
        &self.cell
    }
}

/// Complete pure-planner output for a proper root partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseChunkPartition {
    artifact: RelationalCaseChunkPartitionArtifact,
    chunks: Box<[RelationalCaseChunk]>,
    certificate: SupportPartitionCertificate,
}

impl RelationalCaseChunkPartition {
    pub(crate) const fn artifact(&self) -> &RelationalCaseChunkPartitionArtifact {
        &self.artifact
    }

    pub(crate) fn chunks(&self) -> &[RelationalCaseChunk] {
        &self.chunks
    }

    pub(crate) const fn certificate(&self) -> &SupportPartitionCertificate {
        &self.certificate
    }
}

/// Exact typed receipt preimage for injectivity restricted from the durable
/// mapped-image root to one partition child. Bindings are ordered exactly like
/// [`RelationalCaseChunkPartition::chunks`]; they are data consumed only by the
/// private support-cell issuance gateway, not evidence by themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseChunkInjectivityBinding {
    ordinal: u128,
    chunk_id: RelationalCaseChunkId,
    child_cell_id: SupportCellId,
    child_materializer_id: SupportMaterializerId,
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl RelationalCaseChunkInjectivityBinding {
    pub(crate) const fn ordinal(self) -> u128 {
        self.ordinal
    }

    pub(crate) const fn chunk_id(self) -> RelationalCaseChunkId {
        self.chunk_id
    }

    pub(crate) const fn child_cell_id(self) -> SupportCellId {
        self.child_cell_id
    }

    pub(crate) const fn child_materializer_id(self) -> SupportMaterializerId {
        self.child_materializer_id
    }

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

/// Opaque structural authority recovered by replay-verifying an artifact
/// against the installed plan and one exact root-injectivity value. The type
/// does not itself prove that the supplied evidence was durable: a journal
/// acceptance boundary must first look up that exact record in its catalog,
/// then retain the resulting consequences atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalCaseChunkPartition {
    partition: RelationalCaseChunkPartition,
    durable_root_injectivity_evidence_id: SupportCellEvidenceId,
    durable_root_injectivity_receipt_id: SupportProofReceiptId,
    child_injectivity_bindings: Box<[RelationalCaseChunkInjectivityBinding]>,
}

impl VerifiedRelationalCaseChunkPartition {
    pub(crate) const fn partition(&self) -> &RelationalCaseChunkPartition {
        &self.partition
    }

    pub(crate) const fn artifact(&self) -> &RelationalCaseChunkPartitionArtifact {
        self.partition.artifact()
    }

    pub(crate) const fn durable_root_injectivity_evidence_id(&self) -> SupportCellEvidenceId {
        self.durable_root_injectivity_evidence_id
    }

    pub(crate) const fn durable_root_injectivity_receipt_id(&self) -> SupportProofReceiptId {
        self.durable_root_injectivity_receipt_id
    }

    pub(crate) fn child_injectivity_bindings(&self) -> &[RelationalCaseChunkInjectivityBinding] {
        &self.child_injectivity_bindings
    }

    pub(crate) fn child_and_injectivity_binding(
        &self,
        child_ordinal: usize,
    ) -> Option<(&RelationalCaseChunk, RelationalCaseChunkInjectivityBinding)> {
        Some((
            self.partition.chunks().get(child_ordinal)?,
            *self.child_injectivity_bindings.get(child_ordinal)?,
        ))
    }
}

/// Narrow, explicit reasons why the planner declined a shape. Unsupported is
/// not evidence and leaves the ordinary concrete fallback authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseChunkUnsupported {
    CaseImageIsNotExactComposedSingleton,
    RootExpressionIsNotOrdinalOrProduct,
    ProductHasNoVaryingOrdinalIntervalFactor,
    ProductHasNonSingletonRemainder,
    ProductRankFactorIsNotZeroBasedOrdinalInterval,
    PartitionArtifactBudgetExceeded {
        required_chunks: u128,
        maximum_chunks: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseChunkPlanningOutcome {
    Partitioned(RelationalCaseChunkPartition),
    /// A recognized exact root already fits one bounded materialization unit.
    /// Schedule the root directly: a self-partition would create a support-DAG
    /// cycle.
    AlreadyBounded {
        root_cell_id: SupportCellId,
        cardinality: u128,
    },
    Unsupported(RelationalCaseChunkUnsupported),
}

/// Derive the canonical bounded partition for one already verified mapped case
/// image. No observer or query expression is evaluated here.
pub(crate) fn plan_relational_bounded_case_chunks(
    plan: &RelationalSupportPlan,
    case_image_proof: &RelationalCaseImageInjectivityProof,
) -> Result<RelationalCaseChunkPlanningOutcome, RelationalCaseChunkPartitionError> {
    if !plan.validate_root() {
        return Err(RelationalCaseChunkPartitionError::InvalidPlanRoot);
    }
    let questions = FrozenClassificationQuestionSet::freeze(plan.question_ids().iter().copied())
        .map_err(|_| {
            RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                "support-plan question set could not be frozen",
            )
        })?;
    if !questions.validate_identity() || questions.question_ids() != plan.question_ids() {
        return Err(RelationalCaseChunkPartitionError::InternalPlannerInvariant(
            "support-plan question set is not canonical",
        ));
    }
    let root = plan
        .cases()
        .cell()
        .ok_or(RelationalCaseChunkPartitionError::MissingCaseRoot)?;
    if plan.root_cell_id() != Some(root.id()) {
        return Err(RelationalCaseChunkPartitionError::RootCellMismatch);
    }

    let proof_artifact = case_image_proof.proof().artifact();
    if proof_artifact.plan_root() != plan.root()
        || proof_artifact.relation_id() != plan.relation_id()
        || proof_artifact.case_cell_id() != root.id()
        || proof_artifact.case_materializer_id() != root.materializer_id()
    {
        return Err(RelationalCaseChunkPartitionError::CaseImageProofScopeMismatch);
    }
    root.validate_evidence(case_image_proof.injectivity())?;

    let source_assignment_image_is_proven = match (
        proof_artifact.source_assignment_image_proof(),
        proof_artifact.source_image_proof_reference(),
    ) {
        (RelationalSourceAssignmentImageProof::DirectEndpointCoordinates, None)
        | (RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate, Some(_)) => true,
        _ => false,
    };
    if proof_artifact.assignment_kind() != RelationalCaseImageAssignmentKind::IndependentProduct
        || !source_assignment_image_is_proven
        || proof_artifact.successor_kind() != RelationalSuccessorRecipeKind::Singleton
        || proof_artifact.preimage_kind()
            != RelationalCaseImagePreimageKind::ComposedSingletonAssignment
    {
        return Ok(RelationalCaseChunkPlanningOutcome::Unsupported(
            RelationalCaseChunkUnsupported::CaseImageIsNotExactComposedSingleton,
        ));
    }

    let exact_image_cardinality = root
        .cardinality_with_injectivity(case_image_proof.injectivity())?
        .exact()
        .ok_or(RelationalCaseChunkPartitionError::CaseImageIsNotExact)?;
    let exact_cardinality_evidence = case_image_proof
        .exact_cardinality()
        .ok_or(RelationalCaseChunkPartitionError::CaseImageIsNotExact)?;
    root.validate_evidence(exact_cardinality_evidence)?;
    if *exact_cardinality_evidence.conclusion() != exact_image_cardinality
        || proof_artifact.exact_case_cardinality() != Some(exact_image_cardinality)
    {
        return Err(RelationalCaseChunkPartitionError::CaseImageCardinalityMismatch);
    }

    let recognized = match recognize_root_shape(root.expression())? {
        Ok(recognized) => recognized,
        Err(reason) => return Ok(RelationalCaseChunkPlanningOutcome::Unsupported(reason)),
    };
    let interval_cardinality = recognized.end_exclusive - recognized.start;
    if interval_cardinality != exact_image_cardinality {
        return Err(RelationalCaseChunkPartitionError::CaseImageCardinalityMismatch);
    }
    if interval_cardinality <= RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 {
        return Ok(RelationalCaseChunkPlanningOutcome::AlreadyBounded {
            root_cell_id: root.id(),
            cardinality: interval_cardinality,
        });
    }

    let required_chunks = canonical_chunk_count(interval_cardinality);
    if required_chunks > MAX_EAGER_CASE_CHUNKS {
        return Ok(RelationalCaseChunkPlanningOutcome::Unsupported(
            RelationalCaseChunkUnsupported::PartitionArtifactBudgetExceeded {
                required_chunks,
                maximum_chunks: MAX_EAGER_CASE_CHUNKS,
            },
        ));
    }
    let mut raw_chunks = Vec::with_capacity(chunk_capacity(interval_cardinality)?);
    let mut interval_start = recognized.start;
    let mut ordinal = 0u128;
    while interval_start < recognized.end_exclusive {
        let width = (recognized.end_exclusive - interval_start)
            .min(RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1);
        let interval_end_exclusive = interval_start.checked_add(width).ok_or(
            RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
                "chunk interval endpoint exceeds u128",
            ),
        )?;
        let expression = child_expression(
            root.expression(),
            recognized.shape,
            recognized.factor_index,
            interval_start,
            interval_end_exclusive,
        )?;
        let cell = SupportCell::new(root.space(), expression, root.materializer_id())?;
        raw_chunks.push((ordinal, interval_start, interval_end_exclusive, cell));
        ordinal = ordinal.checked_add(1).ok_or(
            RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
                "chunk ordinal exceeds u128",
            ),
        )?;
        interval_start = interval_end_exclusive;
    }

    let child_cells = raw_chunks
        .iter()
        .map(|(_, _, _, cell)| cell.clone())
        .collect::<Vec<_>>();
    let certificate = match recognized.shape {
        RelationalCaseChunkShape::BareOrdinalInterval => {
            SupportPartitionCertificate::mapped_injective_ordinal_cover(
                root,
                child_cells,
                case_image_proof.injectivity(),
            )?
        }
        RelationalCaseChunkShape::ProductFactor => {
            let factor_index = recognized.factor_index.ok_or(
                RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                    "product shape lost its factor index",
                ),
            )?;
            SupportPartitionCertificate::mapped_injective_product_factor_cover(
                root,
                child_cells,
                factor_index,
                case_image_proof.injectivity(),
            )?
        }
        RelationalCaseChunkShape::ProductRankInterval => {
            SupportPartitionCertificate::mapped_injective_product_rank_interval_cover(
                root,
                child_cells,
                case_image_proof.injectivity(),
            )?
        }
    };
    certificate.validate()?;
    if certificate.parent_id() != root.id()
        || certificate.kind() != recognized.shape.partition_kind()
        || certificate.cardinality() != SupportCardinality::Exact(exact_image_cardinality)
    {
        return Err(RelationalCaseChunkPartitionError::PartitionCertificateMismatch);
    }

    let factor_index = recognized
        .factor_index
        .map(|factor_index| {
            u32::try_from(factor_index).map_err(|_| {
                RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
                    "product factor index exceeds u32",
                )
            })
        })
        .transpose()?;
    let chunks = raw_chunks
        .into_iter()
        .map(|(ordinal, start, end_exclusive, cell)| {
            let id = derive_chunk_id(
                plan.root(),
                proof_artifact.certificate_id(),
                case_image_proof.injectivity().id(),
                root.id(),
                root.materializer_id(),
                recognized.shape,
                factor_index,
                ordinal,
                cell.id(),
                start,
                end_exclusive,
            );
            RelationalCaseChunk {
                descriptor: RelationalCaseChunkDescriptor {
                    id,
                    ordinal,
                    cell_id: cell.id(),
                    interval_start: start,
                    interval_end_exclusive: end_exclusive,
                },
                cell,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let descriptors = chunks
        .iter()
        .map(|chunk| chunk.descriptor.clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut artifact = RelationalCaseChunkPartitionArtifact {
        schema_version: recognized.shape.schema_version(),
        id: RelationalCaseChunkPartitionArtifactId([0; 32]),
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        admission_id: plan.admission_id(),
        questions,
        case_image_certificate_id: proof_artifact.certificate_id(),
        injectivity_evidence_id: case_image_proof.injectivity().id(),
        root_cell_id: root.id(),
        root_materializer_id: root.materializer_id(),
        shape: recognized.shape,
        factor_index,
        interval_start: recognized.start,
        interval_end_exclusive: recognized.end_exclusive,
        max_chunk_coordinates: RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1,
        chunks: descriptors,
        partition_id: certificate.id(),
    };
    artifact.id = derive_artifact_id(&artifact);
    artifact.validate_identity()?;

    let certificate_child_ids = certificate
        .child_ids()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let planned_child_ids = chunks
        .iter()
        .map(|chunk| chunk.cell.id())
        .collect::<BTreeSet<_>>();
    if certificate_child_ids != planned_child_ids || certificate.id() != artifact.partition_id {
        return Err(RelationalCaseChunkPartitionError::PartitionCertificateMismatch);
    }

    Ok(RelationalCaseChunkPlanningOutcome::Partitioned(
        RelationalCaseChunkPartition {
            artifact,
            chunks,
            certificate,
        },
    ))
}

/// Rebuild a retained partition artifact from the installed support plan and
/// an exact supplied root-injectivity value. This recovers the producer proof
/// and structural partition certificate; byte-level identity alone is never
/// treated as authority. Durable callers must independently establish that
/// `durable_root_injectivity` is the exact record in their retained catalog.
pub(crate) fn reverify_relational_case_chunk_partition_artifact(
    artifact: &RelationalCaseChunkPartitionArtifact,
    plan: &RelationalSupportPlan,
    durable_root_injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
) -> Result<VerifiedRelationalCaseChunkPartition, RelationalCaseChunkPartitionError> {
    artifact.validate_identity()?;
    durable_root_injectivity.validate()?;
    if durable_root_injectivity.id() != artifact.injectivity_evidence_id() {
        return Err(
            RelationalCaseChunkPartitionError::DurableRootInjectivityEvidenceIdentityMismatch,
        );
    }
    if durable_root_injectivity.obligation().cell_id() != artifact.root_cell_id()
        || durable_root_injectivity
            .obligation()
            .claim()
            .materializer_id()
            != artifact.root_materializer_id()
    {
        return Err(
            RelationalCaseChunkPartitionError::DurableRootInjectivityEvidenceSubjectMismatch,
        );
    }
    let case_image_proof = prove_relational_case_image_injectivity(plan)?;
    if case_image_proof.injectivity() != durable_root_injectivity {
        return Err(
            RelationalCaseChunkPartitionError::DurableRootInjectivityEvidenceReceiptMismatch,
        );
    }
    let partition = match plan_relational_bounded_case_chunks(plan, &case_image_proof)? {
        RelationalCaseChunkPlanningOutcome::Partitioned(partition)
            if partition.artifact() == artifact =>
        {
            partition
        }
        RelationalCaseChunkPlanningOutcome::Partitioned(_)
        | RelationalCaseChunkPlanningOutcome::AlreadyBounded { .. }
        | RelationalCaseChunkPlanningOutcome::Unsupported(_) => {
            return Err(RelationalCaseChunkPartitionError::ArtifactSemanticMismatch);
        }
    };

    let durable_root_injectivity_receipt_id = durable_root_injectivity.receipt().id();
    let mut child_injectivity_bindings = Vec::with_capacity(partition.chunks().len());
    for chunk in partition.chunks() {
        let claim = InjectiveMappingClaim::new(chunk.cell().materializer_id());
        let obligation = SupportCellObligation::new(chunk.cell(), claim)?;
        let conclusion_digest = claim.conclusion_digest(&CertifiedInjective);
        child_injectivity_bindings.push(RelationalCaseChunkInjectivityBinding {
            ordinal: chunk.descriptor().ordinal(),
            chunk_id: chunk.descriptor().id(),
            child_cell_id: chunk.cell().id(),
            child_materializer_id: chunk.cell().materializer_id(),
            obligation_id: obligation.id(),
            conclusion_digest,
            proof_digest: derive_child_restricted_injectivity_proof_digest(
                partition.artifact(),
                chunk,
                durable_root_injectivity.id(),
                durable_root_injectivity_receipt_id,
                obligation.id(),
                conclusion_digest,
            ),
        });
    }

    Ok(VerifiedRelationalCaseChunkPartition {
        partition,
        durable_root_injectivity_evidence_id: durable_root_injectivity.id(),
        durable_root_injectivity_receipt_id,
        child_injectivity_bindings: child_injectivity_bindings.into_boxed_slice(),
    })
}

/// Derive one canonical nonempty sub-interval cell from an ordered planned
/// chunk. This is the shared construction seam for later homogeneous sweep
/// runs; it preserves mapped-image space, materializer, product remainder and
/// selected factor instead of re-encoding those rules in the sweep driver.
/// `RelationalCaseChunkPartition` is an opaque, planner-validated value, so the
/// per-run path validates only the selected descriptor and bounds in O(1)
/// rather than re-walking the complete parent partition.
pub(crate) fn derive_relational_case_chunk_subinterval_cell(
    partition: &RelationalCaseChunkPartition,
    chunk_ordinal: u128,
    interval_start: u128,
    interval_end_exclusive: u128,
) -> Result<SupportCell, RelationalCaseChunkPartitionError> {
    let chunk_index = usize::try_from(chunk_ordinal).map_err(|_| {
        RelationalCaseChunkPartitionError::ChunkOrdinalOutOfBounds { chunk_ordinal }
    })?;
    let chunk = partition
        .chunks()
        .get(chunk_index)
        .ok_or(RelationalCaseChunkPartitionError::ChunkOrdinalOutOfBounds { chunk_ordinal })?;
    let descriptor = chunk.descriptor();
    if descriptor.ordinal() != chunk_ordinal
        || partition.artifact().chunks().get(chunk_index) != Some(descriptor)
    {
        return Err(RelationalCaseChunkPartitionError::ChunkDescriptorMismatch { chunk_ordinal });
    }
    if interval_start < descriptor.interval_start()
        || interval_end_exclusive > descriptor.interval_end_exclusive()
        || interval_start >= interval_end_exclusive
    {
        return Err(RelationalCaseChunkPartitionError::SubintervalOutsideChunk {
            chunk_ordinal,
            interval_start,
            interval_end_exclusive,
        });
    }
    let factor_index = partition
        .artifact()
        .factor_index()
        .map(|factor_index| {
            usize::try_from(factor_index).map_err(|_| {
                RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
                    "product factor index exceeds usize",
                )
            })
        })
        .transpose()?;
    let expression = child_expression(
        chunk.cell().expression(),
        partition.artifact().shape(),
        factor_index,
        interval_start,
        interval_end_exclusive,
    )?;
    Ok(SupportCell::new(
        chunk.cell().space(),
        expression,
        chunk.cell().materializer_id(),
    )?)
}

/// Decode one canonical chunk coordinate into the exact independent finite
/// `FROM` member ordinal for every finite binding, in binding order.
///
/// V1 one-dimensional artifacts retain their original coordinate convention.
/// V2 ranked products use ordinary row-major mixed radix: the final factor is
/// least significant and therefore varies fastest.
pub(crate) fn decode_relational_case_chunk_finite_ordinals(
    partition: &RelationalCaseChunkPartition,
    chunk_ordinal: u128,
    coordinate: u128,
    finite_factor_count: usize,
) -> Result<Vec<u128>, RelationalCaseChunkPartitionError> {
    let chunk_index = usize::try_from(chunk_ordinal).map_err(|_| {
        RelationalCaseChunkPartitionError::ChunkOrdinalOutOfBounds { chunk_ordinal }
    })?;
    let chunk = partition
        .chunks()
        .get(chunk_index)
        .ok_or(RelationalCaseChunkPartitionError::ChunkOrdinalOutOfBounds { chunk_ordinal })?;
    if coordinate < chunk.descriptor().interval_start()
        || coordinate >= chunk.descriptor().interval_end_exclusive()
    {
        return Err(
            RelationalCaseChunkPartitionError::RankCoordinateOutsideChunk {
                chunk_ordinal,
                coordinate,
            },
        );
    }

    match partition.artifact().shape() {
        RelationalCaseChunkShape::BareOrdinalInterval => {
            if finite_factor_count != 1 {
                return Err(
                    RelationalCaseChunkPartitionError::FiniteFactorArityMismatch {
                        expected: 1,
                        actual: finite_factor_count,
                    },
                );
            }
            Ok(vec![coordinate])
        }
        RelationalCaseChunkShape::ProductFactor => {
            let factor_index = partition
                .artifact()
                .factor_index()
                .and_then(|index| usize::try_from(index).ok())
                .ok_or(RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                    "product chunk lost its factor index",
                ))?;
            let SupportExprKind::Product(factors) = chunk.cell().expression().kind() else {
                return Err(RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                    "product chunk cell lost its product expression",
                ));
            };
            if factors.len() != finite_factor_count {
                return Err(
                    RelationalCaseChunkPartitionError::FiniteFactorArityMismatch {
                        expected: factors.len(),
                        actual: finite_factor_count,
                    },
                );
            }
            let mut ordinals = vec![0u128; finite_factor_count];
            for (index, factor) in factors.iter().enumerate() {
                let SupportExprKind::OrdinalInterval {
                    start,
                    end_exclusive,
                } = factor.kind()
                else {
                    return Err(RelationalCaseChunkPartitionError::NonCanonicalRankFactor {
                        factor_index: index,
                    });
                };
                if *start != 0 || (index != factor_index && *end_exclusive != 1) {
                    return Err(RelationalCaseChunkPartitionError::NonCanonicalRankFactor {
                        factor_index: index,
                    });
                }
            }
            ordinals[factor_index] = coordinate;
            Ok(ordinals)
        }
        RelationalCaseChunkShape::ProductRankInterval => {
            let SupportExprKind::ProductRankInterval { factors, .. } =
                chunk.cell().expression().kind()
            else {
                return Err(RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                    "ranked chunk cell lost its product rank interval",
                ));
            };
            if factors.len() != finite_factor_count {
                return Err(
                    RelationalCaseChunkPartitionError::FiniteFactorArityMismatch {
                        expected: factors.len(),
                        actual: finite_factor_count,
                    },
                );
            }
            let mut remaining = coordinate;
            let mut ordinals = vec![0u128; finite_factor_count];
            for index in (0..factors.len()).rev() {
                let SupportExprKind::OrdinalInterval {
                    start: 0,
                    end_exclusive,
                } = factors[index].kind()
                else {
                    return Err(RelationalCaseChunkPartitionError::NonCanonicalRankFactor {
                        factor_index: index,
                    });
                };
                ordinals[index] = remaining % *end_exclusive;
                remaining /= *end_exclusive;
            }
            if remaining != 0 {
                return Err(
                    RelationalCaseChunkPartitionError::RankCoordinateOutsideProduct { coordinate },
                );
            }
            Ok(ordinals)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecognizedRootShape {
    shape: RelationalCaseChunkShape,
    factor_index: Option<usize>,
    start: u128,
    end_exclusive: u128,
}

fn recognize_root_shape(
    expression: &SupportExpr,
) -> Result<
    Result<RecognizedRootShape, RelationalCaseChunkUnsupported>,
    RelationalCaseChunkPartitionError,
> {
    expression.validate()?;
    match expression.kind() {
        SupportExprKind::OrdinalInterval {
            start,
            end_exclusive,
        } => Ok(Ok(RecognizedRootShape {
            shape: RelationalCaseChunkShape::BareOrdinalInterval,
            factor_index: None,
            start: *start,
            end_exclusive: *end_exclusive,
        })),
        SupportExprKind::Product(factors) => {
            let mut varying_ordinal_factors = Vec::new();
            for (index, factor) in factors.iter().enumerate() {
                if factor.intrinsic_cardinality() == SupportCardinality::Exact(1) {
                    continue;
                }
                match factor.kind() {
                    SupportExprKind::OrdinalInterval {
                        start,
                        end_exclusive,
                    } => {
                        varying_ordinal_factors.push((index, *start, *end_exclusive));
                    }
                    _ => {
                        return Ok(Err(
                            RelationalCaseChunkUnsupported::ProductHasNonSingletonRemainder,
                        ));
                    }
                }
            }
            let Some(&(factor_index, start, end_exclusive)) = varying_ordinal_factors.first()
            else {
                return Ok(Err(
                    RelationalCaseChunkUnsupported::ProductHasNoVaryingOrdinalIntervalFactor,
                ));
            };
            if varying_ordinal_factors.len() > 1 {
                if factors.iter().any(|factor| {
                    !matches!(
                        factor.kind(),
                        SupportExprKind::OrdinalInterval { start: 0, .. }
                    )
                }) {
                    return Ok(Err(
                        RelationalCaseChunkUnsupported::ProductRankFactorIsNotZeroBasedOrdinalInterval,
                    ));
                }
                let cardinality = expression.intrinsic_cardinality().exact().ok_or(
                    RelationalCaseChunkPartitionError::InternalPlannerInvariant(
                        "ranked independent product lost exact cardinality",
                    ),
                )?;
                return Ok(Ok(RecognizedRootShape {
                    shape: RelationalCaseChunkShape::ProductRankInterval,
                    factor_index: None,
                    start: 0,
                    end_exclusive: cardinality,
                }));
            }
            if factors.iter().enumerate().any(|(index, factor)| {
                index != factor_index
                    && factor.intrinsic_cardinality() != SupportCardinality::Exact(1)
            }) {
                return Ok(Err(
                    RelationalCaseChunkUnsupported::ProductHasNonSingletonRemainder,
                ));
            }
            Ok(Ok(RecognizedRootShape {
                shape: RelationalCaseChunkShape::ProductFactor,
                factor_index: Some(factor_index),
                start,
                end_exclusive,
            }))
        }
        _ => Ok(Err(
            RelationalCaseChunkUnsupported::RootExpressionIsNotOrdinalOrProduct,
        )),
    }
}

fn child_expression(
    parent: &SupportExpr,
    shape: RelationalCaseChunkShape,
    factor_index: Option<usize>,
    interval_start: u128,
    interval_end_exclusive: u128,
) -> Result<SupportExpr, RelationalCaseChunkPartitionError> {
    match (shape, parent.kind(), factor_index) {
        (
            RelationalCaseChunkShape::BareOrdinalInterval,
            SupportExprKind::OrdinalInterval { .. },
            None,
        ) => Ok(SupportExpr::ordinal_interval(
            interval_start,
            interval_end_exclusive,
        )?),
        (
            RelationalCaseChunkShape::ProductFactor,
            SupportExprKind::Product(parent_factors),
            Some(factor_index),
        ) => {
            let mut child_factors = parent_factors.to_vec();
            let factor_count = child_factors.len();
            let selected = child_factors.get_mut(factor_index).ok_or(
                RelationalCaseChunkPartitionError::ProductFactorOutOfBounds {
                    factor_index,
                    factor_count,
                },
            )?;
            *selected = SupportExpr::ordinal_interval(interval_start, interval_end_exclusive)?;
            Ok(SupportExpr::product(child_factors)?)
        }
        (
            RelationalCaseChunkShape::ProductRankInterval,
            SupportExprKind::Product(parent_factors),
            None,
        ) => Ok(SupportExpr::product_rank_interval(
            parent_factors.to_vec(),
            interval_start,
            interval_end_exclusive,
        )?),
        (
            RelationalCaseChunkShape::ProductRankInterval,
            SupportExprKind::ProductRankInterval { factors, .. },
            None,
        ) => Ok(SupportExpr::product_rank_interval(
            factors.to_vec(),
            interval_start,
            interval_end_exclusive,
        )?),
        _ => Err(RelationalCaseChunkPartitionError::InternalPlannerInvariant(
            "recognized root shape changed while constructing child cells",
        )),
    }
}

const fn canonical_chunk_count(cardinality: u128) -> u128 {
    (cardinality - 1) / RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1 + 1
}

fn chunk_capacity(cardinality: u128) -> Result<usize, RelationalCaseChunkPartitionError> {
    usize::try_from(canonical_chunk_count(cardinality)).map_err(|_| {
        RelationalCaseChunkPartitionError::ArtifactCapacityExceeded(
            "canonical chunk count exceeds addressable memory",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_chunk_id(
    plan_root: RelationalSupportPlanRoot,
    case_image_certificate_id: [u8; 32],
    injectivity_evidence_id: SupportCellEvidenceId,
    root_cell_id: SupportCellId,
    root_materializer_id: SupportMaterializerId,
    shape: RelationalCaseChunkShape,
    factor_index: Option<u32>,
    ordinal: u128,
    cell_id: SupportCellId,
    interval_start: u128,
    interval_end_exclusive: u128,
) -> RelationalCaseChunkId {
    let schema_version = shape.schema_version();
    let mut hasher = CanonicalChunkHasher::new(match schema_version {
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V1 => CASE_CHUNK_ID_V1,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V2 => CASE_CHUNK_ID_V2,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION => CASE_CHUNK_ID_V3,
        _ => unreachable!("case-chunk shape has a supported schema version"),
    });
    hasher.u32(schema_version);
    hasher.u128(RELATIONAL_CASE_CHUNK_MAX_COORDINATES_V1);
    hasher.digest(plan_root.bytes());
    hasher.digest(case_image_certificate_id);
    hasher.digest(injectivity_evidence_id.bytes());
    hasher.digest(root_cell_id.bytes());
    hasher.digest(root_materializer_id.bytes());
    hasher.u8(shape.tag());
    hasher.optional_u32(factor_index);
    hasher.u128(ordinal);
    hasher.digest(cell_id.bytes());
    hasher.u128(interval_start);
    hasher.u128(interval_end_exclusive);
    RelationalCaseChunkId(hasher.finish())
}

fn derive_child_restricted_injectivity_proof_digest(
    artifact: &RelationalCaseChunkPartitionArtifact,
    chunk: &RelationalCaseChunk,
    durable_root_injectivity_evidence_id: SupportCellEvidenceId,
    durable_root_injectivity_receipt_id: SupportProofReceiptId,
    child_obligation_id: SupportProofObligationId,
    child_conclusion_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = CanonicalChunkHasher::new(match artifact.schema_version() {
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V1 => CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V1,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V2 => CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V2,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION => CASE_CHUNK_RESTRICTED_INJECTIVITY_PROOF_V3,
        _ => unreachable!("validated case-chunk artifact version"),
    });
    hasher.u32(artifact.schema_version());
    hasher.digest(artifact.id().bytes());
    hasher.digest(artifact.partition_id().bytes());
    hasher.digest(artifact.root_cell_id().bytes());
    hasher.digest(artifact.root_materializer_id().bytes());
    hasher.digest(durable_root_injectivity_evidence_id.bytes());
    hasher.digest(durable_root_injectivity_receipt_id.bytes());
    hasher.digest(chunk.descriptor().id().bytes());
    hasher.u128(chunk.descriptor().ordinal());
    hasher.digest(chunk.cell().id().bytes());
    hasher.digest(chunk.cell().materializer_id().bytes());
    hasher.u128(chunk.descriptor().interval_start());
    hasher.u128(chunk.descriptor().interval_end_exclusive());
    hasher.digest(child_obligation_id.bytes());
    hasher.digest(child_conclusion_digest);
    hasher.finish()
}

fn derive_artifact_id(
    artifact: &RelationalCaseChunkPartitionArtifact,
) -> RelationalCaseChunkPartitionArtifactId {
    let mut hasher = CanonicalChunkHasher::new(match artifact.schema_version {
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V1 => CASE_CHUNK_PARTITION_ARTIFACT_ID_V1,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION_V2 => CASE_CHUNK_PARTITION_ARTIFACT_ID_V2,
        RELATIONAL_CASE_CHUNK_PARTITION_VERSION => CASE_CHUNK_PARTITION_ARTIFACT_ID_V3,
        _ => unreachable!("validated case-chunk artifact version"),
    });
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.question_set_root().bytes());
    hasher.digest(artifact.case_image_certificate_id);
    hasher.digest(artifact.injectivity_evidence_id.bytes());
    hasher.digest(artifact.root_cell_id.bytes());
    hasher.digest(artifact.root_materializer_id.bytes());
    hasher.u8(artifact.shape.tag());
    hasher.optional_u32(artifact.factor_index);
    hasher.u128(artifact.interval_start);
    hasher.u128(artifact.interval_end_exclusive);
    hasher.u128(artifact.max_chunk_coordinates);
    hasher.u128(artifact.chunks.len() as u128);
    for chunk in artifact.chunks.iter() {
        hasher.digest(chunk.id.bytes());
        hasher.u128(chunk.ordinal);
        hasher.digest(chunk.cell_id.bytes());
        hasher.u128(chunk.interval_start);
        hasher.u128(chunk.interval_end_exclusive);
    }
    hasher.digest(artifact.partition_id.bytes());
    RelationalCaseChunkPartitionArtifactId(hasher.finish())
}

struct CanonicalChunkHasher(Sha256);

impl CanonicalChunkHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn optional_u32(&mut self, value: Option<u32>) {
        match value {
            None => self.u8(0x00),
            Some(value) => {
                self.u8(0x01);
                self.u32(value);
            }
        }
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseChunkPartitionError {
    UnsupportedArtifactVersion(u32),
    InvalidArtifactShape(&'static str),
    ArtifactIdentityMismatch,
    ArtifactSemanticMismatch,
    DurableRootInjectivityEvidenceIdentityMismatch,
    DurableRootInjectivityEvidenceSubjectMismatch,
    DurableRootInjectivityEvidenceReceiptMismatch,
    ChunkIdentityMismatch {
        ordinal: u128,
    },
    ChunkOrdinalOutOfBounds {
        chunk_ordinal: u128,
    },
    ChunkDescriptorMismatch {
        chunk_ordinal: u128,
    },
    SubintervalOutsideChunk {
        chunk_ordinal: u128,
        interval_start: u128,
        interval_end_exclusive: u128,
    },
    RankCoordinateOutsideChunk {
        chunk_ordinal: u128,
        coordinate: u128,
    },
    RankCoordinateOutsideProduct {
        coordinate: u128,
    },
    FiniteFactorArityMismatch {
        expected: usize,
        actual: usize,
    },
    NonCanonicalRankFactor {
        factor_index: usize,
    },
    InvalidPlanRoot,
    MissingCaseRoot,
    RootCellMismatch,
    CaseImageProofScopeMismatch,
    CaseImageIsNotExact,
    CaseImageCardinalityMismatch,
    PartitionCertificateMismatch,
    ProductFactorOutOfBounds {
        factor_index: usize,
        factor_count: usize,
    },
    ArtifactCapacityExceeded(&'static str),
    InternalPlannerInvariant(&'static str),
    CaseImageProof(RelationalCaseImageInjectivityProofError),
    SupportCell(SupportCellError),
}

impl From<RelationalCaseImageInjectivityProofError> for RelationalCaseChunkPartitionError {
    fn from(error: RelationalCaseImageInjectivityProofError) -> Self {
        Self::CaseImageProof(error)
    }
}

impl From<SupportCellError> for RelationalCaseChunkPartitionError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl fmt::Display for RelationalCaseChunkPartitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion(version) => {
                write!(
                    formatter,
                    "unsupported case-chunk partition version {version}"
                )
            }
            Self::InvalidArtifactShape(message) => {
                write!(
                    formatter,
                    "invalid case-chunk partition artifact: {message}"
                )
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("case-chunk partition artifact identity is not canonical")
            }
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "case-chunk partition artifact does not match the installed support plan",
            ),
            Self::DurableRootInjectivityEvidenceIdentityMismatch => formatter.write_str(
                "case-chunk partition names another durable root injectivity evidence record",
            ),
            Self::DurableRootInjectivityEvidenceSubjectMismatch => formatter.write_str(
                "durable root injectivity evidence names another root cell or materializer",
            ),
            Self::DurableRootInjectivityEvidenceReceiptMismatch => formatter.write_str(
                "durable root injectivity receipt differs from the producer-reverified proof",
            ),
            Self::ChunkIdentityMismatch { ordinal } => write!(
                formatter,
                "case-chunk identity is not canonical at ordinal {ordinal}"
            ),
            Self::ChunkOrdinalOutOfBounds { chunk_ordinal } => write!(
                formatter,
                "case-chunk ordinal {chunk_ordinal} is outside the canonical partition"
            ),
            Self::ChunkDescriptorMismatch { chunk_ordinal } => write!(
                formatter,
                "case-chunk ordinal {chunk_ordinal} does not match the retained artifact"
            ),
            Self::SubintervalOutsideChunk {
                chunk_ordinal,
                interval_start,
                interval_end_exclusive,
            } => write!(
                formatter,
                "sub-interval {interval_start}..{interval_end_exclusive} is outside case chunk {chunk_ordinal}"
            ),
            Self::RankCoordinateOutsideChunk {
                chunk_ordinal,
                coordinate,
            } => write!(
                formatter,
                "coordinate {coordinate} is outside case chunk {chunk_ordinal}"
            ),
            Self::RankCoordinateOutsideProduct { coordinate } => write!(
                formatter,
                "mixed-radix coordinate {coordinate} is outside its product"
            ),
            Self::FiniteFactorArityMismatch { expected, actual } => write!(
                formatter,
                "case-chunk rank basis has {expected} finite factors, but the query has {actual}"
            ),
            Self::NonCanonicalRankFactor { factor_index } => write!(
                formatter,
                "case-chunk rank factor {factor_index} is not a zero-based ordinal interval"
            ),
            Self::InvalidPlanRoot => {
                formatter.write_str("case-chunk support-plan root is not canonical")
            }
            Self::MissingCaseRoot => {
                formatter.write_str("case-chunk planner requires a positive case root")
            }
            Self::RootCellMismatch => {
                formatter.write_str("case-chunk case cell is not the support-plan root")
            }
            Self::CaseImageProofScopeMismatch => formatter
                .write_str("case-image injectivity proof is not bound to this support-plan root"),
            Self::CaseImageIsNotExact => {
                formatter.write_str("case-chunk planner requires exact injective case support")
            }
            Self::CaseImageCardinalityMismatch => formatter.write_str(
                "case-image proof cardinality disagrees with the recognized chunk interval",
            ),
            Self::PartitionCertificateMismatch => formatter.write_str(
                "mapped injective partition certificate disagrees with the planned chunks",
            ),
            Self::ProductFactorOutOfBounds {
                factor_index,
                factor_count,
            } => write!(
                formatter,
                "case-chunk factor index {factor_index} is outside {factor_count} product factors"
            ),
            Self::ArtifactCapacityExceeded(message) => {
                write!(
                    formatter,
                    "case-chunk artifact capacity exceeded: {message}"
                )
            }
            Self::InternalPlannerInvariant(message) => {
                write!(formatter, "case-chunk planner invariant failed: {message}")
            }
            Self::CaseImageProof(error) => {
                write!(
                    formatter,
                    "case-image proof could not be reverified: {error}"
                )
            }
            Self::SupportCell(error) => {
                write!(
                    formatter,
                    "case-chunk support partition is invalid: {error}"
                )
            }
        }
    }
}

impl Error for RelationalCaseChunkPartitionError {}
