//! Replayable transcript core for Explore probe scheduling.
//!
//! Probe artifacts are private scheduler transcripts, not partial Explore
//! answers and not closure certificates. In the observable architecture their
//! decisions and validated singleton observations are committed inside the
//! unified run journal; the standalone artifact shape remains useful as a
//! canonical export/import and deterministic-replay boundary, not as a second
//! run lifecycle. This module deliberately contains no parser, serialization,
//! filesystem or CLI policy.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU128;

use super::{report::ExploreCaseId, ExploreGeneratorAxisRole, ExplorePolarity, ExploreValue};

pub(crate) const PROBE_ARTIFACT_SCHEMA_V2: &str = "futuruna.explore.probe-artifact.v2";

/// Hash-bound semantic identity of one checked probe plan.
///
/// Operational choices such as path, timeout, checkpoint cadence and maximum
/// artifact bytes do not belong here.  Every field is mandatory and validated
/// as lowercase SHA-256 digests before an artifact can influence scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeSemanticIdentity {
    pub(crate) program_hash: Box<str>,
    pub(crate) analysis_program_hash: Box<str>,
    pub(crate) query_hash: Box<str>,
    pub(crate) domain_hash: Box<str>,
    pub(crate) probe_plan_hash: Box<str>,
    pub(crate) evaluator_contract_hash: Box<str>,
}

impl ProbeSemanticIdentity {
    fn validate(&self) -> Result<(), ProbeValidationError> {
        for (name, value) in [
            ("program_hash", self.program_hash.as_ref()),
            ("analysis_program_hash", self.analysis_program_hash.as_ref()),
            ("query_hash", self.query_hash.as_ref()),
            ("domain_hash", self.domain_hash.as_ref()),
            ("probe_plan_hash", self.probe_plan_hash.as_ref()),
            (
                "evaluator_contract_hash",
                self.evaluator_contract_hash.as_ref(),
            ),
        ] {
            require_lowercase_sha256(name, value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProbeSelector {
    BoundaryCandidates,
    BoundaryEndpoints,
    FrontierMidpoints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeBoundaryContract {
    pub(crate) axis: usize,
    pub(crate) step: i64,
    pub(crate) requires_both_endpoints_in_domain: bool,
}

/// Canonical identity and presentation metadata for one probe CaseId axis.
///
/// `label` is never authoritative: fields with the same spelling may coexist
/// in distinct Context, Before and independent-After roles. Durable selection
/// uses the structural coordinates and the descriptor's canonical list index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeDimensionDescriptor {
    pub(crate) bound_index: usize,
    pub(crate) role: ExploreGeneratorAxisRole,
    pub(crate) role_field_index: usize,
    pub(crate) label: String,
}

/// Checked schema and disclosure authorization for a probe artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbePlanContract {
    pub(crate) artifact_schema: Box<str>,
    pub(crate) normalization_version: Box<str>,
    pub(crate) selector_tie_break_version: Box<str>,
    pub(crate) query_name: Box<str>,
    pub(crate) identity: ProbeSemanticIdentity,
    pub(crate) polarity: ExplorePolarity,
    pub(crate) dimensions: Box<[ProbeDimensionDescriptor]>,
    pub(crate) axis_cardinalities: Box<[u128]>,
    pub(crate) boundary: Option<ProbeBoundaryContract>,
    pub(crate) selectors: Box<[ProbeSelector]>,
    pub(crate) semantic_case_cap: NonZeroU128,
    pub(crate) initial_frontier: ProbeFrontierId,
    /// Independently varied, non-boundary dimensions authorized by the
    /// optional `lift ... across [...]` operation, in dimension order.
    pub(crate) lift_dimension_indices: Box<[usize]>,
    pub(crate) retained_configuration_dimension_indices: Box<[usize]>,
    pub(crate) retained_key_names: Box<[String]>,
    pub(crate) retained_shown_names: Box<[String]>,
    pub(crate) mechanism_trace_authorized: bool,
}

impl ProbePlanContract {
    pub(crate) fn validate(&self) -> Result<(), ProbeValidationError> {
        require_nonempty("artifact_schema", &self.artifact_schema)?;
        if self.artifact_schema.as_ref() != PROBE_ARTIFACT_SCHEMA_V2 {
            return Err(invalid(format!(
                "unknown probe artifact schema `{}`",
                self.artifact_schema
            )));
        }
        require_nonempty("normalization_version", &self.normalization_version)?;
        require_nonempty(
            "selector_tie_break_version",
            &self.selector_tie_break_version,
        )?;
        require_nonempty("query_name", &self.query_name)?;
        self.identity.validate()?;
        self.initial_frontier.validate("initial_frontier")?;

        validate_dimension_descriptors(&self.dimensions)?;
        if self.dimensions.len() != self.axis_cardinalities.len() {
            return Err(invalid(format!(
                "probe schema has {} dimensions but {} axis cardinalities",
                self.dimensions.len(),
                self.axis_cardinalities.len()
            )));
        }
        if self.selectors.is_empty() {
            return Err(invalid("probe plan must declare at least one selector"));
        }
        let mut unique_selectors = BTreeSet::new();
        for selector in self.selectors.iter().copied() {
            if !unique_selectors.insert(selector) {
                return Err(invalid(format!(
                    "probe selector {selector:?} occurs more than once"
                )));
            }
        }

        if let Some(boundary) = self.boundary {
            if boundary.axis >= self.dimensions.len() {
                return Err(invalid(format!(
                    "probe boundary axis {} is outside {} dimensions",
                    boundary.axis,
                    self.dimensions.len()
                )));
            }
            if boundary.step <= 0 {
                return Err(invalid("probe boundary step must be positive"));
            }
            if self.dimensions[boundary.axis].role != ExploreGeneratorAxisRole::Before {
                return Err(invalid(format!(
                    "probe boundary axis {} must be a Before dimension, found {:?}",
                    boundary.axis, self.dimensions[boundary.axis].role
                )));
            }
            if !boundary.requires_both_endpoints_in_domain {
                return Err(invalid(
                    "probe artifact v2 requires both boundary endpoints to belong to the declared domain",
                ));
            }
        }
        if self.boundary.is_none()
            && self.selectors.iter().any(|selector| {
                matches!(
                    selector,
                    ProbeSelector::BoundaryCandidates | ProbeSelector::BoundaryEndpoints
                )
            })
        {
            return Err(invalid(
                "probe boundary candidate/endpoint selectors require a boundary contract",
            ));
        }

        validate_dimension_index_subset_order(
            "lift dimension",
            self.dimensions.len(),
            &self.lift_dimension_indices,
        )?;
        match self.boundary {
            Some(boundary) => {
                if self.lift_dimension_indices.contains(&boundary.axis) {
                    return Err(invalid(format!(
                        "probe lift dimension {} `{}` is the boundary axis",
                        boundary.axis, self.dimensions[boundary.axis].label
                    )));
                }
            }
            None if !self.lift_dimension_indices.is_empty() => {
                return Err(invalid("probe lift dimensions require a boundary contract"))
            }
            None => {}
        }
        validate_dimension_index_subset_order(
            "retained configuration",
            self.dimensions.len(),
            &self.retained_configuration_dimension_indices,
        )?;
        validate_unique_nonempty("retained key", &self.retained_key_names)?;
        validate_unique_nonempty("retained shown", &self.retained_shown_names)?;
        let mut output_names = BTreeSet::new();
        for name in self
            .retained_key_names
            .iter()
            .chain(self.retained_shown_names.iter())
        {
            if !output_names.insert(name.as_str()) {
                return Err(invalid(format!(
                    "retained output name `{name}` occurs more than once"
                )));
            }
        }

        declared_case_count(&self.axis_cardinalities)?;
        Ok(())
    }

    fn validate_case_id(&self, case_id: &ExploreCaseId) -> Result<(), ProbeValidationError> {
        if case_id.len() != self.dimensions.len() {
            return Err(invalid(format!(
                "probe CaseId has {} ordinals for {} dimensions",
                case_id.len(),
                self.dimensions.len()
            )));
        }
        for (axis, (&ordinal, &cardinality)) in case_id
            .ordinals()
            .iter()
            .zip(self.axis_cardinalities.iter())
            .enumerate()
        {
            if ordinal >= cardinality {
                return Err(invalid(format!(
                    "probe CaseId ordinal {ordinal} is outside axis {axis} with cardinality {cardinality}"
                )));
            }
        }
        Ok(())
    }

    fn validate_lift_edge(
        &self,
        origin_case_id: &ExploreCaseId,
        candidate_case_id: &ExploreCaseId,
    ) -> Result<(), ProbeValidationError> {
        self.validate_case_id(origin_case_id)?;
        self.validate_case_id(candidate_case_id)?;
        let boundary = self.boundary.ok_or_else(|| {
            invalid("probe artifact contains a lifted case without a boundary contract")
        })?;
        if self.lift_dimension_indices.is_empty() {
            return Err(invalid(
                "probe artifact contains a lifted case but its plan declares no lift dimensions",
            ));
        }
        if origin_case_id == candidate_case_id {
            return Err(invalid(
                "lifted probe candidate must be distinct from its observed origin",
            ));
        }
        if origin_case_id.ordinals()[boundary.axis] != candidate_case_id.ordinals()[boundary.axis] {
            return Err(invalid(
                "lifted probe candidate does not preserve its origin boundary ordinal",
            ));
        }
        let allowed = self
            .lift_dimension_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for (axis, (&origin, &candidate)) in origin_case_id
            .ordinals()
            .iter()
            .zip(candidate_case_id.ordinals())
            .enumerate()
        {
            if origin != candidate && !allowed.contains(&axis) {
                return Err(invalid(format!(
                    "lifted probe candidate changes unauthorized axis {axis} `{}`",
                    self.dimensions[axis].label
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProbeFrontierId(Box<str>);

impl ProbeFrontierId {
    pub(crate) fn new(value: impl Into<Box<str>>) -> Result<Self, ProbeValidationError> {
        let value = Self(value.into());
        value.validate("frontier identity")?;
        Ok(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self, field: &str) -> Result<(), ProbeValidationError> {
        require_lowercase_sha256(field, &self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeFrontierState {
    Open(ProbeFrontierId),
    PlanExhausted,
    NoMoreUniqueProbes,
}

impl ProbeFrontierState {
    fn validate(&self) -> Result<(), ProbeValidationError> {
        if let Self::Open(id) = self {
            id.validate("open frontier identity")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeEndpointState {
    Ineligible,
    EligibleUnevaluated,
    Evaluated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeBoundaryEndpoint {
    pub(crate) value: ExploreValue,
    pub(crate) state: ProbeEndpointState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ProbeBoundaryValues {
    pub(crate) lower: Option<ProbeBoundaryEndpoint>,
    pub(crate) upper: Option<ProbeBoundaryEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeNamedValue {
    pub(crate) name: String,
    pub(crate) value: ExploreValue,
}

/// One retained generator coordinate. The dimension index is authoritative;
/// the contract descriptor supplies any presentation label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeDimensionValue {
    pub(crate) dimension_index: usize,
    pub(crate) value: ExploreValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeClassificationKind {
    Excluded,
    Nonmatch,
    Match,
}

/// Exact result of classifying one selected case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeClassification {
    Excluded { reason: Box<str> },
    Nonmatch { question_value: bool },
    Match { question_value: bool },
}

impl ProbeClassification {
    fn kind(&self) -> ProbeClassificationKind {
        match self {
            Self::Excluded { .. } => ProbeClassificationKind::Excluded,
            Self::Nonmatch { .. } => ProbeClassificationKind::Nonmatch,
            Self::Match { .. } => ProbeClassificationKind::Match,
        }
    }

    fn validate(&self, polarity: ExplorePolarity) -> Result<(), ProbeValidationError> {
        match self {
            Self::Excluded { reason } => require_nonempty("probe exclusion reason", reason)?,
            Self::Nonmatch { question_value } | Self::Match { question_value } => {
                let is_match = match polarity {
                    ExplorePolarity::Matches => *question_value,
                    ExplorePolarity::Violations => !*question_value,
                };
                if is_match != matches!(self, Self::Match { .. }) {
                    return Err(invalid(format!(
                        "probe {:?} classification disagrees with {:?} polarity and question value {question_value}",
                        self.kind(), polarity
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Explicit output availability prevents an exclusion from acquiring values
/// that the evaluator never retained. Both structural and validity exclusions
/// are represented as `Unavailable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeRetainedOutputs {
    Unavailable,
    Available {
        key: Box<[ProbeNamedValue]>,
        shown: Box<[ProbeNamedValue]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeMechanismSignatureRef(Box<str>);

impl ProbeMechanismSignatureRef {
    pub(crate) fn new(value: impl Into<Box<str>>) -> Result<Self, ProbeValidationError> {
        let value = Self(value.into());
        require_nonempty("probe mechanism signature reference", &value.0)?;
        Ok(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Deterministic reason for selecting one observed case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeSchedulingReason {
    Selector {
        selector_index: usize,
        selector: ProbeSelector,
        detail: Box<str>,
    },
    Lifted {
        origin_case_id: ExploreCaseId,
    },
}

impl ProbeSchedulingReason {
    fn validate(
        &self,
        scheduled_case_id: &ExploreCaseId,
        contract: &ProbePlanContract,
        observations: &BTreeMap<ExploreCaseId, &ProbeObservation>,
    ) -> Result<(), ProbeValidationError> {
        match self {
            Self::Selector {
                selector_index,
                selector,
                detail,
            } => {
                require_nonempty("probe scheduling detail", detail)?;
                let declared = contract.selectors.get(*selector_index).ok_or_else(|| {
                    invalid(format!(
                        "probe scheduling reason references selector {selector_index}, but the plan has {} selectors",
                        contract.selectors.len()
                    ))
                })?;
                if declared != selector {
                    return Err(invalid(format!(
                        "probe scheduling reason selector {selector:?} disagrees with declared selector {declared:?} at index {selector_index}"
                    )));
                }
            }
            Self::Lifted { origin_case_id } => {
                contract.validate_lift_edge(origin_case_id, scheduled_case_id)?;
                let origin = observations.get(origin_case_id).ok_or_else(|| {
                    invalid(format!(
                        "lifted scheduling reason references unobserved origin {:?}",
                        origin_case_id.ordinals()
                    ))
                })?;
                if origin.classification.kind() != ProbeClassificationKind::Match {
                    return Err(invalid(format!(
                        "lifted scheduling origin {:?} is not a match",
                        origin_case_id.ordinals()
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Replayable evidence for one and only one classified CaseId.
///
/// An excluded boundary case is phase-neutral here: two evaluated endpoints
/// record a constructible transition rejected by validity, while an ineligible
/// endpoint records structural exclusion before a transition exists. Neither
/// acquires outputs or mechanism evidence; a structural exclusion also must
/// not acquire a TransitionId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeObservation {
    pub(crate) case_id: ExploreCaseId,
    pub(crate) configuration: Box<[ProbeDimensionValue]>,
    pub(crate) boundary_values: ProbeBoundaryValues,
    pub(crate) classification: ProbeClassification,
    pub(crate) outputs: ProbeRetainedOutputs,
    pub(crate) scheduling_reason: ProbeSchedulingReason,
    pub(crate) mechanism_signature: Option<ProbeMechanismSignatureRef>,
}

/// One adaptive scheduling choice and the exact state transition it caused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeDecision {
    pub(crate) sequence: u128,
    pub(crate) observed_before: u128,
    pub(crate) observed_after: u128,
    pub(crate) frontier_before: ProbeFrontierId,
    pub(crate) selected_case_id: ExploreCaseId,
    pub(crate) scheduling_reason: ProbeSchedulingReason,
    pub(crate) classification: ProbeClassificationKind,
    pub(crate) frontier_after: ProbeFrontierState,
}

/// A pending lift is scheduling data only.  Its shape intentionally has no
/// classification, output, endpoint, replay or mechanism field to inherit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeLiftedCandidate {
    pub(crate) origin_case_id: ExploreCaseId,
    pub(crate) candidate_case_id: ExploreCaseId,
    pub(crate) fixed_boundary_value: ExploreValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeCounts {
    /// Distinct observed or currently queued CaseIds known to the plan.
    pub(crate) planned_distinct_cases: u128,
    pub(crate) observed_distinct_cases: u128,
    pub(crate) pending_distinct_cases: u128,
    pub(crate) remaining_case_budget: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbePartialReason {
    Interrupted,
    Timeout,
    ArtifactSizeLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeCompletionReason {
    BudgetReached,
    PlanExhausted,
    NoMoreUniqueProbes,
}

/// Artifact completion is only completion of the finite warm-up plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeArtifactState {
    Partial { reason: ProbePartialReason },
    Complete { reason: ProbeCompletionReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeCursor {
    pub(crate) next_decision: u128,
    pub(crate) frontier: ProbeFrontierState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeArtifact {
    pub(crate) contract: ProbePlanContract,
    pub(crate) state: ProbeArtifactState,
    pub(crate) cursor: ProbeCursor,
    pub(crate) counts: ProbeCounts,
    /// Canonical CaseId order, independent of adaptive scheduling order.
    pub(crate) observations: Box<[ProbeObservation]>,
    /// Adaptive scheduling order, with a monotone frontier chain.
    pub(crate) transcript: Box<[ProbeDecision]>,
    /// Canonical `(origin CaseId, candidate CaseId)` edge order.
    pub(crate) lifted_candidates: Box<[ProbeLiftedCandidate]>,
}

impl ProbeArtifact {
    pub(crate) fn validate(&self) -> Result<(), ProbeValidationError> {
        self.contract.validate()?;
        self.cursor.frontier.validate()?;
        self.validate_observations_and_transcript()?;
        self.validate_lifted_candidates()?;
        self.validate_counts()?;
        self.validate_state()?;
        Ok(())
    }

    pub(crate) fn probe_only_status(&self) -> ProbeOnlyStatus {
        ProbeOnlyStatus {
            phase: ProbeCommandPhase::Probe,
            probe_status: match self.state {
                ProbeArtifactState::Partial { .. } => ProbeRunStatus::Partial,
                ProbeArtifactState::Complete { .. } => ProbeRunStatus::Complete,
            },
            answer_status: ProbeAnswerStatus::NotStarted,
        }
    }

    fn validate_observations_and_transcript(&self) -> Result<(), ProbeValidationError> {
        for pair in self.observations.windows(2) {
            if pair[0].case_id >= pair[1].case_id {
                return Err(invalid(
                    "probe observations must have distinct CaseIds in canonical order",
                ));
            }
        }

        let observations = self
            .observations
            .iter()
            .map(|observation| (observation.case_id.clone(), observation))
            .collect::<BTreeMap<_, _>>();
        for observation in self.observations.iter() {
            self.validate_observation(observation, &observations)?;
        }

        if self.transcript.len() != self.observations.len() {
            return Err(invalid(format!(
                "probe transcript has {} decisions for {} observations",
                self.transcript.len(),
                self.observations.len()
            )));
        }
        let mut selected = BTreeSet::<ExploreCaseId>::new();
        let mut last_selector_index = None;
        let mut expected_frontier =
            ProbeFrontierState::Open(self.contract.initial_frontier.clone());
        for (index, decision) in self.transcript.iter().enumerate() {
            let sequence = index as u128;
            if decision.sequence != sequence
                || decision.observed_before != sequence
                || decision.observed_after != sequence.saturating_add(1)
            {
                return Err(invalid(format!(
                    "probe transcript decision {index} has non-monotone sequence/counts ({}, {} -> {})",
                    decision.sequence, decision.observed_before, decision.observed_after
                )));
            }
            match &expected_frontier {
                ProbeFrontierState::Open(expected) if expected == &decision.frontier_before => {}
                ProbeFrontierState::Open(expected) => {
                    return Err(invalid(format!(
                        "probe transcript decision {index} begins at {:?}, expected {:?}",
                        decision.frontier_before, expected
                    )))
                }
                terminal => {
                    return Err(invalid(format!(
                        "probe transcript continues after terminal frontier {terminal:?}"
                    )))
                }
            }
            self.contract.validate_case_id(&decision.selected_case_id)?;
            match &decision.scheduling_reason {
                ProbeSchedulingReason::Selector { selector_index, .. } => {
                    if last_selector_index.is_some_and(|last| *selector_index < last) {
                        return Err(invalid(format!(
                            "probe transcript decision {index} returns from selector {last_selector_index:?} to earlier selector {selector_index}"
                        )));
                    }
                    last_selector_index = Some(*selector_index);
                }
                ProbeSchedulingReason::Lifted { origin_case_id } => {
                    if !selected.contains(origin_case_id) {
                        return Err(invalid(format!(
                            "probe transcript decision {index} was lifted from origin {:?} before that origin was observed",
                            origin_case_id.ordinals()
                        )));
                    }
                }
            }
            if !selected.insert(decision.selected_case_id.clone()) {
                return Err(invalid(format!(
                    "probe transcript selects CaseId {:?} more than once",
                    decision.selected_case_id.ordinals()
                )));
            }
            let observation = observations
                .get(&decision.selected_case_id)
                .ok_or_else(|| {
                    invalid(format!(
                        "probe transcript CaseId {:?} has no observation",
                        decision.selected_case_id.ordinals()
                    ))
                })?;
            if decision.classification != observation.classification.kind() {
                return Err(invalid(format!(
                    "probe transcript classification for CaseId {:?} disagrees with its observation",
                    decision.selected_case_id.ordinals()
                )));
            }
            if decision.scheduling_reason != observation.scheduling_reason {
                return Err(invalid(format!(
                    "probe transcript scheduling reason for CaseId {:?} disagrees with its observation",
                    decision.selected_case_id.ordinals()
                )));
            }
            decision.scheduling_reason.validate(
                &decision.selected_case_id,
                &self.contract,
                &observations,
            )?;
            decision.frontier_after.validate()?;
            expected_frontier = decision.frontier_after.clone();
        }

        if self.cursor.next_decision != self.transcript.len() as u128 {
            return Err(invalid(format!(
                "probe cursor next decision {} disagrees with transcript length {}",
                self.cursor.next_decision,
                self.transcript.len()
            )));
        }
        if self.transcript.is_empty() {
            let initial = ProbeFrontierState::Open(self.contract.initial_frontier.clone());
            if self.cursor.frontier != initial
                && !matches!(
                    &self.cursor.frontier,
                    ProbeFrontierState::PlanExhausted | ProbeFrontierState::NoMoreUniqueProbes
                )
            {
                return Err(invalid(
                    "empty probe transcript cursor is neither the initial nor a terminal frontier",
                ));
            }
        } else if self.cursor.frontier != expected_frontier {
            return Err(invalid(
                "probe cursor frontier disagrees with the final transcript decision",
            ));
        }
        Ok(())
    }

    fn validate_observation(
        &self,
        observation: &ProbeObservation,
        observations: &BTreeMap<ExploreCaseId, &ProbeObservation>,
    ) -> Result<(), ProbeValidationError> {
        self.contract.validate_case_id(&observation.case_id)?;
        observation
            .classification
            .validate(self.contract.polarity)?;
        self.validate_boundary_values(observation)?;
        validate_dimension_values_exact(
            "retained configuration",
            &observation.configuration,
            &self.contract.dimensions,
            &self.contract.retained_configuration_dimension_indices,
        )?;
        observation.scheduling_reason.validate(
            &observation.case_id,
            &self.contract,
            observations,
        )?;

        match (&observation.classification, &observation.outputs) {
            (ProbeClassification::Excluded { .. }, ProbeRetainedOutputs::Unavailable) => {}
            (ProbeClassification::Excluded { .. }, ProbeRetainedOutputs::Available { .. }) => {
                return Err(invalid(format!(
                    "excluded probe CaseId {:?} must retain outputs as unavailable",
                    observation.case_id.ordinals()
                )))
            }
            (
                ProbeClassification::Match { .. } | ProbeClassification::Nonmatch { .. },
                ProbeRetainedOutputs::Unavailable,
            ) => {
                return Err(invalid(format!(
                    "admissible probe CaseId {:?} must explicitly retain its authorized outputs",
                    observation.case_id.ordinals()
                )))
            }
            (
                ProbeClassification::Match { .. } | ProbeClassification::Nonmatch { .. },
                ProbeRetainedOutputs::Available { key, shown },
            ) => {
                validate_named_values_exact(
                    "retained key",
                    key,
                    &self.contract.retained_key_names,
                )?;
                validate_named_values_exact(
                    "retained shown",
                    shown,
                    &self.contract.retained_shown_names,
                )?;
            }
        }

        if matches!(
            &observation.classification,
            ProbeClassification::Excluded { .. }
        ) && observation.mechanism_signature.is_some()
        {
            return Err(invalid(format!(
                "excluded probe CaseId {:?} must not retain a mechanism signature",
                observation.case_id.ordinals()
            )));
        }
        if observation.mechanism_signature.is_some() && !self.contract.mechanism_trace_authorized {
            return Err(invalid(format!(
                "probe CaseId {:?} retains a mechanism signature without authorization",
                observation.case_id.ordinals()
            )));
        }
        if let Some(signature) = &observation.mechanism_signature {
            require_nonempty("probe mechanism signature reference", &signature.0)?;
        }
        Ok(())
    }

    fn validate_boundary_values(
        &self,
        observation: &ProbeObservation,
    ) -> Result<(), ProbeValidationError> {
        let (lower, upper) = match self.contract.boundary {
            None => {
                if observation.boundary_values.lower.is_some()
                    || observation.boundary_values.upper.is_some()
                {
                    return Err(invalid(format!(
                        "non-boundary probe CaseId {:?} retains boundary endpoint evidence",
                        observation.case_id.ordinals()
                    )));
                }
                return Ok(());
            }
            Some(_) => {
                let lower = observation.boundary_values.lower.as_ref().ok_or_else(|| {
                    invalid(format!(
                        "boundary probe CaseId {:?} omits its lower endpoint",
                        observation.case_id.ordinals()
                    ))
                })?;
                let upper = observation.boundary_values.upper.as_ref().ok_or_else(|| {
                    invalid(format!(
                        "boundary probe CaseId {:?} omits its upper endpoint",
                        observation.case_id.ordinals()
                    ))
                })?;
                (lower, upper)
            }
        };
        let boundary = self
            .contract
            .boundary
            .expect("boundary endpoints were required only for a boundary plan");
        let lower_value = lower.value.int().ok_or_else(|| {
            invalid(format!(
                "boundary probe CaseId {:?} has a non-Int lower endpoint",
                observation.case_id.ordinals()
            ))
        })?;
        let upper_value = upper.value.int().ok_or_else(|| {
            invalid(format!(
                "boundary probe CaseId {:?} has a non-Int upper endpoint",
                observation.case_id.ordinals()
            ))
        })?;
        let expected_upper = lower_value.checked_add(boundary.step).ok_or_else(|| {
            invalid(format!(
                "boundary probe CaseId {:?} endpoint step overflows Int",
                observation.case_id.ordinals()
            ))
        })?;
        if upper_value != expected_upper {
            return Err(invalid(format!(
                "boundary probe CaseId {:?} endpoints {lower_value} and {upper_value} disagree with step {}",
                observation.case_id.ordinals(), boundary.step
            )));
        }

        match &observation.classification {
            ProbeClassification::Excluded { .. } => {
                let validity_excluded = lower.state == ProbeEndpointState::Evaluated
                    && upper.state == ProbeEndpointState::Evaluated;
                let structurally_excluded = lower.state == ProbeEndpointState::Ineligible
                    || upper.state == ProbeEndpointState::Ineligible;
                if !validity_excluded && !structurally_excluded {
                    return Err(invalid(format!(
                        "excluded boundary probe CaseId {:?} has neither two evaluated endpoints nor a structurally ineligible endpoint",
                        observation.case_id.ordinals()
                    )));
                }
            }
            ProbeClassification::Nonmatch { .. } | ProbeClassification::Match { .. } => {
                if lower.state != ProbeEndpointState::Evaluated
                    || upper.state != ProbeEndpointState::Evaluated
                {
                    return Err(invalid(format!(
                        "classified boundary probe CaseId {:?} has an unevaluated or ineligible endpoint",
                        observation.case_id.ordinals()
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_lifted_candidates(&self) -> Result<(), ProbeValidationError> {
        let observations = self
            .observations
            .iter()
            .map(|observation| (observation.case_id.clone(), observation))
            .collect::<BTreeMap<_, _>>();
        let mut previous: Option<(&ExploreCaseId, &ExploreCaseId)> = None;
        for lift in self.lifted_candidates.iter() {
            self.contract
                .validate_lift_edge(&lift.origin_case_id, &lift.candidate_case_id)?;
            if let Some((previous_origin, previous_candidate)) = previous {
                if (previous_origin, previous_candidate)
                    >= (&lift.origin_case_id, &lift.candidate_case_id)
                {
                    return Err(invalid(
                        "lifted probe edges must be distinct and in canonical origin/candidate order",
                    ));
                }
            }
            previous = Some((&lift.origin_case_id, &lift.candidate_case_id));

            let origin = observations.get(&lift.origin_case_id).ok_or_else(|| {
                invalid(format!(
                    "lifted probe candidate {:?} references unobserved origin {:?}",
                    lift.candidate_case_id.ordinals(),
                    lift.origin_case_id.ordinals()
                ))
            })?;
            if origin.classification.kind() != ProbeClassificationKind::Match {
                return Err(invalid(format!(
                    "lifted probe origin {:?} is not a match",
                    lift.origin_case_id.ordinals()
                )));
            }
            if observations.contains_key(&lift.candidate_case_id) {
                return Err(invalid(format!(
                    "lifted probe candidate {:?} is recorded as observed and therefore is not unevaluated",
                    lift.candidate_case_id.ordinals()
                )));
            }
            let lower = origin.boundary_values.lower.as_ref().ok_or_else(|| {
                invalid(format!(
                    "lifted probe origin {:?} has no retained lower boundary value",
                    lift.origin_case_id.ordinals()
                ))
            })?;
            if lower.value != lift.fixed_boundary_value {
                return Err(invalid(format!(
                    "lifted probe candidate {:?} does not retain its origin's lower boundary value",
                    lift.candidate_case_id.ordinals()
                )));
            }
        }
        Ok(())
    }

    fn validate_counts(&self) -> Result<(), ProbeValidationError> {
        let observed = self.observations.len() as u128;
        let pending = self
            .lifted_candidates
            .iter()
            .map(|candidate| &candidate.candidate_case_id)
            .collect::<BTreeSet<_>>()
            .len() as u128;
        let planned = observed
            .checked_add(pending)
            .ok_or_else(|| invalid("probe planned case count exceeds u128::MAX"))?;
        let cap = self.contract.semantic_case_cap.get();
        let remaining = cap.checked_sub(observed).ok_or_else(|| {
            invalid(format!(
                "probe observed count {observed} exceeds semantic case cap {cap}"
            ))
        })?;
        let expected = ProbeCounts {
            planned_distinct_cases: planned,
            observed_distinct_cases: observed,
            pending_distinct_cases: pending,
            remaining_case_budget: remaining,
        };
        if self.counts != expected {
            return Err(invalid(format!(
                "probe counts {:?} disagree with deterministic counts {:?}",
                self.counts, expected
            )));
        }

        let universe = declared_case_count(&self.contract.axis_cardinalities)?;
        if planned > universe {
            return Err(invalid(format!(
                "probe has {planned} distinct planned cases in a universe of {universe}"
            )));
        }
        Ok(())
    }

    fn validate_state(&self) -> Result<(), ProbeValidationError> {
        match self.state {
            ProbeArtifactState::Partial { .. } => {
                if self.counts.remaining_case_budget == 0 {
                    return Err(invalid(
                        "partial probe artifact already reached its semantic case cap",
                    ));
                }
                if !matches!(&self.cursor.frontier, ProbeFrontierState::Open(_)) {
                    return Err(invalid(
                        "partial probe artifact has a semantically terminal frontier",
                    ));
                }
            }
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::BudgetReached,
            } => {
                if self.counts.remaining_case_budget != 0 {
                    return Err(invalid(
                        "budget-complete probe artifact has remaining case budget",
                    ));
                }
            }
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            } => {
                if self.cursor.frontier != ProbeFrontierState::PlanExhausted {
                    return Err(invalid(
                        "plan-exhausted probe artifact lacks a plan-exhausted frontier",
                    ));
                }
                if self.counts.pending_distinct_cases != 0 {
                    return Err(invalid(
                        "plan-exhausted probe artifact still has pending lifted candidates",
                    ));
                }
            }
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::NoMoreUniqueProbes,
            } => {
                if self.cursor.frontier != ProbeFrontierState::NoMoreUniqueProbes {
                    return Err(invalid(
                        "no-more-unique-probes artifact lacks the matching terminal frontier",
                    ));
                }
                if self.counts.pending_distinct_cases != 0 {
                    return Err(invalid(
                        "no-more-unique-probes artifact still has pending lifted candidates",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeCommandPhase {
    Probe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeRunStatus {
    Partial,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeAnswerStatus {
    NotStarted,
}

/// Probe-only status cannot accidentally be represented as an Explore result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeOnlyStatus {
    pub(crate) phase: ProbeCommandPhase,
    pub(crate) probe_status: ProbeRunStatus,
    pub(crate) answer_status: ProbeAnswerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorruptProbeArtifact {
    pub(crate) reason: Box<str>,
}

impl CorruptProbeArtifact {
    pub(crate) fn new(reason: impl Into<Box<str>>) -> Result<Self, ProbeValidationError> {
        let reason = reason.into();
        require_nonempty("corrupt probe artifact reason", &reason)?;
        Ok(Self { reason })
    }
}

pub(crate) enum ProbeArtifactAtEntry<'a> {
    Missing,
    Parsed(&'a ProbeArtifact),
    Corrupt(CorruptProbeArtifact),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeLifecycleAction {
    RunProbeOnly,
    ResumeProbeOnly,
    RefreshProbeOnly,
    ReplayThenExplore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeLifecycleError {
    InvalidCurrentPlan(ProbeValidationError),
    StaleArtifact,
    CorruptArtifact(ProbeValidationError),
    UnreadableArtifact(CorruptProbeArtifact),
}

impl fmt::Display for ProbeLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCurrentPlan(error) => write!(formatter, "invalid current probe plan: {error}"),
            Self::StaleArtifact => write!(
                formatter,
                "probe artifact does not match the current semantic probe plan; explicit refresh required"
            ),
            Self::CorruptArtifact(error) => write!(
                formatter,
                "probe artifact is corrupt ({error}); explicit refresh required"
            ),
            Self::UnreadableArtifact(error) => write!(
                formatter,
                "probe artifact is unreadable ({}); explicit refresh required",
                error.reason
            ),
        }
    }
}

impl Error for ProbeLifecycleError {}

/// Choose the invocation phase solely from state present at invocation entry.
/// A probe-building invocation never falls through to Explore.
pub(crate) fn plan_probe_lifecycle(
    current: &ProbePlanContract,
    artifact: ProbeArtifactAtEntry<'_>,
    explicit_refresh: bool,
) -> Result<ProbeLifecycleAction, ProbeLifecycleError> {
    current
        .validate()
        .map_err(ProbeLifecycleError::InvalidCurrentPlan)?;

    if explicit_refresh {
        return Ok(match artifact {
            ProbeArtifactAtEntry::Missing => ProbeLifecycleAction::RunProbeOnly,
            ProbeArtifactAtEntry::Parsed(_) | ProbeArtifactAtEntry::Corrupt(_) => {
                ProbeLifecycleAction::RefreshProbeOnly
            }
        });
    }

    let artifact = match artifact {
        ProbeArtifactAtEntry::Missing => return Ok(ProbeLifecycleAction::RunProbeOnly),
        ProbeArtifactAtEntry::Parsed(artifact) => artifact,
        ProbeArtifactAtEntry::Corrupt(error) => {
            return Err(ProbeLifecycleError::UnreadableArtifact(error))
        }
    };
    artifact
        .validate()
        .map_err(ProbeLifecycleError::CorruptArtifact)?;
    if &artifact.contract != current {
        return Err(ProbeLifecycleError::StaleArtifact);
    }

    Ok(match artifact.state {
        ProbeArtifactState::Partial { .. } => ProbeLifecycleAction::ResumeProbeOnly,
        ProbeArtifactState::Complete { .. } => ProbeLifecycleAction::ReplayThenExplore,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeValidationError {
    message: String,
}

impl fmt::Display for ProbeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ProbeValidationError {}

fn invalid(message: impl Into<String>) -> ProbeValidationError {
    ProbeValidationError {
        message: message.into(),
    }
}

fn require_nonempty(field: &str, value: &str) -> Result<(), ProbeValidationError> {
    if value.is_empty() {
        Err(invalid(format!("probe {field} must not be empty")))
    } else {
        Ok(())
    }
}

fn require_lowercase_sha256(field: &str, value: &str) -> Result<(), ProbeValidationError> {
    let is_lowercase_sha256 = value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if is_lowercase_sha256 {
        Ok(())
    } else {
        Err(invalid(format!(
            "probe {field} must be a lowercase SHA-256 digest"
        )))
    }
}

fn validate_unique_nonempty(kind: &str, names: &[String]) -> Result<(), ProbeValidationError> {
    let mut unique = BTreeSet::new();
    for name in names {
        require_nonempty(kind, name)?;
        if !unique.insert(name.as_str()) {
            return Err(invalid(format!(
                "probe {kind} `{name}` occurs more than once"
            )));
        }
    }
    Ok(())
}

fn validate_dimension_descriptors(
    dimensions: &[ProbeDimensionDescriptor],
) -> Result<(), ProbeValidationError> {
    let mut bound_indices = BTreeSet::new();
    let mut role_fields = BTreeSet::new();
    let mut previous = None;
    for (dimension_index, dimension) in dimensions.iter().enumerate() {
        require_nonempty("dimension label", &dimension.label)?;
        if !bound_indices.insert(dimension.bound_index) {
            return Err(invalid(format!(
                "probe dimension bound index {} occurs more than once",
                dimension.bound_index
            )));
        }
        if !role_fields.insert((dimension.role, dimension.role_field_index)) {
            return Err(invalid(format!(
                "probe {:?} field index {} occurs in more than one dimension",
                dimension.role, dimension.role_field_index
            )));
        }
        let key = (
            dimension.role,
            dimension.role_field_index,
            dimension.bound_index,
        );
        if previous.is_some_and(|previous| previous >= key) {
            return Err(invalid(format!(
                "probe dimension {dimension_index} is not in canonical Context, Before, AfterIndependent field order"
            )));
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_dimension_index_subset_order(
    kind: &str,
    dimension_count: usize,
    subset: &[usize],
) -> Result<(), ProbeValidationError> {
    let mut previous = None;
    for &dimension_index in subset {
        if dimension_index >= dimension_count {
            return Err(invalid(format!(
                "probe {kind} index {dimension_index} is outside {dimension_count} dimensions"
            )));
        }
        if previous.is_some_and(|previous| previous >= dimension_index) {
            return Err(invalid(format!(
                "probe {kind} indices are not distinct and in canonical dimension order"
            )));
        }
        previous = Some(dimension_index);
    }
    Ok(())
}

fn validate_dimension_values_exact(
    kind: &str,
    values: &[ProbeDimensionValue],
    dimensions: &[ProbeDimensionDescriptor],
    selected_dimension_indices: &[usize],
) -> Result<(), ProbeValidationError> {
    if values.len() != selected_dimension_indices.len() {
        return Err(invalid(format!(
            "probe {kind} has {} values for {} authorized dimensions",
            values.len(),
            selected_dimension_indices.len()
        )));
    }
    for (index, (value, &dimension_index)) in
        values.iter().zip(selected_dimension_indices).enumerate()
    {
        let _dimension = dimensions.get(dimension_index).ok_or_else(|| {
            invalid(format!(
                "probe {kind} references absent dimension index {dimension_index}"
            ))
        })?;
        if value.dimension_index != dimension_index {
            return Err(invalid(format!(
                "probe {kind} value {index} references dimension {}, expected {dimension_index}",
                value.dimension_index
            )));
        }
    }
    Ok(())
}

fn validate_named_values_exact(
    kind: &str,
    values: &[ProbeNamedValue],
    authorized_names: &[String],
) -> Result<(), ProbeValidationError> {
    if values.len() != authorized_names.len() {
        return Err(invalid(format!(
            "probe {kind} has {} values for {} authorized names",
            values.len(),
            authorized_names.len()
        )));
    }
    for (index, (value, authorized)) in values.iter().zip(authorized_names).enumerate() {
        if value.name != *authorized {
            return Err(invalid(format!(
                "probe {kind} value {index} is named `{}`, expected authorized name `{authorized}`",
                value.name
            )));
        }
    }
    Ok(())
}

fn declared_case_count(axis_cardinalities: &[u128]) -> Result<u128, ProbeValidationError> {
    if axis_cardinalities.contains(&0) {
        return Ok(0);
    }
    axis_cardinalities
        .iter()
        .copied()
        .try_fold(1_u128, u128::checked_mul)
        .ok_or_else(|| invalid("probe declared case count exceeds u128::MAX"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> Box<str> {
        let value = seed.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            hash.wrapping_mul(0x100000001b3) ^ u64::from(byte)
        });
        format!("{value:064x}").into()
    }

    fn identity(suffix: &str) -> ProbeSemanticIdentity {
        ProbeSemanticIdentity {
            program_hash: digest(&format!("program-{suffix}")),
            analysis_program_hash: digest(&format!("analysis-{suffix}")),
            query_hash: digest(&format!("query-{suffix}")),
            domain_hash: digest(&format!("domain-{suffix}")),
            probe_plan_hash: digest(&format!("plan-{suffix}")),
            evaluator_contract_hash: digest(&format!("evaluator-{suffix}")),
        }
    }

    fn dimension(
        bound_index: usize,
        role: ExploreGeneratorAxisRole,
        role_field_index: usize,
        label: &str,
    ) -> ProbeDimensionDescriptor {
        ProbeDimensionDescriptor {
            bound_index,
            role,
            role_field_index,
            label: label.to_string(),
        }
    }

    fn contract(suffix: &str, cap: u128) -> ProbePlanContract {
        ProbePlanContract {
            artifact_schema: PROBE_ARTIFACT_SCHEMA_V2.into(),
            normalization_version: "normalization-v2".into(),
            selector_tie_break_version: "tie-break-v1".into(),
            query_name: "income_cliffs".into(),
            identity: identity(suffix),
            polarity: ExplorePolarity::Violations,
            dimensions: vec![
                dimension(0, ExploreGeneratorAxisRole::Context, 0, "commune"),
                dimension(1, ExploreGeneratorAxisRole::Before, 0, "income"),
            ]
            .into_boxed_slice(),
            axis_cardinalities: vec![2, 3].into_boxed_slice(),
            boundary: Some(ProbeBoundaryContract {
                axis: 1,
                step: 1,
                requires_both_endpoints_in_domain: true,
            }),
            selectors: vec![
                ProbeSelector::BoundaryCandidates,
                ProbeSelector::FrontierMidpoints,
            ]
            .into_boxed_slice(),
            semantic_case_cap: NonZeroU128::new(cap).unwrap(),
            initial_frontier: ProbeFrontierId::new(digest("root")).unwrap(),
            lift_dimension_indices: vec![0].into_boxed_slice(),
            retained_configuration_dimension_indices: vec![0, 1].into_boxed_slice(),
            retained_key_names: vec!["income_before".to_string()].into_boxed_slice(),
            retained_shown_names: vec!["loss_ore".to_string()].into_boxed_slice(),
            mechanism_trace_authorized: false,
        }
    }

    fn named(name: &str, value: i64) -> ProbeNamedValue {
        ProbeNamedValue {
            name: name.to_string(),
            value: ExploreValue::Int(value),
        }
    }

    fn dimension_value(dimension_index: usize, value: i64) -> ProbeDimensionValue {
        ProbeDimensionValue {
            dimension_index,
            value: ExploreValue::Int(value),
        }
    }

    fn selector_reason() -> ProbeSchedulingReason {
        ProbeSchedulingReason::Selector {
            selector_index: 0,
            selector: ProbeSelector::BoundaryCandidates,
            detail: "source-event-1".into(),
        }
    }

    fn observation(case_id: ExploreCaseId) -> ProbeObservation {
        ProbeObservation {
            case_id,
            configuration: vec![dimension_value(0, 0), dimension_value(1, 100)].into_boxed_slice(),
            boundary_values: ProbeBoundaryValues {
                lower: Some(ProbeBoundaryEndpoint {
                    value: ExploreValue::Int(100),
                    state: ProbeEndpointState::Evaluated,
                }),
                upper: Some(ProbeBoundaryEndpoint {
                    value: ExploreValue::Int(101),
                    state: ProbeEndpointState::Evaluated,
                }),
            },
            classification: ProbeClassification::Match {
                question_value: false,
            },
            outputs: ProbeRetainedOutputs::Available {
                key: vec![named("income_before", 100)].into_boxed_slice(),
                shown: vec![named("loss_ore", 20)].into_boxed_slice(),
            },
            scheduling_reason: selector_reason(),
            mechanism_signature: None,
        }
    }

    fn artifact(
        contract: ProbePlanContract,
        state: ProbeArtifactState,
        frontier: ProbeFrontierState,
    ) -> ProbeArtifact {
        let case_id = ExploreCaseId::new(vec![0, 1]);
        let observation = observation(case_id.clone());
        let decision = ProbeDecision {
            sequence: 0,
            observed_before: 0,
            observed_after: 1,
            frontier_before: contract.initial_frontier.clone(),
            selected_case_id: case_id,
            scheduling_reason: selector_reason(),
            classification: ProbeClassificationKind::Match,
            frontier_after: frontier.clone(),
        };
        let remaining = contract.semantic_case_cap.get() - 1;
        ProbeArtifact {
            contract,
            state,
            cursor: ProbeCursor {
                next_decision: 1,
                frontier,
            },
            counts: ProbeCounts {
                planned_distinct_cases: 1,
                observed_distinct_cases: 1,
                pending_distinct_cases: 0,
                remaining_case_budget: remaining,
            },
            observations: vec![observation].into_boxed_slice(),
            transcript: vec![decision].into_boxed_slice(),
            lifted_candidates: Vec::new().into_boxed_slice(),
        }
    }

    #[test]
    fn lifecycle_never_falls_through_from_probe_building_to_explore() {
        let contract = contract("same", 2);
        assert_eq!(
            plan_probe_lifecycle(&contract, ProbeArtifactAtEntry::Missing, false).unwrap(),
            ProbeLifecycleAction::RunProbeOnly
        );

        let partial = artifact(
            contract.clone(),
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("next")).unwrap()),
        );
        assert_eq!(
            plan_probe_lifecycle(&contract, ProbeArtifactAtEntry::Parsed(&partial), false).unwrap(),
            ProbeLifecycleAction::ResumeProbeOnly
        );

        let complete = artifact(
            contract.clone(),
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            ProbeFrontierState::PlanExhausted,
        );
        assert_eq!(
            plan_probe_lifecycle(&contract, ProbeArtifactAtEntry::Parsed(&complete), false)
                .unwrap(),
            ProbeLifecycleAction::ReplayThenExplore
        );
        assert_eq!(
            complete.probe_only_status().answer_status,
            ProbeAnswerStatus::NotStarted
        );
    }

    #[test]
    fn stale_or_corrupt_artifacts_fail_closed_until_explicit_refresh() {
        let current = contract("current", 2);
        let stale_contract = contract("stale", 2);
        let stale = artifact(
            stale_contract,
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            ProbeFrontierState::PlanExhausted,
        );
        assert!(matches!(
            plan_probe_lifecycle(&current, ProbeArtifactAtEntry::Parsed(&stale), false),
            Err(ProbeLifecycleError::StaleArtifact)
        ));
        assert_eq!(
            plan_probe_lifecycle(&current, ProbeArtifactAtEntry::Parsed(&stale), true).unwrap(),
            ProbeLifecycleAction::RefreshProbeOnly
        );

        let unreadable = CorruptProbeArtifact::new("truncated JSON").unwrap();
        assert!(matches!(
            plan_probe_lifecycle(
                &current,
                ProbeArtifactAtEntry::Corrupt(unreadable.clone()),
                false
            ),
            Err(ProbeLifecycleError::UnreadableArtifact(_))
        ));
        assert_eq!(
            plan_probe_lifecycle(&current, ProbeArtifactAtEntry::Corrupt(unreadable), true)
                .unwrap(),
            ProbeLifecycleAction::RefreshProbeOnly
        );
    }

    #[test]
    fn malformed_hashes_are_corrupt_instead_of_stale() {
        let current = contract("current", 2);
        let mut malformed = artifact(
            current.clone(),
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            ProbeFrontierState::PlanExhausted,
        );
        malformed.contract.identity.query_hash = "A".repeat(64).into();
        assert!(matches!(
            plan_probe_lifecycle(&current, ProbeArtifactAtEntry::Parsed(&malformed), false),
            Err(ProbeLifecycleError::CorruptArtifact(_))
        ));

        malformed.contract.identity.query_hash = digest("current-query");
        malformed.contract.initial_frontier = ProbeFrontierId("not-a-sha256-frontier".into());
        assert!(matches!(
            plan_probe_lifecycle(&current, ProbeArtifactAtEntry::Parsed(&malformed), false),
            Err(ProbeLifecycleError::CorruptArtifact(_))
        ));
    }

    #[test]
    fn plan_exhaustion_can_complete_below_the_numeric_cap() {
        let artifact = artifact(
            contract("same", 100),
            ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            ProbeFrontierState::PlanExhausted,
        );
        artifact.validate().unwrap();
        assert_eq!(artifact.counts.observed_distinct_cases, 1);
        assert_eq!(artifact.counts.remaining_case_budget, 99);

        let mut invalid_partial = artifact.clone();
        invalid_partial.state = ProbeArtifactState::Partial {
            reason: ProbePartialReason::Timeout,
        };
        assert!(invalid_partial.validate().is_err());
    }

    #[test]
    fn classification_respects_matches_or_violations_polarity() {
        let mut artifact = artifact(
            contract("same", 2),
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("next")).unwrap()),
        );
        artifact.observations[0].classification = ProbeClassification::Match {
            question_value: true,
        };
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn nonmatch_retains_the_authorized_output_shape() {
        let mut artifact = artifact(
            contract("same", 2),
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("next")).unwrap()),
        );
        artifact.observations[0].classification = ProbeClassification::Nonmatch {
            question_value: true,
        };
        artifact.transcript[0].classification = ProbeClassificationKind::Nonmatch;
        artifact.validate().unwrap();

        artifact.observations[0].outputs = ProbeRetainedOutputs::Unavailable;
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn excluded_boundary_probe_preserves_structural_or_validity_phase() {
        let mut validity_excluded = artifact(
            contract("validity-excluded", 2),
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(
                ProbeFrontierId::new(digest("validity-excluded-next")).unwrap(),
            ),
        );
        validity_excluded.observations[0].classification = ProbeClassification::Excluded {
            reason: "transition validity rejected both evaluated endpoints".into(),
        };
        validity_excluded.observations[0].outputs = ProbeRetainedOutputs::Unavailable;
        validity_excluded.transcript[0].classification = ProbeClassificationKind::Excluded;
        validity_excluded.validate().unwrap();

        let mut structurally_excluded = validity_excluded.clone();
        structurally_excluded.observations[0]
            .boundary_values
            .upper
            .as_mut()
            .unwrap()
            .state = ProbeEndpointState::Ineligible;
        structurally_excluded.observations[0].classification = ProbeClassification::Excluded {
            reason: "upper endpoint is structurally ineligible".into(),
        };
        structurally_excluded.validate().unwrap();

        let mut fabricated_outputs = structurally_excluded.clone();
        fabricated_outputs.observations[0].outputs = ProbeRetainedOutputs::Available {
            key: vec![named("income_before", 100)].into_boxed_slice(),
            shown: vec![named("loss_ore", 20)].into_boxed_slice(),
        };
        assert!(fabricated_outputs
            .validate()
            .unwrap_err()
            .to_string()
            .contains("must retain outputs as unavailable"));

        structurally_excluded.contract.mechanism_trace_authorized = true;
        structurally_excluded.observations[0].mechanism_signature =
            Some(ProbeMechanismSignatureRef::new("fabricated-structural-signature").unwrap());
        assert!(structurally_excluded
            .validate()
            .unwrap_err()
            .to_string()
            .contains("must not retain a mechanism signature"));

        let mut incomplete_exclusion = validity_excluded;
        incomplete_exclusion.observations[0]
            .boundary_values
            .upper
            .as_mut()
            .unwrap()
            .state = ProbeEndpointState::EligibleUnevaluated;
        assert!(incomplete_exclusion
            .validate()
            .unwrap_err()
            .to_string()
            .contains("neither two evaluated endpoints nor a structurally ineligible endpoint"));
    }

    #[test]
    fn non_boundary_probe_has_no_endpoint_evidence() {
        let mut plan = contract("generic", 2);
        plan.boundary = None;
        plan.lift_dimension_indices = Vec::new().into_boxed_slice();
        plan.selectors = vec![ProbeSelector::FrontierMidpoints].into_boxed_slice();
        let mut artifact = artifact(
            plan,
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("generic-next")).unwrap()),
        );
        let reason = ProbeSchedulingReason::Selector {
            selector_index: 0,
            selector: ProbeSelector::FrontierMidpoints,
            detail: "canonical-frontier-midpoint".into(),
        };
        artifact.observations[0].boundary_values = ProbeBoundaryValues::default();
        artifact.observations[0].scheduling_reason = reason.clone();
        artifact.transcript[0].scheduling_reason = reason;
        artifact.validate().unwrap();

        artifact.observations[0].boundary_values.lower = Some(ProbeBoundaryEndpoint {
            value: ExploreValue::Int(100),
            state: ProbeEndpointState::Evaluated,
        });
        assert!(artifact.validate().is_err());
    }

    #[test]
    fn lifted_candidates_are_unevaluated_and_inherit_no_evidence() {
        let mut artifact = artifact(
            contract("same", 4),
            ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("lift-queue")).unwrap()),
        );
        artifact.lifted_candidates = vec![ProbeLiftedCandidate {
            origin_case_id: ExploreCaseId::new(vec![0, 1]),
            candidate_case_id: ExploreCaseId::new(vec![1, 1]),
            fixed_boundary_value: ExploreValue::Int(100),
        }]
        .into_boxed_slice();
        artifact.counts = ProbeCounts {
            planned_distinct_cases: 2,
            observed_distinct_cases: 1,
            pending_distinct_cases: 1,
            remaining_case_budget: 3,
        };
        artifact.validate().unwrap();

        let candidate = artifact.lifted_candidates[0].candidate_case_id.clone();
        artifact.observations =
            vec![artifact.observations[0].clone(), observation(candidate)].into_boxed_slice();
        assert!(artifact.validate_lifted_candidates().is_err());
    }
}
