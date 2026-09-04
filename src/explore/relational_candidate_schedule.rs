//! Candidate-informed ordering of canonical relational case chunks.
//!
//! This module changes only *which already-required chunk is attempted next*.
//! It does not create support, classify a case, or establish complement
//! closure. The canonical chunk partition remains the exact work cover; any
//! chunk not nominated by an endpoint or a safely lifted split candidate is
//! selected implicitly as residual fallback work.
//!
//! A source-axis coordinate is lifted only when there is exactly one integer
//! axis, exactly one plan for that axis, and its coordinate interval is
//! identical to the case partition's bare/product-factor interval. Ranked
//! products and plural axes need a real mixed-radix/slab proof before their
//! coordinates can be translated. Until then they simply receive endpoint
//! and canonical residual scheduling, never an error and never less coverage.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use super::relational_bounded_chunk_partition::{
    RelationalCaseChunk, RelationalCaseChunkDescriptor, RelationalCaseChunkId,
    RelationalCaseChunkPartition, RelationalCaseChunkPartitionArtifactId, RelationalCaseChunkShape,
};
use super::relational_proof_strategy::{
    RelationalAxisProofPlan, RelationalGuardOrigin, RelationalProofStrategyInventory,
    RelationalSplitOrigin, RelationalSplitPriority,
};
use super::relational_support_planner::RelationalDimensionId;

pub(crate) const RELATIONAL_CANDIDATE_NOMINATION_VERSION: u32 = 1;

const CANDIDATE_NOMINATION_ROOT_V1: &[u8] =
    b"futuruna.explore.relational-candidate-nomination-root.v1";

/// Operational commitment to why one exact canonical chunk was selected.
///
/// The root is intentionally absent from semantic answer and proof identities.
/// It authenticates scheduler provenance only: the policy/schema domain,
/// partition and complete chunk descriptor, plus every sorted nomination and
/// semantic origin that contributed to this target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCandidateNominationRoot([u8; 32]);

impl RelationalCandidateNominationRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable priority order for why a canonical chunk was nominated.
///
/// Checked boundaries come first because they are direct possible truth-value
/// changes. Compiler-minted source events, boundaries lifted from the
/// producer-owned classification graph, and certificate-proposed piece
/// boundaries follow, then the two range endpoints, and only then a
/// certificate-authorized midpoint. Every other chunk remains present as
/// residual fallback in canonical ordinal order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalCandidateScheduleReason {
    CheckedGuardBoundary,
    SourceEvent,
    LiftedCandidate,
    CertifiedPieceBoundary,
    LowerRangeEndpoint,
    UpperRangeEndpoint,
    CertificateAuthorizedMidpoint,
    ResidualFallback,
}

impl RelationalCandidateScheduleReason {
    fn from_split_priority(priority: RelationalSplitPriority) -> Self {
        match priority {
            RelationalSplitPriority::CheckedGuardBoundary => Self::CheckedGuardBoundary,
            RelationalSplitPriority::SourceEvent => Self::SourceEvent,
            RelationalSplitPriority::LiftedCandidate => Self::LiftedCandidate,
            RelationalSplitPriority::CertifiedPieceBoundary => Self::CertifiedPieceBoundary,
            RelationalSplitPriority::CertificateAuthorizedMidpoint => {
                Self::CertificateAuthorizedMidpoint
            }
        }
    }

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::CheckedGuardBoundary => 0x01,
            Self::SourceEvent => 0x02,
            Self::LiftedCandidate => 0x03,
            Self::CertifiedPieceBoundary => 0x04,
            Self::LowerRangeEndpoint => 0x05,
            Self::UpperRangeEndpoint => 0x06,
            Self::CertificateAuthorizedMidpoint => 0x07,
            Self::ResidualFallback => 0x08,
        }
    }
}

/// Which side of a half-open boundary supplied the concrete coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RelationalCandidateBoundarySide {
    LowerAdjacent,
    UpperAdjacent,
    ExactEndpoint,
    WholeChunkResidual,
}

impl RelationalCandidateBoundarySide {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::LowerAdjacent => 0x01,
            Self::UpperAdjacent => 0x02,
            Self::ExactEndpoint => 0x03,
            Self::WholeChunkResidual => 0x04,
        }
    }
}

/// Operational provenance for one nomination of a canonical chunk.
///
/// `split_coordinate` is the half-open source boundary and
/// `target_coordinate` is the concrete coordinate on one side of it. Range
/// endpoints have no source dimension or value boundary. Residual fallback
/// names the whole chunk and therefore has no point coordinate at all.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelationalCandidateChunkNomination {
    reason: RelationalCandidateScheduleReason,
    side: RelationalCandidateBoundarySide,
    dimension_id: Option<RelationalDimensionId>,
    split_coordinate: Option<u128>,
    value_boundary: Option<i128>,
    target_coordinate: Option<u128>,
    origins: Box<[RelationalSplitOrigin]>,
}

impl RelationalCandidateChunkNomination {
    pub(crate) const fn reason(&self) -> RelationalCandidateScheduleReason {
        self.reason
    }

    pub(crate) const fn side(&self) -> RelationalCandidateBoundarySide {
        self.side
    }

    pub(crate) const fn dimension_id(&self) -> Option<RelationalDimensionId> {
        self.dimension_id
    }

    pub(crate) const fn split_coordinate(&self) -> Option<u128> {
        self.split_coordinate
    }

    pub(crate) const fn value_boundary(&self) -> Option<i128> {
        self.value_boundary
    }

    pub(crate) const fn target_coordinate(&self) -> Option<u128> {
        self.target_coordinate
    }

    pub(crate) fn origins(&self) -> &[RelationalSplitOrigin] {
        &self.origins
    }

    fn endpoint(reason: RelationalCandidateScheduleReason, coordinate: u128) -> Self {
        debug_assert!(matches!(
            reason,
            RelationalCandidateScheduleReason::LowerRangeEndpoint
                | RelationalCandidateScheduleReason::UpperRangeEndpoint
        ));
        Self {
            reason,
            side: RelationalCandidateBoundarySide::ExactEndpoint,
            dimension_id: None,
            split_coordinate: None,
            value_boundary: None,
            target_coordinate: Some(coordinate),
            origins: Box::new([]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lifted(
        reason: RelationalCandidateScheduleReason,
        side: RelationalCandidateBoundarySide,
        dimension_id: RelationalDimensionId,
        split_coordinate: u128,
        value_boundary: i128,
        target_coordinate: u128,
        origins: &[RelationalSplitOrigin],
    ) -> Self {
        debug_assert!(matches!(
            side,
            RelationalCandidateBoundarySide::LowerAdjacent
                | RelationalCandidateBoundarySide::UpperAdjacent
        ));
        Self {
            reason,
            side,
            dimension_id: Some(dimension_id),
            split_coordinate: Some(split_coordinate),
            value_boundary: Some(value_boundary),
            target_coordinate: Some(target_coordinate),
            origins: origins.to_vec().into_boxed_slice(),
        }
    }

    fn residual() -> Self {
        Self {
            reason: RelationalCandidateScheduleReason::ResidualFallback,
            side: RelationalCandidateBoundarySide::WholeChunkResidual,
            dimension_id: None,
            split_coordinate: None,
            value_boundary: None,
            target_coordinate: None,
            origins: Box::new([]),
        }
    }
}

/// Why source-axis split coordinates were, or were not, lifted into chunks.
///
/// Every non-applied disposition is an optimization result, not an Explore
/// failure. Endpoint and residual work still give the exact canonical cover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalCandidateLiftDisposition {
    AppliedExactSingleAxis {
        dimension_id: RelationalDimensionId,
        shape: RelationalCaseChunkShape,
    },
    PlanScopeMismatch,
    ProductRankNeedsSlabProof,
    NoIntegerAxis,
    PluralIntegerAxes,
    MissingAxisPlan,
    PluralAxisPlans,
    AxisPlanMismatch,
    CoordinateIntervalMismatch,
    QuestionSetNotUnary,
}

impl RelationalCandidateLiftDisposition {
    pub(crate) const fn is_applied(self) -> bool {
        matches!(self, Self::AppliedExactSingleAxis { .. })
    }
}

/// One canonical chunk in candidate-first execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCandidateChunkTarget {
    descriptor: RelationalCaseChunkDescriptor,
    nominations: Box<[RelationalCandidateChunkNomination]>,
    nomination_root: RelationalCandidateNominationRoot,
}

impl RelationalCandidateChunkTarget {
    pub(crate) const fn descriptor(&self) -> &RelationalCaseChunkDescriptor {
        &self.descriptor
    }

    pub(crate) fn nominations(&self) -> &[RelationalCandidateChunkNomination] {
        &self.nominations
    }

    pub(crate) const fn nomination_root(&self) -> RelationalCandidateNominationRoot {
        self.nomination_root
    }

    pub(crate) fn primary_reason(&self) -> RelationalCandidateScheduleReason {
        self.nominations
            .first()
            .map(RelationalCandidateChunkNomination::reason)
            .unwrap_or(RelationalCandidateScheduleReason::ResidualFallback)
    }
}

/// Pure candidate-first schedule bound to one canonical chunk partition.
///
/// Only explicitly nominated chunks are retained. Residual order is derived
/// on demand from the partition, so a large exact search does not need a
/// second, duplicated list of all work merely to prioritize a handful of
/// interesting chunks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalCandidateChunkSchedule {
    partition_id: RelationalCaseChunkPartitionArtifactId,
    scheduler_policy_version: u32,
    lift_disposition: RelationalCandidateLiftDisposition,
    nominated: Box<[RelationalCandidateChunkTarget]>,
    /// Immutable ordinal lookup for resuming a multi-slice nominated chunk.
    /// `nominated` is priority-sorted, so an ordinal lookup cannot binary
    /// search it directly without this small O(C) derived index.
    nominated_index_by_ordinal: BTreeMap<u128, usize>,
    nominated_ids: BTreeSet<RelationalCaseChunkId>,
}

impl RelationalCandidateChunkSchedule {
    pub(crate) const fn partition_id(&self) -> RelationalCaseChunkPartitionArtifactId {
        self.partition_id
    }

    pub(crate) const fn lift_disposition(&self) -> RelationalCandidateLiftDisposition {
        self.lift_disposition
    }

    pub(crate) fn nominated(&self) -> &[RelationalCandidateChunkTarget] {
        &self.nominated
    }

    /// Resolve one canonical ordinal back to the same provenance-rich target
    /// the scheduler would have returned. This is used only to resume an
    /// already checkpointed concrete slice, which owns its chunk until that
    /// accumulator is finalized.
    pub(crate) fn target_for_ordinal(
        &self,
        partition: &RelationalCaseChunkPartition,
        chunk_ordinal: u128,
    ) -> Option<RelationalCandidateChunkTarget> {
        if !self.matches_partition(partition) {
            return None;
        }
        self.nominated_index_by_ordinal
            .get(&chunk_ordinal)
            .and_then(|index| self.nominated.get(*index))
            .cloned()
            .or_else(|| {
                usize::try_from(chunk_ordinal)
                    .ok()
                    .and_then(|index| partition.chunks().get(index))
                    .map(|chunk| {
                        residual_target(self.partition_id, self.scheduler_policy_version, chunk)
                    })
            })
    }

    /// Candidate visits alone never prove anything about the complement.
    pub(crate) const fn establishes_complement_closure(&self) -> bool {
        false
    }

    /// Return the highest-priority still-open target. Closed chunk IDs are
    /// durable work-completion facts, so reopening a run does not redo them.
    ///
    /// Explicit nominations cost O(C), where C is the finite extracted
    /// candidate set. Residual lookup begins at the durable committed prefix,
    /// so it visits only an already-occupied sparse suffix before the next
    /// open canonical residual instead of restarting an O(B) partition scan.
    pub(crate) fn next_target_where(
        &self,
        partition: &RelationalCaseChunkPartition,
        residual_start_ordinal: usize,
        mut is_closed: impl FnMut(u128) -> bool,
    ) -> Option<RelationalCandidateChunkTarget> {
        if !self.matches_partition(partition) {
            return None;
        }
        self.nominated
            .iter()
            .find(|target| !is_closed(target.descriptor.ordinal()))
            .cloned()
            .or_else(|| {
                partition
                    .chunks()
                    .iter()
                    .skip(residual_start_ordinal)
                    .find(|chunk| {
                        !self.nominated_ids.contains(&chunk.descriptor().id())
                            && !is_closed(chunk.descriptor().ordinal())
                    })
                    .map(|chunk| {
                        residual_target(self.partition_id, self.scheduler_policy_version, chunk)
                    })
            })
    }

    /// Materialize the complete deterministic order for audit/debug output.
    /// Execution should normally use [`Self::next_target`] and keep residuals
    /// implicit.
    pub(crate) fn complete_order(
        &self,
        partition: &RelationalCaseChunkPartition,
    ) -> Option<Box<[RelationalCandidateChunkTarget]>> {
        if !self.matches_partition(partition) {
            return None;
        }
        let mut targets = self.nominated.to_vec();
        targets.extend(
            partition
                .chunks()
                .iter()
                .filter(|chunk| !self.nominated_ids.contains(&chunk.descriptor().id()))
                .map(|chunk| {
                    residual_target(self.partition_id, self.scheduler_policy_version, chunk)
                }),
        );
        Some(targets.into_boxed_slice())
    }

    /// Check that candidate-first order is a permutation of the canonical
    /// exact partition and that the partition intervals themselves form one
    /// non-overlapping half-open cover.
    pub(crate) fn has_exact_nonoverlapping_cover(
        &self,
        partition: &RelationalCaseChunkPartition,
    ) -> bool {
        if !self.matches_partition(partition) || partition.artifact().validate_identity().is_err() {
            return false;
        }
        let mut expected_start = partition.artifact().interval_start();
        let mut canonical_ids = BTreeSet::new();
        for chunk in partition.chunks() {
            let descriptor = chunk.descriptor();
            if descriptor.interval_start() != expected_start
                || descriptor.interval_start() >= descriptor.interval_end_exclusive()
                || !canonical_ids.insert(descriptor.id())
            {
                return false;
            }
            expected_start = descriptor.interval_end_exclusive();
        }
        if expected_start != partition.artifact().interval_end_exclusive() {
            return false;
        }

        let Some(targets) = self.complete_order(partition) else {
            return false;
        };
        let scheduled_ids = targets
            .iter()
            .map(|target| target.descriptor.id())
            .collect::<BTreeSet<_>>();
        scheduled_ids == canonical_ids && targets.len() == partition.chunks().len()
    }

    fn matches_partition(&self, partition: &RelationalCaseChunkPartition) -> bool {
        self.partition_id == partition.artifact().id()
    }
}

/// Build candidate-first operational order over an already exact canonical
/// case-chunk partition.
///
/// Unsupported lifts are deliberately represented in `lift_disposition` and
/// otherwise ignored. The returned schedule always keeps the endpoints and
/// the complete implicit residual fallback of `partition`.
pub(crate) fn schedule_relational_candidate_chunks(
    inventory: &RelationalProofStrategyInventory,
    axis_plans: &[RelationalAxisProofPlan],
    partition: &RelationalCaseChunkPartition,
    scheduler_policy_version: u32,
) -> RelationalCandidateChunkSchedule {
    let mut nominations = BTreeMap::<u128, BTreeSet<RelationalCandidateChunkNomination>>::new();

    nominate_endpoint(
        partition,
        partition.artifact().interval_start(),
        RelationalCandidateScheduleReason::LowerRangeEndpoint,
        &mut nominations,
    );
    if let Some(upper) = partition.artifact().interval_end_exclusive().checked_sub(1) {
        nominate_endpoint(
            partition,
            upper,
            RelationalCandidateScheduleReason::UpperRangeEndpoint,
            &mut nominations,
        );
    }

    let lift_disposition =
        nominate_exact_lifted_candidates(inventory, axis_plans, partition, &mut nominations);
    finish_schedule(
        partition,
        scheduler_policy_version,
        lift_disposition,
        nominations,
    )
}

fn finish_schedule(
    partition: &RelationalCaseChunkPartition,
    scheduler_policy_version: u32,
    lift_disposition: RelationalCandidateLiftDisposition,
    nominations: BTreeMap<u128, BTreeSet<RelationalCandidateChunkNomination>>,
) -> RelationalCandidateChunkSchedule {
    let mut nominated = nominations
        .into_iter()
        .filter_map(|(ordinal, nominations)| {
            let ordinal = usize::try_from(ordinal).ok()?;
            let chunk = partition.chunks().get(ordinal)?;
            let descriptor = chunk.descriptor().clone();
            let nominations = nominations
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let nomination_root = derive_candidate_nomination_root(
                scheduler_policy_version,
                partition.artifact().id(),
                &descriptor,
                &nominations,
            );
            Some(RelationalCandidateChunkTarget {
                descriptor,
                nominations,
                nomination_root,
            })
        })
        .collect::<Vec<_>>();
    nominated.sort_by_key(|target| (target.primary_reason(), target.descriptor.ordinal()));
    let nominated_index_by_ordinal = nominated
        .iter()
        .enumerate()
        .map(|(index, target)| (target.descriptor.ordinal(), index))
        .collect();
    let nominated_ids = nominated
        .iter()
        .map(|target| target.descriptor.id())
        .collect();

    RelationalCandidateChunkSchedule {
        partition_id: partition.artifact().id(),
        scheduler_policy_version,
        lift_disposition,
        nominated: nominated.into_boxed_slice(),
        nominated_index_by_ordinal,
        nominated_ids,
    }
}

/// Build the exact endpoint-first/canonical-residual order when no unary
/// checked proof inventory can be formed. Plural question sets still use the
/// classified sweep; they merely skip the currently unary affine candidate
/// extractor.
pub(crate) fn schedule_relational_endpoint_chunks(
    partition: &RelationalCaseChunkPartition,
    lift_disposition: RelationalCandidateLiftDisposition,
    scheduler_policy_version: u32,
) -> RelationalCandidateChunkSchedule {
    let mut nominations = BTreeMap::<u128, BTreeSet<RelationalCandidateChunkNomination>>::new();
    nominate_endpoint(
        partition,
        partition.artifact().interval_start(),
        RelationalCandidateScheduleReason::LowerRangeEndpoint,
        &mut nominations,
    );
    if let Some(upper) = partition.artifact().interval_end_exclusive().checked_sub(1) {
        nominate_endpoint(
            partition,
            upper,
            RelationalCandidateScheduleReason::UpperRangeEndpoint,
            &mut nominations,
        );
    }
    finish_schedule(
        partition,
        scheduler_policy_version,
        lift_disposition,
        nominations,
    )
}

/// Test-only control used to prove that candidate order cannot change final
/// semantic evidence. Production has no scheduler toggle: it always uses the
/// candidate-informed policy above.
#[cfg(test)]
pub(crate) fn schedule_relational_canonical_chunks_for_test(
    partition: &RelationalCaseChunkPartition,
    scheduler_policy_version: u32,
) -> RelationalCandidateChunkSchedule {
    finish_schedule(
        partition,
        scheduler_policy_version,
        RelationalCandidateLiftDisposition::NoIntegerAxis,
        BTreeMap::new(),
    )
}

fn nominate_exact_lifted_candidates(
    inventory: &RelationalProofStrategyInventory,
    axis_plans: &[RelationalAxisProofPlan],
    partition: &RelationalCaseChunkPartition,
    nominations: &mut BTreeMap<u128, BTreeSet<RelationalCandidateChunkNomination>>,
) -> RelationalCandidateLiftDisposition {
    let artifact = partition.artifact();
    if inventory.plan_root() != artifact.plan_root()
        || inventory.relation_id() != artifact.relation_id()
    {
        return RelationalCandidateLiftDisposition::PlanScopeMismatch;
    }
    if artifact.shape() == RelationalCaseChunkShape::ProductRankInterval {
        return RelationalCandidateLiftDisposition::ProductRankNeedsSlabProof;
    }
    let [axis] = inventory.axes() else {
        return if inventory.axes().is_empty() {
            RelationalCandidateLiftDisposition::NoIntegerAxis
        } else {
            RelationalCandidateLiftDisposition::PluralIntegerAxes
        };
    };
    let [axis_plan] = axis_plans else {
        return if axis_plans.is_empty() {
            RelationalCandidateLiftDisposition::MissingAxisPlan
        } else {
            RelationalCandidateLiftDisposition::PluralAxisPlans
        };
    };
    if axis_plan.axis() != axis {
        return RelationalCandidateLiftDisposition::AxisPlanMismatch;
    }
    if axis.coordinate_start() != artifact.interval_start()
        || axis.coordinate_end_exclusive() != artifact.interval_end_exclusive()
    {
        return RelationalCandidateLiftDisposition::CoordinateIntervalMismatch;
    }

    for candidate in axis_plan.candidates() {
        let reason = RelationalCandidateScheduleReason::from_split_priority(candidate.priority());
        let Some(lower_coordinate) = candidate.coordinate().checked_sub(1) else {
            continue;
        };
        nominate_lifted_coordinate(
            partition,
            lower_coordinate,
            RelationalCandidateChunkNomination::lifted(
                reason,
                RelationalCandidateBoundarySide::LowerAdjacent,
                axis.dimension_id(),
                candidate.coordinate(),
                candidate.value_boundary(),
                lower_coordinate,
                candidate.origins(),
            ),
            nominations,
        );
        nominate_lifted_coordinate(
            partition,
            candidate.coordinate(),
            RelationalCandidateChunkNomination::lifted(
                reason,
                RelationalCandidateBoundarySide::UpperAdjacent,
                axis.dimension_id(),
                candidate.coordinate(),
                candidate.value_boundary(),
                candidate.coordinate(),
                candidate.origins(),
            ),
            nominations,
        );
    }

    RelationalCandidateLiftDisposition::AppliedExactSingleAxis {
        dimension_id: axis.dimension_id(),
        shape: artifact.shape(),
    }
}

fn nominate_endpoint(
    partition: &RelationalCaseChunkPartition,
    coordinate: u128,
    reason: RelationalCandidateScheduleReason,
    nominations: &mut BTreeMap<u128, BTreeSet<RelationalCandidateChunkNomination>>,
) {
    let nomination = RelationalCandidateChunkNomination::endpoint(reason, coordinate);
    nominate_lifted_coordinate(partition, coordinate, nomination, nominations);
}

fn nominate_lifted_coordinate(
    partition: &RelationalCaseChunkPartition,
    coordinate: u128,
    nomination: RelationalCandidateChunkNomination,
    nominations: &mut BTreeMap<u128, BTreeSet<RelationalCandidateChunkNomination>>,
) {
    let Some(chunk) = chunk_containing_coordinate(partition.chunks(), coordinate) else {
        return;
    };
    nominations
        .entry(chunk.descriptor().ordinal())
        .or_default()
        .insert(nomination);
}

fn chunk_containing_coordinate(
    chunks: &[RelationalCaseChunk],
    coordinate: u128,
) -> Option<&RelationalCaseChunk> {
    chunks
        .binary_search_by(|chunk| {
            let descriptor = chunk.descriptor();
            if descriptor.interval_end_exclusive() <= coordinate {
                std::cmp::Ordering::Less
            } else if descriptor.interval_start() > coordinate {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .ok()
        .map(|index| &chunks[index])
}

fn residual_target(
    partition_id: RelationalCaseChunkPartitionArtifactId,
    scheduler_policy_version: u32,
    chunk: &RelationalCaseChunk,
) -> RelationalCandidateChunkTarget {
    let descriptor = chunk.descriptor().clone();
    let nominations = vec![RelationalCandidateChunkNomination::residual()].into_boxed_slice();
    let nomination_root = derive_candidate_nomination_root(
        scheduler_policy_version,
        partition_id,
        &descriptor,
        &nominations,
    );
    RelationalCandidateChunkTarget {
        descriptor,
        nominations,
        nomination_root,
    }
}

fn derive_candidate_nomination_root(
    scheduler_policy_version: u32,
    partition_id: RelationalCaseChunkPartitionArtifactId,
    descriptor: &RelationalCaseChunkDescriptor,
    nominations: &[RelationalCandidateChunkNomination],
) -> RelationalCandidateNominationRoot {
    let mut canonical_nominations = nominations.to_vec();
    for nomination in &mut canonical_nominations {
        let mut origins = nomination.origins.to_vec();
        origins.sort();
        nomination.origins = origins.into_boxed_slice();
    }
    canonical_nominations.sort();

    let mut hasher = CandidateNominationHasher::new(CANDIDATE_NOMINATION_ROOT_V1);
    hasher.u32(RELATIONAL_CANDIDATE_NOMINATION_VERSION);
    hasher.u32(scheduler_policy_version);
    hasher.digest(partition_id.bytes());
    hasher.digest(descriptor.id().bytes());
    hasher.u128(descriptor.ordinal());
    hasher.digest(descriptor.cell_id().bytes());
    hasher.u128(descriptor.interval_start());
    hasher.u128(descriptor.interval_end_exclusive());
    hasher.u64(canonical_nominations.len() as u64);
    for nomination in &canonical_nominations {
        hasher.u8(nomination.reason.canonical_tag());
        hasher.u8(nomination.side.canonical_tag());
        hasher.optional_digest(nomination.dimension_id.map(RelationalDimensionId::bytes));
        hasher.optional_u128(nomination.split_coordinate);
        hasher.optional_i128(nomination.value_boundary);
        hasher.optional_u128(nomination.target_coordinate);
        hasher.u64(nomination.origins.len() as u64);
        for origin in &nomination.origins {
            hash_candidate_origin(&mut hasher, origin);
        }
    }
    RelationalCandidateNominationRoot(hasher.finish())
}

fn hash_candidate_origin(hasher: &mut CandidateNominationHasher, origin: &RelationalSplitOrigin) {
    match origin {
        RelationalSplitOrigin::CheckedGuard(origin) => {
            hasher.u8(0x01);
            match origin {
                RelationalGuardOrigin::Admission {
                    admission_id,
                    admission_index,
                    ast_path,
                } => {
                    hasher.u8(0x01);
                    hasher.digest(admission_id.bytes());
                    hasher.u32(*admission_index);
                    hasher.path(ast_path);
                }
                RelationalGuardOrigin::Selection {
                    question_id,
                    ast_path,
                } => {
                    hasher.u8(0x02);
                    hasher.digest(question_id.bytes());
                    hasher.path(ast_path);
                }
                RelationalGuardOrigin::FrozenClassification {
                    graph_root,
                    lane_root,
                    formula_path,
                } => {
                    hasher.u8(0x03);
                    hasher.digest(*graph_root);
                    hasher.digest(*lane_root);
                    hasher.path(formula_path);
                }
                RelationalGuardOrigin::SourceEvent {
                    inventory_root,
                    source_event_id,
                    occurrence_id,
                } => {
                    hasher.u8(0x04);
                    hasher.digest(*inventory_root);
                    hasher.digest(*source_event_id);
                    hasher.digest(*occurrence_id);
                }
            }
        }
        RelationalSplitOrigin::CertificateObligation(obligation_id) => {
            hasher.u8(0x02);
            hasher.digest(obligation_id.bytes());
        }
    }
}

struct CandidateNominationHasher(Sha256);

impl CandidateNominationHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.u64(domain.len() as u64);
        hasher.0.update(domain);
        hasher
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn u32(&mut self, value: u32) {
        self.0.update(value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.0.update(value.to_be_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.0.update(value.to_be_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn optional_digest(&mut self, value: Option<[u8; 32]>) {
        match value {
            Some(value) => {
                self.u8(0x01);
                self.digest(value);
            }
            None => self.u8(0x00),
        }
    }

    fn optional_u128(&mut self, value: Option<u128>) {
        match value {
            Some(value) => {
                self.u8(0x01);
                self.u128(value);
            }
            None => self.u8(0x00),
        }
    }

    fn optional_i128(&mut self, value: Option<i128>) {
        match value {
            Some(value) => {
                self.u8(0x01);
                self.i128(value);
            }
            None => self.u8(0x00),
        }
    }

    fn path(&mut self, path: &[u32]) {
        self.u64(path.len() as u64);
        for component in path {
            self.u32(*component);
        }
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nomination_fixture_descriptor() -> RelationalCaseChunkDescriptor {
        RelationalCaseChunkDescriptor::restore_from_canonical_parts(
            RelationalCaseChunkId::from_canonical_bytes([0x11; 32]),
            2,
            super::super::support_cell::SupportCellId::from_journal_codec_bytes([0x22; 32]),
            512,
            700,
        )
        .expect("restore nomination-root fixture descriptor")
    }

    #[test]
    fn reason_order_is_informative_then_endpoints_then_midpoint_then_residual() {
        assert!(
            RelationalCandidateScheduleReason::CheckedGuardBoundary
                < RelationalCandidateScheduleReason::SourceEvent
        );
        assert!(
            RelationalCandidateScheduleReason::SourceEvent
                < RelationalCandidateScheduleReason::LiftedCandidate
        );
        assert!(
            RelationalCandidateScheduleReason::LiftedCandidate
                < RelationalCandidateScheduleReason::CertifiedPieceBoundary
        );
        assert!(
            RelationalCandidateScheduleReason::CertifiedPieceBoundary
                < RelationalCandidateScheduleReason::LowerRangeEndpoint
        );
        assert!(
            RelationalCandidateScheduleReason::UpperRangeEndpoint
                < RelationalCandidateScheduleReason::CertificateAuthorizedMidpoint
        );
        assert!(
            RelationalCandidateScheduleReason::CertificateAuthorizedMidpoint
                < RelationalCandidateScheduleReason::ResidualFallback
        );
    }

    #[test]
    fn implicit_residual_ordinals_complete_a_nomination_without_overlap() {
        let nominated = BTreeSet::from([0u128, 3, 7]);
        let residual = (0u128..9)
            .filter(|ordinal| !nominated.contains(ordinal))
            .collect::<Vec<_>>();
        assert_eq!(residual, [1, 2, 4, 5, 6, 8]);

        let complete = nominated
            .iter()
            .copied()
            .chain(residual.iter().copied())
            .collect::<BTreeSet<_>>();
        assert_eq!(complete, (0u128..9).collect());
        assert_eq!(nominated.len() + residual.len(), complete.len());
    }

    #[test]
    fn nomination_roots_commit_every_reason_origin_subject_and_canonical_order() {
        let policy_version = 3;
        let partition_id = RelationalCaseChunkPartitionArtifactId::from_canonical_bytes([0x33; 32]);
        let descriptor = nomination_fixture_descriptor();
        let dimension = RelationalDimensionId::from_journal_codec_bytes([0x44; 32]);
        let relation = super::super::relation::RelationId::from_canonical_semantic_preimage(
            b"candidate nomination root relation",
        );
        let admission = super::super::relation::AdmissionId::from_canonical_admission_preimage(
            relation,
            b"candidate nomination root admission",
        );
        let question = super::super::relation::QuestionId::from_canonical_find_preimage(
            admission,
            b"candidate nomination root question",
            super::super::relation::FindPolarity::Matches,
        );
        let certificate =
            super::super::support_cell::SupportProofObligationId::from_journal_codec_bytes(
                [0x55; 32],
            );
        let checked_origin =
            RelationalSplitOrigin::CheckedGuard(RelationalGuardOrigin::Selection {
                question_id: question,
                ast_path: Box::new([1, 2]),
            });
        let source_origin =
            RelationalSplitOrigin::CheckedGuard(RelationalGuardOrigin::SourceEvent {
                inventory_root: [0x65; 32],
                source_event_id: [0x66; 32],
                occurrence_id: [0x67; 32],
            });
        let lifted_origin =
            RelationalSplitOrigin::CheckedGuard(RelationalGuardOrigin::FrozenClassification {
                graph_root: [0x77; 32],
                lane_root: [0x88; 32],
                formula_path: Box::new([5, 6]),
            });
        let certificate_origin = RelationalSplitOrigin::CertificateObligation(certificate);

        let lifted_nomination = |reason, origin: &RelationalSplitOrigin| {
            RelationalCandidateChunkNomination::lifted(
                reason,
                RelationalCandidateBoundarySide::UpperAdjacent,
                dimension,
                680,
                680,
                680,
                std::slice::from_ref(origin),
            )
        };
        let root = |nomination: RelationalCandidateChunkNomination| {
            derive_candidate_nomination_root(
                policy_version,
                partition_id,
                &descriptor,
                &[nomination],
            )
        };
        let roots = [
            root(lifted_nomination(
                RelationalCandidateScheduleReason::CheckedGuardBoundary,
                &checked_origin,
            )),
            root(lifted_nomination(
                RelationalCandidateScheduleReason::SourceEvent,
                &source_origin,
            )),
            root(lifted_nomination(
                RelationalCandidateScheduleReason::LiftedCandidate,
                &lifted_origin,
            )),
            root(lifted_nomination(
                RelationalCandidateScheduleReason::CertifiedPieceBoundary,
                &certificate_origin,
            )),
            root(RelationalCandidateChunkNomination::endpoint(
                RelationalCandidateScheduleReason::LowerRangeEndpoint,
                512,
            )),
            root(RelationalCandidateChunkNomination::endpoint(
                RelationalCandidateScheduleReason::UpperRangeEndpoint,
                699,
            )),
            root(lifted_nomination(
                RelationalCandidateScheduleReason::CertificateAuthorizedMidpoint,
                &certificate_origin,
            )),
            root(RelationalCandidateChunkNomination::residual()),
        ];
        assert_eq!(
            roots.iter().copied().collect::<BTreeSet<_>>().len(),
            roots.len()
        );

        let checked_nomination = lifted_nomination(
            RelationalCandidateScheduleReason::CheckedGuardBoundary,
            &checked_origin,
        );
        let source_nomination = lifted_nomination(
            RelationalCandidateScheduleReason::SourceEvent,
            &source_origin,
        );
        let forward = derive_candidate_nomination_root(
            policy_version,
            partition_id,
            &descriptor,
            &[checked_nomination.clone(), source_nomination.clone()],
        );
        let reversed = derive_candidate_nomination_root(
            policy_version,
            partition_id,
            &descriptor,
            &[source_nomination.clone(), checked_nomination.clone()],
        );
        assert_eq!(forward, reversed, "nomination order must be canonical");
        let origin_forward = RelationalCandidateChunkNomination::lifted(
            RelationalCandidateScheduleReason::SourceEvent,
            RelationalCandidateBoundarySide::UpperAdjacent,
            dimension,
            680,
            680,
            680,
            &[checked_origin.clone(), source_origin.clone()],
        );
        let origin_reversed = RelationalCandidateChunkNomination::lifted(
            RelationalCandidateScheduleReason::SourceEvent,
            RelationalCandidateBoundarySide::UpperAdjacent,
            dimension,
            680,
            680,
            680,
            &[source_origin, checked_origin.clone()],
        );
        assert_eq!(
            derive_candidate_nomination_root(
                policy_version,
                partition_id,
                &descriptor,
                &[origin_forward],
            ),
            derive_candidate_nomination_root(
                policy_version,
                partition_id,
                &descriptor,
                &[origin_reversed],
            ),
            "origin order must be canonical"
        );
        assert_ne!(
            derive_candidate_nomination_root(
                policy_version,
                partition_id,
                &descriptor,
                &[checked_nomination.clone(), source_nomination.clone()],
            ),
            derive_candidate_nomination_root(
                policy_version + 1,
                partition_id,
                &descriptor,
                &[checked_nomination, source_nomination],
            ),
            "the root must commit scheduler policy"
        );
        assert_ne!(
            roots[2],
            derive_candidate_nomination_root(
                policy_version,
                RelationalCaseChunkPartitionArtifactId::from_canonical_bytes([0x99; 32]),
                &descriptor,
                &[lifted_nomination(
                    RelationalCandidateScheduleReason::LiftedCandidate,
                    &lifted_origin,
                )],
            ),
            "the root must commit its partition subject"
        );
    }
}
