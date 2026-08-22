//! Bounded preparation of source candidates and proof-certified boundary cells.
//!
//! This pass is an optional optimization over an already checked query.  It
//! never changes the semantic universe: profiles beyond the preparation cap,
//! incomplete adapter fragments, and unproved intervals remain ordinary open
//! scheduler work.

use std::fmt;
use std::num::NonZeroUsize;

use super::classification_regions::{
    certify_profile_classification_regions, ClassificationRegionError, ClassificationRegionProof,
    SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1,
};
use super::source_events::{
    extract_source_event_candidates, PreparedResolvedEventAdapter, SourceEventExtraction,
    SourceEventExtractionRequest, SOURCE_PROOF_ADAPTER_LIMITS_V1,
    SOURCE_PROOF_EXTRACTION_OPTIONS_V1,
};
use super::{ExploreExactDomain, ExploreQueryIr};
use crate::TypeCheckArtifacts;

/// Hard first-generation outer-profile cap for one atomic source-proof phase.
///
/// All per-profile adapter, extraction, and certification budgets are sized
/// against this cap. Profiles outside the deterministic prefix remain open
/// canonical exact work; broader probe traversal requires a future resumable
/// probe cursor rather than increasing this atomic budget.
pub(super) const DEFAULT_SOURCE_PROOF_PROFILE_LIMIT: NonZeroUsize = NonZeroUsize::new(64).unwrap();

#[derive(Debug)]
pub(super) struct SourceProofPlan {
    extractions: Box<[SourceEventExtraction]>,
    proofs: Box<[ClassificationRegionProof]>,
    total_outer_profiles: u128,
    analyzed_outer_profiles: u128,
    proof_incomplete_profiles: u128,
    profile_limit_reached: bool,
}

impl SourceProofPlan {
    pub(super) fn extractions(&self) -> &[SourceEventExtraction] {
        &self.extractions
    }

    pub(super) fn proofs(&self) -> &[ClassificationRegionProof] {
        &self.proofs
    }

    pub(super) const fn total_outer_profiles(&self) -> u128 {
        self.total_outer_profiles
    }

    pub(super) const fn analyzed_outer_profiles(&self) -> u128 {
        self.analyzed_outer_profiles
    }

    pub(super) const fn proof_incomplete_profiles(&self) -> u128 {
        self.proof_incomplete_profiles
    }

    pub(super) const fn profile_limit_reached(&self) -> bool {
        self.profile_limit_reached
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SourceProofPlanError {
    SchedulerUnavailable(String),
    Preparation(String),
    OuterProfileCardinalityExceedsU128,
    Adaptation {
        outer_ordinals: Box<[u128]>,
        detail: String,
    },
    Extraction {
        outer_ordinals: Box<[u128]>,
        detail: String,
    },
    Certification {
        outer_ordinals: Box<[u128]>,
        detail: String,
    },
    CounterOverflow,
}

impl fmt::Display for SourceProofPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchedulerUnavailable(detail) => {
                write!(formatter, "source-proof scheduler is unavailable: {detail}")
            }
            Self::Preparation(detail) => {
                write!(formatter, "source-proof preparation failed: {detail}")
            }
            Self::OuterProfileCardinalityExceedsU128 => {
                formatter.write_str("source-proof outer-profile count exceeds u128::MAX")
            }
            Self::Adaptation {
                outer_ordinals,
                detail,
            } => write!(
                formatter,
                "source-proof adaptation failed for outer profile {outer_ordinals:?}: {detail}"
            ),
            Self::Extraction {
                outer_ordinals,
                detail,
            } => write!(
                formatter,
                "source-event extraction failed for outer profile {outer_ordinals:?}: {detail}"
            ),
            Self::Certification {
                outer_ordinals,
                detail,
            } => write!(
                formatter,
                "classification certification failed for outer profile {outer_ordinals:?}: {detail}"
            ),
            Self::CounterOverflow => {
                formatter.write_str("source-proof profile accounting exceeds u128::MAX")
            }
        }
    }
}

impl std::error::Error for SourceProofPlanError {}

impl SourceProofPlanError {
    /// Optional analysis failures leave the checked finite universe untouched
    /// and may select canonical exact evaluation. Proof-production and
    /// accounting failures are integrity failures and must remain fail-closed.
    pub(super) fn permits_canonical_fallback(&self) -> bool {
        matches!(
            self,
            Self::SchedulerUnavailable(_)
                | Self::Preparation(_)
                | Self::OuterProfileCardinalityExceedsU128
                | Self::Adaptation { .. }
        )
    }
}

/// Prepare a deterministic prefix of outer profiles. A successful plan may be
/// incomplete; only its explicit certificates may close scheduler support.
pub(super) fn prepare_source_proof_plan(
    artifacts: &TypeCheckArtifacts,
    accepted_query_index: usize,
    profile_limit: NonZeroUsize,
) -> Result<SourceProofPlan, SourceProofPlanError> {
    // A caller-specific cap would produce a different probe plan under the
    // same durable run identity. Keep this argument strict until the
    // probe phase itself gains a resumable, identity-bound cursor.
    if profile_limit != DEFAULT_SOURCE_PROOF_PROFILE_LIMIT {
        return Err(SourceProofPlanError::Preparation(format!(
            "source-proof profile limit {} disagrees with identity-bound v1 limit {}",
            profile_limit, DEFAULT_SOURCE_PROOF_PROFILE_LIMIT
        )));
    }

    // Reject scheduler shapes before building the checked adapter. This is an
    // optimization-availability decision only; canonical exact execution
    // still owns the complete query universe.
    let query = artifacts
        .exploration_universes
        .get(accepted_query_index)
        .ok_or_else(|| {
            SourceProofPlanError::Preparation(
                "selected checked-query index is outside the exploration universe".to_string(),
            )
        })?;
    let boundary = query.boundary_hint().ok_or_else(|| {
        SourceProofPlanError::SchedulerUnavailable(
            "selected query has no boundary axis".to_string(),
        )
    })?;
    let dimension = query
        .universe
        .dimensions
        .get(boundary.axis_dimension_index)
        .ok_or_else(|| {
            SourceProofPlanError::Preparation(
                "selected query boundary dimension is outside its universe".to_string(),
            )
        })?;
    if !matches!(&dimension.domain, ExploreExactDomain::IntRange { .. }) {
        return Err(SourceProofPlanError::SchedulerUnavailable(
            "source-candidate scheduling currently requires a dense Int range boundary axis"
                .to_string(),
        ));
    }

    let prepared = PreparedResolvedEventAdapter::prepare(
        artifacts,
        accepted_query_index,
        SOURCE_PROOF_ADAPTER_LIMITS_V1,
    )
    .map_err(|error| SourceProofPlanError::Preparation(error.to_string()))?;
    let query = prepared.checked_query();
    let boundary_dimension = query
        .boundary_hint()
        .ok_or_else(|| {
            SourceProofPlanError::Preparation("selected checked query has no boundary".to_string())
        })?
        .axis_dimension_index;
    let outer_cardinalities = outer_profile_cardinalities(query, boundary_dimension)?;
    let total_outer_profiles = product(&outer_cardinalities)?;
    let mut cursor = OuterProfileCursor::new(outer_cardinalities.into_boxed_slice());
    let mut extractions = Vec::new();
    let mut proofs = Vec::new();
    let mut analyzed_outer_profiles = 0_u128;
    let mut proof_incomplete_profiles = 0_u128;

    let profile_limit = DEFAULT_SOURCE_PROOF_PROFILE_LIMIT.get();
    for outer_ordinals in cursor.by_ref().take(profile_limit) {
        let adapted = prepared
            .adapt_profile(
                prepared.analysis_program_hash(),
                prepared.query_hash(),
                &outer_ordinals,
            )
            .map_err(|error| SourceProofPlanError::Adaptation {
                outer_ordinals: outer_ordinals.clone().into_boxed_slice(),
                detail: error.to_string(),
            })?;
        let extraction = extract_source_event_candidates(SourceEventExtractionRequest {
            query,
            analysis_program_hash: prepared.analysis_program_hash(),
            query_hash: prepared.query_hash(),
            outer_ordinals: &outer_ordinals,
            fragment: &adapted.fragment,
            options: SOURCE_PROOF_EXTRACTION_OPTIONS_V1,
        })
        .map_err(|error| SourceProofPlanError::Extraction {
            outer_ordinals: outer_ordinals.clone().into_boxed_slice(),
            detail: error.to_string(),
        })?;

        match certify_profile_classification_regions(
            &prepared,
            &extraction,
            &adapted.fragment,
            SOURCE_PROOF_CLASSIFICATION_OPTIONS_V1,
        ) {
            Ok(proof) => {
                proof
                    .validate_certificates(&prepared, &extraction, &adapted.fragment)
                    .map_err(|error| SourceProofPlanError::Certification {
                        outer_ordinals: outer_ordinals.clone().into_boxed_slice(),
                        detail: error.to_string(),
                    })?;
                if !proof.is_complete() {
                    proof_incomplete_profiles = proof_incomplete_profiles
                        .checked_add(1)
                        .ok_or(SourceProofPlanError::CounterOverflow)?;
                }
                proofs.push(proof);
            }
            Err(
                ClassificationRegionError::RuntimeConstraintsRequireSeparateProof
                | ClassificationRegionError::ExtractionIncomplete
                | ClassificationRegionError::FragmentIncomplete
                | ClassificationRegionError::UnsupportedBoundaryDomain,
            ) => {
                proof_incomplete_profiles = proof_incomplete_profiles
                    .checked_add(1)
                    .ok_or(SourceProofPlanError::CounterOverflow)?;
            }
            Err(error) => {
                return Err(SourceProofPlanError::Certification {
                    outer_ordinals: outer_ordinals.clone().into_boxed_slice(),
                    detail: error.to_string(),
                })
            }
        }
        extractions.push(extraction);
        analyzed_outer_profiles = analyzed_outer_profiles
            .checked_add(1)
            .ok_or(SourceProofPlanError::CounterOverflow)?;
    }

    Ok(SourceProofPlan {
        extractions: extractions.into_boxed_slice(),
        proofs: proofs.into_boxed_slice(),
        total_outer_profiles,
        analyzed_outer_profiles,
        proof_incomplete_profiles,
        profile_limit_reached: analyzed_outer_profiles < total_outer_profiles,
    })
}

fn outer_profile_cardinalities(
    query: &ExploreQueryIr,
    boundary_dimension: usize,
) -> Result<Vec<u128>, SourceProofPlanError> {
    let cardinalities = query
        .universe
        .dimensions
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != boundary_dimension)
        .map(|(_, dimension)| dimension.domain.cardinality())
        .collect::<Vec<_>>();
    let contains_exact_zero = cardinalities
        .iter()
        .any(|cardinality| cardinality.exact() == Some(0));
    cardinalities
        .into_iter()
        .map(|cardinality| match cardinality.exact() {
            Some(cardinality) => Ok(cardinality),
            // An exact zero annihilates the product. The placeholder is never
            // observed because the cursor sees that zero and yields nothing.
            None if contains_exact_zero => Ok(1),
            None => Err(SourceProofPlanError::OuterProfileCardinalityExceedsU128),
        })
        .collect()
}

fn product(cardinalities: &[u128]) -> Result<u128, SourceProofPlanError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities
        .iter()
        .try_fold(1_u128, |product, cardinality| {
            product
                .checked_mul(*cardinality)
                .ok_or(SourceProofPlanError::OuterProfileCardinalityExceedsU128)
        })
}

struct OuterProfileCursor {
    cardinalities: Box<[u128]>,
    next: Option<Vec<u128>>,
}

impl OuterProfileCursor {
    fn new(cardinalities: Box<[u128]>) -> Self {
        let next = (!cardinalities.contains(&0)).then(|| vec![0; cardinalities.len()]);
        Self {
            cardinalities,
            next,
        }
    }
}

impl Iterator for OuterProfileCursor {
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
