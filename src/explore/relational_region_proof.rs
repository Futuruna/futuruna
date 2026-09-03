//! Exact one-axis region proofs for the relational Explore IR.
//!
//! This is a proof producer, not a candidate generator.  It consumes the
//! immutable [`CheckedExploreQueryView`], its request-bound classification
//! capsule, the support plan minted from that view, and the solver-neutral
//! axis inventory.  The first accepted fragment is intentionally narrow:
//!
//! - one independent finite `Int` binding is the semantic `Before` value;
//! - `Context` is the singleton unit value and there are no auxiliary binds;
//! - the capsule's `Successor` lane is a total singleton quasi-affine
//!   expression of Before;
//! - there are no admission predicates; and
//! - the capsule's already-normalized FIND lane is a Boolean formula of exact
//!   one-axis quasi-affine atoms, possibly through acyclic pure calls.
//!
//! When those conditions hold, the producer proves the case-image cardinality,
//! uniform admission, and uniform FIND classification together.  It emits
//! typed support evidence only after the complete normalized proof has been
//! checked.  Everything else is an explicit concrete-fallback residual.  In
//! particular, seeing every proposed boundary is never complement closure.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
use super::relational_bounded_chunk_partition::{
    RelationalCaseChunkId, RelationalCaseChunkPartitionArtifactId,
    VerifiedRelationalCaseChunkPartition,
};
use super::relational_classification_capsule::{
    ClassificationBinaryOp, ClassificationCallableDefinition, ClassificationCallableId,
    ClassificationCapsuleId, ClassificationConstant, ClassificationInputSlot, ClassificationNodeId,
    ClassificationNodeKey, ClassificationNodeKind, ClassificationProvenanceRoot,
    ClassificationSemanticLane, ClassificationSpecializationRoot, ClassificationTypeId,
    ClassificationUnaryOp, FrozenClassificationProgram, FrozenClassificationQuestionSet,
    RelationalClassificationCapsule,
};
use super::relational_ir::{
    ExploreFindIr, ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr, ExploreSuccessorKindIr,
};
use super::relational_proof_strategy::{
    RelationalIntegerAxis, RelationalProofStrategyError, RelationalProofStrategyInventory,
};
use super::relational_support_planner::{
    RelationalBindingStageId, RelationalDimensionId, RelationalObligationActivation,
    RelationalRootObligationPlan, RelationalStagedObligationDescriptor,
    RelationalSuccessorRecipeKind, RelationalSupportPlan, RelationalSupportPlanRoot,
    RelationalSupportPopulationRecipe,
};
use super::support_cell::{
    AdmissionClassificationClaim, ExactCardinalityClaim, SelectionClassificationClaim, SupportCell,
    SupportCellClaim, SupportCellError, SupportCellId, SupportCellObligation,
    SupportMaterializerId, SupportProofObligationId,
};
use super::support_evidence::{SupportEvidenceRecord, SupportObligationRecord};
use super::support_journal::SupportJournalEvent;
use super::ExploreExactDomain;
use crate::{CheckedExploreQueryView, OwnedCheckedExploreQuery};

pub(crate) const RELATIONAL_REGION_PROOF_VERSION: u32 = 3;

const CERTIFICATE_ID_V3: &[u8] = b"futuruna.explore.relational-region.certificate.v3";
const FORMULA_DIGEST_V3: &[u8] = b"futuruna.explore.relational-region.formula.v3";
const PROOF_DIGEST_V3: &[u8] = b"futuruna.explore.relational-region.proof.v3";
const REPLAY_AUTHORITY_ID_V1: &[u8] = b"futuruna.explore.relational-region.replay-authority.v1";
const STARTER_REGION_ID_V1: &[u8] = b"futuruna.explore.relational-region.correlated-starter.v1";
const MAX_GRAPH_NORMALIZATION_STEPS: usize = 100_000;
const MAX_GRAPH_NORMALIZATION_DEPTH: usize = 1_024;

/// Only zero-selected conclusions may bypass concrete case evaluation in V1.
/// `Rejected` is represented so admission normalization can be added without
/// changing the journal event shape; the current scalar producer proves only
/// `AdmittedNotSelected` and otherwise falls back.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalCertifiedRegionConclusion {
    Rejected,
    AdmittedNotSelected,
}

impl RelationalCertifiedRegionConclusion {
    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::Rejected => 0x01,
            Self::AdmittedNotSelected => 0x02,
        }
    }

    pub(crate) const fn from_canonical_tag(tag: u8) -> Option<Self> {
        match tag {
            0x01 => Some(Self::Rejected),
            0x02 => Some(Self::AdmittedNotSelected),
            _ => None,
        }
    }

    pub(crate) const fn admission(self) -> AdmissionDecision {
        match self {
            Self::Rejected => AdmissionDecision::Rejected,
            Self::AdmittedNotSelected => AdmissionDecision::Admitted,
        }
    }

    pub(crate) const fn selection(self) -> Option<SelectionDecision> {
        match self {
            Self::Rejected => None,
            Self::AdmittedNotSelected => Some(SelectionDecision::NotSelected),
        }
    }
}

/// Exact structural subject of one region theorem. A canonical child is
/// always named through the already accepted mapped-image partition; the
/// proof cannot mint an independent cell or an extensional case root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalRegionProofSubject {
    Root,
    CanonicalChunk {
        partition_artifact_id: RelationalCaseChunkPartitionArtifactId,
        chunk_id: RelationalCaseChunkId,
        chunk_ordinal: u128,
        chunk_cell_id: SupportCellId,
        chunk_materializer_id: SupportMaterializerId,
    },
}

impl RelationalRegionProofSubject {
    pub(crate) const fn canonical_chunk_ordinal(self) -> Option<u128> {
        match self {
            Self::Root => None,
            Self::CanonicalChunk { chunk_ordinal, .. } => Some(chunk_ordinal),
        }
    }
}

/// Identity of the correlated starter subpopulation certified by a region.
///
/// This is not a Cartesian box over starter fields. Its preimage is the exact
/// finite source-assignment coordinate slice, and its image is the checked
/// `Source = (Context, Before)` construction retained by the support plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalStarterRegionId([u8; 32]);

impl RelationalStarterRegionId {
    pub(super) const fn from_canonical_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// The exact typed conclusion authorized by one proof-receipt binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalRegionEvidenceRole {
    Cardinality,
    Admission,
    Selection,
}

impl RelationalRegionEvidenceRole {
    const fn tag(self) -> u8 {
        match self {
            Self::Cardinality => 0x01,
            Self::Admission => 0x02,
            Self::Selection => 0x03,
        }
    }
}

/// Opaque binding consumed by the narrow issuance gateway in `support_cell`.
/// A caller cannot manufacture the enclosing verified proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionEvidenceBinding {
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl RelationalRegionEvidenceBinding {
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

/// Canonical bounded proof artifact.  It is evidence to replay, not authority
/// by itself: a decoder may restore these fields, but support receipts may be
/// issued only after [`reverify_relational_region_proof_artifact`] reproduces
/// the artifact from a producer-bound checked query, support plan, and exact
/// classification capsule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionProofArtifact {
    schema_version: u32,
    certificate_id: [u8; 32],
    replay_authority_id: [u8; 32],
    classification_capsule_id: ClassificationCapsuleId,
    successor_root_id: ClassificationNodeId,
    find_root_id: ClassificationNodeId,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    plan_root: RelationalSupportPlanRoot,
    root_cell_id: SupportCellId,
    subject: RelationalRegionProofSubject,
    conclusion: RelationalCertifiedRegionConclusion,
    starter_region_id: RelationalStarterRegionId,
    source_assignment_cell_id: SupportCellId,
    source_row_cell_id: SupportCellId,
    successor_coordinate_cell_id: SupportCellId,
    axis_stage_id: RelationalBindingStageId,
    axis_dimension_id: RelationalDimensionId,
    axis_cell_id: SupportCellId,
    value_start: i64,
    value_end_exclusive: i64,
    coordinate_start: u128,
    coordinate_end_exclusive: u128,
    case_cardinality: u128,
    selected_formula_digest: [u8; 32],
}

impl RelationalRegionProofArtifact {
    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn certificate_id(&self) -> [u8; 32] {
        self.certificate_id
    }

    pub(crate) const fn replay_authority_id(&self) -> [u8; 32] {
        self.replay_authority_id
    }

    pub(crate) const fn classification_capsule_id(&self) -> ClassificationCapsuleId {
        self.classification_capsule_id
    }

    pub(crate) const fn successor_root_id(&self) -> ClassificationNodeId {
        self.successor_root_id
    }

    pub(crate) const fn find_root_id(&self) -> ClassificationNodeId {
        self.find_root_id
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn question_id(&self) -> QuestionId {
        self.question_id
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn root_cell_id(&self) -> SupportCellId {
        self.root_cell_id
    }

    pub(crate) const fn subject(&self) -> RelationalRegionProofSubject {
        self.subject
    }

    pub(crate) const fn conclusion(&self) -> RelationalCertifiedRegionConclusion {
        self.conclusion
    }

    pub(crate) const fn starter_region_id(&self) -> RelationalStarterRegionId {
        self.starter_region_id
    }

    pub(crate) const fn source_assignment_cell_id(&self) -> SupportCellId {
        self.source_assignment_cell_id
    }

    pub(crate) const fn source_row_cell_id(&self) -> SupportCellId {
        self.source_row_cell_id
    }

    pub(crate) const fn successor_coordinate_cell_id(&self) -> SupportCellId {
        self.successor_coordinate_cell_id
    }

    pub(crate) const fn axis_stage_id(&self) -> RelationalBindingStageId {
        self.axis_stage_id
    }

    pub(crate) const fn axis_dimension_id(&self) -> RelationalDimensionId {
        self.axis_dimension_id
    }

    pub(crate) const fn axis_cell_id(&self) -> SupportCellId {
        self.axis_cell_id
    }

    pub(crate) const fn value_start(&self) -> i64 {
        self.value_start
    }

    pub(crate) const fn value_end_exclusive(&self) -> i64 {
        self.value_end_exclusive
    }

    pub(crate) const fn coordinate_start(&self) -> u128 {
        self.coordinate_start
    }

    pub(crate) const fn coordinate_end_exclusive(&self) -> u128 {
        self.coordinate_end_exclusive
    }

    pub(crate) const fn case_cardinality(&self) -> u128 {
        self.case_cardinality
    }

    pub(crate) const fn selected_formula_digest(&self) -> [u8; 32] {
        self.selected_formula_digest
    }

    /// Decoder seam.  Identity validation is structural only; the returned
    /// artifact remains untrusted until producer-bound reverification.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_canonical_parts(
        schema_version: u32,
        certificate_id: [u8; 32],
        replay_authority_id: [u8; 32],
        classification_capsule_id: ClassificationCapsuleId,
        successor_root_id: ClassificationNodeId,
        find_root_id: ClassificationNodeId,
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        plan_root: RelationalSupportPlanRoot,
        root_cell_id: SupportCellId,
        subject: RelationalRegionProofSubject,
        conclusion: RelationalCertifiedRegionConclusion,
        starter_region_id: RelationalStarterRegionId,
        source_assignment_cell_id: SupportCellId,
        source_row_cell_id: SupportCellId,
        successor_coordinate_cell_id: SupportCellId,
        axis_stage_id: RelationalBindingStageId,
        axis_dimension_id: RelationalDimensionId,
        axis_cell_id: SupportCellId,
        value_start: i64,
        value_end_exclusive: i64,
        coordinate_start: u128,
        coordinate_end_exclusive: u128,
        case_cardinality: u128,
        selected_formula_digest: [u8; 32],
    ) -> Result<Self, RelationalRegionProofError> {
        let artifact = Self {
            schema_version,
            certificate_id,
            replay_authority_id,
            classification_capsule_id,
            successor_root_id,
            find_root_id,
            relation_id,
            admission_id,
            question_id,
            plan_root,
            root_cell_id,
            subject,
            conclusion,
            starter_region_id,
            source_assignment_cell_id,
            source_row_cell_id,
            successor_coordinate_cell_id,
            axis_stage_id,
            axis_dimension_id,
            axis_cell_id,
            value_start,
            value_end_exclusive,
            coordinate_start,
            coordinate_end_exclusive,
            case_cardinality,
            selected_formula_digest,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    fn validate_identity(&self) -> Result<(), RelationalRegionProofError> {
        if self.schema_version != RELATIONAL_REGION_PROOF_VERSION {
            return Err(RelationalRegionProofError::UnsupportedArtifactVersion(
                self.schema_version,
            ));
        }
        if self.value_start >= self.value_end_exclusive
            || self.coordinate_start >= self.coordinate_end_exclusive
            || self.case_cardinality != self.coordinate_end_exclusive - self.coordinate_start
            || u128::try_from(i128::from(self.value_end_exclusive) - i128::from(self.value_start))
                .ok()
                != Some(self.case_cardinality)
        {
            return Err(RelationalRegionProofError::InvalidArtifactShape);
        }
        match self.subject {
            RelationalRegionProofSubject::Root => {
                if self.conclusion != RelationalCertifiedRegionConclusion::AdmittedNotSelected {
                    return Err(RelationalRegionProofError::InvalidArtifactShape);
                }
            }
            RelationalRegionProofSubject::CanonicalChunk { .. } => {}
        }
        if derive_starter_region_id(self) != self.starter_region_id {
            return Err(RelationalRegionProofError::StarterRegionIdentityMismatch);
        }
        let derived = derive_certificate_id(self);
        if derived != self.certificate_id {
            return Err(RelationalRegionProofError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// Authority token obtained only by replay-verifying the canonical artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalRegionProof {
    artifact: RelationalRegionProofArtifact,
    bindings: [RelationalRegionEvidenceBinding; 3],
}

impl VerifiedRelationalRegionProof {
    pub(crate) const fn artifact(&self) -> &RelationalRegionProofArtifact {
        &self.artifact
    }

    pub(crate) const fn certificate_id(&self) -> [u8; 32] {
        self.artifact.certificate_id
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.artifact.plan_root
    }

    pub(crate) const fn root_cell_id(&self) -> SupportCellId {
        self.artifact.root_cell_id
    }

    pub(crate) const fn axis_cell_id(&self) -> SupportCellId {
        self.artifact.axis_cell_id
    }

    pub(crate) const fn value_start(&self) -> i64 {
        self.artifact.value_start
    }

    pub(crate) const fn value_end_exclusive(&self) -> i64 {
        self.artifact.value_end_exclusive
    }

    pub(crate) const fn case_cardinality(&self) -> u128 {
        self.artifact.case_cardinality
    }

    pub(crate) const fn classification_capsule_id(&self) -> ClassificationCapsuleId {
        self.artifact.classification_capsule_id
    }

    pub(crate) const fn successor_root_id(&self) -> ClassificationNodeId {
        self.artifact.successor_root_id
    }

    pub(crate) const fn find_root_id(&self) -> ClassificationNodeId {
        self.artifact.find_root_id
    }

    pub(crate) fn evidence_binding(
        &self,
        role: RelationalRegionEvidenceRole,
    ) -> RelationalRegionEvidenceBinding {
        self.bindings[usize::from(role.tag() - 1)]
    }
}

/// Atomic semantic event batch for a proof-closed, exact-empty selection.
///
/// Events must be appended in this order through [`super::RelationalJournal`].
/// The admitted evidence activates the staged selection obligation before the
/// selection evidence arrives.  Applying only the support sub-journal would
/// therefore (correctly) reject the third event as undeclared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionSupportClosure {
    proof: VerifiedRelationalRegionProof,
    events: Box<[SupportJournalEvent]>,
}

impl RelationalRegionSupportClosure {
    pub(crate) const fn proof(&self) -> &VerifiedRelationalRegionProof {
        &self.proof
    }

    pub(crate) fn events(&self) -> &[SupportJournalEvent] {
        &self.events
    }

    pub(crate) const fn selected_cardinality(&self) -> u128 {
        0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRegionExpressionLayer {
    Successor,
    Selection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRegionExpressionResidualReason {
    StructuredStateProjectionRequired,
    UnboundRelationalValue,
    InvalidClassificationGraph,
    InvalidCallableFrame,
    RecursiveCallable,
    UnsupportedScalarType,
    UnsupportedBooleanOperator,
    UnsupportedIntegerOperator,
    ConditionalTruthVariesOverAxis,
    NormalizationCapacityExceeded,
    NonlinearIntegerExpression,
    NestedQuantizedExpression,
    NonpositiveDivisor,
    QuantizedNumeratorMayBeNegative,
    RuntimeIntegerOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionExpressionResidual {
    layer: RelationalRegionExpressionLayer,
    node_path: Box<[ClassificationNodeId]>,
    reason: RelationalRegionExpressionResidualReason,
}

impl RelationalRegionExpressionResidual {
    pub(crate) const fn layer(&self) -> RelationalRegionExpressionLayer {
        self.layer
    }

    pub(crate) fn node_path(&self) -> &[ClassificationNodeId] {
        &self.node_path
    }

    pub(crate) const fn reason(&self) -> RelationalRegionExpressionResidualReason {
        self.reason
    }
}

/// Typed reason why exact symbolic closure was declined.  Every variant means
/// “retain concrete materialization”; none is evidence about unvisited cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRegionProofResidual {
    IntegerAxisCount { found: usize },
    ClassificationLaneMissing { lane: ClassificationSemanticLane },
    ClassificationLaneResidual { lane: ClassificationSemanticLane },
    BeforeIsNotIndependentIntegerAxis,
    SourceHasAuxiliaryOrNonUnitContext,
    CaseImageCardinalityLiftUnavailable,
    FiniteSuccessorNeedsFiberProof,
    AdmissionFormulaNormalizationRequired { predicates: usize },
    ProofArithmeticCapacityExceeded,
    Expression(RelationalRegionExpressionResidual),
    SelectionUniformlySelected,
    SelectionTruthVariesOverAxis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRegionProofOutcome {
    ExactEmpty(RelationalRegionSupportClosure),
    ConcreteFallback {
        residual: RelationalRegionProofResidual,
    },
}

impl RelationalRegionProofOutcome {
    pub(crate) const fn exact_empty(&self) -> Option<&RelationalRegionSupportClosure> {
        match self {
            Self::ExactEmpty(closure) => Some(closure),
            Self::ConcreteFallback { .. } => None,
        }
    }

    pub(crate) const fn residual(&self) -> Option<&RelationalRegionProofResidual> {
        match self {
            Self::ExactEmpty(_) => None,
            Self::ConcreteFallback { residual } => Some(residual),
        }
    }
}

fn fallback(residual: RelationalRegionProofResidual) -> RelationalRegionProofOutcome {
    RelationalRegionProofOutcome::ConcreteFallback { residual }
}

#[derive(Clone, Debug)]
pub(crate) struct RelationalRegionReplayAuthority {
    id: [u8; 32],
    question_id: QuestionId,
    checked: Arc<OwnedCheckedExploreQuery>,
    support_plan: RelationalSupportPlan,
    capsule: Arc<RelationalClassificationCapsule>,
}

impl RelationalRegionReplayAuthority {
    pub(crate) fn new(
        checked: Arc<OwnedCheckedExploreQuery>,
        support_plan: RelationalSupportPlan,
        capsule: Arc<RelationalClassificationCapsule>,
    ) -> Result<Self, RelationalRegionProofError> {
        let checked_view = checked.view();
        let question_id =
            validate_relational_region_scope(&checked_view, &support_plan, capsule.as_ref())?;
        let mut hasher = CanonicalProofHasher::new(REPLAY_AUTHORITY_ID_V1);
        hasher.u32(RELATIONAL_REGION_PROOF_VERSION);
        hasher.digest(capsule.id().bytes());
        hasher.digest(support_plan.root().bytes());
        hasher.digest(checked_view.relation_id().bytes());
        hasher.digest(checked_view.admission_id().bytes());
        hasher.digest(question_id.bytes());
        let id = hasher.finish();
        Ok(Self {
            id,
            question_id,
            checked,
            support_plan,
            capsule,
        })
    }

    pub(crate) const fn id(&self) -> [u8; 32] {
        self.id
    }

    pub(crate) fn classification_capsule_id(&self) -> ClassificationCapsuleId {
        self.capsule.id()
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan.root()
    }

    pub(crate) fn prove_canonical_child(
        &self,
        partition: &VerifiedRelationalCaseChunkPartition,
        chunk_ordinal: usize,
    ) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
        let target = self.target(partition, chunk_ordinal)?;
        prove_relational_region(
            &self.checked.view(),
            &self.support_plan,
            self.capsule.as_ref(),
            Some(&target),
            self.id,
        )
    }

    pub(crate) fn reverify_canonical_child(
        &self,
        artifact: &RelationalRegionProofArtifact,
        partition: &VerifiedRelationalCaseChunkPartition,
    ) -> Result<VerifiedRelationalRegionProof, RelationalRegionProofError> {
        artifact.validate_identity()?;
        if artifact.replay_authority_id() != self.id
            || artifact.classification_capsule_id() != self.capsule.id()
            || artifact.plan_root() != self.support_plan.root()
        {
            return Err(RelationalRegionProofError::ReplayAuthorityMismatch);
        }
        let RelationalRegionProofSubject::CanonicalChunk { chunk_ordinal, .. } = artifact.subject()
        else {
            return Err(RelationalRegionProofError::ArtifactSemanticMismatch);
        };
        let chunk_ordinal = usize::try_from(chunk_ordinal)
            .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?;
        match self.prove_canonical_child(partition, chunk_ordinal)? {
            RelationalRegionProofOutcome::ExactEmpty(closure)
                if closure.proof().artifact() == artifact =>
            {
                Ok(closure.proof().clone())
            }
            RelationalRegionProofOutcome::ExactEmpty(_) => {
                Err(RelationalRegionProofError::ArtifactSemanticMismatch)
            }
            RelationalRegionProofOutcome::ConcreteFallback { residual } => Err(
                RelationalRegionProofError::ArtifactNoLongerProvable(residual),
            ),
        }
    }

    fn target<'a>(
        &self,
        partition: &'a VerifiedRelationalCaseChunkPartition,
        chunk_ordinal: usize,
    ) -> Result<RelationalRegionProofTarget<'a>, RelationalRegionProofError> {
        let artifact = partition.artifact();
        if artifact.plan_root() != self.support_plan.root()
            || artifact.relation_id() != self.support_plan.relation_id()
            || artifact.admission_id() != self.support_plan.admission_id()
            || artifact.question_ids() != [self.question_id]
            || artifact.root_cell_id()
                != self
                    .support_plan
                    .root_cell_id()
                    .ok_or(RelationalRegionProofError::ExpectedNonemptyRoot)?
        {
            return Err(RelationalRegionProofError::PartitionScopeMismatch);
        }
        let chunk = partition
            .partition()
            .chunks()
            .get(chunk_ordinal)
            .ok_or(RelationalRegionProofError::CanonicalChunkMissing)?;
        if chunk.descriptor().ordinal()
            != u128::try_from(chunk_ordinal)
                .map_err(|_| RelationalRegionProofError::InvalidArtifactShape)?
        {
            return Err(RelationalRegionProofError::CanonicalChunkMismatch);
        }
        Ok(RelationalRegionProofTarget {
            subject: RelationalRegionProofSubject::CanonicalChunk {
                partition_artifact_id: artifact.id(),
                chunk_id: chunk.descriptor().id(),
                chunk_ordinal: chunk.descriptor().ordinal(),
                chunk_cell_id: chunk.cell().id(),
                chunk_materializer_id: chunk.cell().materializer_id(),
            },
            cell: chunk.cell(),
            root_coordinate_start: artifact.interval_start(),
            root_coordinate_end_exclusive: artifact.interval_end_exclusive(),
            coordinate_start: chunk.descriptor().interval_start(),
            coordinate_end_exclusive: chunk.descriptor().interval_end_exclusive(),
        })
    }
}

#[derive(Clone, Copy)]
struct RelationalRegionProofTarget<'a> {
    subject: RelationalRegionProofSubject,
    cell: &'a SupportCell,
    root_coordinate_start: u128,
    root_coordinate_end_exclusive: u128,
    coordinate_start: u128,
    coordinate_end_exclusive: u128,
}

/// Attempt constant-size proof closure of one checked one-axis relation.
///
/// Complexity is proportional to the expanded acyclic capsule expression
/// (with logarithmic node/callable lookup) and its distinct constant-division
/// terms. The integer-domain cardinality is absent from the complexity:
/// `0..<200_000` and `0..<3_000_000` cost the same when the formula closes as
/// one uniform region.
pub(crate) fn prove_relational_exact_empty_region(
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
    prove_relational_region(checked, support_plan, capsule, None, [0; 32])
}

fn validate_relational_region_scope(
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
) -> Result<QuestionId, RelationalRegionProofError> {
    checked
        .closed_query
        .validate()
        .map_err(RelationalRegionProofError::InvalidQuery)?;
    let checked_question_id = require_single_question(checked.question_ids())?;
    let plan_question_id = require_single_question(support_plan.question_ids())?;
    if !support_plan.validate_root()
        || checked.relation_id() != support_plan.relation_id()
        || checked.admission_id() != support_plan.admission_id()
        || checked_question_id != plan_question_id
    {
        return Err(RelationalRegionProofError::CheckedPlanScopeMismatch);
    }
    let checked_program = decode_lowercase_sha256(checked.program_hash())
        .ok_or(RelationalRegionProofError::InvalidCheckedProgramDigest)?;
    let checked_provenance =
        decode_lowercase_sha256(checked.source_coverage().manifest_digest.as_ref())
            .ok_or(RelationalRegionProofError::InvalidCheckedProvenanceDigest)?;
    let questions = FrozenClassificationQuestionSet::freeze([checked_question_id])
        .map_err(|_| RelationalRegionProofError::ClassificationCapsuleScopeMismatch)?;
    if !capsule.validates_binding(
        checked_program,
        checked.relation_id(),
        checked.admission_id(),
        questions.root(),
        support_plan.root(),
        support_plan.root_cell_id(),
    ) || capsule.question_ids() != questions.question_ids()
        || capsule.graph_root() != checked.classification_program().graph_root()
        || capsule.runtime_shape_root()
            != checked.classification_runtime_shapes().runtime_shape_root()
        || capsule.specialization_root() != ClassificationSpecializationRoot::none()
        || capsule.provenance_root()
            != ClassificationProvenanceRoot::from_checked_source_coverage_digest(checked_provenance)
    {
        return Err(RelationalRegionProofError::ClassificationCapsuleScopeMismatch);
    }
    Ok(checked_question_id)
}

fn prove_relational_region(
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
    target: Option<&RelationalRegionProofTarget<'_>>,
    replay_authority_id: [u8; 32],
) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
    let question_id = validate_relational_region_scope(checked, support_plan, capsule)?;
    let graph = capsule.graph();

    let inventory = RelationalProofStrategyInventory::from_checked(checked, support_plan)?;
    let [root_axis] = inventory.axes() else {
        return Ok(fallback(RelationalRegionProofResidual::IntegerAxisCount {
            found: inventory.axes().len(),
        }));
    };
    let axis = match target {
        Some(target)
            if target.root_coordinate_start == root_axis.coordinate_start()
                && target.root_coordinate_end_exclusive == root_axis.coordinate_end_exclusive() =>
        {
            root_axis
                .restrict_to_coordinates(target.coordinate_start, target.coordinate_end_exclusive)?
        }
        Some(_) => {
            return Ok(fallback(
                RelationalRegionProofResidual::CaseImageCardinalityLiftUnavailable,
            ));
        }
        None => root_axis.clone(),
    };
    let axis_lane = ClassificationSemanticLane::SourceBinding(axis.binding_index());
    let axis_root_id = match required_lane_root(graph, axis_lane) {
        Ok(root) => root,
        Err(residual) => return Ok(fallback(residual)),
    };
    if !axis_is_direct_before(checked, graph, axis_root_id, root_axis) {
        return Ok(fallback(
            RelationalRegionProofResidual::BeforeIsNotIndependentIntegerAxis,
        ));
    }
    let context_index = checked.closed_query.source.context_binding_index;
    let context_ordinal = u32::try_from(context_index)
        .map_err(|_| RelationalRegionProofError::ClassificationIndexOverflow("context binding"))?;
    let context_lane = ClassificationSemanticLane::SourceBinding(context_ordinal);
    let context_root_id = match required_lane_root(graph, context_lane) {
        Ok(root) => root,
        Err(residual) => return Ok(fallback(residual)),
    };
    if !source_has_only_direct_before_and_unit_context(checked, graph, context_root_id, root_axis) {
        return Ok(fallback(
            RelationalRegionProofResidual::SourceHasAuxiliaryOrNonUnitContext,
        ));
    }
    if !matches!(
        &checked.closed_query.successor.kind,
        ExploreSuccessorKindIr::Singleton { .. }
    ) {
        return Ok(fallback(
            RelationalRegionProofResidual::FiniteSuccessorNeedsFiberProof,
        ));
    }
    let Some(source_chain) = correlated_source_chain(support_plan, root_axis) else {
        return Ok(fallback(
            RelationalRegionProofResidual::CaseImageCardinalityLiftUnavailable,
        ));
    };

    let admission_lane_count = graph
        .lane_manifest()
        .iter()
        .filter(|entry| matches!(entry.lane, ClassificationSemanticLane::Admission { .. }))
        .count();
    let checked_admission_count = checked.closed_query.admissions.len();
    if checked_admission_count != 0 || admission_lane_count != 0 {
        return Ok(fallback(
            RelationalRegionProofResidual::AdmissionFormulaNormalizationRequired {
                predicates: checked_admission_count.max(admission_lane_count),
            },
        ));
    }
    if checked
        .closed_query
        .finds
        .iter()
        .all(|named_find| matches!(&named_find.find, ExploreFindIr::All { .. }))
    {
        return Ok(fallback(
            RelationalRegionProofResidual::SelectionUniformlySelected,
        ));
    }

    let successor_root_id = match required_lane_root(graph, ClassificationSemanticLane::Successor) {
        Ok(root) => root,
        Err(residual) => return Ok(fallback(residual)),
    };
    let find_root_id =
        match required_lane_root(graph, ClassificationSemanticLane::Find(question_id)) {
            Ok(root) => root,
            Err(residual) => return Ok(fallback(residual)),
        };
    let integer_type = classification_node(graph, axis_root_id)?.ty;
    let boolean_type = classification_node(graph, find_root_id)?.ty;
    if integer_type == boolean_type {
        return Ok(fallback(RelationalRegionProofResidual::Expression(
            RelationalRegionExpressionResidual {
                layer: RelationalRegionExpressionLayer::Selection,
                node_path: vec![find_root_id].into_boxed_slice(),
                reason: RelationalRegionExpressionResidualReason::InvalidClassificationGraph,
            },
        )));
    }
    let scalar_types = RelationalGraphScalarTypes {
        integer: integer_type,
        boolean: boolean_type,
    };
    let successor = match RelationalGraphNormalizer::new(
        graph,
        &axis,
        scalar_types,
        RelationalRegionExpressionLayer::Successor,
        None,
    )
    .normalize_integer(successor_root_id)
    {
        Ok(value) => value,
        Err(residual) => {
            return Ok(fallback(RelationalRegionProofResidual::Expression(
                residual,
            )));
        }
    };
    let selected_formula = match RelationalGraphNormalizer::new(
        graph,
        &axis,
        scalar_types,
        RelationalRegionExpressionLayer::Selection,
        Some(successor.clone()),
    )
    .normalize_boolean(find_root_id)
    {
        Ok(value) => value,
        Err(residual) => {
            return Ok(fallback(RelationalRegionProofResidual::Expression(
                residual,
            )));
        }
    };

    let selected_truth = match selected_formula.truth_domain(&axis) {
        Ok(truth) => truth,
        Err(RelationalRegionProofError::ArithmeticOverflow(_)) => {
            return Ok(fallback(
                RelationalRegionProofResidual::ProofArithmeticCapacityExceeded,
            ));
        }
        Err(error) => return Err(error),
    };
    match selected_truth {
        TruthDomain::FALSE => {}
        TruthDomain::TRUE => {
            return Ok(fallback(
                RelationalRegionProofResidual::SelectionUniformlySelected,
            ));
        }
        TruthDomain::BOTH => {
            return Ok(fallback(
                RelationalRegionProofResidual::SelectionTruthVariesOverAxis,
            ));
        }
        TruthDomain {
            can_be_false: false,
            can_be_true: false,
        } => {
            return Err(RelationalRegionProofError::InternalProofInvariant(
                "selection proof produced an empty truth domain",
            ));
        }
    }

    let (root_cell_id, root_cardinality_obligation, root_admission_obligation) =
        root_obligations(support_plan)?;
    let root_cell = support_plan
        .cell_catalog()
        .get(root_cell_id)
        .ok_or(RelationalRegionProofError::RootCellMissing(root_cell_id))?;
    let proof_cell = target.map_or(root_cell, |target| target.cell);
    let cardinality_obligation = match target {
        Some(_) => SupportCellObligation::new(proof_cell, ExactCardinalityClaim)?,
        None => root_cardinality_obligation,
    };
    let admission_obligation = match target {
        Some(_) => SupportCellObligation::new(
            proof_cell,
            AdmissionClassificationClaim::new(checked.admission_id()),
        )?,
        None => root_admission_obligation,
    };
    let selection_obligation =
        SupportCellObligation::new(proof_cell, SelectionClassificationClaim::new(question_id))?;
    let case_cardinality = axis.cardinality();

    let selected_formula_digest = formula_digest(&selected_formula);
    let mut artifact = RelationalRegionProofArtifact {
        schema_version: RELATIONAL_REGION_PROOF_VERSION,
        certificate_id: [0; 32],
        replay_authority_id,
        classification_capsule_id: capsule.id(),
        successor_root_id,
        find_root_id,
        relation_id: checked.relation_id(),
        admission_id: checked.admission_id(),
        question_id,
        plan_root: support_plan.root(),
        root_cell_id,
        subject: target.map_or(RelationalRegionProofSubject::Root, |target| target.subject),
        conclusion: RelationalCertifiedRegionConclusion::AdmittedNotSelected,
        starter_region_id: RelationalStarterRegionId([0; 32]),
        source_assignment_cell_id: source_chain.assignment_cell.id(),
        source_row_cell_id: source_chain.source_row_cell.id(),
        successor_coordinate_cell_id: source_chain.successor_coordinate_cell.id(),
        axis_stage_id: axis.stage_id(),
        axis_dimension_id: axis.dimension_id(),
        axis_cell_id: axis.cell().id(),
        value_start: axis.value_start(),
        value_end_exclusive: axis.value_end_exclusive(),
        coordinate_start: axis.coordinate_start(),
        coordinate_end_exclusive: axis.coordinate_end_exclusive(),
        case_cardinality,
        selected_formula_digest,
    };
    artifact.starter_region_id = derive_starter_region_id(&artifact);
    artifact.certificate_id = derive_certificate_id(&artifact);
    artifact.validate_identity()?;
    let certificate_id = artifact.certificate_id;
    let bindings = [
        evidence_binding(
            certificate_id,
            RelationalRegionEvidenceRole::Cardinality,
            cardinality_obligation.id(),
            cardinality_obligation
                .claim()
                .conclusion_digest(&case_cardinality),
        ),
        evidence_binding(
            certificate_id,
            RelationalRegionEvidenceRole::Admission,
            admission_obligation.id(),
            admission_obligation
                .claim()
                .conclusion_digest(&AdmissionDecision::Admitted),
        ),
        evidence_binding(
            certificate_id,
            RelationalRegionEvidenceRole::Selection,
            selection_obligation.id(),
            selection_obligation
                .claim()
                .conclusion_digest(&SelectionDecision::NotSelected),
        ),
    ];
    let proof = VerifiedRelationalRegionProof { artifact, bindings };

    let cardinality = super::support_cell::relational_region_proof_gateway::cardinality(
        &proof,
        cardinality_obligation,
        case_cardinality,
    )?;
    let admission = super::support_cell::relational_region_proof_gateway::admission(
        &proof,
        admission_obligation,
        AdmissionDecision::Admitted,
    )?;
    let selection = super::support_cell::relational_region_proof_gateway::selection(
        &proof,
        selection_obligation,
        SelectionDecision::NotSelected,
    )?;
    let mut events = vec![
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Cardinality(cardinality)),
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Admission(admission)),
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Selection(selection)),
        SupportJournalEvent::leaf_sealed(proof_cell.id()),
    ];
    if target.is_none() {
        events.push(SupportJournalEvent::ObligationFrontierSealed);
        events.push(SupportJournalEvent::CatalogSealed);
    }
    Ok(RelationalRegionProofOutcome::ExactEmpty(
        RelationalRegionSupportClosure {
            proof,
            events: events.into_boxed_slice(),
        },
    ))
}

/// Replay verifier for a decoded proof artifact.
///
/// The checked query view, support plan, and exact capsule are required external
/// inputs. The verifier reruns graph normalization and interval closure from
/// those producer-bound artifacts, then requires byte-for-byte semantic
/// equality with the decoded artifact before returning receipt-bearing support
/// events. A journal codec must never skip this call or restore
/// [`VerifiedRelationalRegionProof`] directly.
pub(crate) fn reverify_relational_region_proof_artifact(
    artifact: &RelationalRegionProofArtifact,
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
    capsule: &RelationalClassificationCapsule,
) -> Result<RelationalRegionSupportClosure, RelationalRegionProofError> {
    artifact.validate_identity()?;
    if artifact.classification_capsule_id() != capsule.id()
        || artifact.subject() != RelationalRegionProofSubject::Root
        || artifact.replay_authority_id() != [0; 32]
    {
        return Err(RelationalRegionProofError::ArtifactSemanticMismatch);
    }
    match prove_relational_exact_empty_region(checked, support_plan, capsule)? {
        RelationalRegionProofOutcome::ExactEmpty(closure)
            if closure.proof.artifact() == artifact =>
        {
            Ok(closure)
        }
        RelationalRegionProofOutcome::ExactEmpty(_) => {
            Err(RelationalRegionProofError::ArtifactSemanticMismatch)
        }
        RelationalRegionProofOutcome::ConcreteFallback { residual } => Err(
            RelationalRegionProofError::ArtifactNoLongerProvable(residual),
        ),
    }
}

fn required_lane_root(
    graph: &FrozenClassificationProgram,
    lane: ClassificationSemanticLane,
) -> Result<ClassificationNodeId, RelationalRegionProofResidual> {
    match graph.lane_is_complete(lane) {
        Some(true) => graph
            .roots()
            .binary_search_by_key(&lane, |root| root.lane)
            .ok()
            .map(|index| graph.roots()[index].node)
            .ok_or(RelationalRegionProofResidual::ClassificationLaneMissing { lane }),
        Some(false) => Err(RelationalRegionProofResidual::ClassificationLaneResidual { lane }),
        None => Err(RelationalRegionProofResidual::ClassificationLaneMissing { lane }),
    }
}

fn require_single_question(
    question_ids: &[QuestionId],
) -> Result<QuestionId, RelationalRegionProofError> {
    let [question_id] = question_ids else {
        return Err(RelationalRegionProofError::QuestionArityMismatch {
            actual: question_ids.len(),
        });
    };
    Ok(*question_id)
}

fn classification_node(
    graph: &FrozenClassificationProgram,
    node_id: ClassificationNodeId,
) -> Result<&ClassificationNodeKey, RelationalRegionProofError> {
    graph
        .nodes()
        .binary_search_by_key(&node_id, |(candidate, _)| *candidate)
        .ok()
        .map(|index| &graph.nodes()[index].1)
        .ok_or(RelationalRegionProofError::ClassificationNodeMissing(
            node_id,
        ))
}

fn classification_callable(
    graph: &FrozenClassificationProgram,
    callable_id: ClassificationCallableId,
) -> Result<&ClassificationCallableDefinition, RelationalRegionProofError> {
    graph
        .callables()
        .binary_search_by_key(&callable_id, |definition| definition.callable_id)
        .ok()
        .map(|index| &graph.callables()[index])
        .ok_or(RelationalRegionProofError::ClassificationCallableMissing(
            callable_id,
        ))
}

fn decode_lowercase_sha256(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value
            .as_bytes()
            .iter()
            .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return None;
    }
    let mut digest = [0u8; 32];
    for (index, output) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&value[offset..offset + 2], 16).ok()?;
    }
    Some(digest)
}

fn axis_is_direct_before(
    checked: &CheckedExploreQueryView<'_>,
    graph: &FrozenClassificationProgram,
    axis_root_id: ClassificationNodeId,
    axis: &RelationalIntegerAxis,
) -> bool {
    let Some(binding) = checked
        .closed_query
        .source
        .bindings
        .get(axis.binding_index() as usize)
    else {
        return false;
    };
    if binding.role != ExploreSourceBindingRoleIr::Before || !binding.dependencies.is_empty() {
        return false;
    }
    if !matches!(
        classification_node(graph, axis_root_id).map(|node| &node.kind),
        Ok(ClassificationNodeKind::SourceParameter(ordinal)) if *ordinal == axis.binding_index()
    ) {
        return false;
    }
    let ExploreSourceBindingKindIr::Finite { domain } = &binding.kind else {
        return false;
    };
    match domain {
        super::relational_ir::ExploreFiniteDomainIr::Exact(ExploreExactDomain::IntRange {
            start,
            end_exclusive,
            cardinality,
        }) => {
            *start == axis.value_start()
                && *end_exclusive == axis.value_end_exclusive()
                && u128::from(*cardinality) == axis.cardinality()
        }
        // `RelationalProofStrategyInventory` admits this variant only after
        // both checked endpoints have been evaluated to the exact axis values
        // and matched to the support-plan coordinate interval.
        super::relational_ir::ExploreFiniteDomainIr::IntRange { .. } => true,
        super::relational_ir::ExploreFiniteDomainIr::Exact(
            ExploreExactDomain::Enumerated { .. } | ExploreExactDomain::FiniteType { .. },
        )
        | super::relational_ir::ExploreFiniteDomainIr::Collection { .. } => false,
    }
}

fn source_has_only_direct_before_and_unit_context(
    checked: &CheckedExploreQueryView<'_>,
    graph: &FrozenClassificationProgram,
    context_root_id: ClassificationNodeId,
    axis: &RelationalIntegerAxis,
) -> bool {
    if checked.closed_query.source.bindings.len() != 2 {
        return false;
    }
    let before_index = axis.binding_index() as usize;
    let context_index = checked.closed_query.source.context_binding_index;
    if before_index == context_index
        || checked.closed_query.source.before_binding_index != before_index
    {
        return false;
    }
    let context = &checked.closed_query.source.bindings[context_index];
    context.role == ExploreSourceBindingRoleIr::Context
        && context.dependencies.is_empty()
        && matches!(&context.kind, ExploreSourceBindingKindIr::Singleton { .. })
        && matches!(
            classification_node(graph, context_root_id).map(|node| &node.kind),
            Ok(ClassificationNodeKind::Constant(
                ClassificationConstant::Unit
            ))
        )
}

#[derive(Clone, Copy)]
struct RelationalCorrelatedSourceChain<'a> {
    assignment_cell: &'a SupportCell,
    source_row_cell: &'a SupportCell,
    successor_coordinate_cell: &'a SupportCell,
}

fn correlated_source_chain<'a>(
    plan: &'a RelationalSupportPlan,
    axis: &RelationalIntegerAxis,
) -> Option<RelationalCorrelatedSourceChain<'a>> {
    let Some(assignment_cell) = plan.source_assignments().cell() else {
        return None;
    };
    let RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells } =
        plan.source_assignments().recipe()
    else {
        return None;
    };
    if factor_cells.len() != 1 || factor_cells[0] != axis.cell().id() {
        return None;
    }
    let Some(source_cell) = plan.source_rows().cell() else {
        return None;
    };
    if !matches!(
        plan.source_rows().recipe(),
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell: id }
            if *id == assignment_cell.id()
    ) {
        return None;
    }
    let Some(successor_cell) = plan.successor_coordinates().cell() else {
        return None;
    };
    if !matches!(
        plan.successor_coordinates().recipe(),
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell,
            successor_kind: RelationalSuccessorRecipeKind::Singleton,
        } if *source_row_cell == source_cell.id()
    ) {
        return None;
    }
    let Some(case_cell) = plan.cases().cell() else {
        return None;
    };
    if !matches!(
        plan.cases().recipe(),
        RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell,
        } if *successor_coordinate_cell == successor_cell.id()
    ) || plan.root_cell_id() != Some(case_cell.id())
    {
        return None;
    }
    Some(RelationalCorrelatedSourceChain {
        assignment_cell,
        source_row_cell: source_cell,
        successor_coordinate_cell: successor_cell,
    })
}

type CardinalityObligation = SupportCellObligation<ExactCardinalityClaim>;
type AdmissionObligation = SupportCellObligation<AdmissionClassificationClaim>;

fn root_obligations(
    plan: &RelationalSupportPlan,
) -> Result<(SupportCellId, CardinalityObligation, AdmissionObligation), RelationalRegionProofError>
{
    let RelationalRootObligationPlan::CellBacked {
        root_cell_id,
        descriptors,
    } = plan.root_obligations()
    else {
        return Err(RelationalRegionProofError::ExpectedNonemptyRoot);
    };
    let mut cardinality = None;
    let mut admission = None;
    for descriptor in descriptors.iter() {
        let RelationalStagedObligationDescriptor::Root {
            activation: RelationalObligationActivation::RootCasePopulation,
            obligation,
        } = descriptor
        else {
            continue;
        };
        match obligation {
            SupportObligationRecord::Cardinality(value) if cardinality.is_none() => {
                cardinality = Some(value.clone())
            }
            SupportObligationRecord::Admission(value) if admission.is_none() => {
                admission = Some(value.clone())
            }
            _ => {}
        }
    }
    let cardinality = cardinality.ok_or(RelationalRegionProofError::RootObligationMissing(
        RelationalRegionEvidenceRole::Cardinality,
    ))?;
    let admission = admission.ok_or(RelationalRegionProofError::RootObligationMissing(
        RelationalRegionEvidenceRole::Admission,
    ))?;
    if cardinality.cell_id() != *root_cell_id || admission.cell_id() != *root_cell_id {
        return Err(RelationalRegionProofError::RootObligationCellMismatch);
    }
    Ok((*root_cell_id, cardinality, admission))
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum RelationalRelation {
    Less,
    LessOrEqual,
    Equal,
    NotEqual,
    GreaterOrEqual,
    Greater,
}

impl RelationalRelation {
    const fn tag(self) -> u8 {
        match self {
            Self::Less => 0,
            Self::LessOrEqual => 1,
            Self::Equal => 2,
            Self::NotEqual => 3,
            Self::GreaterOrEqual => 4,
            Self::Greater => 5,
        }
    }

    const fn truth_domain(self, minimum: i128, maximum: i128) -> TruthDomain {
        match self {
            Self::Less if maximum < 0 => TruthDomain::TRUE,
            Self::Less if minimum >= 0 => TruthDomain::FALSE,
            Self::LessOrEqual if maximum <= 0 => TruthDomain::TRUE,
            Self::LessOrEqual if minimum > 0 => TruthDomain::FALSE,
            Self::Equal if minimum == 0 && maximum == 0 => TruthDomain::TRUE,
            Self::Equal if maximum < 0 || minimum > 0 => TruthDomain::FALSE,
            Self::NotEqual if maximum < 0 || minimum > 0 => TruthDomain::TRUE,
            Self::NotEqual if minimum == 0 && maximum == 0 => TruthDomain::FALSE,
            Self::GreaterOrEqual if minimum >= 0 => TruthDomain::TRUE,
            Self::GreaterOrEqual if maximum < 0 => TruthDomain::FALSE,
            Self::Greater if minimum > 0 => TruthDomain::TRUE,
            Self::Greater if maximum <= 0 => TruthDomain::FALSE,
            _ => TruthDomain::BOTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RelationalAffine {
    coefficient: i128,
    intercept: i128,
}

impl RelationalAffine {
    const fn constant(value: i128) -> Self {
        Self {
            coefficient: 0,
            intercept: value,
        }
    }

    const fn axis() -> Self {
        Self {
            coefficient: 1,
            intercept: 0,
        }
    }

    fn bounds(
        self,
        axis: &RelationalIntegerAxis,
    ) -> Result<(i128, i128), RelationalRegionProofError> {
        let lower = self.evaluate(i128::from(axis.value_start()))?;
        let final_axis = i128::from(axis.value_end_exclusive())
            .checked_sub(1)
            .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                "bounding an empty integer axis",
            ))?;
        let upper = self.evaluate(final_axis)?;
        Ok((lower.min(upper), lower.max(upper)))
    }

    fn evaluate(self, axis: i128) -> Result<i128, RelationalRegionProofError> {
        self.coefficient
            .checked_mul(axis)
            .and_then(|value| value.checked_add(self.intercept))
            .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                "evaluating an affine proof term",
            ))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RelationalQuantizedTerm {
    numerator: RelationalAffine,
    positive_divisor: i64,
    coefficient: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalQuasiAffine {
    affine: RelationalAffine,
    terms: Box<[RelationalQuantizedTerm]>,
}

impl RelationalQuasiAffine {
    fn constant(value: i128) -> Self {
        Self {
            affine: RelationalAffine::constant(value),
            terms: Box::new([]),
        }
    }

    fn axis(axis: &RelationalIntegerAxis) -> Result<Self, RelationalRegionProofError> {
        let value = Self {
            affine: RelationalAffine::axis(),
            terms: Box::new([]),
        };
        value.require_runtime_int(axis).map_err(|_| {
            RelationalRegionProofError::InternalProofInvariant(
                "checked integer axis does not fit Futuruna Int",
            )
        })?;
        Ok(value)
    }

    fn is_constant(&self) -> Option<i128> {
        (self.affine.coefficient == 0 && self.terms.is_empty()).then_some(self.affine.intercept)
    }

    fn canonical(
        affine: RelationalAffine,
        terms: impl IntoIterator<Item = RelationalQuantizedTerm>,
    ) -> Result<Self, RelationalRegionProofError> {
        let mut combined = BTreeMap::<(RelationalAffine, i64), i128>::new();
        for term in terms {
            if term.positive_divisor <= 0 {
                return Err(RelationalRegionProofError::InternalProofInvariant(
                    "nonpositive divisor reached canonical proof form",
                ));
            }
            let coefficient = combined
                .get(&(term.numerator, term.positive_divisor))
                .copied()
                .unwrap_or(0)
                .checked_add(term.coefficient)
                .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                    "combining quantized proof terms",
                ))?;
            if coefficient == 0 {
                combined.remove(&(term.numerator, term.positive_divisor));
            } else {
                combined.insert((term.numerator, term.positive_divisor), coefficient);
            }
        }
        Ok(Self {
            affine,
            terms: combined
                .into_iter()
                .map(
                    |((numerator, positive_divisor), coefficient)| RelationalQuantizedTerm {
                        numerator,
                        positive_divisor,
                        coefficient,
                    },
                )
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    fn add(&self, other: &Self) -> Result<Self, RelationalRegionProofError> {
        let affine = RelationalAffine {
            coefficient: self
                .affine
                .coefficient
                .checked_add(other.affine.coefficient)
                .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                    "adding affine coefficients",
                ))?,
            intercept: self
                .affine
                .intercept
                .checked_add(other.affine.intercept)
                .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                    "adding affine intercepts",
                ))?,
        };
        Self::canonical(affine, self.terms.iter().chain(other.terms.iter()).copied())
    }

    fn scale(&self, scalar: i128) -> Result<Self, RelationalRegionProofError> {
        let affine = RelationalAffine {
            coefficient: self.affine.coefficient.checked_mul(scalar).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("scaling an affine coefficient"),
            )?,
            intercept: self.affine.intercept.checked_mul(scalar).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("scaling an affine intercept"),
            )?,
        };
        Self::canonical(
            affine,
            self.terms
                .iter()
                .map(|term| {
                    term.coefficient
                        .checked_mul(scalar)
                        .map(|coefficient| RelationalQuantizedTerm {
                            coefficient,
                            ..*term
                        })
                        .ok_or(RelationalRegionProofError::ArithmeticOverflow(
                            "scaling a quantized coefficient",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    fn subtract(&self, other: &Self) -> Result<Self, RelationalRegionProofError> {
        self.add(&other.scale(-1)?)
    }

    fn bounds(
        &self,
        axis: &RelationalIntegerAxis,
    ) -> Result<(i128, i128), RelationalRegionProofError> {
        let (mut minimum, mut maximum) = self.affine.bounds(axis)?;
        for term in self.terms.iter() {
            let (numerator_minimum, numerator_maximum) = term.numerator.bounds(axis)?;
            if numerator_minimum < 0 {
                return Err(RelationalRegionProofError::InternalProofInvariant(
                    "negative quantized numerator reached exact proof form",
                ));
            }
            let divisor = i128::from(term.positive_divisor);
            let quotient_minimum = numerator_minimum.checked_div(divisor).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("bounding constant division"),
            )?;
            let quotient_maximum = numerator_maximum.checked_div(divisor).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("bounding constant division"),
            )?;
            let first = quotient_minimum.checked_mul(term.coefficient).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("scaling a quantized lower bound"),
            )?;
            let second = quotient_maximum.checked_mul(term.coefficient).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("scaling a quantized upper bound"),
            )?;
            minimum = minimum.checked_add(first.min(second)).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("summing quasi-affine lower bounds"),
            )?;
            maximum = maximum.checked_add(first.max(second)).ok_or(
                RelationalRegionProofError::ArithmeticOverflow("summing quasi-affine upper bounds"),
            )?;
        }
        Ok((minimum, maximum))
    }

    fn require_runtime_int(
        &self,
        axis: &RelationalIntegerAxis,
    ) -> Result<(), RelationalRegionExpressionResidualReason> {
        let (minimum, maximum) = self
            .bounds(axis)
            .map_err(|_| RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)?;
        if minimum < i128::from(i64::MIN) || maximum > i128::from(i64::MAX) {
            Err(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RelationalBooleanFormula {
    Constant(bool),
    Comparison {
        difference: RelationalQuasiAffine,
        relation: RelationalRelation,
    },
    Not(Box<RelationalBooleanFormula>),
    All(Box<[RelationalBooleanFormula]>),
    Any(Box<[RelationalBooleanFormula]>),
}

impl RelationalBooleanFormula {
    fn truth_domain(
        &self,
        axis: &RelationalIntegerAxis,
    ) -> Result<TruthDomain, RelationalRegionProofError> {
        match self {
            Self::Constant(false) => Ok(TruthDomain::FALSE),
            Self::Constant(true) => Ok(TruthDomain::TRUE),
            Self::Comparison {
                difference,
                relation,
            } => {
                let (minimum, maximum) = difference.bounds(axis)?;
                Ok(relation.truth_domain(minimum, maximum))
            }
            Self::Not(inner) => inner.truth_domain(axis).map(TruthDomain::negate),
            Self::All(parts) => {
                let mut result = TruthDomain::TRUE;
                for part in parts.iter() {
                    result = result.and(part.truth_domain(axis)?);
                }
                Ok(result)
            }
            Self::Any(parts) => {
                let mut result = TruthDomain::FALSE;
                for part in parts.iter() {
                    result = result.or(part.truth_domain(axis)?);
                }
                Ok(result)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TruthDomain {
    can_be_false: bool,
    can_be_true: bool,
}

impl TruthDomain {
    const FALSE: Self = Self {
        can_be_false: true,
        can_be_true: false,
    };
    const TRUE: Self = Self {
        can_be_false: false,
        can_be_true: true,
    };
    const BOTH: Self = Self {
        can_be_false: true,
        can_be_true: true,
    };

    const fn negate(self) -> Self {
        Self {
            can_be_false: self.can_be_true,
            can_be_true: self.can_be_false,
        }
    }

    const fn and(self, other: Self) -> Self {
        Self {
            can_be_false: self.can_be_false || other.can_be_false,
            can_be_true: self.can_be_true && other.can_be_true,
        }
    }

    const fn or(self, other: Self) -> Self {
        Self {
            can_be_false: self.can_be_false && other.can_be_false,
            can_be_true: self.can_be_true || other.can_be_true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RelationalGraphScalarTypes {
    integer: ClassificationTypeId,
    boolean: ClassificationTypeId,
}

#[derive(Clone, Debug)]
enum RelationalNormalizedScalar {
    Integer(RelationalQuasiAffine),
    Boolean(RelationalBooleanFormula),
}

#[derive(Clone, Debug)]
struct RelationalCallableFrame {
    callable_id: ClassificationCallableId,
    arguments: Box<[RelationalNormalizedScalar]>,
}

struct RelationalGraphNormalizer<'graph, 'axis> {
    graph: &'graph FrozenClassificationProgram,
    axis: &'axis RelationalIntegerAxis,
    scalar_types: RelationalGraphScalarTypes,
    layer: RelationalRegionExpressionLayer,
    after: Option<RelationalQuasiAffine>,
    frames: Vec<RelationalCallableFrame>,
    active_calls: Vec<ClassificationCallableId>,
    node_path: Vec<ClassificationNodeId>,
    remaining_steps: usize,
}

impl<'graph, 'axis> RelationalGraphNormalizer<'graph, 'axis> {
    fn new(
        graph: &'graph FrozenClassificationProgram,
        axis: &'axis RelationalIntegerAxis,
        scalar_types: RelationalGraphScalarTypes,
        layer: RelationalRegionExpressionLayer,
        after: Option<RelationalQuasiAffine>,
    ) -> Self {
        Self {
            graph,
            axis,
            scalar_types,
            layer,
            after,
            frames: Vec::new(),
            active_calls: Vec::new(),
            node_path: Vec::new(),
            remaining_steps: MAX_GRAPH_NORMALIZATION_STEPS,
        }
    }

    fn begin_node(
        &mut self,
        node_id: ClassificationNodeId,
    ) -> Result<(), RelationalRegionExpressionResidual> {
        if self.remaining_steps == 0 || self.node_path.len() >= MAX_GRAPH_NORMALIZATION_DEPTH {
            self.node_path.push(node_id);
            let residual = self
                .residual(RelationalRegionExpressionResidualReason::NormalizationCapacityExceeded);
            self.node_path.pop();
            return Err(residual);
        }
        self.remaining_steps -= 1;
        self.node_path.push(node_id);
        Ok(())
    }

    fn residual(
        &self,
        reason: RelationalRegionExpressionResidualReason,
    ) -> RelationalRegionExpressionResidual {
        RelationalRegionExpressionResidual {
            layer: self.layer,
            node_path: self.node_path.clone().into_boxed_slice(),
            reason,
        }
    }

    fn node(
        &self,
        node_id: ClassificationNodeId,
    ) -> Result<ClassificationNodeKey, RelationalRegionExpressionResidual> {
        classification_node(self.graph, node_id)
            .cloned()
            .map_err(|_| {
                self.residual(RelationalRegionExpressionResidualReason::InvalidClassificationGraph)
            })
    }

    fn callable(
        &self,
        callable_id: ClassificationCallableId,
    ) -> Result<ClassificationCallableDefinition, RelationalRegionExpressionResidual> {
        classification_callable(self.graph, callable_id)
            .cloned()
            .map_err(|_| {
                self.residual(RelationalRegionExpressionResidualReason::InvalidClassificationGraph)
            })
    }

    fn normalize_integer(
        &mut self,
        node_id: ClassificationNodeId,
    ) -> Result<RelationalQuasiAffine, RelationalRegionExpressionResidual> {
        self.begin_node(node_id)?;
        let result = self.normalize_integer_node(node_id).and_then(|value| {
            value
                .require_runtime_int(self.axis)
                .map_err(|reason| self.residual(reason))?;
            Ok(value)
        });
        self.node_path.pop();
        result
    }

    fn normalize_integer_node(
        &mut self,
        node_id: ClassificationNodeId,
    ) -> Result<RelationalQuasiAffine, RelationalRegionExpressionResidual> {
        let node = self.node(node_id)?;
        if node.ty != self.scalar_types.integer {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::UnsupportedScalarType)
            );
        }
        match node.kind {
            ClassificationNodeKind::Constant(ClassificationConstant::Integer(value)) => {
                Ok(RelationalQuasiAffine::constant(i128::from(value)))
            }
            ClassificationNodeKind::Input(slot) if slot == ClassificationInputSlot::BEFORE => {
                RelationalQuasiAffine::axis(self.axis).map_err(|_| {
                    self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
                })
            }
            ClassificationNodeKind::Input(slot) if slot == ClassificationInputSlot::AFTER => {
                self.after.clone().ok_or_else(|| {
                    self.residual(RelationalRegionExpressionResidualReason::UnboundRelationalValue)
                })
            }
            ClassificationNodeKind::SourceParameter(ordinal)
                if ordinal == self.axis.binding_index() =>
            {
                RelationalQuasiAffine::axis(self.axis).map_err(|_| {
                    self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
                })
            }
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal,
            } => {
                match self.callable_argument(callable_id, ordinal)? {
                    RelationalNormalizedScalar::Integer(value) => Ok(value),
                    RelationalNormalizedScalar::Boolean(_) => Err(self
                        .residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)),
                }
            }
            ClassificationNodeKind::Unary {
                op: ClassificationUnaryOp::IntegerNegateChecked,
                operand,
            } => self.normalize_integer(operand)?.scale(-1).map_err(|_| {
                self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
            }),
            ClassificationNodeKind::Binary { op, left, right } => {
                self.normalize_integer_binary(op, left, right)
            }
            ClassificationNodeKind::If {
                condition,
                then_node,
                else_node,
            } => match self
                .normalize_boolean(condition)?
                .truth_domain(self.axis)
                .map_err(|_| {
                    self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
                })? {
                TruthDomain::TRUE => self.normalize_integer(then_node),
                TruthDomain::FALSE => self.normalize_integer(else_node),
                TruthDomain::BOTH => Err(self.residual(
                    RelationalRegionExpressionResidualReason::ConditionalTruthVariesOverAxis,
                )),
                _ => Err(self.residual(
                    RelationalRegionExpressionResidualReason::InvalidClassificationGraph,
                )),
            },
            ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } => self.normalize_integer_call(callable_id, &arguments),
            ClassificationNodeKind::Construct { .. }
            | ClassificationNodeKind::Project { .. }
            | ClassificationNodeKind::IsVariant { .. } => Err(self.residual(
                RelationalRegionExpressionResidualReason::StructuredStateProjectionRequired,
            )),
            ClassificationNodeKind::Constant(_)
            | ClassificationNodeKind::Input(_)
            | ClassificationNodeKind::SourceParameter(_)
            | ClassificationNodeKind::Unary { .. } => {
                Err(self
                    .residual(RelationalRegionExpressionResidualReason::UnsupportedIntegerOperator))
            }
        }
    }

    fn normalize_integer_binary(
        &mut self,
        op: ClassificationBinaryOp,
        left_id: ClassificationNodeId,
        right_id: ClassificationNodeId,
    ) -> Result<RelationalQuasiAffine, RelationalRegionExpressionResidual> {
        if !matches!(
            op,
            ClassificationBinaryOp::IntegerAddChecked
                | ClassificationBinaryOp::IntegerSubtractChecked
                | ClassificationBinaryOp::IntegerMultiplyChecked
                | ClassificationBinaryOp::IntegerDivideChecked
                | ClassificationBinaryOp::IntegerRemainderChecked
        ) {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::UnsupportedIntegerOperator)
            );
        }
        let left = self.normalize_integer(left_id)?;
        let right = self.normalize_integer(right_id)?;
        match op {
            ClassificationBinaryOp::IntegerAddChecked => left.add(&right),
            ClassificationBinaryOp::IntegerSubtractChecked => left.subtract(&right),
            ClassificationBinaryOp::IntegerMultiplyChecked => {
                match (left.is_constant(), right.is_constant()) {
                    (Some(scalar), _) => right.scale(scalar),
                    (_, Some(scalar)) => left.scale(scalar),
                    _ => {
                        return Err(self.residual(
                            RelationalRegionExpressionResidualReason::NonlinearIntegerExpression,
                        ));
                    }
                }
            }
            ClassificationBinaryOp::IntegerDivideChecked => {
                if let (Some(dividend), Some(divisor)) = (left.is_constant(), right.is_constant()) {
                    return dividend
                        .checked_div(divisor)
                        .map(RelationalQuasiAffine::constant)
                        .ok_or_else(|| {
                            self.residual(
                                RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                            )
                        });
                }
                let Some(divisor) = right
                    .is_constant()
                    .and_then(|value| i64::try_from(value).ok())
                else {
                    return Err(
                        self.residual(RelationalRegionExpressionResidualReason::NonpositiveDivisor)
                    );
                };
                if divisor <= 0 {
                    return Err(
                        self.residual(RelationalRegionExpressionResidualReason::NonpositiveDivisor)
                    );
                }
                if !left.terms.is_empty() {
                    return Err(self.residual(
                        RelationalRegionExpressionResidualReason::NestedQuantizedExpression,
                    ));
                }
                let (minimum, _) = left.affine.bounds(self.axis).map_err(|_| {
                    self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
                })?;
                if minimum < 0 {
                    return Err(self.residual(
                        RelationalRegionExpressionResidualReason::QuantizedNumeratorMayBeNegative,
                    ));
                }
                RelationalQuasiAffine::canonical(
                    RelationalAffine::constant(0),
                    [RelationalQuantizedTerm {
                        numerator: left.affine,
                        positive_divisor: divisor,
                        coefficient: 1,
                    }],
                )
            }
            ClassificationBinaryOp::IntegerRemainderChecked => {
                let (Some(dividend), Some(divisor)) = (left.is_constant(), right.is_constant())
                else {
                    return Err(self.residual(
                        RelationalRegionExpressionResidualReason::UnsupportedIntegerOperator,
                    ));
                };
                return dividend
                    .checked_rem(divisor)
                    .map(RelationalQuasiAffine::constant)
                    .ok_or_else(|| {
                        self.residual(
                            RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                        )
                    });
            }
            _ => unreachable!("integer operation checked above"),
        }
        .map_err(|_| {
            self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
        })
    }

    fn normalize_boolean(
        &mut self,
        node_id: ClassificationNodeId,
    ) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
        self.begin_node(node_id)?;
        let result = self.normalize_boolean_node(node_id);
        self.node_path.pop();
        result
    }

    fn normalize_boolean_node(
        &mut self,
        node_id: ClassificationNodeId,
    ) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
        let node = self.node(node_id)?;
        if node.ty != self.scalar_types.boolean {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::UnsupportedScalarType)
            );
        }
        match node.kind {
            ClassificationNodeKind::Constant(ClassificationConstant::Boolean(value)) => {
                Ok(RelationalBooleanFormula::Constant(value))
            }
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal,
            } => {
                match self.callable_argument(callable_id, ordinal)? {
                    RelationalNormalizedScalar::Boolean(value) => Ok(value),
                    RelationalNormalizedScalar::Integer(_) => Err(self
                        .residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)),
                }
            }
            ClassificationNodeKind::Unary {
                op: ClassificationUnaryOp::BooleanNot,
                operand,
            } => Ok(RelationalBooleanFormula::Not(Box::new(
                self.normalize_boolean(operand)?,
            ))),
            ClassificationNodeKind::Binary { op, left, right } => {
                self.normalize_boolean_binary(op, left, right)
            }
            ClassificationNodeKind::If {
                condition,
                then_node,
                else_node,
            } => match self
                .normalize_boolean(condition)?
                .truth_domain(self.axis)
                .map_err(|_| {
                    self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
                })? {
                TruthDomain::TRUE => self.normalize_boolean(then_node),
                TruthDomain::FALSE => self.normalize_boolean(else_node),
                TruthDomain::BOTH => Err(self.residual(
                    RelationalRegionExpressionResidualReason::ConditionalTruthVariesOverAxis,
                )),
                _ => Err(self.residual(
                    RelationalRegionExpressionResidualReason::InvalidClassificationGraph,
                )),
            },
            ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } => self.normalize_boolean_call(callable_id, &arguments),
            ClassificationNodeKind::Construct { .. }
            | ClassificationNodeKind::Project { .. }
            | ClassificationNodeKind::IsVariant { .. } => Err(self.residual(
                RelationalRegionExpressionResidualReason::StructuredStateProjectionRequired,
            )),
            ClassificationNodeKind::Constant(_)
            | ClassificationNodeKind::Input(_)
            | ClassificationNodeKind::SourceParameter(_)
            | ClassificationNodeKind::Unary { .. } => {
                Err(self
                    .residual(RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator))
            }
        }
    }

    fn normalize_boolean_binary(
        &mut self,
        op: ClassificationBinaryOp,
        left_id: ClassificationNodeId,
        right_id: ClassificationNodeId,
    ) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
        match op {
            ClassificationBinaryOp::BooleanAndShortCircuit
            | ClassificationBinaryOp::BooleanOrShortCircuit => {
                let parts = vec![
                    self.normalize_boolean(left_id)?,
                    self.normalize_boolean(right_id)?,
                ]
                .into_boxed_slice();
                Ok(if op == ClassificationBinaryOp::BooleanAndShortCircuit {
                    RelationalBooleanFormula::All(parts)
                } else {
                    RelationalBooleanFormula::Any(parts)
                })
            }
            ClassificationBinaryOp::Equal | ClassificationBinaryOp::NotEqual => {
                let left_type = self.node(left_id)?.ty;
                let right_type = self.node(right_id)?.ty;
                if left_type != right_type {
                    return Err(self.residual(
                        RelationalRegionExpressionResidualReason::InvalidClassificationGraph,
                    ));
                }
                if left_type == self.scalar_types.boolean {
                    let left = self.normalize_boolean(left_id)?;
                    let right = self.normalize_boolean(right_id)?;
                    let equal = boolean_equivalence(left, right);
                    return Ok(if op == ClassificationBinaryOp::Equal {
                        equal
                    } else {
                        RelationalBooleanFormula::Not(Box::new(equal))
                    });
                }
                self.normalize_integer_comparison(op, left_id, right_id)
            }
            ClassificationBinaryOp::LessThan
            | ClassificationBinaryOp::LessThanOrEqual
            | ClassificationBinaryOp::GreaterThan
            | ClassificationBinaryOp::GreaterThanOrEqual => {
                self.normalize_integer_comparison(op, left_id, right_id)
            }
            _ => {
                Err(self
                    .residual(RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator))
            }
        }
    }

    fn normalize_integer_comparison(
        &mut self,
        op: ClassificationBinaryOp,
        left_id: ClassificationNodeId,
        right_id: ClassificationNodeId,
    ) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
        if self.node(left_id)?.ty != self.scalar_types.integer
            || self.node(right_id)?.ty != self.scalar_types.integer
        {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator)
            );
        }
        let relation = match op {
            ClassificationBinaryOp::Equal => RelationalRelation::Equal,
            ClassificationBinaryOp::NotEqual => RelationalRelation::NotEqual,
            ClassificationBinaryOp::LessThan => RelationalRelation::Less,
            ClassificationBinaryOp::LessThanOrEqual => RelationalRelation::LessOrEqual,
            ClassificationBinaryOp::GreaterThan => RelationalRelation::Greater,
            ClassificationBinaryOp::GreaterThanOrEqual => RelationalRelation::GreaterOrEqual,
            _ => {
                return Err(self.residual(
                    RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator,
                ));
            }
        };
        let difference = self
            .normalize_integer(left_id)?
            .subtract(&self.normalize_integer(right_id)?)
            .map_err(|_| {
                self.residual(RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow)
            })?;
        Ok(RelationalBooleanFormula::Comparison {
            difference,
            relation,
        })
    }

    fn callable_argument(
        &self,
        callable_id: ClassificationCallableId,
        ordinal: u32,
    ) -> Result<RelationalNormalizedScalar, RelationalRegionExpressionResidual> {
        let frame = self.frames.last().ok_or_else(|| {
            self.residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)
        })?;
        if frame.callable_id != callable_id {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)
            );
        }
        frame
            .arguments
            .get(usize::try_from(ordinal).map_err(|_| {
                self.residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)
            })?)
            .cloned()
            .ok_or_else(|| {
                self.residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)
            })
    }

    fn normalize_arguments(
        &mut self,
        definition: &ClassificationCallableDefinition,
        arguments: &[ClassificationNodeId],
    ) -> Result<Box<[RelationalNormalizedScalar]>, RelationalRegionExpressionResidual> {
        if definition.parameter_types.len() != arguments.len() {
            return Err(
                self.residual(RelationalRegionExpressionResidualReason::InvalidCallableFrame)
            );
        }
        definition
            .parameter_types
            .iter()
            .copied()
            .zip(arguments.iter().copied())
            .map(|(parameter_type, argument)| {
                if parameter_type == self.scalar_types.integer {
                    self.normalize_integer(argument)
                        .map(RelationalNormalizedScalar::Integer)
                } else if parameter_type == self.scalar_types.boolean {
                    self.normalize_boolean(argument)
                        .map(RelationalNormalizedScalar::Boolean)
                } else {
                    Err(self
                        .residual(RelationalRegionExpressionResidualReason::UnsupportedScalarType))
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn enter_call(
        &mut self,
        callable_id: ClassificationCallableId,
        arguments: &[ClassificationNodeId],
    ) -> Result<ClassificationNodeId, RelationalRegionExpressionResidual> {
        if self.active_calls.contains(&callable_id) {
            return Err(self.residual(RelationalRegionExpressionResidualReason::RecursiveCallable));
        }
        let definition = self.callable(callable_id)?;
        let normalized = self.normalize_arguments(&definition, arguments)?;
        let body = definition.body;
        self.active_calls.push(callable_id);
        self.frames.push(RelationalCallableFrame {
            callable_id,
            arguments: normalized,
        });
        Ok(body)
    }

    fn leave_call(&mut self) {
        self.frames.pop();
        self.active_calls.pop();
    }

    fn normalize_integer_call(
        &mut self,
        callable_id: ClassificationCallableId,
        arguments: &[ClassificationNodeId],
    ) -> Result<RelationalQuasiAffine, RelationalRegionExpressionResidual> {
        let body = self.enter_call(callable_id, arguments)?;
        let result = self.normalize_integer(body);
        self.leave_call();
        result
    }

    fn normalize_boolean_call(
        &mut self,
        callable_id: ClassificationCallableId,
        arguments: &[ClassificationNodeId],
    ) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
        let body = self.enter_call(callable_id, arguments)?;
        let result = self.normalize_boolean(body);
        self.leave_call();
        result
    }
}

fn boolean_equivalence(
    left: RelationalBooleanFormula,
    right: RelationalBooleanFormula,
) -> RelationalBooleanFormula {
    RelationalBooleanFormula::Any(
        vec![
            RelationalBooleanFormula::All(vec![left.clone(), right.clone()].into_boxed_slice()),
            RelationalBooleanFormula::All(
                vec![
                    RelationalBooleanFormula::Not(Box::new(left)),
                    RelationalBooleanFormula::Not(Box::new(right)),
                ]
                .into_boxed_slice(),
            ),
        ]
        .into_boxed_slice(),
    )
}

fn formula_digest(formula: &RelationalBooleanFormula) -> [u8; 32] {
    let mut hasher = CanonicalProofHasher::new(FORMULA_DIGEST_V3);
    hasher.u32(RELATIONAL_REGION_PROOF_VERSION);
    hasher.formula(formula);
    hasher.finish()
}

fn derive_starter_region_id(artifact: &RelationalRegionProofArtifact) -> RelationalStarterRegionId {
    let mut hasher = CanonicalProofHasher::new(STARTER_REGION_ID_V1);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.replay_authority_id);
    hasher.digest(artifact.classification_capsule_id.bytes());
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.source_assignment_cell_id.bytes());
    hasher.digest(artifact.source_row_cell_id.bytes());
    hasher.digest(artifact.successor_coordinate_cell_id.bytes());
    hasher.digest(artifact.root_cell_id.bytes());
    match artifact.subject {
        RelationalRegionProofSubject::Root => hasher.u8(0x01),
        RelationalRegionProofSubject::CanonicalChunk {
            partition_artifact_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
        } => {
            hasher.u8(0x02);
            hasher.digest(partition_artifact_id.bytes());
            hasher.digest(chunk_id.bytes());
            hasher.u128(chunk_ordinal);
            hasher.digest(chunk_cell_id.bytes());
            hasher.digest(chunk_materializer_id.bytes());
        }
    }
    hasher.digest(artifact.axis_stage_id.bytes());
    hasher.digest(artifact.axis_dimension_id.bytes());
    hasher.digest(artifact.axis_cell_id.bytes());
    hasher.i64(artifact.value_start);
    hasher.i64(artifact.value_end_exclusive);
    hasher.u128(artifact.coordinate_start);
    hasher.u128(artifact.coordinate_end_exclusive);
    RelationalStarterRegionId(hasher.finish())
}

fn derive_certificate_id(artifact: &RelationalRegionProofArtifact) -> [u8; 32] {
    let mut hasher = CanonicalProofHasher::new(CERTIFICATE_ID_V3);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.replay_authority_id);
    hasher.digest(artifact.classification_capsule_id.bytes());
    hasher.digest(artifact.successor_root_id.bytes());
    hasher.digest(artifact.find_root_id.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.question_id.bytes());
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.root_cell_id.bytes());
    match artifact.subject {
        RelationalRegionProofSubject::Root => hasher.u8(0x01),
        RelationalRegionProofSubject::CanonicalChunk {
            partition_artifact_id,
            chunk_id,
            chunk_ordinal,
            chunk_cell_id,
            chunk_materializer_id,
        } => {
            hasher.u8(0x02);
            hasher.digest(partition_artifact_id.bytes());
            hasher.digest(chunk_id.bytes());
            hasher.u128(chunk_ordinal);
            hasher.digest(chunk_cell_id.bytes());
            hasher.digest(chunk_materializer_id.bytes());
        }
    }
    hasher.u8(artifact.conclusion.canonical_tag());
    hasher.digest(artifact.starter_region_id.bytes());
    hasher.digest(artifact.source_assignment_cell_id.bytes());
    hasher.digest(artifact.source_row_cell_id.bytes());
    hasher.digest(artifact.successor_coordinate_cell_id.bytes());
    hasher.digest(artifact.axis_stage_id.bytes());
    hasher.digest(artifact.axis_dimension_id.bytes());
    hasher.digest(artifact.axis_cell_id.bytes());
    hasher.i64(artifact.value_start);
    hasher.i64(artifact.value_end_exclusive);
    hasher.u128(artifact.coordinate_start);
    hasher.u128(artifact.coordinate_end_exclusive);
    hasher.u128(artifact.case_cardinality);
    hasher.digest(artifact.selected_formula_digest);
    hasher.finish()
}

fn evidence_binding(
    certificate_id: [u8; 32],
    role: RelationalRegionEvidenceRole,
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
) -> RelationalRegionEvidenceBinding {
    let mut hasher = CanonicalProofHasher::new(PROOF_DIGEST_V3);
    hasher.u32(RELATIONAL_REGION_PROOF_VERSION);
    hasher.digest(certificate_id);
    hasher.u8(role.tag());
    hasher.digest(obligation_id.bytes());
    hasher.digest(conclusion_digest);
    RelationalRegionEvidenceBinding {
        obligation_id,
        conclusion_digest,
        proof_digest: hasher.finish(),
    }
}

struct CanonicalProofHasher(Sha256);

impl CanonicalProofHasher {
    fn new(domain: &[u8]) -> Self {
        let mut value = Self(Sha256::new());
        value.bytes(domain);
        value
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.bytes(&value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_be_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.bytes(&value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes(&value.to_be_bytes());
    }

    fn quasi_affine(&mut self, value: &RelationalQuasiAffine) {
        self.i128(value.affine.coefficient);
        self.i128(value.affine.intercept);
        self.u128(value.terms.len() as u128);
        for term in value.terms.iter() {
            self.i128(term.numerator.coefficient);
            self.i128(term.numerator.intercept);
            self.i64(term.positive_divisor);
            self.i128(term.coefficient);
        }
    }

    fn formula(&mut self, formula: &RelationalBooleanFormula) {
        match formula {
            RelationalBooleanFormula::Constant(value) => {
                self.u8(0x01);
                self.u8(u8::from(*value));
            }
            RelationalBooleanFormula::Comparison {
                difference,
                relation,
            } => {
                self.u8(0x02);
                self.quasi_affine(difference);
                self.u8(relation.tag());
            }
            RelationalBooleanFormula::Not(inner) => {
                self.u8(0x03);
                self.formula(inner);
            }
            RelationalBooleanFormula::All(parts) => {
                self.u8(0x04);
                self.u128(parts.len() as u128);
                for part in parts.iter() {
                    self.formula(part);
                }
            }
            RelationalBooleanFormula::Any(parts) => {
                self.u8(0x05);
                self.u128(parts.len() as u128);
                for part in parts.iter() {
                    self.formula(part);
                }
            }
        }
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRegionProofError {
    InvalidQuery(String),
    QuestionArityMismatch { actual: usize },
    CheckedPlanScopeMismatch,
    InvalidCheckedProgramDigest,
    InvalidCheckedProvenanceDigest,
    ClassificationCapsuleScopeMismatch,
    ReplayAuthorityMismatch,
    PartitionScopeMismatch,
    CanonicalChunkMissing,
    CanonicalChunkMismatch,
    ClassificationIndexOverflow(&'static str),
    ClassificationNodeMissing(ClassificationNodeId),
    ClassificationCallableMissing(ClassificationCallableId),
    UnsupportedArtifactVersion(u32),
    InvalidArtifactShape,
    StarterRegionIdentityMismatch,
    ArtifactIdentityMismatch,
    ArtifactSemanticMismatch,
    ArtifactNoLongerProvable(RelationalRegionProofResidual),
    ProofStrategy(RelationalProofStrategyError),
    SupportCell(SupportCellError),
    RootCellMissing(SupportCellId),
    ExpectedNonemptyRoot,
    RootObligationMissing(RelationalRegionEvidenceRole),
    RootObligationCellMismatch,
    ArithmeticOverflow(&'static str),
    InternalProofInvariant(&'static str),
}

impl From<RelationalProofStrategyError> for RelationalRegionProofError {
    fn from(value: RelationalProofStrategyError) -> Self {
        Self::ProofStrategy(value)
    }
}

impl From<SupportCellError> for RelationalRegionProofError {
    fn from(value: SupportCellError) -> Self {
        Self::SupportCell(value)
    }
}

impl fmt::Display for RelationalRegionProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(formatter, "invalid relational query: {message}"),
            Self::QuestionArityMismatch { actual } => write!(
                formatter,
                "relational region optimization requires exactly one semantic question, found {actual}"
            ),
            Self::CheckedPlanScopeMismatch => formatter.write_str(
                "checked query and relational support plan have different semantic scope",
            ),
            Self::InvalidCheckedProgramDigest => formatter
                .write_str("checked query program identity is not a canonical SHA-256 digest"),
            Self::InvalidCheckedProvenanceDigest => formatter.write_str(
                "checked query source-coverage identity is not a canonical SHA-256 digest",
            ),
            Self::ClassificationCapsuleScopeMismatch => formatter.write_str(
                "classification capsule does not match the checked query and support scope",
            ),
            Self::ReplayAuthorityMismatch => formatter.write_str(
                "relational region proof does not match the producer-owned replay authority",
            ),
            Self::PartitionScopeMismatch => formatter.write_str(
                "relational region proof partition does not match its checked support scope",
            ),
            Self::CanonicalChunkMissing => formatter
                .write_str("relational region proof names no canonical partition child"),
            Self::CanonicalChunkMismatch => formatter.write_str(
                "relational region proof canonical partition child has changed identity",
            ),
            Self::ClassificationIndexOverflow(context) => {
                write!(formatter, "classification index overflow for {context}")
            }
            Self::ClassificationNodeMissing(_) => formatter
                .write_str("classification capsule graph is missing a referenced node"),
            Self::ClassificationCallableMissing(_) => formatter
                .write_str("classification capsule graph is missing a referenced callable"),
            Self::UnsupportedArtifactVersion(version) => write!(
                formatter,
                "relational region proof artifact version {version} is unsupported"
            ),
            Self::InvalidArtifactShape => {
                formatter.write_str("relational region proof artifact has an invalid shape")
            }
            Self::StarterRegionIdentityMismatch => formatter.write_str(
                "relational region proof does not preserve its correlated starter-region identity",
            ),
            Self::ArtifactIdentityMismatch => formatter
                .write_str("relational region proof artifact identity does not match its payload"),
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "relational region proof artifact does not match its checked query, support plan, and classification capsule",
            ),
            Self::ArtifactNoLongerProvable(residual) => write!(
                formatter,
                "relational region proof artifact now requires concrete fallback: {residual:?}"
            ),
            Self::ProofStrategy(error) => write!(formatter, "invalid proof strategy: {error}"),
            Self::SupportCell(error) => write!(formatter, "invalid support evidence: {error}"),
            Self::RootCellMissing(_) => {
                formatter.write_str("relational region proof root cell is absent from its plan")
            }
            Self::ExpectedNonemptyRoot => {
                formatter.write_str("relational region proof expected a nonempty cell-backed root")
            }
            Self::RootObligationMissing(role) => {
                write!(
                    formatter,
                    "relational region proof is missing its {role:?} obligation"
                )
            }
            Self::RootObligationCellMismatch => formatter
                .write_str("relational region proof obligations do not belong to the case root"),
            Self::ArithmeticOverflow(context) => {
                write!(
                    formatter,
                    "relational region proof arithmetic overflow while {context}"
                )
            }
            Self::InternalProofInvariant(message) => formatter.write_str(message),
        }
    }
}

impl Error for RelationalRegionProofError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProofStrategy(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::relational_analysis_plan::RelationalAnalysisPlan;
    use super::super::relational_bounded_chunk_partition::{
        plan_relational_bounded_case_chunks, reverify_relational_case_chunk_partition_artifact,
        RelationalCaseChunkPlanningOutcome,
    };
    use super::super::relational_case_support_projection::{
        derive_relational_case_support_projection, RelationalCaseSupportCount,
        RelationalCaseSupportProjectionRecord,
    };
    use super::super::relational_journal::{
        RelationalClassifiedSupportFragment, RelationalJournal, RelationalJournalContract,
        RelationalJournalError, RelationalJournalEvent,
    };
    use super::super::relational_journal_codec::{
        decode_relational_journal_entry, encode_relational_journal_entry,
        RelationalJournalCodecLimits,
    };
    use super::super::relational_support_planner::{
        prove_relational_case_image_injectivity, RelationalBindingStage, RelationalSupportPlanner,
    };
    use super::*;
    use crate::{Lexer, Parser, TypeChecker};

    const CAPSULE_EXACT_EMPTY_SOURCE: &str = r#"
> bump(x: Int) -> Int { x + 1 }

? explore capsule_exact_empty {
    from {
        vary before in range(0, 2)
        given context = ()
    }

    transition after = bump(before)
    find cliffs = matches of after < before
}
"#;

    const CAPSULE_VIOLATIONS_EMPTY_SOURCE: &str = r#"
> bump(x: Int) -> Int { x + 1 }

? explore capsule_violations_empty {
    from {
        vary before in range(0, 2)
        given context = ()
    }

    transition after = bump(before)
    find cliffs = violations of after >= before
}
"#;

    const CAPSULE_PARTITIONED_EMPTY_SOURCE: &str = r#"
> bump(x: Int) -> Int { x + 1 }

? explore capsule_partitioned_empty {
    from {
        vary before in range(0, 300)
        given context = ()
    }

    transition after = bump(before)
    find cliffs = matches of after < before
}
"#;

    fn bind_fixture_capsule(
        checked: &CheckedExploreQueryView<'_>,
        support_plan: &RelationalSupportPlan,
        specialization: ClassificationSpecializationRoot,
    ) -> RelationalClassificationCapsule {
        let checked_program = decode_lowercase_sha256(checked.program_hash())
            .expect("checked program identity is canonical lowercase SHA-256");
        let provenance_digest =
            decode_lowercase_sha256(checked.source_coverage().manifest_digest.as_ref())
                .expect("checked source-coverage identity is canonical lowercase SHA-256");
        RelationalClassificationCapsule::bind(
            checked.classification_program(),
            checked.classification_runtime_shapes(),
            checked_program,
            checked.relation_id(),
            checked.admission_id(),
            FrozenClassificationQuestionSet::freeze(checked.question_ids().iter().copied())
                .expect("freeze fixture question set"),
            support_plan.root(),
            support_plan.root_cell_id(),
            specialization,
            ClassificationProvenanceRoot::from_checked_source_coverage_digest(provenance_digest),
        )
        .expect("bind the checked fixture classification capsule")
    }

    #[test]
    fn checked_capsule_closes_exact_empty_region_and_reverify_rejects_scope_drift() {
        let mut lexer = Lexer::new(CAPSULE_EXACT_EMPTY_SOURCE);
        let statements = Parser::new(lexer.tokenize(), CAPSULE_EXACT_EMPTY_SOURCE)
            .parse_program()
            .expect("parse exact-empty capsule fixture");
        let artifacts = TypeChecker::check_with_explore_artifacts(
            &statements,
            None,
            CAPSULE_EXACT_EMPTY_SOURCE,
        );
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join the checked exact-empty query");
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact support for the two-case fixture");
        let capsule = bind_fixture_capsule(
            &checked,
            &support_plan,
            ClassificationSpecializationRoot::none(),
        );

        let outcome = prove_relational_exact_empty_region(&checked, &support_plan, &capsule)
            .expect("prove the exact-empty region from the checked capsule");
        let closure = outcome
            .exact_empty()
            .expect("the bump relation is uniformly not selected");
        assert_eq!(closure.proof().case_cardinality(), 2);
        assert_eq!(closure.selected_cardinality(), 0);

        let events = closure.events();
        assert_eq!(events.len(), 6);
        assert!(matches!(
            &events[0],
            SupportJournalEvent::EvidenceAccepted {
                evidence: SupportEvidenceRecord::Cardinality(evidence),
                ..
            } if *evidence.conclusion() == 2
        ));
        assert!(matches!(
            &events[1],
            SupportJournalEvent::EvidenceAccepted {
                evidence: SupportEvidenceRecord::Admission(evidence),
                ..
            } if *evidence.conclusion() == AdmissionDecision::Admitted
        ));
        assert!(matches!(
            &events[2],
            SupportJournalEvent::EvidenceAccepted {
                evidence: SupportEvidenceRecord::Selection(evidence),
                ..
            } if *evidence.conclusion() == SelectionDecision::NotSelected
        ));
        assert_eq!(
            events[3],
            SupportJournalEvent::leaf_sealed(closure.proof().root_cell_id())
        );
        assert_eq!(events[4], SupportJournalEvent::ObligationFrontierSealed);
        assert_eq!(events[5], SupportJournalEvent::CatalogSealed);

        let reverified = reverify_relational_region_proof_artifact(
            closure.proof().artifact(),
            &checked,
            &support_plan,
            &capsule,
        )
        .expect("the same checked capsule reproduces the proof closure");
        assert_eq!(&reverified, closure);

        let changed_specialization = bind_fixture_capsule(
            &checked,
            &support_plan,
            ClassificationSpecializationRoot::from_exact_witness_digest([0x5a; 32]),
        );
        assert_ne!(changed_specialization.id(), capsule.id());
        assert_eq!(
            reverify_relational_region_proof_artifact(
                closure.proof().artifact(),
                &checked,
                &support_plan,
                &changed_specialization,
            ),
            Err(RelationalRegionProofError::ArtifactSemanticMismatch)
        );
    }

    #[test]
    fn checked_capsule_consumes_already_normalized_violations_polarity_once() {
        let mut lexer = Lexer::new(CAPSULE_VIOLATIONS_EMPTY_SOURCE);
        let statements = Parser::new(lexer.tokenize(), CAPSULE_VIOLATIONS_EMPTY_SOURCE)
            .parse_program()
            .expect("parse violations-polarity capsule fixture");
        let artifacts = TypeChecker::check_with_explore_artifacts(
            &statements,
            None,
            CAPSULE_VIOLATIONS_EMPTY_SOURCE,
        );
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join the checked violations query");
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact support for the violations fixture");
        let capsule = bind_fixture_capsule(
            &checked,
            &support_plan,
            ClassificationSpecializationRoot::none(),
        );

        let outcome = prove_relational_exact_empty_region(&checked, &support_plan, &capsule)
            .expect("prove the normalized violations region");
        let closure = outcome
            .exact_empty()
            .expect("the already-negated violations lane is uniformly not selected");
        assert_eq!(closure.proof().case_cardinality(), 2);
        assert_eq!(closure.selected_cardinality(), 0);
    }

    #[test]
    fn canonical_child_certificate_preserves_correlated_starter_chain() {
        let mut lexer = Lexer::new(CAPSULE_PARTITIONED_EMPTY_SOURCE);
        let statements = Parser::new(lexer.tokenize(), CAPSULE_PARTITIONED_EMPTY_SOURCE)
            .parse_program()
            .expect("parse partitioned exact-empty capsule fixture");
        let artifacts = TypeChecker::check_with_explore_artifacts(
            &statements,
            None,
            CAPSULE_PARTITIONED_EMPTY_SOURCE,
        );
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join the checked partitioned query");
        let question_id = checked.question_ids()[0];
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact support for the partitioned fixture");
        let capsule = Arc::new(bind_fixture_capsule(
            &checked,
            &support_plan,
            ClassificationSpecializationRoot::none(),
        ));
        let case_image = prove_relational_case_image_injectivity(&support_plan)
            .expect("prove the fixture's singleton case-image chain");
        let partition = match plan_relational_bounded_case_chunks(&support_plan, &case_image)
            .expect("plan canonical case children")
        {
            RelationalCaseChunkPlanningOutcome::Partitioned(partition) => partition,
            outcome => panic!("expected a bounded partition, found {outcome:?}"),
        };
        let verified_partition = reverify_relational_case_chunk_partition_artifact(
            partition.artifact(),
            &support_plan,
            case_image.injectivity(),
        )
        .expect("reverify the canonical case partition");
        let authority = Arc::new(
            RelationalRegionReplayAuthority::new(
                Arc::new(checked.to_owned_checked_query()),
                support_plan.clone(),
                Arc::clone(&capsule),
            )
            .expect("bind producer-owned regional replay authority"),
        );

        let outcome = authority
            .prove_canonical_child(&verified_partition, 0)
            .expect("prove the first canonical child");
        let closure = outcome
            .exact_empty()
            .expect("the first child is uniformly not selected");
        let proof = closure.proof();
        let artifact = proof.artifact();
        let child = &verified_partition.partition().chunks()[0];
        let RelationalBindingStage::Finite(axis_stage) = &support_plan.stages()[0] else {
            panic!("the first fixture binding must be its finite starter axis");
        };

        assert_eq!(artifact.case_cardinality(), 256);
        assert_eq!(artifact.axis_cell_id(), axis_stage.cell().unwrap().id());
        assert_ne!(artifact.axis_cell_id(), child.cell().id());
        assert_eq!(
            artifact.source_assignment_cell_id(),
            support_plan.source_assignments().cell().unwrap().id()
        );
        assert_eq!(
            artifact.source_row_cell_id(),
            support_plan.source_rows().cell().unwrap().id()
        );
        assert_eq!(
            artifact.successor_coordinate_cell_id(),
            support_plan.successor_coordinates().cell().unwrap().id()
        );
        assert_ne!(artifact.starter_region_id().bytes(), [0; 32]);
        assert_eq!(closure.events().len(), 4);
        assert_eq!(
            authority
                .reverify_canonical_child(artifact, &verified_partition)
                .expect("same authority reproduces the child theorem")
                .artifact(),
            artifact
        );

        let analysis_plan = RelationalAnalysisPlan::from_checked(&checked)
            .expect("plan analysis for the regional journal fixture");
        let contract = RelationalJournalContract::new(
            checked.relation_id(),
            checked.admission_id(),
            checked.question_ids().iter().copied(),
            checked.transition_schemas().state_schema_id(),
            checked.transition_schemas().context_schema_id(),
            checked.transition_schemas().transition_type_id(),
            analysis_plan.producer_graph_digest().bytes(),
        );
        let make_ready_journal = |authority: Arc<RelationalRegionReplayAuthority>| {
            let mut journal =
                RelationalJournal::new_with_region_replay_authority(contract.clone(), authority);
            journal
                .append(RelationalJournalEvent::analysis_plan_registered(
                    analysis_plan.clone(),
                ))
                .expect("register analysis plan");
            journal
                .append(RelationalJournalEvent::support_plan_registered(
                    support_plan.clone(),
                ))
                .expect("register support plan");
            journal
                .append(
                    RelationalJournalEvent::relational_case_image_injectivity_proof_accepted(
                        case_image.proof().artifact().clone(),
                    ),
                )
                .expect("accept root case-image proof");
            journal
                .append(
                    RelationalJournalEvent::relational_case_chunk_partition_accepted(
                        partition.artifact().clone(),
                    ),
                )
                .expect("accept canonical child partition");
            journal
        };

        let mut journal = make_ready_journal(Arc::clone(&authority));
        journal
            .append(RelationalJournalEvent::relational_region_proof_accepted(
                artifact.clone(),
            ))
            .expect("atomically accept the regional child theorem");
        let view = journal
            .scheduler_view()
            .expect("inspect accepted proof prefix");
        let classified_support_fragments = view
            .classified_support_fragments()
            .expect("read classified support fragments");
        assert_eq!(classified_support_fragments.len(), 1);
        assert!(matches!(
            &classified_support_fragments[0],
            RelationalClassifiedSupportFragment::CertifiedZeroSelected(retained)
                if retained == artifact
        ));
        assert_eq!(
            view.classified_sweep_progress()
                .expect("read classified sweep progress")
                .expect("partition owns classified progress")
                .next_chunk_ordinal(),
            1
        );
        let projection = derive_relational_case_support_projection(
            question_id,
            &verified_partition,
            classified_support_fragments,
            |_| None,
            None,
            None,
        )
        .expect("project the certified child without concrete cases");
        assert_eq!(
            projection.metadata().classified_case_count,
            RelationalCaseSupportCount::LowerBound(256)
        );
        assert_eq!(
            projection.metadata().selected_case_count,
            RelationalCaseSupportCount::LowerBound(0)
        );
        assert_eq!(projection.available_source_record_count(), 3);
        assert!(matches!(
            projection
                .record_at(2)
                .expect("read the certified public region"),
            Some(RelationalCaseSupportProjectionRecord::Region {
                exact_case_count: 256,
                correlated_starter_region_id: Some(starter_region_id),
                ..
            }) if starter_region_id == artifact.starter_region_id()
        ));

        let entries = journal.entries().to_vec();
        assert!(matches!(
            RelationalJournal::replay(contract.clone(), entries.clone()),
            Err(RelationalJournalError::RegionProofReplayAuthorityMissing)
        ));
        let mut wrong_authority = (*authority).clone();
        wrong_authority.id[0] ^= 0xff;
        assert!(matches!(
            RelationalJournal::replay_with_region_replay_authority(
                contract.clone(),
                entries.clone(),
                Arc::new(wrong_authority),
            ),
            Err(RelationalJournalError::RegionProof(_))
        ));

        let limits = RelationalJournalCodecLimits::default();
        let decoded = entries
            .iter()
            .map(|entry| {
                let bytes = encode_relational_journal_entry(entry, limits)
                    .expect("encode canonical journal entry");
                decode_relational_journal_entry(
                    contract.clone(),
                    entry.sequence(),
                    entry.previous(),
                    &bytes,
                    limits,
                )
                .expect("decode canonical journal entry")
            })
            .collect::<Vec<_>>();
        assert_eq!(decoded, entries);
        let replayed = RelationalJournal::replay_with_region_replay_authority(
            contract.clone(),
            decoded,
            Arc::clone(&authority),
        )
        .expect("cold replay reproduces the certified prefix");
        assert_eq!(replayed.head(), journal.head());
        assert_eq!(
            replayed.snapshot().expect("snapshot replay").support(),
            journal.snapshot().expect("snapshot original").support()
        );

        let mut tampered = artifact.clone();
        tampered.selected_formula_digest[0] ^= 0x80;
        let mut tampered_journal = make_ready_journal(Arc::clone(&authority));
        let head_before = tampered_journal.head();
        assert!(matches!(
            tampered_journal.append(RelationalJournalEvent::relational_region_proof_accepted(
                tampered,
            )),
            Err(RelationalJournalError::RegionProof(
                RelationalRegionProofError::ArtifactIdentityMismatch
            ))
        ));
        assert_eq!(tampered_journal.head(), head_before);
        let view = tampered_journal
            .scheduler_view()
            .expect("inspect failed append state");
        assert!(view
            .classified_support_fragments()
            .expect("read classified support fragments")
            .is_empty());
        assert_eq!(
            view.classified_sweep_progress()
                .expect("read classified sweep progress")
                .expect("partition progress remains installed")
                .next_chunk_ordinal(),
            0
        );
    }
}
