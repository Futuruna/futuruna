//! Exact ordered evaluation of a request-bound relational classification capsule.
//!
//! The capsule producer normalizes the FIND lane to a `selected?` Boolean:
//! `matches p` lowers as `p`, `violations p` as `!p`, and `all` as `true`.
//! Admission lanes remain in checked admission-ordinal order. This evaluator
//! owns no host identity or evidence authority; it returns only one positional
//! outcome for each host-materialized subject.
//!
//! Capsule execution is transactional at the whole-batch boundary. Any
//! residual lane, unsupported node shape, or evaluation failure discards every
//! speculative outcome, cache entry, and statistic, then invokes the checked
//! classifier from subject zero. A bounded write-on-commit cache overlay records
//! only speculative insertions and evictions, so a failed capsule attempt cannot
//! leak even an eviction into later chunks or copy the retained cache.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;

use super::relational_classification_capsule::{
    ClassificationBinaryOp, ClassificationCallableId, ClassificationInputLane,
    ClassificationInputSlot, ClassificationLaneStatus, ClassificationNodeId,
    ClassificationNodeKind, ClassificationSemanticLane, ClassificationUnaryOp,
    RelationalClassificationCapsule, RuntimeConstructorKey, RuntimeConstructorShape,
};
use super::relational_classified_sweep::{
    RelationalCheckedClassificationContext, RelationalClassifiedCaseOutcome,
    RelationalClassifiedSweepError, RelationalOrderedClassificationBackend,
    RelationalOrderedClassificationSubject,
};
use super::relational_executor::RelationalExpressionRuntime;
use super::ExploreValue;
use crate::runtime_nominal_declared_type_name;

const MAX_CAPSULE_EVALUATION_DEPTH: usize = 1_024;
const MAX_COMPLETE_CALL_CACHE_LOGICAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_COMPLETE_CALL_CACHE_ENTRY_LOGICAL_BYTES: usize = 256 * 1024;

/// Committed operational statistics. Failed capsule attempts and failed
/// checked fallbacks do not change these counters. For `N` adjacent edges
/// whose shared observation callable receives equal `(Context, State)` values
/// at `After_i` and `Before_(i+1)`, a sufficiently large cache exposes the
/// intended `N + 1` body evaluations through `callable_body_evaluations`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RelationalClassificationEvaluatorStats {
    pub(crate) completed_batches: u128,
    pub(crate) capsule_batches: u128,
    pub(crate) checked_fallback_batches: u128,
    pub(crate) capsule_subjects: u128,
    pub(crate) checked_fallback_subjects: u128,
    pub(crate) admission_root_evaluations: u128,
    pub(crate) find_root_evaluations: u128,
    pub(crate) call_cache_hits: u128,
    pub(crate) call_cache_misses: u128,
    pub(crate) call_cache_insertions: u128,
    pub(crate) call_cache_evictions: u128,
    pub(crate) call_cache_oversized_skips: u128,
    pub(crate) callable_body_evaluations: u128,
}

impl RelationalClassificationEvaluatorStats {
    fn commit(&mut self, delta: Self) {
        self.completed_batches = self
            .completed_batches
            .saturating_add(delta.completed_batches);
        self.capsule_batches = self.capsule_batches.saturating_add(delta.capsule_batches);
        self.checked_fallback_batches = self
            .checked_fallback_batches
            .saturating_add(delta.checked_fallback_batches);
        self.capsule_subjects = self.capsule_subjects.saturating_add(delta.capsule_subjects);
        self.checked_fallback_subjects = self
            .checked_fallback_subjects
            .saturating_add(delta.checked_fallback_subjects);
        self.admission_root_evaluations = self
            .admission_root_evaluations
            .saturating_add(delta.admission_root_evaluations);
        self.find_root_evaluations = self
            .find_root_evaluations
            .saturating_add(delta.find_root_evaluations);
        self.call_cache_hits = self.call_cache_hits.saturating_add(delta.call_cache_hits);
        self.call_cache_misses = self
            .call_cache_misses
            .saturating_add(delta.call_cache_misses);
        self.call_cache_insertions = self
            .call_cache_insertions
            .saturating_add(delta.call_cache_insertions);
        self.call_cache_evictions = self
            .call_cache_evictions
            .saturating_add(delta.call_cache_evictions);
        self.call_cache_oversized_skips = self
            .call_cache_oversized_skips
            .saturating_add(delta.call_cache_oversized_skips);
        self.callable_body_evaluations = self
            .callable_body_evaluations
            .saturating_add(delta.callable_body_evaluations);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationalClassificationEvaluatorFallback {
    /// `None` denotes graph/lane preparation before any subject was inspected.
    pub(crate) subject_index: Option<usize>,
    pub(crate) reason: RelationalClassificationEvaluatorFallbackReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelationalClassificationEvaluatorFallbackReason {
    InvalidCapsuleIdentity,
    ResidualClassificationLane(ClassificationSemanticLane),
    MissingNormalizedFindRoot,
    MissingNode(ClassificationNodeId),
    MissingCallable(ClassificationCallableId),
    UnsupportedInputSlot(ClassificationInputSlot),
    MissingSourceBinding(u32),
    InvalidCallableFrame {
        callable_id: ClassificationCallableId,
        ordinal: u32,
    },
    InvalidCallableApplication(ClassificationCallableId),
    MissingRuntimeShape(ClassificationNodeId),
    RuntimeShapeMismatch(ClassificationNodeId),
    ExpectedBoolean(ClassificationNodeId),
    ExpectedInteger(ClassificationNodeId),
    UnsupportedScalarOperation(ClassificationNodeId),
    CheckedIntegerArithmeticFailed(ClassificationNodeId),
    InvalidCompleteCallCacheState,
    EvaluationDepthExceeded,
}

#[derive(Clone, Debug)]
struct ClassificationExecutionPlan {
    admission_roots: Box<[ClassificationNodeId]>,
    find_root: ClassificationNodeId,
    node_kinds: BTreeMap<ClassificationNodeId, Arc<ClassificationNodeKind>>,
    runtime_shapes_by_constructor: BTreeMap<[u8; 32], Arc<RuntimeConstructorShape>>,
    runtime_shapes_by_variant: BTreeMap<RuntimeConstructorKey, Arc<RuntimeConstructorShape>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CompleteCallCacheKey {
    callable_id: ClassificationCallableId,
    arguments: Box<[ExploreValue]>,
}

/// FIFO is intentional. Hits never mutate cache order, which keeps the
/// operational replacement policy deterministic and lets the transactional
/// overlay represent evictions as one retained-prefix length.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteCallCache {
    capacity: NonZeroUsize,
    entries: BTreeMap<CompleteCallCacheKey, CompleteCallCacheEntry>,
    insertion_order: VecDeque<CompleteCallCacheKey>,
    logical_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteCallCacheEntry {
    value: ExploreValue,
    logical_bytes: usize,
}

impl CompleteCallCache {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            entries: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            logical_bytes: 0,
        }
    }

    /// The cache is private and starts empty; transaction commit is its only
    /// mutation path. This constant-time guard checks the maintained summary,
    /// while commit validates every touched key and charge.
    fn has_valid_summary(&self) -> bool {
        self.entries.len() <= self.capacity.get()
            && self.entries.len() == self.insertion_order.len()
            && self.logical_bytes <= MAX_COMPLETE_CALL_CACHE_LOGICAL_BYTES
    }

    #[cfg(debug_assertions)]
    fn has_valid_full_invariants(&self) -> bool {
        if !self.has_valid_summary() {
            return false;
        }
        let mut keys = BTreeSet::new();
        let mut logical_bytes = 0usize;
        for key in self.insertion_order.iter() {
            if !keys.insert(key) {
                return false;
            }
            let Some(entry) = self.entries.get(key) else {
                return false;
            };
            if complete_call_cache_logical_bytes(key, &entry.value) != Some(entry.logical_bytes) {
                return false;
            }
            let Some(next) = logical_bytes.checked_add(entry.logical_bytes) else {
                return false;
            };
            logical_bytes = next;
        }
        keys.len() == self.entries.len() && logical_bytes == self.logical_bytes
    }
}

/// A speculative FIFO view over the retained cache. The base is exclusively
/// borrowed but remains byte-for-byte untouched until `commit`; dropping this
/// value is therefore an exact rollback whose cost is independent of retained
/// cache size.
struct CompleteCallCacheTransaction<'cache> {
    base: &'cache mut CompleteCallCache,
    base_eviction_count: usize,
    base_evicted_keys: BTreeSet<CompleteCallCacheKey>,
    pending_entries: BTreeMap<CompleteCallCacheKey, CompleteCallCacheEntry>,
    pending_order: VecDeque<CompleteCallCacheKey>,
    logical_bytes: usize,
}

impl<'cache> CompleteCallCacheTransaction<'cache> {
    fn new(base: &'cache mut CompleteCallCache) -> Result<Self, ()> {
        if !base.has_valid_summary() {
            return Err(());
        }
        #[cfg(debug_assertions)]
        if !base.has_valid_full_invariants() {
            return Err(());
        }
        Ok(Self {
            logical_bytes: base.logical_bytes,
            base,
            base_eviction_count: 0,
            base_evicted_keys: BTreeSet::new(),
            pending_entries: BTreeMap::new(),
            pending_order: VecDeque::new(),
        })
    }

    fn get(&self, key: &CompleteCallCacheKey) -> Option<&ExploreValue> {
        if let Some(entry) = self.pending_entries.get(key) {
            return Some(&entry.value);
        }
        if self.base_evicted_keys.contains(key) {
            return None;
        }
        self.base.entries.get(key).map(|entry| &entry.value)
    }

    fn insert_complete(
        &mut self,
        key: CompleteCallCacheKey,
        value: ExploreValue,
    ) -> Result<CacheInsertion, ()> {
        if self.get(&key).is_some() {
            return Ok(CacheInsertion {
                inserted: false,
                evictions: 0,
                skipped_oversized: false,
            });
        }
        let Some(logical_bytes) = complete_call_cache_logical_bytes(&key, &value) else {
            return Ok(CacheInsertion {
                inserted: false,
                evictions: 0,
                skipped_oversized: true,
            });
        };
        let mut evictions = 0usize;
        while self.view_len()? == self.base.capacity.get()
            || self
                .logical_bytes
                .checked_add(logical_bytes)
                .map_or(true, |total| total > MAX_COMPLETE_CALL_CACHE_LOGICAL_BYTES)
        {
            self.evict_oldest()?;
            evictions = evictions.saturating_add(1);
        }
        if self
            .pending_entries
            .insert(
                key.clone(),
                CompleteCallCacheEntry {
                    value,
                    logical_bytes,
                },
            )
            .is_some()
        {
            return Err(());
        }
        self.pending_order.push_back(key);
        self.logical_bytes = self.logical_bytes.checked_add(logical_bytes).ok_or(())?;
        Ok(CacheInsertion {
            inserted: true,
            evictions,
            skipped_oversized: false,
        })
    }

    fn view_len(&self) -> Result<usize, ()> {
        self.base
            .entries
            .len()
            .checked_sub(self.base_eviction_count)
            .and_then(|retained| retained.checked_add(self.pending_entries.len()))
            .ok_or(())
    }

    fn evict_oldest(&mut self) -> Result<(), ()> {
        if self.base_eviction_count < self.base.insertion_order.len() {
            let key = self
                .base
                .insertion_order
                .get(self.base_eviction_count)
                .cloned()
                .ok_or(())?;
            let entry = self.base.entries.get(&key).ok_or(())?;
            self.logical_bytes = self
                .logical_bytes
                .checked_sub(entry.logical_bytes)
                .ok_or(())?;
            self.base_eviction_count = self.base_eviction_count.checked_add(1).ok_or(())?;
            if !self.base_evicted_keys.insert(key) {
                return Err(());
            }
            return Ok(());
        }

        let key = self.pending_order.pop_front().ok_or(())?;
        let entry = self.pending_entries.remove(&key).ok_or(())?;
        self.logical_bytes = self
            .logical_bytes
            .checked_sub(entry.logical_bytes)
            .ok_or(())?;
        Ok(())
    }

    fn validate_commit(&self) -> Result<(), ()> {
        if self.base_evicted_keys.len() != self.base_eviction_count
            || self.pending_entries.len() != self.pending_order.len()
            || self.view_len()? > self.base.capacity.get()
            || self.logical_bytes > MAX_COMPLETE_CALL_CACHE_LOGICAL_BYTES
        {
            return Err(());
        }

        let mut expected_bytes = self.base.logical_bytes;
        let mut retained_prefix_keys = BTreeSet::new();
        for key in self
            .base
            .insertion_order
            .iter()
            .take(self.base_eviction_count)
        {
            if !retained_prefix_keys.insert(key) || !self.base_evicted_keys.contains(key) {
                return Err(());
            }
            let entry = self.base.entries.get(key).ok_or(())?;
            if complete_call_cache_logical_bytes(key, &entry.value) != Some(entry.logical_bytes) {
                return Err(());
            }
            expected_bytes = expected_bytes.checked_sub(entry.logical_bytes).ok_or(())?;
        }
        if self.base.insertion_order.len() < self.base_eviction_count {
            return Err(());
        }
        if retained_prefix_keys.len() != self.base_evicted_keys.len() {
            return Err(());
        }

        let mut pending_keys = BTreeSet::new();
        for key in self.pending_order.iter() {
            if !pending_keys.insert(key)
                || (self.base.entries.contains_key(key) && !self.base_evicted_keys.contains(key))
            {
                return Err(());
            }
            let entry = self.pending_entries.get(key).ok_or(())?;
            if complete_call_cache_logical_bytes(key, &entry.value) != Some(entry.logical_bytes) {
                return Err(());
            }
            expected_bytes = expected_bytes.checked_add(entry.logical_bytes).ok_or(())?;
        }
        (pending_keys.len() == self.pending_entries.len() && expected_bytes == self.logical_bytes)
            .then_some(())
            .ok_or(())
    }

    fn commit(mut self) -> Result<(), ()> {
        self.validate_commit()?;

        // Materialize the pending commit entirely before touching the base so
        // even an overlay inconsistency returns an atomic cache-state error.
        let mut pending = Vec::with_capacity(self.pending_order.len());
        while let Some(key) = self.pending_order.pop_front() {
            let entry = self.pending_entries.remove(&key).ok_or(())?;
            pending.push((key, entry));
        }
        if !self.pending_entries.is_empty() {
            return Err(());
        }

        // Every affected base key and pending collision was validated above.
        // These mutations are therefore infallible for this private cache.
        let evicted = self
            .base
            .insertion_order
            .drain(..self.base_eviction_count)
            .collect::<Vec<_>>();
        for key in evicted {
            let _ = self.base.entries.remove(&key);
        }
        for (key, entry) in pending {
            let _ = self.base.entries.insert(key.clone(), entry);
            self.base.insertion_order.push_back(key);
        }
        self.base.logical_bytes = self.logical_bytes;
        debug_assert!(self.base.has_valid_summary());
        #[cfg(debug_assertions)]
        debug_assert!(self.base.has_valid_full_invariants());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct CacheInsertion {
    inserted: bool,
    evictions: usize,
    skipped_oversized: bool,
}

fn complete_call_cache_logical_bytes(
    key: &CompleteCallCacheKey,
    value: &ExploreValue,
) -> Option<usize> {
    let mut total = 64usize;
    charge_logical_bytes(&mut total, 32)?;
    charge_logical_bytes(
        &mut total,
        key.arguments
            .len()
            .checked_mul(std::mem::size_of::<ExploreValue>())?,
    )?;
    for argument in key.arguments.iter() {
        charge_explore_value(&mut total, argument)?;
    }
    charge_explore_value(&mut total, value)?;
    Some(total)
}

fn charge_explore_value(total: &mut usize, root: &ExploreValue) -> Option<()> {
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        charge_logical_bytes(total, std::mem::size_of::<ExploreValue>())?;
        match value {
            ExploreValue::Int(_)
            | ExploreValue::FloatBits(_)
            | ExploreValue::Character(_)
            | ExploreValue::Boolean(_)
            | ExploreValue::Unit => {}
            ExploreValue::String(value) => charge_logical_bytes(total, value.len())?,
            ExploreValue::List(values)
            | ExploreValue::Set(values)
            | ExploreValue::Tuple(values) => {
                charge_logical_bytes(
                    total,
                    values
                        .len()
                        .checked_mul(std::mem::size_of::<ExploreValue>())?,
                )?;
                pending.extend(values.iter());
            }
            ExploreValue::Constructor {
                type_name,
                variant,
                fields,
                ..
            } => {
                charge_logical_bytes(total, type_name.len())?;
                charge_logical_bytes(total, variant.len())?;
                charge_logical_bytes(
                    total,
                    fields
                        .len()
                        .checked_mul(std::mem::size_of::<(String, ExploreValue)>())?,
                )?;
                for (field_name, field_value) in fields.iter() {
                    charge_logical_bytes(total, field_name.len())?;
                    pending.push(field_value);
                }
            }
        }
    }
    Some(())
}

fn charge_logical_bytes(total: &mut usize, additional: usize) -> Option<()> {
    *total = (*total).checked_add(additional)?;
    (*total <= MAX_COMPLETE_CALL_CACHE_ENTRY_LOGICAL_BYTES).then_some(())
}

/// Ordered capsule evaluator with a complete-call cache that persists across
/// host chunks. The backend is bound to exactly one capsule for its lifetime,
/// so `(callable_id, arguments)` is a sufficient cache namespace.
#[derive(Clone, Debug)]
pub(crate) struct RelationalClassificationEvaluatorBackend {
    capsule: Arc<RelationalClassificationCapsule>,
    plan: Result<ClassificationExecutionPlan, RelationalClassificationEvaluatorFallbackReason>,
    call_cache: CompleteCallCache,
    stats: RelationalClassificationEvaluatorStats,
    last_fallback: Option<RelationalClassificationEvaluatorFallback>,
}

impl RelationalClassificationEvaluatorBackend {
    pub(crate) fn new(
        capsule: Arc<RelationalClassificationCapsule>,
        call_cache_capacity: NonZeroUsize,
    ) -> Self {
        let plan = prepare_execution_plan(capsule.as_ref());
        Self {
            capsule,
            plan,
            call_cache: CompleteCallCache::new(call_cache_capacity),
            stats: RelationalClassificationEvaluatorStats::default(),
            last_fallback: None,
        }
    }

    pub(crate) fn capsule(&self) -> &RelationalClassificationCapsule {
        self.capsule.as_ref()
    }

    pub(crate) const fn stats(&self) -> RelationalClassificationEvaluatorStats {
        self.stats
    }

    pub(crate) fn last_fallback(&self) -> Option<&RelationalClassificationEvaluatorFallback> {
        self.last_fallback.as_ref()
    }

    pub(crate) fn call_cache_len(&self) -> usize {
        self.call_cache.entries.len()
    }

    pub(crate) const fn call_cache_logical_bytes(&self) -> usize {
        self.call_cache.logical_bytes
    }

    pub(crate) const fn call_cache_capacity(&self) -> NonZeroUsize {
        self.call_cache.capacity
    }
}

impl RelationalOrderedClassificationBackend for RelationalClassificationEvaluatorBackend {
    fn classify_ordered_batch<R: RelationalExpressionRuntime>(
        &mut self,
        subjects: &[RelationalOrderedClassificationSubject<'_>],
        checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
    ) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError> {
        let mut capsule_delta = RelationalClassificationEvaluatorStats::default();
        let capsule_attempt = match CompleteCallCacheTransaction::new(&mut self.call_cache) {
            Ok(mut transaction) => match classify_with_capsule(
                self.capsule.as_ref(),
                &self.plan,
                subjects,
                &mut transaction,
                &mut capsule_delta,
            ) {
                Ok(outcomes) => transaction.commit().map(|()| outcomes).map_err(|()| {
                    RelationalClassificationEvaluatorFallback {
                        subject_index: None,
                        reason: RelationalClassificationEvaluatorFallbackReason::InvalidCompleteCallCacheState,
                    }
                }),
                Err(fallback) => Err(fallback),
            },
            Err(()) => Err(RelationalClassificationEvaluatorFallback {
                subject_index: None,
                reason:
                    RelationalClassificationEvaluatorFallbackReason::InvalidCompleteCallCacheState,
            }),
        };
        match capsule_attempt {
            Ok(outcomes) => {
                capsule_delta.completed_batches = 1;
                capsule_delta.capsule_batches = 1;
                capsule_delta.capsule_subjects = subject_count(subjects);
                self.stats.commit(capsule_delta);
                self.last_fallback = None;
                Ok(outcomes)
            }
            Err(fallback) => {
                let outcomes = classify_checked_whole_batch(subjects, checked)?;
                self.stats.commit(RelationalClassificationEvaluatorStats {
                    completed_batches: 1,
                    checked_fallback_batches: 1,
                    checked_fallback_subjects: subject_count(subjects),
                    ..RelationalClassificationEvaluatorStats::default()
                });
                self.last_fallback = Some(fallback);
                Ok(outcomes)
            }
        }
    }
}

fn classify_with_capsule(
    capsule: &RelationalClassificationCapsule,
    plan: &Result<ClassificationExecutionPlan, RelationalClassificationEvaluatorFallbackReason>,
    subjects: &[RelationalOrderedClassificationSubject<'_>],
    transactional_cache: &mut CompleteCallCacheTransaction<'_>,
    delta: &mut RelationalClassificationEvaluatorStats,
) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassificationEvaluatorFallback> {
    let plan = plan
        .as_ref()
        .map_err(|reason| RelationalClassificationEvaluatorFallback {
            subject_index: None,
            reason: *reason,
        })?;
    let mut evaluator = CapsuleBatchEvaluator {
        capsule,
        plan,
        cache: transactional_cache,
        stats: delta,
    };
    let mut outcomes = Vec::with_capacity(subjects.len());
    for (subject_index, subject) in subjects.iter().copied().enumerate() {
        let outcome = evaluator.classify_subject(subject).map_err(|reason| {
            RelationalClassificationEvaluatorFallback {
                subject_index: Some(subject_index),
                reason,
            }
        })?;
        outcomes.push(outcome);
    }
    Ok(outcomes.into_boxed_slice())
}

fn prepare_execution_plan(
    capsule: &RelationalClassificationCapsule,
) -> Result<ClassificationExecutionPlan, RelationalClassificationEvaluatorFallbackReason> {
    if !capsule.validate_identity() {
        return Err(RelationalClassificationEvaluatorFallbackReason::InvalidCapsuleIdentity);
    }
    let graph = capsule.graph();
    let mut admissions = Vec::new();
    let mut find_root = None;
    for entry in graph.lane_manifest() {
        match entry.lane {
            ClassificationSemanticLane::Admission { ordinal, .. } => {
                if entry.status == ClassificationLaneStatus::Residual {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::ResidualClassificationLane(
                            entry.lane,
                        ),
                    );
                }
                let node = root_for_lane(graph.roots(), entry.lane).ok_or(
                    RelationalClassificationEvaluatorFallbackReason::InvalidCapsuleIdentity,
                )?;
                admissions.push((ordinal, node));
            }
            ClassificationSemanticLane::Find => {
                if entry.status == ClassificationLaneStatus::Residual {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::ResidualClassificationLane(
                            entry.lane,
                        ),
                    );
                }
                find_root = root_for_lane(graph.roots(), entry.lane);
            }
            ClassificationSemanticLane::SourceBinding(_)
            | ClassificationSemanticLane::Successor => {}
        }
    }
    admissions.sort_unstable_by_key(|(ordinal, _)| *ordinal);
    let mut node_kinds = BTreeMap::new();
    for (node_id, node) in graph.nodes() {
        if node_kinds
            .insert(*node_id, Arc::new(node.kind.clone()))
            .is_some()
        {
            return Err(RelationalClassificationEvaluatorFallbackReason::InvalidCapsuleIdentity);
        }
    }
    let mut runtime_shapes_by_constructor = BTreeMap::new();
    let mut runtime_shapes_by_variant = BTreeMap::new();
    for shape in capsule.runtime_shapes().shapes() {
        let shape = Arc::new(shape.clone());
        if runtime_shapes_by_constructor
            .insert(shape.constructor_id, Arc::clone(&shape))
            .is_some()
            || runtime_shapes_by_variant
                .insert(shape.key(), shape)
                .is_some()
        {
            return Err(RelationalClassificationEvaluatorFallbackReason::InvalidCapsuleIdentity);
        }
    }
    Ok(ClassificationExecutionPlan {
        admission_roots: admissions
            .into_iter()
            .map(|(_, node)| node)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        find_root: find_root
            .ok_or(RelationalClassificationEvaluatorFallbackReason::MissingNormalizedFindRoot)?,
        node_kinds,
        runtime_shapes_by_constructor,
        runtime_shapes_by_variant,
    })
}

fn root_for_lane(
    roots: &[super::relational_classification_capsule::ClassificationLaneRoot],
    lane: ClassificationSemanticLane,
) -> Option<ClassificationNodeId> {
    roots
        .binary_search_by_key(&lane, |root| root.lane)
        .ok()
        .map(|index| roots[index].node)
}

fn classify_checked_whole_batch<R: RelationalExpressionRuntime>(
    subjects: &[RelationalOrderedClassificationSubject<'_>],
    checked: &mut RelationalCheckedClassificationContext<'_, '_, '_, R>,
) -> Result<Box<[RelationalClassifiedCaseOutcome]>, RelationalClassifiedSweepError> {
    let mut outcomes = Vec::with_capacity(subjects.len());
    for subject in subjects.iter().copied() {
        outcomes.push(checked.classify(subject)?);
    }
    Ok(outcomes.into_boxed_slice())
}

fn subject_count(subjects: &[RelationalOrderedClassificationSubject<'_>]) -> u128 {
    u128::try_from(subjects.len()).unwrap_or(u128::MAX)
}

struct CallableFrame<'arguments> {
    callable_id: ClassificationCallableId,
    arguments: &'arguments [ExploreValue],
}

struct CapsuleBatchEvaluator<'capsule, 'transaction, 'cache, 'stats> {
    capsule: &'capsule RelationalClassificationCapsule,
    plan: &'capsule ClassificationExecutionPlan,
    cache: &'transaction mut CompleteCallCacheTransaction<'cache>,
    stats: &'stats mut RelationalClassificationEvaluatorStats,
}

impl CapsuleBatchEvaluator<'_, '_, '_, '_> {
    fn classify_subject(
        &mut self,
        subject: RelationalOrderedClassificationSubject<'_>,
    ) -> Result<RelationalClassifiedCaseOutcome, RelationalClassificationEvaluatorFallbackReason>
    {
        for index in 0..self.plan.admission_roots.len() {
            let root = self.plan.admission_roots[index];
            self.stats.admission_root_evaluations =
                self.stats.admission_root_evaluations.saturating_add(1);
            if !self.evaluate_boolean(root, subject, None, 0)? {
                return Ok(RelationalClassifiedCaseOutcome::Rejected);
            }
        }
        self.stats.find_root_evaluations = self.stats.find_root_evaluations.saturating_add(1);
        let find_root = self.plan.find_root;
        if self.evaluate_boolean(find_root, subject, None, 0)? {
            Ok(RelationalClassifiedCaseOutcome::AdmittedSelected)
        } else {
            Ok(RelationalClassifiedCaseOutcome::AdmittedNotSelected)
        }
    }

    fn evaluate_boolean(
        &mut self,
        node_id: ClassificationNodeId,
        subject: RelationalOrderedClassificationSubject<'_>,
        frame: Option<&CallableFrame<'_>>,
        depth: usize,
    ) -> Result<bool, RelationalClassificationEvaluatorFallbackReason> {
        match self.evaluate_node(node_id, subject, frame, depth)? {
            ExploreValue::Boolean(value) => Ok(value),
            _ => Err(RelationalClassificationEvaluatorFallbackReason::ExpectedBoolean(node_id)),
        }
    }

    fn evaluate_node(
        &mut self,
        node_id: ClassificationNodeId,
        subject: RelationalOrderedClassificationSubject<'_>,
        frame: Option<&CallableFrame<'_>>,
        depth: usize,
    ) -> Result<ExploreValue, RelationalClassificationEvaluatorFallbackReason> {
        if depth > MAX_CAPSULE_EVALUATION_DEPTH {
            return Err(RelationalClassificationEvaluatorFallbackReason::EvaluationDepthExceeded);
        }
        let kind = self
            .node_kind(node_id)
            .ok_or(RelationalClassificationEvaluatorFallbackReason::MissingNode(node_id))?;
        let child_depth = depth.saturating_add(1);
        match kind.as_ref() {
            ClassificationNodeKind::Constant(value) => Ok(value.to_explore_value()),
            ClassificationNodeKind::Input(slot) => self.evaluate_input(*slot, subject),
            ClassificationNodeKind::SourceParameter(binding_index) => {
                let binding_ordinal = *binding_index;
                let binding_index = usize::try_from(binding_ordinal).map_err(|_| {
                    RelationalClassificationEvaluatorFallbackReason::MissingSourceBinding(
                        binding_ordinal,
                    )
                })?;
                subject.source_binding(binding_index).cloned().ok_or(
                    RelationalClassificationEvaluatorFallbackReason::MissingSourceBinding(
                        binding_ordinal,
                    ),
                )
            }
            ClassificationNodeKind::CallableParameter {
                callable_id,
                ordinal,
            } => frame
                .filter(|frame| frame.callable_id == *callable_id)
                .and_then(|frame| {
                    usize::try_from(*ordinal)
                        .ok()
                        .and_then(|i| frame.arguments.get(i))
                })
                .cloned()
                .ok_or(
                    RelationalClassificationEvaluatorFallbackReason::InvalidCallableFrame {
                        callable_id: *callable_id,
                        ordinal: *ordinal,
                    },
                ),
            ClassificationNodeKind::Construct {
                constructor_id,
                fields,
            } => {
                let shape = self.runtime_shape_for_constructor(*constructor_id).ok_or(
                    RelationalClassificationEvaluatorFallbackReason::MissingRuntimeShape(node_id),
                )?;
                if fields.len() != shape.field_names.len() {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                            node_id,
                        ),
                    );
                }
                let mut runtime_fields = Vec::with_capacity(fields.len());
                for (field, field_name) in fields.iter().copied().zip(shape.field_names.iter()) {
                    runtime_fields.push((
                        field_name.to_string(),
                        self.evaluate_node(field, subject, frame, child_depth)?,
                    ));
                }
                Ok(ExploreValue::Constructor {
                    type_name: shape.type_name.to_string(),
                    variant: shape.variant_name.to_string(),
                    positional: shape.layout.is_positional(),
                    fields: Arc::from(runtime_fields),
                })
            }
            ClassificationNodeKind::Project {
                owner_id,
                variant_ordinal,
                field_ordinal,
                base,
            } => {
                let value = self.evaluate_node(*base, subject, frame, child_depth)?;
                let shape = self
                    .runtime_shape_for_variant(RuntimeConstructorKey {
                        owner_id: *owner_id,
                        variant_ordinal: *variant_ordinal,
                    })
                    .ok_or(
                        RelationalClassificationEvaluatorFallbackReason::MissingRuntimeShape(
                            node_id,
                        ),
                    )?;
                let fields = validated_constructor_fields(&value, shape.as_ref()).ok_or(
                    RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(node_id),
                )?;
                usize::try_from(*field_ordinal)
                    .ok()
                    .and_then(|index| fields.get(index))
                    .map(|(_, value)| value.clone())
                    .ok_or(
                        RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                            node_id,
                        ),
                    )
            }
            ClassificationNodeKind::IsVariant {
                owner_id,
                variant_ordinal,
                base,
            } => {
                let value = self.evaluate_node(*base, subject, frame, child_depth)?;
                let shape = self
                    .runtime_shape_for_variant(RuntimeConstructorKey {
                        owner_id: *owner_id,
                        variant_ordinal: *variant_ordinal,
                    })
                    .ok_or(
                        RelationalClassificationEvaluatorFallbackReason::MissingRuntimeShape(
                            node_id,
                        ),
                    )?;
                let ExploreValue::Constructor {
                    type_name, variant, ..
                } = &value
                else {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                            node_id,
                        ),
                    );
                };
                if runtime_nominal_declared_type_name(type_name) != shape.type_name.as_ref() {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                            node_id,
                        ),
                    );
                }
                if variant.as_str() != shape.variant_name.as_ref() {
                    return Ok(ExploreValue::Boolean(false));
                }
                if validated_constructor_fields(&value, shape.as_ref()).is_none() {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                            node_id,
                        ),
                    );
                }
                Ok(ExploreValue::Boolean(true))
            }
            ClassificationNodeKind::Unary { op, operand } => {
                let value = self.evaluate_node(*operand, subject, frame, child_depth)?;
                self.evaluate_unary(node_id, *op, value)
            }
            ClassificationNodeKind::Binary { op, left, right } => match op {
                ClassificationBinaryOp::BooleanAndShortCircuit => {
                    if !self.evaluate_boolean(*left, subject, frame, child_depth)? {
                        return Ok(ExploreValue::Boolean(false));
                    }
                    self.evaluate_boolean(*right, subject, frame, child_depth)
                        .map(ExploreValue::Boolean)
                }
                ClassificationBinaryOp::BooleanOrShortCircuit => {
                    if self.evaluate_boolean(*left, subject, frame, child_depth)? {
                        return Ok(ExploreValue::Boolean(true));
                    }
                    self.evaluate_boolean(*right, subject, frame, child_depth)
                        .map(ExploreValue::Boolean)
                }
                _ => {
                    let left_value = self.evaluate_node(*left, subject, frame, child_depth)?;
                    let right_value = self.evaluate_node(*right, subject, frame, child_depth)?;
                    self.evaluate_binary(node_id, *op, left_value, right_value)
                }
            },
            ClassificationNodeKind::If {
                condition,
                then_node,
                else_node,
            } => {
                let selected = if self.evaluate_boolean(*condition, subject, frame, child_depth)? {
                    *then_node
                } else {
                    *else_node
                };
                self.evaluate_node(selected, subject, frame, child_depth)
            }
            ClassificationNodeKind::Call {
                callable_id,
                arguments,
            } => self.evaluate_call(
                *callable_id,
                arguments.as_ref(),
                subject,
                frame,
                child_depth,
            ),
        }
    }

    fn evaluate_input(
        &self,
        slot: ClassificationInputSlot,
        subject: RelationalOrderedClassificationSubject<'_>,
    ) -> Result<ExploreValue, RelationalClassificationEvaluatorFallbackReason> {
        match (slot.lane, slot.ordinal) {
            (ClassificationInputLane::Context, 0) => Ok(subject.context().clone()),
            (ClassificationInputLane::State, 0) => Ok(subject.before().clone()),
            (ClassificationInputLane::State, 1) => Ok(subject.after().clone()),
            _ => Err(RelationalClassificationEvaluatorFallbackReason::UnsupportedInputSlot(slot)),
        }
    }

    fn evaluate_unary(
        &self,
        node_id: ClassificationNodeId,
        op: ClassificationUnaryOp,
        value: ExploreValue,
    ) -> Result<ExploreValue, RelationalClassificationEvaluatorFallbackReason> {
        match (op, value) {
            (ClassificationUnaryOp::BooleanNot, ExploreValue::Boolean(value)) => {
                Ok(ExploreValue::Boolean(!value))
            }
            (ClassificationUnaryOp::IntegerNegateChecked, ExploreValue::Int(value)) => {
                value.checked_neg().map(ExploreValue::Int).ok_or(
                    RelationalClassificationEvaluatorFallbackReason::CheckedIntegerArithmeticFailed(
                        node_id,
                    ),
                )
            }
            (ClassificationUnaryOp::BooleanNot, _) => {
                Err(RelationalClassificationEvaluatorFallbackReason::ExpectedBoolean(node_id))
            }
            (ClassificationUnaryOp::IntegerNegateChecked, _) => {
                Err(RelationalClassificationEvaluatorFallbackReason::ExpectedInteger(node_id))
            }
        }
    }

    fn evaluate_binary(
        &self,
        node_id: ClassificationNodeId,
        op: ClassificationBinaryOp,
        left: ExploreValue,
        right: ExploreValue,
    ) -> Result<ExploreValue, RelationalClassificationEvaluatorFallbackReason> {
        use ClassificationBinaryOp as Op;
        match op {
            Op::IntegerAddChecked
            | Op::IntegerSubtractChecked
            | Op::IntegerMultiplyChecked
            | Op::IntegerDivideChecked
            | Op::IntegerRemainderChecked => {
                let (ExploreValue::Int(left), ExploreValue::Int(right)) = (left, right) else {
                    return Err(
                        RelationalClassificationEvaluatorFallbackReason::ExpectedInteger(node_id),
                    );
                };
                let value = match op {
                    Op::IntegerAddChecked => left.checked_add(right),
                    Op::IntegerSubtractChecked => left.checked_sub(right),
                    Op::IntegerMultiplyChecked => left.checked_mul(right),
                    Op::IntegerDivideChecked => left.checked_div(right),
                    Op::IntegerRemainderChecked => left.checked_rem(right),
                    _ => {
                        return Err(
                            RelationalClassificationEvaluatorFallbackReason::UnsupportedScalarOperation(
                                node_id,
                            ),
                        );
                    }
                };
                value.map(ExploreValue::Int).ok_or(
                    RelationalClassificationEvaluatorFallbackReason::CheckedIntegerArithmeticFailed(
                        node_id,
                    ),
                )
            }
            Op::Equal | Op::NotEqual => scalar_equality(&left, &right)
                .map(|equal| ExploreValue::Boolean(if op == Op::Equal { equal } else { !equal }))
                .ok_or(
                    RelationalClassificationEvaluatorFallbackReason::UnsupportedScalarOperation(
                        node_id,
                    ),
                ),
            Op::LessThan | Op::LessThanOrEqual | Op::GreaterThan | Op::GreaterThanOrEqual => {
                scalar_comparison(op, &left, &right)
                    .map(ExploreValue::Boolean)
                    .ok_or(
                        RelationalClassificationEvaluatorFallbackReason::UnsupportedScalarOperation(
                            node_id,
                        ),
                    )
            }
            Op::BooleanAndShortCircuit | Op::BooleanOrShortCircuit => Err(
                RelationalClassificationEvaluatorFallbackReason::UnsupportedScalarOperation(
                    node_id,
                ),
            ),
        }
    }

    fn evaluate_call(
        &mut self,
        callable_id: ClassificationCallableId,
        argument_nodes: &[ClassificationNodeId],
        subject: RelationalOrderedClassificationSubject<'_>,
        caller_frame: Option<&CallableFrame<'_>>,
        depth: usize,
    ) -> Result<ExploreValue, RelationalClassificationEvaluatorFallbackReason> {
        let mut arguments = Vec::with_capacity(argument_nodes.len());
        for argument in argument_nodes.iter().copied() {
            arguments.push(self.evaluate_node(argument, subject, caller_frame, depth)?);
        }
        let key = CompleteCallCacheKey {
            callable_id,
            arguments: arguments.into_boxed_slice(),
        };
        if let Some(value) = self.cache.get(&key).cloned() {
            self.stats.call_cache_hits = self.stats.call_cache_hits.saturating_add(1);
            return Ok(value);
        }
        self.stats.call_cache_misses = self.stats.call_cache_misses.saturating_add(1);

        let definition = self
            .capsule
            .graph()
            .callables()
            .binary_search_by_key(&callable_id, |definition| definition.callable_id)
            .ok()
            .map(|index| &self.capsule.graph().callables()[index])
            .ok_or(RelationalClassificationEvaluatorFallbackReason::MissingCallable(callable_id))?;
        if definition.parameter_types.len() != key.arguments.len() {
            return Err(
                RelationalClassificationEvaluatorFallbackReason::InvalidCallableApplication(
                    callable_id,
                ),
            );
        }
        let body = definition.body;
        self.stats.callable_body_evaluations =
            self.stats.callable_body_evaluations.saturating_add(1);
        let frame = CallableFrame {
            callable_id,
            arguments: &key.arguments,
        };
        let value = self.evaluate_node(body, subject, Some(&frame), depth)?;
        let insertion = self
            .cache
            .insert_complete(key, value.clone())
            .map_err(|()| {
                RelationalClassificationEvaluatorFallbackReason::InvalidCompleteCallCacheState
            })?;
        if insertion.inserted {
            self.stats.call_cache_insertions = self.stats.call_cache_insertions.saturating_add(1);
        }
        self.stats.call_cache_evictions = self
            .stats
            .call_cache_evictions
            .saturating_add(u128::try_from(insertion.evictions).unwrap_or(u128::MAX));
        if insertion.skipped_oversized {
            self.stats.call_cache_oversized_skips =
                self.stats.call_cache_oversized_skips.saturating_add(1);
        }
        Ok(value)
    }

    fn node_kind(&self, node_id: ClassificationNodeId) -> Option<Arc<ClassificationNodeKind>> {
        self.plan.node_kinds.get(&node_id).map(Arc::clone)
    }

    fn runtime_shape_for_constructor(
        &self,
        constructor_id: [u8; 32],
    ) -> Option<Arc<RuntimeConstructorShape>> {
        self.plan
            .runtime_shapes_by_constructor
            .get(&constructor_id)
            .map(Arc::clone)
    }

    fn runtime_shape_for_variant(
        &self,
        key: RuntimeConstructorKey,
    ) -> Option<Arc<RuntimeConstructorShape>> {
        self.plan
            .runtime_shapes_by_variant
            .get(&key)
            .map(Arc::clone)
    }
}

fn validated_constructor_fields<'value>(
    value: &'value ExploreValue,
    shape: &RuntimeConstructorShape,
) -> Option<&'value [(String, ExploreValue)]> {
    let ExploreValue::Constructor {
        type_name,
        variant,
        positional,
        fields,
    } = value
    else {
        return None;
    };
    (runtime_nominal_declared_type_name(type_name) == shape.type_name.as_ref()
        && variant.as_str() == shape.variant_name.as_ref()
        && *positional == shape.layout.is_positional()
        && fields.len() == shape.field_names.len()
        && fields
            .iter()
            .zip(shape.field_names.iter())
            .all(|((actual, _), expected)| actual.as_str() == expected.as_ref()))
    .then_some(fields.as_ref())
}

fn scalar_equality(left: &ExploreValue, right: &ExploreValue) -> Option<bool> {
    match (left, right) {
        (ExploreValue::Int(left), ExploreValue::Int(right)) => Some(left == right),
        (ExploreValue::Boolean(left), ExploreValue::Boolean(right)) => Some(left == right),
        (ExploreValue::String(left), ExploreValue::String(right)) => Some(left == right),
        (ExploreValue::Character(left), ExploreValue::Character(right)) => Some(left == right),
        (ExploreValue::Unit, ExploreValue::Unit) => Some(true),
        _ => None,
    }
}

fn scalar_comparison(
    op: ClassificationBinaryOp,
    left: &ExploreValue,
    right: &ExploreValue,
) -> Option<bool> {
    let (ExploreValue::Int(left), ExploreValue::Int(right)) = (left, right) else {
        return None;
    };
    Some(match op {
        ClassificationBinaryOp::LessThan => left < right,
        ClassificationBinaryOp::LessThanOrEqual => left <= right,
        ClassificationBinaryOp::GreaterThan => left > right,
        ClassificationBinaryOp::GreaterThanOrEqual => left >= right,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::relation::{AdmissionId, FindPolarity, QuestionId, RelationId};
    use super::super::relational_case_executor::{RelationalCaseExecutor, RelationalConcreteCase};
    use super::super::relational_classification_capsule::{
        ClassificationCallableDefinition, ClassificationInterner, ClassificationLaneRoot,
        ClassificationNodeKey, ClassificationProvenanceRoot, ClassificationRuntimeLayout,
        ClassificationSpecializationRoot, ClassificationTypeId, FrozenClassificationProgram,
        FrozenClassificationRuntimeShapes, RuntimeConstructorShape,
    };
    use super::super::relational_executor::{
        RelationalBoundValue, RelationalCompletedSource, RelationalSourceEnumerator,
    };
    use super::super::relational_ir::{
        ExploreFindIr, ExploreQueryIr, ExploreSourceBindingIr, ExploreSourceBindingKindIr,
        ExploreSourceBindingRoleIr, ExploreSourceRelationIr, ExploreSuccessorKindIr,
        ExploreSuccessorRelationIr,
    };
    use super::super::relational_support_planner::RelationalSupportPlanRoot;
    use super::*;
    use crate::explore::{ExploreEnumeratedSource, ExploreExactDomain, ExploreFiniteDomainIr};
    use crate::{
        ExploreRelationMultiplicity, Expr, ExprKind, Literal, Span, Ty,
        EXPLORE_RELATION_NORMALIZATION_VERSION,
    };

    #[derive(Default)]
    struct FixtureRuntime;

    impl RelationalExpressionRuntime for FixtureRuntime {
        fn evaluate(
            &mut self,
            expression: &Expr,
            _expected_ty: &Ty,
            bindings: &[RelationalBoundValue<'_>],
        ) -> Result<ExploreValue, String> {
            fn evaluate(
                expression: &Expr,
                bindings: &[RelationalBoundValue<'_>],
            ) -> Result<ExploreValue, String> {
                match &expression.kind {
                    ExprKind::Unit => Ok(ExploreValue::Unit),
                    ExprKind::Lit(Literal::Int(value)) => Ok(ExploreValue::Int(*value)),
                    ExprKind::Lit(Literal::Bool(value)) => Ok(ExploreValue::Boolean(*value)),
                    ExprKind::Var(name) => bindings
                        .iter()
                        .find(|binding| binding.name == name.as_str())
                        .map(|binding| binding.value.clone())
                        .ok_or_else(|| format!("unbound fixture name {name}")),
                    ExprKind::BinOp(operator, left, right) if operator == "+" => {
                        let (ExploreValue::Int(left), ExploreValue::Int(right)) =
                            (evaluate(left, bindings)?, evaluate(right, bindings)?)
                        else {
                            return Err("fixture addition requires integers".to_string());
                        };
                        left.checked_add(right)
                            .map(ExploreValue::Int)
                            .ok_or_else(|| "fixture addition overflowed".to_string())
                    }
                    ExprKind::BinOp(operator, left, right) if operator == "<" => {
                        let (ExploreValue::Int(left), ExploreValue::Int(right)) =
                            (evaluate(left, bindings)?, evaluate(right, bindings)?)
                        else {
                            return Err("fixture comparison requires integers".to_string());
                        };
                        Ok(ExploreValue::Boolean(left < right))
                    }
                    _ => Err("unsupported evaluator fixture expression".to_string()),
                }
            }

            evaluate(expression, bindings)
        }
    }

    struct MaterializedFixtureCase {
        source: RelationalCompletedSource,
        case: RelationalConcreteCase,
    }

    fn type_id(tag: u8) -> ClassificationTypeId {
        ClassificationTypeId::from_checked_type_digest([tag; 32])
    }

    fn int(value: i64) -> Expr {
        Expr::unspanned(ExprKind::Lit(Literal::Int(value)))
    }

    fn variable(name: &str) -> Expr {
        Expr::unspanned(ExprKind::Var(name.to_string()))
    }

    fn fixture_query(
        before_ty: Ty,
        before_domain: ExploreExactDomain,
        successor: Expr,
        predicate: Expr,
    ) -> ExploreQueryIr {
        ExploreQueryIr {
            name: "classification-evaluator-fixture".to_string(),
            source: ExploreSourceRelationIr {
                normalization_version: EXPLORE_RELATION_NORMALIZATION_VERSION,
                multiplicity: ExploreRelationMultiplicity::SetNormalized,
                bindings: vec![
                    ExploreSourceBindingIr {
                        binding_index: 0,
                        name: "context".to_string(),
                        value_ty: Ty::Unit,
                        role: ExploreSourceBindingRoleIr::Context,
                        dependencies: Box::new([]),
                        kind: ExploreSourceBindingKindIr::Singleton {
                            value: Expr::unspanned(ExprKind::Unit),
                        },
                        span: Span::dummy(),
                    },
                    ExploreSourceBindingIr {
                        binding_index: 1,
                        name: "before".to_string(),
                        value_ty: before_ty.clone(),
                        role: ExploreSourceBindingRoleIr::Before,
                        dependencies: Box::new([]),
                        kind: ExploreSourceBindingKindIr::Finite {
                            domain: ExploreFiniteDomainIr::Exact(before_domain),
                        },
                        span: Span::dummy(),
                    },
                ]
                .into_boxed_slice(),
                context_binding_index: 0,
                before_binding_index: 1,
                context_ty: Ty::Unit,
                before_ty: before_ty.clone(),
            },
            successor: ExploreSuccessorRelationIr {
                multiplicity: ExploreRelationMultiplicity::SetNormalized,
                after_ty: before_ty,
                kind: ExploreSuccessorKindIr::Singleton { value: successor },
                span: Span::dummy(),
            },
            admissions: Box::new([]),
            find: ExploreFindIr::Matches {
                predicate,
                span: Span::dummy(),
            },
            analysis: Box::new([]),
            starter_projections: Box::new([]),
            transition_graphs: Box::new([]),
            span: Span::dummy(),
        }
    }

    fn materialize_cases(
        relation_id: RelationId,
        query: &ExploreQueryIr,
        count: u128,
    ) -> Vec<MaterializedFixtureCase> {
        let sources = RelationalSourceEnumerator::new(relation_id, &query.source).unwrap();
        let cases = RelationalCaseExecutor::new(relation_id, query).unwrap();
        let mut runtime = FixtureRuntime;
        (0..count)
            .map(|ordinal| {
                let source = sources
                    .completed_source_at_independent_finite_ordinals(&[ordinal], &mut runtime)
                    .unwrap();
                let transition = cases
                    .statically_singleton_transition(
                        source.source_key(),
                        source.row(),
                        &mut runtime,
                    )
                    .unwrap()
                    .expect("fixture successor is syntactically singleton");
                let (case, _) = transition.into_parts();
                MaterializedFixtureCase { source, case }
            })
            .collect()
    }

    fn subjects(
        materialized: &[MaterializedFixtureCase],
    ) -> Vec<RelationalOrderedClassificationSubject<'_>> {
        materialized
            .iter()
            .map(|materialized| {
                RelationalOrderedClassificationSubject::new(
                    &materialized.source,
                    &materialized.case,
                )
            })
            .collect()
    }

    fn bind_capsule(
        graph: Arc<FrozenClassificationProgram>,
        runtime_shapes: Arc<FrozenClassificationRuntimeShapes>,
        relation_id: RelationId,
    ) -> Arc<RelationalClassificationCapsule> {
        let admission_id = AdmissionId::from_canonical_admission_digest(relation_id, [31; 32]);
        let question_id =
            QuestionId::from_canonical_find_digest(admission_id, [32; 32], FindPolarity::Matches);
        Arc::new(
            RelationalClassificationCapsule::bind(
                graph,
                runtime_shapes,
                [33; 32],
                relation_id,
                admission_id,
                question_id,
                RelationalSupportPlanRoot::from_journal_codec_bytes([34; 32]),
                None,
                ClassificationSpecializationRoot::none(),
                ClassificationProvenanceRoot::from_checked_source_coverage_digest([35; 32]),
            )
            .unwrap(),
        )
    }

    fn adjacent_observation_graph() -> Arc<FrozenClassificationProgram> {
        let integer = type_id(1);
        let boolean = type_id(2);
        let callable_id = ClassificationCallableId::from_checked_callable_digest([41; 32]);
        let mut interner = ClassificationInterner::default();
        let parameter = interner
            .intern(ClassificationNodeKey {
                ty: integer,
                kind: ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal: 0,
                },
            })
            .unwrap();
        let before = interner
            .intern(ClassificationNodeKey {
                ty: integer,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::BEFORE),
            })
            .unwrap();
        let after = interner
            .intern(ClassificationNodeKey {
                ty: integer,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::AFTER),
            })
            .unwrap();
        let before_observation = interner
            .intern(ClassificationNodeKey {
                ty: integer,
                kind: ClassificationNodeKind::Call {
                    callable_id,
                    arguments: Box::new([before]),
                },
            })
            .unwrap();
        let after_observation = interner
            .intern(ClassificationNodeKey {
                ty: integer,
                kind: ClassificationNodeKind::Call {
                    callable_id,
                    arguments: Box::new([after]),
                },
            })
            .unwrap();
        let find = interner
            .intern(ClassificationNodeKey {
                ty: boolean,
                kind: ClassificationNodeKind::Binary {
                    op: ClassificationBinaryOp::LessThan,
                    left: before_observation,
                    right: after_observation,
                },
            })
            .unwrap();
        Arc::new(
            FrozenClassificationProgram::freeze_with_callables(
                interner,
                [ClassificationCallableDefinition {
                    callable_id,
                    parameter_types: Box::new([integer]),
                    return_type: integer,
                    body: parameter,
                }],
                [ClassificationSemanticLane::Find],
                [ClassificationLaneRoot {
                    lane: ClassificationSemanticLane::Find,
                    node: find,
                }],
                [],
            )
            .unwrap(),
        )
    }

    fn constructor_value(field_name: &str) -> ExploreValue {
        ExploreValue::Constructor {
            type_name: "State".to_string(),
            variant: "Only".to_string(),
            positional: false,
            fields: Arc::new([(field_name.to_string(), ExploreValue::Int(1))]),
        }
    }

    fn variant_graph_and_shapes() -> (
        Arc<FrozenClassificationProgram>,
        Arc<FrozenClassificationRuntimeShapes>,
        ClassificationNodeId,
    ) {
        let owner_id = [51; 32];
        let state = type_id(3);
        let callable_id = ClassificationCallableId::from_checked_callable_digest([53; 32]);
        let mut interner = ClassificationInterner::default();
        let parameter = interner
            .intern(ClassificationNodeKey {
                ty: state,
                kind: ClassificationNodeKind::CallableParameter {
                    callable_id,
                    ordinal: 0,
                },
            })
            .unwrap();
        let before = interner
            .intern(ClassificationNodeKey {
                ty: state,
                kind: ClassificationNodeKind::Input(ClassificationInputSlot::BEFORE),
            })
            .unwrap();
        let observed_before = interner
            .intern(ClassificationNodeKey {
                ty: state,
                kind: ClassificationNodeKind::Call {
                    callable_id,
                    arguments: Box::new([before]),
                },
            })
            .unwrap();
        let find = interner
            .intern(ClassificationNodeKey {
                ty: type_id(2),
                kind: ClassificationNodeKind::IsVariant {
                    owner_id,
                    variant_ordinal: 0,
                    base: observed_before,
                },
            })
            .unwrap();
        let graph = Arc::new(
            FrozenClassificationProgram::freeze_with_callables(
                interner,
                [ClassificationCallableDefinition {
                    callable_id,
                    parameter_types: Box::new([state]),
                    return_type: state,
                    body: parameter,
                }],
                [ClassificationSemanticLane::Find],
                [ClassificationLaneRoot {
                    lane: ClassificationSemanticLane::Find,
                    node: find,
                }],
                [],
            )
            .unwrap(),
        );
        let shapes = Arc::new(
            FrozenClassificationRuntimeShapes::freeze([RuntimeConstructorShape::new(
                owner_id,
                0,
                [52; 32],
                "State".into(),
                "Only".into(),
                ClassificationRuntimeLayout::Named,
                Box::new([Box::<str>::from("amount")]),
            )])
            .unwrap(),
        );
        (graph, shapes, find)
    }

    #[test]
    fn adjacent_edges_reuse_complete_observation_calls_at_n_plus_one_cost() {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(b"classification-call-cache-fixture");
        let query = fixture_query(
            Ty::Name("Int".to_string()),
            ExploreExactDomain::IntRange {
                start: 0,
                end_exclusive: 3,
                cardinality: 3,
            },
            Expr::unspanned(ExprKind::BinOp(
                "+".to_string(),
                Box::new(variable("before")),
                Box::new(int(1)),
            )),
            Expr::unspanned(ExprKind::BinOp(
                "<".to_string(),
                Box::new(variable("before")),
                Box::new(variable("after")),
            )),
        );
        let materialized = materialize_cases(relation_id, &query, 3);
        let subjects = subjects(&materialized);
        let cases = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let expected = {
            let mut runtime = FixtureRuntime;
            let mut checked = RelationalCheckedClassificationContext::new(&cases, &mut runtime);
            subjects
                .iter()
                .copied()
                .map(|subject| checked.classify(subject).unwrap())
                .collect::<Vec<_>>()
        };
        let capsule = bind_capsule(
            adjacent_observation_graph(),
            Arc::new(FrozenClassificationRuntimeShapes::freeze([]).unwrap()),
            relation_id,
        );
        let mut backend =
            RelationalClassificationEvaluatorBackend::new(capsule, NonZeroUsize::new(16).unwrap());
        let mut runtime = FixtureRuntime;
        let mut checked = RelationalCheckedClassificationContext::new(&cases, &mut runtime);

        let outcomes = backend
            .classify_ordered_batch(&subjects, &mut checked)
            .unwrap();

        assert_eq!(
            expected,
            vec![RelationalClassifiedCaseOutcome::AdmittedSelected; 3]
        );
        assert_eq!(outcomes.as_ref(), expected.as_slice());
        let stats = backend.stats();
        assert_eq!(stats.completed_batches, 1);
        assert_eq!(stats.capsule_batches, 1);
        assert_eq!(stats.checked_fallback_batches, 0);
        assert_eq!(stats.call_cache_hits, 2);
        assert_eq!(stats.call_cache_misses, 4);
        assert_eq!(stats.call_cache_insertions, 4);
        assert_eq!(stats.call_cache_evictions, 0);
        assert_eq!(stats.callable_body_evaluations, 4);
        assert_eq!(backend.call_cache_len(), 4);
        assert!(backend.call_cache_logical_bytes() > 0);
        assert!(backend.last_fallback().is_none());
    }

    #[test]
    fn runtime_shape_mismatch_falls_back_for_the_whole_batch_atomically() {
        let relation_id =
            RelationId::from_canonical_semantic_preimage(b"classification-shape-fallback-fixture");
        let query = fixture_query(
            Ty::Name("State".to_string()),
            ExploreExactDomain::Enumerated {
                values: vec![constructor_value("amount"), constructor_value("wrong")],
                source: ExploreEnumeratedSource::ExplicitList,
            },
            variable("before"),
            Expr::unspanned(ExprKind::Lit(Literal::Bool(true))),
        );
        let materialized = materialize_cases(relation_id, &query, 2);
        let subjects = subjects(&materialized);
        let cases = RelationalCaseExecutor::new(relation_id, &query).unwrap();
        let expected = {
            let mut runtime = FixtureRuntime;
            let mut checked = RelationalCheckedClassificationContext::new(&cases, &mut runtime);
            subjects
                .iter()
                .copied()
                .map(|subject| checked.classify(subject).unwrap())
                .collect::<Vec<_>>()
        };
        let (graph, shapes, find) = variant_graph_and_shapes();
        let capsule = bind_capsule(graph, shapes, relation_id);
        let mut backend =
            RelationalClassificationEvaluatorBackend::new(capsule, NonZeroUsize::new(1).unwrap());
        {
            let mut runtime = FixtureRuntime;
            let mut checked = RelationalCheckedClassificationContext::new(&cases, &mut runtime);
            let seeded = backend
                .classify_ordered_batch(&subjects[..1], &mut checked)
                .unwrap();
            assert_eq!(
                seeded.as_ref(),
                &[RelationalClassifiedCaseOutcome::AdmittedSelected]
            );
        }
        let seeded_cache = backend.call_cache.clone();
        let seeded_bytes = backend.call_cache_logical_bytes();
        let seeded_stats = backend.stats();

        let mut runtime = FixtureRuntime;
        let mut checked = RelationalCheckedClassificationContext::new(&cases, &mut runtime);

        let outcomes = backend
            .classify_ordered_batch(&subjects, &mut checked)
            .unwrap();

        assert_eq!(
            expected,
            vec![RelationalClassifiedCaseOutcome::AdmittedSelected; 2]
        );
        assert_eq!(outcomes.as_ref(), expected.as_slice());
        assert_eq!(backend.call_cache, seeded_cache);
        assert_eq!(backend.call_cache_len(), 1);
        assert_eq!(backend.call_cache_logical_bytes(), seeded_bytes);
        assert!(matches!(
            backend.last_fallback(),
            Some(RelationalClassificationEvaluatorFallback {
                subject_index: Some(1),
                reason: RelationalClassificationEvaluatorFallbackReason::RuntimeShapeMismatch(
                    node,
                ),
            }) if *node == find
        ));
        let mut expected_stats = seeded_stats;
        expected_stats.commit(RelationalClassificationEvaluatorStats {
            completed_batches: 1,
            checked_fallback_batches: 1,
            checked_fallback_subjects: 2,
            ..RelationalClassificationEvaluatorStats::default()
        });
        assert_eq!(backend.stats(), expected_stats);
        assert_eq!(seeded_stats.call_cache_misses, 1);
        assert_eq!(seeded_stats.call_cache_insertions, 1);
        assert_eq!(seeded_stats.call_cache_evictions, 0);
        assert_eq!(seeded_stats.callable_body_evaluations, 1);
    }
}
