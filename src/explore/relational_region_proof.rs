//! Exact one-axis region proofs for the relational Explore IR.
//!
//! This is a proof producer, not a candidate generator.  It consumes the
//! immutable [`CheckedExploreQueryView`], the support plan minted from that
//! view, and the solver-neutral axis inventory.  The first accepted fragment
//! is intentionally narrow:
//!
//! - one independent finite `Int` binding is the semantic `Before` value;
//! - `Context` is the singleton unit value and there are no auxiliary binds;
//! - `to after = ...` is a total singleton quasi-affine expression of Before;
//! - there are no admission predicates; and
//! - FIND is a direct Boolean formula of exact one-axis quasi-affine atoms.
//!
//! When those conditions hold, the producer proves the case-image cardinality,
//! uniform admission, and uniform FIND classification together.  It emits
//! typed support evidence only after the complete normalized proof has been
//! checked.  Everything else is an explicit concrete-fallback residual.  In
//! particular, seeing every proposed boundary is never complement closure.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
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
    AdmissionClassificationClaim, ExactCardinalityClaim, SelectionClassificationClaim,
    SupportCellClaim, SupportCellError, SupportCellId, SupportCellObligation,
    SupportProofObligationId,
};
use super::support_evidence::{SupportEvidenceRecord, SupportObligationRecord};
use super::support_journal::SupportJournalEvent;
use super::{ExploreExactDomain, FindPolarity};
use crate::{CheckedExploreQueryView, Expr, ExprKind, Literal};

pub(crate) const RELATIONAL_REGION_PROOF_VERSION: u32 = 1;

const CERTIFICATE_ID_V1: &[u8] = b"futuruna.explore.relational-region.certificate.v1";
const FORMULA_DIGEST_V1: &[u8] = b"futuruna.explore.relational-region.formula.v1";
const PROOF_DIGEST_V1: &[u8] = b"futuruna.explore.relational-region.proof.v1";

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
/// the artifact from a producer-bound checked query and support plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionProofArtifact {
    schema_version: u32,
    certificate_id: [u8; 32],
    program_hash: Box<str>,
    relation_id: RelationId,
    admission_id: AdmissionId,
    question_id: QuestionId,
    plan_root: RelationalSupportPlanRoot,
    root_cell_id: SupportCellId,
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

    pub(crate) fn program_hash(&self) -> &str {
        &self.program_hash
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
        program_hash: Box<str>,
        relation_id: RelationId,
        admission_id: AdmissionId,
        question_id: QuestionId,
        plan_root: RelationalSupportPlanRoot,
        root_cell_id: SupportCellId,
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
            program_hash,
            relation_id,
            admission_id,
            question_id,
            plan_root,
            root_cell_id,
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
        if self.program_hash.len() != 64
            || !self
                .program_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            || self.value_start >= self.value_end_exclusive
            || self.coordinate_start >= self.coordinate_end_exclusive
            || self.case_cardinality != self.coordinate_end_exclusive - self.coordinate_start
            || u128::try_from(i128::from(self.value_end_exclusive) - i128::from(self.value_start))
                .ok()
                != Some(self.case_cardinality)
        {
            return Err(RelationalRegionProofError::InvalidArtifactShape);
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

    pub(crate) fn program_hash(&self) -> &str {
        &self.artifact.program_hash
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
    CheckedRuleGraphNormalizationRequired,
    StructuredStateProjectionRequired,
    UnboundRelationalValue,
    UnsupportedBooleanOperator,
    UnsupportedIntegerOperator,
    NonlinearIntegerExpression,
    NestedQuantizedExpression,
    NonpositiveDivisor,
    QuantizedNumeratorMayBeNegative,
    RuntimeIntegerOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalRegionExpressionResidual {
    layer: RelationalRegionExpressionLayer,
    ast_path: Box<[u32]>,
    reason: RelationalRegionExpressionResidualReason,
}

impl RelationalRegionExpressionResidual {
    pub(crate) const fn layer(&self) -> RelationalRegionExpressionLayer {
        self.layer
    }

    pub(crate) fn ast_path(&self) -> &[u32] {
        &self.ast_path
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
    BeforeIsNotIndependentIntegerAxis,
    SourceHasAuxiliaryOrNonUnitContext,
    CaseImageCardinalityLiftUnavailable,
    FiniteSuccessorNeedsFiberProof,
    AdmissionFormulaNormalizationRequired { predicates: usize },
    ProofArithmeticCapacityExceeded,
    Expression(RelationalRegionExpressionResidual),
    FindAllSelectsNonemptySupport,
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

/// Attempt constant-size proof closure of one checked one-axis relation.
///
/// Complexity is `O(E + T log T)` time and `O(E + T)` memory, where `E` is the
/// directly retained successor/FIND AST size and `T` is the number of distinct
/// constant-division terms.  The integer-domain cardinality is absent from the
/// complexity: `0..<200_000` and `0..<3_000_000` cost the same when the formula
/// closes as one uniform region.
pub(crate) fn prove_relational_exact_empty_region(
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
) -> Result<RelationalRegionProofOutcome, RelationalRegionProofError> {
    checked
        .closed_query
        .validate()
        .map_err(RelationalRegionProofError::InvalidQuery)?;
    if !support_plan.validate_root()
        || checked.relation_id() != support_plan.relation_id()
        || checked.admission_id() != support_plan.admission_id()
        || checked.question_id() != support_plan.question_id()
    {
        return Err(RelationalRegionProofError::CheckedPlanScopeMismatch);
    }

    let inventory = RelationalProofStrategyInventory::from_checked(checked, support_plan)?;
    let [axis] = inventory.axes() else {
        return Ok(fallback(RelationalRegionProofResidual::IntegerAxisCount {
            found: inventory.axes().len(),
        }));
    };
    if !axis_is_direct_before(checked, axis) {
        return Ok(fallback(
            RelationalRegionProofResidual::BeforeIsNotIndependentIntegerAxis,
        ));
    }
    if !source_has_only_direct_before_and_unit_context(checked, axis) {
        return Ok(fallback(
            RelationalRegionProofResidual::SourceHasAuxiliaryOrNonUnitContext,
        ));
    }
    if !case_image_is_one_to_one_singleton(support_plan, axis) {
        return Ok(fallback(
            RelationalRegionProofResidual::CaseImageCardinalityLiftUnavailable,
        ));
    }

    let ExploreSuccessorKindIr::Singleton { value: successor } =
        &checked.closed_query.successor.kind
    else {
        return Ok(fallback(
            RelationalRegionProofResidual::FiniteSuccessorNeedsFiberProof,
        ));
    };
    let before_name = &checked.closed_query.source.bindings[axis.binding_index() as usize].name;
    let successor = match normalize_integer_expression(
        successor,
        RelationalRegionExpressionLayer::Successor,
        before_name,
        None,
        axis,
        &mut Vec::new(),
    ) {
        Ok(value) => value,
        Err(residual) => {
            return Ok(fallback(RelationalRegionProofResidual::Expression(
                residual,
            )));
        }
    };
    if !checked.closed_query.admissions.is_empty() {
        return Ok(fallback(
            RelationalRegionProofResidual::AdmissionFormulaNormalizationRequired {
                predicates: checked.closed_query.admissions.len(),
            },
        ));
    }

    let selected_formula = match &checked.closed_query.find {
        ExploreFindIr::All { .. } => {
            return Ok(fallback(
                RelationalRegionProofResidual::FindAllSelectsNonemptySupport,
            ));
        }
        ExploreFindIr::Matches { predicate, .. } | ExploreFindIr::Violations { predicate, .. } => {
            let formula = match normalize_boolean_expression(
                predicate,
                RelationalRegionExpressionLayer::Selection,
                before_name,
                &successor,
                axis,
                &mut Vec::new(),
            ) {
                Ok(formula) => formula,
                Err(residual) => {
                    return Ok(fallback(RelationalRegionProofResidual::Expression(
                        residual,
                    )));
                }
            };
            match checked.closed_query.find.polarity() {
                FindPolarity::Matches => formula,
                FindPolarity::Violations => RelationalBooleanFormula::Not(Box::new(formula)),
                FindPolarity::All => unreachable!("predicate-bearing FIND cannot be all"),
            }
        }
    };

    let selected_truth = match selected_formula.truth_domain(axis) {
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

    let (root_cell_id, cardinality_obligation, admission_obligation) =
        root_obligations(support_plan)?;
    let root_cell = support_plan
        .cell_catalog()
        .get(root_cell_id)
        .ok_or(RelationalRegionProofError::RootCellMissing(root_cell_id))?;
    let selection_obligation = SupportCellObligation::new(
        root_cell,
        SelectionClassificationClaim::new(checked.question_id()),
    )?;
    let case_cardinality = axis.cardinality();

    let selected_formula_digest = formula_digest(&selected_formula);
    let mut artifact = RelationalRegionProofArtifact {
        schema_version: RELATIONAL_REGION_PROOF_VERSION,
        certificate_id: [0; 32],
        program_hash: checked.program_hash().into(),
        relation_id: checked.relation_id(),
        admission_id: checked.admission_id(),
        question_id: checked.question_id(),
        plan_root: support_plan.root(),
        root_cell_id,
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
    let events = vec![
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Cardinality(cardinality)),
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Admission(admission)),
        SupportJournalEvent::evidence_accepted(SupportEvidenceRecord::Selection(selection)),
        SupportJournalEvent::leaf_sealed(root_cell_id),
        SupportJournalEvent::ObligationFrontierSealed,
        SupportJournalEvent::CatalogSealed,
    ];
    Ok(RelationalRegionProofOutcome::ExactEmpty(
        RelationalRegionSupportClosure {
            proof,
            events: events.into_boxed_slice(),
        },
    ))
}

/// Replay verifier for a decoded proof artifact.
///
/// The checked query view and support plan are required external inputs.  The
/// verifier reruns normalization and interval closure from those producer-bound
/// artifacts, then requires byte-for-byte semantic equality with the decoded
/// artifact before returning receipt-bearing support events.  A journal codec
/// must never skip this call or restore [`VerifiedRelationalRegionProof`]
/// directly.
pub(crate) fn reverify_relational_region_proof_artifact(
    artifact: &RelationalRegionProofArtifact,
    checked: &CheckedExploreQueryView<'_>,
    support_plan: &RelationalSupportPlan,
) -> Result<RelationalRegionSupportClosure, RelationalRegionProofError> {
    artifact.validate_identity()?;
    match prove_relational_exact_empty_region(checked, support_plan)? {
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

fn axis_is_direct_before(
    checked: &CheckedExploreQueryView<'_>,
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
    matches!(
        (&context.role, &context.dependencies[..], &context.kind),
        (
            ExploreSourceBindingRoleIr::Context,
            [],
            ExploreSourceBindingKindIr::Singleton {
                value: Expr {
                    kind: ExprKind::Unit,
                    ..
                }
            }
        )
    )
}

fn case_image_is_one_to_one_singleton(
    plan: &RelationalSupportPlan,
    axis: &RelationalIntegerAxis,
) -> bool {
    let Some(assignment_cell) = plan.source_assignments().cell() else {
        return false;
    };
    let RelationalSupportPopulationRecipe::IndependentAssignmentProduct { factor_cells } =
        plan.source_assignments().recipe()
    else {
        return false;
    };
    if factor_cells.len() != 1 || factor_cells[0] != axis.cell().id() {
        return false;
    }
    let Some(source_cell) = plan.source_rows().cell() else {
        return false;
    };
    if !matches!(
        plan.source_rows().recipe(),
        RelationalSupportPopulationRecipe::SourceRowImage { assignment_cell: id }
            if *id == assignment_cell.id()
    ) {
        return false;
    }
    let Some(successor_cell) = plan.successor_coordinates().cell() else {
        return false;
    };
    if !matches!(
        plan.successor_coordinates().recipe(),
        RelationalSupportPopulationRecipe::SuccessorFiberSum {
            source_row_cell,
            successor_kind: RelationalSuccessorRecipeKind::Singleton,
        } if *source_row_cell == source_cell.id()
    ) {
        return false;
    }
    let Some(case_cell) = plan.cases().cell() else {
        return false;
    };
    matches!(
        plan.cases().recipe(),
        RelationalSupportPopulationRecipe::CaseImage {
            successor_coordinate_cell,
        } if *successor_coordinate_cell == successor_cell.id()
    ) && plan.root_cell_id() == Some(case_cell.id())
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
    fn from_operator(operator: &str) -> Option<Self> {
        match operator {
            "<" => Some(Self::Less),
            "<=" => Some(Self::LessOrEqual),
            "==" => Some(Self::Equal),
            "!=" => Some(Self::NotEqual),
            ">=" => Some(Self::GreaterOrEqual),
            ">" => Some(Self::Greater),
            _ => None,
        }
    }

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

fn expression_residual(
    layer: RelationalRegionExpressionLayer,
    ast_path: &[u32],
    reason: RelationalRegionExpressionResidualReason,
) -> RelationalRegionExpressionResidual {
    RelationalRegionExpressionResidual {
        layer,
        ast_path: ast_path.to_vec().into_boxed_slice(),
        reason,
    }
}

fn normalize_integer_expression(
    expression: &Expr,
    layer: RelationalRegionExpressionLayer,
    before_name: &str,
    after: Option<&RelationalQuasiAffine>,
    axis: &RelationalIntegerAxis,
    ast_path: &mut Vec<u32>,
) -> Result<RelationalQuasiAffine, RelationalRegionExpressionResidual> {
    let normalized = match &expression.kind {
        ExprKind::Lit(Literal::Int(value)) => RelationalQuasiAffine::constant(i128::from(*value)),
        ExprKind::Var(name) if name == before_name => {
            RelationalQuasiAffine::axis(axis).map_err(|_| {
                expression_residual(
                    layer,
                    ast_path,
                    RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                )
            })?
        }
        ExprKind::Var(name) if name == "after" => after.cloned().ok_or_else(|| {
            expression_residual(
                layer,
                ast_path,
                RelationalRegionExpressionResidualReason::UnboundRelationalValue,
            )
        })?,
        ExprKind::Var(_) => {
            return Err(expression_residual(
                layer,
                ast_path,
                RelationalRegionExpressionResidualReason::UnboundRelationalValue,
            ));
        }
        ExprKind::Field(_, _) | ExprKind::Index(_, _) => {
            return Err(expression_residual(
                layer,
                ast_path,
                RelationalRegionExpressionResidualReason::StructuredStateProjectionRequired,
            ));
        }
        ExprKind::App(_, _) => {
            return Err(expression_residual(
                layer,
                ast_path,
                RelationalRegionExpressionResidualReason::CheckedRuleGraphNormalizationRequired,
            ));
        }
        ExprKind::UnOp(operator, inner) if operator == "+" || operator == "-" => {
            ast_path.push(0);
            let inner =
                normalize_integer_expression(inner, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            let inner = inner?;
            if operator == "+" {
                inner
            } else {
                inner.scale(-1).map_err(|_| {
                    expression_residual(
                        layer,
                        ast_path,
                        RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                    )
                })?
            }
        }
        ExprKind::BinOp(operator, left, right)
            if matches!(operator.as_str(), "+" | "-" | "*" | "/") =>
        {
            ast_path.push(0);
            let left =
                normalize_integer_expression(left, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            ast_path.push(1);
            let right =
                normalize_integer_expression(right, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            let (left, right) = (left?, right?);
            match operator.as_str() {
                "+" => left.add(&right),
                "-" => left.subtract(&right),
                "*" => match (left.is_constant(), right.is_constant()) {
                    (Some(scalar), _) => right.scale(scalar),
                    (_, Some(scalar)) => left.scale(scalar),
                    _ => {
                        return Err(expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::NonlinearIntegerExpression,
                        ))
                    }
                },
                "/" => {
                    let Some(divisor) = right.is_constant() else {
                        return Err(expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::NonpositiveDivisor,
                        ));
                    };
                    let Ok(divisor) = i64::try_from(divisor) else {
                        return Err(expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::NonpositiveDivisor,
                        ));
                    };
                    if divisor <= 0 {
                        return Err(expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::NonpositiveDivisor,
                        ));
                    }
                    if !left.terms.is_empty() {
                        return Err(expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::NestedQuantizedExpression,
                        ));
                    }
                    let (minimum, _) = left.affine.bounds(axis).map_err(|_| {
                        expression_residual(
                            layer,
                            ast_path,
                            RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                        )
                    })?;
                    if minimum < 0 {
                        return Err(expression_residual(
                            layer,
                            ast_path,
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
                _ => unreachable!("guarded integer operator"),
            }
            .map_err(|_| {
                expression_residual(
                    layer,
                    ast_path,
                    RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                )
            })?
        }
        ExprKind::BinOp(_, _, _)
        | ExprKind::UnOp(_, _)
        | ExprKind::Lit(_)
        | ExprKind::Unit
        | ExprKind::Lambda(_, _)
        | ExprKind::If(_, _, _)
        | ExprKind::Match(_, _)
        | ExprKind::Block(_)
        | ExprKind::List(_)
        | ExprKind::Tuple(_)
        | ExprKind::Effect(_, _)
        | ExprKind::Handle { .. }
        | ExprKind::Try(_)
        | ExprKind::Conjunction(_)
        | ExprKind::Disjunction(_)
        | ExprKind::Pipe(_, _) => {
            return Err(expression_residual(
                layer,
                ast_path,
                RelationalRegionExpressionResidualReason::UnsupportedIntegerOperator,
            ));
        }
    };
    normalized
        .require_runtime_int(axis)
        .map_err(|reason| expression_residual(layer, ast_path, reason))?;
    Ok(normalized)
}

fn normalize_boolean_expression(
    expression: &Expr,
    layer: RelationalRegionExpressionLayer,
    before_name: &str,
    after: &RelationalQuasiAffine,
    axis: &RelationalIntegerAxis,
    ast_path: &mut Vec<u32>,
) -> Result<RelationalBooleanFormula, RelationalRegionExpressionResidual> {
    match &expression.kind {
        ExprKind::Lit(Literal::Bool(value)) => Ok(RelationalBooleanFormula::Constant(*value)),
        ExprKind::UnOp(operator, inner) if operator == "!" => {
            ast_path.push(0);
            let inner =
                normalize_boolean_expression(inner, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            Ok(RelationalBooleanFormula::Not(Box::new(inner?)))
        }
        ExprKind::BinOp(operator, left, right) if operator == "&&" || operator == "||" => {
            ast_path.push(0);
            let left =
                normalize_boolean_expression(left, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            ast_path.push(1);
            let right =
                normalize_boolean_expression(right, layer, before_name, after, axis, ast_path);
            ast_path.pop();
            let parts = vec![left?, right?].into_boxed_slice();
            Ok(if operator == "&&" {
                RelationalBooleanFormula::All(parts)
            } else {
                RelationalBooleanFormula::Any(parts)
            })
        }
        ExprKind::BinOp(operator, left, right) => {
            let Some(relation) = RelationalRelation::from_operator(operator) else {
                return Err(expression_residual(
                    layer,
                    ast_path,
                    RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator,
                ));
            };
            ast_path.push(0);
            let left =
                normalize_integer_expression(left, layer, before_name, Some(after), axis, ast_path);
            ast_path.pop();
            ast_path.push(1);
            let right = normalize_integer_expression(
                right,
                layer,
                before_name,
                Some(after),
                axis,
                ast_path,
            );
            ast_path.pop();
            let difference = left?.subtract(&right?).map_err(|_| {
                expression_residual(
                    layer,
                    ast_path,
                    RelationalRegionExpressionResidualReason::RuntimeIntegerOverflow,
                )
            })?;
            // This difference is a mathematical comparison form.  Unlike an
            // authored subtraction it need not fit i64; both operands were
            // already proved safe above.
            Ok(RelationalBooleanFormula::Comparison {
                difference,
                relation,
            })
        }
        ExprKind::App(_, _) => Err(expression_residual(
            layer,
            ast_path,
            RelationalRegionExpressionResidualReason::CheckedRuleGraphNormalizationRequired,
        )),
        ExprKind::Field(_, _) | ExprKind::Index(_, _) => Err(expression_residual(
            layer,
            ast_path,
            RelationalRegionExpressionResidualReason::StructuredStateProjectionRequired,
        )),
        _ => Err(expression_residual(
            layer,
            ast_path,
            RelationalRegionExpressionResidualReason::UnsupportedBooleanOperator,
        )),
    }
}

fn formula_digest(formula: &RelationalBooleanFormula) -> [u8; 32] {
    let mut hasher = CanonicalProofHasher::new(FORMULA_DIGEST_V1);
    hasher.u32(RELATIONAL_REGION_PROOF_VERSION);
    hasher.formula(formula);
    hasher.finish()
}

fn derive_certificate_id(artifact: &RelationalRegionProofArtifact) -> [u8; 32] {
    let mut hasher = CanonicalProofHasher::new(CERTIFICATE_ID_V1);
    hasher.u32(artifact.schema_version);
    hasher.bytes(artifact.program_hash.as_bytes());
    hasher.digest(artifact.relation_id.bytes());
    hasher.digest(artifact.admission_id.bytes());
    hasher.digest(artifact.question_id.bytes());
    hasher.digest(artifact.plan_root.bytes());
    hasher.digest(artifact.root_cell_id.bytes());
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
    let mut hasher = CanonicalProofHasher::new(PROOF_DIGEST_V1);
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
    CheckedPlanScopeMismatch,
    UnsupportedArtifactVersion(u32),
    InvalidArtifactShape,
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
            Self::CheckedPlanScopeMismatch => formatter.write_str(
                "checked query and relational support plan have different semantic scope",
            ),
            Self::UnsupportedArtifactVersion(version) => write!(
                formatter,
                "relational region proof artifact version {version} is unsupported"
            ),
            Self::InvalidArtifactShape => {
                formatter.write_str("relational region proof artifact has an invalid shape")
            }
            Self::ArtifactIdentityMismatch => formatter
                .write_str("relational region proof artifact identity does not match its payload"),
            Self::ArtifactSemanticMismatch => formatter.write_str(
                "relational region proof artifact does not match the producer-bound checked query",
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
