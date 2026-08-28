//! Deterministic scheduling and transcript reconstruction for Explore probes.
//!
//! This module owns no evaluation, persistence, CLI policy, or Explore closure.
//! It turns one checked finite domain into a replayable stream of `CaseId`s.
//! Source-event candidates intentionally stop as unsupported until the checked
//! [`super::source_events::ResolvedBoundaryFragment`] adapter exists; treating
//! their absence as exhaustion would silently erase a declared selector.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::probe::{
    ProbeArtifact, ProbeArtifactState, ProbeClassification, ProbeClassificationKind,
    ProbeCompletionReason, ProbeCounts, ProbeDecision, ProbeFrontierId, ProbeFrontierState,
    ProbeLiftedCandidate, ProbePlanContract, ProbeSchedulingReason, ProbeSelector,
};
use super::report::ExploreCaseId;
use super::{ExploreCardinality, ExploreExactDomain, ExploreQueryIr, ExploreValue};

/// Frozen ordering implemented by this scheduler revision.
///
/// Selectors run in source order. Newly created lift work drains before the
/// current selector resumes, ordered by `(origin CaseId, candidate CaseId)`.
/// Endpoint and midpoint ties use canonical `CaseId` order.
pub(crate) const PROBE_SCHEDULER_TIE_BREAK_V1: &str =
    "selectors-source-order;lifts-origin-then-candidate;frontier-largest-then-case-id.v1";

const PROBE_FRONTIER_HASH_DOMAIN_V2: &str = "futuruna.explore.probe-frontier.v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeSchedulerUnsupported {
    BoundaryCandidatesNeedResolvedFragmentAdapter,
    FrontierMidpointsNeedBoundaryAxis,
}

impl fmt::Display for ProbeSchedulerUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundaryCandidatesNeedResolvedFragmentAdapter => formatter.write_str(
                "boundary_candidates requires a checked ResolvedBoundaryFragment adapter",
            ),
            Self::FrontierMidpointsNeedBoundaryAxis => formatter
                .write_str("frontier_midpoints requires a checked boundary axis in scheduler v1"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeSchedulerError(String);

impl fmt::Display for ProbeSchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ProbeSchedulerError {}

fn invalid(message: impl Into<String>) -> ProbeSchedulerError {
    ProbeSchedulerError(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeScheduleOutcome {
    Scheduled(ProbeScheduledCase),
    Complete(ProbeCompletionReason),
    Unsupported(ProbeSchedulerUnsupported),
}

/// One exact scheduler choice. The hidden successor is recomputed before it is
/// committed, so callers cannot manufacture a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeScheduledCase {
    pub(crate) case_id: ExploreCaseId,
    pub(crate) scheduling_reason: ProbeSchedulingReason,
    pub(crate) frontier_before: ProbeFrontierId,
    state_after_selection: SchedulerState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingLift {
    origin_case_id: ExploreCaseId,
    candidate_case_id: ExploreCaseId,
    fixed_boundary_value: ExploreValue,
}

impl PendingLift {
    fn as_artifact_candidate(&self) -> ProbeLiftedCandidate {
        ProbeLiftedCandidate {
            origin_case_id: self.origin_case_id.clone(),
            candidate_case_id: self.candidate_case_id.clone(),
            fixed_boundary_value: self.fixed_boundary_value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SchedulerState {
    selector_index: usize,
    endpoint_cursor: u128,
    observed: BTreeSet<ExploreCaseId>,
    /// One canonical winning origin per unevaluated candidate.
    pending_lifts: BTreeMap<ExploreCaseId, PendingLift>,
}

impl SchedulerState {
    fn initial() -> Self {
        Self {
            selector_index: 0,
            endpoint_cursor: 0,
            observed: BTreeSet::new(),
            pending_lifts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryPoint {
    value: i64,
    ordinal: u128,
}

/// A normalized support of eligible lower boundary values. Dense ranges stay
/// symbolic; explicit sparse axes retain only their already-materialized
/// eligible members.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BoundarySupport {
    Dense {
        first_value: i64,
        first_ordinal: u128,
        cardinality: u128,
    },
    Sparse(Box<[BoundaryPoint]>),
}

impl BoundarySupport {
    fn cardinality(&self) -> u128 {
        match self {
            Self::Dense { cardinality, .. } => *cardinality,
            Self::Sparse(points) => points.len() as u128,
        }
    }

    fn point_at(&self, position: u128) -> Result<BoundaryPoint, ProbeSchedulerError> {
        if position >= self.cardinality() {
            return Err(invalid(format!(
                "boundary support position {position} is outside cardinality {}",
                self.cardinality()
            )));
        }
        match self {
            Self::Dense {
                first_value,
                first_ordinal,
                ..
            } => {
                let value = i128::from(*first_value)
                    .checked_add(
                        i128::try_from(position)
                            .map_err(|_| invalid("boundary support position exceeds i128"))?,
                    )
                    .ok_or_else(|| invalid("dense boundary support value overflow"))?;
                let ordinal = first_ordinal
                    .checked_add(position)
                    .ok_or_else(|| invalid("dense boundary support ordinal overflow"))?;
                Ok(BoundaryPoint {
                    value: i64::try_from(value)
                        .map_err(|_| invalid("dense boundary support value exceeds Int"))?,
                    ordinal,
                })
            }
            Self::Sparse(points) => {
                let index = usize::try_from(position)
                    .map_err(|_| invalid("sparse boundary support position exceeds usize"))?;
                points
                    .get(index)
                    .cloned()
                    .ok_or_else(|| invalid("sparse boundary support position is absent"))
            }
        }
    }

    fn position_of_ordinal(&self, ordinal: u128) -> Option<u128> {
        match self {
            Self::Dense {
                first_ordinal,
                cardinality,
                ..
            } => ordinal
                .checked_sub(*first_ordinal)
                .filter(|position| position < cardinality),
            Self::Sparse(points) => points
                .iter()
                .position(|point| point.ordinal == ordinal)
                .map(|position| position as u128),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundarySchedulingDomain {
    axis: usize,
    step: i64,
    supports: Box<[BoundarySupport]>,
    endpoint_ordinals: Box<[u128]>,
    eligible_lower_count: u128,
}

impl BoundarySchedulingDomain {
    fn from_exact(
        axis: usize,
        step: i64,
        domain: &ExploreExactDomain,
    ) -> Result<Self, ProbeSchedulerError> {
        if step <= 0 {
            return Err(invalid("probe boundary step must be positive"));
        }
        let supports = match domain {
            ExploreExactDomain::IntRange {
                start, cardinality, ..
            } => {
                let step = u64::try_from(step)
                    .map_err(|_| invalid("probe boundary step does not fit u64"))?;
                let eligible = cardinality.saturating_sub(step) as u128;
                if eligible == 0 {
                    Vec::new()
                } else {
                    vec![BoundarySupport::Dense {
                        first_value: *start,
                        first_ordinal: 0,
                        cardinality: eligible,
                    }]
                }
            }
            ExploreExactDomain::Enumerated { values, .. } => {
                // Sparse supports are maximal runs of consecutive numeric
                // lower values. Source-list order remains CaseId order, but it
                // cannot manufacture continuity across an undeclared Int.
                let mut by_value = BTreeMap::<i64, u128>::new();
                for (ordinal, value) in values.iter().enumerate() {
                    let value = value.int().ok_or_else(|| {
                        invalid(format!("probe boundary member {ordinal} is not an Int"))
                    })?;
                    let ordinal = ordinal as u128;
                    if by_value.insert(value, ordinal).is_some() {
                        return Err(invalid(format!(
                            "probe boundary domain repeats Int value {value}"
                        )));
                    }
                }

                let mut normalized = Vec::<Vec<BoundaryPoint>>::new();
                let mut current = Vec::<BoundaryPoint>::new();
                let mut previous_value = None::<i64>;
                for (&value, &ordinal) in &by_value {
                    let Some(upper) = value.checked_add(step) else {
                        continue;
                    };
                    if !by_value.contains_key(&upper) {
                        continue;
                    }
                    let adjacent = previous_value
                        .and_then(|previous| previous.checked_add(1))
                        .is_some_and(|previous_successor| previous_successor == value);
                    if !adjacent && !current.is_empty() {
                        normalized.push(std::mem::take(&mut current));
                    }
                    current.push(BoundaryPoint { value, ordinal });
                    previous_value = Some(value);
                }
                if !current.is_empty() {
                    normalized.push(current);
                }
                normalized
                    .into_iter()
                    .map(|points| BoundarySupport::Sparse(points.into_boxed_slice()))
                    .collect()
            }
            ExploreExactDomain::FiniteType { .. } => {
                return Err(invalid(
                    "probe boundary axis must be an explicit Int list or symbolic Int range",
                ))
            }
        };

        let eligible_lower_count = supports.iter().try_fold(0_u128, |total, support| {
            total
                .checked_add(support.cardinality())
                .ok_or_else(|| invalid("eligible probe boundary count exceeds u128::MAX"))
        })?;
        let mut endpoint_ordinals = BTreeSet::new();
        for support in &supports {
            let cardinality = support.cardinality();
            if cardinality == 0 {
                continue;
            }
            endpoint_ordinals.insert(support.point_at(0)?.ordinal);
            endpoint_ordinals.insert(support.point_at(cardinality - 1)?.ordinal);
        }

        Ok(Self {
            axis,
            step,
            supports: supports.into_boxed_slice(),
            endpoint_ordinals: endpoint_ordinals.into_iter().collect::<Vec<_>>().into(),
            eligible_lower_count,
        })
    }

    fn locate_ordinal(&self, ordinal: u128) -> Option<(usize, u128)> {
        self.supports
            .iter()
            .enumerate()
            .find_map(|(support, values)| {
                values
                    .position_of_ordinal(ordinal)
                    .map(|position| (support, position))
            })
    }

    fn value_for_ordinal(&self, ordinal: u128) -> Result<i64, ProbeSchedulerError> {
        let (support, position) = self.locate_ordinal(ordinal).ok_or_else(|| {
            invalid(format!(
                "boundary ordinal {ordinal} is not an eligible lower endpoint"
            ))
        })?;
        Ok(self.supports[support].point_at(position)?.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeSchedulingDomain {
    axis_cardinalities: Box<[u128]>,
    boundary: Option<BoundarySchedulingDomain>,
    outer_cardinalities: Box<[u128]>,
    outer_profile_count: u128,
    endpoint_case_count: u128,
    lift_dimensions: Box<[usize]>,
}

impl ProbeSchedulingDomain {
    fn from_query(
        contract: &ProbePlanContract,
        query: &ExploreQueryIr,
    ) -> Result<Self, ProbeSchedulerError> {
        contract
            .validate()
            .map_err(|error| invalid(format!("invalid probe contract: {error}")))?;
        if contract.selector_tie_break_version.as_ref() != PROBE_SCHEDULER_TIE_BREAK_V1 {
            return Err(invalid(format!(
                "unsupported probe selector/tie-break version `{}`",
                contract.selector_tie_break_version
            )));
        }
        if contract.polarity != query.query.polarity {
            return Err(invalid(
                "probe contract polarity disagrees with the checked Explore query",
            ));
        }
        if contract.dimensions.len() != query.universe.dimensions.len() {
            return Err(invalid(format!(
                "probe contract has {} dimensions but the checked query has {}",
                contract.dimensions.len(),
                query.universe.dimensions.len()
            )));
        }

        let mut axis_cardinalities = Vec::with_capacity(query.universe.dimensions.len());
        for (axis, (contract_dimension, dimension)) in contract
            .dimensions
            .iter()
            .zip(&query.universe.dimensions)
            .enumerate()
        {
            if contract_dimension.bound_index != dimension.bound_index
                || contract_dimension.role != dimension.role
                || contract_dimension.role_field_index != dimension.role_field_index
                || contract_dimension.label != dimension.name
            {
                return Err(invalid(format!(
                    "probe dimension {axis} descriptor {:?}/field {}/bound {} `{}` disagrees with checked {:?}/field {}/bound {} `{}`",
                    contract_dimension.role,
                    contract_dimension.role_field_index,
                    contract_dimension.bound_index,
                    contract_dimension.label,
                    dimension.role,
                    dimension.role_field_index,
                    dimension.bound_index,
                    dimension.name
                )));
            }
            let cardinality = exact_cardinality(
                dimension.domain.cardinality(),
                &format!(
                    "probe {:?} field {} dimension `{}`",
                    contract_dimension.role,
                    contract_dimension.role_field_index,
                    contract_dimension.label
                ),
            )?;
            if contract.axis_cardinalities[axis] != cardinality {
                return Err(invalid(format!(
                    "probe dimension {axis} `{}` has contract cardinality {}, checked cardinality {cardinality}",
                    contract_dimension.label,
                    contract.axis_cardinalities[axis]
                )));
            }
            axis_cardinalities.push(cardinality);
        }

        let boundary = match (contract.boundary, query.boundary_hint()) {
            (None, None) => None,
            (Some(contract_boundary), Some(query_boundary)) => {
                if contract_boundary.axis != query_boundary.axis_dimension_index
                    || contract_boundary.step != query_boundary.step
                    || !query_boundary.requires_both_endpoints_in_domain
                {
                    return Err(invalid(
                        "probe boundary contract disagrees with the checked Explore boundary",
                    ));
                }
                let dimension = query
                    .universe
                    .dimensions
                    .get(contract_boundary.axis)
                    .ok_or_else(|| invalid("probe boundary axis is outside checked dimensions"))?;
                Some(BoundarySchedulingDomain::from_exact(
                    contract_boundary.axis,
                    contract_boundary.step,
                    &dimension.domain,
                )?)
            }
            _ => {
                return Err(invalid(
                    "probe boundary presence disagrees with the checked Explore query",
                ))
            }
        };

        let boundary_axis = boundary.as_ref().map(|boundary| boundary.axis);
        let outer_cardinalities = axis_cardinalities
            .iter()
            .enumerate()
            .filter_map(|(axis, cardinality)| (Some(axis) != boundary_axis).then_some(*cardinality))
            .collect::<Vec<_>>();
        let outer_profile_count = checked_product(&outer_cardinalities)?;
        let endpoint_axis_count = boundary
            .as_ref()
            .map_or(0, |boundary| boundary.endpoint_ordinals.len() as u128);
        let endpoint_case_count = outer_profile_count
            .checked_mul(endpoint_axis_count)
            .ok_or_else(|| invalid("probe endpoint case count exceeds u128::MAX"))?;

        let lift_dimensions = contract.lift_dimension_indices.to_vec();

        Ok(Self {
            axis_cardinalities: axis_cardinalities.into_boxed_slice(),
            boundary,
            outer_cardinalities: outer_cardinalities.into_boxed_slice(),
            outer_profile_count,
            endpoint_case_count,
            lift_dimensions: lift_dimensions.into_boxed_slice(),
        })
    }

    fn validate_contract(&self, contract: &ProbePlanContract) -> Result<(), ProbeSchedulerError> {
        if self.axis_cardinalities.as_ref() != contract.axis_cardinalities.as_ref() {
            return Err(invalid(
                "probe scheduling domain cardinalities disagree with the contract",
            ));
        }
        if self.lift_dimensions.as_ref() != contract.lift_dimension_indices.as_ref() {
            return Err(invalid(
                "probe scheduling lift dimensions disagree with the contract",
            ));
        }
        match (&self.boundary, contract.boundary) {
            (None, None) => {}
            (Some(domain), Some(boundary))
                if domain.axis == boundary.axis && domain.step == boundary.step => {}
            _ => {
                return Err(invalid(
                    "probe scheduling boundary disagrees with the contract",
                ))
            }
        }
        Ok(())
    }

    fn endpoint_case_at(&self, rank: u128) -> Result<ExploreCaseId, ProbeSchedulerError> {
        let boundary = self
            .boundary
            .as_ref()
            .ok_or_else(|| invalid("boundary endpoint selector has no boundary domain"))?;
        if rank >= self.endpoint_case_count {
            return Err(invalid(format!(
                "probe endpoint rank {rank} is outside {} cases",
                self.endpoint_case_count
            )));
        }
        let mut filtered_cardinalities = self.axis_cardinalities.to_vec();
        filtered_cardinalities[boundary.axis] = boundary.endpoint_ordinals.len() as u128;
        let mut ordinals = unrank_product(&filtered_cardinalities, rank)?;
        let endpoint_index = usize::try_from(ordinals[boundary.axis])
            .map_err(|_| invalid("probe endpoint ordinal index exceeds usize"))?;
        ordinals[boundary.axis] = *boundary
            .endpoint_ordinals
            .get(endpoint_index)
            .ok_or_else(|| invalid("probe endpoint ordinal index is absent"))?;
        Ok(ExploreCaseId::new(ordinals))
    }

    fn outer_rank(&self, case_id: &ExploreCaseId) -> Result<u128, ProbeSchedulerError> {
        self.validate_case_id(case_id)?;
        let boundary_axis = self.boundary.as_ref().map(|boundary| boundary.axis);
        let mut rank = 0_u128;
        for (axis, (&ordinal, &cardinality)) in case_id
            .ordinals()
            .iter()
            .zip(self.axis_cardinalities.iter())
            .enumerate()
        {
            if Some(axis) == boundary_axis {
                continue;
            }
            rank = rank
                .checked_mul(cardinality)
                .and_then(|rank| rank.checked_add(ordinal))
                .ok_or_else(|| invalid("probe outer-profile rank exceeds u128::MAX"))?;
        }
        Ok(rank)
    }

    fn case_for_outer(
        &self,
        outer_rank: u128,
        boundary_ordinal: u128,
    ) -> Result<ExploreCaseId, ProbeSchedulerError> {
        let boundary = self
            .boundary
            .as_ref()
            .ok_or_else(|| invalid("probe outer profile has no boundary axis"))?;
        if outer_rank >= self.outer_profile_count {
            return Err(invalid(format!(
                "probe outer-profile rank {outer_rank} is outside {} profiles",
                self.outer_profile_count
            )));
        }
        let outer = unrank_product(&self.outer_cardinalities, outer_rank)?;
        let mut outer = outer.into_iter();
        let mut full = Vec::with_capacity(self.axis_cardinalities.len());
        for axis in 0..self.axis_cardinalities.len() {
            if axis == boundary.axis {
                full.push(boundary_ordinal);
            } else {
                full.push(
                    outer
                        .next()
                        .ok_or_else(|| invalid("probe outer profile ended unexpectedly"))?,
                );
            }
        }
        if outer.next().is_some() {
            return Err(invalid("probe outer profile retained extra ordinals"));
        }
        let case_id = ExploreCaseId::new(full);
        self.validate_case_id(&case_id)?;
        Ok(case_id)
    }

    fn validate_case_id(&self, case_id: &ExploreCaseId) -> Result<(), ProbeSchedulerError> {
        if case_id.len() != self.axis_cardinalities.len() {
            return Err(invalid(format!(
                "probe CaseId has {} axes, expected {}",
                case_id.len(),
                self.axis_cardinalities.len()
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
                    "probe CaseId ordinal {ordinal} is outside axis {axis} cardinality {cardinality}"
                )));
            }
        }
        Ok(())
    }

    fn midpoint_candidate(
        &self,
        observed: &BTreeSet<ExploreCaseId>,
    ) -> Result<Option<MidpointCandidate>, ProbeSchedulerError> {
        let boundary = match &self.boundary {
            Some(boundary) => boundary,
            None => return Ok(None),
        };
        if boundary.eligible_lower_count == 0 || self.outer_profile_count == 0 {
            return Ok(None);
        }

        let mut selected_by_profile = BTreeMap::<u128, BTreeMap<usize, BTreeSet<u128>>>::new();
        for case_id in observed {
            let boundary_ordinal = case_id.ordinals()[boundary.axis];
            let Some((support, position)) = boundary.locate_ordinal(boundary_ordinal) else {
                continue;
            };
            selected_by_profile
                .entry(self.outer_rank(case_id)?)
                .or_default()
                .entry(support)
                .or_default()
                .insert(position);
        }

        let mut best = None::<MidpointCandidate>;
        for (&outer_rank, selected) in &selected_by_profile {
            if let Some(candidate) = self.midpoint_for_profile(outer_rank, selected)? {
                select_better_midpoint(&mut best, candidate);
            }
        }

        let mut untouched_rank = 0_u128;
        while untouched_rank < self.outer_profile_count
            && selected_by_profile.contains_key(&untouched_rank)
        {
            untouched_rank += 1;
        }
        if untouched_rank < self.outer_profile_count {
            if let Some(candidate) = self.midpoint_for_profile(untouched_rank, &BTreeMap::new())? {
                select_better_midpoint(&mut best, candidate);
            }
        }
        Ok(best)
    }

    fn midpoint_for_profile(
        &self,
        outer_rank: u128,
        selected: &BTreeMap<usize, BTreeSet<u128>>,
    ) -> Result<Option<MidpointCandidate>, ProbeSchedulerError> {
        let boundary = self
            .boundary
            .as_ref()
            .ok_or_else(|| invalid("probe midpoint profile has no boundary"))?;
        let mut best = None::<MidpointCandidate>;
        for (support_index, support) in boundary.supports.iter().enumerate() {
            let positions = selected.get(&support_index);
            for (start, end_exclusive) in
                unselected_gaps(support.cardinality(), positions.into_iter().flatten())
            {
                let width = end_exclusive - start;
                let midpoint_position = start + width / 2;
                let point = support.point_at(midpoint_position)?;
                let candidate = MidpointCandidate {
                    case_id: self.case_for_outer(outer_rank, point.ordinal)?,
                    support_index,
                    gap_start: start,
                    gap_end_exclusive: end_exclusive,
                    midpoint_position,
                    width,
                };
                select_better_midpoint(&mut best, candidate);
            }
        }
        Ok(best)
    }

    fn midpoint_universe_nonempty(&self) -> bool {
        self.boundary.as_ref().is_some_and(|boundary| {
            boundary.eligible_lower_count != 0 && self.outer_profile_count != 0
        })
    }

    fn lift_candidates(
        &self,
        origin: &ExploreCaseId,
        observed: &BTreeSet<ExploreCaseId>,
        pending: &mut BTreeMap<ExploreCaseId, PendingLift>,
        pending_distinct_limit: u128,
    ) -> Result<(), ProbeSchedulerError> {
        if self.lift_dimensions.is_empty() || pending_distinct_limit == 0 {
            return Ok(());
        }
        self.validate_case_id(origin)?;
        let boundary = self
            .boundary
            .as_ref()
            .ok_or_else(|| invalid("probe lift has no boundary axis"))?;
        let boundary_ordinal = origin.ordinals()[boundary.axis];
        let fixed_boundary_value = ExploreValue::Int(boundary.value_for_ordinal(boundary_ordinal)?);
        let varied_cardinalities = self
            .lift_dimensions
            .iter()
            .map(|axis| self.axis_cardinalities[*axis])
            .collect::<Vec<_>>();
        let candidate_count = checked_product(&varied_cardinalities)?;

        for rank in 0..candidate_count {
            let varied = unrank_product(&varied_cardinalities, rank)?;
            let mut ordinals = origin.ordinals().to_vec();
            for (&axis, ordinal) in self.lift_dimensions.iter().zip(varied) {
                ordinals[axis] = ordinal;
            }
            let candidate = ExploreCaseId::new(ordinals);
            if &candidate == origin || observed.contains(&candidate) {
                continue;
            }

            if let Some(existing) = pending.get_mut(&candidate) {
                if origin < &existing.origin_case_id {
                    existing.origin_case_id = origin.clone();
                    existing.fixed_boundary_value = fixed_boundary_value.clone();
                }
                continue;
            }
            if pending.len() as u128 >= pending_distinct_limit {
                break;
            }
            pending.insert(
                candidate.clone(),
                PendingLift {
                    origin_case_id: origin.clone(),
                    candidate_case_id: candidate,
                    fixed_boundary_value: fixed_boundary_value.clone(),
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MidpointCandidate {
    case_id: ExploreCaseId,
    support_index: usize,
    gap_start: u128,
    gap_end_exclusive: u128,
    midpoint_position: u128,
    width: u128,
}

fn select_better_midpoint(best: &mut Option<MidpointCandidate>, candidate: MidpointCandidate) {
    let replace = best.as_ref().map_or(true, |current| {
        candidate.width > current.width
            || (candidate.width == current.width && candidate.case_id < current.case_id)
    });
    if replace {
        *best = Some(candidate);
    }
}

fn unselected_gaps<'a>(
    cardinality: u128,
    selected: impl Iterator<Item = &'a u128>,
) -> Vec<(u128, u128)> {
    let mut gaps = Vec::new();
    let mut start = 0_u128;
    for &position in selected {
        if position >= cardinality {
            continue;
        }
        if start < position {
            gaps.push((start, position));
        }
        start = position + 1;
    }
    if start < cardinality {
        gaps.push((start, cardinality));
    }
    gaps
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeScheduler {
    contract: ProbePlanContract,
    domain: ProbeSchedulingDomain,
    state: SchedulerState,
}

impl ProbeScheduler {
    pub(crate) fn new(
        contract: &ProbePlanContract,
        query: &ExploreQueryIr,
    ) -> Result<Self, ProbeSchedulerError> {
        let domain = ProbeSchedulingDomain::from_query(contract, query)?;
        Self::from_domain(contract, domain)
    }

    fn from_domain(
        contract: &ProbePlanContract,
        domain: ProbeSchedulingDomain,
    ) -> Result<Self, ProbeSchedulerError> {
        contract
            .validate()
            .map_err(|error| invalid(format!("invalid probe contract: {error}")))?;
        if contract.selector_tie_break_version.as_ref() != PROBE_SCHEDULER_TIE_BREAK_V1 {
            return Err(invalid(format!(
                "unsupported probe selector/tie-break version `{}`",
                contract.selector_tie_break_version
            )));
        }
        domain.validate_contract(contract)?;
        let scheduler = Self {
            contract: contract.clone(),
            domain,
            state: SchedulerState::initial(),
        };
        let expected = scheduler.current_frontier_id()?;
        if expected != contract.initial_frontier {
            return Err(invalid(format!(
                "probe initial frontier {} disagrees with derived scheduler frontier {}",
                contract.initial_frontier.as_str(),
                expected.as_str()
            )));
        }
        Ok(scheduler)
    }

    pub(crate) fn next(&self) -> Result<ProbeScheduleOutcome, ProbeSchedulerError> {
        if self.observed_count() >= self.contract.semantic_case_cap.get() {
            return Ok(ProbeScheduleOutcome::Complete(
                ProbeCompletionReason::BudgetReached,
            ));
        }
        match self.select_next_internal()? {
            InternalNext::Scheduled {
                case_id,
                scheduling_reason,
                state_after_selection,
            } => Ok(ProbeScheduleOutcome::Scheduled(ProbeScheduledCase {
                case_id,
                scheduling_reason,
                frontier_before: self.current_frontier_id()?,
                state_after_selection,
            })),
            InternalNext::Complete(reason) => Ok(ProbeScheduleOutcome::Complete(reason)),
            InternalNext::Unsupported(reason) => Ok(ProbeScheduleOutcome::Unsupported(reason)),
        }
    }

    /// Commit only the classification kind needed by scheduling. Full values
    /// remain the evaluator/artifact layer's responsibility.
    pub(crate) fn record_classification(
        &mut self,
        scheduled: &ProbeScheduledCase,
        classification: ProbeClassificationKind,
    ) -> Result<ProbeDecision, ProbeSchedulerError> {
        let expected = match self.next()? {
            ProbeScheduleOutcome::Scheduled(expected) => expected,
            ProbeScheduleOutcome::Complete(reason) => {
                return Err(invalid(format!(
                    "cannot record a probe after completion {reason:?}"
                )))
            }
            ProbeScheduleOutcome::Unsupported(reason) => {
                return Err(invalid(format!(
                    "cannot record a probe while scheduling is unsupported: {reason}"
                )))
            }
        };
        if &expected != scheduled {
            return Err(invalid(
                "probe classification does not commit the deterministic next scheduler choice",
            ));
        }

        let observed_before = self.observed_count();
        let mut next = expected.state_after_selection.clone();
        if !next.observed.insert(expected.case_id.clone()) {
            return Err(invalid(
                "probe scheduler attempted to observe one CaseId twice",
            ));
        }
        if classification == ProbeClassificationKind::Match {
            let observed_after = observed_before
                .checked_add(1)
                .ok_or_else(|| invalid("probe observation count exceeds u128::MAX"))?;
            let remaining = self
                .contract
                .semantic_case_cap
                .get()
                .checked_sub(observed_after)
                .ok_or_else(|| invalid("probe observation exceeded its semantic cap"))?;
            self.domain.lift_candidates(
                &expected.case_id,
                &next.observed,
                &mut next.pending_lifts,
                remaining,
            )?;
        }
        self.state = next;
        let observed_after = self.observed_count();
        let frontier_after = self.frontier_state()?;
        Ok(ProbeDecision {
            sequence: observed_before,
            observed_before,
            observed_after,
            frontier_before: expected.frontier_before,
            selected_case_id: expected.case_id,
            scheduling_reason: expected.scheduling_reason,
            classification,
            frontier_after,
        })
    }

    pub(crate) fn frontier_state(&self) -> Result<ProbeFrontierState, ProbeSchedulerError> {
        if self.observed_count() >= self.contract.semantic_case_cap.get() {
            return Ok(ProbeFrontierState::Open(self.current_frontier_id()?));
        }
        match self.select_next_internal()? {
            InternalNext::Complete(ProbeCompletionReason::PlanExhausted) => {
                Ok(ProbeFrontierState::PlanExhausted)
            }
            InternalNext::Complete(ProbeCompletionReason::NoMoreUniqueProbes) => {
                Ok(ProbeFrontierState::NoMoreUniqueProbes)
            }
            InternalNext::Complete(ProbeCompletionReason::BudgetReached) => {
                Ok(ProbeFrontierState::Open(self.current_frontier_id()?))
            }
            InternalNext::Scheduled { .. } | InternalNext::Unsupported(_) => {
                Ok(ProbeFrontierState::Open(self.current_frontier_id()?))
            }
        }
    }

    pub(crate) fn counts(&self) -> Result<ProbeCounts, ProbeSchedulerError> {
        let observed = self.observed_count();
        let pending = self.state.pending_lifts.len() as u128;
        Ok(ProbeCounts {
            planned_distinct_cases: observed
                .checked_add(pending)
                .ok_or_else(|| invalid("probe planned count exceeds u128::MAX"))?,
            observed_distinct_cases: observed,
            pending_distinct_cases: pending,
            remaining_case_budget: self
                .contract
                .semantic_case_cap
                .get()
                .checked_sub(observed)
                .ok_or_else(|| invalid("probe observed count exceeds semantic cap"))?,
        })
    }

    pub(crate) fn pending_lifted_candidates(&self) -> Box<[ProbeLiftedCandidate]> {
        let mut pending = self
            .state
            .pending_lifts
            .values()
            .cloned()
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| {
            (&left.origin_case_id, &left.candidate_case_id)
                .cmp(&(&right.origin_case_id, &right.candidate_case_id))
        });
        pending
            .iter()
            .map(PendingLift::as_artifact_candidate)
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn observed_count(&self) -> u128 {
        self.state.observed.len() as u128
    }

    fn select_next_internal(&self) -> Result<InternalNext, ProbeSchedulerError> {
        let mut next = self.state.clone();
        let mut skipped_duplicate = false;

        loop {
            while let Some(candidate) = next_pending_lift(&next.pending_lifts) {
                next.pending_lifts.remove(&candidate.candidate_case_id);
                if next.observed.contains(&candidate.candidate_case_id) {
                    skipped_duplicate = true;
                    continue;
                }
                return Ok(InternalNext::Scheduled {
                    case_id: candidate.candidate_case_id,
                    scheduling_reason: ProbeSchedulingReason::Lifted {
                        origin_case_id: candidate.origin_case_id,
                    },
                    state_after_selection: next,
                });
            }

            let Some(&selector) = self.contract.selectors.get(next.selector_index) else {
                return Ok(InternalNext::Complete(if skipped_duplicate {
                    ProbeCompletionReason::NoMoreUniqueProbes
                } else {
                    ProbeCompletionReason::PlanExhausted
                }));
            };
            match selector {
                ProbeSelector::BoundaryCandidates => {
                    return Ok(InternalNext::Unsupported(
                        ProbeSchedulerUnsupported::BoundaryCandidatesNeedResolvedFragmentAdapter,
                    ))
                }
                ProbeSelector::BoundaryEndpoints => {
                    while next.endpoint_cursor < self.domain.endpoint_case_count {
                        let rank = next.endpoint_cursor;
                        next.endpoint_cursor += 1;
                        let case_id = self.domain.endpoint_case_at(rank)?;
                        if next.observed.contains(&case_id) {
                            skipped_duplicate = true;
                            continue;
                        }
                        return Ok(InternalNext::Scheduled {
                            case_id,
                            scheduling_reason: ProbeSchedulingReason::Selector {
                                selector_index: next.selector_index,
                                selector,
                                detail: format!("boundary-endpoint/raw-rank={rank}")
                                    .into_boxed_str(),
                            },
                            state_after_selection: next,
                        });
                    }
                    next.selector_index += 1;
                    next.endpoint_cursor = 0;
                }
                ProbeSelector::FrontierMidpoints => {
                    if self.domain.boundary.is_none() {
                        return Ok(InternalNext::Unsupported(
                            ProbeSchedulerUnsupported::FrontierMidpointsNeedBoundaryAxis,
                        ));
                    }
                    if let Some(candidate) = self.domain.midpoint_candidate(&next.observed)? {
                        return Ok(InternalNext::Scheduled {
                            case_id: candidate.case_id,
                            scheduling_reason: ProbeSchedulingReason::Selector {
                                selector_index: next.selector_index,
                                selector,
                                detail: format!(
                                    "frontier-midpoint/support={};gap={}..{};position={}",
                                    candidate.support_index,
                                    candidate.gap_start,
                                    candidate.gap_end_exclusive,
                                    candidate.midpoint_position
                                )
                                .into_boxed_str(),
                            },
                            state_after_selection: next,
                        });
                    }
                    skipped_duplicate |= self.domain.midpoint_universe_nonempty();
                    next.selector_index += 1;
                    next.endpoint_cursor = 0;
                }
            }
        }
    }

    fn current_frontier_id(&self) -> Result<ProbeFrontierId, ProbeSchedulerError> {
        hash_frontier(&self.contract, &self.domain, &self.state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InternalNext {
    Scheduled {
        case_id: ExploreCaseId,
        scheduling_reason: ProbeSchedulingReason,
        state_after_selection: SchedulerState,
    },
    Complete(ProbeCompletionReason),
    Unsupported(ProbeSchedulerUnsupported),
}

fn next_pending_lift(pending: &BTreeMap<ExploreCaseId, PendingLift>) -> Option<PendingLift> {
    pending
        .values()
        .min_by(|left, right| {
            (&left.origin_case_id, &left.candidate_case_id)
                .cmp(&(&right.origin_case_id, &right.candidate_case_id))
        })
        .cloned()
}

/// Derive the only valid initial frontier for a checked plan. The contract's
/// stored `initial_frontier` is deliberately excluded to avoid a self-hash.
pub(crate) fn canonical_initial_probe_frontier(
    contract: &ProbePlanContract,
    query: &ExploreQueryIr,
) -> Result<ProbeFrontierId, ProbeSchedulerError> {
    let domain = ProbeSchedulingDomain::from_query(contract, query)?;
    domain.validate_contract(contract)?;
    hash_frontier(contract, &domain, &SchedulerState::initial())
}

/// Reconstruct all scheduling state from the adaptive transcript. Stored
/// frontier links and pending lift edges are compared to the reconstruction;
/// they are never used as scheduler input.
pub(crate) fn reconstruct_probe_scheduler(
    current_contract: &ProbePlanContract,
    query: &ExploreQueryIr,
    artifact: &ProbeArtifact,
) -> Result<ProbeScheduler, ProbeSchedulerError> {
    let domain = ProbeSchedulingDomain::from_query(current_contract, query)?;
    replay_probe_artifact_with_domain(current_contract, domain, artifact)
}

fn replay_probe_artifact_with_domain(
    current_contract: &ProbePlanContract,
    domain: ProbeSchedulingDomain,
    artifact: &ProbeArtifact,
) -> Result<ProbeScheduler, ProbeSchedulerError> {
    if &artifact.contract != current_contract {
        return Err(invalid(
            "probe artifact contract is stale for the current checked plan",
        ));
    }
    let mut scheduler = ProbeScheduler::from_domain(current_contract, domain)?;
    let mut observations = BTreeMap::new();
    for observation in artifact.observations.iter() {
        if observations
            .insert(observation.case_id.clone(), observation)
            .is_some()
        {
            return Err(invalid("probe artifact repeats an observation CaseId"));
        }
    }
    if observations.len() != artifact.transcript.len() {
        return Err(invalid(
            "probe artifact transcript and observation populations differ",
        ));
    }

    for (index, stored) in artifact.transcript.iter().enumerate() {
        let expected_sequence = index as u128;
        if stored.sequence != expected_sequence
            || stored.observed_before != expected_sequence
            || stored.observed_after != expected_sequence + 1
        {
            return Err(invalid(format!(
                "probe transcript decision {index} has noncanonical sequence counts"
            )));
        }
        let scheduled = match scheduler.next()? {
            ProbeScheduleOutcome::Scheduled(scheduled) => scheduled,
            ProbeScheduleOutcome::Complete(reason) => {
                return Err(invalid(format!(
                    "probe transcript continues after reconstructed completion {reason:?}"
                )))
            }
            ProbeScheduleOutcome::Unsupported(reason) => {
                return Err(invalid(format!(
                    "probe transcript crosses an unsupported scheduler selector: {reason}"
                )))
            }
        };
        if stored.frontier_before != scheduled.frontier_before
            || stored.selected_case_id != scheduled.case_id
            || stored.scheduling_reason != scheduled.scheduling_reason
        {
            return Err(invalid(format!(
                "probe transcript decision {index} diverges from deterministic reconstruction"
            )));
        }
        let observation = observations.get(&stored.selected_case_id).ok_or_else(|| {
            invalid(format!(
                "probe transcript decision {index} has no matching observation"
            ))
        })?;
        let classification = classification_kind(&observation.classification);
        if stored.classification != classification
            || stored.scheduling_reason != observation.scheduling_reason
        {
            return Err(invalid(format!(
                "probe transcript decision {index} disagrees with its observation"
            )));
        }
        let reconstructed = scheduler.record_classification(&scheduled, classification)?;
        if &reconstructed != stored {
            return Err(invalid(format!(
                "probe transcript decision {index} stores a noncanonical frontier transition"
            )));
        }
    }

    if scheduler.state.observed != observations.keys().cloned().collect() {
        return Err(invalid(
            "probe observations differ from the reconstructed selected CaseIds",
        ));
    }
    if artifact.cursor.next_decision != scheduler.observed_count() {
        return Err(invalid(
            "probe cursor decision index differs from reconstructed observations",
        ));
    }
    let frontier = scheduler.frontier_state()?;
    if artifact.cursor.frontier != frontier {
        return Err(invalid(
            "probe cursor frontier differs from deterministic reconstruction",
        ));
    }
    if artifact.counts != scheduler.counts()? {
        return Err(invalid(
            "probe counts differ from deterministic scheduler state",
        ));
    }
    if artifact.lifted_candidates.as_ref() != scheduler.pending_lifted_candidates().as_ref() {
        return Err(invalid(
            "probe pending lift edges differ from deterministic reconstruction",
        ));
    }

    match artifact.state {
        ProbeArtifactState::Partial { .. } => {
            if matches!(scheduler.next()?, ProbeScheduleOutcome::Complete(_)) {
                return Err(invalid(
                    "partial probe artifact has reached a semantic completion condition",
                ));
            }
        }
        ProbeArtifactState::Complete { reason } => match scheduler.next()? {
            ProbeScheduleOutcome::Complete(actual) if actual == reason => {}
            ProbeScheduleOutcome::Complete(actual) => {
                return Err(invalid(format!(
                    "probe completion reason {reason:?} disagrees with reconstructed {actual:?}"
                )))
            }
            ProbeScheduleOutcome::Scheduled(_) | ProbeScheduleOutcome::Unsupported(_) => {
                return Err(invalid(
                    "complete probe artifact still has a scheduling obligation",
                ))
            }
        },
    }

    artifact
        .validate()
        .map_err(|error| invalid(format!("invalid reconstructed probe artifact: {error}")))?;
    Ok(scheduler)
}

fn classification_kind(classification: &ProbeClassification) -> ProbeClassificationKind {
    match classification {
        ProbeClassification::Excluded { .. } => ProbeClassificationKind::Excluded,
        ProbeClassification::Nonmatch { .. } => ProbeClassificationKind::Nonmatch,
        ProbeClassification::Match { .. } => ProbeClassificationKind::Match,
    }
}

fn exact_cardinality(
    cardinality: ExploreCardinality,
    context: &str,
) -> Result<u128, ProbeSchedulerError> {
    cardinality
        .exact()
        .ok_or_else(|| invalid(format!("{context} exceeds u128::MAX")))
}

fn checked_product(cardinalities: &[u128]) -> Result<u128, ProbeSchedulerError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities.iter().try_fold(1_u128, |product, value| {
        product
            .checked_mul(*value)
            .ok_or_else(|| invalid("probe Cartesian product exceeds u128::MAX"))
    })
}

fn unrank_product(cardinalities: &[u128], rank: u128) -> Result<Vec<u128>, ProbeSchedulerError> {
    let total = checked_product(cardinalities)?;
    if rank >= total {
        return Err(invalid(format!(
            "probe product rank {rank} is outside cardinality {total}"
        )));
    }
    if cardinalities.is_empty() {
        return Ok(Vec::new());
    }
    let mut suffix = vec![1_u128; cardinalities.len()];
    for index in (0..cardinalities.len().saturating_sub(1)).rev() {
        suffix[index] = suffix[index + 1]
            .checked_mul(cardinalities[index + 1])
            .ok_or_else(|| invalid("probe Cartesian stride exceeds u128::MAX"))?;
    }
    let mut remainder = rank;
    let mut ordinals = Vec::with_capacity(cardinalities.len());
    for stride in suffix {
        ordinals.push(remainder / stride);
        remainder %= stride;
    }
    Ok(ordinals)
}

struct CanonicalHasher(Sha256);

impl CanonicalHasher {
    fn new() -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.field(PROBE_FRONTIER_HASH_DOMAIN_V2.as_bytes());
        hasher
    }

    /// Every canonical component is length-prefixed, including fixed-width
    /// integers, so nested values cannot collide through concatenation.
    fn field(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u128).to_be_bytes());
        self.0.update(bytes);
    }

    fn str(&mut self, value: &str) {
        self.field(value.as_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.field(&(value as u128).to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.field(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.field(&value.to_be_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.field(&[u8::from(value)]);
    }

    fn case_id(&mut self, case_id: &ExploreCaseId) {
        self.u128(case_id.len() as u128);
        for ordinal in case_id.ordinals() {
            self.u128(*ordinal);
        }
    }

    fn finish(self) -> Result<ProbeFrontierId, ProbeSchedulerError> {
        ProbeFrontierId::new(format!("{:x}", self.0.finalize()).into_boxed_str())
            .map_err(|error| invalid(format!("invalid derived probe frontier: {error}")))
    }
}

fn hash_frontier(
    contract: &ProbePlanContract,
    domain: &ProbeSchedulingDomain,
    state: &SchedulerState,
) -> Result<ProbeFrontierId, ProbeSchedulerError> {
    let mut hash = CanonicalHasher::new();
    hash.str(&contract.artifact_schema);
    hash.str(&contract.normalization_version);
    hash.str(&contract.selector_tie_break_version);
    hash.str(PROBE_SCHEDULER_TIE_BREAK_V1);
    hash.str(&contract.query_name);
    for identity in [
        &contract.identity.program_hash,
        &contract.identity.analysis_program_hash,
        &contract.identity.query_hash,
        &contract.identity.domain_hash,
        &contract.identity.probe_plan_hash,
        &contract.identity.evaluator_contract_hash,
    ] {
        hash.str(identity);
    }
    hash.str(match contract.polarity {
        super::ExplorePolarity::Violations => "violations",
        super::ExplorePolarity::Matches => "matches",
    });
    hash.u128(contract.semantic_case_cap.get());
    hash.u128(contract.selectors.len() as u128);
    for selector in contract.selectors.iter() {
        hash.str(match selector {
            ProbeSelector::BoundaryCandidates => "boundary_candidates",
            ProbeSelector::BoundaryEndpoints => "boundary_endpoints",
            ProbeSelector::FrontierMidpoints => "frontier_midpoints",
        });
    }
    hash.u128(contract.dimensions.len() as u128);
    for (dimension, cardinality) in contract
        .dimensions
        .iter()
        .zip(contract.axis_cardinalities.iter())
    {
        hash.usize(dimension.bound_index);
        hash.str(match dimension.role {
            super::ExploreGeneratorAxisRole::Context => "context",
            super::ExploreGeneratorAxisRole::Before => "before",
            super::ExploreGeneratorAxisRole::AfterIndependent => "after_independent",
        });
        hash.usize(dimension.role_field_index);
        hash.str(&dimension.label);
        hash.u128(*cardinality);
    }
    hash.u128(contract.lift_dimension_indices.len() as u128);
    for dimension_index in contract.lift_dimension_indices.iter() {
        hash.usize(*dimension_index);
    }
    hash.u128(contract.retained_configuration_dimension_indices.len() as u128);
    for dimension_index in contract.retained_configuration_dimension_indices.iter() {
        hash.usize(*dimension_index);
    }
    hash.u128(contract.retained_key_names.len() as u128);
    for name in contract.retained_key_names.iter() {
        hash.str(name);
    }
    hash.u128(contract.retained_shown_names.len() as u128);
    for name in contract.retained_shown_names.iter() {
        hash.str(name);
    }
    hash.bool(contract.mechanism_trace_authorized);

    match &domain.boundary {
        None => hash.str("no-boundary"),
        Some(boundary) => {
            hash.str("boundary");
            hash.usize(boundary.axis);
            hash.i64(boundary.step);
            hash.bool(
                contract
                    .boundary
                    .is_some_and(|contract| contract.requires_both_endpoints_in_domain),
            );
            hash.u128(boundary.supports.len() as u128);
            for support in boundary.supports.iter() {
                match support {
                    BoundarySupport::Dense {
                        first_value,
                        first_ordinal,
                        cardinality,
                    } => {
                        hash.str("dense");
                        hash.i64(*first_value);
                        hash.u128(*first_ordinal);
                        hash.u128(*cardinality);
                    }
                    BoundarySupport::Sparse(points) => {
                        hash.str("sparse");
                        hash.u128(points.len() as u128);
                        for point in points.iter() {
                            hash.i64(point.value);
                            hash.u128(point.ordinal);
                        }
                    }
                }
            }
        }
    }
    hash.u128(domain.lift_dimensions.len() as u128);
    for axis in domain.lift_dimensions.iter() {
        hash.usize(*axis);
    }

    hash.str("scheduler-state");
    hash.usize(state.selector_index);
    hash.u128(state.endpoint_cursor);
    hash.u128(state.observed.len() as u128);
    for case_id in &state.observed {
        hash.case_id(case_id);
    }
    let mut pending = state.pending_lifts.values().collect::<Vec<_>>();
    pending.sort_by(|left, right| {
        (&left.origin_case_id, &left.candidate_case_id)
            .cmp(&(&right.origin_case_id, &right.candidate_case_id))
    });
    hash.u128(pending.len() as u128);
    for lift in pending {
        hash.case_id(&lift.origin_case_id);
        hash.case_id(&lift.candidate_case_id);
        match &lift.fixed_boundary_value {
            ExploreValue::Int(value) => {
                hash.str("int");
                hash.i64(*value);
            }
            _ => {
                return Err(invalid(
                    "probe pending lift retains a non-Int boundary value",
                ))
            }
        }
    }
    hash.bool(domain.endpoint_case_count != 0);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU128;

    use super::*;
    use crate::explore::probe::{
        ProbeArtifactState, ProbeBoundaryContract, ProbeBoundaryEndpoint, ProbeBoundaryValues,
        ProbeCursor, ProbeDimensionDescriptor, ProbeEndpointState, ProbeObservation,
        ProbePartialReason, ProbeRetainedOutputs, ProbeSemanticIdentity, PROBE_ARTIFACT_SCHEMA_V2,
    };
    use crate::{ExploreGeneratorAxisRole, ExplorePolarity, Lexer, Parser, TypeChecker};

    fn digest(seed: &str) -> Box<str> {
        format!(
            "{:064x}",
            seed.bytes().fold(17_u128, |value, byte| {
                value.wrapping_mul(257).wrapping_add(u128::from(byte))
            })
        )
        .into_boxed_str()
    }

    fn contract(
        cardinalities: &[u128],
        selectors: &[ProbeSelector],
        cap: u128,
    ) -> ProbePlanContract {
        ProbePlanContract {
            artifact_schema: PROBE_ARTIFACT_SCHEMA_V2.into(),
            normalization_version: "probe-normalization-v2".into(),
            selector_tie_break_version: PROBE_SCHEDULER_TIE_BREAK_V1.into(),
            query_name: "probe_canary".into(),
            identity: ProbeSemanticIdentity {
                program_hash: digest("program"),
                analysis_program_hash: digest("analysis"),
                query_hash: digest("query"),
                domain_hash: digest("domain"),
                probe_plan_hash: digest("plan"),
                evaluator_contract_hash: digest("evaluator"),
            },
            polarity: ExplorePolarity::Matches,
            dimensions: (0..cardinalities.len())
                .map(|axis| ProbeDimensionDescriptor {
                    bound_index: axis,
                    role: ExploreGeneratorAxisRole::Before,
                    role_field_index: axis,
                    label: format!("axis_{axis}"),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            axis_cardinalities: cardinalities.to_vec().into_boxed_slice(),
            boundary: Some(ProbeBoundaryContract {
                axis: cardinalities.len() - 1,
                step: 1,
                requires_both_endpoints_in_domain: true,
            }),
            selectors: selectors.to_vec().into_boxed_slice(),
            semantic_case_cap: NonZeroU128::new(cap).unwrap(),
            initial_frontier: ProbeFrontierId::new(digest("placeholder")).unwrap(),
            lift_dimension_indices: Box::new([]),
            retained_configuration_dimension_indices: Box::new([]),
            retained_key_names: Box::new([]),
            retained_shown_names: Box::new([]),
            mechanism_trace_authorized: false,
        }
    }

    fn dense_domain(cardinalities: &[u128]) -> ProbeSchedulingDomain {
        let axis = cardinalities.len() - 1;
        let eligible = cardinalities[axis].saturating_sub(1);
        let supports = if eligible == 0 {
            Vec::new()
        } else {
            vec![BoundarySupport::Dense {
                first_value: 100,
                first_ordinal: 0,
                cardinality: eligible,
            }]
        };
        let endpoint_ordinals = match eligible {
            0 => Vec::new(),
            1 => vec![0],
            _ => vec![0, eligible - 1],
        };
        let outer_cardinalities = cardinalities[..axis].to_vec();
        let outer_profile_count = checked_product(&outer_cardinalities).unwrap();
        ProbeSchedulingDomain {
            axis_cardinalities: cardinalities.to_vec().into_boxed_slice(),
            boundary: Some(BoundarySchedulingDomain {
                axis,
                step: 1,
                supports: supports.into_boxed_slice(),
                endpoint_ordinals: endpoint_ordinals.clone().into_boxed_slice(),
                eligible_lower_count: eligible,
            }),
            outer_cardinalities: outer_cardinalities.into_boxed_slice(),
            outer_profile_count,
            endpoint_case_count: outer_profile_count * endpoint_ordinals.len() as u128,
            lift_dimensions: Box::new([]),
        }
    }

    fn scheduler(cardinalities: &[u128], selectors: &[ProbeSelector], cap: u128) -> ProbeScheduler {
        let domain = dense_domain(cardinalities);
        let mut contract = contract(cardinalities, selectors, cap);
        contract.initial_frontier =
            hash_frontier(&contract, &domain, &SchedulerState::initial()).unwrap();
        ProbeScheduler::from_domain(&contract, domain).unwrap()
    }

    fn scheduled(outcome: ProbeScheduleOutcome) -> ProbeScheduledCase {
        match outcome {
            ProbeScheduleOutcome::Scheduled(scheduled) => scheduled,
            other => panic!("expected scheduled case, got {other:?}"),
        }
    }

    #[test]
    fn role_colliding_dimension_labels_reconcile_structurally_from_query() {
        let source = r#"
# RateState = RateState(rate: Int)
# RateContext = RateContext(rate: Int)

| changed(before: RateState, after: RateState, context: RateContext) ->
    after.rate > before.rate under context.rate >= 0

? explore rate_change {
    over changed(before, after, context)
    find matches
    bounds {
        context.rate in range(1, 3)
        before.rate in range(10, 14)
    }
    transition as RateState context RateContext {
        after.rate = before.rate + 1
    }
    boundaries on before.rate by 1
    output {
        key [rate = before.rate]
        representative first
    }
}
"#;
        let mut lexer = Lexer::new(source);
        let statements = Parser::new(lexer.tokenize(), source)
            .parse_program()
            .expect("parse role-colliding probe fixture");
        let artifacts = TypeChecker::check_with_artifacts(&statements, None, source);
        assert!(
            artifacts.diagnostics.is_empty(),
            "{:?}",
            artifacts.diagnostics
        );
        let query = &artifacts.exploration_universes[0];
        let selectors = [
            ProbeSelector::BoundaryEndpoints,
            ProbeSelector::FrontierMidpoints,
        ];
        let mut contract = contract(&[2, 4], &selectors, 4);
        contract.dimensions = query
            .universe
            .dimensions
            .iter()
            .map(|dimension| ProbeDimensionDescriptor {
                bound_index: dimension.bound_index,
                role: dimension.role,
                role_field_index: dimension.role_field_index,
                label: dimension.name.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        contract.lift_dimension_indices = vec![0].into_boxed_slice();

        let domain = ProbeSchedulingDomain::from_query(&contract, query)
            .expect("structural dimension reconciliation");
        assert_eq!(contract.dimensions[0].label, "rate");
        assert_eq!(contract.dimensions[1].label, "rate");
        assert_eq!(
            contract.dimensions[0].role,
            ExploreGeneratorAxisRole::Context
        );
        assert_eq!(
            contract.dimensions[1].role,
            ExploreGeneratorAxisRole::Before
        );
        assert_eq!(domain.lift_dimensions.as_ref(), [0]);
        assert_eq!(
            domain.boundary.as_ref().map(|boundary| boundary.axis),
            Some(1)
        );
        assert_eq!(contract.selectors.as_ref(), selectors);
    }

    #[test]
    fn endpoint_selection_is_deterministic_and_canonical() {
        let selectors = [ProbeSelector::BoundaryEndpoints];
        let mut first = scheduler(&[2, 5], &selectors, 8);
        let mut second = first.clone();
        let mut observed = Vec::new();
        for _ in 0..4 {
            let left = scheduled(first.next().unwrap());
            let right = scheduled(second.next().unwrap());
            assert_eq!(left.case_id, right.case_id);
            observed.push(left.case_id.ordinals().to_vec());
            first
                .record_classification(&left, ProbeClassificationKind::Nonmatch)
                .unwrap();
            second
                .record_classification(&right, ProbeClassificationKind::Nonmatch)
                .unwrap();
        }
        assert_eq!(
            observed,
            vec![vec![0, 0], vec![0, 3], vec![1, 0], vec![1, 3]]
        );
        assert_eq!(
            first.next().unwrap(),
            ProbeScheduleOutcome::Complete(ProbeCompletionReason::PlanExhausted)
        );
    }

    #[test]
    fn later_selectors_never_schedule_an_observed_case_twice() {
        let selectors = [
            ProbeSelector::BoundaryEndpoints,
            ProbeSelector::FrontierMidpoints,
        ];
        let mut scheduler = scheduler(&[1, 2], &selectors, 4);
        let only = scheduled(scheduler.next().unwrap());
        assert_eq!(only.case_id.ordinals(), &[0, 0]);
        scheduler
            .record_classification(&only, ProbeClassificationKind::Nonmatch)
            .unwrap();
        assert_eq!(
            scheduler.next().unwrap(),
            ProbeScheduleOutcome::Complete(ProbeCompletionReason::NoMoreUniqueProbes)
        );
    }

    #[test]
    fn midpoint_selector_bisects_the_largest_remaining_support() {
        let mut scheduler = scheduler(&[1, 9], &[ProbeSelector::FrontierMidpoints], 8);
        let mut positions = Vec::new();
        for _ in 0..3 {
            let next = scheduled(scheduler.next().unwrap());
            positions.push(next.case_id.ordinals()[1]);
            scheduler
                .record_classification(&next, ProbeClassificationKind::Nonmatch)
                .unwrap();
        }
        assert_eq!(positions, vec![4, 2, 6]);
    }

    #[test]
    fn lifted_matches_drain_before_the_declared_selector_resumes() {
        let selectors = [ProbeSelector::BoundaryEndpoints];
        let mut domain = dense_domain(&[3, 3]);
        domain.lift_dimensions = vec![0].into_boxed_slice();
        let mut contract = contract(&[3, 3], &selectors, 4);
        contract.lift_dimension_indices = vec![0].into_boxed_slice();
        contract.initial_frontier =
            hash_frontier(&contract, &domain, &SchedulerState::initial()).unwrap();
        let mut scheduler = ProbeScheduler::from_domain(&contract, domain).unwrap();

        let origin = scheduled(scheduler.next().unwrap());
        assert_eq!(origin.case_id.ordinals(), &[0, 0]);
        scheduler
            .record_classification(&origin, ProbeClassificationKind::Match)
            .unwrap();

        let first_lift = scheduled(scheduler.next().unwrap());
        assert_eq!(first_lift.case_id.ordinals(), &[1, 0]);
        assert_eq!(
            first_lift.scheduling_reason,
            ProbeSchedulingReason::Lifted {
                origin_case_id: origin.case_id.clone()
            }
        );
        scheduler
            .record_classification(&first_lift, ProbeClassificationKind::Nonmatch)
            .unwrap();
        let second_lift = scheduled(scheduler.next().unwrap());
        assert_eq!(second_lift.case_id.ordinals(), &[2, 0]);
    }

    #[test]
    fn semantic_cap_is_distinct_from_selector_exhaustion() {
        let mut scheduler = scheduler(&[1, 5], &[ProbeSelector::BoundaryEndpoints], 1);
        let first = scheduled(scheduler.next().unwrap());
        scheduler
            .record_classification(&first, ProbeClassificationKind::Nonmatch)
            .unwrap();
        assert_eq!(
            scheduler.next().unwrap(),
            ProbeScheduleOutcome::Complete(ProbeCompletionReason::BudgetReached)
        );
    }

    #[test]
    fn boundary_candidates_never_masquerade_as_an_empty_selector() {
        let scheduler = scheduler(&[1, 3], &[ProbeSelector::BoundaryCandidates], 2);
        assert_eq!(
            scheduler.next().unwrap(),
            ProbeScheduleOutcome::Unsupported(
                ProbeSchedulerUnsupported::BoundaryCandidatesNeedResolvedFragmentAdapter
            )
        );
    }

    #[test]
    fn length_prefixed_frontiers_separate_ambiguous_ordinal_sequences() {
        let selectors = [ProbeSelector::FrontierMidpoints];
        let first = scheduler(&[13, 25], &selectors, 4);
        let mut second = first.clone();
        let mut third = first.clone();
        second
            .state
            .observed
            .insert(ExploreCaseId::new(vec![1, 23]));
        third.state.observed.insert(ExploreCaseId::new(vec![12, 3]));
        assert_eq!(
            first.current_frontier_id().unwrap(),
            first.clone().current_frontier_id().unwrap()
        );
        assert_ne!(
            second.current_frontier_id().unwrap(),
            third.current_frontier_id().unwrap()
        );
    }

    fn observation(scheduled: &ProbeScheduledCase) -> ProbeObservation {
        let lower = 100 + scheduled.case_id.ordinals()[1] as i64;
        ProbeObservation {
            case_id: scheduled.case_id.clone(),
            configuration: Box::new([]),
            boundary_values: ProbeBoundaryValues {
                lower: Some(ProbeBoundaryEndpoint {
                    value: ExploreValue::Int(lower),
                    state: ProbeEndpointState::Evaluated,
                }),
                upper: Some(ProbeBoundaryEndpoint {
                    value: ExploreValue::Int(lower + 1),
                    state: ProbeEndpointState::Evaluated,
                }),
            },
            classification: ProbeClassification::Nonmatch {
                question_value: false,
            },
            outputs: ProbeRetainedOutputs::Available {
                key: Box::new([]),
                shown: Box::new([]),
            },
            scheduling_reason: scheduled.scheduling_reason.clone(),
            mechanism_signature: None,
        }
    }

    #[test]
    fn transcript_replay_rejects_a_stored_frontier_tamper() {
        let selectors = [ProbeSelector::BoundaryEndpoints];
        let mut original = scheduler(&[1, 4], &selectors, 3);
        let scheduled = scheduled(original.next().unwrap());
        let observation = observation(&scheduled);
        let mut decision = original
            .record_classification(&scheduled, ProbeClassificationKind::Nonmatch)
            .unwrap();
        decision.frontier_after =
            ProbeFrontierState::Open(ProbeFrontierId::new(digest("tampered")).unwrap());
        let artifact = ProbeArtifact {
            contract: original.contract.clone(),
            state: ProbeArtifactState::Partial {
                reason: ProbePartialReason::Interrupted,
            },
            cursor: ProbeCursor {
                next_decision: 1,
                frontier: decision.frontier_after.clone(),
            },
            counts: original.counts().unwrap(),
            observations: vec![observation].into_boxed_slice(),
            transcript: vec![decision].into_boxed_slice(),
            lifted_candidates: Box::new([]),
        };
        let error = replay_probe_artifact_with_domain(
            &original.contract,
            original.domain.clone(),
            &artifact,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("noncanonical frontier transition"));
    }
}
