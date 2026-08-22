//! Strict canonical JSON codec for exported/imported probe transcripts.
//!
//! The wire schema is deliberately separate from the evidence core. Decoding
//! succeeds only when the document denotes valid core evidence *and* already
//! equals the unique compact encoding of that evidence, including one final
//! newline. This module owns neither filesystem policy nor public Explore
//! result JSON or the primary unified run journal.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU128;

use serde::{Deserialize, Serialize};

use super::probe::{
    ProbeArtifact, ProbeArtifactState, ProbeBoundaryContract, ProbeBoundaryEndpoint,
    ProbeBoundaryValues, ProbeClassification, ProbeClassificationKind, ProbeCompletionReason,
    ProbeCounts, ProbeCursor, ProbeDecision, ProbeDimensionDescriptor, ProbeDimensionValue,
    ProbeEndpointState, ProbeFrontierId, ProbeFrontierState, ProbeLiftedCandidate,
    ProbeMechanismSignatureRef, ProbeNamedValue, ProbeObservation, ProbePartialReason,
    ProbePlanContract, ProbeRetainedOutputs, ProbeSchedulingReason, ProbeSelector,
    ProbeSemanticIdentity,
};
use super::report::ExploreCaseId;
use super::{ExploreGeneratorAxisRole, ExplorePolarity, ExploreValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeCodecError(String);

impl fmt::Display for ProbeCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ProbeCodecError {}

fn invalid(message: impl Into<String>) -> ProbeCodecError {
    ProbeCodecError(message.into())
}

/// Encodes one validated checkpoint as compact canonical JSON plus `\n`.
pub(crate) fn encode_probe_artifact_v2(
    artifact: &ProbeArtifact,
) -> Result<Vec<u8>, ProbeCodecError> {
    artifact
        .validate()
        .map_err(|error| invalid(format!("invalid probe artifact: {error}")))?;
    let dto = ProbeArtifactV2Dto::from_core(artifact)?;
    let mut bytes = serde_json::to_vec(&dto)
        .map_err(|error| invalid(format!("cannot encode probe artifact: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Decodes only the unique canonical v2 byte representation.
pub(crate) fn decode_probe_artifact_v2(bytes: &[u8]) -> Result<ProbeArtifact, ProbeCodecError> {
    let dto = serde_json::from_slice::<ProbeArtifactV2Dto>(bytes)
        .map_err(|error| invalid(format!("cannot decode probe artifact: {error}")))?;
    let artifact = dto.into_core()?;
    artifact
        .validate()
        .map_err(|error| invalid(format!("invalid probe artifact: {error}")))?;
    let canonical = encode_probe_artifact_v2(&artifact)?;
    if canonical.as_slice() != bytes {
        return Err(invalid(
            "probe artifact bytes are not the canonical compact v2 encoding",
        ));
    }
    Ok(artifact)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeArtifactV2Dto {
    contract: ProbePlanContractV2Dto,
    state: ProbeArtifactStateV1Dto,
    cursor: ProbeCursorV1Dto,
    counts: ProbeCountsV1Dto,
    observations: Vec<ProbeObservationV1Dto>,
    transcript: Vec<ProbeDecisionV1Dto>,
    lifted_candidates: Vec<ProbeLiftedCandidateV1Dto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbePlanContractV2Dto {
    artifact_schema: String,
    normalization_version: String,
    selector_tie_break_version: String,
    query_name: String,
    identity: ProbeSemanticIdentityV1Dto,
    polarity: ExplorePolarityV1Dto,
    dimensions: Vec<ProbeDimensionDescriptorV2Dto>,
    axis_cardinalities: Vec<String>,
    boundary: Option<ProbeBoundaryContractV1Dto>,
    selectors: Vec<ProbeSelectorV1Dto>,
    semantic_case_cap: String,
    initial_frontier: String,
    lift_dimension_indices: Vec<String>,
    retained_configuration_dimension_indices: Vec<String>,
    retained_key_names: Vec<String>,
    retained_shown_names: Vec<String>,
    mechanism_trace_authorized: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeDimensionDescriptorV2Dto {
    bound_index: String,
    role: ExploreGeneratorAxisRoleV2Dto,
    role_field_index: String,
    label: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ExploreGeneratorAxisRoleV2Dto {
    Context,
    Before,
    AfterIndependent,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeSemanticIdentityV1Dto {
    program_hash: String,
    analysis_program_hash: String,
    query_hash: String,
    domain_hash: String,
    probe_plan_hash: String,
    evaluator_contract_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ExplorePolarityV1Dto {
    Violations,
    Matches,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeBoundaryContractV1Dto {
    axis: String,
    step: String,
    requires_both_endpoints_in_domain: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ProbeSelectorV1Dto {
    BoundaryCandidates,
    BoundaryEndpoints,
    FrontierMidpoints,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeArtifactStateV1Dto {
    Partial { reason: ProbePartialReasonV1Dto },
    Complete { reason: ProbeCompletionReasonV1Dto },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ProbePartialReasonV1Dto {
    Interrupted,
    Timeout,
    ArtifactSizeLimit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ProbeCompletionReasonV1Dto {
    BudgetReached,
    PlanExhausted,
    NoMoreUniqueProbes,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeCursorV1Dto {
    next_decision: String,
    frontier: ProbeFrontierStateV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeFrontierStateV1Dto {
    Open { id: String },
    PlanExhausted,
    NoMoreUniqueProbes,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeCountsV1Dto {
    planned_distinct_cases: String,
    observed_distinct_cases: String,
    pending_distinct_cases: String,
    remaining_case_budget: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeObservationV1Dto {
    case_id: Vec<String>,
    configuration: Vec<ProbeDimensionValueV2Dto>,
    boundary_values: ProbeBoundaryValuesV1Dto,
    classification: ProbeClassificationV1Dto,
    outputs: ProbeRetainedOutputsV1Dto,
    scheduling_reason: ProbeSchedulingReasonV1Dto,
    mechanism_signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeDimensionValueV2Dto {
    dimension_index: String,
    value: ExploreValueV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeNamedValueV1Dto {
    name: String,
    value: ExploreValueV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeBoundaryValuesV1Dto {
    lower: Option<ProbeBoundaryEndpointV1Dto>,
    upper: Option<ProbeBoundaryEndpointV1Dto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeBoundaryEndpointV1Dto {
    value: ExploreValueV1Dto,
    state: ProbeEndpointStateV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ProbeEndpointStateV1Dto {
    Ineligible,
    EligibleUnevaluated,
    Evaluated,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeClassificationV1Dto {
    Excluded { reason: String },
    Nonmatch { question_value: bool },
    Match { question_value: bool },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeRetainedOutputsV1Dto {
    Unavailable,
    Available {
        key: Vec<ProbeNamedValueV1Dto>,
        shown: Vec<ProbeNamedValueV1Dto>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeSchedulingReasonV1Dto {
    Selector {
        selector_index: String,
        selector: ProbeSelectorV1Dto,
        detail: String,
    },
    Lifted {
        origin_case_id: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeDecisionV1Dto {
    sequence: String,
    observed_before: String,
    observed_after: String,
    frontier_before: String,
    selected_case_id: Vec<String>,
    scheduling_reason: ProbeSchedulingReasonV1Dto,
    classification: ProbeClassificationKindV1Dto,
    frontier_after: ProbeFrontierStateV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ProbeClassificationKindV1Dto {
    Excluded,
    Nonmatch,
    Match,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeLiftedCandidateV1Dto {
    origin_case_id: Vec<String>,
    candidate_case_id: Vec<String>,
    fixed_boundary_value: ExploreValueV1Dto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ExploreValueV1Dto {
    Int {
        value: String,
    },
    FloatBits {
        bits: String,
    },
    String {
        value: String,
    },
    Character {
        value: char,
    },
    Boolean {
        value: bool,
    },
    Unit,
    List {
        values: Vec<ExploreValueV1Dto>,
    },
    Set {
        values: Vec<ExploreValueV1Dto>,
    },
    Tuple {
        values: Vec<ExploreValueV1Dto>,
    },
    Constructor {
        type_name: String,
        variant: String,
        positional: bool,
        fields: Vec<ExploreConstructorFieldV1Dto>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExploreConstructorFieldV1Dto {
    name: String,
    value: ExploreValueV1Dto,
}

impl ProbeArtifactV2Dto {
    fn from_core(artifact: &ProbeArtifact) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            contract: ProbePlanContractV2Dto::from_core(&artifact.contract),
            state: ProbeArtifactStateV1Dto::from_core(artifact.state),
            cursor: ProbeCursorV1Dto::from_core(&artifact.cursor),
            counts: ProbeCountsV1Dto::from_core(artifact.counts),
            observations: artifact
                .observations
                .iter()
                .map(ProbeObservationV1Dto::from_core)
                .collect::<Result<_, _>>()?,
            transcript: artifact
                .transcript
                .iter()
                .map(ProbeDecisionV1Dto::from_core)
                .collect::<Result<_, _>>()?,
            lifted_candidates: artifact
                .lifted_candidates
                .iter()
                .map(ProbeLiftedCandidateV1Dto::from_core)
                .collect::<Result<_, _>>()?,
        })
    }

    fn into_core(self) -> Result<ProbeArtifact, ProbeCodecError> {
        Ok(ProbeArtifact {
            contract: self.contract.into_core()?,
            state: self.state.into_core(),
            cursor: self.cursor.into_core()?,
            counts: self.counts.into_core()?,
            observations: self
                .observations
                .into_iter()
                .map(ProbeObservationV1Dto::into_core)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            transcript: self
                .transcript
                .into_iter()
                .map(ProbeDecisionV1Dto::into_core)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            lifted_candidates: self
                .lifted_candidates
                .into_iter()
                .map(ProbeLiftedCandidateV1Dto::into_core)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        })
    }
}

impl ProbePlanContractV2Dto {
    fn from_core(contract: &ProbePlanContract) -> Self {
        Self {
            artifact_schema: contract.artifact_schema.to_string(),
            normalization_version: contract.normalization_version.to_string(),
            selector_tie_break_version: contract.selector_tie_break_version.to_string(),
            query_name: contract.query_name.to_string(),
            identity: ProbeSemanticIdentityV1Dto::from_core(&contract.identity),
            polarity: ExplorePolarityV1Dto::from_core(contract.polarity),
            dimensions: contract
                .dimensions
                .iter()
                .map(ProbeDimensionDescriptorV2Dto::from_core)
                .collect(),
            axis_cardinalities: contract
                .axis_cardinalities
                .iter()
                .map(|value| value.to_string())
                .collect(),
            boundary: contract.boundary.map(ProbeBoundaryContractV1Dto::from_core),
            selectors: contract
                .selectors
                .iter()
                .copied()
                .map(ProbeSelectorV1Dto::from_core)
                .collect(),
            semantic_case_cap: contract.semantic_case_cap.get().to_string(),
            initial_frontier: contract.initial_frontier.as_str().to_string(),
            lift_dimension_indices: contract
                .lift_dimension_indices
                .iter()
                .map(|index| (*index as u128).to_string())
                .collect(),
            retained_configuration_dimension_indices: contract
                .retained_configuration_dimension_indices
                .iter()
                .map(|index| (*index as u128).to_string())
                .collect(),
            retained_key_names: contract.retained_key_names.to_vec(),
            retained_shown_names: contract.retained_shown_names.to_vec(),
            mechanism_trace_authorized: contract.mechanism_trace_authorized,
        }
    }

    fn into_core(self) -> Result<ProbePlanContract, ProbeCodecError> {
        let semantic_case_cap = parse_u128("semantic_case_cap", &self.semantic_case_cap)?;
        let semantic_case_cap = NonZeroU128::new(semantic_case_cap)
            .ok_or_else(|| invalid("probe semantic_case_cap must be greater than zero"))?;
        Ok(ProbePlanContract {
            artifact_schema: self.artifact_schema.into_boxed_str(),
            normalization_version: self.normalization_version.into_boxed_str(),
            selector_tie_break_version: self.selector_tie_break_version.into_boxed_str(),
            query_name: self.query_name.into_boxed_str(),
            identity: self.identity.into_core(),
            polarity: self.polarity.into_core(),
            dimensions: self
                .dimensions
                .into_iter()
                .map(ProbeDimensionDescriptorV2Dto::into_core)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            axis_cardinalities: parse_u128_list("axis_cardinalities", self.axis_cardinalities)?
                .into_boxed_slice(),
            boundary: self
                .boundary
                .map(ProbeBoundaryContractV1Dto::into_core)
                .transpose()?,
            selectors: self
                .selectors
                .into_iter()
                .map(ProbeSelectorV1Dto::into_core)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            semantic_case_cap,
            initial_frontier: ProbeFrontierId::new(self.initial_frontier.into_boxed_str())
                .map_err(|error| invalid(format!("invalid initial frontier: {error}")))?,
            lift_dimension_indices: parse_usize_list(
                "lift_dimension_indices",
                self.lift_dimension_indices,
            )?
            .into_boxed_slice(),
            retained_configuration_dimension_indices: parse_usize_list(
                "retained_configuration_dimension_indices",
                self.retained_configuration_dimension_indices,
            )?
            .into_boxed_slice(),
            retained_key_names: self.retained_key_names.into_boxed_slice(),
            retained_shown_names: self.retained_shown_names.into_boxed_slice(),
            mechanism_trace_authorized: self.mechanism_trace_authorized,
        })
    }
}

impl ProbeDimensionDescriptorV2Dto {
    fn from_core(dimension: &ProbeDimensionDescriptor) -> Self {
        Self {
            bound_index: (dimension.bound_index as u128).to_string(),
            role: ExploreGeneratorAxisRoleV2Dto::from_core(dimension.role),
            role_field_index: (dimension.role_field_index as u128).to_string(),
            label: dimension.label.clone(),
        }
    }

    fn into_core(self) -> Result<ProbeDimensionDescriptor, ProbeCodecError> {
        Ok(ProbeDimensionDescriptor {
            bound_index: parse_usize("dimensions[].bound_index", &self.bound_index)?,
            role: self.role.into_core(),
            role_field_index: parse_usize("dimensions[].role_field_index", &self.role_field_index)?,
            label: self.label,
        })
    }
}

impl ExploreGeneratorAxisRoleV2Dto {
    fn from_core(role: ExploreGeneratorAxisRole) -> Self {
        match role {
            ExploreGeneratorAxisRole::Context => Self::Context,
            ExploreGeneratorAxisRole::Before => Self::Before,
            ExploreGeneratorAxisRole::AfterIndependent => Self::AfterIndependent,
        }
    }

    fn into_core(self) -> ExploreGeneratorAxisRole {
        match self {
            Self::Context => ExploreGeneratorAxisRole::Context,
            Self::Before => ExploreGeneratorAxisRole::Before,
            Self::AfterIndependent => ExploreGeneratorAxisRole::AfterIndependent,
        }
    }
}

impl ProbeSemanticIdentityV1Dto {
    fn from_core(identity: &ProbeSemanticIdentity) -> Self {
        Self {
            program_hash: identity.program_hash.to_string(),
            analysis_program_hash: identity.analysis_program_hash.to_string(),
            query_hash: identity.query_hash.to_string(),
            domain_hash: identity.domain_hash.to_string(),
            probe_plan_hash: identity.probe_plan_hash.to_string(),
            evaluator_contract_hash: identity.evaluator_contract_hash.to_string(),
        }
    }

    fn into_core(self) -> ProbeSemanticIdentity {
        ProbeSemanticIdentity {
            program_hash: self.program_hash.into_boxed_str(),
            analysis_program_hash: self.analysis_program_hash.into_boxed_str(),
            query_hash: self.query_hash.into_boxed_str(),
            domain_hash: self.domain_hash.into_boxed_str(),
            probe_plan_hash: self.probe_plan_hash.into_boxed_str(),
            evaluator_contract_hash: self.evaluator_contract_hash.into_boxed_str(),
        }
    }
}

impl ExplorePolarityV1Dto {
    fn from_core(polarity: ExplorePolarity) -> Self {
        match polarity {
            ExplorePolarity::Violations => Self::Violations,
            ExplorePolarity::Matches => Self::Matches,
        }
    }

    fn into_core(self) -> ExplorePolarity {
        match self {
            Self::Violations => ExplorePolarity::Violations,
            Self::Matches => ExplorePolarity::Matches,
        }
    }
}

impl ProbeBoundaryContractV1Dto {
    fn from_core(boundary: ProbeBoundaryContract) -> Self {
        Self {
            axis: (boundary.axis as u128).to_string(),
            step: boundary.step.to_string(),
            requires_both_endpoints_in_domain: boundary.requires_both_endpoints_in_domain,
        }
    }

    fn into_core(self) -> Result<ProbeBoundaryContract, ProbeCodecError> {
        Ok(ProbeBoundaryContract {
            axis: parse_usize("boundary.axis", &self.axis)?,
            step: parse_i64("boundary.step", &self.step)?,
            requires_both_endpoints_in_domain: self.requires_both_endpoints_in_domain,
        })
    }
}

impl ProbeSelectorV1Dto {
    fn from_core(selector: ProbeSelector) -> Self {
        match selector {
            ProbeSelector::BoundaryCandidates => Self::BoundaryCandidates,
            ProbeSelector::BoundaryEndpoints => Self::BoundaryEndpoints,
            ProbeSelector::FrontierMidpoints => Self::FrontierMidpoints,
        }
    }

    fn into_core(self) -> ProbeSelector {
        match self {
            Self::BoundaryCandidates => ProbeSelector::BoundaryCandidates,
            Self::BoundaryEndpoints => ProbeSelector::BoundaryEndpoints,
            Self::FrontierMidpoints => ProbeSelector::FrontierMidpoints,
        }
    }
}

impl ProbeArtifactStateV1Dto {
    fn from_core(state: ProbeArtifactState) -> Self {
        match state {
            ProbeArtifactState::Partial { reason } => Self::Partial {
                reason: ProbePartialReasonV1Dto::from_core(reason),
            },
            ProbeArtifactState::Complete { reason } => Self::Complete {
                reason: ProbeCompletionReasonV1Dto::from_core(reason),
            },
        }
    }

    fn into_core(self) -> ProbeArtifactState {
        match self {
            Self::Partial { reason } => ProbeArtifactState::Partial {
                reason: reason.into_core(),
            },
            Self::Complete { reason } => ProbeArtifactState::Complete {
                reason: reason.into_core(),
            },
        }
    }
}

impl ProbePartialReasonV1Dto {
    fn from_core(reason: ProbePartialReason) -> Self {
        match reason {
            ProbePartialReason::Interrupted => Self::Interrupted,
            ProbePartialReason::Timeout => Self::Timeout,
            ProbePartialReason::ArtifactSizeLimit => Self::ArtifactSizeLimit,
        }
    }

    fn into_core(self) -> ProbePartialReason {
        match self {
            Self::Interrupted => ProbePartialReason::Interrupted,
            Self::Timeout => ProbePartialReason::Timeout,
            Self::ArtifactSizeLimit => ProbePartialReason::ArtifactSizeLimit,
        }
    }
}

impl ProbeCompletionReasonV1Dto {
    fn from_core(reason: ProbeCompletionReason) -> Self {
        match reason {
            ProbeCompletionReason::BudgetReached => Self::BudgetReached,
            ProbeCompletionReason::PlanExhausted => Self::PlanExhausted,
            ProbeCompletionReason::NoMoreUniqueProbes => Self::NoMoreUniqueProbes,
        }
    }

    fn into_core(self) -> ProbeCompletionReason {
        match self {
            Self::BudgetReached => ProbeCompletionReason::BudgetReached,
            Self::PlanExhausted => ProbeCompletionReason::PlanExhausted,
            Self::NoMoreUniqueProbes => ProbeCompletionReason::NoMoreUniqueProbes,
        }
    }
}

impl ProbeCursorV1Dto {
    fn from_core(cursor: &ProbeCursor) -> Self {
        Self {
            next_decision: cursor.next_decision.to_string(),
            frontier: ProbeFrontierStateV1Dto::from_core(&cursor.frontier),
        }
    }

    fn into_core(self) -> Result<ProbeCursor, ProbeCodecError> {
        Ok(ProbeCursor {
            next_decision: parse_u128("cursor.next_decision", &self.next_decision)?,
            frontier: self.frontier.into_core()?,
        })
    }
}

impl ProbeFrontierStateV1Dto {
    fn from_core(frontier: &ProbeFrontierState) -> Self {
        match frontier {
            ProbeFrontierState::Open(id) => Self::Open {
                id: id.as_str().to_string(),
            },
            ProbeFrontierState::PlanExhausted => Self::PlanExhausted,
            ProbeFrontierState::NoMoreUniqueProbes => Self::NoMoreUniqueProbes,
        }
    }

    fn into_core(self) -> Result<ProbeFrontierState, ProbeCodecError> {
        Ok(match self {
            Self::Open { id } => ProbeFrontierState::Open(
                ProbeFrontierId::new(id.into_boxed_str())
                    .map_err(|error| invalid(format!("invalid open frontier: {error}")))?,
            ),
            Self::PlanExhausted => ProbeFrontierState::PlanExhausted,
            Self::NoMoreUniqueProbes => ProbeFrontierState::NoMoreUniqueProbes,
        })
    }
}

impl ProbeCountsV1Dto {
    fn from_core(counts: ProbeCounts) -> Self {
        Self {
            planned_distinct_cases: counts.planned_distinct_cases.to_string(),
            observed_distinct_cases: counts.observed_distinct_cases.to_string(),
            pending_distinct_cases: counts.pending_distinct_cases.to_string(),
            remaining_case_budget: counts.remaining_case_budget.to_string(),
        }
    }

    fn into_core(self) -> Result<ProbeCounts, ProbeCodecError> {
        Ok(ProbeCounts {
            planned_distinct_cases: parse_u128(
                "counts.planned_distinct_cases",
                &self.planned_distinct_cases,
            )?,
            observed_distinct_cases: parse_u128(
                "counts.observed_distinct_cases",
                &self.observed_distinct_cases,
            )?,
            pending_distinct_cases: parse_u128(
                "counts.pending_distinct_cases",
                &self.pending_distinct_cases,
            )?,
            remaining_case_budget: parse_u128(
                "counts.remaining_case_budget",
                &self.remaining_case_budget,
            )?,
        })
    }
}

impl ProbeObservationV1Dto {
    fn from_core(observation: &ProbeObservation) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            case_id: case_id_from_core(&observation.case_id),
            configuration: observation
                .configuration
                .iter()
                .map(ProbeDimensionValueV2Dto::from_core)
                .collect::<Result<_, _>>()?,
            boundary_values: ProbeBoundaryValuesV1Dto::from_core(&observation.boundary_values)?,
            classification: ProbeClassificationV1Dto::from_core(&observation.classification),
            outputs: ProbeRetainedOutputsV1Dto::from_core(&observation.outputs)?,
            scheduling_reason: ProbeSchedulingReasonV1Dto::from_core(
                &observation.scheduling_reason,
            ),
            mechanism_signature: observation
                .mechanism_signature
                .as_ref()
                .map(|signature| signature.as_str().to_string()),
        })
    }

    fn into_core(self) -> Result<ProbeObservation, ProbeCodecError> {
        Ok(ProbeObservation {
            case_id: case_id_into_core("observation.case_id", self.case_id)?,
            configuration: self
                .configuration
                .into_iter()
                .map(ProbeDimensionValueV2Dto::into_core)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            boundary_values: self.boundary_values.into_core()?,
            classification: self.classification.into_core(),
            outputs: self.outputs.into_core()?,
            scheduling_reason: self.scheduling_reason.into_core()?,
            mechanism_signature: self
                .mechanism_signature
                .map(|signature| {
                    ProbeMechanismSignatureRef::new(signature.into_boxed_str()).map_err(|error| {
                        invalid(format!("invalid mechanism signature reference: {error}"))
                    })
                })
                .transpose()?,
        })
    }
}

impl ProbeDimensionValueV2Dto {
    fn from_core(value: &ProbeDimensionValue) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            dimension_index: (value.dimension_index as u128).to_string(),
            value: ExploreValueV1Dto::from_core(&value.value)?,
        })
    }

    fn into_core(self) -> Result<ProbeDimensionValue, ProbeCodecError> {
        Ok(ProbeDimensionValue {
            dimension_index: parse_usize(
                "observation.configuration[].dimension_index",
                &self.dimension_index,
            )?,
            value: self.value.into_core()?,
        })
    }
}

impl ProbeNamedValueV1Dto {
    fn from_core(named: &ProbeNamedValue) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            name: named.name.clone(),
            value: ExploreValueV1Dto::from_core(&named.value)?,
        })
    }

    fn into_core(self) -> Result<ProbeNamedValue, ProbeCodecError> {
        Ok(ProbeNamedValue {
            name: self.name,
            value: self.value.into_core()?,
        })
    }
}

impl ProbeBoundaryValuesV1Dto {
    fn from_core(values: &ProbeBoundaryValues) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            lower: values
                .lower
                .as_ref()
                .map(ProbeBoundaryEndpointV1Dto::from_core)
                .transpose()?,
            upper: values
                .upper
                .as_ref()
                .map(ProbeBoundaryEndpointV1Dto::from_core)
                .transpose()?,
        })
    }

    fn into_core(self) -> Result<ProbeBoundaryValues, ProbeCodecError> {
        Ok(ProbeBoundaryValues {
            lower: self
                .lower
                .map(ProbeBoundaryEndpointV1Dto::into_core)
                .transpose()?,
            upper: self
                .upper
                .map(ProbeBoundaryEndpointV1Dto::into_core)
                .transpose()?,
        })
    }
}

impl ProbeBoundaryEndpointV1Dto {
    fn from_core(endpoint: &ProbeBoundaryEndpoint) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            value: ExploreValueV1Dto::from_core(&endpoint.value)?,
            state: ProbeEndpointStateV1Dto::from_core(endpoint.state),
        })
    }

    fn into_core(self) -> Result<ProbeBoundaryEndpoint, ProbeCodecError> {
        Ok(ProbeBoundaryEndpoint {
            value: self.value.into_core()?,
            state: self.state.into_core(),
        })
    }
}

impl ProbeEndpointStateV1Dto {
    fn from_core(state: ProbeEndpointState) -> Self {
        match state {
            ProbeEndpointState::Ineligible => Self::Ineligible,
            ProbeEndpointState::EligibleUnevaluated => Self::EligibleUnevaluated,
            ProbeEndpointState::Evaluated => Self::Evaluated,
        }
    }

    fn into_core(self) -> ProbeEndpointState {
        match self {
            Self::Ineligible => ProbeEndpointState::Ineligible,
            Self::EligibleUnevaluated => ProbeEndpointState::EligibleUnevaluated,
            Self::Evaluated => ProbeEndpointState::Evaluated,
        }
    }
}

impl ProbeClassificationV1Dto {
    fn from_core(classification: &ProbeClassification) -> Self {
        match classification {
            ProbeClassification::Excluded { reason } => Self::Excluded {
                reason: reason.to_string(),
            },
            ProbeClassification::Nonmatch { question_value } => Self::Nonmatch {
                question_value: *question_value,
            },
            ProbeClassification::Match { question_value } => Self::Match {
                question_value: *question_value,
            },
        }
    }

    fn into_core(self) -> ProbeClassification {
        match self {
            Self::Excluded { reason } => ProbeClassification::Excluded {
                reason: reason.into_boxed_str(),
            },
            Self::Nonmatch { question_value } => ProbeClassification::Nonmatch { question_value },
            Self::Match { question_value } => ProbeClassification::Match { question_value },
        }
    }
}

impl ProbeRetainedOutputsV1Dto {
    fn from_core(outputs: &ProbeRetainedOutputs) -> Result<Self, ProbeCodecError> {
        Ok(match outputs {
            ProbeRetainedOutputs::Unavailable => Self::Unavailable,
            ProbeRetainedOutputs::Available { key, shown } => Self::Available {
                key: key
                    .iter()
                    .map(ProbeNamedValueV1Dto::from_core)
                    .collect::<Result<_, _>>()?,
                shown: shown
                    .iter()
                    .map(ProbeNamedValueV1Dto::from_core)
                    .collect::<Result<_, _>>()?,
            },
        })
    }

    fn into_core(self) -> Result<ProbeRetainedOutputs, ProbeCodecError> {
        Ok(match self {
            Self::Unavailable => ProbeRetainedOutputs::Unavailable,
            Self::Available { key, shown } => ProbeRetainedOutputs::Available {
                key: key
                    .into_iter()
                    .map(ProbeNamedValueV1Dto::into_core)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                shown: shown
                    .into_iter()
                    .map(ProbeNamedValueV1Dto::into_core)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
        })
    }
}

impl ProbeSchedulingReasonV1Dto {
    fn from_core(reason: &ProbeSchedulingReason) -> Self {
        match reason {
            ProbeSchedulingReason::Selector {
                selector_index,
                selector,
                detail,
            } => Self::Selector {
                selector_index: (*selector_index as u128).to_string(),
                selector: ProbeSelectorV1Dto::from_core(*selector),
                detail: detail.to_string(),
            },
            ProbeSchedulingReason::Lifted { origin_case_id } => Self::Lifted {
                origin_case_id: case_id_from_core(origin_case_id),
            },
        }
    }

    fn into_core(self) -> Result<ProbeSchedulingReason, ProbeCodecError> {
        Ok(match self {
            Self::Selector {
                selector_index,
                selector,
                detail,
            } => ProbeSchedulingReason::Selector {
                selector_index: parse_usize("scheduling_reason.selector_index", &selector_index)?,
                selector: selector.into_core(),
                detail: detail.into_boxed_str(),
            },
            Self::Lifted { origin_case_id } => ProbeSchedulingReason::Lifted {
                origin_case_id: case_id_into_core(
                    "scheduling_reason.origin_case_id",
                    origin_case_id,
                )?,
            },
        })
    }
}

impl ProbeDecisionV1Dto {
    fn from_core(decision: &ProbeDecision) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            sequence: decision.sequence.to_string(),
            observed_before: decision.observed_before.to_string(),
            observed_after: decision.observed_after.to_string(),
            frontier_before: decision.frontier_before.as_str().to_string(),
            selected_case_id: case_id_from_core(&decision.selected_case_id),
            scheduling_reason: ProbeSchedulingReasonV1Dto::from_core(&decision.scheduling_reason),
            classification: ProbeClassificationKindV1Dto::from_core(decision.classification),
            frontier_after: ProbeFrontierStateV1Dto::from_core(&decision.frontier_after),
        })
    }

    fn into_core(self) -> Result<ProbeDecision, ProbeCodecError> {
        Ok(ProbeDecision {
            sequence: parse_u128("decision.sequence", &self.sequence)?,
            observed_before: parse_u128("decision.observed_before", &self.observed_before)?,
            observed_after: parse_u128("decision.observed_after", &self.observed_after)?,
            frontier_before: ProbeFrontierId::new(self.frontier_before.into_boxed_str())
                .map_err(|error| invalid(format!("invalid decision frontier_before: {error}")))?,
            selected_case_id: case_id_into_core(
                "decision.selected_case_id",
                self.selected_case_id,
            )?,
            scheduling_reason: self.scheduling_reason.into_core()?,
            classification: self.classification.into_core(),
            frontier_after: self.frontier_after.into_core()?,
        })
    }
}

impl ProbeClassificationKindV1Dto {
    fn from_core(classification: ProbeClassificationKind) -> Self {
        match classification {
            ProbeClassificationKind::Excluded => Self::Excluded,
            ProbeClassificationKind::Nonmatch => Self::Nonmatch,
            ProbeClassificationKind::Match => Self::Match,
        }
    }

    fn into_core(self) -> ProbeClassificationKind {
        match self {
            Self::Excluded => ProbeClassificationKind::Excluded,
            Self::Nonmatch => ProbeClassificationKind::Nonmatch,
            Self::Match => ProbeClassificationKind::Match,
        }
    }
}

impl ProbeLiftedCandidateV1Dto {
    fn from_core(candidate: &ProbeLiftedCandidate) -> Result<Self, ProbeCodecError> {
        Ok(Self {
            origin_case_id: case_id_from_core(&candidate.origin_case_id),
            candidate_case_id: case_id_from_core(&candidate.candidate_case_id),
            fixed_boundary_value: ExploreValueV1Dto::from_core(&candidate.fixed_boundary_value)?,
        })
    }

    fn into_core(self) -> Result<ProbeLiftedCandidate, ProbeCodecError> {
        Ok(ProbeLiftedCandidate {
            origin_case_id: case_id_into_core(
                "lifted_candidate.origin_case_id",
                self.origin_case_id,
            )?,
            candidate_case_id: case_id_into_core(
                "lifted_candidate.candidate_case_id",
                self.candidate_case_id,
            )?,
            fixed_boundary_value: self.fixed_boundary_value.into_core()?,
        })
    }
}

impl ExploreValueV1Dto {
    fn from_core(value: &ExploreValue) -> Result<Self, ProbeCodecError> {
        Ok(match value {
            ExploreValue::Int(value) => Self::Int {
                value: value.to_string(),
            },
            ExploreValue::FloatBits(bits) => Self::FloatBits {
                bits: format!("{bits:016x}"),
            },
            ExploreValue::String(value) => Self::String {
                value: value.clone(),
            },
            ExploreValue::Character(value) => Self::Character { value: *value },
            ExploreValue::Boolean(value) => Self::Boolean { value: *value },
            ExploreValue::Unit => Self::Unit,
            ExploreValue::List(values) => Self::List {
                values: values
                    .iter()
                    .map(Self::from_core)
                    .collect::<Result<_, _>>()?,
            },
            ExploreValue::Set(values) => {
                validate_canonical_set(values)?;
                Self::Set {
                    values: values
                        .iter()
                        .map(Self::from_core)
                        .collect::<Result<_, _>>()?,
                }
            }
            ExploreValue::Tuple(values) => Self::Tuple {
                values: values
                    .iter()
                    .map(Self::from_core)
                    .collect::<Result<_, _>>()?,
            },
            ExploreValue::Constructor {
                type_name,
                variant,
                positional,
                fields,
            } => {
                validate_constructor(type_name, variant, fields)?;
                Self::Constructor {
                    type_name: type_name.clone(),
                    variant: variant.clone(),
                    positional: *positional,
                    fields: fields
                        .iter()
                        .map(|(name, value)| {
                            Ok(ExploreConstructorFieldV1Dto {
                                name: name.clone(),
                                value: Self::from_core(value)?,
                            })
                        })
                        .collect::<Result<_, ProbeCodecError>>()?,
                }
            }
        })
    }

    fn into_core(self) -> Result<ExploreValue, ProbeCodecError> {
        Ok(match self {
            Self::Int { value } => ExploreValue::Int(parse_i64("value.int", &value)?),
            Self::FloatBits { bits } => {
                ExploreValue::FloatBits(parse_float_bits("value.float_bits", &bits)?)
            }
            Self::String { value } => ExploreValue::String(value),
            Self::Character { value } => ExploreValue::Character(value),
            Self::Boolean { value } => ExploreValue::Boolean(value),
            Self::Unit => ExploreValue::Unit,
            Self::List { values } => ExploreValue::List(
                values
                    .into_iter()
                    .map(Self::into_core)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Set { values } => {
                let values = values
                    .into_iter()
                    .map(Self::into_core)
                    .collect::<Result<Vec<_>, _>>()?;
                validate_canonical_set(&values)?;
                ExploreValue::Set(values)
            }
            Self::Tuple { values } => ExploreValue::Tuple(
                values
                    .into_iter()
                    .map(Self::into_core)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Constructor {
                type_name,
                variant,
                positional,
                fields,
            } => {
                let fields = fields
                    .into_iter()
                    .map(|field| Ok((field.name, field.value.into_core()?)))
                    .collect::<Result<Vec<_>, ProbeCodecError>>()?;
                validate_constructor(&type_name, &variant, &fields)?;
                ExploreValue::Constructor {
                    type_name,
                    variant,
                    positional,
                    fields,
                }
            }
        })
    }
}

fn case_id_from_core(case_id: &ExploreCaseId) -> Vec<String> {
    case_id
        .ordinals()
        .iter()
        .map(|ordinal| ordinal.to_string())
        .collect()
}

fn case_id_into_core(field: &str, ordinals: Vec<String>) -> Result<ExploreCaseId, ProbeCodecError> {
    Ok(ExploreCaseId::new(parse_u128_list(field, ordinals)?))
}

fn parse_u128_list(field: &str, values: Vec<String>) -> Result<Vec<u128>, ProbeCodecError> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_u128(&format!("{field}[{index}]"), &value))
        .collect()
}

fn parse_usize_list(field: &str, values: Vec<String>) -> Result<Vec<usize>, ProbeCodecError> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_usize(&format!("{field}[{index}]"), &value))
        .collect()
}

fn parse_u128(field: &str, value: &str) -> Result<u128, ProbeCodecError> {
    let bytes = value.as_bytes();
    let canonical = !bytes.is_empty()
        && bytes.iter().all(|byte| byte.is_ascii_digit())
        && (bytes == b"0"
            || bytes
                .first()
                .is_some_and(|byte| (b'1'..=b'9').contains(byte)));
    if !canonical {
        return Err(invalid(format!(
            "probe {field} is not a minimal unsigned decimal string"
        )));
    }
    value
        .parse::<u128>()
        .map_err(|_| invalid(format!("probe {field} exceeds u128::MAX")))
}

fn parse_usize(field: &str, value: &str) -> Result<usize, ProbeCodecError> {
    let value = parse_u128(field, value)?;
    usize::try_from(value)
        .map_err(|_| invalid(format!("probe {field} exceeds the host usize range")))
}

fn parse_i64(field: &str, value: &str) -> Result<i64, ProbeCodecError> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    let negative = digits.len() != value.len();
    let canonical = !digits.is_empty()
        && digits.as_bytes().iter().all(|byte| byte.is_ascii_digit())
        && (digits == "0"
            || digits
                .as_bytes()
                .first()
                .is_some_and(|byte| (b'1'..=b'9').contains(byte)))
        && !(negative && digits == "0");
    if !canonical {
        return Err(invalid(format!(
            "probe {field} is not a minimal signed decimal string"
        )));
    }
    value
        .parse::<i64>()
        .map_err(|_| invalid(format!("probe {field} is outside the Int range")))
}

fn parse_float_bits(field: &str, value: &str) -> Result<u64, ProbeCodecError> {
    if value.len() != 16
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(format!(
            "probe {field} must contain exactly 16 lowercase hexadecimal digits"
        )));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| invalid(format!("probe {field} is not valid FloatBits")))
}

fn validate_canonical_set(values: &[ExploreValue]) -> Result<(), ProbeCodecError> {
    let mut previous: Option<String> = None;
    for (index, value) in values.iter().enumerate() {
        validate_explore_value(value)?;
        let key = value.runtime_display_key();
        if previous.as_ref().is_some_and(|prior| prior >= &key) {
            return Err(invalid(format!(
                "probe set member {index} is duplicate or outside canonical value order"
            )));
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_constructor(
    type_name: &str,
    variant: &str,
    fields: &[(String, ExploreValue)],
) -> Result<(), ProbeCodecError> {
    if type_name.is_empty() || variant.is_empty() {
        return Err(invalid(
            "probe constructor type_name and variant must not be empty",
        ));
    }
    let mut names = BTreeSet::new();
    for (name, value) in fields {
        if name.is_empty() {
            return Err(invalid("probe constructor field name must not be empty"));
        }
        if !names.insert(name.as_str()) {
            return Err(invalid(format!(
                "probe constructor field `{name}` occurs more than once"
            )));
        }
        validate_explore_value(value)?;
    }
    Ok(())
}

fn validate_explore_value(value: &ExploreValue) -> Result<(), ProbeCodecError> {
    match value {
        ExploreValue::List(values) | ExploreValue::Tuple(values) => {
            for value in values {
                validate_explore_value(value)?;
            }
            Ok(())
        }
        ExploreValue::Set(values) => validate_canonical_set(values),
        ExploreValue::Constructor {
            type_name,
            variant,
            fields,
            ..
        } => validate_constructor(type_name, variant, fields),
        ExploreValue::Int(_)
        | ExploreValue::FloatBits(_)
        | ExploreValue::String(_)
        | ExploreValue::Character(_)
        | ExploreValue::Boolean(_)
        | ExploreValue::Unit => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::probe::PROBE_ARTIFACT_SCHEMA_V2;
    use super::*;

    fn digest(byte: &str) -> Box<str> {
        byte.repeat(32).into_boxed_str()
    }

    fn named(name: &str, value: ExploreValue) -> ProbeNamedValue {
        ProbeNamedValue {
            name: name.to_string(),
            value,
        }
    }

    fn dimension_value(dimension_index: usize, value: ExploreValue) -> ProbeDimensionValue {
        ProbeDimensionValue {
            dimension_index,
            value,
        }
    }

    fn fixture() -> ProbeArtifact {
        let initial_frontier = ProbeFrontierId::new(digest("11")).expect("frontier");
        let case_id = ExploreCaseId::new(vec![u128::MAX - 1]);
        let scheduling_reason = ProbeSchedulingReason::Selector {
            selector_index: 0,
            selector: ProbeSelector::FrontierMidpoints,
            detail: "largest-open-region".into(),
        };
        let configuration_value = ExploreValue::Tuple(vec![
            ExploreValue::Int(-7),
            ExploreValue::String("-7".to_string()),
            ExploreValue::List(vec![ExploreValue::Unit]),
            // Futuruna set values are ordered by their runtime display key:
            // lexical "10" precedes lexical "2".
            ExploreValue::Set(vec![ExploreValue::Int(10), ExploreValue::Int(2)]),
            ExploreValue::Tuple(vec![ExploreValue::Boolean(false)]),
            ExploreValue::Constructor {
                type_name: "Pair".to_string(),
                variant: "Pair".to_string(),
                positional: true,
                fields: vec![
                    ("_0".to_string(), ExploreValue::Character('ø')),
                    ("_1".to_string(), ExploreValue::Boolean(true)),
                ],
            },
        ]);
        let observation = ProbeObservation {
            case_id: case_id.clone(),
            configuration: vec![dimension_value(0, configuration_value)].into_boxed_slice(),
            boundary_values: ProbeBoundaryValues::default(),
            classification: ProbeClassification::Match {
                question_value: true,
            },
            outputs: ProbeRetainedOutputs::Available {
                key: vec![named("key", ExploreValue::String("typed".to_string()))]
                    .into_boxed_slice(),
                shown: vec![named(
                    "shown",
                    ExploreValue::FloatBits(0x0123_4567_89ab_cdef),
                )]
                .into_boxed_slice(),
            },
            scheduling_reason: scheduling_reason.clone(),
            mechanism_signature: Some(
                ProbeMechanismSignatureRef::new("mechanism-v1").expect("mechanism reference"),
            ),
        };
        let decision = ProbeDecision {
            sequence: 0,
            observed_before: 0,
            observed_after: 1,
            frontier_before: initial_frontier.clone(),
            selected_case_id: case_id,
            scheduling_reason,
            classification: ProbeClassificationKind::Match,
            frontier_after: ProbeFrontierState::PlanExhausted,
        };
        ProbeArtifact {
            contract: ProbePlanContract {
                artifact_schema: PROBE_ARTIFACT_SCHEMA_V2.into(),
                normalization_version: "normalization-v2".into(),
                selector_tie_break_version: "tie-break-v1".into(),
                query_name: "typed_probe".into(),
                identity: ProbeSemanticIdentity {
                    program_hash: digest("01"),
                    analysis_program_hash: digest("02"),
                    query_hash: digest("03"),
                    domain_hash: digest("04"),
                    probe_plan_hash: digest("05"),
                    evaluator_contract_hash: digest("06"),
                },
                polarity: ExplorePolarity::Matches,
                dimensions: vec![ProbeDimensionDescriptor {
                    bound_index: 0,
                    role: ExploreGeneratorAxisRole::Before,
                    role_field_index: 0,
                    label: "value".to_string(),
                }]
                .into_boxed_slice(),
                axis_cardinalities: vec![u128::MAX].into_boxed_slice(),
                boundary: None,
                selectors: vec![ProbeSelector::FrontierMidpoints].into_boxed_slice(),
                semantic_case_cap: NonZeroU128::new(1).expect("nonzero cap"),
                initial_frontier,
                lift_dimension_indices: Vec::new().into_boxed_slice(),
                retained_configuration_dimension_indices: vec![0].into_boxed_slice(),
                retained_key_names: vec!["key".to_string()].into_boxed_slice(),
                retained_shown_names: vec!["shown".to_string()].into_boxed_slice(),
                mechanism_trace_authorized: true,
            },
            state: ProbeArtifactState::Complete {
                reason: ProbeCompletionReason::PlanExhausted,
            },
            cursor: ProbeCursor {
                next_decision: 1,
                frontier: ProbeFrontierState::PlanExhausted,
            },
            counts: ProbeCounts {
                planned_distinct_cases: 1,
                observed_distinct_cases: 1,
                pending_distinct_cases: 0,
                remaining_case_budget: 0,
            },
            observations: vec![observation].into_boxed_slice(),
            transcript: vec![decision].into_boxed_slice(),
            lifted_candidates: Vec::new().into_boxed_slice(),
        }
    }

    #[test]
    fn canonical_v2_roundtrip_preserves_typed_values() {
        let artifact = fixture();
        let bytes = encode_probe_artifact_v2(&artifact).expect("encode");
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(decode_probe_artifact_v2(&bytes).expect("decode"), artifact);

        let text = std::str::from_utf8(&bytes).expect("utf8");
        assert!(text.contains(&format!("\"{}\"", u128::MAX)));
        assert!(text.contains("\"bits\":\"0123456789abcdef\""));
        for tag in [
            "\"kind\":\"int\"",
            "\"kind\":\"float_bits\"",
            "\"kind\":\"string\"",
            "\"kind\":\"character\"",
            "\"kind\":\"boolean\"",
            "\"kind\":\"unit\"",
            "\"kind\":\"list\"",
            "\"kind\":\"set\"",
            "\"kind\":\"tuple\"",
            "\"kind\":\"constructor\"",
        ] {
            assert!(text.contains(tag), "missing explicit value tag {tag}");
        }
    }

    #[test]
    fn decoder_rejects_alternate_bytes_and_scalar_spellings() {
        let bytes = encode_probe_artifact_v2(&fixture()).expect("encode");

        let mut missing_newline = bytes.clone();
        missing_newline.pop();
        assert!(decode_probe_artifact_v2(&missing_newline).is_err());

        let mut spaced = bytes.clone();
        spaced.insert(1, b' ');
        assert!(decode_probe_artifact_v2(&spaced).is_err());

        let text = std::str::from_utf8(&bytes).expect("utf8");
        let nonminimal_u128 = text.replacen(
            &format!("\"{}\"", u128::MAX),
            &format!("\"0{}\"", u128::MAX),
            1,
        );
        assert!(decode_probe_artifact_v2(nonminimal_u128.as_bytes()).is_err());

        let nonminimal_i64 = text.replacen("\"value\":\"-7\"", "\"value\":\"-07\"", 1);
        assert!(decode_probe_artifact_v2(nonminimal_i64.as_bytes()).is_err());

        let uppercase_float = text.replacen("0123456789abcdef", "0123456789ABCDEF", 1);
        assert!(decode_probe_artifact_v2(uppercase_float.as_bytes()).is_err());

        let unknown = text.replacen('{', "{\"unknown\":true,", 1);
        assert!(decode_probe_artifact_v2(unknown.as_bytes()).is_err());
    }

    #[test]
    fn encoder_rejects_noncanonical_sets_and_duplicate_constructor_fields() {
        let mut noncanonical_set = fixture();
        noncanonical_set.observations[0].configuration[0].value =
            ExploreValue::Set(vec![ExploreValue::Int(2), ExploreValue::Int(10)]);
        assert!(encode_probe_artifact_v2(&noncanonical_set).is_err());

        let mut duplicate_fields = fixture();
        duplicate_fields.observations[0].configuration[0].value = ExploreValue::Constructor {
            type_name: "Pair".to_string(),
            variant: "Pair".to_string(),
            positional: false,
            fields: vec![
                ("left".to_string(), ExploreValue::Int(1)),
                ("left".to_string(), ExploreValue::Int(2)),
            ],
        };
        assert!(encode_probe_artifact_v2(&duplicate_fields).is_err());
    }
}
