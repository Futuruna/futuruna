//! Private fresh-replay mint for the first executable mechanism slice.
//!
//! This deliberately supports one closed profile only: two checked Explore
//! `show` call roots which resolve to the same top-level function, whose body
//! is a single `if` and contains no nested traceable calls or dynamic-control
//! events.  That restriction lets the ordinary Interpreter decide the branch
//! while every durable identity still comes from checked structural sites.
//! Runtime AST addresses are never observed, compared, hashed, or persisted.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use super::exact::{ExactFreshMatchReplayError, ExactFreshMatchShowObserver, ExactStreamEvaluator};
use super::mechanism::{
    CanonicalSignatureInterner, CheckedMechanismObservationRequestV1, CheckedMechanismRequestId,
    DynamicEndpointTraceV1, DynamicEventKind, DynamicEventOutcome, DynamicMechanismSignature,
    EndpointOccurrenceV1, IfDecisionOutcome, MechanismActivationStepV1, MechanismNumericBin,
    MechanismOccurrenceSlotV1, MechanismSignatureId, MechanismSiteId,
};
use super::mechanism_request::{build_checked_mechanism_request_v1, MechanismTraceSelectionV1};
use super::mechanism_stream::{
    seal_fresh_replay_confirmed_mechanism_batch_v1, MechanismBinAssignmentOutcomeV1,
    MechanismBinAssignmentV1, MechanismCanonicalCaseIdV1, MechanismCaseObservationProposalV1,
    MechanismObservationBatchProposalV1, MechanismPermanentUntracedReasonV1,
    MechanismValidationReceiptDigestV1, ValidatedMechanismObservationBatchV1,
};
use super::report::{ExploreEvaluationPhase, ExploreLimitResource, ExploreStopReason};
use super::source_events::{
    CheckedProgramSiteIndex, CheckedSourceExpressionSlice, ResolvedEventAdapterLimits,
};
use crate::{
    has_named_args, CheckedCallTarget, CheckedResolutionIssue, Env, ExploreRuntimeFailure,
    ExploreRuntimeResource, Expr, ExprKind, ExprSiteId, Interpreter, Stmt, TypeCheckArtifacts,
    Value,
};

const SINGLE_IF_REPLAY_RECEIPT_V1: &[u8] =
    b"futuruna.explore.single-if-mechanism-replay-receipt.v1";
const SINGLE_IF_REPLAY_RECEIPT_V2: &[u8] =
    b"futuruna.explore.single-if-mechanism-replay-receipt.v2";

#[derive(Clone)]
struct CheckedEndpointReplayV1 {
    arguments: Box<[Expr]>,
}

#[derive(Clone)]
struct CheckedBinReplayV1 {
    show_index: usize,
    field_name: Box<str>,
    bins: Box<[MechanismNumericBin]>,
}

/// Checked, source-bound plan for the deliberately constrained first runtime
/// mechanism profile. The request is suitable for sequence-zero stream
/// identity; the remaining fields are local replay material only.
pub(super) struct CheckedSingleIfMechanismRuntimePlanV1 {
    request: CheckedMechanismObservationRequestV1,
    before_show_index: usize,
    after_show_index: usize,
    before: CheckedEndpointReplayV1,
    after: CheckedEndpointReplayV1,
    parameter_names: Box<[Box<str>]>,
    condition: Expr,
    if_site: MechanismSiteId,
    bin_fields: Box<[CheckedBinReplayV1]>,
}

impl CheckedSingleIfMechanismRuntimePlanV1 {
    pub(super) fn from_show_call_roots(
        artifacts: &TypeCheckArtifacts,
        accepted_query_index: usize,
        before_show_index: usize,
        after_show_index: usize,
    ) -> Result<Self, String> {
        Self::from_trace_selection(
            artifacts,
            accepted_query_index,
            MechanismTraceSelectionV1 {
                before_show_index,
                after_show_index,
                bin_fields: Box::default(),
                retained_examples_per_signature: 1,
            },
        )
    }

    pub(super) fn from_trace_selection(
        artifacts: &TypeCheckArtifacts,
        accepted_query_index: usize,
        selection: MechanismTraceSelectionV1,
    ) -> Result<Self, String> {
        let before_show_index = selection.before_show_index;
        let after_show_index = selection.after_show_index;
        if before_show_index == after_show_index {
            return Err("mechanism before and after show roots must be distinct".to_string());
        }
        let mut selected_bin_fields = selection.bin_fields.to_vec();
        selected_bin_fields.sort_by_key(|field| field.show_index);
        let request =
            build_checked_mechanism_request_v1(artifacts, accepted_query_index, selection)
                .map_err(|error| format!("cannot check single-if mechanism request: {error}"))?;
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(|error| format!("cannot select checked mechanism query: {error:?}"))?;
        let before_site = checked
            .artifact
            .sites
            .show
            .get(before_show_index)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "mechanism before show index {before_show_index} is outside {} checked roots",
                    checked.artifact.sites.show.len()
                )
            })?;
        let after_site = checked
            .artifact
            .sites
            .show
            .get(after_show_index)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "mechanism after show index {after_show_index} is outside {} checked roots",
                    checked.artifact.sites.show.len()
                )
            })?;

        let source_index =
            CheckedProgramSiteIndex::build(artifacts, ResolvedEventAdapterLimits::default())
                .map_err(|error| format!("cannot index checked mechanism source: {error}"))?;
        let before_source = source_index
            .expression_slice(&before_site)
            .map_err(|error| format!("cannot locate before mechanism root: {error}"))?;
        let after_source = source_index
            .expression_slice(&after_site)
            .map_err(|error| format!("cannot locate after mechanism root: {error}"))?;
        validate_endpoint_slice(artifacts, &before_source)?;
        validate_endpoint_slice(artifacts, &after_source)?;

        let before_target = checked_call_target(artifacts, &before_site, "before")?;
        let after_target = checked_call_target(artifacts, &after_site, "after")?;
        if before_target != after_target {
            return Err(
                "mechanism show roots do not resolve to the same exact callable".to_string(),
            );
        }
        let callable = match before_target {
            CheckedCallTarget::Function { callable, .. } => callable,
            _ => {
                return Err(
                    "single-if mechanism replay requires a top-level function target".to_string(),
                )
            }
        };
        if !callable.structural_path.is_empty() {
            return Err(
                "single-if mechanism replay does not yet support methods or nested functions"
                    .to_string(),
            );
        }

        let before = endpoint_replay(&before_source, "before")?;
        let after = endpoint_replay(&after_source, "after")?;
        let callable_source = source_index
            .callable_slice(&callable)
            .map_err(|error| format!("cannot locate common mechanism callable: {error}"))?;
        let (parameter_names, condition, if_site) = single_if_callable_parts(&callable_source)?;
        validate_callable_slice(artifacts, &callable_source.body, &if_site)?;
        if before.arguments.len() != parameter_names.len()
            || after.arguments.len() != parameter_names.len()
        {
            return Err(
                "mechanism endpoint arity disagrees with the checked common function".to_string(),
            );
        }

        if selected_bin_fields.len() != request.observation.bin_fields.len() {
            return Err(
                "checked mechanism bin selection width changed while building runtime plan"
                    .to_string(),
            );
        }
        let bin_fields = selected_bin_fields
            .into_iter()
            .zip(request.observation.bin_fields.iter())
            .map(|(selected, checked)| CheckedBinReplayV1 {
                show_index: selected.show_index,
                field_name: checked.name.clone(),
                bins: checked.bins.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self {
            request,
            before_show_index,
            after_show_index,
            before,
            after,
            parameter_names,
            condition,
            if_site: MechanismSiteId::from_expression_site(&if_site)
                .map_err(|error| error.to_string())?,
            bin_fields,
        })
    }

    pub(super) fn request(&self) -> &CheckedMechanismObservationRequestV1 {
        &self.request
    }
}

fn checked_call_target(
    artifacts: &TypeCheckArtifacts,
    site: &crate::ExprSiteId,
    endpoint: &str,
) -> Result<CheckedCallTarget, String> {
    artifacts
        .checked_resolutions
        .expressions
        .get(site)
        .and_then(|resolution| resolution.call_target.clone())
        .ok_or_else(|| format!("mechanism {endpoint} show root is not a checked call"))
}

fn endpoint_replay(
    source: &CheckedSourceExpressionSlice<'_>,
    endpoint: &str,
) -> Result<CheckedEndpointReplayV1, String> {
    let ExprKind::App(_, arguments) = &source.root.expression.kind else {
        return Err(format!(
            "mechanism {endpoint} show root must be a direct function call"
        ));
    };
    if has_named_args(arguments.as_slice()) {
        return Err(format!(
            "mechanism {endpoint} show root does not yet support named arguments"
        ));
    }
    Ok(CheckedEndpointReplayV1 {
        arguments: arguments.clone().into_boxed_slice(),
    })
}

fn require_fully_checked_slice(
    artifacts: &TypeCheckArtifacts,
    source: &CheckedSourceExpressionSlice<'_>,
) -> Result<(), String> {
    let direct_callee_path =
        matches!(&source.root.expression.kind, ExprKind::App(_, _)).then(|| {
            let mut path = source.root.site.ast_path.to_vec();
            path.push(0);
            path
        });
    for item in source.descendants.iter() {
        let issues = artifacts
            .checked_resolutions
            .issues_for_reachable_sites([&item.site]);
        let exact_root_call_callee = direct_callee_path.as_deref()
            == Some(item.site.ast_path.as_ref())
            && matches!(&item.expression.kind, ExprKind::Var(_))
            && issues.len() == 1
            && issues.contains(&CheckedResolutionIssue::TypeNotResolved);
        if exact_root_call_callee {
            // The checker records the exact callable on the enclosing App
            // site. A bare function name is not itself a first-class typed
            // value, so its callee child intentionally has no standalone Ty.
            continue;
        }
        if !issues.is_empty() {
            return Err(format!(
                "single-if mechanism source has incomplete checked resolution at structural path {:?} ({:?}): {issues:?}",
                item.site.ast_path, item.expression.kind
            ));
        }
    }
    Ok(())
}

fn dynamic_control_kind(expression: &Expr) -> Option<&'static str> {
    match &expression.kind {
        ExprKind::If(_, _, _) => Some("if"),
        ExprKind::Match(_, _) => Some("match"),
        ExprKind::Conjunction(_) => Some("conjunction"),
        ExprKind::Disjunction(_) => Some("disjunction"),
        ExprKind::BinOp(operator, _, _) if operator == "&&" => Some("short-circuit-and"),
        ExprKind::BinOp(operator, _, _) if operator == "||" => Some("short-circuit-or"),
        _ => None,
    }
}

fn has_checked_call(artifacts: &TypeCheckArtifacts, site: &ExprSiteId) -> bool {
    artifacts
        .checked_resolutions
        .expressions
        .get(site)
        .and_then(|resolution| resolution.call_target.as_ref())
        .is_some()
}

fn validate_endpoint_slice(
    artifacts: &TypeCheckArtifacts,
    source: &CheckedSourceExpressionSlice<'_>,
) -> Result<(), String> {
    require_fully_checked_slice(artifacts, source)?;
    for item in source.descendants.iter() {
        let is_root = item.site == source.root.site;
        if let Some(kind) = dynamic_control_kind(item.expression) {
            return Err(format!(
                "single-if mechanism endpoint contains unsupported dynamic control `{kind}`"
            ));
        }
        if has_checked_call(artifacts, &item.site) && !is_root {
            return Err("single-if mechanism endpoint contains a nested checked call".to_string());
        }
    }
    Ok(())
}

fn validate_callable_slice(
    artifacts: &TypeCheckArtifacts,
    source: &CheckedSourceExpressionSlice<'_>,
    selected_if_site: &ExprSiteId,
) -> Result<(), String> {
    require_fully_checked_slice(artifacts, source)?;
    let mut selected_if_seen = false;
    for item in source.descendants.iter() {
        if let Some(kind) = dynamic_control_kind(item.expression) {
            if &item.site != selected_if_site
                || !matches!(&item.expression.kind, ExprKind::If(_, _, _))
            {
                return Err(format!(
                    "single-if mechanism function contains unsupported dynamic control `{kind}` outside its selected `if` site"
                ));
            }
            selected_if_seen = true;
        }
        if has_checked_call(artifacts, &item.site) {
            return Err("single-if mechanism function contains a nested checked call".to_string());
        }
    }
    if selected_if_seen {
        Ok(())
    } else {
        Err("single-if mechanism function lost its selected checked `if` site".to_string())
    }
}

fn single_if_callable_parts(
    source: &super::source_events::CheckedCallableSourceSlice<'_>,
) -> Result<(Box<[Box<str>]>, Expr, crate::ExprSiteId), String> {
    let Stmt::Defn(crate::Defn::Fn { params, body, .. }) = &*source.declaration.statement else {
        return Err("checked mechanism callable is not a source function".to_string());
    };
    let ExprKind::Block(statements) = &body.kind else {
        return Err("single-if mechanism function body is not a source block".to_string());
    };
    let [Stmt::Expr(if_expression)] = statements.as_slice() else {
        return Err(
            "single-if mechanism function must contain exactly one expression statement"
                .to_string(),
        );
    };
    let ExprKind::If(condition, _, _) = &if_expression.kind else {
        return Err("single-if mechanism function expression is not an `if`".to_string());
    };
    let mut if_sites = source
        .body
        .descendants
        .iter()
        .filter(|item| matches!(item.expression.kind, ExprKind::If(_, _, _)))
        .map(|item| item.site.clone());
    let if_site = if_sites
        .next()
        .ok_or_else(|| "single-if mechanism body has no checked `if` site".to_string())?;
    if if_sites.next().is_some() {
        return Err("single-if mechanism body has more than one `if` site".to_string());
    }
    Ok((
        params
            .iter()
            .map(|parameter| parameter.name.clone().into_boxed_str())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        (**condition).clone(),
        if_site,
    ))
}

/// Non-forgeable outside this module: the fields are private and construction
/// follows a fresh matching-case replay or an explicit permanent-untraced
/// result from that replay.
pub(super) struct RuntimeConfirmedMechanismObservationV1 {
    checked_request_id: CheckedMechanismRequestId,
    rank: u128,
    definitions: Box<[DynamicMechanismSignature]>,
    observation: MechanismCaseObservationProposalV1,
}

impl RuntimeConfirmedMechanismObservationV1 {
    pub(super) const fn rank(&self) -> u128 {
        self.rank
    }
}

struct SingleIfReplayObserver<'plan> {
    plan: &'plan CheckedSingleIfMechanismRuntimePlanV1,
    before: Option<IfDecisionOutcome>,
    after: Option<IfDecisionOutcome>,
    bin_assignments: Vec<MechanismBinAssignmentV1>,
}

impl<'plan> SingleIfReplayObserver<'plan> {
    fn new(plan: &'plan CheckedSingleIfMechanismRuntimePlanV1) -> Self {
        Self {
            plan,
            before: None,
            after: None,
            bin_assignments: Vec::with_capacity(plan.bin_fields.len()),
        }
    }

    fn finish(
        mut self,
    ) -> Result<
        (
            IfDecisionOutcome,
            IfDecisionOutcome,
            Box<[MechanismBinAssignmentV1]>,
        ),
        String,
    > {
        if self.bin_assignments.len() != self.plan.bin_fields.len() {
            return Err(format!(
                "mechanism replay observed {} bin fields but the checked request requires {}",
                self.bin_assignments.len(),
                self.plan.bin_fields.len()
            ));
        }
        self.bin_assignments
            .sort_by(|left, right| left.field_name.cmp(&right.field_name));
        Ok((
            self.before
                .ok_or_else(|| "before mechanism show root was not replayed".to_string())?,
            self.after
                .ok_or_else(|| "after mechanism show root was not replayed".to_string())?,
            self.bin_assignments.into_boxed_slice(),
        ))
    }
}

impl ExactFreshMatchShowObserver for SingleIfReplayObserver<'_> {
    fn before_show(
        &mut self,
        show_index: usize,
        interpreter: &mut Interpreter,
        environment: &Env,
        step_limit: usize,
        collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let (slot, endpoint, phase_name) = if show_index == self.plan.before_show_index {
            (
                &mut self.before,
                &self.plan.before,
                "mechanism before endpoint",
            )
        } else if show_index == self.plan.after_show_index {
            (
                &mut self.after,
                &self.plan.after,
                "mechanism after endpoint",
            )
        } else {
            return Ok(());
        };
        if slot.is_some() {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "mechanism show index {show_index} was replayed more than once"
            )));
        }
        *slot = Some(replay_endpoint(
            interpreter,
            environment,
            endpoint,
            &self.plan.parameter_names,
            &self.plan.condition,
            phase_name,
            step_limit,
            collection_limit,
        )?);
        Ok(())
    }

    fn after_show(
        &mut self,
        show_index: usize,
        _interpreter: &mut Interpreter,
        _environment: &Env,
        value: &super::ExploreValue,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let Some(field) = self
            .plan
            .bin_fields
            .iter()
            .find(|field| field.show_index == show_index)
        else {
            return Ok(());
        };
        let super::ExploreValue::Int(value) = value else {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "checked Int mechanism bin field `{}` replayed as a non-Int value",
                field.field_name
            )));
        };
        self.bin_assignments.push(assign_numeric_bin(field, *value));
        Ok(())
    }
}

fn assign_numeric_bin(field: &CheckedBinReplayV1, value: i64) -> MechanismBinAssignmentV1 {
    let insertion = field
        .bins
        .partition_point(|bin| bin.lower_inclusive <= value);
    let selected = insertion
        .checked_sub(1)
        .and_then(|index| field.bins.get(index))
        .copied()
        .filter(|bin| value < bin.upper_exclusive);
    match selected {
        Some(bin) => MechanismBinAssignmentV1::binned(field.field_name.clone(), bin),
        None => MechanismBinAssignmentV1::outside_declared_bins(field.field_name.clone()),
    }
}

pub(super) fn mint_single_if_mechanism_observation_v1(
    plan: &CheckedSingleIfMechanismRuntimePlanV1,
    evaluator: &ExactStreamEvaluator<'_>,
    rank: u128,
) -> Result<RuntimeConfirmedMechanismObservationV1, String> {
    if &plan.request.observation.query != evaluator.checked_mechanism_query_id() {
        return Err(
            "single-if mechanism plan and exact evaluator name different checked queries"
                .to_string(),
        );
    }
    let ordinals = evaluator.canonical_ordinals_for_rank(rank)?;
    let case_id = MechanismCanonicalCaseIdV1::new(rank, ordinals);
    let mut observer = SingleIfReplayObserver::new(plan);
    let replay = evaluator.fresh_replay_confirmed_match_shows(rank, &mut observer);

    match replay {
        Ok(()) => {
            let (before, after, bin_assignments) = observer.finish()?;
            observed_token(plan, case_id, before, after, bin_assignments)
        }
        Err(ExactFreshMatchReplayError::ReplayUnavailable(_)) => Ok(untraced_token(
            plan,
            case_id,
            MechanismPermanentUntracedReasonV1::ReplayUnavailable,
        )),
        Err(ExactFreshMatchReplayError::ObservationUnsupported(_)) => Ok(untraced_token(
            plan,
            case_id,
            MechanismPermanentUntracedReasonV1::ObservationUnsupported,
        )),
        Err(ExactFreshMatchReplayError::NotConfirmedMatch) => Err(format!(
            "mechanism replay rank {rank} no longer fresh-replays as an admissible match"
        )),
        Err(ExactFreshMatchReplayError::OperationalLimit(stop)) => Err(format!(
            "cannot fresh-confirm mechanism replay rank {rank}: {stop:?}"
        )),
        Err(ExactFreshMatchReplayError::Failure(message)) => Err(format!(
            "cannot fresh-replay mechanism rank {rank}: {message}"
        )),
    }
}

fn replay_endpoint(
    interpreter: &mut Interpreter,
    environment: &Env,
    endpoint: &CheckedEndpointReplayV1,
    parameter_names: &[Box<str>],
    condition: &Expr,
    phase_name: &str,
    step_limit: usize,
    collection_limit: usize,
) -> Result<IfDecisionOutcome, ExactFreshMatchReplayError> {
    let mut arguments = Vec::with_capacity(endpoint.arguments.len());
    for argument in endpoint.arguments.iter() {
        arguments.push(eval_replay_expression(
            interpreter,
            argument,
            environment,
            phase_name,
            step_limit,
            collection_limit,
        )?);
    }
    let mut function_environment = environment.child();
    for (parameter, argument) in parameter_names.iter().zip(arguments) {
        function_environment.set(parameter.to_string(), argument);
    }
    match eval_replay_expression(
        interpreter,
        condition,
        &function_environment,
        phase_name,
        step_limit,
        collection_limit,
    )? {
        Value::Bool(true) => Ok(IfDecisionOutcome::Then),
        Value::Bool(false) => Ok(IfDecisionOutcome::Else),
        _ => Err(ExactFreshMatchReplayError::ObservationUnsupported(
            "single-if mechanism condition did not evaluate to Bool".to_string(),
        )),
    }
}

fn eval_replay_expression(
    interpreter: &mut Interpreter,
    expression: &Expr,
    environment: &Env,
    phase_name: &str,
    step_limit: usize,
    collection_limit: usize,
) -> Result<Value, ExactFreshMatchReplayError> {
    interpreter
        .eval_exact_exploration(expression, environment, step_limit, collection_limit)
        .map_err(|failure| classify_replay_failure(failure, phase_name))
}

fn classify_replay_failure(
    failure: ExploreRuntimeFailure,
    phase_name: &str,
) -> ExactFreshMatchReplayError {
    let message = failure.to_string();
    match failure {
        ExploreRuntimeFailure::OperationalLimit {
            resource,
            limit,
            observed,
        } => ExactFreshMatchReplayError::OperationalLimit(ExploreStopReason::RuntimeLimit {
            resource: match resource {
                ExploreRuntimeResource::InitializationSteps
                | ExploreRuntimeResource::ExpressionSteps => ExploreLimitResource::Steps,
                ExploreRuntimeResource::CollectionMembers { operation } => {
                    ExploreLimitResource::CollectionMembers { operation }
                }
            },
            limit,
            observed,
            phase: ExploreEvaluationPhase::Show {
                name: phase_name.to_string(),
            },
        }),
        ExploreRuntimeFailure::UnsupportedCapability { .. }
        | ExploreRuntimeFailure::ProducedOutput => {
            ExactFreshMatchReplayError::ObservationUnsupported(message)
        }
        ExploreRuntimeFailure::RuntimeError { .. } | ExploreRuntimeFailure::Panicked => {
            ExactFreshMatchReplayError::Failure(message)
        }
    }
}

fn observed_token(
    plan: &CheckedSingleIfMechanismRuntimePlanV1,
    case_id: MechanismCanonicalCaseIdV1,
    before: IfDecisionOutcome,
    after: IfDecisionOutcome,
    bin_assignments: Box<[MechanismBinAssignmentV1]>,
) -> Result<RuntimeConfirmedMechanismObservationV1, String> {
    let slot = MechanismOccurrenceSlotV1::new(
        &plan.request.observation,
        0,
        Vec::<MechanismActivationStepV1>::new(),
        plan.if_site.clone(),
        DynamicEventKind::IfDecision,
        0,
    )
    .map_err(|error| error.to_string())?;
    let before_trace = DynamicEndpointTraceV1::new(
        &plan.request.observation,
        BTreeSet::from([slot.clone()]),
        [EndpointOccurrenceV1::new(
            slot.clone(),
            DynamicEventOutcome::IfDecision(before),
            BTreeSet::new(),
        )],
    )
    .map_err(|error| error.to_string())?;
    let after_trace = DynamicEndpointTraceV1::new(
        &plan.request.observation,
        BTreeSet::from([slot.clone()]),
        [EndpointOccurrenceV1::new(
            slot,
            DynamicEventOutcome::IfDecision(after),
            BTreeSet::new(),
        )],
    )
    .map_err(|error| error.to_string())?;
    let signature = DynamicMechanismSignature::from_endpoint_traces(
        &plan.request.observation,
        before_trace,
        after_trace,
    )
    .map_err(|error| error.to_string())?;
    let mut interner = CanonicalSignatureInterner::new(&plan.request.observation);
    let signature_id = interner
        .intern(signature)
        .map_err(|error| error.to_string())?;
    let definitions = interner
        .into_signatures()
        .into_values()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let receipt = replay_receipt(
        &plan.request,
        &case_id,
        Some(&signature_id),
        &bin_assignments,
        None,
    );
    Ok(RuntimeConfirmedMechanismObservationV1 {
        checked_request_id: plan.request.id.clone(),
        rank: case_id.rank,
        definitions,
        observation: MechanismCaseObservationProposalV1::observed(
            case_id,
            signature_id,
            bin_assignments,
            receipt,
        ),
    })
}

fn untraced_token(
    plan: &CheckedSingleIfMechanismRuntimePlanV1,
    case_id: MechanismCanonicalCaseIdV1,
    reason: MechanismPermanentUntracedReasonV1,
) -> RuntimeConfirmedMechanismObservationV1 {
    let receipt = replay_receipt(&plan.request, &case_id, None, &[], Some(reason));
    RuntimeConfirmedMechanismObservationV1 {
        checked_request_id: plan.request.id.clone(),
        rank: case_id.rank,
        definitions: Box::default(),
        observation: MechanismCaseObservationProposalV1::permanently_untraced(
            case_id, reason, receipt,
        ),
    }
}

fn replay_receipt(
    request: &CheckedMechanismObservationRequestV1,
    case_id: &MechanismCanonicalCaseIdV1,
    signature: Option<&MechanismSignatureId>,
    bin_assignments: &[MechanismBinAssignmentV1],
    untraced: Option<MechanismPermanentUntracedReasonV1>,
) -> MechanismValidationReceiptDigestV1 {
    fn segment(hasher: &mut Sha256, bytes: &[u8]) {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    let mut hasher = Sha256::new();
    let bins_requested = !request.observation.bin_fields.is_empty();
    segment(
        &mut hasher,
        if bins_requested {
            SINGLE_IF_REPLAY_RECEIPT_V2
        } else {
            SINGLE_IF_REPLAY_RECEIPT_V1
        },
    );
    segment(&mut hasher, &request.id.digest_bytes());
    segment(&mut hasher, &case_id.rank.to_le_bytes());
    segment(&mut hasher, &(case_id.ordinals.len() as u64).to_le_bytes());
    for ordinal in case_id.ordinals.iter().copied() {
        segment(&mut hasher, &ordinal.to_le_bytes());
    }
    if bins_requested {
        segment(&mut hasher, &(bin_assignments.len() as u64).to_le_bytes());
        for assignment in bin_assignments {
            segment(&mut hasher, assignment.field_name.as_bytes());
            match assignment.outcome {
                MechanismBinAssignmentOutcomeV1::Binned(bin) => {
                    segment(&mut hasher, b"binned");
                    segment(&mut hasher, &bin.lower_inclusive.to_le_bytes());
                    segment(&mut hasher, &bin.upper_exclusive.to_le_bytes());
                }
                MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins => {
                    segment(&mut hasher, b"outside-declared-bins");
                }
                MechanismBinAssignmentOutcomeV1::ReplayUnavailable => {
                    segment(&mut hasher, b"replay-unavailable");
                }
                MechanismBinAssignmentOutcomeV1::ObservationUnsupported => {
                    segment(&mut hasher, b"observation-unsupported");
                }
            }
        }
    }
    match (signature, untraced) {
        (Some(signature), None) => {
            segment(&mut hasher, b"observed");
            segment(&mut hasher, &signature.digest_bytes());
        }
        (None, Some(MechanismPermanentUntracedReasonV1::ReplayUnavailable)) => {
            segment(&mut hasher, b"replay-unavailable");
        }
        (None, Some(MechanismPermanentUntracedReasonV1::ObservationUnsupported)) => {
            segment(&mut hasher, b"observation-unsupported");
        }
        _ => unreachable!("runtime replay receipt has one canonical outcome"),
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    MechanismValidationReceiptDigestV1::new(bytes)
}

pub(super) fn seal_runtime_confirmed_mechanism_observation_v1(
    plan: &CheckedSingleIfMechanismRuntimePlanV1,
    confirmed: RuntimeConfirmedMechanismObservationV1,
) -> Result<ValidatedMechanismObservationBatchV1, String> {
    if confirmed.checked_request_id != plan.request.id {
        return Err(
            "runtime mechanism observation token belongs to another checked request".to_string(),
        );
    }
    let proposal = MechanismObservationBatchProposalV1::new(
        &plan.request,
        confirmed.definitions,
        vec![confirmed.observation],
    )
    .map_err(|error| error.to_string())?;
    let expected = proposal.clone();
    seal_fresh_replay_confirmed_mechanism_batch_v1(&plan.request, proposal, move |candidate| {
        if candidate != &expected {
            return Err("runtime mechanism proposal changed after its fresh replay".to_string());
        }
        Ok(())
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin_field(bins: Vec<MechanismNumericBin>) -> CheckedBinReplayV1 {
        CheckedBinReplayV1 {
            show_index: 0,
            field_name: "loss_ore".into(),
            bins: bins.into_boxed_slice(),
        }
    }

    #[test]
    fn numeric_bin_assignment_is_half_open_and_preserves_gaps() {
        let field = bin_field(vec![
            MechanismNumericBin::new(-5_000, 0).expect("negative bin"),
            MechanismNumericBin::new(5_000, 10_000).expect("positive bin"),
        ]);
        for (value, expected) in [
            (-5_000, Some((-5_000, 0))),
            (-1, Some((-5_000, 0))),
            (0, None),
            (4_999, None),
            (5_000, Some((5_000, 10_000))),
            (9_999, Some((5_000, 10_000))),
            (10_000, None),
        ] {
            let assignment = assign_numeric_bin(&field, value);
            match (assignment.outcome, expected) {
                (MechanismBinAssignmentOutcomeV1::Binned(actual), Some((lower, upper))) => {
                    assert_eq!(
                        (actual.lower_inclusive, actual.upper_exclusive),
                        (lower, upper)
                    );
                }
                (MechanismBinAssignmentOutcomeV1::OutsideDeclaredBins, None) => {}
                (actual, expected) => {
                    panic!("value {value} assigned as {actual:?}; expected {expected:?}")
                }
            }
        }
    }

    #[test]
    fn operational_observer_failure_stays_pending_and_runtime_failure_fails_closed() {
        match classify_replay_failure(
            ExploreRuntimeFailure::OperationalLimit {
                resource: ExploreRuntimeResource::ExpressionSteps,
                limit: 10,
                observed: 11,
            },
            "before",
        ) {
            ExactFreshMatchReplayError::OperationalLimit(ExploreStopReason::RuntimeLimit {
                resource: ExploreLimitResource::Steps,
                limit: 10,
                observed: 11,
                ..
            }) => {}
            other => panic!("operational limit became durable failure evidence: {other:?}"),
        }

        assert!(matches!(
            classify_replay_failure(
                ExploreRuntimeFailure::RuntimeError {
                    message: "boom".to_string(),
                },
                "after",
            ),
            ExactFreshMatchReplayError::Failure(message) if message == "boom"
        ));
    }
}
