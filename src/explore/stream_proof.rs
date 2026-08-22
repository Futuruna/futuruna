//! Conservative lowering from checked source proofs into durable exact-stream
//! evidence.
//!
//! A classification certificate can close only an admissible nonmatch. Proof
//! matches remain singleton evaluator work because v1 cannot attach their
//! complete report projection. Structural boundary suffixes are excluded
//! without evaluation. Both forms lower to rank intervals only when the
//! boundary axis has mixed-radix stride one; otherwise their support remains
//! open rather than being expanded into singleton records.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::case_graph::CaseTerminal;
use super::classification_regions::SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1;
use super::exact_stream::{
    encode_exact_closed_region_batch_v1, exact_closed_region_batch_limit_v1,
    seal_revalidated_proof_or_structure_batch, ExactCanonicalCaseIdV1, ExactClosedClassificationV1,
    ExactClosedRankRegionProposalV1, ExactClosedRegionBatchProposalV1, ExactClosedRegionKindV1,
    ExactStreamError, ExactValidationReceiptDigestV1, ValidatedExactClosedRegionBatchV1,
};
use super::source_events::SOURCE_PROOF_EXTRACTION_OPTIONS_V1;
use super::source_proof_plan::{SourceProofPlan, DEFAULT_SOURCE_PROOF_PROFILE_LIMIT};
use super::{ExploreExactDomain, ExplorePolarity, ExploreQueryIr};

const STRUCTURAL_SUFFIX_RECEIPT_V1: &[u8] =
    b"futuruna.explore.exact-stream.structural-boundary-suffix.v1";
const SOURCE_PROOF_OUTPUT_DIGEST_V1: &[u8] =
    b"futuruna.explore.exact-stream.source-proof-output.v1";
const CANDIDATE_RANK_MAGIC_V1: &[u8; 8] = b"FXCAN001";
/// Aggregate durable hint bound across the complete first-generation probe.
/// The 64-profile by 64-ordinal preparation budget can produce at most this
/// many distinct candidate ranks. Excess hints remain canonical fallback.
const MAX_CANDIDATE_RANKS_V1: usize = DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get()
    * SOURCE_PROOF_EXTRACTION_OPTIONS_V1
        .max_candidate_ordinals
        .get();
/// Exact canonical v1 encoding size at the rank-count cap (magic + count +
/// fixed-width u128 ranks). Keeping this derived alongside the count prevents
/// a second, accidentally looser allocation budget.
const MAX_CANDIDATE_RANK_BYTES_V1: usize = 8 + 4 + (MAX_CANDIDATE_RANKS_V1 * 16);
/// Proof/structural rectangles retained by one atomic first-generation probe.
/// Certification beyond this cap stays open; the wire format may impose an
/// even smaller effective bound.
const MAX_SOURCE_PROOF_CLOSED_REGIONS_V1: usize = DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get()
    * SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1
        .max_refinement_cells
        .get();

pub(super) const fn source_proof_candidate_rank_limit_v1() -> usize {
    MAX_CANDIDATE_RANKS_V1
}

pub(super) const fn source_proof_candidate_rank_bytes_limit_v1() -> usize {
    MAX_CANDIDATE_RANK_BYTES_V1
}

pub(super) const fn source_proof_closed_region_limit_v1() -> usize {
    let wire_limit = exact_closed_region_batch_limit_v1();
    if wire_limit < MAX_SOURCE_PROOF_CLOSED_REGIONS_V1 {
        wire_limit
    } else {
        MAX_SOURCE_PROOF_CLOSED_REGIONS_V1
    }
}

/// Exact accounting for support this adapter sealed or deliberately left open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceProofExactCoverageSummaryV1 {
    universe_case_count: u128,
    boundary_rank_stride: u128,
    sealed_proof_nonmatch_cases: u128,
    open_proof_nonmatch_cases: u128,
    open_proof_match_cases: u128,
    sealed_structural_excluded_cases: u128,
    open_structural_excluded_cases: u128,
    region_limit_reached: bool,
    candidate_limit_reached: bool,
}

impl SourceProofExactCoverageSummaryV1 {
    pub(super) const fn universe_case_count(self) -> u128 {
        self.universe_case_count
    }

    pub(super) const fn boundary_rank_stride(self) -> u128 {
        self.boundary_rank_stride
    }

    pub(super) const fn sealed_proof_nonmatch_cases(self) -> u128 {
        self.sealed_proof_nonmatch_cases
    }

    pub(super) const fn open_proof_nonmatch_cases(self) -> u128 {
        self.open_proof_nonmatch_cases
    }

    pub(super) const fn open_proof_match_cases(self) -> u128 {
        self.open_proof_match_cases
    }

    pub(super) const fn sealed_structural_excluded_cases(self) -> u128 {
        self.sealed_structural_excluded_cases
    }

    pub(super) const fn open_structural_excluded_cases(self) -> u128 {
        self.open_structural_excluded_cases
    }

    pub(super) const fn region_limit_reached(self) -> bool {
        self.region_limit_reached
    }

    pub(super) const fn candidate_limit_reached(self) -> bool {
        self.candidate_limit_reached
    }
}

/// Content identity of the normalized closed-region blob, canonical candidate
/// blob, and explicit open-support accounting returned by this adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct SourceProofExactOutputDigestV1([u8; 32]);

impl SourceProofExactOutputDigestV1 {
    pub(super) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Producer identities retained from the validated source plan. A coordinator
/// must bind these to the run header before installing proof-derived evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceProofProducerIdentityV1 {
    analysis_program_digest: [u8; 32],
    query_digest: [u8; 32],
}

impl SourceProofProducerIdentityV1 {
    pub(super) const fn analysis_program_digest(self) -> [u8; 32] {
        self.analysis_program_digest
    }

    pub(super) const fn query_digest(self) -> [u8; 32] {
        self.query_digest
    }
}

/// Prepared source-proof contribution for one exact stream.
///
/// `closed_regions` is `None` when there is no compact nonmatching or
/// structural support to seal. Candidate order is the deterministic source
/// plan selection with first-occurrence deduplication, then canonical rank
/// order; candidates already closed by the returned batch are omitted.
#[derive(Debug)]
pub(super) struct PreparedSourceProofExactCoverageV1 {
    closed_regions: Option<ValidatedExactClosedRegionBatchV1>,
    candidate_case_ids: Box<[ExactCanonicalCaseIdV1]>,
    summary: SourceProofExactCoverageSummaryV1,
    output_digest: SourceProofExactOutputDigestV1,
    producer_identity: Option<SourceProofProducerIdentityV1>,
}

impl PreparedSourceProofExactCoverageV1 {
    pub(super) fn closed_regions(&self) -> Option<&ValidatedExactClosedRegionBatchV1> {
        self.closed_regions.as_ref()
    }

    pub(super) fn candidate_case_ids(&self) -> &[ExactCanonicalCaseIdV1] {
        &self.candidate_case_ids
    }

    pub(super) const fn summary(&self) -> SourceProofExactCoverageSummaryV1 {
        self.summary
    }

    pub(super) const fn output_digest(&self) -> SourceProofExactOutputDigestV1 {
        self.output_digest
    }

    pub(super) const fn producer_identity(&self) -> Option<SourceProofProducerIdentityV1> {
        self.producer_identity
    }

    pub(super) fn encode_candidate_ranks_v1(
        &self,
    ) -> Result<Vec<u8>, SourceProofExactAdapterError> {
        let ranks = self
            .candidate_case_ids
            .iter()
            .map(|case_id| case_id.rank)
            .collect::<Vec<_>>();
        encode_source_proof_candidate_ranks_v1(&ranks, self.summary.universe_case_count)
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        Option<ValidatedExactClosedRegionBatchV1>,
        Box<[ExactCanonicalCaseIdV1]>,
        SourceProofExactCoverageSummaryV1,
        SourceProofExactOutputDigestV1,
        Option<SourceProofProducerIdentityV1>,
    ) {
        (
            self.closed_regions,
            self.candidate_case_ids,
            self.summary,
            self.output_digest,
            self.producer_identity,
        )
    }
}

/// Invalid checked-plan identity or arithmetic while preparing stream proof
/// evidence. The plan constructor has already performed the expensive source
/// proof validation; this adapter still rechecks every identity it consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceProofExactAdapterError(Box<str>);

impl SourceProofExactAdapterError {
    fn invalid(message: impl Into<String>) -> Self {
        Self(message.into().into_boxed_str())
    }
}

impl fmt::Display for SourceProofExactAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SourceProofExactAdapterError {}

impl From<ExactStreamError> for SourceProofExactAdapterError {
    fn from(error: ExactStreamError) -> Self {
        Self::invalid(error.to_string())
    }
}

/// Prepare proof/structural closure and source-event scheduling hints for one
/// checked query. This does not evaluate a case and never seals a proof match.
pub(super) fn prepare_source_proof_exact_coverage_v1(
    query: &ExploreQueryIr,
    plan: &SourceProofPlan,
) -> Result<PreparedSourceProofExactCoverageV1, SourceProofExactAdapterError> {
    let shape = ExactBoundaryShape::from_checked_query(query)?;
    let producer_identity = validate_plan_identity(query, plan, &shape)?;

    let derived = derive_closed_regions(query, plan, &shape)?;
    let closed_index = ClosedRankIndex::from_proposal(derived.proposal.as_ref());
    let (candidate_case_ids, candidate_limit_reached) =
        derive_candidate_case_ids(plan, &shape, &closed_index)?;
    let candidate_case_ids = candidate_case_ids.into_boxed_slice();
    if producer_identity.is_none() && (derived.proposal.is_some() || !candidate_case_ids.is_empty())
    {
        return Err(SourceProofExactAdapterError::invalid(
            "source-derived output is missing its producer identity",
        ));
    }
    let mut summary = derived.summary;
    summary.candidate_limit_reached = candidate_limit_reached;

    let candidate_ranks = candidate_case_ids
        .iter()
        .map(|case_id| case_id.rank)
        .collect::<Vec<_>>();
    let candidate_bytes =
        encode_source_proof_candidate_ranks_v1(&candidate_ranks, shape.universe_case_count)?;
    let region_bytes = derived
        .proposal
        .as_ref()
        .map(encode_exact_closed_region_batch_v1)
        .transpose()?;
    let output_digest = derive_output_digest(
        &shape,
        region_bytes.as_deref(),
        &candidate_bytes,
        summary,
        producer_identity,
    );

    let closed_regions = match derived.proposal {
        None => None,
        Some(proposal) => {
            let sealed = seal_revalidated_proof_or_structure_batch(proposal, |candidate| {
                let confirmed = derive_closed_regions(query, plan, &shape)
                    .map_err(|error| error.to_string())?
                    .proposal
                    .ok_or_else(|| {
                        "source plan no longer derives a nonempty closed-region batch".to_string()
                    })?;
                if &confirmed != candidate {
                    return Err(
                        "closed-region/receipt union disagrees with the validated source plan"
                            .to_string(),
                    );
                }
                Ok(())
            })?;
            Some(sealed)
        }
    };

    Ok(PreparedSourceProofExactCoverageV1 {
        closed_regions,
        candidate_case_ids,
        summary,
        output_digest,
        producer_identity,
    })
}

#[derive(Debug, Clone)]
struct ExactBoundaryShape {
    axis_cardinalities: Box<[u128]>,
    universe_case_count: u128,
    boundary_dimension: usize,
    boundary_start: i64,
    boundary_end_exclusive: i64,
    boundary_cardinality: u128,
    boundary_step: i64,
    eligible_end_exclusive: i64,
    boundary_rank_stride: u128,
}

impl ExactBoundaryShape {
    fn from_checked_query(query: &ExploreQueryIr) -> Result<Self, SourceProofExactAdapterError> {
        let boundary = query.boundary_hint().ok_or_else(|| {
            SourceProofExactAdapterError::invalid(
                "source-proof stream lowering requires a checked boundary query",
            )
        })?;
        if boundary.step <= 0 {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "checked boundary step {} is not positive",
                boundary.step
            )));
        }
        if !boundary.requires_both_endpoints_in_domain {
            return Err(SourceProofExactAdapterError::invalid(
                "source-proof stream lowering requires both boundary endpoints in-domain",
            ));
        }
        let dimension = query
            .universe
            .dimensions
            .get(boundary.axis_dimension_index)
            .ok_or_else(|| {
                SourceProofExactAdapterError::invalid(format!(
                    "boundary axis index {} is outside {} dimensions",
                    boundary.axis_dimension_index,
                    query.universe.dimensions.len()
                ))
            })?;
        if dimension.name != boundary.axis {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "boundary names `{}` but dimension {} is `{}`",
                boundary.axis, boundary.axis_dimension_index, dimension.name
            )));
        }
        let (start, end_exclusive, declared_cardinality) = match &dimension.domain {
            ExploreExactDomain::IntRange {
                start,
                end_exclusive,
                cardinality,
            } => (*start, *end_exclusive, u128::from(*cardinality)),
            ExploreExactDomain::Enumerated { .. } | ExploreExactDomain::FiniteType { .. } => {
                return Err(SourceProofExactAdapterError::invalid(
                    "source-proof stream lowering requires a dense Int boundary range",
                ))
            }
        };
        let width = i128::from(end_exclusive)
            .checked_sub(i128::from(start))
            .and_then(|width| u128::try_from(width).ok())
            .ok_or_else(|| {
                SourceProofExactAdapterError::invalid(
                    "checked boundary range is reversed or exceeds u128",
                )
            })?;
        if width != declared_cardinality {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "dense boundary width {width} disagrees with cardinality {declared_cardinality}"
            )));
        }

        let mut axis_cardinalities = Vec::with_capacity(query.universe.dimensions.len());
        for dimension in &query.universe.dimensions {
            let cardinality = dimension.domain.cardinality().exact().ok_or_else(|| {
                SourceProofExactAdapterError::invalid(format!(
                    "dimension `{}` cardinality exceeds u128",
                    dimension.name
                ))
            })?;
            axis_cardinalities.push(cardinality);
        }
        let universe_case_count = checked_product(&axis_cardinalities, "case universe")?;
        let boundary_rank_stride = if universe_case_count == 0 {
            0
        } else {
            checked_product(
                &axis_cardinalities[boundary.axis_dimension_index + 1..],
                "boundary rank stride",
            )?
        };
        let eligible_end_exclusive = i128::from(end_exclusive)
            .checked_sub(i128::from(boundary.step))
            .map(|end| end.max(i128::from(start)))
            .and_then(|end| i64::try_from(end).ok())
            .ok_or_else(|| {
                SourceProofExactAdapterError::invalid(
                    "eligible boundary endpoint arithmetic overflowed",
                )
            })?;

        Ok(Self {
            axis_cardinalities: axis_cardinalities.into_boxed_slice(),
            universe_case_count,
            boundary_dimension: boundary.axis_dimension_index,
            boundary_start: start,
            boundary_end_exclusive: end_exclusive,
            boundary_cardinality: declared_cardinality,
            boundary_step: boundary.step,
            eligible_end_exclusive,
            boundary_rank_stride,
        })
    }

    fn outer_cardinalities(&self) -> Box<[u128]> {
        self.axis_cardinalities
            .iter()
            .enumerate()
            .filter_map(|(index, cardinality)| {
                (index != self.boundary_dimension).then_some(*cardinality)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    fn validate_outer_ordinals(
        &self,
        outer_ordinals: &[u128],
    ) -> Result<(), SourceProofExactAdapterError> {
        if outer_ordinals.len() + 1 != self.axis_cardinalities.len() {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "outer profile has {} ordinals for {} non-boundary axes",
                outer_ordinals.len(),
                self.axis_cardinalities.len().saturating_sub(1)
            )));
        }
        let mut outer_index = 0_usize;
        for (axis, cardinality) in self.axis_cardinalities.iter().copied().enumerate() {
            if axis == self.boundary_dimension {
                continue;
            }
            let ordinal = outer_ordinals[outer_index];
            if ordinal >= cardinality {
                return Err(SourceProofExactAdapterError::invalid(format!(
                    "outer ordinal {ordinal} at axis {axis} is outside cardinality {cardinality}"
                )));
            }
            outer_index += 1;
        }
        Ok(())
    }

    fn case_id(
        &self,
        outer_ordinals: &[u128],
        boundary_ordinal: u128,
    ) -> Result<ExactCanonicalCaseIdV1, SourceProofExactAdapterError> {
        self.validate_outer_ordinals(outer_ordinals)?;
        if boundary_ordinal >= self.boundary_cardinality {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "boundary ordinal {boundary_ordinal} is outside cardinality {}",
                self.boundary_cardinality
            )));
        }
        let mut outer_index = 0_usize;
        let mut rank = 0_u128;
        let mut ordinals = Vec::with_capacity(self.axis_cardinalities.len());
        for (axis, cardinality) in self.axis_cardinalities.iter().copied().enumerate() {
            let ordinal = if axis == self.boundary_dimension {
                boundary_ordinal
            } else {
                let ordinal = outer_ordinals[outer_index];
                outer_index += 1;
                ordinal
            };
            rank = rank
                .checked_mul(cardinality)
                .and_then(|prefix| prefix.checked_add(ordinal))
                .ok_or_else(|| {
                    SourceProofExactAdapterError::invalid("mixed-radix candidate rank exceeds u128")
                })?;
            ordinals.push(ordinal);
        }
        if rank >= self.universe_case_count {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "candidate rank {rank} is outside universe {}",
                self.universe_case_count
            )));
        }
        Ok(ExactCanonicalCaseIdV1::new(rank, ordinals))
    }

    fn compact_rank_interval(
        &self,
        outer_ordinals: &[u128],
        start: i64,
        end_exclusive: i64,
    ) -> Result<(u128, u128), SourceProofExactAdapterError> {
        if self.boundary_rank_stride != 1 {
            return Err(SourceProofExactAdapterError::invalid(
                "non-unit boundary rank stride is not compactly representable",
            ));
        }
        if start < self.boundary_start
            || start >= end_exclusive
            || end_exclusive > self.boundary_end_exclusive
        {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "boundary interval [{start}, {end_exclusive}) is empty or outside [{}, {})",
                self.boundary_start, self.boundary_end_exclusive
            )));
        }
        let start_ordinal = dense_boundary_ordinal(self.boundary_start, start)?;
        let width = dense_boundary_ordinal(start, end_exclusive)?;
        let start_rank = self.case_id(outer_ordinals, start_ordinal)?.rank;
        let end_rank_exclusive = start_rank.checked_add(width).ok_or_else(|| {
            SourceProofExactAdapterError::invalid("closed rank interval exceeds u128")
        })?;
        if end_rank_exclusive > self.universe_case_count {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "closed rank interval [{start_rank}, {end_rank_exclusive}) exceeds universe {}",
                self.universe_case_count
            )));
        }
        Ok((start_rank, end_rank_exclusive))
    }
}

fn validate_plan_identity(
    query: &ExploreQueryIr,
    plan: &SourceProofPlan,
    shape: &ExactBoundaryShape,
) -> Result<Option<SourceProofProducerIdentityV1>, SourceProofExactAdapterError> {
    let expected_name = query.query.name.as_deref().unwrap_or("<anonymous>");
    let expected_axis = &query.universe.dimensions[shape.boundary_dimension].name;
    let mut identity = None::<SourceProofProducerIdentityV1>;
    let mut extraction_profiles = BTreeSet::<Box<[u128]>>::new();

    for extraction in plan.extractions() {
        if extraction.query_name != expected_name
            || extraction.axis_name != *expected_axis
            || extraction.step != shape.boundary_step
        {
            return Err(SourceProofExactAdapterError::invalid(
                "source extraction does not identify the supplied checked query",
            ));
        }
        let current_identity = SourceProofProducerIdentityV1 {
            analysis_program_digest: parse_lowercase_sha256(
                &extraction.analysis_program_hash,
                "analysis program hash",
            )?,
            query_digest: parse_lowercase_sha256(&extraction.query_hash, "query hash")?,
        };
        match identity {
            None => identity = Some(current_identity),
            Some(existing) if existing == current_identity => {}
            Some(_) => {
                return Err(SourceProofExactAdapterError::invalid(
                    "source plan mixes producer or checked-query identities",
                ))
            }
        }
        shape.validate_outer_ordinals(&extraction.outer_ordinals)?;
        if !extraction_profiles.insert(extraction.outer_ordinals.clone()) {
            return Err(SourceProofExactAdapterError::invalid(format!(
                "source plan repeats outer profile {:?}",
                extraction.outer_ordinals
            )));
        }
        for candidate in extraction.candidates.iter() {
            if candidate.events.is_empty() {
                return Err(SourceProofExactAdapterError::invalid(format!(
                    "source candidate {} has no event",
                    candidate.boundary_value
                )));
            }
            let expected_ordinal =
                dense_boundary_ordinal(shape.boundary_start, candidate.boundary_value)?;
            if candidate.boundary_ordinal != expected_ordinal
                || candidate.boundary_ordinal >= shape.boundary_cardinality
                || candidate.boundary_value >= shape.eligible_end_exclusive
            {
                return Err(SourceProofExactAdapterError::invalid(format!(
                    "source candidate {} has an invalid dense boundary ordinal or endpoint",
                    candidate.boundary_value
                )));
            }
        }
    }

    let mut proof_profiles = BTreeSet::<Box<[u128]>>::new();
    for proof in plan.proofs() {
        let mut proof_profile = None::<Box<[u128]>>;
        for region in proof.regions() {
            shape.validate_outer_ordinals(region.outer_ordinals())?;
            if !extraction_profiles.contains(region.outer_ordinals()) {
                return Err(SourceProofExactAdapterError::invalid(
                    "classification proof has no matching validated source extraction",
                ));
            }
            match &proof_profile {
                None => proof_profile = Some(region.outer_ordinals().into()),
                Some(profile) if profile.as_ref() == region.outer_ordinals() => {}
                Some(_) => {
                    return Err(SourceProofExactAdapterError::invalid(
                        "one classification proof mixes outer profiles",
                    ))
                }
            }
            if region.certificate().interval() != region.interval() {
                return Err(SourceProofExactAdapterError::invalid(
                    "classification certificate interval disagrees with its region",
                ));
            }
            parse_lowercase_sha256(region.certificate().id(), "classification certificate id")?;
            let interval = region.interval();
            if interval.is_empty()
                || interval.start() < shape.boundary_start
                || interval.end_exclusive() > shape.eligible_end_exclusive
            {
                return Err(SourceProofExactAdapterError::invalid(
                    "classification certificate lies outside the eligible boundary interval",
                ));
            }
            let expected = match (
                query.query.polarity,
                region.certificate().raw_question_value(),
            ) {
                (ExplorePolarity::Matches, true) | (ExplorePolarity::Violations, false) => {
                    CaseTerminal::AdmissibleMatch
                }
                (ExplorePolarity::Matches, false) | (ExplorePolarity::Violations, true) => {
                    CaseTerminal::AdmissibleNonmatch
                }
            };
            if region.classification() != &expected {
                return Err(SourceProofExactAdapterError::invalid(
                    "classification certificate truth/polarity does not match its terminal",
                ));
            }
        }
        if let Some(profile) = proof_profile {
            if !proof_profiles.insert(profile.clone()) {
                return Err(SourceProofExactAdapterError::invalid(format!(
                    "source plan repeats a classification proof for outer profile {profile:?}"
                )));
            }
        }
    }
    Ok(identity)
}

struct DerivedClosedRegions {
    proposal: Option<ExactClosedRegionBatchProposalV1>,
    summary: SourceProofExactCoverageSummaryV1,
}

fn derive_closed_regions(
    query: &ExploreQueryIr,
    plan: &SourceProofPlan,
    shape: &ExactBoundaryShape,
) -> Result<DerivedClosedRegions, SourceProofExactAdapterError> {
    let _producer_identity = validate_plan_identity(query, plan, shape)?;
    let limit = source_proof_closed_region_limit_v1();
    let mut regions = Vec::<ExactClosedRankRegionProposalV1>::new();
    let mut summary = SourceProofExactCoverageSummaryV1 {
        universe_case_count: shape.universe_case_count,
        boundary_rank_stride: shape.boundary_rank_stride,
        sealed_proof_nonmatch_cases: 0,
        open_proof_nonmatch_cases: 0,
        open_proof_match_cases: 0,
        sealed_structural_excluded_cases: 0,
        open_structural_excluded_cases: 0,
        region_limit_reached: false,
        candidate_limit_reached: false,
    };

    derive_structural_suffix(shape, limit, &mut regions, &mut summary)?;

    for proof in plan.proofs() {
        for region in proof.regions() {
            let count = region.interval().cardinality();
            match region.classification() {
                CaseTerminal::AdmissibleMatch => {
                    summary.open_proof_match_cases = checked_add(
                        summary.open_proof_match_cases,
                        count,
                        "open proof-match support",
                    )?;
                }
                CaseTerminal::AdmissibleNonmatch => {
                    let receipt = ExactValidationReceiptDigestV1::new(parse_lowercase_sha256(
                        region.certificate().id(),
                        "classification certificate id",
                    )?);
                    if shape.boundary_rank_stride != 1 || regions.len() == limit {
                        summary.open_proof_nonmatch_cases = checked_add(
                            summary.open_proof_nonmatch_cases,
                            count,
                            "open proof-nonmatch support",
                        )?;
                        if shape.boundary_rank_stride == 1 {
                            summary.region_limit_reached = true;
                        }
                        continue;
                    }
                    let (start_rank, end_rank_exclusive) = shape.compact_rank_interval(
                        region.outer_ordinals(),
                        region.interval().start(),
                        region.interval().end_exclusive(),
                    )?;
                    regions.push(ExactClosedRankRegionProposalV1::new(
                        start_rank,
                        end_rank_exclusive,
                        ExactClosedRegionKindV1::Proof,
                        ExactClosedClassificationV1::AdmissibleNonmatch,
                        receipt,
                    )?);
                    summary.sealed_proof_nonmatch_cases = checked_add(
                        summary.sealed_proof_nonmatch_cases,
                        count,
                        "sealed proof-nonmatch support",
                    )?;
                }
                CaseTerminal::Excluded
                | CaseTerminal::EligibilityOpen(_)
                | CaseTerminal::AdmissibleOpen(_) => {
                    return Err(SourceProofExactAdapterError::invalid(
                        "classification proof carries an unsupported terminal",
                    ))
                }
            }
        }
    }

    let proposal = if regions.is_empty() {
        None
    } else {
        Some(ExactClosedRegionBatchProposalV1::new(regions)?)
    };
    Ok(DerivedClosedRegions { proposal, summary })
}

fn derive_structural_suffix(
    shape: &ExactBoundaryShape,
    limit: usize,
    regions: &mut Vec<ExactClosedRankRegionProposalV1>,
    summary: &mut SourceProofExactCoverageSummaryV1,
) -> Result<(), SourceProofExactAdapterError> {
    let suffix_width =
        dense_boundary_ordinal(shape.eligible_end_exclusive, shape.boundary_end_exclusive)?;
    if suffix_width == 0 || shape.universe_case_count == 0 {
        return Ok(());
    }
    let outer_cardinalities = shape.outer_cardinalities();
    let outer_profiles = checked_product(&outer_cardinalities, "outer profile universe")?;
    let total_support = suffix_width.checked_mul(outer_profiles).ok_or_else(|| {
        SourceProofExactAdapterError::invalid("structural suffix support exceeds u128")
    })?;
    if shape.boundary_rank_stride != 1 {
        summary.open_structural_excluded_cases = total_support;
        return Ok(());
    }

    // When every boundary point is structurally ineligible the union is the
    // whole rank universe and needs one receipt/region, independent of the
    // number of outer profiles.
    if suffix_width == shape.boundary_cardinality {
        if regions.len() == limit {
            summary.open_structural_excluded_cases = total_support;
            summary.region_limit_reached = true;
            return Ok(());
        }
        let receipt = structural_receipt(shape, None, 0, shape.universe_case_count);
        regions.push(ExactClosedRankRegionProposalV1::new(
            0,
            shape.universe_case_count,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            receipt,
        )?);
        summary.sealed_structural_excluded_cases = total_support;
        return Ok(());
    }

    let available = limit.saturating_sub(regions.len()) as u128;
    if outer_profiles > available {
        summary.open_structural_excluded_cases = total_support;
        summary.region_limit_reached = true;
        return Ok(());
    }
    for outer_ordinals in OuterOrdinalCursor::new(outer_cardinalities) {
        let (start_rank, end_rank_exclusive) = shape.compact_rank_interval(
            &outer_ordinals,
            shape.eligible_end_exclusive,
            shape.boundary_end_exclusive,
        )?;
        let receipt =
            structural_receipt(shape, Some(&outer_ordinals), start_rank, end_rank_exclusive);
        regions.push(ExactClosedRankRegionProposalV1::new(
            start_rank,
            end_rank_exclusive,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            receipt,
        )?);
    }
    summary.sealed_structural_excluded_cases = total_support;
    Ok(())
}

fn derive_candidate_case_ids(
    plan: &SourceProofPlan,
    shape: &ExactBoundaryShape,
    closed: &ClosedRankIndex,
) -> Result<(Vec<ExactCanonicalCaseIdV1>, bool), SourceProofExactAdapterError> {
    let mut candidates = BTreeMap::<u128, ExactCanonicalCaseIdV1>::new();
    let mut limit_reached = false;
    for extraction in plan.extractions() {
        for candidate in extraction.candidates.iter() {
            let case_id = shape.case_id(&extraction.outer_ordinals, candidate.boundary_ordinal)?;
            if closed.contains(case_id.rank) || candidates.contains_key(&case_id.rank) {
                continue;
            }
            if candidates.len() == MAX_CANDIDATE_RANKS_V1 {
                // Source events affect only order. Excess hints remain ordinary
                // canonical fallback work and cannot change exact closure.
                limit_reached = true;
                continue;
            }
            candidates.insert(case_id.rank, case_id);
        }
    }
    Ok((candidates.into_values().collect(), limit_reached))
}

/// Encode a canonical sorted/unique candidate-rank set for durable discovery
/// events. The universe cardinality is part of validation, not the blob: the
/// containing run manifest binds the blob to its exact universe.
pub(super) fn encode_source_proof_candidate_ranks_v1(
    ranks: &[u128],
    universe_case_count: u128,
) -> Result<Vec<u8>, SourceProofExactAdapterError> {
    validate_candidate_ranks(ranks, universe_case_count)?;
    let encoded_len = CANDIDATE_RANK_MAGIC_V1
        .len()
        .checked_add(4)
        .and_then(|prefix| prefix.checked_add(ranks.len().checked_mul(16)?))
        .ok_or_else(|| {
            SourceProofExactAdapterError::invalid("candidate-rank blob length exceeds usize")
        })?;
    if encoded_len > MAX_CANDIDATE_RANK_BYTES_V1 {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate-rank blob requires {encoded_len} bytes; limit is {MAX_CANDIDATE_RANK_BYTES_V1}"
        )));
    }
    let count = u32::try_from(ranks.len())
        .map_err(|_| SourceProofExactAdapterError::invalid("candidate-rank count exceeds u32"))?;
    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.extend_from_slice(CANDIDATE_RANK_MAGIC_V1);
    bytes.extend_from_slice(&count.to_le_bytes());
    for rank in ranks {
        bytes.extend_from_slice(&rank.to_le_bytes());
    }
    Ok(bytes)
}

/// Decode only the unique canonical v1 candidate-rank representation. Length
/// and count are checked before allocation, and exact re-encoding rejects any
/// alternate or trailing representation.
pub(super) fn decode_source_proof_candidate_ranks_v1(
    bytes: &[u8],
    universe_case_count: u128,
) -> Result<Box<[u128]>, SourceProofExactAdapterError> {
    if bytes.len() > MAX_CANDIDATE_RANK_BYTES_V1 {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate-rank blob has {} bytes; limit is {MAX_CANDIDATE_RANK_BYTES_V1}",
            bytes.len()
        )));
    }
    if bytes.len() < CANDIDATE_RANK_MAGIC_V1.len() + 4
        || &bytes[..CANDIDATE_RANK_MAGIC_V1.len()] != CANDIDATE_RANK_MAGIC_V1
    {
        return Err(SourceProofExactAdapterError::invalid(
            "candidate-rank blob has invalid v1 magic or truncated header",
        ));
    }
    let count_offset = CANDIDATE_RANK_MAGIC_V1.len();
    let count = u32::from_le_bytes(
        bytes[count_offset..count_offset + 4]
            .try_into()
            .expect("candidate header length was checked"),
    ) as usize;
    if count > MAX_CANDIDATE_RANKS_V1 {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate-rank count {count} exceeds limit {MAX_CANDIDATE_RANKS_V1}"
        )));
    }
    let expected_len = count_offset
        .checked_add(4)
        .and_then(|prefix| prefix.checked_add(count.checked_mul(16)?))
        .ok_or_else(|| {
            SourceProofExactAdapterError::invalid("candidate-rank blob length exceeds usize")
        })?;
    if expected_len != bytes.len() {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate-rank blob length {} disagrees with canonical length {expected_len}",
            bytes.len()
        )));
    }
    let mut ranks = Vec::with_capacity(count);
    for chunk in bytes[count_offset + 4..].chunks_exact(16) {
        ranks.push(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("candidate body length is a checked multiple of 16"),
        ));
    }
    validate_candidate_ranks(&ranks, universe_case_count)?;
    let canonical = encode_source_proof_candidate_ranks_v1(&ranks, universe_case_count)?;
    if canonical.as_slice() != bytes {
        return Err(SourceProofExactAdapterError::invalid(
            "candidate-rank bytes are not the canonical v1 encoding",
        ));
    }
    Ok(ranks.into_boxed_slice())
}

fn validate_candidate_ranks(
    ranks: &[u128],
    universe_case_count: u128,
) -> Result<(), SourceProofExactAdapterError> {
    if ranks.len() > MAX_CANDIDATE_RANKS_V1 {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate-rank count {} exceeds limit {MAX_CANDIDATE_RANKS_V1}",
            ranks.len()
        )));
    }
    if let Some(rank) = ranks
        .iter()
        .copied()
        .find(|rank| *rank >= universe_case_count)
    {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "candidate rank {rank} is outside universe {universe_case_count}"
        )));
    }
    if ranks.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SourceProofExactAdapterError::invalid(
            "candidate ranks are not strictly increasing and unique",
        ));
    }
    Ok(())
}

fn derive_output_digest(
    shape: &ExactBoundaryShape,
    region_bytes: Option<&[u8]>,
    candidate_bytes: &[u8],
    summary: SourceProofExactCoverageSummaryV1,
    producer_identity: Option<SourceProofProducerIdentityV1>,
) -> SourceProofExactOutputDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_PROOF_OUTPUT_DIGEST_V1);
    hasher.update((shape.axis_cardinalities.len() as u64).to_be_bytes());
    for cardinality in shape.axis_cardinalities.iter().copied() {
        hasher.update(cardinality.to_be_bytes());
    }
    hasher.update((shape.boundary_dimension as u64).to_be_bytes());
    hasher.update(shape.boundary_start.to_be_bytes());
    hasher.update(shape.boundary_end_exclusive.to_be_bytes());
    hasher.update(shape.boundary_step.to_be_bytes());
    match producer_identity {
        None => hasher.update([0_u8]),
        Some(identity) => {
            hasher.update([1_u8]);
            hasher.update(identity.analysis_program_digest);
            hasher.update(identity.query_digest);
        }
    }
    match region_bytes {
        None => hasher.update([0_u8]),
        Some(bytes) => {
            hasher.update([1_u8]);
            hasher.update((bytes.len() as u64).to_be_bytes());
            hasher.update(bytes);
        }
    }
    hasher.update((candidate_bytes.len() as u64).to_be_bytes());
    hasher.update(candidate_bytes);
    for count in [
        summary.universe_case_count,
        summary.boundary_rank_stride,
        summary.sealed_proof_nonmatch_cases,
        summary.open_proof_nonmatch_cases,
        summary.open_proof_match_cases,
        summary.sealed_structural_excluded_cases,
        summary.open_structural_excluded_cases,
    ] {
        hasher.update(count.to_be_bytes());
    }
    hasher.update([u8::from(summary.region_limit_reached)]);
    hasher.update([u8::from(summary.candidate_limit_reached)]);
    SourceProofExactOutputDigestV1(hasher.finalize().into())
}

fn dense_boundary_ordinal(start: i64, value: i64) -> Result<u128, SourceProofExactAdapterError> {
    i128::from(value)
        .checked_sub(i128::from(start))
        .and_then(|ordinal| u128::try_from(ordinal).ok())
        .ok_or_else(|| {
            SourceProofExactAdapterError::invalid(format!(
                "boundary value {value} has no dense ordinal from {start}"
            ))
        })
}

fn checked_product(
    cardinalities: &[u128],
    name: &str,
) -> Result<u128, SourceProofExactAdapterError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities
        .iter()
        .try_fold(1_u128, |product, cardinality| {
            product.checked_mul(*cardinality).ok_or_else(|| {
                SourceProofExactAdapterError::invalid(format!("{name} exceeds u128"))
            })
        })
}

fn checked_add(left: u128, right: u128, name: &str) -> Result<u128, SourceProofExactAdapterError> {
    left.checked_add(right)
        .ok_or_else(|| SourceProofExactAdapterError::invalid(format!("{name} exceeds u128")))
}

fn parse_lowercase_sha256(
    value: &str,
    name: &str,
) -> Result<[u8; 32], SourceProofExactAdapterError> {
    let bytes = value.as_bytes();
    if bytes.len() != 64
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte))
    {
        return Err(SourceProofExactAdapterError::invalid(format!(
            "{name} is not a lowercase SHA-256"
        )));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(digest)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("lowercase SHA-256 was validated before decoding"),
    }
}

fn structural_receipt(
    shape: &ExactBoundaryShape,
    outer_ordinals: Option<&[u128]>,
    start_rank: u128,
    end_rank_exclusive: u128,
) -> ExactValidationReceiptDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(STRUCTURAL_SUFFIX_RECEIPT_V1);
    hasher.update((shape.axis_cardinalities.len() as u64).to_be_bytes());
    for cardinality in shape.axis_cardinalities.iter().copied() {
        hasher.update(cardinality.to_be_bytes());
    }
    hasher.update((shape.boundary_dimension as u64).to_be_bytes());
    hasher.update(shape.boundary_start.to_be_bytes());
    hasher.update(shape.boundary_end_exclusive.to_be_bytes());
    hasher.update(shape.boundary_step.to_be_bytes());
    match outer_ordinals {
        None => hasher.update([0_u8]),
        Some(ordinals) => {
            hasher.update([1_u8]);
            hasher.update((ordinals.len() as u64).to_be_bytes());
            for ordinal in ordinals {
                hasher.update(ordinal.to_be_bytes());
            }
        }
    }
    hasher.update(start_rank.to_be_bytes());
    hasher.update(end_rank_exclusive.to_be_bytes());
    ExactValidationReceiptDigestV1::new(hasher.finalize().into())
}

struct ClosedRankIndex(BTreeMap<u128, u128>);

impl ClosedRankIndex {
    fn from_proposal(proposal: Option<&ExactClosedRegionBatchProposalV1>) -> Self {
        let intervals = proposal
            .into_iter()
            .flat_map(|proposal| proposal.regions.iter())
            .map(|region| (region.start_rank, region.end_rank_exclusive))
            .collect();
        Self(intervals)
    }

    fn contains(&self, rank: u128) -> bool {
        self.0
            .range(..=rank)
            .next_back()
            .is_some_and(|(_, end_exclusive)| rank < *end_exclusive)
    }
}

struct OuterOrdinalCursor {
    cardinalities: Box<[u128]>,
    next: Option<Vec<u128>>,
}

impl OuterOrdinalCursor {
    fn new(cardinalities: Box<[u128]>) -> Self {
        let next = (!cardinalities.contains(&0)).then(|| vec![0; cardinalities.len()]);
        Self {
            cardinalities,
            next,
        }
    }
}

impl Iterator for OuterOrdinalCursor {
    type Item = Vec<u128>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next.clone()?;
        if self.cardinalities.is_empty() {
            self.next = None;
            return Some(current);
        }
        let mut following = current.clone();
        for index in (0..following.len()).rev() {
            following[index] += 1;
            if following[index] < self.cardinalities[index] {
                self.next = Some(following);
                return Some(current);
            }
            following[index] = 0;
        }
        self.next = None;
        Some(current)
    }
}
