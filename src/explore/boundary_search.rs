//! Deterministic candidate-first scheduling for one finite boundary axis.
//!
//! Candidates are scheduling hints over the unchanged declared query
//! universe. Evaluating one candidate closes only that singleton. A wider cell
//! closes only through an explicit region certificate. Candidate scheduling is
//! one-pass, mutable profile partitions are indexed by interval start, and the
//! cost ledger is maintained by checked deltas rather than global rescans.
//!
//! [`BoundaryPlan`] is intentionally an audit/export representation here, not
//! the mutable hot-path structure. Likewise, the sparse point case DAG contains
//! evaluated singleton classifications only; certified regions remain in their
//! profile plans until a rectangle-to-case-DAG lowering exists. Every other
//! declared assignment remains conservatively eligibility-open in that DAG.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::error::Error;
use std::fmt;

use super::boundary_plan::{BoundaryCell, BoundaryInterval, BoundaryPlan, BoundaryPlanError};
use super::case_graph::{
    CaseDecisionDag, CaseGraphError, CaseOpenReason, CaseTerminal, OrderedDecisionDag,
};
use super::certified_region::{
    lower_certified_case_regions, CertifiedCaseRegion, CertifiedOrdinalInterval,
};
use super::report::ExploreCaseId;

/// Why one exact boundary-plan cell is closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundaryClosureCertificate<Certificate> {
    /// A normal runtime evaluation establishes one singleton only.
    SingletonEvaluation,
    /// An explicit proof establishes every point in the recorded region.
    Region(Certificate),
}

/// V1 does not publish a mechanism signature for a certified region. The
/// uninhabited signature payload makes every exported closed cell carry
/// `None`; signature extrapolation requires a future, distinct invariance proof
/// contract rather than reusing a classification certificate.
pub(super) type ProfileBoundaryPlan<Certificate> =
    BoundaryPlan<CaseTerminal, Option<Infallible>, BoundaryClosureCertificate<Certificate>, ()>;

/// One source-derived scheduling hint guarded by an exact outer profile.
/// `outer_ordinals` contains every canonical generator axis except the boundary axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundarySearchCandidate<Hint> {
    outer_ordinals: Box<[u128]>,
    boundary_value: i64,
    hint: Hint,
}

impl<Hint> BoundarySearchCandidate<Hint> {
    pub(super) fn new(
        outer_ordinals: impl Into<Box<[u128]>>,
        boundary_value: i64,
        hint: Hint,
    ) -> Self {
        Self {
            outer_ordinals: outer_ordinals.into(),
            boundary_value,
            hint,
        }
    }

    pub(super) fn outer_ordinals(&self) -> &[u128] {
        &self.outer_ordinals
    }

    pub(super) fn boundary_value(&self) -> i64 {
        self.boundary_value
    }

    pub(super) fn hint(&self) -> &Hint {
        &self.hint
    }
}

/// One exact evaluation requested by the candidate-first scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundarySearchWork<Hint> {
    Candidate {
        case_id: ExploreCaseId,
        /// Canonically ordered, deduplicated metadata. Hints never affect the
        /// classification or closure of this CaseId.
        hints: Box<[Hint]>,
    },
    Fallback {
        case_id: ExploreCaseId,
    },
}

impl<Hint> BoundarySearchWork<Hint> {
    pub(super) fn case_id(&self) -> &ExploreCaseId {
        match self {
            Self::Candidate { case_id, .. } | Self::Fallback { case_id } => case_id,
        }
    }

    pub(super) fn candidate_hints(&self) -> Option<&[Hint]> {
        match self {
            Self::Candidate { hints, .. } => Some(hints),
            Self::Fallback { .. } => None,
        }
    }

    pub(super) fn is_candidate(&self) -> bool {
        matches!(self, Self::Candidate { .. })
    }
}

/// Whether work was produced, candidate results are intentionally awaited, or
/// no unscheduled canonical CaseId remains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundarySearchStep<Hint> {
    Work(BoundarySearchWork<Hint>),
    WaitingForCandidateEvaluations { pending: u128 },
    Exhausted,
}

impl<Hint> BoundarySearchStep<Hint> {
    pub(super) fn work(&self) -> Option<&BoundarySearchWork<Hint>> {
        match self {
            Self::Work(work) => Some(work),
            Self::WaitingForCandidateEvaluations { .. } | Self::Exhausted => None,
        }
    }

    pub(super) fn into_work(self) -> Option<BoundarySearchWork<Hint>> {
        match self {
            Self::Work(work) => Some(work),
            Self::WaitingForCandidateEvaluations { .. } | Self::Exhausted => None,
        }
    }
}

/// Exact, incrementally maintained scheduling and proof-cost accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoundarySearchCost {
    declared_cases: u128,
    eligible_cases: u128,
    structurally_outside_eligible_cases: u128,
    distinct_candidate_cases: u128,
    scheduled_candidates: u128,
    evaluated_candidates: u128,
    singleton_closed_cases: u128,
    certificate_closed_cases: u128,
    remaining_open_cases: u128,
    fallback_work: u128,
    scheduled_fallback: u128,
    evaluated_fallback: u128,
}

impl BoundarySearchCost {
    pub(super) fn declared_cases(self) -> u128 {
        self.declared_cases
    }

    /// Statically eligible lower-endpoint cases. Runtime constraints may still
    /// classify an eligible case as excluded.
    pub(super) fn eligible_cases(self) -> u128 {
        self.eligible_cases
    }

    /// Declared assignments outside the scheduler's eligible boundary slice.
    /// They remain conservative eligibility-open in `point_case_graph()`.
    pub(super) fn structurally_outside_eligible_cases(self) -> u128 {
        self.structurally_outside_eligible_cases
    }

    pub(super) fn distinct_candidate_cases(self) -> u128 {
        self.distinct_candidate_cases
    }

    pub(super) fn scheduled_candidates(self) -> u128 {
        self.scheduled_candidates
    }

    pub(super) fn evaluated_candidates(self) -> u128 {
        self.evaluated_candidates
    }

    pub(super) fn singleton_closed_cases(self) -> u128 {
        self.singleton_closed_cases
    }

    pub(super) fn certificate_closed_cases(self) -> u128 {
        self.certificate_closed_cases
    }

    pub(super) fn remaining_open_cases(self) -> u128 {
        self.remaining_open_cases
    }

    /// Open cases without a candidate hint, hence residual singleton work.
    pub(super) fn fallback_work(self) -> u128 {
        self.fallback_work
    }

    pub(super) fn scheduled_fallback(self) -> u128 {
        self.scheduled_fallback
    }

    pub(super) fn evaluated_fallback(self) -> u128 {
        self.evaluated_fallback
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkOrigin {
    Candidate,
    Fallback,
}

#[derive(Debug, Clone)]
enum IndexedCellState<Certificate> {
    Open,
    Singleton {
        classification: CaseTerminal,
    },
    Certified {
        classification: CaseTerminal,
        certificate: Certificate,
    },
}

impl<Certificate> IndexedCellState<Certificate> {
    fn classification(&self) -> Option<&CaseTerminal> {
        match self {
            Self::Open => None,
            Self::Singleton { classification } | Self::Certified { classification, .. } => {
                Some(classification)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct IndexedBoundaryCell<Certificate> {
    interval: BoundaryInterval,
    state: IndexedCellState<Certificate>,
}

impl<Certificate> IndexedBoundaryCell<Certificate> {
    fn open(interval: BoundaryInterval) -> Self {
        Self {
            interval,
            state: IndexedCellState::Open,
        }
    }

    fn singleton(interval: BoundaryInterval, classification: CaseTerminal) -> Self {
        Self {
            interval,
            state: IndexedCellState::Singleton { classification },
        }
    }

    fn certified(
        interval: BoundaryInterval,
        classification: CaseTerminal,
        certificate: Certificate,
    ) -> Self {
        Self {
            interval,
            state: IndexedCellState::Certified {
                classification,
                certificate,
            },
        }
    }

    fn is_open(&self) -> bool {
        matches!(&self.state, IndexedCellState::Open)
    }
}

struct CellReplacement<Certificate> {
    target_start: i64,
    replacement: Vec<IndexedBoundaryCell<Certificate>>,
}

struct IndexedPlanMutation<Certificate> {
    replacements: Vec<CellReplacement<Certificate>>,
    new_open: u128,
    new_singleton_closed: u128,
    new_certificate_closed: u128,
    singleton_delta: u128,
    certificate_delta: u128,
}

impl<Certificate> IndexedPlanMutation<Certificate> {
    fn newly_closed(&self) -> Result<u128, BoundarySearchError> {
        checked_add(self.singleton_delta, self.certificate_delta)
    }
}

/// Mutable monotone interval partition used only inside the scheduler.
/// Point lookup and one-cell splits are `O(log C)` for `C` current cells.
#[derive(Debug)]
struct IndexedProfileBoundaryPlan<Certificate> {
    declared: BoundaryInterval,
    cells: BTreeMap<i64, IndexedBoundaryCell<Certificate>>,
    open: u128,
    singleton_closed: u128,
    certificate_closed: u128,
}

impl<Certificate: Clone> IndexedProfileBoundaryPlan<Certificate> {
    fn new(declared: BoundaryInterval) -> Self {
        let mut cells = BTreeMap::new();
        if !declared.is_empty() {
            cells.insert(declared.start(), IndexedBoundaryCell::open(declared));
        }
        Self {
            declared,
            cells,
            open: declared.cardinality(),
            singleton_closed: 0,
            certificate_closed: 0,
        }
    }

    fn cell_containing(&self, point: i64) -> Option<&IndexedBoundaryCell<Certificate>> {
        self.cells
            .range(..=point)
            .next_back()
            .map(|(_, cell)| cell)
            .filter(|cell| cell.interval.contains(point))
    }

    fn prepare_singleton(
        &self,
        point: i64,
        classification: &CaseTerminal,
    ) -> Result<IndexedPlanMutation<Certificate>, BoundarySearchError> {
        let current = self
            .cell_containing(point)
            .ok_or(BoundarySearchError::InternalInvariant(
                "eligible point was absent from its indexed profile plan",
            ))?;
        if let Some(existing) = current.state.classification() {
            if existing != classification {
                return Err(BoundarySearchError::ConflictingClosedClassification {
                    interval: current.interval,
                    existing: existing.clone(),
                    proposed: classification.clone(),
                });
            }
            return Ok(IndexedPlanMutation {
                replacements: Vec::new(),
                new_open: self.open,
                new_singleton_closed: self.singleton_closed,
                new_certificate_closed: self.certificate_closed,
                singleton_delta: 0,
                certificate_delta: 0,
            });
        }

        let point_end = point
            .checked_add(1)
            .ok_or(BoundarySearchError::InternalInvariant(
                "an eligible point had no representable exclusive successor",
            ))?;
        let singleton = BoundaryInterval::new(point, point_end)?;
        let mut replacement = Vec::with_capacity(3);
        if current.interval.start() < point {
            replacement.push(IndexedBoundaryCell::open(BoundaryInterval::new(
                current.interval.start(),
                point,
            )?));
        }
        replacement.push(IndexedBoundaryCell::singleton(
            singleton,
            classification.clone(),
        ));
        if point_end < current.interval.end_exclusive() {
            replacement.push(IndexedBoundaryCell::open(BoundaryInterval::new(
                point_end,
                current.interval.end_exclusive(),
            )?));
        }
        Ok(IndexedPlanMutation {
            replacements: vec![CellReplacement {
                target_start: current.interval.start(),
                replacement,
            }],
            new_open: self.open.checked_sub(1).ok_or(
                BoundarySearchError::CardinalityConservation {
                    eligible: self.declared.cardinality(),
                    singleton_closed: self.singleton_closed,
                    certificate_closed: self.certificate_closed,
                    open: self.open,
                },
            )?,
            new_singleton_closed: checked_add(self.singleton_closed, 1)?,
            new_certificate_closed: self.certificate_closed,
            singleton_delta: 1,
            certificate_delta: 0,
        })
    }

    fn prepare_region(
        &self,
        interval: BoundaryInterval,
        classification: &CaseTerminal,
        certificate: &Certificate,
    ) -> Result<IndexedPlanMutation<Certificate>, BoundarySearchError> {
        let affected = self.overlapping_cells(interval);
        if affected.is_empty() {
            return Err(BoundarySearchError::InternalInvariant(
                "an in-domain certificate did not intersect its indexed plan",
            ));
        }

        let mut replacements = Vec::new();
        let mut certificate_delta = 0_u128;
        for current in affected {
            if let Some(existing) = current.state.classification() {
                if existing != classification {
                    return Err(BoundarySearchError::ConflictingClosedClassification {
                        interval: interval_intersection(current.interval, interval)?,
                        existing: existing.clone(),
                        proposed: classification.clone(),
                    });
                }
                continue;
            }

            let overlap = interval_intersection(current.interval, interval)?;
            certificate_delta = checked_add(certificate_delta, overlap.cardinality())?;
            let mut replacement = Vec::with_capacity(3);
            if current.interval.start() < overlap.start() {
                replacement.push(IndexedBoundaryCell::open(BoundaryInterval::new(
                    current.interval.start(),
                    overlap.start(),
                )?));
            }
            replacement.push(IndexedBoundaryCell::certified(
                overlap,
                classification.clone(),
                certificate.clone(),
            ));
            if overlap.end_exclusive() < current.interval.end_exclusive() {
                replacement.push(IndexedBoundaryCell::open(BoundaryInterval::new(
                    overlap.end_exclusive(),
                    current.interval.end_exclusive(),
                )?));
            }
            replacements.push(CellReplacement {
                target_start: current.interval.start(),
                replacement,
            });
        }

        Ok(IndexedPlanMutation {
            replacements,
            new_open: self.open.checked_sub(certificate_delta).ok_or(
                BoundarySearchError::CardinalityConservation {
                    eligible: self.declared.cardinality(),
                    singleton_closed: self.singleton_closed,
                    certificate_closed: self.certificate_closed,
                    open: self.open,
                },
            )?,
            new_singleton_closed: self.singleton_closed,
            new_certificate_closed: checked_add(self.certificate_closed, certificate_delta)?,
            singleton_delta: 0,
            certificate_delta,
        })
    }

    fn overlapping_cells(
        &self,
        interval: BoundaryInterval,
    ) -> Vec<IndexedBoundaryCell<Certificate>> {
        let mut affected = Vec::new();
        if let Some((_, cell)) = self.cells.range(..=interval.start()).next_back() {
            if intervals_overlap(cell.interval, interval) {
                affected.push(cell.clone());
            }
        }
        for (_, cell) in self.cells.range(interval.start()..interval.end_exclusive()) {
            if affected
                .last()
                .is_some_and(|prior| prior.interval.start() == cell.interval.start())
            {
                continue;
            }
            if intervals_overlap(cell.interval, interval) {
                affected.push(cell.clone());
            }
        }
        affected
    }

    /// All fallible checks and arithmetic happen while preparing a mutation.
    /// Applying it performs only deterministic map replacement and assignments.
    fn apply(&mut self, mutation: IndexedPlanMutation<Certificate>) {
        for refinement in mutation.replacements {
            let removed = self.cells.remove(&refinement.target_start);
            debug_assert!(removed.is_some());
            for cell in refinement.replacement {
                let replaced = self.cells.insert(cell.interval.start(), cell);
                debug_assert!(replaced.is_none());
            }
        }
        self.open = mutation.new_open;
        self.singleton_closed = mutation.new_singleton_closed;
        self.certificate_closed = mutation.new_certificate_closed;
    }

    fn snapshot(&self) -> Result<ProfileBoundaryPlan<Certificate>, BoundarySearchError> {
        let mut snapshot = BoundaryPlan::new(self.declared.start(), self.declared.end_exclusive())?;
        if self.declared.is_empty() {
            return Ok(snapshot);
        }
        let replacement = self
            .cells
            .values()
            .map(|cell| match &cell.state {
                IndexedCellState::Open => BoundaryCell::open(cell.interval),
                IndexedCellState::Singleton { classification } => BoundaryCell::closed(
                    cell.interval,
                    classification.clone(),
                    None::<Infallible>,
                    BoundaryClosureCertificate::SingletonEvaluation,
                ),
                IndexedCellState::Certified {
                    classification,
                    certificate,
                } => BoundaryCell::closed(
                    cell.interval,
                    classification.clone(),
                    None::<Infallible>,
                    BoundaryClosureCertificate::Region(certificate.clone()),
                ),
            })
            .collect::<Vec<_>>();
        snapshot.refine_open(self.declared, replacement)?;
        Ok(snapshot)
    }

    fn audit(&self) -> Result<(), BoundarySearchError> {
        if self.declared.is_empty() {
            if !self.cells.is_empty()
                || self.open != 0
                || self.singleton_closed != 0
                || self.certificate_closed != 0
            {
                return Err(BoundarySearchError::InternalInvariant(
                    "an empty indexed profile plan retained evidence",
                ));
            }
            return Ok(());
        }

        let mut expected_start = self.declared.start();
        let mut open = 0_u128;
        let mut singleton = 0_u128;
        let mut certified = 0_u128;
        for (&start, cell) in &self.cells {
            if start != cell.interval.start() || start != expected_start || cell.interval.is_empty()
            {
                return Err(BoundarySearchError::InternalInvariant(
                    "indexed profile cells are not an ordered exact cover",
                ));
            }
            let cardinality = cell.interval.cardinality();
            match &cell.state {
                IndexedCellState::Open => open = checked_add(open, cardinality)?,
                IndexedCellState::Singleton { .. } => {
                    if cardinality != 1 {
                        return Err(BoundarySearchError::InternalInvariant(
                            "singleton evaluation evidence spans more than one point",
                        ));
                    }
                    singleton = checked_add(singleton, cardinality)?;
                }
                IndexedCellState::Certified { .. } => {
                    certified = checked_add(certified, cardinality)?;
                }
            }
            expected_start = cell.interval.end_exclusive();
        }
        if expected_start != self.declared.end_exclusive()
            || open != self.open
            || singleton != self.singleton_closed
            || certified != self.certificate_closed
            || checked_add(checked_add(open, singleton)?, certified)? != self.declared.cardinality()
        {
            return Err(BoundarySearchError::InternalInvariant(
                "indexed profile cardinality counters do not conserve the declared interval",
            ));
        }
        self.snapshot()?.validate()?;
        Ok(())
    }
}

/// Lazy candidate-first scheduler over full canonical Explore CaseIds.
#[derive(Debug)]
pub(super) struct CandidateFirstBoundarySearch<Hint, Certificate> {
    axis_cardinalities: Box<[u128]>,
    boundary_dimension: usize,
    declared_boundary: BoundaryInterval,
    eligible_boundary: BoundaryInterval,
    declared_cases: u128,
    eligible_cases: u128,
    candidates: BTreeMap<ExploreCaseId, BTreeSet<Hint>>,
    candidate_order: Box<[ExploreCaseId]>,
    next_candidate_index: usize,
    candidates_by_profile: BTreeMap<Box<[u128]>, BTreeMap<i64, ExploreCaseId>>,
    open_candidates: BTreeSet<ExploreCaseId>,
    open_candidate_cases: u128,
    plans: BTreeMap<Box<[u128]>, IndexedProfileBoundaryPlan<Certificate>>,
    evaluated_cases: BTreeMap<ExploreCaseId, CaseTerminal>,
    pending: BTreeMap<ExploreCaseId, WorkOrigin>,
    scheduled_candidates: BTreeSet<ExploreCaseId>,
    evaluated_candidates: BTreeSet<ExploreCaseId>,
    scheduled_fallback: BTreeSet<ExploreCaseId>,
    evaluated_fallback: BTreeSet<ExploreCaseId>,
    scheduled_candidate_cases: u128,
    evaluated_candidate_cases: u128,
    scheduled_fallback_cases: u128,
    evaluated_fallback_cases: u128,
    singleton_closed_cases: u128,
    certificate_closed_cases: u128,
    remaining_open_cases: u128,
    fallback_cursor: CanonicalEligibleCursor,
}

impl<Hint, Certificate> CandidateFirstBoundarySearch<Hint, Certificate>
where
    Hint: Clone + Ord,
    Certificate: Clone,
{
    pub(super) fn new(
        axis_cardinalities: impl Into<Box<[u128]>>,
        boundary_dimension: usize,
        declared_boundary: BoundaryInterval,
        eligible_boundary: BoundaryInterval,
        candidates: impl IntoIterator<Item = BoundarySearchCandidate<Hint>>,
    ) -> Result<Self, BoundarySearchError> {
        let axis_cardinalities = axis_cardinalities.into();
        let boundary_cardinality = axis_cardinalities.get(boundary_dimension).copied().ok_or(
            BoundarySearchError::BoundaryDimensionOutOfBounds {
                dimension: boundary_dimension,
                axis_count: axis_cardinalities.len(),
            },
        )?;
        if boundary_cardinality != declared_boundary.cardinality() {
            return Err(BoundarySearchError::BoundaryCardinalityMismatch {
                axis: boundary_cardinality,
                declared: declared_boundary.cardinality(),
            });
        }
        if !eligible_boundary.is_within(declared_boundary) {
            return Err(BoundarySearchError::EligibleBoundaryOutsideDeclared {
                declared: declared_boundary,
                eligible: eligible_boundary,
            });
        }

        let declared_cases = checked_product(&axis_cardinalities)?;
        let outer_cardinalities = axis_cardinalities
            .iter()
            .enumerate()
            .filter_map(|(dimension, cardinality)| {
                (dimension != boundary_dimension).then_some(*cardinality)
            })
            .collect::<Vec<_>>();
        let eligible_cases = if eligible_boundary.is_empty() || outer_cardinalities.contains(&0) {
            0
        } else {
            checked_product(&outer_cardinalities)?
                .checked_mul(eligible_boundary.cardinality())
                .ok_or(BoundarySearchError::CardinalityOverflow)?
        };
        let eligible_start_ordinal =
            boundary_offset(declared_boundary.start(), eligible_boundary.start())?;
        let eligible_end_ordinal =
            boundary_offset(declared_boundary.start(), eligible_boundary.end_exclusive())?;
        let fallback_cursor = CanonicalEligibleCursor::new(
            axis_cardinalities.clone(),
            boundary_dimension,
            eligible_start_ordinal,
            eligible_end_ordinal,
        );

        let mut search = Self {
            axis_cardinalities,
            boundary_dimension,
            declared_boundary,
            eligible_boundary,
            declared_cases,
            eligible_cases,
            candidates: BTreeMap::new(),
            candidate_order: Vec::new().into_boxed_slice(),
            next_candidate_index: 0,
            candidates_by_profile: BTreeMap::new(),
            open_candidates: BTreeSet::new(),
            open_candidate_cases: 0,
            plans: BTreeMap::new(),
            evaluated_cases: BTreeMap::new(),
            pending: BTreeMap::new(),
            scheduled_candidates: BTreeSet::new(),
            evaluated_candidates: BTreeSet::new(),
            scheduled_fallback: BTreeSet::new(),
            evaluated_fallback: BTreeSet::new(),
            scheduled_candidate_cases: 0,
            evaluated_candidate_cases: 0,
            scheduled_fallback_cases: 0,
            evaluated_fallback_cases: 0,
            singleton_closed_cases: 0,
            certificate_closed_cases: 0,
            remaining_open_cases: eligible_cases,
            fallback_cursor,
        };
        for candidate in candidates {
            let BoundarySearchCandidate {
                outer_ordinals,
                boundary_value,
                hint,
            } = candidate;
            let case_id = search.case_id_for_outer(&outer_ordinals, boundary_value)?;
            search
                .candidates
                .entry(case_id.clone())
                .or_default()
                .insert(hint);
            let previous = search
                .candidates_by_profile
                .entry(outer_ordinals)
                .or_default()
                .insert(boundary_value, case_id.clone());
            if previous
                .as_ref()
                .is_some_and(|previous| previous != &case_id)
            {
                return Err(BoundarySearchError::InternalInvariant(
                    "one profile boundary value produced multiple CaseIds",
                ));
            }
        }
        search.candidate_order = search.candidates.keys().cloned().collect::<Vec<_>>().into();
        search.open_candidates = search.candidate_order.iter().cloned().collect();
        search.open_candidate_cases = usize_count(search.open_candidates.len())?;
        search.cost_ledger()?;
        Ok(search)
    }

    pub(super) fn axis_cardinalities(&self) -> &[u128] {
        &self.axis_cardinalities
    }

    pub(super) fn boundary_dimension(&self) -> usize {
        self.boundary_dimension
    }

    pub(super) fn declared_boundary(&self) -> BoundaryInterval {
        self.declared_boundary
    }

    pub(super) fn eligible_boundary(&self) -> BoundaryInterval {
        self.eligible_boundary
    }

    pub(super) fn candidate_case_ids(&self) -> impl Iterator<Item = &ExploreCaseId> {
        self.candidate_order.iter()
    }

    pub(super) fn candidate_hints(&self, case_id: &ExploreCaseId) -> Option<&BTreeSet<Hint>> {
        self.candidates.get(case_id)
    }

    /// Canonically ordered point evidence. Representative selection must use
    /// CaseId order and proof closure, never evaluation/discovery order.
    pub(super) fn evaluated_cases(&self) -> &BTreeMap<ExploreCaseId, CaseTerminal> {
        &self.evaluated_cases
    }

    /// Export one profile as the ordinary proof-plan representation. An
    /// untouched valid profile exports as one fully open cell. This is an
    /// explicit `O(C)` audit/export operation, never part of singleton search.
    pub(super) fn profile_plan_snapshot(
        &self,
        outer_ordinals: &[u128],
    ) -> Result<ProfileBoundaryPlan<Certificate>, BoundarySearchError> {
        self.validate_outer_path(outer_ordinals)?;
        match self.plans.get(outer_ordinals) {
            Some(plan) => plan.snapshot(),
            None => {
                IndexedProfileBoundaryPlan::<Certificate>::new(self.eligible_boundary).snapshot()
            }
        }
    }

    /// Each candidate is considered once in canonical CaseId order. Fallback
    /// waits for scheduled candidates, allowing their results to drive region
    /// certification before residual singleton work is committed.
    pub(super) fn next_work(&mut self) -> Result<BoundarySearchStep<Hint>, BoundarySearchError> {
        let mut scan = self.next_candidate_index;
        while scan < self.candidate_order.len() {
            let case_id = self.candidate_order[scan].clone();
            scan += 1;
            if !self.point_is_open(&case_id)? {
                continue;
            }
            if self.scheduled_candidates.contains(&case_id)
                || self.scheduled_fallback.contains(&case_id)
                || self.pending.contains_key(&case_id)
                || self.evaluated_cases.contains_key(&case_id)
            {
                return Err(BoundarySearchError::InternalInvariant(
                    "the one-pass candidate cursor reached already-known work",
                ));
            }
            let scheduled_candidate_cases = checked_add(self.scheduled_candidate_cases, 1)?;
            let hints = self
                .candidates
                .get(&case_id)
                .ok_or(BoundarySearchError::InternalInvariant(
                    "candidate order referenced absent hint metadata",
                ))?
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            self.next_candidate_index = scan;
            self.scheduled_candidate_cases = scheduled_candidate_cases;
            let newly_scheduled = self.scheduled_candidates.insert(case_id.clone());
            let replaced_pending = self
                .pending
                .insert(case_id.clone(), WorkOrigin::Candidate)
                .is_some();
            debug_assert!(newly_scheduled);
            debug_assert!(!replaced_pending);
            return Ok(BoundarySearchStep::Work(BoundarySearchWork::Candidate {
                case_id,
                hints,
            }));
        }
        let pending_candidates = self
            .scheduled_candidate_cases
            .checked_sub(self.evaluated_candidate_cases)
            .ok_or(BoundarySearchError::InternalInvariant(
                "evaluated candidate count exceeds scheduled candidate count",
            ))?;
        if pending_candidates != 0 {
            self.next_candidate_index = scan;
            return Ok(BoundarySearchStep::WaitingForCandidateEvaluations {
                pending: pending_candidates,
            });
        }

        let mut fallback_cursor = self.fallback_cursor.clone();
        while let Some(case_id) = fallback_cursor.next() {
            if self.pending.contains_key(&case_id) {
                continue;
            }
            if !self.point_is_open(&case_id)? {
                if self.boundary_dimension + 1 == self.axis_cardinalities.len() {
                    let end_exclusive = self.closed_cell_end(&case_id)?.ok_or(
                        BoundarySearchError::InternalInvariant(
                            "a non-open fallback point had no closed interval",
                        ),
                    )?;
                    fallback_cursor.skip_current_boundary_interval(boundary_offset(
                        self.declared_boundary.start(),
                        end_exclusive,
                    )?)?;
                }
                continue;
            }
            if self.scheduled_candidates.contains(&case_id)
                || self.scheduled_fallback.contains(&case_id)
                || self.evaluated_cases.contains_key(&case_id)
            {
                return Err(BoundarySearchError::InternalInvariant(
                    "the canonical fallback cursor reached already-known work",
                ));
            }
            let scheduled_fallback_cases = checked_add(self.scheduled_fallback_cases, 1)?;
            self.next_candidate_index = scan;
            self.fallback_cursor = fallback_cursor;
            self.scheduled_fallback_cases = scheduled_fallback_cases;
            let newly_scheduled = self.scheduled_fallback.insert(case_id.clone());
            let replaced_pending = self
                .pending
                .insert(case_id.clone(), WorkOrigin::Fallback)
                .is_some();
            debug_assert!(newly_scheduled);
            debug_assert!(!replaced_pending);
            return Ok(BoundarySearchStep::Work(BoundarySearchWork::Fallback {
                case_id,
            }));
        }
        self.next_candidate_index = scan;
        self.fallback_cursor = fallback_cursor;
        Ok(BoundarySearchStep::Exhausted)
    }

    /// Record one scheduled normal-runtime classification. All conflict and
    /// arithmetic checks are completed before the indexed plan or ledgers are
    /// mutated, so an error leaves the scheduler unchanged.
    pub(super) fn record_evaluation(
        &mut self,
        case_id: ExploreCaseId,
        classification: CaseTerminal,
    ) -> Result<(), BoundarySearchError> {
        if !terminal_is_closed(&classification) {
            return Err(BoundarySearchError::OpenClassificationCannotClose);
        }
        let origin = self.pending.get(&case_id).copied().ok_or_else(|| {
            BoundarySearchError::EvaluationWasNotScheduled {
                case_id: case_id.clone(),
            }
        })?;
        if self.evaluated_cases.contains_key(&case_id)
            || self.evaluated_candidates.contains(&case_id)
            || self.evaluated_fallback.contains(&case_id)
        {
            return Err(BoundarySearchError::InternalInvariant(
                "pending work was already recorded as evaluated",
            ));
        }
        let (outer_ordinals, boundary_value) = self.split_case_id(&case_id)?;
        let mutation = match self.plans.get(outer_ordinals.as_ref()) {
            Some(plan) => plan.prepare_singleton(boundary_value, &classification)?,
            None => IndexedProfileBoundaryPlan::<Certificate>::new(self.eligible_boundary)
                .prepare_singleton(boundary_value, &classification)?,
        };
        let newly_closed = mutation.newly_closed()?;
        let singleton_closed_cases =
            checked_add(self.singleton_closed_cases, mutation.singleton_delta)?;
        let certificate_closed_cases =
            checked_add(self.certificate_closed_cases, mutation.certificate_delta)?;
        let remaining_open_cases = self.remaining_open_cases.checked_sub(newly_closed).ok_or(
            BoundarySearchError::CardinalityConservation {
                eligible: self.eligible_cases,
                singleton_closed: self.singleton_closed_cases,
                certificate_closed: self.certificate_closed_cases,
                open: self.remaining_open_cases,
            },
        )?;
        let closes_open_candidate = newly_closed != 0 && self.open_candidates.contains(&case_id);
        if newly_closed != 0 && self.candidates.contains_key(&case_id) && !closes_open_candidate {
            return Err(BoundarySearchError::InternalInvariant(
                "an open candidate singleton was absent from the open-candidate index",
            ));
        }
        let open_candidate_cases = if closes_open_candidate {
            self.open_candidate_cases.checked_sub(1).ok_or(
                BoundarySearchError::InternalInvariant("open candidate set and counter disagree"),
            )?
        } else {
            self.open_candidate_cases
        };
        let (evaluated_candidate_cases, evaluated_fallback_cases) = match origin {
            WorkOrigin::Candidate => (
                checked_add(self.evaluated_candidate_cases, 1)?,
                self.evaluated_fallback_cases,
            ),
            WorkOrigin::Fallback => (
                self.evaluated_candidate_cases,
                checked_add(self.evaluated_fallback_cases, 1)?,
            ),
        };

        self.plans
            .entry(outer_ordinals)
            .or_insert_with(|| IndexedProfileBoundaryPlan::new(self.eligible_boundary))
            .apply(mutation);
        if closes_open_candidate {
            self.open_candidates.remove(&case_id);
        }
        self.open_candidate_cases = open_candidate_cases;
        self.singleton_closed_cases = singleton_closed_cases;
        self.certificate_closed_cases = certificate_closed_cases;
        self.remaining_open_cases = remaining_open_cases;
        self.evaluated_candidate_cases = evaluated_candidate_cases;
        self.evaluated_fallback_cases = evaluated_fallback_cases;
        self.pending.remove(&case_id);
        self.evaluated_cases.insert(case_id.clone(), classification);
        match origin {
            WorkOrigin::Candidate => {
                self.evaluated_candidates.insert(case_id);
            }
            WorkOrigin::Fallback => {
                self.evaluated_fallback.insert(case_id);
            }
        }
        Ok(())
    }

    /// Close the open portions of one exact profile interval. Existing closed
    /// evidence is immutable and must agree. Candidate hints and similar probe
    /// observations never invoke this method implicitly.
    pub(super) fn certify_region(
        &mut self,
        outer_ordinals: &[u128],
        interval: BoundaryInterval,
        classification: CaseTerminal,
        certificate: Certificate,
    ) -> Result<(), BoundarySearchError> {
        self.validate_outer_path(outer_ordinals)?;
        if interval.is_empty() {
            return Err(BoundarySearchError::EmptyCertificateInterval);
        }
        if !interval.is_within(self.eligible_boundary) {
            return Err(BoundarySearchError::CertificateOutsideEligible {
                eligible: self.eligible_boundary,
                interval,
            });
        }
        if !terminal_is_closed(&classification) {
            return Err(BoundarySearchError::OpenClassificationCannotClose);
        }

        let outer_ordinals = outer_ordinals.to_vec().into_boxed_slice();
        let mutation = match self.plans.get(outer_ordinals.as_ref()) {
            Some(plan) => plan.prepare_region(interval, &classification, &certificate)?,
            None => IndexedProfileBoundaryPlan::new(self.eligible_boundary).prepare_region(
                interval,
                &classification,
                &certificate,
            )?,
        };
        let newly_closed = mutation.newly_closed()?;
        let singleton_closed_cases =
            checked_add(self.singleton_closed_cases, mutation.singleton_delta)?;
        let certificate_closed_cases =
            checked_add(self.certificate_closed_cases, mutation.certificate_delta)?;
        let remaining_open_cases = self.remaining_open_cases.checked_sub(newly_closed).ok_or(
            BoundarySearchError::CardinalityConservation {
                eligible: self.eligible_cases,
                singleton_closed: self.singleton_closed_cases,
                certificate_closed: self.certificate_closed_cases,
                open: self.remaining_open_cases,
            },
        )?;
        let closing_candidates = self
            .candidates_by_profile
            .get(outer_ordinals.as_ref())
            .map(|profile| {
                profile
                    .range(interval.start()..interval.end_exclusive())
                    .filter_map(|(_, case_id)| {
                        self.open_candidates
                            .contains(case_id)
                            .then_some(case_id.clone())
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let closing_candidate_count = usize_count(closing_candidates.len())?;
        if closing_candidate_count > newly_closed {
            return Err(BoundarySearchError::InternalInvariant(
                "region closure found more open candidates than newly closed points",
            ));
        }
        let open_candidate_cases = self
            .open_candidate_cases
            .checked_sub(closing_candidate_count)
            .ok_or(BoundarySearchError::InternalInvariant(
                "region closure removed more candidates than remain open",
            ))?;

        self.plans
            .entry(outer_ordinals)
            .or_insert_with(|| IndexedProfileBoundaryPlan::new(self.eligible_boundary))
            .apply(mutation);
        for case_id in closing_candidates {
            self.open_candidates.remove(&case_id);
        }
        self.open_candidate_cases = open_candidate_cases;
        self.singleton_closed_cases = singleton_closed_cases;
        self.certificate_closed_cases = certificate_closed_cases;
        self.remaining_open_cases = remaining_open_cases;
        Ok(())
    }

    /// Build singleton evidence only. Region-certified and structurally
    /// ineligible but unmaterialized assignments deliberately remain
    /// `EligibilityOpen(EvaluationUnknown)` in this graph.
    pub(super) fn point_case_graph(&self) -> Result<CaseDecisionDag, BoundarySearchError> {
        OrderedDecisionDag::from_sparse_classifications(
            self.axis_cardinalities.to_vec(),
            self.evaluated_cases
                .iter()
                .map(|(case_id, terminal)| (case_id.ordinals().to_vec(), terminal.clone())),
            CaseTerminal::EligibilityOpen(CaseOpenReason::EvaluationUnknown),
        )
        .map_err(BoundarySearchError::CaseGraph)
    }

    /// Lower every materialized singleton and proof-certified interval into
    /// the canonical case DAG without expanding an interval to CaseIds.
    /// Structurally ineligible boundary endpoints are exact `Excluded`
    /// rectangles across every outer profile. Untouched eligible support stays
    /// explicitly open.
    pub(super) fn certified_case_graph(
        &self,
        open_reason: CaseOpenReason,
        open_point_evidence: impl IntoIterator<Item = (ExploreCaseId, CaseTerminal)>,
    ) -> Result<CaseDecisionDag, BoundarySearchError> {
        if self.declared_cases == 0 {
            return lower_certified_case_regions(
                self.axis_cardinalities.to_vec(),
                std::iter::empty::<CertifiedCaseRegion<()>>(),
                open_reason,
            )
            .map(|lowering| lowering.into_case_graph())
            .map_err(|error| BoundarySearchError::CertifiedRegionLowering(error.to_string()));
        }
        let mut regions = Vec::<CertifiedCaseRegion<()>>::new();

        for interval in [
            BoundaryInterval::new(
                self.declared_boundary.start(),
                self.eligible_boundary.start(),
            )?,
            BoundaryInterval::new(
                self.eligible_boundary.end_exclusive(),
                self.declared_boundary.end_exclusive(),
            )?,
        ] {
            if interval.is_empty() {
                continue;
            }
            regions.push(CertifiedCaseRegion::rectangle(
                self.full_outer_rectangle(interval)?,
                CaseTerminal::Excluded,
                (),
            ));
        }

        for (outer_ordinals, plan) in &self.plans {
            self.validate_outer_path(outer_ordinals)?;
            for cell in plan.cells.values() {
                let Some(classification) = cell.state.classification() else {
                    continue;
                };
                regions.push(CertifiedCaseRegion::rectangle(
                    self.profile_rectangle(outer_ordinals, cell.interval)?,
                    classification.clone(),
                    (),
                ));
            }
        }

        for (case_id, classification) in open_point_evidence {
            if terminal_is_closed(&classification) {
                return Err(BoundarySearchError::InternalInvariant(
                    "open point evidence carried a closed classification",
                ));
            }
            self.validate_full_case_id(&case_id)?;
            let (outer_ordinals, boundary_value) = self.split_case_id(&case_id)?;
            if !self.point_is_open(&case_id)? {
                return Err(BoundarySearchError::InternalInvariant(
                    "open point evidence overlaps a closed scheduler cell",
                ));
            }
            let end_exclusive = boundary_value
                .checked_add(1)
                .ok_or(BoundarySearchError::CardinalityOverflow)?;
            regions.push(CertifiedCaseRegion::rectangle(
                self.profile_rectangle(
                    &outer_ordinals,
                    BoundaryInterval::new(boundary_value, end_exclusive)?,
                )?,
                classification,
                (),
            ));
        }

        lower_certified_case_regions(self.axis_cardinalities.to_vec(), regions, open_reason)
            .map(|lowering| lowering.into_case_graph())
            .map_err(|error| BoundarySearchError::CertifiedRegionLowering(error.to_string()))
    }

    fn full_outer_rectangle(
        &self,
        boundary: BoundaryInterval,
    ) -> Result<Vec<CertifiedOrdinalInterval>, BoundarySearchError> {
        self.axis_cardinalities
            .iter()
            .enumerate()
            .map(|(dimension, &cardinality)| {
                if dimension == self.boundary_dimension {
                    self.boundary_ordinal_interval(boundary)
                } else {
                    CertifiedOrdinalInterval::new(0, cardinality).map_err(|error| {
                        BoundarySearchError::CertifiedRegionLowering(error.to_string())
                    })
                }
            })
            .collect()
    }

    fn profile_rectangle(
        &self,
        outer_ordinals: &[u128],
        boundary: BoundaryInterval,
    ) -> Result<Vec<CertifiedOrdinalInterval>, BoundarySearchError> {
        let mut outer_index = 0_usize;
        self.axis_cardinalities
            .iter()
            .enumerate()
            .map(|(dimension, _)| {
                if dimension == self.boundary_dimension {
                    self.boundary_ordinal_interval(boundary)
                } else {
                    let ordinal = *outer_ordinals.get(outer_index).ok_or(
                        BoundarySearchError::InternalInvariant(
                            "validated outer profile lost an ordinal during DAG lowering",
                        ),
                    )?;
                    outer_index += 1;
                    let end_exclusive = ordinal
                        .checked_add(1)
                        .ok_or(BoundarySearchError::CardinalityOverflow)?;
                    CertifiedOrdinalInterval::new(ordinal, end_exclusive).map_err(|error| {
                        BoundarySearchError::CertifiedRegionLowering(error.to_string())
                    })
                }
            })
            .collect()
    }

    fn boundary_ordinal_interval(
        &self,
        interval: BoundaryInterval,
    ) -> Result<CertifiedOrdinalInterval, BoundarySearchError> {
        let start = boundary_offset(self.declared_boundary.start(), interval.start())?;
        let end_exclusive =
            boundary_offset(self.declared_boundary.start(), interval.end_exclusive())?;
        CertifiedOrdinalInterval::new(start, end_exclusive)
            .map_err(|error| BoundarySearchError::CertifiedRegionLowering(error.to_string()))
    }

    /// Constant-time checked accounting. Use [`Self::audit`] for a full scan of
    /// interval structure, candidate indexes, and work ledgers.
    pub(super) fn cost_ledger(&self) -> Result<BoundarySearchCost, BoundarySearchError> {
        let closed = checked_add(self.singleton_closed_cases, self.certificate_closed_cases)?;
        if checked_add(closed, self.remaining_open_cases)? != self.eligible_cases {
            return Err(BoundarySearchError::CardinalityConservation {
                eligible: self.eligible_cases,
                singleton_closed: self.singleton_closed_cases,
                certificate_closed: self.certificate_closed_cases,
                open: self.remaining_open_cases,
            });
        }
        if self.evaluated_candidate_cases > self.scheduled_candidate_cases
            || self.evaluated_fallback_cases > self.scheduled_fallback_cases
        {
            return Err(BoundarySearchError::InternalInvariant(
                "evaluated work exceeds scheduled work",
            ));
        }
        let fallback_work = self
            .remaining_open_cases
            .checked_sub(self.open_candidate_cases)
            .ok_or(BoundarySearchError::InternalInvariant(
                "open candidate support exceeds remaining open support",
            ))?;
        Ok(BoundarySearchCost {
            declared_cases: self.declared_cases,
            eligible_cases: self.eligible_cases,
            structurally_outside_eligible_cases: self
                .declared_cases
                .checked_sub(self.eligible_cases)
                .ok_or(BoundarySearchError::InternalInvariant(
                    "eligible support exceeds the declared universe",
                ))?,
            distinct_candidate_cases: usize_count(self.candidates.len())?,
            scheduled_candidates: self.scheduled_candidate_cases,
            evaluated_candidates: self.evaluated_candidate_cases,
            singleton_closed_cases: self.singleton_closed_cases,
            certificate_closed_cases: self.certificate_closed_cases,
            remaining_open_cases: self.remaining_open_cases,
            fallback_work,
            scheduled_fallback: self.scheduled_fallback_cases,
            evaluated_fallback: self.evaluated_fallback_cases,
        })
    }

    /// Explicit full structural audit. This is intentionally absent from the
    /// per-singleton hot path.
    pub(super) fn audit(&self) -> Result<BoundarySearchCost, BoundarySearchError> {
        let cost = self.cost_ledger()?;
        if usize_count(self.scheduled_candidates.len())? != self.scheduled_candidate_cases
            || usize_count(self.evaluated_candidates.len())? != self.evaluated_candidate_cases
            || usize_count(self.scheduled_fallback.len())? != self.scheduled_fallback_cases
            || usize_count(self.evaluated_fallback.len())? != self.evaluated_fallback_cases
            || usize_count(self.open_candidates.len())? != self.open_candidate_cases
        {
            return Err(BoundarySearchError::InternalInvariant(
                "incremental work counters disagree with their identity sets",
            ));
        }
        if !self
            .evaluated_candidates
            .is_subset(&self.scheduled_candidates)
            || !self.evaluated_fallback.is_subset(&self.scheduled_fallback)
            || !self
                .scheduled_candidates
                .is_disjoint(&self.scheduled_fallback)
            || self
                .pending
                .keys()
                .any(|case_id| self.evaluated_cases.contains_key(case_id))
        {
            return Err(BoundarySearchError::InternalInvariant(
                "scheduled, pending and evaluated identity sets disagree",
            ));
        }
        let mut pending_candidate_cases = 0_u128;
        let mut pending_fallback_cases = 0_u128;
        for (case_id, origin) in &self.pending {
            let scheduled = match origin {
                WorkOrigin::Candidate => {
                    pending_candidate_cases = checked_add(pending_candidate_cases, 1)?;
                    self.scheduled_candidates.contains(case_id)
                }
                WorkOrigin::Fallback => {
                    pending_fallback_cases = checked_add(pending_fallback_cases, 1)?;
                    self.scheduled_fallback.contains(case_id)
                }
            };
            if !scheduled {
                return Err(BoundarySearchError::InternalInvariant(
                    "pending work lacks its scheduled-work entry",
                ));
            }
        }
        if checked_add(self.evaluated_candidate_cases, pending_candidate_cases)?
            != self.scheduled_candidate_cases
            || checked_add(self.evaluated_fallback_cases, pending_fallback_cases)?
                != self.scheduled_fallback_cases
            || usize_count(self.evaluated_cases.len())?
                != checked_add(
                    self.evaluated_candidate_cases,
                    self.evaluated_fallback_cases,
                )?
            || self.evaluated_cases.keys().any(|case_id| {
                !self.evaluated_candidates.contains(case_id)
                    && !self.evaluated_fallback.contains(case_id)
            })
            || self
                .evaluated_candidates
                .iter()
                .chain(&self.evaluated_fallback)
                .any(|case_id| !self.evaluated_cases.contains_key(case_id))
        {
            return Err(BoundarySearchError::InternalInvariant(
                "scheduled work is not partitioned exactly into pending and evaluated work",
            ));
        }
        if self.candidate_order.as_ref()
            != self
                .candidates
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .as_slice()
            || self.next_candidate_index > self.candidate_order.len()
        {
            return Err(BoundarySearchError::InternalInvariant(
                "canonical candidate order or cursor is invalid",
            ));
        }
        let mut indexed_candidates = 0_u128;
        for (outer_ordinals, profile) in &self.candidates_by_profile {
            self.validate_outer_path(outer_ordinals)?;
            for (&boundary_value, case_id) in profile {
                let expected = self.case_id_for_outer(outer_ordinals, boundary_value)?;
                if &expected != case_id || !self.candidates.contains_key(case_id) {
                    return Err(BoundarySearchError::InternalInvariant(
                        "candidate profile index disagrees with canonical CaseIds",
                    ));
                }
                indexed_candidates = checked_add(indexed_candidates, 1)?;
            }
        }
        if indexed_candidates != usize_count(self.candidates.len())? {
            return Err(BoundarySearchError::InternalInvariant(
                "candidate profile index does not cover every candidate exactly once",
            ));
        }

        let mut singleton_closed = 0_u128;
        let mut certificate_closed = 0_u128;
        for (outer_ordinals, plan) in &self.plans {
            self.validate_outer_path(outer_ordinals)?;
            plan.audit()?;
            singleton_closed = checked_add(singleton_closed, plan.singleton_closed)?;
            certificate_closed = checked_add(certificate_closed, plan.certificate_closed)?;
        }
        if singleton_closed != self.singleton_closed_cases
            || certificate_closed != self.certificate_closed_cases
        {
            return Err(BoundarySearchError::InternalInvariant(
                "global closure counters disagree with indexed profile plans",
            ));
        }

        let recomputed_open_candidates = self
            .candidate_order
            .iter()
            .filter_map(|case_id| match self.point_is_open(case_id) {
                Ok(true) => Some(Ok(case_id.clone())),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if recomputed_open_candidates != self.open_candidates {
            return Err(BoundarySearchError::InternalInvariant(
                "incremental open-candidate index disagrees with profile plans",
            ));
        }
        for (case_id, classification) in &self.evaluated_cases {
            let (outer, value) = self.split_case_id(case_id)?;
            let plan =
                self.plans
                    .get(outer.as_ref())
                    .ok_or(BoundarySearchError::InternalInvariant(
                        "evaluated CaseId has no materialized profile plan",
                    ))?;
            let existing = plan
                .cell_containing(value)
                .and_then(|cell| cell.state.classification())
                .ok_or(BoundarySearchError::InternalInvariant(
                    "evaluated CaseId remains open in its profile plan",
                ))?;
            if existing != classification {
                return Err(BoundarySearchError::InternalInvariant(
                    "evaluated classification disagrees with profile closure evidence",
                ));
            }
        }
        Ok(cost)
    }

    fn case_id_for_outer(
        &self,
        outer_ordinals: &[u128],
        boundary_value: i64,
    ) -> Result<ExploreCaseId, BoundarySearchError> {
        self.validate_outer_path(outer_ordinals)?;
        if !self.eligible_boundary.contains(boundary_value) {
            return Err(BoundarySearchError::CandidateOutsideEligible {
                value: boundary_value,
                eligible: self.eligible_boundary,
            });
        }
        let boundary_ordinal = boundary_offset(self.declared_boundary.start(), boundary_value)?;
        let mut outer = outer_ordinals.iter().copied();
        let mut full = Vec::with_capacity(self.axis_cardinalities.len());
        for dimension in 0..self.axis_cardinalities.len() {
            if dimension == self.boundary_dimension {
                full.push(boundary_ordinal);
            } else {
                full.push(outer.next().ok_or(BoundarySearchError::InternalInvariant(
                    "validated outer path ended while constructing a CaseId",
                ))?);
            }
        }
        if outer.next().is_some() {
            return Err(BoundarySearchError::InternalInvariant(
                "validated outer path retained ordinals after CaseId construction",
            ));
        }
        Ok(ExploreCaseId::new(full))
    }

    fn split_case_id(
        &self,
        case_id: &ExploreCaseId,
    ) -> Result<(Box<[u128]>, i64), BoundarySearchError> {
        self.validate_full_case_id(case_id)?;
        let boundary_ordinal = case_id.ordinals()[self.boundary_dimension];
        let value = i128::from(self.declared_boundary.start())
            .checked_add(i128::try_from(boundary_ordinal).map_err(|_| {
                BoundarySearchError::InternalInvariant(
                    "validated boundary ordinal did not fit an Int-domain offset",
                )
            })?)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(BoundarySearchError::InternalInvariant(
                "validated boundary ordinal did not map into its Int domain",
            ))?;
        if !self.eligible_boundary.contains(value) {
            return Err(BoundarySearchError::CaseOutsideEligible {
                case_id: case_id.clone(),
            });
        }
        let outer = case_id
            .ordinals()
            .iter()
            .enumerate()
            .filter_map(|(dimension, ordinal)| {
                (dimension != self.boundary_dimension).then_some(*ordinal)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok((outer, value))
    }

    fn validate_outer_path(&self, outer_ordinals: &[u128]) -> Result<(), BoundarySearchError> {
        let expected = self.axis_cardinalities.len() - 1;
        if outer_ordinals.len() != expected {
            return Err(BoundarySearchError::OuterPathArity {
                expected,
                actual: outer_ordinals.len(),
            });
        }
        let mut outer_index = 0;
        for (dimension, &cardinality) in self.axis_cardinalities.iter().enumerate() {
            if dimension == self.boundary_dimension {
                continue;
            }
            let ordinal = outer_ordinals[outer_index];
            if ordinal >= cardinality {
                return Err(BoundarySearchError::OuterOrdinalOutOfBounds {
                    outer_dimension: outer_index,
                    source_dimension: dimension,
                    ordinal,
                    cardinality,
                });
            }
            outer_index += 1;
        }
        Ok(())
    }

    fn validate_full_case_id(&self, case_id: &ExploreCaseId) -> Result<(), BoundarySearchError> {
        if case_id.len() != self.axis_cardinalities.len() {
            return Err(BoundarySearchError::CasePathArity {
                expected: self.axis_cardinalities.len(),
                actual: case_id.len(),
            });
        }
        for (dimension, (&ordinal, &cardinality)) in case_id
            .ordinals()
            .iter()
            .zip(self.axis_cardinalities.iter())
            .enumerate()
        {
            if ordinal >= cardinality {
                return Err(BoundarySearchError::CaseOrdinalOutOfBounds {
                    dimension,
                    ordinal,
                    cardinality,
                });
            }
        }
        Ok(())
    }

    fn point_is_open(&self, case_id: &ExploreCaseId) -> Result<bool, BoundarySearchError> {
        let (outer_ordinals, boundary_value) = self.split_case_id(case_id)?;
        let Some(plan) = self.plans.get(outer_ordinals.as_ref()) else {
            return Ok(true);
        };
        let cell =
            plan.cell_containing(boundary_value)
                .ok_or(BoundarySearchError::InternalInvariant(
                    "eligible point was absent from an indexed profile plan",
                ))?;
        Ok(cell.is_open())
    }

    fn closed_cell_end(&self, case_id: &ExploreCaseId) -> Result<Option<i64>, BoundarySearchError> {
        let (outer_ordinals, boundary_value) = self.split_case_id(case_id)?;
        let Some(plan) = self.plans.get(outer_ordinals.as_ref()) else {
            return Ok(None);
        };
        let cell =
            plan.cell_containing(boundary_value)
                .ok_or(BoundarySearchError::InternalInvariant(
                    "eligible point was absent from an indexed profile plan",
                ))?;
        Ok((!cell.is_open()).then_some(cell.interval.end_exclusive()))
    }
}

/// Lazy lexicographic cursor over full canonical-axis CaseIds with one axis
/// restricted to a contiguous eligible ordinal interval.
#[derive(Debug, Clone)]
struct CanonicalEligibleCursor {
    boundary_dimension: usize,
    minima: Box<[u128]>,
    maxima_exclusive: Box<[u128]>,
    current: Box<[u128]>,
    first: bool,
    exhausted: bool,
}

impl CanonicalEligibleCursor {
    fn new(
        axis_cardinalities: Box<[u128]>,
        boundary_dimension: usize,
        boundary_start: u128,
        boundary_end_exclusive: u128,
    ) -> Self {
        let mut minima = vec![0_u128; axis_cardinalities.len()];
        let mut maxima_exclusive = axis_cardinalities.to_vec();
        minima[boundary_dimension] = boundary_start;
        maxima_exclusive[boundary_dimension] = boundary_end_exclusive;
        let exhausted = minima
            .iter()
            .zip(&maxima_exclusive)
            .any(|(minimum, maximum)| minimum >= maximum);
        Self {
            boundary_dimension,
            current: minima.clone().into_boxed_slice(),
            minima: minima.into_boxed_slice(),
            maxima_exclusive: maxima_exclusive.into_boxed_slice(),
            first: true,
            exhausted,
        }
    }

    /// Fast-forward a closed interval only when no trailing canonical axis
    /// can interleave another profile inside it.
    fn skip_current_boundary_interval(
        &mut self,
        end_exclusive: u128,
    ) -> Result<(), BoundarySearchError> {
        if self.boundary_dimension + 1 != self.current.len() {
            return Err(BoundarySearchError::InternalInvariant(
                "attempted an interval cursor jump with trailing axes",
            ));
        }
        let current = self.current[self.boundary_dimension];
        let maximum = self.maxima_exclusive[self.boundary_dimension];
        if end_exclusive <= current || end_exclusive > maximum {
            return Err(BoundarySearchError::InternalInvariant(
                "closed interval produced an invalid cursor jump",
            ));
        }
        self.current[self.boundary_dimension] = end_exclusive - 1;
        Ok(())
    }
}

impl Iterator for CanonicalEligibleCursor {
    type Item = ExploreCaseId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }
        if self.first {
            self.first = false;
            return Some(ExploreCaseId::new(self.current.clone()));
        }
        for dimension in (0..self.current.len()).rev() {
            let next = self.current[dimension]
                .checked_add(1)
                .expect("a cursor ordinal below its exclusive maximum cannot overflow");
            if next < self.maxima_exclusive[dimension] {
                self.current[dimension] = next;
                for trailing in dimension + 1..self.current.len() {
                    self.current[trailing] = self.minima[trailing];
                }
                return Some(ExploreCaseId::new(self.current.clone()));
            }
        }
        self.exhausted = true;
        None
    }
}

fn terminal_is_closed(terminal: &CaseTerminal) -> bool {
    matches!(
        terminal,
        CaseTerminal::Excluded | CaseTerminal::AdmissibleNonmatch | CaseTerminal::AdmissibleMatch
    )
}

fn intervals_overlap(left: BoundaryInterval, right: BoundaryInterval) -> bool {
    left.start() < right.end_exclusive() && right.start() < left.end_exclusive()
}

fn interval_intersection(
    left: BoundaryInterval,
    right: BoundaryInterval,
) -> Result<BoundaryInterval, BoundarySearchError> {
    let start = left.start().max(right.start());
    let end_exclusive = left.end_exclusive().min(right.end_exclusive());
    if start >= end_exclusive {
        return Err(BoundarySearchError::InternalInvariant(
            "requested the intersection of disjoint boundary intervals",
        ));
    }
    BoundaryInterval::new(start, end_exclusive).map_err(BoundarySearchError::BoundaryPlan)
}

fn boundary_offset(start: i64, value: i64) -> Result<u128, BoundarySearchError> {
    u128::try_from(i128::from(value) - i128::from(start)).map_err(|_| {
        BoundarySearchError::InternalInvariant(
            "a boundary value below its declared start has no ordinal",
        )
    })
}

fn checked_product(cardinalities: &[u128]) -> Result<u128, BoundarySearchError> {
    if cardinalities.contains(&0) {
        return Ok(0);
    }
    cardinalities
        .iter()
        .copied()
        .try_fold(1_u128, |total, cardinality| {
            total
                .checked_mul(cardinality)
                .ok_or(BoundarySearchError::CardinalityOverflow)
        })
}

fn checked_add(left: u128, right: u128) -> Result<u128, BoundarySearchError> {
    left.checked_add(right)
        .ok_or(BoundarySearchError::CardinalityOverflow)
}

fn usize_count(value: usize) -> Result<u128, BoundarySearchError> {
    u128::try_from(value).map_err(|_| BoundarySearchError::CardinalityOverflow)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundarySearchError {
    BoundaryDimensionOutOfBounds {
        dimension: usize,
        axis_count: usize,
    },
    BoundaryCardinalityMismatch {
        axis: u128,
        declared: u128,
    },
    EligibleBoundaryOutsideDeclared {
        declared: BoundaryInterval,
        eligible: BoundaryInterval,
    },
    CandidateOutsideEligible {
        value: i64,
        eligible: BoundaryInterval,
    },
    OuterPathArity {
        expected: usize,
        actual: usize,
    },
    OuterOrdinalOutOfBounds {
        outer_dimension: usize,
        source_dimension: usize,
        ordinal: u128,
        cardinality: u128,
    },
    CasePathArity {
        expected: usize,
        actual: usize,
    },
    CaseOrdinalOutOfBounds {
        dimension: usize,
        ordinal: u128,
        cardinality: u128,
    },
    CaseOutsideEligible {
        case_id: ExploreCaseId,
    },
    EmptyCertificateInterval,
    CertificateOutsideEligible {
        eligible: BoundaryInterval,
        interval: BoundaryInterval,
    },
    OpenClassificationCannotClose,
    EvaluationWasNotScheduled {
        case_id: ExploreCaseId,
    },
    ConflictingClosedClassification {
        interval: BoundaryInterval,
        existing: CaseTerminal,
        proposed: CaseTerminal,
    },
    CardinalityOverflow,
    CardinalityConservation {
        eligible: u128,
        singleton_closed: u128,
        certificate_closed: u128,
        open: u128,
    },
    BoundaryPlan(BoundaryPlanError),
    CaseGraph(CaseGraphError),
    CertifiedRegionLowering(String),
    InternalInvariant(&'static str),
}

impl From<BoundaryPlanError> for BoundarySearchError {
    fn from(error: BoundaryPlanError) -> Self {
        Self::BoundaryPlan(error)
    }
}

impl fmt::Display for BoundarySearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundaryDimensionOutOfBounds {
                dimension,
                axis_count,
            } => write!(
                formatter,
                "boundary dimension {dimension} is outside {axis_count} canonical axes"
            ),
            Self::BoundaryCardinalityMismatch { axis, declared } => write!(
                formatter,
                "boundary axis cardinality {axis} does not equal declared Int interval cardinality {declared}"
            ),
            Self::EligibleBoundaryOutsideDeclared { declared, eligible } => write!(
                formatter,
                "eligible boundary [{}, {}) lies outside declared boundary [{}, {})",
                eligible.start(),
                eligible.end_exclusive(),
                declared.start(),
                declared.end_exclusive()
            ),
            Self::CandidateOutsideEligible { value, eligible } => write!(
                formatter,
                "candidate value {value} lies outside eligible interval [{}, {})",
                eligible.start(),
                eligible.end_exclusive()
            ),
            Self::OuterPathArity { expected, actual } => write!(
                formatter,
                "outer profile has {actual} ordinals, expected {expected} non-boundary axes"
            ),
            Self::OuterOrdinalOutOfBounds {
                outer_dimension,
                source_dimension,
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "outer ordinal {ordinal} at outer dimension {outer_dimension} is outside source dimension {source_dimension} cardinality {cardinality}"
            ),
            Self::CasePathArity { expected, actual } => write!(
                formatter,
                "boundary-search CaseId has {actual} ordinals, expected {expected}"
            ),
            Self::CaseOrdinalOutOfBounds {
                dimension,
                ordinal,
                cardinality,
            } => write!(
                formatter,
                "CaseId ordinal {ordinal} is outside dimension {dimension} cardinality {cardinality}"
            ),
            Self::CaseOutsideEligible { case_id } => write!(
                formatter,
                "CaseId {:?} is outside the eligible boundary slice",
                case_id.ordinals()
            ),
            Self::EmptyCertificateInterval => {
                write!(formatter, "a boundary region certificate cannot be empty")
            }
            Self::CertificateOutsideEligible { eligible, interval } => write!(
                formatter,
                "certified interval [{}, {}) lies outside eligible boundary [{}, {})",
                interval.start(),
                interval.end_exclusive(),
                eligible.start(),
                eligible.end_exclusive()
            ),
            Self::OpenClassificationCannotClose => write!(
                formatter,
                "an open classification cannot close a singleton or certified region"
            ),
            Self::EvaluationWasNotScheduled { case_id } => write!(
                formatter,
                "CaseId {:?} was evaluated without a pending scheduler request",
                case_id.ordinals()
            ),
            Self::ConflictingClosedClassification {
                interval,
                existing,
                proposed,
            } => write!(
                formatter,
                "boundary interval [{}, {}) is already closed as {:?}, not {:?}",
                interval.start(),
                interval.end_exclusive(),
                existing,
                proposed
            ),
            Self::CardinalityOverflow => {
                write!(formatter, "boundary-search cardinality exceeds u128::MAX")
            }
            Self::CardinalityConservation {
                eligible,
                singleton_closed,
                certificate_closed,
                open,
            } => write!(
                formatter,
                "boundary-search support does not conserve eligible cardinality {eligible}: singleton={singleton_closed}, certified={certificate_closed}, open={open}"
            ),
            Self::BoundaryPlan(error) => write!(formatter, "{error}"),
            Self::CaseGraph(error) => write!(formatter, "{error}"),
            Self::CertifiedRegionLowering(error) => write!(
                formatter,
                "cannot lower boundary-search proof cells into the case graph: {error}"
            ),
            Self::InternalInvariant(message) => {
                write!(formatter, "boundary-search invariant failed: {message}")
            }
        }
    }
}

impl Error for BoundarySearchError {}

#[cfg(test)]
mod tests {
    use super::super::case_graph::CheckedCardinality;
    use super::*;

    type TestSearch = CandidateFirstBoundarySearch<&'static str, &'static str>;

    fn interval(start: i64, end_exclusive: i64) -> BoundaryInterval {
        BoundaryInterval::new(start, end_exclusive).unwrap()
    }

    fn candidate(
        outer: &[u128],
        boundary_value: i64,
        hint: &'static str,
    ) -> BoundarySearchCandidate<&'static str> {
        BoundarySearchCandidate::new(outer.to_vec(), boundary_value, hint)
    }

    #[test]
    fn candidate_cursor_and_hint_dedup_are_discovery_order_independent() {
        let mut search = TestSearch::new(
            vec![2, 4, 2],
            1,
            interval(100, 104),
            interval(100, 103),
            [
                candidate(&[1, 0], 101, "last"),
                candidate(&[0, 1], 102, "beta"),
                candidate(&[0, 0], 100, "first"),
                candidate(&[0, 1], 102, "alpha"),
                candidate(&[0, 1], 102, "alpha"),
            ],
        )
        .unwrap();

        let first = search.next_work().unwrap().into_work().unwrap();
        let second = search.next_work().unwrap().into_work().unwrap();
        let third = search.next_work().unwrap().into_work().unwrap();
        assert_eq!(first.case_id().ordinals(), &[0, 0, 0]);
        assert_eq!(second.case_id().ordinals(), &[0, 2, 1]);
        assert_eq!(second.candidate_hints(), Some(["alpha", "beta"].as_slice()));
        assert_eq!(third.case_id().ordinals(), &[1, 1, 0]);
        assert_eq!(
            search.next_work().unwrap(),
            BoundarySearchStep::WaitingForCandidateEvaluations { pending: 3 }
        );

        for work in [third, first, second] {
            search
                .record_evaluation(work.case_id().clone(), CaseTerminal::AdmissibleNonmatch)
                .unwrap();
        }
        assert_eq!(
            search
                .evaluated_cases()
                .keys()
                .map(|case_id| case_id.ordinals().to_vec())
                .collect::<Vec<_>>(),
            vec![vec![0, 0, 0], vec![0, 2, 1], vec![1, 1, 0]]
        );

        let fallback = search.next_work().unwrap().into_work().unwrap();
        assert!(!fallback.is_candidate());
        assert_eq!(fallback.case_id().ordinals(), &[0, 0, 1]);
        let cost = search.cost_ledger().unwrap();
        assert_eq!(cost.distinct_candidate_cases(), 3);
        assert_eq!(cost.scheduled_candidates(), 3);
        assert_eq!(cost.evaluated_candidates(), 3);
        assert_eq!(cost.scheduled_fallback(), 1);
        assert_eq!(cost.evaluated_fallback(), 0);
        search.audit().unwrap();
    }

    #[test]
    fn point_dag_is_conservative_while_indexed_certificates_close_cost_support() {
        let mut search = TestSearch::new(
            vec![2, 5],
            1,
            interval(0, 5),
            interval(0, 4),
            [candidate(&[1], 2, "event")],
        )
        .unwrap();
        let work = search.next_work().unwrap().into_work().unwrap();
        search
            .record_evaluation(work.case_id().clone(), CaseTerminal::AdmissibleMatch)
            .unwrap();
        search
            .certify_region(
                &[0],
                interval(0, 2),
                CaseTerminal::AdmissibleNonmatch,
                "interval-proof",
            )
            .unwrap();

        let graph = search.point_case_graph().unwrap();
        assert_eq!(
            graph.terminal_counts().unwrap(),
            BTreeMap::from([
                (CaseTerminal::AdmissibleMatch, CheckedCardinality::Exact(1)),
                (
                    CaseTerminal::EligibilityOpen(CaseOpenReason::EvaluationUnknown),
                    CheckedCardinality::Exact(9),
                ),
            ])
        );
        let cost = search.cost_ledger().unwrap();
        assert_eq!(cost.declared_cases(), 10);
        assert_eq!(cost.eligible_cases(), 8);
        assert_eq!(cost.structurally_outside_eligible_cases(), 2);
        assert_eq!(cost.singleton_closed_cases(), 1);
        assert_eq!(cost.certificate_closed_cases(), 2);
        assert_eq!(cost.remaining_open_cases(), 5);
        assert_eq!(cost.fallback_work(), 5);
        search.audit().unwrap();
    }

    #[test]
    fn scheduling_never_closes_more_than_one_point_without_a_certificate() {
        let mut search = TestSearch::new(
            vec![1, 10],
            1,
            interval(0, 10),
            interval(0, 10),
            [candidate(&[0], 5, "same-branch-nearby")],
        )
        .unwrap();
        let scheduled = search.next_work().unwrap().into_work().unwrap();
        assert_eq!(search.cost_ledger().unwrap().remaining_open_cases(), 10);
        search
            .record_evaluation(
                scheduled.case_id().clone(),
                CaseTerminal::AdmissibleNonmatch,
            )
            .unwrap();
        assert_eq!(search.cost_ledger().unwrap().singleton_closed_cases(), 1);

        search
            .certify_region(
                &[0],
                interval(0, 5),
                CaseTerminal::AdmissibleNonmatch,
                "exact-proof",
            )
            .unwrap();
        let cost = search.cost_ledger().unwrap();
        assert_eq!(cost.singleton_closed_cases(), 1);
        assert_eq!(cost.certificate_closed_cases(), 5);
        assert_eq!(cost.remaining_open_cases(), 4);
        search.audit().unwrap();
    }

    #[test]
    fn exported_v1_plan_carries_no_region_signature() {
        let mut search = TestSearch::new(
            vec![1, 8],
            1,
            interval(0, 8),
            interval(0, 8),
            std::iter::empty(),
        )
        .unwrap();
        search
            .certify_region(
                &[0],
                interval(2, 7),
                CaseTerminal::AdmissibleMatch,
                "classification-proof-only",
            )
            .unwrap();

        let snapshot = search.profile_plan_snapshot(&[0]).unwrap();
        for cell in snapshot.cells() {
            if let Some((_, signature, _)) = cell.closed_evidence() {
                assert_eq!(signature, &None::<Infallible>);
            }
        }
        snapshot.validate().unwrap();
    }

    #[test]
    fn fallback_is_canonical_with_boundary_at_any_source_position() {
        let mut search = TestSearch::new(
            vec![2, 3, 2],
            1,
            interval(10, 13),
            interval(10, 13),
            [candidate(&[1, 0], 11, "late-canonical-candidate")],
        )
        .unwrap();
        let candidate = search.next_work().unwrap().into_work().unwrap();
        assert_eq!(candidate.case_id().ordinals(), &[1, 1, 0]);
        assert_eq!(
            search.next_work().unwrap(),
            BoundarySearchStep::WaitingForCandidateEvaluations { pending: 1 }
        );
        search
            .record_evaluation(
                candidate.case_id().clone(),
                CaseTerminal::AdmissibleNonmatch,
            )
            .unwrap();

        let first = search.next_work().unwrap().into_work().unwrap();
        let second = search.next_work().unwrap().into_work().unwrap();
        assert_eq!(first.case_id().ordinals(), &[0, 0, 0]);
        assert_eq!(second.case_id().ordinals(), &[0, 0, 1]);
    }

    #[test]
    fn rightmost_fallback_jumps_over_three_million_certified_points() {
        let mut search = TestSearch::new(
            vec![1, 3_000_000],
            1,
            interval(0, 3_000_000),
            interval(0, 3_000_000),
            std::iter::empty(),
        )
        .unwrap();
        search
            .certify_region(
                &[0],
                interval(0, 2_999_999),
                CaseTerminal::AdmissibleNonmatch,
                "interval-proof",
            )
            .unwrap();
        let work = search.next_work().unwrap().into_work().unwrap();
        assert_eq!(work.case_id().ordinals(), &[0, 2_999_999]);
        let cost = search.cost_ledger().unwrap();
        assert_eq!(cost.certificate_closed_cases(), 2_999_999);
        assert_eq!(cost.fallback_work(), 1);
    }

    #[test]
    fn conflicting_certification_is_atomic() {
        let mut search = TestSearch::new(
            vec![1, 6],
            1,
            interval(0, 6),
            interval(0, 6),
            std::iter::empty(),
        )
        .unwrap();
        search
            .certify_region(
                &[0],
                interval(1, 5),
                CaseTerminal::AdmissibleMatch,
                "first-proof",
            )
            .unwrap();
        let before = search.cost_ledger().unwrap();
        let error = search
            .certify_region(
                &[0],
                interval(3, 6),
                CaseTerminal::AdmissibleNonmatch,
                "conflicting-proof",
            )
            .unwrap_err();
        assert!(matches!(
            error,
            BoundarySearchError::ConflictingClosedClassification { .. }
        ));
        assert_eq!(search.cost_ledger().unwrap(), before);
        search.audit().unwrap();
    }

    #[test]
    fn conflicting_pending_evaluation_is_atomic_and_remains_pending() {
        let mut search = TestSearch::new(
            vec![1, 6],
            1,
            interval(0, 6),
            interval(0, 6),
            [candidate(&[0], 3, "candidate")],
        )
        .unwrap();
        let work = search.next_work().unwrap().into_work().unwrap();
        search
            .certify_region(
                &[0],
                interval(1, 5),
                CaseTerminal::AdmissibleMatch,
                "region-proof",
            )
            .unwrap();
        let before_cost = search.cost_ledger().unwrap();
        let before_plan = search.profile_plan_snapshot(&[0]).unwrap();

        let error = search
            .record_evaluation(work.case_id().clone(), CaseTerminal::AdmissibleNonmatch)
            .unwrap_err();
        assert!(matches!(
            error,
            BoundarySearchError::ConflictingClosedClassification { .. }
        ));
        assert_eq!(search.cost_ledger().unwrap(), before_cost);
        assert_eq!(search.profile_plan_snapshot(&[0]).unwrap(), before_plan);
        assert_eq!(
            search.next_work().unwrap(),
            BoundarySearchStep::WaitingForCandidateEvaluations { pending: 1 }
        );
        search.audit().unwrap();
    }

    #[test]
    fn empty_outer_product_and_zero_outer_axes_are_exact() {
        let mut empty = TestSearch::new(
            vec![0, 5],
            1,
            interval(0, 5),
            interval(0, 4),
            std::iter::empty(),
        )
        .unwrap();
        let cost = empty.cost_ledger().unwrap();
        assert_eq!(cost.declared_cases(), 0);
        assert_eq!(cost.eligible_cases(), 0);
        assert_eq!(cost.remaining_open_cases(), 0);
        assert_eq!(empty.next_work().unwrap(), BoundarySearchStep::Exhausted);
        assert_eq!(
            empty.point_case_graph().unwrap().universe_cardinality(),
            CheckedCardinality::Exact(0)
        );

        let mut no_outer_axes = TestSearch::new(
            vec![3],
            0,
            interval(20, 23),
            interval(20, 23),
            std::iter::empty(),
        )
        .unwrap();
        assert_eq!(
            no_outer_axes
                .next_work()
                .unwrap()
                .into_work()
                .unwrap()
                .case_id()
                .ordinals(),
            &[0]
        );
    }
}
