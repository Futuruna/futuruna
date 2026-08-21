//! Owner-local coverage-plan journal state for exact Explore residual work.
//!
//! The journal partitions only a normalized residual support.  Ranks already
//! closed by independently checked certificates are absent from every shard
//! and can never be rematerialized by this module.  Journal coverage therefore
//! means “every residual singleton chunk is durably committed”, not public
//! Explore completion, certificate validity, or mechanism closure.
//!
//! This is deliberately not the pre-probe run genesis or the full observable
//! lifecycle. [`ExploreRunContract`] binds a post-proof residual support and a
//! shard width, so a higher-level run stream must first bind the full semantic
//! case universe, then accept this state as one later coverage-plan epoch. The
//! stream also owns the order-sensitive journal head, arrival-order-independent
//! evidence root, pause/resume records, and terminal seal.
//!
//! This file owns no paths or filesystem operations.  Instead, its state
//! machine models three append-only persistence boundaries:
//!
//! 1. one immutable run-contract header is installed before any journal entry;
//! 2. a prepared lease becomes dispatchable only with a typed receipt for its
//!    exact, small, immutable attempt entry; and
//! 3. a validated completion becomes authoritative only with a typed receipt
//!    for a content-addressed chunk installed with file and directory sync.
//!
//! A compact index may be derived from these entries, but is never authority.
//! The canonical final manifest is streamed once after every residual chunk is
//! authoritative.  Completion and its coverage seal remain unavailable until
//! storage returns a single-use receipt binding the exact manifest generation,
//! content hash, coverage root, and durable write generation.
//!
//! Production resume is intentionally unavailable in this pure core.  A future
//! child storage adapter must reconstruct the immutable header, attempt entries,
//! and chunks synchronously while holding a real, non-`Clone` owner/read RAII
//! guard.  An identity-only receipt that could be delayed across replacement is
//! not an acceptable substitute.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use sha2::{Digest, Sha256};

use super::report::ExploreCaseId;

pub(crate) const EXPLORE_SHARD_WIDTH_V1_MIN: usize = 256;
pub(crate) const EXPLORE_SHARD_WIDTH_V1_MAX: usize = 4096;

const RESIDUAL_SUPPORT_HASH_V1: &[u8] = b"futuruna.explore.residual-support.v1";
const RUN_CONTRACT_HASH_V1: &[u8] = b"futuruna.explore.run-contract.v1";
const SHARD_DESCRIPTOR_HASH_V1: &[u8] = b"futuruna.explore.run-shard.v1";
const ATTEMPT_ENTRY_HASH_V1: &[u8] = b"futuruna.explore.run-attempt.v1";
const SHARD_EVIDENCE_HASH_V1: &[u8] = b"futuruna.explore.run-shard-evidence.v1";
const COVERAGE_HASH_V1: &[u8] = b"futuruna.explore.run-coverage.v1";
const CANONICAL_CHUNK_MAGIC_V1: &[u8] = b"futuruna.explore.singleton-chunk.v1";
const CANONICAL_FINAL_MANIFEST_MAGIC_V1: &[u8] = b"futuruna.explore.final-manifest.v1";

/// Exact observation contract for dynamic mechanisms.
///
/// `None` is itself hash-bound.  It cannot be confused with an old evaluator
/// accidentally omitting requested observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExploreMechanismObservationIdentity {
    None { explicit_none_hash: Box<str> },
    Observe { observation_request_hash: Box<str> },
}

impl ExploreMechanismObservationIdentity {
    fn validate(&self) -> Result<(), ExploreRunStateError> {
        match self {
            Self::None { explicit_none_hash } => {
                require_lowercase_sha256("mechanism_explicit_none_hash", explicit_none_hash)
            }
            Self::Observe {
                observation_request_hash,
            } => require_lowercase_sha256(
                "mechanism_observation_request_hash",
                observation_request_hash,
            ),
        }
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        match self {
            Self::None { explicit_none_hash } => {
                hasher.segment(b"mechanism-none");
                hasher.segment(explicit_none_hash.as_bytes());
            }
            Self::Observe {
                observation_request_hash,
            } => {
                hasher.segment(b"mechanism-observe");
                hasher.segment(observation_request_hash.as_bytes());
            }
        }
    }
}

/// Exact semantic, disclosure, evaluator, and persistence-schema identities.
///
/// The report request, mechanism mode, retained-data authorization, and chunk
/// record schema are independent fields.  None is hidden behind the run-state
/// artifact schema hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreRunIdentity {
    pub(crate) program_hash: Box<str>,
    pub(crate) analysis_program_hash: Box<str>,
    pub(crate) query_hash: Box<str>,
    pub(crate) domain_hash: Box<str>,
    pub(crate) evaluator_contract_hash: Box<str>,
    pub(crate) report_request_hash: Box<str>,
    pub(crate) mechanism_observation: ExploreMechanismObservationIdentity,
    pub(crate) retention_authorization_hash: Box<str>,
    pub(crate) run_state_artifact_schema_hash: Box<str>,
    pub(crate) chunk_record_schema_hash: Box<str>,
}

impl ExploreRunIdentity {
    pub(crate) fn validate(&self) -> Result<(), ExploreRunStateError> {
        for (field, value) in [
            ("program_hash", self.program_hash.as_ref()),
            ("analysis_program_hash", self.analysis_program_hash.as_ref()),
            ("query_hash", self.query_hash.as_ref()),
            ("domain_hash", self.domain_hash.as_ref()),
            (
                "evaluator_contract_hash",
                self.evaluator_contract_hash.as_ref(),
            ),
            ("report_request_hash", self.report_request_hash.as_ref()),
            (
                "retention_authorization_hash",
                self.retention_authorization_hash.as_ref(),
            ),
            (
                "run_state_artifact_schema_hash",
                self.run_state_artifact_schema_hash.as_ref(),
            ),
            (
                "chunk_record_schema_hash",
                self.chunk_record_schema_hash.as_ref(),
            ),
        ] {
            require_lowercase_sha256(field, value)?;
        }
        self.mechanism_observation.validate()
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        for value in [
            self.program_hash.as_ref(),
            self.analysis_program_hash.as_ref(),
            self.query_hash.as_ref(),
            self.domain_hash.as_ref(),
            self.evaluator_contract_hash.as_ref(),
            self.report_request_hash.as_ref(),
            self.retention_authorization_hash.as_ref(),
            self.run_state_artifact_schema_hash.as_ref(),
            self.chunk_record_schema_hash.as_ref(),
        ] {
            hasher.segment(value.as_bytes());
        }
        self.mechanism_observation.hash_into(hasher);
    }
}

/// Canonical finite CaseId universe.  The last declared axis advances fastest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalCaseSpace {
    axis_cardinalities: Box<[u128]>,
    declared_case_count: u128,
}

impl CanonicalCaseSpace {
    pub(crate) fn new(
        axis_cardinalities: impl Into<Box<[u128]>>,
    ) -> Result<Self, ExploreRunStateError> {
        let axis_cardinalities = axis_cardinalities.into();
        let declared_case_count = checked_product(&axis_cardinalities)?;
        Ok(Self {
            axis_cardinalities,
            declared_case_count,
        })
    }

    pub(crate) fn axis_cardinalities(&self) -> &[u128] {
        &self.axis_cardinalities
    }

    pub(crate) fn declared_case_count(&self) -> u128 {
        self.declared_case_count
    }

    pub(crate) fn case_id_at_rank(
        &self,
        rank: u128,
    ) -> Result<ExploreCaseId, ExploreRunStateError> {
        if rank >= self.declared_case_count {
            return Err(invalid(format!(
                "CaseId rank {rank} is outside declared universe {}",
                self.declared_case_count
            )));
        }
        let mut remainder = rank;
        let mut ordinals = vec![0_u128; self.axis_cardinalities.len()];
        for axis in (0..self.axis_cardinalities.len()).rev() {
            let cardinality = self.axis_cardinalities[axis];
            // A zero cardinality makes U zero, rejected above before division.
            ordinals[axis] = remainder % cardinality;
            remainder /= cardinality;
        }
        if remainder != 0 {
            return Err(invalid("CaseId unranking left a nonzero remainder"));
        }
        Ok(ExploreCaseId::new(ordinals))
    }

    pub(crate) fn rank_of_case_id(
        &self,
        case_id: &ExploreCaseId,
    ) -> Result<u128, ExploreRunStateError> {
        if case_id.len() != self.axis_cardinalities.len() {
            return Err(invalid(format!(
                "CaseId has {} axes, expected {}",
                case_id.len(),
                self.axis_cardinalities.len()
            )));
        }
        if self.declared_case_count == 0 {
            return Err(invalid("an empty CaseId universe has no ranks"));
        }
        let mut rank = 0_u128;
        for (axis, (&ordinal, &cardinality)) in case_id
            .ordinals()
            .iter()
            .zip(self.axis_cardinalities.iter())
            .enumerate()
        {
            if ordinal >= cardinality {
                return Err(invalid(format!(
                    "CaseId ordinal {ordinal} is outside axis {axis} cardinality {cardinality}"
                )));
            }
            rank = rank
                .checked_mul(cardinality)
                .and_then(|value| value.checked_add(ordinal))
                .ok_or_else(|| invalid("CaseId rank exceeds u128::MAX"))?;
        }
        if rank >= self.declared_case_count {
            return Err(invalid("CaseId rank is outside the declared universe"));
        }
        Ok(rank)
    }
}

/// One nonempty end-exclusive interval in declared CaseId-rank space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExploreRankInterval {
    start: u128,
    end_exclusive: u128,
}

impl ExploreRankInterval {
    pub(crate) fn start(&self) -> u128 {
        self.start
    }

    pub(crate) fn end_exclusive(&self) -> u128 {
        self.end_exclusive
    }

    pub(crate) fn case_count(&self) -> u128 {
        self.end_exclusive - self.start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreResidualSupportId([u8; 32]);

/// Sorted, disjoint, adjacency-merged residual support within U.
///
/// Input order is immaterial.  Adjacent intervals normalize into one interval;
/// overlapping intervals fail closed because accepting them would conceal
/// competing coverage authorities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreResidualSupport {
    intervals: Box<[ExploreRankInterval]>,
    residual_case_count: u128,
    id: ExploreResidualSupportId,
}

impl ExploreResidualSupport {
    pub(crate) fn new(
        case_space: &CanonicalCaseSpace,
        intervals: impl IntoIterator<Item = (u128, u128)>,
    ) -> Result<Self, ExploreRunStateError> {
        let mut sorted = intervals
            .into_iter()
            .map(|(start, end_exclusive)| {
                if start >= end_exclusive {
                    return Err(invalid(format!(
                        "residual rank interval [{start}, {end_exclusive}) is empty or reversed"
                    )));
                }
                if end_exclusive > case_space.declared_case_count {
                    return Err(invalid(format!(
                        "residual rank interval [{start}, {end_exclusive}) exceeds declared U={}",
                        case_space.declared_case_count
                    )));
                }
                Ok(ExploreRankInterval {
                    start,
                    end_exclusive,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        sorted.sort_unstable();

        let mut normalized = Vec::<ExploreRankInterval>::with_capacity(sorted.len());
        for interval in sorted {
            if let Some(previous) = normalized.last_mut() {
                if interval.start < previous.end_exclusive {
                    return Err(invalid(format!(
                        "residual rank intervals overlap at [{}, {}) and [{}, {})",
                        previous.start,
                        previous.end_exclusive,
                        interval.start,
                        interval.end_exclusive
                    )));
                }
                if interval.start == previous.end_exclusive {
                    previous.end_exclusive = interval.end_exclusive;
                    continue;
                }
            }
            normalized.push(interval);
        }

        let residual_case_count = normalized.iter().try_fold(0_u128, |total, interval| {
            total
                .checked_add(interval.case_count())
                .ok_or_else(|| invalid("residual support count exceeds u128::MAX"))
        })?;
        let id = derive_residual_support_id(case_space.declared_case_count, &normalized);
        Ok(Self {
            intervals: normalized.into_boxed_slice(),
            residual_case_count,
            id,
        })
    }

    pub(crate) fn intervals(&self) -> &[ExploreRankInterval] {
        &self.intervals
    }

    pub(crate) fn residual_case_count(&self) -> u128 {
        self.residual_case_count
    }

    pub(crate) fn id(&self) -> ExploreResidualSupportId {
        self.id
    }
}

/// Checked, `usize`-backed v1 shard width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreShardWidthV1(usize);

impl ExploreShardWidthV1 {
    pub(crate) fn new(width: usize) -> Result<Self, ExploreRunStateError> {
        if !(EXPLORE_SHARD_WIDTH_V1_MIN..=EXPLORE_SHARD_WIDTH_V1_MAX).contains(&width) {
            return Err(invalid(format!(
                "Explore v1 shard width {width} is outside {}..={} cases",
                EXPLORE_SHARD_WIDTH_V1_MIN, EXPLORE_SHARD_WIDTH_V1_MAX
            )));
        }
        Ok(Self(width))
    }

    pub(crate) fn get(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreRunContractId([u8; 32]);

/// Immutable evidence-reuse contract.  Retry ceilings are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreRunContract {
    identity: ExploreRunIdentity,
    case_space: CanonicalCaseSpace,
    residual_support: ExploreResidualSupport,
    shard_width: ExploreShardWidthV1,
    /// Cumulative shard count before each normalized support interval, plus a
    /// final total.  This is the immutable index for O(log N) sparse lookup.
    shard_prefixes: Box<[u128]>,
    shard_count: u128,
    id: ExploreRunContractId,
}

impl ExploreRunContract {
    pub(crate) fn new(
        identity: ExploreRunIdentity,
        axis_cardinalities: impl Into<Box<[u128]>>,
        residual_intervals: impl IntoIterator<Item = (u128, u128)>,
        shard_width: ExploreShardWidthV1,
    ) -> Result<Self, ExploreRunStateError> {
        identity.validate()?;
        let case_space = CanonicalCaseSpace::new(axis_cardinalities)?;
        let residual_support = ExploreResidualSupport::new(&case_space, residual_intervals)?;
        let shard_prefixes = checked_shard_prefixes(&residual_support, shard_width)?;
        let shard_count = shard_prefixes.last().copied().unwrap_or(0);
        let id = derive_run_contract_id(&identity, &case_space, &residual_support, shard_width);
        Ok(Self {
            identity,
            case_space,
            residual_support,
            shard_width,
            shard_prefixes,
            shard_count,
            id,
        })
    }

    pub(crate) fn identity(&self) -> &ExploreRunIdentity {
        &self.identity
    }

    pub(crate) fn case_space(&self) -> &CanonicalCaseSpace {
        &self.case_space
    }

    pub(crate) fn residual_support(&self) -> &ExploreResidualSupport {
        &self.residual_support
    }

    pub(crate) fn shard_width(&self) -> ExploreShardWidthV1 {
        self.shard_width
    }

    pub(crate) fn shard_count(&self) -> u128 {
        self.shard_count
    }

    pub(crate) fn id(&self) -> ExploreRunContractId {
        self.id
    }

    pub(crate) fn shard(
        &self,
        shard_ordinal: u128,
    ) -> Result<ExploreShardDescriptor, ExploreRunStateError> {
        if shard_ordinal >= self.shard_count {
            return Err(invalid(format!(
                "residual shard ordinal {shard_ordinal} is outside shard count {}",
                self.shard_count
            )));
        }
        let support_interval_index = interval_index_for_shard(&self.shard_prefixes, shard_ordinal)
            .ok_or_else(|| invalid("residual shard ordinal did not resolve into support"))?;
        let local_ordinal = shard_ordinal - self.shard_prefixes[support_interval_index];
        self.shard_in_interval(support_interval_index, local_ordinal, shard_ordinal)
    }

    /// Return the residual shard containing `rank`, or `None` when the rank is
    /// certificate-closed (outside residual support).  Both support and shard
    /// lookup are logarithmic in the number of sparse support intervals.
    pub(crate) fn shard_containing_rank(
        &self,
        rank: u128,
    ) -> Result<Option<ExploreShardDescriptor>, ExploreRunStateError> {
        if rank >= self.case_space.declared_case_count {
            return Err(invalid(format!(
                "CaseId rank {rank} is outside declared U={}",
                self.case_space.declared_case_count
            )));
        }
        let Some(interval_index) = interval_index_for_rank(&self.residual_support.intervals, rank)
        else {
            return Ok(None);
        };
        let support = self.residual_support.intervals[interval_index];
        let width = self.shard_width.0 as u128;
        let local_ordinal = (rank - support.start) / width;
        let shard_ordinal = self.shard_prefixes[interval_index]
            .checked_add(local_ordinal)
            .ok_or_else(|| invalid("residual shard ordinal exceeds u128::MAX"))?;
        self.shard_in_interval(interval_index, local_ordinal, shard_ordinal)
            .map(Some)
    }

    fn shard_in_interval(
        &self,
        support_interval_index: usize,
        local_ordinal: u128,
        shard_ordinal: u128,
    ) -> Result<ExploreShardDescriptor, ExploreRunStateError> {
        let support = self.residual_support.intervals[support_interval_index];
        let width = self.shard_width.0 as u128;
        let offset = local_ordinal
            .checked_mul(width)
            .ok_or_else(|| invalid("residual shard offset exceeds u128::MAX"))?;
        let start_rank = support
            .start
            .checked_add(offset)
            .ok_or_else(|| invalid("residual shard start exceeds u128::MAX"))?;
        let remaining = support.end_exclusive - start_rank;
        let case_count_u128 = remaining.min(width);
        let case_count = usize::try_from(case_count_u128)
            .map_err(|_| invalid("v1 residual shard width exceeds usize"))?;
        let end_rank_exclusive = start_rank
            .checked_add(case_count_u128)
            .ok_or_else(|| invalid("residual shard end exceeds u128::MAX"))?;
        let first_case_id = self.case_space.case_id_at_rank(start_rank)?;
        let last_case_id = self.case_space.case_id_at_rank(end_rank_exclusive - 1)?;
        let id = derive_shard_id(
            self.id,
            shard_ordinal,
            support_interval_index,
            start_rank,
            end_rank_exclusive,
            case_count,
            &first_case_id,
            &last_case_id,
        );
        Ok(ExploreShardDescriptor {
            run_contract_id: self.id,
            id,
            shard_ordinal,
            support_interval_index,
            start_rank,
            end_rank_exclusive,
            case_count,
            first_case_id,
            last_case_id,
        })
    }

    fn validate(&self) -> Result<(), ExploreRunStateError> {
        self.identity.validate()?;
        let rebuilt_space = CanonicalCaseSpace::new(self.case_space.axis_cardinalities.clone())?;
        if rebuilt_space != self.case_space {
            return Err(invalid(
                "run contract carries a stale declared CaseId universe",
            ));
        }
        let rebuilt_support = ExploreResidualSupport::new(
            &self.case_space,
            self.residual_support
                .intervals
                .iter()
                .map(|interval| (interval.start, interval.end_exclusive)),
        )?;
        if rebuilt_support != self.residual_support {
            return Err(invalid(
                "run contract carries noncanonical residual support",
            ));
        }
        let expected_prefixes = checked_shard_prefixes(&self.residual_support, self.shard_width)?;
        if expected_prefixes != self.shard_prefixes
            || expected_prefixes.last().copied().unwrap_or(0) != self.shard_count
        {
            return Err(invalid("run contract carries a stale residual shard index"));
        }
        let expected = derive_run_contract_id(
            &self.identity,
            &self.case_space,
            &self.residual_support,
            self.shard_width,
        );
        if expected != self.id {
            return Err(invalid("run contract hash conflicts with its exact fields"));
        }
        Ok(())
    }

    fn validate_descriptor(
        &self,
        descriptor: &ExploreShardDescriptor,
    ) -> Result<(), ExploreRunStateError> {
        if descriptor.run_contract_id != self.id {
            return Err(invalid(format!(
                "residual shard {} belongs to a stale run contract",
                descriptor.shard_ordinal
            )));
        }
        let expected = self.shard(descriptor.shard_ordinal)?;
        if &expected != descriptor {
            return Err(invalid(format!(
                "residual shard {} is not its immutable canonical descriptor",
                descriptor.shard_ordinal
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreShardId([u8; 32]);

/// One immutable shard wholly contained in one residual-support interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreShardDescriptor {
    run_contract_id: ExploreRunContractId,
    id: ExploreShardId,
    shard_ordinal: u128,
    support_interval_index: usize,
    start_rank: u128,
    end_rank_exclusive: u128,
    case_count: usize,
    first_case_id: ExploreCaseId,
    last_case_id: ExploreCaseId,
}

impl ExploreShardDescriptor {
    pub(crate) fn id(&self) -> ExploreShardId {
        self.id
    }

    pub(crate) fn shard_ordinal(&self) -> u128 {
        self.shard_ordinal
    }

    pub(crate) fn support_interval_index(&self) -> usize {
        self.support_interval_index
    }

    pub(crate) fn start_rank(&self) -> u128 {
        self.start_rank
    }

    pub(crate) fn end_rank_exclusive(&self) -> u128 {
        self.end_rank_exclusive
    }

    pub(crate) fn case_count(&self) -> usize {
        self.case_count
    }

    pub(crate) fn first_case_id(&self) -> &ExploreCaseId {
        &self.first_case_id
    }

    pub(crate) fn last_case_id(&self) -> &ExploreCaseId {
        &self.last_case_id
    }
}

/// Invocation-local retry ceiling.  It is absent from run/shard/chunk hashes,
/// so raising it never invalidates already committed chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreRunInvocationPolicy {
    max_attempts_per_shard: NonZeroU32,
}

impl ExploreRunInvocationPolicy {
    pub(crate) fn new(max_attempts_per_shard: NonZeroU32) -> Self {
        Self {
            max_attempts_per_shard,
        }
    }

    pub(crate) fn max_attempts_per_shard(self) -> NonZeroU32 {
        self.max_attempts_per_shard
    }
}

/// Immutable header installed once before append-only journal entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreRunContractHeader {
    contract: ExploreRunContract,
}

impl ExploreRunContractHeader {
    fn new(contract: ExploreRunContract) -> Self {
        Self { contract }
    }

    pub(crate) fn contract(&self) -> &ExploreRunContract {
        &self.contract
    }

    pub(crate) fn id(&self) -> ExploreRunContractId {
        self.contract.id
    }
}

/// Invocation-local epoch recorded in each immutable attempt entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreLeaseEpoch([u8; 32]);

impl ExploreLeaseEpoch {
    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStateError> {
        require_lowercase_sha256("lease_epoch", value)?;
        Ok(Self(parse_sha256(value)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreAttemptEntryId([u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreLeaseToken {
    epoch: ExploreLeaseEpoch,
    shard_ordinal: u128,
    shard_id: ExploreShardId,
    attempt: NonZeroU32,
}

/// One small append-only durability record.  It does not copy the run ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreAttemptJournalEntry {
    run_contract_id: ExploreRunContractId,
    token: ExploreLeaseToken,
    id: ExploreAttemptEntryId,
}

impl ExploreAttemptJournalEntry {
    pub(crate) fn id(&self) -> ExploreAttemptEntryId {
        self.id
    }

    pub(crate) fn shard_ordinal(&self) -> u128 {
        self.token.shard_ordinal
    }

    pub(crate) fn attempt(&self) -> NonZeroU32 {
        self.token.attempt
    }

    fn validate(&self, contract: &ExploreRunContract) -> Result<(), ExploreRunStateError> {
        if self.run_contract_id != contract.id {
            return Err(invalid("attempt entry belongs to a stale run contract"));
        }
        let descriptor = contract.shard(self.token.shard_ordinal)?;
        if descriptor.id != self.token.shard_id
            || derive_attempt_entry_id(self.run_contract_id, self.token) != self.id
        {
            return Err(invalid(format!(
                "attempt entry identity conflicts for residual shard {}",
                self.token.shard_ordinal
            )));
        }
        Ok(())
    }
}

/// First lease phase.  This value is not dispatch authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExplorePreparedLease {
    attempt_entry: ExploreAttemptJournalEntry,
    descriptor: ExploreShardDescriptor,
}

impl ExplorePreparedLease {
    pub(crate) fn descriptor(&self) -> &ExploreShardDescriptor {
        &self.descriptor
    }

    pub(crate) fn attempt(&self) -> NonZeroU32 {
        self.attempt_entry.token.attempt
    }

    pub(crate) fn attempt_entry(&self) -> &ExploreAttemptJournalEntry {
        &self.attempt_entry
    }
}

/// Storage-minted nonce for one immutable entry, chunk, or manifest install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreStorageWriteGenerationId([u8; 32]);

/// Exact durable attempt identity retained through dispatch and completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExploreLeasePersistenceIdentity {
    run_contract_id: ExploreRunContractId,
    token: ExploreLeaseToken,
    attempt_entry_id: ExploreAttemptEntryId,
    storage_write_generation: ExploreStorageWriteGenerationId,
}

pub(crate) use storage_boundary::{
    ExploreDurableChunkReceipt, ExploreDurableFinalManifestReceipt, ExplorePersistedAttemptReceipt,
};

/// Second lease phase and the only value authorizing evaluator dispatch.
#[derive(Debug)]
pub(crate) struct ExploreActiveLease {
    persistence: ExploreLeasePersistenceIdentity,
    descriptor: ExploreShardDescriptor,
}

impl ExploreActiveLease {
    pub(crate) fn descriptor(&self) -> &ExploreShardDescriptor {
        &self.descriptor
    }

    pub(crate) fn attempt(&self) -> NonZeroU32 {
        self.persistence.token.attempt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExploreActiveLeaseState {
    persistence: ExploreLeasePersistenceIdentity,
    descriptor: ExploreShardDescriptor,
}

impl ExploreActiveLease {
    fn state(&self) -> ExploreActiveLeaseState {
        ExploreActiveLeaseState {
            persistence: self.persistence,
            descriptor: self.descriptor.clone(),
        }
    }
}

/// Opaque ordinary-evaluator evidence identity for one exact residual CaseId.
/// No mechanism signature is represented or inferred here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreShardCaseEvidence {
    rank: u128,
    case_id: ExploreCaseId,
    evidence_hash: Box<str>,
}

impl ExploreShardCaseEvidence {
    pub(crate) fn new(
        rank: u128,
        case_id: ExploreCaseId,
        evidence_hash: impl Into<Box<str>>,
    ) -> Result<Self, ExploreRunStateError> {
        let evidence_hash = evidence_hash.into();
        require_lowercase_sha256("case_evidence_hash", &evidence_hash)?;
        Ok(Self {
            rank,
            case_id,
            evidence_hash,
        })
    }

    pub(crate) fn rank(&self) -> u128 {
        self.rank
    }

    pub(crate) fn case_id(&self) -> &ExploreCaseId {
        &self.case_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreShardEvidenceId([u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreChunkContentId([u8; 32]);

impl ExploreChunkContentId {
    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }
}

/// Purely validated worker output.  It is still uncommitted and non-durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreValidatedCompletion {
    lease_persistence: ExploreLeasePersistenceIdentity,
    run_contract_id: ExploreRunContractId,
    descriptor: ExploreShardDescriptor,
    records: Box<[ExploreShardCaseEvidence]>,
    evidence_id: ExploreShardEvidenceId,
    canonical_bytes: Box<[u8]>,
    content_id: ExploreChunkContentId,
}

impl ExploreValidatedCompletion {
    pub(crate) fn descriptor(&self) -> &ExploreShardDescriptor {
        &self.descriptor
    }

    pub(crate) fn evidence_id(&self) -> ExploreShardEvidenceId {
        self.evidence_id
    }

    pub(crate) fn content_id(&self) -> ExploreChunkContentId {
        self.content_id
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Canonical, storage-generation-independent identity of one chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreChunkIdentity {
    run_contract_id: ExploreRunContractId,
    descriptor: ExploreShardDescriptor,
    record_count: usize,
    evidence_id: ExploreShardEvidenceId,
    content_id: ExploreChunkContentId,
}

impl ExploreChunkIdentity {
    pub(crate) fn descriptor(&self) -> &ExploreShardDescriptor {
        &self.descriptor
    }

    pub(crate) fn record_count(&self) -> usize {
        self.record_count
    }

    pub(crate) fn evidence_id(&self) -> ExploreShardEvidenceId {
        self.evidence_id
    }

    pub(crate) fn content_id(&self) -> ExploreChunkContentId {
        self.content_id
    }
}

impl ExploreValidatedCompletion {
    fn as_chunk_identity(&self) -> ExploreChunkIdentity {
        ExploreChunkIdentity {
            run_contract_id: self.run_contract_id,
            descriptor: self.descriptor.clone(),
            record_count: self.records.len(),
            evidence_id: self.evidence_id,
            content_id: self.content_id,
        }
    }
}

/// Authoritative immutable chunk-directory entry.  It is not a mutable
/// manifest row; its write generation is operational and excluded from the
/// semantic coverage root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreCommittedChunk {
    chunk: ExploreChunkIdentity,
    storage_write_generation: ExploreStorageWriteGenerationId,
}

impl ExploreCommittedChunk {
    pub(crate) fn descriptor(&self) -> &ExploreShardDescriptor {
        &self.chunk.descriptor
    }

    pub(crate) fn canonical_identity(&self) -> &ExploreChunkIdentity {
        &self.chunk
    }
}

/// Sealed owner-local persistence boundary.
///
/// The coordinator can consume these receipt types but cannot construct them:
/// their fields and all production issuers are private to this module.  The
/// eventual filesystem adapter must be implemented as a child of this module.
/// Attempt issuance requires the immutable contract header and exact small
/// entry to be installed and directory-synced.  Chunk and final-manifest
/// issuance additionally require exact read-back after file and parent-directory
/// sync.  The pure core intentionally does not pretend an in-memory hash proves
/// any of those facts.
mod storage_boundary {
    use super::*;

    /// Single-use evidence that one exact small attempt entry crossed the
    /// storage durability boundary.
    #[derive(Debug)]
    pub(crate) struct ExplorePersistedAttemptReceipt {
        identity: ExploreLeasePersistenceIdentity,
    }

    impl ExplorePersistedAttemptReceipt {
        pub(super) fn into_verified_identity(
            self,
            prepared: &ExplorePreparedLease,
        ) -> Result<ExploreLeasePersistenceIdentity, ExploreRunStateError> {
            if self.identity.run_contract_id != prepared.attempt_entry.run_contract_id
                || self.identity.token != prepared.attempt_entry.token
                || self.identity.attempt_entry_id != prepared.attempt_entry.id
            {
                return Err(invalid(format!(
                    "persisted-attempt receipt conflicts for residual shard {}",
                    prepared.descriptor.shard_ordinal
                )));
            }
            Ok(self.identity)
        }
    }

    /// Single-use evidence that one exact canonical chunk was immutably
    /// installed and read back under one storage write generation.
    #[derive(Debug)]
    pub(crate) struct ExploreDurableChunkReceipt {
        lease_persistence: ExploreLeasePersistenceIdentity,
        chunk: ExploreChunkIdentity,
        storage_write_generation: ExploreStorageWriteGenerationId,
    }

    impl ExploreDurableChunkReceipt {
        pub(super) fn into_verified_committed_chunk(
            self,
            completion: &ExploreValidatedCompletion,
        ) -> Result<ExploreCommittedChunk, ExploreRunStateError> {
            let expected = completion.as_chunk_identity();
            if self.lease_persistence != completion.lease_persistence || self.chunk != expected {
                return Err(invalid(format!(
                    "durable-chunk receipt conflicts for residual shard {}",
                    completion.descriptor.shard_ordinal
                )));
            }
            Ok(ExploreCommittedChunk {
                chunk: self.chunk,
                storage_write_generation: self.storage_write_generation,
            })
        }
    }

    /// Single-use proof that one exact canonical final-manifest generation was
    /// written, read back, and made durable.  This is the only authority that
    /// can turn complete chunk coverage into an exposed coverage seal.
    #[derive(Debug)]
    pub(crate) struct ExploreDurableFinalManifestReceipt {
        identity: ExploreFinalManifestIdentity,
        storage_write_generation: ExploreStorageWriteGenerationId,
    }

    impl ExploreDurableFinalManifestReceipt {
        pub(super) fn into_verified_identity(
            self,
            prepared: &ExplorePreparedFinalManifest,
        ) -> Result<
            (
                ExploreFinalManifestIdentity,
                ExploreStorageWriteGenerationId,
            ),
            ExploreRunStateError,
        > {
            if self.identity != prepared.identity {
                return Err(invalid(
                    "durable final-manifest receipt conflicts with its prepared generation",
                ));
            }
            Ok((self.identity, self.storage_write_generation))
        }
    }

    fn issue_persisted_attempt_after_storage_commit(
        prepared: &ExplorePreparedLease,
        exact_entry_read_after_commit: &ExploreAttemptJournalEntry,
        storage_write_generation: ExploreStorageWriteGenerationId,
    ) -> Result<ExplorePersistedAttemptReceipt, ExploreRunStateError> {
        if exact_entry_read_after_commit != &prepared.attempt_entry {
            return Err(invalid(
                "storage adapter read back a different immutable attempt entry",
            ));
        }
        Ok(ExplorePersistedAttemptReceipt {
            identity: ExploreLeasePersistenceIdentity {
                run_contract_id: prepared.attempt_entry.run_contract_id,
                token: prepared.attempt_entry.token,
                attempt_entry_id: prepared.attempt_entry.id,
                storage_write_generation,
            },
        })
    }

    fn issue_durable_chunk_after_storage_commit(
        completion: &ExploreValidatedCompletion,
        exact_bytes_read_after_commit: &[u8],
        storage_write_generation: ExploreStorageWriteGenerationId,
    ) -> Result<ExploreDurableChunkReceipt, ExploreRunStateError> {
        if exact_bytes_read_after_commit != completion.canonical_bytes.as_ref()
            || ExploreChunkContentId(Sha256::digest(exact_bytes_read_after_commit).into())
                != completion.content_id
        {
            return Err(invalid(
                "durable chunk read-back conflicts with validated canonical bytes",
            ));
        }
        Ok(ExploreDurableChunkReceipt {
            lease_persistence: completion.lease_persistence,
            chunk: completion.as_chunk_identity(),
            storage_write_generation,
        })
    }

    fn issue_durable_final_manifest_after_storage_commit(
        prepared: &ExplorePreparedFinalManifest,
        content_id_read_after_commit: ExploreFinalManifestContentId,
        storage_write_generation: ExploreStorageWriteGenerationId,
    ) -> Result<ExploreDurableFinalManifestReceipt, ExploreRunStateError> {
        if content_id_read_after_commit != prepared.identity.content_id {
            return Err(invalid(
                "final-manifest read-back conflicts with canonical streamed content",
            ));
        }
        Ok(ExploreDurableFinalManifestReceipt {
            identity: prepared.identity,
            storage_write_generation,
        })
    }

    /// Test-only stand-in for the future filesystem adapter.  Production code
    /// cannot mint any receipt through this API.
    #[cfg(test)]
    pub(super) mod canary {
        use super::*;

        fn digest(byte: char) -> [u8; 32] {
            parse_named_sha256("canary_generation", &byte.to_string().repeat(64)).unwrap()
        }

        pub(in super::super) fn persisted_attempt(
            prepared: &ExplorePreparedLease,
            write_generation: char,
        ) -> Result<ExplorePersistedAttemptReceipt, ExploreRunStateError> {
            issue_persisted_attempt_after_storage_commit(
                prepared,
                &prepared.attempt_entry,
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }

        pub(in super::super) fn persisted_attempt_readback(
            prepared: &ExplorePreparedLease,
            exact_entry_read_back: &ExploreAttemptJournalEntry,
            write_generation: char,
        ) -> Result<ExplorePersistedAttemptReceipt, ExploreRunStateError> {
            issue_persisted_attempt_after_storage_commit(
                prepared,
                exact_entry_read_back,
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }

        pub(in super::super) fn durable_chunk(
            completion: &ExploreValidatedCompletion,
            write_generation: char,
        ) -> Result<ExploreDurableChunkReceipt, ExploreRunStateError> {
            issue_durable_chunk_after_storage_commit(
                completion,
                completion.canonical_bytes(),
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }

        pub(in super::super) fn durable_chunk_readback(
            completion: &ExploreValidatedCompletion,
            exact_bytes_read_back: &[u8],
            write_generation: char,
        ) -> Result<ExploreDurableChunkReceipt, ExploreRunStateError> {
            issue_durable_chunk_after_storage_commit(
                completion,
                exact_bytes_read_back,
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }

        pub(in super::super) fn durable_final_manifest(
            prepared: &ExplorePreparedFinalManifest,
            write_generation: char,
        ) -> Result<ExploreDurableFinalManifestReceipt, ExploreRunStateError> {
            issue_durable_final_manifest_after_storage_commit(
                prepared,
                prepared.identity.content_id,
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }

        pub(in super::super) fn durable_final_manifest_readback(
            prepared: &ExplorePreparedFinalManifest,
            content_id_read_back: ExploreFinalManifestContentId,
            write_generation: char,
        ) -> Result<ExploreDurableFinalManifestReceipt, ExploreRunStateError> {
            issue_durable_final_manifest_after_storage_commit(
                prepared,
                content_id_read_back,
                ExploreStorageWriteGenerationId(digest(write_generation)),
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreJournalCoverageId([u8; 32]);

/// Caller-provided nonce for one prepared final-manifest generation.  It is
/// content-bound but is not durability authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreFinalManifestGenerationId([u8; 32]);

impl ExploreFinalManifestGenerationId {
    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStateError> {
        require_lowercase_sha256("final_manifest_generation", value)?;
        Ok(Self(parse_sha256(value)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExploreFinalManifestContentId([u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExploreFinalManifestIdentity {
    run_contract_id: ExploreRunContractId,
    residual_support_id: ExploreResidualSupportId,
    lease_epoch: ExploreLeaseEpoch,
    generation: ExploreFinalManifestGenerationId,
    coverage_id: ExploreJournalCoverageId,
    content_id: ExploreFinalManifestContentId,
    shard_count: u128,
    committed_residual_case_count: u128,
}

/// Nonauthoritative exact identity for the canonical manifest that storage must
/// stream, durably install, and read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExplorePreparedFinalManifest {
    identity: ExploreFinalManifestIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExploreDurableFinalManifest {
    identity: ExploreFinalManifestIdentity,
    storage_write_generation: ExploreStorageWriteGenerationId,
}

/// A durable assertion of full residual-journal coverage, never full semantic U.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExploreJournalCoverageSeal {
    run_contract_id: ExploreRunContractId,
    residual_support_id: ExploreResidualSupportId,
    coverage_id: ExploreJournalCoverageId,
    shard_count: u128,
    committed_residual_case_count: u128,
}

/// Canonical rank-ordered assembly of the residual chunk set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreJournalCoverageAssembly {
    pub(crate) run_contract_id: ExploreRunContractId,
    pub(crate) residual_support_id: ExploreResidualSupportId,
    pub(crate) coverage_id: ExploreJournalCoverageId,
    pub(crate) declared_case_count: u128,
    pub(crate) residual_case_count: u128,
    pub(crate) residual_support: Box<[ExploreRankInterval]>,
    pub(crate) shard_count: u128,
    pub(crate) committed_chunks: Box<[ExploreChunkIdentity]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExplorePrepareNext {
    Prepared(ExplorePreparedLease),
    CommittedResidualCoverageReady,
    FinalManifestPersistenceRequired,
    AwaitingPreparedOrActive {
        lease_count: usize,
    },
    RetryLimitReached {
        descriptor: ExploreShardDescriptor,
        attempts_started: NonZeroU32,
        invocation_ceiling: NonZeroU32,
    },
}

/// Owner-local coordinator state with explicit persistence phases.
#[derive(Debug)]
pub(crate) struct ExploreRunState {
    contract_header: ExploreRunContractHeader,
    invocation_policy: ExploreRunInvocationPolicy,
    lease_epoch: ExploreLeaseEpoch,
    /// In-memory compact indexes only.  The immutable entries/chunks remain
    /// persistence authority; these caches are never a resume artifact.
    attempts_started: BTreeMap<u128, NonZeroU32>,
    attempt_entries: BTreeMap<(u128, u32), ExploreAttemptJournalEntry>,
    committed: BTreeMap<u128, ExploreCommittedChunk>,
    storage_write_generations: BTreeSet<ExploreStorageWriteGenerationId>,
    prepared: BTreeMap<u128, ExplorePreparedLease>,
    active: BTreeMap<u128, ExploreActiveLeaseState>,
    next_never_dispatched_ordinal: u128,
    retry_ready: BTreeSet<u128>,
    committed_prefix_shards: u128,
    committed_residual_case_count: u128,
    prepared_final_manifest: Option<ExplorePreparedFinalManifest>,
    durable_final_manifest: Option<ExploreDurableFinalManifest>,
}

impl ExploreRunState {
    pub(crate) fn new(
        contract: ExploreRunContract,
        invocation_policy: ExploreRunInvocationPolicy,
        lease_epoch: ExploreLeaseEpoch,
    ) -> Self {
        Self {
            contract_header: ExploreRunContractHeader::new(contract),
            invocation_policy,
            lease_epoch,
            attempts_started: BTreeMap::new(),
            attempt_entries: BTreeMap::new(),
            committed: BTreeMap::new(),
            storage_write_generations: BTreeSet::new(),
            prepared: BTreeMap::new(),
            active: BTreeMap::new(),
            next_never_dispatched_ordinal: 0,
            retry_ready: BTreeSet::new(),
            committed_prefix_shards: 0,
            committed_residual_case_count: 0,
            prepared_final_manifest: None,
            durable_final_manifest: None,
        }
    }

    pub(crate) fn contract(&self) -> &ExploreRunContract {
        self.contract_header.contract()
    }

    pub(crate) fn contract_header(&self) -> &ExploreRunContractHeader {
        &self.contract_header
    }

    pub(crate) fn invocation_policy(&self) -> ExploreRunInvocationPolicy {
        self.invocation_policy
    }

    /// Prepare the lowest-rank retry, otherwise the next never-dispatched
    /// ordinal.  Each ordinal enters and leaves the monotonic cursor once;
    /// dispatch does not rescan a growing committed suffix.
    pub(crate) fn prepare_next(&mut self) -> Result<ExplorePrepareNext, ExploreRunStateError> {
        if self.durable_final_manifest.is_some() {
            return Ok(ExplorePrepareNext::CommittedResidualCoverageReady);
        }
        if self.committed_prefix_shards == self.contract().shard_count {
            if self.prepared.is_empty() && self.active.is_empty() {
                return Ok(ExplorePrepareNext::FinalManifestPersistenceRequired);
            }
            return Ok(ExplorePrepareNext::AwaitingPreparedOrActive {
                lease_count: self.inflight_lease_count()?,
            });
        }

        let ceiling = self.invocation_policy.max_attempts_per_shard;
        let retry_ordinal = self.retry_ready.iter().next().copied();
        let (ordinal, from_retry) = if let Some(ordinal) = retry_ordinal {
            let previous = self
                .attempts_started
                .get(&ordinal)
                .map_or(0, |value| value.get());
            if previous >= ceiling.get() {
                return Ok(ExplorePrepareNext::RetryLimitReached {
                    descriptor: self.contract().shard(ordinal)?,
                    attempts_started: NonZeroU32::new(previous)
                        .ok_or_else(|| invalid("retry-limited shard has zero attempts"))?,
                    invocation_ceiling: ceiling,
                });
            }
            (ordinal, true)
        } else if self.next_never_dispatched_ordinal < self.contract().shard_count {
            (self.next_never_dispatched_ordinal, false)
        } else {
            return Ok(ExplorePrepareNext::AwaitingPreparedOrActive {
                lease_count: self.inflight_lease_count()?,
            });
        };

        if self.committed.contains_key(&ordinal)
            || self.prepared.contains_key(&ordinal)
            || self.active.contains_key(&ordinal)
        {
            return Err(invalid(format!(
                "scheduler index points at unavailable residual shard {ordinal}"
            )));
        }
        let previous = self
            .attempts_started
            .get(&ordinal)
            .map_or(0, |value| value.get());
        let attempt = NonZeroU32::new(
            previous
                .checked_add(1)
                .ok_or_else(|| invalid("residual shard attempt exceeds u32::MAX"))?,
        )
        .ok_or_else(|| invalid("residual shard attempt became zero"))?;
        let descriptor = self.contract().shard(ordinal)?;
        let token = ExploreLeaseToken {
            epoch: self.lease_epoch,
            shard_ordinal: ordinal,
            shard_id: descriptor.id,
            attempt,
        };
        let attempt_entry = ExploreAttemptJournalEntry {
            run_contract_id: self.contract_header.id(),
            token,
            id: derive_attempt_entry_id(self.contract_header.id(), token),
        };
        let prepared = ExplorePreparedLease {
            attempt_entry,
            descriptor,
        };
        if from_retry {
            self.retry_ready.remove(&ordinal);
        } else {
            self.next_never_dispatched_ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| invalid("residual shard scheduler exceeds u128::MAX"))?;
        }
        self.prepared.insert(ordinal, prepared.clone());
        Ok(ExplorePrepareNext::Prepared(prepared))
    }

    /// Activate only after this exact small attempt entry is durably appended.
    pub(crate) fn activate(
        &mut self,
        prepared: ExplorePreparedLease,
        receipt: ExplorePersistedAttemptReceipt,
    ) -> Result<ExploreActiveLease, ExploreRunStateError> {
        let ordinal = prepared.descriptor.shard_ordinal;
        let stored = self
            .prepared
            .get(&ordinal)
            .ok_or_else(|| invalid("prepared lease is no longer awaiting activation"))?;
        if stored != &prepared {
            return Err(invalid(format!(
                "prepared lease conflicts for residual shard {ordinal}"
            )));
        }
        self.contract().validate_descriptor(&prepared.descriptor)?;
        if prepared.attempt_entry.run_contract_id != self.contract_header.id()
            || prepared.attempt_entry.token.shard_ordinal != ordinal
            || prepared.attempt_entry.token.shard_id != prepared.descriptor.id
        {
            return Err(invalid(format!(
                "prepared attempt and descriptor conflict for residual shard {ordinal}"
            )));
        }
        prepared.attempt_entry.validate(self.contract())?;
        let persistence = receipt.into_verified_identity(&prepared)?;
        if persistence.token.epoch != self.lease_epoch {
            return Err(invalid("persisted attempt carries a stale lease epoch"));
        }
        if self
            .storage_write_generations
            .contains(&persistence.storage_write_generation)
        {
            return Err(invalid(
                "persisted attempt reuses an existing storage write generation",
            ));
        }
        let previous = self
            .attempts_started
            .get(&ordinal)
            .map_or(0, |attempt| attempt.get());
        let expected_attempt = previous
            .checked_add(1)
            .ok_or_else(|| invalid("residual shard attempt exceeds u32::MAX"))?;
        if persistence.token.attempt.get() != expected_attempt {
            return Err(invalid(format!(
                "persisted attempt is not the next journal entry for residual shard {ordinal}"
            )));
        }
        let key = (ordinal, persistence.token.attempt.get());
        if self.attempt_entries.contains_key(&key) {
            return Err(invalid(format!(
                "attempt journal already contains residual shard {ordinal} attempt {}",
                persistence.token.attempt
            )));
        }
        let active = ExploreActiveLease {
            persistence,
            descriptor: prepared.descriptor.clone(),
        };
        self.prepared.remove(&ordinal);
        self.storage_write_generations
            .insert(persistence.storage_write_generation);
        self.attempts_started
            .insert(ordinal, persistence.token.attempt);
        self.attempt_entries.insert(key, prepared.attempt_entry);
        self.active.insert(ordinal, active.state());
        Ok(active)
    }

    /// Release an entry that never crossed the durability boundary.  It does
    /// not consume an attempt number.
    pub(crate) fn abandon_prepared(
        &mut self,
        prepared: ExplorePreparedLease,
    ) -> Result<ExploreShardDescriptor, ExploreRunStateError> {
        let ordinal = prepared.descriptor.shard_ordinal;
        let stored = self
            .prepared
            .get(&ordinal)
            .ok_or_else(|| invalid("prepared lease is no longer reserved"))?;
        if stored != &prepared {
            return Err(invalid(format!(
                "prepared lease conflicts for residual shard {ordinal}"
            )));
        }
        let descriptor = self
            .prepared
            .remove(&ordinal)
            .ok_or_else(|| invalid("prepared residual lease disappeared"))?
            .descriptor;
        self.retry_ready.insert(ordinal);
        Ok(descriptor)
    }

    pub(crate) fn abandon_active(
        &mut self,
        active: ExploreActiveLease,
    ) -> Result<ExploreShardDescriptor, ExploreRunStateError> {
        let ordinal = self.active_ordinal(active.persistence.token)?;
        let stored = self
            .active
            .get(&ordinal)
            .ok_or_else(|| invalid("active residual lease disappeared"))?;
        if stored != &active.state() {
            return Err(invalid(format!(
                "active lease conflicts for residual shard {ordinal}"
            )));
        }
        let descriptor = self
            .active
            .remove(&ordinal)
            .ok_or_else(|| invalid("active residual lease disappeared"))?
            .descriptor;
        self.retry_ready.insert(ordinal);
        Ok(descriptor)
    }

    pub(crate) fn abandon_all_active(&mut self) -> Box<[ExploreShardDescriptor]> {
        let descriptors = self
            .active
            .values()
            .map(|lease| lease.descriptor.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.retry_ready.extend(self.active.keys().copied());
        self.active.clear();
        descriptors
    }

    /// Validate exact records and canonical bytes without committing anything.
    pub(crate) fn validate_completion(
        &self,
        active: &ExploreActiveLease,
        records: impl Into<Box<[ExploreShardCaseEvidence]>>,
    ) -> Result<ExploreValidatedCompletion, ExploreRunStateError> {
        let ordinal = self.active_ordinal(active.persistence.token)?;
        let stored = self
            .active
            .get(&ordinal)
            .ok_or_else(|| invalid("active residual lease disappeared"))?;
        if stored != &active.state() {
            return Err(invalid(format!(
                "completion lease conflicts for residual shard {ordinal}"
            )));
        }
        let records = records.into();
        validate_completion_records(self.contract(), &active.descriptor, &records)?;
        let evidence_id =
            derive_shard_evidence_id(self.contract_header.id(), &active.descriptor, &records);
        let canonical_bytes =
            encode_canonical_chunk(self.contract(), &active.descriptor, &records)?;
        let content_id = ExploreChunkContentId(Sha256::digest(canonical_bytes.as_ref()).into());
        Ok(ExploreValidatedCompletion {
            lease_persistence: active.persistence,
            run_contract_id: self.contract_header.id(),
            descriptor: active.descriptor.clone(),
            records,
            evidence_id,
            canonical_bytes,
            content_id,
        })
    }

    /// Admit a content-addressed chunk only after immutable installation,
    /// read-back, file sync, and parent-directory sync.  The chunk directory is
    /// authoritative; no mutable per-attempt manifest rewrite occurs here.
    pub(crate) fn commit_durable_chunk(
        &mut self,
        completion: ExploreValidatedCompletion,
        durable_receipt: ExploreDurableChunkReceipt,
    ) -> Result<ExploreCommittedChunk, ExploreRunStateError> {
        if self.prepared_final_manifest.is_some() || self.durable_final_manifest.is_some() {
            return Err(invalid(
                "final-manifest state cannot accept another residual chunk",
            ));
        }
        let ordinal = self.active_ordinal(completion.lease_persistence.token)?;
        let active = self
            .active
            .get(&ordinal)
            .ok_or_else(|| invalid("active residual lease disappeared"))?;
        if active.persistence != completion.lease_persistence
            || active.descriptor != completion.descriptor
        {
            return Err(invalid(format!(
                "validated completion conflicts with active residual shard {ordinal}"
            )));
        }
        let committed_chunk = durable_receipt.into_verified_committed_chunk(&completion)?;
        if self
            .storage_write_generations
            .contains(&committed_chunk.storage_write_generation)
        {
            return Err(invalid(
                "durable receipt reuses an existing storage write generation",
            ));
        }
        if self.committed.contains_key(&ordinal) {
            return Err(invalid(format!(
                "residual shard {ordinal} is already committed"
            )));
        }
        let record_count = u128::try_from(committed_chunk.chunk.record_count)
            .map_err(|_| invalid("committed v1 chunk width exceeds u128"))?;
        let next_committed_count = self
            .committed_residual_case_count
            .checked_add(record_count)
            .ok_or_else(|| invalid("committed residual count exceeds u128::MAX"))?;
        if next_committed_count > self.contract().residual_support.residual_case_count {
            return Err(invalid("committed chunks exceed residual support"));
        }
        self.active.remove(&ordinal);
        self.retry_ready.remove(&ordinal);
        self.storage_write_generations
            .insert(committed_chunk.storage_write_generation);
        self.committed.insert(ordinal, committed_chunk.clone());
        self.committed_residual_case_count = next_committed_count;
        self.advance_committed_prefix();
        Ok(committed_chunk)
    }

    pub(crate) fn prepared_lease_count(&self) -> usize {
        self.prepared.len()
    }

    pub(crate) fn active_lease_count(&self) -> usize {
        self.active.len()
    }

    /// Only authoritative, directory-synced residual chunks contribute.
    pub(crate) fn committed_residual_case_count(&self) -> Result<u128, ExploreRunStateError> {
        Ok(self.committed_residual_case_count)
    }

    pub(crate) fn open_residual_case_count(&self) -> Result<u128, ExploreRunStateError> {
        self.contract()
            .residual_support
            .residual_case_count
            .checked_sub(self.committed_residual_case_count)
            .ok_or_else(|| invalid("committed chunks exceed residual support"))
    }

    /// Prepare a content-bound manifest generation without asserting durable
    /// completion.  Its content ID is computed in one streaming pass over the
    /// canonical BTree order and never materializes a full ledger snapshot.
    pub(crate) fn prepare_final_manifest(
        &mut self,
        generation: ExploreFinalManifestGenerationId,
    ) -> Result<ExplorePreparedFinalManifest, ExploreRunStateError> {
        if self.durable_final_manifest.is_some() {
            return Err(invalid(
                "residual coverage already has a durable final manifest",
            ));
        }
        if self.prepared_final_manifest.is_some() {
            return Err(invalid("a final-manifest generation is already prepared"));
        }
        let identity = self.current_final_manifest_identity(generation)?;
        let prepared = ExplorePreparedFinalManifest { identity };
        // From this point, chunk admission is closed, so the one streamed
        // identity cannot change while storage persists it.
        self.prepared_final_manifest = Some(prepared.clone());
        Ok(prepared)
    }

    /// Expose the residual coverage seal only after storage durably installs
    /// and reads back the exact prepared final-manifest generation.
    pub(crate) fn commit_final_manifest(
        &mut self,
        prepared: ExplorePreparedFinalManifest,
        durable_receipt: ExploreDurableFinalManifestReceipt,
    ) -> Result<ExploreJournalCoverageSeal, ExploreRunStateError> {
        let stored = self
            .prepared_final_manifest
            .as_ref()
            .ok_or_else(|| invalid("no final-manifest generation is awaiting durability"))?;
        if stored != &prepared {
            return Err(invalid("prepared final-manifest generation conflicts"));
        }
        let (identity, storage_write_generation) =
            durable_receipt.into_verified_identity(&prepared)?;
        if self
            .storage_write_generations
            .contains(&storage_write_generation)
        {
            return Err(invalid(
                "final manifest reuses an existing storage write generation",
            ));
        }
        let seal = ExploreJournalCoverageSeal {
            run_contract_id: identity.run_contract_id,
            residual_support_id: identity.residual_support_id,
            coverage_id: identity.coverage_id,
            shard_count: identity.shard_count,
            committed_residual_case_count: identity.committed_residual_case_count,
        };
        self.storage_write_generations
            .insert(storage_write_generation);
        self.durable_final_manifest = Some(ExploreDurableFinalManifest {
            identity,
            storage_write_generation,
        });
        self.prepared_final_manifest = None;
        Ok(seal)
    }

    /// This assembly is deliberately inaccessible before final-manifest
    /// durability.  Allocating the identity list happens only at final output.
    pub(crate) fn canonical_committed_residual_coverage(
        &self,
    ) -> Result<ExploreJournalCoverageAssembly, ExploreRunStateError> {
        let durable = self
            .durable_final_manifest
            .ok_or_else(|| invalid("residual coverage has no durable final manifest"))?;
        if !self
            .storage_write_generations
            .contains(&durable.storage_write_generation)
        {
            return Err(invalid(
                "durable final-manifest write generation is absent from journal state",
            ));
        }
        let committed_chunks = self
            .committed
            .values()
            .map(|entry| entry.chunk.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(ExploreJournalCoverageAssembly {
            run_contract_id: durable.identity.run_contract_id,
            residual_support_id: durable.identity.residual_support_id,
            coverage_id: durable.identity.coverage_id,
            declared_case_count: self.contract().case_space.declared_case_count,
            residual_case_count: durable.identity.committed_residual_case_count,
            residual_support: self.contract().residual_support.intervals.clone(),
            shard_count: durable.identity.shard_count,
            committed_chunks,
        })
    }

    fn current_final_manifest_identity(
        &self,
        generation: ExploreFinalManifestGenerationId,
    ) -> Result<ExploreFinalManifestIdentity, ExploreRunStateError> {
        if self.committed_prefix_shards != self.contract().shard_count {
            let descriptor = self.contract().shard(self.committed_prefix_shards)?;
            return Err(invalid(format!(
                "authoritative residual chunks have an open rank interval [{}, {})",
                descriptor.start_rank, descriptor.end_rank_exclusive
            )));
        }
        if !self.prepared.is_empty() || !self.active.is_empty() {
            return Err(invalid(
                "final manifest conflicts with nonterminal residual leases",
            ));
        }
        if self.committed_residual_case_count
            != self.contract().residual_support.residual_case_count
        {
            return Err(invalid(format!(
                "authoritative chunks cover {} residual cases, expected {}",
                self.committed_residual_case_count,
                self.contract().residual_support.residual_case_count
            )));
        }
        let chunk_count = u128::try_from(self.committed.len())
            .map_err(|_| invalid("committed chunk count exceeds u128"))?;
        if chunk_count != self.contract().shard_count {
            return Err(invalid(format!(
                "authoritative chunk count {chunk_count} conflicts with shard count {}",
                self.contract().shard_count
            )));
        }
        let coverage_id = derive_coverage_id_from_committed(
            self.contract_header.id(),
            self.contract().residual_support.id,
            self.committed_residual_case_count,
            &self.committed,
        );
        let content_id = derive_final_manifest_content_id(
            self.contract_header.id(),
            self.contract().residual_support.id,
            self.lease_epoch,
            generation,
            coverage_id,
            self.contract().shard_count,
            self.committed_residual_case_count,
            &self.committed,
        );
        Ok(ExploreFinalManifestIdentity {
            run_contract_id: self.contract_header.id(),
            residual_support_id: self.contract().residual_support.id,
            lease_epoch: self.lease_epoch,
            generation,
            coverage_id,
            content_id,
            shard_count: self.contract().shard_count,
            committed_residual_case_count: self.committed_residual_case_count,
        })
    }

    fn active_ordinal(&self, token: ExploreLeaseToken) -> Result<u128, ExploreRunStateError> {
        if token.epoch != self.lease_epoch {
            return Err(invalid("completion carries a stale lease epoch"));
        }
        let ordinal = token.shard_ordinal;
        let Some(active) = self.active.get(&ordinal) else {
            return Err(invalid("completion carries no active residual lease"));
        };
        if active.persistence.token != token {
            return Err(invalid(format!(
                "completion carries a stale attempt for residual shard {ordinal}"
            )));
        }
        Ok(ordinal)
    }

    fn inflight_lease_count(&self) -> Result<usize, ExploreRunStateError> {
        self.prepared
            .len()
            .checked_add(self.active.len())
            .ok_or_else(|| invalid("in-flight residual lease count exceeds usize"))
    }

    fn advance_committed_prefix(&mut self) {
        let shard_count = self.contract().shard_count;
        while self.committed_prefix_shards < shard_count
            && self.committed.contains_key(&self.committed_prefix_shards)
        {
            self.committed_prefix_shards += 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExploreRunStateError(String);

impl fmt::Display for ExploreRunStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ExploreRunStateError {}

fn invalid(message: impl Into<String>) -> ExploreRunStateError {
    ExploreRunStateError(message.into())
}

fn checked_product(cardinalities: &[u128]) -> Result<u128, ExploreRunStateError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities
        .iter()
        .copied()
        .try_fold(1_u128, |total, value| {
            total
                .checked_mul(value)
                .ok_or_else(|| invalid("declared CaseId universe exceeds u128::MAX"))
        })
}

fn checked_shard_prefixes(
    residual: &ExploreResidualSupport,
    width: ExploreShardWidthV1,
) -> Result<Box<[u128]>, ExploreRunStateError> {
    let width = width.0 as u128;
    let prefix_capacity = residual
        .intervals
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid("residual shard prefix index exceeds usize"))?;
    let mut prefixes = Vec::with_capacity(prefix_capacity);
    prefixes.push(0_u128);
    for interval in residual.intervals.iter() {
        let next = prefixes
            .last()
            .copied()
            .ok_or_else(|| invalid("residual shard prefix index is empty"))?
            .checked_add(shards_for_case_count(interval.case_count(), width))
            .ok_or_else(|| invalid("residual shard count exceeds u128::MAX"))?;
        prefixes.push(next);
    }
    Ok(prefixes.into_boxed_slice())
}

fn shards_for_case_count(case_count: u128, width: u128) -> u128 {
    case_count / width + u128::from(case_count % width != 0)
}

fn interval_index_for_shard(prefixes: &[u128], shard_ordinal: u128) -> Option<usize> {
    if prefixes.len() < 2 || shard_ordinal >= *prefixes.last()? {
        return None;
    }
    let mut low = 0_usize;
    let mut high = prefixes.len() - 1;
    while low < high {
        let middle = low + (high - low) / 2;
        if prefixes[middle + 1] <= shard_ordinal {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Some(low)
}

fn interval_index_for_rank(intervals: &[ExploreRankInterval], rank: u128) -> Option<usize> {
    let mut low = 0_usize;
    let mut high = intervals.len();
    while low < high {
        let middle = low + (high - low) / 2;
        if intervals[middle].end_exclusive <= rank {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    intervals
        .get(low)
        .filter(|interval| interval.start <= rank)
        .map(|_| low)
}

fn require_lowercase_sha256(field: &str, value: &str) -> Result<(), ExploreRunStateError> {
    let valid = value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "Explore run-state {field} must be a lowercase SHA-256 digest"
        )))
    }
}

fn parse_named_sha256(field: &str, value: &str) -> Result<[u8; 32], ExploreRunStateError> {
    require_lowercase_sha256(field, value)?;
    parse_sha256(value)
}

fn parse_sha256(value: &str) -> Result<[u8; 32], ExploreRunStateError> {
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let high = decode_hex(value.as_bytes()[index * 2])?;
        let low = decode_hex(value.as_bytes()[index * 2 + 1])?;
        *byte = (high << 4) | low;
    }
    Ok(digest)
}

fn decode_hex(byte: u8) -> Result<u8, ExploreRunStateError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid("lowercase SHA-256 digest contains non-hex data")),
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn derive_residual_support_id(
    declared_case_count: u128,
    intervals: &[ExploreRankInterval],
) -> ExploreResidualSupportId {
    let mut hasher = StableHasher::new(RESIDUAL_SUPPORT_HASH_V1);
    hasher.u128(declared_case_count);
    hasher.u128(intervals.len() as u128);
    for interval in intervals {
        hasher.u128(interval.start);
        hasher.u128(interval.end_exclusive);
    }
    ExploreResidualSupportId(hasher.finish())
}

fn derive_run_contract_id(
    identity: &ExploreRunIdentity,
    case_space: &CanonicalCaseSpace,
    residual_support: &ExploreResidualSupport,
    shard_width: ExploreShardWidthV1,
) -> ExploreRunContractId {
    let mut hasher = StableHasher::new(RUN_CONTRACT_HASH_V1);
    identity.hash_into(&mut hasher);
    hasher.u128(case_space.axis_cardinalities.len() as u128);
    for cardinality in case_space.axis_cardinalities.iter().copied() {
        hasher.u128(cardinality);
    }
    hasher.u128(case_space.declared_case_count);
    hasher.segment(&(residual_support.id.0));
    hasher.u128(shard_width.0 as u128);
    ExploreRunContractId(hasher.finish())
}

fn derive_shard_id(
    run_contract_id: ExploreRunContractId,
    shard_ordinal: u128,
    support_interval_index: usize,
    start_rank: u128,
    end_rank_exclusive: u128,
    case_count: usize,
    first_case_id: &ExploreCaseId,
    last_case_id: &ExploreCaseId,
) -> ExploreShardId {
    let mut hasher = StableHasher::new(SHARD_DESCRIPTOR_HASH_V1);
    hasher.segment(&run_contract_id.0);
    hasher.u128(shard_ordinal);
    hasher.u128(support_interval_index as u128);
    hasher.u128(start_rank);
    hasher.u128(end_rank_exclusive);
    hasher.u128(case_count as u128);
    hash_case_id(&mut hasher, first_case_id);
    hash_case_id(&mut hasher, last_case_id);
    ExploreShardId(hasher.finish())
}

fn derive_attempt_entry_id(
    run_contract_id: ExploreRunContractId,
    token: ExploreLeaseToken,
) -> ExploreAttemptEntryId {
    let mut hasher = StableHasher::new(ATTEMPT_ENTRY_HASH_V1);
    hasher.segment(&run_contract_id.0);
    hasher.segment(&(token.epoch.0));
    hasher.segment(&(token.shard_id.0));
    hasher.u128(token.shard_ordinal);
    hasher.u32(token.attempt.get());
    ExploreAttemptEntryId(hasher.finish())
}

fn derive_shard_evidence_id(
    run_contract_id: ExploreRunContractId,
    descriptor: &ExploreShardDescriptor,
    records: &[ExploreShardCaseEvidence],
) -> ExploreShardEvidenceId {
    let mut hasher = StableHasher::new(SHARD_EVIDENCE_HASH_V1);
    hasher.segment(&run_contract_id.0);
    hasher.segment(&(descriptor.id.0));
    hasher.u128(records.len() as u128);
    for record in records {
        hasher.u128(record.rank);
        hash_case_id(&mut hasher, &record.case_id);
        hasher.segment(record.evidence_hash.as_bytes());
    }
    ExploreShardEvidenceId(hasher.finish())
}

fn derive_coverage_id_from_committed(
    run_contract_id: ExploreRunContractId,
    residual_support_id: ExploreResidualSupportId,
    residual_case_count: u128,
    committed: &BTreeMap<u128, ExploreCommittedChunk>,
) -> ExploreJournalCoverageId {
    let mut hasher = StableHasher::new(COVERAGE_HASH_V1);
    hasher.segment(&run_contract_id.0);
    hasher.segment(&residual_support_id.0);
    hasher.u128(residual_case_count);
    hasher.u128(committed.len() as u128);
    for chunk in committed.values() {
        hash_chunk_identity(&mut hasher, &chunk.chunk);
    }
    ExploreJournalCoverageId(hasher.finish())
}

fn derive_final_manifest_content_id(
    run_contract_id: ExploreRunContractId,
    residual_support_id: ExploreResidualSupportId,
    lease_epoch: ExploreLeaseEpoch,
    generation: ExploreFinalManifestGenerationId,
    coverage_id: ExploreJournalCoverageId,
    shard_count: u128,
    residual_case_count: u128,
    committed: &BTreeMap<u128, ExploreCommittedChunk>,
) -> ExploreFinalManifestContentId {
    let mut hasher = Sha256::new();
    let result: Result<(), ()> = stream_canonical_final_manifest(
        run_contract_id,
        residual_support_id,
        lease_epoch,
        generation,
        coverage_id,
        shard_count,
        residual_case_count,
        committed,
        |bytes| {
            hasher.update(bytes);
            Ok(())
        },
    );
    debug_assert!(result.is_ok());
    ExploreFinalManifestContentId(hasher.finalize().into())
}

/// Emit the exact final-manifest byte stream without collecting it.  A future
/// storage child must use this encoder for both durable write and read-back
/// hashing while its real owner/read guard remains live.
fn stream_canonical_final_manifest<E>(
    run_contract_id: ExploreRunContractId,
    residual_support_id: ExploreResidualSupportId,
    lease_epoch: ExploreLeaseEpoch,
    generation: ExploreFinalManifestGenerationId,
    coverage_id: ExploreJournalCoverageId,
    shard_count: u128,
    residual_case_count: u128,
    committed: &BTreeMap<u128, ExploreCommittedChunk>,
    mut write: impl FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), E> {
    stream_segment(&mut write, CANONICAL_FINAL_MANIFEST_MAGIC_V1)?;
    stream_segment(&mut write, &run_contract_id.0)?;
    stream_segment(&mut write, &residual_support_id.0)?;
    stream_segment(&mut write, &lease_epoch.0)?;
    stream_segment(&mut write, &generation.0)?;
    stream_segment(&mut write, &coverage_id.0)?;
    write(&shard_count.to_be_bytes())?;
    write(&residual_case_count.to_be_bytes())?;
    write(&(committed.len() as u128).to_be_bytes())?;
    for chunk in committed.values() {
        write(&chunk.chunk.descriptor.shard_ordinal.to_be_bytes())?;
        stream_segment(&mut write, &chunk.chunk.descriptor.id.0)?;
        write(&(chunk.chunk.record_count as u128).to_be_bytes())?;
        stream_segment(&mut write, &chunk.chunk.evidence_id.0)?;
        stream_segment(&mut write, &chunk.chunk.content_id.0)?;
    }
    Ok(())
}

fn stream_segment<E>(
    write: &mut impl FnMut(&[u8]) -> Result<(), E>,
    bytes: &[u8],
) -> Result<(), E> {
    write(&(bytes.len() as u128).to_be_bytes())?;
    write(bytes)
}

fn hash_chunk_identity(hasher: &mut StableHasher, chunk: &ExploreChunkIdentity) {
    hasher.u128(chunk.descriptor.shard_ordinal);
    hasher.segment(&(chunk.descriptor.id.0));
    hasher.u128(chunk.record_count as u128);
    hasher.segment(&(chunk.evidence_id.0));
    hasher.segment(&(chunk.content_id.0));
}

fn hash_case_id(hasher: &mut StableHasher, case_id: &ExploreCaseId) {
    hasher.u128(case_id.len() as u128);
    for ordinal in case_id.ordinals().iter().copied() {
        hasher.u128(ordinal);
    }
}

fn validate_completion_records(
    contract: &ExploreRunContract,
    descriptor: &ExploreShardDescriptor,
    records: &[ExploreShardCaseEvidence],
) -> Result<(), ExploreRunStateError> {
    contract.validate_descriptor(descriptor)?;
    if records.len() != descriptor.case_count {
        return Err(invalid(format!(
            "residual shard {} has {} records for {} canonical ranks",
            descriptor.shard_ordinal,
            records.len(),
            descriptor.case_count
        )));
    }
    for (offset, record) in records.iter().enumerate() {
        require_lowercase_sha256("case_evidence_hash", &record.evidence_hash)?;
        let offset = u128::try_from(offset)
            .map_err(|_| invalid("v1 residual record offset exceeds u128"))?;
        let expected_rank = descriptor
            .start_rank
            .checked_add(offset)
            .ok_or_else(|| invalid("residual record rank exceeds u128::MAX"))?;
        if record.rank != expected_rank {
            return Err(invalid(format!(
                "residual shard {} record rank {} leaves a gap or overlap at {expected_rank}",
                descriptor.shard_ordinal, record.rank
            )));
        }
        let expected_case_id = contract.case_space.case_id_at_rank(expected_rank)?;
        if record.case_id != expected_case_id {
            return Err(invalid(format!(
                "residual shard {} record at rank {expected_rank} has a conflicting CaseId",
                descriptor.shard_ordinal
            )));
        }
    }
    Ok(())
}

/// Canonical binary record form used only to bind decoded records to immutable
/// bytes.  Filesystem layout, filenames and manifest encoding remain separate.
fn encode_canonical_chunk(
    contract: &ExploreRunContract,
    descriptor: &ExploreShardDescriptor,
    records: &[ExploreShardCaseEvidence],
) -> Result<Box<[u8]>, ExploreRunStateError> {
    let mut bytes = Vec::new();
    push_segment(&mut bytes, CANONICAL_CHUNK_MAGIC_V1);
    push_segment(
        &mut bytes,
        contract.identity.chunk_record_schema_hash.as_bytes(),
    );
    push_segment(&mut bytes, &contract.id.0);
    push_segment(&mut bytes, &descriptor.id.0);
    bytes.extend_from_slice(&descriptor.shard_ordinal.to_be_bytes());
    bytes.extend_from_slice(&descriptor.start_rank.to_be_bytes());
    bytes.extend_from_slice(&descriptor.end_rank_exclusive.to_be_bytes());
    let record_count = u32::try_from(records.len())
        .map_err(|_| invalid("canonical chunk record count exceeds u32::MAX"))?;
    bytes.extend_from_slice(&record_count.to_be_bytes());
    for record in records {
        bytes.extend_from_slice(&record.rank.to_be_bytes());
        let axis_count = u32::try_from(record.case_id.len())
            .map_err(|_| invalid("canonical CaseId axis count exceeds u32::MAX"))?;
        bytes.extend_from_slice(&axis_count.to_be_bytes());
        for ordinal in record.case_id.ordinals() {
            bytes.extend_from_slice(&ordinal.to_be_bytes());
        }
        bytes.extend_from_slice(&parse_named_sha256(
            "case_evidence_hash",
            &record.evidence_hash,
        )?);
    }
    Ok(bytes.into_boxed_slice())
}

fn push_segment(bytes: &mut Vec<u8>, segment: &[u8]) {
    bytes.extend_from_slice(&(segment.len() as u128).to_be_bytes());
    bytes.extend_from_slice(segment);
}

struct StableHasher(Sha256);

impl StableHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.segment(domain);
        hasher
    }

    fn segment(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u128).to_be_bytes());
        self.0.update(bytes);
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> Box<str> {
        byte.to_string().repeat(64).into_boxed_str()
    }

    fn identity(report_seed: char) -> ExploreRunIdentity {
        ExploreRunIdentity {
            program_hash: digest('a'),
            analysis_program_hash: digest('b'),
            query_hash: digest('c'),
            domain_hash: digest('d'),
            evaluator_contract_hash: digest('e'),
            report_request_hash: digest(report_seed),
            mechanism_observation: ExploreMechanismObservationIdentity::None {
                explicit_none_hash: digest('1'),
            },
            retention_authorization_hash: digest('2'),
            run_state_artifact_schema_hash: digest('3'),
            chunk_record_schema_hash: digest('4'),
        }
    }

    fn contract(
        report_seed: char,
        universe: u128,
        residual: Vec<(u128, u128)>,
        width: usize,
    ) -> ExploreRunContract {
        ExploreRunContract::new(
            identity(report_seed),
            vec![universe],
            residual,
            ExploreShardWidthV1::new(width).unwrap(),
        )
        .unwrap()
    }

    fn policy(attempts: u32) -> ExploreRunInvocationPolicy {
        ExploreRunInvocationPolicy::new(NonZeroU32::new(attempts).unwrap())
    }

    fn epoch(byte: char) -> ExploreLeaseEpoch {
        ExploreLeaseEpoch::from_lowercase_sha256(&byte.to_string().repeat(64)).unwrap()
    }

    fn final_generation(byte: char) -> ExploreFinalManifestGenerationId {
        ExploreFinalManifestGenerationId::from_lowercase_sha256(&byte.to_string().repeat(64))
            .unwrap()
    }

    fn records(
        contract: &ExploreRunContract,
        descriptor: &ExploreShardDescriptor,
    ) -> Vec<ExploreShardCaseEvidence> {
        (descriptor.start_rank..descriptor.end_rank_exclusive)
            .map(|rank| {
                ExploreShardCaseEvidence::new(
                    rank,
                    contract.case_space.case_id_at_rank(rank).unwrap(),
                    format!("{rank:064x}"),
                )
                .unwrap()
            })
            .collect()
    }

    fn prepare(state: &mut ExploreRunState) -> ExplorePreparedLease {
        match state.prepare_next().unwrap() {
            ExplorePrepareNext::Prepared(prepared) => prepared,
            other => panic!("expected prepared lease, got {other:?}"),
        }
    }

    fn activate_prepared(
        state: &mut ExploreRunState,
        prepared: ExplorePreparedLease,
        write_generation: char,
    ) -> ExploreActiveLease {
        let receipt =
            storage_boundary::canary::persisted_attempt(&prepared, write_generation).unwrap();
        state.activate(prepared, receipt).unwrap()
    }

    fn prepare_and_activate(
        state: &mut ExploreRunState,
        write_generation: char,
    ) -> ExploreActiveLease {
        let prepared = prepare(state);
        activate_prepared(state, prepared, write_generation)
    }

    fn validate_and_commit(
        state: &mut ExploreRunState,
        active: &ExploreActiveLease,
        storage_write_generation: char,
    ) -> ExploreCommittedChunk {
        let completion = state
            .validate_completion(active, records(state.contract(), active.descriptor()))
            .unwrap();
        let receipt =
            storage_boundary::canary::durable_chunk(&completion, storage_write_generation).unwrap();
        state.commit_durable_chunk(completion, receipt).unwrap()
    }

    fn persist_final(
        state: &mut ExploreRunState,
        generation: char,
        write_generation: char,
    ) -> ExploreJournalCoverageSeal {
        let prepared = state
            .prepare_final_manifest(final_generation(generation))
            .unwrap();
        let receipt =
            storage_boundary::canary::durable_final_manifest(&prepared, write_generation).unwrap();
        state.commit_final_manifest(prepared, receipt).unwrap()
    }

    #[test]
    fn residual_support_normalizes_and_sparse_lookup_stays_canonical() {
        let mixed_radix = CanonicalCaseSpace::new(vec![2_u128, 3_u128]).unwrap();
        assert_eq!(
            mixed_radix.case_id_at_rank(4).unwrap().ordinals(),
            [1_u128, 1_u128]
        );
        assert_eq!(
            mixed_radix.rank_of_case_id(&ExploreCaseId::new(vec![1_u128, 1_u128])),
            Ok(4)
        );

        let run_contract = contract('7', 1_000, vec![(700, 710), (20, 610), (10, 20)], 256);
        assert_eq!(
            run_contract.residual_support.intervals,
            vec![
                ExploreRankInterval {
                    start: 10,
                    end_exclusive: 610,
                },
                ExploreRankInterval {
                    start: 700,
                    end_exclusive: 710,
                },
            ]
            .into_boxed_slice()
        );
        assert_eq!(run_contract.residual_support.residual_case_count, 610);
        assert_eq!(run_contract.shard_count, 4);
        assert_eq!(
            (
                run_contract.shard(2).unwrap().start_rank,
                run_contract.shard(2).unwrap().end_rank_exclusive,
            ),
            (522, 610)
        );
        assert_eq!(
            (
                run_contract.shard(3).unwrap().start_rank,
                run_contract.shard(3).unwrap().end_rank_exclusive,
            ),
            (700, 710)
        );
        assert!(run_contract.shard_containing_rank(650).unwrap().is_none());
        assert_eq!(
            run_contract
                .shard_containing_rank(705)
                .unwrap()
                .unwrap()
                .shard_ordinal,
            3
        );
    }

    #[test]
    fn attempt_receipt_binds_only_the_exact_small_entry() {
        let run_contract = contract('7', 600, vec![(0, 600)], 256);
        let mut state = ExploreRunState::new(run_contract, policy(2), epoch('8'));
        let first = prepare(&mut state);
        let first_receipt = storage_boundary::canary::persisted_attempt(&first, '1').unwrap();

        let second = prepare(&mut state);
        let active = state.activate(first, first_receipt).unwrap();
        assert_eq!(active.descriptor().shard_ordinal, 0);
        assert_eq!(state.attempt_entries.len(), 1);
        assert_eq!(state.prepared_lease_count(), 1);

        let mut wrong_entry = second.attempt_entry().clone();
        wrong_entry.token.shard_ordinal = 0;
        assert!(
            storage_boundary::canary::persisted_attempt_readback(&second, &wrong_entry, '2',)
                .is_err()
        );
    }

    #[test]
    fn monotonic_cursor_does_not_rescan_behind_a_slow_first_shard() {
        let run_contract = contract('7', 1_024, vec![(0, 1_024)], 256);
        let mut state = ExploreRunState::new(run_contract, policy(2), epoch('8'));
        let slow = prepare_and_activate(&mut state, '1');

        for (expected_ordinal, attempt_generation, chunk_generation) in
            [(1_u128, '2', '3'), (2, '4', '5'), (3, '6', '7')]
        {
            let active = prepare_and_activate(&mut state, attempt_generation);
            assert_eq!(active.descriptor().shard_ordinal, expected_ordinal);
            validate_and_commit(&mut state, &active, chunk_generation);
        }

        assert_eq!(state.next_never_dispatched_ordinal, 4);
        assert_eq!(state.committed_prefix_shards, 0);
        assert!(matches!(
            state.prepare_next().unwrap(),
            ExplorePrepareNext::AwaitingPreparedOrActive { lease_count: 1 }
        ));

        state.abandon_active(slow).unwrap();
        let retry = prepare(&mut state);
        assert_eq!(retry.descriptor().shard_ordinal, 0);
        assert_eq!(retry.attempt().get(), 2);
    }

    #[test]
    fn abandoning_an_unpersisted_entry_does_not_consume_an_attempt() {
        let run_contract = contract('7', 256, vec![(0, 256)], 256);
        let mut state = ExploreRunState::new(run_contract, policy(1), epoch('8'));
        let first = prepare(&mut state);
        state.abandon_prepared(first).unwrap();
        let replacement = prepare(&mut state);
        assert_eq!(replacement.attempt().get(), 1);

        let active = activate_prepared(&mut state, replacement, '1');
        state.abandon_active(active).unwrap();
        assert!(matches!(
            state.prepare_next().unwrap(),
            ExplorePrepareNext::RetryLimitReached { .. }
        ));
    }

    #[test]
    fn final_coverage_is_hidden_until_the_exact_manifest_is_durable() {
        let run_contract = contract('7', 300, vec![(0, 300)], 256);
        let mut state = ExploreRunState::new(run_contract, policy(2), epoch('8'));
        let first = prepare_and_activate(&mut state, '1');
        let second = prepare_and_activate(&mut state, '2');
        validate_and_commit(&mut state, &second, '3');
        validate_and_commit(&mut state, &first, '4');

        assert_eq!(state.committed_residual_case_count(), Ok(300));
        assert!(matches!(
            state.prepare_next().unwrap(),
            ExplorePrepareNext::FinalManifestPersistenceRequired
        ));
        assert!(state.canonical_committed_residual_coverage().is_err());

        let prepared = state.prepare_final_manifest(final_generation('5')).unwrap();
        assert!(state.canonical_committed_residual_coverage().is_err());
        let mut wrong_content = prepared.identity.content_id;
        wrong_content.0[0] ^= 0xff;
        assert!(storage_boundary::canary::durable_final_manifest_readback(
            &prepared,
            wrong_content,
            '6',
        )
        .is_err());

        let receipt = storage_boundary::canary::durable_final_manifest(&prepared, '6').unwrap();
        let seal = state.commit_final_manifest(prepared, receipt).unwrap();
        assert_eq!(seal.committed_residual_case_count, 300);
        assert!(matches!(
            state.prepare_next().unwrap(),
            ExplorePrepareNext::CommittedResidualCoverageReady
        ));
        assert_eq!(
            state
                .canonical_committed_residual_coverage()
                .unwrap()
                .residual_case_count,
            300
        );
    }

    #[test]
    fn final_manifest_is_canonical_across_chunk_arrival_order() {
        let run_contract = contract('7', 600, vec![(0, 600)], 256);
        let mut left = ExploreRunState::new(run_contract.clone(), policy(2), epoch('8'));
        let left_active = [
            prepare_and_activate(&mut left, '1'),
            prepare_and_activate(&mut left, '2'),
            prepare_and_activate(&mut left, '3'),
        ];
        validate_and_commit(&mut left, &left_active[2], '4');
        validate_and_commit(&mut left, &left_active[0], '5');
        validate_and_commit(&mut left, &left_active[1], '6');

        let mut right = ExploreRunState::new(run_contract, policy(2), epoch('8'));
        let right_active = [
            prepare_and_activate(&mut right, '1'),
            prepare_and_activate(&mut right, '2'),
            prepare_and_activate(&mut right, '3'),
        ];
        validate_and_commit(&mut right, &right_active[0], '4');
        validate_and_commit(&mut right, &right_active[1], '5');
        validate_and_commit(&mut right, &right_active[2], '6');

        let left_manifest = left.prepare_final_manifest(final_generation('7')).unwrap();
        let right_manifest = right.prepare_final_manifest(final_generation('7')).unwrap();
        assert_eq!(
            left_manifest.identity.coverage_id,
            right_manifest.identity.coverage_id
        );
        assert_eq!(
            left_manifest.identity.content_id,
            right_manifest.identity.content_id
        );

        let left_receipt =
            storage_boundary::canary::durable_final_manifest(&left_manifest, 'a').unwrap();
        let right_receipt =
            storage_boundary::canary::durable_final_manifest(&right_manifest, 'b').unwrap();
        left.commit_final_manifest(left_manifest, left_receipt)
            .unwrap();
        right
            .commit_final_manifest(right_manifest, right_receipt)
            .unwrap();
        assert_eq!(
            left.canonical_committed_residual_coverage().unwrap(),
            right.canonical_committed_residual_coverage().unwrap()
        );
    }

    #[test]
    fn manifest_generation_and_content_prevent_receipt_crossing() {
        let run_contract = contract('7', 0, Vec::new(), 256);
        let mut first = ExploreRunState::new(run_contract.clone(), policy(1), epoch('8'));
        let mut second = ExploreRunState::new(run_contract, policy(1), epoch('9'));
        let first_prepared = first.prepare_final_manifest(final_generation('1')).unwrap();
        let stale_receipt =
            storage_boundary::canary::durable_final_manifest(&first_prepared, '2').unwrap();
        let second_prepared = second
            .prepare_final_manifest(final_generation('1'))
            .unwrap();
        assert!(second
            .commit_final_manifest(second_prepared, stale_receipt)
            .is_err());
    }

    #[test]
    fn empty_residual_support_still_requires_a_durable_final_manifest() {
        let run_contract = contract('7', 1_000, Vec::new(), 256);
        let mut state = ExploreRunState::new(run_contract, policy(1), epoch('8'));
        assert_eq!(state.open_residual_case_count(), Ok(0));
        assert!(matches!(
            state.prepare_next().unwrap(),
            ExplorePrepareNext::FinalManifestPersistenceRequired
        ));
        assert!(state.canonical_committed_residual_coverage().is_err());

        let seal = persist_final(&mut state, '1', '2');
        assert_eq!(seal.shard_count, 0);
        let assembly = state.canonical_committed_residual_coverage().unwrap();
        assert_eq!(assembly.declared_case_count, 1_000);
        assert_eq!(assembly.residual_case_count, 0);
        assert!(assembly.committed_chunks.is_empty());
    }
}
