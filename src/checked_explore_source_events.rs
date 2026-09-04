//! Compiler-minted, solver-neutral rule/source boundary candidates.
//!
//! This inventory is deliberately advisory. A supported checked rule atom can
//! nominate an affine source boundary; every unsupported path becomes a
//! durable residual and contributes no cut. Concrete enumeration therefore
//! remains the completeness authority.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    checked_explore_semantic_binders, checked_explore_semantic_dependency_root_digest, explore,
    named_arg_parts, typed_rule_head_argument, visit_ast_expr_children, AnalysisProgramId,
    AstChild, CheckedBinderKind, CheckedBinderSiteId, CheckedCallTarget,
    CheckedExploreQueryArtifactIssue, CheckedExploreQuerySites, CheckedExploreSemanticDependency,
    CheckedExploreSemanticIndex, CheckedResolutionArtifacts, CheckedRuleCandidateResolution,
    CheckedValueBinding, ExprKind, ExprSiteId, Literal, RuleDispatchKey, RuleDispatchTier, Ty,
};

pub(crate) const CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION: u32 = 1;

// Work counts entered syntax/call-expansion and condition nodes. Outputs count
// pre-dedup events and residuals; the last slot is reserved for exhaustion.
const SOURCE_EVENT_EXTRACTION_WORK_LIMIT: u32 = 65_536;
const SOURCE_EVENT_EXTRACTION_OUTPUT_LIMIT: u32 = 8_192;

const SOURCE_EVENT_DOMAIN: &[u8] = b"futuruna.checked-explore-source-event.v1\0";
const OCCURRENCE_DOMAIN: &[u8] = b"futuruna.checked-explore-boundary-occurrence.v1\0";
const RESIDUAL_DOMAIN: &[u8] = b"futuruna.checked-explore-source-event-residual.v1\0";
const INVENTORY_DOMAIN: &[u8] = b"futuruna.checked-explore-source-event-inventory.v1\0";
const ROUTE_DOMAIN: &[u8] = b"futuruna.checked-explore-source-event-route.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CheckedExploreSourceEventId([u8; 32]);

impl CheckedExploreSourceEventId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CheckedExploreBoundaryOccurrenceId([u8; 32]);

impl CheckedExploreBoundaryOccurrenceId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CheckedExploreSourceEventResidualId([u8; 32]);

impl CheckedExploreSourceEventResidualId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CheckedExploreSourceEventRelation {
    Less,
    LessOrEqual,
    Equal,
    NotEqual,
    GreaterOrEqual,
    Greater,
}

impl CheckedExploreSourceEventRelation {
    fn tag(self) -> u8 {
        match self {
            Self::Less => 0x01,
            Self::LessOrEqual => 0x02,
            Self::Equal => 0x03,
            Self::NotEqual => 0x04,
            Self::GreaterOrEqual => 0x05,
            Self::Greater => 0x06,
        }
    }

    fn reverse(self) -> Self {
        match self {
            Self::Less => Self::Greater,
            Self::LessOrEqual => Self::GreaterOrEqual,
            Self::Equal => Self::Equal,
            Self::NotEqual => Self::NotEqual,
            Self::GreaterOrEqual => Self::LessOrEqual,
            Self::Greater => Self::Less,
        }
    }

    fn negate(self) -> Self {
        match self {
            Self::Less => Self::GreaterOrEqual,
            Self::LessOrEqual => Self::Greater,
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::GreaterOrEqual => Self::Less,
            Self::Greater => Self::LessOrEqual,
        }
    }
}

/// Semantic layer containing the source-linked call occurrence. Authored
/// names and source locations are intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CheckedExploreSourceEventLayer {
    SourceBinding {
        binding_index: u32,
    },
    Successor,
    Admission {
        admission_id: explore::AdmissionId,
        admission_index: u32,
    },
    Find {
        question_id: explore::QuestionId,
    },
}

/// Checked source provenance retained as an annotation beside the stable,
/// name-free event identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedExploreSourceEventOrigin {
    pub(crate) family_digest: [u8; 32],
    pub(crate) candidate_ordinal: u32,
    pub(crate) tier: RuleDispatchTier,
    pub(crate) condition_atom_path: Box<[u32]>,
    pub(crate) call_route_digest: [u8; 32],
    pub(crate) call_site: ExprSiteId,
    pub(crate) candidate: CheckedRuleCandidateResolution,
}

/// One exact affine source atom after checked rule-head binder substitution.
/// It denotes `coefficient * source[binding_index] + intercept REL 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedExploreAffineSourceEvent {
    pub(crate) occurrence_id: CheckedExploreBoundaryOccurrenceId,
    pub(crate) source_event_id: CheckedExploreSourceEventId,
    pub(crate) layer: CheckedExploreSourceEventLayer,
    pub(crate) source_binding_index: u32,
    pub(crate) coefficient: i128,
    pub(crate) intercept: i128,
    pub(crate) relation: CheckedExploreSourceEventRelation,
    pub(crate) origin: CheckedExploreSourceEventOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CheckedExploreSourceEventUnsupportedShape {
    SourceDomain,
    RuleHead,
    Condition,
    Expression,
    Call,
    Successor,
}

/// Every reason that prevents this conservative producer from minting a cut.
/// The variants contain only semantic numeric facts, never spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CheckedExploreSourceEventResidualReason {
    MissingExpressionResolution,
    UnsupportedSourceAxis,
    UnresolvedOrDynamicCall,
    CallableUnsealed,
    RuleFamilyMissing,
    RuleFamilyUnsealed,
    NamedArgumentMismatch { expected: u32, actual: u32 },
    ArityMismatch { expected: u32, actual: u32 },
    RuleHeadPatternUnsupported,
    RuleHeadBinderMismatch,
    ScopedReceiverNeedsBinding,
    RecursiveCall,
    EffectfulCallable,
    OpenCapture,
    MultipleAxes { count: u32 },
    NonAffine,
    ArithmeticOverflow,
    UnsupportedShape(CheckedExploreSourceEventUnsupportedShape),
    ExtractionWorkBudgetExceeded { limit: u32 },
    ExtractionOutputBudgetExceeded { limit: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedExploreSourceEventResidual {
    pub(crate) residual_id: CheckedExploreSourceEventResidualId,
    pub(crate) layer: CheckedExploreSourceEventLayer,
    pub(crate) route_digest: [u8; 32],
    pub(crate) dependency_digest: Option<[u8; 32]>,
    pub(crate) reason: CheckedExploreSourceEventResidualReason,
    /// Diagnostic-only checked source address. It is excluded from identity.
    pub(crate) site: Option<ExprSiteId>,
}

/// Immutable producer result retained by the checked query and its owned
/// continuation. Neither events nor residuals authorize pruning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedExploreSourceEventInventory {
    version: u32,
    analysis_program: AnalysisProgramId,
    relation_id: explore::RelationId,
    admission_id: explore::AdmissionId,
    question_ids: Box<[explore::QuestionId]>,
    source_binding_count: u32,
    admission_count: u32,
    events: Box<[CheckedExploreAffineSourceEvent]>,
    residuals: Box<[CheckedExploreSourceEventResidual]>,
    inventory_root: [u8; 32],
}

impl CheckedExploreSourceEventInventory {
    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn relation_id(&self) -> explore::RelationId {
        self.relation_id
    }

    pub(crate) const fn admission_id(&self) -> explore::AdmissionId {
        self.admission_id
    }

    pub(crate) fn question_ids(&self) -> &[explore::QuestionId] {
        &self.question_ids
    }

    pub(crate) fn events(&self) -> &[CheckedExploreAffineSourceEvent] {
        &self.events
    }

    pub(crate) fn residuals(&self) -> &[CheckedExploreSourceEventResidual] {
        &self.residuals
    }

    pub(crate) const fn inventory_root(&self) -> [u8; 32] {
        self.inventory_root
    }

    pub(crate) fn validate_identity(&self) -> bool {
        if self.version != CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION
            || !strictly_sorted_by(&self.question_ids, |question| question.bytes())
            || !strictly_sorted_by(&self.events, |event| event.occurrence_id.bytes())
            || !strictly_sorted_by(&self.residuals, |residual| residual.residual_id.bytes())
        {
            return false;
        }
        for event in self.events.iter() {
            if event.source_binding_index >= self.source_binding_count
                || !self.layer_is_valid(event.layer)
                || event.origin.call_site.analysis_program != self.analysis_program
                || event.origin.candidate.head_site.analysis_program != self.analysis_program
                || event
                    .origin
                    .candidate
                    .condition_site
                    .as_ref()
                    .is_some_and(|site| site.analysis_program != self.analysis_program)
                || event.origin.candidate.tier != event.origin.tier
                || source_event_id(&event.origin, event.relation) != event.source_event_id
                || occurrence_id(event) != event.occurrence_id
            {
                return false;
            }
        }
        for residual in self.residuals.iter() {
            if !self.layer_is_valid(residual.layer)
                || residual
                    .site
                    .as_ref()
                    .is_some_and(|site| site.analysis_program != self.analysis_program)
                || residual_id(
                    self.relation_id,
                    residual.layer,
                    residual.route_digest,
                    residual.dependency_digest,
                    residual.reason,
                ) != residual.residual_id
            {
                return false;
            }
        }
        inventory_root(self) == self.inventory_root
    }

    fn layer_is_valid(&self, layer: CheckedExploreSourceEventLayer) -> bool {
        match layer {
            CheckedExploreSourceEventLayer::SourceBinding { binding_index } => {
                binding_index < self.source_binding_count
            }
            CheckedExploreSourceEventLayer::Successor => true,
            CheckedExploreSourceEventLayer::Admission {
                admission_id,
                admission_index,
            } => admission_id == self.admission_id && admission_index < self.admission_count,
            CheckedExploreSourceEventLayer::Find { question_id } => {
                self.question_ids.binary_search(&question_id).is_ok()
            }
        }
    }
}

fn strictly_sorted_by<T, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> bool {
    items.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AffineTerm {
    coefficients: BTreeMap<u32, i128>,
    intercept: i128,
    minimum: i128,
    maximum: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AffineError {
    MissingResolution,
    UnsupportedSourceAxis,
    UnsupportedSourceDomain,
    BinderMismatch,
    OpenCapture,
    NonAffine,
    Overflow,
    UnsupportedShape,
}

type AffineValue = Result<AffineTerm, AffineError>;
type AffineEnvironment = BTreeMap<CheckedBinderSiteId, AffineValue>;

#[derive(Debug, Clone, Copy)]
struct SourceEventExtractionLimits {
    work_items: u32,
    output_items: u32,
}

impl SourceEventExtractionLimits {
    const DEFAULT: Self = Self {
        work_items: SOURCE_EVENT_EXTRACTION_WORK_LIMIT,
        output_items: SOURCE_EVENT_EXTRACTION_OUTPUT_LIMIT,
    };
}

impl AffineTerm {
    fn constant(value: i64) -> Self {
        Self {
            coefficients: BTreeMap::new(),
            intercept: i128::from(value),
            minimum: i128::from(value),
            maximum: i128::from(value),
        }
    }

    fn axis(binding_index: u32, start: i64, end_exclusive: i64) -> AffineValue {
        let maximum = end_exclusive.checked_sub(1).ok_or(AffineError::Overflow)?;
        if start > maximum {
            return Err(AffineError::UnsupportedSourceAxis);
        }
        Self::from_parts(
            BTreeMap::from([(binding_index, 1)]),
            0,
            i128::from(start),
            i128::from(maximum),
        )
    }

    fn from_parts(
        coefficients: BTreeMap<u32, i128>,
        intercept: i128,
        minimum: i128,
        maximum: i128,
    ) -> AffineValue {
        if minimum < i128::from(i64::MIN) || maximum > i128::from(i64::MAX) {
            return Err(AffineError::Overflow);
        }
        Ok(Self {
            coefficients,
            intercept,
            minimum,
            maximum,
        })
    }

    fn add(&self, other: &Self) -> AffineValue {
        let coefficients = combine_coefficients(&self.coefficients, &other.coefficients, false)?;
        Self::from_parts(
            coefficients,
            self.intercept
                .checked_add(other.intercept)
                .ok_or(AffineError::Overflow)?,
            self.minimum
                .checked_add(other.minimum)
                .ok_or(AffineError::Overflow)?,
            self.maximum
                .checked_add(other.maximum)
                .ok_or(AffineError::Overflow)?,
        )
    }

    fn subtract(&self, other: &Self) -> AffineValue {
        let coefficients = combine_coefficients(&self.coefficients, &other.coefficients, true)?;
        Self::from_parts(
            coefficients,
            self.intercept
                .checked_sub(other.intercept)
                .ok_or(AffineError::Overflow)?,
            self.minimum
                .checked_sub(other.maximum)
                .ok_or(AffineError::Overflow)?,
            self.maximum
                .checked_sub(other.minimum)
                .ok_or(AffineError::Overflow)?,
        )
    }

    fn negate(&self) -> AffineValue {
        let coefficients = self
            .coefficients
            .iter()
            .map(|(axis, coefficient)| {
                coefficient
                    .checked_neg()
                    .map(|coefficient| (*axis, coefficient))
                    .ok_or(AffineError::Overflow)
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Self::from_parts(
            coefficients,
            self.intercept.checked_neg().ok_or(AffineError::Overflow)?,
            self.maximum.checked_neg().ok_or(AffineError::Overflow)?,
            self.minimum.checked_neg().ok_or(AffineError::Overflow)?,
        )
    }

    fn multiply_constant(&self, constant: i128) -> AffineValue {
        if constant == 0 {
            return Ok(Self::constant(0));
        }
        let coefficients = self
            .coefficients
            .iter()
            .map(|(axis, coefficient)| {
                coefficient
                    .checked_mul(constant)
                    .map(|coefficient| (*axis, coefficient))
                    .ok_or(AffineError::Overflow)
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let left = self
            .minimum
            .checked_mul(constant)
            .ok_or(AffineError::Overflow)?;
        let right = self
            .maximum
            .checked_mul(constant)
            .ok_or(AffineError::Overflow)?;
        Self::from_parts(
            coefficients,
            self.intercept
                .checked_mul(constant)
                .ok_or(AffineError::Overflow)?,
            left.min(right),
            left.max(right),
        )
    }

    fn constant_value(&self) -> Option<i128> {
        self.coefficients.is_empty().then_some(self.intercept)
    }
}

fn combine_coefficients(
    left: &BTreeMap<u32, i128>,
    right: &BTreeMap<u32, i128>,
    subtract: bool,
) -> Result<BTreeMap<u32, i128>, AffineError> {
    let mut combined = left.clone();
    for (axis, coefficient) in right {
        let coefficient = if subtract {
            coefficient.checked_neg().ok_or(AffineError::Overflow)?
        } else {
            *coefficient
        };
        let next = combined
            .get(axis)
            .copied()
            .unwrap_or(0)
            .checked_add(coefficient)
            .ok_or(AffineError::Overflow)?;
        if next == 0 {
            combined.remove(axis);
        } else {
            combined.insert(*axis, next);
        }
    }
    Ok(combined)
}

struct SourceEventProducer<'a, 'program> {
    resolutions: &'a CheckedResolutionArtifacts,
    index: &'a CheckedExploreSemanticIndex<'program>,
    semantic_binders: BTreeMap<CheckedBinderSiteId, Box<str>>,
    relation_id: explore::RelationId,
    events: Vec<CheckedExploreAffineSourceEvent>,
    residuals: Vec<CheckedExploreSourceEventResidual>,
    family_digests: BTreeMap<RuleDispatchKey, Option<[u8; 32]>>,
    callable_digests: BTreeMap<crate::CheckedCallableId, Option<[u8; 32]>>,
    active_families: BTreeSet<RuleDispatchKey>,
    active_callables: BTreeSet<crate::CheckedCallableId>,
    limits: SourceEventExtractionLimits,
    remaining_work_items: u32,
    remaining_output_items: u32,
    extraction_halted: bool,
}

pub(crate) fn checked_explore_source_event_inventory(
    program: &crate::CheckedAnalysisProgram,
    resolutions: &CheckedResolutionArtifacts,
    query: &explore::ExploreQueryIr,
    sites: &CheckedExploreQuerySites,
    relation_id: explore::RelationId,
    admission_id: explore::AdmissionId,
    find_question_ids: &[explore::QuestionId],
    index: &CheckedExploreSemanticIndex<'_>,
) -> Result<CheckedExploreSourceEventInventory, CheckedExploreQueryArtifactIssue> {
    checked_explore_source_event_inventory_with_limits(
        program,
        resolutions,
        query,
        sites,
        relation_id,
        admission_id,
        find_question_ids,
        index,
        SourceEventExtractionLimits::DEFAULT,
    )
}

#[allow(clippy::too_many_arguments)]
fn checked_explore_source_event_inventory_with_limits(
    program: &crate::CheckedAnalysisProgram,
    resolutions: &CheckedResolutionArtifacts,
    query: &explore::ExploreQueryIr,
    sites: &CheckedExploreQuerySites,
    relation_id: explore::RelationId,
    admission_id: explore::AdmissionId,
    find_question_ids: &[explore::QuestionId],
    index: &CheckedExploreSemanticIndex<'_>,
    limits: SourceEventExtractionLimits,
) -> Result<CheckedExploreSourceEventInventory, CheckedExploreQueryArtifactIssue> {
    assert!(
        limits.work_items > 0 && limits.output_items > 0,
        "source-event extraction limits must reserve terminal residual capacity"
    );
    if query.source.bindings.len() != sites.source_bindings.len()
        || query.admissions.len() != sites.admissions.len()
        || query.finds.len() != sites.find_predicates.len()
        || query.finds.len() != find_question_ids.len()
    {
        return Err(CheckedExploreQueryArtifactIssue::AnalysisGraph(
            "source-event inventory inputs diverged".into(),
        ));
    }
    let semantic_binders = checked_explore_semantic_binders(query, sites)?;
    let mut producer = SourceEventProducer {
        resolutions,
        index,
        semantic_binders,
        relation_id,
        events: Vec::new(),
        residuals: Vec::new(),
        family_digests: BTreeMap::new(),
        callable_digests: BTreeMap::new(),
        active_families: BTreeSet::new(),
        active_callables: BTreeSet::new(),
        limits,
        remaining_work_items: limits.work_items,
        remaining_output_items: limits.output_items,
        extraction_halted: false,
    };
    let mut environment = AffineEnvironment::new();

    for (binding, checked) in query
        .source
        .bindings
        .iter()
        .zip(sites.source_bindings.iter())
    {
        if producer.extraction_halted() {
            break;
        }
        let binding_index = u32::try_from(binding.binding_index).map_err(|_| {
            CheckedExploreQueryArtifactIssue::AnalysisGraph(
                "source binding index exceeds the source-event ABI".into(),
            )
        })?;
        let layer = CheckedExploreSourceEventLayer::SourceBinding { binding_index };
        let route = base_route(relation_id, layer);
        producer.walk_expression(&checked.expression, &environment, layer, route, 0);
        if producer.extraction_halted() {
            break;
        }
        let value = match &binding.kind {
            explore::ExploreSourceBindingKindIr::Singleton { .. } => {
                producer.affine_expression(&checked.expression, &environment)
            }
            explore::ExploreSourceBindingKindIr::Finite { domain } => {
                let value = producer.source_axis_term(
                    binding_index,
                    &binding.value_ty,
                    domain,
                    &checked.expression,
                    &environment,
                );
                if let Err(error) = value {
                    producer.record_residual(
                        layer,
                        extend_route(route, 0x08, 0, None),
                        Some(checked.expression.clone()),
                        None,
                        residual_from_affine_error(error),
                    );
                }
                value
            }
        };
        environment.insert(checked.binder.clone(), value);
    }

    if !producer.extraction_halted() {
        let successor_layer = CheckedExploreSourceEventLayer::Successor;
        let successor_route = base_route(relation_id, successor_layer);
        producer.walk_expression(
            &sites.successor,
            &environment,
            successor_layer,
            successor_route,
            0,
        );
        if !producer.extraction_halted() {
            let after_value = match &query.successor.kind {
                explore::ExploreSuccessorKindIr::Singleton { .. } => {
                    producer.affine_expression(&sites.successor, &environment)
                }
                explore::ExploreSuccessorKindIr::Finite { .. } => {
                    producer.record_residual(
                        successor_layer,
                        extend_route(successor_route, 0x09, 0, None),
                        Some(sites.successor.clone()),
                        None,
                        CheckedExploreSourceEventResidualReason::UnsupportedShape(
                            CheckedExploreSourceEventUnsupportedShape::Successor,
                        ),
                    );
                    Err(AffineError::UnsupportedShape)
                }
            };
            if let Some((after_binder, _)) = producer
                .semantic_binders
                .iter()
                .find(|(_, role)| role.as_ref() == "successor-after")
            {
                environment.insert(after_binder.clone(), after_value);
            }
        }
    }

    for (admission, site) in query.admissions.iter().zip(sites.admissions.iter()) {
        if producer.extraction_halted() {
            break;
        }
        let admission_index = u32::try_from(admission.admission_index).map_err(|_| {
            CheckedExploreQueryArtifactIssue::AnalysisGraph(
                "admission index exceeds the source-event ABI".into(),
            )
        })?;
        let layer = CheckedExploreSourceEventLayer::Admission {
            admission_id,
            admission_index,
        };
        producer.walk_expression(site, &environment, layer, base_route(relation_id, layer), 0);
    }
    for ((find, site), question_id) in query
        .finds
        .iter()
        .zip(sites.find_predicates.iter())
        .zip(find_question_ids.iter().copied())
    {
        if producer.extraction_halted() {
            break;
        }
        if find.find.predicate().is_none() {
            continue;
        }
        let Some(site) = site else {
            return Err(CheckedExploreQueryArtifactIssue::AnalysisGraph(
                "FIND predicate and checked source-event site diverged".into(),
            ));
        };
        let layer = CheckedExploreSourceEventLayer::Find { question_id };
        producer.walk_expression(site, &environment, layer, base_route(relation_id, layer), 0);
    }

    producer
        .events
        .sort_by_key(|event| event.occurrence_id.bytes());
    producer
        .events
        .dedup_by_key(|event| event.occurrence_id.bytes());
    producer
        .residuals
        .sort_by_key(|residual| residual.residual_id.bytes());
    producer
        .residuals
        .dedup_by_key(|residual| residual.residual_id.bytes());
    let mut question_ids = find_question_ids.to_vec();
    question_ids.sort_unstable();
    question_ids.dedup();
    let mut inventory = CheckedExploreSourceEventInventory {
        version: CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION,
        analysis_program: program.id.clone(),
        relation_id,
        admission_id,
        question_ids: question_ids.into_boxed_slice(),
        source_binding_count: u32::try_from(query.source.bindings.len()).map_err(|_| {
            CheckedExploreQueryArtifactIssue::AnalysisGraph(
                "source binding count exceeds the source-event ABI".into(),
            )
        })?,
        admission_count: u32::try_from(query.admissions.len()).map_err(|_| {
            CheckedExploreQueryArtifactIssue::AnalysisGraph(
                "admission count exceeds the source-event ABI".into(),
            )
        })?,
        events: producer.events.into_boxed_slice(),
        residuals: producer.residuals.into_boxed_slice(),
        inventory_root: [0; 32],
    };
    inventory.inventory_root = inventory_root(&inventory);
    if !inventory.validate_identity() {
        return Err(CheckedExploreQueryArtifactIssue::AnalysisGraph(
            "source-event inventory failed its producer identity check".into(),
        ));
    }
    Ok(inventory)
}

impl SourceEventProducer<'_, '_> {
    fn extraction_halted(&self) -> bool {
        self.extraction_halted
    }

    fn consume_work_item(
        &mut self,
        layer: CheckedExploreSourceEventLayer,
        route_digest: [u8; 32],
        site: Option<ExprSiteId>,
        dependency_digest: Option<[u8; 32]>,
    ) -> bool {
        if self.extraction_halted {
            return false;
        }
        if self.remaining_work_items == 0 {
            self.halt_extraction(
                layer,
                route_digest,
                site,
                dependency_digest,
                CheckedExploreSourceEventResidualReason::ExtractionWorkBudgetExceeded {
                    limit: self.limits.work_items,
                },
            );
            return false;
        }
        self.remaining_work_items -= 1;
        true
    }

    fn reserve_output_item(
        &mut self,
        layer: CheckedExploreSourceEventLayer,
        route_digest: [u8; 32],
        site: Option<ExprSiteId>,
        dependency_digest: Option<[u8; 32]>,
    ) -> bool {
        if self.extraction_halted {
            return false;
        }
        // The final slot is reserved for the explicit terminal residual.
        if self.remaining_output_items <= 1 {
            self.halt_extraction(
                layer,
                route_digest,
                site,
                dependency_digest,
                CheckedExploreSourceEventResidualReason::ExtractionOutputBudgetExceeded {
                    limit: self.limits.output_items,
                },
            );
            return false;
        }
        self.remaining_output_items -= 1;
        true
    }

    fn halt_extraction(
        &mut self,
        layer: CheckedExploreSourceEventLayer,
        route_digest: [u8; 32],
        site: Option<ExprSiteId>,
        dependency_digest: Option<[u8; 32]>,
        reason: CheckedExploreSourceEventResidualReason,
    ) {
        if self.extraction_halted {
            return;
        }
        self.extraction_halted = true;
        self.remaining_output_items = self.remaining_output_items.saturating_sub(1);
        self.residuals.push(CheckedExploreSourceEventResidual {
            residual_id: residual_id(
                self.relation_id,
                layer,
                route_digest,
                dependency_digest,
                reason,
            ),
            layer,
            route_digest,
            dependency_digest,
            reason,
            site,
        });
    }

    fn source_axis_term(
        &self,
        binding_index: u32,
        ty: &Ty,
        domain: &explore::ExploreFiniteDomainIr,
        site: &ExprSiteId,
        environment: &AffineEnvironment,
    ) -> AffineValue {
        if !matches!(ty, Ty::Name(name) if name == "Int") {
            return Err(AffineError::UnsupportedSourceAxis);
        }
        match domain {
            explore::ExploreFiniteDomainIr::Exact(explore::ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                cardinality,
            }) if u128::from(*cardinality)
                == i128::from(*end_exclusive)
                    .checked_sub(i128::from(*start))
                    .and_then(|value| u128::try_from(value).ok())
                    .unwrap_or(u128::MAX) =>
            {
                AffineTerm::axis(binding_index, *start, *end_exclusive)
            }
            explore::ExploreFiniteDomainIr::IntRange { .. } => {
                let start = self
                    .affine_expression(&child_site(site, 1), environment)?
                    .constant_value()
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or(AffineError::UnsupportedSourceAxis)?;
                let end = self
                    .affine_expression(&child_site(site, 2), environment)?
                    .constant_value()
                    .and_then(|value| i64::try_from(value).ok())
                    .ok_or(AffineError::UnsupportedSourceAxis)?;
                AffineTerm::axis(binding_index, start, end)
            }
            _ => Err(AffineError::UnsupportedSourceDomain),
        }
    }

    fn affine_expression(&self, site: &ExprSiteId, environment: &AffineEnvironment) -> AffineValue {
        if self.resolutions.unsupported_sites.get(site).is_some() {
            return Err(AffineError::MissingResolution);
        }
        let resolution = self
            .resolutions
            .expressions
            .get(site)
            .ok_or(AffineError::MissingResolution)?;
        let expression = self
            .index
            .expression(site)
            .ok_or(AffineError::MissingResolution)?;
        match &expression.kind {
            ExprKind::Lit(Literal::Int(value)) => Ok(AffineTerm::constant(*value)),
            ExprKind::Var(_) => match resolution.value_binding.as_ref() {
                Some(CheckedValueBinding::Binder { kind, site }) => environment
                    .get(site)
                    .cloned()
                    .unwrap_or(Err(if *kind == CheckedBinderKind::RuleHead {
                        AffineError::BinderMismatch
                    } else {
                        AffineError::OpenCapture
                    })),
                _ => Err(AffineError::OpenCapture),
            },
            ExprKind::UnOp(operator, _) if operator == "+" => {
                self.affine_expression(&child_site(site, 0), environment)
            }
            ExprKind::UnOp(operator, _) if operator == "-" => self
                .affine_expression(&child_site(site, 0), environment)?
                .negate(),
            ExprKind::BinOp(operator, _, _) if operator == "+" || operator == "-" => {
                let left = self.affine_expression(&child_site(site, 0), environment)?;
                let right = self.affine_expression(&child_site(site, 1), environment)?;
                if operator == "+" {
                    left.add(&right)
                } else {
                    left.subtract(&right)
                }
            }
            ExprKind::BinOp(operator, _, _) if operator == "*" => {
                let left = self.affine_expression(&child_site(site, 0), environment)?;
                let right = self.affine_expression(&child_site(site, 1), environment)?;
                if let Some(constant) = left.constant_value() {
                    right.multiply_constant(constant)
                } else if let Some(constant) = right.constant_value() {
                    left.multiply_constant(constant)
                } else {
                    Err(AffineError::NonAffine)
                }
            }
            ExprKind::App(_, _) => Err(AffineError::NonAffine),
            ExprKind::BinOp(_, _, _) => Err(AffineError::NonAffine),
            ExprKind::Lit(_) | ExprKind::Unit => Err(AffineError::UnsupportedShape),
            _ => Err(AffineError::UnsupportedShape),
        }
    }

    fn walk_expression(
        &mut self,
        site: &ExprSiteId,
        environment: &AffineEnvironment,
        layer: CheckedExploreSourceEventLayer,
        route: [u8; 32],
        depth: usize,
    ) {
        if !self.consume_work_item(layer, route, Some(site.clone()), None) {
            return;
        }
        if depth > 128 {
            self.record_residual(
                layer,
                route,
                Some(site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::RecursiveCall,
            );
            return;
        }
        let Some(expression) = self.index.expression(site).cloned() else {
            self.record_residual(
                layer,
                route,
                Some(site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::MissingExpressionResolution,
            );
            return;
        };
        let is_application = matches!(expression.kind, ExprKind::App(_, _));
        let call_target = self
            .resolutions
            .expressions
            .get(site)
            .and_then(|resolution| resolution.call_target.clone());
        match call_target {
            Some(CheckedCallTarget::RuleFamily(family)) => {
                self.walk_rule_family(site, &family, environment, layer, route, depth)
            }
            Some(CheckedCallTarget::Function {
                callable, arity, ..
            }) => self.walk_callable(site, &callable, arity, environment, layer, route, depth),
            Some(CheckedCallTarget::BoundCallable { .. }) => self.record_residual(
                layer,
                route,
                Some(site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::UnresolvedOrDynamicCall,
            ),
            Some(CheckedCallTarget::ScopedMember { .. }) => self.record_residual(
                layer,
                route,
                Some(site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::ScopedReceiverNeedsBinding,
            ),
            Some(CheckedCallTarget::Builtin { .. })
            | Some(CheckedCallTarget::Constructor { .. }) => {}
            None if is_application => self.record_residual(
                layer,
                route,
                Some(site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::UnresolvedOrDynamicCall,
            ),
            None => {}
        }
        if self.extraction_halted {
            return;
        }

        if let ExprKind::App(_, arguments) = &expression.kind {
            self.walk_expression(
                &child_site(site, 0),
                environment,
                layer,
                extend_route(route, 0x01, 0, None),
                depth,
            );
            if self.extraction_halted {
                return;
            }
            let actuals = match self.canonical_call_arguments(site, arguments.len()) {
                Ok(actuals) => actuals,
                Err(reason) => {
                    self.record_residual(
                        layer,
                        extend_route(route, 0x01, u32::MAX, None),
                        Some(site.clone()),
                        None,
                        reason,
                    );
                    return;
                }
            };
            for (canonical_index, actual) in actuals.iter().enumerate() {
                if self.extraction_halted {
                    return;
                }
                let Ok(route_index) = u32::try_from(canonical_index.saturating_add(1)) else {
                    self.record_residual(
                        layer,
                        extend_route(route, 0x01, u32::MAX, None),
                        Some(site.clone()),
                        None,
                        CheckedExploreSourceEventResidualReason::ArithmeticOverflow,
                    );
                    return;
                };
                self.walk_expression(
                    actual,
                    environment,
                    layer,
                    extend_route(route, 0x01, route_index, None),
                    depth,
                );
            }
            return;
        }

        let mut expression_children = Vec::new();
        let mut child_index = 0_u32;
        visit_ast_expr_children(&expression, &mut |child| {
            if matches!(child, AstChild::Expr(_)) {
                expression_children.push(child_index);
            }
            child_index = child_index.saturating_add(1);
        });

        for child_index in expression_children {
            if self.extraction_halted {
                return;
            }
            let child = child_site(site, child_index);
            self.walk_expression(
                &child,
                environment,
                layer,
                extend_route(route, 0x01, child_index, None),
                depth,
            );
        }
    }

    fn walk_callable(
        &mut self,
        call_site: &ExprSiteId,
        callable: &crate::CheckedCallableId,
        arity: usize,
        environment: &AffineEnvironment,
        layer: CheckedExploreSourceEventLayer,
        route: [u8; 32],
        depth: usize,
    ) {
        let Some(descriptor) = self.index.callables.get(callable).cloned() else {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::CallableUnsealed,
            );
            return;
        };
        if !descriptor.effects.is_empty() {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::EffectfulCallable,
            );
            return;
        }
        if !self.active_callables.insert(callable.clone()) {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::RecursiveCall,
            );
            return;
        }
        let digest = self.callable_digest(callable);
        let Some(digest) = digest else {
            self.active_callables.remove(callable);
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::CallableUnsealed,
            );
            return;
        };
        let result = (|| {
            let actuals = self.canonical_call_arguments(call_site, arity)?;
            if actuals.len() != descriptor.parameter_sites.len() {
                return Err(CheckedExploreSourceEventResidualReason::ArityMismatch {
                    expected: u32::try_from(descriptor.parameter_sites.len()).unwrap_or(u32::MAX),
                    actual: u32::try_from(actuals.len()).unwrap_or(u32::MAX),
                });
            }
            let mut call_environment = environment.clone();
            for (binder, actual) in descriptor.parameter_sites.iter().zip(actuals.iter()) {
                call_environment
                    .insert(binder.clone(), self.affine_expression(actual, environment));
            }
            let callable_route = extend_route(route, 0x03, 0, Some(digest));
            self.walk_expression(
                &descriptor.body_site,
                &call_environment,
                layer,
                callable_route,
                depth.saturating_add(1),
            );
            Ok(())
        })();
        self.active_callables.remove(callable);
        if let Err(reason) = result {
            self.record_residual(layer, route, Some(call_site.clone()), Some(digest), reason);
        }
    }

    fn walk_rule_family(
        &mut self,
        call_site: &ExprSiteId,
        family_key: &RuleDispatchKey,
        environment: &AffineEnvironment,
        layer: CheckedExploreSourceEventLayer,
        route: [u8; 32],
        depth: usize,
    ) {
        if family_key.scope.is_some() {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::ScopedReceiverNeedsBinding,
            );
            return;
        }
        let Some(family) = self.resolutions.rule_families.get(family_key).cloned() else {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::RuleFamilyMissing,
            );
            return;
        };
        if !self.active_families.insert(family_key.clone()) {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::RecursiveCall,
            );
            return;
        }
        let family_digest = self.family_digest(family_key);
        if family_digest.is_none() {
            self.record_residual(
                layer,
                route,
                Some(call_site.clone()),
                None,
                CheckedExploreSourceEventResidualReason::RuleFamilyUnsealed,
            );
            self.active_families.remove(family_key);
            return;
        }
        let result = (|| {
            let actuals = self.canonical_call_arguments(call_site, family_key.arity)?;
            for (candidate_index, candidate) in family.candidates.iter().enumerate() {
                if self.extraction_halted {
                    break;
                }
                let candidate_ordinal = u32::try_from(candidate_index)
                    .map_err(|_| CheckedExploreSourceEventResidualReason::ArithmeticOverflow)?;
                let candidate_route = extend_route(route, 0x02, candidate_ordinal, family_digest);
                let candidate_environment =
                    match self.bind_rule_head(candidate, &actuals, environment) {
                        Ok(environment) => environment,
                        Err(reason) => {
                            self.record_residual(
                                layer,
                                candidate_route,
                                Some(candidate.head_site.clone()),
                                family_digest,
                                reason,
                            );
                            continue;
                        }
                    };
                if let (Some(condition), Some(family_digest)) =
                    (&candidate.condition_site, family_digest)
                {
                    self.collect_condition_atoms(
                        call_site,
                        condition,
                        condition,
                        &candidate_environment,
                        layer,
                        candidate_route,
                        family_digest,
                        candidate_ordinal,
                        candidate,
                        false,
                    );
                }
                if self.extraction_halted {
                    break;
                }
                if let Some(condition) = &candidate.condition_site {
                    self.walk_expression(
                        condition,
                        &candidate_environment,
                        layer,
                        extend_route(candidate_route, 0x04, 0, None),
                        depth.saturating_add(1),
                    );
                }
                if self.extraction_halted {
                    break;
                }
                if let Some(value) = &candidate.value_site {
                    self.walk_expression(
                        value,
                        &candidate_environment,
                        layer,
                        extend_route(candidate_route, 0x05, 0, None),
                        depth.saturating_add(1),
                    );
                }
            }
            Ok(())
        })();
        self.active_families.remove(family_key);
        if let Err(reason) = result {
            self.record_residual(layer, route, Some(call_site.clone()), family_digest, reason);
        }
    }

    fn bind_rule_head(
        &self,
        candidate: &CheckedRuleCandidateResolution,
        actuals: &[ExprSiteId],
        caller_environment: &AffineEnvironment,
    ) -> Result<AffineEnvironment, CheckedExploreSourceEventResidualReason> {
        let expression = self
            .index
            .expression(&candidate.head_site)
            .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
        let ExprKind::App(_, arguments) = &expression.kind else {
            return Err(CheckedExploreSourceEventResidualReason::UnsupportedShape(
                CheckedExploreSourceEventUnsupportedShape::RuleHead,
            ));
        };
        if arguments.len() != actuals.len() {
            return Err(CheckedExploreSourceEventResidualReason::ArityMismatch {
                expected: u32::try_from(arguments.len()).unwrap_or(u32::MAX),
                actual: u32::try_from(actuals.len()).unwrap_or(u32::MAX),
            });
        }
        let mut environment = caller_environment.clone();
        for (index, actual) in actuals.iter().enumerate() {
            let head_argument = self.unwrap_argument_site(child_site(
                &candidate.head_site,
                u32::try_from(index + 1)
                    .map_err(|_| CheckedExploreSourceEventResidualReason::ArithmeticOverflow)?,
            ))?;
            let head_expression = self
                .index
                .expression(&head_argument)
                .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
            let ExprKind::Var(name) = &head_expression.kind else {
                return Err(CheckedExploreSourceEventResidualReason::RuleHeadPatternUnsupported);
            };
            if name == "_" {
                continue;
            }
            let resolution = self
                .resolutions
                .expressions
                .get(&head_argument)
                .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
            let Some(CheckedValueBinding::Binder {
                kind: CheckedBinderKind::RuleHead,
                site: binder,
            }) = resolution.value_binding.as_ref()
            else {
                return Err(CheckedExploreSourceEventResidualReason::RuleHeadBinderMismatch);
            };
            let value = self.affine_expression(actual, caller_environment);
            if let Some(previous) = environment.get(binder) {
                if previous != &value {
                    return Err(CheckedExploreSourceEventResidualReason::RuleHeadBinderMismatch);
                }
            } else {
                environment.insert(binder.clone(), value);
            }
        }
        Ok(environment)
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_condition_atoms(
        &mut self,
        source_call_site: &ExprSiteId,
        condition_root: &ExprSiteId,
        site: &ExprSiteId,
        environment: &AffineEnvironment,
        layer: CheckedExploreSourceEventLayer,
        call_route: [u8; 32],
        family_digest: [u8; 32],
        candidate_ordinal: u32,
        candidate: &CheckedRuleCandidateResolution,
        negated: bool,
    ) {
        let work_route = atom_route(call_route, condition_root, site);
        if !self.consume_work_item(layer, work_route, Some(site.clone()), Some(family_digest)) {
            return;
        }
        let Some(expression) = self.index.expression(site).cloned() else {
            self.record_residual(
                layer,
                call_route,
                Some(site.clone()),
                Some(family_digest),
                CheckedExploreSourceEventResidualReason::MissingExpressionResolution,
            );
            return;
        };
        match &expression.kind {
            ExprKind::BinOp(operator, _, _) if operator == "&&" || operator == "||" => {
                for child_index in [0_u32, 1] {
                    if self.extraction_halted {
                        return;
                    }
                    self.collect_condition_atoms(
                        source_call_site,
                        condition_root,
                        &child_site(site, child_index),
                        environment,
                        layer,
                        call_route,
                        family_digest,
                        candidate_ordinal,
                        candidate,
                        negated,
                    );
                }
            }
            ExprKind::UnOp(operator, _) if operator == "!" || operator == "not" => {
                self.collect_condition_atoms(
                    source_call_site,
                    condition_root,
                    &child_site(site, 0),
                    environment,
                    layer,
                    call_route,
                    family_digest,
                    candidate_ordinal,
                    candidate,
                    !negated,
                );
            }
            ExprKind::BinOp(operator, _, _) => {
                let Some(mut relation) = relation_from_operator(operator) else {
                    self.record_residual(
                        layer,
                        atom_route(call_route, condition_root, site),
                        Some(site.clone()),
                        Some(family_digest),
                        CheckedExploreSourceEventResidualReason::UnsupportedShape(
                            CheckedExploreSourceEventUnsupportedShape::Condition,
                        ),
                    );
                    return;
                };
                if negated {
                    relation = relation.negate();
                }
                let atom_path = relative_path(condition_root, site).unwrap_or_default();
                self.emit_atom(
                    source_call_site,
                    site,
                    environment,
                    layer,
                    call_route,
                    family_digest,
                    candidate_ordinal,
                    candidate,
                    atom_path,
                    relation,
                );
            }
            ExprKind::Lit(Literal::Bool(_)) => {}
            _ => self.record_residual(
                layer,
                atom_route(call_route, condition_root, site),
                Some(site.clone()),
                Some(family_digest),
                CheckedExploreSourceEventResidualReason::UnsupportedShape(
                    CheckedExploreSourceEventUnsupportedShape::Condition,
                ),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_atom(
        &mut self,
        source_call_site: &ExprSiteId,
        site: &ExprSiteId,
        environment: &AffineEnvironment,
        layer: CheckedExploreSourceEventLayer,
        call_route: [u8; 32],
        family_digest: [u8; 32],
        candidate_ordinal: u32,
        candidate: &CheckedRuleCandidateResolution,
        atom_path: Vec<u32>,
        mut relation: CheckedExploreSourceEventRelation,
    ) {
        if self.extraction_halted {
            return;
        }
        let atom_route = framed_atom_route(call_route, &atom_path);
        let left = self.affine_expression(&child_site(site, 0), environment);
        let right = self.affine_expression(&child_site(site, 1), environment);
        let difference = match (left, right) {
            (Ok(left), Ok(right)) => comparison_difference(&left, &right),
            (Err(error), _) | (_, Err(error)) => Err(error),
        };
        let mut difference = match difference {
            Ok(difference) => difference,
            Err(error) => {
                self.record_residual(
                    layer,
                    atom_route,
                    Some(site.clone()),
                    Some(family_digest),
                    residual_from_affine_error(error),
                );
                return;
            }
        };
        if difference.coefficients.is_empty() {
            return;
        }
        if difference.coefficients.len() != 1 {
            self.record_residual(
                layer,
                atom_route,
                Some(site.clone()),
                Some(family_digest),
                CheckedExploreSourceEventResidualReason::MultipleAxes {
                    count: u32::try_from(difference.coefficients.len()).unwrap_or(u32::MAX),
                },
            );
            return;
        }
        let (&source_binding_index, &coefficient) = difference
            .coefficients
            .iter()
            .next()
            .expect("one affine axis");
        let Some((coefficient, intercept, canonical_relation)) =
            normalize_atom(coefficient, difference.intercept, relation)
        else {
            self.record_residual(
                layer,
                atom_route,
                Some(site.clone()),
                Some(family_digest),
                CheckedExploreSourceEventResidualReason::ArithmeticOverflow,
            );
            return;
        };
        relation = canonical_relation;
        difference.coefficients.clear();
        let origin = CheckedExploreSourceEventOrigin {
            family_digest,
            candidate_ordinal,
            tier: candidate.tier,
            condition_atom_path: atom_path.into_boxed_slice(),
            call_route_digest: call_route,
            call_site: source_call_site.clone(),
            candidate: candidate.clone(),
        };
        let source_event_id = source_event_id(&origin, relation);
        let mut event = CheckedExploreAffineSourceEvent {
            occurrence_id: CheckedExploreBoundaryOccurrenceId([0; 32]),
            source_event_id,
            layer,
            source_binding_index,
            coefficient,
            intercept,
            relation,
            origin,
        };
        event.occurrence_id = occurrence_id(&event);
        if !self.reserve_output_item(layer, atom_route, Some(site.clone()), Some(family_digest)) {
            return;
        }
        self.events.push(event);
    }

    fn canonical_call_arguments(
        &self,
        call_site: &ExprSiteId,
        expected_arity: usize,
    ) -> Result<Vec<ExprSiteId>, CheckedExploreSourceEventResidualReason> {
        let expression = self
            .index
            .expression(call_site)
            .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
        let ExprKind::App(_, arguments) = &expression.kind else {
            return Err(CheckedExploreSourceEventResidualReason::UnsupportedShape(
                CheckedExploreSourceEventUnsupportedShape::Call,
            ));
        };
        if arguments.len() != expected_arity {
            return Err(CheckedExploreSourceEventResidualReason::ArityMismatch {
                expected: u32::try_from(expected_arity).unwrap_or(u32::MAX),
                actual: u32::try_from(arguments.len()).unwrap_or(u32::MAX),
            });
        }
        let resolution = self
            .resolutions
            .expressions
            .get(call_site)
            .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
        let has_named = arguments
            .iter()
            .any(|argument| named_arg_parts(argument).is_some());
        let source_indices = match resolution.named_arguments.as_ref() {
            Some(order)
                if order.canonical_source_indices.len() == expected_arity
                    && order.parameter_names.len() == expected_arity =>
            {
                let indices = order.canonical_source_indices.to_vec();
                let unique = indices.iter().copied().collect::<BTreeSet<_>>();
                if indices.iter().any(|index| *index >= arguments.len())
                    || unique.len() != indices.len()
                {
                    return Err(
                        CheckedExploreSourceEventResidualReason::NamedArgumentMismatch {
                            expected: u32::try_from(expected_arity).unwrap_or(u32::MAX),
                            actual: u32::try_from(unique.len()).unwrap_or(u32::MAX),
                        },
                    );
                }
                indices
            }
            Some(_) => {
                return Err(
                    CheckedExploreSourceEventResidualReason::NamedArgumentMismatch {
                        expected: u32::try_from(expected_arity).unwrap_or(u32::MAX),
                        actual: u32::try_from(arguments.len()).unwrap_or(u32::MAX),
                    },
                )
            }
            None if has_named => {
                return Err(
                    CheckedExploreSourceEventResidualReason::NamedArgumentMismatch {
                        expected: u32::try_from(expected_arity).unwrap_or(u32::MAX),
                        actual: u32::try_from(arguments.len()).unwrap_or(u32::MAX),
                    },
                )
            }
            None => (0..arguments.len()).collect(),
        };
        source_indices
            .into_iter()
            .map(|source_index| {
                let source_index = u32::try_from(source_index + 1)
                    .map_err(|_| CheckedExploreSourceEventResidualReason::ArithmeticOverflow)?;
                self.unwrap_argument_site(child_site(call_site, source_index))
            })
            .collect()
    }

    fn unwrap_argument_site(
        &self,
        mut site: ExprSiteId,
    ) -> Result<ExprSiteId, CheckedExploreSourceEventResidualReason> {
        loop {
            let expression = self
                .index
                .expression(&site)
                .ok_or(CheckedExploreSourceEventResidualReason::MissingExpressionResolution)?;
            if typed_rule_head_argument(expression).is_some() {
                site = child_site(&site, 1);
            } else if named_arg_parts(expression).is_some() {
                site = child_site(&site, 2);
            } else {
                return Ok(site);
            }
        }
    }

    fn family_digest(&mut self, family: &RuleDispatchKey) -> Option<[u8; 32]> {
        if let Some(digest) = self.family_digests.get(family) {
            return *digest;
        }
        let digest = checked_explore_semantic_dependency_root_digest(
            self.index,
            self.resolutions,
            &self.semantic_binders,
            CheckedExploreSemanticDependency::RuleFamily(family.clone()),
        )
        .ok();
        self.family_digests.insert(family.clone(), digest);
        digest
    }

    fn callable_digest(&mut self, callable: &crate::CheckedCallableId) -> Option<[u8; 32]> {
        if let Some(digest) = self.callable_digests.get(callable) {
            return *digest;
        }
        let digest = checked_explore_semantic_dependency_root_digest(
            self.index,
            self.resolutions,
            &self.semantic_binders,
            CheckedExploreSemanticDependency::Callable(callable.clone()),
        )
        .ok();
        self.callable_digests.insert(callable.clone(), digest);
        digest
    }

    fn record_residual(
        &mut self,
        layer: CheckedExploreSourceEventLayer,
        route_digest: [u8; 32],
        site: Option<ExprSiteId>,
        dependency_digest: Option<[u8; 32]>,
        reason: CheckedExploreSourceEventResidualReason,
    ) {
        if !self.reserve_output_item(layer, route_digest, site.clone(), dependency_digest) {
            return;
        }
        self.residuals.push(CheckedExploreSourceEventResidual {
            residual_id: residual_id(
                self.relation_id,
                layer,
                route_digest,
                dependency_digest,
                reason,
            ),
            layer,
            route_digest,
            dependency_digest,
            reason,
            site,
        });
    }
}

fn child_site(site: &ExprSiteId, child: u32) -> ExprSiteId {
    let mut child_site = site.clone();
    let mut path = child_site.ast_path.to_vec();
    path.push(child);
    child_site.ast_path = path.into_boxed_slice();
    child_site
}

fn relative_path(root: &ExprSiteId, site: &ExprSiteId) -> Option<Vec<u32>> {
    (root.analysis_program == site.analysis_program
        && root.declaration == site.declaration
        && root.normalized_declaration_ordinal == site.normalized_declaration_ordinal)
        .then(|| site.ast_path.strip_prefix(root.ast_path.as_ref()))
        .flatten()
        .map(<[u32]>::to_vec)
}

fn comparison_difference(left: &AffineTerm, right: &AffineTerm) -> AffineValue {
    let coefficients = combine_coefficients(&left.coefficients, &right.coefficients, true)?;
    Ok(AffineTerm {
        coefficients,
        intercept: left
            .intercept
            .checked_sub(right.intercept)
            .ok_or(AffineError::Overflow)?,
        minimum: i128::MIN,
        maximum: i128::MAX,
    })
}

fn relation_from_operator(operator: &str) -> Option<CheckedExploreSourceEventRelation> {
    match operator {
        "<" => Some(CheckedExploreSourceEventRelation::Less),
        "<=" => Some(CheckedExploreSourceEventRelation::LessOrEqual),
        "==" => Some(CheckedExploreSourceEventRelation::Equal),
        "!=" => Some(CheckedExploreSourceEventRelation::NotEqual),
        ">=" => Some(CheckedExploreSourceEventRelation::GreaterOrEqual),
        ">" => Some(CheckedExploreSourceEventRelation::Greater),
        _ => None,
    }
}

fn normalize_atom(
    mut coefficient: i128,
    mut intercept: i128,
    mut relation: CheckedExploreSourceEventRelation,
) -> Option<(i128, i128, CheckedExploreSourceEventRelation)> {
    if coefficient < 0 {
        coefficient = coefficient.checked_neg()?;
        intercept = intercept.checked_neg()?;
        relation = relation.reverse();
    }
    let divisor = gcd_u128(coefficient.unsigned_abs(), intercept.unsigned_abs());
    if divisor > 1 {
        let divisor = i128::try_from(divisor).ok()?;
        coefficient /= divisor;
        intercept /= divisor;
    }
    Some((coefficient, intercept, relation))
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn residual_from_affine_error(error: AffineError) -> CheckedExploreSourceEventResidualReason {
    match error {
        AffineError::MissingResolution => {
            CheckedExploreSourceEventResidualReason::MissingExpressionResolution
        }
        AffineError::UnsupportedSourceAxis => {
            CheckedExploreSourceEventResidualReason::UnsupportedSourceAxis
        }
        AffineError::UnsupportedSourceDomain => {
            CheckedExploreSourceEventResidualReason::UnsupportedShape(
                CheckedExploreSourceEventUnsupportedShape::SourceDomain,
            )
        }
        AffineError::BinderMismatch => {
            CheckedExploreSourceEventResidualReason::RuleHeadBinderMismatch
        }
        AffineError::OpenCapture => CheckedExploreSourceEventResidualReason::OpenCapture,
        AffineError::NonAffine => CheckedExploreSourceEventResidualReason::NonAffine,
        AffineError::Overflow => CheckedExploreSourceEventResidualReason::ArithmeticOverflow,
        AffineError::UnsupportedShape => CheckedExploreSourceEventResidualReason::UnsupportedShape(
            CheckedExploreSourceEventUnsupportedShape::Expression,
        ),
    }
}

fn base_route(relation_id: explore::RelationId, layer: CheckedExploreSourceEventLayer) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ROUTE_DOMAIN);
    hasher.update(relation_id.bytes());
    hash_layer(&mut hasher, layer);
    hasher.finalize().into()
}

fn extend_route(parent: [u8; 32], tag: u8, ordinal: u32, dependency: Option<[u8; 32]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ROUTE_DOMAIN);
    hasher.update(parent);
    hasher.update([tag]);
    hasher.update(ordinal.to_be_bytes());
    match dependency {
        Some(dependency) => {
            hasher.update([0x01]);
            hasher.update(dependency);
        }
        None => hasher.update([0x00]),
    }
    hasher.finalize().into()
}

fn atom_route(call_route: [u8; 32], condition_root: &ExprSiteId, site: &ExprSiteId) -> [u8; 32] {
    framed_atom_route(
        call_route,
        &relative_path(condition_root, site).unwrap_or_default(),
    )
}

fn framed_atom_route(call_route: [u8; 32], atom_path: &[u32]) -> [u8; 32] {
    atom_path
        .iter()
        .copied()
        .fold(extend_route(call_route, 0x06, 0, None), |route, child| {
            extend_route(route, 0x07, child, None)
        })
}

fn hash_layer(hasher: &mut Sha256, layer: CheckedExploreSourceEventLayer) {
    match layer {
        CheckedExploreSourceEventLayer::SourceBinding { binding_index } => {
            hasher.update([0x01]);
            hasher.update(binding_index.to_be_bytes());
        }
        CheckedExploreSourceEventLayer::Successor => hasher.update([0x02]),
        CheckedExploreSourceEventLayer::Admission {
            admission_id,
            admission_index,
        } => {
            hasher.update([0x03]);
            hasher.update(admission_id.bytes());
            hasher.update(admission_index.to_be_bytes());
        }
        CheckedExploreSourceEventLayer::Find { question_id } => {
            hasher.update([0x04]);
            hasher.update(question_id.bytes());
        }
    }
}

fn tier_tag(tier: RuleDispatchTier) -> u8 {
    match tier {
        RuleDispatchTier::Exception => 0x01,
        RuleDispatchTier::ConditionalDefault => 0x02,
        RuleDispatchTier::Clause => 0x03,
        RuleDispatchTier::UnconditionalDefault => 0x04,
    }
}

fn source_event_id(
    origin: &CheckedExploreSourceEventOrigin,
    relation: CheckedExploreSourceEventRelation,
) -> CheckedExploreSourceEventId {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_EVENT_DOMAIN);
    hasher.update(CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION.to_be_bytes());
    hasher.update(origin.family_digest);
    hasher.update(origin.candidate_ordinal.to_be_bytes());
    hasher.update([tier_tag(origin.tier), relation.tag()]);
    hasher.update((origin.condition_atom_path.len() as u64).to_be_bytes());
    for child in origin.condition_atom_path.iter() {
        hasher.update(child.to_be_bytes());
    }
    CheckedExploreSourceEventId(hasher.finalize().into())
}

fn occurrence_id(event: &CheckedExploreAffineSourceEvent) -> CheckedExploreBoundaryOccurrenceId {
    let mut hasher = Sha256::new();
    hasher.update(OCCURRENCE_DOMAIN);
    hasher.update(CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION.to_be_bytes());
    hasher.update(event.source_event_id.bytes());
    hash_layer(&mut hasher, event.layer);
    hasher.update(event.source_binding_index.to_be_bytes());
    hasher.update(event.coefficient.to_be_bytes());
    hasher.update(event.intercept.to_be_bytes());
    hasher.update([event.relation.tag()]);
    hasher.update(event.origin.call_route_digest);
    CheckedExploreBoundaryOccurrenceId(hasher.finalize().into())
}

fn hash_residual_reason(hasher: &mut Sha256, reason: CheckedExploreSourceEventResidualReason) {
    match reason {
        CheckedExploreSourceEventResidualReason::MissingExpressionResolution => {
            hasher.update([0x01])
        }
        CheckedExploreSourceEventResidualReason::UnsupportedSourceAxis => hasher.update([0x02]),
        CheckedExploreSourceEventResidualReason::UnresolvedOrDynamicCall => hasher.update([0x03]),
        CheckedExploreSourceEventResidualReason::CallableUnsealed => hasher.update([0x04]),
        CheckedExploreSourceEventResidualReason::RuleFamilyMissing => hasher.update([0x05]),
        CheckedExploreSourceEventResidualReason::RuleFamilyUnsealed => hasher.update([0x06]),
        CheckedExploreSourceEventResidualReason::NamedArgumentMismatch { expected, actual } => {
            hasher.update([0x07]);
            hasher.update(expected.to_be_bytes());
            hasher.update(actual.to_be_bytes());
        }
        CheckedExploreSourceEventResidualReason::ArityMismatch { expected, actual } => {
            hasher.update([0x08]);
            hasher.update(expected.to_be_bytes());
            hasher.update(actual.to_be_bytes());
        }
        CheckedExploreSourceEventResidualReason::RuleHeadPatternUnsupported => {
            hasher.update([0x09])
        }
        CheckedExploreSourceEventResidualReason::RuleHeadBinderMismatch => hasher.update([0x0a]),
        CheckedExploreSourceEventResidualReason::ScopedReceiverNeedsBinding => {
            hasher.update([0x0b])
        }
        CheckedExploreSourceEventResidualReason::RecursiveCall => hasher.update([0x0c]),
        CheckedExploreSourceEventResidualReason::EffectfulCallable => hasher.update([0x0d]),
        CheckedExploreSourceEventResidualReason::OpenCapture => hasher.update([0x0e]),
        CheckedExploreSourceEventResidualReason::MultipleAxes { count } => {
            hasher.update([0x0f]);
            hasher.update(count.to_be_bytes());
        }
        CheckedExploreSourceEventResidualReason::NonAffine => hasher.update([0x10]),
        CheckedExploreSourceEventResidualReason::ArithmeticOverflow => hasher.update([0x11]),
        CheckedExploreSourceEventResidualReason::UnsupportedShape(shape) => {
            hasher.update([0x12]);
            hasher.update([match shape {
                CheckedExploreSourceEventUnsupportedShape::SourceDomain => 0x01,
                CheckedExploreSourceEventUnsupportedShape::RuleHead => 0x02,
                CheckedExploreSourceEventUnsupportedShape::Condition => 0x03,
                CheckedExploreSourceEventUnsupportedShape::Expression => 0x04,
                CheckedExploreSourceEventUnsupportedShape::Call => 0x05,
                CheckedExploreSourceEventUnsupportedShape::Successor => 0x06,
            }]);
        }
        CheckedExploreSourceEventResidualReason::ExtractionWorkBudgetExceeded { limit } => {
            hasher.update([0x13]);
            hasher.update(limit.to_be_bytes());
        }
        CheckedExploreSourceEventResidualReason::ExtractionOutputBudgetExceeded { limit } => {
            hasher.update([0x14]);
            hasher.update(limit.to_be_bytes());
        }
    }
}

fn residual_id(
    relation_id: explore::RelationId,
    layer: CheckedExploreSourceEventLayer,
    route_digest: [u8; 32],
    dependency_digest: Option<[u8; 32]>,
    reason: CheckedExploreSourceEventResidualReason,
) -> CheckedExploreSourceEventResidualId {
    let mut hasher = Sha256::new();
    hasher.update(RESIDUAL_DOMAIN);
    hasher.update(CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION.to_be_bytes());
    hasher.update(relation_id.bytes());
    hash_layer(&mut hasher, layer);
    hasher.update(route_digest);
    match dependency_digest {
        Some(dependency_digest) => {
            hasher.update([0x01]);
            hasher.update(dependency_digest);
        }
        None => hasher.update([0x00]),
    }
    hash_residual_reason(&mut hasher, reason);
    CheckedExploreSourceEventResidualId(hasher.finalize().into())
}

fn inventory_root(inventory: &CheckedExploreSourceEventInventory) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(INVENTORY_DOMAIN);
    hasher.update(inventory.version.to_be_bytes());
    hasher.update(inventory.relation_id.bytes());
    hasher.update(inventory.admission_id.bytes());
    hasher.update(inventory.source_binding_count.to_be_bytes());
    hasher.update(inventory.admission_count.to_be_bytes());
    hasher.update((inventory.question_ids.len() as u64).to_be_bytes());
    for question_id in inventory.question_ids.iter() {
        hasher.update(question_id.bytes());
    }
    hasher.update((inventory.events.len() as u64).to_be_bytes());
    for event in inventory.events.iter() {
        hasher.update(event.occurrence_id.bytes());
    }
    hasher.update((inventory.residuals.len() as u64).to_be_bytes());
    for residual in inventory.residuals.iter() {
        hasher.update(residual.residual_id.bytes());
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser, TypeChecker};

    fn artifacts(source: &str) -> crate::TypeCheckArtifacts {
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse source-event fixture");
        TypeChecker::check_with_explore_artifacts(&statements, None, source)
    }

    fn checked_inventory(source: &str) -> CheckedExploreSourceEventInventory {
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected source-event diagnostics: {:?}",
            artifacts.diagnostics
        );
        artifacts
            .checked_exploration_query(0)
            .expect("checked source-event query")
            .source_event_inventory()
            .clone()
    }

    fn checked_inventory_with_limits(
        source: &str,
        limits: SourceEventExtractionLimits,
    ) -> CheckedExploreSourceEventInventory {
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected limited source-event diagnostics: {:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("checked limited source-event query");
        let semantic_index = CheckedExploreSemanticIndex::build(&artifacts.analysis_program);
        checked_explore_source_event_inventory_with_limits(
            &artifacts.analysis_program,
            &artifacts.checked_resolutions,
            checked.closed_query,
            &checked.artifact.sites,
            checked.relation_id(),
            checked.admission_id(),
            checked.find_question_ids(),
            &semantic_index,
            limits,
        )
        .expect("limited source-event inventory")
    }

    #[test]
    fn checked_explore_source_event_work_budget_exhaustion_is_stable_and_fail_closed() {
        let identity = |source: &str| {
            let inventory = checked_inventory_with_limits(
                source,
                SourceEventExtractionLimits {
                    work_items: 1,
                    output_items: 4,
                },
            );
            assert!(inventory.validate_identity(), "{inventory:#?}");
            assert!(inventory.events().is_empty(), "{inventory:#?}");
            assert_eq!(inventory.residuals().len(), 1, "{inventory:#?}");
            assert_eq!(
                inventory.residuals()[0].reason,
                CheckedExploreSourceEventResidualReason::ExtractionWorkBudgetExceeded { limit: 1 }
            );
            (
                inventory.residuals()[0].residual_id,
                inventory.inventory_root(),
            )
        };

        let baseline = identity(
            r#"
? explore budgeted_query {
    from {
        vary before in range(0, 10)
        given context = ()
    }
    transition after = before + 1
    find cases = all
}
"#,
        );
        let renamed_and_repositioned = identity(
            r#"


? explore renamed_query {
    from {
        vary before in range(0, 10)
        given context = ()
    }
    transition after = before + 1
    find renamed_cases = all
}
"#,
        );
        assert_eq!(baseline, renamed_and_repositioned);
    }

    #[test]
    fn checked_explore_source_event_output_budget_reserves_one_terminal_residual() {
        let source = r#"
| bounded_rate(value: Int) -> 0
| exception thresholds bounded_rate(value: Int) -> 1 under value >= 2 && value >= 4 && value >= 6

? explore bounded_outputs {
    from {
        vary before in range(0, 10)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of bounded_rate(before) == 1
}
"#;
        let complete = checked_inventory(source);
        assert_eq!(complete.events().len(), 3, "{complete:#?}");

        let limited = checked_inventory_with_limits(
            source,
            SourceEventExtractionLimits {
                work_items: 1_024,
                output_items: 2,
            },
        );
        assert!(limited.validate_identity(), "{limited:#?}");
        assert_eq!(limited.events().len(), 1, "{limited:#?}");
        assert_eq!(limited.residuals().len(), 1, "{limited:#?}");
        assert_eq!(
            limited.residuals()[0].reason,
            CheckedExploreSourceEventResidualReason::ExtractionOutputBudgetExceeded { limit: 2 }
        );
        assert!(complete
            .events()
            .iter()
            .any(|event| event.occurrence_id == limited.events()[0].occurrence_id));
    }

    #[test]
    fn checked_explore_source_event_extracts_exception_threshold_and_survives_ownership() {
        let source = r#"
| source_event_rate(value: Int) -> 0
| exception threshold source_event_rate(value: Int) -> 1 under value >= 680

? explore source_event_boundary {
    from {
        vary before in range(0, 700)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of source_event_rate(before) == 1
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected source-event diagnostics: {:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("checked source-event query");
        let inventory = checked.source_event_inventory();
        assert!(inventory.validate_identity());
        assert_eq!(
            inventory.version(),
            CHECKED_EXPLORE_SOURCE_EVENT_INVENTORY_VERSION
        );
        assert_eq!(inventory.events().len(), 1, "{inventory:#?}");
        let event = &inventory.events()[0];
        assert_eq!(event.source_binding_index, 0);
        assert_eq!(event.coefficient, 1);
        assert_eq!(event.intercept, -680);
        assert_eq!(
            event.relation,
            CheckedExploreSourceEventRelation::GreaterOrEqual
        );
        assert_eq!(event.origin.tier, RuleDispatchTier::Exception);
        assert_eq!(event.origin.candidate_ordinal, 0);
        assert!(matches!(
            event.layer,
            CheckedExploreSourceEventLayer::Find { .. }
        ));

        let root = inventory.inventory_root();
        let occurrence = event.occurrence_id;
        let owned = checked.to_owned_checked_query();
        drop(artifacts);
        let owned_view = owned.view();
        let retained = owned_view.source_event_inventory();
        assert!(retained.validate_identity());
        assert_eq!(retained.inventory_root(), root);
        assert_eq!(retained.events()[0].occurrence_id, occurrence);
    }

    #[test]
    fn checked_explore_source_event_retains_nonlinear_condition_as_residual_without_cut() {
        let source = r#"
| nonlinear_rate(value: Int) -> 0
| exception threshold nonlinear_rate(value: Int) -> 1 under value * value >= 680

? explore nonlinear_boundary {
    from {
        vary before in range(0, 700)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of nonlinear_rate(before) == 1
}
"#;
        let artifacts = artifacts(source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "unexpected nonlinear diagnostics: {:?}",
            artifacts.diagnostics
        );
        let checked = artifacts
            .checked_exploration_query(0)
            .expect("checked nonlinear source-event query");
        let inventory = checked.source_event_inventory();
        assert!(inventory.validate_identity());
        assert!(inventory.events().is_empty(), "{inventory:#?}");
        assert!(inventory.residuals().iter().any(|residual| {
            residual.reason == CheckedExploreSourceEventResidualReason::NonAffine
        }));
    }

    #[test]
    fn checked_explore_source_event_named_argument_reordering_and_names_are_not_identity() {
        let event_identity = |source: &str| {
            let artifacts = artifacts(source);
            assert!(
                artifacts.diagnostics.is_empty(),
                "unexpected canonical source-event diagnostics: {:?}",
                artifacts.diagnostics
            );
            let checked = artifacts
                .checked_exploration_query(0)
                .expect("checked canonical source-event query");
            let inventory = checked.source_event_inventory();
            assert!(inventory.validate_identity());
            assert_eq!(inventory.events().len(), 1, "{inventory:#?}");
            assert!(inventory.residuals().is_empty(), "{inventory:#?}");
            (
                inventory.events()[0].source_event_id,
                inventory.events()[0].occurrence_id,
                inventory.inventory_root(),
            )
        };
        let baseline = event_identity(
            r#"
| named_boundary(value: Int, offset: Int) -> 0
| exception threshold named_boundary(value: Int, offset: Int) -> 1 under value + offset >= 680

? explore canonical_query {
    from {
        vary before in range(0, 700)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of named_boundary(offset = 0, value = before) == 1
}
"#,
        );
        let reordered_and_renamed = event_identity(
            r#"
| renamed_boundary(amount: Int, shift: Int) -> 0
| exception renamed_threshold renamed_boundary(amount: Int, shift: Int) -> 1 under amount + shift >= 680

? explore renamed_query {
    from {
        vary before in range(0, 700)
        given context = ()
    }
    transition after = before + 1
    find renamed_cases = matches of renamed_boundary(amount = before, shift = 0) == 1
}
"#,
        );
        assert_eq!(baseline, reordered_and_renamed);
    }

    #[test]
    fn checked_explore_source_event_records_unsupported_finite_domains_without_polluting_singletons(
    ) {
        let source_domain = checked_inventory(
            r#"
? explore unsupported_source_domain {
    from {
        vary before in [0, 1]
        given context = ()
    }
    transition after = before
    find cases = all
}
"#,
        );
        assert!(source_domain.events().is_empty(), "{source_domain:#?}");
        assert_eq!(source_domain.residuals().len(), 1, "{source_domain:#?}");
        assert!(matches!(
            source_domain.residuals()[0],
            CheckedExploreSourceEventResidual {
                layer: CheckedExploreSourceEventLayer::SourceBinding { binding_index: 0 },
                reason: CheckedExploreSourceEventResidualReason::UnsupportedShape(
                    CheckedExploreSourceEventUnsupportedShape::SourceDomain
                ),
                ..
            }
        ));

        let finite_successor = checked_inventory(
            r#"
? explore unsupported_finite_successor {
    from {
        vary before in range(0, 2)
        given context = ()
    }
    transition after in [before]
    find cases = all
}
"#,
        );
        assert!(
            finite_successor.events().is_empty(),
            "{finite_successor:#?}"
        );
        assert_eq!(
            finite_successor.residuals().len(),
            1,
            "{finite_successor:#?}"
        );
        assert!(matches!(
            finite_successor.residuals()[0],
            CheckedExploreSourceEventResidual {
                layer: CheckedExploreSourceEventLayer::Successor,
                reason: CheckedExploreSourceEventResidualReason::UnsupportedShape(
                    CheckedExploreSourceEventUnsupportedShape::Successor
                ),
                ..
            }
        ));
    }

    #[test]
    fn checked_explore_source_event_frames_each_failed_condition_atom_route() {
        let inventory = checked_inventory(
            r#"
| two_nonlinear_atoms(value: Int) -> 0
| exception thresholds two_nonlinear_atoms(value: Int) -> 1 under value * value >= 10 && (value + 1) * (value + 1) >= 20

? explore two_nonlinear_boundaries {
    from {
        vary before in range(0, 10)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of two_nonlinear_atoms(before) == 1
}
"#,
        );
        assert!(inventory.events().is_empty(), "{inventory:#?}");
        let nonlinear = inventory
            .residuals()
            .iter()
            .filter(|residual| {
                residual.reason == CheckedExploreSourceEventResidualReason::NonAffine
            })
            .collect::<Vec<_>>();
        assert_eq!(nonlinear.len(), 2, "{inventory:#?}");
        assert_ne!(nonlinear[0].residual_id, nonlinear[1].residual_id);
        assert_ne!(nonlinear[0].route_digest, nonlinear[1].route_digest);
    }

    #[test]
    fn checked_explore_source_event_does_not_mint_a_zero_coefficient_cut() {
        let inventory = checked_inventory(
            r#"
| zeroed_boundary(value: Int) -> 0
| exception threshold zeroed_boundary(value: Int) -> 1 under value * 0 >= 1

? explore zeroed_source_boundary {
    from {
        vary before in range(0, 10)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of zeroed_boundary(before) == 1
}
"#,
        );
        assert!(inventory.events().is_empty(), "{inventory:#?}");
        assert!(inventory.residuals().is_empty(), "{inventory:#?}");
    }

    #[test]
    fn checked_explore_source_event_canonicalizes_nested_calls_in_named_actuals() {
        let identity = |arguments: &str| {
            let source = format!(
                r#"
| nested_boundary(value: Int) -> 0
| exception threshold nested_boundary(value: Int) -> 1 under value >= 680

> add_named(left: Int, right: Int) -> Int {{ left + right }}

? explore nested_named_boundary {{
    from {{
        vary before in range(0, 700)
        given context = ()
    }}
    transition after = before + 1
    find cases = matches of add_named({arguments}) == 1
}}
"#,
            );
            let inventory = checked_inventory(&source);
            assert_eq!(inventory.events().len(), 1, "{inventory:#?}");
            assert!(inventory.residuals().is_empty(), "{inventory:#?}");
            (
                inventory.events()[0].source_event_id,
                inventory.events()[0].occurrence_id,
                inventory.inventory_root(),
            )
        };
        assert_eq!(
            identity("right = 0, left = nested_boundary(before)"),
            identity("left = nested_boundary(before), right = 0")
        );
    }

    #[test]
    fn checked_explore_source_event_fail_closed_matrix_has_no_cuts() {
        let fixtures = [
            (
                "multiple axes",
                r#"
| combined_axes(left: Int, right: Int) -> 0
| exception threshold combined_axes(left: Int, right: Int) -> 1 under left + right >= 3

? explore multiple_source_axes {
    from {
        vary left in range(0, 3)
        vary right in range(0, 3)
        let before = left
        let context = right
    }
    transition after = before + 1
    find cases = matches of combined_axes(before, context) == 1
}
"#,
                CheckedExploreSourceEventResidualReason::MultipleAxes { count: 2 },
            ),
            (
                "scoped receiver",
                r#"
# ScopedBoundary(base: Int) {
    | rate(value: Int) -> base
    | exception threshold rate(value: Int) -> 1 under value >= 2
}

? explore scoped_source_boundary {
    from {
        vary before in range(0, 3)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of ScopedBoundary(base = 0).rate(value = before) == 1
}
"#,
                CheckedExploreSourceEventResidualReason::ScopedReceiverNeedsBinding,
            ),
            (
                "recursive family",
                r#"
| recursive_boundary(value: Int) -> 0
| exception recursive recursive_boundary(value: Int) -> 1 under recursive_boundary(value) == 1

? explore recursive_source_boundary {
    from {
        vary before in range(0, 3)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of recursive_boundary(before) == 1
}
"#,
                CheckedExploreSourceEventResidualReason::RecursiveCall,
            ),
            (
                "integer overflow",
                r#"
| overflowing_boundary(value: Int) -> 0
| exception threshold overflowing_boundary(value: Int) -> 1 under value + 9223372036854775807 >= 0

? explore overflowing_source_boundary {
    from {
        vary before in range(0, 2)
        given context = ()
    }
    transition after = before
    find cases = matches of overflowing_boundary(before) == 1
}
"#,
                CheckedExploreSourceEventResidualReason::ArithmeticOverflow,
            ),
        ];
        for (label, source, reason) in fixtures {
            let inventory = checked_inventory(source);
            assert!(inventory.events().is_empty(), "{label}: {inventory:#?}");
            assert!(
                inventory
                    .residuals()
                    .iter()
                    .any(|residual| residual.reason == reason),
                "{label}: {inventory:#?}"
            );
        }

        let effectful = artifacts(
            r#"
# effect Marker {
    > mark(value: Int) -> Int
}

> declared_effect(value: Int) -> Int with Marker { value }

? explore effectful_source_boundary {
    from {
        vary before in range(0, 3)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of declared_effect(before) >= 0
}
"#,
        );
        assert!(effectful.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exploration expressions must use only pure")
        }));
        assert!(effectful.checked_exploration_query(0).is_err());

        let wrong_arity = artifacts(
            r#"
| arity_boundary(value: Int, offset: Int) -> 0

? explore wrong_arity_source_boundary {
    from {
        vary before in range(0, 3)
        given context = ()
    }
    transition after = before + 1
    find cases = matches of arity_boundary(before) == 1
}
"#,
        );
        assert!(!wrong_arity.diagnostics.is_empty());
        assert!(wrong_arity.checked_exploration_query(0).is_err());
    }
}
