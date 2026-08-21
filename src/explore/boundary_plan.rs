//! Exact refinement bookkeeping for one bounded integer boundary axis.
//!
//! A boundary plan is a partition of a declared half-open `Int` domain into
//! ordered, disjoint cells. Probe observations may guide scheduling, but only
//! a certificate-bearing exact cover can close any part of the domain. This
//! keeps exhaustive, SMT, and CEGAR backends on the same monotone refinement
//! contract.

use std::fmt;

/// A half-open integer interval `[start, end_exclusive)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct BoundaryInterval {
    start: i64,
    end_exclusive: i64,
}

impl BoundaryInterval {
    pub(super) fn new(start: i64, end_exclusive: i64) -> Result<Self, BoundaryPlanError> {
        if start > end_exclusive {
            return Err(BoundaryPlanError::InvalidInterval {
                start,
                end_exclusive,
            });
        }
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    pub(super) fn start(self) -> i64 {
        self.start
    }

    pub(super) fn end_exclusive(self) -> i64 {
        self.end_exclusive
    }

    pub(super) fn is_empty(self) -> bool {
        self.start == self.end_exclusive
    }

    pub(super) fn cardinality(self) -> u128 {
        let width = i128::from(self.end_exclusive) - i128::from(self.start);
        u128::try_from(width).expect("a validated boundary interval has nonnegative width")
    }

    pub(super) fn contains(self, value: i64) -> bool {
        value >= self.start && value < self.end_exclusive
    }

    pub(super) fn is_within(self, outer: Self) -> bool {
        self.start >= outer.start && self.end_exclusive <= outer.end_exclusive
    }

    /// Choose the ordinary half-open binary-search midpoint. For an even
    /// cardinality this is the upper of the two central integer values.
    pub(super) fn canonical_midpoint(self) -> Option<i64> {
        if self.is_empty() {
            return None;
        }
        let offset = self.cardinality() / 2;
        let midpoint = i128::from(self.start)
            + i128::try_from(offset).expect("an Int interval width always fits i128");
        Some(i64::try_from(midpoint).expect("an in-domain midpoint always fits Int"))
    }
}

/// Closure state for one exact interval cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundaryCellState<Classification, Signature, Certificate> {
    Open,
    Closed {
        classification: Classification,
        signature: Signature,
        certificate: Certificate,
    },
}

/// One cell in a boundary plan's exact partition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundaryCell<Classification, Signature, Certificate> {
    interval: BoundaryInterval,
    state: BoundaryCellState<Classification, Signature, Certificate>,
}

impl<Classification, Signature, Certificate> BoundaryCell<Classification, Signature, Certificate> {
    pub(super) fn open(interval: BoundaryInterval) -> Self {
        Self {
            interval,
            state: BoundaryCellState::Open,
        }
    }

    pub(super) fn closed(
        interval: BoundaryInterval,
        classification: Classification,
        signature: Signature,
        certificate: Certificate,
    ) -> Self {
        Self {
            interval,
            state: BoundaryCellState::Closed {
                classification,
                signature,
                certificate,
            },
        }
    }

    pub(super) fn interval(&self) -> BoundaryInterval {
        self.interval
    }

    pub(super) fn state(&self) -> &BoundaryCellState<Classification, Signature, Certificate> {
        &self.state
    }

    pub(super) fn is_open(&self) -> bool {
        matches!(&self.state, BoundaryCellState::Open)
    }

    pub(super) fn closed_evidence(&self) -> Option<(&Classification, &Signature, &Certificate)> {
        match &self.state {
            BoundaryCellState::Open => None,
            BoundaryCellState::Closed {
                classification,
                signature,
                certificate,
            } => Some((classification, signature, certificate)),
        }
    }
}

/// A deterministic midpoint requested from one currently open cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoundaryProbeRequest {
    cell: BoundaryInterval,
    point: i64,
}

impl BoundaryProbeRequest {
    pub(super) fn cell(self) -> BoundaryInterval {
        self.cell
    }

    pub(super) fn point(self) -> i64 {
        self.point
    }
}

/// Scheduling evidence recorded for a probe. It is deliberately separate
/// from [`BoundaryCellState::Closed`] and cannot alter plan closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundaryProbe<ProbeEvidence> {
    request: BoundaryProbeRequest,
    evidence: ProbeEvidence,
}

impl<ProbeEvidence> BoundaryProbe<ProbeEvidence> {
    pub(super) fn request(&self) -> BoundaryProbeRequest {
        self.request
    }

    pub(super) fn evidence(&self) -> &ProbeEvidence {
        &self.evidence
    }
}

/// Cardinality accounting for a validated plan partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoundaryPlanSummary {
    declared: u128,
    open: u128,
    closed: u128,
}

impl BoundaryPlanSummary {
    pub(super) fn declared(self) -> u128 {
        self.declared
    }

    pub(super) fn open(self) -> u128 {
        self.open
    }

    pub(super) fn closed(self) -> u128 {
        self.closed
    }
}

/// A monotone exact partition of one declared finite integer axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoundaryPlan<Classification, Signature, Certificate, ProbeEvidence> {
    declared: BoundaryInterval,
    cells: Vec<BoundaryCell<Classification, Signature, Certificate>>,
    probes: Vec<BoundaryProbe<ProbeEvidence>>,
}

impl<Classification, Signature, Certificate, ProbeEvidence>
    BoundaryPlan<Classification, Signature, Certificate, ProbeEvidence>
{
    pub(super) fn new(start: i64, end_exclusive: i64) -> Result<Self, BoundaryPlanError> {
        let declared = BoundaryInterval::new(start, end_exclusive)?;
        let cells = if declared.is_empty() {
            Vec::new()
        } else {
            vec![BoundaryCell::open(declared)]
        };
        let plan = Self {
            declared,
            cells,
            probes: Vec::new(),
        };
        plan.validate()?;
        Ok(plan)
    }

    pub(super) fn declared(&self) -> BoundaryInterval {
        self.declared
    }

    pub(super) fn cells(&self) -> &[BoundaryCell<Classification, Signature, Certificate>] {
        &self.cells
    }

    pub(super) fn probes(&self) -> &[BoundaryProbe<ProbeEvidence>] {
        &self.probes
    }

    pub(super) fn cell_containing(
        &self,
        point: i64,
    ) -> Option<&BoundaryCell<Classification, Signature, Certificate>> {
        self.cells.iter().find(|cell| cell.interval.contains(point))
    }

    /// Return the exact open frontier in canonical axis order.
    pub(super) fn open_frontier(&self) -> Vec<BoundaryInterval> {
        self.cells
            .iter()
            .filter(|cell| cell.is_open())
            .map(|cell| cell.interval)
            .collect()
    }

    /// Choose the midpoint of the largest open cell. Ordered cells break ties
    /// toward the lower interval, making scheduling independent of discovery
    /// order. A previously observed point remains semantically open until it
    /// is certified, but it is never scheduled twice.
    pub(super) fn next_midpoint_probe(&self) -> Option<BoundaryProbeRequest> {
        let mut selected: Option<&BoundaryCell<Classification, Signature, Certificate>> = None;
        for cell in self.cells.iter().filter(|cell| cell.is_open()) {
            let midpoint = cell
                .interval
                .canonical_midpoint()
                .expect("validated plan cells are nonempty");
            if self
                .probes
                .iter()
                .any(|probe| probe.request.point == midpoint)
            {
                continue;
            }
            let replace = selected
                .map(|current| cell.interval.cardinality() > current.interval.cardinality())
                .unwrap_or(true);
            if replace {
                selected = Some(cell);
            }
        }
        selected.map(|cell| BoundaryProbeRequest {
            cell: cell.interval,
            point: cell
                .interval
                .canonical_midpoint()
                .expect("validated plan cells are nonempty"),
        })
    }

    pub(super) fn midpoint_probe_for(
        &self,
        cell: BoundaryInterval,
    ) -> Result<BoundaryProbeRequest, BoundaryPlanError> {
        let current = self
            .cells
            .iter()
            .find(|candidate| candidate.interval == cell)
            .ok_or(BoundaryPlanError::TargetIsNotCell { target: cell })?;
        if !current.is_open() {
            return Err(BoundaryPlanError::CellAlreadyClosed { cell });
        }
        let point = cell
            .canonical_midpoint()
            .expect("validated plan cells are nonempty");
        if self.probes.iter().any(|probe| probe.request.point == point) {
            return Err(BoundaryPlanError::ProbeAlreadyRecorded { point });
        }
        Ok(BoundaryProbeRequest { cell, point })
    }

    /// Record an observation for a currently open cell's canonical midpoint.
    /// The observation partitions scheduling support around that point, but
    /// every replacement cell remains open. Therefore it preserves unresolved
    /// cardinality exactly and cannot masquerade as a region certificate.
    pub(super) fn record_probe(
        &mut self,
        request: BoundaryProbeRequest,
        evidence: ProbeEvidence,
    ) -> Result<(), BoundaryPlanError> {
        let canonical = self.midpoint_probe_for(request.cell)?;
        if canonical != request {
            return Err(BoundaryPlanError::NonCanonicalProbe {
                cell: request.cell,
                expected: canonical.point,
                actual: request.point,
            });
        }
        let point_end = request
            .point
            .checked_add(1)
            .expect("a point below an exclusive Int endpoint has a successor");
        let mut replacement = Vec::with_capacity(3);
        if request.cell.start < request.point {
            replacement.push(BoundaryCell::open(BoundaryInterval {
                start: request.cell.start,
                end_exclusive: request.point,
            }));
        }
        replacement.push(BoundaryCell::open(BoundaryInterval {
            start: request.point,
            end_exclusive: point_end,
        }));
        if point_end < request.cell.end_exclusive {
            replacement.push(BoundaryCell::open(BoundaryInterval {
                start: point_end,
                end_exclusive: request.cell.end_exclusive,
            }));
        }
        self.refine_open(request.cell, replacement)?;
        self.probes.push(BoundaryProbe { request, evidence });
        Ok(())
    }

    pub(super) fn record_next_midpoint_probe(
        &mut self,
        evidence: ProbeEvidence,
    ) -> Result<BoundaryProbeRequest, BoundaryPlanError> {
        let request = self.next_midpoint_probe().ok_or_else(|| {
            if self.open_frontier().is_empty() {
                BoundaryPlanError::OpenFrontierEmpty
            } else {
                BoundaryPlanError::OpenFrontierFullyProbed
            }
        })?;
        self.record_probe(request, evidence)?;
        Ok(request)
    }

    /// Replace exactly one open cell with an ordered, disjoint exact cover.
    /// Existing closed cells cannot be targeted, merged, reopened, or changed.
    pub(super) fn refine_open(
        &mut self,
        target: BoundaryInterval,
        replacement: impl IntoIterator<Item = BoundaryCell<Classification, Signature, Certificate>>,
    ) -> Result<BoundaryPlanSummary, BoundaryPlanError> {
        self.validate()?;
        let index = self
            .cells
            .iter()
            .position(|cell| cell.interval == target)
            .ok_or(BoundaryPlanError::TargetIsNotCell { target })?;
        if !self.cells[index].is_open() {
            return Err(BoundaryPlanError::CellAlreadyClosed { cell: target });
        }

        let replacement = replacement.into_iter().collect::<Vec<_>>();
        validate_exact_cover(target, &replacement)?;
        self.cells.splice(index..=index, replacement);
        self.validate()
    }

    /// Validate the exact cover and prove `open + closed == declared`.
    pub(super) fn validate(&self) -> Result<BoundaryPlanSummary, BoundaryPlanError> {
        if self.declared.is_empty() {
            if !self.cells.is_empty() {
                return Err(BoundaryPlanError::EmptyDomainHasCells {
                    cell_count: self.cells.len(),
                });
            }
        } else {
            validate_exact_cover(self.declared, &self.cells)?;
        }

        let mut open = 0_u128;
        let mut closed = 0_u128;
        for cell in &self.cells {
            if cell.is_open() {
                open = open
                    .checked_add(cell.interval.cardinality())
                    .ok_or(BoundaryPlanError::CardinalityOverflow)?;
            } else {
                closed = closed
                    .checked_add(cell.interval.cardinality())
                    .ok_or(BoundaryPlanError::CardinalityOverflow)?;
            }
        }
        let conserved = open
            .checked_add(closed)
            .ok_or(BoundaryPlanError::CardinalityOverflow)?;
        let declared = self.declared.cardinality();
        if conserved != declared {
            return Err(BoundaryPlanError::CardinalityMismatch {
                declared,
                covered: conserved,
            });
        }

        for probe in &self.probes {
            let cell = probe.request.cell;
            if cell.is_empty()
                || !cell.is_within(self.declared)
                || cell.canonical_midpoint() != Some(probe.request.point)
            {
                return Err(BoundaryPlanError::InvalidRecordedProbe {
                    cell,
                    point: probe.request.point,
                });
            }
        }

        Ok(BoundaryPlanSummary {
            declared,
            open,
            closed,
        })
    }
}

fn validate_exact_cover<Classification, Signature, Certificate>(
    target: BoundaryInterval,
    cells: &[BoundaryCell<Classification, Signature, Certificate>],
) -> Result<(), BoundaryPlanError> {
    if cells.is_empty() {
        return Err(BoundaryPlanError::EmptyReplacement { target });
    }

    let mut expected_start = target.start;
    let mut covered = 0_u128;
    for cell in cells {
        if cell.interval.is_empty() {
            return Err(BoundaryPlanError::EmptyCell {
                interval: cell.interval,
            });
        }
        if !cell.interval.is_within(target) {
            return Err(BoundaryPlanError::CellOutsideTarget {
                target,
                cell: cell.interval,
            });
        }
        if cell.interval.start != expected_start {
            return Err(BoundaryPlanError::CoverGapOrOverlap {
                target,
                expected_start,
                actual_start: cell.interval.start,
            });
        }
        covered = covered
            .checked_add(cell.interval.cardinality())
            .ok_or(BoundaryPlanError::CardinalityOverflow)?;
        expected_start = cell.interval.end_exclusive;
    }

    if expected_start != target.end_exclusive {
        return Err(BoundaryPlanError::CoverWrongEnd {
            target,
            actual_end: expected_start,
        });
    }
    if covered != target.cardinality() {
        return Err(BoundaryPlanError::CardinalityMismatch {
            declared: target.cardinality(),
            covered,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BoundaryPlanError {
    InvalidInterval {
        start: i64,
        end_exclusive: i64,
    },
    EmptyDomainHasCells {
        cell_count: usize,
    },
    EmptyCell {
        interval: BoundaryInterval,
    },
    TargetIsNotCell {
        target: BoundaryInterval,
    },
    CellAlreadyClosed {
        cell: BoundaryInterval,
    },
    EmptyReplacement {
        target: BoundaryInterval,
    },
    CellOutsideTarget {
        target: BoundaryInterval,
        cell: BoundaryInterval,
    },
    CoverGapOrOverlap {
        target: BoundaryInterval,
        expected_start: i64,
        actual_start: i64,
    },
    CoverWrongEnd {
        target: BoundaryInterval,
        actual_end: i64,
    },
    CardinalityOverflow,
    CardinalityMismatch {
        declared: u128,
        covered: u128,
    },
    NonCanonicalProbe {
        cell: BoundaryInterval,
        expected: i64,
        actual: i64,
    },
    ProbeAlreadyRecorded {
        point: i64,
    },
    InvalidRecordedProbe {
        cell: BoundaryInterval,
        point: i64,
    },
    OpenFrontierEmpty,
    OpenFrontierFullyProbed,
}

impl fmt::Display for BoundaryPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterval {
                start,
                end_exclusive,
            } => write!(
                formatter,
                "invalid boundary interval [{start}, {end_exclusive}): start exceeds end"
            ),
            Self::EmptyDomainHasCells { cell_count } => write!(
                formatter,
                "empty boundary domain unexpectedly contains {cell_count} cells"
            ),
            Self::EmptyCell { interval } => write!(
                formatter,
                "boundary partition contains empty cell [{}, {})",
                interval.start, interval.end_exclusive
            ),
            Self::TargetIsNotCell { target } => write!(
                formatter,
                "boundary refinement target [{}, {}) is not one exact current cell",
                target.start, target.end_exclusive
            ),
            Self::CellAlreadyClosed { cell } => write!(
                formatter,
                "closed boundary cell [{}, {}) cannot be refined or reopened",
                cell.start, cell.end_exclusive
            ),
            Self::EmptyReplacement { target } => write!(
                formatter,
                "open boundary cell [{}, {}) requires a nonempty exact replacement cover",
                target.start, target.end_exclusive
            ),
            Self::CellOutsideTarget { target, cell } => write!(
                formatter,
                "replacement cell [{}, {}) lies outside target [{}, {})",
                cell.start, cell.end_exclusive, target.start, target.end_exclusive
            ),
            Self::CoverGapOrOverlap {
                target,
                expected_start,
                actual_start,
            } => write!(
                formatter,
                "replacement cover for [{}, {}) expected next cell at {}, found {}",
                target.start, target.end_exclusive, expected_start, actual_start
            ),
            Self::CoverWrongEnd { target, actual_end } => write!(
                formatter,
                "replacement cover for [{}, {}) ends at {}",
                target.start, target.end_exclusive, actual_end
            ),
            Self::CardinalityOverflow => {
                write!(formatter, "boundary-plan cardinality arithmetic overflowed")
            }
            Self::CardinalityMismatch { declared, covered } => write!(
                formatter,
                "boundary plan covers {covered} integer points but declares {declared}"
            ),
            Self::NonCanonicalProbe {
                cell,
                expected,
                actual,
            } => write!(
                formatter,
                "probe {} is not canonical for [{}, {}); expected {}",
                actual, cell.start, cell.end_exclusive, expected
            ),
            Self::ProbeAlreadyRecorded { point } => {
                write!(formatter, "boundary point {point} has already been probed")
            }
            Self::InvalidRecordedProbe { cell, point } => write!(
                formatter,
                "recorded probe {} is invalid for scheduled cell [{}, {})",
                point, cell.start, cell.end_exclusive
            ),
            Self::OpenFrontierEmpty => {
                write!(formatter, "boundary plan has no remaining open frontier")
            }
            Self::OpenFrontierFullyProbed => write!(
                formatter,
                "every point in the remaining open boundary frontier has already been probed"
            ),
        }
    }
}

impl std::error::Error for BoundaryPlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    type TestPlan = BoundaryPlan<&'static str, &'static str, &'static str, &'static str>;
    type TestCell = BoundaryCell<&'static str, &'static str, &'static str>;

    fn interval(start: i64, end_exclusive: i64) -> BoundaryInterval {
        BoundaryInterval::new(start, end_exclusive).unwrap()
    }

    fn closed(
        start: i64,
        end_exclusive: i64,
        classification: &'static str,
        signature: &'static str,
    ) -> TestCell {
        TestCell::closed(
            interval(start, end_exclusive),
            classification,
            signature,
            "exact-certificate",
        )
    }

    fn open(start: i64, end_exclusive: i64) -> TestCell {
        TestCell::open(interval(start, end_exclusive))
    }

    #[test]
    fn probes_alone_never_close_a_region() {
        let mut plan = TestPlan::new(0, 16).unwrap();
        let before = plan.validate().unwrap();

        let request = plan
            .record_next_midpoint_probe("runtime-observation")
            .unwrap();

        assert_eq!(request.point(), 8);
        assert_eq!(plan.probes().len(), 1);
        assert_eq!(plan.probes()[0].evidence(), &"runtime-observation");
        assert_eq!(
            plan.open_frontier(),
            vec![interval(0, 8), interval(8, 9), interval(9, 16)]
        );
        assert_eq!(plan.validate().unwrap(), before);
        assert_eq!(before.open(), 16);
        assert_eq!(before.closed(), 0);
    }

    #[test]
    fn successive_midpoint_probes_refine_without_repeating_points() {
        let mut plan = TestPlan::new(0, 16).unwrap();

        let first = plan.record_next_midpoint_probe("first").unwrap();
        let second = plan.record_next_midpoint_probe("second").unwrap();
        let third = plan.record_next_midpoint_probe("third").unwrap();

        assert_eq!([first.point(), second.point(), third.point()], [8, 4, 12]);
        assert_eq!(plan.validate().unwrap().open(), 16);
        assert_eq!(plan.validate().unwrap().closed(), 0);
        assert_eq!(
            plan.probes()
                .iter()
                .map(|probe| probe.request().point())
                .collect::<Vec<_>>(),
            vec![8, 4, 12]
        );
    }

    #[test]
    fn exhausted_probe_schedule_keeps_an_explicit_open_frontier() {
        let mut plan = TestPlan::new(0, 3).unwrap();
        plan.record_next_midpoint_probe("middle").unwrap();
        plan.record_next_midpoint_probe("left").unwrap();
        plan.record_next_midpoint_probe("right").unwrap();

        assert_eq!(plan.next_midpoint_probe(), None);
        assert_eq!(
            plan.record_next_midpoint_probe("duplicate").unwrap_err(),
            BoundaryPlanError::OpenFrontierFullyProbed
        );
        assert_eq!(plan.validate().unwrap().open(), 3);
        assert_eq!(plan.open_frontier().len(), 3);
    }

    #[test]
    fn binary_midpoint_choice_is_overflow_safe_and_deterministic() {
        assert_eq!(interval(0, 10).canonical_midpoint(), Some(5));
        assert_eq!(interval(-10, 0).canonical_midpoint(), Some(-5));
        assert_eq!(interval(4, 5).canonical_midpoint(), Some(4));
        assert_eq!(interval(i64::MIN, i64::MAX).canonical_midpoint(), Some(-1));

        let mut plan = TestPlan::new(0, 10).unwrap();
        plan.refine_open(interval(0, 10), [open(0, 4), open(4, 10)])
            .unwrap();
        let request = plan.next_midpoint_probe().unwrap();
        assert_eq!(request.cell(), interval(4, 10));
        assert_eq!(request.point(), 7);
    }

    #[test]
    fn exact_split_conserves_the_declared_domain() {
        let mut plan = TestPlan::new(0, 10).unwrap();
        let summary = plan
            .refine_open(
                interval(0, 10),
                [
                    closed(0, 3, "match", "left"),
                    open(3, 7),
                    closed(7, 10, "no-match", "right"),
                ],
            )
            .unwrap();

        assert_eq!(summary.declared(), 10);
        assert_eq!(summary.open(), 4);
        assert_eq!(summary.closed(), 6);
        assert_eq!(plan.open_frontier(), vec![interval(3, 7)]);
        assert_eq!(plan.cells().len(), 3);

        let before = plan.clone();
        assert!(plan
            .refine_open(interval(3, 7), [open(3, 5), open(6, 7)])
            .is_err());
        assert_eq!(plan, before, "an invalid cover must be atomic");
    }

    #[test]
    fn closed_cells_cannot_reopen_or_change() {
        let mut plan = TestPlan::new(0, 10).unwrap();
        plan.refine_open(
            interval(0, 10),
            [closed(0, 5, "match", "stable"), open(5, 10)],
        )
        .unwrap();
        let before = plan.clone();

        let error = plan.refine_open(interval(0, 5), [open(0, 5)]).unwrap_err();

        assert_eq!(
            error,
            BoundaryPlanError::CellAlreadyClosed {
                cell: interval(0, 5)
            }
        );
        assert_eq!(plan, before);

        let error = plan
            .refine_open(
                interval(0, 5),
                [closed(0, 5, "no-match", "different-mechanism")],
            )
            .unwrap_err();
        assert_eq!(
            error,
            BoundaryPlanError::CellAlreadyClosed {
                cell: interval(0, 5)
            }
        );
        assert_eq!(plan, before);
    }

    #[test]
    fn increasing_refinement_only_shrinks_open_support() {
        let mut plan = TestPlan::new(0, 20).unwrap();
        let initial = plan.validate().unwrap();
        plan.refine_open(
            interval(0, 20),
            [open(0, 5), closed(5, 15, "match", "middle"), open(15, 20)],
        )
        .unwrap();
        let first = plan.validate().unwrap();
        let first_frontier = plan.open_frontier();

        plan.refine_open(
            interval(0, 5),
            [closed(0, 2, "no-match", "prefix"), open(2, 5)],
        )
        .unwrap();
        let second = plan.validate().unwrap();

        assert!(initial.open() >= first.open());
        assert!(first.open() >= second.open());
        assert!(plan.open_frontier().iter().all(|new_cell| {
            first_frontier.iter().any(|old_cell| {
                new_cell.start() >= old_cell.start()
                    && new_cell.end_exclusive() <= old_cell.end_exclusive()
            })
        }));
    }

    #[test]
    fn non_monotone_classification_requires_multiple_cells() {
        let truth = [
            "match", "match", "match", "none", "none", "none", "match", "match", "match",
        ];
        let has_sound_single_threshold = (0..=truth.len()).any(|split| {
            truth[..split].iter().all(|value| *value == "match")
                && truth[split..].iter().all(|value| *value == "none")
        }) || (0..=truth.len()).any(|split| {
            truth[..split].iter().all(|value| *value == "none")
                && truth[split..].iter().all(|value| *value == "match")
        });
        assert!(!has_sound_single_threshold);

        let mut plan = TestPlan::new(0, 9).unwrap();
        plan.refine_open(
            interval(0, 9),
            [
                closed(0, 3, "match", "left-path"),
                closed(3, 6, "none", "middle-path"),
                closed(6, 9, "match", "right-path"),
            ],
        )
        .unwrap();

        for (point, expected) in truth.into_iter().enumerate() {
            let evidence = plan
                .cell_containing(point as i64)
                .and_then(TestCell::closed_evidence)
                .unwrap();
            assert_eq!(*evidence.0, expected);
        }
        assert_eq!(plan.cells().len(), 3);
        assert!(plan.open_frontier().is_empty());
    }

    #[test]
    fn disconnected_cells_may_share_one_mechanism_signature() {
        let mut plan = TestPlan::new(0, 8).unwrap();
        plan.refine_open(
            interval(0, 8),
            [
                closed(0, 2, "match", "shared-mechanism"),
                open(2, 6),
                closed(6, 8, "match", "shared-mechanism"),
            ],
        )
        .unwrap();

        let signatures = plan
            .cells()
            .iter()
            .filter_map(TestCell::closed_evidence)
            .map(|(_, signature, _)| *signature)
            .collect::<Vec<_>>();
        assert_eq!(signatures, vec!["shared-mechanism", "shared-mechanism"]);
        assert_eq!(plan.open_frontier(), vec![interval(2, 6)]);
        assert_eq!(plan.validate().unwrap().closed(), 4);
    }
}
