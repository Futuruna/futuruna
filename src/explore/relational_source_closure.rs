//! Exact aggregate closure evidence for concrete source-relation traversal.
//!
//! A per-fiber exhaustion receipt proves that the checked source executor
//! reached the end of one binding fiber. It does not, by itself, prove that a
//! scheduler discovered every dependent child fiber. This module closes that
//! gap by recording the executor's actual yielded continuations and checking
//! the resulting dependent-product tree from the unique empty root.
//!
//! The accumulator is intentionally arrival-order independent and replay
//! idempotent. Only [`SourceTraversalAccumulator::finish`] can mint a compact
//! [`SourceRelationExhaustionReceipt`]. A cursor or a caller-supplied count is
//! never completion authority.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::relation::{
    canonical_source_key_set_commitment, RelationId, RelationProvenance, SourceKey,
    SourceKeySetRoot,
};
use super::relational_executor::{
    RelationalBindingSelection, RelationalFiberMember, RelationalSourceAdvance,
    RelationalSourceContinuation, RelationalSourceCursor, RelationalSourcePrefixSnapshot,
    SourceBindingExhaustionReceipt, SourceBindingExhaustionReceiptId,
    RELATIONAL_SOURCE_CURSOR_VERSION, SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION,
};
use super::relational_frontier::CanonicalSourcePrefix;
use super::relational_support_planner::{RelationalSupportPlan, RelationalSupportPlanRoot};
use super::transition::canonical_explore_value_digest;
use super::ExploreValue;

pub(crate) const RELATIONAL_SOURCE_CLOSURE_SCHEMA_VERSION: u32 = 2;
pub(crate) const RELATIONAL_SOURCE_CLOSURE_PRODUCER_ABI_VERSION: u32 = 2;

const SOURCE_TRAVERSAL_EDGE_ID_V2: &[u8] = b"futuruna.explore.source-traversal.edge-id.v2";
const SOURCE_TRAVERSAL_EDGE_ROOT_V2: &[u8] = b"futuruna.explore.source-traversal.edge-root.v2";
const SOURCE_TRAVERSAL_FRONTIER_ROOT_V2: &[u8] =
    b"futuruna.explore.source-traversal.frontier-root.v2";
const SOURCE_TRAVERSAL_ADVANCE_ID_V2: &[u8] = b"futuruna.explore.source-traversal.advance-id.v2";
const SOURCE_FIBER_RECEIPT_SET_ROOT_V2: &[u8] =
    b"futuruna.explore.source-fiber-receipt-set-root.v2";
const SOURCE_RELATION_EXHAUSTION_RECEIPT_ID_V2: &[u8] =
    b"futuruna.explore.source-relation-exhaustion-receipt.v2";

/// Content identity of one observed canonical source-traversal edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceTraversalEdgeId([u8; 32]);

impl SourceTraversalEdgeId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content root of the canonical set of source-traversal edges.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceTraversalEdgeRoot([u8; 32]);

impl SourceTraversalEdgeRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content root of the canonical set of accepted source-fiber exhaustion
/// receipt identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceFiberReceiptSetRoot([u8; 32]);

impl SourceFiberReceiptSetRoot {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent commitment to an open or closed concrete source
/// traversal prefix. Unlike the aggregate exhaustion receipt, this root is
/// available before the dependent-product tree has closed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceTraversalFrontierRoot([u8; 32]);

impl SourceTraversalFrontierRoot {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Arrival-order-independent identity of one complete checked source advance.
///
/// This is the direct journal event commitment: it includes cursor and prefix
/// support semantics, yielded-member support coordinates, terminal source
/// provenance, or the full exhaustion receipt, as applicable. A yielded
/// [`SourceTraversalEdgeId`] additionally binds this identity to its checked
/// topological subject and target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceTraversalAdvanceId([u8; 32]);

impl SourceTraversalAdvanceId {
    pub(super) const fn from_journal_codec_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Derive the journal-stable identity of one source advance without
    /// consulting mutable traversal state. Replay must still pass the claimed
    /// identity through [`SourceTraversalAccumulator::prepare_claimed_observation`]
    /// before committing it.
    pub(crate) fn derive(
        relation_id: RelationId,
        support_plan_root: RelationalSupportPlanRoot,
        advance: &RelationalSourceAdvance,
    ) -> Self {
        derive_advance_id(relation_id, support_plan_root, advance)
    }

    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Content identity of one verified aggregate source-exhaustion receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceRelationExhaustionReceiptId([u8; 32]);

impl SourceRelationExhaustionReceiptId {
    pub(crate) const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct SourceFiberKey {
    binding_index: u32,
    prefix_digest: [u8; 32],
}

impl SourceFiberKey {
    fn new(binding_index: u32, prefix_digest: [u8; 32]) -> Self {
        Self {
            binding_index,
            prefix_digest,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct SourceTraversalEdgeKey {
    fiber: SourceFiberKey,
    canonical_ordinal: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceTraversalTarget {
    ChildFiber {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    Source {
        source_key: SourceKey,
        terminal_prefix_digest: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceTraversalEdge {
    id: SourceTraversalEdgeId,
    advance_id: SourceTraversalAdvanceId,
    key: SourceTraversalEdgeKey,
    target: SourceTraversalTarget,
}

impl SourceTraversalEdge {
    fn new(
        relation_id: RelationId,
        advance_id: SourceTraversalAdvanceId,
        key: SourceTraversalEdgeKey,
        target: SourceTraversalTarget,
    ) -> Self {
        let id = derive_edge_id(relation_id, advance_id, key, &target);
        Self {
            id,
            advance_id,
            key,
            target,
        }
    }
}

/// Whether observing one executor advance added new aggregate evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceTraversalObservation {
    Yielded {
        edge_id: SourceTraversalEdgeId,
        inserted: bool,
    },
    Exhausted {
        receipt_id: SourceBindingExhaustionReceiptId,
        inserted: bool,
    },
}

/// A fully checked source-traversal mutation holding an exclusive borrow of
/// its accumulator until it is either committed or abandoned.
///
/// Preparation performs every semantic check and builds all owned values that
/// commit needs. Because this guard owns the accumulator's mutable borrow, no
/// intervening traversal observation can invalidate that preflight. Dropping
/// the guard is a rollback; [`Self::commit`] is logically infallible and does
/// not clone the accumulator.
#[derive(Debug)]
#[must_use = "a prepared source traversal observation must be committed or explicitly dropped"]
pub(crate) struct PreparedSourceTraversalObservation<'a> {
    accumulator: &'a mut SourceTraversalAccumulator,
    advance_id: SourceTraversalAdvanceId,
    observation: SourceTraversalObservation,
    mutation: Option<PreparedSourceTraversalMutation>,
}

impl PreparedSourceTraversalObservation<'_> {
    /// Stable content identity for journal hashing and replay deduplication.
    pub(crate) const fn advance_id(&self) -> SourceTraversalAdvanceId {
        self.advance_id
    }

    /// Result that committing this already checked mutation will produce.
    pub(crate) const fn observation(&self) -> SourceTraversalObservation {
        self.observation
    }

    /// Commit the prepared mutation. All logical failure paths occurred during
    /// preparation, while the exclusive borrow prevents stale preflight data.
    pub(crate) fn commit(self) -> SourceTraversalObservation {
        let Self {
            accumulator,
            advance_id: _,
            observation,
            mutation,
        } = self;
        if let Some(mutation) = mutation {
            accumulator.commit_mutation(mutation);
        }
        observation
    }
}

#[derive(Debug)]
enum PreparedSourceTraversalMutation {
    Yielded {
        parent: SourceFiberKey,
        parent_prefix: CanonicalSourcePrefix,
        child_claim: Option<(SourceFiberKey, CanonicalSourcePrefix)>,
        source_claim: Option<(SourceKey, SourceContentClaim)>,
        edge: SourceTraversalEdge,
    },
    Exhausted {
        subject: SourceFiberKey,
        prefix: CanonicalSourcePrefix,
        receipt: SourceBindingExhaustionReceipt,
    },
}

/// Arrival-order-independent concrete traversal proof under construction.
///
/// Construction starts from a validated logical support plan so the relation,
/// binding count, projection roles, and support-plan root cannot be combined
/// independently by the caller.
#[derive(Clone, Debug)]
pub(crate) struct SourceTraversalAccumulator {
    relation_id: RelationId,
    support_plan_root: RelationalSupportPlanRoot,
    binding_count: u32,
    context_binding_index: u32,
    before_binding_index: u32,
    prefixes: BTreeMap<SourceFiberKey, CanonicalSourcePrefix>,
    receipts: BTreeMap<SourceFiberKey, SourceBindingExhaustionReceipt>,
    receipt_subjects: BTreeMap<SourceBindingExhaustionReceiptId, SourceFiberKey>,
    edges: BTreeMap<SourceTraversalEdgeKey, SourceTraversalEdge>,
    edge_subjects: BTreeMap<SourceTraversalEdgeId, SourceTraversalEdgeKey>,
    child_parents: BTreeMap<SourceFiberKey, SourceTraversalEdgeKey>,
    source_claims: BTreeMap<SourceKey, SourceContentClaim>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceContentClaim {
    context: ExploreValue,
    before: ExploreValue,
}

impl SourceTraversalAccumulator {
    /// Start a concrete closure proof for one validated support plan.
    pub(crate) fn for_plan(
        plan: &RelationalSupportPlan,
    ) -> Result<Self, SourceTraversalClosureError> {
        if !plan.validate_root() {
            return Err(SourceTraversalClosureError::InvalidSupportPlanRoot);
        }
        if plan.stages().is_empty() {
            return Err(SourceTraversalClosureError::EmptyBindingPlan);
        }
        let binding_count = u32::try_from(plan.stages().len())
            .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;

        let mut context_binding_index = None;
        let mut before_binding_index = None;
        for (expected, stage) in plan.stages().iter().enumerate() {
            let expected = u32::try_from(expected)
                .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
            if stage.binding_index() != expected {
                return Err(SourceTraversalClosureError::NonCanonicalBindingStage {
                    actual: stage.binding_index(),
                    expected,
                });
            }
            match stage.role() {
                super::relational_ir::ExploreSourceBindingRoleIr::Auxiliary => {}
                super::relational_ir::ExploreSourceBindingRoleIr::Context => {
                    if context_binding_index.replace(expected).is_some() {
                        return Err(SourceTraversalClosureError::DuplicateContextBinding);
                    }
                }
                super::relational_ir::ExploreSourceBindingRoleIr::Before => {
                    if before_binding_index.replace(expected).is_some() {
                        return Err(SourceTraversalClosureError::DuplicateBeforeBinding);
                    }
                }
            }
        }

        Ok(Self {
            relation_id: plan.relation_id(),
            support_plan_root: plan.root(),
            binding_count,
            context_binding_index: context_binding_index
                .ok_or(SourceTraversalClosureError::MissingContextBinding)?,
            before_binding_index: before_binding_index
                .ok_or(SourceTraversalClosureError::MissingBeforeBinding)?,
            prefixes: BTreeMap::new(),
            receipts: BTreeMap::new(),
            receipt_subjects: BTreeMap::new(),
            edges: BTreeMap::new(),
            edge_subjects: BTreeMap::new(),
            child_parents: BTreeMap::new(),
            source_claims: BTreeMap::new(),
        })
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn binding_count(&self) -> u32 {
        self.binding_count
    }

    /// Whether at least one producer advance has been accepted.
    ///
    /// Registering a support plan allocates this accumulator before the root
    /// work node is seeded, so accumulator presence alone cannot mean source
    /// traversal has begun. Every accepted yielded/exhausted advance installs
    /// its canonical parent prefix, making this the minimal exact distinction.
    pub(crate) fn has_observations(&self) -> bool {
        !self.prefixes.is_empty()
    }

    /// Look up one already accepted producer receipt by its content identity.
    /// This supports work-completion validation without duplicating receipt
    /// state in the outer journal.
    pub(crate) fn fiber_receipt(
        &self,
        receipt_id: SourceBindingExhaustionReceiptId,
    ) -> Option<&SourceBindingExhaustionReceipt> {
        self.receipt_subjects
            .get(&receipt_id)
            .and_then(|subject| self.receipts.get(subject))
    }

    /// Commit every accepted traversal fact without depending on arrival
    /// order. This is prefix evidence, not a claim of exhaustion.
    pub(crate) fn frontier_root(&self) -> SourceTraversalFrontierRoot {
        let mut hasher = SourceClosureHasher::new(SOURCE_TRAVERSAL_FRONTIER_ROOT_V2);
        hash_versions(&mut hasher);
        hasher.digest(self.relation_id.bytes());
        hasher.digest(self.support_plan_root.bytes());
        hasher.u32(self.binding_count);

        hasher.u128(self.prefixes.len() as u128);
        for fiber in self.prefixes.keys() {
            hasher.u32(fiber.binding_index);
            hasher.digest(fiber.prefix_digest);
        }
        hasher.u128(self.receipt_subjects.len() as u128);
        for receipt_id in self.receipt_subjects.keys() {
            hasher.digest(receipt_id.bytes());
        }
        hasher.u128(self.edge_subjects.len() as u128);
        for edge_id in self.edge_subjects.keys() {
            hasher.digest(edge_id.bytes());
        }
        hasher.u128(self.source_claims.len() as u128);
        for source_key in self.source_claims.keys() {
            hasher.digest(source_key.bytes());
        }
        SourceTraversalFrontierRoot(hasher.finish())
    }

    /// Preflight one checked executor result without mutating the accumulator.
    ///
    /// The returned guard holds this accumulator's exclusive borrow. A caller
    /// may therefore preflight coordinated mutations in other journal fields
    /// before calling [`PreparedSourceTraversalObservation::commit`], without
    /// cloning this potentially large accumulator and without allowing a stale
    /// prepared mutation to be committed later.
    pub(crate) fn prepare_observation<'a>(
        &'a mut self,
        advance: &RelationalSourceAdvance,
    ) -> Result<PreparedSourceTraversalObservation<'a>, SourceTraversalClosureError> {
        self.prepare_observation_inner(None, advance)
    }

    /// Recompute and validate a journal event's claimed content identity while
    /// preparing its mutation. A mismatched claim leaves the accumulator
    /// untouched.
    pub(crate) fn prepare_claimed_observation<'a>(
        &'a mut self,
        claimed_advance_id: SourceTraversalAdvanceId,
        advance: &RelationalSourceAdvance,
    ) -> Result<PreparedSourceTraversalObservation<'a>, SourceTraversalClosureError> {
        self.prepare_observation_inner(Some(claimed_advance_id), advance)
    }

    fn prepare_observation_inner<'a>(
        &'a mut self,
        claimed_advance_id: Option<SourceTraversalAdvanceId>,
        advance: &RelationalSourceAdvance,
    ) -> Result<PreparedSourceTraversalObservation<'a>, SourceTraversalClosureError> {
        let advance_id =
            SourceTraversalAdvanceId::derive(self.relation_id, self.support_plan_root, advance);
        if let Some(claimed) = claimed_advance_id {
            if claimed != advance_id {
                return Err(SourceTraversalClosureError::AdvanceIdMismatch {
                    claimed,
                    derived: advance_id,
                });
            }
        }
        let (observation, mutation) = match advance {
            RelationalSourceAdvance::Yielded { .. } => {
                let (edge_id, inserted, mutation) = self.prepare_yielded(advance, advance_id)?;
                (
                    SourceTraversalObservation::Yielded { edge_id, inserted },
                    mutation,
                )
            }
            RelationalSourceAdvance::Exhausted {
                cursor,
                cardinality,
                receipt,
            } => {
                if cursor.binding_index() != receipt.binding_index()
                    || cursor.canonical_prefix().digest() != receipt.prefix_digest()
                    || cursor.next_member_ordinal() != receipt.terminal_ordinal()
                    || *cardinality != receipt.emitted_member_count()
                {
                    return Err(SourceTraversalClosureError::ExhaustedAdvanceMismatch);
                }
                let (inserted, mutation) =
                    self.prepare_receipt(cursor.canonical_prefix(), receipt.clone())?;
                (
                    SourceTraversalObservation::Exhausted {
                        receipt_id: receipt.id(),
                        inserted,
                    },
                    mutation,
                )
            }
        };
        Ok(PreparedSourceTraversalObservation {
            accumulator: self,
            advance_id,
            observation,
            mutation,
        })
    }

    /// Observe one checked executor result in a single operation. Replaying an
    /// identical result is idempotent; conflicting evidence for the same
    /// semantic edge or fiber is rejected before mutation.
    pub(crate) fn observe(
        &mut self,
        advance: &RelationalSourceAdvance,
    ) -> Result<SourceTraversalObservation, SourceTraversalClosureError> {
        Ok(self.prepare_observation(advance)?.commit())
    }

    /// Accept one executor-issued exact fiber receipt together with the
    /// canonical prefix whose work item produced it.
    pub(crate) fn accept_receipt(
        &mut self,
        prefix: &CanonicalSourcePrefix,
        receipt: SourceBindingExhaustionReceipt,
    ) -> Result<bool, SourceTraversalClosureError> {
        let (inserted, mutation) = self.prepare_receipt(prefix, receipt)?;
        if let Some(mutation) = mutation {
            self.commit_mutation(mutation);
        }
        Ok(inserted)
    }

    fn prepare_receipt(
        &self,
        prefix: &CanonicalSourcePrefix,
        receipt: SourceBindingExhaustionReceipt,
    ) -> Result<(bool, Option<PreparedSourceTraversalMutation>), SourceTraversalClosureError> {
        receipt
            .validate_identity()
            .map_err(|error| SourceTraversalClosureError::InvalidFiberReceipt(error.to_string()))?;
        if receipt.relation_id() != self.relation_id {
            return Err(SourceTraversalClosureError::ReceiptRelationMismatch);
        }
        if receipt.binding_index() >= self.binding_count {
            return Err(SourceTraversalClosureError::BindingIndexOutOfBounds {
                binding_index: receipt.binding_index(),
                binding_count: self.binding_count,
            });
        }
        validate_prefix(prefix)?;
        let prefix_len = u32::try_from(prefix.values().len())
            .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
        if prefix_len != receipt.binding_index()
            || prefix.digest() != receipt.prefix_digest()
            || receipt.terminal_ordinal() != receipt.emitted_member_count()
        {
            return Err(SourceTraversalClosureError::ReceiptSubjectMismatch);
        }

        let subject = SourceFiberKey::new(receipt.binding_index(), receipt.prefix_digest());
        self.preflight_prefix(subject, prefix)?;
        if let Some(existing) = self.receipts.get(&subject) {
            if existing == &receipt {
                return Ok((false, None));
            }
            return Err(SourceTraversalClosureError::ConflictingFiberReceipt {
                binding_index: subject.binding_index,
                prefix_digest: subject.prefix_digest,
            });
        }
        if let Some(existing_subject) = self.receipt_subjects.get(&receipt.id()) {
            if *existing_subject != subject {
                return Err(SourceTraversalClosureError::FiberReceiptIdCollision {
                    receipt_id: receipt.id(),
                });
            }
        }
        let mut observed_count = 0u128;
        let mut last_ordinal = None;
        for (edge_key, _) in self.edges_for(subject) {
            observed_count += 1;
            last_ordinal = Some(edge_key.canonical_ordinal);
        }
        if observed_count > receipt.emitted_member_count()
            || last_ordinal.is_some_and(|ordinal| ordinal >= receipt.emitted_member_count())
        {
            return Err(SourceTraversalClosureError::ReceiptRejectsObservedEdges {
                binding_index: subject.binding_index,
                prefix_digest: subject.prefix_digest,
                emitted_member_count: receipt.emitted_member_count(),
                observed_count,
            });
        }

        Ok((
            true,
            Some(PreparedSourceTraversalMutation::Exhausted {
                subject,
                prefix: prefix.clone(),
                receipt,
            }),
        ))
    }

    fn prepare_yielded(
        &self,
        advance: &RelationalSourceAdvance,
        advance_id: SourceTraversalAdvanceId,
    ) -> Result<
        (
            SourceTraversalEdgeId,
            bool,
            Option<PreparedSourceTraversalMutation>,
        ),
        SourceTraversalClosureError,
    > {
        let RelationalSourceAdvance::Yielded {
            member,
            resume,
            continuation,
        } = advance
        else {
            return Err(SourceTraversalClosureError::ExpectedYieldedAdvance);
        };

        let binding_index = resume.binding_index();
        if binding_index >= self.binding_count {
            return Err(SourceTraversalClosureError::BindingIndexOutOfBounds {
                binding_index,
                binding_count: self.binding_count,
            });
        }
        let parent_prefix = resume.canonical_prefix();
        validate_prefix(parent_prefix)?;
        let prefix_len = u32::try_from(parent_prefix.values().len())
            .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
        if prefix_len != binding_index {
            return Err(SourceTraversalClosureError::CursorPrefixLengthMismatch {
                binding_index,
                prefix_len,
            });
        }
        let canonical_ordinal = resume
            .next_member_ordinal()
            .checked_sub(1)
            .ok_or(SourceTraversalClosureError::YieldResumeDidNotAdvance)?;
        if member.canonical_ordinal() != canonical_ordinal {
            return Err(SourceTraversalClosureError::YieldOrdinalMismatch {
                member: member.canonical_ordinal(),
                resume: canonical_ordinal,
            });
        }

        let parent = SourceFiberKey::new(binding_index, parent_prefix.digest());
        let key = SourceTraversalEdgeKey {
            fiber: parent,
            canonical_ordinal,
        };
        let mut child_values = parent_prefix.values().to_vec();
        child_values.push(member.value().clone());
        let expected_child_prefix = CanonicalSourcePrefix::from_values(child_values)
            .map_err(|error| SourceTraversalClosureError::InvalidPrefix(error.to_string()))?;
        let next_binding_index = binding_index
            .checked_add(1)
            .ok_or(SourceTraversalClosureError::BindingCountOverflow)?;

        let (target, child_claim, source_claim) = match continuation {
            RelationalSourceContinuation::Expand(child) => {
                if next_binding_index >= self.binding_count {
                    return Err(SourceTraversalClosureError::ExpectedSourceContinuation {
                        binding_index,
                    });
                }
                if child.binding_index() != next_binding_index
                    || child.next_member_ordinal() != 0
                    || child.canonical_prefix() != &expected_child_prefix
                {
                    return Err(SourceTraversalClosureError::ChildContinuationMismatch {
                        binding_index,
                        canonical_ordinal,
                    });
                }
                let child_key =
                    SourceFiberKey::new(next_binding_index, expected_child_prefix.digest());
                (
                    SourceTraversalTarget::ChildFiber {
                        binding_index: next_binding_index,
                        prefix_digest: expected_child_prefix.digest(),
                    },
                    Some((child_key, expected_child_prefix)),
                    None,
                )
            }
            RelationalSourceContinuation::Source(source) => {
                if next_binding_index != self.binding_count {
                    return Err(SourceTraversalClosureError::ExpectedChildContinuation {
                        binding_index,
                    });
                }
                self.validate_completed_source(source, &expected_child_prefix)?;
                let source_key = source.source_key();
                let claim = SourceContentClaim {
                    context: source.row().context().clone(),
                    before: source.row().before().clone(),
                };
                (
                    SourceTraversalTarget::Source {
                        source_key,
                        terminal_prefix_digest: expected_child_prefix.digest(),
                    },
                    None,
                    Some((source_key, claim)),
                )
            }
        };
        let edge = SourceTraversalEdge::new(self.relation_id, advance_id, key, target);

        self.preflight_prefix(parent, parent_prefix)?;
        if let Some((child_key, child_prefix)) = &child_claim {
            self.preflight_prefix(*child_key, child_prefix)?;
            if let Some(existing_parent) = self.child_parents.get(child_key) {
                if *existing_parent != key {
                    return Err(SourceTraversalClosureError::ChildHasMultipleParents {
                        binding_index: child_key.binding_index,
                        prefix_digest: child_key.prefix_digest,
                    });
                }
            }
        }
        if let Some((source_key, claim)) = &source_claim {
            if let Some(existing) = self.source_claims.get(source_key) {
                if existing != claim {
                    return Err(SourceTraversalClosureError::SourceKeyCollision {
                        source_key: *source_key,
                    });
                }
            }
        }
        if let Some(existing) = self.edges.get(&key) {
            if existing == &edge {
                return Ok((edge.id, false, None));
            }
            return Err(SourceTraversalClosureError::ConflictingTraversalEdge {
                binding_index,
                prefix_digest: parent.prefix_digest,
                canonical_ordinal,
            });
        }
        if let Some(existing_key) = self.edge_subjects.get(&edge.id) {
            if *existing_key != key {
                return Err(SourceTraversalClosureError::TraversalEdgeIdCollision {
                    edge_id: edge.id,
                });
            }
        }
        if let Some(receipt) = self.receipts.get(&parent) {
            if canonical_ordinal >= receipt.emitted_member_count() {
                return Err(SourceTraversalClosureError::EdgeBeyondReceipt {
                    binding_index,
                    prefix_digest: parent.prefix_digest,
                    canonical_ordinal,
                    emitted_member_count: receipt.emitted_member_count(),
                });
            }
        }

        let edge_id = edge.id;
        Ok((
            edge_id,
            true,
            Some(PreparedSourceTraversalMutation::Yielded {
                parent,
                parent_prefix: parent_prefix.clone(),
                child_claim,
                source_claim,
                edge,
            }),
        ))
    }

    fn commit_mutation(&mut self, mutation: PreparedSourceTraversalMutation) {
        match mutation {
            PreparedSourceTraversalMutation::Yielded {
                parent,
                parent_prefix,
                child_claim,
                source_claim,
                edge,
            } => {
                let key = edge.key;
                let edge_id = edge.id;
                self.prefixes.entry(parent).or_insert(parent_prefix);
                if let Some((child_key, child_prefix)) = child_claim {
                    self.prefixes.entry(child_key).or_insert(child_prefix);
                    self.child_parents.entry(child_key).or_insert(key);
                }
                if let Some((source_key, claim)) = source_claim {
                    self.source_claims.entry(source_key).or_insert(claim);
                }
                self.edge_subjects.insert(edge_id, key);
                self.edges.insert(key, edge);
            }
            PreparedSourceTraversalMutation::Exhausted {
                subject,
                prefix,
                receipt,
            } => {
                let receipt_id = receipt.id();
                self.prefixes.entry(subject).or_insert(prefix);
                self.receipt_subjects.insert(receipt_id, subject);
                self.receipts.insert(subject, receipt);
            }
        }
    }

    fn validate_completed_source(
        &self,
        source: &super::relational_executor::RelationalCompletedSource,
        expected_prefix: &CanonicalSourcePrefix,
    ) -> Result<(), SourceTraversalClosureError> {
        let snapshot = source.prefix();
        if snapshot.version != RELATIONAL_SOURCE_CURSOR_VERSION {
            return Err(
                SourceTraversalClosureError::CompletedPrefixVersionMismatch {
                    actual: snapshot.version,
                    expected: RELATIONAL_SOURCE_CURSOR_VERSION,
                },
            );
        }
        let reconstructed = CanonicalSourcePrefix::from_values(snapshot.values.to_vec())
            .map_err(|error| SourceTraversalClosureError::InvalidPrefix(error.to_string()))?;
        if reconstructed.digest() != snapshot.digest
            || &reconstructed != expected_prefix
            || snapshot.selections.len() != snapshot.values.len()
        {
            return Err(SourceTraversalClosureError::CompletedPrefixMismatch);
        }
        for (index, selection) in snapshot.selections.iter().enumerate() {
            let expected_index = u32::try_from(index)
                .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
            let parent = CanonicalSourcePrefix::from_values(snapshot.values[..index].to_vec())
                .map_err(|error| SourceTraversalClosureError::InvalidPrefix(error.to_string()))?;
            if selection.binding_index != expected_index
                || selection.parent_prefix_digest != parent.digest()
                || selection.raw_support_ordinals.is_empty()
                || selection
                    .raw_support_ordinals
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(SourceTraversalClosureError::CompletedPrefixMismatch);
            }
        }

        let context_index = usize::try_from(self.context_binding_index)
            .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
        let before_index = usize::try_from(self.before_binding_index)
            .map_err(|_| SourceTraversalClosureError::BindingCountOverflow)?;
        if snapshot.values.get(context_index) != Some(source.row().context())
            || snapshot.values.get(before_index) != Some(source.row().before())
        {
            return Err(SourceTraversalClosureError::SourceProjectionMismatch);
        }
        let derived = SourceKey::derive(self.relation_id, source.row());
        if derived != source.source_key() {
            return Err(SourceTraversalClosureError::SourceKeyMismatch {
                claimed: source.source_key(),
                derived,
            });
        }
        Ok(())
    }

    fn preflight_prefix(
        &self,
        subject: SourceFiberKey,
        prefix: &CanonicalSourcePrefix,
    ) -> Result<(), SourceTraversalClosureError> {
        if let Some(existing) = self.prefixes.get(&subject) {
            if existing != prefix {
                return Err(SourceTraversalClosureError::PrefixDigestCollision {
                    binding_index: subject.binding_index,
                    prefix_digest: subject.prefix_digest,
                });
            }
        }
        Ok(())
    }

    fn edges_for(
        &self,
        subject: SourceFiberKey,
    ) -> impl DoubleEndedIterator<Item = (&SourceTraversalEdgeKey, &SourceTraversalEdge)> {
        self.edges.range(
            SourceTraversalEdgeKey {
                fiber: subject,
                canonical_ordinal: 0,
            }..=SourceTraversalEdgeKey {
                fiber: subject,
                canonical_ordinal: u128::MAX,
            },
        )
    }

    /// Verify the complete dependent-product traversal and mint its compact
    /// aggregate exhaustion receipt. The large prefix, edge, and source-value
    /// indexes stay borrowed in place; closure scratch space contains only
    /// compact canonical identities.
    pub(crate) fn finish(
        &self,
    ) -> Result<SourceRelationExhaustionReceipt, SourceTraversalClosureError> {
        let root_prefix = CanonicalSourcePrefix::empty();
        let root = SourceFiberKey::new(0, root_prefix.digest());
        match self.prefixes.get(&root) {
            Some(prefix) if prefix == &root_prefix => {}
            Some(_) => return Err(SourceTraversalClosureError::NonCanonicalRootPrefix),
            None => return Err(SourceTraversalClosureError::MissingRootFiber),
        }
        if self.child_parents.contains_key(&root) {
            return Err(SourceTraversalClosureError::RootHasParent);
        }

        let mut frontier = VecDeque::from([root]);
        let mut visited = BTreeSet::new();
        let mut reached_sources = BTreeSet::new();
        let mut reached_edge_ids = BTreeSet::new();

        while let Some(subject) = frontier.pop_front() {
            if !visited.insert(subject) {
                continue;
            }
            let receipt = self.receipts.get(&subject).ok_or(
                SourceTraversalClosureError::MissingReachableFiberReceipt {
                    binding_index: subject.binding_index,
                    prefix_digest: subject.prefix_digest,
                },
            )?;
            receipt.validate_identity().map_err(|error| {
                SourceTraversalClosureError::InvalidFiberReceipt(error.to_string())
            })?;
            if receipt.relation_id() != self.relation_id
                || receipt.binding_index() != subject.binding_index
                || receipt.prefix_digest() != subject.prefix_digest
                || self.receipt_subjects.get(&receipt.id()) != Some(&subject)
            {
                return Err(SourceTraversalClosureError::ReceiptSubjectMismatch);
            }
            let observed_count = self.edges_for(subject).count() as u128;
            if observed_count != receipt.emitted_member_count() {
                return Err(SourceTraversalClosureError::FiberEdgeCountMismatch {
                    binding_index: subject.binding_index,
                    prefix_digest: subject.prefix_digest,
                    expected: receipt.emitted_member_count(),
                    actual: observed_count,
                });
            }
            for (expected_ordinal, (edge_key, edge)) in (0u128..).zip(self.edges_for(subject)) {
                let ordinal = edge_key.canonical_ordinal;
                if ordinal != expected_ordinal {
                    return Err(SourceTraversalClosureError::NonContiguousFiberOrdinal {
                        binding_index: subject.binding_index,
                        prefix_digest: subject.prefix_digest,
                        expected: expected_ordinal,
                        actual: ordinal,
                    });
                }
                if edge.key.fiber != subject || edge.key.canonical_ordinal != ordinal {
                    return Err(SourceTraversalClosureError::CorruptTraversalEdge);
                }
                let derived =
                    derive_edge_id(self.relation_id, edge.advance_id, edge.key, &edge.target);
                if derived != edge.id || !reached_edge_ids.insert(edge.id) {
                    return Err(SourceTraversalClosureError::CorruptTraversalEdge);
                }
                match edge.target {
                    SourceTraversalTarget::ChildFiber {
                        binding_index,
                        prefix_digest,
                    } => {
                        let child = SourceFiberKey::new(binding_index, prefix_digest);
                        if binding_index != subject.binding_index + 1
                            || binding_index >= self.binding_count
                        {
                            return Err(SourceTraversalClosureError::CorruptTraversalEdge);
                        }
                        if self.child_parents.get(&child) != Some(&edge.key)
                            || !self.prefixes.contains_key(&child)
                        {
                            return Err(SourceTraversalClosureError::MissingReachableChild {
                                binding_index,
                                prefix_digest,
                            });
                        }
                        frontier.push_back(child);
                    }
                    SourceTraversalTarget::Source { source_key, .. } => {
                        if subject.binding_index + 1 != self.binding_count
                            || !self.source_claims.contains_key(&source_key)
                        {
                            return Err(SourceTraversalClosureError::CorruptTraversalEdge);
                        }
                        reached_sources.insert(source_key);
                    }
                }
            }
        }

        if visited.len() != self.prefixes.len()
            || visited.len() != self.receipts.len()
            || !visited.iter().eq(self.prefixes.keys())
            || !visited.iter().eq(self.receipts.keys())
        {
            return Err(SourceTraversalClosureError::OrphanFiberEvidence {
                reachable: visited.len() as u128,
                known_prefixes: self.prefixes.len() as u128,
                receipts: self.receipts.len() as u128,
            });
        }
        if self.child_parents.len() + 1 != visited.len()
            || !visited
                .iter()
                .filter(|fiber| **fiber != root)
                .eq(self.child_parents.keys())
        {
            return Err(SourceTraversalClosureError::OrphanChildEvidence);
        }
        if reached_sources.len() != self.source_claims.len()
            || !reached_sources.iter().eq(self.source_claims.keys())
        {
            return Err(SourceTraversalClosureError::OrphanSourceEvidence);
        }
        if reached_edge_ids.len() != self.edges.len()
            || self.edge_subjects.len() != self.edges.len()
            || !reached_edge_ids.iter().eq(self.edge_subjects.keys())
        {
            return Err(SourceTraversalClosureError::OrphanTraversalEdge);
        }

        if self.receipt_subjects.len() != self.receipts.len() {
            return Err(SourceTraversalClosureError::FiberReceiptIdCollisionInProof);
        }
        let (fiber_receipt_root, fiber_receipt_count) = derive_fiber_receipt_set_commitment(
            self.relation_id,
            self.support_plan_root,
            self.receipt_subjects.keys().copied(),
        );
        let (source_key_root, source_key_count) =
            canonical_source_key_set_commitment(self.relation_id, reached_sources.iter().copied());
        let (traversal_edge_root, traversal_edge_count) =
            derive_edge_set_commitment(self.relation_id, reached_edge_ids.iter().copied());

        Ok(SourceRelationExhaustionReceipt::issue(
            self.relation_id,
            self.support_plan_root,
            self.binding_count,
            root_prefix.digest(),
            fiber_receipt_root,
            fiber_receipt_count,
            source_key_root,
            source_key_count,
            traversal_edge_root,
            traversal_edge_count,
        ))
    }
}

/// Constant-size semantic proof that concrete source enumeration is closed.
///
/// The roots and exact counts commit the prior incremental fiber, source, and
/// edge events without embedding their audit-sized identity arrays in this
/// terminal event. All fields are private. The only issuer is a fully verified
/// traversal accumulator; a codec may restore the compact claim, but journal
/// replay independently rebuilds and compares it before granting closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRelationExhaustionReceipt {
    schema_version: u32,
    producer_abi_version: u32,
    source_cursor_version: u32,
    fiber_receipt_version: u32,
    id: SourceRelationExhaustionReceiptId,
    relation_id: RelationId,
    support_plan_root: RelationalSupportPlanRoot,
    binding_count: u32,
    root_prefix_digest: [u8; 32],
    fiber_receipt_root: SourceFiberReceiptSetRoot,
    fiber_receipt_count: u128,
    source_key_root: SourceKeySetRoot,
    source_key_count: u128,
    traversal_edge_root: SourceTraversalEdgeRoot,
    traversal_edge_count: u128,
}

impl SourceRelationExhaustionReceipt {
    pub(super) fn restore_from_journal_codec(
        relation_id: RelationId,
        support_plan_root: RelationalSupportPlanRoot,
        binding_count: u32,
        fiber_receipt_root: SourceFiberReceiptSetRoot,
        fiber_receipt_count: u128,
        source_key_root: SourceKeySetRoot,
        source_key_count: u128,
        traversal_edge_root: SourceTraversalEdgeRoot,
        traversal_edge_count: u128,
    ) -> Result<Self, SourceTraversalClosureError> {
        let root_prefix_digest = CanonicalSourcePrefix::empty().digest();
        let restored = Self::issue(
            relation_id,
            support_plan_root,
            binding_count,
            root_prefix_digest,
            fiber_receipt_root,
            fiber_receipt_count,
            source_key_root,
            source_key_count,
            traversal_edge_root,
            traversal_edge_count,
        );
        restored.validate_identity()?;
        Ok(restored)
    }

    #[allow(clippy::too_many_arguments)]
    fn issue(
        relation_id: RelationId,
        support_plan_root: RelationalSupportPlanRoot,
        binding_count: u32,
        root_prefix_digest: [u8; 32],
        fiber_receipt_root: SourceFiberReceiptSetRoot,
        fiber_receipt_count: u128,
        source_key_root: SourceKeySetRoot,
        source_key_count: u128,
        traversal_edge_root: SourceTraversalEdgeRoot,
        traversal_edge_count: u128,
    ) -> Self {
        let schema_version = RELATIONAL_SOURCE_CLOSURE_SCHEMA_VERSION;
        let producer_abi_version = RELATIONAL_SOURCE_CLOSURE_PRODUCER_ABI_VERSION;
        let source_cursor_version = RELATIONAL_SOURCE_CURSOR_VERSION;
        let fiber_receipt_version = SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION;
        let id = derive_aggregate_receipt_id(
            schema_version,
            producer_abi_version,
            source_cursor_version,
            fiber_receipt_version,
            relation_id,
            support_plan_root,
            binding_count,
            root_prefix_digest,
            fiber_receipt_root,
            fiber_receipt_count,
            source_key_root,
            source_key_count,
            traversal_edge_root,
            traversal_edge_count,
        );
        Self {
            schema_version,
            producer_abi_version,
            source_cursor_version,
            fiber_receipt_version,
            id,
            relation_id,
            support_plan_root,
            binding_count,
            root_prefix_digest,
            fiber_receipt_root,
            fiber_receipt_count,
            source_key_root,
            source_key_count,
            traversal_edge_root,
            traversal_edge_count,
        }
    }

    pub(crate) const fn id(&self) -> SourceRelationExhaustionReceiptId {
        self.id
    }

    pub(crate) const fn relation_id(&self) -> RelationId {
        self.relation_id
    }

    pub(crate) const fn support_plan_root(&self) -> RelationalSupportPlanRoot {
        self.support_plan_root
    }

    pub(crate) const fn binding_count(&self) -> u32 {
        self.binding_count
    }

    pub(crate) const fn fiber_receipt_root(&self) -> SourceFiberReceiptSetRoot {
        self.fiber_receipt_root
    }

    pub(crate) const fn fiber_receipt_count(&self) -> u128 {
        self.fiber_receipt_count
    }

    /// Canonical set root of unique semantic sources. Multiple concrete leaves
    /// may converge on one key when auxiliary bindings project to the same
    /// `(Context, Before)` row; the exact set commitment normalizes that
    /// convergence without retaining the keys in this receipt.
    pub(crate) const fn source_key_root(&self) -> SourceKeySetRoot {
        self.source_key_root
    }

    pub(crate) const fn source_key_count(&self) -> u128 {
        self.source_key_count
    }

    pub(crate) const fn traversal_edge_root(&self) -> SourceTraversalEdgeRoot {
        self.traversal_edge_root
    }

    pub(crate) const fn traversal_edge_count(&self) -> u128 {
        self.traversal_edge_count
    }

    pub(crate) fn validate_identity(&self) -> Result<(), SourceTraversalClosureError> {
        if self.schema_version != RELATIONAL_SOURCE_CLOSURE_SCHEMA_VERSION
            || self.producer_abi_version != RELATIONAL_SOURCE_CLOSURE_PRODUCER_ABI_VERSION
            || self.source_cursor_version != RELATIONAL_SOURCE_CURSOR_VERSION
            || self.fiber_receipt_version != SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION
        {
            return Err(SourceTraversalClosureError::UnsupportedAggregateReceiptVersion);
        }
        if self.binding_count == 0
            || self.fiber_receipt_count == 0
            || self.root_prefix_digest != CanonicalSourcePrefix::empty().digest()
        {
            return Err(SourceTraversalClosureError::NonCanonicalAggregateReceipt);
        }
        let derived = derive_aggregate_receipt_id(
            self.schema_version,
            self.producer_abi_version,
            self.source_cursor_version,
            self.fiber_receipt_version,
            self.relation_id,
            self.support_plan_root,
            self.binding_count,
            self.root_prefix_digest,
            self.fiber_receipt_root,
            self.fiber_receipt_count,
            self.source_key_root,
            self.source_key_count,
            self.traversal_edge_root,
            self.traversal_edge_count,
        );
        if derived != self.id {
            return Err(SourceTraversalClosureError::AggregateReceiptIdMismatch {
                claimed: self.id,
                derived,
            });
        }
        Ok(())
    }
}

fn validate_prefix(prefix: &CanonicalSourcePrefix) -> Result<(), SourceTraversalClosureError> {
    let reconstructed = CanonicalSourcePrefix::from_values(prefix.values().to_vec())
        .map_err(|error| SourceTraversalClosureError::InvalidPrefix(error.to_string()))?;
    if &reconstructed != prefix {
        return Err(SourceTraversalClosureError::InvalidPrefix(
            "digest does not match canonical prefix values".to_string(),
        ));
    }
    Ok(())
}

fn derive_advance_id(
    relation_id: RelationId,
    support_plan_root: RelationalSupportPlanRoot,
    advance: &RelationalSourceAdvance,
) -> SourceTraversalAdvanceId {
    let mut hasher = SourceClosureHasher::new(SOURCE_TRAVERSAL_ADVANCE_ID_V2);
    hash_versions(&mut hasher);
    hasher.digest(relation_id.bytes());
    hasher.digest(support_plan_root.bytes());
    match advance {
        RelationalSourceAdvance::Yielded {
            member,
            resume,
            continuation,
        } => {
            hasher.u8(0x01);
            hash_fiber_member(&mut hasher, member);
            hash_source_cursor(&mut hasher, resume);
            match continuation {
                RelationalSourceContinuation::Expand(child) => {
                    hasher.u8(0x01);
                    hash_source_cursor(&mut hasher, child);
                }
                RelationalSourceContinuation::Source(source) => {
                    hasher.u8(0x02);
                    hasher.digest(source.source_key().bytes());
                    hasher.digest(canonical_explore_value_digest(source.row().context()));
                    hasher.digest(canonical_explore_value_digest(source.row().before()));
                    hash_relation_provenance(&mut hasher, source.row().provenance());
                    hash_source_prefix_snapshot(&mut hasher, source.prefix());
                }
            }
        }
        RelationalSourceAdvance::Exhausted {
            cursor,
            cardinality,
            receipt,
        } => {
            hasher.u8(0x02);
            hash_source_cursor(&mut hasher, cursor);
            hasher.u128(*cardinality);
            hasher.u32(receipt.version());
            hasher.digest(receipt.id().bytes());
            hasher.digest(receipt.relation_id().bytes());
            hasher.u32(receipt.binding_index());
            hasher.digest(receipt.prefix_digest());
            hasher.u128(receipt.terminal_ordinal());
            hasher.u128(receipt.emitted_member_count());
            hasher.digest(receipt.emitted_members_commitment());
        }
    }
    SourceTraversalAdvanceId(hasher.finish())
}

fn hash_fiber_member(hasher: &mut SourceClosureHasher, member: &RelationalFiberMember) {
    hasher.digest(canonical_explore_value_digest(member.value()));
    hasher.u128(member.canonical_ordinal());
    hasher.u128(member.raw_support_ordinals().len() as u128);
    for ordinal in member.raw_support_ordinals() {
        hasher.u128(*ordinal);
    }
}

fn hash_source_cursor(hasher: &mut SourceClosureHasher, cursor: &RelationalSourceCursor) {
    let snapshot = cursor.snapshot();
    hasher.u32(snapshot.version);
    hasher.u32(snapshot.binding_index);
    hash_source_prefix_snapshot(hasher, &snapshot.prefix);
    hasher.u128(snapshot.next_member_ordinal);
}

fn hash_source_prefix_snapshot(
    hasher: &mut SourceClosureHasher,
    prefix: &RelationalSourcePrefixSnapshot,
) {
    hasher.u32(prefix.version);
    hasher.u128(prefix.values.len() as u128);
    for value in prefix.values.iter() {
        hasher.digest(canonical_explore_value_digest(value));
    }
    hasher.digest(prefix.digest);
    hasher.u128(prefix.selections.len() as u128);
    for selection in prefix.selections.iter() {
        hash_binding_selection(hasher, selection);
    }
}

fn hash_binding_selection(
    hasher: &mut SourceClosureHasher,
    selection: &RelationalBindingSelection,
) {
    hasher.u32(selection.binding_index);
    hasher.u128(selection.canonical_ordinal);
    hasher.digest(selection.parent_prefix_digest);
    hasher.u128(selection.raw_support_ordinals.len() as u128);
    for ordinal in selection.raw_support_ordinals.iter() {
        hasher.u128(*ordinal);
    }
}

fn hash_relation_provenance(hasher: &mut SourceClosureHasher, provenance: &RelationProvenance) {
    hasher.u128(provenance.lineage().len() as u128);
    for lineage_id in provenance.lineage() {
        hasher.digest(lineage_id.bytes());
    }
    hasher.u128(provenance.support().len() as u128);
    for support_id in provenance.support() {
        hasher.digest(support_id.bytes());
    }
}

fn derive_edge_id(
    relation_id: RelationId,
    advance_id: SourceTraversalAdvanceId,
    key: SourceTraversalEdgeKey,
    target: &SourceTraversalTarget,
) -> SourceTraversalEdgeId {
    let mut hasher = SourceClosureHasher::new(SOURCE_TRAVERSAL_EDGE_ID_V2);
    hash_versions(&mut hasher);
    hasher.digest(relation_id.bytes());
    hasher.digest(advance_id.bytes());
    hasher.u32(key.fiber.binding_index);
    hasher.digest(key.fiber.prefix_digest);
    hasher.u128(key.canonical_ordinal);
    match target {
        SourceTraversalTarget::ChildFiber {
            binding_index,
            prefix_digest,
        } => {
            hasher.u8(0x01);
            hasher.u32(*binding_index);
            hasher.digest(*prefix_digest);
        }
        SourceTraversalTarget::Source {
            source_key,
            terminal_prefix_digest,
        } => {
            hasher.u8(0x02);
            hasher.digest(source_key.bytes());
            hasher.digest(*terminal_prefix_digest);
        }
    }
    SourceTraversalEdgeId(hasher.finish())
}

fn derive_fiber_receipt_set_commitment(
    relation_id: RelationId,
    support_plan_root: RelationalSupportPlanRoot,
    receipt_ids: impl ExactSizeIterator<Item = SourceBindingExhaustionReceiptId>,
) -> (SourceFiberReceiptSetRoot, u128) {
    let exact_count = receipt_ids.len() as u128;
    let mut hasher = SourceClosureHasher::new(SOURCE_FIBER_RECEIPT_SET_ROOT_V2);
    hash_versions(&mut hasher);
    hasher.digest(relation_id.bytes());
    hasher.digest(support_plan_root.bytes());
    hasher.u128(exact_count);
    for receipt_id in receipt_ids {
        hasher.digest(receipt_id.bytes());
    }
    (SourceFiberReceiptSetRoot(hasher.finish()), exact_count)
}

fn derive_edge_set_commitment(
    relation_id: RelationId,
    edge_ids: impl ExactSizeIterator<Item = SourceTraversalEdgeId>,
) -> (SourceTraversalEdgeRoot, u128) {
    let exact_count = edge_ids.len() as u128;
    let mut hasher = SourceClosureHasher::new(SOURCE_TRAVERSAL_EDGE_ROOT_V2);
    hash_versions(&mut hasher);
    hasher.digest(relation_id.bytes());
    hasher.u128(exact_count);
    for edge_id in edge_ids {
        hasher.digest(edge_id.bytes());
    }
    (SourceTraversalEdgeRoot(hasher.finish()), exact_count)
}

#[allow(clippy::too_many_arguments)]
fn derive_aggregate_receipt_id(
    schema_version: u32,
    producer_abi_version: u32,
    source_cursor_version: u32,
    fiber_receipt_version: u32,
    relation_id: RelationId,
    support_plan_root: RelationalSupportPlanRoot,
    binding_count: u32,
    root_prefix_digest: [u8; 32],
    fiber_receipt_root: SourceFiberReceiptSetRoot,
    fiber_receipt_count: u128,
    source_key_root: SourceKeySetRoot,
    source_key_count: u128,
    traversal_edge_root: SourceTraversalEdgeRoot,
    traversal_edge_count: u128,
) -> SourceRelationExhaustionReceiptId {
    let mut hasher = SourceClosureHasher::new(SOURCE_RELATION_EXHAUSTION_RECEIPT_ID_V2);
    hasher.u32(schema_version);
    hasher.u32(producer_abi_version);
    hasher.u32(source_cursor_version);
    hasher.u32(fiber_receipt_version);
    hasher.digest(relation_id.bytes());
    hasher.digest(support_plan_root.bytes());
    hasher.u32(binding_count);
    hasher.digest(root_prefix_digest);
    hasher.digest(fiber_receipt_root.bytes());
    hasher.u128(fiber_receipt_count);
    hasher.digest(source_key_root.bytes());
    hasher.u128(source_key_count);
    hasher.digest(traversal_edge_root.bytes());
    hasher.u128(traversal_edge_count);
    SourceRelationExhaustionReceiptId(hasher.finish())
}

fn hash_versions(hasher: &mut SourceClosureHasher) {
    hasher.u32(RELATIONAL_SOURCE_CLOSURE_SCHEMA_VERSION);
    hasher.u32(RELATIONAL_SOURCE_CLOSURE_PRODUCER_ABI_VERSION);
    hasher.u32(RELATIONAL_SOURCE_CURSOR_VERSION);
    hasher.u32(SOURCE_BINDING_EXHAUSTION_RECEIPT_VERSION);
}

struct SourceClosureHasher(Sha256);

impl SourceClosureHasher {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Sha256::new());
        hasher.bytes(domain);
        hasher
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u128).to_be_bytes());
        self.0.update(value);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceTraversalClosureError {
    InvalidSupportPlanRoot,
    EmptyBindingPlan,
    BindingCountOverflow,
    NonCanonicalBindingStage {
        actual: u32,
        expected: u32,
    },
    DuplicateContextBinding,
    DuplicateBeforeBinding,
    MissingContextBinding,
    MissingBeforeBinding,
    AdvanceIdMismatch {
        claimed: SourceTraversalAdvanceId,
        derived: SourceTraversalAdvanceId,
    },
    ExpectedYieldedAdvance,
    ExhaustedAdvanceMismatch,
    InvalidFiberReceipt(String),
    ReceiptRelationMismatch,
    ReceiptSubjectMismatch,
    BindingIndexOutOfBounds {
        binding_index: u32,
        binding_count: u32,
    },
    InvalidPrefix(String),
    CursorPrefixLengthMismatch {
        binding_index: u32,
        prefix_len: u32,
    },
    YieldResumeDidNotAdvance,
    YieldOrdinalMismatch {
        member: u128,
        resume: u128,
    },
    ExpectedSourceContinuation {
        binding_index: u32,
    },
    ExpectedChildContinuation {
        binding_index: u32,
    },
    ChildContinuationMismatch {
        binding_index: u32,
        canonical_ordinal: u128,
    },
    CompletedPrefixVersionMismatch {
        actual: u32,
        expected: u32,
    },
    CompletedPrefixMismatch,
    SourceProjectionMismatch,
    SourceKeyMismatch {
        claimed: SourceKey,
        derived: SourceKey,
    },
    PrefixDigestCollision {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    ConflictingFiberReceipt {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    FiberReceiptIdCollision {
        receipt_id: SourceBindingExhaustionReceiptId,
    },
    ReceiptRejectsObservedEdges {
        binding_index: u32,
        prefix_digest: [u8; 32],
        emitted_member_count: u128,
        observed_count: u128,
    },
    ChildHasMultipleParents {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    SourceKeyCollision {
        source_key: SourceKey,
    },
    ConflictingTraversalEdge {
        binding_index: u32,
        prefix_digest: [u8; 32],
        canonical_ordinal: u128,
    },
    TraversalEdgeIdCollision {
        edge_id: SourceTraversalEdgeId,
    },
    EdgeBeyondReceipt {
        binding_index: u32,
        prefix_digest: [u8; 32],
        canonical_ordinal: u128,
        emitted_member_count: u128,
    },
    NonCanonicalRootPrefix,
    MissingRootFiber,
    RootHasParent,
    MissingReachableFiberReceipt {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    FiberEdgeCountMismatch {
        binding_index: u32,
        prefix_digest: [u8; 32],
        expected: u128,
        actual: u128,
    },
    NonContiguousFiberOrdinal {
        binding_index: u32,
        prefix_digest: [u8; 32],
        expected: u128,
        actual: u128,
    },
    CorruptTraversalEdge,
    MissingReachableChild {
        binding_index: u32,
        prefix_digest: [u8; 32],
    },
    OrphanFiberEvidence {
        reachable: u128,
        known_prefixes: u128,
        receipts: u128,
    },
    OrphanChildEvidence,
    OrphanSourceEvidence,
    OrphanTraversalEdge,
    FiberReceiptIdCollisionInProof,
    UnsupportedAggregateReceiptVersion,
    NonCanonicalAggregateReceipt,
    AggregateReceiptIdMismatch {
        claimed: SourceRelationExhaustionReceiptId,
        derived: SourceRelationExhaustionReceiptId,
    },
}

impl fmt::Display for SourceTraversalClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSupportPlanRoot => {
                formatter.write_str("source traversal received an invalid support-plan root")
            }
            Self::EmptyBindingPlan => {
                formatter.write_str("source traversal requires at least one binding stage")
            }
            Self::BindingCountOverflow => {
                formatter.write_str("source traversal binding count exceeds canonical u32 space")
            }
            Self::NonCanonicalBindingStage { actual, expected } => write!(
                formatter,
                "source binding stage index {actual} is not canonical; expected {expected}"
            ),
            Self::DuplicateContextBinding => {
                formatter.write_str("source support plan has more than one Context binding")
            }
            Self::DuplicateBeforeBinding => {
                formatter.write_str("source support plan has more than one Before binding")
            }
            Self::MissingContextBinding => {
                formatter.write_str("source support plan has no Context binding")
            }
            Self::MissingBeforeBinding => {
                formatter.write_str("source support plan has no Before binding")
            }
            Self::AdvanceIdMismatch { .. } => formatter.write_str(
                "claimed source traversal advance identity does not match its semantic payload",
            ),
            Self::ExpectedYieldedAdvance => {
                formatter.write_str("source traversal edge requires a yielded executor advance")
            }
            Self::ExhaustedAdvanceMismatch => formatter
                .write_str("source exhausted advance disagrees with its producer-issued receipt"),
            Self::InvalidFiberReceipt(message) => {
                write!(
                    formatter,
                    "invalid source-fiber exhaustion receipt: {message}"
                )
            }
            Self::ReceiptRelationMismatch => {
                formatter.write_str("source-fiber receipt belongs to another relation")
            }
            Self::ReceiptSubjectMismatch => formatter
                .write_str("source-fiber receipt disagrees with its canonical binding prefix"),
            Self::BindingIndexOutOfBounds {
                binding_index,
                binding_count,
            } => write!(
                formatter,
                "source binding index {binding_index} is outside {binding_count} planned stages"
            ),
            Self::InvalidPrefix(message) => {
                write!(formatter, "invalid canonical source prefix: {message}")
            }
            Self::CursorPrefixLengthMismatch {
                binding_index,
                prefix_len,
            } => write!(
                formatter,
                "source cursor binding index {binding_index} disagrees with prefix length {prefix_len}"
            ),
            Self::YieldResumeDidNotAdvance => formatter
                .write_str("yielded source advance did not advance its parent-fiber ordinal"),
            Self::YieldOrdinalMismatch { member, resume } => write!(
                formatter,
                "yielded source member ordinal {member} disagrees with resume ordinal {resume}"
            ),
            Self::ExpectedSourceContinuation { binding_index } => write!(
                formatter,
                "last source binding {binding_index} yielded another child fiber"
            ),
            Self::ExpectedChildContinuation { binding_index } => write!(
                formatter,
                "non-terminal source binding {binding_index} yielded a completed source"
            ),
            Self::ChildContinuationMismatch { .. } => formatter.write_str(
                "yielded child cursor is not the canonical zero cursor for its extended prefix",
            ),
            Self::CompletedPrefixVersionMismatch { actual, expected } => write!(
                formatter,
                "completed source prefix version {actual} is unsupported; expected {expected}"
            ),
            Self::CompletedPrefixMismatch => formatter
                .write_str("completed source does not carry the canonical yielded binding prefix"),
            Self::SourceProjectionMismatch => formatter.write_str(
                "completed source Context/Before row disagrees with its planned binding projection",
            ),
            Self::SourceKeyMismatch { .. } => {
                formatter.write_str("completed source key is not canonical for its relation row")
            }
            Self::PrefixDigestCollision { .. } => formatter.write_str(
                "canonical source-prefix digest collision was rejected by traversal proof",
            ),
            Self::ConflictingFiberReceipt { .. } => {
                formatter.write_str("one source fiber received conflicting exhaustion receipts")
            }
            Self::FiberReceiptIdCollision { .. } | Self::FiberReceiptIdCollisionInProof => {
                formatter.write_str("source-fiber exhaustion receipt identity collision rejected")
            }
            Self::ReceiptRejectsObservedEdges { .. } => formatter.write_str(
                "source-fiber receipt cardinality excludes already observed traversal edges",
            ),
            Self::ChildHasMultipleParents { .. } => formatter
                .write_str("canonical dependent source fiber has more than one traversal parent"),
            Self::SourceKeyCollision { .. } => {
                formatter.write_str("SourceKey collision for unequal source content rejected")
            }
            Self::ConflictingTraversalEdge { .. } => formatter
                .write_str("one source-fiber ordinal yielded conflicting traversal targets"),
            Self::TraversalEdgeIdCollision { .. } => {
                formatter.write_str("source traversal edge identity collision rejected")
            }
            Self::EdgeBeyondReceipt { .. } => formatter
                .write_str("source traversal edge ordinal is outside its accepted fiber receipt"),
            Self::NonCanonicalRootPrefix => {
                formatter.write_str("source traversal root is not the canonical empty prefix")
            }
            Self::MissingRootFiber => {
                formatter.write_str("source traversal has no binding-zero empty-prefix root")
            }
            Self::RootHasParent => {
                formatter.write_str("source traversal root unexpectedly has a parent edge")
            }
            Self::MissingReachableFiberReceipt { .. } => formatter.write_str(
                "reachable source fiber has no exact producer-issued exhaustion receipt",
            ),
            Self::FiberEdgeCountMismatch { .. } => formatter.write_str(
                "reachable source fiber does not contain exactly its receipted ordinal edges",
            ),
            Self::NonContiguousFiberOrdinal { .. } => formatter.write_str(
                "reachable source fiber edge ordinals are not the exact interval 0..<count",
            ),
            Self::CorruptTraversalEdge => {
                formatter.write_str("source traversal edge failed identity or shape validation")
            }
            Self::MissingReachableChild { .. } => formatter.write_str(
                "source traversal edge names a child fiber without canonical child evidence",
            ),
            Self::OrphanFiberEvidence { .. } => formatter.write_str(
                "source traversal contains prefix or receipt evidence unreachable from its root",
            ),
            Self::OrphanChildEvidence => formatter
                .write_str("source traversal contains orphan or missing child-parent evidence"),
            Self::OrphanSourceEvidence => {
                formatter.write_str("source traversal contains orphan semantic source evidence")
            }
            Self::OrphanTraversalEdge => {
                formatter.write_str("source traversal contains an edge unreachable from its root")
            }
            Self::UnsupportedAggregateReceiptVersion => formatter.write_str(
                "source-relation exhaustion receipt schema or producer ABI is unsupported",
            ),
            Self::NonCanonicalAggregateReceipt => {
                formatter.write_str("source-relation exhaustion receipt payload is not canonical")
            }
            Self::AggregateReceiptIdMismatch { .. } => formatter.write_str(
                "source-relation exhaustion receipt identity does not match its payload",
            ),
        }
    }
}

impl Error for SourceTraversalClosureError {}
