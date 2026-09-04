//! Logical, factorized support planning for checked relational Explore queries.
//!
//! This layer turns one immutable [`CheckedExploreQueryView`] into a compact
//! support DAG. It never enumerates a Cartesian prefix. Every finite `FROM`
//! binding becomes one fiber factor, while singleton bindings remain
//! deterministic maps with cardinality multiplier one. Runtime-dependent
//! fibers are represented by open [`SupportExpr::join_reference`] cells and an
//! exact dependency-key recipe; they are never disguised as exact products.
//! Statically empty factors are represented as terminal logical emptiness, not
//! as illegal empty cells or open joins, and zero propagates through every
//! downstream population.
//!
//! The concrete source and successor executors remain the canonical fallback
//! materializers. The planning pass describes what they must materialize and
//! never silently promotes a mapped image. A separate producer-chain verifier
//! below can subsequently replay the recognized case-image contracts and
//! issue injectivity (and, for one exact singleton specialization, mapped-image
//! cardinality) through a narrow proof gateway. Classification and mechanism
//! claims remain separate. Every minted cell is returned in one canonical
//! registration catalog so a journal cannot register a root while omitting
//! cells named by its recipes.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
use super::relational_ir::{
    ExploreFiniteDomainIr, ExploreQueryIr, ExploreSourceBindingKindIr, ExploreSourceBindingRoleIr,
    ExploreSuccessorKindIr,
};
use super::relational_source_image_exactness::{
    prove_relational_source_image_exactness, CertifiedSourcePopulationRoot,
    RelationalSourceImageExactnessProofError,
};
use super::support_cell::{
    AdmissionClassificationClaim, CertifiedInjective, ExactCardinalityClaim, InjectiveMappingClaim,
    SupportCell, SupportCellClaim, SupportCellError, SupportCellEvidence, SupportCellEvidenceId,
    SupportCellId, SupportCellObligation, SupportCellSpace, SupportExpr, SupportExprKind,
    SupportExtensionalTarget, SupportMaterializerId, SupportProducerId, SupportProofObligationId,
};
use super::support_evidence::SupportObligationRecord;
use super::{ExploreCardinality, ExploreExactDomain};
use crate::{
    CheckedExploreCoverageClassification, CheckedExploreQueryView,
    CheckedExploreSourceCoverageManifest, CheckedExploreSourceImageProjectionCertificate,
    CheckedExploreSourceProjectionEndpoint, CheckedExploreSourceProjectionFactorKind,
    CheckedExploreSourceProjectionWitnessKind, ExploreAdmissionScope, Expr, ExprKind, Literal,
};

pub(crate) const RELATIONAL_SUPPORT_PLANNER_VERSION: u32 = 3;
pub(crate) const RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION: u32 = 1;

const BINDING_STAGE_ID_V1: &[u8] = b"futuruna.explore.relational-support.binding-stage.v1";
const DIMENSION_ID_V1: &[u8] = b"futuruna.explore.relational-support.dimension.v1";
const FACTOR_PRODUCER_V1: &[u8] = b"futuruna.explore.relational-support.factor-producer.v1";
const ASSIGNMENT_PRODUCER_V1: &[u8] = b"futuruna.explore.relational-support.assignment-producer.v1";
const SUCCESSOR_PRODUCER_V1: &[u8] = b"futuruna.explore.relational-support.successor-producer.v1";
const FACTOR_MATERIALIZER_V1: &[u8] = b"futuruna.explore.relational-support.factor-materializer.v1";
const ASSIGNMENT_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.assignment-materializer.v1";
const SOURCE_IMAGE_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.source-image-materializer.v1";
const SUCCESSOR_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.successor-materializer.v1";
const COMPOSED_SINGLETON_SUCCESSOR_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.composed-singleton-successor-materializer.v1";
const CASE_IMAGE_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.case-image-materializer.v1";
const SUPPORT_PLAN_ROOT_V2: &[u8] = b"futuruna.explore.relational-support.plan-root.v2";
const CASE_IMAGE_INJECTIVITY_CERTIFICATE_V1: &[u8] =
    b"futuruna.explore.relational-support.case-image-injectivity-certificate.v1";
const CASE_IMAGE_INJECTIVITY_PROOF_V1: &[u8] =
    b"futuruna.explore.relational-support.case-image-injectivity-proof.v1";
const CASE_IMAGE_INJECTIVITY_CERTIFICATE_V2: &[u8] =
    b"futuruna.explore.relational-support.case-image-injectivity-certificate.v2";
const CASE_IMAGE_INJECTIVITY_PROOF_V2: &[u8] =
    b"futuruna.explore.relational-support.case-image-injectivity-proof.v2";

pub(crate) const RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION_V1: u32 = 1;
pub(crate) const RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION: u32 = 2;

/// Canonical identity of one complete logical support plan.
///
/// The root commits semantic identities, factorization, recipes, exactness,
/// coverage qualification, the complete cell catalog, and staged root work.
/// Authored names, spans, runtime resources, and scheduling choices never
/// enter its preimage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalSupportPlanRoot([u8; 32]);

impl RelationalSupportPlanRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of one logical `FROM` binding stage.
///
/// Authored names and source locations are deliberately absent. The checked
/// relation, canonical binding position, binding kind, role, and resolved
/// dependency positions determine the identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalBindingStageId([u8; 32]);

impl RelationalBindingStageId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable identity of the dimension introduced by one finite binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalDimensionId([u8; 32]);

impl RelationalDimensionId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// How a worker reconstructs the smallest environment that can affect one
/// binding expression.
///
/// A physical cache hashes the canonical values at these binding positions.
/// Unrelated earlier prefix values are intentionally excluded, allowing one
/// opened fiber to be reused across every prefix with the same declared
/// dependency tuple.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalDependencyKeyRecipe {
    binding_indices: Box<[u32]>,
    binding_stage_ids: Box<[RelationalBindingStageId]>,
}

impl RelationalDependencyKeyRecipe {
    pub(super) fn restore_from_journal_codec(
        binding_indices: Box<[u32]>,
        binding_stage_ids: Box<[RelationalBindingStageId]>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        if binding_indices.len() != binding_stage_ids.len()
            || binding_indices.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "dependency-key positions are not canonical",
            ));
        }
        Ok(Self {
            binding_indices,
            binding_stage_ids,
        })
    }

    pub(crate) fn binding_indices(&self) -> &[u32] {
        &self.binding_indices
    }

    pub(crate) fn binding_stage_ids(&self) -> &[RelationalBindingStageId] {
        &self.binding_stage_ids
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.binding_indices.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalFiniteDomainRecipeKind {
    CheckedExact,
    CheckedCollection,
    CheckedIntRange,
}

/// Logical exactness of a planned population before any accepted proof is
/// attached. Positive `StructuralExact` counts are reflected by a cell's own
/// support expression; zero belongs to [`RelationalPlannedSupport::ExactEmpty`]
/// and has no cell. `Open` is never promoted by this planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportExactness {
    StructuralExact(u128),
    Open {
        confirmed_lower_bound: u128,
        reason: RelationalSupportOpenReason,
    },
}

impl RelationalSupportExactness {
    pub(crate) const fn exact(self) -> Option<u128> {
        match self {
            Self::StructuralExact(value) => Some(value),
            Self::Open { .. } => None,
        }
    }

    pub(crate) const fn lower_bound(self) -> u128 {
        match self {
            Self::StructuralExact(value) => value,
            Self::Open {
                confirmed_lower_bound,
                ..
            } => confirmed_lower_bound,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportOpenReason {
    RuntimeDomain,
    DependentFiberJoin,
    NaturalJoin,
    CoordinateCardinalityExceedsU128,
    CoordinateCardinalityOverflow,
    MappedImageNeedsEvidence,
    SuccessorFiberSum,
}

/// Why a logical factor or population is already known to be empty.
///
/// Exact emptiness is represented outside [`SupportCell`]. Support cells are
/// materialization work units and their algebra intentionally rejects empty
/// expressions. Keeping the two states disjoint prevents an exact empty set
/// from being weakened into an open join merely to obtain a cell identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalExactEmptyReason {
    StaticFiniteDomain {
        stage_id: RelationalBindingStageId,
    },
    EmptyDependencyKeySpace {
        stage_id: RelationalBindingStageId,
        empty_input_dimension: RelationalDimensionId,
    },
    EmptyAssignmentFactor {
        stage_id: RelationalBindingStageId,
    },
    StaticSuccessorDomain,
    UpstreamPopulation(RelationalSupportPopulationKind),
}

/// A logical support set is either a materializable cell or statically exact
/// empty. The `Cell` variant does not assert non-emptiness: an open cell may
/// still materialize to zero. It only means emptiness is not already known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalPlannedSupport {
    Cell {
        cell: SupportCell,
        exactness: RelationalSupportExactness,
    },
    ExactEmpty {
        reason: RelationalExactEmptyReason,
    },
}

impl RelationalPlannedSupport {
    pub(super) fn restore_from_journal_codec(
        cell: Option<SupportCell>,
        exactness: RelationalSupportExactness,
        exact_empty_reason: Option<RelationalExactEmptyReason>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        match (cell, exact_empty_reason) {
            (Some(cell), None) => Self::cell(cell, exactness),
            (None, Some(reason)) if exactness == RelationalSupportExactness::StructuralExact(0) => {
                Ok(Self::exact_empty(reason))
            }
            _ => Err(RelationalSupportPlannerError::PlanInvariant(
                "planned support wire shape is not canonical",
            )),
        }
    }

    fn cell(
        cell: SupportCell,
        exactness: RelationalSupportExactness,
    ) -> Result<Self, RelationalSupportPlannerError> {
        if exactness.exact() == Some(0) {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "exact cardinality zero must use exact-empty support",
            ));
        }
        if exactness.lower_bound() != cell.cardinality().lower_bound()
            || exactness.exact() != cell.cardinality().exact()
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "planned support exactness disagrees with its cell",
            ));
        }
        Ok(Self::Cell { cell, exactness })
    }

    const fn exact_empty(reason: RelationalExactEmptyReason) -> Self {
        Self::ExactEmpty { reason }
    }

    pub(crate) const fn cell_ref(&self) -> Option<&SupportCell> {
        match self {
            Self::Cell { cell, .. } => Some(cell),
            Self::ExactEmpty { .. } => None,
        }
    }

    pub(crate) const fn exactness(&self) -> RelationalSupportExactness {
        match self {
            Self::Cell { exactness, .. } => *exactness,
            Self::ExactEmpty { .. } => RelationalSupportExactness::StructuralExact(0),
        }
    }

    pub(crate) const fn exact_empty_reason(&self) -> Option<RelationalExactEmptyReason> {
        match self {
            Self::Cell { .. } => None,
            Self::ExactEmpty { reason } => Some(*reason),
        }
    }

    pub(crate) const fn is_exact_empty(&self) -> bool {
        matches!(self, Self::ExactEmpty { .. })
    }
}

/// Canonical dimension schema of one finite factor. `key_dimensions` are the
/// varied coordinates capable of changing its domain. `output_dimension` is
/// the new value selected from that fiber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalFactorSchema {
    key_dimensions: Box<[RelationalDimensionId]>,
    output_dimension: RelationalDimensionId,
}

impl RelationalFactorSchema {
    pub(super) fn restore_from_journal_codec(
        key_dimensions: Box<[RelationalDimensionId]>,
        output_dimension: RelationalDimensionId,
    ) -> Self {
        Self {
            key_dimensions,
            output_dimension,
        }
    }

    pub(crate) fn key_dimensions(&self) -> &[RelationalDimensionId] {
        &self.key_dimensions
    }

    pub(crate) const fn output_dimension(&self) -> RelationalDimensionId {
        self.output_dimension
    }
}

/// Executable recipe for one finite `FROM` fiber. The checked query remains
/// the expression authority; the recipe carries only stable positional keys
/// and the support/materializer identities needed to resume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalFiniteFactorRecipe {
    binding_index: u32,
    dependency_key: RelationalDependencyKeyRecipe,
    domain_kind: RelationalFiniteDomainRecipeKind,
    producer_id: SupportProducerId,
    materializer_id: SupportMaterializerId,
    /// Exact set-normalized local fiber cardinality when it is statically
    /// available. This is not a global dependent-join count.
    known_local_cardinality: Option<u128>,
}

impl RelationalFiniteFactorRecipe {
    pub(super) fn restore_from_journal_codec(
        binding_index: u32,
        dependency_key: RelationalDependencyKeyRecipe,
        domain_kind: RelationalFiniteDomainRecipeKind,
        producer_id: SupportProducerId,
        materializer_id: SupportMaterializerId,
        known_local_cardinality: Option<u128>,
    ) -> Self {
        Self {
            binding_index,
            dependency_key,
            domain_kind,
            producer_id,
            materializer_id,
            known_local_cardinality,
        }
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        self.binding_index
    }

    pub(crate) const fn dependency_key(&self) -> &RelationalDependencyKeyRecipe {
        &self.dependency_key
    }

    pub(crate) const fn domain_kind(&self) -> RelationalFiniteDomainRecipeKind {
        self.domain_kind
    }

    pub(crate) const fn producer_id(&self) -> SupportProducerId {
        self.producer_id
    }

    pub(crate) const fn materializer_id(&self) -> SupportMaterializerId {
        self.materializer_id
    }

    pub(crate) const fn known_local_cardinality(&self) -> Option<u128> {
        self.known_local_cardinality
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalFiniteFactorStage {
    stage_id: RelationalBindingStageId,
    role: ExploreSourceBindingRoleIr,
    dimension_id: RelationalDimensionId,
    schema: RelationalFactorSchema,
    support: RelationalPlannedSupport,
    recipe: RelationalFiniteFactorRecipe,
}

impl RelationalFiniteFactorStage {
    pub(super) fn restore_from_journal_codec(
        stage_id: RelationalBindingStageId,
        role: ExploreSourceBindingRoleIr,
        dimension_id: RelationalDimensionId,
        schema: RelationalFactorSchema,
        support: RelationalPlannedSupport,
        recipe: RelationalFiniteFactorRecipe,
    ) -> Result<Self, RelationalSupportPlannerError> {
        if schema.output_dimension != dimension_id {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "finite factor output dimension disagrees with its stage",
            ));
        }
        let restored = Self {
            stage_id,
            role,
            dimension_id,
            schema,
            support,
            recipe,
        };
        if let Some(cell) = restored.cell() {
            if cell.space() != SupportCellSpace::ProducerCoordinates(restored.recipe.producer_id)
                || cell.materializer_id() != restored.recipe.materializer_id
            {
                return Err(RelationalSupportPlannerError::PlanInvariant(
                    "finite factor cell disagrees with its recipe",
                ));
            }
        }
        if !restored.support.is_exact_empty()
            && (restored.exactness().lower_bound()
                != restored
                    .cell()
                    .map_or(0, |cell| cell.cardinality().lower_bound())
                || restored.exactness().exact()
                    != restored.cell().and_then(|cell| cell.cardinality().exact()))
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "finite factor exactness disagrees with its cell",
            ));
        }
        Ok(restored)
    }

    pub(crate) const fn stage_id(&self) -> RelationalBindingStageId {
        self.stage_id
    }

    pub(crate) const fn role(&self) -> ExploreSourceBindingRoleIr {
        self.role
    }

    pub(crate) const fn dimension_id(&self) -> RelationalDimensionId {
        self.dimension_id
    }

    pub(crate) const fn schema(&self) -> &RelationalFactorSchema {
        &self.schema
    }

    pub(crate) const fn support(&self) -> &RelationalPlannedSupport {
        &self.support
    }

    pub(crate) const fn cell(&self) -> Option<&SupportCell> {
        self.support.cell_ref()
    }

    pub(crate) const fn exactness(&self) -> RelationalSupportExactness {
        self.support.exactness()
    }

    pub(crate) const fn recipe(&self) -> &RelationalFiniteFactorRecipe {
        &self.recipe
    }
}

/// One deterministic singleton extension. It may construct an auxiliary,
/// Context, or Before value, but introduces no new support coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSingletonMapStage {
    stage_id: RelationalBindingStageId,
    binding_index: u32,
    role: ExploreSourceBindingRoleIr,
    dependency_key: RelationalDependencyKeyRecipe,
    input_dimensions: Box<[RelationalDimensionId]>,
}

impl RelationalSingletonMapStage {
    pub(super) fn restore_from_journal_codec(
        stage_id: RelationalBindingStageId,
        binding_index: u32,
        role: ExploreSourceBindingRoleIr,
        dependency_key: RelationalDependencyKeyRecipe,
        input_dimensions: Box<[RelationalDimensionId]>,
    ) -> Self {
        Self {
            stage_id,
            binding_index,
            role,
            dependency_key,
            input_dimensions,
        }
    }

    pub(crate) const fn stage_id(&self) -> RelationalBindingStageId {
        self.stage_id
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        self.binding_index
    }

    pub(crate) const fn role(&self) -> ExploreSourceBindingRoleIr {
        self.role
    }

    pub(crate) const fn dependency_key(&self) -> &RelationalDependencyKeyRecipe {
        &self.dependency_key
    }

    pub(crate) fn input_dimensions(&self) -> &[RelationalDimensionId] {
        &self.input_dimensions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalBindingStage {
    Finite(RelationalFiniteFactorStage),
    Singleton(RelationalSingletonMapStage),
}

impl RelationalBindingStage {
    pub(crate) const fn stage_id(&self) -> RelationalBindingStageId {
        match self {
            Self::Finite(stage) => stage.stage_id,
            Self::Singleton(stage) => stage.stage_id,
        }
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        match self {
            Self::Finite(stage) => stage.recipe.binding_index,
            Self::Singleton(stage) => stage.binding_index,
        }
    }

    pub(crate) const fn role(&self) -> ExploreSourceBindingRoleIr {
        match self {
            Self::Finite(stage) => stage.role,
            Self::Singleton(stage) => stage.role,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportPopulationKind {
    SourceAssignments,
    SourceRows,
    SuccessorCoordinates,
    Cases,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportPopulationRecipe {
    ExactEmpty {
        reason: RelationalExactEmptyReason,
    },
    IndependentAssignmentProduct {
        factor_cells: Box<[SupportCellId]>,
    },
    DependentAssignmentJoin {
        factor_cells: Box<[SupportCellId]>,
    },
    SourceRowImage {
        assignment_cell: SupportCellId,
    },
    SuccessorFiberSum {
        source_row_cell: SupportCellId,
        successor_kind: RelationalSuccessorRecipeKind,
    },
    CaseImage {
        successor_coordinate_cell: SupportCellId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSuccessorRecipeKind {
    Singleton,
    FiniteExact,
    FiniteCollection,
    FiniteIntRange,
}

/// One logical population and either its materializable cell or exact-empty
/// terminal. A mapped-image cell remains explicitly open even when its
/// coordinate preimage is structurally exact; only an empty preimage maps to
/// exact empty without injectivity evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalPlannedPopulation {
    kind: RelationalSupportPopulationKind,
    support: RelationalPlannedSupport,
    recipe: RelationalSupportPopulationRecipe,
}

impl RelationalPlannedPopulation {
    pub(super) fn restore_from_journal_codec(
        kind: RelationalSupportPopulationKind,
        support: RelationalPlannedSupport,
        recipe: RelationalSupportPopulationRecipe,
    ) -> Result<Self, RelationalSupportPlannerError> {
        match (&support, &recipe) {
            (
                RelationalPlannedSupport::ExactEmpty { reason: left },
                RelationalSupportPopulationRecipe::ExactEmpty { reason: right },
            ) if left == right => {}
            (
                RelationalPlannedSupport::Cell { .. },
                RelationalSupportPopulationRecipe::ExactEmpty { .. },
            )
            | (RelationalPlannedSupport::ExactEmpty { .. }, _) => {
                return Err(RelationalSupportPlannerError::PlanInvariant(
                    "population support and recipe disagree about exact emptiness",
                ));
            }
            (RelationalPlannedSupport::Cell { .. }, _) => {}
        }
        Ok(Self {
            kind,
            support,
            recipe,
        })
    }

    pub(crate) const fn kind(&self) -> RelationalSupportPopulationKind {
        self.kind
    }

    pub(crate) const fn support(&self) -> &RelationalPlannedSupport {
        &self.support
    }

    pub(crate) const fn cell(&self) -> Option<&SupportCell> {
        self.support.cell_ref()
    }

    pub(crate) const fn exactness(&self) -> RelationalSupportExactness {
        self.support.exactness()
    }

    pub(crate) const fn recipe(&self) -> &RelationalSupportPopulationRecipe {
        &self.recipe
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCoverageStatus {
    NoKnownGaps,
    HasCoverageGaps,
}

/// Breadth qualifier carried beside exact relation support. Coverage gaps
/// qualify claims about the intended profile universe; they do not make the
/// explicitly declared finite relation inexact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCoverageQualifier {
    status: RelationalCoverageStatus,
    manifest_digest: Box<str>,
    semantic_dependency_digest: [u8; 32],
    varied_dimensions: usize,
    derived_subjects: usize,
    conditioned_subjects: usize,
    irrelevance_certificates: usize,
    coverage_gaps: usize,
}

impl RelationalCoverageQualifier {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        status: RelationalCoverageStatus,
        manifest_digest: Box<str>,
        semantic_dependency_digest: [u8; 32],
        varied_dimensions: usize,
        derived_subjects: usize,
        conditioned_subjects: usize,
        irrelevance_certificates: usize,
        coverage_gaps: usize,
    ) -> Result<Self, RelationalSupportPlannerError> {
        if manifest_digest.len() != 64
            || !manifest_digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || matches!(status, RelationalCoverageStatus::NoKnownGaps) != (coverage_gaps == 0)
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "coverage qualifier is not canonical",
            ));
        }
        Ok(Self {
            status,
            manifest_digest,
            semantic_dependency_digest,
            varied_dimensions,
            derived_subjects,
            conditioned_subjects,
            irrelevance_certificates,
            coverage_gaps,
        })
    }

    pub(crate) const fn status(&self) -> RelationalCoverageStatus {
        self.status
    }

    pub(crate) fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub(crate) const fn semantic_dependency_digest(&self) -> [u8; 32] {
        self.semantic_dependency_digest
    }

    pub(crate) const fn varied_dimensions(&self) -> usize {
        self.varied_dimensions
    }

    pub(crate) const fn derived_subjects(&self) -> usize {
        self.derived_subjects
    }

    pub(crate) const fn conditioned_subjects(&self) -> usize {
        self.conditioned_subjects
    }

    pub(crate) const fn irrelevance_certificates(&self) -> usize {
        self.irrelevance_certificates
    }

    pub(crate) const fn coverage_gaps(&self) -> usize {
        self.coverage_gaps
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalObligationActivation {
    RootCasePopulation,
    AdmissionDecision(AdmissionDecision),
    SelectionDecision(SelectionDecision),
}

/// An already-instantiated root obligation or a claim that becomes meaningful
/// only on a uniformly classified leaf. FIND is deliberately activated only
/// for admitted support; rejected cases have no selection evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStagedObligationDescriptor {
    Root {
        activation: RelationalObligationActivation,
        obligation: SupportObligationRecord,
    },
    SelectionOnAdmitted {
        activation: RelationalObligationActivation,
        question_id: QuestionId,
    },
}

impl RelationalStagedObligationDescriptor {
    pub(crate) const fn activation(&self) -> RelationalObligationActivation {
        match self {
            Self::Root { activation, .. } | Self::SelectionOnAdmitted { activation, .. } => {
                *activation
            }
        }
    }
}

/// Canonically ordered payload for registering every cell in a support plan.
///
/// Journal integration must register this payload as a unit before scheduling
/// a root. It includes finite factor cells and all assignment, row,
/// successor-coordinate, and case intermediates. Ordering is by content ID,
/// not construction order, so equivalent plans serialize identically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSupportCellCatalog {
    cells: Box<[SupportCell]>,
}

impl RelationalSupportCellCatalog {
    fn from_cells(
        cells: impl IntoIterator<Item = SupportCell>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        let mut canonical = BTreeMap::<SupportCellId, SupportCell>::new();
        for cell in cells {
            cell.validate()?;
            match canonical.get(&cell.id()) {
                Some(existing) if existing != &cell => {
                    return Err(RelationalSupportPlannerError::SupportCellIdCollision(
                        cell.id(),
                    ));
                }
                Some(_) => {}
                None => {
                    canonical.insert(cell.id(), cell);
                }
            }
        }
        Ok(Self {
            cells: canonical
                .into_values()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    pub(crate) fn cells(&self) -> &[SupportCell] {
        &self.cells
    }

    pub(crate) fn cell_ids(&self) -> impl ExactSizeIterator<Item = SupportCellId> + '_ {
        self.cells.iter().map(SupportCell::id)
    }

    pub(crate) fn get(&self, id: SupportCellId) -> Option<&SupportCell> {
        self.cells
            .binary_search_by_key(&id, SupportCell::id)
            .ok()
            .map(|index| &self.cells[index])
    }

    pub(crate) fn contains(&self, id: SupportCellId) -> bool {
        self.get(id).is_some()
    }
}

/// Root proof work after logical support planning.
///
/// An exact-empty root is already resolved at cardinality zero. Admission and
/// selection have no cases to classify, so no illegal empty cell or vacuous
/// per-cell obligation is minted. A cell-backed root carries every staged
/// descriptor that must be discharged after registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalRootObligationPlan {
    ResolvedExactEmpty {
        admission_id: AdmissionId,
    },
    CellBacked {
        root_cell_id: SupportCellId,
        descriptors: Box<[RelationalStagedObligationDescriptor]>,
    },
}

impl RelationalRootObligationPlan {
    pub(crate) const fn root_cell_id(&self) -> Option<SupportCellId> {
        match self {
            Self::ResolvedExactEmpty { .. } => None,
            Self::CellBacked { root_cell_id, .. } => Some(*root_cell_id),
        }
    }

    pub(crate) const fn resolved_exact_cardinality(&self) -> Option<u128> {
        match self {
            Self::ResolvedExactEmpty { .. } => Some(0),
            Self::CellBacked { .. } => None,
        }
    }

    pub(crate) const fn admission_id(&self) -> Option<AdmissionId> {
        match self {
            Self::ResolvedExactEmpty { admission_id, .. } => Some(*admission_id),
            Self::CellBacked { .. } => None,
        }
    }

    pub(crate) fn descriptors(&self) -> &[RelationalStagedObligationDescriptor] {
        match self {
            Self::ResolvedExactEmpty { .. } => &[],
            Self::CellBacked { descriptors, .. } => descriptors,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSupportPlan {
    root: RelationalSupportPlanRoot,
    payload: RelationalSupportPlanPayload,
}

/// One checked literal admission predicate retained as a replayable producer
/// fact. Spans and authored names are absent; canonical position, scope, and
/// the checked Boolean value are the complete recognized shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalLiteralAdmissionPredicate {
    admission_index: u32,
    scope: ExploreAdmissionScope,
    value: bool,
}

impl RelationalLiteralAdmissionPredicate {
    pub(super) const fn restore_from_journal_codec(
        admission_index: u32,
        scope: ExploreAdmissionScope,
        value: bool,
    ) -> Self {
        Self {
            admission_index,
            scope,
            value,
        }
    }

    pub(crate) const fn admission_index(self) -> u32 {
        self.admission_index
    }

    pub(crate) const fn scope(self) -> ExploreAdmissionScope {
        self.scope
    }

    pub(crate) const fn value(self) -> bool {
        self.value
    }
}

/// Canonical plan-owned recipe available to the uniform-admission verifier.
///
/// `Unsupported` is an explicit fail-closed boundary: it records that the
/// checked admission layer contains at least one expression whose uniformity
/// this producer does not know how to prove. It must never be interpreted as
/// a request to evaluate a representative case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalUniformAdmissionProofRecipe {
    LiteralConjunction {
        predicates: Box<[RelationalLiteralAdmissionPredicate]>,
    },
    Unsupported,
}

impl RelationalUniformAdmissionProofRecipe {
    pub(super) fn restore_literal_conjunction_from_journal_codec(
        predicates: Box<[RelationalLiteralAdmissionPredicate]>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        if predicates.iter().enumerate().any(|(expected, predicate)| {
            u32::try_from(expected).ok() != Some(predicate.admission_index())
        }) {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "literal admission predicates are not in canonical index order",
            ));
        }
        Ok(Self::LiteralConjunction { predicates })
    }

    pub(super) const fn restore_unsupported_from_journal_codec() -> Self {
        Self::Unsupported
    }

    pub(crate) fn literal_predicates(&self) -> Option<&[RelationalLiteralAdmissionPredicate]> {
        match self {
            Self::LiteralConjunction { predicates } => Some(predicates),
            Self::Unsupported => None,
        }
    }
}

/// Owned canonical payload sealed by [`RelationalSupportPlanRoot`]. Keeping it
/// separate makes it impossible to accept a caller-supplied root for an
/// independently assembled payload.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalSupportPlanPayload {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_ids: Box<[QuestionId]>,
    uniform_admission_proof: RelationalUniformAdmissionProofRecipe,
    stages: Box<[RelationalBindingStage]>,
    source_assignments: RelationalPlannedPopulation,
    source_rows: RelationalPlannedPopulation,
    successor_coordinates: RelationalPlannedPopulation,
    cases: RelationalPlannedPopulation,
    cell_catalog: RelationalSupportCellCatalog,
    root_obligations: RelationalRootObligationPlan,
    coverage: RelationalCoverageQualifier,
    source_image_projection: Option<CheckedExploreSourceImageProjectionCertificate>,
}

impl RelationalSupportPlan {
    fn from_payload(payload: RelationalSupportPlanPayload) -> Self {
        let root = derive_support_plan_root(&payload);
        Self { root, payload }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_ids: Box<[QuestionId]>,
        uniform_admission_proof: RelationalUniformAdmissionProofRecipe,
        stages: Box<[RelationalBindingStage]>,
        source_assignments: RelationalPlannedPopulation,
        source_rows: RelationalPlannedPopulation,
        successor_coordinates: RelationalPlannedPopulation,
        cases: RelationalPlannedPopulation,
        root_obligations: RelationalRootObligationPlan,
        coverage: RelationalCoverageQualifier,
        source_image_projection: Option<CheckedExploreSourceImageProjectionCertificate>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        let mut unique_stage_ids = BTreeSet::new();
        if stages
            .iter()
            .enumerate()
            .any(|(index, stage)| u32::try_from(index).ok() != Some(stage.binding_index()))
            || stages
                .iter()
                .any(|stage| !unique_stage_ids.insert(stage.stage_id()))
            || source_assignments.kind() != RelationalSupportPopulationKind::SourceAssignments
            || source_rows.kind() != RelationalSupportPopulationKind::SourceRows
            || successor_coordinates.kind() != RelationalSupportPopulationKind::SuccessorCoordinates
            || cases.kind() != RelationalSupportPopulationKind::Cases
            || question_ids.windows(2).any(|pair| pair[0] >= pair[1])
            || matches!(
                &root_obligations,
                RelationalRootObligationPlan::ResolvedExactEmpty {
                    admission_id: root_admission_id,
                } if *root_admission_id != admission_id
            )
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "support-plan stages or population roles are not canonical",
            ));
        }
        validate_restored_stages(relation_id, &coverage, &stages)?;
        validate_source_image_projection_recipe(
            relation_id,
            &coverage,
            &stages,
            source_image_projection.as_ref(),
        )?;
        validate_root_question_descriptors(&root_obligations, &question_ids)?;
        let cell_catalog = build_cell_catalog(
            &stages,
            [
                &source_assignments,
                &source_rows,
                &successor_coordinates,
                &cases,
            ],
            &root_obligations,
        )?;
        Ok(Self::from_payload(RelationalSupportPlanPayload {
            relation_id,
            admission_id,
            question_ids,
            uniform_admission_proof,
            stages,
            source_assignments,
            source_rows,
            successor_coordinates,
            cases,
            cell_catalog,
            root_obligations,
            coverage,
            source_image_projection,
        }))
    }

    pub(crate) const fn root(&self) -> RelationalSupportPlanRoot {
        self.root
    }

    pub(crate) fn validate_root(&self) -> bool {
        self.root == derive_support_plan_root(&self.payload)
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.payload.relation_id
    }

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.payload.admission_id
    }

    pub(crate) fn question_ids(&self) -> &[QuestionId] {
        &self.payload.question_ids
    }

    pub(crate) const fn uniform_admission_proof(&self) -> &RelationalUniformAdmissionProofRecipe {
        &self.payload.uniform_admission_proof
    }

    pub(crate) fn stages(&self) -> &[RelationalBindingStage] {
        &self.payload.stages
    }

    pub(crate) const fn source_assignments(&self) -> &RelationalPlannedPopulation {
        &self.payload.source_assignments
    }

    pub(crate) const fn source_rows(&self) -> &RelationalPlannedPopulation {
        &self.payload.source_rows
    }

    pub(crate) const fn successor_coordinates(&self) -> &RelationalPlannedPopulation {
        &self.payload.successor_coordinates
    }

    pub(crate) const fn cases(&self) -> &RelationalPlannedPopulation {
        &self.payload.cases
    }

    /// Canonical registration payload. Consumers must register these cells as
    /// a unit rather than registering only [`Self::root_cell_id`].
    pub(crate) const fn cell_catalog(&self) -> &RelationalSupportCellCatalog {
        &self.payload.cell_catalog
    }

    pub(crate) fn all_cells(&self) -> &[SupportCell] {
        self.payload.cell_catalog.cells()
    }

    /// Case root used after catalog registration. This is absent for a
    /// statically empty relation and must not be used as the registration
    /// payload; use [`Self::all_cells`] for that.
    pub(crate) const fn root_cell_id(&self) -> Option<SupportCellId> {
        self.payload.root_obligations.root_cell_id()
    }

    pub(crate) const fn root_obligations(&self) -> &RelationalRootObligationPlan {
        &self.payload.root_obligations
    }

    pub(crate) fn obligations(&self) -> &[RelationalStagedObligationDescriptor] {
        self.payload.root_obligations.descriptors()
    }

    pub(crate) const fn coverage(&self) -> &RelationalCoverageQualifier {
        &self.payload.coverage
    }

    pub(crate) fn source_image_projection(
        &self,
    ) -> Option<&CheckedExploreSourceImageProjectionCertificate> {
        self.payload.source_image_projection.as_ref()
    }
}

fn validate_root_question_descriptors(
    root_obligations: &RelationalRootObligationPlan,
    question_ids: &[QuestionId],
) -> Result<(), RelationalSupportPlannerError> {
    let RelationalRootObligationPlan::CellBacked { descriptors, .. } = root_obligations else {
        return Ok(());
    };
    let descriptor_questions = descriptors
        .iter()
        .filter_map(|descriptor| match descriptor {
            RelationalStagedObligationDescriptor::SelectionOnAdmitted { question_id, .. } => {
                Some(*question_id)
            }
            RelationalStagedObligationDescriptor::Root { .. } => None,
        })
        .collect::<Vec<_>>();
    if descriptor_questions != question_ids {
        return Err(RelationalSupportPlannerError::PlanInvariant(
            "support-plan FIND obligations do not match its canonical question set",
        ));
    }
    Ok(())
}

/// Whether canonical source assignments can be recovered from the resulting
/// `(Context, Before)` row without evaluating user expressions.
///
/// Finite bindings are already set-normalized by their checked producer.  A
/// finite Context or Before coordinate is retained verbatim in the source row,
/// so independent products and dependent joins over only those coordinates
/// are injective into source rows.  A finite Auxiliary coordinate is not
/// retained and therefore needs a separate expression-level proof; singleton
/// auxiliaries introduce no varying coordinate and are harmless here.
fn source_assignment_image_is_structurally_injective(stages: &[RelationalBindingStage]) -> bool {
    let context_count = stages
        .iter()
        .filter(|stage| stage.role() == ExploreSourceBindingRoleIr::Context)
        .count();
    let before_count = stages
        .iter()
        .filter(|stage| stage.role() == ExploreSourceBindingRoleIr::Before)
        .count();
    context_count == 1
        && before_count == 1
        && stages.iter().all(|stage| {
            !matches!(
                stage,
                RelationalBindingStage::Finite(finite)
                    if finite.role() == ExploreSourceBindingRoleIr::Auxiliary
            )
        })
}

fn validate_source_image_projection_recipe(
    relation_id: RelationId,
    coverage: &RelationalCoverageQualifier,
    stages: &[RelationalBindingStage],
    certificate: Option<&CheckedExploreSourceImageProjectionCertificate>,
) -> Result<(), RelationalSupportPlannerError> {
    let Some(certificate) = certificate else {
        return Ok(());
    };
    if !certificate.validate_identity()
        || certificate.relation_id != relation_id
        || certificate.semantic_dependency_digest != coverage.semantic_dependency_digest()
    {
        return Err(RelationalSupportPlannerError::PlanInvariant(
            "source-image projection certificate identity or semantic scope is invalid",
        ));
    }
    let finite_stages = stages
        .iter()
        .filter_map(|stage| match stage {
            RelationalBindingStage::Finite(finite) => Some(finite),
            RelationalBindingStage::Singleton(_) => None,
        })
        .collect::<Vec<_>>();
    if finite_stages.len() != certificate.factors.len()
        || certificate.factors.len() != certificate.witnesses.len()
    {
        return Err(RelationalSupportPlannerError::PlanInvariant(
            "source-image projection certificate does not cover every finite factor",
        ));
    }
    let mut endpoint_paths = BTreeSet::new();
    for ((stage, factor), witness) in finite_stages
        .iter()
        .zip(certificate.factors.iter())
        .zip(certificate.witnesses.iter())
    {
        let kind_matches = matches!(
            (factor.kind, witness.kind, stage.recipe().domain_kind()),
            (
                CheckedExploreSourceProjectionFactorKind::AffineIntRange { .. },
                CheckedExploreSourceProjectionWitnessKind::Affine { coefficient, .. },
                RelationalFiniteDomainRecipeKind::CheckedIntRange,
            ) if coefficient != 0
        ) || matches!(
            (factor.kind, witness.kind, stage.recipe().domain_kind()),
            (
                CheckedExploreSourceProjectionFactorKind::ExactFinite { plan_digest },
                CheckedExploreSourceProjectionWitnessKind::DirectIdentity {
                    plan_digest: witness_plan_digest,
                },
                RelationalFiniteDomainRecipeKind::CheckedExact,
            ) if plan_digest == witness_plan_digest
        );
        if stage.recipe().binding_index() != factor.binding_index
            || !stage.recipe().dependency_key().is_empty()
            || !stage.schema().key_dimensions().is_empty()
            || stage.recipe().known_local_cardinality() != Some(factor.exact_cardinality)
            || stage.exactness().exact() != Some(factor.exact_cardinality)
            || stage.cell().is_none()
            || witness.factor_binding_index != factor.binding_index
            || !kind_matches
        {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "source-image projection factor disagrees with the exact independent stage",
            ));
        }
        let path_key = (
            match witness.endpoint {
                CheckedExploreSourceProjectionEndpoint::Context => 0_u8,
                CheckedExploreSourceProjectionEndpoint::Before => 1_u8,
            },
            witness.path.to_vec(),
        );
        if !endpoint_paths.insert(path_key) {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "source-image projection witnesses reuse one endpoint field path",
            ));
        }
    }
    Ok(())
}

/// Assignment support recognized by the producer-chain injectivity verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseImageAssignmentKind {
    IndependentProduct,
    DependentJoin,
}

impl RelationalCaseImageAssignmentKind {
    const fn tag(self) -> u8 {
        match self {
            Self::IndependentProduct => 0x01,
            Self::DependentJoin => 0x02,
        }
    }
}

/// Whether the proof also establishes assignment-to-source-row injectivity.
///
/// This is deliberately separate from case-image injectivity. A generic case
/// proof starts from already normalized successor coordinates and remains
/// sound when multiple FROM assignments may collapse to one source row. Only
/// A direct endpoint proof or an identity-visible separating-projection proof
/// authorizes composing a singleton successor over the assignment expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSourceAssignmentImageProof {
    Unproven,
    DirectEndpointCoordinates,
    SeparatedProjectionCertificate,
}

impl RelationalSourceAssignmentImageProof {
    const fn tag(self) -> u8 {
        match self {
            Self::Unproven => 0x01,
            Self::DirectEndpointCoordinates => 0x02,
            Self::SeparatedProjectionCertificate => 0x03,
        }
    }
}

/// Identity-visible reference to the separately verified source-image proof
/// composed by a v2 case-image certificate. The enum strategy alone never
/// authorizes ranked case coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseSourceImageProofReference {
    compiler_certificate_id: [u8; 32],
    source_exactness_certificate_id: [u8; 32],
    source_injectivity_evidence_id: SupportCellEvidenceId,
    source_population_root: CertifiedSourcePopulationRoot,
}

impl RelationalCaseSourceImageProofReference {
    pub(super) const fn restore_from_journal_codec(
        compiler_certificate_id: [u8; 32],
        source_exactness_certificate_id: [u8; 32],
        source_injectivity_evidence_id: SupportCellEvidenceId,
        source_population_root: CertifiedSourcePopulationRoot,
    ) -> Self {
        Self {
            compiler_certificate_id,
            source_exactness_certificate_id,
            source_injectivity_evidence_id,
            source_population_root,
        }
    }

    pub(crate) const fn compiler_certificate_id(self) -> [u8; 32] {
        self.compiler_certificate_id
    }

    pub(crate) const fn source_exactness_certificate_id(self) -> [u8; 32] {
        self.source_exactness_certificate_id
    }

    pub(crate) const fn source_injectivity_evidence_id(self) -> SupportCellEvidenceId {
        self.source_injectivity_evidence_id
    }

    pub(crate) const fn source_population_root(self) -> CertifiedSourcePopulationRoot {
        self.source_population_root
    }
}

/// Coordinate contract feeding the final case-image materializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseImagePreimageKind {
    CanonicalSuccessorFiberJoin,
    ComposedSingletonAssignment,
}

impl RelationalCaseImagePreimageKind {
    const fn tag(self) -> u8 {
        match self {
            Self::CanonicalSuccessorFiberJoin => 0x01,
            Self::ComposedSingletonAssignment => 0x02,
        }
    }
}

/// Canonical replay artifact for the checked producer chain ending in the
/// mapped case root. It is not proof authority by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseImageInjectivityProofArtifact {
    schema_version: u32,
    certificate_id: [u8; 32],
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    binding_stage_ids: Box<[RelationalBindingStageId]>,
    finite_factor_cell_ids: Box<[SupportCellId]>,
    assignment_kind: RelationalCaseImageAssignmentKind,
    source_assignment_image_proof: RelationalSourceAssignmentImageProof,
    source_image_proof_reference: Option<RelationalCaseSourceImageProofReference>,
    source_assignment_cell_id: SupportCellId,
    source_row_cell_id: SupportCellId,
    successor_coordinate_cell_id: SupportCellId,
    successor_kind: RelationalSuccessorRecipeKind,
    preimage_kind: RelationalCaseImagePreimageKind,
    case_cell_id: SupportCellId,
    case_materializer_id: SupportMaterializerId,
    exact_case_cardinality: Option<u128>,
}

impl RelationalCaseImageInjectivityProofArtifact {
    /// Codec seam for a retained artifact. This validates canonical identity
    /// and shape only; it deliberately does not create a verified proof token.
    /// Receipt authority still requires
    /// [`reverify_relational_case_image_injectivity_artifact`].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        schema_version: u32,
        certificate_id: [u8; 32],
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        binding_stage_ids: Box<[RelationalBindingStageId]>,
        finite_factor_cell_ids: Box<[SupportCellId]>,
        assignment_kind: RelationalCaseImageAssignmentKind,
        source_assignment_image_proof: RelationalSourceAssignmentImageProof,
        source_image_proof_reference: Option<RelationalCaseSourceImageProofReference>,
        source_assignment_cell_id: SupportCellId,
        source_row_cell_id: SupportCellId,
        successor_coordinate_cell_id: SupportCellId,
        successor_kind: RelationalSuccessorRecipeKind,
        preimage_kind: RelationalCaseImagePreimageKind,
        case_cell_id: SupportCellId,
        case_materializer_id: SupportMaterializerId,
        exact_case_cardinality: Option<u128>,
    ) -> Result<Self, RelationalCaseImageInjectivityProofError> {
        let artifact = Self {
            schema_version,
            certificate_id,
            plan_root,
            relation_id,
            binding_stage_ids,
            finite_factor_cell_ids,
            assignment_kind,
            source_assignment_image_proof,
            source_image_proof_reference,
            source_assignment_cell_id,
            source_row_cell_id,
            successor_coordinate_cell_id,
            successor_kind,
            preimage_kind,
            case_cell_id,
            case_materializer_id,
            exact_case_cardinality,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    pub(crate) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(crate) const fn certificate_id(&self) -> [u8; 32] {
        self.certificate_id
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) fn binding_stage_ids(&self) -> &[RelationalBindingStageId] {
        &self.binding_stage_ids
    }

    pub(crate) fn finite_factor_cell_ids(&self) -> &[SupportCellId] {
        &self.finite_factor_cell_ids
    }

    pub(crate) const fn assignment_kind(&self) -> RelationalCaseImageAssignmentKind {
        self.assignment_kind
    }

    pub(crate) const fn source_assignment_image_proof(
        &self,
    ) -> RelationalSourceAssignmentImageProof {
        self.source_assignment_image_proof
    }

    pub(crate) const fn source_image_proof_reference(
        &self,
    ) -> Option<RelationalCaseSourceImageProofReference> {
        self.source_image_proof_reference
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

    pub(crate) const fn successor_kind(&self) -> RelationalSuccessorRecipeKind {
        self.successor_kind
    }

    pub(crate) const fn preimage_kind(&self) -> RelationalCaseImagePreimageKind {
        self.preimage_kind
    }

    pub(crate) const fn case_cell_id(&self) -> SupportCellId {
        self.case_cell_id
    }

    pub(crate) const fn case_materializer_id(&self) -> SupportMaterializerId {
        self.case_materializer_id
    }

    pub(crate) const fn exact_case_cardinality(&self) -> Option<u128> {
        self.exact_case_cardinality
    }

    fn validate_identity(&self) -> Result<(), RelationalCaseImageInjectivityProofError> {
        if !matches!(
            self.schema_version,
            RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION_V1
                | RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION
        ) {
            return Err(
                RelationalCaseImageInjectivityProofError::UnsupportedArtifactVersion(
                    self.schema_version,
                ),
            );
        }
        let v1_specialization_shape = matches!(
            (
                self.schema_version,
                self.assignment_kind,
                self.source_assignment_image_proof,
                self.source_image_proof_reference,
                self.successor_kind,
                self.preimage_kind,
                self.exact_case_cardinality,
            ),
            (
                RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION_V1,
                RelationalCaseImageAssignmentKind::IndependentProduct,
                RelationalSourceAssignmentImageProof::DirectEndpointCoordinates,
                None,
                RelationalSuccessorRecipeKind::Singleton,
                RelationalCaseImagePreimageKind::ComposedSingletonAssignment,
                Some(_),
            )
        );
        let v2_specialization_shape = matches!(
            (
                self.schema_version,
                self.assignment_kind,
                self.source_assignment_image_proof,
                self.source_image_proof_reference,
                self.successor_kind,
                self.preimage_kind,
                self.exact_case_cardinality,
            ),
            (
                RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION,
                RelationalCaseImageAssignmentKind::IndependentProduct,
                RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate,
                Some(_),
                RelationalSuccessorRecipeKind::Singleton,
                RelationalCaseImagePreimageKind::ComposedSingletonAssignment,
                Some(_),
            )
        );
        let generic_shape = self.schema_version
            == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION_V1
            && self.source_image_proof_reference.is_none()
            && matches!(
                self.source_assignment_image_proof,
                RelationalSourceAssignmentImageProof::Unproven
                    | RelationalSourceAssignmentImageProof::DirectEndpointCoordinates
            )
            && self.preimage_kind == RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin
            && self.exact_case_cardinality.is_none();
        let mut unique_binding_stage_ids = BTreeSet::new();
        let duplicate_binding_stage_id = self
            .binding_stage_ids
            .iter()
            .any(|stage_id| !unique_binding_stage_ids.insert(*stage_id));
        let mut unique_finite_factor_cell_ids = BTreeSet::new();
        let duplicate_finite_factor_cell_id = self
            .finite_factor_cell_ids
            .iter()
            .any(|cell_id| !unique_finite_factor_cell_ids.insert(*cell_id));
        if self.binding_stage_ids.is_empty()
            || duplicate_binding_stage_id
            || duplicate_finite_factor_cell_id
            || matches!(self.exact_case_cardinality, Some(0))
            || (!v1_specialization_shape && !v2_specialization_shape && !generic_shape)
        {
            return Err(RelationalCaseImageInjectivityProofError::InvalidArtifactShape);
        }
        let derived = derive_case_image_injectivity_certificate_id(self);
        if derived != self.certificate_id {
            return Err(RelationalCaseImageInjectivityProofError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// Opaque typed binding consumed by the support-cell issuance gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseImageEvidenceBinding {
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl RelationalCaseImageEvidenceBinding {
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

/// Authority token returned only after replaying recognized producer recipes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalCaseImageInjectivityProof {
    artifact: RelationalCaseImageInjectivityProofArtifact,
    injectivity_binding: RelationalCaseImageEvidenceBinding,
    cardinality_binding: Option<RelationalCaseImageEvidenceBinding>,
}

impl VerifiedRelationalCaseImageInjectivityProof {
    pub(crate) const fn artifact(&self) -> &RelationalCaseImageInjectivityProofArtifact {
        &self.artifact
    }

    pub(crate) const fn injectivity_binding(&self) -> RelationalCaseImageEvidenceBinding {
        self.injectivity_binding
    }

    pub(crate) const fn cardinality_binding(&self) -> Option<RelationalCaseImageEvidenceBinding> {
        self.cardinality_binding
    }
}

/// Typed evidence emitted by one successful producer-chain verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCaseImageInjectivityProof {
    proof: VerifiedRelationalCaseImageInjectivityProof,
    injectivity: SupportCellEvidence<InjectiveMappingClaim>,
    exact_cardinality: Option<SupportCellEvidence<ExactCardinalityClaim>>,
}

impl RelationalCaseImageInjectivityProof {
    pub(crate) const fn proof(&self) -> &VerifiedRelationalCaseImageInjectivityProof {
        &self.proof
    }

    pub(crate) const fn injectivity(&self) -> &SupportCellEvidence<InjectiveMappingClaim> {
        &self.injectivity
    }

    pub(crate) const fn exact_cardinality(
        &self,
    ) -> Option<&SupportCellEvidence<ExactCardinalityClaim>> {
        self.exact_cardinality.as_ref()
    }
}

/// Verify the canonical producer chain and issue typed case-image evidence.
///
/// General normalized successor coordinates establish final case-image
/// injectivity. Exact case cardinality is issued only for the stronger
/// composed-singleton specialization whose source assignment image is itself
/// structurally injective and whose independent assignment product is exact.
pub(crate) fn prove_relational_case_image_injectivity(
    plan: &RelationalSupportPlan,
) -> Result<RelationalCaseImageInjectivityProof, RelationalCaseImageInjectivityProofError> {
    let (mut artifact, case_cell) = verify_case_image_producer_chain(plan)?;
    artifact.certificate_id = derive_case_image_injectivity_certificate_id(&artifact);
    artifact.validate_identity()?;

    let injectivity_obligation = SupportCellObligation::new(
        case_cell,
        InjectiveMappingClaim::new(case_cell.materializer_id()),
    )?;
    let injectivity_conclusion = CertifiedInjective;
    let injectivity_binding = case_image_evidence_binding(
        artifact.schema_version,
        artifact.certificate_id,
        0x01,
        injectivity_obligation.id(),
        injectivity_obligation
            .claim()
            .conclusion_digest(&injectivity_conclusion),
    );

    let cardinality_obligation = artifact
        .exact_case_cardinality
        .map(|_| SupportCellObligation::new(case_cell, ExactCardinalityClaim))
        .transpose()?;
    let cardinality_binding = cardinality_obligation.as_ref().map(|obligation| {
        let count = artifact
            .exact_case_cardinality
            .expect("cardinality obligation follows exact artifact count");
        case_image_evidence_binding(
            artifact.schema_version,
            artifact.certificate_id,
            0x02,
            obligation.id(),
            obligation.claim().conclusion_digest(&count),
        )
    });
    let proof = VerifiedRelationalCaseImageInjectivityProof {
        artifact,
        injectivity_binding,
        cardinality_binding,
    };
    let injectivity = super::support_cell::relational_case_image_proof_gateway::injectivity(
        &proof,
        injectivity_obligation,
    )?;
    let exact_cardinality = match (
        cardinality_obligation,
        proof.artifact.exact_case_cardinality,
    ) {
        (Some(obligation), Some(count)) => Some(
            super::support_cell::relational_case_image_proof_gateway::cardinality(
                &proof, obligation, count,
            )?,
        ),
        (None, None) => None,
        _ => {
            return Err(
                RelationalCaseImageInjectivityProofError::InternalProofInvariant(
                    "cardinality obligation disagrees with the verified artifact",
                ),
            );
        }
    };
    Ok(RelationalCaseImageInjectivityProof {
        proof,
        injectivity,
        exact_cardinality,
    })
}

/// Replay-verify a retained artifact against the current canonical plan.
pub(crate) fn reverify_relational_case_image_injectivity_artifact(
    artifact: &RelationalCaseImageInjectivityProofArtifact,
    plan: &RelationalSupportPlan,
) -> Result<RelationalCaseImageInjectivityProof, RelationalCaseImageInjectivityProofError> {
    artifact.validate_identity()?;
    let verified = prove_relational_case_image_injectivity(plan)?;
    if verified.proof.artifact() != artifact {
        return Err(RelationalCaseImageInjectivityProofError::ArtifactSemanticMismatch);
    }
    Ok(verified)
}

fn verify_case_image_producer_chain(
    plan: &RelationalSupportPlan,
) -> Result<
    (RelationalCaseImageInjectivityProofArtifact, &SupportCell),
    RelationalCaseImageInjectivityProofError,
> {
    if !plan.validate_root() {
        return Err(RelationalCaseImageInjectivityProofError::InvalidPlanRoot);
    }
    validate_restored_stages(plan.relation_id(), plan.coverage(), plan.stages())?;
    for cell in plan.all_cells() {
        cell.validate()?;
    }

    let finite_stages = plan
        .stages()
        .iter()
        .filter_map(|stage| match stage {
            RelationalBindingStage::Finite(factor) => Some(factor),
            RelationalBindingStage::Singleton(_) => None,
        })
        .collect::<Vec<_>>();
    let finite_factor_cell_ids = finite_stages
        .iter()
        .map(|factor| {
            factor.cell().map(SupportCell::id).ok_or(
                RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                    "case-image proof does not apply to an exact-empty finite factor",
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let assignment_cell = plan.source_assignments().cell().ok_or(
        RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
            "case-image proof does not apply to an exact-empty assignment population",
        ),
    )?;
    let assignment_producer_id = derive_assignment_producer_id(
        plan.relation_id(),
        plan.coverage().semantic_dependency_digest(),
        &finite_stages,
    )?;
    let assignment_kind = match plan.source_assignments().recipe() {
        RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells }
            if factor_cells.as_ref() == finite_factor_cell_ids.as_slice() =>
        {
            if finite_stages.iter().any(|factor| {
                !factor.schema().key_dimensions().is_empty() || factor.exactness().exact().is_none()
            }) {
                return Err(
                    RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                        "independent assignment recipe contains a dependent or open factor",
                    ),
                );
            }
            RelationalCaseImageAssignmentKind::IndependentProduct
        }
        RelationalSupportPopulationRecipe::DependentAssignmentJoin { factor_cells }
            if factor_cells.as_ref() == finite_factor_cell_ids.as_slice() =>
        {
            RelationalCaseImageAssignmentKind::DependentJoin
        }
        _ => {
            return Err(
                RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                    "source assignments are not the normalized finite-stage product or join",
                ),
            );
        }
    };
    let expected_assignment_expression = match assignment_kind {
        RelationalCaseImageAssignmentKind::IndependentProduct => {
            if finite_stages.is_empty() {
                SupportExpr::singleton(super::ExploreValue::Unit)
            } else {
                SupportExpr::product(
                    finite_stages
                        .iter()
                        .map(|factor| {
                            factor
                                .cell()
                                .expect("exact-empty factor rejected above")
                                .expression()
                                .clone()
                        })
                        .collect(),
                )?
            }
        }
        RelationalCaseImageAssignmentKind::DependentJoin => {
            SupportExpr::join_reference(assignment_producer_id, finite_factor_cell_ids.clone())
        }
    };
    let expected_assignment_materializer = derive_materializer_id(
        ASSIGNMENT_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id.bytes(),
    );
    require_cell_contract(
        assignment_cell,
        SupportCellSpace::ProducerCoordinates(assignment_producer_id),
        &expected_assignment_expression,
        expected_assignment_materializer,
        "source-assignment cell",
    )?;

    let source_cell = plan.source_rows().cell().ok_or(
        RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
            "case-image proof does not apply to an exact-empty source-row population",
        ),
    )?;
    if !matches!(
        plan.source_rows().recipe(),
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell: id }
            if *id == assignment_cell.id()
    ) {
        return Err(
            RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                "source-row image does not consume the verified assignment cell",
            ),
        );
    }
    let expected_source_materializer = derive_materializer_id(
        SOURCE_IMAGE_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id.bytes(),
    );
    require_cell_contract(
        source_cell,
        SupportCellSpace::MappedImage {
            producer_id: assignment_producer_id,
            target: SupportExtensionalTarget::SourceRows(plan.relation_id()),
        },
        assignment_cell.expression(),
        expected_source_materializer,
        "source-row image cell",
    )?;

    let successor_cell = plan.successor_coordinates().cell().ok_or(
        RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
            "case-image proof does not apply to an exact-empty successor population",
        ),
    )?;
    let successor_kind = match plan.successor_coordinates().recipe() {
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell,
            successor_kind,
        } if *source_row_cell == source_cell.id() => *successor_kind,
        _ => {
            return Err(
                RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                    "successor coordinates are not the normalized source-row fiber sum",
                ),
            );
        }
    };
    let successor_producer_id = derive_successor_producer_id(
        plan.relation_id(),
        plan.coverage().semantic_dependency_digest(),
        source_cell.id(),
        successor_kind,
    );
    let (source_assignment_image_proof, source_image_proof_reference) =
        if source_assignment_image_is_structurally_injective(plan.stages()) {
            (
                RelationalSourceAssignmentImageProof::DirectEndpointCoordinates,
                None,
            )
        } else if let Some(compiler_certificate) = plan.source_image_projection() {
            let source_proof = prove_relational_source_image_exactness(plan)?;
            if source_proof
                .proof()
                .artifact()
                .compiler_projection_certificate_id()
                != Some(compiler_certificate.certificate_id)
            {
                return Err(
                    RelationalCaseImageInjectivityProofError::InternalProofInvariant(
                        "source exactness proof does not name the plan projection certificate",
                    ),
                );
            }
            (
                RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate,
                Some(RelationalCaseSourceImageProofReference {
                    compiler_certificate_id: compiler_certificate.certificate_id,
                    source_exactness_certificate_id: source_proof
                        .proof()
                        .artifact()
                        .certificate_id(),
                    source_injectivity_evidence_id: source_proof.injectivity().id(),
                    source_population_root: source_proof.population_root(),
                }),
            )
        } else {
            (RelationalSourceAssignmentImageProof::Unproven, None)
        };
    let can_compose_singleton = successor_kind == RelationalSuccessorRecipeKind::Singleton
        && assignment_kind == RelationalCaseImageAssignmentKind::IndependentProduct
        && matches!(
            source_assignment_image_proof,
            RelationalSourceAssignmentImageProof::DirectEndpointCoordinates
                | RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate
        )
        && assignment_cell.coordinate_cardinality().exact().is_some();
    let (preimage_kind, expected_successor_expression, successor_materializer_domain) =
        if can_compose_singleton {
            (
                RelationalCaseImagePreimageKind::ComposedSingletonAssignment,
                assignment_cell.expression().clone(),
                COMPOSED_SINGLETON_SUCCESSOR_MATERIALIZER_V1,
            )
        } else {
            (
                RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin,
                SupportExpr::join_reference(successor_producer_id, vec![source_cell.id()]),
                SUCCESSOR_MATERIALIZER_V1,
            )
        };
    let expected_successor_materializer = derive_materializer_id(
        successor_materializer_domain,
        plan.relation_id(),
        successor_producer_id.bytes(),
    );
    require_cell_contract(
        successor_cell,
        SupportCellSpace::ProducerCoordinates(successor_producer_id),
        &expected_successor_expression,
        expected_successor_materializer,
        "successor-coordinate cell",
    )?;

    let case_cell = plan.cases().cell().ok_or(
        RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
            "case-image proof does not apply to an exact-empty case population",
        ),
    )?;
    if !matches!(
        plan.cases().recipe(),
        RelationalSupportPopulationRecipe::CaseImage { successor_coordinate_cell }
            if *successor_coordinate_cell == successor_cell.id()
    ) || plan.root_cell_id() != Some(case_cell.id())
    {
        return Err(
            RelationalCaseImageInjectivityProofError::UnsupportedPlanShape(
                "case image is not the planned root over verified successor coordinates",
            ),
        );
    }
    let expected_case_materializer = derive_materializer_id(
        CASE_IMAGE_MATERIALIZER_V1,
        plan.relation_id(),
        successor_producer_id.bytes(),
    );
    require_cell_contract(
        case_cell,
        SupportCellSpace::MappedImage {
            producer_id: successor_producer_id,
            target: SupportExtensionalTarget::Cases(plan.relation_id()),
        },
        successor_cell.expression(),
        expected_case_materializer,
        "case-image root cell",
    )?;

    let exact_case_cardinality = match preimage_kind {
        RelationalCaseImagePreimageKind::ComposedSingletonAssignment => {
            Some(case_cell.coordinate_cardinality().exact().ok_or(
                RelationalCaseImageInjectivityProofError::InternalProofInvariant(
                    "composed singleton case coordinates lost exact cardinality",
                ),
            )?)
        }
        RelationalCaseImagePreimageKind::CanonicalSuccessorFiberJoin => None,
    };
    let artifact = RelationalCaseImageInjectivityProofArtifact {
        schema_version: if source_assignment_image_proof
            == RelationalSourceAssignmentImageProof::SeparatedProjectionCertificate
        {
            RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION
        } else {
            RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION_V1
        },
        certificate_id: [0; 32],
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        binding_stage_ids: plan
            .stages()
            .iter()
            .map(RelationalBindingStage::stage_id)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        finite_factor_cell_ids: finite_factor_cell_ids.into_boxed_slice(),
        assignment_kind,
        source_assignment_image_proof,
        source_image_proof_reference,
        source_assignment_cell_id: assignment_cell.id(),
        source_row_cell_id: source_cell.id(),
        successor_coordinate_cell_id: successor_cell.id(),
        successor_kind,
        preimage_kind,
        case_cell_id: case_cell.id(),
        case_materializer_id: case_cell.materializer_id(),
        exact_case_cardinality,
    };
    Ok((artifact, case_cell))
}

fn require_cell_contract(
    cell: &SupportCell,
    expected_space: SupportCellSpace,
    expected_expression: &SupportExpr,
    expected_materializer: SupportMaterializerId,
    subject: &'static str,
) -> Result<(), RelationalCaseImageInjectivityProofError> {
    if cell.space() != expected_space
        || cell.expression() != expected_expression
        || cell.materializer_id() != expected_materializer
    {
        return Err(RelationalCaseImageInjectivityProofError::CellContractMismatch(subject));
    }
    Ok(())
}

fn derive_case_image_injectivity_certificate_id(
    artifact: &RelationalCaseImageInjectivityProofArtifact,
) -> [u8; 32] {
    let domain = if artifact.schema_version == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION {
        CASE_IMAGE_INJECTIVITY_CERTIFICATE_V2
    } else {
        CASE_IMAGE_INJECTIVITY_CERTIFICATE_V1
    };
    let mut hasher = CanonicalPlannerHasher::new(domain);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.u128(artifact.binding_stage_ids.len() as u128);
    for stage_id in &artifact.binding_stage_ids {
        hasher.digest(stage_id.bytes());
    }
    hasher.u128(artifact.finite_factor_cell_ids.len() as u128);
    for cell_id in &artifact.finite_factor_cell_ids {
        hasher.digest(cell_id.bytes());
    }
    hasher.u8(artifact.assignment_kind.tag());
    hasher.u8(artifact.source_assignment_image_proof.tag());
    if artifact.schema_version == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION {
        match artifact.source_image_proof_reference {
            Some(reference) => {
                hasher.u8(0x01);
                hasher.digest(reference.compiler_certificate_id);
                hasher.digest(reference.source_exactness_certificate_id);
                hasher.digest(reference.source_injectivity_evidence_id.bytes());
                hasher.digest(reference.source_population_root.bytes());
            }
            None => hasher.u8(0x02),
        }
    }
    hasher.digest(artifact.source_assignment_cell_id.bytes());
    hasher.digest(artifact.source_row_cell_id.bytes());
    hasher.digest(artifact.successor_coordinate_cell_id.bytes());
    hasher.u8(match artifact.successor_kind {
        RelationalSuccessorRecipeKind::Singleton => 0x01,
        RelationalSuccessorRecipeKind::FiniteExact => 0x02,
        RelationalSuccessorRecipeKind::FiniteCollection => 0x03,
        RelationalSuccessorRecipeKind::FiniteIntRange => 0x04,
    });
    hasher.u8(artifact.preimage_kind.tag());
    hasher.digest(artifact.case_cell_id.bytes());
    hasher.digest(artifact.case_materializer_id.bytes());
    match artifact.exact_case_cardinality {
        Some(count) => {
            hasher.u8(0x01);
            hasher.u128(count);
        }
        None => hasher.u8(0x02),
    }
    hasher.finish()
}

fn case_image_evidence_binding(
    schema_version: u32,
    certificate_id: [u8; 32],
    role_tag: u8,
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
) -> RelationalCaseImageEvidenceBinding {
    let domain = if schema_version == RELATIONAL_CASE_IMAGE_INJECTIVITY_PROOF_VERSION {
        CASE_IMAGE_INJECTIVITY_PROOF_V2
    } else {
        CASE_IMAGE_INJECTIVITY_PROOF_V1
    };
    let mut hasher = CanonicalPlannerHasher::new(domain);
    hasher.digest(certificate_id);
    hasher.u8(role_tag);
    hasher.digest(obligation_id.bytes());
    hasher.digest(conclusion_digest);
    RelationalCaseImageEvidenceBinding {
        obligation_id,
        conclusion_digest,
        proof_digest: hasher.finish(),
    }
}

fn validate_restored_stages(
    relation_id: RelationId,
    coverage: &RelationalCoverageQualifier,
    stages: &[RelationalBindingStage],
) -> Result<(), RelationalSupportPlannerError> {
    let dependency_shapes = stages
        .iter()
        .map(|stage| {
            let (finite, dependency_key) = match stage {
                RelationalBindingStage::Finite(stage) => (true, stage.recipe.dependency_key()),
                RelationalBindingStage::Singleton(stage) => (false, stage.dependency_key()),
            };
            let binding_index = stage.binding_index();
            for (dependency_index, dependency_stage_id) in dependency_key
                .binding_indices()
                .iter()
                .zip(dependency_key.binding_stage_ids())
            {
                let Ok(dependency_position) = usize::try_from(*dependency_index) else {
                    return Err(RelationalSupportPlannerError::PlanInvariant(
                        "restored dependency index is not representable",
                    ));
                };
                let Some(dependency_stage) = stages.get(dependency_position) else {
                    return Err(RelationalSupportPlannerError::PlanInvariant(
                        "restored dependency names an absent binding stage",
                    ));
                };
                if *dependency_index >= binding_index
                    || dependency_stage.stage_id() != *dependency_stage_id
                {
                    return Err(RelationalSupportPlannerError::PlanInvariant(
                        "restored dependency key disagrees with the stage catalog",
                    ));
                }
            }
            Ok(BindingDependencyShape {
                finite,
                dependencies: dependency_key.binding_indices().into(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lineages = derive_dimension_lineages(&dependency_shapes)?;
    let mut dimensions_by_binding = BTreeMap::<u32, RelationalDimensionId>::new();
    for (stage, lineage) in stages.iter().zip(&lineages) {
        let (finite, dependency_key, input_dimensions) = match stage {
            RelationalBindingStage::Finite(stage) => (
                true,
                stage.recipe.dependency_key(),
                stage.schema.key_dimensions(),
            ),
            RelationalBindingStage::Singleton(stage) => {
                (false, stage.dependency_key(), stage.input_dimensions())
            }
        };
        let binding_index = stage.binding_index();
        let expected_input_dimensions = lineage
            .input_dimensions
            .iter()
            .map(|index| {
                dimensions_by_binding.get(index).copied().ok_or(
                    RelationalSupportPlannerError::PlanInvariant(
                        "restored dimension lineage names an absent finite binding",
                    ),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if input_dimensions != expected_input_dimensions.as_slice() {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "restored input dimensions disagree with finite dependencies",
            ));
        }
        let expected_stage_id = derive_binding_stage_id(
            relation_id,
            coverage.semantic_dependency_digest(),
            binding_index,
            stage.role(),
            finite,
            dependency_key.binding_indices(),
        );
        if stage.stage_id() != expected_stage_id {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "restored binding-stage identity does not match its semantic preimage",
            ));
        }
        if let RelationalBindingStage::Finite(stage) = stage {
            let expected_dimension = derive_dimension_id(expected_stage_id);
            let expected_producer = derive_factor_producer_id(
                relation_id,
                coverage.semantic_dependency_digest(),
                expected_stage_id,
                expected_dimension,
                input_dimensions,
            );
            let expected_materializer = derive_materializer_id(
                FACTOR_MATERIALIZER_V1,
                relation_id,
                expected_producer.bytes(),
            );
            if stage.dimension_id() != expected_dimension
                || stage.recipe.producer_id() != expected_producer
                || stage.recipe.materializer_id() != expected_materializer
            {
                return Err(RelationalSupportPlannerError::PlanInvariant(
                    "restored finite-stage identities do not match their semantic preimages",
                ));
            }
            dimensions_by_binding.insert(binding_index, expected_dimension);
        }
    }
    Ok(())
}

fn derive_support_plan_root(payload: &RelationalSupportPlanPayload) -> RelationalSupportPlanRoot {
    let mut hasher = CanonicalPlannerHasher::new(SUPPORT_PLAN_ROOT_V2);
    hasher.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    hasher.u32(RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION);

    hasher.u8(0x01);
    hasher.digest(payload.relation_id.bytes());
    hasher.u8(0x02);
    hasher.digest(payload.admission_id.bytes());
    hasher.u8(0x03);
    hasher.u128(payload.question_ids.len() as u128);
    for question_id in &payload.question_ids {
        hasher.digest(question_id.bytes());
    }

    hash_coverage_qualifier(&mut hasher, &payload.coverage);

    // A supported projection certificate introduces its own tagged extension
    // to the already versioned plural-question root preimage.
    if let Some(certificate) = &payload.source_image_projection {
        hasher.u8(0x06);
        hash_source_image_projection_certificate(&mut hasher, certificate);
    }

    hasher.u8(0x05);
    hash_uniform_admission_proof_recipe(&mut hasher, &payload.uniform_admission_proof);

    hasher.u8(0x10);
    hasher.u128(payload.stages.len() as u128);
    for stage in &payload.stages {
        hash_binding_stage(&mut hasher, stage);
    }

    hasher.u8(0x20);
    hash_population(&mut hasher, &payload.source_assignments);
    hasher.u8(0x21);
    hash_population(&mut hasher, &payload.source_rows);
    hasher.u8(0x22);
    hash_population(&mut hasher, &payload.successor_coordinates);
    hasher.u8(0x23);
    hash_population(&mut hasher, &payload.cases);

    hasher.u8(0x30);
    hasher.u128(payload.cell_catalog.cells.len() as u128);
    for cell in &payload.cell_catalog.cells {
        hasher.digest(cell.id().bytes());
    }

    hasher.u8(0x40);
    hash_root_obligation_plan(&mut hasher, &payload.root_obligations);
    RelationalSupportPlanRoot(hasher.finish())
}

fn hash_source_image_projection_certificate(
    hasher: &mut CanonicalPlannerHasher,
    certificate: &CheckedExploreSourceImageProjectionCertificate,
) {
    hasher.u32(certificate.version);
    hasher.digest(certificate.relation_id.bytes());
    hasher.digest(certificate.semantic_dependency_digest);
    hasher.digest(certificate.total_construction_digest);
    hasher.u128(certificate.factors.len() as u128);
    for factor in certificate.factors.iter() {
        hasher.u32(factor.binding_index);
        hasher.digest(factor.binder_digest);
        match factor.kind {
            CheckedExploreSourceProjectionFactorKind::AffineIntRange {
                start,
                end_exclusive,
            } => {
                hasher.u8(0x01);
                hasher.bytes(&start.to_le_bytes());
                hasher.bytes(&end_exclusive.to_le_bytes());
            }
            CheckedExploreSourceProjectionFactorKind::ExactFinite { plan_digest } => {
                hasher.u8(0x02);
                hasher.digest(plan_digest);
            }
        }
        hasher.u128(factor.exact_cardinality);
    }
    hasher.u128(certificate.witnesses.len() as u128);
    for witness in certificate.witnesses.iter() {
        hasher.u32(witness.factor_binding_index);
        hasher.u8(match witness.endpoint {
            CheckedExploreSourceProjectionEndpoint::Context => 0x01,
            CheckedExploreSourceProjectionEndpoint::Before => 0x02,
        });
        hasher.u128(witness.path.len() as u128);
        for field in witness.path.iter() {
            hasher.digest(field.owner_digest);
            hasher.u32(field.variant_index);
            hasher.u32(field.field_index);
        }
        match witness.kind {
            CheckedExploreSourceProjectionWitnessKind::Affine {
                coefficient,
                offset,
                output_min,
                output_max,
                proof_digest,
            } => {
                hasher.u8(0x01);
                hasher.bytes(&coefficient.to_le_bytes());
                hasher.bytes(&offset.to_le_bytes());
                hasher.bytes(&output_min.to_le_bytes());
                hasher.bytes(&output_max.to_le_bytes());
                hasher.digest(proof_digest);
            }
            CheckedExploreSourceProjectionWitnessKind::DirectIdentity { plan_digest } => {
                hasher.u8(0x02);
                hasher.digest(plan_digest);
            }
        }
        hasher.digest(witness.witness_id);
    }
    hasher.digest(certificate.certificate_id);
}

fn hash_uniform_admission_proof_recipe(
    hasher: &mut CanonicalPlannerHasher,
    recipe: &RelationalUniformAdmissionProofRecipe,
) {
    match recipe {
        RelationalUniformAdmissionProofRecipe::LiteralConjunction { predicates } => {
            hasher.u8(0x01);
            hasher.u128(predicates.len() as u128);
            for predicate in predicates {
                hasher.u32(predicate.admission_index());
                hasher.u8(match predicate.scope() {
                    ExploreAdmissionScope::Before => 0x01,
                    ExploreAdmissionScope::After => 0x02,
                    ExploreAdmissionScope::Transition => 0x03,
                });
                hasher.u8(u8::from(predicate.value()));
            }
        }
        RelationalUniformAdmissionProofRecipe::Unsupported => hasher.u8(0x02),
    }
}

fn hash_coverage_qualifier(
    hasher: &mut CanonicalPlannerHasher,
    coverage: &RelationalCoverageQualifier,
) {
    hasher.u8(0x04);
    hasher.digest(coverage.semantic_dependency_digest);
    hasher.bytes(coverage.manifest_digest.as_bytes());
    hasher.u8(match coverage.status {
        RelationalCoverageStatus::NoKnownGaps => 0x01,
        RelationalCoverageStatus::HasCoverageGaps => 0x02,
    });
    hasher.u128(coverage.varied_dimensions as u128);
    hasher.u128(coverage.derived_subjects as u128);
    hasher.u128(coverage.conditioned_subjects as u128);
    hasher.u128(coverage.irrelevance_certificates as u128);
    hasher.u128(coverage.coverage_gaps as u128);
}

fn hash_binding_stage(hasher: &mut CanonicalPlannerHasher, stage: &RelationalBindingStage) {
    match stage {
        RelationalBindingStage::Finite(stage) => {
            hasher.u8(0x01);
            hasher.digest(stage.stage_id.bytes());
            hasher.u32(stage.recipe.binding_index);
            hash_binding_role(hasher, stage.role);
            hasher.digest(stage.dimension_id.bytes());

            hasher.u8(0x11);
            hasher.u128(stage.schema.key_dimensions.len() as u128);
            for dimension in &stage.schema.key_dimensions {
                hasher.digest(dimension.bytes());
            }
            hasher.digest(stage.schema.output_dimension.bytes());

            hasher.u8(0x12);
            hash_dependency_key(hasher, &stage.recipe.dependency_key);
            hasher.u8(match stage.recipe.domain_kind {
                RelationalFiniteDomainRecipeKind::CheckedExact => 0x01,
                RelationalFiniteDomainRecipeKind::CheckedCollection => 0x02,
                RelationalFiniteDomainRecipeKind::CheckedIntRange => 0x03,
            });
            hasher.digest(stage.recipe.producer_id.bytes());
            hasher.digest(stage.recipe.materializer_id.bytes());
            match stage.recipe.known_local_cardinality {
                Some(cardinality) => {
                    hasher.u8(0x01);
                    hasher.u128(cardinality);
                }
                None => hasher.u8(0x02),
            }

            hasher.u8(0x13);
            hash_planned_support(hasher, &stage.support);
        }
        RelationalBindingStage::Singleton(stage) => {
            hasher.u8(0x02);
            hasher.digest(stage.stage_id.bytes());
            hasher.u32(stage.binding_index);
            hash_binding_role(hasher, stage.role);
            hash_dependency_key(hasher, &stage.dependency_key);
            hasher.u128(stage.input_dimensions.len() as u128);
            for dimension in &stage.input_dimensions {
                hasher.digest(dimension.bytes());
            }
            // A singleton is a deterministic map and contributes multiplier
            // one rather than a support coordinate.
            hasher.u8(0x14);
            hasher.u128(1);
        }
    }
}

fn hash_binding_role(hasher: &mut CanonicalPlannerHasher, role: ExploreSourceBindingRoleIr) {
    hasher.u8(match role {
        ExploreSourceBindingRoleIr::Auxiliary => 0x01,
        ExploreSourceBindingRoleIr::Context => 0x02,
        ExploreSourceBindingRoleIr::Before => 0x03,
    });
}

fn hash_dependency_key(
    hasher: &mut CanonicalPlannerHasher,
    recipe: &RelationalDependencyKeyRecipe,
) {
    hasher.u128(recipe.binding_indices.len() as u128);
    for index in &recipe.binding_indices {
        hasher.u32(*index);
    }
    hasher.u128(recipe.binding_stage_ids.len() as u128);
    for stage_id in &recipe.binding_stage_ids {
        hasher.digest(stage_id.bytes());
    }
}

fn hash_planned_support(hasher: &mut CanonicalPlannerHasher, support: &RelationalPlannedSupport) {
    match support {
        RelationalPlannedSupport::Cell { cell, exactness } => {
            hasher.u8(0x01);
            hasher.digest(cell.id().bytes());
            hash_support_exactness(hasher, *exactness);
        }
        RelationalPlannedSupport::ExactEmpty { reason } => {
            hasher.u8(0x02);
            hash_support_exactness(hasher, RelationalSupportExactness::StructuralExact(0));
            hash_exact_empty_reason(hasher, *reason);
        }
    }
}

fn hash_support_exactness(
    hasher: &mut CanonicalPlannerHasher,
    exactness: RelationalSupportExactness,
) {
    match exactness {
        RelationalSupportExactness::StructuralExact(cardinality) => {
            hasher.u8(0x01);
            hasher.u128(cardinality);
        }
        RelationalSupportExactness::Open {
            confirmed_lower_bound,
            reason,
        } => {
            hasher.u8(0x02);
            hasher.u128(confirmed_lower_bound);
            hasher.u8(match reason {
                RelationalSupportOpenReason::RuntimeDomain => 0x01,
                RelationalSupportOpenReason::DependentFiberJoin => 0x02,
                RelationalSupportOpenReason::NaturalJoin => 0x03,
                RelationalSupportOpenReason::CoordinateCardinalityExceedsU128 => 0x04,
                RelationalSupportOpenReason::CoordinateCardinalityOverflow => 0x05,
                RelationalSupportOpenReason::MappedImageNeedsEvidence => 0x06,
                RelationalSupportOpenReason::SuccessorFiberSum => 0x07,
            });
        }
    }
}

fn hash_exact_empty_reason(
    hasher: &mut CanonicalPlannerHasher,
    reason: RelationalExactEmptyReason,
) {
    match reason {
        RelationalExactEmptyReason::StaticFiniteDomain { stage_id } => {
            hasher.u8(0x01);
            hasher.digest(stage_id.bytes());
        }
        RelationalExactEmptyReason::EmptyDependencyKeySpace {
            stage_id,
            empty_input_dimension,
        } => {
            hasher.u8(0x02);
            hasher.digest(stage_id.bytes());
            hasher.digest(empty_input_dimension.bytes());
        }
        RelationalExactEmptyReason::EmptyAssignmentFactor { stage_id } => {
            hasher.u8(0x03);
            hasher.digest(stage_id.bytes());
        }
        RelationalExactEmptyReason::StaticSuccessorDomain => hasher.u8(0x04),
        RelationalExactEmptyReason::UpstreamPopulation(population) => {
            hasher.u8(0x05);
            hash_population_kind(hasher, population);
        }
    }
}

fn hash_population(hasher: &mut CanonicalPlannerHasher, population: &RelationalPlannedPopulation) {
    hash_population_kind(hasher, population.kind);
    hash_planned_support(hasher, &population.support);
    match &population.recipe {
        RelationalSupportPopulationRecipe::ExactEmpty { reason } => {
            hasher.u8(0x01);
            hash_exact_empty_reason(hasher, *reason);
        }
        RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells } => {
            hasher.u8(0x02);
            hash_cell_id_sequence(hasher, factor_cells);
        }
        RelationalSupportPopulationRecipe::DependentAssignmentJoin { factor_cells } => {
            hasher.u8(0x03);
            hash_cell_id_sequence(hasher, factor_cells);
        }
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell } => {
            hasher.u8(0x04);
            hasher.digest(assignment_cell.bytes());
        }
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell,
            successor_kind,
        } => {
            hasher.u8(0x05);
            hasher.digest(source_row_cell.bytes());
            hasher.u8(match successor_kind {
                RelationalSuccessorRecipeKind::Singleton => 0x01,
                RelationalSuccessorRecipeKind::FiniteExact => 0x02,
                RelationalSuccessorRecipeKind::FiniteCollection => 0x03,
                RelationalSuccessorRecipeKind::FiniteIntRange => 0x04,
            });
        }
        RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell,
        } => {
            hasher.u8(0x06);
            hasher.digest(successor_coordinate_cell.bytes());
        }
    }
}

fn hash_population_kind(
    hasher: &mut CanonicalPlannerHasher,
    kind: RelationalSupportPopulationKind,
) {
    hasher.u8(match kind {
        RelationalSupportPopulationKind::SourceAssignments => 0x01,
        RelationalSupportPopulationKind::SourceRows => 0x02,
        RelationalSupportPopulationKind::SuccessorCoordinates => 0x03,
        RelationalSupportPopulationKind::Cases => 0x04,
    });
}

fn hash_cell_id_sequence(hasher: &mut CanonicalPlannerHasher, cells: &[SupportCellId]) {
    hasher.u128(cells.len() as u128);
    for cell in cells {
        hasher.digest(cell.bytes());
    }
}

fn hash_root_obligation_plan(
    hasher: &mut CanonicalPlannerHasher,
    root: &RelationalRootObligationPlan,
) {
    match root {
        RelationalRootObligationPlan::ResolvedExactEmpty { admission_id } => {
            hasher.u8(0x01);
            hasher.u128(0);
            hasher.digest(admission_id.bytes());
        }
        RelationalRootObligationPlan::CellBacked {
            root_cell_id,
            descriptors,
        } => {
            hasher.u8(0x02);
            hasher.digest(root_cell_id.bytes());
            hasher.u128(descriptors.len() as u128);
            for descriptor in descriptors {
                hash_obligation_descriptor(hasher, descriptor);
            }
        }
    }
}

fn hash_obligation_descriptor(
    hasher: &mut CanonicalPlannerHasher,
    descriptor: &RelationalStagedObligationDescriptor,
) {
    match descriptor {
        RelationalStagedObligationDescriptor::Root {
            activation,
            obligation,
        } => {
            hasher.u8(0x01);
            hash_obligation_activation(hasher, *activation);
            hasher.u8(match obligation {
                SupportObligationRecord::Cardinality(_) => 0x01,
                SupportObligationRecord::Injectivity(_) => 0x02,
                SupportObligationRecord::Admission(_) => 0x03,
                SupportObligationRecord::Selection(_) => 0x04,
                SupportObligationRecord::UniformValue(_) => 0x05,
                SupportObligationRecord::UniformMechanism(_) => 0x06,
            });
            hasher.digest(obligation.cell_id().bytes());
            hasher.digest(obligation.id().bytes());
        }
        RelationalStagedObligationDescriptor::SelectionOnAdmitted {
            activation,
            question_id,
        } => {
            hasher.u8(0x02);
            hash_obligation_activation(hasher, *activation);
            hasher.digest(question_id.bytes());
        }
    }
}

fn hash_obligation_activation(
    hasher: &mut CanonicalPlannerHasher,
    activation: RelationalObligationActivation,
) {
    match activation {
        RelationalObligationActivation::RootCasePopulation => hasher.u8(0x01),
        RelationalObligationActivation::AdmissionDecision(decision) => {
            hasher.u8(0x02);
            hasher.u8(match decision {
                AdmissionDecision::Rejected => 0x01,
                AdmissionDecision::Admitted => 0x02,
            });
        }
        RelationalObligationActivation::SelectionDecision(decision) => {
            hasher.u8(0x03);
            hasher.u8(match decision {
                SelectionDecision::NotSelected => 0x01,
                SelectionDecision::Selected => 0x02,
            });
        }
    }
}

/// Planner input can only be obtained from the immutable joined checked-query
/// boundary. Its fields are intentionally private so callers cannot pair an IR
/// with independently supplied semantic identities.
pub(crate) struct RelationalSupportPlanner<'a> {
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_ids: &'a [QuestionId],
    query: &'a ExploreQueryIr,
    coverage: &'a CheckedExploreSourceCoverageManifest,
    source_image_projection: Option<&'a CheckedExploreSourceImageProjectionCertificate>,
}

impl<'a> RelationalSupportPlanner<'a> {
    pub(crate) fn from_checked(
        checked: &'a CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalSupportPlannerError> {
        checked
            .closed_query
            .validate()
            .map_err(RelationalSupportPlannerError::InvalidQuery)?;
        if !checked.source_coverage().validate_identity() {
            return Err(RelationalSupportPlannerError::CoverageIdentityInvalid);
        }
        if checked.source_coverage().relation_id != checked.relation_id() {
            return Err(RelationalSupportPlannerError::CoverageRelationMismatch);
        }
        Ok(Self {
            relation_id: checked.relation_id(),
            admission_id: checked.admission_id(),
            question_ids: checked.question_ids(),
            query: checked.closed_query,
            coverage: checked.source_coverage(),
            source_image_projection: checked.source_image_projection(),
        })
    }

    pub(crate) fn plan(&self) -> Result<RelationalSupportPlan, RelationalSupportPlannerError> {
        let uniform_admission_proof = uniform_admission_proof_recipe(self.query)?;
        let dependency_shapes = binding_dependency_shapes(self.query)?;
        let lineages = derive_dimension_lineages(&dependency_shapes)?;
        let mut stages = Vec::with_capacity(self.query.source.bindings.len());
        let mut stage_ids = Vec::with_capacity(self.query.source.bindings.len());
        let mut dimensions_by_binding = BTreeMap::<u32, RelationalDimensionId>::new();
        let mut factor_support_by_binding = BTreeMap::<u32, RelationalPlannedSupport>::new();

        for (position, binding) in self.query.source.bindings.iter().enumerate() {
            let binding_index = u32::try_from(binding.binding_index)
                .map_err(|_| RelationalSupportPlannerError::IndexExceedsU32("source binding"))?;
            let dependency_indices = binding
                .dependencies
                .iter()
                .map(|dependency| {
                    u32::try_from(dependency.binding_index).map_err(|_| {
                        RelationalSupportPlannerError::IndexExceedsU32("source dependency")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let finite = matches!(&binding.kind, ExploreSourceBindingKindIr::Finite { .. });
            let dependency_key =
                dependency_key_recipe(binding_index, &dependency_indices, &stage_ids)?;
            let stage_id = derive_binding_stage_id(
                self.relation_id,
                self.coverage.semantic_dependency_digest,
                binding_index,
                binding.role,
                finite,
                &dependency_indices,
            );
            stage_ids.push(stage_id);
            let input_dimension_bindings = &lineages[position].input_dimensions;
            let input_dimensions = input_dimension_bindings
                .iter()
                .map(|index| {
                    dimensions_by_binding.get(index).copied().ok_or(
                        RelationalSupportPlannerError::DimensionMissing {
                            binding_index,
                            dimension_binding_index: *index,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            match &binding.kind {
                ExploreSourceBindingKindIr::Singleton { .. } => {
                    stages.push(RelationalBindingStage::Singleton(
                        RelationalSingletonMapStage {
                            stage_id,
                            binding_index,
                            role: binding.role,
                            dependency_key,
                            input_dimensions: input_dimensions.into_boxed_slice(),
                        },
                    ));
                }
                ExploreSourceBindingKindIr::Finite { domain } => {
                    let dimension_id = derive_dimension_id(stage_id);
                    dimensions_by_binding.insert(binding_index, dimension_id);
                    let producer_id = derive_factor_producer_id(
                        self.relation_id,
                        self.coverage.semantic_dependency_digest,
                        stage_id,
                        dimension_id,
                        &input_dimensions,
                    );
                    let materializer_id = derive_materializer_id(
                        FACTOR_MATERIALIZER_V1,
                        self.relation_id,
                        producer_id.bytes(),
                    );
                    let mut input_cells = Vec::with_capacity(input_dimension_bindings.len());
                    let mut empty_input_dimension = None;
                    for index in input_dimension_bindings {
                        let input_support = factor_support_by_binding.get(index).ok_or(
                            RelationalSupportPlannerError::DimensionSupportMissing {
                                binding_index,
                                dimension_binding_index: *index,
                            },
                        )?;
                        match input_support.cell_ref() {
                            Some(cell) => input_cells.push(cell.id()),
                            None => {
                                if empty_input_dimension.is_none() {
                                    empty_input_dimension =
                                        Some(*dimensions_by_binding.get(index).ok_or(
                                            RelationalSupportPlannerError::DimensionMissing {
                                                binding_index,
                                                dimension_binding_index: *index,
                                            },
                                        )?);
                                }
                            }
                        }
                    }
                    let domain_cardinality = static_domain_cardinality(domain)?;
                    let support = if domain_cardinality == StaticDomainCardinality::Exact(0) {
                        RelationalPlannedSupport::exact_empty(
                            RelationalExactEmptyReason::StaticFiniteDomain { stage_id },
                        )
                    } else if let Some(empty_input_dimension) = empty_input_dimension {
                        RelationalPlannedSupport::exact_empty(
                            RelationalExactEmptyReason::EmptyDependencyKeySpace {
                                stage_id,
                                empty_input_dimension,
                            },
                        )
                    } else {
                        let (expression, exactness) = if input_dimensions.is_empty() {
                            match domain_cardinality {
                                StaticDomainCardinality::Exact(count) => (
                                    SupportExpr::ordinal_interval(0, count)?,
                                    RelationalSupportExactness::StructuralExact(count),
                                ),
                                StaticDomainCardinality::ExceedsU128 => (
                                    SupportExpr::join_reference(producer_id, input_cells.clone()),
                                    RelationalSupportExactness::Open {
                                        confirmed_lower_bound: 0,
                                        reason: RelationalSupportOpenReason::CoordinateCardinalityExceedsU128,
                                    },
                                ),
                                StaticDomainCardinality::Runtime => (
                                    SupportExpr::join_reference(producer_id, input_cells.clone()),
                                    RelationalSupportExactness::Open {
                                        confirmed_lower_bound: 0,
                                        reason: RelationalSupportOpenReason::RuntimeDomain,
                                    },
                                ),
                            }
                        } else {
                            (
                                SupportExpr::join_reference(producer_id, input_cells.clone()),
                                RelationalSupportExactness::Open {
                                    confirmed_lower_bound: 0,
                                    reason: RelationalSupportOpenReason::DependentFiberJoin,
                                },
                            )
                        };
                        let cell = SupportCell::new(
                            SupportCellSpace::ProducerCoordinates(producer_id),
                            expression,
                            materializer_id,
                        )?;
                        RelationalPlannedSupport::cell(cell, exactness)?
                    };
                    factor_support_by_binding.insert(binding_index, support.clone());
                    let recipe = RelationalFiniteFactorRecipe {
                        binding_index,
                        dependency_key,
                        domain_kind: finite_domain_recipe_kind(domain),
                        producer_id,
                        materializer_id,
                        known_local_cardinality: domain_cardinality.exact(),
                    };
                    stages.push(RelationalBindingStage::Finite(
                        RelationalFiniteFactorStage {
                            stage_id,
                            role: binding.role,
                            dimension_id,
                            schema: RelationalFactorSchema {
                                key_dimensions: input_dimensions.into_boxed_slice(),
                                output_dimension: dimension_id,
                            },
                            support,
                            recipe,
                        },
                    ));
                }
            }
        }

        let finite_stages = stages
            .iter()
            .filter_map(|stage| match stage {
                RelationalBindingStage::Finite(factor) => Some(factor),
                RelationalBindingStage::Singleton(_) => None,
            })
            .collect::<Vec<_>>();
        validate_source_image_projection_recipe(
            self.relation_id,
            &coverage_qualifier(self.coverage),
            &stages,
            self.source_image_projection,
        )?;
        let source_assignment_image_is_proven_injective =
            source_assignment_image_is_structurally_injective(&stages)
                || self.source_image_projection.is_some();
        let empty_assignment_factor = finite_stages
            .iter()
            .find(|factor| factor.support.is_exact_empty())
            .copied();
        let (source_assignments, assignment_producer_id) = if let Some(empty_factor) =
            empty_assignment_factor
        {
            let reason = RelationalExactEmptyReason::EmptyAssignmentFactor {
                stage_id: empty_factor.stage_id,
            };
            (
                exact_empty_population(RelationalSupportPopulationKind::SourceAssignments, reason),
                None,
            )
        } else {
            let factor_cells = finite_stages
                .iter()
                .map(|factor| {
                    factor.cell().map(SupportCell::id).ok_or(
                        RelationalSupportPlannerError::PlannedCellMissing("finite factor"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let assignment_producer_id = derive_assignment_producer_id(
                self.relation_id,
                self.coverage.semantic_dependency_digest,
                &finite_stages,
            )?;
            let assignment_materializer_id = derive_materializer_id(
                ASSIGNMENT_MATERIALIZER_V1,
                self.relation_id,
                assignment_producer_id.bytes(),
            );
            let all_factors_independent_and_exact = finite_stages.iter().all(|factor| {
                factor.schema.key_dimensions.is_empty() && factor.exactness().exact().is_some()
            });
            let (assignment_expression, assignment_exactness, assignment_recipe) = if finite_stages
                .is_empty()
            {
                (
                    SupportExpr::singleton(super::ExploreValue::Unit),
                    RelationalSupportExactness::StructuralExact(1),
                    RelationalSupportPopulationRecipe::IndependentAssignmentProduct {
                        factor_cells: factor_cells.clone().into_boxed_slice(),
                    },
                )
            } else if all_factors_independent_and_exact {
                let product = finite_stages.iter().try_fold(1u128, |product, factor| {
                    product
                        .checked_mul(
                            factor
                                .exactness()
                                .exact()
                                .expect("independent exact factor checked"),
                        )
                        .ok_or(())
                });
                match product {
                    Ok(product) => (
                        SupportExpr::product(
                            finite_stages
                                .iter()
                                .map(|factor| {
                                    factor
                                        .cell()
                                        .expect("exact-empty factors handled")
                                        .expression()
                                        .clone()
                                })
                                .collect(),
                        )?,
                        RelationalSupportExactness::StructuralExact(product),
                        RelationalSupportPopulationRecipe::IndependentAssignmentProduct {
                            factor_cells: factor_cells.clone().into_boxed_slice(),
                        },
                    ),
                    Err(()) => (
                        SupportExpr::join_reference(assignment_producer_id, factor_cells.clone()),
                        RelationalSupportExactness::Open {
                            confirmed_lower_bound: 0,
                            reason: RelationalSupportOpenReason::CoordinateCardinalityOverflow,
                        },
                        RelationalSupportPopulationRecipe::DependentAssignmentJoin {
                            factor_cells: factor_cells.clone().into_boxed_slice(),
                        },
                    ),
                }
            } else {
                (
                    SupportExpr::join_reference(assignment_producer_id, factor_cells.clone()),
                    RelationalSupportExactness::Open {
                        confirmed_lower_bound: 0,
                        reason: RelationalSupportOpenReason::NaturalJoin,
                    },
                    RelationalSupportPopulationRecipe::DependentAssignmentJoin {
                        factor_cells: factor_cells.clone().into_boxed_slice(),
                    },
                )
            };
            let assignment_cell = SupportCell::new(
                SupportCellSpace::ProducerCoordinates(assignment_producer_id),
                assignment_expression,
                assignment_materializer_id,
            )?;
            (
                RelationalPlannedPopulation {
                    kind: RelationalSupportPopulationKind::SourceAssignments,
                    support: RelationalPlannedSupport::cell(assignment_cell, assignment_exactness)?,
                    recipe: assignment_recipe,
                },
                Some(assignment_producer_id),
            )
        };

        let source_rows = if let Some(assignment_cell) = source_assignments.cell() {
            let assignment_producer_id = assignment_producer_id.ok_or(
                RelationalSupportPlannerError::PlannedProducerMissing("source assignments"),
            )?;
            let source_materializer_id = derive_materializer_id(
                SOURCE_IMAGE_MATERIALIZER_V1,
                self.relation_id,
                assignment_producer_id.bytes(),
            );
            let source_cell = SupportCell::new(
                SupportCellSpace::MappedImage {
                    producer_id: assignment_producer_id,
                    target: SupportExtensionalTarget::SourceRows(self.relation_id),
                },
                assignment_cell.expression().clone(),
                source_materializer_id,
            )?;
            let source_exactness = RelationalSupportExactness::Open {
                confirmed_lower_bound: source_cell.cardinality().lower_bound(),
                reason: RelationalSupportOpenReason::MappedImageNeedsEvidence,
            };
            RelationalPlannedPopulation {
                kind: RelationalSupportPopulationKind::SourceRows,
                support: RelationalPlannedSupport::cell(source_cell, source_exactness)?,
                recipe: RelationalSupportPopulationRecipe::SourceRowImage {
                    assignment_cell: assignment_cell.id(),
                },
            }
        } else {
            exact_empty_population(
                RelationalSupportPopulationKind::SourceRows,
                RelationalExactEmptyReason::UpstreamPopulation(
                    RelationalSupportPopulationKind::SourceAssignments,
                ),
            )
        };

        let successor_static_cardinality =
            static_successor_local_cardinality(&self.query.successor.kind)?;
        let (successor_coordinates, successor_producer_id) = if successor_static_cardinality
            == StaticDomainCardinality::Exact(0)
        {
            (
                exact_empty_population(
                    RelationalSupportPopulationKind::SuccessorCoordinates,
                    RelationalExactEmptyReason::StaticSuccessorDomain,
                ),
                None,
            )
        } else if let Some(source_cell) = source_rows.cell() {
            let successor_producer_id = derive_successor_producer_id(
                self.relation_id,
                self.coverage.semantic_dependency_digest,
                source_cell.id(),
                successor_recipe_kind(&self.query.successor.kind),
            );
            // A singleton TO over an exact independent assignment product
            // has a canonical coordinate-preserving composition whenever
            // every varying source coordinate is itself Context or Before.
            // Keep that product visible so a later accepted injectivity
            // proof can lift factor partitions all the way to cases.  All
            // other shapes retain the generic source-row fiber join.
            let composed_singleton_assignment = matches!(
                &self.query.successor.kind,
                ExploreSuccessorKindIr::Singleton { .. }
            )
            .then_some(())
            .filter(|_| source_assignment_image_is_proven_injective)
            .and_then(|_| match source_assignments.recipe() {
                RelationalSupportPopulationRecipe::IndependentAssignmentProduct { .. } => {
                    source_assignments.cell()
                }
                _ => None,
            })
            .filter(|cell| cell.coordinate_cardinality().exact().is_some());
            let successor_expression = composed_singleton_assignment.map_or_else(
                || SupportExpr::join_reference(successor_producer_id, vec![source_cell.id()]),
                |assignment_cell| assignment_cell.expression().clone(),
            );
            let successor_materializer_id = derive_materializer_id(
                if composed_singleton_assignment.is_some() {
                    COMPOSED_SINGLETON_SUCCESSOR_MATERIALIZER_V1
                } else {
                    SUCCESSOR_MATERIALIZER_V1
                },
                self.relation_id,
                successor_producer_id.bytes(),
            );
            let successor_cell = SupportCell::new(
                SupportCellSpace::ProducerCoordinates(successor_producer_id),
                successor_expression,
                successor_materializer_id,
            )?;
            let successor_exactness = composed_singleton_assignment.map_or(
                RelationalSupportExactness::Open {
                    confirmed_lower_bound: 0,
                    reason: RelationalSupportOpenReason::SuccessorFiberSum,
                },
                |assignment_cell| {
                    RelationalSupportExactness::StructuralExact(
                        assignment_cell
                            .coordinate_cardinality()
                            .exact()
                            .expect("composed singleton assignment was required exact"),
                    )
                },
            );
            (
                RelationalPlannedPopulation {
                    kind: RelationalSupportPopulationKind::SuccessorCoordinates,
                    support: RelationalPlannedSupport::cell(successor_cell, successor_exactness)?,
                    recipe: RelationalSupportPopulationRecipe::SuccessorFiberSum {
                        source_row_cell: source_cell.id(),
                        successor_kind: successor_recipe_kind(&self.query.successor.kind),
                    },
                },
                Some(successor_producer_id),
            )
        } else {
            (
                exact_empty_population(
                    RelationalSupportPopulationKind::SuccessorCoordinates,
                    RelationalExactEmptyReason::UpstreamPopulation(
                        RelationalSupportPopulationKind::SourceRows,
                    ),
                ),
                None,
            )
        };

        let cases = if let Some(successor_cell) = successor_coordinates.cell() {
            let successor_producer_id = successor_producer_id.ok_or(
                RelationalSupportPlannerError::PlannedProducerMissing("successor coordinates"),
            )?;
            let case_materializer_id = derive_materializer_id(
                CASE_IMAGE_MATERIALIZER_V1,
                self.relation_id,
                successor_producer_id.bytes(),
            );
            let case_cell = SupportCell::new(
                SupportCellSpace::MappedImage {
                    producer_id: successor_producer_id,
                    target: SupportExtensionalTarget::Cases(self.relation_id),
                },
                successor_cell.expression().clone(),
                case_materializer_id,
            )?;
            let case_exactness = RelationalSupportExactness::Open {
                confirmed_lower_bound: case_cell.cardinality().lower_bound(),
                reason: RelationalSupportOpenReason::MappedImageNeedsEvidence,
            };
            RelationalPlannedPopulation {
                kind: RelationalSupportPopulationKind::Cases,
                support: RelationalPlannedSupport::cell(case_cell, case_exactness)?,
                recipe: RelationalSupportPopulationRecipe::CaseImage {
                    successor_coordinate_cell: successor_cell.id(),
                },
            }
        } else {
            exact_empty_population(
                RelationalSupportPopulationKind::Cases,
                RelationalExactEmptyReason::UpstreamPopulation(
                    RelationalSupportPopulationKind::SuccessorCoordinates,
                ),
            )
        };

        let root_obligations = if let Some(case_cell) = cases.cell() {
            let cardinality = SupportObligationRecord::Cardinality(SupportCellObligation::new(
                case_cell,
                ExactCardinalityClaim,
            )?);
            let injectivity = SupportObligationRecord::Injectivity(SupportCellObligation::new(
                case_cell,
                InjectiveMappingClaim::new(case_cell.materializer_id()),
            )?);
            let admission = SupportObligationRecord::Admission(SupportCellObligation::new(
                case_cell,
                AdmissionClassificationClaim::new(self.admission_id),
            )?);
            RelationalRootObligationPlan::CellBacked {
                root_cell_id: case_cell.id(),
                descriptors: vec![
                    RelationalStagedObligationDescriptor::Root {
                        activation: RelationalObligationActivation::RootCasePopulation,
                        obligation: cardinality,
                    },
                    RelationalStagedObligationDescriptor::Root {
                        activation: RelationalObligationActivation::RootCasePopulation,
                        obligation: injectivity,
                    },
                    RelationalStagedObligationDescriptor::Root {
                        activation: RelationalObligationActivation::RootCasePopulation,
                        obligation: admission,
                    },
                ]
                .into_iter()
                .chain(self.question_ids.iter().copied().map(|question_id| {
                    RelationalStagedObligationDescriptor::SelectionOnAdmitted {
                        activation: RelationalObligationActivation::AdmissionDecision(
                            AdmissionDecision::Admitted,
                        ),
                        question_id,
                    }
                }))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            }
        } else {
            RelationalRootObligationPlan::ResolvedExactEmpty {
                admission_id: self.admission_id,
            }
        };
        let cell_catalog = build_cell_catalog(
            &stages,
            [
                &source_assignments,
                &source_rows,
                &successor_coordinates,
                &cases,
            ],
            &root_obligations,
        )?;

        Ok(RelationalSupportPlan::from_payload(
            RelationalSupportPlanPayload {
                relation_id: self.relation_id,
                admission_id: self.admission_id,
                question_ids: self.question_ids.to_vec().into_boxed_slice(),
                uniform_admission_proof,
                stages: stages.into_boxed_slice(),
                source_assignments,
                source_rows,
                successor_coordinates,
                cases,
                cell_catalog,
                root_obligations,
                coverage: coverage_qualifier(self.coverage),
                source_image_projection: self.source_image_projection.cloned(),
            },
        ))
    }
}

fn uniform_admission_proof_recipe(
    query: &ExploreQueryIr,
) -> Result<RelationalUniformAdmissionProofRecipe, RelationalSupportPlannerError> {
    let mut predicates = Vec::with_capacity(query.admissions.len());
    for admission in &query.admissions {
        let ExprKind::Lit(Literal::Bool(value)) = &admission.predicate.kind else {
            return Ok(RelationalUniformAdmissionProofRecipe::Unsupported);
        };
        let admission_index = u32::try_from(admission.admission_index)
            .map_err(|_| RelationalSupportPlannerError::IndexExceedsU32("admission"))?;
        predicates.push(RelationalLiteralAdmissionPredicate {
            admission_index,
            scope: admission.scope,
            value: *value,
        });
    }
    RelationalUniformAdmissionProofRecipe::restore_literal_conjunction_from_journal_codec(
        predicates.into_boxed_slice(),
    )
}

fn exact_empty_population(
    kind: RelationalSupportPopulationKind,
    reason: RelationalExactEmptyReason,
) -> RelationalPlannedPopulation {
    RelationalPlannedPopulation {
        kind,
        support: RelationalPlannedSupport::exact_empty(reason),
        recipe: RelationalSupportPopulationRecipe::ExactEmpty { reason },
    }
}

fn build_cell_catalog<'a>(
    stages: &'a [RelationalBindingStage],
    populations: impl IntoIterator<Item = &'a RelationalPlannedPopulation>,
    root_obligations: &RelationalRootObligationPlan,
) -> Result<RelationalSupportCellCatalog, RelationalSupportPlannerError> {
    let populations = populations.into_iter().collect::<Vec<_>>();
    let mut cells = Vec::new();
    for stage in stages {
        if let RelationalBindingStage::Finite(factor) = stage {
            if let Some(cell) = factor.cell() {
                if cell.space() != SupportCellSpace::ProducerCoordinates(factor.recipe.producer_id)
                    || cell.materializer_id() != factor.recipe.materializer_id
                {
                    return Err(RelationalSupportPlannerError::PlanInvariant(
                        "finite factor cell disagrees with its producer or materializer recipe",
                    ));
                }
                cells.push(cell.clone());
            }
        }
    }
    for population in &populations {
        if let Some(cell) = population.cell() {
            cells.push(cell.clone());
        }
    }

    let catalog = RelationalSupportCellCatalog::from_cells(cells)?;
    for cell in catalog.cells() {
        validate_expression_catalog_references(cell.expression(), &catalog)?;
    }
    for population in &populations {
        validate_population_catalog_references(population, &catalog)?;
    }
    let cases = populations
        .iter()
        .copied()
        .find(|population| population.kind == RelationalSupportPopulationKind::Cases)
        .ok_or(RelationalSupportPlannerError::PlanInvariant(
            "cell catalog requires a case population",
        ))?;
    match root_obligations {
        RelationalRootObligationPlan::ResolvedExactEmpty { .. } => {
            if !cases.support.is_exact_empty() {
                return Err(RelationalSupportPlannerError::PlanInvariant(
                    "an exact-empty root must match an exact-empty case population at cardinality zero",
                ));
            }
        }
        RelationalRootObligationPlan::CellBacked {
            root_cell_id,
            descriptors,
        } => {
            require_catalog_cell(&catalog, *root_cell_id, "root obligation")?;
            if cases.cell().map(SupportCell::id) != Some(*root_cell_id) {
                return Err(RelationalSupportPlannerError::PlanInvariant(
                    "a cell-backed root must name the case population cell",
                ));
            }
            for descriptor in descriptors {
                if let RelationalStagedObligationDescriptor::Root { obligation, .. } = descriptor {
                    require_catalog_cell(
                        &catalog,
                        obligation.cell_id(),
                        "root obligation descriptor",
                    )?;
                    if obligation.cell_id() != *root_cell_id {
                        return Err(RelationalSupportPlannerError::PlanInvariant(
                            "root obligation descriptor must name the root case cell",
                        ));
                    }
                }
            }
        }
    }
    Ok(catalog)
}

fn validate_expression_catalog_references(
    expression: &SupportExpr,
    catalog: &RelationalSupportCellCatalog,
) -> Result<(), RelationalSupportPlannerError> {
    match expression.kind() {
        SupportExprKind::JoinReference { inputs, .. } => {
            for input in inputs {
                require_catalog_cell(catalog, *input, "join expression")?;
            }
        }
        SupportExprKind::Product(factors)
        | SupportExprKind::ProductRankInterval { factors, .. }
        | SupportExprKind::Union(factors) => {
            for factor in factors {
                validate_expression_catalog_references(factor, catalog)?;
            }
        }
        SupportExprKind::Difference {
            minuend,
            subtrahend,
        } => {
            validate_expression_catalog_references(minuend, catalog)?;
            validate_expression_catalog_references(subtrahend, catalog)?;
        }
        SupportExprKind::Singleton(_)
        | SupportExprKind::FiniteEnum(_)
        | SupportExprKind::OrdinalInterval { .. }
        | SupportExprKind::OrdinalCongruence { .. } => {}
    }
    Ok(())
}

fn validate_population_catalog_references(
    population: &RelationalPlannedPopulation,
    catalog: &RelationalSupportCellCatalog,
) -> Result<(), RelationalSupportPlannerError> {
    match (&population.support, &population.recipe) {
        (
            RelationalPlannedSupport::ExactEmpty {
                reason: support_reason,
            },
            RelationalSupportPopulationRecipe::ExactEmpty {
                reason: recipe_reason,
            },
        ) if support_reason == recipe_reason => return Ok(()),
        (RelationalPlannedSupport::ExactEmpty { .. }, _)
        | (
            RelationalPlannedSupport::Cell { .. },
            RelationalSupportPopulationRecipe::ExactEmpty { .. },
        ) => {
            return Err(RelationalSupportPlannerError::PlanInvariant(
                "population support and recipe disagree about exact emptiness",
            ));
        }
        (RelationalPlannedSupport::Cell { .. }, _) => {}
    }

    match &population.recipe {
        RelationalSupportPopulationRecipe::ExactEmpty { .. } => {
            unreachable!("exact-empty recipe returned above")
        }
        RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells }
        | RelationalSupportPopulationRecipe::DependentAssignmentJoin { factor_cells } => {
            for factor_cell in factor_cells {
                require_catalog_cell(catalog, *factor_cell, "assignment recipe")?;
            }
        }
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell } => {
            require_catalog_cell(catalog, *assignment_cell, "source-row recipe")?;
        }
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell, ..
        } => {
            require_catalog_cell(catalog, *source_row_cell, "successor recipe")?;
        }
        RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell,
        } => {
            require_catalog_cell(catalog, *successor_coordinate_cell, "case recipe")?;
        }
    }
    Ok(())
}

fn require_catalog_cell(
    catalog: &RelationalSupportCellCatalog,
    cell_id: SupportCellId,
    referenced_by: &'static str,
) -> Result<(), RelationalSupportPlannerError> {
    if catalog.contains(cell_id) {
        Ok(())
    } else {
        Err(RelationalSupportPlannerError::CatalogCellMissing {
            referenced_by,
            cell_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingDependencyShape {
    finite: bool,
    dependencies: Box<[u32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingDimensionLineage {
    input_dimensions: BTreeSet<u32>,
    value_dimensions: BTreeSet<u32>,
}

fn binding_dependency_shapes(
    query: &ExploreQueryIr,
) -> Result<Vec<BindingDependencyShape>, RelationalSupportPlannerError> {
    query
        .source
        .bindings
        .iter()
        .map(|binding| {
            Ok(BindingDependencyShape {
                finite: matches!(&binding.kind, ExploreSourceBindingKindIr::Finite { .. }),
                dependencies: binding
                    .dependencies
                    .iter()
                    .map(|dependency| {
                        u32::try_from(dependency.binding_index).map_err(|_| {
                            RelationalSupportPlannerError::IndexExceedsU32("source dependency")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            })
        })
        .collect()
}

fn dependency_key_recipe(
    binding_index: u32,
    dependency_indices: &[u32],
    prior_stage_ids: &[RelationalBindingStageId],
) -> Result<RelationalDependencyKeyRecipe, RelationalSupportPlannerError> {
    let binding_stage_ids = dependency_indices
        .iter()
        .map(|index| {
            prior_stage_ids.get(*index as usize).copied().ok_or(
                RelationalSupportPlannerError::DependencyStageMissing {
                    binding_index,
                    dependency_index: *index,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelationalDependencyKeyRecipe {
        binding_indices: dependency_indices.to_vec().into_boxed_slice(),
        binding_stage_ids: binding_stage_ids.into_boxed_slice(),
    })
}

/// Compute the varied dimensions capable of changing every binding value.
/// The result is independent of unrelated prefix order and is deliberately
/// small enough to test without constructing parser or checker artifacts.
fn derive_dimension_lineages(
    shapes: &[BindingDependencyShape],
) -> Result<Vec<BindingDimensionLineage>, RelationalSupportPlannerError> {
    let mut lineages = Vec::<BindingDimensionLineage>::with_capacity(shapes.len());
    for (position, shape) in shapes.iter().enumerate() {
        let binding_index = u32::try_from(position)
            .map_err(|_| RelationalSupportPlannerError::IndexExceedsU32("source binding"))?;
        let mut input_dimensions = BTreeSet::new();
        let mut previous = None;
        for dependency in shape.dependencies.iter().copied() {
            if dependency >= binding_index
                || previous.is_some_and(|previous| dependency <= previous)
            {
                return Err(RelationalSupportPlannerError::NonCanonicalDependency {
                    binding_index,
                    dependency_index: dependency,
                });
            }
            let lineage = lineages.get(dependency as usize).ok_or(
                RelationalSupportPlannerError::DependencyStageMissing {
                    binding_index,
                    dependency_index: dependency,
                },
            )?;
            input_dimensions.extend(lineage.value_dimensions.iter().copied());
            previous = Some(dependency);
        }
        let mut value_dimensions = input_dimensions.clone();
        if shape.finite {
            value_dimensions.insert(binding_index);
        }
        lineages.push(BindingDimensionLineage {
            input_dimensions,
            value_dimensions,
        });
    }
    Ok(lineages)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticDomainCardinality {
    Exact(u128),
    ExceedsU128,
    Runtime,
}

impl StaticDomainCardinality {
    const fn exact(self) -> Option<u128> {
        match self {
            Self::Exact(value) => Some(value),
            Self::ExceedsU128 | Self::Runtime => None,
        }
    }
}

/// Exact value-space facts recovered from an authored checked `IntRange`.
///
/// This is deliberately narrower than general constant folding. Only the
/// closed integer-literal arithmetic fragment is accepted, with every
/// operation checked under Futuruna's `i64` runtime semantics. A caller may
/// use this fact to plan ordinal support without evaluating or sampling the
/// range at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalStaticIntRange {
    start: i64,
    end_exclusive: i64,
    cardinality: u128,
}

impl RelationalStaticIntRange {
    pub(crate) const fn start(self) -> i64 {
        self.start
    }

    pub(crate) const fn end_exclusive(self) -> i64 {
        self.end_exclusive
    }

    pub(crate) const fn cardinality(self) -> u128 {
        self.cardinality
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticIntegerExpression {
    Value(i64),
    NonGround,
}

/// Statically evaluate an authored checked integer range when both endpoints
/// belong to the closed intrinsic integer fragment.
///
/// `Ok(None)` means that runtime/fiber evaluation is still required. Arithmetic
/// overflow, division or remainder by zero, and reversed endpoints are errors:
/// none of those conditions may silently degrade into apparently open support.
pub(crate) fn statically_evaluate_checked_int_range(
    domain: &ExploreFiniteDomainIr,
) -> Result<Option<RelationalStaticIntRange>, RelationalSupportPlannerError> {
    let ExploreFiniteDomainIr::IntRange {
        start,
        end_exclusive,
    } = domain
    else {
        return Ok(None);
    };
    let start = evaluate_static_integer_endpoint(start).map_err(|message| {
        RelationalSupportPlannerError::InvalidExactDomain(format!(
            "authored integer-range start endpoint {message}"
        ))
    })?;
    let end_exclusive = evaluate_static_integer_endpoint(end_exclusive).map_err(|message| {
        RelationalSupportPlannerError::InvalidExactDomain(format!(
            "authored integer-range end endpoint {message}"
        ))
    })?;
    let (StaticIntegerExpression::Value(start), StaticIntegerExpression::Value(end_exclusive)) =
        (start, end_exclusive)
    else {
        return Ok(None);
    };
    if start > end_exclusive {
        return Err(RelationalSupportPlannerError::InvalidExactDomain(format!(
            "authored integer range starts after its end: {start}..{end_exclusive}"
        )));
    }
    let cardinality =
        u128::try_from(i128::from(end_exclusive) - i128::from(start)).map_err(|_| {
            RelationalSupportPlannerError::InvalidExactDomain(format!(
                "authored integer range {start}..{end_exclusive} has unrepresentable cardinality"
            ))
        })?;
    Ok(Some(RelationalStaticIntRange {
        start,
        end_exclusive,
        cardinality,
    }))
}

fn evaluate_static_integer_endpoint(
    expression: &Expr,
) -> Result<StaticIntegerExpression, &'static str> {
    match &expression.kind {
        ExprKind::Lit(Literal::Int(value)) => Ok(StaticIntegerExpression::Value(*value)),
        ExprKind::UnOp(operator, inner) if operator == "+" => {
            evaluate_static_integer_endpoint(inner)
        }
        ExprKind::UnOp(operator, inner) if operator == "-" => {
            match evaluate_static_integer_endpoint(inner)? {
                StaticIntegerExpression::Value(value) => value
                    .checked_neg()
                    .map(StaticIntegerExpression::Value)
                    .ok_or("overflows during negation"),
                StaticIntegerExpression::NonGround => Ok(StaticIntegerExpression::NonGround),
            }
        }
        ExprKind::BinOp(operator, left, right)
            if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%") =>
        {
            let left = evaluate_static_integer_endpoint(left)?;
            let right = evaluate_static_integer_endpoint(right)?;
            if matches!(operator.as_str(), "/" | "%") && right == StaticIntegerExpression::Value(0)
            {
                return Err(if operator.as_str() == "/" {
                    "divides by zero"
                } else {
                    "takes a remainder by zero"
                });
            }
            let (StaticIntegerExpression::Value(left), StaticIntegerExpression::Value(right)) =
                (left, right)
            else {
                return Ok(StaticIntegerExpression::NonGround);
            };
            let value = match operator.as_str() {
                "+" => left.checked_add(right).ok_or("overflows during addition")?,
                "-" => left
                    .checked_sub(right)
                    .ok_or("overflows during subtraction")?,
                "*" => left
                    .checked_mul(right)
                    .ok_or("overflows during multiplication")?,
                "/" => left.checked_div(right).ok_or("overflows during division")?,
                "%" => left
                    .checked_rem(right)
                    .ok_or("overflows during remainder")?,
                _ => unreachable!("guarded checked integer operator"),
            };
            Ok(StaticIntegerExpression::Value(value))
        }
        _ => Ok(StaticIntegerExpression::NonGround),
    }
}

fn static_domain_cardinality(
    domain: &ExploreFiniteDomainIr,
) -> Result<StaticDomainCardinality, RelationalSupportPlannerError> {
    match domain {
        ExploreFiniteDomainIr::IntRange { .. } => {
            Ok(statically_evaluate_checked_int_range(domain)?
                .map(|range| StaticDomainCardinality::Exact(range.cardinality()))
                .unwrap_or(StaticDomainCardinality::Runtime))
        }
        ExploreFiniteDomainIr::Collection { .. } => Ok(StaticDomainCardinality::Runtime),
        ExploreFiniteDomainIr::Exact(ExploreExactDomain::Enumerated { values, .. }) => {
            let unique = values.iter().collect::<BTreeSet<_>>();
            let count = u128::try_from(unique.len()).map_err(|_| {
                RelationalSupportPlannerError::IndexExceedsU32("finite enumeration length")
            })?;
            Ok(StaticDomainCardinality::Exact(count))
        }
        ExploreFiniteDomainIr::Exact(ExploreExactDomain::IntRange {
            start,
            end_exclusive,
            cardinality,
        }) => {
            if start > end_exclusive {
                return Err(RelationalSupportPlannerError::InvalidExactDomain(
                    "integer range starts after its end".to_string(),
                ));
            }
            let implied =
                u128::try_from(i128::from(*end_exclusive) - i128::from(*start)).map_err(|_| {
                    RelationalSupportPlannerError::InvalidExactDomain(
                        "integer range cardinality is not representable".to_string(),
                    )
                })?;
            if implied != u128::from(*cardinality) {
                return Err(RelationalSupportPlannerError::InvalidExactDomain(format!(
                    "integer range declares cardinality {cardinality} but its endpoints imply {implied}"
                )));
            }
            Ok(StaticDomainCardinality::Exact(implied))
        }
        ExploreFiniteDomainIr::Exact(ExploreExactDomain::FiniteType { plan, .. }) => {
            match plan.cardinality() {
                ExploreCardinality::Exact(value) => Ok(StaticDomainCardinality::Exact(value)),
                ExploreCardinality::ExceedsU128 => Ok(StaticDomainCardinality::ExceedsU128),
            }
        }
    }
}

fn static_successor_local_cardinality(
    kind: &ExploreSuccessorKindIr,
) -> Result<StaticDomainCardinality, RelationalSupportPlannerError> {
    match kind {
        ExploreSuccessorKindIr::Singleton { .. } => Ok(StaticDomainCardinality::Exact(1)),
        ExploreSuccessorKindIr::Finite { domain } => static_domain_cardinality(domain),
    }
}

fn finite_domain_recipe_kind(domain: &ExploreFiniteDomainIr) -> RelationalFiniteDomainRecipeKind {
    match domain {
        ExploreFiniteDomainIr::Exact(_) => RelationalFiniteDomainRecipeKind::CheckedExact,
        ExploreFiniteDomainIr::Collection { .. } => {
            RelationalFiniteDomainRecipeKind::CheckedCollection
        }
        ExploreFiniteDomainIr::IntRange { .. } => RelationalFiniteDomainRecipeKind::CheckedIntRange,
    }
}

fn successor_recipe_kind(kind: &ExploreSuccessorKindIr) -> RelationalSuccessorRecipeKind {
    match kind {
        ExploreSuccessorKindIr::Singleton { .. } => RelationalSuccessorRecipeKind::Singleton,
        ExploreSuccessorKindIr::Finite {
            domain: ExploreFiniteDomainIr::Exact(_),
        } => RelationalSuccessorRecipeKind::FiniteExact,
        ExploreSuccessorKindIr::Finite {
            domain: ExploreFiniteDomainIr::Collection { .. },
        } => RelationalSuccessorRecipeKind::FiniteCollection,
        ExploreSuccessorKindIr::Finite {
            domain: ExploreFiniteDomainIr::IntRange { .. },
        } => RelationalSuccessorRecipeKind::FiniteIntRange,
    }
}

fn coverage_qualifier(
    manifest: &CheckedExploreSourceCoverageManifest,
) -> RelationalCoverageQualifier {
    let mut varied_dimensions = 0usize;
    let mut derived_subjects = 0usize;
    let mut conditioned_subjects = 0usize;
    let mut irrelevance_certificates = 0usize;
    let mut coverage_gaps = 0usize;
    for entry in manifest.entries.iter() {
        match &entry.classification {
            CheckedExploreCoverageClassification::VariedFiniteDimension { .. } => {
                varied_dimensions += 1;
            }
            CheckedExploreCoverageClassification::DerivedFromDeclaredDimensions { .. } => {
                derived_subjects += 1;
            }
            CheckedExploreCoverageClassification::ConditionedSingletonOrSourceRestriction => {
                conditioned_subjects += 1;
            }
            CheckedExploreCoverageClassification::ExactIrrelevanceCertificate { .. } => {
                irrelevance_certificates += 1;
            }
            CheckedExploreCoverageClassification::CoverageGap { .. } => {
                coverage_gaps += 1;
            }
        }
    }
    RelationalCoverageQualifier {
        status: if coverage_gaps == 0 {
            RelationalCoverageStatus::NoKnownGaps
        } else {
            RelationalCoverageStatus::HasCoverageGaps
        },
        manifest_digest: manifest.manifest_digest.clone(),
        semantic_dependency_digest: manifest.semantic_dependency_digest,
        varied_dimensions,
        derived_subjects,
        conditioned_subjects,
        irrelevance_certificates,
        coverage_gaps,
    }
}

fn derive_binding_stage_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    binding_index: u32,
    role: ExploreSourceBindingRoleIr,
    finite: bool,
    dependencies: &[u32],
) -> RelationalBindingStageId {
    let mut hasher = CanonicalPlannerHasher::new(BINDING_STAGE_ID_V1);
    hasher.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    hasher.digest(relation_id.bytes());
    hasher.digest(source_dependency_digest);
    hasher.u32(binding_index);
    hasher.u8(match role {
        ExploreSourceBindingRoleIr::Auxiliary => 0x01,
        ExploreSourceBindingRoleIr::Context => 0x02,
        ExploreSourceBindingRoleIr::Before => 0x03,
    });
    hasher.u8(if finite { 0x01 } else { 0x02 });
    hasher.u64(dependencies.len() as u64);
    for dependency in dependencies {
        hasher.u32(*dependency);
    }
    RelationalBindingStageId(hasher.finish())
}

fn derive_dimension_id(stage_id: RelationalBindingStageId) -> RelationalDimensionId {
    let mut hasher = CanonicalPlannerHasher::new(DIMENSION_ID_V1);
    hasher.digest(stage_id.bytes());
    RelationalDimensionId(hasher.finish())
}

fn derive_factor_producer_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    stage_id: RelationalBindingStageId,
    dimension_id: RelationalDimensionId,
    key_dimensions: &[RelationalDimensionId],
) -> SupportProducerId {
    let mut preimage = CanonicalPlannerBytes::new(FACTOR_PRODUCER_V1);
    preimage.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(source_dependency_digest);
    preimage.digest(stage_id.bytes());
    preimage.digest(dimension_id.bytes());
    preimage.u64(key_dimensions.len() as u64);
    for dimension in key_dimensions {
        preimage.digest(dimension.bytes());
    }
    SupportProducerId::from_canonical_preimage(preimage.as_slice())
}

fn derive_assignment_producer_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    factors: &[&RelationalFiniteFactorStage],
) -> Result<SupportProducerId, RelationalSupportPlannerError> {
    let mut preimage = CanonicalPlannerBytes::new(ASSIGNMENT_PRODUCER_V1);
    preimage.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(source_dependency_digest);
    preimage.u64(factors.len() as u64);
    for factor in factors {
        preimage.digest(factor.stage_id.bytes());
        preimage.digest(factor.dimension_id.bytes());
        preimage.digest(
            factor
                .cell()
                .ok_or(RelationalSupportPlannerError::PlannedCellMissing(
                    "assignment factor",
                ))?
                .id()
                .bytes(),
        );
    }
    Ok(SupportProducerId::from_canonical_preimage(
        preimage.as_slice(),
    ))
}

fn derive_successor_producer_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    source_cell_id: SupportCellId,
    kind: RelationalSuccessorRecipeKind,
) -> SupportProducerId {
    let mut preimage = CanonicalPlannerBytes::new(SUCCESSOR_PRODUCER_V1);
    preimage.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(source_dependency_digest);
    preimage.digest(source_cell_id.bytes());
    preimage.u8(match kind {
        RelationalSuccessorRecipeKind::Singleton => 0x01,
        RelationalSuccessorRecipeKind::FiniteExact => 0x02,
        RelationalSuccessorRecipeKind::FiniteCollection => 0x03,
        RelationalSuccessorRecipeKind::FiniteIntRange => 0x04,
    });
    SupportProducerId::from_canonical_preimage(preimage.as_slice())
}

fn derive_materializer_id(
    domain: &[u8],
    relation_id: RelationId,
    producer_digest: [u8; 32],
) -> SupportMaterializerId {
    let mut preimage = CanonicalPlannerBytes::new(domain);
    preimage.u32(RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(producer_digest);
    SupportMaterializerId::from_canonical_preimage(preimage.as_slice())
}

struct CanonicalPlannerBytes(Vec<u8>);

impl CanonicalPlannerBytes {
    fn new(domain: &[u8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(domain.len() as u64).to_be_bytes());
        bytes.extend_from_slice(domain);
        Self(bytes)
    }

    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.extend_from_slice(&digest);
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

struct CanonicalPlannerHasher(Sha256);

impl CanonicalPlannerHasher {
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

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u128(value.len() as u128);
        self.0.update(value);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSupportPlannerError {
    InvalidQuery(String),
    CoverageIdentityInvalid,
    CoverageRelationMismatch,
    IndexExceedsU32(&'static str),
    NonCanonicalDependency {
        binding_index: u32,
        dependency_index: u32,
    },
    DependencyStageMissing {
        binding_index: u32,
        dependency_index: u32,
    },
    DimensionMissing {
        binding_index: u32,
        dimension_binding_index: u32,
    },
    DimensionSupportMissing {
        binding_index: u32,
        dimension_binding_index: u32,
    },
    PlannedCellMissing(&'static str),
    PlannedProducerMissing(&'static str),
    SupportCellIdCollision(SupportCellId),
    CatalogCellMissing {
        referenced_by: &'static str,
        cell_id: SupportCellId,
    },
    PlanInvariant(&'static str),
    InvalidExactDomain(String),
    SupportCell(SupportCellError),
}

impl From<SupportCellError> for RelationalSupportPlannerError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl fmt::Display for RelationalSupportPlannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => {
                write!(
                    formatter,
                    "invalid checked relational support query: {message}"
                )
            }
            Self::CoverageIdentityInvalid => {
                formatter.write_str("checked source-coverage manifest identity is invalid")
            }
            Self::CoverageRelationMismatch => formatter
                .write_str("checked source-coverage manifest belongs to a different relation"),
            Self::IndexExceedsU32(subject) => {
                write!(formatter, "{subject} does not fit the durable u32 schema")
            }
            Self::NonCanonicalDependency {
                binding_index,
                dependency_index,
            } => write!(
                formatter,
                "binding {binding_index} has non-canonical dependency {dependency_index}"
            ),
            Self::DependencyStageMissing {
                binding_index,
                dependency_index,
            } => write!(
                formatter,
                "binding {binding_index} references absent dependency stage {dependency_index}"
            ),
            Self::DimensionMissing {
                binding_index,
                dimension_binding_index,
            } => write!(
                formatter,
                "binding {binding_index} references absent dimension {dimension_binding_index}"
            ),
            Self::DimensionSupportMissing {
                binding_index,
                dimension_binding_index,
            } => write!(
                formatter,
                "binding {binding_index} references absent factor support {dimension_binding_index}"
            ),
            Self::PlannedCellMissing(subject) => {
                write!(
                    formatter,
                    "{subject} unexpectedly has no planned support cell"
                )
            }
            Self::PlannedProducerMissing(subject) => {
                write!(formatter, "{subject} unexpectedly has no planned producer")
            }
            Self::SupportCellIdCollision(cell_id) => write!(
                formatter,
                "different support cells share catalog id {cell_id:?}"
            ),
            Self::CatalogCellMissing {
                referenced_by,
                cell_id,
            } => write!(
                formatter,
                "{referenced_by} references support cell {cell_id:?} absent from the canonical catalog"
            ),
            Self::PlanInvariant(message) => {
                write!(formatter, "invalid relational support plan: {message}")
            }
            Self::InvalidExactDomain(message) => {
                write!(formatter, "invalid exact support domain: {message}")
            }
            Self::SupportCell(error) => write!(formatter, "invalid planned support cell: {error}"),
        }
    }
}

impl Error for RelationalSupportPlannerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCaseImageInjectivityProofError {
    UnsupportedArtifactVersion(u32),
    InvalidArtifactShape,
    ArtifactIdentityMismatch,
    ArtifactSemanticMismatch,
    InvalidPlanRoot,
    UnsupportedPlanShape(&'static str),
    CellContractMismatch(&'static str),
    InternalProofInvariant(&'static str),
    SourceImage(RelationalSourceImageExactnessProofError),
    Planner(RelationalSupportPlannerError),
    SupportCell(SupportCellError),
}

impl From<RelationalSupportPlannerError> for RelationalCaseImageInjectivityProofError {
    fn from(error: RelationalSupportPlannerError) -> Self {
        Self::Planner(error)
    }
}

impl From<SupportCellError> for RelationalCaseImageInjectivityProofError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl From<RelationalSourceImageExactnessProofError> for RelationalCaseImageInjectivityProofError {
    fn from(error: RelationalSourceImageExactnessProofError) -> Self {
        Self::SourceImage(error)
    }
}

impl fmt::Display for RelationalCaseImageInjectivityProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion(version) => write!(
                formatter,
                "unsupported relational case-image proof artifact version {version}"
            ),
            Self::InvalidArtifactShape => {
                formatter.write_str("invalid relational case-image proof artifact shape")
            }
            Self::ArtifactIdentityMismatch => formatter
                .write_str("relational case-image proof artifact identity does not match content"),
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "relational case-image proof artifact no longer matches its producer plan",
            ),
            Self::InvalidPlanRoot => {
                formatter.write_str("relational support plan root does not match its payload")
            }
            Self::UnsupportedPlanShape(message) => {
                write!(
                    formatter,
                    "unsupported case-image producer shape: {message}"
                )
            }
            Self::CellContractMismatch(subject) => {
                write!(
                    formatter,
                    "{subject} does not match its checked producer contract"
                )
            }
            Self::InternalProofInvariant(message) => {
                write!(formatter, "case-image proof invariant failed: {message}")
            }
            Self::SourceImage(error) => {
                write!(formatter, "invalid composed source-image proof: {error}")
            }
            Self::Planner(error) => write!(formatter, "invalid producer support plan: {error}"),
            Self::SupportCell(error) => write!(formatter, "invalid case-image support: {error}"),
        }
    }
}

impl Error for RelationalCaseImageInjectivityProofError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SourceImage(error) => Some(error),
            Self::Planner(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::relation::FindPolarity;

    fn shape(finite: bool, dependencies: impl IntoIterator<Item = u32>) -> BindingDependencyShape {
        BindingDependencyShape {
            finite,
            dependencies: dependencies
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    fn restored_finite_stage(
        relation_id: RelationId,
        semantic_dependency_digest: [u8; 32],
        binding_index: u32,
        role: ExploreSourceBindingRoleIr,
        dependency_indices: &[u32],
        dependency_stage_ids: &[RelationalBindingStageId],
        input_dimensions: &[RelationalDimensionId],
    ) -> (RelationalBindingStage, RelationalDimensionId) {
        let stage_id = derive_binding_stage_id(
            relation_id,
            semantic_dependency_digest,
            binding_index,
            role,
            true,
            dependency_indices,
        );
        let dimension_id = derive_dimension_id(stage_id);
        let producer_id = derive_factor_producer_id(
            relation_id,
            semantic_dependency_digest,
            stage_id,
            dimension_id,
            input_dimensions,
        );
        let materializer_id =
            derive_materializer_id(FACTOR_MATERIALIZER_V1, relation_id, producer_id.bytes());
        let cell = SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer_id),
            SupportExpr::ordinal_interval(0, 1).unwrap(),
            materializer_id,
        )
        .unwrap();
        (
            RelationalBindingStage::Finite(RelationalFiniteFactorStage {
                stage_id,
                role,
                dimension_id,
                schema: RelationalFactorSchema {
                    key_dimensions: input_dimensions.into(),
                    output_dimension: dimension_id,
                },
                support: RelationalPlannedSupport::cell(
                    cell,
                    RelationalSupportExactness::StructuralExact(1),
                )
                .unwrap(),
                recipe: RelationalFiniteFactorRecipe {
                    binding_index,
                    dependency_key: RelationalDependencyKeyRecipe {
                        binding_indices: dependency_indices.into(),
                        binding_stage_ids: dependency_stage_ids.into(),
                    },
                    domain_kind: RelationalFiniteDomainRecipeKind::CheckedIntRange,
                    producer_id,
                    materializer_id,
                    known_local_cardinality: Some(1),
                },
            }),
            dimension_id,
        )
    }

    fn restored_singleton_stage(
        relation_id: RelationId,
        semantic_dependency_digest: [u8; 32],
        binding_index: u32,
        role: ExploreSourceBindingRoleIr,
        dependency_indices: &[u32],
        dependency_stage_ids: &[RelationalBindingStageId],
        input_dimensions: &[RelationalDimensionId],
    ) -> RelationalBindingStage {
        RelationalBindingStage::Singleton(RelationalSingletonMapStage {
            stage_id: derive_binding_stage_id(
                relation_id,
                semantic_dependency_digest,
                binding_index,
                role,
                false,
                dependency_indices,
            ),
            binding_index,
            role,
            dependency_key: RelationalDependencyKeyRecipe {
                binding_indices: dependency_indices.into(),
                binding_stage_ids: dependency_stage_ids.into(),
            },
            input_dimensions: input_dimensions.into(),
        })
    }

    fn empty_root_stage(binding_index: u32, marker: u8) -> RelationalBindingStage {
        let stage_id = RelationalBindingStageId([marker; 32]);
        let dimension_id = RelationalDimensionId([marker.wrapping_add(1); 32]);
        let producer_id = SupportProducerId::from_canonical_preimage(&[
            b'p',
            marker,
            u8::try_from(binding_index).unwrap(),
        ]);
        let materializer_id = SupportMaterializerId::from_canonical_preimage(&[
            b'm',
            marker,
            u8::try_from(binding_index).unwrap(),
        ]);
        RelationalBindingStage::Finite(RelationalFiniteFactorStage {
            stage_id,
            role: ExploreSourceBindingRoleIr::Auxiliary,
            dimension_id,
            schema: RelationalFactorSchema {
                key_dimensions: Box::new([]),
                output_dimension: dimension_id,
            },
            support: RelationalPlannedSupport::exact_empty(
                RelationalExactEmptyReason::StaticFiniteDomain { stage_id },
            ),
            recipe: RelationalFiniteFactorRecipe {
                binding_index,
                dependency_key: RelationalDependencyKeyRecipe {
                    binding_indices: Box::new([]),
                    binding_stage_ids: Box::new([]),
                },
                domain_kind: RelationalFiniteDomainRecipeKind::CheckedExact,
                producer_id,
                materializer_id,
                known_local_cardinality: Some(0),
            },
        })
    }

    fn empty_root_payload(
        stages: Vec<RelationalBindingStage>,
        cell_catalog: RelationalSupportCellCatalog,
    ) -> RelationalSupportPlanPayload {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"root fixture relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"root fixture admission");
        let question_id = QuestionId::from_canonical_find_preimage(
            admission_id,
            b"root fixture question",
            FindPolarity::All,
        );
        let empty_stage_id = stages
            .first()
            .expect("root fixture has an empty factor")
            .stage_id();
        RelationalSupportPlanPayload {
            relation_id,
            admission_id,
            question_ids: vec![question_id].into_boxed_slice(),
            uniform_admission_proof: RelationalUniformAdmissionProofRecipe::Unsupported,
            stages: stages.into_boxed_slice(),
            source_assignments: exact_empty_population(
                RelationalSupportPopulationKind::SourceAssignments,
                RelationalExactEmptyReason::EmptyAssignmentFactor {
                    stage_id: empty_stage_id,
                },
            ),
            source_rows: exact_empty_population(
                RelationalSupportPopulationKind::SourceRows,
                RelationalExactEmptyReason::UpstreamPopulation(
                    RelationalSupportPopulationKind::SourceAssignments,
                ),
            ),
            successor_coordinates: exact_empty_population(
                RelationalSupportPopulationKind::SuccessorCoordinates,
                RelationalExactEmptyReason::UpstreamPopulation(
                    RelationalSupportPopulationKind::SourceRows,
                ),
            ),
            cases: exact_empty_population(
                RelationalSupportPopulationKind::Cases,
                RelationalExactEmptyReason::UpstreamPopulation(
                    RelationalSupportPopulationKind::SuccessorCoordinates,
                ),
            ),
            cell_catalog,
            root_obligations: RelationalRootObligationPlan::ResolvedExactEmpty { admission_id },
            coverage: RelationalCoverageQualifier {
                status: RelationalCoverageStatus::NoKnownGaps,
                manifest_digest: "root-fixture-manifest".into(),
                semantic_dependency_digest: [0x44; 32],
                varied_dimensions: 2,
                derived_subjects: 1,
                conditioned_subjects: 0,
                irrelevance_certificates: 1,
                coverage_gaps: 0,
            },
            source_image_projection: None,
        }
    }

    fn root_fixture_cell(marker: u8) -> SupportCell {
        let producer_id = SupportProducerId::from_canonical_preimage(&[b'c', b'p', marker]);
        let materializer_id = SupportMaterializerId::from_canonical_preimage(&[b'c', b'm', marker]);
        SupportCell::new(
            SupportCellSpace::ProducerCoordinates(producer_id),
            SupportExpr::singleton(crate::explore::ExploreValue::Int(i64::from(marker))),
            materializer_id,
        )
        .unwrap()
    }

    #[test]
    fn independent_finite_bindings_remain_product_factors() {
        let lineages = derive_dimension_lineages(&[
            shape(true, []),
            shape(true, []),
            shape(false, [0, 1]),
            shape(false, [2]),
        ])
        .unwrap();

        assert!(lineages[0].input_dimensions.is_empty());
        assert_eq!(lineages[0].value_dimensions, BTreeSet::from([0]));
        assert!(lineages[1].input_dimensions.is_empty());
        assert_eq!(lineages[1].value_dimensions, BTreeSet::from([1]));
        assert_eq!(lineages[2].input_dimensions, BTreeSet::from([0, 1]));
        assert_eq!(lineages[2].value_dimensions, BTreeSet::from([0, 1]));
        assert_eq!(lineages[3].value_dimensions, BTreeSet::from([0, 1]));
    }

    #[test]
    fn dependent_fiber_key_excludes_unrelated_prefix_dimension() {
        // A and unrelated U are finite. `derived` depends only on A; the
        // final finite fiber depends only on `derived`. Its semantic input
        // lineage must therefore contain A but not U.
        let lineages = derive_dimension_lineages(&[
            shape(true, []),   // A
            shape(true, []),   // U
            shape(false, [0]), // derived(A)
            shape(true, [2]),  // fiber(derived)
        ])
        .unwrap();

        assert_eq!(lineages[3].input_dimensions, BTreeSet::from([0]));
        assert!(!lineages[3].input_dimensions.contains(&1));
        assert_eq!(lineages[3].value_dimensions, BTreeSet::from([0, 3]));

        let stages = [
            RelationalBindingStageId([0; 32]),
            RelationalBindingStageId([1; 32]),
            RelationalBindingStageId([2; 32]),
        ];
        let key = dependency_key_recipe(3, &[2], &stages).unwrap();
        assert_eq!(key.binding_indices(), &[2]);
        assert_eq!(key.binding_stage_ids(), &[stages[2]]);
    }

    #[test]
    fn restored_stages_preserve_transitive_singleton_dimension_lineage() {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(b"restored transitive lineage");
        let semantic_dependency_digest = [0x8d; 32];
        let coverage = RelationalCoverageQualifier {
            status: RelationalCoverageStatus::NoKnownGaps,
            manifest_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            semantic_dependency_digest,
            varied_dimensions: 2,
            derived_subjects: 3,
            conditioned_subjects: 0,
            irrelevance_certificates: 0,
            coverage_gaps: 0,
        };

        let (distance, distance_dimension) = restored_finite_stage(
            relation_id,
            semantic_dependency_digest,
            0,
            ExploreSourceBindingRoleIr::Auxiliary,
            &[],
            &[],
            &[],
        );
        let distance_stage_id = distance.stage_id();
        let profile = restored_singleton_stage(
            relation_id,
            semantic_dependency_digest,
            1,
            ExploreSourceBindingRoleIr::Auxiliary,
            &[0],
            &[distance_stage_id],
            &[distance_dimension],
        );
        let profile_stage_id = profile.stage_id();
        let (income, income_dimension) = restored_finite_stage(
            relation_id,
            semantic_dependency_digest,
            2,
            ExploreSourceBindingRoleIr::Auxiliary,
            &[],
            &[],
            &[],
        );
        let income_stage_id = income.stage_id();
        let before = restored_singleton_stage(
            relation_id,
            semantic_dependency_digest,
            3,
            ExploreSourceBindingRoleIr::Before,
            &[1, 2],
            &[profile_stage_id, income_stage_id],
            &[distance_dimension, income_dimension],
        );
        let context = restored_singleton_stage(
            relation_id,
            semantic_dependency_digest,
            4,
            ExploreSourceBindingRoleIr::Context,
            &[],
            &[],
            &[],
        );
        let stages = vec![distance, profile, income, before, context];

        validate_restored_stages(relation_id, &coverage, &stages).unwrap();

        let mut missing_transitive_dimension = stages;
        let RelationalBindingStage::Singleton(before) = &mut missing_transitive_dimension[3] else {
            unreachable!();
        };
        before.input_dimensions = Box::new([income_dimension]);
        assert!(matches!(
            validate_restored_stages(relation_id, &coverage, &missing_transitive_dimension),
            Err(RelationalSupportPlannerError::PlanInvariant(
                "restored input dimensions disagree with finite dependencies"
            ))
        ));
    }

    #[test]
    fn exact_empty_support_is_zero_without_a_cell() {
        let stage_id = RelationalBindingStageId([7; 32]);
        let support =
            RelationalPlannedSupport::exact_empty(RelationalExactEmptyReason::StaticFiniteDomain {
                stage_id,
            });
        assert_eq!(
            support.exactness(),
            RelationalSupportExactness::StructuralExact(0)
        );
        assert!(support.cell_ref().is_none());

        let population = exact_empty_population(
            RelationalSupportPopulationKind::SourceAssignments,
            RelationalExactEmptyReason::EmptyAssignmentFactor { stage_id },
        );
        assert_eq!(
            population.exactness(),
            RelationalSupportExactness::StructuralExact(0)
        );
        assert!(population.cell().is_none());
        assert!(matches!(
            population.recipe(),
            RelationalSupportPopulationRecipe::ExactEmpty { .. }
        ));
    }

    #[test]
    fn exact_empty_root_has_no_cell_or_runtime_obligations() {
        let relation_id = RelationId::from_canonical_semantic_preimage(b"empty relation");
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"admission");
        let root = RelationalRootObligationPlan::ResolvedExactEmpty { admission_id };

        assert_eq!(root.root_cell_id(), None);
        assert_eq!(root.resolved_exact_cardinality(), Some(0));
        assert_eq!(root.admission_id(), Some(admission_id));
        assert!(root.descriptors().is_empty());
    }

    #[test]
    fn canonical_catalog_contains_intermediate_recipe_dependencies() {
        let upstream_producer =
            SupportProducerId::from_canonical_preimage(b"catalog upstream producer");
        let upstream_materializer =
            SupportMaterializerId::from_canonical_preimage(b"catalog upstream materializer");
        let upstream = SupportCell::new(
            SupportCellSpace::ProducerCoordinates(upstream_producer),
            SupportExpr::singleton(crate::explore::ExploreValue::Unit),
            upstream_materializer,
        )
        .unwrap();

        let downstream_producer =
            SupportProducerId::from_canonical_preimage(b"catalog downstream producer");
        let downstream_materializer =
            SupportMaterializerId::from_canonical_preimage(b"catalog downstream materializer");
        let downstream = SupportCell::new(
            SupportCellSpace::ProducerCoordinates(downstream_producer),
            SupportExpr::join_reference(downstream_producer, vec![upstream.id()]),
            downstream_materializer,
        )
        .unwrap();

        let complete = RelationalSupportCellCatalog::from_cells(vec![
            downstream.clone(),
            upstream.clone(),
            upstream.clone(),
        ])
        .unwrap();
        assert_eq!(complete.cells().len(), 2);
        assert!(complete
            .cell_ids()
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
        validate_expression_catalog_references(downstream.expression(), &complete).unwrap();

        let incomplete =
            RelationalSupportCellCatalog::from_cells(vec![downstream.clone()]).unwrap();
        assert!(matches!(
            validate_expression_catalog_references(downstream.expression(), &incomplete),
            Err(RelationalSupportPlannerError::CatalogCellMissing { .. })
        ));
    }

    #[test]
    fn plan_root_normalizes_catalog_insertion_order() {
        let first = root_fixture_cell(1);
        let second = root_fixture_cell(2);
        let forward_catalog =
            RelationalSupportCellCatalog::from_cells(vec![first.clone(), second.clone()]).unwrap();
        let reverse_catalog =
            RelationalSupportCellCatalog::from_cells(vec![second, first]).unwrap();
        let forward = RelationalSupportPlan::from_payload(empty_root_payload(
            vec![empty_root_stage(0, 0x51)],
            forward_catalog,
        ));
        let reverse = RelationalSupportPlan::from_payload(empty_root_payload(
            vec![empty_root_stage(0, 0x51)],
            reverse_catalog,
        ));

        assert!(forward.validate_root());
        assert!(reverse.validate_root());
        assert_eq!(forward.root(), reverse.root());

        let incomplete_catalog =
            RelationalSupportCellCatalog::from_cells(vec![root_fixture_cell(1)]).unwrap();
        let incomplete = RelationalSupportPlan::from_payload(empty_root_payload(
            vec![empty_root_stage(0, 0x51)],
            incomplete_catalog,
        ));
        assert_ne!(forward.root(), incomplete.root());
    }

    #[test]
    fn plan_root_commits_semantic_stage_order_and_exact_empty_reasons() {
        let catalog = RelationalSupportCellCatalog::from_cells(Vec::new()).unwrap();
        let original_payload = empty_root_payload(
            vec![empty_root_stage(0, 0x61), empty_root_stage(1, 0x62)],
            catalog,
        );
        let original = RelationalSupportPlan::from_payload(original_payload.clone());

        let mut reordered_payload = original_payload.clone();
        reordered_payload.stages.swap(0, 1);
        let reordered = RelationalSupportPlan::from_payload(reordered_payload);
        assert_ne!(original.root(), reordered.root());

        let mut changed_reason_payload = original_payload;
        changed_reason_payload.cases = exact_empty_population(
            RelationalSupportPopulationKind::Cases,
            RelationalExactEmptyReason::StaticSuccessorDomain,
        );
        let changed_reason = RelationalSupportPlan::from_payload(changed_reason_payload);
        assert_ne!(original.root(), changed_reason.root());

        let mut stale_root = original.clone();
        stale_root.payload.coverage.coverage_gaps = 1;
        assert!(!stale_root.validate_root());
    }
}
