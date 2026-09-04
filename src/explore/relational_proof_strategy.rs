//! Solver-neutral proof strategy for relational Explore support.
//!
//! This module is deliberately a planner, not a proof authority. It derives
//! exact half-open split coordinates from the narrow part of a checked query
//! that is already an independent integer range and from affine Boolean atoms
//! whose runtime arithmetic is proved overflow-free over that range. Those
//! coordinates are useful scheduling facts; they do not classify either side
//! of a split and candidate exhaustion never closes the complement.
//!
//! Midpoint/binary refinement is proposed only beside an explicit
//! monotonicity or piecewise-uniformity proof obligation. The obligation is a
//! proposition still to prove, not a certificate. Until accepted typed
//! support-cell evidence is present, every proposed interval retains exact
//! materialization as its fallback.
//!
//! Physical solver choice, worker layout, probe order, and resource policy do
//! not occur in this module and therefore cannot enter semantic identities.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::relation::{AdmissionDecision, AdmissionId, QuestionId, RelationId, SelectionDecision};
use super::relational_ir::{ExploreFiniteDomainIr, ExploreSourceBindingKindIr};
use super::relational_support_planner::{
    statically_evaluate_checked_int_range, RelationalBindingStage, RelationalBindingStageId,
    RelationalDimensionId, RelationalRootObligationPlan, RelationalSupportPlan,
    RelationalSupportPlanRoot, RelationalSupportPlannerError,
};
use super::support_cell::{
    SupportCell, SupportCellError, SupportCellEvidenceId, SupportCellId, SupportExpr,
    SupportExprKind, SupportPartitionCertificate, SupportProofObligationId,
};
use super::support_evidence::{
    SupportEvidenceRecord, SupportEvidenceRoot, SupportEvidenceSnapshot, SupportObligationRecord,
};
use super::ExploreExactDomain;
use crate::{CheckedExploreQueryView, Expr, ExprKind, Literal};

pub(crate) const RELATIONAL_PROOF_STRATEGY_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIntegerAxisSupportKind {
    /// The support plan itself contains the exact ordinal interval, so a
    /// structural interval-cover certificate can be issued immediately.
    StructuralOrdinalInterval,
}

/// One independent, statically exact integer-range coordinate in a checked
/// relational support plan.
///
/// `value_*` are Futuruna `Int` values. `coordinate_*` are half-open producer
/// ordinals in the associated support cell. Keeping the two explicit avoids
/// treating a salary value such as 199_999 as though it were a case ordinal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalIntegerAxis {
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    stage_id: RelationalBindingStageId,
    dimension_id: RelationalDimensionId,
    binding_index: u32,
    cell: SupportCell,
    support_kind: RelationalIntegerAxisSupportKind,
    value_start: i64,
    value_end_exclusive: i64,
    coordinate_start: u128,
    coordinate_end_exclusive: u128,
}

impl RelationalIntegerAxis {
    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn stage_id(&self) -> RelationalBindingStageId {
        self.stage_id
    }

    pub(crate) const fn dimension_id(&self) -> RelationalDimensionId {
        self.dimension_id
    }

    pub(crate) const fn binding_index(&self) -> u32 {
        self.binding_index
    }

    pub(crate) const fn cell(&self) -> &SupportCell {
        &self.cell
    }

    pub(crate) const fn support_kind(&self) -> RelationalIntegerAxisSupportKind {
        self.support_kind
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

    pub(crate) const fn cardinality(&self) -> u128 {
        self.coordinate_end_exclusive - self.coordinate_start
    }

    /// Restrict this exact one-dimensional source factor to a half-open slice
    /// of its existing producer coordinates. The source-factor cell remains
    /// unchanged: a downstream case-image child is not the same support object
    /// as the starter axis, even when an injective singleton-successor chain
    /// gives both populations the same coordinate interval.
    pub(crate) fn restrict_to_coordinates(
        &self,
        coordinate_start: u128,
        coordinate_end_exclusive: u128,
    ) -> Result<Self, RelationalProofStrategyError> {
        if coordinate_start < self.coordinate_start
            || coordinate_end_exclusive > self.coordinate_end_exclusive
            || coordinate_start >= coordinate_end_exclusive
        {
            return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                dimension_id: self.dimension_id,
            });
        }
        let start_offset = coordinate_start.checked_sub(self.coordinate_start).ok_or(
            RelationalProofStrategyError::ArithmeticOverflow(
                "restricting an integer axis coordinate",
            ),
        )?;
        let end_offset = coordinate_end_exclusive
            .checked_sub(self.coordinate_start)
            .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
                "restricting an integer axis coordinate",
            ))?;
        let value_start = i128::from(self.value_start)
            .checked_add(i128::try_from(start_offset).map_err(|_| {
                RelationalProofStrategyError::ArithmeticOverflow(
                    "translating a restricted integer-axis start",
                )
            })?)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
                "translating a restricted integer-axis start",
            ))?;
        let value_end_exclusive = i128::from(self.value_start)
            .checked_add(i128::try_from(end_offset).map_err(|_| {
                RelationalProofStrategyError::ArithmeticOverflow(
                    "translating a restricted integer-axis end",
                )
            })?)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
                "translating a restricted integer-axis end",
            ))?;
        Ok(Self {
            plan_root: self.plan_root,
            relation_id: self.relation_id,
            stage_id: self.stage_id,
            dimension_id: self.dimension_id,
            binding_index: self.binding_index,
            cell: self.cell.clone(),
            support_kind: self.support_kind,
            value_start,
            value_end_exclusive,
            coordinate_start,
            coordinate_end_exclusive,
        })
    }

    fn interior_coordinate_for_value_boundary(
        &self,
        value_boundary: i128,
    ) -> Result<Option<u128>, RelationalProofStrategyError> {
        let value_start = i128::from(self.value_start);
        let value_end = i128::from(self.value_end_exclusive);
        if value_boundary <= value_start || value_boundary >= value_end {
            return Ok(None);
        }
        let offset = u128::try_from(value_boundary.checked_sub(value_start).ok_or(
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating an integer split into a coordinate",
            ),
        )?)
        .map_err(|_| {
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating an integer split into a coordinate",
            )
        })?;
        let coordinate = self.coordinate_start.checked_add(offset).ok_or(
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating an integer split into a coordinate",
            ),
        )?;
        if coordinate <= self.coordinate_start || coordinate >= self.coordinate_end_exclusive {
            return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                dimension_id: self.dimension_id,
            });
        }
        Ok(Some(coordinate))
    }

    fn value_boundary_for_coordinate(
        &self,
        coordinate: u128,
    ) -> Result<i128, RelationalProofStrategyError> {
        if coordinate < self.coordinate_start || coordinate > self.coordinate_end_exclusive {
            return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                dimension_id: self.dimension_id,
            });
        }
        let offset = coordinate.checked_sub(self.coordinate_start).ok_or(
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating a coordinate into an integer split",
            ),
        )?;
        let offset = i128::try_from(offset).map_err(|_| {
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating a coordinate into an integer split",
            )
        })?;
        i128::from(self.value_start).checked_add(offset).ok_or(
            RelationalProofStrategyError::ArithmeticOverflow(
                "translating a coordinate into an integer split",
            ),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalGuardRelation {
    Less,
    LessOrEqual,
    Equal,
    NotEqual,
    GreaterOrEqual,
    Greater,
}

impl RelationalGuardRelation {
    /// Integer image cuts at which the truth value can change.
    ///
    /// `difference < 0` and `difference >= 0` split at zero. Integer
    /// `difference <= 0` is the same region as `difference < 1`; equality and
    /// inequality need both cuts to isolate the zero cell.
    const fn image_cuts(self) -> &'static [i128] {
        match self {
            Self::Less | Self::GreaterOrEqual => &[0],
            Self::LessOrEqual | Self::Greater => &[1],
            Self::Equal | Self::NotEqual => &[0, 1],
        }
    }
}

/// Stable semantic location of one checked guard atom. Source spans and names
/// are annotations elsewhere and intentionally absent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalGuardOrigin {
    Admission {
        admission_id: AdmissionId,
        admission_index: u32,
        ast_path: Box<[u32]>,
    },
    Selection {
        question_id: QuestionId,
        ast_path: Box<[u32]>,
    },
    /// A future checked rule-graph adapter can contribute an exact semantic
    /// site digest without exposing a backend or source path.
    CheckedRule {
        semantic_site_digest: [u8; 32],
        ast_path: Box<[u32]>,
    },
}

/// One checked affine atom `coefficient * axis + intercept RELATION 0`.
///
/// This record schedules possible changes only. Constructing it cannot prove
/// a classification or close any support.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCheckedGuardAtom {
    plan_root: RelationalSupportPlanRoot,
    dimension_id: RelationalDimensionId,
    coefficient: i128,
    intercept: i128,
    relation: RelationalGuardRelation,
    origin: RelationalGuardOrigin,
}

impl RelationalCheckedGuardAtom {
    /// Adapter seam for a future checked rule-normalization pass. Supplying a
    /// bogus atom can only worsen scheduling: the resulting candidates still
    /// carry no proof authority and exact fallback remains mandatory.
    pub(crate) fn from_checked_rule_normal_form(
        axis: &RelationalIntegerAxis,
        coefficient: i128,
        intercept: i128,
        relation: RelationalGuardRelation,
        semantic_site_digest: [u8; 32],
        ast_path: impl Into<Box<[u32]>>,
    ) -> Result<Self, RelationalProofStrategyError> {
        if coefficient == 0 {
            return Err(RelationalProofStrategyError::ConstantGuardAtom);
        }
        affine_bounds_over_axis(axis, coefficient, intercept)?;
        Ok(Self {
            plan_root: axis.plan_root,
            dimension_id: axis.dimension_id,
            coefficient,
            intercept,
            relation,
            origin: RelationalGuardOrigin::CheckedRule {
                semantic_site_digest,
                ast_path: ast_path.into(),
            },
        })
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn dimension_id(&self) -> RelationalDimensionId {
        self.dimension_id
    }

    pub(crate) const fn coefficient(&self) -> i128 {
        self.coefficient
    }

    pub(crate) const fn intercept(&self) -> i128 {
        self.intercept
    }

    pub(crate) const fn relation(&self) -> RelationalGuardRelation {
        self.relation
    }

    pub(crate) const fn origin(&self) -> &RelationalGuardOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalStrategyResidualReason {
    DependentIntegerRangeNeedsFiberSpecialization,
    RuntimeIntegerRangeNeedsFiberSpecialization,
    GuardNeedsCheckedRuleNormalization,
    GuardDependsOnMultipleAxes,
    GuardArithmeticMayOverflow,
    IntervalCertificateNotAccepted,
    ExactRootAxisLiftUnavailable,
}

/// Exact fallback work retained by the strategy. A residual is not an
/// exploration failure; it is support that still needs canonical evaluation
/// or a separately accepted proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResidualMaterialization {
    cell_id: Option<SupportCellId>,
    dimension_id: Option<RelationalDimensionId>,
    coordinate_interval: Option<(u128, u128)>,
    reason: RelationalStrategyResidualReason,
}

impl RelationalResidualMaterialization {
    pub(crate) const fn cell_id(&self) -> Option<SupportCellId> {
        self.cell_id
    }

    pub(crate) const fn dimension_id(&self) -> Option<RelationalDimensionId> {
        self.dimension_id
    }

    pub(crate) const fn coordinate_interval(&self) -> Option<(u128, u128)> {
        self.coordinate_interval
    }

    pub(crate) const fn reason(&self) -> &RelationalStrategyResidualReason {
        &self.reason
    }
}

/// Checked, solver-neutral inventory from which a scheduler can request one
/// axis plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalProofStrategyInventory {
    plan_root: RelationalSupportPlanRoot,
    relation_id: RelationId,
    axes: Box<[RelationalIntegerAxis]>,
    guard_atoms: Box<[RelationalCheckedGuardAtom]>,
    residuals: Box<[RelationalResidualMaterialization]>,
}

impl RelationalProofStrategyInventory {
    pub(crate) fn from_checked(
        checked: &CheckedExploreQueryView<'_>,
        support_plan: &RelationalSupportPlan,
    ) -> Result<Self, RelationalProofStrategyError> {
        checked
            .closed_query
            .validate()
            .map_err(RelationalProofStrategyError::InvalidQuery)?;
        if !support_plan.validate_root() {
            return Err(RelationalProofStrategyError::SupportPlanRootMismatch);
        }
        let question_id = require_single_question(checked.question_ids())?;
        let plan_question_id = require_single_question(support_plan.question_ids())?;
        if checked.relation_id() != support_plan.relation_id()
            || checked.admission_id() != support_plan.admission_id()
            || question_id != plan_question_id
        {
            return Err(RelationalProofStrategyError::CheckedPlanScopeMismatch);
        }

        let mut axes = Vec::new();
        let mut residuals = Vec::new();

        for stage in support_plan.stages() {
            let RelationalBindingStage::Finite(stage) = stage else {
                continue;
            };
            let binding_index = usize::try_from(stage.recipe().binding_index()).map_err(|_| {
                RelationalProofStrategyError::IndexConversion("source binding index")
            })?;
            let binding = checked
                .closed_query
                .source
                .bindings
                .get(binding_index)
                .ok_or(RelationalProofStrategyError::BindingStageMismatch {
                    binding_index: stage.recipe().binding_index(),
                })?;
            if binding.binding_index != binding_index || binding.role != stage.role() {
                return Err(RelationalProofStrategyError::BindingStageMismatch {
                    binding_index: stage.recipe().binding_index(),
                });
            }
            let ExploreSourceBindingKindIr::Finite { domain } = &binding.kind else {
                return Err(RelationalProofStrategyError::BindingStageMismatch {
                    binding_index: stage.recipe().binding_index(),
                });
            };

            match domain {
                ExploreFiniteDomainIr::Exact(ExploreExactDomain::IntRange {
                    start,
                    end_exclusive,
                    cardinality,
                }) => {
                    if *cardinality == 0 {
                        continue;
                    }
                    let Some(cell) = stage.cell() else {
                        residuals.push(RelationalResidualMaterialization {
                            cell_id: None,
                            dimension_id: Some(stage.dimension_id()),
                            coordinate_interval: None,
                            reason: RelationalStrategyResidualReason::DependentIntegerRangeNeedsFiberSpecialization,
                        });
                        continue;
                    };
                    if !stage.schema().key_dimensions().is_empty() {
                        residuals.push(RelationalResidualMaterialization {
                            cell_id: Some(cell.id()),
                            dimension_id: Some(stage.dimension_id()),
                            coordinate_interval: None,
                            reason: RelationalStrategyResidualReason::DependentIntegerRangeNeedsFiberSpecialization,
                        });
                        continue;
                    }
                    let SupportExprKind::OrdinalInterval {
                        start: coordinate_start,
                        end_exclusive: coordinate_end_exclusive,
                    } = cell.expression().kind()
                    else {
                        return Err(RelationalProofStrategyError::AxisCellShapeMismatch {
                            dimension_id: stage.dimension_id(),
                        });
                    };
                    if *coordinate_end_exclusive - *coordinate_start != u128::from(*cardinality)
                        || i128::from(*end_exclusive) - i128::from(*start)
                            != i128::from(*cardinality)
                    {
                        return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                            dimension_id: stage.dimension_id(),
                        });
                    }
                    axes.push(RelationalIntegerAxis {
                        plan_root: support_plan.root(),
                        relation_id: support_plan.relation_id(),
                        stage_id: stage.stage_id(),
                        dimension_id: stage.dimension_id(),
                        binding_index: stage.recipe().binding_index(),
                        cell: cell.clone(),
                        support_kind: RelationalIntegerAxisSupportKind::StructuralOrdinalInterval,
                        value_start: *start,
                        value_end_exclusive: *end_exclusive,
                        coordinate_start: *coordinate_start,
                        coordinate_end_exclusive: *coordinate_end_exclusive,
                    });
                }
                ExploreFiniteDomainIr::IntRange { .. } => {
                    let Some(range) = statically_evaluate_checked_int_range(domain)? else {
                        residuals.push(RelationalResidualMaterialization {
                            cell_id: stage.cell().map(SupportCell::id),
                            dimension_id: Some(stage.dimension_id()),
                            coordinate_interval: None,
                            reason: RelationalStrategyResidualReason::RuntimeIntegerRangeNeedsFiberSpecialization,
                        });
                        continue;
                    };
                    if range.cardinality() == 0 {
                        continue;
                    }
                    if !stage.schema().key_dimensions().is_empty() {
                        residuals.push(RelationalResidualMaterialization {
                            cell_id: stage.cell().map(SupportCell::id),
                            dimension_id: Some(stage.dimension_id()),
                            coordinate_interval: None,
                            reason: RelationalStrategyResidualReason::DependentIntegerRangeNeedsFiberSpecialization,
                        });
                        continue;
                    }
                    let Some(cell) = stage.cell() else {
                        return Err(RelationalProofStrategyError::AxisCellShapeMismatch {
                            dimension_id: stage.dimension_id(),
                        });
                    };
                    let SupportExprKind::OrdinalInterval {
                        start: coordinate_start,
                        end_exclusive: coordinate_end_exclusive,
                    } = cell.expression().kind()
                    else {
                        return Err(RelationalProofStrategyError::AxisCellShapeMismatch {
                            dimension_id: stage.dimension_id(),
                        });
                    };
                    if *coordinate_start != 0
                        || coordinate_end_exclusive.checked_sub(*coordinate_start)
                            != Some(range.cardinality())
                        || stage.exactness().exact() != Some(range.cardinality())
                    {
                        return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                            dimension_id: stage.dimension_id(),
                        });
                    }
                    axes.push(RelationalIntegerAxis {
                        plan_root: support_plan.root(),
                        relation_id: support_plan.relation_id(),
                        stage_id: stage.stage_id(),
                        dimension_id: stage.dimension_id(),
                        binding_index: stage.recipe().binding_index(),
                        cell: cell.clone(),
                        support_kind: RelationalIntegerAxisSupportKind::StructuralOrdinalInterval,
                        value_start: range.start(),
                        value_end_exclusive: range.end_exclusive(),
                        coordinate_start: *coordinate_start,
                        coordinate_end_exclusive: *coordinate_end_exclusive,
                    });
                }
                ExploreFiniteDomainIr::Exact(
                    ExploreExactDomain::Enumerated { .. } | ExploreExactDomain::FiniteType { .. },
                )
                | ExploreFiniteDomainIr::Collection { .. } => {}
            }
        }

        axes.sort_by_key(RelationalIntegerAxis::dimension_id);
        if let Some(case_root_cell_id) = support_plan.root_cell_id() {
            for axis in &axes {
                if case_root_cell_id != axis.cell.id() {
                    residuals.push(RelationalResidualMaterialization {
                        cell_id: Some(case_root_cell_id),
                        dimension_id: Some(axis.dimension_id),
                        coordinate_interval: None,
                        reason: RelationalStrategyResidualReason::ExactRootAxisLiftUnavailable,
                    });
                }
            }
        }
        // Names only address checked source binders while inspecting the
        // retained AST. They never enter an axis identity or split coordinate.
        let axis_by_name = axes
            .iter()
            .filter_map(|axis| {
                checked
                    .closed_query
                    .source
                    .bindings
                    .get(axis.binding_index as usize)
                    .map(|binding| (binding.name.clone(), axis.dimension_id))
            })
            .collect::<BTreeMap<_, _>>();
        let axes_by_dimension = axes
            .iter()
            .map(|axis| (axis.dimension_id, axis))
            .collect::<BTreeMap<_, _>>();

        let mut guard_atoms = Vec::new();
        for admission in checked.closed_query.admissions.iter() {
            let admission_index = u32::try_from(admission.admission_index)
                .map_err(|_| RelationalProofStrategyError::IndexConversion("admission index"))?;
            collect_direct_guard_atoms(
                &admission.predicate,
                RelationalGuardLayer::Admission {
                    admission_id: support_plan.admission_id(),
                    admission_index,
                },
                &axis_by_name,
                &axes_by_dimension,
                &mut Vec::new(),
                &mut guard_atoms,
                &mut residuals,
            )?;
        }
        if checked.closed_query.finds.len() != checked.find_question_ids().len()
            || checked
                .find_question_ids()
                .iter()
                .any(|candidate| *candidate != question_id)
        {
            return Err(RelationalProofStrategyError::CheckedPlanScopeMismatch);
        }
        for named_find in checked.closed_query.finds.iter() {
            if let Some(predicate) = named_find.find.predicate() {
                collect_direct_guard_atoms(
                    predicate,
                    RelationalGuardLayer::Selection { question_id },
                    &axis_by_name,
                    &axes_by_dimension,
                    &mut Vec::new(),
                    &mut guard_atoms,
                    &mut residuals,
                )?;
            }
        }
        guard_atoms.sort_by(|left, right| {
            (
                left.dimension_id,
                &left.origin,
                left.relation,
                left.coefficient,
                left.intercept,
            )
                .cmp(&(
                    right.dimension_id,
                    &right.origin,
                    right.relation,
                    right.coefficient,
                    right.intercept,
                ))
        });
        guard_atoms.dedup();

        Ok(Self {
            plan_root: support_plan.root(),
            relation_id: support_plan.relation_id(),
            axes: axes.into_boxed_slice(),
            guard_atoms: guard_atoms.into_boxed_slice(),
            residuals: residuals.into_boxed_slice(),
        })
    }

    pub(crate) const fn plan_root(&self) -> RelationalSupportPlanRoot {
        self.plan_root
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) fn axes(&self) -> &[RelationalIntegerAxis] {
        &self.axes
    }

    pub(crate) fn guard_atoms(&self) -> &[RelationalCheckedGuardAtom] {
        &self.guard_atoms
    }

    pub(crate) fn residuals(&self) -> &[RelationalResidualMaterialization] {
        &self.residuals
    }

    pub(crate) fn axis(
        &self,
        dimension_id: RelationalDimensionId,
    ) -> Option<&RelationalIntegerAxis> {
        self.axes
            .binary_search_by_key(&dimension_id, RelationalIntegerAxis::dimension_id)
            .ok()
            .map(|index| &self.axes[index])
    }

    pub(crate) fn plan_axis(
        &self,
        dimension_id: RelationalDimensionId,
        additional_checked_atoms: &[RelationalCheckedGuardAtom],
        certificate_obligation: Option<RelationalIntervalCertificateObligation>,
    ) -> Result<RelationalAxisProofPlan, RelationalProofStrategyError> {
        let axis = self
            .axis(dimension_id)
            .ok_or(RelationalProofStrategyError::UnknownAxis { dimension_id })?;
        let mut atoms = self
            .guard_atoms
            .iter()
            .filter(|atom| atom.dimension_id == dimension_id)
            .cloned()
            .collect::<Vec<_>>();
        for atom in additional_checked_atoms {
            if atom.plan_root != self.plan_root {
                return Err(RelationalProofStrategyError::GuardPlanMismatch);
            }
            if atom.dimension_id == dimension_id {
                atoms.push(atom.clone());
            }
        }
        atoms.sort_by(|left, right| {
            (
                &left.origin,
                left.relation,
                left.coefficient,
                left.intercept,
            )
                .cmp(&(
                    &right.origin,
                    right.relation,
                    right.coefficient,
                    right.intercept,
                ))
        });
        atoms.dedup();

        let mut candidates = BTreeMap::<u128, CandidateAccumulator>::new();
        for atom in &atoms {
            for image_cut in atom.relation.image_cuts() {
                let value_boundary =
                    inverse_affine_image_cut(atom.coefficient, atom.intercept, *image_cut)?;
                let Some(coordinate) =
                    axis.interior_coordinate_for_value_boundary(value_boundary)?
                else {
                    continue;
                };
                insert_candidate(
                    &mut candidates,
                    coordinate,
                    value_boundary,
                    RelationalSplitPriority::CheckedGuardBoundary,
                    RelationalSplitOrigin::CheckedGuard(atom.origin.clone()),
                )?;
            }
        }

        if let Some(obligation) = &certificate_obligation {
            obligation.validate_for(axis)?;
            match obligation.kind() {
                RelationalIntervalCertificateKind::Monotonicity { .. } => {
                    insert_balanced_candidate(axis, obligation, &mut candidates)?;
                }
                RelationalIntervalCertificateKind::PiecewiseUniform { value_boundaries } => {
                    for value_boundary in value_boundaries.iter().copied() {
                        let Some(coordinate) =
                            axis.interior_coordinate_for_value_boundary(value_boundary)?
                        else {
                            continue;
                        };
                        insert_candidate(
                            &mut candidates,
                            coordinate,
                            value_boundary,
                            RelationalSplitPriority::CertifiedPieceBoundary,
                            RelationalSplitOrigin::CertificateObligation(
                                obligation.target_obligation_id,
                            ),
                        )?;
                    }
                    if candidates.is_empty() {
                        insert_balanced_candidate(axis, obligation, &mut candidates)?;
                    }
                }
            }
        }

        let mut candidates = candidates
            .into_iter()
            .map(|(coordinate, accumulator)| RelationalSplitCandidate {
                coordinate,
                value_boundary: accumulator.value_boundary,
                priority: accumulator.priority,
                origins: accumulator.origins.into_iter().collect::<Vec<_>>().into(),
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| (candidate.priority, candidate.coordinate));
        let intervals = partition_intervals(axis, &candidates)?;
        let mut residual_materialization = intervals
            .iter()
            .map(|interval| RelationalResidualMaterialization {
                cell_id: Some(axis.cell.id()),
                dimension_id: Some(axis.dimension_id),
                coordinate_interval: Some((interval.start, interval.end_exclusive)),
                reason: RelationalStrategyResidualReason::IntervalCertificateNotAccepted,
            })
            .collect::<Vec<_>>();
        if let Some(obligation) = &certificate_obligation {
            if obligation.target_cell_id != axis.cell.id() {
                residual_materialization.push(RelationalResidualMaterialization {
                    cell_id: Some(obligation.target_cell_id),
                    dimension_id: Some(axis.dimension_id),
                    coordinate_interval: None,
                    reason: RelationalStrategyResidualReason::ExactRootAxisLiftUnavailable,
                });
            }
        }

        Ok(RelationalAxisProofPlan {
            axis: axis.clone(),
            candidates: candidates.into_boxed_slice(),
            intervals,
            certificate_obligation,
            residual_materialization: residual_materialization.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationalGuardLayer {
    Admission {
        admission_id: AdmissionId,
        admission_index: u32,
    },
    Selection {
        question_id: QuestionId,
    },
}

impl RelationalGuardLayer {
    fn origin(self, ast_path: &[u32]) -> RelationalGuardOrigin {
        match self {
            Self::Admission {
                admission_id,
                admission_index,
            } => RelationalGuardOrigin::Admission {
                admission_id,
                admission_index,
                ast_path: ast_path.to_vec().into_boxed_slice(),
            },
            Self::Selection { question_id } => RelationalGuardOrigin::Selection {
                question_id,
                ast_path: ast_path.to_vec().into_boxed_slice(),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct AffineSummary {
    coefficients: BTreeMap<RelationalDimensionId, i128>,
    intercept: i128,
    minimum: i128,
    maximum: i128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AffineParseIssue {
    Unsupported,
    RuntimeOverflow,
    ArithmeticOverflow,
}

#[allow(clippy::too_many_arguments)]
fn collect_direct_guard_atoms(
    expression: &Expr,
    layer: RelationalGuardLayer,
    axis_by_name: &BTreeMap<String, RelationalDimensionId>,
    axes_by_dimension: &BTreeMap<RelationalDimensionId, &RelationalIntegerAxis>,
    ast_path: &mut Vec<u32>,
    atoms: &mut Vec<RelationalCheckedGuardAtom>,
    residuals: &mut Vec<RelationalResidualMaterialization>,
) -> Result<(), RelationalProofStrategyError> {
    match &expression.kind {
        ExprKind::BinOp(operator, left, right) if operator == "&&" || operator == "||" => {
            ast_path.push(0);
            collect_direct_guard_atoms(
                left,
                layer,
                axis_by_name,
                axes_by_dimension,
                ast_path,
                atoms,
                residuals,
            )?;
            ast_path.pop();
            ast_path.push(1);
            collect_direct_guard_atoms(
                right,
                layer,
                axis_by_name,
                axes_by_dimension,
                ast_path,
                atoms,
                residuals,
            )?;
            ast_path.pop();
        }
        ExprKind::UnOp(operator, inner) if operator == "!" => {
            ast_path.push(0);
            collect_direct_guard_atoms(
                inner,
                layer,
                axis_by_name,
                axes_by_dimension,
                ast_path,
                atoms,
                residuals,
            )?;
            ast_path.pop();
        }
        ExprKind::Lit(Literal::Bool(_)) => {}
        ExprKind::BinOp(operator, left, right) => {
            let Some(relation) = guard_relation(operator) else {
                residuals.push(guard_residual(
                    axes_by_dimension,
                    RelationalStrategyResidualReason::GuardNeedsCheckedRuleNormalization,
                ));
                return Ok(());
            };
            let left = parse_affine(left, axis_by_name, axes_by_dimension);
            let right = parse_affine(right, axis_by_name, axes_by_dimension);
            let (left, right) = match (left, right) {
                (Ok(left), Ok(right)) => (left, right),
                (Err(AffineParseIssue::RuntimeOverflow), _)
                | (_, Err(AffineParseIssue::RuntimeOverflow))
                | (Err(AffineParseIssue::ArithmeticOverflow), _)
                | (_, Err(AffineParseIssue::ArithmeticOverflow)) => {
                    residuals.push(guard_residual(
                        axes_by_dimension,
                        RelationalStrategyResidualReason::GuardArithmeticMayOverflow,
                    ));
                    return Ok(());
                }
                _ => {
                    residuals.push(guard_residual(
                        axes_by_dimension,
                        RelationalStrategyResidualReason::GuardNeedsCheckedRuleNormalization,
                    ));
                    return Ok(());
                }
            };
            let difference = match comparison_difference(left, right) {
                Ok(difference) => difference,
                Err(_) => {
                    residuals.push(guard_residual(
                        axes_by_dimension,
                        RelationalStrategyResidualReason::GuardArithmeticMayOverflow,
                    ));
                    return Ok(());
                }
            };
            let nonzero = difference
                .coefficients
                .iter()
                .filter(|(_, coefficient)| **coefficient != 0)
                .collect::<Vec<_>>();
            match nonzero.as_slice() {
                [] => {}
                [(dimension_id, coefficient)] => {
                    let axis = axes_by_dimension.get(dimension_id).ok_or(
                        RelationalProofStrategyError::UnknownAxis {
                            dimension_id: **dimension_id,
                        },
                    )?;
                    // The operands were each proved runtime-safe. The i128
                    // difference is a mathematical comparison normal form and
                    // is not evaluated as a Futuruna subtraction.
                    affine_bounds_over_axis(axis, **coefficient, difference.intercept)?;
                    atoms.push(RelationalCheckedGuardAtom {
                        plan_root: axis.plan_root,
                        dimension_id: **dimension_id,
                        coefficient: **coefficient,
                        intercept: difference.intercept,
                        relation,
                        origin: layer.origin(ast_path),
                    });
                }
                _ => residuals.push(guard_residual(
                    axes_by_dimension,
                    RelationalStrategyResidualReason::GuardDependsOnMultipleAxes,
                )),
            }
        }
        _ => residuals.push(guard_residual(
            axes_by_dimension,
            RelationalStrategyResidualReason::GuardNeedsCheckedRuleNormalization,
        )),
    }
    Ok(())
}

fn guard_residual(
    axes_by_dimension: &BTreeMap<RelationalDimensionId, &RelationalIntegerAxis>,
    reason: RelationalStrategyResidualReason,
) -> RelationalResidualMaterialization {
    if axes_by_dimension.len() == 1 {
        let axis = axes_by_dimension.values().next().expect("one axis checked");
        RelationalResidualMaterialization {
            cell_id: Some(axis.cell.id()),
            dimension_id: Some(axis.dimension_id),
            coordinate_interval: Some((axis.coordinate_start, axis.coordinate_end_exclusive)),
            reason,
        }
    } else {
        RelationalResidualMaterialization {
            cell_id: None,
            dimension_id: None,
            coordinate_interval: None,
            reason,
        }
    }
}

fn guard_relation(operator: &str) -> Option<RelationalGuardRelation> {
    match operator {
        "<" => Some(RelationalGuardRelation::Less),
        "<=" => Some(RelationalGuardRelation::LessOrEqual),
        "==" => Some(RelationalGuardRelation::Equal),
        "!=" => Some(RelationalGuardRelation::NotEqual),
        ">=" => Some(RelationalGuardRelation::GreaterOrEqual),
        ">" => Some(RelationalGuardRelation::Greater),
        _ => None,
    }
}

fn parse_affine(
    expression: &Expr,
    axis_by_name: &BTreeMap<String, RelationalDimensionId>,
    axes_by_dimension: &BTreeMap<RelationalDimensionId, &RelationalIntegerAxis>,
) -> Result<AffineSummary, AffineParseIssue> {
    match &expression.kind {
        ExprKind::Lit(Literal::Int(value)) => Ok(AffineSummary {
            coefficients: BTreeMap::new(),
            intercept: i128::from(*value),
            minimum: i128::from(*value),
            maximum: i128::from(*value),
        }),
        ExprKind::Var(name) => {
            let dimension_id = axis_by_name
                .get(name)
                .copied()
                .ok_or(AffineParseIssue::Unsupported)?;
            let axis = axes_by_dimension
                .get(&dimension_id)
                .ok_or(AffineParseIssue::Unsupported)?;
            let maximum = i128::from(axis.value_end_exclusive)
                .checked_sub(1)
                .ok_or(AffineParseIssue::ArithmeticOverflow)?;
            Ok(AffineSummary {
                coefficients: BTreeMap::from([(dimension_id, 1)]),
                intercept: 0,
                minimum: i128::from(axis.value_start),
                maximum,
            })
        }
        ExprKind::UnOp(operator, inner) if operator == "+" => {
            parse_affine(inner, axis_by_name, axes_by_dimension)
        }
        ExprKind::UnOp(operator, inner) if operator == "-" => {
            let value = parse_affine(inner, axis_by_name, axes_by_dimension)?;
            scale_affine(value, -1)
        }
        ExprKind::BinOp(operator, left, right) if operator == "+" => add_affine(
            parse_affine(left, axis_by_name, axes_by_dimension)?,
            parse_affine(right, axis_by_name, axes_by_dimension)?,
        ),
        ExprKind::BinOp(operator, left, right) if operator == "-" => subtract_affine(
            parse_affine(left, axis_by_name, axes_by_dimension)?,
            parse_affine(right, axis_by_name, axes_by_dimension)?,
        ),
        ExprKind::BinOp(operator, left, right) if operator == "*" => {
            let left = parse_affine(left, axis_by_name, axes_by_dimension)?;
            let right = parse_affine(right, axis_by_name, axes_by_dimension)?;
            if left.coefficients.is_empty() {
                scale_affine(right, left.intercept)
            } else if right.coefficients.is_empty() {
                scale_affine(left, right.intercept)
            } else {
                Err(AffineParseIssue::Unsupported)
            }
        }
        _ => Err(AffineParseIssue::Unsupported),
    }
}

fn add_affine(
    mut left: AffineSummary,
    right: AffineSummary,
) -> Result<AffineSummary, AffineParseIssue> {
    for (dimension_id, coefficient) in right.coefficients {
        let next = left
            .coefficients
            .get(&dimension_id)
            .copied()
            .unwrap_or(0)
            .checked_add(coefficient)
            .ok_or(AffineParseIssue::ArithmeticOverflow)?;
        if next == 0 {
            left.coefficients.remove(&dimension_id);
        } else {
            left.coefficients.insert(dimension_id, next);
        }
    }
    let intercept = left
        .intercept
        .checked_add(right.intercept)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let minimum = left
        .minimum
        .checked_add(right.minimum)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let maximum = left
        .maximum
        .checked_add(right.maximum)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    ensure_runtime_int_bounds(minimum, maximum)?;
    Ok(AffineSummary {
        coefficients: left.coefficients,
        intercept,
        minimum,
        maximum,
    })
}

fn subtract_affine(
    left: AffineSummary,
    right: AffineSummary,
) -> Result<AffineSummary, AffineParseIssue> {
    add_affine(left, scale_affine(right, -1)?)
}

/// Mathematical comparison normal form. Unlike an authored subtraction, this
/// difference is never evaluated by the Futuruna runtime and therefore need
/// not fit `i64`; both authored operands were checked separately above.
fn comparison_difference(
    mut left: AffineSummary,
    right: AffineSummary,
) -> Result<AffineSummary, AffineParseIssue> {
    for (dimension_id, coefficient) in right.coefficients {
        let next = left
            .coefficients
            .get(&dimension_id)
            .copied()
            .unwrap_or(0)
            .checked_sub(coefficient)
            .ok_or(AffineParseIssue::ArithmeticOverflow)?;
        if next == 0 {
            left.coefficients.remove(&dimension_id);
        } else {
            left.coefficients.insert(dimension_id, next);
        }
    }
    let intercept = left
        .intercept
        .checked_sub(right.intercept)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let minimum = left
        .minimum
        .checked_sub(right.maximum)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let maximum = left
        .maximum
        .checked_sub(right.minimum)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    Ok(AffineSummary {
        coefficients: left.coefficients,
        intercept,
        minimum,
        maximum,
    })
}

fn scale_affine(value: AffineSummary, scalar: i128) -> Result<AffineSummary, AffineParseIssue> {
    let coefficients = value
        .coefficients
        .into_iter()
        .map(|(dimension_id, coefficient)| {
            coefficient
                .checked_mul(scalar)
                .map(|coefficient| (dimension_id, coefficient))
                .ok_or(AffineParseIssue::ArithmeticOverflow)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let intercept = value
        .intercept
        .checked_mul(scalar)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let first = value
        .minimum
        .checked_mul(scalar)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let second = value
        .maximum
        .checked_mul(scalar)
        .ok_or(AffineParseIssue::ArithmeticOverflow)?;
    let minimum = first.min(second);
    let maximum = first.max(second);
    ensure_runtime_int_bounds(minimum, maximum)?;
    Ok(AffineSummary {
        coefficients,
        intercept,
        minimum,
        maximum,
    })
}

fn ensure_runtime_int_bounds(minimum: i128, maximum: i128) -> Result<(), AffineParseIssue> {
    if minimum < i128::from(i64::MIN) || maximum > i128::from(i64::MAX) {
        Err(AffineParseIssue::RuntimeOverflow)
    } else {
        Ok(())
    }
}

fn affine_bounds_over_axis(
    axis: &RelationalIntegerAxis,
    coefficient: i128,
    intercept: i128,
) -> Result<(i128, i128), RelationalProofStrategyError> {
    let first = coefficient
        .checked_mul(i128::from(axis.value_start))
        .and_then(|value| value.checked_add(intercept))
        .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
            "bounding an affine guard over an integer axis",
        ))?;
    let final_value = i128::from(axis.value_end_exclusive).checked_sub(1).ok_or(
        RelationalProofStrategyError::ArithmeticOverflow(
            "bounding an affine guard over an integer axis",
        ),
    )?;
    let last = coefficient
        .checked_mul(final_value)
        .and_then(|value| value.checked_add(intercept))
        .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
            "bounding an affine guard over an integer axis",
        ))?;
    Ok((first.min(last), first.max(last)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMonotonicityDirection {
    Nondecreasing,
    Nonincreasing,
}

/// Proposition that must be discharged before interval reasoning can replace
/// exact materialization. It intentionally contains no solver/backend choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalIntervalCertificateKind {
    Monotonicity {
        direction: RelationalMonotonicityDirection,
    },
    PiecewiseUniform {
        /// Exact integer-value cell boundaries proposed by the checked
        /// normalizer. They are hints until the obligation is accepted.
        value_boundaries: Box<[i128]>,
    },
}

/// Explicit certificate obligation authorizing interval-oriented proof work.
/// This is not accepted evidence and has no constructor that claims closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalIntervalCertificateObligation {
    plan_root: RelationalSupportPlanRoot,
    axis_cell_id: SupportCellId,
    dimension_id: RelationalDimensionId,
    target_cell_id: SupportCellId,
    target_obligation_id: SupportProofObligationId,
    semantic_subject_digest: [u8; 32],
    kind: RelationalIntervalCertificateKind,
}

impl RelationalIntervalCertificateObligation {
    pub(crate) fn monotonicity(
        axis: &RelationalIntegerAxis,
        target: &SupportObligationRecord,
        semantic_subject_digest: [u8; 32],
        direction: RelationalMonotonicityDirection,
    ) -> Self {
        Self {
            plan_root: axis.plan_root,
            axis_cell_id: axis.cell.id(),
            dimension_id: axis.dimension_id,
            target_cell_id: target.cell_id(),
            target_obligation_id: target.id(),
            semantic_subject_digest,
            kind: RelationalIntervalCertificateKind::Monotonicity { direction },
        }
    }

    pub(crate) fn piecewise_uniform(
        axis: &RelationalIntegerAxis,
        target: &SupportObligationRecord,
        semantic_subject_digest: [u8; 32],
        mut value_boundaries: Vec<i128>,
    ) -> Self {
        value_boundaries.sort();
        value_boundaries.dedup();
        Self {
            plan_root: axis.plan_root,
            axis_cell_id: axis.cell.id(),
            dimension_id: axis.dimension_id,
            target_cell_id: target.cell_id(),
            target_obligation_id: target.id(),
            semantic_subject_digest,
            kind: RelationalIntervalCertificateKind::PiecewiseUniform {
                value_boundaries: value_boundaries.into_boxed_slice(),
            },
        }
    }

    pub(crate) const fn target_cell_id(&self) -> SupportCellId {
        self.target_cell_id
    }

    pub(crate) const fn target_obligation_id(&self) -> SupportProofObligationId {
        self.target_obligation_id
    }

    pub(crate) const fn semantic_subject_digest(&self) -> [u8; 32] {
        self.semantic_subject_digest
    }

    pub(crate) const fn kind(&self) -> &RelationalIntervalCertificateKind {
        &self.kind
    }

    fn validate_for(
        &self,
        axis: &RelationalIntegerAxis,
    ) -> Result<(), RelationalProofStrategyError> {
        if self.plan_root != axis.plan_root
            || self.axis_cell_id != axis.cell.id()
            || self.dimension_id != axis.dimension_id
        {
            return Err(RelationalProofStrategyError::CertificateAxisMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalSplitPriority {
    CheckedGuardBoundary,
    CertifiedPieceBoundary,
    CertificateAuthorizedMidpoint,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalSplitOrigin {
    CheckedGuard(RelationalGuardOrigin),
    CertificateObligation(SupportProofObligationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSplitCandidate {
    coordinate: u128,
    value_boundary: i128,
    priority: RelationalSplitPriority,
    origins: Box<[RelationalSplitOrigin]>,
}

impl RelationalSplitCandidate {
    pub(crate) const fn coordinate(&self) -> u128 {
        self.coordinate
    }

    pub(crate) const fn value_boundary(&self) -> i128 {
        self.value_boundary
    }

    pub(crate) const fn priority(&self) -> RelationalSplitPriority {
        self.priority
    }

    pub(crate) fn origins(&self) -> &[RelationalSplitOrigin] {
        &self.origins
    }
}

struct CandidateAccumulator {
    value_boundary: i128,
    priority: RelationalSplitPriority,
    origins: BTreeSet<RelationalSplitOrigin>,
}

fn insert_candidate(
    candidates: &mut BTreeMap<u128, CandidateAccumulator>,
    coordinate: u128,
    value_boundary: i128,
    priority: RelationalSplitPriority,
    origin: RelationalSplitOrigin,
) -> Result<(), RelationalProofStrategyError> {
    match candidates.get_mut(&coordinate) {
        Some(existing) => {
            if existing.value_boundary != value_boundary {
                return Err(RelationalProofStrategyError::CandidateValueMismatch { coordinate });
            }
            existing.priority = existing.priority.min(priority);
            existing.origins.insert(origin);
        }
        None => {
            candidates.insert(
                coordinate,
                CandidateAccumulator {
                    value_boundary,
                    priority,
                    origins: BTreeSet::from([origin]),
                },
            );
        }
    }
    Ok(())
}

fn insert_balanced_candidate(
    axis: &RelationalIntegerAxis,
    obligation: &RelationalIntervalCertificateObligation,
    candidates: &mut BTreeMap<u128, CandidateAccumulator>,
) -> Result<(), RelationalProofStrategyError> {
    if axis.cardinality() <= 1 {
        return Ok(());
    }
    let coordinate = axis
        .coordinate_start
        .checked_add(axis.cardinality() / 2)
        .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
            "computing a certificate-authorized midpoint",
        ))?;
    if coordinate == axis.coordinate_start || coordinate == axis.coordinate_end_exclusive {
        return Ok(());
    }
    let value_boundary = axis.value_boundary_for_coordinate(coordinate)?;
    insert_candidate(
        candidates,
        coordinate,
        value_boundary,
        RelationalSplitPriority::CertificateAuthorizedMidpoint,
        RelationalSplitOrigin::CertificateObligation(obligation.target_obligation_id),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalOrdinalInterval {
    start: u128,
    end_exclusive: u128,
}

impl RelationalOrdinalInterval {
    pub(crate) const fn start(self) -> u128 {
        self.start
    }

    pub(crate) const fn end_exclusive(self) -> u128 {
        self.end_exclusive
    }

    pub(crate) const fn cardinality(self) -> u128 {
        self.end_exclusive - self.start
    }
}

/// One exact split proposal plus mandatory fallback work. Even when every
/// candidate has been visited, `establishes_complement_closure()` remains
/// false until the evidence catalog contains accepted typed conclusions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalAxisProofPlan {
    axis: RelationalIntegerAxis,
    candidates: Box<[RelationalSplitCandidate]>,
    intervals: Box<[RelationalOrdinalInterval]>,
    certificate_obligation: Option<RelationalIntervalCertificateObligation>,
    residual_materialization: Box<[RelationalResidualMaterialization]>,
}

impl RelationalAxisProofPlan {
    pub(crate) const fn axis(&self) -> &RelationalIntegerAxis {
        &self.axis
    }

    pub(crate) fn candidates(&self) -> &[RelationalSplitCandidate] {
        &self.candidates
    }

    pub(crate) fn intervals(&self) -> &[RelationalOrdinalInterval] {
        &self.intervals
    }

    pub(crate) const fn certificate_obligation(
        &self,
    ) -> Option<&RelationalIntervalCertificateObligation> {
        self.certificate_obligation.as_ref()
    }

    pub(crate) fn residual_materialization(&self) -> &[RelationalResidualMaterialization] {
        &self.residual_materialization
    }

    pub(crate) const fn establishes_complement_closure(&self) -> bool {
        false
    }

    /// Materialize the proposed cuts as a structurally exact interval
    /// partition and clone each supplied typed claim onto every child.
    /// Classification evidence is deliberately absent from the result.
    pub(crate) fn structural_refinement(
        &self,
        parent_obligations: &[SupportObligationRecord],
    ) -> Result<RelationalStructuralAxisRefinement, RelationalProofStrategyError> {
        if self.candidates.is_empty() {
            return Err(RelationalProofStrategyError::NoInteriorSplitCandidate);
        }
        for obligation in parent_obligations {
            if obligation.cell_id() != self.axis.cell.id() {
                return Err(RelationalProofStrategyError::ObligationCellMismatch {
                    obligation_id: obligation.id(),
                    expected: self.axis.cell.id(),
                    actual: obligation.cell_id(),
                });
            }
        }
        let children = self
            .intervals
            .iter()
            .map(|interval| {
                SupportCell::new(
                    self.axis.cell.space(),
                    SupportExpr::ordinal_interval(interval.start, interval.end_exclusive)?,
                    self.axis.cell.materializer_id(),
                )
                .map_err(RelationalProofStrategyError::SupportCell)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let partition =
            SupportPartitionCertificate::ordinal_interval_cover(&self.axis.cell, children.clone())?;
        let child_obligations = parent_obligations
            .iter()
            .map(|parent| {
                let children = children
                    .iter()
                    .map(|child| clone_obligation_for_child(parent, child))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(RelationalChildObligationSet {
                    parent_obligation_id: parent.id(),
                    children: children.into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, RelationalProofStrategyError>>()?;
        Ok(RelationalStructuralAxisRefinement {
            parent_cell_id: self.axis.cell.id(),
            children: children.into_boxed_slice(),
            partition,
            child_obligations: child_obligations.into_boxed_slice(),
        })
    }
}

fn partition_intervals(
    axis: &RelationalIntegerAxis,
    candidates: &[RelationalSplitCandidate],
) -> Result<Box<[RelationalOrdinalInterval]>, RelationalProofStrategyError> {
    let mut cuts = BTreeSet::from([axis.coordinate_start, axis.coordinate_end_exclusive]);
    for candidate in candidates {
        if candidate.coordinate <= axis.coordinate_start
            || candidate.coordinate >= axis.coordinate_end_exclusive
        {
            return Err(RelationalProofStrategyError::AxisCoordinateMismatch {
                dimension_id: axis.dimension_id,
            });
        }
        cuts.insert(candidate.coordinate);
    }
    let cuts = cuts.into_iter().collect::<Vec<_>>();
    Ok(cuts
        .windows(2)
        .map(|bounds| RelationalOrdinalInterval {
            start: bounds[0],
            end_exclusive: bounds[1],
        })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn inverse_affine_image_cut(
    coefficient: i128,
    intercept: i128,
    image_cut: i128,
) -> Result<i128, RelationalProofStrategyError> {
    if coefficient == 0 {
        return Err(RelationalProofStrategyError::ConstantGuardAtom);
    }
    if coefficient > 0 {
        ceil_div(
            image_cut.checked_sub(intercept).ok_or(
                RelationalProofStrategyError::ArithmeticOverflow("inverting an affine guard cut"),
            )?,
            coefficient,
        )
    } else {
        let positive =
            coefficient
                .checked_neg()
                .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
                    "inverting an affine guard cut",
                ))?;
        floor_div(
            intercept.checked_sub(image_cut).ok_or(
                RelationalProofStrategyError::ArithmeticOverflow("inverting an affine guard cut"),
            )?,
            positive,
        )?
        .checked_add(1)
        .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
            "inverting an affine guard cut",
        ))
    }
}

fn floor_div(
    numerator: i128,
    positive_denominator: i128,
) -> Result<i128, RelationalProofStrategyError> {
    if positive_denominator <= 0 {
        return Err(RelationalProofStrategyError::ArithmeticOverflow(
            "performing Euclidean floor division",
        ));
    }
    numerator.checked_div_euclid(positive_denominator).ok_or(
        RelationalProofStrategyError::ArithmeticOverflow("performing Euclidean floor division"),
    )
}

fn ceil_div(
    numerator: i128,
    positive_denominator: i128,
) -> Result<i128, RelationalProofStrategyError> {
    let floor = floor_div(numerator, positive_denominator)?;
    let remainder = numerator.checked_rem_euclid(positive_denominator).ok_or(
        RelationalProofStrategyError::ArithmeticOverflow("performing Euclidean ceil division"),
    )?;
    if remainder == 0 {
        Ok(floor)
    } else {
        floor
            .checked_add(1)
            .ok_or(RelationalProofStrategyError::ArithmeticOverflow(
                "performing Euclidean ceil division",
            ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalChildObligationSet {
    parent_obligation_id: SupportProofObligationId,
    children: Box<[SupportObligationRecord]>,
}

impl RelationalChildObligationSet {
    pub(crate) const fn parent_obligation_id(&self) -> SupportProofObligationId {
        self.parent_obligation_id
    }

    pub(crate) fn children(&self) -> &[SupportObligationRecord] {
        &self.children
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalStructuralAxisRefinement {
    parent_cell_id: SupportCellId,
    children: Box<[SupportCell]>,
    partition: SupportPartitionCertificate,
    child_obligations: Box<[RelationalChildObligationSet]>,
}

impl RelationalStructuralAxisRefinement {
    pub(crate) const fn parent_cell_id(&self) -> SupportCellId {
        self.parent_cell_id
    }

    pub(crate) fn children(&self) -> &[SupportCell] {
        &self.children
    }

    pub(crate) const fn partition(&self) -> &SupportPartitionCertificate {
        &self.partition
    }

    pub(crate) fn child_obligations(&self) -> &[RelationalChildObligationSet] {
        &self.child_obligations
    }

    pub(crate) const fn carries_classification_evidence(&self) -> bool {
        false
    }
}

fn clone_obligation_for_child(
    parent: &SupportObligationRecord,
    child: &SupportCell,
) -> Result<SupportObligationRecord, RelationalProofStrategyError> {
    use super::support_cell::SupportCellObligation;

    let obligation = match parent {
        SupportObligationRecord::Cardinality(parent) => SupportObligationRecord::Cardinality(
            SupportCellObligation::new(child, *parent.claim())?,
        ),
        SupportObligationRecord::Injectivity(parent) => SupportObligationRecord::Injectivity(
            SupportCellObligation::new(child, *parent.claim())?,
        ),
        SupportObligationRecord::Admission(parent) => {
            SupportObligationRecord::Admission(SupportCellObligation::new(child, *parent.claim())?)
        }
        SupportObligationRecord::Selection(parent) => {
            SupportObligationRecord::Selection(SupportCellObligation::new(child, *parent.claim())?)
        }
        SupportObligationRecord::UniformValue(parent) => SupportObligationRecord::UniformValue(
            SupportCellObligation::new(child, *parent.claim())?,
        ),
        SupportObligationRecord::UniformMechanism(parent) => {
            SupportObligationRecord::UniformMechanism(SupportCellObligation::new(
                child,
                *parent.claim(),
            )?)
        }
    };
    Ok(obligation)
}

/// Why exact selected support is known without expanding cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalExactSelectedSupportBasis {
    /// The support planner proved the whole case relation structurally empty,
    /// so no illegal empty cell and no vacuous selection obligation exists.
    StaticExactEmpty {
        plan_root: RelationalSupportPlanRoot,
        admission_id: AdmissionId,
        question_id: QuestionId,
    },
    /// An accepted exact partition covers the case root and every active leaf
    /// carries exact count plus the admission/selection facts needed to count
    /// selected cases.
    AcceptedLeafEvidence {
        plan_root: RelationalSupportPlanRoot,
        support_evidence_root: SupportEvidenceRoot,
        case_root_cell_id: SupportCellId,
        leaves: Box<[RelationalSelectedLeafEvidence]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalSelectedLeafEvidence {
    cell_id: SupportCellId,
    exact_cardinality: u128,
    cardinality_evidence_id: SupportCellEvidenceId,
    admission: AdmissionDecision,
    admission_evidence_id: SupportCellEvidenceId,
    selection: Option<SelectionDecision>,
    selection_evidence_id: Option<SupportCellEvidenceId>,
}

impl RelationalSelectedLeafEvidence {
    pub(crate) const fn cell_id(&self) -> SupportCellId {
        self.cell_id
    }

    pub(crate) const fn exact_cardinality(&self) -> u128 {
        self.exact_cardinality
    }

    pub(crate) const fn cardinality_evidence_id(&self) -> SupportCellEvidenceId {
        self.cardinality_evidence_id
    }

    pub(crate) const fn admission(&self) -> AdmissionDecision {
        self.admission
    }

    pub(crate) const fn admission_evidence_id(&self) -> SupportCellEvidenceId {
        self.admission_evidence_id
    }

    pub(crate) const fn selection(&self) -> Option<SelectionDecision> {
        self.selection
    }

    pub(crate) const fn selection_evidence_id(&self) -> Option<SupportCellEvidenceId> {
        self.selection_evidence_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalExactSelectedSupport {
    selected_cardinality: u128,
    basis: RelationalExactSelectedSupportBasis,
}

impl RelationalExactSelectedSupport {
    pub(crate) const fn selected_cardinality(&self) -> u128 {
        self.selected_cardinality
    }

    pub(crate) const fn is_exact_empty(&self) -> bool {
        self.selected_cardinality == 0
    }

    pub(crate) const fn basis(&self) -> &RelationalExactSelectedSupportBasis {
        &self.basis
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalSelectedSupportResidual {
    SupportFrontierOpen,
    ObligationFrontierOpen,
    CaseRootNotDeclared(SupportCellId),
    CaseRootCellMissing(SupportCellId),
    PartitionChildCellMissing(SupportCellId),
    LeafNotActive(SupportCellId),
    ExactCardinalityEvidenceMissing(SupportCellId),
    AdmissionEvidenceMissing(SupportCellId),
    SelectionEvidenceMissing(SupportCellId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalSelectedSupportAssessment {
    Exact(RelationalExactSelectedSupport),
    Open {
        residuals: Box<[RelationalSelectedSupportResidual]>,
    },
}

impl RelationalSelectedSupportAssessment {
    pub(crate) const fn exact(&self) -> Option<&RelationalExactSelectedSupport> {
        match self {
            Self::Exact(exact) => Some(exact),
            Self::Open { .. } => None,
        }
    }

    pub(crate) fn residuals(&self) -> &[RelationalSelectedSupportResidual] {
        match self {
            Self::Exact(_) => &[],
            Self::Open { residuals } => residuals,
        }
    }
}

/// Derive exact selected support only from structural exact emptiness or a
/// complete accepted support/evidence frontier.
///
/// For a nonempty planned root, every active leaf needs exact cardinality and
/// admission evidence. An admitted leaf additionally needs accepted selection
/// evidence; rejected leaves contribute zero selected cases without inventing
/// a selection obligation. The selected cardinality is the checked sum of the
/// exact cardinalities of leaves concluded `Selected`.
pub(crate) fn assess_relational_selected_support(
    support_plan: &RelationalSupportPlan,
    snapshot: &SupportEvidenceSnapshot,
) -> Result<RelationalSelectedSupportAssessment, RelationalProofStrategyError> {
    if !support_plan.validate_root() {
        return Err(RelationalProofStrategyError::SupportPlanRootMismatch);
    }
    let question_id = require_single_question(support_plan.question_ids())?;
    match support_plan.root_obligations() {
        RelationalRootObligationPlan::ResolvedExactEmpty { admission_id } => {
            return Ok(RelationalSelectedSupportAssessment::Exact(
                RelationalExactSelectedSupport {
                    selected_cardinality: 0,
                    basis: RelationalExactSelectedSupportBasis::StaticExactEmpty {
                        plan_root: support_plan.root(),
                        admission_id: *admission_id,
                        question_id,
                    },
                },
            ));
        }
        RelationalRootObligationPlan::CellBacked { root_cell_id, .. } => {
            assess_cell_backed_selected_support(support_plan, snapshot, *root_cell_id, question_id)
        }
    }
}

fn assess_cell_backed_selected_support(
    support_plan: &RelationalSupportPlan,
    snapshot: &SupportEvidenceSnapshot,
    root_cell_id: SupportCellId,
    question_id: QuestionId,
) -> Result<RelationalSelectedSupportAssessment, RelationalProofStrategyError> {
    let mut residuals = BTreeSet::new();
    if !snapshot.support_frontier_is_complete() {
        residuals.insert(RelationalSelectedSupportResidual::SupportFrontierOpen);
    }
    if !snapshot.obligation_frontier_is_complete() {
        residuals.insert(RelationalSelectedSupportResidual::ObligationFrontierOpen);
    }
    if !snapshot.root_cell_ids().any(|id| id == root_cell_id) {
        residuals.insert(RelationalSelectedSupportResidual::CaseRootNotDeclared(
            root_cell_id,
        ));
    }
    if snapshot.cell(root_cell_id).is_none() {
        residuals.insert(RelationalSelectedSupportResidual::CaseRootCellMissing(
            root_cell_id,
        ));
    }
    if !residuals.is_empty() {
        return Ok(RelationalSelectedSupportAssessment::Open {
            residuals: residuals.into_iter().collect::<Vec<_>>().into(),
        });
    }

    let mut leaves = Vec::new();
    collect_partition_leaves(snapshot, root_cell_id, &mut leaves, &mut residuals)?;
    let active_cells = snapshot.active_leaf_ids().collect::<BTreeSet<_>>();
    let active_obligations = snapshot
        .active_obligation_leaf_ids()
        .collect::<BTreeSet<_>>();
    let evidence_index = index_selected_leaf_evidence(
        snapshot,
        support_plan.admission_id(),
        question_id,
        &active_obligations,
    )?;
    let mut leaf_evidence = Vec::new();
    let mut selected_cardinality = 0_u128;

    for leaf_id in leaves {
        if !active_cells.contains(&leaf_id) {
            residuals.insert(RelationalSelectedSupportResidual::LeafNotActive(leaf_id));
            continue;
        }
        let Some(&(cardinality, cardinality_evidence_id)) =
            evidence_index.cardinality.get(&leaf_id)
        else {
            residuals.insert(
                RelationalSelectedSupportResidual::ExactCardinalityEvidenceMissing(leaf_id),
            );
            continue;
        };
        let Some(&(admission, admission_evidence_id)) = evidence_index.admission.get(&leaf_id)
        else {
            residuals.insert(RelationalSelectedSupportResidual::AdmissionEvidenceMissing(
                leaf_id,
            ));
            continue;
        };

        let (selection, selection_evidence_id) = match admission {
            AdmissionDecision::Rejected => (None, None),
            AdmissionDecision::Admitted => {
                let Some(&(selection, evidence_id)) = evidence_index.selection.get(&leaf_id) else {
                    residuals.insert(RelationalSelectedSupportResidual::SelectionEvidenceMissing(
                        leaf_id,
                    ));
                    continue;
                };
                if selection == SelectionDecision::Selected {
                    selected_cardinality = selected_cardinality
                        .checked_add(cardinality)
                        .ok_or(RelationalProofStrategyError::SelectedCardinalityOverflow)?;
                }
                (Some(selection), Some(evidence_id))
            }
        };
        leaf_evidence.push(RelationalSelectedLeafEvidence {
            cell_id: leaf_id,
            exact_cardinality: cardinality,
            cardinality_evidence_id,
            admission,
            admission_evidence_id,
            selection,
            selection_evidence_id,
        });
    }

    if !residuals.is_empty() {
        return Ok(RelationalSelectedSupportAssessment::Open {
            residuals: residuals.into_iter().collect::<Vec<_>>().into(),
        });
    }
    leaf_evidence.sort_by_key(RelationalSelectedLeafEvidence::cell_id);
    Ok(RelationalSelectedSupportAssessment::Exact(
        RelationalExactSelectedSupport {
            selected_cardinality,
            basis: RelationalExactSelectedSupportBasis::AcceptedLeafEvidence {
                plan_root: support_plan.root(),
                support_evidence_root: snapshot.root(),
                case_root_cell_id: root_cell_id,
                leaves: leaf_evidence.into_boxed_slice(),
            },
        },
    ))
}

fn collect_partition_leaves(
    snapshot: &SupportEvidenceSnapshot,
    root_cell_id: SupportCellId,
    leaves: &mut Vec<SupportCellId>,
    residuals: &mut BTreeSet<RelationalSelectedSupportResidual>,
) -> Result<(), RelationalProofStrategyError> {
    let mut stack = vec![root_cell_id];
    let mut seen = BTreeSet::new();
    while let Some(cell_id) = stack.pop() {
        if !seen.insert(cell_id) {
            return Err(RelationalProofStrategyError::PartitionDagReusesCell { cell_id });
        }
        let Some(partition) = snapshot.partition_for_parent(cell_id) else {
            leaves.push(cell_id);
            continue;
        };
        partition.validate()?;
        for child_id in partition.child_ids().iter().rev() {
            if snapshot.cell(*child_id).is_none() {
                residuals.insert(
                    RelationalSelectedSupportResidual::PartitionChildCellMissing(*child_id),
                );
                continue;
            }
            stack.push(*child_id);
        }
    }
    Ok(())
}

#[derive(Default)]
struct SelectedLeafEvidenceIndex {
    cardinality: BTreeMap<SupportCellId, (u128, SupportCellEvidenceId)>,
    admission: BTreeMap<SupportCellId, (AdmissionDecision, SupportCellEvidenceId)>,
    selection: BTreeMap<SupportCellId, (SelectionDecision, SupportCellEvidenceId)>,
}

fn index_selected_leaf_evidence(
    snapshot: &SupportEvidenceSnapshot,
    admission_id: AdmissionId,
    question_id: QuestionId,
    active_obligations: &BTreeSet<SupportProofObligationId>,
) -> Result<SelectedLeafEvidenceIndex, RelationalProofStrategyError> {
    let mut index = SelectedLeafEvidenceIndex::default();
    for obligation in snapshot.obligations() {
        if !active_obligations.contains(&obligation.id()) {
            continue;
        }
        match obligation {
            SupportObligationRecord::Cardinality(obligation) => {
                let cell = snapshot.cell(obligation.cell_id()).ok_or(
                    RelationalProofStrategyError::SnapshotCellMissing {
                        cell_id: obligation.cell_id(),
                    },
                )?;
                for evidence in snapshot.evidence_for_obligation(obligation.id()) {
                    let SupportEvidenceRecord::Cardinality(evidence) = evidence else {
                        continue;
                    };
                    let exact = cell.cardinality_from_evidence(evidence)?.exact().ok_or(
                        RelationalProofStrategyError::AcceptedCardinalityNotExact {
                            cell_id: cell.id(),
                        },
                    )?;
                    index
                        .cardinality
                        .entry(cell.id())
                        .or_insert((exact, evidence.id()));
                    break;
                }
            }
            SupportObligationRecord::Admission(obligation)
                if obligation.claim().admission_id() == admission_id =>
            {
                for evidence in snapshot.evidence_for_obligation(obligation.id()) {
                    let SupportEvidenceRecord::Admission(evidence) = evidence else {
                        continue;
                    };
                    index
                        .admission
                        .entry(obligation.cell_id())
                        .or_insert((*evidence.conclusion(), evidence.id()));
                    break;
                }
            }
            SupportObligationRecord::Selection(obligation)
                if obligation.claim().question_id() == question_id =>
            {
                for evidence in snapshot.evidence_for_obligation(obligation.id()) {
                    let SupportEvidenceRecord::Selection(evidence) = evidence else {
                        continue;
                    };
                    index
                        .selection
                        .entry(obligation.cell_id())
                        .or_insert((*evidence.conclusion(), evidence.id()));
                    break;
                }
            }
            SupportObligationRecord::Injectivity(_)
            | SupportObligationRecord::Admission(_)
            | SupportObligationRecord::Selection(_)
            | SupportObligationRecord::UniformValue(_)
            | SupportObligationRecord::UniformMechanism(_) => {}
        }
    }
    Ok(index)
}

fn require_single_question(
    question_ids: &[QuestionId],
) -> Result<QuestionId, RelationalProofStrategyError> {
    let [question_id] = question_ids else {
        return Err(RelationalProofStrategyError::QuestionArityMismatch {
            actual: question_ids.len(),
        });
    };
    Ok(*question_id)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalProofStrategyError {
    InvalidQuery(String),
    QuestionArityMismatch {
        actual: usize,
    },
    SupportPlanner(RelationalSupportPlannerError),
    SupportPlanRootMismatch,
    CheckedPlanScopeMismatch,
    IndexConversion(&'static str),
    BindingStageMismatch {
        binding_index: u32,
    },
    AxisCellShapeMismatch {
        dimension_id: RelationalDimensionId,
    },
    AxisCoordinateMismatch {
        dimension_id: RelationalDimensionId,
    },
    UnknownAxis {
        dimension_id: RelationalDimensionId,
    },
    GuardPlanMismatch,
    ConstantGuardAtom,
    ArithmeticOverflow(&'static str),
    CertificateAxisMismatch,
    CandidateValueMismatch {
        coordinate: u128,
    },
    NoInteriorSplitCandidate,
    ObligationCellMismatch {
        obligation_id: SupportProofObligationId,
        expected: SupportCellId,
        actual: SupportCellId,
    },
    SnapshotCellMissing {
        cell_id: SupportCellId,
    },
    PartitionDagReusesCell {
        cell_id: SupportCellId,
    },
    AcceptedCardinalityNotExact {
        cell_id: SupportCellId,
    },
    SelectedCardinalityOverflow,
    SupportCell(SupportCellError),
}

impl From<SupportCellError> for RelationalProofStrategyError {
    fn from(error: SupportCellError) -> Self {
        Self::SupportCell(error)
    }
}

impl From<RelationalSupportPlannerError> for RelationalProofStrategyError {
    fn from(error: RelationalSupportPlannerError) -> Self {
        Self::SupportPlanner(error)
    }
}

impl fmt::Display for RelationalProofStrategyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(formatter, "invalid relational query: {message}"),
            Self::QuestionArityMismatch { actual } => write!(
                formatter,
                "relational proof optimization requires exactly one semantic question, found {actual}"
            ),
            Self::SupportPlanner(error) => {
                write!(formatter, "invalid relational support plan input: {error}")
            }
            Self::SupportPlanRootMismatch => {
                formatter.write_str("relational support-plan root does not match its payload")
            }
            Self::CheckedPlanScopeMismatch => formatter.write_str(
                "checked query and relational support plan have different semantic scope",
            ),
            Self::IndexConversion(subject) => {
                write!(
                    formatter,
                    "{subject} does not fit the durable integer schema"
                )
            }
            Self::BindingStageMismatch { binding_index } => write!(
                formatter,
                "support stage {binding_index} does not match its checked source binding"
            ),
            Self::AxisCellShapeMismatch { .. } => formatter.write_str(
                "independent checked integer range does not have ordinal-interval support",
            ),
            Self::AxisCoordinateMismatch { .. } => formatter
                .write_str("integer values and support coordinates do not describe the same range"),
            Self::UnknownAxis { .. } => {
                formatter.write_str("requested relational integer axis is absent")
            }
            Self::GuardPlanMismatch => {
                formatter.write_str("checked guard atom belongs to another support plan")
            }
            Self::ConstantGuardAtom => {
                formatter.write_str("constant guard atom has no integer-axis split")
            }
            Self::ArithmeticOverflow(context) => {
                write!(
                    formatter,
                    "proof-strategy arithmetic overflow while {context}"
                )
            }
            Self::CertificateAxisMismatch => formatter.write_str(
                "interval certificate obligation belongs to another plan, cell, or dimension",
            ),
            Self::CandidateValueMismatch { coordinate } => write!(
                formatter,
                "split coordinate {coordinate} was paired with two integer boundaries"
            ),
            Self::NoInteriorSplitCandidate => {
                formatter.write_str("axis proof plan has no interior split candidate")
            }
            Self::ObligationCellMismatch { .. } => {
                formatter.write_str("proof obligation belongs to another support cell")
            }
            Self::SnapshotCellMissing { .. } => {
                formatter.write_str("support-evidence snapshot is missing a partition cell")
            }
            Self::PartitionDagReusesCell { .. } => formatter
                .write_str("one support cell appears more than once below the selected case root"),
            Self::AcceptedCardinalityNotExact { .. } => formatter
                .write_str("accepted cardinality evidence did not yield an exact cardinality"),
            Self::SelectedCardinalityOverflow => {
                formatter.write_str("exact selected cardinality exceeds u128::MAX")
            }
            Self::SupportCell(error) => {
                write!(formatter, "invalid support-cell proof work: {error}")
            }
        }
    }
}

impl Error for RelationalProofStrategyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SupportPlanner(error) => Some(error),
            Self::SupportCell(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::relational_support_planner::RelationalSupportPlanner;
    use super::*;
    use crate::{Lexer, Parser, TypeChecker};

    const FINITE_AXIS_WITH_CHECKED_CUT: &str = r#"
? explore finite_axis_with_checked_cut {
    from {
        vary before in range(0, 8)
        given context = ()
    }

    transition after = before + 1
    find upper_half = matches of before >= 4
}
"#;

    #[test]
    fn candidate_cuts_leave_the_complete_interval_cover_as_open_residual_work() {
        let mut lexer = Lexer::new(FINITE_AXIS_WITH_CHECKED_CUT);
        let statements = Parser::new(lexer.tokenize(), FINITE_AXIS_WITH_CHECKED_CUT)
            .parse_program()
            .expect("parse the finite-axis proof-strategy fixture");
        let artifacts = TypeChecker::check_with_explore_artifacts(
            &statements,
            None,
            FINITE_AXIS_WITH_CHECKED_CUT,
        );
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("join the checked finite-axis query");
        let support_plan = RelationalSupportPlanner::from_checked(&checked)
            .and_then(|planner| planner.plan())
            .expect("plan exact support for the finite-axis query");
        let inventory = RelationalProofStrategyInventory::from_checked(&checked, &support_plan)
            .expect("derive the checked proof-strategy inventory");
        let [axis] = inventory.axes() else {
            panic!("fixture must expose exactly one independent integer axis")
        };
        let plan = inventory
            .plan_axis(axis.dimension_id(), &[], None)
            .expect("plan the checked finite axis");

        let [candidate] = plan.candidates() else {
            panic!("the checked comparison must yield exactly one candidate cut")
        };
        assert_eq!(candidate.coordinate(), 4);
        assert_eq!(candidate.value_boundary(), 4);
        assert_eq!(
            candidate.priority(),
            RelationalSplitPriority::CheckedGuardBoundary
        );
        assert!(!candidate.origins().is_empty());

        let mut next_start = axis.coordinate_start();
        let mut covered = 0_u128;
        for interval in plan.intervals() {
            assert_eq!(interval.start(), next_start);
            assert!(interval.start() < interval.end_exclusive());
            covered += interval.cardinality();
            next_start = interval.end_exclusive();
        }
        assert_eq!(next_start, axis.coordinate_end_exclusive());
        assert_eq!(covered, axis.cardinality());

        let residual_intervals = plan
            .residual_materialization()
            .iter()
            .map(|residual| {
                assert_eq!(residual.cell_id(), Some(axis.cell().id()));
                assert_eq!(residual.dimension_id(), Some(axis.dimension_id()));
                assert_eq!(
                    residual.reason(),
                    &RelationalStrategyResidualReason::IntervalCertificateNotAccepted
                );
                residual
                    .coordinate_interval()
                    .expect("every unproved interval remains concrete fallback work")
            })
            .collect::<Vec<_>>();
        let planned_intervals = plan
            .intervals()
            .iter()
            .map(|interval| (interval.start(), interval.end_exclusive()))
            .collect::<Vec<_>>();
        assert_eq!(residual_intervals, planned_intervals);

        assert!(!plan.establishes_complement_closure());
    }
}
