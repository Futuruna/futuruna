//! Private fresh-replay mints for the first executable mechanism profiles.
//!
//! Each profile pairs checked Explore `show` call roots. The original profile
//! evaluates one direct `if`; the nested profile executes one direct positional
//! helper activation whose body contains one `if`; the rule profile records the
//! canonical candidate-attempt prefix and terminal selection of one direct
//! global rule family. The ordinary Interpreter decides every actual outcome,
//! and every durable identity comes from checked structural sites. Runtime AST
//! addresses are never observed, compared, hashed, or persisted.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::exact::{ExactFreshMatchReplayError, ExactFreshMatchShowObserver, ExactStreamEvaluator};
use super::mechanism::{
    CanonicalSignatureInterner, CheckedMechanismObservationRequestV1, CheckedMechanismRequestId,
    DynamicEndpointTraceV1, DynamicEventKind, DynamicEventOutcome, DynamicMechanismSignature,
    EndpointOccurrenceV1, IfDecisionOutcome, MechanismActivationStepV1, MechanismCallableSiteId,
    MechanismNumericBin, MechanismOccurrenceSlotV1, MechanismSignatureId, MechanismSiteId,
    RuleAttemptOutcome, RuleSelectionOutcome,
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
    has_named_args, CheckedCallTarget, CheckedResolutionIssue, CheckedRuntimeIfDecisionV1,
    CheckedRuntimeRuleAttemptOutcomeV1, CheckedRuntimeRuleSelectionV1,
    CheckedRuntimeRuleTraceErrorV1, CheckedRuntimeRuleTracePlanV1, CheckedRuntimeRuleTraceV1,
    CheckedRuntimeTraceErrorV1, CheckedRuntimeTracePlanV1, CheckedRuntimeTraceV1, Env,
    ExploreRuntimeFailure, ExploreRuntimeResource, Expr, ExprKind, ExprSiteId, Interpreter, Stmt,
    TypeCheckArtifacts, Value,
};

const SINGLE_IF_REPLAY_RECEIPT_V1: &[u8] =
    b"futuruna.explore.single-if-mechanism-replay-receipt.v1";
const SINGLE_IF_REPLAY_RECEIPT_V2: &[u8] =
    b"futuruna.explore.single-if-mechanism-replay-receipt.v2";

#[derive(Clone)]
struct CheckedEndpointReplayV1 {
    endpoint: Expr,
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
    checked_show_sites: Box<[ExprSiteId]>,
    before_show_index: usize,
    after_show_index: usize,
    before: CheckedEndpointReplayV1,
    after: CheckedEndpointReplayV1,
    callable: crate::CheckedCallableId,
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
        artifacts.require_mechanism_runtime_root_v1()?;
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
        let checked_show_sites = checked.artifact.sites.show.clone();
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
            checked_show_sites,
            before_show_index,
            after_show_index,
            before,
            after,
            callable,
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

/// Checked plan for the first actual nested activation profile. The paired
/// show roots call one common endpoint function; that function calls one
/// helper whose ordinary evaluation executes exactly one checked `if`.
pub(super) struct CheckedNestedIfMechanismRuntimePlanV1 {
    request: CheckedMechanismObservationRequestV1,
    before_show_index: usize,
    after_show_index: usize,
    before_show_site: ExprSiteId,
    after_show_site: ExprSiteId,
    before_trace: CheckedRuntimeTracePlanV1,
    after_trace: CheckedRuntimeTracePlanV1,
    profile: CheckedNestedIfProfileV1,
}

impl CheckedNestedIfMechanismRuntimePlanV1 {
    pub(super) fn from_show_call_roots(
        artifacts: &TypeCheckArtifacts,
        accepted_query_index: usize,
        before_show_index: usize,
        after_show_index: usize,
    ) -> Result<Self, String> {
        artifacts.require_mechanism_runtime_root_v1()?;
        if before_show_index == after_show_index {
            return Err("mechanism before and after show roots must be distinct".to_string());
        }
        let request = build_checked_mechanism_request_v1(
            artifacts,
            accepted_query_index,
            MechanismTraceSelectionV1 {
                before_show_index,
                after_show_index,
                bin_fields: Box::default(),
                retained_examples_per_signature: 1,
            },
        )
        .map_err(|error| format!("cannot check nested-if mechanism request: {error}"))?;
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(|error| format!("cannot select checked mechanism query: {error:?}"))?;
        let before_show_site = checked
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
        let after_show_site = checked
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
            .expression_slice(&before_show_site)
            .map_err(|error| format!("cannot locate before mechanism root: {error}"))?;
        let after_source = source_index
            .expression_slice(&after_show_site)
            .map_err(|error| format!("cannot locate after mechanism root: {error}"))?;
        validate_endpoint_slice(artifacts, &before_source)?;
        validate_endpoint_slice(artifacts, &after_source)?;
        require_nested_trace_endpoint_subset(&before_source, "before mechanism endpoint")?;
        require_nested_trace_endpoint_subset(&after_source, "after mechanism endpoint")?;

        let before_target = checked_call_target(artifacts, &before_show_site, "before")?;
        let after_target = checked_call_target(artifacts, &after_show_site, "after")?;
        if before_target != after_target {
            return Err(
                "mechanism show roots do not resolve to the same exact callable".to_string(),
            );
        }
        let root_callable = match before_target {
            CheckedCallTarget::Function { callable, .. } => callable,
            _ => {
                return Err(
                    "nested-if mechanism replay requires a top-level function target".to_string(),
                )
            }
        };
        if !root_callable.structural_path.is_empty() {
            return Err(
                "nested-if mechanism replay does not yet support methods or nested functions"
                    .to_string(),
            );
        }
        let profile = nested_if_profile(artifacts, &source_index, root_callable)?;
        let before_trace = nested_if_runtime_trace_plan(
            &artifacts.analysis_program.id,
            before_show_site.clone(),
            &profile,
        )?;
        let after_trace = nested_if_runtime_trace_plan(
            &artifacts.analysis_program.id,
            after_show_site.clone(),
            &profile,
        )?;

        Ok(Self {
            request,
            before_show_index,
            after_show_index,
            before_show_site,
            after_show_site,
            before_trace,
            after_trace,
            profile,
        })
    }

    pub(super) fn request(&self) -> &CheckedMechanismObservationRequestV1 {
        &self.request
    }
}

/// Checked plan for the first direct rule-dispatch profile. Each paired show
/// root calls the same global family directly; the ordinary dispatcher emits
/// authenticated candidate attempts and one terminal selection.
pub(super) struct CheckedRuleDispatchMechanismRuntimePlanV1 {
    request: CheckedMechanismObservationRequestV1,
    before_show_index: usize,
    after_show_index: usize,
    before_show_site: ExprSiteId,
    after_show_site: ExprSiteId,
    before_trace: CheckedRuntimeRuleTracePlanV1,
    after_trace: CheckedRuntimeRuleTracePlanV1,
}

impl CheckedRuleDispatchMechanismRuntimePlanV1 {
    pub(super) fn from_show_call_roots(
        artifacts: &TypeCheckArtifacts,
        accepted_query_index: usize,
        before_show_index: usize,
        after_show_index: usize,
    ) -> Result<Self, String> {
        artifacts.require_mechanism_runtime_root_v1()?;
        if before_show_index == after_show_index {
            return Err("mechanism before and after show roots must be distinct".to_string());
        }
        let request = build_checked_mechanism_request_v1(
            artifacts,
            accepted_query_index,
            MechanismTraceSelectionV1 {
                before_show_index,
                after_show_index,
                bin_fields: Box::default(),
                retained_examples_per_signature: 1,
            },
        )
        .map_err(|error| format!("cannot check rule-dispatch mechanism request: {error}"))?;
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(|error| format!("cannot select checked mechanism query: {error:?}"))?;
        let before_show_site = checked
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
        let after_show_site = checked
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
            .expression_slice(&before_show_site)
            .map_err(|error| format!("cannot locate before rule mechanism root: {error}"))?;
        let after_source = source_index
            .expression_slice(&after_show_site)
            .map_err(|error| format!("cannot locate after rule mechanism root: {error}"))?;
        validate_rule_trace_endpoint(artifacts, &before_source, "before rule mechanism endpoint")?;
        validate_rule_trace_endpoint(artifacts, &after_source, "after rule mechanism endpoint")?;

        let before_target = checked_call_target(artifacts, &before_show_site, "before")?;
        let after_target = checked_call_target(artifacts, &after_show_site, "after")?;
        if before_target != after_target {
            return Err(
                "mechanism show roots do not resolve to the same exact rule family".to_string(),
            );
        }
        let CheckedCallTarget::RuleFamily(family_key) = before_target else {
            return Err(
                "rule-dispatch mechanism replay requires a direct checked rule-family target"
                    .to_string(),
            );
        };
        if family_key.scope.is_some() {
            return Err(
                "rule-dispatch mechanism V1 does not yet support RuleScope families".to_string(),
            );
        }
        let family = artifacts
            .checked_resolutions
            .rule_families
            .get(&family_key)
            .cloned()
            .ok_or_else(|| {
                "rule-dispatch mechanism target has no producer-minted candidate catalogue"
                    .to_string()
            })?;
        validate_rule_trace_candidates(artifacts, &source_index, &family)?;
        let before_trace = CheckedRuntimeRuleTracePlanV1::new(
            artifacts.analysis_program.id.clone(),
            before_show_site.clone(),
            family.clone(),
        )?;
        let after_trace = CheckedRuntimeRuleTracePlanV1::new(
            artifacts.analysis_program.id.clone(),
            after_show_site.clone(),
            family,
        )?;

        Ok(Self {
            request,
            before_show_index,
            after_show_index,
            before_show_site,
            after_show_site,
            before_trace,
            after_trace,
        })
    }

    pub(super) fn request(&self) -> &CheckedMechanismObservationRequestV1 {
        &self.request
    }
}

fn validate_rule_trace_endpoint(
    artifacts: &TypeCheckArtifacts,
    source: &CheckedSourceExpressionSlice<'_>,
    label: &str,
) -> Result<(), String> {
    require_fully_checked_slice(artifacts, source)?;
    let ExprKind::App(function, arguments) = &source.root.expression.kind else {
        return Err(format!("{label} must be a direct rule-family application"));
    };
    if !matches!(&function.kind, ExprKind::Var(_)) {
        return Err(format!("{label} must call a direct named rule family"));
    }
    if has_named_args(arguments) {
        return Err(format!("{label} must use positional arguments"));
    }
    for item in source.descendants.iter() {
        if item.site == source.root.site {
            continue;
        }
        if matches!(&item.expression.kind, ExprKind::App(_, _)) {
            return Err(format!(
                "{label} arguments cannot contain another application in rule-dispatch V1"
            ));
        }
        if let Some(kind) = dynamic_control_kind(item.expression) {
            return Err(format!(
                "{label} contains unsupported dynamic control `{kind}` at structural path {:?}",
                item.site.ast_path
            ));
        }
        if !matches!(
            &item.expression.kind,
            ExprKind::Var(_)
                | ExprKind::Lit(_)
                | ExprKind::Unit
                | ExprKind::BinOp(_, _, _)
                | ExprKind::UnOp(_, _)
        ) {
            return Err(format!(
                "{label} contains an expression outside the direct rule-dispatch subset at structural path {:?}",
                item.site.ast_path
            ));
        }
    }
    Ok(())
}

fn validate_rule_trace_candidates(
    artifacts: &TypeCheckArtifacts,
    source_index: &CheckedProgramSiteIndex<'_>,
    family: &crate::CheckedRuleFamilyResolution,
) -> Result<(), String> {
    for candidate in family.candidates.iter() {
        if !candidate.statement_path.is_empty()
            || !candidate
                .declaration
                .declaration
                .module
                .internal_path
                .is_empty()
        {
            return Err(
                "rule-dispatch mechanism V1 requires top-level checked candidates".to_string(),
            );
        }
        validate_rule_trace_candidate_head(source_index, family, candidate)?;
        for (role, site) in candidate
            .condition_site
            .iter()
            .map(|site| ("guard", site))
            .chain(candidate.value_site.iter().map(|site| ("body/value", site)))
        {
            let source = source_index
                .expression_slice(site)
                .map_err(|error| format!("cannot locate checked rule candidate {role}: {error}"))?;
            require_fully_checked_slice(artifacts, &source)?;
            for item in std::iter::once(&source.root).chain(source.descendants.iter()) {
                if let Some(kind) = dynamic_control_kind(item.expression) {
                    return Err(format!(
                        "rule-dispatch mechanism candidate {role} contains unsupported dynamic control `{kind}`"
                    ));
                }
                if matches!(&item.expression.kind, ExprKind::App(_, _)) {
                    return Err(format!(
                        "rule-dispatch mechanism candidate {role} contains a nested application"
                    ));
                }
                if !matches!(
                    &item.expression.kind,
                    ExprKind::Var(_)
                        | ExprKind::Lit(_)
                        | ExprKind::Unit
                        | ExprKind::BinOp(_, _, _)
                        | ExprKind::UnOp(_, _)
                ) {
                    return Err(format!(
                        "rule-dispatch mechanism candidate {role} contains an expression outside the direct trace subset"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The legacy parser accepts arbitrary expressions in a rule head, while the
/// ordinary dispatcher interprets only this recursive pattern grammar. Keep
/// V1 inside that explicit grammar so its `HeadMismatch` evidence never relies
/// on the matcher's historical catch-all arm.
fn validate_rule_trace_candidate_head(
    source_index: &CheckedProgramSiteIndex<'_>,
    family: &crate::CheckedRuleFamilyResolution,
    candidate: &crate::CheckedRuleCandidateResolution,
) -> Result<(), String> {
    let source = source_index
        .expression_slice(&candidate.head_site)
        .map_err(|error| format!("cannot locate checked rule candidate head: {error}"))?;
    let parameters = match &source.root.expression.kind {
        ExprKind::App(function, parameters)
            if matches!(&function.kind, ExprKind::Var(name) if name == &family.key.name)
                && parameters.len() == family.key.arity =>
        {
            parameters.as_slice()
        }
        ExprKind::Var(name) if name == &family.key.name && family.key.arity == 0 => &[],
        _ => {
            return Err(
                "rule-dispatch mechanism candidate head disagrees with its checked family"
                    .to_string(),
            )
        }
    };
    if parameters.iter().all(rule_trace_matcher_pattern_v1) {
        Ok(())
    } else {
        Err(
            "rule-dispatch mechanism candidate head contains an expression outside the direct matcher subset"
                .to_string(),
        )
    }
}

fn rule_trace_matcher_pattern_v1(pattern: &Expr) -> bool {
    match &pattern.kind {
        ExprKind::Var(_) | ExprKind::Lit(_) => true,
        ExprKind::Tuple(items) => items.iter().all(rule_trace_matcher_pattern_v1),
        ExprKind::App(function, arguments) if matches!(&function.kind, ExprKind::Var(_)) => {
            arguments.iter().all(rule_trace_matcher_pattern_v1)
        }
        ExprKind::App(_, _)
        | ExprKind::Lambda(_, _)
        | ExprKind::BinOp(_, _, _)
        | ExprKind::UnOp(_, _)
        | ExprKind::If(_, _, _)
        | ExprKind::Match(_, _)
        | ExprKind::Block(_)
        | ExprKind::Field(_, _)
        | ExprKind::Index(_, _)
        | ExprKind::List(_)
        | ExprKind::Effect(_, _)
        | ExprKind::Handle { .. }
        | ExprKind::Try(_)
        | ExprKind::Conjunction(_)
        | ExprKind::Disjunction(_)
        | ExprKind::Pipe(_, _)
        | ExprKind::Unit => false,
    }
}

fn nested_if_runtime_trace_plan(
    analysis_program: &crate::AnalysisProgramId,
    endpoint_root: ExprSiteId,
    profile: &CheckedNestedIfProfileV1,
) -> Result<CheckedRuntimeTracePlanV1, String> {
    CheckedRuntimeTracePlanV1::new(
        analysis_program.clone(),
        endpoint_root.clone(),
        profile.root_callable.clone(),
        BTreeMap::from([
            (endpoint_root, profile.root_callable.clone()),
            (
                profile.nested_call_site.clone(),
                profile.nested_callable.clone(),
            ),
        ]),
        BTreeMap::from([
            (
                profile.root_callable.clone(),
                profile.root_body_site.clone(),
            ),
            (
                profile.nested_callable.clone(),
                profile.nested_body_site.clone(),
            ),
        ]),
        BTreeSet::from([profile.if_site.clone()]),
    )
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
        endpoint: source.root.expression.clone(),
        arguments: arguments.clone().into_boxed_slice(),
    })
}

fn require_fully_checked_slice(
    artifacts: &TypeCheckArtifacts,
    source: &CheckedSourceExpressionSlice<'_>,
) -> Result<(), String> {
    for item in source.descendants.iter() {
        let issues = artifacts
            .checked_resolutions
            .issues_for_reachable_sites([&item.site]);
        let mut parent_path = item.site.ast_path.to_vec();
        let is_callee_child = parent_path.pop() == Some(0);
        let parent_site = is_callee_child.then(|| ExprSiteId {
            analysis_program: item.site.analysis_program.clone(),
            declaration: item.site.declaration.clone(),
            normalized_declaration_ordinal: item.site.normalized_declaration_ordinal,
            ast_path: parent_path.into_boxed_slice(),
        });
        let exact_checked_call_callee = parent_site
            .as_ref()
            .and_then(|site| artifacts.checked_resolutions.expressions.get(site))
            .and_then(|resolution| resolution.call_target.as_ref())
            .is_some()
            && matches!(&item.expression.kind, ExprKind::Var(_))
            && issues.len() == 1
            && issues.contains(&CheckedResolutionIssue::TypeNotResolved);
        if exact_checked_call_callee {
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

fn require_nested_trace_expression_subset(
    source: &CheckedSourceExpressionSlice<'_>,
    label: &str,
) -> Result<(), String> {
    let expressions = std::iter::once(&source.root).chain(
        source
            .descendants
            .iter()
            .filter(|item| item.site != source.root.site),
    );
    for item in expressions {
        match &item.expression.kind {
            ExprKind::Var(_)
            | ExprKind::Lit(_)
            | ExprKind::Unit
            | ExprKind::App(_, _)
            | ExprKind::UnOp(_, _)
            | ExprKind::If(_, _, _) => {}
            ExprKind::BinOp(operator, _, _) if operator != "&&" && operator != "||" => {}
            ExprKind::Block(statements) if matches!(statements.as_slice(), [Stmt::Expr(_)]) => {}
            ExprKind::BinOp(operator, _, _) => {
                return Err(format!(
                    "{label} contains unsupported short-circuit operator `{operator}` at structural path {:?}",
                    item.site.ast_path
                ));
            }
            ExprKind::Block(_) => {
                return Err(format!(
                    "{label} contains a block outside the one-expression trace subset at structural path {:?}",
                    item.site.ast_path
                ));
            }
            unsupported => {
                return Err(format!(
                    "{label} contains an expression outside the nested trace subset at structural path {:?} ({unsupported:?})",
                    item.site.ast_path
                ));
            }
        }
    }
    Ok(())
}

fn require_nested_trace_endpoint_subset(
    source: &CheckedSourceExpressionSlice<'_>,
    label: &str,
) -> Result<(), String> {
    require_nested_trace_expression_subset(source, label)?;
    let ExprKind::App(function, arguments) = &source.root.expression.kind else {
        return Err(format!("{label} must be a direct function application"));
    };
    if !matches!(&function.kind, ExprKind::Var(_)) {
        return Err(format!("{label} must call a direct named function"));
    }
    if has_named_args(arguments) {
        return Err(format!("{label} must use positional arguments"));
    }
    if source.descendants.iter().any(|item| {
        item.site != source.root.site && matches!(&item.expression.kind, ExprKind::App(_, _))
    }) {
        return Err(format!(
            "{label} arguments cannot contain another application in the first nested trace profile"
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct CheckedNestedIfProfileV1 {
    root_callable: crate::CheckedCallableId,
    root_body_site: ExprSiteId,
    nested_call_site: ExprSiteId,
    nested_callable: crate::CheckedCallableId,
    nested_body_site: ExprSiteId,
    if_site: ExprSiteId,
}

fn require_top_level_single_expression_function(
    source: &super::source_events::CheckedCallableSourceSlice<'_>,
    label: &str,
) -> Result<(), String> {
    let Stmt::Defn(crate::Defn::Fn { body, .. }) = &*source.declaration.statement else {
        return Err(format!("{label} is not a checked top-level function"));
    };
    let ExprKind::Block(statements) = &body.kind else {
        return Err(format!("{label} body is not a block"));
    };
    if !matches!(statements.as_slice(), [Stmt::Expr(_)]) {
        return Err(format!(
            "{label} must contain exactly one expression statement in the first nested trace profile"
        ));
    }
    Ok(())
}

fn nested_if_profile(
    artifacts: &TypeCheckArtifacts,
    source_index: &CheckedProgramSiteIndex<'_>,
    root_callable: crate::CheckedCallableId,
) -> Result<CheckedNestedIfProfileV1, String> {
    let root_source = source_index
        .callable_slice(&root_callable)
        .map_err(|error| format!("cannot locate nested-trace endpoint function: {error}"))?;
    require_top_level_single_expression_function(&root_source, "mechanism endpoint function")?;
    require_fully_checked_slice(artifacts, &root_source.body)?;
    require_nested_trace_expression_subset(&root_source.body, "mechanism endpoint function")?;

    let mut nested_call = None;
    for item in root_source.body.descendants.iter() {
        if let Some(kind) = dynamic_control_kind(item.expression) {
            return Err(format!(
                "nested-if endpoint function contains unsupported dynamic control `{kind}`"
            ));
        }
        let ExprKind::App(function, arguments) = &item.expression.kind else {
            if has_checked_call(artifacts, &item.site) {
                return Err(
                    "nested-if endpoint checked call is not represented by an application"
                        .to_string(),
                );
            }
            continue;
        };
        let target = artifacts
            .checked_resolutions
            .expressions
            .get(&item.site)
            .and_then(|resolution| resolution.call_target.as_ref())
            .ok_or_else(|| {
                "nested-if endpoint function contains an application without an exact checked target"
                    .to_string()
            })?;
        if !matches!(&function.kind, ExprKind::Var(_)) {
            return Err(
                "nested-if endpoint helper activation is not a direct named call".to_string(),
            );
        }
        if has_named_args(arguments) {
            return Err(
                "nested-if endpoint helper activation must use positional arguments".to_string(),
            );
        }
        let CheckedCallTarget::Function { callable, .. } = target else {
            return Err(
                "nested-if endpoint function contains a non-function checked call".to_string(),
            );
        };
        if !callable.structural_path.is_empty() {
            return Err(
                "nested-if endpoint function contains a method or nested-function call".to_string(),
            );
        }
        if nested_call
            .replace((item.site.clone(), callable.clone()))
            .is_some()
        {
            return Err(
                "nested-if endpoint function contains more than one checked helper call"
                    .to_string(),
            );
        }
    }
    let (nested_call_site, nested_callable) = nested_call.ok_or_else(|| {
        "nested-if endpoint function has no checked helper activation".to_string()
    })?;
    if nested_callable == root_callable {
        return Err("nested-if mechanism profile rejects recursive endpoint calls".to_string());
    }

    let nested_source = source_index
        .callable_slice(&nested_callable)
        .map_err(|error| format!("cannot locate nested mechanism helper: {error}"))?;
    require_top_level_single_expression_function(&nested_source, "nested mechanism helper")?;
    require_fully_checked_slice(artifacts, &nested_source.body)?;
    require_nested_trace_expression_subset(&nested_source.body, "nested mechanism helper")?;
    let mut if_site = None;
    for item in nested_source.body.descendants.iter() {
        if let Some(kind) = dynamic_control_kind(item.expression) {
            if !matches!(&item.expression.kind, ExprKind::If(_, _, _)) {
                return Err(format!(
                    "nested mechanism helper contains unsupported dynamic control `{kind}`"
                ));
            }
            if if_site.replace(item.site.clone()).is_some() {
                return Err(
                    "nested mechanism helper contains more than one dynamic event".to_string(),
                );
            }
        }
        if has_checked_call(artifacts, &item.site) {
            return Err("nested mechanism helper contains another checked call".to_string());
        }
        if matches!(&item.expression.kind, ExprKind::App(_, _)) {
            return Err(
                "nested mechanism helper contains another application outside the trace plan"
                    .to_string(),
            );
        }
    }
    let if_site = if_site
        .ok_or_else(|| "nested mechanism helper has no checked `if` decision".to_string())?;

    Ok(CheckedNestedIfProfileV1 {
        root_callable,
        root_body_site: root_source.body.root.site.clone(),
        nested_call_site,
        nested_callable,
        nested_body_site: nested_source.body.root.site.clone(),
        if_site,
    })
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

#[derive(Debug)]
pub(super) enum MechanismRuntimeMintErrorV1 {
    OperationalLimit(ExploreStopReason),
    Failure(String),
}

impl From<String> for MechanismRuntimeMintErrorV1 {
    fn from(message: String) -> Self {
        Self::Failure(message)
    }
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

    fn require_show_site(
        &self,
        show_index: usize,
        show_site: &ExprSiteId,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let Some(expected) = self.plan.checked_show_sites.get(show_index) else {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "mechanism replay received unknown show index {show_index}"
            )));
        };
        if expected != show_site {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "mechanism replay show index {show_index} disagrees with its producer-minted checked site"
            )));
        }
        Ok(())
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
        show_site: &ExprSiteId,
        interpreter: &mut Interpreter,
        environment: &Env,
        step_limit: usize,
        collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        self.require_show_site(show_index, show_site)?;
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
        interpreter
            .authenticate_checked_runtime_direct_callable_v1(
                &endpoint.endpoint,
                environment,
                &self.plan.request.observation.analysis_program,
                &self.plan.callable,
            )
            .map_err(|error| {
                ExactFreshMatchReplayError::Failure(format!(
                    "cannot authenticate {phase_name}: {error}"
                ))
            })?;
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
        show_site: &ExprSiteId,
        _interpreter: &mut Interpreter,
        _environment: &Env,
        value: &super::ExploreValue,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        self.require_show_site(show_index, show_site)?;
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

struct NestedIfReplayObserver<'plan> {
    plan: &'plan CheckedNestedIfMechanismRuntimePlanV1,
    before: Option<CheckedRuntimeTraceV1>,
    after: Option<CheckedRuntimeTraceV1>,
}

impl<'plan> NestedIfReplayObserver<'plan> {
    fn new(plan: &'plan CheckedNestedIfMechanismRuntimePlanV1) -> Self {
        Self {
            plan,
            before: None,
            after: None,
        }
    }

    fn finish(self) -> Result<(CheckedRuntimeTraceV1, CheckedRuntimeTraceV1), String> {
        Ok((
            self.before
                .ok_or_else(|| "before nested mechanism root was not replayed".to_string())?,
            self.after
                .ok_or_else(|| "after nested mechanism root was not replayed".to_string())?,
        ))
    }
}

impl ExactFreshMatchShowObserver for NestedIfReplayObserver<'_> {
    fn before_show(
        &mut self,
        show_index: usize,
        show_site: &ExprSiteId,
        interpreter: &mut Interpreter,
        _environment: &Env,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let (expected_site, trace_plan, observed) = if show_index == self.plan.before_show_index {
            (
                &self.plan.before_show_site,
                &self.plan.before_trace,
                &self.before,
            )
        } else if show_index == self.plan.after_show_index {
            (
                &self.plan.after_show_site,
                &self.plan.after_trace,
                &self.after,
            )
        } else {
            return Ok(());
        };
        if expected_site != show_site {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "nested mechanism show index {show_index} disagrees with its producer-minted checked site"
            )));
        }
        if observed.is_some() {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "nested mechanism show index {show_index} was replayed more than once"
            )));
        }
        interpreter
            .begin_checked_runtime_trace_v1(trace_plan.clone())
            .map_err(|error| {
                ExactFreshMatchReplayError::Failure(format!(
                    "cannot arm checked nested mechanism trace: {error:?}"
                ))
            })
    }

    fn after_show(
        &mut self,
        show_index: usize,
        show_site: &ExprSiteId,
        interpreter: &mut Interpreter,
        _environment: &Env,
        _value: &super::ExploreValue,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let (expected_site, observed) = if show_index == self.plan.before_show_index {
            (&self.plan.before_show_site, &mut self.before)
        } else if show_index == self.plan.after_show_index {
            (&self.plan.after_show_site, &mut self.after)
        } else {
            return Ok(());
        };
        if expected_site != show_site {
            interpreter.abort_checked_runtime_trace_v1();
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "nested mechanism show index {show_index} changed checked site during replay"
            )));
        }
        if observed.is_some() {
            interpreter.abort_checked_runtime_trace_v1();
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "nested mechanism show index {show_index} produced more than one trace"
            )));
        }
        match interpreter.finish_checked_runtime_trace_v1() {
            Ok(trace) => {
                *observed = Some(trace);
                Ok(())
            }
            Err(error) => Err(classify_checked_trace_finish_error(error)),
        }
    }
}

struct RuleDispatchReplayObserver<'plan> {
    plan: &'plan CheckedRuleDispatchMechanismRuntimePlanV1,
    before: Option<CheckedRuntimeRuleTraceV1>,
    after: Option<CheckedRuntimeRuleTraceV1>,
}

impl<'plan> RuleDispatchReplayObserver<'plan> {
    fn new(plan: &'plan CheckedRuleDispatchMechanismRuntimePlanV1) -> Self {
        Self {
            plan,
            before: None,
            after: None,
        }
    }

    fn finish(self) -> Result<(CheckedRuntimeRuleTraceV1, CheckedRuntimeRuleTraceV1), String> {
        Ok((
            self.before
                .ok_or_else(|| "before rule mechanism root was not replayed".to_string())?,
            self.after
                .ok_or_else(|| "after rule mechanism root was not replayed".to_string())?,
        ))
    }
}

impl ExactFreshMatchShowObserver for RuleDispatchReplayObserver<'_> {
    fn requires_rule_candidate_tokens_v1(&self) -> bool {
        true
    }

    fn before_show(
        &mut self,
        show_index: usize,
        show_site: &ExprSiteId,
        interpreter: &mut Interpreter,
        _environment: &Env,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let (expected_site, trace_plan, observed) = if show_index == self.plan.before_show_index {
            (
                &self.plan.before_show_site,
                &self.plan.before_trace,
                &self.before,
            )
        } else if show_index == self.plan.after_show_index {
            (
                &self.plan.after_show_site,
                &self.plan.after_trace,
                &self.after,
            )
        } else {
            return Ok(());
        };
        if expected_site != show_site {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "rule mechanism show index {show_index} disagrees with its producer-minted checked site"
            )));
        }
        if observed.is_some() {
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "rule mechanism show index {show_index} was replayed more than once"
            )));
        }
        interpreter
            .begin_checked_runtime_rule_trace_v1(trace_plan.clone())
            .map_err(|error| {
                ExactFreshMatchReplayError::Failure(format!(
                    "cannot arm checked rule mechanism trace: {error:?}"
                ))
            })
    }

    fn after_show(
        &mut self,
        show_index: usize,
        show_site: &ExprSiteId,
        interpreter: &mut Interpreter,
        _environment: &Env,
        _value: &super::ExploreValue,
        _step_limit: usize,
        _collection_limit: usize,
    ) -> Result<(), ExactFreshMatchReplayError> {
        let (expected_site, observed) = if show_index == self.plan.before_show_index {
            (&self.plan.before_show_site, &mut self.before)
        } else if show_index == self.plan.after_show_index {
            (&self.plan.after_show_site, &mut self.after)
        } else {
            return Ok(());
        };
        if expected_site != show_site {
            interpreter.abort_checked_runtime_rule_trace_v1();
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "rule mechanism show index {show_index} changed checked site during replay"
            )));
        }
        if observed.is_some() {
            interpreter.abort_checked_runtime_rule_trace_v1();
            return Err(ExactFreshMatchReplayError::Failure(format!(
                "rule mechanism show index {show_index} produced more than one trace"
            )));
        }
        match interpreter.finish_checked_runtime_rule_trace_v1() {
            Ok(trace) => {
                *observed = Some(trace);
                Ok(())
            }
            Err(error) => Err(classify_checked_rule_trace_finish_error(error)),
        }
    }
}

fn classify_checked_rule_trace_finish_error(
    error: CheckedRuntimeRuleTraceErrorV1,
) -> ExactFreshMatchReplayError {
    match error {
        CheckedRuntimeRuleTraceErrorV1::Unsupported(reason)
            if reason.is_observation_unsupported() =>
        {
            ExactFreshMatchReplayError::ObservationUnsupported(format!(
                "checked rule mechanism trace is unsupported: {reason:?}"
            ))
        }
        CheckedRuntimeRuleTraceErrorV1::Unsupported(reason) => ExactFreshMatchReplayError::Failure(
            format!("checked rule mechanism trace invariant failed: {reason:?}"),
        ),
        error => ExactFreshMatchReplayError::Failure(format!(
            "cannot finish checked rule mechanism trace: {error:?}"
        )),
    }
}

fn classify_checked_trace_finish_error(
    error: CheckedRuntimeTraceErrorV1,
) -> ExactFreshMatchReplayError {
    match error {
        CheckedRuntimeTraceErrorV1::Unsupported(reason) if reason.is_observation_unsupported() => {
            ExactFreshMatchReplayError::ObservationUnsupported(format!(
                "checked nested mechanism trace is unsupported: {reason:?}"
            ))
        }
        CheckedRuntimeTraceErrorV1::Unsupported(reason) => ExactFreshMatchReplayError::Failure(
            format!("checked nested mechanism trace invariant failed: {reason:?}"),
        ),
        error => ExactFreshMatchReplayError::Failure(format!(
            "cannot finish checked nested mechanism trace: {error:?}"
        )),
    }
}

fn lower_checked_nested_if_trace(
    request: &CheckedMechanismObservationRequestV1,
    expected_plan: &CheckedRuntimeTracePlanV1,
    profile: &CheckedNestedIfProfileV1,
    trace: CheckedRuntimeTraceV1,
) -> Result<DynamicEndpointTraceV1, String> {
    if trace.analysis_program != request.observation.analysis_program {
        return Err("checked nested trace belongs to another analysis program".to_string());
    }
    if &trace.analysis_program != expected_plan.analysis_program() {
        return Err("checked nested trace disagrees with its endpoint trace plan".to_string());
    }
    if &trace.endpoint_root != expected_plan.endpoint_root() {
        return Err("checked nested trace returned another endpoint root".to_string());
    }
    if &trace.implicit_root_callable != expected_plan.implicit_root_callable()
        || trace.implicit_root_callable != profile.root_callable
    {
        return Err("checked nested trace returned another implicit root callable".to_string());
    }
    let [activation] = trace.event.activation_path.as_ref() else {
        return Err(
            "checked nested trace must contain exactly one helper activation frame".to_string(),
        );
    };
    if activation.call_site != profile.nested_call_site
        || activation.callable != profile.nested_callable
        || activation.invocation_ordinal != 0
    {
        return Err(
            "checked nested trace helper activation disagrees with its checked profile".to_string(),
        );
    }
    if trace.event.site != profile.if_site || trace.event.visit_ordinal != 0 {
        return Err(
            "checked nested trace event disagrees with its checked single-if profile".to_string(),
        );
    }

    let activation_call_site = MechanismSiteId::from_expression_site(&activation.call_site)
        .map_err(|error| error.to_string())?;
    let activation_callable = MechanismCallableSiteId::function(
        MechanismSiteId::from_callable(&trace.analysis_program, &activation.callable)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let activation = MechanismActivationStepV1::new(
        &request.observation,
        activation_call_site,
        activation_callable,
        activation.invocation_ordinal,
    )
    .map_err(|error| error.to_string())?;
    let slot = MechanismOccurrenceSlotV1::new(
        &request.observation,
        0,
        vec![activation],
        MechanismSiteId::from_expression_site(&trace.event.site)
            .map_err(|error| error.to_string())?,
        DynamicEventKind::IfDecision,
        trace.event.visit_ordinal,
    )
    .map_err(|error| error.to_string())?;
    let decision = match trace.event.decision {
        CheckedRuntimeIfDecisionV1::Then => IfDecisionOutcome::Then,
        CheckedRuntimeIfDecisionV1::Else => IfDecisionOutcome::Else,
    };
    DynamicEndpointTraceV1::new(
        &request.observation,
        BTreeSet::from([slot.clone()]),
        [EndpointOccurrenceV1::new(
            slot,
            DynamicEventOutcome::IfDecision(decision),
            BTreeSet::new(),
        )],
    )
    .map_err(|error| error.to_string())
}

fn lower_checked_rule_dispatch_trace(
    request: &CheckedMechanismObservationRequestV1,
    expected_plan: &CheckedRuntimeRuleTracePlanV1,
    trace: CheckedRuntimeRuleTraceV1,
) -> Result<DynamicEndpointTraceV1, String> {
    if trace.analysis_program != request.observation.analysis_program
        || &trace.analysis_program != expected_plan.analysis_program()
    {
        return Err("checked rule trace belongs to another analysis program".to_string());
    }
    if &trace.endpoint_root != expected_plan.endpoint_root()
        || &trace.family != expected_plan.family()
    {
        return Err("checked rule trace disagrees with its endpoint trace plan".to_string());
    }
    if trace.attempts.is_empty()
        || trace.attempts.len() > expected_plan.candidates().len()
        || trace
            .attempts
            .iter()
            .zip(expected_plan.candidates())
            .any(|(actual, expected)| &actual.candidate != expected || actual.visit_ordinal != 0)
    {
        return Err("checked rule trace returned an invalid candidate prefix".to_string());
    }

    let mut occurrences = Vec::with_capacity(trace.attempts.len() + 1);
    let mut previous_attempt = None;
    for attempt in trace.attempts.iter() {
        let slot = MechanismOccurrenceSlotV1::new(
            &request.observation,
            0,
            Vec::<MechanismActivationStepV1>::new(),
            MechanismSiteId::from_rule_candidate(&trace.analysis_program, &attempt.candidate)
                .map_err(|error| error.to_string())?,
            DynamicEventKind::RuleAttempt,
            attempt.visit_ordinal,
        )
        .map_err(|error| error.to_string())?;
        let outcome = match attempt.outcome {
            CheckedRuntimeRuleAttemptOutcomeV1::HeadMismatch => RuleAttemptOutcome::HeadMismatch,
            CheckedRuntimeRuleAttemptOutcomeV1::GuardFalse => RuleAttemptOutcome::GuardFalse,
            CheckedRuntimeRuleAttemptOutcomeV1::BodyFalse => RuleAttemptOutcome::BodyFalse,
            CheckedRuntimeRuleAttemptOutcomeV1::Applicable => RuleAttemptOutcome::Applicable,
        };
        let dependencies = previous_attempt.iter().cloned().collect::<BTreeSet<_>>();
        occurrences.push(EndpointOccurrenceV1::new(
            slot.clone(),
            DynamicEventOutcome::RuleAttempt(outcome),
            dependencies,
        ));
        previous_attempt = Some(slot);
    }
    let last_attempt = previous_attempt
        .ok_or_else(|| "checked rule trace has no terminal candidate attempt".to_string())?;
    let selection_slot = MechanismOccurrenceSlotV1::new(
        &request.observation,
        0,
        Vec::<MechanismActivationStepV1>::new(),
        MechanismSiteId::from_rule_family(&trace.analysis_program, &trace.family)
            .map_err(|error| error.to_string())?,
        DynamicEventKind::RuleSelection,
        0,
    )
    .map_err(|error| error.to_string())?;
    let selection = match trace.selection {
        CheckedRuntimeRuleSelectionV1::NoApplicableRule => {
            if trace.attempts.len() != expected_plan.candidates().len() {
                return Err(
                    "no-applicable rule trace did not attempt the complete checked family"
                        .to_string(),
                );
            }
            RuleSelectionOutcome::NoApplicableRule
        }
        CheckedRuntimeRuleSelectionV1::Selected(candidate) => {
            if trace.attempts.last().is_none_or(|attempt| {
                attempt.candidate != candidate
                    || attempt.outcome != CheckedRuntimeRuleAttemptOutcomeV1::Applicable
            }) {
                return Err(
                    "selected rule trace does not end in its applicable candidate".to_string(),
                );
            }
            RuleSelectionOutcome::Selected(
                MechanismSiteId::from_rule_candidate(&trace.analysis_program, &candidate)
                    .map_err(|error| error.to_string())?,
            )
        }
    };
    occurrences.push(EndpointOccurrenceV1::new(
        selection_slot.clone(),
        DynamicEventOutcome::RuleSelection(selection),
        BTreeSet::from([last_attempt]),
    ));
    DynamicEndpointTraceV1::new(
        &request.observation,
        BTreeSet::from([selection_slot]),
        occurrences,
    )
    .map_err(|error| error.to_string())
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
) -> Result<RuntimeConfirmedMechanismObservationV1, MechanismRuntimeMintErrorV1> {
    if !evaluator.mechanism_runtime_root_authorized() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "single-if mechanism evaluator lacks an immutable root-only runtime authorization"
                .to_string(),
        ));
    }
    if &plan.request.observation.query != evaluator.checked_mechanism_query_id() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "single-if mechanism plan and exact evaluator name different checked queries"
                .to_string(),
        ));
    }
    let ordinals = evaluator.canonical_ordinals_for_rank(rank)?;
    let case_id = MechanismCanonicalCaseIdV1::new(rank, ordinals);
    let mut observer = SingleIfReplayObserver::new(plan);
    let replay = evaluator.fresh_replay_confirmed_match_shows(rank, &mut observer);

    match replay {
        Ok(()) => {
            let (before, after, bin_assignments) = observer.finish()?;
            Ok(observed_token(
                plan,
                case_id,
                before,
                after,
                bin_assignments,
            )?)
        }
        Err(ExactFreshMatchReplayError::ReplayUnavailable(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ReplayUnavailable,
        )),
        Err(ExactFreshMatchReplayError::ObservationUnsupported(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ObservationUnsupported,
        )),
        Err(ExactFreshMatchReplayError::NotConfirmedMatch) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "mechanism replay rank {rank} no longer fresh-replays as an admissible match"
            )))
        }
        Err(ExactFreshMatchReplayError::OperationalLimit(stop)) => {
            Err(MechanismRuntimeMintErrorV1::OperationalLimit(stop))
        }
        Err(ExactFreshMatchReplayError::Failure(message)) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "cannot fresh-replay mechanism rank {rank}: {message}"
            )))
        }
    }
}

pub(super) fn mint_nested_if_mechanism_observation_v1(
    plan: &CheckedNestedIfMechanismRuntimePlanV1,
    evaluator: &ExactStreamEvaluator<'_>,
    rank: u128,
) -> Result<RuntimeConfirmedMechanismObservationV1, MechanismRuntimeMintErrorV1> {
    if !evaluator.mechanism_runtime_root_authorized() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "nested-if mechanism evaluator lacks an immutable root-only runtime authorization"
                .to_string(),
        ));
    }
    if &plan.request.observation.query != evaluator.checked_mechanism_query_id() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "nested-if mechanism plan and exact evaluator name different checked queries"
                .to_string(),
        ));
    }
    let ordinals = evaluator.canonical_ordinals_for_rank(rank)?;
    let case_id = MechanismCanonicalCaseIdV1::new(rank, ordinals);
    let mut observer = NestedIfReplayObserver::new(plan);
    let replay = evaluator.fresh_replay_confirmed_match_shows(rank, &mut observer);

    match replay {
        Ok(()) => {
            let (before, after) = observer.finish()?;
            let before_trace = lower_checked_nested_if_trace(
                &plan.request,
                &plan.before_trace,
                &plan.profile,
                before,
            )?;
            let after_trace = lower_checked_nested_if_trace(
                &plan.request,
                &plan.after_trace,
                &plan.profile,
                after,
            )?;
            Ok(observed_trace_token(
                &plan.request,
                case_id,
                before_trace,
                after_trace,
                Box::default(),
            )?)
        }
        Err(ExactFreshMatchReplayError::ReplayUnavailable(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ReplayUnavailable,
        )),
        Err(ExactFreshMatchReplayError::ObservationUnsupported(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ObservationUnsupported,
        )),
        Err(ExactFreshMatchReplayError::NotConfirmedMatch) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "nested mechanism replay rank {rank} no longer fresh-replays as an admissible match"
            )))
        }
        Err(ExactFreshMatchReplayError::OperationalLimit(stop)) => {
            Err(MechanismRuntimeMintErrorV1::OperationalLimit(stop))
        }
        Err(ExactFreshMatchReplayError::Failure(message)) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "cannot fresh-replay nested mechanism rank {rank}: {message}"
            )))
        }
    }
}

pub(super) fn mint_rule_dispatch_mechanism_observation_v1(
    plan: &CheckedRuleDispatchMechanismRuntimePlanV1,
    evaluator: &ExactStreamEvaluator<'_>,
    rank: u128,
) -> Result<RuntimeConfirmedMechanismObservationV1, MechanismRuntimeMintErrorV1> {
    if !evaluator.mechanism_runtime_root_authorized() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "rule-dispatch mechanism evaluator lacks an immutable root-only runtime authorization"
                .to_string(),
        ));
    }
    if &plan.request.observation.query != evaluator.checked_mechanism_query_id() {
        return Err(MechanismRuntimeMintErrorV1::Failure(
            "rule-dispatch mechanism plan and exact evaluator name different checked queries"
                .to_string(),
        ));
    }
    let ordinals = evaluator.canonical_ordinals_for_rank(rank)?;
    let case_id = MechanismCanonicalCaseIdV1::new(rank, ordinals);
    let mut observer = RuleDispatchReplayObserver::new(plan);
    let replay = evaluator.fresh_replay_confirmed_match_shows(rank, &mut observer);

    match replay {
        Ok(()) => {
            let (before, after) = observer.finish()?;
            let before_trace =
                lower_checked_rule_dispatch_trace(&plan.request, &plan.before_trace, before)?;
            let after_trace =
                lower_checked_rule_dispatch_trace(&plan.request, &plan.after_trace, after)?;
            Ok(observed_trace_token(
                &plan.request,
                case_id,
                before_trace,
                after_trace,
                Box::default(),
            )?)
        }
        Err(ExactFreshMatchReplayError::ReplayUnavailable(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ReplayUnavailable,
        )),
        Err(ExactFreshMatchReplayError::ObservationUnsupported(_)) => Ok(untraced_token(
            &plan.request,
            case_id,
            MechanismPermanentUntracedReasonV1::ObservationUnsupported,
        )),
        Err(ExactFreshMatchReplayError::NotConfirmedMatch) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "rule mechanism replay rank {rank} no longer fresh-replays as an admissible match"
            )))
        }
        Err(ExactFreshMatchReplayError::OperationalLimit(stop)) => {
            Err(MechanismRuntimeMintErrorV1::OperationalLimit(stop))
        }
        Err(ExactFreshMatchReplayError::Failure(message)) => {
            Err(MechanismRuntimeMintErrorV1::Failure(format!(
                "cannot fresh-replay rule mechanism rank {rank}: {message}"
            )))
        }
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
        _ => Err(ExactFreshMatchReplayError::Failure(
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
    observed_trace_token(
        &plan.request,
        case_id,
        before_trace,
        after_trace,
        bin_assignments,
    )
}

fn observed_trace_token(
    request: &CheckedMechanismObservationRequestV1,
    case_id: MechanismCanonicalCaseIdV1,
    before_trace: DynamicEndpointTraceV1,
    after_trace: DynamicEndpointTraceV1,
    bin_assignments: Box<[MechanismBinAssignmentV1]>,
) -> Result<RuntimeConfirmedMechanismObservationV1, String> {
    let signature = DynamicMechanismSignature::from_endpoint_traces(
        &request.observation,
        before_trace,
        after_trace,
    )
    .map_err(|error| error.to_string())?;
    let mut interner = CanonicalSignatureInterner::new(&request.observation);
    let signature_id = interner
        .intern(signature)
        .map_err(|error| error.to_string())?;
    let definitions = interner
        .into_signatures()
        .into_values()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let receipt = replay_receipt(
        request,
        &case_id,
        Some(&signature_id),
        &bin_assignments,
        None,
    );
    Ok(RuntimeConfirmedMechanismObservationV1 {
        checked_request_id: request.id.clone(),
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
    request: &CheckedMechanismObservationRequestV1,
    case_id: MechanismCanonicalCaseIdV1,
    reason: MechanismPermanentUntracedReasonV1,
) -> RuntimeConfirmedMechanismObservationV1 {
    let receipt = replay_receipt(request, &case_id, None, &[], Some(reason));
    RuntimeConfirmedMechanismObservationV1 {
        checked_request_id: request.id.clone(),
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
    request: &CheckedMechanismObservationRequestV1,
    confirmed: RuntimeConfirmedMechanismObservationV1,
) -> Result<ValidatedMechanismObservationBatchV1, String> {
    if confirmed.checked_request_id != request.id {
        return Err(
            "runtime mechanism observation token belongs to another checked request".to_string(),
        );
    }
    let proposal = MechanismObservationBatchProposalV1::new(
        request,
        confirmed.definitions,
        vec![confirmed.observation],
    )
    .map_err(|error| error.to_string())?;
    let expected = proposal.clone();
    seal_fresh_replay_confirmed_mechanism_batch_v1(request, proposal, move |candidate| {
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

        assert!(matches!(
            classify_checked_trace_finish_error(CheckedRuntimeTraceErrorV1::Unsupported(
                crate::CheckedRuntimeTraceUnsupportedV1::NamedArguments,
            )),
            ExactFreshMatchReplayError::ObservationUnsupported(_)
        ));
        assert!(matches!(
            classify_checked_trace_finish_error(CheckedRuntimeTraceErrorV1::Unsupported(
                crate::CheckedRuntimeTraceUnsupportedV1::RuntimeCallableMismatch,
            )),
            ExactFreshMatchReplayError::Failure(_)
        ));

        let unit = Expr::unspanned(ExprKind::Unit);
        let endpoint = CheckedEndpointReplayV1 {
            endpoint: unit.clone(),
            arguments: Box::default(),
        };
        assert!(matches!(
            replay_endpoint(
                &mut Interpreter::new(),
                &Env::new(),
                &endpoint,
                &[],
                &unit,
                "checked non-Bool condition",
                10,
                10,
            ),
            Err(ExactFreshMatchReplayError::Failure(message))
                if message.contains("did not evaluate to Bool")
        ));
    }
}
