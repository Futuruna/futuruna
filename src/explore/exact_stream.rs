//! Durable exact-case semantic records and their arrival-order-independent
//! replay reducer.
//!
//! This module deliberately does not depend on the one-shot exact executor's
//! runtime or accumulator types. Workers may propose records in any order. A
//! coordinator persists their validation receipt, crosses the private semantic
//! validation boundary, and only then feeds sealed evidence to
//! [`ExactEvidenceReducer`]. The reducer's snapshot is a pure function of that
//! accepted evidence set.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{
    run_stream::{CanonicalDigest, ExactCaseSupport, ExploreCaseUniverse},
    ExploreValue,
};

const OBSERVATION_MAGIC_V1: &[u8; 8] = b"FXOBS001";
const OBSERVATION_BATCH_MAGIC_V1: &[u8; 8] = b"FXOBB001";
const CLOSED_REGION_BATCH_MAGIC_V1: &[u8; 8] = b"FXREG001";
const SEMANTIC_FACT_DIGEST_V1: &[u8] = b"futuruna.explore.exact-semantic-fact.v1";

// These are wire-safety limits, not exploration limits. A coordinator may
// choose smaller chunks while every individual semantic record stays bounded.
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_VALUE_BYTES: usize = 8 * 1024 * 1024;
const MAX_VALUE_NODES: usize = 100_000;
const MAX_VALUE_DEPTH: usize = 32;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_SEQUENCE_ITEMS: usize = 65_536;
const MAX_AXES: usize = 4_096;
const MAX_PROJECTION_FIELDS: usize = 65_536;
const MAX_OBSERVATIONS_PER_BATCH: usize = 65_536;
const MAX_REGIONS_PER_BATCH: usize = 65_536;

/// Identity-bound limits for the cursor-bearing observable result preview.
///
/// The exact reducer retains every group. A pause snapshot only clones a
/// canonical raw-key prefix whose aggregate payload fits all three limits, so
/// observing or replay-verifying a large run never becomes an O(all groups)
/// operation. Full terminal publication deliberately remains a separate
/// frontier.
pub(crate) const EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1: usize = 256;
pub(crate) const EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1: usize = 16_384;
pub(crate) const EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1: usize = 4 * 1024 * 1024;
pub(crate) const EXACT_OBSERVABLE_RESULT_PREVIEW_JSON_BYTE_LIMIT_V1: usize = 8 * 1024 * 1024;

/// A rejected semantic record or malformed canonical wire payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactStreamError(Box<str>);

impl ExactStreamError {
    fn invalid(message: impl Into<String>) -> Self {
        Self(message.into().into_boxed_str())
    }
}

impl fmt::Display for ExactStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ExactStreamError {}

/// The three terminal classifications accepted as closed case evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExactClosedClassificationV1 {
    Excluded,
    AdmissibleNonmatch,
    AdmissibleMatch,
}

/// Whether a closed interval came from a semantic proof or from structure
/// whose classification requires no per-case evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExactClosedRegionKindV1 {
    Proof,
    Structural,
}

/// Digest of normalized answer content, never of rank, support, worker, proof,
/// or arrival metadata. Only the private validation boundary may mint one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExactSemanticFactDigestV1([u8; 32]);

impl ExactSemanticFactDigestV1 {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Ordered-journal provenance for an evaluator or proof validation receipt.
/// This digest is deliberately not part of normalized semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExactValidationReceiptDigestV1([u8; 32]);

impl ExactValidationReceiptDigestV1 {
    pub(crate) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identity of one case in a mixed-radix universe.
///
/// Both forms are retained intentionally. Replay verifies that `rank` and
/// `ordinals` identify the same case, making accidental cursor/schema drift a
/// hard error instead of silently moving evidence to another configuration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExactCanonicalCaseIdV1 {
    pub(crate) rank: u128,
    pub(crate) ordinals: Box<[u128]>,
}

impl ExactCanonicalCaseIdV1 {
    pub(crate) fn new(rank: u128, ordinals: impl Into<Box<[u128]>>) -> Self {
        Self {
            rank,
            ordinals: ordinals.into(),
        }
    }
}

/// Every report value required from one matching case.
///
/// There is no projection-only fast path in the durable format: a matching
/// singleton is atomic only after its key, every extrema value, shown values,
/// and (for ordered representative policies) objective are all available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactMatchProjectionV1 {
    pub(crate) key: Box<[ExploreValue]>,
    pub(crate) extrema: Box<[i64]>,
    pub(crate) shown: Box<[ExploreValue]>,
    pub(crate) representative_objective: Option<i64>,
}

impl ExactMatchProjectionV1 {
    pub(crate) fn new(
        key: impl Into<Box<[ExploreValue]>>,
        extrema: impl Into<Box<[i64]>>,
        shown: impl Into<Box<[ExploreValue]>>,
        representative_objective: Option<i64>,
    ) -> Result<Self, ExactStreamError> {
        let projection = Self {
            key: key.into(),
            extrema: extrema.into(),
            shown: shown.into(),
            representative_objective,
        };
        validate_projection_wire(&projection)?;
        Ok(projection)
    }
}

/// Decoded, untrusted proposal for one fully evaluated singleton case.
///
/// The receipt is producer provenance. It is persisted in the ordered journal,
/// but excluded from the normalized semantic digest and EvidenceRoot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactCaseObservationProposalV1 {
    pub(crate) case_id: ExactCanonicalCaseIdV1,
    pub(crate) classification: ExactClosedClassificationV1,
    pub(crate) match_projection: Option<ExactMatchProjectionV1>,
    pub(crate) validation_receipt_digest: ExactValidationReceiptDigestV1,
}

impl ExactCaseObservationProposalV1 {
    pub(crate) fn new(
        case_id: ExactCanonicalCaseIdV1,
        classification: ExactClosedClassificationV1,
        match_projection: Option<ExactMatchProjectionV1>,
        validation_receipt_digest: ExactValidationReceiptDigestV1,
    ) -> Result<Self, ExactStreamError> {
        let observation = Self {
            case_id,
            classification,
            match_projection,
            validation_receipt_digest,
        };
        validate_observation_wire(&observation)?;
        Ok(observation)
    }
}

/// Evaluator-confirmed singleton evidence. Its private fields make it
/// impossible for a decoder or worker proposal to enter the reducer directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedExactCaseObservationV1 {
    proposal: ExactCaseObservationProposalV1,
    semantic_fact_digest: ExactSemanticFactDigestV1,
}

impl ValidatedExactCaseObservationV1 {
    pub(crate) fn proposal(&self) -> &ExactCaseObservationProposalV1 {
        &self.proposal
    }

    pub(crate) const fn semantic_fact_digest(&self) -> ExactSemanticFactDigestV1 {
        self.semantic_fact_digest
    }
}

/// Nonempty, canonically rank-sorted proposal batch. Duplicate ranks are
/// rejected before any evaluator-confirmed wrapper can be minted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactCaseObservationBatchProposalV1 {
    pub(crate) observations: Box<[ExactCaseObservationProposalV1]>,
}

impl ExactCaseObservationBatchProposalV1 {
    pub(crate) fn new(
        observations: impl Into<Box<[ExactCaseObservationProposalV1]>>,
    ) -> Result<Self, ExactStreamError> {
        let mut observations = observations.into().into_vec();
        validate_nonempty_batch_len(
            "exact observation",
            observations.len(),
            MAX_OBSERVATIONS_PER_BATCH,
        )?;
        observations.sort_by(|left, right| left.case_id.rank.cmp(&right.case_id.rank));
        validate_canonical_observation_sequence(&observations)?;
        Ok(Self {
            observations: observations.into_boxed_slice(),
        })
    }
}

/// Evaluator-confirmed, atomic observation slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedExactCaseObservationBatchV1 {
    observations: Box<[ValidatedExactCaseObservationV1]>,
}

impl ValidatedExactCaseObservationBatchV1 {
    pub(crate) fn observations(&self) -> &[ValidatedExactCaseObservationV1] {
        &self.observations
    }
}

/// One decoded, untrusted half-open rank interval proposed by a proof or
/// structural validator.
///
/// `AdmissibleMatch` is decodable as an untrusted proposal, but cannot be
/// sealed in v1: matching ranks remain singleton work until their complete
/// projection and optional retained ledger row have been replayed. Adjacent
/// equal semantic regions coalesce and retain the canonical union of their
/// separate validation-receipt digests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExactClosedRankRegionProposalV1 {
    pub(crate) start_rank: u128,
    pub(crate) end_rank_exclusive: u128,
    pub(crate) kind: ExactClosedRegionKindV1,
    pub(crate) classification: ExactClosedClassificationV1,
    pub(crate) validation_receipt_digests: Box<[ExactValidationReceiptDigestV1]>,
}

impl ExactClosedRankRegionProposalV1 {
    pub(crate) fn new(
        start_rank: u128,
        end_rank_exclusive: u128,
        kind: ExactClosedRegionKindV1,
        classification: ExactClosedClassificationV1,
        validation_receipt_digest: ExactValidationReceiptDigestV1,
    ) -> Result<Self, ExactStreamError> {
        Self::from_receipts(
            start_rank,
            end_rank_exclusive,
            kind,
            classification,
            vec![validation_receipt_digest],
        )
    }

    fn from_receipts(
        start_rank: u128,
        end_rank_exclusive: u128,
        kind: ExactClosedRegionKindV1,
        classification: ExactClosedClassificationV1,
        validation_receipt_digests: impl Into<Box<[ExactValidationReceiptDigestV1]>>,
    ) -> Result<Self, ExactStreamError> {
        if start_rank >= end_rank_exclusive {
            return Err(ExactStreamError::invalid(format!(
                "closed rank region [{start_rank}, {end_rank_exclusive}) is empty or reversed"
            )));
        }
        let mut validation_receipt_digests = validation_receipt_digests.into().into_vec();
        validate_nonempty_batch_len(
            "closed-region validation receipt",
            validation_receipt_digests.len(),
            MAX_REGIONS_PER_BATCH,
        )?;
        validation_receipt_digests.sort_unstable();
        validation_receipt_digests.dedup();
        Ok(Self {
            start_rank,
            end_rank_exclusive,
            kind,
            classification,
            validation_receipt_digests: validation_receipt_digests.into_boxed_slice(),
        })
    }

    pub(crate) fn case_count(&self) -> u128 {
        self.end_rank_exclusive - self.start_rank
    }
}

/// Proof/structure-confirmed closed region. Matching proof regions are not
/// sealable in v1 because they still require singleton projection replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedExactClosedRankRegionV1 {
    proposal: ExactClosedRankRegionProposalV1,
    semantic_fact_digest: ExactSemanticFactDigestV1,
}

impl ValidatedExactClosedRankRegionV1 {
    pub(crate) fn proposal(&self) -> &ExactClosedRankRegionProposalV1 {
        &self.proposal
    }

    pub(crate) const fn semantic_fact_digest(&self) -> ExactSemanticFactDigestV1 {
        self.semantic_fact_digest
    }
}

/// Canonically rank-sorted, internally disjoint closed-region evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactClosedRegionBatchProposalV1 {
    pub(crate) regions: Box<[ExactClosedRankRegionProposalV1]>,
}

impl ExactClosedRegionBatchProposalV1 {
    pub(crate) fn new(
        regions: impl Into<Box<[ExactClosedRankRegionProposalV1]>>,
    ) -> Result<Self, ExactStreamError> {
        let mut regions = regions.into().into_vec();
        validate_nonempty_batch_len("closed-region", regions.len(), MAX_REGIONS_PER_BATCH)?;
        regions.sort_unstable();
        validate_canonical_region_sequence(&regions)?;
        regions = coalesce_adjacent_regions(regions)?;
        validate_normalized_region_sequence(&regions)?;
        Ok(Self {
            regions: regions.into_boxed_slice(),
        })
    }
}

/// Validator-confirmed atomic region slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedExactClosedRegionBatchV1 {
    regions: Box<[ValidatedExactClosedRankRegionV1]>,
}

impl ValidatedExactClosedRegionBatchV1 {
    pub(crate) fn regions(&self) -> &[ValidatedExactClosedRankRegionV1] {
        &self.regions
    }
}

/// Shape of the complete projection bound into the run's report contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactProjectionShapeV1 {
    pub(crate) key_width: usize,
    pub(crate) extrema_width: usize,
    pub(crate) shown_width: usize,
}

impl ExactProjectionShapeV1 {
    pub(crate) fn new(
        key_width: usize,
        extrema_width: usize,
        shown_width: usize,
    ) -> Result<Self, ExactStreamError> {
        for (name, width) in [
            ("key", key_width),
            ("extrema", extrema_width),
            ("shown", shown_width),
        ] {
            if width > MAX_PROJECTION_FIELDS {
                return Err(ExactStreamError::invalid(format!(
                    "{name} projection width {width} exceeds limit {MAX_PROJECTION_FIELDS}"
                )));
            }
        }
        Ok(Self {
            key_width,
            extrema_width,
            shown_width,
        })
    }
}

/// Deterministic representative-selection policy from the checked report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactRepresentativePolicyV1 {
    First,
    Maximize,
    Minimize,
}

/// A monotone count: always an honest lower bound, and exact only after its
/// required frontier is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactCountBoundV1 {
    pub(crate) lower_bound: u128,
    pub(crate) exact: Option<u128>,
}

impl ExactCountBoundV1 {
    fn new(lower_bound: u128, closed: bool) -> Self {
        Self {
            lower_bound,
            exact: closed.then_some(lower_bound),
        }
    }
}

/// Deterministic extrema accumulated for one result group and measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactExtremaAggregateV1 {
    pub(crate) minimum: i64,
    pub(crate) maximum: i64,
    pub(crate) spread: u128,
    pub(crate) observed_support: u128,
    pub(crate) minimum_tie_support: u128,
    pub(crate) maximum_tie_support: u128,
    pub(crate) minimum_witness: ExactCanonicalCaseIdV1,
    pub(crate) maximum_witness: ExactCanonicalCaseIdV1,
    /// False means these endpoints are observations, not closed extrema.
    pub(crate) closed: bool,
}

/// One canonical key group reduced from complete matching observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactResultAggregateV1 {
    pub(crate) key: Box<[ExploreValue]>,
    pub(crate) support: ExactCountBoundV1,
    pub(crate) extrema: Box<[ExactExtremaAggregateV1]>,
    pub(crate) representative_case_id: ExactCanonicalCaseIdV1,
    pub(crate) representative_shown: Box<[ExploreValue]>,
    pub(crate) representative_objective: Option<i64>,
    /// A provisional candidate is still deterministic, but only a closed
    /// selection may be rendered as the final representative.
    pub(crate) representative_selection_closed: bool,
}

/// Optional authorized lossless ledger of matching singleton observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactMatchingLedgerRowV1 {
    pub(crate) case_id: ExactCanonicalCaseIdV1,
    pub(crate) match_projection: ExactMatchProjectionV1,
    pub(crate) semantic_fact_digest: ExactSemanticFactDigestV1,
}

/// Optional authorized lossless semantic ledger. Validation receipts remain in
/// the ordered journal and deliberately do not enter this answer snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactMatchingLedgerSnapshotV1 {
    pub(crate) observations: Box<[ExactMatchingLedgerRowV1]>,
    pub(crate) complete: bool,
}

/// Arrival-order-independent replay snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactEvidenceSnapshotV1 {
    pub(crate) universe_case_count: u128,
    pub(crate) closed_case_count: u128,
    pub(crate) open_case_count: u128,
    pub(crate) excluded: ExactCountBoundV1,
    pub(crate) admissible: ExactCountBoundV1,
    pub(crate) matching: ExactCountBoundV1,
    /// Matches represented by complete singleton projections.
    pub(crate) projected_matching_case_count: u128,
    /// Reserved for a future two-frontier proof-match format; v1 keeps this at
    /// zero because matching regions cannot be sealed.
    pub(crate) unprojected_matching_case_count: u128,
    pub(crate) projection_complete: bool,
    /// Number of raw result groups retained by the reducer at this cursor.
    /// This is an honest lower bound while projection is open and exact after
    /// projection closure, independently of how many rows are disclosed.
    pub(crate) observed_result_group_count: u128,
    /// True only when `results` contains every observed raw group. Observable
    /// pause snapshots may carry a bounded canonical prefix instead.
    pub(crate) result_group_scan_complete: bool,
    pub(crate) results: Box<[ExactResultAggregateV1]>,
    pub(crate) matching_ledger: Option<ExactMatchingLedgerSnapshotV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutableExtrema {
    minimum: i64,
    maximum: i64,
    support: u128,
    minimum_tie_support: u128,
    maximum_tie_support: u128,
    minimum_witness: ExactCanonicalCaseIdV1,
    maximum_witness: ExactCanonicalCaseIdV1,
}

impl MutableExtrema {
    fn first(value: i64, witness: &ExactCanonicalCaseIdV1) -> Self {
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

    fn observe(&mut self, value: i64, witness: &ExactCanonicalCaseIdV1) {
        self.support = self
            .support
            .checked_add(1)
            .expect("disjoint matching support cannot exceed the u128 universe");
        if value < self.minimum {
            self.minimum = value;
            self.minimum_tie_support = 1;
            self.minimum_witness = witness.clone();
        } else if value == self.minimum {
            self.minimum_tie_support = self
                .minimum_tie_support
                .checked_add(1)
                .expect("minimum tie support cannot exceed matching support");
            if witness.rank < self.minimum_witness.rank {
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
                .expect("maximum tie support cannot exceed matching support");
            if witness.rank < self.maximum_witness.rank {
                self.maximum_witness = witness.clone();
            }
        }
    }

    fn snapshot(&self, closed: bool) -> ExactExtremaAggregateV1 {
        ExactExtremaAggregateV1 {
            minimum: self.minimum,
            maximum: self.maximum,
            spread: (self.maximum as i128 - self.minimum as i128) as u128,
            observed_support: self.support,
            minimum_tie_support: self.minimum_tie_support,
            maximum_tie_support: self.maximum_tie_support,
            minimum_witness: self.minimum_witness.clone(),
            maximum_witness: self.maximum_witness.clone(),
            closed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepresentativeCandidate {
    case_id: ExactCanonicalCaseIdV1,
    shown: Box<[ExploreValue]>,
    objective: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutableResultGroup {
    support: u128,
    extrema: Box<[MutableExtrema]>,
    representative: RepresentativeCandidate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ObservableResultCostV1 {
    value_nodes: usize,
    semantic_bytes: usize,
}

/// Persistent exact rank supports for the closed case-classification
/// partition.
///
/// `closed` is retained separately because it is the hot duplicate-detection
/// support. The three classified supports are extended from the same bounded
/// delta, making their disjoint union equal to `closed` by construction. A
/// later case-DAG lowerer may obtain a bounded clone of these authenticated
/// roots without retaining or copying a case ledger.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactClosedClassificationSupportsV1 {
    closed: ExactCaseSupport,
    excluded: ExactCaseSupport,
    admissible_nonmatch: ExactCaseSupport,
    admissible_match: ExactCaseSupport,
}

#[allow(dead_code)]
impl ExactClosedClassificationSupportsV1 {
    fn empty(universe: &ExploreCaseUniverse) -> Self {
        Self {
            closed: ExactCaseSupport::empty(universe),
            excluded: ExactCaseSupport::empty(universe),
            admissible_nonmatch: ExactCaseSupport::empty(universe),
            admissible_match: ExactCaseSupport::empty(universe),
        }
    }

    fn merge_delta(
        &self,
        universe: &ExploreCaseUniverse,
        excluded: Vec<(u128, u128)>,
        admissible_nonmatch: Vec<(u128, u128)>,
        admissible_match: Vec<(u128, u128)>,
    ) -> Result<Self, ExactStreamError> {
        let excluded_delta = exact_case_support(universe, "excluded delta", excluded)?;
        let admissible_nonmatch_delta =
            exact_case_support(universe, "admissible-nonmatch delta", admissible_nonmatch)?;
        let admissible_match_delta =
            exact_case_support(universe, "admissible-match delta", admissible_match)?;

        // The delta union is deliberately constructed from its typed fibers.
        // This rejects conflicting classifications inside one batch before
        // any reducer state changes.
        let classified_delta = merge_exact_case_supports(
            "classification delta",
            &excluded_delta,
            &admissible_nonmatch_delta,
        )?;
        let classified_delta = merge_exact_case_supports(
            "classification delta",
            &classified_delta,
            &admissible_match_delta,
        )?;

        // This one global merge rejects every overlap with earlier evidence,
        // including an attempt to change an already-closed classification.
        let closed = merge_exact_case_supports("closed support", &self.closed, &classified_delta)?;
        let excluded =
            merge_exact_case_supports("excluded support", &self.excluded, &excluded_delta)?;
        let admissible_nonmatch = merge_exact_case_supports(
            "admissible-nonmatch support",
            &self.admissible_nonmatch,
            &admissible_nonmatch_delta,
        )?;
        let admissible_match = merge_exact_case_supports(
            "admissible-match support",
            &self.admissible_match,
            &admissible_match_delta,
        )?;

        Ok(Self {
            closed,
            excluded,
            admissible_nonmatch,
            admissible_match,
        })
    }

    fn validate_scalar_counts(
        &self,
        closed: u128,
        excluded: u128,
        admissible_nonmatch: u128,
        admissible_match: u128,
    ) -> Result<(), ExactStreamError> {
        for (label, support, expected) in [
            ("closed", &self.closed, closed),
            ("excluded", &self.excluded, excluded),
            (
                "admissible nonmatch",
                &self.admissible_nonmatch,
                admissible_nonmatch,
            ),
            ("admissible match", &self.admissible_match, admissible_match),
        ] {
            if support.case_count() != expected {
                return Err(ExactStreamError::invalid(format!(
                    "{label} persistent support count {} disagrees with scalar count {expected}",
                    support.case_count()
                )));
            }
        }
        let classified = excluded
            .checked_add(admissible_nonmatch)
            .and_then(|count| count.checked_add(admissible_match))
            .ok_or_else(|| {
                ExactStreamError::invalid("classified persistent support count exceeds u128::MAX")
            })?;
        if classified != closed {
            return Err(ExactStreamError::invalid(format!(
                "classified persistent support count {classified} disagrees with closed count {closed}"
            )));
        }
        Ok(())
    }

    /// Expensive structural validation, called only after the caller has
    /// bounded the complete interval population.
    fn validate_exact_partition(&self) -> Result<(), ExactStreamError> {
        let mut excluded = self.excluded.iter_intervals().peekable();
        let mut admissible_nonmatch = self.admissible_nonmatch.iter_intervals().peekable();
        let mut admissible_match = self.admissible_match.iter_intervals().peekable();
        let mut closed = self.closed.iter_intervals();
        let mut classified_union = None::<(u128, u128)>;

        loop {
            let source = [
                excluded.peek().map(|interval| (interval.start(), 0_u8)),
                admissible_nonmatch
                    .peek()
                    .map(|interval| (interval.start(), 1_u8)),
                admissible_match
                    .peek()
                    .map(|interval| (interval.start(), 2_u8)),
            ]
            .into_iter()
            .flatten()
            .min();
            let Some((_, source)) = source else {
                break;
            };
            let interval = match source {
                0 => excluded.next(),
                1 => admissible_nonmatch.next(),
                2 => admissible_match.next(),
                _ => unreachable!("classification support source is bounded to three fibers"),
            }
            .expect("peeked classification support interval must still exist");

            match classified_union {
                None => {
                    classified_union = Some((interval.start(), interval.end_exclusive()));
                }
                Some((_, end_exclusive)) if interval.start() < end_exclusive => {
                    return Err(ExactStreamError::invalid(
                        "closed classification supports overlap",
                    ));
                }
                Some((start, end_exclusive)) if interval.start() == end_exclusive => {
                    classified_union = Some((start, interval.end_exclusive()));
                }
                Some(expected) => {
                    let actual = closed
                        .next()
                        .map(|interval| (interval.start(), interval.end_exclusive()));
                    if actual != Some(expected) {
                        return Err(ExactStreamError::invalid(
                            "classified persistent supports do not form the exact closed support",
                        ));
                    }
                    classified_union = Some((interval.start(), interval.end_exclusive()));
                }
            }
        }

        if let Some(expected) = classified_union {
            let actual = closed
                .next()
                .map(|interval| (interval.start(), interval.end_exclusive()));
            if actual != Some(expected) {
                return Err(ExactStreamError::invalid(
                    "classified persistent supports do not form the exact closed support",
                ));
            }
        }
        if closed.next().is_some() {
            return Err(ExactStreamError::invalid(
                "classified persistent supports do not form the exact closed support",
            ));
        }
        Ok(())
    }

    pub(crate) fn closed(&self) -> &ExactCaseSupport {
        &self.closed
    }

    pub(crate) fn support(&self, classification: ExactClosedClassificationV1) -> &ExactCaseSupport {
        match classification {
            ExactClosedClassificationV1::Excluded => &self.excluded,
            ExactClosedClassificationV1::AdmissibleNonmatch => &self.admissible_nonmatch,
            ExactClosedClassificationV1::AdmissibleMatch => &self.admissible_match,
        }
    }

    /// Aggregate interval population across the closed support and its three
    /// typed fibers. This is O(1) and lets a caller reject materialization
    /// before traversing any persistent tree.
    pub(crate) fn total_interval_count(&self) -> Option<usize> {
        self.closed
            .interval_count()
            .checked_add(self.excluded.interval_count())
            .and_then(|count| count.checked_add(self.admissible_nonmatch.interval_count()))
            .and_then(|count| count.checked_add(self.admissible_match.interval_count()))
    }

    pub(crate) fn identity_hashes(&self) -> [CanonicalDigest; 4] {
        [
            self.closed.identity_hash(),
            self.excluded.identity_hash(),
            self.admissible_nonmatch.identity_hash(),
            self.admissible_match.identity_hash(),
        ]
    }
}

fn exact_case_support(
    universe: &ExploreCaseUniverse,
    label: &str,
    intervals: Vec<(u128, u128)>,
) -> Result<ExactCaseSupport, ExactStreamError> {
    ExactCaseSupport::new(universe, intervals).map_err(|error| {
        ExactStreamError::invalid(format!(
            "cannot construct {label} persistent support: {error}"
        ))
    })
}

fn merge_exact_case_supports(
    label: &str,
    left: &ExactCaseSupport,
    right: &ExactCaseSupport,
) -> Result<ExactCaseSupport, ExactStreamError> {
    left.merge_disjoint(right)
        .map_err(|error| ExactStreamError::invalid(format!("cannot extend {label}: {error}")))
}

/// Pure reducer for exact classification and projection evidence.
///
/// The reducer is intentionally not `Clone`: copying the accumulated result
/// map at every durable block would make both forward execution and replay
/// quadratic. Coordinators prevalidate one compact block, durably append its
/// journal event, and then apply the sealed block with no remaining semantic
/// failure path. A process failure between append and apply is recovered by
/// replaying that durable event into a fresh reducer.
pub(crate) struct ExactEvidenceReducer {
    axis_cardinalities: Box<[u128]>,
    case_universe: ExploreCaseUniverse,
    universe_case_count: u128,
    projection_shape: ExactProjectionShapeV1,
    representative_policy: ExactRepresentativePolicyV1,
    classification_supports: ExactClosedClassificationSupportsV1,
    closed_case_count: u128,
    excluded_case_count: u128,
    admissible_nonmatch_case_count: u128,
    matching_case_count: u128,
    projected_matching_case_count: u128,
    unprojected_matching_case_count: u128,
    groups: BTreeMap<Box<[ExploreValue]>, MutableResultGroup>,
    matching_ledger: Option<BTreeMap<u128, ExactMatchingLedgerRowV1>>,
}

/// Non-forgeable proof that the enclosed matching support came from an exact
/// reducer after every case in its declared universe was classified. A bare
/// [`ExactCaseSupport`] is only a set; it cannot establish that the matching
/// complement is closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactClosedMatchSupportV1 {
    support: ExactCaseSupport,
}

impl ExactClosedMatchSupportV1 {
    pub(crate) fn support(&self) -> &ExactCaseSupport {
        &self.support
    }

    pub(crate) fn case_count(&self) -> u128 {
        self.support.case_count()
    }

    #[cfg(test)]
    pub(in crate::explore) fn from_support_for_test(support: ExactCaseSupport) -> Self {
        Self { support }
    }
}

pub(crate) struct PreparedExactClosedRegionBatchV1 {
    prior_closed_case_count: u128,
    prior_classification_supports: ExactClosedClassificationSupportsV1,
    next_classification_supports: ExactClosedClassificationSupportsV1,
    next_closed_case_count: u128,
    next_excluded_case_count: u128,
    next_admissible_nonmatch_case_count: u128,
}

pub(crate) struct PreparedExactObservationBatchV1 {
    batch: ValidatedExactCaseObservationBatchV1,
    prior_closed_case_count: u128,
    prior_classification_supports: ExactClosedClassificationSupportsV1,
    next_classification_supports: ExactClosedClassificationSupportsV1,
    next_closed_case_count: u128,
    next_excluded_case_count: u128,
    next_admissible_nonmatch_case_count: u128,
    next_matching_case_count: u128,
    next_projected_matching_case_count: u128,
}

impl ExactEvidenceReducer {
    pub(crate) fn new(
        axis_cardinalities: impl Into<Box<[u128]>>,
        projection_shape: ExactProjectionShapeV1,
        representative_policy: ExactRepresentativePolicyV1,
        retain_matching_ledger: bool,
    ) -> Result<Self, ExactStreamError> {
        let axis_cardinalities = axis_cardinalities.into();
        if axis_cardinalities.len() > MAX_AXES {
            return Err(ExactStreamError::invalid(format!(
                "case universe has {} axes; limit is {MAX_AXES}",
                axis_cardinalities.len()
            )));
        }
        let case_universe =
            ExploreCaseUniverse::new(axis_cardinalities.clone()).map_err(|error| {
                ExactStreamError::invalid(format!(
                    "cannot construct exact reducer case universe: {error}"
                ))
            })?;
        let universe_case_count = case_universe.case_count();
        let classification_supports = ExactClosedClassificationSupportsV1::empty(&case_universe);
        Ok(Self {
            axis_cardinalities,
            case_universe,
            universe_case_count,
            projection_shape,
            representative_policy,
            classification_supports,
            closed_case_count: 0,
            excluded_case_count: 0,
            admissible_nonmatch_case_count: 0,
            matching_case_count: 0,
            projected_matching_case_count: 0,
            unprojected_matching_case_count: 0,
            groups: BTreeMap::new(),
            matching_ledger: retain_matching_ledger.then(BTreeMap::new),
        })
    }

    pub(crate) fn universe_case_count(&self) -> u128 {
        self.universe_case_count
    }

    /// Cheap progress scalar for hot coordinator paths. Unlike `snapshot`,
    /// this does not clone result groups, witnesses, or the matching ledger.
    pub(crate) fn closed_case_count(&self) -> u128 {
        self.closed_case_count
    }

    /// Return the admissible-match ranks confirmed by evidence accepted so
    /// far, whether or not classification is closed.
    ///
    /// This is a monotone known subset: exact reducer transitions reject
    /// duplicate or conflicting classification, so accepted matching ranks are
    /// never withdrawn. A coordinator may use the subset to authorize early
    /// mechanism replay for those ranks only. It must not treat the subset as
    /// the complete matching target while any case rank remains open.
    ///
    /// `ExactCaseSupport` is persistent, so this clone shares its authenticated
    /// tree and does not enumerate ranks or intervals.
    pub(crate) fn confirmed_admissible_match_support(&self) -> ExactCaseSupport {
        self.debug_assert_classification_support_counts();
        self.classification_supports.admissible_match.clone()
    }

    /// Return the authoritative admissible-match support after the complete
    /// case universe has been classified.
    ///
    /// Before closure, the retained matching fiber is only a lower bound and
    /// must not be installed as a downstream mechanism target. At closure, the
    /// reducer's disjoint typed fibers form the exact `closed` partition, and
    /// overlap rejection prevents any later classification extension. The
    /// resulting content identity is therefore independent of evidence arrival
    /// order. A coordinator must obtain this support after case closure and
    /// install the derived mechanism target before accepting mechanism
    /// observations.
    ///
    /// The returned token is unforgeable outside this reducer module; callers
    /// cannot accidentally promote a known matching subset merely because its
    /// support equals another local set. Its persistent support clone shares
    /// the authenticated tree and does not enumerate ranks or intervals.
    pub(crate) fn authoritative_admissible_match_support(
        &self,
    ) -> Option<ExactClosedMatchSupportV1> {
        if self.closed_case_count != self.universe_case_count {
            return None;
        }
        Some(ExactClosedMatchSupportV1 {
            support: self.confirmed_admissible_match_support(),
        })
    }

    /// Return authenticated classification supports only when their complete
    /// interval population fits the caller's explicit traversal bound.
    ///
    /// Validation of disjointness and exact union is intentionally performed
    /// here, at a bounded traversal boundary, rather than on every hot reducer
    /// mutation.
    #[allow(dead_code)]
    pub(crate) fn classification_supports_bounded(
        &self,
        max_total_intervals: usize,
    ) -> Result<Option<ExactClosedClassificationSupportsV1>, ExactStreamError> {
        let Some(total_intervals) = self.classification_supports.total_interval_count() else {
            return Ok(None);
        };
        if total_intervals > max_total_intervals {
            return Ok(None);
        }
        self.classification_supports.validate_scalar_counts(
            self.closed_case_count,
            self.excluded_case_count,
            self.admissible_nonmatch_case_count,
            self.matching_case_count,
        )?;
        self.classification_supports.validate_exact_partition()?;
        Ok(Some(self.classification_supports.clone()))
    }

    /// O(1) count of uniform typed rank runs needed by the case-DAG lowerer.
    /// This excludes the redundant `closed` union index.
    pub(crate) fn classification_rank_run_count(&self) -> Option<usize> {
        self.classification_supports
            .excluded
            .interval_count()
            .checked_add(
                self.classification_supports
                    .admissible_nonmatch
                    .interval_count(),
            )
            .and_then(|count| {
                count.checked_add(
                    self.classification_supports
                        .admissible_match
                        .interval_count(),
                )
            })
    }

    /// O(1) traversal bound including the redundant authenticated closed
    /// support and all three typed fibers.
    pub(crate) fn classification_support_interval_count(&self) -> Option<usize> {
        self.classification_supports.total_interval_count()
    }

    pub(crate) fn classification_support_identity_hashes(&self) -> [CanonicalDigest; 4] {
        self.classification_supports.identity_hashes()
    }

    fn debug_assert_classification_support_counts(&self) {
        debug_assert!(
            self.classification_supports
                .validate_scalar_counts(
                    self.closed_case_count,
                    self.excluded_case_count,
                    self.admissible_nonmatch_case_count,
                    self.matching_case_count,
                )
                .is_ok(),
            "exact classification support counts disagree with reducer scalars"
        );
    }

    pub(crate) fn canonical_case_id_at_rank(
        &self,
        rank: u128,
    ) -> Result<ExactCanonicalCaseIdV1, ExactStreamError> {
        let ordinals =
            unrank_mixed_radix(&self.axis_cardinalities, self.universe_case_count, rank)?;
        Ok(ExactCanonicalCaseIdV1::new(rank, ordinals))
    }

    /// Accept one structural/proof batch atomically. Every rank must still be
    /// open, both within the batch and against earlier evidence.
    pub(crate) fn accept_closed_region_batch(
        &mut self,
        batch: &ValidatedExactClosedRegionBatchV1,
    ) -> Result<(), ExactStreamError> {
        let prepared = self.prepare_closed_region_batch(batch.clone())?;
        self.apply_prepared_closed_region_batch(prepared);
        Ok(())
    }

    /// Validate one compact region delta without copying accumulated state.
    pub(crate) fn prepare_closed_region_batch(
        &self,
        batch: ValidatedExactClosedRegionBatchV1,
    ) -> Result<PreparedExactClosedRegionBatchV1, ExactStreamError> {
        validate_validated_region_sequence(&batch.regions)?;
        let mut delta_closed = 0_u128;
        let mut delta_excluded = 0_u128;
        let mut delta_nonmatch = 0_u128;
        let mut excluded_intervals = Vec::new();
        let mut nonmatch_intervals = Vec::new();

        for region in batch.regions.iter() {
            let region = &region.proposal;
            if region.end_rank_exclusive > self.universe_case_count {
                return Err(ExactStreamError::invalid(format!(
                    "closed rank region [{}, {}) exceeds universe cardinality {}",
                    region.start_rank, region.end_rank_exclusive, self.universe_case_count
                )));
            }
            let count = region.case_count();
            delta_closed = checked_count_add(delta_closed, count, "closed-region batch")?;
            match region.classification {
                ExactClosedClassificationV1::Excluded => {
                    delta_excluded = checked_count_add(delta_excluded, count, "excluded region")?;
                    excluded_intervals.push((region.start_rank, region.end_rank_exclusive));
                }
                ExactClosedClassificationV1::AdmissibleNonmatch => {
                    delta_nonmatch =
                        checked_count_add(delta_nonmatch, count, "nonmatching region")?;
                    nonmatch_intervals.push((region.start_rank, region.end_rank_exclusive));
                }
                ExactClosedClassificationV1::AdmissibleMatch => {
                    return Err(ExactStreamError::invalid(
                        "v1 cannot close an AdmissibleMatch region before singleton projection replay",
                    ));
                }
            }
        }

        // These checks make the mutation section below infallible and keep a
        // rejected batch atomic.
        let next_closed = checked_count_add(self.closed_case_count, delta_closed, "closed")?;
        if next_closed > self.universe_case_count {
            return Err(ExactStreamError::invalid(
                "closed evidence exceeds the case universe",
            ));
        }
        let next_excluded =
            checked_count_add(self.excluded_case_count, delta_excluded, "excluded")?;
        let next_nonmatch = checked_count_add(
            self.admissible_nonmatch_case_count,
            delta_nonmatch,
            "admissible nonmatch",
        )?;
        let next_classification_supports = self.classification_supports.merge_delta(
            &self.case_universe,
            excluded_intervals,
            nonmatch_intervals,
            Vec::new(),
        )?;
        next_classification_supports.validate_scalar_counts(
            next_closed,
            next_excluded,
            next_nonmatch,
            self.matching_case_count,
        )?;

        Ok(PreparedExactClosedRegionBatchV1 {
            prior_closed_case_count: self.closed_case_count,
            prior_classification_supports: self.classification_supports.clone(),
            next_classification_supports,
            next_closed_case_count: next_closed,
            next_excluded_case_count: next_excluded,
            next_admissible_nonmatch_case_count: next_nonmatch,
        })
    }

    /// Apply a block previously checked against this exact reducer state.
    ///
    /// This has no semantic error return by design: callers may invoke it only
    /// after the corresponding journal transition is durable. A stale token is
    /// an internal invariant violation; recovery replays the durable event.
    pub(crate) fn apply_prepared_closed_region_batch(
        &mut self,
        prepared: PreparedExactClosedRegionBatchV1,
    ) {
        assert_eq!(
            self.closed_case_count, prepared.prior_closed_case_count,
            "prepared exact region block is stale"
        );
        assert_eq!(
            self.classification_supports, prepared.prior_classification_supports,
            "prepared exact region support block is stale"
        );
        self.classification_supports = prepared.next_classification_supports;
        self.closed_case_count = prepared.next_closed_case_count;
        self.excluded_case_count = prepared.next_excluded_case_count;
        self.admissible_nonmatch_case_count = prepared.next_admissible_nonmatch_case_count;
        self.debug_assert_classification_support_counts();
    }

    pub(crate) fn accept_closed_region(
        &mut self,
        region: ValidatedExactClosedRankRegionV1,
    ) -> Result<(), ExactStreamError> {
        self.accept_closed_region_batch(&ValidatedExactClosedRegionBatchV1 {
            regions: vec![region].into_boxed_slice(),
        })
    }

    /// Accept one fully evaluated singleton atomically.
    pub(crate) fn accept_observation(
        &mut self,
        observation: ValidatedExactCaseObservationV1,
    ) -> Result<(), ExactStreamError> {
        self.accept_observation_batch(&ValidatedExactCaseObservationBatchV1 {
            observations: vec![observation].into_boxed_slice(),
        })
    }

    /// Validate an evaluator-confirmed slice completely, then mutate the
    /// reducer without any remaining fallible semantic step.
    pub(crate) fn accept_observation_batch(
        &mut self,
        batch: &ValidatedExactCaseObservationBatchV1,
    ) -> Result<(), ExactStreamError> {
        let prepared = self.prepare_observation_batch(batch.clone())?;
        self.apply_prepared_observation_batch(prepared);
        Ok(())
    }

    /// Validate one compact observation delta without copying accumulated
    /// result groups or the optional matching ledger.
    pub(crate) fn prepare_observation_batch(
        &self,
        batch: ValidatedExactCaseObservationBatchV1,
    ) -> Result<PreparedExactObservationBatchV1, ExactStreamError> {
        validate_validated_observation_sequence(&batch.observations)?;
        let mut delta_excluded = 0_u128;
        let mut delta_nonmatch = 0_u128;
        let mut delta_matching = 0_u128;
        let mut group_deltas = BTreeMap::<Box<[ExploreValue]>, u128>::new();
        let mut excluded_intervals = Vec::new();
        let mut nonmatch_intervals = Vec::new();
        let mut matching_intervals = Vec::new();

        for observation in batch.observations.iter() {
            let proposal = &observation.proposal;
            validate_case_id(
                &self.axis_cardinalities,
                self.universe_case_count,
                &proposal.case_id,
            )?;
            self.validate_observation_projection(proposal)?;
            match proposal.classification {
                ExactClosedClassificationV1::Excluded => {
                    delta_excluded = checked_count_add(delta_excluded, 1, "excluded batch")?;
                    excluded_intervals.push((proposal.case_id.rank, proposal.case_id.rank + 1));
                }
                ExactClosedClassificationV1::AdmissibleNonmatch => {
                    delta_nonmatch = checked_count_add(delta_nonmatch, 1, "nonmatching batch")?;
                    nonmatch_intervals.push((proposal.case_id.rank, proposal.case_id.rank + 1));
                }
                ExactClosedClassificationV1::AdmissibleMatch => {
                    delta_matching = checked_count_add(delta_matching, 1, "matching batch")?;
                    matching_intervals.push((proposal.case_id.rank, proposal.case_id.rank + 1));
                    let projection = proposal
                        .match_projection
                        .as_ref()
                        .expect("validated matching proposal has a projection");
                    let entry = group_deltas.entry(projection.key.clone()).or_insert(0);
                    *entry = checked_count_add(*entry, 1, "result-group batch")?;
                }
            }
        }

        let batch_len = batch.observations.len() as u128;
        let next_closed = checked_count_add(self.closed_case_count, batch_len, "closed")?;
        if next_closed > self.universe_case_count {
            return Err(ExactStreamError::invalid(
                "closed observation evidence exceeds the case universe",
            ));
        }
        let next_excluded =
            checked_count_add(self.excluded_case_count, delta_excluded, "excluded")?;
        let next_nonmatch = checked_count_add(
            self.admissible_nonmatch_case_count,
            delta_nonmatch,
            "admissible nonmatch",
        )?;
        let next_matching =
            checked_count_add(self.matching_case_count, delta_matching, "matching")?;
        let next_projected = checked_count_add(
            self.projected_matching_case_count,
            delta_matching,
            "projected matching",
        )?;
        let next_classification_supports = self.classification_supports.merge_delta(
            &self.case_universe,
            excluded_intervals,
            nonmatch_intervals,
            matching_intervals,
        )?;
        next_classification_supports.validate_scalar_counts(
            next_closed,
            next_excluded,
            next_nonmatch,
            next_matching,
        )?;
        for (key, delta) in group_deltas {
            if let Some(group) = self.groups.get(key.as_ref()) {
                checked_count_add(group.support, delta, "result-group support")?;
                for extrema in group.extrema.iter() {
                    checked_count_add(extrema.support, delta, "extrema support")?;
                    checked_count_add(extrema.minimum_tie_support, delta, "minimum tie support")?;
                    checked_count_add(extrema.maximum_tie_support, delta, "maximum tie support")?;
                }
            }
        }

        Ok(PreparedExactObservationBatchV1 {
            batch,
            prior_closed_case_count: self.closed_case_count,
            prior_classification_supports: self.classification_supports.clone(),
            next_classification_supports,
            next_closed_case_count: next_closed,
            next_excluded_case_count: next_excluded,
            next_admissible_nonmatch_case_count: next_nonmatch,
            next_matching_case_count: next_matching,
            next_projected_matching_case_count: next_projected,
        })
    }

    /// Apply a block previously checked against this exact reducer state.
    pub(crate) fn apply_prepared_observation_batch(
        &mut self,
        prepared: PreparedExactObservationBatchV1,
    ) {
        assert_eq!(
            self.closed_case_count, prepared.prior_closed_case_count,
            "prepared exact observation block is stale"
        );
        assert_eq!(
            self.classification_supports, prepared.prior_classification_supports,
            "prepared exact observation support block is stale"
        );
        for observation in prepared.batch.observations.iter() {
            let proposal = &observation.proposal;
            if let Some(projection) = proposal.match_projection.as_ref() {
                self.observe_match(&proposal.case_id, projection);
                if let Some(ledger) = self.matching_ledger.as_mut() {
                    let row = ExactMatchingLedgerRowV1 {
                        case_id: proposal.case_id.clone(),
                        match_projection: projection.clone(),
                        semantic_fact_digest: observation.semantic_fact_digest,
                    };
                    let previous = ledger.insert(proposal.case_id.rank, row);
                    debug_assert!(previous.is_none(), "coverage rejected duplicate CaseId");
                }
            }
        }
        self.classification_supports = prepared.next_classification_supports;
        self.closed_case_count = prepared.next_closed_case_count;
        self.excluded_case_count = prepared.next_excluded_case_count;
        self.admissible_nonmatch_case_count = prepared.next_admissible_nonmatch_case_count;
        self.matching_case_count = prepared.next_matching_case_count;
        self.projected_matching_case_count = prepared.next_projected_matching_case_count;
        self.debug_assert_classification_support_counts();
    }

    pub(crate) fn snapshot(&self) -> ExactEvidenceSnapshotV1 {
        let complete_classification = self.closed_case_count == self.universe_case_count;
        let projection_complete =
            complete_classification && self.unprojected_matching_case_count == 0;
        let admissible_case_count = self
            .admissible_nonmatch_case_count
            .checked_add(self.matching_case_count)
            .expect("disjoint admissible classifications fit the case universe");

        let results = self
            .groups
            .iter()
            .map(|(key, group)| snapshot_result_group(key, group, projection_complete))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let matching_ledger =
            self.matching_ledger
                .as_ref()
                .map(|ledger| ExactMatchingLedgerSnapshotV1 {
                    observations: ledger
                        .values()
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    complete: projection_complete,
                });

        ExactEvidenceSnapshotV1 {
            universe_case_count: self.universe_case_count,
            closed_case_count: self.closed_case_count,
            open_case_count: self.universe_case_count - self.closed_case_count,
            excluded: ExactCountBoundV1::new(self.excluded_case_count, complete_classification),
            admissible: ExactCountBoundV1::new(admissible_case_count, complete_classification),
            matching: ExactCountBoundV1::new(self.matching_case_count, complete_classification),
            projected_matching_case_count: self.projected_matching_case_count,
            unprojected_matching_case_count: self.unprojected_matching_case_count,
            projection_complete,
            observed_result_group_count: self.groups.len() as u128,
            result_group_scan_complete: true,
            results,
            matching_ledger,
        }
    }

    /// Build the bounded cursor-bearing view used for every pause and replay
    /// verification. The reducer keeps all groups; this clones only a
    /// deterministic canonical-key prefix and never clones the optional
    /// matching ledger.
    pub(crate) fn observable_snapshot(&self) -> ExactEvidenceSnapshotV1 {
        let complete_classification = self.closed_case_count == self.universe_case_count;
        let projection_complete =
            complete_classification && self.unprojected_matching_case_count == 0;
        let admissible_case_count = self
            .admissible_nonmatch_case_count
            .checked_add(self.matching_case_count)
            .expect("disjoint admissible classifications fit the case universe");

        let mut results = Vec::new();
        let mut used = ObservableResultCostV1::default();
        for (key, group) in self.groups.iter() {
            if results.len() == EXACT_OBSERVABLE_RESULT_PREVIEW_GROUP_LIMIT_V1 {
                break;
            }
            let Some(next) = observable_result_cost_after(used, key, group) else {
                break;
            };
            used = next;
            results.push(snapshot_result_group(key, group, projection_complete));
        }
        let observed_result_group_count = self.groups.len() as u128;
        let result_group_scan_complete = results.len() as u128 == observed_result_group_count;

        ExactEvidenceSnapshotV1 {
            universe_case_count: self.universe_case_count,
            closed_case_count: self.closed_case_count,
            open_case_count: self.universe_case_count - self.closed_case_count,
            excluded: ExactCountBoundV1::new(self.excluded_case_count, complete_classification),
            admissible: ExactCountBoundV1::new(admissible_case_count, complete_classification),
            matching: ExactCountBoundV1::new(self.matching_case_count, complete_classification),
            projected_matching_case_count: self.projected_matching_case_count,
            unprojected_matching_case_count: self.unprojected_matching_case_count,
            projection_complete,
            observed_result_group_count,
            result_group_scan_complete,
            results: results.into_boxed_slice(),
            matching_ledger: None,
        }
    }

    fn validate_observation_projection(
        &self,
        observation: &ExactCaseObservationProposalV1,
    ) -> Result<(), ExactStreamError> {
        let Some(projection) = observation.match_projection.as_ref() else {
            return Ok(());
        };
        for (name, actual, expected) in [
            ("key", projection.key.len(), self.projection_shape.key_width),
            (
                "extrema",
                projection.extrema.len(),
                self.projection_shape.extrema_width,
            ),
            (
                "shown",
                projection.shown.len(),
                self.projection_shape.shown_width,
            ),
        ] {
            if actual != expected {
                return Err(ExactStreamError::invalid(format!(
                    "matching CaseId rank {} has {actual} {name} values; report requires {expected}",
                    observation.case_id.rank
                )));
            }
        }
        match (
            self.representative_policy,
            projection.representative_objective,
        ) {
            (ExactRepresentativePolicyV1::First, None)
            | (ExactRepresentativePolicyV1::Maximize, Some(_))
            | (ExactRepresentativePolicyV1::Minimize, Some(_)) => Ok(()),
            (ExactRepresentativePolicyV1::First, Some(_)) => Err(ExactStreamError::invalid(
                "representative-first observation must not retain an objective",
            )),
            (ExactRepresentativePolicyV1::Maximize, None)
            | (ExactRepresentativePolicyV1::Minimize, None) => Err(ExactStreamError::invalid(
                "ordered representative observation is missing its Int objective",
            )),
        }
    }

    fn observe_match(
        &mut self,
        case_id: &ExactCanonicalCaseIdV1,
        projection: &ExactMatchProjectionV1,
    ) {
        let candidate = RepresentativeCandidate {
            case_id: case_id.clone(),
            shown: projection.shown.clone(),
            objective: projection.representative_objective,
        };
        match self.groups.get_mut(projection.key.as_ref()) {
            Some(group) => {
                group.support = group
                    .support
                    .checked_add(1)
                    .expect("group support cannot exceed the u128 universe");
                for (extrema, value) in group.extrema.iter_mut().zip(projection.extrema.iter()) {
                    extrema.observe(*value, case_id);
                }
                if representative_is_better(
                    self.representative_policy,
                    &candidate,
                    &group.representative,
                ) {
                    group.representative = candidate;
                }
            }
            None => {
                self.groups.insert(
                    projection.key.clone(),
                    MutableResultGroup {
                        support: 1,
                        extrema: projection
                            .extrema
                            .iter()
                            .map(|value| MutableExtrema::first(*value, case_id))
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        representative: candidate,
                    },
                );
            }
        }
    }
}

fn snapshot_result_group(
    key: &[ExploreValue],
    group: &MutableResultGroup,
    projection_complete: bool,
) -> ExactResultAggregateV1 {
    ExactResultAggregateV1 {
        key: key.to_vec().into_boxed_slice(),
        support: ExactCountBoundV1::new(group.support, projection_complete),
        extrema: group
            .extrema
            .iter()
            .map(|extrema| extrema.snapshot(projection_complete))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        representative_case_id: group.representative.case_id.clone(),
        representative_shown: group.representative.shown.clone(),
        representative_objective: group.representative.objective,
        representative_selection_closed: projection_complete,
    }
}

fn observable_result_cost_after(
    mut cost: ObservableResultCostV1,
    key: &[ExploreValue],
    group: &MutableResultGroup,
) -> Option<ObservableResultCostV1> {
    // Charge a conservative in-memory/semantic envelope for the non-value
    // aggregate shape as well as the recursively nested ExploreValues. The
    // JSON renderer applies its own exact byte cap after labels and escaping.
    charge_observable_result_bytes(&mut cost, 256)?;
    charge_observable_result_bytes(
        &mut cost,
        group
            .representative
            .case_id
            .ordinals
            .len()
            .checked_mul(16)?,
    )?;
    for extrema in group.extrema.iter() {
        charge_observable_result_bytes(&mut cost, 128)?;
        charge_observable_result_bytes(
            &mut cost,
            extrema.minimum_witness.ordinals.len().checked_mul(16)?,
        )?;
        charge_observable_result_bytes(
            &mut cost,
            extrema.maximum_witness.ordinals.len().checked_mul(16)?,
        )?;
    }
    for value in key.iter().chain(group.representative.shown.iter()) {
        charge_observable_result_value(&mut cost, value)?;
    }
    Some(cost)
}

fn charge_observable_result_value(
    cost: &mut ObservableResultCostV1,
    value: &ExploreValue,
) -> Option<()> {
    cost.value_nodes = cost.value_nodes.checked_add(1)?;
    if cost.value_nodes > EXACT_OBSERVABLE_RESULT_PREVIEW_VALUE_NODE_LIMIT_V1 {
        return None;
    }
    charge_observable_result_bytes(cost, 1)?; // canonical semantic tag
    match value {
        ExploreValue::Int(_) | ExploreValue::FloatBits(_) => {
            charge_observable_result_bytes(cost, 8)
        }
        ExploreValue::String(value) => {
            charge_observable_result_bytes(cost, 4_usize.checked_add(value.len())?)
        }
        ExploreValue::Character(_) => charge_observable_result_bytes(cost, 4),
        ExploreValue::Boolean(_) => charge_observable_result_bytes(cost, 1),
        ExploreValue::Unit => Some(()),
        ExploreValue::List(values) | ExploreValue::Set(values) | ExploreValue::Tuple(values) => {
            charge_observable_result_bytes(cost, 4)?;
            for child in values {
                charge_observable_result_value(cost, child)?;
            }
            Some(())
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            fields,
            ..
        } => {
            charge_observable_result_bytes(cost, 4_usize.checked_add(type_name.len())?)?;
            charge_observable_result_bytes(cost, 4_usize.checked_add(variant.len())?)?;
            charge_observable_result_bytes(cost, 5)?;
            for (name, child) in fields {
                charge_observable_result_bytes(cost, 4_usize.checked_add(name.len())?)?;
                charge_observable_result_value(cost, child)?;
            }
            Some(())
        }
    }
}

fn charge_observable_result_bytes(cost: &mut ObservableResultCostV1, amount: usize) -> Option<()> {
    cost.semantic_bytes = cost.semantic_bytes.checked_add(amount)?;
    (cost.semantic_bytes <= EXACT_OBSERVABLE_RESULT_PREVIEW_SEMANTIC_BYTE_LIMIT_V1).then_some(())
}

fn representative_is_better(
    policy: ExactRepresentativePolicyV1,
    candidate: &RepresentativeCandidate,
    incumbent: &RepresentativeCandidate,
) -> bool {
    match policy {
        ExactRepresentativePolicyV1::First => candidate.case_id.rank < incumbent.case_id.rank,
        ExactRepresentativePolicyV1::Maximize => {
            let candidate_objective = candidate
                .objective
                .expect("validated maximize candidate has an objective");
            let incumbent_objective = incumbent
                .objective
                .expect("validated maximize incumbent has an objective");
            candidate_objective > incumbent_objective
                || (candidate_objective == incumbent_objective
                    && candidate.case_id.rank < incumbent.case_id.rank)
        }
        ExactRepresentativePolicyV1::Minimize => {
            let candidate_objective = candidate
                .objective
                .expect("validated minimize candidate has an objective");
            let incumbent_objective = incumbent
                .objective
                .expect("validated minimize incumbent has an objective");
            candidate_objective < incumbent_objective
                || (candidate_objective == incumbent_objective
                    && candidate.case_id.rank < incumbent.case_id.rank)
        }
    }
}

fn unrank_mixed_radix(
    cardinalities: &[u128],
    case_count: u128,
    rank: u128,
) -> Result<Box<[u128]>, ExactStreamError> {
    if rank >= case_count {
        return Err(ExactStreamError::invalid(format!(
            "CaseId rank {rank} is outside universe cardinality {case_count}"
        )));
    }
    let mut remainder = rank;
    let mut ordinals = vec![0_u128; cardinalities.len()];
    for (index, cardinality) in cardinalities.iter().copied().enumerate().rev() {
        // `case_count > rank` proves every cardinality is nonzero.
        ordinals[index] = remainder % cardinality;
        remainder /= cardinality;
    }
    Ok(ordinals.into_boxed_slice())
}

fn validate_case_id(
    cardinalities: &[u128],
    case_count: u128,
    case_id: &ExactCanonicalCaseIdV1,
) -> Result<(), ExactStreamError> {
    if case_id.rank >= case_count {
        return Err(ExactStreamError::invalid(format!(
            "CaseId rank {} is outside universe cardinality {case_count}",
            case_id.rank
        )));
    }
    if case_id.ordinals.len() != cardinalities.len() {
        return Err(ExactStreamError::invalid(format!(
            "CaseId rank {} has {} ordinals for {} axes",
            case_id.rank,
            case_id.ordinals.len(),
            cardinalities.len()
        )));
    }
    let mut computed_rank = 0_u128;
    for (axis, (&ordinal, &cardinality)) in case_id
        .ordinals
        .iter()
        .zip(cardinalities.iter())
        .enumerate()
    {
        if ordinal >= cardinality {
            return Err(ExactStreamError::invalid(format!(
                "CaseId ordinal {ordinal} at axis {axis} is outside cardinality {cardinality}"
            )));
        }
        computed_rank = computed_rank
            .checked_mul(cardinality)
            .and_then(|prefix| prefix.checked_add(ordinal))
            .ok_or_else(|| ExactStreamError::invalid("CaseId mixed-radix rank overflow"))?;
    }
    if computed_rank != case_id.rank {
        return Err(ExactStreamError::invalid(format!(
            "CaseId rank {} disagrees with canonical mixed-radix rank {computed_rank}",
            case_id.rank
        )));
    }
    Ok(())
}

fn checked_count_add(left: u128, right: u128, name: &str) -> Result<u128, ExactStreamError> {
    left.checked_add(right)
        .ok_or_else(|| ExactStreamError::invalid(format!("{name} count exceeds u128::MAX")))
}

fn validate_nonempty_batch_len(
    name: &str,
    actual: usize,
    limit: usize,
) -> Result<(), ExactStreamError> {
    if actual == 0 {
        return Err(ExactStreamError::invalid(format!(
            "{name} batch must not be empty"
        )));
    }
    validate_sequence_len(name, actual, limit)
}

fn validate_canonical_observation_sequence(
    observations: &[ExactCaseObservationProposalV1],
) -> Result<(), ExactStreamError> {
    validate_nonempty_batch_len(
        "exact observation",
        observations.len(),
        MAX_OBSERVATIONS_PER_BATCH,
    )?;
    for observation in observations {
        validate_observation_wire(observation)?;
    }
    for pair in observations.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if left.case_id.rank >= right.case_id.rank {
            return Err(ExactStreamError::invalid(format!(
                "exact observation batch has duplicate or noncanonical ranks {} and {}",
                left.case_id.rank, right.case_id.rank
            )));
        }
    }
    Ok(())
}

fn validate_validated_observation_sequence(
    observations: &[ValidatedExactCaseObservationV1],
) -> Result<(), ExactStreamError> {
    validate_nonempty_batch_len(
        "validated exact observation",
        observations.len(),
        MAX_OBSERVATIONS_PER_BATCH,
    )?;
    for observation in observations {
        validate_observation_wire(&observation.proposal)?;
        let expected = derive_observation_semantic_digest(&observation.proposal)?;
        if observation.semantic_fact_digest != expected {
            return Err(ExactStreamError::invalid(
                "validated exact observation has a non-derived semantic digest",
            ));
        }
    }
    for pair in observations.windows(2) {
        let left = pair[0].proposal.case_id.rank;
        let right = pair[1].proposal.case_id.rank;
        if left >= right {
            return Err(ExactStreamError::invalid(format!(
                "validated exact observation batch has duplicate or noncanonical ranks {left} and {right}"
            )));
        }
    }
    Ok(())
}

fn validate_canonical_region_sequence(
    regions: &[ExactClosedRankRegionProposalV1],
) -> Result<(), ExactStreamError> {
    validate_nonempty_batch_len("closed-region", regions.len(), MAX_REGIONS_PER_BATCH)?;
    for region in regions {
        validate_region_proposal(region)?;
    }
    for pair in regions.windows(2) {
        validate_ordered_region_pair(&pair[0], &pair[1])?;
    }
    Ok(())
}

fn validate_region_proposal(
    region: &ExactClosedRankRegionProposalV1,
) -> Result<(), ExactStreamError> {
    if region.start_rank >= region.end_rank_exclusive {
        return Err(ExactStreamError::invalid(format!(
            "closed rank region [{}, {}) is empty or reversed",
            region.start_rank, region.end_rank_exclusive
        )));
    }
    validate_nonempty_batch_len(
        "closed-region validation receipt",
        region.validation_receipt_digests.len(),
        MAX_REGIONS_PER_BATCH,
    )?;
    if region
        .validation_receipt_digests
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(ExactStreamError::invalid(
            "closed-region validation receipts are not strictly canonical",
        ));
    }
    Ok(())
}

fn validate_ordered_region_pair(
    left: &ExactClosedRankRegionProposalV1,
    right: &ExactClosedRankRegionProposalV1,
) -> Result<(), ExactStreamError> {
    if left.start_rank > right.start_rank || (left.start_rank == right.start_rank && left > right) {
        return Err(ExactStreamError::invalid(
            "closed-region batch is not in canonical rank order",
        ));
    }
    if left.end_rank_exclusive > right.start_rank {
        return Err(ExactStreamError::invalid(format!(
            "closed regions [{}, {}) and [{}, {}) overlap",
            left.start_rank, left.end_rank_exclusive, right.start_rank, right.end_rank_exclusive
        )));
    }
    Ok(())
}

fn validate_validated_region_sequence(
    regions: &[ValidatedExactClosedRankRegionV1],
) -> Result<(), ExactStreamError> {
    validate_nonempty_batch_len(
        "validated closed-region",
        regions.len(),
        MAX_REGIONS_PER_BATCH,
    )?;
    for region in regions {
        validate_region_proposal(&region.proposal)?;
        if region.proposal.classification == ExactClosedClassificationV1::AdmissibleMatch {
            return Err(ExactStreamError::invalid(
                "v1 cannot seal an AdmissibleMatch region before singleton projection replay",
            ));
        }
        let expected = derive_region_semantic_digest(&region.proposal)?;
        if region.semantic_fact_digest != expected {
            return Err(ExactStreamError::invalid(
                "validated closed region has a non-derived semantic digest",
            ));
        }
    }
    for pair in regions.windows(2) {
        let left = &pair[0].proposal;
        let right = &pair[1].proposal;
        validate_ordered_region_pair(left, right)?;
        if regions_can_coalesce(left, right) {
            return Err(ExactStreamError::invalid(
                "validated closed-region batch contains non-normalized adjacent regions",
            ));
        }
    }
    Ok(())
}

fn validate_normalized_region_sequence(
    regions: &[ExactClosedRankRegionProposalV1],
) -> Result<(), ExactStreamError> {
    validate_canonical_region_sequence(regions)?;
    if regions
        .windows(2)
        .any(|pair| regions_can_coalesce(&pair[0], &pair[1]))
    {
        return Err(ExactStreamError::invalid(
            "closed-region batch contains non-normalized adjacent regions",
        ));
    }
    Ok(())
}

fn regions_can_coalesce(
    left: &ExactClosedRankRegionProposalV1,
    right: &ExactClosedRankRegionProposalV1,
) -> bool {
    left.end_rank_exclusive == right.start_rank
        && left.kind == right.kind
        && left.classification == right.classification
}

fn coalesce_adjacent_regions(
    regions: Vec<ExactClosedRankRegionProposalV1>,
) -> Result<Vec<ExactClosedRankRegionProposalV1>, ExactStreamError> {
    let mut normalized = Vec::<ExactClosedRankRegionProposalV1>::new();
    for region in regions {
        if let Some(previous) = normalized.last_mut() {
            if regions_can_coalesce(previous, &region) {
                previous.end_rank_exclusive = region.end_rank_exclusive;
                let validation_receipt_digests = previous
                    .validation_receipt_digests
                    .iter()
                    .chain(region.validation_receipt_digests.iter())
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                validate_sequence_len(
                    "coalesced closed-region validation receipt",
                    validation_receipt_digests.len(),
                    MAX_REGIONS_PER_BATCH,
                )?;
                previous.validation_receipt_digests = validation_receipt_digests.into_boxed_slice();
                continue;
            }
        }
        normalized.push(region);
    }
    Ok(normalized)
}

fn derive_observation_semantic_digest(
    observation: &ExactCaseObservationProposalV1,
) -> Result<ExactSemanticFactDigestV1, ExactStreamError> {
    derive_semantic_digest(
        observation.classification,
        observation.match_projection.as_ref(),
    )
}

fn derive_region_semantic_digest(
    region: &ExactClosedRankRegionProposalV1,
) -> Result<ExactSemanticFactDigestV1, ExactStreamError> {
    derive_semantic_digest(region.classification, None)
}

fn derive_semantic_digest(
    classification: ExactClosedClassificationV1,
    projection: Option<&ExactMatchProjectionV1>,
) -> Result<ExactSemanticFactDigestV1, ExactStreamError> {
    let mut writer = CanonicalWriter::new();
    writer.bytes(SEMANTIC_FACT_DIGEST_V1)?;
    writer.u8(classification_tag(classification))?;
    match projection {
        None => writer.u8(0)?,
        Some(projection) => {
            validate_projection_wire(projection)?;
            writer.u8(1)?;
            let mut budget = ValueBudget::default();
            write_value_slice(&mut writer, &mut budget, &projection.key)?;
            writer.len(
                projection.extrema.len(),
                MAX_PROJECTION_FIELDS,
                "extrema projection",
            )?;
            for value in projection.extrema.iter().copied() {
                writer.i64(value)?;
            }
            write_value_slice(&mut writer, &mut budget, &projection.shown)?;
            match projection.representative_objective {
                None => writer.u8(0)?,
                Some(value) => {
                    writer.u8(1)?;
                    writer.i64(value)?;
                }
            }
        }
    }
    let digest: [u8; 32] = Sha256::digest(writer.finish()).into();
    Ok(ExactSemanticFactDigestV1(digest))
}

/// Private mint boundary. Future production adapters must validate the actual
/// evaluator result or proof certificate before calling these seams. Persisted
/// bytes intentionally decode only to proposal types.
mod validation_boundary {
    use super::*;

    fn seal_evaluator_confirmed_observation(
        proposal: ExactCaseObservationProposalV1,
    ) -> Result<ValidatedExactCaseObservationV1, ExactStreamError> {
        validate_observation_wire(&proposal)?;
        let semantic_fact_digest = derive_observation_semantic_digest(&proposal)?;
        Ok(ValidatedExactCaseObservationV1 {
            proposal,
            semantic_fact_digest,
        })
    }

    /// Production mint seam. `confirm` must replay the checked evaluator and
    /// verify every receipt/projection against this exact canonical proposal
    /// batch. The trusted coordinator must not substitute syntactic checking.
    pub(super) fn seal_evaluator_confirmed_canonical_observation_batch<Confirm>(
        proposal: ExactCaseObservationBatchProposalV1,
        confirm: Confirm,
    ) -> Result<ValidatedExactCaseObservationBatchV1, ExactStreamError>
    where
        Confirm: FnOnce(&ExactCaseObservationBatchProposalV1) -> Result<(), ExactStreamError>,
    {
        validate_canonical_observation_sequence(&proposal.observations)?;
        confirm(&proposal)?;
        let mut observations = Vec::new();
        for observation in proposal.observations.into_vec() {
            observations.push(seal_evaluator_confirmed_observation(observation)?);
        }
        let validated = ValidatedExactCaseObservationBatchV1 {
            observations: observations.into_boxed_slice(),
        };
        validate_validated_observation_sequence(&validated.observations)?;
        Ok(validated)
    }

    /// Production mint seam. `confirm` must resolve the retained receipt set,
    /// revalidate checked source certificates/structure, and prove that their
    /// disjoint union is exactly each proposed interval and classification.
    pub(super) fn seal_revalidated_proof_or_structure_batch<Confirm>(
        proposal: ExactClosedRegionBatchProposalV1,
        confirm: Confirm,
    ) -> Result<ValidatedExactClosedRegionBatchV1, ExactStreamError>
    where
        Confirm: FnOnce(&ExactClosedRegionBatchProposalV1) -> Result<(), ExactStreamError>,
    {
        validate_normalized_region_sequence(&proposal.regions)?;
        if proposal
            .regions
            .iter()
            .any(|region| region.classification == ExactClosedClassificationV1::AdmissibleMatch)
        {
            return Err(ExactStreamError::invalid(
                "v1 keeps proof-matching regions open for singleton projection replay",
            ));
        }
        confirm(&proposal)?;
        let mut regions = Vec::new();
        for proposal in proposal.regions.into_vec() {
            let semantic_fact_digest = derive_region_semantic_digest(&proposal)?;
            regions.push(ValidatedExactClosedRankRegionV1 {
                proposal,
                semantic_fact_digest,
            });
        }
        let validated = ValidatedExactClosedRegionBatchV1 {
            regions: regions.into_boxed_slice(),
        };
        validate_validated_region_sequence(&validated.regions)?;
        Ok(validated)
    }

    #[cfg(test)]
    pub(super) fn test_only_seal_observation(
        proposal: ExactCaseObservationProposalV1,
    ) -> Result<ValidatedExactCaseObservationV1, ExactStreamError> {
        seal_evaluator_confirmed_observation(proposal)
    }

    #[cfg(test)]
    pub(super) fn test_only_seal_observation_batch(
        proposal: ExactCaseObservationBatchProposalV1,
    ) -> Result<ValidatedExactCaseObservationBatchV1, ExactStreamError> {
        seal_evaluator_confirmed_canonical_observation_batch(proposal, |_| Ok(()))
    }

    #[cfg(test)]
    pub(super) fn test_only_seal_region_batch(
        proposal: ExactClosedRegionBatchProposalV1,
    ) -> Result<ValidatedExactClosedRegionBatchV1, ExactStreamError> {
        seal_revalidated_proof_or_structure_batch(proposal, |_| Ok(()))
    }
}

/// Maximum number of source regions that one canonical v1 proposal may carry.
///
/// Trusted producer adapters use this before materializing proof rectangles;
/// it is a wire bound, never license to silently truncate semantic support.
pub(super) const fn exact_closed_region_batch_limit_v1() -> usize {
    MAX_REGIONS_PER_BATCH
}

/// Trusted evaluator boundary exposed to sibling coordinator adapters.
///
/// `confirm` must replay the checked evaluator and bind every proposal to its
/// producer receipt. Decoded records cannot reach this function without an
/// explicit trusted adapter making that confirmation.
pub(super) fn seal_evaluator_confirmed_canonical_observation_batch<Confirm>(
    proposal: ExactCaseObservationBatchProposalV1,
    confirm: Confirm,
) -> Result<ValidatedExactCaseObservationBatchV1, ExactStreamError>
where
    Confirm: FnOnce(&ExactCaseObservationBatchProposalV1) -> Result<(), String>,
{
    validation_boundary::seal_evaluator_confirmed_canonical_observation_batch(
        proposal,
        |proposal| confirm(proposal).map_err(ExactStreamError::invalid),
    )
}

/// Restore observations which this coordinator previously minted and durably
/// committed.
///
/// This is deliberately distinct from evaluator confirmation.  Reopening an
/// owner-local stream must replay canonical evidence, not re-evaluate every
/// completed CaseId.  Before returning `Ok`, `confirm_commitment` must have
/// verified the blob content digest, canonical journal envelope, historical
/// writer fence, and the transition's committed support/facts.  Decoded worker
/// proposals which have not crossed that durable coordinator boundary must use
/// evaluator revalidation instead.
pub(super) fn restore_coordinator_committed_observation_batch_v1<Confirm>(
    proposal: ExactCaseObservationBatchProposalV1,
    confirm_commitment: Confirm,
) -> Result<ValidatedExactCaseObservationBatchV1, ExactStreamError>
where
    Confirm: FnOnce(&ValidatedExactCaseObservationBatchV1) -> Result<(), String>,
{
    let validated = validation_boundary::seal_evaluator_confirmed_canonical_observation_batch(
        proposal,
        |_| Ok(()),
    )?;
    confirm_commitment(&validated).map_err(ExactStreamError::invalid)?;
    Ok(validated)
}

/// Trusted proof/structure boundary exposed to sibling coordinator adapters.
///
/// `confirm` must rederive the complete normalized interval/receipt union from
/// checked certificates or structural facts. A decoder can construct only the
/// proposal passed to this seam, never the validated wrapper it returns.
pub(super) fn seal_revalidated_proof_or_structure_batch<Confirm>(
    proposal: ExactClosedRegionBatchProposalV1,
    confirm: Confirm,
) -> Result<ValidatedExactClosedRegionBatchV1, ExactStreamError>
where
    Confirm: FnOnce(&ExactClosedRegionBatchProposalV1) -> Result<(), String>,
{
    validation_boundary::seal_revalidated_proof_or_structure_batch(proposal, |proposal| {
        confirm(proposal).map_err(ExactStreamError::invalid)
    })
}

/// Restore proof/structural regions which this coordinator previously minted
/// and durably committed.
///
/// The confirmation is about the authenticated local commitment, not a second
/// execution of the source proof.  This keeps restart cost proportional to
/// canonical replay rather than prior proof work.  Fresh or remote region
/// proposals must continue through `seal_revalidated_proof_or_structure_batch`.
pub(super) fn restore_coordinator_committed_region_batch_v1<Confirm>(
    proposal: ExactClosedRegionBatchProposalV1,
    confirm_commitment: Confirm,
) -> Result<ValidatedExactClosedRegionBatchV1, ExactStreamError>
where
    Confirm: FnOnce(&ValidatedExactClosedRegionBatchV1) -> Result<(), String>,
{
    let validated =
        validation_boundary::seal_revalidated_proof_or_structure_batch(proposal, |_| Ok(()))?;
    confirm_commitment(&validated).map_err(ExactStreamError::invalid)?;
    Ok(validated)
}

fn validate_observation_wire(
    observation: &ExactCaseObservationProposalV1,
) -> Result<(), ExactStreamError> {
    if observation.case_id.ordinals.len() > MAX_AXES {
        return Err(ExactStreamError::invalid(format!(
            "CaseId has {} ordinals; limit is {MAX_AXES}",
            observation.case_id.ordinals.len()
        )));
    }
    match (
        observation.classification,
        observation.match_projection.as_ref(),
    ) {
        (ExactClosedClassificationV1::AdmissibleMatch, Some(projection)) => {
            validate_projection_wire(projection)
        }
        (ExactClosedClassificationV1::AdmissibleMatch, None) => Err(ExactStreamError::invalid(
            "matching singleton observation is missing its complete projection",
        )),
        (ExactClosedClassificationV1::Excluded, Some(_))
        | (ExactClosedClassificationV1::AdmissibleNonmatch, Some(_)) => Err(
            ExactStreamError::invalid("nonmatching singleton observation carries a projection"),
        ),
        (ExactClosedClassificationV1::Excluded, None)
        | (ExactClosedClassificationV1::AdmissibleNonmatch, None) => Ok(()),
    }
}

fn validate_projection_wire(projection: &ExactMatchProjectionV1) -> Result<(), ExactStreamError> {
    for (name, width) in [
        ("key", projection.key.len()),
        ("extrema", projection.extrema.len()),
        ("shown", projection.shown.len()),
    ] {
        if width > MAX_PROJECTION_FIELDS {
            return Err(ExactStreamError::invalid(format!(
                "{name} projection width {width} exceeds limit {MAX_PROJECTION_FIELDS}"
            )));
        }
    }
    let mut budget = ValueBudget::default();
    for value in projection.key.iter().chain(projection.shown.iter()) {
        validate_value(value, 0, &mut budget)?;
    }
    Ok(())
}

#[derive(Default)]
struct ValueBudget {
    nodes: usize,
    bytes: usize,
}

impl ValueBudget {
    fn node(&mut self, depth: usize) -> Result<(), ExactStreamError> {
        if depth > MAX_VALUE_DEPTH {
            return Err(ExactStreamError::invalid(format!(
                "ExploreValue nesting depth exceeds {MAX_VALUE_DEPTH}"
            )));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| ExactStreamError::invalid("ExploreValue node count overflow"))?;
        if self.nodes > MAX_VALUE_NODES {
            return Err(ExactStreamError::invalid(format!(
                "ExploreValue node count exceeds {MAX_VALUE_NODES}"
            )));
        }
        Ok(())
    }

    fn bytes(&mut self, count: usize) -> Result<(), ExactStreamError> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or_else(|| ExactStreamError::invalid("ExploreValue byte count overflow"))?;
        if self.bytes > MAX_VALUE_BYTES {
            return Err(ExactStreamError::invalid(format!(
                "ExploreValue encoded bytes exceed {MAX_VALUE_BYTES}"
            )));
        }
        Ok(())
    }
}

fn validate_value(
    value: &ExploreValue,
    depth: usize,
    budget: &mut ValueBudget,
) -> Result<(), ExactStreamError> {
    budget.node(depth)?;
    budget.bytes(1)?; // tag
    match value {
        ExploreValue::Int(_) | ExploreValue::FloatBits(_) => budget.bytes(8),
        ExploreValue::String(value) => validate_text(value, budget),
        ExploreValue::Character(_) => budget.bytes(4),
        ExploreValue::Boolean(_) => budget.bytes(1),
        ExploreValue::Unit => Ok(()),
        ExploreValue::List(values) | ExploreValue::Set(values) | ExploreValue::Tuple(values) => {
            validate_sequence_len("ExploreValue sequence", values.len(), MAX_SEQUENCE_ITEMS)?;
            budget.bytes(4)?;
            for child in values {
                validate_value(child, depth + 1, budget)?;
            }
            if let ExploreValue::Set(values) = value {
                validate_runtime_set_order(values)?;
            }
            Ok(())
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            validate_sequence_len("constructor field", fields.len(), MAX_SEQUENCE_ITEMS)?;
            if type_name.is_empty() || variant.is_empty() {
                return Err(ExactStreamError::invalid(
                    "constructor type_name and variant must not be empty",
                ));
            }
            if fields.is_empty() && !positional {
                return Err(ExactStreamError::invalid(
                    "nullary constructors must use the canonical positional spelling",
                ));
            }
            validate_text(type_name, budget)?;
            validate_text(variant, budget)?;
            budget.bytes(5)?; // positional flag plus field count
            let mut field_names = BTreeSet::new();
            for (name, field) in fields {
                if name.is_empty() {
                    return Err(ExactStreamError::invalid(
                        "constructor field name must not be empty",
                    ));
                }
                if !field_names.insert(name.as_str()) {
                    return Err(ExactStreamError::invalid(format!(
                        "constructor field `{name}` occurs more than once"
                    )));
                }
                validate_text(name, budget)?;
                validate_value(field, depth + 1, budget)?;
            }
            Ok(())
        }
    }
}

fn validate_runtime_set_order(values: &[ExploreValue]) -> Result<(), ExactStreamError> {
    let mut previous: Option<String> = None;
    for (index, value) in values.iter().enumerate() {
        let key = value.runtime_display_key();
        if previous.as_ref().is_some_and(|prior| prior >= &key) {
            return Err(ExactStreamError::invalid(format!(
                "ExploreValue set member {index} is duplicate or outside canonical runtime value order"
            )));
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_text(value: &str, budget: &mut ValueBudget) -> Result<(), ExactStreamError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(ExactStreamError::invalid(format!(
            "string has {} UTF-8 bytes; limit is {MAX_STRING_BYTES}",
            value.len()
        )));
    }
    budget.bytes(4 + value.len())
}

fn validate_sequence_len(name: &str, actual: usize, limit: usize) -> Result<(), ExactStreamError> {
    if actual > limit {
        return Err(ExactStreamError::invalid(format!(
            "{name} length {actual} exceeds limit {limit}"
        )));
    }
    Ok(())
}

/// Encode one singleton observation using the canonical bounded v1 binary
/// representation. No Rust or serde layout is part of this contract.
pub(crate) fn encode_exact_case_observation_v1(
    observation: &ExactCaseObservationProposalV1,
) -> Result<Vec<u8>, ExactStreamError> {
    validate_observation_wire(observation)?;
    let mut writer = CanonicalWriter::new();
    writer.bytes(OBSERVATION_MAGIC_V1)?;
    write_observation_body(&mut writer, observation)?;
    Ok(writer.finish())
}

fn write_observation_body(
    writer: &mut CanonicalWriter,
    observation: &ExactCaseObservationProposalV1,
) -> Result<(), ExactStreamError> {
    writer.u8(classification_tag(observation.classification))?;
    writer.u128(observation.case_id.rank)?;
    writer.len(
        observation.case_id.ordinals.len(),
        MAX_AXES,
        "CaseId ordinals",
    )?;
    for ordinal in observation.case_id.ordinals.iter().copied() {
        writer.u128(ordinal)?;
    }
    writer.bytes(&observation.validation_receipt_digest.0)?;
    if let Some(projection) = observation.match_projection.as_ref() {
        let mut budget = ValueBudget::default();
        write_value_slice(writer, &mut budget, &projection.key)?;
        writer.len(
            projection.extrema.len(),
            MAX_PROJECTION_FIELDS,
            "extrema projection",
        )?;
        for value in projection.extrema.iter().copied() {
            writer.i64(value)?;
        }
        write_value_slice(writer, &mut budget, &projection.shown)?;
        match projection.representative_objective {
            None => writer.u8(0)?,
            Some(value) => {
                writer.u8(1)?;
                writer.i64(value)?;
            }
        }
    }
    Ok(())
}

/// Decode only the unique canonical v1 byte representation.
pub(crate) fn decode_exact_case_observation_v1(
    bytes: &[u8],
) -> Result<ExactCaseObservationProposalV1, ExactStreamError> {
    let mut reader = CanonicalReader::new(bytes)?;
    reader.magic(OBSERVATION_MAGIC_V1, "exact observation")?;
    let observation = read_observation_body(&mut reader)?;
    reader.finish()?;
    let canonical = encode_exact_case_observation_v1(&observation)?;
    if canonical.as_slice() != bytes {
        return Err(ExactStreamError::invalid(
            "exact observation bytes are not the canonical v1 encoding",
        ));
    }
    Ok(observation)
}

fn read_observation_body(
    reader: &mut CanonicalReader<'_>,
) -> Result<ExactCaseObservationProposalV1, ExactStreamError> {
    let classification = decode_classification(reader.u8()?)?;
    let rank = reader.u128()?;
    let ordinal_count = reader.len(MAX_AXES, "CaseId ordinals")?;
    let mut ordinals = Vec::new();
    for _ in 0..ordinal_count {
        ordinals.push(reader.u128()?);
    }
    let validation_receipt_digest = ExactValidationReceiptDigestV1(reader.array_32()?);
    let match_projection = if classification == ExactClosedClassificationV1::AdmissibleMatch {
        let mut budget = ValueBudget::default();
        let key = read_value_slice(reader, &mut budget)?;
        let extrema_count = reader.len(MAX_PROJECTION_FIELDS, "extrema projection")?;
        let mut extrema = Vec::new();
        for _ in 0..extrema_count {
            extrema.push(reader.i64()?);
        }
        let shown = read_value_slice(reader, &mut budget)?;
        let representative_objective = match reader.u8()? {
            0 => None,
            1 => Some(reader.i64()?),
            tag => {
                return Err(ExactStreamError::invalid(format!(
                    "invalid representative-objective option tag {tag}"
                )))
            }
        };
        Some(ExactMatchProjectionV1::new(
            key,
            extrema,
            shown,
            representative_objective,
        )?)
    } else {
        None
    };
    ExactCaseObservationProposalV1::new(
        ExactCanonicalCaseIdV1::new(rank, ordinals),
        classification,
        match_projection,
        validation_receipt_digest,
    )
}

/// Encode one nonempty atomic observation slice. Batch boundaries are journal
/// provenance; normalized semantic identities are derived per observation.
pub(crate) fn encode_exact_case_observation_batch_v1(
    batch: &ExactCaseObservationBatchProposalV1,
) -> Result<Vec<u8>, ExactStreamError> {
    validate_canonical_observation_sequence(&batch.observations)?;
    let mut writer = CanonicalWriter::new();
    writer.bytes(OBSERVATION_BATCH_MAGIC_V1)?;
    writer.len(
        batch.observations.len(),
        MAX_OBSERVATIONS_PER_BATCH,
        "exact observations",
    )?;
    for observation in batch.observations.iter() {
        write_observation_body(&mut writer, observation)?;
    }
    Ok(writer.finish())
}

/// Decode an untrusted observation slice. Nothing returned by this function is
/// reducer-acceptable until the evaluator validation boundary seals it.
pub(crate) fn decode_exact_case_observation_batch_v1(
    bytes: &[u8],
) -> Result<ExactCaseObservationBatchProposalV1, ExactStreamError> {
    let mut reader = CanonicalReader::new(bytes)?;
    reader.magic(OBSERVATION_BATCH_MAGIC_V1, "exact observation batch")?;
    let observation_count = reader.len(MAX_OBSERVATIONS_PER_BATCH, "exact observations")?;
    if observation_count == 0 {
        return Err(ExactStreamError::invalid(
            "exact observation batch must not be empty",
        ));
    }
    let mut observations = Vec::new();
    for _ in 0..observation_count {
        observations.push(read_observation_body(&mut reader)?);
    }
    reader.finish()?;
    let batch = ExactCaseObservationBatchProposalV1::new(observations)?;
    let canonical = encode_exact_case_observation_batch_v1(&batch)?;
    if canonical.as_slice() != bytes {
        return Err(ExactStreamError::invalid(
            "exact observation batch bytes are not the canonical v1 encoding",
        ));
    }
    Ok(batch)
}

/// Encode one canonical, internally disjoint closed-region batch.
pub(crate) fn encode_exact_closed_region_batch_v1(
    batch: &ExactClosedRegionBatchProposalV1,
) -> Result<Vec<u8>, ExactStreamError> {
    validate_normalized_region_sequence(&batch.regions)?;
    let mut writer = CanonicalWriter::new();
    writer.bytes(CLOSED_REGION_BATCH_MAGIC_V1)?;
    writer.len(batch.regions.len(), MAX_REGIONS_PER_BATCH, "closed regions")?;
    for region in batch.regions.iter() {
        writer.u8(region_kind_tag(region.kind))?;
        writer.u8(classification_tag(region.classification))?;
        writer.u128(region.start_rank)?;
        writer.u128(region.end_rank_exclusive)?;
        writer.len(
            region.validation_receipt_digests.len(),
            MAX_REGIONS_PER_BATCH,
            "closed-region validation receipts",
        )?;
        for receipt in region.validation_receipt_digests.iter() {
            writer.bytes(&receipt.0)?;
        }
    }
    Ok(writer.finish())
}

/// Decode only the unique canonical v1 closed-region byte representation.
pub(crate) fn decode_exact_closed_region_batch_v1(
    bytes: &[u8],
) -> Result<ExactClosedRegionBatchProposalV1, ExactStreamError> {
    let mut reader = CanonicalReader::new(bytes)?;
    reader.magic(CLOSED_REGION_BATCH_MAGIC_V1, "closed-region batch")?;
    let region_count = reader.len(MAX_REGIONS_PER_BATCH, "closed regions")?;
    if region_count == 0 {
        return Err(ExactStreamError::invalid(
            "closed-region batch must not be empty",
        ));
    }
    let mut regions = Vec::new();
    for _ in 0..region_count {
        let kind = decode_region_kind(reader.u8()?)?;
        let classification = decode_classification(reader.u8()?)?;
        let start_rank = reader.u128()?;
        let end_rank_exclusive = reader.u128()?;
        let receipt_count =
            reader.len(MAX_REGIONS_PER_BATCH, "closed-region validation receipts")?;
        if receipt_count == 0 {
            return Err(ExactStreamError::invalid(
                "closed region must retain at least one validation receipt",
            ));
        }
        let mut validation_receipt_digests = Vec::new();
        for _ in 0..receipt_count {
            validation_receipt_digests.push(ExactValidationReceiptDigestV1(reader.array_32()?));
        }
        regions.push(ExactClosedRankRegionProposalV1::from_receipts(
            start_rank,
            end_rank_exclusive,
            kind,
            classification,
            validation_receipt_digests,
        )?);
    }
    reader.finish()?;
    let batch = ExactClosedRegionBatchProposalV1::new(regions)?;
    let canonical = encode_exact_closed_region_batch_v1(&batch)?;
    if canonical.as_slice() != bytes {
        return Err(ExactStreamError::invalid(
            "closed-region bytes are not the canonical v1 encoding",
        ));
    }
    Ok(batch)
}

fn classification_tag(classification: ExactClosedClassificationV1) -> u8 {
    match classification {
        ExactClosedClassificationV1::Excluded => 0,
        ExactClosedClassificationV1::AdmissibleNonmatch => 1,
        ExactClosedClassificationV1::AdmissibleMatch => 2,
    }
}

fn decode_classification(tag: u8) -> Result<ExactClosedClassificationV1, ExactStreamError> {
    match tag {
        0 => Ok(ExactClosedClassificationV1::Excluded),
        1 => Ok(ExactClosedClassificationV1::AdmissibleNonmatch),
        2 => Ok(ExactClosedClassificationV1::AdmissibleMatch),
        _ => Err(ExactStreamError::invalid(format!(
            "invalid closed-classification tag {tag}"
        ))),
    }
}

fn region_kind_tag(kind: ExactClosedRegionKindV1) -> u8 {
    match kind {
        ExactClosedRegionKindV1::Proof => 0,
        ExactClosedRegionKindV1::Structural => 1,
    }
}

fn decode_region_kind(tag: u8) -> Result<ExactClosedRegionKindV1, ExactStreamError> {
    match tag {
        0 => Ok(ExactClosedRegionKindV1::Proof),
        1 => Ok(ExactClosedRegionKindV1::Structural),
        _ => Err(ExactStreamError::invalid(format!(
            "invalid closed-region kind tag {tag}"
        ))),
    }
}

struct CanonicalWriter {
    bytes: Vec<u8>,
}

impl CanonicalWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ExactStreamError> {
        let next_len = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| ExactStreamError::invalid("canonical record byte count overflow"))?;
        if next_len > MAX_RECORD_BYTES {
            return Err(ExactStreamError::invalid(format!(
                "canonical record exceeds {MAX_RECORD_BYTES} bytes"
            )));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), ExactStreamError> {
        self.bytes(&[value])
    }

    fn u32(&mut self, value: u32) -> Result<(), ExactStreamError> {
        self.bytes(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ExactStreamError> {
        self.bytes(&value.to_be_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), ExactStreamError> {
        self.bytes(&value.to_be_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<(), ExactStreamError> {
        self.bytes(&value.to_be_bytes())
    }

    fn len(&mut self, value: usize, limit: usize, name: &str) -> Result<(), ExactStreamError> {
        validate_sequence_len(name, value, limit)?;
        let value = u32::try_from(value)
            .map_err(|_| ExactStreamError::invalid(format!("{name} length exceeds u32::MAX")))?;
        self.u32(value)
    }
}

struct CanonicalReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> CanonicalReader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, ExactStreamError> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(ExactStreamError::invalid(format!(
                "canonical record has {} bytes; limit is {MAX_RECORD_BYTES}",
                bytes.len()
            )));
        }
        Ok(Self { bytes, position: 0 })
    }

    fn finish(&self) -> Result<(), ExactStreamError> {
        if self.position != self.bytes.len() {
            return Err(ExactStreamError::invalid(format!(
                "canonical record has {} trailing bytes",
                self.bytes.len() - self.position
            )));
        }
        Ok(())
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ExactStreamError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| ExactStreamError::invalid("canonical record offset overflow"))?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| {
            ExactStreamError::invalid(format!(
                "canonical record ended at byte {} while reading {count} bytes",
                self.position
            ))
        })?;
        self.position = end;
        Ok(value)
    }

    fn magic(&mut self, expected: &[u8], name: &str) -> Result<(), ExactStreamError> {
        if self.take(expected.len())? != expected {
            return Err(ExactStreamError::invalid(format!(
                "invalid {name} v1 magic"
            )));
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, ExactStreamError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, ExactStreamError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(u32::from_be_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, ExactStreamError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(bytes))
    }

    fn u128(&mut self) -> Result<u128, ExactStreamError> {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(self.take(16)?);
        Ok(u128::from_be_bytes(bytes))
    }

    fn i64(&mut self) -> Result<i64, ExactStreamError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(i64::from_be_bytes(bytes))
    }

    fn array_32(&mut self) -> Result<[u8; 32], ExactStreamError> {
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(self.take(32)?);
        Ok(bytes)
    }

    fn len(&mut self, limit: usize, name: &str) -> Result<usize, ExactStreamError> {
        let value = self.u32()? as usize;
        validate_sequence_len(name, value, limit)?;
        Ok(value)
    }
}

fn write_text(
    writer: &mut CanonicalWriter,
    budget: &mut ValueBudget,
    value: &str,
) -> Result<(), ExactStreamError> {
    if value.len() > MAX_STRING_BYTES {
        return Err(ExactStreamError::invalid(format!(
            "string has {} UTF-8 bytes; limit is {MAX_STRING_BYTES}",
            value.len()
        )));
    }
    budget.bytes(4 + value.len())?;
    writer.len(value.len(), MAX_STRING_BYTES, "string")?;
    writer.bytes(value.as_bytes())
}

fn read_text(
    reader: &mut CanonicalReader<'_>,
    budget: &mut ValueBudget,
) -> Result<String, ExactStreamError> {
    let len = reader.len(MAX_STRING_BYTES, "string")?;
    budget.bytes(4 + len)?;
    let value = std::str::from_utf8(reader.take(len)?)
        .map_err(|_| ExactStreamError::invalid("canonical string is not valid UTF-8"))?;
    Ok(value.to_string())
}

fn write_value_slice(
    writer: &mut CanonicalWriter,
    budget: &mut ValueBudget,
    values: &[ExploreValue],
) -> Result<(), ExactStreamError> {
    writer.len(values.len(), MAX_PROJECTION_FIELDS, "value projection")?;
    for value in values {
        write_value(writer, budget, value, 0)?;
    }
    Ok(())
}

fn read_value_slice(
    reader: &mut CanonicalReader<'_>,
    budget: &mut ValueBudget,
) -> Result<Box<[ExploreValue]>, ExactStreamError> {
    let count = reader.len(MAX_PROJECTION_FIELDS, "value projection")?;
    let mut values = Vec::new();
    for _ in 0..count {
        values.push(read_value(reader, budget, 0)?);
    }
    Ok(values.into_boxed_slice())
}

fn write_value(
    writer: &mut CanonicalWriter,
    budget: &mut ValueBudget,
    value: &ExploreValue,
    depth: usize,
) -> Result<(), ExactStreamError> {
    budget.node(depth)?;
    budget.bytes(1)?;
    match value {
        ExploreValue::Int(value) => {
            writer.u8(0)?;
            budget.bytes(8)?;
            writer.i64(*value)
        }
        ExploreValue::FloatBits(value) => {
            writer.u8(1)?;
            budget.bytes(8)?;
            writer.u64(*value)
        }
        ExploreValue::String(value) => {
            writer.u8(2)?;
            write_text(writer, budget, value)
        }
        ExploreValue::Character(value) => {
            writer.u8(3)?;
            budget.bytes(4)?;
            writer.u32(u32::from(*value))
        }
        ExploreValue::Boolean(value) => {
            writer.u8(4)?;
            budget.bytes(1)?;
            writer.u8(u8::from(*value))
        }
        ExploreValue::Unit => writer.u8(5),
        ExploreValue::List(values) => {
            writer.u8(6)?;
            write_nested_values(writer, budget, values, depth)
        }
        ExploreValue::Set(values) => {
            validate_runtime_set_order(values)?;
            writer.u8(7)?;
            write_nested_values(writer, budget, values, depth)
        }
        ExploreValue::Tuple(values) => {
            writer.u8(8)?;
            write_nested_values(writer, budget, values, depth)
        }
        ExploreValue::Constructor {
            type_name,
            variant,
            positional,
            fields,
        } => {
            writer.u8(9)?;
            write_text(writer, budget, type_name)?;
            write_text(writer, budget, variant)?;
            budget.bytes(5)?;
            writer.u8(u8::from(*positional))?;
            writer.len(fields.len(), MAX_SEQUENCE_ITEMS, "constructor fields")?;
            for (name, value) in fields {
                write_text(writer, budget, name)?;
                write_value(writer, budget, value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn write_nested_values(
    writer: &mut CanonicalWriter,
    budget: &mut ValueBudget,
    values: &[ExploreValue],
    depth: usize,
) -> Result<(), ExactStreamError> {
    budget.bytes(4)?;
    writer.len(values.len(), MAX_SEQUENCE_ITEMS, "ExploreValue sequence")?;
    for value in values {
        write_value(writer, budget, value, depth + 1)?;
    }
    Ok(())
}

fn read_value(
    reader: &mut CanonicalReader<'_>,
    budget: &mut ValueBudget,
    depth: usize,
) -> Result<ExploreValue, ExactStreamError> {
    budget.node(depth)?;
    budget.bytes(1)?;
    match reader.u8()? {
        0 => {
            budget.bytes(8)?;
            Ok(ExploreValue::Int(reader.i64()?))
        }
        1 => {
            budget.bytes(8)?;
            Ok(ExploreValue::FloatBits(reader.u64()?))
        }
        2 => Ok(ExploreValue::String(read_text(reader, budget)?)),
        3 => {
            budget.bytes(4)?;
            let scalar = reader.u32()?;
            let value = char::from_u32(scalar).ok_or_else(|| {
                ExactStreamError::invalid(format!(
                    "ExploreValue character scalar {scalar} is invalid"
                ))
            })?;
            Ok(ExploreValue::Character(value))
        }
        4 => {
            budget.bytes(1)?;
            match reader.u8()? {
                0 => Ok(ExploreValue::Boolean(false)),
                1 => Ok(ExploreValue::Boolean(true)),
                tag => Err(ExactStreamError::invalid(format!(
                    "invalid ExploreValue Boolean tag {tag}"
                ))),
            }
        }
        5 => Ok(ExploreValue::Unit),
        6 => Ok(ExploreValue::List(read_nested_values(
            reader, budget, depth,
        )?)),
        7 => {
            let values = read_nested_values(reader, budget, depth)?;
            validate_runtime_set_order(&values)?;
            Ok(ExploreValue::Set(values))
        }
        8 => Ok(ExploreValue::Tuple(read_nested_values(
            reader, budget, depth,
        )?)),
        9 => {
            let type_name = read_text(reader, budget)?;
            let variant = read_text(reader, budget)?;
            budget.bytes(5)?;
            let positional = match reader.u8()? {
                0 => false,
                1 => true,
                tag => {
                    return Err(ExactStreamError::invalid(format!(
                        "invalid constructor positional tag {tag}"
                    )))
                }
            };
            let field_count = reader.len(MAX_SEQUENCE_ITEMS, "constructor fields")?;
            let mut fields = Vec::new();
            for _ in 0..field_count {
                let name = read_text(reader, budget)?;
                let value = read_value(reader, budget, depth + 1)?;
                fields.push((name, value));
            }
            Ok(ExploreValue::Constructor {
                type_name,
                variant,
                positional,
                fields,
            })
        }
        tag => Err(ExactStreamError::invalid(format!(
            "invalid ExploreValue tag {tag}"
        ))),
    }
}

fn read_nested_values(
    reader: &mut CanonicalReader<'_>,
    budget: &mut ValueBudget,
    depth: usize,
) -> Result<Vec<ExploreValue>, ExactStreamError> {
    budget.bytes(4)?;
    let count = reader.len(MAX_SEQUENCE_ITEMS, "ExploreValue sequence")?;
    let mut values = Vec::new();
    for _ in 0..count {
        values.push(read_value(reader, budget, depth + 1)?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(byte: u8) -> ExactValidationReceiptDigestV1 {
        ExactValidationReceiptDigestV1::new([byte; 32])
    }

    fn projection(
        key: i64,
        extrema: i64,
        shown: &str,
        objective: Option<i64>,
    ) -> ExactMatchProjectionV1 {
        ExactMatchProjectionV1::new(
            vec![ExploreValue::Int(key)],
            vec![extrema],
            vec![ExploreValue::String(shown.to_string())],
            objective,
        )
        .unwrap()
    }

    fn matching(
        reducer: &ExactEvidenceReducer,
        rank: u128,
        key: i64,
        extrema: i64,
        shown: &str,
        objective: Option<i64>,
    ) -> ValidatedExactCaseObservationV1 {
        validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                reducer.canonical_case_id_at_rank(rank).unwrap(),
                ExactClosedClassificationV1::AdmissibleMatch,
                Some(projection(key, extrema, shown, objective)),
                receipt(rank as u8),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn classified_singleton(
        reducer: &ExactEvidenceReducer,
        rank: u128,
        classification: ExactClosedClassificationV1,
    ) -> ValidatedExactCaseObservationV1 {
        assert_ne!(classification, ExactClosedClassificationV1::AdmissibleMatch);
        validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                reducer.canonical_case_id_at_rank(rank).unwrap(),
                classification,
                None,
                receipt((rank as u8).wrapping_add(31)),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn interval_pairs(support: &ExactCaseSupport) -> Vec<(u128, u128)> {
        support
            .intervals()
            .into_iter()
            .map(|interval| (interval.start(), interval.end_exclusive()))
            .collect()
    }

    fn observation_batch(
        observations: impl IntoIterator<Item = ValidatedExactCaseObservationV1>,
    ) -> ValidatedExactCaseObservationBatchV1 {
        let proposal = ExactCaseObservationBatchProposalV1::new(
            observations
                .into_iter()
                .map(|observation| observation.proposal().clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        validation_boundary::test_only_seal_observation_batch(proposal).unwrap()
    }

    fn region_batch(
        regions: impl IntoIterator<Item = ValidatedExactClosedRankRegionV1>,
    ) -> ValidatedExactClosedRegionBatchV1 {
        let proposal = ExactClosedRegionBatchProposalV1::new(
            regions
                .into_iter()
                .map(|region| region.proposal().clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        validation_boundary::test_only_seal_region_batch(proposal).unwrap()
    }

    fn sealed_region(
        start_rank: u128,
        end_rank_exclusive: u128,
        kind: ExactClosedRegionKindV1,
        classification: ExactClosedClassificationV1,
        receipt_byte: u8,
    ) -> ValidatedExactClosedRankRegionV1 {
        let proposal =
            ExactClosedRegionBatchProposalV1::new(vec![ExactClosedRankRegionProposalV1::new(
                start_rank,
                end_rank_exclusive,
                kind,
                classification,
                receipt(receipt_byte),
            )
            .unwrap()])
            .unwrap();
        validation_boundary::test_only_seal_region_batch(proposal)
            .unwrap()
            .regions
            .into_vec()
            .pop()
            .unwrap()
    }

    #[test]
    fn replay_aggregates_do_not_depend_on_arrival_order() {
        let shape = ExactProjectionShapeV1::new(1, 1, 1).unwrap();
        let mut forward = ExactEvidenceReducer::new(
            vec![2, 3],
            shape,
            ExactRepresentativePolicyV1::Maximize,
            true,
        )
        .unwrap();
        let mut reverse = ExactEvidenceReducer::new(
            vec![2, 3],
            shape,
            ExactRepresentativePolicyV1::Maximize,
            true,
        )
        .unwrap();

        let excluded = sealed_region(
            0,
            2,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            90,
        );
        let nonmatch = sealed_region(
            5,
            6,
            ExactClosedRegionKindV1::Proof,
            ExactClosedClassificationV1::AdmissibleNonmatch,
            91,
        );
        let observations = [
            matching(&forward, 2, 7, 40, "rank-two", Some(9)),
            matching(&forward, 3, 7, 20, "rank-three", Some(11)),
            matching(&forward, 4, 7, 20, "rank-four", Some(11)),
        ];

        forward.accept_closed_region(excluded.clone()).unwrap();
        for observation in observations.iter().cloned() {
            forward.accept_observation(observation).unwrap();
        }
        forward.accept_closed_region(nonmatch.clone()).unwrap();

        reverse.accept_closed_region(nonmatch).unwrap();
        for observation in observations.iter().rev().cloned() {
            reverse.accept_observation(observation).unwrap();
        }
        reverse.accept_closed_region(excluded).unwrap();

        let expected = forward.snapshot();
        assert_eq!(reverse.snapshot(), expected);
        assert_eq!(expected.closed_case_count, 6);
        assert_eq!(expected.matching.exact, Some(3));
        assert!(expected.projection_complete);
        assert_eq!(expected.results.len(), 1);
        let result = &expected.results[0];
        assert_eq!(result.support.exact, Some(3));
        assert_eq!(result.representative_case_id.rank, 3);
        assert_eq!(result.extrema[0].minimum, 20);
        assert_eq!(result.extrema[0].minimum_tie_support, 2);
        assert_eq!(result.extrema[0].minimum_witness.rank, 3);
        assert_eq!(result.extrema[0].maximum, 40);
        assert_eq!(
            expected
                .matching_ledger
                .as_ref()
                .unwrap()
                .observations
                .iter()
                .map(|observation| observation.case_id.rank)
                .collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }

    #[test]
    fn classification_supports_do_not_depend_on_arrival_order() {
        let shape = ExactProjectionShapeV1::new(1, 1, 1).unwrap();
        let mut forward =
            ExactEvidenceReducer::new(vec![12], shape, ExactRepresentativePolicyV1::First, false)
                .unwrap();
        let mut reverse =
            ExactEvidenceReducer::new(vec![12], shape, ExactRepresentativePolicyV1::First, false)
                .unwrap();

        let excluded_region = sealed_region(
            0,
            2,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            70,
        );
        let nonmatch_region = sealed_region(
            10,
            12,
            ExactClosedRegionKindV1::Proof,
            ExactClosedClassificationV1::AdmissibleNonmatch,
            71,
        );
        let observations = [
            matching(&forward, 2, 1, 20, "first-match", None),
            classified_singleton(&forward, 4, ExactClosedClassificationV1::Excluded),
            classified_singleton(&forward, 5, ExactClosedClassificationV1::AdmissibleNonmatch),
            matching(&forward, 7, 1, 10, "second-match", None),
        ];

        forward
            .accept_closed_region(excluded_region.clone())
            .unwrap();
        for observation in observations.iter().cloned() {
            forward.accept_observation(observation).unwrap();
        }
        forward
            .accept_closed_region(nonmatch_region.clone())
            .unwrap();

        reverse.accept_closed_region(nonmatch_region).unwrap();
        for observation in observations.iter().rev().cloned() {
            reverse.accept_observation(observation).unwrap();
        }
        reverse.accept_closed_region(excluded_region).unwrap();

        let forward_supports = forward
            .classification_supports_bounded(16)
            .unwrap()
            .unwrap();
        let reverse_supports = reverse
            .classification_supports_bounded(16)
            .unwrap()
            .unwrap();
        assert_eq!(forward_supports, reverse_supports);
        assert_eq!(
            interval_pairs(forward_supports.closed()),
            vec![(0, 3), (4, 6), (7, 8), (10, 12)]
        );
    }

    #[test]
    fn authoritative_matching_support_is_closure_gated_and_arrival_order_independent() {
        let shape = ExactProjectionShapeV1::new(1, 1, 1).unwrap();
        let mut forward =
            ExactEvidenceReducer::new(vec![8], shape, ExactRepresentativePolicyV1::First, false)
                .unwrap();
        let mut reverse =
            ExactEvidenceReducer::new(vec![8], shape, ExactRepresentativePolicyV1::First, false)
                .unwrap();

        let low_excluded = sealed_region(
            0,
            2,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            101,
        );
        let middle_nonmatch = sealed_region(
            3,
            5,
            ExactClosedRegionKindV1::Proof,
            ExactClosedClassificationV1::AdmissibleNonmatch,
            102,
        );
        let high_excluded = sealed_region(
            6,
            8,
            ExactClosedRegionKindV1::Structural,
            ExactClosedClassificationV1::Excluded,
            103,
        );

        forward
            .accept_closed_region_batch(&region_batch([
                low_excluded.clone(),
                middle_nonmatch.clone(),
                high_excluded.clone(),
            ]))
            .unwrap();
        let forward_low_match = matching(&forward, 2, 1, 2, "low", None);
        forward.accept_observation(forward_low_match).unwrap();
        assert_eq!(
            interval_pairs(&forward.confirmed_admissible_match_support()),
            vec![(2, 3)]
        );
        assert!(forward.authoritative_admissible_match_support().is_none());
        let forward_high_match = matching(&forward, 5, 1, 5, "high", None);
        forward.accept_observation(forward_high_match).unwrap();

        let reverse_high_match = matching(&reverse, 5, 1, 5, "high", None);
        reverse.accept_observation(reverse_high_match).unwrap();
        reverse.accept_closed_region(high_excluded).unwrap();
        reverse.accept_closed_region(middle_nonmatch).unwrap();
        let reverse_low_match = matching(&reverse, 2, 1, 2, "low", None);
        reverse.accept_observation(reverse_low_match).unwrap();
        assert!(reverse.authoritative_admissible_match_support().is_none());
        reverse.accept_closed_region(low_excluded).unwrap();

        let forward_target = forward
            .authoritative_admissible_match_support()
            .expect("complete classification has an authoritative target");
        let reverse_target = reverse
            .authoritative_admissible_match_support()
            .expect("complete classification has an authoritative target");
        assert_eq!(forward_target, reverse_target);
        assert_eq!(forward_target.case_count(), 2);
        assert_eq!(
            interval_pairs(forward_target.support()),
            vec![(2, 3), (5, 6)]
        );
    }

    #[test]
    fn mixed_regions_and_singletons_form_one_exact_classification_partition() {
        let mut reducer = ExactEvidenceReducer::new(
            vec![10],
            ExactProjectionShapeV1::new(1, 1, 1).unwrap(),
            ExactRepresentativePolicyV1::First,
            false,
        )
        .unwrap();
        let regions = region_batch([
            sealed_region(
                0,
                3,
                ExactClosedRegionKindV1::Structural,
                ExactClosedClassificationV1::Excluded,
                80,
            ),
            sealed_region(
                6,
                9,
                ExactClosedRegionKindV1::Proof,
                ExactClosedClassificationV1::AdmissibleNonmatch,
                81,
            ),
        ]);
        reducer.accept_closed_region_batch(&regions).unwrap();
        let rank_three = classified_singleton(&reducer, 3, ExactClosedClassificationV1::Excluded);
        let rank_four = matching(&reducer, 4, 9, 4, "lower", None);
        let rank_five =
            classified_singleton(&reducer, 5, ExactClosedClassificationV1::AdmissibleNonmatch);
        let rank_nine = matching(&reducer, 9, 9, 9, "upper", None);
        let observations = observation_batch([rank_three, rank_four, rank_five, rank_nine]);
        reducer.accept_observation_batch(&observations).unwrap();

        assert!(reducer
            .classification_supports_bounded(4)
            .unwrap()
            .is_none());
        let supports = reducer.classification_supports_bounded(5).unwrap().unwrap();
        assert_eq!(interval_pairs(supports.closed()), vec![(0, 10)]);
        assert_eq!(
            interval_pairs(supports.support(ExactClosedClassificationV1::Excluded)),
            vec![(0, 4)]
        );
        assert_eq!(
            interval_pairs(supports.support(ExactClosedClassificationV1::AdmissibleNonmatch)),
            vec![(5, 9)]
        );
        assert_eq!(
            interval_pairs(supports.support(ExactClosedClassificationV1::AdmissibleMatch)),
            vec![(4, 5), (9, 10)]
        );

        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.closed_case_count, 10);
        assert_eq!(snapshot.excluded.exact, Some(4));
        assert_eq!(snapshot.admissible.exact, Some(6));
        assert_eq!(snapshot.matching.exact, Some(2));
    }

    #[test]
    fn alternating_classifications_preserve_fragmented_fibers_and_compact_union() {
        let mut reducer = ExactEvidenceReducer::new(
            vec![16],
            ExactProjectionShapeV1::new(0, 0, 0).unwrap(),
            ExactRepresentativePolicyV1::First,
            false,
        )
        .unwrap();

        for rank in (0_u128..16).step_by(2) {
            let observation =
                classified_singleton(&reducer, rank, ExactClosedClassificationV1::Excluded);
            reducer.accept_observation(observation).unwrap();
        }
        for rank in (0_u128..8).rev().map(|index| index * 2 + 1) {
            let observation = classified_singleton(
                &reducer,
                rank,
                ExactClosedClassificationV1::AdmissibleNonmatch,
            );
            reducer.accept_observation(observation).unwrap();
        }

        assert!(reducer
            .classification_supports_bounded(16)
            .unwrap()
            .is_none());
        let supports = reducer
            .classification_supports_bounded(17)
            .unwrap()
            .unwrap();
        assert_eq!(supports.closed().interval_count(), 1);
        assert_eq!(
            supports
                .support(ExactClosedClassificationV1::Excluded)
                .interval_count(),
            8
        );
        assert_eq!(
            supports
                .support(ExactClosedClassificationV1::AdmissibleNonmatch)
                .interval_count(),
            8
        );
        assert_eq!(
            supports
                .support(ExactClosedClassificationV1::AdmissibleMatch)
                .interval_count(),
            0
        );
        assert_eq!(interval_pairs(supports.closed()), vec![(0, 16)]);
        assert_eq!(reducer.snapshot().closed_case_count, 16);
    }

    #[test]
    fn proof_match_region_cannot_be_sealed_in_v1() {
        let reducer = ExactEvidenceReducer::new(
            vec![4],
            ExactProjectionShapeV1::new(1, 1, 1).unwrap(),
            ExactRepresentativePolicyV1::First,
            true,
        )
        .unwrap();
        let proposal =
            ExactClosedRegionBatchProposalV1::new(vec![ExactClosedRankRegionProposalV1::new(
                0,
                4,
                ExactClosedRegionKindV1::Proof,
                ExactClosedClassificationV1::AdmissibleMatch,
                receipt(3),
            )
            .unwrap()])
            .unwrap();
        assert!(validation_boundary::test_only_seal_region_batch(proposal).is_err());

        let snapshot = reducer.snapshot();
        assert_eq!(snapshot.matching.exact, None);
        assert_eq!(snapshot.open_case_count, 4);
        assert_eq!(snapshot.unprojected_matching_case_count, 0);
        assert!(!snapshot.projection_complete);
        assert!(snapshot.results.is_empty());
        assert!(!snapshot.matching_ledger.unwrap().complete);
    }

    #[test]
    fn duplicate_or_inconsistent_case_identity_is_rejected_atomically() {
        let mut reducer = ExactEvidenceReducer::new(
            vec![2, 3],
            ExactProjectionShapeV1::new(0, 0, 0).unwrap(),
            ExactRepresentativePolicyV1::First,
            false,
        )
        .unwrap();
        let observation = validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(4, vec![1, 1]),
                ExactClosedClassificationV1::Excluded,
                None,
                receipt(1),
            )
            .unwrap(),
        )
        .unwrap();
        reducer.accept_observation(observation.clone()).unwrap();
        let before = reducer.snapshot();
        assert!(reducer.accept_observation(observation).is_err());
        assert_eq!(reducer.snapshot(), before);

        let inconsistent = validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(3, vec![1, 1]),
                ExactClosedClassificationV1::Excluded,
                None,
                receipt(2),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(reducer.accept_observation(inconsistent).is_err());
        assert_eq!(reducer.snapshot(), before);
    }

    #[test]
    fn observation_codec_round_trips_nested_values_and_rejects_trailing_bytes() {
        let value = ExploreValue::Constructor {
            type_name: "Example".to_string(),
            variant: "Some".to_string(),
            positional: true,
            fields: vec![(
                "0".to_string(),
                ExploreValue::Tuple(vec![
                    ExploreValue::FloatBits(f64::NAN.to_bits()),
                    ExploreValue::Character('🚀'),
                    ExploreValue::Boolean(true),
                    ExploreValue::Unit,
                    ExploreValue::List(vec![ExploreValue::Int(-9)]),
                    ExploreValue::Set(vec![ExploreValue::Int(1), ExploreValue::Int(2)]),
                ]),
            )],
        };
        let observation = ExactCaseObservationProposalV1::new(
            ExactCanonicalCaseIdV1::new(7, vec![1, 3]),
            ExactClosedClassificationV1::AdmissibleMatch,
            Some(
                ExactMatchProjectionV1::new(
                    vec![value],
                    vec![i64::MIN, i64::MAX],
                    vec![ExploreValue::String("shown".to_string())],
                    Some(42),
                )
                .unwrap(),
            ),
            receipt(8),
        )
        .unwrap();
        let encoded = encode_exact_case_observation_v1(&observation).unwrap();
        assert_eq!(
            decode_exact_case_observation_v1(&encoded).unwrap(),
            observation
        );

        let mut with_trailing = encoded;
        with_trailing.push(0);
        assert!(decode_exact_case_observation_v1(&with_trailing).is_err());
    }

    #[test]
    fn closed_region_batch_codec_is_canonical_and_bounded() {
        let batch = ExactClosedRegionBatchProposalV1::new(vec![
            ExactClosedRankRegionProposalV1::new(
                9,
                11,
                ExactClosedRegionKindV1::Proof,
                ExactClosedClassificationV1::AdmissibleNonmatch,
                receipt(2),
            )
            .unwrap(),
            ExactClosedRankRegionProposalV1::new(
                1,
                3,
                ExactClosedRegionKindV1::Structural,
                ExactClosedClassificationV1::Excluded,
                receipt(1),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(batch.regions[0].start_rank, 1);
        let encoded = encode_exact_closed_region_batch_v1(&batch).unwrap();
        assert_eq!(
            decode_exact_closed_region_batch_v1(&encoded).unwrap(),
            batch
        );

        let mut bad_tag = encoded;
        bad_tag[12] = 99;
        assert!(decode_exact_closed_region_batch_v1(&bad_tag).is_err());
    }

    #[test]
    fn semantic_digest_excludes_rank_and_validation_receipt() {
        let left = validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(1, vec![1]),
                ExactClosedClassificationV1::AdmissibleMatch,
                Some(projection(7, 20, "same", Some(9))),
                receipt(1),
            )
            .unwrap(),
        )
        .unwrap();
        let right = validation_boundary::test_only_seal_observation(
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(9, vec![9]),
                ExactClosedClassificationV1::AdmissibleMatch,
                Some(projection(7, 20, "same", Some(9))),
                receipt(2),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(left.semantic_fact_digest(), right.semantic_fact_digest());
        assert_ne!(
            left.proposal().validation_receipt_digest,
            right.proposal().validation_receipt_digest
        );
    }

    #[test]
    fn observation_batch_is_canonical_and_reducer_rejection_is_atomic() {
        let mut reducer = ExactEvidenceReducer::new(
            vec![3],
            ExactProjectionShapeV1::new(0, 0, 0).unwrap(),
            ExactRepresentativePolicyV1::First,
            false,
        )
        .unwrap();
        let proposal = ExactCaseObservationBatchProposalV1::new(vec![
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(2, vec![2]),
                ExactClosedClassificationV1::Excluded,
                None,
                receipt(2),
            )
            .unwrap(),
            ExactCaseObservationProposalV1::new(
                ExactCanonicalCaseIdV1::new(1, vec![99]),
                ExactClosedClassificationV1::Excluded,
                None,
                receipt(1),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(proposal.observations[0].case_id.rank, 1);
        let encoded = encode_exact_case_observation_batch_v1(&proposal).unwrap();
        assert_eq!(
            decode_exact_case_observation_batch_v1(&encoded).unwrap(),
            proposal
        );
        let validated = validation_boundary::test_only_seal_observation_batch(proposal).unwrap();
        let before = reducer.snapshot();
        assert!(reducer.accept_observation_batch(&validated).is_err());
        assert_eq!(reducer.snapshot(), before);
    }

    #[test]
    fn observation_batch_rejects_duplicate_ranks_before_sealing() {
        let observations = [1_u8, 2_u8]
            .into_iter()
            .map(|receipt_byte| {
                ExactCaseObservationProposalV1::new(
                    ExactCanonicalCaseIdV1::new(0, vec![0]),
                    ExactClosedClassificationV1::Excluded,
                    None,
                    receipt(receipt_byte),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(ExactCaseObservationBatchProposalV1::new(observations).is_err());
        assert!(ExactCaseObservationBatchProposalV1::new(Vec::new()).is_err());
        assert!(ExactClosedRegionBatchProposalV1::new(Vec::new()).is_err());
    }

    #[test]
    fn region_batch_coalesces_adjacent_equal_proposals() {
        let batch = ExactClosedRegionBatchProposalV1::new(vec![
            ExactClosedRankRegionProposalV1::new(
                0,
                2,
                ExactClosedRegionKindV1::Proof,
                ExactClosedClassificationV1::AdmissibleNonmatch,
                receipt(7),
            )
            .unwrap(),
            ExactClosedRankRegionProposalV1::new(
                2,
                5,
                ExactClosedRegionKindV1::Proof,
                ExactClosedClassificationV1::AdmissibleNonmatch,
                receipt(8),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(batch.regions.len(), 1);
        assert_eq!(batch.regions[0].start_rank, 0);
        assert_eq!(batch.regions[0].end_rank_exclusive, 5);
        assert_eq!(batch.regions[0].validation_receipt_digests.len(), 2);
    }

    #[test]
    fn set_and_constructor_validation_match_runtime_canonical_forms() {
        assert!(ExactMatchProjectionV1::new(
            vec![ExploreValue::Set(vec![
                ExploreValue::Int(10),
                ExploreValue::Int(2),
            ])],
            Vec::new(),
            Vec::new(),
            None,
        )
        .is_ok());
        assert!(ExactMatchProjectionV1::new(
            vec![ExploreValue::Set(vec![
                ExploreValue::Int(2),
                ExploreValue::Int(10),
            ])],
            Vec::new(),
            Vec::new(),
            None,
        )
        .is_err());
        assert!(ExactMatchProjectionV1::new(
            vec![ExploreValue::Constructor {
                type_name: "Example".to_string(),
                variant: "Pair".to_string(),
                positional: false,
                fields: vec![
                    ("left".to_string(), ExploreValue::Unit),
                    ("left".to_string(), ExploreValue::Unit),
                ],
            }],
            Vec::new(),
            Vec::new(),
            None,
        )
        .is_err());
    }

    #[test]
    fn value_codec_enforces_explicit_nesting_limit() {
        let mut value = ExploreValue::Unit;
        for _ in 0..=MAX_VALUE_DEPTH {
            value = ExploreValue::List(vec![value]);
        }
        assert!(ExactMatchProjectionV1::new(vec![value], Vec::new(), Vec::new(), None).is_err());
    }
}
