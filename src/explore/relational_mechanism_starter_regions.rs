//! Bounded, exact point-fiber regions for mechanism starter support.
//!
//! This module is deliberately independent of publication and journal
//! machinery.  It consumes an already authenticated canonical stream of
//! `(SourceKey, SuccessorKey)` members with their typed values and builds a
//! bounded navigation index.  Every V1 region is one complete source fiber:
//! exact `(Context, Before)` values map to an ordered, nonempty set of exact
//! `(SuccessorKey, After)` members.  Consequently the representation cannot
//! widen correlated source coordinates into a Cartesian product.
//!
//! Limits are applied transactionally at source boundaries.  If either the
//! committed-fiber limit or the per-fiber successor limit is reached, the
//! whole current source is omitted and the summary records the canonical
//! cursor immediately before that source.  A caller can therefore resume the
//! authoritative page stream without losing or partially representing a
//! source fiber. A publication adapter may apply the same whole-fiber rule
//! after exact encoded-size measurement. Semantic roots bind every cap and
//! depend on canonical members and completion, never on caller page
//! boundaries.

use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;

use sha2::{Digest, Sha256};

use super::relation::{SourceKey, SuccessorKey};
use super::{transition::canonical_explore_value_digest, ExploreValue};

pub(crate) const RELATIONAL_MECHANISM_STARTER_REGION_VERSION: u32 = 1;

const SUCCESSOR_FIBER_ID_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-region.successor-fiber-id.v1";
const REGION_ID_V1: &[u8] = b"futuruna.explore.relational-mechanism-starter-region.region-id.v1";
const CONTENT_GENESIS_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-region.content-genesis.v1";
const CONTENT_APPEND_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-region.content-append.v1";
const SUMMARY_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-mechanism-starter-region.summary-root.v1";

/// Deterministic hard limits for one region accumulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterRegionLimits {
    maximum_committed_fibers: NonZeroUsize,
    maximum_successors_per_fiber: NonZeroUsize,
    maximum_encoded_region_bytes: NonZeroUsize,
}

impl RelationalMechanismStarterRegionLimits {
    pub(crate) const fn new(
        maximum_committed_fibers: NonZeroUsize,
        maximum_successors_per_fiber: NonZeroUsize,
        maximum_encoded_region_bytes: NonZeroUsize,
    ) -> Self {
        Self {
            maximum_committed_fibers,
            maximum_successors_per_fiber,
            maximum_encoded_region_bytes,
        }
    }

    pub(crate) const fn maximum_committed_fibers(self) -> NonZeroUsize {
        self.maximum_committed_fibers
    }

    pub(crate) const fn maximum_successors_per_fiber(self) -> NonZeroUsize {
        self.maximum_successors_per_fiber
    }

    pub(crate) const fn maximum_encoded_region_bytes(self) -> NonZeroUsize {
        self.maximum_encoded_region_bytes
    }
}

/// Canonical member coordinate used both for ordering and fallback paging.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterRegionCursor {
    source_key: SourceKey,
    successor_key: SuccessorKey,
}

impl RelationalMechanismStarterRegionCursor {
    pub(crate) const fn new(source_key: SourceKey, successor_key: SuccessorKey) -> Self {
        Self {
            source_key,
            successor_key,
        }
    }

    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn successor_key(self) -> SuccessorKey {
        self.successor_key
    }
}

/// Borrowed input member.  Callers remain responsible for authenticating the
/// keys and values against their enclosing exact-fiber authority.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationalMechanismStarterRegionMemberRef<'a> {
    source_key: SourceKey,
    context: &'a ExploreValue,
    before: &'a ExploreValue,
    successor_key: SuccessorKey,
    after: &'a ExploreValue,
}

impl<'a> RelationalMechanismStarterRegionMemberRef<'a> {
    pub(crate) const fn new(
        source_key: SourceKey,
        context: &'a ExploreValue,
        before: &'a ExploreValue,
        successor_key: SuccessorKey,
        after: &'a ExploreValue,
    ) -> Self {
        Self {
            source_key,
            context,
            before,
            successor_key,
            after,
        }
    }

    pub(crate) const fn cursor(self) -> RelationalMechanismStarterRegionCursor {
        RelationalMechanismStarterRegionCursor::new(self.source_key, self.successor_key)
    }
}

/// Exact typed member of one dependent successor fiber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterRegionSuccessor {
    successor_key: SuccessorKey,
    after: ExploreValue,
}

impl RelationalMechanismStarterRegionSuccessor {
    pub(crate) const fn successor_key(&self) -> SuccessorKey {
        self.successor_key
    }

    pub(crate) const fn after(&self) -> &ExploreValue {
        &self.after
    }
}

/// Content identity of one source-relative ordered After fiber.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterFiberId([u8; 32]);

impl RelationalMechanismStarterFiberId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of one exact typed point-fiber region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterRegionId([u8; 32]);

impl RelationalMechanismStarterRegionId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One exact V1 region.  The source tuple and its After fiber are inseparable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterRegion {
    id: RelationalMechanismStarterRegionId,
    fiber_id: RelationalMechanismStarterFiberId,
    source_key: SourceKey,
    context: ExploreValue,
    before: ExploreValue,
    successors: Box<[RelationalMechanismStarterRegionSuccessor]>,
}

impl RelationalMechanismStarterRegion {
    pub(crate) const fn id(&self) -> RelationalMechanismStarterRegionId {
        self.id
    }

    pub(crate) const fn fiber_id(&self) -> RelationalMechanismStarterFiberId {
        self.fiber_id
    }

    pub(crate) const fn source_key(&self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn context(&self) -> &ExploreValue {
        &self.context
    }

    pub(crate) const fn before(&self) -> &ExploreValue {
        &self.before
    }

    pub(crate) fn successors(&self) -> &[RelationalMechanismStarterRegionSuccessor] {
        &self.successors
    }

    pub(crate) fn end_cursor(&self) -> RelationalMechanismStarterRegionCursor {
        let successor = self
            .successors
            .last()
            .expect("validated starter regions have nonempty successor fibers");
        RelationalMechanismStarterRegionCursor::new(self.source_key, successor.successor_key)
    }
}

/// Page-boundary-independent root of the represented exact region prefix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterRegionContentRoot([u8; 32]);

impl RelationalMechanismStarterRegionContentRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Identity of the complete summary, including its cap policy and fallback.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalMechanismStarterRegionSummaryRoot([u8; 32]);

impl RelationalMechanismStarterRegionSummaryRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterRegionFallbackReason {
    CommittedFiberLimit { limit: NonZeroUsize },
    SuccessorsPerFiberLimit { limit: NonZeroUsize },
    EncodedRegionByteLimit { limit: NonZeroUsize },
}

/// Explicit handoff to the authoritative canonical page stream.  `start_after`
/// always precedes the first omitted source; no member of `source_key` is
/// represented by this summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterRegionFallback {
    source_key: SourceKey,
    start_after: Option<RelationalMechanismStarterRegionCursor>,
    reason: RelationalMechanismStarterRegionFallbackReason,
}

impl RelationalMechanismStarterRegionFallback {
    pub(crate) const fn source_key(self) -> SourceKey {
        self.source_key
    }

    pub(crate) const fn start_after(self) -> Option<RelationalMechanismStarterRegionCursor> {
        self.start_after
    }

    pub(crate) const fn reason(self) -> RelationalMechanismStarterRegionFallbackReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterRegionCompletion {
    Complete,
    Capped(RelationalMechanismStarterRegionFallback),
}

/// Immutable bounded result.  Counts describe only the represented exact
/// prefix; an enclosing projection closure remains authoritative for totals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalMechanismStarterRegionSummary {
    root: RelationalMechanismStarterRegionSummaryRoot,
    content_root: RelationalMechanismStarterRegionContentRoot,
    limits: RelationalMechanismStarterRegionLimits,
    represented_exact_case_count: u128,
    represented_exact_starter_count: u128,
    regions: Box<[RelationalMechanismStarterRegion]>,
    completion: RelationalMechanismStarterRegionCompletion,
}

impl RelationalMechanismStarterRegionSummary {
    pub(crate) const fn root(&self) -> RelationalMechanismStarterRegionSummaryRoot {
        self.root
    }

    pub(crate) const fn content_root(&self) -> RelationalMechanismStarterRegionContentRoot {
        self.content_root
    }

    pub(crate) const fn limits(&self) -> RelationalMechanismStarterRegionLimits {
        self.limits
    }

    pub(crate) const fn represented_exact_case_count(&self) -> u128 {
        self.represented_exact_case_count
    }

    pub(crate) const fn represented_exact_starter_count(&self) -> u128 {
        self.represented_exact_starter_count
    }

    pub(crate) fn regions(&self) -> &[RelationalMechanismStarterRegion] {
        &self.regions
    }

    pub(crate) const fn completion(&self) -> RelationalMechanismStarterRegionCompletion {
        self.completion
    }

    /// Replace the represented prefix with the complete fibers before
    /// `region_index`. The rejected region and every following region remain
    /// reachable through the canonical starter-page fallback. This adapter is
    /// used after exact publication-line sizing, which depends on the outer
    /// artifact envelope and therefore does not belong in the pure member
    /// accumulator.
    pub(crate) fn cap_before_encoded_region(
        mut self,
        region_index: usize,
    ) -> Result<Self, RelationalMechanismStarterRegionError> {
        let rejected = self
            .regions
            .get(region_index)
            .ok_or(RelationalMechanismStarterRegionError::InvalidEncodedRegionIndex)?;
        let fallback = RelationalMechanismStarterRegionFallback {
            source_key: rejected.source_key,
            start_after: region_index
                .checked_sub(1)
                .and_then(|index| self.regions.get(index))
                .map(RelationalMechanismStarterRegion::end_cursor),
            reason: RelationalMechanismStarterRegionFallbackReason::EncodedRegionByteLimit {
                limit: self.limits.maximum_encoded_region_bytes,
            },
        };
        let retained = &self.regions[..region_index];
        let mut content_root = derive_content_genesis();
        let mut represented_exact_case_count = 0_u128;
        for region in retained {
            content_root = append_content_region(content_root, region.id);
            represented_exact_case_count = represented_exact_case_count
                .checked_add(region.successors.len() as u128)
                .ok_or(RelationalMechanismStarterRegionError::CountOverflow)?;
        }
        let represented_exact_starter_count = region_index as u128;
        let completion = RelationalMechanismStarterRegionCompletion::Capped(fallback);
        let root = derive_summary_root(
            content_root,
            self.limits,
            represented_exact_case_count,
            represented_exact_starter_count,
            region_index,
            completion,
        );
        self.regions = self.regions[..region_index].to_vec().into_boxed_slice();
        self.content_root = content_root;
        self.represented_exact_case_count = represented_exact_case_count;
        self.represented_exact_starter_count = represented_exact_starter_count;
        self.completion = completion;
        self.root = root;
        Ok(self)
    }
}

/// Result of accepting a member or page.  `Capped` is a successful bounded
/// outcome; callers should stop feeding input and use its rewind cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterRegionAccept {
    Accepted,
    Capped(RelationalMechanismStarterRegionFallback),
}

#[derive(Clone, Debug)]
struct PendingSourceFiber {
    source_key: SourceKey,
    context: ExploreValue,
    before: ExploreValue,
    successors: Vec<RelationalMechanismStarterRegionSuccessor>,
    start_after: Option<RelationalMechanismStarterRegionCursor>,
}

/// Pure bounded accumulator over a canonical member stream.
#[derive(Clone, Debug)]
pub(crate) struct RelationalMechanismStarterRegionAccumulator {
    limits: RelationalMechanismStarterRegionLimits,
    pending: Option<PendingSourceFiber>,
    regions: Vec<RelationalMechanismStarterRegion>,
    last_accepted_cursor: Option<RelationalMechanismStarterRegionCursor>,
    fallback: Option<RelationalMechanismStarterRegionFallback>,
    represented_exact_case_count: u128,
    represented_exact_starter_count: u128,
    content_root: RelationalMechanismStarterRegionContentRoot,
}

impl RelationalMechanismStarterRegionAccumulator {
    pub(crate) fn new(limits: RelationalMechanismStarterRegionLimits) -> Self {
        Self {
            limits,
            pending: None,
            regions: Vec::new(),
            last_accepted_cursor: None,
            fallback: None,
            represented_exact_case_count: 0,
            represented_exact_starter_count: 0,
            content_root: derive_content_genesis(),
        }
    }

    pub(crate) const fn limits(&self) -> RelationalMechanismStarterRegionLimits {
        self.limits
    }

    pub(crate) fn accept(
        &mut self,
        member: RelationalMechanismStarterRegionMemberRef<'_>,
    ) -> Result<RelationalMechanismStarterRegionAccept, RelationalMechanismStarterRegionError> {
        if let Some(fallback) = self.fallback {
            return Ok(RelationalMechanismStarterRegionAccept::Capped(fallback));
        }

        let cursor = member.cursor();
        if self
            .last_accepted_cursor
            .is_some_and(|previous| cursor <= previous)
        {
            return Err(
                RelationalMechanismStarterRegionError::NonCanonicalMemberOrder {
                    previous: self.last_accepted_cursor,
                    next: cursor,
                },
            );
        }

        match self.pending.as_ref() {
            Some(pending) if pending.source_key == member.source_key => {
                if &pending.context != member.context || &pending.before != member.before {
                    return Err(RelationalMechanismStarterRegionError::SourceValueConflict {
                        source_key: member.source_key,
                    });
                }
                if pending.successors.len() == self.limits.maximum_successors_per_fiber.get() {
                    let fallback = RelationalMechanismStarterRegionFallback {
                        source_key: pending.source_key,
                        start_after: pending.start_after,
                        reason:
                            RelationalMechanismStarterRegionFallbackReason::SuccessorsPerFiberLimit {
                                limit: self.limits.maximum_successors_per_fiber,
                            },
                    };
                    self.pending = None;
                    self.last_accepted_cursor = fallback.start_after;
                    self.fallback = Some(fallback);
                    return Ok(RelationalMechanismStarterRegionAccept::Capped(fallback));
                }
            }
            Some(_) => {
                self.commit_pending()?;
                if self.regions.len() == self.limits.maximum_committed_fibers.get() {
                    let fallback = RelationalMechanismStarterRegionFallback {
                        source_key: member.source_key,
                        start_after: self.last_accepted_cursor,
                        reason:
                            RelationalMechanismStarterRegionFallbackReason::CommittedFiberLimit {
                                limit: self.limits.maximum_committed_fibers,
                            },
                    };
                    self.fallback = Some(fallback);
                    return Ok(RelationalMechanismStarterRegionAccept::Capped(fallback));
                }
            }
            None => {
                if self.regions.len() == self.limits.maximum_committed_fibers.get() {
                    let fallback = RelationalMechanismStarterRegionFallback {
                        source_key: member.source_key,
                        start_after: self.last_accepted_cursor,
                        reason:
                            RelationalMechanismStarterRegionFallbackReason::CommittedFiberLimit {
                                limit: self.limits.maximum_committed_fibers,
                            },
                    };
                    self.fallback = Some(fallback);
                    return Ok(RelationalMechanismStarterRegionAccept::Capped(fallback));
                }
            }
        }

        if self.pending.is_none() {
            self.pending = Some(PendingSourceFiber {
                source_key: member.source_key,
                context: member.context.clone(),
                before: member.before.clone(),
                successors: Vec::with_capacity(
                    self.limits.maximum_successors_per_fiber.get().min(16),
                ),
                start_after: self.last_accepted_cursor,
            });
        }
        let pending = self
            .pending
            .as_mut()
            .expect("the current source fiber was installed above");
        if pending.source_key != member.source_key {
            return Err(RelationalMechanismStarterRegionError::InternalSourceBoundary);
        }
        pending
            .successors
            .push(RelationalMechanismStarterRegionSuccessor {
                successor_key: member.successor_key,
                after: member.after.clone(),
            });
        self.last_accepted_cursor = Some(cursor);
        Ok(RelationalMechanismStarterRegionAccept::Accepted)
    }

    /// Consume any borrowed member iterator.  This has no notion of a physical
    /// page; splitting the same canonical sequence differently cannot affect
    /// the resulting IDs or roots.
    pub(crate) fn accept_page<'a>(
        &mut self,
        members: impl IntoIterator<Item = RelationalMechanismStarterRegionMemberRef<'a>>,
    ) -> Result<RelationalMechanismStarterRegionAccept, RelationalMechanismStarterRegionError> {
        for member in members {
            let disposition = self.accept(member)?;
            if matches!(
                disposition,
                RelationalMechanismStarterRegionAccept::Capped(_)
            ) {
                return Ok(disposition);
            }
        }
        Ok(RelationalMechanismStarterRegionAccept::Accepted)
    }

    /// Finish the caller-declared input stream.  Calling this method asserts
    /// that no canonical input remains unless the accumulator already capped.
    pub(crate) fn finish(
        mut self,
    ) -> Result<RelationalMechanismStarterRegionSummary, RelationalMechanismStarterRegionError>
    {
        if self.fallback.is_none() {
            self.commit_pending()?;
        }
        let completion = self.fallback.map_or(
            RelationalMechanismStarterRegionCompletion::Complete,
            RelationalMechanismStarterRegionCompletion::Capped,
        );
        let root = derive_summary_root(
            self.content_root,
            self.limits,
            self.represented_exact_case_count,
            self.represented_exact_starter_count,
            self.regions.len(),
            completion,
        );
        Ok(RelationalMechanismStarterRegionSummary {
            root,
            content_root: self.content_root,
            limits: self.limits,
            represented_exact_case_count: self.represented_exact_case_count,
            represented_exact_starter_count: self.represented_exact_starter_count,
            regions: self.regions.into_boxed_slice(),
            completion,
        })
    }

    fn commit_pending(&mut self) -> Result<(), RelationalMechanismStarterRegionError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        if pending.successors.is_empty()
            || pending.successors.len() > self.limits.maximum_successors_per_fiber.get()
            || self.regions.len() >= self.limits.maximum_committed_fibers.get()
        {
            return Err(RelationalMechanismStarterRegionError::InternalLimitInvariant);
        }
        if self
            .regions
            .last()
            .is_some_and(|previous| previous.source_key >= pending.source_key)
        {
            return Err(RelationalMechanismStarterRegionError::InternalSourceBoundary);
        }

        let fiber_id = derive_fiber_id(pending.source_key, &pending.successors);
        let id = derive_region_id(
            pending.source_key,
            &pending.context,
            &pending.before,
            fiber_id,
        );
        let case_count = u128::try_from(pending.successors.len())
            .map_err(|_| RelationalMechanismStarterRegionError::CountOverflow)?;
        self.represented_exact_case_count = self
            .represented_exact_case_count
            .checked_add(case_count)
            .ok_or(RelationalMechanismStarterRegionError::CountOverflow)?;
        self.represented_exact_starter_count = self
            .represented_exact_starter_count
            .checked_add(1)
            .ok_or(RelationalMechanismStarterRegionError::CountOverflow)?;
        self.content_root = append_content_region(self.content_root, id);
        self.regions.push(RelationalMechanismStarterRegion {
            id,
            fiber_id,
            source_key: pending.source_key,
            context: pending.context,
            before: pending.before,
            successors: pending.successors.into_boxed_slice(),
        });
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalMechanismStarterRegionError {
    NonCanonicalMemberOrder {
        previous: Option<RelationalMechanismStarterRegionCursor>,
        next: RelationalMechanismStarterRegionCursor,
    },
    SourceValueConflict {
        source_key: SourceKey,
    },
    InvalidEncodedRegionIndex,
    CountOverflow,
    InternalSourceBoundary,
    InternalLimitInvariant,
}

impl fmt::Display for RelationalMechanismStarterRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalMemberOrder { .. } => formatter.write_str(
                "mechanism starter region members are not in strict (SourceKey, SuccessorKey) order",
            ),
            Self::SourceValueConflict { .. } => formatter.write_str(
                "one mechanism starter SourceKey was paired with conflicting Context/Before values",
            ),
            Self::InvalidEncodedRegionIndex => formatter.write_str(
                "mechanism starter encoded-region cap does not address a represented fiber",
            ),
            Self::CountOverflow => {
                formatter.write_str("mechanism starter region count overflowed")
            }
            Self::InternalSourceBoundary => formatter.write_str(
                "mechanism starter region accumulator violated a source-fiber boundary",
            ),
            Self::InternalLimitInvariant => formatter.write_str(
                "mechanism starter region accumulator violated its hard-limit invariant",
            ),
        }
    }
}

impl Error for RelationalMechanismStarterRegionError {}

fn derive_fiber_id(
    source_key: SourceKey,
    successors: &[RelationalMechanismStarterRegionSuccessor],
) -> RelationalMechanismStarterFiberId {
    let mut encoder = RegionHasher::new(SUCCESSOR_FIBER_ID_V1);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_REGION_VERSION);
    encoder.digest(source_key.bytes());
    encoder.u128(successors.len() as u128);
    for successor in successors {
        encoder.digest(successor.successor_key.bytes());
        encoder.digest(canonical_explore_value_digest(&successor.after));
    }
    RelationalMechanismStarterFiberId(encoder.finish())
}

fn derive_region_id(
    source_key: SourceKey,
    context: &ExploreValue,
    before: &ExploreValue,
    fiber_id: RelationalMechanismStarterFiberId,
) -> RelationalMechanismStarterRegionId {
    let mut encoder = RegionHasher::new(REGION_ID_V1);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_REGION_VERSION);
    encoder.digest(source_key.bytes());
    encoder.digest(canonical_explore_value_digest(context));
    encoder.digest(canonical_explore_value_digest(before));
    encoder.digest(fiber_id.bytes());
    RelationalMechanismStarterRegionId(encoder.finish())
}

fn derive_content_genesis() -> RelationalMechanismStarterRegionContentRoot {
    let mut encoder = RegionHasher::new(CONTENT_GENESIS_V1);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_REGION_VERSION);
    RelationalMechanismStarterRegionContentRoot(encoder.finish())
}

fn append_content_region(
    prior: RelationalMechanismStarterRegionContentRoot,
    region_id: RelationalMechanismStarterRegionId,
) -> RelationalMechanismStarterRegionContentRoot {
    let mut encoder = RegionHasher::new(CONTENT_APPEND_V1);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_REGION_VERSION);
    encoder.digest(prior.bytes());
    encoder.digest(region_id.bytes());
    RelationalMechanismStarterRegionContentRoot(encoder.finish())
}

fn derive_summary_root(
    content_root: RelationalMechanismStarterRegionContentRoot,
    limits: RelationalMechanismStarterRegionLimits,
    represented_exact_case_count: u128,
    represented_exact_starter_count: u128,
    region_count: usize,
    completion: RelationalMechanismStarterRegionCompletion,
) -> RelationalMechanismStarterRegionSummaryRoot {
    let mut encoder = RegionHasher::new(SUMMARY_ROOT_V1);
    encoder.u32(RELATIONAL_MECHANISM_STARTER_REGION_VERSION);
    encoder.digest(content_root.bytes());
    encoder.u128(limits.maximum_committed_fibers.get() as u128);
    encoder.u128(limits.maximum_successors_per_fiber.get() as u128);
    encoder.u128(limits.maximum_encoded_region_bytes.get() as u128);
    encoder.u128(represented_exact_case_count);
    encoder.u128(represented_exact_starter_count);
    encoder.u128(region_count as u128);
    match completion {
        RelationalMechanismStarterRegionCompletion::Complete => encoder.tag(0x01),
        RelationalMechanismStarterRegionCompletion::Capped(fallback) => {
            encoder.tag(0x02);
            encoder.digest(fallback.source_key.bytes());
            match fallback.start_after {
                Some(cursor) => {
                    encoder.tag(0x01);
                    encoder.digest(cursor.source_key.bytes());
                    encoder.digest(cursor.successor_key.bytes());
                }
                None => encoder.tag(0x00),
            }
            match fallback.reason {
                RelationalMechanismStarterRegionFallbackReason::CommittedFiberLimit { limit } => {
                    encoder.tag(0x01);
                    encoder.u128(limit.get() as u128);
                }
                RelationalMechanismStarterRegionFallbackReason::SuccessorsPerFiberLimit {
                    limit,
                } => {
                    encoder.tag(0x02);
                    encoder.u128(limit.get() as u128);
                }
                RelationalMechanismStarterRegionFallbackReason::EncodedRegionByteLimit {
                    limit,
                } => {
                    encoder.tag(0x03);
                    encoder.u128(limit.get() as u128);
                }
            }
        }
    }
    RelationalMechanismStarterRegionSummaryRoot(encoder.finish())
}

struct RegionHasher(Sha256);

impl RegionHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u128).to_be_bytes());
        self.0.update(bytes);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.0.update(digest);
    }

    fn u32(&mut self, value: u32) {
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
    use std::collections::BTreeSet;
    use std::num::NonZeroUsize;

    use super::*;

    fn source_key(tag: u8) -> SourceKey {
        SourceKey::from_journal_codec_bytes([tag; 32])
    }

    fn successor_key(tag: u8) -> SuccessorKey {
        SuccessorKey::from_journal_codec_bytes([tag; 32])
    }

    struct TestMember {
        source_key: SourceKey,
        context: ExploreValue,
        before: ExploreValue,
        successor_key: SuccessorKey,
        after: ExploreValue,
    }

    impl TestMember {
        fn as_ref(&self) -> RelationalMechanismStarterRegionMemberRef<'_> {
            RelationalMechanismStarterRegionMemberRef::new(
                self.source_key,
                &self.context,
                &self.before,
                self.successor_key,
                &self.after,
            )
        }
    }

    fn member(
        source_tag: u8,
        successor_tag: u8,
        context: &str,
        before: i64,
        after: i64,
    ) -> TestMember {
        TestMember {
            source_key: source_key(source_tag),
            context: ExploreValue::String(context.into()),
            before: ExploreValue::Int(before),
            successor_key: successor_key(successor_tag),
            after: ExploreValue::Int(after),
        }
    }

    fn limits(fibers: usize, successors: usize) -> RelationalMechanismStarterRegionLimits {
        RelationalMechanismStarterRegionLimits::new(
            NonZeroUsize::new(fibers).unwrap(),
            NonZeroUsize::new(successors).unwrap(),
            NonZeroUsize::new(1 << 20).unwrap(),
        )
    }

    #[test]
    fn diagonal_sources_reconstruct_without_cartesian_widening() {
        let mut accumulator = RelationalMechanismStarterRegionAccumulator::new(limits(8, 8));
        accumulator
            .accept(member(1, 11, "Copenhagen", 100, 101).as_ref())
            .unwrap();
        accumulator
            .accept(member(2, 12, "Aarhus", 200, 201).as_ref())
            .unwrap();
        let summary = accumulator.finish().unwrap();

        let triples = summary
            .regions()
            .iter()
            .flat_map(|region| {
                region.successors().iter().map(|successor| {
                    (
                        region.context().clone(),
                        region.before().clone(),
                        successor.after().clone(),
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(triples.len(), 2);
        assert!(triples.contains(&(
            ExploreValue::String("Copenhagen".into()),
            ExploreValue::Int(100),
            ExploreValue::Int(101),
        )));
        assert!(triples.contains(&(
            ExploreValue::String("Aarhus".into()),
            ExploreValue::Int(200),
            ExploreValue::Int(201),
        )));
        assert!(!triples.contains(&(
            ExploreValue::String("Copenhagen".into()),
            ExploreValue::Int(200),
            ExploreValue::Int(201),
        )));
    }

    #[test]
    fn one_starter_retains_two_ordered_afters() {
        let mut accumulator = RelationalMechanismStarterRegionAccumulator::new(limits(4, 4));
        accumulator
            .accept(member(1, 11, "same", 100, 101).as_ref())
            .unwrap();
        accumulator
            .accept(member(1, 12, "same", 100, 102).as_ref())
            .unwrap();
        let summary = accumulator.finish().unwrap();

        assert_eq!(summary.represented_exact_starter_count(), 1);
        assert_eq!(summary.represented_exact_case_count(), 2);
        assert_eq!(summary.regions().len(), 1);
        assert_eq!(summary.regions()[0].successors().len(), 2);
        assert!(
            summary.regions()[0].successors()[0].successor_key()
                < summary.regions()[0].successors()[1].successor_key()
        );
    }

    #[test]
    fn semantic_roots_ignore_input_page_boundaries() {
        let members = vec![
            member(1, 11, "one", 100, 101),
            member(1, 12, "one", 100, 102),
            member(2, 13, "two", 200, 201),
        ];

        let mut single_page = RelationalMechanismStarterRegionAccumulator::new(limits(8, 8));
        single_page
            .accept_page(members.iter().map(|member| member.as_ref()))
            .unwrap();
        let single_page = single_page.finish().unwrap();

        let mut split_pages = RelationalMechanismStarterRegionAccumulator::new(limits(8, 8));
        split_pages
            .accept_page(members[..1].iter().map(|member| member.as_ref()))
            .unwrap();
        split_pages
            .accept_page(members[1..2].iter().map(|member| member.as_ref()))
            .unwrap();
        split_pages
            .accept_page(members[2..].iter().map(|member| member.as_ref()))
            .unwrap();
        let split_pages = split_pages.finish().unwrap();

        assert_eq!(single_page.content_root(), split_pages.content_root());
        assert_eq!(single_page.root(), split_pages.root());
        assert_eq!(single_page.regions(), split_pages.regions());
    }

    #[test]
    fn committed_fiber_cap_falls_back_before_the_next_whole_source() {
        let mut accumulator = RelationalMechanismStarterRegionAccumulator::new(limits(1, 8));
        accumulator
            .accept(member(1, 11, "one", 100, 101).as_ref())
            .unwrap();
        let disposition = accumulator
            .accept(member(2, 12, "two", 200, 201).as_ref())
            .unwrap();
        let RelationalMechanismStarterRegionAccept::Capped(fallback) = disposition else {
            panic!("second source should exceed the committed-fiber cap");
        };
        assert_eq!(fallback.source_key(), source_key(2));
        assert_eq!(
            fallback.start_after(),
            Some(RelationalMechanismStarterRegionCursor::new(
                source_key(1),
                successor_key(11),
            ))
        );

        let summary = accumulator.finish().unwrap();
        assert_eq!(summary.regions().len(), 1);
        assert_eq!(summary.represented_exact_starter_count(), 1);
        assert_eq!(summary.represented_exact_case_count(), 1);
        assert_eq!(
            summary.completion(),
            RelationalMechanismStarterRegionCompletion::Capped(fallback)
        );
    }

    #[test]
    fn overlarge_first_fiber_emits_zero_regions_and_rewinds_to_start() {
        let mut accumulator = RelationalMechanismStarterRegionAccumulator::new(limits(8, 1));
        accumulator
            .accept(member(1, 11, "one", 100, 101).as_ref())
            .unwrap();
        let disposition = accumulator
            .accept(member(1, 12, "one", 100, 102).as_ref())
            .unwrap();
        let RelationalMechanismStarterRegionAccept::Capped(fallback) = disposition else {
            panic!("second successor should exceed the per-fiber cap");
        };
        assert_eq!(fallback.source_key(), source_key(1));
        assert_eq!(fallback.start_after(), None);

        let summary = accumulator.finish().unwrap();
        assert!(summary.regions().is_empty());
        assert_eq!(summary.represented_exact_starter_count(), 0);
        assert_eq!(summary.represented_exact_case_count(), 0);
        assert_eq!(
            summary.completion(),
            RelationalMechanismStarterRegionCompletion::Capped(fallback)
        );
    }

    #[test]
    fn encoded_region_cap_discards_the_whole_rejected_fiber() {
        let mut accumulator = RelationalMechanismStarterRegionAccumulator::new(limits(8, 8));
        accumulator
            .accept(member(1, 11, "one", 100, 101).as_ref())
            .unwrap();
        accumulator
            .accept(member(2, 12, "two", 200, 201).as_ref())
            .unwrap();
        accumulator
            .accept(member(2, 13, "two", 200, 202).as_ref())
            .unwrap();

        let summary = accumulator
            .finish()
            .unwrap()
            .cap_before_encoded_region(1)
            .unwrap();
        let RelationalMechanismStarterRegionCompletion::Capped(fallback) = summary.completion()
        else {
            panic!("encoded size rejection should cap the exact region prefix");
        };
        assert_eq!(summary.regions().len(), 1);
        assert_eq!(summary.represented_exact_starter_count(), 1);
        assert_eq!(summary.represented_exact_case_count(), 1);
        assert_eq!(fallback.source_key(), source_key(2));
        assert_eq!(
            fallback.start_after(),
            Some(RelationalMechanismStarterRegionCursor::new(
                source_key(1),
                successor_key(11),
            ))
        );
        assert_eq!(
            fallback.reason(),
            RelationalMechanismStarterRegionFallbackReason::EncodedRegionByteLimit {
                limit: NonZeroUsize::new(1 << 20).unwrap(),
            }
        );
    }
}
