//! Reference exact-finite execution for a checked bounded Explore query.
//!
//! This module deliberately consumes [`ExploreQueryIr`], not source syntax. It
//! records one exact case classification for every visited assignment and only
//! publishes values produced by a fresh ordinary-runtime replay. The baseline
//! streams canonical domain order; the candidate-first seam may reorder dense
//! boundary singletons but leaves the declared universe and evaluator
//! unchanged.

use super::boundary_plan::BoundaryInterval;
use super::boundary_search::{
    BoundarySearchCandidate, BoundarySearchCost, BoundarySearchStep, CandidateFirstBoundarySearch,
};
use super::case_graph::{
    CaseDecisionDag, CaseGraphBuilder, CaseOpenReason, CaseTerminal, CheckedCardinality,
    OrderedDecisionDag,
};
use super::classification_regions::ClassificationRegionCertificate;
use super::exact_stream::{
    encode_exact_case_observation_v2, seal_evaluator_confirmed_canonical_observation_batch,
    ExactCanonicalCaseIdV1, ExactCaseObservationBatchProposalV2, ExactCaseObservationProposalV2,
    ExactCaseOutcomeV2, ExactConstructibleCaseOutcomeV2, ExactMatchProjectionV1,
    ExactValidationReceiptDigestV1, ValidatedExactCaseObservationBatchV2,
};
use super::report::{
    ExploreCaseId, ExploreClosure, ExploreCompletionMethod, ExploreCount, ExploreCounts,
    ExploreCoverage, ExploreEvaluationPhase, ExploreExactEvidence, ExploreExactOutcome,
    ExploreExactReport, ExploreExecutionBudget, ExploreExtremaSummary, ExploreGroupCounts,
    ExploreGroupFilter, ExploreLayerClosures, ExploreLedgerEvidence, ExploreLedgerRequest,
    ExploreLedgerRow, ExploreLimitResource, ExploreReportDimension, ExploreReportRequest,
    ExploreReportSchema, ExploreResultKey, ExploreResultRow, ExploreSearchDecisionDagEvidence,
    ExploreSearchDecisionDagRequest, ExploreSearchEvidence, ExploreSemanticTransitionGraphRequest,
    ExploreStopReason,
};
use super::source_events::{SourceEventExtraction, SourceEventLabel};
use super::source_proof_plan::SourceProofPlan;
use super::transition::{TransitionInstance, TransitionSchemaIdentities, TransitionSupportIndex};
use super::*;
use sha2::{Digest, Sha256};

const EXACT_STREAM_EVALUATOR_RECEIPT_V2: &[u8] =
    b"futuruna.explore.exact-transition-evaluator-receipt.v2";

impl ExploreFiniteTypePlan {
    /// Return the inhabitant at `ordinal` in the same canonical order used by
    /// finite-type enumeration, without materializing the surrounding product.
    pub(super) fn exact_value_at(&self, ordinal: u128) -> Result<ExploreValue, String> {
        let cardinality = exact_cardinality(self.cardinality(), "finite type")?;
        if ordinal >= cardinality {
            return Err(format!(
                "finite-type ordinal {} is outside cardinality {}",
                ordinal, cardinality
            ));
        }

        match self {
            Self::Unit => Ok(ExploreValue::Unit),
            Self::Bool => Ok(ExploreValue::Boolean(ordinal == 1)),
            Self::Tuple { elements, .. } => {
                let ordinals =
                    unrank_product(elements.iter().map(|plan| plan.cardinality()), ordinal)?;
                let values = elements
                    .iter()
                    .zip(ordinals)
                    .map(|(plan, ordinal)| plan.exact_value_at(ordinal))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExploreValue::Tuple(values))
            }
            Self::Sum {
                type_name,
                variants,
                ..
            } => {
                let mut remainder = ordinal;
                for variant in variants {
                    let variant_cardinality = product_cardinality(
                        variant.fields.iter().map(|field| field.plan.cardinality()),
                        "finite variant",
                    )?;
                    if remainder >= variant_cardinality {
                        remainder -= variant_cardinality;
                        continue;
                    }

                    let ordinals = unrank_product(
                        variant.fields.iter().map(|field| field.plan.cardinality()),
                        remainder,
                    )?;
                    let fields = variant
                        .fields
                        .iter()
                        .zip(ordinals)
                        .map(|(field, ordinal)| {
                            field
                                .plan
                                .exact_value_at(ordinal)
                                .map(|value| (field.name.clone(), value))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(ExploreValue::Constructor {
                        type_name: type_name.clone(),
                        variant: variant.name.clone(),
                        positional: variant.positional,
                        fields,
                    });
                }

                Err(format!(
                    "finite sum ordinal {} did not select a variant",
                    ordinal
                ))
            }
        }
    }
}

impl ExploreExactDomain {
    /// Decode one canonical domain ordinal without allocating the domain.
    pub(super) fn exact_value_at(&self, ordinal: u128) -> Result<ExploreValue, String> {
        let cardinality = exact_cardinality(self.cardinality(), "exploration domain")?;
        if ordinal >= cardinality {
            return Err(format!(
                "exploration-domain ordinal {} is outside cardinality {}",
                ordinal, cardinality
            ));
        }

        match self {
            Self::Enumerated { values, .. } => {
                let index = usize::try_from(ordinal)
                    .map_err(|_| format!("domain ordinal {} does not fit usize", ordinal))?;
                values
                    .get(index)
                    .cloned()
                    .ok_or_else(|| format!("domain ordinal {} is absent", ordinal))
            }
            Self::IntRange { start, .. } => {
                let value = i128::from(*start)
                    .checked_add(i128::try_from(ordinal).map_err(|_| {
                        format!("integer-range ordinal {} does not fit i128", ordinal)
                    })?)
                    .ok_or_else(|| "integer-range value overflow".to_string())?;
                i64::try_from(value)
                    .map(ExploreValue::Int)
                    .map_err(|_| format!("integer-range value {} does not fit Int", value))
            }
            Self::FiniteType { plan, .. } => plan.exact_value_at(ordinal),
        }
    }

    /// Boundary axes are statically restricted to `Int`. This membership test
    /// stays lazy for ranges and never widens a sparse enumerated domain.
    fn contains_boundary_int(&self, value: i64) -> Result<bool, String> {
        match self {
            Self::Enumerated { values, .. } => Ok(values
                .iter()
                .any(|candidate| candidate.int() == Some(value))),
            Self::IntRange {
                start,
                end_exclusive,
                ..
            } => Ok(value >= *start && value < *end_exclusive),
            Self::FiniteType { .. } => Err(
                "an exploration boundary axis cannot use a finite non-Int type domain".to_string(),
            ),
        }
    }
}

fn exact_cardinality(cardinality: ExploreCardinality, subject: &str) -> Result<u128, String> {
    cardinality
        .exact()
        .ok_or_else(|| format!("{} cardinality exceeds u128::MAX", subject))
}

fn product_cardinality(
    cardinalities: impl IntoIterator<Item = ExploreCardinality>,
    subject: &str,
) -> Result<u128, String> {
    let cardinalities = cardinalities.into_iter().collect::<Vec<_>>();
    // Zero annihilates the whole product even when an earlier prefix would
    // overflow u128.  Inspect the complete shape before multiplying so an
    // empty finite variant is not misreported as unsupported merely because
    // its fields happen to be declared in an unfortunate order.
    if cardinalities
        .iter()
        .any(|cardinality| cardinality.exact() == Some(0))
    {
        return Ok(0);
    }

    let mut product = 1_u128;
    for cardinality in cardinalities {
        let cardinality = exact_cardinality(cardinality, subject)?;
        product = product
            .checked_mul(cardinality)
            .ok_or_else(|| format!("{} cardinality exceeds u128::MAX", subject))?;
    }
    Ok(product)
}

/// Unrank a lexicographic product whose rightmost coordinate changes fastest.
fn unrank_product(
    cardinalities: impl IntoIterator<Item = ExploreCardinality>,
    ordinal: u128,
) -> Result<Vec<u128>, String> {
    let cardinalities = cardinalities.into_iter().collect::<Vec<_>>();
    // Keep unranking consistent with product cardinality: a zero coordinate
    // makes the product empty before any other coordinate can overflow.
    if cardinalities
        .iter()
        .any(|cardinality| cardinality.exact() == Some(0))
    {
        return Err(format!(
            "finite-product ordinal {} is outside cardinality 0",
            ordinal
        ));
    }
    let cardinalities = cardinalities
        .into_iter()
        .map(|cardinality| exact_cardinality(cardinality, "finite product"))
        .collect::<Result<Vec<_>, _>>()?;
    let product = cardinalities
        .iter()
        .try_fold(1_u128, |product, cardinality| {
            product
                .checked_mul(*cardinality)
                .ok_or_else(|| "finite product cardinality exceeds u128::MAX".to_string())
        })?;
    if ordinal >= product {
        return Err(format!(
            "finite-product ordinal {} is outside cardinality {}",
            ordinal, product
        ));
    }

    let mut remainder = ordinal;
    let mut ordinals = vec![0_u128; cardinalities.len()];
    for (index, cardinality) in cardinalities.iter().enumerate().rev() {
        // A zero-cardinality product has no valid ordinal, so the range check
        // above returns before this division.
        ordinals[index] = remainder % cardinality;
        remainder /= cardinality;
    }
    Ok(ordinals)
}

/// Canonical mixed-radix cursor. A query with no varied dimensions has one
/// empty assignment; any zero-cardinality dimension makes the product empty.
struct CanonicalAssignmentCursor {
    cardinalities: Box<[u128]>,
    current: Box<[u128]>,
    first: bool,
    exhausted: bool,
}

impl CanonicalAssignmentCursor {
    fn new(cardinalities: Box<[u128]>) -> Self {
        let exhausted = cardinalities.iter().any(|cardinality| *cardinality == 0);
        let current = vec![0; cardinalities.len()].into_boxed_slice();
        Self {
            cardinalities,
            current,
            first: true,
            exhausted,
        }
    }
}

impl Iterator for CanonicalAssignmentCursor {
    type Item = Box<[u128]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }
        if self.first {
            self.first = false;
            return Some(self.current.clone());
        }

        for index in (0..self.current.len()).rev() {
            let next = self.current[index] + 1;
            if next < self.cardinalities[index] {
                self.current[index] = next;
                return Some(self.current.clone());
            }
            self.current[index] = 0;
        }

        // The zero-dimensional product yielded its single empty assignment on
        // the first call and also arrives here.
        self.exhausted = true;
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OrdinalCaseId(Box<[u128]>);

impl OrdinalCaseId {
    fn assignment(&self, query: &ExploreQueryIr) -> Result<Vec<ExploreValue>, String> {
        if self.0.len() != query.universe.dimensions.len() {
            return Err(format!(
                "case identity has {} ordinals for {} dimensions",
                self.0.len(),
                query.universe.dimensions.len()
            ));
        }
        query
            .universe
            .dimensions
            .iter()
            .zip(self.0.iter().copied())
            .map(|(dimension, ordinal)| dimension.domain.exact_value_at(ordinal))
            .collect()
    }
}

fn query_axis_cardinalities(query: &ExploreQueryIr) -> Result<Box<[u128]>, String> {
    query
        .universe
        .dimensions
        .iter()
        .map(|dimension| {
            exact_cardinality(
                dimension.domain.cardinality(),
                &format!("exploration dimension `{}`", dimension.name),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn assignment_values(
    query: &ExploreQueryIr,
    ordinals: &[u128],
) -> Result<Vec<ExploreValue>, String> {
    OrdinalCaseId(ordinals.to_vec().into_boxed_slice()).assignment(query)
}

#[derive(Debug)]
enum ExactEngineFailure {
    OperationalLimit(ExploreStopReason),
    Unsupported(String),
    Error(String),
}

impl ExactEngineFailure {
    fn contextualize(self, context: &str) -> Self {
        match self {
            Self::OperationalLimit(stop) => Self::OperationalLimit(stop),
            Self::Unsupported(message) => Self::Unsupported(format!("while {context}: {message}")),
            Self::Error(message) => Self::Error(format!("while {context}: {message}")),
        }
    }
}

fn runtime_failure(
    failure: ExploreRuntimeFailure,
    phase: ExploreEvaluationPhase,
) -> ExactEngineFailure {
    match failure {
        ExploreRuntimeFailure::OperationalLimit {
            resource,
            limit,
            observed,
        } => {
            let resource = match resource {
                ExploreRuntimeResource::InitializationSteps
                | ExploreRuntimeResource::ExpressionSteps => ExploreLimitResource::Steps,
                ExploreRuntimeResource::CollectionMembers { operation } => {
                    ExploreLimitResource::CollectionMembers { operation }
                }
            };
            ExactEngineFailure::OperationalLimit(ExploreStopReason::RuntimeLimit {
                resource,
                limit,
                observed,
                phase,
            })
        }
        ExploreRuntimeFailure::UnsupportedCapability { message } => {
            ExactEngineFailure::Unsupported(message)
        }
        ExploreRuntimeFailure::ProducedOutput => ExactEngineFailure::Unsupported(
            "exact exploration evaluation attempted to produce observable output".to_string(),
        ),
        ExploreRuntimeFailure::RuntimeError { message } => ExactEngineFailure::Error(message),
        ExploreRuntimeFailure::Panicked => {
            ExactEngineFailure::Error("exact exploration evaluation panicked".to_string())
        }
    }
}

struct ExactRuntime {
    interpreter: Interpreter,
    base_env: Env,
}

struct ExactRuntimeContext<'a> {
    statements: &'a [Stmt],
    source_dir: Option<&'a str>,
    artifacts: &'a TypeCheckArtifacts,
    catalog: &'a calculate::TypeCatalog,
    roots: &'a BTreeSet<ExploreRuntimeRoot>,
    step_limit: usize,
    collection_limit: usize,
    phase_override: Option<ExploreEvaluationPhase>,
}

impl ExactRuntimeContext<'_> {
    fn phase(&self, phase: ExploreEvaluationPhase) -> ExploreEvaluationPhase {
        self.phase_override.clone().unwrap_or(phase)
    }

    fn for_replay(&self) -> Self {
        Self {
            statements: self.statements,
            source_dir: self.source_dir,
            artifacts: self.artifacts,
            catalog: self.catalog,
            roots: self.roots,
            step_limit: self.step_limit,
            collection_limit: self.collection_limit,
            phase_override: Some(ExploreEvaluationPhase::Replay),
        }
    }

    /// Construct a new interpreter and dependency-closed declaration
    /// environment. This is called once for enumeration and once for each
    /// publicly exposed case replay; runtime state is never shared between
    /// those phases.
    fn fresh(&self) -> Result<ExactRuntime, ExactEngineFailure> {
        let mut interpreter = Interpreter::new();
        interpreter.suppress_output = true;
        interpreter.install_rule_dispatch_metadata(self.artifacts);
        interpreter.source_dir = self.source_dir.map(str::to_string);
        let mut base_env = interpreter.default_env();
        interpreter
            .initialize_exploration_program(
                self.roots,
                self.statements,
                &mut base_env,
                self.step_limit,
                self.collection_limit,
            )
            .map_err(|failure| {
                runtime_failure(failure, self.phase(ExploreEvaluationPhase::Initialization))
            })
            .map_err(|failure| failure.contextualize("initializing the Explore runtime"))?;
        Ok(ExactRuntime {
            interpreter,
            base_env,
        })
    }
}

impl ExactRuntime {
    fn eval_value(
        &mut self,
        expression: &Expr,
        env: &Env,
        expected: &Ty,
        catalog: &calculate::TypeCatalog,
        step_limit: usize,
        collection_limit: usize,
        context: &str,
        phase: ExploreEvaluationPhase,
    ) -> Result<ExploreValue, ExactEngineFailure> {
        let runtime = self
            .interpreter
            .eval_exact_exploration(expression, env, step_limit, collection_limit)
            .map_err(|failure| runtime_failure(failure, phase))
            .map_err(|failure| failure.contextualize(context))?;
        runtime_value_to_explore_value(&runtime, expected, catalog)
            .map_err(|message| ExactEngineFailure::Error(format!("while {context}: {message}")))
    }

    fn eval_bool(
        &mut self,
        expression: &Expr,
        env: &Env,
        catalog: &calculate::TypeCatalog,
        step_limit: usize,
        collection_limit: usize,
        context: &str,
        phase: ExploreEvaluationPhase,
    ) -> Result<bool, ExactEngineFailure> {
        let value = self.eval_value(
            expression,
            env,
            &Ty::Name("Bool".to_string()),
            catalog,
            step_limit,
            collection_limit,
            context,
            phase,
        )?;
        match value {
            ExploreValue::Boolean(value) => Ok(value),
            other => Err(ExactEngineFailure::Error(format!(
                "while {context}: expected Bool, received canonical value {other:?}"
            ))),
        }
    }
}

fn required_runtime_roots(query: &ExploreQueryIr) -> BTreeSet<ExploreRuntimeRoot> {
    let mut roots = BTreeSet::from([ExploreRuntimeRoot::Rule {
        name: query.query.rule_name.clone(),
        arity: query.query.rule_arity,
    }]);
    let mut bound = query
        .query
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .collect::<BTreeSet<_>>();
    let mut typed_receivers = query
        .query
        .inputs
        .iter()
        .filter_map(|input| type_name(&input.ty).map(|ty| (input.name.clone(), ty.to_string())))
        .collect::<BTreeMap<_, _>>();

    // Only normalized compact aliases introduce bare generator names into the
    // runtime environment. Explicit State/Context field labels are not
    // bindings and therefore must not shadow or suppress same-named roots.
    for alias in &query.transition.flat_aliases {
        let ExploreFlatAliasSource::Dimension { dimension_index } = alias.source else {
            continue;
        };
        let Some(dimension) = query.universe.dimensions.get(dimension_index) else {
            continue;
        };
        bound.insert(alias.name.clone());
        if let Some(ty) = type_name(&dimension.value_ty) {
            typed_receivers.insert(alias.name.clone(), ty.to_string());
        }
    }

    for (fact_index, fact) in query.universe.facts.iter().enumerate() {
        if let ExploreFactValue::Derived { expression, .. } = &fact.value {
            collect_typed_explore_runtime_roots(expression, &mut roots, &bound, &typed_receivers);
        }
        for alias in &query.transition.flat_aliases {
            if alias.source != (ExploreFlatAliasSource::Fact { fact_index }) {
                continue;
            }
            bound.insert(alias.name.clone());
            if let Some(ty) = type_name(&fact.value_ty) {
                typed_receivers.insert(alias.name.clone(), ty.to_string());
            }
        }
    }
    for constraint in &query.universe.constraints {
        collect_typed_explore_runtime_roots(
            &constraint.predicate,
            &mut roots,
            &bound,
            &typed_receivers,
        );
    }
    for schema in [
        &query.transition.state_schema,
        &query.transition.context_schema,
    ] {
        for field in &schema.fields {
            if let ExploreProductFieldSourceIr::TransitionExpression { expression } = &field.source
            {
                collect_typed_explore_runtime_roots(
                    expression,
                    &mut roots,
                    &bound,
                    &typed_receivers,
                );
            }
        }
    }
    for field in &query.transition.after_fields {
        if let ExploreAfterFieldSourceIr::Derived { expression, .. } = &field.source {
            collect_typed_explore_runtime_roots(expression, &mut roots, &bound, &typed_receivers);
        }
    }
    for field in &query.query.output.key {
        collect_typed_explore_runtime_roots(&field.value, &mut roots, &bound, &typed_receivers);
    }
    for field in &query.query.output.extrema {
        collect_typed_explore_runtime_roots(&field.value, &mut roots, &bound, &typed_receivers);
    }
    for field in &query.query.output.extrema {
        bound.insert(field.name.clone());
        if let Some(ty) = type_name(&field.ty) {
            typed_receivers.insert(field.name.clone(), ty.to_string());
        }
    }
    for field in &query.query.output.show {
        collect_typed_explore_runtime_roots(&field.value, &mut roots, &bound, &typed_receivers);
        bound.insert(field.name.clone());
        if let Some(ty) = type_name(&field.ty) {
            typed_receivers.insert(field.name.clone(), ty.to_string());
        }
    }
    match &query.query.output.representative {
        ExploreRepresentative::First { .. } => {}
        ExploreRepresentative::Maximize { objective, .. }
        | ExploreRepresentative::Minimize { objective, .. } => {
            collect_typed_explore_runtime_roots(objective, &mut roots, &bound, &typed_receivers);
        }
    }
    roots
}

fn question_expression(query: &ExploreQueryIr) -> Expr {
    ExprKind::App(
        Box::new(ExprKind::Var(query.query.rule_name.clone()).into()),
        query
            .query
            .inputs
            .iter()
            .map(|input| ExprKind::Var(input.name.clone()).into())
            .collect(),
    )
    .into()
}

fn bind_canonical(env: &mut Env, name: &str, value: &ExploreValue) {
    env.set(name.to_string(), runtime_value_from_explore_value(value));
}

struct ExactSourceConstruction {
    env: Env,
    dimension_values: Vec<ExploreValue>,
    fact_values: Vec<Option<ExploreValue>>,
}

struct ExactProductValue {
    canonical: ExploreValue,
    runtime: Value,
    fields: Vec<ExploreValue>,
}

/// One fully materialized semantic transition. Endpoint-local environments
/// are views of the same canonical values: the transition view retains compact
/// scalar aliases at `before`, while the after view replaces only compact
/// state aliases with their after values.
struct ExactTransitionFrame {
    context: ExploreValue,
    before: ExploreValue,
    after: ExploreValue,
    endpoint_membership: bool,
    transition_env: Env,
    after_endpoint_env: Env,
}

impl ExactTransitionFrame {
    fn transition_env(&self) -> &Env {
        &self.transition_env
    }

    fn after_endpoint_env(&self) -> &Env {
        &self.after_endpoint_env
    }

    fn into_semantic_transition(self, schemas: &TransitionSchemaIdentities) -> TransitionInstance {
        schemas.instantiate(self.context, self.before, self.after)
    }
}

fn bind_flat_aliases_for_source(
    env: &mut Env,
    query: &ExploreQueryIr,
    source: ExploreFlatAliasSource,
    value: &ExploreValue,
) {
    for alias in &query.transition.flat_aliases {
        if alias.source == source {
            bind_canonical(env, &alias.name, value);
        }
    }
}

fn build_source_construction(
    runtime: &ExactRuntime,
    query: &ExploreQueryIr,
    assignment: &[ExploreValue],
) -> Result<ExactSourceConstruction, ExactEngineFailure> {
    if assignment.len() != query.universe.dimensions.len() {
        return Err(ExactEngineFailure::Error(format!(
            "assignment has {} values for {} Explore dimensions",
            assignment.len(),
            query.universe.dimensions.len()
        )));
    }

    let mut env = runtime.base_env.child();
    let dimension_values = assignment.to_vec();
    for (dimension_index, value) in dimension_values.iter().enumerate() {
        bind_flat_aliases_for_source(
            &mut env,
            query,
            ExploreFlatAliasSource::Dimension { dimension_index },
            value,
        );
    }
    Ok(ExactSourceConstruction {
        env,
        dimension_values,
        fact_values: vec![None; query.universe.facts.len()],
    })
}

fn evaluate_fact_phase(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    source: &mut ExactSourceConstruction,
    role: ExploreGeneratorAxisRole,
) -> Result<(), ExactEngineFailure> {
    for (fact_index, fact) in query.universe.facts.iter().enumerate() {
        if fact.role != role {
            continue;
        }
        if fact.role_field_index
            >= match role {
                ExploreGeneratorAxisRole::Context => query.transition.context_schema.fields.len(),
                ExploreGeneratorAxisRole::Before => query.transition.state_schema.fields.len(),
                ExploreGeneratorAxisRole::AfterIndependent => 0,
            }
        {
            return Err(ExactEngineFailure::Error(format!(
                "Explore fact `{}` has invalid {:?} field index {}",
                fact.name, role, fact.role_field_index
            )));
        }
        let value = match &fact.value {
            ExploreFactValue::Fixed(value) => value.clone(),
            ExploreFactValue::Derived { expression, .. } => runtime.eval_value(
                expression,
                &source.env,
                &fact.value_ty,
                runtime_context.catalog,
                runtime_context.step_limit,
                runtime_context.collection_limit,
                &format!("evaluating derived Explore fact `{}`", fact.name),
                runtime_context.phase(ExploreEvaluationPhase::DerivedFact {
                    name: fact.name.clone(),
                }),
            )?,
        };
        bind_flat_aliases_for_source(
            &mut source.env,
            query,
            ExploreFlatAliasSource::Fact { fact_index },
            &value,
        );
        let slot = &mut source.fact_values[fact_index];
        if slot.replace(value).is_some() {
            return Err(ExactEngineFailure::Error(format!(
                "Explore fact `{}` was evaluated more than once",
                fact.name
            )));
        }
    }
    Ok(())
}

fn materialize_product_values(
    schema: &ExploreProductSchemaIr,
    values: Vec<ExploreValue>,
    role: &str,
) -> Result<ExactProductValue, ExactEngineFailure> {
    if values.len() != schema.fields.len() {
        return Err(ExactEngineFailure::Error(format!(
            "{role} value has {} fields for schema width {}",
            values.len(),
            schema.fields.len()
        )));
    }
    let (canonical, runtime) = match &schema.identity {
        TypedExploreProductSchemaIdentity::Unit => {
            if !values.is_empty() {
                return Err(ExactEngineFailure::Error(format!(
                    "unit {role} schema unexpectedly contains {} fields",
                    values.len()
                )));
            }
            (ExploreValue::Unit, Value::Unit)
        }
        TypedExploreProductSchemaIdentity::Synthetic { .. } => (
            ExploreValue::Tuple(values.clone()),
            Value::Tuple(
                values
                    .iter()
                    .map(runtime_value_from_explore_value)
                    .collect(),
            ),
        ),
        TypedExploreProductSchemaIdentity::Declared { ty } => {
            let Ty::Name(type_name) = ty else {
                return Err(ExactEngineFailure::Error(format!(
                    "declared {role} schema `{ty}` is not a nominal product type"
                )));
            };
            (
                ExploreValue::Tuple(values.clone()),
                Value::NamedConstructor(
                    type_name.clone(),
                    schema
                        .fields
                        .iter()
                        .zip(values.iter())
                        .map(|(field, value)| {
                            (field.name.clone(), runtime_value_from_explore_value(value))
                        })
                        .collect::<Vec<_>>()
                        .into(),
                ),
            )
        }
    };
    Ok(ExactProductValue {
        canonical,
        runtime,
        fields: values,
    })
}

/// Build the runtime-only partial `after` product visible while evaluating one
/// derived field. Static dependency checks guarantee that the expression can
/// observe only fields whose values are already present; unit placeholders for
/// other fields are therefore unreachable and never enter canonical state.
fn materialize_partial_after_runtime(
    schema: &ExploreProductSchemaIr,
    values: &[Option<ExploreValue>],
) -> Result<Value, ExactEngineFailure> {
    if values.len() != schema.fields.len() {
        return Err(ExactEngineFailure::Error(format!(
            "partial after value has {} fields for schema width {}",
            values.len(),
            schema.fields.len()
        )));
    }
    let runtime_value = |value: &Option<ExploreValue>| {
        value
            .as_ref()
            .map(runtime_value_from_explore_value)
            .unwrap_or(Value::Unit)
    };
    match &schema.identity {
        TypedExploreProductSchemaIdentity::Unit => Ok(Value::Unit),
        TypedExploreProductSchemaIdentity::Synthetic { .. } => {
            Ok(Value::Tuple(values.iter().map(runtime_value).collect()))
        }
        TypedExploreProductSchemaIdentity::Declared { ty } => {
            let Ty::Name(type_name) = ty else {
                return Err(ExactEngineFailure::Error(format!(
                    "declared partial after schema `{ty}` is not a nominal product type"
                )));
            };
            Ok(Value::NamedConstructor(
                type_name.clone(),
                schema
                    .fields
                    .iter()
                    .zip(values)
                    .map(|(field, value)| (field.name.clone(), runtime_value(value)))
                    .collect::<Vec<_>>()
                    .into(),
            ))
        }
    }
}

fn build_product_value(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    schema: &ExploreProductSchemaIr,
    source: &ExactSourceConstruction,
    role: &str,
) -> Result<ExactProductValue, ExactEngineFailure> {
    let mut values = Vec::with_capacity(schema.fields.len());
    for field in &schema.fields {
        if field.field_index != values.len() {
            return Err(ExactEngineFailure::Error(format!(
                "{role} field `{}` has index {}, expected {}",
                field.name,
                field.field_index,
                values.len()
            )));
        }
        let value = match &field.source {
            ExploreProductFieldSourceIr::Dimension { dimension_index } => source
                .dimension_values
                .get(*dimension_index)
                .cloned()
                .ok_or_else(|| {
                    ExactEngineFailure::Error(format!(
                        "{role} field `{}` references absent dimension {}",
                        field.name, dimension_index
                    ))
                })?,
            ExploreProductFieldSourceIr::Fact { fact_index } => source
                .fact_values
                .get(*fact_index)
                .and_then(Option::as_ref)
                .cloned()
                .ok_or_else(|| {
                    ExactEngineFailure::Error(format!(
                        "{role} field `{}` references absent fact {}",
                        field.name, fact_index
                    ))
                })?,
            ExploreProductFieldSourceIr::TransitionExpression { expression } => runtime
                .eval_value(
                    expression,
                    &source.env,
                    &field.value_ty,
                    runtime_context.catalog,
                    runtime_context.step_limit,
                    runtime_context.collection_limit,
                    &format!("constructing {role} field `{}`", field.name),
                    runtime_context.phase(ExploreEvaluationPhase::DerivedFact {
                        name: field.name.clone(),
                    }),
                )?,
        };
        values.push(value);
    }

    materialize_product_values(schema, values, role)
}

fn build_after_values(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    source: &ExactSourceConstruction,
    before_values: &[ExploreValue],
    construction_env: &Env,
) -> Result<(Vec<ExploreValue>, bool), ExactEngineFailure> {
    let mut after_values = vec![None; query.transition.after_fields.len()];
    let mut pending_derived = BTreeSet::new();
    for (expected_index, field) in query.transition.after_fields.iter().enumerate() {
        if field.field_index != expected_index {
            return Err(ExactEngineFailure::Error(format!(
                "after field `{}` has index {}, expected {}",
                field.name, field.field_index, expected_index
            )));
        }
        let value = match &field.source {
            ExploreAfterFieldSourceIr::FrameBefore { before_field_index } => {
                let before_field = query
                    .transition
                    .state_schema
                    .fields
                    .get(*before_field_index)
                    .ok_or_else(|| {
                        ExactEngineFailure::Error(format!(
                            "after field `{}` frames absent before field {}",
                            field.name, before_field_index
                        ))
                    })?;
                if before_field.name != field.name
                    || !crate::checked_ty_structurally_equal(
                        &before_field.value_ty,
                        &field.value_ty,
                    )
                {
                    return Err(ExactEngineFailure::Error(format!(
                        "after field `{}` does not exactly frame before field {}",
                        field.name, before_field_index
                    )));
                }
                Some(
                    before_values
                        .get(*before_field_index)
                        .cloned()
                        .ok_or_else(|| {
                            ExactEngineFailure::Error(format!(
                                "after field `{}` frames absent before value {}",
                                field.name, before_field_index
                            ))
                        })?,
                )
            }
            ExploreAfterFieldSourceIr::Derived { .. } => {
                pending_derived.insert(field.field_index);
                None
            }
            ExploreAfterFieldSourceIr::IndependentDomain { dimension_index } => {
                let dimension =
                    query
                        .universe
                        .dimensions
                        .get(*dimension_index)
                        .ok_or_else(|| {
                            ExactEngineFailure::Error(format!(
                            "independent after field `{}` references absent generator dimension {}",
                            field.name, dimension_index
                        ))
                        })?;
                if dimension.role != ExploreGeneratorAxisRole::AfterIndependent
                    || dimension.role_field_index != field.field_index
                {
                    return Err(ExactEngineFailure::Error(format!(
                        "independent after field `{}` does not own generator dimension {}",
                        field.name, dimension_index
                    )));
                }
                let value = source
                    .dimension_values
                    .get(*dimension_index)
                    .cloned()
                    .ok_or_else(|| {
                        ExactEngineFailure::Error(format!(
                            "independent after field `{}` references absent dimension {}",
                            field.name, dimension_index
                        ))
                    })?;
                Some(value)
            }
        };
        after_values[field.field_index] = value;
    }

    while !pending_derived.is_empty() {
        let ready = pending_derived.iter().copied().find(|field_index| {
            let Some(field) = query.transition.after_fields.get(*field_index) else {
                return false;
            };
            let ExploreAfterFieldSourceIr::Derived {
                after_dependencies, ..
            } = &field.source
            else {
                return false;
            };
            after_dependencies.iter().all(|dependency| {
                dependency.field_index < after_values.len()
                    && after_values[dependency.field_index].is_some()
            })
        });
        let Some(field_index) = ready else {
            return Err(ExactEngineFailure::Error(format!(
                "after-construction dependency graph is cyclic or references an absent field: {:?}",
                pending_derived
            )));
        };
        let field = &query.transition.after_fields[field_index];
        let ExploreAfterFieldSourceIr::Derived {
            expression,
            after_dependencies,
            ..
        } = &field.source
        else {
            unreachable!("pending after-construction node must be derived")
        };
        if after_dependencies
            .iter()
            .any(|dependency| dependency.field_index == field_index)
        {
            return Err(ExactEngineFailure::Error(format!(
                "derived after field `{}` depends on itself",
                field.name
            )));
        }
        let mut derived_env = construction_env.child();
        for dependency in after_dependencies {
            let dependency_field = query
                .transition
                .state_schema
                .fields
                .get(dependency.field_index)
                .ok_or_else(|| {
                    ExactEngineFailure::Error(format!(
                        "derived after field `{}` references absent dependency {}",
                        field.name, dependency.field_index
                    ))
                })?;
            if dependency.binding_name != dependency_field.name {
                return Err(ExactEngineFailure::Error(format!(
                    "derived after dependency binding `{}` disagrees with state field `{}`",
                    dependency.binding_name, dependency_field.name
                )));
            }
            let dependency_value =
                after_values[dependency.field_index]
                    .as_ref()
                    .ok_or_else(|| {
                        ExactEngineFailure::Error(format!(
                            "derived after field `{}` dependency `{}` is not constructed",
                            field.name, dependency_field.name
                        ))
                    })?;
            for alias in &query.transition.flat_aliases {
                if matches!(
                    alias.role,
                    ExploreFlatAliasRole::State { field_index }
                        if field_index == dependency.field_index
                ) {
                    bind_canonical(&mut derived_env, &alias.name, dependency_value);
                }
            }
        }
        derived_env.set(
            "after".to_string(),
            materialize_partial_after_runtime(&query.transition.state_schema, &after_values)?,
        );
        let value = runtime.eval_value(
            expression,
            &derived_env,
            &field.value_ty,
            runtime_context.catalog,
            runtime_context.step_limit,
            runtime_context.collection_limit,
            &format!("constructing derived after field `{}`", field.name),
            runtime_context.phase(ExploreEvaluationPhase::DerivedFact {
                name: field.name.clone(),
            }),
        )?;
        after_values[field_index] = Some(value);
        pending_derived.remove(&field_index);
    }

    let after_values = after_values
        .into_iter()
        .enumerate()
        .map(|(field_index, value)| {
            value.ok_or_else(|| {
                ExactEngineFailure::Error(format!(
                    "after field {} was not constructed",
                    field_index
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut endpoint_membership = true;
    for membership in &query.transition.after_membership {
        let value = after_values
            .get(membership.after_field_index)
            .ok_or_else(|| {
                ExactEngineFailure::Error(format!(
                    "after-membership references absent field {}",
                    membership.after_field_index
                ))
            })?;
        let after = value.int().ok_or_else(|| {
            ExactEngineFailure::Error(format!(
                "after-membership field {} is not an Int",
                membership.after_field_index
            ))
        })?;
        let before_dimension = query
            .universe
            .dimensions
            .get(membership.before_dimension_index)
            .ok_or_else(|| {
                ExactEngineFailure::Error(format!(
                    "after-membership references absent before dimension {}",
                    membership.before_dimension_index
                ))
            })?;
        if before_dimension.role != ExploreGeneratorAxisRole::Before {
            return Err(ExactEngineFailure::Error(format!(
                "after-membership dimension `{}` is not a before-state axis",
                before_dimension.name
            )));
        }
        if !before_dimension
            .domain
            .contains_boundary_int(after)
            .map_err(ExactEngineFailure::Error)?
        {
            endpoint_membership = false;
        }
    }
    Ok((after_values, endpoint_membership))
}

fn bind_compact_after_aliases(
    env: &mut Env,
    query: &ExploreQueryIr,
    after_values: &[ExploreValue],
) -> Result<(), ExactEngineFailure> {
    for alias in &query.transition.flat_aliases {
        let ExploreFlatAliasRole::State { field_index } = alias.role else {
            continue;
        };
        let value = after_values.get(field_index).ok_or_else(|| {
            ExactEngineFailure::Error(format!(
                "compact after alias `{}` references absent state field {}",
                alias.name, field_index
            ))
        })?;
        bind_canonical(env, &alias.name, value);
    }
    Ok(())
}

fn build_transition_frame(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    assignment: &[ExploreValue],
) -> Result<ExactTransitionFrame, ExactEngineFailure> {
    let mut source = build_source_construction(runtime, query, assignment)?;
    evaluate_fact_phase(
        runtime,
        runtime_context,
        query,
        &mut source,
        ExploreGeneratorAxisRole::Context,
    )?;
    let context = build_product_value(
        runtime,
        runtime_context,
        &query.transition.context_schema,
        &source,
        "transition-context",
    )?;
    source
        .env
        .set("context".to_string(), context.runtime.clone());
    evaluate_fact_phase(
        runtime,
        runtime_context,
        query,
        &mut source,
        ExploreGeneratorAxisRole::Before,
    )?;
    let before = build_product_value(
        runtime,
        runtime_context,
        &query.transition.state_schema,
        &source,
        "before-state",
    )?;

    let mut construction_env = source.env.child();
    construction_env.set("before".to_string(), before.runtime.clone());
    let (after_values, endpoint_membership) = build_after_values(
        runtime,
        runtime_context,
        query,
        &source,
        &before.fields,
        &construction_env,
    )?;
    let after = materialize_product_values(
        &query.transition.state_schema,
        after_values.clone(),
        "after-state",
    )?;

    let mut transition_env = construction_env.child();
    transition_env.set("after".to_string(), after.runtime.clone());
    let mut after_endpoint_env = transition_env.child();
    bind_compact_after_aliases(&mut after_endpoint_env, query, &after_values)?;

    Ok(ExactTransitionFrame {
        context: context.canonical,
        before: before.canonical,
        after: after.canonical,
        endpoint_membership,
        transition_env,
        after_endpoint_env,
    })
}

enum Admissibility {
    /// The generator coordinate cannot denote a total typed transition. It is
    /// closed as outside the structural transition universe before any fact
    /// or after-field expression is evaluated, and therefore has no
    /// TransitionId/support edge.
    StructurallyExcluded,
    Excluded(ExactTransitionFrame),
    Admissible(ExactTransitionFrame),
}

pub(super) fn endpoint_memberships_are_structurally_eligible(
    query: &ExploreQueryIr,
    assignment: &[ExploreValue],
) -> Result<bool, String> {
    for membership in &query.transition.after_membership {
        let dimension = query
            .universe
            .dimensions
            .get(membership.before_dimension_index)
            .ok_or_else(|| {
                format!(
                    "endpoint-membership dimension {} is outside {} dimensions",
                    membership.before_dimension_index,
                    query.universe.dimensions.len()
                )
            })?;
        if dimension.role != ExploreGeneratorAxisRole::Before
            || dimension.role_field_index != membership.after_field_index
        {
            return Err(format!(
                "endpoint-membership field {} is not paired with its indexed Before dimension",
                membership.after_field_index
            ));
        }
        let lower = assignment
            .get(membership.before_dimension_index)
            .and_then(ExploreValue::int)
            .ok_or_else(|| {
                format!(
                    "endpoint-membership Before dimension `{}` is not an Int assignment",
                    dimension.name
                )
            })?;
        let after = match membership.preconstruction {
            ExploreAfterMembershipPreconstructionIr::RelativeIntStep { step } => {
                let Some(after) = lower.checked_add(step) else {
                    return Ok(false);
                };
                after
            }
        };
        if !dimension.domain.contains_boundary_int(after)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn evaluate_admissibility(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    assignment: &[ExploreValue],
) -> Result<Admissibility, ExactEngineFailure> {
    // Boundary membership is a structural contract, not a fallible after
    // computation. Close the ineligible coordinate before derived facts so an
    // out-of-domain or overflowing successor cannot consume runtime budget or
    // turn a finite search into an evaluator error.
    if !endpoint_memberships_are_structurally_eligible(query, assignment)
        .map_err(ExactEngineFailure::Error)?
    {
        return Ok(Admissibility::StructurallyExcluded);
    }
    let frame = build_transition_frame(runtime, runtime_context, query, assignment)?;
    if !frame.endpoint_membership {
        return Ok(Admissibility::Excluded(frame));
    }

    for (index, constraint) in query.universe.constraints.iter().enumerate() {
        let context = format!("evaluating Explore `where` constraint {}", index + 1);
        match constraint.scope {
            ExploreConstraintScope::Before => {
                if !runtime.eval_bool(
                    &constraint.predicate,
                    frame.transition_env(),
                    runtime_context.catalog,
                    runtime_context.step_limit,
                    runtime_context.collection_limit,
                    &format!("{context} at the before endpoint"),
                    runtime_context.phase(ExploreEvaluationPhase::Constraint { index }),
                )? {
                    return Ok(Admissibility::Excluded(frame));
                }
            }
            ExploreConstraintScope::After | ExploreConstraintScope::Transition => {
                let environment = match constraint.scope {
                    ExploreConstraintScope::After => frame.after_endpoint_env(),
                    ExploreConstraintScope::Transition => frame.transition_env(),
                    ExploreConstraintScope::Before | ExploreConstraintScope::BothEndpoints => {
                        unreachable!()
                    }
                };
                if !runtime.eval_bool(
                    &constraint.predicate,
                    environment,
                    runtime_context.catalog,
                    runtime_context.step_limit,
                    runtime_context.collection_limit,
                    &match constraint.scope {
                        ExploreConstraintScope::After => {
                            format!("{context} at the after endpoint")
                        }
                        ExploreConstraintScope::Transition => {
                            format!("{context} over the transition")
                        }
                        ExploreConstraintScope::Before | ExploreConstraintScope::BothEndpoints => {
                            unreachable!()
                        }
                    },
                    runtime_context.phase(ExploreEvaluationPhase::Constraint { index }),
                )? {
                    return Ok(Admissibility::Excluded(frame));
                }
            }
            ExploreConstraintScope::BothEndpoints => {
                if !runtime.eval_bool(
                    &constraint.predicate,
                    frame.transition_env(),
                    runtime_context.catalog,
                    runtime_context.step_limit,
                    runtime_context.collection_limit,
                    &format!("{context} at the before endpoint"),
                    runtime_context.phase(ExploreEvaluationPhase::Constraint { index }),
                )? {
                    return Ok(Admissibility::Excluded(frame));
                }
                if !runtime.eval_bool(
                    &constraint.predicate,
                    frame.after_endpoint_env(),
                    runtime_context.catalog,
                    runtime_context.step_limit,
                    runtime_context.collection_limit,
                    &format!("{context} at the upper boundary endpoint"),
                    runtime_context.phase(ExploreEvaluationPhase::Constraint { index }),
                )? {
                    return Ok(Admissibility::Excluded(frame));
                }
            }
        }
    }

    Ok(Admissibility::Admissible(frame))
}

fn evaluate_polarity(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    question: &Expr,
    lower_env: &Env,
) -> Result<bool, ExactEngineFailure> {
    let predicate = runtime.eval_bool(
        question,
        lower_env,
        runtime_context.catalog,
        runtime_context.step_limit,
        runtime_context.collection_limit,
        &format!("evaluating Explore question `{}`", query.query.rule_name),
        runtime_context.phase(ExploreEvaluationPhase::Question),
    )?;
    Ok(match query.query.polarity {
        ExplorePolarity::Matches => predicate,
        ExplorePolarity::Violations => !predicate,
    })
}

fn evaluate_key(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    lower_env: &Env,
) -> Result<Box<[ExploreValue]>, ExactEngineFailure> {
    query
        .query
        .output
        .key
        .iter()
        .map(|field| {
            runtime.eval_value(
                &field.value,
                lower_env,
                &field.ty,
                runtime_context.catalog,
                runtime_context.step_limit,
                runtime_context.collection_limit,
                &format!("evaluating Explore key field `{}`", field.name),
                runtime_context.phase(ExploreEvaluationPhase::Key {
                    name: field.name.clone(),
                }),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn evaluate_extrema_value(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    lower_env: &Env,
    extrema_index: usize,
) -> Result<i64, ExactEngineFailure> {
    let field = query
        .query
        .output
        .extrema
        .get(extrema_index)
        .ok_or_else(|| {
            ExactEngineFailure::Error(format!(
                "Explore extrema index {extrema_index} is outside the checked output schema"
            ))
        })?;
    let value = runtime.eval_value(
        &field.value,
        lower_env,
        &field.ty,
        runtime_context.catalog,
        runtime_context.step_limit,
        runtime_context.collection_limit,
        &format!("evaluating Explore extrema field `{}`", field.name),
        runtime_context.phase(ExploreEvaluationPhase::Extrema {
            name: field.name.clone(),
        }),
    )?;
    value.int().ok_or_else(|| {
        ExactEngineFailure::Error(format!(
            "checked Explore extrema `{}` did not evaluate to Int",
            field.name
        ))
    })
}

fn evaluate_extrema(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    lower_env: &Env,
) -> Result<Box<[i64]>, ExactEngineFailure> {
    (0..query.query.output.extrema.len())
        .map(|extrema_index| {
            evaluate_extrema_value(runtime, runtime_context, query, lower_env, extrema_index)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn evaluate_shown_and_objective(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    lower_env: &Env,
    extrema_values: &[i64],
) -> Result<(Box<[ExploreValue]>, Option<i64>), ExactEngineFailure> {
    let mut output_env = lower_env.child();
    if extrema_values.len() != query.query.output.extrema.len() {
        return Err(ExactEngineFailure::Error(
            "Explore extrema value count disagrees with the checked output schema".to_string(),
        ));
    }
    for (field, value) in query.query.output.extrema.iter().zip(extrema_values) {
        output_env.set(field.name.clone(), Value::Int(*value));
    }
    let mut shown = Vec::with_capacity(query.query.output.show.len());
    for field in &query.query.output.show {
        let value = runtime.eval_value(
            &field.value,
            &output_env,
            &field.ty,
            runtime_context.catalog,
            runtime_context.step_limit,
            runtime_context.collection_limit,
            &format!("evaluating Explore shown field `{}`", field.name),
            runtime_context.phase(ExploreEvaluationPhase::Show {
                name: field.name.clone(),
            }),
        )?;
        bind_canonical(&mut output_env, &field.name, &value);
        shown.push(value);
    }

    let objective = match &query.query.output.representative {
        ExploreRepresentative::First { .. } => None,
        ExploreRepresentative::Maximize { objective, .. }
        | ExploreRepresentative::Minimize { objective, .. } => {
            let objective_ty = query
                .query
                .output
                .representative_ty
                .as_ref()
                .ok_or_else(|| {
                    ExactEngineFailure::Error(
                        "ordered Explore representative has no checked objective type".to_string(),
                    )
                })?;
            let value = runtime.eval_value(
                objective,
                &output_env,
                objective_ty,
                runtime_context.catalog,
                runtime_context.step_limit,
                runtime_context.collection_limit,
                "evaluating the Explore representative objective",
                runtime_context.phase(ExploreEvaluationPhase::Objective),
            )?;
            Some(value.int().ok_or_else(|| {
                ExactEngineFailure::Unsupported(format!(
                    "exact-finite representative objective `{objective_ty}` is not the v1 ordered Int scalar"
                ))
            })?)
        }
    };
    Ok((shown.into_boxed_slice(), objective))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchObservation {
    case_id: OrdinalCaseId,
    key: Box<[ExploreValue]>,
    extrema: Box<[i64]>,
    shown: Box<[ExploreValue]>,
    objective: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ExtremaWitnessExpectation {
    key: Box<[ExploreValue]>,
    extrema_index: usize,
    value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactExtremaAccumulator {
    minimum: i64,
    maximum: i64,
    support: u128,
    minimum_tie_support: u128,
    maximum_tie_support: u128,
    minimum_witness: OrdinalCaseId,
    maximum_witness: OrdinalCaseId,
}

impl ExactExtremaAccumulator {
    fn first(value: i64, witness: &OrdinalCaseId) -> Self {
        Self {
            minimum: value,
            maximum: value,
            support: 1,
            minimum_tie_support: 1,
            maximum_tie_support: 1,
            minimum_witness: witness.clone(),
            maximum_witness: witness.clone(),
        }
    }

    fn check_observe(&self, value: i64) -> Result<(), ExactEngineFailure> {
        incremented_counter(self.support, "extrema support")?;
        if value == self.minimum {
            incremented_counter(self.minimum_tie_support, "extrema minimum-tie support")?;
        }
        if value == self.maximum {
            incremented_counter(self.maximum_tie_support, "extrema maximum-tie support")?;
        }
        Ok(())
    }

    /// Apply an observation after `check_observe` has succeeded for the same
    /// value. The checked additions are assertions of that acceptance
    /// precondition, not new semantic failure points.
    fn observe_prechecked(&mut self, value: i64, witness: &OrdinalCaseId) {
        self.support = self
            .support
            .checked_add(1)
            .expect("prechecked extrema support increment");
        if value < self.minimum {
            self.minimum = value;
            self.minimum_tie_support = 1;
            self.minimum_witness = witness.clone();
        } else if value == self.minimum {
            self.minimum_tie_support = self
                .minimum_tie_support
                .checked_add(1)
                .expect("prechecked extrema minimum-tie support increment");
            if witness < &self.minimum_witness {
                self.minimum_witness = witness.clone();
            }
        }
        if value > self.maximum {
            self.maximum = value;
            self.maximum_tie_support = 1;
            self.maximum_witness = witness.clone();
        } else if value == self.maximum {
            self.maximum_tie_support = self
                .maximum_tie_support
                .checked_add(1)
                .expect("prechecked extrema maximum-tie support increment");
            if witness < &self.maximum_witness {
                self.maximum_witness = witness.clone();
            }
        }
    }

    fn summary(&self) -> ExploreExtremaSummary {
        ExploreExtremaSummary {
            minimum: self.minimum,
            maximum: self.maximum,
            spread: (self.maximum as i128 - self.minimum as i128) as u128,
            minimum_tie_support: self.minimum_tie_support,
            maximum_tie_support: self.maximum_tie_support,
            minimum_witness: ExploreCaseId::new(self.minimum_witness.0.clone()),
            maximum_witness: ExploreCaseId::new(self.maximum_witness.0.clone()),
        }
    }
}

fn candidate_is_better(
    policy: &ExploreRepresentative,
    candidate: &SearchObservation,
    incumbent: &SearchObservation,
) -> Result<bool, ExactEngineFailure> {
    match policy {
        ExploreRepresentative::First { .. } => Ok(candidate.case_id < incumbent.case_id),
        ExploreRepresentative::Maximize { .. } => {
            let candidate_objective = candidate.objective.ok_or_else(|| {
                ExactEngineFailure::Error("maximize candidate has no objective".to_string())
            })?;
            let incumbent_objective = incumbent.objective.ok_or_else(|| {
                ExactEngineFailure::Error("maximize incumbent has no objective".to_string())
            })?;
            Ok(candidate_objective > incumbent_objective
                || (candidate_objective == incumbent_objective
                    && candidate.case_id < incumbent.case_id))
        }
        ExploreRepresentative::Minimize { .. } => {
            let candidate_objective = candidate.objective.ok_or_else(|| {
                ExactEngineFailure::Error("minimize candidate has no objective".to_string())
            })?;
            let incumbent_objective = incumbent.objective.ok_or_else(|| {
                ExactEngineFailure::Error("minimize incumbent has no objective".to_string())
            })?;
            Ok(candidate_objective < incumbent_objective
                || (candidate_objective == incumbent_objective
                    && candidate.case_id < incumbent.case_id))
        }
    }
}

struct ExactSearchState {
    declared: u128,
    admissible: u128,
    matching: u128,
    keys_seen: BTreeSet<Box<[ExploreValue]>>,
    key_supports: BTreeMap<Box<[ExploreValue]>, u128>,
    key_extrema: BTreeMap<Box<[ExploreValue]>, Box<[ExactExtremaAccumulator]>>,
    /// Policy-selected CaseIds are kept independently from materialized shown
    /// values. Canonical-order `first` is prefix-stable; candidate-first
    /// observations retain the least seen CaseId but do not close selection
    /// until the key class closes.
    selected_representatives: BTreeMap<Box<[ExploreValue]>, OrdinalCaseId>,
    candidates: BTreeMap<Box<[ExploreValue]>, SearchObservation>,
    ledger: Vec<SearchObservation>,
    /// Semantic state/edge projection over exactly evaluated singleton cases.
    /// This is distinct from the search-decision case DAG below.
    transition_support: TransitionSupportIndex,
    case_graph: CaseDecisionDag,
    projection_observations_complete: bool,
    representative_selection_observations_complete: bool,
    ledger_observations_complete: bool,
    search_trace: ExactSearchTrace,
    stop: Option<ExploreStopReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactSearchTrace {
    Canonical,
    SourceCandidateFirst(ExploreSearchEvidence),
}

struct ExactAccumulator {
    classified: u128,
    admissible: u128,
    matching: u128,
    keys_seen: BTreeSet<Box<[ExploreValue]>>,
    key_supports: BTreeMap<Box<[ExploreValue]>, u128>,
    key_extrema: BTreeMap<Box<[ExploreValue]>, Box<[ExactExtremaAccumulator]>>,
    selected_representatives: BTreeMap<Box<[ExploreValue]>, OrdinalCaseId>,
    candidates: BTreeMap<Box<[ExploreValue]>, SearchObservation>,
    ledger: Vec<SearchObservation>,
    transition_support: TransitionSupportIndex,
    projection_observations_complete: bool,
    representative_selection_observations_complete: bool,
    ledger_observations_complete: bool,
}

impl ExactAccumulator {
    fn new() -> Self {
        Self {
            classified: 0,
            admissible: 0,
            matching: 0,
            keys_seen: BTreeSet::new(),
            key_supports: BTreeMap::new(),
            key_extrema: BTreeMap::new(),
            selected_representatives: BTreeMap::new(),
            candidates: BTreeMap::new(),
            ledger: Vec::new(),
            transition_support: TransitionSupportIndex::default(),
            projection_observations_complete: true,
            representative_selection_observations_complete: true,
            ledger_observations_complete: true,
        }
    }

    fn finish(
        self,
        declared: u128,
        case_graph: CaseDecisionDag,
        search_trace: ExactSearchTrace,
        stop: Option<ExploreStopReason>,
    ) -> ExactSearchState {
        ExactSearchState {
            declared,
            admissible: self.admissible,
            matching: self.matching,
            keys_seen: self.keys_seen,
            key_supports: self.key_supports,
            key_extrema: self.key_extrema,
            selected_representatives: self.selected_representatives,
            candidates: self.candidates,
            ledger: self.ledger,
            transition_support: self.transition_support,
            case_graph,
            projection_observations_complete: self.projection_observations_complete,
            representative_selection_observations_complete: self
                .representative_selection_observations_complete,
            ledger_observations_complete: self.ledger_observations_complete,
            search_trace,
            stop,
        }
    }
}

struct ObservedCase {
    terminal: CaseTerminal,
    stop: Option<ExploreStopReason>,
}

impl ObservedCase {
    fn is_closed(&self) -> bool {
        matches!(
            &self.terminal,
            CaseTerminal::Excluded
                | CaseTerminal::AdmissibleNonmatch
                | CaseTerminal::AdmissibleMatch
        )
    }
}

fn case_terminal_is_closed(terminal: &CaseTerminal) -> bool {
    matches!(
        terminal,
        CaseTerminal::Excluded | CaseTerminal::AdmissibleNonmatch | CaseTerminal::AdmissibleMatch
    )
}

fn query_name(query: &ExploreQueryIr) -> String {
    query
        .query
        .name
        .clone()
        .unwrap_or_else(|| "<anonymous>".to_string())
}

fn failure_report(
    query: &ExploreQueryIr,
    failure: ExactEngineFailure,
) -> Result<ExploreExactReport, String> {
    let outcome = match failure {
        ExactEngineFailure::OperationalLimit(_) => {
            return Err(
                "internal Explore error: operational limit escaped evidence finalization"
                    .to_string(),
            )
        }
        ExactEngineFailure::Unsupported(diagnostic) => {
            ExploreExactOutcome::Unsupported { diagnostic }
        }
        ExactEngineFailure::Error(diagnostic) => ExploreExactOutcome::Error {
            diagnostics: vec![diagnostic].into_boxed_slice(),
        },
    };
    ExploreExactReport::with_deferred_mechanism(query_name(query), query.query.polarity, outcome)
}

fn report_schema(query: &ExploreQueryIr) -> Result<ExploreReportSchema, String> {
    let group_filter = match &query.query.output.having {
        None => ExploreGroupFilter::All,
        Some(crate::TypedExploreHaving::Varies { extrema_index, .. }) => {
            ExploreGroupFilter::Varies {
                extrema_index: *extrema_index,
            }
        }
    };
    ExploreReportSchema::with_grouped_extrema(
        query
            .universe
            .dimensions
            .iter()
            .map(|dimension| ExploreReportDimension {
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                label: dimension.name.clone(),
            })
            .collect::<Vec<_>>(),
        query_axis_cardinalities(query)?,
        query
            .query
            .output
            .key
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>(),
        query
            .query
            .output
            .extrema
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>(),
        query
            .query
            .output
            .show
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>(),
        group_filter,
    )
}

fn checked_counter_increment(counter: &mut u128, name: &str) -> Result<(), ExactEngineFailure> {
    *counter = counter.checked_add(1).ok_or_else(|| {
        ExactEngineFailure::Error(format!("Explore {name} counter exceeds u128::MAX"))
    })?;
    Ok(())
}

fn incremented_counter(counter: u128, name: &str) -> Result<u128, ExactEngineFailure> {
    let mut next = counter;
    checked_counter_increment(&mut next, name)?;
    Ok(next)
}

enum PreparedGroupExtrema {
    Existing,
    New(Box<[ExactExtremaAccumulator]>),
}

fn prepare_group_extrema(
    accumulators: &BTreeMap<Box<[ExploreValue]>, Box<[ExactExtremaAccumulator]>>,
    key: &[ExploreValue],
    values: &[i64],
    case_id: &OrdinalCaseId,
) -> Result<PreparedGroupExtrema, ExactEngineFailure> {
    if let Some(group) = accumulators.get(key) {
        if group.len() != values.len() {
            return Err(ExactEngineFailure::Error(
                "Explore extrema accumulator width changed within one result key".to_string(),
            ));
        }
        for (accumulator, value) in group.iter().zip(values) {
            accumulator.check_observe(*value)?;
        }
        Ok(PreparedGroupExtrema::Existing)
    } else {
        Ok(PreparedGroupExtrema::New(
            values
                .iter()
                .copied()
                .map(|value| ExactExtremaAccumulator::first(value, case_id))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ))
    }
}

/// A fully evaluated CaseId and its one total semantic transition.
///
/// Classification and transition support are accepted as one transaction.
/// Matching cases distinguish the projection-only `first` fast path from a
/// materialized observation so a ledger request can never be accepted without
/// its complete row.
struct ExactCaseTransaction {
    case_id: OrdinalCaseId,
    transition: TransitionInstance,
    outcome: ExactCaseOutcome,
}

enum ExactCaseOutcome {
    Excluded,
    AdmissibleNonmatch,
    AdmissibleMatch(MatchingCaseTransaction),
}

enum MatchingCaseTransaction {
    ProjectionOnly {
        case_id: OrdinalCaseId,
        key: Box<[ExploreValue]>,
        extrema: Box<[i64]>,
    },
    Materialized {
        observation: SearchObservation,
        retain_ledger: bool,
    },
}

impl MatchingCaseTransaction {
    fn projection(&self) -> (&OrdinalCaseId, &[ExploreValue], &[i64]) {
        match self {
            Self::ProjectionOnly {
                case_id,
                key,
                extrema,
            } => (case_id, key, extrema),
            Self::Materialized { observation, .. } => {
                (&observation.case_id, &observation.key, &observation.extrema)
            }
        }
    }
}

enum ExactCaseEvaluation {
    StructurallyExcluded(OrdinalCaseId),
    Complete(ExactCaseTransaction),
    Open(ObservedCase),
}

fn finish_case_evaluation(
    evaluation: Result<ExactCaseEvaluation, ExactEngineFailure>,
) -> Result<ExactCaseEvaluation, ExactEngineFailure> {
    match evaluation {
        Ok(evaluation) => Ok(evaluation),
        Err(ExactEngineFailure::OperationalLimit(stop)) => {
            Ok(ExactCaseEvaluation::Open(ObservedCase {
                // The durable retry unit is the whole CaseId. Even when an
                // inner phase learned admissibility or polarity, no fragment
                // is accepted until every requested observation is ready.
                terminal: CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
                stop: Some(stop),
            }))
        }
        Err(failure) => Err(failure),
    }
}

/// Evaluate one CaseId without mutating semantic evidence. The accumulator is
/// consulted only to preserve the `first` materialization fast path; every
/// requested field must finish before this function yields a transaction.
fn evaluate_case_transaction(
    accumulator: &ExactAccumulator,
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    transition_schemas: &TransitionSchemaIdentities,
    question: &Expr,
    ordinals: &[u128],
    retain_ledger: bool,
) -> Result<ExactCaseEvaluation, ExactEngineFailure> {
    let evaluation = (|| {
        let assignment = assignment_values(query, ordinals).map_err(ExactEngineFailure::Error)?;
        let case_id = OrdinalCaseId(ordinals.to_vec().into_boxed_slice());
        let frame = match evaluate_admissibility(runtime, runtime_context, query, &assignment)? {
            Admissibility::StructurallyExcluded => {
                return Ok(ExactCaseEvaluation::StructurallyExcluded(case_id))
            }
            Admissibility::Excluded(frame) => {
                return Ok(ExactCaseEvaluation::Complete(ExactCaseTransaction {
                    case_id,
                    transition: frame.into_semantic_transition(transition_schemas),
                    outcome: ExactCaseOutcome::Excluded,
                }))
            }
            Admissibility::Admissible(frame) => frame,
        };
        let transition_env = frame.transition_env();

        if !evaluate_polarity(runtime, runtime_context, query, question, transition_env)? {
            return Ok(ExactCaseEvaluation::Complete(ExactCaseTransaction {
                case_id,
                transition: frame.into_semantic_transition(transition_schemas),
                outcome: ExactCaseOutcome::AdmissibleNonmatch,
            }));
        }

        let key = evaluate_key(runtime, runtime_context, query, transition_env)?;
        let extrema = evaluate_extrema(runtime, runtime_context, query, transition_env)?;
        let is_first = matches!(
            &query.query.output.representative,
            ExploreRepresentative::First { .. }
        );
        let first_replaces = is_first
            && accumulator
                .selected_representatives
                .get(&key)
                .map_or(true, |incumbent| case_id < *incumbent);
        let representative_needs_values = if is_first { first_replaces } else { true };
        let matching = if !retain_ledger && !representative_needs_values {
            MatchingCaseTransaction::ProjectionOnly {
                case_id: case_id.clone(),
                key,
                extrema,
            }
        } else {
            let (shown, objective) = evaluate_shown_and_objective(
                runtime,
                runtime_context,
                query,
                transition_env,
                &extrema,
            )?;
            MatchingCaseTransaction::Materialized {
                observation: SearchObservation {
                    case_id: case_id.clone(),
                    key,
                    extrema,
                    shown,
                    objective,
                },
                retain_ledger,
            }
        };
        Ok(ExactCaseEvaluation::Complete(ExactCaseTransaction {
            case_id,
            transition: frame.into_semantic_transition(transition_schemas),
            outcome: ExactCaseOutcome::AdmissibleMatch(matching),
        }))
    })();
    finish_case_evaluation(evaluation)
}

/// Check a complete case transaction against the current accumulator, then
/// commit it without any remaining fallible semantic work. A rejected
/// transaction therefore leaves the accumulator unchanged as well.
fn accept_case_transaction(
    accumulator: &mut ExactAccumulator,
    representative: &ExploreRepresentative,
    transaction: ExactCaseTransaction,
) -> Result<ObservedCase, ExactEngineFailure> {
    let ExactCaseTransaction {
        case_id,
        transition,
        outcome,
    } = transaction;
    let (admissible_transition, matching_transition) = match &outcome {
        ExactCaseOutcome::Excluded => (false, false),
        ExactCaseOutcome::AdmissibleNonmatch => (true, false),
        ExactCaseOutcome::AdmissibleMatch(_) => (true, true),
    };
    let prepared_transition = accumulator
        .transition_support
        .prepare_intern(transition, admissible_transition, matching_transition)
        .map_err(|error| {
            ExactEngineFailure::Error(format!(
                "cannot accept canonical Explore transition support: {error}"
            ))
        })?;

    let terminal = match outcome {
        ExactCaseOutcome::Excluded => {
            let classified = incremented_counter(accumulator.classified, "classified-case")?;
            accumulator
                .transition_support
                .commit_prepared(prepared_transition);
            accumulator.classified = classified;
            CaseTerminal::Excluded
        }
        ExactCaseOutcome::AdmissibleNonmatch => {
            let classified = incremented_counter(accumulator.classified, "classified-case")?;
            let admissible = incremented_counter(accumulator.admissible, "admissible-case")?;
            accumulator
                .transition_support
                .commit_prepared(prepared_transition);
            accumulator.classified = classified;
            accumulator.admissible = admissible;
            CaseTerminal::AdmissibleNonmatch
        }
        ExactCaseOutcome::AdmissibleMatch(matching) => {
            let (matching_case_id, key, extrema) = matching.projection();
            if matching_case_id != &case_id {
                return Err(ExactEngineFailure::Error(
                    "matching Explore projection belongs to another CaseId".to_string(),
                ));
            }
            let classified = incremented_counter(accumulator.classified, "classified-case")?;
            let admissible = incremented_counter(accumulator.admissible, "admissible-case")?;
            let matching_count = incremented_counter(accumulator.matching, "matching-case")?;
            let support = incremented_counter(
                accumulator.key_supports.get(key).copied().unwrap_or(0),
                "result-key support",
            )?;
            let group_extrema =
                prepare_group_extrema(&accumulator.key_extrema, key, extrema, &case_id)?;
            let replace_representative = match &matching {
                MatchingCaseTransaction::ProjectionOnly { .. } => {
                    if !matches!(representative, ExploreRepresentative::First { .. }) {
                        return Err(ExactEngineFailure::Error(
                            "ordered Explore representative transaction omitted materialized values"
                                .to_string(),
                        ));
                    }
                    if accumulator
                        .selected_representatives
                        .get(key)
                        .is_none_or(|incumbent| &case_id < incumbent)
                    {
                        return Err(ExactEngineFailure::Error(
                            "first Explore representative transaction omitted required materialized values"
                                .to_string(),
                        ));
                    }
                    false
                }
                MatchingCaseTransaction::Materialized { observation, .. } => match representative {
                    ExploreRepresentative::First { .. } => accumulator
                        .selected_representatives
                        .get(key)
                        .is_none_or(|incumbent| &case_id < incumbent),
                    ExploreRepresentative::Maximize { .. }
                    | ExploreRepresentative::Minimize { .. } => accumulator
                        .candidates
                        .get(key)
                        .map(|incumbent| {
                            candidate_is_better(representative, observation, incumbent)
                        })
                        .transpose()?
                        .unwrap_or(true),
                },
            };

            // Every check above reads immutable state. From here onward the
            // complete transaction is applied with no semantic failure point.
            accumulator
                .transition_support
                .commit_prepared(prepared_transition);
            accumulator.classified = classified;
            accumulator.admissible = admissible;
            accumulator.matching = matching_count;
            accumulator
                .keys_seen
                .insert(key.to_vec().into_boxed_slice());
            accumulator
                .key_supports
                .insert(key.to_vec().into_boxed_slice(), support);
            match group_extrema {
                PreparedGroupExtrema::Existing => {
                    let group = accumulator
                        .key_extrema
                        .get_mut(key)
                        .expect("prechecked Explore extrema group exists");
                    for (extrema_accumulator, value) in group.iter_mut().zip(extrema) {
                        extrema_accumulator.observe_prechecked(*value, &case_id);
                    }
                }
                PreparedGroupExtrema::New(group) => {
                    accumulator
                        .key_extrema
                        .insert(key.to_vec().into_boxed_slice(), group);
                }
            }
            if let MatchingCaseTransaction::Materialized {
                observation,
                retain_ledger,
            } = matching
            {
                if retain_ledger {
                    accumulator.ledger.push(observation.clone());
                }
                if replace_representative {
                    accumulator
                        .selected_representatives
                        .insert(observation.key.clone(), observation.case_id.clone());
                    accumulator
                        .candidates
                        .insert(observation.key.clone(), observation);
                }
            }
            CaseTerminal::AdmissibleMatch
        }
    };
    Ok(ObservedCase {
        terminal,
        stop: None,
    })
}

/// Evaluate and atomically accept one canonical CaseId. Search order is
/// intentionally absent from this function: canonical exhaustion and
/// source-candidate-first scheduling feed observations through exactly the
/// same semantic path.
fn evaluate_and_observe_case(
    accumulator: &mut ExactAccumulator,
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    transition_schemas: &TransitionSchemaIdentities,
    question: &Expr,
    ordinals: &[u128],
    retain_ledger: bool,
) -> Result<ObservedCase, ExactEngineFailure> {
    match evaluate_case_transaction(
        accumulator,
        runtime,
        runtime_context,
        query,
        transition_schemas,
        question,
        ordinals,
        retain_ledger,
    )? {
        ExactCaseEvaluation::StructurallyExcluded(_) => {
            accumulator.classified =
                incremented_counter(accumulator.classified, "classified-case")?;
            Ok(ObservedCase {
                terminal: CaseTerminal::Excluded,
                stop: None,
            })
        }
        ExactCaseEvaluation::Complete(transaction) => {
            accept_case_transaction(accumulator, &query.query.output.representative, transaction)
        }
        ExactCaseEvaluation::Open(observed) => Ok(observed),
    }
}

/// One in-process evaluator owned by a durable stream invocation.
///
/// It never owns semantic accumulator state. Each call produces either one
/// complete proposal for the requested rank or an operationally open case;
/// the coordinator validates, persists, and applies the proposal separately.
pub(super) struct ExactStreamEvaluator<'a> {
    statements: &'a [Stmt],
    source_dir: Option<&'a str>,
    artifacts: &'a TypeCheckArtifacts,
    query: &'a ExploreQueryIr,
    transition_schemas: TransitionSchemaIdentities,
    catalog: calculate::TypeCatalog,
    roots: BTreeSet<ExploreRuntimeRoot>,
    runtime: ExactRuntime,
    question: Expr,
    step_limit: usize,
    collection_limit: usize,
}

#[derive(Debug)]
pub(super) enum ExactStreamEvaluatorPrepareError {
    OperationalLimit(ExploreStopReason),
    Failure(String),
}

impl ExactStreamEvaluatorPrepareError {
    fn into_message(self) -> String {
        match self {
            Self::OperationalLimit(stop) => {
                format!("durable exact evaluator hit an operational limit: {stop:?}")
            }
            Self::Failure(message) => message,
        }
    }
}

#[derive(Debug)]
pub(super) enum ExactStreamCaseAttempt {
    Complete(ExactEvaluatorConfirmedObservationV2),
    Open(ExploreStopReason),
}

/// Non-forgeable (outside this module) evidence that one proposal came
/// directly from the coordinator-owned checked evaluator, not from decoded
/// worker or storage bytes.
#[derive(Debug)]
pub(super) struct ExactEvaluatorConfirmedObservationV2 {
    proposal: ExactCaseObservationProposalV2,
    transition_schemas: TransitionSchemaIdentities,
}

/// One evaluator-sealed batch kept opaque until the coordinator commits it.
/// The persisted proposal, reducer evidence and checked schemas cannot be
/// supplied independently, so one journal blob can never be paired with
/// another batch's semantic facts or reducer delta.
#[derive(Debug)]
pub(super) struct SealedExactObservationBatchV2 {
    proposal: ExactCaseObservationBatchProposalV2,
    validated: ValidatedExactCaseObservationBatchV2,
    transition_schemas: TransitionSchemaIdentities,
}

impl SealedExactObservationBatchV2 {
    pub(super) fn proposal(&self) -> &ExactCaseObservationBatchProposalV2 {
        &self.proposal
    }

    pub(super) fn validated(&self) -> &ValidatedExactCaseObservationBatchV2 {
        &self.validated
    }

    pub(super) fn transition_schemas(&self) -> &TransitionSchemaIdentities {
        &self.transition_schemas
    }

    pub(super) fn into_validated(self) -> ValidatedExactCaseObservationBatchV2 {
        self.validated
    }

    pub(super) fn into_proposal(self) -> ExactCaseObservationBatchProposalV2 {
        self.proposal
    }
}

impl ExactEvaluatorConfirmedObservationV2 {
    pub(super) fn rank(&self) -> u128 {
        self.proposal.case_id.rank
    }

    pub(super) fn canonical_encoded_len(&self) -> Result<usize, String> {
        encode_exact_case_observation_v2(&self.proposal, &self.transition_schemas)
            .map(|bytes| bytes.len())
            .map_err(|error| error.to_string())
    }
}

/// Cross the exact-stream mint boundary without re-running local work. The
/// private token type proves every item was returned by this trusted evaluator
/// instance; future out-of-process workers must use a separate revalidation
/// adapter and cannot construct these tokens.
pub(super) fn seal_local_evaluator_observation_batch_v2(
    confirmed: Vec<ExactEvaluatorConfirmedObservationV2>,
) -> Result<SealedExactObservationBatchV2, String> {
    let transition_schemas = confirmed
        .first()
        .map(|confirmed| confirmed.transition_schemas.clone())
        .ok_or_else(|| "trusted exact evaluator batch must not be empty".to_string())?;
    if confirmed
        .iter()
        .any(|confirmed| confirmed.transition_schemas != transition_schemas)
    {
        return Err(
            "trusted exact evaluator batch mixes checked transition schema identities".to_string(),
        );
    }
    let proposal = ExactCaseObservationBatchProposalV2::new(
        confirmed
            .into_iter()
            .map(|confirmed| confirmed.proposal)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
    .map_err(|error| error.to_string())?;
    let persisted = proposal.clone();
    let validated = seal_evaluator_confirmed_canonical_observation_batch(proposal, |_| Ok(()))
        .map_err(|error| error.to_string())?;
    Ok(SealedExactObservationBatchV2 {
        proposal: persisted,
        validated,
        transition_schemas,
    })
}

impl<'a> ExactStreamEvaluator<'a> {
    pub(super) fn transition_schemas(&self) -> &TransitionSchemaIdentities {
        &self.transition_schemas
    }

    pub(super) fn prepare(
        supplied_statements: &'a [Stmt],
        supplied_source_dir: Option<&'a str>,
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        step_limit: usize,
        collection_limit: usize,
    ) -> Result<Self, String> {
        Self::prepare_classified(
            supplied_statements,
            supplied_source_dir,
            artifacts,
            accepted_query_index,
            step_limit,
            collection_limit,
        )
        .map_err(ExactStreamEvaluatorPrepareError::into_message)
    }

    pub(super) fn prepare_classified(
        supplied_statements: &'a [Stmt],
        supplied_source_dir: Option<&'a str>,
        artifacts: &'a TypeCheckArtifacts,
        accepted_query_index: usize,
        step_limit: usize,
        collection_limit: usize,
    ) -> Result<Self, ExactStreamEvaluatorPrepareError> {
        if step_limit == 0 || collection_limit == 0 {
            return Err(ExactStreamEvaluatorPrepareError::Failure(
                "durable exact evaluation requires positive step and collection limits".to_string(),
            ));
        }
        artifacts
            .validate_checked_runtime_entry_v1(supplied_statements, supplied_source_dir)
            .map_err(|error| {
                ExactStreamEvaluatorPrepareError::Failure(format!(
                    "cannot select immutable exact runtime entry: {error}"
                ))
            })?;
        let (statements, source_dir) =
            artifacts
                .checked_runtime_root_program_v1()
                .map_err(|error| {
                    ExactStreamEvaluatorPrepareError::Failure(format!(
                        "cannot select immutable exact runtime root: {error}"
                    ))
                })?;
        let checked = artifacts
            .checked_exploration_query(accepted_query_index)
            .map_err(|error| {
                ExactStreamEvaluatorPrepareError::Failure(format!(
                    "cannot revalidate exact evaluator query: {error:?}"
                ))
            })?;
        let query = checked.closed_query;
        let transition_schemas = checked.transition_schemas().clone();
        let catalog = calculate::TypeCatalog::collect_checked(statements, source_dir).map_err(
            |diagnostics| {
                ExactStreamEvaluatorPrepareError::Failure(format!(
                    "cannot construct durable exact Explore type catalog: {}",
                    diagnostics.join("; ")
                ))
            },
        )?;
        let roots = required_runtime_roots(query);
        let runtime = ExactRuntimeContext {
            statements,
            source_dir,
            artifacts,
            catalog: &catalog,
            roots: &roots,
            step_limit,
            collection_limit,
            phase_override: None,
        }
        .fresh()
        .map_err(exact_stream_prepare_error)?;
        Ok(Self {
            statements,
            source_dir,
            artifacts,
            query,
            transition_schemas,
            catalog,
            roots,
            runtime,
            question: question_expression(query),
            step_limit,
            collection_limit,
        })
    }

    /// Evaluate one rank as a whole-CaseId retry unit. Matching cases always
    /// materialize every projected field even when an in-memory representative
    /// optimization could otherwise skip them.
    pub(super) fn evaluate_rank(&mut self, rank: u128) -> Result<ExactStreamCaseAttempt, String> {
        let ordinals = unrank_product(
            self.query
                .universe
                .dimensions
                .iter()
                .map(|dimension| dimension.domain.cardinality()),
            rank,
        )?;
        let context = ExactRuntimeContext {
            statements: self.statements,
            source_dir: self.source_dir,
            artifacts: self.artifacts,
            catalog: &self.catalog,
            roots: &self.roots,
            step_limit: self.step_limit,
            collection_limit: self.collection_limit,
            phase_override: None,
        };
        let transaction = evaluate_case_transaction(
            &ExactAccumulator::new(),
            &mut self.runtime,
            &context,
            self.query,
            &self.transition_schemas,
            &self.question,
            &ordinals,
            // Durable observations are deliberately fully materialized.
            true,
        )
        .map_err(exact_stream_failure_message)?;

        let outcome = match transaction {
            ExactCaseEvaluation::Open(observed) => {
                let stop = observed.stop.ok_or_else(|| {
                    "durable exact evaluation left a case open without a stop reason".to_string()
                })?;
                return Ok(ExactStreamCaseAttempt::Open(stop));
            }
            ExactCaseEvaluation::StructurallyExcluded(case_id) => {
                if case_id.0.as_ref() != ordinals.as_slice() {
                    return Err(
                        "durable evaluator structurally excluded another CaseId".to_string()
                    );
                }
                ExactCaseOutcomeV2::StructurallyExcluded
            }
            ExactCaseEvaluation::Complete(ExactCaseTransaction {
                case_id,
                transition,
                outcome,
            }) => {
                if case_id.0.as_ref() != ordinals.as_slice() {
                    return Err(
                        "durable evaluator returned a transaction for another CaseId".to_string(),
                    );
                }
                let outcome = match outcome {
                    ExactCaseOutcome::Excluded => ExactConstructibleCaseOutcomeV2::ValidityExcluded,
                    ExactCaseOutcome::AdmissibleNonmatch => {
                        ExactConstructibleCaseOutcomeV2::AdmissibleNonmatch
                    }
                    ExactCaseOutcome::AdmissibleMatch(MatchingCaseTransaction::Materialized {
                        observation,
                        ..
                    }) => {
                        if observation.case_id != case_id {
                            return Err(
                                "durable evaluator returned a projection for another CaseId"
                                    .to_string(),
                            );
                        }
                        let projection = ExactMatchProjectionV1::new(
                            observation.key,
                            observation.extrema,
                            observation.shown,
                            observation.objective,
                        )
                        .map_err(|error| error.to_string())?;
                        ExactConstructibleCaseOutcomeV2::AdmissibleMatch(projection)
                    }
                    ExactCaseOutcome::AdmissibleMatch(
                        MatchingCaseTransaction::ProjectionOnly { .. },
                    ) => {
                        return Err("durable evaluator produced a projection-only matching case"
                            .to_string())
                    }
                };
                ExactCaseOutcomeV2::Constructible {
                    transition,
                    outcome,
                }
            }
        };

        let mut proposal = ExactCaseObservationProposalV2::new(
            ExactCanonicalCaseIdV1::new(rank, ordinals),
            outcome,
            ExactValidationReceiptDigestV1::new([0; 32]),
        )
        .map_err(|error| error.to_string())?;
        let provisional = encode_exact_case_observation_v2(&proposal, &self.transition_schemas)
            .map_err(|error| error.to_string())?;
        let mut receipt = Sha256::new();
        receipt.update(EXACT_STREAM_EVALUATOR_RECEIPT_V2);
        receipt.update((provisional.len() as u64).to_le_bytes());
        receipt.update(&provisional);
        proposal.validation_receipt_digest =
            ExactValidationReceiptDigestV1::new(receipt.finalize().into());
        Ok(ExactStreamCaseAttempt::Complete(
            ExactEvaluatorConfirmedObservationV2 {
                proposal,
                transition_schemas: self.transition_schemas.clone(),
            },
        ))
    }
}

fn exact_stream_prepare_error(failure: ExactEngineFailure) -> ExactStreamEvaluatorPrepareError {
    match failure {
        ExactEngineFailure::OperationalLimit(stop) => {
            ExactStreamEvaluatorPrepareError::OperationalLimit(stop)
        }
        ExactEngineFailure::Unsupported(message) | ExactEngineFailure::Error(message) => {
            ExactStreamEvaluatorPrepareError::Failure(message)
        }
    }
}

fn exact_stream_failure_message(failure: ExactEngineFailure) -> String {
    match failure {
        ExactEngineFailure::OperationalLimit(stop) => {
            format!("durable exact evaluator hit an operational limit: {stop:?}")
        }
        ExactEngineFailure::Unsupported(message) | ExactEngineFailure::Error(message) => message,
    }
}

fn graph_terminal_count(
    counts: &BTreeMap<CaseTerminal, CheckedCardinality>,
    terminal: &CaseTerminal,
) -> Result<u128, String> {
    match counts
        .get(terminal)
        .copied()
        .unwrap_or(CheckedCardinality::Exact(0))
    {
        CheckedCardinality::Exact(count) => Ok(count),
        CheckedCardinality::ExceedsU128 => Err(format!(
            "case-graph count for terminal {terminal:?} exceeds u128::MAX"
        )),
    }
}

/// Reconcile proof-closed case support with evaluator observations. Proof
/// regions may establish exact admissible/nonmatching counts without running
/// one runtime per case. If a proof closes matching support that was not
/// evaluated, case counts remain exact while projection, representative and
/// ledger layers stay explicitly open.
fn absorb_graph_proof_counts(
    accumulator: &mut ExactAccumulator,
    graph: &CaseDecisionDag,
) -> Result<(), ExactEngineFailure> {
    let counts = graph
        .terminal_counts()
        .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
    let nonmatching = graph_terminal_count(&counts, &CaseTerminal::AdmissibleNonmatch)
        .map_err(ExactEngineFailure::Error)?;
    let matching = graph_terminal_count(&counts, &CaseTerminal::AdmissibleMatch)
        .map_err(ExactEngineFailure::Error)?;
    let admissible_open = graph_terminal_count(
        &counts,
        &CaseTerminal::AdmissibleOpen(CaseOpenReason::SearchBudgetExhausted),
    )
    .and_then(|count| {
        graph_terminal_count(
            &counts,
            &CaseTerminal::AdmissibleOpen(CaseOpenReason::EvaluationUnknown),
        )
        .and_then(|other| {
            count
                .checked_add(other)
                .ok_or_else(|| "case-graph admissible-open support exceeds u128::MAX".to_string())
        })
    })
    .map_err(ExactEngineFailure::Error)?;
    let known_admissible = nonmatching
        .checked_add(matching)
        .and_then(|count| count.checked_add(admissible_open))
        .ok_or_else(|| {
            ExactEngineFailure::Error(
                "case-graph known-admissible support exceeds u128::MAX".to_string(),
            )
        })?;
    if known_admissible < accumulator.admissible || matching < accumulator.matching {
        return Err(ExactEngineFailure::Error(
            "proof-lowered case graph lost an evaluated admissible or matching case".to_string(),
        ));
    }
    if matching > accumulator.matching {
        accumulator.projection_observations_complete = false;
        accumulator.representative_selection_observations_complete = false;
        accumulator.ledger_observations_complete = false;
    }
    accumulator.admissible = known_admissible;
    accumulator.matching = matching;
    Ok(())
}

#[derive(Clone, Copy)]
struct CaseLayerClosure {
    admissibility: bool,
    polarity: bool,
    closed_cases: u128,
    open_cases: u128,
}

fn validate_graph_counts(
    graph: &CaseDecisionDag,
    declared: u128,
    admissible: u128,
    matching: u128,
) -> Result<CaseLayerClosure, String> {
    let counts = graph.terminal_counts().map_err(|error| error.to_string())?;
    let excluded = graph_terminal_count(&counts, &CaseTerminal::Excluded)?;
    let eligibility_open = graph_terminal_count(
        &counts,
        &CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
    )?
    .checked_add(graph_terminal_count(
        &counts,
        &CaseTerminal::EligibilityOpen(CaseOpenReason::EvaluationUnknown),
    )?)
    .ok_or_else(|| "case-graph eligibility-open count exceeds u128::MAX".to_string())?;
    let nonmatching = graph_terminal_count(&counts, &CaseTerminal::AdmissibleNonmatch)?;
    let graph_matching = graph_terminal_count(&counts, &CaseTerminal::AdmissibleMatch)?;
    let admissible_open = graph_terminal_count(
        &counts,
        &CaseTerminal::AdmissibleOpen(CaseOpenReason::SearchBudgetExhausted),
    )?
    .checked_add(graph_terminal_count(
        &counts,
        &CaseTerminal::AdmissibleOpen(CaseOpenReason::EvaluationUnknown),
    )?)
    .ok_or_else(|| "case-graph admissible-open count exceeds u128::MAX".to_string())?;

    let graph_admissible = nonmatching
        .checked_add(graph_matching)
        .and_then(|count| count.checked_add(admissible_open))
        .ok_or_else(|| "case-graph known-admissible count exceeds u128::MAX".to_string())?;
    let graph_declared = excluded
        .checked_add(graph_admissible)
        .and_then(|count| count.checked_add(eligibility_open))
        .ok_or_else(|| "case-graph universe count exceeds u128::MAX".to_string())?;

    if graph_declared != declared {
        return Err(format!(
            "case graph classifies {graph_declared} assignments, expected {declared}"
        ));
    }
    if graph_admissible != admissible {
        return Err(format!(
            "case graph knows {graph_admissible} admissible assignments, accumulator knows {admissible}"
        ));
    }
    if graph_matching != matching {
        return Err(format!(
            "case graph knows {graph_matching} matching assignments, accumulator knows {matching}"
        ));
    }
    let closed_cases = excluded
        .checked_add(nonmatching)
        .and_then(|count| count.checked_add(graph_matching))
        .ok_or_else(|| "case-graph closed-case count exceeds u128::MAX".to_string())?;
    let open_cases = eligibility_open
        .checked_add(admissible_open)
        .ok_or_else(|| "case-graph open-case count exceeds u128::MAX".to_string())?;
    Ok(CaseLayerClosure {
        admissibility: eligibility_open == 0,
        polarity: eligibility_open == 0 && admissible_open == 0,
        closed_cases,
        open_cases,
    })
}

fn replay_and_confirm(
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    question: &Expr,
    expected: &SearchObservation,
) -> Result<(), ExactEngineFailure> {
    let replay_context = runtime_context.for_replay();
    let mut runtime = replay_context.fresh()?;
    let assignment = expected
        .case_id
        .assignment(query)
        .map_err(ExactEngineFailure::Error)?;
    let frame = match evaluate_admissibility(&mut runtime, &replay_context, query, &assignment)? {
        Admissibility::StructurallyExcluded | Admissibility::Excluded(_) => {
            return Err(ExactEngineFailure::Error(
                "fresh Explore replay excluded a previously matching configuration".to_string(),
            ))
        }
        Admissibility::Admissible(frame) => frame,
    };
    let transition_env = frame.transition_env();
    if !evaluate_polarity(
        &mut runtime,
        &replay_context,
        query,
        question,
        transition_env,
    )? {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay changed a previously matching question result".to_string(),
        ));
    }
    let key = evaluate_key(&mut runtime, &replay_context, query, transition_env)?;
    if key != expected.key {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay disagreed with the enumerated result key".to_string(),
        ));
    }
    let extrema = evaluate_extrema(&mut runtime, &replay_context, query, transition_env)?;
    if extrema != expected.extrema {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay disagreed with the enumerated extrema values".to_string(),
        ));
    }
    let (shown, objective) = evaluate_shown_and_objective(
        &mut runtime,
        &replay_context,
        query,
        transition_env,
        &extrema,
    )?;
    if shown != expected.shown {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay disagreed with the enumerated shown values".to_string(),
        ));
    }
    if objective != expected.objective {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay disagreed with the enumerated representative objective"
                .to_string(),
        ));
    }
    Ok(())
}

fn replay_and_confirm_extrema_witness(
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    question: &Expr,
    case_id: &OrdinalCaseId,
    expectations: &BTreeSet<ExtremaWitnessExpectation>,
) -> Result<(), ExactEngineFailure> {
    if expectations.is_empty() {
        return Err(ExactEngineFailure::Error(
            "internal Explore error: extrema witness replay has no expectation".to_string(),
        ));
    }
    let replay_context = runtime_context.for_replay();
    let mut runtime = replay_context.fresh()?;
    let assignment = case_id
        .assignment(query)
        .map_err(ExactEngineFailure::Error)?;
    let frame = match evaluate_admissibility(&mut runtime, &replay_context, query, &assignment)? {
        Admissibility::StructurallyExcluded | Admissibility::Excluded(_) => {
            return Err(ExactEngineFailure::Error(
                "fresh Explore replay excluded an extrema endpoint witness".to_string(),
            ))
        }
        Admissibility::Admissible(frame) => frame,
    };
    let transition_env = frame.transition_env();
    if !evaluate_polarity(
        &mut runtime,
        &replay_context,
        query,
        question,
        transition_env,
    )? {
        return Err(ExactEngineFailure::Error(
            "fresh Explore replay changed an extrema endpoint witness to a nonmatch".to_string(),
        ));
    }
    let key = evaluate_key(&mut runtime, &replay_context, query, transition_env)?;
    if expectations
        .iter()
        .any(|expectation| expectation.key.as_ref() != key.as_ref())
    {
        return Err(ExactEngineFailure::Error(
            "fresh Explore extrema witness replay disagreed with its result key".to_string(),
        ));
    }

    let mut replayed_values = BTreeMap::<usize, i64>::new();
    for expectation in expectations {
        if !replayed_values.contains_key(&expectation.extrema_index) {
            let value = evaluate_extrema_value(
                &mut runtime,
                &replay_context,
                query,
                transition_env,
                expectation.extrema_index,
            )?;
            replayed_values.insert(expectation.extrema_index, value);
        }
        let value = replayed_values
            .get(&expectation.extrema_index)
            .copied()
            .expect("extrema witness value was just replayed");
        if value != expectation.value {
            return Err(ExactEngineFailure::Error(format!(
                "fresh Explore replay produced extrema value {value}, expected endpoint {}",
                expectation.value
            )));
        }
    }
    Ok(())
}

fn group_is_emitted(
    query: &ExploreQueryIr,
    extrema: &[ExactExtremaAccumulator],
) -> Result<bool, String> {
    match &query.query.output.having {
        None => Ok(true),
        Some(crate::TypedExploreHaving::Varies { extrema_index, .. }) => extrema
            .get(*extrema_index)
            .map(|summary| summary.minimum < summary.maximum)
            .ok_or_else(|| {
                format!(
                    "internal Explore error: varies index {extrema_index} has no extrema accumulator"
                )
            }),
    }
}

fn first_representatives_closed(search_trace: ExactSearchTrace, projection_closed: bool) -> bool {
    projection_closed || matches!(search_trace, ExactSearchTrace::Canonical)
}

fn finalize_report(
    query: &ExploreQueryIr,
    request: ExploreReportRequest,
    runtime_context: Option<&ExactRuntimeContext<'_>>,
    question: &Expr,
    mut state: ExactSearchState,
) -> Result<ExploreExactReport, String> {
    if request.semantic_transition_graph == ExploreSemanticTransitionGraphRequest::Include {
        return Err(
            "the synchronous exact report cannot represent requested semantic-transition graph evidence; use the durable Explore stream publisher"
                .to_string(),
        );
    }
    let case_closure = validate_graph_counts(
        &state.case_graph,
        state.declared,
        state.admissible,
        state.matching,
    )?;
    let transition_count = state.transition_support.len() as u128;
    let admissible_transition_count = state.transition_support.admissible_transition_count();
    let matching_transition_count = state.transition_support.matching_transition_count();
    if transition_count > case_closure.closed_cases
        || admissible_transition_count > state.admissible
        || matching_transition_count > state.matching
    {
        return Err(format!(
            "semantic transition populations exceed their case populations: U_T={transition_count}, D_T={admissible_transition_count}, M_T={matching_transition_count}, closed={}, admissible={}, matching={}",
            case_closure.closed_cases, state.admissible, state.matching,
        ));
    }

    let aggregate_projection_materialized = state.key_extrema.len() == state.keys_seen.len()
        && state.keys_seen.iter().all(|key| {
            let Some(extrema) = state.key_extrema.get(key) else {
                return false;
            };
            let Some(support) = state.key_supports.get(key) else {
                return false;
            };
            extrema.len() == query.query.output.extrema.len()
                && extrema
                    .iter()
                    .all(|accumulator| accumulator.support == *support)
        });
    let projection_closed = case_closure.polarity
        && state.projection_observations_complete
        && aggregate_projection_materialized;

    let mut emitted_keys = BTreeSet::<Box<[ExploreValue]>>::new();
    for key in &state.keys_seen {
        let emitted = match state.key_extrema.get(key) {
            Some(extrema) => group_is_emitted(query, extrema)?,
            None => query.query.output.having.is_none(),
        };
        if emitted {
            emitted_keys.insert(key.clone());
        }
    }
    let mut extrema_witness_targets =
        BTreeMap::<OrdinalCaseId, BTreeSet<ExtremaWitnessExpectation>>::new();
    if projection_closed {
        for key in &emitted_keys {
            let extrema = state.key_extrema.get(key).ok_or_else(|| {
                "internal Explore error: emitted key has no extrema accumulator".to_string()
            })?;
            for (extrema_index, accumulator) in extrema.iter().enumerate() {
                for (case_id, value) in [
                    (&accumulator.minimum_witness, accumulator.minimum),
                    (&accumulator.maximum_witness, accumulator.maximum),
                ] {
                    extrema_witness_targets
                        .entry(case_id.clone())
                        .or_default()
                        .insert(ExtremaWitnessExpectation {
                            key: key.clone(),
                            extrema_index,
                            value,
                        });
                }
            }
        }
    }
    // `first` is prefix-stable in canonical CaseId order: once a key has been
    // observed, no later case can replace its representative.  Ordered
    // objectives need the whole key class to close before their optimum is
    // final.  Projection may therefore remain open while already discovered
    // `first` rows are selection-closed and replayable.
    let selected_keys_complete = state.selected_representatives.len() == state.keys_seen.len()
        && state
            .keys_seen
            .iter()
            .all(|key| state.selected_representatives.contains_key(key));
    let representatives_closed = state.representative_selection_observations_complete
        && selected_keys_complete
        && match &query.query.output.representative {
            ExploreRepresentative::First { .. } => {
                first_representatives_closed(state.search_trace, projection_closed)
            }
            ExploreRepresentative::Maximize { .. } | ExploreRepresentative::Minimize { .. } => {
                projection_closed
            }
        };
    let representative_rows_materialized = state.candidates.len()
        == state.selected_representatives.len()
        && state.candidates.iter().all(|(key, observation)| {
            state.selected_representatives.get(key) == Some(&observation.case_id)
        });
    let grouped_projection_ready = query.query.output.extrema.is_empty() || projection_closed;
    let representative_targets =
        if representatives_closed && representative_rows_materialized && grouped_projection_ready {
            state
                .candidates
                .iter()
                .filter(|(key, _)| emitted_keys.contains(*key))
                .map(|(_, observation)| observation.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

    state
        .ledger
        .sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let mut replay_targets = BTreeMap::<OrdinalCaseId, SearchObservation>::new();
    for observation in representative_targets.iter().chain(&state.ledger) {
        if let Some(prior) = replay_targets.insert(observation.case_id.clone(), observation.clone())
        {
            if prior != *observation {
                return failure_report(
                    query,
                    ExactEngineFailure::Error(
                        "one Explore case accumulated inconsistent replay expectations".to_string(),
                    ),
                );
            }
        }
    }

    let mut confirmed_extrema_witnesses = BTreeSet::<OrdinalCaseId>::new();
    let mut extrema_witness_replay_stopped = false;
    if !extrema_witness_targets.is_empty() {
        let runtime_context = runtime_context.ok_or_else(|| {
            "internal Explore error: extrema witness targets exist without a runtime context"
                .to_string()
        })?;
        for (case_id, expectations) in &extrema_witness_targets {
            match replay_and_confirm_extrema_witness(
                runtime_context,
                query,
                question,
                case_id,
                expectations,
            ) {
                Ok(()) => {
                    confirmed_extrema_witnesses.insert(case_id.clone());
                }
                Err(ExactEngineFailure::OperationalLimit(stop)) => {
                    if state.stop.is_none() {
                        state.stop = Some(stop);
                    }
                    extrema_witness_replay_stopped = true;
                    break;
                }
                Err(failure) => return failure_report(query, failure),
            }
        }
    }

    let mut confirmed = BTreeSet::<OrdinalCaseId>::new();
    if !extrema_witness_replay_stopped && !replay_targets.is_empty() {
        let runtime_context = runtime_context.ok_or_else(|| {
            "internal Explore error: replay targets exist without a runtime context".to_string()
        })?;
        for (case_id, observation) in &replay_targets {
            match replay_and_confirm(runtime_context, query, question, observation) {
                Ok(()) => {
                    confirmed.insert(case_id.clone());
                }
                Err(ExactEngineFailure::OperationalLimit(stop)) => {
                    if state.stop.is_none() {
                        state.stop = Some(stop);
                    }
                    break;
                }
                Err(failure) => return failure_report(query, failure),
            }
        }
    }

    let ledger_rows = state
        .ledger
        .iter()
        .filter(|observation| confirmed.contains(&observation.case_id))
        .map(|observation| {
            observation
                .case_id
                .assignment(query)
                .map(|dimensions| {
                    ExploreLedgerRow::confirmed(
                        ExploreCaseId::new(observation.case_id.0.clone()),
                        dimensions,
                        ExploreResultKey::new(observation.key.clone()),
                        observation.shown.clone(),
                    )
                })
                .map_err(ExactEngineFailure::Error)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|failure| match failure {
            ExactEngineFailure::Error(message) => message,
            _ => "unexpected failure while constructing the Explore ledger".to_string(),
        })?;

    let extrema_witnesses_closed = query.query.output.extrema.is_empty()
        || (projection_closed
            && extrema_witness_targets
                .keys()
                .all(|case_id| confirmed_extrema_witnesses.contains(case_id)));
    let result_rows_closed = representatives_closed
        && representative_rows_materialized
        && grouped_projection_ready
        && extrema_witnesses_closed
        && representative_targets
            .iter()
            .all(|observation| confirmed.contains(&observation.case_id));
    let results = if result_rows_closed {
        representative_targets
            .iter()
            .map(|observation| {
                let observed_support = state
                    .key_supports
                    .get(&observation.key)
                    .copied()
                    .ok_or_else(|| {
                        "internal Explore error: representative key has no support count"
                            .to_string()
                    })?;
                let support = if projection_closed {
                    ExploreCount::Exact(observed_support)
                } else {
                    ExploreCount::LowerBound(observed_support)
                };
                let extrema = state.key_extrema.get(&observation.key).ok_or_else(|| {
                    "internal Explore error: representative key has no extrema accumulator"
                        .to_string()
                })?;
                if extrema
                    .iter()
                    .any(|accumulator| accumulator.support != observed_support)
                {
                    return Err(
                        "internal Explore error: extrema support disagrees with result-key support"
                            .to_string(),
                    );
                }
                Ok(ExploreResultRow::confirmed_with_support_and_extrema(
                    ExploreResultKey::new(observation.key.clone()),
                    extrema
                        .iter()
                        .map(ExactExtremaAccumulator::summary)
                        .collect::<Vec<_>>(),
                    observation.shown.clone(),
                    ExploreCaseId::new(observation.case_id.0.clone()),
                    support,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        Vec::new()
    };
    let ledger_closed = match request.ledger {
        ExploreLedgerRequest::Omit => true,
        ExploreLedgerRequest::MatchingConfigurations => {
            case_closure.polarity
                && state.ledger_observations_complete
                && state.ledger.len() as u128 == state.matching
                && state
                    .ledger
                    .iter()
                    .all(|observation| confirmed.contains(&observation.case_id))
        }
    };

    let exact_or_lower = |value, closed| {
        if closed {
            ExploreCount::Exact(value)
        } else {
            ExploreCount::LowerBound(value)
        }
    };
    let raw_group_count = state.keys_seen.len() as u128;
    let emitted_group_count = emitted_keys.len() as u128;
    let qualifying_configuration_count = emitted_keys.iter().try_fold(0_u128, |total, key| {
        let support = state.key_supports.get(key).copied().ok_or_else(|| {
            "internal Explore error: emitted key has no support count".to_string()
        })?;
        total
            .checked_add(support)
            .ok_or_else(|| "Explore qualifying-configuration count exceeds u128::MAX".to_string())
    })?;
    let counts = ExploreCounts {
        declared_assignments: ExploreCount::Exact(state.declared),
        admissible_configurations: exact_or_lower(state.admissible, case_closure.admissibility),
        matching_configurations: exact_or_lower(state.matching, case_closure.polarity),
        distinct_result_keys: exact_or_lower(emitted_group_count, projection_closed),
    };
    let group_counts = match &query.query.output.having {
        None => ExploreGroupCounts::unfiltered(
            exact_or_lower(raw_group_count, projection_closed),
            counts.matching_configurations,
        ),
        Some(crate::TypedExploreHaving::Varies { .. }) if projection_closed => {
            let suppressed_groups = raw_group_count
                .checked_sub(emitted_group_count)
                .ok_or_else(|| {
                    "internal Explore error: emitted groups exceed raw groups".to_string()
                })?;
            let suppressed_configurations = state
                .matching
                .checked_sub(qualifying_configuration_count)
                .ok_or_else(|| {
                    "internal Explore error: qualifying configurations exceed matches".to_string()
                })?;
            ExploreGroupCounts {
                raw_groups: ExploreCount::Exact(raw_group_count),
                emitted_groups: ExploreCount::Exact(emitted_group_count),
                suppressed_groups: ExploreCount::Exact(suppressed_groups),
                qualifying_configurations: ExploreCount::Exact(qualifying_configuration_count),
                suppressed_configurations: ExploreCount::Exact(suppressed_configurations),
            }
        }
        Some(crate::TypedExploreHaving::Varies { .. }) => ExploreGroupCounts {
            raw_groups: ExploreCount::LowerBound(raw_group_count),
            emitted_groups: ExploreCount::LowerBound(emitted_group_count),
            suppressed_groups: ExploreCount::LowerBound(0),
            qualifying_configurations: ExploreCount::LowerBound(qualifying_configuration_count),
            suppressed_configurations: ExploreCount::LowerBound(0),
        },
    };
    let closures = ExploreLayerClosures {
        admissibility: if case_closure.admissibility {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
        polarity: if case_closure.polarity {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
        projection: if projection_closed {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
        representatives: if representatives_closed {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
        rows: if result_rows_closed {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
        views: if ledger_closed {
            ExploreClosure::Closed
        } else {
            ExploreClosure::Open
        },
    };
    let coverage = ExploreCoverage::from_counts(
        counts.admissible_configurations,
        counts.matching_configurations,
    );
    let search = match state.search_trace {
        ExactSearchTrace::Canonical => ExploreSearchEvidence::Canonical {
            classified_cases: case_closure.closed_cases,
            remaining_open_cases: case_closure.open_cases,
            exhausted: case_closure.open_cases == 0,
        },
        ExactSearchTrace::SourceCandidateFirst(search) => {
            if search.remaining_open_cases() != case_closure.open_cases {
                return Err(format!(
                    "source-candidate scheduler reports {} open cases but exact evaluation reports {}",
                    search.remaining_open_cases(),
                    case_closure.open_cases
                ));
            }
            search
        }
    };
    let search_decision_dag = match request.search_decision_dag {
        ExploreSearchDecisionDagRequest::Omit => ExploreSearchDecisionDagEvidence::Omitted,
        ExploreSearchDecisionDagRequest::Include => {
            ExploreSearchDecisionDagEvidence::Included(state.case_graph)
        }
    };
    let ledger = match request.ledger {
        ExploreLedgerRequest::Omit => ExploreLedgerEvidence::Omitted,
        ExploreLedgerRequest::MatchingConfigurations => {
            ExploreLedgerEvidence::MatchingConfigurations {
                rows: ledger_rows.into_boxed_slice(),
            }
        }
    };
    let evidence = ExploreExactEvidence {
        request,
        schema: report_schema(query)?,
        search,
        counts,
        group_counts,
        coverage,
        closures,
        results: results.into_boxed_slice(),
        search_decision_dag,
        ledger,
    };
    let all_requested_layers_closed = closures.all_closed();
    let completion_method = match search {
        ExploreSearchEvidence::SourceCandidateFirst {
            certified_region_closed_cases,
            ..
        } if certified_region_closed_cases != 0 => {
            ExploreCompletionMethod::ExactFiniteCertifiedClosure
        }
        ExploreSearchEvidence::Canonical { .. }
        | ExploreSearchEvidence::SourceCandidateFirst { .. } => {
            ExploreCompletionMethod::ExactFiniteExhaustion
        }
    };
    let outcome = match state.stop {
        Some(stop) => ExploreExactOutcome::Partial { stop, evidence },
        None if all_requested_layers_closed => ExploreExactOutcome::Complete {
            method: completion_method,
            evidence,
        },
        None => ExploreExactOutcome::Unknown {
            reason: "case classification is closed, but proof-closed matching cases do not have complete replayed projection evidence"
                .into(),
            evidence,
        },
    };
    ExploreExactReport::with_deferred_mechanism(query_name(query), query.query.polarity, outcome)
}

enum ExactWorkOrder<'a> {
    Canonical,
    DenseSourceCandidateFirst { plan: &'a SourceProofPlan },
}

type DenseCandidateSearch =
    CandidateFirstBoundarySearch<SourceEventLabel, ClassificationRegionCertificate>;

/// Exhaust one checked finite Explore query through ordinary interpreter
/// semantics. Operational caps retain a canonical open suffix; unsupported
/// semantics and replay disagreements never masquerade as partial success.
pub(super) fn execute_exact_finite(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    request: ExploreReportRequest,
    budget: ExploreExecutionBudget,
) -> Result<ExploreExactReport, String> {
    execute_exact_finite_with_order(
        statements,
        source_dir,
        artifacts,
        accepted_query_index,
        request,
        budget,
        ExactWorkOrder::Canonical,
    )
}

/// Execute source-derived boundary candidates before canonical fallback.
///
/// Extractions influence singleton order only. Their labels never classify a
/// case, close an interval, select a row, or become mechanism evidence.
pub(super) fn execute_exact_finite_candidate_first(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    request: ExploreReportRequest,
    budget: ExploreExecutionBudget,
    plan: &SourceProofPlan,
) -> Result<ExploreExactReport, String> {
    execute_exact_finite_with_order(
        statements,
        source_dir,
        artifacts,
        accepted_query_index,
        request,
        budget,
        ExactWorkOrder::DenseSourceCandidateFirst { plan },
    )
}

fn execute_exact_finite_with_order(
    statements: &[Stmt],
    source_dir: Option<&str>,
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    request: ExploreReportRequest,
    budget: ExploreExecutionBudget,
    work_order: ExactWorkOrder<'_>,
) -> Result<ExploreExactReport, String> {
    let checked = artifacts
        .checked_exploration_query(accepted_query_index)
        .map_err(|error| format!("cannot revalidate exact evaluator query: {error:?}"))?;
    let query = checked.closed_query;
    let transition_schemas = checked.transition_schemas();
    if budget.step_limit == 0 || budget.collection_limit == 0 {
        return failure_report(
            query,
            ExactEngineFailure::Error(
                "Explore execution requires positive step and collection limits".to_string(),
            ),
        );
    }

    let axis_cardinalities = match query_axis_cardinalities(query) {
        Ok(cardinalities) => cardinalities,
        Err(diagnostic) => {
            return failure_report(query, ExactEngineFailure::Unsupported(diagnostic))
        }
    };
    let declared = if axis_cardinalities.contains(&0) {
        Some(0)
    } else {
        axis_cardinalities
            .iter()
            .try_fold(1_u128, |product, cardinality| {
                product.checked_mul(*cardinality)
            })
    };
    let declared = match declared {
        Some(declared) => declared,
        None => {
            return failure_report(
                query,
                ExactEngineFailure::Unsupported(
                    "exact-finite assignment count exceeds u128::MAX".to_string(),
                ),
            )
        }
    };
    match query.universe.cartesian_count_before_constraints.exact() {
        Some(ir_declared) if ir_declared == declared => {}
        Some(ir_declared) => {
            return failure_report(
                query,
                ExactEngineFailure::Error(format!(
                "Explore IR declares {ir_declared} assignments but its domains contain {declared}"
            )),
            )
        }
        None => {
            return failure_report(
                query,
                ExactEngineFailure::Unsupported(
                    "Explore IR assignment count exceeds u128::MAX".to_string(),
                ),
            )
        }
    }

    let question = question_expression(query);

    // Empty Cartesian space annihilates every search order. In particular, a
    // dense boundary scheduler cannot be constructed for an empty boundary
    // interval, but that is not Unsupported: the exact answer is Complete /
    // Empty without evaluating or validating any scheduling hints.
    if declared == 0 {
        let case_graph = empty_or_open_case_graph(
            &axis_cardinalities,
            CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
        )?;
        let search_trace = match work_order {
            ExactWorkOrder::Canonical => ExactSearchTrace::Canonical,
            ExactWorkOrder::DenseSourceCandidateFirst { .. } => {
                ExactSearchTrace::SourceCandidateFirst(
                    ExploreSearchEvidence::SourceCandidateFirst {
                        distinct_source_candidates: 0,
                        scheduled_source_candidates: 0,
                        evaluated_source_candidates: 0,
                        scheduled_fallback_cases: 0,
                        evaluated_fallback_cases: 0,
                        singleton_closed_cases: 0,
                        certified_region_closed_cases: 0,
                        pending_evaluations: 0,
                        remaining_open_cases: 0,
                        exhausted: true,
                    },
                )
            }
        };
        return finalize_report(
            query,
            request,
            None,
            &question,
            ExactAccumulator::new().finish(declared, case_graph, search_trace, None),
        );
    }

    let mut candidate_search = match work_order {
        ExactWorkOrder::Canonical => None,
        ExactWorkOrder::DenseSourceCandidateFirst { plan } => {
            match build_dense_candidate_search(query, &axis_cardinalities, plan.extractions()) {
                Ok(search) => match validate_classification_proofs_without_closure(plan) {
                    Ok(()) => Some(search),
                    Err(diagnostic) => {
                        return failure_report(query, ExactEngineFailure::Error(diagnostic))
                    }
                },
                Err(diagnostic) => {
                    return failure_report(query, ExactEngineFailure::Error(diagnostic))
                }
            }
        }
    };
    // If certificates and structural boundary facts already close the whole
    // scheduler, publish that result without constructing a type catalog or
    // runtime. This is the proof-first fast path.
    if let Some(search) = candidate_search.as_ref() {
        let fully_closed = search
            .cost_ledger()
            .map_err(|error| error.to_string())?
            .remaining_open_cases()
            == 0;
        if fully_closed {
            let state = match unevaluated_candidate_state(
                search,
                declared,
                CaseOpenReason::EvaluationUnknown,
                None,
            ) {
                Ok(state) => state,
                Err(failure) => return failure_report(query, failure),
            };
            return finalize_report(query, request, None, &question, state);
        }
    }
    // A zero budget is an observation of no cases. Candidate extraction is
    // still validated, but neither user code nor one scheduler item is run.
    if budget.case_limit == Some(0) {
        let state = match candidate_search.as_ref() {
            Some(search) => {
                match unevaluated_candidate_state(
                    search,
                    declared,
                    CaseOpenReason::SearchBudgetExhausted,
                    Some(ExploreStopReason::CaseLimit { limit: 0 }),
                ) {
                    Ok(state) => state,
                    Err(failure) => return failure_report(query, failure),
                }
            }
            None => ExactAccumulator::new().finish(
                declared,
                empty_or_open_case_graph(
                    &axis_cardinalities,
                    CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
                )?,
                ExactSearchTrace::Canonical,
                Some(ExploreStopReason::CaseLimit { limit: 0 }),
            ),
        };
        return finalize_report(query, request, None, &question, state);
    }

    let catalog = match calculate::TypeCatalog::collect_checked(statements, source_dir) {
        Ok(catalog) => catalog,
        Err(diagnostics) => {
            return failure_report(
                query,
                ExactEngineFailure::Error(format!(
                    "cannot construct exact Explore type catalog: {}",
                    diagnostics.join("; ")
                )),
            )
        }
    };
    let roots = required_runtime_roots(query);
    let runtime_context = ExactRuntimeContext {
        statements,
        source_dir,
        artifacts,
        catalog: &catalog,
        roots: &roots,
        step_limit: budget.step_limit,
        collection_limit: budget.collection_limit,
        phase_override: None,
    };
    let mut runtime = match runtime_context.fresh() {
        Ok(runtime) => runtime,
        Err(ExactEngineFailure::OperationalLimit(stop)) => {
            let state = match candidate_search.as_ref() {
                Some(search) => {
                    match unevaluated_candidate_state(
                        search,
                        declared,
                        CaseOpenReason::SearchBudgetExhausted,
                        Some(stop),
                    ) {
                        Ok(state) => state,
                        Err(failure) => return failure_report(query, failure),
                    }
                }
                None => ExactAccumulator::new().finish(
                    declared,
                    empty_or_open_case_graph(
                        &axis_cardinalities,
                        CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted),
                    )?,
                    ExactSearchTrace::Canonical,
                    Some(stop),
                ),
            };
            return finalize_report(query, request, Some(&runtime_context), &question, state);
        }
        Err(failure) => return failure_report(query, failure),
    };
    let retain_ledger = matches!(request.ledger, ExploreLedgerRequest::MatchingConfigurations);

    let state = match candidate_search.as_mut() {
        None => run_canonical_order(
            &mut runtime,
            &runtime_context,
            query,
            &question,
            &axis_cardinalities,
            declared,
            retain_ledger,
            budget.case_limit,
            transition_schemas,
        ),
        Some(search) => run_candidate_first_order(
            &mut runtime,
            &runtime_context,
            query,
            &question,
            &axis_cardinalities,
            declared,
            retain_ledger,
            budget.case_limit,
            search,
            transition_schemas,
        ),
    };
    let state = match state {
        Ok(state) => state,
        Err(failure) => return failure_report(query, failure),
    };
    finalize_report(query, request, Some(&runtime_context), &question, state)
}

/// Validate classification proofs without closing constructible cases.
///
/// A polarity certificate does not carry the exact semantic transition image
/// required by the canonical case transaction. Both matching and nonmatching
/// regions therefore remain scheduler work until a future certificate can
/// prove their complete `Context + Before -> After` partition and support.
fn validate_classification_proofs_without_closure(plan: &SourceProofPlan) -> Result<(), String> {
    for proof in plan.proofs() {
        for region in proof.regions() {
            if region.certificate().interval() != region.interval() {
                return Err(
                    "classification-region certificate interval disagrees with its region"
                        .to_string(),
                );
            }
            match region.classification() {
                CaseTerminal::AdmissibleNonmatch | CaseTerminal::AdmissibleMatch => {}
                CaseTerminal::Excluded
                | CaseTerminal::EligibilityOpen(_)
                | CaseTerminal::AdmissibleOpen(_) => {
                    return Err(
                        "classification-region proof carried an unsupported terminal".to_string(),
                    )
                }
            }
        }
    }
    Ok(())
}

fn build_dense_candidate_search(
    query: &ExploreQueryIr,
    axis_cardinalities: &[u128],
    extractions: &[SourceEventExtraction],
) -> Result<DenseCandidateSearch, String> {
    let boundary = query.boundary_hint().ok_or_else(|| {
        "source-candidate-first execution requires a checked boundary query".to_string()
    })?;
    let dimension = query
        .universe
        .dimensions
        .get(boundary.axis_dimension_index)
        .ok_or_else(|| {
            format!(
                "boundary dimension {} is outside {} Explore axes",
                boundary.axis_dimension_index,
                query.universe.dimensions.len()
            )
        })?;
    if dimension.name != boundary.axis {
        return Err(format!(
            "boundary dimension names `{}` but the boundary names `{}`",
            dimension.name, boundary.axis
        ));
    }
    let (start, end_exclusive) = match &dimension.domain {
        ExploreExactDomain::IntRange {
            start,
            end_exclusive,
            ..
        } => (*start, *end_exclusive),
        ExploreExactDomain::Enumerated { .. } => {
            return Err(
                "source-candidate-first execution does not yet support an enumerated Int boundary axis; ordinal mapping fails closed"
                    .to_string(),
            )
        }
        ExploreExactDomain::FiniteType { .. } => {
            return Err(
                "source-candidate-first execution requires a dense Int range boundary axis"
                    .to_string(),
            )
        }
    };
    let declared_boundary =
        BoundaryInterval::new(start, end_exclusive).map_err(|error| error.to_string())?;
    let eligible_end_exclusive = i128::from(end_exclusive)
        .checked_sub(i128::from(boundary.step))
        .ok_or_else(|| "boundary endpoint eligibility arithmetic overflowed".to_string())?
        .max(i128::from(start));
    let eligible_end_exclusive = i64::try_from(eligible_end_exclusive)
        .map_err(|_| "boundary endpoint eligibility is outside Futuruna Int".to_string())?;
    let eligible_boundary =
        BoundaryInterval::new(start, eligible_end_exclusive).map_err(|error| error.to_string())?;
    let expected_query_name = query_name(query);
    let mut candidates = Vec::<BoundarySearchCandidate<SourceEventLabel>>::new();
    let mut extraction_identity = None::<(&str, &str)>;
    for extraction in extractions {
        if extraction.query_name != expected_query_name
            || extraction.axis_name != boundary.axis
            || extraction.step != boundary.step
        {
            return Err(format!(
                "source-event extraction `{}` / `{}` / step {} does not match query `{}` / `{}` / step {}",
                extraction.query_name,
                extraction.axis_name,
                extraction.step,
                expected_query_name,
                boundary.axis,
                boundary.step
            ));
        }
        if extraction.analysis_program_hash.is_empty() || extraction.query_hash.is_empty() {
            return Err(
                "source-event extraction requires nonempty program and query hashes".to_string(),
            );
        }
        match extraction_identity {
            None => {
                extraction_identity = Some((
                    extraction.analysis_program_hash.as_str(),
                    extraction.query_hash.as_str(),
                ));
            }
            Some((program_hash, query_hash))
                if program_hash == extraction.analysis_program_hash
                    && query_hash == extraction.query_hash => {}
            Some(_) => {
                return Err(
                    "source-event extractions from different program or query hashes cannot share one scheduler"
                        .to_string(),
                )
            }
        }
        validate_candidate_outer_ordinals(
            query,
            boundary.axis_dimension_index,
            &extraction.outer_ordinals,
            axis_cardinalities,
        )?;
        for candidate in extraction.candidates.iter() {
            let expected_ordinal = i128::from(candidate.boundary_value)
                .checked_sub(i128::from(start))
                .and_then(|ordinal| u128::try_from(ordinal).ok())
                .ok_or_else(|| {
                    format!(
                        "source candidate value {} has no dense ordinal in [{start}, {end_exclusive})",
                        candidate.boundary_value
                    )
                })?;
            if candidate.boundary_ordinal != expected_ordinal {
                return Err(format!(
                    "source candidate value {} claims ordinal {}, expected {}",
                    candidate.boundary_value, candidate.boundary_ordinal, expected_ordinal
                ));
            }
            if candidate.events.is_empty() {
                return Err(format!(
                    "source candidate at boundary value {} carries no source event",
                    candidate.boundary_value
                ));
            }
            for event in candidate.events.iter() {
                candidates.push(BoundarySearchCandidate::new(
                    extraction.outer_ordinals.clone(),
                    candidate.boundary_value,
                    event.label.clone(),
                ));
            }
        }
    }

    // The scheduler visits only valid lower-endpoint pairs. The certified DAG
    // exporter closes the remaining declared suffix structurally as Excluded,
    // without running user code for an endpoint that cannot form a pair.
    CandidateFirstBoundarySearch::new(
        axis_cardinalities.to_vec(),
        boundary.axis_dimension_index,
        declared_boundary,
        eligible_boundary,
        candidates,
    )
    .map_err(|error| error.to_string())
}

fn validate_candidate_outer_ordinals(
    query: &ExploreQueryIr,
    boundary_dimension: usize,
    outer_ordinals: &[u128],
    axis_cardinalities: &[u128],
) -> Result<(), String> {
    let expected = query.universe.dimensions.len().saturating_sub(1);
    if outer_ordinals.len() != expected {
        return Err(format!(
            "source-event extraction has {} outer ordinals, expected {expected}",
            outer_ordinals.len()
        ));
    }
    let mut outer_index = 0;
    for (dimension_index, dimension) in query.universe.dimensions.iter().enumerate() {
        if dimension_index == boundary_dimension {
            continue;
        }
        let cardinality = axis_cardinalities[dimension_index];
        let ordinal = outer_ordinals[outer_index];
        if ordinal >= cardinality {
            return Err(format!(
                "source-event outer ordinal {ordinal} is outside dimension `{}` cardinality {cardinality}",
                dimension.name
            ));
        }
        outer_index += 1;
    }
    Ok(())
}

fn candidate_search_evidence(cost: BoundarySearchCost) -> Result<ExploreSearchEvidence, String> {
    let scheduled = cost
        .scheduled_candidates()
        .checked_add(cost.scheduled_fallback())
        .ok_or_else(|| "source-candidate scheduled-work count exceeds u128::MAX".to_string())?;
    let evaluated = cost
        .evaluated_candidates()
        .checked_add(cost.evaluated_fallback())
        .ok_or_else(|| "source-candidate evaluated-work count exceeds u128::MAX".to_string())?;
    let pending_evaluations = scheduled
        .checked_sub(evaluated)
        .ok_or_else(|| "source-candidate evaluated work exceeds scheduled work".to_string())?;
    // The boundary scheduler operates on valid lower endpoints. Its exported
    // case DAG also closes the declared prefix/suffix that cannot form a pair;
    // include that structural proof so work evidence conserves the same U.
    let certified_region_closed_cases = cost
        .certificate_closed_cases()
        .checked_add(cost.structurally_outside_eligible_cases())
        .ok_or_else(|| "source-candidate certified-region count exceeds u128::MAX".to_string())?;
    Ok(ExploreSearchEvidence::SourceCandidateFirst {
        distinct_source_candidates: cost.distinct_candidate_cases(),
        scheduled_source_candidates: cost.scheduled_candidates(),
        evaluated_source_candidates: cost.evaluated_candidates(),
        scheduled_fallback_cases: cost.scheduled_fallback(),
        evaluated_fallback_cases: cost.evaluated_fallback(),
        singleton_closed_cases: cost.singleton_closed_cases(),
        certified_region_closed_cases,
        pending_evaluations,
        remaining_open_cases: cost.remaining_open_cases(),
        exhausted: cost.remaining_open_cases() == 0 && pending_evaluations == 0,
    })
}

fn candidate_search_trace(
    search: Option<&DenseCandidateSearch>,
) -> Result<ExactSearchTrace, String> {
    match search {
        None => Ok(ExactSearchTrace::Canonical),
        Some(search) => search
            .cost_ledger()
            .map_err(|error| error.to_string())
            .and_then(candidate_search_evidence)
            .map(ExactSearchTrace::SourceCandidateFirst),
    }
}

/// Finalize a candidate-first scheduler before any singleton evaluation while
/// retaining every certificate-backed and structural case count in both the
/// DAG and the scalar report evidence.
fn unevaluated_candidate_state(
    search: &DenseCandidateSearch,
    declared: u128,
    open_reason: CaseOpenReason,
    stop: Option<ExploreStopReason>,
) -> Result<ExactSearchState, ExactEngineFailure> {
    let case_graph = search
        .certified_case_graph(open_reason, std::iter::empty())
        .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
    let search_trace = candidate_search_trace(Some(search)).map_err(ExactEngineFailure::Error)?;
    let mut accumulator = ExactAccumulator::new();
    absorb_graph_proof_counts(&mut accumulator, &case_graph)?;
    Ok(accumulator.finish(declared, case_graph, search_trace, stop))
}

fn empty_or_open_case_graph(
    axis_cardinalities: &[u128],
    default: CaseTerminal,
) -> Result<CaseDecisionDag, String> {
    OrderedDecisionDag::from_sparse_classifications(
        axis_cardinalities.to_vec(),
        std::iter::empty::<(Vec<u128>, CaseTerminal)>(),
        default,
    )
    .map_err(|error| error.to_string())
}

fn run_canonical_order(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    question: &Expr,
    axis_cardinalities: &[u128],
    declared: u128,
    retain_ledger: bool,
    case_limit: Option<u128>,
    transition_schemas: &TransitionSchemaIdentities,
) -> Result<ExactSearchState, ExactEngineFailure> {
    let mut accumulator = ExactAccumulator::new();
    let mut graph_builder = CaseGraphBuilder::new(axis_cardinalities.to_vec());
    let mut cursor = CanonicalAssignmentCursor::new(axis_cardinalities.to_vec().into());
    let mut stop = None;
    while let Some(ordinals) = cursor.next() {
        if case_limit.is_some_and(|limit| accumulator.classified >= limit) {
            stop = Some(ExploreStopReason::CaseLimit {
                limit: case_limit.expect("case limit was present"),
            });
            break;
        }
        let observed = evaluate_and_observe_case(
            &mut accumulator,
            runtime,
            runtime_context,
            query,
            transition_schemas,
            question,
            &ordinals,
            retain_ledger,
        )?;
        graph_builder
            .classify(&ordinals, observed.terminal)
            .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
        if observed.stop.is_some() {
            stop = observed.stop;
            break;
        }
    }
    let case_graph = if stop.is_none() {
        graph_builder.finish_complete()
    } else {
        graph_builder.finish_with_remainder(CaseTerminal::EligibilityOpen(
            CaseOpenReason::SearchBudgetExhausted,
        ))
    }
    .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
    Ok(accumulator.finish(declared, case_graph, ExactSearchTrace::Canonical, stop))
}

fn run_candidate_first_order(
    runtime: &mut ExactRuntime,
    runtime_context: &ExactRuntimeContext<'_>,
    query: &ExploreQueryIr,
    question: &Expr,
    axis_cardinalities: &[u128],
    declared: u128,
    retain_ledger: bool,
    case_limit: Option<u128>,
    search: &mut DenseCandidateSearch,
    transition_schemas: &TransitionSchemaIdentities,
) -> Result<ExactSearchState, ExactEngineFailure> {
    let mut accumulator = ExactAccumulator::new();
    let mut classifications = BTreeMap::<ExploreCaseId, CaseTerminal>::new();
    let mut stop = None;
    loop {
        let cost = search
            .cost_ledger()
            .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
        if case_limit.is_some_and(|limit| {
            accumulator.classified >= limit && cost.remaining_open_cases() != 0
        }) {
            stop = Some(ExploreStopReason::CaseLimit {
                limit: case_limit.expect("case limit was present"),
            });
            break;
        }
        let work = match search
            .next_work()
            .map_err(|error| ExactEngineFailure::Error(error.to_string()))?
        {
            BoundarySearchStep::Work(work) => work,
            BoundarySearchStep::WaitingForCandidateEvaluations { pending } => {
                return Err(ExactEngineFailure::Error(format!(
                    "synchronous source-candidate executor has {pending} pending evaluations"
                )))
            }
            BoundarySearchStep::Exhausted => break,
        };
        let case_id = work.case_id().clone();
        let observed = evaluate_and_observe_case(
            &mut accumulator,
            runtime,
            runtime_context,
            query,
            transition_schemas,
            question,
            case_id.ordinals(),
            retain_ledger,
        )?;
        if classifications
            .insert(case_id.clone(), observed.terminal.clone())
            .is_some()
        {
            return Err(ExactEngineFailure::Error(
                "source-candidate scheduler evaluated one CaseId more than once".to_string(),
            ));
        }
        if observed.is_closed() {
            search
                .record_evaluation(case_id, observed.terminal)
                .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
        } else if observed.stop.is_none() {
            return Err(ExactEngineFailure::Error(
                "source-candidate evaluation remained open without an operational stop".to_string(),
            ));
        }
        if observed.stop.is_some() {
            stop = observed.stop;
            break;
        }
    }

    let cost = search
        .cost_ledger()
        .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
    if accumulator.classified != cost.singleton_closed_cases() {
        return Err(ExactEngineFailure::Error(format!(
            "source-candidate exact accumulator classified {} cases but the scheduler closed {} singletons",
            accumulator.classified,
            cost.singleton_closed_cases()
        )));
    }
    let closed_classification_identities = classifications
        .values()
        .filter(|terminal| case_terminal_is_closed(terminal))
        .count() as u128;
    if closed_classification_identities != accumulator.classified {
        return Err(ExactEngineFailure::Error(
            "source-candidate closed-classification identities disagree with the exact accumulator"
                .to_string(),
        ));
    }
    let open_reason = if stop.is_some() {
        CaseOpenReason::SearchBudgetExhausted
    } else {
        CaseOpenReason::EvaluationUnknown
    };
    let case_graph = search
        .certified_case_graph(
            open_reason,
            classifications
                .iter()
                .filter(|(_, terminal)| !case_terminal_is_closed(terminal))
                .map(|(case_id, terminal)| (case_id.clone(), terminal.clone())),
        )
        .map_err(|error| ExactEngineFailure::Error(error.to_string()))?;
    absorb_graph_proof_counts(&mut accumulator, &case_graph)?;
    let search_evidence = candidate_search_evidence(cost).map_err(ExactEngineFailure::Error)?;
    Ok(accumulator.finish(
        declared,
        case_graph,
        ExactSearchTrace::SourceCandidateFirst(search_evidence),
        stop,
    ))
}

#[cfg(test)]
mod atomic_case_tests {
    use super::*;

    fn first_representative() -> ExploreRepresentative {
        ExploreRepresentative::First {
            span: crate::Span::dummy(),
        }
    }

    fn semantic_transition() -> TransitionInstance {
        semantic_transition_to(2)
    }

    fn semantic_transition_to(after: i64) -> TransitionInstance {
        TransitionInstance::new(
            b"test.state-schema.v1".as_slice(),
            b"test.transition-schema.v1".as_slice(),
            ExploreValue::Unit,
            ExploreValue::Tuple(vec![ExploreValue::Int(1)]),
            ExploreValue::Tuple(vec![ExploreValue::Int(after)]),
        )
    }

    #[test]
    fn operational_limit_leaves_the_whole_case_eligibility_open() {
        let evaluation = finish_case_evaluation(Err(ExactEngineFailure::OperationalLimit(
            ExploreStopReason::CaseLimit { limit: 7 },
        )))
        .expect("an operational limit is evidence, not an engine failure");

        let ExactCaseEvaluation::Open(observed) = evaluation else {
            panic!("an operational limit must not yield an accepted transaction");
        };
        assert!(matches!(
            &observed.terminal,
            CaseTerminal::EligibilityOpen(CaseOpenReason::SearchBudgetExhausted)
        ));
        assert!(matches!(
            observed.stop,
            Some(ExploreStopReason::CaseLimit { limit: 7 })
        ));
    }

    #[test]
    fn stream_preparation_preserves_initialization_limit_as_retryable() {
        let stop = ExploreStopReason::RuntimeLimit {
            resource: ExploreLimitResource::Steps,
            limit: 10,
            observed: 11,
            phase: ExploreEvaluationPhase::Initialization,
        };
        let classified =
            exact_stream_prepare_error(ExactEngineFailure::OperationalLimit(stop.clone()));

        assert!(matches!(
            classified,
            ExactStreamEvaluatorPrepareError::OperationalLimit(reason) if reason == stop
        ));
    }

    #[test]
    fn excluded_and_nonmatching_cases_retain_exact_transition_support() {
        let mut accumulator = ExactAccumulator::new();
        for (ordinal, outcome) in [
            (0, ExactCaseOutcome::Excluded),
            (1, ExactCaseOutcome::AdmissibleNonmatch),
        ] {
            accept_case_transaction(
                &mut accumulator,
                &first_representative(),
                ExactCaseTransaction {
                    case_id: OrdinalCaseId(vec![ordinal].into_boxed_slice()),
                    transition: semantic_transition(),
                    outcome,
                },
            )
            .expect("closed classification should retain its transition");
        }

        assert_eq!(accumulator.classified, 2);
        assert_eq!(accumulator.admissible, 1);
        assert_eq!(accumulator.matching, 0);
        assert_eq!(accumulator.transition_support.len(), 1);
        assert_eq!(
            accumulator.transition_support.admissible_transition_count(),
            1
        );
        assert_eq!(
            accumulator.transition_support.matching_transition_count(),
            0
        );
    }

    #[test]
    fn distinct_transition_edges_are_counted_without_a_case_map() {
        let mut accumulator = ExactAccumulator::new();
        accept_case_transaction(
            &mut accumulator,
            &first_representative(),
            ExactCaseTransaction {
                case_id: OrdinalCaseId(vec![0].into_boxed_slice()),
                transition: semantic_transition(),
                outcome: ExactCaseOutcome::Excluded,
            },
        )
        .expect("first complete case should be accepted");

        accept_case_transaction(
            &mut accumulator,
            &first_representative(),
            ExactCaseTransaction {
                case_id: OrdinalCaseId(vec![1].into_boxed_slice()),
                transition: semantic_transition_to(3),
                outcome: ExactCaseOutcome::AdmissibleNonmatch,
            },
        )
        .expect("a distinct case may contribute a distinct transition");

        assert_eq!(accumulator.classified, 2);
        assert_eq!(accumulator.admissible, 1);
        assert_eq!(accumulator.transition_support.len(), 2);
        assert_eq!(
            accumulator.transition_support.admissible_transition_count(),
            1
        );
    }

    #[test]
    fn matching_acceptance_commits_counts_projection_representative_and_ledger_together() {
        let mut accumulator = ExactAccumulator::new();
        let key = vec![ExploreValue::Int(7)].into_boxed_slice();
        let case_id = OrdinalCaseId(vec![0].into_boxed_slice());
        let observed = accept_case_transaction(
            &mut accumulator,
            &first_representative(),
            ExactCaseTransaction {
                case_id: case_id.clone(),
                transition: semantic_transition(),
                outcome: ExactCaseOutcome::AdmissibleMatch(MatchingCaseTransaction::Materialized {
                    observation: SearchObservation {
                        case_id: case_id.clone(),
                        key: key.clone(),
                        extrema: vec![10].into_boxed_slice(),
                        shown: vec![ExploreValue::Int(20)].into_boxed_slice(),
                        objective: None,
                    },
                    retain_ledger: true,
                }),
            },
        )
        .expect("complete matching transaction should be accepted");

        assert!(matches!(observed.terminal, CaseTerminal::AdmissibleMatch));
        assert_eq!(accumulator.classified, 1);
        assert_eq!(accumulator.admissible, 1);
        assert_eq!(accumulator.matching, 1);
        assert_eq!(accumulator.key_supports.get(&key), Some(&1));
        assert_eq!(accumulator.key_extrema[&key][0].support, 1);
        assert_eq!(
            accumulator.selected_representatives.get(&key),
            Some(&case_id)
        );
        assert_eq!(accumulator.candidates[&key].case_id, case_id);
        assert_eq!(accumulator.ledger.len(), 1);
        assert_eq!(accumulator.transition_support.len(), 1);
        assert_eq!(
            accumulator.transition_support.matching_transition_count(),
            1
        );
    }

    #[test]
    fn failed_matching_acceptance_does_not_partially_mutate_the_accumulator() {
        let mut accumulator = ExactAccumulator::new();
        let key = vec![ExploreValue::Int(7)].into_boxed_slice();
        let incumbent = OrdinalCaseId(vec![0].into_boxed_slice());
        accumulator.classified = 3;
        accumulator.admissible = 2;
        accumulator.matching = 1;
        accumulator.keys_seen.insert(key.clone());
        accumulator.key_supports.insert(key.clone(), 1);
        accumulator.key_extrema.insert(
            key.clone(),
            vec![ExactExtremaAccumulator {
                minimum: 10,
                maximum: 10,
                support: u128::MAX,
                minimum_tie_support: 1,
                maximum_tie_support: 1,
                minimum_witness: incumbent.clone(),
                maximum_witness: incumbent.clone(),
            }]
            .into_boxed_slice(),
        );
        accumulator
            .selected_representatives
            .insert(key.clone(), incumbent);

        let result = accept_case_transaction(
            &mut accumulator,
            &first_representative(),
            ExactCaseTransaction {
                case_id: OrdinalCaseId(vec![1].into_boxed_slice()),
                transition: semantic_transition(),
                outcome: ExactCaseOutcome::AdmissibleMatch(
                    MatchingCaseTransaction::ProjectionOnly {
                        case_id: OrdinalCaseId(vec![1].into_boxed_slice()),
                        key: key.clone(),
                        extrema: vec![10].into_boxed_slice(),
                    },
                ),
            },
        );

        assert!(matches!(result, Err(ExactEngineFailure::Error(_))));
        assert_eq!(accumulator.classified, 3);
        assert_eq!(accumulator.admissible, 2);
        assert_eq!(accumulator.matching, 1);
        assert_eq!(accumulator.key_supports.get(&key), Some(&1));
        assert_eq!(accumulator.key_extrema[&key][0].support, u128::MAX);
        assert_eq!(accumulator.keys_seen.len(), 1);
        assert!(accumulator.candidates.is_empty());
        assert!(accumulator.ledger.is_empty());
        assert!(accumulator.transition_support.is_empty());
    }
}

#[cfg(test)]
mod candidate_first_tests {
    use super::*;

    fn open_candidate_search() -> ExploreSearchEvidence {
        ExploreSearchEvidence::SourceCandidateFirst {
            distinct_source_candidates: 1,
            scheduled_source_candidates: 1,
            evaluated_source_candidates: 1,
            scheduled_fallback_cases: 0,
            evaluated_fallback_cases: 0,
            singleton_closed_cases: 1,
            certified_region_closed_cases: 0,
            pending_evaluations: 0,
            remaining_open_cases: 9,
            exhausted: false,
        }
    }

    #[test]
    fn candidate_first_withholds_first_rows_until_projection_closes() {
        assert!(!first_representatives_closed(
            ExactSearchTrace::SourceCandidateFirst(open_candidate_search()),
            false,
        ));
        assert!(first_representatives_closed(
            ExactSearchTrace::SourceCandidateFirst(open_candidate_search()),
            true,
        ));
        assert!(first_representatives_closed(
            ExactSearchTrace::Canonical,
            false,
        ));
    }
}
