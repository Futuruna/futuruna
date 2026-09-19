//! Producer-bound proof of uniform admission over one positive support cell.
//!
//! This verifier recognizes only a complete conjunction of checked Boolean
//! literals retained by [`RelationalSupportPlan`]. It never samples or
//! enumerates cases. Any other predicate shape remains unsupported so the
//! concrete executor stays authoritative until a stronger symbolic producer
//! recipe exists.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionDecision, AdmissionId, RelationId};
use super::relational_support_planner::{
    RelationalObligationActivation, RelationalStagedObligationDescriptor, RelationalSupportPlan,
    RelationalSupportPlanRoot,
};
use super::support_cell::{
    AdmissionClassificationClaim, SupportCellClaim, SupportCellError, SupportCellEvidence,
    SupportCellId, SupportCellObligation, SupportProofObligationId,
};
use super::support_evidence::SupportObligationRecord;
use crate::ExploreAdmissionScope;

pub(crate) const RELATIONAL_UNIFORM_ADMISSION_PROOF_VERSION: u32 = 1;

const UNIFORM_ADMISSION_RECIPE_DIGEST_V1: &[u8] =
    b"futuruna.explore.relational-uniform-admission.recipe-digest.v1";
const UNIFORM_ADMISSION_CERTIFICATE_V1: &[u8] =
    b"futuruna.explore.relational-uniform-admission.certificate.v1";
const UNIFORM_ADMISSION_PROOF_V1: &[u8] = b"futuruna.explore.relational-uniform-admission.proof.v1";

/// Canonical replay artifact for one plan-owned uniform classification. The
/// artifact is data, not authority; replay must call
/// [`reverify_relational_uniform_admission_artifact`] against the installed
/// support plan before a receipt can be issued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalUniformAdmissionProofArtifact {
    schema_version: u32,
    certificate_id: [u8; 32],
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    admission_id: AdmissionId,
    case_cell_id: SupportCellId,
    predicate_count: u32,
    recipe_digest: [u8; 32],
    decision: AdmissionDecision,
}

impl RelationalUniformAdmissionProofArtifact {
    /// Reconstruct a retained artifact and validate only its canonical
    /// structural identity. This deliberately does not create proof authority.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_from_journal_codec(
        schema_version: u32,
        certificate_id: [u8; 32],
        plan_root: RelationalSupportPlanRoot,
        relation_id: RelationId,
        admission_id: AdmissionId,
        case_cell_id: SupportCellId,
        predicate_count: u32,
        recipe_digest: [u8; 32],
        decision: AdmissionDecision,
    ) -> Result<Self, RelationalUniformAdmissionProofError> {
        let artifact = Self {
            schema_version,
            certificate_id,
            plan_root,
            relation_id,
            admission_id,
            case_cell_id,
            predicate_count,
            recipe_digest,
            decision,
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

    pub(crate) const fn admission_id(&self) -> AdmissionId {
        self.admission_id
    }

    pub(crate) const fn case_cell_id(&self) -> SupportCellId {
        self.case_cell_id
    }

    pub(crate) const fn predicate_count(&self) -> u32 {
        self.predicate_count
    }

    pub(crate) const fn recipe_digest(&self) -> [u8; 32] {
        self.recipe_digest
    }

    pub(crate) const fn decision(&self) -> AdmissionDecision {
        self.decision
    }

    fn validate_identity(&self) -> Result<(), RelationalUniformAdmissionProofError> {
        if self.schema_version != RELATIONAL_UNIFORM_ADMISSION_PROOF_VERSION {
            return Err(
                RelationalUniformAdmissionProofError::UnsupportedArtifactVersion(
                    self.schema_version,
                ),
            );
        }
        if derive_certificate_id(self) != self.certificate_id {
            return Err(RelationalUniformAdmissionProofError::ArtifactIdentityMismatch);
        }
        Ok(())
    }
}

/// Opaque typed binding consumed only by the support-cell issuance gateway.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalUniformAdmissionEvidenceBinding {
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
    proof_digest: [u8; 32],
}

impl RelationalUniformAdmissionEvidenceBinding {
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

/// Authority token returned only after the installed plan recipe is replayed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedRelationalUniformAdmissionProof {
    artifact: RelationalUniformAdmissionProofArtifact,
    evidence_binding: RelationalUniformAdmissionEvidenceBinding,
}

impl VerifiedRelationalUniformAdmissionProof {
    pub(crate) const fn artifact(&self) -> &RelationalUniformAdmissionProofArtifact {
        &self.artifact
    }

    pub(crate) const fn evidence_binding(&self) -> RelationalUniformAdmissionEvidenceBinding {
        self.evidence_binding
    }
}

/// Typed evidence issued by one successful complete-recipe verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalUniformAdmissionProof {
    proof: VerifiedRelationalUniformAdmissionProof,
    evidence: SupportCellEvidence<AdmissionClassificationClaim>,
}

impl RelationalUniformAdmissionProof {
    pub(crate) const fn proof(&self) -> &VerifiedRelationalUniformAdmissionProof {
        &self.proof
    }

    pub(crate) const fn evidence(&self) -> &SupportCellEvidence<AdmissionClassificationClaim> {
        &self.evidence
    }
}

/// Prove a single admission decision for every member of a positive case cell.
///
/// The empty conjunction is `Admitted`; otherwise every retained predicate is
/// a checked Boolean literal and conjunction semantics make any `false`
/// predicate uniformly `Rejected`. Unsupported plan recipes fail closed.
pub(crate) fn prove_relational_uniform_admission(
    plan: &RelationalSupportPlan,
) -> Result<RelationalUniformAdmissionProof, RelationalUniformAdmissionProofError> {
    if !plan.validate_root() {
        return Err(RelationalUniformAdmissionProofError::InvalidPlanRoot);
    }
    let predicates = plan.uniform_admission_proof().literal_predicates().ok_or(
        RelationalUniformAdmissionProofError::UnsupportedPlanShape(
            "admission predicate is not a direct checked Boolean literal",
        ),
    )?;
    let predicate_count = u32::try_from(predicates.len()).map_err(|_| {
        RelationalUniformAdmissionProofError::InternalProofInvariant(
            "literal admission predicate count exceeds durable u32 schema",
        )
    })?;
    let case_cell =
        plan.cases()
            .cell()
            .ok_or(RelationalUniformAdmissionProofError::UnsupportedPlanShape(
                "uniform admission evidence requires a positive case support cell",
            ))?;
    if plan.root_cell_id() != Some(case_cell.id()) {
        return Err(RelationalUniformAdmissionProofError::RootCellMismatch);
    }

    let expected_obligation = SupportCellObligation::new(
        case_cell,
        AdmissionClassificationClaim::new(plan.admission_id()),
    )?;
    let mut declared_obligation = None;
    for descriptor in plan.obligations() {
        let RelationalStagedObligationDescriptor::Root {
            activation,
            obligation: SupportObligationRecord::Admission(obligation),
        } = descriptor
        else {
            continue;
        };
        if *activation != RelationalObligationActivation::RootCasePopulation
            || obligation != &expected_obligation
            || declared_obligation.replace(obligation.clone()).is_some()
        {
            return Err(RelationalUniformAdmissionProofError::RootAdmissionObligationMismatch);
        }
    }
    let declared_obligation = declared_obligation
        .ok_or(RelationalUniformAdmissionProofError::RootAdmissionObligationMismatch)?;

    let decision = if predicates.iter().any(|predicate| !predicate.value()) {
        AdmissionDecision::Rejected
    } else {
        AdmissionDecision::Admitted
    };
    let recipe_digest = derive_recipe_digest(predicates);
    let mut artifact = RelationalUniformAdmissionProofArtifact {
        schema_version: RELATIONAL_UNIFORM_ADMISSION_PROOF_VERSION,
        certificate_id: [0; 32],
        plan_root: plan.root(),
        relation_id: plan.relation_id(),
        admission_id: plan.admission_id(),
        case_cell_id: case_cell.id(),
        predicate_count,
        recipe_digest,
        decision,
    };
    artifact.certificate_id = derive_certificate_id(&artifact);
    artifact.validate_identity()?;

    let evidence_binding = derive_evidence_binding(
        artifact.certificate_id,
        declared_obligation.id(),
        declared_obligation.claim().conclusion_digest(&decision),
    );
    let proof = VerifiedRelationalUniformAdmissionProof {
        artifact,
        evidence_binding,
    };
    let evidence = super::support_cell::relational_uniform_admission_proof_gateway::admission(
        &proof,
        declared_obligation,
        decision,
    )?;
    Ok(RelationalUniformAdmissionProof { proof, evidence })
}

/// Re-establish authority for a retained artifact from the installed plan.
pub(crate) fn reverify_relational_uniform_admission_artifact(
    artifact: &RelationalUniformAdmissionProofArtifact,
    plan: &RelationalSupportPlan,
) -> Result<RelationalUniformAdmissionProof, RelationalUniformAdmissionProofError> {
    artifact.validate_identity()?;
    let verified = prove_relational_uniform_admission(plan)?;
    if verified.proof().artifact() != artifact {
        return Err(RelationalUniformAdmissionProofError::ArtifactSemanticMismatch);
    }
    Ok(verified)
}

fn derive_recipe_digest(
    predicates: &[super::relational_support_planner::RelationalLiteralAdmissionPredicate],
) -> [u8; 32] {
    let mut hasher = CanonicalAdmissionHasher::new(UNIFORM_ADMISSION_RECIPE_DIGEST_V1);
    hasher.u32(predicates.len() as u32);
    for predicate in predicates {
        hasher.u32(predicate.admission_index());
        hasher.u8(match predicate.scope() {
            ExploreAdmissionScope::Before => 0x01,
            ExploreAdmissionScope::After => 0x02,
            ExploreAdmissionScope::Transition => 0x03,
        });
        hasher.u8(u8::from(predicate.value()));
    }
    hasher.finish()
}

fn derive_certificate_id(artifact: &RelationalUniformAdmissionProofArtifact) -> [u8; 32] {
    let mut hasher = CanonicalAdmissionHasher::new(UNIFORM_ADMISSION_CERTIFICATE_V1);
    hasher.u32(artifact.schema_version);
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.case_cell_id.bytes());
    hasher.u32(artifact.predicate_count);
    hasher.digest(artifact.recipe_digest);
    hasher.u8(match artifact.decision {
        AdmissionDecision::Rejected => 0x01,
        AdmissionDecision::Admitted => 0x02,
    });
    hasher.finish()
}

fn derive_evidence_binding(
    certificate_id: [u8; 32],
    obligation_id: SupportProofObligationId,
    conclusion_digest: [u8; 32],
) -> RelationalUniformAdmissionEvidenceBinding {
    let mut hasher = CanonicalAdmissionHasher::new(UNIFORM_ADMISSION_PROOF_V1);
    hasher.digest(certificate_id);
    hasher.digest(obligation_id.bytes());
    hasher.digest(conclusion_digest);
    RelationalUniformAdmissionEvidenceBinding {
        obligation_id,
        conclusion_digest,
        proof_digest: hasher.finish(),
    }
}

struct CanonicalAdmissionHasher(Sha256);

impl CanonicalAdmissionHasher {
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

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalUniformAdmissionProofError {
    UnsupportedArtifactVersion(u32),
    ArtifactIdentityMismatch,
    ArtifactSemanticMismatch,
    InvalidPlanRoot,
    UnsupportedPlanShape(&'static str),
    RootCellMismatch,
    RootAdmissionObligationMismatch,
    InternalProofInvariant(&'static str),
    SupportCell(SupportCellError),
}

impl From<SupportCellError> for RelationalUniformAdmissionProofError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl fmt::Display for RelationalUniformAdmissionProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArtifactVersion(version) => {
                write!(
                    formatter,
                    "unsupported uniform-admission proof version {version}"
                )
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("uniform-admission proof artifact identity is not canonical")
            }
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "uniform-admission proof artifact does not match the installed support plan",
            ),
            Self::InvalidPlanRoot => {
                formatter.write_str("uniform-admission proof plan root is not canonical")
            }
            Self::UnsupportedPlanShape(reason) => {
                write!(
                    formatter,
                    "unsupported uniform-admission proof shape: {reason}"
                )
            }
            Self::RootCellMismatch => formatter
                .write_str("uniform-admission proof case cell is not the support-plan root"),
            Self::RootAdmissionObligationMismatch => formatter.write_str(
                "uniform-admission proof has no unique matching root admission obligation",
            ),
            Self::InternalProofInvariant(message) => {
                write!(
                    formatter,
                    "uniform-admission proof invariant failed: {message}"
                )
            }
            Self::SupportCell(error) => {
                write!(
                    formatter,
                    "uniform-admission support evidence is invalid: {error}"
                )
            }
        }
    }
}

impl Error for RelationalUniformAdmissionProofError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}
