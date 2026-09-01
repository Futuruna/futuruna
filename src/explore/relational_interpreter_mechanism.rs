//! Checked-interpreter adapter for relational mechanism replay.
//!
//! Every uncached endpoint command constructs a fresh ground evaluator, closes
//! the immutable bindings needed by the checked observer callable, and
//! evaluates that callable under the interpreter's exact-exploration contract.
//! One complete immutable proposal may be reused for an identical checked
//! observation/state/context tuple; mutable evaluator state is never reused.
//! The raw interpreter trace retains producer-owned checked identities; this
//! module only projects those identities into the relational replay ABI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::relational_mechanism_executor::{
    RelationalIfDecisionOutcome, RelationalMechanismActivationPathId,
    RelationalMechanismActivationPathNode, RelationalMechanismActivationStep,
    RelationalMechanismCalleeId, RelationalMechanismEndpointReplayProgress,
    RelationalMechanismEndpointReplayRequest, RelationalMechanismEndpointTraceProposal,
    RelationalMechanismEventKind, RelationalMechanismEventOutcome, RelationalMechanismOccurrenceId,
    RelationalMechanismOccurrenceProposal, RelationalMechanismPermanentUnavailable,
    RelationalMechanismReplayError, RelationalMechanismReplayObservationId,
    RelationalMechanismReplayRuntime, RelationalMechanismSiteId, RelationalRuleAttemptOutcome,
    RelationalRuleSelectionOutcome, RelationalShortCircuitOutcome,
};
use super::transition::canonical_explore_value_digest;
use super::{
    append_required_binding, collect_ground_bindings_inner, expression_query_dependencies,
    runtime_value_from_explore_value, ExploreRuntimeGroundEvaluator, GroundDefinitions,
    EXPLORE_GROUND_COLLECTION_LIMIT,
};
use crate::{
    AnalysisProgramId, CheckedAnalysisProgram, CheckedCallableId, CheckedExploreAnalysisIdentity,
    CheckedExploreQueryView, CheckedInterpreterIfDecisionOutcome,
    CheckedInterpreterMechanismActivation, CheckedInterpreterMechanismCallee,
    CheckedInterpreterMechanismCatalog, CheckedInterpreterMechanismEvaluationError,
    CheckedInterpreterMechanismEventKind, CheckedInterpreterMechanismEventOutcome,
    CheckedInterpreterMechanismEventSite, CheckedInterpreterMechanismTrace,
    CheckedInterpreterMechanismTraceError, CheckedInterpreterRuleAttemptOutcome,
    CheckedInterpreterRuleSelectionOutcome, CheckedInterpreterShortCircuitOutcome,
    CheckedMechanismRuleMemoPlan, ExploreRuntimeFailure, ModuleId, SourcedImportKind, SourcedStmt,
    Stmt, TypeCheckArtifacts,
};

// A fixed capability of replay ABI v3, not a caller-selected slice/resource
// budget. Crossing it is a typed unavailable terminal: calling the same ABI
// again at the same ceiling would otherwise strand the stream forever.
const RELATIONAL_INTERPRETER_MECHANISM_STEP_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RelationalInterpreterEndpointCacheKey {
    observation_id: RelationalMechanismReplayObservationId,
    state_value_digest: [u8; 32],
    context_value_digest: [u8; 32],
}

/// Production fresh-replay adapter backed by one coherent checked program.
///
/// The catalog and declaration graph are immutable and may be shared between
/// commands. Interpreter state, top-level binding values, endpoint arguments,
/// budgets, output buffers, and trace stacks are deliberately not shared. The
/// one-entry proposal cache retains only completed projected evidence.
pub(crate) struct RelationalInterpreterMechanismReplayRuntime {
    catalog: Arc<CheckedInterpreterMechanismCatalog>,
    definitions: Arc<GroundDefinitions>,
    mechanism_memo_plan: Option<CheckedMechanismRuleMemoPlan>,
    required_binding_orders: BTreeMap<CheckedCallableId, Box<[String]>>,
    /// Exactly one immutable proposal, sufficient for adjacent grid edges:
    /// one edge's After is the next edge's Before. This is deliberately not a
    /// population-sized memo and never retains mutable evaluator state.
    last_complete_endpoint: Option<(
        RelationalInterpreterEndpointCacheKey,
        RelationalMechanismEndpointTraceProposal,
    )>,
    step_limit: usize,
    collection_limit: usize,
}

impl RelationalInterpreterMechanismReplayRuntime {
    /// Build from the artifact-owned runtime snapshot, never from a caller's
    /// later mutable statement buffer.
    pub(crate) fn from_checked_artifacts(
        artifacts: &TypeCheckArtifacts,
        checked: &CheckedExploreQueryView<'_>,
    ) -> Result<Self, RelationalInterpreterMechanismReplayError> {
        artifacts
            .checked_runtime_root_program_v1()
            .map_err(RelationalInterpreterMechanismReplayError::CheckedRuntimeSnapshot)?;
        let mut definitions = checked_ground_definitions(&artifacts.analysis_program)?;
        definitions.rule_dispatch_return_types = artifacts.rule_dispatch_return_types.clone();
        definitions.rule_dispatch_return_issues = artifacts.rule_dispatch_return_issues.clone();
        definitions.rule_dispatch_boolean_miss_safe_keys =
            artifacts.rule_dispatch_boolean_miss_safe_keys.clone();
        let mechanism_memo_plan = artifacts.checked_mechanism_rule_memo_plan(checked);
        Self::from_checked_definitions(
            artifacts,
            checked,
            Arc::new(definitions),
            mechanism_memo_plan,
        )
    }

    /// Parent-module integration seam for callers that already hold the exact
    /// collected declaration graph used by relational elaboration.
    pub(super) fn from_checked_definitions(
        artifacts: &TypeCheckArtifacts,
        checked: &CheckedExploreQueryView<'_>,
        definitions: Arc<GroundDefinitions>,
        mechanism_memo_plan: Option<CheckedMechanismRuleMemoPlan>,
    ) -> Result<Self, RelationalInterpreterMechanismReplayError> {
        if checked.artifact.identity.analysis_program != artifacts.analysis_program.id {
            return Err(
                RelationalInterpreterMechanismReplayError::CheckedTraceCatalog(
                    CheckedInterpreterMechanismTraceError::CatalogScopeMismatch,
                ),
            );
        }
        let roots = checked
            .analysis_nodes()
            .filter_map(|(_, identity)| match identity {
                CheckedExploreAnalysisIdentity::Mechanisms { observation, .. } => {
                    Some((&observation.template_site, &observation.endpoint_template))
                }
                CheckedExploreAnalysisIdentity::View { .. } => None,
            });
        let catalog = CheckedInterpreterMechanismCatalog::from_checked(
            &artifacts.analysis_program,
            &artifacts.checked_resolutions,
            roots,
        )
        .map_err(RelationalInterpreterMechanismReplayError::CheckedTraceCatalog)?;
        Ok(Self {
            catalog: Arc::new(catalog),
            definitions,
            mechanism_memo_plan,
            required_binding_orders: BTreeMap::new(),
            last_complete_endpoint: None,
            step_limit: RELATIONAL_INTERPRETER_MECHANISM_STEP_LIMIT,
            collection_limit: EXPLORE_GROUND_COLLECTION_LIMIT as usize,
        })
    }

    fn required_binding_order(
        &mut self,
        callable: &CheckedCallableId,
    ) -> Result<Box<[String]>, RelationalInterpreterMechanismReplayError> {
        if let Some(order) = self.required_binding_orders.get(callable) {
            return Ok(order.clone());
        }
        let checked = self.catalog.callable(callable).ok_or_else(|| {
            RelationalInterpreterMechanismReplayError::CheckedTraceCatalog(
                CheckedInterpreterMechanismTraceError::CheckedCallableMissing(callable.clone()),
            )
        })?;
        let names = self
            .definitions
            .bindings
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let roots = expression_query_dependencies(checked.body(), &names, &self.definitions);
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        for root in roots {
            append_required_binding(
                &root,
                &names,
                &self.definitions,
                &mut visiting,
                &mut visited,
                &mut order,
            )
            .map_err(RelationalInterpreterMechanismReplayError::GroundBinding)?;
        }
        let order = order.into_boxed_slice();
        self.required_binding_orders
            .insert(callable.clone(), order.clone());
        Ok(order)
    }

    fn replay_checked_endpoint(
        &mut self,
        request: RelationalMechanismEndpointReplayRequest<'_>,
    ) -> Result<RelationalMechanismEndpointReplayProgress, RelationalInterpreterMechanismReplayError>
    {
        let observation = request.observation();
        let cache_key = RelationalInterpreterEndpointCacheKey {
            observation_id: request.observation_id(),
            state_value_digest: canonical_explore_value_digest(request.state()),
            context_value_digest: canonical_explore_value_digest(request.context()),
        };
        if let Some((retained_key, retained)) = &self.last_complete_endpoint {
            if *retained_key == cache_key {
                return Ok(RelationalMechanismEndpointReplayProgress::Complete(
                    retained.clone(),
                ));
            }
        }
        let binding_order = self.required_binding_order(&observation.endpoint_template)?;

        // Fresh construction is the isolation boundary. In particular, Before
        // cannot leave memoized rule values, mutable scope state, output, or a
        // partial trace for After (or for another case).
        let mut evaluator = ExploreRuntimeGroundEvaluator::new(&self.definitions);
        if let Some(plan) = self.mechanism_memo_plan.clone() {
            evaluator
                .interpreter
                .install_checked_mechanism_rule_memo_plan(plan)
                .map_err(RelationalInterpreterMechanismReplayError::CheckedMemoPlan)?;
        }
        evaluator
            .evaluate_required_bindings(&binding_order)
            .map_err(RelationalInterpreterMechanismReplayError::GroundBinding)?;
        let base_env = evaluator.base_env.clone();
        let evaluation = evaluator.interpreter.eval_checked_mechanism_endpoint(
            Arc::clone(&self.catalog),
            &observation.template_site,
            &observation.endpoint_template,
            runtime_value_from_explore_value(request.state()),
            runtime_value_from_explore_value(request.context()),
            request.state_type(),
            request.context_type(),
            request.observation_type(),
            &base_env,
            self.step_limit,
            self.collection_limit,
        );
        let progress = match evaluation {
            Ok(trace) => project_checked_trace(&observation.template_site.analysis_program, trace)
                .map(RelationalMechanismEndpointReplayProgress::Complete),
            Err(error) => {
                if std::env::var_os("FUTURUNA_EXPLORE_TRACE").is_some() {
                    eprintln!(
                        "Explore mechanism replay unavailable: endpoint={:?}; case={:?}; error={error}",
                        request.endpoint(),
                        request.case_id(),
                    );
                }
                match permanent_unavailability(&error) {
                    Some(reason) => Ok(
                        RelationalMechanismEndpointReplayProgress::PermanentlyUnavailable(reason),
                    ),
                    None => Err(RelationalInterpreterMechanismReplayError::Endpoint(error)),
                }
            }
        }?;
        if let RelationalMechanismEndpointReplayProgress::Complete(proposal) = &progress {
            self.last_complete_endpoint = Some((cache_key, proposal.clone()));
        }
        Ok(progress)
    }
}

/// Reconstruct the interpreter declaration graph from the exact syntax owned
/// by Phase A. Plain/hash imports are already flattened into this sequence;
/// no replay-time filesystem import is permitted to replace a checked body.
pub(super) fn checked_ground_definitions(
    program: &CheckedAnalysisProgram,
) -> Result<GroundDefinitions, RelationalInterpreterMechanismReplayError> {
    let root_modules = program
        .declarations
        .iter()
        .filter(|declaration| matches!(&declaration.import_kind, SourcedImportKind::Root))
        .map(|declaration| declaration.id.module.clone())
        .collect::<BTreeSet<_>>();
    if root_modules.len() != 1 {
        return Err(
            RelationalInterpreterMechanismReplayError::CheckedRuntimeSnapshot(
                "checked analysis does not identify exactly one root module".into(),
            ),
        );
    }
    let root_module = root_modules.iter().next().expect("checked one root module");

    let mut groups = Vec::<(String, Vec<&Stmt>)>::new();
    for declaration in program.declarations.iter() {
        let origin = checked_declaration_origin(declaration, root_module);
        if let Some((active_origin, statements)) = groups.last_mut() {
            if active_origin == &origin {
                statements.push(declaration.statement.as_ref());
                continue;
            }
        }
        groups.push((origin, vec![declaration.statement.as_ref()]));
    }

    let mut definitions = GroundDefinitions::default();
    let mut visited = BTreeSet::new();
    let mut errors = Vec::new();
    for (origin, statements) in groups {
        collect_ground_bindings_inner(
            &statements,
            None,
            &origin,
            &mut visited,
            &mut definitions,
            &mut errors,
        );
    }
    if errors.is_empty() {
        Ok(definitions)
    } else {
        Err(RelationalInterpreterMechanismReplayError::GroundDefinitions(errors.into_boxed_slice()))
    }
}

fn checked_declaration_origin(declaration: &SourcedStmt, root_module: &ModuleId) -> String {
    if &declaration.id.module == root_module {
        return "<root>".into();
    }
    let import_kind = match &declaration.import_kind {
        SourcedImportKind::Root => "root",
        SourcedImportKind::PlainImport => "plain",
        SourcedImportKind::HashImport { selected_hash } => selected_hash.as_ref(),
        SourcedImportKind::QualifiedImport { module_name } => module_name.as_ref(),
    };
    let internal_path = declaration.id.module.internal_path.join("/");
    format!(
        "<checked:{import_kind}:{}:{internal_path}>",
        declaration.id.module.content_hash
    )
}

impl RelationalMechanismReplayRuntime for RelationalInterpreterMechanismReplayRuntime {
    type Error = RelationalInterpreterMechanismReplayError;

    fn replay_fresh_endpoint(
        &mut self,
        request: RelationalMechanismEndpointReplayRequest<'_>,
    ) -> Result<RelationalMechanismEndpointReplayProgress, Self::Error> {
        self.replay_checked_endpoint(request)
    }
}

fn permanent_unavailability(
    error: &CheckedInterpreterMechanismEvaluationError,
) -> Option<RelationalMechanismPermanentUnavailable> {
    match error {
        CheckedInterpreterMechanismEvaluationError::Runtime(
            ExploreRuntimeFailure::RuntimeError { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::ObserverEvaluationFailed),
        CheckedInterpreterMechanismEvaluationError::Runtime(
            ExploreRuntimeFailure::OperationalLimit { .. },
        )
        | CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::TraceCapacity { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::ReplayAbiCapacityExceeded),
        CheckedInterpreterMechanismEvaluationError::Runtime(
            ExploreRuntimeFailure::UnsupportedCapability { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::ObservationInstrumentationUnsupported),
        CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::CheckedCallableHasEffects(_)
            | CheckedInterpreterMechanismTraceError::CheckedCallableArity { .. },
        ) => Some(RelationalMechanismPermanentUnavailable::CheckedCallableNotReplayable),
        CheckedInterpreterMechanismEvaluationError::Trace(
            CheckedInterpreterMechanismTraceError::RuntimeExpressionMissing { .. }
            | CheckedInterpreterMechanismTraceError::AmbiguousRuntimeExpression { .. }
            | CheckedInterpreterMechanismTraceError::ScopedCallableMissing { .. }
            | CheckedInterpreterMechanismTraceError::AmbiguousScopedCallable { .. }
            | CheckedInterpreterMechanismTraceError::ActivationTargetMismatch
            | CheckedInterpreterMechanismTraceError::MemoizedRuleSelectionMismatch
            | CheckedInterpreterMechanismTraceError::CallableBodyMismatch(_),
        ) => Some(RelationalMechanismPermanentUnavailable::ObservationInstrumentationUnsupported),
        _ => None,
    }
}

fn project_checked_trace(
    analysis_program: &AnalysisProgramId,
    trace: CheckedInterpreterMechanismTrace,
) -> Result<RelationalMechanismEndpointTraceProposal, RelationalInterpreterMechanismReplayError> {
    let activation_paths = trace
        .activation_paths
        .iter()
        .map(|path| {
            let parent = path
                .parent
                .map(|parent| {
                    RelationalMechanismActivationPathId::from_index(parent.index())
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)
                })
                .transpose()?;
            Ok(RelationalMechanismActivationPathNode::new(
                parent,
                project_activation(analysis_program, &path.activation)?,
            ))
        })
        .collect::<Result<Vec<_>, RelationalInterpreterMechanismReplayError>>()?;

    let occurrences = trace
        .events
        .iter()
        .map(|event| {
            let dependencies = event
                .dependencies
                .iter()
                .map(|index| {
                    RelationalMechanismOccurrenceId::from_index(*index)
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(RelationalMechanismOccurrenceProposal::new(
                event.root_index,
                RelationalMechanismActivationPathId::from_index(event.activation_path.index())
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
                project_event_site(analysis_program, &event.site)?,
                project_event_kind(event.kind),
                event.visit_ordinal,
                project_outcome(analysis_program, &event.outcome)?,
                dependencies.into_boxed_slice(),
            ))
        })
        .collect::<Result<Vec<_>, RelationalInterpreterMechanismReplayError>>()?;

    let roots = trace
        .roots
        .iter()
        .map(|index| {
            RelationalMechanismOccurrenceId::from_index(*index)
                .map_err(RelationalInterpreterMechanismReplayError::Projection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelationalMechanismEndpointTraceProposal::new(
        activation_paths.into_boxed_slice(),
        roots.into_boxed_slice(),
        occurrences.into_boxed_slice(),
    ))
}

fn project_activation(
    analysis_program: &AnalysisProgramId,
    activation: &CheckedInterpreterMechanismActivation,
) -> Result<RelationalMechanismActivationStep, RelationalInterpreterMechanismReplayError> {
    let call_site = RelationalMechanismSiteId::from_checked_expression(&activation.call_site)
        .map_err(RelationalInterpreterMechanismReplayError::Projection)?;
    let callee = match &activation.callee {
        CheckedInterpreterMechanismCallee::Function(callable) => {
            RelationalMechanismCalleeId::function(
                RelationalMechanismSiteId::from_checked_callable(analysis_program, callable)
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
            )
        }
        CheckedInterpreterMechanismCallee::RuleFamily(family) => {
            RelationalMechanismCalleeId::rule_family(
                RelationalMechanismSiteId::from_checked_rule_family(analysis_program, family)
                    .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
            )
        }
    }
    .map_err(RelationalInterpreterMechanismReplayError::Projection)?;
    RelationalMechanismActivationStep::new(call_site, callee, activation.invocation_ordinal)
        .map_err(RelationalInterpreterMechanismReplayError::Projection)
}

fn project_event_site(
    analysis_program: &AnalysisProgramId,
    site: &CheckedInterpreterMechanismEventSite,
) -> Result<RelationalMechanismSiteId, RelationalInterpreterMechanismReplayError> {
    match site {
        CheckedInterpreterMechanismEventSite::Expression(site) => {
            RelationalMechanismSiteId::from_checked_expression(site)
        }
        CheckedInterpreterMechanismEventSite::RuleFamily(family) => {
            RelationalMechanismSiteId::from_checked_rule_family(analysis_program, family)
        }
        CheckedInterpreterMechanismEventSite::RuleCandidate(candidate) => {
            RelationalMechanismSiteId::from_checked_rule_candidate(analysis_program, candidate)
        }
    }
    .map_err(RelationalInterpreterMechanismReplayError::Projection)
}

const fn project_event_kind(
    kind: CheckedInterpreterMechanismEventKind,
) -> RelationalMechanismEventKind {
    match kind {
        CheckedInterpreterMechanismEventKind::RuleAttempt => {
            RelationalMechanismEventKind::RuleAttempt
        }
        CheckedInterpreterMechanismEventKind::RuleSelection => {
            RelationalMechanismEventKind::RuleSelection
        }
        CheckedInterpreterMechanismEventKind::IfDecision => {
            RelationalMechanismEventKind::IfDecision
        }
        CheckedInterpreterMechanismEventKind::MatchDecision => {
            RelationalMechanismEventKind::MatchDecision
        }
        CheckedInterpreterMechanismEventKind::ShortCircuitAnd => {
            RelationalMechanismEventKind::ShortCircuitAnd
        }
        CheckedInterpreterMechanismEventKind::ShortCircuitOr => {
            RelationalMechanismEventKind::ShortCircuitOr
        }
    }
}

fn project_outcome(
    analysis_program: &AnalysisProgramId,
    outcome: &CheckedInterpreterMechanismEventOutcome,
) -> Result<RelationalMechanismEventOutcome, RelationalInterpreterMechanismReplayError> {
    Ok(match outcome {
        CheckedInterpreterMechanismEventOutcome::RuleAttempt(outcome) => {
            RelationalMechanismEventOutcome::RuleAttempt(match outcome {
                CheckedInterpreterRuleAttemptOutcome::HeadMismatch => {
                    RelationalRuleAttemptOutcome::HeadMismatch
                }
                CheckedInterpreterRuleAttemptOutcome::GuardFalse => {
                    RelationalRuleAttemptOutcome::GuardFalse
                }
                CheckedInterpreterRuleAttemptOutcome::BodyFalse => {
                    RelationalRuleAttemptOutcome::BodyFalse
                }
                CheckedInterpreterRuleAttemptOutcome::Applicable => {
                    RelationalRuleAttemptOutcome::Applicable
                }
            })
        }
        CheckedInterpreterMechanismEventOutcome::RuleSelection(outcome) => {
            RelationalMechanismEventOutcome::RuleSelection(match outcome {
                CheckedInterpreterRuleSelectionOutcome::NoApplicableRule => {
                    RelationalRuleSelectionOutcome::NoApplicableRule
                }
                CheckedInterpreterRuleSelectionOutcome::Selected(candidate) => {
                    RelationalRuleSelectionOutcome::Selected(
                        RelationalMechanismSiteId::from_checked_rule_candidate(
                            analysis_program,
                            candidate,
                        )
                        .map_err(RelationalInterpreterMechanismReplayError::Projection)?,
                    )
                }
            })
        }
        CheckedInterpreterMechanismEventOutcome::IfDecision(outcome) => {
            RelationalMechanismEventOutcome::IfDecision(match outcome {
                CheckedInterpreterIfDecisionOutcome::Then => RelationalIfDecisionOutcome::Then,
                CheckedInterpreterIfDecisionOutcome::Else => RelationalIfDecisionOutcome::Else,
            })
        }
        CheckedInterpreterMechanismEventOutcome::MatchDecision { arm_index } => {
            RelationalMechanismEventOutcome::MatchDecision {
                arm_index: *arm_index,
            }
        }
        CheckedInterpreterMechanismEventOutcome::ShortCircuit(outcome) => {
            RelationalMechanismEventOutcome::ShortCircuit(match outcome {
                CheckedInterpreterShortCircuitOutcome::SkippedRight { result } => {
                    RelationalShortCircuitOutcome::SkippedRight { result: *result }
                }
                CheckedInterpreterShortCircuitOutcome::EvaluatedRight { result } => {
                    RelationalShortCircuitOutcome::EvaluatedRight { result: *result }
                }
            })
        }
    })
}

#[derive(Debug)]
pub(crate) enum RelationalInterpreterMechanismReplayError {
    CheckedRuntimeSnapshot(String),
    CheckedMemoPlan(String),
    GroundDefinitions(Box<[String]>),
    CheckedTraceCatalog(CheckedInterpreterMechanismTraceError),
    GroundBinding(String),
    Endpoint(CheckedInterpreterMechanismEvaluationError),
    Projection(RelationalMechanismReplayError),
    TraceIndex {
        relation: &'static str,
        event_index: usize,
        referenced_index: usize,
        event_count: usize,
    },
}

impl fmt::Display for RelationalInterpreterMechanismReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedRuntimeSnapshot(error) => write!(
                formatter,
                "checked mechanism runtime snapshot is unavailable: {error}"
            ),
            Self::CheckedMemoPlan(error) => write!(
                formatter,
                "checked mechanism rule memo plan is unavailable: {error}"
            ),
            Self::GroundDefinitions(errors) => write!(
                formatter,
                "checked mechanism declaration graph is not executable: {}",
                errors.join("; ")
            ),
            Self::CheckedTraceCatalog(error) => {
                write!(
                    formatter,
                    "checked mechanism trace catalog is invalid: {error}"
                )
            }
            Self::GroundBinding(error) => write!(
                formatter,
                "checked mechanism immutable binding evaluation failed: {error}"
            ),
            Self::Endpoint(error) => {
                write!(
                    formatter,
                    "checked mechanism endpoint evaluation failed: {error}"
                )
            }
            Self::Projection(error) => {
                write!(
                    formatter,
                    "checked mechanism trace projection failed: {error}"
                )
            }
            Self::TraceIndex {
                relation,
                event_index,
                referenced_index,
                event_count,
            } => write!(
                formatter,
                "checked mechanism event {event_index} has {relation} index {referenced_index}, but the trace has {event_count} events"
            ),
        }
    }
}

impl std::error::Error for RelationalInterpreterMechanismReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CheckedTraceCatalog(error) => Some(error),
            Self::Endpoint(error) => Some(error),
            Self::CheckedRuntimeSnapshot(_)
            | Self::CheckedMemoPlan(_)
            | Self::GroundDefinitions(_)
            | Self::GroundBinding(_)
            | Self::Projection(_)
            | Self::TraceIndex { .. } => None,
        }
    }
}
