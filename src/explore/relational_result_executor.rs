//! Checked relational result-view lowering and concrete singleton execution.
//!
//! The executor is an adapter between canonical result IR and the pure
//! [`ResultViewBuilder`]. Concrete cases/incidences contribute only row-local
//! evidence. The reducer closes groups, aggregates, `having`, and the candidate
//! population before calling back into this module for the public `select`
//! projection and choice objectives. This keeps aggregate-dependent
//! expressions out of per-row ingestion without inventing a second evaluator.
//!
//! A concrete row is a certified singleton cell. A future symbolic-cell path
//! needs explicit uniformity, cardinality, and distinct-count evidence; it must
//! not call these entry points with a representative `RelationalCaseId`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::choice_relation::{ChoiceCandidate, ChoiceMember};
use super::mechanism_incidence::MechanismSignatureId;
use super::relation::{ChoiceId, RelationalCaseRef, SourceKey, SourceRow, ViewId};
use super::relational_ir::{
    ExploreAggregateReducerIr, ExploreResultChoiceIr, ExploreResultFieldIr, ExploreResultGrainIr,
    ExploreResultHavingIr, ExploreResultInputIr, ExploreResultViewIr,
};
use super::result_evidence::RelationalResultEvidenceRecord;
use super::result_view::{
    close_exact_certified_groups, close_exact_certified_single_group,
    close_exact_grouped_without_choice_from_borrowed, CertifiedResultGroupSummary,
    CertifiedResultInputRoot, ClosedResultView, CompactClosedResultView,
    EvaluatedResultContribution, MechanismIncidenceRowId, ResultClosedGroupRef, ResultClosedRowRef,
    ResultOutputRow, ResultValue, ResultViewBuilder, ResultViewError, ResultViewFinishError,
    ResultViewGrain, ResultViewHaving, ResultViewInputKind, ResultViewInputRowId,
    ResultViewProjectionError, ResultViewProjector, ResultViewSnapshot, ResultViewSpec,
};
use super::structural_mechanism::{ExecutionProfileId, StructuralMechanismId};
use super::transition::TransitionId;
use super::ExploreValue;
use crate::{Expr, ExprKind, Ty};

/// One exact runtime binding. Later bindings shadow earlier bindings with the
/// same name, matching sequential result aliases such as `select [cases]`
/// where the selected field intentionally reuses an aggregate's name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultBinding {
    name: Box<str>,
    value: ResultValue,
}

impl RelationalResultBinding {
    fn new(name: impl Into<Box<str>>, value: impl Into<ResultValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn value(&self) -> &ResultValue {
        &self.value
    }
}

/// Small checked evaluator boundary shared by row-local and group-closed
/// result expressions. Semantic IDs remain typed [`ResultValue`] variants.
/// Implementations must execute the already-checked deterministic expression
/// semantics: the same expression, expected type, and ordered bindings must
/// return the same value and must not perform an external effect.
pub(crate) trait RelationalResultExpressionRuntime {
    fn evaluate(
        &mut self,
        expression: &Expr,
        expected_ty: &Ty,
        bindings: &[RelationalResultBinding],
    ) -> Result<ResultValue, String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectionStage {
    RowLocal,
    GroupClosed,
}

#[derive(Clone, Copy)]
struct ObjectiveIrRef<'a> {
    expression: &'a Expr,
    ty: &'a Ty,
}

/// Lowered result descriptor plus its checked staging plan.
pub(crate) struct RelationalResultExecutor<'ir> {
    view: &'ir ExploreResultViewIr,
    spec: ResultViewSpec,
    select_stages: Box<[ProjectionStage]>,
    objectives: Box<[ObjectiveIrRef<'ir>]>,
    objective_stages: Box<[ProjectionStage]>,
    objective_select_prefix: usize,
}

impl<'ir> RelationalResultExecutor<'ir> {
    /// Lower only already-resolved IR. Names and declaration positions remain
    /// exactly as emitted by elaboration; this layer never resolves source
    /// syntax again.
    pub(crate) fn lower(
        view_id: ViewId,
        view: &'ir ExploreResultViewIr,
    ) -> Result<Self, RelationalResultExecutorError> {
        let is_choice_display = view.choose.is_some();
        if is_choice_display && !view.aggregates.is_empty() {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice-backed display aggregation is not supported until the closed Choice relation carries the required aggregate evidence"
                    .into(),
            ));
        }
        let input_kind = match &view.input {
            ExploreResultInputIr::Sources => ResultViewInputKind::Source,
            ExploreResultInputIr::Find { .. } => ResultViewInputKind::Case,
            ExploreResultInputIr::MechanismIncidence { .. } => ResultViewInputKind::Incidence,
        };
        // In the transitional nested spelling, GROUP/MEASURE/HAVING/CHOOSE
        // lower to the independently journaled Choice relation.  Its display
        // is a row-preserving projection over the closed members, not another
        // grouped reducer over the FIND candidates.
        let grain = if is_choice_display {
            ResultViewGrain::EachCase
        } else {
            match &view.grain {
                ExploreResultGrainIr::EachCase { .. } => ResultViewGrain::EachCase,
                ExploreResultGrainIr::EachIncidence { .. } => ResultViewGrain::EachIncidence,
                ExploreResultGrainIr::GroupAll { .. } => ResultViewGrain::GroupAll,
                ExploreResultGrainIr::GroupBy { fields, .. } => ResultViewGrain::GroupBy {
                    field_names: field_names(fields),
                },
            }
        };
        let having = match &view.having {
            None => None,
            Some(ExploreResultHavingIr::Varies {
                measure_name,
                measure_index,
                ..
            }) => {
                let Some(measure) = view.measures.get(*measure_index) else {
                    return Err(RelationalResultExecutorError::InvalidView(format!(
                        "having measure index {measure_index} is absent"
                    )));
                };
                if measure.name != *measure_name {
                    return Err(RelationalResultExecutorError::InvalidView(format!(
                        "having measure `{measure_name}` resolves to `{}` at index {measure_index}",
                        measure.name
                    )));
                }
                Some(ResultViewHaving::Varies {
                    measure_index: *measure_index,
                })
            }
        };
        let spec = ResultViewSpec::new(
            view_id,
            input_kind,
            grain,
            if is_choice_display {
                Box::new([])
            } else {
                field_names(&view.measures)
            },
            if is_choice_display {
                Box::new([])
            } else {
                view.aggregates
                    .iter()
                    .map(|field| Box::<str>::from(field.name.as_str()))
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            },
            field_names(&view.select),
            if is_choice_display { None } else { having },
            None,
        )
        .map_err(RelationalResultExecutorError::Reducer)?;

        let aggregate_names = view
            .aggregates
            .iter()
            .map(|field| field.name.as_str())
            .collect::<BTreeSet<_>>();
        for aggregate in view.aggregates.iter() {
            let ExploreAggregateReducerIr::CountDistinct { value, .. } = &aggregate.reducer;
            if expression_mentions_any(value, &aggregate_names) {
                return Err(RelationalResultExecutorError::InvalidView(format!(
                    "count_distinct aggregate `{}` is not row-local",
                    aggregate.name
                )));
            }
            if aggregate.ty.to_string() != "Int" {
                return Err(RelationalResultExecutorError::InvalidView(format!(
                    "count_distinct aggregate `{}` has non-Int output type {}",
                    aggregate.name, aggregate.ty
                )));
            }
        }

        let grouped_without_choice = spec.grain().is_grouped() && spec.choice().is_none();
        let mut closed_names = aggregate_names;
        let mut select_stages = Vec::with_capacity(view.select.len());
        for field in view.select.iter() {
            let stage =
                if grouped_without_choice || expression_mentions_any(&field.value, &closed_names) {
                    ProjectionStage::GroupClosed
                } else {
                    ProjectionStage::RowLocal
                };
            if matches!(stage, ProjectionStage::GroupClosed) {
                closed_names.insert(field.name.as_str());
            }
            select_stages.push(stage);
        }

        let objectives = objective_refs(view.choose.as_ref());
        let objective_select_prefix = objective_select_prefix(&objectives, &view.select);
        let mut objective_stages = Vec::with_capacity(objectives.len());
        for objective in objectives.iter() {
            if objective.ty.to_string() != "Int" {
                return Err(RelationalResultExecutorError::InvalidView(format!(
                    "choice objective has non-Int type {}",
                    objective.ty
                )));
            }
            objective_stages.push(
                if expression_mentions_any(objective.expression, &closed_names) {
                    ProjectionStage::GroupClosed
                } else {
                    ProjectionStage::RowLocal
                },
            );
        }

        Ok(Self {
            view,
            spec,
            select_stages: select_stages.into_boxed_slice(),
            objectives,
            objective_stages: objective_stages.into_boxed_slice(),
            objective_select_prefix,
        })
    }

    pub(crate) const fn spec(&self) -> &ResultViewSpec {
        &self.spec
    }

    /// No reduction, membership choice or deferred SELECT evaluation can
    /// change one row's projection in this fragment.
    pub(crate) fn supports_row_local_case_projection(&self) -> bool {
        matches!(self.view.input, ExploreResultInputIr::Find { .. })
            && matches!(self.view.grain, ExploreResultGrainIr::EachCase { .. })
            && self.view.choose.is_none()
            && self.view.having.is_none()
            && self.view.aggregates.is_empty()
            && self.objectives.is_empty()
            && self
                .select_stages
                .iter()
                .all(|stage| *stage == ProjectionStage::RowLocal)
    }

    /// Derive projection DATA from a row record. This does not attest that its
    /// measures are correct. The driver checks every new row against a live
    /// evaluation (or its own bounded warm receipt) before journal admission.
    pub(crate) fn row_local_case_projection(
        &self,
        record: &RelationalResultEvidenceRecord,
    ) -> Option<ResultOutputRow> {
        if !self.supports_row_local_case_projection()
            || record.view_id() != self.spec.view_id()
            || !matches!(record.row_id(), ResultViewInputRowId::Case(_))
            || !record.grain_values().is_empty()
            || record.measures().len() != self.view.measures.len()
            || !record.distinct_arguments().is_empty()
            || record.early_select_len() != self.view.select.len()
            || record.early_objectives_len() != 0
        {
            return None;
        }
        let values = record
            .early_select_iter()
            .map(|value| value.cloned())
            .collect::<Option<Vec<_>>>()?;
        Some(ResultOutputRow::from_projected_parts(
            record.row_id(),
            values.into_boxed_slice(),
        ))
    }

    pub(crate) fn execution(&self) -> RelationalResultExecution<'_, 'ir> {
        RelationalResultExecution {
            executor: self,
            reducer: ResultViewBuilder::new(self.spec.clone()),
            row_states: BTreeMap::new(),
        }
    }

    /// Close a grouped result without choice over already verified durable
    /// contributions. Group projection depends only on exact key/aggregate
    /// state, so no per-row base-binding map is retained.
    pub(crate) fn close_grouped_without_choice_from_borrowed<
        R: RelationalResultExpressionRuntime,
    >(
        &self,
        contributions: &[&EvaluatedResultContribution],
        runtime: &mut R,
    ) -> Result<CompactClosedResultView, RelationalResultExecutorError> {
        let row_states = BTreeMap::new();
        let mut projector = RelationalClosedProjector {
            executor: self,
            row_states: &row_states,
            runtime,
        };
        close_exact_grouped_without_choice_from_borrowed(&self.spec, contributions, &mut projector)
            .map_err(RelationalResultExecutorError::Finish)
    }

    /// Project one exact group backed by a proof-certified source population.
    /// No representative row enters the reducer and no synthetic SourceKey is
    /// retained as population evidence.
    pub(crate) fn close_certified_single_source_group<R: RelationalResultExpressionRuntime>(
        &self,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        group_values: &[ResultValue],
        runtime: &mut R,
    ) -> Result<CompactClosedResultView, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Source {
            return Err(RelationalResultExecutorError::WrongConcreteInput {
                expected: self.spec.input_kind(),
                actual: ResultViewInputKind::Source,
            });
        }
        let row_states = BTreeMap::new();
        let mut projector = RelationalClosedProjector {
            executor: self,
            row_states: &row_states,
            runtime,
        };
        close_exact_certified_single_group(
            &self.spec,
            certified_input_root,
            exact_input_count,
            group_values,
            &mut projector,
        )
        .map_err(RelationalResultExecutorError::Finish)
    }

    /// Project exact groups backed by one proof-certified source population.
    /// No representative SourceKey becomes reducer or publication evidence.
    pub(crate) fn close_certified_source_groups<R: RelationalResultExpressionRuntime>(
        &self,
        certified_input_root: CertifiedResultInputRoot,
        exact_input_count: u128,
        groups: &[CertifiedResultGroupSummary],
        runtime: &mut R,
    ) -> Result<CompactClosedResultView, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Source {
            return Err(RelationalResultExecutorError::WrongConcreteInput {
                expected: self.spec.input_kind(),
                actual: ResultViewInputKind::Source,
            });
        }
        let row_states = BTreeMap::new();
        let mut projector = RelationalClosedProjector {
            executor: self,
            row_states: &row_states,
            runtime,
        };
        close_exact_certified_groups(
            &self.spec,
            certified_input_root,
            exact_input_count,
            groups,
            &mut projector,
        )
        .map_err(RelationalResultExecutorError::Finish)
    }

    pub(crate) fn evaluate_concrete_case<R: RelationalResultExpressionRuntime>(
        &self,
        case: RelationalCaseRef<'_>,
        runtime: &mut R,
    ) -> Result<RelationalResultEvidence, RelationalResultExecutorError> {
        if self.view.choose.is_some() {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice-backed displays must be evaluated from closed Choice members".into(),
            ));
        }
        if self.spec.input_kind() != ResultViewInputKind::Case {
            return Err(RelationalResultExecutorError::WrongConcreteInput {
                expected: self.spec.input_kind(),
                actual: ResultViewInputKind::Case,
            });
        }
        let row_id = ResultViewInputRowId::Case(case.case_id());
        self.evaluate_concrete(concrete_case_bindings(case, row_id, None), row_id, runtime)
    }

    /// Evaluate only semantic choice inputs. Public SELECT fields and
    /// aggregates are intentionally unreachable from this path, so their
    /// failure cannot prevent candidate accumulation or membership closure.
    pub(crate) fn evaluate_choice_candidate<R: RelationalResultExpressionRuntime>(
        &self,
        choice_id: ChoiceId,
        case: RelationalCaseRef<'_>,
        runtime: &mut R,
    ) -> Result<ChoiceCandidate, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Case || self.view.choose.is_none() {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice candidate evaluation requires a chosen case-input view".into(),
            ));
        }
        if self
            .objective_stages
            .iter()
            .any(|stage| !matches!(stage, ProjectionStage::RowLocal))
        {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice membership cannot depend on group-closed display state".into(),
            ));
        }

        let mut bindings =
            concrete_case_bindings(case, ResultViewInputRowId::Case(case.case_id()), None);
        let mut partition_values = Vec::new();
        for field in group_fields(&self.view.grain) {
            let value = evaluate_field(runtime, field, &bindings, "choice partition")?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            partition_values.push(value);
        }
        let mut measures = Vec::with_capacity(self.view.measures.len());
        for field in self.view.measures.iter() {
            let value = evaluate_field(runtime, field, &bindings, "choice measure")?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            measures.push(value);
        }
        let objectives = self
            .objectives
            .iter()
            .map(|objective| evaluate_objective(runtime, *objective, &bindings))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ChoiceCandidate::new(
            choice_id,
            case.case_id(),
            partition_values,
            measures,
            objectives,
        ))
    }

    /// Project one canonical member of an already closed Choice relation.
    /// Partition keys and measures are consumed from the authenticated member
    /// payload; neither the FIND population nor the choice policy is
    /// evaluated on this display path.
    pub(crate) fn evaluate_choice_member<R: RelationalResultExpressionRuntime>(
        &self,
        choice_id: ChoiceId,
        member: &ChoiceMember,
        case: RelationalCaseRef<'_>,
        runtime: &mut R,
    ) -> Result<RelationalResultEvidence, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Case
            || self.view.choose.is_none()
            || self.spec.grain() != &ResultViewGrain::EachCase
            || self.spec.choice().is_some()
            || self.spec.having().is_some()
            || !self.spec.measure_names().is_empty()
            || !self.spec.aggregate_names().is_empty()
        {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice-member projection requires a row-preserving display spec".into(),
            ));
        }
        let candidate = member.candidate();
        if candidate.choice_id() != choice_id || candidate.case_id() != case.case_id() {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice member does not match its display input".into(),
            ));
        }
        if candidate.partition_values().len() != group_fields(&self.view.grain).len()
            || candidate.measures().len() != self.view.measures.len()
            || candidate.objectives().len() != self.objectives.len()
        {
            return Err(RelationalResultExecutorError::InvalidView(
                "choice member payload does not match the checked choice schema".into(),
            ));
        }

        let row_id = ResultViewInputRowId::Case(case.case_id());
        let mut member_bindings = concrete_case_bindings(case, row_id, None);
        for (field, value) in group_fields(&self.view.grain)
            .iter()
            .zip(candidate.partition_values())
        {
            member_bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
        }
        for (field, value) in self.view.measures.iter().zip(candidate.measures()) {
            member_bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
        }

        let mut bindings = member_bindings.clone();
        let mut early_select = Vec::with_capacity(self.view.select.len());
        for (field, stage) in self.view.select.iter().zip(self.select_stages.iter()) {
            if !matches!(stage, ProjectionStage::RowLocal) {
                return Err(RelationalResultExecutorError::InvalidView(
                    "choice-backed display SELECT requires unavailable group-closed evidence"
                        .into(),
                ));
            }
            let value = evaluate_field(runtime, field, &bindings, "choice display selected field")?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            early_select.push(Some(value));
        }

        Ok(RelationalResultEvidence {
            contribution: EvaluatedResultContribution::new(
                self.spec.view_id(),
                row_id,
                Box::<[ResultValue]>::default(),
                Box::<[ResultValue]>::default(),
                Box::<[ResultValue]>::default(),
            ),
            state: RelationalResultRowState {
                row_id,
                base_bindings: member_bindings.into_boxed_slice(),
                early_select: early_select.into_boxed_slice(),
                early_objectives: candidate
                    .objectives()
                    .iter()
                    .copied()
                    .map(Some)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        })
    }

    pub(crate) fn evaluate_concrete_source<R: RelationalResultExpressionRuntime>(
        &self,
        source_key: SourceKey,
        source: &SourceRow,
        runtime: &mut R,
    ) -> Result<RelationalResultEvidence, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Source {
            return Err(RelationalResultExecutorError::WrongConcreteInput {
                expected: self.spec.input_kind(),
                actual: ResultViewInputKind::Source,
            });
        }
        let row_id = ResultViewInputRowId::Source(source_key);
        self.evaluate_concrete(concrete_source_bindings(source), row_id, runtime)
    }

    pub(crate) fn evaluate_concrete_incidence<R: RelationalResultExpressionRuntime>(
        &self,
        case: RelationalCaseRef<'_>,
        transition_id: TransitionId,
        signature_id: MechanismSignatureId,
        structural_mechanism_id: StructuralMechanismId,
        execution_profile_id: ExecutionProfileId,
        runtime: &mut R,
    ) -> Result<RelationalResultEvidence, RelationalResultExecutorError> {
        if self.spec.input_kind() != ResultViewInputKind::Incidence {
            return Err(RelationalResultExecutorError::WrongConcreteInput {
                expected: self.spec.input_kind(),
                actual: ResultViewInputKind::Incidence,
            });
        }
        let incidence = MechanismIncidenceRowId::new(case.case_id(), transition_id, signature_id);
        let row_id = ResultViewInputRowId::Incidence(incidence);
        self.evaluate_concrete(
            concrete_case_bindings(
                case,
                row_id,
                Some((structural_mechanism_id, execution_profile_id)),
            ),
            row_id,
            runtime,
        )
    }

    fn evaluate_concrete<R: RelationalResultExpressionRuntime>(
        &self,
        base_bindings: Vec<RelationalResultBinding>,
        row_id: ResultViewInputRowId,
        runtime: &mut R,
    ) -> Result<RelationalResultEvidence, RelationalResultExecutorError> {
        let mut bindings = base_bindings.clone();

        let mut group_values = Vec::new();
        for field in group_fields(&self.view.grain) {
            let value = evaluate_field(runtime, field, &bindings, "group field")?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            group_values.push(value);
        }

        let mut measures = Vec::with_capacity(self.view.measures.len());
        for field in self.view.measures.iter() {
            let value = evaluate_field(runtime, field, &bindings, "measure")?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            measures.push(value);
        }

        let mut distinct_arguments = Vec::with_capacity(self.view.aggregates.len());
        for aggregate in self.view.aggregates.iter() {
            let ExploreAggregateReducerIr::CountDistinct {
                value, value_ty, ..
            } = &aggregate.reducer;
            let argument = runtime
                .evaluate(value, value_ty, &bindings)
                .map_err(|message| RelationalResultExecutorError::Evaluation {
                    phase: "count_distinct argument",
                    field: aggregate.name.clone().into_boxed_str(),
                    message: message.into_boxed_str(),
                })?;
            distinct_arguments.push(argument);
        }

        let mut early_select = Vec::with_capacity(self.view.select.len());
        for (field, stage) in self.view.select.iter().zip(self.select_stages.iter()) {
            if matches!(stage, ProjectionStage::RowLocal) {
                let value = evaluate_field(runtime, field, &bindings, "selected field")?;
                bindings.push(RelationalResultBinding::new(
                    field.name.as_str(),
                    value.clone(),
                ));
                early_select.push(Some(value));
            } else {
                early_select.push(None);
            }
        }

        let mut early_objectives = Vec::with_capacity(self.objectives.len());
        for (objective, stage) in self.objectives.iter().zip(self.objective_stages.iter()) {
            if matches!(stage, ProjectionStage::RowLocal) {
                early_objectives.push(Some(evaluate_objective(runtime, *objective, &bindings)?));
            } else {
                early_objectives.push(None);
            }
        }

        Ok(RelationalResultEvidence {
            contribution: EvaluatedResultContribution::new(
                self.spec.view_id(),
                row_id,
                group_values,
                measures,
                distinct_arguments,
            ),
            state: RelationalResultRowState {
                row_id,
                base_bindings: base_bindings.into_boxed_slice(),
                early_select: early_select.into_boxed_slice(),
                early_objectives: early_objectives.into_boxed_slice(),
            },
        })
    }
}

/// Atomic pair of reducer evidence and the exact singleton bindings needed by
/// the post-group expression stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalResultEvidence {
    contribution: EvaluatedResultContribution,
    state: RelationalResultRowState,
}

impl RelationalResultEvidence {
    pub(crate) const fn row_id(&self) -> ResultViewInputRowId {
        self.state.row_id
    }

    pub(crate) const fn contribution(&self) -> &EvaluatedResultContribution {
        &self.contribution
    }

    /// Cached row-local SELECT values in declaration order. `None` marks a
    /// projection that must wait for group closure; the durable evidence layer
    /// retains these values without copying the relation-owned base bindings.
    pub(crate) fn early_select(&self) -> &[Option<ResultValue>] {
        &self.state.early_select
    }

    /// Cached row-local choice objectives in canonical objective order. `None`
    /// marks an objective that must be evaluated against a closed group.
    pub(crate) fn early_objectives(&self) -> &[Option<i64>] {
        &self.state.early_objectives
    }

    /// After deterministic re-evaluation has been compared with its durable
    /// record, reuse that record's canonical value backing for the reducer
    /// rebuild. The row-local base bindings stay freshly derived from the
    /// relation; only already-verified value storage is replaced.
    pub(crate) fn reuse_verified_durable_value_storage(
        &mut self,
        contribution: &EvaluatedResultContribution,
        early_select: Box<[Option<ResultValue>]>,
    ) {
        debug_assert_eq!(&self.contribution, contribution);
        debug_assert_eq!(self.early_select(), early_select.as_ref());
        self.contribution = contribution.clone();
        self.state.early_select = early_select;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RelationalResultRowState {
    row_id: ResultViewInputRowId,
    base_bindings: Box<[RelationalResultBinding]>,
    early_select: Box<[Option<ResultValue>]>,
    early_objectives: Box<[Option<i64>]>,
}

/// Set-semantic incremental execution for one lowered result view.
pub(crate) struct RelationalResultExecution<'executor, 'ir> {
    executor: &'executor RelationalResultExecutor<'ir>,
    reducer: ResultViewBuilder,
    row_states: BTreeMap<ResultViewInputRowId, RelationalResultRowState>,
}

impl<'executor, 'ir> RelationalResultExecution<'executor, 'ir> {
    /// Preflight both maps before mutation. Once the reducer insertion
    /// succeeds, installing the already-validated row state has no fallible
    /// semantic operation after it.
    pub(crate) fn insert(
        &mut self,
        evidence: RelationalResultEvidence,
    ) -> Result<bool, RelationalResultExecutorError> {
        let row_id = evidence.row_id();
        for (component, expected, actual) in [
            (
                "staged selected fields",
                self.executor.select_stages.len(),
                evidence.state.early_select.len(),
            ),
            (
                "staged choice objectives",
                self.executor.objective_stages.len(),
                evidence.state.early_objectives.len(),
            ),
        ] {
            if expected != actual {
                return Err(RelationalResultExecutorError::ClosedShape {
                    component,
                    expected,
                    actual,
                });
            }
        }
        let existing_contribution = self.reducer.contribution(row_id);
        let existing_state = self.row_states.get(&row_id);
        match (existing_contribution, existing_state) {
            (Some(contribution), Some(state)) => {
                return if contribution == &evidence.contribution && state == &evidence.state {
                    Ok(false)
                } else {
                    Err(RelationalResultExecutorError::EvidenceConflict { row_id })
                };
            }
            (None, None) => {}
            _ => return Err(RelationalResultExecutorError::ReducerStateDiverged { row_id }),
        }

        let inserted = self
            .reducer
            .insert(evidence.contribution)
            .map_err(RelationalResultExecutorError::Reducer)?;
        if !inserted {
            return Err(RelationalResultExecutorError::ReducerStateDiverged { row_id });
        }
        let replaced = self.row_states.insert(row_id, evidence.state);
        debug_assert!(replaced.is_none());
        Ok(true)
    }

    pub(crate) fn seal_input(&mut self) -> bool {
        self.reducer.seal_input()
    }

    pub(crate) fn snapshot<R: RelationalResultExpressionRuntime>(
        &self,
        runtime: &mut R,
    ) -> Result<ResultViewSnapshot, RelationalResultExecutorError> {
        let mut projector = RelationalClosedProjector {
            executor: self.executor,
            row_states: &self.row_states,
            runtime,
        };
        self.reducer
            .snapshot(&mut projector)
            .map_err(RelationalResultExecutorError::Projection)
    }

    pub(crate) fn finish<R: RelationalResultExpressionRuntime>(
        &self,
        runtime: &mut R,
    ) -> Result<ClosedResultView, RelationalResultExecutorError> {
        let mut projector = RelationalClosedProjector {
            executor: self.executor,
            row_states: &self.row_states,
            runtime,
        };
        self.reducer
            .finish(&mut projector)
            .map_err(RelationalResultExecutorError::Finish)
    }
}

struct RelationalClosedProjector<'a, 'ir, R> {
    executor: &'a RelationalResultExecutor<'ir>,
    row_states: &'a BTreeMap<ResultViewInputRowId, RelationalResultRowState>,
    runtime: &'a mut R,
}

impl<R: RelationalResultExpressionRuntime> ResultViewProjector
    for RelationalClosedProjector<'_, '_, R>
{
    fn project_group(
        &mut self,
        group: ResultClosedGroupRef<'_>,
    ) -> Result<Box<[ResultValue]>, ResultViewProjectionError> {
        let mut bindings = self
            .executor
            .group_bindings(group)
            .map_err(projector_error)?;
        let mut values = Vec::with_capacity(self.executor.view.select.len());
        for field in self.executor.view.select.iter() {
            let value = evaluate_field(self.runtime, field, &bindings, "closed selected field")
                .map_err(projector_error)?;
            bindings.push(RelationalResultBinding::new(
                field.name.as_str(),
                value.clone(),
            ));
            values.push(value);
        }
        Ok(values.into_boxed_slice())
    }

    fn evaluate_objectives(
        &mut self,
        row: ResultClosedRowRef<'_>,
    ) -> Result<Box<[i64]>, ResultViewProjectionError> {
        let contribution = row.contribution();
        let row_id = contribution.row_id();
        let state = self.row_states.get(&row_id).ok_or_else(|| {
            ResultViewProjectionError::evaluation(format!(
                "result projection has no singleton bindings for row {row_id:?}"
            ))
        })?;
        if self
            .executor
            .objective_stages
            .iter()
            .all(|stage| matches!(stage, ProjectionStage::RowLocal))
        {
            return collect_early_objectives(state);
        }

        let bindings = closed_row_bindings(self.executor, contribution, row.group(), state)
            .map_err(projector_error)?;
        let (bindings, _) = evaluate_select_prefix(
            self.runtime,
            self.executor,
            state,
            bindings,
            self.executor.objective_select_prefix,
        )?;

        let mut objectives = Vec::with_capacity(self.executor.objectives.len());
        for ((objective, stage), early) in self
            .executor
            .objectives
            .iter()
            .zip(self.executor.objective_stages.iter())
            .zip(state.early_objectives.iter())
        {
            let value = match (stage, early) {
                (ProjectionStage::RowLocal, Some(value)) => *value,
                (ProjectionStage::GroupClosed, None) => {
                    evaluate_objective(self.runtime, *objective, &bindings)
                        .map_err(projector_error)?
                }
                _ => {
                    return Err(ResultViewProjectionError::evaluation(
                        "result objective staging mismatch",
                    ));
                }
            };
            objectives.push(value);
        }

        Ok(objectives.into_boxed_slice())
    }

    fn project_row(
        &mut self,
        row: ResultClosedRowRef<'_>,
    ) -> Result<Box<[ResultValue]>, ResultViewProjectionError> {
        let contribution = row.contribution();
        let row_id = contribution.row_id();
        let state = self.row_states.get(&row_id).ok_or_else(|| {
            ResultViewProjectionError::evaluation(format!(
                "result projection has no singleton bindings for row {row_id:?}"
            ))
        })?;
        let bindings = closed_row_bindings(self.executor, contribution, row.group(), state)
            .map_err(projector_error)?;
        let (_, values) = evaluate_select_prefix(
            self.runtime,
            self.executor,
            state,
            bindings,
            self.executor.view.select.len(),
        )?;

        Ok(values)
    }
}

fn collect_early_objectives(
    state: &RelationalResultRowState,
) -> Result<Box<[i64]>, ResultViewProjectionError> {
    state
        .early_objectives
        .iter()
        .map(|value| {
            (*value).ok_or_else(|| {
                ResultViewProjectionError::evaluation("result objective staging mismatch")
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn closed_row_bindings(
    executor: &RelationalResultExecutor<'_>,
    contribution: &EvaluatedResultContribution,
    group: Option<ResultClosedGroupRef<'_>>,
    state: &RelationalResultRowState,
) -> Result<Vec<RelationalResultBinding>, RelationalResultExecutorError> {
    if state.row_id != contribution.row_id() {
        return Err(RelationalResultExecutorError::ReducerStateDiverged {
            row_id: contribution.row_id(),
        });
    }
    let mut bindings = state.base_bindings.to_vec();
    append_intermediate_bindings(
        &mut bindings,
        executor.spec.grain().group_field_names(),
        contribution.group_values(),
    )?;
    append_intermediate_bindings(
        &mut bindings,
        executor.spec.measure_names(),
        contribution.measures(),
    )?;
    if let Some(group) = group {
        executor.append_aggregate_bindings(&mut bindings, group)?;
    }
    Ok(bindings)
}

fn evaluate_select_prefix(
    runtime: &mut impl RelationalResultExpressionRuntime,
    executor: &RelationalResultExecutor<'_>,
    state: &RelationalResultRowState,
    mut bindings: Vec<RelationalResultBinding>,
    prefix: usize,
) -> Result<(Vec<RelationalResultBinding>, Box<[ResultValue]>), ResultViewProjectionError> {
    if prefix > executor.view.select.len() {
        return Err(ResultViewProjectionError::evaluation(
            "result projection prefix exceeds SELECT arity",
        ));
    }
    let mut values = Vec::with_capacity(prefix);
    for ((field, stage), early) in executor
        .view
        .select
        .iter()
        .zip(executor.select_stages.iter())
        .zip(state.early_select.iter())
        .take(prefix)
    {
        let value = match (stage, early) {
            (ProjectionStage::RowLocal, Some(value)) => value.clone(),
            (ProjectionStage::GroupClosed, None) => {
                evaluate_field(runtime, field, &bindings, "closed selected field")
                    .map_err(projector_error)?
            }
            _ => {
                return Err(ResultViewProjectionError::evaluation(format!(
                    "result projection staging mismatch for `{}`",
                    field.name
                )));
            }
        };
        bindings.push(RelationalResultBinding::new(
            field.name.as_str(),
            value.clone(),
        ));
        values.push(value);
    }
    Ok((bindings, values.into_boxed_slice()))
}

impl RelationalResultExecutor<'_> {
    fn group_bindings(
        &self,
        group: ResultClosedGroupRef<'_>,
    ) -> Result<Vec<RelationalResultBinding>, RelationalResultExecutorError> {
        let mut bindings = Vec::new();
        append_intermediate_bindings(
            &mut bindings,
            self.spec.grain().group_field_names(),
            group.key().values(),
        )?;
        self.append_aggregate_bindings(&mut bindings, group)?;
        Ok(bindings)
    }

    fn append_aggregate_bindings(
        &self,
        bindings: &mut Vec<RelationalResultBinding>,
        group: ResultClosedGroupRef<'_>,
    ) -> Result<(), RelationalResultExecutorError> {
        if group
            .aggregates()
            .iter()
            .any(|aggregate| aggregate.count().is_exact() != group.input_is_sealed())
        {
            return Err(RelationalResultExecutorError::InvalidView(
                "closed aggregate evidence disagrees with the input frontier".to_string(),
            ));
        }
        if group.aggregates().len() != self.view.aggregates.len() {
            return Err(RelationalResultExecutorError::ClosedShape {
                component: "aggregates",
                expected: self.view.aggregates.len(),
                actual: group.aggregates().len(),
            });
        }
        for (declared, aggregate) in self.view.aggregates.iter().zip(group.aggregates().iter()) {
            if aggregate.name() != declared.name {
                return Err(RelationalResultExecutorError::InvalidView(format!(
                    "closed aggregate `{}` occupies the position declared for `{}`",
                    aggregate.name(),
                    declared.name
                )));
            }
            let count = i64::try_from(aggregate.count().current()).map_err(|_| {
                RelationalResultExecutorError::AggregateCountOverflow {
                    aggregate: declared.name.clone().into_boxed_str(),
                    count: aggregate.count().current(),
                }
            })?;
            bindings.push(RelationalResultBinding::new(
                declared.name.as_str(),
                ExploreValue::Int(count),
            ));
        }
        Ok(())
    }
}

fn concrete_source_bindings(source: &SourceRow) -> Vec<RelationalResultBinding> {
    vec![
        RelationalResultBinding::new("context", source.context().clone()),
        RelationalResultBinding::new("before", source.before().clone()),
    ]
}

fn concrete_case_bindings(
    case: RelationalCaseRef<'_>,
    row_id: ResultViewInputRowId,
    structural_assignment: Option<(StructuralMechanismId, ExecutionProfileId)>,
) -> Vec<RelationalResultBinding> {
    let mut bindings = vec![
        RelationalResultBinding::new("context", case.context().clone()),
        RelationalResultBinding::new("before", case.before().clone()),
        RelationalResultBinding::new("after", case.after().clone()),
    ];
    match row_id {
        ResultViewInputRowId::Source(_) => {
            unreachable!("source rows use source-only base bindings")
        }
        ResultViewInputRowId::Case(case_id) => {
            bindings.push(RelationalResultBinding::new("case_id", case_id));
        }
        ResultViewInputRowId::Incidence(incidence) => {
            let (structural_mechanism_id, execution_profile_id) = structural_assignment
                .expect("mechanism-incidence rows require a durable structural assignment");
            bindings.extend([
                RelationalResultBinding::new("case_id", incidence.case_id()),
                RelationalResultBinding::new("transition_id", incidence.transition_id()),
                RelationalResultBinding::new("signature_id", incidence.signature_id()),
                RelationalResultBinding::new("structural_mechanism_id", structural_mechanism_id),
                RelationalResultBinding::new("execution_profile_id", execution_profile_id),
            ]);
        }
    }
    bindings
}

fn append_intermediate_bindings(
    bindings: &mut Vec<RelationalResultBinding>,
    names: &[Box<str>],
    values: &[ResultValue],
) -> Result<(), RelationalResultExecutorError> {
    if names.len() != values.len() {
        return Err(RelationalResultExecutorError::ClosedShape {
            component: "named intermediate values",
            expected: names.len(),
            actual: values.len(),
        });
    }
    bindings.extend(
        names
            .iter()
            .zip(values.iter())
            .map(|(name, value)| RelationalResultBinding::new(name.clone(), value.clone())),
    );
    Ok(())
}

fn evaluate_field(
    runtime: &mut impl RelationalResultExpressionRuntime,
    field: &ExploreResultFieldIr,
    bindings: &[RelationalResultBinding],
    phase: &'static str,
) -> Result<ResultValue, RelationalResultExecutorError> {
    runtime
        .evaluate(&field.value, &field.ty, bindings)
        .map_err(|message| RelationalResultExecutorError::Evaluation {
            phase,
            field: field.name.clone().into_boxed_str(),
            message: message.into_boxed_str(),
        })
}

fn evaluate_objective(
    runtime: &mut impl RelationalResultExpressionRuntime,
    objective: ObjectiveIrRef<'_>,
    bindings: &[RelationalResultBinding],
) -> Result<i64, RelationalResultExecutorError> {
    let value = runtime
        .evaluate(objective.expression, objective.ty, bindings)
        .map_err(|message| RelationalResultExecutorError::Evaluation {
            phase: "choice objective",
            field: Box::<str>::from("<objective>"),
            message: message.into_boxed_str(),
        })?;
    match value {
        ResultValue::Value(ExploreValue::Int(value)) => Ok(value),
        _ => Err(RelationalResultExecutorError::ExpectedIntObjective),
    }
}

fn projector_error(error: RelationalResultExecutorError) -> ResultViewProjectionError {
    ResultViewProjectionError::evaluation(error.to_string())
}

fn field_names(fields: &[ExploreResultFieldIr]) -> Box<[Box<str>]> {
    fields
        .iter()
        .map(|field| Box::<str>::from(field.name.as_str()))
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn group_fields(grain: &ExploreResultGrainIr) -> &[ExploreResultFieldIr] {
    match grain {
        ExploreResultGrainIr::GroupBy { fields, .. } => fields,
        ExploreResultGrainIr::EachCase { .. }
        | ExploreResultGrainIr::EachIncidence { .. }
        | ExploreResultGrainIr::GroupAll { .. } => &[],
    }
}

fn objective_refs(choice: Option<&ExploreResultChoiceIr>) -> Box<[ObjectiveIrRef<'_>]> {
    match choice {
        None => Box::new([]),
        Some(ExploreResultChoiceIr::Optimize {
            objective,
            objective_ty,
            ..
        }) => vec![ObjectiveIrRef {
            expression: objective,
            ty: objective_ty,
        }]
        .into_boxed_slice(),
        Some(ExploreResultChoiceIr::Pareto { objectives, .. }) => objectives
            .iter()
            .map(|objective| ObjectiveIrRef {
                expression: &objective.value,
                ty: &objective.ty,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn objective_select_prefix(
    objectives: &[ObjectiveIrRef<'_>],
    select: &[ExploreResultFieldIr],
) -> usize {
    let mut prefix = 0;
    for objective in objectives {
        for (index, field) in select.iter().enumerate() {
            let name = BTreeSet::from([field.name.as_str()]);
            if expression_mentions_any(objective.expression, &name) {
                prefix = prefix.max(index + 1);
            }
        }
    }
    prefix
}

/// Conservative staging classifier. Result-local names are reserved by the
/// checked IR, so an occurrence cannot be captured by a nested binder. Blocks
/// are staged late whenever a closed alias exists because their statement
/// graph is intentionally left to the ordinary evaluator.
fn expression_mentions_any(expression: &Expr, names: &BTreeSet<&str>) -> bool {
    if names.is_empty() {
        return false;
    }
    match &expression.kind {
        ExprKind::Var(name) => names.contains(name.as_str()),
        ExprKind::Lit(_) | ExprKind::Unit => false,
        ExprKind::App(function, arguments) => {
            expression_mentions_any(function, names)
                || arguments
                    .iter()
                    .any(|argument| expression_mentions_any(argument, names))
        }
        ExprKind::Lambda(_, body) | ExprKind::Try(body) => expression_mentions_any(body, names),
        ExprKind::BinOp(_, left, right)
        | ExprKind::Index(left, right)
        | ExprKind::Pipe(left, right) => {
            expression_mentions_any(left, names) || expression_mentions_any(right, names)
        }
        ExprKind::UnOp(_, value) | ExprKind::Field(value, _) => {
            expression_mentions_any(value, names)
        }
        ExprKind::If(condition, then_value, else_value) => {
            expression_mentions_any(condition, names)
                || expression_mentions_any(then_value, names)
                || expression_mentions_any(else_value, names)
        }
        ExprKind::Match(value, arms) => {
            expression_mentions_any(value, names)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expression_mentions_any(guard, names))
                        || expression_mentions_any(&arm.body, names)
                })
        }
        ExprKind::Block(_) => true,
        ExprKind::List(values)
        | ExprKind::Tuple(values)
        | ExprKind::Conjunction(values)
        | ExprKind::Disjunction(values)
        | ExprKind::Effect(_, values) => values
            .iter()
            .any(|value| expression_mentions_any(value, names)),
        ExprKind::Handle { handlers, body, .. } => {
            expression_mentions_any(body, names)
                || handlers
                    .iter()
                    .any(|handler| expression_mentions_any(&handler.body, names))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationalResultExecutorError {
    InvalidView(String),
    Reducer(ResultViewError),
    Projection(ResultViewProjectionError),
    Finish(ResultViewFinishError),
    WrongConcreteInput {
        expected: ResultViewInputKind,
        actual: ResultViewInputKind,
    },
    Evaluation {
        phase: &'static str,
        field: Box<str>,
        message: Box<str>,
    },
    ExpectedIntObjective,
    AggregateCountOverflow {
        aggregate: Box<str>,
        count: u128,
    },
    ClosedShape {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
    EvidenceConflict {
        row_id: ResultViewInputRowId,
    },
    ReducerStateDiverged {
        row_id: ResultViewInputRowId,
    },
}

impl fmt::Display for RelationalResultExecutorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidView(message) => {
                write!(formatter, "invalid relational result view: {message}")
            }
            Self::Reducer(error) => error.fmt(formatter),
            Self::Projection(error) => error.fmt(formatter),
            Self::Finish(error) => error.fmt(formatter),
            Self::WrongConcreteInput { .. } => {
                formatter.write_str("concrete result input has the wrong relation kind")
            }
            Self::Evaluation {
                phase,
                field,
                message,
            } => write!(formatter, "could not evaluate {phase} `{field}`: {message}"),
            Self::ExpectedIntObjective => {
                formatter.write_str("checked result choice objective did not evaluate to Int")
            }
            Self::AggregateCountOverflow { aggregate, count } => write!(
                formatter,
                "aggregate `{aggregate}` count {count} does not fit Futuruna Int"
            ),
            Self::ClosedShape { component, .. } => {
                write!(
                    formatter,
                    "closed result view has the wrong number of {component}"
                )
            }
            Self::EvidenceConflict { .. } => formatter
                .write_str("result input row was rediscovered with different singleton evidence"),
            Self::ReducerStateDiverged { .. } => {
                formatter.write_str("result reducer and singleton projection state diverged")
            }
        }
    }
}

impl Error for RelationalResultExecutorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::mechanism_incidence::MechanismSignatureDefinition;
    use crate::explore::relation::{
        AdmissionId, FindPolarity, MechanismRequestId, MechanismTargetId, QuestionId,
        RelationCatalogBuilder, RelationId, RelationLineageId, RelationProvenance,
        RelationSupportId, SourceRow, SuccessorRow, ViewInputId,
    };
    use crate::explore::relational_ir::{ExploreAggregateFieldIr, ExploreParetoObjectiveIr};
    use crate::explore::result_view::{ResultViewCount, ResultViewStatus};
    use crate::{ExploreChooseCardinality, ExploreOptimizeDirection, Literal, Span};
    use sha2::{Digest, Sha256};

    struct FixtureRuntime;

    impl RelationalResultExpressionRuntime for FixtureRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            bindings: &[RelationalResultBinding],
        ) -> Result<ResultValue, String> {
            match &expression.kind {
                ExprKind::Var(name) => bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.name() == name)
                    .map(|binding| binding.value().clone())
                    .ok_or_else(|| format!("unbound fixture variable `{name}`")),
                ExprKind::Lit(Literal::Int(value)) => {
                    Ok(ResultValue::Value(ExploreValue::Int(*value)))
                }
                ExprKind::Lit(Literal::Str(value)) => {
                    Ok(ResultValue::Value(ExploreValue::String(value.clone())))
                }
                ExprKind::BinOp(operator, left, right) => {
                    let left = self.evaluate(left, &int_ty(), bindings)?;
                    let right = self.evaluate(right, &int_ty(), bindings)?;
                    let (
                        ResultValue::Value(ExploreValue::Int(left)),
                        ResultValue::Value(ExploreValue::Int(right)),
                    ) = (left, right)
                    else {
                        return Err("fixture arithmetic requires Int".to_string());
                    };
                    let value = match operator.as_str() {
                        "+" => left.checked_add(right),
                        "-" => left.checked_sub(right),
                        "*" => left.checked_mul(right),
                        _ => return Err(format!("unsupported fixture operator `{operator}`")),
                    }
                    .ok_or_else(|| "fixture arithmetic overflow".to_string())?;
                    Ok(ResultValue::Value(ExploreValue::Int(value)))
                }
                _ => Err("unsupported fixture expression".to_string()),
            }
        }
    }

    fn int_ty() -> Ty {
        Ty::Name("Int".to_string())
    }

    fn string_ty() -> Ty {
        Ty::Name("String".to_string())
    }

    fn opaque_id_ty(name: &str) -> Ty {
        Ty::Name(name.to_string())
    }

    fn var(name: &str) -> Expr {
        Expr::unspanned(ExprKind::Var(name.to_string()))
    }

    fn int(value: i64) -> Expr {
        Expr::unspanned(ExprKind::Lit(Literal::Int(value)))
    }

    fn multiply(left: Expr, right: Expr) -> Expr {
        Expr::unspanned(ExprKind::BinOp(
            "*".to_string(),
            Box::new(left),
            Box::new(right),
        ))
    }

    fn field(name: &str, value: Expr, ty: Ty) -> ExploreResultFieldIr {
        ExploreResultFieldIr {
            name: name.to_string(),
            value,
            ty,
            span: Span::dummy(),
        }
    }

    fn aggregate(name: &str, value: Expr, value_ty: Ty) -> ExploreAggregateFieldIr {
        ExploreAggregateFieldIr {
            name: name.to_string(),
            reducer: ExploreAggregateReducerIr::CountDistinct { value, value_ty },
            ty: int_ty(),
            span: Span::dummy(),
        }
    }

    fn identities(name: &str) -> (RelationId, QuestionId, MechanismRequestId) {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(format!("relation-{name}").as_bytes());
        let admission_id =
            AdmissionId::from_canonical_admission_preimage(relation_id, b"supported");
        let question_id =
            QuestionId::from_canonical_find_preimage(admission_id, b"selected", FindPolarity::All);
        let request_id = MechanismRequestId::from_canonical_request_preimages(
            question_id,
            MechanismTargetId::Selected,
            b"endpoint",
            b"control",
        );
        (relation_id, question_id, request_id)
    }

    fn provenance(name: &str) -> RelationProvenance {
        RelationProvenance::new(
            [RelationLineageId::from_canonical_preimage(
                format!("lineage-{name}").as_bytes(),
            )],
            [RelationSupportId::from_canonical_preimage(
                format!("support-{name}").as_bytes(),
            )],
        )
    }

    fn insert_case(
        builder: &mut RelationCatalogBuilder,
        name: &str,
        context: ExploreValue,
        before: i64,
        after: i64,
    ) -> super::super::relation::RelationalCaseId {
        let source_key = builder
            .insert_source(SourceRow::new(
                context,
                ExploreValue::Int(before),
                provenance(&format!("source-{name}")),
            ))
            .unwrap();
        builder
            .insert_successor(
                source_key,
                SuccessorRow::new(
                    ExploreValue::Int(after),
                    provenance(&format!("successor-{name}")),
                ),
            )
            .unwrap()
            .1
    }

    fn transition(name: &str) -> TransitionId {
        TransitionId::from_bytes(Sha256::digest(format!("transition-{name}")).into())
    }

    fn structural_ids(name: &str) -> (StructuralMechanismId, ExecutionProfileId) {
        (
            StructuralMechanismId::from_journal_codec_bytes(
                Sha256::digest(format!("mechanism-{name}")).into(),
            ),
            ExecutionProfileId::from_journal_codec_bytes(
                Sha256::digest(format!("profile-{name}")).into(),
            ),
        )
    }

    #[test]
    fn two_raw_signatures_quotient_to_one_structural_mechanism_count() {
        let (relation_id, _, request_id) = identities("shared-signature-bin");
        let mut catalog = RelationCatalogBuilder::new(relation_id);
        let carl = insert_case(
            &mut catalog,
            "Carl",
            ExploreValue::String("Carl".to_string()),
            199_999,
            200_000,
        );
        let john = insert_case(
            &mut catalog,
            "John",
            ExploreValue::String("John".to_string()),
            9_999,
            10_000,
        );
        let catalog = catalog.snapshot();
        let view = ExploreResultViewIr {
            node_index: 1,
            name: "mechanisms_per_bin".to_string(),
            input: ExploreResultInputIr::MechanismIncidence {
                request_node_index: 0,
            },
            grain: ExploreResultGrainIr::GroupBy {
                fields: vec![field("bin_start_ore", int(5_000), int_ty())].into_boxed_slice(),
                span: Span::dummy(),
            },
            measures: Box::new([]),
            aggregates: vec![
                aggregate(
                    "mechanisms",
                    var("structural_mechanism_id"),
                    opaque_id_ty("StructuralMechanismId"),
                ),
                aggregate(
                    "raw_signatures",
                    var("signature_id"),
                    opaque_id_ty("MechanismSignatureId"),
                ),
                aggregate("cases", var("case_id"), opaque_id_ty("CaseId")),
            ]
            .into_boxed_slice(),
            having: None,
            select: vec![
                field("mechanism_count", var("mechanisms"), int_ty()),
                field("raw_signature_count", var("raw_signatures"), int_ty()),
                field("twice_the_cases", multiply(var("cases"), int(2)), int_ty()),
            ]
            .into_boxed_slice(),
            choose: None,
            span: Span::dummy(),
        };
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::MechanismIncidence(request_id),
            b"mechanisms-per-bin",
        );
        let executor = RelationalResultExecutor::lower(view_id, &view).unwrap();
        let carl_signature = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"carl-differential-signature".as_slice(),
        )
        .id();
        let john_signature = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"john-differential-signature".as_slice(),
        )
        .id();
        let (structural_mechanism_id, execution_profile_id) = structural_ids("shared");
        let mut runtime = FixtureRuntime;
        let mut execution = executor.execution();
        execution
            .insert(
                executor
                    .evaluate_concrete_incidence(
                        catalog.case(carl).unwrap(),
                        transition("Carl"),
                        carl_signature,
                        structural_mechanism_id,
                        execution_profile_id,
                        &mut runtime,
                    )
                    .unwrap(),
            )
            .unwrap();
        execution
            .insert(
                executor
                    .evaluate_concrete_incidence(
                        catalog.case(john).unwrap(),
                        transition("John"),
                        john_signature,
                        structural_mechanism_id,
                        execution_profile_id,
                        &mut runtime,
                    )
                    .unwrap(),
            )
            .unwrap();

        let open = execution.snapshot(&mut runtime).unwrap();
        assert_eq!(open.status(), ResultViewStatus::Provisional);
        assert_eq!(open.counts().input_rows(), ResultViewCount::LowerBound(2));
        let group = &open.output().groups().unwrap()[0];
        assert_eq!(
            group.projected_values(),
            Some(
                [
                    ResultValue::Value(ExploreValue::Int(1)),
                    ResultValue::Value(ExploreValue::Int(2)),
                    ResultValue::Value(ExploreValue::Int(4)),
                ]
                .as_slice()
            )
        );
        assert_eq!(open.contributions().len(), 2);

        execution.seal_input();
        let closed = execution.finish(&mut runtime).unwrap();
        assert_eq!(closed.counts().input_rows(), ResultViewCount::Exact(2));
        assert_eq!(
            closed.snapshot().output().groups().unwrap()[0].projected_values(),
            Some(
                [
                    ResultValue::Value(ExploreValue::Int(1)),
                    ResultValue::Value(ExploreValue::Int(2)),
                    ResultValue::Value(ExploreValue::Int(4)),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn choice_member_display_projects_only_authenticated_tied_members() {
        let (relation_id, question_id, _) = identities("municipality-ties");
        let mut catalog = RelationCatalogBuilder::new(relation_id);
        let copenhagen = insert_case(
            &mut catalog,
            "Copenhagen",
            ExploreValue::String("Copenhagen".to_string()),
            0,
            100_000,
        );
        let aarhus = insert_case(
            &mut catalog,
            "Aarhus",
            ExploreValue::String("Aarhus".to_string()),
            0,
            100_000,
        );
        let odense = insert_case(
            &mut catalog,
            "Odense",
            ExploreValue::String("Odense".to_string()),
            0,
            120_000,
        );
        let catalog = catalog.snapshot();
        let view = ExploreResultViewIr {
            node_index: 0,
            name: "lowest_tax".to_string(),
            input: ExploreResultInputIr::Find {
                find_name: "all_cases".into(),
                find_index: 0,
            },
            grain: ExploreResultGrainIr::GroupAll {
                span: Span::dummy(),
            },
            measures: vec![field("tax_ore", var("after"), int_ty())].into_boxed_slice(),
            aggregates: Box::new([]),
            having: Some(ExploreResultHavingIr::Varies {
                measure_name: "tax_ore".to_string(),
                measure_index: 0,
                span: Span::dummy(),
            }),
            select: vec![
                field("municipality", var("context"), string_ty()),
                field("payable", var("tax_ore"), int_ty()),
            ]
            .into_boxed_slice(),
            choose: Some(ExploreResultChoiceIr::Optimize {
                cardinality: ExploreChooseCardinality::All,
                direction: ExploreOptimizeDirection::Minimize,
                objective: var("tax_ore"),
                objective_ty: int_ty(),
                span: Span::dummy(),
            }),
            span: Span::dummy(),
        };
        let choice_id = ChoiceId::from_canonical_choice_preimage(question_id, b"lowest-tax");
        let view_id =
            ViewId::from_canonical_view_preimage(ViewInputId::Choice(choice_id), b"display");
        let executor = RelationalResultExecutor::lower(view_id, &view).unwrap();
        assert_eq!(executor.spec().grain(), &ResultViewGrain::EachCase);
        assert!(executor.spec().choice().is_none());
        assert!(executor.spec().having().is_none());
        let mut runtime = FixtureRuntime;
        let candidates = [copenhagen, aarhus, odense]
            .into_iter()
            .map(|case_id| {
                executor
                    .evaluate_choice_candidate(
                        choice_id,
                        catalog.case(case_id).unwrap(),
                        &mut runtime,
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut execution = executor.execution();
        for (ordinal, candidate) in candidates
            .into_iter()
            .filter(|candidate| candidate.objectives() == [100_000])
            .enumerate()
        {
            let member = ChoiceMember::restore_from_journal_codec(ordinal as u128, candidate);
            let case_id = member.case_id();
            execution
                .insert(
                    executor
                        .evaluate_choice_member(
                            choice_id,
                            &member,
                            catalog.case(case_id).unwrap(),
                            &mut runtime,
                        )
                        .unwrap(),
                )
                .unwrap();
        }
        execution.seal_input();
        let closed = execution.finish(&mut runtime).unwrap();
        let chosen = closed.snapshot().output().rows().unwrap();
        assert_eq!(chosen.len(), 2);
        assert_eq!(
            chosen
                .iter()
                .map(|row| row.row_id().case_id().expect("chosen case row"))
                .collect::<BTreeSet<_>>(),
            [copenhagen, aarhus].into_iter().collect()
        );
        assert!(chosen.iter().all(|row| {
            row.values().get(1) == Some(&ResultValue::Value(ExploreValue::Int(100_000)))
        }));
    }

    #[test]
    fn incidence_projection_preserves_typed_identity_values() {
        let (relation_id, _, request_id) = identities("typed-incidence");
        let mut catalog = RelationCatalogBuilder::new(relation_id);
        let case_id = insert_case(
            &mut catalog,
            "Case",
            ExploreValue::String("profile".to_string()),
            1,
            2,
        );
        let catalog = catalog.snapshot();
        let view = ExploreResultViewIr {
            node_index: 1,
            name: "typed_ids".to_string(),
            input: ExploreResultInputIr::MechanismIncidence {
                request_node_index: 0,
            },
            grain: ExploreResultGrainIr::EachIncidence {
                span: Span::dummy(),
            },
            measures: Box::new([]),
            aggregates: Box::new([]),
            having: None,
            select: vec![
                field("case", var("case_id"), opaque_id_ty("CaseId")),
                field(
                    "transition",
                    var("transition_id"),
                    opaque_id_ty("TransitionId"),
                ),
                field(
                    "signature",
                    var("signature_id"),
                    opaque_id_ty("MechanismSignatureId"),
                ),
                field(
                    "structural_mechanism",
                    var("structural_mechanism_id"),
                    opaque_id_ty("StructuralMechanismId"),
                ),
                field(
                    "execution_profile",
                    var("execution_profile_id"),
                    opaque_id_ty("ExecutionProfileId"),
                ),
            ]
            .into_boxed_slice(),
            choose: None,
            span: Span::dummy(),
        };
        let view_id = ViewId::from_canonical_view_preimage(
            ViewInputId::MechanismIncidence(request_id),
            b"typed-ids",
        );
        let executor = RelationalResultExecutor::lower(view_id, &view).unwrap();
        let transition_id = transition("Case");
        let signature_id = MechanismSignatureDefinition::from_canonical_definition(
            request_id,
            b"typed-signature".as_slice(),
        )
        .id();
        let (structural_mechanism_id, execution_profile_id) = structural_ids("typed");
        let mut runtime = FixtureRuntime;
        let mut execution = executor.execution();
        execution
            .insert(
                executor
                    .evaluate_concrete_incidence(
                        catalog.case(case_id).unwrap(),
                        transition_id,
                        signature_id,
                        structural_mechanism_id,
                        execution_profile_id,
                        &mut runtime,
                    )
                    .unwrap(),
            )
            .unwrap();
        execution.seal_input();
        let closed = execution.finish(&mut runtime).unwrap();
        assert_eq!(
            closed.snapshot().output().rows().unwrap()[0].values(),
            [
                ResultValue::CaseId(case_id),
                ResultValue::TransitionId(transition_id),
                ResultValue::SignatureId(signature_id),
                ResultValue::StructuralMechanismId(structural_mechanism_id),
                ResultValue::ExecutionProfileId(execution_profile_id),
            ]
        );
    }

    #[allow(dead_code)]
    fn _pareto_ir_fixture(objective: Expr) -> ExploreResultChoiceIr {
        ExploreResultChoiceIr::Pareto {
            objectives: vec![ExploreParetoObjectiveIr {
                direction: ExploreOptimizeDirection::Maximize,
                value: objective,
                ty: int_ty(),
                span: Span::dummy(),
            }]
            .into_boxed_slice(),
            span: Span::dummy(),
        }
    }
}
