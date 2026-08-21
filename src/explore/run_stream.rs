//! Pure state machine for one durable, observable Explore run.
//!
//! The run has two commitments with deliberately different meanings:
//!
//! - [`JournalHead`] is an order-sensitive hash chain used for durable resume
//!   cursors and follower gap detection.
//! - [`EvidenceRoot`] is an arrival-order-independent commitment to normalized
//!   semantic evidence and the exact required frontier.
//!
//! This module owns neither files nor processes. A storage coordinator must
//! durably install each returned [`CommittedRunEvent`] before publishing it,
//! and must issue fenced writer leases only while holding the owner-local
//! writer lock. Worker output remains a proposal until the coordinator has
//! validated it and prepares one of the transitions below. Only the storage
//! coordinator may install the record and then call `apply_committed`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::{Arc, OnceLock};

use sha2::{Digest, Sha256};

const ANSWER_SCOPE_HASH_V1: &[u8] = b"futuruna.explore.answer-scope.v1";
const CASE_UNIVERSE_HASH_V1: &[u8] = b"futuruna.explore.case-universe.v1";
const CASE_SUPPORT_HASH_V2: &[u8] = b"futuruna.explore.case-support.v2";
const CASE_SUPPORT_TREAP_PRIORITY_HASH_V2: &[u8] =
    b"futuruna.explore.case-support-treap-priority.v2";
const CASE_SUPPORT_TREAP_EMPTY_HASH_V2: &[u8] = b"futuruna.explore.case-support-treap-empty.v2";
const CASE_SUPPORT_TREAP_NODE_HASH_V2: &[u8] = b"futuruna.explore.case-support-treap-node.v2";
const REQUIRED_FRONTIER_HASH_V2: &[u8] = b"futuruna.explore.required-frontier.v2";
const RUN_HEADER_HASH_V1: &[u8] = b"futuruna.explore.run-header.v1";
const RUN_ID_HASH_V1: &[u8] = b"futuruna.explore.run-id.v1";
const JOURNAL_ANCHOR_HASH_V1: &[u8] = b"futuruna.explore.journal-anchor.v1";
const JOURNAL_EVENT_HASH_V1: &[u8] = b"futuruna.explore.journal-event.v1";
const EVIDENCE_ROOT_HASH_V2: &[u8] = b"futuruna.explore.evidence-root.v2";
const SEMANTIC_KEY_HASH_V1: &[u8] = b"futuruna.explore.semantic-key.v1";
const SEMANTIC_ENTRY_HASH_V2: &[u8] = b"futuruna.explore.semantic-entry.v2";
const SEMANTIC_TREAP_PRIORITY_HASH_V1: &[u8] = b"futuruna.explore.semantic-treap-priority.v1";
const SEMANTIC_TREAP_EMPTY_HASH_V2: &[u8] = b"futuruna.explore.semantic-treap-empty.v2";
const SEMANTIC_TREAP_NODE_HASH_V2: &[u8] = b"futuruna.explore.semantic-treap-node.v2";
const COVERAGE_PLAN_HASH_V2: &[u8] = b"futuruna.explore.coverage-plan.v2";
const LEASE_ID_HASH_V1: &[u8] = b"futuruna.explore.writer-lease.v1";
const TERMINAL_PAYLOAD_HASH_V1: &[u8] = b"futuruna.explore.terminal-payload.v1";
const TERMINAL_METHOD_HASH_V1: &[u8] = b"futuruna.explore.terminal-method.v1";
const RUN_RECORD_PAYLOAD_HASH_V3: &[u8] = b"futuruna.explore.run-record-payload.v3";

/// One already-computed canonical SHA-256 identity.
///
/// Parsing rejects mixed case, prefixes and abbreviated hashes so malformed
/// identities cannot silently acquire a second canonical spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalDigest([u8; 32]);

impl CanonicalDigest {
    pub(crate) const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn from_lowercase_sha256(
        field: &'static str,
        value: &str,
    ) -> Result<Self, ExploreRunStreamError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(ExploreRunStreamError::InvalidDigest { field });
        }
        let mut bytes = [0_u8; 32];
        for (index, output) in bytes.iter_mut().enumerate() {
            let high = decode_hex(value.as_bytes()[index * 2]);
            let low = decode_hex(value.as_bytes()[index * 2 + 1]);
            let (Some(high), Some(low)) = (high, low) else {
                return Err(ExploreRunStreamError::InvalidDigest { field });
            };
            *output = (high << 4) | low;
        }
        Ok(Self(bytes))
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }

    fn hash_into(self, hasher: &mut StableHasher) {
        hasher.segment(&self.0);
    }
}

/// Canonical schema identities needed to replay and render this run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExploreRunSchemas {
    journal_record: CanonicalDigest,
    semantic_evidence: CanonicalDigest,
    snapshot: CanonicalDigest,
    terminal_result: CanonicalDigest,
}

impl ExploreRunSchemas {
    pub(crate) fn new(
        journal_record: CanonicalDigest,
        semantic_evidence: CanonicalDigest,
        snapshot: CanonicalDigest,
        terminal_result: CanonicalDigest,
    ) -> Self {
        Self {
            journal_record,
            semantic_evidence,
            snapshot,
            terminal_result,
        }
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        self.journal_record.hash_into(hasher);
        self.semantic_evidence.hash_into(hasher);
        self.snapshot.hash_into(hasher);
        self.terminal_result.hash_into(hasher);
    }

    pub(crate) fn journal_record(&self) -> CanonicalDigest {
        self.journal_record
    }

    pub(crate) fn semantic_evidence(&self) -> CanonicalDigest {
        self.semantic_evidence
    }

    pub(crate) fn snapshot(&self) -> CanonicalDigest {
        self.snapshot
    }

    pub(crate) fn terminal_result(&self) -> CanonicalDigest {
        self.terminal_result
    }
}

/// Immutable semantic, evaluator and disclosure identities selected before a
/// probe or proof plan runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExploreRunIdentity {
    program_hash: CanonicalDigest,
    analysis_program_hash: CanonicalDigest,
    query_hash: CanonicalDigest,
    domain_hash: CanonicalDigest,
    report_request_hash: CanonicalDigest,
    probe_plan_hash: CanonicalDigest,
    evaluator_contract_hash: CanonicalDigest,
    mechanism_observation_hash: CanonicalDigest,
    retention_authorization_hash: CanonicalDigest,
    schemas: ExploreRunSchemas,
}

impl ExploreRunIdentity {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        program_hash: CanonicalDigest,
        analysis_program_hash: CanonicalDigest,
        query_hash: CanonicalDigest,
        domain_hash: CanonicalDigest,
        report_request_hash: CanonicalDigest,
        probe_plan_hash: CanonicalDigest,
        evaluator_contract_hash: CanonicalDigest,
        mechanism_observation_hash: CanonicalDigest,
        retention_authorization_hash: CanonicalDigest,
        schemas: ExploreRunSchemas,
    ) -> Self {
        Self {
            program_hash,
            analysis_program_hash,
            query_hash,
            domain_hash,
            report_request_hash,
            probe_plan_hash,
            evaluator_contract_hash,
            mechanism_observation_hash,
            retention_authorization_hash,
            schemas,
        }
    }

    fn hash_answer_scope_into(&self, hasher: &mut StableHasher) {
        self.program_hash.hash_into(hasher);
        self.analysis_program_hash.hash_into(hasher);
        self.query_hash.hash_into(hasher);
        self.domain_hash.hash_into(hasher);
        self.report_request_hash.hash_into(hasher);
        self.evaluator_contract_hash.hash_into(hasher);
        self.mechanism_observation_hash.hash_into(hasher);
        self.retention_authorization_hash.hash_into(hasher);
        self.schemas.semantic_evidence.hash_into(hasher);
        self.schemas.terminal_result.hash_into(hasher);
    }

    fn hash_header_into(&self, hasher: &mut StableHasher) {
        self.hash_answer_scope_into(hasher);
        // Probe order affects the run journal, not the answer/evidence identity.
        self.probe_plan_hash.hash_into(hasher);
        self.schemas.hash_into(hasher);
    }

    pub(crate) fn program_hash(&self) -> CanonicalDigest {
        self.program_hash
    }

    pub(crate) fn analysis_program_hash(&self) -> CanonicalDigest {
        self.analysis_program_hash
    }

    pub(crate) fn query_hash(&self) -> CanonicalDigest {
        self.query_hash
    }

    pub(crate) fn domain_hash(&self) -> CanonicalDigest {
        self.domain_hash
    }

    pub(crate) fn report_request_hash(&self) -> CanonicalDigest {
        self.report_request_hash
    }

    pub(crate) fn probe_plan_hash(&self) -> CanonicalDigest {
        self.probe_plan_hash
    }

    pub(crate) fn evaluator_contract_hash(&self) -> CanonicalDigest {
        self.evaluator_contract_hash
    }

    pub(crate) fn mechanism_observation_hash(&self) -> CanonicalDigest {
        self.mechanism_observation_hash
    }

    pub(crate) fn retention_authorization_hash(&self) -> CanonicalDigest {
        self.retention_authorization_hash
    }

    pub(crate) fn schemas(&self) -> &ExploreRunSchemas {
        &self.schemas
    }
}

/// Canonical full CaseId-rank universe. The last source axis advances fastest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExploreCaseUniverse {
    axis_cardinalities: Box<[u128]>,
    case_count: u128,
    id: CaseUniverseId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CaseUniverseId([u8; 32]);

impl ExploreCaseUniverse {
    pub(crate) fn new(
        axis_cardinalities: impl Into<Box<[u128]>>,
    ) -> Result<Self, ExploreRunStreamError> {
        let axis_cardinalities = axis_cardinalities.into();
        let case_count = if axis_cardinalities.contains(&0) {
            0
        } else {
            axis_cardinalities
                .iter()
                .copied()
                .try_fold(1_u128, |total, value| {
                    total
                        .checked_mul(value)
                        .ok_or(ExploreRunStreamError::CaseUniverseOverflow)
                })?
        };
        let mut hasher = StableHasher::new(CASE_UNIVERSE_HASH_V1);
        hasher.u128(axis_cardinalities.len() as u128);
        for cardinality in axis_cardinalities.iter().copied() {
            hasher.u128(cardinality);
        }
        hasher.u128(case_count);
        Ok(Self {
            axis_cardinalities,
            case_count,
            id: CaseUniverseId(hasher.finish()),
        })
    }

    pub(crate) fn axis_cardinalities(&self) -> &[u128] {
        &self.axis_cardinalities
    }

    pub(crate) fn case_count(&self) -> u128 {
        self.case_count
    }

    pub(crate) fn identity_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.id.0)
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.segment(&self.id.0);
    }
}

/// One nonempty, half-open interval in canonical CaseId-rank space.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExploreRankInterval {
    start: u128,
    end_exclusive: u128,
}

impl ExploreRankInterval {
    pub(crate) fn start(self) -> u128 {
        self.start
    }

    pub(crate) fn end_exclusive(self) -> u128 {
        self.end_exclusive
    }

    pub(crate) fn case_count(self) -> u128 {
        self.end_exclusive - self.start
    }
}

/// A normalized exact subset of one full case universe.
///
/// The interval set is a persistent authenticated treap. Its priority is a
/// digest of the interval start, so one normalized set has one shape no matter
/// which sequence of insertions produced it and has expected logarithmic
/// height. Frontier subtraction and semantic support union path-copy only
/// nodes touched by the bounded delta. A complete interval vector is
/// deliberately materialized only by [`Self::intervals`] at codec, snapshot,
/// or diagnostic boundaries.
#[derive(Clone, Debug)]
pub(crate) struct ExactCaseSupport {
    universe_id: CaseUniverseId,
    universe_case_count: u128,
    root: Option<Arc<CaseSupportTreapNode>>,
    case_count: u128,
    interval_count: usize,
    id: CaseSupportId,
}

impl PartialEq for ExactCaseSupport {
    fn eq(&self, other: &Self) -> bool {
        self.universe_id == other.universe_id
            && self.universe_case_count == other.universe_case_count
            && self.case_count == other.case_count
            && self.interval_count == other.interval_count
            && self.id == other.id
    }
}

impl Eq for ExactCaseSupport {}

#[derive(Debug)]
struct CaseSupportTreapNode {
    interval: ExploreRankInterval,
    priority: [u8; 32],
    left: Option<Arc<Self>>,
    right: Option<Arc<Self>>,
    subtree_case_count: u128,
    subtree_interval_count: usize,
    subtree_hash: [u8; 32],
}

impl CaseSupportTreapNode {
    fn leaf(interval: ExploreRankInterval) -> Result<Arc<Self>, ExploreRunStreamError> {
        Self::new(
            interval,
            case_support_treap_priority(interval.start),
            None,
            None,
        )
    }

    fn rebuild(
        source: &Self,
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Result<Arc<Self>, ExploreRunStreamError> {
        Self::new(source.interval, source.priority, left, right)
    }

    fn new(
        interval: ExploreRankInterval,
        priority: [u8; 32],
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Result<Arc<Self>, ExploreRunStreamError> {
        let subtree_case_count = case_support_treap_case_count(&left)
            .checked_add(interval.case_count())
            .and_then(|count| count.checked_add(case_support_treap_case_count(&right)))
            .ok_or(ExploreRunStreamError::CaseSupportOverflow)?;
        let subtree_interval_count = case_support_treap_interval_count(&left)
            .checked_add(1)
            .and_then(|count| count.checked_add(case_support_treap_interval_count(&right)))
            .ok_or(ExploreRunStreamError::CaseSupportOverflow)?;
        let mut hasher = StableHasher::new(CASE_SUPPORT_TREAP_NODE_HASH_V2);
        hasher.u128(case_support_treap_interval_count(&left) as u128);
        hasher.u128(case_support_treap_case_count(&left));
        hasher.segment(&case_support_treap_hash(&left));
        hasher.u128(interval.start);
        hasher.u128(interval.end_exclusive);
        hasher.u128(case_support_treap_interval_count(&right) as u128);
        hasher.u128(case_support_treap_case_count(&right));
        hasher.segment(&case_support_treap_hash(&right));
        Ok(Arc::new(Self {
            interval,
            priority,
            left,
            right,
            subtree_case_count,
            subtree_interval_count,
            subtree_hash: hasher.finish(),
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CaseSupportId([u8; 32]);

impl ExactCaseSupport {
    pub(crate) fn new(
        universe: &ExploreCaseUniverse,
        intervals: impl IntoIterator<Item = (u128, u128)>,
    ) -> Result<Self, ExploreRunStreamError> {
        let mut intervals = intervals
            .into_iter()
            .map(|(start, end_exclusive)| {
                if start >= end_exclusive {
                    return Err(ExploreRunStreamError::InvalidRankInterval {
                        start,
                        end_exclusive,
                    });
                }
                if end_exclusive > universe.case_count {
                    return Err(ExploreRunStreamError::RankIntervalOutsideUniverse {
                        start,
                        end_exclusive,
                        universe_case_count: universe.case_count,
                    });
                }
                Ok(ExploreRankInterval {
                    start,
                    end_exclusive,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        intervals.sort_unstable();

        let mut normalized = Vec::<ExploreRankInterval>::with_capacity(intervals.len());
        for interval in intervals {
            if let Some(previous) = normalized.last_mut() {
                if interval.start < previous.end_exclusive {
                    return Err(ExploreRunStreamError::OverlappingRankIntervals {
                        left_start: previous.start,
                        left_end_exclusive: previous.end_exclusive,
                        right_start: interval.start,
                        right_end_exclusive: interval.end_exclusive,
                    });
                }
                if interval.start == previous.end_exclusive {
                    previous.end_exclusive = interval.end_exclusive;
                    continue;
                }
            }
            normalized.push(interval);
        }

        let root = case_support_treap_from_sorted(&normalized)?;
        Ok(Self::from_root(universe.id, universe.case_count, root))
    }

    pub(crate) fn full(universe: &ExploreCaseUniverse) -> Self {
        let intervals = if universe.case_count == 0 {
            Vec::new()
        } else {
            vec![ExploreRankInterval {
                start: 0,
                end_exclusive: universe.case_count,
            }]
        };
        let root = case_support_treap_from_sorted(&intervals)
            .expect("one in-universe interval cannot overflow support metadata");
        Self::from_root(universe.id, universe.case_count, root)
    }

    pub(crate) fn empty(universe: &ExploreCaseUniverse) -> Self {
        Self::from_root(universe.id, universe.case_count, None)
    }

    fn from_root(
        universe_id: CaseUniverseId,
        universe_case_count: u128,
        root: Option<Arc<CaseSupportTreapNode>>,
    ) -> Self {
        let case_count = case_support_treap_case_count(&root);
        let interval_count = case_support_treap_interval_count(&root);
        let mut hasher = StableHasher::new(CASE_SUPPORT_HASH_V2);
        hasher.segment(&universe_id.0);
        hasher.u128(universe_case_count);
        hasher.u128(interval_count as u128);
        hasher.u128(case_count);
        hasher.segment(&case_support_treap_hash(&root));
        Self {
            universe_id,
            universe_case_count,
            root,
            case_count,
            interval_count,
            id: CaseSupportId(hasher.finish()),
        }
    }

    /// Materialize the complete canonical interval sequence.
    ///
    /// This is intentionally not used by frontier scheduling or mutation.
    pub(crate) fn intervals(&self) -> Vec<ExploreRankInterval> {
        let mut intervals = Vec::with_capacity(self.interval_count);
        intervals.extend(self.iter_intervals());
        intervals
    }

    pub(crate) fn interval_count(&self) -> usize {
        self.interval_count
    }

    fn iter_intervals(&self) -> ExactCaseSupportIntervalIter<'_> {
        ExactCaseSupportIntervalIter::new(self.root.as_ref())
    }

    pub(crate) fn case_count(&self) -> u128 {
        self.case_count
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub(crate) fn first_rank(&self) -> Option<u128> {
        case_support_treap_first(self.root.as_ref()).map(|interval| interval.start)
    }

    pub(crate) fn contains_rank(&self, rank: u128) -> bool {
        case_support_treap_containing(self.root.as_ref(), rank).is_some()
    }

    /// First supported rank greater than or equal to `rank`.
    pub(crate) fn first_rank_at_or_after(&self, rank: u128) -> Option<u128> {
        if let Some(interval) = case_support_treap_containing(self.root.as_ref(), rank) {
            return Some(rank);
        }
        case_support_treap_lower_bound(self.root.as_ref(), rank).map(|interval| interval.start)
    }

    pub(crate) fn identity_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.id.0)
    }

    /// Return the exact remainder after removing a proven subset.
    ///
    /// This is the scheduler-side counterpart to frontier conservation: it
    /// refuses a stale or partially overlapping closure instead of silently
    /// clipping it to the currently open support.
    pub(crate) fn subtract_exact(&self, removed: &Self) -> Result<Self, ExploreRunStreamError> {
        if self.universe_id != removed.universe_id {
            return Err(ExploreRunStreamError::StaleCaseUniverse);
        }
        if removed.is_empty() {
            return Ok(self.clone());
        }

        let expected_case_count = self
            .case_count
            .checked_sub(removed.case_count)
            .ok_or(ExploreRunStreamError::FrontierNotConserved)?;
        let mut root = self.root.clone();
        for closed in removed.iter_intervals() {
            let open = case_support_treap_containing(root.as_ref(), closed.start)
                .ok_or(ExploreRunStreamError::FrontierNotConserved)?;
            if closed.end_exclusive > open.end_exclusive {
                return Err(ExploreRunStreamError::FrontierNotConserved);
            }
            root = case_support_treap_delete(root, open.start)?;
            if open.start < closed.start {
                root = Some(case_support_treap_insert_absent(
                    root,
                    ExploreRankInterval {
                        start: open.start,
                        end_exclusive: closed.start,
                    },
                )?);
            }
            if closed.end_exclusive < open.end_exclusive {
                root = Some(case_support_treap_insert_absent(
                    root,
                    ExploreRankInterval {
                        start: closed.end_exclusive,
                        end_exclusive: open.end_exclusive,
                    },
                )?);
            }
        }
        let remainder = Self::from_root(self.universe_id, self.universe_case_count, root);
        if remainder.case_count != expected_case_count {
            return Err(ExploreRunStreamError::FrontierNotConserved);
        }
        Ok(remainder)
    }

    fn exact_disjoint_union(
        &self,
        left: &Self,
        right: &Self,
    ) -> Result<bool, ExploreRunStreamError> {
        if self.universe_id != left.universe_id || self.universe_id != right.universe_id {
            return Err(ExploreRunStreamError::StaleCaseUniverse);
        }
        Ok(self
            .subtract_exact(left)
            .is_ok_and(|remainder| remainder == *right))
    }

    fn merge_disjoint(&self, other: &Self) -> Result<Self, ExploreRunStreamError> {
        if self.universe_id != other.universe_id {
            return Err(ExploreRunStreamError::StaleCaseUniverse);
        }
        let expected_case_count = self
            .case_count
            .checked_add(other.case_count)
            .ok_or(ExploreRunStreamError::CaseSupportOverflow)?;
        let mut root = self.root.clone();
        for interval in other.iter_intervals() {
            root = Some(case_support_treap_insert_disjoint(root, interval)?);
        }
        let merged = Self::from_root(self.universe_id, self.universe_case_count, root);
        if merged.case_count != expected_case_count {
            return Err(ExploreRunStreamError::OverlappingSemanticEvidence);
        }
        Ok(merged)
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.segment(&self.id.0);
    }
}

struct ExactCaseSupportIntervalIter<'a> {
    stack: Vec<&'a CaseSupportTreapNode>,
}

impl<'a> ExactCaseSupportIntervalIter<'a> {
    fn new(root: Option<&'a Arc<CaseSupportTreapNode>>) -> Self {
        let mut iter = Self { stack: Vec::new() };
        iter.push_left(root.map(|node| node.as_ref()));
        iter
    }

    fn push_left(&mut self, mut node: Option<&'a CaseSupportTreapNode>) {
        while let Some(current) = node {
            self.stack.push(current);
            node = current.left.as_deref();
        }
    }
}

impl Iterator for ExactCaseSupportIntervalIter<'_> {
    type Item = ExploreRankInterval;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        self.push_left(node.right.as_deref());
        Some(node.interval)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn case_support_treap_empty_hash() -> [u8; 32] {
    static EMPTY_HASH: OnceLock<[u8; 32]> = OnceLock::new();
    *EMPTY_HASH.get_or_init(|| StableHasher::new(CASE_SUPPORT_TREAP_EMPTY_HASH_V2).finish())
}

fn case_support_treap_hash(root: &Option<Arc<CaseSupportTreapNode>>) -> [u8; 32] {
    root.as_ref()
        .map_or_else(case_support_treap_empty_hash, |node| node.subtree_hash)
}

fn case_support_treap_case_count(root: &Option<Arc<CaseSupportTreapNode>>) -> u128 {
    root.as_ref().map_or(0, |node| node.subtree_case_count)
}

fn case_support_treap_interval_count(root: &Option<Arc<CaseSupportTreapNode>>) -> usize {
    root.as_ref().map_or(0, |node| node.subtree_interval_count)
}

fn case_support_treap_priority(start: u128) -> [u8; 32] {
    let mut hasher = StableHasher::new(CASE_SUPPORT_TREAP_PRIORITY_HASH_V2);
    hasher.u128(start);
    hasher.finish()
}

fn case_support_priority_precedes(
    priority: [u8; 32],
    start: u128,
    node: &CaseSupportTreapNode,
) -> bool {
    (priority, start) < (node.priority, node.interval.start)
}

fn case_support_treap_from_sorted(
    intervals: &[ExploreRankInterval],
) -> Result<Option<Arc<CaseSupportTreapNode>>, ExploreRunStreamError> {
    if intervals.is_empty() {
        return Ok(None);
    }

    struct CartesianNode {
        interval: ExploreRankInterval,
        priority: [u8; 32],
        left: Option<usize>,
        right: Option<usize>,
    }

    // Linear Cartesian-tree construction avoids path-copying the transient
    // prefixes when a bounded wire/full-plan support is first materialized.
    let mut nodes = Vec::<CartesianNode>::with_capacity(intervals.len());
    let mut right_spine = Vec::<usize>::new();
    for interval in intervals.iter().copied() {
        let priority = case_support_treap_priority(interval.start);
        let mut left = None;
        while let Some(&prior) = right_spine.last() {
            if (priority, interval.start) >= (nodes[prior].priority, nodes[prior].interval.start) {
                break;
            }
            left = right_spine.pop();
        }
        let index = nodes.len();
        nodes.push(CartesianNode {
            interval,
            priority,
            left,
            right: None,
        });
        if let Some(&parent) = right_spine.last() {
            nodes[parent].right = Some(index);
        }
        right_spine.push(index);
    }

    let root = right_spine[0];
    let mut preorder = Vec::with_capacity(nodes.len());
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        preorder.push(index);
        if let Some(left) = nodes[index].left {
            pending.push(left);
        }
        if let Some(right) = nodes[index].right {
            pending.push(right);
        }
    }
    let mut frozen = vec![None; nodes.len()];
    for index in preorder.into_iter().rev() {
        let left = nodes[index].left.and_then(|child| frozen[child].take());
        let right = nodes[index].right.and_then(|child| frozen[child].take());
        frozen[index] = Some(CaseSupportTreapNode::new(
            nodes[index].interval,
            nodes[index].priority,
            left,
            right,
        )?);
    }
    Ok(frozen[root].take())
}

fn case_support_treap_first(
    mut root: Option<&Arc<CaseSupportTreapNode>>,
) -> Option<ExploreRankInterval> {
    let mut first = None;
    while let Some(node) = root {
        first = Some(node.interval);
        root = node.left.as_ref();
    }
    first
}

fn case_support_treap_containing(
    mut root: Option<&Arc<CaseSupportTreapNode>>,
    rank: u128,
) -> Option<ExploreRankInterval> {
    while let Some(node) = root {
        if rank < node.interval.start {
            root = node.left.as_ref();
        } else if rank >= node.interval.end_exclusive {
            root = node.right.as_ref();
        } else {
            return Some(node.interval);
        }
    }
    None
}

fn case_support_treap_lower_bound(
    mut root: Option<&Arc<CaseSupportTreapNode>>,
    start: u128,
) -> Option<ExploreRankInterval> {
    let mut candidate = None;
    while let Some(node) = root {
        if node.interval.start < start {
            root = node.right.as_ref();
        } else {
            candidate = Some(node.interval);
            root = node.left.as_ref();
        }
    }
    candidate
}

fn case_support_treap_predecessor(
    mut root: Option<&Arc<CaseSupportTreapNode>>,
    start: u128,
) -> Option<ExploreRankInterval> {
    let mut candidate = None;
    while let Some(node) = root {
        if node.interval.start > start {
            root = node.left.as_ref();
        } else {
            candidate = Some(node.interval);
            root = node.right.as_ref();
        }
    }
    candidate
}

fn case_support_treap_insert_disjoint(
    mut root: Option<Arc<CaseSupportTreapNode>>,
    interval: ExploreRankInterval,
) -> Result<Arc<CaseSupportTreapNode>, ExploreRunStreamError> {
    let mut normalized = interval;
    if let Some(previous) = case_support_treap_predecessor(root.as_ref(), normalized.start) {
        if previous.end_exclusive > normalized.start {
            return Err(ExploreRunStreamError::OverlappingSemanticEvidence);
        }
        if previous.end_exclusive == normalized.start {
            root = case_support_treap_delete(root, previous.start)?;
            normalized.start = previous.start;
        }
    }
    if let Some(next) = case_support_treap_lower_bound(root.as_ref(), normalized.start) {
        if next.start < normalized.end_exclusive {
            return Err(ExploreRunStreamError::OverlappingSemanticEvidence);
        }
        if next.start == normalized.end_exclusive {
            root = case_support_treap_delete(root, next.start)?;
            normalized.end_exclusive = next.end_exclusive;
        }
    }
    case_support_treap_insert_absent(root, normalized)
}

fn case_support_treap_insert_absent(
    root: Option<Arc<CaseSupportTreapNode>>,
    interval: ExploreRankInterval,
) -> Result<Arc<CaseSupportTreapNode>, ExploreRunStreamError> {
    let Some(root) = root else {
        return CaseSupportTreapNode::leaf(interval);
    };
    let priority = case_support_treap_priority(interval.start);
    if case_support_priority_precedes(priority, interval.start, &root) {
        let (left, right) = case_support_treap_split(Some(root), interval.start)?;
        return CaseSupportTreapNode::new(interval, priority, left, right);
    }
    match interval.start.cmp(&root.interval.start) {
        std::cmp::Ordering::Less => CaseSupportTreapNode::rebuild(
            &root,
            Some(case_support_treap_insert_absent(
                root.left.clone(),
                interval,
            )?),
            root.right.clone(),
        ),
        std::cmp::Ordering::Greater => CaseSupportTreapNode::rebuild(
            &root,
            root.left.clone(),
            Some(case_support_treap_insert_absent(
                root.right.clone(),
                interval,
            )?),
        ),
        std::cmp::Ordering::Equal => {
            unreachable!("case_support_treap_insert_absent requires a missing start")
        }
    }
}

fn case_support_treap_split(
    root: Option<Arc<CaseSupportTreapNode>>,
    start: u128,
) -> Result<
    (
        Option<Arc<CaseSupportTreapNode>>,
        Option<Arc<CaseSupportTreapNode>>,
    ),
    ExploreRunStreamError,
> {
    let Some(root) = root else {
        return Ok((None, None));
    };
    if root.interval.start < start {
        let (middle, right) = case_support_treap_split(root.right.clone(), start)?;
        Ok((
            Some(CaseSupportTreapNode::rebuild(
                &root,
                root.left.clone(),
                middle,
            )?),
            right,
        ))
    } else {
        let (left, middle) = case_support_treap_split(root.left.clone(), start)?;
        Ok((
            left,
            Some(CaseSupportTreapNode::rebuild(
                &root,
                middle,
                root.right.clone(),
            )?),
        ))
    }
}

fn case_support_treap_delete(
    root: Option<Arc<CaseSupportTreapNode>>,
    start: u128,
) -> Result<Option<Arc<CaseSupportTreapNode>>, ExploreRunStreamError> {
    let Some(root) = root else {
        return Ok(None);
    };
    match start.cmp(&root.interval.start) {
        std::cmp::Ordering::Less => Ok(Some(CaseSupportTreapNode::rebuild(
            &root,
            case_support_treap_delete(root.left.clone(), start)?,
            root.right.clone(),
        )?)),
        std::cmp::Ordering::Greater => Ok(Some(CaseSupportTreapNode::rebuild(
            &root,
            root.left.clone(),
            case_support_treap_delete(root.right.clone(), start)?,
        )?)),
        std::cmp::Ordering::Equal => {
            case_support_treap_merge(root.left.clone(), root.right.clone())
        }
    }
}

fn case_support_treap_merge(
    left: Option<Arc<CaseSupportTreapNode>>,
    right: Option<Arc<CaseSupportTreapNode>>,
) -> Result<Option<Arc<CaseSupportTreapNode>>, ExploreRunStreamError> {
    match (left, right) {
        (None, right) => Ok(right),
        (left, None) => Ok(left),
        (Some(left), Some(right)) => {
            if (left.priority, left.interval.start) < (right.priority, right.interval.start) {
                Ok(Some(CaseSupportTreapNode::rebuild(
                    &left,
                    left.left.clone(),
                    case_support_treap_merge(left.right.clone(), Some(right))?,
                )?))
            } else {
                Ok(Some(CaseSupportTreapNode::rebuild(
                    &right,
                    case_support_treap_merge(Some(left), right.left.clone())?,
                    right.right.clone(),
                )?))
            }
        }
    }
}

/// One non-case completion obligation named by the report contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RequiredObligationId(CanonicalDigest);

impl RequiredObligationId {
    pub(crate) fn new(identity: CanonicalDigest) -> Self {
        Self(identity)
    }

    pub(crate) fn identity(self) -> CanonicalDigest {
        self.0
    }
}

/// Exact completion-blocking frontier at one committed cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequiredFrontier {
    open_cases: ExactCaseSupport,
    open_obligations: BTreeSet<RequiredObligationId>,
    id: RequiredFrontierId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RequiredFrontierId([u8; 32]);

impl RequiredFrontier {
    pub(crate) fn new(
        open_cases: ExactCaseSupport,
        open_obligations: impl IntoIterator<Item = RequiredObligationId>,
    ) -> Result<Self, ExploreRunStreamError> {
        let mut obligations = BTreeSet::new();
        for obligation in open_obligations {
            if !obligations.insert(obligation) {
                return Err(ExploreRunStreamError::DuplicateRequiredObligation);
            }
        }
        Ok(Self::from_normalized(open_cases, obligations))
    }

    fn from_normalized(
        open_cases: ExactCaseSupport,
        open_obligations: BTreeSet<RequiredObligationId>,
    ) -> Self {
        let mut hasher = StableHasher::new(REQUIRED_FRONTIER_HASH_V2);
        hasher.segment(&open_cases.id.0);
        hasher.u128(open_obligations.len() as u128);
        for obligation in &open_obligations {
            obligation.0.hash_into(&mut hasher);
        }
        Self {
            open_cases,
            open_obligations,
            id: RequiredFrontierId(hasher.finish()),
        }
    }

    pub(crate) fn open_cases(&self) -> &ExactCaseSupport {
        &self.open_cases
    }

    pub(crate) fn open_obligations(&self) -> &BTreeSet<RequiredObligationId> {
        &self.open_obligations
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.open_cases.is_empty() && self.open_obligations.is_empty()
    }

    pub(crate) fn identity_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.id.0)
    }

    /// Derive the unique next frontier by removing one exact bounded delta.
    fn close_exact(&self, newly_closed: &Self) -> Result<Self, ExploreRunStreamError> {
        let open_cases = self.open_cases.subtract_exact(&newly_closed.open_cases)?;
        if !newly_closed
            .open_obligations
            .is_subset(&self.open_obligations)
        {
            return Err(ExploreRunStreamError::FrontierNotConserved);
        }
        let open_obligations = self
            .open_obligations
            .difference(&newly_closed.open_obligations)
            .copied()
            .collect();
        Ok(Self::from_normalized(open_cases, open_obligations))
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.segment(&self.id.0);
    }
}

/// Semantic layer in the canonical answer state.
///
/// These variants describe what a fact means. They deliberately do not encode
/// whether the fact arrived from a probe, singleton evaluator, region
/// certificate or replay worker; that producer provenance belongs only to the
/// ordered journal payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum SemanticEvidenceLayer {
    CaseClassification,
    RepresentativeSelection,
    MechanismObservation,
    ExtremaWitness,
    MechanismTargetClosure,
    AnswerAggregation,
}

impl SemanticEvidenceLayer {
    fn hash_into(self, hasher: &mut StableHasher) {
        hasher.u8(match self {
            Self::CaseClassification => 0,
            Self::RepresentativeSelection => 1,
            Self::MechanismObservation => 2,
            Self::ExtremaWitness => 3,
            Self::MechanismTargetClosure => 4,
            Self::AnswerAggregation => 5,
        });
    }

    fn closes_case_frontier(self) -> bool {
        self == Self::CaseClassification
    }

    fn closes_obligation_frontier(self) -> bool {
        matches!(
            self,
            Self::RepresentativeSelection | Self::ExtremaWitness | Self::MechanismTargetClosure
        )
    }

    fn closes_any_frontier(self) -> bool {
        self.closes_case_frontier() || self.closes_obligation_frontier()
    }
}

/// Canonical subject proven by one semantic fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SemanticEvidenceSubject {
    Cases(ExactCaseSupport),
    Obligations(BTreeSet<RequiredObligationId>),
    Global,
}

impl SemanticEvidenceSubject {
    pub(crate) fn cases(support: ExactCaseSupport) -> Self {
        Self::Cases(support)
    }

    pub(crate) fn obligations(
        obligations: impl IntoIterator<Item = RequiredObligationId>,
    ) -> Result<Self, ExploreRunStreamError> {
        let mut normalized = BTreeSet::new();
        for obligation in obligations {
            if !normalized.insert(obligation) {
                return Err(ExploreRunStreamError::DuplicateRequiredObligation);
            }
        }
        Ok(Self::Obligations(normalized))
    }

    pub(crate) fn global() -> Self {
        Self::Global
    }

    fn merge_disjoint(&self, other: &Self) -> Result<Self, ExploreRunStreamError> {
        match (self, other) {
            (Self::Cases(left), Self::Cases(right)) => Ok(Self::Cases(left.merge_disjoint(right)?)),
            (Self::Obligations(left), Self::Obligations(right)) => {
                if !left.is_disjoint(right) {
                    return Err(ExploreRunStreamError::OverlappingSemanticEvidence);
                }
                Ok(Self::Obligations(left.union(right).copied().collect()))
            }
            (Self::Global, Self::Global) => {
                Err(ExploreRunStreamError::DuplicateGlobalSemanticEvidence)
            }
            _ => Err(ExploreRunStreamError::ConflictingSemanticEvidenceSubject),
        }
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        match self {
            Self::Cases(support) => {
                hasher.u8(0);
                support.hash_into(hasher);
            }
            Self::Obligations(obligations) => {
                hasher.u8(1);
                hasher.u128(obligations.len() as u128);
                for obligation in obligations {
                    obligation.0.hash_into(hasher);
                }
            }
            Self::Global => hasher.u8(2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct SemanticEvidenceKey {
    layer: SemanticEvidenceLayer,
    normalized_content_hash: CanonicalDigest,
}

impl SemanticEvidenceKey {
    fn hash_into(self, hasher: &mut StableHasher) {
        self.layer.hash_into(hasher);
        self.normalized_content_hash.hash_into(hasher);
    }

    fn identity(self) -> [u8; 32] {
        let mut hasher = StableHasher::new(SEMANTIC_KEY_HASH_V1);
        self.hash_into(&mut hasher);
        hasher.finish()
    }
}

/// One normalized semantic relation in the answer state.
///
/// Repeated commits for the same `(layer, normalized_content_hash)` merge only
/// disjoint subjects. Consequently one wide batch and many smaller batches
/// have the same final entry and authenticated root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticEvidenceFact {
    key: SemanticEvidenceKey,
    subject: SemanticEvidenceSubject,
}

impl SemanticEvidenceFact {
    pub(crate) fn new(
        layer: SemanticEvidenceLayer,
        normalized_content_hash: CanonicalDigest,
        subject: SemanticEvidenceSubject,
    ) -> Result<Self, ExploreRunStreamError> {
        match (layer, &subject) {
            (SemanticEvidenceLayer::CaseClassification, SemanticEvidenceSubject::Cases(_)) => {}
            (layer, SemanticEvidenceSubject::Obligations(_))
                if layer.closes_obligation_frontier() => {}
            (layer, _) if !layer.closes_any_frontier() => {}
            _ => return Err(ExploreRunStreamError::SemanticLayerSubjectMismatch),
        }
        Ok(Self {
            key: SemanticEvidenceKey {
                layer,
                normalized_content_hash,
            },
            subject,
        })
    }

    pub(crate) fn layer(&self) -> SemanticEvidenceLayer {
        self.key.layer
    }

    pub(crate) fn normalized_content_hash(&self) -> CanonicalDigest {
        self.key.normalized_content_hash
    }

    pub(crate) fn subject(&self) -> &SemanticEvidenceSubject {
        &self.subject
    }

    fn merge_disjoint(&self, other: &Self) -> Result<Self, ExploreRunStreamError> {
        if self.key != other.key {
            return Err(ExploreRunStreamError::ConflictingSemanticEvidenceKey);
        }
        Ok(Self {
            key: self.key,
            subject: self.subject.merge_disjoint(&other.subject)?,
        })
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        self.key.hash_into(hasher);
        self.subject.hash_into(hasher);
    }

    fn entry_hash(&self) -> [u8; 32] {
        let mut hasher = StableHasher::new(SEMANTIC_ENTRY_HASH_V2);
        self.hash_into(&mut hasher);
        hasher.finish()
    }
}

/// Persistent canonical authenticated map keyed by semantic identity.
///
/// A key-derived total heap priority gives the treap a unique shape for one
/// map, independent of insertion order. `Arc` path copying changes only the
/// search path for an insertion or value replacement; earlier prepared states
/// remain immutable until storage authorizes their installation.
#[derive(Clone, Debug)]
struct SemanticEvidenceMap {
    root: Option<Arc<SemanticTreapNode>>,
    len: u128,
}

#[derive(Debug)]
struct SemanticTreapNode {
    fact: SemanticEvidenceFact,
    priority: [u8; 32],
    left: Option<Arc<Self>>,
    right: Option<Arc<Self>>,
    subtree_hash: [u8; 32],
}

impl SemanticTreapNode {
    fn new(
        fact: SemanticEvidenceFact,
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Arc<Self> {
        let priority = semantic_treap_priority(fact.key);
        let mut hasher = StableHasher::new(SEMANTIC_TREAP_NODE_HASH_V2);
        hasher.segment(&semantic_treap_hash(&left));
        hasher.segment(&fact.entry_hash());
        hasher.segment(&semantic_treap_hash(&right));
        Arc::new(Self {
            fact,
            priority,
            left,
            right,
            subtree_hash: hasher.finish(),
        })
    }
}

impl SemanticEvidenceMap {
    fn empty() -> Self {
        Self { root: None, len: 0 }
    }

    fn root_hash(&self) -> [u8; 32] {
        semantic_treap_hash(&self.root)
    }

    fn canonical_facts(&self) -> Vec<SemanticEvidenceFact> {
        let mut facts = Vec::new();
        semantic_treap_collect(&self.root, &mut facts);
        facts
    }

    fn with_facts(&self, facts: &[SemanticEvidenceFact]) -> Result<Self, ExploreRunStreamError> {
        let mut next = self.clone();
        for fact in facts {
            next = next.with_fact(fact.clone())?;
        }
        Ok(next)
    }

    fn with_fact(&self, fact: SemanticEvidenceFact) -> Result<Self, ExploreRunStreamError> {
        if let Some(existing) = semantic_treap_get(self.root.as_ref(), fact.key) {
            let merged = existing.merge_disjoint(&fact)?;
            return Ok(Self {
                root: Some(semantic_treap_replace(
                    self.root
                        .as_ref()
                        .expect("a found semantic entry requires a root"),
                    merged,
                )),
                len: self.len,
            });
        }
        Ok(Self {
            root: Some(semantic_treap_insert_absent(self.root.clone(), fact)),
            len: self
                .len
                .checked_add(1)
                .ok_or(ExploreRunStreamError::SemanticEvidenceCountOverflow)?,
        })
    }
}

fn semantic_treap_collect(
    root: &Option<Arc<SemanticTreapNode>>,
    facts: &mut Vec<SemanticEvidenceFact>,
) {
    let Some(node) = root else {
        return;
    };
    semantic_treap_collect(&node.left, facts);
    facts.push(node.fact.clone());
    semantic_treap_collect(&node.right, facts);
}

fn semantic_treap_empty_hash() -> [u8; 32] {
    StableHasher::new(SEMANTIC_TREAP_EMPTY_HASH_V2).finish()
}

fn semantic_treap_hash(root: &Option<Arc<SemanticTreapNode>>) -> [u8; 32] {
    root.as_ref()
        .map_or_else(semantic_treap_empty_hash, |node| node.subtree_hash)
}

fn semantic_treap_priority(key: SemanticEvidenceKey) -> [u8; 32] {
    let mut hasher = StableHasher::new(SEMANTIC_TREAP_PRIORITY_HASH_V1);
    hasher.segment(&key.identity());
    hasher.finish()
}

fn semantic_priority_precedes(
    priority: [u8; 32],
    key: SemanticEvidenceKey,
    node: &SemanticTreapNode,
) -> bool {
    (priority, key) < (node.priority, node.fact.key)
}

fn semantic_treap_get(
    mut root: Option<&Arc<SemanticTreapNode>>,
    key: SemanticEvidenceKey,
) -> Option<&SemanticEvidenceFact> {
    while let Some(node) = root {
        match key.cmp(&node.fact.key) {
            std::cmp::Ordering::Less => root = node.left.as_ref(),
            std::cmp::Ordering::Greater => root = node.right.as_ref(),
            std::cmp::Ordering::Equal => return Some(&node.fact),
        }
    }
    None
}

fn semantic_treap_replace(
    root: &Arc<SemanticTreapNode>,
    replacement: SemanticEvidenceFact,
) -> Arc<SemanticTreapNode> {
    match replacement.key.cmp(&root.fact.key) {
        std::cmp::Ordering::Less => SemanticTreapNode::new(
            root.fact.clone(),
            Some(semantic_treap_replace(
                root.left
                    .as_ref()
                    .expect("replacement key was previously found in the left subtree"),
                replacement,
            )),
            root.right.clone(),
        ),
        std::cmp::Ordering::Greater => SemanticTreapNode::new(
            root.fact.clone(),
            root.left.clone(),
            Some(semantic_treap_replace(
                root.right
                    .as_ref()
                    .expect("replacement key was previously found in the right subtree"),
                replacement,
            )),
        ),
        std::cmp::Ordering::Equal => {
            SemanticTreapNode::new(replacement, root.left.clone(), root.right.clone())
        }
    }
}

fn semantic_treap_insert_absent(
    root: Option<Arc<SemanticTreapNode>>,
    fact: SemanticEvidenceFact,
) -> Arc<SemanticTreapNode> {
    let Some(root) = root else {
        return SemanticTreapNode::new(fact, None, None);
    };
    let priority = semantic_treap_priority(fact.key);
    if semantic_priority_precedes(priority, fact.key, &root) {
        let (left, right) = semantic_treap_split(Some(root), fact.key);
        return SemanticTreapNode::new(fact, left, right);
    }
    match fact.key.cmp(&root.fact.key) {
        std::cmp::Ordering::Less => SemanticTreapNode::new(
            root.fact.clone(),
            Some(semantic_treap_insert_absent(root.left.clone(), fact)),
            root.right.clone(),
        ),
        std::cmp::Ordering::Greater => SemanticTreapNode::new(
            root.fact.clone(),
            root.left.clone(),
            Some(semantic_treap_insert_absent(root.right.clone(), fact)),
        ),
        std::cmp::Ordering::Equal => {
            unreachable!("semantic_treap_insert_absent requires a missing key")
        }
    }
}

fn semantic_treap_split(
    root: Option<Arc<SemanticTreapNode>>,
    key: SemanticEvidenceKey,
) -> (
    Option<Arc<SemanticTreapNode>>,
    Option<Arc<SemanticTreapNode>>,
) {
    let Some(root) = root else {
        return (None, None);
    };
    if root.fact.key < key {
        let (middle, right) = semantic_treap_split(root.right.clone(), key);
        (
            Some(SemanticTreapNode::new(
                root.fact.clone(),
                root.left.clone(),
                middle,
            )),
            right,
        )
    } else {
        let (left, middle) = semantic_treap_split(root.left.clone(), key);
        (
            left,
            Some(SemanticTreapNode::new(
                root.fact.clone(),
                middle,
                root.right.clone(),
            )),
        )
    }
}

fn normalize_semantic_facts(
    facts: impl IntoIterator<Item = SemanticEvidenceFact>,
) -> Result<Box<[SemanticEvidenceFact]>, ExploreRunStreamError> {
    let mut normalized = BTreeMap::<SemanticEvidenceKey, SemanticEvidenceFact>::new();
    for fact in facts {
        if let Some(existing) = normalized.remove(&fact.key) {
            let merged = existing.merge_disjoint(&fact)?;
            normalized.insert(merged.key, merged);
        } else {
            normalized.insert(fact.key, fact);
        }
    }
    let facts = normalized.into_values().collect::<Vec<_>>();
    validate_exclusive_fact_batch(&facts)?;
    Ok(facts.into_boxed_slice())
}

fn validate_exclusive_fact_batch(
    facts: &[SemanticEvidenceFact],
) -> Result<(), ExploreRunStreamError> {
    let mut case_intervals = Vec::<(ExploreRankInterval, SemanticEvidenceKey)>::new();
    let mut obligation_owners = BTreeMap::<RequiredObligationId, SemanticEvidenceKey>::new();
    for fact in facts {
        match (&fact.key.layer, &fact.subject) {
            (
                SemanticEvidenceLayer::CaseClassification,
                SemanticEvidenceSubject::Cases(support),
            ) => {
                case_intervals.extend(
                    support
                        .iter_intervals()
                        .map(|interval| (interval, fact.key)),
                );
            }
            (layer, SemanticEvidenceSubject::Obligations(obligations))
                if layer.closes_obligation_frontier() =>
            {
                for obligation in obligations {
                    if obligation_owners.insert(*obligation, fact.key).is_some() {
                        return Err(ExploreRunStreamError::ContradictorySemanticEvidence);
                    }
                }
            }
            _ => {}
        }
    }
    case_intervals.sort_unstable_by_key(|(interval, key)| (*interval, *key));
    for pair in case_intervals.windows(2) {
        let (left, left_key) = pair[0];
        let (right, right_key) = pair[1];
        if right.start < left.end_exclusive && left_key != right_key {
            return Err(ExploreRunStreamError::ContradictorySemanticEvidence);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct AnswerScopeId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RunHeaderCommitment([u8; 32]);

/// Caller-supplied unique seed for one attempt at the same immutable question.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExploreRunNonce(CanonicalDigest);

impl ExploreRunNonce {
    pub(crate) fn new(value: CanonicalDigest) -> Result<Self, ExploreRunStreamError> {
        if value.0 == [0; 32] {
            return Err(ExploreRunStreamError::ZeroRunNonce);
        }
        Ok(Self(value))
    }

    pub(crate) fn identity(self) -> CanonicalDigest {
        self.0
    }
}

/// Stable identity of one durable exploration attempt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExploreRunId([u8; 32]);

impl ExploreRunId {
    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStreamError> {
        Ok(Self(
            CanonicalDigest::from_lowercase_sha256("run_id", value)?.0,
        ))
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }
}

/// Immutable pre-probe genesis header.
///
/// The full case universe and required obligations are bound here. A proof
/// residual, shard width, jobs, time limit and checkpoint cadence are
/// intentionally impossible to put in this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExploreRunHeader {
    identity: ExploreRunIdentity,
    case_universe: ExploreCaseUniverse,
    required_obligations: BTreeSet<RequiredObligationId>,
    answer_scope_id: AnswerScopeId,
    commitment: RunHeaderCommitment,
    nonce: ExploreRunNonce,
    run_id: ExploreRunId,
}

impl ExploreRunHeader {
    pub(crate) fn new(
        identity: ExploreRunIdentity,
        case_universe: ExploreCaseUniverse,
        required_obligations: impl IntoIterator<Item = RequiredObligationId>,
        nonce: ExploreRunNonce,
    ) -> Result<Self, ExploreRunStreamError> {
        let mut obligations = BTreeSet::new();
        for obligation in required_obligations {
            if !obligations.insert(obligation) {
                return Err(ExploreRunStreamError::DuplicateRequiredObligation);
            }
        }

        let mut answer_scope = StableHasher::new(ANSWER_SCOPE_HASH_V1);
        identity.hash_answer_scope_into(&mut answer_scope);
        case_universe.hash_into(&mut answer_scope);
        answer_scope.u128(obligations.len() as u128);
        for obligation in &obligations {
            obligation.0.hash_into(&mut answer_scope);
        }
        let answer_scope_id = AnswerScopeId(answer_scope.finish());

        let mut commitment = StableHasher::new(RUN_HEADER_HASH_V1);
        identity.hash_header_into(&mut commitment);
        case_universe.hash_into(&mut commitment);
        commitment.u128(obligations.len() as u128);
        for obligation in &obligations {
            obligation.0.hash_into(&mut commitment);
        }
        let commitment = RunHeaderCommitment(commitment.finish());

        let mut run_id = StableHasher::new(RUN_ID_HASH_V1);
        run_id.segment(&commitment.0);
        nonce.0.hash_into(&mut run_id);
        let run_id = ExploreRunId(run_id.finish());

        Ok(Self {
            identity,
            case_universe,
            required_obligations: obligations,
            answer_scope_id,
            commitment,
            nonce,
            run_id,
        })
    }

    pub(crate) fn run_id(&self) -> ExploreRunId {
        self.run_id
    }

    pub(crate) fn identity(&self) -> &ExploreRunIdentity {
        &self.identity
    }

    pub(crate) fn case_universe(&self) -> &ExploreCaseUniverse {
        &self.case_universe
    }

    pub(crate) fn required_obligations(&self) -> &BTreeSet<RequiredObligationId> {
        &self.required_obligations
    }

    pub(crate) fn nonce(&self) -> ExploreRunNonce {
        self.nonce
    }

    pub(crate) fn answer_scope_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.answer_scope_id.0)
    }

    pub(crate) fn commitment_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.commitment.0)
    }

    fn initial_frontier(&self) -> RequiredFrontier {
        RequiredFrontier::from_normalized(
            ExactCaseSupport::full(&self.case_universe),
            self.required_obligations.clone(),
        )
    }
}

/// Identity of the process/coordinator holding a fenced writer grant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExploreWriterId(CanonicalDigest);

impl ExploreWriterId {
    pub(crate) fn new(identity: CanonicalDigest) -> Self {
        Self(identity)
    }

    pub(crate) fn identity(self) -> CanonicalDigest {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct FencedLeaseId([u8; 32]);

/// Storage-authorized writer grant.
///
/// Its constructor validates shape only. The storage adapter must call it only
/// after atomically acquiring the run lock and advancing its persistent fence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FencedWriterLease {
    run_id: ExploreRunId,
    generation: NonZeroU64,
    writer_id: ExploreWriterId,
    fence_receipt_hash: CanonicalDigest,
    id: FencedLeaseId,
}

impl FencedWriterLease {
    /// Mint boundary for the owner-local storage coordinator.
    ///
    /// Until the storage adapter is a child module with its own private mint
    /// token, visibility is restricted to the enclosing Explore subsystem.
    /// Callers MUST hold the durable writer lock and have atomically advanced
    /// the persistent fence before constructing this capability.
    pub(super) fn new(
        run_id: ExploreRunId,
        generation: NonZeroU64,
        writer_id: ExploreWriterId,
        fence_receipt_hash: CanonicalDigest,
    ) -> Self {
        let mut hasher = StableHasher::new(LEASE_ID_HASH_V1);
        hasher.segment(&run_id.0);
        hasher.u64(generation.get());
        writer_id.0.hash_into(&mut hasher);
        fence_receipt_hash.hash_into(&mut hasher);
        Self {
            run_id,
            generation,
            writer_id,
            fence_receipt_hash,
            id: FencedLeaseId(hasher.finish()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Reconstruct a historical lease embedded in a committed payload. This
    /// recomputes its content identity but does not reacquire the storage lock;
    /// it is replay input only and MUST NOT authorize fresh dispatch. A live
    /// coordinator continues only with the separately minted lock-held lease.
    pub(super) fn from_recorded_fields(
        run_id: ExploreRunId,
        generation: NonZeroU64,
        writer_id: ExploreWriterId,
        fence_receipt_hash: CanonicalDigest,
        expected_lease_id_hash: CanonicalDigest,
    ) -> Result<Self, ExploreRunStreamError> {
        let lease = Self::new(run_id, generation, writer_id, fence_receipt_hash);
        if lease.id.0 != expected_lease_id_hash.0 {
            return Err(ExploreRunStreamError::RecordedLeaseIdMismatch);
        }
        Ok(lease)
    }

    pub(crate) fn run_id(self) -> ExploreRunId {
        self.run_id
    }

    pub(crate) fn generation(self) -> NonZeroU64 {
        self.generation
    }

    pub(crate) fn lease_id_hash(self) -> CanonicalDigest {
        CanonicalDigest(self.id.0)
    }

    pub(crate) fn writer_id(self) -> ExploreWriterId {
        self.writer_id
    }

    pub(crate) fn fence_receipt_hash(self) -> CanonicalDigest {
        self.fence_receipt_hash
    }

    fn hash_into(self, hasher: &mut StableHasher) {
        hasher.segment(&self.id.0);
        hasher.u64(self.generation.get());
        self.writer_id.0.hash_into(hasher);
        self.fence_receipt_hash.hash_into(hasher);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct JournalHead([u8; 32]);

impl JournalHead {
    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStreamError> {
        Ok(Self(
            CanonicalDigest::from_lowercase_sha256("journal_head", value)?.0,
        ))
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct EvidenceRoot([u8; 32]);

impl EvidenceRoot {
    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStreamError> {
        Ok(Self(
            CanonicalDigest::from_lowercase_sha256("evidence_root", value)?.0,
        ))
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunLifecycle {
    Running,
    Paused,
    Sealed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PauseReason {
    Explicit,
    TimeLimit,
    Interrupt,
    ResourcePressure,
    StorageLimit,
    ProbeMilestone,
    EvaluationLimit,
    FinalizationPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlEventKind {
    RunOpened,
    Paused(PauseReason),
    Resumed,
    Recovered,
    TerminalSealed(TerminalSealKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryEventKind {
    ProbeDecision,
    CandidateDiscovered,
    LiftScheduled,
    ProbePlanCompleted,
    SchedulingHint,
    /// Content-addressed pointer to a canonical derived snapshot. Publishing
    /// it changes the ordered journal only; replay derives the same answer
    /// state and verifies the snapshot separately.
    SnapshotPublished,
    /// Content-addressed pointer to the history-independent semantic answer
    /// bytes later committed by a terminal seal.
    TerminalResultPublished,
    /// Content-addressed bounded source-probe transcript. The manifest binds
    /// candidate/coverage blobs and is the durable restart seam before any
    /// candidate CaseId is evaluated.
    ProbePlanPrepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrontierEvidenceKind {
    SingletonClassification,
    /// One coordinator-bounded batch of fully evaluated whole CaseIds.
    ///
    /// This is ordered-journal provenance only. Normalized semantic evidence
    /// remains derived per CaseId, so changing operational batch boundaries
    /// cannot change the EvidenceRoot.
    BoundedExactBatchClassification,
    CertifiedRegionClassification,
    ExactExhaustion,
    RepresentativeSelectionClosed,
    MechanismTargetClosed,
    /// One bounded block drawn exclusively from the prepared source-probe
    /// candidate set. This tag records provenance only; normalized per-CaseId
    /// facts and the compact closure delta are shared with ordinary batches.
    ProbeCandidateBatchClassification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationEvidenceKind {
    RepresentativeReplayed,
    MechanismObserved,
    ExtremaWitnessReplayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EvidenceEventKind {
    CoveragePlanAccepted,
    FrontierAdvanced(FrontierEvidenceKind),
    ObservationAccepted(ObservationEvidenceKind),
}

/// The outer variant is the record's trust class, so discovery payloads cannot
/// be accidentally accepted through an evidence API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunEventKind {
    Control(ControlEventKind),
    Discovery(DiscoveryEventKind),
    Evidence(EvidenceEventKind),
}

/// One immutable committed event envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommittedRunEvent {
    run_id: ExploreRunId,
    sequence: u64,
    previous_journal_head: JournalHead,
    journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    kind: RunEventKind,
    canonical_payload_hash: CanonicalDigest,
    lease_generation: NonZeroU64,
    lease_id: FencedLeaseId,
}

impl CommittedRunEvent {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_decoded_envelope(
        run_id: ExploreRunId,
        sequence: u64,
        previous_journal_head: JournalHead,
        journal_head: JournalHead,
        evidence_root: EvidenceRoot,
        kind: RunEventKind,
        canonical_payload_hash: CanonicalDigest,
        lease_generation: NonZeroU64,
        lease_id_hash: CanonicalDigest,
        payload: &CanonicalRunRecordPayload,
    ) -> Result<Self, ExploreRunStreamError> {
        let lease = payload.lease();
        let genesis_shape_valid = if sequence == 0 {
            matches!(payload, CanonicalRunRecordPayload::RunOpened { .. })
                && previous_journal_head == derive_journal_anchor(run_id)
        } else {
            !matches!(payload, CanonicalRunRecordPayload::RunOpened { .. })
        };
        if !genesis_shape_valid
            || lease.run_id != run_id
            || payload.event_kind() != kind
            || payload.canonical_payload_hash() != canonical_payload_hash
            || lease.generation != lease_generation
            || lease.id.0 != lease_id_hash.0
            || derive_journal_head(
                run_id,
                sequence,
                previous_journal_head,
                evidence_root,
                kind,
                canonical_payload_hash,
                lease,
            ) != journal_head
        {
            return Err(ExploreRunStreamError::CommittedEnvelopeMismatch);
        }
        Ok(Self {
            run_id,
            sequence,
            previous_journal_head,
            journal_head,
            evidence_root,
            kind,
            canonical_payload_hash,
            lease_generation,
            lease_id: lease.id,
        })
    }

    pub(crate) fn run_id(&self) -> ExploreRunId {
        self.run_id
    }

    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn previous_journal_head(&self) -> JournalHead {
        self.previous_journal_head
    }

    pub(crate) fn journal_head(&self) -> JournalHead {
        self.journal_head
    }

    pub(crate) fn evidence_root(&self) -> EvidenceRoot {
        self.evidence_root
    }

    pub(crate) fn kind(&self) -> RunEventKind {
        self.kind
    }

    pub(crate) fn canonical_payload_hash(&self) -> CanonicalDigest {
        self.canonical_payload_hash
    }

    pub(crate) fn lease_generation(&self) -> NonZeroU64 {
        self.lease_generation
    }

    pub(crate) fn lease_id_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.lease_id.0)
    }
}

/// Durable continuation cursor. Passing a stale cursor fails without mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExploreRunCursor {
    run_id: ExploreRunId,
    sequence: u64,
    journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    lifecycle: RunLifecycle,
    active_lease_id: Option<FencedLeaseId>,
    last_lease_generation: NonZeroU64,
    last_coverage_epoch: Option<NonZeroU64>,
}

impl ExploreRunCursor {
    pub(crate) fn run_id(self) -> ExploreRunId {
        self.run_id
    }

    pub(crate) fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) fn journal_head(self) -> JournalHead {
        self.journal_head
    }

    pub(crate) fn evidence_root(self) -> EvidenceRoot {
        self.evidence_root
    }

    pub(crate) fn lifecycle(self) -> RunLifecycle {
        self.lifecycle
    }

    pub(crate) fn active_lease_id_hash(self) -> Option<CanonicalDigest> {
        self.active_lease_id.map(|id| CanonicalDigest(id.0))
    }

    pub(crate) fn last_lease_generation(self) -> NonZeroU64 {
        self.last_lease_generation
    }

    pub(crate) fn last_coverage_epoch(self) -> Option<NonZeroU64> {
        self.last_coverage_epoch
    }
}

/// A proof/residual/sharding plan accepted only after `RunOpened`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoveragePlan {
    run_id: ExploreRunId,
    proof_set_id: CanonicalDigest,
    certified_closed: ExactCaseSupport,
    residual_open: ExactCaseSupport,
    semantic_facts: Box<[SemanticEvidenceFact]>,
    proof_receipt_hash: CanonicalDigest,
    sharding_epoch: NonZeroU64,
    shard_width: NonZeroU64,
    id: CoveragePlanId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CoveragePlanId([u8; 32]);

impl CoveragePlan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        header: &ExploreRunHeader,
        proof_set_id: CanonicalDigest,
        certified_closed: ExactCaseSupport,
        residual_open: ExactCaseSupport,
        semantic_facts: impl IntoIterator<Item = SemanticEvidenceFact>,
        proof_receipt_hash: CanonicalDigest,
        sharding_epoch: NonZeroU64,
        shard_width: NonZeroU64,
    ) -> Result<Self, ExploreRunStreamError> {
        if certified_closed.universe_id != header.case_universe.id
            || residual_open.universe_id != header.case_universe.id
        {
            return Err(ExploreRunStreamError::StaleCaseUniverse);
        }
        let semantic_facts = normalize_semantic_facts(semantic_facts)?;
        validate_fact_universes(header.case_universe(), &semantic_facts)?;
        validate_frontier_facts(
            &RequiredFrontier::from_normalized(certified_closed.clone(), BTreeSet::new()),
            &semantic_facts,
        )?;
        let mut hasher = StableHasher::new(COVERAGE_PLAN_HASH_V2);
        hasher.segment(&header.run_id.0);
        proof_set_id.hash_into(&mut hasher);
        hasher.segment(&certified_closed.id.0);
        hasher.segment(&residual_open.id.0);
        hash_semantic_fact_batch(&semantic_facts, &mut hasher);
        proof_receipt_hash.hash_into(&mut hasher);
        hasher.u64(sharding_epoch.get());
        hasher.u64(shard_width.get());
        Ok(Self {
            run_id: header.run_id,
            proof_set_id,
            certified_closed,
            residual_open,
            semantic_facts,
            proof_receipt_hash,
            sharding_epoch,
            shard_width,
            id: CoveragePlanId(hasher.finish()),
        })
    }

    pub(crate) fn sharding_epoch(&self) -> NonZeroU64 {
        self.sharding_epoch
    }

    pub(crate) fn run_id(&self) -> ExploreRunId {
        self.run_id
    }

    pub(crate) fn proof_set_id(&self) -> CanonicalDigest {
        self.proof_set_id
    }

    pub(crate) fn certified_closed(&self) -> &ExactCaseSupport {
        &self.certified_closed
    }

    pub(crate) fn residual_open(&self) -> &ExactCaseSupport {
        &self.residual_open
    }

    pub(crate) fn semantic_facts(&self) -> &[SemanticEvidenceFact] {
        &self.semantic_facts
    }

    pub(crate) fn proof_receipt_hash(&self) -> CanonicalDigest {
        self.proof_receipt_hash
    }

    pub(crate) fn identity_hash(&self) -> CanonicalDigest {
        CanonicalDigest(self.id.0)
    }

    pub(crate) fn shard_width(&self) -> NonZeroU64 {
        self.shard_width
    }

    fn hash_into(&self, hasher: &mut StableHasher) {
        hasher.segment(&self.id.0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSealKind {
    Completed,
    Partial,
    Unknown,
    Unsupported,
    Error,
    Cancelled,
}

/// Hash of `A = render(E)`, where `A` omits history, execution metadata and the
/// seal envelope. It is computed before a terminal journal head exists.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct TerminalPayloadHash([u8; 32]);

impl TerminalPayloadHash {
    pub(crate) fn from_canonical_semantic_payload(bytes: &[u8]) -> Self {
        let mut hasher = StableHasher::new(TERMINAL_PAYLOAD_HASH_V1);
        hasher.segment(bytes);
        Self(hasher.finish())
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }

    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStreamError> {
        Ok(Self(
            CanonicalDigest::from_lowercase_sha256("terminal_payload_hash", value)?.0,
        ))
    }
}

/// Canonical completion/stop-method commitment, also constructed before seal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct TerminalMethodHash([u8; 32]);

impl TerminalMethodHash {
    pub(crate) fn from_canonical_method(bytes: &[u8]) -> Self {
        let mut hasher = StableHasher::new(TERMINAL_METHOD_HASH_V1);
        hasher.segment(bytes);
        Self(hasher.finish())
    }

    pub(crate) fn from_lowercase_sha256(value: &str) -> Result<Self, ExploreRunStreamError> {
        Ok(Self(
            CanonicalDigest::from_lowercase_sha256("terminal_method_hash", value)?.0,
        ))
    }

    pub(crate) fn to_lowercase_hex(self) -> String {
        lowercase_hex(&self.0)
    }
}

/// Non-circular terminal commitment. `terminal_journal_head` is derived only
/// after a record committing all preceding fields has been appended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSeal {
    kind: TerminalSealKind,
    journal_head_before_seal: JournalHead,
    terminal_journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    terminal_payload_hash: TerminalPayloadHash,
    method_hash: TerminalMethodHash,
}

impl TerminalSeal {
    pub(crate) fn kind(&self) -> TerminalSealKind {
        self.kind
    }

    pub(crate) fn journal_head_before_seal(&self) -> JournalHead {
        self.journal_head_before_seal
    }

    pub(crate) fn terminal_journal_head(&self) -> JournalHead {
        self.terminal_journal_head
    }

    pub(crate) fn evidence_root(&self) -> EvidenceRoot {
        self.evidence_root
    }

    pub(crate) fn terminal_payload_hash(&self) -> TerminalPayloadHash {
        self.terminal_payload_hash
    }

    pub(crate) fn method_hash(&self) -> TerminalMethodHash {
        self.method_hash
    }
}

/// Complete decoded logical record. The storage codec may move large fact
/// bodies into immutable content-addressed blobs, but replay must recover this
/// typed value before the reducer accepts the envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalRunRecordPayload {
    RunOpened {
        header: ExploreRunHeader,
        lease: FencedWriterLease,
    },
    Discovery {
        kind: DiscoveryEventKind,
        canonical_discovery_hash: CanonicalDigest,
        lease: FencedWriterLease,
    },
    SemanticObservation {
        producer_kind: ObservationEvidenceKind,
        semantic_facts: Box<[SemanticEvidenceFact]>,
        validation_receipt_hash: CanonicalDigest,
        lease: FencedWriterLease,
    },
    CoveragePlanAccepted {
        plan: CoveragePlan,
        lease: FencedWriterLease,
    },
    FrontierTransition {
        producer_kind: FrontierEvidenceKind,
        previous_frontier_commitment: CanonicalDigest,
        newly_closed: RequiredFrontier,
        next_frontier_commitment: CanonicalDigest,
        semantic_facts: Box<[SemanticEvidenceFact]>,
        validation_receipt_hash: CanonicalDigest,
        lease: FencedWriterLease,
    },
    Paused {
        reason: PauseReason,
        previous_journal_head: JournalHead,
        evidence_root: EvidenceRoot,
        lease: FencedWriterLease,
    },
    Resumed {
        previous_journal_head: JournalHead,
        evidence_root: EvidenceRoot,
        lease: FencedWriterLease,
    },
    Recovered {
        previous_journal_head: JournalHead,
        evidence_root: EvidenceRoot,
        lease: FencedWriterLease,
    },
    TerminalSeal {
        kind: TerminalSealKind,
        journal_head_before_seal: JournalHead,
        evidence_root: EvidenceRoot,
        terminal_payload_hash: TerminalPayloadHash,
        method_hash: TerminalMethodHash,
        lease: FencedWriterLease,
    },
}

impl CanonicalRunRecordPayload {
    pub(crate) fn event_kind(&self) -> RunEventKind {
        match self {
            Self::RunOpened { .. } => RunEventKind::Control(ControlEventKind::RunOpened),
            Self::Discovery { kind, .. } => RunEventKind::Discovery(*kind),
            Self::SemanticObservation { producer_kind, .. } => {
                RunEventKind::Evidence(EvidenceEventKind::ObservationAccepted(*producer_kind))
            }
            Self::CoveragePlanAccepted { .. } => {
                RunEventKind::Evidence(EvidenceEventKind::CoveragePlanAccepted)
            }
            Self::FrontierTransition { producer_kind, .. } => {
                RunEventKind::Evidence(EvidenceEventKind::FrontierAdvanced(*producer_kind))
            }
            Self::Paused { reason, .. } => RunEventKind::Control(ControlEventKind::Paused(*reason)),
            Self::Resumed { .. } => RunEventKind::Control(ControlEventKind::Resumed),
            Self::Recovered { .. } => RunEventKind::Control(ControlEventKind::Recovered),
            Self::TerminalSeal { kind, .. } => {
                RunEventKind::Control(ControlEventKind::TerminalSealed(*kind))
            }
        }
    }

    pub(crate) fn lease(&self) -> FencedWriterLease {
        match self {
            Self::RunOpened { lease, .. }
            | Self::Discovery { lease, .. }
            | Self::SemanticObservation { lease, .. }
            | Self::CoveragePlanAccepted { lease, .. }
            | Self::FrontierTransition { lease, .. }
            | Self::Paused { lease, .. }
            | Self::Resumed { lease, .. }
            | Self::Recovered { lease, .. }
            | Self::TerminalSeal { lease, .. } => *lease,
        }
    }

    pub(crate) fn canonical_payload_hash(&self) -> CanonicalDigest {
        let mut hasher = StableHasher::new(RUN_RECORD_PAYLOAD_HASH_V3);
        match self {
            Self::RunOpened { header, lease } => {
                hasher.u8(0);
                hasher.segment(&header.commitment.0);
                hasher.segment(&header.run_id.0);
                lease.hash_into(&mut hasher);
            }
            Self::Discovery {
                kind,
                canonical_discovery_hash,
                lease,
            } => {
                hasher.u8(1);
                hash_discovery_kind(*kind, &mut hasher);
                canonical_discovery_hash.hash_into(&mut hasher);
                lease.hash_into(&mut hasher);
            }
            Self::SemanticObservation {
                producer_kind,
                semantic_facts,
                validation_receipt_hash,
                lease,
            } => {
                hasher.u8(2);
                hash_observation_kind(*producer_kind, &mut hasher);
                hash_semantic_fact_batch(semantic_facts, &mut hasher);
                validation_receipt_hash.hash_into(&mut hasher);
                lease.hash_into(&mut hasher);
            }
            Self::CoveragePlanAccepted { plan, lease } => {
                hasher.u8(3);
                plan.hash_into(&mut hasher);
                lease.hash_into(&mut hasher);
            }
            Self::FrontierTransition {
                producer_kind,
                previous_frontier_commitment,
                newly_closed,
                next_frontier_commitment,
                semantic_facts,
                validation_receipt_hash,
                lease,
            } => {
                hasher.u8(4);
                hash_frontier_kind(*producer_kind, &mut hasher);
                previous_frontier_commitment.hash_into(&mut hasher);
                newly_closed.hash_into(&mut hasher);
                next_frontier_commitment.hash_into(&mut hasher);
                hash_semantic_fact_batch(semantic_facts, &mut hasher);
                validation_receipt_hash.hash_into(&mut hasher);
                lease.hash_into(&mut hasher);
            }
            Self::Paused {
                reason,
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                hasher.u8(5);
                hash_pause_reason(*reason, &mut hasher);
                hasher.segment(&previous_journal_head.0);
                hasher.segment(&evidence_root.0);
                lease.hash_into(&mut hasher);
            }
            Self::Resumed {
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                hasher.u8(6);
                hasher.segment(&previous_journal_head.0);
                hasher.segment(&evidence_root.0);
                lease.hash_into(&mut hasher);
            }
            Self::Recovered {
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                hasher.u8(7);
                hasher.segment(&previous_journal_head.0);
                hasher.segment(&evidence_root.0);
                lease.hash_into(&mut hasher);
            }
            Self::TerminalSeal {
                kind,
                journal_head_before_seal,
                evidence_root,
                terminal_payload_hash,
                method_hash,
                lease,
            } => {
                hasher.u8(8);
                hash_terminal_kind(*kind, &mut hasher);
                hasher.segment(&journal_head_before_seal.0);
                hasher.segment(&evidence_root.0);
                hasher.segment(&terminal_payload_hash.0);
                hasher.segment(&method_hash.0);
                lease.hash_into(&mut hasher);
            }
        }
        CanonicalDigest(hasher.finish())
    }
}

#[derive(Clone, Debug)]
struct PreparedRunState {
    lifecycle: RunLifecycle,
    sequence: u64,
    journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    frontier: RequiredFrontier,
    evidence: SemanticEvidenceMap,
    active_lease: Option<FencedWriterLease>,
    last_lease_generation: NonZeroU64,
    last_coverage_epoch: Option<NonZeroU64>,
    terminal_seal: Option<TerminalSeal>,
}

/// Non-mutating proposal which storage must durably install before application
/// or publication.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRunTransition {
    base_cursor: ExploreRunCursor,
    event: CommittedRunEvent,
    payload: CanonicalRunRecordPayload,
    next: PreparedRunState,
}

impl PreparedRunTransition {
    pub(crate) fn event(&self) -> &CommittedRunEvent {
        &self.event
    }

    pub(crate) fn payload(&self) -> &CanonicalRunRecordPayload {
        &self.payload
    }

    pub(crate) fn resulting_cursor(&self) -> ExploreRunCursor {
        cursor_from_state(self.event.run_id, &self.next)
    }
}

/// Prepared genesis. `install_committed` must be called only after storage has
/// durably created the header, fence and sequence-zero record.
#[derive(Clone, Debug)]
pub(crate) struct PreparedRunOpen {
    event: CommittedRunEvent,
    payload: CanonicalRunRecordPayload,
    state: ExploreRunStream,
}

impl PreparedRunOpen {
    pub(crate) fn event(&self) -> &CommittedRunEvent {
        &self.event
    }

    pub(crate) fn payload(&self) -> &CanonicalRunRecordPayload {
        &self.payload
    }

    pub(crate) fn install_committed(self) -> ExploreRunStream {
        self.state
    }
}

/// Current committed run state. Prior events belong to durable storage; this
/// reducer retains only the authenticated semantic map and last envelope.
#[derive(Clone, Debug)]
pub(crate) struct ExploreRunStream {
    header: ExploreRunHeader,
    lifecycle: RunLifecycle,
    sequence: u64,
    journal_head: JournalHead,
    evidence_root: EvidenceRoot,
    frontier: RequiredFrontier,
    evidence: SemanticEvidenceMap,
    active_lease: Option<FencedWriterLease>,
    last_lease_generation: NonZeroU64,
    last_coverage_epoch: Option<NonZeroU64>,
    last_committed_event: CommittedRunEvent,
    terminal_seal: Option<TerminalSeal>,
}

impl ExploreRunStream {
    pub(crate) fn prepare_open(
        header: ExploreRunHeader,
        initial_lease: FencedWriterLease,
    ) -> Result<PreparedRunOpen, ExploreRunStreamError> {
        if initial_lease.run_id != header.run_id {
            return Err(ExploreRunStreamError::StaleRunId);
        }
        if initial_lease.generation.get() != 1 {
            return Err(ExploreRunStreamError::InvalidInitialLeaseGeneration);
        }
        let frontier = header.initial_frontier();
        let evidence = SemanticEvidenceMap::empty();
        let evidence_root = derive_evidence_root(header.answer_scope_id, &evidence, &frontier);
        let anchor = derive_journal_anchor(header.run_id);
        let payload = CanonicalRunRecordPayload::RunOpened {
            header: header.clone(),
            lease: initial_lease,
        };
        let kind = payload.event_kind();
        let canonical_payload_hash = payload.canonical_payload_hash();
        let journal_head = derive_journal_head(
            header.run_id,
            0,
            anchor,
            evidence_root,
            kind,
            canonical_payload_hash,
            initial_lease,
        );
        let event = CommittedRunEvent {
            run_id: header.run_id,
            sequence: 0,
            previous_journal_head: anchor,
            journal_head,
            evidence_root,
            kind,
            canonical_payload_hash,
            lease_generation: initial_lease.generation,
            lease_id: initial_lease.id,
        };
        let state = Self {
            header,
            lifecycle: RunLifecycle::Running,
            sequence: 0,
            journal_head,
            evidence_root,
            frontier,
            evidence,
            active_lease: Some(initial_lease),
            last_lease_generation: initial_lease.generation,
            last_coverage_epoch: None,
            last_committed_event: event.clone(),
            terminal_seal: None,
        };
        Ok(PreparedRunOpen {
            event,
            payload,
            state,
        })
    }

    pub(crate) fn replay_open(
        payload: CanonicalRunRecordPayload,
        expected: &CommittedRunEvent,
    ) -> Result<Self, ExploreRunStreamError> {
        let CanonicalRunRecordPayload::RunOpened { header, lease } = payload.clone() else {
            return Err(ExploreRunStreamError::ExpectedRunOpenedPayload);
        };
        let prepared = Self::prepare_open(header, lease)?;
        if prepared.payload != payload || prepared.event != *expected {
            return Err(ExploreRunStreamError::CommittedEnvelopeMismatch);
        }
        Ok(prepared.install_committed())
    }

    pub(crate) fn header(&self) -> &ExploreRunHeader {
        &self.header
    }

    pub(crate) fn lifecycle(&self) -> RunLifecycle {
        self.lifecycle
    }

    pub(crate) fn frontier(&self) -> &RequiredFrontier {
        &self.frontier
    }

    pub(crate) fn journal_head(&self) -> JournalHead {
        self.journal_head
    }

    pub(crate) fn evidence_root(&self) -> EvidenceRoot {
        self.evidence_root
    }

    /// Canonical key-order materialization for snapshots/final rendering. The
    /// hot commit path never calls this O(M) operation.
    pub(crate) fn semantic_evidence_facts(&self) -> Vec<SemanticEvidenceFact> {
        self.evidence.canonical_facts()
    }

    pub(crate) fn last_committed_event(&self) -> &CommittedRunEvent {
        &self.last_committed_event
    }

    pub(crate) fn terminal_seal(&self) -> Option<&TerminalSeal> {
        self.terminal_seal.as_ref()
    }

    pub(crate) fn cursor(&self) -> ExploreRunCursor {
        ExploreRunCursor {
            run_id: self.header.run_id,
            sequence: self.sequence,
            journal_head: self.journal_head,
            evidence_root: self.evidence_root,
            lifecycle: self.lifecycle,
            active_lease_id: self.active_lease.map(|lease| lease.id),
            last_lease_generation: self.last_lease_generation,
            last_coverage_epoch: self.last_coverage_epoch,
        }
    }

    /// Prepare scheduling provenance. Discovery never closes support or changes
    /// the evidence root.
    pub(crate) fn prepare_discovery(
        &self,
        lease: FencedWriterLease,
        kind: DiscoveryEventKind,
        canonical_discovery_hash: CanonicalDigest,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        let payload = CanonicalRunRecordPayload::Discovery {
            kind,
            canonical_discovery_hash,
            lease,
        };
        self.finish_prepared(payload, self.prepared_state())
    }

    pub(crate) fn prepare_observation(
        &self,
        lease: FencedWriterLease,
        producer_kind: ObservationEvidenceKind,
        semantic_facts: impl IntoIterator<Item = SemanticEvidenceFact>,
        validation_receipt_hash: CanonicalDigest,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        let semantic_facts = normalize_semantic_facts(semantic_facts)?;
        if semantic_facts.is_empty() {
            return Err(ExploreRunStreamError::EmptySemanticEvidence);
        }
        if semantic_facts
            .iter()
            .any(|fact| fact.layer().closes_any_frontier())
        {
            return Err(ExploreRunStreamError::ObservationCannotCloseFrontier);
        }
        validate_fact_universes(self.header.case_universe(), &semantic_facts)?;
        let mut next = self.prepared_state();
        next.evidence = self.evidence.with_facts(&semantic_facts)?;
        next.evidence_root =
            derive_evidence_root(self.header.answer_scope_id, &next.evidence, &next.frontier);
        let payload = CanonicalRunRecordPayload::SemanticObservation {
            producer_kind,
            semantic_facts,
            validation_receipt_hash,
            lease,
        };
        self.finish_prepared(payload, next)
    }

    pub(crate) fn prepare_coverage_plan(
        &self,
        lease: FencedWriterLease,
        plan: CoveragePlan,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        if plan.run_id != self.header.run_id {
            return Err(ExploreRunStreamError::StaleRunId);
        }
        if self
            .last_coverage_epoch
            .is_some_and(|epoch| plan.sharding_epoch <= epoch)
        {
            return Err(ExploreRunStreamError::StaleCoveragePlanEpoch);
        }
        if !self
            .frontier
            .open_cases
            .exact_disjoint_union(&plan.certified_closed, &plan.residual_open)?
        {
            return Err(ExploreRunStreamError::FrontierNotConserved);
        }
        validate_fact_universes(self.header.case_universe(), &plan.semantic_facts)?;
        validate_frontier_facts(
            &RequiredFrontier::from_normalized(plan.certified_closed.clone(), BTreeSet::new()),
            &plan.semantic_facts,
        )?;

        let mut next = self.prepared_state();
        next.frontier = RequiredFrontier::from_normalized(
            plan.residual_open.clone(),
            self.frontier.open_obligations.clone(),
        );
        next.evidence = self.evidence.with_facts(&plan.semantic_facts)?;
        next.evidence_root =
            derive_evidence_root(self.header.answer_scope_id, &next.evidence, &next.frontier);
        next.last_coverage_epoch = Some(plan.sharding_epoch);
        let payload = CanonicalRunRecordPayload::CoveragePlanAccepted { plan, lease };
        self.finish_prepared(payload, next)
    }

    /// Derive and commit `next = previous - newly_closed` without changing the
    /// committed cursor. The record carries only both commitments and the
    /// bounded closure delta; replay reconstructs the same next frontier.
    pub(crate) fn prepare_frontier_transition(
        &self,
        lease: FencedWriterLease,
        producer_kind: FrontierEvidenceKind,
        newly_closed: RequiredFrontier,
        semantic_facts: impl IntoIterator<Item = SemanticEvidenceFact>,
        validation_receipt_hash: CanonicalDigest,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        if newly_closed.open_cases.is_empty() && newly_closed.open_obligations.is_empty() {
            return Err(ExploreRunStreamError::EmptySemanticClosure);
        }
        let previous_frontier_commitment = self.frontier.identity_hash();
        let next_open = self.frontier.close_exact(&newly_closed)?;
        let next_frontier_commitment = next_open.identity_hash();
        let semantic_facts = normalize_semantic_facts(semantic_facts)?;
        validate_fact_universes(self.header.case_universe(), &semantic_facts)?;
        validate_frontier_facts(&newly_closed, &semantic_facts)?;
        let mut next = self.prepared_state();
        next.frontier = next_open;
        next.evidence = self.evidence.with_facts(&semantic_facts)?;
        next.evidence_root =
            derive_evidence_root(self.header.answer_scope_id, &next.evidence, &next.frontier);
        let payload = CanonicalRunRecordPayload::FrontierTransition {
            producer_kind,
            previous_frontier_commitment,
            newly_closed,
            next_frontier_commitment,
            semantic_facts,
            validation_receipt_hash,
            lease,
        };
        self.finish_prepared(payload, next)
    }

    pub(crate) fn prepare_pause(
        &self,
        lease: FencedWriterLease,
        reason: PauseReason,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        let payload = CanonicalRunRecordPayload::Paused {
            reason,
            previous_journal_head: self.journal_head,
            evidence_root: self.evidence_root,
            lease,
        };
        let mut next = self.prepared_state();
        next.lifecycle = RunLifecycle::Paused;
        next.active_lease = None;
        self.finish_prepared(payload, next)
    }

    pub(crate) fn prepare_resume(
        &self,
        paused_cursor: ExploreRunCursor,
        next_lease: FencedWriterLease,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_lifecycle(RunLifecycle::Paused)?;
        self.require_cursor(paused_cursor)?;
        self.validate_next_lease(next_lease)?;
        let payload = CanonicalRunRecordPayload::Resumed {
            previous_journal_head: self.journal_head,
            evidence_root: self.evidence_root,
            lease: next_lease,
        };
        let mut next = self.prepared_state();
        next.lifecycle = RunLifecycle::Running;
        next.active_lease = Some(next_lease);
        next.last_lease_generation = next_lease.generation;
        self.finish_prepared(payload, next)
    }

    pub(crate) fn prepare_recovery(
        &self,
        running_cursor: ExploreRunCursor,
        recovery_lease: FencedWriterLease,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_lifecycle(RunLifecycle::Running)?;
        self.require_cursor(running_cursor)?;
        self.validate_next_lease(recovery_lease)?;
        let payload = CanonicalRunRecordPayload::Recovered {
            previous_journal_head: self.journal_head,
            evidence_root: self.evidence_root,
            lease: recovery_lease,
        };
        let mut next = self.prepared_state();
        next.active_lease = Some(recovery_lease);
        next.last_lease_generation = recovery_lease.generation;
        self.finish_prepared(payload, next)
    }

    pub(crate) fn prepare_seal(
        &self,
        lease: FencedWriterLease,
        kind: TerminalSealKind,
        terminal_payload_hash: TerminalPayloadHash,
        method_hash: TerminalMethodHash,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        self.require_active_lease(lease)?;
        if kind == TerminalSealKind::Completed && !self.frontier.is_closed() {
            return Err(ExploreRunStreamError::CompletedWithOpenFrontier);
        }
        let payload = CanonicalRunRecordPayload::TerminalSeal {
            kind,
            journal_head_before_seal: self.journal_head,
            evidence_root: self.evidence_root,
            terminal_payload_hash,
            method_hash,
            lease,
        };
        let mut next = self.prepared_state();
        next.lifecycle = RunLifecycle::Sealed;
        next.active_lease = None;
        self.finish_prepared(payload, next)
    }

    /// Install one storage-authorized transition. Failure leaves `self`
    /// unchanged; callers publish only after this succeeds.
    pub(crate) fn apply_committed(
        &mut self,
        prepared: PreparedRunTransition,
    ) -> Result<(), ExploreRunStreamError> {
        if prepared.base_cursor != self.cursor() {
            return Err(ExploreRunStreamError::StalePreparedTransition);
        }
        validate_prepared_envelope(&prepared)?;
        let PreparedRunTransition { event, next, .. } = prepared;
        self.lifecycle = next.lifecycle;
        self.sequence = next.sequence;
        self.journal_head = next.journal_head;
        self.evidence_root = next.evidence_root;
        self.frontier = next.frontier;
        self.evidence = next.evidence;
        self.active_lease = next.active_lease;
        self.last_lease_generation = next.last_lease_generation;
        self.last_coverage_epoch = next.last_coverage_epoch;
        self.terminal_seal = next.terminal_seal;
        self.last_committed_event = event;
        Ok(())
    }

    /// Pure replay reducer for one decoded contiguous record.
    pub(crate) fn replay_committed(
        &mut self,
        payload: CanonicalRunRecordPayload,
        expected: &CommittedRunEvent,
    ) -> Result<(), ExploreRunStreamError> {
        let prepared = self.prepare_decoded_payload(payload.clone())?;
        if prepared.payload != payload || prepared.event != *expected {
            return Err(ExploreRunStreamError::CommittedEnvelopeMismatch);
        }
        self.apply_committed(prepared)
    }

    fn prepare_decoded_payload(
        &self,
        payload: CanonicalRunRecordPayload,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        match payload {
            CanonicalRunRecordPayload::RunOpened { .. } => {
                Err(ExploreRunStreamError::UnexpectedRunOpenedPayload)
            }
            CanonicalRunRecordPayload::Discovery {
                kind,
                canonical_discovery_hash,
                lease,
            } => self.prepare_discovery(lease, kind, canonical_discovery_hash),
            CanonicalRunRecordPayload::SemanticObservation {
                producer_kind,
                semantic_facts,
                validation_receipt_hash,
                lease,
            } => self.prepare_observation(
                lease,
                producer_kind,
                Vec::from(semantic_facts),
                validation_receipt_hash,
            ),
            CanonicalRunRecordPayload::CoveragePlanAccepted { plan, lease } => {
                self.prepare_coverage_plan(lease, plan)
            }
            CanonicalRunRecordPayload::FrontierTransition {
                producer_kind,
                previous_frontier_commitment,
                newly_closed,
                next_frontier_commitment,
                semantic_facts,
                validation_receipt_hash,
                lease,
            } => {
                if previous_frontier_commitment != self.frontier.identity_hash() {
                    return Err(ExploreRunStreamError::StaleRunCursor);
                }
                let prepared = self.prepare_frontier_transition(
                    lease,
                    producer_kind,
                    newly_closed,
                    Vec::from(semantic_facts),
                    validation_receipt_hash,
                )?;
                let CanonicalRunRecordPayload::FrontierTransition {
                    next_frontier_commitment: derived_next,
                    ..
                } = prepared.payload()
                else {
                    unreachable!("frontier preparation returns a frontier payload")
                };
                if *derived_next != next_frontier_commitment {
                    return Err(ExploreRunStreamError::FrontierNotConserved);
                }
                Ok(prepared)
            }
            CanonicalRunRecordPayload::Paused {
                reason,
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                self.require_replay_base(previous_journal_head, evidence_root)?;
                self.prepare_pause(lease, reason)
            }
            CanonicalRunRecordPayload::Resumed {
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                self.require_replay_base(previous_journal_head, evidence_root)?;
                self.prepare_resume(self.cursor(), lease)
            }
            CanonicalRunRecordPayload::Recovered {
                previous_journal_head,
                evidence_root,
                lease,
            } => {
                self.require_replay_base(previous_journal_head, evidence_root)?;
                self.prepare_recovery(self.cursor(), lease)
            }
            CanonicalRunRecordPayload::TerminalSeal {
                kind,
                journal_head_before_seal,
                evidence_root,
                terminal_payload_hash,
                method_hash,
                lease,
            } => {
                self.require_replay_base(journal_head_before_seal, evidence_root)?;
                self.prepare_seal(lease, kind, terminal_payload_hash, method_hash)
            }
        }
    }

    fn prepared_state(&self) -> PreparedRunState {
        PreparedRunState {
            lifecycle: self.lifecycle,
            sequence: self.sequence,
            journal_head: self.journal_head,
            evidence_root: self.evidence_root,
            frontier: self.frontier.clone(),
            evidence: self.evidence.clone(),
            active_lease: self.active_lease,
            last_lease_generation: self.last_lease_generation,
            last_coverage_epoch: self.last_coverage_epoch,
            terminal_seal: self.terminal_seal.clone(),
        }
    }

    fn finish_prepared(
        &self,
        payload: CanonicalRunRecordPayload,
        mut next: PreparedRunState,
    ) -> Result<PreparedRunTransition, ExploreRunStreamError> {
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(ExploreRunStreamError::SequenceExhausted)?;
        let lease = payload.lease();
        let kind = payload.event_kind();
        let canonical_payload_hash = payload.canonical_payload_hash();
        let journal_head = derive_journal_head(
            self.header.run_id,
            sequence,
            self.journal_head,
            next.evidence_root,
            kind,
            canonical_payload_hash,
            lease,
        );
        let event = CommittedRunEvent {
            run_id: self.header.run_id,
            sequence,
            previous_journal_head: self.journal_head,
            journal_head,
            evidence_root: next.evidence_root,
            kind,
            canonical_payload_hash,
            lease_generation: lease.generation,
            lease_id: lease.id,
        };
        next.sequence = sequence;
        next.journal_head = journal_head;
        if let CanonicalRunRecordPayload::TerminalSeal {
            kind,
            journal_head_before_seal,
            evidence_root,
            terminal_payload_hash,
            method_hash,
            ..
        } = &payload
        {
            next.terminal_seal = Some(TerminalSeal {
                kind: *kind,
                journal_head_before_seal: *journal_head_before_seal,
                terminal_journal_head: journal_head,
                evidence_root: *evidence_root,
                terminal_payload_hash: *terminal_payload_hash,
                method_hash: *method_hash,
            });
        }
        Ok(PreparedRunTransition {
            base_cursor: self.cursor(),
            event,
            payload,
            next,
        })
    }

    fn require_active_lease(&self, lease: FencedWriterLease) -> Result<(), ExploreRunStreamError> {
        self.require_lifecycle(RunLifecycle::Running)?;
        if lease.run_id != self.header.run_id {
            return Err(ExploreRunStreamError::StaleRunId);
        }
        if self.active_lease != Some(lease) {
            return Err(ExploreRunStreamError::StaleWriterLease);
        }
        Ok(())
    }

    fn require_lifecycle(&self, expected: RunLifecycle) -> Result<(), ExploreRunStreamError> {
        if self.lifecycle != expected {
            return Err(ExploreRunStreamError::WrongLifecycle {
                expected,
                actual: self.lifecycle,
            });
        }
        Ok(())
    }

    fn require_cursor(&self, cursor: ExploreRunCursor) -> Result<(), ExploreRunStreamError> {
        if cursor != self.cursor() {
            return Err(ExploreRunStreamError::StaleRunCursor);
        }
        Ok(())
    }

    fn require_replay_base(
        &self,
        journal_head: JournalHead,
        evidence_root: EvidenceRoot,
    ) -> Result<(), ExploreRunStreamError> {
        if journal_head != self.journal_head || evidence_root != self.evidence_root {
            return Err(ExploreRunStreamError::StaleRunCursor);
        }
        Ok(())
    }

    fn validate_next_lease(&self, lease: FencedWriterLease) -> Result<(), ExploreRunStreamError> {
        if lease.run_id != self.header.run_id {
            return Err(ExploreRunStreamError::StaleRunId);
        }
        if lease.generation <= self.last_lease_generation {
            return Err(ExploreRunStreamError::StaleWriterLeaseGeneration);
        }
        Ok(())
    }
}

fn cursor_from_state(run_id: ExploreRunId, state: &PreparedRunState) -> ExploreRunCursor {
    ExploreRunCursor {
        run_id,
        sequence: state.sequence,
        journal_head: state.journal_head,
        evidence_root: state.evidence_root,
        lifecycle: state.lifecycle,
        active_lease_id: state.active_lease.map(|lease| lease.id),
        last_lease_generation: state.last_lease_generation,
        last_coverage_epoch: state.last_coverage_epoch,
    }
}

fn validate_prepared_envelope(
    prepared: &PreparedRunTransition,
) -> Result<(), ExploreRunStreamError> {
    let lease = prepared.payload.lease();
    let event = &prepared.event;
    if event.run_id != prepared.base_cursor.run_id
        || event.previous_journal_head != prepared.base_cursor.journal_head
        || event.sequence
            != prepared
                .base_cursor
                .sequence
                .checked_add(1)
                .ok_or(ExploreRunStreamError::SequenceExhausted)?
        || event.kind != prepared.payload.event_kind()
        || event.canonical_payload_hash != prepared.payload.canonical_payload_hash()
        || event.evidence_root != prepared.next.evidence_root
        || event.sequence != prepared.next.sequence
        || event.journal_head != prepared.next.journal_head
        || event.lease_generation != lease.generation
        || event.lease_id != lease.id
    {
        return Err(ExploreRunStreamError::PreparedTransitionCorrupt);
    }
    let recomputed = derive_journal_head(
        event.run_id,
        event.sequence,
        event.previous_journal_head,
        event.evidence_root,
        event.kind,
        event.canonical_payload_hash,
        lease,
    );
    if recomputed != event.journal_head {
        return Err(ExploreRunStreamError::PreparedTransitionCorrupt);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExploreRunStreamError {
    InvalidDigest {
        field: &'static str,
    },
    ZeroRunNonce,
    CaseUniverseOverflow,
    CaseSupportOverflow,
    SemanticEvidenceCountOverflow,
    InvalidRankInterval {
        start: u128,
        end_exclusive: u128,
    },
    RankIntervalOutsideUniverse {
        start: u128,
        end_exclusive: u128,
        universe_case_count: u128,
    },
    OverlappingRankIntervals {
        left_start: u128,
        left_end_exclusive: u128,
        right_start: u128,
        right_end_exclusive: u128,
    },
    DuplicateRequiredObligation,
    OverlappingSemanticEvidence,
    DuplicateGlobalSemanticEvidence,
    ConflictingSemanticEvidenceSubject,
    ConflictingSemanticEvidenceKey,
    ContradictorySemanticEvidence,
    SemanticLayerSubjectMismatch,
    EmptySemanticEvidence,
    ObservationCannotCloseFrontier,
    FrontierFactsMismatch,
    StaleCaseUniverse,
    StaleRunId,
    InvalidInitialLeaseGeneration,
    RecordedLeaseIdMismatch,
    StaleWriterLease,
    StaleWriterLeaseGeneration,
    StaleRunCursor,
    StalePreparedTransition,
    StaleCoveragePlanEpoch,
    WrongLifecycle {
        expected: RunLifecycle,
        actual: RunLifecycle,
    },
    FrontierNotConserved,
    EmptySemanticClosure,
    DuplicateSemanticEvidence,
    ExpectedRunOpenedPayload,
    UnexpectedRunOpenedPayload,
    CommittedEnvelopeMismatch,
    PreparedTransitionCorrupt,
    CompletedWithOpenFrontier,
    SequenceExhausted,
}

impl fmt::Display for ExploreRunStreamError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDigest { field } => write!(
                out,
                "Explore run-stream {field} must be one lowercase SHA-256 digest"
            ),
            Self::ZeroRunNonce => write!(out, "Explore run nonce must not be the zero digest"),
            Self::CaseUniverseOverflow => {
                write!(out, "declared Explore case universe exceeds u128::MAX")
            }
            Self::CaseSupportOverflow => {
                write!(out, "Explore exact case support exceeds u128::MAX")
            }
            Self::SemanticEvidenceCountOverflow => {
                write!(out, "Explore semantic evidence count exceeds u128::MAX")
            }
            Self::InvalidRankInterval {
                start,
                end_exclusive,
            } => write!(
                out,
                "Explore rank interval [{start}, {end_exclusive}) is empty or reversed"
            ),
            Self::RankIntervalOutsideUniverse {
                start,
                end_exclusive,
                universe_case_count,
            } => write!(
                out,
                "Explore rank interval [{start}, {end_exclusive}) exceeds U={universe_case_count}"
            ),
            Self::OverlappingRankIntervals {
                left_start,
                left_end_exclusive,
                right_start,
                right_end_exclusive,
            } => write!(
                out,
                "Explore rank intervals [{left_start}, {left_end_exclusive}) and \
                 [{right_start}, {right_end_exclusive}) overlap"
            ),
            Self::DuplicateRequiredObligation => {
                write!(
                    out,
                    "Explore required frontier contains a duplicate obligation"
                )
            }
            Self::OverlappingSemanticEvidence => write!(
                out,
                "Explore semantic evidence repeats or overlaps already accepted support"
            ),
            Self::DuplicateGlobalSemanticEvidence => {
                write!(out, "Explore global semantic evidence was already accepted")
            }
            Self::ConflictingSemanticEvidenceSubject => write!(
                out,
                "Explore semantic evidence uses incompatible subjects for one identity"
            ),
            Self::ConflictingSemanticEvidenceKey => {
                write!(out, "Explore semantic evidence identities do not match")
            }
            Self::ContradictorySemanticEvidence => write!(
                out,
                "Explore semantic facts make conflicting claims about one exclusive subject"
            ),
            Self::SemanticLayerSubjectMismatch => write!(
                out,
                "Explore semantic evidence layer cannot use the supplied subject"
            ),
            Self::EmptySemanticEvidence => {
                write!(out, "Explore semantic evidence batch must not be empty")
            }
            Self::ObservationCannotCloseFrontier => write!(
                out,
                "Explore frontier-closing evidence requires an exact frontier transition"
            ),
            Self::FrontierFactsMismatch => write!(
                out,
                "Explore semantic facts do not exactly cover the newly closed frontier"
            ),
            Self::StaleCaseUniverse => write!(out, "Explore support belongs to another universe"),
            Self::StaleRunId => write!(out, "Explore record belongs to another run"),
            Self::InvalidInitialLeaseGeneration => {
                write!(out, "Explore genesis requires writer lease generation 1")
            }
            Self::RecordedLeaseIdMismatch => {
                write!(
                    out,
                    "Explore recorded writer lease identity does not match its fields"
                )
            }
            Self::StaleWriterLease => write!(out, "Explore writer lease is not active"),
            Self::StaleWriterLeaseGeneration => {
                write!(out, "Explore writer lease generation did not advance")
            }
            Self::StaleRunCursor => write!(out, "Explore continuation cursor is stale"),
            Self::StalePreparedTransition => {
                write!(
                    out,
                    "Explore prepared transition no longer matches the committed cursor"
                )
            }
            Self::StaleCoveragePlanEpoch => {
                write!(out, "Explore coverage-plan epoch did not advance")
            }
            Self::WrongLifecycle { expected, actual } => write!(
                out,
                "Explore run lifecycle is {actual:?}, expected {expected:?}"
            ),
            Self::FrontierNotConserved => write!(
                out,
                "Explore frontier transition is not an exact disjoint partition"
            ),
            Self::EmptySemanticClosure => {
                write!(out, "Explore semantic frontier closure is empty")
            }
            Self::DuplicateSemanticEvidence => {
                write!(out, "Explore semantic evidence was already accepted")
            }
            Self::ExpectedRunOpenedPayload => {
                write!(out, "Explore replay genesis requires a RunOpened payload")
            }
            Self::UnexpectedRunOpenedPayload => {
                write!(out, "Explore RunOpened may appear only at sequence zero")
            }
            Self::CommittedEnvelopeMismatch => write!(
                out,
                "Explore decoded record does not reproduce its committed envelope"
            ),
            Self::PreparedTransitionCorrupt => {
                write!(
                    out,
                    "Explore prepared transition failed its internal commitment check"
                )
            }
            Self::CompletedWithOpenFrontier => write!(
                out,
                "Explore Completed seal requires an empty required frontier"
            ),
            Self::SequenceExhausted => write!(out, "Explore journal sequence exhausted u64"),
        }
    }
}

impl Error for ExploreRunStreamError {}

fn derive_journal_anchor(run_id: ExploreRunId) -> JournalHead {
    let mut hasher = StableHasher::new(JOURNAL_ANCHOR_HASH_V1);
    hasher.segment(&run_id.0);
    JournalHead(hasher.finish())
}

fn derive_journal_head(
    run_id: ExploreRunId,
    sequence: u64,
    previous: JournalHead,
    evidence_root: EvidenceRoot,
    kind: RunEventKind,
    canonical_payload_hash: CanonicalDigest,
    lease: FencedWriterLease,
) -> JournalHead {
    let mut hasher = StableHasher::new(JOURNAL_EVENT_HASH_V1);
    hasher.segment(&run_id.0);
    hasher.u64(sequence);
    hasher.segment(&previous.0);
    hasher.segment(&evidence_root.0);
    hash_event_kind(kind, &mut hasher);
    canonical_payload_hash.hash_into(&mut hasher);
    hasher.u64(lease.generation.get());
    hasher.segment(&lease.id.0);
    JournalHead(hasher.finish())
}

fn derive_evidence_root(
    answer_scope_id: AnswerScopeId,
    evidence: &SemanticEvidenceMap,
    frontier: &RequiredFrontier,
) -> EvidenceRoot {
    let mut hasher = StableHasher::new(EVIDENCE_ROOT_HASH_V2);
    hasher.segment(&answer_scope_id.0);
    hasher.u128(evidence.len);
    hasher.segment(&evidence.root_hash());
    hasher.segment(&frontier.id.0);
    EvidenceRoot(hasher.finish())
}

fn hash_semantic_fact_batch(facts: &[SemanticEvidenceFact], hasher: &mut StableHasher) {
    hasher.u128(facts.len() as u128);
    for fact in facts {
        fact.hash_into(hasher);
    }
}

fn validate_fact_universes(
    universe: &ExploreCaseUniverse,
    facts: &[SemanticEvidenceFact],
) -> Result<(), ExploreRunStreamError> {
    for fact in facts {
        if let SemanticEvidenceSubject::Cases(support) = fact.subject() {
            if support.universe_id != universe.id {
                return Err(ExploreRunStreamError::StaleCaseUniverse);
            }
        }
    }
    Ok(())
}

fn validate_frontier_facts(
    newly_closed: &RequiredFrontier,
    facts: &[SemanticEvidenceFact],
) -> Result<(), ExploreRunStreamError> {
    let mut closed_cases: Option<ExactCaseSupport> = None;
    let mut closed_obligations = BTreeSet::<RequiredObligationId>::new();
    for fact in facts {
        if fact.layer().closes_case_frontier() {
            let SemanticEvidenceSubject::Cases(support) = fact.subject() else {
                return Err(ExploreRunStreamError::SemanticLayerSubjectMismatch);
            };
            closed_cases = Some(match closed_cases {
                Some(current) => current.merge_disjoint(support)?,
                None => support.clone(),
            });
        }
        if fact.layer().closes_obligation_frontier() {
            let SemanticEvidenceSubject::Obligations(obligations) = fact.subject() else {
                return Err(ExploreRunStreamError::SemanticLayerSubjectMismatch);
            };
            for obligation in obligations {
                if !closed_obligations.insert(*obligation) {
                    return Err(ExploreRunStreamError::ContradictorySemanticEvidence);
                }
            }
        }
    }
    let cases_match = match closed_cases {
        Some(support) => support == newly_closed.open_cases,
        None => newly_closed.open_cases.is_empty(),
    };
    if !cases_match || closed_obligations != newly_closed.open_obligations {
        return Err(ExploreRunStreamError::FrontierFactsMismatch);
    }
    Ok(())
}

fn hash_event_kind(kind: RunEventKind, hasher: &mut StableHasher) {
    match kind {
        RunEventKind::Control(kind) => {
            hasher.u8(0);
            hash_control_kind(kind, hasher);
        }
        RunEventKind::Discovery(kind) => {
            hasher.u8(1);
            hash_discovery_kind(kind, hasher);
        }
        RunEventKind::Evidence(kind) => {
            hasher.u8(2);
            hash_evidence_kind(kind, hasher);
        }
    }
}

fn hash_control_kind(kind: ControlEventKind, hasher: &mut StableHasher) {
    match kind {
        ControlEventKind::RunOpened => hasher.u8(0),
        ControlEventKind::Paused(reason) => {
            hasher.u8(1);
            hash_pause_reason(reason, hasher);
        }
        ControlEventKind::Resumed => hasher.u8(2),
        ControlEventKind::Recovered => hasher.u8(3),
        ControlEventKind::TerminalSealed(kind) => {
            hasher.u8(4);
            hash_terminal_kind(kind, hasher);
        }
    }
}

fn hash_discovery_kind(kind: DiscoveryEventKind, hasher: &mut StableHasher) {
    hasher.u8(match kind {
        DiscoveryEventKind::ProbeDecision => 0,
        DiscoveryEventKind::CandidateDiscovered => 1,
        DiscoveryEventKind::LiftScheduled => 2,
        DiscoveryEventKind::ProbePlanCompleted => 3,
        DiscoveryEventKind::SchedulingHint => 4,
        DiscoveryEventKind::SnapshotPublished => 5,
        DiscoveryEventKind::TerminalResultPublished => 6,
        DiscoveryEventKind::ProbePlanPrepared => 7,
    });
}

fn hash_pause_reason(reason: PauseReason, hasher: &mut StableHasher) {
    hasher.u8(match reason {
        PauseReason::Explicit => 0,
        PauseReason::TimeLimit => 1,
        PauseReason::Interrupt => 2,
        PauseReason::ResourcePressure => 3,
        PauseReason::StorageLimit => 4,
        PauseReason::ProbeMilestone => 5,
        PauseReason::EvaluationLimit => 6,
        PauseReason::FinalizationPending => 7,
    });
}

fn hash_frontier_kind(kind: FrontierEvidenceKind, hasher: &mut StableHasher) {
    hasher.u8(match kind {
        FrontierEvidenceKind::SingletonClassification => 0,
        FrontierEvidenceKind::CertifiedRegionClassification => 1,
        FrontierEvidenceKind::ExactExhaustion => 2,
        FrontierEvidenceKind::RepresentativeSelectionClosed => 3,
        FrontierEvidenceKind::MechanismTargetClosed => 4,
        FrontierEvidenceKind::BoundedExactBatchClassification => 5,
        FrontierEvidenceKind::ProbeCandidateBatchClassification => 6,
    });
}

fn hash_observation_kind(kind: ObservationEvidenceKind, hasher: &mut StableHasher) {
    hasher.u8(match kind {
        ObservationEvidenceKind::RepresentativeReplayed => 0,
        ObservationEvidenceKind::MechanismObserved => 1,
        ObservationEvidenceKind::ExtremaWitnessReplayed => 2,
    });
}

fn hash_evidence_kind(kind: EvidenceEventKind, hasher: &mut StableHasher) {
    match kind {
        EvidenceEventKind::CoveragePlanAccepted => hasher.u8(0),
        EvidenceEventKind::FrontierAdvanced(kind) => {
            hasher.u8(1);
            hash_frontier_kind(kind, hasher);
        }
        EvidenceEventKind::ObservationAccepted(kind) => {
            hasher.u8(2);
            hash_observation_kind(kind, hasher);
        }
    }
}

fn hash_terminal_kind(kind: TerminalSealKind, hasher: &mut StableHasher) {
    hasher.u8(match kind {
        TerminalSealKind::Completed => 0,
        TerminalSealKind::Partial => 1,
        TerminalSealKind::Unknown => 2,
        TerminalSealKind::Unsupported => 3,
        TerminalSealKind::Error => 4,
        TerminalSealKind::Cancelled => 5,
    });
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
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

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> CanonicalDigest {
        CanonicalDigest::from_lowercase_sha256("test", &byte.to_string().repeat(64)).unwrap()
    }

    fn identity(probe: char) -> ExploreRunIdentity {
        ExploreRunIdentity::new(
            digest('1'),
            digest('2'),
            digest('3'),
            digest('4'),
            digest('5'),
            digest(probe),
            digest('6'),
            digest('7'),
            digest('8'),
            ExploreRunSchemas::new(digest('9'), digest('a'), digest('b'), digest('c')),
        )
    }

    fn header(universe: u128, nonce: char, probe: char) -> ExploreRunHeader {
        ExploreRunHeader::new(
            identity(probe),
            ExploreCaseUniverse::new(vec![universe]).unwrap(),
            [],
            ExploreRunNonce::new(digest(nonce)).unwrap(),
        )
        .unwrap()
    }

    fn lease(header: &ExploreRunHeader, generation: u64, writer: char) -> FencedWriterLease {
        FencedWriterLease::new(
            header.run_id(),
            NonZeroU64::new(generation).unwrap(),
            ExploreWriterId::new(digest(writer)),
            digest(if generation % 2 == 0 { 'd' } else { 'e' }),
        )
    }

    fn open(header: ExploreRunHeader, lease: FencedWriterLease) -> ExploreRunStream {
        ExploreRunStream::prepare_open(header, lease)
            .unwrap()
            .install_committed()
    }

    fn apply(stream: &mut ExploreRunStream, prepared: PreparedRunTransition) {
        stream.apply_committed(prepared).unwrap();
    }

    fn case_fact(
        header: &ExploreRunHeader,
        interval: (u128, u128),
        content: char,
    ) -> SemanticEvidenceFact {
        SemanticEvidenceFact::new(
            SemanticEvidenceLayer::CaseClassification,
            digest(content),
            SemanticEvidenceSubject::cases(
                ExactCaseSupport::new(header.case_universe(), [interval]).unwrap(),
            ),
        )
        .unwrap()
    }

    fn global_fact(layer: SemanticEvidenceLayer, content: char) -> SemanticEvidenceFact {
        SemanticEvidenceFact::new(layer, digest(content), SemanticEvidenceSubject::global())
            .unwrap()
    }

    #[test]
    fn genesis_binds_full_universe_but_coverage_is_later() {
        let first = header(10, 'f', 'd');
        let changed_universe = header(11, 'f', 'd');
        let changed_attempt = header(10, 'e', 'd');
        assert_ne!(first.run_id(), changed_universe.run_id());
        assert_ne!(first.run_id(), changed_attempt.run_id());

        let run_id = first.run_id();
        let initial_lease = lease(&first, 1, 'a');
        let closed = ExactCaseSupport::new(first.case_universe(), [(0, 4)]).unwrap();
        let residual = ExactCaseSupport::new(first.case_universe(), [(4, 10)]).unwrap();
        let plan = CoveragePlan::new(
            &first,
            digest('1'),
            closed.clone(),
            residual,
            [case_fact(&first, (0, 4), '2')],
            digest('3'),
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(256).unwrap(),
        )
        .unwrap();
        let mut stream = open(first, initial_lease);
        let prepared = stream.prepare_coverage_plan(initial_lease, plan).unwrap();
        assert_eq!(stream.frontier().open_cases().case_count(), 10);
        apply(&mut stream, prepared);
        assert_eq!(stream.header().run_id(), run_id);
        assert_eq!(stream.header().case_universe().case_count(), 10);
        assert_eq!(stream.frontier().open_cases().case_count(), 6);
    }

    #[test]
    fn evidence_root_ignores_arrival_order_but_journal_head_does_not() {
        let first_header = header(10, 'f', 'd');
        let second_header = first_header.clone();
        let first_lease = lease(&first_header, 1, 'a');
        let second_lease = lease(&second_header, 1, 'a');
        let mut first = open(first_header, first_lease);
        let mut second = open(second_header, second_lease);

        let first_a = first
            .prepare_observation(
                first_lease,
                ObservationEvidenceKind::MechanismObserved,
                [global_fact(
                    SemanticEvidenceLayer::MechanismObservation,
                    '1',
                )],
                digest('3'),
            )
            .unwrap();
        apply(&mut first, first_a);
        let first_b = first
            .prepare_observation(
                first_lease,
                ObservationEvidenceKind::RepresentativeReplayed,
                [global_fact(SemanticEvidenceLayer::AnswerAggregation, '2')],
                digest('4'),
            )
            .unwrap();
        apply(&mut first, first_b);
        let second_b = second
            .prepare_observation(
                second_lease,
                ObservationEvidenceKind::RepresentativeReplayed,
                [global_fact(SemanticEvidenceLayer::AnswerAggregation, '2')],
                digest('4'),
            )
            .unwrap();
        apply(&mut second, second_b);
        let second_a = second
            .prepare_observation(
                second_lease,
                ObservationEvidenceKind::MechanismObserved,
                [global_fact(
                    SemanticEvidenceLayer::MechanismObservation,
                    '1',
                )],
                digest('3'),
            )
            .unwrap();
        apply(&mut second, second_a);

        assert_eq!(first.evidence_root(), second.evidence_root());
        assert_ne!(first.journal_head(), second.journal_head());
    }

    #[test]
    fn evidence_root_ignores_batching_and_producer_method() {
        let first_header = header(4, 'f', 'd');
        let second_header = first_header.clone();
        let first_lease = lease(&first_header, 1, 'a');
        let second_lease = lease(&second_header, 1, 'a');
        let mut one_batch = open(first_header.clone(), first_lease);
        let mut two_batches = open(second_header.clone(), second_lease);

        let wide = one_batch
            .prepare_frontier_transition(
                first_lease,
                FrontierEvidenceKind::CertifiedRegionClassification,
                RequiredFrontier::new(ExactCaseSupport::full(first_header.case_universe()), [])
                    .unwrap(),
                [case_fact(&first_header, (0, 4), '1')],
                digest('2'),
            )
            .unwrap();
        apply(&mut one_batch, wide);

        let left = two_batches
            .prepare_frontier_transition(
                second_lease,
                FrontierEvidenceKind::SingletonClassification,
                RequiredFrontier::new(
                    ExactCaseSupport::new(second_header.case_universe(), [(0, 2)]).unwrap(),
                    [],
                )
                .unwrap(),
                [case_fact(&second_header, (0, 2), '1')],
                digest('3'),
            )
            .unwrap();
        apply(&mut two_batches, left);
        let right = two_batches
            .prepare_frontier_transition(
                second_lease,
                FrontierEvidenceKind::ExactExhaustion,
                RequiredFrontier::new(
                    ExactCaseSupport::new(second_header.case_universe(), [(2, 4)]).unwrap(),
                    [],
                )
                .unwrap(),
                [case_fact(&second_header, (2, 4), '1')],
                digest('4'),
            )
            .unwrap();
        apply(&mut two_batches, right);

        assert_eq!(one_batch.evidence_root(), two_batches.evidence_root());
        assert_ne!(one_batch.journal_head(), two_batches.journal_head());
    }

    #[test]
    fn decoded_payload_replays_the_same_committed_envelope() {
        let header = header(10, 'f', 'd');
        let lease = lease(&header, 1, 'a');
        let mut original = open(header.clone(), lease);
        let prepared = original
            .prepare_discovery(lease, DiscoveryEventKind::ProbeDecision, digest('1'))
            .unwrap();
        let payload = prepared.payload().clone();
        let event = prepared.event().clone();
        apply(&mut original, prepared);

        let decoded = CommittedRunEvent::from_decoded_envelope(
            event.run_id(),
            event.sequence(),
            event.previous_journal_head(),
            event.journal_head(),
            event.evidence_root(),
            event.kind(),
            event.canonical_payload_hash(),
            event.lease_generation(),
            event.lease_id_hash(),
            &payload,
        )
        .unwrap();
        assert_eq!(decoded, event);

        let mut replay = open(header, lease);
        replay.replay_committed(payload, &decoded).unwrap();
        assert_eq!(replay.cursor(), original.cursor());
    }

    #[test]
    fn discovery_changes_only_the_ordered_journal() {
        let header = header(10, 'f', 'd');
        let lease = lease(&header, 1, 'a');
        let mut stream = open(header, lease);
        let root = stream.evidence_root();
        let frontier = stream.frontier().clone();
        let head = stream.journal_head();
        let prepared = stream
            .prepare_discovery(lease, DiscoveryEventKind::ProbeDecision, digest('1'))
            .unwrap();
        assert_eq!(stream.journal_head(), head);
        apply(&mut stream, prepared);
        assert_eq!(stream.evidence_root(), root);
        assert_eq!(stream.frontier(), &frontier);
        assert_ne!(stream.journal_head(), head);
    }

    #[test]
    fn coverage_plan_requires_exact_frontier_conservation() {
        let header = header(10, 'f', 'd');
        let lease = lease(&header, 1, 'a');
        let stream = open(header.clone(), lease);
        let plan = CoveragePlan::new(
            &header,
            digest('1'),
            ExactCaseSupport::new(header.case_universe(), [(0, 4)]).unwrap(),
            ExactCaseSupport::new(header.case_universe(), [(5, 10)]).unwrap(),
            [case_fact(&header, (0, 4), '2')],
            digest('3'),
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(256).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stream.prepare_coverage_plan(lease, plan),
            Err(ExploreRunStreamError::FrontierNotConserved)
        ));
        assert_eq!(stream.frontier().open_cases().case_count(), 10);

        let overlap = CoveragePlan::new(
            &header,
            digest('1'),
            ExactCaseSupport::new(header.case_universe(), [(0, 4)]).unwrap(),
            ExactCaseSupport::new(header.case_universe(), [(3, 10)]).unwrap(),
            [case_fact(&header, (0, 4), '2')],
            digest('3'),
            NonZeroU64::new(1).unwrap(),
            NonZeroU64::new(256).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            stream.prepare_coverage_plan(lease, overlap),
            Err(ExploreRunStreamError::FrontierNotConserved)
        ));
    }

    #[test]
    fn support_subtraction_preserves_fragmented_exact_frontier() {
        let header = header(20, 'f', 'd');
        let open =
            ExactCaseSupport::new(header.case_universe(), [(0, 5), (8, 15), (18, 20)]).unwrap();
        let closed = ExactCaseSupport::new(
            header.case_universe(),
            [(1, 3), (4, 5), (8, 10), (12, 14), (18, 20)],
        )
        .unwrap();
        let residual = open.subtract_exact(&closed).unwrap();
        assert_eq!(residual.case_count(), 5);
        assert_eq!(
            residual
                .intervals()
                .iter()
                .map(|interval| (interval.start(), interval.end_exclusive()))
                .collect::<Vec<_>>(),
            vec![(0, 1), (3, 4), (10, 12), (14, 15)]
        );
        assert_eq!(residual.first_rank(), Some(0));
        assert!(residual.contains_rank(10));
        assert!(!residual.contains_rank(12));

        let outside = ExactCaseSupport::new(header.case_universe(), [(5, 6)]).unwrap();
        assert!(matches!(
            open.subtract_exact(&outside),
            Err(ExploreRunStreamError::FrontierNotConserved)
        ));
    }

    #[test]
    fn pause_resume_and_recovery_fence_stale_writers() {
        let header = header(10, 'f', 'd');
        let first = lease(&header, 1, 'a');
        let second = lease(&header, 2, 'b');
        let third = lease(&header, 3, 'c');
        let mut stream = open(header, first);
        let pause = stream.prepare_pause(first, PauseReason::TimeLimit).unwrap();
        let paused = pause.resulting_cursor();
        apply(&mut stream, pause);
        assert_eq!(stream.lifecycle(), RunLifecycle::Paused);
        assert!(matches!(
            stream.prepare_discovery(first, DiscoveryEventKind::SchedulingHint, digest('1')),
            Err(ExploreRunStreamError::WrongLifecycle { .. })
        ));
        let resume = stream.prepare_resume(paused, second).unwrap();
        apply(&mut stream, resume);
        assert_eq!(stream.lifecycle(), RunLifecycle::Running);
        assert!(matches!(
            stream.prepare_discovery(first, DiscoveryEventKind::SchedulingHint, digest('1')),
            Err(ExploreRunStreamError::StaleWriterLease)
        ));

        let running = stream.cursor();
        let recovery = stream.prepare_recovery(running, third).unwrap();
        apply(&mut stream, recovery);
        assert!(matches!(
            stream.prepare_discovery(second, DiscoveryEventKind::SchedulingHint, digest('1')),
            Err(ExploreRunStreamError::StaleWriterLease)
        ));
        let discovery = stream
            .prepare_discovery(third, DiscoveryEventKind::SchedulingHint, digest('1'))
            .unwrap();
        apply(&mut stream, discovery);
    }

    #[test]
    fn completed_is_reserved_for_a_closed_required_frontier() {
        let header = header(2, 'f', 'd');
        let lease = lease(&header, 1, 'a');
        let mut stream = open(header.clone(), lease);
        let payload = TerminalPayloadHash::from_canonical_semantic_payload(b"empty");
        let method = TerminalMethodHash::from_canonical_method(b"exact-exhaustion");
        assert!(matches!(
            stream.prepare_seal(lease, TerminalSealKind::Completed, payload, method),
            Err(ExploreRunStreamError::CompletedWithOpenFrontier)
        ));

        let closed_cases = ExactCaseSupport::full(header.case_universe());
        let close = stream
            .prepare_frontier_transition(
                lease,
                FrontierEvidenceKind::ExactExhaustion,
                RequiredFrontier::new(closed_cases, []).unwrap(),
                [case_fact(&header, (0, 2), '1')],
                digest('2'),
            )
            .unwrap();
        apply(&mut stream, close);
        let before = stream.journal_head();
        let seal_transition = stream
            .prepare_seal(lease, TerminalSealKind::Completed, payload, method)
            .unwrap();
        apply(&mut stream, seal_transition);
        let seal = stream.terminal_seal().unwrap();
        assert_eq!(seal.journal_head_before_seal(), before);
        assert_ne!(
            seal.journal_head_before_seal(),
            seal.terminal_journal_head()
        );
        assert_eq!(seal.terminal_payload_hash(), payload);
        assert_eq!(stream.lifecycle(), RunLifecycle::Sealed);
    }

    #[test]
    fn noncomplete_terminal_seal_preserves_an_open_frontier() {
        let header = header(10, 'f', 'd');
        let lease = lease(&header, 1, 'a');
        let mut stream = open(header, lease);
        let seal_transition = stream
            .prepare_seal(
                lease,
                TerminalSealKind::Unknown,
                TerminalPayloadHash::from_canonical_semantic_payload(b"partial"),
                TerminalMethodHash::from_canonical_method(b"backend-unknown"),
            )
            .unwrap();
        apply(&mut stream, seal_transition);
        let seal = stream.terminal_seal().unwrap();
        assert_eq!(seal.kind(), TerminalSealKind::Unknown);
        assert!(!stream.frontier().is_closed());
    }

    #[test]
    fn probe_plan_changes_run_id_but_not_equivalent_evidence_root() {
        let first_header = header(10, 'f', 'd');
        let second_header = header(10, 'f', 'e');
        assert_ne!(first_header.run_id(), second_header.run_id());
        let first_lease = lease(&first_header, 1, 'a');
        let second_lease = lease(&second_header, 1, 'a');
        let first = open(first_header, first_lease);
        let second = open(second_header, second_lease);
        assert_eq!(first.evidence_root(), second.evidence_root());
    }
}
