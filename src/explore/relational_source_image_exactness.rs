//! Plan-bound exactness proof for one normalized source-row image.
//!
//! The retained v1 strategy recognizes an independent singleton `Context`
//! followed by one exact finite `Before` factor. V2 additionally composes a
//! compiler-minted separating-projection certificate over an ordered product
//! of independent factors. Each varying coordinate must survive in a distinct
//! `(Context, Before)` constructor-field path as either a checked nonzero
//! affine projection or an exact finite direct identity; unsupported
//! construction or arithmetic remains open.
//!
//! The retained artifact is not proof authority. A decoder may restore its
//! canonical parts and validate its content identity, but typed evidence and a
//! certified population root are available only from re-verification against
//! the complete retained [`RelationalSupportPlan`]. The prover routes issuance
//! through a narrow private `support_cell` gateway and returns both accepted
//! evidence values with a root that excludes the evolving global
//! support-evidence catalog.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;

use sha2::{Digest, Sha256};

use super::relation::RelationId;
use super::relational_ir::ExploreSourceBindingRoleIr;
use super::relational_support_planner::{
    RelationalBindingStage, RelationalBindingStageId, RelationalDimensionId,
    RelationalSupportExactness, RelationalSupportOpenReason, RelationalSupportPlan,
    RelationalSupportPlanRoot, RelationalSupportPopulationKind, RelationalSupportPopulationRecipe,
    RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION, RELATIONAL_SUPPORT_PLANNER_VERSION,
};
use super::support_cell::{
    CertifiedInjective, ExactCardinalityClaim, InjectiveMappingClaim, SupportCell,
    SupportCellClaim, SupportCellError, SupportCellEvidence, SupportCellEvidenceId, SupportCellId,
    SupportCellObligation, SupportCellSpace, SupportExpr, SupportExtensionalTarget,
    SupportMaterializerId, SupportProducerId, SupportProofObligationId,
};
use crate::{CheckedExploreSourceProjectionFactorKind, CheckedExploreSourceProjectionWitnessKind};

const ASSIGNMENT_PRODUCER_V1: &[u8] = b"futuruna.explore.relational-support.assignment-producer.v1";
const ASSIGNMENT_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.assignment-materializer.v1";
const SOURCE_IMAGE_MATERIALIZER_V1: &[u8] =
    b"futuruna.explore.relational-support.source-image-materializer.v1";
const SOURCE_IMAGE_EXACTNESS_CERTIFICATE_V1: &[u8] =
    b"futuruna.explore.relational-source-image-exactness.certificate.v1";
const SOURCE_IMAGE_EXACTNESS_PROOF_V1: &[u8] =
    b"futuruna.explore.relational-source-image-exactness.proof.v1";
const SOURCE_IMAGE_EXACTNESS_CERTIFICATE_V2: &[u8] =
    b"futuruna.explore.relational-source-image-exactness.certificate.v2";
const SOURCE_IMAGE_EXACTNESS_PROOF_V2: &[u8] =
    b"futuruna.explore.relational-source-image-exactness.proof.v2";
const CERTIFIED_SOURCE_POPULATION_ROOT_V1: &[u8] =
    b"futuruna.explore.certified-source-population.root.v1";
const CERTIFIED_SOURCE_POPULATION_ROOT_V2: &[u8] =
    b"futuruna.explore.certified-source-population.root.v2";

pub(crate) const RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1: u32 = 1;
pub(crate) const RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION: u32 = 2;

/// Stable source-population identity derived from one verified producer proof.
///
/// The root commits the complete proof artifact and its two typed evidence
/// identities. It intentionally excludes the mutable global support
/// catalog and remains stable while unrelated admission, FIND, or mechanism
/// evidence is appended. A decoded root has no authority until compared with a
/// freshly reverified [`RelationalSourceImageExactnessProof`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CertifiedSourcePopulationRoot([u8; 32]);

impl CertifiedSourcePopulationRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Cold-replay handle for one exact, support-certified source population.
///
/// This value deliberately names only the immutable plan/proof scope and the
/// two exact typed evidence identities. It excludes the evolving global
/// support catalog, and it carries no authority unless reminted by replaying
/// the retained proof artifact against the installed support plan and
/// exact-matching both durable evidence records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CertifiedSourcePopulationShape {
    DirectBeforeFactor {
        context_stage_id: RelationalBindingStageId,
        before_stage_id: RelationalBindingStageId,
        before_dimension_id: RelationalDimensionId,
        before_factor_cell_id: SupportCellId,
    },
    SeparatedProjection {
        compiler_certificate_id: [u8; 32],
        factor_binding_root: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CertifiedSourcePopulationBinding {
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    certificate_id: [u8; 32],
    shape: CertifiedSourcePopulationShape,
    source_cell_id: SupportCellId,
    source_materializer_id: SupportMaterializerId,
    injectivity_evidence_id: SupportCellEvidenceId,
    cardinality_evidence_id: SupportCellEvidenceId,
    population_root: CertifiedSourcePopulationRoot,
    exact_cardinality: u128,
}

impl CertifiedSourcePopulationBinding {
    fn from_verified(proof: &RelationalSourceImageExactnessProof) -> Self {
        let artifact = proof.proof().artifact();
        Self {
            plan_root: artifact.plan_root(),
            relation_id: artifact.relation_id(),
            certificate_id: artifact.certificate_id(),
            shape: artifact.certified_population_shape(),
            source_cell_id: artifact.source_row_cell_id(),
            source_materializer_id: artifact.source_materializer_id(),
            injectivity_evidence_id: proof.injectivity().id(),
            cardinality_evidence_id: proof.exact_cardinality().id(),
            population_root: proof.population_root(),
            exact_cardinality: artifact.exact_source_cardinality(),
        }
    }

    pub(crate) const fn plan_root(self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn certificate_id(self) -> [u8; 32] {
        self.certificate_id
    }

    /// Proof strategy and exact factor identity retained by this population
    /// binding. The direct variant remains strong enough for the specialized
    /// `count_distinct(before)` summary; the separated variant deliberately
    /// does not claim that one-factor shape.
    pub(crate) const fn shape(self) -> CertifiedSourcePopulationShape {
        self.shape
    }

    pub(crate) const fn source_cell_id(self) -> SupportCellId {
        self.source_cell_id
    }

    pub(crate) const fn source_materializer_id(self) -> SupportMaterializerId {
        self.source_materializer_id
    }

    pub(crate) const fn injectivity_evidence_id(self) -> SupportCellEvidenceId {
        self.injectivity_evidence_id
    }

    pub(crate) const fn cardinality_evidence_id(self) -> SupportCellEvidenceId {
        self.cardinality_evidence_id
    }

    pub(crate) const fn population_root(self) -> CertifiedSourcePopulationRoot {
        self.population_root
    }

    pub(crate) const fn exact_cardinality(self) -> u128 {
        self.exact_cardinality
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceImageFactorBinding {
    stage_id: RelationalBindingStageId,
    dimension_id: RelationalDimensionId,
    factor_cell_id: SupportCellId,
    exact_cardinality: u128,
}

impl RelationalSourceImageFactorBinding {
    pub(super) const fn restore_from_journal_codec(
        stage_id: RelationalBindingStageId,
        dimension_id: RelationalDimensionId,
        factor_cell_id: SupportCellId,
        exact_cardinality: u128,
    ) -> Self {
        Self {
            stage_id,
            dimension_id,
            factor_cell_id,
            exact_cardinality,
        }
    }

    pub(crate) const fn stage_id(self) -> RelationalBindingStageId {
        self.stage_id
    }

    pub(crate) const fn dimension_id(self) -> RelationalDimensionId {
        self.dimension_id
    }

    pub(crate) const fn factor_cell_id(self) -> SupportCellId {
        self.factor_cell_id
    }

    pub(crate) const fn exact_cardinality(self) -> u128 {
        self.exact_cardinality
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSourceImageExactnessProofShape {
    DirectBeforeFactor {
        context_stage_id: RelationalBindingStageId,
        before_stage_id: RelationalBindingStageId,
        before_dimension_id: RelationalDimensionId,
        before_factor_cell_id: SupportCellId,
    },
    SeparatedProjection {
        compiler_certificate_id: [u8; 32],
        factors: Box<[RelationalSourceImageFactorBinding]>,
        witness_ids: Box<[[u8; 32]]>,
    },
}

/// Canonical replay artifact for assignment-to-source-image exactness.
///
/// All fields are codec-visible because durable replay must compare the exact
/// canonical payload. `restore_from_journal_codec` validates only artifact
/// shape and identity; it never constructs a verified token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceImageExactnessProofArtifact {
    schema_version: u32,
    certificate_id: [u8; 32],
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    binding_stage_ids: Box<[RelationalBindingStageId]>,
    shape: RelationalSourceImageExactnessProofShape,
    source_assignment_cell_id: SupportCellId,
    source_assignment_producer_id: SupportProducerId,
    source_assignment_materializer_id: SupportMaterializerId,
    source_row_cell_id: SupportCellId,
    source_materializer_id: SupportMaterializerId,
    exact_source_cardinality: u128,
}

impl RelationalSourceImageExactnessProofArtifact {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_v1_from_journal_codec(
        schema_version: u32,
        certificate_id: [u8; 32],
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        binding_stage_ids: Box<[RelationalBindingStageId]>,
        context_stage_id: RelationalBindingStageId,
        before_stage_id: RelationalBindingStageId,
        before_dimension_id: RelationalDimensionId,
        before_factor_cell_id: SupportCellId,
        source_assignment_cell_id: SupportCellId,
        source_assignment_producer_id: SupportProducerId,
        source_assignment_materializer_id: SupportMaterializerId,
        source_row_cell_id: SupportCellId,
        source_materializer_id: SupportMaterializerId,
        exact_source_cardinality: u128,
    ) -> Result<Self, RelationalSourceImageExactnessProofError> {
        let artifact = Self {
            schema_version,
            certificate_id,
            plan_root,
            relation_id,
            binding_stage_ids,
            shape: RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
                context_stage_id,
                before_stage_id,
                before_dimension_id,
                before_factor_cell_id,
            },
            source_assignment_cell_id,
            source_assignment_producer_id,
            source_assignment_materializer_id,
            source_row_cell_id,
            source_materializer_id,
            exact_source_cardinality,
        };
        artifact.validate_identity()?;
        Ok(artifact)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_v2_from_journal_codec(
        schema_version: u32,
        certificate_id: [u8; 32],
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        binding_stage_ids: Box<[RelationalBindingStageId]>,
        compiler_certificate_id: [u8; 32],
        factors: Box<[RelationalSourceImageFactorBinding]>,
        witness_ids: Box<[[u8; 32]]>,
        source_assignment_cell_id: SupportCellId,
        source_assignment_producer_id: SupportProducerId,
        source_assignment_materializer_id: SupportMaterializerId,
        source_row_cell_id: SupportCellId,
        source_materializer_id: SupportMaterializerId,
        exact_source_cardinality: u128,
    ) -> Result<Self, RelationalSourceImageExactnessProofError> {
        let artifact = Self {
            schema_version,
            certificate_id,
            plan_root,
            relation_id,
            binding_stage_ids,
            shape: RelationalSourceImageExactnessProofShape::SeparatedProjection {
                compiler_certificate_id,
                factors,
                witness_ids,
            },
            source_assignment_cell_id,
            source_assignment_producer_id,
            source_assignment_materializer_id,
            source_row_cell_id,
            source_materializer_id,
            exact_source_cardinality,
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

    pub(crate) const fn shape(&self) -> &RelationalSourceImageExactnessProofShape {
        &self.shape
    }

    pub(crate) const fn source_assignment_cell_id(&self) -> SupportCellId {
        self.source_assignment_cell_id
    }

    pub(crate) const fn source_assignment_producer_id(&self) -> SupportProducerId {
        self.source_assignment_producer_id
    }

    pub(crate) const fn source_assignment_materializer_id(&self) -> SupportMaterializerId {
        self.source_assignment_materializer_id
    }

    pub(crate) const fn source_row_cell_id(&self) -> SupportCellId {
        self.source_row_cell_id
    }

    pub(crate) const fn source_materializer_id(&self) -> SupportMaterializerId {
        self.source_materializer_id
    }

    pub(crate) const fn exact_source_cardinality(&self) -> u128 {
        self.exact_source_cardinality
    }

    pub(crate) fn compiler_projection_certificate_id(&self) -> Option<[u8; 32]> {
        match &self.shape {
            RelationalSourceImageExactnessProofShape::DirectBeforeFactor { .. } => None,
            RelationalSourceImageExactnessProofShape::SeparatedProjection {
                compiler_certificate_id,
                ..
            } => Some(*compiler_certificate_id),
        }
    }

    fn certified_population_shape(&self) -> CertifiedSourcePopulationShape {
        match &self.shape {
            RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
                context_stage_id,
                before_stage_id,
                before_dimension_id,
                before_factor_cell_id,
            } => CertifiedSourcePopulationShape::DirectBeforeFactor {
                context_stage_id: *context_stage_id,
                before_stage_id: *before_stage_id,
                before_dimension_id: *before_dimension_id,
                before_factor_cell_id: *before_factor_cell_id,
            },
            RelationalSourceImageExactnessProofShape::SeparatedProjection {
                compiler_certificate_id,
                factors,
                ..
            } => CertifiedSourcePopulationShape::SeparatedProjection {
                compiler_certificate_id: *compiler_certificate_id,
                factor_binding_root: derive_factor_binding_root(factors),
            },
        }
    }

    fn validate_identity(&self) -> Result<(), RelationalSourceImageExactnessProofError> {
        if !matches!(
            self.schema_version,
            RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1
                | RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION
        ) {
            return Err(
                RelationalSourceImageExactnessProofError::UnsupportedArtifactVersion {
                    actual: self.schema_version,
                    expected: RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION,
                },
            );
        }
        let binding_stage_ids_are_unique = self
            .binding_stage_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            == self.binding_stage_ids.len();
        let shape_valid = binding_stage_ids_are_unique
            && match &self.shape {
                RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
                    context_stage_id,
                    before_stage_id,
                    ..
                } => {
                    self.schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1
                        && self.binding_stage_ids.len() == 2
                        && self.binding_stage_ids[0] == *context_stage_id
                        && self.binding_stage_ids[1] == *before_stage_id
                        && context_stage_id != before_stage_id
                }
                RelationalSourceImageExactnessProofShape::SeparatedProjection {
                    factors,
                    witness_ids,
                    ..
                } => {
                    self.schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION
                        && separated_projection_shape_is_valid(
                            &self.binding_stage_ids,
                            factors,
                            witness_ids,
                            self.exact_source_cardinality,
                        )
                }
            };
        if !shape_valid || self.exact_source_cardinality == 0 {
            return Err(RelationalSourceImageExactnessProofError::InvalidArtifactShape);
        }
        let derived = derive_source_image_exactness_certificate_id(self);
        if derived != self.certificate_id {
            return Err(RelationalSourceImageExactnessProofError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

fn separated_projection_shape_is_valid(
    binding_stage_ids: &[RelationalBindingStageId],
    factors: &[RelationalSourceImageFactorBinding],
    witness_ids: &[[u8; 32]],
    exact_source_cardinality: u128,
) -> bool {
    if binding_stage_ids.is_empty()
        || factors.is_empty()
        || factors.len() != witness_ids.len()
        || exact_source_cardinality == 0
    {
        return false;
    }
    let mut prior_binding_position = None;
    let mut dimensions = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut witnesses = BTreeSet::new();
    let mut product = 1_u128;
    for (factor, witness_id) in factors.iter().zip(witness_ids.iter()) {
        let Some(binding_position) = binding_stage_ids
            .iter()
            .position(|stage_id| *stage_id == factor.stage_id)
        else {
            return false;
        };
        if factor.exact_cardinality == 0
            || prior_binding_position.is_some_and(|prior| binding_position <= prior)
            || !dimensions.insert(factor.dimension_id)
            || !cells.insert(factor.factor_cell_id)
            || !witnesses.insert(*witness_id)
        {
            return false;
        }
        let Some(next_product) = product.checked_mul(factor.exact_cardinality) else {
            return false;
        };
        product = next_product;
        prior_binding_position = Some(binding_position);
    }
    product == exact_source_cardinality
}

/// Claim-typed input to a future `support_cell` issuance gateway.
///
/// The marker makes it impossible to exchange the cardinality binding for the
/// injectivity binding even though both share the same canonical wire fields.
/// This is not accepted evidence by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceImageEvidenceBinding<C: SupportCellClaim> {
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
    claim: PhantomData<fn() -> C>,
}

impl<C: SupportCellClaim> RelationalSourceImageEvidenceBinding<C> {
    const fn new(
        obligation_id: SupportProofObligationId,
        conclusion_digest: [u8; 32],
        proof_digest: [u8; 32],
    ) -> Self {
        Self {
            obligation_id,
            conclusion_digest,
            proof_digest,
            claim: PhantomData,
        }
    }

    pub(crate) const fn obligation_id(&self) -> SupportProofObligationId {
        self.obligation_id
    }

    pub(crate) const fn conclusion_digest(&self) -> [u8; 32] {
        self.conclusion_digest
    }

    pub(crate) const fn proof_digest(&self) -> [u8; 32] {
        self.proof_digest
    }
}

pub(crate) type RelationalSourceImageInjectivityEvidenceBinding =
    RelationalSourceImageEvidenceBinding<InjectiveMappingClaim>;
pub(crate) type RelationalSourceImageCardinalityEvidenceBinding =
    RelationalSourceImageEvidenceBinding<ExactCardinalityClaim>;

/// Opaque authority returned only after replaying the recognized plan shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalSourceImageExactnessProof {
    artifact: RelationalSourceImageExactnessProofArtifact,
    source_cell: SupportCell,
    injectivity_binding: RelationalSourceImageInjectivityEvidenceBinding,
    cardinality_binding: RelationalSourceImageCardinalityEvidenceBinding,
}

impl VerifiedRelationalSourceImageExactnessProof {
    pub(crate) const fn artifact(&self) -> &RelationalSourceImageExactnessProofArtifact {
        &self.artifact
    }

    /// Retained only so the private `support_cell` gateway can reconstruct the
    /// exact typed obligations proved above. Callers cannot substitute a cell
    /// when evidence is minted.
    pub(super) const fn source_cell(&self) -> &SupportCell {
        &self.source_cell
    }

    pub(crate) const fn injectivity_binding(
        &self,
    ) -> RelationalSourceImageInjectivityEvidenceBinding {
        self.injectivity_binding
    }

    pub(crate) const fn cardinality_binding(
        &self,
    ) -> RelationalSourceImageCardinalityEvidenceBinding {
        self.cardinality_binding
    }
}

/// Complete source-image proof result after the private gateway has minted the
/// two accepted, typed evidence values. The population root commits their
/// exact evidence IDs, so changing either gateway verifier contract changes
/// the certified population identity even when the producer artifact and
/// pre-gateway bindings are unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSourceImageExactnessProof {
    proof: VerifiedRelationalSourceImageExactnessProof,
    injectivity: SupportCellEvidence<InjectiveMappingClaim>,
    exact_cardinality: SupportCellEvidence<ExactCardinalityClaim>,
    population_root: CertifiedSourcePopulationRoot,
}

impl RelationalSourceImageExactnessProof {
    pub(crate) const fn proof(&self) -> &VerifiedRelationalSourceImageExactnessProof {
        &self.proof
    }

    pub(crate) const fn injectivity(&self) -> &SupportCellEvidence<InjectiveMappingClaim> {
        &self.injectivity
    }

    pub(crate) const fn exact_cardinality(&self) -> &SupportCellEvidence<ExactCardinalityClaim> {
        &self.exact_cardinality
    }

    pub(crate) const fn population_root(&self) -> CertifiedSourcePopulationRoot {
        self.population_root
    }

    pub(crate) fn population_binding(&self) -> CertifiedSourcePopulationBinding {
        CertifiedSourcePopulationBinding::from_verified(self)
    }
}

/// Verify the current plan, mint both typed evidence values through the private
/// gateway, and seal their exact identities into one stable population root.
pub(crate) fn prove_relational_source_image_exactness(
    plan: &RelationalSupportPlan,
) -> Result<RelationalSourceImageExactnessProof, RelationalSourceImageExactnessProofError> {
    let (mut artifact, source_cell) = verify_source_image_producer_chain(plan)?;
    artifact.certificate_id = derive_source_image_exactness_certificate_id(&artifact);
    artifact.validate_identity()?;

    let injectivity_obligation = SupportCellObligation::new(
        source_cell,
        InjectiveMappingClaim::new(source_cell.materializer_id()),
    )?;
    let injectivity_conclusion = CertifiedInjective;
    let injectivity_binding = derive_evidence_binding(
        artifact.schema_version,
        artifact.certificate_id,
        0x01,
        &injectivity_obligation,
        &injectivity_conclusion,
    );

    let cardinality_obligation = SupportCellObligation::new(source_cell, ExactCardinalityClaim)?;
    let cardinality_binding = derive_evidence_binding(
        artifact.schema_version,
        artifact.certificate_id,
        0x02,
        &cardinality_obligation,
        &artifact.exact_source_cardinality,
    );
    let proof = VerifiedRelationalSourceImageExactnessProof {
        artifact,
        source_cell: source_cell.clone(),
        injectivity_binding,
        cardinality_binding,
    };
    let injectivity =
        super::support_cell::relational_source_image_exactness_gateway::injectivity(&proof)?;
    let exact_cardinality =
        super::support_cell::relational_source_image_exactness_gateway::cardinality(&proof)?;
    let population_root =
        derive_certified_source_population_root(proof.artifact(), &injectivity, &exact_cardinality);

    Ok(RelationalSourceImageExactnessProof {
        proof,
        injectivity,
        exact_cardinality,
        population_root,
    })
}

/// Restore proof authority only by recomputing the complete recognized plan
/// proof and comparing the exact canonical artifact.
pub(crate) fn reverify_relational_source_image_exactness_artifact(
    artifact: &RelationalSourceImageExactnessProofArtifact,
    plan: &RelationalSupportPlan,
) -> Result<RelationalSourceImageExactnessProof, RelationalSourceImageExactnessProofError> {
    artifact.validate_identity()?;
    let verified = prove_relational_source_image_exactness(plan)?;
    if verified.proof().artifact() != artifact {
        return Err(RelationalSourceImageExactnessProofError::ArtifactSemanticMismatch);
    }
    Ok(verified)
}

fn verify_source_image_producer_chain(
    plan: &RelationalSupportPlan,
) -> Result<
    (RelationalSourceImageExactnessProofArtifact, &SupportCell),
    RelationalSourceImageExactnessProofError,
> {
    if plan.source_image_projection().is_some() {
        verify_source_image_producer_chain_v2(plan)
    } else {
        verify_source_image_producer_chain_v1(plan)
    }
}

fn verify_source_image_producer_chain_v1(
    plan: &RelationalSupportPlan,
) -> Result<
    (RelationalSourceImageExactnessProofArtifact, &SupportCell),
    RelationalSourceImageExactnessProofError,
> {
    if !plan.validate_root() {
        return Err(RelationalSourceImageExactnessProofError::InvalidPlanRoot);
    }
    for cell in plan.all_cells() {
        cell.validate()?;
    }

    // The first bounded slice is intentionally narrower than the general
    // structural source-image observation in the support planner. Requiring
    // these two independent stages proves one global Context value rather than
    // merely one Context value per dependent prefix.
    let [RelationalBindingStage::Singleton(context), RelationalBindingStage::Finite(before)] =
        plan.stages()
    else {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "source proof requires exactly singleton Context then finite Before",
            ),
        );
    };
    if context.binding_index() != 0
        || context.role() != ExploreSourceBindingRoleIr::Context
        || !context.dependency_key().is_empty()
        || !context.input_dimensions().is_empty()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "Context is not one independent singleton stage",
            ),
        );
    }
    if before.recipe().binding_index() != 1
        || before.role() != ExploreSourceBindingRoleIr::Before
        || !before.recipe().dependency_key().is_empty()
        || !before.schema().key_dimensions().is_empty()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "Before is not one independent finite factor",
            ),
        );
    }

    let exact_source_cardinality = before
        .exactness()
        .exact()
        .filter(|count| *count > 0)
        .ok_or(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "Before factor is empty, open, or exceeds exact positive support",
            ),
        )?;
    let before_factor_cell = before.cell().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "Before factor has no positive support cell",
        ),
    )?;
    if before.recipe().known_local_cardinality() != Some(exact_source_cardinality)
        || before_factor_cell.space()
            != SupportCellSpace::ProducerCoordinates(before.recipe().producer_id())
        || before_factor_cell.materializer_id() != before.recipe().materializer_id()
        || before_factor_cell.cardinality().exact() != Some(exact_source_cardinality)
        || before_factor_cell.coordinate_cardinality().exact() != Some(exact_source_cardinality)
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "Before factor recipe, cell, and exact count disagree",
            ),
        );
    }

    let assignment_population = plan.source_assignments();
    if assignment_population.kind() != RelationalSupportPopulationKind::SourceAssignments
        || assignment_population.exactness().exact() != Some(exact_source_cardinality)
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "assignment population is not structurally exact",
            ),
        );
    }
    let assignment_cell = assignment_population.cell().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "assignment population has no positive support cell",
        ),
    )?;
    match assignment_population.recipe() {
        RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells }
            if factor_cells.as_ref() == std::slice::from_ref(&before_factor_cell.id()) => {}
        _ => {
            return Err(
                RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                    "assignments are not the exact one-factor Before product",
                ),
            );
        }
    }

    let assignment_producer_id = derive_assignment_producer_id(
        plan.relation_id(),
        plan.coverage().semantic_dependency_digest(),
        before.stage_id(),
        before.dimension_id(),
        before_factor_cell.id(),
    );
    let assignment_materializer_id = derive_materializer_id(
        ASSIGNMENT_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id,
    );
    if assignment_cell.space() != SupportCellSpace::ProducerCoordinates(assignment_producer_id)
        || assignment_cell.materializer_id() != assignment_materializer_id
        || assignment_cell.expression() != before_factor_cell.expression()
        || assignment_cell.cardinality().exact() != Some(exact_source_cardinality)
        || assignment_cell.coordinate_cardinality().exact() != Some(exact_source_cardinality)
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "assignment cell is not the canonical exact Before product",
            ),
        );
    }

    let source_population = plan.source_rows();
    if source_population.kind() != RelationalSupportPopulationKind::SourceRows {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "planned source population has the wrong semantic kind",
            ),
        );
    }
    match source_population.exactness() {
        RelationalSupportExactness::Open {
            confirmed_lower_bound,
            reason: RelationalSupportOpenReason::MappedImageNeedsEvidence,
        } if confirmed_lower_bound
            == source_population
                .cell()
                .map_or(0, |cell| cell.cardinality().lower_bound()) => {}
        _ => {
            return Err(
                RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                    "source image is not awaiting mapped-image evidence",
                ),
            );
        }
    }
    let source_cell = source_population.cell().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "source image has no positive support cell",
        ),
    )?;
    if !matches!(
        source_population.recipe(),
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell: id }
            if *id == assignment_cell.id()
    ) {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "source image does not consume the exact assignment cell",
            ),
        );
    }
    let source_materializer_id = derive_materializer_id(
        SOURCE_IMAGE_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id,
    );
    if source_cell.space()
        != (SupportCellSpace::MappedImage {
            producer_id: assignment_producer_id,
            target: SupportExtensionalTarget::SourceRows(plan.relation_id()),
        })
        || source_cell.materializer_id() != source_materializer_id
        || source_cell.expression() != assignment_cell.expression()
        || source_cell.coordinate_cardinality().exact() != Some(exact_source_cardinality)
        || source_cell.cardinality().exact().is_some()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "source cell is not the canonical open image of exact assignments",
            ),
        );
    }

    // The two-stage checks above are the structural injectivity proof: the
    // singleton Context introduces no varying coordinate, and the only varying
    // coordinate is Before, which SourceRow retains verbatim.
    let artifact = RelationalSourceImageExactnessProofArtifact {
        schema_version: RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1,
        certificate_id: [0; 32],
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        binding_stage_ids: vec![context.stage_id(), before.stage_id()].into_boxed_slice(),
        shape: RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
            context_stage_id: context.stage_id(),
            before_stage_id: before.stage_id(),
            before_dimension_id: before.dimension_id(),
            before_factor_cell_id: before_factor_cell.id(),
        },
        source_assignment_cell_id: assignment_cell.id(),
        source_assignment_producer_id: assignment_producer_id,
        source_assignment_materializer_id: assignment_materializer_id,
        source_row_cell_id: source_cell.id(),
        source_materializer_id,
        exact_source_cardinality,
    };
    Ok((artifact, source_cell))
}

fn verify_source_image_producer_chain_v2(
    plan: &RelationalSupportPlan,
) -> Result<
    (RelationalSourceImageExactnessProofArtifact, &SupportCell),
    RelationalSourceImageExactnessProofError,
> {
    if !plan.validate_root() {
        return Err(RelationalSourceImageExactnessProofError::InvalidPlanRoot);
    }
    for cell in plan.all_cells() {
        cell.validate()?;
    }
    let certificate = plan.source_image_projection().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "separated source proof has no compiler certificate",
        ),
    )?;
    if !certificate.validate_identity()
        || certificate.relation_id != plan.relation_id()
        || certificate.semantic_dependency_digest != plan.coverage().semantic_dependency_digest()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "compiler source projection certificate does not match the support plan",
            ),
        );
    }

    let finite_stages = plan
        .stages()
        .iter()
        .filter_map(|stage| match stage {
            RelationalBindingStage::Finite(finite) => Some(finite),
            RelationalBindingStage::Singleton(_) => None,
        })
        .collect::<Vec<_>>();
    if finite_stages.len() != certificate.factors.len()
        || certificate.factors.len() != certificate.witnesses.len()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "compiler source projection certificate does not cover every finite stage",
            ),
        );
    }

    let mut factor_bindings = Vec::with_capacity(finite_stages.len());
    let mut factor_cell_ids = Vec::with_capacity(finite_stages.len());
    let mut exact_source_cardinality = 1_u128;
    for ((stage, factor), witness) in finite_stages
        .iter()
        .zip(certificate.factors.iter())
        .zip(certificate.witnesses.iter())
    {
        let factor_cell = stage.cell().ok_or(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "separated source proof has an exact-empty finite factor",
            ),
        )?;
        let kind_matches = matches!(
            (factor.kind, witness.kind, stage.recipe().domain_kind()),
            (
                CheckedExploreSourceProjectionFactorKind::AffineIntRange { .. },
                CheckedExploreSourceProjectionWitnessKind::Affine { coefficient, .. },
                super::relational_support_planner::RelationalFiniteDomainRecipeKind::CheckedIntRange,
            ) if coefficient != 0
        ) || matches!(
            (factor.kind, witness.kind, stage.recipe().domain_kind()),
            (
                CheckedExploreSourceProjectionFactorKind::ExactFinite { plan_digest },
                CheckedExploreSourceProjectionWitnessKind::DirectIdentity {
                    plan_digest: witness_plan_digest,
                },
                super::relational_support_planner::RelationalFiniteDomainRecipeKind::CheckedExact,
            ) if plan_digest == witness_plan_digest
        );
        if stage.recipe().binding_index() != factor.binding_index
            || !stage.recipe().dependency_key().is_empty()
            || !stage.schema().key_dimensions().is_empty()
            || stage.recipe().known_local_cardinality() != Some(factor.exact_cardinality)
            || stage.exactness().exact() != Some(factor.exact_cardinality)
            || witness.factor_binding_index != factor.binding_index
            || !kind_matches
            || factor_cell.space()
                != SupportCellSpace::ProducerCoordinates(stage.recipe().producer_id())
            || factor_cell.materializer_id() != stage.recipe().materializer_id()
            || factor_cell.cardinality().exact() != Some(factor.exact_cardinality)
            || factor_cell.coordinate_cardinality().exact() != Some(factor.exact_cardinality)
        {
            return Err(
                RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                    "separated source factor, witness, stage, and cell disagree",
                ),
            );
        }
        exact_source_cardinality = exact_source_cardinality
            .checked_mul(factor.exact_cardinality)
            .ok_or(
                RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                    "separated source factor product exceeds exact support",
                ),
            )?;
        factor_cell_ids.push(factor_cell.id());
        factor_bindings.push(RelationalSourceImageFactorBinding {
            stage_id: stage.stage_id(),
            dimension_id: stage.dimension_id(),
            factor_cell_id: factor_cell.id(),
            exact_cardinality: factor.exact_cardinality,
        });
    }

    let assignment_population = plan.source_assignments();
    if assignment_population.kind() != RelationalSupportPopulationKind::SourceAssignments
        || assignment_population.exactness().exact() != Some(exact_source_cardinality)
        || !matches!(
            assignment_population.recipe(),
            RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells }
                if factor_cells.as_ref() == factor_cell_ids.as_slice()
        )
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "separated source assignments are not the exact ordered factor product",
            ),
        );
    }
    let assignment_cell = assignment_population.cell().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "separated source assignment population has no cell",
        ),
    )?;
    let assignment_producer_id = derive_assignment_product_producer_id(
        plan.relation_id(),
        plan.coverage().semantic_dependency_digest(),
        &factor_bindings,
    );
    let assignment_materializer_id = derive_materializer_id(
        ASSIGNMENT_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id,
    );
    let expected_assignment_expression = SupportExpr::product(
        finite_stages
            .iter()
            .map(|stage| {
                stage
                    .cell()
                    .expect("positive separated factor checked")
                    .expression()
                    .clone()
            })
            .collect(),
    )?;
    if assignment_cell.space() != SupportCellSpace::ProducerCoordinates(assignment_producer_id)
        || assignment_cell.materializer_id() != assignment_materializer_id
        || assignment_cell.expression() != &expected_assignment_expression
        || assignment_cell.cardinality().exact() != Some(exact_source_cardinality)
        || assignment_cell.coordinate_cardinality().exact() != Some(exact_source_cardinality)
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "separated source assignment cell is not the canonical exact product",
            ),
        );
    }

    let source_population = plan.source_rows();
    if source_population.kind() != RelationalSupportPopulationKind::SourceRows
        || !matches!(
            source_population.exactness(),
            RelationalSupportExactness::Open {
                reason: RelationalSupportOpenReason::MappedImageNeedsEvidence,
                ..
            }
        )
        || !matches!(
            source_population.recipe(),
            RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell: id }
                if *id == assignment_cell.id()
        )
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "separated source image is not the open image of exact assignments",
            ),
        );
    }
    let source_cell = source_population.cell().ok_or(
        RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
            "separated source image has no cell",
        ),
    )?;
    let source_materializer_id = derive_materializer_id(
        SOURCE_IMAGE_MATERIALIZER_V1,
        plan.relation_id(),
        assignment_producer_id,
    );
    if source_cell.space()
        != (SupportCellSpace::MappedImage {
            producer_id: assignment_producer_id,
            target: SupportExtensionalTarget::SourceRows(plan.relation_id()),
        })
        || source_cell.materializer_id() != source_materializer_id
        || source_cell.expression() != assignment_cell.expression()
        || source_cell.coordinate_cardinality().exact() != Some(exact_source_cardinality)
        || source_cell.cardinality().exact().is_some()
    {
        return Err(
            RelationalSourceImageExactnessProofError::UnsupportedPlanShape(
                "separated source cell does not preserve the exact assignment coordinates",
            ),
        );
    }

    let artifact = RelationalSourceImageExactnessProofArtifact {
        schema_version: RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION,
        certificate_id: [0; 32],
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        binding_stage_ids: plan
            .stages()
            .iter()
            .map(RelationalBindingStage::stage_id)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        shape: RelationalSourceImageExactnessProofShape::SeparatedProjection {
            compiler_certificate_id: certificate.certificate_id,
            factors: factor_bindings.into_boxed_slice(),
            witness_ids: certificate
                .witnesses
                .iter()
                .map(|witness| witness.witness_id)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        source_assignment_cell_id: assignment_cell.id(),
        source_assignment_producer_id: assignment_producer_id,
        source_assignment_materializer_id: assignment_materializer_id,
        source_row_cell_id: source_cell.id(),
        source_materializer_id,
        exact_source_cardinality,
    };
    Ok((artifact, source_cell))
}

fn derive_evidence_binding<C: SupportCellClaim>(
    schema_version: u32,
    certificate_id: [u8; 32],
    claim_tag: u8,
    obligation: &SupportCellObligation<C>,
    conclusion: &C::Conclusion,
) -> RelationalSourceImageEvidenceBinding<C> {
    let conclusion_digest = obligation.claim().conclusion_digest(conclusion);
    let mut hasher = CanonicalHasher::new(
        if schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1 {
            SOURCE_IMAGE_EXACTNESS_PROOF_V1
        } else {
            SOURCE_IMAGE_EXACTNESS_PROOF_V2
        },
    );
    hasher.digest(certificate_id);
    hasher.tag(claim_tag);
    hasher.digest(obligation.id().bytes());
    hasher.digest(conclusion_digest);
    RelationalSourceImageEvidenceBinding::new(obligation.id(), conclusion_digest, hasher.finish())
}

fn derive_source_image_exactness_certificate_id(
    artifact: &RelationalSourceImageExactnessProofArtifact,
) -> [u8; 32] {
    let mut hasher = CanonicalHasher::new(
        if artifact.schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1 {
            SOURCE_IMAGE_EXACTNESS_CERTIFICATE_V1
        } else {
            SOURCE_IMAGE_EXACTNESS_CERTIFICATE_V2
        },
    );
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.u128(artifact.binding_stage_ids.len() as u128);
    for stage_id in &artifact.binding_stage_ids {
        hasher.digest(stage_id.bytes());
    }
    match &artifact.shape {
        RelationalSourceImageExactnessProofShape::DirectBeforeFactor {
            context_stage_id,
            before_stage_id,
            before_dimension_id,
            before_factor_cell_id,
        } => {
            // Preserve the exact v1 certificate preimage.
            hasher.digest(context_stage_id.bytes());
            hasher.digest(before_stage_id.bytes());
            hasher.digest(before_dimension_id.bytes());
            hasher.digest(before_factor_cell_id.bytes());
        }
        RelationalSourceImageExactnessProofShape::SeparatedProjection {
            compiler_certificate_id,
            factors,
            witness_ids,
        } => {
            hasher.tag(0x02);
            hasher.digest(*compiler_certificate_id);
            hasher.u128(factors.len() as u128);
            for factor in factors.iter() {
                hasher.digest(factor.stage_id.bytes());
                hasher.digest(factor.dimension_id.bytes());
                hasher.digest(factor.factor_cell_id.bytes());
                hasher.u128(factor.exact_cardinality);
            }
            hasher.u128(witness_ids.len() as u128);
            for witness_id in witness_ids.iter() {
                hasher.digest(*witness_id);
            }
        }
    }
    hasher.digest(artifact.source_assignment_cell_id.bytes());
    hasher.digest(artifact.source_assignment_producer_id.bytes());
    hasher.digest(artifact.source_assignment_materializer_id.bytes());
    hasher.digest(artifact.source_row_cell_id.bytes());
    hasher.digest(artifact.source_materializer_id.bytes());
    hasher.u128(artifact.exact_source_cardinality);
    hasher.finish()
}

fn derive_certified_source_population_root(
    artifact: &RelationalSourceImageExactnessProofArtifact,
    injectivity: &SupportCellEvidence<InjectiveMappingClaim>,
    cardinality: &SupportCellEvidence<ExactCardinalityClaim>,
) -> CertifiedSourcePopulationRoot {
    let mut hasher = CanonicalHasher::new(
        if artifact.schema_version == RELATIONAL_SOURCE_IMAGE_EXACTNESS_PROOF_VERSION_V1 {
            CERTIFIED_SOURCE_POPULATION_ROOT_V1
        } else {
            CERTIFIED_SOURCE_POPULATION_ROOT_V2
        },
    );
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.certificate_id);
    hasher.digest(artifact.source_row_cell_id.bytes());
    hasher.digest(artifact.source_materializer_id.bytes());
    hasher.u128(artifact.exact_source_cardinality);
    hasher.digest(injectivity.id().bytes());
    hasher.digest(cardinality.id().bytes());
    CertifiedSourcePopulationRoot(hasher.finish())
}

fn derive_factor_binding_root(factors: &[RelationalSourceImageFactorBinding]) -> [u8; 32] {
    let mut hasher =
        CanonicalHasher::new(b"futuruna.explore.relational-source-image-factor-binding-root.v1");
    hasher.u128(factors.len() as u128);
    for factor in factors {
        hasher.digest(factor.stage_id.bytes());
        hasher.digest(factor.dimension_id.bytes());
        hasher.digest(factor.factor_cell_id.bytes());
        hasher.u128(factor.exact_cardinality);
    }
    hasher.finish()
}

// These two derivations intentionally mirror the canonical support planner.
// They are local only because this bounded module must not edit the shared
// planner; wiring should expose/factor the planner helpers rather than retain
// duplicate identity code indefinitely.
fn derive_assignment_producer_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    before_stage_id: RelationalBindingStageId,
    before_dimension_id: RelationalDimensionId,
    before_factor_cell_id: SupportCellId,
) -> SupportProducerId {
    let mut preimage = CanonicalPlannerBytes::new(ASSIGNMENT_PRODUCER_V1);
    preimage.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(source_dependency_digest);
    preimage.u64(1);
    preimage.digest(before_stage_id.bytes());
    preimage.digest(before_dimension_id.bytes());
    preimage.digest(before_factor_cell_id.bytes());
    SupportProducerId::from_canonical_preimage(preimage.as_slice())
}

fn derive_assignment_product_producer_id(
    relation_id: RelationId,
    source_dependency_digest: [u8; 32],
    factors: &[RelationalSourceImageFactorBinding],
) -> SupportProducerId {
    let mut preimage = CanonicalPlannerBytes::new(ASSIGNMENT_PRODUCER_V1);
    preimage.u32(RELATIONAL_SUPPORT_PLANNER_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(source_dependency_digest);
    preimage.u64(factors.len() as u64);
    for factor in factors {
        preimage.digest(factor.stage_id.bytes());
        preimage.digest(factor.dimension_id.bytes());
        preimage.digest(factor.factor_cell_id.bytes());
    }
    SupportProducerId::from_canonical_preimage(preimage.as_slice())
}

fn derive_materializer_id(
    domain: &[u8],
    relation_id: RelationId,
    producer_id: SupportProducerId,
) -> SupportMaterializerId {
    let mut preimage = CanonicalPlannerBytes::new(domain);
    preimage.u32(RELATIONAL_SUPPORT_MATERIALIZER_ABI_VERSION);
    preimage.digest(relation_id.bytes());
    preimage.digest(producer_id.bytes());
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

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
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
pub(crate) enum RelationalSourceImageExactnessProofError {
    UnsupportedArtifactVersion { actual: u32, expected: u32 },
    InvalidArtifactShape,
    ArtifactIdentityMismatch,
    ArtifactSemanticMismatch,
    InvalidPlanRoot,
    UnsupportedPlanShape(&'static str),
    SupportCell(SupportCellError),
}

impl From<SupportCellError> for RelationalSourceImageExactnessProofError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl fmt::Display for RelationalSourceImageExactnessProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion { actual, expected } => write!(
                formatter,
                "source-image exactness artifact version {actual} is unsupported; expected {expected}"
            ),
            Self::InvalidArtifactShape => {
                formatter.write_str("source-image exactness artifact has an invalid shape")
            }
            Self::ArtifactIdentityMismatch => formatter
                .write_str("source-image exactness artifact identity does not match its payload"),
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "source-image exactness artifact does not match the reverified support plan",
            ),
            Self::InvalidPlanRoot => {
                formatter.write_str("source-image exactness proof received an invalid plan root")
            }
            Self::UnsupportedPlanShape(message) => {
                write!(
                    formatter,
                    "source-image exactness proof is unavailable: {message}"
                )
            }
            Self::SupportCell(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl Error for RelationalSourceImageExactnessProofError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}
